// SPLIT: Split from emPainter.h — scanline tool functions extracted
use crate::emColor::emColor;

/// C++ MaxInterpolationBytesAtOnce = 1024.
/// Buffer is always 1024 bytes; pixel count = 1024 / channel_count.
pub(crate) const MAX_INTERP_BYTES: usize = 1024;

/// Stack-allocated interpolation buffer matching C++ ScanlineTool's 1024-byte buffer.
/// Holds up to `MAX_INTERP_BYTES / ch` pixels of interpolated source data.
pub struct InterpolationBuffer {
    data: [u8; MAX_INTERP_BYTES],
    len: usize,
    ch: u8,
}

impl InterpolationBuffer {
    #[inline]
    pub fn new(ch: u8) -> Self {
        Self {
            data: [0u8; MAX_INTERP_BYTES],
            len: 0,
            ch,
        }
    }

    #[inline]
    pub fn max_pixels(&self) -> usize {
        MAX_INTERP_BYTES / self.ch as usize
    }

    #[inline]
    pub fn set_len(&mut self, n: usize) {
        debug_assert!(n <= self.max_pixels());
        self.len = n;
    }

    /// Raw data slice for the first `len` pixels. Length = len * ch bytes.
    #[inline]
    pub fn raw_data(&self) -> &[u8] {
        &self.data[..self.len * self.ch as usize]
    }

    /// Expand pixel `i` to RGBA for blending. Missing channels filled per C++ convention.
    #[inline]
    pub fn pixel_rgba(&self, i: usize) -> [u8; 4] {
        let ch = self.ch as usize;
        let off = i * ch;
        match ch {
            1 => {
                let g = self.data[off];
                [g, g, g, 255]
            }
            3 => [self.data[off], self.data[off + 1], self.data[off + 2], 255],
            4 => [
                self.data[off],
                self.data[off + 1],
                self.data[off + 2],
                self.data[off + 3],
            ],
            _ => [0, 0, 0, 0],
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, i: usize, rgba: [u8; 4]) {
        let ch = self.ch as usize;
        let off = i * ch;
        match ch {
            1 => self.data[off] = rgba[0],
            3 => {
                self.data[off] = rgba[0];
                self.data[off + 1] = rgba[1];
                self.data[off + 2] = rgba[2];
            }
            4 => {
                self.data[off] = rgba[0];
                self.data[off + 1] = rgba[1];
                self.data[off + 2] = rgba[2];
                self.data[off + 3] = rgba[3];
            }
            _ => {}
        }
    }

}

/// Blend mode determined once per paint call. Controls which blend *path*
/// (canvas vs source-over), not the alpha value (which varies per pixel).
pub enum BlendMode {
    /// Canvas-color compositing (C++ HAVE_CVC). Writes RGB only.
    CanvasBlend { canvas: emColor, painter_alpha: u8 },
    /// Standard source-over alpha compositing. Writes RGBA.
    SourceOver { painter_alpha: u8 },
}

impl BlendMode {
    /// Construct from current painter state (after canvas/alpha overrides).
    pub(crate) fn from_state(canvas_color: emColor, alpha: u8) -> Self {
        if canvas_color.IsOpaque() {
            BlendMode::CanvasBlend {
                canvas: canvas_color,
                painter_alpha: alpha,
            }
        } else {
            BlendMode::SourceOver {
                painter_alpha: alpha,
            }
        }
    }
}

// ── Blend functions ────────────────────────────────────────────────

/// Blend `count` straight-alpha RGBA pixels from `buf` onto destination.
/// Matches `blend_pixel_unchecked` + `blend_with_coverage_unchecked` exactly.
///
/// Coverage application:
/// - cov >= 0x1000: use source alpha as-is
/// - cov > 0: adjusted_alpha = (src_alpha * cov + 0x800) >> 12
/// - cov <= 0: skip pixel
///
/// Then combined_alpha = (adjusted_alpha * painter_alpha + 127) / 255.
pub(crate) fn blend_scanline(
    dest: &mut [u8],
    buf: &InterpolationBuffer,
    count: usize,
    coverages: Option<&[i32]>,
    mode: &BlendMode,
) {
    match mode {
        BlendMode::CanvasBlend {
            canvas,
            painter_alpha,
        } => blend_scanline_canvas(dest, buf, count, coverages, *canvas, *painter_alpha),
        BlendMode::SourceOver { painter_alpha } => {
            blend_scanline_source_over(dest, buf, count, coverages, *painter_alpha)
        }
    }
}

/// Canvas-blend path: writes RGB only (dest alpha unchanged).
fn blend_scanline_canvas(
    dest: &mut [u8],
    buf: &InterpolationBuffer,
    count: usize,
    coverages: Option<&[i32]>,
    canvas: emColor,
    painter_alpha: u8,
) {
    for i in 0..count {
        let cov = coverages.map_or(0x1000, |c| c[i]);
        if cov <= 0 {
            continue;
        }

        let src = buf.pixel_rgba(i);
        let src_a = src[3];

        // Apply coverage to source alpha
        let adjusted_a = if cov >= 0x1000 {
            src_a
        } else {
            ((src_a as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8
        };

        // Combine with painter alpha
        let combined_alpha = if painter_alpha == 255 {
            adjusted_a
        } else {
            ((adjusted_a as u32 * painter_alpha as u32 + 127) / 255) as u8
        };

        if combined_alpha == 0 {
            continue;
        }

        let off = i * 4;
        let existing = emColor::rgba(dest[off], dest[off + 1], dest[off + 2], dest[off + 3]);
        let src_color = emColor::rgba(src[0], src[1], src[2], adjusted_a);
        let result = existing.canvas_blend(src_color, canvas, combined_alpha);
        dest[off] = result.GetRed();
        dest[off + 1] = result.GetGreen();
        dest[off + 2] = result.GetBlue();
        // Canvas blend: dest alpha unchanged
    }
}

/// Source-over path: writes RGBA.
fn blend_scanline_source_over(
    dest: &mut [u8],
    buf: &InterpolationBuffer,
    count: usize,
    coverages: Option<&[i32]>,
    painter_alpha: u8,
) {
    use super::emColor::blend_hash_lookup;

    // AVX2 fast path: full coverage, painter_alpha=255, 4-channel buffer.
    #[cfg(target_arch = "x86_64")]
    if coverages.is_none()
        && painter_alpha == 255
        && buf.ch == 4
        && is_x86_feature_detected!("avx2")
    {
        unsafe {
            super::emPainterScanlineAvx2::blend_source_over_avx2(dest, buf.raw_data(), count);
        }
        return;
    }

    for i in 0..count {
        let cov = coverages.map_or(0x1000, |c| c[i]);
        if cov <= 0 {
            continue;
        }

        let src = buf.pixel_rgba(i);
        let src_a = src[3];

        // Apply coverage to source alpha
        let adjusted_a = if cov >= 0x1000 {
            src_a
        } else {
            ((src_a as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8
        };

        let ea = if painter_alpha == 255 {
            adjusted_a as u16
        } else {
            ((adjusted_a as u32 * painter_alpha as u32 + 127) / 255) as u16
        };

        if ea == 0 {
            continue;
        }

        let off = i * 4;

        // Opaque fast path
        if ea >= 255 {
            dest[off] = src[0];
            dest[off + 1] = src[1];
            dest[off + 2] = src[2];
            dest[off + 3] = 255;
            continue;
        }

        // Background: Blinn div255 (matches C++).
        // Source: C++ hash table lookup (emPainter_ScTlPSCol.cpp:119).
        let alpha = ea as u8;
        let t = (255 - alpha as u32) * 257;
        dest[off] = (((dest[off] as u32 * t + 0x8073) >> 16)
            + blend_hash_lookup(src[0], alpha) as u32) as u8;
        dest[off + 1] = (((dest[off + 1] as u32 * t + 0x8073) >> 16)
            + blend_hash_lookup(src[1], alpha) as u32) as u8;
        dest[off + 2] = (((dest[off + 2] as u32 * t + 0x8073) >> 16)
            + blend_hash_lookup(src[2], alpha) as u32) as u8;
        dest[off + 3] = (((dest[off + 3] as u32 * t + 0x8073) >> 16)
            + blend_hash_lookup(255, alpha) as u32) as u8;
    }
}

/// Blend `count` premultiplied-alpha RGBA pixels from `buf` onto destination.
/// Matches `blend_pixel_premul_unchecked` + `blend_premul_with_coverage_unchecked`.
///
/// Coverage in premul path: o_eff = (cov * painter_alpha + 127) / 255,
/// then all 4 premul channels scaled by (ch * o_eff + 0x800) >> 12.
pub(crate) fn blend_scanline_premul(
    dest: &mut [u8],
    buf: &InterpolationBuffer,
    count: usize,
    coverages: Option<&[i32]>,
    mode: &BlendMode,
) {
    match mode {
        BlendMode::CanvasBlend {
            canvas,
            painter_alpha,
        } => blend_scanline_premul_canvas(dest, buf, count, coverages, *canvas, *painter_alpha),
        BlendMode::SourceOver { painter_alpha } => {
            blend_scanline_premul_source_over(dest, buf, count, coverages, *painter_alpha)
        }
    }
}

/// Premul canvas-blend path.
fn blend_scanline_premul_canvas(
    dest: &mut [u8],
    buf: &InterpolationBuffer,
    count: usize,
    coverages: Option<&[i32]>,
    canvas: emColor,
    painter_alpha: u8,
) {
    use super::emColor::blend_hash_lookup;

    for i in 0..count {
        let cov = coverages.map_or(0x1000, |c| c[i]);
        if cov <= 0 {
            continue;
        }

        let src = buf.pixel_rgba(i);

        // Apply coverage + painter_alpha
        let o_eff = if painter_alpha == 255 {
            cov
        } else {
            (cov * painter_alpha as i32 + 127) / 255
        };

        let pm = if o_eff >= 0x1000 {
            src
        } else if o_eff > 0 {
            [
                ((src[0] as i32 * o_eff + 0x800) >> 12) as u8,
                ((src[1] as i32 * o_eff + 0x800) >> 12) as u8,
                ((src[2] as i32 * o_eff + 0x800) >> 12) as u8,
                ((src[3] as i32 * o_eff + 0x800) >> 12) as u8,
            ]
        } else {
            continue;
        };

        let a = pm[3];
        if a == 0 {
            continue;
        }

        let off = i * 4;
        // Canvas-blend: C++ does packed u32 arithmetic with wrapping.
        // pix = hR[sr] + hG[sg] + hB[sb]          (source contribution, shifted)
        // pix -= hcR[a] + hcG[a] + hcB[a]          (canvas contribution, shifted)
        // *p += pix                                  (wrapping add to dest pixel)
        //
        // On little-endian with OPFI_8888_0BGR layout [R:0, G:8, B:16, 0:24],
        // the packed u32 is: R | (G << 8) | (B << 16).
        // Carries between channels propagate via wrapping, matching C++ exactly.
        //
        // C++ reference: emPainter_ScTlPSInt.cpp lines 369-371 (HAVE_CVC path).
        let pix_r = pm[0] as u32;
        let pix_g = pm[1] as u32;
        let pix_b = pm[2] as u32;
        let pix: u32 = pix_r | (pix_g << 8) | (pix_b << 16);

        let cr = blend_hash_lookup(canvas.GetRed(), a) as u32;
        let cg = blend_hash_lookup(canvas.GetGreen(), a) as u32;
        let cb = blend_hash_lookup(canvas.GetBlue(), a) as u32;
        let cvs: u32 = cr | (cg << 8) | (cb << 16);

        let dest_packed: u32 = dest[off] as u32
            | ((dest[off + 1] as u32) << 8)
            | ((dest[off + 2] as u32) << 16);
        let result = dest_packed.wrapping_add(pix.wrapping_sub(cvs));
        dest[off] = result as u8;
        dest[off + 1] = (result >> 8) as u8;
        dest[off + 2] = (result >> 16) as u8;
        // Canvas blend: dest alpha unchanged
    }
}

/// Premul source-over path.
fn blend_scanline_premul_source_over(
    dest: &mut [u8],
    buf: &InterpolationBuffer,
    count: usize,
    coverages: Option<&[i32]>,
    painter_alpha: u8,
) {
    // AVX2 fast path: full coverage, painter_alpha=255, 4-channel buffer.
    #[cfg(target_arch = "x86_64")]
    if coverages.is_none()
        && painter_alpha == 255
        && buf.ch == 4
        && is_x86_feature_detected!("avx2")
    {
        unsafe {
            super::emPainterScanlineAvx2::blend_premul_source_over_avx2(dest, buf.raw_data(), count);
        }
        return;
    }

    for i in 0..count {
        let cov = coverages.map_or(0x1000, |c| c[i]);
        if cov <= 0 {
            continue;
        }

        let src = buf.pixel_rgba(i);

        let o_eff = if painter_alpha == 255 {
            cov
        } else {
            (cov * painter_alpha as i32 + 127) / 255
        };

        let pm = if o_eff >= 0x1000 {
            src
        } else if o_eff > 0 {
            [
                ((src[0] as i32 * o_eff + 0x800) >> 12) as u8,
                ((src[1] as i32 * o_eff + 0x800) >> 12) as u8,
                ((src[2] as i32 * o_eff + 0x800) >> 12) as u8,
                ((src[3] as i32 * o_eff + 0x800) >> 12) as u8,
            ]
        } else {
            continue;
        };

        let a = pm[3] as u32;
        if a == 0 {
            continue;
        }

        let off = i * 4;

        if a >= 255 {
            dest[off] = pm[0];
            dest[off + 1] = pm[1];
            dest[off + 2] = pm[2];
            dest[off + 3] = 255;
            continue;
        }

        // Blinn div255: (x * 257 + 0x8073) >> 16
        let t = (255 - a) * 257;
        dest[off] = (((dest[off] as u32 * t + 0x8073) >> 16) + pm[0] as u32) as u8;
        dest[off + 1] = (((dest[off + 1] as u32 * t + 0x8073) >> 16) + pm[1] as u32) as u8;
        dest[off + 2] = (((dest[off + 2] as u32 * t + 0x8073) >> 16) + pm[2] as u32) as u8;
        dest[off + 3] = (((dest[off + 3] as u32 * t + 0x8073) >> 16) + a) as u8;
    }
}

/// Fused color-mapping + compositing for IMAGE_COLORED (font glyphs).
///
/// Literal port of C++ `PaintScanlineIntG1`, `PaintScanlineIntG2`,
/// `PaintScanlineIntG1G2` for CHANNELS=1, PIXEL_SIZE=4.
///
/// Unlike `blend_scanline` (which takes premapped RGBA source pixels),
/// this function takes raw grayscale luminance values and two gradient
/// endpoint colors, fusing the color mapping and compositing into one
/// step. This matches C++ integer rounding exactly.
///
/// # Parameters
/// - `dest`: destination RGBA pixel buffer (4 bytes per pixel)
/// - `lums`: grayscale values, one per pixel (from interpolation)
/// - `count`: number of pixels to process
/// - `coverages`: per-pixel coverage values in [0, 0x1000]; None = all full
/// - `color1`: gradient color for luminance=0 (background color)
/// - `color2`: gradient color for luminance=255 (foreground color)
/// - `mode`: blend mode (canvas or source-over, with painter_alpha)
#[allow(clippy::too_many_arguments)]
pub fn blend_colored_scanline(
    dest: &mut [u8],
    lums: &[u8],
    count: usize,
    coverages: Option<&[i32]>,
    color1: emColor,
    color2: emColor,
    mode: &BlendMode,
) {
    use super::emColor::blend_hash_lookup;

    let c1_transparent = color1.GetAlpha() == 0;
    let c2_transparent = color2.GetAlpha() == 0;

    // Extract painter_alpha and canvas from the blend mode
    let (canvas_opt, painter_alpha) = match mode {
        BlendMode::CanvasBlend {
            canvas,
            painter_alpha,
        } => (Some(*canvas), *painter_alpha),
        BlendMode::SourceOver { painter_alpha } => (None, *painter_alpha),
    };

    // Pre-extract color components for use in inner loop
    let c1r = color1.GetRed();
    let c1g = color1.GetGreen();
    let c1b = color1.GetBlue();
    let c2r = color2.GetRed();
    let c2g = color2.GetGreen();
    let c2b = color2.GetBlue();
    let c1a = color1.GetAlpha();
    let c2a = color2.GetAlpha();

    for i in 0..count {
        let cov = coverages.map_or(0x1000i32, |c| c[i]);
        if cov <= 0 {
            continue;
        }

        let g = lums[i] as u32;

        // Compute effective opacity: apply painter_alpha to coverage
        // In C++, HAVE_ALPHA is 0 for IMAGE_COLORED, so sct.Alpha is not used.
        // But painter_alpha may be < 255 if the Rust painter state has it set.
        let o = if painter_alpha < 255 {
            (cov * painter_alpha as i32 + 127) / 255
        } else {
            cov
        };

        // Compute per-color opacity: o1 for color1, o2 for color2
        let o1 = if !c1_transparent {
            (o * c1a as i32 + 127) / 255
        } else {
            0
        };
        let o2 = if !c2_transparent {
            (o * c2a as i32 + 127) / 255
        } else {
            0
        };

        // --- Variant dispatch: G1, G2, or G1G2 ---
        // pix_r, pix_g, pix_b are the premultiplied hash-table-blended pixel values.
        // `a` is the composite alpha for compositing.
        let (pix_r, pix_g, pix_b, a): (u8, u8, u8, u32);

        if c1_transparent && !c2_transparent {
            // === G2 variant: color1 transparent ===
            if o2 < 0x1000 {
                // Partial opacity
                let a_val = (g as i32 * o2 + 0x800) >> 12;
                if a_val == 0 {
                    continue;
                }
                let a8 = a_val as u8;
                pix_r = blend_hash_lookup(c2r, a8);
                pix_g = blend_hash_lookup(c2g, a8);
                pix_b = blend_hash_lookup(c2b, a8);
                a = a_val as u32;
            } else {
                // Full opacity (o2 >= 0x1000)
                let a_val = g;
                if a_val == 0 {
                    continue;
                }
                pix_r = blend_hash_lookup(c2r, a_val as u8);
                pix_g = blend_hash_lookup(c2g, a_val as u8);
                pix_b = blend_hash_lookup(c2b, a_val as u8);
                a = a_val;
            }
        } else if !c1_transparent && c2_transparent {
            // === G1 variant: color2 transparent ===
            if o1 < 0x1000 {
                // Partial opacity
                // In C++ for CHANNELS=1, a=255 (since no alpha channel in source)
                let a_val = ((255 - g) as i32 * o1 + 0x800) >> 12;
                if a_val == 0 {
                    continue;
                }
                let a8 = a_val as u8;
                pix_r = blend_hash_lookup(c1r, a8);
                pix_g = blend_hash_lookup(c1g, a8);
                pix_b = blend_hash_lookup(c1b, a8);
                a = a_val as u32;
            } else {
                // Full opacity
                let a_val = 255 - g;
                if a_val == 0 {
                    continue;
                }
                pix_r = blend_hash_lookup(c1r, a_val as u8);
                pix_g = blend_hash_lookup(c1g, a_val as u8);
                pix_b = blend_hash_lookup(c1b, a_val as u8);
                a = a_val;
            }
        } else if !c1_transparent && !c2_transparent {
            // === G1G2 variant: both colors present ===
            if o1 < 0x1000 || o2 < 0x1000 {
                // Partial opacity
                let a1 = ((255 - g) as i32 * o1 + 0x800) >> 12;
                let a2 = (g as i32 * o2 + 0x800) >> 12;
                let a_val = (a1 + a2) as u32;
                if a_val == 0 {
                    continue;
                }
                // C++ uses blend_hash_lookup(255, blended_channel) for each channel
                pix_r = blend_hash_lookup(
                    255,
                    (((c1r as u32 * a1 as u32 + c2r as u32 * a2 as u32) * 257 + 0x8073) >> 16)
                        as u8,
                );
                pix_g = blend_hash_lookup(
                    255,
                    (((c1g as u32 * a1 as u32 + c2g as u32 * a2 as u32) * 257 + 0x8073) >> 16)
                        as u8,
                );
                pix_b = blend_hash_lookup(
                    255,
                    (((c1b as u32 * a1 as u32 + c2b as u32 * a2 as u32) * 257 + 0x8073) >> 16)
                        as u8,
                );
                a = a_val;
            } else {
                // Full opacity: both o1 >= 0x1000 and o2 >= 0x1000
                // a=255 (for CHANNELS=1, a=255 in C++)
                // C++ full path: g directly used
                pix_r = blend_hash_lookup(
                    255,
                    (((c1r as u32 * (255 - g) + c2r as u32 * g) * 257 + 0x8073) >> 16) as u8,
                );
                pix_g = blend_hash_lookup(
                    255,
                    (((c1g as u32 * (255 - g) + c2g as u32 * g) * 257 + 0x8073) >> 16) as u8,
                );
                pix_b = blend_hash_lookup(
                    255,
                    (((c1b as u32 * (255 - g) + c2b as u32 * g) * 257 + 0x8073) >> 16) as u8,
                );
                a = 255;
            }
        } else {
            // Both colors transparent — nothing to draw
            continue;
        }

        // --- Compositing ---
        let off = i * 4;

        if a >= 255 {
            // Opaque fast path
            dest[off] = pix_r;
            dest[off + 1] = pix_g;
            dest[off + 2] = pix_b;
            if canvas_opt.is_none() {
                dest[off + 3] = 255;
            }
            // Canvas mode: dest alpha unchanged
            continue;
        }

        let a8 = a as u8;

        if let Some(canvas) = canvas_opt {
            // Canvas blend (HAVE_CVC)
            let cr = blend_hash_lookup(canvas.GetRed(), a8) as i32;
            let cg = blend_hash_lookup(canvas.GetGreen(), a8) as i32;
            let cb = blend_hash_lookup(canvas.GetBlue(), a8) as i32;
            dest[off] = (dest[off] as i32 + pix_r as i32 - cr).clamp(0, 255) as u8;
            dest[off + 1] = (dest[off + 1] as i32 + pix_g as i32 - cg).clamp(0, 255) as u8;
            dest[off + 2] = (dest[off + 2] as i32 + pix_b as i32 - cb).clamp(0, 255) as u8;
            // Canvas blend: dest alpha unchanged
        } else {
            // Source-over (no canvas)
            let t = (255 - a) * 257;
            dest[off] =
                (((dest[off] as u32 * t + 0x8073) >> 16) + pix_r as u32) as u8;
            dest[off + 1] =
                (((dest[off + 1] as u32 * t + 0x8073) >> 16) + pix_g as u32) as u8;
            dest[off + 2] =
                (((dest[off + 2] as u32 * t + 0x8073) >> 16) + pix_b as u32) as u8;
            dest[off + 3] =
                (((dest[off + 3] as u32 * t + 0x8073) >> 16) + blend_hash_lookup(255, a8) as u32)
                    as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emImage::emImage;

    /// Build a minimal painter-like setup for testing blend equivalence.
    /// Returns (dest image data as Vec<u8>, target_width).
    fn make_dest(width: u32, height: u32, fill: emColor) -> Vec<u8> {
        let mut data = vec![0u8; (width * height * 4) as usize];
        for chunk in data.chunks_exact_mut(4) {
            chunk[0] = fill.GetRed();
            chunk[1] = fill.GetGreen();
            chunk[2] = fill.GetBlue();
            chunk[3] = fill.GetAlpha();
        }
        data
    }

    /// Reference per-pixel blend matching blend_pixel_unchecked (source-over).
    fn ref_blend_source_over(dest: &mut [u8], color: emColor, painter_alpha: u8) {
        let ca = color.GetAlpha() as u16;
        let ea = if painter_alpha == 255 {
            ca
        } else {
            ((ca as u32 * painter_alpha as u32 + 127) / 255) as u16
        };
        if ea == 0 {
            return;
        }
        if ea >= 255 {
            dest[0] = color.GetRed();
            dest[1] = color.GetGreen();
            dest[2] = color.GetBlue();
            dest[3] = 255;
            return;
        }
        let alpha = ea as u32;
        let t = (255 - alpha) * 257;
        dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16)
            + ((color.GetRed() as u32 * alpha + 127) / 255)) as u8;
        dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16)
            + ((color.GetGreen() as u32 * alpha + 127) / 255)) as u8;
        dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16)
            + ((color.GetBlue() as u32 * alpha + 127) / 255)) as u8;
        dest[3] = (((dest[3] as u32 * t + 0x8073) >> 16)
            + ((255u32 * alpha + 127) / 255)) as u8;
    }

    /// Reference per-pixel blend matching blend_pixel_unchecked (canvas).
    fn ref_blend_canvas(dest: &mut [u8], color: emColor, canvas: emColor, painter_alpha: u8) {
        let combined_alpha = if painter_alpha == 255 {
            color.GetAlpha()
        } else {
            ((color.GetAlpha() as u32 * painter_alpha as u32 + 127) / 255) as u8
        };
        if combined_alpha == 0 {
            return;
        }
        let existing = emColor::rgba(dest[0], dest[1], dest[2], dest[3]);
        let result = existing.canvas_blend(color, canvas, combined_alpha);
        dest[0] = result.GetRed();
        dest[1] = result.GetGreen();
        dest[2] = result.GetBlue();
    }

    /// Reference premul blend matching blend_pixel_premul_unchecked (source-over).
    fn ref_blend_premul_source_over(dest: &mut [u8], pm: [u8; 4]) {
        let a = pm[3] as u32;
        if a == 0 {
            return;
        }
        if a >= 255 {
            dest[0] = pm[0];
            dest[1] = pm[1];
            dest[2] = pm[2];
            dest[3] = 255;
            return;
        }
        let t = (255 - a) * 257;
        dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + pm[0] as u32) as u8;
        dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + pm[1] as u32) as u8;
        dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + pm[2] as u32) as u8;
        dest[3] = (((dest[3] as u32 * t + 0x8073) >> 16) + a) as u8;
    }

    /// Reference premul blend matching blend_pixel_premul_unchecked (canvas).
    fn ref_blend_premul_canvas(dest: &mut [u8], pm: [u8; 4], canvas: emColor) {
        use crate::emColor::blend_hash_lookup;
        let a = pm[3];
        if a == 0 {
            return;
        }
        // C++ uses packed u32 wrapping arithmetic — carries propagate between
        // channels intentionally. Match that, not per-channel clamping.
        let pix: u32 = pm[0] as u32 | ((pm[1] as u32) << 8) | ((pm[2] as u32) << 16);
        let cr = blend_hash_lookup(canvas.GetRed(), a) as u32;
        let cg = blend_hash_lookup(canvas.GetGreen(), a) as u32;
        let cb = blend_hash_lookup(canvas.GetBlue(), a) as u32;
        let cvs: u32 = cr | (cg << 8) | (cb << 16);
        let dest_packed: u32 =
            dest[0] as u32 | ((dest[1] as u32) << 8) | ((dest[2] as u32) << 16);
        let result = dest_packed.wrapping_add(pix.wrapping_sub(cvs));
        dest[0] = result as u8;
        dest[1] = (result >> 8) as u8;
        dest[2] = (result >> 16) as u8;
    }

    #[test]
    fn blend_scanline_source_over_matches_perpixel() {
        let colors = [
            emColor::rgba(255, 0, 0, 255),   // opaque red
            emColor::rgba(0, 255, 0, 128),    // semi-transparent green
            emColor::rgba(0, 0, 255, 0),      // fully transparent
            emColor::rgba(200, 100, 50, 200), // arbitrary
        ];
        let bg = emColor::rgba(100, 100, 100, 200);
        let painter_alpha = 200u8;

        // Fill buffer
        let mut buf = InterpolationBuffer::new(4);
        for (i, c) in colors.iter().enumerate() {
            buf.set_pixel(i, [c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha()]);
        }
        buf.set_len(colors.len());

        // Scanline blend
        let mut dest_scan = make_dest(colors.len() as u32, 1, bg);
        let mode = BlendMode::SourceOver { painter_alpha };
        blend_scanline(&mut dest_scan, &buf, colors.len(), None, &mode);

        // Reference per-pixel blend
        let mut dest_ref = make_dest(colors.len() as u32, 1, bg);
        for (i, c) in colors.iter().enumerate() {
            ref_blend_source_over(&mut dest_ref[i * 4..(i + 1) * 4], *c, painter_alpha);
        }

        assert_eq!(dest_scan, dest_ref, "source-over scanline vs per-pixel mismatch");
    }

    #[test]
    fn blend_scanline_canvas_matches_perpixel() {
        let colors = [
            emColor::rgba(255, 0, 0, 255),
            emColor::rgba(0, 255, 0, 128),
            emColor::rgba(0, 0, 255, 0),
            emColor::rgba(200, 100, 50, 200),
        ];
        let bg = emColor::rgba(100, 100, 100, 200);
        let canvas = emColor::rgba(50, 50, 50, 255);
        let painter_alpha = 200u8;

        let mut buf = InterpolationBuffer::new(4);
        for (i, c) in colors.iter().enumerate() {
            buf.set_pixel(i, [c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha()]);
        }
        buf.set_len(colors.len());

        let mut dest_scan = make_dest(colors.len() as u32, 1, bg);
        let mode = BlendMode::CanvasBlend {
            canvas,
            painter_alpha,
        };
        blend_scanline(&mut dest_scan, &buf, colors.len(), None, &mode);

        let mut dest_ref = make_dest(colors.len() as u32, 1, bg);
        for (i, c) in colors.iter().enumerate() {
            ref_blend_canvas(&mut dest_ref[i * 4..(i + 1) * 4], *c, canvas, painter_alpha);
        }

        assert_eq!(dest_scan, dest_ref, "canvas scanline vs per-pixel mismatch");
    }

    #[test]
    fn blend_scanline_with_coverage_matches_perpixel() {
        let colors = [
            emColor::rgba(255, 0, 0, 255),
            emColor::rgba(0, 255, 0, 128),
            emColor::rgba(200, 100, 50, 200),
        ];
        let coverages = [0x1000i32, 0x800, 0x100];
        let bg = emColor::rgba(100, 100, 100, 200);
        let painter_alpha = 180u8;

        let mut buf = InterpolationBuffer::new(4);
        for (i, c) in colors.iter().enumerate() {
            buf.set_pixel(i, [c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha()]);
        }
        buf.set_len(colors.len());

        let mut dest_scan = make_dest(colors.len() as u32, 1, bg);
        let mode = BlendMode::SourceOver { painter_alpha };
        blend_scanline(
            &mut dest_scan,
            &buf,
            colors.len(),
            Some(&coverages),
            &mode,
        );

        // Reference: apply coverage then blend
        let mut dest_ref = make_dest(colors.len() as u32, 1, bg);
        for (i, c) in colors.iter().enumerate() {
            let cov = coverages[i];
            let adjusted_a = if cov >= 0x1000 {
                c.GetAlpha()
            } else {
                ((c.GetAlpha() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8
            };
            let adjusted = emColor::rgba(c.GetRed(), c.GetGreen(), c.GetBlue(), adjusted_a);
            ref_blend_source_over(&mut dest_ref[i * 4..(i + 1) * 4], adjusted, painter_alpha);
        }

        assert_eq!(
            dest_scan, dest_ref,
            "coverage source-over scanline vs per-pixel mismatch"
        );
    }

    #[test]
    fn blend_scanline_premul_source_over_matches() {
        let pm_pixels: [[u8; 4]; 4] = [
            [200, 0, 0, 200],   // premul red
            [0, 100, 0, 128],   // premul green
            [0, 0, 0, 0],       // transparent
            [255, 255, 255, 255], // opaque white
        ];
        let bg = emColor::rgba(100, 100, 100, 200);

        let mut buf = InterpolationBuffer::new(4);
        for (i, pm) in pm_pixels.iter().enumerate() {
            buf.set_pixel(i, *pm);
        }
        buf.set_len(pm_pixels.len());

        // Full coverage, painter_alpha=255
        let mut dest_scan = make_dest(pm_pixels.len() as u32, 1, bg);
        let mode = BlendMode::SourceOver { painter_alpha: 255 };
        blend_scanline_premul(&mut dest_scan, &buf, pm_pixels.len(), None, &mode);

        let mut dest_ref = make_dest(pm_pixels.len() as u32, 1, bg);
        for (i, pm) in pm_pixels.iter().enumerate() {
            ref_blend_premul_source_over(&mut dest_ref[i * 4..(i + 1) * 4], *pm);
        }

        assert_eq!(dest_scan, dest_ref, "premul source-over mismatch");
    }

    #[test]
    fn blend_scanline_premul_canvas_matches() {
        let pm_pixels: [[u8; 4]; 3] = [
            [200, 0, 0, 200],
            [0, 100, 0, 128],
            [255, 255, 255, 255],
        ];
        let bg = emColor::rgba(100, 100, 100, 200);
        let canvas = emColor::rgba(50, 50, 50, 255);

        let mut buf = InterpolationBuffer::new(4);
        for (i, pm) in pm_pixels.iter().enumerate() {
            buf.set_pixel(i, *pm);
        }
        buf.set_len(pm_pixels.len());

        let mut dest_scan = make_dest(pm_pixels.len() as u32, 1, bg);
        let mode = BlendMode::CanvasBlend {
            canvas,
            painter_alpha: 255,
        };
        blend_scanline_premul(&mut dest_scan, &buf, pm_pixels.len(), None, &mode);

        let mut dest_ref = make_dest(pm_pixels.len() as u32, 1, bg);
        for (i, pm) in pm_pixels.iter().enumerate() {
            ref_blend_premul_canvas(&mut dest_ref[i * 4..(i + 1) * 4], *pm, canvas);
        }

        assert_eq!(dest_scan, dest_ref, "premul canvas mismatch");
    }

    #[test]
    fn blend_scanline_premul_with_coverage_matches() {
        let pm_pixels: [[u8; 4]; 3] = [
            [200, 0, 0, 200],
            [0, 100, 0, 128],
            [50, 50, 50, 100],
        ];
        let coverages = [0x1000i32, 0x800, 0x400];
        let bg = emColor::rgba(100, 100, 100, 200);
        let painter_alpha = 200u8;

        let mut buf = InterpolationBuffer::new(4);
        for (i, pm) in pm_pixels.iter().enumerate() {
            buf.set_pixel(i, *pm);
        }
        buf.set_len(pm_pixels.len());

        let mut dest_scan = make_dest(pm_pixels.len() as u32, 1, bg);
        let mode = BlendMode::SourceOver { painter_alpha };
        blend_scanline_premul(
            &mut dest_scan,
            &buf,
            pm_pixels.len(),
            Some(&coverages),
            &mode,
        );

        // Reference: apply coverage + painter_alpha to premul, then blend
        let mut dest_ref = make_dest(pm_pixels.len() as u32, 1, bg);
        for (i, pm) in pm_pixels.iter().enumerate() {
            let cov = coverages[i];
            let o_eff = (cov * painter_alpha as i32 + 127) / 255;
            let pm_mod = if o_eff >= 0x1000 {
                *pm
            } else if o_eff > 0 {
                [
                    ((pm[0] as i32 * o_eff + 0x800) >> 12) as u8,
                    ((pm[1] as i32 * o_eff + 0x800) >> 12) as u8,
                    ((pm[2] as i32 * o_eff + 0x800) >> 12) as u8,
                    ((pm[3] as i32 * o_eff + 0x800) >> 12) as u8,
                ]
            } else {
                continue;
            };
            ref_blend_premul_source_over(&mut dest_ref[i * 4..(i + 1) * 4], pm_mod);
        }

        assert_eq!(dest_scan, dest_ref, "premul coverage mismatch");
    }

    #[test]
    fn interpolation_buffer_basics() {
        let mut buf = InterpolationBuffer::new(4);
        assert_eq!(buf.max_pixels(), 256);
        buf.set_len(2);
        buf.set_pixel(0, [255, 0, 0, 255]);
        buf.set_pixel(1, [0, 255, 0, 128]);
        assert_eq!(buf.pixel_rgba(0), [255, 0, 0, 255]);
        assert_eq!(buf.pixel_rgba(1), [0, 255, 0, 128]);

        let mut buf1 = InterpolationBuffer::new(1);
        assert_eq!(buf1.max_pixels(), 1024);
        buf1.set_len(1);
        buf1.set_pixel(0, [128, 0, 0, 0]);
        assert_eq!(buf1.pixel_rgba(0), [128, 128, 128, 255]);
    }

    #[test]
    fn row_slice_basics() {
        let img = emImage::new(4, 4, 4);
        let row = img.row_slice(0);
        assert_eq!(row.len(), 16);
        let row3 = img.row_slice(3);
        assert_eq!(row3.len(), 16);
    }

    #[test]
    fn test_source_over_alpha_update() {
        // Blend src RGBA(100,150,200,128) onto dst RGBA(50,75,100,200)
        // with full coverage and painter_alpha=255.
        // Expected alpha: div255(dst_a * (255 - src_a)) + div255(255 * src_a)
        // where div255(x) = (x * 257 + 0x8073) >> 16  (Blinn formula)
        let src = emColor::rgba(100, 150, 200, 128);
        let dst = emColor::rgba(50, 75, 100, 200);

        let mut buf = InterpolationBuffer::new(4);
        buf.set_pixel(0, [src.GetRed(), src.GetGreen(), src.GetBlue(), src.GetAlpha()]);
        buf.set_len(1);

        let mut dest = make_dest(1, 1, dst);
        let mode = BlendMode::SourceOver { painter_alpha: 255 };
        blend_scanline(&mut dest, &buf, 1, None, &mode);

        // Compute expected alpha:
        // Background: Blinn div255; Source: (c*a+127)/255 (C++ hash table)
        let src_a = 128u32;
        let dst_a = 200u32;
        let t = (255 - src_a) * 257; // inv_alpha * 257
        let expected_a =
            (((dst_a * t + 0x8073) >> 16) + ((255u32 * src_a + 127) / 255)) as u8;

        assert_eq!(
            dest[3], expected_a,
            "source-over alpha: got {} expected {}",
            dest[3], expected_a
        );
        // Sanity: output alpha should be higher than both inputs blended
        assert!(dest[3] > 128, "output alpha should exceed src alpha");
    }

    #[test]
    fn test_premul_source_over_alpha_update() {
        // Premul source: pm_r=50, pm_g=75, pm_b=100, pm_a=128
        // Dst: RGBA(50,75,100,200)
        // Expected: out_a = div255(dst_a * (255 - pm_a)) + pm_a
        // where div255(x) = (x * 257 + 0x8073) >> 16
        let pm = [50u8, 75, 100, 128];
        let dst = emColor::rgba(50, 75, 100, 200);

        let mut buf = InterpolationBuffer::new(4);
        buf.set_pixel(0, pm);
        buf.set_len(1);

        let mut dest = make_dest(1, 1, dst);
        let mode = BlendMode::SourceOver { painter_alpha: 255 };
        blend_scanline_premul(&mut dest, &buf, 1, None, &mode);

        let pm_a = 128u32;
        let dst_a = 200u32;
        let t = (255 - pm_a) * 257;
        let expected_a = (((dst_a * t + 0x8073) >> 16) + pm_a) as u8;

        assert_eq!(
            dest[3], expected_a,
            "premul source-over alpha: got {} expected {}",
            dest[3], expected_a
        );
    }

    #[test]
    fn test_canvas_blend_preserves_alpha() {
        // Canvas blend should leave destination alpha unchanged.
        let src = emColor::rgba(200, 100, 50, 200);
        let dst = emColor::rgba(100, 100, 100, 177);
        let canvas = emColor::rgba(80, 80, 80, 255);

        let mut buf = InterpolationBuffer::new(4);
        buf.set_pixel(0, [src.GetRed(), src.GetGreen(), src.GetBlue(), src.GetAlpha()]);
        buf.set_len(1);

        let mut dest = make_dest(1, 1, dst);
        let original_alpha = dest[3];
        let mode = BlendMode::CanvasBlend {
            canvas,
            painter_alpha: 200,
        };
        blend_scanline(&mut dest, &buf, 1, None, &mode);

        assert_eq!(
            dest[3], original_alpha,
            "canvas blend must not modify dest alpha: got {} expected {}",
            dest[3], original_alpha
        );

        // Also verify RGB actually changed (so the blend did something)
        let pristine = make_dest(1, 1, dst);
        assert_ne!(
            &dest[..3],
            &pristine[..3],
            "canvas blend should modify RGB channels"
        );
    }

    #[test]
    fn blend_scanline_zero_coverage_noop() {
        let bg = emColor::rgba(100, 100, 100, 200);
        let mut dest = make_dest(2, 1, bg);
        let dest_copy = dest.clone();

        let mut buf = InterpolationBuffer::new(4);
        buf.set_pixel(0, [255, 0, 0, 255]);
        buf.set_pixel(1, [0, 255, 0, 255]);
        buf.set_len(2);

        let coverages = [0i32, 0];
        let mode = BlendMode::SourceOver { painter_alpha: 255 };
        blend_scanline(&mut dest, &buf, 2, Some(&coverages), &mode);

        assert_eq!(dest, dest_copy, "zero coverage should not modify dest");
    }
}


#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_BlendMode_from_state() {
        let mut p_canvas_color = crate::emColor::emColor::rgba(kani::any(), kani::any(), kani::any(), kani::any());
        let mut p_alpha: u8 = kani::any::<u8>();
        let _r = BlendMode::from_state(p_canvas_color, p_alpha);
    }

    #[kani::proof]
    fn kani_private_InterpolationBuffer_new() {
        let mut p_ch: u8 = kani::any::<u8>();
        let _r = InterpolationBuffer::new(p_ch);
    }
}
