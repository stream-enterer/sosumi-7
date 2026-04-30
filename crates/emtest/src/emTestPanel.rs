//! emTestPanel — plugin port of C++ emTest/emTestPanel.cpp.
//!
//! Task 6 scope: TestPanel + TkTestGrp + TkTest with the core widget groups
//! (Buttons, Check, Radio, Text, Scalar sf1–sf3, Color). Adds BgColor
//! persistence via emVarModel keyed on the panel identity, plus the teddy.tga
//! test image and a flat PolyDrawPanel placeholder. Extended widget groups
//! (Tunnels, ListBoxes, dialogs, file selection) and the structured
//! PolyDrawPanel are deferred to later tasks (7–11).

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emColorField::emColorField;
use emcore::emContext::emContext;
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::{ConstructCtx, EngineCtx, PanelCtx, SchedCtx};
use emcore::emImage::emImage;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLabel::emLabel;
use emcore::emLook::emLook;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::{PanelId, ViewConditionType};
use emcore::emRadioBox::emRadioBox;
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use emcore::emRasterGroup::emRasterGroup;
use emcore::emRasterLayout::emRasterLayout;
use emcore::emRes::emGetInsResImage;
use emcore::emScalarField::emScalarField;
use emcore::emSignal::SignalId;
use emcore::emStroke::{emStroke, LineCap, LineJoin};
use emcore::emStrokeEnd::{emStrokeEnd, StrokeEndType};
use emcore::emTextField::emTextField;
use emcore::emTexture::{emTexture, ImageExtension, ImageQuality};
use emcore::emTunnel::emTunnel;
use emcore::emVarModel;

// ─── constants ──────────────────────────────────────────────────────

const MAX_DEPTH: u32 = 10;
const MAX_LOG_ENTRIES: usize = 20;
const DEFAULT_BG: emColor = emColor::rgba(0x00, 0x1C, 0x38, 0xFF);

const CHILD_LAYOUT: [(&str, f64, f64, f64, f64); 7] = [
    ("TkTestGrp", 0.20, 0.15, 0.30, 0.12),
    ("1", 0.70, 0.05, 0.12, 0.12),
    ("2", 0.83, 0.05, 0.12, 0.12),
    ("3", 0.70, 0.18, 0.12, 0.12),
    ("4", 0.83, 0.18, 0.12, 0.12),
    ("BgColorField", 0.775, 0.34, 0.10, 0.02),
    ("PolyDraw", 0.05, 0.92, 0.08, 0.04),
];

// ─── widget wrapper PanelBehaviors ──────────────────────────────────

struct ButtonPanel {
    widget: emButton,
}
impl PanelBehavior for ButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.cycle_blink(s.in_focused_path());
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.widget.on_focus_changed(state.in_focused_path());
        }
    }
}

struct ScalarFieldPanel {
    widget: emScalarField,
}
impl PanelBehavior for ScalarFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

/// sf6 "Play Position" wrapper that reads a dynamic max from a shared Cell before
/// painting. The Cell is written by sf5's `on_value` callback (synchronously).
/// The max update cannot use `SetMaxValue` (requires PanelCtx); `set_max_silent`
/// keeps the display in sync without firing signals.
struct ScalarFieldWithDynamicMax {
    widget: emScalarField,
    max_ref: Rc<Cell<f64>>,
}

impl PanelBehavior for ScalarFieldWithDynamicMax {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let new_max = self.max_ref.get();
        self.widget.set_max_silent(new_max);
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, canvas_color, w, h, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.widget.create_expansion_children(ctx);
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.widget.LayoutChildren(ctx, rect.w, rect.h);
    }
    fn Cycle(&mut self, _ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        self.widget.Cycle(ctx)
    }
}

/// Wraps emLabel as a control-panel child.
struct LabelPanel {
    widget: emLabel,
}
impl PanelBehavior for LabelPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .PaintContent(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
}

// ─── TestPanel ──────────────────────────────────────────────────────

/// Shared bg color slot — written by ColorField on_color callback (synchronous,
/// from `SetColor` via `SchedCtx::fire`), read by TestPanel::Paint and Drop.
/// The C++ original drives the same data flow via Cycle + IsSignaled; the
/// Rust callback hop is synchronous within the same input/cycle pass and is
/// not a Cycle-drained polling intermediary.
type BgShared = Rc<Cell<emColor>>;

pub(crate) struct TestPanel {
    depth: u32,
    /// Root-context handle for VarModel lookups in Drop.
    root_ctx: Rc<emContext>,
    /// `"emTestPanel - BgColor of " + identity` — populated lazily on first
    /// `LayoutChildren` once the tree assigns this panel its identity path.
    /// Empty until then; Drop checks for empty before persisting.
    identity_key: String,
    bg_shared: BgShared,
    input_log: Vec<String>,
    test_image: emImage,
}

impl TestPanel {
    pub(crate) fn new(depth: u32, root_ctx: Rc<emContext>, initial_bg: emColor) -> Self {
        let test_image = emGetInsResImage("emTest", "icons/teddy.tga");
        Self {
            depth,
            root_ctx,
            identity_key: String::new(),
            bg_shared: Rc::new(Cell::new(initial_bg)),
            input_log: Vec::new(),
            test_image,
        }
    }

    fn bg_color(&self) -> emColor {
        self.bg_shared.get()
    }

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

        p.PaintPolygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg, bg);

        // Holed polygon (even-odd winding) — C++ PaintPolygon 10-pt outer+inner square.
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
            bg,
        );

        // Holed polygon (non-zero winding, reversed inner).
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
            bg,
        );

        let circle: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle, emColor::YELLOW, bg);

        // Clipped circle — C++ creates sub-painter with restricted clip rect.
        p.push_state();
        p.SetClipping(0.51, 0.81, 0.08, 0.08);
        let circle2: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle2, emColor::GREEN, bg);
        p.pop_state();

        // Ellipse (polygon approximation).
        let ellipse: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        p.PaintPolygon(&ellipse, emColor::rgba(255, 0, 0, 92), bg);

        p.PaintPolygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            bg,
        );
        p.PaintPolygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], emColor::RED, bg);
        p.PaintPolygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            bg,
        );
        p.PaintPolygon(&[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)], emColor::RED, bg);

        // Thin triangles.
        p.PaintPolygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            emColor::rgba(187, 136, 255, 192),
            bg,
        );
        p.PaintPolygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            emColor::rgba(136, 187, 255, 192),
            bg,
        );

        // Bowtie (self-intersecting quad).
        p.PaintPolygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            emColor::rgba(136, 187, 255, 192),
            bg,
        );

        // Ellipses (cx, cy, rx, ry).
        p.PaintEllipse(0.055, 0.805, 0.005, 0.005, emColor::WHITE, bg);
        p.PaintEllipse(0.07, 0.805, 0.01, 0.005, emColor::WHITE, bg);
        p.PaintEllipse(0.0925, 0.805, 0.0025, 0.005, emColor::WHITE, bg);

        // Ellipse sectors.
        p.PaintEllipseSector(0.105, 0.805, 0.005, 0.005, 45.0, 305.0, emColor::WHITE, bg);
        p.PaintEllipseSector(0.12, 0.805, 0.01, 0.005, 45.0, -395.0, emColor::WHITE, bg);

        // Rect outlines.
        let thin_stroke = emStroke::new(emColor::WHITE, 0.001);
        p.PaintRectOutline(0.05, 0.82, 0.01, 0.01, &thin_stroke, bg);
        let thick_stroke = emStroke::new(emColor::WHITE, 0.008);
        p.PaintRectOutline(0.10, 0.82, 0.01, 0.01, &thick_stroke, bg);

        // Round rects.
        p.PaintRoundRect(0.05, 0.84, 0.01, 0.01, 0.001, 0.001, emColor::WHITE, bg);
        p.PaintRoundRect(0.07, 0.84, 0.02, 0.01, 0.002, 0.002, emColor::WHITE, bg);
        p.PaintRoundRect(0.10, 0.84, 0.01, 0.01, 0.003, 0.003, emColor::WHITE, bg);

        // Ellipse outlines.
        let outline_stroke = emStroke::new(emColor::WHITE, 0.003);
        p.PaintEllipseOutline(0.055, 0.865, 0.005, 0.005, &outline_stroke, bg);
        let thin_outline = emStroke::new(emColor::WHITE, 0.001);
        p.PaintEllipseOutline(0.075, 0.865, 0.01, 0.005, &thin_outline, bg);

        // Round rect outlines.
        let rr_stroke = emStroke::new(emColor::WHITE, 0.001);
        p.PaintRoundRectOutline(0.05, 0.88, 0.01, 0.01, 0.001, 0.001, &rr_stroke, bg);
        p.PaintRoundRectOutline(0.07, 0.88, 0.02, 0.01, 0.002, 0.002, &rr_stroke, bg);

        // Bezier curves.
        p.PaintBezier(
            &[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)],
            emColor::WHITE,
            bg,
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
            bg,
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
            bg,
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
            bg,
        );

        // All StrokeEndType variants in radial pattern.
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
                bg,
            );
        }

        // Polyline with contour arrow.
        let mut poly_s = emStroke::new(emColor::WHITE, 0.0005);
        poly_s.cap = LineCap::Round;
        poly_s.join = LineJoin::Round;
        poly_s.start_end = emStrokeEnd::new(StrokeEndType::ContourArrow);
        poly_s.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
        p.PaintPolyline(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &poly_s,
            false,
            bg,
        );

        // Polygon outline.
        p.PaintPolygonOutline(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            emColor::RED,
            0.0002,
            bg,
        );

        // Textured polygons — star shapes (matching C++ emLinearGradientTexture /
        // emRadialGradientTexture / emImageTexture polygon calls).
        let star = make_star(0.215, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star,
            &emTexture::LinearGradient {
                color_a: emColor::rgba(0, 0xFF, 0, 0x80),
                color_b: emColor::rgba(0xFF, 0xFF, 0, 0xFF),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
            bg,
        );

        let star2 = make_star(0.235, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star2,
            &emTexture::RadialGradient {
                color_inner: emColor::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: emColor::rgba(0, 0, 0xFF, 0x60),
                center: (0.21, 0.90),
                radius_x: 0.05,
                radius_y: 0.035,
            },
            bg,
        );

        let h_ratio = if self.test_image.GetWidth() > 0 {
            0.001 * self.test_image.GetHeight() as f64 / self.test_image.GetWidth() as f64
        } else {
            0.001
        };
        let star3 = make_star(0.255, 0.917, 0.015, 0.015, 8);
        p.paint_polygon_textured(
            &star3,
            &emTexture::emImage {
                image: self.test_image.clone(),
                x: 0.0,
                y: 0.0,
                w: 0.002,
                h: h_ratio,
                alpha: 255,
                extension: ImageExtension::Repeat,
                quality: ImageQuality::Bilinear,
            },
            bg,
        );

        // Gradient rects.
        p.paint_linear_gradient(
            0.2,
            0.94,
            0.02,
            0.01,
            emColor::rgba(0, 0, 0, 0x80),
            emColor::rgba(0x80, 0x80, 0x80, 0x80),
            true,
            bg,
        );
        p.paint_radial_gradient(
            0.225,
            0.946,
            0.004,
            0.008,
            emColor::rgba(0xFF, 0x88, 0, 0xFF),
            emColor::rgba(0, 0x55, 0, 0xFF),
            bg,
        );
        p.PaintEllipse(
            0.24,
            0.945,
            0.01,
            0.005,
            emColor::rgba(0, 0xCC, 0x88, 0xFF),
            bg,
        );

        // emImage scaled.
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

impl Drop for TestPanel {
    fn drop(&mut self) {
        if self.identity_key.is_empty() {
            return;
        }
        let bg = self.bg_shared.get();
        if bg != DEFAULT_BG {
            emVarModel::Set(&self.root_ctx, &self.identity_key, bg);
        }
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

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let bg = self.bg_color();
        let fg = if state.is_focused() {
            emColor::rgba(255, 136, 136, 255)
        } else if state.in_focused_path() {
            emColor::rgba(187, 136, 136, 255)
        } else {
            emColor::rgba(136, 136, 136, 255)
        };

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
            bg,
        );

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
            0.0,
        );

        if state.viewed_rect.w < 25.0 {
            painter.pop_state();
            return;
        }

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
            0.0,
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
            0.0,
        );

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

        self.paint_primitives(painter, fg, bg);
        painter.pop_state();
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
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

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Lazy identity-key init: the tree assigns identity at insertion time.
        if self.identity_key.is_empty() {
            let identity = ctx.tree.GetIdentity(ctx.id);
            // Mirror C++ key: emVarModel<emColor>::GetAndRemove key is
            // "emTestPanel - BgColor of " + GetIdentity().
            let key = format!("emTestPanel - BgColor of {identity}");
            // Restore persisted bg if present.
            let bg = emVarModel::GetAndRemove(&self.root_ctx, &key, self.bg_shared.get());
            self.bg_shared.set(bg);
            self.identity_key = key;
        }

        let bg = self.bg_color();

        if !ctx.children().is_empty() {
            for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
                if let Some(child) = ctx.find_child_by_name(name) {
                    ctx.layout_child_canvas(child, x, y, cw, ch, bg);
                }
            }
            return;
        }

        let bg_shared = self.bg_shared.clone();
        let root_ctx = self.root_ctx.clone();

        // TkTestGrp — C++ AutoExpand creates child named "TkTestGrp".
        let tktest_id = ctx.create_child_with("TkTestGrp", Box::new(TkTestGrpPanel::new()));
        ctx.tree.SetAutoExpansionThreshold(
            tktest_id,
            900.0,
            ViewConditionType::Area,
            ctx.scheduler.as_deref_mut(),
        );

        // Recursive child TestPanels — C++ names are "1", "2", "3", "4".
        if self.depth < MAX_DEPTH {
            for i in 1..=4u32 {
                let tp_id = ctx.create_child_with(
                    &format!("{i}"),
                    Box::new(TestPanel::new(self.depth + 1, root_ctx.clone(), DEFAULT_BG)),
                );
                ctx.tree.SetAutoExpansionThreshold(
                    tp_id,
                    900.0,
                    ViewConditionType::Area,
                    ctx.scheduler.as_deref_mut(),
                );
            }
        }

        // Background ColorField — C++ name "BgColorField".
        let bg_for_cf = bg_shared.clone();
        let mut cf = emColorField::new(ctx, emLook::new());
        cf.SetCaption("Background Color");
        cf.SetEditable(true);
        cf.set_initial_alpha_enabled(true);
        cf.set_initial_color(bg_shared.get());
        cf.on_color = Some(Box::new(move |color, _sched: &mut SchedCtx<'_>| {
            bg_for_cf.set(color);
        }));
        ctx.create_child_with("BgColorField", Box::new(ColorFieldPanel { widget: cf }));

        // PolyDraw — C++ name "PolyDraw" (flat placeholder; Task 11 restructures).
        ctx.create_child_with("PolyDraw", Box::new(PolyDrawPanel::new()));

        for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
            if let Some(child) = ctx.find_child_by_name(name) {
                ctx.layout_child_canvas(child, x, y, cw, ch, bg);
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

// ─── TkTestGrpPanel ─────────────────────────────────────────────────

struct TkTestGrpPanel {
    border: emBorder,
    look: Rc<emLook>,
    children_created: bool,
}

impl TkTestGrpPanel {
    fn new() -> Self {
        let look = emLook::new();
        let border = emBorder::new(OuterBorderType::Group)
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
    fn IsOpaque(&self) -> bool {
        true
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        self.border.paint_border(
            p,
            canvas_color,
            w,
            h,
            &self.look,
            s.is_focused(),
            s.enabled,
            1.0,
        );
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // DIVERGED: (dependency-forced) — emSplitter is not yet ported; 2×2 grid
        // laid out manually. C++ TkTestGrp::AutoExpand creates nested emSplitter
        // panels (emTestPanel.cpp:882-908). Observable difference: panel
        // proportions use fixed 80/20 splits instead of user-draggable splitters.
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;
            // C++: t1a, t1b go in sp1 (vertical, left 80 %);
            //      t2a, t2b go in sp2 (vertical, right 20 %).
            ctx.create_child_with("t1a", Box::new(TkTestPanel::new(self.look.clone())));
            ctx.create_child_with("t1b", Box::new(TkTestPanel::new(self.look.clone())));
            ctx.create_child_with("t2a", Box::new(TkTestPanel::new(self.look.clone())));
            let t2b_id = ctx.create_child_with(
                "t2b",
                // C++ emTestPanel.cpp:910: t2b->SetCaption("Disabled").
                Box::new(TkTestPanel::new(self.look.clone()).with_caption("Disabled")),
            );
            // C++ emTestPanel.cpp:909: t2b->SetEnableSwitch(false).
            ctx.tree
                .SetEnableSwitch(t2b_id, false, ctx.scheduler.as_deref_mut());
        }

        // sp->SetPos(0.8): horizontal split — sp1 left 80 %, sp2 right 20 %.
        // sp1->SetPos(0.8), sp2->SetPos(0.8): vertical split — top 80 %, bottom 20 %.
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        let left_w = cr.w * 0.8;
        let right_w = cr.w * 0.2;
        let top_h = cr.h * 0.8;
        let bot_h = cr.h * 0.2;

        if let Some(id) = ctx.find_child_by_name("t1a") {
            ctx.layout_child(id, cr.x, cr.y, left_w, top_h);
        }
        if let Some(id) = ctx.find_child_by_name("t1b") {
            ctx.layout_child(id, cr.x, cr.y + top_h, left_w, bot_h);
        }
        if let Some(id) = ctx.find_child_by_name("t2a") {
            ctx.layout_child(id, cr.x + left_w, cr.y, right_w, top_h);
        }
        if let Some(id) = ctx.find_child_by_name("t2b") {
            ctx.layout_child(id, cr.x + left_w, cr.y + top_h, right_w, bot_h);
        }

        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ─── TkTest helper formatters ────────────────────────────────────────

/// C++ `TextOfTimeValue` (emTestPanel.cpp:844-870).
fn text_of_time_value(val: i64, mark_interval: u64) -> String {
    let ms = val.unsigned_abs();
    let h = ms / 3_600_000;
    let m = (ms / 60_000) % 60;
    let s = (ms / 1_000) % 60;
    let ms_r = ms % 1_000;
    match mark_interval {
        0..=9 => format!("{h:02}:{m:02}:{s:02}\n.{ms_r:03}"),
        10..=99 => format!("{h:02}:{m:02}:{s:02}\n.{:02}", ms_r / 10),
        100..=999 => format!("{h:02}:{m:02}:{s:02}\n.{}", ms_r / 100),
        1_000..=59_999 => format!("{h:02}:{m:02}:{s:02}"),
        _ => format!("{h:02}:{m:02}"),
    }
}

/// C++ `TextOfLevelValue` (emTestPanel.cpp:873-878).
fn text_of_level_value(val: i64, _mark_interval: u64) -> String {
    format!("Level {val}")
}

// ─── TkTestPanel — core widget groups ────────────────────────────────

struct TkTestPanel {
    look: Rc<emLook>,
    border: emBorder,
    layout: emRasterLayout,
    children_created: bool,
    /// PlayLength value signal (sf5) — stored for Cycle (Task 9).
    sf5_len_signal: Option<SignalId>,
    /// Current max for sf6 "Play Position" — set by sf5's on_value callback,
    /// read by ScalarFieldWithDynamicMax::Paint before rendering.
    sf6_max: Rc<Cell<f64>>,
}

impl TkTestPanel {
    fn new(look: Rc<emLook>) -> Self {
        let border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Toolkit Test");
        let mut layout = emRasterLayout::new();
        layout.preferred_child_tallness = 0.3;
        // sf5 initial value is 4h in ms; sf6 max starts equal to sf5's initial value.
        let sf5_initial = 4.0 * 3_600_000.0_f64;
        Self {
            look,
            border,
            layout,
            children_created: false,
            sf5_len_signal: None,
            sf6_max: Rc::new(Cell::new(sf5_initial)),
        }
    }

    /// Override the border caption — used to set "Disabled" on t2b
    /// (C++ emTestPanel.cpp:910: `t2b->SetCaption("Disabled")`).
    fn with_caption(mut self, caption: &str) -> Self {
        self.border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption(caption);
        self
    }

    fn make_category(ctx: &mut PanelCtx, name: &str, caption: &str, pct: Option<f64>) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(2.5);
        rg.border.caption = caption.to_string();
        if let Some(p) = pct {
            rg.layout.preferred_child_tallness = p;
        }
        let id = ctx.tree.create_child(ctx.id, name, None);
        ctx.tree.set_behavior(id, Box::new(rg));
        id
    }

    fn create_all_categories(&mut self, ctx: &mut PanelCtx) {
        let look = self.look.clone();

        // 1. Buttons — C++ emTestPanel.cpp:556-566.
        let gid = Self::make_category(ctx, "buttons", "Buttons", None);
        {
            let id = ctx.tree.create_child(gid, "b1", None);
            let w = emButton::new(ctx, "Button", look.clone());
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: w }));

            let mut b2 = emButton::new(ctx, "Long Desc", look.clone());
            let mut desc = String::new();
            for _ in 0..100 {
                desc.push_str("This is a looooooooooooooooooooooooooooooooooooooooooooooooooooooong description of the button.\n");
            }
            b2.SetDescription(&desc);
            let id = ctx.tree.create_child(gid, "b2", None);
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b2 }));

            let mut b3 = emButton::new(ctx, "NoEOI", look.clone());
            b3.SetNoEOI(true);
            let id = ctx.tree.create_child(gid, "b3", None);
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b3 }));
        }

        // 2. Check Buttons and Boxes — C++ :568-575.
        let gid = Self::make_category(ctx, "checkbuttons", "Check Buttons and Boxes", None);
        {
            for i in 1..=3 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"), None);
                let w = emCheckButton::new(ctx, "Check Button", look.clone());
                ctx.tree
                    .set_behavior(id, Box::new(CheckButtonPanel { widget: w }));
            }
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"), None);
                let w = emCheckBox::new(ctx, "Check Box", look.clone());
                ctx.tree
                    .set_behavior(id, Box::new(CheckBoxPanel { widget: w }));
            }
        }

        // 3. Radio Buttons and Boxes — C++ :577-584.
        // C++: emRadioBox extends emRadioButton; all 6 widgets (r1-r3 buttons,
        // r4-r6 boxes) share the same RasterGroup parent, so selecting any one
        // deselects the others. One RadioGroup covers all six.
        let gid = Self::make_category(ctx, "radiobuttons", "Radio Buttons and Boxes", None);
        {
            let rg = RadioGroup::new(ctx);
            for i in 1..=3usize {
                let id = ctx.tree.create_child(gid, &format!("r{i}"), None);
                let w = emRadioButton::new(ctx, "Radio Button", look.clone(), rg.clone(), i - 1);
                ctx.tree
                    .set_behavior(id, Box::new(RadioButtonPanel { widget: w }));
            }
            for i in 4..=6usize {
                let id = ctx.tree.create_child(gid, &format!("r{i}"), None);
                let w = emRadioBox::new("Radio Box", look.clone(), rg.clone(), i - 4);
                ctx.tree
                    .set_behavior(id, Box::new(RadioBoxPanel { widget: w }));
            }
        }

        // 4. Text Fields — C++ :586-609.
        let gid = Self::make_category(ctx, "textfields", "Text Fields", None);
        {
            let mut tf1 = emTextField::new(ctx, look.clone());
            tf1.SetCaption("Read-Only");
            tf1.SetDescription("This is a read-only text field.");
            tf1.SetText("Read-Only");
            let id = ctx.tree.create_child(gid, "tf1", None);
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf1 }));

            let mut tf2 = emTextField::new(ctx, look.clone());
            tf2.SetCaption("Editable");
            tf2.SetDescription("This is an editable text field.");
            tf2.SetEditable(true);
            tf2.SetText("Editable");
            let id = ctx.tree.create_child(gid, "tf2", None);
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf2 }));

            let mut tf3 = emTextField::new(ctx, look.clone());
            tf3.SetCaption("Password");
            tf3.SetDescription("This is an editable password text field.");
            tf3.SetEditable(true);
            tf3.SetText("Password");
            tf3.SetPasswordMode(true);
            let id = ctx.tree.create_child(gid, "tf3", None);
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf3 }));

            let mut mltf1 = emTextField::new(ctx, look.clone());
            mltf1.SetCaption("Multi-Line");
            mltf1.SetDescription("This is an editable multi-line text field.");
            mltf1.SetEditable(true);
            mltf1.SetMultiLineMode(true);
            mltf1.SetText("first line\nsecond line\n...");
            let id = ctx.tree.create_child(gid, "mltf1", None);
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: mltf1 }));
        }

        // 5. Scalar Fields (sf1–sf6) — C++ :611-660.
        let gid = Self::make_category(ctx, "scalarfields", "Scalar Fields", Some(0.1));
        {
            let mut sf1 = emScalarField::new(ctx, 0.0, 10.0, look.clone());
            sf1.SetCaption("Read-Only");
            let id = ctx.tree.create_child(gid, "sf1", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf1 }));

            let mut sf2 = emScalarField::new(ctx, 0.0, 10.0, look.clone());
            sf2.SetCaption("Editable");
            sf2.SetEditable(true);
            let id = ctx.tree.create_child(gid, "sf2", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf2 }));

            let mut sf3 = emScalarField::new(ctx, -1000.0, 1000.0, look.clone());
            sf3.SetEditable(true);
            sf3.set_initial_value(0.0);
            sf3.SetScaleMarkIntervals(&[1000, 100, 10, 5, 1]);
            let id = ctx.tree.create_child(gid, "sf3", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf3 }));

            // sf4 — Level — C++ :624-630.
            let mut sf4 = emScalarField::new(ctx, 1.0, 5.0, look.clone());
            sf4.SetCaption("Level");
            sf4.SetEditable(true);
            sf4.SetTextBoxTallness(0.25);
            sf4.set_initial_value(3.0);
            sf4.SetTextOfValueFunc(Box::new(text_of_level_value));
            let id = ctx.tree.create_child(gid, "sf4", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf4 }));

            // sf5 — Play Length — C++ :632-638. Captures sf6_max for on_value.
            let sf6_max = Rc::clone(&self.sf6_max);
            let mut sf5 = emScalarField::new(ctx, 0.0, 24.0 * 3_600_000.0, look.clone());
            sf5.SetCaption("Play Length");
            sf5.SetEditable(true);
            sf5.set_initial_value(4.0 * 3_600_000.0);
            sf5.SetScaleMarkIntervals(&[
                3_600_000, 900_000, 300_000, 60_000, 10_000, 1_000, 100, 10, 1,
            ]);
            sf5.SetTextOfValueFunc(Box::new(text_of_time_value));
            self.sf5_len_signal = Some(sf5.value_signal);
            sf5.on_value = Some(Box::new(move |val, _sched| {
                sf6_max.set(val);
            }));
            let id = ctx.tree.create_child(gid, "sf5", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf5 }));

            // sf6 — Play Position — C++ :640-644. Dynamic max tracks sf5's value.
            let sf6_max_ref = Rc::clone(&self.sf6_max);
            let sf6_initial_max = self.sf6_max.get();
            let mut sf6 = emScalarField::new(ctx, 0.0, sf6_initial_max, look.clone());
            sf6.SetCaption("Play Position");
            sf6.SetEditable(true);
            sf6.SetScaleMarkIntervals(&[
                3_600_000, 900_000, 300_000, 60_000, 10_000, 1_000, 100, 10, 1,
            ]);
            sf6.SetTextOfValueFunc(Box::new(text_of_time_value));
            let id = ctx.tree.create_child(gid, "sf6", None);
            ctx.tree.set_behavior(
                id,
                Box::new(ScalarFieldWithDynamicMax {
                    widget: sf6,
                    max_ref: sf6_max_ref,
                }),
            );
        }

        // 6. Color Fields — C++ :646-660.
        let gid = Self::make_category(ctx, "colorfields", "Color Fields", Some(0.4));
        {
            let mut cf1 = emColorField::new(ctx, look.clone());
            cf1.SetCaption("Read-Only");
            cf1.set_initial_color(emColor::rgba(0xBB, 0x22, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf1", None);
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf1 }));

            let mut cf2 = emColorField::new(ctx, look.clone());
            cf2.SetCaption("Editable");
            cf2.SetEditable(true);
            cf2.set_initial_color(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf2", None);
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf2 }));

            let mut cf3 = emColorField::new(ctx, look.clone());
            cf3.SetCaption("Editable, Alpha Enabled");
            cf3.SetEditable(true);
            cf3.set_initial_alpha_enabled(true);
            cf3.set_initial_color(emColor::rgba(0x22, 0x22, 0xBB, 0xFF));
            let id = ctx.tree.create_child(gid, "cf3", None);
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf3 }));
        }

        // 7. Tunnels — C++ :662-680.
        // `emTunnel` implements `PanelBehavior` directly; content is created as
        // a child of the tunnel panel in the tree (matching C++ parent/child hierarchy).
        let gid = Self::make_category(ctx, "tunnels", "Tunnels", Some(0.4));
        {
            // t1: default depth, emButton content — C++ :666-667.
            let t1 = emTunnel::new(look.clone()).with_caption("Tunnel");
            let t1_id = ctx.tree.create_child(gid, "t1", None);
            ctx.tree.set_behavior(t1_id, Box::new(t1));
            {
                let btn = emButton::new(ctx, "End Of Tunnel", look.clone());
                let e_id = ctx.tree.create_child(t1_id, "e", None);
                ctx.tree
                    .set_behavior(e_id, Box::new(ButtonPanel { widget: btn }));
            }

            // t2: SetDepth(30.0), emRasterGroup content — C++ :669-671.
            let mut t2 = emTunnel::new(look.clone()).with_caption("Deeper Tunnel");
            t2.SetDepth(30.0);
            let t2_id = ctx.tree.create_child(gid, "t2", None);
            ctx.tree.set_behavior(t2_id, Box::new(t2));
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                let e_id = ctx.tree.create_child(t2_id, "e", None);
                ctx.tree.set_behavior(e_id, Box::new(rg));
            }

            // t3: SetChildTallness(1.0), emRasterGroup content — C++ :673-675.
            let mut t3 = emTunnel::new(look.clone()).with_caption("Square End");
            t3.SetChildTallness(1.0);
            let t3_id = ctx.tree.create_child(gid, "t3", None);
            ctx.tree.set_behavior(t3_id, Box::new(t3));
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                let e_id = ctx.tree.create_child(t3_id, "e", None);
                ctx.tree.set_behavior(e_id, Box::new(rg));
            }

            // t4: SetChildTallness(1.0), SetDepth(0.0), emRasterGroup content — C++ :677-680.
            let mut t4 = emTunnel::new(look.clone()).with_caption("Square End, Zero Depth");
            t4.SetChildTallness(1.0);
            t4.SetDepth(0.0);
            let t4_id = ctx.tree.create_child(gid, "t4", None);
            ctx.tree.set_behavior(t4_id, Box::new(t4));
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                let e_id = ctx.tree.create_child(t4_id, "e", None);
                ctx.tree.set_behavior(e_id, Box::new(rg));
            }
        }
    }
}

impl PanelBehavior for TkTestPanel {
    fn IsOpaque(&self) -> bool {
        true
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        self.border.paint_border(
            p,
            canvas_color,
            w,
            h,
            &self.look,
            s.is_focused(),
            s.enabled,
            1.0,
        );
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        if !self.children_created {
            self.children_created = true;
            self.create_all_categories(ctx);
        }
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        self.layout.do_layout_skip(ctx, None, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ─── PolyDrawPanel — flat placeholder ───────────────────────────────
//
// Task 11 will restructure this to mirror the C++ PolyDrawPanel splitter
// hierarchy; for Task 6 we keep a non-interactive flat panel that paints a
// gradient + polygon so the layout slot is filled.

struct PolyDrawPanel {
    vertices: Vec<(f64, f64)>,
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
        Self { vertices }
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn IsOpaque(&self) -> bool {
        true
    }
    fn Paint(
        &mut self,
        p: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        p.PaintRect(
            0.0,
            0.0,
            w,
            h,
            emColor::rgba(80, 80, 160, 0xFF),
            canvas_color,
        );
        let scaled: Vec<(f64, f64)> = self
            .vertices
            .iter()
            .map(|&(vx, vy)| (vx * w, vy * h))
            .collect();
        p.PaintPolygon(&scaled, emColor::WHITE, emColor::TRANSPARENT);
        p.PaintTextBoxed(
            0.0,
            h - 0.05 * h,
            w,
            0.05 * h,
            "Poly Draw",
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

// ─── helpers ────────────────────────────────────────────────────────

/// Star polygon helper used in paint_primitives textured-polygon demos.
/// Alternates outer (r=1.0) and inner (r=0.4) vertices around (cx, cy).
fn make_star(cx: f64, cy: f64, rx: f64, ry: f64, points: usize) -> Vec<(f64, f64)> {
    let mut verts = Vec::with_capacity(points * 2);
    for i in 0..(points * 2) {
        let a = PI * i as f64 / points as f64 - PI / 2.0;
        let r = if i % 2 == 0 { 1.0 } else { 0.4 };
        verts.push((cx + a.cos() * rx * r, cy + a.sin() * ry * r));
    }
    verts
}

// ─── plugin entry ───────────────────────────────────────────────────

pub(crate) fn new_root_panel(ctx: &mut dyn ConstructCtx) -> Box<dyn PanelBehavior> {
    let root_ctx = ctx.root_context().clone();
    Box::new(TestPanel::new(0, root_ctx, DEFAULT_BG))
}
