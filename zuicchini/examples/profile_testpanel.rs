//! Headless profiling harness for the TestPanel paint path.
//! Run with:
//!   cargo run --release --example profile_testpanel
//! or under samply:
//!   samply record -- cargo run --release --example profile_testpanel

use std::time::Instant;

use zuicchini::foundation::Color;
use zuicchini::foundation::Image;
use zuicchini::panel::{PanelBehavior, PanelState, PanelTree, ViewFlags};
use zuicchini::render::{
    ImageExtension, ImageQuality, LineCap, LineJoin, Painter, Stroke, StrokeEnd, StrokeEndType,
    Texture, TileCache, TILE_SIZE,
};

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Inline a minimal TestPanel that exercises the same paint primitives
// ---------------------------------------------------------------------------

struct TestPanel {
    test_image: Image,
}

impl TestPanel {
    fn new() -> Self {
        let mut img = Image::new(64, 64, 4);
        for y in 0..64u32 {
            for x in 0..64u32 {
                img.set_pixel_channel(x, y, 0, (x * 4) as u8);
                img.set_pixel_channel(x, y, 1, (y * 4) as u8);
                img.set_pixel_channel(x, y, 2, 128);
                img.set_pixel_channel(x, y, 3, 255);
            }
        }
        Self { test_image: img }
    }
}

impl PanelBehavior for TestPanel {
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        if state.viewed_rect.w < 25.0 {
            return;
        }

        painter.push_state();
        painter.scale(w, w);
        let h = h / w;

        let fg = Color::grey(136);
        let bg = Color::rgba(0x00, 0x1C, 0x38, 0xFF);

        // Background + border
        painter.paint_rect(0.0, 0.0, 1.0, h, bg);
        painter.paint_rect_outlined(0.01, 0.01, 1.0 - 0.02, h - 0.02, &Stroke::new(fg, 0.02));

        // TODO(font): paint text here

        // TODO(font): paint text here

        // TODO(font): paint text here

        // --- All the paint_primitives from the real TestPanel ---

        // TODO(font): paint text here
        painter.paint_rect(0.25, 0.8, 0.05, 0.05, Color::rgba(255, 0, 0, 32));

        // Polygons
        painter.paint_polygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg);
        painter.paint_polygon_even_odd(
            &[
                (0.90, 0.90),
                (0.94, 0.90),
                (0.94, 0.94),
                (0.90, 0.94),
                (0.90, 0.90),
                (0.91, 0.91),
                (0.93, 0.91),
                (0.93, 0.93),
                (0.91, 0.93),
                (0.91, 0.91),
            ],
            Color::rgba(255, 255, 255, 128),
        );
        painter.paint_polygon(
            &[
                (0.80, 0.90),
                (0.84, 0.90),
                (0.84, 0.94),
                (0.80, 0.94),
                (0.80, 0.90),
                (0.81, 0.91),
                (0.81, 0.93),
                (0.83, 0.93),
                (0.83, 0.91),
                (0.81, 0.91),
            ],
            Color::WHITE,
        );

        // Circle (64-sided polygon)
        let circle: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.paint_polygon(&circle, Color::rgba(255, 255, 0, 255));

        // Clipped circle
        let clipped: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.push_state();
        painter.clip_rect(0.51, 0.81, 0.08, 0.08);
        painter.paint_polygon(&clipped, Color::rgba(0, 255, 0, 255));
        painter.pop_state();

        // Ellipse
        let ellipse: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        painter.paint_polygon(&ellipse, Color::rgba(255, 0, 0, 92));

        // Adjacent triangles
        painter.paint_polygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            Color::rgba(187, 255, 255, 255),
        );
        painter.paint_polygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], Color::RED);
        painter.paint_polygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            Color::rgba(187, 255, 255, 255),
        );
        painter.paint_polygon(&[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)], Color::RED);

        // Thin polygons
        painter.paint_polygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            Color::rgba(187, 136, 255, 192),
        );
        painter.paint_polygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            Color::rgba(136, 187, 255, 192),
        );
        painter.paint_polygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            Color::rgba(136, 187, 255, 192),
        );

        // Ellipses
        painter.paint_ellipse(0.055, 0.805, 0.005, 0.005, Color::WHITE);
        painter.paint_ellipse(0.07, 0.805, 0.01, 0.005, Color::WHITE);
        painter.paint_ellipse(0.0925, 0.805, 0.0025, 0.005, Color::WHITE);

        // Ellipse sectors
        let deg = PI / 180.0;
        painter.paint_ellipse_sector(
            0.105,
            0.805,
            0.005,
            0.005,
            45.0 * deg,
            350.0 * deg,
            Color::WHITE,
        );
        painter.paint_ellipse_sector(
            0.12,
            0.805,
            0.01,
            0.005,
            -350.0 * deg,
            45.0 * deg,
            Color::WHITE,
        );
        painter.paint_ellipse_sector(
            0.1325,
            0.805,
            0.0025,
            0.005,
            245.0 * deg,
            295.0 * deg,
            Color::WHITE,
        );
        painter.paint_ellipse_sector(
            0.145,
            0.805,
            0.005,
            0.005,
            195.0 * deg,
            245.0 * deg,
            Color::WHITE,
        );

        // Rect outlines
        painter.paint_rect_outlined(0.05, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.001));
        let mut sd = Stroke::new(Color::WHITE, 0.001);
        sd.dash_pattern = vec![0.002, 0.001];
        painter.paint_rect_outlined(0.07, 0.82, 0.02, 0.01, &sd);
        painter.paint_rect_outlined(0.10, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.008));
        painter.paint_rect_outlined(0.13, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.011));

        // Round rects
        painter.paint_round_rect(0.05, 0.84, 0.01, 0.01, 0.001, Color::WHITE);
        painter.paint_round_rect(0.07, 0.84, 0.02, 0.01, 0.002, Color::WHITE);
        painter.paint_round_rect(0.10, 0.84, 0.01, 0.01, 0.003, Color::WHITE);
        painter.paint_round_rect(0.13, 0.84, 0.01, 0.01, 0.006, Color::WHITE);
        painter.paint_round_rect(0.15, 0.84, 0.01, 0.01, 0.0, Color::WHITE);

        // Ellipse outlines
        painter.paint_ellipse_outlined(
            0.055,
            0.865,
            0.005,
            0.005,
            &Stroke::new(Color::WHITE, 0.003),
        );
        painter.paint_ellipse_outlined(
            0.075,
            0.865,
            0.01,
            0.005,
            &Stroke::new(Color::WHITE, 0.001),
        );
        let mut dot_s = Stroke::new(Color::WHITE, 0.00025);
        dot_s.join = LineJoin::Round;
        dot_s.cap = LineCap::Round;
        dot_s.dash_pattern = vec![0.0001, 0.0005];
        painter.paint_ellipse_outlined(0.0925, 0.865, 0.0025, 0.005, &dot_s);

        // Ellipse arcs
        painter.paint_ellipse_arc(
            0.105,
            0.865,
            0.005,
            0.005,
            90.0 * deg,
            225.0 * deg,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_ellipse_sector_outlined(
            0.12,
            0.865,
            0.01,
            0.005,
            45.0 * deg,
            -320.0 * deg,
            &Stroke::new(Color::WHITE, 0.0001),
        );
        painter.paint_ellipse_arc(
            0.1325,
            0.865,
            0.0025,
            0.005,
            245.0 * deg,
            295.0 * deg,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_ellipse_arc(
            0.145,
            0.865,
            0.005,
            0.005,
            195.0 * deg,
            245.0 * deg,
            &Stroke::new(Color::WHITE, 0.001),
        );
        let mut rs = Stroke::new(Color::WHITE, 0.0001);
        rs.join = LineJoin::Round;
        rs.cap = LineCap::Round;
        rs.start_end = StrokeEnd::new(StrokeEndType::Cap);
        rs.finish_end = StrokeEnd::new(StrokeEndType::LineArrow);
        painter.paint_ellipse_arc(0.155, 0.865, 0.005, 0.005, 0.0, -145.0 * deg, &rs);

        // Round rect outlines
        painter.paint_round_rect_outlined(
            0.05,
            0.88,
            0.01,
            0.01,
            0.001,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_round_rect_outlined(
            0.07,
            0.88,
            0.02,
            0.01,
            0.002,
            &Stroke::new(Color::WHITE, 0.001),
        );
        painter.paint_round_rect_outlined(
            0.10,
            0.88,
            0.01,
            0.01,
            0.003,
            &Stroke::new(Color::WHITE, 0.003),
        );
        painter.paint_round_rect_outlined(
            0.12,
            0.88,
            0.01,
            0.01,
            0.006,
            &Stroke::new(Color::WHITE, 0.0001),
        );
        let mut dds = Stroke::new(Color::WHITE, 0.00002);
        dds.dash_pattern = vec![0.0001, 0.00005, 0.00003, 0.00005];
        painter.paint_round_rect_outlined(0.135, 0.88, 0.01, 0.01, 0.001, &dds);
        painter.paint_round_rect_outlined(
            0.15,
            0.88,
            0.01,
            0.01,
            0.0,
            &Stroke::new(Color::WHITE, 0.001),
        );

        // Bezier curves
        painter.paint_bezier(&[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)], Color::WHITE);
        painter.paint_bezier(
            &[
                (0.065, 0.91),
                (0.05, 0.902),
                (0.058, 0.89),
                (0.065, 0.900),
                (0.072, 0.89),
                (0.08, 0.902),
            ],
            Color::WHITE,
        );
        let mut rd = Stroke::new(Color::WHITE, 0.0002);
        rd.join = LineJoin::Round;
        rd.cap = LineCap::Round;
        rd.dash_pattern = vec![0.001, 0.0005];
        painter.paint_bezier_outline(
            &[
                (0.085, 0.91),
                (0.07, 0.902),
                (0.078, 0.89),
                (0.085, 0.900),
                (0.092, 0.89),
                (0.10, 0.902),
            ],
            &rd,
        );
        let mut bls = Stroke::new(Color::WHITE, 0.0002);
        bls.join = LineJoin::Round;
        bls.cap = LineCap::Round;
        bls.dash_pattern = vec![0.001, 0.0005];
        bls.start_end = StrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(Color::RED);
        bls.finish_end = StrokeEnd::new(StrokeEndType::Arrow);
        painter.paint_bezier_line(
            &[(0.105, 0.91), (0.09, 0.902), (0.098, 0.89), (0.105, 0.900)],
            &bls,
        );

        // Stroke end types (34 lines)
        let n = 17usize;
        for i in 0..(2 * n) {
            let a = 2.0 * PI * i as f64 / (2 * n) as f64;
            let mut ls = Stroke::new(Color::WHITE, 0.0001);
            if i & 1 != 0 {
                ls.join = LineJoin::Round;
                ls.cap = LineCap::Round;
            }
            ls.start_end = StrokeEnd::new(StrokeEndType::Cap);
            let end_type = match i / 2 {
                0 => StrokeEndType::Butt,
                1 => StrokeEndType::Cap,
                2 => StrokeEndType::Arrow,
                3 => StrokeEndType::ContourArrow,
                4 => StrokeEndType::LineArrow,
                5 => StrokeEndType::Triangle,
                6 => StrokeEndType::ContourTriangle,
                7 => StrokeEndType::Square,
                8 => StrokeEndType::ContourSquare,
                9 => StrokeEndType::HalfSquare,
                10 => StrokeEndType::Circle,
                11 => StrokeEndType::ContourCircle,
                12 => StrokeEndType::HalfCircle,
                13 => StrokeEndType::Diamond,
                14 => StrokeEndType::ContourDiamond,
                15 => StrokeEndType::HalfDiamond,
                _ => StrokeEndType::Stroke,
            };
            ls.finish_end =
                StrokeEnd::new(end_type).with_inner_color(Color::rgba(0xFF, 0xFF, 0xFF, 0x40));
            painter.paint_line_stroked(
                0.117 + 0.002 * a.cos(),
                0.903 + 0.002 * a.sin(),
                0.117 + 0.0075 * a.cos(),
                0.903 + 0.0075 * a.sin(),
                &ls,
            );
        }

        // Polyline with stroke ends
        let mut ps = Stroke::new(Color::WHITE, 0.0005);
        ps.join = LineJoin::Round;
        ps.cap = LineCap::Round;
        ps.start_end =
            StrokeEnd::new(StrokeEndType::ContourArrow).with_inner_color(Color::TRANSPARENT);
        ps.finish_end = StrokeEnd::new(StrokeEndType::Cap);
        painter.paint_solid_polyline(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &ps,
            false,
        );

        // Polygon outline
        painter.paint_polygon_outlined(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            Color::RED,
            0.0002,
        );

        // Textured polygons
        let star = |ox: f64| -> Vec<(f64, f64)> {
            vec![
                (ox, 0.905),
                (ox + 0.015, 0.912),
                (ox + 0.030, 0.900),
                (ox + 0.022, 0.915),
                (ox + 0.030, 0.930),
                (ox + 0.020, 0.922),
                (ox + 0.005, 0.935),
                (ox + 0.012, 0.920),
            ]
        };
        painter.paint_polygon_textured(
            &star(0.200),
            &Texture::LinearGradient {
                color_a: Color::rgba(0, 255, 0, 128),
                color_b: Color::rgba(255, 255, 0, 255),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
        );
        painter.paint_polygon_textured(
            &star(0.220),
            &Texture::RadialGradient {
                color_inner: Color::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: Color::rgba(0, 0, 0xFF, 0x60),
                center: (0.235, 0.918),
                radius: 0.04,
            },
        );
        painter.paint_polygon_textured(
            &star(0.240),
            &Texture::Image {
                image: self.test_image.clone(),
                extension: ImageExtension::Clamp,
                quality: ImageQuality::Bilinear,
            },
        );

        // Gradient/image rects
        painter.paint_linear_gradient(
            0.2,
            0.94,
            0.02,
            0.01,
            Color::rgba(0, 0, 0, 128),
            Color::rgba(128, 128, 128, 128),
            true,
        );
        painter.paint_radial_gradient(
            0.225,
            0.945,
            0.004,
            0.005,
            Color::rgba(255, 136, 0, 255),
            Color::rgba(0, 85, 0, 255),
        );

        // Ellipse with radial gradient
        let eg: Vec<_> = (0..64)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / 64.0;
                (0.24 + 0.01 * a.cos(), 0.945 + 0.005 * a.sin())
            })
            .collect();
        painter.paint_polygon_textured(
            &eg,
            &Texture::RadialGradient {
                color_inner: Color::TRANSPARENT,
                color_outer: Color::rgba(0, 204, 136, 255),
                center: (0.24, 0.945),
                radius: 0.01,
            },
        );

        // Image rects
        painter.paint_image_scaled(
            0.26,
            0.94,
            0.02,
            0.01,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Clamp,
        );
        painter.paint_image_scaled(
            0.275,
            0.907,
            0.002,
            0.002,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Repeat,
        );
        painter.paint_image_scaled(
            0.275,
            0.910,
            0.002,
            0.002,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Clamp,
        );
        painter.paint_image_scaled(
            0.275,
            0.913,
            0.002,
            0.002,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Zero,
        );

        painter.pop_state();
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

fn main() {
    let iterations: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    let vw: u32 = 1920;
    let vh: u32 = 1080;

    // Setup
    let mut tree = PanelTree::new();
    let root = tree.create_root("test_panel_root");
    tree.set_behavior(root, Box::new(TestPanel::new()));
    let tallness = vh as f64 / vw as f64;
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, tallness);
    tree.set_focusable(root, true);

    let mut view = zuicchini::panel::View::new(root, vw as f64, vh as f64);
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    tree.deliver_notices(true);
    view.update(&mut tree);

    let mut tile_cache = TileCache::new(vw, vh, 256);

    // Warmup
    let (cols, rows) = tile_cache.grid_size();
    for row in 0..rows {
        for col in 0..cols {
            let tile = tile_cache.get_or_create(col, row);
            tile.image.fill(Color::BLACK);
            let mut painter = Painter::new(&mut tile.image);
            painter.translate(
                -(col as f64 * TILE_SIZE as f64),
                -(row as f64 * TILE_SIZE as f64),
            );
            view.paint(&mut tree, &mut painter);
        }
    }

    // --- Mode 1: Per-tile painting (current approach) ---
    let t0 = Instant::now();
    for _ in 0..iterations {
        for row in 0..rows {
            for col in 0..cols {
                let tile = tile_cache.get_or_create(col, row);
                tile.image.fill(Color::BLACK);
                let mut painter = Painter::new(&mut tile.image);
                painter.translate(
                    -(col as f64 * TILE_SIZE as f64),
                    -(row as f64 * TILE_SIZE as f64),
                );
                view.paint(&mut tree, &mut painter);
            }
        }
    }
    let per_tile = t0.elapsed();

    // --- Mode 2: Single-buffer painting (viewport-sized buffer, copy to tiles) ---
    let mut viewport_buf = Image::new(vw, vh, 4);
    // Warmup
    viewport_buf.fill(Color::BLACK);
    {
        let mut painter = Painter::new(&mut viewport_buf);
        view.paint(&mut tree, &mut painter);
    }

    let t0 = Instant::now();
    for _ in 0..iterations {
        viewport_buf.fill(Color::BLACK);
        {
            let mut painter = Painter::new(&mut viewport_buf);
            view.paint(&mut tree, &mut painter);
        }
        // Copy to tiles (simulates the upload path)
        for row in 0..rows {
            for col in 0..cols {
                let tile = tile_cache.get_or_create(col, row);
                tile.image.copy_from_rect(
                    0,
                    0,
                    &viewport_buf,
                    (col * TILE_SIZE, row * TILE_SIZE, TILE_SIZE, TILE_SIZE),
                );
            }
        }
    }
    let single_buf = t0.elapsed();

    println!("=== TestPanel Profile ({vw}x{vh}, {iterations} frames, {cols}x{rows} tiles) ===");
    println!(
        "  Per-tile:    {:>8.2?}/frame  (total {:>8.2?})",
        per_tile / iterations as u32,
        per_tile
    );
    println!(
        "  Single-buf:  {:>8.2?}/frame  (total {:>8.2?})",
        single_buf / iterations as u32,
        single_buf
    );
    let speedup = per_tile.as_secs_f64() / single_buf.as_secs_f64();
    println!("  Speedup:     {speedup:.2}x");
}
