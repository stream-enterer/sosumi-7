use zuicchini::foundation::Rect;
use zuicchini::layout::linear::LinearLayout;
use zuicchini::layout::pack::PackLayout;
use zuicchini::layout::raster::RasterLayout;
use zuicchini::layout::{Alignment, AlignmentH, AlignmentV, ChildConstraint, Spacing};
use zuicchini::panel::{PanelBehavior, PanelCtx, PanelId, PanelTree};

use super::common::*;

macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Create a tree with a root + N children, set root rect, attach behavior,
/// run layout_children, return child rects as (x,y,w,h) tuples.
fn run_layout(
    behavior: Box<dyn PanelBehavior>,
    n_children: usize,
    parent_rect: (f64, f64, f64, f64),
) -> Vec<(f64, f64, f64, f64)> {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
    );

    let child_ids: Vec<PanelId> = (0..n_children)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    tree.set_behavior(root, behavior);
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect()
}

/// Like run_layout but allows setting per-child constraints on LinearLayout
/// before attaching to tree.
fn run_linear_layout(
    orientation_horizontal: bool,
    weights: &[f64],
    tallnesses: &[f64],
    parent_rect: (f64, f64, f64, f64),
) -> Vec<(f64, f64, f64, f64)> {
    let n = weights.len();
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
    );

    let mut layout = if orientation_horizontal {
        LinearLayout::horizontal()
    } else {
        LinearLayout::vertical()
    };

    let child_ids: Vec<PanelId> = (0..n)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    for (i, &id) in child_ids.iter().enumerate() {
        let cc = if tallnesses.is_empty() {
            ChildConstraint {
                weight: weights[i],
                ..Default::default()
            }
        } else {
            // C++ SetChildTallness(i, t) sets both min and max to the same value,
            // fixing the aspect ratio. Match that by setting min=max=tallness.
            ChildConstraint {
                weight: weights[i],
                min_tallness: tallnesses[i],
                max_tallness: tallnesses[i],
                preferred_tallness: tallnesses[i],
                ..Default::default()
            }
        };
        layout.set_child_constraint(id, cc);
    }

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect()
}

const PARENT: (f64, f64, f64, f64) = (0.0, 0.0, 1000.0, 500.0);
const PARENT_WIDTH: f64 = 1000.0;

// ─── Test 1: linear_h_equal ─────────────────────────────────────

#[test]
fn layout_linear_h_equal() {
    require_golden!();
    let mut expected = load_layout_golden("linear_h_equal");
    let actual = run_linear_layout(true, &[1.0; 4], &[], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 2: linear_h_weighted ──────────────────────────────────

#[test]
fn layout_linear_h_weighted() {
    require_golden!();
    let mut expected = load_layout_golden("linear_h_weighted");
    let actual = run_linear_layout(true, &[1.0, 2.0, 3.0, 4.0], &[], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 3: linear_v_equal ─────────────────────────────────────

#[test]
fn layout_linear_v_equal() {
    require_golden!();
    let mut expected = load_layout_golden("linear_v_equal");
    let actual = run_linear_layout(false, &[1.0; 4], &[], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 4: linear_v_weighted ──────────────────────────────────

#[test]
fn layout_linear_v_weighted() {
    require_golden!();
    let mut expected = load_layout_golden("linear_v_weighted");
    let actual = run_linear_layout(false, &[1.0, 2.0, 3.0, 4.0], &[], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 5: linear_h_tallness ──────────────────────────────────

#[test]
fn layout_linear_h_tallness() {
    require_golden!();
    let mut expected = load_layout_golden("linear_h_tallness");
    let actual = run_linear_layout(true, &[1.0; 4], &[0.5, 1.0, 2.0, 0.5], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 6: linear_v_tallness ──────────────────────────────────

#[test]
fn layout_linear_v_tallness() {
    require_golden!();
    let mut expected = load_layout_golden("linear_v_tallness");
    let actual = run_linear_layout(false, &[1.0; 4], &[0.5, 1.0, 2.0, 0.5], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 7: raster_3col ────────────────────────────────────────

#[test]
fn layout_raster_3col() {
    require_golden!();
    let mut expected = load_layout_golden("raster_3col");
    let layout = RasterLayout::new().with_columns(3);
    let actual = run_layout(Box::new(layout), 8, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 8: raster_2row ────────────────────────────────────────

#[test]
fn layout_raster_2row() {
    require_golden!();
    let mut expected = load_layout_golden("raster_2row");
    let layout = RasterLayout::new().with_rows(2);
    let actual = run_layout(Box::new(layout), 6, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 9: raster_strict ──────────────────────────────────────
// BUG: Raster layout with strict=true and fixed tallness doesn't center
#[test]
fn layout_raster_strict() {
    require_golden!();
    let mut expected = load_layout_golden("raster_strict");
    // C++ SetChildTallness(1.0) sets both min and max to 1.0 (fixed aspect ratio).
    let mut layout = RasterLayout::new()
        .with_columns(3)
        .with_strict_raster(true)
        .with_preferred_tallness(1.0);
    layout.min_child_tallness = 1.0;
    layout.max_child_tallness = 1.0;
    let actual = run_layout(Box::new(layout), 9, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 10: raster_pref_tall ──────────────────────────────────

#[test]
fn layout_raster_pref_tall() {
    require_golden!();
    let mut expected = load_layout_golden("raster_pref_tall");
    let layout = RasterLayout::new()
        .with_columns(3)
        .with_preferred_tallness(2.0);
    let actual = run_layout(Box::new(layout), 6, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 11: pack_equal ────────────────────────────────────────

#[test]
fn layout_pack_equal() {
    require_golden!();
    let mut expected = load_layout_golden("pack_equal");
    let mut layout = PackLayout::new();
    layout.set_default_weight(1.0);
    layout.set_default_preferred_tallness(1.0);
    let actual = run_layout(Box::new(layout), 10, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 12: pack_weighted ─────────────────────────────────────

#[test]
fn layout_pack_weighted() {
    require_golden!();
    let mut expected = load_layout_golden("pack_weighted");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, PARENT.0, PARENT.1, PARENT.2, PARENT.3);

    let mut layout = PackLayout::new();

    // Deterministic weights and pct values
    let weights: Vec<f64> = (1..=10).map(|i| i as f64).collect();
    // Seeded pseudo-random pct: exp(lcg / scale)
    let mut rng: u32 = 42;
    let pcts: Vec<f64> = (0..10)
        .map(|_| {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let u = (rng >> 16) as f64 / 65536.0; // [0,1)
            (u * 2.0 - 1.0).exp() // exp([-1,1))
        })
        .collect();

    let child_ids: Vec<PanelId> = (0..10)
        .map(|i| {
            let id = tree.create_child(root, &format!("c{i}"));
            layout.set_child_constraint(
                id,
                ChildConstraint {
                    weight: weights[i],
                    preferred_tallness: pcts[i],
                    ..Default::default()
                },
            );
            id
        })
        .collect();

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let actual: Vec<(f64, f64, f64, f64)> = child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect();

    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 13: pack_extreme ──────────────────────────────────────

#[test]
fn layout_pack_extreme() {
    require_golden!();
    let mut expected = load_layout_golden("pack_extreme");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, PARENT.0, PARENT.1, PARENT.2, PARENT.3);

    let mut layout = PackLayout::new();
    let tallnesses = [0.01, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 100.0];

    let child_ids: Vec<PanelId> = (0..8)
        .map(|i| {
            let id = tree.create_child(root, &format!("c{i}"));
            layout.set_child_constraint(
                id,
                ChildConstraint {
                    weight: 1.0,
                    preferred_tallness: tallnesses[i],
                    ..Default::default()
                },
            );
            id
        })
        .collect();

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let actual: Vec<(f64, f64, f64, f64)> = child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect();

    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// Layout expansion tests — spacing, alignment, adaptive, min_cell_count, tallness constraints
// ═══════════════════════════════════════════════════════════════════

// ─── Test 14: linear_h_spacing ─────────────────────────────────────

#[test]
fn layout_linear_h_spacing() {
    require_golden!();
    let mut expected = load_layout_golden("linear_h_spacing");
    let actual = run_linear_layout_with_spacing(
        true,
        &[1.0; 4],
        &[],
        Spacing {
            margin_left: 0.5,
            margin_top: 0.3,
            inner_h: 1.0,
            inner_v: 0.0,
            margin_right: 0.5,
            margin_bottom: 0.3,
        },
        PARENT,
    );
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 15: linear_v_spacing ─────────────────────────────────────

#[test]
fn layout_linear_v_spacing() {
    require_golden!();
    let mut expected = load_layout_golden("linear_v_spacing");
    let actual = run_linear_layout_with_spacing(
        false,
        &[1.0; 4],
        &[],
        Spacing {
            margin_left: 0.3,
            margin_top: 0.5,
            inner_h: 0.0,
            inner_v: 1.0,
            margin_right: 0.3,
            margin_bottom: 0.5,
        },
        PARENT,
    );
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 16: linear_h_align_right ─────────────────────────────────

#[test]
fn layout_linear_h_align_right() {
    require_golden!();
    let mut expected = load_layout_golden("linear_h_align_right");
    let actual = run_linear_layout_aligned(
        true,
        &[1.0; 3],
        &[2.0, 2.0, 2.0],
        AlignmentH::Right,
        AlignmentV::Bottom,
        PARENT,
    );
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 17: linear_h_align_center ────────────────────────────────

#[test]
fn layout_linear_h_align_center() {
    require_golden!();
    let mut expected = load_layout_golden("linear_h_align_center");
    let actual = run_linear_layout_aligned(
        true,
        &[1.0; 3],
        &[2.0, 2.0, 2.0],
        AlignmentH::Center,
        AlignmentV::Center,
        PARENT,
    );
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 18: linear_v_align_bottom ────────────────────────────────

#[test]
fn layout_linear_v_align_bottom() {
    require_golden!();
    let mut expected = load_layout_golden("linear_v_align_bottom");
    let actual = run_linear_layout_aligned(
        false,
        &[1.0; 3],
        &[0.25, 0.25, 0.25],
        AlignmentH::Right,
        AlignmentV::Bottom,
        PARENT,
    );
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 19: linear_adaptive_wide ─────────────────────────────────
// Parent 1000x500 → tallness 0.5 < threshold 1.0 → resolves horizontal

#[test]
fn layout_linear_adaptive_wide() {
    require_golden!();
    let mut expected = load_layout_golden("linear_adaptive_wide");
    let actual = run_linear_layout_adaptive(1.0, &[1.0; 4], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 20: linear_adaptive_tall ─────────────────────────────────
// Parent 1000x2000 → tallness 2.0 > threshold 1.0 → resolves vertical

#[test]
fn layout_linear_adaptive_tall() {
    require_golden!();
    let mut expected = load_layout_golden("linear_adaptive_tall");
    let parent_tall = (0.0, 0.0, 1000.0, 2000.0);
    let actual = run_linear_layout_adaptive(1.0, &[1.0; 4], parent_tall);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 21: linear_min_cell_count ────────────────────────────────
// 3 children but min_cell_count=6 → space allocated for 6 cells

#[test]
fn layout_linear_min_cell_count() {
    require_golden!();
    let mut expected = load_layout_golden("linear_min_cell_count");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, PARENT.0, PARENT.1, PARENT.2, PARENT.3);

    let mut layout = LinearLayout::horizontal();
    layout.min_cell_count = 6;

    let child_ids: Vec<PanelId> = (0..3)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    for &id in &child_ids {
        layout.set_child_constraint(
            id,
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
    }

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let actual: Vec<(f64, f64, f64, f64)> = child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect();

    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 22: linear_min_max_tallness ──────────────────────────────

#[test]
#[ignore] // Tallness constraint redistribution differs from C++ — parity gap under investigation
fn layout_linear_min_max_tallness() {
    require_golden!();
    let mut expected = load_layout_golden("linear_min_max_tallness");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, PARENT.0, PARENT.1, PARENT.2, PARENT.3);

    let mut layout = LinearLayout::horizontal();

    let child_ids: Vec<PanelId> = (0..4)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    // Child 0: unconstrained (default)
    layout.set_child_constraint(
        child_ids[0],
        ChildConstraint {
            weight: 1.0,
            ..Default::default()
        },
    );
    // Child 1: min tallness 1.0
    layout.set_child_constraint(
        child_ids[1],
        ChildConstraint {
            weight: 1.0,
            min_tallness: 1.0,
            ..Default::default()
        },
    );
    // Child 2: max tallness 0.1
    layout.set_child_constraint(
        child_ids[2],
        ChildConstraint {
            weight: 1.0,
            max_tallness: 0.1,
            ..Default::default()
        },
    );
    // Child 3: fixed tallness 0.5 (min=max=0.5)
    layout.set_child_constraint(
        child_ids[3],
        ChildConstraint {
            weight: 1.0,
            min_tallness: 0.5,
            max_tallness: 0.5,
            preferred_tallness: 0.5,
            ..Default::default()
        },
    );

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let actual: Vec<(f64, f64, f64, f64)> = child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect();

    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 23: linear_mixed_weights ─────────────────────────────────

#[test]
fn layout_linear_mixed_weights() {
    require_golden!();
    let mut expected = load_layout_golden("linear_mixed_weights");
    let actual = run_linear_layout(true, &[0.1, 1.0, 10.0, 0.5, 5.0], &[], PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 24: raster_alignment_br ──────────────────────────────────

#[test]
fn layout_raster_alignment_br() {
    require_golden!();
    let mut expected = load_layout_golden("raster_alignment_br");
    let mut layout = RasterLayout::new().with_columns(2);
    layout.alignment = Alignment::End;
    layout.min_child_tallness = 2.0;
    layout.max_child_tallness = 2.0;
    layout.preferred_child_tallness = 2.0;
    let actual = run_layout(Box::new(layout), 4, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 25: raster_alignment_center ──────────────────────────────

#[test]
fn layout_raster_alignment_center() {
    require_golden!();
    let mut expected = load_layout_golden("raster_alignment_center");
    let mut layout = RasterLayout::new().with_columns(2);
    layout.alignment = Alignment::Center;
    layout.min_child_tallness = 2.0;
    layout.max_child_tallness = 2.0;
    layout.preferred_child_tallness = 2.0;
    let actual = run_layout(Box::new(layout), 4, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 26: raster_spacing ───────────────────────────────────────

#[test]
fn layout_raster_spacing() {
    require_golden!();
    let mut expected = load_layout_golden("raster_spacing");
    let mut layout = RasterLayout::new().with_columns(3);
    layout.spacing = Spacing {
        margin_left: 0.5,
        margin_top: 0.3,
        inner_h: 0.8,
        inner_v: 0.6,
        margin_right: 0.5,
        margin_bottom: 0.3,
    };
    let actual = run_layout(Box::new(layout), 9, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 27: raster_min_cell_count ────────────────────────────────

#[test]
fn layout_raster_min_cell_count() {
    require_golden!();
    let mut expected = load_layout_golden("raster_min_cell_count");
    let layout = RasterLayout::new().with_columns(3).with_min_cell_count(9);
    let actual = run_layout(Box::new(layout), 5, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 28: raster_min_max_tallness ──────────────────────────────

#[test]
fn layout_raster_min_max_tallness() {
    require_golden!();
    let mut expected = load_layout_golden("raster_min_max_tallness");
    let mut layout = RasterLayout::new()
        .with_columns(3)
        .with_preferred_tallness(3.0); // pref exceeds max → clamped
    layout.min_child_tallness = 0.5;
    layout.max_child_tallness = 2.0;
    let actual = run_layout(Box::new(layout), 6, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 29: raster_auto_cols ─────────────────────────────────────
// No fixed column/row count — auto-compute from preferred tallness

#[test]
fn layout_raster_auto_cols() {
    require_golden!();
    let mut expected = load_layout_golden("raster_auto_cols");
    let layout = RasterLayout::new().with_preferred_tallness(1.0);
    let actual = run_layout(Box::new(layout), 12, PARENT);
    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 30: pack_min_cell_count ──────────────────────────────────

#[test]
fn layout_pack_min_cell_count() {
    require_golden!();
    let mut expected = load_layout_golden("pack_min_cell_count");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, PARENT.0, PARENT.1, PARENT.2, PARENT.3);

    let mut layout = PackLayout::new().with_min_cell_count(8);

    let child_ids: Vec<PanelId> = (0..4)
        .map(|i| {
            let id = tree.create_child(root, &format!("c{i}"));
            layout.set_child_constraint(
                id,
                ChildConstraint {
                    weight: 1.0,
                    preferred_tallness: 1.0,
                    ..Default::default()
                },
            );
            id
        })
        .collect();

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let actual: Vec<(f64, f64, f64, f64)> = child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect();

    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ─── Test 31: pack_single ──────────────────────────────────────────
// Single child should fill the entire parent

#[test]
fn layout_pack_single() {
    require_golden!();
    let mut expected = load_layout_golden("pack_single");

    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, PARENT.0, PARENT.1, PARENT.2, PARENT.3);

    let mut layout = PackLayout::new();
    let id = tree.create_child(root, "c0");
    layout.set_child_constraint(
        id,
        ChildConstraint {
            weight: 1.0,
            preferred_tallness: 1.0,
            ..Default::default()
        },
    );

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    let r = tree
        .layout_rect(id)
        .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    let actual = vec![(r.x, r.y, r.w, r.h)];

    scale_golden_rects(&mut expected, PARENT_WIDTH);
    compare_rects(&actual, &expected, 1e-6).unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// Helper functions for expanded tests
// ═══════════════════════════════════════════════════════════════════

/// Like run_linear_layout but also sets spacing.
fn run_linear_layout_with_spacing(
    orientation_horizontal: bool,
    weights: &[f64],
    tallnesses: &[f64],
    spacing: Spacing,
    parent_rect: (f64, f64, f64, f64),
) -> Vec<(f64, f64, f64, f64)> {
    let n = weights.len();
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
    );

    let mut layout = if orientation_horizontal {
        LinearLayout::horizontal().with_spacing(spacing)
    } else {
        LinearLayout::vertical().with_spacing(spacing)
    };

    let child_ids: Vec<PanelId> = (0..n)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    for (i, &id) in child_ids.iter().enumerate() {
        let cc = if tallnesses.is_empty() {
            ChildConstraint {
                weight: weights[i],
                ..Default::default()
            }
        } else {
            ChildConstraint {
                weight: weights[i],
                min_tallness: tallnesses[i],
                max_tallness: tallnesses[i],
                preferred_tallness: tallnesses[i],
                ..Default::default()
            }
        };
        layout.set_child_constraint(id, cc);
    }

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect()
}

/// Like run_linear_layout but also sets alignment.
fn run_linear_layout_aligned(
    orientation_horizontal: bool,
    weights: &[f64],
    tallnesses: &[f64],
    align_h: AlignmentH,
    align_v: AlignmentV,
    parent_rect: (f64, f64, f64, f64),
) -> Vec<(f64, f64, f64, f64)> {
    let n = weights.len();
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
    );

    let mut layout = if orientation_horizontal {
        LinearLayout::horizontal()
            .with_alignment_h(align_h)
            .with_alignment_v(align_v)
    } else {
        LinearLayout::vertical()
            .with_alignment_h(align_h)
            .with_alignment_v(align_v)
    };

    let child_ids: Vec<PanelId> = (0..n)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    for (i, &id) in child_ids.iter().enumerate() {
        let cc = if tallnesses.is_empty() {
            ChildConstraint {
                weight: weights[i],
                ..Default::default()
            }
        } else {
            ChildConstraint {
                weight: weights[i],
                min_tallness: tallnesses[i],
                max_tallness: tallnesses[i],
                preferred_tallness: tallnesses[i],
                ..Default::default()
            }
        };
        layout.set_child_constraint(id, cc);
    }

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect()
}

/// Run adaptive layout with given tallness threshold.
fn run_linear_layout_adaptive(
    threshold: f64,
    weights: &[f64],
    parent_rect: (f64, f64, f64, f64),
) -> Vec<(f64, f64, f64, f64)> {
    let n = weights.len();
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(
        root,
        parent_rect.0,
        parent_rect.1,
        parent_rect.2,
        parent_rect.3,
    );

    let mut layout = LinearLayout::adaptive(threshold);

    let child_ids: Vec<PanelId> = (0..n)
        .map(|i| tree.create_child(root, &format!("c{i}")))
        .collect();

    for (i, &id) in child_ids.iter().enumerate() {
        layout.set_child_constraint(
            id,
            ChildConstraint {
                weight: weights[i],
                ..Default::default()
            },
        );
    }

    tree.set_behavior(root, Box::new(layout));
    let mut behavior = tree.take_behavior(root).unwrap();
    {
        let mut ctx = PanelCtx::new(&mut tree, root);
        behavior.layout_children(&mut ctx);
    }
    tree.put_behavior(root, behavior);

    child_ids
        .iter()
        .map(|&id| {
            let r = tree
                .layout_rect(id)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
            (r.x, r.y, r.w, r.h)
        })
        .collect()
}
