//! Rust replacement functions for C++ emPainter, exported as C ABI.
//! Built as cdylib, linked into the C++ golden generator.

use std::sync::LazyLock;

// ── Layer 1: Blend hash table ────────────────────────────────────

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
            hash[(255 - a1 as usize) << 8 | (255 - a2 as usize)] =
                (range + c3 - c1 - c2) as u8;
        }
    }
    hash
});

#[inline(always)]
fn blend_hash(color: u8, alpha: u8) -> u8 {
    BLEND_HASH[(color as usize) << 8 | alpha as usize]
}

#[no_mangle]
pub extern "C" fn rust_blend_hash_lookup(color: u8, alpha: u8) -> u8 {
    blend_hash(color, alpha)
}

// ── Layer 2: Source-over compositing ─────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn rust_blend_source_over_simple(
    dest: *mut u8,
    src: *const u8,
    count: i32,
) {
    let count = count as usize;
    for i in 0..count {
        let s_off = i * 4;
        let d_off = i * 4;
        let sr = *src.add(s_off);
        let sg = *src.add(s_off + 1);
        let sb = *src.add(s_off + 2);
        let sa = *src.add(s_off + 3);
        if sa == 0 { continue; }
        if sa >= 255 {
            *dest.add(d_off) = sr;
            *dest.add(d_off + 1) = sg;
            *dest.add(d_off + 2) = sb;
            *dest.add(d_off + 3) = 255;
            continue;
        }
        let alpha = sa;
        let t = (255 - alpha as u32) * 257;
        *dest.add(d_off) = (((*dest.add(d_off) as u32 * t + 0x8073) >> 16)
            + blend_hash(sr, alpha) as u32) as u8;
        *dest.add(d_off + 1) = (((*dest.add(d_off + 1) as u32 * t + 0x8073) >> 16)
            + blend_hash(sg, alpha) as u32) as u8;
        *dest.add(d_off + 2) = (((*dest.add(d_off + 2) as u32 * t + 0x8073) >> 16)
            + blend_hash(sb, alpha) as u32) as u8;
        *dest.add(d_off + 3) = (((*dest.add(d_off + 3) as u32 * t + 0x8073) >> 16)
            + blend_hash(255, alpha) as u32) as u8;
    }
}

// ── Layer 3: Area sampling interpolation ─────────────────────────

use emcore::emPainterInterpolation::{
    AreaSampleTransform, AreaSampleCarryState, SectionBounds,
    interpolate_scanline_area_sampled,
};
use emcore::emPainterScanlineTool::InterpolationBuffer;
use emcore::emTexture::ImageExtension;
use emcore::emImage::emImage;

/// C-compatible struct for passing transform parameters across FFI.
#[repr(C)]
pub struct CAreaSampleTransform {
    pub tdx: i64,
    pub tdy: i64,
    pub tx: i64,
    pub ty: i64,
    pub odx: u32,
    pub ody: u32,
    pub img_w: i32,
    pub img_h: i32,
    pub stride_x: u32,
    pub stride_y: u32,
    pub off_x: i32,
    pub off_y: i32,
}

/// Run Rust area sampling interpolation for `count` pixels starting at (dest_x, dest_y).
/// Writes RGBA output to `out_buf` (must be count*4 bytes).
/// Image data is at `img_data` with dimensions img_w x img_h, 4 channels.
/// Section bounds: source rect within the image.
/// Returns number of pixels written.
#[no_mangle]
pub unsafe extern "C" fn rust_interpolate_area_sampled(
    img_data: *const u8,
    img_w: i32,
    img_h: i32,
    xfm: *const CAreaSampleTransform,
    sec_ox: i32, sec_oy: i32, sec_w: i32, sec_h: i32,
    dest_x: i32,
    dest_y: i32,
    count: i32,
    out_buf: *mut u8,
) -> i32 {
    let xfm_ref = &*xfm;

    // Create an emImage from raw data
    let img_size = (img_w * img_h * 4) as usize;
    let img_slice = std::slice::from_raw_parts(img_data, img_size);
    let mut image = emImage::new(img_w as u32, img_h as u32, 4);
    // Copy pixel data into the image
    let map = image.GetWritableMap();
    map[..img_size].copy_from_slice(img_slice);

    let transform = AreaSampleTransform {
        tdx: xfm_ref.tdx,
        tdy: xfm_ref.tdy,
        tx: xfm_ref.tx,
        ty: xfm_ref.ty,
        odx: xfm_ref.odx,
        ody: xfm_ref.ody,
        img_w: xfm_ref.img_w,
        img_h: xfm_ref.img_h,
        stride_x: xfm_ref.stride_x,
        stride_y: xfm_ref.stride_y,
        off_x: xfm_ref.off_x,
        off_y: xfm_ref.off_y,
    };

    let sec = SectionBounds {
        ox: sec_ox,
        oy: sec_oy,
        w: sec_w,
        h: sec_h,
    };

    let count_u = count as usize;
    let mut ibuf = InterpolationBuffer::new(4);
    let mut carry = AreaSampleCarryState::new();

    interpolate_scanline_area_sampled(
        &image,
        dest_x,
        dest_y,
        count_u,
        &transform,
        &sec,
        ImageExtension::Clamp,
        &mut ibuf,
        &mut carry,
    );

    // Copy output to caller's buffer
    let raw = ibuf.raw_data();
    let out_slice = std::slice::from_raw_parts_mut(out_buf, count_u * 4);
    let copy_len = raw.len().min(out_slice.len());
    out_slice[..copy_len].copy_from_slice(&raw[..copy_len]);

    count
}

// ── Layer 4: Coverage/opacity computation ────────────────────────

/// Compute Rust SubPixelEdges coverage for a pixel, matching the emPainter
/// internal SubPixelEdges::coverage() function.
/// Duplicated here because SubPixelEdges is private to emPainter.rs.
#[no_mangle]
pub extern "C" fn rust_get_coverage(
    dx_px: f64, dy_px: f64, dw_px: f64, dh_px: f64,
    px: i32, py: i32,
) -> i32 {
    // Fixed12 arithmetic matching emPainter.rs
    let fx1 = (dx_px * 4096.0) as i32;
    let fy1 = (dy_px * 4096.0) as i32;
    let fx2 = ((dx_px + dw_px) * 4096.0) as i32;
    let fy2 = ((dy_px + dh_px) * 4096.0) as i32;

    let ix1 = fx1 >> 12;
    let iy1 = fy1 >> 12;
    let ixe_raw = fx2 + 0xFFF;
    let ix2 = ixe_raw >> 12;
    let iy2 = fy2 >> 12;  // C++ truncates, not ceil

    let frac_left = 0x1000i32.saturating_sub(fx1 & 0xFFF);
    let frac_right = (ixe_raw & 0xFFF) + 1;  // C++ ax2 = (ixe & 0xfff) + 1
    let frac_top = 0x1000i32.saturating_sub(fy1 & 0xFFF);
    let frac_bottom = fy2 & 0xFFF;
    let raw_w = (fx2 as i64 - fx1 as i64) as i32;
    let raw_h = (fy2 as i64 - fy1 as i64) as i32;

    // alpha_y
    let alpha_y = if py == iy1 && py == iy2 - 1 {
        (frac_top + frac_bottom).min(0x1000) - 0x1000 + raw_h.min(0x1000)
    } else if py == iy1 {
        frac_top
    } else if py == iy2 - 1 && frac_bottom != 0 {
        frac_bottom
    } else {
        0x1000
    };
    if alpha_y <= 0 { return 0; }

    // alpha_x
    let alpha_x = if px == ix1 && px == ix2 - 1 {
        (frac_left + frac_right).min(0x1000) - 0x1000 + raw_w.min(0x1000)
    } else if px == ix1 {
        frac_left
    } else if px == ix2 - 1 && frac_right != 0 {
        frac_right
    } else {
        0x1000
    };
    if alpha_x <= 0 { return 0; }

    ((alpha_x as i64 * alpha_y as i64 + 0x7ff) >> 12) as i32
}
