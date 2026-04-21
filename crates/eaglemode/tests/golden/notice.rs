use std::cell::RefCell;
use std::rc::Rc;

use emcore::emPanel::NoticeFlags;
use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emView::{emView, ViewFlags};

use super::common::*;
use super::support::NoticeBehavior;

fn sap(view: &mut emView, tree: &mut PanelTree, panel: emcore::emPanelTree::PanelId) {
    let mut tvh = emcore::test_view_harness::TestViewHarness::new();
    view.set_active_panel(tree, panel, false, &mut tvh.sched_ctx());
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

/// Attach NoticeBehavior to a panel and return the shared accumulator.
fn attach_notice(tree: &mut PanelTree, id: PanelId) -> Rc<RefCell<NoticeFlags>> {
    let acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    tree.set_behavior(id, Box::new(NoticeBehavior::new(acc.clone())));
    acc
}

/// Reset accumulated flags to empty.
fn hard_reset_file_state(acc: &Rc<RefCell<NoticeFlags>>) {
    *acc.borrow_mut() = NoticeFlags::empty();
}

/// Settle: deliver notices and update viewing, matching C++ scheduler behavior.
fn settle(tree: &mut PanelTree, view: &mut emView) {
    let mut ts = TestSched::new();
    for _ in 0..5 {
        view.HandleNotice(tree, ts.sched_mut());
        ts.with(|sc| view.Update(tree, sc));
    }
}

// ─── Test 1: notice_active_changed ──────────────────────────────
// C++ view is unfocused → Activate(child1) → ACTIVE_CHANGED only.

#[test]
fn notice_active_changed() {
    require_golden!();
    let expected = load_notice_golden("notice_active_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    // C++ emView starts unfocused; Rust emView::new starts focused.
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_child2);

    // Action: activate child1
    sap(&mut view, &mut tree, child1);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_active_changed",
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
    let mut ts = TestSched::new();
    let expected = load_notice_golden("notice_focus_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    // Start unfocused to match C++
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_child2);

    // Action: focus child1 (sets view focused + activates)
    ts.with(|sc| view.focus_panel(&mut tree, child1, sc));

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_focus_changed",
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
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_child2);

    // Action: change child1's layout rect
    tree.Layout(child1, 0.1, 0.1, 0.3, 0.5, 1.0, None);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_layout_changed",
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
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);

    // Action: add new child
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

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
        "notice_children_changed",
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
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);

    // Start unfocused
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    // Activate child1
    sap(&mut view, &mut tree, child1);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);

    // Action: gain window focus
    view.SetFocused(&mut tree, true);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![acc_root.borrow().bits(), acc_child1.borrow().bits()];
    // Use full mask — Rust explicitly queues UPDATE_PRIORITY_CHANGED in set_window_focused
    compare_notices(
        "notice_window_focus_gained",
        &actual,
        &expected,
        &["root", "child1"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 6: notice_window_focus_lost ───────────────────────────
// emView focused → set_window_focused(false) → same flags as gained.

#[test]
fn notice_window_focus_lost() {
    require_golden!();
    let expected = load_notice_golden("notice_window_focus_lost");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);

    // Start unfocused, then gain focus to match C++ setup
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    // Activate child1 + gain focus
    sap(&mut view, &mut tree, child1);
    view.SetFocused(&mut tree, true);

    // Settle
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);

    // Action: lose window focus
    view.SetFocused(&mut tree, false);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![acc_root.borrow().bits(), acc_child1.borrow().bits()];
    compare_notices(
        "notice_window_focus_lost",
        &actual,
        &expected,
        &["root", "child1"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 7: notice_window_resize ───────────────────────────────
// C++ view with VF_ROOT_SAME_TALLNESS → resize viewport → LAYOUT_CHANGED
// on root (root layout rect updated by SetGeometry).

#[test]
fn notice_window_resize() {
    require_golden!();
    let expected = load_notice_golden("notice_window_resize");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None); // 600/800 tallness
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_child2);

    // Action: resize viewport (triggers root layout update via ROOT_SAME_TALLNESS)
    let mut sched = emcore::emScheduler::EngineScheduler::new();
    let root_ctx = emcore::emContext::emContext::NewRoot();
    let mut fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let __cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
        std::cell::RefCell::new(None);
    let __pa: std::rc::Rc<std::cell::RefCell<Vec<emcore::emEngineCtx::FrameworkDeferredAction>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut sc = emcore::emEngineCtx::SchedCtx {
        scheduler: &mut sched,
        framework_actions: &mut fw,
        root_context: &root_ctx,
        framework_clipboard: &__cb,
        current_engine: None,
        pending_actions: &__pa,
    };
    view.SetGeometry(&mut tree, 0.0, 0.0, 1200.0, 800.0, 1.0, &mut sc);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_window_resize",
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Phase 2: notice_recursive_enable ────────────────────────────
// Disable child1 (which has a grandchild) → ENABLE_CHANGED on child1 + gc.

#[test]
fn notice_recursive_enable() {
    require_golden!();
    let expected = load_notice_golden("notice_recursive_enable");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc = tree.create_child(child1, "gc", None);
    tree.Layout(gc, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_gc = attach_notice(&mut tree, gc);
    let acc_child2 = attach_notice(&mut tree, child2);

    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_gc);
    hard_reset_file_state(&acc_child2);

    tree.SetEnableSwitch(child1, false, None);

    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_gc.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_recursive_enable",
        &actual,
        &expected,
        &["root", "child1", "gc", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Phase 2: notice_re_enable ───────────────────────────────────
// Disable then re-enable child1 → ENABLE_CHANGED fires again.

#[test]
fn notice_re_enable() {
    require_golden!();
    let expected = load_notice_golden("notice_re_enable");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let gc = tree.create_child(child1, "gc", None);
    tree.Layout(gc, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_gc = attach_notice(&mut tree, gc);
    let acc_child2 = attach_notice(&mut tree, child2);

    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_gc);
    hard_reset_file_state(&acc_child2);

    // Disable first
    tree.SetEnableSwitch(child1, false, None);

    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_gc);
    hard_reset_file_state(&acc_child2);

    // Re-enable
    tree.SetEnableSwitch(child1, true, None);

    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_gc.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_re_enable",
        &actual,
        &expected,
        &["root", "child1", "gc", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Phase 3: notice_remove_child ────────────────────────────────
// Remove child2 → CHILDREN_CHANGED on root.

#[test]
fn notice_remove_child() {
    require_golden!();
    let expected = load_notice_golden("notice_remove_child");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);

    // Remove child2 (using tree.remove, not view.remove_panel)
    tree.remove(child2, None);

    settle(&mut tree, &mut view);

    let actual = vec![acc_root.borrow().bits(), acc_child1.borrow().bits()];
    compare_notices(
        "notice_remove_child",
        &actual,
        &expected,
        &["root", "child1"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Phase 6: notice_focus_and_layout ────────────────────────────
// Focus + layout change in same settle → both flags appear.

#[test]
fn notice_focus_and_layout() {
    require_golden!();
    let mut ts = TestSched::new();
    let expected = load_notice_golden("notice_focus_and_layout");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_child2);

    // Two actions before settle: focus + layout change
    ts.with(|sc| view.focus_panel(&mut tree, child1, sc));
    tree.Layout(child1, 0.1, 0.1, 0.3, 0.5, 1.0, None);

    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_focus_and_layout",
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Phase 6: notice_add_and_activate ────────────────────────────
// Add new child and activate it before settling.

#[test]
fn notice_add_and_activate() {
    require_golden!();
    let expected = load_notice_golden("notice_add_and_activate");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);

    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);

    // Add new child and activate it before settling
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);
    let acc_child2 = attach_notice(&mut tree, child2);
    sap(&mut view, &mut tree, child2);

    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_add_and_activate",
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}

// ─── Test 8: notice_enable_changed ──────────────────────────────
// C++ view unfocused → child1->SetEnableSwitch(false) → NF_ENABLE_CHANGED.

#[test]
fn notice_enable_changed() {
    require_golden!();
    let expected = load_notice_golden("notice_enable_changed");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child1 = tree.create_child(root, "child1", None);
    tree.Layout(child1, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    let child2 = tree.create_child(root, "child2", None);
    tree.Layout(child2, 0.5, 0.0, 0.5, 1.0, 1.0, None);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.SetFocused(&mut tree, false);

    let acc_root = attach_notice(&mut tree, root);
    let acc_child1 = attach_notice(&mut tree, child1);
    let acc_child2 = attach_notice(&mut tree, child2);

    // Settle initial notices
    settle(&mut tree, &mut view);
    hard_reset_file_state(&acc_root);
    hard_reset_file_state(&acc_child1);
    hard_reset_file_state(&acc_child2);

    // Action: disable child1
    tree.SetEnableSwitch(child1, false, None);

    // Deliver new notices
    settle(&mut tree, &mut view);

    let actual = vec![
        acc_root.borrow().bits(),
        acc_child1.borrow().bits(),
        acc_child2.borrow().bits(),
    ];
    compare_notices(
        "notice_enable_changed",
        &actual,
        &expected,
        &["root", "child1", "child2"],
        NOTICE_FULL_MASK,
    )
    .unwrap();
}
