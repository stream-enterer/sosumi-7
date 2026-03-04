/// RGBA color packed into a `u32` with layout R[31:24] G[23:16] B[15:8] A[7:0].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Color(u32);

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(255, 0, 0);
    pub const GREEN: Color = Color::rgb(0, 255, 0);
    pub const BLUE: Color = Color::rgb(0, 0, 255);
    pub const TRANSPARENT: Color = Color(0);

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a as u32)
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    #[inline]
    pub const fn r(self) -> u8 {
        (self.0 >> 24) as u8
    }

    #[inline]
    pub const fn g(self) -> u8 {
        (self.0 >> 16) as u8
    }

    #[inline]
    pub const fn b(self) -> u8 {
        (self.0 >> 8) as u8
    }

    #[inline]
    pub const fn a(self) -> u8 {
        self.0 as u8
    }

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// Create a color from HSV values. `h` in [0, 360), `s` and `v` in [0, 1].
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let s = s.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);
        let h = ((h % 360.0) + 360.0) % 360.0;

        let c = v * s;
        let h_prime = h / 60.0;
        let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
        let m = v - c;

        let (r1, g1, b1) = match h_prime as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Self::rgb(
            ((r1 + m) * 255.0 + 0.5) as u8,
            ((g1 + m) * 255.0 + 0.5) as u8,
            ((b1 + m) * 255.0 + 0.5) as u8,
        )
    }

    /// Convert to HSV. Returns `(h, s, v)` with h in [0, 360), s and v in [0, 1].
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let r = self.r() as f32 / 255.0;
        let g = self.g() as f32 / 255.0;
        let b = self.b() as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0 + 6.0) % 360.0
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        let s = if max == 0.0 { 0.0 } else { delta / max };

        (h, s, max)
    }

    /// Standard alpha blend: `self` over `other` using `alpha` (0–255).
    pub fn blend(self, other: Color, alpha: u8) -> Color {
        let a = alpha as u16;
        let inv_a = 255 - a;
        let r = (self.r() as u16 * a + other.r() as u16 * inv_a) / 255;
        let g = (self.g() as u16 * a + other.g() as u16 * inv_a) / 255;
        let b = (self.b() as u16 * a + other.b() as u16 * inv_a) / 255;
        let out_a = (self.a() as u16 * a + other.a() as u16 * inv_a) / 255;
        Color::rgba(r as u8, g as u8, b as u8, out_a as u8)
    }

    /// emCore canvas blend: `target += (source - canvas) * alpha`.
    ///
    /// `self` is the current target pixel, `source` is the color being painted,
    /// `canvas` is the background canvas color, `alpha` is blend strength (0–255).
    pub fn canvas_blend(self, source: Color, canvas: Color, alpha: u8) -> Color {
        let a = alpha as i16;
        let blend_ch = |target: u8, src: u8, cvs: u8| -> u8 {
            let result = target as i16 + ((src as i16 - cvs as i16) * a) / 255;
            result.clamp(0, 255) as u8
        };
        Color::rgba(
            blend_ch(self.r(), source.r(), canvas.r()),
            blend_ch(self.g(), source.g(), canvas.g()),
            blend_ch(self.b(), source.b(), canvas.b()),
            blend_ch(self.a(), source.a(), canvas.a()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_access() {
        let c = Color::rgba(10, 20, 30, 40);
        assert_eq!(c.r(), 10);
        assert_eq!(c.g(), 20);
        assert_eq!(c.b(), 30);
        assert_eq!(c.a(), 40);
    }

    #[test]
    fn rgb_sets_alpha_255() {
        let c = Color::rgb(1, 2, 3);
        assert_eq!(c.a(), 255);
    }

    #[test]
    fn named_constants() {
        assert_eq!(Color::BLACK, Color::rgb(0, 0, 0));
        assert_eq!(Color::WHITE, Color::rgb(255, 255, 255));
        assert_eq!(Color::TRANSPARENT.a(), 0);
    }

    #[test]
    fn blend_extremes() {
        let a = Color::rgb(255, 0, 0);
        let b = Color::rgb(0, 0, 255);
        // Full alpha -> source
        assert_eq!(a.blend(b, 255), a);
        // Zero alpha -> dest
        assert_eq!(a.blend(b, 0), b);
    }

    #[test]
    fn canvas_blend_identity() {
        let target = Color::rgb(100, 100, 100);
        // source == canvas -> no change
        let result = target.canvas_blend(Color::rgb(50, 50, 50), Color::rgb(50, 50, 50), 255);
        assert_eq!(result.r(), 100);
        assert_eq!(result.g(), 100);
        assert_eq!(result.b(), 100);
    }

    #[test]
    fn hsv_round_trip() {
        let original = Color::rgb(200, 100, 50);
        let (h, s, v) = original.to_hsv();
        let reconstructed = Color::from_hsv(h, s, v);
        // Allow ±1 due to rounding
        assert!((original.r() as i16 - reconstructed.r() as i16).abs() <= 1);
        assert!((original.g() as i16 - reconstructed.g() as i16).abs() <= 1);
        assert!((original.b() as i16 - reconstructed.b() as i16).abs() <= 1);
    }

    #[test]
    fn hsv_pure_colors() {
        let (h, s, v) = Color::RED.to_hsv();
        assert!((h - 0.0).abs() < 1.0);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        let (h, _, _) = Color::GREEN.to_hsv();
        assert!((h - 120.0).abs() < 1.0);

        let (h, _, _) = Color::BLUE.to_hsv();
        assert!((h - 240.0).abs() < 1.0);
    }
}
