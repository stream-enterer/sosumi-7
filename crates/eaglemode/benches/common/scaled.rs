use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPanel::{PanelBehavior, PanelState};

use emcore::emPanelTree::{PanelId, PanelTree};

use emcore::emPainter::emPainter;
use emcore::emView::{emView, ViewFlags};

use super::{DEFAULT_VH, DEFAULT_VW};

fn make_sched() -> emcore::test_view_harness::TestSched {
    emcore::test_view_harness::TestSched::new()
}

// ---------------------------------------------------------------------------
// Trivial panel for scaling benchmarks
// ---------------------------------------------------------------------------

pub struct ColorPanel {
    color: emColor,
}

impl PanelBehavior for ColorPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        painter.PaintRect(0.0, 0.0, w, h, self.color, emColor::TRANSPARENT);
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Tree builder
// ---------------------------------------------------------------------------

/// Build a balanced tree with `panel_count` panels (branching factor 4).
/// Returns the tree with a primed emView at DEFAULT_VW x DEFAULT_VH.
pub fn build_scaled_tree(panel_count: usize) -> (PanelTree, emView, PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("scaled_root");
    let tallness = DEFAULT_VH as f64 / DEFAULT_VW as f64;
    tree.Layout(root, 0.0, 0.0, 1.0, tallness, 1.0, None);
    tree.set_behavior(
        root,
        Box::new(ColorPanel {
            color: color_for_index(0),
        }),
    );
    tree.set_focusable(root, true);

    if panel_count > 1 {
        let mut parents = vec![root];
        let mut created = 1usize;
        let branching = 4usize;

        'outer: while created < panel_count {
            let mut next_parents = Vec::new();
            for &parent in &parents {
                for child_idx in 0..branching {
                    if created >= panel_count {
                        break 'outer;
                    }
                    let child = tree.create_child(parent, &format!("p{created}"), None);
                    let siblings = branching.min(panel_count - created + child_idx);
                    let x = child_idx as f64 / siblings as f64;
                    let w = 1.0 / siblings as f64;
                    tree.Layout(child, x, 0.0, w, 1.0, 1.0, None);
                    tree.set_behavior(
                        child,
                        Box::new(ColorPanel {
                            color: color_for_index(created),
                        }),
                    );
                    next_parents.push(child);
                    created += 1;
                }
            }
            parents = next_parents;
        }
    }

    let mut view = emView::new(
        emcore::emContext::emContext::NewRoot(),
        root,
        DEFAULT_VW as f64,
        DEFAULT_VH as f64,
    );
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    // SP5: HandleNotice is now driven from emView::Update internally.
    let mut ts = make_sched();
    ts.with(|sc| view.Update(&mut tree, sc));

    (tree, view, root)
}

/// Execute one frame with pan+zoom on a scaled tree (no tile copy).
pub fn run_one_scaled_frame(
    tree: &mut PanelTree,
    view: &mut emView,
    viewport_buf: &mut emImage,
    dx: f64,
    dy: f64,
    dz: f64,
) {
    let fix_x = DEFAULT_VW as f64 / 2.0;
    let fix_y = DEFAULT_VH as f64 / 2.0;

    let mut ts = make_sched();
    ts.with(|sc| {
        view.RawScrollAndZoom(tree, fix_x, fix_y, dx, dy, dz, sc);
        view.Update(tree, sc);
    });

    viewport_buf.fill(emColor::BLACK);
    {
        let mut painter = emPainter::new(viewport_buf);
        view.Paint(tree, &mut painter, emColor::TRANSPARENT);
    }

    view.clear_viewport_changed();
}

fn color_for_index(idx: usize) -> emColor {
    let r = ((idx * 73 + 29) % 256) as u8;
    let g = ((idx * 137 + 43) % 256) as u8;
    let b = ((idx * 53 + 97) % 256) as u8;
    emColor::rgba(r, g, b, 255)
}
