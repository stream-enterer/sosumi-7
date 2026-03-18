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
        ImageExtension::EdgeOrZero => {
            unreachable!("EdgeOrZero must be resolved before interpolation")
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

/// Sample a pixel from a sub-rect of an image with extension mode handling.
/// `ix`, `iy` are relative to the section origin; `sec.ox`, `sec.oy` offset
/// into the image.
pub(crate) fn sample_section_pixel(
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
        ImageExtension::EdgeOrZero => {
            unreachable!("EdgeOrZero must be resolved before interpolation")
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

/// Pre-computed Y-axis weights for area sampling column accumulation.
struct YWeights {
    oy1: u32,
    oy1n: u32,
    ody: u32,
    row0: i32,
}

/// 24-bit fixed-point area sampling transform for downscaling.
/// Matches C++ emPainter_ScTl Init (lines 296-343) for the area-sampled path.
///
/// Key difference from `ScaleTransform24`: NO -0.5 pixel-center offset.
/// TX = tx_sub * tdx_f64 (not (tx_sub - 0.5) * tdx_f64).
pub(crate) struct AreaSampleTransform {
    /// Source-per-dest horizontal step (24fp), post-reduction.
    pub tdx: i64,
    /// Source-per-dest vertical step (24fp), post-reduction.
    pub tdy: i64,
    /// X origin offset in 24fp.
    pub tx: i64,
    /// Y origin offset in 24fp.
    pub ty: i64,
    /// Rational inverse of TDX: ((1<<40)-1)/TDX+1.
    pub odx: u32,
    /// Rational inverse of TDY: ((1<<40)-1)/TDY+1.
    pub ody: u32,
    /// Reduced source width.
    pub img_w: i32,
    /// Reduced source height.
    pub img_h: i32,
    /// Pre-reduction stride X.
    pub stride_x: u32,
    /// Pre-reduction stride Y.
    pub stride_y: u32,
    /// Centering offset X.
    pub off_x: i32,
    /// Centering offset Y.
    pub off_y: i32,
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
        ImageExtension::EdgeOrZero => {
            unreachable!("EdgeOrZero must be resolved before interpolation")
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

/// Compute the rational inverse for area sampling weight normalization.
/// Matches C++ `(((emInt64)1<<40)-1)/span+1`.
fn rational_inv(span: i64) -> u32 {
    if span <= 0x200 {
        0x7FFF_FFFF
    } else {
        (((1i64 << 40) - 1) / span + 1) as u32
    }
}

/// Read a pixel from the image at reduced-grid coordinates with section offset.
/// Coordinates are clamped to section bounds (pixel-level EXTEND_EDGE).
fn read_area_pixel<'a>(
    image: &'a Image,
    sec: &SectionBounds,
    col: i32,
    row: i32,
    xfm: &AreaSampleTransform,
) -> &'a [u8] {
    let rx = (xfm.off_x + col * xfm.stride_x as i32).clamp(0, sec.w - 1);
    let ry = (xfm.off_y + row * xfm.stride_y as i32).clamp(0, sec.h - 1);
    image.pixel((sec.ox + rx) as u32, (sec.oy + ry) as u32)
}

/// Area sampling with 24-bit fixed-point integer arithmetic.
/// Matches C++ `InterpolateImageAreaSampled` (non-tiled) exactly.
///
/// Handles CHANNELS=1, 3, and 4 with correct per-channel FINPREMUL:
/// - CHANNELS=4: RGB division `(x + 0x7F7F) / 0xFF00`, alpha shift `(x + 0x7F) >> 8`
/// - CHANNELS=1/3: shift `(x + 0x7F) >> 8` for all channels
///
/// Returns straight-alpha Color (premul->straight conversion done internally for 4-ch).
///
/// Note: production code uses `interpolate_scanline_area_sampled` which hoists Y setup
/// and adds pCy column-reuse. This per-pixel version is retained as a test reference.
#[cfg(test)]
pub(crate) fn sample_area_fp(
    image: &Image,
    dest_x: i32,
    dest_y: i32,
    xfm: &AreaSampleTransform,
    sec: &SectionBounds,
    ext: ImageExtension,
) -> Color {
    let ch = image.channel_count();

    // --- Y setup (C++ emPainter_ScTlIntImg.cpp lines 686-725) ---
    let mut ty1 = dest_y as i64 * xfm.tdy - xfm.ty;
    let mut ty2 = ty1 + xfm.tdy;
    let ty_end = (xfm.img_h as i64) << 24;
    let mut ody = xfm.ody;

    // EXACT if/else if structure from C++ — NOT sequential max/min.
    if ty1 < 0 {
        if ty2 <= 0 {
            if ext == ImageExtension::Zero {
                return Color::TRANSPARENT;
            }
            ty2 = 1 << 24; // EXTEND_EDGE: clamp to first row
        } else if ty2 > ty_end {
            ty2 = ty_end;
        }
        ty1 = 0;
        ody = rational_inv(ty2);
    } else if ty2 > ty_end {
        if ty1 >= ty_end {
            if ext == ImageExtension::Zero {
                return Color::TRANSPARENT;
            }
            ty1 = ty_end - (1 << 24); // EXTEND_EDGE: clamp to last row
        }
        ody = rational_inv(ty_end - ty1);
    }

    let oy1 = {
        let w = ((0x100_0000i64 - (ty1 & 0xFF_FFFF)) as u64 * ody as u64 + 0xFF_FFFF) >> 24;
        if w >= 0x10000 || ody == 0x7FFF_FFFF {
            0x10000u32
        } else {
            w as u32
        }
    };
    let yw = YWeights {
        oy1,
        oy1n: 0x10000u32 - oy1,
        ody,
        row0: (ty1 >> 24) as i32,
    };

    // --- X setup (C++ lines 727-776) ---
    let mut tx1 = dest_x as i64 * xfm.tdx - xfm.tx;
    let mut tx2 = tx1 + xfm.tdx;
    let tx_end = (xfm.img_w as i64) << 24;
    let mut odx = xfm.odx;

    if tx1 < 0 {
        tx1 = 0;
        if tx2 <= 0 {
            if ext == ImageExtension::Zero {
                return Color::TRANSPARENT;
            }
            tx2 = 1 << 24; // EXTEND_EDGE
        } else if tx2 > tx_end {
            tx2 = tx_end;
        }
        odx = rational_inv(tx2);
    } else if tx2 > tx_end {
        if tx1 >= tx_end {
            if ext == ImageExtension::Zero {
                return Color::TRANSPARENT;
            }
            tx1 = tx_end - (1 << 24); // EXTEND_EDGE
        }
        odx = rational_inv(tx_end - tx1);
    }

    // First column weight (C++ line 777-778).
    let ox = {
        let w = ((0x100_0000i64 - (tx1 & 0xFF_FFFF)) as u64 * odx as u64 + 0xFF_FFFF) >> 24;
        if odx == 0x7FFF_FFFF {
            0x7FFF_FFFFu32
        } else {
            w as u32
        }
    };
    let col0 = (tx1 >> 24) as i32;
    // Safety bound: max column from coordinate range.
    let col_bound = ((tx2 - 1).max(tx1) >> 24) as i32 + 1;

    // --- Column + row accumulation (C++ lines 790-825) ---
    let mut cyx_r: u64 = 0x7F_FFFF;
    let mut cyx_g: u64 = 0x7F_FFFF;
    let mut cyx_b: u64 = 0x7F_FFFF;
    let mut cyx_a: u64 = 0x7F_FFFF;

    let mut remaining = 0x10000u32;
    let mut col = col0;
    let mut col_weight = ox;

    while remaining > 0 && col <= col_bound {
        let w = if col_weight >= remaining {
            remaining
        } else {
            col_weight
        };

        // Y-accumulate for this column, then FINPREMUL.
        let (cy_r, cy_g, cy_b, cy_a) = y_accumulate(image, sec, ch, col, &yw, xfm);

        cyx_r += cy_r * w as u64;
        cyx_g += cy_g * w as u64;
        cyx_b += cy_b * w as u64;
        cyx_a += cy_a * w as u64;

        remaining -= w;
        col += 1;
        col_weight = odx;
    }

    // Output: WRITE_NO_ROUND_SHR_COLOR(cyx, 24)
    let out_r = (cyx_r >> 24) as u8;
    let out_g = (cyx_g >> 24) as u8;
    let out_b = (cyx_b >> 24) as u8;

    match ch {
        4 => {
            let out_a = (cyx_a >> 24) as u8;
            // Premul -> straight alpha conversion.
            if out_a == 0 {
                Color::TRANSPARENT
            } else if out_a == 255 {
                Color::rgba(out_r, out_g, out_b, 255)
            } else {
                let a16 = out_a as u16;
                let sr = ((out_r as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                let sg = ((out_g as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                let sb = ((out_b as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                Color::rgba(sr, sg, sb, out_a)
            }
        }
        3 => Color::rgba(out_r, out_g, out_b, 255),
        _ => Color::rgba(out_r, out_r, out_r, 255), // 1-ch gray
    }
}

/// Y-accumulate a single column for area sampling, then apply FINPREMUL.
/// Returns (cy_r, cy_g, cy_b, cy_a) after FINPREMUL_SHR_COLOR(cy, 8).
fn y_accumulate(
    image: &Image,
    sec: &SectionBounds,
    ch: u8,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
) -> (u64, u64, u64, u64) {
    let p = read_area_pixel(image, sec, col, yw.row0, xfm);

    match ch {
        4 => y_accumulate_4ch(image, sec, col, yw, xfm, p),
        3 => y_accumulate_3ch(image, sec, col, yw, xfm, p),
        _ => y_accumulate_1ch(image, sec, col, yw, xfm, p),
    }
}

/// CHANNELS=4: premultiplied alpha accumulation.
/// READ_PREMUL_MUL_COLOR: cy_a = p[3]*oy1; cy_r = p[0]*cy_a
/// FINPREMUL_SHR_COLOR(cy,8): RGB = (x + 0x7F7F) / 0xFF00; A = (x + 0x7F) >> 8
fn y_accumulate_4ch(
    image: &Image,
    sec: &SectionBounds,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
    p: &[u8],
) -> (u64, u64, u64, u64) {
    // READ_PREMUL_MUL_COLOR(cy, p, oy1) for CHANNELS=4
    let mut ca = p[3] as u64 * yw.oy1 as u64;
    let mut cr = p[0] as u64 * ca;
    let mut cg = p[1] as u64 * ca;
    let mut cb = p[2] as u64 * ca;

    let mut oys = yw.oy1n as u64;
    if oys > 0 {
        let mut r = yw.row0 + 1;
        if oys > yw.ody as u64 {
            // Interior rows: DEFINE_AND_READ_PREMUL_COLOR + ADD_READ_PREMUL_COLOR
            let pi = read_area_pixel(image, sec, col, r, xfm);
            let mut ta = pi[3] as u64;
            let mut tr = pi[0] as u64 * ta;
            let mut tg = pi[1] as u64 * ta;
            let mut tb = pi[2] as u64 * ta;
            r += 1;
            oys -= yw.ody as u64;
            while oys > yw.ody as u64 {
                let pi = read_area_pixel(image, sec, col, r, xfm);
                let a = pi[3] as u64;
                ta += a;
                tr += pi[0] as u64 * a;
                tg += pi[1] as u64 * a;
                tb += pi[2] as u64 * a;
                r += 1;
                oys -= yw.ody as u64;
            }
            // ADD_MUL_COLOR(cy, ctmp, ody)
            ca += ta * yw.ody as u64;
            cr += tr * yw.ody as u64;
            cg += tg * yw.ody as u64;
            cb += tb * yw.ody as u64;
        }
        // Last row: ADD_READ_PREMUL_MUL_COLOR(cy, p, oys)
        let pl = read_area_pixel(image, sec, col, r, xfm);
        let al = pl[3] as u64 * oys;
        ca += al;
        cr += pl[0] as u64 * al;
        cg += pl[1] as u64 * al;
        cb += pl[2] as u64 * al;
    }

    // FINPREMUL_SHR_COLOR(cy, 8) for CHANNELS=4
    // RGB: integer division (x + 0x7F7F) / 0xFF00  (NOT shift!)
    // Alpha: shift (x + 0x7F) >> 8
    let fr = (cr + 0x7F7F) / 0xFF00;
    let fg = (cg + 0x7F7F) / 0xFF00;
    let fb = (cb + 0x7F7F) / 0xFF00;
    let fa = (ca + 0x7F) >> 8;
    (fr, fg, fb, fa)
}

/// CHANNELS=3: no premultiplication.
/// READ_PREMUL_MUL_COLOR: cy_r = p[0]*oy1 (direct multiply, no alpha)
/// FINPREMUL_SHR_COLOR(cy,8): all channels use shift (x + 0x7F) >> 8
fn y_accumulate_3ch(
    image: &Image,
    sec: &SectionBounds,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
    p: &[u8],
) -> (u64, u64, u64, u64) {
    let mut cr = p[0] as u64 * yw.oy1 as u64;
    let mut cg = p[1] as u64 * yw.oy1 as u64;
    let mut cb = p[2] as u64 * yw.oy1 as u64;

    let mut oys = yw.oy1n as u64;
    if oys > 0 {
        let mut r = yw.row0 + 1;
        if oys > yw.ody as u64 {
            let pi = read_area_pixel(image, sec, col, r, xfm);
            let mut tr = pi[0] as u64;
            let mut tg = pi[1] as u64;
            let mut tb = pi[2] as u64;
            r += 1;
            oys -= yw.ody as u64;
            while oys > yw.ody as u64 {
                let pi = read_area_pixel(image, sec, col, r, xfm);
                tr += pi[0] as u64;
                tg += pi[1] as u64;
                tb += pi[2] as u64;
                r += 1;
                oys -= yw.ody as u64;
            }
            cr += tr * yw.ody as u64;
            cg += tg * yw.ody as u64;
            cb += tb * yw.ody as u64;
        }
        let pl = read_area_pixel(image, sec, col, r, xfm);
        cr += pl[0] as u64 * oys;
        cg += pl[1] as u64 * oys;
        cb += pl[2] as u64 * oys;
    }

    // FINPREMUL_SHR_COLOR(cy, 8) for CHANNELS=3: shift only
    ((cr + 0x7F) >> 8, (cg + 0x7F) >> 8, (cb + 0x7F) >> 8, 0)
}

/// CHANNELS=1: single gray channel, no premultiplication.
/// FINPREMUL_SHR_COLOR(cy,8): shift (x + 0x7F) >> 8
fn y_accumulate_1ch(
    image: &Image,
    sec: &SectionBounds,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
    p: &[u8],
) -> (u64, u64, u64, u64) {
    let mut cg = p[0] as u64 * yw.oy1 as u64;

    let mut oys = yw.oy1n as u64;
    if oys > 0 {
        let mut r = yw.row0 + 1;
        if oys > yw.ody as u64 {
            let pi = read_area_pixel(image, sec, col, r, xfm);
            let mut tg = pi[0] as u64;
            r += 1;
            oys -= yw.ody as u64;
            while oys > yw.ody as u64 {
                let pi = read_area_pixel(image, sec, col, r, xfm);
                tg += pi[0] as u64;
                r += 1;
                oys -= yw.ody as u64;
            }
            cg += tg * yw.ody as u64;
        }
        let pl = read_area_pixel(image, sec, col, r, xfm);
        cg += pl[0] as u64 * oys;
    }

    let fg = (cg + 0x7F) >> 8;
    (fg, fg, fg, 0)
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

/// Hermite basis factor table for adaptive interpolation (scale 1024).
/// Each entry: [fv1, fv2, fs1, fs2] matching C++ InterpolateFourValuesAdaptive.
static ADAPTIVE_TABLE: OnceLock<[[i32; 4]; 257]> = OnceLock::new();

fn adaptive_factors() -> &'static [[i32; 4]; 257] {
    ADAPTIVE_TABLE.get_or_init(|| {
        let mut table = [[0i32; 4]; 257];
        for (i, entry) in table.iter_mut().enumerate() {
            let o = i as f64 / 256.0;
            let o2 = o * o;
            let o3 = o2 * o;
            let fv1 = (2.0 * o3 - 3.0 * o2 + 1.0) * 1024.0;
            let fv2 = (-2.0 * o3 + 3.0 * o2) * 1024.0;
            let fs1 = (o3 - 2.0 * o2 + o) * 1024.0;
            let fs2 = (o3 - o2) * 1024.0;
            *entry = [
                fv1.round() as i32,
                fv2.round() as i32,
                fs1.round() as i32,
                fs2.round() as i32,
            ];
        }
        table
    })
}

/// Adaptive 4-value interpolation with anti-ringing slope/peak adaptation.
/// Matches C++ `InterpolateFourValuesAdaptive` optimized branch exactly.
/// Returns interpolated value at scale 1024.
fn interpolate_four_values_adaptive(v0: i32, mut v1: i32, mut v2: i32, v3: i32, o: u32) -> i64 {
    let s01 = v1 - v0;
    let s12 = v2 - v1;
    let s32 = v2 - v3;

    let mut s1: i32 = 0;
    let mut s2: i32 = 0;

    if s12 < 0 {
        if s01 < 0 {
            s1 = s01 << 1;
            if s1 < s12 {
                s1 = s12;
            }
            let mut t = s12 << 1;
            if t < s01 {
                t = s01;
            }
            if s1 > t {
                s1 = t;
            }
            let q = s1 + (s32 << 1);
            if q < 0 {
                s1 += if q > s1 { q } else { s1 };
            }
        } else if s01 > 0 {
            let s21 = -s12;
            let t = (s01 + s21 + 7) >> 4;
            let s = if s21 < s01 { s21 } else { s01 };
            v1 += if s < t { s } else { t };
        }
        if s32 > 0 {
            let s23 = -s32;
            s2 = s23 << 1;
            if s2 < s12 {
                s2 = s12;
            }
            let mut t = s12 << 1;
            if t < s23 {
                t = s23;
            }
            if s2 > t {
                s2 = t;
            }
            let q = s2 - (s01 << 1);
            if q < 0 {
                s2 += if q > s2 { q } else { s2 };
            }
        } else if s32 < 0 {
            let t = (s12 + s32 + 7) >> 4;
            let s = if s12 > s32 { s12 } else { s32 };
            v2 += if s > t { s } else { t };
        }
    } else if s12 > 0 {
        if s01 > 0 {
            s1 = s01 << 1;
            if s1 > s12 {
                s1 = s12;
            }
            let mut t = s12 << 1;
            if t > s01 {
                t = s01;
            }
            if s1 < t {
                s1 = t;
            }
            let q = s1 + (s32 << 1);
            if q > 0 {
                s1 += if q < s1 { q } else { s1 };
            }
        } else if s01 < 0 {
            let s21 = -s12;
            let t = (s21 + s01 + 7) >> 4;
            let s = if s21 > s01 { s21 } else { s01 };
            v1 += if s > t { s } else { t };
        }
        if s32 < 0 {
            let s23 = -s32;
            s2 = s23 << 1;
            if s2 > s12 {
                s2 = s12;
            }
            let mut t = s12 << 1;
            if t > s23 {
                t = s23;
            }
            if s2 < t {
                s2 = t;
            }
            let q = s2 - (s01 << 1);
            if q > 0 {
                s2 += if q < s2 { q } else { s2 };
            }
        } else if s32 > 0 {
            let t = (s32 + s12 + 7) >> 4;
            let s = if s12 < s32 { s12 } else { s32 };
            v2 += if s < t { s } else { t };
        }
    }

    let f = &adaptive_factors()[o as usize];
    v1 as i64 * f[0] as i64
        + v2 as i64 * f[1] as i64
        + s1 as i64 * f[2] as i64
        + s2 as i64 * f[3] as i64
}

/// Adaptive sampling with premultiplied alpha, 24-bit fixed-point coordinates.
/// Matches C++ InterpolateImageAdaptive for CHANNELS==4, EXTEND_ZERO.
///
/// Same separable structure as bicubic: Y-interpolate 4 columns, then X-interpolate.
/// But uses anti-ringing adaptive interpolation instead of fixed Catmull-Rom weights.
pub(crate) fn sample_adaptive_premul_fp(
    image: &Image,
    tx: i64,
    ty: i64,
    ext: ImageExtension,
) -> [u8; 4] {
    let iy = (ty >> 24) as i32;
    let ix = (tx >> 24) as i32;

    let oy = (((ty & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;
    let ox = (((tx & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;

    // Y-pass: for each of 4 columns, read premul values and adaptively interpolate.
    // C++ reads: Ca = pixel_alpha, Cr = R*Ca, Cg = G*Ca, Cb = B*Ca
    // Then calls InterpolateFourValuesAdaptive per channel.
    let mut col_rgb = [[0i64; 3]; 4];
    let mut col_a = [0i64; 4];

    for col in 0..4 {
        let mut pm = [[0i32; 4]; 4]; // pm[row] = [r*a, g*a, b*a, a]
        for (row, pm_row) in pm.iter_mut().enumerate() {
            let p = sample_pixel(image, ix + col as i32, iy + row as i32, ext);
            let a = p[3] as i32;
            *pm_row = [p[0] as i32 * a, p[1] as i32 * a, p[2] as i32 * a, a];
        }

        // Adaptive interpolation per channel (result at scale 1024)
        for ch in 0..3 {
            col_rgb[col][ch] =
                interpolate_four_values_adaptive(pm[0][ch], pm[1][ch], pm[2][ch], pm[3][ch], oy);
        }
        let a_interp = interpolate_four_values_adaptive(pm[0][3], pm[1][3], pm[2][3], pm[3][3], oy);

        // FINPREMUL: divide RGB by 255 to undo premultiplication.
        col_rgb[col][0] = (col_rgb[col][0] + 0x7f) / 0xff;
        col_rgb[col][1] = (col_rgb[col][1] + 0x7f) / 0xff;
        col_rgb[col][2] = (col_rgb[col][2] + 0x7f) / 0xff;
        col_a[col] = a_interp;
    }

    // X-pass: adaptively interpolate the 4 column results.
    let mut final_rgb = [0i64; 3];
    for ch in 0..3 {
        final_rgb[ch] = interpolate_four_values_adaptive(
            col_rgb[0][ch] as i32,
            col_rgb[1][ch] as i32,
            col_rgb[2][ch] as i32,
            col_rgb[3][ch] as i32,
            ox,
        );
    }
    let final_a = interpolate_four_values_adaptive(
        col_a[0] as i32,
        col_a[1] as i32,
        col_a[2] as i32,
        col_a[3] as i32,
        ox,
    );

    // WRITE_SHR_CLIP: >>20 with rounding, clamp(rgb, 0, alpha).
    let rnd = (1i64 << 19) - 1;
    let a = ((final_a + rnd) >> 20).clamp(0, 255);
    let mut result = [0u8; 4];
    for c in 0..3 {
        let v = ((final_rgb[c] + rnd) >> 20).clamp(0, a);
        result[c] = v as u8;
    }
    result[3] = a as u8;
    result
}

/// Section-aware adaptive sampling (for 9-slice upscaling).
/// Same as `sample_adaptive_premul_fp` but respects section bounds via
/// `sample_pixel_section` instead of `sample_pixel`.
pub(crate) fn sample_adaptive_premul_fp_section(
    image: &Image,
    tx: i64,
    ty: i64,
    sec: &SectionBounds,
    ext: ImageExtension,
) -> [u8; 4] {
    let iy = (ty >> 24) as i32;
    let ix = (tx >> 24) as i32;

    let oy = (((ty & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;
    let ox = (((tx & 0xFF_FFFF) as u32) + 0x7FFF) >> 16;

    let mut col_rgb = [[0i64; 3]; 4];
    let mut col_a = [0i64; 4];

    for col in 0..4 {
        let mut pm = [[0i32; 4]; 4];
        for (row, pm_row) in pm.iter_mut().enumerate() {
            let p = sample_pixel_section(image, ix + col as i32, iy + row as i32, sec, ext);
            let a = p[3] as i32;
            *pm_row = [p[0] as i32 * a, p[1] as i32 * a, p[2] as i32 * a, a];
        }

        for ch in 0..3 {
            col_rgb[col][ch] =
                interpolate_four_values_adaptive(pm[0][ch], pm[1][ch], pm[2][ch], pm[3][ch], oy);
        }
        let a_interp = interpolate_four_values_adaptive(pm[0][3], pm[1][3], pm[2][3], pm[3][3], oy);

        col_rgb[col][0] = (col_rgb[col][0] + 0x7f) / 0xff;
        col_rgb[col][1] = (col_rgb[col][1] + 0x7f) / 0xff;
        col_rgb[col][2] = (col_rgb[col][2] + 0x7f) / 0xff;
        col_a[col] = a_interp;
    }

    let mut final_rgb = [0i64; 3];
    for ch in 0..3 {
        final_rgb[ch] = interpolate_four_values_adaptive(
            col_rgb[0][ch] as i32,
            col_rgb[1][ch] as i32,
            col_rgb[2][ch] as i32,
            col_rgb[3][ch] as i32,
            ox,
        );
    }
    let final_a = interpolate_four_values_adaptive(
        col_a[0] as i32,
        col_a[1] as i32,
        col_a[2] as i32,
        col_a[3] as i32,
        ox,
    );

    let rnd = (1i64 << 19) - 1;
    let a = ((final_a + rnd) >> 20).clamp(0, 255);
    let mut result = [0u8; 4];
    for c in 0..3 {
        let v = ((final_rgb[c] + rnd) >> 20).clamp(0, a);
        result[c] = v as u8;
    }
    result[3] = a as u8;
    result
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

/// Scanline area-sampled interpolation: fills `buf` with `count` consecutive
/// output pixels starting at `(dest_x_start, dest_y)`.
///
/// Optimizations over per-pixel `sample_area_fp`:
/// 1. Y setup (ty1, ty2, ody, oy1, yw) computed once per row
/// 2. pCy column-reuse: when consecutive dest pixels map to the same source
///    column, the Y-accumulated result is reused (critical for downscaling)
///
/// Output format matches `sample_area_fp`: straight-alpha RGBA in `buf`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn interpolate_scanline_area_sampled(
    image: &Image,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    xfm: &AreaSampleTransform,
    sec: &SectionBounds,
    ext: ImageExtension,
    buf: &mut super::scanline_tool::InterpolationBuffer,
) {
    let ch = image.channel_count();

    // --- Y setup (hoisted out of per-pixel loop) ---
    let mut ty1 = dest_y as i64 * xfm.tdy - xfm.ty;
    let mut ty2 = ty1 + xfm.tdy;
    let ty_end = (xfm.img_h as i64) << 24;
    let mut ody = xfm.ody;

    // Track whether entire row is out-of-bounds for EXTEND_ZERO early exit.
    let mut y_oob = false;

    if ty1 < 0 {
        if ty2 <= 0 {
            if ext == ImageExtension::Zero {
                y_oob = true;
            } else {
                ty2 = 1 << 24;
            }
        } else if ty2 > ty_end {
            ty2 = ty_end;
        }
        if !y_oob {
            ty1 = 0;
            ody = rational_inv(ty2);
        }
    } else if ty2 > ty_end {
        if ty1 >= ty_end {
            if ext == ImageExtension::Zero {
                y_oob = true;
            } else {
                ty1 = ty_end - (1 << 24);
            }
        }
        if !y_oob {
            ody = rational_inv(ty_end - ty1);
        }
    }

    if y_oob {
        // All pixels in this row are transparent for EXTEND_ZERO.
        for i in 0..count {
            buf.set_pixel(i, [0, 0, 0, 0]);
        }
        buf.set_len(count);
        return;
    }

    let oy1 = {
        let w = ((0x100_0000i64 - (ty1 & 0xFF_FFFF)) as u64 * ody as u64 + 0xFF_FFFF) >> 24;
        if w >= 0x10000 || ody == 0x7FFF_FFFF {
            0x10000u32
        } else {
            w as u32
        }
    };
    let yw = YWeights {
        oy1,
        oy1n: 0x10000u32 - oy1,
        ody,
        row0: (ty1 >> 24) as i32,
    };

    // pCy column-reuse state: cache the Y-accumulated result for the last
    // source column to avoid redundant computation.
    let mut prev_cy_col: i32 = i32::MIN;
    let mut cached_cy: (u64, u64, u64, u64) = (0, 0, 0, 0);

    let tx_end = (xfm.img_w as i64) << 24;

    for pixel_idx in 0..count {
        let dest_x = dest_x_start + pixel_idx as i32;

        // --- X setup (per dest pixel, same as sample_area_fp) ---
        let mut tx1 = dest_x as i64 * xfm.tdx - xfm.tx;
        let mut tx2 = tx1 + xfm.tdx;
        let mut odx = xfm.odx;

        let mut x_oob = false;
        if tx1 < 0 {
            tx1 = 0;
            if tx2 <= 0 {
                if ext == ImageExtension::Zero {
                    x_oob = true;
                } else {
                    tx2 = 1 << 24;
                }
            } else if tx2 > tx_end {
                tx2 = tx_end;
            }
            if !x_oob {
                odx = rational_inv(tx2);
            }
        } else if tx2 > tx_end {
            if tx1 >= tx_end {
                if ext == ImageExtension::Zero {
                    x_oob = true;
                } else {
                    tx1 = tx_end - (1 << 24);
                }
            }
            if !x_oob {
                odx = rational_inv(tx_end - tx1);
            }
        }

        if x_oob {
            buf.set_pixel(pixel_idx, [0, 0, 0, 0]);
            continue;
        }

        // First column weight
        let ox = {
            let w =
                ((0x100_0000i64 - (tx1 & 0xFF_FFFF)) as u64 * odx as u64 + 0xFF_FFFF) >> 24;
            if odx == 0x7FFF_FFFF {
                0x7FFF_FFFFu32
            } else {
                w as u32
            }
        };
        let col0 = (tx1 >> 24) as i32;
        let col_bound = ((tx2 - 1).max(tx1) >> 24) as i32 + 1;

        // --- Column + row accumulation with pCy reuse ---
        let mut cyx_r: u64 = 0x7F_FFFF;
        let mut cyx_g: u64 = 0x7F_FFFF;
        let mut cyx_b: u64 = 0x7F_FFFF;
        let mut cyx_a: u64 = 0x7F_FFFF;

        let mut remaining = 0x10000u32;
        let mut col = col0;
        let mut col_weight = ox;

        while remaining > 0 && col <= col_bound {
            let w = if col_weight >= remaining {
                remaining
            } else {
                col_weight
            };

            // pCy column-reuse: check if this column was already Y-accumulated.
            let (cy_r, cy_g, cy_b, cy_a) = if col == prev_cy_col {
                cached_cy
            } else {
                let cy = y_accumulate(image, sec, ch, col, &yw, xfm);
                prev_cy_col = col;
                cached_cy = cy;
                cy
            };

            cyx_r += cy_r * w as u64;
            cyx_g += cy_g * w as u64;
            cyx_b += cy_b * w as u64;
            cyx_a += cy_a * w as u64;

            remaining -= w;
            col += 1;
            col_weight = odx;
        }

        // Output: WRITE_NO_ROUND_SHR_COLOR(cyx, 24)
        let out_r = (cyx_r >> 24) as u8;
        let out_g = (cyx_g >> 24) as u8;
        let out_b = (cyx_b >> 24) as u8;

        let rgba = match ch {
            4 => {
                let out_a = (cyx_a >> 24) as u8;
                if out_a == 0 {
                    [0, 0, 0, 0]
                } else if out_a == 255 {
                    [out_r, out_g, out_b, 255]
                } else {
                    let a16 = out_a as u16;
                    let sr = ((out_r as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                    let sg = ((out_g as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                    let sb = ((out_b as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                    [sr, sg, sb, out_a]
                }
            }
            3 => [out_r, out_g, out_b, 255],
            _ => [out_r, out_r, out_r, 255],
        };
        buf.set_pixel(pixel_idx, rgba);
    }
    buf.set_len(count);
}

/// Scanline adaptive premul interpolation: fills `buf` with `count` consecutive
/// output pixels of premultiplied RGBA.
#[allow(clippy::too_many_arguments)]
pub(crate) fn interpolate_scanline_adaptive_premul(
    image: &Image,
    px: i32,
    py: i32,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    sxfm: &ScaleTransform24,
    ext: ImageExtension,
    buf: &mut super::scanline_tool::InterpolationBuffer,
) {
    for i in 0..count {
        let col = dest_x_start + i as i32;
        let tx = (col - px) as i64 * sxfm.tdx + sxfm.base_x - 0x180_0000;
        let ty = (dest_y - py) as i64 * sxfm.tdy + sxfm.base_y - 0x180_0000;
        let pm = sample_adaptive_premul_fp(image, tx, ty, ext);
        buf.set_pixel(i, pm);
    }
    buf.set_len(count);
}

/// Scanline adaptive premul interpolation with section bounds (for 9-slice).
#[allow(clippy::too_many_arguments)]
pub(crate) fn interpolate_scanline_adaptive_premul_section(
    image: &Image,
    px: i32,
    py: i32,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    sxfm: &ScaleTransform24,
    sec: &SectionBounds,
    ext: ImageExtension,
    buf: &mut super::scanline_tool::InterpolationBuffer,
) {
    for i in 0..count {
        let col = dest_x_start + i as i32;
        let tx = (col - px) as i64 * sxfm.tdx + sxfm.base_x - 0x180_0000;
        let ty = (dest_y - py) as i64 * sxfm.tdy + sxfm.base_y - 0x180_0000;
        let pm = sample_adaptive_premul_fp_section(image, tx, ty, sec, ext);
        buf.set_pixel(i, pm);
    }
    buf.set_len(count);
}

/// Scanline nearest-neighbor interpolation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn interpolate_scanline_nearest(
    image: &Image,
    px: i32,
    py: i32,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    sxfm: &ScaleTransform24,
    ext: ImageExtension,
    buf: &mut super::scanline_tool::InterpolationBuffer,
) {
    // ty is constant for the whole row
    let ty = (dest_y - py) as i64 * sxfm.tdy + sxfm.base_y;
    let src_y = (ty >> 24) as f64;
    for i in 0..count {
        let col = dest_x_start + i as i32;
        let tx = (col - px) as i64 * sxfm.tdx + sxfm.base_x;
        let c = sample_nearest(image, (tx >> 24) as f64, src_y, ext);
        buf.set_pixel(i, [c.r(), c.g(), c.b(), c.a()]);
    }
    buf.set_len(count);
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

    // ── 24fp area sampling unit tests ──────────────────────────────

    /// Helper: construct an AreaSampleTransform for testing.
    /// Assumes identity painter state (scale=1, offset=0, dest origin at 0).
    fn make_area_xfm(src_w: u32, src_h: u32, dest_w: f64, dest_h: f64) -> AreaSampleTransform {
        let tdx_f64 = ((src_w as i64) << 24) as f64 / dest_w;
        let tdy_f64 = ((src_h as i64) << 24) as f64 / dest_h;
        let tdx = tdx_f64 as i64;
        let tdy = tdy_f64 as i64;
        AreaSampleTransform {
            tdx,
            tdy,
            tx: 0,
            ty: 0,
            odx: rational_inv(tdx),
            ody: rational_inv(tdy),
            img_w: src_w as i32,
            img_h: src_h as i32,
            stride_x: 1,
            stride_y: 1,
            off_x: 0,
            off_y: 0,
        }
    }

    fn full_sec(w: u32, h: u32) -> SectionBounds {
        SectionBounds {
            ox: 0,
            oy: 0,
            w: w as i32,
            h: h as i32,
        }
    }

    #[test]
    fn area_sample_fp_solid_4ch() {
        // 4x4 RGBA, all pixels solid red — uniform input must give uniform output.
        let mut img = Image::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let p = img.pixel_mut(x, y);
                p[0] = 255;
                p[3] = 255;
            }
        }
        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        assert_eq!(c, Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn area_sample_fp_gradient_4ch() {
        // 4x2 RGBA: left half (128,0,0,255), right half (0,128,0,255).
        // 4:1 X downscale, 2:1 Y → 1 dest pixel covers entire image.
        let mut img = Image::new(4, 2, 4);
        for y in 0..2u32 {
            for x in 0..4u32 {
                let p = img.pixel_mut(x, y);
                if x < 2 {
                    p[0] = 128;
                } else {
                    p[1] = 128;
                }
                p[3] = 255;
            }
        }
        let xfm = make_area_xfm(4, 2, 1.0, 1.0);
        let sec = full_sec(4, 2);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        // Equal-weight average: (64, 64, 0, 255) ± 1 for integer rounding.
        assert!((c.r() as i32 - 64).abs() <= 1, "r={} expected ~64", c.r());
        assert!((c.g() as i32 - 64).abs() <= 1, "g={} expected ~64", c.g());
        assert_eq!(c.b(), 0);
        assert_eq!(c.a(), 255);
    }

    #[test]
    fn area_sample_fp_alpha_4ch() {
        // 2x2 RGBA: (0,0)=(255,0,0,128), rest=(0,0,0,0).
        // Covers premul accumulation with mixed alpha.
        let mut img = Image::new(2, 2, 4);
        let p = img.pixel_mut(0, 0);
        p[0] = 255;
        p[3] = 128;
        // 2:1 downscale → 1 dest pixel covers all 4 source pixels.
        let xfm = make_area_xfm(2, 2, 1.0, 1.0);
        let sec = full_sec(2, 2);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        // 1 of 4 pixels has alpha=128 → low alpha, non-zero red.
        assert!(c.a() > 0, "alpha should be non-zero, got {}", c.a());
        assert!(c.r() > 0, "red should be non-zero, got {}", c.r());
        assert_eq!(c.g(), 0);
        assert_eq!(c.b(), 0);
    }

    #[test]
    fn area_sample_fp_extend_edge() {
        // 2x2 solid image, dest pixel maps fully outside bounds.
        // EXTEND_EDGE must clamp to edge pixel.
        let mut img = Image::new(2, 2, 4);
        for y in 0..2u32 {
            for x in 0..2u32 {
                let p = img.pixel_mut(x, y);
                p[0] = 200;
                p[1] = 100;
                p[2] = 50;
                p[3] = 255;
            }
        }
        let tdx = 1i64 << 24;
        let tdy = 1i64 << 24;
        let xfm = AreaSampleTransform {
            tdx,
            tdy,
            tx: 5 << 24, // dest pixel 0 maps to source -5, fully outside
            ty: 5 << 24,
            odx: rational_inv(tdx),
            ody: rational_inv(tdy),
            img_w: 2,
            img_h: 2,
            stride_x: 1,
            stride_y: 1,
            off_x: 0,
            off_y: 0,
        };
        let sec = full_sec(2, 2);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        assert_eq!(c, Color::rgba(200, 100, 50, 255));
    }

    #[test]
    fn area_sample_fp_extend_zero() {
        // Same setup as extend_edge but with EXTEND_ZERO → transparent.
        let mut img = Image::new(2, 2, 4);
        for y in 0..2u32 {
            for x in 0..2u32 {
                let p = img.pixel_mut(x, y);
                p[0] = 200;
                p[1] = 100;
                p[2] = 50;
                p[3] = 255;
            }
        }
        let tdx = 1i64 << 24;
        let tdy = 1i64 << 24;
        let xfm = AreaSampleTransform {
            tdx,
            tdy,
            tx: 5 << 24,
            ty: 5 << 24,
            odx: rational_inv(tdx),
            ody: rational_inv(tdy),
            img_w: 2,
            img_h: 2,
            stride_x: 1,
            stride_y: 1,
            off_x: 0,
            off_y: 0,
        };
        let sec = full_sec(2, 2);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Zero);
        assert_eq!(c, Color::TRANSPARENT);
    }

    #[test]
    fn area_sample_fp_1ch() {
        // 4x4 grayscale with gradient values.
        let mut img = Image::new(4, 4, 1);
        for y in 0..4u32 {
            for x in 0..4u32 {
                img.pixel_mut(x, y)[0] = (x * 60 + y * 20) as u8;
            }
        }
        // 2:1 downscale: dest pixel (0,0) covers source (0,0)=0, (1,0)=60, (0,1)=20, (1,1)=80.
        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        // Average = (0+60+20+80)/4 = 40 ± 1.
        assert!(
            (c.r() as i32 - 40).abs() <= 1,
            "gray={} expected ~40",
            c.r()
        );
        // 1-ch: all channels equal, alpha=255.
        assert_eq!(c.r(), c.g());
        assert_eq!(c.r(), c.b());
        assert_eq!(c.a(), 255);
    }

    #[test]
    fn area_sample_fp_3ch() {
        // 4x4 RGB, uniform (100,150,200) → exact roundtrip.
        let mut img = Image::new(4, 4, 3);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let p = img.pixel_mut(x, y);
                p[0] = 100;
                p[1] = 150;
                p[2] = 200;
            }
        }
        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        assert_eq!(c, Color::rgba(100, 150, 200, 255));
    }
}
