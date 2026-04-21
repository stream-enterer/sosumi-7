use std::rc::Rc;
use std::sync::Arc;

use bitflags::bitflags;

use crate::emImage::emImage;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPanelTree::{PanelId, PanelTree};
use crate::emRenderThreadPool::emRenderThreadPool;
use crate::emSignal::SignalId;
use crate::emView::emView;
use crate::emViewAnimator::emViewAnimator;
use crate::emViewInputFilter::{
    emCheatVIF, emDefaultTouchVIF, emKeyboardZoomScrollVIF, emMouseZoomScrollVIF,
    emViewInputFilter, CheatAction,
};
use crate::emViewRendererCompositor::WgpuCompositor;
use crate::emViewRendererTileCache::TileCache;

use crate::emGUIFramework::GpuContext;
use crate::emScreen::emScreen;

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

/// The OS-level surface state of an `emWindow`.
///
/// Popup windows created from `emView::RawVisitAbs` are constructed in
/// `Pending` state (no winit/wgpu objects yet) because `RawVisitAbs`
/// runs inside `emView::Update` where `&ActiveEventLoop` is unavailable.
/// The `emGUIFramework::about_to_wait` drain materializes the surface on
/// the next tick, transitioning to `Materialized`.
///
/// Non-popup windows (the home window, duplicate/ccw children) enter
/// `Materialized` directly at construction time.
pub(crate) struct PendingSurface {
    pub flags: WindowFlags,
    pub caption: String,
    pub requested_pos_size: Option<(i32, i32, i32, i32)>,
}

pub(crate) struct MaterializedSurface {
    pub winit_window: Arc<winit::window::Window>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub compositor: WgpuCompositor,
    pub tile_cache: TileCache,
    pub viewport_buffer: crate::emImage::emImage,
}

impl MaterializedSurface {
    pub(crate) fn build(gpu: &GpuContext, winit_window: Arc<winit::window::Window>) -> Self {
        let size = winit_window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);

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
        let viewport_buffer = crate::emImage::emImage::new(w, h, 4);

        Self {
            winit_window,
            surface,
            surface_config,
            compositor,
            tile_cache,
            viewport_buffer,
        }
    }
}

pub(crate) enum OsSurface {
    Pending(Box<PendingSurface>),
    Materialized(Box<MaterializedSurface>),
}

/// An eaglemode-rs window: owns a winit window, wgpu surface, compositor, tile
/// cache, and view.
pub struct emWindow {
    pub(crate) os_surface: OsSurface,
    pub view: emView,
    pub flags: WindowFlags,
    pub close_signal: SignalId,
    pub flags_signal: SignalId,
    pub focus_signal: SignalId,
    pub geometry_signal: SignalId,
    root_panel: PanelId,
    /// The panel tree owned by this emWindow. Matches C++ emView::RootPanel
    /// ownership (each emView has its own root panel). Phase 3.5.A precedent:
    /// lifts emSubViewPanel::sub_tree (emSubViewPanel.rs:23) from sub-view
    /// container to window container.
    ///
    /// Task 4: field added, constructed by every ctor via
    /// `PanelTree::default()` (an empty sentinel); not yet used.
    /// Task 6: scheduler dispatch take/put uses this field.
    /// Task 7: home window starts building its real tree here on startup.
    /// Task 8: popup path migrates to own this tree.
    pub tree: PanelTree,
    vif_chain: Vec<Box<dyn emViewInputFilter>>,
    cheat_vif: emCheatVIF,
    touch_vif: emDefaultTouchVIF,
    pub active_animator: Option<Box<dyn emViewAnimator>>,
    window_icon: Option<emImage>,
    last_mouse_pos: (f64, f64),
    screensaver_inhibit_count: u32,
    screensaver_cookie: Option<u32>,
    flags_changed: bool,
    focus_changed: bool,
    geometry_changed: bool,
    wm_res_name: String,
    render_pool: emRenderThreadPool,
}

/// Contract for methods on `emWindow` with respect to `OsSurface` state.
///
/// Most accessors and setters below assume `OsSurface::Materialized`
/// (they touch the winit window or wgpu surface). Calling them while
/// the window is still `OsSurface::Pending` will panic via
/// [`winit_window`](emWindow::winit_window).
///
/// Methods that panic on `Pending` state:
/// - `SetWindowFlags`
/// - `SetViewPos`, `SetViewSize`, `SetViewPosSize`
/// - `SetWinPos`, `SetWinSize`, `SetWinPosSize`, `SetWinPosViewSize`
/// - `set_win_pos_view_size_from_geometry`
/// - `GetBorderSizes`, `GetMonitorIndex`
/// - `Raise`, `SetRootTitle`
/// - `SetWindowIcon`
/// - `MoveMousePointer`
///
/// Methods that tolerate `Pending` state:
/// - `is_materialized`, `winit_window_if_materialized`
/// - `request_redraw` (no-op while Pending)
/// - `render`, `resize` (early-return while Pending)
/// - `invalidate`, `mark_dirty_rect`, `invalidate_rect` (no-op while Pending)
/// - Signal getters (`SignalClosing`, `GetWindowFlagsSignal`, etc.)
/// - Signal-latch accessors (`flags_changed`, `focus_changed`, `geometry_changed`)
///
/// Per-call-site audits for graceful `Pending` handling are deferred to
/// later tasks in the W3 popup-architecture sequence; this task
/// (constructor only) documents the contract without rewriting call
/// sites.
impl emWindow {
    /// Create a new window with a wgpu surface and rendering pipeline.
    ///
    /// Returns a plain `emWindow`. The caller inserts it into `App::windows`
    /// under the winit `WindowId` returned by `winit_window().id()`, then
    /// wires the view-port back-reference via `wire_viewport_window_id()`.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        event_loop: &winit::event_loop::ActiveEventLoop,
        gpu: &GpuContext,
        parent_context: Rc<crate::emContext::emContext>,
        root_panel: PanelId,
        flags: WindowFlags,
        close_signal: SignalId,
        flags_signal: SignalId,
        focus_signal: SignalId,
        geometry_signal: SignalId,
    ) -> Self {
        let mut attrs = winit::window::WindowAttributes::default().with_title("eaglemode-rs");

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
                .expect("failed to create window"),
        );

        let materialized = MaterializedSurface::build(gpu, winit_window);
        let w = materialized.surface_config.width;
        let h = materialized.surface_config.height;
        let view = emView::new(parent_context, root_panel, w as f64, h as f64);
        let max_render_threads = view.CoreConfig.borrow().GetRec().max_render_threads;

        let vif_chain: Vec<Box<dyn emViewInputFilter>> = vec![
            {
                let mut mouse_vif = emMouseZoomScrollVIF::new();
                let zflpp = view.GetZoomFactorLogarithmPerPixel();
                mouse_vif.set_mouse_anim_params(1.0, 0.25, zflpp);
                mouse_vif.set_wheel_anim_params(1.0, 0.25, zflpp);
                Box::new(mouse_vif)
            },
            Box::new(emKeyboardZoomScrollVIF::new()),
        ];

        let mut window = Self {
            os_surface: OsSurface::Materialized(Box::new(materialized)),
            view,
            flags,
            close_signal,
            flags_signal,
            focus_signal,
            geometry_signal,
            root_panel,
            tree: PanelTree::default(),
            vif_chain,
            cheat_vif: emCheatVIF::new(),
            touch_vif: emDefaultTouchVIF::new(),
            active_animator: None,
            window_icon: None,
            last_mouse_pos: (0.0, 0.0),
            screensaver_inhibit_count: 0,
            screensaver_cookie: None,
            flags_changed: false,
            focus_changed: false,
            geometry_changed: false,
            wm_res_name: String::from("eaglemode-rs"),
            render_pool: emRenderThreadPool::new(max_render_threads),
        };

        // Wire the emViewPort back-reference (Phase 6 / Phase-5 absorbed work;
        // Phase-2 port-ownership-rewrite: WindowId instead of Weak). The
        // current view-port (initially the home port) stores the owning
        // window's WindowId so PaintView and InvalidatePainting can dispatch
        // to backend machinery by resolving through `EngineCtx::windows`.
        // Matches the C++ emWindowPort constructor which stores &Window on
        // the port.
        let window_id = window.winit_window().id();
        {
            let vp = window.view.CurrentViewPort.clone();
            vp.borrow_mut().window_id = Some(window_id);
        }

        // Phase 9 (emview-rewrite-followups): seed the view's max_popup_rect
        // from the owning monitor's bounds so popup geometry uses real
        // screen dimensions. `current_monitor()` may return None on Wayland
        // without position queries; in that case the home-rect fallback in
        // `GetMaxPopupViewRect` applies.
        if let Some(monitor) = window.winit_window().current_monitor() {
            let pos = monitor.position();
            let size = monitor.size();
            window
                .view
                .set_max_popup_rect(Some(crate::emPanel::Rect::new(
                    pos.x as f64,
                    pos.y as f64,
                    size.width as f64,
                    size.height as f64,
                )));
        }

        window
    }

    /// Port of C++ `emWindow` ctor with `WF_POPUP`. Creates an undecorated,
    /// always-on-top popup window sharing the scheduler context with the
    /// owner.
    #[allow(clippy::too_many_arguments)]
    pub fn new_popup(
        event_loop: &winit::event_loop::ActiveEventLoop,
        gpu: &GpuContext,
        parent_context: Rc<crate::emContext::emContext>,
        root_panel: PanelId,
        close_signal: SignalId,
        flags_signal: SignalId,
        focus_signal: SignalId,
        geometry_signal: SignalId,
    ) -> Self {
        Self::create(
            event_loop,
            gpu,
            parent_context,
            root_panel,
            WindowFlags::POPUP | WindowFlags::UNDECORATED | WindowFlags::AUTO_DELETE,
            close_signal,
            flags_signal,
            focus_signal,
            geometry_signal,
        )
    }

    /// Construct an `emWindow` in `Pending` state — no winit/wgpu objects
    /// yet. Callable from any context (does NOT require `&ActiveEventLoop`
    /// or `&GpuContext`). The OS surface is materialized later by the
    /// `emGUIFramework::about_to_wait` drain that consumes pending popup
    /// windows.
    ///
    /// Mirrors the first side-effects of C++ `emView::RawVisitAbs`
    /// popup-entry (`emView.cpp:1636-1643`) at the struct level: the
    /// `emWindow` object exists and every emCore observer sees a
    /// fully-wired popup immediately, even before the OS window is
    /// created.
    #[allow(clippy::too_many_arguments)]
    pub fn new_popup_pending(
        parent_context: Rc<crate::emContext::emContext>,
        root_panel: PanelId,
        flags: WindowFlags,
        caption: String,
        close_signal: SignalId,
        flags_signal: SignalId,
        focus_signal: SignalId,
        geometry_signal: SignalId,
        background_color: crate::emColor::emColor,
    ) -> Self {
        // Placeholder geometry — real position/size lands via
        // `SetViewPosSize` before materialization.
        let mut view = emView::new(parent_context, root_panel, 1.0, 1.0);
        view.SetBackgroundColor(background_color);
        let max_render_threads = view.CoreConfig.borrow().GetRec().max_render_threads;

        let vif_chain: Vec<Box<dyn emViewInputFilter>> = vec![
            {
                let mut mouse_vif = emMouseZoomScrollVIF::new();
                let zflpp = view.GetZoomFactorLogarithmPerPixel();
                mouse_vif.set_mouse_anim_params(1.0, 0.25, zflpp);
                mouse_vif.set_wheel_anim_params(1.0, 0.25, zflpp);
                Box::new(mouse_vif)
            },
            Box::new(emKeyboardZoomScrollVIF::new()),
        ];

        Self {
            os_surface: OsSurface::Pending(Box::new(PendingSurface {
                flags,
                caption,
                requested_pos_size: None,
            })),
            view,
            flags,
            close_signal,
            flags_signal,
            focus_signal,
            geometry_signal,
            root_panel,
            tree: PanelTree::default(),
            vif_chain,
            cheat_vif: emCheatVIF::new(),
            touch_vif: emDefaultTouchVIF::new(),
            active_animator: None,
            window_icon: None,
            last_mouse_pos: (0.0, 0.0),
            screensaver_inhibit_count: 0,
            screensaver_cookie: None,
            flags_changed: false,
            focus_changed: false,
            geometry_changed: false,
            wm_res_name: String::from("eaglemode-rs"),
            render_pool: emRenderThreadPool::new(max_render_threads),
        }
        // NOTE: `emViewPort::window_id` is wired once the popup acquires a
        // winit WindowId — i.e. on materialization (`materialize_popup_surface`).
        // Until then `window_id` remains `None`; PaintView / InvalidatePainting
        // are no-ops, which matches the "Pending state is observable but has
        // no OS surface" contract.
    }

    /// Set the `window_id` back-reference on the current view-port. Called
    /// by the framework once a pending popup's OS surface has been
    /// materialized and its winit WindowId is known.
    pub(crate) fn wire_viewport_window_id(&self, window_id: winit::window::WindowId) {
        let vp = self.view.CurrentViewPort.clone();
        vp.borrow_mut().window_id = Some(window_id);
    }

    // DIVERGED: Plan listed `materialized()`/`materialized_mut()` returning 6-tuple borrows; inlined as match in callers because the multi-borrow signature is awkward in Rust.
    /// Public accessor for the materialized winit window.
    ///
    /// Panics if the window is still in `OsSurface::Pending`. External
    /// code that needs to tolerate `Pending` should call
    /// [`winit_window_if_materialized`](Self::winit_window_if_materialized).
    pub fn winit_window(&self) -> &Arc<winit::window::Window> {
        match &self.os_surface {
            OsSurface::Materialized(m) => &m.winit_window,
            OsSurface::Pending(_) => panic!("emWindow::winit_window() called while Pending"),
        }
    }

    pub fn is_materialized(&self) -> bool {
        matches!(self.os_surface, OsSurface::Materialized(_))
    }

    pub(crate) fn winit_window_if_materialized(&self) -> Option<&Arc<winit::window::Window>> {
        match &self.os_surface {
            OsSurface::Materialized(m) => Some(&m.winit_window),
            OsSurface::Pending(_) => None,
        }
    }

    /// Handle a resize event.
    pub fn resize(
        &mut self,
        gpu: &GpuContext,
        tree: &mut crate::emPanelTree::PanelTree,
        width: u32,
        height: u32,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        let w = width.max(1);
        let h = height.max(1);
        match &mut self.os_surface {
            OsSurface::Materialized(m) => {
                m.surface_config.width = w;
                m.surface_config.height = h;
                m.surface.configure(&gpu.device, &m.surface_config);
                m.compositor.resize(w, h);
                m.tile_cache.resize(w, h);
                m.viewport_buffer.setup(w, h, 4);
            }
            OsSurface::Pending(_) => return,
        }
        self.view
            .SetGeometry(tree, 0.0, 0.0, w as f64, h as f64, 1.0, ctx);
    }

    /// Update the render thread pool from emCoreConfig.
    pub fn set_max_render_threads(&mut self, max_render_threads: i32) {
        self.render_pool.UpdateThreadCount(max_render_threads);
    }

    /// Render a frame: paint dirty tiles on CPU, upload to GPU, composite.
    pub fn render(&mut self, tree: &mut crate::emPanelTree::PanelTree, gpu: &GpuContext) {
        use crate::emPainter::emPainter;

        let (winit_window, surface, surface_config, compositor, tile_cache, viewport_buffer) =
            match &mut self.os_surface {
                OsSurface::Materialized(m) => {
                    let MaterializedSurface {
                        winit_window,
                        surface,
                        surface_config,
                        compositor,
                        tile_cache,
                        viewport_buffer,
                    } = m.as_mut();
                    (
                        winit_window,
                        surface,
                        surface_config,
                        compositor,
                        tile_cache,
                        viewport_buffer,
                    )
                }
                OsSurface::Pending(_) => return,
            };
        let view = &mut self.view;

        // Phase 5 (emview-rewrite-followups): consume cursor-dirty flag set
        // by emViewPort::InvalidateCursor and apply the cached cursor to
        // the winit window. Matches the C++ emWindowPort frame prologue.
        {
            let vp = view.CurrentViewPort.clone();
            let dirty = vp.borrow().cursor_dirty;
            if dirty {
                let cursor = vp.borrow().cursor;
                winit_window.set_cursor(cursor.to_winit_cursor());
                vp.borrow_mut().cursor_dirty = false;
            }
        }

        let (cols, rows) = tile_cache.grid_size();
        let tile_size = crate::emViewRendererTileCache::TILE_SIZE;

        // Count dirty tiles to choose rendering strategy.
        let mut dirty_count = 0u32;
        for row in 0..rows {
            for col in 0..cols {
                if tile_cache.get_or_create(col, row).dirty {
                    dirty_count += 1;
                }
            }
        }

        if dirty_count > cols * rows / 2 {
            // Many dirty tiles (e.g. panning): paint into viewport-sized buffer
            // once, then copy tile-sized chunks. Avoids redundant tree walks and
            // re-rasterization of primitives across tiles.
            viewport_buffer.fill(crate::emColor::emColor::BLACK);
            {
                let mut painter = emPainter::new(viewport_buffer);
                view.Paint(tree, &mut painter, crate::emColor::emColor::TRANSPARENT);
            }
            for row in 0..rows {
                for col in 0..cols {
                    let tile = tile_cache.get_or_create(col, row);
                    if tile.dirty {
                        tile.image.copy_from_rect(
                            0,
                            0,
                            viewport_buffer,
                            (col * tile_size, row * tile_size, tile_size, tile_size),
                        );
                        tile.dirty = false;
                        let tile_ref = tile_cache.GetRec(col, row).unwrap();
                        compositor.upload_tile(&gpu.device, &gpu.queue, col, row, tile_ref);
                    }
                }
            }
        } else if self.render_pool.GetThreadCount() > 1 && dirty_count > 1 {
            // Multi-threaded rendering via display list.
            // Phase 1: Record all draw operations single-threaded.
            Self::render_parallel_inner(
                view,
                tile_cache,
                compositor,
                surface_config,
                &mut self.render_pool,
                tree,
                gpu,
                cols,
                rows,
                tile_size,
            );
        } else {
            // Few dirty tiles, single-threaded: paint per-tile.
            for row in 0..rows {
                for col in 0..cols {
                    let tile = tile_cache.get_or_create(col, row);
                    if tile.dirty {
                        tile.image.fill(crate::emColor::emColor::BLACK);
                        {
                            let mut painter = emPainter::new(&mut tile.image);
                            let ts = tile_size as f64;
                            painter.translate(-(col as f64 * ts), -(row as f64 * ts));
                            view.Paint(tree, &mut painter, crate::emColor::emColor::TRANSPARENT);
                        }
                        tile.dirty = false;
                        let tile_ref = tile_cache.GetRec(col, row).unwrap();
                        compositor.upload_tile(&gpu.device, &gpu.queue, col, row, tile_ref);
                    }
                }
            }
        }

        tile_cache.advance_frame();

        // Composite and present
        match compositor.render_frame(&gpu.device, &gpu.queue, surface, surface_config) {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&gpu.device, surface_config);
            }
            Err(e) => {
                log::error!("render error: {e}");
            }
        }
    }

    /// Multi-threaded tile rendering using a display list.
    ///
    /// Phase 1 (single-threaded): Walk the panel tree and record all draw
    /// operations into a `DrawList` using a recording `emPainter`.
    ///
    /// Phase 2 (parallel): Replay the `DrawList` into each dirty tile's
    /// buffer concurrently, with tile-specific clipping.
    ///
    /// Phase 3 (single-threaded): Upload rendered tiles to GPU.
    #[allow(clippy::too_many_arguments)]
    fn render_parallel_inner(
        view: &mut emView,
        tile_cache: &mut TileCache,
        compositor: &mut WgpuCompositor,
        surface_config: &wgpu::SurfaceConfiguration,
        render_pool: &mut emRenderThreadPool,
        tree: &mut crate::emPanelTree::PanelTree,
        gpu: &GpuContext,
        cols: u32,
        rows: u32,
        tile_size: u32,
    ) {
        use crate::emColor::emColor;
        use crate::emPainter::emPainter;
        use crate::emPainterDrawList::DrawList;

        let vp_w = surface_config.width;
        let vp_h = surface_config.height;

        // Phase 1: Record draw operations.
        let mut draw_list = DrawList::new();
        {
            let mut painter = emPainter::new_recording(vp_w, vp_h, draw_list.ops_mut());
            view.Paint(tree, &mut painter, emColor::TRANSPARENT);
        }

        // Collect dirty tiles.
        let mut dirty_tiles: Vec<(u32, u32)> = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                if tile_cache.get_or_create(col, row).dirty {
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
        let results: Vec<std::sync::Mutex<Option<crate::emImage::emImage>>> = dirty_tiles
            .iter()
            .map(|_| std::sync::Mutex::new(None))
            .collect();
        let results_ref = &results;
        let dirty_ref = &dirty_tiles;

        render_pool.CallParallel(
            |idx| {
                let (col, row) = dirty_ref[idx];
                let mut buffer = crate::emImage::emImage::new(tile_size, tile_size, 4);
                buffer.fill(emColor::BLACK);
                {
                    let mut painter = emPainter::new(&mut buffer);
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
                let tile = tile_cache.get_or_create(*col, *row);
                tile.image = buffer;
                tile.dirty = false;
                let tile_ref = tile_cache.GetRec(*col, *row).unwrap();
                compositor.upload_tile(&gpu.device, &gpu.queue, *col, *row, tile_ref);
            }
        }
    }

    /// Translate a winit window event to an eaglemode-rs emInputEvent.
    pub fn handle_input(event: &winit::event::WindowEvent) -> Option<emInputEvent> {
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
                let mut input_event = emInputEvent {
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
                Some(emInputEvent {
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
            WindowEvent::CursorMoved { position, .. } => Some(emInputEvent {
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
                    Some(emInputEvent {
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
                    Some(emInputEvent {
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
    pub fn dispatch_input(
        &mut self,
        tree: &mut PanelTree,
        event: &emInputEvent,
        state: &mut emInputState,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) {
        // Track mouse position for cursor warping (skip wheel events).
        if !matches!(
            event.key,
            InputKey::WheelUp | InputKey::WheelDown | InputKey::WheelLeft | InputKey::WheelRight
        ) {
            self.last_mouse_pos = (event.mouse_x, event.mouse_y);
        }

        // C++ emView.cpp:1004: forward input to ActiveAnimator first. The
        // animator slot lives here (not on `emView`) per the Rust
        // structural divergence documented on `emView::Input`. A `visiting`
        // animator may eat the event, in which case the VIF chain and
        // panel broadcast below see an empty event.
        let mut event = event.clone();
        if let Some(mut anim) = self.active_animator.take() {
            let was_active = anim.is_active();
            emViewAnimator::Input(anim.as_mut(), &mut event, state);
            let deactivated = was_active && !anim.is_active();
            if anim.is_active() {
                self.active_animator = Some(anim);
            }
            // else: animator self-dropped.
            if deactivated {
                // C++ emViewAnimator.cpp:1060: clear seek-pos so the next notice
                // cycle doesn't fire SOUGHT_NAME_CHANGED on a stale target.
                self.view.SetSeekPos(tree, None, "");
                // C++ emViewAnimator.cpp:1061: whole-view InvalidatePainting() skipped.
                // Rust has no emViewAnimator::InvalidatePainting method; the visiting
                // overlay will repaint correctly on the next scheduled paint cycle.
            }
        }

        // Phase 5 (emview-rewrite-followups): route through emViewPort.
        // This stamps the input clock and invokes emView::Input for the
        // C++ prologue bookkeeping (LastMouseX/Y, CursorInvalid). The
        // VIF-chain + panel broadcast below remain the actual dispatch
        // mechanism; Phases 6/8 migrate that into emView::Input proper.
        {
            let vp = self.view.CurrentViewPort.clone();
            let mut vp = vp.borrow_mut();
            vp.input_clock_ms = crate::emScheduler::emGetClockMS();
            vp.InputToView(&mut self.view, tree, &event, state, ctx);
        }

        // Run VIF chain
        for vif in &mut self.vif_chain {
            if vif.filter(&event, state, &mut self.view, tree, ctx) {
                return;
            }
        }

        // Drain pending mouse warp from stick-mouse-when-navigating
        if let Some(mouse_vif) = self
            .vif_chain
            .first_mut()
            .and_then(|v| v.as_any_mut().downcast_mut::<emMouseZoomScrollVIF>())
        {
            let (wx, wy) = mouse_vif.drain_pending_warp();
            if wx.abs() > 0.1 || wy.abs() > 0.1 {
                self.MoveMousePointer(wx, wy);
                // Adjust emInputState to match new cursor position
                state.mouse_x += wx;
                state.mouse_y += wy;
            }
        }

        // Run cheat VIF (never consumes events, but may produce actions)
        self.cheat_vif
            .filter(&event, state, &mut self.view, tree, ctx);
        for action in self.cheat_vif.drain_actions() {
            match action {
                CheatAction::PanFunction
                | CheatAction::EmulateMiddleButton
                | CheatAction::StickMouseWhenNavigating => {
                    if let Some(mouse_vif) = self
                        .vif_chain
                        .first_mut()
                        .and_then(|v| v.as_any_mut().downcast_mut::<emMouseZoomScrollVIF>())
                    {
                        match action {
                            CheatAction::PanFunction => {
                                let current = mouse_vif.pan_function();
                                mouse_vif.set_pan_function(!current);
                            }
                            CheatAction::EmulateMiddleButton => {
                                let current = mouse_vif.emulate_middle_button();
                                mouse_vif.set_emulate_middle_button(!current);
                            }
                            CheatAction::StickMouseWhenNavigating => {
                                let current = mouse_vif.stick_mouse();
                                mouse_vif.set_stick_mouse(!current);
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                CheatAction::TreeDump => {
                    self.view.dump_tree(tree);
                }
                CheatAction::Screenshot => {
                    take_screenshot();
                }
            }
        }

        // Tab / Shift+Tab focus cycling (C++ emPanel.cpp FocusNext/FocusPrev).
        // User-nav gate: C++ `emView::Visit*` nav methods do not gate internally
        // (see comment above the nav block in emView.rs); the user-nav callers
        // gate on `NO_USER_NAVIGATION`.
        if event.key == InputKey::Tab && event.variant == InputVariant::Press {
            if !self
                .view
                .flags
                .contains(crate::emView::ViewFlags::NO_USER_NAVIGATION)
            {
                if state.GetShift() {
                    self.view.VisitPrev(tree);
                } else {
                    self.view.VisitNext(tree);
                }
            }
            return;
        }

        // For mouse press: hit test and set active panel
        if event.variant == InputVariant::Press
            && matches!(
                event.key,
                InputKey::MouseLeft | InputKey::MouseRight | InputKey::MouseMiddle
            )
        {
            let panel = {
                let v = &self.view;
                v.GetFocusablePanelAt(tree, event.mouse_x, event.mouse_y)
                    .unwrap_or_else(|| v.GetRootPanel())
            };
            self.view.set_active_panel(tree, panel, false, ctx);
        }

        // Stamp modifier keys from emInputState onto the event
        let ev = event.clone().with_modifiers(state);

        // Dispatch to ALL viewed panels in post-order, matching C++
        // emPanel::Input recursive broadcast. Each panel receives the event
        // with mouse coords transformed to its local space.
        // RUST_ONLY: widget_utils.rs -- debug trace aid, no C++ equivalent
        let trace = {
            use std::sync::OnceLock;
            static ENABLED: OnceLock<bool> = OnceLock::new();
            *ENABLED.get_or_init(|| std::env::var("TRACE_INPUT").is_ok())
        };
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
        let wf = self.view.IsFocused();
        let viewed = tree.viewed_panels_dfs();
        let mut consumed = false;
        for panel_id in viewed {
            let mut panel_ev = ev.clone();
            panel_ev.mouse_x = tree.ViewToPanelX(panel_id, ev.mouse_x);
            panel_ev.mouse_y =
                tree.ViewToPanelY(panel_id, ev.mouse_y, self.view.GetCurrentPixelTallness());

            if let Some(mut behavior) = tree.take_behavior(panel_id) {
                let panel_state =
                    tree.build_panel_state(panel_id, wf, self.view.GetCurrentPixelTallness());
                // C++ RecurseInput (emView.cpp:2055-2058): keyboard events are
                // suppressed for panels not in the active path.
                if panel_ev.is_keyboard_event() && !panel_state.in_active_path {
                    tree.put_behavior(panel_id, behavior);
                    continue;
                }
                // Phase 1.76 Task 2: construct a fresh per-panel PanelCtx
                // carrying the outer dispatch scheduler so wakes emitted by
                // `behavior.Input` (including sub-view `set_active_panel` /
                // `Update` via emSubViewPanel) propagate to the real scheduler.
                consumed = {
                    let pixel_tallness = self.view.GetCurrentPixelTallness();
                    let mut panel_ctx = crate::emEngineCtx::PanelCtx::with_sched_reach(
                        tree,
                        panel_id,
                        pixel_tallness,
                        ctx.scheduler,
                        ctx.framework_actions,
                        ctx.root_context,
                        ctx.framework_clipboard,
                    );
                    behavior.Input(&panel_ev, &panel_state, state, &mut panel_ctx)
                };
                if trace && is_press_release {
                    let name = tree
                        .GetRec(panel_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or("?");
                    eprintln!(
                        "  {:?} {:?} local=({:.4},{:.4}) consumed={}",
                        panel_id, name, panel_ev.mouse_x, panel_ev.mouse_y, consumed
                    );
                }
                // TF-003: emProcess scroll-to-visible requests from behaviors
                if let Some(rect) = behavior.take_scroll_to_visible() {
                    self.view.scroll_to_panel_rect(tree, panel_id, rect, ctx);
                }
                tree.put_behavior(panel_id, behavior);
                if consumed {
                    if trace && is_press_release {
                        let name = tree
                            .GetRec(panel_id)
                            .map(|p| p.name.as_str())
                            .unwrap_or("?");
                        eprintln!("  >>> CONSUMED by {:?}", name);
                    }
                    self.view.InvalidatePainting(tree, panel_id);
                    break;
                }
            }
        }

        // Arrow key sibling navigation and Home/End/PageUp/PageDown.
        // (C++ emPanel.cpp:1168-1198 routes these via emPanel::Input fallback;
        // Rust routes via this post-behavior block for architectural consistency
        // with the existing arrow-key arrangement.)
        // Only fires if no behavior consumed the event.
        // User-nav gate: C++ `emView::Visit*` nav methods do not gate internally;
        // the user-nav caller (this keybinding block) gates on `NO_USER_NAVIGATION`.
        let user_nav_blocked = self
            .view
            .flags
            .contains(crate::emView::ViewFlags::NO_USER_NAVIGATION);
        if !consumed && !user_nav_blocked && event.variant == InputVariant::Press {
            match event.key {
                InputKey::ArrowLeft if state.IsNoMod() => self.view.VisitLeft(tree),
                InputKey::ArrowRight if state.IsNoMod() => self.view.VisitRight(tree),
                InputKey::ArrowUp if state.IsNoMod() => self.view.VisitUp(tree),
                InputKey::ArrowDown if state.IsNoMod() => self.view.VisitDown(tree),

                // C++ emPanel.cpp:1168-1180: Home with modifier variants.
                InputKey::Home if state.IsNoMod() => self.view.VisitFirst(tree),
                InputKey::Home if state.IsAltMod() => {
                    if let Some(p) = self.view.GetActivePanel() {
                        let adherent = self.view.IsActivationAdherent();
                        self.view.VisitFullsized(tree, p, adherent, false);
                    }
                }
                InputKey::Home if state.IsShiftAltMod() => {
                    if let Some(p) = self.view.GetActivePanel() {
                        let adherent = self.view.IsActivationAdherent();
                        self.view.VisitFullsized(tree, p, adherent, true);
                    }
                }

                // C++ emPanel.cpp:1182-1198
                InputKey::End if state.IsNoMod() => self.view.VisitLast(tree),
                InputKey::PageUp if state.IsNoMod() => self.view.VisitOut(tree),
                InputKey::PageDown if state.IsNoMod() => self.view.VisitIn(tree),

                _ => {}
            }
        }
    }

    /// Signal ID for window flags changes.
    ///
    /// Matches C++ emWindow::GetWindowFlagsSignal. Fired from
    /// `about_to_wait` when `flags_changed` is set.
    pub fn GetWindowFlagsSignal(&self) -> SignalId {
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

    /// Signal ID for focus changes.
    ///
    /// Matches C++ emWindow::GetFocusSignal. Fired from
    /// `about_to_wait` when `focus_changed` is set.
    pub fn GetFocusSignal(&self) -> SignalId {
        self.focus_signal
    }

    /// Whether focus changed since the last call to `clear_focus_changed`.
    pub fn focus_changed(&self) -> bool {
        self.focus_changed
    }

    /// Reset the focus-changed latch.
    pub fn clear_focus_changed(&mut self) {
        self.focus_changed = false;
    }

    /// Mark focus as changed (called from the event loop on Focused event).
    pub fn set_focus_changed(&mut self) {
        self.focus_changed = true;
    }

    /// Signal ID for geometry changes.
    ///
    /// Matches C++ emWindow::GetGeometrySignal. Fired from
    /// `about_to_wait` when `geometry_changed` is set.
    pub fn GetGeometrySignal(&self) -> SignalId {
        self.geometry_signal
    }

    /// Whether geometry changed since the last call to `clear_geometry_changed`.
    pub fn geometry_changed(&self) -> bool {
        self.geometry_changed
    }

    /// Reset the geometry-changed latch.
    pub fn clear_geometry_changed(&mut self) {
        self.geometry_changed = false;
    }

    /// Mark geometry as changed (called from the event loop on Resized/Moved).
    pub fn set_geometry_changed(&mut self) {
        self.geometry_changed = true;
    }

    /// Window manager resource name (WM_CLASS instance on X11).
    ///
    /// Matches C++ emWindow::GetWMResName. Returns a static default;
    /// set with `set_wm_res_name`.
    pub fn GetWMResName(&self) -> &str {
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
            self.winit_window()
                .set_outer_position(winit::dpi::LogicalPosition::new(x as f64, y as f64));
        }
        if let (Some(w), Some(h)) = (w, h) {
            let _ = self
                .winit_window()
                .request_inner_size(winit::dpi::LogicalSize::new(w as f64, h as f64));
        }
    }

    /// Return the window border sizes (left, top, right, bottom) in pixels.
    ///
    /// Matches C++ emWindowPort::GetBorderSizes. Winit does not expose
    /// decoration sizes directly, so this returns a reasonable default
    /// for decorated windows and zero for undecorated ones.
    pub fn GetBorderSizes(&self) -> (i32, i32, i32, i32) {
        if self.flags.contains(WindowFlags::UNDECORATED)
            || self.flags.contains(WindowFlags::FULLSCREEN)
        {
            return (0, 0, 0, 0);
        }

        match self.winit_window().inner_position() {
            Ok(inner_pos) => {
                // X11: exact per-side sizes from position/size differences.
                let outer_pos = self.winit_window().outer_position().unwrap_or_default();
                let outer = self.winit_window().outer_size();
                let inner = self.winit_window().inner_size();
                let left = (inner_pos.x - outer_pos.x).max(0);
                let top = (inner_pos.y - outer_pos.y).max(0);
                let right = (outer.width as i32 - inner.width as i32 - left).max(0);
                let bottom = (outer.height as i32 - inner.height as i32 - top).max(0);
                (left, top, right, bottom)
            }
            Err(_) => {
                // Wayland: inner_position not supported.
                let outer = self.winit_window().outer_size();
                let inner = self.winit_window().inner_size();
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

    pub fn view(&self) -> &emView {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut emView {
        &mut self.view
    }

    /// Tick VIF animations (wheel zoom spring, grip pan spring).
    /// Returns true if any animation is still active.
    pub fn tick_vif_animations(
        &mut self,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        let mut active = false;
        for vif in &mut self.vif_chain {
            if vif.animate(&mut self.view, tree, dt, ctx) {
                active = true;
            }
        }
        // Tick touch gesture timer (C++ emDefaultTouchVIF::Cycle)
        let dt_ms = (dt * 1000.0) as i32;
        self.touch_vif
            .cycle_gesture(&mut self.view, tree, dt_ms, ctx);
        // Tick fling animation
        if self.touch_vif.animate_fling(&mut self.view, tree, dt, ctx) {
            active = true;
        }
        active
    }

    /// Handle a winit Touch event by routing to the emDefaultTouchVIF.
    /// Returns true if the event was consumed.
    pub fn handle_touch(
        &mut self,
        touch: &winit::event::Touch,
        tree: &mut PanelTree,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        use winit::event::TouchPhase;
        match touch.phase {
            TouchPhase::Started => self.touch_vif.touch_start(
                touch.id,
                touch.location.x,
                touch.location.y,
                &mut self.view,
                tree,
                ctx,
            ),
            TouchPhase::Moved => {
                // dt=0.016 is a reasonable default; the real frame delta is
                // applied in cycle_gesture which runs each frame.
                self.touch_vif.touch_move(
                    touch.id,
                    touch.location.x,
                    touch.location.y,
                    0.016,
                    &mut self.view,
                    tree,
                    ctx,
                )
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.touch_vif
                    .touch_end(touch.id, &mut self.view, tree, ctx)
            }
        }
    }

    pub fn touch_vif_mut(&mut self) -> &mut emDefaultTouchVIF {
        &mut self.touch_vif
    }

    pub fn root_panel(&self) -> PanelId {
        self.root_panel
    }

    /// Take the panel tree out of this window, leaving an empty sentinel
    /// behind. Used exclusively by the scheduler's per-window dispatch
    /// (Phase 3.5.A Task 6) to let engine Cycles access the tree without
    /// aliasing `ctx.windows`. Callers outside the scheduler MUST pair this
    /// with a `put_tree` call before returning control to App code.
    ///
    /// Invariant: between `take_tree` and `put_tree`, no code reads
    /// `self.tree` on this window. Mirrors the `tree.take_behavior` /
    /// `tree.put_behavior` invariant already used for SubView dispatch
    /// (emScheduler.rs:138-169).
    pub fn take_tree(&mut self) -> PanelTree {
        std::mem::take(&mut self.tree)
    }

    /// Restore a panel tree previously taken via `take_tree`.
    pub fn put_tree(&mut self, tree: PanelTree) {
        self.tree = tree;
    }

    pub fn request_redraw(&self) {
        if let Some(w) = self.winit_window_if_materialized() {
            w.request_redraw();
        }
    }

    /// Request the window manager to bring this window to front.
    pub fn Raise(&self) {
        self.winit_window().focus_window();
    }

    /// Set the window title.
    pub fn SetRootTitle(&self, title: &str) {
        self.winit_window().set_title(title);
    }

    /// Mark all tiles as dirty so the next render repaints everything.
    pub fn invalidate(&mut self) {
        if let OsSurface::Materialized(m) = &mut self.os_surface {
            m.tile_cache.mark_all_dirty();
        }
    }

    /// Invalidate a pixel-coordinate rectangle `(x, y, w, h)` in the tile
    /// cache. Entry point for `emViewPort::InvalidatePainting`.
    ///
    /// Matches C++ `emWindowPort::InvalidatePainting`: forwards a dirty rect
    /// from the view to the backend compositor (the tile cache, here).
    pub fn invalidate_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.mark_dirty_rect(x, y, x + w, y + h);
    }

    /// Mark only the tiles overlapping the given pixel-coordinate rectangle as
    /// dirty. `x1`/`y1` are inclusive, `x2`/`y2` are exclusive — matching the
    /// `ClipRect` convention.
    pub fn mark_dirty_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        use crate::emViewRendererTileCache::TILE_SIZE;

        let tile_cache = match &mut self.os_surface {
            OsSurface::Materialized(m) => &mut m.tile_cache,
            OsSurface::Pending(_) => return,
        };

        let ts = TILE_SIZE as f64;
        let (cols, rows) = tile_cache.grid_size();

        // Clamp to viewport and convert to tile grid coordinates.
        let col_start = (x1 / ts).floor().max(0.0) as u32;
        let row_start = (y1 / ts).floor().max(0.0) as u32;
        let col_end = (x2 / ts).ceil().min(cols as f64) as u32;
        let row_end = (y2 / ts).ceil().min(rows as f64) as u32;

        for row in row_start..row_end {
            for col in col_start..col_end {
                tile_cache.mark_dirty(col, row);
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
    pub fn SetWindowFlags(&mut self, new_flags: WindowFlags) {
        if self.flags == new_flags {
            return;
        }
        let old = self.flags;
        self.flags = new_flags;
        self.flags_changed = true;

        // Apply decoration changes.
        if old.contains(WindowFlags::UNDECORATED) != new_flags.contains(WindowFlags::UNDECORATED) {
            self.winit_window()
                .set_decorations(!new_flags.contains(WindowFlags::UNDECORATED));
        }

        // Apply maximized changes.
        if old.contains(WindowFlags::MAXIMIZED) != new_flags.contains(WindowFlags::MAXIMIZED) {
            self.winit_window()
                .set_maximized(new_flags.contains(WindowFlags::MAXIMIZED));
        }

        // Apply fullscreen changes.
        if old.contains(WindowFlags::FULLSCREEN) != new_flags.contains(WindowFlags::FULLSCREEN) {
            if new_flags.contains(WindowFlags::FULLSCREEN) {
                self.winit_window()
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            } else {
                self.winit_window().set_fullscreen(None);
            }
        }
    }

    /// Set the content area (view) position.
    ///
    /// Matches C++ emWindow::SetViewPos (PSAS_VIEW pos, PSAS_IGNORE size).
    /// Winit does not distinguish between view vs window position on all
    /// platforms, so this uses inner-size-aware outer position.
    pub fn SetViewPos(&self, x: f64, y: f64) {
        self.winit_window()
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    }

    /// Set the content area (view) size.
    ///
    /// Matches C++ emWindow::SetViewSize (PSAS_IGNORE pos, PSAS_VIEW size).
    pub fn SetViewSize(&self, w: f64, h: f64) {
        let _ = self
            .winit_window()
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    /// Set the content area position and size.
    ///
    /// Matches C++ emWindow::SetViewPosSize (PSAS_VIEW pos, PSAS_VIEW size).
    pub fn SetViewPosSize(&mut self, x: f64, y: f64, w: f64, h: f64) {
        match &mut self.os_surface {
            OsSurface::Materialized(m) => {
                m.winit_window
                    .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
                let _ = m
                    .winit_window
                    .request_inner_size(winit::dpi::LogicalSize::new(w, h));
            }
            OsSurface::Pending(p) => {
                // Stash the requested geometry; applied at materialization.
                p.requested_pos_size = Some((x as i32, y as i32, w as i32, h as i32));
            }
        }
    }

    /// Set the window position (including decorations).
    ///
    /// Matches C++ emWindow::SetWinPos (PSAS_WINDOW pos, PSAS_IGNORE size).
    pub fn SetWinPos(&self, x: f64, y: f64) {
        self.winit_window()
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
    }

    /// Set the window size (including decorations).
    ///
    /// Matches C++ emWindow::SetWinSize (PSAS_IGNORE pos, PSAS_WINDOW size).
    /// Winit's `request_inner_size` sets the content area size, not the outer
    /// window size. We subtract border decoration sizes to convert outer
    /// dimensions to inner dimensions.
    pub fn SetWinSize(&self, w: f64, h: f64) {
        let (left, top, right, bottom) = self.GetBorderSizes();
        let inner_w = (w - (left + right) as f64).max(1.0);
        let inner_h = (h - (top + bottom) as f64).max(1.0);
        let _ = self
            .winit_window()
            .request_inner_size(winit::dpi::LogicalSize::new(inner_w, inner_h));
    }

    /// Set window position and size (including decorations).
    ///
    /// Matches C++ emWindow::SetWinPosSize (PSAS_WINDOW pos, PSAS_WINDOW size).
    pub fn SetWinPosSize(&self, x: f64, y: f64, w: f64, h: f64) {
        let (left, top, right, bottom) = self.GetBorderSizes();
        self.winit_window()
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        let inner_w = (w - (left + right) as f64).max(1.0);
        let inner_h = (h - (top + bottom) as f64).max(1.0);
        let _ = self
            .winit_window()
            .request_inner_size(winit::dpi::LogicalSize::new(inner_w, inner_h));
    }

    /// Set window position (outer, including decorations) and view (content) size.
    ///
    /// Matches C++ emWindow::SetWinPosViewSize (PSAS_WINDOW pos, PSAS_VIEW size).
    pub fn SetWinPosViewSize(&self, x: f64, y: f64, w: f64, h: f64) {
        self.winit_window()
            .set_outer_position(winit::dpi::LogicalPosition::new(x, y));
        let _ = self
            .winit_window()
            .request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    // ---------------------------------------------------------------
    // D-WINDOW-01b: Monitor/DPI/close methods
    // ---------------------------------------------------------------

    /// Return the monitor index this window is on, by maximum overlap.
    ///
    /// Matches C++ emWindow::GetMonitorIndex: delegates to
    /// emScreen::monitor_index_of_rect with the window's outer position
    /// and inner size.
    pub fn GetMonitorIndex(&self, screen: &emScreen) -> Option<usize> {
        let pos = self.winit_window().outer_position().unwrap_or_default();
        let size = self.winit_window().inner_size();
        screen.GetMonitorIndexOfRect(pos.x, pos.y, size.width, size.height)
    }

    /// Signal the close signal, triggering auto-delete if WF_AUTO_DELETE is set.
    ///
    /// Matches C++ emWindow::SignalClosing: Signal(CloseSignal).
    /// The caller (App) must call `scheduler.fire(window.close_signal)` to
    /// actually fire the signal in the scheduler. This method returns the
    /// signal ID so the caller can fire it.
    pub fn SignalClosing(&self) -> SignalId {
        self.close_signal
    }

    // ---------------------------------------------------------------
    // D-WINDOW-01c: Misc window methods
    // ---------------------------------------------------------------

    /// Set the window icon from an eaglemode-rs emImage.
    ///
    /// Matches C++ emWindow::SetWindowIcon: copies the image, then applies
    /// it to the winit window. The image is converted to RGBA if needed.
    pub fn SetWindowIcon(&mut self, icon: &emImage) {
        self.window_icon = Some(icon.clone());

        if icon.IsEmpty() {
            self.winit_window().set_window_icon(None);
            return;
        }

        // Convert to RGBA (4 channels) if not already.
        let rgba = if icon.GetChannelCount() == 4 {
            icon.clone()
        } else {
            icon.get_converted(4)
        };

        if let Ok(winit_icon) = winit::window::Icon::from_rgba(
            rgba.GetMap().to_vec(),
            rgba.GetWidth(),
            rgba.GetHeight(),
        ) {
            self.winit_window().set_window_icon(Some(winit_icon));
        } else {
            log::error!(
                "failed to create window icon from {}x{} image",
                rgba.GetWidth(),
                rgba.GetHeight()
            );
        }
    }

    /// Get the current window icon, if set.
    pub fn GetWindowIcon(&self) -> Option<&emImage> {
        self.window_icon.as_ref()
    }

    /// Inhibit the screensaver. Increments an internal counter; issues the
    /// D-Bus Inhibit call on the 0→1 transition.
    ///
    /// Matches C++ emWindowPort::InhibitScreensaver.
    pub fn InhibitScreensaver(&mut self) {
        self.screensaver_inhibit_count += 1;
        if self.screensaver_inhibit_count == 1 {
            self.screensaver_cookie = super::emWindowPlatform::InhibitScreensaver();
            super::emWindowPlatform::start_screensaver_keepalive();
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
    pub fn AllowScreensaver(&mut self) {
        self.screensaver_inhibit_count = self.screensaver_inhibit_count.saturating_sub(1);
        if self.screensaver_inhibit_count == 0 {
            if let Some(cookie) = self.screensaver_cookie.take() {
                super::emWindowPlatform::uninhibit_screensaver(cookie);
            }
            super::emWindowPlatform::stop_screensaver_keepalive();
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
    /// emScreen::can_move_mouse_pointer() before calling.
    pub fn MoveMousePointer(&self, dx: f64, dy: f64) {
        let target_x = self.last_mouse_pos.0 + dx;
        let target_y = self.last_mouse_pos.1 + dy;
        if let Err(e) = self
            .winit_window()
            .set_cursor_position(winit::dpi::PhysicalPosition::new(target_x, target_y))
        {
            log::debug!("move_mouse_pointer failed: {e}");
        }
    }

    /// Emit an acoustic warning beep via libcanberra (Linux) or no-op (other).
    ///
    /// Matches C++ emScreen::Beep.
    pub fn Beep(&self) {
        super::emWindowPlatform::system_beep();
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

/// Find the next unused screenshot filename and shell out to `xwd -root`.
///
/// Port of C++ emCheatVIF screenshot handling: numbered files 000-999 in
/// temp_dir, using `xwd -root` to capture the X11 root window.
fn take_screenshot() {
    let path = match find_next_screenshot_path() {
        Some(p) => p,
        None => {
            eprintln!("[Screenshot] all 1000 screenshot slots (000-999) are taken");
            return;
        }
    };

    let result = std::process::Command::new("xwd")
        .arg("-root")
        .stdout(std::fs::File::create(&path).unwrap_or_else(|e| {
            eprintln!("[Screenshot] cannot create {}: {e}", path.display());
            // Return /dev/null as a fallback to avoid panic
            std::fs::File::create("/dev/null").expect("/dev/null")
        }))
        .status();

    match result {
        Ok(status) if status.success() => {
            eprintln!("[Screenshot] saved to {}", path.display());
        }
        Ok(status) => {
            eprintln!("[Screenshot] xwd exited with {status}");
            let _ = std::fs::remove_file(&path);
        }
        Err(e) => {
            eprintln!("[Screenshot] xwd not found or failed: {e}");
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Find the next unused `eaglemode_screenshot_NNN.xwd` path in temp_dir.
pub(crate) fn find_next_screenshot_path() -> Option<std::path::PathBuf> {
    find_next_screenshot_path_in(&std::env::temp_dir())
}

/// Find the next unused screenshot path within a given directory.
pub(crate) fn find_next_screenshot_path_in(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    for n in 0..1000u32 {
        let name = format!("eaglemode_screenshot_{:03}.xwd", n);
        let path = dir.join(&name);
        if !path.exists() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::EngineScheduler;

    /// Phase 2 Task 2: verifies `emWindow::view` is a plain `emView`
    /// (no `Rc<RefCell<>>` wrapper). The test body requires only that the
    /// struct field has type `emView` — the `&emView` borrow compiles iff
    /// the field is plain.
    #[test]
    fn window_view_is_plain() {
        let mut scheduler = EngineScheduler::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        let close_sig = scheduler.create_signal();
        let flags_sig = scheduler.create_signal();
        let focus_sig = scheduler.create_signal();
        let geom_sig = scheduler.create_signal();
        let win = emWindow::new_popup_pending(
            crate::emContext::emContext::NewRoot(),
            root,
            WindowFlags::empty(),
            "test".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        let _: &emView = &win.view;
    }

    /// Verify that a headless window constructed via `new_popup_pending` +
    /// `RegisterEngines` registers engines correctly — same observable
    /// postcondition previously checked by the deleted `new_for_test` constructor.
    #[test]
    fn headless_window_register_engines_registers_engines() {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
        let win_id = winit::window::WindowId::dummy();
        let sched = std::rc::Rc::new(std::cell::RefCell::new(EngineScheduler::new()));
        let close_sig = sched.borrow_mut().create_signal();
        let flags_sig = sched.borrow_mut().create_signal();
        let focus_sig = sched.borrow_mut().create_signal();
        let geom_sig = sched.borrow_mut().create_signal();
        let mut win = emWindow::new_popup_pending(
            crate::emContext::emContext::NewRoot(),
            root,
            WindowFlags::empty(),
            "test".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        // Phase 2 Task 7: engines identify their owning view via
        // `PanelScope::Toplevel(win_id)`.
        let scope = crate::emPanelScope::PanelScope::Toplevel(win_id);
        {
            let v = win.view_mut();
            let root = v.Context.GetRootContext();
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let mut s = sched.borrow_mut();
            let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                std::cell::RefCell::new(None);
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut s,
                framework_actions: &mut fw,
                root_context: &root,
                framework_clipboard: &__cb,
                current_engine: None,
            };
            v.RegisterEngines(
                &mut sc,
                &mut tree,
                scope,
                crate::emEngine::TreeLocation::Outer,
            );
        }
        assert!(win.view().update_engine_id.is_some());

        // Scheduler cleanup for Drop debug_asserts.
        {
            let v = win.view_mut();
            if let Some(id) = v.update_engine_id.take() {
                sched.borrow_mut().remove_engine(id);
            }
            if let Some(id) = v.visiting_va_engine_id.take() {
                sched.borrow_mut().remove_engine(id);
            }
            if let Some(s) = v.EOISignal.take() {
                sched.borrow_mut().remove_signal(s);
            }
        }
    }

    #[test]
    fn new_popup_pending_constructs_without_event_loop() {
        let mut scheduler = EngineScheduler::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");

        let close_sig = scheduler.create_signal();
        let flags_sig = scheduler.create_signal();
        let focus_sig = scheduler.create_signal();
        let geom_sig = scheduler.create_signal();
        let bg_color = crate::emColor::emColor::rgba(0, 0, 0, 0xFF);

        let popup = emWindow::new_popup_pending(
            crate::emContext::emContext::NewRoot(),
            root,
            WindowFlags::POPUP | WindowFlags::UNDECORATED | WindowFlags::AUTO_DELETE,
            "emViewPopup".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            bg_color,
        );

        assert!(
            !popup.is_materialized(),
            "new_popup_pending must start in Pending state"
        );
        match &popup.os_surface {
            OsSurface::Pending(ps) => {
                assert!(ps.flags.contains(WindowFlags::POPUP));
                assert!(ps.flags.contains(WindowFlags::UNDECORATED));
                assert!(ps.flags.contains(WindowFlags::AUTO_DELETE));
                assert_eq!(ps.caption, "emViewPopup");
                assert!(ps.requested_pos_size.is_none());
            }
            OsSurface::Materialized(_) => panic!("expected Pending"),
        }
        assert_eq!(popup.close_signal, close_sig);
        assert_eq!(popup.flags_signal, flags_sig);
        assert_eq!(popup.focus_signal, focus_sig);
        assert_eq!(popup.geometry_signal, geom_sig);
        assert_eq!(popup.root_panel, root);
        assert_eq!(popup.view().GetBackgroundColor(), bg_color);
        assert!(popup.winit_window_if_materialized().is_none());
    }

    #[test]
    fn screenshot_numbering_skips_existing() {
        // Create a temp dir with 000 and 001 present, assert targets 002
        let dir = std::env::temp_dir().join("eaglemode_screenshot_test");
        let _ = std::fs::create_dir_all(&dir);

        // Create files 000 and 001
        std::fs::write(dir.join("eaglemode_screenshot_000.xwd"), b"").unwrap();
        std::fs::write(dir.join("eaglemode_screenshot_001.xwd"), b"").unwrap();

        let path = find_next_screenshot_path_in(&dir).expect("should find a path");
        assert!(
            path.ends_with("eaglemode_screenshot_002.xwd"),
            "expected 002, got {:?}",
            path
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn screenshot_numbering_starts_at_000() {
        let dir = std::env::temp_dir().join("eaglemode_screenshot_test_empty");
        let _ = std::fs::create_dir_all(&dir);
        // Remove any existing files
        for n in 0..10u32 {
            let _ = std::fs::remove_file(dir.join(format!("eaglemode_screenshot_{:03}.xwd", n)));
        }

        let path = find_next_screenshot_path_in(&dir).expect("should find a path");
        assert!(
            path.ends_with("eaglemode_screenshot_000.xwd"),
            "expected 000, got {:?}",
            path
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Task 4 (Phase 3.5.A): verifies take_tree/put_tree roundtrip.
    /// - take on an empty-sentinel window returns an empty tree.
    /// - put_tree then take_tree round-trips a populated tree.
    ///
    /// Uses public API (GetRootPanel) for emptiness checks — matching
    /// Task 3 pattern in emPanelTree tests.
    #[test]
    fn take_tree_put_tree_roundtrip() {
        let mut scheduler = EngineScheduler::new();
        let mut dummy_tree = PanelTree::new();
        let root = dummy_tree.create_root_deferred_view("root");
        let close_sig = scheduler.create_signal();
        let flags_sig = scheduler.create_signal();
        let focus_sig = scheduler.create_signal();
        let geom_sig = scheduler.create_signal();
        let mut win = emWindow::new_popup_pending(
            crate::emContext::emContext::NewRoot(),
            root,
            WindowFlags::empty(),
            "test".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );

        // Initial tree is the empty sentinel — GetRootPanel() returns None.
        let taken = win.take_tree();
        assert!(
            taken.GetRootPanel().is_none(),
            "default sentinel tree must be empty"
        );
        win.put_tree(taken);

        // Build a populated tree, put it in, take it out — root must survive.
        let mut populated = PanelTree::new();
        let _r = populated.create_root_deferred_view("populated_root");
        win.put_tree(populated);
        let taken2 = win.take_tree();
        assert!(
            taken2.GetRootPanel().is_some(),
            "populated tree must have a root after roundtrip"
        );
        win.put_tree(taken2);
    }
}
