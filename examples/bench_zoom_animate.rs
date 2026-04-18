//! Benchmark continuous zoom animation — simulates the actual zoom
//! animation loop to find frame-time spikes (choppiness).
//!
//! Run: cargo run --release --example bench_zoom_animate

use std::time::Instant;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emImage::emImage;
use eaglemode_rs::emCore::emPanel::{PanelBehavior, PanelState};
use eaglemode_rs::emCore::emPanelTree::PanelTree;
use eaglemode_rs::emCore::emView::{emView, ViewFlags};
use eaglemode_rs::emCore::emPainter::emPainter;
use eaglemode_rs::emCore::emStroke::emStroke;
use eaglemode_rs::emCore::emViewRendererTileCache::{TileCache, TILE_SIZE};

const VW: u32 = 1920;
const VH: u32 = 1080;

// Panel with moderate complexity (shapes, not just color fill)
struct GamePanel {
    test_image: emImage,
}

impl GamePanel {
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

impl PanelBehavior for GamePanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        use std::f64::consts::PI;

        if state.viewed_rect.w < 25.0 {
            return;
        }

        painter.push_state();
        painter.scale(w, w);
        let h = h / w;

        let bg = emColor::rgba(0x00, 0x1C, 0x38, 0xFF);
        let fg = emColor::SetGrey(136);

        painter.PaintRect(0.0, 0.0, 1.0, h, bg, emColor::TRANSPARENT);
        painter.PaintRectOutline(
            0.01, 0.01, 1.0 - 0.02, h - 0.02,
            &emStroke::new(fg, 0.02), emColor::TRANSPARENT,
        );

        // Several polygons
        for i in 0..5 {
            let ox = 0.1 + i as f64 * 0.15;
            painter.PaintPolygon(
                &[(ox, 0.3), (ox + 0.1, 0.3), (ox + 0.05, 0.5)],
                emColor::rgba(100 + i * 30, 50, 200 - i * 30, 200),
                emColor::TRANSPARENT,
            );
        }

        // Ellipses
        for i in 0..4 {
            let cx = 0.15 + i as f64 * 0.2;
            painter.PaintEllipse(cx, 0.7, 0.05, 0.03, emColor::WHITE, emColor::TRANSPARENT);
        }

        // Circle
        let circle: Vec<_> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.08 + 0.5, a.cos() * 0.08 + 0.85)
            })
            .collect();
        painter.PaintPolygon(&circle, emColor::rgba(255, 255, 0, 180), emColor::TRANSPARENT);

        // emImage
        painter.paint_image_scaled(
            0.3, 0.1, 0.1, 0.1,
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

fn build_tree(panel_count: usize) -> (PanelTree, emView) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    let tallness = VH as f64 / VW as f64;
    tree.Layout(root, 0.0, 0.0, 1.0, tallness);
    tree.set_behavior(root, Box::new(GamePanel::new()));
    tree.set_focusable(root, true);

    if panel_count > 1 {
        let mut parents = vec![root];
        let mut created = 1usize;
        let branching = 4;
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
                    tree.set_behavior(child, Box::new(GamePanel::new()));
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
    view.clear_viewport_changed();

    (tree, view)
}

fn simulate_zoom_animation(panel_count: usize, start_zoom: f64, zoom_speed: f64, frames: usize) {
    let (mut tree, mut view) = build_tree(panel_count);

    let mut buf = emImage::new(VW, VH, 4);
    let mut tc = TileCache::new(VW, VH, 256);
    let (cols, rows) = tc.grid_size();

    let cx = VW as f64 / 2.0;
    let cy = VH as f64 / 2.0;

    // Pre-zoom to start level
    let steps = 200;
    let per_step = start_zoom.powf(1.0 / steps as f64);
    for _ in 0..steps {
        view.Zoom(per_step, cx, cy);
    }
    tree.HandleNotice(true, 1.0);
    view.update(&mut tree);
    view.clear_viewport_changed();

    // Warmup frame
    buf.fill(emColor::BLACK);
    {
        let mut painter = emPainter::new(&mut buf);
        view.Paint(&mut tree, &mut painter);
    }

    // Simulate continuous zoom animation
    let dt = 1.0 / 60.0;
    let mut frame_times = Vec::with_capacity(frames);

    for _ in 0..frames {
        let frame_start = Instant::now();

        // 1. Zoom step (simulates animator)
        let zflpp = view.GetZoomFactorLogarithmPerPixel();
        let re_fac = (-zoom_speed * dt * zflpp).exp();
        let area_factor = 1.0 / (re_fac * re_fac);
        view.Zoom(area_factor, cx, cy);

        // 2. Update
        tree.HandleNotice(true, dt);
        view.update(&mut tree);
        view.clear_viewport_changed();

        // 3. Paint (full viewport, simulating mark_all_dirty)
        buf.fill(emColor::BLACK);
        {
            let mut painter = emPainter::new(&mut buf);
            view.Paint(&mut tree, &mut painter);
        }

        // 4. Tile copy
        for row in 0..rows {
            for col in 0..cols {
                let tile = tc.get_or_create(col, row);
                tile.image.copy_from_rect(
                    0, 0, &buf,
                    (col * TILE_SIZE, row * TILE_SIZE, TILE_SIZE, TILE_SIZE),
                );
            }
        }

        let elapsed = frame_start.elapsed().as_micros() as u64;
        frame_times.push(elapsed);
    }

    // Report
    let mut sorted = frame_times.clone();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    let p99 = sorted[(sorted.len() as f64 * 0.99) as usize];
    let max = *sorted.last().unwrap();
    let over_budget = frame_times.iter().filter(|&&t| t > 16600).count();

    // Find jank: frames that are >2x the median
    let jank_threshold = median * 2;
    let jank_frames: Vec<(usize, u64)> = frame_times
        .iter()
        .enumerate()
        .filter(|(_, &t)| t > jank_threshold)
        .map(|(i, &t)| (i, t))
        .collect();

    println!(
        "  {:<8} start={:<6} median={:>5}us  p99={:>5}us  max={:>5}us  over_budget={}/{}",
        format!("{}p", panel_count),
        format!("{:.0}x", start_zoom),
        median, p99, max, over_budget, frames,
    );
    if !jank_frames.is_empty() {
        let first_five: Vec<_> = jank_frames.iter().take(5).collect();
        print!("           jank frames (>{jank_threshold}us): ");
        for (i, t) in &first_five {
            print!("#{i}={t}us ");
        }
        if jank_frames.len() > 5 {
            print!("... ({} total)", jank_frames.len());
        }
        println!();
    }
}

fn main() {
    println!("=== bench_zoom_animate ({VW}x{VH}) ===");
    println!("  Simulates continuous zoom-in animation at various starting depths.");
    println!();

    // Various panel counts and starting zoom levels
    for &panels in &[1, 10, 50, 200] {
        for &start in &[1.0, 10.0, 100.0, 1000.0] {
            simulate_zoom_animation(panels, start, 300.0, 240);
        }
        println!();
    }
}
