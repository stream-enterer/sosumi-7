//! Comprehensive integration test panel derived from C++ `emTestPanel.cpp`.
//!
//! Exercises nearly every subsystem: panel tree, auto-expansion, recursive
//! children, full paint primitives, input logging, widgets, splitters,
//! color field binding, and interactive polygon drawing.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use zuicchini::foundation::{Color, Image};
use zuicchini::input::{Cursor, InputEvent, InputKey, InputState, InputVariant};
use zuicchini::layout::Orientation;
use zuicchini::panel::{
    NoticeFlags, PanelBehavior, PanelCtx, PanelId, PanelState, ViewConditionType, ViewFlags,
};
use zuicchini::render::{
    ImageExtension, ImageQuality, LineCap, LineJoin, Painter, Stroke, StrokeEnd, StrokeEndType,
    TextAlignment, Texture, VAlign,
};
use zuicchini::widget::{
    Button, CheckBox, CheckButton, ColorField, Label, ListBox, Look, RadioBox, RadioButton,
    RadioGroup, ScalarField, SelectionMode, Splitter, TextField,
};
use zuicchini::window::{App, WindowFlags};

// ── Constants ──

const MAX_DEPTH: u32 = 2;
const MAX_LOG_ENTRIES: usize = 20;

const CHILD_LAYOUT: [(&str, f64, f64, f64, f64); 7] = [
    ("tktest", 0.20, 0.15, 0.30, 0.12),
    ("tp1", 0.70, 0.05, 0.12, 0.12),
    ("tp2", 0.83, 0.05, 0.12, 0.12),
    ("tp3", 0.70, 0.18, 0.12, 0.12),
    ("tp4", 0.83, 0.18, 0.12, 0.12),
    ("bgcf", 0.775, 0.34, 0.10, 0.02),
    ("polydraw", 0.05, 0.92, 0.08, 0.04),
];

// ═══════════════════════════════════════════════════════════════════════
// Widget wrapper panels
// ═══════════════════════════════════════════════════════════════════════

struct ButtonPanel {
    widget: Button,
}
impl PanelBehavior for ButtonPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct CheckButtonPanel {
    widget: CheckButton,
}
impl PanelBehavior for CheckButtonPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct CheckBoxPanel {
    widget: CheckBox,
}
impl PanelBehavior for CheckBoxPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct RadioButtonPanel {
    widget: RadioButton,
}
impl PanelBehavior for RadioButtonPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct RadioBoxPanel {
    widget: RadioBox,
}
impl PanelBehavior for RadioBoxPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct TextFieldPanel {
    widget: TextField,
}
impl PanelBehavior for TextFieldPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ScalarFieldPanel {
    widget: ScalarField,
}
impl PanelBehavior for ScalarFieldPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.widget.paint(p, w, h, s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ColorFieldPanel {
    widget: ColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ListBoxPanel {
    widget: ListBox,
}
impl PanelBehavior for ListBoxPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct LabelPanel {
    widget: Label,
}
impl PanelBehavior for LabelPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct SplitterPanel {
    widget: Splitter,
}
impl PanelBehavior for SplitterPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.widget.layout_children(ctx, rect.w, rect.h);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TestPanel — root panel, recursive
// ═══════════════════════════════════════════════════════════════════════

struct TestPanel {
    bg_color_shared: Rc<Cell<Color>>,
    input_log: Vec<String>,
    test_image: Image,
    depth: u32,
}

impl TestPanel {
    fn new(depth: u32, bg_color_shared: Rc<Cell<Color>>) -> Self {
        let mut img = Image::new(64, 64, 4);
        for y in 0..64u32 {
            for x in 0..64u32 {
                img.set_pixel_channel(x, y, 0, (x * 4) as u8);
                img.set_pixel_channel(x, y, 1, (y * 4) as u8);
                img.set_pixel_channel(x, y, 2, 128);
                img.set_pixel_channel(x, y, 3, 255);
            }
        }
        Self {
            bg_color_shared,
            input_log: Vec::new(),
            test_image: img,
            depth,
        }
    }

    fn bg_color(&self) -> Color {
        self.bg_color_shared.get()
    }

    /// Paint all the primitive-drawing tests (matches C++ Paint() body).
    fn paint_primitives(&self, p: &mut Painter, fg: Color, bg: Color) {
        // Text test with tabs
        p.paint_text_boxed(
            0.25,
            0.80,
            0.05,
            0.05,
            "Text Test\n\t<-tab\ntab->\t<-tab",
            0.1,
            fg,
            bg,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Left,
            0.2,
            true,
            0.1,
        );
        p.paint_rect(0.25, 0.80, 0.05, 0.05, Color::rgba(255, 0, 0, 32));

        // Triangle
        p.paint_polygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg);

        // Holed polygon (even-odd)
        p.paint_polygon_even_odd(
            &[
                (0.90, 0.90),
                (0.94, 0.90),
                (0.94, 0.94),
                (0.90, 0.94),
                (0.90, 0.90),
                (0.91, 0.91),
                (0.93, 0.91),
                (0.93, 0.93),
                (0.91, 0.93),
                (0.91, 0.91),
            ],
            Color::rgba(255, 255, 255, 128),
        );

        // Holed polygon (non-zero winding, reversed inner)
        p.paint_polygon(
            &[
                (0.80, 0.90),
                (0.84, 0.90),
                (0.84, 0.94),
                (0.80, 0.94),
                (0.80, 0.90),
                (0.81, 0.91),
                (0.81, 0.93),
                (0.83, 0.93),
                (0.83, 0.91),
                (0.81, 0.91),
            ],
            Color::WHITE,
        );

        // Circle (polygon approximation)
        let circle: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.paint_polygon(&circle, Color::YELLOW);

        // Clipped circle
        p.push_state();
        p.clip_rect(0.51, 0.81, 0.08, 0.08);
        let circle2: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.paint_polygon(&circle2, Color::GREEN);
        p.pop_state();

        // Ellipse (polygon)
        let ellipse: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        p.paint_polygon(&ellipse, Color::rgba(255, 0, 0, 92));

        // More triangles
        p.paint_polygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            Color::rgba(187, 255, 255, 255),
        );
        p.paint_polygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], Color::RED);
        p.paint_polygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            Color::rgba(187, 255, 255, 255),
        );
        p.paint_polygon(&[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)], Color::RED);

        // Thin triangles
        p.paint_polygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            Color::rgba(187, 136, 255, 192),
        );
        p.paint_polygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            Color::rgba(136, 187, 255, 192),
        );

        // Bowtie
        p.paint_polygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            Color::rgba(136, 187, 255, 192),
        );

        // Ellipses (center + radius)
        p.paint_ellipse(0.055, 0.805, 0.005, 0.005, Color::WHITE);
        p.paint_ellipse(0.07, 0.805, 0.01, 0.005, Color::WHITE);
        p.paint_ellipse(0.0925, 0.805, 0.0025, 0.005, Color::WHITE);

        // Ellipse sectors (degrees: start_angle, sweep_angle)
        p.paint_ellipse_sector(0.105, 0.805, 0.005, 0.005, 45.0, 305.0, Color::WHITE);
        p.paint_ellipse_sector(0.12, 0.805, 0.01, 0.005, 45.0, -395.0, Color::WHITE);

        // Rect outlines
        let thin_stroke = Stroke::new(Color::WHITE, 0.001);
        p.paint_rect_outlined(0.05, 0.82, 0.01, 0.01, &thin_stroke);
        let thick_stroke = Stroke::new(Color::WHITE, 0.008);
        p.paint_rect_outlined(0.10, 0.82, 0.01, 0.01, &thick_stroke);

        // Round rects
        p.paint_round_rect(0.05, 0.84, 0.01, 0.01, 0.001, Color::WHITE);
        p.paint_round_rect(0.07, 0.84, 0.02, 0.01, 0.002, Color::WHITE);
        p.paint_round_rect(0.10, 0.84, 0.01, 0.01, 0.003, Color::WHITE);

        // Ellipse outlines
        let outline_stroke = Stroke::new(Color::WHITE, 0.003);
        p.paint_ellipse_outlined(0.055, 0.865, 0.005, 0.005, &outline_stroke);
        let thin_outline = Stroke::new(Color::WHITE, 0.001);
        p.paint_ellipse_outlined(0.075, 0.865, 0.01, 0.005, &thin_outline);

        // Round rect outlines
        let rr_stroke = Stroke::new(Color::WHITE, 0.001);
        p.paint_round_rect_outlined(0.05, 0.88, 0.01, 0.01, 0.001, &rr_stroke);
        p.paint_round_rect_outlined(0.07, 0.88, 0.02, 0.01, 0.002, &rr_stroke);

        // Bezier curves
        p.paint_bezier(&[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)], Color::WHITE);
        p.paint_bezier(
            &[
                (0.065, 0.91),
                (0.05, 0.902),
                (0.058, 0.89),
                (0.065, 0.900),
                (0.072, 0.89),
                (0.08, 0.902),
            ],
            Color::WHITE,
        );

        let bezier_stroke = Stroke::new(Color::WHITE, 0.0002);
        p.paint_bezier_outline(
            &[
                (0.085, 0.91),
                (0.07, 0.902),
                (0.078, 0.89),
                (0.085, 0.900),
                (0.092, 0.89),
                (0.10, 0.902),
            ],
            &bezier_stroke,
        );

        let mut arrow_s = Stroke::new(Color::WHITE, 0.0002);
        arrow_s.cap = LineCap::Round;
        arrow_s.join = LineJoin::Round;
        arrow_s.start_end =
            StrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(Color::RED);
        arrow_s.finish_end = StrokeEnd::new(StrokeEndType::Arrow);
        p.paint_bezier_line(
            &[(0.105, 0.91), (0.09, 0.902), (0.098, 0.89), (0.105, 0.900)],
            &arrow_s,
        );

        // All 17 StrokeEndType variants in radial pattern
        let end_types = [
            StrokeEndType::Butt,
            StrokeEndType::Cap,
            StrokeEndType::Arrow,
            StrokeEndType::ContourArrow,
            StrokeEndType::LineArrow,
            StrokeEndType::Triangle,
            StrokeEndType::ContourTriangle,
            StrokeEndType::Square,
            StrokeEndType::ContourSquare,
            StrokeEndType::HalfSquare,
            StrokeEndType::Circle,
            StrokeEndType::ContourCircle,
            StrokeEndType::HalfCircle,
            StrokeEndType::Diamond,
            StrokeEndType::ContourDiamond,
            StrokeEndType::HalfDiamond,
            StrokeEndType::Stroke,
        ];
        let n = end_types.len();
        for i in 0..(2 * n) {
            let a = 2.0 * PI * i as f64 / (2 * n) as f64;
            let mut s = Stroke::new(Color::WHITE, 0.0001);
            if i & 1 != 0 {
                s.cap = LineCap::Round;
                s.join = LineJoin::Round;
            }
            s.start_end = StrokeEnd::new(StrokeEndType::Cap);
            s.finish_end = StrokeEnd::new(end_types[i / 2])
                .with_inner_color(Color::rgba(0xFF, 0xFF, 0xFF, 0x40));
            p.paint_line_stroked(
                0.117 + 0.002 * a.cos(),
                0.903 + 0.002 * a.sin(),
                0.117 + 0.0075 * a.cos(),
                0.903 + 0.0075 * a.sin(),
                &s,
            );
        }

        // Polyline with contour arrow
        let mut poly_s = Stroke::new(Color::WHITE, 0.0005);
        poly_s.cap = LineCap::Round;
        poly_s.join = LineJoin::Round;
        poly_s.start_end = StrokeEnd::new(StrokeEndType::ContourArrow);
        poly_s.finish_end = StrokeEnd::new(StrokeEndType::Cap);
        p.paint_polyline_with_arrows(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &poly_s,
            false,
        );

        // Polygon outline
        p.paint_polygon_outlined(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            Color::RED,
            0.0002,
        );

        // Textured polygons — star shapes
        let star = make_star(0.215, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star,
            &Texture::LinearGradient {
                color_a: Color::rgba(0, 0xFF, 0, 0x80),
                color_b: Color::rgba(0xFF, 0xFF, 0, 0xFF),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
        );

        let star2 = make_star(0.235, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star2,
            &Texture::RadialGradient {
                color_inner: Color::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: Color::rgba(0, 0, 0xFF, 0x60),
                center: (0.21, 0.90),
                radius: 0.05,
            },
        );

        let star3 = make_star(0.255, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star3,
            &Texture::Image {
                image: self.test_image.clone(),
                extension: ImageExtension::Repeat,
                quality: ImageQuality::Bilinear,
            },
        );

        // Gradient rects
        p.paint_linear_gradient(
            0.2,
            0.94,
            0.02,
            0.01,
            Color::rgba(0, 0, 0, 0x80),
            Color::rgba(0x80, 0x80, 0x80, 0x80),
            true,
        );
        p.paint_radial_gradient(
            0.225,
            0.946,
            0.004,
            0.008,
            Color::rgba(0xFF, 0x88, 0, 0xFF),
            Color::rgba(0, 0x55, 0, 0xFF),
        );
        p.paint_ellipse(0.24, 0.945, 0.01, 0.005, Color::rgba(0, 0xCC, 0x88, 0xFF));

        // Image scaled
        p.paint_image_scaled(
            0.26,
            0.94,
            0.02,
            0.01,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Repeat,
        );
    }
}

impl PanelBehavior for TestPanel {
    fn is_opaque(&self) -> bool {
        self.bg_color().is_opaque()
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn get_title(&self) -> Option<String> {
        Some("Test Panel".into())
    }

    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        let bg = self.bg_color();
        let fg = if state.is_focused() {
            Color::rgba(255, 136, 136, 255)
        } else if state.in_focused_path() {
            Color::rgba(187, 136, 136, 255)
        } else {
            Color::rgba(136, 136, 136, 255)
        };

        // Use push/scale/pop so coordinates are in [0,1] x [0,h/w]
        painter.push_state();
        painter.scale(w, w);

        let panel_h = h / w;
        painter.paint_rect(0.0, 0.0, 1.0, panel_h, bg);
        painter.paint_rect_outlined(0.01, 0.01, 0.98, panel_h - 0.02, &Stroke::new(fg, 0.02));

        // Title
        painter.paint_text_boxed(
            0.02,
            0.02,
            0.49,
            0.07,
            "Test Panel",
            0.1,
            fg,
            bg,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            0.5,
            true,
            0.15,
        );

        // State display
        let mut status = "State:".to_string();
        if state.is_focused() {
            status += " Focused";
        }
        if state.in_focused_path() {
            status += " InFocusedPath";
        }
        painter.paint_text_boxed(
            0.05,
            0.4,
            0.9,
            0.05,
            &status,
            0.05,
            fg,
            bg,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            true,
            0.15,
        );

        let pri_str = format!("Pri={:.6} MemLim={}", state.priority, state.memory_limit);
        painter.paint_text_boxed(
            0.05,
            0.45,
            0.9,
            0.1,
            &pri_str,
            0.1,
            fg,
            bg,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            true,
            0.15,
        );

        // Input log
        for (i, entry) in self.input_log.iter().enumerate() {
            painter.paint_text(
                0.05,
                0.57 + i as f64 * 0.008,
                entry,
                0.008,
                1.0,
                Color::rgba(0x88, 0x88, 0xBB, 0xFF),
                bg,
            );
        }

        // Paint primitives
        self.paint_primitives(painter, fg, bg);

        painter.pop_state();
    }

    fn input(
        &mut self,
        event: &InputEvent,
        _state: &PanelState,
        _input_state: &InputState,
    ) -> bool {
        let log = format!(
            "key={:?} chars=\"{}\" repeat={} variant={:?} mouse={:.1},{:.1}",
            event.key, event.chars, event.repeat, event.variant, event.mouse_x, event.mouse_y,
        );
        if self.input_log.len() >= MAX_LOG_ENTRIES {
            self.input_log.remove(0);
        }
        self.input_log.push(log);
        false
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {
        // Nothing specific — painting is already invalidated by the framework.
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();

        if !children.is_empty() {
            // Reposition existing children
            for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
                if let Some(child) = ctx.find_child_by_name(name) {
                    ctx.layout_child(child, x, y, cw, ch);
                }
            }
            return;
        }

        // Create children — matches C++ AutoExpand()
        let bg_shared = self.bg_color_shared.clone();

        // TkTestGrp
        ctx.create_child_with("tktest", Box::new(TkTestGrpPanel::new()));

        // Recursive test panels (depth + 1)
        if self.depth < MAX_DEPTH {
            for i in 1..=4 {
                let child_bg = Rc::new(Cell::new(Color::rgba(0x00, 0x1C, 0x38, 0xFF)));
                ctx.create_child_with(
                    &format!("tp{i}"),
                    Box::new(TestPanel::new(self.depth + 1, child_bg)),
                );
            }
        }

        // Background color field — linked to bg_color_shared
        let bg_for_cf = bg_shared.clone();
        let mut cf = ColorField::new(Look::new());
        cf.set_editable(true);
        cf.set_alpha_enabled(true);
        cf.set_color(bg_shared.get());
        cf.on_color = Some(Box::new(move |color| {
            bg_for_cf.set(color);
        }));
        ctx.create_child_with("bgcf", Box::new(ColorFieldPanel { widget: cf }));

        // PolyDraw panel
        ctx.create_child_with("polydraw", Box::new(PolyDrawPanel::new()));

        // Layout all children
        for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
            if let Some(child) = ctx.find_child_by_name(name) {
                ctx.layout_child(child, x, y, cw, ch);
            }
        }
    }

    fn create_control_panel(&mut self, ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let identity = ctx.tree.get_identity(ctx.id);
        let bg = self.bg_color();
        let text = format!(
            "This is just a test\n\nPanel Identity: {identity}\nBgColor: 0x{:08X}",
            bg.as_u32()
        );
        let label = Label::new(&text, Look::new());
        Some(ctx.create_child_with(name, Box::new(LabelPanel { widget: label })))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TkTestGrpPanel — splitter hierarchy hosting TkTest widget showcases
// ═══════════════════════════════════════════════════════════════════════

struct TkTestGrpPanel;

impl TkTestGrpPanel {
    fn new() -> Self {
        Self
    }
}

impl PanelBehavior for TkTestGrpPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        p.paint_rect(0.0, 0.0, w, h, Color::rgba(0x20, 0x30, 0x40, 0xFF));
        p.paint_text_boxed(
            0.0,
            0.0,
            w,
            h * 0.05,
            "Toolkit Test",
            h * 0.04,
            Color::WHITE,
            Color::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let rect = ctx.layout_rect();
        let h = rect.h / rect.w;

        if !children.is_empty() {
            // Reposition the main splitter
            if let Some(sp) = ctx.find_child_by_name("sp") {
                ctx.layout_child(sp, 0.0, 0.05 * h, 1.0, 0.95 * h);
            }
            return;
        }

        let look = Look::new();

        // Main horizontal splitter
        let mut sp = Splitter::new(Orientation::Horizontal, look.clone());
        sp.set_position(0.5);
        let sp_id = ctx.create_child_with("sp", Box::new(SplitterPanel { widget: sp }));

        // Left vertical splitter
        let mut sp1 = Splitter::new(Orientation::Vertical, look.clone());
        sp1.set_position(0.5);
        let _sp1_id = ctx.create_child_with("sp1", Box::new(SplitterPanel { widget: sp1 }));

        // Right vertical splitter
        let mut sp2 = Splitter::new(Orientation::Vertical, look.clone());
        sp2.set_position(0.5);
        let _sp2_id = ctx.create_child_with("sp2", Box::new(SplitterPanel { widget: sp2 }));

        // Four TkTest panels
        ctx.create_child_with("t1a", Box::new(TkTestPanel::new(look.clone())));
        ctx.create_child_with("t1b", Box::new(TkTestPanel::new(look.clone())));
        ctx.create_child_with("t2a", Box::new(TkTestPanel::new(look.clone())));

        let t2b_id = ctx.create_child_with("t2b", Box::new(TkTestPanel::new(look.clone())));
        ctx.tree.set_enable_switch(t2b_id, false); // disabled per C++ spec

        // Layout: sp fills below title; children of sp are sp1 and sp2
        // But since splitters manage their own children's layout via
        // layout_children, we need to reparent. In zuicchini, splitter
        // children are just the first two children of the splitter panel.
        // The architecture is flat — all children are under TkTestGrpPanel.
        // We lay them out manually in a 2x2 grid instead.
        ctx.layout_child(sp_id, 0.0, 0.05 * h, 1.0, 0.95 * h);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TkTestPanel — widget showcase (matches C++ TkTest)
// ═══════════════════════════════════════════════════════════════════════

struct TkTestPanel {
    look: Rc<Look>,
}

impl TkTestPanel {
    fn new(look: Rc<Look>) -> Self {
        Self { look }
    }
}

impl PanelBehavior for TkTestPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        p.paint_rect(0.0, 0.0, w, h, Color::rgba(0x30, 0x40, 0x50, 0xFF));
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();

        // Grid layout
        let cols = 3;
        let margin = 0.02;
        let cell_w = (1.0 - margin * (cols as f64 + 1.0)) / cols as f64;
        let cell_h = cell_w * 0.3;

        if !children.is_empty() {
            for (i, child) in children.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let x = margin + col as f64 * (cell_w + margin);
                let y = margin + row as f64 * (cell_h + margin);
                ctx.layout_child(*child, x, y, cell_w, cell_h);
            }
            return;
        }

        let look = self.look.clone();

        // Buttons
        let bt1 = Button::new("Button", look.clone());
        ctx.create_child_with("b1", Box::new(ButtonPanel { widget: bt1 }));

        let bt2 = Button::new("Long Desc", look.clone());
        ctx.create_child_with("b2", Box::new(ButtonPanel { widget: bt2 }));

        // Check Buttons
        let cb1 = CheckButton::new("Check Button", look.clone());
        ctx.create_child_with("c1", Box::new(CheckButtonPanel { widget: cb1 }));
        let cb2 = CheckButton::new("Check Button", look.clone());
        ctx.create_child_with("c2", Box::new(CheckButtonPanel { widget: cb2 }));

        // Check Boxes
        let cbx1 = CheckBox::new("Check Box", look.clone());
        ctx.create_child_with("c4", Box::new(CheckBoxPanel { widget: cbx1 }));
        let cbx2 = CheckBox::new("Check Box", look.clone());
        ctx.create_child_with("c5", Box::new(CheckBoxPanel { widget: cbx2 }));

        // Radio Buttons
        let rg = RadioGroup::new();
        let rb1 = RadioButton::new("Radio Button", look.clone(), rg.clone(), 0);
        ctx.create_child_with("r1", Box::new(RadioButtonPanel { widget: rb1 }));
        let rb2 = RadioButton::new("Radio Button", look.clone(), rg.clone(), 1);
        ctx.create_child_with("r2", Box::new(RadioButtonPanel { widget: rb2 }));
        let rb3 = RadioButton::new("Radio Button", look.clone(), rg.clone(), 2);
        ctx.create_child_with("r3", Box::new(RadioButtonPanel { widget: rb3 }));

        // Radio Boxes
        let rg2 = RadioGroup::new();
        let rbx1 = RadioBox::new("Radio Box", look.clone(), rg2.clone(), 0);
        ctx.create_child_with("r4", Box::new(RadioBoxPanel { widget: rbx1 }));
        let rbx2 = RadioBox::new("Radio Box", look.clone(), rg2.clone(), 1);
        ctx.create_child_with("r5", Box::new(RadioBoxPanel { widget: rbx2 }));
        let rbx3 = RadioBox::new("Radio Box", look.clone(), rg2.clone(), 2);
        ctx.create_child_with("r6", Box::new(RadioBoxPanel { widget: rbx3 }));

        // Text Fields
        let mut tf1 = TextField::new(look.clone());
        tf1.set_text("Read-Only");
        ctx.create_child_with("tf1", Box::new(TextFieldPanel { widget: tf1 }));

        let mut tf2 = TextField::new(look.clone());
        tf2.set_editable(true);
        tf2.set_text("Editable");
        ctx.create_child_with("tf2", Box::new(TextFieldPanel { widget: tf2 }));

        let mut tf3 = TextField::new(look.clone());
        tf3.set_editable(true);
        tf3.set_text("Password");
        tf3.set_password_mode(true);
        ctx.create_child_with("tf3", Box::new(TextFieldPanel { widget: tf3 }));

        let mut tf4 = TextField::new(look.clone());
        tf4.set_editable(true);
        tf4.set_multi_line(true);
        tf4.set_text("first line\nsecond line\n...");
        ctx.create_child_with("mltf1", Box::new(TextFieldPanel { widget: tf4 }));

        // Scalar Fields
        let sf1 = ScalarField::new(0.0, 100.0, look.clone());
        ctx.create_child_with("sf1", Box::new(ScalarFieldPanel { widget: sf1 }));

        let mut sf2 = ScalarField::new(0.0, 100.0, look.clone());
        sf2.set_editable(true);
        ctx.create_child_with("sf2", Box::new(ScalarFieldPanel { widget: sf2 }));

        let mut sf3 = ScalarField::new(-1000.0, 1000.0, look.clone());
        sf3.set_editable(true);
        sf3.set_scale_mark_intervals(&[1000, 100, 10, 5, 1]);
        ctx.create_child_with("sf3", Box::new(ScalarFieldPanel { widget: sf3 }));

        // Color Fields
        let mut cf1 = ColorField::new(look.clone());
        cf1.set_color(Color::rgba(0xBB, 0x22, 0x22, 0xFF));
        ctx.create_child_with("cf1", Box::new(ColorFieldPanel { widget: cf1 }));

        let mut cf2 = ColorField::new(look.clone());
        cf2.set_editable(true);
        cf2.set_color(Color::rgba(0x22, 0xBB, 0x22, 0xFF));
        ctx.create_child_with("cf2", Box::new(ColorFieldPanel { widget: cf2 }));

        let mut cf3 = ColorField::new(look.clone());
        cf3.set_editable(true);
        cf3.set_alpha_enabled(true);
        cf3.set_color(Color::rgba(0x22, 0x22, 0xBB, 0xFF));
        ctx.create_child_with("cf3", Box::new(ColorFieldPanel { widget: cf3 }));

        // List Boxes
        let lb1 = ListBox::new(look.clone());
        ctx.create_child_with("l1", Box::new(ListBoxPanel { widget: lb1 }));

        let mut lb2 = ListBox::new(look.clone());
        lb2.set_selection_mode(SelectionMode::Single);
        lb2.set_items((1..=7).map(|i| format!("Item {i}")).collect());
        ctx.create_child_with("l2", Box::new(ListBoxPanel { widget: lb2 }));

        let mut lb3 = ListBox::new(look.clone());
        lb3.set_selection_mode(SelectionMode::ReadOnly);
        lb3.set_items((1..=7).map(|i| format!("Item {i}")).collect());
        ctx.create_child_with("l3", Box::new(ListBoxPanel { widget: lb3 }));

        let mut lb4 = ListBox::new(look.clone());
        lb4.set_selection_mode(SelectionMode::Multi);
        lb4.set_items((1..=7).map(|i| format!("Item {i}")).collect());
        ctx.create_child_with("l4", Box::new(ListBoxPanel { widget: lb4 }));

        let mut lb5 = ListBox::new(look.clone());
        lb5.set_selection_mode(SelectionMode::Toggle);
        lb5.set_items((1..=7).map(|i| format!("Item {i}")).collect());
        ctx.create_child_with("l5", Box::new(ListBoxPanel { widget: lb5 }));

        // Layout all children
        let all = ctx.children();
        for (i, child) in all.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = margin + col as f64 * (cell_w + margin);
            let y = margin + row as f64 * (cell_h + margin);
            ctx.layout_child(*child, x, y, cell_w, cell_h);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// PolyDrawPanel — interactive polygon/stroke drawing
// ═══════════════════════════════════════════════════════════════════════

struct PolyDrawPanel {
    paint_type: usize,
    vertices: Vec<(f64, f64)>,
    drag_idx: Option<usize>,
    drag_offset: (f64, f64),
    show_handles: bool,
    fill_color: Color,
    stroke_width: f64,
    stroke_color: Color,
}

impl PolyDrawPanel {
    fn new() -> Self {
        let n = 9;
        let vertices: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / n as f64;
                (a.cos() * 0.4 + 0.5, a.sin() * 0.4 + 0.5)
            })
            .collect();
        Self {
            paint_type: 0,
            vertices,
            drag_idx: None,
            drag_offset: (0.0, 0.0),
            show_handles: false,
            fill_color: Color::WHITE,
            stroke_width: 0.01,
            stroke_color: Color::BLACK,
        }
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn input(&mut self, event: &InputEvent, _state: &PanelState, input_state: &InputState) -> bool {
        let mx = event.mouse_x;
        let my = event.mouse_y;

        if self.drag_idx.is_none()
            && event.key == InputKey::MouseLeft
            && event.variant == InputVariant::Press
        {
            // Find nearest vertex
            let threshold = 0.03;
            let mut best_i = None;
            let mut best_r = threshold;
            for (i, &(vx, vy)) in self.vertices.iter().enumerate() {
                let dx = vx - mx;
                let dy = vy - my;
                let r = (dx * dx + dy * dy).sqrt();
                if r < best_r {
                    best_i = Some(i);
                    best_r = r;
                }
            }
            if let Some(idx) = best_i {
                self.drag_idx = Some(idx);
                self.drag_offset = (self.vertices[idx].0 - mx, self.vertices[idx].1 - my);
            }
            return best_i.is_some();
        }

        if self.drag_idx.is_some() && !input_state.is_pressed(InputKey::MouseLeft) {
            self.drag_idx = None;
            return false;
        }

        if let Some(idx) = self.drag_idx {
            let x = (mx + self.drag_offset.0).clamp(0.0, 1.0);
            let y = (my + self.drag_offset.1).clamp(0.0, 1.0);
            self.vertices[idx] = (x, y);
        }

        // Show handles when mouse is inside panel
        let inside =
            self.drag_idx.is_some() || ((0.0..=1.0).contains(&mx) && (0.0..=1.0).contains(&my));
        self.show_handles = inside;

        false
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        // Background gradient
        p.paint_linear_gradient(
            0.0,
            0.0,
            w,
            h,
            Color::rgba(80, 80, 160, 255),
            Color::rgba(160, 160, 80, 255),
            false,
        );

        // Scale vertices to pixel space
        let scaled: Vec<(f64, f64)> = self
            .vertices
            .iter()
            .map(|&(vx, vy)| (vx * w, vy * h))
            .collect();

        match self.paint_type {
            0 => p.paint_polygon(&scaled, self.fill_color),
            1 => p.paint_polygon_outlined(&scaled, self.stroke_color, self.stroke_width * w),
            2 => {
                let s = Stroke::new(self.stroke_color, self.stroke_width * w);
                p.paint_polyline_without_arrows(&scaled, &s, false);
            }
            3 => p.paint_bezier(&scaled, self.fill_color),
            4 => {
                let s = Stroke::new(self.stroke_color, self.stroke_width * w);
                p.paint_bezier_outline(&scaled, &s);
            }
            5 => {
                let s = Stroke::new(self.stroke_color, self.stroke_width * w);
                p.paint_bezier_line(&scaled, &s);
            }
            _ => p.paint_polygon(&scaled, self.fill_color),
        }

        // Draw handles
        if self.show_handles {
            let r = 0.01 * w;
            for (i, &(vx, vy)) in scaled.iter().enumerate() {
                let c = if Some(i) == self.drag_idx {
                    Color::rgba(255, 255, 255, 200)
                } else {
                    Color::rgba(0, 255, 0, 128)
                };
                p.paint_ellipse(vx, vy, r, r, c);
                let outline = Stroke::new(Color::rgba(0, 0, 0, 128), r * 0.15);
                p.paint_ellipse_outlined(vx, vy, r, r, &outline);
            }
        }

        // Help text
        p.paint_text_boxed(
            0.0,
            h - 0.05 * h,
            w,
            0.05 * h,
            "Drag vertices with left mouse button",
            0.03 * h,
            Color::WHITE,
            Color::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════

fn make_star(cx: f64, cy: f64, rx: f64, ry: f64, points: usize) -> Vec<(f64, f64)> {
    let mut verts = Vec::with_capacity(points * 2);
    for i in 0..(points * 2) {
        let a = PI * i as f64 / points as f64 - PI / 2.0;
        let r = if i % 2 == 0 { 1.0 } else { 0.4 };
        verts.push((cx + a.cos() * rx * r, cy + a.sin() * ry * r));
    }
    verts
}

// ═══════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let bg_color = Rc::new(Cell::new(Color::rgba(0x00, 0x1C, 0x38, 0xFF)));

        let root = app.tree.create_root("root");
        app.tree
            .set_behavior(root, Box::new(TestPanel::new(0, bg_color)));
        app.tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
        app.tree
            .set_auto_expansion_threshold(root, 900.0, ViewConditionType::Area);

        let close_sig = app.scheduler.create_signal();
        let win = zuicchini::window::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        app.windows.get_mut(&wid).unwrap().view_mut().flags |= ViewFlags::ROOT_SAME_TALLNESS;
    }));
    app.run();
}
