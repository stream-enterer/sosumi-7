//! Benchmark PaintContent cost at various zoom depths.
//!
//! Pre-zooms to several levels, then measures PaintContent cost at each.
//! Tests whether clipping effectively limits work when most of the
//! panel is off-screen.
//!
//! Run: cargo run --release --example bench_zoom_depth

use std::time::Instant;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emImage::emImage;
use eaglemode_rs::emCore::emPanel::{PanelBehavior, PanelState};
use eaglemode_rs::emCore::emPanelTree::PanelTree;
use eaglemode_rs::emCore::emView::{emView, ViewFlags};
use eaglemode_rs::emCore::emPainter::emPainter;

// Reuse the same TestPanel from bench_interaction
struct TestPanel {
    test_image: emImage,
}

impl TestPanel {
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

impl PanelBehavior for TestPanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        use std::f64::consts::PI;
        use eaglemode_rs::emCore::emStroke::emStroke;

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
            0.01, 0.01, 1.0 - 0.02, h - 0.02,
            &emStroke::new(fg, 0.02), emColor::TRANSPARENT,
        );

        painter.PaintRect(0.25, 0.8, 0.05, 0.05, emColor::rgba(255, 0, 0, 32), emColor::TRANSPARENT);

        painter.PaintPolygon(
            &[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)],
            fg, emColor::TRANSPARENT,
        );

        let circle: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        painter.PaintPolygon(&circle, emColor::rgba(255, 255, 0, 255), emColor::TRANSPARENT);

        painter.PaintEllipse(0.055, 0.805, 0.005, 0.005, emColor::WHITE, emColor::TRANSPARENT);
        painter.PaintEllipse(0.07, 0.805, 0.01, 0.005, emColor::WHITE, emColor::TRANSPARENT);

        painter.PaintRoundRect(0.05, 0.84, 0.01, 0.01, 0.001, 0.001, emColor::WHITE);
        painter.PaintRoundRect(0.07, 0.84, 0.02, 0.01, 0.002, 0.002, emColor::WHITE);

        painter.PaintEllipseOutline(
            0.055, 0.865, 0.005, 0.005,
            &emStroke::new(emColor::WHITE, 0.003), emColor::TRANSPARENT,
        );

        painter.PaintRoundRectOutline(
            0.05, 0.88, 0.01, 0.01, 0.001, 0.001,
            &emStroke::new(emColor::WHITE, 0.001),
        );

        painter.paint_image_scaled(
            0.26, 0.94, 0.02, 0.01,
            &self.test_image,
            eaglemode_rs::emCore::emTexture::ImageQuality::Bilinear,
            eaglemode_rs::emCore::emTexture::ImageExtension::Clamp,
        );

        painter.pop_state();
    }

    fn IsOpaque(&self) -> bool {
        true
    }
}

const VW: u32 = 1920;
const VH: u32 = 1080;
const FRAMES: usize = 60;

fn measure_at_zoom(zoom_factor: f64) -> (f64, f64, f64) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("bench_root");
    tree.set_behavior(root, Box::new(TestPanel::new()));
    let tallness = VH as f64 / VW as f64;
    tree.Layout(root, 0.0, 0.0, 1.0, tallness);
    tree.set_focusable(root, true);

    let core_config = std::rc::Rc::new(std::cell::RefCell::new(
        eaglemode_rs::emCore::emCoreConfig::emCoreConfig::default(),
    ));
    let mut view = emView::new(root, VW as f64, VH as f64, core_config);
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    tree.HandleNotice(true, 1.0);
    view.update(&mut tree);

    // Zoom in by the specified factor, centered on viewport center
    let cx = VW as f64 / 2.0;
    let cy = VH as f64 / 2.0;
    // Apply zoom in steps to avoid numerical issues
    let steps = 100;
    let per_step = zoom_factor.powf(1.0 / steps as f64);
    for _ in 0..steps {
        view.Zoom(per_step, cx, cy);
    }
    tree.HandleNotice(true, 1.0);
    view.update(&mut tree);
    view.clear_viewport_changed();

    let mut buf = emImage::new(VW, VH, 4);

    // Warmup
    buf.fill(emColor::BLACK);
    {
        let mut painter = emPainter::new(&mut buf);
        view.Paint(&mut tree, &mut painter);
    }

    // Measure
    let mut times = Vec::with_capacity(FRAMES);
    for _ in 0..FRAMES {
        buf.fill(emColor::BLACK);
        let t = Instant::now();
        {
            let mut painter = emPainter::new(&mut buf);
            view.Paint(&mut tree, &mut painter);
        }
        times.push(t.elapsed().as_micros() as f64);
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];
    let p99 = times[(times.len() as f64 * 0.99) as usize];
    let max = *times.last().unwrap();
    (median, p99, max)
}

fn measure_nested_at_zoom(panel_count: usize, zoom_factor: f64) -> (f64, f64, f64) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    let tallness = VH as f64 / VW as f64;
    tree.Layout(root, 0.0, 0.0, 1.0, tallness);
    tree.set_behavior(root, Box::new(TestPanel::new()));
    tree.set_focusable(root, true);

    // Build nested children: each child fills most of the GetParentContext,
    // like Eagle Mode's recursive zoom structure
    if panel_count > 1 {
        let mut parents = vec![root];
        let mut created = 1usize;
        let branching = 4usize;

        while created < panel_count {
            let mut next_parents = Vec::new();
            for &GetParentContext in &parents {
                for child_idx in 0..branching {
                    if created >= panel_count {
                        break;
                    }
                    let child = tree.create_child(GetParentContext, &format!("p{created}"));
                    let siblings = branching.min(panel_count - created + child_idx);
                    let x = child_idx as f64 / siblings as f64;
                    let w = 1.0 / siblings as f64;
                    tree.Layout(child, x, 0.0, w, 1.0);
                    tree.set_behavior(child, Box::new(TestPanel::new()));
                    next_parents.push(child);
                    created += 1;
                }
            }
            parents = next_parents;
        }
    }

    let core_config = std::rc::Rc::new(std::cell::RefCell::new(
        eaglemode_rs::emCore::emCoreConfig::emCoreConfig::default(),
    ));
    let mut view = emView::new(root, VW as f64, VH as f64, core_config);
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    tree.HandleNotice(true, 1.0);
    view.update(&mut tree);

    // Zoom in
    let cx = VW as f64 / 2.0;
    let cy = VH as f64 / 2.0;
    let steps = 100;
    let per_step = zoom_factor.powf(1.0 / steps as f64);
    for _ in 0..steps {
        view.Zoom(per_step, cx, cy);
    }
    tree.HandleNotice(true, 1.0);
    view.update(&mut tree);
    view.clear_viewport_changed();

    let mut buf = emImage::new(VW, VH, 4);

    // Warmup
    buf.fill(emColor::BLACK);
    {
        let mut painter = emPainter::new(&mut buf);
        view.Paint(&mut tree, &mut painter);
    }

    // Measure
    let mut times = Vec::with_capacity(FRAMES);
    for _ in 0..FRAMES {
        buf.fill(emColor::BLACK);
        let t = Instant::now();
        {
            let mut painter = emPainter::new(&mut buf);
            view.Paint(&mut tree, &mut painter);
        }
        times.push(t.elapsed().as_micros() as f64);
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];
    let p99 = times[(times.len() as f64 * 0.99) as usize];
    let max = *times.last().unwrap();
    (median, p99, max)
}

fn main() {
    println!("=== bench_zoom_depth ({VW}x{VH}, {FRAMES} frames/level) ===");
    println!();

    // Single panel at various zoom levels
    println!("--- Single Panel ---");
    println!("{:<12} {:>10} {:>10} {:>10}", "Zoom", "Median", "p99", "Max");
    println!("{:<12} {:>10} {:>10} {:>10}", "----", "------", "---", "---");
    for &zoom in &[1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 1000.0] {
        let (median, p99, max) = measure_at_zoom(zoom);
        println!(
            "{:<12} {:>9.0}us {:>9.0}us {:>9.0}us",
            format!("{zoom}x"), median, p99, max,
        );
    }

    // Nested panels (50 panels) at various zoom levels
    println!();
    println!("--- 50 Nested Panels ---");
    println!("{:<12} {:>10} {:>10} {:>10}", "Zoom", "Median", "p99", "Max");
    println!("{:<12} {:>10} {:>10} {:>10}", "----", "------", "---", "---");
    for &zoom in &[1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 1000.0] {
        let (median, p99, max) = measure_nested_at_zoom(50, zoom);
        println!(
            "{:<12} {:>9.0}us {:>9.0}us {:>9.0}us",
            format!("{zoom}x"), median, p99, max,
        );
    }

    // 200 panels at various zoom levels
    println!();
    println!("--- 200 Nested Panels ---");
    println!("{:<12} {:>10} {:>10} {:>10}", "Zoom", "Median", "p99", "Max");
    println!("{:<12} {:>10} {:>10} {:>10}", "----", "------", "---", "---");
    for &zoom in &[1.0, 2.0, 5.0, 10.0, 50.0, 100.0, 1000.0] {
        let (median, p99, max) = measure_nested_at_zoom(200, zoom);
        println!(
            "{:<12} {:>9.0}us {:>9.0}us {:>9.0}us",
            format!("{zoom}x"), median, p99, max,
        );
    }
}
