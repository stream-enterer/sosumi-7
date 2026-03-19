//! Systematic interaction tests for CheckButton and CheckBox at 1x and 2x zoom,
//! driven through the full input dispatch pipeline (PipelineTestHarness).
//!
//! These tests verify that mouse clicks toggle the checked state correctly when
//! dispatched through the coordinate-transform pipeline at different zoom levels.


use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::input::{Cursor, InputEvent, InputState};
use zuicchini::panel::{PanelBehavior, PanelState};
use zuicchini::render::{Painter, SoftwareCompositor};
use zuicchini::widget::{CheckBox, CheckButton, Look};

use super::support::pipeline::PipelineTestHarness;

// ---------------------------------------------------------------------------
// PanelBehavior wrapper for CheckButton (shared via Rc<RefCell>)
// ---------------------------------------------------------------------------

struct SharedCheckButtonPanel {
    inner: Rc<RefCell<CheckButton>>,
}

impl PanelBehavior for SharedCheckButtonPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.inner.borrow_mut().paint(painter, w, h, state.enabled);
    }

    fn input(
        &mut self,
        event: &InputEvent,
        state: &PanelState,
        input_state: &InputState,
    ) -> bool {
        self.inner.borrow_mut().input(event, state, input_state)
    }

    fn get_cursor(&self) -> Cursor {
        self.inner.borrow().get_cursor()
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// PanelBehavior wrapper for CheckBox (shared via Rc<RefCell>)
// ---------------------------------------------------------------------------

struct SharedCheckBoxPanel {
    inner: Rc<RefCell<CheckBox>>,
}

impl PanelBehavior for SharedCheckBoxPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.inner.borrow_mut().paint(painter, w, h, state.enabled);
    }

    fn input(
        &mut self,
        event: &InputEvent,
        state: &PanelState,
        input_state: &InputState,
    ) -> bool {
        self.inner.borrow_mut().input(event, state, input_state)
    }

    fn get_cursor(&self) -> Cursor {
        self.inner.borrow().get_cursor()
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Test 1: CheckButton toggle at 1x and 2x zoom
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_toggle_1x_and_2x() {
    // 1. Create PipelineTestHarness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    // 2. Create CheckButton (initially unchecked).
    let look = Look::new();
    let cb = CheckButton::new("Toggle Me", look);
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
        !cb_ref.borrow().is_checked(),
        "CheckButton should start unchecked"
    );

    // 4. At 1x: click center -> assert checked() == true.
    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().is_checked(),
        "CheckButton should be checked after first click at 1x"
    );

    // 5. Click again -> assert checked() == false (toggle back).
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().is_checked(),
        "CheckButton should be unchecked after second click at 1x"
    );

    // 6. At 2x: set_zoom(2.0), tick, render. Click center -> assert checked() == true.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().is_checked(),
        "CheckButton should be checked after first click at 2x"
    );

    // 7. Click again -> assert checked() == false.
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().is_checked(),
        "CheckButton should be unchecked after second click at 2x"
    );
}

// ---------------------------------------------------------------------------
// Test 2: CheckBox toggle at 1x and 2x zoom
// ---------------------------------------------------------------------------

#[test]
fn checkbox_toggle_1x_and_2x() {
    // 1. Create PipelineTestHarness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    // 2. Create CheckBox (initially unchecked).
    let look = Look::new();
    let cb = CheckBox::new("Enable Option", look);
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
        !cb_ref.borrow().is_checked(),
        "CheckBox should start unchecked"
    );

    // 4. At 1x: click center -> assert is_checked() == true.
    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().is_checked(),
        "CheckBox should be checked after first click at 1x"
    );

    // 5. Click again -> assert is_checked() == false (toggle back).
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().is_checked(),
        "CheckBox should be unchecked after second click at 1x"
    );

    // 6. At 2x: set_zoom(2.0), tick, render. Click center -> assert is_checked() == true.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().is_checked(),
        "CheckBox should be checked after first click at 2x"
    );

    // 7. Click again -> assert is_checked() == false.
    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().is_checked(),
        "CheckBox should be unchecked after second click at 2x"
    );
}

// ---------------------------------------------------------------------------
// Helper: create a PipelineTestHarness with a CheckButton, optionally with
// an on_check callback that records states into a shared Vec.
// ---------------------------------------------------------------------------

fn setup_checkbutton_harness() -> (
    PipelineTestHarness,
    Rc<RefCell<CheckButton>>,
    zuicchini::panel::PanelId,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.root();
    let look = Look::new();
    let cb = CheckButton::new("Test", look);
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

fn setup_checkbutton_with_recorder() -> (
    PipelineTestHarness,
    Rc<RefCell<CheckButton>>,
    Rc<RefCell<Vec<bool>>>,
    zuicchini::panel::PanelId,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.root();
    let look = Look::new();
    let mut cb = CheckButton::new("Test", look);
    let states = Rc::new(RefCell::new(Vec::new()));
    let states_clone = states.clone();
    cb.on_check = Some(Box::new(move |checked| {
        states_clone.borrow_mut().push(checked);
    }));
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

    // First click: unchecked -> checked, callback receives true
    h.click(400.0, 300.0);
    assert_eq!(*states.borrow(), vec![true]);

    // Second click: checked -> unchecked, callback receives false
    h.click(400.0, 300.0);
    assert_eq!(*states.borrow(), vec![true, false]);
}

// ---------------------------------------------------------------------------
// Test: Programmatic set_checked fires on_check callback
// C++ ref: emCheckButton.cpp:SetChecked -> Signal(CheckSignal)
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_set_checked_fires_callback() {
    let states = Rc::new(RefCell::new(Vec::new()));
    let states_clone = states.clone();

    let look = Look::new();
    let mut cb = CheckButton::new("Test", look);
    cb.on_check = Some(Box::new(move |checked| {
        states_clone.borrow_mut().push(checked);
    }));

    // set_checked(true) from false -> fires callback with true
    cb.set_checked(true);
    assert_eq!(*states.borrow(), vec![true]);

    // set_checked(false) from true -> fires callback with false
    cb.set_checked(false);
    assert_eq!(*states.borrow(), vec![true, false]);
}

// ---------------------------------------------------------------------------
// Test: Programmatic set_checked with same value is a no-op (no callback)
// C++ ref: emCheckButton.cpp:SetChecked — early return if Checked==checked
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_set_checked_noop_same_value() {
    let states = Rc::new(RefCell::new(Vec::new()));
    let states_clone = states.clone();

    let look = Look::new();
    let mut cb = CheckButton::new("Test", look);
    cb.on_check = Some(Box::new(move |checked| {
        states_clone.borrow_mut().push(checked);
    }));

    // Setting to false when already false -> no callback
    cb.set_checked(false);
    assert!(states.borrow().is_empty(), "no callback for same value");

    cb.set_checked(true);
    assert_eq!(states.borrow().len(), 1);

    // Setting to true when already true -> no callback
    cb.set_checked(true);
    assert_eq!(
        states.borrow().len(),
        1,
        "no callback for redundant set_checked(true)"
    );
}

// ---------------------------------------------------------------------------
// Test: Double-click results in two toggles (on then off)
// C++ ref: emButton.cpp:Input — each press/release cycle triggers Clicked()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_double_click_toggles_twice() {
    let (mut h, cb_ref, states, _panel_id) = setup_checkbutton_with_recorder();

    // Two clicks in rapid succession
    h.click(400.0, 300.0);
    h.click(400.0, 300.0);

    assert!(
        !cb_ref.borrow().is_checked(),
        "two clicks should toggle on then off"
    );
    assert_eq!(
        *states.borrow(),
        vec![true, false],
        "callback should record both toggles"
    );
}

// ---------------------------------------------------------------------------
// Test: Enter key toggles checked state (inherited from Button)
// C++ ref: emButton.cpp:113-119 — Enter press triggers Click()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_enter_key_toggles() {
    let (mut h, cb_ref, states, _panel_id) = setup_checkbutton_with_recorder();

    // First click to activate the panel (put it in active path for keyboard dispatch)
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().is_checked());
    states.borrow_mut().clear();

    // Now Enter should toggle checked -> unchecked
    h.press_key(zuicchini::input::InputKey::Enter);
    assert!(
        !cb_ref.borrow().is_checked(),
        "Enter should toggle checked -> unchecked"
    );
    assert_eq!(*states.borrow(), vec![false]);

    // Enter again toggles back
    h.press_key(zuicchini::input::InputKey::Enter);
    assert!(
        cb_ref.borrow().is_checked(),
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

    // Set Shift in InputState so dispatch's with_modifiers stamps it onto events
    h.input_state.press(zuicchini::input::InputKey::Shift);
    h.click(400.0, 300.0);
    h.input_state.release(zuicchini::input::InputKey::Shift);

    assert!(
        cb_ref.borrow().is_checked(),
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

    // First click to activate the panel (keyboard dispatch requires active path)
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().is_checked());

    // Now Shift+Enter should toggle back
    h.input_state.press(zuicchini::input::InputKey::Shift);
    h.press_key(zuicchini::input::InputKey::Enter);
    h.input_state.release(zuicchini::input::InputKey::Shift);

    assert!(
        !cb_ref.borrow().is_checked(),
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

    h.input_state.press(zuicchini::input::InputKey::Ctrl);
    h.click(400.0, 300.0);
    h.input_state.release(zuicchini::input::InputKey::Ctrl);

    assert!(
        !cb_ref.borrow().is_checked(),
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

    h.input_state.press(zuicchini::input::InputKey::Alt);
    h.click(400.0, 300.0);
    h.input_state.release(zuicchini::input::InputKey::Alt);

    assert!(
        !cb_ref.borrow().is_checked(),
        "Alt+Click should be rejected"
    );
}

// ---------------------------------------------------------------------------
// Test: Meta+Click is rejected
// C++ ref: emButton.cpp:82 — only NoMod or ShiftMod accepted
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_meta_click_rejected() {
    let (mut h, cb_ref, _panel_id) = setup_checkbutton_harness();

    h.input_state.press(zuicchini::input::InputKey::Meta);
    h.click(400.0, 300.0);
    h.input_state.release(zuicchini::input::InputKey::Meta);

    assert!(
        !cb_ref.borrow().is_checked(),
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

    // Activate via click first
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().is_checked());

    h.input_state.press(zuicchini::input::InputKey::Ctrl);
    h.press_key(zuicchini::input::InputKey::Enter);
    h.input_state.release(zuicchini::input::InputKey::Ctrl);

    assert!(
        cb_ref.borrow().is_checked(),
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

    // Activate via click first
    h.click(400.0, 300.0);
    assert!(cb_ref.borrow().is_checked());

    h.input_state.press(zuicchini::input::InputKey::Alt);
    h.press_key(zuicchini::input::InputKey::Enter);
    h.input_state.release(zuicchini::input::InputKey::Alt);

    assert!(
        cb_ref.borrow().is_checked(),
        "Alt+Enter should be rejected (state unchanged from checked)"
    );
}

// ---------------------------------------------------------------------------
// Test: Disabled CheckButton rejects mouse click
// C++ ref: emButton.cpp:55 — Click() gates on IsEnabled()
//          emButton.cpp:83 — Input() gates on IsEnabled()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_disabled_rejects_click() {
    let (mut h, cb_ref, panel_id) = setup_checkbutton_harness();

    // Disable the panel via the tree's enable switch
    h.tree.set_enable_switch(panel_id, false);
    h.tick_n(3);
    // Re-render so the CheckButton's cached `enabled` field updates
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        !cb_ref.borrow().is_checked(),
        "Disabled CheckButton should reject click"
    );
}

// ---------------------------------------------------------------------------
// Test: Disabled CheckButton rejects Enter key
// C++ ref: emButton.cpp:116 — Enter gated on IsEnabled()
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_disabled_rejects_enter() {
    let (mut h, cb_ref, panel_id) = setup_checkbutton_harness();

    h.tree.set_enable_switch(panel_id, false);
    h.tick_n(3);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.press_key(zuicchini::input::InputKey::Enter);
    assert!(
        !cb_ref.borrow().is_checked(),
        "Disabled CheckButton should reject Enter"
    );
}

// ---------------------------------------------------------------------------
// Test: Re-enabled CheckButton accepts input after being disabled
// C++ ref: emButton.cpp:55,83,116 — IsEnabled() gates all input
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_reenable_accepts_input() {
    let (mut h, cb_ref, panel_id) = setup_checkbutton_harness();

    // Disable
    h.tree.set_enable_switch(panel_id, false);
    h.tick_n(3);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(!cb_ref.borrow().is_checked(), "disabled: click rejected");

    // Re-enable
    h.tree.set_enable_switch(panel_id, true);
    h.tick_n(3);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert!(
        cb_ref.borrow().is_checked(),
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
        !cb_ref.borrow().is_checked(),
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
        !cb_ref.borrow().is_checked(),
        "Press inside + release outside should not toggle"
    );
}

// ---------------------------------------------------------------------------
// Test: Initial state is unchecked (default construction)
// C++ ref: emCheckButton.cpp:30 — Checked=false in constructor
// ---------------------------------------------------------------------------

#[test]
fn checkbutton_initial_state_unchecked() {
    let look = Look::new();
    let cb = CheckButton::new("Test", look);
    assert!(
        !cb.is_checked(),
        "CheckButton should be unchecked on construction"
    );
}
