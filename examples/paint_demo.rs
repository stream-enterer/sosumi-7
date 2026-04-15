//! Paint demo derived from C++ `PaintExample.cpp` and `emTestPanel.cpp` PaintContent section.
//!
//! A single panel exercising every major emPainter drawing primitive:
//! images, rectangles, ellipses, polygons, lines, beziers, gradients,
//! textured polygons, clipping, stroke end types, and text rendering.

use std::f64::consts::PI;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emImage::emImage;
use eaglemode_rs::emCore::emPanel::{PanelBehavior, PanelState};
use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::{emPainter, TextAlignment, VAlign};

use eaglemode_rs::emCore::emStroke::{LineCap, LineJoin, emStroke};

use eaglemode_rs::emCore::emStrokeEnd::{emStrokeEnd, StrokeEndType};

use eaglemode_rs::emCore::emTexture::{ImageExtension, ImageQuality, emTexture};
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

struct PaintPanel {
    test_image: emImage,
}

impl PaintPanel {
    fn new() -> Self {
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

impl PanelBehavior for PaintPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        // Background
        p.PaintRect(0.0, 0.0, w, h, emColor::WHITE, emColor::TRANSPARENT);

        // ── Section 1: emImage ──
        p.paint_image_scaled(
            0.05 * w,
            0.05 * h,
            0.2 * w,
            0.2 * h,
            &self.test_image,
            ImageQuality::Bilinear,
            ImageExtension::Clamp,
        );

        // ── Section 2: Filled shapes ──
        p.PaintRect(
            0.3 * w,
            0.05 * h,
            0.15 * w,
            0.15 * h,
            emColor::GREEN,
            emColor::TRANSPARENT,
        );

        // Ellipse (center + radius)
        p.PaintEllipse(
            0.6 * w,
            0.15 * h,
            0.08 * w,
            0.08 * h,
            emColor::rgba(0x33, 0xCC, 0x88, 0xFF),
            emColor::TRANSPARENT,
        );

        // Triangle
        p.PaintPolygon(
            &[
                (0.05 * w, 0.35 * h),
                (0.25 * w, 0.45 * h),
                (0.02 * w, 0.55 * h),
            ],
            emColor::rgba(255, 128, 0, 255),
            emColor::TRANSPARENT,
        );

        // Round rect
        p.PaintRoundRect(
            0.3 * w,
            0.30 * h,
            0.15 * w,
            0.15 * h,
            0.02 * w,
            0.02 * w,
            emColor::rgba(0x88, 0x44, 0xCC, 0xFF),
        );

        // ── Section 3: Outlines ──
        let outline = emStroke::new(emColor::rgba(0x00, 0x80, 0xC0, 0xFF), 0.005 * w);
        p.PaintRectOutline(
            0.5 * w,
            0.30 * h,
            0.15 * w,
            0.15 * h,
            &outline,
            emColor::TRANSPARENT,
        );

        p.PaintEllipseOutline(
            0.77 * w,
            0.37 * h,
            0.08 * w,
            0.08 * h,
            &outline,
            emColor::TRANSPARENT,
        );

        p.PaintPolygonOutline(
            &[
                (0.05 * w, 0.60 * h),
                (0.15 * w, 0.58 * h),
                (0.20 * w, 0.68 * h),
                (0.10 * w, 0.72 * h),
            ],
            emColor::rgba(255, 0, 0, 255),
            0.003 * w,
            emColor::TRANSPARENT,
        );

        p.PaintRoundRectOutline(0.25 * w, 0.55 * h, 0.15 * w, 0.15 * h, 0.02 * w, 0.02 * w, &outline);

        // ── Section 4: Text ──
        p.PaintTextBoxed(
            0.50 * w,
            0.05 * h,
            0.25 * w,
            0.20 * h,
            "Centered text\nin\nthe bottom-right\nof a box",
            0.017 * w,
            emColor::rgba(0, 0x80, 0xC0, 0xFF),
            emColor::TRANSPARENT,
            TextAlignment::Right,
            VAlign::Bottom,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );

        p.PaintText(
            0.78 * w,
            0.05 * h,
            "paint_text()",
            0.02 * w,
            1.0,
            emColor::BLACK,
            emColor::TRANSPARENT,
        );

        // ── Section 5: Stroked line ──
        let mut stroke_line = emStroke::new(emColor::rgba(255, 0, 0, 128), 0.015 * w);
        stroke_line.cap = LineCap::Round;
        stroke_line.join = LineJoin::Round;
        stroke_line.start_end = emStrokeEnd::new(StrokeEndType::Cap);
        stroke_line.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
        p.paint_line_stroked(
            0.45 * w,
            0.55 * h,
            0.65 * w,
            0.72 * h,
            &stroke_line,
            emColor::TRANSPARENT,
        );

        // ── Section 6: Bezier curves ──
        let bezier_pts = [
            (0.70 * w, 0.55 * h),
            (0.60 * w, 0.60 * h),
            (0.80 * w, 0.65 * h),
            (0.70 * w, 0.72 * h),
        ];
        p.PaintBezier(
            &bezier_pts,
            emColor::rgba(0x00, 0xAA, 0x00, 0xFF),
            emColor::TRANSPARENT,
        );

        let bezier_stroke = emStroke::new(emColor::rgba(0xCC, 0x00, 0x88, 0xFF), 0.003 * w);
        p.PaintBezierOutline(&bezier_pts, &bezier_stroke, emColor::TRANSPARENT);

        let mut arrow_stroke = emStroke::new(emColor::rgba(0x00, 0x00, 0xFF, 0xFF), 0.004 * w);
        arrow_stroke.cap = LineCap::Round;
        arrow_stroke.join = LineJoin::Round;
        arrow_stroke.start_end =
            emStrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(emColor::RED);
        arrow_stroke.finish_end = emStrokeEnd::new(StrokeEndType::Arrow);
        let bezier_pts2 = [
            (0.82 * w, 0.55 * h),
            (0.92 * w, 0.60 * h),
            (0.82 * w, 0.65 * h),
            (0.92 * w, 0.72 * h),
        ];
        p.PaintBezierLine(&bezier_pts2, &arrow_stroke, emColor::TRANSPARENT);

        // ── Section 7: Gradients ──
        p.paint_linear_gradient(
            0.05 * w,
            0.78 * h,
            0.15 * w,
            0.08 * h,
            emColor::rgba(0, 0xFF, 0, 0x80),
            emColor::rgba(0xFF, 0xFF, 0, 0xFF),
            true,
            emColor::TRANSPARENT,
        );

        p.paint_radial_gradient(
            0.30 * w,
            0.82 * h,
            0.08 * w,
            0.06 * h,
            emColor::rgba(0xFF, 0x88, 0, 0xFF),
            emColor::rgba(0, 0x55, 0, 0xFF),
            emColor::TRANSPARENT,
        );

        // ── Section 8: Textured polygons ──
        let star = make_star(0.55 * w, 0.84 * h, 0.06 * w, 0.06 * h, 5);
        p.paint_polygon_textured(
            &star,
            &emTexture::LinearGradient {
                color_a: emColor::rgba(0, 0xFF, 0, 0x80),
                color_b: emColor::rgba(0xFF, 0xFF, 0, 0xFF),
                start: (0.49 * w, 0.78 * h),
                end: (0.61 * w, 0.90 * h),
            },
            emColor::TRANSPARENT,
        );

        let star2 = make_star(0.70 * w, 0.84 * h, 0.06 * w, 0.06 * h, 5);
        p.paint_polygon_textured(
            &star2,
            &emTexture::RadialGradient {
                color_inner: emColor::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: emColor::rgba(0, 0, 0xFF, 0x60),
                center: (0.70 * w, 0.84 * h),
                radius: 0.06 * w,
            },
            emColor::TRANSPARENT,
        );

        let star3 = make_star(0.85 * w, 0.84 * h, 0.06 * w, 0.06 * h, 5);
        p.paint_polygon_textured(
            &star3,
            &emTexture::emImage {
                image: self.test_image.clone(),
                extension: ImageExtension::Repeat,
                quality: ImageQuality::Bilinear,
            },
            emColor::TRANSPARENT,
        );

        // ── Section 9: Clipping demo ──
        p.push_state();
        p.SetClipping(0.05 * w, 0.88 * h, 0.15 * w, 0.10 * h);
        // Draw a circle that extends beyond the clip rectangle
        let verts: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * 2.0 * i as f64 / 64.0;
                (
                    a.cos() * 0.10 * w + 0.125 * w,
                    a.sin() * 0.10 * h + 0.93 * h,
                )
            })
            .collect();
        p.PaintPolygon(&verts, emColor::rgba(255, 255, 0, 180), emColor::TRANSPARENT);
        p.pop_state();

        // ── Section 10: All 17 StrokeEndType variants ──
        let end_types = [
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
            StrokeEndType::emStroke,
        ];
        let n = end_types.len();
        let center_x = 0.45 * w;
        let center_y = 0.92 * h;
        let inner_r = 0.01 * w;
        let outer_r = 0.04 * w;
        for (i, &et) in end_types.iter().enumerate() {
            for side in 0..2u32 {
                let idx = i * 2 + side as usize;
                let a = 2.0 * PI * idx as f64 / (2 * n) as f64;
                let mut s = emStroke::new(emColor::WHITE, 0.002 * w);
                if side == 1 {
                    s.cap = LineCap::Round;
                    s.join = LineJoin::Round;
                }
                s.start_end = emStrokeEnd::new(StrokeEndType::Cap);
                s.finish_end =
                    emStrokeEnd::new(et).with_inner_color(emColor::rgba(0xFF, 0xFF, 0xFF, 0x40));
                p.paint_line_stroked(
                    center_x + inner_r * a.cos(),
                    center_y + inner_r * a.sin(),
                    center_x + outer_r * a.cos(),
                    center_y + outer_r * a.sin(),
                    &s,
                    emColor::TRANSPARENT,
                );
            }
        }
    }
}

/// Generate a star polygon with the given number of points.
fn make_star(cx: f64, cy: f64, rx: f64, ry: f64, points: usize) -> Vec<(f64, f64)> {
    let mut verts = Vec::with_capacity(points * 2);
    for i in 0..(points * 2) {
        let a = PI * i as f64 / points as f64 - PI / 2.0;
        let r = if i % 2 == 0 { 1.0 } else { 0.4 };
        verts.push((cx + a.cos() * rx * r, cy + a.sin() * ry * r));
    }
    verts
}

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let root = app.tree.create_root("root");
        app.tree.set_behavior(root, Box::new(PaintPanel::new()));
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let close_sig = app.scheduler.borrow_mut().create_signal();
        let flags_sig = app.scheduler.borrow_mut().create_signal();
        let focus_sig = app.scheduler.borrow_mut().create_signal();
        let geometry_sig = app.scheduler.borrow_mut().create_signal();
        let win = eaglemode_rs::emCore::emWindow::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
            flags_sig,
            focus_sig,
            geometry_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        app.windows.get_mut(&wid).unwrap().view_mut().flags |= ViewFlags::ROOT_SAME_TALLNESS;
    }));
    app.run();
}
