use std::sync::Arc;

use bitflags::bitflags;

use crate::input::{InputEvent, InputKey, InputVariant};
use crate::panel::{PanelId, View};
use crate::render::{TileCache, WgpuCompositor};
use crate::scheduler::SignalId;

use super::app::GpuContext;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct WindowFlags: u32 {
        const MODAL       = 0b0000_0001;
        const UNDECORATED = 0b0000_0010;
        const POPUP       = 0b0000_0100;
        const MAXIMIZED   = 0b0000_1000;
        const FULLSCREEN  = 0b0001_0000;
        const AUTO_DELETE = 0b0010_0000;
    }
}

/// A zuicchini window: owns a winit window, wgpu surface, compositor, tile
/// cache, and view.
pub struct ZuiWindow {
    pub winit_window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    compositor: WgpuCompositor,
    tile_cache: TileCache,
    view: View,
    pub flags: WindowFlags,
    pub close_signal: SignalId,
    root_panel: PanelId,
}

impl ZuiWindow {
    /// Create a new window with a wgpu surface and rendering pipeline.
    pub fn create(
        event_loop: &winit::event_loop::ActiveEventLoop,
        gpu: &GpuContext,
        root_panel: PanelId,
        flags: WindowFlags,
        close_signal: SignalId,
    ) -> Self {
        let mut attrs = winit::window::WindowAttributes::default().with_title("zuicchini");

        if flags.contains(WindowFlags::UNDECORATED) {
            attrs = attrs.with_decorations(false);
        }
        if flags.contains(WindowFlags::MAXIMIZED) {
            attrs = attrs.with_maximized(true);
        }

        let winit_window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let size = winit_window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);

        // Create surface — use Arc clone for 'static lifetime
        let surface = gpu
            .instance
            .create_surface(winit_window.clone())
            .expect("failed to create surface");

        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gpu.device, &surface_config);

        let compositor = WgpuCompositor::new(&gpu.device, format, w, h);
        let tile_cache = TileCache::new(w, h, 256);
        let view = View::new(root_panel, w as f64, h as f64);

        Self {
            winit_window,
            surface,
            surface_config,
            compositor,
            tile_cache,
            view,
            flags,
            close_signal,
            root_panel,
        }
    }

    /// Handle a resize event.
    pub fn resize(&mut self, gpu: &GpuContext, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        self.surface_config.width = w;
        self.surface_config.height = h;
        self.surface.configure(&gpu.device, &self.surface_config);
        self.compositor.resize(w, h);
        self.tile_cache.resize(w, h);
        self.view.set_viewport(w as f64, h as f64);
    }

    /// Render a frame: paint dirty tiles on CPU, upload to GPU, composite.
    pub fn render(&mut self, tree: &mut crate::panel::PanelTree, gpu: &GpuContext) {
        use crate::render::Painter;

        // Paint dirty tiles
        let (cols, rows) = self.tile_cache.grid_size();
        for row in 0..rows {
            for col in 0..cols {
                let tile = self.tile_cache.get_or_create(col, row);
                if tile.dirty {
                    // Clear and repaint
                    tile.image.fill(crate::foundation::Color::BLACK);
                    {
                        let mut painter = Painter::new(&mut tile.image);
                        // Offset painter to tile position
                        let tile_size = crate::render::TILE_SIZE as f64;
                        painter.translate(-(col as f64 * tile_size), -(row as f64 * tile_size));
                        self.view.paint(tree, &mut painter);
                    }
                    tile.dirty = false;

                    // Upload to GPU
                    let tile_ref = self.tile_cache.get(col, row).unwrap();
                    self.compositor
                        .upload_tile(&gpu.device, &gpu.queue, col, row, tile_ref);
                }
            }
        }

        self.tile_cache.advance_frame();

        // Composite and present
        match self.compositor.render_frame(
            &gpu.device,
            &gpu.queue,
            &self.surface,
            &self.surface_config,
        ) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&gpu.device, &self.surface_config);
            }
            Err(e) => {
                log::error!("render error: {e}");
            }
        }
    }

    /// Translate a winit window event to a zuicchini InputEvent.
    pub fn handle_input(event: &winit::event::WindowEvent) -> Option<InputEvent> {
        use winit::event::WindowEvent;
        use winit::keyboard::{Key, NamedKey};

        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                let variant = match event.state {
                    winit::event::ElementState::Pressed => {
                        if event.repeat {
                            InputVariant::Repeat
                        } else {
                            InputVariant::Press
                        }
                    }
                    winit::event::ElementState::Released => InputVariant::Release,
                };

                let key = match &event.logical_key {
                    Key::Named(named) => match named {
                        NamedKey::Escape => Some(InputKey::Escape),
                        NamedKey::Tab => Some(InputKey::Tab),
                        NamedKey::Enter => Some(InputKey::Enter),
                        NamedKey::Backspace => Some(InputKey::Backspace),
                        NamedKey::Delete => Some(InputKey::Delete),
                        NamedKey::Insert => Some(InputKey::Insert),
                        NamedKey::Home => Some(InputKey::Home),
                        NamedKey::End => Some(InputKey::End),
                        NamedKey::PageUp => Some(InputKey::PageUp),
                        NamedKey::PageDown => Some(InputKey::PageDown),
                        NamedKey::ArrowUp => Some(InputKey::ArrowUp),
                        NamedKey::ArrowDown => Some(InputKey::ArrowDown),
                        NamedKey::ArrowLeft => Some(InputKey::ArrowLeft),
                        NamedKey::ArrowRight => Some(InputKey::ArrowRight),
                        NamedKey::Shift => Some(InputKey::Shift),
                        NamedKey::Control => Some(InputKey::Ctrl),
                        NamedKey::Alt => Some(InputKey::Alt),
                        NamedKey::Super => Some(InputKey::Meta),
                        NamedKey::Space => Some(InputKey::Space),
                        NamedKey::F1 => Some(InputKey::F1),
                        NamedKey::F2 => Some(InputKey::F2),
                        NamedKey::F3 => Some(InputKey::F3),
                        NamedKey::F4 => Some(InputKey::F4),
                        NamedKey::F5 => Some(InputKey::F5),
                        NamedKey::F6 => Some(InputKey::F6),
                        NamedKey::F7 => Some(InputKey::F7),
                        NamedKey::F8 => Some(InputKey::F8),
                        NamedKey::F9 => Some(InputKey::F9),
                        NamedKey::F10 => Some(InputKey::F10),
                        NamedKey::F11 => Some(InputKey::F11),
                        NamedKey::F12 => Some(InputKey::F12),
                        _ => None,
                    },
                    Key::Character(c) => {
                        let ch = c.chars().next()?;
                        Some(InputKey::Key(ch))
                    }
                    _ => None,
                };

                let key = key?;
                let mut input_event = InputEvent {
                    key,
                    variant,
                    chars: String::new(),
                    is_repeat: event.repeat,
                    mouse_x: 0.0,
                    mouse_y: 0.0,
                };
                if let Some(ref text) = event.text {
                    input_event.chars = text.to_string();
                }
                Some(input_event)
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let key = match button {
                    winit::event::MouseButton::Left => InputKey::MouseLeft,
                    winit::event::MouseButton::Right => InputKey::MouseRight,
                    winit::event::MouseButton::Middle => InputKey::MouseMiddle,
                    winit::event::MouseButton::Back => InputKey::MouseX1,
                    winit::event::MouseButton::Forward => InputKey::MouseX2,
                    _ => return None,
                };
                let variant = match state {
                    winit::event::ElementState::Pressed => InputVariant::Press,
                    winit::event::ElementState::Released => InputVariant::Release,
                };
                Some(InputEvent {
                    key,
                    variant,
                    chars: String::new(),
                    is_repeat: false,
                    mouse_x: 0.0,
                    mouse_y: 0.0,
                })
            }
            _ => None,
        }
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    pub fn root_panel(&self) -> PanelId {
        self.root_panel
    }

    pub fn request_redraw(&self) {
        self.winit_window.request_redraw();
    }

    /// Mark all tiles as dirty so the next render repaints everything.
    pub fn invalidate(&mut self) {
        self.tile_cache.mark_all_dirty();
    }
}
