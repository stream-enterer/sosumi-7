use crate::foundation::Rect;
use crate::panel::{NoticeFlags, PanelBehavior, PanelCtx};
use crate::render::Painter;

use super::{Alignment, Spacing};

/// Raster (grid) layout: arranges children in a uniform grid.
pub struct RasterLayout {
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
    pub alignment: Alignment,
    pub spacing: Spacing,
}

impl Default for RasterLayout {
    fn default() -> Self {
        Self {
            row_major: true,
            fixed_columns: None,
            fixed_rows: None,
            preferred_child_tallness: 1.0,
            min_child_tallness: 0.0,
            max_child_tallness: f64::INFINITY,
            alignment: Alignment::Stretch,
            spacing: Spacing::default(),
        }
    }
}

impl RasterLayout {
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

    fn do_layout(&mut self, ctx: &mut PanelCtx) {
        let Rect { w, h, .. } = ctx.layout_rect();
        let children = ctx.children();
        let n = children.len();
        if n == 0 {
            return;
        }

        let sp = &self.spacing;
        let usable_w = (w - sp.margin_left - sp.margin_right).max(0.0);
        let usable_h = (h - sp.margin_top - sp.margin_bottom).max(0.0);

        let (cols, rows) = self.compute_grid_dims(n, usable_w, usable_h);
        if cols == 0 || rows == 0 {
            return;
        }

        let gap_w = if cols > 1 {
            sp.inner_h * (cols - 1) as f64
        } else {
            0.0
        };
        let gap_h = if rows > 1 {
            sp.inner_v * (rows - 1) as f64
        } else {
            0.0
        };

        let cell_w = ((usable_w - gap_w) / cols as f64).max(0.0);
        let mut cell_h = ((usable_h - gap_h) / rows as f64).max(0.0);

        // Clamp cell tallness
        if cell_w > 0.0 {
            let tallness = cell_h / cell_w;
            let clamped = tallness.clamp(self.min_child_tallness, self.max_child_tallness);
            cell_h = cell_w * clamped;
        }

        for (i, child) in children.iter().enumerate() {
            let (col, row) = if self.row_major {
                (i % cols, i / cols)
            } else {
                (i / rows, i % rows)
            };

            let x = sp.margin_left + col as f64 * (cell_w + sp.inner_h);
            let y = sp.margin_top + row as f64 * (cell_h + sp.inner_v);
            ctx.layout_child(*child, x, y, cell_w, cell_h);
        }
    }

    fn compute_grid_dims(&self, n: usize, usable_w: f64, usable_h: f64) -> (usize, usize) {
        match (self.fixed_columns, self.fixed_rows) {
            (Some(c), Some(r)) => (c, r),
            (Some(c), None) => {
                let c = c.max(1);
                (c, n.div_ceil(c))
            }
            (None, Some(r)) => {
                let r = r.max(1);
                (n.div_ceil(r), r)
            }
            (None, None) => self.auto_grid(n, usable_w, usable_h),
        }
    }

    /// Pick column count that makes cells closest to preferred_child_tallness.
    fn auto_grid(&self, n: usize, usable_w: f64, usable_h: f64) -> (usize, usize) {
        if n == 0 {
            return (0, 0);
        }
        let mut best_cols = 1;
        let mut best_score = f64::INFINITY;

        for c in 1..=n {
            let r = n.div_ceil(c);
            let gap_w = if c > 1 {
                self.spacing.inner_h * (c - 1) as f64
            } else {
                0.0
            };
            let gap_h = if r > 1 {
                self.spacing.inner_v * (r - 1) as f64
            } else {
                0.0
            };
            let cw = (usable_w - gap_w) / c as f64;
            let ch = (usable_h - gap_h) / r as f64;
            if cw <= 0.0 || ch <= 0.0 {
                continue;
            }
            let tallness = ch / cw;
            let score = (tallness - self.preferred_child_tallness).abs();
            if score < best_score {
                best_score = score;
                best_cols = c;
            }
        }

        let rows = n.div_ceil(best_cols);
        (best_cols, rows)
    }
}

impl PanelBehavior for RasterLayout {
    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.do_layout(ctx);
    }

    fn notice(&mut self, _flags: NoticeFlags) {}
}

/// RasterGroup wraps RasterLayout with border painting and focusable support.
pub struct RasterGroup {
    pub layout: RasterLayout,
}

impl RasterGroup {
    pub fn new() -> Self {
        Self {
            layout: RasterLayout::new(),
        }
    }
}

impl Default for RasterGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelBehavior for RasterGroup {
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
    use crate::panel::{PanelId, PanelTree};

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
    fn fixed_columns() {
        let (mut tree, root, children) = setup(6, 300.0, 200.0);
        let mut layout = RasterLayout::new().with_columns(3);
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // 3 cols, 2 rows => each cell 100x100
        let r0 = tree.get(children[0]).unwrap().layout_rect;
        assert!((r0.w - 100.0).abs() < 0.01);
        assert!((r0.h - 100.0).abs() < 0.01);
        // Child 3 is at row 1, col 0
        let r3 = tree.get(children[3]).unwrap().layout_rect;
        assert!((r3.x - 0.0).abs() < 0.01);
        assert!((r3.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn auto_column_count_square() {
        let (mut tree, root, children) = setup(4, 400.0, 400.0);
        let mut layout = RasterLayout::new().with_preferred_tallness(1.0);
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // 4 items in 400x400 with tallness 1.0 → 2x2 grid, each 200x200
        let r0 = tree.get(children[0]).unwrap().layout_rect;
        assert!((r0.w - 200.0).abs() < 0.01);
        assert!((r0.h - 200.0).abs() < 0.01);
    }

    #[test]
    fn column_major() {
        let (mut tree, root, children) = setup(4, 200.0, 200.0);
        let mut layout = RasterLayout::new().with_columns(2);
        layout.row_major = false;
        layout.do_layout(&mut PanelCtx::new(&mut tree, root));

        // Column-major: child 0 at (0,0), child 1 at (0,100), child 2 at (100,0), child 3 at (100,100)
        let r1 = tree.get(children[1]).unwrap().layout_rect;
        assert!((r1.x - 0.0).abs() < 0.01);
        assert!((r1.y - 100.0).abs() < 0.01);
        let r2 = tree.get(children[2]).unwrap().layout_rect;
        assert!((r2.x - 100.0).abs() < 0.01);
        assert!((r2.y - 0.0).abs() < 0.01);
    }
}
