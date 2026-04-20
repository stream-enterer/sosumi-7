//! Systematic interaction test for emRadioButton at 1x and 2x zoom, driven
//! through the full Input dispatch pipeline (PipelineTestHarness).
//!
//! Three radio buttons share a group, each installed in its own child panel
//! stacked vertically. Clicking each panel's center selects the corresponding
//! radio button. The test verifies correct selection at both 1x and 2x zoom.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emCursor::emCursor;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use emcore::emViewRenderer::SoftwareCompositor;

use super::support::pipeline::PipelineTestHarness;

// ---------------------------------------------------------------------------
// RadioButtonBehavior -- minimal PanelBehavior wrapper for emRadioButton
// ---------------------------------------------------------------------------

struct RadioButtonBehavior {
    widget: emRadioButton,
}

impl RadioButtonBehavior {
    fn new(widget: emRadioButton) -> Self {
        Self { widget }
    }
}

impl PanelBehavior for RadioButtonBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

/// Click each of three vertically-stacked radio buttons at 1x and 2x zoom,
/// verifying the group selection state after each Click.
#[test]
fn radiobutton_select_1x_and_2x() {
    let mut h = PipelineTestHarness::new();

    let look = emLook::new();
    let group: Rc<RefCell<RadioGroup>> = RadioGroup::new(&mut h.sched_ctx());

    // Create 3 RadioButtons sharing the same group.
    let rb0 = emRadioButton::new("Option A", look.clone(), group.clone(), 0);
    let rb1 = emRadioButton::new("Option B", look.clone(), group.clone(), 1);
    let rb2 = emRadioButton::new("Option C", look.clone(), group.clone(), 2);

    assert_eq!(group.borrow().GetCount(), 3);
    assert_eq!(group.borrow().GetChecked(), None);

    // ── Build pipeline harness (800x600 viewport) ────────────────────
    let root = h.get_root_panel();

    // Each radio button gets its own child panel, stacked vertically:
    //   panel 0: y=0.00..0.33  (top third)
    //   panel 1: y=0.33..0.66  (middle third)
    //   panel 2: y=0.66..1.00  (bottom third)
    let panel0 = h.add_panel_with(root, "radio0", Box::new(RadioButtonBehavior::new(rb0)));
    h.tree.Layout(panel0, 0.0, 0.0, 1.0, 1.0 / 3.0, 1.0, None);

    let panel1 = h.add_panel_with(root, "radio1", Box::new(RadioButtonBehavior::new(rb1)));
    h.tree
        .Layout(panel1, 0.0, 1.0 / 3.0, 1.0, 1.0 / 3.0, 1.0, None);

    let panel2 = h.add_panel_with(root, "radio2", Box::new(RadioButtonBehavior::new(rb2)));
    h.tree
        .Layout(panel2, 0.0, 2.0 / 3.0, 1.0, 1.0 / 3.0, 1.0, None);

    // Settle layout and viewing Restore.
    h.tick_n(5);

    // Render so that emRadioButton::PaintContent() caches last_w/last_h (required
    // for hit_test to function).
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // ── Helper: compute view-space center of a panel ─────────────────
    let panel_center = |harness: &PipelineTestHarness, panel_id| {
        let state = harness.tree.build_panel_state(
            panel_id,
            harness.view.IsFocused(),
            harness.view.GetCurrentPixelTallness(),
        );
        let vr = state.viewed_rect;
        (vr.x + vr.w * 0.5, vr.y + vr.h * 0.5)
    };

    // ── 1x zoom: Click each radio button ─────────────────────────────
    {
        let (cx, cy) = panel_center(&h, panel0);
        h.click(cx, cy);
        assert_eq!(
            group.borrow().GetChecked(),
            Some(0),
            "1x: clicking panel 0 should select radio button 0"
        );
    }
    {
        let (cx, cy) = panel_center(&h, panel1);
        h.click(cx, cy);
        assert_eq!(
            group.borrow().GetChecked(),
            Some(1),
            "1x: clicking panel 1 should select radio button 1"
        );
    }
    {
        let (cx, cy) = panel_center(&h, panel2);
        h.click(cx, cy);
        assert_eq!(
            group.borrow().GetChecked(),
            Some(2),
            "1x: clicking panel 2 should select radio button 2"
        );
    }

    // ── 2x zoom: same test at higher magnification ───────────────────
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    {
        let (cx, cy) = panel_center(&h, panel0);
        h.click(cx, cy);
        assert_eq!(
            group.borrow().GetChecked(),
            Some(0),
            "2x: clicking panel 0 should select radio button 0"
        );
    }
    {
        let (cx, cy) = panel_center(&h, panel1);
        h.click(cx, cy);
        assert_eq!(
            group.borrow().GetChecked(),
            Some(1),
            "2x: clicking panel 1 should select radio button 1"
        );
    }
    {
        let (cx, cy) = panel_center(&h, panel2);
        h.click(cx, cy);
        assert_eq!(
            group.borrow().GetChecked(),
            Some(2),
            "2x: clicking panel 2 should select radio button 2"
        );
    }
}

// ---------------------------------------------------------------------------
// BP-13 emRadioButton exclusion tests -- shared harness
// ---------------------------------------------------------------------------

struct RadioButtonHarness {
    h: PipelineTestHarness,
    group: Rc<RefCell<RadioGroup>>,
    panels: [PanelId; 3],
    compositor: SoftwareCompositor,
}

impl RadioButtonHarness {
    fn new() -> Self {
        let mut h = PipelineTestHarness::new();
        let look = emLook::new();
        let group: Rc<RefCell<RadioGroup>> = RadioGroup::new(&mut h.sched_ctx());

        let rb0 = emRadioButton::new("Option A", look.clone(), group.clone(), 0);
        let rb1 = emRadioButton::new("Option B", look.clone(), group.clone(), 1);
        let rb2 = emRadioButton::new("Option C", look.clone(), group.clone(), 2);

        assert_eq!(group.borrow().GetCount(), 3);
        assert_eq!(group.borrow().GetChecked(), None);

        let root = h.get_root_panel();

        let panel0 = h.add_panel_with(root, "radio0", Box::new(RadioButtonBehavior::new(rb0)));
        h.tree.Layout(panel0, 0.0, 0.0, 1.0, 1.0 / 3.0, 1.0, None);

        let panel1 = h.add_panel_with(root, "radio1", Box::new(RadioButtonBehavior::new(rb1)));
        h.tree
            .Layout(panel1, 0.0, 1.0 / 3.0, 1.0, 1.0 / 3.0, 1.0, None);

        let panel2 = h.add_panel_with(root, "radio2", Box::new(RadioButtonBehavior::new(rb2)));
        h.tree
            .Layout(panel2, 0.0, 2.0 / 3.0, 1.0, 1.0 / 3.0, 1.0, None);

        h.tick_n(5);

        let mut compositor = SoftwareCompositor::new(800, 600);
        compositor.render(&mut h.tree, &h.view);

        Self {
            h,
            group,
            panels: [panel0, panel1, panel2],
            compositor,
        }
    }

    fn panel_center(&self, index: usize) -> (f64, f64) {
        let state = self.h.tree.build_panel_state(
            self.panels[index],
            self.h.view.IsFocused(),
            self.h.view.GetCurrentPixelTallness(),
        );
        let vr = state.viewed_rect;
        (vr.x + vr.w * 0.5, vr.y + vr.h * 0.5)
    }

    fn checked(&self) -> Option<usize> {
        self.group.borrow().GetChecked()
    }

    fn click_option(&mut self, index: usize) {
        let (cx, cy) = self.panel_center(index);
        self.h.click(cx, cy);
    }
}

// ---------------------------------------------------------------------------
// BP-13: Click radio A -> A GetChecked, B and C deselected (mutual exclusion)
// ---------------------------------------------------------------------------

/// Click radio A, verify A is GetChecked and B/C are deselected.
/// C++ ref: emRadioButton.cpp:Clicked -> Mechanism::SetChecked -> SetCheckIndex.
#[test]
fn bp13_click_a_selects_a_deselects_bc() {
    let mut t = RadioButtonHarness::new();

    t.click_option(0);
    assert_eq!(
        t.checked(),
        Some(0),
        "A should be selected after clicking A"
    );
    // Verify B and C are not GetChecked by checking group state
    assert_ne!(t.checked(), Some(1), "B must not be selected");
    assert_ne!(t.checked(), Some(2), "C must not be selected");
}

// ---------------------------------------------------------------------------
// BP-13: Click radio B -> B GetChecked, A and C deselected
// ---------------------------------------------------------------------------

/// Click radio B after A is GetChecked, verify B is now GetChecked and A/C are not.
/// C++ ref: emRadioButton.cpp:Clicked -> Mechanism::SetChecked -> SetCheckIndex.
#[test]
fn bp13_click_b_selects_b_deselects_ac() {
    let mut t = RadioButtonHarness::new();

    // First select A
    t.click_option(0);
    assert_eq!(t.checked(), Some(0));

    // Now Click B
    t.click_option(1);
    assert_eq!(
        t.checked(),
        Some(1),
        "B should be selected after clicking B"
    );
    assert_ne!(t.checked(), Some(0), "A must be deselected");
    assert_ne!(t.checked(), Some(2), "C must not be selected");
}

// ---------------------------------------------------------------------------
// BP-13: Click already-GetChecked radio -> no change, no redundant callback
// ---------------------------------------------------------------------------

/// Clicking an already-GetChecked radio button must not change state and must
/// not fire a redundant callback.
/// C++ ref: emRadioButton::Mechanism::SetCheckIndex — early return if CheckIndex==index.
#[test]
fn bp13_click_already_selected_no_change_no_callback() {
    let mut t = RadioButtonHarness::new();

    // Select A
    t.click_option(0);
    assert_eq!(t.checked(), Some(0));

    // Install callback tracker AFTER initial selection
    let callbacks = Rc::new(RefCell::new(Vec::new()));
    let cb_clone = callbacks.clone();
    t.group.borrow_mut().on_select = Some(Box::new(
        move |idx, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
            cb_clone.borrow_mut().push(idx);
        },
    ));

    // Click A again -- should be no-op, no callback
    t.click_option(0);
    assert_eq!(
        t.checked(),
        Some(0),
        "re-clicking already-selected must not deselect it"
    );
    assert!(
        callbacks.borrow().is_empty(),
        "no callback should fire when clicking already-selected radio button"
    );
}

// ---------------------------------------------------------------------------
// BP-13: Programmatic SetCheckIndex -> correct button checked + signal fired
// ---------------------------------------------------------------------------

/// Programmatic SetCheckIndex selects the correct button and fires the callback.
/// C++ ref: emRadioButton::Mechanism::SetCheckIndex.
#[test]
fn bp13_programmatic_set_check_index_fires_callback() {
    let mut t = RadioButtonHarness::new();

    let callbacks = Rc::new(RefCell::new(Vec::new()));
    let cb_clone = callbacks.clone();
    t.group.borrow_mut().on_select = Some(Box::new(
        move |idx, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
            cb_clone.borrow_mut().push(idx);
        },
    ));

    // Programmatically select button 2
    {
        let mut __ctx = t.h.panel_ctx();
        t.group.borrow_mut().SetCheckIndex(Some(2), &mut __ctx);
    }
    assert_eq!(
        t.checked(),
        Some(2),
        "set_check_index(Some(2)) should select button 2"
    );
    assert_eq!(
        *callbacks.borrow(),
        vec![Some(2)],
        "callback should fire with Some(2)"
    );

    // Now change to button 0
    {
        let mut __ctx = t.h.panel_ctx();
        t.group.borrow_mut().SetCheckIndex(Some(0), &mut __ctx);
    }
    assert_eq!(
        t.checked(),
        Some(0),
        "set_check_index(Some(0)) should select button 0"
    );
    assert_eq!(
        *callbacks.borrow(),
        vec![Some(2), Some(0)],
        "callback should fire for each change"
    );
}

// ---------------------------------------------------------------------------
// BP-13: Programmatic SetCheckIndex to same GetValue -> no callback
// ---------------------------------------------------------------------------

/// Setting check_index to the already-GetChecked GetValue must be a no-op (no callback).
/// C++ ref: emRadioButton::Mechanism::SetCheckIndex — early return if CheckIndex==index.
#[test]
fn bp13_programmatic_set_check_index_same_value_no_callback() {
    let mut t = RadioButtonHarness::new();

    // Select button 1
    {
        let mut __ctx = t.h.panel_ctx();
        t.group.borrow_mut().SetCheckIndex(Some(1), &mut __ctx);
    }
    assert_eq!(t.checked(), Some(1));

    // Install callback tracker AFTER initial selection
    let callbacks = Rc::new(RefCell::new(Vec::new()));
    let cb_clone = callbacks.clone();
    t.group.borrow_mut().on_select = Some(Box::new(
        move |idx, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
            cb_clone.borrow_mut().push(idx);
        },
    ));

    // Set same index again -- no-op
    {
        let mut __ctx = t.h.panel_ctx();
        t.group.borrow_mut().SetCheckIndex(Some(1), &mut __ctx);
    }
    assert_eq!(t.checked(), Some(1));
    assert!(
        callbacks.borrow().is_empty(),
        "no callback when set_check_index to same value"
    );
}

// ---------------------------------------------------------------------------
// BP-13: Enter key selects radio button (inherited from emButton)
// ---------------------------------------------------------------------------

/// Enter key press on a radio button selects it, matching C++ emButton.cpp:113-119.
/// The pipeline dispatches Enter as a keyboard event to the active panel.
#[test]
fn bp13_enter_key_selects_radio_button() {
    let mut t = RadioButtonHarness::new();

    // Make panel 1 the active panel so keyboard events reach it
    let (cx, cy) = t.panel_center(1);
    // Click panel 1 to make it active
    t.h.click(cx, cy);
    // Panel 1 is now active and GetChecked via mouse Click
    assert_eq!(t.checked(), Some(1));

    // Now HardResetFileState selection programmatically to test Enter independently
    {
        let mut __ctx = t.h.panel_ctx();
        t.group.borrow_mut().SetCheckIndex(None, &mut __ctx);
    }
    assert_eq!(t.checked(), None);

    // Press Enter -- should select panel 1 (the active panel)
    t.h.press_key(InputKey::Enter);
    assert_eq!(
        t.checked(),
        Some(1),
        "Enter key should select the active radio button"
    );
}

// ---------------------------------------------------------------------------
// BP-13: Modifier gating -- Ctrl/Alt/Meta rejected, Shift accepted
// ---------------------------------------------------------------------------

/// Mouse Click with Ctrl modifier is rejected by emRadioButton Input handler.
/// C++ ref: emButton.cpp:82 — (state.IsNoMod() || state.IsShiftMod()).
#[test]
fn bp13_ctrl_click_rejected() {
    let mut t = RadioButtonHarness::new();

    // Hold Ctrl in the Input state so dispatch stamps it on the event
    t.h.input_state.press(InputKey::Ctrl);

    let (cx, cy) = t.panel_center(0);
    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(cx, cy);
    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(cx, cy);
    t.h.dispatch(&press);
    t.h.dispatch(&release);

    t.h.input_state.release(InputKey::Ctrl);

    assert_eq!(
        t.checked(),
        None,
        "Ctrl+click must not select a radio button"
    );
}

/// Mouse Click with Alt modifier is rejected.
/// C++ ref: emButton.cpp:82.
#[test]
fn bp13_alt_click_rejected() {
    let mut t = RadioButtonHarness::new();

    t.h.input_state.press(InputKey::Alt);

    let (cx, cy) = t.panel_center(0);
    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(cx, cy);
    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(cx, cy);
    t.h.dispatch(&press);
    t.h.dispatch(&release);

    t.h.input_state.release(InputKey::Alt);

    assert_eq!(
        t.checked(),
        None,
        "Alt+click must not select a radio button"
    );
}

/// Mouse Click with Meta modifier is rejected.
/// C++ ref: emButton.cpp:82.
#[test]
fn bp13_meta_click_rejected() {
    let mut t = RadioButtonHarness::new();

    t.h.input_state.press(InputKey::Meta);

    let (cx, cy) = t.panel_center(0);
    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(cx, cy);
    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(cx, cy);
    t.h.dispatch(&press);
    t.h.dispatch(&release);

    t.h.input_state.release(InputKey::Meta);

    assert_eq!(
        t.checked(),
        None,
        "Meta+click must not select a radio button"
    );
}

/// Mouse Click with Shift modifier is accepted (Shift is allowed).
/// C++ ref: emButton.cpp:82 — IsShiftMod().
#[test]
fn bp13_shift_click_accepted() {
    let mut t = RadioButtonHarness::new();

    t.h.input_state.press(InputKey::Shift);

    let (cx, cy) = t.panel_center(0);
    let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(cx, cy);
    let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(cx, cy);
    t.h.dispatch(&press);
    t.h.dispatch(&release);

    t.h.input_state.release(InputKey::Shift);

    assert_eq!(
        t.checked(),
        Some(0),
        "Shift+click must be accepted by radio button"
    );
}

// ---------------------------------------------------------------------------
// BP-13: Disabled radio button rejects Input
// ---------------------------------------------------------------------------

/// A disabled radio button must reject all Input events.
/// C++ ref: emButton::Input checks enabled state via panel state.
#[test]
fn bp13_disabled_radio_rejects_input() {
    let mut t = RadioButtonHarness::new();

    // Disable panel 0
    t.h.tree.SetEnableSwitch(t.panels[0], false, None);
    t.h.tick_n(3);
    // Re-render so the disabled state is propagated to the widget via PaintContent
    t.compositor.render(&mut t.h.tree, &t.h.view);

    // Try clicking the disabled panel
    t.click_option(0);
    assert_eq!(
        t.checked(),
        None,
        "disabled radio button must not accept clicks"
    );

    // Enable panel 1 and verify it still works
    t.click_option(1);
    assert_eq!(
        t.checked(),
        Some(1),
        "enabled radio button should still work"
    );
}
