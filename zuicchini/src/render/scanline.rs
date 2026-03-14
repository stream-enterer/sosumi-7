use crate::foundation::Fixed12;

/// Winding rule for polygon fill.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum WindingRule {
    EvenOdd,
    NonZero,
}

/// f64 clip bounds for scanline rasterization, matching C++ emPainter's
/// `double ClipX1, ClipY1, ClipX2, ClipY2`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ClipBounds {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// A horizontal span with per-pixel opacity for AA.
///
/// Opacities are stored in 12-bit scale (0–4096) matching C++ emPainter internals.
/// The final conversion to 8-bit alpha incorporates the color alpha:
///   `alpha = (color_alpha * opacity_12bit + 0x800) >> 12`
/// This avoids the precision loss of a two-step 12→8→8 conversion.
#[derive(Clone, Debug)]
pub(crate) struct Span {
    pub x_start: i32,
    pub x_end: i32,
    /// Opacity of leftmost pixel (0-4096 scale).
    pub opacity_beg: i32,
    /// Opacity of interior pixels (0-4096 scale).
    pub opacity_mid: i32,
    /// Opacity of rightmost pixel (0-4096 scale).
    pub opacity_end: i32,
}

/// Rasterize polygon into per-scanline spans with AA coverage.
///
/// Vertices are in pixel-space f64 coordinates. NonZero uses the C++-ported
/// polynomial coverage algorithm; EvenOdd uses edge-crossing with Fixed12.
pub(crate) fn rasterize(
    vertices: &[(f64, f64)],
    clip: ClipBounds,
    winding_rule: WindingRule,
) -> Vec<(i32, Vec<Span>)> {
    match winding_rule {
        WindingRule::NonZero => rasterize_polynomial(vertices, clip),
        WindingRule::EvenOdd => rasterize_edge_crossing(vertices, clip),
    }
}

// ─── Polynomial coverage rasterizer (NonZero) ───────────────────────

/// Scan entry: quadratic polynomial coefficients at a pixel x position.
#[derive(Clone, Debug)]
struct ScanEntry {
    a0: f64,
    a1: f64,
    a2: f64,
    x: i32,
}

/// Insert or accumulate a scan entry into the sorted entry list for a scanline.
fn add_scan_entry(scanlines: &mut [Vec<ScanEntry>], row: i32, x: i32, a0: f64, a1: f64, a2: f64) {
    if row < 0 || row as usize >= scanlines.len() {
        return;
    }
    let entries = &mut scanlines[row as usize];
    match entries.binary_search_by_key(&x, |e| e.x) {
        Ok(idx) => {
            entries[idx].a0 += a0;
            entries[idx].a1 += a1;
            entries[idx].a2 += a2;
        }
        Err(idx) => {
            entries.insert(idx, ScanEntry { a0, a1, a2, x });
        }
    }
}

/// Round absolute value: `(int)(a0 >= 0 ? 0.5 + a0 : 0.5 - a0)`.
fn round_abs(a: f64) -> i32 {
    if a >= 0.0 {
        (0.5 + a) as i32
    } else {
        (0.5 - a) as i32
    }
}

/// Build a Span from polynomial coverage values (in 0-4096 scale).
fn make_poly_span(x: i32, w: i32, alpha: i32, alpha2: i32, alpha3: i32) -> Span {
    if w == 1 {
        Span {
            x_start: x,
            x_end: x + 1,
            opacity_beg: alpha,
            opacity_mid: alpha,
            opacity_end: alpha,
        }
    } else if w == 2 {
        Span {
            x_start: x,
            x_end: x + 2,
            opacity_beg: alpha,
            opacity_mid: alpha,
            opacity_end: alpha2,
        }
    } else {
        Span {
            x_start: x,
            x_end: x + w,
            opacity_beg: alpha,
            opacity_mid: alpha2,
            opacity_end: alpha3,
        }
    }
}

/// Polynomial AA coverage rasterizer, ported from C++ emPainter::PaintPolygon.
fn rasterize_polynomial(vertices: &[(f64, f64)], clip: ClipBounds) -> Vec<(i32, Vec<Span>)> {
    let n = vertices.len();
    if n < 3 {
        return Vec::new();
    }

    // Compute polygon bounding box (vertices are already in pixel space).
    let mut min_x = vertices[0].0;
    let mut max_x = vertices[0].0;
    let mut min_y = vertices[0].1;
    let mut max_y = vertices[0].1;
    for &(x, y) in &vertices[1..] {
        if x < min_x {
            min_x = x;
        } else if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        } else if y > max_y {
            max_y = y;
        }
    }

    // Intersect with clip bounds (f64, matching C++ emPainter).
    let clip_x1 = clip.x1;
    let clip_y1 = clip.y1;
    let clip_x2 = clip.x2;
    let clip_y2 = clip.y2;

    if min_y < clip_y1 {
        min_y = clip_y1;
    }
    if max_y > clip_y2 {
        max_y = clip_y2;
    }
    if min_y >= max_y {
        return Vec::new();
    }
    if min_x < clip_x1 {
        min_x = clip_x1;
    }
    if max_x > clip_x2 - 0.0001 {
        max_x = clip_x2 - 0.0001;
    }
    if min_x >= max_x {
        return Vec::new();
    }

    let sly1 = min_y as i32;
    let sly2 = max_y.ceil() as i32;
    let num_scanlines = (sly2 - sly1) as usize;
    if num_scanlines == 0 {
        return Vec::new();
    }

    let mut scanlines: Vec<Vec<ScanEntry>> = vec![Vec::new(); num_scanlines];

    // Process edges in reverse order, matching C++ iteration.
    let mut x0 = vertices[0].0;
    let mut y0_iter = vertices[0].1;

    for i in (0..n).rev() {
        let y1_prev = y0_iter;
        y0_iter = vertices[i].1;

        let (mut x1, mut y1, mut x2, mut y2, va);
        if y1_prev > y0_iter {
            y1 = y0_iter;
            y2 = y1_prev;
            x2 = x0;
            x1 = vertices[i].0;
            x0 = x1;
            va = 4096.0_f64;
        } else {
            y1 = y1_prev;
            y2 = y0_iter;
            x1 = x0;
            x2 = vertices[i].0;
            x0 = x2;
            va = -4096.0_f64;
        }

        if y1 >= max_y || y2 <= min_y {
            continue;
        }

        // Y-clip.
        if y1 < min_y {
            if y2 - y1 >= 0.0001 {
                x1 += (min_y - y1) * (x2 - x1) / (y2 - y1);
            }
            y1 = min_y;
        }
        if y2 > max_y {
            if y2 - y1 >= 0.0001 {
                x2 += (max_y - y2) * (x2 - x1) / (y2 - y1);
            }
            y2 = max_y;
        }

        // X-clip: may produce 0-2 extra vertical segments.
        let mut extra_count = 0usize;
        let mut ex1 = [0.0_f64; 2];
        let mut ey1_arr = [0.0_f64; 2];
        let mut ex2 = [0.0_f64; 2];
        let mut ey2_arr = [0.0_f64; 2];

        if x1 < x2 {
            if x1 < min_x {
                if x2 > min_x && x2 - x1 >= 0.0001 {
                    ey1_arr[0] = y1;
                    y1 += (min_x - x1) * (y2 - y1) / (x2 - x1);
                    ey2_arr[0] = y1;
                    ex1[0] = min_x;
                    ex2[0] = min_x;
                    x1 = min_x;
                    extra_count = 1;
                } else {
                    x1 = min_x;
                    x2 = min_x;
                }
            }
            if x2 > max_x {
                if x1 < max_x && x2 - x1 >= 0.0001 {
                    ey2_arr[extra_count] = y2;
                    y2 += (max_x - x2) * (y2 - y1) / (x2 - x1);
                    ey1_arr[extra_count] = y2;
                    ex1[extra_count] = max_x;
                    ex2[extra_count] = max_x;
                    x2 = max_x;
                    extra_count += 1;
                } else {
                    x1 = max_x;
                    x2 = max_x;
                }
            }
        } else {
            if x1 > max_x {
                if x2 < max_x && x2 - x1 <= -0.0001 {
                    ey1_arr[0] = y1;
                    y1 += (max_x - x1) * (y2 - y1) / (x2 - x1);
                    ey2_arr[0] = y1;
                    ex1[0] = max_x;
                    ex2[0] = max_x;
                    x1 = max_x;
                    extra_count = 1;
                } else {
                    x1 = max_x;
                    x2 = max_x;
                }
            }
            if x2 < min_x {
                if x1 > min_x && x2 - x1 <= -0.0001 {
                    ey2_arr[extra_count] = y2;
                    y2 += (min_x - x2) * (y2 - y1) / (x2 - x1);
                    ey1_arr[extra_count] = y2;
                    ex1[extra_count] = min_x;
                    ex2[extra_count] = min_x;
                    x2 = min_x;
                    extra_count += 1;
                } else {
                    x1 = min_x;
                    x2 = min_x;
                }
            }
        }

        // Process main segment, then any extra vertical segments from x-clipping.
        loop {
            let dy = y2 - y1;
            if dy >= 0.0001 {
                let mut sy = y1 as i32;
                let sy2 = (y2.ceil() as i32) - 1;
                let ax = x1.floor();
                let mut sx = ax as i32;
                let mut t = ax + 1.0 - x1;
                let dx = x2 - x1;

                if dx >= 0.0001 || dx <= -0.0001 {
                    // Non-vertical edge: quadratic polynomial.
                    let a2 = va * dy / dx;
                    let mut a0 = t * t * 0.5 * a2;
                    let mut a1 = (t + 0.5) * a2;
                    let dx_per_row = dx / dy;
                    let mut x_cur = x1 + (sy as f64 + 1.0 - y1) * dx_per_row;

                    loop {
                        if sy >= sy2 {
                            if sy > sy2 {
                                break;
                            }
                            x_cur = x2;
                        }
                        add_scan_entry(&mut scanlines, sy - sly1, sx, a0, a1, a2);
                        let ax2 = x_cur.floor();
                        sx = ax2 as i32;
                        t = ax2 + 1.0 - x_cur;
                        a0 = t * t * 0.5 * a2;
                        a1 = (t + 0.5) * a2;
                        add_scan_entry(&mut scanlines, sy - sly1, sx, -a0, -a1, -a2);
                        x_cur += dx_per_row;
                        sy += 1;
                    }
                } else {
                    // Near-vertical edge: linear polynomial.
                    let mut a1 = va * (sy as f64 + 1.0 - y1);
                    loop {
                        if sy >= sy2 {
                            if sy > sy2 {
                                break;
                            }
                            a1 -= va * (sy2 as f64 + 1.0 - y2);
                        }
                        let a0 = t * a1;
                        add_scan_entry(&mut scanlines, sy - sly1, sx, a0, a1, 0.0);
                        a1 = va;
                        sy += 1;
                    }
                }
            }

            if extra_count == 0 {
                break;
            }
            extra_count -= 1;
            x1 = ex1[extra_count];
            y1 = ey1_arr[extra_count];
            x2 = ex2[extra_count];
            y2 = ey2_arr[extra_count];
        }
    }

    // Coverage inner loop: walk scan entries per scanline and emit spans.
    let mut result: Vec<(i32, Vec<Span>)> = Vec::with_capacity(num_scanlines);

    for (idx, entries) in scanlines.iter().enumerate() {
        if entries.is_empty() {
            continue;
        }

        let sy = sly1 + idx as i32;
        let spans = compute_spans_polynomial(entries);
        if !spans.is_empty() {
            result.push((sy, spans));
        }
    }

    result
}

/// Walk sorted scan entries for one scanline and emit coverage spans.
/// Ported from C++ emPainter lines 637-716.
fn compute_spans_polynomial(entries: &[ScanEntry]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut a1 = 0.0_f64;
    let mut a2 = 0.0_f64;
    let mut ei = 0;
    let mut sx = entries[0].x;

    loop {
        // Forward-difference step + accumulate entries at sx.
        let mut a0 = a1;
        a1 += a2;
        while ei < entries.len() && entries[ei].x == sx {
            a0 += entries[ei].a0;
            a1 += entries[ei].a1;
            a2 += entries[ei].a2;
            ei += 1;
        }
        let sx0 = sx;
        sx += 1;
        let alpha = round_abs(a0);

        if alpha == 0 {
            // Skip optimization: predict polynomial at next entry's position.
            if ei < entries.len() && entries[ei].x > sx {
                let t = a1 + a2 * (entries[ei].x - 1 - sx) as f64;
                let ta = round_abs(t);
                if ta == 0 {
                    a1 = t + a2;
                    sx = entries[ei].x;
                }
            }
            if ei >= entries.len() {
                break;
            }
            continue;
        }

        if ei >= entries.len() {
            spans.push(make_poly_span(sx0, 1, alpha, 0, 0));
            break;
        }

        // Read second pixel.
        a0 = a1;
        a1 += a2;
        while ei < entries.len() && entries[ei].x == sx {
            a0 += entries[ei].a0;
            a1 += entries[ei].a1;
            a2 += entries[ei].a2;
            ei += 1;
        }
        sx += 1;
        let alpha2 = round_abs(a0);

        if alpha2 == 0 {
            spans.push(make_poly_span(sx0, 1, alpha, 0, 0));
            if ei >= entries.len() {
                break;
            }
            continue;
        }

        if ei >= entries.len() {
            spans.push(make_poly_span(sx0, 2, alpha, 0, alpha2));
            break;
        }

        // Skip optimization: if alpha2 is constant until next entry, jump ahead.
        if entries[ei].x > sx {
            let t = a1 + a2 * (entries[ei].x - 1 - sx) as f64;
            let ta = round_abs(t);
            if alpha2 == ta {
                a1 = t + a2;
                sx = entries[ei].x;
            }
        }

        // Read third pixel.
        a0 = a1;
        a1 += a2;
        while ei < entries.len() && entries[ei].x == sx {
            a0 += entries[ei].a0;
            a1 += entries[ei].a1;
            a2 += entries[ei].a2;
            ei += 1;
        }
        sx += 1;
        let alpha3 = round_abs(a0);

        if alpha3 == 0 {
            spans.push(make_poly_span(sx0, sx - 1 - sx0, alpha, alpha2, alpha2));
        } else {
            spans.push(make_poly_span(sx0, sx - sx0, alpha, alpha2, alpha3));
        }

        if ei >= entries.len() {
            break;
        }
    }

    spans
}

// ─── Edge-crossing rasterizer (EvenOdd) ─────────────────────────────

/// Edge in the active edge table (used by edge-crossing rasterizer).
#[derive(Clone, Debug)]
struct Edge {
    x_cur: Fixed12,
    dx_per_row: Fixed12,
    y_bot: i32,
    direction: i8,
}

/// Build edge list from f64 vertices, converting to Fixed12 internally.
fn build_edges(vertices: &[(f64, f64)]) -> Vec<(i32, Edge)> {
    let n = vertices.len();
    if n < 3 {
        return Vec::new();
    }

    let mut edges: Vec<(i32, Edge)> = Vec::with_capacity(n);

    for i in 0..n {
        let (x0f, y0f) = vertices[i];
        let (x1f, y1f) = vertices[(i + 1) % n];

        let x0 = Fixed12::from_f64(x0f);
        let y0 = Fixed12::from_f64(y0f);
        let x1 = Fixed12::from_f64(x1f);
        let y1 = Fixed12::from_f64(y1f);

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
        let dx_per_row = if dy_fixed.raw() != 0 {
            Fixed12::from_raw(((dx_fixed.raw() as i64 * 4096) / dy_fixed.raw() as i64) as i32)
        } else {
            Fixed12::ZERO
        };

        let prestep = Fixed12::from_i32(top_iy) - if direction > 0 { y0 } else { y1 };
        let x_start = top_x
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

/// Edge-crossing rasterizer for EvenOdd winding rule.
fn rasterize_edge_crossing(vertices: &[(f64, f64)], clip: ClipBounds) -> Vec<(i32, Vec<Span>)> {
    let edges = build_edges(vertices);
    if edges.is_empty() {
        return Vec::new();
    }

    let clip_y_start = clip.y1 as i32;
    let clip_y_end = clip.y2.ceil() as i32;
    let clip_x_start = clip.x1 as i32;
    let clip_x_end = clip.x2.ceil() as i32;

    let y_min = edges.first().map(|(y, _)| *y).unwrap_or(0);
    let y_max = edges.iter().map(|(_, e)| e.y_bot).max().unwrap_or(0);
    let scan_start = y_min.max(clip_y_start);
    let scan_end = y_max.min(clip_y_end);

    if scan_start >= scan_end {
        return Vec::new();
    }

    let mut result: Vec<(i32, Vec<Span>)> = Vec::with_capacity((scan_end - scan_start) as usize);
    let mut aet: Vec<Edge> = Vec::new();
    let mut edge_idx = 0;

    for y in scan_start..scan_end {
        while edge_idx < edges.len() && edges[edge_idx].0 <= y {
            let (y_top, edge) = &edges[edge_idx];
            if edge.y_bot > y && *y_top <= y {
                let mut e = edge.clone();
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

        aet.retain(|e| e.y_bot > y);

        if aet.is_empty() {
            continue;
        }

        aet.sort_by_key(|e| e.x_cur.raw());
        let spans = generate_spans_edge_crossing(&aet, clip_x_start, clip_x_end);

        if !spans.is_empty() {
            result.push((y, spans));
        }

        for e in &mut aet {
            e.x_cur += e.dx_per_row;
        }
    }

    result
}

/// Generate spans from sorted active edges using even-odd winding.
fn generate_spans_edge_crossing(aet: &[Edge], clip_x_start: i32, clip_x_end: i32) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut winding = 0i32;
    let mut i = 0;
    let mut x_enter = Fixed12::ZERO;

    while i < aet.len() {
        let inside_before = winding & 1 != 0;
        let x_cur = aet[i].x_cur;

        while i < aet.len() && aet[i].x_cur == x_cur {
            winding += aet[i].direction as i32;
            i += 1;
        }

        let inside_after = winding & 1 != 0;

        if !inside_before && inside_after {
            x_enter = x_cur;
        } else if inside_before && !inside_after {
            if let Some(span) = make_edge_span(x_enter, x_cur, clip_x_start, clip_x_end) {
                spans.push(span);
            }
        }
    }

    spans
}

/// Create a span from fixed-point enter/exit x coordinates with AA coverage.
fn make_edge_span(
    x_enter: Fixed12,
    x_exit: Fixed12,
    clip_x_start: i32,
    clip_x_end: i32,
) -> Option<Span> {
    let x0 = x_enter.to_i32();
    let x1 = x_exit.to_i32();

    let px_start = x0.max(clip_x_start);
    let px_end = if x_exit.frac() == 0 { x1 } else { x1 + 1 }.min(clip_x_end);

    if px_start >= px_end {
        return None;
    }

    let frac_enter = x_enter.frac();
    let frac_exit = x_exit.frac();

    let opacity_beg = if x0 >= clip_x_start {
        4096 - frac_enter
    } else {
        0x1000
    };

    let opacity_end = if frac_exit == 0 {
        0x1000
    } else if x1 < clip_x_end {
        frac_exit
    } else {
        0x1000
    };

    if px_end - px_start == 1 {
        let coverage = (x_exit.raw() - x_enter.raw()).max(0);
        return Some(Span {
            x_start: px_start,
            x_end: px_end,
            opacity_beg: coverage,
            opacity_mid: coverage,
            opacity_end: coverage,
        });
    }

    Some(Span {
        x_start: px_start,
        x_end: px_end,
        opacity_beg,
        opacity_mid: 0x1000,
        opacity_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_vertices(x: f64, y: f64, w: f64, h: f64) -> Vec<(f64, f64)> {
        vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
    }

    #[test]
    fn pixel_aligned_rect() {
        let verts = rect_vertices(10.0, 10.0, 5.0, 3.0);
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);

        assert_eq!(rows.len(), 3, "Should have 3 scanlines for height=3");
        for (y, spans) in &rows {
            assert!(*y >= 10 && *y < 13);
            assert_eq!(spans.len(), 1);
            assert_eq!(spans[0].x_start, 10);
            assert_eq!(spans[0].x_end, 15);
            assert_eq!(spans[0].opacity_mid, 0x1000);
        }
    }

    #[test]
    fn sub_pixel_rect_has_partial_opacity() {
        let verts = rect_vertices(10.5, 10.0, 5.0, 2.0);
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);

        assert_eq!(rows.len(), 2);
        for (_, spans) in &rows {
            assert_eq!(spans.len(), 1);
            assert!(spans[0].opacity_beg < 0x1000);
        }
    }

    #[test]
    fn clipping() {
        let verts = rect_vertices(0.0, 0.0, 100.0, 100.0);
        let clip = ClipBounds {
            x1: 10.0,
            y1: 10.0,
            x2: 15.0,
            y2: 15.0,
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
        let verts: Vec<(f64, f64)> = vec![];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::NonZero);
        assert!(rows.is_empty());
    }

    #[test]
    fn triangle() {
        let verts = vec![(50.0, 10.0), (90.0, 90.0), (10.0, 90.0)];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);
        assert!(!rows.is_empty());
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
        // Outer CW square + inner CCW square = ring.
        let verts = vec![
            (0.0, 0.0),
            (20.0, 0.0),
            (20.0, 20.0),
            (0.0, 20.0),
            // Bridge to inner
            (0.0, 0.0),
            // Inner CCW
            (5.0, 5.0),
            (5.0, 15.0),
            (15.0, 15.0),
            (15.0, 5.0),
            (5.0, 5.0),
        ];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 30.0,
            y2: 30.0,
        };

        let rows = rasterize(&verts, clip, WindingRule::NonZero);
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
        let verts = vec![
            (50.0, 10.0), // apex
            (52.0, 90.0), // bottom-right
            (48.0, 90.0), // bottom-left
        ];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);
        assert!(!rows.is_empty());
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
        let verts = vec![(10.0, 10.0), (90.0, 90.0), (90.0, 10.0), (10.0, 90.0)];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::EvenOdd);
        assert!(!rows.is_empty());
        let has_two_spans = rows
            .iter()
            .any(|(y, spans)| *y > 20 && *y < 45 && spans.len() == 2);
        assert!(
            has_two_spans,
            "bowtie should produce 2 separate spans on some scanlines"
        );
    }

    #[test]
    fn nonzero_filled_rect() {
        // Verify the polynomial rasterizer fills a simple rectangle correctly.
        let verts = rect_vertices(10.0, 10.0, 5.0, 3.0);
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::NonZero);
        assert_eq!(rows.len(), 3, "Should have 3 scanlines for height=3");
        for (y, spans) in &rows {
            assert!(*y >= 10 && *y < 13);
            assert!(!spans.is_empty());
            // Check that coverage spans the full rect width.
            let min_x = spans.iter().map(|s| s.x_start).min().unwrap();
            let max_x = spans.iter().map(|s| s.x_end).max().unwrap();
            assert_eq!(min_x, 10);
            assert_eq!(max_x, 15);
            // Interior should be fully opaque.
            for span in spans {
                assert_eq!(span.opacity_mid, 0x1000);
            }
        }
    }

    #[test]
    fn nonzero_triangle() {
        let verts = vec![(50.0, 10.0), (90.0, 90.0), (10.0, 90.0)];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        };
        let rows = rasterize(&verts, clip, WindingRule::NonZero);
        assert!(!rows.is_empty());
        // Triangle should get wider toward the bottom.
        let first_width: i32 = rows[0].1.iter().map(|s| s.x_end - s.x_start).sum();
        let last_width: i32 = rows
            .last()
            .unwrap()
            .1
            .iter()
            .map(|s| s.x_end - s.x_start)
            .sum();
        assert!(last_width > first_width);
    }
}
