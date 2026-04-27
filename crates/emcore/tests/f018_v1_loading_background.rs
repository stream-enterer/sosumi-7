//! V.1 regression-prevention harness for F018: VFS_LOADING background must be
//! `view.background_color`, not BLACK.
//!
//! Spec: docs/superpowers/specs/2026-04-25-F018-compositor-integration-contract-design.md §V.1
//! Audit: docs/debug/F018-audit.md §III.1, §I.1, §I.4
//!
//! This test asserts the post-fix invariant in `SoftwareCompositor`. The
//! conditional `ClearWithCanvas(view.background_color, ...)` recorded by
//! `emView::Paint` for non-opaque SVPs already overwrites the BLACK pre-fill —
//! so this test passes today. After F018 Phase 1 plumbs `view.background_color`
//! through the unconditional pre-fill sites (`emViewRenderer.rs` and
//! `emWindow.rs`), it continues to pass. The test catches regressions in
//! either layer. The wgpu `LoadOp::Clear` path
//! (`emViewRendererCompositor::render_frame`) and the `emWindow` per-tile
//! `tile.image.fill` are not exercised here — they are covered by F018 Task 5's
//! manual visual gate against the eaglemode binary.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emFileModel::{emFileModel, FileModelState};
use emcore::emFilePanel::{emFilePanel, VirtualFileState};
use emcore::emPanelTree::{PanelId, PanelTree};
use emcore::emSignal::SignalId;
use emcore::emView::emView;
use emcore::emViewRenderer::SoftwareCompositor;
use emcore::test_view_harness::TestSched;

/// Spec V.1: a non-opaque SVP (emFilePanel during VFS_LOADING) must reveal
/// `view.background_color` in regions it does not paint.
#[test]
fn vfs_loading_reveals_background_color() {
    // Framebuffer dimensions for the compositor under test. The view, panel
    // tree, and compositor all share these dimensions so the panel rect spans
    // the full framebuffer.
    const FB_DIM: u32 = 256;
    // Top-left corner sample: far from the centered loading widget regardless
    // of its exact extent. (4, 4) keeps a 4-pixel margin from the panel edge
    // to avoid any edge-pixel coverage artifacts. Decoupling the sample point
    // from the loading-widget layout means future widget-size changes do not
    // silently invalidate this test.
    const SAMPLE_X: u32 = 4;
    const SAMPLE_Y: u32 = 4;

    // Use a distinct, non-default color so the assertion proves we observe
    // the configured value, not a default that coincidentally matches.
    let bg = emColor::rgba(0xFF, 0x00, 0x00, 0xFF); // opaque red

    let (mut tree, panel_id) = build_loading_directory_panel();
    let mut view = build_view_with_svp(&mut tree, panel_id);
    view.SetBackgroundColor(bg);

    let mut compositor = SoftwareCompositor::new(FB_DIM, FB_DIM);
    compositor.render(&mut tree, &view);

    // V.1 asserts an exact-color match: the conditional ClearWithCanvas writes
    // opaque background_color, no blending at the sample point.
    let fb = compositor.framebuffer();
    let px = fb.GetPixel(SAMPLE_X, SAMPLE_Y);
    let px_color = emColor::rgba(px[0], px[1], px[2], px[3]);
    let tol = 0;
    assert!(
        channel_diff(px_color, bg) <= tol,
        "expected background red ~{:?}, got {:?}",
        bg,
        px_color,
    );
}

fn channel_diff(a: emColor, b: emColor) -> i32 {
    let (ar, ag, ab, _aa) = (
        a.GetRed() as i32,
        a.GetGreen() as i32,
        a.GetBlue() as i32,
        a.GetAlpha() as i32,
    );
    let (br, bg_, bb, _ba) = (
        b.GetRed() as i32,
        b.GetGreen() as i32,
        b.GetBlue() as i32,
        b.GetAlpha() as i32,
    );
    (ar - br).abs().max((ag - bg_).abs()).max((ab - bb).abs())
}

/// Construct a minimal `PanelTree` containing one `emFilePanel` whose attached
/// `emFileModel` has been transitioned to `FileState::Loading { progress: 42.0 }`,
/// which `emFilePanel::compute_vir_file_state` then maps to
/// `VirtualFileState::Loading { progress: 42.0 }` — the Rust analogue of
/// C++ `VFS_LOADING`. The panel is wired in as the tree's root behavior; the
/// view installed by `build_view_with_svp` then selects it as the SVP.
fn build_loading_directory_panel() -> (PanelTree, PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    // Build an emFileModel<String> and force it into Loading state with a
    // mid-progress value (any non-final progress puts it in VFS_LOADING).
    let model: Rc<RefCell<emFileModel<String>>> = Rc::new(RefCell::new(emFileModel::new(
        PathBuf::from("/tmp/f018_v1_loading_background.test"),
        SignalId::default(),
    )));
    {
        let mut m = model.borrow_mut();
        assert!(m.Load(), "model.Load() must transition Waiting -> Loading");
        m.set_progress(42.0);
    }

    // Attach to a fresh emFilePanel and confirm it observes VFS_LOADING.
    let mut panel = emFilePanel::new();
    panel.SetFileModel(Some(model.clone() as Rc<RefCell<dyn FileModelState>>));
    debug_assert!(
        matches!(panel.GetVirFileState(), VirtualFileState::Loading { .. }),
        "emFilePanel must observe VFS_LOADING after model.Load() + set_progress(42.0); \
         saw {:?}",
        panel.GetVirFileState()
    );

    tree.put_behavior(root, Box::new(panel));

    (tree, root)
}

/// Build the `emView` and run a settle pass so the SVP is selected.
/// Sets the panel-tree's view, then drives `view.Update` (which internally
/// drains pending notices via `HandleNotice` until quiet — see
/// `emView::Update`). After this, `view.GetSupremeViewedPanel()` is
/// `Some(panel_id)`.
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
