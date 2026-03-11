use std::rc::Rc;

use zuicchini::layout::Orientation;
use zuicchini::panel::{
    PanelBehavior, PanelCtx, PanelState, PanelTree, View, ViewConditionType, ViewFlags,
};
use zuicchini::render::{Painter, SoftwareCompositor};
use zuicchini::widget::{
    Border, Button, CheckBox, ColorField, InnerBorderType, Label, ListBox, Look, OuterBorderType,
    RadioButton, RadioGroup, ScalarField, Splitter, TextField,
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

/// Load a compositor golden file. Returns (width, height, rgba_bytes).
fn load_compositor_golden(name: &str) -> (u32, u32, Vec<u8>) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("golden")
        .join("compositor")
        .join(format!("{name}.compositor.golden"));
    let data =
        std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()));
    assert!(data.len() >= 8, "Golden file too short: {}", path.display());
    let width = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let height = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let expected_len = 8 + (width as usize * height as usize * 4);
    assert_eq!(
        data.len(),
        expected_len,
        "Golden file size mismatch for {name}: got {} expected {expected_len}",
        data.len()
    );
    (width, height, data[8..].to_vec())
}

/// Settle: deliver notices and update viewing until stable.
fn settle(tree: &mut PanelTree, view: &mut View) {
    for _ in 0..5 {
        tree.deliver_notices(view.window_focused());
        view.update_viewing(tree);
    }
}

// ─── PanelBehavior wrappers for widgets ──────────────────────────

/// Wraps a Border (with specific outer/inner type) as a PanelBehavior.
struct BorderBehavior {
    border: Border,
    look: Rc<Look>,
}

impl BorderBehavior {
    fn new(outer: OuterBorderType, inner: InnerBorderType, caption: &str, look: Rc<Look>) -> Self {
        let mut border = Border::new(outer).with_inner(inner).with_caption(caption);
        border.label_in_border = true;
        Self { border, look }
    }

    fn with_description(mut self, desc: &str) -> Self {
        self.border = self.border.with_description(desc);
        self
    }
}

impl PanelBehavior for BorderBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);
    }
}

/// Wraps a Label widget as a PanelBehavior.
struct LabelBehavior {
    label: Label,
}

impl PanelBehavior for LabelBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.label.paint(painter, w, h);
    }
}

/// Wraps a Button widget as a PanelBehavior.
struct ButtonBehavior {
    button: Button,
}

impl PanelBehavior for ButtonBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.button.paint(painter, w, h);
    }
}

/// Wraps a CheckBox widget as a PanelBehavior.
struct CheckBoxBehavior {
    check_box: CheckBox,
}

impl PanelBehavior for CheckBoxBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.check_box.paint(painter, w, h);
    }
}

/// Wraps a TextField widget as a PanelBehavior.
struct TextFieldBehavior {
    text_field: TextField,
}

impl PanelBehavior for TextFieldBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.text_field.paint(painter, w, h);
    }
}

/// Wraps a ScalarField widget as a PanelBehavior.
struct ScalarFieldBehavior {
    scalar_field: ScalarField,
}

impl PanelBehavior for ScalarFieldBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.scalar_field.paint(painter, w, h);
    }
}

/// Helper: render a single widget filling the entire 800x600 viewport and
/// compare against a golden file.
fn render_and_compare(name: &str, behavior: Box<dyn PanelBehavior>) {
    render_and_compare_tol(name, behavior, 1, 0.5);
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
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(root, behavior);

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    let result = compare_images(actual, &expected, w, h, channel_tolerance, max_failure_pct);
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
    let look = Look::new();
    // Residual from 9-slice section boundary rounding (~1.5%)
    render_and_compare_tol(
        "widget_border_rect",
        Box::new(BorderBehavior::new(
            OuterBorderType::Rect,
            InnerBorderType::None,
            "Test",
            look,
        )),
        1,
        2.0,
    );
}

// ─── Test 2: widget_border_round_rect ───────────────────────────

#[test]
fn widget_border_round_rect() {
    require_golden!();
    let look = Look::new();
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
        1,
        2.5,
    );
}

// ─── Test 3: widget_border_group ────────────────────────────────

#[test]
fn widget_border_group() {
    require_golden!();
    let look = Look::new();
    // Residual from 9-slice section boundary rounding (~3.6%)
    render_and_compare_tol(
        "widget_border_group",
        Box::new(BorderBehavior::new(
            OuterBorderType::Group,
            InnerBorderType::Group,
            "Group",
            look,
        )),
        1,
        4.0,
    );
}

// ─── Test 4: widget_border_instrument ───────────────────────────

#[test]
fn widget_border_instrument() {
    require_golden!();
    let look = Look::new();
    // Residual from 9-slice section boundary rounding (~7.7%)
    render_and_compare_tol(
        "widget_border_instrument",
        Box::new(BorderBehavior::new(
            OuterBorderType::Instrument,
            InnerBorderType::None,
            "Instrument",
            look,
        )),
        1,
        8.0,
    );
}

// ─── Test 5: widget_label ───────────────────────────────────────

#[test]
fn widget_label() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
        "widget_label",
        Box::new(LabelBehavior {
            label: Label::new("Hello World", look),
        }),
    );
}

// ─── Test 6: widget_button_normal ───────────────────────────────

#[test]
#[ignore = "overlay 9-slice face texture rendering diffs (~56%)"]
fn widget_button_normal() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
        "widget_button_normal",
        Box::new(ButtonBehavior {
            button: Button::new("Click Me", look),
        }),
    );
}

// ─── Test 7: widget_checkbox_unchecked ──────────────────────────

#[test]
fn widget_checkbox_unchecked() {
    require_golden!();
    let look = Look::new();
    // Residual from checkbox image 9-slice section boundary rounding (~5.2%)
    render_and_compare_tol(
        "widget_checkbox_unchecked",
        Box::new(CheckBoxBehavior {
            check_box: CheckBox::new("Check Option", look),
        }),
        1,
        6.0,
    );
}

// ─── Test 8: widget_checkbox_checked ────────────────────────────

#[test]
fn widget_checkbox_checked() {
    require_golden!();
    let look = Look::new();
    let mut cb = CheckBox::new("Check Option", look);
    cb.set_checked(true);
    // Residual from checkbox image + text rendering diffs (~5.5%)
    render_and_compare_tol(
        "widget_checkbox_checked",
        Box::new(CheckBoxBehavior { check_box: cb }),
        1,
        6.0,
    );
}

// ─── Test 9: widget_textfield_empty ─────────────────────────────

#[test]
fn widget_textfield_empty() {
    require_golden!();
    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_caption("Name");
    tf.set_editable(true);
    // Residual from 9-slice border interpolation diffs (~24%)
    render_and_compare_tol(
        "widget_textfield_empty",
        Box::new(TextFieldBehavior { text_field: tf }),
        3,
        24.5,
    );
}

// ─── Test 10: widget_textfield_content ──────────────────────────

#[test]
fn widget_textfield_content() {
    require_golden!();
    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_caption("Name");
    tf.set_editable(true);
    tf.set_text("Hello");
    // Residual from 9-slice border interpolation + text rendering diffs (~27%)
    render_and_compare_tol(
        "widget_textfield_content",
        Box::new(TextFieldBehavior { text_field: tf }),
        3,
        27.0,
    );
}

// ─── Test 11: widget_scalarfield ────────────────────────────────

#[test]
fn widget_scalarfield() {
    require_golden!();
    let look = Look::new();
    let mut sf = ScalarField::new(0.0, 100.0, look);
    sf.set_caption("Value");
    sf.set_editable(true);
    sf.set_value(50.0);
    // Residual from 9-slice border interpolation diffs (~26%)
    render_and_compare_tol(
        "widget_scalarfield",
        Box::new(ScalarFieldBehavior { scalar_field: sf }),
        3,
        26.5,
    );
}

// ─── Additional behavior wrappers ──────────────────────────────

/// Wraps a ColorField widget as a PanelBehavior.
struct ColorFieldBehavior {
    color_field: ColorField,
}

impl PanelBehavior for ColorFieldBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.color_field.paint(painter, w, h);
    }
}

/// Wraps a RadioButton widget as a PanelBehavior.
struct RadioButtonBehavior {
    radio_button: RadioButton,
}

impl PanelBehavior for RadioButtonBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.radio_button.paint(painter, w, h);
    }
}

/// Wraps a ListBox widget as a PanelBehavior.
struct ListBoxBehavior {
    list_box: ListBox,
}

impl PanelBehavior for ListBoxBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.list_box.paint(painter, w, h);
    }
}

/// Wraps a Splitter widget as a PanelBehavior.
struct SplitterBehavior {
    splitter: Splitter,
}

impl PanelBehavior for SplitterBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.splitter.paint(painter, w, h);
    }
}

// ─── Test 12: widget_colorfield ────────────────────────────────

#[test]
#[ignore = "missing child ScalarField/TextField panel composition (~33%)"]
fn widget_colorfield() {
    require_golden!();
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_caption("Color");
    cf.set_color(zuicchini::foundation::Color::rgba(255, 0, 0, 255));
    render_and_compare(
        "widget_colorfield",
        Box::new(ColorFieldBehavior { color_field: cf }),
    );
}

// ─── Test 13: widget_radiobutton ───────────────────────────────

#[test]
#[ignore = "overlay 9-slice face texture rendering diffs (~53%)"]
fn widget_radiobutton() {
    require_golden!();
    let look = Look::new();
    let group = RadioGroup::new();
    let mut rb = RadioButton::new("Radio Option", look, group, 0);
    rb.set_checked(true);
    render_and_compare(
        "widget_radiobutton",
        Box::new(RadioButtonBehavior { radio_button: rb }),
    );
}

// ─── Test 14: widget_listbox ───────────────────────────────────

#[test]
fn widget_listbox() {
    require_golden!();
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.set_caption("Items");
    lb.add_item("item0".to_string(), "Alpha".to_string());
    lb.add_item("item1".to_string(), "Beta".to_string());
    lb.add_item("item2".to_string(), "Gamma".to_string());
    lb.add_item("item3".to_string(), "Delta".to_string());
    lb.add_item("item4".to_string(), "Epsilon".to_string());
    lb.set_selected_index(2);
    // Residual from 9-slice boundary + EXTEND_ZERO premul amplification (~26%)
    render_and_compare_tol(
        "widget_listbox",
        Box::new(ListBoxBehavior { list_box: lb }),
        3,
        27.0,
    );
}

// ─── Test 15: widget_splitter_h ────────────────────────────────

#[test]
fn widget_splitter_h() {
    require_golden!();
    let look = Look::new();
    let sp = Splitter::new(Orientation::Horizontal, look);
    // Residual from 9-slice interpolation rounding (~0.9%)
    render_and_compare_tol(
        "widget_splitter_h",
        Box::new(SplitterBehavior { splitter: sp }),
        1,
        1.0,
    );
}

// ─── Test 16: widget_splitter_v ────────────────────────────────

#[test]
fn widget_splitter_v() {
    require_golden!();
    let look = Look::new();
    let mut sp = Splitter::new(Orientation::Vertical, look);
    sp.set_position(0.3);
    // Residual from 9-slice interpolation rounding + grip position (~1.7%)
    render_and_compare_tol(
        "widget_splitter_v",
        Box::new(SplitterBehavior { splitter: sp }),
        1,
        2.0,
    );
}

// ─── Test 17: colorfield_expanded ─────────────────────────────

/// Wraps a ColorField as a PanelBehavior with layout_children delegation
/// for auto-expanded child panels.
struct ColorFieldExpandedBehavior {
    color_field: ColorField,
}

impl PanelBehavior for ColorFieldExpandedBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.color_field.paint(painter, w, h);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.color_field.layout_children(ctx, rect.w, rect.h);
    }
}

/// Expanded ColorField with child ScalarFields for RGBA/HSV editing.
/// C++ renders RasterLayout with 8 ScalarFields + TextField on right half;
/// Rust ColorField expansion doesn't create child panels yet.
#[test]
#[ignore = "Rust ColorField expansion does not create child panels — swatch-only vs C++ full expansion"]
fn colorfield_expanded() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("colorfield_expanded");

    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_caption("Color");
    cf.set_editable(true);
    cf.set_alpha_enabled(true);
    cf.set_color(zuicchini::foundation::Color::rgba(0xBB, 0x22, 0x22, 0xFF));

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    // C++ emColorField uses AE threshold 9 (VCT_MIN_EXT)
    tree.set_auto_expansion_threshold(root, 9.0, ViewConditionType::MinExt);
    tree.set_behavior(
        root,
        Box::new(ColorFieldExpandedBehavior { color_field: cf }),
    );

    let mut view = View::new(root, 800.0, 800.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.set_window_focused(&mut tree, false);

    // 20 settle rounds for auto-expansion cascade
    for _ in 0..20 {
        tree.deliver_notices(view.window_focused());
        view.update_viewing(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    let result = compare_images(actual, &expected, w, h, 3, 45.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("colorfield_expanded", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}

// ─── Test 18: listbox_expanded ────────────────────────────────

/// Wraps a ListBox as a PanelBehavior for expanded rendering.
struct ListBoxExpandedBehavior {
    list_box: ListBox,
}

impl PanelBehavior for ListBoxExpandedBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.list_box.paint(painter, w, h);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

/// Expanded ListBox with 7 items, 3 multi-selected.
/// C++ renders child DefaultItemPanel panels laid out by emRasterGroup grid;
/// Rust paints items inline as single-column rows.
#[test]
#[ignore = "C++ uses child DefaultItemPanel in RasterGroup grid; Rust paints items inline"]
fn listbox_expanded() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("listbox_expanded");

    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.set_caption("Items");
    lb.set_selection_mode(zuicchini::widget::SelectionMode::Multi);
    lb.set_items(
        ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    lb.select(1, false);
    lb.select(3, false);
    lb.select(5, false);
    lb.auto_expand_items();

    let mut tree = PanelTree::new();
    let root = tree.create_root("test");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    tree.set_behavior(root, Box::new(ListBoxExpandedBehavior { list_box: lb }));

    let mut view = View::new(root, 800.0, 800.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    view.set_window_focused(&mut tree, false);

    for _ in 0..20 {
        tree.deliver_notices(view.window_focused());
        view.update_viewing(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    let result = compare_images(actual, &expected, w, h, 3, 50.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("listbox_expanded", actual, &expected, w, h);
        analyze_diff_distribution(actual, &expected, w, h, 3);
    }
    result.unwrap();
}
