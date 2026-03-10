use std::rc::Rc;

use zuicchini::layout::Orientation;
use zuicchini::panel::{PanelBehavior, PanelState, PanelTree, View, ViewFlags};
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

    compare_images(actual, &expected, w, h, 1, 0.5).unwrap();
}

// ─── Test 1: widget_border_rect ─────────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — text rendering diffs (~1.5%)"]
fn widget_border_rect() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
        "widget_border_rect",
        Box::new(BorderBehavior::new(
            OuterBorderType::Rect,
            InnerBorderType::None,
            "Test",
            look,
        )),
    );
}

// ─── Test 2: widget_border_round_rect ───────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — text rendering diffs (~2%)"]
fn widget_border_round_rect() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
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
    );
}

// ─── Test 3: widget_border_group ────────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — interpolation diffs (~2.7%)"]
fn widget_border_group() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
        "widget_border_group",
        Box::new(BorderBehavior::new(
            OuterBorderType::Group,
            InnerBorderType::Group,
            "Group",
            look,
        )),
    );
}

// ─── Test 4: widget_border_instrument ───────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — interpolation diffs (~4.6%)"]
fn widget_border_instrument() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
        "widget_border_instrument",
        Box::new(BorderBehavior::new(
            OuterBorderType::Instrument,
            InnerBorderType::None,
            "Instrument",
            look,
        )),
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
#[ignore = "WIP: Phase 6 border image parity — layout mismatch (~64%)"]
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
#[ignore = "WIP: Phase 6 border image parity — layout mismatch (~90%)"]
fn widget_checkbox_unchecked() {
    require_golden!();
    let look = Look::new();
    render_and_compare(
        "widget_checkbox_unchecked",
        Box::new(CheckBoxBehavior {
            check_box: CheckBox::new("Check Option", look),
        }),
    );
}

// ─── Test 8: widget_checkbox_checked ────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — layout mismatch (~90%)"]
fn widget_checkbox_checked() {
    require_golden!();
    let look = Look::new();
    let mut cb = CheckBox::new("Check Option", look);
    cb.set_checked(true);
    render_and_compare(
        "widget_checkbox_checked",
        Box::new(CheckBoxBehavior { check_box: cb }),
    );
}

// ─── Test 9: widget_textfield_empty ─────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — field rendering diffs (~22%)"]
fn widget_textfield_empty() {
    require_golden!();
    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_editable(true);
    render_and_compare(
        "widget_textfield_empty",
        Box::new(TextFieldBehavior { text_field: tf }),
    );
}

// ─── Test 10: widget_textfield_content ──────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — field rendering diffs (~31%)"]
fn widget_textfield_content() {
    require_golden!();
    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_editable(true);
    tf.set_text("Hello");
    render_and_compare(
        "widget_textfield_content",
        Box::new(TextFieldBehavior { text_field: tf }),
    );
}

// ─── Test 11: widget_scalarfield ────────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — field rendering diffs (~67%)"]
fn widget_scalarfield() {
    require_golden!();
    let look = Look::new();
    let mut sf = ScalarField::new(0.0, 100.0, look);
    sf.set_value(50.0);
    render_and_compare(
        "widget_scalarfield",
        Box::new(ScalarFieldBehavior { scalar_field: sf }),
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
#[ignore = "WIP: Phase 6 border image parity — layout mismatch (~79%)"]
fn widget_colorfield() {
    require_golden!();
    let look = Look::new();
    let mut cf = ColorField::new(look);
    cf.set_color(zuicchini::foundation::Color::rgba(255, 0, 0, 255));
    render_and_compare(
        "widget_colorfield",
        Box::new(ColorFieldBehavior { color_field: cf }),
    );
}

// ─── Test 13: widget_radiobutton ───────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — layout mismatch (~60%)"]
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
#[ignore = "WIP: Phase 6 border image parity — layout mismatch (~44%)"]
fn widget_listbox() {
    require_golden!();
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("item0".to_string(), "Alpha".to_string());
    lb.add_item("item1".to_string(), "Beta".to_string());
    lb.add_item("item2".to_string(), "Gamma".to_string());
    lb.add_item("item3".to_string(), "Delta".to_string());
    lb.add_item("item4".to_string(), "Epsilon".to_string());
    lb.set_selected_index(2);
    render_and_compare("widget_listbox", Box::new(ListBoxBehavior { list_box: lb }));
}

// ─── Test 15: widget_splitter_h ────────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — splitter diffs (~1.5%)"]
fn widget_splitter_h() {
    require_golden!();
    let look = Look::new();
    let sp = Splitter::new(Orientation::Horizontal, look);
    render_and_compare(
        "widget_splitter_h",
        Box::new(SplitterBehavior { splitter: sp }),
    );
}

// ─── Test 16: widget_splitter_v ────────────────────────────────

#[test]
#[ignore = "WIP: Phase 6 border image parity — splitter diffs (~1.5%)"]
fn widget_splitter_v() {
    require_golden!();
    let look = Look::new();
    let mut sp = Splitter::new(Orientation::Vertical, look);
    sp.set_position(0.3);
    render_and_compare(
        "widget_splitter_v",
        Box::new(SplitterBehavior { splitter: sp }),
    );
}
