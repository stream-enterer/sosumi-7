//! Headless profiling harness — exercises the hot paths (init, layout, paint)
//! without opening a window. Run with:
//!   cargo run --release --example profile_hotpaths
//! or under samply:
//!   samply record -- cargo run --release --example profile_hotpaths

use std::time::Instant;

use zuicchini::foundation::Color;
use zuicchini::layout::pack::PackLayout;
use zuicchini::layout::ChildConstraint;
use zuicchini::panel::{PanelBehavior, PanelTree, ViewFlags};
use zuicchini::render::{Painter, TileCache, TILE_SIZE};
use zuicchini::widget::{Border, Look, OuterBorderType};

struct BorderPanel {
    border: Border,
    look: Look,
}

impl PanelBehavior for BorderPanel {
    fn paint(
        &mut self,
        painter: &mut Painter,
        w: f64,
        h: f64,
        _state: &zuicchini::panel::PanelState,
    ) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);
    }

    fn is_opaque(&self) -> bool {
        true
    }
}

fn build_tree(panel_count: usize) -> (PanelTree, zuicchini::panel::PanelId) {
    use rand::Rng;
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    let mut layout = PackLayout::new();
    let mut rng = rand::rng();

    for i in 0..panel_count {
        let weight: f64 = rng.random_range(1.0..100.0);
        let pct: f64 = rng.random_range(-2.5_f64..2.5).exp();
        let hue: u32 = rng.random_range(0..360);
        let color = Color::from_hsv(hue as f32, 0.5, 0.5);
        let look = Look {
            bg_color: color,
            ..Look::default()
        };
        let caption = format!("{pct:.4}");
        let border = Border::new(OuterBorderType::Filled).with_caption(&caption);
        let child = tree.create_child(root, &format!("{i:06}"));
        tree.set_behavior(child, Box::new(BorderPanel { border, look }));
        layout.set_child_constraint(
            child,
            ChildConstraint {
                weight,
                preferred_tallness: pct,
                ..Default::default()
            },
        );
    }
    tree.set_behavior(root, Box::new(layout));
    (tree, root)
}

fn main() {
    let panel_count: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let iterations: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    let vw: u32 = 1920;
    let vh: u32 = 1080;

    // ── Phase 1: Tree construction ──
    let t0 = Instant::now();
    let (mut tree, root) = build_tree(panel_count);
    let t_build = t0.elapsed();

    let t_font = std::time::Duration::ZERO;

    // ── Phase 3: View creation ──
    let t0 = Instant::now();
    let mut view = zuicchini::panel::View::new(root, vw as f64, vh as f64);
    view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
    let t_view = t0.elapsed();

    // ── Phase 4: Initial layout (set_layout_rect + deliver_notices) ──
    let t0 = Instant::now();
    let tallness = vh as f64 / vw as f64;
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, tallness);
    tree.deliver_notices(true);
    let t_layout = t0.elapsed();

    // ── Phase 5: View update (update_viewing) ──
    let t0 = Instant::now();
    view.update(&mut tree);
    let t_update = t0.elapsed();

    // ── Phase 6: Tile cache creation ──
    let t0 = Instant::now();
    let mut tile_cache = TileCache::new(vw, vh, 256);
    let t_tilecache = t0.elapsed();

    // ── Phase 7: Full paint pass ──
    let t0 = Instant::now();
    let (cols, rows) = tile_cache.grid_size();
    for row in 0..rows {
        for col in 0..cols {
            let tile = tile_cache.get_or_create(col, row);
            tile.image.fill(Color::BLACK);
            let mut painter = Painter::new(&mut tile.image);
            let tile_size = TILE_SIZE as f64;
            painter.translate(-(col as f64 * tile_size), -(row as f64 * tile_size));
            view.paint(&mut tree, &mut painter);
            tile.dirty = false;
        }
    }
    let t_paint = t0.elapsed();

    // ── Phase 8: Simulated resize loop (relayout + repaint) ──
    let t0 = Instant::now();
    for i in 0..iterations {
        // Simulate resize: vary height slightly
        let new_h = vh + (i as u32 % 100);
        let new_tallness = new_h as f64 / vw as f64;
        tree.set_layout_rect(root, 0.0, 0.0, 1.0, new_tallness);
        tree.deliver_notices(true);
        view.set_viewport(vw as f64, new_h as f64);
        view.update(&mut tree);

        tile_cache.resize(vw, new_h);
        let (cols, rows) = tile_cache.grid_size();
        for row in 0..rows {
            for col in 0..cols {
                let tile = tile_cache.get_or_create(col, row);
                tile.image.fill(Color::BLACK);
                let mut painter = Painter::new(&mut tile.image);
                let tile_size = TILE_SIZE as f64;
                painter.translate(-(col as f64 * tile_size), -(row as f64 * tile_size));
                view.paint(&mut tree, &mut painter);
                tile.dirty = false;
            }
        }
    }
    let t_resize = t0.elapsed();

    println!(
        "=== Profile Results ({panel_count} panels, {vw}x{vh}, {iterations} resize iters) ==="
    );
    println!("  Tree build:     {:>8.2?}", t_build);
    println!("  Font cache:     {:>8.2?}", t_font);
    println!("  View create:    {:>8.2?}", t_view);
    println!("  Initial layout: {:>8.2?}", t_layout);
    println!("  View update:    {:>8.2?}", t_update);
    println!("  Tile cache:     {:>8.2?}", t_tilecache);
    println!("  First paint:    {:>8.2?}", t_paint);
    println!(
        "  Resize loop:    {:>8.2?} ({:.2?}/iter)",
        t_resize,
        t_resize / iterations as u32
    );
    println!("  Tiles/frame:    {}x{} = {}", cols, rows, cols * rows);
    println!(
        "  Total:          {:>8.2?}",
        t_build + t_font + t_view + t_layout + t_update + t_tilecache + t_paint + t_resize
    );
}
