//! Task-8 Path B popup-cancellation test: if the view zooms back inside
//! the home rect before `App::about_to_wait` has a chance to materialize
//! the popup's OS surface, the popup is dropped cleanly by emView's
//! teardown path (emView.cpp:1676-1680). Because the popup lives on
//! `emView::PopupWindow` (never in a framework `pending_popups` map), the
//! teardown is a plain `Option::take` — no cross-frame dangling state.
//!
//! The `App::materialize_pending_popup` drain would then find no view
//! holding a `Pending` popup and silently no-op (verified by code
//! inspection; full-App harness requires an ActiveEventLoop).

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
fn popup_torn_down_before_materialize_leaves_no_observable_state() {
    let mut ts = TestSched::new();
    let (mut tree, root, child) = setup_tree();
    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 640.0, 480.0);

    // Bring the view into steady state, enable POPUP_ZOOM, then zoom
    // outside home → popup allocated in Pending state.
    ts.with(|sc| view.Update(&mut tree, sc));
    ts.with(|sc| view.SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree, sc));
    ts.with(|sc| view.RawVisit(&mut tree, child, 0.0, 0.0, 0.1, true, sc));
    assert!(
        view.PopupWindow.is_some(),
        "precondition: POPUP_ZOOM outside-home visit must allocate popup"
    );
    assert!(view.PopupCloseSignal.is_some());

    // Simulate the cancellation: zoom back inside the home rect. ZoomOut
    // visits the root with rel=zoom_out_rel_a (fully fits home), which
    // triggers the `else if PopupWindow.is_some()` branch in RawVisitAbs
    // (emView.cpp:1676-1680) and drops the popup.
    ts.with(|sc| view.ZoomOut(&mut tree, sc));

    // Observable contract (Task-8 Path B):
    //   1. emView::PopupWindow is None — the popup was dropped in-place.
    //   2. PopupCloseSignal mirror is cleared (no dangling signal-id).
    //   3. No framework `pending_popups` map exists to leak entries into,
    //      because popups live on emView directly (matches C++ ownership).
    assert!(
        view.PopupWindow.is_none(),
        "teardown must drop PopupWindow in place (emView.cpp:1676-1680)"
    );
    assert!(
        view.PopupCloseSignal.is_none(),
        "popup close-signal mirror must be cleared on teardown"
    );
}
