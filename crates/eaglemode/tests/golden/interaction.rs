use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emView::emView;

use super::common::*;

fn sap(view: &mut emView, tree: &mut PanelTree, panel: PanelId, adherent: bool) {
    let mut tvh = emcore::test_view_harness::TestViewHarness::new();
    view.set_active_panel(tree, panel, adherent, &mut tvh.sched_ctx());
}

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
fn three_panel_tree() -> (PanelTree, emView, PanelId, PanelId, PanelId) {
    let mut ts = TestSched::new();
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    (tree, view, root, child1, child2)
}

// ─── Test 1: activate_click ────────────────────────────────────
#[test]
fn interaction_activate_click() {
    require_golden!();
    let expected = load_behavioral_golden("activate_click");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    sap(&mut view, &mut tree, child1, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "activate_click",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 2: activate_path ─────────────────────────────────────
#[test]
fn interaction_activate_path() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("activate_path");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);
    let gc = tree.create_child(child1, "gc", None);
    tree.Layout(gc, 0.0, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));
    sap(&mut view, &mut tree, gc, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "activate_path",
        &actual,
        &expected,
        &["root", "child1", "gc", "child2"],
    )
    .unwrap();
}

// ─── Test 3: activate_switch ───────────────────────────────────
#[test]
fn interaction_activate_switch() {
    require_golden!();
    let expected = load_behavioral_golden("activate_switch");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    sap(&mut view, &mut tree, child1, false);
    sap(&mut view, &mut tree, child2, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "activate_switch",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 4: focus_click ───────────────────────────────────────
#[test]
fn interaction_focus_click() {
    require_golden!();
    let expected = load_behavioral_golden("focus_click");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_click",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 5: activate_remove ──────────────────────────────────
#[test]
fn interaction_activate_remove() {
    require_golden!();
    let mut ts = TestSched::new();
    let expected = load_behavioral_golden("activate_remove");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    sap(&mut view, &mut tree, child1, false);
    ts.with(|sc| view.remove_panel(&mut tree, child1, sc));

    let actual = vec![panel_state(&tree, root), panel_state(&tree, child2)];
    compare_behavioral("activate_remove", &actual, &expected, &["root", "child2"]).unwrap();
}

// ─── Test 6: activate_nonfocusable ─────────────────────────────
#[test]
fn interaction_activate_nonfocusable() {
    require_golden!();
    let expected = load_behavioral_golden("activate_nonfocusable");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    tree.set_focusable(child1, false);
    sap(&mut view, &mut tree, child1, false);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "activate_nonfocusable",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 7: focus_tab_forward ────────────────────────────────
#[test]
fn interaction_focus_tab_forward() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_forward");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitNext(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_tab_forward",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 8: focus_tab_backward ───────────────────────────────
#[test]
fn interaction_focus_tab_backward() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_backward");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child2, true);
    view.VisitPrev(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_tab_backward",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 9: focus_unfocusable_skip ───────────────────────────
#[test]
fn interaction_focus_unfocusable_skip() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_unfocusable_skip");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.33, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.33, 0.0, 0.33, 1.0, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.66, 0.0, 0.34, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    tree.set_focusable(child2, false);
    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitNext(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_unfocusable_skip",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Test 10: focus_nested ────────────────────────────────────
#[test]
fn interaction_focus_nested() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_nested");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc = tree.create_child(child1, "gc", None);
    tree.Layout(gc, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitIn(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_nested",
        &actual,
        &expected,
        &["root", "child1", "gc", "child2"],
    )
    .unwrap();
}

// ─── Test 10b: focus_visit_out ──────────────────────────────────
#[test]
fn interaction_focus_visit_out() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_out");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc = tree.create_child(child1, "gc", None);
    tree.Layout(gc, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, gc, true);
    view.VisitOut(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_visit_out",
        &actual,
        &expected,
        &["root", "child1", "gc", "child2"],
    )
    .unwrap();
}

// ─── Test 10c: focus_tab_wrap ───────────────────────────────────
#[test]
fn interaction_focus_tab_wrap() {
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_wrap");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child2, true);
    view.VisitNext(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_tab_wrap",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Phase 1: focus_visit_first ──────────────────────────────
#[test]
fn interaction_focus_visit_first() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_first");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.33, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.33, 0.0, 0.33, 1.0, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.66, 0.0, 0.34, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child2, true);
    view.VisitFirst(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_visit_first",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Phase 1: focus_visit_last ───────────────────────────────
#[test]
fn interaction_focus_visit_last() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_last");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.33, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.33, 0.0, 0.33, 1.0, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.66, 0.0, 0.34, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitLast(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_visit_last",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Phase 1: focus_visit_left ───────────────────────────────
#[test]
fn interaction_focus_visit_left() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_left");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.33, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.33, 0.0, 0.33, 1.0, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.66, 0.0, 0.34, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child3, true);
    view.VisitLeft(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_visit_left",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Phase 1: focus_visit_right ──────────────────────────────
#[test]
fn interaction_focus_visit_right() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_right");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.33, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.33, 0.0, 0.33, 1.0, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.66, 0.0, 0.34, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitRight(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_visit_right",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Phase 1: focus_visit_down ───────────────────────────────
#[test]
fn interaction_focus_visit_down() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_down");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 1.0, 0.33, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.0, 0.33, 1.0, 0.33, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.0, 0.66, 1.0, 0.34, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitDown(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_visit_down",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Phase 1: focus_visit_up ─────────────────────────────────
#[test]
fn interaction_focus_visit_up() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_up");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 1.0, 0.33, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.0, 0.33, 1.0, 0.33, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.0, 0.66, 1.0, 0.34, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child3, true);
    view.VisitUp(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "focus_visit_up",
        &actual,
        &expected,
        &["root", "child1", "child2", "child3"],
    )
    .unwrap();
}

// ─── Phase 2: focus_disabled_panel ───────────────────────────
#[test]
fn interaction_focus_disabled_panel() {
    require_golden!();
    let expected = load_behavioral_golden("focus_disabled_panel");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    tree.SetEnableSwitch(child1, false, None);
    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_disabled_panel",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Phase 3: activate_remove_middle ─────────────────────────
#[test]
fn interaction_activate_remove_middle() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("activate_remove_middle");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.33, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.33, 0.0, 0.33, 1.0, 1.0, None);
    let child3 = tree.create_child(root, "child3", None);
    tree.Layout(child3, 0.66, 0.0, 0.34, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    ts.with(|sc| view.remove_panel(&mut tree, child2, sc));

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child3),
    ];
    compare_behavioral(
        "activate_remove_middle",
        &actual,
        &expected,
        &["root", "child1", "child3"],
    )
    .unwrap();
}

// ─── Phase 3: activate_remove_in_path ────────────────────────
#[test]
fn interaction_activate_remove_in_path() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("activate_remove_in_path");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc = tree.create_child(child1, "gc", None);
    tree.Layout(gc, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, gc, true);
    ts.with(|sc| view.remove_panel(&mut tree, child1, sc));

    let actual = vec![panel_state(&tree, root), panel_state(&tree, child2)];
    compare_behavioral(
        "activate_remove_in_path",
        &actual,
        &expected,
        &["root", "child2"],
    )
    .unwrap();
}

// ─── Phase 4: focus_tab_deep ─────────────────────────────────
#[test]
fn interaction_focus_tab_deep() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_deep");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc1 = tree.create_child(child1, "gc1", None);
    tree.Layout(gc1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc2 = tree.create_child(child1, "gc2", None);
    tree.Layout(gc2, 0.5, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, gc1, true);
    view.VisitNext(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc1),
        panel_state(&tree, gc2),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_tab_deep",
        &actual,
        &expected,
        &["root", "child1", "gc1", "gc2", "child2"],
    )
    .unwrap();
}

// ─── Phase 4: focus_tab_ascend ───────────────────────────────
#[test]
fn interaction_focus_tab_ascend() {
    let mut ts = TestSched::new();
    require_golden!();
    let expected = load_behavioral_golden("focus_tab_ascend");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let gc1 = tree.create_child(child1, "gc1", None);
    tree.Layout(gc1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc2 = tree.create_child(child1, "gc2", None);
    tree.Layout(gc2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 100.0, 100.0);
    ts.with(|sc| view.Update(&mut tree, sc));

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, gc2, true);
    view.VisitNext(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, gc1),
        panel_state(&tree, gc2),
    ];
    compare_behavioral(
        "focus_tab_ascend",
        &actual,
        &expected,
        &["root", "child1", "gc1", "gc2"],
    )
    .unwrap();
}

// ─── Phase 4: focus_visit_out_to_root ────────────────────────
#[test]
fn interaction_focus_visit_out_to_root() {
    require_golden!();
    let expected = load_behavioral_golden("focus_visit_out_to_root");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    view.VisitOut(&mut tree);
    view.pump_visiting_va(&mut tree);

    let actual = vec![
        panel_state(&tree, root),
        panel_state(&tree, child1),
        panel_state(&tree, child2),
    ];
    compare_behavioral(
        "focus_visit_out_to_root",
        &actual,
        &expected,
        &["root", "child1", "child2"],
    )
    .unwrap();
}

// ─── Test 11: focus_remove_focused ────────────────────────────
#[test]
fn interaction_focus_remove_focused() {
    require_golden!();
    let mut ts = TestSched::new();
    let expected = load_behavioral_golden("focus_remove_focused");
    let (mut tree, mut view, root, child1, child2) = three_panel_tree();

    view.SetFocused(&mut tree, true);
    sap(&mut view, &mut tree, child1, true);
    ts.with(|sc| view.remove_panel(&mut tree, child1, sc));

    let actual = vec![panel_state(&tree, root), panel_state(&tree, child2)];
    compare_behavioral(
        "focus_remove_focused",
        &actual,
        &expected,
        &["root", "child2"],
    )
    .unwrap();
}
