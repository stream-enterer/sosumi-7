use std::collections::HashMap;

use crate::emPanel::Rect;
use crate::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use crate::emPanelCtx::PanelCtx;
use crate::emPanelTree::PanelId;

use super::emTiling::{get_constraint, ChildConstraint, Spacing};

/// Pack layout: recursive binary space partition that minimizes deviation from
/// preferred tallness. Port of C++ emPackLayout.
pub struct emPackLayout {
    pub spacing: Spacing,
    pub child_constraints: HashMap<PanelId, ChildConstraint>,
    pub default_constraint: ChildConstraint,
    /// Minimum number of cells (pads with empty space if fewer children).
    pub min_cell_count: usize,
}

impl emPackLayout {
    pub fn new() -> Self {
        Self {
            spacing: Spacing::default(),
            child_constraints: HashMap::new(),
            default_constraint: ChildConstraint::default(),
            min_cell_count: 0,
        }
    }

    pub fn with_spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn with_min_cell_count(mut self, count: usize) -> Self {
        self.min_cell_count = count;
        self
    }

    pub fn set_child_constraint(&mut self, child: PanelId, constraint: ChildConstraint) {
        self.child_constraints.insert(child, constraint);
    }

    /// Set the default weight for all children and clear per-child overrides (D-LAYOUT-11).
    pub fn set_default_weight(&mut self, weight: f64) {
        self.default_constraint.weight = weight;
        self.child_constraints.clear();
    }

    /// Set the default preferred tallness for all children and clear per-child overrides
    /// (D-LAYOUT-11).
    pub fn set_default_preferred_tallness(&mut self, pct: f64) {
        self.default_constraint.preferred_tallness = pct;
        self.child_constraints.clear();
    }

    pub(crate) fn do_layout_skip(
        &mut self,
        ctx: &mut PanelCtx,
        skip: Option<PanelId>,
        content_rect: Option<Rect>,
    ) {
        self.do_layout_inner(ctx, skip, content_rect);
    }

    fn do_layout(&mut self, ctx: &mut PanelCtx) {
        self.do_layout_inner(ctx, None, None);
    }

    fn do_layout_inner(
        &mut self,
        ctx: &mut PanelCtx,
        skip: Option<PanelId>,
        content_rect: Option<Rect>,
    ) {
        let cr = content_rect.unwrap_or_else(|| ctx.layout_rect());
        let Rect {
            x: origin_x,
            y: origin_y,
            w,
            h,
        } = cr;
        let mut children = ctx.children();
        if let Some(skip_id) = skip {
            children.retain(|&id| id != skip_id);
        }
        if children.is_empty() {
            return;
        }

        let sp = self.spacing.clamped();

        // Proportional spacing: convert margins from proportions to pixels.
        let denom_x = sp.margin_left + sp.margin_right + 1.0;
        let denom_y = sp.margin_top + sp.margin_bottom + 1.0;

        if denom_x < 1e-100 || denom_y < 1e-100 {
            return;
        }

        let sx = w / denom_x;
        let sy = h / denom_y;
        let actual_ml = sp.margin_left * sx;
        let actual_mt = sp.margin_top * sy;
        let content_w = sx;
        let content_h = sy;

        let rect = PackRect {
            x: origin_x + actual_ml,
            y: origin_y + actual_mt,
            w: content_w,
            h: content_h,
        };

        // Build items with weights and preferred tallness
        let mut items: Vec<PackItem> = children
            .iter()
            .map(|&id| {
                let cc = get_constraint(&self.child_constraints, id, &self.default_constraint);
                PackItem {
                    id: Some(id),
                    weight: cc.weight,
                    preferred_tallness: cc.preferred_tallness.max(1e-4),
                }
            })
            .collect();

        // Pad with empty cells for min_cell_count
        let pad_count = self.min_cell_count.saturating_sub(items.len());
        for _ in 0..pad_count {
            items.push(PackItem {
                id: None,
                weight: self.default_constraint.weight,
                preferred_tallness: self.default_constraint.preferred_tallness.max(1e-4),
            });
        }

        // Build prefix sums for weights and log(PCT)
        let count = items.len();
        let mut cum_weight = vec![0.0_f64; count + 1];
        let mut cum_log_pct = vec![0.0_f64; count + 1];
        for i in 0..count {
            cum_weight[i + 1] = cum_weight[i] + items[i].weight;
            cum_log_pct[i + 1] = cum_log_pct[i] + items[i].preferred_tallness.ln();
        }

        let packer = Packer {
            items,
            cum_weight,
            cum_log_pct,
        };

        let mut assignments = Vec::with_capacity(packer.items.len());
        packer.pack_n(0, count, rect, 1e100, true, &mut assignments);

        for (id, r) in assignments {
            if let Some(panel_id) = id {
                ctx.layout_child(panel_id, r.x, r.y, r.w, r.h);
            }
        }
    }
}

impl Default for emPackLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelBehavior for emPackLayout {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}


#[derive(Copy, Clone, Debug)]
struct PackRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Clone, Debug)]
struct PackItem {
    /// None for padding cells from min_cell_count.
    id: Option<PanelId>,
    weight: f64,
    preferred_tallness: f64,
}

/// Holds items and prefix-sum arrays, provides the recursive pack methods.
/// The prefix sums match C++ FillTPIs / GetTPIWeightSum / GetTPILogPCTSum.
struct Packer {
    items: Vec<PackItem>,
    /// cum_weight[i] = sum of weights for items 0..i
    cum_weight: Vec<f64>,
    /// cum_log_pct[i] = sum of ln(preferred_tallness) for items 0..i
    cum_log_pct: Vec<f64>,
}

impl Packer {
    /// Sum of weights for items[index..index+count]. Matches C++ GetTPIWeightSum.
    fn weight_sum(&self, index: usize, count: usize) -> f64 {
        self.cum_weight[index + count] - self.cum_weight[index]
    }

    /// Sum of log(PCT) for items[index..index+count]. Matches C++ GetTPILogPCTSum.
    fn log_pct_sum(&self, index: usize, count: usize) -> f64 {
        self.cum_log_pct[index + count] - self.cum_log_pct[index]
    }

    /// Compute weight ratio for splitting items at `div` within items[index..index+count].
    fn weight_ratio(&self, index: usize, div: usize, count: usize) -> f64 {
        let w_total = self.weight_sum(index, count);
        if w_total <= 0.0 {
            return 0.5;
        }
        self.weight_sum(index, div) / w_total
    }

    /// Core recursive pack algorithm. Matches C++ PackN.
    ///
    /// `index` and `count` define a range within `self.items`.
    /// When `execute` is true, assigns rects to `out`.
    /// When `execute` is false, only computes and returns the error (search pass).
    fn pack_n(
        &self,
        index: usize,
        count: usize,
        rect: PackRect,
        best_error: f64,
        execute: bool,
        out: &mut Vec<(Option<PanelId>, PackRect)>,
    ) -> f64 {
        match count {
            0 => 0.0,
            1 => self.pack1(index, rect, execute, out),
            2 => self.pack2(index, rect, best_error, execute, out),
            3 => self.pack3(index, rect, best_error, execute, out),
            _ => self.pack_n_general(index, count, rect, best_error, execute, out),
        }
    }

    /// Pack1: single item. Rate it and optionally assign its rect.
    fn pack1(
        &self,
        index: usize,
        rect: PackRect,
        execute: bool,
        out: &mut Vec<(Option<PanelId>, PackRect)>,
    ) -> f64 {
        let error = rate_cell(rect.w, rect.h, self.items[index].preferred_tallness);
        if execute {
            out.push((self.items[index].id, rect));
        }
        error
    }

    /// Pack2: two items. Try horizontal and vertical splits, pick the better one.
    fn pack2(
        &self,
        index: usize,
        rect: PackRect,
        best_error: f64,
        execute: bool,
        out: &mut Vec<(Option<PanelId>, PackRect)>,
    ) -> f64 {
        let ratio = self.weight_ratio(index, 1, 2);
        let (rh1, rh2) = split_rect_h(rect, ratio);
        let (rv1, rv2) = split_rect_v(rect, ratio);

        let eh = rate_cell(rh1.w, rh1.h, self.items[index].preferred_tallness)
            + rate_cell(rh2.w, rh2.h, self.items[index + 1].preferred_tallness);
        let ev = rate_cell(rv1.w, rv1.h, self.items[index].preferred_tallness)
            + rate_cell(rv2.w, rv2.h, self.items[index + 1].preferred_tallness);

        let (horizontal, error) = if eh <= ev { (true, eh) } else { (false, ev) };

        if error >= best_error {
            return error;
        }

        if execute {
            if horizontal {
                out.push((self.items[index].id, rh1));
                out.push((self.items[index + 1].id, rh2));
            } else {
                out.push((self.items[index].id, rv1));
                out.push((self.items[index + 1].id, rv2));
            }
        }

        error
    }

    /// Pack3: three items. Try all 4 split configurations (div=1,2 x horiz,vert).
    fn pack3(
        &self,
        index: usize,
        rect: PackRect,
        mut best_error: f64,
        execute: bool,
        out: &mut Vec<(Option<PanelId>, PackRect)>,
    ) -> f64 {
        let mut best_div = 1_usize;
        let mut best_horizontal = true;

        // div=1 horizontal
        let e = self.rate_horizontally(index, 3, rect, 1, best_error);
        if e < best_error {
            best_error = e;
            best_div = 1;
            best_horizontal = true;
        }

        // div=1 vertical
        let e = self.rate_vertically(index, 3, rect, 1, best_error);
        if e < best_error {
            best_error = e;
            best_div = 1;
            best_horizontal = false;
        }

        // div=2 horizontal
        let e = self.rate_horizontally(index, 3, rect, 2, best_error);
        if e < best_error {
            best_error = e;
            best_div = 2;
            best_horizontal = true;
        }

        // div=2 vertical
        let e = self.rate_vertically(index, 3, rect, 2, best_error);
        if e < best_error {
            best_error = e;
            best_div = 2;
            best_horizontal = false;
        }

        if execute {
            // D-LAYOUT-07: Epsilon relaxation for execute pass
            best_error = best_error * 1.00000001 + 1e-100;
            let ratio = self.weight_ratio(index, best_div, 3);
            let (r1, r2) = if best_horizontal {
                split_rect_h(rect, ratio)
            } else {
                split_rect_v(rect, ratio)
            };
            self.pack_n(index, best_div, r1, best_error, true, out);
            self.pack_n(index + best_div, 3 - best_div, r2, best_error, true, out);
        }

        best_error
    }

    /// General case for 4+ items. Matches C++ PackN for count >= 4.
    fn pack_n_general(
        &self,
        index: usize,
        count: usize,
        rect: PackRect,
        mut best_error: f64,
        execute: bool,
        out: &mut Vec<(Option<PanelId>, PackRect)>,
    ) -> f64 {
        // Number of split points to test (C++ formula)
        let n: usize = if count <= 7 {
            (count - 1) * 2
        } else {
            match count {
                8 => 11,
                9 => 8,
                10 => 6,
                11 => 4,
                12..=15 => 3,
                16..=20 => 2,
                _ => 1,
            }
        };

        // D-LAYOUT-06: determine orientation ordering from log(PCT) average
        let log_pct_avg = self.log_pct_sum(index, count) / count as f64;
        let log_tallness = if rect.w > 0.0 {
            (rect.h / rect.w).ln()
        } else {
            f64::INFINITY
        };
        let test_horizontal_first = log_tallness < log_pct_avg;

        let mut best_div = count / 2;
        let mut best_horizontal = test_horizontal_first;

        if n > 1 {
            // D-LAYOUT-05: Multi-point alternating pattern around midpoint
            let mut i = 0_usize;
            while i < n {
                let div = if i & 2 != 0 {
                    (count + (i >> 1) + 1) >> 1
                } else {
                    (count.wrapping_sub(i >> 1)) >> 1
                };

                // Clamp div to valid range [1, count-1]
                let div = div.clamp(1, count - 1);

                // Try both orientations, testing the preferred one first
                if test_horizontal_first {
                    let e = self.rate_horizontally(index, count, rect, div, best_error);
                    if e < best_error {
                        best_error = e;
                        best_div = div;
                        best_horizontal = true;
                    }
                    let e = self.rate_vertically(index, count, rect, div, best_error);
                    if e < best_error {
                        best_error = e;
                        best_div = div;
                        best_horizontal = false;
                    }
                } else {
                    let e = self.rate_vertically(index, count, rect, div, best_error);
                    if e < best_error {
                        best_error = e;
                        best_div = div;
                        best_horizontal = false;
                    }
                    let e = self.rate_horizontally(index, count, rect, div, best_error);
                    if e < best_error {
                        best_error = e;
                        best_div = div;
                        best_horizontal = true;
                    }
                }

                i += 2;
            }
        } else {
            // n <= 1: just use midpoint, set large initial error
            best_error = 1e100;
        }

        if execute {
            // D-LAYOUT-07: Epsilon relaxation for execute pass
            best_error = best_error * 1.00000001 + 1e-100;
            let ratio = self.weight_ratio(index, best_div, count);
            let (r1, r2) = if best_horizontal {
                split_rect_h(rect, ratio)
            } else {
                split_rect_v(rect, ratio)
            };
            self.pack_n(index, best_div, r1, best_error, true, out);
            self.pack_n(
                index + best_div,
                count - best_div,
                r2,
                best_error,
                true,
                out,
            );
        }

        best_error
    }

    /// Rate a horizontal split at `div`. Rates the smaller half first for better pruning.
    /// Matches C++ RateHorizontally.
    fn rate_horizontally(
        &self,
        index: usize,
        count: usize,
        rect: PackRect,
        div: usize,
        best_error: f64,
    ) -> f64 {
        let ratio = self.weight_ratio(index, div, count);
        let (r1, r2) = split_rect_h(rect, ratio);
        self.rate_split(index, count, r1, r2, div, best_error)
    }

    /// Rate a vertical split at `div`. Rates the smaller half first for better pruning.
    /// Matches C++ RateVertically.
    fn rate_vertically(
        &self,
        index: usize,
        count: usize,
        rect: PackRect,
        div: usize,
        best_error: f64,
    ) -> f64 {
        let ratio = self.weight_ratio(index, div, count);
        let (r1, r2) = split_rect_v(rect, ratio);
        self.rate_split(index, count, r1, r2, div, best_error)
    }

    /// Rate a split into two halves. Common logic for rate_horizontally/rate_vertically.
    /// Rates the smaller half first for better pruning.
    fn rate_split(
        &self,
        index: usize,
        count: usize,
        r1: PackRect,
        r2: PackRect,
        div: usize,
        best_error: f64,
    ) -> f64 {
        let rest = count - div;
        if div <= rest {
            let e1 = self.pack_n(index, div, r1, best_error, false, &mut Vec::new());
            if e1 < best_error {
                let e2 = self.pack_n(
                    index + div,
                    rest,
                    r2,
                    best_error - e1,
                    false,
                    &mut Vec::new(),
                );
                e1 + e2
            } else {
                e1
            }
        } else {
            let e2 = self.pack_n(index + div, rest, r2, best_error, false, &mut Vec::new());
            if e2 < best_error {
                let e1 = self.pack_n(index, div, r1, best_error - e2, false, &mut Vec::new());
                e1 + e2
            } else {
                e2
            }
        }
    }
}

/// Rate a single cell: `error = w/h * PCT; if < 1 invert; cubed - 1`. Matches C++ RateCell.
fn rate_cell(w: f64, h: f64, pct: f64) -> f64 {
    if w <= 0.0 || h <= 0.0 {
        return 1e100;
    }
    let mut error = w / h * pct;
    if error < 1.0 {
        error = 1.0 / error;
    }
    error * error * error - 1.0
}

/// Split a rect horizontally (side by side) by weight ratio.
fn split_rect_h(rect: PackRect, ratio: f64) -> (PackRect, PackRect) {
    let w1 = rect.w * ratio;
    (
        PackRect {
            x: rect.x,
            y: rect.y,
            w: w1,
            h: rect.h,
        },
        PackRect {
            x: rect.x + w1,
            y: rect.y,
            w: rect.w - w1,
            h: rect.h,
        },
    )
}

/// Split a rect vertically (stacked) by weight ratio.
fn split_rect_v(rect: PackRect, ratio: f64) -> (PackRect, PackRect) {
    let h1 = rect.h * ratio;
    (
        PackRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: h1,
        },
        PackRect {
            x: rect.x,
            y: rect.y + h1,
            w: rect.w,
            h: rect.h - h1,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;

    fn setup(n: usize, w: f64, h: f64) -> (PanelTree, PanelId, Vec<PanelId>) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, w, h);
        let mut children = Vec::new();
        for i in 0..n {
            children.push(tree.create_child(root, &format!("c{i}")));
        }
        (tree, root, children)
    }

    #[test]
    fn single_child_fills_rect() {
        let (mut tree, root, children) = setup(1, 400.0, 300.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r = tree.GetRec(children[0]).unwrap().layout_rect;
        assert!((r.x - 0.0).abs() < 0.001);
        assert!((r.y - 0.0).abs() < 0.001);
        assert!((r.w - 1.0).abs() < 0.001);
        assert!((r.h - 0.75).abs() < 0.001);
    }

    #[test]
    fn two_children_split() {
        let (mut tree, root, children) = setup(2, 400.0, 200.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // Both children should cover the full area
        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        let total_area = r0.w * r0.h + r1.w * r1.h;
        assert!((total_area - 1.0 * 0.5).abs() < 0.001);
    }

    #[test]
    fn respects_margins() {
        // Proportional: margin=0.5 means denom=0.5+0.5+1.0=2.0
        // Normalized rect is (0,0,1.0,0.75). sx=1.0/2=0.5, sy=0.75/2=0.375
        // actual_ml=0.25, actual_mt=0.1875, content_w=0.5, content_h=0.375
        let (mut tree, root, children) = setup(1, 400.0, 300.0);
        let mut layout = emPackLayout::new().with_spacing(crate::emTiling::Spacing::uniform(0.5, 0.0));
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r = tree.GetRec(children[0]).unwrap().layout_rect;
        assert!((r.x - 0.25).abs() < 0.001, "x: {}", r.x);
        assert!((r.y - 0.1875).abs() < 0.001, "y: {}", r.y);
        assert!((r.w - 0.5).abs() < 0.001, "w: {}", r.w);
        assert!((r.h - 0.375).abs() < 0.001, "h: {}", r.h);
    }

    #[test]
    fn multiple_children() {
        let (mut tree, root, children) = setup(5, 500.0, 500.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // All children should have positive dimensions
        for (i, child) in children.iter().enumerate() {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0, "child {i} has zero width");
            assert!(r.h > 0.0, "child {i} has zero height");
        }
    }

    #[test]
    fn seven_children_brute_force() {
        let (mut tree, root, children) = setup(7, 700.0, 400.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for child in &children {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0);
            assert!(r.h > 0.0);
        }
    }

    #[test]
    fn min_cell_count_pads_with_empty() {
        let (mut tree_no_pad, root_no_pad, children_no_pad) = setup(2, 400.0, 200.0);
        let mut layout_no_pad = emPackLayout::new();
        layout_no_pad.do_layout(&mut PanelCtx::new(&mut tree_no_pad, root_no_pad));

        let (mut tree_pad, root_pad, children_pad) = setup(2, 400.0, 200.0);
        let mut layout_pad = emPackLayout::new().with_min_cell_count(4);
        layout_pad.do_layout(&mut PanelCtx::new(&mut tree_pad, root_pad));

        let area_no_pad: f64 = children_no_pad
            .iter()
            .map(|c| {
                let r = tree_no_pad.GetRec(*c).unwrap().layout_rect;
                r.w * r.h
            })
            .sum();
        let area_pad: f64 = children_pad
            .iter()
            .map(|c| {
                let r = tree_pad.GetRec(*c).unwrap().layout_rect;
                r.w * r.h
            })
            .sum();
        assert!(
            area_pad < area_no_pad,
            "padded area {} should be less than unpadded {}",
            area_pad,
            area_no_pad
        );
    }

    #[test]
    fn large_count_uses_multipoint_search() {
        // 15 children: exercises the n=3 multipoint search path
        let (mut tree, root, children) = setup(15, 800.0, 600.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0, "child {i} has zero width");
            assert!(r.h > 0.0, "child {i} has zero height");
        }
    }

    #[test]
    fn very_large_count() {
        // 25 children: exercises the n=1 path (>20 items)
        let (mut tree, root, children) = setup(25, 1000.0, 800.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0, "child {i} has zero width");
            assert!(r.h > 0.0, "child {i} has zero height");
        }
    }

    #[test]
    fn set_default_weight_clears_overrides() {
        let (mut tree, root, children) = setup(3, 300.0, 300.0);
        let mut layout = emPackLayout::new();
        layout.set_child_constraint(
            children[0],
            ChildConstraint {
                weight: 5.0,
                ..Default::default()
            },
        );
        assert!(!layout.child_constraints.is_empty());
        layout.set_default_weight(2.0);
        assert!(layout.child_constraints.is_empty());
        assert!((layout.default_constraint.weight - 2.0).abs() < 1e-10);

        layout.do_layout(&mut PanelCtx::new(&mut tree, root));
        for child in &children {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0);
            assert!(r.h > 0.0);
        }
    }

    #[test]
    fn set_default_preferred_tallness_clears_overrides() {
        let (mut tree, root, children) = setup(3, 300.0, 300.0);
        let mut layout = emPackLayout::new();
        layout.set_child_constraint(
            children[0],
            ChildConstraint {
                preferred_tallness: 2.0,
                ..Default::default()
            },
        );
        assert!(!layout.child_constraints.is_empty());
        layout.set_default_preferred_tallness(0.5);
        assert!(layout.child_constraints.is_empty());
        assert!((layout.default_constraint.preferred_tallness - 0.5).abs() < 1e-10);

        layout.do_layout(&mut PanelCtx::new(&mut tree, root));
        for child in &children {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0);
            assert!(r.h > 0.0);
        }
    }

    #[test]
    fn rate_cell_basic() {
        // Perfect match: w/h * PCT = 1.0 -> error = 0.0
        let e = rate_cell(100.0, 20.0, 0.2);
        assert!(e.abs() < 1e-10, "perfect match should have ~0 error: {e}");

        // 2:1 mismatch: w/h * PCT = 2.0 -> error = 8.0 - 1.0 = 7.0
        let e = rate_cell(100.0, 10.0, 0.2);
        assert!((e - 7.0).abs() < 1e-10, "2:1 mismatch error: {e}");
    }

    #[test]
    fn total_area_preserved() {
        // Verify the pack algorithm preserves total area (no gaps)
        let (mut tree, root, children) = setup(6, 600.0, 400.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let total_area: f64 = children
            .iter()
            .map(|c| {
                let r = tree.GetRec(*c).unwrap().layout_rect;
                r.w * r.h
            })
            .sum();
        let expected = 1.0 * (400.0 / 600.0);
        assert!(
            (total_area - expected).abs() < 0.001,
            "total area {total_area} should be ~{expected}"
        );
    }

    #[test]
    fn eight_children_multipoint() {
        // 8 children: n=11 split points
        let (mut tree, root, children) = setup(8, 800.0, 500.0);
        let mut layout = emPackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0, "child {i} has zero width");
            assert!(r.h > 0.0, "child {i} has zero height");
        }
    }
}


#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_rate_cell() {
        let mut p_w: f64 = kani::any::<f64>();
        kani::assume(p_w.is_finite());
        let mut p_h: f64 = kani::any::<f64>();
        kani::assume(p_h.is_finite());
        let mut p_pct: f64 = kani::any::<f64>();
        kani::assume(p_pct.is_finite());
        let _r = rate_cell(p_w, p_h, p_pct);
        assert!(_r.is_finite());
    }
}
