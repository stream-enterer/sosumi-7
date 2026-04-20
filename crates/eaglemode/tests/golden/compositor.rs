use emcore::emColor::emColor;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPanelTree::PanelTree;
use emcore::emView::{emView, ViewFlags};
use emcore::emViewRenderer::SoftwareCompositor;

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
    color: emColor,
}

impl ColorFillBehavior {
    fn new(color: emColor) -> Self {
        Self { color }
    }
}

impl PanelBehavior for ColorFillBehavior {
    fn Paint(&mut self, painter: &mut emPainter, vw: f64, vh: f64, _state: &PanelState) {
        painter.PaintRect(0.0, 0.0, vw, vh, self.color, emColor::TRANSPARENT);
    }
}

/// Settle: deliver notices and update viewing until stable.
fn settle(tree: &mut PanelTree, view: &mut emView) {
    let mut ts = TestSched::new();
    for _ in 0..5 {
        view.HandleNotice(tree);
        ts.with(|sc| view.Update(tree, sc));
    }
}

// ─── Test 1: composite_single_panel ────────────────────────────────
// Root panel fills entire viewport with RED.

#[test]
fn compositor_single_panel() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_single_panel");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    // Tallness must match viewport aspect ratio (600/800 = 0.75) so root fills viewport.
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    tree.set_behavior(
        root,
        Box::new(ColorFillBehavior::new(emColor::rgba(255, 0, 0, 255))),
    );

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    compare_images("composite_single_panel", actual, &expected, w, h, 0, 0.0).unwrap();
}

// ─── Test 3: composite_overlap ─────────────────────────────────────
// Two overlapping panels: A=RED, B=BLUE painted on top.

#[test]
fn compositor_overlap() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_overlap");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);

    let panel_a = tree.create_child(root, "panelA", None);
    tree.Layout(panel_a, 0.1, 0.1, 0.4, 0.3, 1.0, None);
    tree.set_behavior(
        panel_a,
        Box::new(ColorFillBehavior::new(emColor::rgba(255, 0, 0, 255))),
    );

    let panel_b = tree.create_child(root, "panelB", None);
    tree.Layout(panel_b, 0.3, 0.2, 0.4, 0.3, 1.0, None);
    tree.set_behavior(
        panel_b,
        Box::new(ColorFillBehavior::new(emColor::rgba(0, 0, 255, 255))),
    );

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    compare_images("composite_overlap", actual, &expected, w, h, 0, 0.0).unwrap();
}

// ─── Test 4: composite_nested ──────────────────────────────────────
// Parent container (no PaintContent) with a GREEN child inside.

#[test]
fn compositor_nested() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_nested");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);

    let parent = tree.create_child(root, "parent", None);
    tree.Layout(parent, 0.1, 0.075, 0.8, 0.6, 1.0, None);

    let child = tree.create_child(parent, "child", None);
    tree.Layout(child, 0.1, 0.075, 0.8, 0.6, 1.0, None);
    tree.set_behavior(
        child,
        Box::new(ColorFillBehavior::new(emColor::rgba(0, 255, 0, 255))),
    );

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    compare_images("composite_nested", actual, &expected, w, h, 0, 0.0).unwrap();
}

// ─── Test 5: composite_canvas_color ────────────────────────────────
// Root WHITE, child RED@128 alpha — tests canvas color propagation.

#[test]
fn compositor_canvas_color() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_canvas_color");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    tree.set_behavior(
        root,
        Box::new(ColorFillBehavior::new(emColor::rgba(255, 255, 255, 255))),
    );

    let child = tree.create_child(root, "child", None);
    tree.Layout(child, 0.1, 0.075, 0.8, 0.6, 1.0, None);
    tree.SetCanvasColor(child, emColor::rgba(255, 255, 255, 255), None);
    tree.set_behavior(
        child,
        Box::new(ColorFillBehavior::new(emColor::rgba(255, 0, 0, 128))),
    );

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    compare_images("composite_canvas_color", actual, &expected, w, h, 0, 0.0).unwrap();
}

// ─── Test 2: composite_two_children ────────────────────────────────
// Left half RED, right half BLUE.

#[test]
fn compositor_two_children() {
    require_golden!();
    let (w, h, expected) = load_compositor_golden("composite_two_children");

    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
    // Root has no painting behavior — children PaintContent on top of gray background.

    let left = tree.create_child(root, "left", None);
    tree.Layout(left, 0.0, 0.0, 0.5, 0.75, 1.0, None);
    tree.set_behavior(
        left,
        Box::new(ColorFillBehavior::new(emColor::rgba(255, 0, 0, 255))),
    );

    let right = tree.create_child(root, "right", None);
    tree.Layout(right, 0.5, 0.0, 0.5, 0.75, 1.0, None);
    tree.set_behavior(
        right,
        Box::new(ColorFillBehavior::new(emColor::rgba(0, 0, 255, 255))),
    );

    let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    settle(&mut tree, &mut view);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    compare_images("composite_two_children", actual, &expected, w, h, 0, 0.0).unwrap();
}
