//! Phase 5 acceptance test (emview-rewrite-followups).
//!
//! Verifies that input events flow through the C++ chain:
//! `emViewPort::InputToView` → `emView::Input`.
//!
//! A real `emWindow` requires a `winit::ActiveEventLoop` + `GpuContext`
//! which cannot be constructed from a unit test. Per the plan's
//! escalation clause, the test instantiates `emViewPort` and `emView`
//! directly and verifies the dispatch path.

use emcore::test_view_harness::TestSched;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPanelTree::PanelTree;
use emcore::emView::emView;

#[test]
fn input_routes_through_viewport() {
    let mut ts = TestSched::new();
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_focusable(root, true);
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    let event = emInputEvent::press(InputKey::MouseLeft).with_mouse(10.0, 10.0);
    let state = emInputState::new();

    let vp_rc = view.CurrentViewPort.clone();
    let count_before = vp_rc.borrow().input_event_count;
    ts.with(|sc| {
        vp_rc
            .borrow_mut()
            .InputToView(&mut view, &mut tree, &event, &state, sc);
    });
    let count_after = vp_rc.borrow().input_event_count;

    assert_eq!(
        count_after,
        count_before + 1,
        "InputToView did not run — chain is broken"
    );
}

#[test]
fn input_to_view_updates_last_mouse_position() {
    let mut ts = TestSched::new();
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_focusable(root, true);
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    let event = emInputEvent::mouse_move(InputKey::MouseLeft, 42.0, 84.0);
    let mut state = emInputState::new();
    state.mouse_x = 42.0;
    state.mouse_y = 84.0;

    let vp_rc = view.CurrentViewPort.clone();
    ts.with(|sc| {
        vp_rc
            .borrow_mut()
            .InputToView(&mut view, &mut tree, &event, &state, sc);
    });

    assert!((view.LastMouseX - 42.0).abs() < 0.01);
    assert!((view.LastMouseY - 84.0).abs() < 0.01);
}
