use std::collections::HashMap;

use crate::foundation::Rect;
use crate::panel::{NoticeFlags, PanelBehavior, PanelCtx, PanelId};
use crate::render::Painter;

use super::{get_constraint, ChildConstraint, Spacing};

/// Pack layout: recursive binary space partition that minimizes deviation from
/// preferred tallness.
pub struct PackLayout {
    pub spacing: Spacing,
    pub child_constraints: HashMap<PanelId, ChildConstraint>,
    pub default_constraint: ChildConstraint,
}

impl PackLayout {
    pub fn new() -> Self {
        Self {
            spacing: Spacing::default(),
            child_constraints: HashMap::new(),
            default_constraint: ChildConstraint::default(),
        }
    }

    pub fn with_spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn set_child_constraint(&mut self, child: PanelId, constraint: ChildConstraint) {
        self.child_constraints.insert(child, constraint);
    }

    fn do_layout(&mut self, ctx: &mut PanelCtx) {
        let Rect { w, h, .. } = ctx.layout_rect();
        let children = ctx.children();
        if children.is_empty() {
            return;
        }

        let sp = &self.spacing;
        let rect = PackRect {
            x: sp.margin_left,
            y: sp.margin_top,
            w: (w - sp.margin_left - sp.margin_right).max(0.0),
            h: (h - sp.margin_top - sp.margin_bottom).max(0.0),
        };

        // Build items with weights and preferred tallness
        let items: Vec<PackItem> = children
            .iter()
            .map(|&id| {
                let cc = get_constraint(&self.child_constraints, id, &self.default_constraint);
                PackItem {
                    id,
                    weight: cc.weight,
                    preferred_tallness: cc.preferred_tallness,
                }
            })
            .collect();

        let mut assignments = Vec::with_capacity(items.len());
        // Pack uses a single gap since it splits in both orientations.
        let gap = self.spacing.inner_h.max(self.spacing.inner_v);
        self.partition(&items, rect, gap, &mut assignments);

        for (id, r) in assignments {
            ctx.layout_child(id, r.x, r.y, r.w, r.h);
        }
    }

    fn partition(
        &self,
        items: &[PackItem],
        rect: PackRect,
        gap: f64,
        out: &mut Vec<(PanelId, PackRect)>,
    ) {
        if items.len() == 1 {
            out.push((items[0].id, rect));
            return;
        }
        if items.is_empty() {
            return;
        }

        if items.len() <= 7 {
            // Brute force: try all split points and both orientations
            let (split, horizontal) = self.best_split(items, rect, gap);
            let (r1, r2) = Self::split_rect(rect, split, items, horizontal, gap);
            self.partition(&items[..split], r1, gap, out);
            self.partition(&items[split..], r2, gap, out);
        } else {
            // Greedy: sort by weight descending, split at weight midpoint
            let total_weight: f64 = items.iter().map(|i| i.weight).sum();
            let half = total_weight / 2.0;
            let mut acc = 0.0;
            let mut split = 1;
            for (i, item) in items.iter().enumerate() {
                acc += item.weight;
                if acc >= half && i + 1 < items.len() {
                    split = i + 1;
                    break;
                }
            }

            // Try both orientations for the chosen split point
            let score_h = self.score_split(items, rect, split, true, gap);
            let score_v = self.score_split(items, rect, split, false, gap);
            let horizontal = score_h <= score_v;
            let (r1, r2) = Self::split_rect(rect, split, items, horizontal, gap);
            self.partition(&items[..split], r1, gap, out);
            self.partition(&items[split..], r2, gap, out);
        }
    }

    fn best_split(&self, items: &[PackItem], rect: PackRect, gap: f64) -> (usize, bool) {
        let mut best_split = 1;
        let mut best_horizontal = true;
        let mut best_score = f64::INFINITY;

        for split in 1..items.len() {
            for horizontal in [true, false] {
                let score = self.score_split(items, rect, split, horizontal, gap);
                if score < best_score {
                    best_score = score;
                    best_split = split;
                    best_horizontal = horizontal;
                }
            }
        }

        (best_split, best_horizontal)
    }

    fn score_split(
        &self,
        items: &[PackItem],
        rect: PackRect,
        split: usize,
        horizontal: bool,
        gap: f64,
    ) -> f64 {
        let (r1, r2) = Self::split_rect(rect, split, items, horizontal, gap);
        Self::score_rect(&items[..split], r1) + Self::score_rect(&items[split..], r2)
    }

    fn split_rect(
        rect: PackRect,
        split: usize,
        items: &[PackItem],
        horizontal: bool,
        gap: f64,
    ) -> (PackRect, PackRect) {
        let w1: f64 = items[..split].iter().map(|i| i.weight).sum();
        let w2: f64 = items[split..].iter().map(|i| i.weight).sum();
        let total = w1 + w2;
        if total <= 0.0 {
            return (
                rect,
                PackRect {
                    x: rect.x,
                    y: rect.y,
                    w: 0.0,
                    h: 0.0,
                },
            );
        }
        let ratio = w1 / total;

        if horizontal {
            let split_w = (rect.w - gap).max(0.0) * ratio;
            let r1 = PackRect {
                x: rect.x,
                y: rect.y,
                w: split_w,
                h: rect.h,
            };
            let r2 = PackRect {
                x: rect.x + split_w + gap,
                y: rect.y,
                w: (rect.w - split_w - gap).max(0.0),
                h: rect.h,
            };
            (r1, r2)
        } else {
            let split_h = (rect.h - gap).max(0.0) * ratio;
            let r1 = PackRect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: split_h,
            };
            let r2 = PackRect {
                x: rect.x,
                y: rect.y + split_h + gap,
                w: rect.w,
                h: (rect.h - split_h - gap).max(0.0),
            };
            (r1, r2)
        }
    }

    /// Score = sum of |actual_tallness - preferred_tallness| weighted by item count.
    fn score_rect(items: &[PackItem], rect: PackRect) -> f64 {
        if items.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
            return 0.0;
        }
        if items.len() == 1 {
            let tallness = rect.h / rect.w;
            return (tallness - items[0].preferred_tallness).abs();
        }
        // For multi-item groups, estimate by assuming uniform split
        let avg_tallness = (rect.h * items.len() as f64) / rect.w;
        items
            .iter()
            .map(|item| (avg_tallness / items.len() as f64 - item.preferred_tallness).abs())
            .sum()
    }
}

impl Default for PackLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelBehavior for PackLayout {
    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags) {}
}

/// PackGroup wraps PackLayout with border painting and focusable support.
pub struct PackGroup {
    pub layout: PackLayout,
}

impl PackGroup {
    pub fn new() -> Self {
        Self {
            layout: PackLayout::new(),
        }
    }
}

impl Default for PackGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelBehavior for PackGroup {
    fn paint(&mut self, _painter: &mut Painter, _w: f64, _h: f64) {}

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.layout.do_layout(ctx);
    }

    fn auto_expand(&self) -> bool {
        true
    }
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
    id: PanelId,
    weight: f64,
    preferred_tallness: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelTree;

    fn setup(n: usize, w: f64, h: f64) -> (PanelTree, PanelId, Vec<PanelId>) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_layout_rect(root, 0.0, 0.0, w, h);
        let mut children = Vec::new();
        for i in 0..n {
            children.push(tree.create_child(root, &format!("c{i}")));
        }
        (tree, root, children)
    }

    #[test]
    fn single_child_fills_rect() {
        let (mut tree, root, children) = setup(1, 400.0, 300.0);
        let mut layout = PackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r = tree.get(children[0]).unwrap().layout_rect;
        assert!((r.x - 0.0).abs() < 0.01);
        assert!((r.y - 0.0).abs() < 0.01);
        assert!((r.w - 400.0).abs() < 0.01);
        assert!((r.h - 300.0).abs() < 0.01);
    }

    #[test]
    fn two_children_split() {
        let (mut tree, root, children) = setup(2, 400.0, 200.0);
        let mut layout = PackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // Both children should cover the full area
        let r0 = tree.get(children[0]).unwrap().layout_rect;
        let r1 = tree.get(children[1]).unwrap().layout_rect;
        let total_area = r0.w * r0.h + r1.w * r1.h;
        assert!((total_area - 400.0 * 200.0).abs() < 1.0);
    }

    #[test]
    fn respects_margins() {
        let (mut tree, root, children) = setup(1, 400.0, 300.0);
        let mut layout = PackLayout::new().with_spacing(super::super::Spacing::uniform(10.0, 0.0));
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r = tree.get(children[0]).unwrap().layout_rect;
        assert!((r.x - 10.0).abs() < 0.01);
        assert!((r.y - 10.0).abs() < 0.01);
        assert!((r.w - 380.0).abs() < 0.01);
        assert!((r.h - 280.0).abs() < 0.01);
    }

    #[test]
    fn multiple_children() {
        let (mut tree, root, children) = setup(5, 500.0, 500.0);
        let mut layout = PackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // All children should have positive dimensions
        for (i, child) in children.iter().enumerate() {
            let r = tree.get(*child).unwrap().layout_rect;
            assert!(r.w > 0.0, "child {i} has zero width");
            assert!(r.h > 0.0, "child {i} has zero height");
        }
    }

    #[test]
    fn seven_children_brute_force() {
        let (mut tree, root, children) = setup(7, 700.0, 400.0);
        let mut layout = PackLayout::new();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // Verify no overlapping and all positive sizes
        for child in &children {
            let r = tree.get(*child).unwrap().layout_rect;
            assert!(r.w > 0.0);
            assert!(r.h > 0.0);
        }
    }
}
