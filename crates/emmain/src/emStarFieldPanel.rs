// Port of C++ emMain/emStarFieldPanel
// Fractal starfield: each panel contains pseudo-random stars and subdivides
// into 4 child quadrants when zoomed in enough.

use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPainter::emPainter;
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::ViewConditionType;
use emcore::emResTga::load_tga;
use emcore::emTexture::ImageExtension;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const BG_COLOR: u32 = 0x000000FF;
pub const MIN_PANEL_SIZE: f64 = 64.0;
pub const MIN_STAR_RADIUS: f64 = 0.3;

/// XOR masks applied to successive GetRandom() outputs to form child seeds.
/// Matches C++ emStarFieldPanel.cpp child seed derivation.
const CHILD_SEED_XOR: [u32; 4] = [0x74fc8324, 0x058f56a9, 0xfc863e37, 0x8bef7891];

// ── PRNG ─────────────────────────────────────────────────────────────────────

/// One step of the LCG used in C++ emStarFieldPanel::GetRandom().
/// `seed = seed * 1664525 + 1013904223` (Knuth/Numerical Recipes LCG, wrapping u32).
#[inline]
fn lcg_step(seed: u32) -> u32 {
    seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

/// Return the next random value and advance the seed in-place.
#[inline]
fn get_random_u32(seed: &mut u32) -> u32 {
    *seed = lcg_step(*seed);
    *seed
}

/// Return a float in [min_val, max_val) matching C++ `GetRandom(double, double)`.
/// C++ formula: `GetRandom() * (maxVal - minVal) / EM_UINT32_MAX + minVal`
/// where `EM_UINT32_MAX = 0xFFFFFFFF`.
#[inline]
fn get_random_range(seed: &mut u32, min_val: f64, max_val: f64) -> f64 {
    let r = get_random_u32(seed);
    r as f64 * (max_val - min_val) / (u32::MAX as f64) + min_val
}

// ── HSV → RGB ─────────────────────────────────────────────────────────────────

/// Convert HSV to `emColor`. Uses the exact C++ integer algorithm from
/// `emColor::SetHSVA` (emColor.cpp:868-918).
///
/// `h` ∈ [0, 360), `s` ∈ [0, 100], `v` ∈ [0, 100].
#[inline]
fn hsv_to_color(h: f64, s: f64, v: f64) -> emColor {
    emColor::SetHSVA(h as f32, s as f32, v as f32)
}

// ── Star ─────────────────────────────────────────────────────────────────────

/// A single star's position, radius, and color.
///
/// All coordinates are in panel-normalized space: X, Y ∈ [0, 1],
/// Radius is a fraction of the panel width.
#[derive(Clone, Debug)]
pub struct Star {
    // DIVERGED: field names uppercase to match C++ struct member names
    pub X: f64,
    pub Y: f64,
    pub Radius: f64,
    pub Color: emColor,
}

// ── emStarFieldPanel ─────────────────────────────────────────────────────────

/// Fractal starfield panel.
///
/// Port of C++ `emStarFieldPanel` from `emMain/emStarFieldPanel.cpp`.
///
/// Each panel displays pseudo-random stars on a black background and
/// recursively subdivides into 4 child quadrants when the panel is wide
/// enough in the viewport (`viewed_width >= 2 * MIN_PANEL_SIZE = 128 px`).
pub struct emStarFieldPanel {
    depth: i32,
    child_random_seeds: [u32; 4],
    stars: Vec<Star>,
    star_shape: emImage,
    /// Cached viewport width from the last `notice()` call; used by
    /// `LayoutChildren` to decide whether children should exist.
    noticed_viewed_w: f64,
}

impl emStarFieldPanel {
    /// Create a new `emStarFieldPanel` at the given recursion depth with a
    /// deterministic seed.
    ///
    /// Port of C++ `emStarFieldPanel` constructor.
    pub fn new(depth: i32, seed: u32) -> Self {
        let mut random_seed = seed;

        // Generate stars.
        // C++: if (Depth < 1) StarCount = 0;
        //      else StarCount = (int)(emMin(Depth*3, 400) * GetRandom(0.5, 1.0));
        let stars = if depth < 1 {
            Vec::new()
        } else {
            let max_count = ((depth * 3).min(400)) as f64;
            let count = (max_count * get_random_range(&mut random_seed, 0.5, 1.0)) as usize;
            let mut stars = Vec::with_capacity(count);
            for _ in 0..count {
                let r = MIN_STAR_RADIUS / MIN_PANEL_SIZE * get_random_range(&mut random_seed, 0.5, 1.0);
                let x = get_random_range(&mut random_seed, r, 1.0 - r);
                let y = get_random_range(&mut random_seed, r, 1.0 - r);
                let h = get_random_range(&mut random_seed, 0.0, 360.0);
                let s = get_random_range(&mut random_seed, 0.0, 15.0);
                stars.push(Star {
                    X: x,
                    Y: y,
                    Radius: r,
                    Color: hsv_to_color(h, s, 100.0),
                });
            }
            stars
        };

        // Derive child seeds.
        // C++: ChildRandomSeed[i] = GetRandom() ^ CHILD_SEED_XOR[i]
        let mut child_random_seeds = [0u32; 4];
        for (i, xor) in CHILD_SEED_XOR.iter().enumerate() {
            child_random_seeds[i] = get_random_u32(&mut random_seed) ^ xor;
        }

        let star_shape = load_tga(include_bytes!("../../../res/emMain/Star.tga"))
            .expect("failed to load Star.tga");

        Self {
            depth,
            child_random_seeds,
            stars,
            star_shape,
            noticed_viewed_w: 0.0,
        }
    }

    /// Child quadrant layout in panel-normalized coordinates: (x, y, w, h).
    /// Matches C++ `UpdateChildren` quadrant layout.
    const CHILD_RECTS: [(f64, f64, f64, f64); 4] = [
        (0.0, 0.0, 0.5, 0.5), // child 0: top-left
        (0.5, 0.0, 0.5, 0.5), // child 1: top-right
        (0.0, 0.5, 0.5, 0.5), // child 2: bottom-left
        (0.5, 0.5, 0.5, 0.5), // child 3: bottom-right
    ];
}

impl PanelBehavior for emStarFieldPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn GetCanvasColor(&self) -> emColor {
        emColor::from_packed(BG_COLOR)
    }

    fn get_title(&self) -> Option<String> {
        Some("Star Field".to_string())
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {
        let bg = emColor::from_packed(BG_COLOR);
        painter.Clear(bg);

        let (sx, _sy) = painter.scaling();
        let src_w = self.star_shape.GetWidth();
        let src_h = self.star_shape.GetHeight();

        // DIVERGED: C++ PaintOverlay — Rust has no PaintOverlay trait method;
        // stars render in Paint (before children) instead of after.
        for star in &self.stars {
            let mut r = star.Radius;
            let vr = sx * r;

            if vr <= MIN_STAR_RADIUS {
                continue;
            }

            if vr > 4.0 {
                // Tier 1: textured star with glow
                let hue = star.Color.GetHue();
                let sat = star.Color.GetSat();
                let alpha = (sat * 18.0).min(255.0) as u8;
                let x = star.X - r;
                let y = star.Y - r;
                let d = r * 2.0;
                // Glow pass
                let glow_color = emColor::SetHSVA_with_alpha(hue, 100.0, 100.0, alpha);
                painter.PaintImageColored(
                    x, y, d, d,
                    &self.star_shape, 0, 0, src_w, src_h,
                    emColor::TRANSPARENT, // color1: black bg → transparent
                    glow_color,           // color2: white star → glow
                    emColor::TRANSPARENT, ImageExtension::Zero,
                );
                // Star pass
                let star_color = emColor::SetHSVA(hue, (sat - 10.0).max(0.0), 100.0);
                painter.PaintImageColored(
                    x, y, d, d,
                    &self.star_shape, 0, 0, src_w, src_h,
                    emColor::TRANSPARENT, // color1: black bg → transparent
                    star_color,           // color2: white star → star color
                    emColor::TRANSPARENT, ImageExtension::Zero,
                );
            } else {
                r *= 0.6;
                let vr = sx * r;
                if vr > 1.2 {
                    // Tier 2: ellipse
                    // C++ PaintOverlay passes no canvasColor (defaults to 0 = transparent)
                    painter.PaintEllipse(star.X, star.Y, r, r, star.Color, emColor::TRANSPARENT);
                } else {
                    // Tier 3: rect
                    r *= 0.8862;
                    let x = star.X - r;
                    let y = star.Y - r;
                    let d = r * 2.0;
                    // C++ PaintOverlay passes no canvasColor (defaults to 0 = transparent)
                    painter.PaintRect(x, y, d, d, star.Color, emColor::TRANSPARENT);
                }
            }
        }
    }

    fn notice(&mut self, _flags: NoticeFlags, state: &PanelState) {
        // Cache viewport width so LayoutChildren can decide on children.
        self.noticed_viewed_w = state.viewed_rect.w;
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Register auto-expansion threshold: expand when viewed width >= 2 * MIN_PANEL_SIZE.
        // This is idempotent and ensures the correct threshold is set after the first
        // expansion (the very first call may use the default Area threshold, but once
        // LayoutChildren runs, Width threshold is recorded for subsequent checks).
        ctx.tree.SetAutoExpansionThreshold(
            ctx.id,
            2.0 * MIN_PANEL_SIZE,
            ViewConditionType::Width,
        );

        let children = ctx.children();
        let bg = emColor::from_packed(BG_COLOR);

        if children.is_empty() {
            // Create 4 quadrant child panels.
            // NOTE: Easter egg (TicTacToePanel at depth > 50) is not implemented.
            // C++ emStarFieldPanel.cpp: if (Depth > 50 && rand%11213 == 0) create TicTacToe.
            for i in 0..4 {
                let child_depth = self.depth + 1;
                let child_seed = self.child_random_seeds[i];
                let child = Box::new(emStarFieldPanel::new(child_depth, child_seed));
                let child_id = ctx.create_child_with(&format!("{i}"), child);
                // Set the child's auto-expand threshold.
                ctx.tree.SetAutoExpansionThreshold(
                    child_id,
                    2.0 * MIN_PANEL_SIZE,
                    ViewConditionType::Width,
                );
                // Position child in its quadrant.
                let (cx, cy, cw, ch) = Self::CHILD_RECTS[i];
                ctx.layout_child_canvas(child_id, cx, cy, cw, ch, bg);
            }
        } else {
            // Reposition existing children (e.g. after LAYOUT_CHANGED).
            for (i, &child_id) in children.iter().enumerate() {
                if i >= 4 {
                    break;
                }
                let (cx, cy, cw, ch) = Self::CHILD_RECTS[i];
                ctx.layout_child_canvas(child_id, cx, cy, cw, ch, bg);
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prng_deterministic() {
        let p1 = emStarFieldPanel::new(5, 0xABCDABCD);
        let p2 = emStarFieldPanel::new(5, 0xABCDABCD);
        assert_eq!(p1.stars.len(), p2.stars.len());
        for (a, b) in p1.stars.iter().zip(p2.stars.iter()) {
            assert!((a.X - b.X).abs() < 1e-10);
            assert!((a.Y - b.Y).abs() < 1e-10);
            assert!((a.Radius - b.Radius).abs() < 1e-10);
            assert_eq!(a.Color, b.Color);
        }
    }

    #[test]
    fn test_depth_0_no_stars() {
        let p = emStarFieldPanel::new(0, 0x12345678);
        assert_eq!(p.stars.len(), 0);
    }

    #[test]
    fn test_depth_1_has_stars() {
        let p = emStarFieldPanel::new(1, 0x12345678);
        assert!(!p.stars.is_empty());
        // min(1*3, 400) * random(0.5, 1.0) ∈ [1.5, 3.0) → 1..3 stars
        assert!(p.stars.len() <= 3, "depth-1 panel should have at most 3 stars, got {}", p.stars.len());
    }

    #[test]
    fn test_stars_in_bounds() {
        let p = emStarFieldPanel::new(10, 0xDEADBEEF);
        for star in &p.stars {
            assert!(star.X >= 0.0 && star.X <= 1.0, "X={} out of bounds", star.X);
            assert!(star.Y >= 0.0 && star.Y <= 1.0, "Y={} out of bounds", star.Y);
            assert!(star.Radius > 0.0, "Radius must be positive");
        }
    }

    #[test]
    fn test_child_seeds_computed() {
        let p = emStarFieldPanel::new(5, 0xABCD1234);
        // Seeds should be non-zero and distinct.
        assert_ne!(p.child_random_seeds[0], 0);
        assert_ne!(p.child_random_seeds[0], p.child_random_seeds[1]);
        assert_ne!(p.child_random_seeds[1], p.child_random_seeds[2]);
        assert_ne!(p.child_random_seeds[2], p.child_random_seeds[3]);
    }

    #[test]
    fn test_panel_behavior() {
        let p = emStarFieldPanel::new(5, 0xABCD1234);
        assert!(p.IsOpaque());
        assert_eq!(p.get_title(), Some("Star Field".to_string()));
        assert!(p.auto_expand());
    }

    #[test]
    fn test_different_seeds_differ() {
        let p1 = emStarFieldPanel::new(5, 0x11111111);
        let p2 = emStarFieldPanel::new(5, 0x22222222);
        // Very unlikely to produce identical star counts and positions.
        let same = p1.stars.len() == p2.stars.len()
            && p1
                .stars
                .iter()
                .zip(p2.stars.iter())
                .all(|(a, b)| (a.X - b.X).abs() < 1e-10);
        assert!(!same, "Different seeds should produce different stars");
    }

    #[test]
    fn test_child_seeds_deterministic() {
        let p1 = emStarFieldPanel::new(3, 0xCAFEBABE);
        let p2 = emStarFieldPanel::new(3, 0xCAFEBABE);
        assert_eq!(p1.child_random_seeds, p2.child_random_seeds);
    }

    #[test]
    fn test_star_shape_loaded() {
        let img = emcore::emResTga::load_tga(include_bytes!("../../../res/emMain/Star.tga"))
            .expect("failed to load Star.tga");
        assert!(img.GetWidth() > 0);
        assert!(img.GetHeight() > 0);
    }

    #[test]
    fn test_depth_large_star_count() {
        // depth=100: min(300, 400) = 300 max stars
        let p = emStarFieldPanel::new(100, 0x99887766);
        assert!(p.stars.len() <= 400);
        assert!(!p.stars.is_empty());
    }

    #[test]
    fn test_star_radius_range() {
        // radius = MIN_STAR_RADIUS / MIN_PANEL_SIZE * random(0.5, 1.0)
        //        = 0.3 / 64.0 * [0.5, 1.0]
        //        ∈ [0.00234375, 0.0046875]
        let p = emStarFieldPanel::new(50, 0xFEDCBA98);
        let min_r = MIN_STAR_RADIUS / MIN_PANEL_SIZE * 0.5;
        let max_r = MIN_STAR_RADIUS / MIN_PANEL_SIZE * 1.0;
        for star in &p.stars {
            assert!(
                star.Radius >= min_r * 0.999 && star.Radius <= max_r * 1.001,
                "Radius {} out of range [{}, {}]",
                star.Radius,
                min_r,
                max_r
            );
        }
    }
}
