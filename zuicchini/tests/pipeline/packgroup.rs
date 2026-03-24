//! Pipeline tests for emPackGroup: verifies that pack-layout children are
//! correctly positioned after auto-expansion, and that the group's
//! PanelBehavior contract (auto_expand, child count) is upheld.

use zuicchini::emCore::emPackGroup::emPackGroup;
use zuicchini::emCore::emPanel::{PanelBehavior, PanelState};
use zuicchini::emCore::emPainter::emPainter;

use super::support::pipeline::PipelineTestHarness;

/// Minimal no-op behavior used as a child placeholder.
struct DummyBehavior;

impl PanelBehavior for DummyBehavior {
    fn Paint(&mut self, _painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// After expansion, all children added to a PackGroup should be present in the
/// tree and have non-zero-area layout rects assigned by the pack layout.
#[test]
fn packgroup_layouts_children() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let pg_id = h.add_panel_with(root, "pack", Box::new(emPackGroup::new()));

    let c1 = h.add_panel_with(pg_id, "child_a", Box::new(DummyBehavior));
    let c2 = h.add_panel_with(pg_id, "child_b", Box::new(DummyBehavior));
    let c3 = h.add_panel_with(pg_id, "child_c", Box::new(DummyBehavior));

    h.expand_to(4.0);

    // All three children should still be present under the packgroup.
    let child_count = h.tree.children(pg_id).count();
    assert_eq!(child_count, 3, "expected 3 children under packgroup, got {child_count}");

    // Each child should have a layout rect with positive area.
    for (label, id) in [("child_a", c1), ("child_b", c2), ("child_c", c3)] {
        let rect = h
            .tree
            .layout_rect(id)
            .unwrap_or_else(|| panic!("no layout rect for {label}"));
        assert!(
            rect.w > 0.0 && rect.h > 0.0,
            "{label} has zero-area rect: {rect:?}",
        );
    }
}

/// emPackGroup::auto_expand() must return true (it is an auto-expanding
/// container by design).
#[test]
fn packgroup_auto_expand() {
    let pg = emPackGroup::new();
    assert!(pg.auto_expand(), "emPackGroup should auto-expand");
}

/// Children laid out by a PackGroup should not overlap each other.
#[test]
fn packgroup_children_no_overlap() {
    let mut h = PipelineTestHarness::new();
    let root = h.get_root_panel();

    let pg_id = h.add_panel_with(root, "pack", Box::new(emPackGroup::new()));

    let c1 = h.add_panel_with(pg_id, "a", Box::new(DummyBehavior));
    let c2 = h.add_panel_with(pg_id, "b", Box::new(DummyBehavior));
    let c3 = h.add_panel_with(pg_id, "c", Box::new(DummyBehavior));

    h.expand_to(4.0);

    let rects: Vec<_> = [c1, c2, c3]
        .iter()
        .filter_map(|&id| h.tree.layout_rect(id))
        .collect();

    assert_eq!(rects.len(), 3, "expected 3 layout rects");

    // Pairwise non-overlap check.
    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            assert!(
                !rects[i].intersects(&rects[j]),
                "children {i} and {j} overlap: {:?} vs {:?}",
                rects[i],
                rects[j],
            );
        }
    }
}
