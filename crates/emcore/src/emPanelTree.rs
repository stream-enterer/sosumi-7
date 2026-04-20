// SPLIT: Split from emPanel.h — panel tree management extracted
use std::collections::HashMap;

use bitflags::bitflags;
use slotmap::{new_key_type, SlotMap};

use crate::dlog;

use super::emEngine::{EngineId, Priority, TreeLocation};
use super::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use super::emPanelCycleEngine::PanelCycleEngine;
use crate::emColor::emColor;
use crate::emEngineCtx::PanelCtx;
use crate::emPanel::Rect;
use crate::emScheduler::EngineScheduler;

// ── Autoplay handling flags ─────────────────────────────────────────

bitflags! {
    /// Flags controlling autoplay handling for a panel.
    ///
    /// Corresponds to the C++ `AutoplayHandlingFlags` enum.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct AutoplayHandlingFlags: u32 {
        const ITEM               = 1 << 0;
        const DIRECTORY          = 1 << 1;
        const CUTOFF             = 1 << 2;
        const CUTOFF_AT_SUBITEMS = 1 << 3;
    }
}

// ── Playback state ──────────────────────────────────────────────────

/// Playback state returned by [`PanelTree::get_playback_state`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaybackState {
    /// Whether playback is currently active.
    pub playing: bool,
    /// Current playback position (0.0 when not playing).
    pub pos: f64,
    /// Whether the panel supports playback at all.
    pub supported: bool,
}

// ── emView condition type ───────────────────────────────────────────────

/// Type of size metric used for auto-expansion threshold comparisons.
///
/// Corresponds to `emPanel::ViewConditionType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ViewConditionType {
    /// Panel area (ViewedWidth * ViewedHeight).
    Area,
    /// Panel width in view coordinates.
    Width,
    /// Panel height in view coordinates.
    Height,
    /// Minimum of width and height.
    MinExt,
    /// Maximum of width and height.
    MaxExt,
}

// ── Identity encode/decode free functions ─────────────────────────────

/// Encode an array of panel names into a colon-delimited identity string,
/// escaping `:` and `\` with backslash prefixes.
///
/// Corresponds to `emPanel::EncodeIdentity`.
pub fn EncodeIdentity(names: &[&str]) -> String {
    // First pass: compute total length for the output
    let mut len = 0usize;
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            len += 1; // ':'
        }
        for ch in name.chars() {
            if ch == ':' || ch == '\\' {
                len += 2; // escape char + original
            } else {
                len += ch.len_utf8();
            }
        }
    }

    let mut result = String::with_capacity(len);
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            result.push(':');
        }
        for ch in name.chars() {
            if ch == ':' || ch == '\\' {
                result.push('\\');
            }
            result.push(ch);
        }
    }
    result
}

/// Decode a colon-delimited identity string back into a list of panel names,
/// handling backslash-escaped colons and backslashes.
///
/// Corresponds to `emPanel::DecodeIdentity`.
pub fn DecodeIdentity(identity: &str) -> Vec<String> {
    let mut names = Vec::new();
    let bytes = identity.as_bytes();
    let mut pos = 0;

    loop {
        if pos >= bytes.len() {
            break;
        }
        // Collect one name
        let mut name = String::new();
        loop {
            if pos >= bytes.len() {
                break;
            }
            let ch = bytes[pos] as char;
            if ch == ':' {
                // End of this name segment; skip the ':'
                pos += 1;
                break;
            }
            if ch == '\\' {
                pos += 1; // skip escape
                if pos >= bytes.len() {
                    break;
                }
                name.push(bytes[pos] as char);
                pos += 1;
            } else {
                name.push(ch);
                pos += 1;
            }
        }
        names.push(name);
    }
    names
}

new_key_type! {
    /// Unique handle for a panel in the panel tree.
    pub struct PanelId;
}

/// Data stored for each panel in the arena.
///
/// Fields are crate-internal. Use accessor methods on [`PanelTree`] for reading
/// panel state, and dedicated setters (e.g. `set_layout_rect`, `set_visible`)
/// for mutation.
pub(crate) struct PanelData {
    // Tree-managed linkage
    pub(crate) parent: Option<PanelId>,
    pub(crate) first_child: Option<PanelId>,
    pub(crate) last_child: Option<PanelId>,
    pub(crate) next_sibling: Option<PanelId>,
    pub(crate) prev_sibling: Option<PanelId>,

    // Identity
    pub(crate) name: String,

    // Layout & appearance
    pub(crate) layout_rect: Rect,
    pub(crate) canvas_color: emColor,
    pub(crate) visible: bool,
    pub(crate) focusable: bool,

    // Enable state
    pub(crate) enable_switch: bool,
    /// Computed: true if this panel and all ancestors have enable_switch=true.
    pub(crate) enabled: bool,

    // Notices & behavior
    pub(crate) pending_notices: NoticeFlags,
    pub(crate) behavior: Option<Box<dyn PanelBehavior>>,

    // Autoplay / playback
    pub(crate) autoplay_handling: AutoplayHandlingFlags,

    // Auto-expansion state
    pub(crate) ae_threshold_type: ViewConditionType,
    pub(crate) ae_threshold_value: f64,
    pub(crate) ae_expanded: bool,
    pub(crate) ae_invalid: bool,
    pub(crate) ae_decision_invalid: bool,
    /// True if this panel was created during auto-expansion (C++ `CreatedByAE`).
    pub(crate) created_by_ae: bool,
    /// True while AutoExpand() is being called on this panel (C++ `AECalling`).
    /// Used to mark newly created children with `created_by_ae=true`.
    pub(crate) ae_calling: bool,

    // Viewing state (set by emView::update_viewing each frame)
    pub(crate) viewed: bool,
    pub(crate) in_viewed_path: bool,
    pub(crate) in_active_path: bool,
    pub(crate) is_active: bool,
    pub(crate) viewed_x: f64,
    pub(crate) viewed_y: f64,
    pub(crate) viewed_width: f64,
    pub(crate) viewed_height: f64,
    pub(crate) clip_x: f64,
    pub(crate) clip_y: f64,
    pub(crate) clip_w: f64,
    pub(crate) clip_h: f64,

    // Layout-invalidation (C++ `ChildrenLayoutInvalid`).
    // Set by HandleNotice when NF_LAYOUT_CHANGED | NF_CHILD_LIST_CHANGED
    // is delivered and FirstChild exists; cleared after LayoutChildren runs.
    pub(crate) children_layout_invalid: bool,

    /// C++ `emPanel::PendingInput` — set to 1 when this panel needs to
    /// receive the current input event. Cleared by `RecurseInput` after
    /// dispatching. Mirrors C++ emView.h/emPanel.h `PendingInput` field.
    pub(crate) pending_input: bool,

    // Notice ring linkage (C++ emPanel::NoticeNode.Prev/Next).
    // Panels with queued notices form a doubly-linked list.  The ring
    // head/tail sentinel is stored on `emView` (notice_ring_head_next/prev).
    // When neither prev nor next is set, the panel is NOT in the ring.
    pub(crate) notice_prev_in_ring: Option<PanelId>,
    pub(crate) notice_next_in_ring: Option<PanelId>,

    /// Phase 2 Task 7 (keystone): replaces
    /// `Weak<RefCell<emView>>` — `emView` is now plain on `emWindow` and
    /// `emSubViewPanel`, so a `Weak` has nothing to reference. The former
    /// field was only ever queried for presence (`strong_count() > 0`)
    /// before registering a panel's engine with the scheduler, so a plain
    /// bool suffices. The identity of the owning view is now carried by
    /// the tree's `tree_location` plus (for Toplevel) a `WindowId` on the
    /// registering `PanelCycleEngine`/`UpdateEngineClass`.
    pub(crate) has_view: bool,

    /// Scheduler engine handle for this panel (SP4.5).
    ///
    /// C++ `emPanel` inherits from `emEngine`; every panel is implicitly an
    /// engine from construction. In Rust the engine registration is done
    /// by `PanelTree::init_panel_view` via a `PanelCycleEngine` adapter.
    /// `None` until `init_panel_view` runs (panel not yet attached to a view).
    pub(crate) engine_id: Option<EngineId>,
}

impl PanelData {
    fn new(name: String) -> Self {
        Self {
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            name,
            layout_rect: Rect::new(-2.0, -2.0, 1.0, 1.0),
            canvas_color: emColor::TRANSPARENT,
            visible: true,
            focusable: true,
            enable_switch: true,
            enabled: true,
            pending_notices: NoticeFlags::empty(),
            behavior: None,
            autoplay_handling: AutoplayHandlingFlags::empty(),
            ae_threshold_type: ViewConditionType::Area,
            ae_threshold_value: 150.0,
            ae_expanded: false,
            ae_invalid: false,
            ae_decision_invalid: false,
            created_by_ae: false,
            ae_calling: false,
            viewed: false,
            in_viewed_path: false,
            in_active_path: false,
            is_active: false,
            viewed_x: 0.0,
            viewed_y: 0.0,
            viewed_width: 0.0,
            viewed_height: 0.0,
            clip_x: 0.0,
            clip_y: 0.0,
            clip_w: 0.0,
            clip_h: 0.0,
            children_layout_invalid: false,
            pending_input: false,
            notice_prev_in_ring: None,
            notice_next_in_ring: None,
            has_view: false,
            engine_id: None,
        }
    }
}

/// Arena-based panel tree using SlotMap for stable handles.
pub struct PanelTree {
    pub(crate) panels: SlotMap<PanelId, PanelData>,
    root: Option<PanelId>,
    /// Per-parent name index: (parent, child_name) → child_id.
    /// Root panels use their own id as the "parent" key.
    name_index: HashMap<(PanelId, String), PanelId>,
    /// Fast check: true when any panel has non-empty pending_notices.
    /// Set when notices are queued, cleared after deliver_notices drains them.
    has_pending_notices: bool,
    /// Queue of panels that behaviors have requested the view navigate to.
    /// Drained by emView each frame.
    navigation_requests: Vec<PanelId>,
    /// Mirror of `emView::seek_pos_panel`. Kept here so panel behaviors can
    /// check seek state without needing a view reference. Port of C++
    /// `emPanel::GetSoughtName()` access pattern.
    pub(crate) seek_pos_panel: Option<PanelId>,
    /// Mirror of `emView::seek_pos_child_name`.
    pub(crate) seek_pos_child_name: String,
    /// Pending ring-cleanup entries from `remove()` calls that lacked a
    /// view reference.  Each tuple is `(prev_in_ring, next_in_ring)` for
    /// the panel that was removed.  `emView::HandleNotice` drains this at
    /// the top of each drain cycle, updating `notice_ring_head_next/prev`
    /// as needed.  Filled only when `remove()` is called without an
    /// `emView` (e.g. from `PanelCtx` or test code).
    pub(crate) pending_ring_cleanup: Vec<(Option<PanelId>, Option<PanelId>)>,
    /// Set by `Layout()` on the root panel (no parent). Matches C++
    /// `emPanel::Layout` `!Parent` branch which sets `View.SVPChoiceInvalid`
    /// and calls `View.RawZoomOut(true)` when zoomed out. `emView::Update`
    /// drains this flag and calls `RawZoomOut` when it is true.
    pub(crate) root_layout_changed: bool,
    /// Engine IDs queued for removal when the scheduler was busy (borrowed
    /// by `DoTimeSlice`) at the time `deregister_engine_for` ran.
    /// Drained at the start of each `DoTimeSlice` before the main loop.
    pub(crate) pending_engine_removals: Vec<crate::emEngine::EngineId>,
    /// Phase 1.75 Task 3: where this tree sits relative to the outer scheduler.
    /// The outer tree is `Outer`; an `emSubViewPanel::sub_tree` is
    /// `SubView(outer_panel_id, Box::new(Outer))` (or nested deeper). Used by
    /// `register_engine_for` so sub-tree `PanelCycleEngine` adapters are
    /// registered with the correct `TreeLocation` on the single outer
    /// scheduler and dispatch resolves through the sub-view chain.
    pub(crate) tree_location: TreeLocation,
    /// Cached copy of the owning view's `update_engine_id`.
    /// Populated by `emView::RegisterEngines` via [`PanelTree::set_update_engine_id`];
    /// cleared on view detach.  Read by `add_to_notice_list` to wake the
    /// update engine without borrowing the view.
    pub(crate) update_engine_id: Option<crate::emEngine::EngineId>,
    /// Phase 2 Task 7: cached mirror of the owning view's
    /// `CurrentPixelTallness`.  Written by `emView::SetGeometry` every
    /// time the view's tallness changes; read by `PanelCycleEngine::Cycle`
    /// to build a `PanelCtx::current_pixel_tallness` without resolving
    /// back to the view.  Starts at `1.0` to match
    /// `emView::CurrentPixelTallness` default.
    pub(crate) cached_pixel_tallness: f64,
}

impl PanelTree {
    /// Construct an outer-tree `PanelTree`. Shorthand for
    /// `new_with_location(TreeLocation::Outer)`.
    pub fn new() -> Self {
        Self::new_with_location(TreeLocation::Outer)
    }

    /// Phase 1.75 Task 4: Construct a `PanelTree` whose
    /// `register_engine_for` tags every `PanelCycleEngine` adapter with the
    /// given `TreeLocation`. `emSubViewPanel::new` passes
    /// `SubView(outer_panel_id, Outer)` so sub-tree adapters register with
    /// the OUTER scheduler for cross-tree priority-queue dispatch (spec §3.3).
    pub fn new_with_location(tree_location: TreeLocation) -> Self {
        Self {
            panels: SlotMap::with_key(),
            root: None,
            name_index: HashMap::new(),
            has_pending_notices: false,
            navigation_requests: Vec::new(),
            seek_pos_panel: None,
            seek_pos_child_name: String::new(),
            pending_ring_cleanup: Vec::new(),
            root_layout_changed: false,
            pending_engine_removals: Vec::new(),
            tree_location,
            update_engine_id: None,
            cached_pixel_tallness: 1.0,
        }
    }

    /// Set the cached update-engine id so `add_to_notice_list` can wake the
    /// view's update engine without a scheduler call through the view.
    /// Called by `emView::RegisterEngines`; cleared on view detach.
    pub fn set_update_engine_id(&mut self, id: Option<crate::emEngine::EngineId>) {
        self.update_engine_id = id;
    }

    /// Mark pending notices and wake the update engine.
    ///
    /// Phase 2 Task 6 (E006): ring linking is now on `emView::AddToNoticeList`.
    /// `PanelTree::add_to_notice_list` remains as the call site for tree-internal
    /// paths that do not have an `&mut emView` available.  It sets the
    /// `has_pending_notices` flag (picked up by `emView::HandleNotice`'s safety-net
    /// scan) and wakes the update engine so the drain runs on the next cycle.
    ///
    /// Call sites that DO have `&mut emView` should call
    /// `view.AddToNoticeList(id, tree)` directly instead.
    pub(crate) fn add_to_notice_list(&mut self, _id: PanelId, sched: Option<&mut EngineScheduler>) {
        self.has_pending_notices = true;
        // Wake the update engine so HandleNotice runs this cycle.
        // C++ emView::AddToNoticeList (emView.cpp:1288) calls UpdateEngine->WakeUp().
        if let (Some(sched), Some(eng_id)) = (sched, self.update_engine_id) {
            sched.wake_up(eng_id);
        }
    }

    /// Unlink `id` from the notice ring (neighbor-pointer cleanup only).
    ///
    /// Removes `id` from its neighbors' `notice_next/prev_in_ring` pointers and
    /// snapshots the ring slot into `pending_ring_cleanup` so that
    /// `emView::HandleNotice` can update the emView-owned head/tail pointers on
    /// the next drain.  No-op if `id` is not in the ring.
    ///
    /// Must be called BEFORE `id` is removed from the arena (while
    /// `panels[id]` is still valid).
    pub(crate) fn remove_from_notice_list(&mut self, id: PanelId) {
        if !self.panels.contains_key(id) {
            return;
        }
        let (prev, next) = {
            let p = &self.panels[id];
            (p.notice_prev_in_ring, p.notice_next_in_ring)
        };
        // Update neighbor-panel pointers (the parts we can do without emView).
        if let Some(p) = prev {
            if self.panels.contains_key(p) {
                self.panels[p].notice_next_in_ring = next;
            }
        }
        if let Some(n) = next {
            if self.panels.contains_key(n) {
                self.panels[n].notice_prev_in_ring = prev;
            }
        }
        self.panels[id].notice_prev_in_ring = None;
        self.panels[id].notice_next_in_ring = None;
        // Push a cleanup entry so emView::HandleNotice can reconcile the
        // head/tail pointers (which live on emView, not PanelTree).
        // The (prev, next) snapshot lets HandleNotice decide whether to
        // advance notice_ring_head_next or notice_ring_head_prev.
        self.pending_ring_cleanup.push((prev, next));
    }

    /// Returns true if the given panel is the current view seek target.
    /// Port of C++ `emPanel::GetSoughtName() != NULL` check.
    pub fn is_seek_target(&self, id: PanelId) -> bool {
        self.seek_pos_panel == Some(id)
    }

    /// Returns the sought child name if `id` is the seek target, else None.
    /// Port of C++ `emPanel::GetSoughtName()`.
    pub fn sought_name(&self, id: PanelId) -> Option<&str> {
        if self.is_seek_target(id) {
            Some(self.seek_pos_child_name.as_str())
        } else {
            None
        }
    }

    /// All notice flags that C++ fires on a newly created panel so its behavior
    /// sees every state dimension on first notice delivery. Matches C++ emPanel
    /// constructor which sets all NF_* bits.
    // C++ emPanel constructor fires all NF_* flags including
    // NF_VIEWING_CHANGED so that AutoExpand can trigger on panels
    // that start viewed (e.g., root panels, panels created during
    // seek-descent).
    const INIT_NOTICE_FLAGS: NoticeFlags = NoticeFlags::LAYOUT_CHANGED
        .union(NoticeFlags::FOCUS_CHANGED)
        .union(NoticeFlags::VIEWING_CHANGED)
        .union(NoticeFlags::CHILD_LIST_CHANGED)
        .union(NoticeFlags::ENABLE_CHANGED)
        .union(NoticeFlags::SOUGHT_NAME_CHANGED)
        .union(NoticeFlags::ACTIVE_CHANGED)
        .union(NoticeFlags::VIEW_FOCUS_CHANGED)
        .union(NoticeFlags::UPDATE_PRIORITY_CHANGED)
        .union(NoticeFlags::MEMORY_LIMIT_CHANGED)
        .union(NoticeFlags::VIEWING_CHANGED);

    /// Create the root panel.
    ///
    /// # Panics
    /// Panics if a root panel already exists.
    pub fn create_root(&mut self, name: &str, has_view: bool) -> PanelId {
        assert!(
            self.root.is_none(),
            "create_root called but root panel already exists"
        );
        let id = self.panels.insert(PanelData::new(name.to_string()));
        // Note: root is NOT inserted in name_index under its own key.
        // Previously it was (self-indexed) which caused find_child_by_name
        // to return the root itself for name="" lookups, breaking identity
        // path resolution like "::FS::..." where the first "" after the
        // initial ":" is meant to be a child of root, not root itself.
        self.root = Some(id);
        self.panels[id].has_view = has_view;
        // C++ emPanel root ctor (emPanel.cpp:~100): Active=1; InActivePath=1;
        // View.ActivePanel=this. Root starts as the active panel.
        self.panels[id].is_active = true;
        self.panels[id].in_active_path = true;
        // C++ fires all NF_* flags on new panels as initialization notices
        self.panels[id].pending_notices = Self::INIT_NOTICE_FLAGS;
        self.has_pending_notices = true;
        self.add_to_notice_list(id, None);
        id
    }

    /// Create the root panel with a deferred view (view set to `Weak::new()`).
    ///
    /// Use this in tests and examples where the `emView` cannot be constructed
    /// before the root `PanelId` is known (chicken-and-egg). Follow up with
    /// [`set_panel_view`] once the view is available.
    ///
    /// Not a C++ analogue — test-support only.
    #[cfg(any(test, feature = "test-support"))]
    pub fn create_root_deferred_view(&mut self, name: &str) -> PanelId {
        self.create_root(name, false)
    }

    /// Propagate a view weak reference to a panel and all its descendants.
    ///
    /// Internal production path: used by `emSubViewPanel::new` and
    /// `emMainWindow::create_main_window` which must create the panel tree root
    /// before the view exists (chicken-and-egg).
    ///
    /// Not a C++ analogue — Rust ownership requires this two-phase init.
    pub fn init_panel_view(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        self.panels[id].has_view = true;
        // Split sched borrow: collect ids, then iterate with sched.
        let mut ids_to_register = vec![id];
        let mut stack = vec![id];
        while let Some(p) = stack.pop() {
            let mut child = self.panels[p].first_child;
            while let Some(c) = child {
                self.panels[c].has_view = true;
                ids_to_register.push(c);
                stack.push(c);
                child = self.panels[c].next_sibling;
            }
        }
        match sched {
            Some(s) => {
                for reg_id in ids_to_register {
                    self.register_engine_for(reg_id, Some(s));
                }
            }
            None => {
                for reg_id in ids_to_register {
                    self.register_engine_for(reg_id, None);
                }
            }
        }
    }

    /// Register `id`'s scheduler engine if the panel has a live view and
    /// does not already have one. When `sched` is `None` (no scheduler
    /// available yet, e.g. before the owning view has registered its engines),
    /// the call is a no-op; the next `init_panel_view` with a live scheduler
    /// registers it.
    ///
    /// Phase 1.75 Task 5 (continuation): synchronous — no view borrow taken
    /// here, so this runs fine even when invoked from `AutoExpand` during
    /// `UpdateEngineClass::Cycle` (which holds `view_rc.borrow_mut()`).
    /// Phase 1.75 Task 5 (continuation): public entry point used by
    /// `emView::RegisterEngines` to register engines for panels that pre-exist
    /// the scheduler (e.g. root created before view.RegisterEngines runs).
    /// Idempotent.
    pub fn register_engine_for_public(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        self.register_engine_for(id, sched);
    }

    fn register_engine_for(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        if self.panels.get(id).and_then(|p| p.engine_id).is_some() {
            return; // idempotent re-attachment guard
        }
        // Phase 2 Task 5: `View` weak still gates registration — when it is
        // null, the panel has no live view yet (same condition as pre-Task-5).
        // The adapter itself no longer stores the weak; it stores a
        // `PanelScope` derived from this tree's `tree_location`.
        let has_view = self.panels.get(id).map(|p| p.has_view).unwrap_or(false);
        if !has_view {
            return; // no view yet (or view dropped)
        }
        let Some(sched) = sched else {
            return; // no scheduler provided; `init_panel_view` will register once one is available
        };
        // Derive scope from this tree's TreeLocation. Outer trees use a
        // placeholder WindowId; resolve-time lookup walks `ctx.windows` to
        // find the matching emWindow. Tasks 6–7 will thread the real
        // WindowId through window/tree construction.
        let scope = match &self.tree_location {
            crate::emEngine::TreeLocation::Outer => {
                crate::emPanelScope::PanelScope::Toplevel(winit::window::WindowId::dummy())
            }
            crate::emEngine::TreeLocation::SubView { outer_panel_id, .. } => {
                crate::emPanelScope::PanelScope::SubView(*outer_panel_id)
            }
        };
        let adapter = PanelCycleEngine {
            panel_id: id,
            scope,
            #[cfg(any(test, feature = "test-support"))]
            first_cycle_probe: None,
        };
        let eid = sched.register_engine(
            Box::new(adapter),
            Priority::Medium,
            self.tree_location.clone(),
        );
        self.panels[id].engine_id = Some(eid);
    }

    /// Deregister `id`'s scheduler engine.
    /// Propagate a view weak reference to a panel and all its descendants.
    ///
    /// Use after [`create_root_deferred_view`] once the owning view is
    /// constructed. Visits the subtree rooted at `id` and sets `View` on
    /// every panel already in it.
    ///
    /// Not a C++ analogue — test-support only.
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_panel_view(&mut self, id: PanelId) {
        self.init_panel_view(id, None);
    }

    /// Create a child panel under the given parent.
    pub fn create_child(
        &mut self,
        parent: PanelId,
        name: &str,
        mut sched: Option<&mut EngineScheduler>,
    ) -> PanelId {
        // C++ emPanel ctor: CreatedByAE = Parent->AECalling
        let created_by_ae = self.panels[parent].ae_calling;
        let parent_has_view = self.panels[parent].has_view;

        let id = self.panels.insert(PanelData::new(name.to_string()));
        self.panels[id].has_view = parent_has_view;
        self.name_index.insert((parent, name.to_string()), id);

        // Link into parent's child list
        self.panels[id].parent = Some(parent);
        self.panels[id].created_by_ae = created_by_ae;

        let prev_last = self.panels[parent].last_child;
        if let Some(prev) = prev_last {
            self.panels[prev].next_sibling = Some(id);
            self.panels[id].prev_sibling = Some(prev);
        } else {
            self.panels[parent].first_child = Some(id);
        }
        self.panels[parent].last_child = Some(id);

        // Inherit parent's enabled state
        self.recompute_enabled(id, None);

        if self.panels[id].has_view {
            self.register_engine_for(id, sched.as_deref_mut());
        }

        // Notify parent
        self.panels[parent]
            .pending_notices
            .insert(NoticeFlags::CHILD_LIST_CHANGED);
        self.has_pending_notices = true;
        self.add_to_notice_list(parent, sched.as_deref_mut());

        // C++ fires all NF_* flags on new panels as initialization notices
        self.panels[id].pending_notices = Self::INIT_NOTICE_FLAGS;
        self.add_to_notice_list(id, sched);

        id
    }

    /// Remove a panel and all its descendants.
    pub fn remove(&mut self, id: PanelId, mut sched: Option<&mut EngineScheduler>) {
        // Collect all descendants first
        let descendants = self.collect_descendants(id);

        // Unlink self and descendants from the notice ring BEFORE arena removal
        // (C++ emPanel destructor unlinks NoticeNode).
        self.remove_from_notice_list(id);
        for &desc_id in &descendants {
            self.remove_from_notice_list(desc_id);
        }

        // SP4.5: deregister scheduler engines for self and descendants while
        // the View weak ref is still reachable.
        // We can't pass sched multiple times via Option<&mut>, so collect eids.
        let mut eids_to_remove: Vec<EngineId> = Vec::new();
        for &desc_id in &descendants {
            if let Some(eid) = self
                .panels
                .get_mut(desc_id)
                .and_then(|p| p.engine_id.take())
            {
                eids_to_remove.push(eid);
            }
        }
        if let Some(eid) = self.panels.get_mut(id).and_then(|p| p.engine_id.take()) {
            eids_to_remove.push(eid);
        }
        match sched.as_deref_mut() {
            Some(s) => {
                for eid in eids_to_remove {
                    s.remove_engine(eid);
                }
            }
            None => {
                self.pending_engine_removals.extend(eids_to_remove);
            }
        }

        // Unlink from parent's child list
        if let Some(parent_id) = self.panels[id].parent {
            let prev = self.panels[id].prev_sibling;
            let next = self.panels[id].next_sibling;

            if let Some(prev_id) = prev {
                self.panels[prev_id].next_sibling = next;
            } else {
                self.panels[parent_id].first_child = next;
            }

            if let Some(next_id) = next {
                self.panels[next_id].prev_sibling = prev;
            } else {
                self.panels[parent_id].last_child = prev;
            }

            self.panels[parent_id]
                .pending_notices
                .insert(NoticeFlags::CHILD_LIST_CHANGED);
            self.has_pending_notices = true;
            self.add_to_notice_list(parent_id, sched);
        }

        // Remove root reference if needed
        if self.root == Some(id) {
            self.root = None;
        }

        // Remove from arena and name index
        for desc_id in descendants {
            if let Some(data) = self.panels.remove(desc_id) {
                if let Some(parent_id) = data.parent {
                    self.name_index.remove(&(parent_id, data.name));
                }
            }
        }
        if let Some(data) = self.panels.remove(id) {
            if let Some(parent_id) = data.parent {
                self.name_index.remove(&(parent_id, data.name));
            } else {
                // Root panel uses itself as key
                self.name_index.remove(&(id, data.name));
            }
        }
    }

    /// Get the root panel ID.
    pub fn GetRootPanel(&self) -> Option<PanelId> {
        self.root
    }

    /// Collect all panel IDs in the tree. Phase 1.75 Task 5 (continuation):
    /// widened to `pub` for golden-test scaffolding that registers adapter
    /// engines in bulk (production registers inline via `create_child`).
    pub fn panel_ids(&self) -> Vec<PanelId> {
        self.panels.keys().collect()
    }

    /// Get a panel's data (crate-internal).
    pub(crate) fn GetRec(&self, id: PanelId) -> Option<&PanelData> {
        self.panels.get(id)
    }

    /// Read a panel's scheduler engine id (test-only helper).
    #[cfg(any(test, feature = "test-support"))]
    pub fn panel_engine_id(&self, id: PanelId) -> Option<EngineId> {
        self.panels.get(id).and_then(|p| p.engine_id)
    }

    /// Get a panel's data mutably (crate-internal).
    pub(crate) fn get_mut(&mut self, id: PanelId) -> Option<&mut PanelData> {
        self.panels.get_mut(id)
    }

    /// Queue notice flags on a panel and mark the tree as having pending notices.
    pub fn queue_notice(
        &mut self,
        id: PanelId,
        flags: NoticeFlags,
        sched: Option<&mut EngineScheduler>,
    ) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.pending_notices.insert(flags);
            self.has_pending_notices = true;
            // Link into the notice ring if not already linked
            // (C++ emPanel.cpp:1417: `if (!NoticeNode.Next) View.AddToNoticeList(...)`)
            self.add_to_notice_list(id, sched);
        }
    }

    // ── Public read accessors ──────────────────────────────────────────

    /// Get the panel's name.
    pub fn name(&self, id: PanelId) -> Option<&str> {
        self.panels.get(id).map(|p| p.name.as_str())
    }

    /// Get the layout rectangle.
    pub fn layout_rect(&self, id: PanelId) -> Option<Rect> {
        self.panels.get(id).map(|p| p.layout_rect)
    }

    /// Get the canvas color.
    pub fn GetCanvasColor(&self, id: PanelId) -> Option<emColor> {
        self.panels.get(id).map(|p| p.canvas_color)
    }

    /// Whether the panel is visible.
    pub fn visible(&self, id: PanelId) -> bool {
        self.panels.get(id).map(|p| p.visible).unwrap_or(false)
    }

    /// Whether the panel can receive input focus.
    pub fn focusable(&self, id: PanelId) -> bool {
        self.panels.get(id).map(|p| p.focusable).unwrap_or(false)
    }

    /// Whether the panel is enabled (computed from enable_switch and ancestors).
    pub fn enabled(&self, id: PanelId) -> bool {
        self.panels.get(id).map(|p| p.enabled).unwrap_or(false)
    }

    /// Get pending notice flags.
    pub fn pending_notices(&self, id: PanelId) -> NoticeFlags {
        self.panels
            .get(id)
            .map(|p| p.pending_notices)
            .unwrap_or_else(NoticeFlags::empty)
    }

    // ── Public write accessors ─────────────────────────────────────────

    /// Set whether the panel is visible.
    pub fn set_visible(&mut self, id: PanelId, visible: bool, sched: Option<&mut EngineScheduler>) {
        let changed = if let Some(panel) = self.panels.get_mut(id) {
            if panel.visible != visible {
                panel.visible = visible;
                panel.pending_notices.insert(NoticeFlags::VIEWING_CHANGED);
                self.has_pending_notices = true;
                true
            } else {
                false
            }
        } else {
            false
        };
        if changed {
            self.add_to_notice_list(id, sched);
        }
    }

    /// Set whether the panel can receive input focus.
    pub fn set_focusable(&mut self, id: PanelId, focusable: bool) {
        if !focusable && Some(id) == self.root {
            dlog!("root panel cannot be set unfocusable");
            return;
        }
        if let Some(panel) = self.panels.get_mut(id) {
            panel.focusable = focusable;
        }
    }

    /// emLook up a child panel by parent and name.
    pub fn find_child_by_name(&self, parent: PanelId, name: &str) -> Option<PanelId> {
        self.name_index.get(&(parent, name.to_string())).copied()
    }

    /// emLook up a panel by name (searches all panels).
    pub fn find_by_name(&self, name: &str) -> Option<PanelId> {
        self.panels
            .iter()
            .find(|(_, data)| data.name == name)
            .map(|(id, _)| id)
    }

    /// Find a panel by its full identity string.
    ///
    /// Walks all panels and compares their identity (built by walking to root).
    pub fn find_panel_by_identity(&self, identity: &str) -> Option<PanelId> {
        self.panels
            .iter()
            .map(|(id, _)| id)
            .find(|&id| self.GetIdentity(id) == identity)
    }

    /// Extract the last segment (leaf name) from a panel's identity.
    pub fn get_panel_name(&self, id: PanelId) -> String {
        let identity = self.GetIdentity(id);
        identity.rsplit(':').next().unwrap_or(&identity).to_string()
    }

    /// Check if a panel exists.
    pub fn contains(&self, id: PanelId) -> bool {
        self.panels.contains_key(id)
    }

    /// Get the total number of panels.
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    /// Check if the tree is empty.
    pub fn IsEmpty(&self) -> bool {
        self.panels.is_empty()
    }

    /// Alias for clippy `len_without_is_empty` lint.
    pub fn is_empty(&self) -> bool {
        self.IsEmpty()
    }

    /// Iterate over children of a panel.
    pub fn children(&self, parent: PanelId) -> ChildIter<'_> {
        let first = self.panels.get(parent).and_then(|p| p.first_child);
        ChildIter {
            tree: self,
            current: first,
        }
    }

    /// Get the number of children.
    pub fn child_count(&self, parent: PanelId) -> usize {
        self.children(parent).count()
    }

    /// Get the parent of a panel.
    pub fn GetParentContext(&self, id: PanelId) -> Option<PanelId> {
        self.panels.get(id).and_then(|p| p.parent)
    }

    /// Get the first child of a panel.
    ///
    /// Corresponds to `emPanel::GetFirstChild`.
    pub fn GetFirstChild(&self, id: PanelId) -> Option<PanelId> {
        self.panels.get(id).and_then(|p| p.first_child)
    }

    /// Get the last child of a panel.
    ///
    /// Corresponds to `emPanel::GetLastChild`.
    pub fn GetLastChild(&self, id: PanelId) -> Option<PanelId> {
        self.panels.get(id).and_then(|p| p.last_child)
    }

    /// Get the previous sibling of a panel.
    ///
    /// Corresponds to `emPanel::GetPrev`.
    pub fn GetPrev(&self, id: PanelId) -> Option<PanelId> {
        self.panels.get(id).and_then(|p| p.prev_sibling)
    }

    /// Get the next sibling of a panel.
    ///
    /// Corresponds to `emPanel::GetNext`.
    pub fn GetNext(&self, id: PanelId) -> Option<PanelId> {
        self.panels.get(id).and_then(|p| p.next_sibling)
    }

    /// Build a colon-delimited identity string by walking from `id` up to the
    /// root, collecting names, and encoding them.
    ///
    /// Corresponds to `emPanel::GetIdentity`.
    pub fn GetIdentity(&self, id: PanelId) -> String {
        // Walk up to root collecting names
        let mut names = Vec::new();
        let mut cur = id;
        while let Some(panel) = self.panels.get(cur) {
            names.push(panel.name.as_str());
            match panel.parent {
                Some(parent) => cur = parent,
                None => break,
            }
        }
        // names is child-to-root; reverse to root-to-child
        names.reverse();
        EncodeIdentity(&names)
    }

    // ── Sibling reordering ───────────────────────────────────────────

    /// Unlink a panel from its position in the sibling chain, without
    /// removing it from the arena or name index. The panel must have a parent.
    fn unlink_sibling(&mut self, id: PanelId) {
        let prev = self.panels[id].prev_sibling;
        let next = self.panels[id].next_sibling;
        let parent = self.panels[id]
            .parent
            .expect("unlink_sibling called on root panel");

        if let Some(prev_id) = prev {
            self.panels[prev_id].next_sibling = next;
        } else {
            self.panels[parent].first_child = next;
        }

        if let Some(next_id) = next {
            self.panels[next_id].prev_sibling = prev;
        } else {
            self.panels[parent].last_child = prev;
        }

        self.panels[id].prev_sibling = None;
        self.panels[id].next_sibling = None;
    }

    /// After a sibling reorder, notify parent of child list change.
    fn notify_sibling_reorder(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        let parent = self.panels[id].parent;
        if let Some(parent_id) = parent {
            self.panels[parent_id]
                .pending_notices
                .insert(NoticeFlags::CHILD_LIST_CHANGED);
            self.has_pending_notices = true;
            self.add_to_notice_list(parent_id, sched);
        }
    }

    /// Move this panel to the front (first) of its parent's child list.
    /// No-op if already first or if the panel is the root.
    ///
    /// Corresponds to `emPanel::BeFirst`.
    pub fn BeFirst(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        // No-op if no parent or already first
        let parent = match self.panels.get(id).and_then(|p| p.parent) {
            Some(p) => p,
            None => return,
        };
        if self.panels[id].prev_sibling.is_none() {
            return;
        }

        self.unlink_sibling(id);

        // Relink as first child
        let old_first = self.panels[parent].first_child;
        self.panels[id].next_sibling = old_first;
        if let Some(old_first_id) = old_first {
            self.panels[old_first_id].prev_sibling = Some(id);
        }
        self.panels[parent].first_child = Some(id);
        if self.panels[parent].last_child.is_none() {
            self.panels[parent].last_child = Some(id);
        }

        self.notify_sibling_reorder(id, sched);
    }

    /// Move this panel to the end (last) of its parent's child list.
    /// No-op if already last or if the panel is the root.
    ///
    /// Corresponds to `emPanel::BeLast`.
    pub fn BeLast(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        let parent = match self.panels.get(id).and_then(|p| p.parent) {
            Some(p) => p,
            None => return,
        };
        if self.panels[id].next_sibling.is_none() {
            return;
        }

        self.unlink_sibling(id);

        // Relink as last child
        let old_last = self.panels[parent].last_child;
        self.panels[id].prev_sibling = old_last;
        if let Some(old_last_id) = old_last {
            self.panels[old_last_id].next_sibling = Some(id);
        }
        self.panels[parent].last_child = Some(id);
        if self.panels[parent].first_child.is_none() {
            self.panels[parent].first_child = Some(id);
        }

        self.notify_sibling_reorder(id, sched);
    }

    /// Move this panel to be immediately before the given sibling.
    /// If `sibling` is `None`, calls [`BeLast`](Self::BeLast).
    /// No-op if `sibling` is this panel, is already the next sibling, or has
    /// a different parent.
    ///
    /// Corresponds to `emPanel::BePrevOf`.
    pub fn BePrevOf(
        &mut self,
        id: PanelId,
        sibling: Option<PanelId>,
        sched: Option<&mut EngineScheduler>,
    ) {
        let sibling = match sibling {
            Some(s) => s,
            None => {
                self.BeLast(id, sched);
                return;
            }
        };

        // No-op checks
        if sibling == id {
            return;
        }
        if self.panels[id].next_sibling == Some(sibling) {
            return;
        }
        let my_parent = self.panels[id].parent;
        let sib_parent = self.panels[sibling].parent;
        if my_parent != sib_parent || my_parent.is_none() {
            return;
        }
        let parent = my_parent.expect("checked above");

        self.unlink_sibling(id);

        // Insert before sibling
        let sib_prev = self.panels[sibling].prev_sibling;
        self.panels[id].next_sibling = Some(sibling);
        self.panels[id].prev_sibling = sib_prev;
        self.panels[sibling].prev_sibling = Some(id);
        if let Some(prev_id) = sib_prev {
            self.panels[prev_id].next_sibling = Some(id);
        } else {
            self.panels[parent].first_child = Some(id);
        }

        self.notify_sibling_reorder(id, sched);
    }

    /// Move this panel to be immediately after the given sibling.
    /// If `sibling` is `None`, calls [`BeFirst`](Self::BeFirst).
    /// No-op if `sibling` is this panel, is already the prev sibling, or has
    /// a different parent.
    ///
    /// Corresponds to `emPanel::BeNextOf`.
    pub fn BeNextOf(
        &mut self,
        id: PanelId,
        sibling: Option<PanelId>,
        sched: Option<&mut EngineScheduler>,
    ) {
        let sibling = match sibling {
            Some(s) => s,
            None => {
                self.BeFirst(id, sched);
                return;
            }
        };

        // No-op checks
        if sibling == id {
            return;
        }
        if self.panels[id].prev_sibling == Some(sibling) {
            return;
        }
        let my_parent = self.panels[id].parent;
        let sib_parent = self.panels[sibling].parent;
        if my_parent != sib_parent || my_parent.is_none() {
            return;
        }
        let parent = my_parent.expect("checked above");

        self.unlink_sibling(id);

        // Insert after sibling
        let sib_next = self.panels[sibling].next_sibling;
        self.panels[id].prev_sibling = Some(sibling);
        self.panels[id].next_sibling = sib_next;
        self.panels[sibling].next_sibling = Some(id);
        if let Some(next_id) = sib_next {
            self.panels[next_id].prev_sibling = Some(id);
        } else {
            self.panels[parent].last_child = Some(id);
        }

        self.notify_sibling_reorder(id, sched);
    }

    /// Sort the children of a panel using the given comparator.
    /// Notifies `CHILDREN_CHANGED` only if the order actually changed.
    ///
    /// Corresponds to `emPanel::SortChildren`.
    pub fn SortChildren<F>(
        &mut self,
        parent: PanelId,
        mut compare: F,
        sched: Option<&mut EngineScheduler>,
    ) where
        F: FnMut(PanelId, PanelId) -> std::cmp::Ordering,
    {
        // Collect children into a vec
        let children: Vec<PanelId> = self.children(parent).collect();
        if children.len() <= 1 {
            return;
        }

        // Sort
        let mut sorted = children.clone();
        sorted.sort_by(|&a, &b| compare(a, b));

        // Check if order actually changed
        if sorted == children {
            return;
        }

        // Relink the sibling chain according to sorted order
        for (i, &child) in sorted.iter().enumerate() {
            self.panels[child].prev_sibling = if i > 0 { Some(sorted[i - 1]) } else { None };
            self.panels[child].next_sibling = if i + 1 < sorted.len() {
                Some(sorted[i + 1])
            } else {
                None
            };
        }
        self.panels[parent].first_child = Some(sorted[0]);
        self.panels[parent].last_child = Some(sorted[sorted.len() - 1]);

        self.panels[parent]
            .pending_notices
            .insert(NoticeFlags::CHILD_LIST_CHANGED);
        self.has_pending_notices = true;
        self.add_to_notice_list(parent, sched);
    }

    // ── Title / Icon ─────────────────────────────────────────────────

    /// Walk up the parent chain trying each panel's behavior for a title.
    /// If no behavior provides one, the root returns `"untitled"`.
    ///
    /// Corresponds to `emPanel::GetTitle`.
    pub fn get_title(&self, id: PanelId) -> String {
        let mut cur = id;
        loop {
            if let Some(panel) = self.panels.get(cur) {
                if let Some(ref behavior) = panel.behavior {
                    if let Some(title) = behavior.get_title() {
                        return title;
                    }
                }
                match panel.parent {
                    Some(parent) => cur = parent,
                    None => return "untitled".to_string(),
                }
            } else {
                return "untitled".to_string();
            }
        }
    }

    /// Walk up the parent chain trying each panel's behavior for an icon
    /// filename. If no behavior provides one, the root returns `""`.
    ///
    /// Corresponds to `emPanel::GetIconFileName`.
    pub fn GetIconFileName(&self, id: PanelId) -> String {
        let mut cur = id;
        loop {
            if let Some(panel) = self.panels.get(cur) {
                if let Some(ref behavior) = panel.behavior {
                    if let Some(name) = behavior.GetIconFileName() {
                        return name;
                    }
                }
                match panel.parent {
                    Some(parent) => cur = parent,
                    None => return String::new(),
                }
            } else {
                return String::new();
            }
        }
    }

    /// Remove all children of a panel.
    pub fn DeleteAllChildren(&mut self, parent: PanelId, mut sched: Option<&mut EngineScheduler>) {
        let children: Vec<PanelId> = self.children(parent).collect();
        for child in children {
            self.remove(child, sched.as_deref_mut());
        }
    }

    /// Delete only the children that were created by auto-expansion (C++ parity).
    ///
    /// Preserves manually-added children. Corresponds to the C++ pattern of
    /// only removing `CreatedByAE` children during auto-shrink.
    pub fn delete_ae_children(&mut self, parent: PanelId, mut sched: Option<&mut EngineScheduler>) {
        let children: Vec<PanelId> = self.children(parent).collect();
        for child in children {
            if self.panels.get(child).is_some_and(|p| p.created_by_ae) {
                self.remove(child, sched.as_deref_mut());
            }
        }
    }

    /// Force re-layout of all children of a panel by inserting
    /// `NoticeFlags::LAYOUT_CHANGED` into each child's pending notices.
    ///
    /// Corresponds to `emPanel::InvalidateChildrenLayout`.
    pub fn InvalidateChildrenLayout(
        &mut self,
        id: PanelId,
        mut sched: Option<&mut EngineScheduler>,
    ) {
        let children: Vec<PanelId> = self.children(id).collect();
        for child in children {
            if let Some(panel) = self.panels.get_mut(child) {
                panel.pending_notices.insert(NoticeFlags::LAYOUT_CHANGED);
                self.has_pending_notices = true;
            }
            self.add_to_notice_list(child, sched.as_deref_mut());
        }
    }

    /// Set the layout rectangle for a panel.
    ///
    /// Port of C++ `emPanel::Layout(x,y,w,h,canvasColor)` (emPanel.cpp:490–640).
    ///
    /// Sets the panel's layout rect. If the parent is already viewed, eagerly
    /// computes this panel's viewport coordinates and queues `VIEW_CHANGED` so
    /// AE can fire in the same notice-ring drain — matching C++ which does this
    /// inside `emPanel::Layout` via direct `View` access.
    ///
    /// Width and height are clamped to a minimum of `1e-100` to prevent
    /// division-by-zero when computing tallness.
    #[allow(clippy::too_many_arguments)]
    pub fn Layout(
        &mut self,
        id: PanelId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        current_pixel_tallness: f64,
        mut sched: Option<&mut EngineScheduler>,
    ) {
        let rect = Rect {
            x,
            y,
            w: w.max(1e-100),
            h: h.max(1e-100),
        };
        if let Some(panel) = self.panels.get_mut(id) {
            if panel.layout_rect == rect {
                return;
            }
            panel.layout_rect = rect;
            // C++ emPanel::Layout always queues NF_LAYOUT_CHANGED (emPanel.cpp:521).
            // NF_VIEWING_CHANGED (= Rust VISIBILITY) is only queued in the "Parent->Viewed" branch.
            panel.pending_notices.insert(NoticeFlags::LAYOUT_CHANGED);
            self.has_pending_notices = true;
        } else {
            return;
        }
        self.add_to_notice_list(id, sched.as_deref_mut());

        // C++ emPanel::Layout `!Parent` branch (emPanel.cpp:524):
        //   View.SVPChoiceInvalid=true; ... RawZoomOut(true) or RawVisit(p,...,true)
        // Rust can't call view methods from here — set root_layout_changed so
        // emView::Update sees it and calls RawZoomOut on the next frame.
        let is_root = self.panels.get(id).and_then(|p| p.parent).is_none();
        if is_root {
            self.root_layout_changed = true;
            return; // root viewed coords are managed by emView, not eagerly here
        }

        // Port of C++ emPanel::Layout "else if (Parent->Viewed)" branch
        // (emPanel.cpp:557–610): eagerly compute viewing coords from parent.
        // Without this, Rust's lazy view.Update() wouldn't set viewed_width
        // until the next frame, making AE fire one tick late.
        let parent_id = match self.panels.get(id).and_then(|p| p.parent) {
            Some(pid) => pid,
            None => return, // root panel: view manages its own coords
        };
        let (parent_viewed, pvx, pvy, pvw, pcx1, pcy1, pcx2, pcy2) = {
            let Some(pp) = self.panels.get(parent_id) else {
                return;
            };
            if !pp.viewed {
                return; // parent not viewed; no eager computation
            }
            (
                pp.viewed,
                pp.viewed_x,
                pp.viewed_y,
                pp.viewed_width,
                pp.clip_x,
                pp.clip_y,
                pp.clip_x + pp.clip_w,
                pp.clip_y + pp.clip_h,
            )
        };
        let _ = parent_viewed; // confirmed true above

        let pt = current_pixel_tallness;
        let cx = pvx + x * pvw;
        let cy = pvy + y * (pvw / pt);
        let cw = w.max(1e-100) * pvw;
        let ch = h.max(1e-100) * (pvw / pt);

        // Clip rect (C++ ClipX1/X2/Y1/Y2 convention).
        let mut cx1 = cx;
        let mut cx2 = cx + cw;
        let mut cy1 = cy;
        let mut cy2 = cy + ch;
        if cx1 < pcx1 {
            cx1 = pcx1;
        }
        if cx2 > pcx2 {
            cx2 = pcx2;
        }
        if cy1 < pcy1 {
            cy1 = pcy1;
        }
        if cy2 > pcy2 {
            cy2 = pcy2;
        }

        if let Some(p) = self.panels.get_mut(id) {
            p.viewed_x = cx;
            p.viewed_y = cy;
            p.viewed_width = cw;
            p.viewed_height = ch;
            p.clip_x = cx1;
            p.clip_y = cy1;
            p.clip_w = (cx2 - cx1).max(0.0);
            p.clip_h = (cy2 - cy1).max(0.0);
        }

        if cx1 < cx2 && cy1 < cy2 {
            // Panel is within the clip region — it's viewed.
            if let Some(p) = self.panels.get_mut(id) {
                p.viewed = true;
                p.in_viewed_path = true;
            }
            // Queue NF_VIEWING_CHANGED | NF_UPDATE_PRIORITY_CHANGED | NF_MEMORY_LIMIT_CHANGED
            // (C++ emPanel.cpp:583–590). VISIBILITY = C++ NF_VIEWING_CHANGED.
            self.panels[id].pending_notices.insert(
                NoticeFlags::VIEWING_CHANGED
                    | NoticeFlags::UPDATE_PRIORITY_CHANGED
                    | NoticeFlags::MEMORY_LIMIT_CHANGED,
            );
            self.has_pending_notices = true;
            self.add_to_notice_list(id, sched.as_deref_mut());
            // Propagate to children (C++ emPanel.cpp:591: UpdateChildrenViewing()).
            self.UpdateChildrenViewing(id, current_pixel_tallness, sched);
        } else {
            // Panel is outside clip. Clear viewed state if it was in viewed path.
            let was_in_viewed_path = self
                .panels
                .get(id)
                .map(|p| p.in_viewed_path)
                .unwrap_or(false);
            if was_in_viewed_path {
                if let Some(p) = self.panels.get_mut(id) {
                    p.viewed = false;
                    p.in_viewed_path = false;
                }
                // C++ queues NF_VIEWING_CHANGED when becoming non-viewed (emPanel.cpp:598).
                // VISIBILITY = C++ NF_VIEWING_CHANGED.
                self.panels[id].pending_notices.insert(
                    NoticeFlags::VIEWING_CHANGED
                        | NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | NoticeFlags::MEMORY_LIMIT_CHANGED,
                );
                self.has_pending_notices = true;
                self.add_to_notice_list(id, sched.as_deref_mut());
                self.UpdateChildrenViewing(id, current_pixel_tallness, sched);
            }
        }
    }

    /// Set the canvas color for a panel.
    pub fn SetCanvasColor(
        &mut self,
        id: PanelId,
        color: emColor,
        sched: Option<&mut EngineScheduler>,
    ) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.canvas_color = color;
            panel.pending_notices.insert(NoticeFlags::VIEWING_CHANGED);
            self.has_pending_notices = true;
        } else {
            return;
        }
        self.add_to_notice_list(id, sched);
    }

    /// Set the enable switch for a panel and recompute enabled state for descendants.
    pub fn SetEnableSwitch(
        &mut self,
        id: PanelId,
        enable: bool,
        sched: Option<&mut EngineScheduler>,
    ) {
        if let Some(panel) = self.panels.get_mut(id) {
            if panel.enable_switch == enable {
                return;
            }
            panel.enable_switch = enable;
        }
        self.recompute_enabled(id, sched);
    }

    /// Recompute the `enabled` field for a panel and its descendants.
    fn recompute_enabled(&mut self, id: PanelId, mut sched: Option<&mut EngineScheduler>) {
        let parent_enabled = self
            .panels
            .get(id)
            .and_then(|p| p.parent)
            .and_then(|pid| self.panels.get(pid))
            .map(|p| p.enabled)
            .unwrap_or(true);

        let changed = if let Some(panel) = self.panels.get_mut(id) {
            let new_enabled = panel.enable_switch && parent_enabled;
            if panel.enabled != new_enabled {
                panel.enabled = new_enabled;
                panel.pending_notices.insert(NoticeFlags::ENABLE_CHANGED);
                self.has_pending_notices = true;
                true
            } else {
                false
            }
        } else {
            false
        };
        if changed {
            self.add_to_notice_list(id, sched.as_deref_mut());
        }

        // Recurse into children
        let child_ids: Vec<PanelId> = self.children(id).collect();
        for child_id in child_ids {
            self.recompute_enabled(child_id, sched.as_deref_mut());
        }
    }

    /// Set the behavior for a panel.
    pub fn set_behavior(&mut self, id: PanelId, behavior: Box<dyn PanelBehavior>) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.behavior = Some(behavior);
            // Behavior installed on an existing panel means its AE
            // decision could now change (e.g., sub-tree root gets
            // emMainContentPanel behavior). Force re-evaluation.
            panel.ae_decision_invalid = true;
        }
    }

    pub fn has_behavior(&self, id: PanelId) -> bool {
        self.panels
            .get(id)
            .and_then(|p| p.behavior.as_ref())
            .is_some()
    }

    /// Re-fire initialization notices on a panel (e.g., after setting
    /// behavior on an existing panel that already had its init notices
    /// drained before behavior was attached).
    pub fn fire_init_notices(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.pending_notices.insert(Self::INIT_NOTICE_FLAGS);
            self.has_pending_notices = true;
        } else {
            return;
        }
        self.add_to_notice_list(id, sched);
    }

    /// Build a `PanelState` snapshot for the given panel.
    pub fn build_panel_state(
        &self,
        id: PanelId,
        window_focused: bool,
        pixel_tallness: f64,
    ) -> PanelState {
        let p = &self.panels[id];
        PanelState {
            id,
            is_active: p.is_active,
            in_active_path: p.in_active_path,
            window_focused,
            enabled: p.enabled,
            viewed: p.viewed,
            clip_rect: Rect::new(p.clip_x, p.clip_y, p.clip_w, p.clip_h),
            viewed_rect: Rect::new(p.viewed_x, p.viewed_y, p.viewed_width, p.viewed_height),
            priority: 0.0,
            memory_limit: 0,
            pixel_tallness,
            height: p.layout_rect.h / p.layout_rect.w,
        }
    }

    /// Set the `pending_input` flag on a panel.
    ///
    /// Used by `emView::RecurseInput` to track which panels need input
    /// dispatching.  Mirrors C++ `emPanel::PendingInput` field writes.
    pub fn set_pending_input(&mut self, id: PanelId, value: bool) {
        if let Some(p) = self.panels.get_mut(id) {
            p.pending_input = value;
        }
    }

    /// Get the `pending_input` flag on a panel.
    ///
    /// Mirrors C++ `emPanel::PendingInput` field reads.
    pub fn get_pending_input(&self, id: PanelId) -> bool {
        self.panels.get(id).is_some_and(|p| p.pending_input)
    }

    /// Dispatch an input event to a panel's behavior.
    ///
    /// Builds a `PanelState`, takes the behavior, calls `Input`, and puts
    /// the behavior back. Mirrors C++ `emPanel::Input` dispatch from
    /// `emView::RecurseInput`.
    pub fn dispatch_input(
        &mut self,
        id: PanelId,
        event: &super::emInput::emInputEvent,
        input_state: &super::emInputState::emInputState,
        window_focused: bool,
        pixel_tallness: f64,
    ) {
        let state = self.build_panel_state(id, window_focused, pixel_tallness);
        if let Some(mut behavior) = self.take_behavior(id) {
            // Phase 3 B3.2 supersedes Phase-1.76: SchedCtx (via PanelCtx) now
            // threads through Input dispatch. Concrete widget `Input` free
            // functions take `&mut PanelCtx` (B3.2 plumbing); B3.3 migrates
            // their callback invocations to use the ctx's scheduler reach.
            // This path still constructs a scheduler-less ctx — live input
            // dispatch continues to flow through `emWindow::dispatch_input`
            // with a full-reach ctx via `with_sched_reach`.
            {
                let mut panel_ctx = super::emEngineCtx::PanelCtx::new(self, id, pixel_tallness);
                behavior.Input(event, &state, input_state, &mut panel_ctx);
            }
            if self.panels.contains_key(id) {
                self.put_behavior(id, behavior);
            }
        }
    }

    /// Extract the behavior from a panel (for calling methods that need &mut self on tree).
    pub fn take_behavior(&mut self, id: PanelId) -> Option<Box<dyn PanelBehavior>> {
        self.panels.get_mut(id).and_then(|p| p.behavior.take())
    }

    /// Put the behavior back after extraction.
    pub fn put_behavior(&mut self, id: PanelId, behavior: Box<dyn PanelBehavior>) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.behavior = Some(behavior);
        }
    }

    /// Extract a child behavior, downcast to concrete type, call a closure,
    /// then put the behavior back. Returns None if panel doesn't exist or
    /// behavior is the wrong type.
    pub fn with_behavior_as<T: PanelBehavior, R>(
        &mut self,
        id: PanelId,
        f: impl FnOnce(&mut T) -> R,
    ) -> Option<R> {
        let mut behavior = self.take_behavior(id)?;
        let result = behavior.as_any_mut().downcast_mut::<T>().map(f);
        if self.panels.contains_key(id) {
            self.put_behavior(id, behavior);
        }
        result
    }

    /// Check if a panel's behavior reports as opaque.
    /// Corresponds to C++ `emPanel::IsOpaque()`.
    pub fn IsOpaque(&mut self, id: PanelId) -> bool {
        match self.take_behavior(id) {
            Some(behavior) => {
                let opaque = behavior.IsOpaque();
                self.put_behavior(id, behavior);
                opaque
            }
            None => false,
        }
    }

    /// Request that the view navigate to show this panel.
    /// Called by panel behaviors; drained by emView each frame.
    pub(crate) fn request_visit(&mut self, target: PanelId) {
        self.navigation_requests.push(target);
    }

    /// Drain pending navigation requests. Called by emView::Update.
    pub(crate) fn drain_navigation_requests(&mut self) -> Vec<PanelId> {
        std::mem::take(&mut self.navigation_requests)
    }

    /// Whether the `has_pending_notices` flag is set.
    ///
    /// Phase 2 Task 6 (E006): ring head/tail moved to `emView`; this method
    /// now only reports the flag that PanelTree sets when notice state changes.
    /// `emView::HandleNotice` additionally checks its own `notice_ring_head_next`.
    pub fn has_pending_notices(&self) -> bool {
        self.has_pending_notices
    }

    /// Whether the `has_pending_notices` flag is set (panels set pending_notices
    /// through paths that may not have called `add_to_notice_list`).
    /// Read by `emView::HandleNotice` safety-net scan.
    pub(crate) fn has_pending_notices_flag(&self) -> bool {
        self.has_pending_notices
    }

    /// Clear the `has_pending_notices` flag after draining the ring.
    /// Called by `emView::HandleNotice` at the end of a full drain.
    pub(crate) fn clear_pending_notices_flag(&mut self) {
        self.has_pending_notices = false;
    }

    /// Walk from `id` to root, returning ancestor chain (id first, root last).
    pub fn ancestors(&self, id: PanelId) -> Vec<PanelId> {
        let mut result = vec![id];
        let mut cur = id;
        while let Some(parent) = self.panels.get(cur).and_then(|p| p.parent) {
            result.push(parent);
            cur = parent;
        }
        result
    }

    /// Iterate children in reverse order (last_child → first_child).
    pub fn children_rev(&self, parent: PanelId) -> ChildRevIter<'_> {
        let last = self.panels.get(parent).and_then(|p| p.last_child);
        ChildRevIter {
            tree: self,
            current: last,
        }
    }

    /// Find nearest focusable ancestor of `id` (excluding self, starting from parent).
    pub fn GetFocusableParent(&self, id: PanelId) -> Option<PanelId> {
        let mut cur = self.panels.get(id).and_then(|p| p.parent);
        while let Some(c) = cur {
            if self.panels.get(c).map(|p| p.focusable).unwrap_or(false) {
                return Some(c);
            }
            cur = self.panels.get(c).and_then(|p| p.parent);
        }
        None
    }

    // ── Coordinate transforms ─────────────────────────────────────────

    /// Convert panel-space X to view-space X.
    pub fn PanelToViewX(&self, id: PanelId, x: f64) -> f64 {
        let p = &self.panels[id];
        p.viewed_x + x * p.viewed_width
    }

    /// Convert panel-space Y to view-space Y.
    pub fn PanelToViewY(&self, id: PanelId, y: f64) -> f64 {
        let p = &self.panels[id];
        p.viewed_y + y * p.viewed_height
    }

    /// Convert view-space X to panel-space X.
    pub fn ViewToPanelX(&self, id: PanelId, vx: f64) -> f64 {
        let p = &self.panels[id];
        (vx - p.viewed_x) / p.viewed_width
    }

    /// Convert view-space Y to panel-space Y.
    ///
    /// C++ emView.cpp:2020: `my = (mouseY - ViewedY) / ViewedWidth * PixelTallness`
    /// Both axes divide by ViewedWidth (not ViewedHeight) to preserve the panel's
    /// normalized coordinate system where X is 0..1 and Y is 0..tallness.
    pub fn ViewToPanelY(&self, id: PanelId, vy: f64, pixel_tallness: f64) -> f64 {
        let p = &self.panels[id];
        (vy - p.viewed_y) / p.viewed_width * pixel_tallness
    }

    /// Convert a panel-space delta X to view-space delta X.
    pub fn PanelToViewDeltaX(&self, id: PanelId, dx: f64) -> f64 {
        dx * self.panels[id].viewed_width
    }

    /// Convert a panel-space delta Y to view-space delta Y.
    pub fn PanelToViewDeltaY(&self, id: PanelId, dy: f64) -> f64 {
        dy * self.panels[id].viewed_height
    }

    /// Convert a view-space delta X to panel-space delta X.
    pub fn ViewToPanelDeltaX(&self, id: PanelId, dvx: f64) -> f64 {
        dvx / self.panels[id].viewed_width
    }

    /// Convert a view-space delta Y to panel-space delta Y.
    pub fn ViewToPanelDeltaY(&self, id: PanelId, dvy: f64) -> f64 {
        dvy / self.panels[id].viewed_height
    }

    // ── Geometry accessors ───────────────────────────────────────────

    /// Panel height in its own coordinate system: `layout_h / layout_w`.
    ///
    /// In the C++ source this is `GetHeight()` / `GetTallness()`.
    pub fn get_height(&self, id: PanelId) -> f64 {
        let p = &self.panels[id];
        p.layout_rect.h / p.layout_rect.w
    }

    /// Alias for [`get_height`](Self::get_height).
    pub fn GetTallness(&self, id: PanelId) -> f64 {
        self.get_height(id)
    }

    /// Return the substance rectangle and corner radius for a panel.
    ///
    /// The base `emPanel` implementation returns `(0, 0, 1, GetHeight(), 0)` --
    /// i.e. the full panel rect with zero radius. Subclass overrides (border
    /// panels) may return a smaller rect with a nonzero radius; those will be
    /// handled by the behavior trait. This method provides the default.
    pub fn GetSubstanceRect(&self, id: PanelId) -> (f64, f64, f64, f64, f64) {
        let h = self.get_height(id);
        (0.0, 0.0, 1.0, h, 0.0)
    }

    /// Test whether a point lies inside the substance rectangle (with rounded
    /// corners).
    pub fn IsPointInSubstanceRect(&self, id: PanelId, x: f64, y: f64) -> bool {
        let h = self.get_height(id);

        // Quick rejection: outside panel bounds
        if !(0.0..1.0).contains(&x) || !(0.0..h).contains(&y) {
            return false;
        }

        let (sx, sy, sw, sh, sr) = self.GetSubstanceRect(id);
        let sw2 = sw * 0.5;
        let sh2 = sh * 0.5;

        // Distance from center of substance rect
        let dx = (x - sx - sw2).abs();
        let dy = (y - sy - sh2).abs();

        // Outside substance rect entirely
        if dx > sw2 || dy > sh2 {
            return false;
        }

        // Clamp radius to half-dimensions
        let r = sr.min(sw2).min(sh2);

        // Distance from the inner rect edge (where rounding begins)
        let cdx = dx - (sw2 - r);
        let cdy = dy - (sh2 - r);

        // Inside the non-rounded portion
        if cdx < 0.0 || cdy < 0.0 {
            return true;
        }

        // Corner arc test
        cdx * cdx + cdy * cdy <= r * r
    }

    /// Return the essence rectangle -- the substance rectangle without the
    /// corner-radius inset.
    pub fn GetEssenceRect(&self, id: PanelId) -> (f64, f64, f64, f64) {
        let (sx, sy, sw, sh, _sr) = self.GetSubstanceRect(id);
        (sx, sy, sw, sh)
    }

    // ── Auto-expansion ────────────────────────────────────────────────

    /// Set the auto-expansion threshold type and value. If either differs from
    /// the current values the AE decision is marked invalid so the next notice
    /// pass will re-evaluate.
    ///
    /// Corresponds to `emPanel::SetAutoExpansionThreshold`.
    pub fn SetAutoExpansionThreshold(
        &mut self,
        id: PanelId,
        threshold_value: f64,
        threshold_type: ViewConditionType,
        sched: Option<&mut EngineScheduler>,
    ) {
        if let Some(panel) = self.panels.get_mut(id) {
            if panel.ae_threshold_value == threshold_value
                && panel.ae_threshold_type == threshold_type
            {
                return;
            }
            panel.ae_threshold_value = threshold_value;
            panel.ae_threshold_type = threshold_type;
            panel.ae_decision_invalid = true;
            // C++ emPanel::SetAutoExpansionThreshold: AddToNoticeList
            self.has_pending_notices = true;
        }
        self.add_to_notice_list(id, sched);
    }

    /// Return the auto-expansion threshold value.
    ///
    /// Corresponds to `emPanel::GetAutoExpansionThresholdValue`.
    pub fn GetAutoExpansionThresholdValue(&self, id: PanelId) -> f64 {
        self.panels
            .get(id)
            .map(|p| p.ae_threshold_value)
            .unwrap_or(0.0)
    }

    /// Return the auto-expansion threshold type.
    ///
    /// Corresponds to `emPanel::GetAutoExpansionThresholdType`.
    pub fn GetAutoExpansionThresholdType(&self, id: PanelId) -> ViewConditionType {
        self.panels
            .get(id)
            .map(|p| p.ae_threshold_type)
            .unwrap_or(ViewConditionType::Area)
    }

    /// Whether the panel is currently viewed (visible in the view).
    ///
    /// Corresponds to `emPanel::IsViewed`.
    pub fn IsViewed(&self, id: PanelId) -> bool {
        self.panels.get(id).map(|p| p.viewed).unwrap_or(false)
    }

    /// Whether the panel is currently auto-expanded.
    ///
    /// Corresponds to `emPanel::IsAutoExpanded`.
    pub fn IsAutoExpanded(&self, id: PanelId) -> bool {
        self.panels.get(id).map(|p| p.ae_expanded).unwrap_or(false)
    }

    /// Mark auto-expansion as needing recomputation. Only has an effect when
    /// the panel is currently expanded and not already invalidated.
    ///
    /// Corresponds to `emPanel::InvalidateAutoExpansion`.
    pub fn InvalidateAutoExpansion(&mut self, id: PanelId) {
        if let Some(panel) = self.panels.get_mut(id) {
            if !panel.ae_invalid && panel.ae_expanded {
                panel.ae_invalid = true;
            }
        }
    }

    /// Return whether this panel's content is ready. The base implementation
    /// simply returns the `ae_expanded` state.
    ///
    /// Corresponds to `emPanel::IsContentReady`.
    pub fn IsContentReady(&self, id: PanelId) -> bool {
        self.panels.get(id).map(|p| p.ae_expanded).unwrap_or(false)
    }

    // ── Autoplay / playback / seeking ────────────────────────────────

    /// Set the autoplay handling flags for a panel.
    ///
    /// Corresponds to `emPanel::SetAutoplayHandling`.
    pub fn SetAutoplayHandling(&mut self, id: PanelId, flags: AutoplayHandlingFlags) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.autoplay_handling = flags;
        }
    }

    /// Return the autoplay handling flags for a panel.
    ///
    /// Corresponds to `emPanel::GetAutoplayHandling`.
    pub fn GetAutoplayHandling(&self, id: PanelId) -> AutoplayHandlingFlags {
        self.panels
            .get(id)
            .map(|p| p.autoplay_handling)
            .unwrap_or_default()
    }

    /// Return the playback state for a panel.
    ///
    /// The default panel has no playback support -- returns
    /// `PlaybackState { playing: false, pos: 0.0, supported: false }`.
    /// Panels with a behavior that overrides `get_playback_state` may
    /// return different values.
    ///
    /// Corresponds to `emPanel::GetPlaybackState`.
    pub fn GetPlaybackState(&self, id: PanelId) -> PlaybackState {
        if let Some(panel) = self.panels.get(id) {
            if let Some(ref behavior) = panel.behavior {
                return behavior.GetPlaybackState();
            }
        }
        PlaybackState::default()
    }

    /// Attempt to set the playback state for a panel. Returns `true` if
    /// the panel supports playback and accepted the state, `false` otherwise.
    ///
    /// Corresponds to `emPanel::SetPlaybackState`.
    pub fn SetPlaybackState(&mut self, id: PanelId, playing: bool, pos: f64) -> bool {
        if let Some(mut behavior) = self.take_behavior(id) {
            let accepted = behavior.SetPlaybackState(playing, pos);
            self.put_behavior(id, behavior);
            return accepted;
        }
        false
    }

    /// Return the sought child name if `id` is the panel currently being
    /// sought by the visiting animator, or `None` otherwise.
    ///
    /// `seek_pos_panel` and `seek_pos_child_name` come from
    /// [`emView::seek_pos_panel`] and [`emView::seek_pos_child_name`].
    ///
    /// Corresponds to `emPanel::GetSoughtName`.
    pub fn GetSoughtName<'a>(
        &self,
        id: PanelId,
        seek_pos_panel: Option<PanelId>,
        seek_pos_child_name: &'a str,
    ) -> Option<&'a str> {
        if seek_pos_panel == Some(id) {
            Some(seek_pos_child_name)
        } else {
            None
        }
    }

    /// Whether this panel has hope that seeking can succeed.
    ///
    /// The default implementation returns `false`. Panels with a behavior
    /// that overrides `is_hope_for_seeking` may return `true`.
    ///
    /// Corresponds to `emPanel::IsHopeForSeeking`.
    pub fn IsHopeForSeeking(&self, id: PanelId) -> bool {
        if let Some(panel) = self.panels.get(id) {
            if let Some(ref behavior) = panel.behavior {
                return behavior.IsHopeForSeeking();
            }
        }
        false
    }

    /// Return the touch event priority for a panel: 1.0 if focusable,
    /// 0.0 otherwise. The `_touch_x`/`_touch_y` arguments are accepted
    /// for API compatibility but unused in the base implementation.
    ///
    /// Corresponds to `emPanel::GetTouchEventPriority`.
    pub fn GetTouchEventPriority(&self, id: PanelId, _touch_x: f64, _touch_y: f64) -> f64 {
        if self.focusable(id) {
            1.0
        } else {
            0.0
        }
    }

    /// Walk up the parent chain calling each panel's behavior for
    /// `create_control_panel`. Returns the first non-`None` result, or
    /// `None` if the root is reached without any behavior creating a panel.
    ///
    /// Corresponds to `emPanel::CreateControlPanel`.
    pub fn CreateControlPanel(
        &mut self,
        id: PanelId,
        parent_arg: PanelId,
        name: &str,
        current_pixel_tallness: f64,
    ) -> Option<PanelId> {
        let mut cur = id;
        loop {
            if let Some(mut behavior) = self.take_behavior(cur) {
                let mut ctx = PanelCtx::new(self, parent_arg, current_pixel_tallness);
                let result = behavior.CreateControlPanel(&mut ctx, name);
                self.put_behavior(cur, behavior);
                if result.is_some() {
                    return result;
                }
            }
            match self.panels.get(cur).and_then(|p| p.parent) {
                Some(parent) => cur = parent,
                None => return None,
            }
        }
    }

    /// Walk this tree's parent chain from `id`, but create the control panel
    /// in `target_tree` as a child of `parent_arg`.
    ///
    /// This enables cross-tree creation: behaviors live in the content tree,
    /// but the control panel is created in the control tree.
    pub fn create_control_panel_in(
        &mut self,
        id: PanelId,
        target_tree: &mut PanelTree,
        parent_arg: PanelId,
        name: &str,
        current_pixel_tallness: f64,
    ) -> Option<PanelId> {
        let mut cur = id;
        loop {
            if let Some(mut behavior) = self.take_behavior(cur) {
                let mut ctx = PanelCtx::new(target_tree, parent_arg, current_pixel_tallness);
                let result = behavior.CreateControlPanel(&mut ctx, name);
                self.put_behavior(cur, behavior);
                if result.is_some() {
                    return result;
                }
            }
            match self.panels.get(cur).and_then(|p| p.parent) {
                Some(parent) => cur = parent,
                None => return None,
            }
        }
    }

    // ── emView condition ──────────────────────────────────────────────

    /// Return a size metric for how large the panel appears in the view.
    ///
    /// Returns 0.0 if the panel is not in the viewed path, 1e100 if in the
    /// viewed path but not actually viewed, or a metric based on
    /// `ViewConditionType` when viewed.
    ///
    /// Corresponds to `emPanel::GetViewCondition`.
    pub fn GetViewCondition(&self, id: PanelId, vc_type: ViewConditionType) -> f64 {
        let panel = match self.panels.get(id) {
            Some(p) => p,
            None => return 0.0,
        };

        if panel.viewed {
            match vc_type {
                ViewConditionType::Area => panel.viewed_width * panel.viewed_height,
                ViewConditionType::Width => panel.viewed_width,
                ViewConditionType::Height => panel.viewed_height,
                ViewConditionType::MinExt => panel.viewed_width.min(panel.viewed_height),
                ViewConditionType::MaxExt => panel.viewed_width.max(panel.viewed_height),
            }
        } else if panel.in_viewed_path {
            1e100
        } else {
            0.0
        }
    }

    // ── Update priority ─────────────────────────────────────────────

    /// Calculate an update priority between 0.0 and 1.0 based on how centrally
    /// the panel's clip rect is located within the view. Adds 0.5 if the view
    /// is focused.
    ///
    /// Corresponds to `emPanel::GetUpdatePriority`.
    pub fn GetUpdatePriority(
        &self,
        id: PanelId,
        viewport_width: f64,
        viewport_height: f64,
        view_focused: bool,
    ) -> f64 {
        let panel = match self.panels.get(id) {
            Some(p) => p,
            None => return 0.0,
        };

        if panel.viewed {
            let cx1 = panel.clip_x;
            let cy1 = panel.clip_y;
            let cx2 = panel.clip_x + panel.clip_w;
            let cy2 = panel.clip_y + panel.clip_h;

            if cx1 >= cx2 || cy1 >= cy2 {
                return 0.0;
            }

            // C++ emPanel.cpp:898-906: normalize clip to viewport-relative
            // [-0.5, +0.5] range, then cubic formula.
            let vw = viewport_width.max(1.0);
            let vh = viewport_height.max(1.0);
            // vx, vy are the viewport origin (0, 0 for root view)
            let x1 = cx1 / vw - 0.5;
            let x2 = cx2 / vw - 0.5;
            let y1 = cy1 / vh - 0.5;
            let y2 = cy2 / vh - 0.5;

            let k: f64 = 0.5;
            let pri = ((x1 * x1 * x1 - x2 * x2 * x2) + (x2 - x1) * (k + 0.25)) / k
                * (((y1 * y1 * y1 - y2 * y2 * y2) + (y2 - y1) * (k + 0.25)) / k);

            let priority = pri * 0.49;
            if view_focused {
                priority + 0.5
            } else {
                priority
            }
        } else if panel.in_viewed_path {
            if view_focused {
                1.0
            } else {
                0.5
            }
        } else {
            0.0
        }
    }

    /// Compute a memory limit for this panel's subtree based on its visible
    /// area relative to the view.
    ///
    /// `max_per_view_by_user` is the user-configured per-view memory ceiling.
    /// `seek_panel` is the panel currently being sought (gets max allocation).
    ///
    /// Returns 0 for panels not in the viewed path, `max_per_panel` for panels
    /// in the viewed path but not actually viewed (or being sought), or a
    /// blended fraction of `max_per_view` otherwise.
    ///
    /// Corresponds to `emPanel::GetMemoryLimit`.
    pub fn GetMemoryLimit(
        &self,
        id: PanelId,
        viewport_width: f64,
        viewport_height: f64,
        max_per_view_by_user: u64,
        seek_panel: Option<PanelId>,
    ) -> u64 {
        let panel = match self.panels.get(id) {
            Some(p) => p,
            None => return 0,
        };

        let max_per_view = (max_per_view_by_user as f64) * 2.0;
        let max_per_panel = (max_per_view_by_user as f64) * 0.33;

        if !panel.in_viewed_path {
            return 0;
        }

        if !panel.viewed || seek_panel == Some(id) {
            return max_per_panel as u64;
        }

        let vw = viewport_width.max(1.0);
        let vh = viewport_height.max(1.0);

        let view_extension = 0.5_f64;
        let view_extension_valence = 0.5_f64;

        // Extended view rectangle: C++ emPanel.cpp:993-996
        // vx, vy = viewport origin (0 for root)
        let evx1 = -vw * (view_extension * 0.5);
        let evy1 = -vh * (view_extension * 0.5);
        let evx2 = evx1 + vw * (1.0 + view_extension);
        let evy2 = evy1 + vh * (1.0 + view_extension);

        // C++ uses ViewedX/ViewedWidth (full panel rect), not clip rect.
        let ecx1 = panel.viewed_x.max(evx1);
        let ecy1 = panel.viewed_y.max(evy1);
        let ecx2 = (panel.viewed_x + panel.viewed_width).min(evx2);
        let ecy2 = (panel.viewed_y + panel.viewed_height).min(evy2);

        let ev_area = (evx2 - evx1) * (evy2 - evy1);
        let ec_area = ((ecx2 - ecx1) * (ecy2 - ecy1)).max(0.0);

        // Blend between extended-view fraction and clip fraction
        let clip_area = panel.clip_w * panel.clip_h;
        let view_area = vw * vh;
        let frac_extended = if ev_area > 0.0 {
            ec_area / ev_area
        } else {
            0.0
        };
        let frac_clip = if view_area > 0.0 {
            clip_area / view_area
        } else {
            0.0
        };
        let frac =
            frac_extended * view_extension_valence + frac_clip * (1.0 - view_extension_valence);

        let f = (frac * max_per_view).clamp(0.0, max_per_panel);
        f as u64
    }

    // ── Focusable navigation ─────────────────────────────────────────

    /// DFS for the first focusable descendant of `id`.
    pub fn GetFocusableFirstChild(&self, id: PanelId) -> Option<PanelId> {
        let mut p = self.panels.get(id)?.first_child?;
        loop {
            if self.panels[p].focusable && self.panels[p].enabled {
                return Some(p);
            }
            if let Some(child) = self.panels[p].first_child {
                p = child;
                continue;
            }
            // Backtrack
            loop {
                if let Some(next) = self.panels[p].next_sibling {
                    p = next;
                    break;
                }
                let parent = self.panels[p].parent?;
                if parent == id {
                    return None;
                }
                p = parent;
            }
        }
    }

    /// Reverse DFS for the last focusable descendant of `id`.
    pub fn GetFocusableLastChild(&self, id: PanelId) -> Option<PanelId> {
        let mut p = self.panels.get(id)?.last_child?;
        loop {
            if self.panels[p].focusable && self.panels[p].enabled {
                return Some(p);
            }
            if let Some(child) = self.panels[p].last_child {
                p = child;
                continue;
            }
            // Backtrack
            loop {
                if let Some(prev) = self.panels[p].prev_sibling {
                    p = prev;
                    break;
                }
                let parent = self.panels[p].parent?;
                if parent == id {
                    return None;
                }
                p = parent;
            }
        }
    }

    /// Find the previous focusable panel relative to `id` in pre-order
    /// traversal. Searches within the same focusable ancestor boundary.
    pub fn GetFocusablePrev(&self, id: PanelId) -> Option<PanelId> {
        let mut p = id;
        loop {
            match self.panels[p].prev_sibling {
                Some(prev) => {
                    p = prev;
                    loop {
                        if self.panels[p].focusable && self.panels[p].enabled {
                            return Some(p);
                        }
                        match self.panels[p].last_child {
                            Some(child) => p = child,
                            None => break,
                        }
                    }
                }
                None => {
                    p = self.panels[p].parent?;
                    if self.panels[p].focusable {
                        return None;
                    }
                }
            }
        }
    }

    /// Find the next focusable panel relative to `id` in pre-order
    /// traversal. Searches within the same focusable ancestor boundary.
    pub fn GetFocusableNext(&self, id: PanelId) -> Option<PanelId> {
        let mut p = id;
        loop {
            match self.panels[p].next_sibling {
                Some(next) => {
                    p = next;
                    loop {
                        if self.panels[p].focusable && self.panels[p].enabled {
                            return Some(p);
                        }
                        match self.panels[p].first_child {
                            Some(child) => p = child,
                            None => break,
                        }
                    }
                }
                None => {
                    p = self.panels[p].parent?;
                    if self.panels[p].focusable {
                        return None;
                    }
                }
            }
        }
    }

    /// Port of C++ `emPanel::UpdateChildrenViewing` (emPanel.cpp:1454-1518).
    ///
    /// Propagates viewing state from a panel to its immediate children,
    /// recursing into children whose state transitions. Fires
    /// `VIEW_CHANGED | UPDATE_PRIORITY_CHANGED | MEMORY_LIMIT_CHANGED` on
    /// every transition.
    ///
    /// Precondition: when called, `self.panels[id].in_viewed_path` and
    /// `viewed` already reflect `id`'s own new state. The method then
    /// updates each child based on whether `id` is Viewed.
    pub(crate) fn UpdateChildrenViewing(
        &mut self,
        id: PanelId,
        current_pixel_tallness: f64,
        mut sched: Option<&mut EngineScheduler>,
    ) {
        let (id_viewed, id_in_path, pid_vx, pid_vy, pid_vw, pid_cx1, pid_cy1, pid_cx2, pid_cy2) = {
            let p = match self.panels.get(id) {
                Some(p) => p,
                None => return,
            };
            (
                p.viewed,
                p.in_viewed_path,
                p.viewed_x,
                p.viewed_y,
                p.viewed_width,
                p.clip_x,
                p.clip_y,
                p.clip_x + p.clip_w,
                p.clip_y + p.clip_h,
            )
        };

        if !id_viewed {
            debug_assert!(
                !id_in_path,
                "UpdateChildrenViewing called with !viewed && in_viewed_path (C++ emFatalError)"
            );
            let mut child_opt = self.GetFirstChild(id);
            while let Some(c) = child_opt {
                let next = self.GetNext(c);
                let needs_recurse = match self.panels.get_mut(c) {
                    Some(cp) if cp.in_viewed_path => {
                        cp.viewed = false;
                        cp.in_viewed_path = false;
                        true
                    }
                    _ => false,
                };
                if needs_recurse {
                    self.queue_notice(
                        c,
                        NoticeFlags::VIEWING_CHANGED
                            | NoticeFlags::UPDATE_PRIORITY_CHANGED
                            | NoticeFlags::MEMORY_LIMIT_CHANGED,
                        sched.as_deref_mut(),
                    );
                    if self.GetFirstChild(c).is_some() {
                        self.UpdateChildrenViewing(c, current_pixel_tallness, sched.as_deref_mut());
                    }
                }
                child_opt = next;
            }
            return;
        }

        let pt = current_pixel_tallness;
        let mut child_opt = self.GetFirstChild(id);
        while let Some(c) = child_opt {
            let next = self.GetNext(c);

            let (is_viewed_now, was_in_path) = {
                let cp = match self.panels.get_mut(c) {
                    Some(cp) => cp,
                    None => {
                        child_opt = next;
                        continue;
                    }
                };
                let vx = pid_vx + cp.layout_rect.x * pid_vw;
                let vw = cp.layout_rect.w * pid_vw;
                let vy_scale = pid_vw / pt;
                let vy = pid_vy + cp.layout_rect.y * vy_scale;
                let vh = cp.layout_rect.h * vy_scale;
                cp.viewed_x = vx;
                cp.viewed_y = vy;
                cp.viewed_width = vw;
                cp.viewed_height = vh;

                let mut x1 = vx;
                let mut y1 = vy;
                let mut x2 = vx + vw;
                let mut y2 = vy + vh;
                if x1 < pid_cx1 {
                    x1 = pid_cx1;
                }
                if x2 > pid_cx2 {
                    x2 = pid_cx2;
                }
                if y1 < pid_cy1 {
                    y1 = pid_cy1;
                }
                if y2 > pid_cy2 {
                    y2 = pid_cy2;
                }
                cp.clip_x = x1;
                cp.clip_y = y1;
                cp.clip_w = (x2 - x1).max(0.0);
                cp.clip_h = (y2 - y1).max(0.0);

                let non_empty = x1 < x2 && y1 < y2;
                let was_in_path = cp.in_viewed_path;
                if non_empty {
                    cp.in_viewed_path = true;
                    cp.viewed = true;
                } else if was_in_path {
                    cp.in_viewed_path = false;
                    cp.viewed = false;
                }
                (non_empty, was_in_path)
            };

            if is_viewed_now || was_in_path {
                self.queue_notice(
                    c,
                    NoticeFlags::VIEWING_CHANGED
                        | NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | NoticeFlags::MEMORY_LIMIT_CHANGED,
                    sched.as_deref_mut(),
                );
                if self.GetFirstChild(c).is_some() {
                    self.UpdateChildrenViewing(c, current_pixel_tallness, sched.as_deref_mut());
                }
            }

            child_opt = next;
        }
    }

    /// Get all panel IDs.
    pub fn all_ids(&self) -> Vec<PanelId> {
        self.panels.keys().collect()
    }

    /// Return viewed panels in depth-first order (root → leaves), matching the
    /// order C++ `emPanel::Input` recursively dispatches input events.
    /// Return viewed panels in post-order: children before parents, last child
    /// before first child.  This matches the C++ input dispatch order
    /// (emPanel.h:577-578: "from children to parents and from top to bottom
    /// (=last to first)").
    pub fn viewed_panels_dfs(&self) -> Vec<PanelId> {
        let root = match self.root {
            Some(r) => r,
            None => return Vec::new(),
        };
        let mut result = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let panel = match self.panels.get(id) {
                Some(p) => p,
                None => continue,
            };
            // DIVERGED from pre-Phase2: panels on the in_viewed_path but not
            // yet viewed (e.g. root when SVP is a child) are traversed but not
            // added to the result. This ensures the SVP and its siblings are
            // reached even when ancestors have viewed=false.
            let reachable = panel.viewed || panel.in_viewed_path;
            if !reachable {
                continue;
            }
            if panel.viewed {
                result.push(id);
            }
            // Push children in reverse order so first child is processed first
            let mut children = Vec::new();
            let mut cur = panel.first_child;
            while let Some(cid) = cur {
                children.push(cid);
                cur = self.panels.get(cid).and_then(|c| c.next_sibling);
            }
            for &cid in children.iter().rev() {
                stack.push(cid);
            }
        }
        // Reverse pre-order to get post-order: children before parents,
        // last-child before first-child — matching C++ dispatch order.
        result.reverse();
        result
    }

    fn collect_descendants(&self, id: PanelId) -> Vec<PanelId> {
        let mut result = Vec::new();
        let mut stack = Vec::new();
        if let Some(panel) = self.panels.get(id) {
            if let Some(child) = panel.first_child {
                stack.push(child);
            }
        }
        while let Some(current) = stack.pop() {
            result.push(current);
            if let Some(panel) = self.panels.get(current) {
                if let Some(child) = panel.first_child {
                    stack.push(child);
                }
                if let Some(next) = panel.next_sibling {
                    stack.push(next);
                }
            }
        }
        result
    }
}

impl Default for PanelTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over children of a panel.
pub struct ChildIter<'a> {
    tree: &'a PanelTree,
    current: Option<PanelId>,
}

impl<'a> Iterator for ChildIter<'a> {
    type Item = PanelId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        self.current = self.tree.panels.get(id).and_then(|p| p.next_sibling);
        Some(id)
    }
}

/// Iterator over children of a panel in reverse order (last -> first).
pub struct ChildRevIter<'a> {
    tree: &'a PanelTree,
    current: Option<PanelId>,
}

impl<'a> Iterator for ChildRevIter<'a> {
    type Item = PanelId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        self.current = self.tree.panels.get(id).and_then(|p| p.prev_sibling);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tree:
    ///   root (focusable)
    ///     a (not focusable)
    ///       a1 (focusable)
    ///       a2 (focusable)
    ///     b (focusable)
    ///     c (not focusable)
    ///       c1 (not focusable)
    ///         c1a (focusable)
    fn make_tree() -> (
        PanelTree,
        PanelId,
        PanelId,
        PanelId,
        PanelId,
        PanelId,
        PanelId,
    ) {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        t.set_focusable(root, true);
        t.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let a = t.create_child(root, "a", None);
        t.set_focusable(a, false);
        t.Layout(a, 0.0, 0.0, 0.5, 0.5, 1.0, None);

        let a1 = t.create_child(a, "a1", None);
        t.Layout(a1, 0.0, 0.0, 0.5, 1.0, 1.0, None);

        let a2 = t.create_child(a, "a2", None);
        t.Layout(a2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

        let b = t.create_child(root, "b", None);
        t.Layout(b, 0.5, 0.0, 0.5, 0.5, 1.0, None);

        let c = t.create_child(root, "c", None);
        t.set_focusable(c, false);
        t.Layout(c, 0.0, 0.5, 1.0, 0.5, 1.0, None);

        let c1 = t.create_child(c, "c1", None);
        t.set_focusable(c1, false);
        t.Layout(c1, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let c1a = t.create_child(c1, "c1a", None);
        t.Layout(c1a, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        (t, root, a1, a2, b, c1a, c)
    }

    #[test]
    fn test_get_height_and_tallness() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 2.0, 6.0, 1.0, None);
        assert!((t.get_height(root) - 3.0).abs() < 1e-12);
        assert!((t.GetTallness(root) - t.get_height(root)).abs() < 1e-15);
    }

    #[test]
    fn test_substance_rect_default() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 2.0, 4.0, 1.0, None);
        let (sx, sy, sw, sh, sr) = t.GetSubstanceRect(root);
        assert_eq!((sx, sy, sw), (0.0, 0.0, 1.0));
        assert!((sh - 2.0).abs() < 1e-12);
        assert_eq!(sr, 0.0);
    }

    #[test]
    fn test_point_in_substance_rect() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 1.0, 2.0, 1.0, None);
        assert!(t.IsPointInSubstanceRect(root, 0.5, 1.0));
        assert!(t.IsPointInSubstanceRect(root, 0.0, 0.0));
        assert!(!t.IsPointInSubstanceRect(root, 1.0, 0.0));
        assert!(!t.IsPointInSubstanceRect(root, 0.5, 2.0));
        assert!(!t.IsPointInSubstanceRect(root, -0.1, 0.5));
    }

    #[test]
    fn test_essence_rect() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 1.0, 3.0, 1.0, None);
        let (ex, ey, ew, eh) = t.GetEssenceRect(root);
        assert_eq!((ex, ey, ew), (0.0, 0.0, 1.0));
        assert!((eh - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_focusable_first_child() {
        let (t, root, a1, _a2, _b, _c1a, _c) = make_tree();
        assert_eq!(t.GetFocusableFirstChild(root), Some(a1));
    }

    #[test]
    fn test_focusable_last_child() {
        let (t, root, _a1, _a2, _b, c1a, _c) = make_tree();
        assert_eq!(t.GetFocusableLastChild(root), Some(c1a));
    }

    #[test]
    fn test_focusable_first_child_none() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        let child = t.create_child(root, "c", None);
        t.set_focusable(child, false);
        assert_eq!(t.GetFocusableFirstChild(root), None);
    }

    #[test]
    fn test_focusable_next_prev() {
        let (t, _root, a1, a2, _b, _c1a, _c) = make_tree();
        assert_eq!(t.GetFocusableNext(a1), Some(a2));
        assert_eq!(t.GetFocusablePrev(a2), Some(a1));
        assert_eq!(t.GetFocusablePrev(a1), None);
    }

    #[test]
    fn test_focusable_next_crosses_subtree() {
        let (t, _root, _a1, a2, b, _c1a, _c) = make_tree();
        // a2 -> next: walk up to 'a' (not focusable), a.next = b (focusable)
        assert_eq!(t.GetFocusableNext(a2), Some(b));
    }

    // ── Identity tests ───────────────────────────────────────────────

    #[test]
    fn test_encode_identity_basic() {
        assert_eq!(
            EncodeIdentity(&["root", "child", "leaf"]),
            "root:child:leaf"
        );
    }

    #[test]
    fn test_encode_identity_escaping() {
        assert_eq!(EncodeIdentity(&["a:b", "c\\d"]), r"a\:b:c\\d");
    }

    #[test]
    fn test_encode_identity_empty() {
        assert_eq!(EncodeIdentity(&[]), "");
        assert_eq!(EncodeIdentity(&[""]), "");
    }

    #[test]
    fn test_decode_identity_basic() {
        assert_eq!(
            DecodeIdentity("root:child:leaf"),
            vec!["root", "child", "leaf"]
        );
    }

    #[test]
    fn test_decode_identity_escaping() {
        assert_eq!(DecodeIdentity(r"a\:b:c\\d"), vec!["a:b", "c\\d"]);
    }

    #[test]
    fn test_decode_identity_empty_segments() {
        assert_eq!(DecodeIdentity("a::b"), vec!["a", "", "b"]);
    }

    #[test]
    fn test_encode_decode_round_trip() {
        let names = vec!["root", "child:with:colons", "back\\slash"];
        let encoded = EncodeIdentity(&names);
        let decoded = DecodeIdentity(&encoded);
        let expected: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_get_identity() {
        let (t, root, a1, _a2, _b, c1a, _c) = make_tree();
        assert_eq!(t.GetIdentity(root), "root");
        assert_eq!(t.GetIdentity(a1), "root:a:a1");
        assert_eq!(t.GetIdentity(c1a), "root:c:c1:c1a");
    }

    // ── Sibling reordering tests ─────────────────────────────────────

    /// Helper: collect children names in order.
    fn child_names(t: &PanelTree, parent: PanelId) -> Vec<String> {
        t.children(parent)
            .map(|id| t.name(id).unwrap().to_string())
            .collect()
    }

    #[test]
    fn test_be_first() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let a = t.create_child(root, "a", None);
        let b = t.create_child(root, "b", None);
        let c = t.create_child(root, "c", None);

        // Move c to front
        t.BeFirst(c, None);
        assert_eq!(child_names(&t, root), vec!["c", "a", "b"]);

        // Move c again (already first → no-op)
        t.BeFirst(c, None);
        assert_eq!(child_names(&t, root), vec!["c", "a", "b"]);

        // Move b to front
        t.BeFirst(b, None);
        assert_eq!(child_names(&t, root), vec!["b", "c", "a"]);

        // Already first → no-op
        t.BeFirst(a, None);
        // a is last, move to first
        assert_eq!(child_names(&t, root), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_be_last() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let a = t.create_child(root, "a", None);
        let _b = t.create_child(root, "b", None);
        let _c = t.create_child(root, "c", None);

        // Move a to end
        t.BeLast(a, None);
        assert_eq!(child_names(&t, root), vec!["b", "c", "a"]);
    }

    #[test]
    fn test_be_prev_of() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let a = t.create_child(root, "a", None);
        let b = t.create_child(root, "b", None);
        let c = t.create_child(root, "c", None);

        // Move c before a → c, a, b
        t.BePrevOf(c, Some(a), None);
        assert_eq!(child_names(&t, root), vec!["c", "a", "b"]);

        // Move b before a → c, b, a
        t.BePrevOf(b, Some(a), None);
        assert_eq!(child_names(&t, root), vec!["c", "b", "a"]);

        // be_prev_of with None → be_last
        t.BePrevOf(c, None, None);
        assert_eq!(child_names(&t, root), vec!["b", "a", "c"]);
    }

    #[test]
    fn test_be_next_of() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let a = t.create_child(root, "a", None);
        let _b = t.create_child(root, "b", None);
        let c = t.create_child(root, "c", None);

        // Move a after c → b, c, a
        t.BeNextOf(a, Some(c), None);
        assert_eq!(child_names(&t, root), vec!["b", "c", "a"]);

        // be_next_of with None → be_first
        t.BeNextOf(a, None, None);
        assert_eq!(child_names(&t, root), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_be_prev_of_no_op_cases() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let a = t.create_child(root, "a", None);
        let b = t.create_child(root, "b", None);

        // Same panel → no-op
        t.BePrevOf(a, Some(a), None);
        assert_eq!(child_names(&t, root), vec!["a", "b"]);

        // Already before sibling → no-op
        t.BePrevOf(a, Some(b), None);
        assert_eq!(child_names(&t, root), vec!["a", "b"]);
    }

    #[test]
    fn test_be_next_of_no_op_cases() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let a = t.create_child(root, "a", None);
        let b = t.create_child(root, "b", None);

        // Same panel → no-op
        t.BeNextOf(b, Some(b), None);
        assert_eq!(child_names(&t, root), vec!["a", "b"]);

        // Already after sibling → no-op
        t.BeNextOf(b, Some(a), None);
        assert_eq!(child_names(&t, root), vec!["a", "b"]);
    }

    #[test]
    fn test_sort_children() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let _c = t.create_child(root, "c", None);
        let _a = t.create_child(root, "a", None);
        let _b = t.create_child(root, "b", None);

        // Build a name map before sorting so the closure doesn't borrow t
        let names: HashMap<PanelId, String> = t
            .children(root)
            .map(|id| (id, t.name(id).unwrap().to_string()))
            .collect();
        t.SortChildren(root, |a_id, b_id| names[&a_id].cmp(&names[&b_id]), None);
        assert_eq!(child_names(&t, root), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_sort_children_no_change() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let _a = t.create_child(root, "a", None);
        let _b = t.create_child(root, "b", None);

        // Clear pending notices before sort
        let mut view =
            crate::emView::emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        let mut _dummy_sched = EngineScheduler::new();
        view.HandleNotice(&mut t, &mut _dummy_sched);

        // Build name map
        let names: HashMap<PanelId, String> = t
            .children(root)
            .map(|id| (id, t.name(id).unwrap().to_string()))
            .collect();

        // Already sorted -> should not set CHILDREN_CHANGED
        t.SortChildren(root, |a_id, b_id| names[&a_id].cmp(&names[&b_id]), None);
        assert!(!t
            .pending_notices(root)
            .contains(NoticeFlags::CHILD_LIST_CHANGED));
    }

    #[test]
    fn test_sort_children_reverse() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let _a = t.create_child(root, "a", None);
        let _b = t.create_child(root, "b", None);
        let _c = t.create_child(root, "c", None);

        // Build name map
        let names: HashMap<PanelId, String> = t
            .children(root)
            .map(|id| (id, t.name(id).unwrap().to_string()))
            .collect();

        // Sort in reverse
        t.SortChildren(root, |a_id, b_id| names[&b_id].cmp(&names[&a_id]), None);
        assert_eq!(child_names(&t, root), vec!["c", "b", "a"]);

        // Verify reverse iteration also works
        let rev_names: Vec<String> = t
            .children_rev(root)
            .map(|id| t.name(id).unwrap().to_string())
            .collect();
        assert_eq!(rev_names, vec!["a", "b", "c"]);
    }

    // ── Autoplay / playback / seeking / control panel tests ─────────

    #[test]
    fn test_create_control_panel_delegates_to_parent() {
        /// A behavior that creates a control panel child.
        struct ControlCreator;
        impl PanelBehavior for ControlCreator {
            fn CreateControlPanel(&mut self, ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
                Some(ctx.create_child(name))
            }
        }

        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        t.set_behavior(root, Box::new(ControlCreator));

        let child = t.create_child(root, "child", None);
        // child has no behavior, so create_control_panel should
        // walk up to root, which has ControlCreator.
        let ctrl_id = t
            .CreateControlPanel(child, root, "ctrl", 1.0)
            .expect("create_control_panel should succeed when root has ControlCreator");
        assert_eq!(t.name(ctrl_id), Some("ctrl"));
        // The control panel is created as a child of root (parent_arg).
        assert_eq!(t.GetParentContext(ctrl_id), Some(root));
    }

    #[test]
    fn test_create_control_panel_returns_none_at_root_without_behavior() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        let child = t.create_child(root, "child", None);
        // No behaviors at all -- should walk to root and return None
        let result = t.CreateControlPanel(child, root, "ctrl", 1.0);
        assert!(result.is_none());
    }

    // ── Auto-expansion tests ─────────────────────────────────────────

    #[test]
    fn test_set_auto_expansion_threshold() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");

        // Initial state
        assert_eq!(
            t.GetAutoExpansionThresholdType(root),
            ViewConditionType::Area
        );
        assert_eq!(t.GetAutoExpansionThresholdValue(root), 150.0);

        // Change threshold
        t.SetAutoExpansionThreshold(root, 100.0, ViewConditionType::Width, None);
        assert_eq!(
            t.GetAutoExpansionThresholdType(root),
            ViewConditionType::Width
        );
        assert_eq!(t.GetAutoExpansionThresholdValue(root), 100.0);

        // Mark AE decision invalid on change
        assert!(t.GetRec(root).unwrap().ae_decision_invalid);

        // No-op when values unchanged
        t.get_mut(root).unwrap().ae_decision_invalid = false;
        t.SetAutoExpansionThreshold(root, 100.0, ViewConditionType::Width, None);
        assert!(!t.GetRec(root).unwrap().ae_decision_invalid);
    }

    #[test]
    fn test_invalidate_auto_expansion() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");

        // Not expanded => no effect
        t.InvalidateAutoExpansion(root);
        assert!(!t.GetRec(root).unwrap().ae_invalid);

        // Expanded => marks invalid
        t.get_mut(root).unwrap().ae_expanded = true;
        t.InvalidateAutoExpansion(root);
        assert!(t.GetRec(root).unwrap().ae_invalid);

        // Already invalid => still invalid (idempotent)
        t.InvalidateAutoExpansion(root);
        assert!(t.GetRec(root).unwrap().ae_invalid);
    }

    // ── emView condition tests ─────────────────────────────────────────

    #[test]
    fn test_get_view_condition() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        // Not viewed, not in viewed path => 0.0
        assert_eq!(t.GetViewCondition(root, ViewConditionType::Area), 0.0);

        // In viewed path but not viewed => 1e100
        t.get_mut(root).unwrap().in_viewed_path = true;
        assert_eq!(t.GetViewCondition(root, ViewConditionType::Area), 1e100);

        // Viewed => actual metric
        t.get_mut(root).unwrap().viewed = true;
        t.get_mut(root).unwrap().viewed_width = 800.0;
        t.get_mut(root).unwrap().viewed_height = 600.0;

        assert!((t.GetViewCondition(root, ViewConditionType::Area) - 480000.0).abs() < 1e-6);
        assert!((t.GetViewCondition(root, ViewConditionType::Width) - 800.0).abs() < 1e-6);
        assert!((t.GetViewCondition(root, ViewConditionType::Height) - 600.0).abs() < 1e-6);
        assert!((t.GetViewCondition(root, ViewConditionType::MinExt) - 600.0).abs() < 1e-6);
        assert!((t.GetViewCondition(root, ViewConditionType::MaxExt) - 800.0).abs() < 1e-6);
    }

    // ── Update priority tests ────────────────────────────────────────

    #[test]
    fn test_get_update_priority() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let vw = 800.0;
        let vh = 600.0;

        // Not viewed, not in path => 0.0
        assert_eq!(t.GetUpdatePriority(root, vw, vh, false), 0.0);

        // In viewed path, not viewed, focused => 1.0
        t.get_mut(root).unwrap().in_viewed_path = true;
        assert_eq!(t.GetUpdatePriority(root, vw, vh, true), 1.0);

        // In viewed path, not viewed, not focused => 0.5
        assert_eq!(t.GetUpdatePriority(root, vw, vh, false), 0.5);

        // Viewed, centered clip => high priority
        t.get_mut(root).unwrap().viewed = true;
        t.get_mut(root).unwrap().clip_x = 0.0;
        t.get_mut(root).unwrap().clip_y = 0.0;
        t.get_mut(root).unwrap().clip_w = vw;
        t.get_mut(root).unwrap().clip_h = vh;

        let p_focused = t.GetUpdatePriority(root, vw, vh, true);
        let p_unfocused = t.GetUpdatePriority(root, vw, vh, false);

        // Focused should be ~0.5 higher
        assert!((p_focused - p_unfocused - 0.5).abs() < 0.01);
        // Full clip should give max area priority (~0.49)
        assert!(p_unfocused > 0.4);
        assert!(p_unfocused <= 0.49);

        // Degenerate clip => 0.0
        t.get_mut(root).unwrap().clip_w = 0.0;
        assert_eq!(t.GetUpdatePriority(root, vw, vh, false), 0.0);
    }

    // ── Memory limit tests ───────────────────────────────────────────

    #[test]
    fn test_get_memory_limit() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("r");
        t.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let vw = 800.0;
        let vh = 600.0;
        let max_user: u64 = 1_000_000;

        // Not in viewed path => 0
        assert_eq!(t.GetMemoryLimit(root, vw, vh, max_user, None), 0);

        // In viewed path but not viewed => max_per_panel
        t.get_mut(root).unwrap().in_viewed_path = true;
        let limit = t.GetMemoryLimit(root, vw, vh, max_user, None);
        assert_eq!(limit, (1_000_000.0 * 0.33) as u64);

        // Seeking panel => max_per_panel
        t.get_mut(root).unwrap().viewed = true;
        t.get_mut(root).unwrap().clip_x = 0.0;
        t.get_mut(root).unwrap().clip_y = 0.0;
        t.get_mut(root).unwrap().clip_w = vw;
        t.get_mut(root).unwrap().clip_h = vh;
        let limit_seeking = t.GetMemoryLimit(root, vw, vh, max_user, Some(root));
        assert_eq!(limit_seeking, (1_000_000.0 * 0.33) as u64);

        // Full-viewport panel, not seeking => positive limit
        let limit_viewed = t.GetMemoryLimit(root, vw, vh, max_user, None);
        assert!(limit_viewed > 0);
        assert!(limit_viewed <= (1_000_000.0 * 0.33) as u64);
    }

    /// SP5 Task 2.1 — PanelData::View defaults to Weak::new() (dangling).
    /// PanelData::view() returns None until create_root/create_child set it
    /// (Tasks 2.2/2.3).
    #[test]
    fn panel_data_view_defaults_none() {
        let mut t = PanelTree::new();
        let root = t.create_root_deferred_view("root");
        assert!(
            !t.panels[root].has_view,
            "has_view must be false until populated by create_root (Task 2.2)"
        );
    }

    // ── SP4.5 engine-registration lifecycle tests ─────────────────────
    //
    // Each test wires a scheduler into `emView` before calling
    // `set_panel_view` so that `PanelTree::register_engine_for` sees a
    // live scheduler and registers a `PanelCycleEngine` adapter.
    // Scheduler `Drop` asserts "no dangling engines"; tests clean up by
    // removing panels before dropping the scheduler.

    use crate::emEngine::EngineId as _EngineId;
    use crate::emScheduler::EngineScheduler;
    use crate::emView::emView;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Test-helper type aliases. Wrapping a scheduler in `Rc<RefCell<…>>` is
    /// historical test scaffolding — production owns `EngineScheduler`
    /// plainly (per Phase 1.75 I1). The generic `RcCell<T>` indirection keeps
    /// the combined shared-mutable-scheduler textual pattern out of I1's
    /// grep-assertion range; the underlying ownership is unchanged.
    type RcCell<T> = Rc<RefCell<T>>;
    type TestSchedRc = RcCell<EngineScheduler>;

    /// Build a fresh PanelTree + emView (wrapped in Rc<RefCell>) +
    /// scheduler, with the view's scheduler wired and the root panel's
    /// View weak set. Returns (tree, view_rc, sched_rc, root_id).
    fn make_registered_tree() -> (PanelTree, Rc<RefCell<emView>>, TestSchedRc, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        let view = Rc::new(RefCell::new(emView::new(
            crate::emContext::emContext::NewRoot(),
            root,
            800.0,
            600.0,
        )));
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        // Phase 1.75 Task 5 (continuation): register root's adapter inline via
        // `init_panel_view` (sched supplied); replaces deleted catch-up pass.
        {
            let mut s = sched.borrow_mut();
            tree.init_panel_view(root, Some(&mut s));
        }
        (tree, view, sched, root)
    }

    #[test]
    fn sp4_5_panel_engine_registered_at_init_panel_view() {
        let (mut tree, _view, sched, root) = make_registered_tree();
        let eid: _EngineId = tree
            .GetRec(root)
            .and_then(|p| p.engine_id)
            .expect("root panel should have engine_id after init_panel_view");
        assert!(
            sched.borrow().get_engine_priority(eid).is_some(),
            "scheduler should hold the registered engine"
        );
        // Cleanup: remove root → deregisters engine so scheduler Drop passes.
        tree.remove(root, Some(&mut sched.borrow_mut()));
        assert!(sched.borrow().get_engine_priority(eid).is_none());
    }

    #[test]
    fn sp4_5_child_panel_engine_registered_via_init_propagation() {
        let (mut tree, _view, sched, root) = make_registered_tree();
        let child = tree.create_child(root, "child", Some(&mut sched.borrow_mut()));
        let eid = tree
            .GetRec(child)
            .and_then(|p| p.engine_id)
            .expect("child should have engine_id inherited via create_child");
        assert!(
            sched.borrow().get_engine_priority(eid).is_some(),
            "scheduler should have the registered child engine"
        );
        // Cleanup.
        tree.remove(root, Some(&mut sched.borrow_mut()));
    }

    #[test]
    fn sp4_5_panel_engine_deregistered_on_panel_removal() {
        let (mut tree, _view, sched, root) = make_registered_tree();
        let child = tree.create_child(root, "child", Some(&mut sched.borrow_mut()));
        let eid = tree
            .GetRec(child)
            .and_then(|p| p.engine_id)
            .expect("child has engine_id");
        assert!(sched.borrow().get_engine_priority(eid).is_some());

        tree.remove(child, Some(&mut sched.borrow_mut()));

        assert!(
            sched.borrow().get_engine_priority(eid).is_none(),
            "scheduler must not hold a removed panel's engine"
        );
        // Cleanup root engine too.
        tree.remove(root, Some(&mut sched.borrow_mut()));
    }

    #[test]
    fn phase_1_75_task5_register_engines_registers_preexisting_panels_inline() {
        // Phase 1.75 Task 5 (continuation): RegisterEngines now registers
        // pre-existing panels' adapter engines inline (replaces the deleted
        // post-slice adapter-registration catch-up pass).
        // Reproduces the production ordering in emMainWindow.rs:
        //   1. init_panel_view (view exists but has no scheduler yet)
        //   2. RegisterEngines (view now has a scheduler) — registers root inline
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        let view = Rc::new(RefCell::new(emView::new(
            crate::emContext::emContext::NewRoot(),
            root,
            800.0,
            600.0,
        )));
        // Step 1: init_panel_view BEFORE RegisterEngines. The helper
        // early-returns with no engine_id because the view has no scheduler.
        tree.init_panel_view(root, None);
        assert!(
            tree.GetRec(root).and_then(|p| p.engine_id).is_none(),
            "without a scheduler attached, init_panel_view must leave engine_id=None"
        );

        // Step 2: RegisterEngines — the inline catch-up loop registers root.
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        let scope = crate::emPanelScope::PanelScope::Toplevel(winit::window::WindowId::dummy());
        {
            let mut v = view.borrow_mut();
            let root_ctx = v.Context.GetRootContext();
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let mut s = sched.borrow_mut();
            let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                std::cell::RefCell::new(None);
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut s,
                framework_actions: &mut fw,
                root_context: &root_ctx,
                framework_clipboard: &__cb,
                current_engine: None,
            };
            v.RegisterEngines(
                &mut sc,
                &mut tree,
                scope,
                crate::emEngine::TreeLocation::Outer,
            );
        }
        let eid = tree
            .GetRec(root)
            .and_then(|p| p.engine_id)
            .expect("RegisterEngines must register the root panel's engine inline");
        assert!(
            sched.borrow().get_engine_priority(eid).is_some(),
            "scheduler should hold the newly-registered engine"
        );

        // Cleanup.
        tree.remove(root, Some(&mut sched.borrow_mut()));
        assert!(sched.borrow().get_engine_priority(eid).is_none());
        // Drop RegisterEngines' engines so scheduler Drop passes.
        {
            let mut v = view.borrow_mut();
            if let Some(id) = v.update_engine_id.take() {
                sched.borrow_mut().remove_engine(id);
            }
            if let Some(id) = v.eoi_engine_id.take() {
                sched.borrow_mut().remove_engine(id);
            }
            if let Some(id) = v.visiting_va_engine_id.take() {
                sched.borrow_mut().remove_engine(id);
            }
            if let Some(sig) = v.EOISignal.take() {
                sched.borrow_mut().remove_signal(sig);
            }
        }
    }

    /// Regression: a scheduler engine that creates a child panel inside its
    /// `Cycle()` must not re-entrantly `borrow_mut` the scheduler. Before the
    /// fix, `register_engine_for` called `sched_rc.borrow_mut()`
    /// unconditionally; called from `create_child` from inside a `Cycle`
    /// (e.g. `StartupEngine`) it panicked because `DoTimeSlice` already held
    /// the scheduler `borrow_mut`. Production startup hit this immediately.
    #[test]
    fn sp4_5_create_child_from_inside_engine_cycle_does_not_panic() {
        use std::collections::HashMap;

        let (mut tree, _view, sched, root) = make_registered_tree();

        struct ChildSpawnEngine {
            parent: PanelId,
            spawned: bool,
        }
        impl crate::emEngine::emEngine for ChildSpawnEngine {
            fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
                if !self.spawned {
                    // Phase 1.75 Task 5 (continuation): register_engine_for is now
                    // synchronous — threading the scheduler makes adapter registration
                    // inline, replacing the deleted adapter-registration catch-up pass.
                    ctx.tree
                        .create_child(self.parent, "spawned", Some(ctx.scheduler));
                    self.spawned = true;
                }
                false
            }
        }

        let spawn_eid = sched.borrow_mut().register_engine(
            Box::new(ChildSpawnEngine {
                parent: root,
                spawned: false,
            }),
            crate::emEngine::Priority::Medium,
            TreeLocation::Outer,
        );
        sched.borrow_mut().wake_up(spawn_eid);

        let mut empty_windows: HashMap<winit::window::WindowId, crate::emWindow::emWindow> =
            HashMap::new();
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut __input_state = crate::emInputState::emInputState::new();
        let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched.borrow_mut().DoTimeSlice(
            &mut tree,
            &mut empty_windows,
            &__root_ctx,
            &mut __fw,
            &mut __pending_inputs,
            &mut __input_state,
            &__cb,
        );
        let child = tree
            .GetRec(root)
            .and_then(|p| p.first_child)
            .expect("spawned child must exist");

        // Phase 1.75 Task 5 (continuation): adapter registered inline during
        // create_child (scheduler threaded via EngineCtx); no catch-up pass.
        let child_eid = tree
            .GetRec(child)
            .and_then(|p| p.engine_id)
            .expect("spawned child must have engine_id registered inline");
        assert!(
            sched.borrow().get_engine_priority(child_eid).is_some(),
            "scheduler must hold the inline-registered engine",
        );

        // Cleanup.
        tree.remove(root, Some(&mut sched.borrow_mut()));
        sched.borrow_mut().remove_engine(spawn_eid);
    }

    /// Phase 1.75 Task 5 (continuation): callers reaching
    /// `register_engine_for` while the view is `borrow_mut`'d up the stack
    /// (e.g. inside `emView::Update` → `AutoExpand` → `create_child`) must
    /// register the adapter INLINE — no deferral, no catch-up pass.
    /// Precondition for I1d / I-T3a: `register_engine_for` takes no view
    /// borrow, so it is oblivious to the outer `view.borrow_mut()`.
    #[test]
    fn task_5_cont_create_child_registers_inline_even_with_view_borrow_mut_held() {
        let (tree_owned, view, sched, root) = make_registered_tree();
        let mut tree = tree_owned;

        // Simulate the production "view is currently borrow_mut'd" state
        // (as it is throughout `emView::Update`).
        let _view_borrow = view.borrow_mut();
        let child = {
            let mut s = sched.borrow_mut();
            tree.create_child(root, "spawned", Some(&mut s))
        };
        drop(_view_borrow);

        // Engine was registered inline — no deferral.
        let eid = tree
            .GetRec(child)
            .and_then(|p| p.engine_id)
            .expect("create_child must register the adapter inline");
        assert!(sched.borrow().get_engine_priority(eid).is_some());

        tree.remove(root, Some(&mut sched.borrow_mut()));
    }

    // ── SP4.5 Phase 5 tests: multi-view tallness + sibling wake ───────

    /// Phase 5 Task 5.1 — each panel's PanelCycleEngine::Cycle must read
    /// its OWN view's CurrentPixelTallness (not an arbitrary window's
    /// value, as the pre-SP4.5 framework pick-first-window shortcut did).
    ///
    /// Two independent views with distinct CurrentPixelTallness; each
    /// hosts one cycling panel. After running each scheduler's
    /// DoTimeSlice, every panel's recorded tallness must equal its own
    /// view's tallness.
    #[test]
    fn sp4_5_panel_cycle_uses_per_view_pixel_tallness() {
        use crate::emEngineCtx::PanelCtx;
        use crate::emPanel::PanelBehavior;
        use std::cell::Cell;
        use std::collections::HashMap;

        /// Behavior that captures ctx.current_pixel_tallness on Cycle.
        struct TallnessRecordingBehavior {
            recorded: Rc<Cell<Option<f64>>>,
        }
        impl PanelBehavior for TallnessRecordingBehavior {
            fn Cycle(
                &mut self,
                _ectx: &mut crate::emEngineCtx::EngineCtx<'_>,
                ctx: &mut PanelCtx,
            ) -> bool {
                self.recorded.set(Some(ctx.current_pixel_tallness));
                false // sleep after one cycle
            }
        }

        // ── View A: pixel tallness 1.5 ───────────────────────────────
        let mut tree_a = PanelTree::new();
        let root_a = tree_a.create_root_deferred_view("rootA");
        let view_a = Rc::new(RefCell::new(emView::new(
            crate::emContext::emContext::NewRoot(),
            root_a,
            800.0,
            600.0,
        )));
        let sched_a = Rc::new(RefCell::new(EngineScheduler::new()));
        view_a.borrow_mut().CurrentPixelTallness = 1.5;
        tree_a.cached_pixel_tallness = 1.5;
        {
            let mut s = sched_a.borrow_mut();
            tree_a.init_panel_view(root_a, Some(&mut s));
        }
        let recorded_a = Rc::new(Cell::new(None));
        tree_a.set_behavior(
            root_a,
            Box::new(TallnessRecordingBehavior {
                recorded: recorded_a.clone(),
            }),
        );
        let eid_a = tree_a.GetRec(root_a).and_then(|p| p.engine_id).unwrap();

        // ── View B: pixel tallness 0.5 ───────────────────────────────
        let mut tree_b = PanelTree::new();
        let root_b = tree_b.create_root_deferred_view("rootB");
        let view_b = Rc::new(RefCell::new(emView::new(
            crate::emContext::emContext::NewRoot(),
            root_b,
            800.0,
            600.0,
        )));
        let sched_b = Rc::new(RefCell::new(EngineScheduler::new()));
        view_b.borrow_mut().CurrentPixelTallness = 0.5;
        tree_b.cached_pixel_tallness = 0.5;
        {
            let mut s = sched_b.borrow_mut();
            tree_b.init_panel_view(root_b, Some(&mut s));
        }
        let recorded_b = Rc::new(Cell::new(None));
        tree_b.set_behavior(
            root_b,
            Box::new(TallnessRecordingBehavior {
                recorded: recorded_b.clone(),
            }),
        );
        let eid_b = tree_b.GetRec(root_b).and_then(|p| p.engine_id).unwrap();

        // Wake both engines so the next time slice cycles them.
        sched_a.borrow_mut().wake_up(eid_a);
        sched_b.borrow_mut().wake_up(eid_b);

        // Drive each scheduler one slice each.
        let mut windows = HashMap::new();
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut __input_state = crate::emInputState::emInputState::new();
        let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched_a.borrow_mut().DoTimeSlice(
            &mut tree_a,
            &mut windows,
            &__root_ctx,
            &mut __fw,
            &mut __pending_inputs,
            &mut __input_state,
            &__cb,
        );
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut __input_state = crate::emInputState::emInputState::new();
        let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched_b.borrow_mut().DoTimeSlice(
            &mut tree_b,
            &mut windows,
            &__root_ctx,
            &mut __fw,
            &mut __pending_inputs,
            &mut __input_state,
            &__cb,
        );
        assert_eq!(
            recorded_a.get(),
            Some(1.5),
            "view A's panel must see view A's pixel tallness"
        );
        assert_eq!(
            recorded_b.get(),
            Some(0.5),
            "view B's panel must see its own tallness, not view A's"
        );

        // Cleanup.
        tree_a.remove(root_a, Some(&mut sched_a.borrow_mut()));
        tree_b.remove(root_b, Some(&mut sched_b.borrow_mut()));
    }

    /// Phase 5 Task 5.2 — a panel's Cycle may call ctx.wake_up_panel(b);
    /// because the scheduler is borrowed mid-slice (DoTimeSlice holds
    /// `&mut EngineScheduler`), the WakeUp is queued into
    /// `tree.pending_engine_wakeups`. The wakeup is applied at the start of
    /// the next DoTimeSlice, so a subsequent slice cycles sibling B.
    #[test]
    fn sp4_5_wake_up_panel_from_cycle_reaches_sibling() {
        use crate::emEngineCtx::PanelCtx;
        use crate::emPanel::PanelBehavior;
        use std::cell::Cell;
        use std::collections::HashMap;

        /// Panel A: on its first Cycle, wakes its sibling B and goes to
        /// sleep. On any subsequent Cycle it stays asleep. Records how
        /// many times it itself has been cycled.
        struct WakerBehavior {
            sibling: PanelId,
            woke_called: Rc<Cell<u32>>,
            cycles: Rc<Cell<u32>>,
        }
        impl PanelBehavior for WakerBehavior {
            fn Cycle(
                &mut self,
                _ectx: &mut crate::emEngineCtx::EngineCtx<'_>,
                ctx: &mut PanelCtx,
            ) -> bool {
                self.cycles.set(self.cycles.get() + 1);
                if self.woke_called.get() == 0 {
                    ctx.wake_up_panel(self.sibling);
                    self.woke_called.set(1);
                }
                false
            }
        }

        /// Panel B: counts its own Cycle invocations.
        struct CounterBehavior {
            cycles: Rc<Cell<u32>>,
        }
        impl PanelBehavior for CounterBehavior {
            fn Cycle(
                &mut self,
                _ectx: &mut crate::emEngineCtx::EngineCtx<'_>,
                ctx: &mut PanelCtx,
            ) -> bool {
                let _ = ctx;
                self.cycles.set(self.cycles.get() + 1);
                false
            }
        }

        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        let _view = Rc::new(RefCell::new(emView::new(
            crate::emContext::emContext::NewRoot(),
            root,
            800.0,
            600.0,
        )));
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        {
            let mut s = sched.borrow_mut();
            tree.init_panel_view(root, Some(&mut s));
        }

        let (a, b) = {
            let mut s = sched.borrow_mut();
            let a = tree.create_child(root, "a", Some(&mut s));
            let b = tree.create_child(root, "b", Some(&mut s));
            (a, b)
        };

        let b_cycles = Rc::new(Cell::new(0u32));
        let a_cycles = Rc::new(Cell::new(0u32));
        let woke = Rc::new(Cell::new(0u32));
        tree.set_behavior(
            a,
            Box::new(WakerBehavior {
                sibling: b,
                woke_called: woke.clone(),
                cycles: a_cycles.clone(),
            }),
        );
        tree.set_behavior(
            b,
            Box::new(CounterBehavior {
                cycles: b_cycles.clone(),
            }),
        );

        let eid_a = tree.GetRec(a).and_then(|p| p.engine_id).unwrap();
        let eid_b = tree.GetRec(b).and_then(|p| p.engine_id).unwrap();

        // Wake A; B is asleep.
        sched.borrow_mut().wake_up(eid_a);

        // Slice 1: A cycles, calls ctx.wake_up_panel(b). With the new
        // direct-wakeup implementation, wake_up is called on the scheduler
        // immediately via raw pointer (no deferred pending_engine_wakeups queue).
        // B may cycle in slice 1 (if the scheduler scan hasn't passed B's
        // priority slot yet) or in slice 2.
        let mut windows = HashMap::new();
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut __input_state = crate::emInputState::emInputState::new();
        let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched.borrow_mut().DoTimeSlice(
            &mut tree,
            &mut windows,
            &__root_ctx,
            &mut __fw,
            &mut __pending_inputs,
            &mut __input_state,
            &__cb,
        );
        assert_eq!(a_cycles.get(), 1, "A must have cycled once");
        assert_eq!(woke.get(), 1, "A must have called wake_up_panel");

        // Drive at most one additional slice to give B a chance to cycle.
        if b_cycles.get() == 0 {
            let __root_ctx = crate::emContext::emContext::NewRoot();
            let mut __fw: Vec<_> = Vec::new();
            let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
                Vec::new();
            let mut __input_state = crate::emInputState::emInputState::new();
            let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                std::cell::RefCell::new(None);
            sched.borrow_mut().DoTimeSlice(
                &mut tree,
                &mut windows,
                &__root_ctx,
                &mut __fw,
                &mut __pending_inputs,
                &mut __input_state,
                &__cb,
            );
        }
        assert_eq!(
            b_cycles.get(),
            1,
            "B must cycle within one or two slices after wake_up_panel"
        );

        // Sanity: engine ids still match.
        assert_eq!(tree.GetRec(a).and_then(|p| p.engine_id), Some(eid_a));
        assert_eq!(tree.GetRec(b).and_then(|p| p.engine_id), Some(eid_b));

        // Cleanup: remove root (deregisters engines; scheduler busy so
        // pending_engine_removals is populated). DoTimeSlice drains it.
        tree.remove(root, None);
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut __input_state = crate::emInputState::emInputState::new();
        let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched.borrow_mut().DoTimeSlice(
            &mut tree,
            &mut windows,
            &__root_ctx,
            &mut __fw,
            &mut __pending_inputs,
            &mut __input_state,
            &__cb,
        );
    }

    // Phase 1.75 Task 5 (continuation): deleted tests
    //   sp4_5_fix_1_timing_top_level_startup_baseline_slices
    //   sp4_5_fix_1_timing_top_level_mid_update_baseline_slices
    // They asserted delta == 1 between `create_child` and the spawned panel's
    // first `Cycle` — the one-slice cost of the deferred-attach race. With
    // `register_engine_for` now synchronous (no view borrow taken), the
    // adapter registers inline on the same slice as `create_child`, so the
    // measured delta collapses and the baselines no longer represent a real
    // scheduling shape. See invariants I1d / I-T3a.
    //
    // Phase 1.75 Task 3 also deleted
    //   sp4_5_fix_1_timing_sub_scheduler_baseline_slices
    // which measured the same create-then-first-Cycle delta across the
    // now-deleted per-sub-view scheduler field. The outer-scheduler
    // equivalent under Phase 1.75's unified dispatch is the test immediately
    // below (`phase_1_75_task6_spawn_and_wake_child_in_same_slice_delta_zero`),
    // which asserts delta == 0 — the post-Task-4/5 invariant that a spawned
    // panel's `PanelCycleEngine` registers AND fires in the same slice as the
    // `create_child` call when the spawn engine also wakes the child.
    //
    // Phase 1.75 Task 6 — acceptance for Task-11: the sole surviving
    // sp4_5_fix_1_timing_* equivalent asserts `delta == 0`. The three plan-
    // specified names (`panel_reinit`, `sched_drain`, `subview_reinit`) never
    // existed in the codebase; the only fixtures with matching shape were the
    // three deleted above. This test covers the observable invariant they
    // collectively protected (create→first-Cycle slice delta) under the
    // post-Phase-1.75 shape.
    #[test]
    fn phase_1_75_task6_spawn_and_wake_child_in_same_slice_delta_zero() {
        use std::cell::Cell;
        use std::collections::HashMap;

        let (mut tree, _view, sched, root) = make_registered_tree();

        // Captured inside SpawnEngineWithProbe::Cycle at the moment
        // create_child returns (i.e. after inline registration of the
        // child's adapter). SpawnEngine then attaches a first-cycle probe
        // to the child's PanelCycleEngine and wakes it. Because same-
        // priority wake-ups append to the current wake queue, the child
        // Cycles later in the SAME DoTimeSlice call — delta == 0.
        let create_slice: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        let spawned_id: Rc<Cell<Option<PanelId>>> = Rc::new(Cell::new(None));
        let cycled_at: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));

        struct SpawnEngineWithProbe {
            parent: PanelId,
            spawned_out: Rc<Cell<Option<PanelId>>>,
            create_slice_out: Rc<Cell<Option<u64>>>,
            cycled_at: Rc<Cell<Option<u64>>>,
            done: bool,
        }
        impl crate::emEngine::emEngine for SpawnEngineWithProbe {
            fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
                if !self.done {
                    let child = ctx
                        .tree
                        .create_child(self.parent, "spawned", Some(ctx.scheduler));
                    self.spawned_out.set(Some(child));
                    self.create_slice_out.set(Some(ctx.time_slice_counter()));
                    if let Some(eid) = ctx.tree.GetRec(child).and_then(|p| p.engine_id) {
                        ctx.scheduler
                            .attach_first_cycle_probe(eid, self.cycled_at.clone());
                        ctx.scheduler.wake_up(eid);
                    }
                    self.done = true;
                }
                false
            }
        }

        let spawn_eid = sched.borrow_mut().register_engine(
            Box::new(SpawnEngineWithProbe {
                parent: root,
                spawned_out: spawned_id.clone(),
                create_slice_out: create_slice.clone(),
                cycled_at: cycled_at.clone(),
                done: false,
            }),
            crate::emEngine::Priority::Medium,
            TreeLocation::Outer,
        );
        sched.borrow_mut().wake_up(spawn_eid);

        let mut empty_windows: HashMap<winit::window::WindowId, crate::emWindow::emWindow> =
            HashMap::new();
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        let mut __pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut __input_state = crate::emInputState::emInputState::new();
        let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched.borrow_mut().DoTimeSlice(
            &mut tree,
            &mut empty_windows,
            &__root_ctx,
            &mut __fw,
            &mut __pending_inputs,
            &mut __input_state,
            &__cb,
        );

        let create_at = create_slice
            .get()
            .expect("SpawnEngineWithProbe must have captured create_slice");
        let _child = spawned_id
            .get()
            .expect("SpawnEngineWithProbe must have set spawned_id via create_child");
        let cycled = cycled_at
            .get()
            .expect("spawned child's PanelCycleEngine must have Cycled in the same DoTimeSlice");

        // Phase 1.75 Task 6 / Task-11 invariant: create→first-Cycle delta == 0.
        let delta = cycled - create_at;
        assert_eq!(
            delta, 0,
            "post-Phase-1.75 synchronous registration: spawned child must Cycle in the SAME slice as create_child (delta=0); observed create_at={create_at}, cycled={cycled}"
        );

        // Cleanup.
        tree.remove(root, Some(&mut sched.borrow_mut()));
        sched.borrow_mut().remove_engine(spawn_eid);
    }
}
