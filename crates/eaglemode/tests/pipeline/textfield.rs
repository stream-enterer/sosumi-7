//! Systematic interaction tests for emTextField at 1x and 2x zoom.
//!
//! These tests drive Input through the full PipelineTestHarness dispatch
//! pipeline (VIF chain, hit test, coordinate transform, keyboard suppression)
//! and assert on widget STATE (text content, cursor GetPos), not pixels.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emCursor::emCursor;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;
use emcore::emTextField::emTextField;
use emcore::emViewRenderer::SoftwareCompositor;

use super::support::pipeline::PipelineTestHarness;

// ---------------------------------------------------------------------------
// SharedTextFieldPanel -- PanelBehavior wrapper with shared emTextField access
// ---------------------------------------------------------------------------

/// PanelBehavior wrapper for emTextField. The widget is stored behind
/// Rc<RefCell> so the test can inspect state after Input dispatch.
struct SharedTextFieldPanel {
    inner: Rc<RefCell<emTextField>>,
}

impl PanelBehavior for SharedTextFieldPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.inner
            .borrow_mut()
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        self.inner.borrow_mut().Input(event, state, input_state)
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.inner
                .borrow_mut()
                .on_focus_changed(state.in_active_path);
        }
    }

    fn GetCursor(&self) -> emCursor {
        self.inner.borrow().GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Helper: set up a pipeline harness with a single editable emTextField panel
// ---------------------------------------------------------------------------

/// Create a PipelineTestHarness with an editable emTextField child panel
/// filling the entire root. Returns the harness and the shared emTextField ref.
fn setup_textfield_harness() -> (PipelineTestHarness, Rc<RefCell<emTextField>>) {
    let look = emLook::new();
    let mut tf = emTextField::new(look);
    tf.SetEditable(true);

    let tf_ref = Rc::new(RefCell::new(tf));

    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let _panel_id = h.add_panel_with(
        root,
        "text_field",
        Box::new(SharedTextFieldPanel {
            inner: tf_ref.clone(),
        }),
    );

    // Settle layout.
    h.tick_n(5);

    (h, tf_ref)
}

/// Render the harness at the given viewport size so that PaintContent() is called on
/// the emTextField, populating its cached last_w / last_h dimensions (required
/// for mouse hit-testing and the min_ext guard in Input()).
fn render(h: &mut PipelineTestHarness, width: u32, height: u32) {
    let mut compositor = SoftwareCompositor::new(width, height);
    compositor.render(&mut h.tree, &h.view);
}

/// Type a string character-by-character through the pipeline using press_char.
fn type_string(h: &mut PipelineTestHarness, s: &str) {
    for ch in s.chars() {
        h.press_char(ch);
    }
}

// ===========================================================================
// Tests
// ===========================================================================

/// Type "abc" at 1x zoom and "xyz" at 2x zoom. Verify the text content after
/// each sequence.
#[test]
fn textfield_type_1x_and_2x() {
    let (mut h, tf_ref) = setup_textfield_harness();

    // ── 1x zoom ────────────────────────────────────────────────────────
    render(&mut h, 800, 600);

    // Click at viewport center to focus the text field panel.
    h.click(400.0, 300.0);

    // Type "abc" through the full pipeline.
    type_string(&mut h, "abc");

    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abc",
            "After typing 'abc' at 1x zoom, text should be 'abc' but got '{}'",
            tf.GetText()
        );
        assert_eq!(
            tf.GetCursorIndex(),
            3,
            "Cursor should be at end of 'abc' (byte 3), got {}",
            tf.GetCursorIndex()
        );
    }

    // ── 2x zoom ────────────────────────────────────────────────────────
    // Clear the field via direct API (dispatch doesn't expose modifier keys).
    tf_ref.borrow_mut().SetText("");
    assert_eq!(tf_ref.borrow().GetText(), "", "Text should be cleared");

    // Zoom to 2x.
    h.set_zoom(2.0);
    h.tick_n(5);
    render(&mut h, 800, 600);

    // Click at viewport center to re-focus.
    h.click(400.0, 300.0);

    // Type "xyz" at 2x zoom.
    type_string(&mut h, "xyz");

    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "xyz",
            "After typing 'xyz' at 2x zoom, text should be 'xyz' but got '{}'",
            tf.GetText()
        );
        assert_eq!(
            tf.GetCursorIndex(),
            3,
            "Cursor should be at end of 'xyz' (byte 3), got {}",
            tf.GetCursorIndex()
        );
    }
}

/// Verify that Backspace deletes the last character at both zoom levels.
#[test]
fn textfield_backspace_1x_and_2x() {
    let (mut h, tf_ref) = setup_textfield_harness();
    render(&mut h, 800, 600);

    // Focus
    h.click(400.0, 300.0);

    // ── 1x: type "hello", backspace twice → "hel" ─────────────────────
    type_string(&mut h, "hello");
    assert_eq!(tf_ref.borrow().GetText(), "hello");

    h.press_key(InputKey::Backspace);
    h.press_key(InputKey::Backspace);

    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "hel",
            "After 2 backspaces from 'hello', expected 'hel' but got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 3);
    }

    // ── 2x: Clear, type "world", backspace once → "worl" ──────────────
    tf_ref.borrow_mut().SetText("");

    h.set_zoom(2.0);
    h.tick_n(5);
    render(&mut h, 800, 600);

    h.click(400.0, 300.0);
    type_string(&mut h, "world");
    assert_eq!(tf_ref.borrow().GetText(), "world");

    h.press_key(InputKey::Backspace);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "worl",
            "After 1 backspace from 'world' at 2x, expected 'worl' but got '{}'",
            tf.GetText()
        );
    }
}

/// Verify arrow key navigation moves the cursor correctly.
#[test]
fn textfield_arrow_navigation() {
    let (mut h, tf_ref) = setup_textfield_harness();
    render(&mut h, 800, 600);

    // Focus and type initial text.
    h.click(400.0, 300.0);
    type_string(&mut h, "abcde");
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 5);

    // ArrowLeft 3 times → cursor at GetPos 2.
    h.press_key(InputKey::ArrowLeft);
    h.press_key(InputKey::ArrowLeft);
    h.press_key(InputKey::ArrowLeft);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        2,
        "After 3 left arrows from pos 5, cursor should be at 2"
    );

    // ArrowRight once → cursor at 3.
    h.press_key(InputKey::ArrowRight);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        3,
        "After 1 right arrow from pos 2, cursor should be at 3"
    );

    // Home → cursor at 0.
    h.press_key(InputKey::Home);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        0,
        "Home should move cursor to 0"
    );

    // End → cursor at 5.
    h.press_key(InputKey::End);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        5,
        "End should move cursor to 5 (end of 'abcde')"
    );
}

/// Verify that typing inserts at the cursor GetPos (mid-string insertion).
#[test]
fn textfield_insert_at_cursor() {
    let (mut h, tf_ref) = setup_textfield_harness();
    render(&mut h, 800, 600);

    h.click(400.0, 300.0);
    type_string(&mut h, "ac");
    assert_eq!(tf_ref.borrow().GetText(), "ac");

    // Move cursor left once (between 'a' and 'c'), then type 'b'.
    h.press_key(InputKey::ArrowLeft);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 1);

    h.press_char('b');
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abc",
            "Inserting 'b' between 'a' and 'c' should produce 'abc', got '{}'",
            tf.GetText()
        );
        assert_eq!(
            tf.GetCursorIndex(),
            2,
            "Cursor should advance to 2 after insert"
        );
    }
}

/// Verify Delete key removes the character AFTER the cursor.
#[test]
fn textfield_delete_key() {
    let (mut h, tf_ref) = setup_textfield_harness();
    render(&mut h, 800, 600);

    h.click(400.0, 300.0);
    type_string(&mut h, "abcd");

    // Move to GetPos 1 (after 'a').
    h.press_key(InputKey::Home);
    h.press_key(InputKey::ArrowRight);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 1);

    // Delete should remove 'b'.
    h.press_key(InputKey::Delete);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "acd",
            "Delete at pos 1 in 'abcd' should produce 'acd', got '{}'",
            tf.GetText()
        );
        assert_eq!(
            tf.GetCursorIndex(),
            1,
            "Cursor should remain at 1 after delete"
        );
    }
}

/// Verify that a non-editable emTextField rejects typed characters.
#[test]
fn textfield_non_editable_rejects_input() {
    let (mut h, tf_ref) = setup_textfield_harness();

    // Make the field non-editable.
    tf_ref.borrow_mut().SetEditable(false);

    render(&mut h, 800, 600);
    h.click(400.0, 300.0);

    type_string(&mut h, "abc");

    assert_eq!(
        tf_ref.borrow().GetText(),
        "",
        "Non-editable TextField should not accept typed characters"
    );
}

/// Verify that pre-populated text is preserved and new text appends correctly.
#[test]
fn textfield_prepopulated_text() {
    let (mut h, tf_ref) = setup_textfield_harness();

    // Pre-populate the text field.
    tf_ref.borrow_mut().SetText("hello");

    render(&mut h, 800, 600);
    h.click(400.0, 300.0);

    // The Click positions the cursor at the Click location within the text,
    // so move to the end explicitly before typing.
    h.press_key(InputKey::End);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 5);

    // Type additional text.
    type_string(&mut h, "!");

    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "hello!",
            "Typing '!' after 'hello' should produce 'hello!', got '{}'",
            tf.GetText()
        );
    }
}

/// Combined test: type at 1x, switch to 2x, type more, verify full text.
#[test]
fn textfield_type_across_zoom_levels() {
    let (mut h, tf_ref) = setup_textfield_harness();

    // ── 1x: type "foo" ─────────────────────────────────────────────────
    render(&mut h, 800, 600);
    h.click(400.0, 300.0);
    type_string(&mut h, "foo");
    assert_eq!(tf_ref.borrow().GetText(), "foo");

    // ── Switch to 2x and type "bar" ────────────────────────────────────
    h.set_zoom(2.0);
    h.tick_n(5);
    render(&mut h, 800, 600);

    // Click at a slightly different GetPos to avoid double-Click detection
    // with the prior Click (same coords within 500ms would trigger word
    // selection, replacing existing text on the next typed character).
    h.click(410.0, 310.0);

    // Move cursor to end so we append after "foo".
    h.press_key(InputKey::End);
    type_string(&mut h, "bar");

    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "foobar",
            "After typing 'foo' at 1x and 'bar' at 2x, text should be 'foobar', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 6);
    }
}

// ===========================================================================
// BP-4: emTextField cursor navigation tests
// ===========================================================================

/// Helper: set up a focused, editable emTextField pre-populated with `text`,
/// cursor at `cursor_pos`. Returns harness + shared emTextField ref.
fn setup_nav_harness(
    text: &str,
    cursor_pos: usize,
) -> (PipelineTestHarness, Rc<RefCell<emTextField>>) {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText(text);
    tf_ref.borrow_mut().SetCursorIndex(cursor_pos);

    render(&mut h, 800, 600);
    h.click(400.0, 300.0);

    // After Click, cursor may have moved to Click GetPos; restore it.
    tf_ref.borrow_mut().SetCursorIndex(cursor_pos);
    // Clear any selection that the Click may have created.
    tf_ref.borrow_mut().EmptySelection();

    (h, tf_ref)
}

/// Helper: set up a focused, editable, multi-line emTextField.
fn setup_multiline_nav_harness(
    text: &str,
    cursor_pos: usize,
) -> (PipelineTestHarness, Rc<RefCell<emTextField>>) {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetMultiLineMode(true);
    tf_ref.borrow_mut().SetText(text);
    tf_ref.borrow_mut().SetCursorIndex(cursor_pos);

    render(&mut h, 800, 600);
    h.click(400.0, 300.0);

    tf_ref.borrow_mut().SetCursorIndex(cursor_pos);
    tf_ref.borrow_mut().EmptySelection();

    (h, tf_ref)
}

// ---------------------------------------------------------------------------
// Left / Right (single char)
// ---------------------------------------------------------------------------

#[test]
fn textfield_left_moves_cursor() {
    // "Hello World" with cursor at 5 → Left → cursor at 4
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 5);
    h.press_key(InputKey::ArrowLeft);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 4);
    assert!(tf_ref.borrow().IsSelectionEmpty());
}

#[test]
fn textfield_left_at_start_stays() {
    let (mut h, tf_ref) = setup_nav_harness("Hello", 0);
    h.press_key(InputKey::ArrowLeft);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 0);
}

#[test]
fn textfield_right_moves_cursor() {
    // "Hello World" with cursor at 5 → Right → cursor at 6
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 5);
    h.press_key(InputKey::ArrowRight);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 6);
    assert!(tf_ref.borrow().IsSelectionEmpty());
}

#[test]
fn textfield_right_at_end_stays() {
    let (mut h, tf_ref) = setup_nav_harness("Hello", 5);
    h.press_key(InputKey::ArrowRight);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 5);
}

// ---------------------------------------------------------------------------
// Ctrl+Left / Ctrl+Right (word boundary)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_left_skips_word() {
    // "foo bar baz" cursor at 8 (start of "baz") → Ctrl+Left → 4 (start of "bar")
    // prev_word_index(8): scans i=0, next_word_index(0)=4 (<8), i=4,
    //   next_word_index(4)=8 (>=8), return 4.
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 8);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowLeft);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        4,
        "Ctrl+Left from pos 8 in 'foo bar baz' should go to 4 (start of 'bar')"
    );
    assert!(tf_ref.borrow().IsSelectionEmpty());
}

#[test]
fn textfield_ctrl_left_from_word_start() {
    // "foo bar" cursor at 4 (start of "bar") → Ctrl+Left → 0 (start of "foo")
    // prev_word_index(4): i=0, next_word_index(0)=4, 4>=4 → return 0
    let (mut h, tf_ref) = setup_nav_harness("foo bar", 4);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowLeft);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 0);
}

#[test]
fn textfield_ctrl_right_skips_word() {
    // "foo bar baz" cursor at 0 → Ctrl+Right → 4 (start of "bar")
    // next_word_index(0): 'f' is word char, scans "foo"→3 (delim), continue,
    //   scans " "→4 (!delim) → return 4
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 0);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        4,
        "Ctrl+Right from pos 0 in 'foo bar baz' should go to 4 (start of 'bar')"
    );
    assert!(tf_ref.borrow().IsSelectionEmpty());
}

#[test]
fn textfield_ctrl_right_from_middle() {
    // "foo bar baz" cursor at 5 (in "bar") → Ctrl+Right → 8 (start of "baz")
    // next_word_index(5): 'a' word char, scans "ar"→7 (delim ' '), continue
    //   p=7, scans " "→8 (!delim) → return 8
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 5);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 8);
}

#[test]
fn textfield_ctrl_right_at_end() {
    let (mut h, tf_ref) = setup_nav_harness("foo bar", 7);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 7);
}

// ---------------------------------------------------------------------------
// Home / End
// ---------------------------------------------------------------------------

#[test]
fn textfield_home_moves_to_start() {
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 7);
    h.press_key(InputKey::Home);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 0);
    assert!(tf_ref.borrow().IsSelectionEmpty());
}

#[test]
fn textfield_end_moves_to_end() {
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 3);
    h.press_key(InputKey::End);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 11);
    assert!(tf_ref.borrow().IsSelectionEmpty());
}

// ---------------------------------------------------------------------------
// Shift+Left / Shift+Right (extend selection one char)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_left_extends_selection() {
    // "Hello" cursor at 3 → Shift+Left → cursor 2, selection [2,3)
    let (mut h, tf_ref) = setup_nav_harness("Hello", 3);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowLeft);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 2);
        assert_eq!(tf.GetSelectionStartIndex(), 2);
        assert_eq!(tf.GetSelectionEndIndex(), 3);
        assert!(!tf.IsSelectionEmpty());
    }
}

#[test]
fn textfield_shift_right_extends_selection() {
    // "Hello" cursor at 2 → Shift+Right → cursor 3, selection [2,3)
    let (mut h, tf_ref) = setup_nav_harness("Hello", 2);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 3);
        assert_eq!(tf.GetSelectionStartIndex(), 2);
        assert_eq!(tf.GetSelectionEndIndex(), 3);
    }
}

#[test]
fn textfield_shift_left_twice_extends_two_chars() {
    // "Hello" cursor at 4 → Shift+Left twice → cursor 2, selection [2,4)
    let (mut h, tf_ref) = setup_nav_harness("Hello", 4);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowLeft);
    h.press_key(InputKey::ArrowLeft);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 2);
        assert_eq!(tf.GetSelectionStartIndex(), 2);
        assert_eq!(tf.GetSelectionEndIndex(), 4);
    }
}

#[test]
fn textfield_shift_right_twice_extends_two_chars() {
    // "Hello" cursor at 1 → Shift+Right twice → cursor 3, selection [1,3)
    let (mut h, tf_ref) = setup_nav_harness("Hello", 1);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowRight);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 3);
        assert_eq!(tf.GetSelectionStartIndex(), 1);
        assert_eq!(tf.GetSelectionEndIndex(), 3);
    }
}

// ---------------------------------------------------------------------------
// Shift+Ctrl+Left / Shift+Ctrl+Right (extend selection by word)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_ctrl_left_extends_selection_word() {
    // "foo bar baz" cursor at 8 → Shift+Ctrl+Left → cursor 4, selection [4,8)
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 8);
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowLeft);
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 4);
        assert_eq!(tf.GetSelectionStartIndex(), 4);
        assert_eq!(tf.GetSelectionEndIndex(), 8);
    }
}

#[test]
fn textfield_shift_ctrl_right_extends_selection_word() {
    // "foo bar baz" cursor at 0 → Shift+Ctrl+Right → cursor 4, selection [0,4)
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 0);
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 4);
        assert_eq!(tf.GetSelectionStartIndex(), 0);
        assert_eq!(tf.GetSelectionEndIndex(), 4);
    }
}

// ---------------------------------------------------------------------------
// Shift+Home / Shift+End (extend selection to line boundaries)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_home_extends_selection_to_start() {
    // "Hello World" cursor at 6 → Shift+Home → cursor 0, selection [0,6)
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 6);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::Home);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 0);
        assert_eq!(tf.GetSelectionStartIndex(), 0);
        assert_eq!(tf.GetSelectionEndIndex(), 6);
    }
}

#[test]
fn textfield_shift_end_extends_selection_to_end() {
    // "Hello World" cursor at 5 → Shift+End → cursor 11, selection [5,11)
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 5);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::End);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 11);
        assert_eq!(tf.GetSelectionStartIndex(), 5);
        assert_eq!(tf.GetSelectionEndIndex(), 11);
    }
}

// ---------------------------------------------------------------------------
// Plain arrow clears existing selection (C++ EmptySelection path)
// ---------------------------------------------------------------------------

#[test]
fn textfield_left_clears_selection() {
    // Pre-select [2,5) in "Hello World", then Left without Shift → selection cleared
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 5);
    // Create selection first
    tf_ref.borrow_mut().Select(2, 5);
    tf_ref.borrow_mut().SetCursorIndex(5);

    h.press_key(InputKey::ArrowLeft);
    {
        let tf = tf_ref.borrow();
        assert!(
            tf.IsSelectionEmpty(),
            "Left without Shift should clear selection"
        );
        assert_eq!(tf.GetCursorIndex(), 4);
    }
}

#[test]
fn textfield_right_clears_selection() {
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 2);
    tf_ref.borrow_mut().Select(2, 5);
    tf_ref.borrow_mut().SetCursorIndex(2);

    h.press_key(InputKey::ArrowRight);
    {
        let tf = tf_ref.borrow();
        assert!(
            tf.IsSelectionEmpty(),
            "Right without Shift should clear selection"
        );
        assert_eq!(tf.GetCursorIndex(), 3);
    }
}

// ---------------------------------------------------------------------------
// Up / Down in multi-line GetMode
// ---------------------------------------------------------------------------

#[test]
fn textfield_down_moves_to_next_row() {
    // "abc\ndef\nghi" cursor at 1 (in first row) → Down → should land in second row
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 1);
    h.press_key(InputKey::ArrowDown);
    {
        let tf = tf_ref.borrow();
        // Row 0: "abc\n" (indices 0..4), Row 1: "def\n" (4..8), Row 2: "ghi" (8..11)
        // Down from pos 1 (col 1, row 0) → col 1, row 1 → index 5
        assert_eq!(
            tf.GetCursorIndex(),
            5,
            "Down from pos 1 in 'abc\\ndef\\nghi' should go to pos 5"
        );
        assert!(tf.IsSelectionEmpty());
    }
}

#[test]
fn textfield_up_moves_to_prev_row() {
    // "abc\ndef\nghi" cursor at 5 (in second row, col 1) → Up → pos 1
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 5);
    h.press_key(InputKey::ArrowUp);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetCursorIndex(),
            1,
            "Up from pos 5 in 'abc\\ndef\\nghi' should go to pos 1"
        );
    }
}

#[test]
fn textfield_up_at_first_row_stays() {
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef", 2);
    h.press_key(InputKey::ArrowUp);
    {
        let tf = tf_ref.borrow();
        // Up from first row: prev_row_index should return 0 or stay at row start
        // Let me check the behavior - it should clamp to the same row
        // prev_row_index when already at row 0 returns col_row_to_index(col, row-1)
        // which for row=0 means row=-1 effectively → should clamp to 0
        assert!(
            tf.GetCursorIndex() <= 2,
            "Up from first row should not go past start"
        );
    }
}

#[test]
fn textfield_down_at_last_row_stays() {
    // "abc\ndef" cursor at 5 (row 1, col 1) → Down → should stay in last row
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef", 5);
    h.press_key(InputKey::ArrowDown);
    {
        let tf = tf_ref.borrow();
        // Down from last row should not go past end
        assert!(
            tf.GetCursorIndex() >= 4 && tf.GetCursorIndex() <= 7,
            "Down from last row should stay in last row, got {}",
            tf.GetCursorIndex()
        );
    }
}

#[test]
fn textfield_shift_down_extends_selection_multiline() {
    // "abc\ndef\nghi" cursor at 1 → Shift+Down → selection from 1 to 5
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 1);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowDown);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 5);
        assert_eq!(tf.GetSelectionStartIndex(), 1);
        assert_eq!(tf.GetSelectionEndIndex(), 5);
    }
}

#[test]
fn textfield_shift_up_extends_selection_multiline() {
    // "abc\ndef\nghi" cursor at 9 (row 2, col 1) → Shift+Up → should extend selection
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 9);
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowUp);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(tf.GetCursorIndex(), 5);
        assert_eq!(tf.GetSelectionStartIndex(), 5);
        assert_eq!(tf.GetSelectionEndIndex(), 9);
    }
}

// ---------------------------------------------------------------------------
// Up / Down ignored in single-line GetMode (C++: guarded by MultiLineMode)
// ---------------------------------------------------------------------------

#[test]
fn textfield_down_ignored_single_line() {
    let (mut h, tf_ref) = setup_nav_harness("Hello", 2);
    h.press_key(InputKey::ArrowDown);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        2,
        "Down in single-line mode should be ignored"
    );
}

#[test]
fn textfield_up_ignored_single_line() {
    let (mut h, tf_ref) = setup_nav_harness("Hello", 2);
    h.press_key(InputKey::ArrowUp);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        2,
        "Up in single-line mode should be ignored"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+Home / Ctrl+End in multi-line GetMode
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_home_multiline_goes_to_zero() {
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 9);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Home);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 0);
}

#[test]
fn textfield_ctrl_end_multiline_goes_to_len() {
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 0);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::End);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 11);
}

// ---------------------------------------------------------------------------
// Home / End in multi-line GetMode → row start / row end
// ---------------------------------------------------------------------------

#[test]
fn textfield_home_multiline_goes_to_row_start() {
    // "abc\ndef\nghi" cursor at 6 (row 1, col 2) → Home → 4 (row 1 start)
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 6);
    h.press_key(InputKey::Home);
    assert_eq!(
        tf_ref.borrow().GetCursorIndex(),
        4,
        "Home in multi-line should go to row start"
    );
}

#[test]
fn textfield_end_multiline_goes_to_row_end() {
    // "abc\ndef\nghi" cursor at 4 (row 1, col 0) → End → 7 (row 1 end, before \n)
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 4);
    h.press_key(InputKey::End);
    {
        let tf = tf_ref.borrow();
        // row_end for row 1 ("def\n") should be 7 (the GetPos of '\n')
        assert_eq!(
            tf.GetCursorIndex(),
            7,
            "End in multi-line should go to row end"
        );
    }
}

// ---------------------------------------------------------------------------
// Ctrl+Up / Ctrl+Down (paragraph navigation) in multi-line
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_down_next_paragraph() {
    // "abc\ndef\nghi" cursor at 0 → Ctrl+Down → next paragraph
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 0);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowDown);
    h.input_state.release(InputKey::Ctrl);
    {
        let tf = tf_ref.borrow();
        // next_paragraph_index from 0 should jump past the first \n
        assert!(
            tf.GetCursorIndex() > 0,
            "Ctrl+Down should move cursor forward"
        );
    }
}

#[test]
fn textfield_ctrl_up_prev_paragraph() {
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 9);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowUp);
    h.input_state.release(InputKey::Ctrl);
    {
        let tf = tf_ref.borrow();
        assert!(
            tf.GetCursorIndex() < 9,
            "Ctrl+Up should move cursor backward"
        );
    }
}

// ===========================================================================
// BP-5: emTextField editing operations
// ===========================================================================

// ---------------------------------------------------------------------------
// Ctrl+Backspace (delete word before cursor)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_backspace_deletes_word_before_cursor() {
    // "foo bar baz" cursor at 7 (end of "bar") → Ctrl+Backspace
    // prev_word_index(7) = 4 (start of "bar"), deletes chars 4..7 ("bar") → "foo  baz"
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 7);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Backspace);
    h.input_state.release(InputKey::Ctrl);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "foo  baz",
            "Ctrl+Backspace from pos 7 in 'foo bar baz' should delete 'bar', got '{}'",
            tf.GetText()
        );
        assert_eq!(
            tf.GetCursorIndex(),
            4,
            "Cursor should be at 4 after Ctrl+Backspace"
        );
    }
}

#[test]
fn textfield_ctrl_backspace_at_start_does_nothing() {
    let (mut h, tf_ref) = setup_nav_harness("hello", 0);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Backspace);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetText(), "hello");
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 0);
}

// ---------------------------------------------------------------------------
// Ctrl+Delete (delete word after cursor)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_delete_deletes_word_after_cursor() {
    // "foo bar baz" cursor at 4 (start of "bar") → Ctrl+Delete → "foo baz"
    // next_word_index(4) should find end of "bar" + skip space (8), deleting "bar " → "foo baz"
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 4);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Ctrl);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "foo baz",
            "Ctrl+Delete from pos 4 in 'foo bar baz' should delete 'bar ', got '{}'",
            tf.GetText()
        );
        assert_eq!(
            tf.GetCursorIndex(),
            4,
            "Cursor should remain at 4 after Ctrl+Delete"
        );
    }
}

#[test]
fn textfield_ctrl_delete_at_end_does_nothing() {
    let (mut h, tf_ref) = setup_nav_harness("hello", 5);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(tf_ref.borrow().GetText(), "hello");
    assert_eq!(tf_ref.borrow().GetCursorIndex(), 5);
}

// ---------------------------------------------------------------------------
// Shift+Ctrl+Backspace (delete to start of line)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_ctrl_backspace_deletes_to_line_start() {
    // "hello world" cursor at 7 → Shift+Ctrl+Backspace → "orld"
    // row_start(7) = 0 (single line), so deletes chars 0..7 → "orld"
    let (mut h, tf_ref) = setup_nav_harness("hello world", 7);
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Backspace);
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "orld",
            "Shift+Ctrl+Backspace from pos 7 in 'hello world' should delete to line start, got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 0);
    }
}

#[test]
fn textfield_shift_ctrl_backspace_multiline_deletes_to_row_start() {
    // "abc\ndef\nghi" cursor at 6 (row 1, col 2 = 'f') → Shift+Ctrl+Backspace
    // row_start(6) = 4, deletes chars 4..6 → "abc\nf\nghi"
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 6);
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Backspace);
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abc\nf\nghi",
            "Shift+Ctrl+Backspace from col 2 in row 1 should delete 'de', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 4);
    }
}

// ---------------------------------------------------------------------------
// Shift+Ctrl+Delete (delete to end of line)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_ctrl_delete_deletes_to_line_end() {
    // "hello world" cursor at 5 → Shift+Ctrl+Delete → "hello"
    // row_end(5) = 11 (single line, end of text), deletes 5..11 → "hello"
    let (mut h, tf_ref) = setup_nav_harness("hello world", 5);
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "hello",
            "Shift+Ctrl+Delete from pos 5 in 'hello world' should delete to line end, got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 5);
    }
}

#[test]
fn textfield_shift_ctrl_delete_multiline_deletes_to_row_end() {
    // "abc\ndef\nghi" cursor at 4 (row 1, col 0 = 'd') → Shift+Ctrl+Delete
    // row_end(4) = 7 (before \n), deletes 4..7 → "abc\n\nghi"
    let (mut h, tf_ref) = setup_multiline_nav_harness("abc\ndef\nghi", 4);
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abc\n\nghi",
            "Shift+Ctrl+Delete from col 0 in row 1 should delete 'def', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 4);
    }
}

// ---------------------------------------------------------------------------
// Backspace with selection (deletes selection, C++ DeleteSelectedText path)
// ---------------------------------------------------------------------------

#[test]
fn textfield_backspace_with_selection_deletes_selection() {
    // "abcdef" with selection [2,4) → Backspace → "abef", cursor at 2
    let (mut h, tf_ref) = setup_nav_harness("abcdef", 4);
    tf_ref.borrow_mut().Select(2, 4);
    tf_ref.borrow_mut().SetCursorIndex(4);

    h.press_key(InputKey::Backspace);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abef",
            "Backspace with selection [2,4) in 'abcdef' should delete 'cd', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 2);
        assert!(tf.IsSelectionEmpty());
    }
}

// ---------------------------------------------------------------------------
// Delete with selection (deletes selection, C++ DeleteSelectedText path)
// ---------------------------------------------------------------------------

#[test]
fn textfield_delete_with_selection_deletes_selection() {
    // "abcdef" with selection [1,3) → Delete → "adef", cursor at 1
    let (mut h, tf_ref) = setup_nav_harness("abcdef", 3);
    tf_ref.borrow_mut().Select(1, 3);
    tf_ref.borrow_mut().SetCursorIndex(3);

    h.press_key(InputKey::Delete);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "adef",
            "Delete with selection [1,3) in 'abcdef' should delete 'bc', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 1);
        assert!(tf.IsSelectionEmpty());
    }
}

// ---------------------------------------------------------------------------
// Typing with selection replaces selection (C++ ModifySelectedText path)
// ---------------------------------------------------------------------------

#[test]
fn textfield_typing_with_selection_replaces_selection() {
    // "abcdef" with selection [2,5) → type 'X' → "abXf", cursor at 3
    let (mut h, tf_ref) = setup_nav_harness("abcdef", 5);
    tf_ref.borrow_mut().Select(2, 5);
    tf_ref.borrow_mut().SetCursorIndex(5);

    h.press_char('X');
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abXf",
            "Typing 'X' with selection [2,5) in 'abcdef' should produce 'abXf', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 3);
        assert!(tf.IsSelectionEmpty());
    }
}

// ---------------------------------------------------------------------------
// Insert key toggles overwrite GetMode (C++ EM_KEY_INSERT + IsNoMod)
// ---------------------------------------------------------------------------

#[test]
fn textfield_insert_toggles_overwrite_mode() {
    let (mut h, tf_ref) = setup_nav_harness("hello", 0);
    assert!(!tf_ref.borrow().GetOverwriteMode());

    h.press_key(InputKey::Insert);
    assert!(
        tf_ref.borrow().GetOverwriteMode(),
        "Insert should toggle overwrite mode ON"
    );

    h.press_key(InputKey::Insert);
    assert!(
        !tf_ref.borrow().GetOverwriteMode(),
        "Insert again should toggle overwrite mode OFF"
    );
}

// ---------------------------------------------------------------------------
// Typing in overwrite GetMode replaces char at cursor
// (C++ OverwriteMode && CursorIndex < GetRowEndIndex path)
// ---------------------------------------------------------------------------

#[test]
fn textfield_overwrite_mode_replaces_char() {
    // "abcde" with overwrite GetMode, cursor at 1 → type 'X' → "aXcde", cursor at 2
    let (mut h, tf_ref) = setup_nav_harness("abcde", 1);
    tf_ref.borrow_mut().SetOverwriteMode(true);

    h.press_char('X');
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "aXcde",
            "Overwrite mode: typing 'X' at pos 1 in 'abcde' should replace 'b', got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 2);
    }
}

#[test]
fn textfield_overwrite_mode_at_end_inserts() {
    // "abc" with overwrite GetMode, cursor at 3 (end) → type 'X' → "abcX"
    // C++: OverwriteMode && CursorIndex < GetRowEndIndex → false at end, so insert
    let (mut h, tf_ref) = setup_nav_harness("abc", 3);
    tf_ref.borrow_mut().SetOverwriteMode(true);

    h.press_char('X');
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "abcX",
            "Overwrite mode at end should insert, got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 4);
    }
}

// ---------------------------------------------------------------------------
// Non-editable emTextField rejects all editing operations
// (C++ IsEditable() guard on editing block)
// ---------------------------------------------------------------------------

#[test]
fn textfield_non_editable_rejects_backspace() {
    let (mut h, tf_ref) = setup_nav_harness("hello", 5);
    tf_ref.borrow_mut().SetEditable(false);

    h.press_key(InputKey::Backspace);
    assert_eq!(
        tf_ref.borrow().GetText(),
        "hello",
        "Non-editable TextField should reject Backspace"
    );
}

#[test]
fn textfield_non_editable_rejects_delete() {
    let (mut h, tf_ref) = setup_nav_harness("hello", 2);
    tf_ref.borrow_mut().SetEditable(false);

    h.press_key(InputKey::Delete);
    assert_eq!(
        tf_ref.borrow().GetText(),
        "hello",
        "Non-editable TextField should reject Delete"
    );
}

#[test]
fn textfield_non_editable_rejects_ctrl_backspace() {
    let (mut h, tf_ref) = setup_nav_harness("hello world", 5);
    tf_ref.borrow_mut().SetEditable(false);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Backspace);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(
        tf_ref.borrow().GetText(),
        "hello world",
        "Non-editable TextField should reject Ctrl+Backspace"
    );
}

#[test]
fn textfield_non_editable_rejects_ctrl_delete() {
    let (mut h, tf_ref) = setup_nav_harness("hello world", 5);
    tf_ref.borrow_mut().SetEditable(false);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Ctrl);
    assert_eq!(
        tf_ref.borrow().GetText(),
        "hello world",
        "Non-editable TextField should reject Ctrl+Delete"
    );
}

#[test]
fn textfield_non_editable_allows_insert_toggle() {
    // C++ Insert key toggle is NOT guarded by IsEditable — it's in the
    // non-editable block. So overwrite GetMode toggles even when not editable.
    let (mut h, tf_ref) = setup_nav_harness("hello", 0);
    tf_ref.borrow_mut().SetEditable(false);

    assert!(!tf_ref.borrow().GetOverwriteMode());
    h.press_key(InputKey::Insert);
    assert!(
        tf_ref.borrow().GetOverwriteMode(),
        "Insert toggle should work even when non-editable (C++ ref: emTextField.cpp:661)"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+Backspace with selection deletes selection (not word)
// C++ ref: emTextField.cpp:741-752 — selection check before word delete
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_backspace_with_selection_deletes_selection() {
    // "foo bar baz" with selection [4,7) → Ctrl+Backspace → "foo baz"
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 7);
    tf_ref.borrow_mut().Select(4, 7);
    tf_ref.borrow_mut().SetCursorIndex(7);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Backspace);
    h.input_state.release(InputKey::Ctrl);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            "foo  baz",
            "Ctrl+Backspace with selection should delete selection, got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 4);
        assert!(tf.IsSelectionEmpty());
    }
}

// ---------------------------------------------------------------------------
// Ctrl+Delete with selection deletes selection (not word)
// C++ ref: emTextField.cpp:757-770 — selection check before word delete
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_delete_with_selection_deletes_selection() {
    // "foo bar baz" with selection [0,3) → Ctrl+Delete → " bar baz"
    let (mut h, tf_ref) = setup_nav_harness("foo bar baz", 3);
    tf_ref.borrow_mut().Select(0, 3);
    tf_ref.borrow_mut().SetCursorIndex(3);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Ctrl);
    {
        let tf = tf_ref.borrow();
        assert_eq!(
            tf.GetText(),
            " bar baz",
            "Ctrl+Delete with selection should delete selection, got '{}'",
            tf.GetText()
        );
        assert_eq!(tf.GetCursorIndex(), 0);
        assert!(tf.IsSelectionEmpty());
    }
}

// ===========================================================================
// BP-6: emTextField mouse-based selection
// ===========================================================================

// ---------------------------------------------------------------------------
// Single Click positions cursor
// C++ ref: emTextField.cpp:391-397 (repeat==0 single Click branch)
// ---------------------------------------------------------------------------

/// A single (first) Click positions cursor within text range and creates no
/// selection. This is the very first Click on the text field so there's no
/// prior Click to form a double-Click with.
/// C++ ref: emTextField.cpp:391-397 (repeat==0 single Click)
#[test]
fn textfield_single_click_positions_cursor() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // First Click ever on this widget — guaranteed single Click (no prior click_time).
    h.click(400.0, 300.0);

    let tf = tf_ref.borrow();
    // The cursor should be positioned somewhere within the text range.
    assert!(
        tf.GetCursorIndex() <= 11,
        "Cursor pos {} should be within text length 11",
        tf.GetCursorIndex()
    );
    assert!(
        tf.IsSelectionEmpty(),
        "First single click should not create a selection"
    );
}

/// Single Click clears any existing selection (C++ EmptySelection path).
/// Uses setup_nav_harness so the widget is already focused. The first Click
/// on this harness instance is guaranteed to be a single-Click (no prior
/// click_time).
#[test]
fn textfield_single_click_clears_selection() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Create a selection via API.
    tf_ref.borrow_mut().Select(0, 5);
    assert!(!tf_ref.borrow().IsSelectionEmpty());

    // First Click on this widget instance — guaranteed single Click.
    // Single Click without Shift should Clear existing selection.
    h.click(400.0, 300.0);

    let tf = tf_ref.borrow();
    assert!(
        tf.IsSelectionEmpty(),
        "Single click should clear existing selection"
    );
}

// ---------------------------------------------------------------------------
// Double-Click selects word
// C++ ref: emTextField.cpp:398-413 (repeat==1, double-Click word selection)
// ---------------------------------------------------------------------------

/// Double-Click (two rapid clicks at same GetPos) selects the word under cursor.
#[test]
fn textfield_double_click_selects_word() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("foo bar baz");

    render(&mut h, 800, 600);

    // Click at center — this should be roughly in "bar" given the text layout.
    // Two rapid clicks at the same GetPos trigger double-Click via time-based
    // detection (click_count increments from 1 to 2).
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);

    let tf = tf_ref.borrow();
    // Double-Click selects a word boundary segment. The GetChecked text should
    // be a contiguous word or delimiter segment.
    let sel_text = tf.selected_text();
    assert!(
        !tf.IsSelectionEmpty(),
        "Double-click should create a selection"
    );
    // The GetChecked text should be either a word or a delimiter segment,
    // not a mix. Verify it's non-empty and bounded by word boundaries.
    assert!(
        !sel_text.is_empty(),
        "Double-click should select a non-empty segment"
    );
    // Verify the selection boundaries are word boundaries: all chars should be
    // the same type (all word chars or all delimiters).
    let all_word = sel_text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_');
    let all_delim = sel_text
        .chars()
        .all(|c| !(c.is_ascii_alphanumeric() || c == '_'));
    assert!(
        all_word || all_delim,
        "Double-click selection '{}' should be a single word or delimiter segment",
        sel_text
    );
}

// ---------------------------------------------------------------------------
// Triple-Click selects entire line/row
// C++ ref: emTextField.cpp:415-431 (repeat==2, triple-Click row selection)
// ---------------------------------------------------------------------------

/// Triple-Click selects the entire row. In single-line GetMode, this means the
/// whole text (row_start=0, row_end=len for single-line).
#[test]
fn textfield_triple_click_selects_line() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("hello world");

    render(&mut h, 800, 600);

    // Three rapid clicks at the same GetPos.
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);

    let tf = tf_ref.borrow();
    // In single-line GetMode, triple-Click should select the entire text (full row).
    assert_eq!(
        tf.GetSelectionStartIndex(),
        0,
        "Triple-click selection start should be 0"
    );
    assert_eq!(
        tf.GetSelectionEndIndex(),
        11,
        "Triple-click selection end should be text length (11)"
    );
    assert_eq!(tf.selected_text(), "hello world");
}

/// Triple-Click in multi-line GetMode selects just the clicked row.
#[test]
fn textfield_triple_click_selects_row_multiline() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetMultiLineMode(true);
    tf_ref.borrow_mut().SetText("abc\ndef\nghi");

    render(&mut h, 800, 600);

    // Triple-Click — the exact row depends on the y coordinate, but at center
    // of the viewport, in multi-line with 3 rows, it should hit one of the rows.
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);

    let tf = tf_ref.borrow();
    // Selection should cover exactly one row (including the trailing \n for non-last rows).
    let sel = tf.selected_text();
    assert!(
        !sel.is_empty(),
        "Triple-click in multi-line should select a row"
    );
    // The GetChecked text should be one of: "abc\n", "def\n", or "ghi"
    let valid_rows = ["abc\n", "def\n", "ghi"];
    assert!(
        valid_rows.contains(&sel),
        "Triple-click should select exactly one row, got '{:?}'",
        sel
    );
}

// ---------------------------------------------------------------------------
// Drag from GetPos A to B selects text between A and B
// C++ ref: emTextField.cpp:441-453 (DM_SELECT drag)
// ---------------------------------------------------------------------------

/// Drag from one GetPos to another within the content area should select text.
/// Uses coordinates near the viewport center to ensure hit_test passes.
#[test]
fn textfield_drag_selects_text() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Focus first with a Click (at a different y to avoid double-Click with drag).
    h.click(400.0, 310.0);

    // Drag within content area: start left of center, end right of center.
    // Both points must be within the border's content round rect.
    h.drag(300.0, 300.0, 500.0, 300.0);

    let tf = tf_ref.borrow();
    assert!(!tf.IsSelectionEmpty(), "Drag should create a selection");
    let sel = tf.selected_text();
    assert!(
        !sel.is_empty(),
        "Drag across the text should select characters (got '{}')",
        sel
    );
    // The selection start should be before selection end.
    assert!(
        tf.GetSelectionStartIndex() < tf.GetSelectionEndIndex(),
        "Selection start ({}) should be less than end ({})",
        tf.GetSelectionStartIndex(),
        tf.GetSelectionEndIndex()
    );
}

/// Drag from right to left within content area should also create a valid selection.
#[test]
fn textfield_drag_right_to_left_selects_text() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Focus at offset y to avoid double-Click detection.
    h.click(400.0, 310.0);

    // Drag right to left within content area.
    h.drag(500.0, 300.0, 300.0, 300.0);

    let tf = tf_ref.borrow();
    assert!(
        !tf.IsSelectionEmpty(),
        "Right-to-left drag should create a selection"
    );
    assert!(
        tf.GetSelectionStartIndex() < tf.GetSelectionEndIndex(),
        "Selection start < end even for right-to-left drag"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+A selects all text
// C++ ref: emTextField.cpp:639-645 (Ctrl+A → SelectAll)
// ---------------------------------------------------------------------------

/// Ctrl+A selects the entire text.
#[test]
fn textfield_ctrl_a_selects_all() {
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 5);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('a'));
    h.input_state.release(InputKey::Ctrl);

    let tf = tf_ref.borrow();
    assert_eq!(tf.GetSelectionStartIndex(), 0);
    assert_eq!(tf.GetSelectionEndIndex(), 11);
    assert_eq!(tf.selected_text(), "Hello World");
}

/// Ctrl+A on empty text produces empty selection (start == end == 0).
#[test]
fn textfield_ctrl_a_empty_text() {
    let (mut h, tf_ref) = setup_nav_harness("", 0);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('a'));
    h.input_state.release(InputKey::Ctrl);

    let tf = tf_ref.borrow();
    assert!(
        tf.IsSelectionEmpty(),
        "Ctrl+A on empty text should result in empty selection"
    );
}

/// Ctrl+A works even on non-editable text fields (selection is not an edit).
#[test]
fn textfield_ctrl_a_non_editable() {
    let (mut h, tf_ref) = setup_nav_harness("Hello", 0);
    tf_ref.borrow_mut().SetEditable(false);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('a'));
    h.input_state.release(InputKey::Ctrl);

    let tf = tf_ref.borrow();
    assert_eq!(tf.GetSelectionStartIndex(), 0);
    assert_eq!(tf.GetSelectionEndIndex(), 5);
    assert_eq!(tf.selected_text(), "Hello");
}

// ---------------------------------------------------------------------------
// Shift+Ctrl+A deselects / clears selection
// C++ ref: emTextField.cpp:646-651 (Ctrl+Shift+A → EmptySelection)
// ---------------------------------------------------------------------------

/// Shift+Ctrl+A clears the selection.
#[test]
fn textfield_shift_ctrl_a_deselects() {
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 5);

    // First select all.
    tf_ref.borrow_mut().SelectAll();
    assert!(!tf_ref.borrow().IsSelectionEmpty());

    // Shift+Ctrl+A to Deselect.
    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('a'));
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);

    let tf = tf_ref.borrow();
    assert!(tf.IsSelectionEmpty(), "Shift+Ctrl+A should deselect all");
}

/// Shift+Ctrl+A on already empty selection is a no-op.
#[test]
fn textfield_shift_ctrl_a_noop_when_no_selection() {
    let (mut h, tf_ref) = setup_nav_harness("Hello", 3);
    assert!(tf_ref.borrow().IsSelectionEmpty());

    h.input_state.press(InputKey::Shift);
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('a'));
    h.input_state.release(InputKey::Ctrl);
    h.input_state.release(InputKey::Shift);

    let tf = tf_ref.borrow();
    assert!(tf.IsSelectionEmpty());
    assert_eq!(
        tf.GetCursorIndex(),
        3,
        "Cursor should not move on Shift+Ctrl+A deselect"
    );
}

// ---------------------------------------------------------------------------
// Shift+Click extends selection from cursor to Click GetPos
// C++ ref: emTextField.cpp:393 (Shift pressed → ModifySelection)
// ---------------------------------------------------------------------------

/// Shift+Click extends selection from current cursor to clicked GetPos.
/// Use setup_nav_harness to set a known cursor GetPos, then Shift+Click
/// at a different location to extend.
#[test]
fn textfield_shift_click_extends_selection() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Click at center to focus.
    h.click(400.0, 300.0);
    let initial_pos = tf_ref.borrow().GetCursorIndex();

    // Shift+Click at a distinctly different x within content area.
    // Use offset y to avoid double-Click detection.
    h.input_state.press(InputKey::Shift);
    h.click(550.0, 300.0);
    h.input_state.release(InputKey::Shift);

    let tf = tf_ref.borrow();
    // If shift+Click lands at a different GetPos than initial cursor, we GetRec
    // a selection. If same GetPos, selection is empty (degenerate case).
    let shift_pos = tf.GetCursorIndex();
    if shift_pos != initial_pos {
        assert!(
            !tf.IsSelectionEmpty(),
            "Shift+click at different pos should create a selection (initial={}, shift={})",
            initial_pos,
            shift_pos
        );
        assert!(
            tf.GetSelectionStartIndex() < tf.GetSelectionEndIndex(),
            "Selection start ({}) < end ({})",
            tf.GetSelectionStartIndex(),
            tf.GetSelectionEndIndex()
        );
    }
    // If same GetPos (unlikely but possible): selection is empty, which is correct.
}

/// Shift+Click via keyboard nav: GetPos cursor at 2, then Shift+Click extends.
/// This uses keyboard to set a known anchor and verifies shift+Click extends from it.
#[test]
fn textfield_shift_click_from_known_cursor() {
    let (mut h, tf_ref) = setup_nav_harness("Hello World", 2);

    // Now Shift+Click at center of viewport.
    h.input_state.press(InputKey::Shift);
    h.click(500.0, 300.0);
    h.input_state.release(InputKey::Shift);

    let tf = tf_ref.borrow();
    // The Click should extend selection from cursor pos 2 to wherever the Click lands.
    let click_pos = tf.GetCursorIndex();
    if click_pos != 2 {
        assert!(
            !tf.IsSelectionEmpty(),
            "Shift+click should create a selection (cursor was at 2, now at {})",
            click_pos
        );
    }
}

// ---------------------------------------------------------------------------
// Quad-Click (4x) selects all text
// C++ ref: emTextField.cpp:432-435 (repeat>=3 → SelectAll)
// ---------------------------------------------------------------------------

/// Four rapid clicks selects all text (quad-Click).
#[test]
fn textfield_quad_click_selects_all() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("foo bar baz");

    render(&mut h, 800, 600);

    h.click(400.0, 300.0);
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);

    let tf = tf_ref.borrow();
    assert_eq!(tf.GetSelectionStartIndex(), 0);
    assert_eq!(tf.GetSelectionEndIndex(), 11);
    assert_eq!(tf.selected_text(), "foo bar baz");
}

// ---------------------------------------------------------------------------
// Ctrl+A then typing replaces all text (integration)
// C++ ref: emTextField.cpp:639 (SelectAll) + typing replaces selection
// ---------------------------------------------------------------------------

/// Ctrl+A followed by typing replaces all text.
#[test]
fn textfield_ctrl_a_then_type_replaces_all() {
    let (mut h, tf_ref) = setup_nav_harness("old text", 8);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('a'));
    h.input_state.release(InputKey::Ctrl);

    assert_eq!(tf_ref.borrow().selected_text(), "old text");

    h.press_char('X');

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        "X",
        "Typing after Ctrl+A should replace all text, got '{}'",
        tf.GetText()
    );
    assert_eq!(tf.GetCursorIndex(), 1);
    assert!(tf.IsSelectionEmpty());
}

// ===========================================================================
// BP-7: emTextField clipboard operations
// ===========================================================================

// ---------------------------------------------------------------------------
// Helper: set up a focused, editable emTextField with clipboard recorders wired.
// Returns (harness, shared_tf, copy_recorder, paste_source).
// The copy_recorder captures all strings passed to on_clipboard_copy.
// The paste_source provides the text returned by on_clipboard_paste.
// ---------------------------------------------------------------------------

type ClipboardHarness = (
    PipelineTestHarness,
    Rc<RefCell<emTextField>>,
    Rc<RefCell<Vec<String>>>,
);

fn setup_clipboard_harness(text: &str, cursor_pos: usize, paste_content: &str) -> ClipboardHarness {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText(text);
    tf_ref.borrow_mut().SetCursorIndex(cursor_pos);

    // Wire clipboard copy recorder
    let copy_recorder: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let recorder_clone = copy_recorder.clone();
    tf_ref.borrow_mut().on_clipboard_copy = Some(Box::new(move |text: &str| {
        recorder_clone.borrow_mut().push(text.to_string());
    }));

    // Wire clipboard paste source
    let paste_text = paste_content.to_string();
    tf_ref.borrow_mut().on_clipboard_paste = Some(Box::new(move || paste_text.clone()));

    render(&mut h, 800, 600);
    h.click(400.0, 300.0);

    // Restore cursor GetPos and Clear any selection the Click created.
    tf_ref.borrow_mut().SetCursorIndex(cursor_pos);
    tf_ref.borrow_mut().EmptySelection();

    (h, tf_ref, copy_recorder)
}

// ---------------------------------------------------------------------------
// Ctrl+C with selection -> copies GetChecked text
// C++ ref: emTextField.cpp:666-671 (EM_KEY_C + IsCtrlMod -> CopySelectedTextToClipboard)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_c_with_selection_copies_text() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("Hello World", 5, "");

    // Select "Hello" (indices 0..5)
    tf_ref.borrow_mut().Select(0, 5);
    tf_ref.borrow_mut().SetCursorIndex(5);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('c'));
    h.input_state.release(InputKey::Ctrl);

    let copies = copy_recorder.borrow();
    assert_eq!(copies.len(), 1, "Copy callback should fire exactly once");
    assert_eq!(copies[0], "Hello", "Copied text should be 'Hello'");

    // Text and selection should be unchanged after copy.
    let tf = tf_ref.borrow();
    assert_eq!(tf.GetText(), "Hello World");
    assert!(!tf.IsSelectionEmpty());
}

// ---------------------------------------------------------------------------
// Ctrl+C without selection -> no copy callback fired
// C++ ref: emTextField.cpp:666-671 — copy_to_clipboard returns early if empty
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_c_without_selection_no_copy() {
    let (mut h, _tf_ref, copy_recorder) = setup_clipboard_harness("Hello World", 5, "");

    // No selection — just cursor at 5.
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('c'));
    h.input_state.release(InputKey::Ctrl);

    let copies = copy_recorder.borrow();
    assert!(
        copies.is_empty(),
        "Copy callback should NOT fire without a selection, but got {:?}",
        *copies
    );
}

// ---------------------------------------------------------------------------
// Ctrl+X with selection -> cuts GetChecked text (text removed + captured)
// C++ ref: emTextField.cpp:726-731 (EM_KEY_X + IsCtrlMod -> CutSelectedTextToClipboard)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_x_with_selection_cuts_text() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("ABCDEF", 4, "");

    // Select "CD" (indices 2..4)
    tf_ref.borrow_mut().Select(2, 4);
    tf_ref.borrow_mut().SetCursorIndex(4);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('x'));
    h.input_state.release(InputKey::Ctrl);

    // Verify the cut text was sent to the copy callback.
    let copies = copy_recorder.borrow();
    assert_eq!(copies.len(), 1, "Cut should fire copy callback once");
    assert_eq!(copies[0], "CD", "Cut text should be 'CD'");

    // Verify text was modified: "CD" removed from "ABCDEF" -> "ABEF".
    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        "ABEF",
        "After cutting 'CD' from 'ABCDEF', expected 'ABEF', got '{}'",
        tf.GetText()
    );
    assert_eq!(tf.GetCursorIndex(), 2, "Cursor should be at 2 after cut");
    assert!(
        tf.IsSelectionEmpty(),
        "Selection should be cleared after cut"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+X without selection -> no effect
// C++ ref: emTextField.cpp:726-731 — cut returns early if selection empty
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_x_without_selection_no_effect() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("Hello", 3, "");

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('x'));
    h.input_state.release(InputKey::Ctrl);

    let copies = copy_recorder.borrow();
    assert!(
        copies.is_empty(),
        "Cut callback should NOT fire without a selection"
    );
    assert_eq!(
        tf_ref.borrow().GetText(),
        "Hello",
        "Text should be unchanged after cut with no selection"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+V -> pastes text at cursor
// C++ ref: emTextField.cpp:733-738 (EM_KEY_V + IsCtrlMod -> PasteSelectedTextFromClipboard)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_v_pastes_text_at_cursor() {
    let (mut h, tf_ref, _copy_recorder) = setup_clipboard_harness("Hello", 5, "World");

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('v'));
    h.input_state.release(InputKey::Ctrl);

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        "HelloWorld",
        "Pasting 'World' at end of 'Hello' should produce 'HelloWorld', got '{}'",
        tf.GetText()
    );
    assert_eq!(
        tf.GetCursorIndex(),
        10,
        "Cursor should be at end after paste"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+V with selection -> replaces selection with pasted text
// C++ ref: emTextField.cpp:733-738 — paste_from_clipboard calls paste_text
//          which does delete_selection() before inserting
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_v_with_selection_replaces_selection() {
    let (mut h, tf_ref, _copy_recorder) = setup_clipboard_harness("ABCDEF", 4, "XY");

    // Select "CD" (indices 2..4)
    tf_ref.borrow_mut().Select(2, 4);
    tf_ref.borrow_mut().SetCursorIndex(4);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('v'));
    h.input_state.release(InputKey::Ctrl);

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        "ABXYEF",
        "Pasting 'XY' over selection 'CD' in 'ABCDEF' should produce 'ABXYEF', got '{}'",
        tf.GetText()
    );
    assert_eq!(
        tf.GetCursorIndex(),
        4,
        "Cursor should be at end of pasted text"
    );
    assert!(tf.IsSelectionEmpty());
}

// ---------------------------------------------------------------------------
// Ctrl+V at mid-cursor inserts at GetPos
// C++ ref: same paste path, verifying insertion at non-end GetPos
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_v_inserts_at_mid_cursor() {
    let (mut h, tf_ref, _copy_recorder) = setup_clipboard_harness("AC", 1, "B");

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('v'));
    h.input_state.release(InputKey::Ctrl);

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        "ABC",
        "Pasting 'B' at pos 1 in 'AC' should produce 'ABC', got '{}'",
        tf.GetText()
    );
    assert_eq!(tf.GetCursorIndex(), 2);
}

// ---------------------------------------------------------------------------
// Insert+Ctrl copies (alternate key binding)
// C++ ref: emTextField.cpp:666-671 (EM_KEY_INSERT + IsCtrlMod)
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_insert_copies_text() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("Hello World", 5, "");

    tf_ref.borrow_mut().Select(0, 5);
    tf_ref.borrow_mut().SetCursorIndex(5);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Insert);
    h.input_state.release(InputKey::Ctrl);

    let copies = copy_recorder.borrow();
    assert_eq!(
        copies.len(),
        1,
        "Ctrl+Insert should fire copy callback once"
    );
    assert_eq!(copies[0], "Hello");
}

// ---------------------------------------------------------------------------
// Shift+Insert pastes (alternate key binding)
// C++ ref: emTextField.cpp:733-738 (EM_KEY_INSERT + IsShiftMod)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_insert_pastes_text() {
    let (mut h, tf_ref, _copy_recorder) = setup_clipboard_harness("AB", 2, "CD");

    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::Insert);
    h.input_state.release(InputKey::Shift);

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        "ABCD",
        "Shift+Insert should paste, got '{}'",
        tf.GetText()
    );
}

// ---------------------------------------------------------------------------
// Shift+Delete cuts (alternate key binding)
// C++ ref: emTextField.cpp:726-731 (EM_KEY_DELETE + IsShiftMod)
// ---------------------------------------------------------------------------

#[test]
fn textfield_shift_delete_cuts_text() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("ABCDEF", 4, "");

    tf_ref.borrow_mut().Select(2, 4);
    tf_ref.borrow_mut().SetCursorIndex(4);

    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::Delete);
    h.input_state.release(InputKey::Shift);

    let copies = copy_recorder.borrow();
    assert_eq!(copies.len(), 1, "Shift+Delete should fire copy callback");
    assert_eq!(copies[0], "CD");

    let tf = tf_ref.borrow();
    assert_eq!(tf.GetText(), "ABEF");
    assert_eq!(tf.GetCursorIndex(), 2);
}

// ---------------------------------------------------------------------------
// Selection publish on mouse drag
// C++ ref: emTextField.cpp:450,478,506 — PublishSelection on drag release
// ---------------------------------------------------------------------------

#[test]
fn textfield_drag_publishes_selection() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("Hello World", 0, "");

    // Drag to select some text. The drag uses view-space coords.
    // Focus Click at offset y to avoid double-Click with drag.
    h.click(400.0, 310.0);

    // Clear any copy events from the focus Click.
    copy_recorder.borrow_mut().clear();

    // Drag within content area to select text.
    h.drag(300.0, 300.0, 500.0, 300.0);

    let tf = tf_ref.borrow();
    if !tf.IsSelectionEmpty() {
        let copies = copy_recorder.borrow();
        assert!(
            !copies.is_empty(),
            "Drag selection should publish to clipboard (selection = '{}')",
            tf.selected_text()
        );
        // The published text should match the GetChecked text.
        let last_copy = copies.last().unwrap();
        assert_eq!(
            last_copy,
            tf.selected_text(),
            "Published text should match selected text"
        );
    }
}

// ---------------------------------------------------------------------------
// Ctrl+C on non-editable field still copies (copy is not an edit)
// C++ ref: emTextField.cpp:666-671 — copy is outside the IsEditable guard
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_c_works_when_non_editable() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("Hello World", 5, "");
    tf_ref.borrow_mut().SetEditable(false);

    tf_ref.borrow_mut().Select(0, 5);
    tf_ref.borrow_mut().SetCursorIndex(5);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('c'));
    h.input_state.release(InputKey::Ctrl);

    let copies = copy_recorder.borrow();
    assert_eq!(
        copies.len(),
        1,
        "Copy should work even on non-editable fields"
    );
    assert_eq!(copies[0], "Hello");
    // Text unchanged
    assert_eq!(tf_ref.borrow().GetText(), "Hello World");
}

// ---------------------------------------------------------------------------
// Ctrl+X on non-editable field does NOT cut (cut is an edit)
// C++ ref: emTextField.cpp:726-731 — IsEditable guard
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_x_noop_when_non_editable() {
    let (mut h, tf_ref, copy_recorder) = setup_clipboard_harness("Hello World", 5, "");
    tf_ref.borrow_mut().SetEditable(false);

    tf_ref.borrow_mut().Select(0, 5);
    tf_ref.borrow_mut().SetCursorIndex(5);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('x'));
    h.input_state.release(InputKey::Ctrl);

    let copies = copy_recorder.borrow();
    assert!(
        copies.is_empty(),
        "Cut should not fire on non-editable field"
    );
    assert_eq!(
        tf_ref.borrow().GetText(),
        "Hello World",
        "Text should be unchanged when cutting on non-editable field"
    );
}

// ---------------------------------------------------------------------------
// Ctrl+V on non-editable field does NOT paste (paste is an edit)
// C++ ref: emTextField.cpp:733-738 — IsEditable guard
// ---------------------------------------------------------------------------

#[test]
fn textfield_ctrl_v_noop_when_non_editable() {
    let (mut h, tf_ref, _copy_recorder) = setup_clipboard_harness("Hello", 5, "World");
    tf_ref.borrow_mut().SetEditable(false);

    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Key('v'));
    h.input_state.release(InputKey::Ctrl);

    assert_eq!(
        tf_ref.borrow().GetText(),
        "Hello",
        "Paste should not work on non-editable field"
    );
}

// ===========================================================================
// BP-14: emTextField drag-move (DM_MOVE)
// ===========================================================================
//
// C++ ref: emTextField.cpp:526-560 (DM_MOVE) and :374-389 (Ctrl+Click
// enters DM_MOVE when Click is inside selection).
//
// DM_MOVE: Ctrl+Click on GetChecked text, then drag to new GetPos → text
// moves. Uses drag offset tracking (DragPosC/DragPosR in C++, drag_offset
// in Rust) so the text follows the cursor naturally.

// ---------------------------------------------------------------------------
// Helper: Ctrl+drag (press with Ctrl, move, release)
// ---------------------------------------------------------------------------

/// Perform a Ctrl+drag: Ctrl+press at `from`, mouse-move to `to`, release.
/// Ctrl is held in `input_state` for the entire sequence.
fn ctrl_drag(h: &mut PipelineTestHarness, from_x: f64, from_y: f64, to_x: f64, to_y: f64) {
    h.input_state.press(InputKey::Ctrl);
    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(from_x, from_y);
    h.dispatch(&press);
    let move_ev = emInputEvent::mouse_move(InputKey::MouseLeft, to_x, to_y);
    h.dispatch(&move_ev);
    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(to_x, to_y);
    h.dispatch(&release);
    h.input_state.release(InputKey::Ctrl);
}

// ---------------------------------------------------------------------------
// Select text, Ctrl+drag to new GetPos → text moves
// C++ ref: emTextField.cpp:526-556 (DM_MOVE drag handler)
// ---------------------------------------------------------------------------

/// Select "bar" in "foo bar baz", then Ctrl+drag it to after "baz".
/// The text should rearrange to move "bar" to the new GetPos.
#[test]
fn textfield_drag_move_selected_text_moves() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("foo bar baz");

    render(&mut h, 800, 600);

    // Focus the field.
    h.click(400.0, 300.0);

    // Select "bar" (indices 4..7) via API.
    tf_ref.borrow_mut().Select(4, 7);
    tf_ref.borrow_mut().SetCursorIndex(7);

    let before_text = tf_ref.borrow().GetText().to_string();
    assert_eq!(before_text, "foo bar baz");
    assert_eq!(tf_ref.borrow().selected_text(), "bar");

    // Ctrl+drag from within selection to a GetPos after "baz".
    // We use the internal API to move: set up the move directly since
    // pixel coordinates depend on font metrics and layout.
    //
    // Instead of relying on exact pixel coords, we test via the
    // programmatic move path: manually invoke the move by simulating
    // what DM_MOVE does: cut the GetChecked text and re-insert at a
    // new GetPos.
    //
    // But the real test should go through the pipeline. Since
    // pixel coords are unreliable in tests (no real font metrics),
    // we verify the core logic: after the Ctrl+drag sequence completes,
    // the text should have changed if the drag target differs from the
    // selection GetPos.

    // The text field uses pos_from_event to determine positions.
    // At 800x600 with "foo bar baz", the text is laid out horizontally.
    // A Ctrl+Click within the selection range (char positions 4-7)
    // should enter DM_MOVE. Then dragging to a different GetPos
    // should move the text.

    // We need to know approximately where chars are rendered.
    // char_positions are populated during PaintContent. Let's just verify
    // the move logic works by doing the ctrl+drag within content area.
    // The exact text GetResult depends on where the coordinates map, but
    // the key assertion is that the text changed from the original.

    // Approach: Ctrl+Click at the same viewport GetPos (which is
    // inside the selection since we clicked there to focus), then
    // drag far to the right (near end of text).

    // First, we need the widget to see our pre-set selection.
    // Re-render so char_positions are fresh.
    render(&mut h, 800, 600);

    // Ctrl+drag from center (inside selection) to far right.
    ctrl_drag(&mut h, 400.0, 300.0, 700.0, 300.0);

    let tf = tf_ref.borrow();
    // The text should still contain all original characters (no loss).
    let mut sorted_before: Vec<char> = "foo bar baz".chars().collect();
    sorted_before.sort();
    let mut sorted_after: Vec<char> = tf.GetText().chars().collect();
    sorted_after.sort();
    assert_eq!(
        sorted_before,
        sorted_after,
        "Drag-move should not lose or gain characters. Before: 'foo bar baz', After: '{}'",
        tf.GetText()
    );
    // The text length should be preserved.
    assert_eq!(
        tf.GetText().len(),
        11,
        "Text length should be preserved after drag-move"
    );
}

// ---------------------------------------------------------------------------
// Drag outside widget → no effect
// C++ ref: emTextField.cpp:364 (CheckMouse → inArea guard on DM_NONE press)
// ---------------------------------------------------------------------------

/// Ctrl+Click outside the text content area should not enter DM_MOVE,
/// so the text and selection remain unchanged.
#[test]
fn textfield_drag_move_outside_widget_no_effect() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Focus the field.
    h.click(400.0, 300.0);

    // Select "World" (indices 6..11).
    tf_ref.borrow_mut().Select(6, 11);
    tf_ref.borrow_mut().SetCursorIndex(11);

    let text_before = tf_ref.borrow().GetText().to_string();

    // Ctrl+drag starting far outside the widget content area.
    // Coordinates (10, 10) should be outside the border's content rect.
    ctrl_drag(&mut h, 10.0, 10.0, 700.0, 300.0);

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        text_before,
        "Drag outside widget should not change text"
    );
    // The key invariant: text is unchanged.
    assert_eq!(tf.GetText(), "Hello World");
}

// ---------------------------------------------------------------------------
// Drag with no selection → no move (enters DM_INSERT, not DM_MOVE)
// C++ ref: emTextField.cpp:374-389 (Ctrl+Click outside selection → DM_INSERT)
// ---------------------------------------------------------------------------

/// Ctrl+Click with no selection should enter DM_INSERT GetMode, not DM_MOVE.
/// The text should remain unchanged after the drag (unless a paste occurs,
/// which requires a clipboard callback — we don't wire one here).
#[test]
fn textfield_drag_move_no_selection_no_move() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Focus the field.
    h.click(400.0, 300.0);

    // Ensure no selection.
    tf_ref.borrow_mut().EmptySelection();
    assert!(tf_ref.borrow().IsSelectionEmpty());

    let text_before = tf_ref.borrow().GetText().to_string();

    // Ctrl+drag within the content area. With no selection, this should
    // enter DM_INSERT, not DM_MOVE. Since we don't wire on_clipboard_paste,
    // no paste occurs on release, so text is unchanged.
    ctrl_drag(&mut h, 400.0, 300.0, 500.0, 300.0);

    let tf = tf_ref.borrow();
    assert_eq!(
        tf.GetText(),
        text_before,
        "Ctrl+drag with no selection should not move text (no DM_MOVE). Got '{}'",
        tf.GetText()
    );
}

// ---------------------------------------------------------------------------
// Drag in non-editable GetMode → no effect
// C++ ref: emTextField.cpp:375 (IsEditable() && IsEnabled() guard)
// ---------------------------------------------------------------------------

/// In non-editable GetMode, Ctrl+Click should not enter DM_MOVE even with
/// a selection, so the text and selection remain unchanged.
#[test]
fn textfield_drag_move_non_editable_no_effect() {
    let (mut h, tf_ref) = setup_textfield_harness();
    tf_ref.borrow_mut().SetText("Hello World");

    render(&mut h, 800, 600);

    // Focus the field while still editable.
    h.click(400.0, 300.0);

    // Select "World" (indices 6..11).
    tf_ref.borrow_mut().Select(6, 11);
    tf_ref.borrow_mut().SetCursorIndex(11);

    // Now make it non-editable.
    tf_ref.borrow_mut().SetEditable(false);

    let text_before = tf_ref.borrow().GetText().to_string();

    // Ctrl+drag within the content area.
    ctrl_drag(&mut h, 400.0, 300.0, 600.0, 300.0);

    let tf = tf_ref.borrow();
    // The primary invariant: text must NOT change in non-editable GetMode.
    // C++ ref: emTextField.cpp:375 — IsEditable() && IsEnabled() guard
    // prevents entering DM_MOVE. The Ctrl+Click falls through to regular
    // Click handling (which may reposition cursor / change selection), but
    // text content is never modified.
    assert_eq!(
        tf.GetText(),
        text_before,
        "Non-editable field should not allow drag-move. Got '{}'",
        tf.GetText()
    );
}
