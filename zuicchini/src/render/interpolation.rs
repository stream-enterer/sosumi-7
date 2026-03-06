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
    let ix = x as i32;
    let iy = y as i32;
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

/// Scaling context for area sampling.
pub(crate) struct ScaleContext {
    pub src_w: f64,
    pub src_h: f64,
    pub dest_w: f64,
    pub dest_h: f64,
}

/// Area sampling (box filter) for downscaling.
pub(crate) fn sample_area(
    image: &Image,
    x: f64,
    y: f64,
    ctx: &ScaleContext,
    ext: ImageExtension,
) -> Color {
    let scale_x = ctx.src_w / ctx.dest_w;
    let scale_y = ctx.src_h / ctx.dest_h;

    let x0 = x * scale_x;
    let y0 = y * scale_y;
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
                *w = lanczos_sinc(x, 2.0);
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

/// Sample a radial gradient.
pub(crate) fn sample_radial_gradient(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    inner: Color,
    outer: Color,
    point: (f64, f64),
) -> Color {
    let dx = (point.0 - cx) / rx;
    let dy = (point.1 - cy) / ry;
    let t = (dx * dx + dy * dy).sqrt().min(1.0);
    inner.lerp(outer, t)
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
    fn radial_gradient_center() {
        let c = sample_radial_gradient(
            50.0,
            50.0,
            50.0,
            50.0,
            Color::WHITE,
            Color::BLACK,
            (50.0, 50.0),
        );
        assert_eq!(c.r(), 255);
    }

    #[test]
    fn radial_gradient_edge() {
        let c = sample_radial_gradient(
            50.0,
            50.0,
            50.0,
            50.0,
            Color::WHITE,
            Color::BLACK,
            (100.0, 50.0),
        );
        assert_eq!(c.r(), 0);
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
