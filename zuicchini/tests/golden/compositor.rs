use zuicchini::foundation::Color;
use zuicchini::panel::{PanelBehavior, PanelState, PanelTree, View, ViewFlags};
use zuicchini::render::{Painter, SoftwareCompositor};

use super::common::*;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Behavior that fills its panel with a solid color.
struct ColorFillBehavior {
    color: Color,
}

impl ColorFillBehavior {
    fn new(color: Color) -> Self {
        Self { color }
    }
}

impl PanelBehavior for ColorFillBehavior {
    fn paint(&mut self, painter: &mut Painter, vw: f64, vh: f64, _state: &PanelState) {
        painter.paint_rect(0.0, 0.0, vw, vh, self.color, Color::TRANSPARENT);
    }
}

/// Settle: deliver notices and update viewing until stable.
fn settle(tree: &mut PanelTree, view: &mut View) {
    for _ in 0..5 {
        tree.deliver_notices(view.window_focused(), view.pixel_tallness());
        view.update_viewing(tree);
    }
}

// ─── Test 1: composite_single_panel ────────────────────────────────
// Root panel fills entire viewport with RED.

#[test]
fn compositor_single_panel() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_single_panel");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    // Tallness must match viewport aspect ratio (600/800 = 0.75) so root fills viewport.
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(
        root,
        Box::new(ColorFillBehavior::new(Color::rgba(255, 0, 0, 255))),
    );

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    compare_images("composite_single_panel", actual, &expected, w, h, 1, 0.5).unwrap();
}

// ─── Test 3: composite_overlap ─────────────────────────────────────
// Two overlapping panels: A=RED, B=BLUE painted on top.

#[test]
fn compositor_overlap() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_overlap");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);

    let panel_a = tree.create_child(root, "panelA");
    tree.set_layout_rect(panel_a, 0.1, 0.1, 0.4, 0.3);
    tree.set_behavior(
        panel_a,
        Box::new(ColorFillBehavior::new(Color::rgba(255, 0, 0, 255))),
    );

    let panel_b = tree.create_child(root, "panelB");
    tree.set_layout_rect(panel_b, 0.3, 0.2, 0.4, 0.3);
    tree.set_behavior(
        panel_b,
        Box::new(ColorFillBehavior::new(Color::rgba(0, 0, 255, 255))),
    );

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    compare_images("composite_overlap", actual, &expected, w, h, 1, 0.5).unwrap();
}

// ─── Test 4: composite_nested ──────────────────────────────────────
// Parent container (no paint) with a GREEN child inside.

#[test]
fn compositor_nested() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_nested");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);

    let parent = tree.create_child(root, "parent");
    tree.set_layout_rect(parent, 0.1, 0.075, 0.8, 0.6);

    let child = tree.create_child(parent, "child");
    tree.set_layout_rect(child, 0.1, 0.075, 0.8, 0.6);
    tree.set_behavior(
        child,
        Box::new(ColorFillBehavior::new(Color::rgba(0, 255, 0, 255))),
    );

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    compare_images("composite_nested", actual, &expected, w, h, 1, 0.5).unwrap();
}

// ─── Test 5: composite_canvas_color ────────────────────────────────
// Root WHITE, child RED@128 alpha — tests canvas color propagation.

#[test]
fn compositor_canvas_color() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_canvas_color");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_behavior(
        root,
        Box::new(ColorFillBehavior::new(Color::rgba(255, 255, 255, 255))),
    );

    let child = tree.create_child(root, "child");
    tree.set_layout_rect(child, 0.1, 0.075, 0.8, 0.6);
    tree.set_canvas_color(child, Color::rgba(255, 255, 255, 255));
    tree.set_behavior(
        child,
        Box::new(ColorFillBehavior::new(Color::rgba(255, 0, 0, 128))),
    );

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    compare_images("composite_canvas_color", actual, &expected, w, h, 1, 0.5).unwrap();
}

// ─── Test 2: composite_two_children ────────────────────────────────
// Left half RED, right half BLUE.

#[test]
fn compositor_two_children() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_two_children");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    // Root has no painting behavior — children paint on top of gray background.

    let left = tree.create_child(root, "left");
    tree.set_layout_rect(left, 0.0, 0.0, 0.5, 0.75);
    tree.set_behavior(
        left,
        Box::new(ColorFillBehavior::new(Color::rgba(255, 0, 0, 255))),
    );

    let right = tree.create_child(root, "right");
    tree.set_layout_rect(right, 0.5, 0.0, 0.5, 0.75);
    tree.set_behavior(
        right,
        Box::new(ColorFillBehavior::new(Color::rgba(0, 0, 255, 255))),
    );

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().data();

    compare_images("composite_two_children", actual, &expected, w, h, 1, 0.5).unwrap();
}
