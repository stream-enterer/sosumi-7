use std::sync::Arc;

use bitflags::bitflags;

use crate::foundation::Image;
use crate::input::{InputEvent, InputKey, InputState, InputVariant};
use crate::panel::{
    KeyboardZoomScrollVIF, MouseZoomScrollVIF, PanelId, PanelTree, View, ViewAnimator,
    ViewInputFilter,
};
use crate::render::thread_pool::RenderThreadPool;
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
/// Fixed height of the control strip in pixels.
const CONTROL_STRIP_PX: u32 = 32;

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
    pub flags_signal: SignalId,
    root_panel: PanelId,
    vif_chain: Vec<Box<dyn ViewInputFilter>>,
    pub active_animator: Option<Box<dyn ViewAnimator>>,
    window_icon: Option<Image>,
    last_mouse_pos: (f64, f64),
    screensaver_inhibit_count: u32,
    screensaver_cookie: Option<u32>,
    flags_changed: bool,
    wm_res_name: String,
    render_pool: RenderThreadPool,
    /// Separate panel tree for the control panel region.
    pub(crate) control_tree: PanelTree,
    /// View for the control panel region.
    pub(crate) control_view: View,
    /// Currently active control panel (child of control_root).
    pub(crate) control_panel_id: Option<PanelId>,
    /// Height of the control strip: 0 when hidden, CONTROL_STRIP_PX when active.
    pub(crate) control_strip_height: u32,
}

impl ZuiWindow {
    /// Create a new window with a wgpu surface and rendering pipeline.
    pub fn create(
        event_loop: &winit::event_loop::ActiveEventLoop,
        gpu: &GpuContext,
        root_panel: PanelId,
        flags: WindowFlags,
        close_signal: SignalId,
        flags_signal: SignalId,
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

        // Create control tree with a root panel
        let mut control_tree = PanelTree::new();
        let control_root = control_tree.create_root("control_root");
        control_tree.set_layout_rect(control_root, 0.0, 0.0, 1.0, 1.0);
        // Hidden initially — zero viewport height
        let control_view = View::new(control_root, w as f64, 0.0);

        let vif_chain: Vec<Box<dyn ViewInputFilter>> = vec![
            {
                let mut mouse_vif = MouseZoomScrollVIF::new();
                let zflpp = view.get_zoom_factor_log_per_pixel();
                mouse_vif.set_mouse_anim_params(1.0, 0.25, zflpp);
                mouse_vif.set_wheel_anim_params(1.0, 0.25, zflpp);
                Box::new(mouse_vif)
            },
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
            flags_signal,
            root_panel,
            vif_chain,
            active_animator: None,
            window_icon: None,
            last_mouse_pos: (0.0, 0.0),
            screensaver_inhibit_count: 0,
            screensaver_cookie: None,
            flags_changed: false,
            wm_res_name: String::from("zuicchini"),
            render_pool: RenderThreadPool::new(
                crate::model::CoreConfig::default().max_render_threads,
            ),
            control_tree,
            control_view,
            control_panel_id: None,
            control_strip_height: 0,
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
        let ch = self.content_height();
        self.view.set_viewport(tree, w as f64, ch as f64);
        if self.control_strip_height > 0 {
            self.control_view.set_viewport(
                &mut self.control_tree,
                w as f64,
                self.control_strip_height as f64,
            );
        }
    }

    /// Height available for the content viewport.
    pub(crate) fn content_height(&self) -> u32 {
        self.surface_config
            .height
            .saturating_sub(self.control_strip_height)
    }

    /// Show the control strip at the bottom of the window.
    pub(crate) fn show_control_strip(&mut self, tree: &mut PanelTree) {
        if self.control_strip_height == 0 {
            self.control_strip_height = CONTROL_STRIP_PX;
            let w = self.surface_config.width;
            let ch = self.content_height();
            self.view.set_viewport(tree, w as f64, ch as f64);
            self.control_view.set_viewport(
                &mut self.control_tree,
                w as f64,
                CONTROL_STRIP_PX as f64,
            );
            self.invalidate();
        }
    }

    /// Hide the control strip, giving all space back to content.
    pub(crate) fn hide_control_strip(&mut self, tree: &mut PanelTree) {
        if self.control_strip_height > 0 {
            self.control_strip_height = 0;
            let w = self.surface_config.width;
            let h = self.surface_config.height;
            self.view.set_viewport(tree, w as f64, h as f64);
            self.invalidate();
        }
    }

    /// Update the render thread pool from CoreConfig.
    pub fn set_max_render_threads(&mut self, max_render_threads: i32) {
        self.render_pool.update_thread_count(max_render_threads);
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
            let ctrl_height = self.control_strip_height;
            let content_h = self.content_height() as f64;
            let ctrl_root = self.control_view.root();
            let ctrl_bg = self.control_view.background_color();
            {
                let mut painter = Painter::new(&mut self.viewport_buffer);
                self.view.paint(tree, &mut painter);
                if ctrl_height > 0 {
                    self.control_view.paint_sub_tree(
                        &mut self.control_tree,
                        &mut painter,
                        ctrl_root,
                        (0.0, content_h),
                        ctrl_bg,
                    );
                }
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
        } else if self.render_pool.thread_count() > 1 && dirty_count > 1 {
            // Multi-threaded rendering via display list.
            // Phase 1: Record all draw operations single-threaded.
            self.render_parallel(tree, gpu, cols, rows, tile_size);
        } else {
            // Few dirty tiles, single-threaded: paint per-tile.
            let content_h = self.content_height();
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
                            if self.control_strip_height > 0
                                && row * tile_size + tile_size > content_h
                            {
                                let control_root = self.control_view.root();
                                let bg = self.control_view.background_color();
                                self.control_view.paint_sub_tree(
                                    &mut self.control_tree,
                                    &mut painter,
                                    control_root,
                                    (-(col as f64 * ts), -(row as f64 * ts) + content_h as f64),
                                    bg,
                                );
                            }
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

    /// Multi-threaded tile rendering using a display list.
    ///
    /// Phase 1 (single-threaded): Walk the panel tree and record all draw
    /// operations into a `DrawList` using a recording `Painter`.
    ///
    /// Phase 2 (parallel): Replay the `DrawList` into each dirty tile's
    /// buffer concurrently, with tile-specific clipping.
    ///
    /// Phase 3 (single-threaded): Upload rendered tiles to GPU.
    fn render_parallel(
        &mut self,
        tree: &mut crate::panel::PanelTree,
        gpu: &GpuContext,
        cols: u32,
        rows: u32,
        tile_size: u32,
    ) {
        use crate::foundation::Color;
        use crate::render::draw_list::DrawList;
        use crate::render::Painter;

        let vp_w = self.surface_config.width;
        let vp_h = self.surface_config.height;

        // Phase 1: Record draw operations.
        let mut draw_list = DrawList::new();
        {
            let mut painter = Painter::new_recording(vp_w, vp_h, draw_list.ops_mut());
            self.view.paint(tree, &mut painter);
            if self.control_strip_height > 0 {
                let content_h = self.content_height() as f64;
                let control_root = self.control_view.root();
                let bg = self.control_view.background_color();
                self.control_view.paint_sub_tree(
                    &mut self.control_tree,
                    &mut painter,
                    control_root,
                    (0.0, content_h),
                    bg,
                );
            }
        }

        // Collect dirty tiles.
        let mut dirty_tiles: Vec<(u32, u32)> = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                if self.tile_cache.get_or_create(col, row).dirty {
                    dirty_tiles.push((col, row));
                }
            }
        }

        if dirty_tiles.is_empty() {
            return;
        }

        // Phase 2: Parallel replay into tile buffers.
        let ts = tile_size as f64;
        let draw_list_ref = &draw_list;
        let results: Vec<std::sync::Mutex<Option<crate::foundation::Image>>> = dirty_tiles
            .iter()
            .map(|_| std::sync::Mutex::new(None))
            .collect();
        let results_ref = &results;
        let dirty_ref = &dirty_tiles;

        self.render_pool.call_parallel(
            |idx| {
                let (col, row) = dirty_ref[idx];
                let mut buffer = crate::foundation::Image::new(tile_size, tile_size, 4);
                buffer.fill(Color::BLACK);
                {
                    let mut painter = Painter::new(&mut buffer);
                    let tile_offset = (col as f64 * ts, row as f64 * ts);
                    draw_list_ref.replay(&mut painter, tile_offset);
                }
                *results_ref[idx].lock().expect("result mutex poisoned") = Some(buffer);
            },
            dirty_tiles.len(),
        );

        // Phase 3: Upload results to GPU.
        for (idx, (col, row)) in dirty_tiles.iter().enumerate() {
            if let Some(buffer) = results[idx].lock().expect("result mutex poisoned").take() {
                let tile = self.tile_cache.get_or_create(*col, *row);
                tile.image = buffer;
                tile.dirty = false;
                let tile_ref = self.tile_cache.get(*col, *row).unwrap();
                self.compositor
                    .upload_tile(&gpu.device, &gpu.queue, *col, *row, tile_ref);
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
                        NamedKey::AltGraph => Some(InputKey::AltGr),
                        NamedKey::PrintScreen => Some(InputKey::Print),
                        NamedKey::Pause => Some(InputKey::Pause),
                        NamedKey::ContextMenu => Some(InputKey::Menu),
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
                    repeat: if event.repeat { 1 } else { 0 },
                    source_variant: 0,
                    mouse_x: 0.0,
                    mouse_y: 0.0,
                    shift: false,
                    ctrl: false,
                    alt: false,
                    meta: false,
                    eaten: false,
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
                    repeat: 0,
                    source_variant: 0,
                    mouse_x: 0.0,
                    mouse_y: 0.0,
                    shift: false,
                    ctrl: false,
                    alt: false,
                    meta: false,
                    eaten: false,
                })
            }
            WindowEvent::CursorMoved { position, .. } => Some(InputEvent {
                key: InputKey::MouseLeft, // Dummy key for position-only events
                variant: InputVariant::Move,
                chars: String::new(),
                repeat: 0,
                source_variant: 0,
                mouse_x: position.x,
                mouse_y: position.y,
                shift: false,
                ctrl: false,
                alt: false,
                meta: false,
                eaten: false,
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
                        repeat: 0,
                        source_variant: 0,
                        mouse_x: dx,
                        mouse_y: dy,
                        shift: false,
                        ctrl: false,
                        alt: false,
                        meta: false,
                        eaten: false,
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
                        repeat: 0,
                        source_variant: 0,
                        mouse_x: dx,
                        mouse_y: dy,
                        shift: false,
                        ctrl: false,
                        alt: false,
                        meta: false,
                        eaten: false,
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
        // Track mouse position for cursor warping (skip wheel events).
        if !matches!(
            event.key,
            InputKey::WheelUp | InputKey::WheelDown | InputKey::WheelLeft | InputKey::WheelRight
        ) {
            self.last_mouse_pos = (event.mouse_x, event.mouse_y);
        }

        // Route to control region if mouse is below the content viewport
        let content_h = self.content_height() as f64;
        if self.control_strip_height > 0 && event.mouse_y >= content_h {
            let mut ctrl_event = event.clone();
            ctrl_event.mouse_y -= content_h;

            // For mouse press: set active panel in the CONTROL view
            if ctrl_event.variant == InputVariant::Press
                && matches!(
                    ctrl_event.key,
                    InputKey::MouseLeft | InputKey::MouseRight | InputKey::MouseMiddle
                )
            {
                let panel = self
                    .control_view
                    .get_focusable_panel_at(
                        &self.control_tree,
                        ctrl_event.mouse_x,
                        ctrl_event.mouse_y,
                    )
                    .unwrap_or_else(|| self.control_view.root());
                self.control_view
                    .set_active_panel(&mut self.control_tree, panel, false);
            }

            // Dispatch to control tree panels
            let ctrl_ev = ctrl_event.with_modifiers(state);
            let wf = self.view.window_focused();
            let viewed = self.control_tree.viewed_panels_dfs();
            for panel_id in viewed {
                let mut panel_ev = ctrl_ev.clone();
                panel_ev.mouse_x = self.control_tree.view_to_panel_x(panel_id, ctrl_ev.mouse_x);
                panel_ev.mouse_y = self.control_tree.view_to_panel_y(
                    panel_id,
                    ctrl_ev.mouse_y,
                    self.control_view.pixel_tallness(),
                );
                if let Some(mut behavior) = self.control_tree.take_behavior(panel_id) {
                    let panel_state = self.control_tree.build_panel_state(
                        panel_id,
                        wf,
                        self.control_view.pixel_tallness(),
                    );
                    let consumed = behavior.input(&panel_ev, &panel_state, state);
                    self.control_tree.put_behavior(panel_id, behavior);
                    if consumed {
                        self.control_view
                            .invalidate_painting(&self.control_tree, panel_id);
                        break;
                    }
                }
            }
            return;
        }

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
            let panel = self
                .view
                .get_focusable_panel_at(tree, event.mouse_x, event.mouse_y)
                .unwrap_or_else(|| self.view.root());
            self.view.set_active_panel(tree, panel, false);
        }

        // Stamp modifier keys from InputState onto the event
        let ev = event.clone().with_modifiers(state);

        // Dispatch to ALL viewed panels in post-order, matching C++
        // emPanel::Input recursive broadcast. Each panel receives the event
        // with mouse coords transformed to its local space.
        let trace = crate::widget::trace_input_enabled();
        let is_press_release = matches!(ev.variant, InputVariant::Press | InputVariant::Release)
            && matches!(
                ev.key,
                InputKey::MouseLeft | InputKey::MouseRight | InputKey::MouseMiddle
            );
        if trace && is_press_release {
            eprintln!(
                "[INPUT] {:?} {:?} view=({:.1},{:.1})",
                ev.key, ev.variant, ev.mouse_x, ev.mouse_y
            );
        }
        let wf = self.view.window_focused();
        let viewed = tree.viewed_panels_dfs();
        for panel_id in viewed {
            let mut panel_ev = ev.clone();
            panel_ev.mouse_x = tree.view_to_panel_x(panel_id, ev.mouse_x);
            panel_ev.mouse_y =
                tree.view_to_panel_y(panel_id, ev.mouse_y, self.view.pixel_tallness());

            if let Some(mut behavior) = tree.take_behavior(panel_id) {
                let panel_state = tree.build_panel_state(panel_id, wf, self.view.pixel_tallness());
                let consumed = behavior.input(&panel_ev, &panel_state, state);
                if trace && is_press_release {
                    let name = tree.get(panel_id).map(|p| p.name.as_str()).unwrap_or("?");
                    eprintln!(
                        "  {:?} {:?} local=({:.4},{:.4}) consumed={}",
                        panel_id, name, panel_ev.mouse_x, panel_ev.mouse_y, consumed
                    );
                }
                // TF-003: Process scroll-to-visible requests from behaviors
                if let Some(rect) = behavior.take_scroll_to_visible() {
                    self.view.scroll_to_panel_rect(tree, panel_id, rect);
                }
                tree.put_behavior(panel_id, behavior);
                if consumed {
                    if trace && is_press_release {
                        let name = tree.get(panel_id).map(|p| p.name.as_str()).unwrap_or("?");
                        eprintln!("  >>> CONSUMED by {:?}", name);
                    }
                    self.view.invalidate_painting(tree, panel_id);
                    break;
                }
            }
        }
    }

    /// Signal ID for window flags changes.
    ///
    /// Matches C++ emWindow::GetWindowFlagsSignal. Fired from
    /// `about_to_wait` when `flags_changed` is set.
    pub fn window_flags_signal(&self) -> SignalId {
        self.flags_signal
    }

    /// Whether window flags changed since the last call to
    /// `clear_flags_changed`.
    pub fn flags_changed(&self) -> bool {
        self.flags_changed
    }

    /// Reset the flags-changed latch.
    pub fn clear_flags_changed(&mut self) {
        self.flags_changed = false;
    }

    /// Window manager resource name (WM_CLASS instance on X11).
    ///
    /// Matches C++ emWindow::GetWMResName. Returns a static default;
    /// set with `set_wm_res_name`.
    pub fn wm_res_name(&self) -> &str {
        &self.wm_res_name
    }

    /// Set the window manager resource name.
    pub fn set_wm_res_name(&mut self, name: &str) {
        self.wm_res_name = name.to_string();
    }

    /// Parse an X11-style geometry string and apply position/size.
    ///
    /// Format: `WxH+X+Y` or `WxH-X-Y` (partial forms accepted).
    /// Matches C++ emWindow::SetWinPosViewSize(geometry) overload.
    pub fn set_win_pos_view_size_from_geometry(&self, geometry: &str) {
        let (w, h, x, y) = parse_x11_geometry(geometry);
        if let (Some(x), Some(y)) = (x, y) {
            self.winit_window
                .set_outer_position(winit::dpi::LogicalPosition::new(x as f64, y as f64));
        }
        if let (Some(w), Some(h)) = (w, h) {
            let _ = self
                .winit_window
                .request_inner_size(winit::dpi::LogicalSize::new(w as f64, h as f64));
        }
    }

    /// Return the window border sizes (left, top, right, bottom) in pixels.
    ///
    /// Matches C++ emWindowPort::GetBorderSizes. Winit does not expose
    /// decoration sizes directly, so this returns a reasonable default
    /// for decorated windows and zero for undecorated ones.
    pub fn border_sizes(&self) -> (i32, i32, i32, i32) {
        if self.flags.contains(WindowFlags::UNDECORATED)
            || self.flags.contains(WindowFlags::FULLSCREEN)
        {
            return (0, 0, 0, 0);
        }

        match self.winit_window.inner_position() {
            Ok(inner_pos) => {
                // X11: exact per-side sizes from position/size differences.
                let outer_pos = self.winit_window.outer_position().unwrap_or_default();
                let outer = self.winit_window.outer_size();
                let inner = self.winit_window.inner_size();
                let left = (inner_pos.x - outer_pos.x).max(0);
                let top = (inner_pos.y - outer_pos.y).max(0);
                let right = (outer.width as i32 - inner.width as i32 - left).max(0);
                let bottom = (outer.height as i32 - inner.height as i32 - top).max(0);
                (left, top, right, bottom)
            }
            Err(_) => {
                // Wayland: inner_position not supported.
                let outer = self.winit_window.outer_size();
                let inner = self.winit_window.inner_size();
                if outer == inner {
                    // SSD: compositor draws decorations outside the surface.
                    (0, 0, 0, 0)
                } else {
                    // CSD (e.g. sctk-adwaita): infer from size differences.
                    let dw = outer.width as i32 - inner.width as i32;
                    let dh = outer.height as i32 - inner.height as i32;
                    let border = (dw / 2).max(0);
                    let top = (dh - border).max(0);
                    (border, top, border, border)
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

    /// Tick VIF animations (wheel zoom spring, grip pan spring).
    /// Returns true if any animation is still active.
    pub fn tick_vif_animations(&mut self, tree: &mut PanelTree, dt: f64) -> bool {
        let view = &mut self.view;
        let mut active = false;
        for vif in &mut self.vif_chain {
            if vif.animate(view, tree, dt) {
                active = true;
            }
        }
        active
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

    /// Mark only the tiles overlapping the given pixel-coordinate rectangle as
    /// dirty. `x1`/`y1` are inclusive, `x2`/`y2` are exclusive — matching the
    /// `ClipRect` convention.
    pub fn mark_dirty_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        use crate::render::TILE_SIZE;

        let ts = TILE_SIZE as f64;
        let (cols, rows) = self.tile_cache.grid_size();

        // Clamp to viewport and convert to tile grid coordinates.
        let col_start = (x1 / ts).floor().max(0.0) as u32;
        let row_start = (y1 / ts).floor().max(0.0) as u32;
        let col_end = (x2 / ts).ceil().min(cols as f64) as u32;
        let row_end = (y2 / ts).ceil().min(rows as f64) as u32;

        for row in row_start..row_end {
            for col in col_start..col_end {
                self.tile_cache.mark_dirty(col, row);
            }
        }
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
        self.flags_changed = true;

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
    /// window size. We subtract border decoration sizes to convert outer
    /// dimensions to inner dimensions.
    pub fn set_win_size(&self, w: f64, h: f64) {
        let (left, top, right, bottom) = self.border_sizes();
        let inner_w = (w - (left + right) as f64).max(1.0);
        let inner_h = (h - (top + bottom) as f64).max(1.0);
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(inner_w, inner_h));
    }

    /// Set window position and size (including decorations).
    ///
    /// Matches C++ emWindow::SetWinPosSize (PSAS_WINDOW pos, PSAS_WINDOW size).
    pub fn set_win_pos_size(&self, x: f64, y: f64, w: f64, h: f64) {
        let (left, top, right, bottom) = self.border_sizes();
        self.winit_window
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        let inner_w = (w - (left + right) as f64).max(1.0);
        let inner_h = (h - (top + bottom) as f64).max(1.0);
        let _ = self
            .winit_window
            .request_inner_size(winit::dpi::LogicalSize::new(inner_w, inner_h));
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

    /// Inhibit the screensaver. Increments an internal counter; issues the
    /// D-Bus Inhibit call on the 0→1 transition.
    ///
    /// Matches C++ emWindowPort::InhibitScreensaver.
    pub fn inhibit_screensaver(&mut self) {
        self.screensaver_inhibit_count += 1;
        if self.screensaver_inhibit_count == 1 {
            self.screensaver_cookie = super::platform::inhibit_screensaver();
        }
        log::debug!(
            "screensaver inhibited (count={})",
            self.screensaver_inhibit_count
        );
    }

    /// Allow the screensaver. Decrements the internal counter; issues the
    /// D-Bus UnInhibit call on the 1→0 transition.
    ///
    /// Matches C++ emWindowPort::AllowScreensaver.
    pub fn allow_screensaver(&mut self) {
        self.screensaver_inhibit_count = self.screensaver_inhibit_count.saturating_sub(1);
        if self.screensaver_inhibit_count == 0 {
            if let Some(cookie) = self.screensaver_cookie.take() {
                super::platform::uninhibit_screensaver(cookie);
            }
        }
        log::debug!(
            "screensaver allowed (count={})",
            self.screensaver_inhibit_count
        );
    }

    /// Returns whether the screensaver is currently inhibited.
    pub fn is_screensaver_inhibited(&self) -> bool {
        self.screensaver_inhibit_count > 0
    }

    /// Move the mouse pointer by (dx, dy) pixels relative to last known position.
    ///
    /// Matches C++ emScreen::MoveMousePointer. Uses winit's set_cursor_position
    /// which works on X11. On Wayland, set_cursor_position returns NotSupported
    /// and the error is logged at debug level. Callers should check
    /// Screen::can_move_mouse_pointer() before calling.
    pub fn move_mouse_pointer(&self, dx: f64, dy: f64) {
        let target_x = self.last_mouse_pos.0 + dx;
        let target_y = self.last_mouse_pos.1 + dy;
        if let Err(e) = self
            .winit_window
            .set_cursor_position(winit::dpi::PhysicalPosition::new(target_x, target_y))
        {
            log::debug!("move_mouse_pointer failed: {e}");
        }
    }

    /// Emit an acoustic warning beep via libcanberra (Linux) or no-op (other).
    ///
    /// Matches C++ emScreen::Beep.
    pub fn beep(&self) {
        super::platform::system_beep();
    }
}

/// Parse an X11-style geometry string: `WxH+X+Y`, `WxH-X-Y`, `+X+Y`, `WxH`.
///
/// Returns `(width, height, x, y)` where each component is `None` if absent.
fn parse_x11_geometry(s: &str) -> (Option<i32>, Option<i32>, Option<i32>, Option<i32>) {
    let s = s.trim();
    if s.is_empty() {
        return (None, None, None, None);
    }

    let mut width = None;
    let mut height = None;
    let mut x = None;
    let mut y = None;

    // Split at the first '+' or '-' that follows a digit (size/position boundary).
    let pos_start = s
        .find(|c: char| (c == '+' || c == '-') && !s.starts_with(c))
        .unwrap_or(s.len());

    let size_part = &s[..pos_start];
    let pos_part = &s[pos_start..];

    // Parse WxH
    if !size_part.is_empty() {
        if let Some(xi) = size_part.find('x').or_else(|| size_part.find('X')) {
            width = size_part[..xi].parse::<i32>().ok();
            height = size_part[xi + 1..].parse::<i32>().ok();
        }
    }

    // Parse position: +X+Y or -X-Y or +X-Y etc.
    if !pos_part.is_empty() {
        // Collect sign+digits tokens
        let mut tokens: Vec<i32> = Vec::new();
        let mut chars = pos_part.chars().peekable();
        while chars.peek().is_some() {
            let sign = match chars.peek() {
                Some('+') => {
                    chars.next();
                    1
                }
                Some('-') => {
                    chars.next();
                    -1
                }
                _ => 1,
            };
            let mut num_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    num_str.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(n) = num_str.parse::<i32>() {
                tokens.push(sign * n);
            }
        }
        if tokens.len() >= 2 {
            x = Some(tokens[0]);
            y = Some(tokens[1]);
        } else if tokens.len() == 1 {
            x = Some(tokens[0]);
        }
    }

    (width, height, x, y)
}
