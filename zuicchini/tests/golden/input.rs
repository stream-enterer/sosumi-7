use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::input::{InputEvent, InputKey};
use zuicchini::panel::{PanelId, PanelTree};

use super::common::*;
use super::support::{InputTrackingBehavior, TestHarness};

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Attach InputTrackingBehavior to a panel and return the shared flag.
fn attach_input(tree: &mut PanelTree, id: PanelId) -> Rc<RefCell<bool>> {
    let flag = Rc::new(RefCell::new(false));
    tree.set_behavior(id, Box::new(InputTrackingBehavior::new(flag.clone())));
    flag
}

/// Query (is_active, in_active_path) for a panel.
fn panel_state(tree: &PanelTree, id: PanelId) -> (bool, bool) {
    let state = tree.build_panel_state(id, false, 1.0);
    (state.is_active, state.in_active_path)
}

// ─── Test 1: input_mouse_hit ────────────────────────────────────

#[test]
fn input_mouse_hit() {
    require_golden!();
    let expected = load_input_golden("input_mouse_hit");

    let mut h = TestHarness::new();
    let root = h.root();

    let child1 = h.add_panel(root, "child1");
    h.tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = h.add_panel(root, "child2");
    h.tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let recv_root = attach_input(&mut h.tree, root);
    let recv_child1 = attach_input(&mut h.tree, child1);
    let recv_child2 = attach_input(&mut h.tree, child2);

    // Settle
    h.tick_n(5);
    *recv_root.borrow_mut() = false;
    *recv_child1.borrow_mut() = false;
    *recv_child2.borrow_mut() = false;

    // Click at (600, 300) → right half → child2
    h.input_state.set_mouse(600.0, 300.0);
    let event = InputEvent::press(InputKey::MouseLeft).with_mouse(600.0, 300.0);
    h.inject_input(&event);
    h.tick();

    let (a_root, p_root) = panel_state(&h.tree, root);
    let (a_c1, p_c1) = panel_state(&h.tree, child1);
    let (a_c2, p_c2) = panel_state(&h.tree, child2);
    let actual = vec![
        (*recv_root.borrow(), a_root, p_root),
        (*recv_child1.borrow(), a_c1, p_c1),
        (*recv_child2.borrow(), a_c2, p_c2),
    ];
    // C++ and Rust both broadcast Input() to all viewed panels.
    compare_input(&actual, &expected, &["root", "child1", "child2"], true).unwrap();
}

// ─── Test 2: input_key_to_focused ───────────────────────────────

#[test]
fn input_key_to_focused() {
    require_golden!();
    let expected = load_input_golden("input_key_to_focused");

    let mut h = TestHarness::new();
    let root = h.root();

    let child1 = h.add_panel(root, "child1");
    h.tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = h.add_panel(root, "child2");
    h.tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    // Focus child1
    h.view.focus_panel(&mut h.tree, child1);

    let recv_root = attach_input(&mut h.tree, root);
    let recv_child1 = attach_input(&mut h.tree, child1);
    let recv_child2 = attach_input(&mut h.tree, child2);

    // Settle
    h.tick_n(5);
    *recv_root.borrow_mut() = false;
    *recv_child1.borrow_mut() = false;
    *recv_child2.borrow_mut() = false;

    // Key press
    let event = InputEvent::press(InputKey::Key('a')).with_chars("a");
    h.inject_input(&event);
    h.tick();

    let (a_root, p_root) = panel_state(&h.tree, root);
    let (a_c1, p_c1) = panel_state(&h.tree, child1);
    let (a_c2, p_c2) = panel_state(&h.tree, child2);
    let actual = vec![
        (*recv_root.borrow(), a_root, p_root),
        (*recv_child1.borrow(), a_c1, p_c1),
        (*recv_child2.borrow(), a_c2, p_c2),
    ];
    // C++ and Rust both broadcast Input() to all viewed panels.
    compare_input(&actual, &expected, &["root", "child1", "child2"], true).unwrap();
}

// ─── Test 3: input_scroll_delta ─────────────────────────────────

#[test]
fn input_scroll_delta() {
    require_golden!();
    let expected = load_input_golden("input_scroll_delta");

    let mut h = TestHarness::new();
    let root = h.root();

    let child1 = h.add_panel(root, "child1");
    h.tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);

    // Activate child1
    h.view.set_active_panel(&mut h.tree, child1, false);

    let recv_root = attach_input(&mut h.tree, root);
    let recv_child1 = attach_input(&mut h.tree, child1);

    // Settle
    h.tick_n(5);
    *recv_root.borrow_mut() = false;
    *recv_child1.borrow_mut() = false;

    // Clear VIF chain: C++ golden generator uses DoInputToView which bypasses VIF.
    // MouseZoomScrollVIF would consume the wheel event before it reaches panels.
    h.vif_chain.clear();

    // Wheel event
    h.input_state.set_mouse(200.0, 300.0);
    let event = InputEvent::press(InputKey::WheelUp).with_mouse(200.0, 300.0);
    h.inject_input(&event);
    h.tick();

    let (a_root, p_root) = panel_state(&h.tree, root);
    let (a_c1, p_c1) = panel_state(&h.tree, child1);
    let actual = vec![
        (*recv_root.borrow(), a_root, p_root),
        (*recv_child1.borrow(), a_c1, p_c1),
    ];
    compare_input(&actual, &expected, &["root", "child1"], true).unwrap();
}

// ─── Phase 5: input_mouse_miss ──────────────────────────────────

#[test]
fn input_mouse_miss() {
    require_golden!();
    let expected = load_input_golden("input_mouse_miss");

    let mut h = TestHarness::new();
    let root = h.root();

    h.tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.5); // Only covers top half
    let child1 = h.add_panel(root, "child1");
    h.tree.set_layout_rect(child1, 0.0, 0.0, 1.0, 1.0);

    let recv_root = attach_input(&mut h.tree, root);
    let recv_child1 = attach_input(&mut h.tree, child1);

    // Settle
    h.tick_n(5);
    *recv_root.borrow_mut() = false;
    *recv_child1.borrow_mut() = false;

    // Click outside panel clip area.
    // C++ root Layout(0,0,1,0.5) → root is 800×400 centered → miss below y=500.
    // Rust update_viewing maps root differently (clip starts at ~y=55), so click
    // above the clip top to achieve the same empty-space miss scenario.
    h.input_state.set_mouse(400.0, 20.0);
    let event = InputEvent::press(InputKey::MouseLeft).with_mouse(400.0, 20.0);
    h.inject_input(&event);
    h.tick();

    let (a_root, p_root) = panel_state(&h.tree, root);
    let (a_c1, p_c1) = panel_state(&h.tree, child1);

    let actual = vec![
        (*recv_root.borrow(), a_root, p_root),
        (*recv_child1.borrow(), a_c1, p_c1),
    ];
    compare_input(&actual, &expected, &["root", "child1"], true).unwrap();
}

// ─── Phase 5: input_nested_hit ──────────────────────────────────

#[test]
fn input_nested_hit() {
    require_golden!();
    let expected = load_input_golden("input_nested_hit");

    let mut h = TestHarness::new();
    let root = h.root();

    h.tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    let child1 = h.add_panel(root, "child1");
    h.tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let gc = h.add_panel(child1, "gc");
    h.tree.set_layout_rect(gc, 0.0, 0.0, 1.0, 1.0);
    let child2 = h.add_panel(root, "child2");
    h.tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let recv_root = attach_input(&mut h.tree, root);
    let recv_child1 = attach_input(&mut h.tree, child1);
    let recv_gc = attach_input(&mut h.tree, gc);
    let recv_child2 = attach_input(&mut h.tree, child2);

    // Settle
    h.tick_n(5);
    *recv_root.borrow_mut() = false;
    *recv_child1.borrow_mut() = false;
    *recv_gc.borrow_mut() = false;
    *recv_child2.borrow_mut() = false;

    // Click at (100, 300) → inside gc
    h.input_state.set_mouse(100.0, 300.0);
    let event = InputEvent::press(InputKey::MouseLeft).with_mouse(100.0, 300.0);
    h.inject_input(&event);
    h.tick();

    let (a_root, p_root) = panel_state(&h.tree, root);
    let (a_c1, p_c1) = panel_state(&h.tree, child1);
    let (a_gc, p_gc) = panel_state(&h.tree, gc);
    let (a_c2, p_c2) = panel_state(&h.tree, child2);
    let actual = vec![
        (*recv_root.borrow(), a_root, p_root),
        (*recv_child1.borrow(), a_c1, p_c1),
        (*recv_gc.borrow(), a_gc, p_gc),
        (*recv_child2.borrow(), a_c2, p_c2),
    ];
    compare_input(
        &actual,
        &expected,
        &["root", "child1", "gc", "child2"],
        true,
    )
    .unwrap();
}

// ─── Test 4: input_drag_sequence ────────────────────────────────

#[test]
fn input_drag_sequence() {
    require_golden!();
    let expected = load_input_golden("input_drag_sequence");

    let mut h = TestHarness::new();
    let root = h.root();

    let child1 = h.add_panel(root, "child1");
    h.tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = h.add_panel(root, "child2");
    h.tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let recv_root = attach_input(&mut h.tree, root);
    let recv_child1 = attach_input(&mut h.tree, child1);
    let recv_child2 = attach_input(&mut h.tree, child2);

    // Settle
    h.tick_n(5);
    *recv_root.borrow_mut() = false;
    *recv_child1.borrow_mut() = false;
    *recv_child2.borrow_mut() = false;

    // Mouse down on child1
    h.input_state.set_mouse(200.0, 300.0);
    h.input_state.press(InputKey::MouseLeft);
    let event = InputEvent::press(InputKey::MouseLeft).with_mouse(200.0, 300.0);
    h.inject_input(&event);

    // Mouse move
    h.input_state.set_mouse(300.0, 300.0);
    let event = InputEvent::mouse_move(InputKey::MouseLeft, 300.0, 300.0);
    h.inject_input(&event);

    // Mouse up
    h.input_state.set_mouse(300.0, 300.0);
    h.input_state.release(InputKey::MouseLeft);
    let event = InputEvent::release(InputKey::MouseLeft).with_mouse(300.0, 300.0);
    h.inject_input(&event);

    h.tick();

    let (a_root, p_root) = panel_state(&h.tree, root);
    let (a_c1, p_c1) = panel_state(&h.tree, child1);
    let (a_c2, p_c2) = panel_state(&h.tree, child2);
    let actual = vec![
        (*recv_root.borrow(), a_root, p_root),
        (*recv_child1.borrow(), a_c1, p_c1),
        (*recv_child2.borrow(), a_c2, p_c2),
    ];
    // C++ and Rust both broadcast Input() to all viewed panels.
    compare_input(&actual, &expected, &["root", "child1", "child2"], true).unwrap();
}
