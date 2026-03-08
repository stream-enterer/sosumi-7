use std::collections::HashMap;

use crate::foundation::Rect;
use crate::panel::{NoticeFlags, PanelBehavior, PanelCtx, PanelId, PanelState};
use crate::render::Painter;

use super::{
    get_constraint, AlignmentH, AlignmentV, ChildConstraint, Orientation, ResolvedOrientation,
    Spacing,
};
use crate::widget::{Border, InnerBorderType, Look, OuterBorderType};

/// Linear layout: arranges children along a single axis with weighted distribution.
///
/// Implements the C++ emLinearLayout spacing model with absolute spacing units
/// and per-axis alignment (D-LAYOUT-01 through D-LAYOUT-04).
pub struct LinearLayout {
    pub orientation: Orientation,
    pub alignment_h: AlignmentH,
    pub alignment_v: AlignmentV,
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
            alignment_h: AlignmentH::default(),
            alignment_v: AlignmentV::default(),
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

    pub fn with_alignment_h(mut self, alignment: AlignmentH) -> Self {
        self.alignment_h = alignment;
        self
    }

    pub fn with_alignment_v(mut self, alignment: AlignmentV) -> Self {
        self.alignment_v = alignment;
        self
    }

    pub fn set_child_constraint(&mut self, child: PanelId, constraint: ChildConstraint) {
        self.child_constraints.insert(child, constraint);
    }

    pub(crate) fn do_layout_skip(&mut self, ctx: &mut PanelCtx, skip: Option<PanelId>) {
        self.do_layout_inner(ctx, skip);
    }

    fn do_layout(&mut self, ctx: &mut PanelCtx) {
        self.do_layout_inner(ctx, None);
    }

    /// Core layout algorithm matching C++ emLinearLayout::LayoutChildren.
    ///
    /// Uses the C++ absolute spacing model (D-LAYOUT-01):
    /// 1. Compute spacing scale factors (sx, sy) and unit sizes (ux, uy)
    /// 2. Call calculate_force with normalized dimensions (w/ux, h/uy)
    /// 3. Compute child sizes from force (these are in abstract pixel-equiv units)
    /// 4. Apply per-axis alignment post-processing (D-LAYOUT-03)
    /// 5. Convert spacing proportions to pixels and position children
    fn do_layout_inner(&mut self, ctx: &mut PanelCtx, skip: Option<PanelId>) {
        let Rect { w, h, .. } = ctx.layout_rect();
        let mut children = ctx.children();
        if let Some(skip_id) = skip {
            children.retain(|&id| id != skip_id);
        }
        if children.is_empty() {
            return;
        }

        // Clamp degenerate dimensions to 1E-100 and continue layout (C++ parity).
        let mut w = w.max(1e-100);
        let mut h = h.max(1e-100);

        let resolved = self.orientation.resolve(w, h);
        let horizontal = resolved == ResolvedOrientation::Horizontal;
        let sp = self.spacing.clamped();

        let cells = children.len().max(self.min_cell_count);

        // C++ step: cols/rows for spacing calculation
        let cols = if horizontal { cells } else { 1 };
        let rows = if horizontal { 1 } else { cells };

        // Total spacing units in each axis (SpaceL + SpaceR + SpaceH*(cols-1), etc.)
        let sx = sp.margin_left + sp.margin_right + sp.inner_h * cols.saturating_sub(1) as f64;
        let sy = sp.margin_top + sp.margin_bottom + sp.inner_v * rows.saturating_sub(1) as f64;

        // Unit size: spacing overhead per cell + 1.0 for the cell content
        let ux = sx / cols as f64 + 1.0;
        let uy = sy / rows as f64 + 1.0;

        if ux < 1e-100 || uy < 1e-100 {
            return;
        }

        // Normalized dimensions: content area extent (all cells combined)
        // w/ux = total content width in abstract units
        // h/uy = total content height in abstract units
        let nw = w / ux;
        let nh = h / uy;

        // Calculate force. CalculateForce distributes the main-axis content extent
        // among cells items based on weights.
        let force = self.calculate_force(&children, cells, nw, nh, horizontal);

        // Compute child sizes in abstract units from force
        let mut child_widths = Vec::with_capacity(children.len());
        let mut child_heights = Vec::with_capacity(children.len());
        for child in &children {
            let cc = get_constraint(&self.child_constraints, *child, &self.default_constraint);
            let (mut cw, mut ch) = if horizontal {
                (cc.weight * force, nh)
            } else {
                (nw, cc.weight * force)
            };

            // Apply tallness constraints
            if cw > 0.0 && ch > 0.0 {
                let tallness = ch / cw;
                let max_t = cc.max_tallness.max(cc.min_tallness);
                let clamped = tallness.clamp(cc.min_tallness, max_t);
                if (clamped - tallness).abs() > 1e-10 {
                    if horizontal {
                        cw = ch / clamped;
                    } else {
                        ch = cw * clamped;
                    }
                }
            }

            child_widths.push(cw);
            child_heights.push(ch);
        }

        // Compute bounding box of children + spacing in abstract units
        let total_cw: f64 = if horizontal {
            child_widths.iter().sum::<f64>()
                + sp.inner_h * children.len().saturating_sub(1) as f64
                + sp.margin_left
                + sp.margin_right
        } else {
            child_widths.iter().cloned().fold(0.0_f64, f64::max) + sp.margin_left + sp.margin_right
        };
        let total_ch: f64 = if horizontal {
            child_heights.iter().cloned().fold(0.0_f64, f64::max) + sp.margin_top + sp.margin_bottom
        } else {
            child_heights.iter().sum::<f64>()
                + sp.inner_v * children.len().saturating_sub(1) as f64
                + sp.margin_top
                + sp.margin_bottom
        };

        // Alignment step (D-LAYOUT-03): determine which axis has surplus and apply
        // per-axis alignment. C++: if (w*ch >= h*cw) -> horizontal surplus
        let mut x_offset = 0.0;
        let mut y_offset = 0.0;

        if w * total_ch >= h * total_cw {
            // Horizontal surplus: reduce w to fill aspect ratio
            let t = if total_ch > 1e-100 {
                h * total_cw / total_ch
            } else {
                w
            };
            match self.alignment_h {
                AlignmentH::Right => x_offset = w - t,
                AlignmentH::Center => x_offset = (w - t) * 0.5,
                AlignmentH::Left => {}
            }
            w = t;
        } else {
            // Vertical surplus: reduce h to fill aspect ratio
            let t = if total_cw > 1e-100 {
                w * total_ch / total_cw
            } else {
                h
            };
            match self.alignment_v {
                AlignmentV::Bottom => y_offset = h - t,
                AlignmentV::Center => y_offset = (h - t) * 0.5,
                AlignmentV::Top => {}
            }
            h = t;
        }

        // Convert spacing to pixels using (possibly adjusted) w, h.
        // C++: sx_scale = (w - w/ux) / sx, then x += sx_scale * SpaceL, gap = sx_scale * SpaceH
        let (space_x, gap_x) = if sx >= 1e-100 {
            let sx_scale = (w - w / ux) / sx;
            (sx_scale * sp.margin_left, sx_scale * sp.inner_h)
        } else {
            (0.0, 0.0)
        };

        let (space_y, gap_y) = if sy >= 1e-100 {
            let sy_scale = (h - h / uy) / sy;
            (sy_scale * sp.margin_top, sy_scale * sp.inner_v)
        } else {
            (0.0, 0.0)
        };

        // Content scale: maps abstract sizes (computed using pre-alignment nw, nh)
        // to pixels (using post-alignment w, h). The post-alignment content extent
        // in pixels is w/ux, while the abstract extent is nw (= orig_w/ux).
        let scale_x = (w / ux) / nw;
        let scale_y = (h / uy) / nh;

        // Position children
        if horizontal {
            let mut x = x_offset + space_x;
            let base_y = y_offset + space_y;
            for (i, child) in children.iter().enumerate() {
                let pixel_w = child_widths[i] * scale_x;
                let pixel_h = child_heights[i] * scale_y;
                ctx.layout_child(*child, x, base_y, pixel_w, pixel_h);
                x += pixel_w + gap_x;
            }
        } else {
            let base_x = x_offset + space_x;
            let mut y = y_offset + space_y;
            for (i, child) in children.iter().enumerate() {
                let pixel_w = child_widths[i] * scale_x;
                let pixel_h = child_heights[i] * scale_y;
                ctx.layout_child(*child, base_x, y, pixel_w, pixel_h);
                y += pixel_h + gap_y;
            }
        }
    }

    /// Iterative force solver matching C++ CalculateForce (D-LAYOUT-04).
    ///
    /// Takes the number of cells (including min_cell_count padding), the
    /// normalized container dimensions (w/ux, h/uy), and orientation.
    /// Returns force (abstract units per weight unit) that distributes the
    /// main-axis content extent among cells while respecting tallness constraints.
    ///
    /// C++ conflict resolution: when both compressed and expanded children exist,
    /// uses `compressedLength + expandedLength + freeLength < totalLength` to decide
    /// whether to release compressed (space left over) or expanded (over-committed).
    fn calculate_force(
        &self,
        children: &[PanelId],
        cell_count: usize,
        nw: f64,
        nh: f64,
        horizontal: bool,
    ) -> f64 {
        let n = children.len();
        // total_length is the main-axis content extent in abstract units
        let total_length = if horizontal { nw } else { nh };
        let cross = if horizontal { nh } else { nw };

        if n == 0 || total_length <= 0.0 {
            return 0.0;
        }

        let constraints: Vec<&ChildConstraint> = children
            .iter()
            .map(|c| get_constraint(&self.child_constraints, *c, &self.default_constraint))
            .collect();

        #[derive(Clone, Copy, PartialEq)]
        enum State {
            Free,
            Compressed(f64),
            Expanded(f64),
        }

        let mut states = vec![State::Free; n];
        // Include min_cell_count padding in free weight
        let pad_weight = if cell_count > n {
            (cell_count - n) as f64 * self.default_constraint.weight
        } else {
            0.0
        };
        let mut free_weight: f64 = constraints.iter().map(|c| c.weight).sum::<f64>() + pad_weight;
        let mut free_length = total_length;

        for _ in 0..n + 2 {
            if free_weight <= 0.0 {
                break;
            }
            let force = free_length / free_weight;

            let mut any_changed = false;
            let mut has_compressed = false;
            let mut has_expanded = false;

            for i in 0..n {
                if states[i] != State::Free {
                    continue;
                }
                let cc = constraints[i];
                let main_size = cc.weight * force;
                if main_size <= 0.0 {
                    continue;
                }

                let (cw, ch) = if horizontal {
                    (main_size, cross)
                } else {
                    (cross, main_size)
                };
                if cw <= 0.0 {
                    continue;
                }
                let tallness = ch / cw;
                let max_t = cc.max_tallness.max(cc.min_tallness);

                if tallness > max_t {
                    // Too tall -> child needs more main space -> expanded
                    let fixed = if horizontal {
                        cross / max_t
                    } else {
                        cross * max_t
                    };
                    states[i] = State::Expanded(fixed);
                    free_weight -= cc.weight;
                    free_length -= fixed;
                    any_changed = true;
                    has_expanded = true;
                } else if tallness < cc.min_tallness {
                    // Too wide -> child needs less main space -> compressed
                    let fixed = if horizontal {
                        cross / cc.min_tallness
                    } else {
                        cross * cc.min_tallness
                    };
                    states[i] = State::Compressed(fixed);
                    free_weight -= cc.weight;
                    free_length -= fixed;
                    any_changed = true;
                    has_compressed = true;
                }
            }

            if !any_changed {
                break;
            }

            // Conflict resolution (D-LAYOUT-04): C++ includes free children's
            // sizes at current force in the comparison.
            if has_compressed && has_expanded {
                let compressed_length: f64 = states
                    .iter()
                    .map(|s| match s {
                        State::Compressed(f) => *f,
                        _ => 0.0,
                    })
                    .sum();
                let expanded_length: f64 = states
                    .iter()
                    .map(|s| match s {
                        State::Expanded(f) => *f,
                        _ => 0.0,
                    })
                    .sum();

                // Free children's sizes at current force
                let current_force = if free_weight > 0.0 {
                    free_length / free_weight
                } else {
                    0.0
                };
                let free_child_length: f64 = (0..n)
                    .filter(|&i| states[i] == State::Free)
                    .map(|i| constraints[i].weight * current_force)
                    .sum::<f64>();

                // C++: if (compressedLength + expandedLength + freeLength < totalLength)
                if compressed_length + expanded_length + free_child_length < total_length {
                    // Space left over: release compressed, keep expanded
                    free_length = total_length - expanded_length;
                    for i in 0..n {
                        if let State::Compressed(_) = states[i] {
                            states[i] = State::Free;
                            free_weight += constraints[i].weight;
                        }
                    }
                } else {
                    // Over-committed: release expanded, keep compressed
                    free_length = total_length - compressed_length;
                    for i in 0..n {
                        if let State::Expanded(_) = states[i] {
                            states[i] = State::Free;
                            free_weight += constraints[i].weight;
                        }
                    }
                }
            }
        }

        if free_weight > 0.0 {
            free_length / free_weight
        } else {
            0.0
        }
    }
}

impl PanelBehavior for LinearLayout {
    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

/// LinearGroup: a LinearLayout that also paints a border and is focusable.
/// Replicates C++ emLinearGroup which inherits from emLinearLayout (which
/// inherits from emBorder).
pub struct LinearGroup {
    pub layout: LinearLayout,
    pub border: Border,
    pub look: Look,
}

impl LinearGroup {
    pub fn horizontal() -> Self {
        Self {
            layout: LinearLayout::horizontal(),
            border: Border::new(OuterBorderType::Group).with_inner(InnerBorderType::Group),
            look: Look::default(),
        }
    }

    pub fn vertical() -> Self {
        Self {
            layout: LinearLayout::vertical(),
            border: Border::new(OuterBorderType::Group).with_inner(InnerBorderType::Group),
            look: Look::default(),
        }
    }
}

impl PanelBehavior for LinearGroup {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.is_focused(), state.enabled);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        // C++ base-call: position aux panel first, then layout remaining children
        let aux_id = super::position_aux_panel(ctx, &self.border);
        self.layout.do_layout_skip(ctx, aux_id);
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
        // 4 children in 400x200, no spacing. force=100.
        // No alignment adjustment (aspect ratios match). Each child 100x200.
        let (mut tree, root, children) = setup_tree(4);
        let mut layout = LinearLayout::horizontal();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.get(*child).unwrap().layout_rect;
            assert!((r.w - 100.0).abs() < 0.01, "child {i} width: {}", r.w);
            assert!((r.h - 200.0).abs() < 0.01, "child {i} height: {}", r.h);
            assert!(
                (r.x - (i as f64 * 100.0)).abs() < 0.01,
                "child {i} x: {}",
                r.x
            );
            assert!((r.y - 0.0).abs() < 0.01, "child {i} y: {}", r.y);
        }
    }

    #[test]
    fn vertical_equal_weight() {
        // 2 children in 300x400, vertical, no spacing. force=200.
        // No alignment adjustment. Each child 300x200.
        let (mut tree, root, children) = setup_tree(2);
        tree.set_layout_rect(root, 0.0, 0.0, 300.0, 400.0);
        let mut layout = LinearLayout::vertical();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.get(*child).unwrap().layout_rect;
            assert!((r.w - 300.0).abs() < 0.01, "child {i} width: {}", r.w);
            assert!((r.h - 200.0).abs() < 0.01, "child {i} height: {}", r.h);
            assert!(
                (r.y - (i as f64 * 200.0)).abs() < 0.01,
                "child {i} y: {}",
                r.y
            );
        }
    }

    #[test]
    fn weighted_distribution() {
        // 3 children in 300x100, weights [1,2,1], no spacing.
        // total_weight=4, force=75. Widths: 75, 150, 75.
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

        let w0 = tree.get(children[0]).unwrap().layout_rect.w;
        let w1 = tree.get(children[1]).unwrap().layout_rect.w;
        let w2 = tree.get(children[2]).unwrap().layout_rect.w;
        assert!((w0 - 75.0).abs() < 0.01, "w0={w0}");
        assert!((w1 - 150.0).abs() < 0.01, "w1={w1}");
        assert!((w2 - 75.0).abs() < 0.01, "w2={w2}");
    }

    #[test]
    fn spacing() {
        // C++ spacing model: margin_left=0.5, margin_right=0.5, inner_h=1.0
        // sx=2.0, ux=2.0, nw=100, nh=100, force=50.
        // Abstract bbox: cw=102, ch=100. Horizontal surplus: w scaled to 102,
        // centered at x_offset=49. sx_scale=25.5, child pixel_w=25.5.
        let (mut tree, root, children) = setup_tree(2);
        tree.set_layout_rect(root, 0.0, 0.0, 200.0, 100.0);
        let mut layout = LinearLayout::horizontal().with_spacing(Spacing {
            inner_h: 1.0,
            inner_v: 0.0,
            margin_left: 0.5,
            margin_right: 0.5,
            margin_top: 0.0,
            margin_bottom: 0.0,
        });
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r0 = tree.get(children[0]).unwrap().layout_rect;
        let r1 = tree.get(children[1]).unwrap().layout_rect;
        assert!((r0.x - 61.75).abs() < 0.01, "r0.x: {}", r0.x);
        assert!((r0.w - 25.5).abs() < 0.01, "r0.w: {}", r0.w);
        assert!((r1.x - 112.75).abs() < 0.01, "r1.x: {}", r1.x);
        assert!((r1.w - 25.5).abs() < 0.01, "r1.w: {}", r1.w);
        assert!((r0.h - 100.0).abs() < 0.01, "r0.h: {}", r0.h);
    }

    #[test]
    fn tallness_constraints() {
        // 2 children in 600x100. Child 0 min_tallness=0.5 compresses to w=200.
        // Child 1 gets remaining 400. No alignment adjustment.
        let (mut tree, root, children) = setup_tree(2);
        tree.set_layout_rect(root, 0.0, 0.0, 600.0, 100.0);
        let mut layout = LinearLayout::horizontal();
        layout.set_child_constraint(
            children[0],
            ChildConstraint {
                weight: 1.0,
                min_tallness: 0.5,
                ..Default::default()
            },
        );
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r0 = tree.get(children[0]).unwrap().layout_rect;
        let r1 = tree.get(children[1]).unwrap().layout_rect;
        assert!((r0.w - 200.0).abs() < 0.01, "r0.w: {}", r0.w);
        assert!((r0.h - 100.0).abs() < 0.01, "r0.h: {}", r0.h);
        assert!((r1.w - 400.0).abs() < 0.01, "r1.w: {}", r1.w);
    }

    #[test]
    fn force_convergence() {
        // 3 children in 900x100. Child 0 min_tallness=0.5, child 1 max_tallness=0.2.
        // Both compressed+expanded triggers D-LAYOUT-04 conflict resolution.
        // compressed+expanded+free = 900 = total (not < total), so over-committed:
        // release expanded, keep compressed. Result: 200, 500, 200.
        let (mut tree, root, children) = setup_tree(3);
        tree.set_layout_rect(root, 0.0, 0.0, 900.0, 100.0);
        let mut layout = LinearLayout::horizontal();
        layout.set_child_constraint(
            children[0],
            ChildConstraint {
                weight: 1.0,
                min_tallness: 0.5,
                ..Default::default()
            },
        );
        layout.set_child_constraint(
            children[1],
            ChildConstraint {
                weight: 1.0,
                max_tallness: 0.2,
                ..Default::default()
            },
        );
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r0 = tree.get(children[0]).unwrap().layout_rect;
        let r1 = tree.get(children[1]).unwrap().layout_rect;
        let r2 = tree.get(children[2]).unwrap().layout_rect;
        assert!((r0.w - 200.0).abs() < 0.01, "r0.w: {}", r0.w);
        assert!((r1.w - 500.0).abs() < 0.01, "r1.w: {}", r1.w);
        assert!((r2.w - 200.0).abs() < 0.01, "r2.w: {}", r2.w);
    }
}
