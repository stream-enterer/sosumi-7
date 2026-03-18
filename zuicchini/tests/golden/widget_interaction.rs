use zuicchini::foundation::Rect;
use zuicchini::input::{InputEvent, InputKey};
use zuicchini::layout::Orientation;
use zuicchini::panel::{PanelBehavior, PanelCtx, PanelState, PanelTree};
use zuicchini::render::Painter;
use zuicchini::widget::{
    Button, CheckBox, CheckButton, ListBox, Look, RadioButton, RadioGroup, ScalarField,
    SelectionMode, Splitter, TextField,
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

// ─── Test 1: widget_checkbox_toggle ──────────────────────────────

#[test]
fn widget_checkbox_toggle() {
    require_golden!();
    let golden = load_widget_state_golden("widget_checkbox_toggle");
    assert_eq!(golden.len(), 3, "unexpected golden file size");

    let look = Look::new();
    let mut cb = CheckBox::new("Check Option", look);

    // Initial state
    assert_eq!(
        cb.is_checked() as u8,
        golden[0],
        "initial checked state mismatch"
    );

    // After first activation (Enter is instant — no release needed)
    cb.input(&InputEvent::press(InputKey::Enter));
    assert_eq!(cb.is_checked() as u8, golden[1], "after 1st click mismatch");

    // After second activation
    cb.input(&InputEvent::press(InputKey::Enter));
    assert_eq!(cb.is_checked() as u8, golden[2], "after 2nd click mismatch");
}

// ─── Test 1b: widget_checkbutton_toggle ──────────────────────────

#[test]
fn widget_checkbutton_toggle() {
    require_golden!();
    let golden = load_widget_state_golden("widget_checkbutton_toggle");
    assert_eq!(golden.len(), 3, "unexpected golden file size");

    let look = Look::new();
    let mut cb = CheckButton::new("Toggle Option", look);

    // Initial state
    assert_eq!(
        cb.is_checked() as u8,
        golden[0],
        "initial checked state mismatch"
    );

    // After first activation (Enter is instant — no release needed)
    cb.input(&InputEvent::press(InputKey::Enter));
    assert_eq!(cb.is_checked() as u8, golden[1], "after 1st click mismatch");

    // After second activation
    cb.input(&InputEvent::press(InputKey::Enter));
    assert_eq!(cb.is_checked() as u8, golden[2], "after 2nd click mismatch");
}

// ─── Test 2: widget_radiobutton_switch ───────────────────────────

#[test]
fn widget_radiobutton_switch() {
    require_golden!();
    let golden = load_widget_state_golden("widget_radiobutton_switch");
    assert_eq!(golden.len(), 8, "unexpected golden file size");

    let look = Look::new();
    let group = RadioGroup::new();
    let _rb_a = RadioButton::new("Option A", look.clone(), group.clone(), 0);
    let mut rb_b = RadioButton::new("Option B", look.clone(), group.clone(), 1);
    let _rb_c = RadioButton::new("Option C", look, group.clone(), 2);

    // Initial: A checked
    group.borrow_mut().select(0);
    let initial = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    assert_eq!(
        group.borrow().selected(),
        Some(initial),
        "initial radio check mismatch"
    );

    // Activate B (Enter is instant — no release needed)
    rb_b.input(&InputEvent::press(InputKey::Enter));
    let after = u32::from_le_bytes(golden[4..8].try_into().unwrap()) as usize;
    assert_eq!(
        group.borrow().selected(),
        Some(after),
        "after switch mismatch"
    );
}

// ─── Test 3: widget_listbox_select ───────────────────────────────

#[test]
fn widget_listbox_select() {
    require_golden!();
    let golden = load_widget_state_golden("widget_listbox_select");
    assert!(golden.len() >= 4, "golden file too short");

    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.set_selection_mode(SelectionMode::Single);
    lb.add_item("item0".to_string(), "Alpha".to_string());
    lb.add_item("item1".to_string(), "Beta".to_string());
    lb.add_item("item2".to_string(), "Gamma".to_string());
    lb.add_item("item3".to_string(), "Delta".to_string());
    lb.add_item("item4".to_string(), "Epsilon".to_string());

    // Select 2, then 4 (single mode should replace)
    lb.select(2, true);
    lb.select(4, true);

    // Parse golden: [u32 count][u32 * count indices]
    let count = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let mut expected_indices: Vec<usize> = Vec::new();
    for i in 0..count {
        let off = 4 + i * 4;
        expected_indices
            .push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
    }

    assert_eq!(
        lb.selected_indices(),
        &expected_indices,
        "listbox selection mismatch"
    );
}

// ─── Test 4: widget_splitter_setpos ──────────────────────────────

#[test]
fn widget_splitter_setpos() {
    require_golden!();
    let golden = load_widget_state_golden("widget_splitter_setpos");
    assert_eq!(golden.len(), 24, "unexpected golden file size");

    let look = Look::new();
    let mut sp = Splitter::new(Orientation::Horizontal, look);
    sp.set_limits(0.0, 1.0);

    let eps = 1e-9;

    // Normal value
    sp.set_position(0.7);
    let expected_1 = f64::from_le_bytes(golden[0..8].try_into().unwrap());
    assert!(
        (sp.position() - expected_1).abs() < eps,
        "pos 0.7: actual={} expected={}",
        sp.position(),
        expected_1
    );

    // Above max — should clamp
    sp.set_position(1.5);
    let expected_2 = f64::from_le_bytes(golden[8..16].try_into().unwrap());
    assert!(
        (sp.position() - expected_2).abs() < eps,
        "pos 1.5 clamped: actual={} expected={}",
        sp.position(),
        expected_2
    );

    // Below min — should clamp
    sp.set_position(-0.5);
    let expected_3 = f64::from_le_bytes(golden[16..24].try_into().unwrap());
    assert!(
        (sp.position() - expected_3).abs() < eps,
        "pos -0.5 clamped: actual={} expected={}",
        sp.position(),
        expected_3
    );
}

// ─── Test 5: widget_textfield_type ───────────────────────────────

#[test]
fn widget_textfield_type() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_type");
    assert!(golden.len() >= 8, "golden file too short");

    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_editable(true);

    // Type "abc"
    for ch in ['a', 'b', 'c'] {
        let event = InputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        tf.input(&event);
    }

    // Parse golden: [u32 text_len][text_bytes][u32 cursor_pos]
    let text_len = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let text = std::str::from_utf8(&golden[4..4 + text_len]).expect("invalid UTF-8 in golden");
    let cursor_off = 4 + text_len;
    let cursor =
        u32::from_le_bytes(golden[cursor_off..cursor_off + 4].try_into().unwrap()) as usize;

    assert_eq!(tf.text(), text, "text mismatch");
    assert_eq!(tf.cursor_pos(), cursor, "cursor mismatch");
}

// ─── Test 6: widget_textfield_backspace ──────────────────────────

#[test]
fn widget_textfield_backspace() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_backspace");
    assert!(golden.len() >= 8, "golden file too short");

    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_editable(true);

    // Type "abc"
    for ch in ['a', 'b', 'c'] {
        let event = InputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        tf.input(&event);
    }

    // Backspace
    tf.input(&InputEvent::press(InputKey::Backspace));

    // Parse golden: [u32 text_len][text_bytes][u32 cursor_pos]
    let text_len = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let text = std::str::from_utf8(&golden[4..4 + text_len]).expect("invalid UTF-8 in golden");
    let cursor_off = 4 + text_len;
    let cursor =
        u32::from_le_bytes(golden[cursor_off..cursor_off + 4].try_into().unwrap()) as usize;

    assert_eq!(tf.text(), text, "text mismatch");
    assert_eq!(tf.cursor_pos(), cursor, "cursor mismatch");
}

// ─── Test 7: widget_textfield_select ────────────────────────────

#[test]
fn widget_textfield_select() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_select");
    assert_eq!(golden.len(), 12, "unexpected golden file size");

    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_editable(true);

    // Type "abcdef"
    for ch in ['a', 'b', 'c', 'd', 'e', 'f'] {
        let event = InputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        tf.input(&event);
    }

    // Shift+ArrowLeft × 3 to select last 3 chars
    for _ in 0..3 {
        tf.input(&InputEvent::press(InputKey::ArrowLeft).with_shift());
    }

    // Parse golden: [u32 sel_start][u32 sel_end][u32 cursor]
    let sel_start = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let sel_end = u32::from_le_bytes(golden[4..8].try_into().unwrap()) as usize;
    let cursor = u32::from_le_bytes(golden[8..12].try_into().unwrap()) as usize;

    assert_eq!(tf.selection_start(), sel_start, "sel_start mismatch");
    assert_eq!(tf.selection_end(), sel_end, "sel_end mismatch");
    assert_eq!(tf.cursor_pos(), cursor, "cursor mismatch");
}

// ─── Test 8: widget_scalarfield_inc ─────────────────────────────

#[test]
fn widget_scalarfield_inc() {
    require_golden!();
    let golden = load_widget_state_golden("widget_scalarfield_inc");
    assert_eq!(golden.len(), 16, "unexpected golden file size");

    let look = Look::new();
    let mut sf = ScalarField::new(0.0, 100.0, look);
    sf.set_value(50.0);

    let eps = 1e-9;

    // Press "+" to increment
    sf.input(&InputEvent::press(InputKey::Key('+')));
    let expected_inc = f64::from_le_bytes(golden[0..8].try_into().unwrap());
    assert!(
        (sf.value() - expected_inc).abs() < eps,
        "after +: actual={} expected={}",
        sf.value(),
        expected_inc
    );

    // Press "-" to decrement
    sf.input(&InputEvent::press(InputKey::Key('-')));
    let expected_dec = f64::from_le_bytes(golden[8..16].try_into().unwrap());
    assert!(
        (sf.value() - expected_dec).abs() < eps,
        "after -: actual={} expected={}",
        sf.value(),
        expected_dec
    );
}

// ─── Test 9: widget_button_click ────────────────────────────────

#[test]
fn widget_button_click() {
    require_golden!();
    let golden = load_widget_state_golden("widget_button_click");
    assert_eq!(golden.len(), 3, "unexpected golden file size");

    let look = Look::new();
    let mut btn = Button::new("Click Me", look);

    // Initial state: not pressed
    assert_eq!(
        btn.is_pressed() as u8,
        golden[0],
        "initial pressed state mismatch"
    );

    // After programmatic click(): pressed state unchanged (click is instantaneous)
    btn.click();
    assert_eq!(
        btn.is_pressed() as u8,
        golden[1],
        "after 1st click pressed mismatch"
    );

    // After second click
    btn.click();
    assert_eq!(
        btn.is_pressed() as u8,
        golden[2],
        "after 2nd click pressed mismatch"
    );
}

// ─── Test 10: widget_listbox_multi ──────────────────────────────

#[test]
fn widget_listbox_multi() {
    require_golden!();
    let golden = load_widget_state_golden("widget_listbox_multi");
    assert!(golden.len() >= 4, "golden file too short");

    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.set_selection_mode(SelectionMode::Multi);
    lb.add_item("item0".to_string(), "Alpha".to_string());
    lb.add_item("item1".to_string(), "Beta".to_string());
    lb.add_item("item2".to_string(), "Gamma".to_string());
    lb.add_item("item3".to_string(), "Delta".to_string());
    lb.add_item("item4".to_string(), "Epsilon".to_string());

    // Select items 1 and 3 additively
    lb.select(1, false);
    lb.select(3, false);

    // Parse golden: [u32 count][u32*count indices]
    let count = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let mut expected_indices: Vec<usize> = Vec::new();
    for i in 0..count {
        let off = 4 + i * 4;
        expected_indices
            .push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
    }

    assert_eq!(
        lb.selected_indices(),
        &expected_indices,
        "listbox multi-selection mismatch"
    );
}

// ─── Test 11: widget_listbox_toggle ─────────────────────────────

#[test]
fn widget_listbox_toggle() {
    require_golden!();
    let golden = load_widget_state_golden("widget_listbox_toggle");
    assert!(golden.len() >= 8, "golden file too short");

    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.set_selection_mode(SelectionMode::Toggle);
    lb.add_item("item0".to_string(), "Alpha".to_string());
    lb.add_item("item1".to_string(), "Beta".to_string());
    lb.add_item("item2".to_string(), "Gamma".to_string());
    lb.add_item("item3".to_string(), "Delta".to_string());
    lb.add_item("item4".to_string(), "Epsilon".to_string());

    // Toggle item 2 on — first snapshot
    lb.toggle_selection(2);

    let count1 = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    let mut expected1: Vec<usize> = Vec::new();
    let mut off = 4;
    for _ in 0..count1 {
        expected1.push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
        off += 4;
    }
    assert_eq!(
        lb.selected_indices(),
        &expected1,
        "after toggle-on mismatch"
    );

    // Toggle item 2 off — second snapshot
    lb.toggle_selection(2);

    let count2 = u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize;
    off += 4;
    let mut expected2: Vec<usize> = Vec::new();
    for _ in 0..count2 {
        expected2.push(u32::from_le_bytes(golden[off..off + 4].try_into().unwrap()) as usize);
        off += 4;
    }
    assert_eq!(
        lb.selected_indices(),
        &expected2,
        "after toggle-off mismatch"
    );
}

// ─── Test 12: widget_textfield_cursor_nav ───────────────────────

#[test]
fn widget_textfield_cursor_nav() {
    require_golden!();
    let golden = load_widget_state_golden("widget_textfield_cursor_nav");
    assert_eq!(golden.len(), 8, "unexpected golden file size");

    let look = Look::new();
    let mut tf = TextField::new(look);
    tf.set_editable(true);
    tf.set_multi_line(true);
    tf.set_text("abc\ndef");
    tf.set_cursor_index(7); // End of "abc\ndef"

    let cursor_before = u32::from_le_bytes(golden[0..4].try_into().unwrap()) as usize;
    assert_eq!(
        tf.cursor_pos(),
        cursor_before,
        "cursor before ArrowUp mismatch"
    );

    // ArrowUp
    tf.input(&InputEvent::press(InputKey::ArrowUp));

    let cursor_after = u32::from_le_bytes(golden[4..8].try_into().unwrap()) as usize;
    assert_eq!(
        tf.cursor_pos(),
        cursor_after,
        "cursor after ArrowUp mismatch"
    );
}

// ─── Test 13: widget_splitter_drag ──────────────────────────────

#[test]
fn widget_splitter_drag() {
    require_golden!();
    let golden = load_widget_state_golden("widget_splitter_drag");
    assert_eq!(golden.len(), 16, "unexpected golden file size");

    let look = Look::new();
    let mut sp = Splitter::new(Orientation::Horizontal, look);
    sp.set_limits(0.0, 1.0);
    sp.set_position(0.5);

    let eps = 1e-9;

    let expected_before = f64::from_le_bytes(golden[0..8].try_into().unwrap());
    assert!(
        (sp.position() - expected_before).abs() < eps,
        "pos before: actual={} expected={}",
        sp.position(),
        expected_before
    );

    // Set position to 0.7 (matching C++ SetPos(0.7))
    sp.set_position(0.7);
    let expected_after = f64::from_le_bytes(golden[8..16].try_into().unwrap());
    assert!(
        (sp.position() - expected_after).abs() < eps,
        "pos after: actual={} expected={}",
        sp.position(),
        expected_after
    );
}

// ─── Test 14: splitter_layout_h ─────────────────────────────────

/// Wraps a Splitter as PanelBehavior for layout testing.
struct SplitterLayoutBehavior {
    splitter: Splitter,
}

impl PanelBehavior for SplitterLayoutBehavior {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        self.splitter.paint(painter, w, h);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.splitter.layout_children(ctx, rect.w, rect.h);
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
        for i in 0..9 {
            let off = base + i * 8;
            vals[i] = f64::from_le_bytes(data[off..off + 8].try_into().unwrap());
        }
        result.push(vals);
    }
    result
}

/// Run splitter layout for a single position, return [pos, c0_x, c0_y, c0_w, c0_h, c1_x, c1_y, c1_w, c1_h].
fn run_splitter_layout_step(
    orientation: Orientation,
    parent_rect: (f64, f64, f64, f64),
    pos: f64,
) -> [f64; 9] {
    let look = Look::new();
    let mut sp = Splitter::new(orientation, look);
    sp.set_limits(0.0, 1.0);
    sp.set_position(pos);
    let clamped_pos = sp.position();

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
    );
    let c0 = tree.create_child(root, "left");
    let c1 = tree.create_child(root, "right");

    tree.set_behavior(root, Box::new(SplitterLayoutBehavior { splitter: sp }));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
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

#[test]
fn splitter_layout_h() {
    require_golden!();
    let golden = load_widget_state_golden("splitter_layout_h");
    let expected = parse_splitter_layout_golden(&golden);
    assert_eq!(expected.len(), 4);

    // C++ uses layout (0,0,1.0,0.75), positions: 0.5, 0.3, 0.8, 1.5 (clamped to 1.0)
    let positions = [0.5, 0.3, 0.8, 1.5];
    let parent = (0.0, 0.0, 1.0, 0.75);

    let eps = 1e-9;
    for (i, &pos) in positions.iter().enumerate() {
        let actual = run_splitter_layout_step(Orientation::Horizontal, parent, pos);
        for j in 0..9 {
            assert!(
                (actual[j] - expected[i][j]).abs() < eps,
                "step {i} field {j}: actual={:.6} expected={:.6}",
                actual[j],
                expected[i][j]
            );
        }
    }
}

#[test]
fn splitter_layout_v() {
    require_golden!();
    let golden = load_widget_state_golden("splitter_layout_v");
    let expected = parse_splitter_layout_golden(&golden);
    assert_eq!(expected.len(), 4);

    // C++ uses layout (0,0,1.0,1.0), positions: 0.5, 0.2, 0.7, 0.0 (at min)
    let positions = [0.5, 0.2, 0.7, 0.0];
    let parent = (0.0, 0.0, 1.0, 1.0);

    let eps = 1e-9;
    for (i, &pos) in positions.iter().enumerate() {
        let actual = run_splitter_layout_step(Orientation::Vertical, parent, pos);
        for j in 0..9 {
            assert!(
                (actual[j] - expected[i][j]).abs() < eps,
                "step {i} field {j}: actual={:.6} expected={:.6}",
                actual[j],
                expected[i][j]
            );
        }
    }
}
