use std::rc::Rc;

use emcore::emLinearGroup::emLinearGroup;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emTiling::Orientation;

use emcore::emPanelCtx::PanelCtx;

use emcore::emPanelTree::{PanelTree, ViewConditionType};

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emPainter::emPainter;
use emcore::emView::{emView, ViewFlags};
use emcore::emViewRenderer::SoftwareCompositor;

use emcore::emButton::emButton;

use emcore::emCheckBox::emCheckBox;

use emcore::emColorField::emColorField;

use emcore::emErrorPanel::emErrorPanel;

use emcore::emFilePanel::emFilePanel;

use emcore::emFileSelectionBox::emFileSelectionBox;

use emcore::emLabel::emLabel;

use emcore::emListBox::emListBox;

use emcore::emLook::emLook;

use emcore::emRadioButton::{emRadioButton, RadioGroup};

use emcore::emScalarField::emScalarField;

use emcore::emSplitter::emSplitter;

use emcore::emTextField::emTextField;

use emcore::emTunnel::emTunnel;

use super::common::*;
use super::draw_op_dump::{dump_draw_ops_enabled, install_direct_op_logger};

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
fn settle(tree: &mut PanelTree, view: &mut emView) {
    for _ in 0..5 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(tree);
    }
}

/// Install direct-mode op logger on compositor before rendering.
fn render_with_ops(
    compositor: &mut SoftwareCompositor,
    tree: &mut PanelTree,
    view: &emView,
    name: &str,
) {
    let dump = dump_draw_ops_enabled();
    compositor.render_with_setup(tree, view, |painter| {
        if dump {
            install_direct_op_logger(painter, name);
        }
    });
}

// ─── PanelBehavior wrappers for widgets ──────────────────────────

/// Wraps a emBorder (with specific outer/inner type) as a PanelBehavior.
struct BorderBehavior {
    border: emBorder,
    look: Rc<emLook>,
}

impl BorderBehavior {
    fn new(
        outer: OuterBorderType,
        inner: InnerBorderType,
        caption: &str,
        look: Rc<emLook>,
    ) -> Self {
        let mut border = emBorder::new(outer).with_inner(inner).with_caption(caption);
        border.label_in_border = true;
        Self { border, look }
    }

    fn with_description(mut self, desc: &str) -> Self {
        self.border = self.border.with_description(desc);
        self
    }
}

impl PanelBehavior for BorderBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
    }
}

/// Wraps a emLabel widget as a PanelBehavior.
struct LabelBehavior {
    label: emLabel,
}

impl PanelBehavior for LabelBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.label
            .PaintContent(painter, w, h, state.enabled, pixel_scale);
    }
}

/// Wraps a emButton widget as a PanelBehavior.
struct ButtonBehavior {
    button: emButton,
}

impl PanelBehavior for ButtonBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button.Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

/// Wraps a emCheckBox widget as a PanelBehavior.
struct CheckBoxBehavior {
    check_box: emCheckBox,
}

impl PanelBehavior for CheckBoxBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_box
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

/// Wraps a emTextField widget as a PanelBehavior.
struct TextFieldBehavior {
    text_field: emTextField,
}

impl PanelBehavior for TextFieldBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.text_field
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

/// Wraps a emScalarField widget as a PanelBehavior.
struct ScalarFieldBehavior {
    scalar_field: emScalarField,
}

impl PanelBehavior for ScalarFieldBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.scalar_field
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

/// Helper: render a single widget filling the entire 800x600 viewport and
/// compare against a golden file.
fn render_and_compare(name: &str, behavior: Box<dyn PanelBehavior>) {
    render_and_compare_tol(name, behavior, 0, 0.0);
}

fn render_and_compare_tol(
    name: &str,
    behavior: Box<dyn PanelBehavior>,
    channel_tolerance: u8,
    max_failure_pct: f64,
) {
    let (w, h, expected) = load_compositor_golden(name);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, behavior);

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, name);
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        name,
        actual,
        &expected,
        w,
        h,
        channel_tolerance,
        max_failure_pct,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images(name, actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, channel_tolerance);
    }
    result.unwrap();
}

// ─── Test 1: widget_border_rect ─────────────────────────────────

#[test]
fn widget_border_rect() {
    require_golden!();
    let look = emLook::new();
    // Residual from 9-slice section boundary rounding (~1.5%)
    render_and_compare_tol(
        "widget_border_rect",
        Box::new(BorderBehavior::new(
            OuterBorderType::Rect,
            InnerBorderType::None,
            "Test",
            look,
        )),
        0,
        0.0,
    );
}

// ─── Test 2: widget_border_round_rect ───────────────────────────

#[test]
fn widget_border_round_rect() {
    require_golden!();
    let look = emLook::new();
    // Residual from 9-slice section boundary rounding (~2.1%)
    render_and_compare_tol(
        "widget_border_round_rect",
        Box::new(
            BorderBehavior::new(
                OuterBorderType::RoundRect,
                InnerBorderType::None,
                "Caption",
                look,
            )
            .with_description("Description text"),
        ),
        0,
        0.0,
    );
}

// ─── Test 3: widget_border_group ────────────────────────────────

#[test]
fn widget_border_group() {
    require_golden!();
    let look = emLook::new();
    // Residual from 9-slice section boundary rounding (~3.6%)
    render_and_compare_tol(
        "widget_border_group",
        Box::new(BorderBehavior::new(
            OuterBorderType::Group,
            InnerBorderType::Group,
            "Group",
            look,
        )),
        0,
        0.0,
    );
}

// ─── Test 4: widget_border_instrument ───────────────────────────

#[test]
fn widget_border_instrument() {
    require_golden!();
    let look = emLook::new();
    // Residual from 9-slice section boundary rounding (~2.6%)
    render_and_compare_tol(
        "widget_border_instrument",
        Box::new(BorderBehavior::new(
            OuterBorderType::Instrument,
            InnerBorderType::None,
            "Instrument",
            look,
        )),
        0,
        0.0,
    );
}

// ─── Test 5: widget_label ───────────────────────────────────────

#[test]
fn widget_label() {
    require_golden!();
    let look = emLook::new();
    render_and_compare(
        "widget_label",
        Box::new(LabelBehavior {
            label: emLabel::new("Hello World", look),
        }),
    );
}

// ─── Test 6: widget_button_normal ───────────────────────────────

#[test]
fn widget_button_normal() {
    require_golden!();
    let look = emLook::new();
    // Residual diffs from text rendering and 9-slice boundary rounding (~0.9%)
    render_and_compare_tol(
        "widget_button_normal",
        Box::new(ButtonBehavior {
            button: emButton::new("Click Me", look),
        }),
        0,
        0.0,
    );
}

// ─── Test 7: widget_checkbox_unchecked ──────────────────────────

#[test]
fn widget_checkbox_unchecked() {
    require_golden!();
    let look = emLook::new();
    // Residual from checkbox GetImage 9-slice section boundary rounding (~4.8%)
    render_and_compare_tol(
        "widget_checkbox_unchecked",
        Box::new(CheckBoxBehavior {
            check_box: emCheckBox::new("Check Option", look),
        }),
        0,
        0.0,
    );
}

// ─── Test 8: widget_checkbox_checked ────────────────────────────

#[test]
fn widget_checkbox_checked() {
    require_golden!();
    let look = emLook::new();
    let mut cb = emCheckBox::new("Check Option", look);
    cb.SetChecked(true);
    // Residual from checkbox GetImage + text rendering diffs (~5.1%)
    render_and_compare_tol(
        "widget_checkbox_checked",
        Box::new(CheckBoxBehavior { check_box: cb }),
        0,
        0.0,
    );
}

// ─── Test 9: widget_textfield_empty ─────────────────────────────

#[test]
fn widget_textfield_empty() {
    require_golden!();
    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetCaption("Name");
    tf.SetEditable(true);
    render_and_compare_tol(
        "widget_textfield_empty",
        Box::new(TextFieldBehavior { text_field: tf }),
        0,
        0.0,
    );
}

// ─── Test 10: widget_textfield_content ──────────────────────────

#[test]
fn widget_textfield_content() {
    require_golden!();
    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetCaption("Name");
    tf.SetEditable(true);
    tf.SetText("Hello");
    // Residual from 9-slice border interpolation + text rendering diffs
    render_and_compare_tol(
        "widget_textfield_content",
        Box::new(TextFieldBehavior { text_field: tf }),
        0,
        0.0,
    );
}

// ─── Test 11: widget_scalarfield ────────────────────────────────

#[test]
fn widget_scalarfield() {
    require_golden!();
    let look = emLook::new();
    let mut sf = emScalarField::new(0.0, 100.0, look);
    sf.SetCaption("Value");
    sf.SetEditable(true);
    sf.SetValue(50.0);
    // Residual from 9-slice border interpolation + text rendering diffs (~4.7%)
    render_and_compare_tol(
        "widget_scalarfield",
        Box::new(ScalarFieldBehavior { scalar_field: sf }),
        0,
        0.0,
    );
}

// ─── Additional behavior wrappers ──────────────────────────────

/// Wraps a emRadioButton widget as a PanelBehavior.
struct RadioButtonBehavior {
    radio_button: emRadioButton,
}

impl PanelBehavior for RadioButtonBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.radio_button
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }
}

/// Wraps a emListBox widget as a PanelBehavior.
struct ListBoxBehavior {
    list_box: emListBox,
}

impl PanelBehavior for ListBoxBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.list_box.Paint(painter, w, h, pixel_scale);
    }
}

/// Wraps a emSplitter widget as a PanelBehavior.
struct SplitterBehavior {
    splitter: emSplitter,
}

impl PanelBehavior for SplitterBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        self.splitter.PaintContent(painter, w, h, _state.enabled);
    }
}

// ─── Test 12: widget_colorfield ────────────────────────────────

/// C++ emColorField constructor calls SetAutoExpansionThreshold(9, VCT_MIN_EXT).
/// At 800×600 with layout 1.0×0.75, min_ext=600 >> 9 > 1, triggering expansion.
/// The golden includes child ScalarFields (RGB/HSV) on the right half.
#[test]
fn widget_colorfield() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_colorfield");

    let look = emLook::new();
    let mut cf = emColorField::new(look);
    cf.SetCaption("Color");
    cf.SetColor(emcore::emColor::emColor::rgba(255, 0, 0, 255));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    // C++ emColorField.cpp:36 — SetAutoExpansionThreshold(9, VCT_MIN_EXT)
    tree.SetAutoExpansionThreshold(root, 9.0, ViewConditionType::MinExt);
    tree.set_behavior(
        root,
        Box::new(ColorFieldExpandedBehavior { color_field: cf }),
    );

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 30)
    for _ in 0..30 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_colorfield");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_colorfield", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_colorfield", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── Test 13: widget_radiobutton ───────────────────────────────

#[test]
fn widget_radiobutton() {
    require_golden!();
    let look = emLook::new();
    let group = RadioGroup::new();
    let mut rb = emRadioButton::new("Radio Option", look, group, 0);
    rb.set_checked(true);
    // Residual diffs from text rendering and 9-slice boundary rounding (~0.8%)
    render_and_compare_tol(
        "widget_radiobutton",
        Box::new(RadioButtonBehavior { radio_button: rb }),
        0,
        0.0,
    );
}

// ─── Test 14: widget_listbox ───────────────────────────────────

#[test]
fn widget_listbox() {
    require_golden!();
    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetCaption("Items");
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());
    lb.AddItem("item3".to_string(), "Delta".to_string());
    lb.AddItem("item4".to_string(), "Epsilon".to_string());
    lb.SetSelectedIndex(2);
    // Residual from 9-slice boundary + text rendering + arch diff (~8.8%)
    render_and_compare_tol(
        "widget_listbox",
        Box::new(ListBoxBehavior { list_box: lb }),
        0,
        0.0,
    );
}

// ─── Test 15: widget_splitter_h ────────────────────────────────

#[test]
fn widget_splitter_h() {
    require_golden!();
    let look = emLook::new();
    let sp = emSplitter::new(Orientation::Horizontal, look);
    // Residual from 9-slice interpolation rounding (~0.9%)
    render_and_compare_tol(
        "widget_splitter_h",
        Box::new(SplitterBehavior { splitter: sp }),
        0,
        0.0,
    );
}

// ─── Test 16: widget_splitter_v ────────────────────────────────

#[test]
fn widget_splitter_v() {
    require_golden!();
    let look = emLook::new();
    let mut sp = emSplitter::new(Orientation::Vertical, look);
    sp.SetPos(0.3);
    // Residual from 9-slice interpolation rounding + grip GetPos (~1.7%)
    render_and_compare_tol(
        "widget_splitter_v",
        Box::new(SplitterBehavior { splitter: sp }),
        0,
        0.0,
    );
}

// ─── Test 17: colorfield_expanded ─────────────────────────────

/// Wraps a emColorField as a PanelBehavior with LayoutChildren delegation
/// for auto-expanded child panels.
struct ColorFieldExpandedBehavior {
    color_field: emColorField,
}

impl PanelBehavior for ColorFieldExpandedBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.color_field.Paint(painter, w, h, pixel_scale);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.color_field.create_expansion_children(ctx);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.color_field.LayoutChildren(ctx, rect.w, rect.h);
    }
}

/// Expanded emColorField with child ScalarFields for RGBA/HSV editing.
/// C++ renders emRasterLayout with 8 ScalarFields + emTextField on right half.
#[test]
fn colorfield_expanded() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("colorfield_expanded");

    let look = emLook::new();
    let mut cf = emColorField::new(look);
    cf.SetCaption("Color");
    cf.SetDescription("Test color field");
    cf.SetEditable(true);
    cf.SetAlphaEnabled(true);
    cf.SetColor(emcore::emColor::emColor::rgba(0xBB, 0x22, 0x22, 0xFF));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
    // C++ emColorField uses AE threshold 9 (VCT_MIN_EXT)
    tree.SetAutoExpansionThreshold(root, 9.0, ViewConditionType::MinExt);
    tree.set_behavior(
        root,
        Box::new(ColorFieldExpandedBehavior { color_field: cf }),
    );

    let mut view = emView::new(root, 800.0, 800.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "colorfield_expanded");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("colorfield_expanded", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("colorfield_expanded", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── Test 18: listbox_expanded ────────────────────────────────

/// Wraps a emListBox as a PanelBehavior for expanded rendering.
struct ListBoxExpandedBehavior {
    list_box: emListBox,
}

impl PanelBehavior for ListBoxExpandedBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.list_box.Paint(painter, w, h, pixel_scale);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.list_box.create_item_children(ctx);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.list_box.layout_item_children(ctx, rect.w, rect.h);
    }
}

/// Expanded emListBox with 7 items, 3 multi-GetChecked.
/// C++ renders child DefaultItemPanel panels laid out by emRasterGroup grid.
#[test]
fn listbox_expanded() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("listbox_expanded");

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetCaption("Items");
    lb.SetSelectionType(emcore::emListBox::SelectionMode::Multi);
    lb.set_items(
        ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    lb.Select(1, false);
    lb.Select(3, false);
    lb.Select(5, false);
    lb.auto_expand_items();

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
    tree.set_behavior(root, Box::new(ListBoxExpandedBehavior { list_box: lb }));

    let mut view = emView::new(root, 800.0, 800.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "listbox_expanded");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("listbox_expanded", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("listbox_expanded", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── BV-1: widget_border_rect_extreme_tall ──────────────────────

#[test]
fn golden_widget_border_rect_extreme_tall() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_border_rect_extreme_tall");

    let look = emLook::new();
    let behavior = BorderBehavior::new(OuterBorderType::Rect, InnerBorderType::None, "Test", look);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 8.0);
    tree.set_behavior(root, Box::new(behavior));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_border_rect_extreme_tall",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_border_rect_extreme_tall",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_border_rect_extreme_tall", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-2: widget_border_rect_extreme_wide ─────────────────────

#[test]
fn golden_widget_border_rect_extreme_wide() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_border_rect_extreme_wide");

    let look = emLook::new();
    let behavior = BorderBehavior::new(OuterBorderType::Rect, InnerBorderType::None, "Test", look);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.05);
    tree.set_behavior(root, Box::new(behavior));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_border_rect_extreme_wide",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_border_rect_extreme_wide",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_border_rect_extreme_wide", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-3: widget_border_roundrect_thin ─────────────────────────

#[test]
fn golden_widget_border_roundrect_thin() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_border_roundrect_thin");

    let look = emLook::new();
    let behavior = BorderBehavior::new(
        OuterBorderType::RoundRect,
        InnerBorderType::None,
        "Test",
        look,
    );

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.002);
    tree.set_behavior(root, Box::new(behavior));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_border_roundrect_thin",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_border_roundrect_thin",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_border_roundrect_thin", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-4: widget_border_instrument_cramped ─────────────────────

#[test]
fn golden_widget_border_instrument_cramped() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_border_instrument_cramped");

    let look = emLook::new();
    let behavior = BorderBehavior::new(
        OuterBorderType::Instrument,
        InnerBorderType::None,
        "ABCDEFGHIJ",
        look,
    )
    .with_description("Long description that fills space");

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.15);
    tree.set_behavior(root, Box::new(behavior));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_border_instrument_cramped",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_border_instrument_cramped",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_border_instrument_cramped", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-5: widget_label_single_char ─────────────────────────────

#[test]
fn golden_widget_label_single_char() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_label_single_char");

    let look = emLook::new();
    let label = emLabel::new("X", look);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.1);
    tree.set_behavior(root, Box::new(LabelBehavior { label }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_label_single_char",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_label_single_char", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_label_single_char", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-6: widget_label_empty ───────────────────────────────────

#[test]
fn golden_widget_label_empty() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_label_empty");

    let look = emLook::new();
    let label = emLabel::new("", look);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(LabelBehavior { label }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_label_empty");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_label_empty", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_label_empty", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-7: widget_label_long_narrow ─────────────────────────────

#[test]
fn golden_widget_label_long_narrow() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_label_long_narrow");

    let look = emLook::new();
    let label = emLabel::new(
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789 !@#$%^&*() test",
        look,
    );

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 4.0);
    tree.set_behavior(root, Box::new(LabelBehavior { label }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_label_long_narrow",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_label_long_narrow", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_label_long_narrow", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// Coverage extension golden tests (CAP audit)
// ═══════════════════════════════════════════════════════════════════

// ─── CAP-0023: widget_error_panel ──────────────────────────────

#[test]
fn widget_error_panel() {
    require_golden!();
    let panel = emErrorPanel::new("Test error: something went wrong");

    render_and_compare_tol("widget_error_panel", Box::new(panel), 0, 0.0);
}

// ─── CAP-0076: widget_tunnel ───────────────────────────────────

/// Wraps a emTunnel widget as a PanelBehavior.
struct TunnelBehavior {
    tunnel: emTunnel,
}

impl PanelBehavior for TunnelBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.tunnel.paint_tunnel(painter, w, h, pixel_scale);
    }
}

#[test]
fn widget_tunnel() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_tunnel");

    let look = emLook::new();
    let mut tunnel = emTunnel::new(look).with_caption("Tunnel Test");
    tunnel.SetDepth(10.0);
    tunnel.SetChildTallness(0.75);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(TunnelBehavior { tunnel }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_tunnel");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_tunnel", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_tunnel", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── CAP-0026: widget_file_panel ───────────────────────────────

#[test]
fn widget_file_panel() {
    require_golden!();
    // Matches C++ gen: `new emFilePanel(view, "test", NULL, true)` — no file model.
    let panel = emFilePanel::new();

    render_and_compare_tol("widget_file_panel", Box::new(panel), 0, 0.0);
}

// ─── CAP-0027: widget_file_selection_box ───────────────────────

#[test]
fn widget_file_selection_box() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_file_selection_box");

    let mut fsb = emFileSelectionBox::new("Select File");
    fsb.set_parent_directory(std::path::Path::new("/nonexistent_golden_test_dir"));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(fsb));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_file_selection_box",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_file_selection_box", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_file_selection_box", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── BV-8: widget_textfield_empty_wide ──────────────────────────

#[test]
fn golden_widget_textfield_empty_wide() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_textfield_empty_wide");

    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetCaption("Name");
    tf.SetEditable(true);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.05);
    tree.set_behavior(root, Box::new(TextFieldBehavior { text_field: tf }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_textfield_empty_wide",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_textfield_empty_wide",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_textfield_empty_wide", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-9: widget_textfield_single_char_square ──────────────────

#[test]
fn golden_widget_textfield_single_char_square() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_textfield_single_char_square");

    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetCaption("Name");
    tf.SetEditable(true);
    tf.SetText("A");

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
    tree.set_behavior(root, Box::new(TextFieldBehavior { text_field: tf }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_textfield_single_char_square",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_textfield_single_char_square",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images(
            "widget_textfield_single_char_square",
            actual,
            &expected,
            w,
            h,
        );
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-10: widget_scalarfield_min_value ────────────────────────

#[test]
fn golden_widget_scalarfield_min_value() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_scalarfield_min_value");

    let look = emLook::new();
    let mut sf = emScalarField::new(-1_000_000_000_000.0, 1_000_000_000_000.0, look);
    sf.SetCaption("Value");
    sf.SetEditable(true);
    sf.SetValue(-1_000_000_000_000.0);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(ScalarFieldBehavior { scalar_field: sf }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_scalarfield_min_value",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_scalarfield_min_value",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_scalarfield_min_value", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-11: widget_scalarfield_max_value ────────────────────────

#[test]
fn golden_widget_scalarfield_max_value() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_scalarfield_max_value");

    let look = emLook::new();
    let mut sf = emScalarField::new(-1_000_000_000_000.0, 1_000_000_000_000.0, look);
    sf.SetCaption("Value");
    sf.SetEditable(true);
    sf.SetValue(1_000_000_000_000.0);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(ScalarFieldBehavior { scalar_field: sf }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_scalarfield_max_value",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_scalarfield_max_value",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_scalarfield_max_value", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-12: widget_scalarfield_zero_range ───────────────────────

#[test]
fn golden_widget_scalarfield_zero_range() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_scalarfield_zero_range");

    let look = emLook::new();
    let mut sf = emScalarField::new(50.0, 50.0, look);
    sf.SetCaption("Value");
    sf.SetEditable(true);
    sf.SetValue(50.0);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(ScalarFieldBehavior { scalar_field: sf }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_scalarfield_zero_range",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_scalarfield_zero_range",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_scalarfield_zero_range", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-13: widget_listbox_empty ────────────────────────────────

#[test]
fn golden_widget_listbox_empty() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_listbox_empty");

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetCaption("Items");

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(ListBoxBehavior { list_box: lb }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_listbox_empty");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_listbox_empty", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_listbox_empty", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-14: widget_listbox_single ───────────────────────────────

#[test]
fn golden_widget_listbox_single() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_listbox_single");

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetCaption("Items");
    lb.AddItem("item0".to_string(), "Solo".to_string());

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(ListBoxBehavior { list_box: lb }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_listbox_single");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_listbox_single", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_listbox_single", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-15: widget_listbox_extreme_wide ─────────────────────────

#[test]
fn golden_widget_listbox_extreme_wide() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_listbox_extreme_wide");

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetCaption("Items");
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.05);
    tree.set_behavior(root, Box::new(ListBoxBehavior { list_box: lb }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_listbox_extreme_wide",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_listbox_extreme_wide",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_listbox_extreme_wide", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-16: widget_splitter_h_pos0 ──────────────────────────────

#[test]
fn golden_widget_splitter_h_pos0() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_splitter_h_pos0");

    let look = emLook::new();
    let mut sp = emSplitter::new(Orientation::Horizontal, look);
    sp.SetPos(0.0);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(SplitterBehavior { splitter: sp }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_splitter_h_pos0");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_splitter_h_pos0", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_splitter_h_pos0", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-17: widget_splitter_h_pos1 ──────────────────────────────

#[test]
fn golden_widget_splitter_h_pos1() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_splitter_h_pos1");

    let look = emLook::new();
    let mut sp = emSplitter::new(Orientation::Horizontal, look);
    sp.SetPos(1.0);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, Box::new(SplitterBehavior { splitter: sp }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "widget_splitter_h_pos1");
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("widget_splitter_h_pos1", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_splitter_h_pos1", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-18: widget_splitter_v_extreme_tall ──────────────────────

#[test]
fn golden_widget_splitter_v_extreme_tall() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_splitter_v_extreme_tall");

    let look = emLook::new();
    let mut sp = emSplitter::new(Orientation::Vertical, look);
    sp.SetPos(0.5);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 8.0);
    tree.set_behavior(root, Box::new(SplitterBehavior { splitter: sp }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_splitter_v_extreme_tall",
    );
    let actual = compositor.framebuffer().GetMap();

    // ch_tol=1: 74 pixels at y=304 differ by exactly 1 in R/G channels.
    // Root cause: area-sampling interpolation rounding at sub-pixel grip
    // boundary (extreme downscale: 149 src rows -> 4.5 dest pixels).
    // emSplitter draw ops match C++ exactly; the difference is in the
    // rendering pipeline's area sampler (same class as tktest_1x/2x).
    let result = compare_images(
        "widget_splitter_v_extreme_tall",
        actual,
        &expected,
        w,
        h,
        1,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_splitter_v_extreme_tall", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-21: widget_checkbox_extreme_tall ─────────────────────────

#[test]
fn golden_widget_checkbox_extreme_tall() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_checkbox_extreme_tall");

    let look = emLook::new();
    let cb = emCheckBox::new("Check", look);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 4.0);
    tree.set_behavior(root, Box::new(CheckBoxBehavior { check_box: cb }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_checkbox_extreme_tall",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_checkbox_extreme_tall",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_checkbox_extreme_tall", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-22: widget_tunnel_extreme_wide ──────────────────────────

#[test]
fn golden_widget_tunnel_extreme_wide() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_tunnel_extreme_wide");

    let look = emLook::new();
    let mut tunnel = emTunnel::new(look).with_caption("Tunnel");
    tunnel.SetDepth(10.0);
    tunnel.SetChildTallness(0.75);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.02);
    tree.set_behavior(root, Box::new(TunnelBehavior { tunnel }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_tunnel_extreme_wide",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_tunnel_extreme_wide",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_tunnel_extreme_wide", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-19: widget_colorfield_alpha_zero ─────────────────────────

#[test]
fn golden_widget_colorfield_alpha_zero() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_colorfield_alpha_zero");

    let look = emLook::new();
    let mut cf = emColorField::new(look);
    cf.SetCaption("Color");
    cf.SetColor(emcore::emColor::emColor::rgba(255, 0, 0, 0));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.SetAutoExpansionThreshold(root, 9.0, ViewConditionType::MinExt);
    tree.set_behavior(
        root,
        Box::new(ColorFieldExpandedBehavior { color_field: cf }),
    );

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);

    for _ in 0..30 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_colorfield_alpha_zero",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_colorfield_alpha_zero",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_colorfield_alpha_zero", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-20a: widget_colorfield_alpha_opaque ──────────────────────

#[test]
fn golden_widget_colorfield_alpha_opaque() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_colorfield_alpha_opaque");

    let look = emLook::new();
    let mut cf = emColorField::new(look);
    cf.SetCaption("Color");
    cf.SetColor(emcore::emColor::emColor::rgba(255, 0, 0, 255));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.SetAutoExpansionThreshold(root, 9.0, ViewConditionType::MinExt);
    tree.set_behavior(
        root,
        Box::new(ColorFieldExpandedBehavior { color_field: cf }),
    );

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);

    for _ in 0..30 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_colorfield_alpha_opaque",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_colorfield_alpha_opaque",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_colorfield_alpha_opaque", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── BV-20b: widget_colorfield_alpha_near ────────────────────────

#[test]
fn golden_widget_colorfield_alpha_near() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("widget_colorfield_alpha_near");

    let look = emLook::new();
    let mut cf = emColorField::new(look);
    cf.SetCaption("Color");
    cf.SetColor(emcore::emColor::emColor::rgba(255, 0, 0, 1));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.SetAutoExpansionThreshold(root, 9.0, ViewConditionType::MinExt);
    tree.set_behavior(
        root,
        Box::new(ColorFieldExpandedBehavior { color_field: cf }),
    );

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);

    for _ in 0..30 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "widget_colorfield_alpha_near",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        "widget_colorfield_alpha_near",
        actual,
        &expected,
        w,
        h,
        0,
        0.0,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("widget_colorfield_alpha_near", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── Test: composition_border_nest ──────────────────────────────

/// Nested border hierarchy: outer emBorder (RoundRect/Filled) containing
/// inner emBorder (Rect/Group) containing emLabel + emButton + emTextField.
/// Matches C++ gen_composed_border_nest().
#[test]
fn composition_border_nest() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composed_border_nest");

    let look = emLook::new();

    let mut tree = PanelTree::new();
    let root = tree.create_root("outer");

    // Outer: emLinearGroup vertical, OBT_ROUND_RECT / IBT_NONE, caption "Outer"
    // C++: outer->SetBorderType(OBT_ROUND_RECT, IBT_NONE); outer->SetVertical();
    let mut outer = emLinearGroup::vertical();
    outer.border = emBorder::new(OuterBorderType::RoundRect)
        .with_inner(InnerBorderType::None)
        .with_caption("Outer");
    outer.border.label_in_border = true;
    // C++: outer->DoLayout(0, 0, 800.0/600.0, 1.0);
    tree.Layout(root, 0.0, 0.0, 800.0 / 600.0, 1.0);

    // Inner: emLinearGroup vertical, OBT_RECT / IBT_GROUP, caption "Inner"
    // C++: inner = new Testable<emLinearLayout>(*outer, "inner", "Inner");
    //      inner->SetBorderType(OBT_RECT, IBT_GROUP); inner->SetVertical();
    let inner_id = tree.create_child(root, "inner");
    let mut inner = emLinearGroup::vertical();
    inner.border = emBorder::new(OuterBorderType::Rect)
        .with_inner(InnerBorderType::Group)
        .with_caption("Inner");
    inner.border.label_in_border = true;
    tree.set_behavior(inner_id, Box::new(inner));

    // Children of inner
    // C++: new Testable<emLabel>(*inner, "label", "Test Label")
    let label_id = tree.create_child(inner_id, "label");
    tree.set_behavior(
        label_id,
        Box::new(LabelBehavior {
            label: emLabel::new("Test Label", look.clone()),
        }),
    );

    // C++: new Testable<emButton>(*inner, "button", "Test Button")
    let button_id = tree.create_child(inner_id, "button");
    tree.set_behavior(
        button_id,
        Box::new(ButtonBehavior {
            button: emButton::new("Test Button", look.clone()),
        }),
    );

    // C++: new Testable<emTextField>(*inner, "textfield", "Field", "", emImage(), "Hello", true)
    let tf_id = tree.create_child(inner_id, "textfield");
    let mut tf = emTextField::new(look.clone());
    tf.SetCaption("Field");
    tf.SetEditable(true);
    tf.SetText("Hello");
    tree.set_behavior(tf_id, Box::new(TextFieldBehavior { text_field: tf }));

    // Set outer behavior last (after children are created)
    tree.set_behavior(root, Box::new(outer));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window
    view.SetFocused(&mut tree, false);

    // C++: TerminateEngine ctrl(sched, 200) — 200 settle rounds
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(&mut compositor, &mut tree, &view, "composed_border_nest");
    let actual = compositor.framebuffer().GetMap();

    // After fixing the emLinearLayout alignment origin bug, layout matches C++
    // closely. Remaining 2.3% (at tol=0) comes from widget-level rendering
    // differences (button text kerning, border image interpolation, text
    // positioning). At tol=3 these are well under 1%.
    let result = compare_images("composed_border_nest", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("composed_border_nest", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── Test: composition_splitter_content ─────────────────────────

/// Wraps a emSplitter with LayoutChildren for composition tests.
struct SplitterCompositionBehavior {
    splitter: emSplitter,
}

impl PanelBehavior for SplitterCompositionBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.splitter.PaintContent(painter, w, h, state.enabled);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.splitter.LayoutChildrenSimple(ctx, rect.w, rect.h);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

/// Composition test: horizontal emSplitter (pos=0.5) with two Borders (OBT_Rect),
/// each containing a emColorField and emListBox. Matches C++ gen_composed_splitter_content().
///
/// In C++, emBorder does NOT auto-layout children — children exist in the tree
/// but stay at default off-screen positions (-2,-2). The golden output shows
/// only the border fill + frame chrome, with children invisible.
#[test]
fn composition_splitter_content() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composed_splitter_content");

    let look = emLook::new();

    // Root: horizontal splitter, pos=0.5, no border (OBT_NONE/IBT_NONE)
    let mut sp = emSplitter::new(Orientation::Horizontal, look.clone());
    sp.SetPos(0.5);

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    // C++ DoLayout(0, 0, 800/600, 1.0)
    tree.Layout(root, 0.0, 0.0, 800.0 / 600.0, 1.0);
    tree.set_behavior(root, Box::new(SplitterCompositionBehavior { splitter: sp }));

    // Left child: emBorder with OBT_Rect/IBT_None, caption "Left".
    // In C++, emBorder positions children at default off-screen — so they're invisible.
    // We use BorderBehavior (PaintContent-only, no child layout) to match.
    let left_id = tree.create_child(root, "left");
    tree.set_behavior(
        left_id,
        Box::new(BorderBehavior::new(
            OuterBorderType::Rect,
            InnerBorderType::None,
            "Left",
            look.clone(),
        )),
    );

    // C++ children exist in the tree but are never positioned/visible.
    // Create them so the tree structure Match, but they'll remain off-screen.
    let _cf_id = tree.create_child(left_id, "color");
    let _lb_id = tree.create_child(left_id, "list");

    // Right child: emBorder with OBT_Rect/IBT_None, caption "Right".
    let right_id = tree.create_child(root, "right");
    tree.set_behavior(
        right_id,
        Box::new(BorderBehavior::new(
            OuterBorderType::Rect,
            InnerBorderType::None,
            "Right",
            look.clone(),
        )),
    );

    let _cf_id = tree.create_child(right_id, "color");
    let _lb_id = tree.create_child(right_id, "list");

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "composed_splitter_content",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("composed_splitter_content", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("composed_splitter_content", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── Test: composition_scrolled_listbox_in_border ───────────────

/// emBorder (OBT_RoundRect, Filled) with caption "Scrolled List" containing
/// a emListBox with 50 items scrolled to item 25.
/// In C++, emBorder doesn't auto-layout children — children stay at default
/// positions, so the golden data shows only the border chrome.
#[test]
fn composition_scrolled_listbox_in_border() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composed_scrolled_listbox");

    let look = emLook::new();

    let mut tree = PanelTree::new();
    let root = tree.create_root("border");

    // C++: emBorder with OBT_ROUND_RECT/IBT_NONE, caption "Scrolled List"
    // emBorder does not layout children — use PaintContent-only BorderBehavior.
    tree.Layout(root, 0.0, 0.0, 800.0 / 600.0, 1.0);
    tree.set_behavior(
        root,
        Box::new(BorderBehavior::new(
            OuterBorderType::RoundRect,
            InnerBorderType::None,
            "Scrolled List",
            look.clone(),
        )),
    );

    // emListBox child exists in tree but won't be visible (emBorder default positions).
    let lb_id = tree.create_child(root, "list");
    let mut lb = emListBox::new(look);
    lb.SetCaption("Items");
    for i in 1..=50 {
        lb.AddItem(format!("item{}", i - 1), format!("Item {}", i));
    }
    lb.SetSelectedIndex(24); // Item 25 (0-based index 24)
    tree.set_behavior(lb_id, Box::new(ListBoxBehavior { list_box: lb }));

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "composed_scrolled_listbox",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("composed_scrolled_listbox", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("composed_scrolled_listbox", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── Test: composition_colorfield_expansion_wide ────────────────

/// emBorder (OBT_RoundRect, IBT_Group) containing a emColorField, rendered at 800x400
/// (wide aspect ratio). In C++, emBorder doesn't auto-layout children, so the
/// golden data shows only the border shape. Verifies border rendering differs
/// correctly between wide and tall aspects after GetSubstanceRect fixes.
#[test]
fn composition_colorfield_expansion_wide() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composed_colorfield_wide");

    let look = emLook::new();

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");

    // C++: border->SetBorderType(OBT_ROUND_RECT, IBT_GROUP);
    // C++: border->DoLayout(0, 0, 800.0/400.0, 1.0);
    tree.Layout(root, 0.0, 0.0, 800.0 / 400.0, 1.0);
    tree.set_behavior(
        root,
        Box::new(BorderBehavior::new(
            OuterBorderType::RoundRect,
            InnerBorderType::Group,
            "Wide",
            look.clone(),
        )),
    );

    // C++ child: emColorField — exists in tree but not positioned by emBorder
    let _cf_id = tree.create_child(root, "color");

    let mut view = emView::new(root, 800.0, 400.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.SetFocused(&mut tree, false);

    // C++: TerminateEngine ctrl(sched, 200)
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "composed_colorfield_wide",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("composed_colorfield_wide", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("composed_colorfield_wide", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

// ─── Test: composition_colorfield_expansion_tall ────────────────

/// emBorder (OBT_RoundRect, IBT_Group) containing a emColorField, rendered at 400x800
/// (tall aspect ratio). Same hierarchy as the wide variant, verifying that the
/// border shape adapts correctly to tall Restore.
#[test]
fn composition_colorfield_expansion_tall() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composed_colorfield_tall");

    let look = emLook::new();

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");

    // C++: border->SetBorderType(OBT_ROUND_RECT, IBT_GROUP);
    // C++: border->DoLayout(0, 0, 400.0/800.0, 1.0);
    tree.Layout(root, 0.0, 0.0, 400.0 / 800.0, 1.0);
    tree.set_behavior(
        root,
        Box::new(BorderBehavior::new(
            OuterBorderType::RoundRect,
            InnerBorderType::Group,
            "Tall",
            look.clone(),
        )),
    );

    // C++ child: emColorField — exists in tree but not positioned by emBorder
    let _cf_id = tree.create_child(root, "color");

    let mut view = emView::new(root, 400.0, 800.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.SetFocused(&mut tree, false);

    // C++: TerminateEngine ctrl(sched, 200)
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    render_with_ops(
        &mut compositor,
        &mut tree,
        &view,
        "composed_colorfield_tall",
    );
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("composed_colorfield_tall", actual, &expected, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("composed_colorfield_tall", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 1);
    }
    result.unwrap();
}

/// Golden test: render a view with STRESS_TEST active and verify the overlay
/// text "Stress Test" appears in the output pixels.
///
/// Renders twice: once without stress test (baseline), once with stress test
/// active. Uses compare_images to verify they differ — the overlay must have
/// painted visible pixels in the top-left region.
#[test]
fn stress_test_overlay_golden() {
    let w: u32 = 800;
    let h: u32 = 600;

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);

    // Render baseline (no stress test)
    let mut view = emView::new(root, w as f64, h as f64);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor_base = SoftwareCompositor::new(w, h);
    compositor_base.render(&mut tree, &view);
    let baseline = compositor_base.framebuffer().GetMap().to_vec();

    // Enable stress test, sync a few frames to accumulate ring buffer entries
    view.flags.insert(ViewFlags::STRESS_TEST);
    for _ in 0..5 {
        view.sync_stress_test();
    }

    let mut compositor_st = SoftwareCompositor::new(w, h);
    compositor_st.render(&mut tree, &view);
    let actual = compositor_st.framebuffer().GetMap();

    // The overlay should make the images differ. compare_images returns Err
    // when images diverge beyond tolerance — we EXPECT divergence here.
    let result = compare_images("stress_test_overlay", actual, &baseline, w, h, 0, 0.0);
    assert!(
        result.is_err(),
        "stress test overlay should produce visible pixel differences vs baseline"
    );

    // Verify the overlay painted in the top-left corner specifically:
    // Check a small region (first 50 rows, first 200 cols) for any pixel
    // that differs between baseline and actual.
    let mut overlay_pixels_differ = false;
    for y in 0..50u32 {
        for x in 0..200u32 {
            let off = ((y * w + x) * 4) as usize;
            for ch in 0..3 {
                if actual[off + ch] != baseline[off + ch] {
                    overlay_pixels_differ = true;
                    break;
                }
            }
            if overlay_pixels_differ {
                break;
            }
        }
        if overlay_pixels_differ {
            break;
        }
    }
    assert!(
        overlay_pixels_differ,
        "stress test overlay should paint visible pixels in the top-left corner"
    );
}
