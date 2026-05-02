//! Regression test for notice-dispatch PanelCtx reach loss.
//!
//! Spec: docs/superpowers/specs/2026-05-02-notice-dispatch-reach-loss-design.md
//! Investigation: docs/debug/investigations/notice-dispatch-reach-loss.md
//!
//! Asserts that the per-callback `PanelCtx` built inside
//! `emView::handle_notice_one` carries full scheduler reach
//! (`as_sched_ctx().is_some()`) for all four behavior dispatch sites.
//!
//! Currently `#[ignore]`d because `handle_notice_one` builds PanelCtx via
//! `PanelCtx::with_scheduler` (only 1 of 5 reach handles set) instead of
//! `PanelCtx::with_sched_reach` (all 5 set). Task 2 extends the
//! `HandleNotice` / `handle_notice_one` signatures with the 3 missing
//! handles and switches all dispatch sites to `with_sched_reach`, which
//! makes this test green.

use std::cell::Cell;
use std::rc::Rc;

use emcore::emEngineCtx::PanelCtx;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelTree;
use emcore::emView::emView;

#[derive(Default)]
struct ReachLog {
    notice: Cell<bool>,
    auto_expand: Cell<bool>,
    auto_shrink: Cell<bool>,
    layout_children: Cell<bool>,
}

struct ReachProbe(Rc<ReachLog>);

impl PanelBehavior for ReachProbe {
    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, ctx: &mut PanelCtx) {
        self.0.notice.set(ctx.as_sched_ctx().is_some());
    }
    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.0.auto_expand.set(ctx.as_sched_ctx().is_some());
    }
    fn AutoShrink(&mut self, ctx: &mut PanelCtx) {
        self.0.auto_shrink.set(ctx.as_sched_ctx().is_some());
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        self.0.layout_children.set(ctx.as_sched_ctx().is_some());
    }
}

/// Drive `notice`, `AutoExpand`, and `LayoutChildren` dispatch paths.
///
/// Flow (two HandleNotice calls to match C++ phase ordering):
///
/// Call 1: queue SOUGHT_NAME_CHANGED → Phase 2 fires `notice()`, sets
///   ae_decision_invalid=true (seek target set), re-adds root to ring.
///
/// Call 2: root re-enters ring with ae_decision_invalid; Phase 3 sees
///   `should_expand && !ae_expanded` → fires `AutoExpand()`.
///   Then Phase 4 sees children_layout_invalid → fires `LayoutChildren()`.
///
/// All three dispatch sites build PanelCtx via `with_scheduler` today, so
/// `as_sched_ctx()` returns None at each. Task 2 switches to
/// `with_sched_reach`, turning this test green.
#[test]
#[ignore = "TDD-red: Task 2 extends HandleNotice with 3 missing reach handles"]
fn notice_dispatch_sites_carry_full_reach_notice_ae_layout() {
    let log = Rc::new(ReachLog::default());

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    // LayoutChildren requires at least one child.
    let _child = tree.create_child(root, "child", None);
    tree.set_behavior(root, Box::new(ReachProbe(log.clone())));

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    let mut sched = emcore::emScheduler::EngineScheduler::new();

    // Make root the seek target so Phase 3 picks AutoExpand.
    tree.set_seek_pos_pub(root, "");
    // Mark children_layout_invalid for Phase 4 — will survive the Phase 2
    // return because Phase 2 only fires notice() and re-adds to ring.
    tree.set_children_layout_invalid_pub(root, true);

    // Call 1: Phase 2 path — queue notice → fires notice(), re-adds to ring.
    tree.queue_notice(root, NoticeFlags::SOUGHT_NAME_CHANGED, None);
    view.HandleNotice(&mut tree, &mut sched, None, None);

    // Call 2: Phase 3+4 path — ae_decision_invalid set by Phase 2; fires
    // AutoExpand() then LayoutChildren().
    view.HandleNotice(&mut tree, &mut sched, None, None);

    assert!(
        log.notice.get(),
        "notice dispatch must carry full scheduler reach"
    );
    assert!(
        log.auto_expand.get(),
        "AutoExpand dispatch must carry full scheduler reach"
    );
    assert!(
        log.layout_children.get(),
        "LayoutChildren dispatch must carry full scheduler reach"
    );
}

/// Drive the Phase-1 `AutoShrink` dispatch path.
///
/// Phase-1 path: set ae_invalid=true + ae_expanded=true → Phase 1 clears
/// ae_invalid, clears ae_expanded, sets ae_decision_invalid, fires
/// `AutoShrink()`.
#[test]
#[ignore = "TDD-red: Task 2 extends HandleNotice with 3 missing reach handles"]
fn notice_dispatch_sites_carry_full_reach_autoshrink_phase1() {
    let log = Rc::new(ReachLog::default());

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_behavior(root, Box::new(ReachProbe(log.clone())));

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    let mut sched = emcore::emScheduler::EngineScheduler::new();

    // Phase-1 AutoShrink path: ae_invalid=true + ae_expanded=true.
    tree.set_ae_invalid_pub(root, true);
    tree.set_ae_expanded_pub(root, true);
    // Enroll root in the safety-net scan (HandleNotice picks up panels with
    // ae_invalid via the safety-net when has_pending_notices is set).
    tree.mark_pending_notices_pub();

    view.HandleNotice(&mut tree, &mut sched, None, None);

    assert!(
        log.auto_shrink.get(),
        "AutoShrink (Phase-1 path) dispatch must carry full scheduler reach"
    );
}

/// Drive the Phase-3 `AutoShrink` dispatch path.
///
/// Phase-3 path: set ae_decision_invalid=true + ae_expanded=true + no seek
/// target → Phase 3 sees `!should_expand && ae_expanded` → fires
/// `AutoShrink()`.
#[test]
#[ignore = "TDD-red: Task 2 extends HandleNotice with 3 missing reach handles"]
fn notice_dispatch_sites_carry_full_reach_autoshrink_phase3() {
    let log = Rc::new(ReachLog::default());

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_behavior(root, Box::new(ReachProbe(log.clone())));

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    let mut sched = emcore::emScheduler::EngineScheduler::new();

    // No seek target → should_expand = false. ae_expanded=true → Phase 3
    // fires AutoShrink.
    tree.set_ae_decision_invalid_pub(root, true);
    tree.set_ae_expanded_pub(root, true);
    tree.mark_pending_notices_pub();

    view.HandleNotice(&mut tree, &mut sched, None, None);

    assert!(
        log.auto_shrink.get(),
        "AutoShrink (Phase-3 path) dispatch must carry full scheduler reach"
    );
}
