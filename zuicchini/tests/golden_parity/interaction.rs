use zuicchini::panel::{PanelId, PanelTree, View};

use super::common::*;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Query (is_active, in_active_path) for a panel via the public API.
fn panel_state(tree: &PanelTree, id: PanelId) -> (bool, bool) {
    let state = tree.build_panel_state(id, false);
    (state.is_active, state.in_active_path)
}

/// Create a standard 3-panel tree (root → child1, child2) with layout rects.
fn three_panel_tree() -> (PanelTree, View, PanelId, PanelId, PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    (tree, view, root, child1, child2)
}

// ─── Test 1: activate_click ────────────────────────────────────
#[test]
fn interaction_activate_click() {
    require_golden!();
    let expected = load_behavioral_golden("activate_click");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_active_panel(&mut tree, child1, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 2: activate_path ─────────────────────────────────────
#[test]
fn interaction_activate_path() {
    require_golden!();
    let expected = load_behavioral_golden("activate_path");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);
    let gc = tree.create_child(child1, "gc");
    tree.set_layout_rect(gc, 0.0, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);
    view.set_active_panel(&mut tree, gc, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "gc", "child2"]).unwrap();
}

// ─── Test 3: activate_switch ───────────────────────────────────
#[test]
fn interaction_activate_switch() {
    require_golden!();
    let expected = load_behavioral_golden("activate_switch");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_active_panel(&mut tree, child1, false);
    view.set_active_panel(&mut tree, child2, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 4: focus_click ───────────────────────────────────────
#[test]
fn interaction_focus_click() {
    require_golden!();
    let expected = load_behavioral_golden("focus_click");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_window_focused(true);
    view.set_active_panel(&mut tree, child1, true);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 5: activate_nonfocusable ─────────────────────────────
#[test]
fn interaction_activate_nonfocusable() {
    require_golden!();
    let expected = load_behavioral_golden("activate_nonfocusable");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    tree.set_focusable(child1, false);
    view.set_active_panel(&mut tree, child1, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}
