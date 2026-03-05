use std::collections::HashMap;

use crate::panel::{NoticeFlags, PanelBehavior, PanelCtx, PanelId};
use crate::render::Painter;

use super::{
    get_constraint, Alignment, ChildConstraint, Orientation, ResolvedOrientation, Spacing,
};

/// Linear layout: arranges children along a single axis with weighted distribution.
pub struct LinearLayout {
    pub orientation: Orientation,
    pub alignment: Alignment,
    pub spacing: Spacing,
    pub child_constraints: HashMap<PanelId, ChildConstraint>,
    pub default_constraint: ChildConstraint,
    /// Minimum number of cells (pads with empty space if fewer children).
    pub min_cell_count: usize,
}

impl LinearLayout {
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            alignment: Alignment::default(),
            spacing: Spacing::default(),
            child_constraints: HashMap::new(),
            default_constraint: ChildConstraint::default(),
            min_cell_count: 0,
        }
    }

    pub fn vertical() -> Self {
        Self {
            orientation: Orientation::Vertical,
            ..Self::horizontal()
        }
    }

    pub fn adaptive(tallness_threshold: f64) -> Self {
        Self {
            orientation: Orientation::Adaptive { tallness_threshold },
            ..Self::horizontal()
        }
    }

    pub fn with_spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn set_child_constraint(&mut self, child: PanelId, constraint: ChildConstraint) {
        self.child_constraints.insert(child, constraint);
    }

    fn do_layout(&mut self, ctx: &mut PanelCtx) {
        let (_, _, w, h) = ctx.layout_rect();
        let children = ctx.children();
        if children.is_empty() {
            return;
        }

        let resolved = self.orientation.resolve(w, h);
        let sp = &self.spacing;

        let (
            main_total,
            cross_total,
            margin_main_start,
            margin_main_end,
            margin_cross_start,
            margin_cross_end,
        ) = match resolved {
            ResolvedOrientation::Horizontal => (
                w,
                h,
                sp.margin_left,
                sp.margin_right,
                sp.margin_top,
                sp.margin_bottom,
            ),
            ResolvedOrientation::Vertical => (
                h,
                w,
                sp.margin_top,
                sp.margin_bottom,
                sp.margin_left,
                sp.margin_right,
            ),
        };

        let cell_count = children.len().max(self.min_cell_count);
        let gap_count = cell_count.saturating_sub(1);
        let available_main =
            (main_total - margin_main_start - margin_main_end - sp.inner * gap_count as f64)
                .max(0.0);
        let cross_available = (cross_total - margin_cross_start - margin_cross_end).max(0.0);

        // Compute weighted distribution
        let total_weight: f64 = children
            .iter()
            .map(|c| get_constraint(&self.child_constraints, *c, &self.default_constraint).weight)
            .sum();

        let total_weight = if total_weight <= 0.0 {
            1.0
        } else {
            total_weight
        };

        // First pass: compute initial sizes, clamp, track surplus
        let mut sizes: Vec<f64> = Vec::with_capacity(children.len());
        let mut surplus = 0.0;
        let mut unfixed_weight = 0.0;

        for child in &children {
            let cc = get_constraint(&self.child_constraints, *child, &self.default_constraint);
            let raw = (cc.weight / total_weight) * available_main;
            let clamped = raw.clamp(cc.min_main, cc.max_main);
            if (clamped - raw).abs() > 0.001 {
                surplus += raw - clamped;
            } else {
                unfixed_weight += cc.weight;
            }
            sizes.push(clamped);
        }

        // Redistribute surplus among unclamped children
        if surplus.abs() > 0.001 && unfixed_weight > 0.0 {
            for (i, child) in children.iter().enumerate() {
                let cc = get_constraint(&self.child_constraints, *child, &self.default_constraint);
                let raw = (cc.weight / total_weight) * available_main;
                let clamped = raw.clamp(cc.min_main, cc.max_main);
                if (clamped - raw).abs() < 0.001 {
                    let extra = surplus * (cc.weight / unfixed_weight);
                    sizes[i] = (sizes[i] + extra).clamp(cc.min_main, cc.max_main);
                }
            }
        }

        // Position children
        let mut main_pos = margin_main_start;
        for (i, child) in children.iter().enumerate() {
            let main_size = sizes[i];
            let (cross_pos, cross_size) = match self.alignment {
                Alignment::Start => (margin_cross_start, cross_available.min(cross_available)),
                Alignment::Center => {
                    let pref = self.child_cross_size(ctx, *child, resolved);
                    let s = pref.min(cross_available);
                    (margin_cross_start + (cross_available - s) / 2.0, s)
                }
                Alignment::End => {
                    let pref = self.child_cross_size(ctx, *child, resolved);
                    let s = pref.min(cross_available);
                    (margin_cross_start + cross_available - s, s)
                }
                Alignment::Stretch => (margin_cross_start, cross_available),
            };

            let (x, y, cw, ch) = match resolved {
                ResolvedOrientation::Horizontal => (main_pos, cross_pos, main_size, cross_size),
                ResolvedOrientation::Vertical => (cross_pos, main_pos, cross_size, main_size),
            };
            ctx.layout_child(*child, x, y, cw, ch);
            main_pos += main_size + sp.inner;
        }
    }

    fn child_cross_size(
        &self,
        ctx: &mut PanelCtx,
        child: PanelId,
        resolved: ResolvedOrientation,
    ) -> f64 {
        let (pw, ph) = ctx.child_preferred_size(child);
        match resolved {
            ResolvedOrientation::Horizontal => ph,
            ResolvedOrientation::Vertical => pw,
        }
    }
}

impl PanelBehavior for LinearLayout {
    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags) {}
}

/// LinearGroup: a LinearLayout that also paints a border and is focusable.
pub struct LinearGroup {
    pub layout: LinearLayout,
}

impl LinearGroup {
    pub fn horizontal() -> Self {
        Self {
            layout: LinearLayout::horizontal(),
        }
    }

    pub fn vertical() -> Self {
        Self {
            layout: LinearLayout::vertical(),
        }
    }
}

impl PanelBehavior for LinearGroup {
    fn paint(&mut self, _painter: &mut Painter, _w: f64, _h: f64) {}

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.layout.do_layout(ctx);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelTree;

    fn setup_tree(n: usize) -> (PanelTree, PanelId, Vec<PanelId>) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_layout_rect(root, 0.0, 0.0, 400.0, 200.0);
        let mut children = Vec::new();
        for i in 0..n {
            let c = tree.create_child(root, &format!("child_{i}"));
            children.push(c);
        }
        (tree, root, children)
    }

    #[test]
    fn horizontal_equal_weight() {
        let (mut tree, root, children) = setup_tree(4);
        let mut layout = LinearLayout::horizontal();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // Each child should get 100px wide, 200px tall
        for (i, child) in children.iter().enumerate() {
            let r = tree.get(*child).unwrap().layout_rect;
            assert!((r.2 - 100.0).abs() < 0.01, "child {i} width: {}", r.2);
            assert!((r.3 - 200.0).abs() < 0.01, "child {i} height: {}", r.3);
            assert!(
                (r.0 - (i as f64 * 100.0)).abs() < 0.01,
                "child {i} x: {}",
                r.0
            );
            assert!((r.1 - 0.0).abs() < 0.01, "child {i} y: {}", r.1);
        }
    }

    #[test]
    fn vertical_equal_weight() {
        let (mut tree, root, children) = setup_tree(2);
        tree.set_layout_rect(root, 0.0, 0.0, 300.0, 400.0);
        let mut layout = LinearLayout::vertical();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.get(*child).unwrap().layout_rect;
            assert!((r.2 - 300.0).abs() < 0.01, "child {i} width: {}", r.2);
            assert!((r.3 - 200.0).abs() < 0.01, "child {i} height: {}", r.3);
            assert!(
                (r.1 - (i as f64 * 200.0)).abs() < 0.01,
                "child {i} y: {}",
                r.1
            );
        }
    }

    #[test]
    fn weighted_distribution() {
        let (mut tree, root, children) = setup_tree(3);
        tree.set_layout_rect(root, 0.0, 0.0, 300.0, 100.0);
        let mut layout = LinearLayout::horizontal();
        layout.set_child_constraint(
            children[0],
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
        layout.set_child_constraint(
            children[1],
            ChildConstraint {
                weight: 2.0,
                ..Default::default()
            },
        );
        layout.set_child_constraint(
            children[2],
            ChildConstraint {
                weight: 1.0,
                ..Default::default()
            },
        );
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let w0 = tree.get(children[0]).unwrap().layout_rect.2;
        let w1 = tree.get(children[1]).unwrap().layout_rect.2;
        let w2 = tree.get(children[2]).unwrap().layout_rect.2;
        assert!((w0 - 75.0).abs() < 0.01);
        assert!((w1 - 150.0).abs() < 0.01);
        assert!((w2 - 75.0).abs() < 0.01);
    }

    #[test]
    fn spacing() {
        let (mut tree, root, children) = setup_tree(2);
        tree.set_layout_rect(root, 0.0, 0.0, 200.0, 100.0);
        let mut layout = LinearLayout::horizontal().with_spacing(Spacing {
            inner: 10.0,
            margin_left: 5.0,
            margin_right: 5.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
        });
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r0 = tree.get(children[0]).unwrap().layout_rect;
        let r1 = tree.get(children[1]).unwrap().layout_rect;
        // Available = 200 - 5 - 5 - 10 = 180, each child = 90
        assert!((r0.0 - 5.0).abs() < 0.01);
        assert!((r0.2 - 90.0).abs() < 0.01);
        assert!((r1.0 - 105.0).abs() < 0.01); // 5 + 90 + 10
        assert!((r1.2 - 90.0).abs() < 0.01);
    }
}
