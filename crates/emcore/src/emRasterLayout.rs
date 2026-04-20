use crate::emPanel::Rect;
use crate::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use crate::emEngineCtx::PanelCtx;
use crate::emPanelTree::PanelId;

use super::emTiling::{AlignmentH, AlignmentV, Spacing};

/// Raster (grid) layout: arranges children in a uniform grid.
pub struct emRasterLayout {
    /// If true, fill row-by-row; otherwise column-by-column.
    pub row_major: bool,
    /// Fixed number of columns (None = auto).
    pub fixed_columns: Option<usize>,
    /// Fixed number of rows (None = auto).
    pub fixed_rows: Option<usize>,
    /// Preferred tallness (h/w) for each cell when auto-computing columns.
    pub preferred_child_tallness: f64,
    /// Minimum cell tallness.
    pub min_child_tallness: f64,
    /// Maximum cell tallness.
    pub max_child_tallness: f64,
    /// Horizontal alignment (C++ EM_ALIGN_LEFT / EM_ALIGN_CENTER / EM_ALIGN_RIGHT).
    pub alignment_h: AlignmentH,
    /// Vertical alignment (C++ EM_ALIGN_TOP / EM_ALIGN_CENTER / EM_ALIGN_BOTTOM).
    pub alignment_v: AlignmentV,
    pub spacing: Spacing,
    /// Minimum number of cells (pads with empty space if fewer children).
    pub min_cell_count: usize,
    /// If true, increase cols/rows to keep cell tallness within min/max range
    /// rather than clamping tallness after the fact.
    pub strict_raster: bool,
}

impl Default for emRasterLayout {
    fn default() -> Self {
        Self {
            row_major: false,
            fixed_columns: None,
            fixed_rows: None,
            preferred_child_tallness: 0.2,
            min_child_tallness: 1e-4,
            max_child_tallness: 1e4,
            alignment_h: AlignmentH::Center,
            alignment_v: AlignmentV::Center,
            spacing: Spacing::default(),
            min_cell_count: 0,
            strict_raster: false,
        }
    }
}

impl emRasterLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_columns(mut self, cols: usize) -> Self {
        self.fixed_columns = Some(cols);
        self
    }

    pub fn with_rows(mut self, rows: usize) -> Self {
        self.fixed_rows = Some(rows);
        self
    }

    pub fn with_spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn with_preferred_tallness(mut self, t: f64) -> Self {
        self.preferred_child_tallness = t;
        self
    }

    pub fn with_min_cell_count(mut self, count: usize) -> Self {
        self.min_cell_count = count;
        self
    }

    pub fn with_strict_raster(mut self, strict: bool) -> Self {
        self.strict_raster = strict;
        self
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
        let n = children.len().max(self.min_cell_count);
        if n == 0 {
            return;
        }

        // Clamp degenerate dimensions to 1E-100 and continue layout (C++ parity).
        let w = w.max(1e-100);
        let h = h.max(1e-100);

        let min_ct = self.min_child_tallness.max(0.0);
        let max_ct = self.max_child_tallness.max(min_ct);
        let pref_ct = self.preferred_child_tallness.clamp(min_ct, max_ct);

        let (mut cols, mut rows) = self.compute_grid_dims_clamped(n, w, h, pref_ct);

        // Strict raster: increase cols or rows so cell tallness stays within bounds
        if self.strict_raster {
            let sp = self.spacing.clamped();
            let compute_tallness = |c: usize, r: usize| -> f64 {
                let ux = sp.margin_left + sp.inner_h * (c - 1) as f64 + sp.margin_right + c as f64;
                let uy = sp.margin_top + sp.inner_v * (r - 1) as f64 + sp.margin_bottom + r as f64;
                if ux < 1e-100 || uy < 1e-100 || w < 1e-100 {
                    return 1.0;
                }
                (h * ux * c as f64) / (w * uy * r as f64)
            };

            if self.row_major && self.fixed_columns.is_none() {
                // Increase cols while ct < min_child_tallness
                while cols < n {
                    let ct = compute_tallness(cols, rows);
                    if ct >= min_ct {
                        break;
                    }
                    cols += 1;
                    rows = n.div_ceil(cols);
                    // Cross-dimension clamp: enforce FixedRowCount (C++ parity)
                    if let Some(fr) = self.fixed_rows {
                        rows = rows.max(fr);
                    }
                }
            } else if !self.row_major && self.fixed_rows.is_none() {
                // Increase rows while ct > max_child_tallness
                while rows < n {
                    let ct = compute_tallness(cols, rows);
                    if ct <= max_ct {
                        break;
                    }
                    rows += 1;
                    cols = n.div_ceil(rows);
                    // Cross-dimension clamp: enforce FixedColumnCount (C++ parity)
                    if let Some(fc) = self.fixed_columns {
                        cols = cols.max(fc);
                    }
                }
            }
        }
        if cols == 0 || rows == 0 {
            return;
        }

        let sp = self.spacing.clamped();

        // Proportional spacing: spacing values are proportions, not pixels.
        // Each cell is 1.0 proportion-unit wide/tall. Scale factors convert to pixels.
        let denom_x =
            sp.margin_left + sp.inner_h * (cols - 1) as f64 + sp.margin_right + cols as f64;
        let denom_y =
            sp.margin_top + sp.inner_v * (rows - 1) as f64 + sp.margin_bottom + rows as f64;

        if denom_x < 1e-100 || denom_y < 1e-100 {
            return;
        }

        // Compute unclamped cell tallness and clamp it.
        let unclamped_tallness = (h * denom_x) / (w * denom_y);
        let clamped_tallness = unclamped_tallness.clamp(min_ct, max_ct);

        // Grid proportions with clamped tallness (C++ parity: lines 348-381 of
        // emRasterLayout.cpp). The clamped tallness changes the effective grid
        // aspect ratio, creating surplus on one axis that alignment consumes.
        let cw_prop = denom_x;
        let ch_prop = clamped_tallness * denom_y;

        // Shrink available space to match grid aspect ratio, then center.
        let mut avail_w = w;
        let mut avail_h = h;
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        if cw_prop > 0.0 && ch_prop > 0.0 {
            if avail_w * ch_prop >= avail_h * cw_prop {
                // Horizontal surplus
                let new_w = avail_h * cw_prop / ch_prop;
                let surplus = avail_w - new_w;
                // C++ emRasterLayout.cpp:355-356
                offset_x = match self.alignment_h {
                    AlignmentH::Center => surplus / 2.0,
                    AlignmentH::Right => surplus,
                    AlignmentH::Left => 0.0,
                };
                avail_w = new_w;
            } else {
                // Vertical surplus
                let new_h = avail_w * ch_prop / cw_prop;
                let surplus = avail_h - new_h;
                // C++ emRasterLayout.cpp:361-362
                offset_y = match self.alignment_v {
                    AlignmentV::Center => surplus / 2.0,
                    AlignmentV::Bottom => surplus,
                    AlignmentV::Top => 0.0,
                };
                avail_h = new_h;
            }
        }

        // Recompute cell sizes from the reduced space.
        let sx = avail_w / denom_x;
        let sy = avail_h / denom_y;
        let cell_w = sx;
        let cell_h = sy;

        let actual_ml = sp.margin_left * sx;
        let actual_mt = sp.margin_top * sy;
        let actual_gap_h = sp.inner_h * sx;
        let actual_gap_v = sp.inner_v * sy;

        let (base_x, base_y) = (
            origin_x + offset_x + actual_ml,
            origin_y + offset_y + actual_mt,
        );

        // Only place actual children; padding cells from min_cell_count are empty.
        for (i, child) in children.iter().enumerate() {
            let (col, row) = if self.row_major {
                (i % cols, i / cols)
            } else {
                (i / rows, i % rows)
            };

            let x = base_x + col as f64 * (cell_w + actual_gap_h);
            let y = base_y + row as f64 * (cell_h + actual_gap_v);
            ctx.layout_child(*child, x, y, cell_w, cell_h);
        }
    }

    fn compute_grid_dims_clamped(&self, n: usize, w: f64, h: f64, pref_ct: f64) -> (usize, usize) {
        match (self.fixed_columns, self.fixed_rows) {
            (Some(c), Some(r)) => {
                let c = c.max(1);
                let r = r.max(n.div_ceil(c));
                (c, r)
            }
            (Some(c), None) => {
                let c = c.max(1);
                let mut r = n.div_ceil(c);
                // Cross-dimension clamp (C++ parity: if (rows<FixedRowCount) rows=FixedRowCount)
                if let Some(fr) = self.fixed_rows {
                    r = r.max(fr);
                }
                (c, r)
            }
            (None, Some(r)) => {
                let r = r.max(1);
                let mut c = n.div_ceil(r);
                // Cross-dimension clamp (C++ parity)
                if let Some(fc) = self.fixed_columns {
                    c = c.max(fc);
                }
                (c, r)
            }
            (None, None) => self.auto_grid_clamped(n, w, h, pref_ct),
        }
    }

    /// Auto-compute grid dimensions matching C++ emRasterLayout (column-major iteration).
    ///
    /// C++ iterates over rows starting at 1, computing cols=ceil(n/rows), and
    /// picks the rows value that minimizes |ln(pref_ct / ct)|. Uses a skip
    /// optimization: `rows = (n + cols - 2) / (cols - 1)` to jump past rows
    /// values that produce the same cols count.
    fn auto_grid_clamped(&self, n: usize, w: f64, h: f64, pref_ct: f64) -> (usize, usize) {
        if n == 0 {
            return (0, 0);
        }
        if pref_ct <= 0.0 || w <= 0.0 || h <= 0.0 {
            return (1, n);
        }

        let sp = self.spacing.clamped();
        let mut rows_best = 1usize;
        let mut err_best = 0.0f64;
        let mut rows = 1usize;

        loop {
            let cols = n.div_ceil(rows);
            let sx = sp.margin_left + sp.margin_right + sp.inner_h * (cols - 1) as f64;
            let sy = sp.margin_top + sp.margin_bottom + sp.inner_v * (rows - 1) as f64;
            let ux = sx / cols as f64 + 1.0;
            let uy = sy / rows as f64 + 1.0;
            let ct = h * ux * cols as f64 / (w * uy * rows as f64);
            let err = (pref_ct / ct).ln().abs();

            if rows == 1 || err < err_best {
                rows_best = rows;
                err_best = err;
            }
            if cols == 1 {
                break;
            }
            // Skip to next rows value that reduces cols (C++ optimization)
            rows = (n + cols - 2) / (cols - 1);
        }

        let cols = n.div_ceil(rows_best);
        (cols, rows_best)
    }
}

impl PanelBehavior for emRasterLayout {
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Propagate parent canvas color to children. emRasterLayout has no
        // border, so pass through unchanged — matches C++ emPanel default.
        let cc = ctx.GetCanvasColor();
        ctx.set_all_children_canvas_color(cc);
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::{PanelId, PanelTree};

    fn setup(n: usize, w: f64, h: f64) -> (PanelTree, PanelId, Vec<PanelId>) {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, w, h, 1.0, None);
        let mut children = Vec::new();
        for i in 0..n {
            children.push(tree.create_child(root, &format!("c{i}"), None));
        }
        (tree, root, children)
    }

    #[test]
    fn fixed_columns() {
        let (mut tree, root, children) = setup(6, 300.0, 200.0);
        let mut layout = emRasterLayout::new().with_columns(3);
        layout.row_major = true;
        layout.do_layout(&mut PanelCtx::new(&mut tree, root, 1.0));

        // 3 cols, 2 rows in normalized space (w=1.0, h=2/3)
        // Each cell: 1/3 x 1/3
        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        assert!((r0.w - 1.0 / 3.0).abs() < 0.001);
        assert!((r0.h - 1.0 / 3.0).abs() < 0.001);
        // Child 3 is at row 1, col 0
        let r3 = tree.GetRec(children[3]).unwrap().layout_rect;
        assert!((r3.x - 0.0).abs() < 0.001);
        assert!((r3.y - 1.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn auto_column_count_square() {
        let (mut tree, root, children) = setup(4, 400.0, 400.0);
        let mut layout = emRasterLayout::new().with_preferred_tallness(1.0);
        layout.do_layout(&mut PanelCtx::new(&mut tree, root, 1.0));

        // 4 items in normalized 1.0x1.0 with tallness 1.0 -> 2x2 grid, each 0.5x0.5
        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        assert!((r0.w - 0.5).abs() < 0.001);
        assert!((r0.h - 0.5).abs() < 0.001);
    }

    #[test]
    fn alignment_center() {
        // 2 items in 400x600. Log scoring picks 1 col x 2 rows.
        // cell_w=400, unclamped tallness=0.75, clamped to max 0.5 -> cell_h=200.
        // Grid is 400x400, vertical surplus=200. Center -> offset_y=100.
        let (mut tree, root, children) = setup(2, 400.0, 600.0);
        let mut layout = emRasterLayout::new();
        layout.row_major = true;
        layout.alignment_h = AlignmentH::Center;
        layout.alignment_v = AlignmentV::Center;
        layout.preferred_child_tallness = 0.5;
        layout.min_child_tallness = 0.1;
        layout.max_child_tallness = 0.5;
        layout.do_layout(&mut PanelCtx::new(&mut tree, root, 1.0));

        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        assert!((r0.y - 0.25).abs() < 0.001, "child 0 y: {}", r0.y);
        assert!((r0.h - 0.5).abs() < 0.001, "child 0 h: {}", r0.h);
        assert!((r1.y - 0.75).abs() < 0.001, "child 1 y: {}", r1.y);
        assert!((r0.w - 1.0).abs() < 0.001, "child 0 w: {}", r0.w);
    }

    #[test]
    fn column_major() {
        let (mut tree, root, children) = setup(4, 200.0, 200.0);
        let mut layout = emRasterLayout::new().with_columns(2);
        layout.row_major = false;
        layout.do_layout(&mut PanelCtx::new(&mut tree, root, 1.0));

        // Column-major: child 0 at (0,0), child 1 at (0,0.5), child 2 at (0.5,0)
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        assert!((r1.x - 0.0).abs() < 0.001);
        assert!((r1.y - 0.5).abs() < 0.001);
        let r2 = tree.GetRec(children[2]).unwrap().layout_rect;
        assert!((r2.x - 0.5).abs() < 0.001);
        assert!((r2.y - 0.0).abs() < 0.001);
    }

    #[test]
    fn min_cell_count_pads_grid() {
        // 2 children but min_cell_count=6, fixed 3 cols -> 2 rows.
        // Each cell is 100x100; children only placed in first 2 slots.
        let (mut tree, root, children) = setup(2, 300.0, 200.0);
        let mut layout = emRasterLayout::new().with_columns(3).with_min_cell_count(6);
        layout.row_major = true;
        layout.do_layout(&mut PanelCtx::new(&mut tree, root, 1.0));

        let r0 = tree.GetRec(children[0]).unwrap().layout_rect;
        let r1 = tree.GetRec(children[1]).unwrap().layout_rect;
        // 6 cells in 3 cols -> 2 rows, each cell 1/3 x 1/3
        assert!((r0.w - 1.0 / 3.0).abs() < 0.001, "child 0 w: {}", r0.w);
        assert!((r0.h - 1.0 / 3.0).abs() < 0.001, "child 0 h: {}", r0.h);
        assert!((r1.x - 1.0 / 3.0).abs() < 0.001, "child 1 x: {}", r1.x);
        assert!((r1.y - 0.0).abs() < 0.001, "child 1 y: {}", r1.y);
    }

    #[test]
    fn strict_raster_row_major_increases_cols() {
        // 4 children in 100x400 (very tall). Without strict, auto picks 1 col x 4 rows
        // (tallness=1.0). With strict + min_child_tallness=2.0, it should increase cols
        // until tallness >= 2.0.
        let (mut tree, root, children) = setup(4, 100.0, 400.0);
        let mut layout = emRasterLayout::new()
            .with_preferred_tallness(1.0)
            .with_strict_raster(true);
        layout.row_major = true;
        layout.min_child_tallness = 2.0;
        layout.max_child_tallness = 1e4;
        layout.do_layout(&mut PanelCtx::new(&mut tree, root, 1.0));

        // All children should be laid out with positive sizes
        for child in &children {
            let r = tree.GetRec(*child).unwrap().layout_rect;
            assert!(r.w > 0.0);
            assert!(r.h > 0.0);
            // Cell tallness should be >= 2.0 (after clamping)
            assert!(r.h / r.w >= 2.0 - 0.01, "tallness: {}", r.h / r.w);
        }
    }
}
