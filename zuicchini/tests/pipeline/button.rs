//! Systematic interaction test for Button at 1x and 2x zoom, driven through
//! the full input dispatch pipeline (PipelineTestHarness).


use std::cell::Cell;
use std::rc::Rc;

use zuicchini::input::{Cursor, InputEvent, InputKey, InputState};
use zuicchini::panel::{PanelBehavior, PanelState};
use zuicchini::render::{Painter, SoftwareCompositor};
use zuicchini::widget::{Button, Look};

use super::support::pipeline::PipelineTestHarness;

/// Minimal PanelBehavior wrapper for Button so it can be installed into the
/// panel tree. Delegates paint/input to the underlying widget.
struct ButtonPanel {
    widget: Button,
}

impl PanelBehavior for ButtonPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.widget.paint(painter, w, h, state.enabled);
    }

    fn input(
        &mut self,
        event: &InputEvent,
        state: &PanelState,
        input_state: &InputState,
    ) -> bool {
        self.widget.input(event, state, input_state)
    }

    fn get_cursor(&self) -> Cursor {
        Cursor::Normal
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

#[test]
fn button_click_1x_and_2x() {
    // 1. Create PipelineTestHarness (800x600 viewport).
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    // 2. Create Button with on_click callback incrementing a shared counter.
    let counter = Rc::new(Cell::new(0u32));
    let counter_clone = counter.clone();

    let look = Look::new();
    let mut btn = Button::new("Systematic Test", look);
    btn.on_click = Some(Box::new(move || {
        counter_clone.set(counter_clone.get() + 1);
    }));

    // 3. Wrap in PanelBehavior and add to tree.
    let _panel_id = h.add_panel_with(root, "button", Box::new(ButtonPanel { widget: btn }));

    // 4. Tick + render (SoftwareCompositor) to populate last_w/last_h.
    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // 5. At 1x zoom: click at viewport center (400, 300).
    h.click(400.0, 300.0);
    assert_eq!(
        counter.get(),
        1,
        "Button callback should have fired once after click at 1x zoom"
    );

    // 6. At 2x zoom: set_zoom, tick, re-render, then click at viewport center.
    h.set_zoom(2.0);
    h.tick_n(5);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);
    assert_eq!(
        counter.get(),
        2,
        "Button callback should have fired again after click at 2x zoom"
    );
}

// ---------------------------------------------------------------------------
// BP-9: Button state machine behavioral parity tests
// C++ ref: emButton.cpp Input() lines 73-123
// ---------------------------------------------------------------------------

/// Helper: create a PipelineTestHarness with a Button installed, returning the
/// harness, panel id, click counter, and press-state log.
///
/// The press-state log records `true` on press, `false` on release, so
/// `[true, false]` means one full press-release cycle.
fn make_button_harness() -> (
    PipelineTestHarness,
    zuicchini::panel::PanelId,
    Rc<Cell<u32>>,
    Rc<std::cell::RefCell<Vec<bool>>>,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let counter = Rc::new(Cell::new(0u32));
    let press_log: Rc<std::cell::RefCell<Vec<bool>>> = Rc::new(std::cell::RefCell::new(Vec::new()));

    let counter_c = counter.clone();
    let press_log_c = press_log.clone();

    let look = Look::new();
    let mut btn = Button::new("BP9", look);
    btn.on_click = Some(Box::new(move || {
        counter_c.set(counter_c.get() + 1);
    }));
    btn.on_press_state = Some(Box::new(move |pressed| {
        press_log_c.borrow_mut().push(pressed);
    }));

    let panel_id = h.add_panel_with(root, "button", Box::new(ButtonPanel { widget: btn }));

    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    (h, panel_id, counter, press_log)
}

/// C++ emButton.cpp:86-92: Mouse left press → Pressed=true, PressStateChanged.
#[test]
fn bp9_mouse_press_enters_pressed_state() {
    let (mut h, _panel_id, _counter, press_log) = make_button_harness();

    // Dispatch only a press (no release) at viewport center.
    let press = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);

    let log = press_log.borrow();
    assert_eq!(log.len(), 1, "press_state callback should fire once on press");
    assert!(log[0], "press_state callback should report pressed=true");
}

/// C++ emButton.cpp:95-110: Mouse release inside → Click fires.
#[test]
fn bp9_mouse_release_inside_fires_click() {
    let (mut h, _panel_id, counter, press_log) = make_button_harness();

    // Full click at viewport center.
    h.click(400.0, 300.0);

    assert_eq!(counter.get(), 1, "click callback should fire on release inside");
    let log = press_log.borrow();
    assert_eq!(log.len(), 2, "should have press + release state changes");
    assert!(log[0], "first state change = pressed");
    assert!(!log[1], "second state change = released");
}

/// C++ emButton.cpp:95-110: Mouse release outside → no Click.
/// Press at center, release far outside the button.
#[test]
fn bp9_mouse_release_outside_no_click() {
    let (mut h, _panel_id, counter, press_log) = make_button_harness();

    // Press at viewport center (inside button).
    let press = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);
    assert_eq!(counter.get(), 0, "no click on press alone");

    // Release far outside the button (top-left corner of viewport, well
    // outside the panel face area).
    let release = InputEvent::release(InputKey::MouseLeft).with_mouse(0.0, 0.0);
    h.dispatch(&release);

    assert_eq!(counter.get(), 0, "no click when release is outside button");
    let log = press_log.borrow();
    assert_eq!(log.len(), 2, "should still get press + release state changes");
    assert!(log[0], "pressed on press");
    assert!(!log[1], "released on release");
}

/// C++ emButton.cpp:113-119: Enter key → instant Click(), no press state.
/// The panel must be in the active path for keyboard events to reach it.
#[test]
fn bp9_enter_key_instant_click_no_press_state() {
    let (mut h, _panel_id, counter, press_log) = make_button_harness();

    // First click to activate the panel (put it in the active path).
    h.click(400.0, 300.0);
    assert_eq!(counter.get(), 1, "setup click");
    press_log.borrow_mut().clear();

    // Now press Enter — should fire click without any press state changes.
    h.press_key(InputKey::Enter);

    assert_eq!(counter.get(), 2, "Enter key should fire click");
    assert!(
        press_log.borrow().is_empty(),
        "Enter key should NOT trigger press state changes (C++ instant Click, no Pressed=true)"
    );
}

/// C++ emButton.cpp:81-82: Only NoMod or ShiftMod accepted. Ctrl rejects.
/// The harness dispatch() stamps modifiers from input_state onto events, so
/// we must set the modifier key in input_state to propagate it correctly.
#[test]
fn bp9_ctrl_click_rejected() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    h.input_state.press(InputKey::Ctrl);
    let press = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    let release = InputEvent::release(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);
    h.dispatch(&release);
    h.input_state.release(InputKey::Ctrl);

    assert_eq!(counter.get(), 0, "Ctrl+click should NOT fire click callback");
}

/// C++ emButton.cpp:81-82: Alt modifier rejects.
#[test]
fn bp9_alt_click_rejected() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    h.input_state.press(InputKey::Alt);
    let press = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    let release = InputEvent::release(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);
    h.dispatch(&release);
    h.input_state.release(InputKey::Alt);

    assert_eq!(counter.get(), 0, "Alt+click should NOT fire click callback");
}

/// C++ emButton.cpp:81-82: Meta modifier rejects.
#[test]
fn bp9_meta_click_rejected() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    h.input_state.press(InputKey::Meta);
    let press = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    let release = InputEvent::release(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);
    h.dispatch(&release);
    h.input_state.release(InputKey::Meta);

    assert_eq!(counter.get(), 0, "Meta+click should NOT fire click callback");
}

/// C++ emButton.cpp:81-82: Shift modifier IS accepted (state.IsShiftMod()).
#[test]
fn bp9_shift_click_accepted() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    h.input_state.press(InputKey::Shift);
    let press = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    let release = InputEvent::release(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&press);
    h.dispatch(&release);
    h.input_state.release(InputKey::Shift);

    assert_eq!(counter.get(), 1, "Shift+click SHOULD fire click callback");
}

/// C++ emButton.cpp:114-115: Enter with Ctrl is rejected.
#[test]
fn bp9_ctrl_enter_rejected() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    // Activate the panel first so keyboard events reach it.
    h.click(400.0, 300.0);
    assert_eq!(counter.get(), 1, "setup click");

    h.input_state.press(InputKey::Ctrl);
    let press = InputEvent::press(InputKey::Enter);
    h.dispatch(&press);
    h.input_state.release(InputKey::Ctrl);

    assert_eq!(counter.get(), 1, "Ctrl+Enter should NOT fire click");
}

/// C++ emButton.cpp:114-115: Enter with Shift IS accepted.
#[test]
fn bp9_shift_enter_accepted() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    // Activate the panel first so keyboard events reach it.
    h.click(400.0, 300.0);
    assert_eq!(counter.get(), 1, "setup click");

    h.input_state.press(InputKey::Shift);
    let press = InputEvent::press(InputKey::Enter);
    h.dispatch(&press);
    h.input_state.release(InputKey::Shift);

    assert_eq!(counter.get(), 2, "Shift+Enter SHOULD fire click");
}

/// C++ emButton.cpp:83: IsEnabled() gates press. Disabled button ignores input.
#[test]
fn bp9_disabled_button_ignores_press() {
    let (mut h, panel_id, counter, press_log) = make_button_harness();

    // Disable the panel via the tree's enable_switch mechanism.
    h.tree.set_enable_switch(panel_id, false);
    h.tick_n(3);
    // Re-render so Button.paint() caches enabled=false.
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.click(400.0, 300.0);

    assert_eq!(counter.get(), 0, "disabled button should not fire click");
    assert!(
        press_log.borrow().is_empty(),
        "disabled button should not enter pressed state"
    );
}

/// C++ emButton.cpp:116: Enter on disabled button does not fire.
#[test]
fn bp9_disabled_button_ignores_enter() {
    let (mut h, panel_id, counter, _press_log) = make_button_harness();

    h.tree.set_enable_switch(panel_id, false);
    h.tick_n(3);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    h.press_key(InputKey::Enter);

    assert_eq!(counter.get(), 0, "disabled button should not fire click on Enter");
}

/// C++ emButton.cpp:84: GetViewCondition(VCT_MIN_EXT) >= 8.0. When the panel
/// is zoomed out so its viewed extent is < 8 pixels, mouse input is ignored.
///
/// We give the button a tiny layout rect (0.001 x 0.001 of the root) so its
/// viewed_rect dimensions are well under 8 pixels at 1x zoom (800*0.001 = 0.8px).
#[test]
fn bp9_vct_min_ext_guard_mouse() {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let counter = Rc::new(Cell::new(0u32));
    let counter_c = counter.clone();

    let look = Look::new();
    let mut btn = Button::new("Tiny", look);
    btn.on_click = Some(Box::new(move || {
        counter_c.set(counter_c.get() + 1);
    }));

    let panel_id = h.tree.create_child(root, "button");
    h.tree.set_focusable(panel_id, true);
    // Tiny layout: 0.1% of root in each dimension → ~0.8px at 800x600.
    h.tree.set_layout_rect(panel_id, 0.0, 0.0, 0.001, 0.001);
    h.tree
        .set_behavior(panel_id, Box::new(ButtonPanel { widget: btn }));

    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // Click near top-left where the tiny panel lives.
    h.click(0.4, 0.3);

    assert_eq!(
        counter.get(),
        0,
        "button too small (VCT_MIN_EXT < 8) should not respond to mouse"
    );
}

/// C++ emButton.cpp:116: Enter also gated on VCT_MIN_EXT >= 8.0.
///
/// Same tiny layout rect approach as the mouse test above.
#[test]
fn bp9_vct_min_ext_guard_enter() {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let counter = Rc::new(Cell::new(0u32));
    let counter_c = counter.clone();

    let look = Look::new();
    let mut btn = Button::new("Tiny", look);
    btn.on_click = Some(Box::new(move || {
        counter_c.set(counter_c.get() + 1);
    }));

    let panel_id = h.tree.create_child(root, "button");
    h.tree.set_focusable(panel_id, true);
    h.tree.set_layout_rect(panel_id, 0.0, 0.0, 0.001, 0.001);
    h.tree
        .set_behavior(panel_id, Box::new(ButtonPanel { widget: btn }));

    h.tick_n(5);
    let mut compositor = SoftwareCompositor::new(800, 600);
    compositor.render(&mut h.tree, &h.view);

    // Activate the panel, then press Enter.
    // Use set_active_panel directly since the panel is too small to click.
    h.view.set_active_panel(&mut h.tree, panel_id, false);
    h.tick_n(1);

    h.press_key(InputKey::Enter);

    assert_eq!(
        counter.get(),
        0,
        "button too small (VCT_MIN_EXT < 8) should not respond to Enter"
    );
}

/// C++ emButton.cpp:95: Release without prior press is a no-op.
#[test]
fn bp9_release_without_press_is_noop() {
    let (mut h, _panel_id, counter, press_log) = make_button_harness();

    // Dispatch release without press.
    let release = InputEvent::release(InputKey::MouseLeft).with_mouse(400.0, 300.0);
    h.dispatch(&release);

    assert_eq!(counter.get(), 0, "release without press should not fire click");
    assert!(
        press_log.borrow().is_empty(),
        "release without press should not fire press_state"
    );
}

/// C++ emButton.cpp: Space key does NOT activate button (only Enter does).
#[test]
fn bp9_space_key_does_not_activate() {
    let (mut h, _panel_id, counter, _press_log) = make_button_harness();

    // Activate the panel first so keyboard events reach it.
    h.click(400.0, 300.0);
    assert_eq!(counter.get(), 1, "setup click");

    h.press_key(InputKey::Space);

    assert_eq!(counter.get(), 1, "Space should NOT activate button (C++ only handles Enter)");
}
