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
    let state = tree.build_panel_state(id, false, 1.0);
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

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 5: activate_remove ──────────────────────────────────
#[test]
fn interaction_activate_remove() {
    require_golden!();
    let expected = load_behavioral_golden("activate_remove");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_active_panel(&mut tree, child1, false);
    view.remove_panel(&mut tree, child1);

    let actual = vec![panel_state(&tree, root), panel_state(&tree, child2)];
    compare_behavioral(&actual, &expected, &["root", "child2"]).unwrap();
}

// ─── Test 6: activate_nonfocusable ─────────────────────────────
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

// ─── Test 7: focus_tab_forward ────────────────────────────────
#[test]
fn interaction_focus_tab_forward() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_forward");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_next(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 8: focus_tab_backward ───────────────────────────────
#[test]
fn interaction_focus_tab_backward() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_backward");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child2, true);
    view.visit_prev(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 9: focus_unfocusable_skip ───────────────────────────
#[test]
fn interaction_focus_unfocusable_skip() {
    require_golden!();
    let expected = load_behavioral_golden("focus_unfocusable_skip");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.33, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.33, 0.0, 0.33, 1.0);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.66, 0.0, 0.34, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    tree.set_focusable(child2, false);
    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_next(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Test 10: focus_nested ────────────────────────────────────
#[test]
fn interaction_focus_nested() {
    require_golden!();
    let expected = load_behavioral_golden("focus_nested");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let gc = tree.create_child(child1, "gc");
    tree.set_layout_rect(gc, 0.0, 0.0, 1.0, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_in(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "gc", "child2"]).unwrap();
}

// ─── Test 10b: focus_visit_out ──────────────────────────────────
#[test]
fn interaction_focus_visit_out() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_out");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let gc = tree.create_child(child1, "gc");
    tree.set_layout_rect(gc, 0.0, 0.0, 1.0, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, gc, true);
    view.visit_out(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "gc", "child2"]).unwrap();
}

// ─── Test 10c: focus_tab_wrap ───────────────────────────────────
#[test]
fn interaction_focus_tab_wrap() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_wrap");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child2, true);
    view.visit_next(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Phase 1: focus_visit_first ──────────────────────────────
#[test]
fn interaction_focus_visit_first() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_first");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.33, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.33, 0.0, 0.33, 1.0);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.66, 0.0, 0.34, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child2, true);
    view.visit_first(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Phase 1: focus_visit_last ───────────────────────────────
#[test]
fn interaction_focus_visit_last() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_last");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.33, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.33, 0.0, 0.33, 1.0);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.66, 0.0, 0.34, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_last(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Phase 1: focus_visit_left ───────────────────────────────
#[test]
fn interaction_focus_visit_left() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_left");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.33, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.33, 0.0, 0.33, 1.0);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.66, 0.0, 0.34, 1.0);

    let mut view = View::new(root, 800.0, 600.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child3, true);
    view.visit_left(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Phase 1: focus_visit_right ──────────────────────────────
#[test]
fn interaction_focus_visit_right() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_right");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.33, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.33, 0.0, 0.33, 1.0);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.66, 0.0, 0.34, 1.0);

    let mut view = View::new(root, 800.0, 600.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_right(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Phase 1: focus_visit_down ───────────────────────────────
#[test]
fn interaction_focus_visit_down() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_down");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 1.0, 0.33);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.0, 0.33, 1.0, 0.33);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.0, 0.66, 1.0, 0.34);

    let mut view = View::new(root, 800.0, 600.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_down(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Phase 1: focus_visit_up ─────────────────────────────────
#[test]
fn interaction_focus_visit_up() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_up");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 1.0, 0.33);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.0, 0.33, 1.0, 0.33);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.0, 0.66, 1.0, 0.34);

    let mut view = View::new(root, 800.0, 600.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child3, true);
    view.visit_up(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2", "child3"]).unwrap();
}

// ─── Phase 2: focus_disabled_panel ───────────────────────────
#[test]
fn interaction_focus_disabled_panel() {
    require_golden!();
    let expected = load_behavioral_golden("focus_disabled_panel");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    tree.set_enable_switch(child1, false);
    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Phase 3: activate_remove_middle ─────────────────────────
#[test]
fn interaction_activate_remove_middle() {
    require_golden!();
    let expected = load_behavioral_golden("activate_remove_middle");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.33, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.33, 0.0, 0.33, 1.0);
    let child3 = tree.create_child(root, "child3");
    tree.set_layout_rect(child3, 0.66, 0.0, 0.34, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.remove_panel(&mut tree, child2);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child3),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child3"]).unwrap();
}

// ─── Phase 3: activate_remove_in_path ────────────────────────
#[test]
fn interaction_activate_remove_in_path() {
    require_golden!();
    let expected = load_behavioral_golden("activate_remove_in_path");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let gc = tree.create_child(child1, "gc");
    tree.set_layout_rect(gc, 0.0, 0.0, 1.0, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, gc, true);
    view.remove_panel(&mut tree, child1);

    let actual = vec![panel_state(&tree, root), panel_state(&tree, child2)];
    compare_behavioral(&actual, &expected, &["root", "child2"]).unwrap();
}

// ─── Phase 4: focus_tab_deep ─────────────────────────────────
#[test]
fn interaction_focus_tab_deep() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_deep");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let gc1 = tree.create_child(child1, "gc1");
    tree.set_layout_rect(gc1, 0.0, 0.0, 0.5, 1.0);
    let gc2 = tree.create_child(child1, "gc2");
    tree.set_layout_rect(gc2, 0.5, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, gc1, true);
    view.visit_next(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc1),
        panel_state(&tree, gc2),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        &actual,
        &expected,
        &["root", "child1", "gc1", "gc2", "child2"],
    )
    .unwrap();
}

// ─── Phase 4: focus_tab_ascend ───────────────────────────────
#[test]
fn interaction_focus_tab_ascend() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_ascend");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 1.0, 1.0);
    let gc1 = tree.create_child(child1, "gc1");
    tree.set_layout_rect(gc1, 0.0, 0.0, 0.5, 1.0);
    let gc2 = tree.create_child(child1, "gc2");
    tree.set_layout_rect(gc2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 100.0, 100.0);
    view.update_viewing(&mut tree);

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, gc2, true);
    view.visit_next(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc1),
        panel_state(&tree, gc2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "gc1", "gc2"]).unwrap();
}

// ─── Phase 4: focus_visit_out_to_root ────────────────────────
#[test]
fn interaction_focus_visit_out_to_root() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_out_to_root");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.visit_out(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(&actual, &expected, &["root", "child1", "child2"]).unwrap();
}

// ─── Test 11: focus_remove_focused ────────────────────────────
#[test]
fn interaction_focus_remove_focused() {
    require_golden!();
    let expected = load_behavioral_golden("focus_remove_focused");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.set_window_focused(&mut tree, true);
    view.set_active_panel(&mut tree, child1, true);
    view.remove_panel(&mut tree, child1);

    let actual = vec![panel_state(&tree, root), panel_state(&tree, child2)];
    compare_behavioral(&actual, &expected, &["root", "child2"]).unwrap();
}
