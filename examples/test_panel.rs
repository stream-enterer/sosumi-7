//! Comprehensive integration test panel derived from C++ `emTestPanel.cpp`.
//!
//! Exercises nearly every subsystem: panel tree, auto-expansion, recursive
//! children, full PaintContent primitives, Input logging, widgets, splitters,
//! color field binding, and interactive polygon drawing.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emImage::emImage;
use eaglemode_rs::emCore::emCursor::emCursor;
use eaglemode_rs::emCore::emInput::{emInputEvent, InputKey, InputVariant};
use eaglemode_rs::emCore::emInputState::emInputState;
use eaglemode_rs::emCore::emRasterGroup::emRasterGroup;
use eaglemode_rs::emCore::emPanel::{NoticeFlags, PanelBehavior, PanelState};

use eaglemode_rs::emCore::emPanelCtx::PanelCtx;

use eaglemode_rs::emCore::emPanelTree::{PanelId, ViewConditionType};

use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::{emPainter, TextAlignment, VAlign};

use eaglemode_rs::emCore::emStroke::{LineCap, LineJoin, emStroke};

use eaglemode_rs::emCore::emStrokeEnd::{emStrokeEnd, StrokeEndType};

use eaglemode_rs::emCore::emTexture::{ImageExtension, ImageQuality, emTexture};
use eaglemode_rs::emCore::emButton::emButton;

use eaglemode_rs::emCore::emCheckBox::emCheckBox;

use eaglemode_rs::emCore::emCheckButton::emCheckButton;

use eaglemode_rs::emCore::emColorField::emColorField;

use eaglemode_rs::emCore::emLabel::emLabel;

use eaglemode_rs::emCore::emListBox::{emListBox, SelectionMode};

use eaglemode_rs::emCore::emLook::emLook;

use eaglemode_rs::emCore::emRadioBox::emRadioBox;

use eaglemode_rs::emCore::emRadioButton::{emRadioButton, RadioGroup};

use eaglemode_rs::emCore::emScalarField::emScalarField;

use eaglemode_rs::emCore::emTextField::emTextField;
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

// ── Constants ──

const MAX_DEPTH: u32 = 10;
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
    widget: emButton,
}
impl PanelBehavior for ButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct CheckButtonPanel {
    widget: emCheckButton,
}
impl PanelBehavior for CheckButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct CheckBoxPanel {
    widget: emCheckBox,
}
impl PanelBehavior for CheckBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct RadioButtonPanel {
    widget: emRadioButton,
}
impl PanelBehavior for RadioButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct RadioBoxPanel {
    widget: emRadioBox,
}
impl PanelBehavior for RadioBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct TextFieldPanel {
    widget: emTextField,
}
impl PanelBehavior for TextFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.cycle_blink(s.in_focused_path());
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.widget.on_focus_changed(state.in_focused_path());
        }
    }
}

struct ScalarFieldPanel {
    widget: emScalarField,
}
impl PanelBehavior for ScalarFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct ColorFieldPanel {
    widget: emColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct ListBoxPanel {
    widget: emListBox,
}
impl PanelBehavior for ListBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct LabelPanel {
    widget: emLabel,
}
impl PanelBehavior for LabelPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.PaintContent(p, w, h, s.enabled, pixel_scale);
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TestPanel — root panel, recursive
// ═══════════════════════════════════════════════════════════════════════

struct TestPanel {
    bg_color_shared: Rc<Cell<emColor>>,
    input_log: Vec<String>,
    test_image: emImage,
    depth: u32,
}

impl TestPanel {
    fn new(depth: u32, bg_color_shared: Rc<Cell<emColor>>) -> Self {
        let mut img = emImage::new(64, 64, 4);
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

    fn bg_color(&self) -> emColor {
        self.bg_color_shared.get()
    }

    /// Paint all the primitive-drawing tests (Match C++ Paint() body).
    fn paint_primitives(&self, p: &mut emPainter, fg: emColor, bg: emColor) {
        // Text test with tabs
        p.PaintTextBoxed(
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
        p.PaintRect(
            0.25,
            0.80,
            0.05,
            0.05,
            emColor::rgba(255, 0, 0, 32),
            emColor::TRANSPARENT,
        );

        // Triangle
        p.PaintPolygon(
            &[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)],
            fg,
            emColor::TRANSPARENT,
        );

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
            emColor::rgba(255, 255, 255, 128),
            emColor::TRANSPARENT,
        );

        // Holed polygon (non-zero winding, reversed inner)
        p.PaintPolygon(
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
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        // Circle (polygon approximation)
        let circle: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle, emColor::YELLOW, emColor::TRANSPARENT);

        // Clipped circle
        p.push_state();
        p.SetClipping(0.51, 0.81, 0.08, 0.08);
        let circle2: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle2, emColor::GREEN, emColor::TRANSPARENT);
        p.pop_state();

        // Ellipse (polygon)
        let ellipse: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        p.PaintPolygon(&ellipse, emColor::rgba(255, 0, 0, 92), emColor::TRANSPARENT);

        // More triangles
        p.PaintPolygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            emColor::TRANSPARENT,
        );
        p.PaintPolygon(
            &[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)],
            emColor::RED,
            emColor::TRANSPARENT,
        );
        p.PaintPolygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            emColor::TRANSPARENT,
        );
        p.PaintPolygon(
            &[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)],
            emColor::RED,
            emColor::TRANSPARENT,
        );

        // Thin triangles
        p.PaintPolygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            emColor::rgba(187, 136, 255, 192),
            emColor::TRANSPARENT,
        );
        p.PaintPolygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            emColor::rgba(136, 187, 255, 192),
            emColor::TRANSPARENT,
        );

        // Bowtie
        p.PaintPolygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            emColor::rgba(136, 187, 255, 192),
            emColor::TRANSPARENT,
        );

        // Ellipses (center + radius)
        p.PaintEllipse(0.055, 0.805, 0.005, 0.005, emColor::WHITE, emColor::TRANSPARENT);
        p.PaintEllipse(0.07, 0.805, 0.01, 0.005, emColor::WHITE, emColor::TRANSPARENT);
        p.PaintEllipse(
            0.0925,
            0.805,
            0.0025,
            0.005,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        // Ellipse sectors (degrees: start_angle, sweep_angle)
        p.PaintEllipseSector(
            0.105,
            0.805,
            0.005,
            0.005,
            45.0,
            305.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        p.PaintEllipseSector(
            0.12,
            0.805,
            0.01,
            0.005,
            45.0,
            -395.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        // Rect outlines
        let thin_stroke = emStroke::new(emColor::WHITE, 0.001);
        p.PaintRectOutline(0.05, 0.82, 0.01, 0.01, &thin_stroke, emColor::TRANSPARENT);
        let thick_stroke = emStroke::new(emColor::WHITE, 0.008);
        p.PaintRectOutline(0.10, 0.82, 0.01, 0.01, &thick_stroke, emColor::TRANSPARENT);

        // Round rects
        p.PaintRoundRect(0.05, 0.84, 0.01, 0.01, 0.001, 0.001, emColor::WHITE);
        p.PaintRoundRect(0.07, 0.84, 0.02, 0.01, 0.002, 0.002, emColor::WHITE);
        p.PaintRoundRect(0.10, 0.84, 0.01, 0.01, 0.003, 0.003, emColor::WHITE);

        // Ellipse outlines
        let outline_stroke = emStroke::new(emColor::WHITE, 0.003);
        p.PaintEllipseOutline(
            0.055,
            0.865,
            0.005,
            0.005,
            &outline_stroke,
            emColor::TRANSPARENT,
        );
        let thin_outline = emStroke::new(emColor::WHITE, 0.001);
        p.PaintEllipseOutline(0.075, 0.865, 0.01, 0.005, &thin_outline, emColor::TRANSPARENT);

        // Round rect outlines
        let rr_stroke = emStroke::new(emColor::WHITE, 0.001);
        p.PaintRoundRectOutline(0.05, 0.88, 0.01, 0.01, 0.001, 0.001, &rr_stroke);
        p.PaintRoundRectOutline(0.07, 0.88, 0.02, 0.01, 0.002, 0.002, &rr_stroke);

        // Bezier curves
        p.PaintBezier(
            &[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)],
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        p.PaintBezier(
            &[
                (0.065, 0.91),
                (0.05, 0.902),
                (0.058, 0.89),
                (0.065, 0.900),
                (0.072, 0.89),
                (0.08, 0.902),
            ],
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        let bezier_stroke = emStroke::new(emColor::WHITE, 0.0002);
        p.PaintBezierOutline(
            &[
                (0.085, 0.91),
                (0.07, 0.902),
                (0.078, 0.89),
                (0.085, 0.900),
                (0.092, 0.89),
                (0.10, 0.902),
            ],
            &bezier_stroke,
            emColor::TRANSPARENT,
        );

        let mut arrow_s = emStroke::new(emColor::WHITE, 0.0002);
        arrow_s.cap = LineCap::Round;
        arrow_s.join = LineJoin::Round;
        arrow_s.start_end =
            emStrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(emColor::RED);
        arrow_s.finish_end = emStrokeEnd::new(StrokeEndType::Arrow);
        p.PaintBezierLine(
            &[(0.105, 0.91), (0.09, 0.902), (0.098, 0.89), (0.105, 0.900)],
            &arrow_s,
            emColor::TRANSPARENT,
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
            StrokeEndType::emStroke,
        ];
        let n = end_types.len();
        for i in 0..(2 * n) {
            let a = 2.0 * PI * i as f64 / (2 * n) as f64;
            let mut s = emStroke::new(emColor::WHITE, 0.0001);
            if i & 1 != 0 {
                s.cap = LineCap::Round;
                s.join = LineJoin::Round;
            }
            s.start_end = emStrokeEnd::new(StrokeEndType::Cap);
            s.finish_end = emStrokeEnd::new(end_types[i / 2])
                .with_inner_color(emColor::rgba(0xFF, 0xFF, 0xFF, 0x40));
            p.paint_line_stroked(
                0.117 + 0.002 * a.cos(),
                0.903 + 0.002 * a.sin(),
                0.117 + 0.0075 * a.cos(),
                0.903 + 0.0075 * a.sin(),
                &s,
                emColor::TRANSPARENT,
            );
        }

        // Polyline with contour arrow
        let mut poly_s = emStroke::new(emColor::WHITE, 0.0005);
        poly_s.cap = LineCap::Round;
        poly_s.join = LineJoin::Round;
        poly_s.start_end = emStrokeEnd::new(StrokeEndType::ContourArrow);
        poly_s.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
        p.PaintPolylineWithArrows(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &poly_s,
            false,
            emColor::TRANSPARENT,
            None,
        );

        // Polygon outline
        p.PaintPolygonOutline(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            emColor::RED,
            0.0002,
            emColor::TRANSPARENT,
        );

        // Textured polygons — star shapes
        let star = make_star(0.215, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star,
            &emTexture::LinearGradient {
                color_a: emColor::rgba(0, 0xFF, 0, 0x80),
                color_b: emColor::rgba(0xFF, 0xFF, 0, 0xFF),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
            emColor::TRANSPARENT,
        );

        let star2 = make_star(0.235, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star2,
            &emTexture::RadialGradient {
                color_inner: emColor::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: emColor::rgba(0, 0, 0xFF, 0x60),
                center: (0.21, 0.90),
                radius: 0.05,
            },
            emColor::TRANSPARENT,
        );

        let star3 = make_star(0.255, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star3,
            &emTexture::emImage {
                image: self.test_image.clone(),
                extension: ImageExtension::Repeat,
                quality: ImageQuality::Bilinear,
            },
            emColor::TRANSPARENT,
        );

        // Gradient rects
        p.paint_linear_gradient(
            0.2,
            0.94,
            0.02,
            0.01,
            emColor::rgba(0, 0, 0, 0x80),
            emColor::rgba(0x80, 0x80, 0x80, 0x80),
            true,
            emColor::TRANSPARENT,
        );
        p.paint_radial_gradient(
            0.225,
            0.946,
            0.004,
            0.008,
            emColor::rgba(0xFF, 0x88, 0, 0xFF),
            emColor::rgba(0, 0x55, 0, 0xFF),
            emColor::TRANSPARENT,
        );
        p.PaintEllipse(
            0.24,
            0.945,
            0.01,
            0.005,
            emColor::rgba(0, 0xCC, 0x88, 0xFF),
            emColor::TRANSPARENT,
        );

        // emImage scaled
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
    fn IsOpaque(&self) -> bool {
        self.bg_color().IsOpaque()
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn get_title(&self) -> Option<String> {
        Some("Test Panel".into())
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let bg = self.bg_color();
        let fg = if state.is_focused() {
            emColor::rgba(255, 136, 136, 255)
        } else if state.in_focused_path() {
            emColor::rgba(187, 136, 136, 255)
        } else {
            emColor::rgba(136, 136, 136, 255)
        };

        // Use push/scale/pop so coordinates are in [0,1] x [0,h/w]
        painter.push_state();
        painter.scale(w, w);

        let panel_h = h / w;
        painter.PaintRect(0.0, 0.0, 1.0, panel_h, bg, emColor::TRANSPARENT);
        painter.PaintRectOutline(
            0.01,
            0.01,
            0.98,
            panel_h - 0.02,
            &emStroke::new(fg, 0.02),
            emColor::TRANSPARENT,
        );

        // Title
        painter.PaintTextBoxed(
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
        painter.PaintTextBoxed(
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
        painter.PaintTextBoxed(
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
            painter.PaintText(
                0.05,
                0.57 + i as f64 * 0.008,
                entry,
                0.008,
                1.0,
                emColor::rgba(0x88, 0x88, 0xBB, 0xFF),
                bg,
            );
        }

        // Paint primitives
        self.paint_primitives(painter, fg, bg);

        painter.pop_state();
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
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

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
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

        // Create children — Match C++ AutoExpand()
        let bg_shared = self.bg_color_shared.clone();

        // TkTestGrp
        ctx.create_child_with("tktest", Box::new(TkTestGrpPanel::new()));

        // Recursive test panels (depth + 1)
        if self.depth < MAX_DEPTH {
            for i in 1..=4 {
                let child_bg = Rc::new(Cell::new(emColor::rgba(0x00, 0x1C, 0x38, 0xFF)));
                let tp_id = ctx.create_child_with(
                    &format!("tp{i}"),
                    Box::new(TestPanel::new(self.depth + 1, child_bg)),
                );
                // C++: every emTestPanel constructor calls SetAutoExpansionThreshold(900.0)
                ctx.tree
                    .SetAutoExpansionThreshold(tp_id, 900.0, ViewConditionType::Area);
            }
        }

        // Background color field — linked to bg_color_shared
        let bg_for_cf = bg_shared.clone();
        let mut cf = emColorField::new(emLook::new());
        cf.SetCaption("Background Color");
        cf.SetEditable(true);
        cf.SetAlphaEnabled(true);
        cf.SetColor(bg_shared.get());
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

    fn CreateControlPanel(&mut self, ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let identity = ctx.tree.GetIdentity(ctx.id);
        let bg = self.bg_color();
        let text = format!(
            "This is just a test\n\nPanel Identity: {identity}\nBgColor: 0x{:08X}",
            bg.GetPacked()
        );
        let label = emLabel::new(&text, emLook::new());
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
    fn IsOpaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _s: &PanelState) {
        p.PaintRect(
            0.0,
            0.0,
            w,
            h,
            emColor::rgba(0x20, 0x30, 0x40, 0xFF),
            emColor::TRANSPARENT,
        );
        p.PaintTextBoxed(
            0.0,
            0.0,
            w,
            h * 0.05,
            "Toolkit Test",
            h * 0.04,
            emColor::WHITE,
            emColor::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let rect = ctx.layout_rect();
        let h = rect.h / rect.w;
        let body_y = 0.05 * h;
        let body_h = 0.95 * h;
        let half_w = 0.5;
        let half_h = body_h * 0.5;

        if !children.is_empty() {
            // Reposition existing children in 2x2 grid
            if let Some(id) = ctx.find_child_by_name("t1a") {
                ctx.layout_child(id, 0.0, body_y, half_w, half_h);
            }
            if let Some(id) = ctx.find_child_by_name("t1b") {
                ctx.layout_child(id, 0.0, body_y + half_h, half_w, half_h);
            }
            if let Some(id) = ctx.find_child_by_name("t2a") {
                ctx.layout_child(id, half_w, body_y, half_w, half_h);
            }
            if let Some(id) = ctx.find_child_by_name("t2b") {
                ctx.layout_child(id, half_w, body_y + half_h, half_w, half_h);
            }
            return;
        }

        let look = emLook::new();

        ctx.create_child_with("t1a", Box::new(TkTestPanel::new(look.clone())));
        ctx.create_child_with("t1b", Box::new(TkTestPanel::new(look.clone())));
        ctx.create_child_with("t2a", Box::new(TkTestPanel::new(look.clone())));

        let t2b_id = ctx.create_child_with("t2b", Box::new(TkTestPanel::new(look.clone())));
        ctx.tree.SetEnableSwitch(t2b_id, false); // disabled per C++ spec

        // Layout all in 2x2 grid
        if let Some(id) = ctx.find_child_by_name("t1a") {
            ctx.layout_child(id, 0.0, body_y, half_w, half_h);
        }
        if let Some(id) = ctx.find_child_by_name("t1b") {
            ctx.layout_child(id, 0.0, body_y + half_h, half_w, half_h);
        }
        if let Some(id) = ctx.find_child_by_name("t2a") {
            ctx.layout_child(id, half_w, body_y, half_w, half_h);
        }
        if let Some(id) = ctx.find_child_by_name("t2b") {
            ctx.layout_child(id, half_w, body_y + half_h, half_w, half_h);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// WidgetGroupPanel — bordered group container for widget categories
// ═══════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
enum WidgetCategory {
    Buttons,
    CheckWidgets,
    RadioWidgets,
    TextFields,
    ScalarFields,
    ColorFields,
    ListBoxes,
}

struct WidgetGroupPanel {
    group: emRasterGroup,
    category: WidgetCategory,
    look: Rc<emLook>,
}

impl WidgetGroupPanel {
    fn new(category: WidgetCategory, caption: &str, look: Rc<emLook>) -> Self {
        let mut group = emRasterGroup::new();
        group.border.caption = caption.to_string();
        group.border.SetBorderScaling(2.5);
        group.look = (*look).clone();
        Self {
            group,
            category,
            look,
        }
    }
}

impl PanelBehavior for WidgetGroupPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.group.Paint(p, w, h, s);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            let look = self.look.clone();
            match self.category {
                WidgetCategory::Buttons => {
                    let bt1 = emButton::new("Button", look.clone());
                    ctx.create_child_with("b1", Box::new(ButtonPanel { widget: bt1 }));
                    let bt2 = emButton::new("Long Desc", look);
                    ctx.create_child_with("b2", Box::new(ButtonPanel { widget: bt2 }));
                }
                WidgetCategory::CheckWidgets => {
                    let cb1 = emCheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c1", Box::new(CheckButtonPanel { widget: cb1 }));
                    let cb2 = emCheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c2", Box::new(CheckButtonPanel { widget: cb2 }));
                    let cbx1 = emCheckBox::new("Check Box", look.clone());
                    ctx.create_child_with("c4", Box::new(CheckBoxPanel { widget: cbx1 }));
                    let cbx2 = emCheckBox::new("Check Box", look);
                    ctx.create_child_with("c5", Box::new(CheckBoxPanel { widget: cbx2 }));
                }
                WidgetCategory::RadioWidgets => {
                    let rg = RadioGroup::new();
                    let rb1 = emRadioButton::new("Radio Button", look.clone(), rg.clone(), 0);
                    ctx.create_child_with("r1", Box::new(RadioButtonPanel { widget: rb1 }));
                    let rb2 = emRadioButton::new("Radio Button", look.clone(), rg.clone(), 1);
                    ctx.create_child_with("r2", Box::new(RadioButtonPanel { widget: rb2 }));
                    let rb3 = emRadioButton::new("Radio Button", look.clone(), rg, 2);
                    ctx.create_child_with("r3", Box::new(RadioButtonPanel { widget: rb3 }));
                    let rg2 = RadioGroup::new();
                    let rbx1 = emRadioBox::new("Radio Box", look.clone(), rg2.clone(), 0);
                    ctx.create_child_with("r4", Box::new(RadioBoxPanel { widget: rbx1 }));
                    let rbx2 = emRadioBox::new("Radio Box", look.clone(), rg2.clone(), 1);
                    ctx.create_child_with("r5", Box::new(RadioBoxPanel { widget: rbx2 }));
                    let rbx3 = emRadioBox::new("Radio Box", look, rg2, 2);
                    ctx.create_child_with("r6", Box::new(RadioBoxPanel { widget: rbx3 }));
                }
                WidgetCategory::TextFields => {
                    let mut tf1 = emTextField::new(look.clone());
                    tf1.SetText("Read-Only");
                    ctx.create_child_with("tf1", Box::new(TextFieldPanel { widget: tf1 }));
                    let mut tf2 = emTextField::new(look.clone());
                    tf2.SetEditable(true);
                    tf2.SetText("Editable");
                    ctx.create_child_with("tf2", Box::new(TextFieldPanel { widget: tf2 }));
                    let mut tf3 = emTextField::new(look.clone());
                    tf3.SetEditable(true);
                    tf3.SetText("Password");
                    tf3.SetPasswordMode(true);
                    ctx.create_child_with("tf3", Box::new(TextFieldPanel { widget: tf3 }));
                    let mut tf4 = emTextField::new(look);
                    tf4.SetEditable(true);
                    tf4.SetMultiLineMode(true);
                    tf4.SetText("first line\nsecond line\n...");
                    ctx.create_child_with("mltf1", Box::new(TextFieldPanel { widget: tf4 }));
                }
                WidgetCategory::ScalarFields => {
                    let sf1 = emScalarField::new(0.0, 100.0, look.clone());
                    ctx.create_child_with("sf1", Box::new(ScalarFieldPanel { widget: sf1 }));
                    let mut sf2 = emScalarField::new(0.0, 100.0, look.clone());
                    sf2.SetEditable(true);
                    ctx.create_child_with("sf2", Box::new(ScalarFieldPanel { widget: sf2 }));
                    let mut sf3 = emScalarField::new(-1000.0, 1000.0, look);
                    sf3.SetEditable(true);
                    sf3.SetScaleMarkIntervals(&[1000, 100, 10, 5, 1]);
                    ctx.create_child_with("sf3", Box::new(ScalarFieldPanel { widget: sf3 }));
                }
                WidgetCategory::ColorFields => {
                    let mut cf1 = emColorField::new(look.clone());
                    cf1.SetColor(emColor::rgba(0xBB, 0x22, 0x22, 0xFF));
                    ctx.create_child_with("cf1", Box::new(ColorFieldPanel { widget: cf1 }));
                    let mut cf2 = emColorField::new(look.clone());
                    cf2.SetEditable(true);
                    cf2.SetColor(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
                    ctx.create_child_with("cf2", Box::new(ColorFieldPanel { widget: cf2 }));
                    let mut cf3 = emColorField::new(look);
                    cf3.SetEditable(true);
                    cf3.SetAlphaEnabled(true);
                    cf3.SetColor(emColor::rgba(0x22, 0x22, 0xBB, 0xFF));
                    ctx.create_child_with("cf3", Box::new(ColorFieldPanel { widget: cf3 }));
                }
                WidgetCategory::ListBoxes => {
                    let lb1 = emListBox::new(look.clone());
                    ctx.create_child_with("l1", Box::new(ListBoxPanel { widget: lb1 }));
                    let mut lb2 = emListBox::new(look.clone());
                    lb2.SetSelectionType(SelectionMode::Single);
                    lb2.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l2", Box::new(ListBoxPanel { widget: lb2 }));
                    let mut lb3 = emListBox::new(look.clone());
                    lb3.SetSelectionType(SelectionMode::ReadOnly);
                    lb3.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l3", Box::new(ListBoxPanel { widget: lb3 }));
                    let mut lb4 = emListBox::new(look.clone());
                    lb4.SetSelectionType(SelectionMode::Multi);
                    lb4.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l4", Box::new(ListBoxPanel { widget: lb4 }));
                    let mut lb5 = emListBox::new(look);
                    lb5.SetSelectionType(SelectionMode::Toggle);
                    lb5.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l5", Box::new(ListBoxPanel { widget: lb5 }));
                }
            }
        }
        self.group.LayoutChildren(ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TkTestPanel — widget showcase (Match C++ TkTest)
// ═══════════════════════════════════════════════════════════════════════

struct TkTestPanel {
    group: emRasterGroup,
    look: Rc<emLook>,
}

impl TkTestPanel {
    fn new(look: Rc<emLook>) -> Self {
        let mut group = emRasterGroup::new();
        group.border.caption = "Toolkit Test".to_string();
        group.border.SetBorderScaling(2.5);
        group.layout.preferred_child_tallness = 0.3;
        group.look = (*look).clone();
        Self { group, look }
    }
}

impl PanelBehavior for TkTestPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.group.Paint(p, w, h, s);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            let look = self.look.clone();

            let groups: &[(&str, &str, WidgetCategory)] = &[
                ("grp_btn", "Buttons", WidgetCategory::Buttons),
                ("grp_chk", "Check Widgets", WidgetCategory::CheckWidgets),
                ("grp_rad", "Radio Widgets", WidgetCategory::RadioWidgets),
                ("grp_txt", "Text Fields", WidgetCategory::TextFields),
                ("grp_scl", "Scalar Fields", WidgetCategory::ScalarFields),
                ("grp_clr", "Color Fields", WidgetCategory::ColorFields),
                ("grp_lst", "List Boxes", WidgetCategory::ListBoxes),
            ];

            for &(name, caption, cat) in groups {
                ctx.create_child_with(
                    name,
                    Box::new(WidgetGroupPanel::new(cat, caption, look.clone())),
                );
            }
        }
        self.group.LayoutChildren(ctx);
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
    fill_color: emColor,
    stroke_width: f64,
    stroke_color: emColor,
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
            fill_color: emColor::WHITE,
            stroke_width: 0.01,
            stroke_color: emColor::BLACK,
        }
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Input(&mut self, event: &emInputEvent, _state: &PanelState, input_state: &emInputState) -> bool {
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

        if self.drag_idx.is_some() && !input_state.Get(InputKey::MouseLeft) {
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

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        // Background gradient
        p.paint_linear_gradient(
            0.0,
            0.0,
            w,
            h,
            emColor::rgba(80, 80, 160, 255),
            emColor::rgba(160, 160, 80, 255),
            false,
            emColor::TRANSPARENT,
        );

        // Scale vertices to pixel space
        let scaled: Vec<(f64, f64)> = self
            .vertices
            .iter()
            .map(|&(vx, vy)| (vx * w, vy * h))
            .collect();

        match self.paint_type {
            0 => p.PaintPolygon(&scaled, self.fill_color, emColor::TRANSPARENT),
            1 => p.PaintPolygonOutline(
                &scaled,
                self.stroke_color,
                self.stroke_width * w,
                emColor::TRANSPARENT,
            ),
            2 => {
                let s = emStroke::new(self.stroke_color, self.stroke_width * w);
                p.PaintPolylineWithoutArrows(&scaled, &s, false, emColor::TRANSPARENT);
            }
            3 => p.PaintBezier(&scaled, self.fill_color, emColor::TRANSPARENT),
            4 => {
                let s = emStroke::new(self.stroke_color, self.stroke_width * w);
                p.PaintBezierOutline(&scaled, &s, emColor::TRANSPARENT);
            }
            5 => {
                let s = emStroke::new(self.stroke_color, self.stroke_width * w);
                p.PaintBezierLine(&scaled, &s, emColor::TRANSPARENT);
            }
            _ => p.PaintPolygon(&scaled, self.fill_color, emColor::TRANSPARENT),
        }

        // Draw handles
        if self.show_handles {
            let r = 0.01 * w;
            for (i, &(vx, vy)) in scaled.iter().enumerate() {
                let c = if Some(i) == self.drag_idx {
                    emColor::rgba(255, 255, 255, 200)
                } else {
                    emColor::rgba(0, 255, 0, 128)
                };
                p.PaintEllipse(vx, vy, r, r, c, emColor::TRANSPARENT);
                let outline = emStroke::new(emColor::rgba(0, 0, 0, 128), r * 0.15);
                p.PaintEllipseOutline(vx, vy, r, r, &outline, emColor::TRANSPARENT);
            }
        }

        // Help text
        p.PaintTextBoxed(
            0.0,
            h - 0.05 * h,
            w,
            0.05 * h,
            "Drag vertices with left mouse button",
            0.03 * h,
            emColor::WHITE,
            emColor::TRANSPARENT,
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
        let bg_color = Rc::new(Cell::new(emColor::rgba(0x00, 0x1C, 0x38, 0xFF)));

        let root = app.tree.create_root("root");
        app.tree
            .set_behavior(root, Box::new(TestPanel::new(0, bg_color)));
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
        app.tree
            .SetAutoExpansionThreshold(root, 900.0, ViewConditionType::Area);

        let close_sig = app.scheduler.borrow_mut().create_signal();
        let flags_sig = app.scheduler.borrow_mut().create_signal();
        let focus_sig = app.scheduler.borrow_mut().create_signal();
        let geometry_sig = app.scheduler.borrow_mut().create_signal();
        let win = eaglemode_rs::emCore::emWindow::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
            flags_sig,
            focus_sig,
            geometry_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        {
            let win = app.windows.get_mut(&wid).unwrap();
            let flags = win.view().flags | ViewFlags::ROOT_SAME_TALLNESS;
            win.view_mut().SetViewFlags(flags, &mut app.tree);
        }
    }));
    app.run();
}
