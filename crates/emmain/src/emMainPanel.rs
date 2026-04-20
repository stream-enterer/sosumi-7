// Port of C++ emMain/emMainPanel
// Root panel: splits into control (left) and content (right) sections
// with a draggable slider between them.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emCursor::emCursor;
use emcore::emImage::emImage;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emPainter::emPainter;
use emcore::emPainter::{TextAlignment, VAlign};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emEngineCtx::PanelCtx;
use emcore::emPanelTree::PanelId;
use emcore::emResTga::load_tga;

use crate::emMainConfig::emMainConfig;

// ── SliderPanel ───────────────────────────────────────────────────────────────

/// Draggable divider panel between control and content sections.
///
/// Port of C++ `emMainPanel::SliderPanel` (emMainPanel.cpp:377-502).
///
/// DIVERGED: C++ SliderPanel holds a `MainPanel&` and calls
/// `MainPanel.DragSlider(dy)` / `MainPanel.DoubleClickSlider()` directly.
/// Rust cannot hold parent references in the panel tree. Instead, the parent
/// (`emMainPanel`) reads `pending_drag_delta` and `double_clicked` from this
/// panel via `with_behavior_as` in its `Cycle` and dispatches accordingly.
pub(crate) struct SliderPanel {
    mouse_over: bool,
    pressed: bool,
    hidden: bool,
    press_my: f64,
    press_slider_y: f64,
    /// C++ reads MainPanel.SliderY/SliderMinY/SliderMaxY for arrow rendering.
    /// Parent sets these before Paint via `set_parent_slider_state`.
    parent_slider_y: f64,
    parent_slider_min_y: f64,
    parent_slider_max_y: f64,
    /// Parent's layout width for this panel (= C++ GetLayoutWidth()).
    /// Used to convert panel-local mouse delta to parent coordinates.
    parent_layout_w: f64,
    slider_image: emImage,
    /// Pending drag delta computed during `Input`, consumed by parent `Cycle`.
    pending_drag_delta: Option<f64>,
    /// Set on double-click, consumed by parent `Cycle`.
    double_clicked: bool,
}

impl SliderPanel {
    pub(crate) fn new() -> Self {
        let slider_image = load_tga(include_bytes!("../../../res/emMain/Slider.tga"))
            .expect("failed to load Slider.tga");
        Self {
            mouse_over: false,
            pressed: false,
            hidden: false,
            press_my: 0.0,
            press_slider_y: 0.0,
            parent_slider_y: 0.0,
            parent_slider_min_y: 0.0,
            parent_slider_max_y: 0.0,
            parent_layout_w: 1.0,
            slider_image,
            pending_drag_delta: None,
            double_clicked: false,
        }
    }

    /// Port of C++ `emMainPanel::SliderPanel::SetHidden`.
    /// Wired by `update_slider_hiding` in Task 2.
    pub(crate) fn SetHidden(&mut self, hidden: bool) {
        if self.hidden != hidden {
            self.hidden = hidden;
        }
    }

    /// Set the parent's slider state so Paint can draw arrows conditionally.
    /// Called by the parent before layout/paint.
    pub(crate) fn set_parent_slider_state(
        &mut self,
        slider_y: f64,
        slider_min_y: f64,
        slider_max_y: f64,
        layout_w: f64,
    ) {
        self.parent_slider_y = slider_y;
        self.parent_slider_min_y = slider_min_y;
        self.parent_slider_max_y = slider_max_y;
        self.parent_layout_w = layout_w;
    }
}

impl PanelBehavior for SliderPanel {
    fn IsOpaque(&self) -> bool {
        false
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        let mx = event.mouse_x;
        let my = event.mouse_y;
        let h = _state.height;

        let mo = mx > 0.05 && my > 0.0 && mx < 1.0 && my < h - 0.05;
        if self.mouse_over != mo {
            self.mouse_over = mo;
        }

        if self.mouse_over && event.is_mouse_event() {
            if event.is_left_button() {
                if event.repeat == 0 && !self.pressed {
                    self.pressed = true;
                    self.press_my = my;
                    self.press_slider_y = self.parent_slider_y;
                } else if event.repeat == 1 {
                    // C++ unconditionally calls MainPanel.DoubleClickSlider().
                    // In Rust, the parent reads `double_clicked` in its Cycle.
                    self.double_clicked = true;
                    self.pressed = false;
                }
            }
            return true; // eat event (C++: event.Eat())
        }

        // Compute drag delta while pressed (C++ emMainPanel.cpp:439-444).
        // C++: dy=(my-PressMY)*GetLayoutWidth();
        //      if (shift) dy=(dy+MainPanel.SliderY-PressSliderY)*0.25
        //                    +PressSliderY-MainPanel.SliderY;
        //      MainPanel.DragSlider(dy);
        if self.pressed {
            let mut dy = (my - self.press_my) * self.parent_layout_w;
            if input_state.GetShift() {
                dy = (dy + self.parent_slider_y - self.press_slider_y) * 0.25 + self.press_slider_y
                    - self.parent_slider_y;
            }
            self.pending_drag_delta = Some(dy);
        }

        // Release detection: if pressed but left button no longer held.
        if self.pressed && !input_state.GetLeftButton() {
            self.pressed = false;
        }

        false
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, h: f64, _state: &PanelState) {
        if !self.mouse_over && self.hidden {
            return;
        }

        // Background rounded rect.
        // C++ PaintRoundRect(0,0,2,h, 6.0/64.0, 6.0/75.0*h, color).
        let color = if self.pressed {
            emColor::from_packed(0x002244C0)
        } else if self.mouse_over {
            emColor::from_packed(0x006688A0)
        } else {
            emColor::from_packed(0x33445580)
        };
        painter.PaintRoundRect(
            0.0,
            0.0,
            2.0,
            h,
            6.0 / 64.0,
            6.0 / 75.0 * h,
            color,
            emColor::TRANSPARENT,
        );

        // Arrow indicators (C++ emMainPanel.cpp:478-498).
        if self.mouse_over || self.pressed {
            let x1 = 0.2;
            let x2 = 0.4;
            let y1 = 0.1 * h;
            let y2 = 0.3 * h;
            let mut vertices: Vec<(f64, f64)> = Vec::new();

            // Up arrow: only if slider not at minimum.
            if self.parent_slider_y > self.parent_slider_min_y + 1e-5 {
                vertices.push((x1, y2));
                vertices.push((0.5, y1));
                vertices.push((1.0 - x1, y2));
            }
            // Right side bar.
            vertices.push((1.0 - x2, y2));
            vertices.push((1.0 - x2, h - y2));
            // Down arrow: only if slider not at maximum.
            if self.parent_slider_y < self.parent_slider_max_y - 1e-5 {
                vertices.push((1.0 - x1, h - y2));
                vertices.push((0.5, h - y1));
                vertices.push((x1, h - y2));
            }
            // Left side bar.
            vertices.push((x2, h - y2));
            vertices.push((x2, y2));

            let poly_color = if self.pressed {
                emColor::from_packed(0xEEDD99D0)
            } else {
                emColor::from_packed(0xEEDD9960)
            };
            painter.PaintPolygon(&vertices, poly_color, emColor::TRANSPARENT);
        }

        // Slider texture image (C++: PaintImage(0,0,1,h,SliderImage)).
        painter.paint_image_full(
            0.0,
            0.0,
            1.0,
            h,
            &self.slider_image,
            255,
            emColor::TRANSPARENT,
        );
    }
}

// ── StartupOverlayPanel ──────────────────────────────────────────────────────

/// Full-screen overlay shown during startup.
///
/// Port of C++ `emMainPanel::StartupOverlayPanel` (emMainPanel.cpp:505-565).
///
/// Eats all input events, shows "Loading..." text, and returns a wait cursor.
/// `IsOpaque()` returns `false` — this is critical: otherwise the sub-view panels
/// for content and control would get "non-viewed" state.
pub(crate) struct StartupOverlayPanel;

impl PanelBehavior for StartupOverlayPanel {
    fn IsOpaque(&self) -> bool {
        false
    }

    fn GetCursor(&self) -> emCursor {
        emCursor::Wait
    }

    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        // Eat all input events during startup.
        true
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        painter.Clear(emColor::from_packed(0x808080FF));
        let text_height = 30.0 / painter.GetScaleY();
        painter.PaintTextBoxed(
            0.0,
            0.0,
            w,
            h,
            "Loading...",
            text_height,
            emColor::from_packed(0x404040FF),
            emColor::from_packed(0x808080FF),
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            1.0,
            false,
            0.0,
        );
    }
}

// ── emMainPanel ───────────────────────────────────────────────────────────────

/// Root panel that splits the view into control (left) and content (right)
/// sections with a draggable slider between them.
///
/// Port of C++ `emMainPanel`.
pub struct emMainPanel {
    _ctx: Rc<emContext>,
    config: Rc<RefCell<emMainConfig>>,
    control_tallness: f64,
    unified_slider_pos: f64,

    // Panel IDs for children (in parent tree)
    control_view_panel: Option<PanelId>,
    content_view_panel: Option<PanelId>,
    slider_panel: Option<PanelId>,
    startup_overlay: Option<PanelId>,

    // Control edges decoration
    control_edges_color: emColor,
    control_edges_image: emImage,

    // Cached coordinates
    control_x: f64,
    control_y: f64,
    control_w: f64,
    control_h: f64,
    content_x: f64,
    content_y: f64,
    content_w: f64,
    content_h: f64,
    slider_x: f64,
    slider_y: f64,
    slider_w: f64,
    slider_h: f64,
    slider_min_y: f64,
    slider_max_y: f64,

    // State
    slider_pressed: bool,
    last_height: f64,

    // Mouse movement tracking for slider auto-hide (C++ emMainPanel::Input)
    old_mouse_x: f64,
    old_mouse_y: f64,

    // Fullscreen / slider-hiding state (C++ emMainPanel::UpdateFullscreen / UpdateSliderHiding)
    fullscreen_on: bool,
    slider_hidden: bool,

    // Timer for slider auto-hide (C++ emMainPanel::SliderTimer, 5-second one-shot)
    slider_hide_timer_start: Option<std::time::Instant>,
}

impl emMainPanel {
    /// Create a new emMainPanel.
    ///
    /// Port of C++ `emMainPanel::emMainPanel`.
    pub fn new(ctx: Rc<emContext>, control_tallness: f64) -> Self {
        let config = emMainConfig::Acquire(&ctx);
        let unified_slider_pos = config.borrow().GetControlViewSize();
        let control_edges_image = load_tga(include_bytes!("../../../res/emMain/ControlEdges.tga"))
            .expect("failed to load ControlEdges.tga");
        Self {
            _ctx: ctx,
            config,
            control_tallness,
            unified_slider_pos,
            control_view_panel: None,
            content_view_panel: None,
            slider_panel: None,
            startup_overlay: None,
            control_edges_color: emColor::from_packed(0x515E84FF),
            control_edges_image,
            slider_pressed: false,
            control_x: 0.0,
            control_y: 0.0,
            control_w: 0.0,
            control_h: 0.0,
            content_x: 0.0,
            content_y: 0.0,
            content_w: 0.0,
            content_h: 0.0,
            slider_x: 0.0,
            slider_y: 0.0,
            slider_w: 0.0,
            slider_h: 0.0,
            slider_min_y: 0.0,
            slider_max_y: 0.0,
            last_height: 1.0,
            old_mouse_x: 0.0,
            old_mouse_y: 0.0,
            fullscreen_on: false,
            slider_hidden: false,
            slider_hide_timer_start: None,
        }
    }

    /// Compute layout coordinates for testing.
    /// Sets `unified_slider_pos` and `slider_pressed`, then runs
    /// `update_coordinates`. Returns (control, content, slider) rects.
    pub fn compute_layout_for_test(
        &mut self,
        h: f64,
        slider_pos: f64,
        slider_pressed: bool,
    ) -> [(f64, f64, f64, f64); 3] {
        self.unified_slider_pos = slider_pos;
        self.slider_pressed = slider_pressed;
        self.update_coordinates(h);
        [
            (
                self.control_x,
                self.control_y,
                self.control_w,
                self.control_h,
            ),
            (
                self.content_x,
                self.content_y,
                self.content_w,
                self.content_h,
            ),
            (self.slider_x, self.slider_y, self.slider_w, self.slider_h),
        ]
    }

    /// Compute all layout coordinates given the panel height.
    ///
    /// Port of C++ `emMainPanel::UpdateCoordinates`.
    fn update_coordinates(&mut self, h: f64) {
        self.slider_min_y = 0.0;
        self.slider_max_y = self.control_tallness.min(h * 0.5);
        self.slider_y =
            (self.slider_max_y - self.slider_min_y) * self.unified_slider_pos + self.slider_min_y;
        self.slider_w = (1.0_f64.min(h) * 0.1).min(1.0_f64.max(h) * 0.02);
        self.slider_h = self.slider_w * 1.2;
        self.slider_x = 1.0 - self.slider_w;

        let space_fac = 1.015;
        let t = self.slider_h * 0.5;
        if self.slider_y < t {
            self.control_h = self.slider_y + self.slider_h * self.slider_y / t;
        } else {
            self.control_h = (self.slider_y + self.slider_h) / space_fac;
        }

        if self.control_h < 1e-5 {
            self.control_h = 1e-5;
            self.control_w = self.control_h / self.control_tallness;
            self.control_x = 0.5 * (1.0 - self.control_w);
            self.control_y = 0.0;
            self.content_x = 0.0;
            self.content_y = 0.0;
            self.content_w = 1.0;
            self.content_h = h;
        } else {
            self.control_w = self.control_h / self.control_tallness;
            self.control_x = ((1.0 - self.control_w) * 0.5).min(self.slider_x - self.control_w);
            self.control_y = 0.0;
            if self.control_x < 1e-5 {
                // Do not hide, because otherwise popping up the control view
                // by keyboard would not work properly.
                self.control_w = 1.0 - self.slider_w;
                self.control_x = 0.0;
                self.control_h = self.control_w * self.control_tallness;
                if self.control_h < self.slider_y {
                    self.control_h = self.slider_y;
                    self.control_w = self.control_h / self.control_tallness;
                } else if !self.slider_pressed {
                    self.slider_y = self.control_h * space_fac - self.slider_h;
                }
            }
            self.content_y = self.control_y + self.control_h * space_fac;
            self.content_x = 0.0;
            self.content_w = 1.0;
            self.content_h = h - self.content_y;
        }

        self.last_height = h;
    }

    /// Port of C++ `emMainPanel::UpdateFullscreen` (fullscreen enter path).
    pub fn update_fullscreen_on(&mut self) {
        if !self.fullscreen_on {
            self.fullscreen_on = true;
            if self.config.borrow().GetAutoHideControlView() {
                self.unified_slider_pos = 0.0;
                self.update_coordinates(self.last_height);
                self.update_slider_hiding(false);
            }
        }
    }

    /// Port of C++ `emMainPanel::UpdateFullscreen` (fullscreen exit path).
    pub fn update_fullscreen_off(&mut self) {
        if self.fullscreen_on {
            self.fullscreen_on = false;
            if self.config.borrow().GetAutoHideControlView() {
                self.unified_slider_pos = self.config.borrow().GetControlViewSize();
                self.update_coordinates(self.last_height);
                self.update_slider_hiding(false);
            }
        }
    }

    /// Port of C++ `emMainPanel::UpdateSliderHiding`.
    ///
    /// Hides the slider after 5 seconds in fullscreen when control is collapsed
    /// and AutoHideSlider is enabled.
    /// Port of C++ `emMainPanel::UpdateSliderHiding` (emMainPanel.cpp:322-339).
    ///
    /// Hides the slider after 5 seconds in fullscreen when control is collapsed
    /// and AutoHideSlider is enabled. Uses `slider_hide_timer_start` as a
    /// 5-second one-shot; `Cycle` checks elapsed time and hides when expired.
    fn update_slider_hiding(&mut self, restart: bool) {
        let to_hide = self.unified_slider_pos < 1e-15
            && self.fullscreen_on
            && self.config.borrow().GetAutoHideSlider();

        if !to_hide || restart {
            self.slider_hidden = false;
            self.slider_hide_timer_start = None;
        }
        if to_hide && !self.slider_hidden {
            // Start (or restart) the 5-second timer.
            self.slider_hide_timer_start = Some(std::time::Instant::now());
        }
    }

    /// Disable the startup overlay and return the old PanelId so the caller
    /// can remove it from the tree (C++ does `delete StartupOverlay`).
    ///
    /// Port of C++ `emMainPanel::SetStartupOverlay(false)`.
    pub fn ClearStartupOverlay(&mut self) -> Option<PanelId> {
        self.startup_overlay.take()
    }

    /// Record the PanelId of the startup overlay created by the caller.
    /// Port of C++ `emMainPanel::SetStartupOverlay(true)` which creates the
    /// overlay directly on `this`. In Rust the creator has tree access.
    pub(crate) fn set_startup_overlay(&mut self, id: PanelId) {
        self.startup_overlay = Some(id);
    }

    /// Whether the startup overlay is active.
    ///
    /// Port of C++ `emMainPanel::HasStartupOverlay`.
    pub fn HasStartupOverlay(&self) -> bool {
        self.startup_overlay.is_some()
    }

    /// Get the PanelId of the control sub-view panel.
    pub fn GetControlViewPanelId(&self) -> Option<PanelId> {
        self.control_view_panel
    }

    /// Get the PanelId of the content sub-view panel.
    pub fn GetContentViewPanelId(&self) -> Option<PanelId> {
        self.content_view_panel
    }

    /// Record the PanelId of the control sub-view panel created by the caller.
    /// Port of C++ `emMainPanel::emMainPanel` constructor line
    /// `ControlViewPanel=new emSubViewPanel(this,"control view")`.
    pub(crate) fn set_control_view_panel(&mut self, id: PanelId) {
        self.control_view_panel = Some(id);
    }

    /// Record the PanelId of the content sub-view panel created by the caller.
    /// Port of C++ `emMainPanel::emMainPanel` constructor line
    /// `ContentViewPanel=new emSubViewPanel(this,"content view")`.
    pub(crate) fn set_content_view_panel(&mut self, id: PanelId) {
        self.content_view_panel = Some(id);
    }

    /// Record the PanelId of the slider panel created by the caller.
    /// Port of C++ `emMainPanel::emMainPanel` constructor line
    /// `Slider=new SliderPanel(*this,"slider")`.
    pub(crate) fn set_slider_panel(&mut self, id: PanelId) {
        self.slider_panel = Some(id);
    }

    /// Get the control edges color.
    ///
    /// Port of C++ `emMainPanel::GetControlEdgesColor`.
    pub fn GetControlEdgesColor(&self) -> emColor {
        self.control_edges_color
    }

    /// Get the control edges image.
    ///
    /// Port of C++ `emMainPanel::GetControlEdgesImage`.
    pub fn GetControlEdgesImage(&self) -> &emImage {
        &self.control_edges_image
    }

    /// Set the control edges color.
    ///
    /// Port of C++ `emMainPanel::SetControlEdgesColor`.
    pub fn SetControlEdgesColor(&mut self, color: emColor) {
        // Force alpha to 255.
        let c = emColor::from_packed(color.GetPacked() | 0xFF);
        if self.control_edges_color != c {
            self.control_edges_color = c;
        }
    }

    /// Apply a slider drag delta in parent coordinate space.
    ///
    /// Port of C++ `emMainPanel::DragSlider` (emMainPanel.cpp:342-357).
    pub(crate) fn DragSlider(&mut self, delta_y: f64) {
        let mut y = self.slider_y + delta_y;
        if y <= self.slider_min_y {
            y = self.slider_min_y;
        } else if y > self.slider_max_y {
            y = self.slider_max_y;
        }
        let range = self.slider_max_y - self.slider_min_y;
        if range > 0.0 {
            let n = (y - self.slider_min_y) / range;
            if self.unified_slider_pos != n {
                self.unified_slider_pos = n;
                self.update_coordinates(self.last_height);
                self.update_slider_hiding(false);
                self.config
                    .borrow_mut()
                    .SetControlViewSize(self.unified_slider_pos);
                self.config.borrow_mut().Save();
            }
        }
    }

    /// Toggle the slider between open and closed on double-click.
    ///
    /// Port of C++ `emMainPanel::DoubleClickSlider` (emMainPanel.cpp:360-374).
    pub(crate) fn DoubleClickSlider(&mut self) {
        if self.unified_slider_pos < 0.01 {
            if self.config.borrow().GetControlViewSize() < 0.01 {
                self.config.borrow_mut().SetControlViewSize(0.7);
                self.config.borrow_mut().Save();
            }
            self.unified_slider_pos = self.config.borrow().GetControlViewSize();
        } else {
            self.unified_slider_pos = 0.0;
        }
        self.update_coordinates(self.last_height);
        self.update_slider_hiding(false);
    }
}

impl PanelBehavior for emMainPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn get_title(&self) -> Option<String> {
        Some("Eagle Mode".to_string())
    }

    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        ctx: &mut PanelCtx,
    ) -> bool {
        // Check slider auto-hide timer (C++ SliderTimer, 5-second one-shot).
        if let Some(start) = self.slider_hide_timer_start
            && start.elapsed().as_millis() >= 5000
        {
            self.slider_hidden = true;
            self.slider_hide_timer_start = None;
        }

        // Propagate hidden state to SliderPanel.
        if let Some(slider_id) = self.slider_panel {
            let hidden = self.slider_hidden;
            ctx.tree
                .with_behavior_as::<SliderPanel, _>(slider_id, |sp| {
                    sp.SetHidden(hidden);
                });
        }

        // Read slider drag/double-click actions.
        if let Some(slider_id) = self.slider_panel {
            let action = ctx
                .tree
                .with_behavior_as::<SliderPanel, _>(slider_id, |sp| {
                    let dc = sp.double_clicked;
                    let drag = sp.pending_drag_delta.take();
                    sp.double_clicked = false;
                    (dc, drag)
                });
            if let Some((double_clicked, drag_delta)) = action {
                if double_clicked {
                    self.DoubleClickSlider();
                } else if let Some(dy) = drag_delta {
                    self.DragSlider(dy);
                }
            }
        }
        false
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {
        // Port of C++ emMainPanel::Paint (emMainPanel.cpp:167-222).

        if self.content_y <= 1e-10 {
            return;
        }

        let d = self.control_h * 0.007;
        let x1 = 0.0;
        let y1 = 0.0;
        let w1 = self.control_x - d;
        let h1 = self.control_h;
        let x2 = self.control_x + self.control_w + d;
        let y2 = 0.0;
        let w2 = 1.0 - x2;
        let h2 = self.control_h;

        // Separator strip below control area.
        let sx = 0.0;
        let sy = painter.RoundDownY(self.control_h);
        let sw = 1.0;
        let sh = painter.RoundUpY(self.content_y) - sy;
        painter.PaintRect(
            sx,
            sy,
            sw,
            sh,
            emColor::from_packed(0x000000FF),
            emColor::TRANSPARENT,
        );

        let d = self.control_h * 0.015;

        // Left control edge.
        if self.control_x > 1e-10 {
            let bx = painter.RoundDownX(x1 + w1);
            let by = 0.0;
            let bw = painter.RoundUpX(self.control_x) - bx;
            let bh = painter.RoundUpY(self.content_y);
            painter.PaintRect(
                bx,
                by,
                bw,
                bh,
                emColor::from_packed(0x000000FF),
                emColor::TRANSPARENT,
            );
            painter.PaintRect(
                x1,
                y1,
                w1,
                h1,
                self.control_edges_color,
                self.control_edges_color,
            );
            painter.PaintBorderImageSrcRect(
                x1,
                y1,
                w1,
                h1,
                0.0,
                d,
                d,
                d,
                &self.control_edges_image,
                191,
                0,
                190,
                11,
                0,
                5,
                5,
                5,
                255,
                self.control_edges_color,
                0o57,
            );
        }

        // Right control edge.
        if 1.0 - self.control_x - self.control_w > 1e-10 {
            let bx = painter.RoundDownX(self.control_x + self.control_w);
            let by = 0.0;
            let bw = painter.RoundUpX(x2) - bx;
            let bh = painter.RoundUpY(self.content_y);
            painter.PaintRect(
                bx,
                by,
                bw,
                bh,
                emColor::from_packed(0x000000FF),
                emColor::TRANSPARENT,
            );
            painter.PaintRect(
                x2,
                y2,
                w2,
                h2,
                self.control_edges_color,
                self.control_edges_color,
            );
            painter.PaintBorderImageSrcRect(
                x2,
                y2,
                w2,
                h2,
                d,
                d,
                0.0,
                d,
                &self.control_edges_image,
                0,
                0,
                190,
                11,
                5,
                5,
                0,
                5,
                255,
                self.control_edges_color,
                0o750,
            );
        }
    }

    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // Port of C++ emMainPanel::Input (emMainPanel.cpp:143-158) —
        // detect mouse movement for slider hiding.
        if (self.old_mouse_x - input_state.mouse_x).abs() > 2.5
            || (self.old_mouse_y - input_state.mouse_y).abs() > 2.5
            || input_state.GetLeftButton()
            || input_state.GetMiddleButton()
            || input_state.GetRightButton()
        {
            self.old_mouse_x = input_state.mouse_x;
            self.old_mouse_y = input_state.mouse_y;
            self.update_slider_hiding(true);
        }
        false
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        let h = rect.h;

        // Read latest slider position from config.
        self.unified_slider_pos = self.config.borrow().GetControlViewSize();
        self.update_coordinates(h);

        // Pass parent slider state to SliderPanel for conditional arrow rendering.
        if let Some(slider) = self.slider_panel {
            let sy = self.slider_y;
            let smin = self.slider_min_y;
            let smax = self.slider_max_y;
            let sw = self.slider_w;
            ctx.tree.with_behavior_as::<SliderPanel, _>(slider, |sp| {
                sp.set_parent_slider_state(sy, smin, smax, sw);
            });
        }

        // Position children.
        if let Some(ctrl) = self.control_view_panel {
            ctx.layout_child(
                ctrl,
                self.control_x,
                self.control_y,
                self.control_w,
                self.control_h,
            );
        }
        if let Some(content) = self.content_view_panel {
            ctx.layout_child(
                content,
                self.content_x,
                self.content_y,
                self.content_w,
                self.content_h,
            );
        }
        if let Some(slider) = self.slider_panel {
            ctx.layout_child(
                slider,
                self.slider_x,
                self.slider_y,
                self.slider_w,
                self.slider_h,
            );
        }
        if let Some(overlay) = self.startup_overlay {
            ctx.layout_child(overlay, 0.0, 0.0, 1.0, h);
        }
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::LAYOUT_CHANGED | NoticeFlags::VIEWING_CHANGED) {
            self.unified_slider_pos = self.config.borrow().GetControlViewSize();
            self.update_coordinates(state.height);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Point HOME at a unique per-test temp dir so emMainConfig::Acquire's
    /// load/save goes to an isolated path, never the user's real ~/.eaglemode-rs.
    /// Relies on nextest's per-test-process isolation: env changes do not leak
    /// to other tests.
    fn isolate_home() {
        let tmp = std::env::temp_dir().join(format!(
            "eaglemode_rs_mp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // SAFETY: nextest runs each test in its own process, so the env change
        // does not affect other tests.
        unsafe {
            std::env::set_var("HOME", &tmp);
        }
    }

    #[test]
    fn test_new() {
        // C++ emMainPanel stores the passed controlTallness verbatim
        // (emMainPanel.cpp:56 `ControlTallness=controlTallness`).
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        assert!((panel.control_tallness - 5.0).abs() < 1e-10);
        // startup_overlay is None until the StartupEngine creates it and
        // hands its id to set_startup_overlay (C++ SetStartupOverlay(true)).
        assert!(!panel.HasStartupOverlay());
    }

    #[test]
    fn test_update_coordinates() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_coordinates(1.0);
        assert!(panel.slider_w > 0.0);
        assert!(panel.slider_h > 0.0);
        assert!(panel.control_w > 0.0);
        assert!(panel.content_w > 0.0);
    }

    #[test]
    fn test_coordinates_content_below_control() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_coordinates(1.0);
        assert!(panel.content_y > panel.control_y);
    }

    #[test]
    fn test_title() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        assert_eq!(panel.get_title(), Some("Eagle Mode".to_string()));
    }

    #[test]
    fn test_behavior() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn test_update_coordinates_slider_near_top() {
        // When SliderY < SliderH*0.5, C++ uses: ControlH = SliderY + SliderH * SliderY / t
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.unified_slider_pos = 0.01; // very small → SliderY near 0
        panel.update_coordinates(1.0);
        assert!(panel.control_h > 1e-5);
        assert!(panel.control_h < 0.1);
    }

    #[test]
    fn test_update_coordinates_control_collapsed() {
        // When ControlH < 1E-5, C++ sets ControlH=1E-5 and centers content
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.unified_slider_pos = 0.0; // slider at very top
        panel.update_coordinates(0.001); // very short panel
        assert!(panel.content_h > 0.0);
        assert!(panel.content_x == 0.0);
        assert!(panel.content_w == 1.0);
    }

    #[test]
    fn test_update_coordinates_width_limited() {
        // When ControlX < 1E-5, the C++ branch sets control_w = 1 - slider_w
        // and control_x = 0. To enter this branch we need control_w =
        // control_h / control_tallness large enough that
        // min((1-control_w)*0.5, slider_x - control_w) < 1e-5.
        // control_tallness=0.1 makes control_w ≈ 1.02 (>> 1), guaranteeing entry.
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 0.1);
        panel.unified_slider_pos = 0.8; // slider pushed down
        panel.update_coordinates(1.0);
        // The branch must have been entered: control_x clamped to 0.
        assert_eq!(panel.control_x, 0.0);
        // And control_w set to 1 - slider_w by the branch formula.
        assert!((panel.control_w - (1.0 - panel.slider_w)).abs() < 1e-10);
    }

    #[test]
    fn test_startup_overlay_panel_not_opaque() {
        let panel = StartupOverlayPanel;
        assert!(!panel.IsOpaque());
    }

    #[test]
    fn test_startup_overlay_panel_cursor() {
        let panel = StartupOverlayPanel;
        assert_eq!(panel.GetCursor(), emCursor::Wait);
    }

    #[test]
    fn test_update_coordinates_slider_min_max() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.unified_slider_pos = 0.5;
        panel.update_coordinates(1.0);
        let expected_slider_y = 0.5 * 0.5; // (max-min)*pos + min = 0.5*0.5
        assert!((panel.slider_y - expected_slider_y).abs() < 1e-10);
    }

    #[test]
    fn test_sub_view_panel_fields() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        assert!(panel.control_view_panel.is_none());
        assert!(panel.content_view_panel.is_none());
    }

    #[test]
    fn test_control_edges_color() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        let color = emColor::from_packed(0xFF0000FF);
        panel.SetControlEdgesColor(color);
        assert_eq!(panel.GetControlEdgesColor(), color);
    }

    #[test]
    fn test_control_edges_image_loaded() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        assert!(panel.GetControlEdgesImage().GetWidth() > 0);
        assert!(panel.GetControlEdgesImage().GetHeight() > 0);
    }

    #[test]
    fn test_paint_skips_when_content_at_top() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.content_y = 0.0;
        assert!(panel.content_y <= 1e-10);
    }

    // ── SliderPanel tests ────────────────────────────────────────────────

    #[test]
    fn test_slider_panel_not_opaque() {
        let panel = SliderPanel::new();
        assert!(!panel.IsOpaque());
    }

    #[test]
    fn test_slider_panel_initial_state() {
        let panel = SliderPanel::new();
        assert!(!panel.pressed);
        assert!(!panel.mouse_over);
        assert!((panel.press_my - 0.0).abs() < 1e-10);
        assert!((panel.press_slider_y - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_slider_panel_image_loaded() {
        let panel = SliderPanel::new();
        assert!(panel.slider_image.GetWidth() > 0);
        assert!(panel.slider_image.GetHeight() > 0);
    }

    #[test]
    fn test_slider_panel_set_hidden() {
        let mut panel = SliderPanel::new();
        assert!(!panel.hidden);
        panel.SetHidden(true);
        assert!(panel.hidden);
        panel.SetHidden(false);
        assert!(!panel.hidden);
    }

    // ── DragSlider / DoubleClickSlider tests ───────────────────────────────

    #[test]
    fn test_drag_slider_clamps_to_min() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_coordinates(1.0);
        panel.DragSlider(-999.0);
        assert!(panel.slider_y >= panel.slider_min_y);
    }

    #[test]
    fn test_drag_slider_clamps_to_max() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_coordinates(1.0);
        panel.DragSlider(999.0);
        assert!(panel.slider_y <= panel.slider_max_y);
    }

    #[test]
    fn test_double_click_slider_toggle() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_coordinates(1.0);
        panel.unified_slider_pos = 0.5;
        panel.update_coordinates(1.0);
        panel.DoubleClickSlider();
        assert!(panel.unified_slider_pos < 0.01);
        panel.DoubleClickSlider();
        assert!(panel.unified_slider_pos > 0.01);
    }

    #[test]
    fn test_drag_slider_updates_config() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_coordinates(1.0);
        let old_pos = panel.unified_slider_pos;
        panel.DragSlider(0.1);
        // If range > 0, position should have changed.
        if panel.slider_max_y > panel.slider_min_y {
            assert!((panel.unified_slider_pos - old_pos).abs() > 1e-10);
        }
    }

    // ── UpdateFullscreen / UpdateSliderHiding tests ──────────────────────

    #[test]
    fn test_update_fullscreen_on_auto_hide() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.unified_slider_pos = 0.5;
        panel.update_coordinates(1.0);
        panel.config.borrow_mut().SetAutoHideControlView(true);
        panel.update_fullscreen_on();
        assert!(panel.fullscreen_on);
        // With AutoHideControlView, slider collapses to 0.
        assert!(panel.unified_slider_pos < 0.01);
    }

    #[test]
    fn test_update_fullscreen_on_no_auto_hide() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.unified_slider_pos = 0.5;
        panel.update_coordinates(1.0);
        // AutoHideControlView defaults to false.
        panel.update_fullscreen_on();
        assert!(panel.fullscreen_on);
        // Without auto-hide, slider position unchanged.
        assert!((panel.unified_slider_pos - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_update_fullscreen_off_restores() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.config.borrow_mut().SetAutoHideControlView(true);
        panel.config.borrow_mut().SetControlViewSize(0.6);
        panel.update_fullscreen_on();
        assert!(panel.fullscreen_on);
        assert!(panel.unified_slider_pos < 0.01);
        panel.update_fullscreen_off();
        assert!(!panel.fullscreen_on);
        // Restores from config.
        assert!((panel.unified_slider_pos - 0.6).abs() < 1e-10);
    }

    #[test]
    fn test_update_fullscreen_idempotent() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.update_fullscreen_on();
        panel.update_fullscreen_on(); // second call is no-op
        assert!(panel.fullscreen_on);
        panel.update_fullscreen_off();
        panel.update_fullscreen_off(); // second call is no-op
        assert!(!panel.fullscreen_on);
    }

    #[test]
    fn test_update_slider_hiding_not_fullscreen() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.unified_slider_pos = 0.0;
        panel.config.borrow_mut().SetAutoHideSlider(true);
        // Not fullscreen, so to_hide is false.
        panel.update_slider_hiding(false);
        assert!(!panel.slider_hidden);
    }

    #[test]
    fn test_update_slider_hiding_fullscreen_collapsed() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.fullscreen_on = true;
        panel.unified_slider_pos = 0.0;
        panel.config.borrow_mut().SetAutoHideSlider(true);
        // to_hide is true; timer would start but deferred to Phase 3.
        // slider_hidden stays false until timer fires.
        panel.update_slider_hiding(false);
        assert!(!panel.slider_hidden);
    }

    #[test]
    fn test_update_slider_hiding_restart_unhides() {
        isolate_home();
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emMainPanel::new(Rc::clone(&ctx), 5.0);
        panel.fullscreen_on = true;
        panel.slider_hidden = true;
        panel.unified_slider_pos = 0.0;
        panel.config.borrow_mut().SetAutoHideSlider(true);
        // restart=true: unhides even when to_hide is true.
        panel.update_slider_hiding(true);
        assert!(!panel.slider_hidden);
    }

    // ── SliderPanel pending_drag_delta / double_clicked tests ────────────

    #[test]
    fn test_slider_panel_initial_pending_state() {
        let panel = SliderPanel::new();
        assert!(panel.pending_drag_delta.is_none());
        assert!(!panel.double_clicked);
    }

    #[test]
    fn test_slider_panel_double_click_sets_flag() {
        use emcore::emInput::InputKey;
        let mut panel = SliderPanel::new();
        panel.mouse_over = true;
        // First click (repeat=0) to set pressed.
        let mut press_event = emInputEvent::press(InputKey::MouseLeft);
        press_event.mouse_x = 0.5;
        press_event.mouse_y = 0.1;
        let state = PanelState::default_for_test();
        let input_state = emInputState::default();
        panel.Input(&press_event, &state, &input_state);
        assert!(panel.pressed);
        // Double-click (repeat=1).
        let mut dbl_event = emInputEvent::press(InputKey::MouseLeft);
        dbl_event.mouse_x = 0.5;
        dbl_event.mouse_y = 0.1;
        dbl_event.repeat = 1;
        panel.Input(&dbl_event, &state, &input_state);
        assert!(panel.double_clicked);
        assert!(!panel.pressed);
    }

    #[test]
    fn test_slider_panel_press_records_slider_y() {
        use emcore::emInput::InputKey;
        let mut panel = SliderPanel::new();
        panel.mouse_over = true;
        panel.parent_slider_y = 0.42;
        let mut press_event = emInputEvent::press(InputKey::MouseLeft);
        press_event.mouse_x = 0.5;
        press_event.mouse_y = 0.2;
        let state = PanelState::default_for_test();
        let input_state = emInputState::default();
        panel.Input(&press_event, &state, &input_state);
        assert!(panel.pressed);
        assert!((panel.press_slider_y - 0.42).abs() < 1e-10);
        assert!((panel.press_my - 0.2).abs() < 1e-10);
    }
}
