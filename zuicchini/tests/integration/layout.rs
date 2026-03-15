use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::input::{InputEvent, InputKey};
use zuicchini::panel::{PanelCtx, PanelId};

use crate::support::{RecordingBehavior, TestHarness};

#[test]
fn parent_resize_triggers_child_relayout() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log_parent = Rc::new(RefCell::new(Vec::new()));
    let log_child = Rc::new(RefCell::new(Vec::new()));

    // Parent has a behavior that relays layout to child
    let mut parent_behavior = RecordingBehavior::new(Rc::clone(&log_parent));
    // Parent's layout_children sets child rect when called
    let child_id_cell: Rc<RefCell<Option<PanelId>>> = Rc::new(RefCell::new(None));
    let child_id_for_layout = Rc::clone(&child_id_cell);
    let call_count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
    let call_count_clone = Rc::clone(&call_count);
    parent_behavior.on_layout = Some(Box::new(move |ctx: &mut PanelCtx| {
        if let Some(cid) = *child_id_for_layout.borrow() {
            // Use a different width each call so set_layout_rect doesn't skip
            let n = *call_count_clone.borrow();
            *call_count_clone.borrow_mut() = n + 1;
            let w = 0.8 - (n as f64) * 0.01;
            ctx.layout_child(cid, 0.0, 0.0, w, 0.8);
        }
    }));

    let parent = h.add_panel_with(root, "parent", Box::new(parent_behavior));
    let child = h.add_panel_with(
        parent,
        "child",
        Box::new(RecordingBehavior::new(Rc::clone(&log_child))),
    );
    *child_id_cell.borrow_mut() = Some(child);

    h.tick();
    log_parent.borrow_mut().clear();
    log_child.borrow_mut().clear();

    // Resize parent — parent gets LAYOUT_CHANGED → layout_children sets child rect
    // → child gets LAYOUT_CHANGED on next deliver_notices
    h.tree.set_layout_rect(parent, 0.0, 0.0, 0.8, 0.8);
    h.tick();

    {
        let parent_entries = log_parent.borrow();
        assert!(
            parent_entries.iter().any(|e| e.contains("LAYOUT_CHANGED")),
            "Parent should get LAYOUT_CHANGED, got: {parent_entries:?}"
        );
        assert!(
            parent_entries.iter().any(|e| e == "layout_children"),
            "Parent's layout_children should be called, got: {parent_entries:?}"
        );
    }

    // Child's LAYOUT_CHANGED comes on the next tick since parent's layout_children
    // sets the child rect during this deliver_notices pass, and the child notice
    // may or may not be delivered in the same pass (depends on snapshot order).
    // Do another tick to be sure.
    h.tick();

    let child_entries = log_child.borrow();
    assert!(
        child_entries.iter().any(|e| e.contains("LAYOUT_CHANGED")),
        "Child should get LAYOUT_CHANGED after parent relays layout, got: {child_entries:?}"
    );
}

#[test]
fn nested_layout_cascade() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log_parent = Rc::new(RefCell::new(Vec::new()));
    let log_child = Rc::new(RefCell::new(Vec::new()));

    // Grandparent → parent (with behavior) → child (with behavior)
    let grandparent = h.add_panel(root, "grandparent");
    let parent = h.add_panel_with(
        grandparent,
        "parent",
        Box::new(RecordingBehavior::new(Rc::clone(&log_parent))),
    );
    let _child = h.add_panel_with(
        parent,
        "child",
        Box::new(RecordingBehavior::new(Rc::clone(&log_child))),
    );

    h.tick();
    log_parent.borrow_mut().clear();
    log_child.borrow_mut().clear();

    // Resize grandparent — cascade should reach parent and child
    h.tree.set_layout_rect(grandparent, 0.0, 0.0, 0.7, 0.7);

    // Resize parent too (simulating the cascade — in a real app, parent's
    // layout_children would set child rects, which triggers child notices)
    h.tree.set_layout_rect(parent, 0.0, 0.0, 0.6, 0.6);
    h.tree.set_layout_rect(_child, 0.0, 0.0, 0.5, 0.5);

    h.tick();

    let parent_entries = log_parent.borrow();
    let child_entries = log_child.borrow();

    assert!(
        parent_entries.iter().any(|e| e.contains("LAYOUT_CHANGED")),
        "Parent should get LAYOUT_CHANGED, got: {parent_entries:?}"
    );
    assert!(
        child_entries.iter().any(|e| e.contains("LAYOUT_CHANGED")),
        "Child should get LAYOUT_CHANGED, got: {child_entries:?}"
    );
}

#[test]
fn layout_affects_hit_test() {
    let mut h = TestHarness::new();
    let root = h.root();

    // Panel at left side initially
    let panel = h.add_panel(root, "movable");
    h.tree.set_layout_rect(panel, 0.0, 0.0, 0.5, 1.0);
    h.tick();

    // Click at right side — should NOT find the panel
    let click_right = InputEvent::press(InputKey::MouseLeft).with_mouse(700.0, 300.0);
    h.inject_input(&click_right);
    let _active_before = h.view.active();

    // Move panel to right side
    h.tree.set_layout_rect(panel, 0.5, 0.0, 0.5, 1.0);
    h.tick();

    // Click at right side — should now find the panel
    let click_right2 = InputEvent::press(InputKey::MouseLeft).with_mouse(700.0, 300.0);
    h.inject_input(&click_right2);

    assert_eq!(
        h.view.active(),
        Some(panel),
        "After moving panel to right side and updating, click at x=700 should find it"
    );
}
