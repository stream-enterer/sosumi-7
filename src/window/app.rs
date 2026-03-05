use std::collections::HashMap;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::panel::PanelTree;
use crate::scheduler::EngineScheduler;

use super::screen::Screen;
use super::zui_window::{WindowFlags, ZuiWindow};

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
                label: Some("zuicchini_device"),
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
    pub screen: Option<Screen>,
    pub scheduler: EngineScheduler,
    pub tree: PanelTree,
    pub windows: HashMap<WindowId, ZuiWindow>,
    setup_fn: Option<SetupFn>,
    initialized: bool,
}

impl App {
    pub fn new(setup: SetupFn) -> Self {
        Self {
            gpu: None,
            screen: None,
            scheduler: EngineScheduler::new(),
            tree: PanelTree::new(),
            windows: HashMap::new(),
            setup_fn: Some(setup),
            initialized: false,
        }
    }

    /// Run the application. This blocks until all windows are closed.
    pub fn run(self) {
        let event_loop = winit::event_loop::EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        let mut app = self;
        event_loop.run_app(&mut app).expect("event loop error");
    }

    /// Get the GPU context (panics if not yet initialized).
    pub fn gpu(&self) -> &GpuContext {
        self.gpu.as_ref().expect("GPU not initialized yet")
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

        // Scan monitors
        self.screen = Some(Screen::from_event_loop(event_loop));

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
                    self.scheduler.fire(win.close_signal);
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
                    win.resize(gpu, size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    win.render(&mut self.tree, gpu);
                }
            }
            ref input_event => {
                if let Some(input) = ZuiWindow::handle_input(input_event) {
                    if let Some(win) = self.windows.get_mut(&window_id) {
                        // Dispatch to the focused panel's behavior
                        if let Some(focused) = win.view().focused() {
                            if let Some(mut behavior) = self.tree.take_behavior(focused) {
                                behavior.input(&input);
                                self.tree.put_behavior(focused, behavior);
                            }
                        }
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Run one scheduler time slice
        self.scheduler.do_time_slice();

        // Deliver notices (includes layout dispatch)
        self.tree.deliver_notices();

        // Request redraws for all windows
        for win in self.windows.values() {
            win.request_redraw();
        }
    }
}
