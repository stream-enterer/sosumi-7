use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::emContext::emContext;
use crate::emInput::{InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPanelTree::PanelTree;
use crate::emScheduler::EngineScheduler;

use crate::emScreen::emScreen;
use super::emWindow::{WindowFlags, ZuiWindow};

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
pub struct App {
    pub gpu: Option<GpuContext>,
    pub screen: Option<emScreen>,
    pub scheduler: Rc<RefCell<EngineScheduler>>,
    pub context: Rc<emContext>,
    pub tree: PanelTree,
    pub windows: HashMap<WindowId, ZuiWindow>,
    pub input_state: emInputState,
    setup_fn: Option<SetupFn>,
    initialized: bool,
    last_frame_time: Instant,
}

impl App {
    pub fn new(setup: SetupFn) -> Self {
        let scheduler = Rc::new(RefCell::new(EngineScheduler::new()));
        let context = emContext::NewRootWithScheduler(Rc::clone(&scheduler));
        Self {
            gpu: None,
            screen: None,
            scheduler,
            context,
            tree: PanelTree::new(),
            windows: HashMap::new(),
            input_state: emInputState::new(),
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
        win: &mut ZuiWindow,
        tree: &mut PanelTree,
        input_state: &mut emInputState,
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
            win.dispatch_input(tree, &ev, input_state);
        }
        win.invalidate();
        win.request_redraw();
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
        let geom_sig = self.scheduler.borrow_mut().create_signal();
        let win_sig = self.scheduler.borrow_mut().create_signal();
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
                    .map(|w| w.flags.contains(WindowFlags::AUTO_DELETE))
                    .unwrap_or(true);

                if let Some(win) = self.windows.get(&window_id) {
                    self.scheduler.borrow_mut().fire(win.close_signal);
                }

                if auto_delete {
                    self.windows.remove(&window_id);
                }

                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    win.resize(gpu, &mut self.tree, size.width, size.height);
                    // Don't request_redraw here — about_to_wait will detect the
                    // layout change from the new tallness and issue a single
                    // repaint after layout is settled.
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    win.render(&mut self.tree, gpu);
                }
            }
            WindowEvent::Focused(focused) => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    win.view_mut().SetFocused(&mut self.tree, focused);
                    win.invalidate();
                    win.request_redraw();
                }
            }
            WindowEvent::Touch(ref touch) => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    win.handle_touch(touch, &mut self.tree);
                    Self::dispatch_forward_events(
                        win,
                        &mut self.tree,
                        &mut self.input_state,
                    );
                    win.invalidate();
                    win.request_redraw();
                }
            }
            ref input_event => {
                if let Some(mut input) = ZuiWindow::handle_input(input_event) {
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

                    if let Some(win) = self.windows.get_mut(&window_id) {
                        win.dispatch_input(&mut self.tree, &input, &mut self.input_state);
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Fire flags_signal for any windows whose flags changed this frame.
        let flags_signals: Vec<_> = self
            .windows
            .values_mut()
            .filter_map(|win| {
                if win.flags_changed() {
                    win.clear_flags_changed();
                    Some(win.flags_signal)
                } else {
                    None
                }
            })
            .collect();
        for sig in flags_signals {
            self.scheduler.borrow_mut().fire(sig);
        }

        // Run one scheduler time slice
        self.scheduler.borrow_mut().DoTimeSlice(&mut self.tree, &mut self.windows);

        // Run per-frame panel cycles
        self.tree.run_panel_cycles();

        // Deliver notices (includes layout dispatch)
        let window_focused = self.windows.values().any(|w| w.view().IsFocused());
        let pixel_tallness = self
            .windows
            .values()
            .next()
            .map(|w| w.view().GetCurrentPixelTallness())
            .unwrap_or(1.0);
        let had_notices = self.tree.HandleNotice(window_focused, pixel_tallness);

        // Update views and tick animators
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame_time)
            .as_secs_f64()
            .clamp(0.001, 0.1);
        self.last_frame_time = now;
        let tree = &mut self.tree;
        let state = &mut self.input_state;
        for win in self.windows.values_mut() {
            // Layout changes from notices require viewed coordinate recomputation.
            if had_notices {
                win.view_mut().mark_viewing_dirty();
            }
            let mut needs_full_repaint = had_notices;

            // Tick animator (take out to avoid borrow conflict)
            if let Some(mut anim) = win.active_animator.take() {
                if anim.animate(win.view_mut(), tree, dt) {
                    win.active_animator = Some(anim);
                    needs_full_repaint = true;
                }
            }

            // Tick VIF animations (wheel zoom spring, grip pan spring)
            if win.tick_vif_animations(tree, dt) {
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
                    win.dispatch_input(tree, &ev, state);
                }
                win.invalidate();
                win.request_redraw();
            }

            // Update view (recompute viewing coords, auto-select active)
            win.view_mut().update(tree);

            // Collect invalidation from sub-view panels (C++ invalidation chain:
            // SubViewClass::InvalidateTitle, SubViewPortClass::InvalidateCursor,
            // SubViewPortClass::InvalidatePainting → SuperPanel → parent view).
            win.view_mut().collect_parent_invalidation(tree);

            // Control panel lifecycle
            if win.view().is_control_panel_invalid() {
                // Destroy old control panel
                if let Some(old_id) = win.control_panel_id.take() {
                    win.control_tree.remove(old_id);
                }

                // Create new control panel in the control tree.
                // Extract active panel from view first to avoid borrow conflict.
                let active = win.view().GetActivePanel();
                let control_root = win.control_view.GetRootPanel();
                let new_id = active.and_then(|active_id| {
                    tree.create_control_panel_in(
                        active_id,
                        &mut win.control_tree,
                        control_root,
                        "context",
                    )
                });
                win.control_panel_id = new_id;

                // Show or hide the control strip
                if new_id.is_some() {
                    win.show_control_strip(tree);
                } else {
                    win.hide_control_strip(tree);
                }

                win.view_mut().clear_control_panel_invalid();
                needs_full_repaint = true;
            }

            // Deliver notices for control tree
            if win.control_strip_height > 0 {
                win.control_tree
                    .HandleNotice(window_focused, pixel_tallness);
                win.control_view.update(&mut win.control_tree);
            }

            // Invalidate the active (focused) panel every frame so that
            // cursor blink and other clock-driven updates repaint. This
            // matches C++ emCore where Input() is called for all viewed
            // panels on every frame, and emTextField invalidates itself
            // when the blink timer fires.
            if let Some(active_id) = win.view().GetActivePanel() {
                win.view_mut().InvalidatePainting(tree, active_id);
            }

            // Check for pending dirty rects from invalidate_painting calls.
            // Convert each dirty rect to tile grid coordinates and mark only
            // the overlapping tiles as dirty (partial repaint).
            let mut has_dirty_rects = false;
            if win.view().has_dirty_rects() {
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
