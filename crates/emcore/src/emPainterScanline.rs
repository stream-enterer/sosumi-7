// SPLIT: Split from emPainter.h — scanline rendering extracted
use crate::emPainter::Fixed12;

/// Winding rule for polygon fill.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindingRule {
    EvenOdd,
    NonZero,
}

/// f64 clip bounds for scanline rasterization, matching C++ emPainter's
/// `double ClipX1, ClipY1, ClipX2, ClipY2`.
#[derive(Copy, Clone, Debug)]
pub struct ClipBounds {
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
pub struct Span {
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
pub fn rasterize(
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
// Line-by-line port of C++ emPainter::PaintPolygon (emPainter.cpp ~460-800).

/// Scan entry: quadratic polynomial coefficients at a pixel x position.
/// C++ struct ScanEntry { double A0, A1, A2; ScanEntry *Next; int X; }
#[derive(Clone, Debug)]
struct ScanEntry {
    a0: f64,
    a1: f64,
    a2: f64,
    x: i32,
}

/// Insert or accumulate a scan entry into the sorted entry list for a scanline.
/// Port of C++ PP_ADD_SCAN_ENTRY macro (lines 547-577).
///
/// The C++ macro traverses a sorted linked list, accumulating into existing
/// entries or inserting new ones. We use a Vec with linear scan for the same
/// sorted-insert/accumulate behavior.
fn add_scan_entry(
    scanlines: &mut [Vec<ScanEntry>],
    row: i32,
    x: i32,
    a0: f64,
    a1: f64,
    a2: f64,
) {
    if row < 0 || row as usize >= scanlines.len() {
        return;
    }
    let entries = &mut scanlines[row as usize];
    // C++ linked-list scan: walk forward while pse->X < x.
    // With a Vec, find the insertion/accumulation point by linear scan
    // (matching C++ traversal order, which starts from the head).
    let mut pos = 0;
    while pos < entries.len() && entries[pos].x < x {
        pos += 1;
    }
    if pos < entries.len() && entries[pos].x == x {
        entries[pos].a0 += a0;
        entries[pos].a1 += a1;
        entries[pos].a2 += a2;
    } else {
        entries.insert(pos, ScanEntry { a0, a1, a2, x });
    }
}

/// Round absolute value: `(int)(a0 >= 0 ? 0.5 + a0 : 0.5 - a0)`.
/// Matches C++ lines 731, 735, 756, 767, 782.
#[inline]
fn round_abs(a: f64) -> i32 {
    if a >= 0.0 {
        (0.5 + a) as i32
    } else {
        (0.5 - a) as i32
    }
}

/// Polynomial AA coverage rasterizer, ported from C++ emPainter::PaintPolygon.
/// Lines 460-800 of emPainter.cpp.
///
/// Vertices are already in pixel space (the C++ ScaleX/ScaleY/OriginX/OriginY
/// transforms are applied by the caller in fill_polygon_aa).
fn rasterize_polynomial(vertices: &[(f64, f64)], clip: ClipBounds) -> Vec<(i32, Vec<Span>)> {
    let n = vertices.len();
    // C++ line 500: if (n<3) return;
    if n < 3 {
        return Vec::new();
    }

    // C++ lines 502-511: Compute polygon bounding box.
    // Vertices are already in pixel space, so no Scale/Origin transforms.
    let mut min_x = vertices[0].0;
    let mut max_x = vertices[0].0;
    let mut min_y = vertices[0].1;
    let mut max_y = vertices[0].1;
    // C++ iterates pxy from last vertex down to vertex[1].
    // We iterate forward from vertex[1] — same result (commutative min/max).
    for &(vx, vy) in &vertices[1..] {
        if max_x < vx {
            max_x = vx;
        } else if min_x > vx {
            min_x = vx;
        }
        if max_y < vy {
            max_y = vy;
        } else if min_y > vy {
            min_y = vy;
        }
    }

    // C++ lines 512-521: Intersect bounding box with clip bounds.
    // (C++ applies Scale/Origin here; we skip since vertices are already pixel-space.)
    if min_y < clip.y1 {
        min_y = clip.y1;
    }
    if max_y > clip.y2 {
        max_y = clip.y2;
    }
    if min_y >= max_y {
        return Vec::new();
    }
    if min_x < clip.x1 {
        min_x = clip.x1;
    }
    if max_x > clip.x2 - 0.0001 {
        max_x = clip.x2 - 0.0001;
    }
    if min_x >= max_x {
        return Vec::new();
    }

    // C++ lines 528-529: sly1=(int)minY; sly2=(int)ceil(maxY);
    let sly1 = min_y as i32;
    let sly2 = max_y.ceil() as i32;
    let num_scanlines = (sly2 - sly1) as usize;
    if num_scanlines == 0 {
        return Vec::new();
    }

    // C++ lines 531-545: Allocate scanline entry lists.
    // We use Vec<Vec<ScanEntry>> indexed by (sy - sly1).
    let mut scanlines: Vec<Vec<ScanEntry>> = vec![Vec::new(); num_scanlines];

    // C++ lines 579-711: Process edges in reverse order.
    // x0=xy[0]*ScaleX+OriginX; y0=xy[1]*ScaleY+OriginY;
    let mut x0 = vertices[0].0;
    let mut y0 = vertices[0].1;

    // C++ line 581: for (pxy=xy+n*2-2; pxy>=xy; pxy-=2)
    // Iterates edges: 0→n-1, n-1→n-2, ..., 1→0
    for idx in (0..n).rev() {
        // C++ lines 582-596: Determine edge direction.
        let y1_prev = y0;
        y0 = vertices[idx].1;

        let (mut x1, mut y1, mut x2, mut y2, va);
        if y1_prev > y0 {
            // C++ lines 584-589: edge goes downward in original order
            y2 = y1_prev;
            y1 = y0;
            x2 = x0;
            x1 = vertices[idx].0;
            x0 = x1;
            va = 0x1000 as f64; // 4096.0
        } else {
            // C++ lines 591-596: edge goes upward in original order
            y2 = y0;
            y1 = y1_prev;
            x1 = x0;
            x2 = vertices[idx].0;
            x0 = x2;
            va = -(0x1000 as f64); // -4096.0
        }

        // C++ line 597: if (y1>=maxY || y2<=minY) continue;
        if y1 >= max_y || y2 <= min_y {
            continue;
        }

        // C++ lines 598-605: Y-clip.
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

        // C++ lines 606-658: X-clip (may produce 0-2 extra vertical segments).
        let mut i = 0usize;
        let mut ex1 = [0.0_f64; 2];
        let mut ey1_arr = [0.0_f64; 2];
        let mut ex2 = [0.0_f64; 2];
        let mut ey2_arr = [0.0_f64; 2];

        if x1 < x2 {
            // C++ lines 608-631
            if x1 < min_x {
                if x2 > min_x && x2 - x1 >= 0.0001 {
                    ey1_arr[0] = y1;
                    y1 += (min_x - x1) * (y2 - y1) / (x2 - x1);
                    ey2_arr[0] = y1;
                    ex1[0] = min_x;
                    ex2[0] = min_x;
                    x1 = min_x;
                    i = 1;
                } else {
                    x1 = min_x;
                    x2 = min_x;
                }
            }
            if x2 > max_x {
                if x1 < max_x && x2 - x1 >= 0.0001 {
                    ey2_arr[i] = y2;
                    y2 += (max_x - x2) * (y2 - y1) / (x2 - x1);
                    ey1_arr[i] = y2;
                    ex1[i] = max_x;
                    ex2[i] = max_x;
                    x2 = max_x;
                    i += 1;
                } else {
                    x1 = max_x;
                    x2 = max_x;
                }
            }
        } else {
            // C++ lines 633-658
            if x1 > max_x {
                if x2 < max_x && x2 - x1 <= -0.0001 {
                    ey1_arr[0] = y1;
                    y1 += (max_x - x1) * (y2 - y1) / (x2 - x1);
                    ey2_arr[0] = y1;
                    ex1[0] = max_x;
                    ex2[0] = max_x;
                    x1 = max_x;
                    i = 1;
                } else {
                    x1 = max_x;
                    x2 = max_x;
                }
            }
            if x2 < min_x {
                if x1 > min_x && x2 - x1 <= -0.0001 {
                    ey2_arr[i] = y2;
                    y2 += (min_x - x2) * (y2 - y1) / (x2 - x1);
                    ey1_arr[i] = y2;
                    ex1[i] = min_x;
                    ex2[i] = min_x;
                    x2 = min_x;
                    i += 1;
                } else {
                    x1 = min_x;
                    x2 = min_x;
                }
            }
        }

        // C++ lines 659-710: Process main segment then extra vertical segments.
        loop {
            let dy = y2 - y1;
            // C++ line 661: if (dy>=0.0001)
            if dy >= 0.0001 {
                let mut sy = y1 as i32;
                let sy2 = (y2.ceil() as i32) - 1;
                let ax = x1.floor();
                let mut sx = ax as i32;
                let mut t = ax + 1.0 - x1;
                let dx = x2 - x1;

                if dx >= 0.0001 || dx <= -0.0001 {
                    // C++ lines 669-688: Non-vertical edge (quadratic polynomial).
                    let a2 = va * dy / dx;
                    let mut a0 = t * t * 0.5 * a2;
                    let mut a1 = (t + 0.5) * a2;
                    // C++ line 672: dx/=dy; (reuse dx as slope per scanline)
                    let dx_per_row = dx / dy;
                    // C++ line 673: x1+=(sy+1-y1)*dx;
                    let mut x_cur = x1 + (sy as f64 + 1.0 - y1) * dx_per_row;

                    // C++ lines 674-688: for(;;)
                    loop {
                        if sy >= sy2 {
                            if sy > sy2 {
                                break;
                            }
                            x_cur = x2;
                        }
                        // C++ line 679: PP_ADD_SCAN_ENTRY(sx,sy,a0,a1,a2)
                        add_scan_entry(&mut scanlines, sy - sly1, sx, a0, a1, a2);
                        let ax2 = x_cur.floor();
                        sx = ax2 as i32;
                        t = ax2 + 1.0 - x_cur;
                        a0 = t * t * 0.5 * a2;
                        a1 = (t + 0.5) * a2;
                        // C++ line 685: PP_ADD_SCAN_ENTRY(sx,sy,(-a0),(-a1),(-a2))
                        add_scan_entry(&mut scanlines, sy - sly1, sx, -a0, -a1, -a2);
                        // C++ line 686: x1+=dx; (advance x for next scanline)
                        x_cur += dx_per_row;
                        sy += 1;
                    }
                } else {
                    // C++ lines 690-701: Near-vertical edge (linear polynomial).
                    let mut a1 = va * (sy as f64 + 1.0 - y1);
                    loop {
                        if sy >= sy2 {
                            if sy > sy2 {
                                break;
                            }
                            a1 -= va * (sy2 as f64 + 1.0 - y2);
                        }
                        let a0 = t * a1;
                        // C++ line 698: PP_ADD_SCAN_ENTRY(sx,sy,a0,a1,0.0)
                        add_scan_entry(&mut scanlines, sy - sly1, sx, a0, a1, 0.0);
                        a1 = va;
                        sy += 1;
                    }
                }
            }

            // C++ lines 704-710: Process extra segments from X-clipping.
            if i == 0 {
                break;
            }
            i -= 1;
            x1 = ex1[i];
            y1 = ey1_arr[i];
            x2 = ex2[i];
            y2 = ey2_arr[i];
        }
    }

    // C++ lines 713-792: Walk scan entries per scanline and emit spans.
    let mut result: Vec<(i32, Vec<Span>)> = Vec::with_capacity(num_scanlines);

    // C++ line 713: sy=sly1;
    // C++ line 714-792: do { ... } while (sy<sly2);
    for (idx, entries) in scanlines.iter().enumerate() {
        // C++ line 716: if (pse!=&seTerminator) { ... }
        if entries.is_empty() {
            continue;
        }

        let sy = sly1 + idx as i32;
        let spans = emit_scanline_spans(entries);
        if !spans.is_empty() {
            result.push((sy, spans));
        }
    }

    result
}

/// Walk sorted scan entries for one scanline and emit coverage spans.
/// Line-by-line port of C++ emPainter::PaintPolygon lines 717-789.
///
/// C++ uses a do { ... } while(pse!=&seTerminator) loop walking a linked list.
/// We use a loop walking a sorted Vec with an index `ei`.
fn emit_scanline_spans(entries: &[ScanEntry]) -> Vec<Span> {
    let mut spans = Vec::new();

    // C++ lines 717-719: a1=0; a2=0; sx=pse->X;
    let mut a1 = 0.0_f64;
    let mut a2 = 0.0_f64;
    let mut ei = 0usize;
    let mut sx = entries[0].x;

    // C++ line 720: do { ... } while (pse!=&seTerminator);
    loop {
        // C++ lines 721-728: Forward-difference step + accumulate entries at sx.
        let mut a0 = a1;
        a1 += a2;
        // C++ uses `if (pse->X==sx)` — at most one entry per X due to
        // add_scan_entry accumulation. We use while for defensive correctness.
        while ei < entries.len() && entries[ei].x == sx {
            a0 += entries[ei].a0;
            a1 += entries[ei].a1;
            a2 += entries[ei].a2;
            ei += 1;
        }

        // C++ lines 729-731:
        let sx0 = sx;
        sx += 1;
        let alpha = round_abs(a0);

        // C++ lines 732-742: alpha==0 path with skip optimization.
        if alpha == 0 {
            // C++ lines 733-740: Skip optimization.
            if ei < entries.len() && entries[ei].x > sx {
                let t = a1 + a2 * (entries[ei].x - 1 - sx) as f64;
                let ta = round_abs(t);
                // C++ line 736: if (alpha==ta) — alpha is 0 here.
                if ta == 0 {
                    a1 = t + a2;
                    sx = entries[ei].x;
                }
            }
            // C++ line 741: continue; → jumps to while(pse!=&seTerminator)
            // In Rust, check if we've consumed all entries (= terminator reached).
            if ei >= entries.len() {
                break;
            }
            continue;
        }

        // C++ lines 743-746: Last entry consumed → single pixel span.
        if ei >= entries.len() {
            // C++ line 744: sct.PaintScanline(sct,sx0,sy,1,alpha,0,0);
            spans.push(Span {
                x_start: sx0,
                x_end: sx0.saturating_add(1),
                opacity_beg: alpha,
                opacity_mid: 0,
                opacity_end: 0,
            });
            break;
        }

        // C++ lines 747-754: Read second pixel.
        a0 = a1;
        a1 += a2;
        while ei < entries.len() && entries[ei].x == sx {
            a0 += entries[ei].a0;
            a1 += entries[ei].a1;
            a2 += entries[ei].a2;
            ei += 1;
        }
        sx += 1;
        // C++ line 756:
        let alpha2 = round_abs(a0);

        // C++ lines 757-760: alpha2==0 → emit 1-pixel span.
        if alpha2 == 0 {
            // C++ line 758: sct.PaintScanline(sct,sx0,sy,1,alpha,0,0);
            spans.push(Span {
                x_start: sx0,
                x_end: sx0.saturating_add(1),
                opacity_beg: alpha,
                opacity_mid: 0,
                opacity_end: 0,
            });
            // C++ line 759: continue; → while(pse!=&seTerminator)
            if ei >= entries.len() {
                break;
            }
            continue;
        }

        // C++ lines 761-764: Last entry consumed → 2-pixel span.
        if ei >= entries.len() {
            // C++ line 762: sct.PaintScanline(sct,sx0,sy,2,alpha,0,alpha2);
            spans.push(Span {
                x_start: sx0,
                x_end: sx0.saturating_add(2),
                opacity_beg: alpha,
                opacity_mid: 0,
                opacity_end: alpha2,
            });
            break;
        }

        // C++ lines 765-771: Skip optimization for constant alpha2 run.
        if entries[ei].x > sx {
            let t = a1 + a2 * (entries[ei].x - 1 - sx) as f64;
            let ta = round_abs(t);
            if alpha2 == ta {
                a1 = t + a2;
                sx = entries[ei].x;
            }
        }

        // C++ lines 773-781: Read third pixel.
        a0 = a1;
        a1 += a2;
        while ei < entries.len() && entries[ei].x == sx {
            a0 += entries[ei].a0;
            a1 += entries[ei].a1;
            a2 += entries[ei].a2;
            ei += 1;
        }
        sx += 1;
        // C++ line 782:
        let alpha3 = round_abs(a0);

        // C++ lines 783-788: Emit multi-pixel span.
        if alpha3 == 0 {
            // C++ line 784: sct.PaintScanline(sct,sx0,sy,sx-1-sx0,alpha,alpha2,alpha2);
            let w = sx - 1 - sx0;
            spans.push(Span {
                x_start: sx0,
                x_end: sx0.saturating_add(w),
                opacity_beg: alpha,
                opacity_mid: alpha2,
                opacity_end: alpha2,
            });
        } else {
            // C++ line 787: sct.PaintScanline(sct,sx0,sy,sx-sx0,alpha,alpha2,alpha3);
            let w = sx - sx0;
            spans.push(Span {
                x_start: sx0,
                x_end: sx0.saturating_add(w),
                opacity_beg: alpha,
                opacity_mid: alpha2,
                opacity_end: alpha3,
            });
        }

        // C++ line 789: } while (pse!=&seTerminator);
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
// DIVERGED: C++ uses i32 subtraction for `x_exit.raw() - x_enter.raw()` and
// `px_end - px_start` which are signed overflow UB for extreme Fixed12 values.
// Rust uses i64 promotion for the coverage difference and saturating_sub for
// span width. In practice coordinates are bounded by viewport pixels.
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

    if px_end.saturating_sub(px_start) == 1 {
        let coverage = (x_exit.raw() as i64 - x_enter.raw() as i64).max(0) as i32;
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

    /// Verify polygon_tri top vertex produces correct coverage.
    /// C++ golden shows partial red coverage at (128,20). The vertex is
    /// at exactly (128.0, 20.0) — integer coordinates.
    #[test]
    fn polygon_tri_top_vertex_coverage() {
        let verts = vec![(128.0, 20.0), (20.0, 230.0), (236.0, 230.0)];
        let clip = ClipBounds {
            x1: 0.0,
            y1: 0.0,
            x2: 256.0,
            y2: 256.0,
        };
        let rows = rasterize_polynomial(&verts, clip);
        let row20 = rows.iter().find(|(y, _)| *y == 20);
        assert!(row20.is_some(), "Row 20 must have spans");
        let spans = &row20.unwrap().1;
        // Pixel 128 should have non-zero coverage (C++ produces rgb(255,189,189)).
        let cov_at_128 = spans
            .iter()
            .find_map(|s| {
                if s.x_start <= 128 && s.x_end > 128 {
                    // Determine which opacity applies to x=128.
                    if 128 == s.x_start {
                        Some(s.opacity_beg)
                    } else if 128 == s.x_end - 1 {
                        Some(s.opacity_end)
                    } else {
                        Some(s.opacity_mid)
                    }
                } else {
                    None
                }
            })
            .unwrap_or(0);
        assert!(
            cov_at_128 > 0,
            "Pixel (128,20) must have non-zero coverage, got 0 (span: {:?})",
            spans
        );
    }
}


#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_make_edge_span() {
        let p_x_enter = crate::emPainter::Fixed12::from_raw(kani::any());
        let p_x_exit = crate::emPainter::Fixed12::from_raw(kani::any());
        let p_clip_x_start: i32 = kani::any::<i32>();
        let p_clip_x_end: i32 = kani::any::<i32>();
        let _r = make_edge_span(p_x_enter, p_x_exit, p_clip_x_start, p_clip_x_end);
    }

    #[kani::proof]
    fn kani_private_round_abs() {
        let p_a: f64 = kani::any::<f64>();
        kani::assume(p_a.is_finite());
        let _r = round_abs(p_a);
    }

    // Layer 3: round_abs symmetry — C++ uses (int)(fabs(x) + 0.5)
    #[kani::proof]
    fn l3_round_abs_symmetric() {
        let v: u16 = kani::any();
        let a = v as f64 / 100.0;
        assert_eq!(round_abs(a), round_abs(-a), "round_abs not symmetric");
    }

    // Layer 3: round_abs matches C++ (int)(fabs(x) + 0.5) for non-negative
    #[kani::proof]
    fn l3_round_abs_matches_cpp() {
        let v: u16 = kani::any();
        let a = v as f64 / 100.0;
        let cpp = (a.abs() + 0.5) as i32;
        assert_eq!(round_abs(a), cpp);
    }
}
