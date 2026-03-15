use crate::support::TestHarness;

#[test]
fn create_tree_tick_destroy() {
    let mut h = TestHarness::new();
    let root = h.root();

    let a = h.add_panel(root, "a");
    let b = h.add_panel(root, "b");
    assert_eq!(h.tree.len(), 3);

    // Tick delivers notices (children get LAYOUT_CHANGED from set_layout_rect)
    h.tick();

    // Remove all children
    h.tree.remove(a);
    h.tree.remove(b);
    assert_eq!(h.tree.len(), 1);

    // No stale state — another tick works fine
    h.tick();
    assert!(h.tree.contains(root));
    assert!(!h.tree.contains(a));
    assert!(!h.tree.contains(b));
}

#[test]
fn remove_active_panel_reselects() {
    let mut h = TestHarness::new();
    let root = h.root();

    let a = h.add_panel(root, "a");
    h.tree.set_layout_rect(a, 0.0, 0.0, 0.5, 1.0);
    let b = h.add_panel(root, "b");
    h.tree.set_layout_rect(b, 0.5, 0.0, 0.5, 1.0);
    h.tick();

    // Make A active
    h.view.set_active_panel(&mut h.tree, a, false);
    h.view.update_viewing(&mut h.tree);
    assert_eq!(h.view.active(), Some(a));

    // Remove A
    h.tree.remove(a);
    h.tick();

    // View should auto-select a new active panel (set_active_panel_best_possible)
    h.view.set_active_panel_best_possible(&mut h.tree);
    let active = h.view.active();
    assert!(active.is_some());
    assert_ne!(active, Some(a));
}

#[test]
fn remove_panel_with_engine() {
    use zuicchini::scheduler::{Engine, EngineCtx, Priority};

    struct DummyEngine;
    impl Engine for DummyEngine {
        fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
            false
        }
    }

    let mut h = TestHarness::new();
    let root = h.root();
    let child = h.add_panel(root, "child");

    // Register an engine associated with this panel
    let eng = h
        .scheduler
        .register_engine(Priority::Medium, Box::new(DummyEngine));
    h.scheduler.wake_up(eng);
    h.tick();

    // Remove panel and its engine
    h.tree.remove(child);
    h.scheduler.remove_engine(eng);

    // Tick should not crash
    h.tick();
    assert!(!h.tree.contains(child));
}

#[test]
fn rapid_create_remove() {
    let mut h = TestHarness::new();
    let root = h.root();

    for i in 0..100 {
        let name = format!("panel_{i}");
        let id = h.add_panel(root, &name);
        h.tree.remove(id);
    }
    h.tick();

    // Only root remains
    assert_eq!(h.tree.len(), 1);
    assert_eq!(h.tree.child_count(root), 0);
}

#[test]
fn stale_panel_id_after_remove() {
    let mut h = TestHarness::new();
    let root = h.root();

    let child = h.add_panel(root, "child");
    h.tree.remove(child);

    assert!(!h.tree.contains(child));
    assert_eq!(h.tree.name(child), None);
    assert_eq!(h.tree.layout_rect(child), None);
    assert!(!h.tree.visible(child));
    assert!(!h.tree.focusable(child));
}
