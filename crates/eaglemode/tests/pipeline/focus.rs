//! Focus and activation parity tests (BP-15 through BP-19).
//!
//! BP-15/16/18: Tab focus cycling — Tab and Shift+Tab key handlers call
//! `visit_next`/`visit_prev` in the Input dispatch pipeline.
//!
//! BP-17: Activation on Click — tests that clicking a panel sets `is_active`
//! and `in_active_path` correctly, matching C++ `SetActivePanel`.
//!
//! BP-19: Arrow key navigation — arrow key handlers call
//! `visit_left/right/up/down` for sibling navigation.

use emcore::emInput::InputKey;
use emcore::emPanelTree::{PanelId, PanelTree};

use super::support::pipeline::PipelineTestHarness;

// ── Helpers ──────────────────────────────────────────────────────────

/// Query (is_active, in_active_path) for a panel via the public build_panel_state API.
fn active_state(tree: &PanelTree, id: PanelId) -> (bool, bool) {
    let state = tree.build_panel_state(id, false, 1.0);
    (state.is_active, state.in_active_path)
}

/// Build a two-branch tree for activation tests:
///
/// ```text
///         root
///        /    \
///    branch_a  branch_b
///      |          |
///    leaf_a     leaf_b
/// ```
///
/// Each child occupies a distinct horizontal half of its parent_context so that
/// Click hit-testing can target them by view-space coordinates.
///
/// Returns (harness, root, branch_a, leaf_a, branch_b, leaf_b).
fn two_branch_tree() -> (
    PipelineTestHarness,
    PanelId,
    PanelId,
    PanelId,
    PanelId,
    PanelId,
) {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // branch_a occupies left half: layout (0, 0, 0.5, 1)
    let branch_a = h.add_panel(root, "branch_a");
    h.tree.Layout(branch_a, 0.0, 0.0, 0.5, 1.0, 1.0, None);

    // branch_b occupies right half: layout (0.5, 0, 0.5, 1)
    let branch_b = h.add_panel(root, "branch_b");
    h.tree.Layout(branch_b, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    // leaf_a fills branch_a entirely: layout (0, 0, 1, 1) within branch_a
    let leaf_a = h.add_panel(branch_a, "leaf_a");
    h.tree.Layout(leaf_a, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    // leaf_b fills branch_b entirely: layout (0, 0, 1, 1) within branch_b
    let leaf_b = h.add_panel(branch_b, "leaf_b");
    h.tree.Layout(leaf_b, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    // Update viewing so panels have computed screen rects for hit testing.
    {
        let __cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut h.scheduler,
            framework_actions: &mut h.framework_actions,
            root_context: &h.root_context,
            framework_clipboard: &__cb,
            current_engine: None,
            pending_actions: &h.pending_actions,
        };

        h.view.Update(&mut h.tree, &mut sc);
    }
    h.tick();

    (h, root, branch_a, leaf_a, branch_b, leaf_b)
}

// ═══════════════════════════════════════════════════════════════════════
// BP-15: Tab forward focus cycling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn tab_forward_cycles_through_focusable_panels() {
    // Build a tree with 5 focusable panels, press Tab 6 times,
    // assert focus sequence wraps around matching C++ order.
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let p1 = h.add_panel(root, "p1");
    let p2 = h.add_panel(root, "p2");
    let p3 = h.add_panel(root, "p3");
    let p4 = h.add_panel(root, "p4");
    let p5 = h.add_panel(root, "p5");
    h.tick();

    // Activate first panel directly (overlapping siblings mean hit-test
    // order is unreliable, so set explicitly like the Shift+Tab test).
    h.set_active_panel(p1);
    h.tick();

    let expected_order = [p2, p3, p4, p5, p1, p2];
    for expected in &expected_order {
        h.press_key(InputKey::Tab);
        h.tick();
        assert_eq!(
            h.view.GetActivePanel(),
            Some(*expected),
            "Expected {:?} to be active after Tab",
            expected
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BP-16: Tab backward (Shift+Tab) focus cycling
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn shift_tab_cycles_backward_through_focusable_panels() {
    // Same tree, press Shift+Tab 6 times, assert reverse sequence.
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let p1 = h.add_panel(root, "p1");
    let p2 = h.add_panel(root, "p2");
    let p3 = h.add_panel(root, "p3");
    let p4 = h.add_panel(root, "p4");
    let p5 = h.add_panel(root, "p5");
    h.tick();

    // Activate last panel — set it directly via the view
    h.set_active_panel(p5);
    h.tick();

    let expected_order = [p4, p3, p2, p1, p5, p4];
    for expected in &expected_order {
        h.input_state.press(InputKey::Shift);
        h.press_key(InputKey::Tab);
        h.input_state.release(InputKey::Shift);
        h.tick();
        assert_eq!(
            h.view.GetActivePanel(),
            Some(*expected),
            "Expected {:?} to be active after Shift+Tab",
            expected
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BP-17: Activation on Click
// ═══════════════════════════════════════════════════════════════════════

// BP-17a: Click non-active panel → it becomes active.
#[test]
fn click_non_active_panel_becomes_active() {
    let (mut h, _root, branch_a, _leaf_a, _branch_b, leaf_b) = two_branch_tree();

    // Activate branch_a directly so we have a known starting state.
    h.set_active_panel(branch_a);
    h.tick();
    assert_eq!(active_state(&h.tree, branch_a), (true, true));

    // Click in the right half (branch_b territory). Use y=150 to stay well
    // inside leaf_b (whose bottom edge is ≈y=300 due to geometric-mean zoom).
    h.click(600.0, 150.0);
    h.tick();

    // The deepest focusable panel under the Click should now be active.
    // That's leaf_b (fills branch_b, which fills right half).
    let (lb_active, lb_path) = active_state(&h.tree, leaf_b);
    assert!(lb_active, "leaf_b should be active after click");
    assert!(lb_path, "leaf_b should be in active path after click");
}

// BP-17b: Old active panel loses is_active after Click on different panel.
#[test]
fn old_active_loses_is_active_on_click() {
    let (mut h, _root, _branch_a, leaf_a, _branch_b, leaf_b) = two_branch_tree();

    // Activate leaf_a.
    h.set_active_panel(leaf_a);
    h.tick();
    assert!(
        active_state(&h.tree, leaf_a).0,
        "leaf_a should start active"
    );

    // Click on right half → activates leaf_b (y=150 avoids leaf_b boundary).
    h.click(600.0, 150.0);
    h.tick();

    let (la_active, _) = active_state(&h.tree, leaf_a);
    assert!(
        !la_active,
        "leaf_a should lose is_active after clicking leaf_b"
    );

    let (lb_active, _) = active_state(&h.tree, leaf_b);
    assert!(lb_active, "leaf_b should gain is_active");
}

// BP-17c: Ancestors of the new active panel GetRec in_active_path=true.
#[test]
fn new_active_ancestors_get_in_active_path() {
    let (mut h, root, branch_a, leaf_a, branch_b, leaf_b) = two_branch_tree();

    // Start with leaf_a active.
    h.set_active_panel(leaf_a);
    h.tick();

    // Click on right half → activates leaf_b.
    h.click(600.0, 150.0);
    h.tick();

    // leaf_b and its ancestors (branch_b, root) should be in_active_path.
    assert!(
        active_state(&h.tree, leaf_b).1,
        "leaf_b should be in active path"
    );
    assert!(
        active_state(&h.tree, branch_b).1,
        "branch_b (parent of active) should be in active path"
    );
    assert!(
        active_state(&h.tree, root).1,
        "root (grandparent of active) should be in active path"
    );

    // branch_a is NOT an ancestor of leaf_b → should not be in active path.
    assert!(
        !active_state(&h.tree, branch_a).1,
        "branch_a (not ancestor of new active) should NOT be in active path"
    );
}

// BP-17d: Non-shared ancestors of old active lose in_active_path.
#[test]
fn old_active_non_shared_ancestors_lose_in_active_path() {
    let (mut h, root, branch_a, leaf_a, branch_b, _leaf_b) = two_branch_tree();

    // Start with leaf_a active.
    h.set_active_panel(leaf_a);
    h.tick();

    // Verify initial state: branch_a and root should be in active path.
    assert!(
        active_state(&h.tree, branch_a).1,
        "branch_a should start in active path"
    );
    assert!(
        active_state(&h.tree, root).1,
        "root should start in active path"
    );
    // branch_b should NOT be in active path initially.
    assert!(
        !active_state(&h.tree, branch_b).1,
        "branch_b should not be in active path initially"
    );

    // Click on right half → activates leaf_b.
    h.click(600.0, 150.0);
    h.tick();

    // branch_a was a unique ancestor of old active (leaf_a) → loses in_active_path.
    assert!(
        !active_state(&h.tree, branch_a).1,
        "branch_a (unique ancestor of old active) should lose in_active_path"
    );
    // leaf_a itself also loses in_active_path.
    assert!(
        !active_state(&h.tree, leaf_a).1,
        "leaf_a (old active) should lose in_active_path"
    );
    // root is shared → should still be in active path.
    assert!(
        active_state(&h.tree, root).1,
        "root (shared ancestor) should remain in active path"
    );
    // New branch gets in_active_path.
    assert!(
        active_state(&h.tree, branch_b).1,
        "branch_b (new active ancestor) should gain in_active_path"
    );
}

// BP-17e: Click already-active panel → no change.
#[test]
fn click_already_active_panel_no_change() {
    let (mut h, root, branch_a, _leaf_a, branch_b, leaf_b) = two_branch_tree();

    // Activate leaf_b via Click on right half.
    h.click(600.0, 150.0);
    h.tick();

    // Record state before re-Click.
    let lb_before = active_state(&h.tree, leaf_b);
    let bb_before = active_state(&h.tree, branch_b);
    let root_before = active_state(&h.tree, root);
    let ba_before = active_state(&h.tree, branch_a);
    let active_before = h.view.GetActivePanel();

    // Click again in the same spot.
    h.click(600.0, 150.0);
    h.tick();

    // Everything should remain identical.
    assert_eq!(
        active_state(&h.tree, leaf_b),
        lb_before,
        "leaf_b state unchanged after re-click"
    );
    assert_eq!(
        active_state(&h.tree, branch_b),
        bb_before,
        "branch_b state unchanged after re-click"
    );
    assert_eq!(
        active_state(&h.tree, root),
        root_before,
        "root state unchanged after re-click"
    );
    assert_eq!(
        active_state(&h.tree, branch_a),
        ba_before,
        "branch_a state unchanged after re-click"
    );
    assert_eq!(
        h.view.GetActivePanel(),
        active_before,
        "active panel unchanged after re-click"
    );
}

// BP-17f: Activation via set_active_panel directly Match Click behavior.
#[test]
fn programmatic_activation_matches_click_behavior() {
    let (mut h, root, branch_a, leaf_a, branch_b, leaf_b) = two_branch_tree();

    // Programmatic activation of leaf_a.
    h.set_active_panel(leaf_a);
    h.tick();

    assert_eq!(active_state(&h.tree, leaf_a), (true, true));
    assert_eq!(active_state(&h.tree, branch_a), (false, true));
    assert_eq!(active_state(&h.tree, root), (false, true));
    assert_eq!(active_state(&h.tree, leaf_b), (false, false));
    assert_eq!(active_state(&h.tree, branch_b), (false, false));

    // Now programmatic switch to leaf_b.
    h.set_active_panel(leaf_b);
    h.tick();

    assert_eq!(active_state(&h.tree, leaf_b), (true, true));
    assert_eq!(active_state(&h.tree, branch_b), (false, true));
    assert_eq!(active_state(&h.tree, root), (false, true));
    assert_eq!(active_state(&h.tree, leaf_a), (false, false));
    assert_eq!(active_state(&h.tree, branch_a), (false, false));
}

// BP-17g: Deeper tree — activation propagates in_active_path through
// multiple levels.
#[test]
fn deep_tree_activation_propagates_in_active_path() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Build: root → mid → deep → leaf
    let mid = h.add_panel(root, "mid");
    h.tree.Layout(mid, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let deep = h.add_panel(mid, "deep");
    h.tree.Layout(deep, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let leaf = h.add_panel(deep, "leaf");
    h.tree.Layout(leaf, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    {
        let __cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let mut sc = emcore::emEngineCtx::SchedCtx {
            scheduler: &mut h.scheduler,
            framework_actions: &mut h.framework_actions,
            root_context: &h.root_context,
            framework_clipboard: &__cb,
            current_engine: None,
            pending_actions: &h.pending_actions,
        };

        h.view.Update(&mut h.tree, &mut sc);
    }
    h.tick();

    // Activate the leaf.
    h.set_active_panel(leaf);
    h.tick();

    // Every panel in the chain root→mid→deep→leaf should be in_active_path.
    assert_eq!(active_state(&h.tree, leaf), (true, true), "leaf is active");
    assert_eq!(
        active_state(&h.tree, deep),
        (false, true),
        "deep in active path"
    );
    assert_eq!(
        active_state(&h.tree, mid),
        (false, true),
        "mid in active path"
    );
    assert_eq!(
        active_state(&h.tree, root),
        (false, true),
        "root in active path"
    );

    // Now activate mid — deep and leaf should lose in_active_path.
    h.set_active_panel(mid);
    h.tick();

    assert_eq!(
        active_state(&h.tree, mid),
        (true, true),
        "mid is now active"
    );
    assert_eq!(
        active_state(&h.tree, root),
        (false, true),
        "root still in active path"
    );
    assert_eq!(
        active_state(&h.tree, deep),
        (false, false),
        "deep no longer in active path"
    );
    assert_eq!(
        active_state(&h.tree, leaf),
        (false, false),
        "leaf no longer in active path"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// BP-18: Tab skips disabled/unfocusable panels
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn tab_skips_disabled_and_unfocusable_panels() {
    // Build tree with mix of focusable and unfocusable panels,
    // tab through, assert unfocusable panels are skipped.
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();
    let p1 = h.add_panel(root, "p1");
    let p2_unfocusable = h.add_panel(root, "p2_unfocusable");
    let p3 = h.add_panel(root, "p3");
    let p4_disabled = h.add_panel(root, "p4_disabled");
    let p5 = h.add_panel(root, "p5");

    // Mark p2 as unfocusable and p4 as disabled
    h.tree.set_focusable(p2_unfocusable, false);
    h.tree.SetEnableSwitch(p4_disabled, false, None);
    h.tick();

    // Activate p1 via the view
    h.set_active_panel(p1);
    h.tick();

    // Tab should skip p2_unfocusable and p4_disabled
    let expected_order = [p3, p5, p1]; // wraps around, skipping unfocusable/disabled
    for expected in &expected_order {
        h.press_key(InputKey::Tab);
        h.tick();
        assert_eq!(
            h.view.GetActivePanel(),
            Some(*expected),
            "Expected {:?} to be active after Tab (skipping unfocusable/disabled)",
            expected
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BP-19: Arrow key navigation between sibling panels
// ═══════════════════════════════════════════════════════════════════════

// ── Horizontal layout: Left/Right ────────────────────────────────────

#[test]
fn arrow_right_moves_focus_to_right_sibling() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Three children laid out left-to-right: [p1 | p2 | p3]
    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.333, 1.0, 1.0, None);
    let p2 = h.add_panel(root, "p2");
    h.tree.Layout(p2, 0.333, 0.0, 0.334, 1.0, 1.0, None);
    let p3 = h.add_panel(root, "p3");
    h.tree.Layout(p3, 0.667, 0.0, 0.333, 1.0, 1.0, None);
    h.tick();

    // Start focus on p1
    h.set_active_panel(p1);
    h.tick();
    assert_eq!(h.view.GetActivePanel(), Some(p1));

    // ArrowRight should move to p2
    h.press_key(InputKey::ArrowRight);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p2),
        "ArrowRight from p1 should focus p2"
    );

    // ArrowRight again should move to p3
    h.press_key(InputKey::ArrowRight);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p3),
        "ArrowRight from p2 should focus p3"
    );
}

#[test]
fn arrow_left_moves_focus_to_left_sibling() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Three children laid out left-to-right: [p1 | p2 | p3]
    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.333, 1.0, 1.0, None);
    let p2 = h.add_panel(root, "p2");
    h.tree.Layout(p2, 0.333, 0.0, 0.334, 1.0, 1.0, None);
    let p3 = h.add_panel(root, "p3");
    h.tree.Layout(p3, 0.667, 0.0, 0.333, 1.0, 1.0, None);
    h.tick();

    // Start focus on p3
    h.set_active_panel(p3);
    h.tick();
    assert_eq!(h.view.GetActivePanel(), Some(p3));

    // ArrowLeft should move to p2
    h.press_key(InputKey::ArrowLeft);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p2),
        "ArrowLeft from p3 should focus p2"
    );

    // ArrowLeft again should move to p1
    h.press_key(InputKey::ArrowLeft);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowLeft from p2 should focus p1"
    );
}

// ── Vertical layout: Up/Down ─────────────────────────────────────────

#[test]
fn arrow_down_moves_focus_to_lower_sibling() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Three children laid out top-to-bottom (tall, narrow panels):
    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 1.0, 0.333, 1.0, None);
    let p2 = h.add_panel(root, "p2");
    h.tree.Layout(p2, 0.0, 0.333, 1.0, 0.334, 1.0, None);
    let p3 = h.add_panel(root, "p3");
    h.tree.Layout(p3, 0.0, 0.667, 1.0, 0.333, 1.0, None);
    h.tick();

    h.set_active_panel(p1);
    h.tick();
    assert_eq!(h.view.GetActivePanel(), Some(p1));

    h.press_key(InputKey::ArrowDown);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p2),
        "ArrowDown from p1 should focus p2"
    );

    h.press_key(InputKey::ArrowDown);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p3),
        "ArrowDown from p2 should focus p3"
    );
}

#[test]
fn arrow_up_moves_focus_to_upper_sibling() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 1.0, 0.333, 1.0, None);
    let p2 = h.add_panel(root, "p2");
    h.tree.Layout(p2, 0.0, 0.333, 1.0, 0.334, 1.0, None);
    let p3 = h.add_panel(root, "p3");
    h.tree.Layout(p3, 0.0, 0.667, 1.0, 0.333, 1.0, None);
    h.tick();

    h.set_active_panel(p3);
    h.tick();
    assert_eq!(h.view.GetActivePanel(), Some(p3));

    h.press_key(InputKey::ArrowUp);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p2),
        "ArrowUp from p3 should focus p2"
    );

    h.press_key(InputKey::ArrowUp);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowUp from p2 should focus p1"
    );
}

// ── Cross-axis: arrow keys should not navigate perpendicular ─────────

#[test]
fn arrow_up_down_no_effect_on_horizontal_layout() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let _p2 = h.add_panel(root, "p2");
    h.tree.Layout(_p2, 0.5, 0.0, 0.5, 1.0, 1.0, None);
    h.tick();

    h.set_active_panel(p1);
    h.tick();

    h.press_key(InputKey::ArrowUp);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowUp should not move focus in a horizontal layout"
    );

    h.press_key(InputKey::ArrowDown);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowDown should not move focus in a horizontal layout"
    );
}

#[test]
fn arrow_left_right_no_effect_on_vertical_layout() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 1.0, 0.5, 1.0, None);
    let _p2 = h.add_panel(root, "p2");
    h.tree.Layout(_p2, 0.0, 0.5, 1.0, 0.5, 1.0, None);
    h.tick();

    h.set_active_panel(p1);
    h.tick();

    h.press_key(InputKey::ArrowLeft);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowLeft should not move focus in a vertical layout"
    );

    h.press_key(InputKey::ArrowRight);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowRight should not move focus in a vertical layout"
    );
}

// ── Edge case: no movement at boundary ───────────────────────────────

#[test]
fn arrow_at_boundary_stays_on_current_panel() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.333, 1.0, 1.0, None);
    let _p2 = h.add_panel(root, "p2");
    h.tree.Layout(_p2, 0.333, 0.0, 0.334, 1.0, 1.0, None);
    let p3 = h.add_panel(root, "p3");
    h.tree.Layout(p3, 0.667, 0.0, 0.333, 1.0, 1.0, None);
    h.tick();

    // At leftmost panel, ArrowLeft should not change focus
    h.set_active_panel(p1);
    h.tick();
    h.press_key(InputKey::ArrowLeft);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "ArrowLeft at leftmost panel should stay on p1"
    );

    // At rightmost panel, ArrowRight should not change focus
    h.set_active_panel(p3);
    h.tick();
    h.press_key(InputKey::ArrowRight);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p3),
        "ArrowRight at rightmost panel should stay on p3"
    );
}

// ── Modifier keys should NOT trigger navigation ──────────────────────

#[test]
fn arrow_with_modifier_does_not_navigate() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let _p2 = h.add_panel(root, "p2");
    h.tree.Layout(_p2, 0.5, 0.0, 0.5, 1.0, 1.0, None);
    h.tick();

    h.set_active_panel(p1);
    h.tick();

    // Ctrl+ArrowRight should NOT navigate
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Ctrl);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "Ctrl+ArrowRight should not trigger sibling navigation"
    );

    // Shift+ArrowRight should NOT navigate
    h.input_state.press(InputKey::Shift);
    h.press_key(InputKey::ArrowRight);
    h.input_state.release(InputKey::Shift);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "Shift+ArrowRight should not trigger sibling navigation"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// BP-20: Home/End/PageUp/PageDown navigation via dispatch fallback
// C++ emPanel.cpp:1168-1198
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn home_end_pageup_pagedown_route_through_animator() {
    // W4 Phase 4 Task 4.2: Home/End/PageUp/PageDown fire sibling/ancestor
    // navigation via the emWindow dispatch fallback.
    // Matches C++ emPanel.cpp:1168-1198.

    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    // Three children side-by-side so VisitFirst/VisitLast have somewhere to go.
    // Layout: [p1 | p2 | p3] left-to-right, all equal-width.
    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.333, 1.0, 1.0, None);
    let p2 = h.add_panel(root, "p2");
    h.tree.Layout(p2, 0.333, 0.0, 0.334, 1.0, 1.0, None);
    let p3 = h.add_panel(root, "p3");
    h.tree.Layout(p3, 0.667, 0.0, 0.333, 1.0, 1.0, None);
    h.tick();

    // ── Home (no mod) → VisitFirst → first focusable sibling ─────────
    h.set_active_panel(p3);
    h.tick();
    assert_eq!(h.view.GetActivePanel(), Some(p3));

    h.press_key(InputKey::Home);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "Home should move focus to first sibling (p1)"
    );

    // ── End (no mod) → VisitLast → last focusable sibling ────────────
    h.set_active_panel(p1);
    h.tick();

    h.press_key(InputKey::End);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p3),
        "End should move focus to last sibling (p3)"
    );

    // ── PageUp (no mod) → VisitOut → parent ──────────────────────────
    // Start on p2 (a child of root); PageUp should visit root (parent).
    h.set_active_panel(p2);
    h.tick();

    h.press_key(InputKey::PageUp);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(root),
        "PageUp should move focus to parent (root)"
    );

    // ── PageDown (no mod) → VisitIn → first child ────────────────────
    // Start on root; PageDown should descend to first focusable child (p1).
    h.set_active_panel(root);
    h.tick();

    h.press_key(InputKey::PageDown);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(p1),
        "PageDown should move focus to first child (p1)"
    );
}

#[test]
fn home_with_modifier_does_not_navigate_siblings() {
    // Home+Ctrl (not a recognized modifier combo for Home) should be a no-op
    // for sibling navigation, matching the no-mod guard on VisitFirst.
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let p1 = h.add_panel(root, "p1");
    h.tree.Layout(p1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let _p2 = h.add_panel(root, "p2");
    h.tree.Layout(_p2, 0.5, 0.0, 0.5, 1.0, 1.0, None);
    h.tick();

    h.set_active_panel(_p2);
    h.tick();

    // Ctrl+Home — not a recognized combo → no navigation
    h.input_state.press(InputKey::Ctrl);
    h.press_key(InputKey::Home);
    h.input_state.release(InputKey::Ctrl);
    h.tick();
    assert_eq!(
        h.view.GetActivePanel(),
        Some(_p2),
        "Ctrl+Home should not trigger VisitFirst"
    );
}
