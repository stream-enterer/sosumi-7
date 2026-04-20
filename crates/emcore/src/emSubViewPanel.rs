use std::cell::RefCell;
use std::rc::Rc;

use crate::emCursor::emCursor;
use crate::emInput::emInputEvent;
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
use crate::emViewAnimator::emViewAnimator;

use super::emPanel::{NoticeFlags, PanelBehavior, PanelState, ParentInvalidation};
use super::emPanelCtx::PanelCtx;
use super::emPanelTree::{PanelId, PanelTree};
use super::emView::{emView, ViewFlags};

/// A panel that embeds a sub-view within the parent view's panel tree.
///
/// This enables split-view or embedded-view functionality by maintaining a
/// separate [`emView`] and [`PanelTree`] that are rendered within the bounds of
/// this panel. Input is forwarded from the parent to the sub-view, and
/// painting is delegated to the sub-view's own render pipeline.
///
/// Corresponds to C++ `emSubViewPanel`.
pub struct emSubViewPanel {
    sub_tree: PanelTree,
    sub_view: Rc<RefCell<emView>>,
    /// Cached viewed geometry from the parent panel (absolute viewport pixels).
    viewed_x: f64,
    viewed_y: f64,
    viewed_width: f64,
    viewed_height: f64,
    /// C++ emView has ActiveAnimator — an animator that drives zoom/scroll
    /// within this sub-view. Ticked during Paint alongside sub-tree lifecycle.
    pub active_animator: Option<Box<dyn emViewAnimator>>,
    /// DIVERGED: C++ shares the parent emContext's scheduler via context-chain
    /// lookup (emContext::GetScheduler). Rust emSubViewPanel owns a nested
    /// PanelTree and emView; EngineCtx::tree is singular, so a single scheduler
    /// cannot cycle engines across two trees. The forced concession: this
    /// sub-view gets its own EngineScheduler. It is ticked from the outer
    /// PanelCycleEngine (PanelBehavior::Cycle below) once per parent-scheduler
    /// slice, preserving C++ observable cross-frame settlement behavior.
    /// Unrelated to SP7's emContext threading — each emSubViewPanel still owns
    /// its own scheduler because engines cycle against a single PanelTree.
    pub(crate) sub_scheduler: std::rc::Rc<std::cell::RefCell<crate::emScheduler::EngineScheduler>>,
    /// Wall-clock timestamp of previous Cycle, used for active_animator dt.
    last_cycle: Option<std::time::Instant>,
}

impl emSubViewPanel {
    /// Create a new emSubViewPanel with an empty sub-view.
    ///
    /// The sub-tree is initialized with a root panel. Use [`sub_root`],
    /// [`sub_tree_mut`], and [`sub_view_mut`] to populate the sub-view.
    ///
    /// Phase 1.75 Task 3: `outer_panel_id` is the `PanelId` of the outer-tree
    /// panel into which this `emSubViewPanel` will be installed. Caller MUST
    /// `create_child` (or `create_root`) the outer slot first to obtain the
    /// id, then pass it here — the sub_view's engines (`UpdateEngineClass`,
    /// `VisitingVAEngineClass`) and the sub-tree's `PanelCycleEngine` adapters
    /// register with the OUTER scheduler at `TreeLocation::SubView(outer_panel_id,
    /// Outer)`, so dispatch resolves through this panel's `sub_tree` on the
    /// single shared priority queue (spec §3.3). The legacy `sub_scheduler`
    /// field remains (deletion in Task 4) but is created empty — no engines
    /// register on it.
    pub fn new(
        parent_context: Rc<crate::emContext::emContext>,
        outer_panel_id: PanelId,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> Self {
        let mut sub_tree = PanelTree::new();
        // Tag the sub_tree's TreeLocation so `register_engine_for` tags every
        // sub-tree `PanelCycleEngine` adapter with `SubView(outer_panel_id,
        // Outer)` for dispatch through the outer scheduler.
        let sub_location = crate::emEngine::TreeLocation::SubView {
            outer_panel_id,
            rest: Box::new(crate::emEngine::TreeLocation::Outer),
        };
        sub_tree.set_tree_location(sub_location.clone());

        // Deferred-view create: sub_view needs root, root needs view weak.
        // Resolve chicken-and-egg: create root with empty Weak, then wire
        // the view back after construction.
        let root = sub_tree.create_root("", std::rc::Weak::new());
        // Last arg is pixel tallness; sub_view.CurrentPixelTallness starts at 1.0.
        sub_tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let sub_view = Rc::new(RefCell::new(emView::new(parent_context, root, 1.0, 1.0)));
        sub_tree.init_panel_view(root, Rc::downgrade(&sub_view), Some(ctx.scheduler));

        // Phase 1.75 Task 3: register sub_view engines on the OUTER scheduler
        // (via ctx) tagged with SubView location. No engines register on
        // sub_scheduler anymore.
        {
            let mut v = sub_view.borrow_mut();
            v.RegisterEngines(ctx, std::rc::Rc::downgrade(&sub_view), sub_location);
        }

        // Phase 1.75 Task 3 transitional: sub_scheduler still exists (Task 4
        // deletes it) but holds no registered engines. Kept only so the
        // transitional SchedCtxes built inside `Input` / `sync_geometry` /
        // `raw_zoom_out` / `Cycle` still have a scheduler to point at — their
        // `wake_up` calls for outer-registered engines are silent no-ops on
        // this empty scheduler, which is acceptable transitional behavior for
        // the brief lifetime of Task 3 (Task 4 rewires these paths to the
        // outer scheduler and removes the field entirely).
        let sub_scheduler = std::rc::Rc::new(std::cell::RefCell::new(
            crate::emScheduler::EngineScheduler::new(),
        ));

        Self {
            sub_tree,
            sub_view,
            viewed_x: 0.0,
            viewed_y: 0.0,
            viewed_width: 1.0,
            viewed_height: 1.0,
            active_animator: None,
            sub_scheduler,
            last_cycle: None,
        }
    }

    /// Get the root panel ID of the sub-view's panel tree.
    pub fn sub_root(&self) -> PanelId {
        self.sub_tree
            .GetRootPanel()
            .expect("SubViewPanel sub-tree always has a root")
    }

    /// Get a reference to the sub-view's panel tree.
    pub fn sub_tree(&self) -> &PanelTree {
        &self.sub_tree
    }

    /// Get a mutable reference to the sub-view's panel tree.
    pub fn sub_tree_mut(&mut self) -> &mut PanelTree {
        &mut self.sub_tree
    }

    /// Get a reference to the sub-view.
    pub fn GetSubView(&self) -> std::cell::Ref<'_, emView> {
        self.sub_view.borrow()
    }

    /// Get a mutable reference to the sub-view.
    pub fn sub_view_mut(&self) -> std::cell::RefMut<'_, emView> {
        self.sub_view.borrow_mut()
    }

    /// Get the `Rc<RefCell<emView>>` for the sub-view.
    ///
    /// Used by SP5 Task 2.2 to downgrade to `Weak<RefCell<emView>>` for
    /// per-view notice dispatch on emPanel::View.
    pub fn sub_view_rc(&self) -> &Rc<RefCell<emView>> {
        &self.sub_view
    }

    /// Visit a panel in the sub-view by identity string.
    ///
    /// This method borrows `sub_view` and `sub_tree` simultaneously, which
    /// requires access to `self` rather than separate `sub_view_mut()` and
    /// `sub_tree_mut()` calls (which would conflict on `&mut self`).
    pub fn visit_by_identity(
        &mut self,
        identity: &str,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        adherent: bool,
        subject: &str,
    ) {
        self.sub_view
            .borrow_mut()
            .VisitByIdentity(identity, rel_x, rel_y, rel_a, adherent, subject);
    }

    /// Set the view flags on the sub-view.
    pub fn set_sub_view_flags(&mut self, flags: ViewFlags) {
        self.sub_view.borrow_mut().flags = flags;
    }

    /// Call `RawZoomOut` on the sub-view, building a SchedCtx from the
    /// sub-view's own scheduler. Exposed as a public method so external crates
    /// can invoke it without needing direct access to `sub_scheduler`
    /// (`pub(crate)`).
    ///
    /// C++ equivalent: `ContentView.RawZoomOut()` called from
    /// emMainWindow::Cycle (state 10).
    pub fn raw_zoom_out(&mut self, force_viewing_update: bool) {
        let root_ctx = self.sub_view.borrow().GetRootContext();
        let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
        let mut sched = self.sub_scheduler.borrow_mut();
        let mut sc = crate::emEngineCtx::SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            current_engine: None,
        };
        self.sub_view
            .borrow_mut()
            .RawZoomOut(&mut self.sub_tree, force_viewing_update, &mut sc);
    }

    /// Update the sub-view geometry to match the parent panel's viewed area.
    ///
    /// In C++ this delegates to `emViewPort::SetViewGeometry`. The sub-view's
    /// viewport size is set to match the parent panel's pixel dimensions.
    fn sync_geometry(&mut self, state: &PanelState) {
        let (w, h) = if state.viewed {
            self.viewed_x = state.viewed_rect.x;
            self.viewed_y = state.viewed_rect.y;
            self.viewed_width = state.viewed_rect.w;
            self.viewed_height = state.viewed_rect.h;
            (self.viewed_width, self.viewed_height)
        } else {
            // Not viewed — give the sub-view a default geometry.
            // C++ uses (0, 0, 1, GetHeight(), 1.0).
            self.viewed_x = 0.0;
            self.viewed_y = 0.0;
            self.viewed_width = 1.0;
            self.viewed_height = state.height;
            (1.0_f64, state.height)
        };
        // Build a SchedCtx from the sub-view's own scheduler.
        // sub_scheduler is independent of the parent scheduler so borrow_mut
        // does not conflict with the parent DoTimeSlice.
        let root_ctx = self.sub_view.borrow().GetRootContext();
        let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
        let mut sched = self.sub_scheduler.borrow_mut();
        let mut sc = crate::emEngineCtx::SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut fw,
            root_context: &root_ctx,
            current_engine: None,
        };
        self.sub_view
            .borrow_mut()
            .SetGeometry(&mut self.sub_tree, 0.0, 0.0, w, h, 1.0, &mut sc);
    }
}

impl PanelBehavior for emSubViewPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn as_sub_view_panel_mut(&mut self) -> Option<&mut emSubViewPanel> {
        Some(self)
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // C++ emView.cpp:1004 via emSubViewPanel.cpp:77: forward input to
        // the sub-view's ActiveAnimator first. Rust stores the sub-view's
        // animator on emSubViewPanel (not on the sub-view's emView), so this
        // forward happens here.
        let mut event_local = event.clone();
        if let Some(mut anim) = self.active_animator.take() {
            let was_active = anim.is_active();
            emViewAnimator::Input(anim.as_mut(), &mut event_local, input_state);
            let deactivated = was_active && !anim.is_active();
            if anim.is_active() {
                self.active_animator = Some(anim);
            }
            // else: animator self-dropped.
            if deactivated {
                // C++ emViewAnimator.cpp:1060: clear seek-pos so the next notice
                // cycle doesn't fire SOUGHT_NAME_CHANGED on a stale target.
                self.sub_view
                    .borrow_mut()
                    .SetSeekPos(&mut self.sub_tree, None, "");
                // C++ emViewAnimator.cpp:1061: whole-view InvalidatePainting() skipped.
                // Rust has no emViewAnimator::InvalidatePainting method; the visiting
                // overlay will repaint correctly on the next scheduled paint cycle.
            }
        }
        let event = &event_local;

        // C++ emSubViewPanel::Input:
        //   if (IsFocusable() && (event.IsMouseEvent() || event.IsTouchEvent())) {
        //       Focus();
        //       SubViewPort->SetViewFocused(IsFocused());
        //   }
        //   SubViewPort->InputToView(event, state);
        //
        // Focus() on mouse/touch is already handled by the parent window's
        // dispatch loop (zui_window.rs hit-test + set_active_panel). We still
        // propagate focus state to the sub-view here, matching C++.
        if event.is_mouse_event() || event.is_touch_event() {
            self.sub_view
                .borrow_mut()
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }

        // Forward input to the sub-view's panels (C++ InputToView).
        // The event mouse coords are in panel-local space (x: 0..1, y: 0..h).
        // Convert to sub-view viewport pixel coords for the sub-tree dispatch.
        let sub_vx = event.mouse_x * self.viewed_width;
        let sub_vy = event.mouse_y * self.viewed_width / state.pixel_tallness;

        // Build a SchedCtx from the sub-view's own scheduler for method calls below.
        let root_ctx_for_input = self.sub_view.borrow().GetRootContext();
        let mut fw_input: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();

        // Hit-test and set active panel on mouse press (mirrors parent window logic).
        if event.is_mouse_event() && event.variant == crate::emInput::InputVariant::Press {
            let panel = self
                .sub_view
                .borrow()
                .GetFocusablePanelAt(&self.sub_tree, sub_vx, sub_vy)
                .unwrap_or_else(|| self.sub_view.borrow().GetRootPanel());
            let mut sched = self.sub_scheduler.borrow_mut();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut sched,
                framework_actions: &mut fw_input,
                root_context: &root_ctx_for_input,
                current_engine: None,
            };
            self.sub_view
                .borrow_mut()
                .set_active_panel(&mut self.sub_tree, panel, false, &mut sc);
        }

        // Ensure sub-view viewing state is current for coordinate transforms.
        {
            let mut sched = self.sub_scheduler.borrow_mut();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut sched,
                framework_actions: &mut fw_input,
                root_context: &root_ctx_for_input,
                current_engine: None,
            };
            self.sub_view
                .borrow_mut()
                .Update(&mut self.sub_tree, &mut sc);
        }

        // Dispatch to sub-tree panels (DFS order, matching C++ RecurseInput).
        let wf = self.sub_view.borrow().IsFocused();
        let pixel_tallness = self.sub_view.borrow().GetCurrentPixelTallness();
        let viewed = self.sub_tree.viewed_panels_dfs();
        for panel_id in viewed {
            let mut panel_ev = event.clone();
            panel_ev.mouse_x = self.sub_tree.ViewToPanelX(panel_id, sub_vx);
            panel_ev.mouse_y = self.sub_tree.ViewToPanelY(panel_id, sub_vy, pixel_tallness);
            if let Some(mut behavior) = self.sub_tree.take_behavior(panel_id) {
                let panel_state = self
                    .sub_tree
                    .build_panel_state(panel_id, wf, pixel_tallness);
                // Suppress keyboard events for panels not in the active path.
                if panel_ev.is_keyboard_event() && !panel_state.in_active_path {
                    self.sub_tree.put_behavior(panel_id, behavior);
                    continue;
                }
                let consumed = behavior.Input(&panel_ev, &panel_state, input_state);
                self.sub_tree.put_behavior(panel_id, behavior);
                if consumed {
                    self.sub_view
                        .borrow_mut()
                        .InvalidatePainting(&self.sub_tree, panel_id);
                    return true;
                }
            }
        }

        false
    }

    fn Cycle(
        &mut self,
        _ectx: &mut crate::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // Wall-clock dt for the active_animator tick (matching
        // VisitingVAEngineClass pattern).
        let now = std::time::Instant::now();
        let dt = self
            .last_cycle
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.016)
            .clamp(0.001, 0.1);
        self.last_cycle = Some(now);

        // 1) Tick the ActiveAnimator (C++ emView::ActiveAnimator; Rust stores
        //    on emSubViewPanel per SP1 §5.1 item 3). Take/put preserves Rust
        //    borrow rules.
        let animator_active = if let Some(mut anim) = self.active_animator.take() {
            let root_ctx_for_anim = self.sub_view.borrow().GetRootContext();
            let mut fw_anim: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let mut sched_anim = self.sub_scheduler.borrow_mut();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut sched_anim,
                framework_actions: &mut fw_anim,
                root_context: &root_ctx_for_anim,
                current_engine: None,
            };
            let still_active = anim.animate(
                &mut self.sub_view.borrow_mut(),
                &mut self.sub_tree,
                dt,
                &mut sc,
            );
            drop(sched_anim);
            if still_active {
                self.active_animator = Some(anim);
            }
            still_active
        } else {
            false
        };

        // 2) Drive one sub-scheduler slice. sub-view engines never access
        //    ctx.windows (view-direct after Phase 1), so an empty window map
        //    is safe.
        let mut empty_windows: std::collections::HashMap<
            winit::window::WindowId,
            std::rc::Rc<std::cell::RefCell<crate::emWindow::emWindow>>,
        > = std::collections::HashMap::new();
        let __root_ctx = crate::emContext::emContext::NewRoot();
        let mut __fw: Vec<_> = Vec::new();
        self.sub_scheduler.borrow_mut().DoTimeSlice(
            &mut self.sub_tree,
            &mut empty_windows,
            &__root_ctx,
            &mut __fw,
        );
        // SP4.5 fix: register any sub-tree panels created via `create_child`
        // from inside a sub-scheduler engine's `Cycle`. Their
        // `register_engine_for` deferred while the sub-scheduler was
        // `borrow_mut`'d by `DoTimeSlice`; now it's released.
        self.sub_tree
            .register_pending_engines(&mut self.sub_scheduler.borrow_mut());

        // 3) Stay awake iff the sub-scheduler or active_animator still has work.
        animator_active || self.sub_scheduler.borrow().has_awake_engines()
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        // C++ NF_FOCUS_CHANGED → SetViewFocused(IsFocused())
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.sub_view
                .borrow_mut()
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }
        // C++ NF_VIEWING_CHANGED → SetViewGeometry(...)
        if flags.intersects(NoticeFlags::VIEWING_CHANGED | NoticeFlags::LAYOUT_CHANGED) {
            self.sync_geometry(state);
        }
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, state: &PanelState) {
        if !state.viewed {
            return;
        }
        // C++ emSubViewPanel::Paint (src/emCore/emSubViewPanel.cpp:94) just
        // delegates to SubViewPort->PaintView. No settlement inside Paint —
        // sub-view settlement happens across frames via sub_scheduler, driven
        // from PanelBehavior::Cycle above.
        let base_offset = painter.origin();
        let bg = self.sub_view.borrow().GetBackgroundColor();
        let root = self.sub_root();
        self.sub_view.borrow_mut().paint_sub_tree(
            &mut self.sub_tree,
            painter,
            root,
            base_offset,
            bg,
        );
    }

    fn GetCursor(&self) -> emCursor {
        self.sub_view.borrow().GetCursor()
    }

    fn get_title(&self) -> Option<String> {
        // C++ delegates to SubView->GetTitle(), which walks the sub-view's
        // panel tree for a title.
        let root = self.sub_root();
        Some(self.sub_tree.get_title(root))
    }

    fn drain_parent_invalidation(&mut self) -> Option<ParentInvalidation> {
        let title = self.sub_view.borrow().is_title_invalid();
        let cursor = self.sub_view.borrow().is_cursor_invalid();
        let has_dirty = self.sub_view.borrow().has_dirty_rects();

        if !title && !cursor && !has_dirty {
            return None;
        }

        if title {
            self.sub_view.borrow_mut().clear_title_invalid();
        }
        if cursor {
            self.sub_view.borrow_mut().clear_cursor_invalid();
        }

        // C++ SubViewPortClass::InvalidatePainting calls
        // SuperPanel.InvalidatePaintingOnView(x,y,w,h) which forwards
        // directly to GetView().InvalidatePainting(x,y,w,h). The dirty
        // rects are already in absolute view (pixel) coordinates, so we
        // pass them through unchanged.
        let dirty_rects = if has_dirty {
            self.sub_view.borrow_mut().take_dirty_rects()
        } else {
            Vec::new()
        };

        Some(ParentInvalidation {
            dirty_rects,
            title_invalid: title,
            cursor_invalid: cursor,
        })
    }
}

#[cfg(test)]
mod sp8_tests {
    use super::*;

    struct NoopEngine;
    impl crate::emEngine::emEngine for NoopEngine {
        fn Cycle(&mut self, _ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
            false
        }
    }

    /// Phase 1.75 Task 3 harness: a bare outer tree + outer scheduler + a
    /// child panel id, plus a constructed `emSubViewPanel` whose engines live
    /// on the outer scheduler at `SubView(owner_id, Outer)`. Test-only helper;
    /// Task 4 simplifies this further when `sub_scheduler` goes away.
    struct SvpTestHarness {
        outer_tree: crate::emPanelTree::PanelTree,
        outer_sched: crate::emScheduler::EngineScheduler,
        owner_id: crate::emPanelTree::PanelId,
        panel: emSubViewPanel,
    }

    impl SvpTestHarness {
        fn new() -> Self {
            let mut outer_tree = crate::emPanelTree::PanelTree::new();
            let _root = outer_tree.create_root("owner_root", std::rc::Weak::new());
            let owner_id =
                outer_tree.create_child(outer_tree.GetRootPanel().unwrap(), "owner_sv", None);
            let mut outer_sched = crate::emScheduler::EngineScheduler::new();
            let root_ctx = crate::emContext::emContext::NewRoot();
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let panel = {
                let mut sc = crate::emEngineCtx::SchedCtx {
                    scheduler: &mut outer_sched,
                    framework_actions: &mut fw,
                    root_context: &root_ctx,
                    current_engine: None,
                };
                emSubViewPanel::new(root_ctx.clone(), owner_id, &mut sc)
            };
            Self {
                outer_tree,
                outer_sched,
                owner_id,
                panel,
            }
        }

        /// Deregister sub-view engines from the outer scheduler + remove
        /// sub-tree panels so the outer scheduler's Drop debug_assert passes.
        fn teardown(mut self) {
            let sub_root = self.panel.sub_root();
            self.panel
                .sub_tree
                .remove(sub_root, Some(&mut self.outer_sched));
            let mut view = self.panel.sub_view.borrow_mut();
            if let Some(eid) = view.update_engine_id.take() {
                self.outer_sched.remove_engine(eid);
            }
            if let Some(eid) = view.visiting_va_engine_id.take() {
                self.outer_sched.remove_engine(eid);
            }
            if let Some(sig) = view.EOISignal.take() {
                self.outer_sched.remove_signal(sig);
            }
            drop(view);
            // The panel's sub_scheduler is empty (Task 3 moved all engines to
            // outer); it drops cleanly via its own Drop.
            let _ = self.panel;
            // Owner panel's adapter engine lives on outer_sched — drop via
            // outer_tree.remove.
            let owner = self.owner_id;
            self.outer_tree.remove(owner, Some(&mut self.outer_sched));
        }
    }

    #[test]
    fn sp8_sub_view_update_engine_registered() {
        let h = SvpTestHarness::new();
        {
            let sub_view = h.panel.GetSubView();
            assert!(
                sub_view.update_engine_id.is_some(),
                "sub_view must have update engine registered in new()"
            );
        }
        h.teardown();
    }

    #[test]
    fn sp8_sub_tree_root_panel_engine_registered() {
        let h = SvpTestHarness::new();
        let sub_root = h.panel.sub_root();
        let engine_id = h.panel.sub_tree().panel_engine_id(sub_root);
        assert!(
            engine_id.is_some(),
            "sub-tree root panel must have PanelCycleEngine registered on outer scheduler"
        );
        // Phase 1.75 Task 3: after RegisterEngines the UpdateEngineClass is
        // woken on the OUTER scheduler (not sub_scheduler).
        assert!(
            h.outer_sched.has_awake_engines(),
            "outer scheduler must have awake engines after sub-view construction"
        );
        assert!(h.panel.last_cycle.is_none());
        h.teardown();
    }

    #[test]
    fn sp8_cycle_drives_sub_scheduler() {
        // Phase 1.75 Task 3: sub_scheduler is empty; `emSubViewPanel::Cycle`
        // still drives it transitionally (Task 4 strips this), but it runs
        // no engines. The cycle should still call without panic and return
        // `animator_active || has_awake_engines()` — with no animator and
        // empty sub_scheduler, that's `false`.
        let mut h = SvpTestHarness::new();

        // Drive Cycle via a fake PanelCtx — construct a throwaway owner tree
        // and id. We don't care about the PanelCtx internals, only that Cycle
        // executes.
        let mut owner_tree = crate::emPanelTree::PanelTree::new();
        let owner_id = owner_tree.create_root("owner", std::rc::Weak::new());
        let mut pctx = crate::emPanelCtx::PanelCtx::new(&mut owner_tree, owner_id, 1.0);

        // Harness supplies the EngineCtx scaffolding for the Cycle call.
        let mut th = crate::test_view_harness::TestViewHarness::new();
        let dummy_eid = th.scheduler.register_engine(
            Box::new(NoopEngine),
            crate::emEngine::Priority::Medium,
            crate::emEngine::TreeLocation::Outer,
        );

        let stay_awake = {
            let mut ectx = th.engine_ctx(dummy_eid);
            <emSubViewPanel as PanelBehavior>::Cycle(&mut h.panel, &mut ectx, &mut pctx)
        };
        assert!(
            !stay_awake,
            "Cycle must return false when sub_scheduler is empty and no animator"
        );
        th.scheduler.remove_engine(dummy_eid);
        h.teardown();
    }
}

// Phase 1.75 Task 3: the former `sp4_5_fix_1_tests::sp4_5_fix_1_timing_sub_scheduler_baseline_slices`
// fixture measured slice delta through the transitional `sub_scheduler`
// catch-up path inside `emSubViewPanel::Cycle`. With sub-view + sub-tree
// engines now registered on the OUTER scheduler with
// `TreeLocation::SubView(owner, Outer)`, the sub_scheduler is empty by
// construction and that fixture tested a path that no longer exists. Phase
// 1.75 Task 6 is scheduled to rewrite this against the unified outer
// scheduler (asserting `delta == 0`). Until Task 6 runs, nextest count
// drops by 1 (baseline 2456 → 2455); the other sp4_5_fix_1_timing_*
// fixtures in `crates/emcore/tests/` continue to pass.
