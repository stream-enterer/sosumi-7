use zuicchini::foundation::{Color, Image};
use zuicchini::render::{
    DashType, LineCap, LineJoin, Painter, Stroke, StrokeEnd, StrokeEndType, TextAlignment, VAlign,
};

use super::common::*;

fn white_canvas(w: u32, h: u32) -> Image {
    let mut img = Image::new(w, h, 4);
    img.fill(Color::WHITE);
    img
}

/// Create a Painter with TRANSPARENT canvas (standard alpha blending).
/// In C++ emPainter, canvasColor defaults to 0 (non-opaque) per call, which
/// uses standard alpha blending. Match that behavior here.
fn white_painter(img: &mut Image) -> Painter<'_> {
    let mut p = Painter::new(img);
    p.set_canvas_color(Color::TRANSPARENT);
    p
}

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

// ─── Test 1: rect_solid ──────────────────────────────────────────
#[test]
fn painter_rect_solid() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("rect_solid");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_rect(20.0, 20.0, 100.0, 80.0, Color::RED, Color::TRANSPARENT);
    }
    compare_images("rect_solid", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 2: rect_alpha ─────────────────────────────────────────
#[test]
fn painter_rect_alpha() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("rect_alpha");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_rect(
            20.0,
            20.0,
            100.0,
            80.0,
            Color::rgba(255, 0, 0, 128),
            Color::TRANSPARENT,
        );
    }
    compare_images("rect_alpha", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 3: rect_overlap ───────────────────────────────────────
#[test]
fn painter_rect_overlap() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("rect_overlap");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_rect(20.0, 20.0, 100.0, 80.0, Color::RED, Color::TRANSPARENT);
        p.paint_rect(
            60.0,
            40.0,
            100.0,
            80.0,
            Color::rgba(0, 0, 255, 128),
            Color::TRANSPARENT,
        );
    }
    compare_images("rect_overlap", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 4: ellipse_basic ──────────────────────────────────────
#[test]
fn painter_ellipse_basic() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("ellipse_basic");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // C++ PaintEllipse(28,28,200,150) → cx=128 cy=103 rx=100 ry=75
        p.paint_ellipse(128.0, 103.0, 100.0, 75.0, Color::GREEN, Color::TRANSPARENT);
    }
    compare_images("ellipse_basic", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 5: ellipse_small ──────────────────────────────────────
#[test]
fn painter_ellipse_small() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("ellipse_small");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // C++ PaintEllipse(118,118,20,20) → cx=128 cy=128 rx=10 ry=10
        p.paint_ellipse(128.0, 128.0, 10.0, 10.0, Color::BLUE, Color::TRANSPARENT);
    }
    compare_images("ellipse_small", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 6: polygon_tri ────────────────────────────────────────
#[test]
fn painter_polygon_tri() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("polygon_tri");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_polygon(
            &[(128.0, 20.0), (20.0, 230.0), (236.0, 230.0)],
            Color::RED,
            Color::TRANSPARENT,
        );
    }
    compare_images("polygon_tri", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 7: polygon_star ───────────────────────────────────────
fn star_vertices() -> Vec<(f64, f64)> {
    let cx = 128.0;
    let cy = 128.0;
    let outer = 110.0;
    let inner = 45.0;
    let mut verts = Vec::with_capacity(10);
    for i in 0..10 {
        let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * 2.0 * i as f64 / 10.0;
        let r = if i % 2 == 0 { outer } else { inner };
        verts.push((cx + r * angle.cos(), cy + r * angle.sin()));
    }
    verts
}

#[test]
fn painter_polygon_star() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("polygon_star");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_polygon(&star_vertices(), Color::MAGENTA, Color::TRANSPARENT);
    }
    compare_images("polygon_star", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 8: polygon_complex ────────────────────────────────────
fn convex_polygon_20() -> Vec<(f64, f64)> {
    let cx = 128.0;
    let cy = 128.0;
    let base_r = 100.0;
    let mut verts = Vec::with_capacity(20);
    // Deterministic "random" perturbation via simple LCG
    let mut rng: u32 = 12345;
    for i in 0..20 {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let perturb = ((rng >> 16) as f64 / 65536.0) * 20.0 - 10.0;
        let angle = std::f64::consts::PI * 2.0 * i as f64 / 20.0;
        let r = base_r + perturb;
        verts.push((cx + r * angle.cos(), cy + r * angle.sin()));
    }
    verts
}

#[test]
fn painter_polygon_complex() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("polygon_complex");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_polygon(&convex_polygon_20(), Color::CYAN, Color::TRANSPARENT);
    }
    compare_images("polygon_complex", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 9: round_rect ─────────────────────────────────────────
#[test]
fn painter_round_rect() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("round_rect");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_round_rect(20.0, 20.0, 200.0, 150.0, 20.0, Color::BLUE);
    }
    compare_images("round_rect", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 10: gradient_h ────────────────────────────────────────
#[test]
fn painter_gradient_h() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("gradient_h");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_linear_gradient(
            0.0,
            0.0,
            256.0,
            256.0,
            Color::RED,
            Color::BLUE,
            true,
            Color::TRANSPARENT,
        );
    }
    compare_images("gradient_h", img.data(), &expected, ew, eh, 0, 1.0).unwrap();
}

// ─── Test 11: gradient_v ────────────────────────────────────────
#[test]
fn painter_gradient_v() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("gradient_v");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_linear_gradient(
            0.0,
            0.0,
            256.0,
            256.0,
            Color::GREEN,
            Color::YELLOW,
            false,
            Color::TRANSPARENT,
        );
    }
    compare_images("gradient_v", img.data(), &expected, ew, eh, 0, 1.0).unwrap();
}

// ─── Test 12: gradient_radial ───────────────────────────────────
#[test]
fn painter_gradient_radial() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("gradient_radial");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_radial_gradient(
            128.0,
            128.0,
            128.0,
            128.0,
            Color::WHITE,
            Color::BLACK,
            Color::TRANSPARENT,
        );
    }
    // Residual: C++ uses integer sqrt lookup table for gradient; Rust uses f64 sqrt.
    // max_diff=50 at polygon boundary AA, 25.08% of pixels differ at ch_tol=1.
    // Gradient interior: pixel-perfect (integer sqrt table matching C++).
    // Residual: ellipse polygon boundary AA (~0.05% pixels, structural).
    compare_images("gradient_radial", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 13: line_basic ────────────────────────────────────────
#[test]
fn painter_line_basic() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("line_basic");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_line_stroked(
            10.0,
            10.0,
            240.0,
            200.0,
            &Stroke::new(Color::BLACK, 3.0),
            Color::TRANSPARENT,
        );
    }
    compare_images("line_basic", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 14: line_thick ────────────────────────────────────────
#[test]
fn painter_line_thick() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("line_thick");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        let stroke = Stroke {
            color: Color::BLUE,
            width: 8.0,
            join: LineJoin::Round,
            cap: LineCap::Round,
            ..Default::default()
        };
        p.paint_line_stroked(10.0, 128.0, 240.0, 128.0, &stroke, Color::TRANSPARENT);
    }
    compare_images("line_thick", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 15: line_ends_all ─────────────────────────────────────
fn all_stroke_end_types() -> Vec<StrokeEndType> {
    vec![
        StrokeEndType::Butt,
        StrokeEndType::Cap,
        StrokeEndType::Arrow,
        StrokeEndType::ContourArrow,
        StrokeEndType::LineArrow,
        StrokeEndType::Triangle,
        StrokeEndType::ContourTriangle,
        StrokeEndType::Square,
        StrokeEndType::ContourSquare,
        StrokeEndType::HalfSquare,
        StrokeEndType::Circle,
        StrokeEndType::ContourCircle,
        StrokeEndType::HalfCircle,
        StrokeEndType::Diamond,
        StrokeEndType::ContourDiamond,
        StrokeEndType::HalfDiamond,
        StrokeEndType::Stroke,
    ]
}

#[test]
fn painter_line_ends_all() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("line_ends_all");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        let types = all_stroke_end_types();
        let spacing = 240.0 / types.len() as f64;
        for (i, end_type) in types.iter().enumerate() {
            let y = 8.0 + spacing * i as f64;
            let mut stroke = Stroke::new(Color::BLACK, 4.0);
            stroke.cap = LineCap::Round;
            stroke.join = LineJoin::Round;
            stroke.finish_end = StrokeEnd::new(*end_type).with_inner_color(Color::WHITE);
            p.paint_line_stroked(30.0, y, 226.0, y, &stroke, Color::TRANSPARENT);
        }
    }
    compare_images("line_ends_all", img.data(), &expected, ew, eh, 1, 1.0).unwrap();
}

// ─── Test 16: line_dashed ───────────────────────────────────────
#[test]
fn painter_line_dashed() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("line_dashed");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // Dashed line: C++ emDashedStroke(BLACK, 3.0, 3.0)
        let mut stroke_dash = Stroke::new(Color::BLACK, 3.0);
        stroke_dash.dash_type = DashType::Dashed;
        stroke_dash.dash_length_factor = 3.0;
        stroke_dash.gap_length_factor = 3.0;
        p.paint_line_stroked(10.0, 64.0, 240.0, 64.0, &stroke_dash, Color::TRANSPARENT);
        // Dotted line: C++ emDottedStroke(BLACK, 3.0)
        let mut stroke_dot = Stroke::new(Color::BLACK, 3.0);
        stroke_dot.dash_type = DashType::Dotted;
        stroke_dot.gap_length_factor = 3.0;
        p.paint_line_stroked(10.0, 128.0, 240.0, 128.0, &stroke_dot, Color::TRANSPARENT);
    }
    compare_images("line_dashed", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 17: outline_rect ──────────────────────────────────────
#[test]
fn painter_outline_rect() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("outline_rect");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_rect_outlined(
            20.0,
            20.0,
            200.0,
            150.0,
            &Stroke::new(Color::BLACK, 3.0),
            Color::TRANSPARENT,
        );
    }
    compare_images("outline_rect", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 18: outline_ellipse ───────────────────────────────────
#[test]
fn painter_outline_ellipse() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("outline_ellipse");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // C++ PaintEllipseOutline(28,28,200,150, 2.0, stroke) → cx=128 cy=103 rx=100 ry=75
        p.paint_ellipse_outlined(
            128.0,
            103.0,
            100.0,
            75.0,
            &Stroke::new(Color::BLACK, 2.0),
            Color::TRANSPARENT,
        );
    }
    compare_images("outline_ellipse", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 19: outline_polygon ───────────────────────────────────
fn pentagon_vertices() -> Vec<(f64, f64)> {
    let cx = 128.0;
    let cy = 128.0;
    let r = 100.0;
    (0..5)
        .map(|i| {
            let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * 2.0 * i as f64 / 5.0;
            (cx + r * angle.cos(), cy + r * angle.sin())
        })
        .collect()
}

#[test]
fn painter_outline_polygon() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("outline_polygon");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_polygon_outlined(&pentagon_vertices(), Color::BLACK, 3.0, Color::TRANSPARENT);
    }
    compare_images("outline_polygon", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 20: outline_round_rect ────────────────────────────────
#[test]
fn painter_outline_round_rect() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("outline_round_rect");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_round_rect_outlined(
            20.0,
            20.0,
            200.0,
            150.0,
            20.0,
            &Stroke::new(Color::BLACK, 3.0),
        );
    }
    // Residual: arc approximation segment count differs slightly from C++.
    // max_diff=162, 0.21% of pixels differ at ch_tol=1.
    compare_images("outline_round_rect", img.data(), &expected, ew, eh, 2, 0.5).unwrap();
}

// ─── Test 21: bezier_filled ─────────────────────────────────────
fn bezier_points() -> Vec<(f64, f64)> {
    // Single cubic bezier: 4 control points
    vec![(20.0, 200.0), (80.0, 20.0), (180.0, 20.0), (236.0, 200.0)]
}

#[test]
fn painter_bezier_filled() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("bezier_filled");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_bezier(&bezier_points(), Color::RED, Color::TRANSPARENT);
    }
    compare_images("bezier_filled", img.data(), &expected, ew, eh, 1, 0.1).unwrap();
}

// ─── Test 22: bezier_stroked ────────────────────────────────────
#[test]
fn painter_bezier_stroked() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("bezier_stroked");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        let mut stroke = Stroke::new(Color::BLACK, 3.0);
        stroke.cap = LineCap::Round;
        stroke.join = LineJoin::Round;
        stroke.start_end = StrokeEnd::new(StrokeEndType::Arrow).with_inner_color(Color::WHITE);
        stroke.finish_end = StrokeEnd::new(StrokeEndType::Arrow).with_inner_color(Color::WHITE);
        p.paint_bezier_line(&bezier_points(), &stroke, Color::TRANSPARENT);
    }
    compare_images("bezier_stroked", img.data(), &expected, ew, eh, 1, 1.0).unwrap();
}

// ─── Test 23: clip_basic ────────────────────────────────────────
#[test]
fn painter_clip_basic() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("clip_basic");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.clip_rect(64.0, 64.0, 128.0, 128.0);
        // Paint full-canvas polygon — only center rect should appear
        p.paint_polygon(
            &[(128.0, 10.0), (10.0, 246.0), (246.0, 246.0)],
            Color::RED,
            Color::TRANSPARENT,
        );
    }
    compare_images("clip_basic", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 24: canvas_color ──────────────────────────────────────
#[test]
fn painter_canvas_color() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("canvas_color");
    let mut img = Image::new(ew, eh, 4);
    img.fill(Color::grey(200));
    {
        let mut p = white_painter(&mut img);
        p.set_canvas_color(Color::grey(200));
        p.paint_rect(
            20.0,
            20.0,
            100.0,
            80.0,
            Color::rgba(255, 0, 0, 128),
            Color::TRANSPARENT,
        );
    }
    compare_images("canvas_color", img.data(), &expected, ew, eh, 0, 0.5).unwrap();
}

// ─── Test 25: image_paint ───────────────────────────────────────
fn procedural_image(w: u32, h: u32) -> Image {
    let mut img = Image::new(w, h, 4);
    for y in 0..h {
        for x in 0..w {
            let r = (x * 255 / w) as u8;
            let g = (y * 255 / h) as u8;
            let b = 128u8;
            let px = img.pixel_mut(x, y);
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = 255;
        }
    }
    img
}

#[test]
fn painter_image_paint() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("image_paint");
    let mut img = white_canvas(ew, eh);
    let src = procedural_image(64, 64);
    {
        let mut p = white_painter(&mut img);
        p.paint_image_full(50.0, 50.0, 64.0, 64.0, &src, 255, Color::TRANSPARENT);
    }
    compare_images("image_paint", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 26: image_scaled ──────────────────────────────────────
#[test]
fn painter_image_scaled() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("image_scaled");
    let mut img = white_canvas(ew, eh);
    let src = procedural_image(64, 64);
    {
        let mut p = white_painter(&mut img);
        p.paint_image_full(28.0, 28.0, 200.0, 200.0, &src, 255, Color::TRANSPARENT);
    }
    // Adaptive interpolation matches C++ UQ_ADAPTIVE; remaining ±1 diffs from
    // FP rounding in Hermite factor table computation.
    compare_images("image_scaled", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 27: multi_compose ─────────────────────────────────────
#[test]
fn painter_multi_compose() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("multi_compose");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // 5 overlapping shapes with varying alpha
        p.paint_rect(
            10.0,
            10.0,
            120.0,
            120.0,
            Color::rgba(255, 0, 0, 180),
            Color::TRANSPARENT,
        );
        p.paint_ellipse(
            100.0,
            60.0,
            80.0,
            80.0,
            Color::rgba(0, 255, 0, 150),
            Color::TRANSPARENT,
        );
        p.paint_polygon(
            &[(128.0, 10.0), (60.0, 200.0), (200.0, 200.0)],
            Color::rgba(0, 0, 255, 120),
            Color::TRANSPARENT,
        );
        p.paint_round_rect(
            140.0,
            80.0,
            100.0,
            100.0,
            15.0,
            Color::rgba(255, 255, 0, 100),
        );
        p.paint_rect(
            30.0,
            150.0,
            200.0,
            80.0,
            Color::rgba(128, 0, 128, 90),
            Color::TRANSPARENT,
        );
    }
    compare_images("multi_compose", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 28: polyline ──────────────────────────────────────────
#[test]
fn painter_polyline() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("polyline");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        let stroke = Stroke {
            color: Color::BLACK,
            width: 4.0,
            join: LineJoin::Round,
            cap: LineCap::Round,
            ..Default::default()
        };
        let verts = [(20.0, 200.0), (80.0, 40.0), (160.0, 200.0), (240.0, 40.0)];
        p.paint_solid_polyline(&verts, &stroke, false, Color::TRANSPARENT);
    }
    compare_images("polyline", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 30: transform_translate ───────────────────────────────
#[test]
fn painter_transform_translate() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_translate");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.translate(50.0, 30.0);
        p.paint_rect(0.0, 0.0, 80.0, 60.0, Color::RED, Color::TRANSPARENT);
        p.pop_state();
    }
    compare_images("transform_translate", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 35: transform_fractional ──────────────────────────────
#[test]
fn painter_transform_fractional() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_fractional");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.translate(0.3, 0.7);
        p.paint_rect(20.0, 20.0, 100.0, 80.0, Color::RED, Color::TRANSPARENT);
        p.pop_state();
    }
    compare_images(
        "transform_fractional",
        img.data(),
        &expected,
        ew,
        eh,
        1,
        0.5,
    )
    .unwrap();
}

// ─── Test 36: transform_identity_roundtrip ─────────────────────
#[test]
fn painter_transform_identity_roundtrip() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_identity_roundtrip");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.scale(2.0, 2.0);
        p.scale(0.5, 0.5);
        p.paint_rect(20.0, 20.0, 100.0, 80.0, Color::RED, Color::TRANSPARENT);
        p.pop_state();
    }
    compare_images(
        "transform_identity_roundtrip",
        img.data(),
        &expected,
        ew,
        eh,
        1,
        0.5,
    )
    .unwrap();
}

// ─── Test 34: transform_ellipse_scaled ──────────────────────────
#[test]
fn painter_transform_ellipse_scaled() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_ellipse_scaled");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.scale(2.0, 1.0);
        // C++ PaintEllipse(10,50,60,60) → bbox center (40,80), radius (30,30) in user space
        p.paint_ellipse(40.0, 80.0, 30.0, 30.0, Color::GREEN, Color::TRANSPARENT);
        p.pop_state();
    }
    compare_images(
        "transform_ellipse_scaled",
        img.data(),
        &expected,
        ew,
        eh,
        1,
        0.5,
    )
    .unwrap();
}

// ─── Test 37: text_basic ────────────────────────────────────────
#[test]
fn painter_text_basic() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("text_basic");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_text(
            10.0,
            80.0,
            "Hello",
            40.0,
            1.0,
            Color::BLACK,
            Color::TRANSPARENT,
        );
    }
    compare_images("text_basic", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 38: text_scaled ───────────────────────────────────────
#[test]
fn painter_text_scaled() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("text_scaled");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_text(
            10.0,
            80.0,
            "Test",
            40.0,
            1.5,
            Color::RED,
            Color::TRANSPARENT,
        );
    }
    compare_images("text_scaled", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 39: text_fitted ───────────────────────────────────────
#[test]
fn painter_text_fitted() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("text_fitted");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_text_boxed(
            20.0,
            20.0,
            216.0,
            80.0,
            "Fitted",
            100.0,
            Color::BLACK,
            Color::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            0.0,
        );
    }
    compare_images("text_fitted", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 40: text_alignment ────────────────────────────────────
#[test]
fn painter_text_alignment() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("text_alignment");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // Top-left box, left text
        p.paint_text_boxed(
            10.0,
            10.0,
            236.0,
            80.0,
            "Left",
            50.0,
            Color::BLACK,
            Color::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            0.5,
            true,
            0.0,
        );
        // Center box, center text
        p.paint_text_boxed(
            10.0,
            120.0,
            236.0,
            80.0,
            "Center",
            50.0,
            Color::BLACK,
            Color::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.0,
        );
        // Bottom-right box, right text
        p.paint_text_boxed(
            10.0,
            230.0,
            236.0,
            80.0,
            "Right",
            50.0,
            Color::BLACK,
            Color::TRANSPARENT,
            TextAlignment::Right,
            VAlign::Bottom,
            TextAlignment::Right,
            0.5,
            true,
            0.0,
        );
    }
    compare_images("text_alignment", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 41: text_clipped ──────────────────────────────────────
#[test]
fn painter_text_clipped() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("text_clipped");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.clip_rect(50.0, 50.0, 150.0, 150.0);
        p.paint_text(
            30.0,
            80.0,
            "Clipped!",
            40.0,
            1.0,
            Color::BLACK,
            Color::TRANSPARENT,
        );
        p.pop_state();
    }
    compare_images("text_clipped", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 42: text_below_threshold ──────────────────────────────
#[test]
fn painter_text_below_threshold() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("text_below_threshold");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.paint_text(
            10.0,
            100.0,
            "tiny text here",
            1.0,
            1.0,
            Color::BLACK,
            Color::TRANSPARENT,
        );
    }
    compare_images(
        "text_below_threshold",
        img.data(),
        &expected,
        ew,
        eh,
        1,
        0.5,
    )
    .unwrap();
}

// ─── Test 33: transform_clip_interaction ────────────────────────
#[test]
fn painter_transform_clip_interaction() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_clip_interaction");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.clip_rect(64.0, 64.0, 128.0, 128.0);
        p.translate(160.0, 100.0);
        p.paint_rect(0.0, 0.0, 80.0, 60.0, Color::RED, Color::TRANSPARENT);
        p.pop_state();
    }
    compare_images(
        "transform_clip_interaction",
        img.data(),
        &expected,
        ew,
        eh,
        1,
        0.5,
    )
    .unwrap();
}

// ─── Test 32: transform_nested ──────────────────────────────────
#[test]
fn painter_transform_nested() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_nested");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // Inner: translate(50,50) then scale(2,2), paint red rect
        p.push_state();
        p.translate(50.0, 50.0);
        p.push_state();
        p.scale(2.0, 2.0);
        p.paint_rect(0.0, 0.0, 30.0, 30.0, Color::RED, Color::TRANSPARENT);
        p.pop_state();
        // Outer: translate(50,50) only, paint blue rect
        p.paint_rect(
            0.0,
            0.0,
            50.0,
            50.0,
            Color::rgba(0, 0, 255, 128),
            Color::TRANSPARENT,
        );
        p.pop_state();
    }
    compare_images("transform_nested", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 31: transform_scale ───────────────────────────────────
#[test]
fn painter_transform_scale() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_scale");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        p.push_state();
        p.scale(2.0, 2.0);
        p.paint_rect(10.0, 10.0, 50.0, 40.0, Color::RED, Color::TRANSPARENT);
        p.pop_state();
    }
    compare_images("transform_scale", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}

// ─── Test 29: ellipse_sector ────────────────────────────────────
#[test]
fn painter_ellipse_sector() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("ellipse_sector");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // C++ PaintEllipseSector(28,28,200,200, 0, 90) → cx=128 cy=128 rx=100 ry=100
        // Start=0° (right), sweep=90° (down to bottom-right quadrant)
        p.paint_ellipse_sector(
            128.0,
            128.0,
            100.0,
            100.0,
            0.0,
            90.0,
            Color::RED,
            Color::TRANSPARENT,
        );
    }
    compare_images("ellipse_sector", img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}
