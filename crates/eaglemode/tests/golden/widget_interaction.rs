use std::cell::Cell;
use std::rc::Rc;

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emCursor::emCursor;
use emcore::emInput::{emInputEvent, InputKey, InputVariant};
use emcore::emInputState::emInputState;
use emcore::emLinearGroup::emLinearGroup;
use emcore::emPainter::emPainter;
use emcore::emPanel::Rect;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::PanelTree;
use emcore::emTiling::Orientation;
use emcore::emView::{emView, ViewFlags};
use emcore::emViewRenderer::SoftwareCompositor;

use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emCheckButton::emCheckButton;
use emcore::emListBox::{emListBox, SelectionMode};
use emcore::emLook::emLook;
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use emcore::emScalarField::emScalarField;
use emcore::emSplitter::emSplitter;
use emcore::emTextField::emTextField;

use super::common::*;

fn default_panel_state() -> PanelState {
    PanelState::default_for_test()
}

fn default_input_state() -> emInputState {
    emInputState::new()
}

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Load a widget state golden file as raw bytes.
fn load_widget_state_golden(name: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("data")
        .join("widget_state")
        .join(format!("{name}.widget_state.golden"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("Cannot read {}: {e}", path.display()))
}

/// Build a check tuple for compare_widget_state: (field, passed, detail).
fn check_u8(field: &str, actual: u8, expected: u8) -> (&str, bool, String) {
    (
        field,
        actual == expected,
        format!("actual={actual} expected={expected}"),
    )
}

fn check_usize(field: &str, actual: usize, expected: usize) -> (&str, bool, String) {
    (
        field,
        actual == expected,
        format!("actual={actual} expected={expected}"),
    )
}

fn check_f64(field: &str, actual: f64, expected: f64, eps: f64) -> (&str, bool, String) {
    (
        field,
        (actual - expected).abs() < eps,
        format!(
            "actual={actual} expected={expected} diff={}",
            (actual - expected).abs()
        ),
    )
}

fn check_str<'a>(field: &'a str, actual: &str, expected: &str) -> (&'a str, bool, String) {
    (
        field,
        actual == expected,
        format!("actual={actual:?} expected={expected:?}"),
    )
}

fn check_indices<'a>(
    field: &'a str,
    actual: &[usize],
    expected: &[usize],
) -> (&'a str, bool, String) {
    (
        field,
        actual == expected,
        format!("actual={actual:?} expected={expected:?}"),
    )
}

fn check_option_usize(field: &str, actual: Option<usize>, expected: usize) -> (&str, bool, String) {
    (
        field,
        actual == Some(expected),
        format!("actual={actual:?} expected=Some({expected})"),
    )
}

// ─── Test 1: widget_checkbox_toggle ──────────────────────────────

#[test]
fn widget_checkbox_toggle() {
    require_golden!();
    let golden = load_widget_state_golden("widget_checkbox_toggle");
    assert_eq!(golden.len(), 3, "unexpected golden file size");

    let look = emLook::new();
    let mut cb = emCheckBox::new("Check Option", look);
    let ps = default_panel_state();
    let is = default_input_state();

    let c0 = check_u8("initial", cb.IsChecked() as u8, golden[0]);

    cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
    let c1 = check_u8("after_1st_click", cb.IsChecked() as u8, golden[1]);

    cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
    let c2 = check_u8("after_2nd_click", cb.IsChecked() as u8, golden[2]);

    compare_widget_state("widget_checkbox_toggle", &[c0, c1, c2]).unwrap();
}

// ─── Test 1b: widget_checkbutton_toggle ──────────────────────────

#[test]
fn widget_checkbutton_toggle() {
    require_golden!();
    let golden = load_widget_state_golden("widget_checkbutton_toggle");
    assert_eq!(golden.len(), 3, "unexpected golden file size");

    let look = emLook::new();
    let mut cb = emCheckButton::new("Toggle Option", look);
    let ps = default_panel_state();
    let is = default_input_state();

    let c0 = check_u8("initial", cb.IsChecked() as u8, golden[0]);

    cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
    let c1 = check_u8("after_1st_click", cb.IsChecked() as u8, golden[1]);

    cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
    let c2 = check_u8("after_2nd_click", cb.IsChecked() as u8, golden[2]);

    compare_widget_state("widget_checkbutton_toggle", &[c0, c1, c2]).unwrap();
}

// ─── Test 2: widget_radiobutton_switch ───────────────────────────

#[test]
fn widget_radiobutton_switch() {
    require_golden!();
    let golden = load_widget_state_golden("widget_radiobutton_switch");
    assert_eq!(golden.len(), 8, "unexpected golden file size");

    let look = emLook::new();
    let group = RadioGroup::new();
    let _rb_a = emRadioButton::new("Option A", look.clone(), group.clone(), 0);
    let mut rb_b = emRadioButton::new("Option B", look.clone(), group.clone(), 1);
    let _rb_c = emRadioButton::new("Option C", look, group.clone(), 2);

    group.borrow_mut().SetChecked(0);
    let initial = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let c0 = check_option_usize("initial", group.borrow().GetChecked(), initial);

    let ps = default_panel_state();
    let is = default_input_state();
    rb_b.Input(&emInputEvent::press(InputKey::Enter), &ps, &is);
    let after = u32::from_le_bytes(golden[4..8].try_into().unwrap()) as usize;
    let c1 = check_option_usize("after_switch", group.borrow().GetChecked(), after);

    compare_widget_state("widget_radiobutton_switch", &[c0, c1]).unwrap();
}

// ─── Test 3: widget_listbox_select ───────────────────────────────

#[test]
fn widget_listbox_select() {
    require_golden!();
    let golden = load_widget_state_golden("widget_listbox_select");
    assert_eq!(
        golden.len(),
        8,
        "golden file size mismatch (expected count + 1 index = 8 bytes)"
    );

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::Single);
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());
    lb.AddItem("item3".to_string(), "Delta".to_string());
    lb.AddItem("item4".to_string(), "Epsilon".to_string());

    lb.Select(2, true);
    lb.Select(4, true);

    let count = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let mut expected_indices: Vec<usize> = Vec::new();
    for i in 0..count {
        let off = 4 + i * 4;
        expected_indices
            .push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
    }

    let c0 = check_indices("selection", lb.GetSelectedIndices(), &expected_indices);
    compare_widget_state("widget_listbox_select", &[c0]).unwrap();
}

// ─── Test 4: widget_splitter_setpos ──────────────────────────────

#[test]
fn widget_splitter_setpos() {
    require_golden!();
    let golden = load_widget_state_golden("widget_splitter_setpos");
    assert_eq!(golden.len(), 24, "unexpected golden file size");

    let look = emLook::new();
    let mut sp = emSplitter::new(Orientation::Horizontal, look);
    sp.SetMinMaxPos(0.0, 1.0);
    let eps = 1e-9;

    sp.SetPos(0.7);
    let expected_1 = f64::from_le_bytes(golden[0..8].try_into().unwrap());
    let c0 = check_f64("pos_0.7", sp.GetPos(), expected_1, eps);

    sp.SetPos(1.5);
    let expected_2 = f64::from_le_bytes(golden[8..16].try_into().unwrap());
    let c1 = check_f64("pos_1.5_clamped", sp.GetPos(), expected_2, eps);

    sp.SetPos(-0.5);
    let expected_3 = f64::from_le_bytes(golden[16..24].try_into().unwrap());
    let c2 = check_f64("pos_-0.5_clamped", sp.GetPos(), expected_3, eps);

    compare_widget_state("widget_splitter_setpos", &[c0, c1, c2]).unwrap();
}

// ─── Test 5: widget_textfield_type ───────────────────────────────

#[test]
fn widget_textfield_type() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_type");
    assert!(golden.len() >= 8, "golden file too short");

    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetEditable(true);
    let ps = default_panel_state();
    let is = default_input_state();

    for ch in ['a', 'b', 'c'] {
        let event = emInputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        tf.Input(&event, &ps, &is);
    }

    let text_len = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let text = std::str::from_utf8(&golden[4..4 + text_len]).expect("invalid UTF-8 in golden");
    let cursor_off = 4 + text_len;
    let cursor =
        u32::from_le_bytes(golden[cursor_off..cursor_off + 4].try_into().unwrap()) as usize;

    let c0 = check_str("text", tf.GetText(), text);
    let c1 = check_usize("cursor", tf.GetCursorIndex(), cursor);
    compare_widget_state("widget_textfield_type", &[c0, c1]).unwrap();
}

// ─── Test 6: widget_textfield_backspace ──────────────────────────

#[test]
fn widget_textfield_backspace() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_backspace");
    assert!(golden.len() >= 8, "golden file too short");

    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetEditable(true);
    let ps = default_panel_state();
    let is = default_input_state();

    for ch in ['a', 'b', 'c'] {
        let event = emInputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        tf.Input(&event, &ps, &is);
    }
    tf.Input(&emInputEvent::press(InputKey::Backspace), &ps, &is);

    let text_len = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let text = std::str::from_utf8(&golden[4..4 + text_len]).expect("invalid UTF-8 in golden");
    let cursor_off = 4 + text_len;
    let cursor =
        u32::from_le_bytes(golden[cursor_off..cursor_off + 4].try_into().unwrap()) as usize;

    let c0 = check_str("text", tf.GetText(), text);
    let c1 = check_usize("cursor", tf.GetCursorIndex(), cursor);
    compare_widget_state("widget_textfield_backspace", &[c0, c1]).unwrap();
}

// ─── Test 7: widget_textfield_select ────────────────────────────

#[test]
fn widget_textfield_select() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_select");
    assert_eq!(golden.len(), 12, "unexpected golden file size");

    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetEditable(true);
    let ps = default_panel_state();
    let is = default_input_state();

    for ch in ['a', 'b', 'c', 'd', 'e', 'f'] {
        let event = emInputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        tf.Input(&event, &ps, &is);
    }
    for _ in 0..3 {
        tf.Input(
            &emInputEvent::press(InputKey::ArrowLeft).with_shift(),
            &ps,
            &is,
        );
    }

    let sel_start = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let sel_end = u32::from_le_bytes(golden[4..8].try_into().unwrap()) as usize;
    let cursor = u32::from_le_bytes(golden[8..12].try_into().unwrap()) as usize;

    let c0 = check_usize("sel_start", tf.GetSelectionStartIndex(), sel_start);
    let c1 = check_usize("sel_end", tf.GetSelectionEndIndex(), sel_end);
    let c2 = check_usize("cursor", tf.GetCursorIndex(), cursor);
    compare_widget_state("widget_textfield_select", &[c0, c1, c2]).unwrap();
}

// ─── Test 8: widget_scalarfield_inc ─────────────────────────────

#[test]
fn widget_scalarfield_inc() {
    require_golden!();
    let golden = load_widget_state_golden("widget_scalarfield_inc");
    assert_eq!(golden.len(), 16, "unexpected golden file size");

    let look = emLook::new();
    let mut sf = emScalarField::new(0.0, 100.0, look);
    sf.SetEditable(true);
    sf.SetValue(50.0);
    let ps = default_panel_state();
    let is = default_input_state();
    let eps = 1e-9;

    sf.Input(&emInputEvent::press(InputKey::Key('+')), &ps, &is);
    let expected_inc = f64::from_le_bytes(golden[0..8].try_into().unwrap());
    let c0 = check_f64("after_inc", sf.GetValue(), expected_inc, eps);

    sf.Input(&emInputEvent::press(InputKey::Key('-')), &ps, &is);
    let expected_dec = f64::from_le_bytes(golden[8..16].try_into().unwrap());
    let c1 = check_f64("after_dec", sf.GetValue(), expected_dec, eps);

    compare_widget_state("widget_scalarfield_inc", &[c0, c1]).unwrap();
}

// ─── Test 9: widget_button_click ────────────────────────────────

#[test]
fn widget_button_click() {
    require_golden!();
    let golden = load_widget_state_golden("widget_button_click");
    assert_eq!(golden.len(), 3, "unexpected golden file size");

    let look = emLook::new();
    let mut btn = emButton::new("Click Me", look);

    let click_count = Rc::new(Cell::new(0u32));
    let cc = click_count.clone();
    btn.on_click = Some(Box::new(move || {
        cc.set(cc.get() + 1);
    }));

    let c0 = check_u8("initial_pressed", btn.IsPressed() as u8, golden[0]);

    btn.Click();
    let c1 = check_u8("after_1st_pressed", btn.IsPressed() as u8, golden[1]);
    let c2 = check_usize("after_1st_count", click_count.get() as usize, 1);

    btn.Click();
    let c3 = check_u8("after_2nd_pressed", btn.IsPressed() as u8, golden[2]);
    let c4 = check_usize("after_2nd_count", click_count.get() as usize, 2);

    compare_widget_state("widget_button_click", &[c0, c1, c2, c3, c4]).unwrap();
}

// ─── Test 10: widget_listbox_multi ──────────────────────────────

#[test]
fn widget_listbox_multi() {
    require_golden!();
    let golden = load_widget_state_golden("widget_listbox_multi");
    assert_eq!(
        golden.len(),
        12,
        "golden file size mismatch (expected count + 2 indices = 12 bytes)"
    );

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::Multi);
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());
    lb.AddItem("item3".to_string(), "Delta".to_string());
    lb.AddItem("item4".to_string(), "Epsilon".to_string());

    lb.Select(1, false);
    lb.Select(3, false);

    let count = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let mut expected_indices: Vec<usize> = Vec::new();
    for i in 0..count {
        let off = 4 + i * 4;
        expected_indices
            .push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
    }

    let c0 = check_indices(
        "multi_selection",
        lb.GetSelectedIndices(),
        &expected_indices,
    );
    compare_widget_state("widget_listbox_multi", &[c0]).unwrap();
}

// ─── Test 11: widget_listbox_toggle ─────────────────────────────

#[test]
fn widget_listbox_toggle() {
    require_golden!();
    let golden = load_widget_state_golden("widget_listbox_toggle");
    assert_eq!(
        golden.len(),
        12,
        "golden file size mismatch (expected 2 snapshots = 12 bytes)"
    );

    let look = emLook::new();
    let mut lb = emListBox::new(look);
    lb.SetSelectionType(SelectionMode::Toggle);
    lb.AddItem("item0".to_string(), "Alpha".to_string());
    lb.AddItem("item1".to_string(), "Beta".to_string());
    lb.AddItem("item2".to_string(), "Gamma".to_string());
    lb.AddItem("item3".to_string(), "Delta".to_string());
    lb.AddItem("item4".to_string(), "Epsilon".to_string());

    lb.ToggleSelection(2);

    let count1 = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let mut expected1: Vec<usize> = Vec::new();
    let mut off = 4;
    for _ in 0..count1 {
        expected1.push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
        off += 4;
    }
    let c0 = check_indices("after_toggle_on", lb.GetSelectedIndices(), &expected1);

    lb.ToggleSelection(2);

    let count2 = u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let mut expected2: Vec<usize> = Vec::new();
    for _ in 0..count2 {
        expected2.push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
        off += 4;
    }
    let c1 = check_indices("after_toggle_off", lb.GetSelectedIndices(), &expected2);

    compare_widget_state("widget_listbox_toggle", &[c0, c1]).unwrap();
}

// ─── Test 12: widget_textfield_cursor_nav ───────────────────────

#[test]
fn widget_textfield_cursor_nav() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_cursor_nav");
    assert_eq!(golden.len(), 8, "unexpected golden file size");

    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetEditable(true);
    tf.SetMultiLineMode(true);
    tf.SetText("abc\ndef");
    tf.SetCursorIndex(7);
    let ps = default_panel_state();
    let is = default_input_state();

    let cursor_before = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let c0 = check_usize("cursor_before", tf.GetCursorIndex(), cursor_before);

    tf.Input(&emInputEvent::press(InputKey::ArrowUp), &ps, &is);
    let cursor_after = u32::from_le_bytes(golden[4..8].try_into().unwrap()) as usize;
    let c1 = check_usize("cursor_after", tf.GetCursorIndex(), cursor_after);

    compare_widget_state("widget_textfield_cursor_nav", &[c0, c1]).unwrap();
}

// ─── Test 13: widget_splitter_drag ──────────────────────────────

#[test]
fn widget_splitter_drag() {
    require_golden!();
    let golden = load_widget_state_golden("widget_splitter_drag");
    assert_eq!(golden.len(), 16, "unexpected golden file size");

    let look = emLook::new();
    let mut sp = emSplitter::new(Orientation::Horizontal, look);
    sp.SetMinMaxPos(0.0, 1.0);
    sp.SetPos(0.5);
    let eps = 1e-9;

    let expected_before = f64::from_le_bytes(golden[0..8].try_into().unwrap());
    let c0 = check_f64("pos_before", sp.GetPos(), expected_before, eps);

    sp.SetPos(0.7);
    let expected_after = f64::from_le_bytes(golden[8..16].try_into().unwrap());
    let c1 = check_f64("pos_after", sp.GetPos(), expected_after, eps);

    compare_widget_state("widget_splitter_drag", &[c0, c1]).unwrap();
}

// ─── Test 14: splitter_layout_h ─────────────────────────────────

/// Wraps a emSplitter as PanelBehavior for layout testing.
struct SplitterLayoutBehavior {
    splitter: emSplitter,
}

impl PanelBehavior for SplitterLayoutBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.splitter.PaintContent(painter, w, h, state.enabled);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.splitter.LayoutChildrenSimple(ctx, rect.w, rect.h);
    }
}

/// Parse splitter layout golden: [u32 steps][steps * 9 f64s]
/// Each step: (pos, c0_x, c0_y, c0_w, c0_h, c1_x, c1_y, c1_w, c1_h)
fn parse_splitter_layout_golden(data: &[u8]) -> Vec<[f64; 9]> {
    let steps = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    assert_eq!(data.len(), 4 + steps * 72, "golden size mismatch");
    let mut result = Vec::with_capacity(steps);
    for s in 0..steps {
        let base = 4 + s * 72;
        let mut vals = [0.0f64; 9];
        for (i, slot) in vals.iter_mut().enumerate() {
            let off = base + i * 8;
            *slot = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        }
        result.push(vals);
    }
    result
}

/// Run splitter layout for a single GetPos, return [pos, c0_x, c0_y, c0_w, c0_h, c1_x, c1_y, c1_w, c1_h].
fn run_splitter_layout_step(
    orientation: Orientation,
    parent_rect: (f64, f64, f64, f64),
    pos: f64,
) -> [f64; 9] {
    let look = emLook::new();
    let mut sp = emSplitter::new(orientation, look);
    sp.SetMinMaxPos(0.0, 1.0);
    sp.SetPos(pos);
    let clamped_pos = sp.GetPos();

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.Layout(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
        1.0,
    );
    let c0 = tree.create_child(root, "left");
    let c1 = tree.create_child(root, "right");

    tree.set_behavior(root, Box::new(SplitterLayoutBehavior { splitter: sp }));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root, 1.0);
        behavior.LayoutChildren(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let r0 = tree
        .layout_rect(c0)
        .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    let r1 = tree
        .layout_rect(c1)
        .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));

    [clamped_pos, r0.x, r0.y, r0.w, r0.h, r1.x, r1.y, r1.w, r1.h]
}

static LAYOUT_FIELDS: [&str; 9] = [
    "pos", "c0_x", "c0_y", "c0_w", "c0_h", "c1_x", "c1_y", "c1_w", "c1_h",
];

#[test]
fn splitter_layout_h() {
    require_golden!();
    let golden = load_widget_state_golden("splitter_layout_h");
    let expected = parse_splitter_layout_golden(&golden);
    assert_eq!(expected.len(), 4);

    let positions = [0.5, 0.3, 0.8, 1.5];
    let parent = (0.0, 0.0, 1.0, 0.75);
    let eps = 1e-9;

    let mut checks = Vec::new();
    for (i, &pos) in positions.iter().enumerate() {
        let actual = run_splitter_layout_step(Orientation::Horizontal, parent, pos);
        for j in 0..9 {
            let field = format!("step{i}_{}", LAYOUT_FIELDS[j]);
            checks.push((
                field,
                (actual[j] - expected[i][j]).abs() < eps,
                format!("actual={:.6} expected={:.6}", actual[j], expected[i][j]),
            ));
        }
    }

    let check_refs: Vec<(&str, bool, String)> = checks
        .iter()
        .map(|(f, ok, d)| (f.as_str(), *ok, d.clone()))
        .collect();
    compare_widget_state("splitter_layout_h", &check_refs).unwrap();
}

#[test]
fn splitter_layout_v() {
    require_golden!();
    let golden = load_widget_state_golden("splitter_layout_v");
    let expected = parse_splitter_layout_golden(&golden);
    assert_eq!(expected.len(), 4);

    let positions = [0.5, 0.2, 0.7, 0.0];
    let parent = (0.0, 0.0, 1.0, 1.0);
    let eps = 1e-9;

    let mut checks = Vec::new();
    for (i, &pos) in positions.iter().enumerate() {
        let actual = run_splitter_layout_step(Orientation::Vertical, parent, pos);
        for j in 0..9 {
            let field = format!("step{i}_{}", LAYOUT_FIELDS[j]);
            checks.push((
                field,
                (actual[j] - expected[i][j]).abs() < eps,
                format!("actual={:.6} expected={:.6}", actual[j], expected[i][j]),
            ));
        }
    }

    let check_refs: Vec<(&str, bool, String)> = checks
        .iter()
        .map(|(f, ok, d)| (f.as_str(), *ok, d.clone()))
        .collect();
    compare_widget_state("splitter_layout_v", &check_refs).unwrap();
}

// ─── Test: composition_click_through_tree ────────────────────────

/// emButton wrapper that delegates Input handling (needed for mouse Click dispatch).
struct ClickableButtonPanel {
    widget: emButton,
}

impl PanelBehavior for ClickableButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, s: &PanelState, is: &emInputState) -> bool {
        self.widget.Input(e, s, is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

/// Dispatch a single Input event through the panel tree.
fn dispatch_event(
    tree: &mut PanelTree,
    view: &mut emView,
    event: &emInputEvent,
    input_state: &emInputState,
) {
    if event.variant == InputVariant::Press
        && matches!(
            event.key,
            InputKey::MouseLeft | InputKey::MouseRight | InputKey::MouseMiddle
        )
    {
        let panel = view
            .GetFocusablePanelAt(tree, event.mouse_x, event.mouse_y)
            .unwrap_or_else(|| view.GetRootPanel());
        view.set_active_panel(tree, panel, false);
    }

    let wf = view.IsFocused();
    let viewed = tree.viewed_panels_dfs();
    for panel_id in viewed {
        let mut panel_ev = event.clone();
        panel_ev.mouse_x = tree.ViewToPanelX(panel_id, event.mouse_x);
        panel_ev.mouse_y =
            tree.ViewToPanelY(panel_id, event.mouse_y, view.GetCurrentPixelTallness());

        if let Some(mut behavior) = tree.take_behavior(panel_id) {
            let panel_state = tree.build_panel_state(panel_id, wf, view.GetCurrentPixelTallness());
            if panel_ev.is_keyboard_event() && !panel_state.in_active_path {
                tree.put_behavior(panel_id, behavior);
                continue;
            }
            let consumed = behavior.Input(&panel_ev, &panel_state, input_state);
            tree.put_behavior(panel_id, behavior);
            if consumed {
                view.InvalidatePainting(tree, panel_id);
                break;
            }
        }
    }
}

#[test]
fn composition_click_through_tree() {
    let click_count = Rc::new(Cell::new(0u32));
    let clicked_clone = click_count.clone();

    let look = emLook::new();

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");

    let mut root_group = emLinearGroup::vertical();
    root_group.border = emBorder::new(OuterBorderType::Rect)
        .with_inner(InnerBorderType::None)
        .with_caption("Root");
    root_group.border.label_in_border = true;
    tree.Layout(root, 0.0, 0.0, 800.0 / 600.0, 1.0, 1.0);

    let container_id = tree.create_child(root, "container");
    let mut container_group = emLinearGroup::vertical();
    container_group.border = emBorder::new(OuterBorderType::Rect)
        .with_inner(InnerBorderType::None)
        .with_caption("Container");
    container_group.border.label_in_border = true;
    tree.set_behavior(container_id, Box::new(container_group));

    let button_id = tree.create_child(container_id, "button");
    let mut btn = emButton::new("Click Me", look);
    btn.on_click = Some(Box::new(move || {
        clicked_clone.set(clicked_clone.get() + 1);
    }));
    tree.set_behavior(button_id, Box::new(ClickableButtonPanel { widget: btn }));

    tree.set_behavior(root, Box::new(root_group));

    let mut view = emView::new_for_test(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    for _ in 0..200 {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        view.Update(&mut tree);
    }

    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut tree, &view);

    let click_x = 400.0;
    let click_y = 300.0;
    let input_state = emInputState::new();

    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(click_x, click_y);
    dispatch_event(&mut tree, &mut view, &press, &input_state);

    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(click_x, click_y);
    dispatch_event(&mut tree, &mut view, &release, &input_state);

    let c0 = check_usize("click_count", click_count.get() as usize, 1);
    compare_widget_state("composition_click_through_tree", &[c0]).unwrap();
}
