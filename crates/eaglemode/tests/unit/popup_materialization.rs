//! Task-8 Path B popup-lifecycle test: under `VF_POPUP_ZOOM`, a zoom that
//! exceeds the home rect allocates a popup `emWindow` in `OsSurface::Pending`
//! and stores it on `emView::PopupWindow`. The popup is NOT inserted into
//! `App::windows`; it lives on the launching view for its entire lifetime
//! (matches C++ emView.cpp:1636-1642 ownership).
//!
//! Full OS-surface materialization requires winit's `ActiveEventLoop` and
//! is exercised by the winit-integration tests. Here we assert the
//! headless-observable contract: the popup is allocated, owned by emView,
//! and reachable via `emView::PopupWindow`.

use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emView::{emView, ViewFlags};
use emcore::test_view_harness::TestSched;

fn setup_tree() -> (PanelTree, PanelId, PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
    let child = tree.create_child(root, "child", None);
    tree.Layout(child, 0.0, 0.0, 0.5, 1.0, 1.0, None);
    (tree, root, child)
}

#[test]
fn popup_allocated_in_pending_state_owned_by_view() {
    let mut ts = TestSched::new();
    let (mut tree, root, child) = setup_tree();
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 640.0, 480.0);

    // First Update handles zoomed_out_before_sg (zoom-to-root).
    ts.with(|sc| view.Update(&mut tree, sc));
    // Enable popup-zoom mode.
    ts.with(|sc| view.SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree, sc));
    // Visit child with very small rel → produces a zoom rect larger than
    // the home rect; ancestor-clamp ascent triggers the popup branch.
    ts.with(|sc| view.RawVisit(&mut tree, child, 0.0, 0.0, 0.1, true, sc));

    // Observable contract (Task-8 Path B):
    //   1. Popup is allocated and owned by emView.
    //   2. App::windows is untouched by the allocation (popup lives on
    //      the view, not the framework registry) — no App exists in
    //      this test; popup creation must not require one.
    //   3. OsSurface is Pending (winit materialization is deferred to the
    //      next App::about_to_wait drain, which requires a live event loop).
    let popup = view
        .PopupWindow
        .as_ref()
        .expect("POPUP_ZOOM + outside-home visit must allocate PopupWindow");
    assert!(
        !popup.is_materialized(),
        "popup must start in Pending (not-yet-materialized) state; OS \
         surface creation is deferred until App::about_to_wait drains \
         the framework actions"
    );
    assert!(
        view.PopupCloseSignal.is_some(),
        "popup close-signal mirror must be populated alongside PopupWindow"
    );
}
