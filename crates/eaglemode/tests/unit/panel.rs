use emcore::emColor::emColor;
use emcore::emPanel::Rect;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};

use emcore::emPanelCtx::PanelCtx;

use emcore::emPanelTree::{PanelId, PanelTree};

use emcore::emView::{emView, ViewFlags};

use emcore::emPainter::emPainter;

struct TestBehavior {
    paint_count: u32,
    last_notice: NoticeFlags,
}

impl TestBehavior {
    fn new() -> Self {
        Self {
            paint_count: 0,
            last_notice: NoticeFlags::empty(),
        }
    }
}

impl PanelBehavior for TestBehavior {
    fn Paint(&mut self, _painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {
        self.paint_count += 1;
    }

    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        self.last_notice = flags;
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

#[test]
fn create_and_remove_panels() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    assert!(tree.contains(root));
    assert_eq!(tree.len(), 1);

    let child = tree.create_child(root, "child");
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.GetParentContext(child), Some(root));

    tree.remove(child);
    assert!(!tree.contains(child));
    assert_eq!(tree.len(), 1);
}

#[test]
fn child_iteration() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    let a = tree.create_child(root, "a");
    let b = tree.create_child(root, "b");
    let c = tree.create_child(root, "c");

    let children: Vec<PanelId> = tree.children(root).collect();
    assert_eq!(children, vec![a, b, c]);
    assert_eq!(tree.child_count(root), 3);
}

#[test]
fn name_lookup() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    let child = tree.create_child(root, "my_panel");

    assert_eq!(tree.find_by_name("my_panel"), Some(child));
    assert_eq!(tree.find_by_name("nonexistent"), None);

    tree.remove(child);
    assert_eq!(tree.find_by_name("my_panel"), None);
}

#[test]
fn panel_ctx_operations() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");

    // Use PanelCtx to create a child
    {
        let mut ctx = PanelCtx::new(&mut tree, root, 1.0);
        let child = ctx.create_child("child_via_ctx");
        ctx.layout_child(child, 10.0, 20.0, 100.0, 50.0);
        assert_eq!(ctx.name(), "root");
        assert_eq!(ctx.children().len(), 1);
    }

    let child_id = tree.find_by_name("child_via_ctx").unwrap();
    let layout = tree.layout_rect(child_id).unwrap();
    assert_eq!(layout, Rect::new(10.0, 20.0, 100.0, 50.0));
}

#[test]
fn notice_flag_propagation() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_behavior(root, Box::new(TestBehavior::new()));

    // Creating a child should set CHILDREN_CHANGED on GetParentContext
    let _child = tree.create_child(root, "child");

    // Verify notice is pending before delivery
    assert!(tree
        .pending_notices(root)
        .contains(NoticeFlags::CHILD_LIST_CHANGED));

    // Deliver notices
    let mut view =
        emcore::emView::emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.HandleNotice(&mut tree);

    // Verify notices were cleared after delivery
    assert!(tree.pending_notices(root).is_empty());
}

#[test]
fn remove_subtree() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    let parent = tree.create_child(root, "parent");
    let child1 = tree.create_child(parent, "child1");
    let child2 = tree.create_child(parent, "child2");
    let grandchild = tree.create_child(child1, "grandchild");
    assert_eq!(tree.len(), 5);

    // Remove GetParentContext and all descendants
    tree.remove(parent);
    assert_eq!(tree.len(), 1);
    assert!(!tree.contains(parent));
    assert!(!tree.contains(child1));
    assert!(!tree.contains(child2));
    assert!(!tree.contains(grandchild));
}

#[test]
fn view_zoom_and_scroll() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.Update(&mut tree); // required: sets viewed_* on root so Scroll/Zoom work

    // Zoom in so the panel is larger than the viewport; scroll won't be clamped.
    view.Zoom(&mut tree, 4.0, 400.0, 300.0);
    view.Update(&mut tree);

    // C++ rel_a = HomeW*HomeH/(vw*vh). Zoom(factor=4): vw *= 4, rel_a /= 16.
    // Starting from zoom-out rel_a (≈1.333 for 800x600 with 1x1 panel), /= 16.
    let (_, _, _, ra_before_scroll) = view
        .get_visited_panel_idiom(&tree)
        .expect("visited panel should exist after zoom");
    assert!(
        ra_before_scroll < 0.5,
        "zoomed in: rel_a should be < zoom-out value"
    );

    // Scroll(dx, dy): at zoomed-in state, pvw ≈ 3200, so delta_rx = 10/3200 ≈ tiny.
    // We just verify that scroll changes rel_x in the correct direction.
    let (_, rx_before, _, _) = view
        .get_visited_panel_idiom(&tree)
        .expect("visited panel should exist before scroll");
    view.Scroll(&mut tree, 10.0, 0.0);
    let (_, rx_after, _, _) = view
        .get_visited_panel_idiom(&tree)
        .expect("visited panel should exist after scroll");
    assert!(
        (rx_after - rx_before).abs() > 1e-10,
        "Scroll should change rel_x"
    );

    // Zoom: factor=2 reduces vw by 2 → rel_a *= 4 (more zoomed in).
    let (_, _, _, ra_before_zoom) = view
        .get_visited_panel_idiom(&tree)
        .expect("visited panel should exist before second zoom");
    view.Zoom(&mut tree, 2.0, 400.0, 300.0);
    let (_, _, _, ra_after_zoom) = view
        .get_visited_panel_idiom(&tree)
        .expect("visited panel should exist after second zoom");
    // C++ convention: Zoom(factor=2) → reFac=0.5 → ra *= reFac^2 = 0.25. rel_a /= 4.
    assert!(
        (ra_after_zoom - ra_before_zoom / 4.0).abs() < 0.01 * ra_before_zoom,
        "Zoom(2) should multiply rel_a by 1/4 (C++ convention)"
    );
}

#[test]
fn view_flags_disable_zoom() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags = ViewFlags::NO_ZOOM;

    view.Zoom(&mut tree, 2.0, 400.0, 300.0);
    // Zoom should have been blocked — NO_ZOOM returns early before setting
    // needs_animator_abort (which every non-blocked Zoom/Scroll sets).
    assert!(!view.needs_animator_abort());
}

#[test]
fn layout_rect_and_canvas_color() {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");

    tree.Layout(root, 10.0, 20.0, 300.0, 200.0, 1.0);
    tree.SetCanvasColor(root, emColor::rgb(128, 128, 128));

    assert_eq!(
        tree.layout_rect(root).unwrap(),
        Rect::new(10.0, 20.0, 300.0, 200.0)
    );
    assert_eq!(
        tree.GetCanvasColor(root).unwrap(),
        emColor::rgb(128, 128, 128)
    );
    assert!(tree
        .pending_notices(root)
        .contains(NoticeFlags::LAYOUT_CHANGED));
    assert!(tree
        .pending_notices(root)
        .contains(NoticeFlags::VIEWING_CHANGED));
}
