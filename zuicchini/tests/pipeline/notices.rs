//! Notice/signal parity tests (BP-20 through BP-24).
//!
//! BP-20: Layout change propagation
//! BP-21: Focus change notices
//! BP-22: Enable change propagation
//! BP-23: Children change notice
//! BP-24: Active change notice

use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::panel::{NoticeFlags, PanelBehavior, PanelCtx, PanelId, PanelState};

use super::support::pipeline::PipelineTestHarness;
use super::support::{NoticeBehavior, TestHarness};

// ═══════════════════════════════════════════════════════════════════════
// Shared helpers and behaviors
// ═══════════════════════════════════════════════════════════════════════

/// A behavior that propagates layout changes to children by calling
/// `layout_child` on each child during `layout_children`. This mirrors what
/// real panel behaviors do (and what C++ `LayoutChildren` overrides do).
struct PropagatingBehavior {
    accumulated: Rc<RefCell<NoticeFlags>>,
}

impl PropagatingBehavior {
    fn new(accumulated: Rc<RefCell<NoticeFlags>>) -> Self {
        Self { accumulated }
    }
}

impl PanelBehavior for PropagatingBehavior {
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        self.accumulated.borrow_mut().insert(flags);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let h = ctx.layout_rect().h;
        for child in children {
            ctx.layout_child(child, 0.0, 0.0, 1.0, h);
        }
    }
}

/// A behavior that records notice calls tagged with a panel name.
/// Log entries have the format `"<name>:notice:<flags_debug>"`.
struct NamedRecordingBehavior {
    name: String,
    log: Rc<RefCell<Vec<String>>>,
}

impl NamedRecordingBehavior {
    fn new(name: &str, log: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            name: name.to_string(),
            log,
        }
    }
}

impl PanelBehavior for NamedRecordingBehavior {
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        self.log
            .borrow_mut()
            .push(format!("{}:notice:{:?}", self.name, flags));
    }
}

/// A behavior that records ACTIVE_CHANGED deliveries with a label into a
/// shared ordered log, enabling cross-panel delivery order assertions.
struct LabeledNoticeBehavior {
    label: String,
    log: Rc<RefCell<Vec<String>>>,
    accumulated: Rc<RefCell<NoticeFlags>>,
}

impl LabeledNoticeBehavior {
    fn new(
        label: &str,
        log: Rc<RefCell<Vec<String>>>,
        accumulated: Rc<RefCell<NoticeFlags>>,
    ) -> Self {
        Self {
            label: label.to_string(),
            log,
            accumulated,
        }
    }
}

impl PanelBehavior for LabeledNoticeBehavior {
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        self.accumulated.borrow_mut().insert(flags);
        if flags.contains(NoticeFlags::ACTIVE_CHANGED) {
            self.log
                .borrow_mut()
                .push(format!("{}:ACTIVE_CHANGED", self.label));
        }
    }
}

/// Run enough ticks to fully settle all creation notices (BP-20).
fn settle(h: &mut PipelineTestHarness) {
    h.tick_n(3);
}

/// Create a NoticeBehavior with a fresh accumulator (BP-22).
fn notice_pair() -> (Rc<RefCell<NoticeFlags>>, Box<NoticeBehavior>) {
    let acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let beh = Box::new(NoticeBehavior::new(Rc::clone(&acc)));
    (acc, beh)
}

/// Check whether ENABLE_CHANGED is set in the accumulator (BP-22).
fn has_enable_changed(acc: &Rc<RefCell<NoticeFlags>>) -> bool {
    acc.borrow().contains(NoticeFlags::ENABLE_CHANGED)
}

/// Clear the accumulator (BP-22).
fn clear_flags(acc: &Rc<RefCell<NoticeFlags>>) {
    *acc.borrow_mut() = NoticeFlags::empty();
}

/// Build two-branch tree for BP-24:
///
/// ```text
///         root
///        /    \
///   branch_a  branch_b
///     |           |
///   leaf_a     leaf_b
/// ```
///
/// Returns (root, root_acc, branch_a, ba_acc, leaf_a, la_acc, branch_b, bb_acc, leaf_b, lb_acc).
fn build_labeled_tree(
    h: &mut TestHarness,
    log: &Rc<RefCell<Vec<String>>>,
) -> (
    PanelId,
    Rc<RefCell<NoticeFlags>>,
    PanelId,
    Rc<RefCell<NoticeFlags>>,
    PanelId,
    Rc<RefCell<NoticeFlags>>,
    PanelId,
    Rc<RefCell<NoticeFlags>>,
    PanelId,
    Rc<RefCell<NoticeFlags>>,
) {
    let root = h.root();
    let root_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    h.tree.set_behavior(
        root,
        Box::new(LabeledNoticeBehavior::new(
            "root",
            log.clone(),
            root_acc.clone(),
        )),
    );

    let branch_a_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let branch_a = h.add_panel_with(
        root,
        "branch_a",
        Box::new(LabeledNoticeBehavior::new(
            "branch_a",
            log.clone(),
            branch_a_acc.clone(),
        )),
    );

    let branch_b_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let branch_b = h.add_panel_with(
        root,
        "branch_b",
        Box::new(LabeledNoticeBehavior::new(
            "branch_b",
            log.clone(),
            branch_b_acc.clone(),
        )),
    );

    let leaf_a_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let leaf_a = h.add_panel_with(
        branch_a,
        "leaf_a",
        Box::new(LabeledNoticeBehavior::new(
            "leaf_a",
            log.clone(),
            leaf_a_acc.clone(),
        )),
    );

    let leaf_b_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let leaf_b = h.add_panel_with(
        branch_b,
        "leaf_b",
        Box::new(LabeledNoticeBehavior::new(
            "leaf_b",
            log.clone(),
            leaf_b_acc.clone(),
        )),
    );

    (
        root,
        root_acc,
        branch_a,
        branch_a_acc,
        leaf_a,
        leaf_a_acc,
        branch_b,
        branch_b_acc,
        leaf_b,
        leaf_b_acc,
    )
}

/// Flush initial notices and clear accumulators and log (BP-24).
fn flush_and_clear(
    h: &mut TestHarness,
    log: &Rc<RefCell<Vec<String>>>,
    accumulators: &[&Rc<RefCell<NoticeFlags>>],
) {
    h.tick();
    log.borrow_mut().clear();
    for acc in accumulators {
        acc.borrow_mut().remove(NoticeFlags::all());
    }
}

// ═══════════════════════════════════════════════════════════════════════
// BP-20: Layout change propagation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn layout_change_fires_on_panel() {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let flags = Rc::new(RefCell::new(NoticeFlags::empty()));
    let panel = h.add_panel_with(
        root,
        "panel",
        Box::new(NoticeBehavior::new(flags.clone())),
    );

    settle(&mut h);
    *flags.borrow_mut() = NoticeFlags::empty();

    h.tree.set_layout_rect(panel, 0.1, 0.1, 0.8, 0.8);
    h.tick();

    assert!(
        flags.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must fire on the panel whose layout rect changed"
    );
}

#[test]
fn layout_change_propagates_to_child() {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let flags_parent = Rc::new(RefCell::new(NoticeFlags::empty()));
    let parent = h.add_panel_with(
        root,
        "parent",
        Box::new(PropagatingBehavior::new(flags_parent.clone())),
    );

    let flags_child = Rc::new(RefCell::new(NoticeFlags::empty()));
    let _child = h.add_panel_with(
        parent,
        "child",
        Box::new(NoticeBehavior::new(flags_child.clone())),
    );

    settle(&mut h);
    *flags_parent.borrow_mut() = NoticeFlags::empty();
    *flags_child.borrow_mut() = NoticeFlags::empty();

    h.tree.set_layout_rect(parent, 0.1, 0.1, 0.8, 0.6);
    h.tick();

    assert!(
        flags_parent.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must fire on the parent"
    );
    assert!(
        flags_child.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must propagate to child via layout_children"
    );
}

#[test]
fn layout_change_propagates_to_grandchild() {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let flags_parent = Rc::new(RefCell::new(NoticeFlags::empty()));
    let parent = h.add_panel_with(
        root,
        "parent",
        Box::new(PropagatingBehavior::new(flags_parent.clone())),
    );

    let flags_child = Rc::new(RefCell::new(NoticeFlags::empty()));
    let child = h.add_panel_with(
        parent,
        "child",
        Box::new(PropagatingBehavior::new(flags_child.clone())),
    );

    let flags_grandchild = Rc::new(RefCell::new(NoticeFlags::empty()));
    let _grandchild = h.add_panel_with(
        child,
        "grandchild",
        Box::new(NoticeBehavior::new(flags_grandchild.clone())),
    );

    settle(&mut h);
    *flags_parent.borrow_mut() = NoticeFlags::empty();
    *flags_child.borrow_mut() = NoticeFlags::empty();
    *flags_grandchild.borrow_mut() = NoticeFlags::empty();

    h.tree.set_layout_rect(parent, 0.1, 0.1, 0.8, 0.6);
    h.tick();

    assert!(
        flags_parent.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must fire on parent"
    );
    assert!(
        flags_child.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must propagate to child"
    );
    assert!(
        flags_grandchild
            .borrow()
            .contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must propagate to grandchild"
    );
}

#[test]
fn layout_change_does_not_leak_to_sibling() {
    let mut h = PipelineTestHarness::new();
    let root = h.root();

    let flags_a = Rc::new(RefCell::new(NoticeFlags::empty()));
    let sibling_a = h.add_panel_with(
        root,
        "sibling_a",
        Box::new(NoticeBehavior::new(flags_a.clone())),
    );
    h.tree.set_layout_rect(sibling_a, 0.0, 0.0, 0.5, 1.0);

    let flags_b = Rc::new(RefCell::new(NoticeFlags::empty()));
    let sibling_b = h.add_panel_with(
        root,
        "sibling_b",
        Box::new(NoticeBehavior::new(flags_b.clone())),
    );
    h.tree.set_layout_rect(sibling_b, 0.5, 0.0, 0.5, 1.0);

    settle(&mut h);
    *flags_a.borrow_mut() = NoticeFlags::empty();
    *flags_b.borrow_mut() = NoticeFlags::empty();

    h.tree.set_layout_rect(sibling_a, 0.0, 0.0, 0.4, 1.0);
    h.tick();

    assert!(
        flags_a.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must fire on sibling_a"
    );
    assert!(
        !flags_b.borrow().contains(NoticeFlags::LAYOUT_CHANGED),
        "LAYOUT_CHANGED must NOT fire on sibling_b"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// BP-21: Focus change notices
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn focus_change_fires_active_and_focus_changed_on_both_panels() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let panel_a = h.add_panel_with(
        root,
        "a",
        Box::new(NamedRecordingBehavior::new("a", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_a, 0.0, 0.0, 0.5, 1.0);

    let panel_b = h.add_panel_with(
        root,
        "b",
        Box::new(NamedRecordingBehavior::new("b", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_b, 0.5, 0.0, 0.5, 1.0);

    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, panel_a, false);
    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, panel_b, false);
    h.tick();

    let entries = log.borrow();

    let a_entries: Vec<&String> = entries.iter().filter(|e| e.starts_with("a:notice:")).collect();
    let b_entries: Vec<&String> = entries.iter().filter(|e| e.starts_with("b:notice:")).collect();

    assert!(!a_entries.is_empty(), "Panel A (old active) must receive a notice");
    assert!(!b_entries.is_empty(), "Panel B (new active) must receive a notice");

    let has_active_changed =
        |entries: &[&String]| entries.iter().any(|e| e.contains("ACTIVE_CHANGED"));
    assert!(
        has_active_changed(&a_entries),
        "Panel A must receive ACTIVE_CHANGED: got {:?}",
        a_entries
    );
    assert!(
        has_active_changed(&b_entries),
        "Panel B must receive ACTIVE_CHANGED: got {:?}",
        b_entries
    );

    let has_focus_changed = |entries: &[&String]| {
        entries
            .iter()
            .any(|e| e.contains("FOCUS_CHANGED") && !e.contains("VIEW_FOCUS_CHANGED"))
    };
    assert!(
        has_focus_changed(&a_entries),
        "Panel A must receive FOCUS_CHANGED when window is focused: got {:?}",
        a_entries
    );
    assert!(
        has_focus_changed(&b_entries),
        "Panel B must receive FOCUS_CHANGED when window is focused: got {:?}",
        b_entries
    );
}

#[test]
fn focus_change_old_panel_notified_before_new_panel() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let panel_a = h.add_panel_with(
        root,
        "a",
        Box::new(NamedRecordingBehavior::new("a", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_a, 0.0, 0.0, 0.5, 1.0);

    let panel_b = h.add_panel_with(
        root,
        "b",
        Box::new(NamedRecordingBehavior::new("b", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_b, 0.5, 0.0, 0.5, 1.0);

    h.tick();

    h.view.set_active_panel(&mut h.tree, panel_a, false);
    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, panel_b, false);
    h.tick();

    let entries = log.borrow();

    let a_idx = entries
        .iter()
        .position(|e| e.starts_with("a:notice:") && e.contains("ACTIVE_CHANGED"));
    let b_idx = entries
        .iter()
        .position(|e| e.starts_with("b:notice:") && e.contains("ACTIVE_CHANGED"));

    let a_idx = a_idx.unwrap_or_else(|| {
        panic!("Panel A must receive ACTIVE_CHANGED. Log: {:?}", *entries)
    });
    let b_idx = b_idx.unwrap_or_else(|| {
        panic!("Panel B must receive ACTIVE_CHANGED. Log: {:?}", *entries)
    });

    assert!(
        a_idx < b_idx,
        "Old active panel A (idx={}) must be notified before new active panel B (idx={}). Log: {:?}",
        a_idx,
        b_idx,
        *entries
    );
}

#[test]
fn focus_change_ancestor_receives_active_changed() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let parent = h.add_panel_with(
        root,
        "parent",
        Box::new(NamedRecordingBehavior::new("parent", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(parent, 0.0, 0.0, 1.0, 1.0);

    let child = h.add_panel_with(
        parent,
        "child",
        Box::new(NamedRecordingBehavior::new("child", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(child, 0.0, 0.0, 1.0, 1.0);

    let sibling = h.add_panel_with(
        root,
        "sibling",
        Box::new(NamedRecordingBehavior::new("sibling", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(sibling, 0.0, 0.0, 0.5, 1.0);

    h.tick();

    h.view.set_active_panel(&mut h.tree, sibling, false);
    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, child, false);
    h.tick();

    let entries = log.borrow();

    let parent_active = entries
        .iter()
        .any(|e| e.starts_with("parent:notice:") && e.contains("ACTIVE_CHANGED"));
    assert!(
        parent_active,
        "Ancestor (parent) must receive ACTIVE_CHANGED. Log: {:?}",
        *entries
    );

    let sibling_active = entries
        .iter()
        .any(|e| e.starts_with("sibling:notice:") && e.contains("ACTIVE_CHANGED"));
    assert!(
        sibling_active,
        "Old active panel (sibling) must receive ACTIVE_CHANGED. Log: {:?}",
        *entries
    );

    let child_active = entries
        .iter()
        .any(|e| e.starts_with("child:notice:") && e.contains("ACTIVE_CHANGED"));
    assert!(
        child_active,
        "New active panel (child) must receive ACTIVE_CHANGED. Log: {:?}",
        *entries
    );
}

#[test]
fn focus_change_no_focus_changed_when_window_unfocused() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let panel_a = h.add_panel_with(
        root,
        "a",
        Box::new(NamedRecordingBehavior::new("a", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_a, 0.0, 0.0, 0.5, 1.0);

    let panel_b = h.add_panel_with(
        root,
        "b",
        Box::new(NamedRecordingBehavior::new("b", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_b, 0.5, 0.0, 0.5, 1.0);

    h.tick();

    h.view.set_active_panel(&mut h.tree, panel_a, false);
    h.tick();

    h.view.set_window_focused(&mut h.tree, false);
    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, panel_b, false);
    h.tick();

    let entries = log.borrow();

    let has_active_changed = entries.iter().any(|e| e.contains("ACTIVE_CHANGED"));
    assert!(
        has_active_changed,
        "ACTIVE_CHANGED must fire even when window is unfocused. Log: {:?}",
        *entries
    );

    let has_focus_changed = entries.iter().any(|e| {
        let stripped = e.replace("VIEW_FOCUS_CHANGED", "");
        stripped.contains("FOCUS_CHANGED")
    });
    assert!(
        !has_focus_changed,
        "FOCUS_CHANGED must NOT fire when window is unfocused. Log: {:?}",
        *entries
    );
}

#[test]
fn focus_change_same_panel_is_noop() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let panel_a = h.add_panel_with(
        root,
        "a",
        Box::new(NamedRecordingBehavior::new("a", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(panel_a, 0.0, 0.0, 1.0, 1.0);

    h.tick();

    h.view.set_active_panel(&mut h.tree, panel_a, false);
    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, panel_a, false);
    h.tick();

    let entries = log.borrow();

    let has_active_changed = entries.iter().any(|e| e.contains("ACTIVE_CHANGED"));
    assert!(
        !has_active_changed,
        "Re-activating the same panel must not fire ACTIVE_CHANGED. Log: {:?}",
        *entries
    );
}

#[test]
fn focus_change_shared_ancestor_receives_notice() {
    let mut h = TestHarness::new();
    let root = h.root();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let mid = h.add_panel_with(
        root,
        "mid",
        Box::new(NamedRecordingBehavior::new("mid", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(mid, 0.0, 0.0, 1.0, 1.0);

    let child_a = h.add_panel_with(
        mid,
        "child_a",
        Box::new(NamedRecordingBehavior::new("child_a", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(child_a, 0.0, 0.0, 0.5, 1.0);

    let child_b = h.add_panel_with(
        mid,
        "child_b",
        Box::new(NamedRecordingBehavior::new("child_b", Rc::clone(&log))),
    );
    h.tree.set_layout_rect(child_b, 0.5, 0.0, 0.5, 1.0);

    h.tick();

    h.view.set_active_panel(&mut h.tree, child_a, false);
    h.tick();
    log.borrow_mut().clear();

    h.view.set_active_panel(&mut h.tree, child_b, false);
    h.tick();

    let entries = log.borrow();

    let mid_active = entries
        .iter()
        .any(|e| e.starts_with("mid:notice:") && e.contains("ACTIVE_CHANGED"));
    assert!(
        mid_active,
        "Shared ancestor (mid) must receive ACTIVE_CHANGED. Log: {:?}",
        *entries
    );

    let ca = entries
        .iter()
        .any(|e| e.starts_with("child_a:notice:") && e.contains("ACTIVE_CHANGED"));
    let cb = entries
        .iter()
        .any(|e| e.starts_with("child_b:notice:") && e.contains("ACTIVE_CHANGED"));
    assert!(ca, "child_a (old active) must get ACTIVE_CHANGED");
    assert!(cb, "child_b (new active) must get ACTIVE_CHANGED");
}

// ═══════════════════════════════════════════════════════════════════════
// BP-22: Enable change propagation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn disable_parent_fires_enable_changed_on_parent() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);
    h.tick();
    clear_flags(&parent_acc);

    h.tree.set_enable_switch(parent, false);
    h.tick();

    assert!(
        has_enable_changed(&parent_acc),
        "Disabling a panel must fire ENABLE_CHANGED on that panel"
    );
}

#[test]
fn disable_parent_propagates_enable_changed_to_descendants() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);

    let (child_acc, child_beh) = notice_pair();
    let child = h.add_panel_with(parent, "child", child_beh);

    let (grandchild_acc, grandchild_beh) = notice_pair();
    let _grandchild = h.add_panel_with(child, "grandchild", grandchild_beh);

    h.tick();
    clear_flags(&parent_acc);
    clear_flags(&child_acc);
    clear_flags(&grandchild_acc);

    h.tree.set_enable_switch(parent, false);
    h.tick();

    assert!(has_enable_changed(&parent_acc), "Parent must get ENABLE_CHANGED");
    assert!(
        has_enable_changed(&child_acc),
        "Child must get ENABLE_CHANGED when ancestor is disabled"
    );
    assert!(
        has_enable_changed(&grandchild_acc),
        "Grandchild must get ENABLE_CHANGED when ancestor is disabled"
    );
}

#[test]
fn reenable_parent_fires_enable_changed_on_parent_and_descendants() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);

    let (child_acc, child_beh) = notice_pair();
    let child = h.add_panel_with(parent, "child", child_beh);

    let (grandchild_acc, grandchild_beh) = notice_pair();
    let _grandchild = h.add_panel_with(child, "grandchild", grandchild_beh);

    h.tick();
    h.tree.set_enable_switch(parent, false);
    h.tick();
    clear_flags(&parent_acc);
    clear_flags(&child_acc);
    clear_flags(&grandchild_acc);

    h.tree.set_enable_switch(parent, true);
    h.tick();

    assert!(
        has_enable_changed(&parent_acc),
        "Re-enabling parent must fire ENABLE_CHANGED on parent"
    );
    assert!(
        has_enable_changed(&child_acc),
        "Re-enabling parent must fire ENABLE_CHANGED on child"
    );
    assert!(
        has_enable_changed(&grandchild_acc),
        "Re-enabling parent must fire ENABLE_CHANGED on grandchild"
    );
}

#[test]
fn sibling_branch_does_not_get_enable_changed() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (branch_a_acc, branch_a_beh) = notice_pair();
    let branch_a = h.add_panel_with(root, "branch_a", branch_a_beh);

    let (child_a_acc, child_a_beh) = notice_pair();
    let _child_a = h.add_panel_with(branch_a, "child_a", child_a_beh);

    let (branch_b_acc, branch_b_beh) = notice_pair();
    let branch_b = h.add_panel_with(root, "branch_b", branch_b_beh);

    let (child_b_acc, child_b_beh) = notice_pair();
    let _child_b = h.add_panel_with(branch_b, "child_b", child_b_beh);

    h.tick();
    clear_flags(&branch_a_acc);
    clear_flags(&child_a_acc);
    clear_flags(&branch_b_acc);
    clear_flags(&child_b_acc);

    h.tree.set_enable_switch(branch_a, false);
    h.tick();

    assert!(has_enable_changed(&branch_a_acc), "Disabled branch root must get ENABLE_CHANGED");
    assert!(has_enable_changed(&child_a_acc), "Disabled branch child must get ENABLE_CHANGED");
    assert!(
        !has_enable_changed(&branch_b_acc),
        "Sibling branch must NOT get ENABLE_CHANGED"
    );
    assert!(
        !has_enable_changed(&child_b_acc),
        "Sibling branch child must NOT get ENABLE_CHANGED"
    );
}

#[test]
fn disable_already_disabled_is_noop() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);

    h.tick();
    h.tree.set_enable_switch(parent, false);
    h.tick();
    clear_flags(&parent_acc);

    h.tree.set_enable_switch(parent, false);
    h.tick();

    assert!(
        !has_enable_changed(&parent_acc),
        "Disabling an already-disabled panel must not fire ENABLE_CHANGED"
    );
}

#[test]
fn enable_already_enabled_is_noop() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);

    h.tick();
    clear_flags(&parent_acc);

    h.tree.set_enable_switch(parent, true);
    h.tick();

    assert!(
        !has_enable_changed(&parent_acc),
        "Enabling an already-enabled panel must not fire ENABLE_CHANGED"
    );
}

#[test]
fn child_with_own_disable_stays_disabled_on_parent_reenable() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);

    let (child_acc, child_beh) = notice_pair();
    let child = h.add_panel_with(parent, "child", child_beh);

    let (grandchild_acc, grandchild_beh) = notice_pair();
    let _grandchild = h.add_panel_with(child, "grandchild", grandchild_beh);

    h.tick();

    h.tree.set_enable_switch(child, false);
    h.tick();

    h.tree.set_enable_switch(parent, false);
    h.tick();
    clear_flags(&parent_acc);
    clear_flags(&child_acc);
    clear_flags(&grandchild_acc);

    h.tree.set_enable_switch(parent, true);
    h.tick();

    assert!(
        has_enable_changed(&parent_acc),
        "Parent must get ENABLE_CHANGED on re-enable"
    );
    assert!(
        !has_enable_changed(&child_acc),
        "Child with own enable_switch=false must NOT get ENABLE_CHANGED \
         when parent is re-enabled"
    );
    assert!(
        !has_enable_changed(&grandchild_acc),
        "Grandchild under a disabled child must NOT get ENABLE_CHANGED \
         when grandparent is re-enabled"
    );
}

#[test]
fn disable_propagates_to_all_children_not_just_first() {
    let mut h = TestHarness::new();
    let root = h.root();

    let (parent_acc, parent_beh) = notice_pair();
    let parent = h.add_panel_with(root, "parent", parent_beh);

    let (c1_acc, c1_beh) = notice_pair();
    let _c1 = h.add_panel_with(parent, "child1", c1_beh);

    let (c2_acc, c2_beh) = notice_pair();
    let _c2 = h.add_panel_with(parent, "child2", c2_beh);

    let (c3_acc, c3_beh) = notice_pair();
    let _c3 = h.add_panel_with(parent, "child3", c3_beh);

    h.tick();
    clear_flags(&parent_acc);
    clear_flags(&c1_acc);
    clear_flags(&c2_acc);
    clear_flags(&c3_acc);

    h.tree.set_enable_switch(parent, false);
    h.tick();

    assert!(has_enable_changed(&parent_acc), "Parent must get notice");
    assert!(has_enable_changed(&c1_acc), "First child must get ENABLE_CHANGED");
    assert!(has_enable_changed(&c2_acc), "Second child must get ENABLE_CHANGED");
    assert!(has_enable_changed(&c3_acc), "Third child must get ENABLE_CHANGED");
}

// ═══════════════════════════════════════════════════════════════════════
// BP-23: Children change notice
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn add_child_fires_children_changed_on_parent() {
    let mut h = TestHarness::new();
    let root = h.root();

    let acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let parent = h.add_panel_with(root, "parent", Box::new(NoticeBehavior::new(acc.clone())));
    h.tick();
    acc.borrow_mut().remove(NoticeFlags::all());

    let _child = h.add_panel(parent, "child");
    h.tick();

    assert!(
        acc.borrow().contains(NoticeFlags::CHILDREN_CHANGED),
        "Adding a child should fire CHILDREN_CHANGED on the parent"
    );
}

#[test]
fn remove_child_fires_children_changed_on_parent() {
    let mut h = TestHarness::new();
    let root = h.root();

    let acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let parent = h.add_panel_with(root, "parent", Box::new(NoticeBehavior::new(acc.clone())));
    let child = h.add_panel(parent, "child");
    h.tick();
    acc.borrow_mut().remove(NoticeFlags::all());

    h.tree.remove(child);
    h.tick();

    assert!(
        acc.borrow().contains(NoticeFlags::CHILDREN_CHANGED),
        "Removing a child should fire CHILDREN_CHANGED on the parent"
    );
}

#[test]
fn add_child_does_not_fire_children_changed_on_grandparent() {
    let mut h = TestHarness::new();
    let root = h.root();

    let gp_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let grandparent =
        h.add_panel_with(root, "grandparent", Box::new(NoticeBehavior::new(gp_acc.clone())));
    let parent = h.add_panel(grandparent, "parent");
    h.tick();
    gp_acc.borrow_mut().remove(NoticeFlags::all());

    let _child = h.add_panel(parent, "child");
    h.tick();

    assert!(
        !gp_acc.borrow().contains(NoticeFlags::CHILDREN_CHANGED),
        "Adding a child to parent should NOT fire CHILDREN_CHANGED on grandparent"
    );
}

#[test]
fn remove_child_does_not_fire_children_changed_on_grandparent() {
    let mut h = TestHarness::new();
    let root = h.root();

    let gp_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let grandparent =
        h.add_panel_with(root, "grandparent", Box::new(NoticeBehavior::new(gp_acc.clone())));
    let parent = h.add_panel(grandparent, "parent");
    let child = h.add_panel(parent, "child");
    h.tick();
    gp_acc.borrow_mut().remove(NoticeFlags::all());

    h.tree.remove(child);
    h.tick();

    assert!(
        !gp_acc.borrow().contains(NoticeFlags::CHILDREN_CHANGED),
        "Removing a child from parent should NOT fire CHILDREN_CHANGED on grandparent"
    );
}

#[test]
fn add_multiple_children_fires_children_changed_on_parent() {
    let mut h = TestHarness::new();
    let root = h.root();

    let acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let parent = h.add_panel_with(root, "parent", Box::new(NoticeBehavior::new(acc.clone())));
    h.tick();
    acc.borrow_mut().remove(NoticeFlags::all());

    let _c1 = h.add_panel(parent, "c1");
    let _c2 = h.add_panel(parent, "c2");
    let _c3 = h.add_panel(parent, "c3");
    h.tick();

    assert!(
        acc.borrow().contains(NoticeFlags::CHILDREN_CHANGED),
        "Adding multiple children should fire CHILDREN_CHANGED on the parent"
    );
}

#[test]
fn children_changed_is_pending_immediately_after_add() {
    let mut h = TestHarness::new();
    let root = h.root();

    let parent = h.add_panel(root, "parent");
    h.tick();

    let _child = h.add_panel(parent, "child");

    assert!(
        h.tree
            .pending_notices(parent)
            .contains(NoticeFlags::CHILDREN_CHANGED),
        "CHILDREN_CHANGED should be pending on parent immediately after create_child"
    );
}

#[test]
fn children_changed_is_pending_immediately_after_remove() {
    let mut h = TestHarness::new();
    let root = h.root();

    let parent = h.add_panel(root, "parent");
    let child = h.add_panel(parent, "child");
    h.tick();

    h.tree.remove(child);

    assert!(
        h.tree
            .pending_notices(parent)
            .contains(NoticeFlags::CHILDREN_CHANGED),
        "CHILDREN_CHANGED should be pending on parent immediately after remove"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// BP-24: Active change notice
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn active_changed_fires_on_old_active_panel() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(
        la_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "Old active panel (leaf_a) should receive ACTIVE_CHANGED"
    );
}

#[test]
fn active_changed_fires_on_new_active_panel() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(
        lb_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "New active panel (leaf_b) should receive ACTIVE_CHANGED"
    );
}

#[test]
fn active_changed_fires_on_old_active_ancestor() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(
        ba_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "Old active ancestor (branch_a) should receive ACTIVE_CHANGED"
    );
}

#[test]
fn active_changed_fires_on_new_active_ancestor() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(
        bb_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "New active ancestor (branch_b) should receive ACTIVE_CHANGED"
    );
}

#[test]
fn shared_ancestor_receives_active_changed() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(
        root_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "Shared ancestor (root) should receive ACTIVE_CHANGED"
    );

    let root_entries: Vec<_> = log
        .borrow()
        .iter()
        .filter(|s| s == &"root:ACTIVE_CHANGED")
        .cloned()
        .collect();
    assert_eq!(
        root_entries.len(),
        1,
        "Shared ancestor (root) should receive ACTIVE_CHANGED exactly once, got {}",
        root_entries.len()
    );
}

#[test]
fn non_path_panels_do_not_receive_active_changed() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    let bystander_acc = Rc::new(RefCell::new(NoticeFlags::empty()));
    let _bystander = h.add_panel_with(
        root,
        "bystander",
        Box::new(LabeledNoticeBehavior::new(
            "bystander",
            log.clone(),
            bystander_acc.clone(),
        )),
    );

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(
        &mut h,
        &log,
        &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc, &bystander_acc],
    );

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(
        !bystander_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "Bystander panel (not on old or new active path) should NOT receive ACTIVE_CHANGED"
    );
}

#[test]
fn delivery_order_old_active_before_new_active() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    let entries = log.borrow().clone();

    let old_leaf_pos = entries
        .iter()
        .position(|s| s == "leaf_a:ACTIVE_CHANGED")
        .expect("leaf_a should have received ACTIVE_CHANGED");
    let new_leaf_pos = entries
        .iter()
        .position(|s| s == "leaf_b:ACTIVE_CHANGED")
        .expect("leaf_b should have received ACTIVE_CHANGED");

    let old_branch_pos = entries
        .iter()
        .position(|s| s == "branch_a:ACTIVE_CHANGED")
        .expect("branch_a should have received ACTIVE_CHANGED");
    let new_branch_pos = entries
        .iter()
        .position(|s| s == "branch_b:ACTIVE_CHANGED")
        .expect("branch_b should have received ACTIVE_CHANGED");

    assert!(
        old_branch_pos < new_branch_pos,
        "Old active ancestor (branch_a, pos={}) should be notified before \
         new active ancestor (branch_b, pos={})",
        old_branch_pos,
        new_branch_pos
    );

    assert!(
        old_leaf_pos < new_leaf_pos,
        "Old active panel (leaf_a, pos={}) should be notified before \
         new active panel (leaf_b, pos={})",
        old_leaf_pos,
        new_leaf_pos
    );
}

#[test]
fn no_active_changed_when_reactivating_same_panel() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, _leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    h.tick();

    assert!(
        !la_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED),
        "Re-activating the same panel should NOT fire ACTIVE_CHANGED"
    );

    let active_entries: Vec<_> = log
        .borrow()
        .iter()
        .filter(|s| s.contains("ACTIVE_CHANGED"))
        .cloned()
        .collect();
    assert!(
        active_entries.is_empty(),
        "No panel should receive ACTIVE_CHANGED when re-activating the same panel, got: {:?}",
        active_entries
    );
}

#[test]
fn all_panels_on_both_paths_receive_active_changed() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (_root, root_acc, _ba, ba_acc, leaf_a, la_acc, _bb, bb_acc, leaf_b, lb_acc) =
        build_labeled_tree(&mut h, &log);

    h.view.set_active_panel(&mut h.tree, leaf_a, false);
    flush_and_clear(&mut h, &log, &[&root_acc, &ba_acc, &la_acc, &bb_acc, &lb_acc]);

    h.view.set_active_panel(&mut h.tree, leaf_b, false);
    h.tick();

    assert!(la_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED), "leaf_a must get ACTIVE_CHANGED");
    assert!(ba_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED), "branch_a must get ACTIVE_CHANGED");
    assert!(root_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED), "root must get ACTIVE_CHANGED");
    assert!(lb_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED), "leaf_b must get ACTIVE_CHANGED");
    assert!(bb_acc.borrow().contains(NoticeFlags::ACTIVE_CHANGED), "branch_b must get ACTIVE_CHANGED");
}
