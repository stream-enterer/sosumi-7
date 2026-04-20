use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::emContext::emContext;
use crate::emEngineCtx::DeferredAction as FrameworkDeferredAction;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPanelTree::PanelTree;
use crate::emScheduler::EngineScheduler;
use crate::emSignal::SignalId;

use super::emWindow::{emWindow, WindowFlags};
use crate::emScreen::emScreen;

/// Shared GPU resources created once and used by all windows.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Default for GpuContext {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuContext {
    /// Create GPU context synchronously using pollster.
    pub fn new() -> Self {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("eaglemode_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .expect("failed to create GPU device");

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
}

/// User-provided setup callback, called once during `resumed()`.
pub type SetupFn = Box<dyn FnOnce(&mut App, &ActiveEventLoop)>;

/// The main application handler integrating winit, wgpu, the panel tree, and
/// the scheduler.
/// Deferred action requiring `&ActiveEventLoop` (window creation, etc.).
pub type DeferredAction = Box<dyn FnOnce(&mut App, &ActiveEventLoop)>;

pub struct App {
    pub gpu: Option<GpuContext>,
    pub screen: Option<emScreen>,
    pub scheduler: EngineScheduler,
    pub context: Rc<emContext>,
    pub tree: PanelTree,
    // NOTE (Phase 1 Task 2): plan calls for `HashMap<WindowId, emWindow>` (plain
    // value) but narrowing the wrapper cascades into dozens of call sites across
    // emWindow / materialize_popup_surface / view wiring. Deferred to a later task.
    pub windows: HashMap<WindowId, Rc<RefCell<emWindow>>>,
    /// Framework-level deferred actions produced by scheduler/view code that
    /// need to run back on `App` between time slices. Spec §3.1 / §3.7 —
    /// passed as `&mut Vec<DeferredAction>` into `EngineScheduler::DoTimeSlice`.
    pub(crate) framework_actions: Vec<FrameworkDeferredAction>,
    pub input_state: emInputState,
    /// Input queue drained by `InputDispatchEngine` (Phase 3) per spec §3.1
    /// and §4 D4.9. Produced by `window_event` on each winit input;
    /// consumed once per slice. Restored in Phase 1.5 Task 1 step 1g after
    /// being speculatively deleted by Chunk 2 (W2 drift).
    ///
    /// NOTE: unused at end of Phase 1.5; `dead_code` warning is spec-mandated
    /// carry-forward — see `2026-04-19-phase-1-5-ledger.md`. Phase 3's
    /// `InputDispatchEngine` consumes this; the warning disappears when it
    /// lands.
    pub(crate) _pending_inputs: Vec<(WindowId, emInputEvent)>,
    /// Deferred actions queued by input handlers that need `&ActiveEventLoop`
    /// (e.g., window creation for Duplicate/CreateControlWindow, popup
    /// surface materialization from `emView::RawVisitAbs`).
    /// Drained each frame in `about_to_wait`.
    ///
    /// `Rc<RefCell<...>>` so `emView` can hold a handle and enqueue without
    /// a borrow of `App`.
    pub pending_actions: Rc<RefCell<Vec<DeferredAction>>>,
    /// Global file-update signal. Port of C++ `emFileModel::AcquireUpdateSignalModel`.
    /// When fired, all file models that listen to it will reload from disk.
    pub file_update_signal: SignalId,
    setup_fn: Option<SetupFn>,
    initialized: bool,
    last_frame_time: Instant,
}

impl App {
    pub fn new(setup: SetupFn) -> Self {
        let mut scheduler = EngineScheduler::new();
        let file_update_signal = scheduler.create_signal();
        let context = emContext::NewRoot();
        Self {
            gpu: None,
            screen: None,
            scheduler,
            context,
            tree: PanelTree::new(),
            windows: HashMap::new(),
            framework_actions: Vec::new(),
            input_state: emInputState::new(),
            _pending_inputs: Vec::new(),
            pending_actions: Rc::new(RefCell::new(Vec::new())),
            file_update_signal,
            setup_fn: Some(setup),
            initialized: false,
            last_frame_time: Instant::now(),
        }
    }

    /// Run the application. This blocks until all windows are closed.
    pub fn run(self) {
        let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        let mut app = self;
        event_loop.run_app(&mut app).expect("event loop error");
        // Work around wgpu segfault on shutdown: dropping Instance/Device
        // after Surface is already destroyed crashes in the Vulkan driver.
        // https://github.com/gfx-rs/wgpu/issues/5781
        std::process::exit(0);
    }

    /// Get the GPU context (panics if not yet initialized).
    pub fn gpu(&self) -> &GpuContext {
        self.gpu.as_ref().expect("GPU not initialized yet")
    }

    pub fn screen(&self) -> &emScreen {
        self.screen.as_ref().expect("Screen not initialized yet")
    }

    /// Dispatch synthetic input events from the touch gesture machine.
    /// Modifier keys are set/cleared on input_state to match C++ InputState
    /// persistence: press events set modifiers, release events clear them.
    fn dispatch_forward_events(
        win: &mut emWindow,
        tree: &mut PanelTree,
        input_state: &mut emInputState,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        let forward_events = win.touch_vif_mut().drain_forward_events();
        if forward_events.is_empty() {
            return;
        }
        for event in &forward_events {
            // C++ parity: modifiers are SET on press and CLEARED on release.
            // They persist across frames so real events also see them.
            match event.variant {
                InputVariant::Press => {
                    if event.shift {
                        input_state.press(InputKey::Shift);
                    }
                    if event.ctrl {
                        input_state.press(InputKey::Ctrl);
                    }
                }
                InputVariant::Release => {
                    if event.shift {
                        input_state.release(InputKey::Shift);
                    }
                    if event.ctrl {
                        input_state.release(InputKey::Ctrl);
                    }
                }
                _ => {}
            }
            input_state.set_mouse(event.mouse_x, event.mouse_y);
            let mut ev = event.clone();
            ev.mouse_x = input_state.mouse_x;
            ev.mouse_y = input_state.mouse_y;
            win.dispatch_input(tree, &ev, input_state, ctx);
        }
        win.invalidate();
        win.request_redraw();
    }

    /// Materialize a popup window's OS surface, transitioning it from
    /// `OsSurface::Pending` to `OsSurface::Materialized`. Called from the
    /// `pending_actions` drain in `about_to_wait` where `&ActiveEventLoop`
    /// is available.
    ///
    /// **Cancellation:** if the popup was already dropped from
    /// `emView::PopupWindow` before this method runs (e.g. a popup-exit
    /// happened in the same frame as popup-entry), `win_rc` is the only
    /// remaining strong reference and the materialization is skipped.
    /// The Rc drops at function end; no winit window is created.
    pub(crate) fn materialize_popup_surface(
        &mut self,
        win_rc: Rc<RefCell<emWindow>>,
        event_loop: &ActiveEventLoop,
    ) {
        use crate::emWindow::{MaterializedSurface, OsSurface};

        // Cancellation check: if we're the only strong ref, the popup was
        // dropped before materialization. Abort silently.
        if Rc::strong_count(&win_rc) == 1 {
            return;
        }

        // Extract Pending params.
        let (flags, caption, requested_pos_size) = {
            let w = win_rc.borrow();
            match &w.os_surface {
                OsSurface::Pending(p) => (p.flags, p.caption.clone(), p.requested_pos_size),
                OsSurface::Materialized(_) => {
                    log::warn!("materialize_popup_surface called on already-materialized window");
                    return;
                }
            }
        };

        // Build winit window attributes — mirror `emWindow::create`.
        let mut attrs = winit::window::WindowAttributes::default().with_title(caption.as_str());
        if flags.contains(WindowFlags::UNDECORATED) {
            attrs = attrs.with_decorations(false);
        }
        if flags.contains(WindowFlags::POPUP) {
            attrs = attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }
        if flags.contains(WindowFlags::MAXIMIZED) {
            attrs = attrs.with_maximized(true);
        }
        if flags.contains(WindowFlags::FULLSCREEN) {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        let winit_window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create popup window"),
        );

        if let Some((x, y, pw, ph)) = requested_pos_size {
            let _ = winit_window
                .request_inner_size(winit::dpi::PhysicalSize::new(pw as u32, ph as u32));
            winit_window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
        }

        let gpu = self.gpu.as_ref().expect("GPU not initialized");
        let materialized = MaterializedSurface::build(gpu, winit_window.clone());
        let w = materialized.surface_config.width;
        let h = materialized.surface_config.height;

        {
            let mut w_mut = win_rc.borrow_mut();
            w_mut.os_surface = OsSurface::Materialized(Box::new(materialized));
            let root = self.context.clone();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut self.scheduler,
                framework_actions: &mut self.framework_actions,
                root_context: &root,
                current_engine: None,
            };
            w_mut
                .view_mut()
                .SetGeometry(&mut self.tree, 0.0, 0.0, w as f64, h as f64, 1.0, &mut sc);
        }

        let window_id = winit_window.id();
        self.windows.insert(window_id, win_rc.clone());
        winit_window.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        // Init GPU
        self.gpu = Some(GpuContext::new());

        // Scan monitors — allocate signal IDs for geometry/window-list changes.
        let geom_sig = self.scheduler.create_signal();
        let win_sig = self.scheduler.create_signal();
        self.screen = Some(emScreen::from_event_loop(event_loop, geom_sig, win_sig));

        // Call user setup
        if let Some(setup) = self.setup_fn.take() {
            setup(self, event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                let auto_delete = self
                    .windows
                    .get(&window_id)
                    .map(|rc| rc.borrow().flags.contains(WindowFlags::AUTO_DELETE))
                    .unwrap_or(true);

                if let Some(rc) = self.windows.get(&window_id) {
                    let sig = rc.borrow().close_signal;
                    self.scheduler.fire(sig);
                }

                if auto_delete {
                    self.windows.remove(&window_id);
                }

                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(rc) = self.windows.get(&window_id).cloned() {
                    let gpu = self.gpu.as_ref().unwrap();
                    let root = self.context.clone();
                    let mut sc = crate::emEngineCtx::SchedCtx {
                        scheduler: &mut self.scheduler,
                        framework_actions: &mut self.framework_actions,
                        root_context: &root,
                        current_engine: None,
                    };
                    let mut win = rc.borrow_mut();
                    win.resize(gpu, &mut self.tree, size.width, size.height, &mut sc);
                    win.set_geometry_changed();
                    // Don't request_redraw here — about_to_wait will detect the
                    // layout change from the new tallness and issue a single
                    // repaint after layout is settled.
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(rc) = self.windows.get(&window_id) {
                    rc.borrow_mut().set_geometry_changed();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(rc) = self.windows.get(&window_id) {
                    let mut win = rc.borrow_mut();
                    let gpu = self.gpu.as_ref().unwrap();
                    win.render(&mut self.tree, gpu);
                }
            }
            WindowEvent::Focused(focused) => {
                if let Some(rc) = self.windows.get(&window_id) {
                    let mut win = rc.borrow_mut();
                    win.view_mut().SetFocused(&mut self.tree, focused);
                    win.set_focus_changed();
                    win.invalidate();
                    win.request_redraw();
                }
            }
            WindowEvent::Touch(ref touch) => {
                if let Some(rc) = self.windows.get(&window_id).cloned() {
                    let mut win = rc.borrow_mut();
                    let mut sc = crate::emEngineCtx::SchedCtx {
                        scheduler: &mut self.scheduler,
                        framework_actions: &mut self.framework_actions,
                        root_context: &self.context,
                        current_engine: None,
                    };
                    win.handle_touch(touch, &mut self.tree, &mut sc);
                    Self::dispatch_forward_events(
                        &mut win,
                        &mut self.tree,
                        &mut self.input_state,
                        &mut sc,
                    );
                    win.invalidate();
                    win.request_redraw();
                }
            }
            ref input_event => {
                if let Some(mut input) = emWindow::handle_input(input_event) {
                    // Update persistent input state
                    match input.variant {
                        InputVariant::Press => self.input_state.press(input.key),
                        InputVariant::Release => self.input_state.release(input.key),
                        InputVariant::Move => {
                            self.input_state.set_mouse(input.mouse_x, input.mouse_y);
                        }
                        _ => {}
                    }

                    // Always populate current mouse position on events,
                    // except for wheel events which encode scroll delta in mouse_x/y.
                    if !matches!(
                        input.key,
                        InputKey::WheelUp
                            | InputKey::WheelDown
                            | InputKey::WheelLeft
                            | InputKey::WheelRight
                    ) {
                        input.mouse_x = self.input_state.mouse_x;
                        input.mouse_y = self.input_state.mouse_y;
                    }

                    if let Some(rc) = self.windows.get(&window_id).cloned() {
                        let mut sc = crate::emEngineCtx::SchedCtx {
                            scheduler: &mut self.scheduler,
                            framework_actions: &mut self.framework_actions,
                            root_context: &self.context,
                            current_engine: None,
                        };
                        rc.borrow_mut().dispatch_input(
                            &mut self.tree,
                            &input,
                            &mut self.input_state,
                            &mut sc,
                        );
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Tick screensaver keepalive (pokes xscreensaver every 59s when inhibited).
        super::emWindowPlatform::tick_screensaver_keepalive();

        // Lazy-wire each view's pending_framework_actions handle so that
        // popup-creation paths in `emView::RawVisitAbs` can enqueue back into
        // `App::pending_actions`. Guarded by is_none() — one-shot init per view.
        for rc in self.windows.values() {
            let mut win = rc.borrow_mut();
            if win.view().pending_framework_actions.is_none() {
                win.view_mut()
                    .set_pending_framework_actions(self.pending_actions.clone());
            }
        }

        // Process deferred actions (window creation from Duplicate/ccw,
        // popup surface materialization, etc.). Drain by move so that
        // closures own their captured `Rc<RefCell<emWindow>>`; this is
        // required by `materialize_popup_surface`'s cancellation check
        // (`Rc::strong_count(&win_rc) == 1`).
        let actions: Vec<DeferredAction> = self.pending_actions.borrow_mut().drain(..).collect();
        for action in actions {
            action(self, event_loop);
        }

        // Fire signals for any windows whose state changed this frame.
        let changed_signals: Vec<_> = self
            .windows
            .values()
            .flat_map(|rc| {
                let mut win = rc.borrow_mut();
                let mut sigs = Vec::new();
                if win.flags_changed() {
                    win.clear_flags_changed();
                    sigs.push(win.flags_signal);
                }
                if win.focus_changed() {
                    win.clear_focus_changed();
                    sigs.push(win.focus_signal);
                }
                if win.geometry_changed() {
                    win.clear_geometry_changed();
                    sigs.push(win.geometry_signal);
                }
                sigs
            })
            .collect();
        for sig in changed_signals {
            self.scheduler.fire(sig);
        }

        // Run one scheduler time slice. `framework_actions` is now owned by
        // `App` per spec §3.1 and passed through as a `&mut Vec<DeferredAction>`
        // parameter — engines push via `EngineCtx::framework_action` and the
        // framework consumes here between slices.
        //
        // Disjoint-field borrow: destructure `self` so scheduler, tree, windows,
        // context, and framework_actions can all be borrowed simultaneously.
        {
            let App {
                ref mut scheduler,
                ref mut tree,
                ref mut windows,
                ref context,
                ref mut framework_actions,
                ..
            } = *self;
            scheduler.DoTimeSlice(tree, windows, context, framework_actions);
        }

        // SP4.5 fix: register any panels created via `create_child` from
        // inside an engine's `Cycle` (e.g. `StartupEngine`). Their
        // `register_engine_for` call deferred when it found the scheduler
        // already `borrow_mut`'d by `DoTimeSlice`. Now that the borrow has
        // been released, walk the tree and register pending engines.
        self.tree.register_pending_engines();

        // Keep event loop pumping while engines are active.
        // C++ runs a tight 10ms loop; Rust uses event-driven winit with
        // ControlFlow::Wait which only fires about_to_wait on OS events.
        // Requesting redraws ensures continuous cycling during startup,
        // animations, and any other engine activity.
        if self.scheduler.has_awake_engines() {
            for rc in self.windows.values() {
                rc.borrow().request_redraw();
            }
        }

        // SP4.5 + SP8: all panel cycling runs through the scheduler's normal
        // engine loop. Top-level panels via PanelCycleEngine registered at
        // init_panel_view; sub-view panels via the same path on each
        // emSubViewPanel's own sub_scheduler, which is driven from the outer
        // PanelCycleEngine's PanelBehavior::Cycle (SP8).

        // Notice dispatch now happens per-view inside emView::Update (SP5,
        // emView.cpp:1303-1314 parity). No global HandleNotice call here.

        // Update views and tick animators
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame_time)
            .as_secs_f64()
            .clamp(0.001, 0.1);
        self.last_frame_time = now;
        let App {
            ref mut scheduler,
            ref mut tree,
            ref mut input_state,
            ref mut framework_actions,
            ref context,
            ref windows,
            ..
        } = *self;
        let state = input_state;
        for rc in windows.values() {
            let mut win = rc.borrow_mut();
            // Notice dispatch (including mark_viewing_dirty) happens inside
            // emView::Update via emView::HandleNotice (SP5).
            let mut needs_full_repaint = false;

            // Build SchedCtx for this window's VIF and animator ticks.
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context: context,
                current_engine: None,
            };

            // Tick animator (take out to avoid borrow conflict)
            if let Some(mut anim) = win.active_animator.take() {
                if anim.animate(&mut win.view_mut(), tree, dt, &mut sc) {
                    win.active_animator = Some(anim);
                    needs_full_repaint = true;
                }
            }

            // Tick VIF animations (wheel zoom spring, grip pan spring)
            if win.tick_vif_animations(tree, dt, &mut sc) {
                needs_full_repaint = true;
            }

            // Dispatch synthetic events from gesture timer transitions
            // (cycle_gesture may have fired 250ms timeouts → EmuMouse/Visit/Menu)
            let forward_events = win.touch_vif_mut().drain_forward_events();
            if !forward_events.is_empty() {
                for event in &forward_events {
                    match event.variant {
                        InputVariant::Press => {
                            if event.shift {
                                state.press(InputKey::Shift);
                            }
                            if event.ctrl {
                                state.press(InputKey::Ctrl);
                            }
                        }
                        InputVariant::Release => {
                            if event.shift {
                                state.release(InputKey::Shift);
                            }
                            if event.ctrl {
                                state.release(InputKey::Ctrl);
                            }
                        }
                        _ => {}
                    }
                    state.set_mouse(event.mouse_x, event.mouse_y);
                    let mut ev = event.clone();
                    ev.mouse_x = state.mouse_x;
                    ev.mouse_y = state.mouse_y;
                    win.dispatch_input(tree, &ev, state, &mut sc);
                }
                win.invalidate();
                win.request_redraw();
            }

            // SP4: Update runs only via UpdateEngineClass::Cycle now
            // (C++ single-caller model, emView.cpp:2523).

            // Collect invalidation from sub-view panels (C++ invalidation chain:
            // SubViewClass::InvalidateTitle, SubViewPortClass::InvalidateCursor,
            // SubViewPortClass::InvalidatePainting → SuperPanel → parent view).
            win.view_mut().collect_parent_invalidation(tree);

            // Invalidate the active (focused) panel every frame so that
            // cursor blink and other clock-driven updates repaint. This
            // matches C++ emCore where Input() is called for all viewed
            // panels on every frame, and emTextField invalidates itself
            // when the blink timer fires.
            let active_id = win.view().GetActivePanel();
            if let Some(active_id) = active_id {
                win.view_mut().InvalidatePainting(tree, active_id);
            }

            // Check for pending dirty rects from invalidate_painting calls.
            // Convert each dirty rect to tile grid coordinates and mark only
            // the overlapping tiles as dirty (partial repaint).
            let mut has_dirty_rects = false;
            let has_dirty = win.view().has_dirty_rects();
            if has_dirty {
                let dirty = win.view_mut().take_dirty_clip_rects();
                log::trace!(
                    "dirty clip rects: {} regions, bounds {:?}",
                    dirty.GetCount(),
                    dirty.GetMinMax()
                );
                for r in dirty.iter() {
                    win.mark_dirty_rect(r.x1, r.y1, r.x2, r.y2);
                }
                has_dirty_rects = true;
            }

            // Check for viewport changes (scroll/zoom/visit from VIFs)
            if win.view().viewport_changed() {
                win.view_mut().clear_viewport_changed();
                needs_full_repaint = true;
            }

            // Stress test: sync state and force full repaint every frame
            win.view_mut().sync_stress_test();
            if win.view().is_stress_test_active() {
                needs_full_repaint = true;
            }

            if needs_full_repaint {
                win.invalidate();
                win.request_redraw();
            } else if has_dirty_rects {
                win.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_scheduler_shape() {
        let framework = App::new(Box::new(|_app, _el| {}));
        let _: &EngineScheduler = &framework.scheduler;
        assert!(framework.framework_actions.is_empty());
    }
}
