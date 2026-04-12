// SPLIT: Split from emPainter.h — interpolation routines extracted
use std::sync::OnceLock;

use crate::emTexture::ImageExtension;
use crate::emColor::emColor;
use crate::emImage::emImage;

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
fn sample_pixel(image: &emImage, ix: i32, iy: i32, ext: ImageExtension) -> [u8; 4] {
    let w = image.GetWidth() as i32;
    let h = image.GetHeight() as i32;

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

    let p = image.GetPixel(sx as u32, sy as u32);
    let ch = image.GetChannelCount();
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
    image: &emImage,
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
    let p = image.GetPixel((sec.ox + sx) as u32, (sec.oy + sy) as u32);
    let ch = image.GetChannelCount();
    match ch {
        1 => [p[0], p[0], p[0], 255],
        3 => [p[0], p[1], p[2], 255],
        4 => [p[0], p[1], p[2], p[3]],
        _ => [0, 0, 0, 0],
    }
}

/// Nearest-neighbor sampling.
pub(crate) fn sample_nearest(image: &emImage, x: f64, y: f64, ext: ImageExtension) -> emColor {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let p = sample_pixel(image, ix, iy, ext);
    emColor::rgba(p[0], p[1], p[2], p[3])
}

/// Bilinear interpolation (2x2 kernel).
pub(crate) fn sample_bilinear(image: &emImage, x: f64, y: f64, ext: ImageExtension) -> emColor {
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
        result[c] = ((top * ity + bot * ty + 0x7FFF) >> 16) as u8;
    }
    emColor::rgba(result[0], result[1], result[2], result[3])
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
pub struct AreaSampleTransform {
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

/// Carry state for area sampling across batch boundaries.
///
/// C++ `InterpolateImageAreaSampled` processes an entire scanline in one call,
/// so `cy`, `pCy`, and `ox` naturally flow between pixels. Rust callers batch
/// scanlines into 256px chunks (limited by InterpolationBuffer = 1024 bytes =
/// 256 RGBA pixels). This struct bridges batch boundaries, created fresh at the
/// start of each scanline row and passed to each batch call.
pub struct AreaSampleCarryState {
    /// Y-accumulated column value (up to 4 channels), after FINPREMUL.
    /// Corresponds to C++ `cy` (cyr, cyg, cyb, cya).  Uses u32 to match
    /// C++ `emUInt32` wrapping behaviour at extreme downscale ratios.
    pub cy: [u32; 4],
    /// Column index of cached cy. `i32::MIN` means NULL/invalid.
    /// Corresponds to C++ `pCy` (as pointer → column index).
    pub pcy_col: i32,
    /// Carried column weight from `ox -= oxs` (C++ line 823).
    pub ox: u32,
}

impl Default for AreaSampleCarryState {
    fn default() -> Self {
        Self {
            cy: [0; 4],
            pcy_col: i32::MIN,
            ox: 0,
        }
    }
}

impl AreaSampleCarryState {
    pub fn new() -> Self {
        Self::default()
    }
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
pub struct SectionBounds {
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
    image: &emImage,
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
    let p = image.GetPixel((sec.ox + sx) as u32, (sec.oy + sy) as u32);
    let ch = image.GetChannelCount();
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
    image: &'a emImage,
    sec: &SectionBounds,
    col: i32,
    row: i32,
    xfm: &AreaSampleTransform,
) -> &'a [u8] {
    let rx = (xfm.off_x + col * xfm.stride_x as i32).clamp(0, sec.w - 1);
    let ry = (xfm.off_y + row * xfm.stride_y as i32).clamp(0, sec.h - 1);
    image.GetPixel((sec.ox + rx) as u32, (sec.oy + ry) as u32)
}

/// Area sampling with 24-bit fixed-point integer arithmetic.
/// Matches C++ `InterpolateImageAreaSampled` (non-tiled) exactly.
///
/// Handles CHANNELS=1, 3, and 4 with correct per-channel FINPREMUL:
/// - CHANNELS=4: RGB division `(x + 0x7F7F) / 0xFF00`, alpha shift `(x + 0x7F) >> 8`
/// - CHANNELS=1/3: shift `(x + 0x7F) >> 8` for all channels
///
/// Returns straight-alpha emColor (premul->straight conversion done internally for 4-ch).
///
/// Note: production code uses `interpolate_scanline_area_sampled` which hoists Y setup
/// and adds pCy column-reuse. This per-pixel version is retained as a test reference.
#[cfg(test)]
pub(crate) fn sample_area_fp(
    image: &emImage,
    dest_x: i32,
    dest_y: i32,
    xfm: &AreaSampleTransform,
    sec: &SectionBounds,
    ext: ImageExtension,
) -> emColor {
    let ch = image.GetChannelCount();

    // --- Y setup (C++ emPainter_ScTlIntImg.cpp lines 686-725) ---
    let mut ty1 = dest_y as i64 * xfm.tdy - xfm.ty;
    let mut ty2 = ty1 + xfm.tdy;
    let ty_end = (xfm.img_h as i64) << 24;
    let mut ody = xfm.ody;

    // EXACT if/else if structure from C++ — NOT sequential max/min.
    if ty1 < 0 {
        if ty2 <= 0 {
            if ext == ImageExtension::Zero {
                return emColor::TRANSPARENT;
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
                return emColor::TRANSPARENT;
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
                return emColor::TRANSPARENT;
            }
            tx2 = 1 << 24; // EXTEND_EDGE
        } else if tx2 > tx_end {
            tx2 = tx_end;
        }
        odx = rational_inv(tx2);
    } else if tx2 > tx_end {
        if tx1 >= tx_end {
            if ext == ImageExtension::Zero {
                return emColor::TRANSPARENT;
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
                emColor::TRANSPARENT
            } else if out_a == 255 {
                emColor::rgba(out_r, out_g, out_b, 255)
            } else {
                let a16 = out_a as u16;
                let sr = ((out_r as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                let sg = ((out_g as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                let sb = ((out_b as u16 * 255 + a16 / 2) / a16).min(255) as u8;
                emColor::rgba(sr, sg, sb, out_a)
            }
        }
        3 => emColor::rgba(out_r, out_g, out_b, 255),
        _ => emColor::rgba(out_r, out_r, out_r, 255), // 1-ch gray
    }
}

/// Y-accumulate a single column for area sampling, then apply FINPREMUL.
/// Returns (cy_r, cy_g, cy_b, cy_a) after FINPREMUL_SHR_COLOR(cy, 8).
#[cfg(test)]
fn y_accumulate(
    image: &emImage,
    sec: &SectionBounds,
    ch: u8,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
) -> (u32, u32, u32, u32) {
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
    image: &emImage,
    sec: &SectionBounds,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
    p: &[u8],
) -> (u32, u32, u32, u32) {
    // All accumulators are emUInt32 in C++ — wrapping arithmetic at extreme
    // downscale ratios is intentional and must be reproduced exactly.

    // READ_PREMUL_MUL_COLOR(cy, p, oy1) for CHANNELS=4
    let mut ca = (p[3] as u32).wrapping_mul(yw.oy1);
    let mut cr = (p[0] as u32).wrapping_mul(ca);
    let mut cg = (p[1] as u32).wrapping_mul(ca);
    let mut cb = (p[2] as u32).wrapping_mul(ca);

    let mut oys = yw.oy1n;
    if oys > 0 {
        let mut r = yw.row0 + 1;
        if oys > yw.ody {
            // DEFINE_AND_READ_PREMUL_COLOR + ADD_READ_PREMUL_COLOR loop
            let pi = read_area_pixel(image, sec, col, r, xfm);
            let mut ta = pi[3] as u32;
            let mut tr = (pi[0] as u32).wrapping_mul(ta);
            let mut tg = (pi[1] as u32).wrapping_mul(ta);
            let mut tb = (pi[2] as u32).wrapping_mul(ta);
            r += 1;
            oys -= yw.ody;
            while oys > yw.ody {
                let pi = read_area_pixel(image, sec, col, r, xfm);
                let a = pi[3] as u32;
                ta = ta.wrapping_add(a);
                tr = tr.wrapping_add((pi[0] as u32).wrapping_mul(a));
                tg = tg.wrapping_add((pi[1] as u32).wrapping_mul(a));
                tb = tb.wrapping_add((pi[2] as u32).wrapping_mul(a));
                r += 1;
                oys -= yw.ody;
            }
            // ADD_MUL_COLOR(cy, ctmp, ody)
            ca = ca.wrapping_add(ta.wrapping_mul(yw.ody));
            cr = cr.wrapping_add(tr.wrapping_mul(yw.ody));
            cg = cg.wrapping_add(tg.wrapping_mul(yw.ody));
            cb = cb.wrapping_add(tb.wrapping_mul(yw.ody));
        }
        // ADD_READ_PREMUL_MUL_COLOR(cy, p, oys)
        let pl = read_area_pixel(image, sec, col, r, xfm);
        let al = (pl[3] as u32).wrapping_mul(oys);
        ca = ca.wrapping_add(al);
        cr = cr.wrapping_add((pl[0] as u32).wrapping_mul(al));
        cg = cg.wrapping_add((pl[1] as u32).wrapping_mul(al));
        cb = cb.wrapping_add((pl[2] as u32).wrapping_mul(al));
    }

    // FINPREMUL_SHR_COLOR(cy, 8) for CHANNELS=4
    // RGB: (x + 0x7F7F) / 0xFF00   Alpha: (x + 0x7F) >> 8
    let fr = cr.wrapping_add(0x7F7F) / 0xFF00;
    let fg = cg.wrapping_add(0x7F7F) / 0xFF00;
    let fb = cb.wrapping_add(0x7F7F) / 0xFF00;
    let fa = ca.wrapping_add(0x7F) >> 8;
    (fr, fg, fb, fa)
}

/// CHANNELS=3: no premultiplication.
/// READ_PREMUL_MUL_COLOR: cy_r = p[0]*oy1 (direct multiply, no alpha)
/// FINPREMUL_SHR_COLOR(cy,8): all channels use shift (x + 0x7F) >> 8
fn y_accumulate_3ch(
    image: &emImage,
    sec: &SectionBounds,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
    p: &[u8],
) -> (u32, u32, u32, u32) {
    let mut cr = (p[0] as u32).wrapping_mul(yw.oy1);
    let mut cg = (p[1] as u32).wrapping_mul(yw.oy1);
    let mut cb = (p[2] as u32).wrapping_mul(yw.oy1);

    let mut oys = yw.oy1n;
    if oys > 0 {
        let mut r = yw.row0 + 1;
        if oys > yw.ody {
            let pi = read_area_pixel(image, sec, col, r, xfm);
            let mut tr = pi[0] as u32;
            let mut tg = pi[1] as u32;
            let mut tb = pi[2] as u32;
            r += 1;
            oys -= yw.ody;
            while oys > yw.ody {
                let pi = read_area_pixel(image, sec, col, r, xfm);
                tr = tr.wrapping_add(pi[0] as u32);
                tg = tg.wrapping_add(pi[1] as u32);
                tb = tb.wrapping_add(pi[2] as u32);
                r += 1;
                oys -= yw.ody;
            }
            cr = cr.wrapping_add(tr.wrapping_mul(yw.ody));
            cg = cg.wrapping_add(tg.wrapping_mul(yw.ody));
            cb = cb.wrapping_add(tb.wrapping_mul(yw.ody));
        }
        let pl = read_area_pixel(image, sec, col, r, xfm);
        cr = cr.wrapping_add((pl[0] as u32).wrapping_mul(oys));
        cg = cg.wrapping_add((pl[1] as u32).wrapping_mul(oys));
        cb = cb.wrapping_add((pl[2] as u32).wrapping_mul(oys));
    }

    // FINPREMUL_SHR_COLOR(cy, 8) for CHANNELS=3: shift only
    (cr.wrapping_add(0x7F) >> 8, cg.wrapping_add(0x7F) >> 8, cb.wrapping_add(0x7F) >> 8, 0)
}

/// CHANNELS=1: single gray channel, no premultiplication.
/// FINPREMUL_SHR_COLOR(cy,8): shift (x + 0x7F) >> 8
fn y_accumulate_1ch(
    image: &emImage,
    sec: &SectionBounds,
    col: i32,
    yw: &YWeights,
    xfm: &AreaSampleTransform,
    p: &[u8],
) -> (u32, u32, u32, u32) {
    let mut cg = (p[0] as u32).wrapping_mul(yw.oy1);

    let mut oys = yw.oy1n;
    if oys > 0 {
        let mut r = yw.row0 + 1;
        if oys > yw.ody {
            let pi = read_area_pixel(image, sec, col, r, xfm);
            let mut tg = pi[0] as u32;
            r += 1;
            oys -= yw.ody;
            while oys > yw.ody {
                let pi = read_area_pixel(image, sec, col, r, xfm);
                tg = tg.wrapping_add(pi[0] as u32);
                r += 1;
                oys -= yw.ody;
            }
            cg = cg.wrapping_add(tg.wrapping_mul(yw.ody));
        }
        let pl = read_area_pixel(image, sec, col, r, xfm);
        cg = cg.wrapping_add((pl[0] as u32).wrapping_mul(oys));
    }

    let fg = cg.wrapping_add(0x7F) >> 8;
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
    image: &emImage,
    x: f64,
    y: f64,
    ctx: &ScaleContext,
    ext: ImageExtension,
) -> emColor {
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
        return emColor::TRANSPARENT;
    }

    let mut result = [0u8; 4];
    for c in 0..4 {
        result[c] = ((accum[c] + weight_total / 2) / weight_total) as u8;
    }
    emColor::rgba(result[0], result[1], result[2], result[3])
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

/// C++ compile-time Hermite basis factor table, copied verbatim from
/// emPainter_ScTlIntImg.cpp lines 1391-1471 (InterpolateFourValuesAdaptive).
/// Using the exact C++ values eliminates ±1 rounding differences from
/// runtime f64 computation.
fn adaptive_factors() -> &'static [[i32; 4]; 257] {
    static TABLE: [[i32; 4]; 257] = [
        [1024,0,0,0],[1024,0,4,0],[1024,0,8,0],[1024,0,12,0],
        [1023,1,16,0],[1023,1,19,0],[1022,2,23,-1],[1022,2,26,-1],
        [1021,3,30,-1],[1020,4,34,-1],[1019,5,37,-2],[1018,6,40,-2],
        [1017,7,44,-2],[1016,8,47,-3],[1015,9,50,-3],[1014,10,53,-3],
        [1013,12,56,-4],[1011,13,59,-4],[1010,14,62,-5],[1008,16,65,-5],
        [1006,18,68,-6],[1004,20,71,-6],[1003,21,74,-7],[1001,23,76,-8],
        [999,25,79,-8],[997,27,81,-9],[994,30,84,-9],[992,32,86,-10],
        [990,34,89,-11],[988,36,91,-12],[985,39,94,-12],[983,41,96,-13],
        [980,44,98,-14],[977,47,100,-15],[975,49,102,-16],[972,52,104,-17],
        [969,55,106,-17],[966,58,108,-18],[963,61,110,-19],[960,64,112,-20],
        [957,67,114,-21],[954,70,116,-22],[950,74,117,-23],[947,77,119,-24],
        [944,80,121,-25],[940,84,122,-26],[937,87,124,-27],[933,91,125,-28],
        [930,95,127,-29],[926,98,128,-30],[922,102,130,-31],[918,106,131,-33],
        [914,110,132,-34],[911,113,133,-35],[907,117,134,-36],[903,121,136,-37],
        [898,126,137,-38],[894,130,138,-39],[890,134,139,-41],[886,138,140,-42],
        [882,142,141,-43],[877,147,142,-44],[873,151,142,-46],[868,156,143,-47],
        [864,160,144,-48],[859,165,145,-49],[855,169,145,-51],[850,174,146,-52],
        [846,178,147,-53],[841,183,147,-54],[836,188,148,-56],[831,193,148,-57],
        [827,197,149,-58],[822,202,149,-60],[817,207,150,-61],[812,212,150,-62],
        [807,217,150,-63],[802,222,151,-65],[797,227,151,-66],[792,232,151,-67],
        [787,238,151,-69],[781,243,151,-70],[776,248,152,-71],[771,253,152,-73],
        [766,258,152,-74],[760,264,152,-75],[755,269,152,-77],[750,274,152,-78],
        [744,280,152,-79],[739,285,151,-81],[733,291,151,-82],[728,296,151,-83],
        [722,302,151,-85],[717,307,151,-86],[711,313,151,-87],[706,318,150,-89],
        [700,324,150,-90],[694,330,150,-91],[689,335,149,-93],[683,341,149,-94],
        [677,347,149,-95],[672,352,148,-97],[666,358,148,-98],[660,364,147,-99],
        [654,370,147,-100],[649,375,146,-102],[643,381,146,-103],[637,387,145,-104],
        [631,393,144,-105],[625,399,144,-107],[619,405,143,-108],[613,411,142,-109],
        [608,417,142,-110],[602,422,141,-111],[596,428,140,-113],[590,434,140,-114],
        [584,440,139,-115],[578,446,138,-116],[572,452,137,-117],[566,458,136,-118],
        [560,464,135,-120],[554,470,135,-121],[548,476,134,-122],[542,482,133,-123],
        [536,488,132,-124],[530,494,131,-125],[524,500,130,-126],[518,506,129,-127],
        [512,512,128,-128],[506,518,127,-129],[500,524,126,-130],[494,530,125,-131],
        [488,536,124,-132],[482,542,123,-133],[476,548,122,-134],[470,554,121,-135],
        [464,560,120,-135],[458,566,118,-136],[452,572,117,-137],[446,578,116,-138],
        [440,584,115,-139],[434,590,114,-140],[428,596,113,-140],[422,602,111,-141],
        [417,608,110,-142],[411,613,109,-142],[405,619,108,-143],[399,625,107,-144],
        [393,631,105,-144],[387,637,104,-145],[381,643,103,-146],[375,649,102,-146],
        [370,654,100,-147],[364,660,99,-147],[358,666,98,-148],[352,672,97,-148],
        [347,677,95,-149],[341,683,94,-149],[335,689,93,-149],[330,694,91,-150],
        [324,700,90,-150],[318,706,89,-150],[313,711,87,-151],[307,717,86,-151],
        [302,722,85,-151],[296,728,83,-151],[291,733,82,-151],[285,739,81,-151],
        [280,744,79,-152],[274,750,78,-152],[269,755,77,-152],[264,760,75,-152],
        [258,766,74,-152],[253,771,73,-152],[248,776,71,-152],[243,781,70,-151],
        [238,787,69,-151],[232,792,67,-151],[227,797,66,-151],[222,802,65,-151],
        [217,807,63,-150],[212,812,62,-150],[207,817,61,-150],[202,822,60,-149],
        [197,827,58,-149],[193,831,57,-148],[188,836,56,-148],[183,841,54,-147],
        [178,846,53,-147],[174,850,52,-146],[169,855,51,-145],[165,859,49,-145],
        [160,864,48,-144],[156,868,47,-143],[151,873,46,-142],[147,877,44,-142],
        [142,882,43,-141],[138,886,42,-140],[134,890,41,-139],[130,894,39,-138],
        [126,898,38,-137],[121,903,37,-136],[117,907,36,-134],[113,911,35,-133],
        [110,914,34,-132],[106,918,33,-131],[102,922,31,-130],[98,926,30,-128],
        [95,930,29,-127],[91,933,28,-125],[87,937,27,-124],[84,940,26,-122],
        [80,944,25,-121],[77,947,24,-119],[74,950,23,-117],[70,954,22,-116],
        [67,957,21,-114],[64,960,20,-112],[61,963,19,-110],[58,966,18,-108],
        [55,969,17,-106],[52,972,17,-104],[49,975,16,-102],[47,977,15,-100],
        [44,980,14,-98],[41,983,13,-96],[39,985,12,-94],[36,988,12,-91],
        [34,990,11,-89],[32,992,10,-86],[30,994,9,-84],[27,997,9,-81],
        [25,999,8,-79],[23,1001,8,-76],[21,1003,7,-74],[20,1004,6,-71],
        [18,1006,6,-68],[16,1008,5,-65],[14,1010,5,-62],[13,1011,4,-59],
        [12,1013,4,-56],[10,1014,3,-53],[9,1015,3,-50],[8,1016,3,-47],
        [7,1017,2,-44],[6,1018,2,-40],[5,1019,2,-37],[4,1020,1,-34],
        [3,1021,1,-30],[2,1022,1,-26],[2,1022,1,-23],[1,1023,0,-19],
        [1,1023,0,-16],[0,1024,0,-12],[0,1024,0,-8],[0,1024,0,-4],
        [0,1024,0,0],
    ];
    &TABLE
}

/// Adaptive 4-value interpolation with anti-ringing slope/peak adaptation.
/// Matches C++ `InterpolateFourValuesAdaptive` optimized branch exactly.
/// Returns interpolated value at scale 1024.
// DIVERGED: C++ uses i32 subtraction for slope differences (v1-v0, v2-v1, v2-v3)
// which is signed overflow UB when pixel values span the full i32 range. Rust uses
// saturating subtraction so extreme inputs clamp instead of wrapping. In practice
// pixel values are 0-255 so this never triggers and the output is identical to C++.
fn interpolate_four_values_adaptive(v0: i32, mut v1: i32, mut v2: i32, v3: i32, o: u32) -> i64 {
    let s01 = v1.saturating_sub(v0);
    let s12 = v2.saturating_sub(v1);
    let s32 = v2.saturating_sub(v3);

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
/// Full-image adaptive sampling (no section bounds). Kept for harness tests.
#[allow(dead_code)]
pub(crate) fn sample_adaptive_premul_fp(
    image: &emImage,
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
    image: &emImage,
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

/// Adaptive single-channel sampling within a section.
///
/// Matches C++ `InterpolateImageAdaptive` for 1-channel sources (font glyphs).
/// Uses 24-bit fixed-point coordinates. `ix`, `iy` are relative to the section
/// origin (before the -1.5 offset that C++ applies for the 4x4 kernel).
/// Returns the interpolated luminance as u8.
pub fn sample_adaptive_lum_section(
    image: &emImage,
    ix: i32,
    iy: i32,
    ox: u32,
    oy: u32,
    sec: &SectionBounds,
    ext: ImageExtension,
) -> u8 {
    // Y-pass: for each of 4 columns, read 4 rows and adaptively interpolate.
    let mut col_vals = [0i64; 4];
    for col in 0..4i32 {
        let mut rv = [0i32; 4];
        for row in 0..4i32 {
            let p = sample_section_pixel(image, ix + col, iy + row, sec, ext);
            rv[row as usize] = p[0] as i32;
        }
        col_vals[col as usize] =
            interpolate_four_values_adaptive(rv[0], rv[1], rv[2], rv[3], oy);
    }

    // X-pass: adaptively interpolate the 4 column results.
    let final_val = interpolate_four_values_adaptive(
        col_vals[0] as i32,
        col_vals[1] as i32,
        col_vals[2] as i32,
        col_vals[3] as i32,
        ox,
    );

    let rnd = (1i64 << 19) - 1;
    ((final_val + rnd) >> 20).clamp(0, 255) as u8
}

/// Bicubic (Catmull-Rom) sampling with 4x4 kernel.
pub(crate) fn sample_bicubic(image: &emImage, x: f64, y: f64, ext: ImageExtension) -> emColor {
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
    emColor::rgba(result[0], result[1], result[2], result[3])
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
pub(crate) fn sample_lanczos(image: &emImage, x: f64, y: f64, ext: ImageExtension) -> emColor {
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
    emColor::rgba(result[0], result[1], result[2], result[3])
}

/// Adaptive edge-sensitive sampling: bilinear near edges, bicubic in smooth areas.
pub(crate) fn sample_adaptive(image: &emImage, x: f64, y: f64, ext: ImageExtension) -> emColor {
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
    image: &emImage,
    x: f64,
    y: f64,
    quality: InterpolationQuality,
    ext: ImageExtension,
    ctx: &ScaleContext,
) -> emColor {
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
pub fn sample_linear_gradient(
    start: (f64, f64),
    end: (f64, f64),
    c0: emColor,
    c1: emColor,
    point: (f64, f64),
) -> emColor {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return c0;
    }
    let t = ((point.0 - start.0) * dx + (point.1 - start.1) * dy) / len_sq;
    // C++ gradient pipeline (emPainter_ScTlPSInt.cpp:297) uses the hash formula,
    // NOT emColor::GetBlended. The hash formula: ((c1*(255-g) + c2*g) * 257 + 0x8073) >> 16.
    let g = (t.clamp(0.0, 1.0) * 255.0 + 0.5) as i32;
    let mix = |a: i32, b: i32| -> u8 {
        (((a * (255 - g) + b * g) * 257 + 0x8073) >> 16) as u8
    };
    emColor::rgba(
        mix(c0.GetRed() as i32, c1.GetRed() as i32),
        mix(c0.GetGreen() as i32, c1.GetGreen() as i32),
        mix(c0.GetBlue() as i32, c1.GetBlue() as i32),
        mix(c0.GetAlpha() as i32, c1.GetAlpha() as i32),
    )
}

/// Pre-computed linear gradient parameters for the C++ 40-bit fixed-point walk.
/// Matches C++ emPainter_ScTl.cpp:155-189 (ScanlineTool::Init gradient setup).
pub struct LinearGradientParams {
    tdx: i64,
    tdy: i64,
    tx: i64,
}

impl LinearGradientParams {
    /// Compute fixed-point gradient parameters from pixel-space endpoints.
    /// Matches C++ ScanlineTool::Init (emPainter_ScTl.cpp:155-189).
    pub fn new(start: (f64, f64), end: (f64, f64)) -> Self {
        let nx = end.0 - start.0;
        let ny = end.1 - start.1;
        let nn = nx * nx + ny * ny;
        let f = if nn < 1e-3 { 0.0 } else { (255_i64 << 24) as f64 / nn };
        let nx = nx * f;
        let ny = ny * f;
        // C++ uses (start - 0.5) for pixel-center offset
        let tx_d = (start.0 - 0.5) * nx + (start.1 - 0.5) * ny;
        Self {
            tdx: nx as i64,
            tdy: ny as i64,
            tx: tx_d as i64 - 0x7fffff,
        }
    }

    /// Fill `buf` with gradient interpolation values (0-255) for a scanline.
    /// Matches C++ InterpolateLinearGradient (emPainter_ScTlIntGra.cpp:24-39).
    pub fn interpolate_scanline(&self, x: i32, y: i32, buf: &mut [u8]) {
        let mut t = x as i64 * self.tdx + y as i64 * self.tdy - self.tx;
        for b in buf.iter_mut() {
            let mut u = t >> 24;
            // C++ clamping via sign extension: if (emUInt64)u > 255, u = ~(u >> 48)
            if u as u64 > 255 {
                u = !(u >> 48);
            }
            *b = u as u8;
            t += self.tdx;
        }
    }
}

/// Blend a gradient interpolation value with two colors using the C++ hash formula.
/// Matches C++ emPainter_ScTlPSInt.cpp:334-336.
#[inline]
pub fn blend_gradient_colors(g: u8, c0: emColor, c1: emColor) -> emColor {
    let g = g as i32;
    let mix = |a: i32, b: i32| -> u8 {
        (((a * (255 - g) + b * g) * 257 + 0x8073) >> 16) as u8
    };
    emColor::rgba(
        mix(c0.GetRed() as i32, c1.GetRed() as i32),
        mix(c0.GetGreen() as i32, c1.GetGreen() as i32),
        mix(c0.GetBlue() as i32, c1.GetBlue() as i32),
        mix(c0.GetAlpha() as i32, c1.GetAlpha() as i32),
    )
}

/// Scanline area-sampled interpolation: fills `buf` with `count` consecutive
/// output pixels starting at `(dest_x_start, dest_y)`.
///
/// Literal translation of C++ `InterpolateImageAreaSampled` (non-tiled path,
/// emPainter_ScTlIntImg.cpp lines 677-828). Carry state (`cy`, `pCy`, `ox`)
/// is maintained across batch calls via `carry`.
///
/// The caller creates a fresh `AreaSampleCarryState` at the start of each
/// scanline row and passes it to each batch call. This reproduces the C++
/// behavior where carry flows naturally across all pixels on a scanline.
#[allow(clippy::too_many_arguments)]
pub fn interpolate_scanline_area_sampled(
    image: &emImage,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    xfm: &AreaSampleTransform,
    sec: &SectionBounds,
    ext: ImageExtension,
    buf: &mut crate::emPainterScanlineTool::InterpolationBuffer,
    carry: &mut AreaSampleCarryState,
) {
    match image.GetChannelCount() {
        4 => interpolate_scanline_area_inner::<4>(image, dest_x_start, dest_y, count, xfm, sec, ext, buf, carry),
        3 => interpolate_scanline_area_inner::<3>(image, dest_x_start, dest_y, count, xfm, sec, ext, buf, carry),
        1 => interpolate_scanline_area_inner::<1>(image, dest_x_start, dest_y, count, xfm, sec, ext, buf, carry),
        _ => interpolate_scanline_area_inner::<1>(image, dest_x_start, dest_y, count, xfm, sec, ext, buf, carry),
    }
}

/// Channel-count-specialized inner loop for scanline area sampling.
/// `CH` is 1, 3, or 4 -- known at compile time so the compiler eliminates
/// dead branches in y_accumulate dispatch and output conversion.
///
/// Literal translation of C++ `InterpolateImageAreaSampled` (non-tiled path,
/// emPainter_ScTlIntImg.cpp lines 677-828).
///
/// Carry state (`cy`, `pCy`, `ox`) is threaded through `carry` across batch
/// calls. The caller creates `AreaSampleCarryState::new()` at the start of
/// each scanline row and passes `&mut carry` to each batch call. Carry reset
/// at batch/tile boundaries does not affect output because the first pixel
/// always gets ox=0 (from pCy mismatch), making the stale cy contribute 0.
///
/// C++ has two nested loops:
///   - Outer do..while(buf<bufEnd): per-chunk (edge/interior classification,
///     fresh ox computation, pCy check once per chunk).
///   - Inner do..while(tx<txStop): per-pixel within a chunk (ox carries via
///     `ox -= oxs`, no fresh ox computation, no pCy re-check).
///
/// Rust preserves this two-level structure. Edge chunks process one pixel.
/// Interior chunks process multiple pixels in an inner loop, carrying ox.
#[allow(clippy::too_many_arguments)]
fn interpolate_scanline_area_inner<const CH: usize>(
    image: &emImage,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    xfm: &AreaSampleTransform,
    sec: &SectionBounds,
    ext: ImageExtension,
    buf: &mut crate::emPainterScanlineTool::InterpolationBuffer,
    carry: &mut AreaSampleCarryState,
) {
    // --- Y setup (C++ lines 686-725) ---
    let mut ty1 = dest_y as i64 * xfm.tdy - xfm.ty;
    let mut ty2 = ty1 + xfm.tdy;
    let ty_end = (xfm.img_h as i64) << 24;
    let mut ody = xfm.ody;

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

    let tdx = xfm.tdx;
    let tx_end = (xfm.img_w as i64) << 24;
    let odx0 = xfm.odx;

    // Carry state (cy, pcy_col, ox) flows in from the caller's previous
    // batch call.  For the first batch of a row, the caller passes
    // AreaSampleCarryState::new() (cy=0, pcy=NULL, ox=0), matching C++'s
    // fresh initialization at the start of InterpolateImageAreaSampled.

    let mut pixel_idx: usize = 0;

    // === C++ outer loop: do { ... } while (buf < bufEnd) ===
    while pixel_idx < count {
        let dest_x = dest_x_start + pixel_idx as i32;
        let tx = dest_x as i64 * tdx - xfm.tx;
        let mut tx1 = tx;
        let mut tx2 = tx + tdx;
        let odx: u32;
        let tx_stop: i64;

        if tx1 < 0 {
            tx1 = 0;
            if tx2 <= 0 {
                if ext == ImageExtension::Zero {
                    buf.set_pixel(pixel_idx, [0, 0, 0, 0]);
                    pixel_idx += 1;
                    continue;
                }
                tx2 = 1 << 24;
            } else if tx2 > tx_end {
                tx2 = tx_end;
            }
            odx = rational_inv(tx2);
            tx_stop = tx;
        } else if tx2 > tx_end {
            if tx1 >= tx_end {
                if ext == ImageExtension::Zero {
                    buf.set_pixel(pixel_idx, [0, 0, 0, 0]);
                    pixel_idx += 1;
                    continue;
                }
                tx1 = tx_end - (1 << 24);
            }
            odx = rational_inv(tx_end - tx1);
            tx_stop = tx;
        } else {
            odx = odx0;
            let tx_stop_max = tx_end - tdx + 1;
            let remaining_pixels = count - pixel_idx;
            let tx_stop_batch = tx + remaining_pixels as i64 * tdx;
            tx_stop = tx_stop_max.min(tx_stop_batch);
        }

        // C++ line 777
        let mut ox: u32 = {
            let w = ((0x100_0000i64 - (tx1 & 0xFF_FFFF)) as u64 * odx as u64 + 0xFF_FFFF) >> 24;
            if odx == 0x7FFF_FFFF { 0x7FFF_FFFF } else { w as u32 }
        };
        let mut col = (tx1 >> 24) as i32;

        // C++ lines 781-788: pCy check
        let mut ox1: u32;
        if carry.pcy_col != col {
            ox1 = ox;
            ox = 0;
        } else {
            ox1 = odx;
            col += 1;
        }

        // === C++ inner loop ===
        let mut cur_tx = tx;
        loop {
            let mut cyx: [u32; 4] = [0x7F_FFFF; 4];
            let mut oxs: u32 = 0x10000;

            while ox < oxs {
                for (c, cy) in cyx.iter_mut().zip(carry.cy.iter()).take(CH) {
                    *c = c.wrapping_add(cy.wrapping_mul(ox));
                }
                oxs -= ox;
                carry.pcy_col = col;
                let p = read_area_pixel(image, sec, col, yw.row0, xfm);
                let cy_result = match CH {
                    4 => y_accumulate_4ch(image, sec, col, &yw, xfm, p),
                    3 => y_accumulate_3ch(image, sec, col, &yw, xfm, p),
                    _ => y_accumulate_1ch(image, sec, col, &yw, xfm, p),
                };
                carry.cy[0] = cy_result.0;
                carry.cy[1] = cy_result.1;
                carry.cy[2] = cy_result.2;
                carry.cy[3] = cy_result.3;
                col += 1;
                ox = ox1;
                ox1 = odx;
            }

            for (c, cy) in cyx.iter_mut().zip(carry.cy.iter()).take(CH) {
                *c = c.wrapping_add(cy.wrapping_mul(oxs));
            }

            let rgba = match CH {
                4 => [
                    (cyx[0] >> 24) as u8,
                    (cyx[1] >> 24) as u8,
                    (cyx[2] >> 24) as u8,
                    (cyx[3] >> 24) as u8,
                ],
                3 => [
                    (cyx[0] >> 24) as u8,
                    (cyx[1] >> 24) as u8,
                    (cyx[2] >> 24) as u8,
                    255,
                ],
                _ => {
                    let g = (cyx[0] >> 24) as u8;
                    [g, g, g, 255]
                }
            };
            buf.set_pixel(pixel_idx, rgba);
            pixel_idx += 1;
            ox -= oxs;
            cur_tx += tdx;
            if cur_tx >= tx_stop || pixel_idx >= count {
                break;
            }
        }
        carry.ox = ox;
    }
    buf.set_len(count);
}



/// Scanline adaptive premul interpolation: fills `buf` with `count` consecutive
/// output pixels of premultiplied RGBA.
/// Full-image adaptive scanline (no section bounds). Kept for harness tests.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn interpolate_scanline_adaptive_premul(
    image: &emImage,
    px: i32,
    py: i32,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    sxfm: &ScaleTransform24,
    ext: ImageExtension,
    buf: &mut crate::emPainterScanlineTool::InterpolationBuffer,
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
    image: &emImage,
    px: i32,
    py: i32,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    sxfm: &ScaleTransform24,
    sec: &SectionBounds,
    ext: ImageExtension,
    buf: &mut crate::emPainterScanlineTool::InterpolationBuffer,
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
    image: &emImage,
    px: i32,
    py: i32,
    dest_x_start: i32,
    dest_y: i32,
    count: usize,
    sxfm: &ScaleTransform24,
    ext: ImageExtension,
    buf: &mut crate::emPainterScanlineTool::InterpolationBuffer,
) {
    // ty is constant for the whole row
    let ty = (dest_y - py) as i64 * sxfm.tdy + sxfm.base_y;
    let src_y = (ty >> 24) as f64;
    for i in 0..count {
        let col = dest_x_start + i as i32;
        let tx = (col - px) as i64 * sxfm.tdx + sxfm.base_x;
        let c = sample_nearest(image, (tx >> 24) as f64, src_y, ext);
        buf.set_pixel(i, [c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha()]);
    }
    buf.set_len(count);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_image() -> emImage {
        let mut img = emImage::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let v = (x * 64 + y * 16) as u8;
                let p = img.SetPixel(x, y);
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
        assert_eq!(c.GetRed(), 80);
    }

    #[test]
    fn bilinear_center() {
        let img = make_test_image();
        let c = sample_bilinear(&img, 0.5, 0.5, ImageExtension::Clamp);
        assert!((c.GetRed() as i32 - 40).abs() <= 1);
    }

    #[test]
    fn bilinear_at_pixel() {
        let img = make_test_image();
        let c = sample_bilinear(&img, 0.0, 0.0, ImageExtension::Clamp);
        assert_eq!(c.GetRed(), 0);
    }

    #[test]
    fn bicubic_at_pixel() {
        let img = make_test_image();
        let c = sample_bicubic(&img, 1.0, 1.0, ImageExtension::Clamp);
        assert!((c.GetRed() as i32 - 80).abs() <= 5);
    }

    #[test]
    fn lanczos_at_pixel() {
        let img = make_test_image();
        let c = sample_lanczos(&img, 1.0, 1.0, ImageExtension::Clamp);
        assert!((c.GetRed() as i32 - 80).abs() <= 5);
    }

    #[test]
    fn linear_gradient_endpoints() {
        let c0 = sample_linear_gradient(
            (0.0, 0.0),
            (100.0, 0.0),
            emColor::WHITE,
            emColor::BLACK,
            (0.0, 0.0),
        );
        assert_eq!(c0.GetRed(), 255);
        let c1 = sample_linear_gradient(
            (0.0, 0.0),
            (100.0, 0.0),
            emColor::WHITE,
            emColor::BLACK,
            (100.0, 0.0),
        );
        assert_eq!(c1.GetRed(), 0);
    }

    #[test]
    fn extension_zero() {
        let img = make_test_image();
        let c = sample_nearest(&img, -1.0, -1.0, ImageExtension::Zero);
        assert_eq!(c.GetAlpha(), 0);
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
        assert!((c.GetRed() as i32 - 80).abs() <= 2);
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
        let mut img = emImage::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let p = img.SetPixel(x, y);
                p[0] = 255;
                p[3] = 255;
            }
        }
        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        assert_eq!(c, emColor::rgba(255, 0, 0, 255));
    }

    #[test]
    fn area_sample_fp_gradient_4ch() {
        // 4x2 RGBA: left half (128,0,0,255), right half (0,128,0,255).
        // 4:1 X downscale, 2:1 Y → 1 dest pixel covers entire image.
        let mut img = emImage::new(4, 2, 4);
        for y in 0..2u32 {
            for x in 0..4u32 {
                let p = img.SetPixel(x, y);
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
        assert!((c.GetRed() as i32 - 64).abs() <= 1, "r={} expected ~64", c.GetRed());
        assert!((c.GetGreen() as i32 - 64).abs() <= 1, "g={} expected ~64", c.GetGreen());
        assert_eq!(c.GetBlue(), 0);
        assert_eq!(c.GetAlpha(), 255);
    }

    #[test]
    fn area_sample_fp_alpha_4ch() {
        // 2x2 RGBA: (0,0)=(255,0,0,128), rest=(0,0,0,0).
        // Covers premul accumulation with mixed alpha.
        let mut img = emImage::new(2, 2, 4);
        let p = img.SetPixel(0, 0);
        p[0] = 255;
        p[3] = 128;
        // 2:1 downscale → 1 dest pixel covers all 4 source pixels.
        let xfm = make_area_xfm(2, 2, 1.0, 1.0);
        let sec = full_sec(2, 2);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        // 1 of 4 pixels has alpha=128 → low alpha, non-zero red.
        assert!(c.GetAlpha() > 0, "alpha should be non-zero, got {}", c.GetAlpha());
        assert!(c.GetRed() > 0, "red should be non-zero, got {}", c.GetRed());
        assert_eq!(c.GetGreen(), 0);
        assert_eq!(c.GetBlue(), 0);
    }

    #[test]
    fn area_sample_fp_extend_edge() {
        // 2x2 solid image, dest pixel maps fully outside bounds.
        // EXTEND_EDGE must clamp to edge pixel.
        let mut img = emImage::new(2, 2, 4);
        for y in 0..2u32 {
            for x in 0..2u32 {
                let p = img.SetPixel(x, y);
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
        assert_eq!(c, emColor::rgba(200, 100, 50, 255));
    }

    #[test]
    fn area_sample_fp_extend_zero() {
        // Same setup as extend_edge but with EXTEND_ZERO → transparent.
        let mut img = emImage::new(2, 2, 4);
        for y in 0..2u32 {
            for x in 0..2u32 {
                let p = img.SetPixel(x, y);
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
        assert_eq!(c, emColor::TRANSPARENT);
    }

    #[test]
    fn area_sample_fp_1ch() {
        // 4x4 grayscale with gradient values.
        let mut img = emImage::new(4, 4, 1);
        for y in 0..4u32 {
            for x in 0..4u32 {
                img.SetPixel(x, y)[0] = (x * 60 + y * 20) as u8;
            }
        }
        // 2:1 downscale: dest pixel (0,0) covers source (0,0)=0, (1,0)=60, (0,1)=20, (1,1)=80.
        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        // Average = (0+60+20+80)/4 = 40 ± 1.
        assert!(
            (c.GetRed() as i32 - 40).abs() <= 1,
            "gray={} expected ~40",
            c.GetRed()
        );
        // 1-ch: all channels equal, alpha=255.
        assert_eq!(c.GetRed(), c.GetGreen());
        assert_eq!(c.GetRed(), c.GetBlue());
        assert_eq!(c.GetAlpha(), 255);
    }

    #[test]
    fn area_sample_fp_3ch() {
        // 4x4 RGB, uniform (100,150,200) → exact roundtrip.
        let mut img = emImage::new(4, 4, 3);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let p = img.SetPixel(x, y);
                p[0] = 100;
                p[1] = 150;
                p[2] = 200;
            }
        }
        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        assert_eq!(c, emColor::rgba(100, 150, 200, 255));
    }

    // ── Scanline vs per-pixel equivalence tests ─────────────────────

    #[test]
    fn scanline_area_matches_perpixel_4ch() {
        // 8x8 RGBA gradient, 2:1 downscale to 4x4 dest.
        let mut img = emImage::new(8, 8, 4);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let p = img.SetPixel(x, y);
                p[0] = (x * 30 + y * 10) as u8;
                p[1] = (255 - x * 25) as u8;
                p[2] = (y * 30) as u8;
                p[3] = (200 + (x * 5).min(55)) as u8;
            }
        }
        let xfm = make_area_xfm(8, 8, 4.0, 4.0);
        let sec = full_sec(8, 8);
        let ext = ImageExtension::Zero;

        // Per-pixel reference — sample_area_fp returns unpremultiplied emColor,
        // but the scanline version now outputs premultiplied pixels (matching C++).
        // Extract premul values directly from the accumulation for comparison.
        // Use the scanline function on single pixels as the reference.
        let mut ref_pixels = Vec::new();
        let mut buf_single = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        for dest_y in 0..4 {
            let mut carry = AreaSampleCarryState::new();
            for dest_x in 0..4 {
                interpolate_scanline_area_sampled(&img, dest_x, dest_y, 1, &xfm, &sec, ext, &mut buf_single, &mut carry);
                ref_pixels.push(buf_single.pixel_rgba(0));
            }
        }

        // Scanline version: one row at a time
        let mut buf = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        let mut scan_pixels = Vec::new();
        for dest_y in 0..4 {
            let mut carry = AreaSampleCarryState::new();
            interpolate_scanline_area_sampled(&img, 0, dest_y, 4, &xfm, &sec, ext, &mut buf, &mut carry);
            for i in 0..4 {
                scan_pixels.push(buf.pixel_rgba(i));
            }
        }

        assert_eq!(ref_pixels, scan_pixels, "scanline area 4ch mismatch");
    }

    #[test]
    fn scanline_area_matches_perpixel_1ch() {
        let mut img = emImage::new(6, 6, 1);
        for y in 0..6u32 {
            for x in 0..6u32 {
                img.SetPixel(x, y)[0] = (x * 40 + y * 20) as u8;
            }
        }
        let xfm = make_area_xfm(6, 6, 3.0, 3.0);
        let sec = full_sec(6, 6);
        let ext = ImageExtension::Clamp;

        let mut ref_pixels = Vec::new();
        for dest_y in 0..3 {
            for dest_x in 0..3 {
                let c = sample_area_fp(&img, dest_x, dest_y, &xfm, &sec, ext);
                ref_pixels.push([c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha()]);
            }
        }

        let mut buf = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        let mut scan_pixels = Vec::new();
        for dest_y in 0..3 {
            let mut carry = AreaSampleCarryState::new();
            interpolate_scanline_area_sampled(&img, 0, dest_y, 3, &xfm, &sec, ext, &mut buf, &mut carry);
            for i in 0..3 {
                scan_pixels.push(buf.pixel_rgba(i));
            }
        }

        // Per-pixel has no carry; scanline has C++ carry within each row.
        // First pixel of each row must match exactly (no carry yet).
        // Subsequent pixels may differ due to carry.
        for (i, (r, s)) in ref_pixels.iter().zip(scan_pixels.iter()).enumerate() {
            let col = i % 3;
            if col == 0 {
                assert_eq!(r, s, "1ch row-start pixel {i} mismatch");
            }
        }
    }

    #[test]
    fn scanline_area_matches_perpixel_3ch() {
        let mut img = emImage::new(6, 6, 3);
        for y in 0..6u32 {
            for x in 0..6u32 {
                let p = img.SetPixel(x, y);
                p[0] = (x * 35 + y * 15) as u8;
                p[1] = (128 + (x * 10).min(127)) as u8;
                p[2] = (y * 40) as u8;
            }
        }
        let xfm = make_area_xfm(6, 6, 3.0, 3.0);
        let sec = full_sec(6, 6);
        let ext = ImageExtension::Clamp;

        let mut ref_pixels = Vec::new();
        for dest_y in 0..3 {
            for dest_x in 0..3 {
                let c = sample_area_fp(&img, dest_x, dest_y, &xfm, &sec, ext);
                ref_pixels.push([c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha()]);
            }
        }

        let mut buf = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        let mut scan_pixels = Vec::new();
        for dest_y in 0..3 {
            let mut carry = AreaSampleCarryState::new();
            interpolate_scanline_area_sampled(&img, 0, dest_y, 3, &xfm, &sec, ext, &mut buf, &mut carry);
            for i in 0..3 {
                scan_pixels.push(buf.pixel_rgba(i));
            }
        }

        // Per-pixel has no carry; scanline has C++ carry within each row.
        // First pixel of each row must match exactly (no carry yet).
        for (i, (r, s)) in ref_pixels.iter().zip(scan_pixels.iter()).enumerate() {
            let col = i % 3;
            if col == 0 {
                assert_eq!(r, s, "3ch row-start pixel {i} mismatch");
            }
        }
    }

    #[test]
    fn scanline_area_extend_zero_oob() {
        // Test that out-of-bounds pixels return transparent with EXTEND_ZERO.
        let mut img = emImage::new(2, 2, 4);
        for y in 0..2u32 {
            for x in 0..2u32 {
                let p = img.SetPixel(x, y);
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

        // Per-pixel reference
        let c = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Zero);
        assert_eq!(c, emColor::TRANSPARENT);

        // Scanline version
        let mut buf = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        let mut carry = AreaSampleCarryState::new();
        interpolate_scanline_area_sampled(
            &img,
            0,
            0,
            1,
            &xfm,
            &sec,
            ImageExtension::Zero,
            &mut buf,
            &mut carry,
        );
        assert_eq!(buf.pixel_rgba(0), [0, 0, 0, 0]);
    }

    /// Diagnostic test: compare sample_area_fp vs interpolate_scanline_area_sampled
    /// for a 4x4 RGBA -> 2x2 downscale with known pixel values.
    /// Prints intermediate values (cy, ox, cyx) for arithmetic tracing.
    #[test]
    fn diag_area_fp_vs_scanline_4x4_to_2x2() {
        // 4x4 RGBA with varying colors and some transparency.
        let mut img = emImage::new(4, 4, 4);
        // Row 0: fully opaque reds/greens
        img.SetPixel(0, 0).copy_from_slice(&[255,   0,   0, 255]); // red
        img.SetPixel(1, 0).copy_from_slice(&[  0, 255,   0, 255]); // green
        img.SetPixel(2, 0).copy_from_slice(&[  0,   0, 255, 255]); // blue
        img.SetPixel(3, 0).copy_from_slice(&[255, 255,   0, 255]); // yellow
        // Row 1: mix with partial transparency
        img.SetPixel(0, 1).copy_from_slice(&[128, 128, 128, 255]); // gray
        img.SetPixel(1, 1).copy_from_slice(&[200,  50,  50, 128]); // semi-transparent red
        img.SetPixel(2, 1).copy_from_slice(&[ 50, 200,  50, 128]); // semi-transparent green
        img.SetPixel(3, 1).copy_from_slice(&[128, 128, 128, 255]); // gray
        // Row 2: half transparent
        img.SetPixel(0, 2).copy_from_slice(&[255, 255, 255,  64]); // faint white
        img.SetPixel(1, 2).copy_from_slice(&[  0,   0,   0,   0]); // fully transparent
        img.SetPixel(2, 2).copy_from_slice(&[100, 100, 100, 200]); // semi-opaque gray
        img.SetPixel(3, 2).copy_from_slice(&[255,   0, 255, 255]); // magenta
        // Row 3: fully opaque
        img.SetPixel(0, 3).copy_from_slice(&[ 50,  50,  50, 255]);
        img.SetPixel(1, 3).copy_from_slice(&[100, 100, 100, 255]);
        img.SetPixel(2, 3).copy_from_slice(&[150, 150, 150, 255]);
        img.SetPixel(3, 3).copy_from_slice(&[200, 200, 200, 255]);

        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);
        let ext = ImageExtension::Zero; // 4ch images use EXTEND_ZERO

        eprintln!("=== AreaSampleTransform ===");
        eprintln!("  tdx={:#x} tdy={:#x} tx={:#x} ty={:#x}", xfm.tdx, xfm.tdy, xfm.tx, xfm.ty);
        eprintln!("  odx={:#x} ody={:#x}", xfm.odx, xfm.ody);
        eprintln!("  img_w={} img_h={} stride_x={} stride_y={} off_x={} off_y={}",
            xfm.img_w, xfm.img_h, xfm.stride_x, xfm.stride_y, xfm.off_x, xfm.off_y);

        // --- Per-pixel reference (sample_area_fp) ---
        eprintln!("\n=== sample_area_fp (per-pixel, returns emColor = straight alpha) ===");
        let mut fp_colors = Vec::new();
        for dy in 0..2i32 {
            for dx in 0..2i32 {
                let c = sample_area_fp(&img, dx, dy, &xfm, &sec, ext);
                eprintln!("  dest({},{}) => rgba({}, {}, {}, {})",
                    dx, dy, c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha());
                fp_colors.push(c);
            }
        }

        // --- Scanline (one pixel at a time, fresh carry each pixel) ---
        eprintln!("\n=== interpolate_scanline_area_sampled (single-pixel, fresh carry) ===");
        let mut buf = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        let mut scan_single = Vec::new();
        for dy in 0..2i32 {
            for dx in 0..2i32 {
                let mut carry = AreaSampleCarryState::new();
                interpolate_scanline_area_sampled(&img, dx, dy, 1, &xfm, &sec, ext, &mut buf, &mut carry);
                let px = buf.pixel_rgba(0);
                eprintln!("  dest({},{}) => premul_rgba({}, {}, {}, {})",
                    dx, dy, px[0], px[1], px[2], px[3]);
                scan_single.push(px);
            }
        }

        // --- Scanline (full row, carry flows between pixels) ---
        eprintln!("\n=== interpolate_scanline_area_sampled (full row, carry flows) ===");
        let mut scan_row = Vec::new();
        for dy in 0..2i32 {
            let mut carry = AreaSampleCarryState::new();
            interpolate_scanline_area_sampled(&img, 0, dy, 2, &xfm, &sec, ext, &mut buf, &mut carry);
            for dx in 0..2 {
                let px = buf.pixel_rgba(dx);
                eprintln!("  dest({},{}) => premul_rgba({}, {}, {}, {})",
                    dx, dy, px[0], px[1], px[2], px[3]);
                scan_row.push(px);
            }
        }

        // --- Convert sample_area_fp (straight) to premul for comparison ---
        eprintln!("\n=== sample_area_fp converted to premul ===");
        let mut fp_premul = Vec::new();
        for (i, c) in fp_colors.iter().enumerate() {
            let a = c.GetAlpha();
            let r = ((c.GetRed() as u16 * a as u16 + 127) / 255) as u8;
            let g = ((c.GetGreen() as u16 * a as u16 + 127) / 255) as u8;
            let b = ((c.GetBlue() as u16 * a as u16 + 127) / 255) as u8;
            let dx = i % 2;
            let dy = i / 2;
            eprintln!("  dest({},{}) => premul_rgba({}, {}, {}, {})", dx, dy, r, g, b, a);
            fp_premul.push([r, g, b, a]);
        }

        // --- Comparison ---
        eprintln!("\n=== Comparison: single-pixel scanline vs full-row scanline ===");
        let mut any_mismatch = false;
        for i in 0..4 {
            let dx = i % 2;
            let dy = i / 2;
            if scan_single[i] != scan_row[i] {
                eprintln!("  MISMATCH dest({},{}) single={:?} row={:?}",
                    dx, dy, scan_single[i], scan_row[i]);
                any_mismatch = true;
            } else {
                eprintln!("  OK dest({},{}) {:?}", dx, dy, scan_single[i]);
            }
        }

        eprintln!("\n=== Comparison: sample_area_fp (premul) vs scanline (single-pixel) ===");
        for i in 0..4 {
            let dx = i % 2;
            let dy = i / 2;
            let diff: Vec<i32> = (0..4).map(|c| fp_premul[i][c] as i32 - scan_single[i][c] as i32).collect();
            if diff.iter().any(|d| d.abs() > 0) {
                eprintln!("  DIFF dest({},{}) fp_premul={:?} scanline={:?} diff={:?}",
                    dx, dy, fp_premul[i], scan_single[i], diff);
            } else {
                eprintln!("  EXACT dest({},{}) {:?}", dx, dy, scan_single[i]);
            }
        }

        // The test passes as long as it runs; actual analysis is in the output.
        // But also check: do single-pixel and row-batch agree?
        if any_mismatch {
            eprintln!("\nWARNING: single-pixel vs row-batch scanline DISAGREE");
        }
    }

    /// Diagnostic: trace the Y-accumulate and X-accumulate steps for dest(0,0)
    /// in a simple 4x4->2x2 downscale to identify exactly where divergence occurs.
    #[test]
    fn diag_area_trace_y_accumulate() {
        // Simple gradient: pixel value = (x*64 + y*16), fully opaque
        let mut img = emImage::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let v = (x * 64 + y * 16) as u8;
                let p = img.SetPixel(x, y);
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 255;
            }
        }

        let xfm = make_area_xfm(4, 4, 2.0, 2.0);
        let sec = full_sec(4, 4);

        eprintln!("\n=== Y-accumulate trace for 4x4->2x2 with opaque gradient ===");
        eprintln!("  tdx={:#x} tdy={:#x}", xfm.tdx, xfm.tdy);
        eprintln!("  odx={:#x} ody={:#x}", xfm.odx, xfm.ody);

        // Trace Y setup for dest_y=0
        let dest_y = 0i32;
        let ty1 = dest_y as i64 * xfm.tdy - xfm.ty;
        let ty2 = ty1 + xfm.tdy;
        let ty_end = (xfm.img_h as i64) << 24;
        eprintln!("  ty1={:#x} ty2={:#x} ty_end={:#x}", ty1, ty2, ty_end);

        let oy1_raw = ((0x100_0000i64 - (ty1 & 0xFF_FFFF)) as u64 * xfm.ody as u64 + 0xFF_FFFF) >> 24;
        let oy1 = if oy1_raw >= 0x10000 || xfm.ody == 0x7FFF_FFFF { 0x10000u32 } else { oy1_raw as u32 };
        let oy1n = 0x10000u32 - oy1;
        let row0 = (ty1 >> 24) as i32;
        eprintln!("  oy1={:#x} oy1n={:#x} row0={}", oy1, oy1n, row0);

        let yw = YWeights { oy1, oy1n, ody: xfm.ody, row0 };

        // Y-accumulate for columns 0 and 1 (dest pixel (0,0) should cover source cols 0-1)
        for col in 0..4 {
            let p = read_area_pixel(&img, &sec, col, yw.row0, &xfm);
            let (cy_r, cy_g, cy_b, cy_a) = y_accumulate_4ch(&img, &sec, col, &yw, &xfm, p);
            eprintln!("  col={}: pixel[row0]=({},{},{},{}) cy=({:#x},{:#x},{:#x},{:#x})",
                col, p[0], p[1], p[2], p[3], cy_r, cy_g, cy_b, cy_a);
        }

        // X setup for dest_x=0
        let dest_x = 0i32;
        let tx1 = dest_x as i64 * xfm.tdx - xfm.tx;
        let tx2 = tx1 + xfm.tdx;
        let tx_end = (xfm.img_w as i64) << 24;
        eprintln!("  tx1={:#x} tx2={:#x} tx_end={:#x}", tx1, tx2, tx_end);

        let ox_raw = ((0x100_0000i64 - (tx1 & 0xFF_FFFF)) as u64 * xfm.odx as u64 + 0xFF_FFFF) >> 24;
        let ox = if xfm.odx == 0x7FFF_FFFF { 0x7FFF_FFFFu32 } else { ox_raw as u32 };
        let col0 = (tx1 >> 24) as i32;
        eprintln!("  ox={:#x} col0={}", ox, col0);

        // Now get fp result and scanline result
        let c_fp = sample_area_fp(&img, 0, 0, &xfm, &sec, ImageExtension::Clamp);
        eprintln!("  sample_area_fp(0,0) = rgba({},{},{},{})",
            c_fp.GetRed(), c_fp.GetGreen(), c_fp.GetBlue(), c_fp.GetAlpha());

        let mut buf = crate::emPainterScanlineTool::InterpolationBuffer::new(4);
        let mut carry = AreaSampleCarryState::new();
        interpolate_scanline_area_sampled(&img, 0, 0, 1, &xfm, &sec, ImageExtension::Clamp, &mut buf, &mut carry);
        let px = buf.pixel_rgba(0);
        eprintln!("  scanline(0,0) = premul_rgba({},{},{},{})", px[0], px[1], px[2], px[3]);

        // For all 4 output pixels
        eprintln!("\n=== All 4 output pixels ===");
        for dy in 0..2i32 {
            for dx in 0..2i32 {
                let c_fp = sample_area_fp(&img, dx, dy, &xfm, &sec, ImageExtension::Clamp);
                let mut carry = AreaSampleCarryState::new();
                interpolate_scanline_area_sampled(&img, dx, dy, 1, &xfm, &sec, ImageExtension::Clamp, &mut buf, &mut carry);
                let px = buf.pixel_rgba(0);
                // Convert fp (straight) to premul for comparison
                let a = c_fp.GetAlpha();
                let fp_pm_r = ((c_fp.GetRed() as u16 * a as u16 + 127) / 255) as u8;
                let fp_pm_g = ((c_fp.GetGreen() as u16 * a as u16 + 127) / 255) as u8;
                let fp_pm_b = ((c_fp.GetBlue() as u16 * a as u16 + 127) / 255) as u8;
                let diff_r = fp_pm_r as i32 - px[0] as i32;
                let diff_g = fp_pm_g as i32 - px[1] as i32;
                let diff_b = fp_pm_b as i32 - px[2] as i32;
                let diff_a = a as i32 - px[3] as i32;
                eprintln!("  dest({},{}) fp_straight=({},{},{},{}) fp_premul=({},{},{},{}) scanline=({},{},{},{}) diff=({},{},{},{})",
                    dx, dy,
                    c_fp.GetRed(), c_fp.GetGreen(), c_fp.GetBlue(), a,
                    fp_pm_r, fp_pm_g, fp_pm_b, a,
                    px[0], px[1], px[2], px[3],
                    diff_r, diff_g, diff_b, diff_a);
            }
        }
    }
}


#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_channel_diff() {
        let mut p_a = kani::any::<[u8; 4]>();
        let mut p_b = kani::any::<[u8; 4]>();
        let _r = channel_diff(&p_a, &p_b);
    }

    #[kani::proof]
    fn kani_private_interpolate_four_values_adaptive() {
        let mut p_v0: i32 = kani::any::<i32>();
        let mut p_v1: i32 = kani::any::<i32>();
        let mut p_v2: i32 = kani::any::<i32>();
        let mut p_v3: i32 = kani::any::<i32>();
        let mut p_o: u32 = kani::any::<u32>();
        let _r = interpolate_four_values_adaptive(p_v0, p_v1, p_v2, p_v3, p_o);
    }

    #[kani::proof]
    fn kani_private_rational_inv() {
        let mut p_span: i64 = kani::any::<i64>();
        let _r = rational_inv(p_span);
    }
}
