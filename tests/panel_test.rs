use zuicchini::foundation::Color;
use zuicchini::panel::{NoticeFlags, PanelBehavior, PanelCtx, PanelId, PanelTree, View, ViewFlags};
use zuicchini::render::Painter;

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
    fn paint(&mut self, _painter: &mut Painter, _w: f64, _h: f64) {
        self.paint_count += 1;
    }

    fn notice(&mut self, flags: NoticeFlags) {
        self.last_notice = flags;
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

#[test]
fn create_and_remove_panels() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    assert!(tree.contains(root));
    assert_eq!(tree.len(), 1);

    let child = tree.create_child(root, "child");
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.parent(child), Some(root));

    tree.remove(child);
    assert!(!tree.contains(child));
    assert_eq!(tree.len(), 1);
}

#[test]
fn child_iteration() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
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
    let root = tree.create_root("root");
    let child = tree.create_child(root, "my_panel");

    assert_eq!(tree.find_by_name("my_panel"), Some(child));
    assert_eq!(tree.find_by_name("nonexistent"), None);

    tree.remove(child);
    assert_eq!(tree.find_by_name("my_panel"), None);
}

#[test]
fn panel_ctx_operations() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");

    // Use PanelCtx to create a child
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        let child = ctx.create_child("child_via_ctx");
        ctx.layout_child(child, 10.0, 20.0, 100.0, 50.0);
        assert_eq!(ctx.name(), "root");
        assert_eq!(ctx.children().len(), 1);
    }

    let child_id = tree.find_by_name("child_via_ctx").unwrap();
    let layout = tree.get(child_id).unwrap().layout_rect;
    assert_eq!(layout, (10.0, 20.0, 100.0, 50.0));
}

#[test]
fn notice_flag_propagation() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_behavior(root, Box::new(TestBehavior::new()));

    // Creating a child should set CHILDREN_CHANGED on parent
    let _child = tree.create_child(root, "child");

    // Verify notice is pending before delivery
    let panel = tree.get(root).unwrap();
    assert!(panel
        .pending_notices
        .contains(NoticeFlags::CHILDREN_CHANGED));

    // Deliver notices
    tree.deliver_notices();

    // Verify notices were cleared after delivery
    let panel = tree.get(root).unwrap();
    assert!(panel.pending_notices.is_empty());
}

#[test]
fn remove_subtree() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    let parent = tree.create_child(root, "parent");
    let child1 = tree.create_child(parent, "child1");
    let child2 = tree.create_child(parent, "child2");
    let grandchild = tree.create_child(child1, "grandchild");
    assert_eq!(tree.len(), 5);

    // Remove parent and all descendants
    tree.remove(parent);
    assert_eq!(tree.len(), 1);
    assert!(!tree.contains(parent));
    assert!(!tree.contains(child1));
    assert!(!tree.contains(child2));
    assert!(!tree.contains(grandchild));
}

#[test]
fn view_visit_and_navigation() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    let child = tree.create_child(root, "child");
    tree.set_layout_rect(child, 0.0, 0.0, 100.0, 100.0);

    let mut view = View::new(root, 800.0, 600.0);
    assert_eq!(view.root(), root);
    assert_eq!(view.current_visit().panel, root);

    // Visit a child
    view.visit(child, 10.0, 20.0, 0.5);
    assert_eq!(view.current_visit().panel, child);
    assert_eq!(view.visit_stack().len(), 2);

    // Go back
    assert!(view.go_back());
    assert_eq!(view.current_visit().panel, root);
    assert_eq!(view.visit_stack().len(), 1);

    // Can't go back past root
    assert!(!view.go_back());
}

#[test]
fn view_zoom_and_scroll() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");

    let mut view = View::new(root, 800.0, 600.0);

    view.scroll(10.0, 20.0);
    assert!((view.current_visit().rel_x - 10.0).abs() < 0.001);
    assert!((view.current_visit().rel_y - 20.0).abs() < 0.001);

    view.zoom(2.0, 400.0, 300.0);
    assert!((view.current_visit().rel_a - 2.0).abs() < 0.001);
}

#[test]
fn view_flags_disable_zoom() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");

    let mut view = View::new(root, 800.0, 600.0);
    view.flags = ViewFlags::NO_ZOOM;

    view.zoom(2.0, 400.0, 300.0);
    // Zoom should have been blocked
    assert!((view.current_visit().rel_a - 1.0).abs() < 0.001);
}

#[test]
fn layout_rect_and_canvas_color() {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");

    tree.set_layout_rect(root, 10.0, 20.0, 300.0, 200.0);
    tree.set_canvas_color(root, Color::rgb(128, 128, 128));

    let panel = tree.get(root).unwrap();
    assert_eq!(panel.layout_rect, (10.0, 20.0, 300.0, 200.0));
    assert_eq!(panel.canvas_color, Color::rgb(128, 128, 128));
    assert!(panel.pending_notices.contains(NoticeFlags::LAYOUT_CHANGED));
    assert!(panel.pending_notices.contains(NoticeFlags::CANVAS_CHANGED));
}
