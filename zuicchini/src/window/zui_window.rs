use std::sync::Arc;

use bitflags::bitflags;

use crate::input::{InputEvent, InputKey, InputState, InputVariant};
use crate::panel::{
    KeyboardZoomScrollVIF, MouseZoomScrollVIF, PanelId, PanelTree, View, ViewAnimator,
    ViewInputFilter,
};
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
    /// Pre-allocated viewport-sized buffer for single-pass rendering.
    /// Used when many tiles are dirty (e.g. during panning) to avoid
    /// redundant tree walks and primitive rasterization across tiles.
    viewport_buffer: crate::foundation::Image,
    pub flags: WindowFlags,
    pub close_signal: SignalId,
    root_panel: PanelId,
    vif_chain: Vec<Box<dyn ViewInputFilter>>,
    pub active_animator: Option<Box<dyn ViewAnimator>>,
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
        if flags.contains(WindowFlags::FULLSCREEN) {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
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
        let viewport_buffer = crate::foundation::Image::new(w, h, 4);
        let view = View::new(root_panel, w as f64, h as f64);

        let vif_chain: Vec<Box<dyn ViewInputFilter>> = vec![
            Box::new(MouseZoomScrollVIF::new()),
            Box::new(KeyboardZoomScrollVIF::new()),
        ];

        Self {
            winit_window,
            surface,
            surface_config,
            compositor,
            tile_cache,
            viewport_buffer,
            view,
            flags,
            close_signal,
            root_panel,
            vif_chain,
            active_animator: None,
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
        self.viewport_buffer.setup(w, h, 4);
        self.view.set_viewport(w as f64, h as f64);
    }

    /// Render a frame: paint dirty tiles on CPU, upload to GPU, composite.
    pub fn render(&mut self, tree: &mut crate::panel::PanelTree, gpu: &GpuContext) {
        use crate::render::Painter;

        let (cols, rows) = self.tile_cache.grid_size();
        let tile_size = crate::render::TILE_SIZE;

        // Count dirty tiles to choose rendering strategy.
        let mut dirty_count = 0u32;
        for row in 0..rows {
            for col in 0..cols {
                if self.tile_cache.get_or_create(col, row).dirty {
                    dirty_count += 1;
                }
            }
        }

        if dirty_count > cols * rows / 2 {
            // Many dirty tiles (e.g. panning): paint into viewport-sized buffer
            // once, then copy tile-sized chunks. Avoids redundant tree walks and
            // re-rasterization of primitives across tiles.
            self.viewport_buffer.fill(crate::foundation::Color::BLACK);
            {
                let mut painter = Painter::new(&mut self.viewport_buffer);
                self.view.paint(tree, &mut painter);
            }
            for row in 0..rows {
                for col in 0..cols {
                    let tile = self.tile_cache.get_or_create(col, row);
                    if tile.dirty {
                        tile.image.copy_from_rect(
                            0,
                            0,
                            &self.viewport_buffer,
                            (col * tile_size, row * tile_size, tile_size, tile_size),
                        );
                        tile.dirty = false;
                        let tile_ref = self.tile_cache.get(col, row).unwrap();
                        self.compositor
                            .upload_tile(&gpu.device, &gpu.queue, col, row, tile_ref);
                    }
                }
            }
        } else {
            // Few dirty tiles: paint per-tile (avoids painting the full viewport).
            for row in 0..rows {
                for col in 0..cols {
                    let tile = self.tile_cache.get_or_create(col, row);
                    if tile.dirty {
                        tile.image.fill(crate::foundation::Color::BLACK);
                        {
                            let mut painter = Painter::new(&mut tile.image);
                            let ts = tile_size as f64;
                            painter.translate(-(col as f64 * ts), -(row as f64 * ts));
                            self.view.paint(tree, &mut painter);
                        }
                        tile.dirty = false;
                        let tile_ref = self.tile_cache.get(col, row).unwrap();
                        self.compositor
                            .upload_tile(&gpu.device, &gpu.queue, col, row, tile_ref);
                    }
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
                    shift: false,
                    ctrl: false,
                    alt: false,
                    meta: false,
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
                    shift: false,
                    ctrl: false,
                    alt: false,
                    meta: false,
                })
            }
            WindowEvent::CursorMoved { position, .. } => Some(InputEvent {
                key: InputKey::MouseLeft, // Dummy key for position-only events
                variant: InputVariant::Move,
                chars: String::new(),
                is_repeat: false,
                mouse_x: position.x,
                mouse_y: position.y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
            }),
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (*x as f64, *y as f64),
                    winit::event::MouseScrollDelta::PixelDelta(p) => (p.x, p.y),
                };
                // Encode scroll as a wheel event with delta in mouse_x/y
                if dy.abs() > dx.abs() {
                    Some(InputEvent {
                        key: if dy > 0.0 {
                            InputKey::WheelUp
                        } else {
                            InputKey::WheelDown
                        },
                        variant: InputVariant::Press,
                        chars: String::new(),
                        is_repeat: false,
                        mouse_x: dx,
                        mouse_y: dy,
                        shift: false,
                        ctrl: false,
                        alt: false,
                        meta: false,
                    })
                } else if dx.abs() > 0.0 {
                    Some(InputEvent {
                        key: if dx > 0.0 {
                            InputKey::WheelRight
                        } else {
                            InputKey::WheelLeft
                        },
                        variant: InputVariant::Press,
                        chars: String::new(),
                        is_repeat: false,
                        mouse_x: dx,
                        mouse_y: dy,
                        shift: false,
                        ctrl: false,
                        alt: false,
                        meta: false,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Dispatch an input event through VIF chain, then to panel behavior.
    pub fn dispatch_input(&mut self, tree: &mut PanelTree, event: &InputEvent, state: &InputState) {
        // Run VIF chain
        for vif in &mut self.vif_chain {
            if vif.filter(event, state, &mut self.view) {
                return;
            }
        }

        // For mouse press: hit test and set active panel
        if event.variant == InputVariant::Press
            && matches!(
                event.key,
                InputKey::MouseLeft | InputKey::MouseRight | InputKey::MouseMiddle
            )
        {
            if let Some(hit) = self
                .view
                .get_focusable_panel_at(tree, event.mouse_x, event.mouse_y)
            {
                self.view.set_active_panel(tree, hit, false);
            }
        }

        // Stamp modifier keys from InputState onto the event
        let ev = event.clone().with_modifiers(state);

        // Dispatch to active panel's behavior with panel-local mouse coords
        let wf = self.view.window_focused();
        if let Some(active) = self.view.active() {
            let mut consumed = false;
            // Transform mouse coords from viewport pixels to panel-local space
            let mut panel_ev = ev.clone();
            panel_ev.mouse_x = tree.view_to_panel_x(active, ev.mouse_x);
            panel_ev.mouse_y = tree.view_to_panel_y(active, ev.mouse_y);

            if let Some(mut behavior) = tree.take_behavior(active) {
                let panel_state = tree.build_panel_state(active, wf);
                consumed = behavior.input(&panel_ev, &panel_state, state);
                // TF-003: Process scroll-to-visible requests from behaviors
                if let Some(rect) = behavior.take_scroll_to_visible() {
                    self.view.scroll_to_panel_rect(tree, active, rect);
                }
                tree.put_behavior(active, behavior);
            }

            // Bubble up parent chain if not consumed
            if !consumed {
                let mut cur = tree.parent(active);
                while let Some(parent_id) = cur {
                    // Re-transform for each parent's coordinate space
                    let mut parent_ev = ev.clone();
                    parent_ev.mouse_x = tree.view_to_panel_x(parent_id, ev.mouse_x);
                    parent_ev.mouse_y = tree.view_to_panel_y(parent_id, ev.mouse_y);

                    if let Some(mut behavior) = tree.take_behavior(parent_id) {
                        let panel_state = tree.build_panel_state(parent_id, wf);
                        consumed = behavior.input(&parent_ev, &panel_state, state);
                        // TF-003: Process scroll-to-visible from parent behaviors
                        if let Some(rect) = behavior.take_scroll_to_visible() {
                            self.view.scroll_to_panel_rect(tree, parent_id, rect);
                        }
                        tree.put_behavior(parent_id, behavior);
                        if consumed {
                            break;
                        }
                    }
                    cur = tree.parent(parent_id);
                }
            }
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

    /// Request the window manager to bring this window to front.
    pub fn raise(&self) {
        self.winit_window.focus_window();
    }

    /// Set the window title.
    pub fn set_title(&self, title: &str) {
        self.winit_window.set_title(title);
    }

    /// Mark all tiles as dirty so the next render repaints everything.
    pub fn invalidate(&mut self) {
        self.tile_cache.mark_all_dirty();
    }
}
