#[allow(unused)]
pub mod scaled;

use std::f64::consts::PI;

use emcore::emColor::emColor;

use emcore::emImage::emImage;
use emcore::emPanel::{PanelBehavior, PanelState};

use emcore::emPanelTree::{PanelId, PanelTree};

use emcore::emPainter::emPainter;
use emcore::emView::{emView, ViewFlags};

use emcore::emStroke::{emStroke, LineCap, LineJoin};

use emcore::emStrokeEnd::{emStrokeEnd, StrokeEndType};

use emcore::emTexture::{emTexture, ImageExtension, ImageQuality};

use emcore::emViewRendererTileCache::{TileCache, TILE_SIZE};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const DEFAULT_VW: u32 = 1920;
pub const DEFAULT_VH: u32 = 1080;

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

pub struct Scenario {
    pub name: &'static str,
    pub dx: f64,
    pub dy: f64,
    pub dz: f64,
}

pub const SCENARIOS: &[Scenario] = &[
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

// ---------------------------------------------------------------------------
// TestPanel (verbatim from examples/bench_interaction.rs)
// ---------------------------------------------------------------------------

pub struct TestPanel {
    test_image: emImage,
}

impl TestPanel {
    pub fn new() -> Self {
        let mut img = emImage::new(64, 64, 4);
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
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        if state.viewed_rect.w < 25.0 {
            return;
        }

        painter.push_state();
        painter.scale(w, w);
        let h = h / w;

        let fg = emColor::SetGrey(136);
        let bg = emColor::rgba(0x00, 0x1C, 0x38, 0xFF);

        painter.PaintRect(0.0, 0.0, 1.0, h, bg, emColor::TRANSPARENT);
        painter.PaintRectOutline(
            0.01,
            0.01,
            1.0 - 0.02,
            h - 0.02,
            &emStroke::new(fg, 0.02),
            emColor::TRANSPARENT,
        );

        let _state_str = format!(
            "State: InFocusedPath ViewFocused Pri={:.3} MemLim={}",
            state.priority, state.memory_limit,
        );
        painter.PaintRect(
            0.25,
            0.8,
            0.05,
            0.05,
            emColor::rgba(255, 0, 0, 32),
            emColor::TRANSPARENT,
        );

        painter.PaintPolygon(
            &[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)],
            fg,
            emColor::TRANSPARENT,
        );
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
            emColor::rgba(255, 255, 255, 128),
            emColor::TRANSPARENT,
        );
        painter.PaintPolygon(
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
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        let circle: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.PaintPolygon(
            &circle,
            emColor::rgba(255, 255, 0, 255),
            emColor::TRANSPARENT,
        );

        let clipped: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.push_state();
        painter.SetClipping(0.51, 0.81, 0.08, 0.08);
        painter.PaintPolygon(
            &clipped,
            emColor::rgba(0, 255, 0, 255),
            emColor::TRANSPARENT,
        );
        painter.pop_state();

        let ellipse: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        painter.PaintPolygon(&ellipse, emColor::rgba(255, 0, 0, 92), emColor::TRANSPARENT);

        painter.PaintPolygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            emColor::TRANSPARENT,
        );
        painter.PaintPolygon(
            &[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)],
            emColor::RED,
            emColor::TRANSPARENT,
        );
        painter.PaintPolygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            emColor::TRANSPARENT,
        );
        painter.PaintPolygon(
            &[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)],
            emColor::RED,
            emColor::TRANSPARENT,
        );

        painter.PaintPolygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            emColor::rgba(187, 136, 255, 192),
            emColor::TRANSPARENT,
        );
        painter.PaintPolygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            emColor::rgba(136, 187, 255, 192),
            emColor::TRANSPARENT,
        );
        painter.PaintPolygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            emColor::rgba(136, 187, 255, 192),
            emColor::TRANSPARENT,
        );

        painter.PaintEllipse(0.05, 0.80, 0.01, 0.01, emColor::WHITE, emColor::TRANSPARENT);
        painter.PaintEllipse(0.06, 0.80, 0.02, 0.01, emColor::WHITE, emColor::TRANSPARENT);
        painter.PaintEllipse(
            0.09,
            0.80,
            0.005,
            0.01,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        painter.PaintEllipseSector(
            0.10,
            0.80,
            0.01,
            0.01,
            45.0,
            305.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseSector(
            0.11,
            0.80,
            0.02,
            0.01,
            -350.0,
            395.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseSector(
            0.13,
            0.80,
            0.005,
            0.01,
            245.0,
            50.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseSector(
            0.14,
            0.80,
            0.01,
            0.01,
            195.0,
            50.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        painter.PaintRectOutline(
            0.05,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        let mut sd = emStroke::new(emColor::WHITE, 0.001);
        sd.dash_pattern = vec![0.002, 0.001];
        painter.PaintRectOutline(0.07, 0.82, 0.02, 0.01, &sd, emColor::TRANSPARENT);
        painter.PaintRectOutline(
            0.10,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.008),
            emColor::TRANSPARENT,
        );
        painter.PaintRectOutline(
            0.13,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.011),
            emColor::TRANSPARENT,
        );

        painter.PaintRoundRect(
            0.05,
            0.84,
            0.01,
            0.01,
            0.001,
            0.001,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRect(
            0.07,
            0.84,
            0.02,
            0.01,
            0.002,
            0.002,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRect(
            0.10,
            0.84,
            0.01,
            0.01,
            0.003,
            0.003,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRect(
            0.13,
            0.84,
            0.01,
            0.01,
            0.006,
            0.006,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRect(
            0.15,
            0.84,
            0.01,
            0.01,
            0.0,
            0.0,
            emColor::WHITE,
            emColor::TRANSPARENT,
        );

        painter.PaintEllipseOutline(
            0.05,
            0.86,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.003),
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseOutline(
            0.065,
            0.86,
            0.02,
            0.01,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        let mut dot_s = emStroke::new(emColor::WHITE, 0.00025);
        dot_s.join = LineJoin::Round;
        dot_s.cap = LineCap::Round;
        dot_s.dash_pattern = vec![0.0001, 0.0005];
        painter.PaintEllipseOutline(0.09, 0.86, 0.005, 0.01, &dot_s, emColor::TRANSPARENT);

        painter.PaintEllipseArc(
            0.10,
            0.86,
            0.01,
            0.01,
            90.0,
            225.0,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseSectorOutline(
            0.11,
            0.86,
            0.02,
            0.01,
            45.0,
            -365.0,
            &emStroke::new(emColor::WHITE, 0.0001),
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseArc(
            0.13,
            0.86,
            0.005,
            0.01,
            245.0,
            295.0,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        painter.PaintEllipseArc(
            0.14,
            0.86,
            0.01,
            0.01,
            195.0,
            245.0,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        let mut rs = emStroke::new(emColor::WHITE, 0.0001);
        rs.join = LineJoin::Round;
        rs.cap = LineCap::Round;
        rs.start_end = emStrokeEnd::new(StrokeEndType::Cap);
        rs.finish_end = emStrokeEnd::new(StrokeEndType::LineArrow);
        painter.PaintEllipseArc(
            0.15,
            0.86,
            0.01,
            0.01,
            0.0,
            -145.0,
            &rs,
            emColor::TRANSPARENT,
        );

        painter.PaintRoundRectOutline(
            0.05,
            0.88,
            0.01,
            0.01,
            0.001,
            0.001,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRectOutline(
            0.07,
            0.88,
            0.02,
            0.01,
            0.002,
            0.002,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRectOutline(
            0.10,
            0.88,
            0.01,
            0.01,
            0.003,
            0.003,
            &emStroke::new(emColor::WHITE, 0.003),
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRectOutline(
            0.12,
            0.88,
            0.01,
            0.01,
            0.006,
            0.006,
            &emStroke::new(emColor::WHITE, 0.0001),
            emColor::TRANSPARENT,
        );
        let mut dds = emStroke::new(emColor::WHITE, 0.00002);
        dds.dash_pattern = vec![0.0001, 0.00005, 0.00003, 0.00005];
        painter.PaintRoundRectOutline(
            0.135,
            0.88,
            0.01,
            0.01,
            0.001,
            0.001,
            &dds,
            emColor::TRANSPARENT,
        );
        painter.PaintRoundRectOutline(
            0.15,
            0.88,
            0.01,
            0.01,
            0.0,
            0.0,
            &emStroke::new(emColor::WHITE, 0.001),
            emColor::TRANSPARENT,
        );

        painter.PaintBezier(
            &[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)],
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        painter.PaintBezier(
            &[
                (0.065, 0.91),
                (0.05, 0.902),
                (0.058, 0.89),
                (0.065, 0.900),
                (0.072, 0.89),
                (0.08, 0.902),
            ],
            emColor::WHITE,
            emColor::TRANSPARENT,
        );
        let mut rd = emStroke::new(emColor::WHITE, 0.0002);
        rd.join = LineJoin::Round;
        rd.cap = LineCap::Round;
        rd.dash_pattern = vec![0.001, 0.0005];
        painter.PaintBezierOutline(
            &[
                (0.085, 0.91),
                (0.07, 0.902),
                (0.078, 0.89),
                (0.085, 0.900),
                (0.092, 0.89),
                (0.10, 0.902),
            ],
            &rd,
            emColor::TRANSPARENT,
        );
        let mut bls = emStroke::new(emColor::WHITE, 0.0002);
        bls.join = LineJoin::Round;
        bls.cap = LineCap::Round;
        bls.dash_pattern = vec![0.001, 0.0005];
        bls.start_end =
            emStrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(emColor::RED);
        bls.finish_end = emStrokeEnd::new(StrokeEndType::Arrow);
        painter.PaintBezierLine(
            &[(0.105, 0.91), (0.09, 0.902), (0.098, 0.89), (0.105, 0.900)],
            &bls,
            emColor::TRANSPARENT,
        );

        let n = 17usize;
        for i in 0..(2 * n) {
            let a = 2.0 * PI * i as f64 / (2 * n) as f64;
            let mut ls = emStroke::new(emColor::WHITE, 0.0001);
            if i & 1 != 0 {
                ls.join = LineJoin::Round;
                ls.cap = LineCap::Round;
            }
            ls.start_end = emStrokeEnd::new(StrokeEndType::Cap);
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
                _ => StrokeEndType::emStroke,
            };
            ls.finish_end =
                emStrokeEnd::new(end_type).with_inner_color(emColor::rgba(0xFF, 0xFF, 0xFF, 0x40));
            painter.paint_line_stroked(
                0.117 + 0.002 * a.cos(),
                0.903 + 0.002 * a.sin(),
                0.117 + 0.0075 * a.cos(),
                0.903 + 0.0075 * a.sin(),
                &ls,
                emColor::TRANSPARENT,
            );
        }

        let mut ps = emStroke::new(emColor::WHITE, 0.0005);
        ps.join = LineJoin::Round;
        ps.cap = LineCap::Round;
        ps.start_end =
            emStrokeEnd::new(StrokeEndType::ContourArrow).with_inner_color(emColor::TRANSPARENT);
        ps.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
        painter.PaintSolidPolyline(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &ps,
            false,
            emColor::TRANSPARENT,
        );

        painter.PaintPolygonOutline(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            emColor::RED,
            0.0002,
            emColor::TRANSPARENT,
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
            &emTexture::LinearGradient {
                color_a: emColor::rgba(0, 255, 0, 128),
                color_b: emColor::rgba(255, 255, 0, 255),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
            emColor::TRANSPARENT,
        );
        painter.paint_polygon_textured(
            &star(0.220),
            &emTexture::RadialGradient {
                color_inner: emColor::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: emColor::rgba(0, 0, 0xFF, 0x60),
                center: (0.235, 0.918),
                radius_x: 0.04,
                radius_y: 0.04,
            },
            emColor::TRANSPARENT,
        );
        painter.paint_polygon_textured(
            &star(0.240),
            &emTexture::emImage {
                image: self.test_image.clone(),
                x: 0.0,
                y: 0.0,
                w: 0.002,
                h: 0.002,
                alpha: 255,
                extension: ImageExtension::Clamp,
                quality: ImageQuality::Bilinear,
            },
            emColor::TRANSPARENT,
        );

        painter.paint_linear_gradient(
            0.2,
            0.94,
            0.02,
            0.01,
            emColor::rgba(0, 0, 0, 128),
            emColor::rgba(128, 128, 128, 128),
            true,
            emColor::TRANSPARENT,
        );
        painter.paint_radial_gradient(
            0.225,
            0.945,
            0.004,
            0.005,
            emColor::rgba(255, 136, 0, 255),
            emColor::rgba(0, 85, 0, 255),
            emColor::TRANSPARENT,
        );

        let eg: Vec<_> = (0..64)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / 64.0;
                (0.24 + 0.01 * a.cos(), 0.945 + 0.005 * a.sin())
            })
            .collect();
        painter.paint_polygon_textured(
            &eg,
            &emTexture::RadialGradient {
                color_inner: emColor::TRANSPARENT,
                color_outer: emColor::rgba(0, 204, 136, 255),
                center: (0.24, 0.945),
                radius_x: 0.01,
                radius_y: 0.01,
            },
            emColor::TRANSPARENT,
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

    fn IsOpaque(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

pub fn setup_tree_and_view(vw: u32, vh: u32) -> (PanelTree, emView, PanelId) {
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("bench_root");
    tree.set_behavior(root, Box::new(TestPanel::new()));
    let tallness = vh as f64 / vw as f64;
    tree.Layout(root, 0.0, 0.0, 1.0, tallness, 1.0);
    tree.set_focusable(root, true);

    let mut view = emView::new(
        emcore::emContext::emContext::NewRoot(),
        root,
        vw as f64,
        vh as f64,
    );
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    // SP5: HandleNotice is now driven from emView::Update internally.
    view.Update(&mut tree);

    (tree, view, root)
}

/// Execute one complete frame Cycle without timing instrumentation.
pub fn run_one_frame(
    tree: &mut PanelTree,
    view: &mut emView,
    viewport_buf: &mut emImage,
    tile_cache: &mut TileCache,
    scenario: &Scenario,
    fix_x: f64,
    fix_y: f64,
) {
    let (cols, rows) = tile_cache.grid_size();

    // 1. Scroll/zoom
    view.RawScrollAndZoom(tree, fix_x, fix_y, scenario.dx, scenario.dy, scenario.dz);

    // 2+3. emView::Update now drives HandleNotice internally (SP5).
    view.Update(tree);

    // 4. Paint
    viewport_buf.fill(emColor::BLACK);
    {
        let mut painter = emPainter::new(viewport_buf);
        view.Paint(tree, &mut painter, emColor::TRANSPARENT);
    }

    // 5. Tile copy
    for row in 0..rows {
        for col in 0..cols {
            let tile = tile_cache.get_or_create(col, row);
            tile.image.copy_from_rect(
                0,
                0,
                viewport_buf,
                (col * TILE_SIZE, row * TILE_SIZE, TILE_SIZE, TILE_SIZE),
            );
        }
    }

    // 6. Frame cleanup
    view.clear_viewport_changed();
}
