use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::emClipboard::emClipboard;
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
    // Phase-2 port-ownership-rewrite: narrowed from
    // `HashMap<WindowId, Rc<RefCell<emWindow>>>` to plain `emWindow`.
    // Top-level windows only. Popups live on `emView::PopupWindow`,
    // matching C++ ownership (emView.h:670 `emWindow * PopupWindow`).
    // Winit events to popup WindowIds are resolved via `find_window_mut`
    // which scans parent views' PopupWindow handles.
    pub windows: HashMap<WindowId, emWindow>,
    /// Phase 3.5.A Task 7: the home window's WindowId, set once by the
    /// setup callback when the home `emWindow` is inserted. `None` until
    /// setup runs. The home window owns its `PanelTree` — reach it via
    /// `self.windows.get_mut(&home_window_id.unwrap()).unwrap().tree` or
    /// the `home_tree_mut()` helper. Formerly `App::tree`.
    pub home_window_id: Option<WindowId>,
    /// Framework-level deferred actions produced by scheduler/view code that
    /// need to run back on `App` between time slices. Spec §3.1 / §3.7 —
    /// passed as `&mut Vec<DeferredAction>` into `EngineScheduler::DoTimeSlice`.
    pub(crate) framework_actions: Vec<FrameworkDeferredAction>,
    pub input_state: emInputState,
    /// Input queue drained by `InputDispatchEngine` (Phase 3) per spec §3.1
    /// and §4 D4.9. Produced by `window_event` on each winit input;
    /// consumed once per slice by the framework-owned InputDispatchEngine
    /// registered at top priority.
    pub(crate) pending_inputs: Vec<(WindowId, emInputEvent)>,
    /// EngineId of the `InputDispatchEngine` registered at framework init.
    /// Winit callbacks wake this engine after enqueuing events so the
    /// scheduler drains `pending_inputs` on the next tick.
    pub(crate) input_dispatch_engine_id: crate::emEngine::EngineId,
    /// Deferred actions queued by input handlers that need `&ActiveEventLoop`
    /// (e.g., window creation for Duplicate/CreateControlWindow, popup
    /// surface materialization from `emView::RawVisitAbs`).
    /// Drained each frame in `about_to_wait`.
    ///
    /// `Rc<RefCell<...>>` so `emView` can hold a handle and enqueue without
    /// a borrow of `App`.
    pub pending_actions: Rc<RefCell<Vec<DeferredAction>>>,
    /// Chartered §3.6(a): mutated from winit text-event callbacks that lack &mut framework reach.
    ///
    /// Phase-3 Task-2 relocation: in C++, `emClipboard` is looked up via
    /// `emRef<emClipboard>` on `emContext`; in Rust, winit text-event callbacks
    /// need write access without `&mut framework` reach, so the clipboard is
    /// chartered here instead of on `emContext`. Accessed through
    /// `EngineCtx::clipboard_mut` / `SchedCtx::clipboard_mut` during engine
    /// cycles, or directly via `framework.clipboard.borrow_mut()` from
    /// winit-side callbacks.
    pub clipboard: RefCell<Option<Box<dyn emClipboard>>>,
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
        // Register the InputDispatchEngine at top priority (Phase 3 / spec
        // §3.1 / §4 D4.9): winit input callbacks enqueue into
        // `pending_inputs` and wake this engine; its `Cycle` drains the
        // queue at the top of each slice before any other engine runs.
        let input_dispatch_engine_id = scheduler.register_engine(
            Box::new(crate::emInputDispatchEngine::InputDispatchEngine),
            crate::emEngine::Priority::VeryHigh,
            crate::emPanelScope::PanelScope::Framework,
        );
        let context = emContext::NewRoot();
        Self {
            gpu: None,
            screen: None,
            scheduler,
            context,
            windows: HashMap::new(),
            home_window_id: None,
            framework_actions: Vec::new(),
            input_state: emInputState::new(),
            pending_inputs: Vec::new(),
            input_dispatch_engine_id,
            pending_actions: Rc::new(RefCell::new(Vec::new())),
            clipboard: RefCell::new(None),
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

    /// Resolve a winit `WindowId` to an `&mut emWindow`, searching both the
    /// top-level `self.windows` map and any popup held by a top-level
    /// window's view (`emView::PopupWindow`).
    ///
    /// C++ reference (emView.cpp:1634-1642 + emView.h:670): the popup
    /// `emWindow*` is owned by the launching `emView`, not a framework
    /// registry. The C++ backend dispatches OS events to each `emWindow`
    /// via its own callback registration; Rust must look up by `WindowId`
    /// because winit delivers events through a single `ApplicationHandler`.
    /// The scan is O(N_windows) which is fine for normal UIs (handful of
    /// windows). This is Task-8 Path B — matches C++ ownership exactly.
    pub(crate) fn find_window_mut(
        windows: &mut HashMap<WindowId, emWindow>,
        window_id: WindowId,
    ) -> Option<&mut emWindow> {
        if windows.contains_key(&window_id) {
            return windows.get_mut(&window_id);
        }
        // Popup path: scan for a parent window whose view holds a popup
        // whose materialized WindowId matches.
        for win in windows.values_mut() {
            let matches = win
                .view()
                .PopupWindow
                .as_ref()
                .and_then(|p| {
                    p.winit_window_if_materialized()
                        .map(|w| w.id() == window_id)
                })
                .unwrap_or(false);
            if matches {
                return win.view_mut().PopupWindow.as_deref_mut();
            }
        }
        None
    }

    /// Mutable access to the home window's panel tree.
    ///
    /// Phase 3.5.A Task 7: `App::tree` is gone; the home window owns its
    /// tree. This helper looks up the home window via `home_window_id`.
    /// Panics if the home window has not been created yet (setup callback
    /// has not run) — same precondition as the former `App::tree`.
    pub fn home_tree_mut(&mut self) -> &mut PanelTree {
        let id = self
            .home_window_id
            .expect("home window not yet created (setup callback has not run)");
        &mut self
            .windows
            .get_mut(&id)
            .expect("home window present in App::windows")
            .tree
    }

    /// Immutable view of the home window's panel tree. See `home_tree_mut`.
    pub fn home_tree(&self) -> &PanelTree {
        let id = self
            .home_window_id
            .expect("home window not yet created (setup callback has not run)");
        &self
            .windows
            .get(&id)
            .expect("home window present in App::windows")
            .tree
    }

    /// Materialize the currently-pending popup window's OS surface.
    ///
    /// Since `App::windows` holds plain `emWindow` (no `Rc`) the popup
    /// cannot be shared between a closure and `emView::PopupWindow` — it
    /// lives in `emView::PopupWindow` for its entire lifetime (Task-8
    /// Path B, mirroring C++ `emView::PopupWindow` ownership from
    /// `emView.cpp:1636`). This method walks `App::windows` to find the
    /// home view holding a `Pending` popup, materializes the OS surface
    /// in place, and wires the new WindowId onto the view-port
    /// back-reference.
    ///
    /// **Cancellation:** if no view holds a `Pending` popup at drain time,
    /// the popup was torn down between enqueue and drain (same-frame
    /// popup-enter/exit). We silently return.
    ///
    /// Winit events addressed to the popup's WindowId are routed via
    /// `find_window_mut` (see above).
    pub(crate) fn materialize_pending_popup(&mut self, event_loop: &ActiveEventLoop) {
        use crate::emWindow::{MaterializedSurface, OsSurface};

        // Find a home window whose view has a `Pending` popup.
        let home_key = self.windows.iter().find_map(|(id, win)| {
            let view = win.view();
            let has_pending = view
                .PopupWindow
                .as_ref()
                .map(|p| matches!(p.os_surface, OsSurface::Pending(_)))
                .unwrap_or(false);
            if has_pending {
                Some(*id)
            } else {
                None
            }
        });
        let Some(home_key) = home_key else {
            // Natural cancellation: popup was torn down before drain.
            return;
        };

        // Extract Pending params without holding the borrow across winit
        // create_window (which needs `&mut self` via event_loop later).
        let (flags, caption, requested_pos_size) = {
            let home = self
                .windows
                .get(&home_key)
                .expect("home_key just found above");
            let view = home.view();
            let popup = view
                .PopupWindow
                .as_ref()
                .expect("Pending popup just found above");
            match &popup.os_surface {
                OsSurface::Pending(p) => (p.flags, p.caption.clone(), p.requested_pos_size),
                OsSurface::Materialized(_) => unreachable!("checked Pending above"),
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
        let popup_window_id = winit_window.id();

        // Install the materialized surface on the popup in place and update
        // its view's geometry. Scope the `&mut home` borrow tightly so it
        // does not conflict with `self.context` / `self.scheduler` below.
        {
            let root = self.context.clone();
            let App {
                scheduler,
                framework_actions,
                windows,
                clipboard,
                ..
            } = self;
            let home = windows.get_mut(&home_key).expect("home_key still present");
            let view = home.view_mut();
            let popup = view
                .PopupWindow
                .as_mut()
                .expect("Pending popup still present");
            popup.os_surface = OsSurface::Materialized(Box::new(materialized));
            popup.wire_viewport_window_id(popup_window_id);
            // Phase 3.5.A Task 8: popup owns its own PanelTree + RootPanel.
            // SetGeometry mutates the popup view's root panel; the tree it
            // writes to must be the popup's own tree (not the home's).
            // Take the popup's tree out for the SetGeometry call, put it
            // back at scope exit.
            let mut popup_tree = popup.take_tree();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context: &root,
                framework_clipboard: clipboard,
                current_engine: None,
            };
            popup.view_mut().SetGeometry(
                &mut popup_tree,
                0.0,
                0.0,
                w as f64,
                h as f64,
                1.0,
                &mut sc,
            );
            popup.put_tree(popup_tree);
        }

        winit_window.request_redraw();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Deregister the framework-owned InputDispatchEngine so the
        // scheduler's drop-time invariant (no engines remaining) holds.
        // `remove_engine` is idempotent on unknown ids so double-drop is
        // safe; it is the only framework-owned engine registered in
        // `App::new`.
        self.scheduler.remove_engine(self.input_dispatch_engine_id);
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
                // Top-level close: auto-delete from `windows` and fire its
                // close signal. Popup close: fire the popup's close signal
                // but do NOT remove it here — teardown happens in
                // `emView::RawVisitAbs`'s pop path when the view zooms
                // back inside home (C++ emView.cpp:1676-1680). Popup is
                // always AUTO_DELETE but ownership lives on emView.
                let (is_popup, auto_delete, sig) = if let Some(win) = self.windows.get(&window_id) {
                    (
                        false,
                        win.flags.contains(WindowFlags::AUTO_DELETE),
                        Some(win.close_signal),
                    )
                } else if let Some(popup) = Self::find_window_mut(&mut self.windows, window_id) {
                    (true, false, Some(popup.close_signal))
                } else {
                    (false, true, None)
                };

                if let Some(sig) = sig {
                    self.scheduler.fire(sig);
                }

                if !is_popup && auto_delete {
                    self.windows.remove(&window_id);
                }

                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    let root = self.context.clone();
                    let mut sc = crate::emEngineCtx::SchedCtx {
                        scheduler: &mut self.scheduler,
                        framework_actions: &mut self.framework_actions,
                        root_context: &root,
                        framework_clipboard: &self.clipboard,
                        current_engine: None,
                    };
                    // Phase 3.5.A Task 7: emWindow::resize drops its external
                    // tree param; the window uses its own `self.tree` via
                    // internal destructure.
                    win.resize(gpu, size.width, size.height, &mut sc);
                    win.set_geometry_changed();
                    // Don't request_redraw here — about_to_wait will detect the
                    // layout change from the new tallness and issue a single
                    // repaint after layout is settled.
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    win.set_geometry_changed();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    // Phase 3.5.A Task 7: render uses `self.tree` internally.
                    win.render(gpu);
                }
            }
            WindowEvent::Focused(focused) => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    // Phase 3.5.A Task 7: window owns its tree; take it out
                    // for the SetFocused call, then put it back.
                    let mut tree = win.take_tree();
                    win.view_mut().SetFocused(&mut tree, focused);
                    win.put_tree(tree);
                    win.set_focus_changed();
                    win.invalidate();
                    win.request_redraw();
                }
            }
            WindowEvent::Touch(ref touch) => {
                let forward_events = {
                    let Some(win) = Self::find_window_mut(&mut self.windows, window_id) else {
                        return;
                    };
                    let mut sc = crate::emEngineCtx::SchedCtx {
                        scheduler: &mut self.scheduler,
                        framework_actions: &mut self.framework_actions,
                        root_context: &self.context,
                        framework_clipboard: &self.clipboard,
                        current_engine: None,
                    };
                    // Phase 3.5.A Task 7: window owns its tree; handle_touch
                    // splits internally.
                    win.handle_touch(touch, &mut sc);
                    win.touch_vif_mut().drain_forward_events()
                };
                // Phase 3: enqueue forward events for the
                // InputDispatchEngine rather than dispatching inline.
                // Modifier-key bookkeeping stays here because it touches
                // `input_state` directly at winit-callback granularity
                // (press/release persistence across frames).
                if !forward_events.is_empty() {
                    for event in &forward_events {
                        match event.variant {
                            InputVariant::Press => {
                                if event.shift {
                                    self.input_state.press(InputKey::Shift);
                                }
                                if event.ctrl {
                                    self.input_state.press(InputKey::Ctrl);
                                }
                            }
                            InputVariant::Release => {
                                if event.shift {
                                    self.input_state.release(InputKey::Shift);
                                }
                                if event.ctrl {
                                    self.input_state.release(InputKey::Ctrl);
                                }
                            }
                            _ => {}
                        }
                        self.input_state.set_mouse(event.mouse_x, event.mouse_y);
                        let mut ev = event.clone();
                        ev.mouse_x = self.input_state.mouse_x;
                        ev.mouse_y = self.input_state.mouse_y;
                        self.pending_inputs.push((window_id, ev));
                    }
                    self.scheduler.wake_up(self.input_dispatch_engine_id);
                }
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
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

                    // Phase 3 / spec §4 D4.9: enqueue for the
                    // InputDispatchEngine to drain at top priority on the
                    // next time slice. `find_window_mut` still scans so we
                    // only enqueue for known (top-level or popup) windows —
                    // unknown WindowIds are silently dropped, matching the
                    // pre-migration `if let Some(win) = ...` gate.
                    if Self::find_window_mut(&mut self.windows, window_id).is_some() {
                        self.pending_inputs.push((window_id, input));
                        self.scheduler.wake_up(self.input_dispatch_engine_id);
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
        for win in self.windows.values_mut() {
            if win.view().pending_framework_actions.is_none() {
                win.view_mut()
                    .set_pending_framework_actions(self.pending_actions.clone());
            }
        }

        // Process deferred actions (window creation from Duplicate/ccw,
        // popup surface materialization, etc.). Drain by move.
        //
        // Phase-2 port-ownership-rewrite: closures no longer carry a captured
        // `Rc<RefCell<emWindow>>`; `App::windows` holds plain `emWindow`.
        // Popup materialization finds its target by walking `App::windows`
        // for a view whose `emView::PopupWindow` is in `Pending` state
        // (see `materialize_pending_popup`). Task-8 Path B — matches C++
        // `emView.cpp:1636` ownership (popup owned by launching view).
        let actions: Vec<DeferredAction> = self.pending_actions.borrow_mut().drain(..).collect();
        for action in actions {
            action(self, event_loop);
        }

        // Fire signals for any windows whose state changed this frame.
        let changed_signals: Vec<_> = self
            .windows
            .values_mut()
            .flat_map(|win| {
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
                ref mut windows,
                ref context,
                ref mut framework_actions,
                ref mut pending_inputs,
                ref mut input_state,
                ref clipboard,
                ..
            } = *self;
            scheduler.DoTimeSlice(
                windows,
                context,
                framework_actions,
                pending_inputs,
                input_state,
                clipboard,
            );
        }

        // Phase 1.75 Task 5 (continuation): the former post-slice
        // adapter-registration catch-up pass has been deleted.
        // `register_engine_for` is now synchronous (no view borrow held),
        // so `create_child` from inside an engine's `Cycle` registers the
        // adapter inline. Scheduler dispatch no longer borrows the
        // tree/view in a way that blocks registration.
        //
        // Keep event loop pumping while engines are active.
        // C++ runs a tight 10ms loop; Rust uses event-driven winit with
        // ControlFlow::Wait which only fires about_to_wait on OS events.
        // Requesting redraws ensures continuous cycling during startup,
        // animations, and any other engine activity.
        if self.scheduler.has_awake_engines() {
            for win in self.windows.values() {
                win.request_redraw();
            }
        }

        // SP4.5 + SP8 + Phase 1.75: all panel cycling runs through the
        // scheduler's normal engine loop. Top-level panels via
        // PanelCycleEngine registered at init_panel_view; sub-view panels
        // register on the SAME outer scheduler with
        // `PanelScope::SubView(outer_id, Outer)` so cross-tree dispatch
        // resolves via take/put on the sub-view's behavior (spec §3.3).

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
            ref mut input_state,
            ref mut framework_actions,
            ref context,
            ref mut windows,
            ref mut pending_inputs,
            ref clipboard,
            input_dispatch_engine_id,
            ..
        } = *self;
        let state = input_state;
        for (win_id, win) in windows.iter_mut() {
            // Notice dispatch (including mark_viewing_dirty) happens inside
            // emView::Update via emView::HandleNotice (SP5).
            let mut needs_full_repaint = false;

            // Build SchedCtx for this window's VIF and animator ticks.
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context: context,
                framework_clipboard: clipboard,
                current_engine: None,
            };

            // Phase 3.5.A Task 7: each window owns its tree. Take it out for
            // the animator + view-side calls below that require `&mut tree`
            // alongside `&mut view`, then put it back.
            let mut tree = win.take_tree();

            // Tick animator (take out to avoid borrow conflict)
            if let Some(mut anim) = win.active_animator.take() {
                if anim.animate(win.view_mut(), &mut tree, dt, &mut sc) {
                    win.active_animator = Some(anim);
                    needs_full_repaint = true;
                }
            }

            // Tick VIF animations (wheel zoom spring, grip pan spring)
            win.put_tree(tree);
            if win.tick_vif_animations(dt, &mut sc) {
                needs_full_repaint = true;
            }
            let mut tree = win.take_tree();

            // Dispatch synthetic events from gesture timer transitions
            // (cycle_gesture may have fired 250ms timeouts → EmuMouse/Visit/Menu)
            //
            // Phase 3: enqueue for InputDispatchEngine instead of inline
            // dispatch. Modifier-key state is updated here (winit-callback-
            // granularity parity). The queued events will be drained on the
            // next DoTimeSlice tick; `request_redraw` below keeps the event
            // loop pumping so that tick happens immediately.
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
                    pending_inputs.push((*win_id, ev));
                }
                scheduler.wake_up(input_dispatch_engine_id);
                win.invalidate();
                win.request_redraw();
            }

            // SP4: Update runs only via UpdateEngineClass::Cycle now
            // (C++ single-caller model, emView.cpp:2523).

            // Collect invalidation from sub-view panels (C++ invalidation chain:
            // SubViewClass::InvalidateTitle, SubViewPortClass::InvalidateCursor,
            // SubViewPortClass::InvalidatePainting → SuperPanel → parent view).
            win.view_mut().collect_parent_invalidation(&mut tree);

            // Invalidate the active (focused) panel every frame so that
            // cursor blink and other clock-driven updates repaint. This
            // matches C++ emCore where Input() is called for all viewed
            // panels on every frame, and emTextField invalidates itself
            // when the blink timer fires.
            let active_id = win.view().GetActivePanel();
            if let Some(active_id) = active_id {
                win.view_mut().InvalidatePainting(&tree, active_id);
            }

            // Phase 3.5.A Task 7: put the tree back on the window.
            win.put_tree(tree);

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
