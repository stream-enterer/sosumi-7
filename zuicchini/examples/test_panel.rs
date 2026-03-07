use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use zuicchini::foundation::{Color, Image};
use zuicchini::input::{InputEvent, InputKey, InputVariant};
use zuicchini::layout::Orientation;
use zuicchini::panel::{
    NoticeFlags, PanelBehavior, PanelCtx, PanelId, PanelState, ViewConditionType, ViewFlags,
};
use zuicchini::render::{
    ImageExtension, ImageQuality, LineCap, LineJoin, Painter, Stroke, StrokeEnd, StrokeEndType,
    TextAlignment, Texture,
};
use zuicchini::widget::{
    Button, CheckBox, CheckButton, ColorField, Label, ListBox, Look, RadioBox, RadioButton,
    RadioGroup, ScalarField, SelectionMode, Splitter, TextField,
};
use zuicchini::window::{App, WindowFlags, ZuiWindow};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// TestPanel (root panel, Phases 1-4)
// ---------------------------------------------------------------------------

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

    fn paint_primitives(&self, painter: &mut Painter, fg: Color) {
        // Text test box
        painter.paint_text_boxed(
            0.25,
            0.8,
            0.05,
            0.05,
            "Text Test\n\t<-tab\ntab->\t<-tab",
            0.1,
            fg,
            TextAlignment::Center,
        );
        painter.paint_rect(0.25, 0.8, 0.05, 0.05, Color::rgba(255, 0, 0, 32));

        // Triangle
        painter.paint_polygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg);

        // Holed polygon (even-odd)
        painter.paint_polygon_even_odd(
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
        painter.paint_polygon(
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

        // Circle
        let circle: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.paint_polygon(&circle, Color::rgba(255, 255, 0, 255));

        // Clipped circle
        let clipped: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.push_state();
        painter.clip_rect(0.51, 0.81, 0.08, 0.08);
        painter.paint_polygon(&clipped, Color::rgba(0, 255, 0, 255));
        painter.pop_state();

        // Ellipse
        let ellipse: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        painter.paint_polygon(&ellipse, Color::rgba(255, 0, 0, 92));

        // Adjacent triangles
        painter.paint_polygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            Color::rgba(187, 255, 255, 255),
        );
        painter.paint_polygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], Color::RED);
        painter.paint_polygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            Color::rgba(187, 255, 255, 255),
        );
        painter.paint_polygon(&[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)], Color::RED);

        // Thin triangles/quads
        painter.paint_polygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            Color::rgba(187, 136, 255, 192),
        );
        painter.paint_polygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            Color::rgba(136, 187, 255, 192),
        );
        painter.paint_polygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            Color::rgba(136, 187, 255, 192),
        );

        // Ellipses (cx, cy, rx, ry)
        painter.paint_ellipse(0.055, 0.805, 0.005, 0.005, Color::WHITE);
        painter.paint_ellipse(0.07, 0.805, 0.01, 0.005, Color::WHITE);
        painter.paint_ellipse(0.0925, 0.805, 0.0025, 0.005, Color::WHITE);

        // Ellipse sectors
        let deg = PI / 180.0;
        painter.paint_ellipse_sector(
            0.105,
            0.805,
            0.005,
            0.005,
            45.0 * deg,
            350.0 * deg,
            Color::WHITE,
        );
        painter.paint_ellipse_sector(
            0.12,
            0.805,
            0.01,
            0.005,
            -350.0 * deg,
            45.0 * deg,
            Color::WHITE,
        );
        painter.paint_ellipse_sector(
            0.1325,
            0.805,
            0.0025,
            0.005,
            245.0 * deg,
            295.0 * deg,
            Color::WHITE,
        );
        painter.paint_ellipse_sector(
            0.145,
            0.805,
            0.005,
            0.005,
            195.0 * deg,
            245.0 * deg,
            Color::WHITE,
        );

        // Rect outlines
        painter.paint_rect_outlined(0.05, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.001));
        let mut sd = Stroke::new(Color::WHITE, 0.001);
        sd.dash_pattern = vec![0.002, 0.001];
        painter.paint_rect_outlined(0.07, 0.82, 0.02, 0.01, &sd);
        painter.paint_rect_outlined(0.10, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.008));
        painter.paint_rect_outlined(0.13, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.011));

        // Round rects
        painter.paint_round_rect(0.05, 0.84, 0.01, 0.01, 0.001, Color::WHITE);
        painter.paint_round_rect(0.07, 0.84, 0.02, 0.01, 0.002, Color::WHITE);
        painter.paint_round_rect(0.10, 0.84, 0.01, 0.01, 0.003, Color::WHITE);
        painter.paint_round_rect(0.13, 0.84, 0.01, 0.01, 0.006, Color::WHITE);
        painter.paint_round_rect(0.15, 0.84, 0.01, 0.01, 0.0, Color::WHITE);

        // Ellipse outlines
        painter.paint_ellipse_outlined(
            0.055,
            0.865,
            0.005,
            0.005,
            &Stroke::new(Color::WHITE, 0.003),
        );
        painter.paint_ellipse_outlined(
            0.075,
            0.865,
            0.01,
            0.005,
            &Stroke::new(Color::WHITE, 0.001),
        );
        let mut dot_s = Stroke::new(Color::WHITE, 0.00025);
        dot_s.join = LineJoin::Round;
        dot_s.cap = LineCap::Round;
        dot_s.dash_pattern = vec![0.0001, 0.0005];
        painter.paint_ellipse_outlined(0.0925, 0.865, 0.0025, 0.005, &dot_s);

        // Ellipse arcs
        painter.paint_ellipse_arc(
            0.105,
            0.865,
            0.005,
            0.005,
            90.0 * deg,
            225.0 * deg,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_ellipse_sector_outlined(
            0.12,
            0.865,
            0.01,
            0.005,
            45.0 * deg,
            -320.0 * deg,
            &Stroke::new(Color::WHITE, 0.0001),
        );
        painter.paint_ellipse_arc(
            0.1325,
            0.865,
            0.0025,
            0.005,
            245.0 * deg,
            295.0 * deg,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_ellipse_arc(
            0.145,
            0.865,
            0.005,
            0.005,
            195.0 * deg,
            245.0 * deg,
            &Stroke::new(Color::WHITE, 0.001),
        );
        let mut rs = Stroke::new(Color::WHITE, 0.0001);
        rs.join = LineJoin::Round;
        rs.cap = LineCap::Round;
        rs.start_end = StrokeEnd::new(StrokeEndType::Cap);
        rs.finish_end = StrokeEnd::new(StrokeEndType::LineArrow);
        painter.paint_ellipse_arc(0.155, 0.865, 0.005, 0.005, 0.0, -145.0 * deg, &rs);

        // Round rect outlines
        painter.paint_round_rect_outlined(
            0.05,
            0.88,
            0.01,
            0.01,
            0.001,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_round_rect_outlined(
            0.07,
            0.88,
            0.02,
            0.01,
            0.002,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_round_rect_outlined(
            0.10,
            0.88,
            0.01,
            0.01,
            0.003,
            &Stroke::new(Color::WHITE, 0.003),
        );
        painter.paint_round_rect_outlined(
            0.12,
            0.88,
            0.01,
            0.01,
            0.006,
            &Stroke::new(Color::WHITE, 0.0001),
        );
        let mut dds = Stroke::new(Color::WHITE, 0.00002);
        dds.dash_pattern = vec![0.0001, 0.00005, 0.00003, 0.00005];
        painter.paint_round_rect_outlined(0.135, 0.88, 0.01, 0.01, 0.001, &dds);
        painter.paint_round_rect_outlined(
            0.15,
            0.88,
            0.01,
            0.01,
            0.0,
            &Stroke::new(Color::WHITE, 0.001),
        );

        // Bezier curves
        painter.paint_bezier(&[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)], Color::WHITE);
        painter.paint_bezier(
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
        let mut rd = Stroke::new(Color::WHITE, 0.0002);
        rd.join = LineJoin::Round;
        rd.cap = LineCap::Round;
        rd.dash_pattern = vec![0.001, 0.0005];
        painter.paint_bezier_outline(
            &[
                (0.085, 0.91),
                (0.07, 0.902),
                (0.078, 0.89),
                (0.085, 0.900),
                (0.092, 0.89),
                (0.10, 0.902),
            ],
            &rd,
        );
        let mut bls = Stroke::new(Color::WHITE, 0.0002);
        bls.join = LineJoin::Round;
        bls.cap = LineCap::Round;
        bls.dash_pattern = vec![0.001, 0.0005];
        bls.start_end = StrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(Color::RED);
        bls.finish_end = StrokeEnd::new(StrokeEndType::Arrow);
        painter.paint_bezier_line(
            &[(0.105, 0.91), (0.09, 0.902), (0.098, 0.89), (0.105, 0.900)],
            &bls,
        );

        // All 17 stroke end types
        let n = 17usize;
        for i in 0..(2 * n) {
            let a = 2.0 * PI * i as f64 / (2 * n) as f64;
            let mut ls = Stroke::new(Color::WHITE, 0.0001);
            if i & 1 != 0 {
                ls.join = LineJoin::Round;
                ls.cap = LineCap::Round;
            }
            ls.start_end = StrokeEnd::new(StrokeEndType::Cap);
            ls.finish_end = StrokeEnd::new(stroke_end_from_index(i / 2))
                .with_inner_color(Color::rgba(0xFF, 0xFF, 0xFF, 0x40));
            painter.paint_line_stroked(
                0.117 + 0.002 * a.cos(),
                0.903 + 0.002 * a.sin(),
                0.117 + 0.0075 * a.cos(),
                0.903 + 0.0075 * a.sin(),
                &ls,
            );
        }

        // Polyline with stroke ends
        let mut ps = Stroke::new(Color::WHITE, 0.0005);
        ps.join = LineJoin::Round;
        ps.cap = LineCap::Round;
        ps.start_end =
            StrokeEnd::new(StrokeEndType::ContourArrow).with_inner_color(Color::TRANSPARENT);
        ps.finish_end = StrokeEnd::new(StrokeEndType::Cap);
        painter.paint_solid_polyline(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &ps,
            false,
        );

        // Polygon outline
        painter.paint_polygon_outlined(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            Color::RED,
            0.0002,
        );

        // Linear-gradient textured star
        let star = |ox: f64| -> Vec<(f64, f64)> {
            vec![
                (ox, 0.905),
                (ox + 0.015, 0.912),
                (ox + 0.030, 0.900),
                (ox + 0.022, 0.915),
                (ox + 0.030, 0.930),
                (ox + 0.020, 0.922),
                (ox + 0.005, 0.935),
                (ox + 0.012, 0.920),
            ]
        };
        painter.paint_polygon_textured(
            &star(0.200),
            &Texture::LinearGradient {
                color_a: Color::rgba(0, 255, 0, 128),
                color_b: Color::rgba(255, 255, 0, 255),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
        );
        painter.paint_polygon_textured(
            &star(0.220),
            &Texture::RadialGradient {
                color_inner: Color::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: Color::rgba(0, 0, 0xFF, 0x60),
                center: (0.235, 0.918),
                radius: 0.04,
            },
        );
        painter.paint_polygon_textured(
            &star(0.240),
            &Texture::Image {
                image: self.test_image.clone(),
                extension: ImageExtension::Clamp,
                quality: ImageQuality::Bilinear,
            },
        );

        // Gradient/image rects
        painter.paint_linear_gradient(
            0.2,
            0.94,
            0.02,
            0.01,
            Color::rgba(0, 0, 0, 128),
            Color::rgba(128, 128, 128, 128),
            true,
        );
        painter.paint_radial_gradient(
            0.225,
            0.945,
            0.004,
            0.005,
            Color::rgba(255, 136, 0, 255),
            Color::rgba(0, 85, 0, 255),
        );

        // Ellipse with radial gradient
        let eg: Vec<_> = (0..64)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / 64.0;
                (0.24 + 0.01 * a.cos(), 0.945 + 0.005 * a.sin())
            })
            .collect();
        painter.paint_polygon_textured(
            &eg,
            &Texture::RadialGradient {
                color_inner: Color::TRANSPARENT,
                color_outer: Color::rgba(0, 204, 136, 255),
                center: (0.24, 0.945),
                radius: 0.01,
            },
        );

        // Image rects
        painter.paint_image_scaled(
            0.26,
            0.94,
            0.02,
            0.01,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Clamp,
        );
        painter.paint_image_scaled(
            0.275,
            0.907,
            0.002,
            0.002,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Repeat,
        );
        painter.paint_image_scaled(
            0.275,
            0.910,
            0.002,
            0.002,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Clamp,
        );
        painter.paint_image_scaled(
            0.275,
            0.913,
            0.002,
            0.002,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Zero,
        );
    }
}

fn stroke_end_from_index(idx: usize) -> StrokeEndType {
    match idx {
        0 => StrokeEndType::Butt,
        1 => StrokeEndType::Cap,
        2 => StrokeEndType::Arrow,
        3 => StrokeEndType::ContourArrow,
        4 => StrokeEndType::LineArrow,
        5 => StrokeEndType::Triangle,
        6 => StrokeEndType::ContourTriangle,
        7 => StrokeEndType::Square,
        8 => StrokeEndType::ContourSquare,
        9 => StrokeEndType::HalfSquare,
        10 => StrokeEndType::Circle,
        11 => StrokeEndType::ContourCircle,
        12 => StrokeEndType::HalfCircle,
        13 => StrokeEndType::Diamond,
        14 => StrokeEndType::ContourDiamond,
        15 => StrokeEndType::HalfDiamond,
        16 => StrokeEndType::Stroke,
        _ => StrokeEndType::Butt,
    }
}

impl PanelBehavior for TestPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        // Scale to C++ normalized panel coordinates (0..1, 0..tallness).
        // The framework passes pixel dimensions; C++ emTestPanel uses
        // GetWidth()=1.0, GetHeight()=tallness for all painting.
        if state.viewed_rect.w < 25.0 {
            return;
        }

        painter.push_state();
        painter.scale(w, w);
        let h = h / w;

        let fg = if state.is_focused() {
            Color::rgba(255, 136, 136, 255)
        } else if state.in_focused_path() {
            Color::rgba(187, 136, 136, 255)
        } else {
            Color::grey(136)
        };

        let bg = self.bg_color();
        painter.paint_rect(0.0, 0.0, 1.0, h, bg);
        painter.paint_rect_outlined(0.01, 0.01, 1.0 - 0.02, h - 0.02, &Stroke::new(fg, 0.02));
        painter.paint_text_boxed(
            0.02,
            0.02,
            0.49,
            0.07,
            "Test Panel",
            0.1,
            fg,
            TextAlignment::Left,
        );

        let mut state_str = "State:".to_string();
        if state.is_focused() {
            state_str += " Focused";
        }
        if state.in_focused_path() {
            state_str += " InFocusedPath";
        }
        if state.window_focused {
            state_str += " ViewFocused";
        }
        painter.paint_text_boxed(
            0.05,
            0.4,
            0.9,
            0.05,
            &state_str,
            0.05,
            fg,
            TextAlignment::Left,
        );

        let log_color = Color::rgba(0x88, 0x88, 0xBB, 0xFF);
        for (i, entry) in self.input_log.iter().enumerate() {
            painter.paint_text(0.05, 0.57 + i as f64 * 0.008, entry, 0.008, log_color);
        }

        self.paint_primitives(painter, fg);
        painter.pop_state();
    }

    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        let log_entry = format!(
            "EVENT: key={:?} chars=\"{}\" repeat={} variant={:?} mouse={:.4},{:.4}",
            event.key, event.chars, event.is_repeat, event.variant, event.mouse_x, event.mouse_y,
        );
        if self.input_log.len() >= MAX_LOG_ENTRIES {
            self.input_log.remove(0);
        }
        self.input_log.push(log_entry);
        false
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}

    fn is_opaque(&self) -> bool {
        self.bg_color().is_opaque()
    }

    fn get_title(&self) -> Option<String> {
        Some("Test Panel".to_string())
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if self.depth >= MAX_DEPTH {
            return;
        }

        let has_children = ctx.child_count() > 0;
        if has_children {
            for &(name, x, y, w, h) in &CHILD_LAYOUT {
                if let Some(id) = ctx.find_child_by_name(name) {
                    ctx.layout_child(id, x, y, w, h);
                }
            }
        } else {
            let bg_shared = self.bg_color_shared.clone();

            // TkTest group
            let tktest = ctx.create_child_with("tktest", Box::new(TkTestGrpPanel::new()));
            ctx.layout_child(tktest, 0.20, 0.15, 0.30, 0.12);
            ctx.tree
                .set_auto_expansion_threshold(tktest, 900.0, ViewConditionType::Area);

            // 4 recursive TestPanels
            for &(name, x, y, w, h) in &CHILD_LAYOUT[1..5] {
                let child = ctx.create_child_with(
                    name,
                    Box::new(TestPanel::new(self.depth + 1, bg_shared.clone())),
                );
                ctx.layout_child(child, x, y, w, h);
                ctx.tree
                    .set_auto_expansion_threshold(child, 900.0, ViewConditionType::Area);
            }

            // BgColor field
            let look = Look::new();
            let bg_for_cb = bg_shared.clone();
            let mut cf = ColorField::new(look);
            cf.set_color(bg_shared.get());
            cf.on_color = Some(Box::new(move |c| {
                bg_for_cb.set(c);
            }));
            let bgcf = ctx.create_child_with("bgcf", Box::new(ColorFieldPanel { field: cf }));
            ctx.layout_child(bgcf, 0.775, 0.34, 0.10, 0.02);

            // PolyDraw panel
            let polydraw = ctx.create_child_with("polydraw", Box::new(PolyDrawPanel::new()));
            ctx.layout_child(polydraw, 0.05, 0.92, 0.08, 0.04);
        }
    }

    fn create_control_panel(&mut self, parent_ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let look = Look::new();
        let caption = format!(
            "This is just a test\n\nPanel Identity: {}\nBgColor: {}",
            name,
            self.bg_color()
        );
        let label = Label::new(&caption, look);
        let id = parent_ctx.create_child_with(name, Box::new(LabelPanel { label }));
        Some(id)
    }
}

// ---------------------------------------------------------------------------
// Widget wrapper panels
// ---------------------------------------------------------------------------

struct ButtonPanel {
    button: Button,
}

impl PanelBehavior for ButtonPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.button.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.button.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct CheckButtonPanel {
    cb: CheckButton,
}

impl PanelBehavior for CheckButtonPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.cb.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.cb.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct CheckBoxPanel {
    cb: CheckBox,
}

impl PanelBehavior for CheckBoxPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.cb.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.cb.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct RadioButtonPanel {
    rb: RadioButton,
}

impl PanelBehavior for RadioButtonPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.rb.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.rb.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct RadioBoxPanel {
    rb: RadioBox,
}

impl PanelBehavior for RadioBoxPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.rb.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.rb.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct TextFieldPanel {
    tf: TextField,
}

impl PanelBehavior for TextFieldPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.tf.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.tf.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ScalarFieldPanel {
    sf: ScalarField,
}

impl PanelBehavior for ScalarFieldPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.sf.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.sf.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ColorFieldPanel {
    field: ColorField,
}

impl PanelBehavior for ColorFieldPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.field.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.field.input(event)
    }
    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.field.layout_children(ctx, rect.w, rect.h);
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ListBoxPanel {
    lb: ListBox,
}

impl PanelBehavior for ListBoxPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.lb.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.lb.input(event)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct SplitterPanel {
    splitter: Splitter,
}

impl PanelBehavior for SplitterPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.splitter.paint(painter, w, h);
    }
    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        self.splitter.input(event)
    }
    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.splitter.layout_children(ctx, rect.w, rect.h);
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct LabelPanel {
    label: Label,
}

impl PanelBehavior for LabelPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.label.paint(painter, w, h);
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// TkTestGrp — toolkit test group (Phase 5)
// ---------------------------------------------------------------------------

struct TkTestGrpPanel;

impl TkTestGrpPanel {
    fn new() -> Self {
        Self
    }
}

impl PanelBehavior for TkTestGrpPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        painter.push_state();
        painter.scale(w, w);
        let h = h / w;
        painter.paint_rect(0.0, 0.0, 1.0, h, Color::rgba(0x20, 0x30, 0x40, 0xFF));
        painter.paint_text_boxed(
            0.01,
            0.01,
            1.0 - 0.02,
            0.03,
            "Toolkit Test",
            0.03,
            Color::WHITE,
            TextAlignment::Left,
        );
        painter.pop_state();
    }

    fn is_opaque(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if ctx.child_count() > 0 {
            // Re-layout: splitter handles children
            if let Some(sp) = ctx.find_child_by_name("sp") {
                let rect = ctx.layout_rect();
                ctx.layout_child(sp, 0.0, 0.04, rect.w, rect.h - 0.04);
            }
            return;
        }

        let look = Look::new();
        let rect = ctx.layout_rect();

        // Create splitter hierarchy: sp -> (sp1, sp2), each -> (tktest_a, tktest_b)
        let sp = ctx.create_child_with(
            "sp",
            Box::new(SplitterPanel {
                splitter: Splitter::new(Orientation::Horizontal, look.clone()),
            }),
        );
        ctx.layout_child(sp, 0.0, 0.04, rect.w, rect.h - 0.04);

        // Left TkTest
        let t1 = ctx.create_child_with("t1", Box::new(TkTestPanel::new(look.clone(), false)));
        // Right TkTest (disabled style)
        let t2 = ctx.create_child_with("t2", Box::new(TkTestPanel::new(look, true)));

        // Layout the splitter children manually
        let half_w = rect.w / 2.0;
        let h = rect.h - 0.04;
        ctx.layout_child(t1, 0.0, 0.04, half_w, h);
        ctx.layout_child(t2, half_w, 0.04, half_w, h);
    }
}

// ---------------------------------------------------------------------------
// TkTestPanel — creates all widget types (Phase 5)
// ---------------------------------------------------------------------------

struct TkTestPanel {
    look: Rc<Look>,
    disabled: bool,
}

impl TkTestPanel {
    fn new(look: Rc<Look>, disabled: bool) -> Self {
        Self { look, disabled }
    }
}

impl PanelBehavior for TkTestPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        painter.push_state();
        painter.scale(w, w);
        let h = h / w;
        let bg = if self.disabled {
            Color::rgba(0x30, 0x30, 0x30, 0xFF)
        } else {
            Color::rgba(0x18, 0x28, 0x38, 0xFF)
        };
        painter.paint_rect(0.0, 0.0, 1.0, h, bg);

        if self.disabled {
            painter.paint_text_boxed(
                0.0,
                0.0,
                1.0,
                h,
                "Disabled",
                0.02,
                Color::grey(100),
                TextAlignment::Center,
            );
        }
        painter.pop_state();
    }

    fn is_opaque(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if ctx.child_count() > 0 || self.disabled {
            // Re-layout existing children
            let children = ctx.children();
            let n = children.len();
            if n > 0 {
                let rect = ctx.layout_rect();
                let cols = 3;
                let cw = rect.w / cols as f64;
                let rows = n.div_ceil(cols);
                let ch = rect.h / rows as f64;
                for (i, &child) in children.iter().enumerate() {
                    let col = i % cols;
                    let row = i / cols;
                    ctx.layout_child(child, col as f64 * cw, row as f64 * ch, cw, ch);
                }
            }
            return;
        }

        let look = self.look.clone();

        // --- Buttons ---
        let b1 = ctx.create_child_with(
            "b1",
            Box::new(ButtonPanel {
                button: Button::new("Button", look.clone()),
            }),
        );
        let b2 = ctx.create_child_with(
            "b2",
            Box::new(ButtonPanel {
                button: Button::new("Long Desc", look.clone()),
            }),
        );

        // --- Check buttons & boxes ---
        let cb1 = ctx.create_child_with(
            "cb1",
            Box::new(CheckButtonPanel {
                cb: CheckButton::new("Check Button", look.clone()),
            }),
        );
        let cb2 = ctx.create_child_with(
            "cb2",
            Box::new(CheckBoxPanel {
                cb: CheckBox::new("Check Box", look.clone()),
            }),
        );

        // --- Radio buttons & boxes ---
        let rg = RadioGroup::new();
        let rb1 = ctx.create_child_with(
            "rb1",
            Box::new(RadioButtonPanel {
                rb: RadioButton::new("Radio A", look.clone(), rg.clone(), 0),
            }),
        );
        let rb2 = ctx.create_child_with(
            "rb2",
            Box::new(RadioButtonPanel {
                rb: RadioButton::new("Radio B", look.clone(), rg.clone(), 1),
            }),
        );
        let rg2 = RadioGroup::new();
        let rx1 = ctx.create_child_with(
            "rx1",
            Box::new(RadioBoxPanel {
                rb: RadioBox::new("Radio Box A", look.clone(), rg2.clone(), 0),
            }),
        );
        let rx2 = ctx.create_child_with(
            "rx2",
            Box::new(RadioBoxPanel {
                rb: RadioBox::new("Radio Box B", look.clone(), rg2.clone(), 1),
            }),
        );

        // --- Text fields ---
        let mut tf_ro = TextField::new(look.clone());
        tf_ro.set_text("Read-Only");
        let tf1 = ctx.create_child_with("tf1", Box::new(TextFieldPanel { tf: tf_ro }));

        let mut tf_ed = TextField::new(look.clone());
        tf_ed.set_text("Editable");
        tf_ed.set_editable(true);
        let tf2 = ctx.create_child_with("tf2", Box::new(TextFieldPanel { tf: tf_ed }));

        let mut tf_pw = TextField::new(look.clone());
        tf_pw.set_text("Password");
        tf_pw.set_editable(true);
        tf_pw.set_password_mode(true);
        let tf3 = ctx.create_child_with("tf3", Box::new(TextFieldPanel { tf: tf_pw }));

        let mut tf_ml = TextField::new(look.clone());
        tf_ml.set_text("first line\nsecond line\n...");
        tf_ml.set_editable(true);
        tf_ml.set_multi_line(true);
        let tf4 = ctx.create_child_with("tf4", Box::new(TextFieldPanel { tf: tf_ml }));

        // --- Scalar fields ---
        let sf1 = ctx.create_child_with(
            "sf1",
            Box::new(ScalarFieldPanel {
                sf: ScalarField::new(0.0, 100.0, look.clone()),
            }),
        );
        let mut sf_ed = ScalarField::new(-1000.0, 1000.0, look.clone());
        sf_ed.set_value(0.0);
        let sf2 = ctx.create_child_with("sf2", Box::new(ScalarFieldPanel { sf: sf_ed }));

        // --- Color fields ---
        let mut cf_ro = ColorField::new(look.clone());
        cf_ro.set_color(Color::rgba(0xBB, 0x22, 0x22, 0xFF));
        let cf1 = ctx.create_child_with("cf1", Box::new(ColorFieldPanel { field: cf_ro }));

        let mut cf_ed = ColorField::new(look.clone());
        cf_ed.set_color(Color::rgba(0x22, 0xBB, 0x22, 0xFF));
        let cf2 = ctx.create_child_with("cf2", Box::new(ColorFieldPanel { field: cf_ed }));

        // --- List boxes ---
        let items: Vec<String> = (1..=7).map(|i| format!("Item {i}")).collect();

        let mut lb1 = ListBox::new(look.clone());
        lb1.set_items(items.clone());
        let l1 = ctx.create_child_with("l1", Box::new(ListBoxPanel { lb: lb1 }));

        let mut lb2 = ListBox::new(look.clone());
        lb2.set_items(items.clone());
        lb2.set_selection_mode(SelectionMode::Multi);
        let l2 = ctx.create_child_with("l2", Box::new(ListBoxPanel { lb: lb2 }));

        let mut lb3 = ListBox::new(look.clone());
        lb3.set_items(items);
        lb3.set_selection_mode(SelectionMode::None);
        let l3 = ctx.create_child_with("l3", Box::new(ListBoxPanel { lb: lb3 }));

        // Grid layout all children
        let children = vec![
            b1, b2, cb1, cb2, rb1, rb2, rx1, rx2, tf1, tf2, tf3, tf4, sf1, sf2, cf1, cf2, l1, l2,
            l3,
        ];
        let rect = ctx.layout_rect();
        let cols = 3;
        let rows = children.len().div_ceil(cols);
        let cw = rect.w / cols as f64;
        let ch = rect.h / rows as f64;
        for (i, child) in children.into_iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            ctx.layout_child(child, col as f64 * cw, row as f64 * ch, cw, ch);
        }
    }
}

// ---------------------------------------------------------------------------
// PolyDrawPanel — interactive polygon/stroke drawing (Phase 6)
// ---------------------------------------------------------------------------

struct PolyDrawPanel {
    paint_type: usize,
    vertices: Vec<(f64, f64)>,
    fill_color: Color,
    stroke_color: Color,
    stroke_width: f64,
    stroke_rounded: bool,
    start_type: usize,
    end_type: usize,
    drag_idx: Option<usize>,
    drag_dx: f64,
    drag_dy: f64,
    show_handles: bool,
}

impl PolyDrawPanel {
    fn new() -> Self {
        let vertex_count = 9;
        let vertices: Vec<_> = (0..vertex_count)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / vertex_count as f64;
                (a.cos() * 0.4 + 0.5, a.sin() * 0.4 + 0.5)
            })
            .collect();
        Self {
            paint_type: 0,
            vertices,
            fill_color: Color::WHITE,
            stroke_color: Color::BLACK,
            stroke_width: 0.01,
            stroke_rounded: false,
            start_type: 0,
            end_type: 0,
            drag_idx: None,
            drag_dx: 0.0,
            drag_dy: 0.0,
            show_handles: false,
        }
    }

    fn build_stroke(&self) -> Stroke {
        let mut s = Stroke::new(self.stroke_color, self.stroke_width);
        if self.stroke_rounded {
            s.join = LineJoin::Round;
            s.cap = LineCap::Round;
        }
        s.start_end = StrokeEnd::new(stroke_end_from_index(self.start_type));
        s.finish_end = StrokeEnd::new(stroke_end_from_index(self.end_type));
        s
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        painter.push_state();
        painter.scale(w, w);
        let h = h / w;
        // Background gradient
        painter.paint_linear_gradient(
            0.0,
            0.0,
            1.0,
            h,
            Color::rgba(80, 80, 160, 255),
            Color::rgba(160, 160, 80, 255),
            false,
        );

        let verts = &self.vertices;
        let stroke = self.build_stroke();

        match self.paint_type {
            0 => painter.paint_polygon(verts, self.fill_color),
            1 => painter.paint_polygon_outlined(verts, stroke.color, stroke.width),
            2 => painter.paint_solid_polyline(verts, &stroke, false),
            3 => painter.paint_bezier(verts, self.fill_color),
            4 => painter.paint_bezier_outline(verts, &stroke),
            5 => painter.paint_bezier_line(verts, &stroke),
            6 if verts.len() >= 2 => {
                painter.paint_line_stroked(verts[0].0, verts[0].1, verts[1].0, verts[1].1, &stroke);
            }
            7 if verts.len() >= 2 => {
                let x = verts[0].0.min(verts[1].0);
                let y = verts[0].1.min(verts[1].1);
                let rw = (verts[1].0 - verts[0].0).abs();
                let rh = (verts[1].1 - verts[0].1).abs();
                painter.paint_rect(x, y, rw, rh, self.fill_color);
            }
            8 if verts.len() >= 2 => {
                let x = verts[0].0.min(verts[1].0);
                let y = verts[0].1.min(verts[1].1);
                let rw = (verts[1].0 - verts[0].0).abs();
                let rh = (verts[1].1 - verts[0].1).abs();
                painter.paint_rect_outlined(x, y, rw, rh, &stroke);
            }
            9 if verts.len() >= 2 => {
                let cx = (verts[0].0 + verts[1].0) / 2.0;
                let cy = (verts[0].1 + verts[1].1) / 2.0;
                let rx = (verts[1].0 - verts[0].0).abs() / 2.0;
                let ry = (verts[1].1 - verts[0].1).abs() / 2.0;
                painter.paint_ellipse(cx, cy, rx, ry, self.fill_color);
            }
            10 if verts.len() >= 2 => {
                let cx = (verts[0].0 + verts[1].0) / 2.0;
                let cy = (verts[0].1 + verts[1].1) / 2.0;
                let rx = (verts[1].0 - verts[0].0).abs() / 2.0;
                let ry = (verts[1].1 - verts[0].1).abs() / 2.0;
                painter.paint_ellipse_outlined(cx, cy, rx, ry, &stroke);
            }
            14 if verts.len() >= 2 => {
                let x = verts[0].0.min(verts[1].0);
                let y = verts[0].1.min(verts[1].1);
                let rw = (verts[1].0 - verts[0].0).abs();
                let rh = (verts[1].1 - verts[0].1).abs();
                painter.paint_round_rect(x, y, rw, rh, rw * 0.2, self.fill_color);
            }
            15 if verts.len() >= 2 => {
                let x = verts[0].0.min(verts[1].0);
                let y = verts[0].1.min(verts[1].1);
                let rw = (verts[1].0 - verts[0].0).abs();
                let rh = (verts[1].1 - verts[0].1).abs();
                painter.paint_round_rect_outlined(x, y, rw, rh, rw * 0.2, &stroke);
            }
            _ => {}
        }

        // Vertex handles
        if self.show_handles {
            let r = 0.02f64.min(0.05);
            for (i, &(x, y)) in verts.iter().enumerate() {
                let mut c = Color::rgba(0, 255, 0, 128);
                if self.drag_idx == Some(i) {
                    c = c.lerp(Color::WHITE, 0.75);
                }
                painter.paint_ellipse(x, y, r, r, c);
                painter.paint_ellipse_outlined(
                    x,
                    y,
                    r,
                    r,
                    &Stroke::new(Color::rgba(0, 0, 0, 128), r * 0.15),
                );
            }
        }

        painter.paint_text_boxed(
            0.0,
            h - 0.06,
            1.0,
            0.06,
            "Drag vertices with left mouse button! (Hold shift for snap grid)",
            0.03,
            Color::WHITE,
            TextAlignment::Center,
        );
        painter.pop_state();
    }

    fn input(&mut self, event: &InputEvent, _state: &PanelState) -> bool {
        let mx = event.mouse_x;
        let my = event.mouse_y;

        match event.variant {
            InputVariant::Press if event.key == InputKey::MouseLeft && self.drag_idx.is_none() => {
                let mut best_i = None;
                let mut best_r = 0.05;
                for (i, &(vx, vy)) in self.vertices.iter().enumerate() {
                    let r = ((vx - mx).powi(2) + (vy - my).powi(2)).sqrt();
                    if r < best_r {
                        best_i = Some(i);
                        best_r = r;
                    }
                }
                if let Some(i) = best_i {
                    self.drag_idx = Some(i);
                    self.drag_dx = self.vertices[i].0 - mx;
                    self.drag_dy = self.vertices[i].1 - my;
                    return true;
                }
            }
            InputVariant::Release if event.key == InputKey::MouseLeft => {
                if self.drag_idx.is_some() {
                    self.drag_idx = None;
                    return true;
                }
            }
            InputVariant::Move if self.drag_idx.is_some() => {
                let i = self.drag_idx.unwrap();
                let mut x = (mx + self.drag_dx).clamp(0.0, 1.0);
                let mut y = (my + self.drag_dy).clamp(0.0, 1.0);
                if event.shift || event.ctrl {
                    let snap = 0.05;
                    x = (x / snap).round() * snap;
                    y = (y / snap).round() * snap;
                }
                self.vertices[i] = (x, y);
                return true;
            }
            _ => {}
        }

        // Show handles when mouse is over panel
        let over = (0.0..=1.0).contains(&mx) && (0.0..=1.0).contains(&my);
        let was = self.show_handles;
        self.show_handles = self.drag_idx.is_some() || over;
        if self.show_handles != was {
            return false; // repaint but don't consume
        }

        false
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let bg_color = Rc::new(Cell::new(Color::rgba(0x00, 0x1C, 0x38, 0xFF)));

        let root = app.tree.create_root("test_panel_root");
        app.tree
            .set_behavior(root, Box::new(TestPanel::new(0, bg_color)));
        app.tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
        app.tree
            .set_auto_expansion_threshold(root, 900.0, ViewConditionType::Area);

        let close_sig = app.scheduler.create_signal();
        let win = ZuiWindow::create(
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
