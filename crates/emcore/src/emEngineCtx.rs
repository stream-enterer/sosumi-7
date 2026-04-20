//! EngineCtx, SchedCtx, InitCtx — event-loop-threaded mutable-state bundles.
//!
//! This module replaces the `Rc`-wrapped scheduler ownership model.
//! See `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.1.

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use crate::emClipboard::emClipboard;
use crate::emColor::emColor;
use crate::emContext::emContext;
use crate::emEngine::{EngineId, Priority, TreeLocation};
use crate::emInput::emInputEvent;
use crate::emInputState::emInputState;
use crate::emPanel::{PanelBehavior, Rect};
use crate::emPanelTree::{PanelId, PanelTree};
use crate::emScheduler::EngineScheduler;
use crate::emSignal::SignalId;
use crate::emWindow::emWindow;

/// Widget callback carried by emCheckButton, emButton, etc. Takes the
/// widget's event payload plus a `SchedCtx` so the callback can fire
/// signals / register engines / access clipboard. Spec §3.5 D6.1.
pub type WidgetCallback<Args> = Box<dyn for<'a, 'b> FnMut(Args, &'b mut SchedCtx<'a>)>;

/// Widget callback taking a borrowed payload (e.g. `&str`, `&DialogResult`,
/// `&[usize]`). Three-lifetime HRTB: `'c` payload, `'b` SchedCtx borrow,
/// `'a` scheduler. `T: ?Sized` supports unsized types like `str` and
/// `[usize]`. Spec §3.5 D6.1.
pub type WidgetCallbackRef<T> = Box<dyn for<'a, 'b, 'c> FnMut(&'c T, &'b mut SchedCtx<'a>)>;

pub enum DeferredAction {
    /// Close a winit window after the current time slice. Drained by the
    /// framework's post-cycle action pump so that window teardown does not
    /// happen inside an engine's Cycle.
    CloseWindow(winit::window::WindowId),
    /// Materialize a popup's winit window after the current time slice.
    /// Popup materialization is deferred to the framework pump (Task 10)
    /// so `emView::RawVisitAbs` can request the popup without owning winit.
    MaterializePopup(winit::window::WindowId),
}

/// Engine-cycle context — the replacement for the legacy `emEngine::EngineCtx`.
///
/// Constructed by `EngineScheduler::DoTimeSlice` immediately before each
/// engine's `Cycle` call. Provides access to the scheduler, panel tree,
/// window registry, root context, and the framework-action drain.
pub struct EngineCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub tree: &'a mut PanelTree,
    pub windows: &'a mut HashMap<winit::window::WindowId, emWindow>,
    pub root_context: &'a Rc<emContext>,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    /// Input-event queue drained by `InputDispatchEngine` (Phase 3,
    /// spec §3.1 / §4 D4.9). Produced by the winit input callback,
    /// consumed once per slice by the top-priority dispatch engine.
    pub pending_inputs: &'a mut Vec<(winit::window::WindowId, emInputEvent)>,
    /// Persistent input state (modifier keys, last mouse pos) maintained
    /// across winit events. Threaded into Cycle so `InputDispatchEngine`
    /// can pass it to `emWindow::dispatch_input` (C++ parity: emInputState
    /// mutation on press/release/move + read during dispatch).
    pub input_state: &'a mut emInputState,
    /// Framework-level clipboard slot (spec §3.1, §3.6(a)). Borrowed from
    /// `emGUIFramework::clipboard`; engines access through `clipboard_mut`.
    /// Phase-3 Task-2 relocation from `emContext::clipboard`.
    pub framework_clipboard: &'a RefCell<Option<Box<dyn emClipboard>>>,
    /// The ID of the engine currently being cycled. Populated by the
    /// scheduler at Cycle-dispatch time.
    pub engine_id: EngineId,
}

pub struct SchedCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
    /// Framework-level clipboard slot (spec §3.1, §3.6(a)). Borrowed from
    /// `emGUIFramework::clipboard`; callers access through `clipboard_mut`.
    /// Phase-3 Task-2 relocation from `emContext::clipboard`.
    pub framework_clipboard: &'a RefCell<Option<Box<dyn emClipboard>>>,
    pub current_engine: Option<EngineId>,
}

/// Construction-only ctx used before the scheduler has started its first
/// time slice. Intentionally trait-only: exposes `ConstructCtx` so engines
/// can be registered and signals created, but does NOT expose
/// fire/connect/remove.
pub struct InitCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
}

pub trait ConstructCtx {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        tree_location: TreeLocation,
    ) -> EngineId;
    fn wake_up(&mut self, eng: EngineId);
}

impl EngineCtx<'_> {
    pub fn framework_action(&mut self, action: DeferredAction) {
        self.framework_actions.push(action);
    }

    /// Take ownership of any pending input events, leaving the buffer empty.
    /// Used by `InputDispatchEngine::Cycle` to drain per slice.
    pub fn take_pending_inputs(&mut self) -> Vec<(winit::window::WindowId, emInputEvent)> {
        std::mem::take(self.pending_inputs)
    }

    pub fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    pub fn fire(&mut self, id: SignalId) {
        self.scheduler.fire(id);
    }

    pub fn remove_signal(&mut self, id: SignalId) {
        self.scheduler.remove_signal(id);
    }

    pub fn wake_up(&mut self, id: EngineId) {
        self.scheduler.wake_up(id);
    }

    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.connect(signal, engine);
    }

    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.disconnect(signal, engine);
    }

    pub fn remove_engine(&mut self, id: EngineId) {
        self.scheduler.remove_engine(id);
    }

    pub fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        tree_location: TreeLocation,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri, tree_location)
    }

    /// Check whether a specific signal has been signaled since the last
    /// time this engine's `Cycle` was called.
    ///
    /// Rust equivalent of C++ `emEngine::IsSignaled()`.
    pub fn IsSignaled(&self, signal: SignalId) -> bool {
        self.scheduler
            .is_signaled_for_engine(signal, self.engine_id)
    }

    /// Forwarded to `EngineScheduler::is_pending`. Used by tests that want
    /// to check whether a signal is merely pending (not yet processed).
    pub fn is_signaled(&self, sig: SignalId) -> bool {
        self.scheduler.is_pending(sig)
    }

    pub fn IsTimeSliceAtEnd(&self) -> bool {
        self.scheduler.IsTimeSliceAtEnd()
    }

    /// Current scheduler time-slice counter. Used by SP4.5-FIX-1 timing
    /// fixtures to measure slices-between-create-and-first-Cycle.
    pub fn time_slice_counter(&self) -> u64 {
        self.scheduler.GetTimeSliceCounter()
    }

    /// The engine whose `Cycle` is currently executing.
    pub fn id(&self) -> EngineId {
        self.engine_id
    }

    pub fn as_sched_ctx(&mut self) -> SchedCtx<'_> {
        SchedCtx {
            scheduler: self.scheduler,
            framework_actions: self.framework_actions,
            root_context: self.root_context,
            framework_clipboard: self.framework_clipboard,
            current_engine: Some(self.engine_id),
        }
    }

    /// Mutable access to the framework-level clipboard slot
    /// (spec §3.1, §3.6(a)).
    pub fn clipboard_mut(&self) -> RefMut<'_, Option<Box<dyn emClipboard>>> {
        self.framework_clipboard.borrow_mut()
    }
}

impl SchedCtx<'_> {
    pub fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    pub fn fire(&mut self, id: SignalId) {
        self.scheduler.fire(id);
    }

    pub fn remove_signal(&mut self, id: SignalId) {
        self.scheduler.remove_signal(id);
    }

    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.connect(signal, engine);
    }

    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.disconnect(signal, engine);
    }

    pub fn remove_engine(&mut self, id: EngineId) {
        self.scheduler.remove_engine(id);
    }

    pub fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        tree_location: TreeLocation,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri, tree_location)
    }

    pub fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }

    pub fn is_signaled(&self, sig: SignalId) -> bool {
        self.scheduler.is_pending(sig)
    }

    pub fn IsSignaled(&self, signal: SignalId) -> bool {
        match self.current_engine {
            Some(eid) => self.scheduler.is_signaled_for_engine(signal, eid),
            None => self.scheduler.is_pending(signal),
        }
    }

    /// Mutable access to the framework-level clipboard slot
    /// (spec §3.1, §3.6(a)).
    pub fn clipboard_mut(&self) -> RefMut<'_, Option<Box<dyn emClipboard>>> {
        self.framework_clipboard.borrow_mut()
    }
}

impl ConstructCtx for EngineCtx<'_> {
    fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        tree_location: TreeLocation,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri, tree_location)
    }

    fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }
}

impl ConstructCtx for SchedCtx<'_> {
    fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        tree_location: TreeLocation,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri, tree_location)
    }

    fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }
}

impl ConstructCtx for InitCtx<'_> {
    fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        tree_location: TreeLocation,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri, tree_location)
    }

    fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }
}

// ───────────────────────────────────────────────────────────────────────────
// PanelCtx — absorbed from emPanelCtx.rs (Phase 1.75 Task 5).
//
// SPLIT: Originally split from emPanel.h into emPanelCtx.rs. Phase 1.75 Task 5
// re-absorbs PanelCtx into emEngineCtx.rs so all ctx-bundle types live together
// (EngineCtx, SchedCtx, InitCtx, PanelCtx) and share the scheduler surface.
// ───────────────────────────────────────────────────────────────────────────

/// Panel context — provides a scoped API for a panel to interact with the tree.
///
/// The pattern is: extract behavior from tree, create PanelCtx, call behavior
/// methods passing ctx, then put behavior back. This avoids borrow conflicts.
pub struct PanelCtx<'a> {
    pub tree: &'a mut PanelTree,
    pub id: PanelId,
    /// Current pixel tallness (height/width ratio of a single pixel) of the
    /// owning view. Mirrors `emView::CurrentPixelTallness`. Passed at ctor
    /// time so layout / viewed-coord computations can use it without needing
    /// a `View&` reference (C++ `emPanel::Layout` reads it via `View&`).
    pub current_pixel_tallness: f64,
    /// Scheduler for engine wakeup. `None` in test-only contexts that do not
    /// need engine wakeup (layout-only tests, etc.).
    pub scheduler: Option<&'a mut EngineScheduler>,
    /// Framework-level clipboard slot (spec §3.1, §3.6(a)). `None` in
    /// layout-only / unit tests that don't need clipboard access. Set by
    /// `PanelCycleEngine` before cycling behaviors so they can build
    /// `SchedCtx`/`EngineCtx` while preserving the clipboard reference.
    pub framework_clipboard: Option<&'a RefCell<Option<Box<dyn emClipboard>>>>,
    /// Deferred-action drain (spec §3.1). `None` in layout-only / unit tests
    /// that do not need to synthesize a `SchedCtx`. Set by `PanelCycleEngine`
    /// (and other full-reach call sites) so behaviors can build a SchedCtx
    /// via `as_sched_ctx()` without losing access to framework actions.
    /// Phase-3 B3.1.
    pub framework_actions: Option<&'a mut Vec<DeferredAction>>,
    /// Root context (spec §3.1). `None` in layout-only / unit tests. Set by
    /// `PanelCycleEngine` and other full-reach call sites so behaviors can
    /// build a `SchedCtx` via `as_sched_ctx()`. Phase-3 B3.1.
    pub root_context: Option<&'a Rc<emContext>>,
}

impl<'a> PanelCtx<'a> {
    /// Create a context for the given panel without a scheduler.
    /// Engine wakeup methods (`wake_up`, `wake_up_panel`) will be no-ops.
    pub fn new(tree: &'a mut PanelTree, id: PanelId, current_pixel_tallness: f64) -> Self {
        Self {
            tree,
            id,
            current_pixel_tallness,
            scheduler: None,
            framework_clipboard: None,
            framework_actions: None,
            root_context: None,
        }
    }

    /// Create a context with a scheduler so engine wakeups are propagated.
    pub fn with_scheduler(
        tree: &'a mut PanelTree,
        id: PanelId,
        current_pixel_tallness: f64,
        scheduler: &'a mut EngineScheduler,
    ) -> Self {
        Self {
            tree,
            id,
            current_pixel_tallness,
            scheduler: Some(scheduler),
            framework_clipboard: None,
            framework_actions: None,
            root_context: None,
        }
    }

    /// Attach the framework-level clipboard slot. Builder-style config per
    /// CLAUDE.md Code Rules (`with_*(self) -> Self`): chain after
    /// `with_scheduler` so behaviors can build `SchedCtx` without losing
    /// clipboard access.
    pub fn with_clipboard(
        mut self,
        framework_clipboard: &'a RefCell<Option<Box<dyn emClipboard>>>,
    ) -> Self {
        self.framework_clipboard = Some(framework_clipboard);
        self
    }

    /// Attach all four scheduler-reach handles at once (scheduler,
    /// framework_actions, root_context, framework_clipboard). Production
    /// call sites that cycle or input-dispatch a panel from inside an
    /// `EngineCtx` context have all four available and should prefer this
    /// over chaining individual builders. Phase-3 B3.1.
    pub fn with_sched_reach(
        tree: &'a mut PanelTree,
        id: PanelId,
        current_pixel_tallness: f64,
        scheduler: &'a mut EngineScheduler,
        framework_actions: &'a mut Vec<DeferredAction>,
        root_context: &'a Rc<emContext>,
        framework_clipboard: &'a RefCell<Option<Box<dyn emClipboard>>>,
    ) -> Self {
        Self {
            tree,
            id,
            current_pixel_tallness,
            scheduler: Some(scheduler),
            framework_clipboard: Some(framework_clipboard),
            framework_actions: Some(framework_actions),
            root_context: Some(root_context),
        }
    }

    /// Synthesize a `SchedCtx` from this `PanelCtx`'s scheduler-reach
    /// handles. Returns `None` if any of the four required handles
    /// (`scheduler`, `framework_actions`, `root_context`,
    /// `framework_clipboard`) is absent. Callers must handle `None`
    /// explicitly — there is no panic landmine. Phase-3 B3.1.
    pub fn as_sched_ctx(&mut self) -> Option<SchedCtx<'_>> {
        let scheduler = self.scheduler.as_deref_mut()?;
        let framework_actions = self.framework_actions.as_deref_mut()?;
        let root_context = self.root_context?;
        let framework_clipboard = self.framework_clipboard?;
        Some(SchedCtx {
            scheduler,
            framework_actions,
            root_context,
            framework_clipboard,
            current_engine: None,
        })
    }

    /// Wake this panel's scheduler engine.
    pub fn wake_up(&mut self) {
        let id = self.id;
        self.wake_up_panel(id);
    }

    /// Wake another panel's scheduler engine.
    /// C++ equivalent: panel->GetView().UpdateEngine->WakeUp().
    pub fn wake_up_panel(&mut self, id: PanelId) {
        let Some(panel) = self.tree.GetRec(id) else {
            return;
        };
        let Some(eid) = panel.engine_id else {
            return;
        };
        if let Some(sched) = self.scheduler.as_deref_mut() {
            sched.wake_up(eid);
        }
    }

    /// Returns true if this panel is the view's current seek target.
    pub fn is_seek_target(&self) -> bool {
        self.tree.is_seek_target(self.id)
    }

    /// Returns the child name being sought from this panel, or "" if not
    /// a seek target. Port of C++ `emPanel::GetSoughtName()`.
    pub fn seek_child_name(&self) -> &str {
        self.tree.sought_name(self.id).unwrap_or("")
    }

    /// Create a child panel under the current panel.
    pub fn create_child(&mut self, name: &str) -> PanelId {
        self.tree
            .create_child(self.id, name, self.scheduler.as_deref_mut())
    }

    /// Create a child with a behavior.
    pub fn create_child_with(&mut self, name: &str, behavior: Box<dyn PanelBehavior>) -> PanelId {
        let child_id = self
            .tree
            .create_child(self.id, name, self.scheduler.as_deref_mut());
        self.tree.set_behavior(child_id, behavior);
        child_id
    }

    /// Remove a child panel.
    pub fn delete_child(&mut self, child: PanelId) {
        // Verify it's actually a child
        if self.tree.GetParentContext(child) == Some(self.id) {
            self.tree.remove(child, self.scheduler.as_deref_mut());
        }
    }

    /// Delete the current panel (removes self from tree).
    pub fn delete_self(mut self) {
        let id = self.id;
        self.tree.remove(id, self.scheduler.as_deref_mut());
    }

    /// Set layout rect for a child panel.
    pub fn layout_child(&mut self, child: PanelId, x: f64, y: f64, w: f64, h: f64) {
        let pt = self.current_pixel_tallness;
        self.tree
            .Layout(child, x, y, w, h, pt, self.scheduler.as_deref_mut());
    }

    /// Set layout rect and canvas color for a child panel.
    ///
    /// C++ equivalent: `child->Layout(x, y, w, h, canvasColor)`.
    /// The canvas_color tells the child what background color it's being
    /// painted on top of, which is needed for correct canvas-color compositing.
    pub fn layout_child_canvas(
        &mut self,
        child: PanelId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        canvas_color: emColor,
    ) {
        let pt = self.current_pixel_tallness;
        self.tree
            .Layout(child, x, y, w, h, pt, self.scheduler.as_deref_mut());
        self.tree
            .SetCanvasColor(child, canvas_color, self.scheduler.as_deref_mut());
    }

    /// Get the parent panel ID.
    pub fn GetParentContext(&self) -> Option<PanelId> {
        self.tree.GetParentContext(self.id)
    }

    /// Iterate over children of the current panel.
    pub fn children(&self) -> Vec<PanelId> {
        self.tree.children(self.id).collect()
    }

    /// Get the name of the current panel.
    pub fn name(&self) -> &str {
        self.tree
            .GetRec(self.id)
            .map(|p| p.name.as_str())
            .unwrap_or("")
    }

    /// Get the layout rect of the current panel in its own coordinate space.
    ///
    /// C++ emPanel behaviors operate in a normalized space where the panel's
    /// own width is always 1.0 and height = LayoutHeight / LayoutWidth
    /// (tallness). All child positions set via `layout_child` must be in
    /// this normalized space.
    pub fn layout_rect(&self) -> Rect {
        self.tree
            .GetRec(self.id)
            .map(|p| {
                let tallness = if p.layout_rect.w > 1e-100 {
                    p.layout_rect.h / p.layout_rect.w
                } else {
                    1.0
                };
                Rect::new(0.0, 0.0, 1.0, tallness)
            })
            .unwrap_or_default()
    }

    /// Set the canvas color.
    pub fn SetCanvasColor(&mut self, color: emColor) {
        self.tree
            .SetCanvasColor(self.id, color, self.scheduler.as_deref_mut());
    }

    /// Get whether the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.tree
            .GetRec(self.id)
            .map(|p| p.visible)
            .unwrap_or(false)
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        if let Some(panel) = self.tree.get_mut(self.id) {
            panel.visible = visible;
        }
    }

    /// Set whether the panel can receive focus.
    pub fn set_focusable(&mut self, focusable: bool) {
        if let Some(panel) = self.tree.get_mut(self.id) {
            panel.focusable = focusable;
        }
    }

    /// Get the preferred size of a child by extracting its behavior, calling
    /// `preferred_size()`, and putting the behavior back.
    pub fn child_preferred_size(&mut self, child: PanelId) -> (f64, f64) {
        if let Some(behavior) = self.tree.take_behavior(child) {
            let size = behavior.preferred_size();
            self.tree.put_behavior(child, behavior);
            size
        } else {
            (0.0, 0.0)
        }
    }

    /// Get the minimum size of a child by extracting its behavior, calling
    /// `min_size()`, and putting the behavior back.
    pub fn child_min_size(&mut self, child: PanelId) -> (f64, f64) {
        if let Some(behavior) = self.tree.take_behavior(child) {
            let size = behavior.min_size();
            self.tree.put_behavior(child, behavior);
            size
        } else {
            (0.0, 0.0)
        }
    }

    /// Remove all children of the current panel.
    pub fn DeleteAllChildren(&mut self) {
        let children: Vec<PanelId> = self.tree.children(self.id).collect();
        for child in children {
            self.tree.remove(child, self.scheduler.as_deref_mut());
        }
    }

    /// Find a child by name.
    pub fn find_child_by_name(&self, name: &str) -> Option<PanelId> {
        self.tree.find_child_by_name(self.id, name)
    }

    /// Request view navigation to a child panel.
    pub fn request_visit(&mut self, child: PanelId) {
        self.tree.request_visit(child);
    }

    /// Get the canvas color of the current panel.
    pub fn GetCanvasColor(&self) -> emColor {
        self.tree
            .GetRec(self.id)
            .map(|p| p.canvas_color)
            .unwrap_or(emColor::TRANSPARENT)
    }

    /// Get whether the panel is enabled.
    pub fn is_enabled(&self) -> bool {
        self.tree
            .GetRec(self.id)
            .map(|p| p.enabled)
            .unwrap_or(false)
    }

    /// Set the enable switch for the current panel.
    pub fn SetEnableSwitch(&mut self, enable: bool) {
        self.tree
            .SetEnableSwitch(self.id, enable, self.scheduler.as_deref_mut());
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.tree.child_count(self.id)
    }

    /// Set canvas color on a child panel.
    ///
    /// C++ equivalent: the canvasColor argument of `child->Layout()`.
    pub fn set_child_canvas_color(&mut self, child: PanelId, color: emColor) {
        self.tree
            .SetCanvasColor(child, color, self.scheduler.as_deref_mut());
    }

    /// Set canvas color on all children of the current panel.
    ///
    /// Used after layout_children to propagate the content area's background
    /// color to all child panels, matching C++ LayoutChildren behavior.
    pub fn set_all_children_canvas_color(&mut self, color: emColor) {
        let children: Vec<PanelId> = self.tree.children(self.id).collect();
        for child in children {
            self.tree
                .SetCanvasColor(child, color, self.scheduler.as_deref_mut());
        }
    }

    /// Port of C++ `emPanel::PanelToViewX(x)`.
    /// Maps panel-space x-coordinate to view (screen) space.
    pub fn panel_to_view_x(&self, x: f64) -> f64 {
        self.tree.PanelToViewX(self.id, x)
    }

    /// Port of C++ `emPanel::PanelToViewY(y)`.
    /// Maps panel-space y-coordinate to view (screen) space.
    /// C++: ViewedY + y * ViewedWidth / CurrentPixelTallness.
    pub fn panel_to_view_y(&self, y: f64) -> f64 {
        if let Some(p) = self.tree.GetRec(self.id) {
            p.viewed_y + y * p.viewed_width / self.current_pixel_tallness
        } else {
            0.0
        }
    }

    /// Port of C++ `emPanel::GetClipX1/X2/Y1/Y2`.
    /// Returns the panel's clip rect in view (screen) space.
    pub fn clip_rect(&self) -> (f64, f64, f64, f64) {
        if let Some(p) = self.tree.GetRec(self.id) {
            (p.clip_x, p.clip_y, p.clip_x + p.clip_w, p.clip_y + p.clip_h)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Make child the first child in sibling order. Port of C++ `BeFirst()`.
    pub fn be_first_child(&mut self, child: PanelId) {
        self.tree.BeFirst(child, self.scheduler.as_deref_mut());
    }

    /// Check if child panel is in the active path.
    pub fn child_in_active_path(&self, child: PanelId) -> bool {
        self.tree
            .GetRec(child)
            .map(|p| p.in_active_path)
            .unwrap_or(false)
    }

    /// Check if child panel is in the viewed path.
    pub fn child_in_viewed_path(&self, child: PanelId) -> bool {
        self.tree
            .GetRec(child)
            .map(|p| p.in_viewed_path)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngine::emEngine;
    use crate::emScheduler::EngineScheduler;

    struct NoopEngine;

    impl emEngine for NoopEngine {
        fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
            false
        }
    }

    #[test]
    fn sched_ctx_exposes_full_api() {
        let mut sched = EngineScheduler::new();
        let mut actions = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            framework_clipboard: &cb,
            current_engine: None,
        };

        assert!(sc.current_engine.is_none());
        assert!(sc.framework_actions.is_empty());
        assert!(Rc::strong_count(sc.root_context) >= 1);

        let sig_a = sc.create_signal();
        let sig_b = sc.create_signal();
        assert_ne!(sig_a, sig_b);

        assert!(!sc.scheduler.is_pending(sig_a));
        sc.fire(sig_a);
        assert!(sc.scheduler.is_pending(sig_a));
        assert!(!sc.scheduler.is_pending(sig_b));

        sc.remove_signal(sig_a);
        sc.fire(sig_a);
        assert!(!sc.scheduler.is_pending(sig_a));
    }

    #[test]
    fn sched_ctx_connect_disconnect_and_engine_lifecycle() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            framework_clipboard: &cb,
            current_engine: None,
        };

        let sig = sc.create_signal();
        let eng = sc.register_engine(Box::new(NoopEngine), Priority::Medium, TreeLocation::Outer);

        sc.connect(sig, eng);
        sc.disconnect(sig, eng);

        sc.wake_up(eng);
        sc.remove_engine(eng);
    }

    #[test]
    fn init_ctx_construct_ctx_trait() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut ic = InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
        };

        assert!(ic.framework_actions.is_empty());
        assert!(Rc::strong_count(ic.root_context) >= 1);

        let sig = <InitCtx as ConstructCtx>::create_signal(&mut ic);
        let eng = <InitCtx as ConstructCtx>::register_engine(
            &mut ic,
            Box::new(NoopEngine),
            Priority::High,
            TreeLocation::Outer,
        );
        <InitCtx as ConstructCtx>::wake_up(&mut ic, eng);

        assert!(!ic.scheduler.is_pending(sig));

        ic.scheduler.remove_engine(eng);
    }

    #[test]
    fn sched_ctx_construct_ctx_trait() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            framework_clipboard: &cb,
            current_engine: None,
        };
        let cc: &mut dyn ConstructCtx = &mut sc;
        let _sig = cc.create_signal();
        let eng = cc.register_engine(
            Box::new(NoopEngine),
            Priority::VeryHigh,
            TreeLocation::Outer,
        );
        cc.wake_up(eng);

        sc.remove_engine(eng);
    }

    #[test]
    fn is_signaled_tracks_fire_and_remove() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            framework_clipboard: &cb,
            current_engine: None,
        };

        let sig = sc.create_signal();
        assert!(!sc.is_signaled(sig));
        sc.fire(sig);
        assert!(sc.is_signaled(sig));
        sc.remove_signal(sig);
        assert!(!sc.is_signaled(sig));
    }

    #[test]
    fn widget_callback_ref_type_check() {
        let _cb: WidgetCallbackRef<str> = Box::new(|_s: &str, _sched: &mut SchedCtx<'_>| {});
        let _cb2: WidgetCallbackRef<[usize]> =
            Box::new(|_s: &[usize], _sched: &mut SchedCtx<'_>| {});
    }

    #[test]
    fn deferred_action_variants_constructible() {
        let wid = winit::window::WindowId::dummy();
        let actions: Vec<DeferredAction> = vec![
            DeferredAction::CloseWindow(wid),
            DeferredAction::MaterializePopup(wid),
        ];
        assert_eq!(actions.len(), 2);
        for a in &actions {
            match a {
                DeferredAction::CloseWindow(_) | DeferredAction::MaterializePopup(_) => {}
            }
        }
    }
}
