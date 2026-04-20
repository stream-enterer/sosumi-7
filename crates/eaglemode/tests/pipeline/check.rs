//! Systematic interaction tests for emCheckButton and emCheckBox at 1x and 2x zoom,
//! driven through the full Input dispatch pipeline (PipelineTestHarness).
//!
//! These tests verify that mouse clicks toggle the checked state correctly when
//! dispatched through the coordinate-transform pipeline at different zoom levels.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emCheckBox::emCheckBox;
use emcore::emCheckButton::emCheckButton;
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emViewRenderer::SoftwareCompositor;

use super::support::pipeline::PipelineTestHarness;

// ---------------------------------------------------------------------------
// PanelBehavior wrapper for emCheckButton (shared via Rc<RefCell>)
// ---------------------------------------------------------------------------

struct SharedCheckButtonPanel {
    inner: Rc<RefCell<emCheckButton>>,
}

impl PanelBehavior for SharedCheckButtonPanel {
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
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.inner
            .borrow_mut()
            .Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.inner.borrow().GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// PanelBehavior wrapper for emCheckBox (shared via Rc<RefCell>)
// ---------------------------------------------------------------------------

struct SharedCheckBoxPanel {
    inner: Rc<RefCell<emCheckBox>>,
}

impl PanelBehavior for SharedCheckBoxPanel {
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
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.inner
            .borrow_mut()
            .Input(event, state, input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.inner.borrow().GetCursor()
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Test 1: emCheckButton toggle at 1x and 2x zoom
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_toggle_1x_and_2x() {
    let mut h = PipelineTestHarness::new();

    // 1. Create PipelineTestHarness (800x600 viewport).
    let root = h.get_root_panel();

    // 2. Create emCheckButton (initially unchecked).
    let look = emLook::new();
    let cb = emCheckButton::new(&mut h.sched_ctx(), "Toggle Me", look);
    let cb_ref = Rc::new(RefCell::new(cb));

    // 3. Wrap in PanelBehavior, add to tree, tick + render.
    let _panel_id = h.add_panel_with(
        root,
        "check_button",
        Box::new(SharedCheckButtonPanel {
            inner: cb_ref.clone(),
        }),
    );
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // Verify initial state.
    assert!(
        !cb_ref.borrow().IsChecked(),
        "CheckButton should start unchecked"
    );

    // 4. At 1x: Click center -> assert checked() == true.
    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().IsChecked(),
        "CheckButton should be checked after first click at 1x"
    );

    // 5. Click again -> assert checked() == false (toggle back).
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "CheckButton should be unchecked after second click at 1x"
    );

    // 6. At 2x: set_zoom(2.0), tick, render. Click center -> assert checked() == true.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().IsChecked(),
        "CheckButton should be checked after first click at 2x"
    );

    // 7. Click again -> assert checked() == false.
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "CheckButton should be unchecked after second click at 2x"
    );
}

// ---------------------------------------------------------------------------
// Test 2: emCheckBox toggle at 1x and 2x zoom
// ---------------------------------------------------------------------------

#[test]
fn checkbox_toggle_1x_and_2x() {
    // 1. Create PipelineTestHarness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // 2. Create emCheckBox (initially unchecked).
    let look = emLook::new();
    let cb = emCheckBox::new("Enable Option", look);
    let cb_ref = Rc::new(RefCell::new(cb));

    // 3. Wrap in PanelBehavior, add to tree, tick + render.
    let _panel_id = h.add_panel_with(
        root,
        "check_box",
        Box::new(SharedCheckBoxPanel {
            inner: cb_ref.clone(),
        }),
    );
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // Verify initial state.
    assert!(
        !cb_ref.borrow().IsChecked(),
        "CheckBox should start unchecked"
    );

    // 4. At 1x: Click center -> assert is_checked() == true.
    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().IsChecked(),
        "CheckBox should be checked after first click at 1x"
    );

    // 5. Click again -> assert is_checked() == false (toggle back).
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "CheckBox should be unchecked after second click at 1x"
    );

    // 6. At 2x: set_zoom(2.0), tick, render. Click center -> assert is_checked() == true.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().IsChecked(),
        "CheckBox should be checked after first click at 2x"
    );

    // 7. Click again -> assert is_checked() == false.
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "CheckBox should be unchecked after second click at 2x"
    );
}

// ---------------------------------------------------------------------------
// Helper: create a PipelineTestHarness with a emCheckButton, optionally with
// an on_check callback that records states into a shared Vec.
// ---------------------------------------------------------------------------

fn setup_checkbutton_harness() -> (
    PipelineTestHarness,
    Rc<RefCell<emCheckButton>>,
    emcore::emPanelTree::PanelId,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let look = emLook::new();
    let cb = emCheckButton::new(&mut h.sched_ctx(), "Test", look);
    let cb_ref = Rc::new(RefCell::new(cb));
    let panel_id = h.add_panel_with(
        root,
        "cb",
        Box::new(SharedCheckButtonPanel {
            inner: cb_ref.clone(),
        }),
    );
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);
    (h, cb_ref, panel_id)
}

type CheckButtonRecorder = (
    PipelineTestHarness,
    Rc<RefCell<emCheckButton>>,
    Rc<RefCell<Vec<bool>>>,
    emcore::emPanelTree::PanelId,
);

fn setup_checkbutton_with_recorder() -> CheckButtonRecorder {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let look = emLook::new();
    let mut cb = emCheckButton::new(&mut h.sched_ctx(), "Test", look);
    let states = Rc::new(RefCell::new(Vec::new()));
    let states_clone = states.clone();
    cb.on_check = Some(Box::new(
        move |checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
            states_clone.borrow_mut().push(checked);
        },
    ));
    let cb_ref = Rc::new(RefCell::new(cb));
    let panel_id = h.add_panel_with(
        root,
        "cb",
        Box::new(SharedCheckButtonPanel {
            inner: cb_ref.clone(),
        }),
    );
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);
    (h, cb_ref, states, panel_id)
}

// ---------------------------------------------------------------------------
// Test: Click fires on_check callback with correct new state
// C++ ref: emCheckButton.cpp:Clicked -> SetChecked(!IsChecked()) -> Signal(CheckSignal)
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_click_fires_on_check_callback() {
    let (mut h, _cb_ref, states, _panel_id) = setup_checkbutton_with_recorder();

    // First Click: unchecked -> checked, callback receives true
    h.click(400.0, 300.0);
    assert_eq!(*states.borrow(), vec![true]);

    // Second Click: checked -> unchecked, callback receives false
    h.click(400.0, 300.0);
    assert_eq!(*states.borrow(), vec![true, false]);
}

// ---------------------------------------------------------------------------
// Test: Programmatic set_checked fires on_check callback
// C++ ref: emCheckButton.cpp:SetChecked -> Signal(CheckSignal)
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_set_checked_fires_callback() {
    let mut h = PipelineTestHarness::new();

    let states = Rc::new(RefCell::new(Vec::new()));
    let states_clone = states.clone();

    let look = emLook::new();
    let mut cb = emCheckButton::new(&mut h.sched_ctx(), "Test", look);
    cb.on_check = Some(Box::new(
        move |checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
            states_clone.borrow_mut().push(checked);
        },
    ));

    // Build a sched-reach ctx via PipelineTestHarness so the callback can fire.
    h.with_panel_ctx_sched(|ctx| {
        cb.SetChecked(true, ctx);
    });
    assert_eq!(*states.borrow(), vec![true]);
    h.with_panel_ctx_sched(|ctx| {
        cb.SetChecked(false, ctx);
    });
    assert_eq!(*states.borrow(), vec![true, false]);
}

// ---------------------------------------------------------------------------
// Test: Programmatic set_checked with same GetValue is a no-op (no callback)
// C++ ref: emCheckButton.cpp:SetChecked — early return if Checked==checked
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_set_checked_noop_same_value() {
    let mut h = PipelineTestHarness::new();

    let states = Rc::new(RefCell::new(Vec::new()));
    let states_clone = states.clone();

    let look = emLook::new();
    let mut cb = emCheckButton::new(&mut h.sched_ctx(), "Test", look);
    cb.on_check = Some(Box::new(
        move |checked, _sched: &mut emcore::emEngineCtx::SchedCtx<'_>| {
            states_clone.borrow_mut().push(checked);
        },
    ));
    h.with_panel_ctx_sched(|ctx| {
        // Setting to false when already false -> no callback
        cb.SetChecked(false, ctx);
    });
    assert!(states.borrow().is_empty(), "no callback for same value");

    h.with_panel_ctx_sched(|ctx| {
        cb.SetChecked(true, ctx);
    });
    assert_eq!(states.borrow().len(), 1);

    // Setting to true when already true -> no callback
    h.with_panel_ctx_sched(|ctx| {
        cb.SetChecked(true, ctx);
    });
    assert_eq!(
        states.borrow().len(),
        1,
        "no callback for redundant set_checked(true)"
    );
}

// ---------------------------------------------------------------------------
// Test: Double-Click results in two toggles (on then off)
// C++ ref: emButton.cpp:Input — each press/release Cycle triggers Clicked()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_double_click_toggles_twice() {
    let (mut h, cb_ref, states, _panel_id) = setup_checkbutton_with_recorder();

    // Two clicks in rapid succession
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);

    assert!(
        !cb_ref.borrow().IsChecked(),
        "two clicks should toggle on then off"
    );
    assert_eq!(
        *states.borrow(),
        vec![true, false],
        "callback should record both toggles"
    );
}

// ---------------------------------------------------------------------------
// Test: Enter key toggles checked state (inherited from emButton)
// C++ ref: emButton.cpp:113-119 — Enter press triggers Click()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_enter_key_toggles() {
    let (mut h, cb_ref, states, _panel_id) = setup_checkbutton_with_recorder();

    // First Click to activate the panel (put it in active path for keyboard dispatch)
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().IsChecked());
    states.borrow_mut().clear();

    // Now Enter should toggle checked -> unchecked
    h.press_key(emcore::emInput::InputKey::Enter);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "Enter should toggle checked -> unchecked"
    );
    assert_eq!(*states.borrow(), vec![false]);

    // Enter again toggles back
    h.press_key(emcore::emInput::InputKey::Enter);
    assert!(
        cb_ref.borrow().IsChecked(),
        "Second Enter should toggle unchecked -> checked"
    );
    assert_eq!(*states.borrow(), vec![false, true]);
}

// ---------------------------------------------------------------------------
// Test: Shift+Click is accepted (C++ allows IsShiftMod)
// C++ ref: emButton.cpp:82 — (state.IsNoMod() || state.IsShiftMod())
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_shift_click_accepted() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    // Set Shift in emInputState so dispatch's with_modifiers stamps it onto events
    h.input_state.press(emcore::emInput::InputKey::Shift);
    h.click(400.0, 300.0);
    h.input_state.release(emcore::emInput::InputKey::Shift);

    assert!(
        cb_ref.borrow().IsChecked(),
        "Shift+Click should be accepted and toggle"
    );
}

// ---------------------------------------------------------------------------
// Test: Shift+Enter is accepted (C++ allows IsShiftMod for keyboard too)
// C++ ref: emButton.cpp:115 — (state.IsNoMod() || state.IsShiftMod())
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_shift_enter_accepted() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    // First Click to activate the panel (keyboard dispatch requires active path)
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().IsChecked());

    // Now Shift+Enter should toggle back
    h.input_state.press(emcore::emInput::InputKey::Shift);
    h.press_key(emcore::emInput::InputKey::Enter);
    h.input_state.release(emcore::emInput::InputKey::Shift);

    assert!(
        !cb_ref.borrow().IsChecked(),
        "Shift+Enter should be accepted and toggle"
    );
}

// ---------------------------------------------------------------------------
// Test: Ctrl+Click is rejected
// C++ ref: emButton.cpp:82 — only NoMod or ShiftMod accepted
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_ctrl_click_rejected() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    h.input_state.press(emcore::emInput::InputKey::Ctrl);
    h.click(400.0, 300.0);
    h.input_state.release(emcore::emInput::InputKey::Ctrl);

    assert!(
        !cb_ref.borrow().IsChecked(),
        "Ctrl+Click should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Test: Alt+Click is rejected
// C++ ref: emButton.cpp:82 — only NoMod or ShiftMod accepted
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_alt_click_rejected() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    h.input_state.press(emcore::emInput::InputKey::Alt);
    h.click(400.0, 300.0);
    h.input_state.release(emcore::emInput::InputKey::Alt);

    assert!(!cb_ref.borrow().IsChecked(), "Alt+Click should be rejected");
}

// ---------------------------------------------------------------------------
// Test: Meta+Click is rejected
// C++ ref: emButton.cpp:82 — only NoMod or ShiftMod accepted
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_meta_click_rejected() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    h.input_state.press(emcore::emInput::InputKey::Meta);
    h.click(400.0, 300.0);
    h.input_state.release(emcore::emInput::InputKey::Meta);

    assert!(
        !cb_ref.borrow().IsChecked(),
        "Meta+Click should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Test: Ctrl+Enter is rejected
// C++ ref: emButton.cpp:115 — only NoMod or ShiftMod accepted for Enter
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_ctrl_enter_rejected() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    // Activate via Click first
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().IsChecked());

    h.input_state.press(emcore::emInput::InputKey::Ctrl);
    h.press_key(emcore::emInput::InputKey::Enter);
    h.input_state.release(emcore::emInput::InputKey::Ctrl);

    assert!(
        cb_ref.borrow().IsChecked(),
        "Ctrl+Enter should be rejected (state unchanged from checked)"
    );
}

// ---------------------------------------------------------------------------
// Test: Alt+Enter is rejected
// C++ ref: emButton.cpp:115 — only NoMod or ShiftMod accepted for Enter
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_alt_enter_rejected() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    // Activate via Click first
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().IsChecked());

    h.input_state.press(emcore::emInput::InputKey::Alt);
    h.press_key(emcore::emInput::InputKey::Enter);
    h.input_state.release(emcore::emInput::InputKey::Alt);

    assert!(
        cb_ref.borrow().IsChecked(),
        "Alt+Enter should be rejected (state unchanged from checked)"
    );
}

// ---------------------------------------------------------------------------
// Test: Disabled emCheckButton rejects mouse Click
// C++ ref: emButton.cpp:55 — Click() gates on IsEnabled()
//          emButton.cpp:83 — Input() gates on IsEnabled()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_disabled_rejects_click() {
    let (mut h, cb_ref, panel_id) = setup_checkbutton_harness();

    // Disable the panel via the tree's enable switch
    h.tree.SetEnableSwitch(panel_id, false, None);
    h.tick_n(3);
    // Re-render so the emCheckButton's cached `enabled` field updates
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "Disabled CheckButton should reject click"
    );
}

// ---------------------------------------------------------------------------
// Test: Disabled emCheckButton rejects Enter key
// C++ ref: emButton.cpp:116 — Enter gated on IsEnabled()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_disabled_rejects_enter() {
    let (mut h, cb_ref, panel_id) = setup_checkbutton_harness();

    h.tree.SetEnableSwitch(panel_id, false, None);
    h.tick_n(3);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.press_key(emcore::emInput::InputKey::Enter);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "Disabled CheckButton should reject Enter"
    );
}

// ---------------------------------------------------------------------------
// Test: Re-enabled emCheckButton accepts Input after being disabled
// C++ ref: emButton.cpp:55,83,116 — IsEnabled() gates all Input
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_reenable_accepts_input() {
    let (mut h, cb_ref, panel_id) = setup_checkbutton_harness();

    // Disable
    h.tree.SetEnableSwitch(panel_id, false, None);
    h.tick_n(3);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(!cb_ref.borrow().IsChecked(), "disabled: click rejected");

    // Re-enable
    h.tree.SetEnableSwitch(panel_id, true, None);
    h.tick_n(3);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().IsChecked(),
        "re-enabled: click should toggle"
    );
}

// ---------------------------------------------------------------------------
// Test: Click outside the button face does not toggle
// C++ ref: emButton.cpp:83 — CheckMouse() hit test gates press
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_click_outside_no_toggle() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    // Click far outside (top-left corner, well outside content area)
    h.click(1.0, 1.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "Click outside button face should not toggle"
    );
}

// ---------------------------------------------------------------------------
// Test: Press inside, release outside does not toggle (drag-off cancels)
// C++ ref: emButton.cpp:101 — CheckMouse() on release gates Click()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_press_inside_release_outside_no_toggle() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    // Press inside, drag to far outside, release outside
    h.drag(400.0, 300.0, 1.0, 1.0);
    assert!(
        !cb_ref.borrow().IsChecked(),
        "Press inside + release outside should not toggle"
    );
}

// ---------------------------------------------------------------------------
// Test: Initial state is unchecked (default construction)
// C++ ref: emCheckButton.cpp:30 — Checked=false in constructor
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_initial_state_unchecked() {
    let mut h = PipelineTestHarness::new();
    let look = emLook::new();
    let cb = emCheckButton::new(&mut h.sched_ctx(), "Test", look);
    assert!(
        !cb.IsChecked(),
        "CheckButton should be unchecked on construction"
    );
}
