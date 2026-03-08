//! Interactive pan/zoom benchmark for the TestPanel paint path.
//!
//! Exercises the full per-frame pipeline (scroll/zoom, notices, update, paint,
//! tile copy) across five scenarios to reveal costs that the static
//! `profile_testpanel` benchmark cannot.
//!
//! Run with:
//!   cargo run --release --example bench_interaction [width height]
//!
//! Default resolution: 1920x1080

use std::f64::consts::PI;
use std::time::Instant;

use zuicchini::foundation::{Color, Image};
use zuicchini::panel::{PanelBehavior, PanelState, PanelTree, View, ViewFlags};
use zuicchini::render::{
    ImageExtension, ImageQuality, LineCap, LineJoin, Painter, Stroke, StrokeEnd, StrokeEndType,
    Texture, TileCache, TILE_SIZE,
};

// ---------------------------------------------------------------------------
// Inline TestPanel (same as profile_testpanel.rs)
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

        painter.paint_rect(0.0, 0.0, 1.0, h, bg);
        painter.paint_rect_outlined(0.01, 0.01, 1.0 - 0.02, h - 0.02, &Stroke::new(fg, 0.02));

        // TODO(font): paint text here
        // TODO(font): paint text here
        let _state_str = format!(
            "State: InFocusedPath ViewFocused Pri={:.3} MemLim={}",
            state.priority, state.memory_limit,
        );
        // TODO(font): paint text here
        // TODO(font): paint text here
        painter.paint_rect(0.25, 0.8, 0.05, 0.05, Color::rgba(255, 0, 0, 32));

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

        let circle: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.paint_polygon(&circle, Color::rgba(255, 255, 0, 255));

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

        let ellipse: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        painter.paint_polygon(&ellipse, Color::rgba(255, 0, 0, 92));

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

        painter.paint_ellipse(0.055, 0.805, 0.005, 0.005, Color::WHITE);
        painter.paint_ellipse(0.07, 0.805, 0.01, 0.005, Color::WHITE);
        painter.paint_ellipse(0.0925, 0.805, 0.0025, 0.005, Color::WHITE);

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

        painter.paint_rect_outlined(0.05, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.001));
        let mut sd = Stroke::new(Color::WHITE, 0.001);
        sd.dash_pattern = vec![0.002, 0.001];
        painter.paint_rect_outlined(0.07, 0.82, 0.02, 0.01, &sd);
        painter.paint_rect_outlined(0.10, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.008));
        painter.paint_rect_outlined(0.13, 0.82, 0.01, 0.01, &Stroke::new(Color::WHITE, 0.011));

        painter.paint_round_rect(0.05, 0.84, 0.01, 0.01, 0.001, Color::WHITE);
        painter.paint_round_rect(0.07, 0.84, 0.02, 0.01, 0.002, Color::WHITE);
        painter.paint_round_rect(0.10, 0.84, 0.01, 0.01, 0.003, Color::WHITE);
        painter.paint_round_rect(0.13, 0.84, 0.01, 0.01, 0.006, Color::WHITE);
        painter.paint_round_rect(0.15, 0.84, 0.01, 0.01, 0.0, Color::WHITE);

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

        painter.paint_polygon_outlined(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            Color::RED,
            0.0002,
        );

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

// ---------------------------------------------------------------------------
// Timing infrastructure
// ---------------------------------------------------------------------------

struct FrameTiming {
    notices_us: u64,
    update_us: u64,
    paint_us: u64,
    tile_copy_us: u64,
    total_us: u64,
}

struct Scenario {
    name: &'static str,
    dx: f64,
    dy: f64,
    dz: f64,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "Static",
        dx: 0.0,
        dy: 0.0,
        dz: 0.0,
    },
    Scenario {
        name: "Pan",
        dx: 5.0,
        dy: 0.0,
        dz: 0.0,
    },
    Scenario {
        name: "Zoom In",
        dx: 0.0,
        dy: 0.0,
        dz: 0.02,
    },
    Scenario {
        name: "Zoom Out",
        dx: 0.0,
        dy: 0.0,
        dz: -0.02,
    },
    Scenario {
        name: "Pan+Zoom",
        dx: 3.0,
        dy: 0.0,
        dz: 0.015,
    },
];

const FRAMES_PER_SCENARIO: usize = 120;

fn setup_tree_and_view(vw: u32, vh: u32) -> (PanelTree, View, zuicchini::panel::PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("bench_root");
    tree.set_behavior(root, Box::new(TestPanel::new()));
    let tallness = vh as f64 / vw as f64;
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, tallness);
    tree.set_focusable(root, true);

    let mut view = View::new(root, vw as f64, vh as f64);
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    tree.deliver_notices(true);
    view.update(&mut tree);

    (tree, view, root)
}

fn run_scenario(scenario: &Scenario, vw: u32, vh: u32) -> (Vec<FrameTiming>, usize) {
    let (mut tree, mut view, _root) = setup_tree_and_view(vw, vh);

    let mut viewport_buf = Image::new(vw, vh, 4);
    let mut tile_cache = TileCache::new(vw, vh, 256);
    let (cols, rows) = tile_cache.grid_size();

    // Warmup: one full frame so caches are primed
    viewport_buf.fill(Color::BLACK);
    {
        let mut painter = Painter::new(&mut viewport_buf);
        view.paint(&mut tree, &mut painter);
    }
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
    view.clear_viewport_changed();

    let fix_x = vw as f64 / 2.0;
    let fix_y = vh as f64 / 2.0;

    let mut timings = Vec::with_capacity(FRAMES_PER_SCENARIO);

    for _ in 0..FRAMES_PER_SCENARIO {
        let frame_start = Instant::now();

        // 1. Scroll/zoom (not timed — simulates user input arriving before the frame)
        view.raw_scroll_and_zoom(
            &mut tree,
            fix_x,
            fix_y,
            scenario.dx,
            scenario.dy,
            scenario.dz,
        );

        // 2. Notices
        let t = Instant::now();
        tree.deliver_notices(true);
        let notices_us = t.elapsed().as_micros() as u64;

        // 3. View update
        let t = Instant::now();
        view.update(&mut tree);
        let update_us = t.elapsed().as_micros() as u64;

        // 4. Paint (single-buffer path)
        let t = Instant::now();
        viewport_buf.fill(Color::BLACK);
        {
            let mut painter = Painter::new(&mut viewport_buf);
            view.paint(&mut tree, &mut painter);
        }
        let paint_us = t.elapsed().as_micros() as u64;

        // 5. Tile copy
        let t = Instant::now();
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
        let tile_copy_us = t.elapsed().as_micros() as u64;

        // 6. Frame cleanup
        view.clear_viewport_changed();

        let total_us = frame_start.elapsed().as_micros() as u64;
        timings.push(FrameTiming {
            notices_us,
            update_us,
            paint_us,
            tile_copy_us,
            total_us,
        });
    }

    (timings, 0)
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn print_stat_line(label: &str, values: &mut [u64]) {
    values.sort_unstable();
    let min = values.first().copied().unwrap_or(0);
    let median = percentile(values, 0.5);
    let p99 = percentile(values, 0.99);
    let max = values.last().copied().unwrap_or(0);
    println!(
        "  {:<14} min={:>5}us  median={:>5}us  p99={:>5}us  max={:>5}us",
        label, min, median, p99, max,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let vw: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1920);
    let vh: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1080);

    let tile_cache = TileCache::new(vw, vh, 256);
    let (cols, rows) = tile_cache.grid_size();

    println!(
        "=== bench_interaction ({vw}x{vh}, {} frames/scenario, {cols}x{rows} tiles) ===",
        FRAMES_PER_SCENARIO,
    );
    println!();

    for scenario in SCENARIOS {
        let (timings, font_size_count) = run_scenario(scenario, vw, vh);

        let mut notices: Vec<u64> = timings.iter().map(|t| t.notices_us).collect();
        let mut update: Vec<u64> = timings.iter().map(|t| t.update_us).collect();
        let mut paint: Vec<u64> = timings.iter().map(|t| t.paint_us).collect();
        let mut tile_copy: Vec<u64> = timings.iter().map(|t| t.tile_copy_us).collect();
        let mut total: Vec<u64> = timings.iter().map(|t| t.total_us).collect();

        println!("--- {} ---", scenario.name);
        print_stat_line("notices:", &mut notices);
        print_stat_line("view.update:", &mut update);
        print_stat_line("paint:", &mut paint);
        print_stat_line("tile_copy:", &mut tile_copy);
        print_stat_line("TOTAL:", &mut total);
        println!("  Font sizes: {} distinct quantized sizes", font_size_count);
        println!();
    }
}
