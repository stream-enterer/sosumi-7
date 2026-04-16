use std::collections::HashMap;

use crate::emPanel::Rect;
use crate::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use crate::emPanelCtx::PanelCtx;
use crate::emPanelTree::PanelId;

use super::emTiling::{
    get_constraint, AlignmentH, AlignmentV, ChildConstraint, Orientation, ResolvedOrientation,
    Spacing,
};

/// Linear layout: arranges children along a single axis with weighted distribution.
///
/// Implements the C++ emLinearLayout spacing model with absolute spacing units
/// and per-axis alignment (D-LAYOUT-01 through D-LAYOUT-04).
pub struct emLinearLayout {
    pub orientation: Orientation,
    pub alignment_h: AlignmentH,
    pub alignment_v: AlignmentV,
    pub spacing: Spacing,
    pub child_constraints: HashMap<PanelId, ChildConstraint>,
    pub default_constraint: ChildConstraint,
    /// Minimum number of cells (pads with empty space if fewer children).
    pub min_cell_count: usize,
}

impl emLinearLayout {
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

    pub fn do_layout_skip(
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

    /// Core layout algorithm matching C++ emLinearLayout::LayoutChildren.
    ///
    /// Uses the C++ absolute spacing model (D-LAYOUT-01):
    /// 1. Compute spacing scale factors (sx, sy) and unit sizes (ux, uy)
    /// 2. Call calculate_force with normalized dimensions (w/ux, h/uy)
    /// 3. Compute child sizes from force (these are in abstract pixel-equiv units)
    /// 4. Apply per-axis alignment post-processing (D-LAYOUT-03)
    /// 5. Convert spacing proportions to pixels and position children
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

        // ─── Pass 1: Compute bounding box in normalized units (C++ lines 375-412) ───
        // C++ computes child sizes in a normalized space: for horizontal, ch=1.0
        // (cross-axis normalized to 1); for vertical, cw=1.0.
        let mut length = 0.0;
        for child in &children {
            let cc = get_constraint(&self.child_constraints, *child, &self.default_constraint);
            let min_ct = cc.min_tallness;
            let max_ct = cc.max_tallness.max(min_ct);
            if horizontal {
                let mut cw = cc.weight * force;
                let ch = 1.0;
                if ch < cw * min_ct {
                    cw = ch / min_ct;
                } else if ch > cw * max_ct {
                    cw = ch / max_ct;
                }
                length += cw;
            } else {
                let cw = 1.0;
                let mut ch = cc.weight * force;
                if ch < cw * min_ct {
                    ch = cw * min_ct;
                } else if ch > cw * max_ct {
                    ch = cw * max_ct;
                }
                length += ch;
            }
        }
        // Include min_cell_count padding cells in total length (C++ parity:
        // padding cells use default_constraint.weight and no tallness clamping).
        if cells > children.len() {
            let pad_count = (cells - children.len()) as f64;
            length += pad_count * self.default_constraint.weight * force;
        }

        // C++ lines 405-412: convert bounding box to pixel dimensions
        let (total_cw, total_ch) = if horizontal {
            // cw = h/uy * ux * length; ch = h
            (h / uy * ux * length, h)
        } else {
            // cw = w; ch = w/ux * uy * length
            (w, w / ux * uy * length)
        };

        // ─── Alignment step (C++ lines 414-425) ───
        // C++ does `x += (w-t)*0.5` etc., i.e. adds the alignment delta to
        // the content-rect origin. Must preserve origin_x / origin_y.
        let mut x_offset = origin_x;
        let mut y_offset = origin_y;

        if w * total_ch >= h * total_cw {
            let t = if total_ch > 1e-100 {
                h * total_cw / total_ch
            } else {
                w
            };
            match self.alignment_h {
                AlignmentH::Right => x_offset += w - t,
                AlignmentH::Center => x_offset += (w - t) * 0.5,
                AlignmentH::Left => {}
            }
            w = t;
        } else {
            let t = if total_cw > 1e-100 {
                w * total_ch / total_cw
            } else {
                h
            };
            match self.alignment_v {
                AlignmentV::Bottom => y_offset += h - t,
                AlignmentV::Center => y_offset += (h - t) * 0.5,
                AlignmentV::Top => {}
            }
            h = t;
        }

        // ─── Spacing (C++ lines 427-439) ───
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

        // ─── Pass 2: Position children using post-alignment dimensions (C++ lines 441-479) ───
        if horizontal {
            let mut cx = x_offset + space_x;
            let base_cy = y_offset + space_y;
            for child in &children {
                let cc = get_constraint(&self.child_constraints, *child, &self.default_constraint);
                let min_ct = cc.min_tallness;
                let max_ct = cc.max_tallness.max(min_ct);
                // C++: ch = h/uy; cw = weight*force*ch; then tallness clamp
                let ch = h / uy;
                let mut cw = cc.weight * force * ch;
                if ch < cw * min_ct {
                    cw = ch / min_ct;
                } else if ch > cw * max_ct {
                    cw = ch / max_ct;
                }
                ctx.layout_child(*child, cx, base_cy, cw, ch);
                cx += cw + gap_x;
            }
        } else {
            let base_cx = x_offset + space_x;
            let mut cy = y_offset + space_y;
            for child in &children {
                let cc = get_constraint(&self.child_constraints, *child, &self.default_constraint);
                let min_ct = cc.min_tallness;
                let max_ct = cc.max_tallness.max(min_ct);
                // C++: cw = w/ux; ch = weight*force*cw; then tallness clamp
                let cw = w / ux;
                let mut ch = cc.weight * force * cw;
                if ch < cw * min_ct {
                    ch = cw * min_ct;
                } else if ch > cw * max_ct {
                    ch = cw * max_ct;
                }
                ctx.layout_child(*child, base_cx, cy, cw, ch);
                cy += ch + gap_y;
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
        // C++ uses totalLength = w/h (horizontal) or h/w (vertical), i.e.,
        // the main-axis extent relative to cross-axis=1.0 in normalized space.
        let total_length = if horizontal { nw / nh } else { nh / nw };
        let cross = 1.0; // Normalized cross-axis

        if n == 0 || total_length <= 0.0 {
            return 0.0;
        }

        let constraints: Vec<&ChildConstraint> = children
            .iter()
            .map(|c| get_constraint(&self.child_constraints, *c, &self.default_constraint))
            .collect();

        // C++ CalculateForce uses a linked-list approach: all children start in
        // the "input" list, each iteration classifies them into compressed/expanded/free,
        // then resolves conflicts and re-iterates with only the free children.
        // We match this by tracking state per-child and iterating until stable.

        // Include min_cell_count padding in free weight
        let pad_weight = if cell_count > n {
            (cell_count - n) as f64 * self.default_constraint.weight
        } else {
            0.0
        };

        // Per-child: None = still in input, Some(None) = free, Some(Some(fixed)) = constrained
        let mut constrained: Vec<Option<f64>> = vec![None; n]; // None = in input list
        let mut remaining_length = total_length;
        let mut last_force = 0.0;

        for _ in 0..n + 2 {
            // Compute weight of input children
            let mut input_weight = pad_weight;
            for i in 0..n {
                if constrained[i].is_none() {
                    input_weight += constraints[i].weight;
                }
            }
            if input_weight < 1e-100 {
                break;
            }
            let force = remaining_length / input_weight;
            last_force = force;

            let mut compressed_length = 0.0;
            let mut expanded_length = 0.0;
            let mut free_length = 0.0;
            let mut compressed: Vec<usize> = Vec::new();
            let mut expanded: Vec<usize> = Vec::new();
            let mut free_list: Vec<usize> = Vec::new();

            // Classify all input children
            let mut input_indices: Vec<usize> =
                (0..n).filter(|&i| constrained[i].is_none()).collect();

            for i in input_indices.drain(..) {
                let cc = constraints[i];
                let main_size = cc.weight * force;
                let min_ct = cc.min_tallness;
                let max_ct = cc.max_tallness.max(min_ct);

                if horizontal {
                    let cw = main_size;
                    if cw <= 0.0 {
                        free_list.push(i);
                        continue;
                    }
                    let ct = cross / cw; // tallness = cross/width
                    if ct >= max_ct {
                        // Tallness too high → needs more width → "compressed" in C++ terms
                        let fixed = cross / max_ct;
                        expanded.push(i);
                        expanded_length += fixed;
                    } else if ct <= min_ct {
                        // Tallness too low → needs less width → "expanded" in C++ terms
                        let fixed = cross / min_ct;
                        compressed.push(i);
                        compressed_length += fixed;
                    } else {
                        free_list.push(i);
                        free_length += cw;
                    }
                } else {
                    let ch = main_size;
                    if ch <= 0.0 || cross <= 0.0 {
                        free_list.push(i);
                        continue;
                    }
                    let ct = ch / cross; // tallness = height/width = ch/cross
                    if ct <= min_ct {
                        let fixed = cross * min_ct;
                        compressed.push(i);
                        compressed_length += fixed;
                    } else if ct >= max_ct {
                        let fixed = cross * max_ct;
                        expanded.push(i);
                        expanded_length += fixed;
                    } else {
                        free_list.push(i);
                        free_length += ch;
                    }
                }
            }

            if compressed.is_empty() && expanded.is_empty() {
                // All free → converged
                break;
            }

            // Conflict resolution (matches C++ emLinearLayout::CalculateForce)
            if compressed.is_empty() {
                // Only expanded: fix expanded children, re-iterate with free
                for &i in &expanded {
                    let cc = constraints[i];
                    let max_ct = cc.max_tallness.max(cc.min_tallness);
                    let fixed = if horizontal {
                        cross / max_ct
                    } else {
                        cross * max_ct
                    };
                    constrained[i] = Some(fixed);
                }
                remaining_length = total_length;
                for fixed in constrained.iter().flatten() {
                    remaining_length -= fixed;
                }
                // Free children stay in input for next iteration (they're already unconstrained)
            } else if expanded.is_empty() {
                // Only compressed: subtract compressed length, re-iterate with free
                for &i in &compressed {
                    let cc = constraints[i];
                    let min_ct = cc.min_tallness;
                    let fixed = if horizontal {
                        cross / min_ct
                    } else {
                        cross * min_ct
                    };
                    constrained[i] = Some(fixed);
                }
                remaining_length = total_length;
                for fixed in constrained.iter().flatten() {
                    remaining_length -= fixed;
                }
            } else if compressed_length + expanded_length + free_length < total_length {
                // Space left over: keep expanded, release compressed
                for &i in &expanded {
                    let cc = constraints[i];
                    let max_ct = cc.max_tallness.max(cc.min_tallness);
                    let fixed = if horizontal {
                        cross / max_ct
                    } else {
                        cross * max_ct
                    };
                    constrained[i] = Some(fixed);
                }
                // Compressed children go back to input (unconstrained)
                remaining_length = total_length;
                for fixed in constrained.iter().flatten() {
                    remaining_length -= fixed;
                }
            } else {
                // Over-committed: keep compressed, release expanded
                for &i in &compressed {
                    let cc = constraints[i];
                    let min_ct = cc.min_tallness;
                    let fixed = if horizontal {
                        cross / min_ct
                    } else {
                        cross * min_ct
                    };
                    constrained[i] = Some(fixed);
                }
                remaining_length = total_length;
                for fixed in constrained.iter().flatten() {
                    remaining_length -= fixed;
                }
            }
        }

        last_force
    }
}

impl PanelBehavior for emLinearLayout {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;

    fn setup_tree(n: usize) -> (PanelTree, PanelId, Vec<PanelId>) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 600.0, 200.0);
        let mut children = Vec::new();
        for i in 0..n {
            let c = tree.create_child(root, &format!("child_{i}"));
            children.push(c);
        }
        (tree, root, children)
    }

    #[test]
    fn horizontal_equal_weight() {
        // 4 children in 600x200, no spacing. Normalized: w=1.0, h=1/3.
        // Each child 0.25 x 1/3 in normalized coords.
        let (mut tree, root, children) = setup_tree(4);
        let mut layout = emLinearLayout::horizontal();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!((r.w - 0.25).abs() < 1e-6, "child {i} width: {}", r.w);
            assert!((r.h - 1.0 / 3.0).abs() < 1e-6, "child {i} height: {}", r.h);
            assert!(
                (r.x - (i as f64 * 0.25)).abs() < 1e-6,
                "child {i} x: {}",
                r.x
            );
            assert!((r.y - 0.0).abs() < 1e-6, "child {i} y: {}", r.y);
        }
    }

    #[test]
    fn vertical_equal_weight() {
        // 2 children in 300x400, vertical, no spacing. Normalized: w=1.0, h=4/3.
        // Each child 1.0 x 2/3 in normalized coords.
        let (mut tree, root, children) = setup_tree(2);
        tree.Layout(root, 0.0, 0.0, 300.0, 400.0);
        let mut layout = emLinearLayout::vertical();
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        for (i, child) in children.iter().enumerate() {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!((r.w - 1.0).abs() < 1e-6, "child {i} width: {}", r.w);
            assert!((r.h - 2.0 / 3.0).abs() < 1e-6, "child {i} height: {}", r.h);
            assert!(
                (r.y - (i as f64 * 2.0 / 3.0)).abs() < 1e-6,
                "child {i} y: {}",
                r.y
            );
        }
    }

    #[test]
    fn weighted_distribution() {
        // 3 children in 300x100, weights [1,2,1], no spacing. Normalized: w=1.0, h=1/3.
        // total_weight=4. Widths: 0.25, 0.5, 0.25 in normalized coords.
        let (mut tree, root, children) = setup_tree(3);
        tree.Layout(root, 0.0, 0.0, 300.0, 100.0);
        let mut layout = emLinearLayout::horizontal();
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

        let w0 = tree.GetRec(children[0]).unwrap().layout_rect.w;
        let w1 = tree.GetRec(children[1]).unwrap().layout_rect.w;
        let w2 = tree.GetRec(children[2]).unwrap().layout_rect.w;
        assert!((w0 - 0.25).abs() < 1e-6, "w0={w0}");
        assert!((w1 - 0.5).abs() < 1e-6, "w1={w1}");
        assert!((w2 - 0.25).abs() < 1e-6, "w2={w2}");
    }

    #[test]
    fn spacing() {
        // C++ spacing model: margin_left=0.5, margin_right=0.5, inner_h=1.0
        // Normalized: w=1.0, h=0.5. All positions in normalized coords.
        let (mut tree, root, children) = setup_tree(2);
        tree.Layout(root, 0.0, 0.0, 200.0, 100.0);
        let mut layout = emLinearLayout::horizontal().with_spacing(Spacing {
            inner_h: 1.0,
            inner_v: 0.0,
            margin_left: 0.5,
            margin_right: 0.5,
            margin_top: 0.0,
            margin_bottom: 0.0,
        });
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        assert!((r0.x - 0.125).abs() < 1e-6, "r0.x: {}", r0.x);
        assert!((r0.w - 0.25).abs() < 1e-6, "r0.w: {}", r0.w);
        assert!((r1.x - 0.625).abs() < 1e-6, "r1.x: {}", r1.x);
        assert!((r1.w - 0.25).abs() < 1e-6, "r1.w: {}", r1.w);
        assert!((r0.h - 0.5).abs() < 1e-6, "r0.h: {}", r0.h);
    }

    #[test]
    fn tallness_constraints() {
        // 2 children in 600x100. Normalized: w=1.0, h=1/6.
        // Child 0 min_tallness=0.5 compresses to w=1/3. Child 1 gets remaining 2/3.
        let (mut tree, root, children) = setup_tree(2);
        tree.Layout(root, 0.0, 0.0, 600.0, 100.0);
        let mut layout = emLinearLayout::horizontal();
        layout.set_child_constraint(
            children[0],
            ChildConstraint {
                weight: 1.0,
                min_tallness: 0.5,
                ..Default::default()
            },
        );
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        assert!((r0.w - 1.0 / 3.0).abs() < 1e-6, "r0.w: {}", r0.w);
        assert!((r0.h - 1.0 / 6.0).abs() < 1e-6, "r0.h: {}", r0.h);
        assert!((r1.w - 2.0 / 3.0).abs() < 1e-6, "r1.w: {}", r1.w);
    }

    #[test]
    fn force_convergence() {
        // 3 children in 900x100. Normalized: w=1.0, h=1/9.
        // Child 0 min_tallness=0.5, child 1 max_tallness=0.2.
        // D-LAYOUT-04 conflict resolution. Result: 2/9, 5/9, 2/9 in normalized coords.
        let (mut tree, root, children) = setup_tree(3);
        tree.Layout(root, 0.0, 0.0, 900.0, 100.0);
        let mut layout = emLinearLayout::horizontal();
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

        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        let r2 = tree.GetRec(children[2]).unwrap().layout_rect;
        assert!((r0.w - 2.0 / 9.0).abs() < 1e-6, "r0.w: {}", r0.w);
        assert!((r1.w - 5.0 / 9.0).abs() < 1e-6, "r1.w: {}", r1.w);
        assert!((r2.w - 2.0 / 9.0).abs() < 1e-6, "r2.w: {}", r2.w);
    }
}
