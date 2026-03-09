use std::sync::Arc;

use bitflags::bitflags;

use crate::foundation::Image;
use crate::input::{InputEvent, InputKey, InputState, InputVariant};
use crate::panel::{
    KeyboardZoomScrollVIF, MouseZoomScrollVIF, PanelId, PanelTree, View, ViewAnimator,
    ViewInputFilter,
};
use crate::render::{TileCache, WgpuCompositor};
use crate::scheduler::SignalId;

use super::app::GpuContext;
use super::screen::Screen;

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
    window_icon: Option<Image>,
    screensaver_inhibit_count: u32,
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
            window_icon: None,
            screensaver_inhibit_count: 0,
        }
    }

    /// Handle a resize event.
    pub fn resize(
        &mut self,
        gpu: &GpuContext,
        tree: &mut crate::panel::PanelTree,
        width: u32,
        height: u32,
    ) {
        let w = width.max(1);
        let h = height.max(1);
        self.surface_config.width = w;
        self.surface_config.height = h;
        self.surface.configure(&gpu.device, &self.surface_config);
        self.compositor.resize(w, h);
        self.tile_cache.resize(w, h);
        self.viewport_buffer.setup(w, h, 4);
        self.view.set_viewport(tree, w as f64, h as f64);
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

        // Dispatch to ALL viewed panels in DFS order (root → leaves), matching
        // C++ emPanel::Input recursive broadcast. Each panel receives the event
        // with mouse coords transformed to its local space.
        let wf = self.view.window_focused();
        let viewed = tree.viewed_panels_dfs();
        for panel_id in viewed {
            let mut panel_ev = ev.clone();
            panel_ev.mouse_x = tree.view_to_panel_x(panel_id, ev.mouse_x);
            panel_ev.mouse_y = tree.view_to_panel_y(panel_id, ev.mouse_y);

            if let Some(mut behavior) = tree.take_behavior(panel_id) {
                let panel_state = tree.build_panel_state(panel_id, wf);
                let consumed = behavior.input(&panel_ev, &panel_state, state);
                // TF-003: Process scroll-to-visible requests from behaviors
                if let Some(rect) = behavior.take_scroll_to_visible() {
                    self.view.scroll_to_panel_rect(tree, panel_id, rect);
                }
                tree.put_behavior(panel_id, behavior);
                if consumed {
                    break;
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

    // ---------------------------------------------------------------
    // D-WINDOW-01a: Window state methods
    // ---------------------------------------------------------------

    /// Set window flags, applying changes to the winit window.
    ///
    /// Matches C++ emWindow::SetWindowFlags: only acts when flags differ,
    /// then updates decorations, maximized, and fullscreen state on the
    /// underlying winit window.
    pub fn set_window_flags(&mut self, new_flags: WindowFlags) {
        if self.flags == new_flags {
            return;
        }
        let old = self.flags;
        self.flags = new_flags;

        // Apply decoration changes.
        if old.contains(WindowFlags::UNDECORATED) != new_flags.contains(WindowFlags::UNDECORATED) {
            self.winit_window
                .set_decorations(!new_flags.contains(WindowFlags::UNDECORATED));
        }

        // Apply maximized changes.
        if old.contains(WindowFlags::MAXIMIZED) != new_flags.contains(WindowFlags::MAXIMIZED) {
            self.winit_window
                .set_maximized(new_flags.contains(WindowFlags::MAXIMIZED));
        }

        // Apply fullscreen changes.
        if old.contains(WindowFlags::FULLSCREEN) != new_flags.contains(WindowFlags::FULLSCREEN) {
            if new_flags.contains(WindowFlags::FULLSCREEN) {
                self.winit_window
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            } else {
                self.winit_window.set_fullscreen(None);
            }
        }
    }

    /// Set the content area (view) position.
    ///
    /// Matches C++ emWindow::SetViewPos (PSAS_VIEW pos, PSAS_IGNORE size).
    /// Winit does not distinguish between view vs window position on all
    /// platforms, so this uses inner-size-aware outer position.
    pub fn set_view_pos(&self, x: f64, y: f64) {
        self.winit_window
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    }

    /// Set the content area (view) size.
    ///
    /// Matches C++ emWindow::SetViewSize (PSAS_IGNORE pos, PSAS_VIEW size).
    pub fn set_view_size(&self, w: f64, h: f64) {
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    /// Set the content area position and size.
    ///
    /// Matches C++ emWindow::SetViewPosSize (PSAS_VIEW pos, PSAS_VIEW size).
    pub fn set_view_pos_size(&self, x: f64, y: f64, w: f64, h: f64) {
        self.winit_window
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    /// Set the window position (including decorations).
    ///
    /// Matches C++ emWindow::SetWinPos (PSAS_WINDOW pos, PSAS_IGNORE size).
    pub fn set_win_pos(&self, x: f64, y: f64) {
        self.winit_window
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    }

    /// Set the window size (including decorations).
    ///
    /// Matches C++ emWindow::SetWinSize (PSAS_IGNORE pos, PSAS_WINDOW size).
    /// Winit's `request_inner_size` sets the content area size, not the outer
    /// window size. There is no direct winit API for setting outer size, so
    /// this sets the inner size as an approximation.
    pub fn set_win_size(&self, w: f64, h: f64) {
        // Winit does not expose set_outer_size; use request_inner_size as the
        // best available approximation.
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    /// Set window position and size (including decorations).
    ///
    /// Matches C++ emWindow::SetWinPosSize (PSAS_WINDOW pos, PSAS_WINDOW size).
    pub fn set_win_pos_size(&self, x: f64, y: f64, w: f64, h: f64) {
        self.winit_window
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    /// Set window position (outer, including decorations) and view (content) size.
    ///
    /// Matches C++ emWindow::SetWinPosViewSize (PSAS_WINDOW pos, PSAS_VIEW size).
    pub fn set_win_pos_view_size(&self, x: f64, y: f64, w: f64, h: f64) {
        self.winit_window
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    // ---------------------------------------------------------------
    // D-WINDOW-01b: Monitor/DPI/close methods
    // ---------------------------------------------------------------

    /// Return the monitor index this window is on, by maximum overlap.
    ///
    /// Matches C++ emWindow::GetMonitorIndex: delegates to
    /// Screen::monitor_index_of_rect with the window's outer position
    /// and inner size.
    pub fn get_monitor_index(&self, screen: &Screen) -> Option<usize> {
        let pos = self.winit_window.outer_position().unwrap_or_default();
        let size = self.winit_window.inner_size();
        screen.monitor_index_of_rect(pos.x, pos.y, size.width, size.height)
    }

    /// Signal the close signal, triggering auto-delete if WF_AUTO_DELETE is set.
    ///
    /// Matches C++ emWindow::SignalClosing: Signal(CloseSignal).
    /// The caller (App) must call `scheduler.fire(window.close_signal)` to
    /// actually fire the signal in the scheduler. This method returns the
    /// signal ID so the caller can fire it.
    pub fn signal_closing(&self) -> SignalId {
        self.close_signal
    }

    // ---------------------------------------------------------------
    // D-WINDOW-01c: Misc window methods
    // ---------------------------------------------------------------

    /// Set the window icon from a zuicchini Image.
    ///
    /// Matches C++ emWindow::SetWindowIcon: copies the image, then applies
    /// it to the winit window. The image is converted to RGBA if needed.
    pub fn set_window_icon(&mut self, icon: &Image) {
        self.window_icon = Some(icon.clone());

        if icon.is_empty() {
            self.winit_window.set_window_icon(None);
            return;
        }

        // Convert to RGBA (4 channels) if not already.
        let rgba = if icon.channel_count() == 4 {
            icon.clone()
        } else {
            icon.get_converted(4)
        };

        if let Ok(winit_icon) =
            winit::window::Icon::from_rgba(rgba.data().to_vec(), rgba.width(), rgba.height())
        {
            self.winit_window.set_window_icon(Some(winit_icon));
        } else {
            log::error!(
                "failed to create window icon from {}x{} image",
                rgba.width(),
                rgba.height()
            );
        }
    }

    /// Get the current window icon, if set.
    pub fn window_icon(&self) -> Option<&Image> {
        self.window_icon.as_ref()
    }

    /// Inhibit the screensaver. Increments an internal counter.
    ///
    /// Matches C++ emWindowPort::InhibitScreensaver. Winit does not have a
    /// built-in screensaver inhibition API, so this only tracks the counter.
    /// TODO: Use platform-specific APIs (e.g. D-Bus on Linux, SetThreadExecutionState on Windows).
    pub fn inhibit_screensaver(&mut self) {
        self.screensaver_inhibit_count += 1;
        log::debug!(
            "screensaver inhibited (count={})",
            self.screensaver_inhibit_count
        );
    }

    /// Allow the screensaver. Decrements the internal counter.
    ///
    /// Matches C++ emWindowPort::AllowScreensaver.
    pub fn allow_screensaver(&mut self) {
        self.screensaver_inhibit_count = self.screensaver_inhibit_count.saturating_sub(1);
        log::debug!(
            "screensaver allowed (count={})",
            self.screensaver_inhibit_count
        );
    }

    /// Returns whether the screensaver is currently inhibited.
    pub fn is_screensaver_inhibited(&self) -> bool {
        self.screensaver_inhibit_count > 0
    }

    /// Move the mouse pointer by (dx, dy) pixels.
    ///
    /// Matches C++ emScreen::MoveMousePointer. Winit does not support
    /// programmatic relative mouse pointer movement on all platforms.
    /// This is a no-op stub.
    pub fn move_mouse_pointer(&self, _dx: f64, _dy: f64) {
        // TODO: Not supported by winit core on all platforms. Would need
        // platform-specific extensions (e.g. xdotool on X11, CGWarpMouseCursorPosition on macOS).
        log::debug!("move_mouse_pointer not supported by winit");
    }

    /// Emit an acoustic warning beep.
    ///
    /// Matches C++ emScreen::Beep. Winit does not provide a beep API.
    /// This is a no-op.
    pub fn beep(&self) {
        // TODO: Platform limitation — winit does not expose a beep/bell API.
        // Could use platform-specific APIs (e.g. XBell on X11, NSBeep on macOS, MessageBeep on Windows).
        log::debug!("beep not supported by winit");
    }
}
