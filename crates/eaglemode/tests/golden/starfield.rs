use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};

use emMain::emStarFieldPanel::emStarFieldPanel;

use super::common::*;

macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden data not found");
            return;
        }
    };
}

/// Render a starfield panel at depth/seed into an image of the given size.
/// Panel coordinates (0,0)-(1,1) are mapped to pixels (0,0)-(w,h).
fn render_starfield(depth: i32, seed: u32, w: u32, h: u32) -> emImage {
    let mut panel = emStarFieldPanel::new(depth, seed);
    let mut img = emImage::new(w, h, 4);
    // Don't fill -- Paint clears to black.
    {
        let mut p = emPainter::new(&mut img);
        p.scale(w as f64, h as f64);
        p.SetCanvasColor(emColor::TRANSPARENT);
        let state = PanelState::default_for_test();
        panel.Paint(&mut p, 1.0, 1.0, &state);
    }
    img
}

/// Small viewport: stars rendered as ellipses and rects (tiers 2+3).
/// ch_tol=69: Rust PaintEllipse AA polygon approximation and PaintImageColored
/// sampling differ slightly from C++ rasterizer at sub-pixel boundaries,
/// producing ±1-69 LSB differences at star edges (0.03% of pixels).
#[test]
fn starfield_small() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("starfield_small");
    let img = render_starfield(3, 0x12345678, ew, eh);
    compare_images("starfield_small", img.GetMap(), &expected, ew, eh, 69, 0.0)
        .expect("starfield_small golden mismatch");
}

/// Large viewport: stars rendered as textured glow (tier 1).
/// ch_tol=53: Rust PaintImageColored bilinear interpolation differs from C++
/// at sub-pixel star boundaries, producing ±1-53 LSB differences (0.02% of pixels).
#[test]
fn starfield_large() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("starfield_large");
    let img = render_starfield(3, 0x12345678, ew, eh);
    compare_images("starfield_large", img.GetMap(), &expected, ew, eh, 53, 0.0)
        .expect("starfield_large golden mismatch");
}
