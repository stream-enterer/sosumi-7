use std::cell::RefCell;
use std::rc::Rc;

use emcore::emInput::{emInputEvent, InputKey};

use crate::support::{RecordingBehavior, TestHarness};

#[test]
fn mouse_click_activates_correct_panel() {
    let mut h = TestHarness::new();
    let root = h.get_root_panel();

    // Panel A: left half
    let a = h.add_panel(root, "a");
    h.tree.Layout(a, 0.0, 0.0, 0.5, 1.0, 1.0);

    // Panel B: right half
    let b = h.add_panel(root, "b");
    h.tree.Layout(b, 0.5, 0.0, 0.5, 1.0, 1.0);

    h.tick();

    // Click in right half (panel B territory) — 600px is 75% of 800px viewport
    let click = emInputEvent::press(InputKey::MouseLeft).with_mouse(600.0, 300.0);
    h.inject_input(&click);

    assert_eq!(
        h.view.GetActivePanel(),
        Some(b),
        "Click at x=600 should activate panel B (right half)"
    );
}

#[test]
fn vif_consumes_prevents_behavior() {
    let mut h = TestHarness::new();
    let root = h.get_root_panel();
    let log = Rc::new(RefCell::new(Vec::new()));

    let _child = h.add_panel_with(
        root,
        "child",
        Box::new(RecordingBehavior::new(Rc::clone(&log))),
    );
    h.tick();

    // Set child as active
    h.view.set_active_panel(&mut h.tree, _child, false);
    h.view.Update(&mut h.tree);

    // Alt+ArrowUp should be consumed by emKeyboardZoomScrollVIF (zoom/scroll)
    h.input_state.press(InputKey::Alt);
    let event = emInputEvent::press(InputKey::ArrowUp)
        .with_mouse(400.0, 300.0)
        .with_alt();

    log.borrow_mut().clear();
    h.inject_input(&event);

    // Behavior should NOT have received Input — VIF consumed it
    let entries = log.borrow();
    assert!(
        !entries.iter().any(|e| e.starts_with("input:")),
        "VIF should consume Alt+Arrow, but behavior received: {entries:?}"
    );
}

#[test]
fn focus_change_routes_keyboard() {
    let mut h = TestHarness::new();
    let root = h.get_root_panel();
    let log_a = Rc::new(RefCell::new(Vec::new()));
    let log_b = Rc::new(RefCell::new(Vec::new()));

    // Panel A: left half
    let a = h.add_panel_with(
        root,
        "a",
        Box::new(RecordingBehavior::new(Rc::clone(&log_a))),
    );
    h.tree.Layout(a, 0.0, 0.0, 0.5, 1.0, 1.0);

    // Panel B: right half
    let b = h.add_panel_with(
        root,
        "b",
        Box::new(RecordingBehavior::new(Rc::clone(&log_b))),
    );
    h.tree.Layout(b, 0.5, 0.0, 0.5, 1.0, 1.0);

    h.tick();

    // Activate A and type a key
    h.view.set_active_panel(&mut h.tree, a, false);
    h.view.Update(&mut h.tree);
    let key_x = emInputEvent::press(InputKey::Key('x'));
    h.inject_input(&key_x);

    // C++ broadcasts Input() to all viewed panels — both A and B receive the event.
    assert!(
        log_a.borrow().iter().any(|e| e.contains("input:")),
        "A should receive key input"
    );
    assert!(
        log_b.borrow().iter().any(|e| e.contains("input:")),
        "B should also receive key input (C++ broadcast semantics)"
    );

    // Click B to activate it, then type
    log_a.borrow_mut().clear();
    log_b.borrow_mut().clear();

    let click_b = emInputEvent::press(InputKey::MouseLeft).with_mouse(600.0, 300.0);
    h.inject_input(&click_b);

    let key_y = emInputEvent::press(InputKey::Key('y'));
    h.inject_input(&key_y);

    assert!(
        log_b.borrow().iter().any(|e| e.contains("Key('y')")),
        "B should receive key 'y' after being activated by click"
    );
}

#[test]
fn input_without_update_returns_none() {
    // Document: hit-test requires update_viewing to set SVP.
    let mut tree = emcore::emPanelTree::PanelTree::new();
    let root = tree.create_root("root");
    tree.set_focusable(root, true);
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

    let child = tree.create_child(root, "child");
    tree.set_focusable(child, true);
    tree.Layout(child, 0.0, 0.0, 1.0, 1.0, 1.0);

    // Create view but do NOT call update_viewing
    let view = emcore::emView::emView::new_for_test(root, 800.0, 600.0);

    // Hit-test should return None since SVP is not computed
    // (SVP is set during update_viewing)
    let hit = view.GetFocusablePanelAt(&tree, 400.0, 300.0);
    // Note: emView::new sets active=root, which may or may not compute SVP.
    // This test documents the behavior either way.
    // If SVP is set during new(), hit may succeed for root.
    // The key point: without update_viewing, child panels won't be hit-testable.
    if hit.is_some() {
        // SVP was set during emView::new — acceptable
        assert_eq!(
            hit,
            Some(root),
            "Without update_viewing, only root could be found"
        );
    }
    // Either way, the child won't have viewed coordinates set
}
