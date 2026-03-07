use crate::foundation::{Fixed12, PixelRect};

/// Winding rule for polygon fill.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum WindingRule {
    EvenOdd,
    NonZero,
}

/// Edge in the active edge table.
#[derive(Clone, Debug)]
struct Edge {
    /// X at current scanline (fixed-point, updated per row).
    x_cur: Fixed12,
    /// Change in x per scanline row.
    dx_per_row: Fixed12,
    /// Bottom scanline (exclusive).
    y_bot: i32,
    /// +1 for downward edge, -1 for upward (used by NonZero winding).
    direction: i8,
}

/// A horizontal span with per-pixel opacity for AA.
#[derive(Clone, Debug)]
pub(crate) struct Span {
    pub x_start: i32,
    pub x_end: i32,
    /// Opacity of leftmost pixel (0-255).
    pub opacity_beg: u8,
    /// Opacity of interior pixels (0-255).
    pub opacity_mid: u8,
    /// Opacity of rightmost pixel (0-255).
    pub opacity_end: u8,
}

/// Build edge list from polygon vertices, sorted by y_top.
fn build_edges(vertices: &[(Fixed12, Fixed12)]) -> Vec<(i32, Edge)> {
    let n = vertices.len();
    if n < 3 {
        return Vec::new();
    }

    let mut edges: Vec<(i32, Edge)> = Vec::with_capacity(n);

    for i in 0..n {
        let (x0, y0) = vertices[i];
        let (x1, y1) = vertices[(i + 1) % n];

        // Skip horizontal edges.
        let iy0 = y0.ceil().to_i32();
        let iy1 = y1.ceil().to_i32();
        if iy0 == iy1 {
            continue;
        }

        let (top_x, top_iy, bot_iy, direction) = if iy0 < iy1 {
            (x0, iy0, iy1, 1i8)
        } else {
            (x1, iy1, iy0, -1i8)
        };

        let dy_fixed = y1 - y0;
        let dx_fixed = x1 - x0;
        // dx_per_row = dx / dy (in fixed-point, but we need per-integer-row step).
        // Using raw values: dx_per_row_raw = dx_raw * 4096 / dy_raw
        let dx_per_row = if dy_fixed.raw() != 0 {
            Fixed12::from_raw(((dx_fixed.raw() as i64 * 4096) / dy_fixed.raw() as i64) as i32)
        } else {
            Fixed12::ZERO
        };

        // Pre-step x to the first scanline.
        let prestep = Fixed12::from_i32(top_iy) - if direction > 0 { y0 } else { y1 };
        let x_at_top = top_x;
        let x_start = x_at_top
            + Fixed12::from_raw(((dx_per_row.raw() as i64 * prestep.raw() as i64) >> 12) as i32);

        edges.push((
            top_iy,
            Edge {
                x_cur: x_start,
                dx_per_row,
                y_bot: bot_iy,
                direction,
            },
        ));
    }

    edges.sort_by_key(|(y_top, _)| *y_top);
    edges
}

/// Rasterize polygon edges into per-scanline spans with AA coverage.
pub(crate) fn rasterize(
    vertices: &[(Fixed12, Fixed12)],
    clip: PixelRect,
    winding_rule: WindingRule,
) -> Vec<(i32, Vec<Span>)> {
    let edges = build_edges(vertices);
    if edges.is_empty() {
        return Vec::new();
    }

    let clip_y_start = clip.y;
    let clip_y_end = clip.y + clip.h;
    let clip_x_start = clip.x;
    let clip_x_end = clip.x + clip.w;

    // Find y range.
    let y_min = edges.first().map(|(y, _)| *y).unwrap_or(0);
    let y_max = edges.iter().map(|(_, e)| e.y_bot).max().unwrap_or(0);
    let scan_start = y_min.max(clip_y_start);
    let scan_end = y_max.min(clip_y_end);

    if scan_start >= scan_end {
        return Vec::new();
    }

    let mut result: Vec<(i32, Vec<Span>)> = Vec::with_capacity((scan_end - scan_start) as usize);

    // Active edge table.
    let mut aet: Vec<Edge> = Vec::new();
    let mut edge_idx = 0;

    for y in scan_start..scan_end {
        // Add new edges that start at this scanline.
        while edge_idx < edges.len() && edges[edge_idx].0 <= y {
            let (y_top, edge) = &edges[edge_idx];
            if edge.y_bot > y && *y_top <= y {
                let mut e = edge.clone();
                // Advance x to current scanline if edge started before scan_start.
                let rows_to_skip = y - *y_top;
                if rows_to_skip > 0 {
                    e.x_cur += Fixed12::from_raw(
                        ((e.dx_per_row.raw() as i64 * rows_to_skip as i64 * 4096) >> 12) as i32,
                    );
                }
                aet.push(e);
            }
            edge_idx += 1;
        }

        // Remove expired edges.
        aet.retain(|e| e.y_bot > y);

        if aet.is_empty() {
            continue;
        }

        // Sort by x_cur.
        aet.sort_by_key(|e| e.x_cur.raw());

        // Generate spans from sorted edges.
        let spans = generate_spans(&aet, winding_rule, clip_x_start, clip_x_end);

        if !spans.is_empty() {
            result.push((y, spans));
        }

        // Advance x for next scanline.
        for e in &mut aet {
            e.x_cur += e.dx_per_row;
        }
    }

    result
}

/// Generate spans from sorted active edges for one scanline.
///
/// Edges at the same x position are grouped and their winding contributions
/// accumulated atomically. This ensures bridge edges (same geometric line
/// traversed in opposite directions) cancel without creating AA seams.
fn generate_spans(
    aet: &[Edge],
    winding_rule: WindingRule,
    clip_x_start: i32,
    clip_x_end: i32,
) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut winding = 0i32;
    let mut i = 0;
    let mut x_enter = Fixed12::ZERO;

    while i < aet.len() {
        let inside_before = is_inside(winding, winding_rule);
        let x_cur = aet[i].x_cur;

        // Accumulate winding for all edges at this x position.
        while i < aet.len() && aet[i].x_cur == x_cur {
            winding += aet[i].direction as i32;
            i += 1;
        }

        let inside_after = is_inside(winding, winding_rule);

        if !inside_before && inside_after {
            // Entering filled region.
            x_enter = x_cur;
        } else if inside_before && !inside_after {
            // Exiting filled region.
            if let Some(span) = make_span(x_enter, x_cur, clip_x_start, clip_x_end) {
                spans.push(span);
            }
        }
    }

    spans
}

fn is_inside(winding: i32, rule: WindingRule) -> bool {
    match rule {
        WindingRule::EvenOdd => winding & 1 != 0,
        WindingRule::NonZero => winding != 0,
    }
}

/// Create a span from fixed-point enter/exit x coordinates with AA coverage.
fn make_span(
    x_enter: Fixed12,
    x_exit: Fixed12,
    clip_x_start: i32,
    clip_x_end: i32,
) -> Option<Span> {
    let x0 = x_enter.to_i32();
    let x1 = x_exit.to_i32();

    // The pixel range to fill (inclusive start, exclusive end in pixel coords).
    // If exit edge lands exactly on a pixel boundary (frac=0), the last filled pixel is x1-1.
    let px_start = x0.max(clip_x_start);
    let px_end = if x_exit.frac() == 0 { x1 } else { x1 + 1 }.min(clip_x_end);

    if px_start >= px_end {
        return None;
    }

    // Compute AA coverage from fractional parts.
    let frac_enter = x_enter.frac();
    let frac_exit = x_exit.frac();

    // Opacity for the leftmost pixel: how much of it is covered.
    // If the edge enters at frac_enter into the pixel, coverage = (4096 - frac_enter) / 4096.
    let opacity_beg = if x0 >= clip_x_start {
        ((4096 - frac_enter) * 255 / 4096) as u8
    } else {
        255
    };

    // Opacity for the rightmost pixel: coverage = frac_exit / 4096.
    let opacity_end = if frac_exit == 0 {
        255
    } else if x1 < clip_x_end {
        (frac_exit * 255 / 4096) as u8
    } else {
        255
    };

    // Single-pixel span.
    if px_end - px_start == 1 {
        // Both edges in same pixel: coverage = (x_exit - x_enter) / 4096.
        let coverage = x_exit.raw() - x_enter.raw();
        let opacity = (coverage.max(0) * 255 / 4096) as u8;
        return Some(Span {
            x_start: px_start,
            x_end: px_end,
            opacity_beg: opacity,
            opacity_mid: opacity,
            opacity_end: opacity,
        });
    }

    Some(Span {
        x_start: px_start,
        x_end: px_end,
        opacity_beg,
        opacity_mid: 255,
        opacity_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_vertices(x: f64, y: f64, w: f64, h: f64) -> Vec<(Fixed12, Fixed12)> {
        vec![
            (Fixed12::from_f64(x), Fixed12::from_f64(y)),
            (Fixed12::from_f64(x + w), Fixed12::from_f64(y)),
            (Fixed12::from_f64(x + w), Fixed12::from_f64(y + h)),
            (Fixed12::from_f64(x), Fixed12::from_f64(y + h)),
        ]
    }

    #[test]
    fn pixel_aligned_rect() {
        let verts = rect_vertices(10.0, 10.0, 5.0, 3.0);
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);

        assert_eq!(rows.len(), 3, "Should have 3 scanlines for height=3");
        for (y, spans) in &rows {
            assert!(*y >= 10 && *y < 13);
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].x_start, 10);
            assert_eq!(spans[0].x_end, 15);
            assert_eq!(spans[0].opacity_mid, 255);
        }
    }

    #[test]
    fn sub_pixel_rect_has_partial_opacity() {
        let verts = rect_vertices(10.5, 10.0, 5.0, 2.0);
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);

        assert_eq!(rows.len(), 2);
        for (_, spans) in &rows {
            assert_eq!(spans.len(), 1);
            // Left edge at 10.5 means partial coverage on pixel 10.
            assert!(spans[0].opacity_beg < 255);
        }
    }

    #[test]
    fn clipping() {
        let verts = rect_vertices(0.0, 0.0, 100.0, 100.0);
        let clip = PixelRect {
            x: 10,
            y: 10,
            w: 5,
            h: 5,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);

        assert_eq!(rows.len(), 5);
        for (y, spans) in &rows {
            assert!(*y >= 10 && *y < 15);
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].x_start, 10);
            assert_eq!(spans[0].x_end, 15);
        }
    }

    #[test]
    fn empty_polygon() {
        let verts: Vec<(Fixed12, Fixed12)> = vec![];
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let rows = rasterize(&verts, clip, WindingRule::NonZero);
        assert!(rows.is_empty());
    }

    #[test]
    fn triangle() {
        let verts = vec![
            (Fixed12::from_f64(50.0), Fixed12::from_f64(10.0)),
            (Fixed12::from_f64(90.0), Fixed12::from_f64(90.0)),
            (Fixed12::from_f64(10.0), Fixed12::from_f64(90.0)),
        ];
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);
        assert!(!rows.is_empty());
        // The triangle should produce spans that get wider toward the bottom.
        let first_width = {
            let s = &rows[0].1[0];
            s.x_end - s.x_start
        };
        let last_width = {
            let s = &rows.last().unwrap().1[0];
            s.x_end - s.x_start
        };
        assert!(last_width > first_width);
    }

    #[test]
    fn nonzero_vs_evenodd_concentric() {
        // A polygon ring (outer CW, inner CCW) — NonZero fills ring, EvenOdd also fills ring.
        // Outer square 0,0 -> 20,20 CW.
        // Inner square 5,5 -> 15,15 CCW.
        let verts = vec![
            // Outer CW
            (Fixed12::from_f64(0.0), Fixed12::from_f64(0.0)),
            (Fixed12::from_f64(20.0), Fixed12::from_f64(0.0)),
            (Fixed12::from_f64(20.0), Fixed12::from_f64(20.0)),
            (Fixed12::from_f64(0.0), Fixed12::from_f64(20.0)),
            // Bridge to inner
            (Fixed12::from_f64(0.0), Fixed12::from_f64(0.0)),
            // Inner CCW
            (Fixed12::from_f64(5.0), Fixed12::from_f64(5.0)),
            (Fixed12::from_f64(5.0), Fixed12::from_f64(15.0)),
            (Fixed12::from_f64(15.0), Fixed12::from_f64(15.0)),
            (Fixed12::from_f64(15.0), Fixed12::from_f64(5.0)),
            (Fixed12::from_f64(5.0), Fixed12::from_f64(5.0)),
        ];
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 30,
            h: 30,
        };

        // With NonZero, the inner CCW cancels the outer CW, creating a hole.
        let rows = rasterize(&verts, clip, WindingRule::NonZero);
        // Check a middle scanline (y=10) — should have two spans (left and right of hole).
        let mid_row = rows.iter().find(|(y, _)| *y == 10);
        assert!(mid_row.is_some(), "Should have scanline at y=10");
        let spans = &mid_row.unwrap().1;
        assert_eq!(
            spans.len(),
            2,
            "NonZero ring should produce 2 spans at y=10"
        );
    }

    #[test]
    fn thin_triangle_narrow_spans() {
        // A tall, narrow triangle pointing up — spans should be narrow, not inflated.
        let verts = vec![
            (Fixed12::from_f64(50.0), Fixed12::from_f64(10.0)), // apex
            (Fixed12::from_f64(52.0), Fixed12::from_f64(90.0)), // bottom-right
            (Fixed12::from_f64(48.0), Fixed12::from_f64(90.0)), // bottom-left
        ];
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);
        assert!(!rows.is_empty());
        // Near the apex (first few scanlines), width must be <= 3 pixels.
        for (_, spans) in rows.iter().take(5) {
            assert_eq!(spans.len(), 1, "should have exactly one span per row");
            let width = spans[0].x_end - spans[0].x_start;
            assert!(
                width <= 3,
                "near apex, span width should be narrow, got {width}"
            );
        }
    }

    #[test]
    fn bowtie_quad_two_span_groups() {
        // A bowtie (self-intersecting quad) — at the crossing scanline, even-odd
        // should produce two separate filled regions.
        // Vertices form an X shape: top-left, bottom-right, top-right, bottom-left.
        let verts = vec![
            (Fixed12::from_f64(10.0), Fixed12::from_f64(10.0)),
            (Fixed12::from_f64(90.0), Fixed12::from_f64(90.0)),
            (Fixed12::from_f64(90.0), Fixed12::from_f64(10.0)),
            (Fixed12::from_f64(10.0), Fixed12::from_f64(90.0)),
        ];
        let clip = PixelRect {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);
        assert!(!rows.is_empty());
        // Away from the crossing point (~y=50), there should be 2 spans on some rows.
        let has_two_spans = rows
            .iter()
            .any(|(y, spans)| *y > 20 && *y < 45 && spans.len() == 2);
        assert!(
            has_two_spans,
            "bowtie should produce 2 separate spans on some scanlines"
        );
    }
}
