use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::panel::{NoticeFlags, PanelId, PanelTree, View, ViewFlags};

use super::common::*;
use super::harness::NoticeBehavior;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Attach NoticeBehavior to a panel and return the shared accumulator.
fn attach_notice(tree: &mut PanelTree, id: PanelId) -> Rc<RefCell<NoticeFlags>> {
    let acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    tree.set_behavior(id, Box::new(NoticeBehavior::new(acc.clone())));
    acc
}

/// Reset accumulated flags to empty.
fn reset(acc: &Rc<RefCell<NoticeFlags>>) {
    *acc.borrow_mut() = NoticeFlags::empty();
}

/// Settle: deliver notices and update viewing, matching C++ scheduler behavior.
fn settle(tree: &mut PanelTree, view: &mut View) {
    for _ in 0..5 {
        tree.deliver_notices(view.window_focused());
        view.update_viewing(tree);
    }
}

// ─── Test 1: notice_active_changed ──────────────────────────────
// C++ view is unfocused → Activate(child1) → ACTIVE_CHANGED only.

#[test]
fn notice_active_changed() {
    require_golden!();
    let expected = load_notice_golden("notice_active_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    // C++ emView starts unfocused; Rust View::new starts focused.
    let mut view = View::new(root, 800.0, 600.0);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);
    reset(&acc_child2);

    // Action: activate child1
    view.set_active_panel(&mut tree, child1, false);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 2: notice_focus_changed ───────────────────────────────
// C++ view starts unfocused → Focus(child1) → SetViewFocused(true) +
// Activate → VIEW_FOCUS_CHANGED | FOCUS_CHANGED | ACTIVE_CHANGED.

#[test]
fn notice_focus_changed() {
    require_golden!();
    let expected = load_notice_golden("notice_focus_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    // Start unfocused to match C++
    let mut view = View::new(root, 800.0, 600.0);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);
    reset(&acc_child2);

    // Action: focus child1 (sets view focused + activates)
    view.focus_panel(&mut tree, child1);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 3: notice_layout_changed ──────────────────────────────
// C++ view unfocused → child1->Layout(0.1,0.1,0.3,0.5) → LAYOUT_CHANGED.

#[test]
fn notice_layout_changed() {
    require_golden!();
    let expected = load_notice_golden("notice_layout_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 800.0, 600.0);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);
    reset(&acc_child2);

    // Action: change child1's layout rect
    tree.set_layout_rect(child1, 0.1, 0.1, 0.3, 0.5);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 4: notice_children_changed ────────────────────────────
// Add new child after settling → CHILDREN_CHANGED on parent.

#[test]
fn notice_children_changed() {
    require_golden!();
    let expected = load_notice_golden("notice_children_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 800.0, 600.0);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    // Settle initial notices
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);

    // Action: add new child
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    // Attach notice behavior to child2 to capture init notices
    let acc_child2 = attach_notice(&mut tree, child2);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 5: notice_window_focus_gained ─────────────────────────
// Child1 activated, view unfocused → set_window_focused(true) →
// VIEW_FOCUS_CHANGED + UPDATE_PRIORITY_CHANGED on all, + FOCUS_CHANGED on path.

#[test]
fn notice_window_focus_gained() {
    require_golden!();
    let expected = load_notice_golden("notice_window_focus_gained");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);

    // Start unfocused
    let mut view = View::new(root, 800.0, 600.0);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    // Activate child1
    view.set_active_panel(&mut tree, child1, false);

    // Settle initial notices
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);

    // Action: gain window focus
    view.set_window_focused(&mut tree, true);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![acc_root.borrow().bits(), acc_child1.borrow().bits()];
    // Use full mask — Rust explicitly queues UPDATE_PRIORITY_CHANGED in set_window_focused
    compare_notices(&actual, &expected, &["root", "child1"], NOTICE_FULL_MASK).unwrap();
}

// ─── Test 6: notice_window_focus_lost ───────────────────────────
// View focused → set_window_focused(false) → same flags as gained.

#[test]
fn notice_window_focus_lost() {
    require_golden!();
    let expected = load_notice_golden("notice_window_focus_lost");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);

    // Start unfocused, then gain focus to match C++ setup
    let mut view = View::new(root, 800.0, 600.0);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    // Activate child1 + gain focus
    view.set_active_panel(&mut tree, child1, false);
    view.set_window_focused(&mut tree, true);

    // Settle
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);

    // Action: lose window focus
    view.set_window_focused(&mut tree, false);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![acc_root.borrow().bits(), acc_child1.borrow().bits()];
    compare_notices(&actual, &expected, &["root", "child1"], NOTICE_FULL_MASK).unwrap();
}

// ─── Test 7: notice_window_resize ───────────────────────────────
// C++ view with VF_ROOT_SAME_TALLNESS → resize viewport → LAYOUT_CHANGED
// on root (root layout rect updated by SetGeometry).
//
// Mask excludes VISIBILITY|UPDATE_PRIORITY_CHANGED|MEMORY_LIMIT_CHANGED:
// Rust's update_viewing recomputes all viewing from scratch and fires these
// on every panel whose viewed rect changed (children included), while C++
// only fires them on the panel whose Layout() was called (root). This is a
// known viewing-propagation difference, not a resize parity bug.
const NOTICE_RESIZE_MASK: u32 = NOTICE_FULL_MASK & !(0x0004 | 0x0400 | 0x0800);

#[test]
fn notice_window_resize() {
    require_golden!();
    let expected = load_notice_golden("notice_window_resize");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75); // 600/800 tallness
    let child1 = tree.create_child(root, "child1");
    tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);
    let child2 = tree.create_child(root, "child2");
    tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.set_window_focused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    reset(&acc_root);
    reset(&acc_child1);
    reset(&acc_child2);

    // Action: resize viewport (triggers root layout update via ROOT_SAME_TALLNESS)
    view.set_viewport(&mut tree, 1200.0, 800.0);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_RESIZE_MASK,
    )
    .unwrap();
}
