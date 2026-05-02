//! IV.3: emView::InvalidatePainting must set SVPChoiceByOpacityInvalid = true
//! to mirror C++ emPanel::InvalidatePainting (emPanel.cpp:1284-1290, 1296-1302).
//!
//! Spec: docs/superpowers/specs/2026-04-25-F018-compositor-integration-contract-design.md §IV.3
//! Audit: docs/debug/F018-audit.md §IV.3
//!
//! Build note: requires the `test-support` feature. The workspace-wide
//! `cargo nextest run` enables it transitively via sibling crates;
//! direct `cargo test -p emcore --test f018_iv3_svpchoice_invalidation`
//! invocations need `--features test-support`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emFileModel::{emFileModel, FileModelState};
use emcore::emFilePanel::emFilePanel;
use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emView::emView;
use emcore::test_view_harness::TestSched;

#[test]
fn invalidate_painting_sets_svp_choice_by_opacity_invalid() {
    let (mut tree, panel_id) = build_minimal_tree();
    let mut view = build_view_with_svp(&mut tree, panel_id);

    // Clear the post-Update state so we observe only InvalidatePainting's
    // effect. The settle pass may set SVPChoiceByOpacityInvalid as part of
    // initial layout; its read-side handler in emView::Update clears it after
    // recomputing the SVP. We assert the precondition explicitly.
    view.SVPChoiceByOpacityInvalid = false;
    assert!(!view.SVPChoiceByOpacityInvalid, "precondition");

    let mut ts = TestSched::new();
    ts.with(|sc| view.InvalidatePainting(sc, &tree, panel_id));

    assert!(
        view.SVPChoiceByOpacityInvalid,
        "InvalidatePainting must set SVPChoiceByOpacityInvalid = true \
         (mirrors C++ emPanel::InvalidatePainting at emPanel.cpp:1284-1290)"
    );
}

#[test]
fn invalidate_painting_rect_sets_svp_choice_by_opacity_invalid() {
    let (mut tree, panel_id) = build_minimal_tree();
    let mut view = build_view_with_svp(&mut tree, panel_id);

    view.SVPChoiceByOpacityInvalid = false;
    assert!(!view.SVPChoiceByOpacityInvalid, "precondition");

    let mut ts = TestSched::new();
    ts.with(|sc| view.invalidate_painting_rect(sc, &tree, panel_id, 0.0, 0.0, 1.0, 1.0));

    assert!(
        view.SVPChoiceByOpacityInvalid,
        "invalidate_painting_rect must set SVPChoiceByOpacityInvalid = true \
         (mirrors C++ emPanel::InvalidatePainting rect overload at emPanel.cpp:1296-1302)"
    );
}

/// Construct a minimal `PanelTree` with one `emFilePanel` as the root behavior.
/// We don't need a Loading state for IV.3 — any viewed panel that the
/// InvalidatePainting overloads accept will do. The panel just needs to be
/// `viewed = true` after the settle pass, which `build_view_with_svp`
/// guarantees by selecting it as the SVP.
fn build_minimal_tree() -> (PanelTree, PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let model: Rc<RefCell<emFileModel<String>>> = Rc::new(RefCell::new(emFileModel::new(
        PathBuf::from("/tmp/f018_iv3_svpchoice_invalidation.test"),
    )));

    let mut panel = emFilePanel::new();
    panel.SetFileModel(Some(model.clone() as Rc<RefCell<dyn FileModelState>>));

    tree.put_behavior(root, Box::new(panel));

    (tree, root)
}

/// Build the `emView` and run a settle pass so the panel becomes viewed
/// (InvalidatePainting and invalidate_painting_rect early-return on
/// non-viewed panels).
fn build_view_with_svp(tree: &mut PanelTree, panel_id: PanelId) -> emView {
    let mut view = emView::new(
        emcore::emContext::emContext::NewRoot(),
        panel_id,
        256.0,
        256.0,
    );
    tree.set_panel_view(panel_id);

    let mut ts = TestSched::new();
    ts.with(|sc| view.Update(tree, sc));

    assert!(
        view.GetSupremeViewedPanel().is_some(),
        "settle loop must populate SupremeViewedPanel"
    );
    view
}
