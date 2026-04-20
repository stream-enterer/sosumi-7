//! Systematic interaction test for emScalarField at 1x and 2x zoom, driven
//! through the full Input dispatch pipeline (PipelineTestHarness).
//!
//! Verifies that Click and drag interactions correctly update the widget's
//! value at both zoom levels, using approximate assertions to account for
//! border insets in the content area.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emCursor::emCursor;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emScalarField::emScalarField;
use emcore::emViewRenderer::SoftwareCompositor;

use super::support::pipeline::PipelineTestHarness;

/// PanelBehavior wrapper for emScalarField so it can be installed into the
/// panel tree. Delegates PaintContent/Input to the underlying widget and syncs
/// the value to a shared handle after every Input event.
struct ScalarFieldPanel {
    sf: emScalarField,
    /// Shared handle so the test can read the value after interaction.
    value: Rc<RefCell<f64>>,
}

impl ScalarFieldPanel {
    fn new(sf: emScalarField, value: Rc<RefCell<f64>>) -> Self {
        Self { sf, value }
    }
}

impl PanelBehavior for ScalarFieldPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.sf.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        let consumed = self.sf.Input(event, state, input_state);
        *self.value.borrow_mut() = self.sf.GetValue();
        consumed
    }

    fn GetCursor(&self) -> emCursor {
        self.sf.GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

/// Helper: assert that `actual` is within `tolerance` of `expected`.
fn assert_approx(actual: f64, expected: f64, tolerance: f64, context: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{context}: expected ~{expected:.1} (+-{tolerance}), got {actual:.1}"
    );
}

#[test]
fn scalarfield_click_and_drag_1x_and_2x() {
    // 1. Create PipelineTestHarness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // 2. Create emScalarField (range 0-100, value 50, editable).
    let look = emLook::new();
    let mut sf = emScalarField::new(0.0, 100.0, look);
    sf.SetValue(50.0);
    sf.SetEditable(true);

    let value = Rc::new(RefCell::new(50.0));
    let value_read = value.clone();

    // 3. Wrap in ScalarFieldPanel and add to tree.
    let behavior = ScalarFieldPanel::new(sf, value);
    let _panel_id = h.add_panel_with(root, "scalar_field", Box::new(behavior));

    // 4. Tick + render via SoftwareCompositor to populate last_w/last_h.
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    let vw = 800.0;
    let vh = 600.0;
    let mid_y = vh * 0.5;

    // ── 5. At 1x zoom ──────────────────────────────────────────────────
    //
    // The emScalarField has an Instrument outer border, InputField inner
    // border, and HowTo space on the left. These insets eat into the
    // usable scale area, so viewport percentages do not map linearly to
    // value percentages. We Click at positions well inside the scale
    // area and use generous tolerances (+-15).

    // Click at ~40% of viewport width -> value should be somewhere near 30-40.
    let click_x_40 = vw * 0.40;
    h.click(click_x_40, mid_y);
    let val_after_click_1x = *value_read.borrow();
    assert!(
        val_after_click_1x > 10.0 && val_after_click_1x < 55.0,
        "1x click at 40% viewport: expected value in 10..55, got {val_after_click_1x:.1}"
    );

    // Drag from 40% to ~65% of viewport width -> value should increase
    // significantly toward the mid-to-high range.
    let drag_to_x = vw * 0.65;
    h.drag(click_x_40, mid_y, drag_to_x, mid_y);
    let val_after_drag_1x = *value_read.borrow();
    assert!(
        val_after_drag_1x > val_after_click_1x + 5.0,
        "1x drag from 40% to 65%: value should increase by >5 from {val_after_click_1x:.1}, \
         got {val_after_drag_1x:.1}"
    );
    assert!(
        val_after_drag_1x > 40.0 && val_after_drag_1x < 90.0,
        "1x drag to 65% viewport: expected value in 40..90, got {val_after_drag_1x:.1}"
    );

    // ── 6. At 2x zoom ──────────────────────────────────────────────────
    //
    // At 2x zoom the panel is magnified 2x: the viewport shows only
    // the center 50% of the panel. The viewport center (400,300) still
    // maps to the panel center (value ~50).

    // Set zoom to 2x, tick, re-render.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    // Click at viewport center to HardResetFileState value to ~50.
    let center_x = vw * 0.5;
    h.click(center_x, mid_y);
    let val_after_center_2x = *value_read.borrow();
    assert_approx(
        val_after_center_2x,
        50.0,
        15.0,
        "2x click at viewport center",
    );

    // Click at ~30% of viewport width at 2x zoom.
    // At 2x the visible panel portion is 25%-75% of the panel, so 30%
    // viewport maps to roughly 40% panel -> value ~30-40.
    let click_x_30_2x = vw * 0.30;
    h.click(click_x_30_2x, mid_y);
    let val_after_click_2x = *value_read.borrow();
    assert!(
        (val_after_click_2x - val_after_center_2x).abs() > 1.0,
        "2x Click at 30% viewport should change value from {val_after_center_2x:.1}, \
         but got {val_after_click_2x:.1}"
    );

    // Drag from 30% to ~70% of viewport width at 2x zoom.
    let drag_to_x_2x = vw * 0.70;
    h.drag(click_x_30_2x, mid_y, drag_to_x_2x, mid_y);
    let val_after_drag_2x = *value_read.borrow();
    assert!(
        val_after_drag_2x > val_after_click_2x,
        "2x drag from 30% to 70% should increase value: \
         was {val_after_click_2x:.1}, now {val_after_drag_2x:.1}"
    );
}

// ── BP-8 behavioral parity tests ──────────────────────────────────────

/// Helper: set up a emScalarField in a PipelineTestHarness, render once to
/// populate `last_w`/`last_h`, and Click at center to activate the panel
/// for keyboard Input. Returns (harness, value_handle, panel_id).
fn setup_sf(
    min: f64,
    max: f64,
    initial: f64,
    editable: bool,
    mark_intervals: &[u64],
    kb_interval: u64,
) -> (
    PipelineTestHarness,
    Rc<RefCell<f64>>,
    emcore::emPanelTree::PanelId,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let look = emLook::new();
    let mut sf = emScalarField::new(min, max, look);
    sf.SetValue(initial);
    sf.SetEditable(editable);
    if !mark_intervals.is_empty() {
        sf.SetScaleMarkIntervals(mark_intervals);
    }
    if kb_interval > 0 {
        sf.SetKeyboardInterval(kb_interval);
    }

    let value = Rc::new(RefCell::new(initial));
    let value_read = value.clone();

    let behavior = ScalarFieldPanel::new(sf, value);
    let panel_id = h.add_panel_with(root, "sf", Box::new(behavior));

    // Tick + render to populate PaintContent dimensions.
    h.tick_n(5);
    let mut comp = SoftwareCompositor::new(800, 600);
    comp.render(&mut h.tree, &h.view);

    // Click center to activate panel for keyboard events.
    h.click(400.0, 300.0);

    (h, value_read, panel_id)
}

/// BP-8a: Click on scale jumps to absolute value GetPos.
/// C++ ref: emScalarField.cpp:250-258 — inArea && LeftButton press → SetValue(mv).
#[test]
fn scalarfield_click_jumps_to_absolute_position() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 0.0, true, &[], 0);

    // Click at ~75% of viewport width — should jump to a high value.
    h.click(600.0, 300.0);
    let val = *value.borrow();
    assert!(
        val > 40.0,
        "click at 75% viewport: expected value > 40, got {val:.1}"
    );

    // Click at ~25% of viewport width — should jump to a lower value.
    h.click(200.0, 300.0);
    let val2 = *value.borrow();
    assert!(
        val2 < val - 10.0,
        "click at 25% viewport: expected value significantly less than {val:.1}, got {val2:.1}"
    );
}

/// BP-8b: Drag continuously updates value.
/// C++ ref: emScalarField.cpp:241-248 — Pressed state continuously sets
/// value to mouse GetPos on every event.
#[test]
fn scalarfield_drag_continuous_update() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[], 0);

    // Multi-step drag: press, move to several positions, release.
    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);
    let v0 = *value.borrow();

    // Move to a GetPos further right.
    let move1 = emInputEvent::mouse_move(InputKey::MouseLeft, 500.0, 300.0);
    h.dispatch(&move1);
    let v1 = *value.borrow();
    assert!(
        v1 > v0,
        "drag move right: expected value > {v0:.1}, got {v1:.1}"
    );

    // Move further right again.
    let move2 = emInputEvent::mouse_move(InputKey::MouseLeft, 600.0, 300.0);
    h.dispatch(&move2);
    let v2 = *value.borrow();
    assert!(
        v2 > v1,
        "drag move further right: expected value > {v1:.1}, got {v2:.1}"
    );

    // Move back left — value should decrease.
    let move3 = emInputEvent::mouse_move(InputKey::MouseLeft, 350.0, 300.0);
    h.dispatch(&move3);
    let v3 = *value.borrow();
    assert!(
        v3 < v2,
        "drag move left: expected value < {v2:.1}, got {v3:.1}"
    );

    // Release.
    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(350.0, 300.0);
    h.dispatch(&release);
}

/// BP-8c: '+' key steps value up by mark interval.
/// C++ ref: emScalarField.cpp:261-265 — strcmp("+") → StepByKeyboard(1).
#[test]
fn scalarfield_plus_key_steps_up() {
    // Range 0-100, mark intervals [10, 5, 1], start at 50.
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[10, 5, 1], 0);

    // C++ auto-interval: range/129 ≈ 0.77, so mindv=1. Scan intervals:
    // [10]>=1 → dv=10, [5]>=1 → dv=5, [1]>=1 → dv=1. Final dv=1.
    // But Rust uses f64 range, so range=100, 100/129=0 → mindv=1, same logic.
    let before = *value.borrow();
    h.press_key(InputKey::Key('+'));
    let after = *value.borrow();
    assert!(
        after > before,
        "'+' key should increase value from {before:.1}, got {after:.1}"
    );
}

/// BP-8d: '-' key steps value down by mark interval.
/// C++ ref: emScalarField.cpp:267-272 — strcmp("-") → StepByKeyboard(-1).
#[test]
fn scalarfield_minus_key_steps_down() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[10, 5, 1], 0);

    let before = *value.borrow();
    h.press_key(InputKey::Key('-'));
    let after = *value.borrow();
    assert!(
        after < before,
        "'-' key should decrease value from {before:.1}, got {after:.1}"
    );
}

/// BP-8e: Keyboard stepping with explicit kb_interval steps by that amount.
/// C++ ref: emScalarField.cpp:483 — if (KBInterval>0) dv=KBInterval.
#[test]
fn scalarfield_keyboard_explicit_interval() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[10, 5, 1], 10);

    h.press_key(InputKey::Key('+'));
    let after_plus = *value.borrow();
    assert_approx(after_plus, 60.0, 1.0, "explicit kb_interval=10: +");

    h.press_key(InputKey::Key('-'));
    let after_minus = *value.borrow();
    assert_approx(after_minus, 50.0, 1.0, "explicit kb_interval=10: -");
}

/// BP-8f: Keyboard stepping snaps to nearest grid mark.
/// C++ ref: emScalarField.cpp:495-503 — integer division rounding snaps to
/// multiples of dv.
#[test]
fn scalarfield_keyboard_snaps_to_grid() {
    // Range 0-100, kb_interval=10, start at 53 (not on grid).
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 53.0, true, &[], 10);

    // C++ step_up: v = 53 + 10 = 63. v >= 0 → (63/10)*10 = 60.
    h.press_key(InputKey::Key('+'));
    let after_plus = *value.borrow();
    assert_approx(
        after_plus,
        60.0,
        1.0,
        "snap-to-grid: 53 + step(10) should snap to 60",
    );

    // Step down from 60: v = 60 - 10 = 50. (50+9)/10*10 = 50.
    h.press_key(InputKey::Key('-'));
    let after_minus = *value.borrow();
    assert_approx(
        after_minus,
        50.0,
        1.0,
        "snap-to-grid: 60 - step(10) should snap to 50",
    );
}

/// BP-8g: Keyboard stepping snaps off-grid value in step-down direction.
/// C++ ref: emScalarField.cpp:495-497.
#[test]
fn scalarfield_keyboard_snap_down_from_off_grid() {
    // Range 0-100, kb_interval=10, start at 47 (not on grid).
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 47.0, true, &[], 10);

    // C++ step_down: v = 47 - 10 = 37. v >= 0 → (37+9)/10*10 = 40.
    h.press_key(InputKey::Key('-'));
    let after_minus = *value.borrow();
    assert_approx(
        after_minus,
        40.0,
        1.0,
        "snap-to-grid down: 47 - step(10) should snap to 40",
    );
}

/// BP-8h: Value clamping at max boundary via keyboard stepping.
/// C++ ref: emScalarField.cpp:503 → SetValue(v) which clamps.
#[test]
fn scalarfield_keyboard_clamp_at_max() {
    // Use kb_interval=10, range 0-100. setup_sf clicks center → value ~50.
    // Step up repeatedly until we hit the ceiling.
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[], 10);

    // Step up many times to reach max.
    for _ in 0..20 {
        h.press_key(InputKey::Key('+'));
    }
    let at_max = *value.borrow();
    assert_approx(at_max, 100.0, 1.0, "clamp at max after many steps");

    // One more step should stay at 100.
    h.press_key(InputKey::Key('+'));
    let still_max = *value.borrow();
    assert_approx(still_max, 100.0, 1.0, "clamp at max: stays at 100");
}

/// BP-8i: Value clamping at min boundary via keyboard stepping.
/// C++ ref: emScalarField.cpp:503 → SetValue(v) which clamps.
#[test]
fn scalarfield_keyboard_clamp_at_min() {
    // Use kb_interval=10, range 0-100. setup_sf clicks center → value ~50.
    // Step down repeatedly until we hit the floor.
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[], 10);

    // Step down many times to reach min.
    for _ in 0..20 {
        h.press_key(InputKey::Key('-'));
    }
    let at_min = *value.borrow();
    assert_approx(at_min, 0.0, 1.0, "clamp at min after many steps");

    // One more step should stay at 0.
    h.press_key(InputKey::Key('-'));
    let still_min = *value.borrow();
    assert_approx(still_min, 0.0, 1.0, "clamp at min: stays at 0");
}

/// BP-8j: Non-editable emScalarField rejects all Input.
/// C++ ref: emScalarField.cpp:251,261,268 — gates on IsEditable().
#[test]
fn scalarfield_non_editable_rejects_input() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, false, &[], 10);

    // Click should not change value.
    h.click(600.0, 300.0);
    let after_click = *value.borrow();
    assert_approx(after_click, 50.0, 0.01, "non-editable: click rejected");

    // Keyboard should not change value.
    h.press_key(InputKey::Key('+'));
    let after_plus = *value.borrow();
    assert_approx(after_plus, 50.0, 0.01, "non-editable: '+' rejected");

    h.press_key(InputKey::Key('-'));
    let after_minus = *value.borrow();
    assert_approx(after_minus, 50.0, 0.01, "non-editable: '-' rejected");
}

/// BP-8k: Disabled emScalarField rejects all Input.
/// C++ ref: emScalarField.cpp:246,251,261,268 — gates on IsEnabled().
#[test]
fn scalarfield_disabled_rejects_input() {
    let (mut h, value, pid) = setup_sf(0.0, 100.0, 50.0, true, &[], 10);

    // Disable the panel via the tree.
    h.tree.SetEnableSwitch(pid, false, None);
    h.tick_n(3);
    // Re-render so that PaintContent() propagates the disabled state into the widget.
    let mut comp = SoftwareCompositor::new(800, 600);
    comp.render(&mut h.tree, &h.view);

    // Click should not change value.
    h.click(600.0, 300.0);
    let after_click = *value.borrow();
    assert_approx(after_click, 50.0, 0.01, "disabled: click rejected");

    // Keyboard should not change value.
    h.press_key(InputKey::Key('+'));
    let after_plus = *value.borrow();
    assert_approx(after_plus, 50.0, 0.01, "disabled: '+' rejected");
}

/// BP-8l: Click on scale sets absolute GetPos (not relative/incremental).
/// C++ ref: emScalarField.cpp:256-258 — SetValue(mv) where mv is computed
/// from mouse x-GetPos on the scale.
#[test]
fn scalarfield_click_is_absolute_not_relative() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 0.0, true, &[], 0);

    // Click at center — value should jump to ~50 regardless of starting at 0.
    h.click(400.0, 300.0);
    let val1 = *value.borrow();
    assert!(
        val1 > 30.0 && val1 < 70.0,
        "absolute click at center: expected ~50, got {val1:.1}"
    );

    // Click at the same spot again — value should stay approximately the same.
    h.click(400.0, 300.0);
    let val2 = *value.borrow();
    assert_approx(val2, val1, 1.0, "repeated click at same position");
}

/// BP-8m: Drag release terminates continuous update.
/// C++ ref: emScalarField.cpp:241-245 — !LeftButton → Pressed=false.
/// After release, further mouse moves should NOT update the value.
#[test]
fn scalarfield_drag_release_stops_update() {
    let (mut h, value, _pid) = setup_sf(0.0, 100.0, 50.0, true, &[], 0);

    // Drag to set a value.
    h.drag(400.0, 300.0, 600.0, 300.0);
    let val_after_drag = *value.borrow();

    // Now just move the mouse (no button held) — value should not change.
    let move_ev = emInputEvent::mouse_move(InputKey::MouseLeft, 200.0, 300.0);
    h.dispatch(&move_ev);
    let val_after_move = *value.borrow();
    assert_approx(
        val_after_move,
        val_after_drag,
        0.01,
        "mouse move after release should not change value",
    );
}

/// BP-8n: Auto keyboard interval selection from scale mark intervals.
/// C++ ref: emScalarField.cpp:484-493 — scans ScaleMarkIntervals to find
/// best match for mindv = range/129.
#[test]
fn scalarfield_keyboard_auto_interval_selection() {
    // Range 0-1000, marks [100, 50, 10, 5, 1], kb_interval=0 (auto).
    // mindv = 1000/129 = 7. Scan: [100]>=7 → dv=100, [50]>=7 → dv=50,
    // [10]>=7 → dv=10, [5]<7 → skip, [1]<7 → skip. Final dv=10.
    let (mut h, value, _pid) = setup_sf(0.0, 1000.0, 500.0, true, &[100, 50, 10, 5, 1], 0);

    h.press_key(InputKey::Key('+'));
    let after = *value.borrow();
    assert_approx(
        after,
        510.0,
        1.0,
        "auto interval: 500 + step should use dv=10",
    );
}
