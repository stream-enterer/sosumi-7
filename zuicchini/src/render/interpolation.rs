use std::sync::OnceLock;

use super::texture::ImageExtension;
use crate::foundation::{Color, Image};

/// Interpolation quality for image sampling.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum InterpolationQuality {
    Nearest,
    Bilinear,
    AreaSampled,
    Bicubic,
    Lanczos,
    Adaptive,
}

/// Sample a pixel from an image with extension mode handling.
fn sample_pixel(image: &Image, ix: i32, iy: i32, ext: ImageExtension) -> [u8; 4] {
    let w = image.width() as i32;
    let h = image.height() as i32;

    let (sx, sy) = match ext {
        ImageExtension::Clamp => (ix.clamp(0, w - 1), iy.clamp(0, h - 1)),
        ImageExtension::Repeat => {
            let sx = ((ix % w) + w) % w;
            let sy = ((iy % h) + h) % h;
            (sx, sy)
        }
        ImageExtension::Zero => {
            if ix < 0 || ix >= w || iy < 0 || iy >= h {
                return [0, 0, 0, 0];
            }
            (ix, iy)
        }
    };

    let p = image.pixel(sx as u32, sy as u32);
    let ch = image.channel_count();
    match ch {
        1 => [p[0], p[0], p[0], 255],
        3 => [p[0], p[1], p[2], 255],
        4 => [p[0], p[1], p[2], p[3]],
        _ => [0, 0, 0, 0],
    }
}

/// Nearest-neighbor sampling.
pub(crate) fn sample_nearest(image: &Image, x: f64, y: f64, ext: ImageExtension) -> Color {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let p = sample_pixel(image, ix, iy, ext);
    Color::rgba(p[0], p[1], p[2], p[3])
}

/// Bilinear interpolation (2x2 kernel).
pub(crate) fn sample_bilinear(image: &Image, x: f64, y: f64, ext: ImageExtension) -> Color {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i32;
    let iy = fy as i32;
    let tx = ((x - fx) * 256.0) as u32;
    let ty = ((y - fy) * 256.0) as u32;
    let itx = 256 - tx;
    let ity = 256 - ty;

    let p00 = sample_pixel(image, ix, iy, ext);
    let p10 = sample_pixel(image, ix + 1, iy, ext);
    let p01 = sample_pixel(image, ix, iy + 1, ext);
    let p11 = sample_pixel(image, ix + 1, iy + 1, ext);

    let mut result = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] as u32 * itx + p10[c] as u32 * tx;
        let bot = p01[c] as u32 * itx + p11[c] as u32 * tx;
        result[c] = ((top * ity + bot * ty + 0x8000) >> 16) as u8;
    }
    Color::rgba(result[0], result[1], result[2], result[3])
}

/// 24-bit fixed-point scaling transform matching C++ emPainter_ScTl.
///
/// Setup uses f64 for TDX/TDY/TX/TY derivation (matching C++ which computes
/// these as `double` then casts to `emInt64`). The per-pixel inner loop
/// is pure i64 integer arithmetic.
pub(crate) struct ScaleTransform24 {
    /// Source-per-dest horizontal step (24fp).
    pub tdx: i64,
    /// Source-per-dest vertical step (24fp).
    pub tdy: i64,
    /// Precomputed X base: `px * tdx - tx_origin`.
    pub base_x: i64,
    /// Precomputed Y base: `py * tdy - ty_origin`.
    pub base_y: i64,
}

/// Source section bounds for 9-slice sub-region sampling.
pub(crate) struct SectionBounds {
    /// Pixel X offset into the full image for the section start.
    pub ox: i32,
    /// Pixel Y offset into the full image for the section start.
    pub oy: i32,
    /// Section width in pixels.
    pub w: i32,
    /// Section height in pixels.
    pub h: i32,
}

/// Sample a pixel with extension mode scoped to a section sub-region.
/// `ix`, `iy`: coordinates relative to the section origin (can be negative / out of bounds).
fn sample_pixel_section(
    image: &Image,
    ix: i32,
    iy: i32,
    sec: &SectionBounds,
    ext: ImageExtension,
) -> [u8; 4] {
    let (sx, sy) = match ext {
        ImageExtension::Clamp => (ix.clamp(0, sec.w - 1), iy.clamp(0, sec.h - 1)),
        ImageExtension::Repeat => {
            let sx = ((ix % sec.w) + sec.w) % sec.w;
            let sy = ((iy % sec.h) + sec.h) % sec.h;
            (sx, sy)
        }
        ImageExtension::Zero => {
            if ix < 0 || ix >= sec.w || iy < 0 || iy >= sec.h {
                return [0, 0, 0, 0];
            }
            (ix, iy)
        }
    };
    let p = image.pixel((sec.ox + sx) as u32, (sec.oy + sy) as u32);
    let ch = image.channel_count();
    match ch {
        1 => [p[0], p[0], p[0], 255],
        3 => [p[0], p[1], p[2], 255],
        4 => [p[0], p[1], p[2], p[3]],
        _ => [0, 0, 0, 0],
    }
}

/// Bilinear interpolation with premultiplied alpha, 24-bit fixed-point coordinates.
/// Matches C++ emPainter_ScTlIntImg bilinear inner loop exactly.
///
/// `tx`, `ty`: source position in 24fp, with method offset (-0x80_0000) already applied.
/// `sec`: section bounds for sub-region clamping.
pub(crate) fn sample_bilinear_premul_fp(
    image: &Image,
    tx: i64,
    ty: i64,
    sec: &SectionBounds,
    ext: ImageExtension,
) -> Color {
    // Integer source position (arithmetic right shift preserves sign).
    let iy = (ty >> 24) as i32;
    let ix = (tx >> 24) as i32;

    // Fractional part → 0–256 weight.
    // Mask extracts low 24 bits (always non-negative after mask).
    // +0x7FFF is C++'s half-LSB rounding bias before the >>16 reduction.
    let oy1 = (((ty & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;
    let oy0 = 256 - oy1;
    let ox1 = (((tx & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;
    let ox0 = 256 - ox1;

    let p00 = sample_pixel_section(image, ix, iy, sec, ext);
    let p10 = sample_pixel_section(image, ix + 1, iy, sec, ext);
    let p01 = sample_pixel_section(image, ix, iy + 1, sec, ext);
    let p11 = sample_pixel_section(image, ix + 1, iy + 1, sec, ext);

    // Fast path: all opaque — premultiplication is identity.
    if p00[3] == 255 && p10[3] == 255 && p01[3] == 255 && p11[3] == 255 {
        let (ox0, ox1, oy0, oy1) = (ox0 as u64, ox1 as u64, oy0 as u64, oy1 as u64);
        let mut result = [0u8; 4];
        for c in 0..4 {
            let top = p00[c] as u64 * ox0 + p10[c] as u64 * ox1;
            let bot = p01[c] as u64 * ox0 + p11[c] as u64 * ox1;
            result[c] = ((top * oy0 + bot * oy1 + 0x8000) >> 16) as u8;
        }
        return Color::rgba(result[0], result[1], result[2], result[3]);
    }

    // Premultiplied alpha path: accumulate r*a*w and a*w.
    let (ox0, ox1, oy0, oy1) = (ox0 as u64, ox1 as u64, oy0 as u64, oy1 as u64);
    let pixels = [p00, p10, p01, p11];
    let weights = [ox0 * oy0, ox1 * oy0, ox0 * oy1, ox1 * oy1];

    let mut pm_r = 0u64;
    let mut pm_g = 0u64;
    let mut pm_b = 0u64;
    let mut pm_a = 0u64;

    for (p, &w) in pixels.iter().zip(weights.iter()) {
        let a = p[3] as u64;
        let aw = a * w;
        pm_r += p[0] as u64 * aw;
        pm_g += p[1] as u64 * aw;
        pm_b += p[2] as u64 * aw;
        pm_a += aw;
    }

    // C++ FINPREMUL_SHR_COLOR(c, 16)
    let final_a = ((pm_a + 0x7FFF) >> 16).min(255);
    if final_a == 0 {
        return Color::TRANSPARENT;
    }

    let div = 0xFF_u64 << 16;
    let round = (div >> 1) - 1;
    let final_r = ((pm_r + round) / div).min(final_a);
    let final_g = ((pm_g + round) / div).min(final_a);
    let final_b = ((pm_b + round) / div).min(final_a);

    if final_a == 255 {
        return Color::rgba(final_r as u8, final_g as u8, final_b as u8, 255);
    }
    let sr = (final_r * 255 / final_a).min(255) as u8;
    let sg = (final_g * 255 / final_a).min(255) as u8;
    let sb = (final_b * 255 / final_a).min(255) as u8;

    Color::rgba(sr, sg, sb, final_a as u8)
}

/// Bicubic sampling with premultiplied alpha, 24-bit fixed-point coordinates.
/// Matches C++ emPainter_ScTlIntImg bicubic inner loop exactly.
///
/// `tx`, `ty`: source position in 24fp, with method offset (-0x180_0000) already applied.
/// The -1.5 offset means `ty >> 24` is already shifted so rows [iy..iy+3] are centered.
pub(crate) fn sample_bicubic_premul_fp(
    image: &Image,
    tx: i64,
    ty: i64,
    ext: ImageExtension,
) -> Color {
    let iy = (ty >> 24) as i32;
    let ix = (tx >> 24) as i32;

    let oy = (((ty & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;
    let ox = (((tx & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;
    let wy = bicubic_factors_hi()[oy as usize];
    let wx = bicubic_factors_hi()[ox as usize];

    // Full 2D premul interpolation: 4x4 kernel.
    // C++ kernel rows are [iy, iy+1, iy+2, iy+3] (offset already in iy).
    let mut pm_rgb = [0i64; 3];
    let mut pm_a = 0i64;
    for row in 0..4i32 {
        let yw = wy[row as usize] as i64;
        for col in 0..4i32 {
            let p = sample_pixel(image, ix + col, iy + row, ext);
            let a = p[3] as i64;
            let w = wx[col as usize] as i64 * yw;
            let aw = a * w;
            pm_a += aw;
            pm_rgb[0] += p[0] as i64 * aw;
            pm_rgb[1] += p[1] as i64 * aw;
            pm_rgb[2] += p[2] as i64 * aw;
        }
    }

    // C++ WRITE_SHR_CLIP: shift right by 20 (1024^2 = 2^20).
    let final_a = (pm_a >> 20).clamp(0, 255);
    let mut result = [0u8; 4];
    for c in 0..3 {
        let v = ((pm_rgb[c] / 255) >> 20).clamp(0, final_a);
        result[c] = v as u8;
    }
    result[3] = final_a as u8;

    // Convert from premultiplied to straight alpha.
    if final_a > 0 && final_a < 255 {
        for item in result.iter_mut().take(3) {
            *item = (*item as u16 * 255 / final_a as u16).min(255) as u8;
        }
    }

    Color::rgba(result[0], result[1], result[2], result[3])
}

/// Scaling context for area sampling.
pub(crate) struct ScaleContext {
    pub src_w: f64,
    pub src_h: f64,
    pub dest_w: f64,
    pub dest_h: f64,
}

/// Area sampling (box filter) for downscaling.
/// `x` and `y` are in source image coordinates.
pub(crate) fn sample_area(
    image: &Image,
    x: f64,
    y: f64,
    ctx: &ScaleContext,
    ext: ImageExtension,
) -> Color {
    let scale_x = ctx.src_w / ctx.dest_w;
    let scale_y = ctx.src_h / ctx.dest_h;

    let x0 = x;
    let y0 = y;
    let x1 = x0 + scale_x;
    let y1 = y0 + scale_y;

    let ix0 = x0.floor() as i32;
    let iy0 = y0.floor() as i32;
    let ix1 = x1.ceil() as i32;
    let iy1 = y1.ceil() as i32;

    let mut accum = [0u32; 4];
    let mut weight_total = 0u32;

    for sy in iy0..iy1 {
        let wy = if sy == iy0 {
            ((sy + 1) as f64 - y0).min(1.0)
        } else if sy == iy1 - 1 {
            (y1 - sy as f64).min(1.0)
        } else {
            1.0
        };

        for sx in ix0..ix1 {
            let wx = if sx == ix0 {
                ((sx + 1) as f64 - x0).min(1.0)
            } else if sx == ix1 - 1 {
                (x1 - sx as f64).min(1.0)
            } else {
                1.0
            };

            let w = (wx * wy * 256.0) as u32;
            let p = sample_pixel(image, sx, sy, ext);
            for c in 0..4 {
                accum[c] += p[c] as u32 * w;
            }
            weight_total += w;
        }
    }

    if weight_total == 0 {
        return Color::TRANSPARENT;
    }

    let mut result = [0u8; 4];
    for c in 0..4 {
        result[c] = ((accum[c] + weight_total / 2) / weight_total) as u8;
    }
    Color::rgba(result[0], result[1], result[2], result[3])
}

/// Bicubic Catmull-Rom factor table (257 entries for fractional position 0..256).
/// Low-precision (scale 256) for non-premul paths.
static BICUBIC_TABLE: OnceLock<[[i16; 4]; 257]> = OnceLock::new();

fn bicubic_factors() -> &'static [[i16; 4]; 257] {
    BICUBIC_TABLE.get_or_init(|| {
        let mut table = [[0i16; 4]; 257];
        for (i, entry) in table.iter_mut().enumerate() {
            let t = i as f64 / 256.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let w0 = -0.5 * t3 + t2 - 0.5 * t;
            let w1 = 1.5 * t3 - 2.5 * t2 + 1.0;
            let w2 = -1.5 * t3 + 2.0 * t2 + 0.5 * t;
            let w3 = 0.5 * t3 - 0.5 * t2;
            *entry = [
                (w0 * 256.0).round() as i16,
                (w1 * 256.0).round() as i16,
                (w2 * 256.0).round() as i16,
                (w3 * 256.0).round() as i16,
            ];
        }
        table
    })
}

/// High-precision bicubic factor table (scale 1024) matching C++ BicubicFactorsTable.
static BICUBIC_TABLE_HI: OnceLock<[[i32; 4]; 257]> = OnceLock::new();

fn bicubic_factors_hi() -> &'static [[i32; 4]; 257] {
    BICUBIC_TABLE_HI.get_or_init(|| {
        let mut table = [[0i32; 4]; 257];
        for (i, entry) in table.iter_mut().enumerate() {
            let t = i as f64 / 256.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let w0 = -0.5 * t3 + t2 - 0.5 * t;
            let w1 = 1.5 * t3 - 2.5 * t2 + 1.0;
            let w2 = -1.5 * t3 + 2.0 * t2 + 0.5 * t;
            let w3 = 0.5 * t3 - 0.5 * t2;
            *entry = [
                (w0 * 1024.0).round() as i32,
                (w1 * 1024.0).round() as i32,
                (w2 * 1024.0).round() as i32,
                (w3 * 1024.0).round() as i32,
            ];
        }
        table
    })
}

/// Bicubic (Catmull-Rom) sampling with 4x4 kernel.
pub(crate) fn sample_bicubic(image: &Image, x: f64, y: f64, ext: ImageExtension) -> Color {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i32;
    let iy = fy as i32;
    let tx = ((x - fx) * 256.0) as usize;
    let ty = ((y - fy) * 256.0) as usize;

    let wx = bicubic_factors()[tx.min(256)];
    let wy = bicubic_factors()[ty.min(256)];

    let mut accum = [0i32; 4];
    for row in 0..4i32 {
        let ry = iy + row - 1;
        let mut row_accum = [0i32; 4];
        for col in 0..4i32 {
            let p = sample_pixel(image, ix + col - 1, ry, ext);
            for c in 0..4 {
                row_accum[c] += p[c] as i32 * wx[col as usize] as i32;
            }
        }
        for c in 0..4 {
            accum[c] += (row_accum[c] >> 8) * wy[row as usize] as i32;
        }
    }

    let mut result = [0u8; 4];
    for c in 0..4 {
        result[c] = (accum[c] >> 8).clamp(0, 255) as u8;
    }
    Color::rgba(result[0], result[1], result[2], result[3])
}

/// Lanczos factor table (257 entries).
static LANCZOS_TABLE: OnceLock<[[i16; 4]; 257]> = OnceLock::new();

fn lanczos_factors() -> &'static [[i16; 4]; 257] {
    LANCZOS_TABLE.get_or_init(|| {
        let mut table = [[0i16; 4]; 257];
        for (i, entry) in table.iter_mut().enumerate() {
            let t = i as f64 / 256.0;
            let mut weights = [0.0f64; 4];
            for (j, w) in weights.iter_mut().enumerate() {
                let x = t + 1.0 - j as f64;
                *w = lanczos_sinc(x, 2.5);
            }
            let sum: f64 = weights.iter().sum();
            if sum.abs() > 1e-10 {
                for w in &mut weights {
                    *w /= sum;
                }
            }
            for (j, w) in weights.iter().enumerate() {
                entry[j] = (*w * 256.0).round() as i16;
            }
        }
        table
    })
}

fn lanczos_sinc(x: f64, a: f64) -> f64 {
    if x.abs() < 1e-10 {
        return 1.0;
    }
    if x.abs() >= a {
        return 0.0;
    }
    let px = std::f64::consts::PI * x;
    (px.sin() / px) * ((px / a).sin() / (px / a))
}

/// Lanczos sampling with 4-tap windowed sinc.
pub(crate) fn sample_lanczos(image: &Image, x: f64, y: f64, ext: ImageExtension) -> Color {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i32;
    let iy = fy as i32;
    let tx = ((x - fx) * 256.0) as usize;
    let ty = ((y - fy) * 256.0) as usize;

    let wx = lanczos_factors()[tx.min(256)];
    let wy = lanczos_factors()[ty.min(256)];

    let mut accum = [0i32; 4];
    for row in 0..4i32 {
        let ry = iy + row - 1;
        let mut row_accum = [0i32; 4];
        for col in 0..4i32 {
            let p = sample_pixel(image, ix + col - 1, ry, ext);
            for c in 0..4 {
                row_accum[c] += p[c] as i32 * wx[col as usize] as i32;
            }
        }
        for c in 0..4 {
            accum[c] += (row_accum[c] >> 8) * wy[row as usize] as i32;
        }
    }

    let mut result = [0u8; 4];
    for c in 0..4 {
        result[c] = (accum[c] >> 8).clamp(0, 255) as u8;
    }
    Color::rgba(result[0], result[1], result[2], result[3])
}

/// Adaptive edge-sensitive sampling: bilinear near edges, bicubic in smooth areas.
pub(crate) fn sample_adaptive(image: &Image, x: f64, y: f64, ext: ImageExtension) -> Color {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i32;
    let iy = fy as i32;

    let p00 = sample_pixel(image, ix, iy, ext);
    let p10 = sample_pixel(image, ix + 1, iy, ext);
    let p01 = sample_pixel(image, ix, iy + 1, ext);
    let p11 = sample_pixel(image, ix + 1, iy + 1, ext);

    let edge = channel_diff(&p00, &p10)
        .max(channel_diff(&p00, &p01))
        .max(channel_diff(&p10, &p11))
        .max(channel_diff(&p01, &p11));

    if edge > 64 {
        sample_bilinear(image, x, y, ext)
    } else {
        sample_bicubic(image, x, y, ext)
    }
}

fn channel_diff(a: &[u8; 4], b: &[u8; 4]) -> u8 {
    let mut max_d = 0u16;
    for c in 0..3 {
        let d = (a[c] as i16 - b[c] as i16).unsigned_abs();
        max_d = max_d.max(d);
    }
    max_d.min(255) as u8
}

/// Sample using the specified quality.
pub(crate) fn sample(
    image: &Image,
    x: f64,
    y: f64,
    quality: InterpolationQuality,
    ext: ImageExtension,
    ctx: &ScaleContext,
) -> Color {
    match quality {
        InterpolationQuality::Nearest => sample_nearest(image, x, y, ext),
        InterpolationQuality::Bilinear => sample_bilinear(image, x, y, ext),
        InterpolationQuality::AreaSampled => sample_area(image, x, y, ctx, ext),
        InterpolationQuality::Bicubic => sample_bicubic(image, x, y, ext),
        InterpolationQuality::Lanczos => sample_lanczos(image, x, y, ext),
        InterpolationQuality::Adaptive => sample_adaptive(image, x, y, ext),
    }
}

/// Sample a linear gradient.
pub(crate) fn sample_linear_gradient(
    start: (f64, f64),
    end: (f64, f64),
    c0: Color,
    c1: Color,
    point: (f64, f64),
) -> Color {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return c0;
    }
    let t = ((point.0 - start.0) * dx + (point.1 - start.1) * dy) / len_sq;
    c0.lerp(c1, t.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_image() -> Image {
        let mut img = Image::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let v = (x * 64 + y * 16) as u8;
                let p = img.pixel_mut(x, y);
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 255;
            }
        }
        img
    }

    #[test]
    fn nearest_center() {
        let img = make_test_image();
        let c = sample_nearest(&img, 1.0, 1.0, ImageExtension::Clamp);
        assert_eq!(c.r(), 80);
    }

    #[test]
    fn bilinear_center() {
        let img = make_test_image();
        let c = sample_bilinear(&img, 0.5, 0.5, ImageExtension::Clamp);
        assert!((c.r() as i32 - 40).abs() <= 1);
    }

    #[test]
    fn bilinear_at_pixel() {
        let img = make_test_image();
        let c = sample_bilinear(&img, 0.0, 0.0, ImageExtension::Clamp);
        assert_eq!(c.r(), 0);
    }

    #[test]
    fn bicubic_at_pixel() {
        let img = make_test_image();
        let c = sample_bicubic(&img, 1.0, 1.0, ImageExtension::Clamp);
        assert!((c.r() as i32 - 80).abs() <= 5);
    }

    #[test]
    fn lanczos_at_pixel() {
        let img = make_test_image();
        let c = sample_lanczos(&img, 1.0, 1.0, ImageExtension::Clamp);
        assert!((c.r() as i32 - 80).abs() <= 5);
    }

    #[test]
    fn linear_gradient_endpoints() {
        let c0 = sample_linear_gradient(
            (0.0, 0.0),
            (100.0, 0.0),
            Color::WHITE,
            Color::BLACK,
            (0.0, 0.0),
        );
        assert_eq!(c0.r(), 255);
        let c1 = sample_linear_gradient(
            (0.0, 0.0),
            (100.0, 0.0),
            Color::WHITE,
            Color::BLACK,
            (100.0, 0.0),
        );
        assert_eq!(c1.r(), 0);
    }

    #[test]
    fn extension_zero() {
        let img = make_test_image();
        let c = sample_nearest(&img, -1.0, -1.0, ImageExtension::Zero);
        assert_eq!(c.a(), 0);
    }

    #[test]
    fn extension_repeat() {
        let img = make_test_image();
        let c0 = sample_nearest(&img, 0.0, 0.0, ImageExtension::Repeat);
        let c4 = sample_nearest(&img, 4.0, 4.0, ImageExtension::Repeat);
        assert_eq!(c0, c4);
    }

    #[test]
    fn area_sample_identity() {
        let img = make_test_image();
        let ctx = ScaleContext {
            src_w: 4.0,
            src_h: 4.0,
            dest_w: 4.0,
            dest_h: 4.0,
        };
        let c = sample_area(&img, 1.0, 1.0, &ctx, ImageExtension::Clamp);
        assert!((c.r() as i32 - 80).abs() <= 2);
    }
}
