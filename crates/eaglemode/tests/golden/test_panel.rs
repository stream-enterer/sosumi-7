//! TestPanel integration golden tests.
//!
//! Compares rendered output of the Rust TestPanel against C++ emTestPanel.
//! Both C++ and Rust use teddy.tga (209x256 RGBA) as the test GetImage. Paint primitives
//! (polygons, ellipses, strokes, beziers) should match within tolerance.
//!
//! Two tests:
//! - `testpanel_root`: Root panel PaintContent only (no auto-expansion). Tests
//!   PaintContent primitives, text, and background rendering.
//! - `testpanel_expanded`: Full tree with auto-expanded children. Tests
//!   integration of layout, widget rendering, and multi-panel composition.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emCursor::emCursor;
use emcore::emImage::emImage;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLinearGroup::emLinearGroup;
use emcore::emLinearLayout::emLinearLayout;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emRasterGroup::emRasterGroup;
use emcore::emRasterLayout::emRasterLayout;
use emcore::emResTga::load_tga;
use emcore::emTiling::{ChildConstraint, Orientation};

use emcore::emPanelCtx::PanelCtx;

use emcore::emPanelTree::{PanelId, PanelTree, ViewConditionType};

use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emView::{emView, ViewFlags};

use emcore::emStroke::{emStroke, LineCap, LineJoin};

use emcore::emStrokeEnd::{emStrokeEnd, StrokeEndType};

use emcore::emTexture::{emTexture, ImageExtension, ImageQuality};

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emPainterDrawList::RecordedOp;
use emcore::emViewRenderer::SoftwareCompositor;

use super::draw_op_dump::{dump_draw_ops, dump_draw_ops_enabled};

use emcore::emButton::emButton;

use emcore::emCheckBox::emCheckBox;

use emcore::emCheckButton::emCheckButton;

use emcore::emColorField::emColorField;

use emcore::emListBox::{emListBox, SelectionMode};

use emcore::emLook::emLook;

use emcore::emRadioBox::emRadioBox;

use emcore::emRadioButton::{emRadioButton, RadioGroup};

use emcore::emScalarField::emScalarField;

use emcore::emSplitter::emSplitter;

use emcore::emTextField::emTextField;

use emcore::emTunnel::emTunnel;

use emcore::emFileSelectionBox::emFileSelectionBox;

use emcore::emLabel::emLabel;

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
/// `rounds` Match C++ TerminateEngine Cycle GetCount from gen_golden.cpp.
fn settle(tree: &mut PanelTree, view: &mut emView, rounds: usize) {
    for _ in 0..rounds {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(tree);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Constants — match C++ emTestPanel
// ═══════════════════════════════════════════════════════════════════

const MAX_DEPTH: u32 = 2;
const DEFAULT_BG: emColor = emColor::rgba(0x00, 0x1C, 0x38, 0xFF);

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
    fn auto_expand(&self) -> bool {
        true
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            self.widget.create_expansion_children(ctx);
        }
        let rect = ctx.layout_rect();
        self.widget.LayoutChildren(ctx, rect.w, rect.h);
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
    fn auto_expand(&self) -> bool {
        true
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            self.widget.create_item_children(ctx);
        }
        let rect = ctx.layout_rect();
        self.widget.layout_item_children(ctx, rect.w, rect.h);
    }
}

/// Wraps emLabel as a PanelBehavior for use as a child panel.
struct LabelPanel {
    widget: emLabel,
}
impl PanelBehavior for LabelPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.PaintContent(p, w, h, s.enabled, pixel_scale);
    }
}

/// Custom list box item panel — matches C++ emTestPanel::CustomItemPanel.
///
/// C++ CustomItemPanel inherits emLinearGroup (horizontal, border_scaling=5.0).
/// On expand, creates "t" (label) and "l" (recursive child CustomListBox).
/// When selected, changes look bg_color to (224,80,128).
struct CustomItemPanelBehavior {
    group: emLinearGroup,
    look: Rc<emLook>,
    children_created: bool,
}

impl CustomItemPanelBehavior {
    fn new(text: String, selected: bool, look: Rc<emLook>) -> Self {
        let mut group = emLinearGroup::horizontal();
        group.border.SetBorderScaling(5.0);
        group.border.caption = text.clone();
        // C++ ItemSelectionChanged: if selected, set look bg to (224,80,128)
        if selected {
            let mut item_look = (*look).clone();
            item_look.bg_color = emColor::rgb(224, 80, 128);
            group.look = item_look;
        } else {
            group.look = (*look).clone();
        }
        Self {
            group,
            look,
            children_created: false,
        }
    }
}

impl PanelBehavior for CustomItemPanelBehavior {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.group.Paint(p, w, h, s);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.children_created = true;

            // C++: label = new emLabel(this, "t", "This is a custom list\n...")
            let label = emLabel::new(
                "This is a custom list\nbox item panel (it is\nrecursive...)",
                self.look.clone(),
            );
            ctx.create_child_with("t", Box::new(LabelPanel { widget: label }));

            // C++: listBox = new CustomListBox(this, "l", "Child List Box")
            let mut child_lb = emListBox::new(self.look.clone());
            child_lb.SetCaption("Child List Box");
            child_lb.SetSelectionType(SelectionMode::Multi);
            for i in 1..=7 {
                child_lb.AddItem(format!("{i}"), format!("Item {i}"));
            }
            child_lb.SetSelectedIndex(0);
            // Recursive: child listbox items also use CustomItemPanelBehavior
            child_lb.set_item_behavior_factory(
                move |_i, text, selected, look, _sel_mode, _enabled| {
                    Box::new(CustomItemPanelBehavior::new(
                        text.to_string(),
                        selected,
                        look,
                    ))
                },
            );
            ctx.create_child_with("l", Box::new(ListBoxPanel { widget: child_lb }));
        }
        // Delegate layout to the emLinearGroup
        self.group.LayoutChildren(ctx);
    }
}

struct SplitterPanel {
    widget: emSplitter,
}
impl PanelBehavior for SplitterPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _s: &PanelState) {
        self.widget.PaintContent(p, w, h, _s.enabled);
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
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.widget.LayoutChildren(ctx, rect.w, rect.h);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Stub panels for unported C++ types
// ═══════════════════════════════════════════════════════════════════

/// Canvas panel for PolyDrawPanel — gradient background + polygon drawing.
/// Extracted from the original PolyDrawPanel.
struct CanvasPanel {
    vertices: Vec<(f64, f64)>,
    fill_color: emColor,
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
            fill_color: emColor::WHITE,
        }
    }
}

impl PanelBehavior for CanvasPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
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

        let scaled: Vec<(f64, f64)> = self
            .vertices
            .iter()
            .map(|&(vx, vy)| (vx * w, vy * h))
            .collect();
        p.PaintPolygon(&scaled, self.fill_color, emColor::TRANSPARENT);

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

// ═══════════════════════════════════════════════════════════════════
// TestPanel — root panel (derived from examples/test_panel.rs)
// ═══════════════════════════════════════════════════════════════════

struct TestPanel {
    bg_color_shared: Rc<Cell<emColor>>,
    test_image: emImage,
    depth: u32,
}

impl TestPanel {
    fn new(depth: u32, bg_color_shared: Rc<Cell<emColor>>) -> Self {
        let img = load_tga(include_bytes!("assets/teddy.tga")).expect("failed to load teddy.tga");
        Self {
            bg_color_shared,
            test_image: img,
            depth,
        }
    }

    fn bg_color(&self) -> emColor {
        self.bg_color_shared.get()
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

        // Triangle
        p.PaintPolygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg, bg);

        // Holed polygon (non-zero winding, same-direction inner — C++ PaintPolygon)
        p.PaintPolygon(
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
            bg,
        );

        // Circle (polygon approximation)
        let circle: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle, emColor::YELLOW, bg);

        // Clipped circle
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
            bg,
        );
        p.PaintPolygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], emColor::RED, bg);
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
        p.PaintPolygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            emColor::rgba(136, 187, 255, 192),
            emColor::TRANSPARENT,
        );

        // Ellipses (center + radius)
        p.PaintEllipse(0.055, 0.805, 0.005, 0.005, emColor::WHITE, bg);
        p.PaintEllipse(0.07, 0.805, 0.01, 0.005, emColor::WHITE, bg);
        p.PaintEllipse(0.0925, 0.805, 0.0025, 0.005, emColor::WHITE, bg);

        // Ellipse sectors
        p.PaintEllipseSector(0.105, 0.805, 0.005, 0.005, 45.0, 305.0, emColor::WHITE, bg);
        p.PaintEllipseSector(0.12, 0.805, 0.01, 0.005, 45.0, -395.0, emColor::WHITE, bg);

        // Rect outlines
        p.PaintRectOutline(
            0.05,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        p.PaintRectOutline(
            0.10,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.008),
            bg,
        );

        // Round rects
        p.SetCanvasColor(bg);
        p.PaintRoundRect(0.05, 0.84, 0.01, 0.01, 0.001, emColor::WHITE, emColor::TRANSPARENT);
        p.PaintRoundRect(0.07, 0.84, 0.02, 0.01, 0.002, emColor::WHITE, emColor::TRANSPARENT);
        p.PaintRoundRect(0.10, 0.84, 0.01, 0.01, 0.003, emColor::WHITE, emColor::TRANSPARENT);

        // Ellipse outlines
        p.PaintEllipseOutline(
            0.055,
            0.865,
            0.005,
            0.005,
            &emStroke::new(emColor::WHITE, 0.003),
            bg,
        );
        p.PaintEllipseOutline(
            0.075,
            0.865,
            0.01,
            0.005,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );

        // Round rect outlines
        p.SetCanvasColor(bg);
        p.PaintRoundRectOutline(
            0.05,
            0.88,
            0.01,
            0.01,
            0.001,
            &emStroke::new(emColor::WHITE, 0.001),
        );
        p.PaintRoundRectOutline(
            0.07,
            0.88,
            0.02,
            0.01,
            0.002,
            &emStroke::new(emColor::WHITE, 0.001),
        );

        // Bezier curves
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
            bg,
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

        painter.push_state();
        painter.scale(w, w);
        let panel_h = h / w;

        painter.PaintRect(0.0, 0.0, 1.0, panel_h, bg, painter.GetCanvasColor());
        painter.SetCanvasColor(bg);
        painter.PaintRectOutline(
            0.01,
            0.01,
            0.98,
            panel_h - 0.02,
            &emStroke::new(fg, 0.02),
            bg,
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

        // C++ emTestPanel.cpp:152 uses %f (6 decimal places) for GetPriority.
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

        // Paint primitives
        self.paint_primitives(painter, fg, bg);

        painter.pop_state();
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let bg = self.bg_color();

        if !children.is_empty() {
            for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
                if let Some(child) = ctx.find_child_by_name(name) {
                    ctx.layout_child_canvas(child, x, y, cw, ch, bg);
                }
            }
            return;
        }

        // Create children — Match C++ AutoExpand()
        let bg_shared = self.bg_color_shared.clone();

        ctx.create_child_with("tktest", Box::new(TkTestGrpPanel::new()));

        if self.depth < MAX_DEPTH {
            for i in 1..=4 {
                let child_bg = Rc::new(Cell::new(DEFAULT_BG));
                let tp_id = ctx.create_child_with(
                    &format!("tp{i}"),
                    Box::new(TestPanel::new(self.depth + 1, child_bg)),
                );
                ctx.tree
                    .SetAutoExpansionThreshold(tp_id, 900.0, ViewConditionType::Area);
            }
        }

        let bg_for_cf = bg_shared.clone();
        let mut cf = emColorField::new(emLook::new());
        cf.SetEditable(true);
        cf.SetAlphaEnabled(true);
        cf.SetColor(bg_shared.get());
        cf.on_color = Some(Box::new(move |color| {
            bg_for_cf.set(color);
        }));
        ctx.create_child_with("bgcf", Box::new(ColorFieldPanel { widget: cf }));

        ctx.create_child_with("polydraw", Box::new(PolyDrawPanel::new()));

        for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
            if let Some(child) = ctx.find_child_by_name(name) {
                ctx.layout_child_canvas(child, x, y, cw, ch, bg);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// TkTestGrpPanel — splitter hierarchy hosting TkTest widget showcases
// ═══════════════════════════════════════════════════════════════════

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

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled, 1.0);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;
            let look = self.look.clone();

            // sp: horizontal splitter, pos=0.8 (C++ emTestPanel.cpp:889)
            let mut sp = emSplitter::new(Orientation::Horizontal, look.clone());
            sp.SetPos(0.8);
            let sp_id = ctx.create_child_with("sp", Box::new(SplitterPanel { widget: sp }));

            // sp1: vertical splitter, child of sp, pos=0.8
            let mut sp1 = emSplitter::new(Orientation::Vertical, look.clone());
            sp1.SetPos(0.8);
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
            let mut sp2 = emSplitter::new(Orientation::Vertical, look.clone());
            sp2.SetPos(0.8);
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
            ctx.tree.SetEnableSwitch(t2b_id, false);
        }

        // Position sp in border content rect
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        if let Some(sp) = ctx.find_child_by_name("sp") {
            ctx.layout_child(sp, cr.x, cr.y, cr.w, cr.h);
        }
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ═══════════════════════════════════════════════════════════════════
// TkTestPanel — widget showcase grid
// ═══════════════════════════════════════════════════════════════════

struct TkTestPanel {
    look: Rc<emLook>,
    border: emBorder,
    children_created: bool,
}

impl TkTestPanel {
    fn new(look: Rc<emLook>) -> Self {
        let border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Toolkit Test");
        Self {
            look,
            border,
            children_created: false,
        }
    }

    fn new_with_caption(look: Rc<emLook>, caption: &str) -> Self {
        let border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption(caption);
        Self {
            look,
            border,
            children_created: false,
        }
    }

    /// Helper: create a emRasterGroup category under `parent_context`.
    fn make_category(
        tree: &mut PanelTree,
        parent_context: PanelId,
        name: &str,
        caption: &str,
        pct: Option<f64>,
        fixed_cols: Option<usize>,
    ) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(2.5);
        rg.border.caption = caption.to_string();
        if let Some(p) = pct {
            rg.layout.preferred_child_tallness = p;
        }
        if let Some(c) = fixed_cols {
            rg.layout.fixed_columns = Some(c);
        }
        let id = tree.create_child(parent_context, name);
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
                    widget: emButton::new("Button", look.clone()),
                }),
            );

            let mut b2 = emButton::new("Long Desc", look.clone());
            {
                let mut desc = String::new();
                for _ in 0..100 {
                    desc.push_str("This is a looooooooooooooooooooooooooooooooooooooooooooooooooooooong description of the button.\n");
                }
                b2.SetDescription(&desc);
            }
            let id = ctx.tree.create_child(gid, "b2");
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b2 }));

            let mut b3 = emButton::new("NoEOI", look.clone());
            b3.SetNoEOI(true);
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
                        widget: emCheckButton::new("Check Button", look.clone()),
                    }),
                );
            }
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(CheckBoxPanel {
                        widget: emCheckBox::new("Check Box", look.clone()),
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
                        widget: emRadioButton::new("Radio Button", look.clone(), rg.clone(), i - 1),
                    }),
                );
            }
            let rg2 = RadioGroup::new();
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("r{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(RadioBoxPanel {
                        widget: emRadioBox::new("Radio Box", look.clone(), rg2.clone(), i - 4),
                    }),
                );
            }
        }

        // 4. Text Fields (C++ :626-656)
        let gid = Self::make_category(ctx.tree, grid_id, "textfields", "Text Fields", None, None);
        {
            let mut tf1 = emTextField::new(look.clone());
            tf1.SetCaption("Read-Only");
            tf1.SetText("Read-Only");
            let id = ctx.tree.create_child(gid, "tf1");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf1 }));

            let mut tf2 = emTextField::new(look.clone());
            tf2.SetCaption("Editable");
            tf2.SetEditable(true);
            tf2.SetText("Editable");
            let id = ctx.tree.create_child(gid, "tf2");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf2 }));

            let mut tf3 = emTextField::new(look.clone());
            tf3.SetCaption("Password");
            tf3.SetEditable(true);
            tf3.SetText("Password");
            tf3.SetPasswordMode(true);
            let id = ctx.tree.create_child(gid, "tf3");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf3 }));

            let mut mltf1 = emTextField::new(look.clone());
            mltf1.SetCaption("Multi-Line");
            mltf1.SetEditable(true);
            mltf1.SetMultiLineMode(true);
            mltf1.SetText("first line\nsecond line\n...");
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
            let mut sf1 = emScalarField::new(0.0, 100.0, look.clone());
            sf1.SetCaption("Read-Only");
            let id = ctx.tree.create_child(gid, "sf1");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf1 }));

            let mut sf2 = emScalarField::new(0.0, 100.0, look.clone());
            sf2.SetCaption("Editable");
            sf2.SetEditable(true);
            let id = ctx.tree.create_child(gid, "sf2");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf2 }));

            let mut sf3 = emScalarField::new(-1000.0, 1000.0, look.clone());
            sf3.SetEditable(true);
            sf3.SetScaleMarkIntervals(&[1000, 100, 10, 5, 1]);
            let id = ctx.tree.create_child(gid, "sf3");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf3 }));

            // sf4: Level 1-5, val=3, custom format, GetTextBoxTallness=0.25
            let mut sf4 = emScalarField::new(1.0, 5.0, look.clone());
            sf4.SetCaption("Level");
            sf4.SetEditable(true);
            sf4.SetValue(3.0);
            sf4.SetTextBoxTallness(0.25);
            sf4.SetTextOfValueFunc(Box::new(|val, _interval| format!("Level {val}")));
            let id = ctx.tree.create_child(gid, "sf4");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf4 }));

            // sf5: PlayLength, time format
            let mut sf5 = emScalarField::new(0.0, 86400000.0, look.clone());
            sf5.SetCaption("Play Length");
            sf5.SetEditable(true);
            sf5.SetValue(14400000.0);
            // C++ emTestPanel.cpp:636
            sf5.SetScaleMarkIntervals(&[3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1]);
            sf5.SetTextOfValueFunc(Box::new(|val, mark_interval| {
                let v = val.unsigned_abs();
                let h = (v / 3600000) as i64;
                let m = ((v / 60000) % 60) as i64;
                let s = ((v / 1000) % 60) as i64;
                let ms = (v % 1000) as i64;
                if mark_interval < 10 {
                    format!("{h:02}:{m:02}:{s:02}\n.{ms:03}")
                } else if mark_interval < 100 {
                    format!("{h:02}:{m:02}:{s:02}\n.{:02}", ms / 10)
                } else if mark_interval < 1000 {
                    format!("{h:02}:{m:02}:{s:02}\n.{:01}", ms / 100)
                } else if mark_interval < 60000 {
                    format!("{h:02}:{m:02}:{s:02}")
                } else {
                    format!("{h:02}:{m:02}")
                }
            }));
            let id = ctx.tree.create_child(gid, "sf5");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf5 }));

            // sf6: PlayPos, same time format, max=sf5.GetValue
            let mut sf6 = emScalarField::new(0.0, 14400000.0, look.clone());
            sf6.SetCaption("Play Position");
            sf6.SetEditable(true);
            // C++ emTestPanel.cpp:643
            sf6.SetScaleMarkIntervals(&[3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1]);
            sf6.SetTextOfValueFunc(Box::new(|val, mark_interval| {
                let v = val.unsigned_abs();
                let h = (v / 3600000) as i64;
                let m = ((v / 60000) % 60) as i64;
                let s = ((v / 1000) % 60) as i64;
                let ms = (v % 1000) as i64;
                if mark_interval < 10 {
                    format!("{h:02}:{m:02}:{s:02}\n.{ms:03}")
                } else if mark_interval < 100 {
                    format!("{h:02}:{m:02}:{s:02}\n.{:02}", ms / 10)
                } else if mark_interval < 1000 {
                    format!("{h:02}:{m:02}:{s:02}\n.{:01}", ms / 100)
                } else if mark_interval < 60000 {
                    format!("{h:02}:{m:02}:{s:02}")
                } else {
                    format!("{h:02}:{m:02}")
                }
            }));
            let id = ctx.tree.create_child(gid, "sf6");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf6 }));
        }

        // 6. emColor Fields (C++ :714-733)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "colorfields",
            "Color Fields",
            Some(0.4),
            None,
        );
        {
            let mut cf1 = emColorField::new(look.clone());
            cf1.SetCaption("Read-Only");
            cf1.SetColor(emColor::rgba(0xBB, 0x22, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf1");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf1 }));
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);

            let mut cf2 = emColorField::new(look.clone());
            cf2.SetCaption("Editable");
            cf2.SetEditable(true);
            cf2.SetColor(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf2");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf2 }));
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);

            let mut cf3 = emColorField::new(look.clone());
            cf3.SetCaption("Editable, Alpha Enabled");
            cf3.SetEditable(true);
            cf3.SetAlphaEnabled(true);
            cf3.SetColor(emColor::rgba(0x22, 0x22, 0xBB, 0xFF));
            let id = ctx.tree.create_child(gid, "cf3");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf3 }));
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);
        }

        // 7. Tunnels (C++ emTestPanel.cpp:662-680)
        let gid = Self::make_category(ctx.tree, grid_id, "tunnels", "Tunnels", Some(0.4), None);
        {
            let tid = ctx.tree.create_child(gid, "t1");
            let t1 = emTunnel::new(look.clone()).with_caption("Tunnel");
            ctx.tree.set_behavior(tid, Box::new(t1));
            let child = ctx.tree.create_child(tid, "e");
            ctx.tree.set_behavior(
                child,
                Box::new(ButtonPanel {
                    widget: emButton::new("End Of Tunnel", look.clone()),
                }),
            );

            let tid = ctx.tree.create_child(gid, "t2");
            let mut t2 = emTunnel::new(look.clone()).with_caption("Deeper Tunnel");
            t2.SetDepth(30.0);
            ctx.tree.set_behavior(tid, Box::new(t2));
            let child = ctx.tree.create_child(tid, "e");
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                ctx.tree.set_behavior(child, Box::new(rg));
            }

            let tid = ctx.tree.create_child(gid, "t3");
            let mut t3 = emTunnel::new(look.clone()).with_caption("Square End");
            t3.SetChildTallness(1.0);
            ctx.tree.set_behavior(tid, Box::new(t3));
            let child = ctx.tree.create_child(tid, "e");
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                ctx.tree.set_behavior(child, Box::new(rg));
            }

            let tid = ctx.tree.create_child(gid, "t4");
            let mut t4 = emTunnel::new(look.clone()).with_caption("Square End, Zero Depth");
            t4.SetChildTallness(1.0);
            t4.SetDepth(0.0);
            ctx.tree.set_behavior(tid, Box::new(t4));
            let child = ctx.tree.create_child(tid, "e");
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                ctx.tree.set_behavior(child, Box::new(rg));
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
            // Helper: add items with numeric names matching C++
            // AddItem(Format("%d",i), Format("Item %d",i))
            fn add_items_1_to_7(lb: &mut emListBox) {
                for i in 1..=7 {
                    lb.AddItem(format!("{i}"), format!("Item {i}"));
                }
            }

            let mut lb1 = emListBox::new(look.clone());
            lb1.SetCaption("Empty");
            let id = ctx.tree.create_child(gid, "l1");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb1 }));

            let mut lb2 = emListBox::new(look.clone());
            lb2.SetCaption("Single-Selection");
            lb2.SetSelectionType(SelectionMode::Single);
            add_items_1_to_7(&mut lb2);
            lb2.SetSelectedIndex(0);
            let id = ctx.tree.create_child(gid, "l2");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb2 }));

            let mut lb3 = emListBox::new(look.clone());
            lb3.SetCaption("Read-Only");
            lb3.SetSelectionType(SelectionMode::ReadOnly);
            add_items_1_to_7(&mut lb3);
            lb3.SetSelectedIndex(2);
            let id = ctx.tree.create_child(gid, "l3");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb3 }));

            let mut lb4 = emListBox::new(look.clone());
            lb4.SetCaption("Multi-Selection");
            lb4.SetSelectionType(SelectionMode::Multi);
            add_items_1_to_7(&mut lb4);
            lb4.Select(1, false);
            lb4.Select(2, false);
            lb4.Select(3, false);
            lb4.Select(4, false);
            let id = ctx.tree.create_child(gid, "l4");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb4 }));

            let mut lb5 = emListBox::new(look.clone());
            lb5.SetCaption("Toggle-Selection");
            lb5.SetSelectionType(SelectionMode::Toggle);
            add_items_1_to_7(&mut lb5);
            lb5.Select(2, false);
            lb5.Select(4, false);
            let id = ctx.tree.create_child(gid, "l5");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb5 }));

            // l6: single column
            let mut lb6 = emListBox::new(look.clone());
            lb6.SetCaption("Single Column");
            lb6.SetSelectionType(SelectionMode::Single);
            add_items_1_to_7(&mut lb6);
            lb6.set_fixed_column_count(Some(1));
            lb6.SetSelectedIndex(0);
            let id = ctx.tree.create_child(gid, "l6");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb6 }));

            // l7: custom list box — C++ CustomListBox with CustomItemPanel items
            let mut lb7 = emListBox::new(look.clone());
            lb7.SetCaption("Custom List Box");
            lb7.SetSelectionType(SelectionMode::Multi);
            add_items_1_to_7(&mut lb7);
            lb7.SetSelectedIndex(0);
            lb7.set_item_behavior_factory(
                move |_i, text, selected, look, _sel_mode, _enabled| {
                    Box::new(CustomItemPanelBehavior::new(
                        text.to_string(),
                        selected,
                        look,
                    ))
                },
            );
            let id = ctx.tree.create_child(gid, "l7");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb7 }));
        }

        // 9. Test emDialog (C++ :800-831)
        let gid = Self::make_category(ctx.tree, grid_id, "dlgs", "Test Dialog", None, Some(1));
        {
            // emRasterLayout with checkboxes
            let mut rl = emRasterLayout::new();
            rl.preferred_child_tallness = 0.1;
            let rl_id = ctx.tree.create_child(gid, "rl");

            // C++ emTestPanel.cpp:738-747
            let cb_items: &[(&str, &str, bool)] = &[
                ("tl", "Top-Level", false),
                ("VF_POPUP_ZOOM", "VF_POPUP_ZOOM", true),
                ("WF_MODAL", "WF_MODAL", true),
                ("WF_UNDECORATED", "WF_UNDECORATED", false),
                ("WF_POPUP", "WF_POPUP", false),
                ("WF_MAXIMIZED", "WF_MAXIMIZED", false),
                ("WF_FULLSCREEN", "WF_FULLSCREEN", false),
            ];
            for &(name, caption, checked) in cb_items {
                let id = ctx.tree.create_child(rl_id, name);
                let mut cb = emCheckBox::new(caption, look.clone());
                if checked {
                    cb.SetChecked(true);
                }
                ctx.tree
                    .set_behavior(id, Box::new(CheckBoxPanel { widget: cb }));
            }
            ctx.tree.set_behavior(rl_id, Box::new(rl));

            let id = ctx.tree.create_child(gid, "bt");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Create Test Dialog", look.clone()),
                }),
            );
        }

        // 10. File Selection (C++ :750-764)
        let gid = Self::make_category(
            ctx.tree,
            grid_id,
            "fileChoosers",
            "File Selection",
            Some(0.3),
            None,
        );
        {
            let id = ctx.tree.create_child(gid, "l8");
            let mut fsb = emFileSelectionBox::new("File Selection Box");
            fsb.set_filters(&[
                "All Files (*)".to_string(),
                "Image Files (*.bmp *.gif *.jpg *.png *.tga)".to_string(),
                "HTML Files (*.htm *.html)".to_string(),
            ]);
            // C++ gen_golden runs with CWD=crates/eaglemode/ — match that.
            fsb.set_parent_directory(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
            ctx.tree.set_behavior(id, Box::new(fsb));

            let id = ctx.tree.create_child(gid, "openFile");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Open...", look.clone()),
                }),
            );

            let id = ctx.tree.create_child(gid, "openFiles");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Open Multi, Allow Dir...", look.clone()),
                }),
            );

            let id = ctx.tree.create_child(gid, "saveFile");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Save As...", look.clone()),
                }),
            );
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

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled, 1.0);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;

            // Create grid child with emRasterLayout (PCT=0.3)
            let mut layout = emRasterLayout::new();
            layout.preferred_child_tallness = 0.3;
            let grid_id = ctx.create_child_with("grid", Box::new(layout));

            // Create all 10 category groups under the grid
            self.create_all_categories(ctx, grid_id);
        }

        // Position grid in border content rect
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        if let Some(grid) = ctx.find_child_by_name("grid") {
            ctx.layout_child(grid, cr.x, cr.y, cr.w, cr.h);
        }
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ═══════════════════════════════════════════════════════════════════
// PolyDrawPanel — polygon drawing with star shape
// ═══════════════════════════════════════════════════════════════════

struct PolyDrawPanel {
    border: emBorder,
    look: Rc<emLook>,
    children_created: bool,
}

impl PolyDrawPanel {
    fn new() -> Self {
        let look = emLook::new();
        let border = emBorder::new(OuterBorderType::Group)
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

    /// Create the 16-method emRadioBox group under a parent.
    fn create_method_radio(
        tree: &mut PanelTree,
        parent_context: PanelId,
        look: &Rc<emLook>,
    ) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(1.5);
        rg.border.caption = "Method".to_string();
        rg.layout.preferred_child_tallness = 0.07;
        let mid = tree.create_child(parent_context, "Method");

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
                    widget: emRadioBox::new(name, look.clone(), method_group.clone(), i),
                }),
            );
        }
        tree.set_behavior(mid, Box::new(rg));
        mid
    }

    /// Create a 4-option dash type emRadioBox group.
    fn create_dash_radio(
        tree: &mut PanelTree,
        parent_context: PanelId,
        look: &Rc<emLook>,
    ) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(1.5);
        rg.border.caption = "Dash Type".to_string();
        rg.layout.preferred_child_tallness = 0.08;
        let did = tree.create_child(parent_context, "StrokeDashType");

        let dash_group = RadioGroup::new();
        let names = ["Solid", "Dashed", "Dotted", "DashDotted"];
        for (i, name) in names.iter().enumerate() {
            let id = tree.create_child(did, name);
            tree.set_behavior(
                id,
                Box::new(RadioBoxPanel {
                    widget: emRadioBox::new(name, look.clone(), dash_group.clone(), i),
                }),
            );
        }
        tree.set_behavior(did, Box::new(rg));
        did
    }

    /// Create a 17-option stroke end type emRadioBox group.
    fn create_stroke_end_radio(
        tree: &mut PanelTree,
        parent_context: PanelId,
        name: &str,
        caption: &str,
        look: &Rc<emLook>,
    ) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(1.5);
        rg.border.caption = caption.to_string();
        rg.layout.preferred_child_tallness = 0.08;
        let sid = tree.create_child(parent_context, name);

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
                    widget: emRadioBox::new(n, look.clone(), group.clone(), i),
                }),
            );
        }
        tree.set_behavior(sid, Box::new(rg));
        sid
    }

    /// Create a horizontal emLinearLayout with 2 children (emTextField + widget).
    fn create_horizontal_pair(
        tree: &mut PanelTree,
        parent_context: PanelId,
        name: &str,
        child1_name: &str,
        child1: Box<dyn PanelBehavior>,
        child2_name: &str,
        child2: Box<dyn PanelBehavior>,
    ) -> PanelId {
        let ll_id = tree.create_child(parent_context, name);
        let c1 = tree.create_child(ll_id, child1_name);
        tree.set_behavior(c1, child1);
        let c2 = tree.create_child(ll_id, child2_name);
        tree.set_behavior(c2, child2);
        tree.set_behavior(ll_id, Box::new(emLinearLayout::horizontal()));
        ll_id
    }

    fn create_controls(&self, ctx: &mut PanelCtx, layout_id: PanelId) {
        let look = self.look.clone();

        // Controls: emRasterLayout with PCT=0.6
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
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("9");
                    tf
                },
            }),
            "FillColor",
            Box::new(ColorFieldPanel {
                widget: {
                    let mut cf = emColorField::new(look.clone());
                    cf.SetEditable(true);
                    cf.SetAlphaEnabled(true);
                    cf.SetColor(emColor::WHITE);
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
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("0.01");
                    tf
                },
            }),
            "WithCanvasColor",
            Box::new(CheckBoxPanel {
                widget: emCheckBox::new("With Canvas Color", look.clone()),
            }),
        );

        // Set general behavior with weight on Method
        let mut gen_group = emLinearGroup::vertical();
        gen_group.border.SetBorderScaling(2.0);
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
                    let mut cf = emColorField::new(look.clone());
                    cf.SetEditable(true);
                    cf.SetAlphaEnabled(true);
                    cf.SetColor(emColor::rgba(0, 0, 0, 0xFF));
                    cf
                },
            }),
        );

        let rounded_id = ctx.tree.create_child(stroke_id, "StrokeRounded");
        ctx.tree.set_behavior(
            rounded_id,
            Box::new(CheckBoxPanel {
                widget: emCheckBox::new("Rounded", look.clone()),
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
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("1.0");
                    tf
                },
            }),
            "GapLengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("1.0");
                    tf
                },
            }),
        );

        let mut stroke_group = emLinearGroup::vertical();
        stroke_group.border.SetBorderScaling(2.0);
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
                    let mut cf = emColorField::new(look.clone());
                    cf.SetEditable(true);
                    cf.SetAlphaEnabled(true);
                    cf.SetColor(emColor::rgba(0xEE, 0xEE, 0xEE, 0xFF));
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
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("1.0");
                    tf
                },
            }),
            "LengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("1.0");
                    tf
                },
            }),
        );

        let mut ss_group = emLinearGroup::vertical();
        ss_group.border.SetBorderScaling(2.0);
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
                    let mut cf = emColorField::new(look.clone());
                    cf.SetEditable(true);
                    cf.SetAlphaEnabled(true);
                    cf.SetColor(emColor::rgba(0xEE, 0xEE, 0xEE, 0xFF));
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
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("1.0");
                    tf
                },
            }),
            "LengthFactor",
            Box::new(TextFieldPanel {
                widget: {
                    let mut tf = emTextField::new(look.clone());
                    tf.SetEditable(true);
                    tf.SetText("1.0");
                    tf
                },
            }),
        );

        let mut se_group = emLinearGroup::vertical();
        se_group.border.SetBorderScaling(2.0);
        se_group.border.caption = "Stroke End".to_string();
        se_group.layout.set_child_constraint(
            se_type_id,
            ChildConstraint {
                weight: 2.0,
                ..Default::default()
            },
        );
        ctx.tree.set_behavior(se_id, Box::new(se_group));

        // Set Controls behavior (emRasterLayout, PCT=0.6)
        let mut ctrl_layout = emRasterLayout::new();
        ctrl_layout.preferred_child_tallness = 0.6;
        ctx.tree.set_behavior(ctrl_id, Box::new(ctrl_layout));

        // ── CanvasPanel ──
        let canvas_id = ctx.tree.create_child(layout_id, "CanvasPanel");
        ctx.tree
            .set_behavior(canvas_id, Box::new(CanvasPanel::new()));
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled, 1.0);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;

            // emLinearLayout child (adaptive, threshold=1.0)
            let layout_id = ctx.create_child("layout");
            self.create_controls(ctx, layout_id);

            // Set behavior last — the adaptive emLinearLayout
            ctx.tree
                .set_behavior(layout_id, Box::new(emLinearLayout::adaptive(1.0)));
        }

        // Position layout in border content rect
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        if let Some(layout) = ctx.find_child_by_name("layout") {
            ctx.layout_child(layout, cr.x, cr.y, cr.w, cr.h);
        }
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
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
    view: &mut emView,
    expected: &(u32, u32, Vec<u8>),
    channel_tolerance: u8,
    max_failure_pct: f64,
    settle_rounds: usize,
) {
    let (w, h, ref expected_data) = *expected;

    settle(tree, view, settle_rounds);

    // Record DrawOps for parameter diff diagnosis when DUMP_DRAW_OPS=1.
    if dump_draw_ops_enabled() {
        let mut ops: Vec<RecordedOp> = Vec::new();
        {
            let mut rec = emPainter::new_recording(w, h, &mut ops);
            rec.set_record_subops(true);
            view.Paint(tree, &mut rec, emColor::TRANSPARENT);
        }
        dump_draw_ops(name, &ops);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(tree, view);
    let actual = compositor.framebuffer().GetMap();

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

/// Root panel PaintContent only — no auto-expansion, tests PaintContent primitives.
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
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
    // Very high threshold prevents auto-expansion (Match C++ gen)
    tree.SetAutoExpansionThreshold(root, 1e9, ViewConditionType::Area);

    let mut view = emView::new(root, 1000.0, 1000.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window — match unfocused state
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 30)
    render_testpanel(
        "testpanel_root",
        &mut tree,
        &mut view,
        &expected,
        0,
        0.0,
        30,
    );
}

/// Full TestPanel tree with auto-expanded children — integration test.
/// Remaining diffs from text GetValue differences (Pri/MemLim runtime values),
/// border positioning, and font rendering (~12%).
#[test]
fn testpanel_expanded() {
    require_golden!();
    let expected = load_compositor_golden("testpanel_expanded");

    let bg_color = Rc::new(Cell::new(DEFAULT_BG));
    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.set_behavior(root, Box::new(TestPanel::new(0, bg_color)));
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
    // C++ default threshold: 900 (VCT_AREA). At 1000x1000, vc=1e6 > 900 → expands.
    tree.SetAutoExpansionThreshold(root, 900.0, ViewConditionType::Area);

    let mut view = emView::new(root, 1000.0, 1000.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window — match unfocused state
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    render_testpanel(
        "testpanel_expanded",
        &mut tree,
        &mut view,
        &expected,
        0,
        0.0,
        200,
    );
}
