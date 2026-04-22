use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

/// Error returned when parsing a hex color string fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorParseError {
    _private: (),
}

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid color string: expected #RRGGBB or #RRGGBBAA")
    }
}

impl std::error::Error for ColorParseError {}

// ── Blend hash table (C++ SharedPixelFormat) ──────────────────────────
//
// C++ emPainter uses precomputed 256×256 hash tables per channel for alpha
// blending (emPainter.cpp:190-234, emPainter.h:794-804). The tables decompose
// the blend `(color * alpha + round) / 255` into four quadrant terms, each
// independently rounded at table-build time. This produces different rounding
// from direct computation for ~0.2% of (color, alpha) pairs (±1 channel value).
//
// C++ has three separate tables (RedHash, GreenHash, BlueHash) because each
// channel can have a different range and bit-shift in the packed pixel. Since
// the Rust port always uses 4-byte RGBA with range=255 for all channels, the
// unshifted table values are identical across channels. One table suffices.
// SharedPixelFormat — single table instead of C++'s three (RedHash, GreenHash, BlueHash), because
// range=255 for all channels makes them identical when unshifted in the fixed RGBA layout.

static BLEND_HASH: LazyLock<Box<[u8; 65536]>> = LazyLock::new(|| {
    let mut hash = Box::new([0u8; 65536]);
    let range: i32 = 255;
    for a1 in 0i32..128 {
        let c1 = (a1 * range + 127) / 255;
        for a2 in 0i32..128 {
            let c2 = (a2 * range + 127) / 255;
            let c3 = (a1 * a2 * range + 32512) / 65025;
            hash[(a1 as usize) << 8 | a2 as usize] = c3 as u8;
            hash[(a1 as usize) << 8 | (255 - a2 as usize)] = (c1 - c3) as u8;
            hash[(255 - a1 as usize) << 8 | a2 as usize] = (c2 - c3) as u8;
            hash[(255 - a1 as usize) << 8 | (255 - a2 as usize)] = (range + c3 - c1 - c2) as u8;
        }
    }
    hash
});

/// Look up the premultiplied blend contribution for a (color, alpha) pair
/// using the C++ hash table decomposition. Returns the same value as
/// `(color * alpha + 127) / 255` for ~99.8% of inputs; differs by ±1 for
/// the remaining ~0.2% due to independently rounded quadrant terms.
///
/// Matches C++ `((PTYPE*)hashTable)[alpha]` where `hashTable` points to
/// the row for `color` (emPainter.cpp:817, emPainter_ScTlPSCol.cpp:97).
#[inline(always)]
pub(crate) fn blend_hash_lookup(color: u8, alpha: u8) -> u8 {
    BLEND_HASH[(color as usize) << 8 | alpha as usize]
}

/// Fused source-over channel blend matching C++ AVX2 PaintScanlineCol/Int.
/// `result = round_div255(src * alpha + dst * (255 - alpha))`
#[inline(always)]
pub(crate) fn blend_channel_fused(src: u8, dst: u8, alpha: u8) -> u8 {
    let v = src as u32 * alpha as u32 + dst as u32 * (255 - alpha as u32);
    let v2 = v + 0x80;
    ((v2 + (v2 >> 8)) >> 8) as u8
}

/// Fast div255 with round-to-nearest, matching C++ AVX2 `(v+0x80+((v+0x80)>>8))>>8`.
#[inline(always)]
pub(crate) fn fast_div255(x: u32) -> u8 {
    let v = x + 0x80;
    ((v + (v >> 8)) >> 8) as u8
}

/// Matches C++ AVX2 `_mm256_mulhrs_epi16(value << 3, opacity)`.
/// Equivalent to `(value * opacity * 8 + 0x4000) >> 15`.
/// Used for premultiplied source scaling in PaintScanlineInt AVX2.
#[inline(always)]
pub(crate) fn mulhrs_scale(value: u8, opacity: i32) -> u8 {
    ((value as i32 * 8 * opacity + 0x4000) >> 15) as u8
}

// DIVERGED: (language-forced) Get — renamed to GetPacked because Rust has no implicit u32 conversion operator
// DIVERGED: (language-forced) Set (all overloads) — not ported (emColor is Copy; use constructors rgba/rgb/SetAlpha instead of mutation)

/// RGBA color packed into a `u32` with layout R[31:24] G[23:16] B[15:8] A[7:0].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct emColor(u32);

impl emColor {
    pub const BLACK: emColor = emColor::rgb(0, 0, 0);
    pub const WHITE: emColor = emColor::rgb(255, 255, 255);
    pub const RED: emColor = emColor::rgb(255, 0, 0);
    pub const GREEN: emColor = emColor::rgb(0, 255, 0);
    pub const BLUE: emColor = emColor::rgb(0, 0, 255);
    pub const GRAY: emColor = emColor::rgb(128, 128, 128);
    pub const YELLOW: emColor = emColor::rgb(255, 255, 0);
    pub const CYAN: emColor = emColor::rgb(0, 255, 255);
    pub const MAGENTA: emColor = emColor::rgb(255, 0, 255);
    pub const TRANSPARENT: emColor = emColor(0);

    /// Construct from a packed RGBA u32 (same layout as internal storage).
    #[inline]
    pub const fn from_packed(packed: u32) -> Self {
        Self(packed)
    }

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self((r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | a as u32)
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    #[inline]
    pub const fn GetRed(self) -> u8 {
        (self.0 >> 24) as u8
    }

    #[inline]
    pub const fn GetGreen(self) -> u8 {
        (self.0 >> 16) as u8
    }

    #[inline]
    pub const fn GetBlue(self) -> u8 {
        (self.0 >> 8) as u8
    }

    #[inline]
    pub const fn GetAlpha(self) -> u8 {
        self.0 as u8
    }

    // DIVERGED: (language-forced) Get — renamed to GetPacked because Rust has no implicit u32 conversion operator
    #[inline]
    pub const fn GetPacked(self) -> u32 {
        self.0
    }

    // DIVERGED: (language-forced) SetHSVA — constructor instead of mutator (emColor is Copy);
    // 4-param variant available as SetHSVA_with_alpha. s/v scale matches C++ [0,100].
    /// Create a color from HSV values. `h` in [0, 360), `s` and `v` in [0, 100].
    ///
    /// Uses the exact C++ integer algorithm (emColor.cpp:868-918):
    ///   cmax = (int)(val*2.55+0.5); cmin = cmax - (int)(cmax*sat*0.01+0.5);
    ///   cunit = cmax-cmin; chue = (int)(cunit*hue/60+0.5);
    ///   then sextant dispatch on chue vs cunit boundaries.
    pub fn SetHSVA(h: f32, s: f32, v: f32) -> Self {
        let s = s.clamp(0.0, 100.0);
        let v = v.clamp(0.0, 100.0);
        let h = if h < 0.0 {
            (h % 360.0) + 360.0
        } else if h >= 360.0 {
            h % 360.0
        } else {
            h
        };

        // Exact C++ expression order and types (float = f32). s/v are already [0,100].
        let cmax = (v * 2.55_f32 + 0.5_f32) as i32;
        let cmin = cmax - (cmax as f32 * s * 0.01_f32 + 0.5_f32) as i32;
        let cunit = cmax - cmin;
        let chue = (cunit as f32 * h * (1.0_f32 / 60.0_f32) + 0.5_f32) as i32;

        let (r, g, b) = if chue <= cunit * 3 {
            if chue <= cunit {
                (cmax, cmin + chue, cmin)
            } else if chue <= cunit * 2 {
                (cmin + 2 * cunit - chue, cmax, cmin)
            } else {
                (cmin, cmax, cmin + chue - 2 * cunit)
            }
        } else if chue <= cunit * 4 {
            (cmin, cmin + 4 * cunit - chue, cmax)
        } else if chue <= cunit * 5 {
            (cmin + chue - 4 * cunit, cmin, cmax)
        } else {
            (cmax, cmin, cmin + 6 * cunit - chue)
        };

        Self::rgb(r as u8, g as u8, b as u8)
    }

    /// Create a color from HSV + alpha. C++ `emColor::SetHSVA(h, s, v, alpha)`.
    pub fn SetHSVA_with_alpha(h: f32, s: f32, v: f32, alpha: u8) -> Self {
        Self::SetHSVA(h, s, v).SetAlpha(alpha)
    }

    /// Convert to HSV. Returns `(h, s, v)` with h in [0, 360), s and v in [0, 100].
    ///
    /// Port of C++ GetHue/GetSat/GetVal integer algorithm (emColor.cpp:793-864).
    pub fn GetHSV(self) -> (f32, f32, f32) {
        let r = self.GetRed() as i32;
        let g = self.GetGreen() as i32;
        let b = self.GetBlue() as i32;

        // C++ GetHue (emColor.cpp:793-825): integer sextant dispatch
        let (u, hh) = if r >= g {
            if g >= b {
                let u = r - b;
                if u == 0 {
                    (1, 0) // u=1 placeholder, hue=0
                } else {
                    (u, g - b)
                }
            } else if r >= b {
                let u = r - g;
                (u, u * 6 - b + g)
            } else {
                let u = b - g;
                (u, u * 4 + r - g)
            }
        } else if r >= b {
            let u = g - b;
            (u, u * 2 - r + b)
        } else if g >= b {
            let u = g - r;
            (u, u * 2 + b - r)
        } else {
            let u = b - r;
            (u, u * 4 - g + r)
        };

        let hue = if r >= g && g >= b && r == b {
            0.0_f32
        } else {
            ((hh * 60) as f32) / u as f32
        };

        // C++ GetSat (emColor.cpp:828-854): integer cmax/cmin
        let cmax = r.max(g).max(b);
        let cmin = r.min(g).min(b);
        let sat = if cmax == 0 {
            0.0_f32
        } else {
            ((cmax - cmin) * 100) as f32 / cmax as f32
        };

        // C++ GetVal (emColor.cpp:857-865): max * (100/255)
        let val = cmax as f32 * (100.0_f32 / 255.0_f32);

        (hue, sat, val)
    }

    /// Get the hue component. Returns [0.0, 360.0).
    /// C++ `emColor::GetHue()`.
    pub fn GetHue(self) -> f32 {
        self.GetHSV().0
    }

    /// Get the saturation component. Returns [0.0, 100.0].
    /// C++ `emColor::GetSat()`.
    pub fn GetSat(self) -> f32 {
        self.GetHSV().1
    }

    /// Get the value (brightness) component. Returns [0.0, 100.0].
    /// C++ `emColor::GetVal()`.
    pub fn GetVal(self) -> f32 {
        self.GetHSV().2
    }

    /// Lighten or darken the color.
    /// `light` in [-100.0, 100.0]: positive blends toward white, negative toward black.
    /// Matches C++ `emColor::GetLighted(float light)`.
    pub fn GetLighted(self, light: f32) -> emColor {
        if light <= 0.0 {
            self.GetBlended(emColor::rgba(0, 0, 0, self.GetAlpha()), (-light) as f64)
        } else {
            self.GetBlended(emColor::rgba(255, 255, 255, self.GetAlpha()), light as f64)
        }
    }

    /// Standard alpha blend: `self` over `other` using `alpha` (0–255).
    ///
    /// Uses `/256` integer math matching C++ emPainter precision.
    pub fn blend(self, other: emColor, alpha: u8) -> emColor {
        let a = alpha as u16;
        let inv_a = 256 - a;
        let r = (self.GetRed() as u16 * a + other.GetRed() as u16 * inv_a) >> 8;
        let g = (self.GetGreen() as u16 * a + other.GetGreen() as u16 * inv_a) >> 8;
        let b = (self.GetBlue() as u16 * a + other.GetBlue() as u16 * inv_a) >> 8;
        let out_a = (self.GetAlpha() as u16 * a + other.GetAlpha() as u16 * inv_a) >> 8;
        emColor::rgba(r as u8, g as u8, b as u8, out_a as u8)
    }

    // DIVERGED: (language-forced) SetAlpha — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the alpha channel replaced.
    #[inline]
    pub const fn SetAlpha(self, a: u8) -> emColor {
        emColor::rgba(self.GetRed(), self.GetGreen(), self.GetBlue(), a)
    }

    /// Linearly interpolate between `self` and `other` by weight `weight` (0.0–100.0).
    ///
    /// Matches C++ `emColor::GetBlended(color, weight)` with 16-bit precision:
    /// `w2 = (int)(weight * 655.36 + 0.5)`, `result = (a*w1 + b*w2 + 32768) >> 16`.
    pub fn GetBlended(self, other: emColor, weight: f64) -> emColor {
        if weight <= 0.0 {
            return self;
        }
        if weight >= 100.0 {
            return other;
        }
        let w2 = (weight * 655.36 + 0.5) as i32;
        let w1 = 65536 - w2;
        let mix = |a: i32, b: i32| -> u8 { ((a * w1 + b * w2 + 32768) >> 16) as u8 };
        emColor::rgba(
            mix(self.GetRed() as i32, other.GetRed() as i32),
            mix(self.GetGreen() as i32, other.GetGreen() as i32),
            mix(self.GetBlue() as i32, other.GetBlue() as i32),
            mix(self.GetAlpha() as i32, other.GetAlpha() as i32),
        )
    }

    /// emCore canvas blend: `target += hash(source,alpha) - hash(canvas,alpha)`.
    ///
    /// `self` is the current target pixel, `source` is the color being painted,
    /// `canvas` is the background canvas color, `alpha` is blend strength (0–255).
    /// Uses the C++ hash table for both source and canvas terms, matching the
    /// 4-quadrant decomposition in emPainter's SharedPixelFormat (emPainter.cpp:190-234).
    pub fn canvas_blend(self, source: emColor, canvas: emColor, alpha: u8) -> emColor {
        let blend_ch = |target: u8, src: u8, cvs: u8| -> u8 {
            let src_term = fast_div255(src as u32 * alpha as u32) as i32;
            let cvs_term = fast_div255(cvs as u32 * alpha as u32) as i32;
            (target as i32 + src_term - cvs_term).clamp(0, 255) as u8
        };
        emColor::rgba(
            blend_ch(self.GetRed(), source.GetRed(), canvas.GetRed()),
            blend_ch(self.GetGreen(), source.GetGreen(), canvas.GetGreen()),
            blend_ch(self.GetBlue(), source.GetBlue(), canvas.GetBlue()),
            blend_ch(self.GetAlpha(), source.GetAlpha(), canvas.GetAlpha()),
        )
    }

    // DIVERGED: (language-forced) SetRed — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the red channel replaced.
    #[inline]
    pub const fn SetRed(self, r: u8) -> emColor {
        emColor::rgba(r, self.GetGreen(), self.GetBlue(), self.GetAlpha())
    }

    // DIVERGED: (language-forced) SetGreen — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the green channel replaced.
    #[inline]
    pub const fn SetGreen(self, g: u8) -> emColor {
        emColor::rgba(self.GetRed(), g, self.GetBlue(), self.GetAlpha())
    }

    // DIVERGED: (language-forced) SetBlue — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the blue channel replaced.
    #[inline]
    pub const fn SetBlue(self, b: u8) -> emColor {
        emColor::rgba(self.GetRed(), self.GetGreen(), b, self.GetAlpha())
    }

    /// Returns `true` if the alpha channel is zero.
    #[inline]
    pub const fn IsTotallyTransparent(self) -> bool {
        self.GetAlpha() == 0
    }

    /// Returns `true` if the alpha channel is 255.
    #[inline]
    pub const fn IsOpaque(self) -> bool {
        self.GetAlpha() == 255
    }

    /// Returns `true` if all RGB channels are equal.
    #[inline]
    pub const fn IsGrey(self) -> bool {
        self.GetRed() == self.GetGreen() && self.GetGreen() == self.GetBlue()
    }

    /// Average of RGB channels as a grey value.
    /// Uses C++ `GetGrey` rounding: `(r + g + b + 1) / 3`.
    pub fn GetGrey(self) -> u8 {
        ((self.GetRed() as u16 + self.GetGreen() as u16 + self.GetBlue() as u16 + 1) / 3) as u8
    }

    // DIVERGED: (language-forced) SetGrey — constructor instead of mutator (emColor is Copy);
    // 2-param variant available as SetGrey_with_alpha.
    /// Construct a grey color with `a=255`.
    #[inline]
    pub const fn SetGrey(val: u8) -> emColor {
        emColor::rgba(val, val, val, 255)
    }

    /// Construct a grey color with explicit alpha. C++ `emColor::SetGrey(val, alpha)`.
    #[inline]
    pub const fn SetGrey_with_alpha(val: u8, alpha: u8) -> emColor {
        emColor::rgba(val, val, val, alpha)
    }

    // DIVERGED: (language-forced) SetHue — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the HSV hue replaced, preserving saturation, value, and alpha.
    pub fn SetHue(self, h: f32) -> emColor {
        let (_old_h, s, v) = self.GetHSV();
        emColor::SetHSVA(h, s, v).SetAlpha(self.GetAlpha())
    }

    // DIVERGED: (language-forced) SetSat — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the HSV saturation replaced, preserving hue, value, and alpha.
    pub fn SetSat(self, s: f32) -> emColor {
        let (h, _old_s, v) = self.GetHSV();
        emColor::SetHSVA(h, s, v).SetAlpha(self.GetAlpha())
    }

    // DIVERGED: (language-forced) SetVal — returns new value instead of mutating (emColor is Copy)
    /// Return a copy with the HSV value replaced, preserving hue, saturation, and alpha.
    pub fn SetVal(self, v: f32) -> emColor {
        let (h, s, _old_v) = self.GetHSV();
        emColor::SetHSVA(h, s, v).SetAlpha(self.GetAlpha())
    }

    /// Parse a color string supporting hex formats and X11 named colors.
    ///
    /// Port of C++ `emGetColorFromString`. Supports:
    /// - `#RGB` (3-char hex, 1 digit/channel)
    /// - `#RGBA` (4-char hex)
    /// - `#RRGGBB` (6-char hex)
    /// - `#RRGGBBAA` (8-char hex)
    /// - `#RRRGGGBBB` (9-char hex, 3 digits/channel)
    /// - `#RRRRGGGGBBBB` (12-char hex)
    /// - `#RRRRGGGGBBBBAAAA` (16-char hex)
    /// - `"none"` → transparent grey `rgba(128, 128, 128, 0)`
    /// - X11 named colors (case-insensitive, no spaces)
    pub fn TryParse(s: &str) -> Option<emColor> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("none") {
            return Some(emColor::rgba(128, 128, 128, 0));
        }
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex);
        }
        // X11 named color lookup (strip spaces, lowercase)
        let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        let rgb = super::emColorX11Colors::lookup_x11_color(&cleaned)?;
        Some(emColor::rgb(rgb[0], rgb[1], rgb[2]))
    }

    /// Parse a hex string (without the '#' prefix) into a emColor.
    fn parse_hex(hex: &str) -> Option<emColor> {
        let len = hex.len();
        // Validate all hex digits
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        match len {
            3 => {
                // #RGB: 1 hex digit per channel, replicate (0xF -> 0xFF)
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(emColor::rgb(r << 4 | r, g << 4 | g, b << 4 | b))
            }
            4 => {
                // #RGBA
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4], 16).ok()?;
                Some(emColor::rgba(
                    r << 4 | r,
                    g << 4 | g,
                    b << 4 | b,
                    a << 4 | a,
                ))
            }
            6 => {
                // #RRGGBB
                let val = u32::from_str_radix(hex, 16).ok()?;
                Some(emColor::rgb((val >> 16) as u8, (val >> 8) as u8, val as u8))
            }
            8 => {
                // #RRGGBBAA
                let val = u32::from_str_radix(hex, 16).ok()?;
                Some(emColor::rgba(
                    (val >> 24) as u8,
                    (val >> 16) as u8,
                    (val >> 8) as u8,
                    val as u8,
                ))
            }
            9 => {
                // #RRRGGGBBB: 3 hex digits per channel, use high byte
                let r = u16::from_str_radix(&hex[0..3], 16).ok()?;
                let g = u16::from_str_radix(&hex[3..6], 16).ok()?;
                let b = u16::from_str_radix(&hex[6..9], 16).ok()?;
                Some(emColor::rgb((r >> 4) as u8, (g >> 4) as u8, (b >> 4) as u8))
            }
            12 => {
                // #RRRRGGGGBBBB: 4 hex digits per channel, use high byte
                let r = u16::from_str_radix(&hex[0..4], 16).ok()?;
                let g = u16::from_str_radix(&hex[4..8], 16).ok()?;
                let b = u16::from_str_radix(&hex[8..12], 16).ok()?;
                Some(emColor::rgb((r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8))
            }
            16 => {
                // #RRRRGGGGBBBBAAAA
                let r = u16::from_str_radix(&hex[0..4], 16).ok()?;
                let g = u16::from_str_radix(&hex[4..8], 16).ok()?;
                let b = u16::from_str_radix(&hex[8..12], 16).ok()?;
                let a = u16::from_str_radix(&hex[12..16], 16).ok()?;
                Some(emColor::rgba(
                    (r >> 8) as u8,
                    (g >> 8) as u8,
                    (b >> 8) as u8,
                    (a >> 8) as u8,
                ))
            }
            _ => None,
        }
    }

    /// Scale alpha by `amount` in \[-100, 100\].
    /// Positive values make more transparent, negative values make more opaque.
    ///
    /// C++ formula (emColor.cpp:945-959):
    ///   tp = amount * 0.01;
    ///   if tp >= 0: a = Alpha*(1-tp)+0.5
    ///   if tp < 0:  a = Alpha*(1+tp) - 255*tp + 0.5
    /// C++ emColor::GetTransparented uses `float` (32-bit) arithmetic.
    pub fn GetTransparented(self, amount: f64) -> emColor {
        let tp = (amount as f32).clamp(-100.0f32, 100.0f32) * 0.01f32;
        let a = self.GetAlpha() as f32;
        let new_a = if tp >= 1.0f32 {
            0u8
        } else if tp >= 0.0f32 {
            (a * (1.0f32 - tp) + 0.5f32) as u8
        } else if tp <= -1.0f32 {
            255u8
        } else {
            (a * (1.0f32 + tp) - 255.0f32 * tp + 0.5f32) as u8
        };
        self.SetAlpha(new_a)
    }
}

// DIVERGED: (language-forced) ToString — implemented as fmt::Display trait (Rust convention)
impl fmt::Display for emColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.IsOpaque() {
            write!(
                f,
                "#{:02X}{:02X}{:02X}",
                self.GetRed(),
                self.GetGreen(),
                self.GetBlue()
            )
        } else {
            write!(
                f,
                "#{:02X}{:02X}{:02X}{:02X}",
                self.GetRed(),
                self.GetGreen(),
                self.GetBlue(),
                self.GetAlpha()
            )
        }
    }
}

impl FromStr for emColor {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ColorParseError { _private: () };
        if !s.starts_with('#') {
            return Err(err());
        }
        let hex = &s[1..];
        match hex.len() {
            6 => {
                let val = u32::from_str_radix(hex, 16).map_err(|_| err())?;
                Ok(emColor::rgb((val >> 16) as u8, (val >> 8) as u8, val as u8))
            }
            8 => {
                let val = u32::from_str_radix(hex, 16).map_err(|_| err())?;
                Ok(emColor::rgba(
                    (val >> 24) as u8,
                    (val >> 16) as u8,
                    (val >> 8) as u8,
                    val as u8,
                ))
            }
            _ => Err(err()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_access() {
        let c = emColor::rgba(10, 20, 30, 40);
        assert_eq!(c.GetRed(), 10);
        assert_eq!(c.GetGreen(), 20);
        assert_eq!(c.GetBlue(), 30);
        assert_eq!(c.GetAlpha(), 40);
    }

    #[test]
    fn rgb_sets_alpha_255() {
        let c = emColor::rgb(1, 2, 3);
        assert_eq!(c.GetAlpha(), 255);
    }

    #[test]
    fn named_constants() {
        assert_eq!(emColor::BLACK, emColor::rgb(0, 0, 0));
        assert_eq!(emColor::WHITE, emColor::rgb(255, 255, 255));
        assert_eq!(emColor::TRANSPARENT.GetAlpha(), 0);
    }

    #[test]
    fn blend_extremes() {
        let a = emColor::rgb(255, 0, 0);
        let b = emColor::rgb(0, 0, 255);
        // Full alpha -> nearly source (C++ /256 precision: 255*255/256 = 254)
        let full = a.blend(b, 255);
        assert!((full.GetRed() as i16 - a.GetRed() as i16).abs() <= 1);
        assert!((full.GetBlue() as i16 - a.GetBlue() as i16).abs() <= 1);
        // Zero alpha -> dest
        assert_eq!(a.blend(b, 0), b);
    }

    #[test]
    fn canvas_blend_identity() {
        let target = emColor::rgb(100, 100, 100);
        // source == canvas -> no change
        let result = target.canvas_blend(emColor::rgb(50, 50, 50), emColor::rgb(50, 50, 50), 255);
        assert_eq!(result.GetRed(), 100);
        assert_eq!(result.GetGreen(), 100);
        assert_eq!(result.GetBlue(), 100);
    }

    #[test]
    fn hsv_round_trip() {
        let original = emColor::rgb(200, 100, 50);
        let (h, s, v) = original.GetHSV();
        let reconstructed = emColor::SetHSVA(h, s, v);
        // Allow ±1 due to rounding
        assert!((original.GetRed() as i16 - reconstructed.GetRed() as i16).abs() <= 1);
        assert!((original.GetGreen() as i16 - reconstructed.GetGreen() as i16).abs() <= 1);
        assert!((original.GetBlue() as i16 - reconstructed.GetBlue() as i16).abs() <= 1);
    }

    #[test]
    fn hsv_pure_colors() {
        let (h, s, v) = emColor::RED.GetHSV();
        assert!((h - 0.0).abs() < 1.0);
        assert!((s - 100.0).abs() < 1.0);
        assert!((v - 100.0).abs() < 1.0);

        let (h, _, _) = emColor::GREEN.GetHSV();
        assert!((h - 120.0).abs() < 1.0);

        let (h, _, _) = emColor::BLUE.GetHSV();
        assert!((h - 240.0).abs() < 1.0);
    }

    #[test]
    fn with_red_preserves_other_channels() {
        let c = emColor::rgba(10, 20, 30, 40).SetRed(99);
        assert_eq!(c.GetRed(), 99);
        assert_eq!(c.GetGreen(), 20);
        assert_eq!(c.GetBlue(), 30);
        assert_eq!(c.GetAlpha(), 40);
    }

    #[test]
    fn with_green_preserves_other_channels() {
        let c = emColor::rgba(10, 20, 30, 40).SetGreen(99);
        assert_eq!(c.GetRed(), 10);
        assert_eq!(c.GetGreen(), 99);
        assert_eq!(c.GetBlue(), 30);
        assert_eq!(c.GetAlpha(), 40);
    }

    #[test]
    fn with_blue_preserves_other_channels() {
        let c = emColor::rgba(10, 20, 30, 40).SetBlue(99);
        assert_eq!(c.GetRed(), 10);
        assert_eq!(c.GetGreen(), 20);
        assert_eq!(c.GetBlue(), 99);
        assert_eq!(c.GetAlpha(), 40);
    }

    #[test]
    fn query_methods() {
        assert!(emColor::TRANSPARENT.IsTotallyTransparent());
        assert!(!emColor::BLACK.IsTotallyTransparent());
        assert!(emColor::WHITE.IsOpaque());
        assert!(!emColor::rgba(0, 0, 0, 128).IsOpaque());
        assert!(emColor::SetGrey(128).IsGrey());
        assert!(!emColor::RED.IsGrey());
    }

    #[test]
    fn grey_round_trip() {
        let g = emColor::SetGrey(128);
        assert_eq!(g.GetRed(), 128);
        assert_eq!(g.GetGreen(), 128);
        assert_eq!(g.GetBlue(), 128);
        assert_eq!(g.GetAlpha(), 255);
        assert_eq!(g.GetGrey(), 128);
    }

    #[test]
    fn to_grey_averages() {
        let c = emColor::rgb(10, 20, 30);
        assert_eq!(c.GetGrey(), 20); // (10+20+30)/3 = 20
    }

    #[test]
    fn with_hue_preserves_sv() {
        let c = emColor::SetHSVA(120.0, 80.0, 60.0);
        let shifted = c.SetHue(240.0);
        let (h, s, v) = shifted.GetHSV();
        assert!((h - 240.0).abs() < 2.0);
        assert!((s - 80.0).abs() < 2.0);
        assert!((v - 60.0).abs() < 2.0);
    }

    #[test]
    fn with_saturation_preserves_hv() {
        let c = emColor::SetHSVA(120.0, 80.0, 60.0);
        let changed = c.SetSat(30.0);
        let (h, s, v) = changed.GetHSV();
        assert!((h - 120.0).abs() < 2.0);
        assert!((s - 30.0).abs() < 2.0);
        assert!((v - 60.0).abs() < 2.0);
    }

    #[test]
    fn with_value_preserves_hs() {
        let c = emColor::SetHSVA(120.0, 80.0, 60.0);
        let changed = c.SetVal(90.0);
        let (h, s, v) = changed.GetHSV();
        assert!((h - 120.0).abs() < 2.0);
        assert!((s - 80.0).abs() < 2.0);
        assert!((v - 90.0).abs() < 2.0);
    }

    #[test]
    fn with_hue_preserves_alpha() {
        let c = emColor::rgba(100, 50, 50, 128);
        let shifted = c.SetHue(180.0);
        assert_eq!(shifted.GetAlpha(), 128);
    }

    #[test]
    fn transparented_extremes() {
        let c = emColor::rgba(100, 100, 100, 200);
        let fully = c.GetTransparented(100.0);
        assert_eq!(fully.GetAlpha(), 0);
        let none = c.GetTransparented(0.0);
        assert_eq!(none.GetAlpha(), 200);
        let opaque = emColor::rgba(100, 100, 100, 0).GetTransparented(-100.0);
        assert_eq!(opaque.GetAlpha(), 255);
    }

    #[test]
    fn display_opaque() {
        assert_eq!(format!("{}", emColor::rgb(255, 128, 0)), "#FF8000");
    }

    #[test]
    fn display_with_alpha() {
        assert_eq!(format!("{}", emColor::rgba(255, 128, 0, 128)), "#FF800080");
    }

    #[test]
    fn from_str_round_trip() {
        let c = emColor::rgba(10, 200, 30, 128);
        let s = format!("{}", c);
        let parsed: emColor = s.parse().unwrap();
        assert_eq!(parsed, c);

        let opaque = emColor::rgb(255, 0, 128);
        let s2 = format!("{}", opaque);
        let parsed2: emColor = s2.parse().unwrap();
        assert_eq!(parsed2, opaque);
    }

    #[test]
    fn test_blend_interpolates_alpha() {
        // blend(self, other, alpha) uses /256 math on all 4 channels including alpha.
        // self=RGBA(100,100,100,200), other=RGBA(200,200,200,50), alpha=128
        let a = emColor::rgba(100, 100, 100, 200);
        let b = emColor::rgba(200, 200, 200, 50);
        let result = a.blend(b, 128);

        // Expected: out_ch = (self_ch * 128 + other_ch * (256-128)) >> 8
        let expected_a = ((200u16 * 128 + 50u16 * 128) >> 8) as u8;
        assert_eq!(
            result.GetAlpha(),
            expected_a,
            "blend alpha: got {} expected {}",
            result.GetAlpha(),
            expected_a
        );

        // Verify RGB uses the same formula (sanity)
        let expected_r = ((100u16 * 128 + 200u16 * 128) >> 8) as u8;
        assert_eq!(result.GetRed(), expected_r);
    }

    #[test]
    fn test_lerp_interpolates_alpha() {
        let a = emColor::rgba(0, 0, 0, 0);
        let b = emColor::rgba(255, 255, 255, 255);
        let result = a.GetBlended(b, 50.0);

        // C++ formula: w2 = (50.0 * 655.36 + 0.5) as i32 = 32768
        // mix(0, 255) = (0 * (65536-32768) + 255 * 32768 + 32768) >> 16
        //             = (8355840 + 32768) >> 16 = 8388608 >> 16 = 128
        assert_eq!(
            result.GetAlpha(),
            128,
            "lerp alpha at weight=50: got {} expected 128",
            result.GetAlpha()
        );
        assert_eq!(result.GetRed(), result.GetAlpha());
        assert_eq!(result.GetGreen(), result.GetAlpha());
        assert_eq!(result.GetBlue(), result.GetAlpha());

        // Verify endpoints
        let at_zero = a.GetBlended(b, 0.0);
        assert_eq!(at_zero.GetAlpha(), 0, "lerp alpha at weight=0 should be 0");
        let at_hundred = a.GetBlended(b, 100.0);
        assert_eq!(
            at_hundred.GetAlpha(),
            255,
            "lerp alpha at weight=100 should be 255"
        );
    }

    #[test]
    fn test_canvas_blend_computes_alpha() {
        // canvas_blend applies the blend_ch formula to all 4 channels including alpha.
        // target=RGBA(100,100,100,200), source=RGBA(200,200,200,180),
        // canvas=RGBA(80,80,80,255), alpha=128
        let target = emColor::rgba(100, 100, 100, 200);
        let source = emColor::rgba(200, 200, 200, 180);
        let canvas = emColor::rgba(80, 80, 80, 255);
        let result = target.canvas_blend(source, canvas, 128);

        // blend_ch for alpha: target_a + round(src_a * alpha / 255) - round(cvs_a * alpha / 255)
        let src_term = (180i32 * 128 + 127) / 255; // = 90
        let cvs_term = (255i32 * 128 + 127) / 255; // = 128
        let expected_a = (200i32 + src_term - cvs_term).clamp(0, 255) as u8; // 200 + 90 - 128 = 162

        assert_eq!(
            result.GetAlpha(),
            expected_a,
            "canvas_blend alpha: got {} expected {}",
            result.GetAlpha(),
            expected_a
        );

        // Verify it's different from input (the blend did modify alpha)
        assert_ne!(
            result.GetAlpha(),
            target.GetAlpha(),
            "canvas_blend should modify alpha channel"
        );
    }

    #[test]
    fn from_str_rejects_invalid() {
        assert!("not a color".parse::<emColor>().is_err());
        assert!("#GG0000".parse::<emColor>().is_err());
        assert!("#12345".parse::<emColor>().is_err());
        assert!("#123456789".parse::<emColor>().is_err());
        assert!("".parse::<emColor>().is_err());
    }

    #[test]
    fn test_get_lighted() {
        let c = emColor::rgb(100, 100, 100);

        // Positive = lighten (blend toward white)
        let lighter = c.GetLighted(50.0);
        // GetLighted(50) = GetBlended(WHITE, 50.0)
        let expected = c.GetBlended(emColor::WHITE.SetAlpha(c.GetAlpha()), 50.0);
        assert_eq!(lighter.GetPacked(), expected.GetPacked());

        // Negative = darken (blend toward black)
        let darker = c.GetLighted(-50.0);
        let expected = c.GetBlended(emColor::rgba(0, 0, 0, c.GetAlpha()), 50.0);
        assert_eq!(darker.GetPacked(), expected.GetPacked());

        // Zero = no change
        let same = c.GetLighted(0.0);
        assert_eq!(same.GetPacked(), c.GetPacked());
    }

    #[test]
    fn test_individual_hsv_accessors() {
        let c = emColor::rgb(255, 0, 0); // pure red
        assert!((c.GetHue() - 0.0).abs() < 1.0);
        assert!((c.GetSat() - 100.0).abs() < 1.0);
        assert!((c.GetVal() - 100.0).abs() < 1.0);

        // Verify consistency with GetHSV tuple
        let (h, s, v) = c.GetHSV();
        assert_eq!(c.GetHue(), h);
        assert_eq!(c.GetSat(), s);
        assert_eq!(c.GetVal(), v);
    }

    #[test]
    fn test_set_hsva_4_param() {
        let c = emColor::SetHSVA_with_alpha(0.0, 100.0, 100.0, 128);
        assert_eq!(c.GetAlpha(), 128);
        assert_eq!(c.GetRed(), 255);
    }

    #[test]
    fn test_set_grey_2_param() {
        let c = emColor::SetGrey_with_alpha(128, 200);
        assert_eq!(c.GetRed(), 128);
        assert_eq!(c.GetGreen(), 128);
        assert_eq!(c.GetBlue(), 128);
        assert_eq!(c.GetAlpha(), 200);
    }
}
