// Rust-only regression test for emMainControlPanel::LayoutChildren.
// Verifies that the panel creates the expected child tree matching C++ structure.
// No C++ golden data needed — these verify structural correctness.

use std::rc::Rc;

use emcore::emContext::emContext;
use emcore::emPanel::PanelBehavior;
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::PanelTree;

use emMain::emMainControlPanel::emMainControlPanel;

#[test]
fn control_panel_layout_children() {
    let ctx = emContext::NewRoot();
    let mut panel = emMainControlPanel::new(Rc::clone(&ctx), None);

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("ctrl_root");
    // Give root a 1:1 layout so normalized coordinates are [0,1] x [0,1].
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    // Call LayoutChildren — this creates children AND positions them.
    {
        let mut pctx = PanelCtx::new(&mut tree, root, 1.0);
        panel.LayoutChildren(&mut pctx);
    }

    // Top-level has 1 child: "lMain" (wrapping general + bookmarks).
    let children: Vec<_> = tree.children(root).collect();
    assert_eq!(
        children.len(),
        1,
        "Expected 1 top-level child (lMain), got {}",
        children.len()
    );

    // The child should have a non-zero layout rect.
    let rect = tree
        .layout_rect(children[0])
        .expect("lMain has no layout rect");
    assert!(
        rect.w > 0.0 && rect.h > 0.0,
        "lMain: expected non-zero rect, got {rect:?}"
    );
}

#[test]
fn control_panel_child_names() {
    let ctx = emContext::NewRoot();
    let mut panel = emMainControlPanel::new(Rc::clone(&ctx), None);

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("ctrl_root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    {
        let mut pctx = PanelCtx::new(&mut tree, root, 1.0);
        panel.LayoutChildren(&mut pctx);
    }

    let children: Vec<_> = tree.children(root).collect();
    let names: Vec<&str> = children.iter().map(|&id| tree.name(id).unwrap()).collect();

    // C++ top-level child 0: lMain (contains general + bookmarks).
    assert_eq!(names, vec!["lMain"]);
}
