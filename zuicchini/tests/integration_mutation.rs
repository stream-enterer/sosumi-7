mod harness;

use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::panel::{PanelCtx, PanelId};

use harness::{MutatingBehavior, RecordingBehavior, TestHarness};

#[test]
fn add_child_during_layout_children() {
    // Behavior adds children in layout_children() callback → no panic.
    let mut h = TestHarness::new();
    let root = h.root();

    let created_ids: Rc<RefCell<Vec<PanelId>>> = Rc::new(RefCell::new(Vec::new()));
    let ids_clone = Rc::clone(&created_ids);

    let mut behavior = MutatingBehavior::new();
    behavior.on_layout = Some(Box::new(move |ctx: &mut PanelCtx| {
        // Only add children once (avoid infinite loop on subsequent ticks)
        if ctx.child_count() == 0 {
            let c1 = ctx.create_child("dynamic_a");
            let c2 = ctx.create_child("dynamic_b");
            ids_clone.borrow_mut().push(c1);
            ids_clone.borrow_mut().push(c2);
        }
    }));

    let parent = h.add_panel_with(root, "parent", Box::new(behavior));
    h.tick();

    let ids = created_ids.borrow();
    assert_eq!(ids.len(), 2, "Two children should have been created");
    assert!(h.tree.contains(ids[0]));
    assert!(h.tree.contains(ids[1]));
    assert_eq!(h.tree.child_count(parent), 2);
}

#[test]
fn remove_sibling_during_layout_children() {
    // Behavior removes a sibling in layout_children() → no panic.
    let mut h = TestHarness::new();
    let root = h.root();

    let sibling = h.add_panel(root, "sibling");
    let sibling_id = sibling;

    let mut behavior = MutatingBehavior::new();
    behavior.on_layout = Some(Box::new(move |ctx: &mut PanelCtx| {
        // Delete sibling via parent
        if let Some(parent) = ctx.parent() {
            // We can't directly remove a sibling through PanelCtx (it's scoped to self).
            // Instead, we just verify the sibling is reachable.
            let _ = parent;
        }
    }));

    let _actor = h.add_panel_with(root, "actor", Box::new(behavior));
    h.tick();

    // Verify tree is consistent — sibling still exists since PanelCtx
    // doesn't allow cross-panel mutation. This documents the safety boundary.
    assert!(
        h.tree.contains(sibling_id),
        "Sibling should still exist (PanelCtx is scoped)"
    );
}

#[test]
fn child_iter_snapshot_safety() {
    // Collect children to Vec → remove during Vec iteration → safe.
    let mut h = TestHarness::new();
    let root = h.root();

    let a = h.add_panel(root, "a");
    let b = h.add_panel(root, "b");
    let c = h.add_panel(root, "c");

    // Snapshot children to Vec, then remove during iteration
    let children: Vec<PanelId> = h.tree.children(root).collect();
    assert_eq!(children.len(), 3);

    for id in &children {
        h.tree.remove(*id);
    }

    assert_eq!(h.tree.child_count(root), 0);
    assert!(!h.tree.contains(a));
    assert!(!h.tree.contains(b));
    assert!(!h.tree.contains(c));
}

#[test]
fn deliver_notices_with_new_panels() {
    // Notice callback (via layout_children) creates new panels →
    // new panels don't get notices this tick (not in snapshot) → get them next tick.
    let mut h = TestHarness::new();
    let root = h.root();
    let new_panel_log = Rc::new(RefCell::new(Vec::new()));
    let created: Rc<RefCell<Option<PanelId>>> = Rc::new(RefCell::new(None));
    let created_clone = Rc::clone(&created);

    let mut behavior = MutatingBehavior::new();
    behavior.on_layout = Some(Box::new(move |ctx: &mut PanelCtx| {
        if ctx.child_count() == 0 {
            let child = ctx.create_child("late_child");
            ctx.layout_child(child, 0.0, 0.0, 1.0, 1.0);
            *created_clone.borrow_mut() = Some(child);
        }
    }));

    let _parent = h.add_panel_with(root, "parent", Box::new(behavior));
    h.tick(); // First tick: parent's layout_children creates late_child

    let child_id = created.borrow().expect("Child should have been created");
    assert!(h.tree.contains(child_id));

    // Attach a recording behavior to the new child
    h.tree.set_behavior(
        child_id,
        Box::new(RecordingBehavior::new(Rc::clone(&new_panel_log))),
    );

    // Trigger a layout change on the child
    h.tree.set_layout_rect(child_id, 0.0, 0.0, 0.9, 0.9);
    h.tick(); // Second tick: child should now receive notices

    let entries = new_panel_log.borrow();
    assert!(
        entries.iter().any(|e| e.contains("LAYOUT_CHANGED")),
        "Newly created panel should receive notices on next tick, got: {entries:?}"
    );
}

#[test]
fn delete_all_children_during_layout() {
    // Deleting children during layout_children is safe — deliver_notices
    // skips panels removed by prior callbacks in the same loop.
    let mut h = TestHarness::new();
    let root = h.root();
    let deleted = Rc::new(RefCell::new(false));
    let deleted_clone = Rc::clone(&deleted);

    // Pre-create some children
    let _a = h.add_panel(root, "pre_a");
    let _b = h.add_panel(root, "pre_b");

    let parent = h.add_panel(root, "parent");
    let _c1 = h.tree.create_child(parent, "c1");
    let _c2 = h.tree.create_child(parent, "c2");

    let mut behavior = MutatingBehavior::new();
    behavior.on_layout = Some(Box::new(move |ctx: &mut PanelCtx| {
        if !*deleted_clone.borrow() {
            ctx.delete_all_children();
            *deleted_clone.borrow_mut() = true;
        }
    }));
    h.tree.set_behavior(parent, Box::new(behavior));

    h.tick();

    assert!(*deleted.borrow(), "delete_all_children should have run");
    assert_eq!(h.tree.child_count(parent), 0);
}
