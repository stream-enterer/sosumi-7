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

/// # Safety
/// `dest` and `src` must point to valid RGBA buffers of at least `count * 4` bytes.
#[no_mangle]
pub unsafe extern "C" fn rust_blend_source_over_simple(
    dest: *mut u8,
    src: *const u8,
    count: i32,
    opacity: i32,
) {
    let count = count as usize;
    for i in 0..count {
        let s_off = i * 4;
        let d_off = i * 4;
        let mut sr = *src.add(s_off) as i32;
        let mut sg = *src.add(s_off + 1) as i32;
        let mut sb = *src.add(s_off + 2) as i32;
        let mut sa = *src.add(s_off + 3) as i32;

        // Apply opacity (Fixed12) — matches C++ PaintScanlineInt opacity scaling
        if opacity < 0x1000 {
            if opacity <= 0 { continue; }
            sr = (sr * opacity + 0x800) >> 12;
            sg = (sg * opacity + 0x800) >> 12;
            sb = (sb * opacity + 0x800) >> 12;
            sa = (sa * opacity + 0x800) >> 12;
        }

        if sa == 0 { continue; }
        if sa >= 255 {
            *dest.add(d_off) = sr as u8;
            *dest.add(d_off + 1) = sg as u8;
            *dest.add(d_off + 2) = sb as u8;
            *dest.add(d_off + 3) = 255;
            continue;
        }
        // Source-over: dest = dest * (1-alpha)/255 + src_premul
        // Matches blend_scanline_premul_source_over in emPainterScanlineTool.rs
        // C++ does: *p = (blended_v) + pix, where pix = hR[r]+hG[g]+hB[b]
        // and hR[v] = v (identity for range=255), so pix = r + (g<<8) + (b<<16)
        // This is equivalent to adding the premul channels directly.
        let a = sa as u32;
        let t = (255 - a) * 257;
        *dest.add(d_off) = ((((*dest.add(d_off) as u32) * t + 0x8073) >> 16)
            + sr as u32) as u8;
        *dest.add(d_off + 1) = ((((*dest.add(d_off + 1) as u32) * t + 0x8073) >> 16)
            + sg as u32) as u8;
        *dest.add(d_off + 2) = ((((*dest.add(d_off + 2) as u32) * t + 0x8073) >> 16)
            + sb as u32) as u8;
        // C++ does NOT write alpha (bits 24-31 are zeroed by the packed pixel math).
        // Match that: don't update dest alpha.
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
///
/// # Safety
/// All pointers must be valid. `img_data`: `img_w * img_h * 4` bytes.
/// `out_buf`: `count * 4` bytes. `xfm`: valid `CAreaSampleTransform`.
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

// ── Layer 6: Colored blend (IMAGE_COLORED / font glyph pipeline) ─

use emcore::emPainterScanlineTool::{blend_colored_scanline, BlendMode};
use emcore::emColor::emColor;
use emcore::emPainterInterpolation::sample_adaptive_lum_section;

/// C-compatible color struct (RGBA u8).
#[repr(C)]
pub struct CColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Run Rust blend_colored_scanline (G2 variant: color1=transparent, color2=given).
/// `lums`: array of luminance values (count elements).
/// `dest`: RGBA output buffer (count*4 bytes), pre-filled with background.
/// `coverage`: Fixed12 coverage per pixel (count elements), or null for full coverage.
///
/// # Safety
/// `dest`: `count * 4` bytes. `lums`: `count` bytes. `coverage`: `count` i32s or null.
#[no_mangle]
pub unsafe extern "C" fn rust_blend_colored_g2(
    dest: *mut u8,
    lums: *const u8,
    count: i32,
    coverage: *const i32,
    color2_r: u8,
    color2_g: u8,
    color2_b: u8,
    color2_a: u8,
    canvas_r: u8,
    canvas_g: u8,
    canvas_b: u8,
    canvas_a: u8,
) {
    let count_u = count as usize;
    let lum_slice = std::slice::from_raw_parts(lums, count_u);
    let dest_slice = std::slice::from_raw_parts_mut(dest, count_u * 4);

    let cov_slice: Option<&[i32]> = if coverage.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(coverage, count_u))
    };

    let color1 = emColor::TRANSPARENT;
    let color2 = emColor::rgba(color2_r, color2_g, color2_b, color2_a);
    let canvas = emColor::rgba(canvas_r, canvas_g, canvas_b, canvas_a);

    let mode = if canvas.IsOpaque() {
        BlendMode::CanvasBlend {
            canvas,
            painter_alpha: 255,
        }
    } else {
        BlendMode::SourceOver {
            painter_alpha: 255,
        }
    };

    blend_colored_scanline(
        dest_slice,
        lum_slice,
        count_u,
        cov_slice,
        color1,
        color2,
        &mode,
    );
}

// ── Layer 7: Adaptive luminance interpolation (font glyph upscaling) ─

/// Sample a single pixel from a 1-channel image using adaptive (bicubic)
/// interpolation, matching C++ InterpolateImageAdaptive for CHANNELS=1.
///
/// `img_data`: raw 1-channel pixel data (img_w * img_h bytes)
/// `ix`, `iy`: integer source coords (relative to section origin)
/// `ox`, `oy`: sub-pixel offsets (0-255 range, 8-bit fixed point)
/// `sec_ox`, `sec_oy`, `sec_w`, `sec_h`: section bounds in image space
///
/// Returns interpolated luminance (0-255).
///
/// # Safety
/// `img_data` must point to `img_w * img_h` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn rust_sample_adaptive_lum(
    img_data: *const u8,
    img_w: i32,
    img_h: i32,
    ix: i32,
    iy: i32,
    ox: u32,
    oy: u32,
    sec_ox: i32,
    sec_oy: i32,
    sec_w: i32,
    sec_h: i32,
) -> u8 {
    let img_size = (img_w * img_h) as usize;
    let img_slice = std::slice::from_raw_parts(img_data, img_size);
    // 1-channel image
    let mut image = emImage::new(img_w as u32, img_h as u32, 1);
    let map = image.GetWritableMap();
    map[..img_size].copy_from_slice(img_slice);

    let sec = emcore::emPainterInterpolation::SectionBounds {
        ox: sec_ox,
        oy: sec_oy,
        w: sec_w,
        h: sec_h,
    };

    sample_adaptive_lum_section(
        &image,
        ix,
        iy,
        ox,
        oy,
        &sec,
        ImageExtension::Zero,
    )
}

// ── Layer 8: Border image boundary computation ──────────────────

use emcore::emPainter::emPainter;

#[repr(C)]
pub struct CBorderImageParams {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
    pub img_w: i32,
    pub img_h: i32,
    pub src_l: i32,
    pub src_t: i32,
    pub src_r: i32,
    pub src_b: i32,
    pub scale_x: f64,
    pub scale_y: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub canvas_r: u8,
    pub canvas_g: u8,
    pub canvas_b: u8,
    pub canvas_a: u8,
}

#[repr(C)]
pub struct CBorderImageBoundaries {
    pub adj_l: f64,
    pub adj_t: f64,
    pub adj_r: f64,
    pub adj_b: f64,
    pub target_rects: [[f64; 4]; 9],
    pub source_rects: [[i32; 4]; 9],
}

/// Compute border image slice boundaries without rendering.
/// Returns 0 on success, -1 if compute_border_image_slices returns None.
///
/// # Safety
/// `params` and `out` must point to valid, aligned structs.
#[no_mangle]
pub unsafe extern "C" fn rust_border_image_boundaries(
    params: *const CBorderImageParams,
    out: *mut CBorderImageBoundaries,
) -> i32 {
    let p = &*params;

    // Create a dummy 1x1 RGBA image as painter target.
    let mut target = emImage::new(1, 1, 4);
    let mut painter = emPainter::new(&mut target);

    // Set scale and offset to match the caller's transform.
    painter.scale(p.scale_x, p.scale_y);
    painter.set_offset(p.origin_x, p.origin_y);

    let canvas = emColor::rgba(p.canvas_r, p.canvas_g, p.canvas_b, p.canvas_a);

    let slices = painter.compute_border_image_slices(
        p.x, p.y, p.w, p.h,
        p.l, p.t, p.r, p.b,
        0, 0, p.img_w, p.img_h,
        p.src_l, p.src_t, p.src_r, p.src_b,
        canvas,
    );

    let Some(slices) = slices else {
        return -1;
    };

    let o = &mut *out;
    o.adj_l = slices.adj_l;
    o.adj_t = slices.adj_t;
    o.adj_r = slices.adj_r;
    o.adj_b = slices.adj_b;

    for i in 0..9 {
        let (rx, ry, rw, rh) = slices.target_rects[i];
        o.target_rects[i] = [rx, ry, rw, rh];
        let (sx, sy, sw, sh) = slices.source_rects[i];
        o.source_rects[i] = [sx, sy, sw, sh];
    }

    0
}

// ── Layer 9: Full border image paint pipeline ───────────────────

/// Paint a border image using the Rust pipeline.
///
/// `src_data`: source image RGBA pixels (src_w * src_h * 4 bytes).
/// `fb_data`: target framebuffer RGBA pixels (fb_w * fb_h * 4 bytes), modified in place.
///
/// Returns 0 on success.
///
/// # Safety
/// `src_data`: `src_w * src_h * 4` bytes. `fb_data`: `fb_w * fb_h * 4` bytes.
#[no_mangle]
pub unsafe extern "C" fn rust_paint_border_image(
    src_data: *const u8,
    src_w: i32,
    src_h: i32,
    fb_data: *mut u8,
    fb_w: i32,
    fb_h: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    l: f64,
    t: f64,
    r: f64,
    b: f64,
    src_l: i32,
    src_t: i32,
    src_r: i32,
    src_b: i32,
    scale_x: f64,
    scale_y: f64,
    origin_x: f64,
    origin_y: f64,
    alpha: u8,
    canvas_r: u8,
    canvas_g: u8,
    canvas_b: u8,
    canvas_a: u8,
    which_sub_rects: i32,
) -> i32 {
    let src_size = (src_w * src_h * 4) as usize;
    let src_slice = std::slice::from_raw_parts(src_data, src_size);
    let mut source = emImage::new(src_w as u32, src_h as u32, 4);
    source.GetWritableMap()[..src_size].copy_from_slice(src_slice);

    let fb_size = (fb_w * fb_h * 4) as usize;
    let fb_slice = std::slice::from_raw_parts(fb_data, fb_size);
    let mut target = emImage::new(fb_w as u32, fb_h as u32, 4);
    target.GetWritableMap()[..fb_size].copy_from_slice(fb_slice);

    let mut painter = emPainter::new(&mut target);
    painter.scale(scale_x, scale_y);
    painter.set_offset(origin_x, origin_y);

    let canvas = emColor::rgba(canvas_r, canvas_g, canvas_b, canvas_a);

    painter.PaintBorderImage(
        x, y, w, h,
        l, t, r, b,
        &source,
        src_l, src_t, src_r, src_b,
        alpha,
        canvas,
        which_sub_rects as u16,
    );

    // Copy result back to caller's framebuffer.
    let result = target.GetMap();
    let out_slice = std::slice::from_raw_parts_mut(fb_data, fb_size);
    out_slice.copy_from_slice(&result[..fb_size]);

    0
}

// ── Layer 11: Polygon rasterization ─────────────────────────────

use emcore::emPainterScanline::{rasterize, ClipBounds, WindingRule};

#[repr(C)]
pub struct CPolygonVertex {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
pub struct CSpan {
    pub x_start: i32,
    pub x_end: i32,
    pub opacity_beg: i32,
    pub opacity_mid: i32,
    pub opacity_end: i32,
}

#[repr(C)]
pub struct CScanlineSpans {
    pub y: i32,
    pub span_count: i32,
    pub spans: [CSpan; 64],
}

/// Rasterize a polygon into per-scanline spans with AA coverage.
///
/// `vertices`: array of `n_vertices` polygon vertices in pixel-space f64.
/// `winding_rule`: 0 = NonZero, 1 = EvenOdd.
/// `out_scanlines`: caller-allocated array of `max_scanlines` `CScanlineSpans`.
/// `out_scanline_count`: receives the actual number of scanlines written.
///
/// Returns 0 on success, -1 if output buffer too small (partial results written).
///
/// # Safety
/// `vertices` must point to `n_vertices` valid `CPolygonVertex` structs.
/// `out_scanlines` must point to `max_scanlines` valid `CScanlineSpans` structs.
/// `out_scanline_count` must point to a valid i32.
#[no_mangle]
pub unsafe extern "C" fn rust_rasterize_polygon(
    vertices: *const CPolygonVertex,
    n_vertices: i32,
    clip_x1: f64,
    clip_y1: f64,
    clip_x2: f64,
    clip_y2: f64,
    winding_rule: i32,
    out_scanlines: *mut CScanlineSpans,
    max_scanlines: i32,
    out_scanline_count: *mut i32,
) -> i32 {
    let n = n_vertices as usize;
    let verts: Vec<(f64, f64)> = std::slice::from_raw_parts(vertices, n)
        .iter()
        .map(|v| (v.x, v.y))
        .collect();

    let clip = ClipBounds {
        x1: clip_x1,
        y1: clip_y1,
        x2: clip_x2,
        y2: clip_y2,
    };

    let wr = if winding_rule == 1 {
        WindingRule::EvenOdd
    } else {
        WindingRule::NonZero
    };

    let scanlines = rasterize(&verts, clip, wr);

    let max = max_scanlines as usize;
    let count = scanlines.len().min(max);

    for (i, (y, spans)) in scanlines.iter().enumerate().take(count) {
        let out = &mut *out_scanlines.add(i);
        out.y = *y;
        let span_count = spans.len().min(64);
        out.span_count = span_count as i32;
        for (j, span) in spans.iter().enumerate().take(span_count) {
            out.spans[j] = CSpan {
                x_start: span.x_start,
                x_end: span.x_end,
                opacity_beg: span.opacity_beg,
                opacity_mid: span.opacity_mid,
                opacity_end: span.opacity_end,
            };
        }
    }

    *out_scanline_count = count as i32;

    if scanlines.len() > max { -1 } else { 0 }
}

// ── Layer 10: Linear gradient interpolation ─────────────────────

#[repr(C)]
pub struct CGradientParams {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// Fill a buffer with linear gradient red-channel values (0-255).
///
/// Uses `sample_linear_gradient` with black→white colors so the red channel
/// equals the gradient parameter value at each pixel center.
///
/// # Safety
/// `params` must point to a valid `CGradientParams`.
/// `out_buffer` must be at least `width` bytes.
#[no_mangle]
pub unsafe extern "C" fn rust_interpolate_linear_gradient(
    params: *const CGradientParams,
    scanline_x: i32,
    scanline_y: i32,
    width: i32,
    out_buffer: *mut u8,
) -> i32 {
    let p = &*params;
    let buf = std::slice::from_raw_parts_mut(out_buffer, width as usize);

    let start = (p.x1, p.y1);
    let end = (p.x2, p.y2);

    // Use the C++ 40-bit fixed-point walk directly.
    let grad = emcore::emPainterInterpolation::LinearGradientParams::new(start, end);
    grad.interpolate_scanline(scanline_x, scanline_y, buf);
    0
}

// ── Layer 12: Full paint_image_rect pipeline ────────────────────

/// Paint an image sub-rect using the full Rust PaintImageSrcRect pipeline.
///
/// `img_data`: source image pixels (`img_w * img_h * img_ch` bytes).
/// `canvas`: target framebuffer RGBA pixels (`canvas_w * canvas_h * 4` bytes), modified in place.
/// `extension`: 0=TILED, 1=EDGE, 2=ZERO, 3=EDGE_OR_ZERO.
///
/// Returns 0 on success.
///
/// # Safety
/// `img_data` must point to `img_w * img_h * img_ch` readable bytes.
/// `canvas` must point to `canvas_w * canvas_h * 4` read/write bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rust_paint_image_rect(
    canvas: *mut u8,
    canvas_w: i32,
    canvas_h: i32,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
    img_data: *const u8,
    img_w: i32,
    img_h: i32,
    img_ch: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    src_x: i32,
    src_y: i32,
    src_w: i32,
    src_h: i32,
    alpha: i32,
    canvas_color: u32,
    extension: i32,
) -> i32 {
    let fb_size = (canvas_w * canvas_h * 4) as usize;
    let fb_slice = std::slice::from_raw_parts(canvas, fb_size);
    let mut target = emImage::new(canvas_w as u32, canvas_h as u32, 4);
    target.GetWritableMap()[..fb_size].copy_from_slice(fb_slice);

    let img_size = (img_w * img_h * img_ch) as usize;
    let img_slice = std::slice::from_raw_parts(img_data, img_size);
    let mut source = emImage::new(img_w as u32, img_h as u32, img_ch as u8);
    source.GetWritableMap()[..img_size].copy_from_slice(img_slice);

    let mut painter = emPainter::new(&mut target);
    painter.SetOrigin(offset_x, offset_y);
    painter.SetScaling(scale_x, scale_y);

    let cc = emColor::from_packed(canvas_color);

    let ext = match extension {
        0 => ImageExtension::Repeat,
        1 => ImageExtension::Clamp,
        2 => ImageExtension::Zero,
        _ => ImageExtension::EdgeOrZero,
    };

    painter.PaintImageSrcRect(
        x, y, w, h, &source,
        src_x, src_y, src_w, src_h,
        alpha as u8, cc, ext,
    );

    // Copy result back to caller's framebuffer.
    let result = target.GetMap();
    let out_slice = std::slice::from_raw_parts_mut(canvas, fb_size);
    out_slice.copy_from_slice(&result[..fb_size]);

    0
}

// ── Layer 13: PaintImageColored (two-color luminance mapping) ──

/// Paint an image colored (two-color luminance mapping) using the full Rust pipeline.
///
/// `img_data`: source image pixels (`img_w * img_h * img_ch` bytes).
/// `canvas`: target framebuffer RGBA pixels (`canvas_w * canvas_h * 4` bytes), modified in place.
/// `color1`, `color2`: the two mapping colors as packed u32 RGBA.
/// `canvas_color`: packed u32 RGBA (0 = no canvas color).
/// `extension`: 0=TILED, 1=EDGE, 2=ZERO, 3=EDGE_OR_ZERO.
///
/// Returns 0 on success.
///
/// # Safety
/// `img_data` must point to `img_w * img_h * img_ch` readable bytes.
/// `canvas` must point to `canvas_w * canvas_h * 4` read/write bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rust_paint_image_colored(
    canvas: *mut u8,
    canvas_w: i32,
    canvas_h: i32,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
    img_data: *const u8,
    img_w: i32,
    img_h: i32,
    img_ch: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    src_x: i32,
    src_y: i32,
    src_w: i32,
    src_h: i32,
    color1: u32,
    color2: u32,
    canvas_color: u32,
    extension: i32,
) -> i32 {
    let fb_size = (canvas_w * canvas_h * 4) as usize;
    let fb_slice = std::slice::from_raw_parts(canvas, fb_size);
    let mut target = emImage::new(canvas_w as u32, canvas_h as u32, 4);
    target.GetWritableMap()[..fb_size].copy_from_slice(fb_slice);

    let img_size = (img_w * img_h * img_ch) as usize;
    let img_slice = std::slice::from_raw_parts(img_data, img_size);
    let mut source = emImage::new(img_w as u32, img_h as u32, img_ch as u8);
    source.GetWritableMap()[..img_size].copy_from_slice(img_slice);

    let mut painter = emPainter::new(&mut target);
    painter.SetOrigin(offset_x, offset_y);
    painter.SetScaling(scale_x, scale_y);

    let c1 = emColor::from_packed(color1);
    let c2 = emColor::from_packed(color2);
    let cc = emColor::from_packed(canvas_color);

    let ext = match extension {
        0 => ImageExtension::Repeat,
        1 => ImageExtension::Clamp,
        2 => ImageExtension::Zero,
        _ => ImageExtension::EdgeOrZero,
    };

    painter.PaintImageColored(
        x, y, w, h, &source,
        src_x as u32, src_y as u32, src_w as u32, src_h as u32,
        c1, c2, cc, ext,
    );

    // Copy result back to caller's framebuffer.
    let result = target.GetMap();
    let out_slice = std::slice::from_raw_parts_mut(canvas, fb_size);
    out_slice.copy_from_slice(&result[..fb_size]);

    0
}

// ── Layer 14: PaintText (text rendering via colored glyph atlas) ──

/// Paint text using the full Rust PaintText pipeline.
///
/// `canvas`: target framebuffer RGBA pixels, modified in place.
/// `text`: null-terminated UTF-8 string.
/// `color`: packed u32 RGBA text color.
/// `canvas_color`: packed u32 RGBA (0 = no canvas color).
///
/// Returns 0 on success.
///
/// # Safety
/// `canvas` must point to `canvas_w * canvas_h * 4` read/write bytes.
/// `text` must be a valid null-terminated UTF-8 string.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rust_paint_text(
    canvas: *mut u8,
    canvas_w: i32,
    canvas_h: i32,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
    text: *const std::ffi::c_char,
    x: f64,
    y: f64,
    char_height: f64,
    width_scale: f64,
    color: u32,
    canvas_color: u32,
) -> i32 {
    let fb_size = (canvas_w * canvas_h * 4) as usize;
    let fb_slice = std::slice::from_raw_parts(canvas, fb_size);
    let mut target = emImage::new(canvas_w as u32, canvas_h as u32, 4);
    target.GetWritableMap()[..fb_size].copy_from_slice(fb_slice);

    let mut painter = emPainter::new(&mut target);
    painter.SetOrigin(offset_x, offset_y);
    painter.SetScaling(scale_x, scale_y);

    let c = emColor::from_packed(color);
    let cc = emColor::from_packed(canvas_color);

    let text_str = std::ffi::CStr::from_ptr(text).to_str().unwrap_or("");

    painter.PaintText(x, y, text_str, char_height, width_scale, c, cc);

    let result = target.GetMap();
    let out_slice = std::slice::from_raw_parts_mut(canvas, fb_size);
    out_slice.copy_from_slice(&result[..fb_size]);

    0
}

// ── Layer 16: PaintRoundRect (AA polygon fill) ──

/// Paint a round rect using the Rust PaintRoundRect pipeline.
///
/// # Safety
/// `canvas` must point to `canvas_w * canvas_h * 4` read/write bytes.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rust_paint_round_rect(
    canvas: *mut u8,
    canvas_w: i32,
    canvas_h: i32,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius: f64,
    color: u32,
) -> i32 {
    let fb_size = (canvas_w * canvas_h * 4) as usize;
    let fb_slice = std::slice::from_raw_parts(canvas, fb_size);
    let mut target = emImage::new(canvas_w as u32, canvas_h as u32, 4);
    target.GetWritableMap()[..fb_size].copy_from_slice(fb_slice);

    let mut painter = emPainter::new(&mut target);
    painter.SetOrigin(offset_x, offset_y);
    painter.SetScaling(scale_x, scale_y);

    let c = emColor::from_packed(color);
    painter.PaintRoundRect(x, y, w, h, radius, c, emColor::TRANSPARENT);

    let result = target.GetMap();
    let out_slice = std::slice::from_raw_parts_mut(canvas, fb_size);
    out_slice.copy_from_slice(&result[..fb_size]);

    0
}

// ── Layer 17: PaintPolyline (solid polyline with stroke) ───────

use emcore::emStroke::{emStroke, LineJoin, LineCap};

/// Paint a solid polyline using the full Rust pipeline.
///
/// `canvas`: RGBA framebuffer (canvas_w * canvas_h * 4 bytes).
/// `vertices`: flat array of x,y pairs (n_vertices * 2 doubles).
/// `stroke_color`: packed RGBA u32.
/// `rounded`: whether stroke is rounded (join+cap).
/// `canvas_color`: packed RGBA u32 (0 = transparent/source-over).
///
/// # Safety
/// `canvas` must point to `canvas_w * canvas_h * 4` read/write bytes.
/// `vertices` must point to `n_vertices * 2` doubles.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn rust_paint_polyline(
    canvas: *mut u8,
    canvas_w: i32,
    canvas_h: i32,
    scale_x: f64,
    scale_y: f64,
    offset_x: f64,
    offset_y: f64,
    vertices: *const f64,
    n_vertices: i32,
    thickness: f64,
    stroke_color: u32,
    rounded: i32,
    canvas_color: u32,
) -> i32 {
    let fb_size = (canvas_w * canvas_h * 4) as usize;
    let fb_slice = std::slice::from_raw_parts(canvas, fb_size);
    let mut target = emImage::new(canvas_w as u32, canvas_h as u32, 4);
    target.GetWritableMap()[..fb_size].copy_from_slice(fb_slice);

    let mut painter = emPainter::new(&mut target);
    painter.SetOrigin(offset_x, offset_y);
    painter.SetScaling(scale_x, scale_y);

    let n = n_vertices as usize;
    let xy_slice = std::slice::from_raw_parts(vertices, n * 2);
    let verts: Vec<(f64, f64)> = (0..n).map(|i| (xy_slice[i * 2], xy_slice[i * 2 + 1])).collect();

    let cc = emColor::from_packed(canvas_color);
    let sc = emColor::from_packed(stroke_color);
    let mut stroke = emStroke::new(sc, thickness);
    if rounded != 0 {
        stroke.join = LineJoin::Round;
        stroke.cap = LineCap::Round;
    }

    painter.PaintSolidPolyline(&verts, &stroke, false, cc);

    let result = target.GetMap();
    let out_slice = std::slice::from_raw_parts_mut(canvas, fb_size);
    out_slice.copy_from_slice(&result[..fb_size]);

    0
}

