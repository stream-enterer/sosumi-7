//! TestPanel integration golden tests.
//!
//! Compares rendered output of the Rust TestPanel against C++ emTestPanel.
//! Both C++ and Rust use teddy.tga (209x256 RGBA) as the test image. Paint primitives
//! (polygons, ellipses, strokes, beziers) should match within tolerance.
//!
//! Two tests:
//! - `testpanel_root`: Root panel paint only (no auto-expansion). Tests
//!   paint primitives, text, and background rendering.
//! - `testpanel_expanded`: Full tree with auto-expanded children. Tests
//!   integration of layout, widget rendering, and multi-panel composition.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use zuicchini::foundation::{load_tga, Color, Image};
use zuicchini::input::{Cursor, InputEvent, InputState};
use zuicchini::layout::linear::{LinearGroup, LinearLayout};
use zuicchini::layout::raster::{RasterGroup, RasterLayout};
use zuicchini::layout::{ChildConstraint, Orientation};
use zuicchini::panel::{
    PanelBehavior, PanelCtx, PanelId, PanelState, PanelTree, View, ViewConditionType, ViewFlags,
};
use zuicchini::render::{
    ImageExtension, ImageQuality, LineCap, LineJoin, Painter, SoftwareCompositor, Stroke,
    StrokeEnd, StrokeEndType, TextAlignment, Texture, VAlign,
};
use zuicchini::widget::{
    Border, Button, CheckBox, CheckButton, ColorField, InnerBorderType, ListBox, Look,
    OuterBorderType, RadioBox, RadioButton, RadioGroup, ScalarField, SelectionMode, Splitter,
    TextField,
};

use super::common::*;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Settle: deliver notices and update viewing until stable.
/// `rounds` matches C++ TerminateEngine cycle count from gen_golden.cpp.
fn settle(tree: &mut PanelTree, view: &mut View, rounds: usize) {
    for _ in 0..rounds {
        tree.deliver_notices(view.window_focused(), view.pixel_tallness());
        view.update_viewing(tree);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Constants — match C++ emTestPanel
// ═══════════════════════════════════════════════════════════════════

const MAX_DEPTH: u32 = 2;
const DEFAULT_BG: Color = Color::rgba(0x00, 0x1C, 0x38, 0xFF);

const CHILD_LAYOUT: [(&str, f64, f64, f64, f64); 7] = [
    ("tktest", 0.20, 0.15, 0.30, 0.12),
    ("tp1", 0.70, 0.05, 0.12, 0.12),
    ("tp2", 0.83, 0.05, 0.12, 0.12),
    ("tp3", 0.70, 0.18, 0.12, 0.12),
    ("tp4", 0.83, 0.18, 0.12, 0.12),
    ("bgcf", 0.775, 0.34, 0.10, 0.02),
    ("polydraw", 0.05, 0.92, 0.08, 0.04),
];

// ═══════════════════════════════════════════════════════════════════
// Widget wrapper panels (from examples/test_panel.rs)
// ═══════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════
// Stub panels for unported C++ types
// ═══════════════════════════════════════════════════════════════════

/// Stub for C++ emTunnel — renders a Group border with caption, positions
/// a single child filling the content area.
struct TunnelStubPanel {
    border: Border,
    look: Rc<Look>,
}

impl TunnelStubPanel {
    fn new(caption: &str, look: Rc<Look>) -> Self {
        let border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption(caption);
        Self { border, look }
    }
}

impl PanelBehavior for TunnelStubPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        if children.is_empty() {
            return;
        }
        let rect = ctx.layout_rect();
        let cr = self.border.content_rect(rect.w, rect.h, &self.look);
        ctx.layout_child(children[0], cr.x, cr.y, cr.w, cr.h);
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

/// Stub for C++ emFileSelectionBox — renders a Group border with caption.
struct FileSelectionBoxStubPanel {
    border: Border,
    look: Rc<Look>,
}

impl FileSelectionBoxStubPanel {
    fn new(look: Rc<Look>) -> Self {
        let border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("File Selection");
        Self { border, look }
    }
}

impl PanelBehavior for FileSelectionBoxStubPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

/// Canvas panel for PolyDrawPanel — gradient background + polygon drawing.
/// Extracted from the original PolyDrawPanel.
struct CanvasPanel {
    vertices: Vec<(f64, f64)>,
    fill_color: Color,
}

impl CanvasPanel {
    fn new() -> Self {
        let n = 9;
        let vertices: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / n as f64;
                (a.cos() * 0.4 + 0.5, a.sin() * 0.4 + 0.5)
            })
            .collect();
        Self {
            vertices,
            fill_color: Color::WHITE,
        }
    }
}

impl PanelBehavior for CanvasPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        p.paint_linear_gradient(
            0.0,
            0.0,
            w,
            h,
            Color::rgba(80, 80, 160, 255),
            Color::rgba(160, 160, 80, 255),
            false,
            Color::TRANSPARENT,
        );

        let scaled: Vec<(f64, f64)> = self
            .vertices
            .iter()
            .map(|&(vx, vy)| (vx * w, vy * h))
            .collect();
        p.paint_polygon(&scaled, self.fill_color, Color::TRANSPARENT);

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

// ═══════════════════════════════════════════════════════════════════
// TestPanel — root panel (derived from examples/test_panel.rs)
// ═══════════════════════════════════════════════════════════════════

struct TestPanel {
    bg_color_shared: Rc<Cell<Color>>,
    test_image: Image,
    depth: u32,
}

impl TestPanel {
    fn new(depth: u32, bg_color_shared: Rc<Cell<Color>>) -> Self {
        let img = load_tga(include_bytes!("assets/teddy.tga")).expect("failed to load teddy.tga");
        Self {
            bg_color_shared,
            test_image: img,
            depth,
        }
    }

    fn bg_color(&self) -> Color {
        self.bg_color_shared.get()
    }

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
        p.paint_rect(
            0.25,
            0.80,
            0.05,
            0.05,
            Color::rgba(255, 0, 0, 32),
            Color::TRANSPARENT,
        );

        // Triangle
        p.paint_polygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg, bg);

        // Holed polygon (non-zero winding, same-direction inner — C++ PaintPolygon)
        p.paint_polygon(
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
            bg,
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
            bg,
        );

        // Circle (polygon approximation)
        let circle: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.paint_polygon(&circle, Color::YELLOW, bg);

        // Clipped circle
        p.push_state();
        p.clip_rect(0.51, 0.81, 0.08, 0.08);
        let circle2: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.paint_polygon(&circle2, Color::GREEN, bg);
        p.pop_state();

        // Ellipse (polygon)
        let ellipse: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        p.paint_polygon(&ellipse, Color::rgba(255, 0, 0, 92), Color::TRANSPARENT);

        // More triangles
        p.paint_polygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            Color::rgba(187, 255, 255, 255),
            bg,
        );
        p.paint_polygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], Color::RED, bg);
        p.paint_polygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            Color::rgba(187, 255, 255, 255),
            Color::TRANSPARENT,
        );
        p.paint_polygon(
            &[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)],
            Color::RED,
            Color::TRANSPARENT,
        );

        // Thin triangles
        p.paint_polygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            Color::rgba(187, 136, 255, 192),
            Color::TRANSPARENT,
        );
        p.paint_polygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            Color::rgba(136, 187, 255, 192),
            Color::TRANSPARENT,
        );
        p.paint_polygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            Color::rgba(136, 187, 255, 192),
            Color::TRANSPARENT,
        );

        // Ellipses (center + radius)
        p.paint_ellipse(0.055, 0.805, 0.005, 0.005, Color::WHITE, bg);
        p.paint_ellipse(0.07, 0.805, 0.01, 0.005, Color::WHITE, bg);
        p.paint_ellipse(0.0925, 0.805, 0.0025, 0.005, Color::WHITE, bg);

        // Ellipse sectors
        p.paint_ellipse_sector(0.105, 0.805, 0.005, 0.005, 45.0, 305.0, Color::WHITE, bg);
        p.paint_ellipse_sector(0.12, 0.805, 0.01, 0.005, 45.0, -395.0, Color::WHITE, bg);

        // Rect outlines
        p.paint_rect_outlined(
            0.05,
            0.82,
            0.01,
            0.01,
            &Stroke::new(Color::WHITE, 0.001),
            bg,
        );
        p.paint_rect_outlined(
            0.10,
            0.82,
            0.01,
            0.01,
            &Stroke::new(Color::WHITE, 0.008),
            bg,
        );

        // Round rects
        p.set_canvas_color(bg);
        p.paint_round_rect(0.05, 0.84, 0.01, 0.01, 0.001, Color::WHITE);
        p.paint_round_rect(0.07, 0.84, 0.02, 0.01, 0.002, Color::WHITE);
        p.paint_round_rect(0.10, 0.84, 0.01, 0.01, 0.003, Color::WHITE);

        // Ellipse outlines
        p.paint_ellipse_outlined(
            0.055,
            0.865,
            0.005,
            0.005,
            &Stroke::new(Color::WHITE, 0.003),
            bg,
        );
        p.paint_ellipse_outlined(
            0.075,
            0.865,
            0.01,
            0.005,
            &Stroke::new(Color::WHITE, 0.001),
            bg,
        );

        // Round rect outlines
        p.set_canvas_color(bg);
        p.paint_round_rect_outlined(
            0.05,
            0.88,
            0.01,
            0.01,
            0.001,
            &Stroke::new(Color::WHITE, 0.001),
        );
        p.paint_round_rect_outlined(
            0.07,
            0.88,
            0.02,
            0.01,
            0.002,
            &Stroke::new(Color::WHITE, 0.001),
        );

        // Bezier curves
        p.paint_bezier(
            &[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)],
            Color::WHITE,
            bg,
        );
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
            bg,
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
            bg,
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
            bg,
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
                Color::TRANSPARENT,
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
            bg,
        );

        // Polygon outline
        p.paint_polygon_outlined(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            Color::RED,
            0.0002,
            Color::TRANSPARENT,
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
            Color::TRANSPARENT,
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
            Color::TRANSPARENT,
        );

        let star3 = make_star(0.255, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star3,
            &Texture::Image {
                image: self.test_image.clone(),
                extension: ImageExtension::Repeat,
                quality: ImageQuality::Bilinear,
            },
            Color::TRANSPARENT,
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
            Color::TRANSPARENT,
        );
        p.paint_radial_gradient(
            0.225,
            0.946,
            0.004,
            0.008,
            Color::rgba(0xFF, 0x88, 0, 0xFF),
            Color::rgba(0, 0x55, 0, 0xFF),
            Color::TRANSPARENT,
        );
        p.paint_ellipse(
            0.24,
            0.945,
            0.01,
            0.005,
            Color::rgba(0, 0xCC, 0x88, 0xFF),
            Color::TRANSPARENT,
        );

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

        painter.push_state();
        painter.scale(w, w);
        let panel_h = h / w;

        painter.paint_rect(0.0, 0.0, 1.0, panel_h, bg, Color::TRANSPARENT);
        painter.paint_rect_outlined(0.01, 0.01, 0.98, panel_h - 0.02, &Stroke::new(fg, 0.02), bg);

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

        // C++ emTestPanel.cpp:152 uses %f (6 decimal places) for priority.
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

        // Paint primitives
        self.paint_primitives(painter, fg, bg);

        painter.pop_state();
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();

        if !children.is_empty() {
            for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
                if let Some(child) = ctx.find_child_by_name(name) {
                    ctx.layout_child(child, x, y, cw, ch);
                }
            }
            return;
        }

        // Create children — matches C++ AutoExpand()
        let bg_shared = self.bg_color_shared.clone();

        ctx.create_child_with("tktest", Box::new(TkTestGrpPanel::new()));

        if self.depth < MAX_DEPTH {
            for i in 1..=4 {
                let child_bg = Rc::new(Cell::new(DEFAULT_BG));
                ctx.create_child_with(
                    &format!("tp{i}"),
                    Box::new(TestPanel::new(self.depth + 1, child_bg)),
                );
            }
        }

        let bg_for_cf = bg_shared.clone();
        let mut cf = ColorField::new(Look::new());
        cf.set_editable(true);
        cf.set_alpha_enabled(true);
        cf.set_color(bg_shared.get());
        cf.on_color = Some(Box::new(move |color| {
            bg_for_cf.set(color);
        }));
        ctx.create_child_with("bgcf", Box::new(ColorFieldPanel { widget: cf }));

        ctx.create_child_with("polydraw", Box::new(PolyDrawPanel::new()));

        for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
            if let Some(child) = ctx.find_child_by_name(name) {
                ctx.layout_child(child, x, y, cw, ch);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// TkTestGrpPanel — splitter hierarchy hosting TkTest widget showcases
// ═══════════════════════════════════════════════════════════════════

struct TkTestGrpPanel {
    border: Border,
    look: Rc<Look>,
    children_created: bool,
}

impl TkTestGrpPanel {
    fn new() -> Self {
        let look = Look::new();
        let border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Toolkit Test");
        Self {
            border,
            look,
            children_created: false,
        }
    }
}

impl PanelBehavior for TkTestGrpPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;
            let look = self.look.clone();

            // sp: horizontal splitter, pos=0.8 (C++ emTestPanel.cpp:889)
            let mut sp = Splitter::new(Orientation::Horizontal, look.clone());
            sp.set_position(0.8);
            let sp_id = ctx.create_child_with("sp", Box::new(SplitterPanel { widget: sp }));

            // sp1: vertical splitter, child of sp, pos=0.8
            let mut sp1 = Splitter::new(Orientation::Vertical, look.clone());
            sp1.set_position(0.8);
            let sp1_id = ctx.tree.create_child(sp_id, "sp1");
            ctx.tree
                .set_behavior(sp1_id, Box::new(SplitterPanel { widget: sp1 }));

            // t1a, t1b: children of sp1
            let t1a_id = ctx.tree.create_child(sp1_id, "t1a");
            ctx.tree
                .set_behavior(t1a_id, Box::new(TkTestPanel::new(look.clone())));
            let t1b_id = ctx.tree.create_child(sp1_id, "t1b");
            ctx.tree
                .set_behavior(t1b_id, Box::new(TkTestPanel::new(look.clone())));

            // sp2: vertical splitter, child of sp, pos=0.8
            let mut sp2 = Splitter::new(Orientation::Vertical, look.clone());
            sp2.set_position(0.8);
            let sp2_id = ctx.tree.create_child(sp_id, "sp2");
            ctx.tree
                .set_behavior(sp2_id, Box::new(SplitterPanel { widget: sp2 }));

            // t2a: child of sp2
            let t2a_id = ctx.tree.create_child(sp2_id, "t2a");
            ctx.tree
                .set_behavior(t2a_id, Box::new(TkTestPanel::new(look.clone())));

            // t2b: child of sp2, disabled, caption="Disabled"
            let t2b_id = ctx.tree.create_child(sp2_id, "t2b");
            ctx.tree.set_behavior(
                t2b_id,
                Box::new(TkTestPanel::new_with_caption(look, "Disabled")),
            );
            ctx.tree.set_enable_switch(t2b_id, false);
        }

        // Position sp in border content rect
        let cr = self.border.content_rect(rect.w, rect.h, &self.look);
        if let Some(sp) = ctx.find_child_by_name("sp") {
            ctx.layout_child(sp, cr.x, cr.y, cr.w, cr.h);
        }
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ═══════════════════════════════════════════════════════════════════
// TkTestPanel — widget showcase grid
// ═══════════════════════════════════════════════════════════════════

struct TkTestPanel {
    look: Rc<Look>,
    border: Border,
    children_created: bool,
}

impl TkTestPanel {
    fn new(look: Rc<Look>) -> Self {
        let border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Toolkit Test");
        Self {
            look,
            border,
            children_created: false,
        }
    }

    fn new_with_caption(look: Rc<Look>, caption: &str) -> Self {
        let border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption(caption);
        Self {
            look,
            border,
            children_created: false,
        }
    }

    /// Helper: create a RasterGroup category under `parent`.
    fn make_category(
        tree: &mut PanelTree,
        parent: PanelId,
        name: &str,
        caption: &str,
        pct: Option<f64>,
        fixed_cols: Option<usize>,
    ) -> PanelId {
        let mut rg = RasterGroup::new();
        rg.border.set_border_scaling(2.5);
        rg.border.caption = caption.to_string();
        if let Some(p) = pct {
            rg.layout.preferred_child_tallness = p;
        }
        if let Some(c) = fixed_cols {
            rg.layout.fixed_columns = Some(c);
        }
        let id = tree.create_child(parent, name);
        tree.set_behavior(id, Box::new(rg));
        id
    }

    fn create_all_categories(&self, ctx: &mut PanelCtx, grid_id: PanelId) {
        let look = self.look.clone();

        // 1. Buttons (C++ emTestPanel.cpp:558-576)
        let gid = Self::make_category(ctx.tree, grid_id, "buttons", "Buttons", None, None);
        {
            let id = ctx.tree.create_child(gid, "b1");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: Button::new("Button", look.clone()),
                }),
            );

            let mut b2 = Button::new("Button", look.clone());
            b2.set_description(
                "This is a long description for testing.\n\
                 It has multiple lines.\n\
                 Third line here.",
            );
            let id = ctx.tree.create_child(gid, "b2");
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b2 }));

            let mut b3 = Button::new("Button", look.clone());
            b3.set_no_eoi(true);
            let id = ctx.tree.create_child(gid, "b3");
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b3 }));
        }

        // 2. Check Buttons and Boxes (C++ :578-598)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "checkbuttons",
            "Check Buttons and Boxes",
            None,
            None,
        );
        {
            for i in 1..=3 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(CheckButtonPanel {
                        widget: CheckButton::new("Check Button", look.clone()),
                    }),
                );
            }
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(CheckBoxPanel {
                        widget: CheckBox::new("Check Box", look.clone()),
                    }),
                );
            }
        }

        // 3. Radio Buttons and Boxes (C++ :600-624)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "radiobuttons",
            "Radio Buttons and Boxes",
            None,
            None,
        );
        {
            let rg = RadioGroup::new();
            for i in 1..=3 {
                let id = ctx.tree.create_child(gid, &format!("r{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(RadioButtonPanel {
                        widget: RadioButton::new("Radio Button", look.clone(), rg.clone(), i - 1),
                    }),
                );
            }
            let rg2 = RadioGroup::new();
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("r{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(RadioBoxPanel {
                        widget: RadioBox::new("Radio Box", look.clone(), rg2.clone(), i - 4),
                    }),
                );
            }
        }

        // 4. Text Fields (C++ :626-656)
        let gid = Self::make_category(ctx.tree, grid_id, "textfields", "Text Fields", None, None);
        {
            let mut tf1 = TextField::new(look.clone());
            tf1.set_text("Read-Only");
            let id = ctx.tree.create_child(gid, "tf1");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf1 }));

            let mut tf2 = TextField::new(look.clone());
            tf2.set_editable(true);
            tf2.set_text("Editable");
            let id = ctx.tree.create_child(gid, "tf2");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf2 }));

            let mut tf3 = TextField::new(look.clone());
            tf3.set_editable(true);
            tf3.set_text("Password");
            tf3.set_password_mode(true);
            let id = ctx.tree.create_child(gid, "tf3");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf3 }));

            let mut mltf1 = TextField::new(look.clone());
            mltf1.set_editable(true);
            mltf1.set_multi_line(true);
            mltf1.set_text("first line\nsecond line\n...");
            let id = ctx.tree.create_child(gid, "mltf1");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: mltf1 }));
        }

        // 5. Scalar Fields (C++ :658-712)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "scalarfields",
            "Scalar Fields",
            Some(0.1),
            None,
        );
        {
            let id = ctx.tree.create_child(gid, "sf1");
            ctx.tree.set_behavior(
                id,
                Box::new(ScalarFieldPanel {
                    widget: ScalarField::new(0.0, 100.0, look.clone()),
                }),
            );

            let mut sf2 = ScalarField::new(0.0, 100.0, look.clone());
            sf2.set_editable(true);
            let id = ctx.tree.create_child(gid, "sf2");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf2 }));

            let mut sf3 = ScalarField::new(-1000.0, 1000.0, look.clone());
            sf3.set_editable(true);
            sf3.set_scale_mark_intervals(&[1000, 100, 10, 5, 1]);
            let id = ctx.tree.create_child(gid, "sf3");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf3 }));

            // sf4: Level 1-5, val=3, custom format, text_box_tallness=0.25
            let mut sf4 = ScalarField::new(1.0, 5.0, look.clone());
            sf4.set_editable(true);
            sf4.set_value(3.0);
            sf4.set_text_box_tallness(0.25);
            sf4.set_text_of_value_fn(Box::new(|val, _interval| format!("Level {val}")));
            let id = ctx.tree.create_child(gid, "sf4");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf4 }));

            // sf5: PlayLength, time format
            let mut sf5 = ScalarField::new(0.0, 86400000.0, look.clone());
            sf5.set_editable(true);
            sf5.set_value(14400000.0);
            sf5.set_scale_mark_intervals(&[
                86400000, 43200000, 21600000, 10800000, 3600000, 1800000, 600000, 300000, 60000,
                30000, 10000, 5000, 1000,
            ]);
            sf5.set_text_of_value_fn(Box::new(|val, _interval| {
                let ms = val.unsigned_abs();
                let s = ms / 1000;
                let m = s / 60;
                let h = m / 60;
                format!("{:02}:{:02}:{:02}", h, m % 60, s % 60)
            }));
            let id = ctx.tree.create_child(gid, "sf5");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf5 }));

            // sf6: PlayPos, same time format, max=sf5.value
            let mut sf6 = ScalarField::new(0.0, 14400000.0, look.clone());
            sf6.set_editable(true);
            sf6.set_text_of_value_fn(Box::new(|val, _interval| {
                let ms = val.unsigned_abs();
                let s = ms / 1000;
                let m = s / 60;
                let h = m / 60;
                format!("{:02}:{:02}:{:02}", h, m % 60, s % 60)
            }));
            let id = ctx.tree.create_child(gid, "sf6");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf6 }));
        }

        // 6. Color Fields (C++ :714-733)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "colorfields",
            "Color Fields",
            Some(0.4),
            None,
        );
        {
            let mut cf1 = ColorField::new(look.clone());
            cf1.set_color(Color::rgba(0xBB, 0x22, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf1");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf1 }));

            let mut cf2 = ColorField::new(look.clone());
            cf2.set_editable(true);
            cf2.set_color(Color::rgba(0x22, 0xBB, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf2");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf2 }));

            let mut cf3 = ColorField::new(look.clone());
            cf3.set_editable(true);
            cf3.set_alpha_enabled(true);
            cf3.set_color(Color::rgba(0x22, 0x22, 0xBB, 0xFF));
            let id = ctx.tree.create_child(gid, "cf3");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf3 }));
        }

        // 7. Tunnels (C++ :735-754) — stub panels, emTunnel not ported
        let gid = Self::make_category(ctx.tree, grid_id, "tunnels", "Tunnels", Some(0.4), None);
        {
            let tunnel_info: [(&str, &str); 4] = [
                ("t1", "Tunnel"),
                ("t2", "Deeper Tunnel"),
                ("t3", "Square End"),
                ("t4", "Square End, Zero Depth"),
            ];
            for (name, caption) in &tunnel_info {
                let tid = ctx.tree.create_child(gid, name);
                ctx.tree
                    .set_behavior(tid, Box::new(TunnelStubPanel::new(caption, look.clone())));
                // Each tunnel has a child button inside
                let child = ctx.tree.create_child(tid, "child");
                ctx.tree.set_behavior(
                    child,
                    Box::new(ButtonPanel {
                        widget: Button::new("Inside", look.clone()),
                    }),
                );
            }
        }

        // 8. List Boxes (C++ :756-798)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "listboxes",
            "List Boxes",
            Some(0.4),
            None,
        );
        {
            let items7: Vec<String> = (1..=7).map(|i| format!("Item {i}")).collect();

            let id = ctx.tree.create_child(gid, "l1");
            ctx.tree.set_behavior(
                id,
                Box::new(ListBoxPanel {
                    widget: ListBox::new(look.clone()),
                }),
            );

            let mut lb2 = ListBox::new(look.clone());
            lb2.set_selection_mode(SelectionMode::Single);
            lb2.set_items(items7.clone());
            let id = ctx.tree.create_child(gid, "l2");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb2 }));

            let mut lb3 = ListBox::new(look.clone());
            lb3.set_selection_mode(SelectionMode::ReadOnly);
            lb3.set_items(items7.clone());
            let id = ctx.tree.create_child(gid, "l3");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb3 }));

            let mut lb4 = ListBox::new(look.clone());
            lb4.set_selection_mode(SelectionMode::Multi);
            lb4.set_items(items7.clone());
            let id = ctx.tree.create_child(gid, "l4");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb4 }));

            let mut lb5 = ListBox::new(look.clone());
            lb5.set_selection_mode(SelectionMode::Toggle);
            lb5.set_items(items7.clone());
            let id = ctx.tree.create_child(gid, "l5");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb5 }));

            // l6: single column
            let mut lb6 = ListBox::new(look.clone());
            lb6.set_selection_mode(SelectionMode::Single);
            lb6.set_items(items7.clone());
            lb6.set_fixed_column_count(Some(1));
            let id = ctx.tree.create_child(gid, "l6");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb6 }));

            // l7: regular listbox (custom panels not ported)
            let mut lb7 = ListBox::new(look.clone());
            lb7.set_selection_mode(SelectionMode::Single);
            lb7.set_items(items7);
            let id = ctx.tree.create_child(gid, "l7");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb7 }));
        }

        // 9. Test Dialog (C++ :800-831)
        let gid = Self::make_category(ctx.tree, grid_id, "dlgs", "Test Dialog", None, Some(1));
        {
            // RasterLayout with checkboxes
            let mut rl = RasterLayout::new();
            rl.preferred_child_tallness = 0.1;
            let rl_id = ctx.tree.create_child(gid, "rl");

            let cb_names = [
                "CbTopLev",
                "CbPZoom",
                "CbModal",
                "CbFullscreen",
                "CbPopup",
                "CbUndec",
                "CbResizable",
            ];
            for name in &cb_names {
                let id = ctx.tree.create_child(rl_id, name);
                ctx.tree.set_behavior(
                    id,
                    Box::new(CheckBoxPanel {
                        widget: CheckBox::new(name, look.clone()),
                    }),
                );
            }
            ctx.tree.set_behavior(rl_id, Box::new(rl));

            // Button
            let id = ctx.tree.create_child(gid, "dlgButton");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: Button::new("Test Dialog...", look.clone()),
                }),
            );
        }

        // 10. File Selection (C++ :833-858) — stub, emFileSelectionBox not ported
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "fileChoosers",
            "File Selection",
            Some(0.3),
            None,
        );
        {
            let id = ctx.tree.create_child(gid, "fsb");
            ctx.tree
                .set_behavior(id, Box::new(FileSelectionBoxStubPanel::new(look.clone())));

            let id = ctx.tree.create_child(gid, "open");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: Button::new("Open", look.clone()),
                }),
            );

            let id = ctx.tree.create_child(gid, "openMulti");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: Button::new("Open Multi", look.clone()),
                }),
            );

            let id = ctx.tree.create_child(gid, "saveAs");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: Button::new("Save As", look.clone()),
                }),
            );
        }
    }
}

impl PanelBehavior for TkTestPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;

            // Create grid child with RasterLayout (PCT=0.3)
            let mut layout = RasterLayout::new();
            layout.preferred_child_tallness = 0.3;
            let grid_id = ctx.create_child_with("grid", Box::new(layout));

            // Create all 10 category groups under the grid
            self.create_all_categories(ctx, grid_id);
        }

        // Position grid in border content rect
        let cr = self.border.content_rect(rect.w, rect.h, &self.look);
        if let Some(grid) = ctx.find_child_by_name("grid") {
            ctx.layout_child(grid, cr.x, cr.y, cr.w, cr.h);
        }
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ═══════════════════════════════════════════════════════════════════
// PolyDrawPanel — polygon drawing with star shape
// ═══════════════════════════════════════════════════════════════════

struct PolyDrawPanel {
    border: Border,
    look: Rc<Look>,
    children_created: bool,
}

impl PolyDrawPanel {
    fn new() -> Self {
        let look = Look::new();
        let border = Border::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Poly Draw Test")
            .with_description(
                "This panel demonstrates the polygon drawing capabilities.\n\
                 You can modify the polygon by dragging vertices.",
            );
        Self {
            border,
            look,
            children_created: false,
        }
    }

    /// Create the 16-method RadioBox group under a parent.
    fn create_method_radio(tree: &mut PanelTree, parent: PanelId, look: &Rc<Look>) -> PanelId {
        let mut rg = RasterGroup::new();
        rg.border.set_border_scaling(1.5);
        rg.border.caption = "Method".to_string();
        rg.layout.preferred_child_tallness = 0.07;
        let mid = tree.create_child(parent, "Method");

        let method_group = RadioGroup::new();
        let names = [
            "PaintPolygon",
            "PaintPolygonOutline",
            "PaintPolyline",
            "PaintBezier",
            "PaintBezierOutline",
            "PaintBezierLine",
            "PaintLine",
            "PaintRect",
            "PaintRectOutline",
            "PaintEllipse",
            "PaintEllipseOutline",
            "PaintEllipseSector",
            "PaintEllipseSectorOutline",
            "PaintEllipseArc",
            "PaintRoundRect",
            "PaintRoundRectOutline",
        ];
        for (i, name) in names.iter().enumerate() {
            let id = tree.create_child(mid, name);
            tree.set_behavior(
                id,
                Box::new(RadioBoxPanel {
                    widget: RadioBox::new(name, look.clone(), method_group.clone(), i),
                }),
            );
        }
        tree.set_behavior(mid, Box::new(rg));
        mid
    }

    /// Create a 4-option dash type RadioBox group.
    fn create_dash_radio(tree: &mut PanelTree, parent: PanelId, look: &Rc<Look>) -> PanelId {
        let mut rg = RasterGroup::new();
        rg.border.set_border_scaling(1.5);
        rg.border.caption = "Dash Type".to_string();
        rg.layout.preferred_child_tallness = 0.08;
        let did = tree.create_child(parent, "StrokeDashType");

        let dash_group = RadioGroup::new();
        let names = ["Solid", "Dashed", "Dotted", "DashDotted"];
        for (i, name) in names.iter().enumerate() {
            let id = tree.create_child(did, name);
            tree.set_behavior(
                id,
                Box::new(RadioBoxPanel {
                    widget: RadioBox::new(name, look.clone(), dash_group.clone(), i),
                }),
            );
        }
        tree.set_behavior(did, Box::new(rg));
        did
    }

    /// Create a 17-option stroke end type RadioBox group.
    fn create_stroke_end_radio(
        tree: &mut PanelTree,
        parent: PanelId,
        name: &str,
        caption: &str,
        look: &Rc<Look>,
    ) -> PanelId {
        let mut rg = RasterGroup::new();
        rg.border.set_border_scaling(1.5);
        rg.border.caption = caption.to_string();
        rg.layout.preferred_child_tallness = 0.08;
        let sid = tree.create_child(parent, name);

        let group = RadioGroup::new();
        let names = [
            "Butt",
            "Cap",
            "Arrow",
            "ContourArrow",
            "LineArrow",
            "Triangle",
            "ContourTriangle",
            "Square",
            "ContourSquare",
            "HalfSquare",
            "Circle",
            "ContourCircle",
            "HalfCircle",
            "Diamond",
            "ContourDiamond",
            "HalfDiamond",
            "Stroke",
        ];
        for (i, n) in names.iter().enumerate() {
            let id = tree.create_child(sid, n);
            tree.set_behavior(
                id,
                Box::new(RadioBoxPanel {
                    widget: RadioBox::new(n, look.clone(), group.clone(), i),
                }),
            );
        }
        tree.set_behavior(sid, Box::new(rg));
        sid
    }

    /// Create a horizontal LinearLayout with 2 children (TextField + widget).
    fn create_horizontal_pair(
        tree: &mut PanelTree,
        parent: PanelId,
        name: &str,
        child1_name: &str,
        child1: Box<dyn PanelBehavior>,
        child2_name: &str,
        child2: Box<dyn PanelBehavior>,
    ) -> PanelId {
        let ll_id = tree.create_child(parent, name);
        let c1 = tree.create_child(ll_id, child1_name);
        tree.set_behavior(c1, child1);
        let c2 = tree.create_child(ll_id, child2_name);
        tree.set_behavior(c2, child2);
        tree.set_behavior(ll_id, Box::new(LinearLayout::horizontal()));
        ll_id
    }

    fn create_controls(&self, ctx: &mut PanelCtx, layout_id: PanelId) {
        let look = self.look.clone();

        // Controls: RasterLayout with PCT=0.6
        let ctrl_id = ctx.tree.create_child(layout_id, "Controls");

        // ── general section ──
        let gen_id = ctx.tree.create_child(ctrl_id, "general");

        let method_id = Self::create_method_radio(ctx.tree, gen_id, &look);

        Self::create_horizontal_pair(
            ctx.tree,
            gen_id,
            "ll",
            "VertexCount",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("9");
                    tf
                },
            }),
            "FillColor",
            Box::new(ColorFieldPanel {
                widget: {
                    let mut cf = ColorField::new(look.clone());
                    cf.set_editable(true);
                    cf.set_alpha_enabled(true);
                    cf.set_color(Color::WHITE);
                    cf
                },
            }),
        );

        Self::create_horizontal_pair(
            ctx.tree,
            gen_id,
            "ll2",
            "StrokeWidth",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("0.01");
                    tf
                },
            }),
            "WithCanvasColor",
            Box::new(CheckBoxPanel {
                widget: CheckBox::new("With Canvas Color", look.clone()),
            }),
        );

        // Set general behavior with weight on Method
        let mut gen_group = LinearGroup::vertical();
        gen_group.border.set_border_scaling(2.0);
        gen_group.border.caption = "General".to_string();
        gen_group.layout.set_child_constraint(
            method_id,
            ChildConstraint {
                weight: 2.0,
                ..Default::default()
            },
        );
        ctx.tree.set_behavior(gen_id, Box::new(gen_group));

        // ── stroke section ──
        let stroke_id = ctx.tree.create_child(ctrl_id, "stroke");

        let stroke_color_id = ctx.tree.create_child(stroke_id, "StrokeColor");
        ctx.tree.set_behavior(
            stroke_color_id,
            Box::new(ColorFieldPanel {
                widget: {
                    let mut cf = ColorField::new(look.clone());
                    cf.set_editable(true);
                    cf.set_alpha_enabled(true);
                    cf.set_color(Color::rgba(0, 0, 0, 0xFF));
                    cf
                },
            }),
        );

        let rounded_id = ctx.tree.create_child(stroke_id, "StrokeRounded");
        ctx.tree.set_behavior(
            rounded_id,
            Box::new(CheckBoxPanel {
                widget: CheckBox::new("Rounded", look.clone()),
            }),
        );

        let dash_id = Self::create_dash_radio(ctx.tree, stroke_id, &look);

        Self::create_horizontal_pair(
            ctx.tree,
            stroke_id,
            "ll",
            "DashLengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("1.0");
                    tf
                },
            }),
            "GapLengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("1.0");
                    tf
                },
            }),
        );

        let mut stroke_group = LinearGroup::vertical();
        stroke_group.border.set_border_scaling(2.0);
        stroke_group.border.caption = "Stroke".to_string();
        stroke_group.layout.set_child_constraint(
            dash_id,
            ChildConstraint {
                weight: 2.0,
                ..Default::default()
            },
        );
        ctx.tree.set_behavior(stroke_id, Box::new(stroke_group));

        // ── strokeStart section ──
        let ss_id = ctx.tree.create_child(ctrl_id, "strokeStart");

        let ss_type_id =
            Self::create_stroke_end_radio(ctx.tree, ss_id, "StrokeStartType", "Type", &look);

        let ss_color_id = ctx.tree.create_child(ss_id, "StrokeStartInnerColor");
        ctx.tree.set_behavior(
            ss_color_id,
            Box::new(ColorFieldPanel {
                widget: {
                    let mut cf = ColorField::new(look.clone());
                    cf.set_editable(true);
                    cf.set_alpha_enabled(true);
                    cf.set_color(Color::rgba(0xEE, 0xEE, 0xEE, 0xFF));
                    cf
                },
            }),
        );

        Self::create_horizontal_pair(
            ctx.tree,
            ss_id,
            "ll",
            "WidthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("1.0");
                    tf
                },
            }),
            "LengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("1.0");
                    tf
                },
            }),
        );

        let mut ss_group = LinearGroup::vertical();
        ss_group.border.set_border_scaling(2.0);
        ss_group.border.caption = "Stroke Start".to_string();
        ss_group.layout.set_child_constraint(
            ss_type_id,
            ChildConstraint {
                weight: 2.0,
                ..Default::default()
            },
        );
        ctx.tree.set_behavior(ss_id, Box::new(ss_group));

        // ── strokeEnd section ──
        let se_id = ctx.tree.create_child(ctrl_id, "strokeEnd");

        let se_type_id =
            Self::create_stroke_end_radio(ctx.tree, se_id, "StrokeEndType", "Type", &look);

        let se_color_id = ctx.tree.create_child(se_id, "StrokeEndInnerColor");
        ctx.tree.set_behavior(
            se_color_id,
            Box::new(ColorFieldPanel {
                widget: {
                    let mut cf = ColorField::new(look.clone());
                    cf.set_editable(true);
                    cf.set_alpha_enabled(true);
                    cf.set_color(Color::rgba(0xEE, 0xEE, 0xEE, 0xFF));
                    cf
                },
            }),
        );

        Self::create_horizontal_pair(
            ctx.tree,
            se_id,
            "ll",
            "WidthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("1.0");
                    tf
                },
            }),
            "LengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = TextField::new(look.clone());
                    tf.set_editable(true);
                    tf.set_text("1.0");
                    tf
                },
            }),
        );

        let mut se_group = LinearGroup::vertical();
        se_group.border.set_border_scaling(2.0);
        se_group.border.caption = "Stroke End".to_string();
        se_group.layout.set_child_constraint(
            se_type_id,
            ChildConstraint {
                weight: 2.0,
                ..Default::default()
            },
        );
        ctx.tree.set_behavior(se_id, Box::new(se_group));

        // Set Controls behavior (RasterLayout, PCT=0.6)
        let mut ctrl_layout = RasterLayout::new();
        ctrl_layout.preferred_child_tallness = 0.6;
        ctx.tree.set_behavior(ctrl_id, Box::new(ctrl_layout));

        // ── CanvasPanel ──
        let canvas_id = ctx.tree.create_child(layout_id, "CanvasPanel");
        ctx.tree
            .set_behavior(canvas_id, Box::new(CanvasPanel::new()));
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;

            // LinearLayout child (adaptive, threshold=1.0)
            let layout_id = ctx.create_child("layout");
            self.create_controls(ctx, layout_id);

            // Set behavior last — the adaptive LinearLayout
            ctx.tree
                .set_behavior(layout_id, Box::new(LinearLayout::adaptive(1.0)));
        }

        // Position layout in border content rect
        let cr = self.border.content_rect(rect.w, rect.h, &self.look);
        if let Some(layout) = ctx.find_child_by_name("layout") {
            ctx.layout_child(layout, cr.x, cr.y, cr.w, cr.h);
        }
        let cc = self
            .border
            .content_canvas_color(ctx.canvas_color(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

fn make_star(cx: f64, cy: f64, rx: f64, ry: f64, points: usize) -> Vec<(f64, f64)> {
    let mut verts = Vec::with_capacity(points * 2);
    for i in 0..(points * 2) {
        let a = PI * i as f64 / points as f64 - PI / 2.0;
        let r = if i % 2 == 0 { 1.0 } else { 0.4 };
        verts.push((cx + a.cos() * rx * r, cy + a.sin() * ry * r));
    }
    verts
}

fn render_testpanel(
    name: &str,
    tree: &mut PanelTree,
    view: &mut View,
    expected: &(u32, u32, Vec<u8>),
    channel_tolerance: u8,
    max_failure_pct: f64,
    settle_rounds: usize,
) {
    let (w, h, ref expected_data) = *expected;

    settle(tree, view, settle_rounds);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(tree, view);
    let actual = compositor.framebuffer().data();

    let result = compare_images(
        name,
        actual,
        expected_data,
        w,
        h,
        channel_tolerance,
        max_failure_pct,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images(name, actual, expected_data, w, h);
        analyze_diff_distribution(actual, expected_data, w, h, channel_tolerance);
    }
    result.unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

/// Root panel paint only — no auto-expansion, tests paint primitives.
/// Known diffs: some ellipse sector/round rect parameters differ,
/// and text rendering variance (runtime values like Pri/MemLim).
#[test]
fn testpanel_root() {
    require_golden!();
    let expected = load_compositor_golden("testpanel_root");

    let bg_color = Rc::new(Cell::new(DEFAULT_BG));
    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.set_behavior(root, Box::new(TestPanel::new(0, bg_color)));
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    // Very high threshold prevents auto-expansion (matches C++ gen)
    tree.set_auto_expansion_threshold(root, 1e9, ViewConditionType::Area);

    let mut view = View::new(root, 1000.0, 1000.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window — match unfocused state
    view.set_window_focused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 30)
    render_testpanel(
        "testpanel_root",
        &mut tree,
        &mut view,
        &expected,
        3,
        12.0,
        30,
    );
}

/// Full TestPanel tree with auto-expanded children — integration test.
/// Remaining diffs from text value differences (Pri/MemLim runtime values),
/// border positioning, and font rendering (~12%).
#[test]
fn testpanel_expanded() {
    require_golden!();
    let expected = load_compositor_golden("testpanel_expanded");

    let bg_color = Rc::new(Cell::new(DEFAULT_BG));
    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.set_behavior(root, Box::new(TestPanel::new(0, bg_color)));
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    // C++ default threshold: 900 (VCT_AREA). At 1000x1000, vc=1e6 > 900 → expands.
    tree.set_auto_expansion_threshold(root, 900.0, ViewConditionType::Area);

    let mut view = View::new(root, 1000.0, 1000.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window — match unfocused state
    view.set_window_focused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    render_testpanel(
        "testpanel_expanded",
        &mut tree,
        &mut view,
        &expected,
        3,
        10.5,
        200,
    );
}
