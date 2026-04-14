use std::sync::OnceLock;

use crate::emPainterDrawList::{DrawOp, RecordedOp, RecordedState};
use super::emFontCache;
use super::emPainterInterpolation;
use super::emPainterScanline::{self, WindingRule};
use super::emPainterScanlineTool::{blend_colored_scanline, blend_colored_scanline_rgb, blend_scanline, blend_scanline_premul, BlendMode, InterpolationBuffer, MAX_INTERP_BYTES};
use super::emStroke::{emStroke, emStrokeEnd, StrokeEndType};
use super::emTexture::{ImageExtension, ImageQuality, emTexture};
use crate::emColor::{blend_hash_lookup, emColor};
use crate::emImage::emImage;

/// Base multiplier for decoration size.
const ARROW_BASE_SIZE: f64 = 10.0;
/// Notch depth ratio for Arrow type.
const ARROW_NOTCH: f64 = 0.3;
/// Circle quality factor matching C++ emPainter::CircleQuality.
const CIRCLE_QUALITY: f64 = 4.5;
/// Maximum miter extension factor.
const MAX_MITER: f64 = 5.0;
/// Minimum relative segment length for short-segment filtering.
const MIN_REL_SEG_LEN: f64 = 0.001;
/// Maximum number of dashes in PaintDashedPolyline (C++ MaxDashes).
const MAX_DASHES: f64 = 1e5;

/// Size of the C++ radial gradient sqrt lookup table.
/// Table maps squared-distance index to sqrt (0–255).
const GRAD_SQRT_TABLE_SIZE: usize = 64771;

/// Return the C++ emCore radial gradient sqrt lookup table.
/// Entry `i` = `round(sqrt(i))` clamped to 255, matching the
/// run-length-encoded table in `emPainter_ScTlIntGra.cpp`.
fn grad_sqrt_table() -> &'static [u8; GRAD_SQRT_TABLE_SIZE] {
    static TABLE: OnceLock<Box<[u8; GRAD_SQRT_TABLE_SIZE]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = Box::new([0u8; GRAD_SQRT_TABLE_SIZE]);
        for i in 0..GRAD_SQRT_TABLE_SIZE {
            t[i] = ((i as f64).sqrt() + 0.5).floor().min(255.0) as u8;
        }
        t
    })
}
/// Default bitmask for `paint_border_image`: all sub-rects except center.
/// Octal 0757 = binary 0b111_101_111.
///
/// Bit layout:
///   8=UL  5=U   2=UR
///   7=L   4=C   1=R
///   6=LL  3=B   0=LR
pub const BORDER_EDGES_ONLY: u16 = 0o757;

/// Get span opacity at a given pixel position, matching C++ PaintPolygon span layout.
#[inline]
fn span_opacity_at(span: &emPainterScanline::Span, x: i32, x_start: i32, x_end: i32) -> i32 {
    if x == x_start {
        span.opacity_beg
    } else if x == x_end - 1 {
        span.opacity_end
    } else {
        span.opacity_mid
    }
}

/// Pre-transformed texture with coordinates in pixel space.
/// Used internally by the textured polygon rasterizer.
enum PixelTexture<'t> {
    Solid(emColor),
    LinearGradient {
        color_a: emColor,
        color_b: emColor,
        /// C++ ScanlineTool TX: `((i64)((tx1-0.5)*nx + (ty1-0.5)*ny)) - 0x7fffff`
        fp_tx: i64,
        /// C++ ScanlineTool TDX: `(i64)nx` where nx = (tx2-tx1) * f
        fp_tdx: i64,
        /// C++ ScanlineTool TDY: `(i64)ny` where ny = (ty2-ty1) * f
        fp_tdy: i64,
    },
    RadialGradient {
        color_inner: emColor,
        color_outer: emColor,
        /// Fixed-point base: `(center_px - 0.5) * tdx`, cast to i64.
        fp_tx: i64,
        /// Fixed-point base: `(center_py - 0.5) * tdy`, cast to i64.
        fp_ty: i64,
        /// Fixed-point X step: `(255 << 23) / prx`, cast to i64.
        fp_tdx: i64,
        /// Fixed-point Y step: `(255 << 23) / pry`, cast to i64.
        fp_tdy: i64,
    },
    emImage {
        image: &'t emImage,
        /// Additional alpha (0–255). C++ `texture.GetAlpha()`.
        alpha: u8,
        extension: ImageExtension,
        /// C++ ScanlineTool Alpha: raw 8-bit texture alpha.
        sct_alpha: u32,
        /// True if HAVE_ALPHA (texture alpha < 255).
        have_alpha: bool,
        /// C++ ScanlineTool TDX (24fp, post-reduction).
        fp_tdx: i64,
        /// C++ ScanlineTool TDY (24fp, post-reduction).
        fp_tdy: i64,
        /// C++ ScanlineTool TX (24fp, post-reduction).
        fp_tx: i64,
        /// C++ ScanlineTool TY (24fp, post-reduction).
        fp_ty: i64,
        /// True if using area sampling (downscale).
        is_area_sampled: bool,
        /// Post-reduction image width.
        img_w: i32,
        /// Post-reduction image height.
        img_h: i32,
        /// C++ ImgDX: X byte stride (channels * stride_x).
        img_dx: isize,
        /// C++ ImgDY: Y byte stride (full_width * channels * stride_y).
        img_dy: isize,
        /// C++ ImgSX: img_w * img_dx (for tiling wrap).
        img_sx: isize,
        /// C++ ImgSY: img_h * img_dy (for tiling wrap).
        img_sy: isize,
        /// Byte offset into image map for pre-reduction centering.
        img_map_offset: usize,
        // f64 fallback fields for sample_pixel_texture
        inv_scale_x: f64,
        inv_scale_y: f64,
        offset_x: f64,
        offset_y: f64,
    },
    ImageColored {
        image: &'t emImage,
        color: emColor,
        extension: ImageExtension,
        quality: ImageQuality,
        inv_scale_x: f64,
        inv_scale_y: f64,
        offset_x: f64,
        offset_y: f64,
    },
}

/// Text alignment for boxed text rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

/// Vertical alignment for boxed text rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

/// Clip rectangle stored as f64 pixel-space edges, matching C++ emPainter's
/// `double ClipX1, ClipY1, ClipX2, ClipY2`.  Truncation to integer happens
/// only at each paint operation's point of use, avoiding the independent-
/// truncation bug where `floor(x) + floor(w) < floor(x + w)`.
#[derive(Copy, Clone, Debug)]
struct ClipRect {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl ClipRect {
    fn IsEmpty(&self) -> bool {
        self.x1 >= self.x2 || self.y1 >= self.y2
    }

    fn to_scanline_clip(self) -> emPainterScanline::ClipBounds {
        emPainterScanline::ClipBounds {
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
        }
    }
}

/// Coordinate transform state.
#[derive(Clone, Debug)]
struct PainterState {
    /// Translation offset.
    offset_x: f64,
    offset_y: f64,
    /// Scale factor.
    scale_x: f64,
    scale_y: f64,
    /// Clip rectangle in pixel coordinates (f64, matching C++ emPainter).
    clip: ClipRect,
    /// Canvas color for canvas_blend operations.
    canvas_color: emColor,
    /// Global alpha multiplier (0–255).
    alpha: u8,
}

/// Sub-pixel rectangle edges for 12-bit fractional coverage.
/// Matches C++ emPainter PaintRect sub-pixel model (emPainter.cpp:334-397).
/// Sub-pixel edge info for X boundaries. Y boundaries use C++ PaintRect
/// iy2=truncate formula directly (computed inline in each paint function).
struct SubPixelEdges {
    ix1: i32,
    iy1: i32,
    ix2: i32,
    frac_left: i32,
    frac_right: i32,
    raw_w: i32,
}

impl SubPixelEdges {
    /// Compute sub-pixel X edges from pixel-space float coordinates.
    /// Y fields (iy1) are computed for interpolation origin only.
    fn new(dx_px: f64, dy_px: f64, dw_px: f64, _dh_px: f64) -> Self {
        let fx1 = Fixed12::from_f64(dx_px);
        let fy1 = Fixed12::from_f64(dy_px);
        let fx2 = Fixed12::from_f64(dx_px + dw_px);
        Self {
            ix1: fx1.to_i32(),
            iy1: fy1.to_i32(),
            ix2: fx2.ceil().to_i32(),
            frac_left: 0x1000i32.saturating_sub(fx1.frac()),
            frac_right: fx2.frac(),
            raw_w: (fx2.raw() as i64 - fx1.raw() as i64) as i32,
        }
    }

    /// X-axis-only coverage (0..=0x1000). Matches C++ ixe=ceil formula.
    #[inline]
    fn coverage_x(&self, px: i32) -> i32 {
        if px == self.ix1 && px == self.ix2 - 1 {
            (self.frac_left + self.frac_right).min(0x1000) - 0x1000 + self.raw_w.min(0x1000)
        } else if px == self.ix1 {
            self.frac_left
        } else if px == self.ix2 - 1 && self.frac_right != 0 {
            self.frac_right
        } else {
            0x1000
        }
    }

    /// Batch X coverage × C++ PaintRect iy2=truncate Y coverage.
    #[allow(clippy::too_many_arguments)]
    fn batch_coverages_cpp_y(
        &self, row: i32, col_start: i32, out: &mut [i32],
        cpp_iy1: i32, cpp_iy2: i32, cpp_ay1: i32, cpp_ay2: i32,
    ) -> bool {
        let ay = if row == cpp_iy1 && cpp_ay1 < 0x1000 { cpp_ay1 }
                 else if row == cpp_iy2 && cpp_ay2 > 0 { cpp_ay2 }
                 else { 0x1000 };
        let mut all_full = true;
        for (i, cov) in out.iter_mut().enumerate() {
            let ax = self.coverage_x(col_start + i as i32);
            *cov = ((ax as i64 * ay as i64 + 0x7ff) >> 12) as i32;
            if *cov < 0x1000 { all_full = false; }
        }
        all_full
    }
}

/// Paint target: either a real image or a draw list for recording.
pub(crate) enum PaintTarget<'a> {
    /// Direct pixel rendering to an image buffer.
    emImage(&'a mut emImage),
    /// Recording mode: draw operations are captured for parallel replay.
    DrawList(&'a mut Vec<RecordedOp>),
}

/// Zero-sized proof that the painter is in direct (non-recording) mode.
/// Can only be created by `try_record()`, ensuring the recording check happened.
#[derive(Clone, Copy)]
struct DirectProof(());

/// CPU software rasterizer that paints into an emImage buffer.
pub struct emPainter<'a> {
    target: PaintTarget<'a>,
    target_width: u32,
    target_height: u32,
    state: PainterState,
    state_stack: Vec<PainterState>,
    /// Nesting depth for compound ops (matches C++ g_draw_op_depth).
    record_depth: u32,
    /// When true, compound ops execute their body in recording mode so
    /// sub-ops (e.g. PaintText inside PaintTextBoxed) get recorded at
    /// depth+1. Only used for diagnostic dumps (DUMP_DRAW_OPS=1).
    /// Production recording keeps this false for correct replay.
    record_subops: bool,
}

/// The 9 target rectangles computed by PaintBorderImage's boundary logic.
/// Each rect is (x, y, w, h) in logical coordinates, plus corresponding
/// source rect (src_x, src_y, src_w, src_h) in image pixels.
/// Order: UL(0), U(1), UR(2), L(3), C(4), R(5), LL(6), B(7), LR(8).
#[derive(Clone, Debug)]
pub struct BorderImageSlices {
    /// Adjusted insets after RoundX/RoundY pixel-rounding.
    pub adj_l: f64,
    pub adj_t: f64,
    pub adj_r: f64,
    pub adj_b: f64,
    /// Center dimensions after inset adjustment.
    pub dst_cx: f64,
    pub dst_cy: f64,
    /// 9 target rects: (x, y, w, h) in logical coordinates.
    pub target_rects: [(f64, f64, f64, f64); 9],
    /// 9 source rects: (src_x, src_y, src_w, src_h) in image pixels.
    pub source_rects: [(i32, i32, i32, i32); 9],
}

impl<'a> emPainter<'a> {
    /// Create a new painter targeting the given RGBA image.
    ///
    /// # Panics
    /// Panics if the image is not 4-channel RGBA.
    pub fn new(target: &'a mut emImage) -> Self {
        assert_eq!(
            target.GetChannelCount(),
            4,
            "Painter requires a 4-channel RGBA image"
        );
        let w = target.GetWidth();
        let h = target.GetHeight();
        Self {
            target: PaintTarget::emImage(target),
            target_width: w,
            target_height: h,
            state: PainterState {
                offset_x: 0.0,
                offset_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                clip: ClipRect {
                    x1: 0.0,
                    y1: 0.0,
                    x2: w as f64,
                    y2: h as f64,
                },
                canvas_color: emColor::TRANSPARENT,
                alpha: 255,
            },
            state_stack: Vec::new(),
            record_depth: 0,
            record_subops: false,
        }
    }

    /// Create a painter in recording mode for the given viewport dimensions.
    ///
    /// Draw operations are captured into `ops` instead of rasterized.
    /// State management (push/pop, offset, clip) is tracked locally so
    /// that getters like `clip_is_empty()` and `canvas_color()` return
    /// correct values during the recording phase.
    pub fn new_recording(width: u32, height: u32, ops: &'a mut Vec<RecordedOp>) -> Self {
        Self {
            target: PaintTarget::DrawList(ops),
            target_width: width,
            target_height: height,
            state: PainterState {
                offset_x: 0.0,
                offset_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                clip: ClipRect {
                    x1: 0.0,
                    y1: 0.0,
                    x2: width as f64,
                    y2: height as f64,
                },
                canvas_color: emColor::TRANSPARENT,
                alpha: 255,
            },
            state_stack: Vec::new(),
            record_depth: 0,
            record_subops: false,
        }
    }

    /// Enable sub-op recording for diagnostic dumps. When set, compound ops
    /// (PaintTextBoxed, etc.) execute their body in recording mode so sub-calls
    /// get recorded at depth+1. Must NOT be used on painters whose DrawList
    /// will be replayed — sub-ops would double-render.
    pub fn set_record_subops(&mut self, enable: bool) {
        self.record_subops = enable;
    }

    /// Get a mutable reference to the target image.
    /// The `DirectProof` parameter statically guarantees we are not in recording mode.
    fn GetImage(&mut self, _proof: DirectProof) -> &mut emImage {
        match &mut self.target {
            PaintTarget::emImage(img) => img,
            PaintTarget::DrawList(_) => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    /// Get an immutable reference to the target image.
    fn image_ref(&self, _proof: DirectProof) -> &emImage {
        match &self.target {
            PaintTarget::emImage(img) => img,
            PaintTarget::DrawList(_) => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    /// Try to record a draw op. Returns `Some(DirectProof)` if in direct mode,
    /// `None` if the op was recorded (recording mode).
    fn try_record(&mut self, op: DrawOp) -> Option<DirectProof> {
        if let PaintTarget::DrawList(ops) = &mut self.target {
            ops.push(RecordedOp {
                depth: self.record_depth,
                op,
                state: RecordedState {
                    scale_x: self.state.scale_x,
                    scale_y: self.state.scale_y,
                    offset_x: self.state.offset_x,
                    offset_y: self.state.offset_y,
                    clip_x1: self.state.clip.x1,
                    clip_y1: self.state.clip.y1,
                    clip_x2: self.state.clip.x2,
                    clip_y2: self.state.clip.y2,
                    alpha: self.state.alpha,
                },
            });
            None
        } else {
            Some(DirectProof(()))
        }
    }

    /// Return a `DirectProof` if we are in direct mode, `None` otherwise.
    /// Use for methods that access pixels but have no corresponding `DrawOp`
    /// (e.g. `Clear`).
    fn require_direct(&self) -> Option<DirectProof> {
        match &self.target {
            PaintTarget::emImage(_) => Some(DirectProof(())),
            PaintTarget::DrawList(_) => None,
        }
    }

    /// Record a state operation unconditionally (for push/pop/set that also
    /// mutate local state regardless of mode).
    fn record_state(&mut self, op: DrawOp) {
        if let PaintTarget::DrawList(ops) = &mut self.target {
            ops.push(RecordedOp {
                depth: self.record_depth,
                op,
                state: RecordedState {
                    scale_x: self.state.scale_x,
                    scale_y: self.state.scale_y,
                    offset_x: self.state.offset_x,
                    offset_y: self.state.offset_y,
                    clip_x1: self.state.clip.x1,
                    clip_y1: self.state.clip.y1,
                    clip_x2: self.state.clip.x2,
                    clip_y2: self.state.clip.y2,
                    alpha: self.state.alpha,
                },
            });
        }
    }

    /// Read a pixel from the target image, returning an owned copy.
    /// Avoids borrow issues when both reading and writing pixels.
    #[inline]
    fn read_pixel(&self, proof: DirectProof, x: u32, y: u32) -> [u8; 4] {
        let p = self.image_ref(proof).GetPixel(x, y);
        [p[0], p[1], p[2], p[3]]
    }

    /// Push the current state onto the stack.
    pub fn push_state(&mut self) {
        self.record_state(DrawOp::PushState);
        self.state_stack.push(self.state.clone());
    }

    /// Pop and restore the previous state.
    ///
    /// # Panics
    /// Panics if the state stack is empty.
    pub fn pop_state(&mut self) {
        self.record_state(DrawOp::PopState);
        self.state = self.state_stack.pop().expect("State stack underflow");
    }

    /// Get the current canvas color.
    pub fn GetCanvasColor(&self) -> emColor {
        self.state.canvas_color
    }

    /// Set the canvas color used for canvas_blend operations.
    pub fn SetCanvasColor(&mut self, color: emColor) {
        self.record_state(DrawOp::SetCanvasColor(color));
        self.state.canvas_color = color;
    }

    /// Set the global alpha multiplier.
    pub fn SetAlpha(&mut self, alpha: u8) {
        self.record_state(DrawOp::SetAlpha(alpha));
        self.state.alpha = alpha;
    }

    /// Get the current offset (for computing absolute panel transforms).
    pub fn offset(&self) -> (f64, f64) {
        (self.state.offset_x, self.state.offset_y)
    }

    /// Set the offset absolutely (not cumulative).
    pub fn set_offset(&mut self, ox: f64, oy: f64) {
        self.record_state(DrawOp::SetOffset(ox, oy));
        self.state.offset_x = ox;
        self.state.offset_y = oy;
    }

    /// Apply a translation.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.state.offset_x += dx * self.state.scale_x;
        self.state.offset_y += dy * self.state.scale_y;
    }

    /// Apply a scale.
    pub fn scale(&mut self, sx: f64, sy: f64) {
        self.state.scale_x *= sx;
        self.state.scale_y *= sy;
    }

    /// Set the clip rectangle (intersection with current clip).
    /// Computes and stores clip edges in f64, matching C++ emPanel.cpp:1478-1495.
    /// Truncation to i32 happens only at each paint operation's point of use.
    pub fn SetClipping(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.record_state(DrawOp::ClipRect { x, y, w, h });
        let px = x * self.state.scale_x + self.state.offset_x;
        let py = y * self.state.scale_y + self.state.offset_y;
        let px2 = px + w * self.state.scale_x;
        let py2 = py + h * self.state.scale_y;

        let clip = self.state.clip;
        // Intersect in f64 (no intermediate i32 truncation).
        let nx1 = px.max(clip.x1);
        let ny1 = py.max(clip.y1);
        let nx2 = px2.min(clip.x2);
        let ny2 = py2.min(clip.y2);
        if nx1 >= nx2 || ny1 >= ny2 {
            self.state.clip = ClipRect {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            };
        } else {
            self.state.clip = ClipRect {
                x1: nx1,
                y1: ny1,
                x2: nx2,
                y2: ny2,
            };
        }
    }

    /// Returns true if the current clip region has zero area.
    pub fn IsClipEmpty(&self) -> bool {
        self.state.clip.IsEmpty()
    }

    /// Get clipping rectangle X1 in pixel coordinates.
    /// Corresponds to C++ `emPainter::GetClipX1`.
    pub fn GetClipX1(&self) -> f64 {
        self.state.clip.x1
    }

    /// Get clipping rectangle Y1 in pixel coordinates.
    /// Corresponds to C++ `emPainter::GetClipY1`.
    pub fn GetClipY1(&self) -> f64 {
        self.state.clip.y1
    }

    /// Get clipping rectangle X2 in pixel coordinates.
    /// Corresponds to C++ `emPainter::GetClipX2`.
    pub fn GetClipX2(&self) -> f64 {
        self.state.clip.x2
    }

    /// Get clipping rectangle Y2 in pixel coordinates.
    /// Corresponds to C++ `emPainter::GetClipY2`.
    pub fn GetClipY2(&self) -> f64 {
        self.state.clip.y2
    }

    /// Get origin X in pixel coordinates.
    /// Corresponds to C++ `emPainter::GetOriginX`.
    pub fn GetOriginX(&self) -> f64 {
        self.state.offset_x
    }

    /// Get origin Y in pixel coordinates.
    /// Corresponds to C++ `emPainter::GetOriginY`.
    pub fn GetOriginY(&self) -> f64 {
        self.state.offset_y
    }

    /// Get X scale factor.
    /// Corresponds to C++ `emPainter::GetScaleX`.
    pub fn GetScaleX(&self) -> f64 {
        self.state.scale_x
    }

    /// Get Y scale factor.
    /// Corresponds to C++ `emPainter::GetScaleY`.
    pub fn GetScaleY(&self) -> f64 {
        self.state.scale_y
    }

    /// Set clip rectangle directly in pixel coordinates, no intersection.
    /// Matches C++ `emPainter::SetClipping(clipX1, clipY1, clipX2, clipY2)`.
    pub fn SetClippingAbsolute(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        // Record as user-space coords for DrawOp compatibility
        let ux = (x1 - self.state.offset_x) / self.state.scale_x;
        let uy = (y1 - self.state.offset_y) / self.state.scale_y;
        let uw = (x2 - x1) / self.state.scale_x;
        let uh = (y2 - y1) / self.state.scale_y;
        self.record_state(DrawOp::ClipRect { x: ux, y: uy, w: uw, h: uh });
        self.state.clip = ClipRect { x1, y1, x2, y2 };
    }

    /// Fill the entire clip region with a color, respecting canvas color for blending.
    /// Corresponds to C++ `emPainter::Clear(texture, canvasColor)`.
    pub fn ClearWithCanvas(&mut self, color: emColor, canvas_color: emColor) {
        let sx = self.state.scale_x;
        let sy = self.state.scale_y;
        let ox = self.state.offset_x;
        let oy = self.state.offset_y;
        self.PaintRect(
            (self.state.clip.x1 - ox) / sx,
            (self.state.clip.y1 - oy) / sy,
            (self.state.clip.x2 - self.state.clip.x1) / sx,
            (self.state.clip.y2 - self.state.clip.y1) / sy,
            color,
            canvas_color,
        );
    }

    /// Set origin (absolute offset, replaces current offset).
    pub fn SetOrigin(&mut self, x: f64, y: f64) {
        self.state.offset_x = x;
        self.state.offset_y = y;
    }

    /// Set scaling (absolute, replaces current scale).
    pub fn SetScaling(&mut self, sx: f64, sy: f64) {
        self.record_state(DrawOp::SetScaling(sx, sy));
        self.state.scale_x = sx;
        self.state.scale_y = sy;
    }

    /// Set the full coordinate transformation (origin + scale) in one call.
    /// Matches C++ `emPainter::SetTransformation`.
    ///
    /// The transform from user coordinates to pixel coordinates is:
    ///   pixel_x = user_x * sx + ox
    ///   pixel_y = user_y * sy + oy
    pub fn SetTransformation(&mut self, ox: f64, oy: f64, sx: f64, sy: f64) {
        self.record_state(DrawOp::SetTransformation { ox, oy, sx, sy });
        self.state.offset_x = ox;
        self.state.offset_y = oy;
        self.state.scale_x = sx;
        self.state.scale_y = sy;
    }

    /// Get the current origin (offset).
    pub fn origin(&self) -> (f64, f64) {
        (self.state.offset_x, self.state.offset_y)
    }

    /// Get the current scaling.
    pub fn scaling(&self) -> (f64, f64) {
        (self.state.scale_x, self.state.scale_y)
    }

    /// Round x coordinate to nearest pixel.
    pub fn RoundX(&self, x: f64) -> f64 {
        ((x * self.state.scale_x + self.state.offset_x).round() - self.state.offset_x)
            / self.state.scale_x
    }

    /// Round y coordinate to nearest pixel.
    pub fn RoundY(&self, y: f64) -> f64 {
        ((y * self.state.scale_y + self.state.offset_y).round() - self.state.offset_y)
            / self.state.scale_y
    }

    /// Round x coordinate down to pixel boundary.
    pub fn RoundDownX(&self, x: f64) -> f64 {
        ((x * self.state.scale_x + self.state.offset_x).floor() - self.state.offset_x)
            / self.state.scale_x
    }

    /// Round y coordinate down to pixel boundary.
    pub fn RoundDownY(&self, y: f64) -> f64 {
        ((y * self.state.scale_y + self.state.offset_y).floor() - self.state.offset_y)
            / self.state.scale_y
    }

    /// Round x coordinate up to pixel boundary.
    pub fn RoundUpX(&self, x: f64) -> f64 {
        ((x * self.state.scale_x + self.state.offset_x).ceil() - self.state.offset_x)
            / self.state.scale_x
    }

    /// Round y coordinate up to pixel boundary.
    pub fn RoundUpY(&self, y: f64) -> f64 {
        ((y * self.state.scale_y + self.state.offset_y).ceil() - self.state.offset_y)
            / self.state.scale_y
    }

    /// Get the left edge of the clip rectangle in user coordinates.
    pub fn GetUserClipX1(&self) -> f64 {
        (self.state.clip.x1 - self.state.offset_x) / self.state.scale_x
    }

    /// Get the top edge of the clip rectangle in user coordinates.
    pub fn GetUserClipY1(&self) -> f64 {
        (self.state.clip.y1 - self.state.offset_y) / self.state.scale_y
    }

    /// Get the right edge of the clip rectangle in user coordinates.
    pub fn GetUserClipX2(&self) -> f64 {
        (self.state.clip.x2 - self.state.offset_x) / self.state.scale_x
    }

    /// Get the bottom edge of the clip rectangle in user coordinates.
    pub fn GetUserClipY2(&self) -> f64 {
        (self.state.clip.y2 - self.state.offset_y) / self.state.scale_y
    }

    // --- Drawing API ---

    /// Fill a rectangle with a solid color using sub-pixel anti-aliasing.
    /// Uses 12-bit fixed-point for fractional edge coverage matching C++ emPainter.
    pub fn PaintRect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintRect {
            x, y, w, h, color, canvas_color,
        }) else { return; };

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

        // C++ emPainter.cpp:402-411: transform and clip
        let mut px = x * self.state.scale_x + self.state.offset_x;
        let mut px2 = px + w * self.state.scale_x;
        if px < self.state.clip.x1 { px = self.state.clip.x1; }
        if px2 > self.state.clip.x2 { px2 = self.state.clip.x2; }
        if px >= px2 { self.state.canvas_color = saved_canvas; return; }
        let mut py = y * self.state.scale_y + self.state.offset_y;
        let mut py2 = py + h * self.state.scale_y;
        if py < self.state.clip.y1 { py = self.state.clip.y1; }
        if py2 > self.state.clip.y2 { py2 = self.state.clip.y2; }
        if py >= py2 { self.state.canvas_color = saved_canvas; return; }

        // C++ emPainter.cpp:418-440: Fixed12 boundary computation
        let ix_raw = (px * 0x1000 as f64) as i32;
        let ixe_raw = (px2 * 0x1000 as f64) as i32 + 0xfff;
        let mut ax1 = 0x1000 - (ix_raw & 0xfff);
        let ax2 = (ixe_raw & 0xfff) + 1;
        let ix = ix_raw >> 12;
        let ixe = ixe_raw >> 12;
        let iw = ixe - ix;
        if iw <= 0 { self.state.canvas_color = saved_canvas; return; }
        if iw <= 1 { ax1 += ax2 - 0x1000; }

        let iy_raw = (py * 0x1000 as f64) as i32;
        let iy2_raw = (py2 * 0x1000 as f64) as i32;
        let mut ay1 = 0x1000 - (iy_raw & 0xfff);
        let mut ay2 = iy2_raw & 0xfff;
        let mut iy = iy_raw >> 12;
        let iy2 = iy2_raw >> 12;
        if iy >= iy2 {
            ay1 += ay2 - 0x1000;
            ay2 = 0;
            if ay1 <= 0 { self.state.canvas_color = saved_canvas; return; }
        }

        // C++ emPainter.cpp:442-456: scanline loop
        let proof = match self.require_direct() {
            Some(p) => p,
            None => { self.state.canvas_color = saved_canvas; return; }
        };
        if ay1 < 0x1000 {
            self.paint_rect_scanline(proof, ix, iy, iw,
                ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32, ay1,
                ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32, color);
            iy += 1;
        }
        while iy < iy2 {
            self.paint_rect_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, color);
            iy += 1;
        }
        if ay2 > 0 {
            self.paint_rect_scanline(proof, ix, iy, iw,
                ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32, ay2,
                ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32, color);
        }

        self.state.canvas_color = saved_canvas;
    }

    /// Fill a rect with a gradient texture using PaintRect sub-pixel boundary
    /// computation. Matches C++ `PaintRect(x,y,w,h, emGradientTexture, canvas)`.
    ///
    /// Uses the same Fixed12 boundary math as `PaintRect` (iy2=truncate), but
    /// samples the gradient texture per pixel instead of using a single color.
    /// Records as `PaintRect` in the draw-op stream.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_rect_with_texture(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        texture: &super::emTexture::emTexture,
        canvas_color: emColor,
    ) {
        // Record as PaintRect with the texture's representative color,
        // matching C++ which records PaintRect(texture.GetColor()).
        let repr_color = match texture {
            super::emTexture::emTexture::LinearGradient { color_a, .. } => *color_a,
            super::emTexture::emTexture::RadialGradient { color_inner, .. } => *color_inner,
            super::emTexture::emTexture::SolidColor(c) => *c,
            super::emTexture::emTexture::emImage { .. }
            | super::emTexture::emTexture::ImageColored { .. }
            | super::emTexture::emTexture::ImageColoredGradient { .. } => emColor::TRANSPARENT,
        };
        let Some(proof) = self.try_record(DrawOp::PaintRect {
            x, y, w, h, color: repr_color, canvas_color,
        }) else { return; };

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

        // --- PaintRect boundary computation (same as PaintRect solid) ---
        let x2 = x + w;
        let y2 = y + h;
        let px1 = (x * self.state.scale_x + self.state.offset_x)
            .max(self.state.clip.x1).min(self.state.clip.x2);
        let px2 = (x2 * self.state.scale_x + self.state.offset_x)
            .max(self.state.clip.x1).min(self.state.clip.x2);
        let py1 = (y * self.state.scale_y + self.state.offset_y)
            .max(self.state.clip.y1).min(self.state.clip.y2);
        let py2 = (y2 * self.state.scale_y + self.state.offset_y)
            .max(self.state.clip.y1).min(self.state.clip.y2);
        if px1 >= px2 || py1 >= py2 {
            self.state.canvas_color = saved_canvas;
            return;
        }

        let ix_raw = (px1 * 4096.0) as i32;
        let ixe_raw = (px2 * 4096.0) as i32 + 0xfff;
        let mut ax1 = 0x1000 - (ix_raw & 0xfff);
        let ax2 = (ixe_raw & 0xfff) + 1;
        let ix = ix_raw >> 12;
        let ixe = ixe_raw >> 12;
        let iw = ixe - ix;
        if iw <= 0 {
            self.state.canvas_color = saved_canvas;
            return;
        }
        if iw <= 1 { ax1 += ax2 - 0x1000; }

        let iy_raw = (py1 * 4096.0) as i32;
        let iy2_raw = (py2 * 4096.0) as i32;
        let mut ay1 = 0x1000 - (iy_raw & 0xfff);
        let mut ay2 = iy2_raw & 0xfff;
        let mut iy = iy_raw >> 12;
        let iy2 = iy2_raw >> 12;
        if iy >= iy2 {
            ay1 += ay2 - 0x1000;
            ay2 = 0;
            if ay1 <= 0 {
                self.state.canvas_color = saved_canvas;
                return;
            }
        }

        // --- Per-pixel gradient rendering ---
        // Prepare gradient interpolation parameters
        match texture {
            super::emTexture::emTexture::LinearGradient { color_a, color_b, start, end } => {
                let pstart = (
                    start.0 * self.state.scale_x + self.state.offset_x,
                    start.1 * self.state.scale_y + self.state.offset_y,
                );
                let pend = (
                    end.0 * self.state.scale_x + self.state.offset_x,
                    end.1 * self.state.scale_y + self.state.offset_y,
                );
                let grad = emPainterInterpolation::LinearGradientParams::new(pstart, pend);
                let mut gbuf = vec![0u8; iw as usize];
                // Scanline loop matching PaintRect
                if ay1 < 0x1000 {
                    self.paint_rect_gradient_scanline(proof, ix, iy, iw, ax1, ay1, ax2, &grad, &mut gbuf, *color_a, *color_b);
                    iy += 1;
                }
                while iy < iy2 {
                    self.paint_rect_gradient_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, &grad, &mut gbuf, *color_a, *color_b);
                    iy += 1;
                }
                if ay2 > 0 {
                    self.paint_rect_gradient_scanline(proof, ix, iy, iw, ax1, ay2, ax2, &grad, &mut gbuf, *color_a, *color_b);
                }
            }
            super::emTexture::emTexture::RadialGradient { color_inner, color_outer, center, radius_x, radius_y } => {
                // Radial gradient: distance from center, normalized by radii
                let pcx = center.0 * self.state.scale_x + self.state.offset_x;
                let pcy = center.1 * self.state.scale_y + self.state.offset_y;
                let prx = radius_x * self.state.scale_x;
                let pry = radius_y * self.state.scale_y;
                if ay1 < 0x1000 {
                    self.paint_rect_radial_scanline(proof, ix, iy, iw, ax1, ay1, ax2, pcx, pcy, prx, pry, *color_inner, *color_outer);
                    iy += 1;
                }
                while iy < iy2 {
                    self.paint_rect_radial_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, pcx, pcy, prx, pry, *color_inner, *color_outer);
                    iy += 1;
                }
                if ay2 > 0 {
                    self.paint_rect_radial_scanline(proof, ix, iy, iw, ax1, ay2, ax2, pcx, pcy, prx, pry, *color_inner, *color_outer);
                }
            }
            super::emTexture::emTexture::SolidColor(c) => {
                // Fallback to solid color PaintRect
                if ay1 < 0x1000 {
                    let a1 = ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
                    let a2 = ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
                    self.paint_rect_scanline(proof, ix, iy, iw, a1, ay1, a2, *c);
                    iy += 1;
                }
                while iy < iy2 {
                    self.paint_rect_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, *c);
                    iy += 1;
                }
                if ay2 > 0 {
                    let a1 = ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
                    let a2 = ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
                    self.paint_rect_scanline(proof, ix, iy, iw, a1, ay2, a2, *c);
                }
            }
            _ => {} // Image textures not handled here
        }

        self.state.canvas_color = saved_canvas;
    }

    /// Render one PaintRect scanline with linear gradient per-pixel colors.
    #[allow(clippy::too_many_arguments)]
    fn paint_rect_gradient_scanline(
        &mut self,
        proof: DirectProof,
        ix: i32, iy: i32, iw: i32,
        ax1: i32, ay: i32, ax2: i32,
        grad: &emPainterInterpolation::LinearGradientParams,
        gbuf: &mut [u8],
        color_a: emColor,
        color_b: emColor,
    ) {
        // Interpolate gradient values for this scanline
        grad.interpolate_scanline(ix, iy, &mut gbuf[..iw as usize]);

        // First pixel
        let a1 = ((ax1 as i64 * ay as i64 + 0x7ff) >> 12) as i32;
        let c0 = emPainterInterpolation::blend_gradient_colors(gbuf[0], color_a, color_b);
        self.blend_with_coverage(proof, ix, iy, c0, a1);

        // Interior pixels
        for (i, &g) in gbuf.iter().enumerate().take((iw as usize).saturating_sub(1)).skip(1) {
            let c = emPainterInterpolation::blend_gradient_colors(g, color_a, color_b);
            self.blend_with_coverage(proof, ix + i as i32, iy, c, ay);
        }

        // Last pixel (if width > 1)
        if iw > 1 {
            let a2 = ((ax2 as i64 * ay as i64 + 0x7ff) >> 12) as i32;
            let c_last = emPainterInterpolation::blend_gradient_colors(gbuf[(iw - 1) as usize], color_a, color_b);
            self.blend_with_coverage(proof, ix + iw - 1, iy, c_last, a2);
        }
    }

    /// Render one PaintRect scanline with radial gradient per-pixel colors.
    /// Uses the C++ radial gradient interpolation formula.
    #[allow(clippy::too_many_arguments)]
    fn paint_rect_radial_scanline(
        &mut self,
        proof: DirectProof,
        ix: i32, iy: i32, iw: i32,
        ax1: i32, ay: i32, ax2: i32,
        pcx: f64, pcy: f64, prx: f64, pry: f64,
        color_inner: emColor,
        color_outer: emColor,
    ) {
        let a1 = ((ax1 as i64 * ay as i64 + 0x7ff) >> 12) as i32;
        let a2_cov = ((ax2 as i64 * ay as i64 + 0x7ff) >> 12) as i32;

        for i in 0..iw as usize {
            let px = (ix + i as i32) as f64 + 0.5;
            let py = iy as f64 + 0.5;
            let dx = (px - pcx) / prx;
            let dy = (py - pcy) / pry;
            let dist_sq = dx * dx + dy * dy;
            let dist = dist_sq.sqrt();
            let g = (dist * 255.0).clamp(0.0, 255.0) as u8;
            let c = emPainterInterpolation::blend_gradient_colors(g, color_inner, color_outer);

            let cov = if i == 0 {
                a1
            } else if i == (iw - 1) as usize && iw > 1 {
                a2_cov
            } else {
                ay
            };
            self.blend_with_coverage(proof, ix + i as i32, iy, c, cov);
        }
    }

    /// Fill an ellipse with a solid color using AA polygon approximation.
    pub fn PaintEllipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let is_recording = self.try_record(DrawOp::PaintEllipse {
            cx,
            cy,
            rx,
            ry,
            color,
            canvas_color,
        }).is_none();
        if is_recording {
            if !self.record_subops {
                return;
            }
            self.record_depth += 1;
        }
        let verts = self.ellipse_polygon(cx, cy, rx, ry);
        self.PaintPolygon(&verts, color, canvas_color);
        if is_recording { self.record_depth -= 1; }
    }

    /// Fill an ellipse with a gradient texture using AA polygon approximation.
    /// Matches C++ `PaintEllipse(x,y,w,h, emGradientTexture, canvas)`.
    /// Records as PaintEllipse in the draw-op stream.
    ///
    /// C++ PaintEllipse uses (x,y,w,h) bounding rect; this takes (cx,cy,rx,ry).
    #[allow(clippy::too_many_arguments)]
    pub fn paint_ellipse_with_texture(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        texture: &super::emTexture::emTexture,
        canvas_color: emColor,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let repr_color = match texture {
            super::emTexture::emTexture::RadialGradient { color_inner, .. } => *color_inner,
            super::emTexture::emTexture::LinearGradient { color_a, .. } => *color_a,
            super::emTexture::emTexture::SolidColor(c) => *c,
            super::emTexture::emTexture::emImage { .. }
            | super::emTexture::emTexture::ImageColored { .. }
            | super::emTexture::emTexture::ImageColoredGradient { .. } => emColor::TRANSPARENT,
        };
        let is_recording = self.try_record(DrawOp::PaintEllipse {
            cx, cy, rx, ry,
            color: repr_color,
            canvas_color,
        }).is_none();
        if is_recording {
            if !self.record_subops {
                return;
            }
            self.record_depth += 1;
        }
        let verts = self.ellipse_polygon(cx, cy, rx, ry);
        self.paint_polygon_textured(&verts, texture, canvas_color);
        if is_recording { self.record_depth -= 1; }
    }

    /// Fill an ellipse sector (pie slice) defined by center, radii, and angle range.
    /// Angles are in **degrees**, matching C++ emPainter convention.
    /// `start_angle` is the start in degrees from +X axis; `sweep_angle` is the
    /// arc length in degrees from start.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintEllipseSector(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        sweep_angle: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintEllipseSector {
            cx,
            cy,
            rx,
            ry,
            start_angle,
            sweep_angle,
            color,
            canvas_color,
        }) else { return; };
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        if sweep_angle == 0.0 {
            return;
        }
        // Normalize negative sweep.
        if sweep_angle < 0.0 {
            return self.PaintEllipseSector(
                cx,
                cy,
                rx,
                ry,
                start_angle + sweep_angle,
                -sweep_angle,
                color,
                canvas_color,
            );
        }
        // Convert degrees to radians.
        let start_rad = start_angle * std::f64::consts::PI / 180.0;
        let sweep_rad = sweep_angle * std::f64::consts::PI / 180.0;
        // Full circle or more — delegate to paint_ellipse.
        if sweep_rad >= 2.0 * std::f64::consts::PI {
            return self.PaintEllipse(cx, cy, rx, ry, color, canvas_color);
        }
        // Match C++ PaintEllipseSector: keep f as float through arc scaling,
        // use round-to-nearest, minimum 3 arc segments, center vertex last.
        let mut f = CIRCLE_QUALITY * (rx * self.state.scale_x + ry * self.state.scale_y).sqrt();
        if f > 256.0 {
            f = 256.0;
        }
        f = f * sweep_rad / (2.0 * std::f64::consts::PI);
        let arc_segments = if f <= 3.0 {
            3
        } else if f >= 256.0 {
            256
        } else {
            (f + 0.5) as usize
        };
        let step = sweep_rad / arc_segments as f64;
        let mut verts = Vec::with_capacity(arc_segments + 2);
        for i in 0..=arc_segments {
            let angle = start_rad + step * i as f64;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        verts.push((cx, cy));
        self.PaintPolygon(&verts, color, canvas_color);
    }

    /// Fill a rectangle with a linear gradient between two colors.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_linear_gradient(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color_a: emColor,
        color_b: emColor,
        horizontal: bool,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintLinearGradient {
            x,
            y,
            w,
            h,
            color_a,
            color_b,
            horizontal,
            canvas_color,
        }) else { return; };
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = py.max(cy1);
        let end_x = (px + pw).min(cx2);
        let end_y = (py + ph).min(cy2);

        let (start, end) = if horizontal {
            ((px as f64, py as f64), ((px + pw) as f64, py as f64))
        } else {
            ((px as f64, py as f64), (px as f64, (py + ph) as f64))
        };

        // C++ 40-bit fixed-point gradient walk (emPainter_ScTlIntGra.cpp:24-39).
        let grad = emPainterInterpolation::LinearGradientParams::new(start, end);

        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4);
        let max_batch = ibuf.max_pixels();
        let mut grad_buf = vec![0u8; max_batch];

        for row in start_y..end_y {
            let mut col = start_x;
            while col < end_x {
                let batch = ((end_x - col) as usize).min(max_batch);
                grad.interpolate_scanline(col, row, &mut grad_buf[..batch]);
                for (i, &g) in grad_buf[..batch].iter().enumerate() {
                    let color = emPainterInterpolation::blend_gradient_colors(
                        g, color_a, color_b,
                    );
                    ibuf.set_pixel(i, [color.GetRed(), color.GetGreen(), color.GetBlue(), color.GetAlpha()]);
                }
                ibuf.set_len(batch);
                let dest_offset = (row as usize * tw + col as usize) * 4;
                let data = self.GetImage(proof).GetWritableMap();
                let dest = &mut data[dest_offset..];
                blend_scanline(dest, &ibuf, batch, None, &mode);
                col += batch as i32;
            }
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Fill an elliptical region with a radial gradient.
    ///
    /// Uses polygon-approximated ellipse boundary with AA scanline rasterization,
    /// matching C++ emPainter's PaintEllipse + emRadialGradientTexture approach.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_radial_gradient(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        color_inner: emColor,
        color_outer: emColor,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintRadialGradient {
            cx,
            cy,
            rx,
            ry,
            color_inner,
            color_outer,
            canvas_color,
        }) else { return; };
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let verts = self.ellipse_polygon(cx, cy, rx, ry);

        let pixel_verts: Vec<(f64, f64)> = verts
            .iter()
            .map(|&(x, y)| {
                (
                    x * self.state.scale_x + self.state.offset_x,
                    y * self.state.scale_y + self.state.offset_y,
                )
            })
            .collect();

        let rows = emPainterScanline::rasterize(
            &pixel_verts,
            self.state.clip.to_scanline_clip(),
            WindingRule::NonZero,
        );

        let pcx = cx * self.state.scale_x + self.state.offset_x;
        let pcy = cy * self.state.scale_y + self.state.offset_y;
        let prx = (rx * self.state.scale_x).max(1e-3);
        let pry = (ry * self.state.scale_y).max(1e-3);

        // C++ emPainter_ScTl.cpp: nx = (255<<23)/rx, TX = (center-0.5)*nx
        let nx = (255_i64 << 23) as f64 / prx;
        let ny = (255_i64 << 23) as f64 / pry;
        let fp_tdx = nx as i64;
        let fp_tdy = ny as i64;
        let fp_tx = ((pcx - 0.5) * nx) as i64;
        let fp_ty = ((pcy - 0.5) * ny) as i64;

        // Ensure sqrt table is initialized.
        let _ = grad_sqrt_table();

        let px_texture = PixelTexture::RadialGradient {
            color_inner,
            color_outer,
            fp_tx,
            fp_ty,
            fp_tdx,
            fp_tdy,
        };

        for (y, spans) in &rows {
            for span in spans {
                self.blit_span_textured(proof, *y, span, &px_texture);
            }
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a line between two points.
    pub fn PaintLine(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintLine {
            x0,
            y0,
            x1,
            y1,
            color,
            canvas_color,
        }) else { return; };
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let px0 = self.to_pixel_x(x0);
        let py0 = self.to_pixel_y(y0);
        let px1 = self.to_pixel_x(x1);
        let py1 = self.to_pixel_y(y1);
        self.draw_line_pixels(proof, px0, py0, px1, py1, color);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon defined by a list of (x, y) vertices.
    /// Uses anti-aliased scanline rasterization with NonZero winding rule.
    pub fn PaintPolygon(&mut self, vertices: &[(f64, f64)], color: emColor, canvas_color: emColor) {
        let Some(proof) = self.try_record(DrawOp::PaintPolygon {
            vertices: vertices.to_vec(),
            color,
            canvas_color,
        }) else { return; };
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        self.fill_polygon_aa(proof, vertices, color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon using even-odd winding rule (for polygon rings with holes).
    pub fn paint_polygon_even_odd(
        &mut self,
        vertices: &[(f64, f64)],
        color: emColor,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintPolygonEvenOdd {
            vertices: vertices.to_vec(),
            color,
            canvas_color,
        }) else { return; };
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        self.fill_polygon_aa(proof, vertices, color, WindingRule::EvenOdd);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon with a texture (gradient, image, or solid color).
    /// Uses anti-aliased scanline rasterization with NonZero winding rule.
    pub fn paint_polygon_textured(
        &mut self,
        vertices: &[(f64, f64)],
        texture: &emTexture,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintPolygonTextured {
            vertices: vertices.to_vec(),
            texture: texture.clone(),
            canvas_color,
        }) else { return; };
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if let emTexture::SolidColor(color) = texture {
            self.fill_polygon_aa(proof, vertices, *color, WindingRule::NonZero);
        } else {
            self.fill_polygon_aa_textured(proof, vertices, texture, WindingRule::NonZero);
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon with a texture using even-odd winding rule.
    pub fn paint_polygon_textured_even_odd(
        &mut self,
        vertices: &[(f64, f64)],
        texture: &emTexture,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintPolygonTexturedEvenOdd {
            vertices: vertices.to_vec(),
            texture: texture.clone(),
            canvas_color,
        }) else { return; };
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if let emTexture::SolidColor(color) = texture {
            self.fill_polygon_aa(proof, vertices, *color, WindingRule::EvenOdd);
        } else {
            self.fill_polygon_aa_textured(proof, vertices, texture, WindingRule::EvenOdd);
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a polygon outline by stroking as a closed polyline with proper joins.
    pub fn PaintPolygonOutline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke_color: emColor,
        thickness: f64,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintPolygonOutline {
            vertices: vertices.to_vec(),
            stroke_color,
            thickness,
            canvas_color,
        }) else { return; };
        if vertices.len() < 2 {
            return;
        }
        let stroke = emStroke::new(stroke_color, thickness);
        self.PaintPolylineWithoutArrows(vertices, &stroke, true, canvas_color);
    }

    /// Draw a polyline with full stroke support (joins, caps, dashes, arrows).
    /// Matches C++ `PaintPolyline(xy, n, thickness, stroke, strokeStart, strokeEnd, canvasColor)`.
    /// Records as a compound op: PaintPolyline at depth N, sub-ops at depth N+1.
    // DIVERGED: PaintPolyline — takes &emStroke with start_end/finish_end fields and closed param; C++ takes separate thickness, strokeStart, strokeEnd args
    pub fn PaintPolyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
    ) {
        if vertices.is_empty() || stroke.width <= 0.0 {
            return;
        }
        let is_recording = self
            .try_record(DrawOp::PaintPolyline {
                vertices: vertices.to_vec(),
                stroke: stroke.clone(),
                closed,
                canvas_color,
            })
            .is_none();
        if is_recording {
            if !self.record_subops {
                return;
            }
            self.record_depth += 1;
        }
        let with_arrows = !closed
            && (stroke.start_end.IsDecorated() || stroke.finish_end.IsDecorated());
        if with_arrows {
            self.PaintPolylineWithArrows(vertices, stroke, closed, canvas_color, None);
        } else {
            self.PaintPolylineWithoutArrows(vertices, stroke, closed, canvas_color);
        }
        if is_recording {
            self.record_depth -= 1;
        }
    }

    /// Fill a rounded rectangle using AA polygon approximation.
    /// Matches C++ `PaintRoundRect(x, y, w, h, rx, ry, texture, canvasColor)`.
    #[allow(clippy::too_many_arguments)]
    /// Literal port of C++ emPainter::PaintRoundRect (emPainter.cpp:1258-1329).
    pub fn PaintRoundRect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        mut rx: f64,
        mut ry: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintRoundRect {
            x, y, w, h, rx, ry, color, canvas_color,
        }) else { return; };

        if w <= 0.0 || h <= 0.0 { return; }

        // C++ line 1299: degenerate to PaintRect when radius is non-positive.
        if rx <= 0.0 || ry <= 0.0 {
            self.PaintRect(x, y, w, h, color, canvas_color);
            return;
        }

        // C++ lines 1305-1306: clamp radii.
        if rx > w * 0.5 { rx = w * 0.5; }
        if ry > h * 0.5 { ry = h * 0.5; }

        // C++ lines 1307-1312: compute vertex count from CircleQuality.
        let circle_quality = CIRCLE_QUALITY;
        let f = circle_quality * (rx * self.state.scale_x + ry * self.state.scale_y).sqrt();
        let f = f.min(256.0) * 0.25;
        let n = if f <= 1.0 { 1 } else if f >= 64.0 { 64 } else { (f + 0.5) as usize };
        let step = std::f64::consts::FRAC_PI_2 / n as f64;

        // C++ lines 1314-1327: generate 4*(n+1) vertices.
        let cx1 = x + rx;
        let cy1 = y + ry;
        let cx2 = x + w - rx;
        let cy2 = y + h - ry;
        let total = 4 * (n + 1);
        let mut verts = Vec::with_capacity(total);
        for i in 0..=n {
            let a = step * i as f64;
            let dx = a.cos();
            let dy = a.sin();
            verts.push((cx1 - dx * rx, cy1 - dy * ry));       // top-left
        }
        for i in 0..=n {
            let a = step * i as f64;
            let dx = a.cos();
            let dy = a.sin();
            verts.push((cx2 + dy * rx, cy1 - dx * ry));       // top-right
        }
        for i in 0..=n {
            let a = step * i as f64;
            let dx = a.cos();
            let dy = a.sin();
            verts.push((cx2 + dx * rx, cy2 + dy * ry));       // bottom-right
        }
        for i in 0..=n {
            let a = step * i as f64;
            let dx = a.cos();
            let dy = a.sin();
            verts.push((cx1 - dy * rx, cy2 + dx * ry));       // bottom-left
        }
        self.PaintPolygon(&verts, color, canvas_color);
    }

    /// Draw a source image at the given position (convenience wrapper).
    /// Draws at 1:1 scale with full opacity and no canvas color.
    pub fn PaintImage(&mut self, x: f64, y: f64, image: &emImage) {
        let Some(_proof) = self.try_record(DrawOp::PaintImageSimple {
            x,
            y,
            image_ptr: image as *const emImage,
        }) else { return; };
        let iw = image.GetWidth() as f64 / self.state.scale_x;
        let ih = image.GetHeight() as f64 / self.state.scale_y;
        self.paint_image_full(x, y, iw, ih, image, 255, emColor::TRANSPARENT);
    }

    /// Draw a source image scaled to fill a destination rectangle with alpha
    /// modulation and canvas color support. Matches C++ `PaintImage`.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_image_full(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &emImage,
        alpha: u8,
        canvas_color: emColor,
    ) {
        if w <= 0.0 || h <= 0.0 || alpha == 0 {
            return;
        }
        let Some(proof) = self.try_record(DrawOp::PaintImageFull {
            x,
            y,
            w,
            h,
            image_ptr: image as *const emImage,
            alpha,
            canvas_color,
        }) else { return; };

        // Save and temporarily override canvas color and alpha.
        let saved_canvas = self.state.canvas_color;
        let saved_alpha = self.state.alpha;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // C++ EXTEND_EDGE_OR_ZERO: even channel count → EXTEND_ZERO.
        let ext = if image.GetChannelCount().is_multiple_of(2) {
            super::emTexture::ImageExtension::Zero
        } else {
            super::emTexture::ImageExtension::Clamp
        };

        let iw = image.GetWidth() as i32;
        let ih = image.GetHeight() as i32;
        self.paint_image_rect(proof, x, y, w, h, image, 0, 0, iw, ih, ext);

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Draw a sub-region of a source image into a destination rectangle.
    /// Matches C++ `PaintImage(x, y, w, h, img, srcX, srcY, srcW, srcH, alpha, canvasColor, ext)`.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintImageSrcRect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        alpha: u8,
        canvas_color: emColor,
        ext: super::emTexture::ImageExtension,
    ) {
        if w <= 0.0 || h <= 0.0 || alpha == 0 {
            return;
        }
        if src_w <= 0 || src_h <= 0 { return; }
        let Some(proof) = self.try_record(DrawOp::PaintImageFull {
            x, y, w, h,
            image_ptr: image as *const emImage,
            alpha,
            canvas_color,
        }) else { return; };

        let saved_canvas = self.state.canvas_color;
        let saved_alpha = self.state.alpha;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // Resolve EdgeOrZero: C++ EXTEND_EDGE_OR_ZERO resolves to EXTEND_ZERO
        // for images with even channel count (alpha channel), EXTEND_EDGE otherwise.
        let resolved_ext = match ext {
            super::emTexture::ImageExtension::EdgeOrZero => {
                if image.GetChannelCount().is_multiple_of(2) {
                    super::emTexture::ImageExtension::Zero
                } else {
                    super::emTexture::ImageExtension::Clamp
                }
            },
            other => other,
        };

        self.paint_image_rect(proof, x, y, w, h, image, src_x, src_y, src_w, src_h, resolved_ext);

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Draw an image with separate paint-rect and texture coordinates.
    ///
    /// Matches C++ `PaintRect(rx, ry, rw, rh, emImageTexture(tx, ty, tw, th, img, ...))`:
    /// - `(rect_x, rect_y, rect_w, rect_h)` is the visible paint rectangle (clipping)
    /// - `(tex_x, tex_y, tex_w, tex_h)` defines where the image maps in world space
    ///
    /// When texture coords differ from rect coords, the image tiles or is offset
    /// within the visible rectangle. The `extension` mode controls what happens
    /// beyond the texture boundary (Repeat for tiling, Clamp for edge, etc.).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintImageTextured(
        &mut self,
        rect_x: f64,
        rect_y: f64,
        rect_w: f64,
        rect_h: f64,
        tex_x: f64,
        tex_y: f64,
        tex_w: f64,
        tex_h: f64,
        image: &emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        alpha: u8,
        extension: super::emTexture::ImageExtension,
    ) {
        if rect_w <= 0.0 || rect_h <= 0.0 || alpha == 0 {
            return;
        }
        if tex_w <= 0.0 || tex_h <= 0.0 || src_w <= 0 || src_h <= 0 { return; }
        let Some(proof) = self.try_record(DrawOp::PaintImageTextured {
            rect_x, rect_y, rect_w, rect_h,
            tex_x, tex_y, tex_w, tex_h,
            image_ptr: image as *const emImage,
            src_x, src_y, src_w, src_h,
            alpha,
            extension,
        }) else { return; };

        let saved_canvas = self.state.canvas_color;
        let saved_alpha = self.state.alpha;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // Resolve EdgeOrZero: C++ resolves to EXTEND_ZERO for even channel count.
        let resolved_ext = match extension {
            super::emTexture::ImageExtension::EdgeOrZero => {
                if image.GetChannelCount().is_multiple_of(2) {
                    super::emTexture::ImageExtension::Zero
                } else {
                    super::emTexture::ImageExtension::Clamp
                }
            },
            other => other,
        };

        self.paint_image_rect_textured(
            proof,
            rect_x, rect_y, rect_w, rect_h,
            tex_x, tex_y, tex_w, tex_h,
            image, src_x, src_y, src_w, src_h, resolved_ext,
        );

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Draw an image with two-color mapping and separate texture coordinates.
    ///
    /// Matches C++ `PaintRect(rx, ry, rw, rh, emImageColoredTexture(tx, ty, tw, th, img, c1, c2))`.
    /// Pixel luminance maps linearly from `color1` (at 0) to `color2` (at 255).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintImageColoredTextured(
        &mut self,
        rect_x: f64,
        rect_y: f64,
        rect_w: f64,
        rect_h: f64,
        tex_x: f64,
        tex_y: f64,
        tex_w: f64,
        tex_h: f64,
        image: &emImage,
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        extension: ImageExtension,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintImageColoredTextured {
            rect_x, rect_y, rect_w, rect_h,
            tex_x, tex_y, tex_w, tex_h,
            image_ptr: image as *const emImage,
            color1, color2,
            canvas_color,
            extension,
        }) else { return; };

        // Floating-point dest rect in pixel space (sub-pixel precision).
        let dx = rect_x * self.state.scale_x + self.state.offset_x;
        let dy = rect_y * self.state.scale_y + self.state.offset_y;
        let dw = rect_w * self.state.scale_x;
        let dh = rect_h * self.state.scale_y;

        // Texture dimensions in pixel space.
        let tw_px = tex_w * self.state.scale_x;
        let th_px = tex_h * self.state.scale_y;

        let sp = SubPixelEdges::new(dx, dy, dw, dh);
        let px = sp.ix1;
        let py = sp.iy1;
        let px2 = sp.ix2;
        let pw = px2 - px;

        // C++ PaintRect-style iy2=truncate for Y coverage.
        let iy_raw = (dy * 4096.0) as i32;
        let iy2_raw = ((dy + dh) * 4096.0) as i32;
        let mut cpp_ay1 = 0x1000 - (iy_raw & 0xfff);
        let mut cpp_ay2 = iy2_raw & 0xfff;
        let cpp_iy1 = iy_raw >> 12;
        let cpp_iy2 = iy2_raw >> 12;
        if cpp_iy1 >= cpp_iy2 {
            cpp_ay1 += cpp_ay2 - 0x1000;
            cpp_ay2 = 0;
            if cpp_ay1 <= 0 { return; }
        }

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = cpp_iy1.max(cy1);
        let end_x = px2.min(cx2);
        let end_y = if cpp_ay2 > 0 || cpp_iy1 >= cpp_iy2 {
            (cpp_iy2 + 1).min(cy2)
        } else {
            cpp_iy2.min(cy2)
        };
        let ph = end_y - start_y;

        let iw = image.GetWidth();
        let ih = image.GetHeight();
        let src_w = iw;
        let src_h = ih;

        if pw <= 0 || ph <= 0 || src_w == 0 || src_h == 0 {
            return;
        }

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

        let ch = image.GetChannelCount();

        // Downscale decision uses texture pixel dimensions.
        let src_w_f = src_w as f64;
        let src_h_f = src_h as f64;
        let ratio_x = src_w_f / tw_px;
        let ratio_y = src_h_f / th_px;
        let downscaling = ratio_x > 1.0 || ratio_y > 1.0;

        let ext = extension.resolve_for_colored(color1, color2);

        let target_w = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(ch);
        let max_batch = ibuf.max_pixels();
        let mut coverages = vec![0i32; max_batch];
        let mut lums = [0u8; MAX_INTERP_BYTES];

        if downscaling {
            let tdx_init = ((src_w as i64) << 24) as f64 / tw_px;
            let tdy_init = ((src_h as i64) << 24) as f64 / th_px;
            let tdx_i = tdx_init as i64;
            let tdy_i = tdy_init as i64;
            let stride_x = if tdx_i > 0xFFFF00 {
                ((tdx_i / 3 + 0xFFFFFF) >> 24) as u32
            } else {
                1
            }
            .max(1);
            let stride_y = if tdy_i > 0xFFFF00 {
                ((tdy_i / 3 + 0xFFFFFF) >> 24) as u32
            } else {
                1
            }
            .max(1);
            let red_w = src_w.div_ceil(stride_x);
            let red_h = src_h.div_ceil(stride_y);
            let off_x = (src_w as i32 - (red_w as i32 - 1) * stride_x as i32 - 1) / 2;
            let off_y = (src_h as i32 - (red_h as i32 - 1) * stride_y as i32 - 1) / 2;
            // Transform uses texture coords.
            let mut xfm = self.area_sample_transform_24(red_w, red_h, tex_x, tex_y, tex_w, tex_h);
            xfm.stride_x = stride_x;
            xfm.stride_y = stride_y;
            xfm.off_x = off_x;
            xfm.off_y = off_y;
            let sec = emPainterInterpolation::SectionBounds {
                ox: 0, oy: 0, w: src_w as i32, h: src_h as i32,
            };

            for row in start_y..end_y {
                let mut carry = emPainterInterpolation::AreaSampleCarryState::new();
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    emPainterInterpolation::interpolate_scanline_area_sampled(
                        image, col, row, batch, &xfm, &sec, ext, &mut ibuf, &mut carry,
                    );
                    let all_full =
                        sp.batch_coverages_cpp_y(row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2);
                    let dest_offset = (row as usize * target_w + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    if ch >= 3 {
                        blend_colored_scanline_rgb(
                            dest, &ibuf, batch,
                            if all_full { None } else { Some(&coverages[..batch]) },
                            color1, color2, &mode,
                        );
                    } else {
                        for (i, lum) in lums[..batch].iter_mut().enumerate() {
                            *lum = ibuf.pixel_rgba(i)[0];
                        }
                        blend_colored_scanline(
                            dest, &lums[..batch], batch,
                            if all_full { None } else { Some(&coverages[..batch]) },
                            color1, color2, &mode,
                        );
                    }
                    col += batch as i32;
                }
            }
        } else {
            // Upscale: use texture coords for transform.
            let sxfm =
                self.scale_transform_24(src_w, src_h, tex_x, tex_y, tex_w, tex_h);
            let sec = emPainterInterpolation::SectionBounds {
                ox: 0, oy: 0, w: src_w as i32, h: src_h as i32,
            };
            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    if ch >= 3 {
                        emPainterInterpolation::interpolate_scanline_adaptive_premul_section(
                            image, px, py, col, row, batch, &sxfm, &sec, ext, &mut ibuf,
                        );
                    } else {
                        for (i, lum) in lums[..batch].iter_mut().enumerate() {
                            let c = col + i as i32;
                            let tx64 = (c - px) as i64 * sxfm.tdx + sxfm.base_x - 0x180_0000;
                            let ty64 = (row - py) as i64 * sxfm.tdy + sxfm.base_y - 0x180_0000;
                            let src_ix = (tx64 >> 24) as i32;
                            let src_iy = (ty64 >> 24) as i32;
                            let ox = (((tx64 & 0xFF_FFFF) as u32).wrapping_add(0x7FFF)) >> 16;
                            let oy = (((ty64 & 0xFF_FFFF) as u32).wrapping_add(0x7FFF)) >> 16;
                            *lum = emPainterInterpolation::sample_adaptive_lum_section(
                                image, src_ix, src_iy, ox, oy, &sec, ext,
                            );
                        }
                    }
                    let all_full =
                        sp.batch_coverages_cpp_y(row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2);
                    let dest_offset = (row as usize * target_w + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    if ch >= 3 {
                        blend_colored_scanline_rgb(
                            dest, &ibuf, batch,
                            if all_full { None } else { Some(&coverages[..batch]) },
                            color1, color2, &mode,
                        );
                    } else {
                        blend_colored_scanline(
                            dest, &lums[..batch], batch,
                            if all_full { None } else { Some(&coverages[..batch]) },
                            color1, color2, &mode,
                        );
                    }
                    col += batch as i32;
                }
            }
        }

        self.state.canvas_color = saved_canvas;
    }

    /// Core image rendering matching C++ PaintRect + ScanlineTool pipeline.
    ///
    /// Maps source sub-rect `(src_x, src_y, src_w, src_h)` into destination
    /// rect `(x, y, w, h)` using C++ PaintRect boundary formulas (iy2=truncate).
    /// Caller must set `self.state.canvas_color` and `self.state.alpha` first.
    ///
    /// This is the SINGLE rendering path for all image painting — matching C++
    /// where PaintImage is `inline PaintRect(x,y,w,h, emImageTexture(...), canvasColor)`.
    #[allow(clippy::too_many_arguments)]
    fn paint_image_rect(
        &mut self,
        proof: DirectProof,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        ext: super::emTexture::ImageExtension,
    ) {
        // Non-textured: rect coords and texture coords are the same.
        self.paint_image_rect_textured(proof, x, y, w, h, x, y, w, h, image, src_x, src_y, src_w, src_h, ext);
    }

    /// Core image rendering with separate paint-rect and texture coordinates.
    ///
    /// Matches C++ `PaintRect(rx, ry, rw, rh, emImageTexture(tx, ty, tw, th, ...))`:
    /// - `(rect_x, rect_y, rect_w, rect_h)` controls pixel clipping boundaries
    /// - `(tex_x, tex_y, tex_w, tex_h)` controls where the image maps in world space
    ///
    /// When tex coords differ from rect coords, the image tiles/offsets within the
    /// visible rect. Caller must set `self.state.canvas_color` and `self.state.alpha`.
    #[allow(clippy::too_many_arguments)]
    fn paint_image_rect_textured(
        &mut self,
        proof: DirectProof,
        rect_x: f64,
        rect_y: f64,
        rect_w: f64,
        rect_h: f64,
        tex_x: f64,
        tex_y: f64,
        tex_w: f64,
        tex_h: f64,
        image: &emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        ext: super::emTexture::ImageExtension,
    ) {
        if rect_w <= 0.0 || rect_h <= 0.0 || tex_w <= 0.0 || tex_h <= 0.0 || src_w <= 0 || src_h <= 0 {
            return;
        }

        // --- C++ PaintRect boundary computation (emPainter.cpp:395-441) ---
        // Literal port: transform to pixel space, clip, THEN compute fixed-point
        // boundaries. This matches PaintRect for color fills exactly.
        let px1 = (rect_x * self.state.scale_x + self.state.offset_x)
            .max(self.state.clip.x1).min(self.state.clip.x2);
        let px2 = ((rect_x + rect_w) * self.state.scale_x + self.state.offset_x)
            .max(self.state.clip.x1).min(self.state.clip.x2);
        if px1 >= px2 { return; }
        let py1 = (rect_y * self.state.scale_y + self.state.offset_y)
            .max(self.state.clip.y1).min(self.state.clip.y2);
        let py2 = ((rect_y + rect_h) * self.state.scale_y + self.state.offset_y)
            .max(self.state.clip.y1).min(self.state.clip.y2);
        if py1 >= py2 { return; }

        // C++ Fixed12 arithmetic (emPainter.cpp:412-430)
        let ix_raw = (px1 * 4096.0) as i32;
        let ixe_raw = (px2 * 4096.0) as i32 + 0xfff;
        let mut ax1 = 0x1000 - (ix_raw & 0xfff);
        let ax2 = (ixe_raw & 0xfff) + 1;
        let ix = ix_raw >> 12;
        let ixe = ixe_raw >> 12;
        let iw = ixe - ix;
        if iw <= 0 { return; }
        if iw <= 1 { ax1 += ax2 - 0x1000; }

        let iy_raw = (py1 * 4096.0) as i32;
        let iy2_raw = (py2 * 4096.0) as i32;
        let ay1 = 0x1000 - (iy_raw & 0xfff);
        let ay2 = iy2_raw & 0xfff;
        let iy = iy_raw >> 12;
        let iy2 = iy2_raw >> 12;
        let (ay1, ay2) = if iy >= iy2 {
            let collapsed = ay1 + ay2 - 0x1000;
            if collapsed <= 0 { return; }
            (collapsed, 0)
        } else {
            (ay1, ay2)
        };

        // --- C++ ScanlineTool::Init (emPainter_ScTl.cpp:228-378) ---
        // Clamp source rect to image bounds
        let iw_img = image.GetWidth() as i32;
        let ih_img = image.GetHeight() as i32;
        let csx = src_x.max(0);
        let csx2 = (src_x + src_w).min(iw_img);
        if csx >= csx2 { return; }
        let csy = src_y.max(0);
        let csy2 = (src_y + src_h).min(ih_img);
        if csy >= csy2 { return; }
        let img_w = csx2 - csx;
        let img_h = csy2 - csy;

        // Texture-to-image coordinate transform
        let tw_px = tex_w * self.state.scale_x;
        let th_px = tex_h * self.state.scale_y;
        let tdx_f64 = ((img_w as i64) << 24) as f64 / tw_px;
        let tdy_f64 = ((img_h as i64) << 24) as f64 / th_px;
        if !(0.0..=2.8e14).contains(&tdx_f64) || !(0.0..=2.8e14).contains(&tdy_f64) { return; }
        let tdx = tdx_f64 as i64;
        let tdy = tdy_f64 as i64;
        let tx_pixel = tex_x * self.state.scale_x + self.state.offset_x;
        let ty_pixel = tex_y * self.state.scale_y + self.state.offset_y;

        let sec = emPainterInterpolation::SectionBounds {
            ox: csx, oy: csy, w: img_w, h: img_h,
        };

        let tw_stride = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4);
        let max_batch = ibuf.max_pixels();
        let mut coverages = vec![0i32; max_batch];

        // Interpolation method selection (C++ emPainter_ScTl.cpp:286-378)
        let downscaling = tdx > 0xFFFF00 || tdy > 0xFFFF00;

        if downscaling {
            // C++ emPainter_ScTl.cpp:296-311: near-1:1 pixel-aligned → NEAREST
            let near_1_to_1 = tdx < 0x10000FF && tdy < 0x10000FF
                && tdx > 0x0FFFF00 && tdy > 0x0FFFF00
                && ((tx_pixel * tdx_f64) as i64 + 0x800) & 0xFFF000 == 0
                && ((ty_pixel * tdy_f64) as i64 + 0x800) & 0xFFF000 == 0;

            if near_1_to_1 {
                let sxfm = self.scale_transform_24(img_w as u32, img_h as u32, tex_x, tex_y, tex_w, tex_h);
                self.paint_image_scanlines_nearest(
                    proof, image, &sxfm, &sec, ext,
                    ix, iy, iy2, iw, ax1, ax2, ay1, ay2,
                    &mode, &mut ibuf, max_batch, &mut coverages, tw_stride,
                );
            } else {
                // Area sampling with pre-reduction (C++ emPainter_ScTl.cpp:312-343)
                let sw_u = img_w as u32;
                let sh_u = img_h as u32;
                let stride_x = if tdx > 0xFFFF00 { ((tdx / 3 + 0xFFFFFF) >> 24) as u32 } else { 1 }.max(1);
                let stride_y = if tdy > 0xFFFF00 { ((tdy / 3 + 0xFFFFFF) >> 24) as u32 } else { 1 }.max(1);
                let red_w = sw_u.div_ceil(stride_x);
                let red_h = sh_u.div_ceil(stride_y);
                let off_x = (sw_u as i32 - (red_w as i32 - 1) * stride_x as i32 - 1) / 2;
                let off_y = (sh_u as i32 - (red_h as i32 - 1) * stride_y as i32 - 1) / 2;
                let mut xfm = self.area_sample_transform_24(red_w, red_h, tex_x, tex_y, tex_w, tex_h);
                xfm.stride_x = stride_x;
                xfm.stride_y = stride_y;
                xfm.off_x = off_x;
                xfm.off_y = off_y;

                self.paint_image_scanlines_area(
                    proof, image, &xfm, &sec, ext,
                    ix, iy, iy2, iw, ax1, ax2, ay1, ay2,
                    &mode, &mut ibuf, max_batch, &mut coverages, tw_stride,
                );
            }
        } else {
            // Upscale or exact 1:1 (C++ emPainter_ScTl.cpp:345-378)
            let sxfm = self.scale_transform_24(img_w as u32, img_h as u32, tex_x, tex_y, tex_w, tex_h);
            let upscaling = tdx < 0x1000000 || tdy < 0x1000000;

            if upscaling {
                self.paint_image_scanlines_adaptive(
                    proof, image, &sxfm, &sec, ext,
                    ix, iy, iy2, iw, ax1, ax2, ay1, ay2,
                    &mode, &mut ibuf, max_batch, &mut coverages, tw_stride,
                );
            } else {
                self.paint_image_scanlines_nearest(
                    proof, image, &sxfm, &sec, ext,
                    ix, iy, iy2, iw, ax1, ax2, ay1, ay2,
                    &mode, &mut ibuf, max_batch, &mut coverages, tw_stride,
                );
            }
        }
    }

    /// Render image scanlines using nearest-neighbor interpolation.
    /// Matches C++ PaintRect scanline loop (partial top, full, partial bottom).
    #[allow(clippy::too_many_arguments)]
    fn paint_image_scanlines_nearest(
        &mut self,
        proof: DirectProof,
        image: &emImage,
        sxfm: &emPainterInterpolation::ScaleTransform24,
        sec: &emPainterInterpolation::SectionBounds,
        ext: super::emTexture::ImageExtension,
        ix: i32, mut iy: i32, iy2: i32, iw: i32,
        ax1: i32, ax2: i32, ay1: i32, ay2: i32,
        mode: &BlendMode,
        ibuf: &mut InterpolationBuffer, max_batch: usize,
        coverages: &mut [i32], tw_stride: usize,
    ) {
        // C++ PaintRect scanline loop (emPainter.cpp:432-441)
        if ay1 < 0x1000 {
            let a1 = ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            self.paint_image_scanline_nearest(
                proof, image, sxfm, sec, ext,
                ix, iy, iw, a1, ay1, a2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
            iy += 1;
        }
        while iy < iy2 {
            self.paint_image_scanline_nearest(
                proof, image, sxfm, sec, ext,
                ix, iy, iw, ax1, 0x1000, ax2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
            iy += 1;
        }
        if ay2 > 0 {
            let a1 = ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            self.paint_image_scanline_nearest(
                proof, image, sxfm, sec, ext,
                ix, iy, iw, a1, ay2, a2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
        }
    }

    /// Render one image scanline with nearest-neighbor interpolation.
    /// Coverage follows C++ PaintScanline pattern: first pixel (a1), middle (a), last (a2).
    #[allow(clippy::too_many_arguments)]
    fn paint_image_scanline_nearest(
        &mut self,
        proof: DirectProof,
        image: &emImage,
        sxfm: &emPainterInterpolation::ScaleTransform24,
        sec: &emPainterInterpolation::SectionBounds,
        ext: super::emTexture::ImageExtension,
        ix: i32, iy: i32, iw: i32,
        a1: i32, a: i32, a2: i32,
        mode: &BlendMode,
        ibuf: &mut InterpolationBuffer, max_batch: usize,
        coverages: &mut [i32], tw_stride: usize,
    ) {
        let mut col = ix;
        let end = ix + iw;
        while col < end {
            let batch = ((end - col) as usize).min(max_batch);
            emPainterInterpolation::interpolate_scanline_nearest_section(
                image, col, iy, batch, sxfm, sec, ext, ibuf,
            );
            // Build coverage array matching C++ PaintScanline three-part pattern
            for (i, cov) in coverages[..batch].iter_mut().enumerate() {
                let px = col + i as i32;
                *cov = if px == ix { a1 }
                    else if px == end - 1 { a2 }
                    else { a };
            }
            let all_full = coverages[..batch].iter().all(|&c| c >= 0x1000);
            let dest_offset = (iy as usize * tw_stride + col as usize) * 4;
            let data = self.GetImage(proof).GetWritableMap();
            let dest = &mut data[dest_offset..];
            if all_full { blend_scanline(dest, ibuf, batch, None, mode); }
            else { blend_scanline(dest, ibuf, batch, Some(&coverages[..batch]), mode); }
            col += batch as i32;
        }
    }

    /// Render image scanlines using area sampling.
    #[allow(clippy::too_many_arguments)]
    fn paint_image_scanlines_area(
        &mut self,
        proof: DirectProof,
        image: &emImage,
        xfm: &emPainterInterpolation::AreaSampleTransform,
        sec: &emPainterInterpolation::SectionBounds,
        ext: super::emTexture::ImageExtension,
        ix: i32, mut iy: i32, iy2: i32, iw: i32,
        ax1: i32, ax2: i32, ay1: i32, ay2: i32,
        mode: &BlendMode,
        ibuf: &mut InterpolationBuffer, max_batch: usize,
        coverages: &mut [i32], tw_stride: usize,
    ) {
        if ay1 < 0x1000 {
            let a1 = ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            self.paint_image_scanline_area(
                proof, image, xfm, sec, ext,
                ix, iy, iw, a1, ay1, a2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
            iy += 1;
        }
        while iy < iy2 {
            self.paint_image_scanline_area(
                proof, image, xfm, sec, ext,
                ix, iy, iw, ax1, 0x1000, ax2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
            iy += 1;
        }
        if ay2 > 0 {
            let a1 = ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            self.paint_image_scanline_area(
                proof, image, xfm, sec, ext,
                ix, iy, iw, a1, ay2, a2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_image_scanline_area(
        &mut self,
        proof: DirectProof,
        image: &emImage,
        xfm: &emPainterInterpolation::AreaSampleTransform,
        sec: &emPainterInterpolation::SectionBounds,
        ext: super::emTexture::ImageExtension,
        ix: i32, iy: i32, iw: i32,
        a1: i32, a: i32, a2: i32,
        mode: &BlendMode,
        ibuf: &mut InterpolationBuffer, max_batch: usize,
        coverages: &mut [i32], tw_stride: usize,
    ) {
        let end = ix + iw;
        let mut carry = emPainterInterpolation::AreaSampleCarryState::new();
        let mut col = ix;
        while col < end {
            let batch = ((end - col) as usize).min(max_batch);
            emPainterInterpolation::interpolate_scanline_area_sampled(
                image, col, iy, batch, xfm, sec, ext, ibuf, &mut carry,
            );
            for (i, cov) in coverages[..batch].iter_mut().enumerate() {
                let px = col + i as i32;
                *cov = if px == ix { a1 }
                    else if px == end - 1 { a2 }
                    else { a };
            }
            let all_full = coverages[..batch].iter().all(|&c| c >= 0x1000);
            let dest_offset = (iy as usize * tw_stride + col as usize) * 4;
            let data = self.GetImage(proof).GetWritableMap();
            let dest = &mut data[dest_offset..];
            if all_full { blend_scanline_premul(dest, ibuf, batch, None, mode); }
            else { blend_scanline_premul(dest, ibuf, batch, Some(&coverages[..batch]), mode); }
            col += batch as i32;
        }
    }

    /// Render image scanlines using adaptive interpolation.
    #[allow(clippy::too_many_arguments)]
    fn paint_image_scanlines_adaptive(
        &mut self,
        proof: DirectProof,
        image: &emImage,
        sxfm: &emPainterInterpolation::ScaleTransform24,
        sec: &emPainterInterpolation::SectionBounds,
        ext: super::emTexture::ImageExtension,
        ix: i32, mut iy: i32, iy2: i32, iw: i32,
        ax1: i32, ax2: i32, ay1: i32, ay2: i32,
        mode: &BlendMode,
        ibuf: &mut InterpolationBuffer, max_batch: usize,
        coverages: &mut [i32], tw_stride: usize,
    ) {
        if ay1 < 0x1000 {
            let a1 = ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            self.paint_image_scanline_adaptive(
                proof, image, sxfm, sec, ext,
                ix, iy, iw, a1, ay1, a2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
            iy += 1;
        }
        while iy < iy2 {
            self.paint_image_scanline_adaptive(
                proof, image, sxfm, sec, ext,
                ix, iy, iw, ax1, 0x1000, ax2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
            iy += 1;
        }
        if ay2 > 0 {
            let a1 = ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            self.paint_image_scanline_adaptive(
                proof, image, sxfm, sec, ext,
                ix, iy, iw, a1, ay2, a2,
                mode, ibuf, max_batch, coverages, tw_stride,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_image_scanline_adaptive(
        &mut self,
        proof: DirectProof,
        image: &emImage,
        sxfm: &emPainterInterpolation::ScaleTransform24,
        sec: &emPainterInterpolation::SectionBounds,
        ext: super::emTexture::ImageExtension,
        ix: i32, iy: i32, iw: i32,
        a1: i32, a: i32, a2: i32,
        mode: &BlendMode,
        ibuf: &mut InterpolationBuffer, max_batch: usize,
        coverages: &mut [i32], tw_stride: usize,
    ) {
        let end = ix + iw;
        let mut col = ix;
        while col < end {
            let batch = ((end - col) as usize).min(max_batch);
            emPainterInterpolation::interpolate_scanline_adaptive_premul_section(
                image, sxfm.px, sxfm.py, col, iy, batch, sxfm, sec, ext, ibuf,
            );
            for (i, cov) in coverages[..batch].iter_mut().enumerate() {
                let px = col + i as i32;
                *cov = if px == ix { a1 }
                    else if px == end - 1 { a2 }
                    else { a };
            }
            let all_full = coverages[..batch].iter().all(|&c| c >= 0x1000);
            let dest_offset = (iy as usize * tw_stride + col as usize) * 4;
            let data = self.GetImage(proof).GetWritableMap();
            let dest = &mut data[dest_offset..];
            if all_full { blend_scanline_premul(dest, ibuf, batch, None, mode); }
            else { blend_scanline_premul(dest, ibuf, batch, Some(&coverages[..batch]), mode); }
            col += batch as i32;
        }
    }

    /// Draw an image with two-color mapping and canvas color support.
    /// Pixel luminance maps linearly from `color1` (at 0) to `color2` (at 255).
    /// C++ `PaintImageColored` = `PaintRect(x,y,w,h, emImageColoredTexture(...), canvasColor)`.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintImageColored(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &emImage,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        extension: ImageExtension,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintImageColored {
            x, y, w, h,
            image_ptr: image as *const emImage,
            src_x, src_y, src_w, src_h,
            color1, color2, canvas_color, extension,
        }) else { return; };

        let texture = super::emTexture::emTexture::ImageColoredGradient {
            image_ref: image as *const emImage,
            src_x, src_y, src_w, src_h,
            color1, color2, extension,
        };
        self.paint_rect_textured(x, y, w, h, &texture, canvas_color);
    }

    /// Core textured rect rendering matching C++ `PaintRect(texture, canvasColor)`.
    /// Uses the same Fixed12 coverage loop as PaintRect(color).
    /// Handles all texture types: solid, gradient, image colored.
    #[allow(clippy::too_many_arguments)]
    fn paint_rect_textured(
        &mut self,
        x: f64, y: f64, w: f64, h: f64,
        texture: &super::emTexture::emTexture,
        canvas_color: emColor,
    ) {
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

        // C++ PaintRect Fixed12 boundary computation
        let x2f = x + w;
        let y2f = y + h;
        let px1 = (x * self.state.scale_x + self.state.offset_x)
            .max(self.state.clip.x1).min(self.state.clip.x2);
        let px2 = (x2f * self.state.scale_x + self.state.offset_x)
            .max(self.state.clip.x1).min(self.state.clip.x2);
        let py1 = (y * self.state.scale_y + self.state.offset_y)
            .max(self.state.clip.y1).min(self.state.clip.y2);
        let py2 = (y2f * self.state.scale_y + self.state.offset_y)
            .max(self.state.clip.y1).min(self.state.clip.y2);
        if px1 >= px2 || py1 >= py2 {
            self.state.canvas_color = saved_canvas;
            return;
        }

        let ix_raw = (px1 * 4096.0) as i32;
        let ixe_raw = (px2 * 4096.0) as i32 + 0xfff;
        let mut ax1 = 0x1000 - (ix_raw & 0xfff);
        let ax2 = (ixe_raw & 0xfff) + 1;
        let ix = ix_raw >> 12;
        let ixe = ixe_raw >> 12;
        let iw = ixe - ix;
        if iw <= 0 { self.state.canvas_color = saved_canvas; return; }
        if iw <= 1 { ax1 += ax2 - 0x1000; }

        let iy_raw = (py1 * 4096.0) as i32;
        let iy2_raw = (py2 * 4096.0) as i32;
        let mut ay1 = 0x1000 - (iy_raw & 0xfff);
        let mut ay2 = iy2_raw & 0xfff;
        let mut iy = iy_raw >> 12;
        let iy2 = iy2_raw >> 12;
        if iy >= iy2 {
            ay1 += ay2 - 0x1000;
            ay2 = 0;
            if ay1 <= 0 { self.state.canvas_color = saved_canvas; return; }
        }

        // Dispatch by texture type — mimics C++ ScanlineTool::Init + PaintScanline loop
        let proof = match self.require_direct() {
            Some(p) => p,
            None => { self.state.canvas_color = saved_canvas; return; }
        };

        match texture {
            super::emTexture::emTexture::SolidColor(c) => {
                let c = *c;
                if ay1 < 0x1000 {
                    let a1 = ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
                    let a2 = ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
                    self.paint_rect_scanline(proof, ix, iy, iw, a1, ay1, a2, c);
                    iy += 1;
                }
                while iy < iy2 {
                    self.paint_rect_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, c);
                    iy += 1;
                }
                if ay2 > 0 {
                    let a1 = ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
                    let a2 = ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
                    self.paint_rect_scanline(proof, ix, iy, iw, a1, ay2, a2, c);
                }
            }
            super::emTexture::emTexture::LinearGradient { color_a, color_b, start, end } => {
                let pstart = (
                    start.0 * self.state.scale_x + self.state.offset_x,
                    start.1 * self.state.scale_y + self.state.offset_y,
                );
                let pend = (
                    end.0 * self.state.scale_x + self.state.offset_x,
                    end.1 * self.state.scale_y + self.state.offset_y,
                );
                let grad = emPainterInterpolation::LinearGradientParams::new(pstart, pend);
                let mut gbuf = vec![0u8; iw as usize];
                if ay1 < 0x1000 {
                    self.paint_rect_gradient_scanline(proof, ix, iy, iw, ax1, ay1, ax2, &grad, &mut gbuf, *color_a, *color_b);
                    iy += 1;
                }
                while iy < iy2 {
                    self.paint_rect_gradient_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, &grad, &mut gbuf, *color_a, *color_b);
                    iy += 1;
                }
                if ay2 > 0 {
                    self.paint_rect_gradient_scanline(proof, ix, iy, iw, ax1, ay2, ax2, &grad, &mut gbuf, *color_a, *color_b);
                }
            }
            super::emTexture::emTexture::RadialGradient { color_inner, color_outer, center, radius_x, radius_y } => {
                let pcx = center.0 * self.state.scale_x + self.state.offset_x;
                let pcy = center.1 * self.state.scale_y + self.state.offset_y;
                let prx = radius_x * self.state.scale_x;
                let pry = radius_y * self.state.scale_y;
                if ay1 < 0x1000 {
                    self.paint_rect_radial_scanline(proof, ix, iy, iw, ax1, ay1, ax2, pcx, pcy, prx, pry, *color_inner, *color_outer);
                    iy += 1;
                }
                while iy < iy2 {
                    self.paint_rect_radial_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, pcx, pcy, prx, pry, *color_inner, *color_outer);
                    iy += 1;
                }
                if ay2 > 0 {
                    self.paint_rect_radial_scanline(proof, ix, iy, iw, ax1, ay2, ax2, pcx, pcy, prx, pry, *color_inner, *color_outer);
                }
            }
            super::emTexture::emTexture::ImageColoredGradient {
                image_ref, src_x, src_y, src_w, src_h, color1, color2, extension,
            } => {
                let image = unsafe { &**image_ref };
                self.paint_rect_image_colored(
                    proof, x, y, w, h,
                    ix, iy, iy2, iw, ax1, ax2, ay1, ay2,
                    image, *src_x, *src_y, *src_w, *src_h,
                    *color1, *color2, *extension,
                );
            }
            _ => {} // emImage, ImageColored — handled by other paths
        }

        self.state.canvas_color = saved_canvas;
    }

    /// Image colored gradient scanline rendering.
    /// Moved from the old PaintImageColored inline body.
    #[allow(clippy::too_many_arguments)]
    fn paint_rect_image_colored(
        &mut self,
        proof: DirectProof,
        x: f64, y: f64, w: f64, h: f64,
        ix: i32, mut iy: i32, iy2: i32, iw: i32,
        ax1: i32, ax2: i32, ay1: i32, ay2: i32,
        image: &emImage,
        src_x: u32, src_y: u32, src_w: u32, src_h: u32,
        color1: emColor, color2: emColor,
        extension: ImageExtension,
    ) {
        if src_w == 0 || src_h == 0 { return; }

        let ch = image.GetChannelCount();
        let dw = w * self.state.scale_x;
        let dh = h * self.state.scale_y;
        let downscaling = (src_w as f64 / dw) > 1.0 || (src_h as f64 / dh) > 1.0;
        let ext = extension.resolve_for_colored(color1, color2);

        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(ch);
        let max_batch = ibuf.max_pixels();
        let mut lums = [0u8; MAX_INTERP_BYTES];

        let sec = emPainterInterpolation::SectionBounds {
            ox: src_x as i32, oy: src_y as i32,
            w: src_w as i32, h: src_h as i32,
        };

        // Scanline loop matching C++ PaintRect: ay1 (top), 0x1000 (interior), ay2 (bottom)
        let ixe = ix + iw;
        let sxfm = if !downscaling { Some(self.scale_transform_24(src_w, src_h, x, y, w, h)) } else { None };

        let mut cur_iy = iy;
        loop {
            let ay = if cur_iy == iy && ay1 < 0x1000 {
                ay1
            } else if cur_iy == iy2 {
                if ay2 <= 0 { break; }
                ay2
            } else if cur_iy > iy2 {
                break;
            } else {
                0x1000
            };

            let mut col = ix;
            if downscaling {
                let tdx_i = (((src_w as i64) << 24) as f64 / dw) as i64;
                let tdy_i = (((src_h as i64) << 24) as f64 / dh) as i64;
                let stride_x = if tdx_i > 0xFFFF00 { ((tdx_i / 3 + 0xFFFFFF) >> 24) as u32 } else { 1 }.max(1);
                let stride_y = if tdy_i > 0xFFFF00 { ((tdy_i / 3 + 0xFFFFFF) >> 24) as u32 } else { 1 }.max(1);
                let red_w = src_w.div_ceil(stride_x);
                let red_h = src_h.div_ceil(stride_y);
                let mut xfm = self.area_sample_transform_24(red_w, red_h, x, y, w, h);
                xfm.stride_x = stride_x;
                xfm.stride_y = stride_y;
                xfm.off_x = (src_w as i32 - (red_w as i32 - 1) * stride_x as i32 - 1) / 2;
                xfm.off_y = (src_h as i32 - (red_h as i32 - 1) * stride_y as i32 - 1) / 2;
                let mut carry = emPainterInterpolation::AreaSampleCarryState::new();
                while col < ixe {
                    let batch = ((ixe - col) as usize).min(max_batch);
                    emPainterInterpolation::interpolate_scanline_area_sampled(
                        image, col, cur_iy, batch, &xfm, &sec, ext, &mut ibuf, &mut carry,
                    );
                    let dest_offset = (cur_iy as usize * tw + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    let mut covs = vec![0i32; batch];
                    for (i, cov) in covs.iter_mut().enumerate() {
                        let ci = col + i as i32;
                        let ax = if ci == ix { ax1 } else if ci == ixe - 1 && iw > 1 { ax2 } else { 0x1000 };
                        *cov = ((ax as i64 * ay as i64 + 0x7ff) >> 12) as i32;
                    }
                    let all_full = covs.iter().all(|&c| c >= 0x1000);
                    if ch >= 3 {
                        blend_colored_scanline_rgb(dest, &ibuf, batch,
                            if all_full { None } else { Some(&covs) }, color1, color2, &mode);
                    } else {
                        for (i, lum) in lums[..batch].iter_mut().enumerate() { *lum = ibuf.pixel_rgba(i)[0]; }
                        blend_colored_scanline(dest, &lums[..batch], batch,
                            if all_full { None } else { Some(&covs) }, color1, color2, &mode);
                    }
                    col += batch as i32;
                }
            } else if let Some(ref sxfm) = sxfm {
                while col < ixe {
                    let batch = ((ixe - col) as usize).min(max_batch);
                    if ch >= 3 {
                        emPainterInterpolation::interpolate_scanline_adaptive_premul_section(
                            image, ix, iy, col, cur_iy, batch, sxfm, &sec, ext, &mut ibuf,
                        );
                    } else {
                        for (i, lum) in lums[..batch].iter_mut().enumerate() {
                            let c = col + i as i32;
                            let tx64 = (c - ix) as i64 * sxfm.tdx + sxfm.base_x - 0x180_0000;
                            let ty64 = (cur_iy - iy) as i64 * sxfm.tdy + sxfm.base_y - 0x180_0000;
                            *lum = emPainterInterpolation::sample_adaptive_lum_section(
                                image, (tx64 >> 24) as i32, (ty64 >> 24) as i32,
                                (((tx64 & 0xFF_FFFF) as u32).wrapping_add(0x7FFF)) >> 16,
                                (((ty64 & 0xFF_FFFF) as u32).wrapping_add(0x7FFF)) >> 16,
                                &sec, ext,
                            );
                        }
                    }
                    let dest_offset = (cur_iy as usize * tw + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    let mut covs = vec![0i32; batch];
                    for (i, cov) in covs.iter_mut().enumerate() {
                        let ci = col + i as i32;
                        let ax = if ci == ix { ax1 } else if ci == ixe - 1 && iw > 1 { ax2 } else { 0x1000 };
                        *cov = ((ax as i64 * ay as i64 + 0x7ff) >> 12) as i32;
                    }
                    let all_full = covs.iter().all(|&c| c >= 0x1000);
                    if ch >= 3 {
                        blend_colored_scanline_rgb(dest, &ibuf, batch,
                            if all_full { None } else { Some(&covs) }, color1, color2, &mode);
                    } else {
                        blend_colored_scanline(dest, &lums[..batch], batch,
                            if all_full { None } else { Some(&covs) }, color1, color2, &mode);
                    }
                    col += batch as i32;
                }
            }
            cur_iy += 1;
        }
    }

    // ── Text rendering ────────────────────────────────────────────────

    /// Calculate the width and height of a text string.
    ///
    /// Matches C++ `emPainter::GetTextSize`:
    /// Measure text dimensions at a given character height.
    /// Literal port of C++ `emPainter::GetTextSize` (emPainter.cpp:2287-2335).
    /// Operates on raw bytes to match C++ byte-index arithmetic exactly.
    pub fn GetTextSize(
        text: &str,
        char_height: f64,
        formatted: bool,
        rel_line_space: f64,
    ) -> (f64, f64) {
        let bytes = text.as_bytes();
        let text_len = bytes.len();
        let (columns, rows);

        if formatted {
            let mut max_columns = 0i32;
            let mut row_count = 1i32;
            let mut rowcols = 0i32;
            let mut i = 0usize;
            while i < text_len {
                let c = bytes[i];
                if c <= 0x0d {
                    if c == 0x09 {
                        // Tab: align to next multiple of 8 using C++ byte-index trick
                        rowcols = (((rowcols + i as i32 + 8) & !7) - i as i32) - 1;
                    } else if c == 0x0a {
                        // LF
                        let rc = rowcols + i as i32;
                        if max_columns < rc {
                            max_columns = rc;
                        }
                        rowcols = -(i as i32) - 1;
                        row_count += 1;
                    } else if c == 0x0d {
                        // CR (optionally followed by LF)
                        let rc = rowcols + i as i32;
                        if max_columns < rc {
                            max_columns = rc;
                        }
                        if i + 1 < text_len && bytes[i + 1] == 0x0a {
                            i += 1;
                        }
                        rowcols = -(i as i32) - 1;
                        row_count += 1;
                    }
                    // C++ checks c==0 for NUL, but Rust &str never contains NUL
                } else if c >= 0x80 {
                    // Multi-byte UTF-8: count decoded character as 1 column,
                    // skip continuation bytes (matching C++ emDecodeChar skip).
                    let n = utf8_char_len(c);
                    if n > 1 {
                        i += n - 1;
                        rowcols -= (n - 1) as i32;
                    }
                }
                i += 1;
            }
            let rc = rowcols + text_len as i32;
            if max_columns < rc {
                max_columns = rc;
            }
            columns = max_columns as usize;
            rows = row_count as usize;
        } else {
            // Non-formatted: count decoded characters
            columns = text.chars().count();
            rows = 1;
        }

        let w = char_height * columns as f64 / emFontCache::CHAR_BOX_TALLNESS;
        let h = char_height * (1.0 + rel_line_space) * rows as f64;
        (w, h)
    }

    /// Paint a single line of text using the Eagle Mode grayscale font atlas.
    ///
    /// Matches C++ `emPainter::PaintText`:
    ///   - `x`, `y`: upper-left corner of first character.
    ///   - `char_height`: character height in user coords.
    ///   - `width_scale`: factor for character width (1.0 = normal).
    ///   - `color`: text color.
    ///   - `canvas_color`: for canvas-color compositing (TRANSPARENT = standard).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintText(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        char_height: f64,
        width_scale: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        if text.is_empty() || char_height <= 0.0 || color.GetAlpha() == 0 {
            return;
        }
        let is_recording = self.try_record(DrawOp::PaintText {
            x,
            y,
            text: text.to_string(),
            char_height,
            width_scale,
            color,
            canvas_color,
        }).is_none();
        if is_recording {
            if !self.record_subops {
                return;
            }
            self.record_depth += 1;
        }

        let rcw = char_height / emFontCache::CHAR_BOX_TALLNESS;
        let char_width = rcw * width_scale;

        // Tiny text fallback: colored rectangles instead of glyphs.
        let pixel_height = char_height * self.state.scale_y;
        if pixel_height < 1.7 {
            self.paint_text_tiny(
                x,
                y,
                text,
                char_width,
                char_height,
                color,
                canvas_color,
            );
            if is_recording { self.record_depth -= 1; }
            return;
        }

        let clip_x1 = self.GetUserClipX1();
        let clip_x2 = self.GetUserClipX2();
        let clip_y1 = self.GetUserClipY1();
        let clip_y2 = self.GetUserClipY2();

        if y >= clip_y2 || y + char_height <= clip_y1 {
            if is_recording { self.record_depth -= 1; }
            return;
        }

        let font_atlas = emFontCache::atlas();

        let mut cx = x;
        for ch in text.chars() {
            if cx >= clip_x2 {
                break;
            }
            let x1 = cx;
            cx += char_width;
            if cx <= clip_x1 {
                continue;
            }

            // C++ PaintText: per-character show_height from font cache glyph dims.
            let (src_x, src_y, src_w, src_h) = emFontCache::GetChar(ch);
            let mut show_height = rcw * src_h as f64 / src_w as f64;
            if show_height > char_height { show_height = char_height; }

            self.PaintImageColored(
                x1,
                y + (char_height - show_height) * 0.5,
                char_width,
                show_height,
                font_atlas,
                src_x,
                src_y,
                src_w,
                src_h,
                emColor::TRANSPARENT,
                color,
                canvas_color,
                ImageExtension::Zero,
            );
        }

        if is_recording { self.record_depth -= 1; }
    }

    /// Tiny-text fallback: at very small sizes, render non-space runs as
    /// colored rectangles with reduced alpha (1/3 per C++).
    #[allow(clippy::too_many_arguments)]
    /// Tiny-text fallback: literal port of C++ PaintText else branch (lines 2139-2163).
    /// Renders non-space character runs as colored rectangles with alpha/3.
    #[allow(clippy::too_many_arguments)]
    fn paint_text_tiny(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        char_width: f64,
        char_height: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        // C++ line 2140: color.SetAlpha((emByte)((color.GetAlpha()+2)/3))
        let reduced_alpha = (color.GetAlpha() as u32).div_ceil(3) as u8;
        let rc = color.SetAlpha(reduced_alpha);
        let mut cx = x;
        let mut run_start = x; // C++ x1 = x initially
        let mut in_run = false;

        // C++ iterates bytes, splits on c <= 0x20 (space and all control chars).
        // For text that has already been split by formatted renderer,
        // only space (0x20) should be present as a whitespace character.
        // But match C++ exactly: split on ANY byte <= 0x20.
        for ch in text.chars() {
            if ch <= ' ' {
                // C++ line 2143-2150: flush run, advance past whitespace.
                if in_run && cx > run_start {
                    self.PaintRect(run_start, y, cx - run_start, char_height, rc, canvas_color);
                }
                cx += char_width;
                run_start = cx;
                in_run = false;
            } else {
                if !in_run {
                    run_start = cx;
                    in_run = true;
                }
                cx += char_width;
            }
        }
        // Flush final run.
        if in_run && cx > run_start {
            self.PaintRect(run_start, y, cx - run_start, char_height, rc, canvas_color);
        }
    }

    /// Paint text fitted into a rectangle, with optional formatting.
    ///
    /// Matches C++ `emPainter::PaintTextBoxed`:
    ///   - Measures text at `max_char_height`, scales down if it exceeds the box.
    ///   - `box_h_align` / `box_v_align`: how to position the text block.
    ///   - `text_alignment`: how to align individual lines horizontally.
    /// C++ emPainter::PaintTextBoxed (emPainter.cpp:2566-2702).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintTextBoxed(
        &mut self,
        mut x: f64, mut y: f64, w: f64, h: f64,
        text: &str, max_char_height: f64,
        color: emColor, canvas_color: emColor,
        box_h_align: TextAlignment, box_v_align: VAlign,
        text_alignment: TextAlignment,
        min_width_scale: f64, formatted: bool, rel_line_space: f64,
    ) {
        if text.is_empty() { return; }
        let is_recording = self.try_record(DrawOp::PaintTextBoxed {
            x, y, w, h, text: text.to_string(), max_char_height, color, canvas_color,
            box_h_align, box_v_align, text_alignment, min_width_scale, formatted, rel_line_space,
        }).is_none();
        if is_recording {
            if !self.record_subops { return; }
            self.record_depth += 1;
        }

        let mut ch = max_char_height;
        let (mut tw, mut th) = Self::GetTextSize(text, ch, formatted, rel_line_space);
        if tw <= 0.0 { if is_recording { self.record_depth -= 1; } return; }

        // C++ lines 2605-2630: scale ch/tw/th to fit in (w,h)
        if th > h { ch *= h / th; tw *= h / th; th = h; }
        let mut ws = w / tw;
        if ws < 1.0 {
            tw = w;
            if ws < min_width_scale {
                th *= ws / min_width_scale; ch *= ws / min_width_scale; ws = min_width_scale;
            }
        } else {
            ws = 1.0;
            if ws < min_width_scale {
                ws = min_width_scale; tw *= ws;
                if tw > w { th *= w / tw; ch *= w / tw; tw = w; }
            }
        }

        // C++ lines 2631-2638: box alignment
        if box_h_align != TextAlignment::Left {
            if box_h_align == TextAlignment::Right { x += w - tw; }
            else { x += (w - tw) * 0.5; }
        }
        if box_v_align != VAlign::Top {
            if box_v_align == VAlign::Bottom { y += h - th + ch * rel_line_space; }
            else { y += (h - th + ch * rel_line_space) * 0.5; }
        }

        if formatted {
            // C++ lines 2639-2696: formatted rendering with tabs and line breaks
            let cw = ch * ws / emFontCache::CHAR_BOX_TALLNESS;
            let bytes = text.as_bytes();
            let text_len = bytes.len();
            let mut ty = y;
            let mut i = 0usize;
            loop {
                let mut tx = x;
                if text_alignment != TextAlignment::Left {
                    let mut j2 = i;
                    let mut cols2 = -(j2 as i32);
                    while j2 < text_len {
                        let c = bytes[j2];
                        if c <= 0x0d {
                            if c == 0x09 { cols2 = ((cols2 + j2 as i32 + 8) & !7) - j2 as i32; }
                            else if c == 0x0a || c == 0x0d { break; }
                        } else if c >= 0x80 {
                            let n = utf8_char_len(c);
                            if n > 1 { j2 += n - 1; cols2 -= (n - 1) as i32; }
                        }
                        j2 += 1;
                    }
                    cols2 += j2 as i32;
                    if text_alignment == TextAlignment::Right { tx += tw - cols2 as f64 * cw; }
                    else { tx += (tw - cols2 as f64 * cw) * 0.5; }
                }

                let mut cols = 0i32;
                let mut j = i;
                let mut k = -(i as i32);
                while i < text_len {
                    let c = bytes[i];
                    if c <= 0x0d {
                        if c == 0x09 {
                            if j < i {
                                self.PaintText(tx + cols as f64 * cw, ty, &text[j..i], ch, ws, color, canvas_color);
                                cols += k + i as i32;
                            }
                            cols = (cols + 8) & !7;
                            j = i + 1; k = -(j as i32);
                        } else if c == 0x0a || c == 0x0d { break; }
                    } else if c >= 0x80 {
                        let n = utf8_char_len(c);
                        if n > 1 { i += n - 1; k -= (n - 1) as i32; }
                    }
                    i += 1;
                }
                if j < i {
                    self.PaintText(tx + cols as f64 * cw, ty, &text[j..i], ch, ws, color, canvas_color);
                }
                if i >= text_len { break; }
                if bytes[i] == 0x0d && i + 1 < text_len && bytes[i + 1] == 0x0a { i += 1; }
                i += 1;
                ty += ch * (1.0 + rel_line_space);
            }
        } else {
            self.PaintText(x, y, text, ch, ws, color, canvas_color);
        }
        if is_recording { self.record_depth -= 1; }
    }

    /// Convenience: measure text width for a single un-formatted line.
    /// Returns the width in the same coordinate space as the painter.
    pub fn measure_text_width(text: &str, char_height: f64) -> f64 {
        char_height * text.chars().count() as f64 / emFontCache::CHAR_BOX_TALLNESS
    }

    /// Draw an image scaled to fill a destination rectangle.
    /// Auto-selects AreaSampled for downscaling.
    #[allow(clippy::too_many_arguments)]
    /// C++ PaintImage = PaintRect(x,y,w,h, emImageTexture(...), canvasColor).
    pub fn paint_image_scaled(
        &mut self,
        x: f64, y: f64, w: f64, h: f64,
        image: &emImage,
        quality: super::emTexture::ImageQuality,
        extension: super::emTexture::ImageExtension,
    ) {
        if w <= 0.0 || h <= 0.0 { return; }
        let Some(_proof) = self.try_record(DrawOp::PaintImageScaled {
            x, y, w, h,
            image_ptr: image as *const emImage,
            quality, extension,
        }) else { return; };

        let texture = super::emTexture::emTexture::emImage {
            image: image.clone(),
            x, y, w, h,
            alpha: 255,
            extension, quality,
        };
        self.paint_rect_with_texture(x, y, w, h, &texture, self.state.canvas_color);
    }

    // --- Bezier curves ---

    /// Fill a cubic Bezier curve region (tessellated to polygon).
    /// `points` length must be a multiple of 3. Uses stride-3 convention:
    /// segment i uses points[i*3], points[i*3+1], points[i*3+2], points[((i+1)*3) % n].
    /// The path is implicitly closed.
    pub fn PaintBezier(&mut self, points: &[(f64, f64)], color: emColor, canvas_color: emColor) {
        let is_recording = self.try_record(DrawOp::PaintBezier {
            points: points.to_vec(),
            color,
            canvas_color,
        }).is_none();
        if is_recording {
            if !self.record_subops {
                return;
            }
            self.record_depth += 1;
        }
        if points.len() < 3 {
            if is_recording { self.record_depth -= 1; }
            return;
        }
        // C++ convention: n -= n%3; truncate to multiple of 3.
        let n = points.len() - points.len() % 3;
        let seg_count = n / 3;
        let s = self.state.scale_x + self.state.scale_y;
        let mut verts = Vec::new();
        for i in 0..seg_count {
            let p0 = points[i * 3];
            let p1 = points[i * 3 + 1];
            let p2 = points[i * 3 + 2];
            // P3 = first point of next segment; wraps to points[0] for last segment.
            let p3 = points[((i + 1) * 3) % n];
            tessellate_cubic_cpp(&mut verts, p0, p1, p2, p3, s, 0.0);
        }
        if verts.len() >= 3 {
            self.PaintPolygon(&verts, color, canvas_color);
        }
        if is_recording { self.record_depth -= 1; }
    }

    /// emStroke a closed Bezier path outline (tessellated to polyline, then stroked).
    /// Corresponds to C++ `PaintBezierOutline`: tessellates + strokes as closed path.
    pub fn PaintBezierOutline(
        &mut self,
        points: &[(f64, f64)],
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintBezierOutline {
            points: points.to_vec(),
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };
        if points.len() < 3 {
            return;
        }
        let n = points.len() - points.len() % 3;
        let seg_count = n / 3;
        let s = self.state.scale_x + self.state.scale_y;
        let mut verts = Vec::new();
        for i in 0..seg_count {
            let p0 = points[i * 3];
            let p1 = points[i * 3 + 1];
            let p2 = points[i * 3 + 2];
            let p3 = points[((i + 1) * 3) % n];
            tessellate_cubic_cpp(&mut verts, p0, p1, p2, p3, s, stroke.width);
        }
        if verts.len() >= 2 {
            self.PaintPolylineWithoutArrows(&verts, stroke, true, canvas_color);
        }
    }

    /// emStroke a cubic Bezier curve (tessellated to polyline).
    /// For open paths, `points` length must be 3k+1. For closed paths, 3k.
    /// Uses stride-3 convention.
    pub fn PaintBezierLine(
        &mut self,
        points: &[(f64, f64)],
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintBezierLine {
            points: points.to_vec(),
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };
        let n = points.len();
        if n < 4 {
            return;
        }
        let closed = n.is_multiple_of(3);
        let seg_count = if closed { n / 3 } else { (n - 1) / 3 };
        if seg_count == 0 {
            return;
        }
        let s = self.state.scale_x + self.state.scale_y;
        let mut verts = Vec::new();
        for i in 0..seg_count {
            let p0 = points[i * 3];
            let p1 = points[i * 3 + 1];
            let p2 = points[i * 3 + 2];
            let p3 = if closed {
                points[((i + 1) * 3) % n]
            } else {
                points[i * 3 + 3]
            };
            tessellate_cubic_cpp(&mut verts, p0, p1, p2, p3, s, stroke.width);
        }
        // For open bezier lines, add the final endpoint (t=1 of last segment).
        if !closed && !verts.is_empty() {
            verts.push(points[n - 1]);
        }
        if verts.len() < 2 {
            return;
        }

        // C++ PaintBezierLine (emPainter.cpp:1651-1684): compute arrow direction
        // vectors from the ORIGINAL control points, then pass them explicitly to
        // PaintPolylineWithArrows (matching C++ nx1,ny1,nx2,ny2 parameters).
        let arrow_dirs = if !closed
            && (stroke.start_end.IsDecorated() || stroke.finish_end.IsDecorated())
        {
            let mut nx1 = 1.0_f64;
            let mut ny1 = 0.0_f64;
            let mut nx2 = 1.0_f64;
            let mut ny2 = 0.0_f64;
            if stroke.start_end.IsDecorated() {
                for j in 1..n {
                    let dx = points[j].0 - points[0].0;
                    let dy = points[j].1 - points[0].1;
                    let ll = dx * dx + dy * dy;
                    if ll > 1e-280 {
                        let l = ll.sqrt();
                        nx1 = dx / l;
                        ny1 = dy / l;
                        break;
                    }
                }
            }
            if stroke.finish_end.IsDecorated() {
                let last = points[n - 1];
                for j in (0..n - 1).rev() {
                    let dx = points[j].0 - last.0;
                    let dy = points[j].1 - last.1;
                    let ll = dx * dx + dy * dy;
                    if ll > 1e-280 {
                        let l = ll.sqrt();
                        nx2 = dx / l;
                        ny2 = dy / l;
                        break;
                    }
                }
            }
            Some(((nx1, ny1), (nx2, ny2)))
        } else {
            None
        };

        self.PaintPolylineWithArrows(&verts, stroke, closed, canvas_color, arrow_dirs);
    }

    // --- 9-slice border images ---

    /// Compute the 9-slice boundary rectangles for a border image.
    ///
    /// `x, y, w, h` — target rectangle in logical coordinates.
    /// `l, t, r, b` — target insets (logical coordinates).
    /// `img_ox, img_oy` — source image origin offset (0 for full-image, src_x/src_y for sub-rect).
    /// `img_w, img_h` — source image region size (full image dims or sub-rect dims).
    /// `src_l, src_t, src_r, src_b` — source insets (image pixel coordinates).
    /// `canvas_color` — when not opaque, target inset boundaries are pixel-rounded.
    ///
    /// Returns `None` if `w <= 0` or `h <= 0`.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_border_image_slices(
        &self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        img_ox: i32,
        img_oy: i32,
        img_w: i32,
        img_h: i32,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        canvas_color: emColor,
    ) -> Option<BorderImageSlices> {
        if w <= 0.0 || h <= 0.0 {
            return None;
        }

        let mut l = l;
        let mut r = r;
        let mut t = t;
        let mut b = b;

        // C++ lines 1903-1908: pixel-round inset boundaries when not opaque.
        if !canvas_color.IsOpaque() {
            let f = self.RoundX(x + l) - x;
            if f > 0.0 && f < w - r { l = f; }
            let f = x + w - self.RoundX(x + w - r);
            if f > 0.0 && f < w - l { r = f; }
            let f = self.RoundY(y + t) - y;
            if f > 0.0 && f < h - b { t = f; }
            let f = y + h - self.RoundY(y + h - b);
            if f > 0.0 && f < h - t { b = f; }
        }

        let src_cx = img_w - src_l - src_r;
        let src_cy = img_h - src_t - src_b;
        let dst_cx = w - l - r;
        let dst_cy = h - t - b;

        // C++ bit layout:  8=UL 5=U 2=UR / 7=L 4=C 1=R / 6=LL 3=B 0=LR
        // Order: UL(0), U(1), UR(2), L(3), C(4), R(5), LL(6), B(7), LR(8)
        let target_rects = [
            (x,             y,             l,      t),      // UL
            (x + l,         y,             dst_cx, t),      // U
            (x + w - r,     y,             r,      t),      // UR
            (x,             y + t,         l,      dst_cy), // L
            (x + l,         y + t,         dst_cx, dst_cy), // C
            (x + w - r,     y + t,         r,      dst_cy), // R
            (x,             y + h - b,     l,      b),      // LL
            (x + l,         y + h - b,     dst_cx, b),      // B
            (x + w - r,     y + h - b,     r,      b),      // LR
        ];

        let ox = img_ox;
        let oy = img_oy;
        let source_rects = [
            (ox,                     oy,                     src_l,  src_t),  // UL
            (ox + src_l,             oy,                     src_cx, src_t),  // U
            (ox + img_w - src_r,     oy,                     src_r,  src_t),  // UR
            (ox,                     oy + src_t,             src_l,  src_cy), // L
            (ox + src_l,             oy + src_t,             src_cx, src_cy), // C
            (ox + img_w - src_r,     oy + src_t,             src_r,  src_cy), // R
            (ox,                     oy + img_h - src_b,     src_l,  src_b),  // LL
            (ox + src_l,             oy + img_h - src_b,     src_cx, src_b),  // B
            (ox + img_w - src_r,     oy + img_h - src_b,     src_r,  src_b),  // LR
        ];

        Some(BorderImageSlices {
            adj_l: l,
            adj_t: t,
            adj_r: r,
            adj_b: b,
            dst_cx,
            dst_cy,
            target_rects,
            source_rects,
        })
    }

    /// Draw a 9-slice border image stretched to fill a rectangle.
    ///
    /// `l,t,r,b` are **target** insets (logical coordinates).
    /// `src_l,src_t,src_r,src_b` are **source** insets (image pixel coordinates).
    /// `which_sub_rects` bitmask: `BORDER_EDGES_ONLY` (0o757) draws all except center.
    /// `canvas_color`: when not opaque, target inset boundaries are pixel-rounded.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintBorderImage(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        image: &emImage,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        alpha: u8,
        canvas_color: emColor,
        which_sub_rects: u16,
    ) {
        if alpha == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let is_recording = self.try_record(DrawOp::PaintBorderImage {
            x, y, w, h, l, t, r, b,
            image_ptr: image as *const emImage,
            src_l, src_t, src_r, src_b,
            alpha, canvas_color, which_sub_rects,
        }).is_none();
        if is_recording {
            if !self.record_subops { return; }
            self.record_depth += 1;
        }

        // Literal port of C++ emPainter::PaintBorderImage (emPainter.cpp:2221-2338).
        // 9 inline PaintImage calls with coordinate computation matching C++ exactly.
        let iw = image.GetWidth() as i32;
        let ih = image.GetHeight() as i32;
        let ext = super::emTexture::ImageExtension::Clamp;

        // C++ lines 2243-2248: RoundX/RoundY pixel-snap when canvas not opaque.
        let mut l = l;
        let mut t = t;
        let mut r = r;
        let mut b = b;
        if !canvas_color.IsOpaque() {
            let f = self.RoundX(x + l) - x;
            if f > 0.0 && f < w - r { l = f; }
            let f = x + w - self.RoundX(x + w - r);
            if f > 0.0 && f < w - l { r = f; }
            let f = self.RoundY(y + t) - y;
            if f > 0.0 && f < h - b { t = f; }
            let f = y + h - self.RoundY(y + h - b);
            if f > 0.0 && f < h - t { b = f; }
        }

        // C++ bit layout: 8=UL 5=U 2=UR / 7=L 4=C 1=R / 6=LL 3=B 0=LR
        let wsr = which_sub_rects;
        // UL
        if wsr & (1 << 8) != 0 {
            self.PaintImageSrcRect(x, y, l, t, image, 0, 0, src_l, src_t, alpha, canvas_color, ext);
        }
        // U
        if wsr & (1 << 5) != 0 {
            self.PaintImageSrcRect(x+l, y, w-l-r, t, image, src_l, 0, iw-src_l-src_r, src_t, alpha, canvas_color, ext);
        }
        // UR
        if wsr & (1 << 2) != 0 {
            self.PaintImageSrcRect(x+w-r, y, r, t, image, iw-src_r, 0, src_r, src_t, alpha, canvas_color, ext);
        }
        // L
        if wsr & (1 << 7) != 0 {
            self.PaintImageSrcRect(x, y+t, l, h-t-b, image, 0, src_t, src_l, ih-src_t-src_b, alpha, canvas_color, ext);
        }
        // C
        if wsr & (1 << 4) != 0 {
            self.PaintImageSrcRect(x+l, y+t, w-l-r, h-t-b, image, src_l, src_t, iw-src_l-src_r, ih-src_t-src_b, alpha, canvas_color, ext);
        }
        // R
        if wsr & (1 << 1) != 0 {
            self.PaintImageSrcRect(x+w-r, y+t, r, h-t-b, image, iw-src_r, src_t, src_r, ih-src_t-src_b, alpha, canvas_color, ext);
        }
        // LL
        if wsr & (1 << 6) != 0 {
            self.PaintImageSrcRect(x, y+h-b, l, b, image, 0, ih-src_b, src_l, src_b, alpha, canvas_color, ext);
        }
        // B
        if wsr & (1 << 3) != 0 {
            self.PaintImageSrcRect(x+l, y+h-b, w-l-r, b, image, src_l, ih-src_b, iw-src_l-src_r, src_b, alpha, canvas_color, ext);
        }
        // LR
        if wsr & (1 << 0) != 0 {
            self.PaintImageSrcRect(x+w-r, y+h-b, r, b, image, iw-src_r, ih-src_b, src_r, src_b, alpha, canvas_color, ext);
        }

        if is_recording { self.record_depth -= 1; }
    }

    /// Draw a 9-slice border image from a sub-rectangle of the source image.
    ///
    /// DIVERGED: C++ `PaintBorderImage` (overload with srcX,srcY,srcW,srcH).
    /// Rust cannot overload, so this is a separate method.
    ///
    /// `src_x,src_y,src_w,src_h` select a sub-rectangle within `image`.
    /// `src_l,src_t,src_r,src_b` are border margins within that sub-rectangle.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintBorderImageSrcRect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        image: &emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        alpha: u8,
        canvas_color: emColor,
        which_sub_rects: u16,
    ) {
        if alpha == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let Some(_proof) = self.try_record(DrawOp::PaintBorderImage {
            x, y, w, h, l, t, r, b,
            image_ptr: image as *const emImage,
            src_l, src_t, src_r, src_b,
            alpha, canvas_color, which_sub_rects,
        }) else { return; };

        // Literal port of C++ PaintBorderImage overload with srcX/Y/W/H.
        let ext = super::emTexture::ImageExtension::Clamp;

        let mut l = l;
        let mut t = t;
        let mut r = r;
        let mut b = b;
        if !canvas_color.IsOpaque() {
            let f = self.RoundX(x + l) - x;
            if f > 0.0 && f < w - r { l = f; }
            let f = x + w - self.RoundX(x + w - r);
            if f > 0.0 && f < w - l { r = f; }
            let f = self.RoundY(y + t) - y;
            if f > 0.0 && f < h - b { t = f; }
            let f = y + h - self.RoundY(y + h - b);
            if f > 0.0 && f < h - t { b = f; }
        }

        let wsr = which_sub_rects;
        let sx = src_x;
        let sy = src_y;
        let sw = src_w;
        let sh = src_h;
        let sl = src_l;
        let st = src_t;
        let sr = src_r;
        let sb = src_b;

        if wsr & (1<<8) != 0 { self.PaintImageSrcRect(x,y,l,t, image, sx,sy,sl,st, alpha,canvas_color,ext); }
        if wsr & (1<<5) != 0 { self.PaintImageSrcRect(x+l,y,w-l-r,t, image, sx+sl,sy,sw-sl-sr,st, alpha,canvas_color,ext); }
        if wsr & (1<<2) != 0 { self.PaintImageSrcRect(x+w-r,y,r,t, image, sx+sw-sr,sy,sr,st, alpha,canvas_color,ext); }
        if wsr & (1<<7) != 0 { self.PaintImageSrcRect(x,y+t,l,h-t-b, image, sx,sy+st,sl,sh-st-sb, alpha,canvas_color,ext); }
        if wsr & (1<<4) != 0 { self.PaintImageSrcRect(x+l,y+t,w-l-r,h-t-b, image, sx+sl,sy+st,sw-sl-sr,sh-st-sb, alpha,canvas_color,ext); }
        if wsr & (1<<1) != 0 { self.PaintImageSrcRect(x+w-r,y+t,r,h-t-b, image, sx+sw-sr,sy+st,sr,sh-st-sb, alpha,canvas_color,ext); }
        if wsr & (1<<6) != 0 { self.PaintImageSrcRect(x,y+h-b,l,b, image, sx,sy+sh-sb,sl,sb, alpha,canvas_color,ext); }
        if wsr & (1<<3) != 0 { self.PaintImageSrcRect(x+l,y+h-b,w-l-r,b, image, sx+sl,sy+sh-sb,sw-sl-sr,sb, alpha,canvas_color,ext); }
        if wsr & (1<<0) != 0 { self.PaintImageSrcRect(x+w-r,y+h-b,r,b, image, sx+sw-sr,sy+sh-sb,sr,sb, alpha,canvas_color,ext); }
    }

    /// 9-slice border image with two-color tinting.
    /// Matches C++ PaintBorderImageColored (emPainter.cpp:2341-2461).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintBorderImageColored(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        mut l: f64,
        mut t: f64,
        mut r: f64,
        mut b: f64,
        image: &emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        which_sub_rects: i32,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintBorderImageColored {
            x, y, w, h, l, t, r, b,
            image_ptr: image as *const emImage,
            src_l, src_t, src_r, src_b,
            color1, color2, canvas_color,
            which_sub_rects: which_sub_rects as u16,
            alpha: 255,
        }) else { return; };

        if !canvas_color.IsOpaque() {
            let f = self.RoundX(x + l) - x;
            if f > 0.0 && f < w - r { l = f; }
            let f = x + w - self.RoundX(x + w - r);
            if f > 0.0 && f < w - l { r = f; }
            let f = self.RoundY(y + t) - y;
            if f > 0.0 && f < h - b { t = f; }
            let f = y + h - self.RoundY(y + h - b);
            if f > 0.0 && f < h - t { b = f; }
        }

        let sx = src_x as u32;
        let sy = src_y as u32;
        let sw = src_w;
        let sh = src_h;
        let sl = src_l;
        let st = src_t;
        let sr = src_r;
        let sb = src_b;

        if which_sub_rects & (1 << 8) != 0 {
            self.PaintImageColored(x, y, l, t, image,
                sx, sy, sl as u32, st as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 5) != 0 {
            self.PaintImageColored(x + l, y, w - l - r, t, image,
                sx + sl as u32, sy, (sw - sl - sr) as u32, st as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 2) != 0 {
            self.PaintImageColored(x + w - r, y, r, t, image,
                sx + (sw - sr) as u32, sy, sr as u32, st as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 7) != 0 {
            self.PaintImageColored(x, y + t, l, h - t - b, image,
                sx, sy + st as u32, sl as u32, (sh - st - sb) as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 4) != 0 {
            self.PaintImageColored(x + l, y + t, w - l - r, h - t - b, image,
                sx + sl as u32, sy + st as u32, (sw - sl - sr) as u32, (sh - st - sb) as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 1) != 0 {
            self.PaintImageColored(x + w - r, y + t, r, h - t - b, image,
                sx + (sw - sr) as u32, sy + st as u32, sr as u32, (sh - st - sb) as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 6) != 0 {
            self.PaintImageColored(x, y + h - b, l, b, image,
                sx, sy + (sh - sb) as u32, sl as u32, sb as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 3) != 0 {
            self.PaintImageColored(x + l, y + h - b, w - l - r, b, image,
                sx + sl as u32, sy + (sh - sb) as u32, (sw - sl - sr) as u32, sb as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
        if which_sub_rects & (1 << 0) != 0 {
            self.PaintImageColored(x + w - r, y + h - b, r, b, image,
                sx + (sw - sr) as u32, sy + (sh - sb) as u32, sr as u32, sb as u32,
                color1, color2, canvas_color, ImageExtension::Clamp);
        }
    }

    // --- Ellipse/sector outline utilities ---

    /// emStroke an arc of an ellipse (no radii, just the curved portion).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintEllipseArc(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        range_angle: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintEllipseArc {
            cx,
            cy,
            rx,
            ry,
            start_angle,
            range_angle,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };
        if rx <= 0.0 || ry <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        if range_angle == 0.0 {
            return;
        }
        let abs_range = range_angle.abs();
        if abs_range >= 2.0 * std::f64::consts::PI {
            self.PaintEllipseOutline(cx, cy, rx, ry, stroke, canvas_color);
            return;
        }
        // C++ includes half-thickness in quality (emPainter.cpp:1759)
        let t2 = stroke.width * 0.5;
        let mut f = CIRCLE_QUALITY
            * ((rx + t2) * self.state.scale_x + (ry + t2) * self.state.scale_y).sqrt();
        if f > 256.0 { f = 256.0; }
        f = f * abs_range / (2.0 * std::f64::consts::PI);
        let n = if f <= 3.0 { 3 } else if f >= 256.0 { 256 } else { (f + 0.5) as usize };
        let step = range_angle / n as f64;
        let vn = n + 1;
        let mut verts = Vec::with_capacity(vn);
        for i in 0..vn {
            let angle = start_angle + step * i as f64;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }

        // C++ line 1804: if (w < thickness || h < thickness) canvasColor = 0;
        let canvas_color = if 2.0 * rx < stroke.width || 2.0 * ry < stroke.width {
            emColor::TRANSPARENT
        } else {
            canvas_color
        };

        // C++ computes exact ellipse tangent directions for arrow rendering
        // (emPainter.cpp:1775-1801) instead of deriving them from polyline vertices.
        let with_arrows = stroke.start_end.IsDecorated() || stroke.finish_end.IsDecorated();
        if with_arrows {
            let compute_normal = |angle: f64, forward: bool| -> (f64, f64) {
                let (mut nx, mut ny) = if forward {
                    (-angle.sin(), angle.cos())
                } else {
                    (angle.sin(), -angle.cos())
                };
                if range_angle < 0.0 { nx = -nx; ny = -ny; }
                let tnx = nx * rx;
                let tny = ny * ry;
                let ll = tnx * tnx + tny * tny;
                if ll > 1e-280 {
                    let l = ll.sqrt();
                    (tnx / l, tny / l)
                } else {
                    (nx, ny)
                }
            };

            let n1 = compute_normal(start_angle, true);
            let n2 = compute_normal(start_angle + range_angle, false);

            self.PaintPolylineWithArrows(&verts, stroke, false, canvas_color, Some((n1, n2)));
        } else {
            self.PaintPolylineWithoutArrows(&verts, stroke, false, canvas_color);
        }
    }

    /// Outline an ellipse sector. Angles in **degrees** (start + sweep).
    /// Matches C++ `PaintEllipseSectorOutline` (emPainter.cpp:1997-2077).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintEllipseSectorOutline(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        sweep_angle: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintEllipseSectorOutline {
            cx,
            cy,
            rx,
            ry,
            start_angle,
            sweep_angle,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };

        let thickness = stroke.width;

        // C++ converts degrees to radians, normalizes negative range.
        let mut start_rad = start_angle * std::f64::consts::PI / 180.0;
        let mut range_rad = sweep_angle * std::f64::consts::PI / 180.0;
        if range_rad <= 0.0 {
            if range_rad == 0.0 { return; }
            start_rad += range_rad;
            range_rad = -range_rad;
        }
        if range_rad >= 2.0 * std::f64::consts::PI {
            self.PaintEllipseOutline(cx, cy, rx, ry, stroke, canvas_color);
            return;
        }
        if thickness <= 0.0 { return; }
        let rx = rx.max(0.0);
        let ry = ry.max(0.0);

        // C++ computes n from outer radii (rx+t2, ry+t2), scaled by sweep.
        let t2 = thickness * 0.5;
        let mut f = CIRCLE_QUALITY
            * ((rx + t2) * self.state.scale_x + (ry + t2) * self.state.scale_y).sqrt();
        if f > 256.0 { f = 256.0; }
        f = f * range_rad / (2.0 * std::f64::consts::PI);
        let n: usize = if f <= 3.0 { 3 } else if f >= 256.0 { 256 } else { (f + 0.5) as usize };
        let step = range_rad / n as f64;

        // Center + n+1 arc points = n+2 total vertices.
        let mut verts = Vec::with_capacity(n + 2);
        verts.push((cx, cy));
        for i in 0..=n {
            let angle = start_rad + step * i as f64;
            verts.push((angle.cos() * rx + cx, angle.sin() * ry + cy));
        }

        // C++ line 2072: canvasColor=0 for thin sectors.
        let canvas_color = if 2.0 * rx < thickness || 2.0 * ry < thickness {
            emColor::TRANSPARENT
        } else {
            canvas_color
        };

        // C++ always uses PaintPolylineWithoutArrows (handles solid+dashed).
        self.PaintPolylineWithoutArrows(&verts, stroke, false, canvas_color);
    }

    /// Draw a rectangle outline. emStroke is centered on the shape boundary.
    ///
    /// Matches C++ `PaintRectOutline`: for solid non-rounded strokes, builds a
    /// 10-vertex polygon (outer rect + bridge + reversed inner rect). For
    /// dashed/rounded strokes, routes through `PaintPolylineWithoutArrows`.
    pub fn PaintRectOutline(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintRectOutline {
            x,
            y,
            w,
            h,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };
        let sw = stroke.width;
        let w = w.max(0.0);
        let h = h.max(0.0);
        if sw <= 0.0 {
            return;
        }
        let t2 = sw * 0.5;
        let rounded = stroke.join == super::emStroke::LineJoin::Round
            || stroke.cap == super::emStroke::LineCap::Round;

        if rounded || stroke.is_dashed() {
            if (w <= sw || h <= sw) && !stroke.is_dashed() {
                self.PaintRoundRect(x - t2, y - t2, w + sw, h + sw, t2, t2, stroke.color, canvas_color);
                return;
            }
            let verts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
            self.PaintPolylineWithoutArrows(&verts, stroke, true, canvas_color);
            return;
        }

        // Outer rect expanded by t2 on each side.
        let ox1 = x - t2;
        let oy1 = y - t2;
        let ox2 = x + w + t2;
        let oy2 = y + h + t2;
        // Inner rect contracted by t2 from shape boundary.
        let ix1 = ox1 + sw;
        let iy1 = oy1 + sw;
        let ix2 = ox2 - sw;
        let iy2 = oy2 - sw;

        if ix1 >= ix2 || iy1 >= iy2 {
            // emStroke fills entire rect.
            self.PaintPolygon(
                &[(ox1, oy1), (ox2, oy1), (ox2, oy2), (ox1, oy2)],
                stroke.color,
                canvas_color,
            );
            return;
        }

        // 10-vertex polygon: outer CW, bridge, inner CCW, bridge back.
        // Must set canvas_color in state like PaintPolygon does, since
        // fill_polygon_aa → blit_span uses self.state.canvas_color for blending.
        let poly = [
            (ox1, oy1),
            (ox2, oy1),
            (ox2, oy2),
            (ox1, oy2),
            (ox1, oy1), // bridge back to start
            (ix1, iy1), // inner start
            (ix1, iy2),
            (ix2, iy2),
            (ix2, iy1),
            (ix1, iy1), // close inner
        ];
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        self.fill_polygon_aa(proof, &poly, stroke.color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a rounded rectangle outline. emStroke is centered on the shape boundary.
    /// Matches C++ `PaintRoundRectOutline` (emPainter.cpp:2080-2218).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintRoundRectOutline(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rx: f64,
        ry: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let thickness = stroke.width;

        let Some(_proof) = self.try_record(DrawOp::PaintRoundRectOutline {
            x,
            y,
            w,
            h,
            rx,
            ry,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };

        if thickness <= 0.0 { return; }
        let w = w.max(0.0);
        let h = h.max(0.0);
        let t2 = thickness * 0.5;

        let mut rx = rx;
        let mut ry = ry;
        if rx > w * 0.5 { rx = w * 0.5; }
        if ry > h * 0.5 { ry = h * 0.5; }
        if rx <= 0.0 || ry <= 0.0 {
            self.PaintRectOutline(x, y, w, h, stroke, canvas_color);
            return;
        }

        rx += t2;
        ry += t2;
        let mut f = CIRCLE_QUALITY * (rx * self.state.scale_x + ry * self.state.scale_y).sqrt();
        if f > 256.0 { f = 256.0; }
        f *= 0.25;
        let n: usize = if f <= 1.0 { 1 } else if f >= 64.0 { 64 } else { (f + 0.5) as usize };
        let step = std::f64::consts::FRAC_PI_2 / n as f64;

        // Corner centers (from outer bounding box edges + outer radii).
        let mut x1 = x - t2 + rx;
        let mut y1 = y - t2 + ry;
        let mut x2 = x + w + t2 - rx;
        let mut y2 = y + h + t2 - ry;

        if stroke.is_dashed() {
            rx -= t2;
            ry -= t2;
            let mut verts = vec![(0.0, 0.0); 4 * (n + 1)];
            for i in 0..=n {
                let dx = (step * i as f64).cos();
                let dy = (step * i as f64).sin();
                verts[i] = (x1 - dx * rx, y1 - dy * ry);
                verts[n + 1 + i] = (x2 + dy * rx, y1 - dx * ry);
                verts[2 * n + 2 + i] = (x2 + dx * rx, y2 + dy * ry);
                verts[3 * n + 3 + i] = (x1 - dy * rx, y2 + dx * ry);
            }
            let canvas_color = if w < thickness || h < thickness {
                emColor::TRANSPARENT
            } else {
                canvas_color
            };
            self.PaintPolylineWithoutArrows(&verts, stroke, true, canvas_color);
            return;
        }

        // Solid stroke: build outer vertices.
        let outer_count = 4 * (n + 1);
        let mut verts = vec![(0.0, 0.0); outer_count];
        for i in 0..=n {
            let dx = (step * i as f64).cos();
            let dy = (step * i as f64).sin();
            verts[i] = (x1 - dx * rx, y1 - dy * ry);
            verts[n + 1 + i] = (x2 + dy * rx, y1 - dx * ry);
            verts[2 * n + 2 + i] = (x2 + dx * rx, y2 + dy * ry);
            verts[3 * n + 3 + i] = (x1 - dy * rx, y2 + dx * ry);
        }

        rx -= thickness;
        ry -= thickness;
        if rx < 0.0 {
            x1 -= rx;
            x2 += rx;
            rx = 0.0;
        }
        if ry < 0.0 {
            y1 -= ry;
            y2 += ry;
            ry = 0.0;
        }

        if x1 - rx >= x2 + rx || y1 - ry >= y2 + ry {
            // Degenerate inner — fill as solid polygon.
            self.PaintPolygon(&verts, stroke.color, canvas_color);
            return;
        }

        // Bridge from outer end back to outer start.
        let outer_start = verts[0];
        verts.push(outer_start);

        // Inner ring with potentially different segment count.
        let mut f = CIRCLE_QUALITY * (rx * self.state.scale_x + ry * self.state.scale_y).sqrt();
        if f > 256.0 { f = 256.0; }
        f *= 0.25;
        let m: usize = if f <= 1.0 { 1 } else if f >= 64.0 { 64 } else { (f + 0.5) as usize };
        let inner_step = std::f64::consts::FRAC_PI_2 / m as f64;

        let final_count = 4 * n + 4 * m + 10;
        verts.resize(final_count, (0.0, 0.0));

        for i in 0..=m {
            let dx = (inner_step * i as f64).cos();
            let dy = (inner_step * i as f64).sin();
            verts[4 * n + 4 * m + 9 - i] = (x1 - dx * rx, y1 - dy * ry);
            verts[4 * n + 3 * m + 8 - i] = (x2 + dy * rx, y1 - dx * ry);
            verts[4 * n + 2 * m + 7 - i] = (x2 + dx * rx, y2 + dy * ry);
            verts[4 * n + m + 6 - i] = (x1 - dy * rx, y2 + dx * ry);
        }

        // Inner start repeated.
        verts[4 * n + 5] = verts[4 * n + 4 * m + 9];

        self.PaintPolygon(&verts, stroke.color, canvas_color);
    }

    /// Draw an ellipse outline. emStroke is centered on the shape boundary.
    /// Matches C++ `PaintEllipseOutline` (emPainter.cpp:1901-1994).
    pub fn PaintEllipseOutline(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintEllipseOutline {
            cx,
            cy,
            rx,
            ry,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };

        let thickness = stroke.width;
        if thickness <= 0.0 { return; }
        let rx = rx.max(0.0);
        let ry = ry.max(0.0);
        let t2 = thickness * 0.5;

        // C++ computes outer radii: rx = w/2+t2, ry = h/2+t2.
        // In our (cx,cy,rx,ry) API, rx/ry are shape radii (= w/2, h/2).
        let orx = rx + t2;
        let ory = ry + t2;

        // C++ computes segment count from OUTER radii.
        let n = adaptive_circle_segments(orx, ory, self.state.scale_x, self.state.scale_y);
        let step = 2.0 * std::f64::consts::PI / n as f64;

        if stroke.is_dashed() {
            // Centerline vertices (rx, ry) but using outer-derived segment count n.
            let mut verts = Vec::with_capacity(n);
            for i in 0..n {
                let angle = step * i as f64;
                verts.push((angle.cos() * rx + cx, angle.sin() * ry + cy));
            }
            let canvas_color = if 2.0 * rx < thickness || 2.0 * ry < thickness {
                emColor::TRANSPARENT
            } else {
                canvas_color
            };
            self.PaintPolylineWithoutArrows(&verts, stroke, false, canvas_color);
            return;
        }

        // Solid: outer vertices using outer radii.
        let mut verts = Vec::with_capacity(n);
        for i in 0..n {
            let angle = step * i as f64;
            verts.push((angle.cos() * orx + cx, angle.sin() * ory + cy));
        }

        // Inner radii.
        let irx = orx - thickness;
        let iry = ory - thickness;
        if irx <= 0.0 || iry <= 0.0 {
            // Degenerate inner — fill outer as solid polygon.
            self.PaintPolygon(&verts, stroke.color, canvas_color);
            return;
        }

        // Bridge from outer end to outer start.
        verts.push(verts[0]);

        // Inner ring with potentially different segment count.
        let m = adaptive_circle_segments(irx, iry, self.state.scale_x, self.state.scale_y);
        let inner_step = 2.0 * std::f64::consts::PI / m as f64;

        let final_count = n + m + 2;
        verts.resize(final_count, (0.0, 0.0));

        // C++ inner vertices in reverse order: xy[n+m+1-i] for i=0..m-1.
        for i in 0..m {
            let angle = inner_step * i as f64;
            verts[n + m + 1 - i] = (angle.cos() * irx + cx, angle.sin() * iry + cy);
        }

        // Inner start repeated.
        verts[n + 1] = verts[n + m + 1];

        self.PaintPolygon(&verts, stroke.color, canvas_color);
    }

    /// Correct blending artifacts along a shared edge between two adjacent polygons.
    /// C++ emPainter::PaintEdgeCorrection (emPainter.cpp:803-997).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintEdgeCorrection(
        &mut self,
        mut x1: f64, mut y1: f64,
        mut x2: f64, mut y2: f64,
        mut color1: emColor, mut color2: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintEdgeCorrection {
            x1, y1, x2, y2, color1, color2,
        }) else { return; };

        x1 = x1 * self.state.scale_x + self.state.offset_x;
        y1 = y1 * self.state.scale_y + self.state.offset_y;
        x2 = x2 * self.state.scale_x + self.state.offset_x;
        y2 = y2 * self.state.scale_y + self.state.offset_y;

        if y1 > y2 {
            let t = y1; y1 = y2; y2 = t;
            let t = x1; x1 = x2; x2 = t;
            let tc = color1; color1 = color2; color2 = tc;
        }

        let dx = x2 - x1;
        let dy = y2 - y1;
        let adx = dx.abs();
        let gx = if dy >= 0.0001 { dx / dy } else { 0.0 };
        let gy = if adx >= 0.0001 { dy / dx } else { 0.0 };

        if y1 < self.state.clip.y1 {
            if y2 <= self.state.clip.y1 { return; }
            x1 += (self.state.clip.y1 - y1) * gx;
            y1 = self.state.clip.y1;
        }
        if y2 > self.state.clip.y2 {
            if y1 >= self.state.clip.y2 { return; }
            x2 += (self.state.clip.y2 - y2) * gx;
            y2 = self.state.clip.y2;
        }

        let mut sx: i32;
        let mut cx1: f64;
        let mut cx2: f64;
        if dx >= 0.0 {
            if x1 < self.state.clip.x1 {
                if x2 <= self.state.clip.x1 { return; }
                y1 += (self.state.clip.x1 - x1) * gy;
                x1 = self.state.clip.x1;
            }
            if x2 > self.state.clip.x2 {
                if x1 >= self.state.clip.x2 { return; }
                y2 += (self.state.clip.x2 - x2) * gy;
                x2 = self.state.clip.x2;
            }
            sx = x1 as i32;
            cx1 = x1;
            cx2 = x2;
        } else {
            if x2 < self.state.clip.x1 {
                if x1 <= self.state.clip.x1 { return; }
                y2 += (self.state.clip.x1 - x2) * gy;
                x2 = self.state.clip.x1;
            }
            if x1 > self.state.clip.x2 {
                if x2 >= self.state.clip.x2 { return; }
                y1 += (self.state.clip.x2 - x1) * gy;
                x1 = self.state.clip.x2;
            }
            sx = x1.ceil() as i32 - 1;
            cx1 = x2;
            cx2 = x1;
        }
        let mut sy = y1 as i32;
        let mut cy1 = y1;
        let mut cy2 = y2;
        if adx > dy {
            cy1 = cy1.floor();
            cy2 = cy2.ceil();
        } else {
            cx1 = cx1.floor();
            cx2 = cx2.ceil();
        }

        if color1.IsTotallyTransparent() || color2.IsTotallyTransparent() { return; }

        let ac1 = color1.GetAlpha() as f64 * (1.0 / 255.0);
        let ac2 = color2.GetAlpha() as f64 * (1.0 / 255.0);

        let h1 = [color1.GetRed(), color1.GetGreen(), color1.GetBlue()];
        let h2 = [color2.GetRed(), color2.GetGreen(), color2.GetBlue()];

        let tw = self.target_width as i32;
        let th = self.target_height as i32;

        loop {
            let mut px1 = sx as f64;
            let mut py1 = sy as f64;
            let mut px2 = px1 + 1.0;
            let mut py2 = py1 + 1.0;
            if px1 < cx1 { px1 = cx1; }
            if py1 < cy1 { py1 = cy1; }
            if px2 > cx2 { px2 = cx2; }
            if py2 > cy2 { py2 = cy2; }
            let mut qx1 = x1;
            let mut qy1 = y1;
            let mut qx2 = x2;
            let mut qy2 = y2;
            if qy1 < py1 { qx1 += (py1 - qy1) * gx; qy1 = py1; }
            if qy2 > py2 { qx2 += (py2 - qy2) * gx; qy2 = py2; }
            let mut a2: f64;
            if dx >= 0.0 {
                if qx1 < px1 { qy1 += (px1 - qx1) * gy; qx1 = px1; }
                if qx2 > px2 { qy2 += (px2 - qx2) * gy; qx2 = px2; }
                a2 = py2 - qy2;
            } else {
                if qx2 < px1 { qy2 += (px1 - qx2) * gy; qx2 = px1; }
                if qx1 > px2 { qy1 += (px2 - qx1) * gy; qx1 = px2; }
                a2 = qy1 - py1;
            }
            a2 = a2 * (px2 - px1) + (qy2 - qy1) * ((qx1 + qx2) * 0.5 - px1);
            let a1 = (py2 - py1) * (px2 - px1) - a2;
            let a1 = a1 * ac1;
            let a2 = a2 * ac2;
            if a1 >= 0.001 && a2 >= 0.001 {
                let t = 255.0 / ((1.0 - a1) * (1.0 - a2));
                let alpha1 = (a1 * a2 * (1.0 - a2) * t) as i32;
                let alpha2 = (a1 * a2 * a2 * t) as i32;
                let alpha3 = ((1.0 - a1 - a2) * t) as i32;

                if sx >= 0 && sx < tw && sy >= 0 && sy < th {
                    let bg = self.read_pixel(proof, sx as u32, sy as u32);
                    let out = self.GetImage(proof).SetPixel(sx as u32, sy as u32);
                    for ch in 0..3 {
                        if alpha3 > 0 {
                            let bg_ch = bg[ch] as i32;
                            let bg_term = (bg_ch * alpha3 + 127) / 255;
                            let c1_term = blend_hash_lookup(h1[ch], alpha1 as u8) as i32;
                            let c2_term = blend_hash_lookup(h2[ch], alpha2 as u8) as i32;
                            out[ch] = (bg_term + c1_term + c2_term).clamp(0, 255) as u8;
                        } else {
                            let c1_term = blend_hash_lookup(h1[ch], alpha1 as u8) as i32;
                            let c2_term = blend_hash_lookup(h2[ch], alpha2 as u8) as i32;
                            out[ch] = (c1_term + c2_term).clamp(0, 255) as u8;
                        }
                    }
                }
            }
            if dx >= 0.0 {
                if ((sy + 1) as f64 - y1) * dx > ((sx + 1) as f64 - x1) * dy {
                    sx += 1;
                    if (sx as f64) < cx2 { continue; }
                    break;
                }
            } else {
                if ((sy + 1) as f64 - y1) * dx < (sx as f64 - x1) * dy {
                    sx -= 1;
                    if sx as f64 + 1.0 > cx1 { continue; }
                    break;
                }
            }
            sy += 1;
            if sy as f64 >= cy2 { break; }
        }
    }

    /// Fill the current clip rect with a solid color.
    pub fn Clear(&mut self, color: emColor) {
        let Some(proof) = self.require_direct() else {
            return;
        };
        let x = self.state.clip.x1 as i32;
        let y = self.state.clip.y1 as i32;
        let w = self.state.clip.x2.ceil() as i32 - x;
        let h = self.state.clip.y2.ceil() as i32 - y;
        self.fill_rect_pixels(proof, x, y, w, h, color);
    }

    /// Draw a dashed polyline by splitting the path into dash/gap segments
    /// and painting each dash as a solid sub-polyline.
    /// C++ emPainter.cpp:3451-3708 — PaintDashedPolyline.
    pub fn PaintDashedPolyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintDashedPolyline {
            vertices: vertices.to_vec(),
            stroke: stroke.clone(),
            closed,
            canvas_color,
        }) else { return; };
        use crate::emStroke::DashType;

        let n = vertices.len();
        if n < 2 {
            self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
            return;
        }

        let thickness = stroke.width;
        let rounded = stroke.cap == super::emStroke::LineCap::Round;
        let have_dashes = stroke.dash_type != DashType::Dotted;
        let have_dots = stroke.dash_type != DashType::Dashed;
        let have_dashes_and_dots = have_dashes && have_dots;
        let is_endless = closed;

        let min_dash_len: f64;
        let pref_dash_len: f64;
        if have_dashes {
            min_dash_len = thickness * if rounded { 1.0 + MIN_REL_SEG_LEN } else { MIN_REL_SEG_LEN };
            pref_dash_len = min_dash_len.max(thickness * 5.0 * stroke.dash_length_factor);
        } else {
            min_dash_len = 0.0;
            pref_dash_len = 0.0;
        }
        let mut dot_len = if have_dots { thickness * (1.0 + MIN_REL_SEG_LEN) } else { 0.0 };
        let pref_gap_len = 0.0f64.max(thickness * 5.0 * stroke.gap_length_factor);
        let min_phase_len = min_dash_len + dot_len;
        let pref_phase_len = pref_dash_len + dot_len + pref_gap_len;

        let num_edges: usize = if is_endless { n } else { n - 1 };
        let mut total_len = 0.0f64;
        let mut x2 = vertices[0].0;
        let mut y2 = vertices[0].1;
        for i in 1..=num_edges {
            let x1 = x2;
            let y1 = y2;
            x2 = vertices[i % n].0;
            y2 = vertices[i % n].1;
            let dx = x2 - x1;
            let dy = y2 - y1;
            total_len += (dx * dx + dy * dy).sqrt();
        }

        let stroke_count: i32;
        let mut dash_len: f64;
        let mut gap_len: f64;
        let mut end_extra: f64;

        if is_endless {
            let max_stroke_count = (MAX_DASHES.min(total_len / min_phase_len)) as i32;
            if max_stroke_count < 1 {
                self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
                return;
            }
            stroke_count = ((MAX_DASHES.min(total_len / pref_phase_len + 0.5)) as i32)
                .max(1).min(max_stroke_count);
            end_extra = 0.0;
            let t = total_len / stroke_count as f64 - dot_len;
            dash_len = min_dash_len.max(t / (pref_phase_len - dot_len) * pref_dash_len);
            gap_len = t - dash_len;
        } else {
            let mut t = total_len;
            if have_dashes { t += thickness.min(min_dash_len); } else { t += thickness; }
            if have_dashes_and_dots { t += dot_len; }
            let max_stroke_count = (MAX_DASHES.min(t / min_phase_len)) as i32;
            if max_stroke_count < 2 {
                self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
                return;
            }
            t = total_len + pref_gap_len;
            if have_dashes { t += thickness.min(pref_dash_len); } else { t += thickness; }
            if have_dashes_and_dots { t += dot_len; }
            stroke_count = ((MAX_DASHES.min(t / pref_phase_len + 0.5)) as i32)
                .max(2).min(max_stroke_count);
            end_extra = thickness;
            if have_dashes {
                t = total_len + end_extra;
                if have_dots { t -= (stroke_count - 1) as f64 * dot_len; }
                let u = stroke_count as f64 * pref_dash_len
                    + (stroke_count - 1) as f64 * pref_gap_len;
                dash_len = min_dash_len.max(t / u * pref_dash_len);
                if dash_len < end_extra {
                    let t2 = t - end_extra;
                    let u2 = u - pref_dash_len;
                    dash_len = min_dash_len.max(t2 / u2 * pref_dash_len);
                    end_extra = dash_len;
                }
            } else {
                dash_len = 0.0;
            }
            t = total_len + end_extra - stroke_count as f64 * (dash_len + dot_len);
            if have_dashes_and_dots { t += dot_len; }
            gap_len = t / (stroke_count - 1) as f64;
            end_extra *= 0.5;
        }

        // Gap too small at screen scale → render as solid with alpha.
        let mut t = gap_len;
        if rounded { t += thickness * 0.215; }
        if t * (self.GetScaleX() + self.GetScaleY()) * 0.5 < 1.2 {
            let phase_len = dash_len + dot_len + gap_len;
            let t_solid = ((phase_len - t) / phase_len).clamp(0.0, 1.0);
            if t_solid <= 0.0 { return; }
            let mut stroke2 = stroke.clone();
            stroke2.color = stroke2.color.SetAlpha(
                (stroke.color.GetAlpha() as f64 * t_solid + 0.5) as u8
            );
            self.PaintSolidPolyline(vertices, &stroke2, closed, canvas_color);
            return;
        }

        let mut stroke_count = stroke_count;
        if have_dashes_and_dots {
            gap_len *= 0.5;
            stroke_count *= 2;
            if !is_endless { stroke_count -= 1; }
        }

        if rounded {
            end_extra = 0.0;
            if have_dashes { dash_len -= thickness; }
            if have_dots { dot_len -= thickness; }
            gap_len += thickness;
        }

        // Clip rect in logical coords, expanded by max radius.
        let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
        let butt_end = emStrokeEnd::butt();
        let r = emPainter::CalculateLinePointMinMaxRadius(thickness, stroke, &cap_end, &cap_end);
        let cx1 = (self.GetClipX1() - self.GetOriginX()) / self.GetScaleX() - r;
        let cy1 = (self.GetClipY1() - self.GetOriginY()) / self.GetScaleY() - r;
        let cx2 = (self.GetClipX2() - self.GetOriginX()) / self.GetScaleX() + r;
        let cy2 = (self.GetClipY2() - self.GetOriginY()) / self.GetScaleY() + r;

        let mut is_in_stroke = false;
        let mut end_of_stroke_reached = false;
        let mut stroke_number: i32 = 1;
        let mut remaining_segment_len = 0.0f64;
        let mut remaining_edge_len = 0.0f64;
        let mut i: i32 = 0;
        x2 = vertices[0].0;
        y2 = vertices[0].1;
        let mut nx = 1.0f64;
        let mut ny = 0.0f64;
        let mut min_x = 0.0f64;
        let mut max_x = 0.0f64;
        let mut min_y = 0.0f64;
        let mut max_y = 0.0f64;
        let mut xy_out: Vec<(f64, f64)> = Vec::new();

        let (mut x1, mut y1) = if is_endless {
            (vertices[n - 1].0, vertices[n - 1].1)
        } else {
            (x2, y2)
        };

        if is_endless {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let ll = dx * dx + dy * dy;
            if ll > 1e-280 {
                let l = ll.sqrt();
                remaining_edge_len =
                    l.min((if have_dashes { dash_len } else { dot_len }) * 0.5);
                nx = dx / l;
                ny = dy / l;
                i -= 1;
            }
        }

        loop {
            while remaining_edge_len <= 1e-140 && i < num_edges as i32 {
                i += 1;
                x1 = x2;
                y1 = y2;
                x2 = vertices[i as usize % n].0;
                y2 = vertices[i as usize % n].1;
                let dx = x2 - x1;
                let dy = y2 - y1;
                let ll = dx * dx + dy * dy;
                let l = ll.sqrt();
                remaining_edge_len += l;
                if ll > 1e-280 {
                    nx = dx / l;
                    ny = dy / l;
                }
            }

            if remaining_segment_len < remaining_edge_len {
                remaining_edge_len -= remaining_segment_len;
                remaining_segment_len = 0.0;
                end_of_stroke_reached = true;
            } else {
                remaining_segment_len -= remaining_edge_len;
                remaining_edge_len = 0.0;
                if i >= num_edges as i32 {
                    if !is_in_stroke { break; }
                    end_of_stroke_reached = true;
                } else if !is_in_stroke {
                    continue;
                }
            }

            let x = x2 - nx * remaining_edge_len;
            let y = y2 - ny * remaining_edge_len;
            if xy_out.is_empty() {
                min_x = x; max_x = x;
                min_y = y; max_y = y;
            } else {
                if min_x > x { min_x = x; } else if max_x < x { max_x = x; }
                if min_y > y { min_y = y; } else if max_y < y { max_y = y; }
            }
            xy_out.push((x, y));

            if !is_in_stroke {
                is_in_stroke = true;
                end_of_stroke_reached = false;
                remaining_segment_len =
                    if have_dashes && (!have_dots || (stroke_number & 1) != 0) {
                        dash_len
                    } else {
                        dot_len
                    };
                if stroke_number == 1 { remaining_segment_len -= end_extra; }
                continue;
            }

            if !end_of_stroke_reached { continue; }

            if min_x < cx2 && min_y < cy2 && max_x > cx1 && max_y > cy1 {
                let start = if !is_endless && stroke_number == 1 {
                    stroke.start_end
                } else if rounded { cap_end } else { butt_end };
                let end = if !is_endless && stroke_number == stroke_count {
                    stroke.finish_end
                } else if rounded { cap_end } else { butt_end };
                let mut solid_stroke = stroke.clone();
                solid_stroke.dash_type = DashType::Solid;
                solid_stroke.dash_pattern.clear();
                solid_stroke.start_end = start;
                solid_stroke.finish_end = end;
                self.PaintSolidPolyline(&xy_out, &solid_stroke, false, canvas_color);
            }

            if stroke_number >= stroke_count { break; }
            stroke_number += 1;
            is_in_stroke = false;
            remaining_segment_len = gap_len;
            xy_out.clear();
        }
    }

    /// Dispatch polyline rendering: if dashed call dashed, else call solid.
    /// Corresponds to C++ `PaintPolylineWithoutArrows` (inline in emPainter.h,
    /// does not log or manage depth — pure dispatch).
    pub fn PaintPolylineWithoutArrows(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
    ) {
        if stroke.is_dashed() {
            self.PaintDashedPolyline(vertices, stroke, closed, canvas_color);
        } else {
            self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
        }
    }

    /// Dispatch polyline rendering with arrow support.
    /// Corresponds to C++ `PaintPolylineWithArrows` (does not log or manage
    /// depth — internal dispatch called by `PaintPolyline`).
    ///
    /// `arrow_dirs`: optional pre-computed direction vectors `((nx1,ny1),(nx2,ny2))`
    /// matching C++ parameters. When `None`, directions are extracted from vertices.
    /// C++ PaintPolylineWithArrowsAlterBuf (emPainter.cpp:2820-2906).
    pub(crate) fn PaintPolylineWithArrows(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
        arrow_dirs: Option<((f64, f64), (f64, f64))>,
    ) {
        let n = vertices.len();
        if n == 0 { return; }

        let mut work = vertices.to_vec();
        let mut p1: usize = 0;
        let mut p2: usize = n - 1;

        let ((nx1, ny1), (nx2, ny2)) = if let Some(dirs) = arrow_dirs {
            dirs
        } else {
            let (sdx, sdy) = Self::extract_segment_dir(vertices, true);
            let (edx, edy) = Self::extract_segment_dir(vertices, false);
            ((sdx, sdy), (-edx, -edy))
        };

        let has_start = stroke.start_end.IsDecorated();
        let has_end = stroke.finish_end.IsDecorated();

        if has_start {
            let (x0, y0) = work[0];
            while p1 < p2 {
                let (ex1, ey1) = (work[p1].0 - x0, work[p1].1 - y0);
                let (ex2, ey2) = (work[p1 + 1].0 - x0, work[p1 + 1].1 - y0);
                let t = Self::cut_line_at_arrow(
                    ex1 * nx1 + ey1 * ny1, ey1 * nx1 - ex1 * ny1,
                    ex2 * nx1 + ey2 * ny1, ey2 * nx1 - ex2 * ny1,
                    stroke.width, stroke, &stroke.start_end,
                );
                if t < 1.0 {
                    work[p1].0 = (1.0 - t) * work[p1].0 + t * work[p1 + 1].0;
                    work[p1].1 = (1.0 - t) * work[p1].1 + t * work[p1 + 1].1;
                    break;
                }
                p1 += 1;
            }
        }

        if has_end {
            let (x0, y0) = work[p2];
            while p2 > p1 {
                let (ex1, ey1) = (work[p2].0 - x0, work[p2].1 - y0);
                let (ex2, ey2) = (work[p2 - 1].0 - x0, work[p2 - 1].1 - y0);
                let t = Self::cut_line_at_arrow(
                    ex1 * nx2 + ey1 * ny2, ey1 * nx2 - ex1 * ny2,
                    ex2 * nx2 + ey2 * ny2, ey2 * nx2 - ex2 * ny2,
                    stroke.width, stroke, &stroke.finish_end,
                );
                if t < 1.0 {
                    work[p2].0 = (1.0 - t) * work[p2].0 + t * work[p2 - 1].0;
                    work[p2].1 = (1.0 - t) * work[p2].1 + t * work[p2 - 1].1;
                    break;
                }
                p2 -= 1;
            }
        }

        
        self.PaintPolylineWithoutArrows(&work[p1..=p2], stroke, closed, canvas_color);

        if has_start {
            let (x, y) = vertices[0];
            self.paint_stroke_end(x, y, nx1, ny1, stroke.width, stroke, &stroke.start_end, emColor::TRANSPARENT);
        }
        if has_end {
            let (x, y) = vertices[n - 1];
            self.paint_stroke_end(x, y, nx2, ny2, stroke.width, stroke, &stroke.finish_end, emColor::TRANSPARENT);
        }
    }

    /// Draw a stroked polyline with proper joins and caps.
    ///
    /// Structural port of C++ `emPainter::PaintSolidPolyline`. Builds a Vertex
    /// array with per-edge direction, per-vertex miter vectors, and edge-length
    /// tracking, then walks forward (right side) and backward (left side) to
    /// produce a single filled polygon.
    pub fn PaintSolidPolyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
    ) {
        if vertices.is_empty() || stroke.width <= 0.0 {
            return;
        }
        let Some(proof) = self.try_record(DrawOp::PaintSolidPolyline {
            vertices: vertices.to_vec(),
            stroke: stroke.clone(),
            closed,
            canvas_color,
        }) else { return; };

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        // --- C++ Vertex flags ---
        const VTX_IS_START: u32 = 1 << 0;
        const VTX_IS_END: u32 = 1 << 1;
        const VTX_IS_NEAR_START_OR_END: u32 = 1 << 2;
        const VTX_DISALLOW_OUTER_MITER: u32 = 1 << 3;

        struct Vtx {
            dir: i32, // 0=right turn, 1=left turn, -1=start/end/collinear
            flags: u32,
            x: f64,
            y: f64,
            nx: f64,      // outgoing edge unit direction X
            ny: f64,      // outgoing edge unit direction Y
            el: [f64; 2], // remaining edge length: [0]=right side, [1]=left side
            nn: f64,      // dot(prev_edge_dir, this_edge_dir)
            mx: f64,      // miter vector X (points toward outer side of turn)
            my: f64,      // miter vector Y
        }

        let n = vertices.len();
        let thickness = stroke.width;
        let d = thickness * 0.5;
        let rounded = stroke.join == super::emStroke::LineJoin::Round
            || stroke.cap == super::emStroke::LineCap::Round;

        // ── Phase 1: Build vertex array with short-segment filtering ──

        let min_seg_len = MIN_REL_SEG_LEN * thickness * 1.01;
        let mut vtx: Vec<Vtx> = Vec::with_capacity(n + 1);

        let mut x1 = vertices[0].0;
        let mut y1 = vertices[0].1;

        for (i, &(x2, y2)) in vertices.iter().enumerate().skip(1) {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let l = (dx * dx + dy * dy).sqrt();
            // Keep segment if long enough, or if it's the only segment
            // and either end is non-cap (not purely rounded-cap line).
            if l >= min_seg_len
                || (l > 1e-140
                    && vtx.is_empty()
                    && i == n - 1
                    && (!rounded
                        || stroke.start_end.end_type != StrokeEndType::Cap
                        || stroke.finish_end.end_type != StrokeEndType::Cap))
            {
                vtx.push(Vtx {
                    dir: 0,
                    flags: 0,
                    x: x1,
                    y: y1,
                    nx: dx / l,
                    ny: dy / l,
                    el: [l, l],
                    nn: 0.0,
                    mx: 0.0,
                    my: 0.0,
                });
                x1 = x2;
                y1 = y2;
            }
        }

        // Sentinel last vertex.
        vtx.push(Vtx {
            dir: 0,
            flags: 0,
            x: x1,
            y: y1,
            nx: 1.0,
            ny: 0.0,
            el: [0.0, 0.0],
            nn: 0.0,
            mx: 0.0,
            my: 0.0,
        });

        if vtx.len() < 2 {
            return;
        }

        let v_last = vtx.len() - 1;

        // ── Phase 1b: Handle closed vs open, set up miter iteration ──

        // miter_pairs: pairs (v1, v2) to process in the miter loop.
        // v1 is the vertex with the incoming edge, v2 is the vertex getting the miter.
        let mut miter_pairs: Vec<(usize, usize)> = Vec::new();

        if closed {
            // Compute closing edge direction on vLast.
            let x2 = vertices[0].0;
            let y2 = vertices[0].1;
            let mut vi = v_last;
            loop {
                let dx = x2 - vtx[vi].x;
                let dy = y2 - vtx[vi].y;
                let ll = dx * dx + dy * dy;
                if ll > 1e-280 {
                    let l = ll.sqrt();
                    vtx[vi].nx = dx / l;
                    vtx[vi].ny = dy / l;
                    vtx[vi].el = [l, l];
                    break;
                }
                if vi == 0 {
                    break;
                }
                vi -= 1;
                // Effectively "vLast--" — shrink the active vertex range.
            }
            // For closed: miter loop starts at (vLast, 0) and goes backward
            // to (0+1's predecessor, 0+1). C++ order: (vLast,0), (vLast-1,vLast), ..., (0,1).
            miter_pairs.push((vi, 0));
            let mut v1i = vi;
            while v1i > 0 {
                v1i -= 1;
                let v2i = v1i + 1;
                miter_pairs.push((v1i, v2i));
            }
        } else {
            // Open polyline.
            vtx[0].flags = VTX_IS_START;
            vtx[0].dir = -1;
            vtx[v_last].flags |= VTX_IS_END;
            vtx[v_last].dir = -1;
            if v_last >= 2 {
                vtx[1].flags = VTX_IS_NEAR_START_OR_END;
                vtx[v_last - 1].flags = VTX_IS_NEAR_START_OR_END;
            }
            // v1 = vLast-2, v2 = vLast-1 down to v1 = vtx[0].
            if v_last >= 2 {
                let mut v1i = v_last - 2;
                loop {
                    let v2i = v1i + 1;
                    miter_pairs.push((v1i, v2i));
                    if v1i == 0 {
                        break;
                    }
                    v1i -= 1;
                }
            }
        }

        // ── Phase 2: Miter computation ──

        let max_m = MAX_MITER * d;

        for &(v1i, v2i) in &miter_pairs {
            let mx_raw = vtx[v1i].nx - vtx[v2i].nx;
            let my_raw = vtx[v1i].ny - vtx[v2i].ny;
            let ll = mx_raw * mx_raw + my_raw * my_raw;
            if ll <= 1e-280 {
                vtx[v2i].dir = -1; // collinear
                continue;
            }
            let l = ll.sqrt();
            let mx_n = mx_raw / l;
            let my_n = my_raw / l;
            let nm_base = vtx[v1i].nx * mx_n + vtx[v1i].ny * my_n;
            let m = d / (1.0 - nm_base * nm_base).max(1e-40).sqrt();
            let nm = nm_base * m;
            let mx = mx_n * m;
            let my = my_n * m;
            vtx[v2i].mx = mx;
            vtx[v2i].my = my;
            if m > max_m {
                vtx[v2i].flags |= VTX_DISALLOW_OUTER_MITER;
            }
            let dir = if vtx[v1i].nx * vtx[v2i].ny - vtx[v1i].ny * vtx[v2i].nx < 0.0 {
                1
            } else {
                0
            };
            vtx[v2i].dir = dir;
            let d_idx = dir as usize;
            vtx[v1i].el[d_idx] -= nm;
            let dot = vtx[v2i].nx * mx + vtx[v2i].ny * my;
            vtx[v2i].el[d_idx] += dot;
            vtx[v2i].nn = vtx[v1i].nx * vtx[v2i].nx + vtx[v1i].ny * vtx[v2i].ny;
        }

        // ── Phase 3: Walk and emit polygon ──

        let scale_sum = self.state.scale_x + self.state.scale_y;
        let mut outline: Vec<f64> = Vec::with_capacity(vtx.len() * 8);

        // State machine using indices. C++ uses pointers v1, e1, e2.
        let mut dir: i32 = 0; // 0 = right side (forward), 1 = left side (backward)
        let mut sd = d; // signed half-width: positive for right, negative for left
        let mut mid_out: usize = 0;

        // e1 = previous edge vertex, e2 = next edge vertex, v1 = current vertex
        let mut v1i: usize = 0;
        let mut e1i: usize = v_last;
        let mut e2i: usize = 0;

        loop {
            // Macro-like inline functions replaced by direct logic.
            let v1_dir = vtx[v1i].dir;
            let v1_flags = vtx[v1i].flags;

            if v1_dir == dir {
                // ── INNER side of turn ──
                let el_e1 = vtx[e1i].el[dir as usize];
                let el_e2 = vtx[e2i].el[dir as usize];
                if el_e1 > 0.0 {
                    if el_e2 > 0.0 {
                        // INNER_MITER
                        outline.push(vtx[v1i].x - vtx[v1i].mx);
                        outline.push(vtx[v1i].y - vtx[v1i].my);
                    } else {
                        // e1 ok, e2 consumed — check near-endpoint
                        if (v1_flags & VTX_IS_NEAR_START_OR_END) != 0 && vtx[v1i].nn >= -0.5 {
                            outline.push(vtx[v1i].x - vtx[v1i].mx);
                            outline.push(vtx[v1i].y - vtx[v1i].my);
                        } else {
                            // BEVEL
                            outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                            outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                            outline.push(vtx[v1i].x - sd * vtx[e2i].ny);
                            outline.push(vtx[v1i].y + sd * vtx[e2i].nx);
                        }
                    }
                } else if el_e2 <= 0.0 {
                    // Both edges consumed
                    if vtx[v1i].nn < 0.5 {
                        // BEVEL
                        outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                        outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                        outline.push(vtx[v1i].x - sd * vtx[e2i].ny);
                        outline.push(vtx[v1i].y + sd * vtx[e2i].nx);
                    }
                    // else SKIP (nn >= 0.5 and both consumed)
                } else {
                    // e1 consumed, e2 ok — check near-endpoint
                    if (v1_flags & VTX_IS_NEAR_START_OR_END) != 0 && vtx[v1i].nn >= -0.5 {
                        outline.push(vtx[v1i].x - vtx[v1i].mx);
                        outline.push(vtx[v1i].y - vtx[v1i].my);
                    } else {
                        // BEVEL
                        outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                        outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                        outline.push(vtx[v1i].x - sd * vtx[e2i].ny);
                        outline.push(vtx[v1i].y + sd * vtx[e2i].nx);
                    }
                }
            } else if v1_dir < 0 {
                // ── START, END, or COLLINEAR vertex ──
                if (v1_flags & (VTX_IS_START | VTX_IS_END)) != 0 {
                    let is_end_on_right = dir == 0 && (v1_flags & VTX_IS_END) != 0;
                    let is_start_on_left = dir == 1 && (v1_flags & VTX_IS_START) != 0;
                    if !is_end_on_right && !is_start_on_left {
                        // SKIP — wrong cap for this walking direction
                    } else {
                        // Determine cap type from stroke end.
                        let st = if dir == 0 {
                            &stroke.finish_end
                        } else {
                            &stroke.start_end
                        };
                        if st.end_type != StrokeEndType::Cap {
                            // BUTT
                            outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                            outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                            outline.push(vtx[v1i].x + sd * vtx[e1i].ny);
                            outline.push(vtx[v1i].y - sd * vtx[e1i].nx);
                        } else if !rounded {
                            // NRCAP (non-rounded cap = square cap)
                            outline.push(vtx[v1i].x + sd * (vtx[e1i].nx - vtx[e1i].ny));
                            outline.push(vtx[v1i].y + sd * (vtx[e1i].ny + vtx[e1i].nx));
                            outline.push(vtx[v1i].x + sd * (vtx[e1i].nx + vtx[e1i].ny));
                            outline.push(vtx[v1i].y + sd * (vtx[e1i].ny - vtx[e1i].nx));
                        } else {
                            // ROUND cap
                            let f = CIRCLE_QUALITY * (d * scale_sum).sqrt() * 0.5;
                            if f < 1.5 {
                                // Degrade to BUTT
                                outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                                outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                                outline.push(vtx[v1i].x + sd * vtx[e1i].ny);
                                outline.push(vtx[v1i].y - sd * vtx[e1i].nx);
                            } else {
                                let a = std::f64::consts::PI;
                                let k = (f + 0.5) as usize;
                                let k = k.clamp(1, 128);
                                let step = a / k as f64;
                                for j in 0..=k {
                                    let c = (step * j as f64).cos();
                                    let s = (step * j as f64).sin();
                                    outline.push(
                                        vtx[v1i].x + sd * (s * vtx[e1i].nx - c * vtx[e1i].ny),
                                    );
                                    outline.push(
                                        vtx[v1i].y + sd * (s * vtx[e1i].ny + c * vtx[e1i].nx),
                                    );
                                }
                            }
                        }
                    }
                }
                // else: collinear, SKIP
            } else {
                // ── OUTER side of turn ──
                if rounded && vtx[v1i].nn < 1.0 {
                    let a = if vtx[v1i].nn > -1.0 {
                        vtx[v1i].nn.acos()
                    } else {
                        std::f64::consts::PI
                    };
                    let f =
                        CIRCLE_QUALITY * (d * scale_sum).sqrt() * a / (2.0 * std::f64::consts::PI);
                    if f >= 1.5 {
                        // ROUND join
                        let k = (f + 0.5) as usize;
                        let k = k.clamp(1, 128);
                        let step = a / k as f64;
                        for j in 0..=k {
                            let c = (step * j as f64).cos();
                            let s = (step * j as f64).sin();
                            outline.push(vtx[v1i].x + sd * (s * vtx[e1i].nx - c * vtx[e1i].ny));
                            outline.push(vtx[v1i].y + sd * (s * vtx[e1i].ny + c * vtx[e1i].nx));
                        }
                    } else if f >= 0.5 {
                        // BEVEL
                        outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                        outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                        outline.push(vtx[v1i].x - sd * vtx[e2i].ny);
                        outline.push(vtx[v1i].y + sd * vtx[e2i].nx);
                    } else {
                        // f < 0.5: fall through to miter/bevel below
                        if (v1_flags & VTX_DISALLOW_OUTER_MITER) == 0 {
                            outline.push(vtx[v1i].x + vtx[v1i].mx);
                            outline.push(vtx[v1i].y + vtx[v1i].my);
                        } else {
                            outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                            outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                            outline.push(vtx[v1i].x - sd * vtx[e2i].ny);
                            outline.push(vtx[v1i].y + sd * vtx[e2i].nx);
                        }
                    }
                } else if (v1_flags & VTX_DISALLOW_OUTER_MITER) == 0 {
                    // OUTER_MITER
                    outline.push(vtx[v1i].x + vtx[v1i].mx);
                    outline.push(vtx[v1i].y + vtx[v1i].my);
                } else {
                    // BEVEL
                    outline.push(vtx[v1i].x - sd * vtx[e1i].ny);
                    outline.push(vtx[v1i].y + sd * vtx[e1i].nx);
                    outline.push(vtx[v1i].x - sd * vtx[e2i].ny);
                    outline.push(vtx[v1i].y + sd * vtx[e2i].nx);
                }
            }

            // ── Advance pointers ──
            if dir == 0 {
                e1i = e2i;
                e2i += 1;
                v1i = e2i;
                if e2i <= v_last {
                    continue;
                }
                // Switch to backward (left side) walk.
                dir = 1;
                sd = -sd;
                mid_out = outline.len();
                v1i = v_last;
                e1i = v_last;
                e2i = v_last;
                if v_last > 0 {
                    e2i = v_last - 1;
                }
            } else {
                if v1i == 0 {
                    break;
                }
                v1i = e2i;
                e1i = e2i;
                if e2i == 0 {
                    e2i = v_last;
                } else {
                    e2i -= 1;
                }
            }
        }

        // ── Closed-polygon stitching ──
        if closed && mid_out > 0 && mid_out < outline.len() {
            outline.push(outline[mid_out]);
            outline.push(outline[mid_out + 1]);
            outline.push(outline[mid_out - 2]);
            outline.push(outline[mid_out - 1]);
        }

        // Convert flat [x,y,x,y,...] to [(x,y),...] for fill_polygon_aa.
        let n_out = outline.len() / 2;
        let poly: Vec<(f64, f64)> = (0..n_out)
            .map(|i| (outline[i * 2], outline[i * 2 + 1]))
            .collect();

        self.fill_polygon_aa(proof, &poly, stroke.color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a stroked line with optional end decorations.
    pub fn paint_line_stroked(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintLineStroked {
            x0,
            y0,
            x1,
            y1,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };
        // C++ PaintLine always uses PaintSolidPolyline which respects thickness.
        // No shortcut to simple PaintLine (which draws 1px regardless of thickness).

        // Route through the polyline system which handles caps, joins,
        // decorations, and dashes correctly — matching C++ PaintLine.
        // C++ PaintLine computes direction (nx,ny) and passes ((nx,ny),(-nx,-ny)).
        let verts = [(x0, y0), (x1, y1)];
        let arrow_dirs = if stroke.start_end.IsDecorated() || stroke.finish_end.IsDecorated() {
            let dx = x1 - x0;
            let dy = y1 - y0;
            let ll = dx * dx + dy * dy;
            let (nx, ny) = if ll > 1e-280 {
                let l = ll.sqrt();
                (dx / l, dy / l)
            } else {
                (1.0, 0.0)
            };
            Some(((nx, ny), (-nx, -ny)))
        } else {
            None
        };
        self.PaintPolylineWithArrows(&verts, stroke, false, canvas_color, arrow_dirs);
    }

    /// Calculate the maximum radius that a line point (including any arrow
    /// decorations) can extend from the vertex. Used for clip-rectangle
    /// expansion when testing visibility.
    /// Corresponds to C++ `CalculateLinePointMinMaxRadius`.
    pub fn CalculateLinePointMinMaxRadius(
        thickness: f64,
        stroke: &emStroke,
        stroke_start: &emStrokeEnd,
        stroke_end: &emStrokeEnd,
    ) -> f64 {
        let mut r = thickness * 0.5;
        if stroke.join != super::emStroke::LineJoin::Round {
            r *= MAX_MITER.max(1.415);
        }
        if stroke_start.IsDecorated() {
            let w = thickness * ARROW_BASE_SIZE * 0.5 * stroke_start.width_factor;
            let l = thickness * ARROW_BASE_SIZE * stroke_start.length_factor;
            r = r.max((w * w + l * l).sqrt());
        }
        if stroke_end.IsDecorated() {
            let w = thickness * ARROW_BASE_SIZE * 0.5 * stroke_end.width_factor;
            let l = thickness * ARROW_BASE_SIZE * stroke_end.length_factor;
            r = r.max((w * w + l * l).sqrt());
        }
        r
    }

    /// Simplified line shortening for arrow decorations.
    /// `(dx, dy)` points INTO the line body. Returns the new endpoint moved inward.
    /// Exact port of C++ `emPainter::CutLineAtArrow`.
    ///
    /// Takes a line segment (x1,y1)→(x2,y2) in decoration-local coordinates
    /// Extract a normalized direction from the first or last non-degenerate
    /// segment of a vertex list. Used as fallback when callers don't provide
    /// pre-computed arrow direction vectors.
    fn extract_segment_dir(vertices: &[(f64, f64)], from_start: bool) -> (f64, f64) {
        let n = vertices.len();
        let mut dx = 0.0;
        let mut dy = 0.0;
        if from_start {
            for i in 0..n - 1 {
                dx = vertices[i + 1].0 - vertices[i].0;
                dy = vertices[i + 1].1 - vertices[i].1;
                if dx * dx + dy * dy > 1e-280 {
                    break;
                }
            }
        } else {
            for i in (0..n - 1).rev() {
                dx = vertices[i + 1].0 - vertices[i].0;
                dy = vertices[i + 1].1 - vertices[i].1;
                if dx * dx + dy * dy > 1e-280 {
                    break;
                }
            }
        }
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-140 {
            (1.0, 0.0)
        } else {
            (dx / len, dy / len)
        }
    }

    /// (x = along main direction, y = perpendicular). Returns parametric t
    /// (0.0–1.0) for where the segment exits the decoration shape. t >= 1.0
    /// means the entire segment is inside the decoration.
    #[allow(clippy::excessive_precision, clippy::needless_return)]
    /// C++ emPainter::CutLineAtArrow (emPainter.cpp:2934-3182).
    /// Returns t in [0, 1]: fraction of line segment outside the arrow shape.
    fn cut_line_at_arrow(
        x1: f64, y1: f64, x2: f64, y2: f64,
        thickness: f64, stroke: &emStroke, stroke_end: &emStrokeEnd,
    ) -> f64 {
        let mut r = (thickness * ARROW_BASE_SIZE * 0.5 * stroke_end.width_factor).abs();
        if r <= 1e-140 { return 0.0; }
        let mut l = thickness * ARROW_BASE_SIZE * stroke_end.length_factor;
        if l <= 1e-140 { return 0.0; }

        let rounded = stroke.cap == super::emStroke::LineCap::Round;
        let mut s: f64;

        match stroke_end.end_type {
            StrokeEndType::Butt | StrokeEndType::Cap => 0.0,

            StrokeEndType::Arrow => {
                let d = thickness * 0.5;
                let b = l / r;
                s = (1.0 + b * b).sqrt() * d;
                let b2 = b * ARROW_NOTCH;
                let u = (1.0 + b2 * b2).sqrt() * d;
                let l2 = l - (s + u) / (1.0 - ARROW_NOTCH);
                r *= l2 / l;
                l = l2;
                Self::cut_shape_arrow(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::ContourArrow => {
                s = thickness * 0.5;
                if !rounded {
                    let sin_a = r / (l * l + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 { s *= sin_a; } else { s /= sin_a; }
                }
                Self::cut_shape_arrow(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::LineArrow => {
                s = thickness * 0.5;
                if !rounded {
                    let sin_a = r / (l * l + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 { s *= sin_a; } else { s /= sin_a; }
                }
                let l2 = s * 1.5;
                r *= l2 / l;
                l = l2;
                s = 0.0;
                Self::cut_shape_triangle(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::Triangle => {
                let d = thickness * 0.5;
                let b = l / r;
                s = (1.0 + b * b).sqrt() * d;
                let l2 = l - s - d;
                r *= l2 / l;
                l = l2;
                Self::cut_shape_triangle(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::ContourTriangle => {
                s = thickness * 0.5;
                if !rounded {
                    let sin_a = r / (l * l + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 { s *= sin_a; } else { s /= sin_a; }
                }
                Self::cut_shape_triangle(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::Square => {
                s = thickness * 0.5;
                r = (r - s).max(0.0);
                l = (l - thickness).max(0.0);
                Self::cut_shape_square(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::ContourSquare => {
                s = thickness * 0.5;
                Self::cut_shape_square(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::HalfSquare => {
                s = thickness * 0.5;
                l = (l * 0.5 - s).max(thickness * 0.0001);
                Self::cut_shape_square(x1 - s, y1, x2 - s, y2, r, l)
            }
            StrokeEndType::Circle => {
                s = thickness * 0.5;
                r = (r - s).max(0.0);
                l = (l - thickness).max(0.0);
                Self::cut_shape_circle(x1, y1, x2, y2, r, l, s, false)
            }
            StrokeEndType::ContourCircle => {
                s = thickness * 0.5;
                Self::cut_shape_circle(x1, y1, x2, y2, r, l, s, false)
            }
            StrokeEndType::HalfCircle => {
                s = if rounded { thickness * 0.5 } else { 0.0 } - l * 0.5;
                Self::cut_shape_circle(x1, y1, x2, y2, r, l, s, true)
            }
            StrokeEndType::Diamond => {
                s = (r * r + l * l * 0.25).sqrt() / r * thickness * 0.5;
                let l2 = l - s - s;
                r *= l2 / l;
                l = l2;
                Self::cut_shape_diamond(x1 - s, y1, x2 - s, y2, r, l, false)
            }
            StrokeEndType::ContourDiamond => {
                s = thickness * 0.5;
                if !rounded {
                    let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 { s *= sin_a; } else { s /= sin_a; }
                }
                Self::cut_shape_diamond(x1 - s, y1, x2 - s, y2, r, l, false)
            }
            StrokeEndType::HalfDiamond => {
                s = thickness * 0.5;
                if !rounded {
                    let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                    s *= sin_a + (1.0 - sin_a).sqrt();
                }
                s -= l * 0.5;
                Self::cut_shape_diamond(x1 - s, y1, x2 - s, y2, r, l, true)
            }
            StrokeEndType::emStroke => {
                l = thickness * (stroke_end.length_factor.abs() - 1.0);
                if l < 0.0 { l = 0.0; }
                s = -l * 0.5;
                Self::cut_shape_square(x1 - s, y1, x2 - s, y2, r, l)
            }
        }
    }

    // --- CutLineAtArrow shape helpers (C++ L_ARROW, L_TRIANGLE, etc.) ---

    fn cut_shape_arrow(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dr = r / l;
        let l2 = (1.0 - ARROW_NOTCH) * l;
        let d2 = r / (l - l2);
        let mut t = 1.0;
        if dy - d2 * dx < -1e-140 {
            if y1 <= d2 * (x1 - l2) { t = 0.0; }
            else if y2 < (x2 - l2) * d2 { t = (d2 * (x1 - l2) - y1) / (dy - d2 * dx); }
        }
        let mut u = 1.0;
        if dy + d2 * dx > 1e-140 {
            if y1 >= -d2 * (x1 - l2) { u = 0.0; }
            else if y2 > -(x2 - l2) * d2 { u = (-d2 * (x1 - l2) - y1) / (dy + d2 * dx); }
        }
        if t < u { t = u; }
        if dy - dr * dx > 1e-140 {
            if y1 >= dr * x1 { return 0.0; }
            if y2 > x2 * dr { t = t.min((dr * x1 - y1) / (dy - dr * dx)); }
        }
        if dy + dr * dx < -1e-140 {
            if y1 <= -dr * x1 { return 0.0; }
            if y2 < -x2 * dr { t = t.min((-dr * x1 - y1) / (dy + dr * dx)); }
        }
        t
    }

    fn cut_shape_triangle(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dr = r / l;
        let mut t = 1.0;
        if dx > 1e-140 {
            if x1 >= l { return 0.0; }
            if x2 > l { t = (l - x1) / dx; }
        }
        if dy - dr * dx > 1e-140 {
            if y1 >= dr * x1 { return 0.0; }
            if y2 > x2 * dr { t = t.min((dr * x1 - y1) / (dy - dr * dx)); }
        }
        if dy + dr * dx < -1e-140 {
            if y1 <= -dr * x1 { return 0.0; }
            if y2 < -x2 * dr { t = t.min((-dr * x1 - y1) / (dy + dr * dx)); }
        }
        t
    }

    fn cut_shape_square(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let mut t = 1.0;
        if dx > 1e-140 {
            if x1 >= l { return 0.0; }
            if x2 > l { t = (l - x1) / dx; }
        } else if dx < -1e-140 {
            if x1 <= 0.0 { return 0.0; }
            if x2 < 0.0 { t = -x1 / dx; }
        }
        if dy > 1e-140 {
            if y1 >= r { return 0.0; }
            if y2 > r { t = t.min((r - y1) / dy); }
        } else if dy < -1e-140 {
            if y1 <= -r { return 0.0; }
            if y2 < -r { t = t.min((-r - y1) / dy); }
        }
        t
    }

    #[allow(clippy::too_many_arguments)]
    fn cut_shape_circle(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64, s: f64, semi: bool) -> f64 {
        let x1 = (x1 - s) * 2.0 / l - 1.0;
        let x2 = (x2 - s) * 2.0 / l - 1.0;
        let y1 = y1 / r;
        let y2 = y2 / r;
        let dx = x2 - x1;
        let dy = y2 - y1;
        let d = dx * dx + dy * dy;
        if d <= 1e-140 { return 1.0; }
        let d1 = x1 * x1 + y1 * y1;
        let d2 = x2 * x2 + y2 * y2;
        let u = (x1 * dx + y1 * dy) / d;
        let disc = (1.0 - d1) / d + u * u;
        if disc < 0.0 {
            return if d1 < d2 { 0.0 } else { 1.0 };
        }
        let mut t = (disc.sqrt() - u).clamp(0.0, 1.0);
        if semi && dx < -1e-140 {
            if x1 <= 0.0 { return 0.0; }
            if x2 < 0.0 { t = t.min(-x1 / dx); }
        }
        t
    }

    fn cut_shape_diamond(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64, semi: bool) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dr = 2.0 * r / l;
        let mut t = 1.0;
        if dy - dr * dx > 1e-140 {
            if y1 >= dr * x1 { return 0.0; }
            if y2 > x2 * dr { t = (dr * x1 - y1) / (dy - dr * dx); }
        }
        if dy + dr * dx < -1e-140 {
            if y1 <= -dr * x1 { return 0.0; }
            if y2 < -x2 * dr { t = t.min((-dr * x1 - y1) / (dy + dr * dx)); }
        }
        if dy - dr * dx < -1e-140 {
            if y1 <= dr * (x1 - l) { return 0.0; }
            if y2 < (x2 - l) * dr { t = t.min((dr * (x1 - l) - y1) / (dy - dr * dx)); }
        }
        if dy + dr * dx > 1e-140 {
            if y1 >= -dr * (x1 - l) { return 0.0; }
            if y2 > -(x2 - l) * dr { t = t.min((-dr * (x1 - l) - y1) / (dy + dr * dx)); }
        }
        if semi && dx < -1e-140 {
            if x1 <= l * 0.5 { return 0.0; }
            if x2 < l * 0.5 { t = t.min((l * 0.5 - x1) / dx); }
        }
        t
    }

    /// C++ emPainter::PaintArrow (emPainter.cpp:3184-3449).
    /// `(nx, ny)`: along-line direction pointing INTO the body (matches C++ PaintArrow params).
    /// Perpendicular computed as `(ny, -nx)`.
    #[allow(clippy::too_many_arguments)]
    fn paint_stroke_end(
        &mut self,
        x: f64, y: f64,
        nx: f64, ny: f64,
        thickness: f64,
        stroke: &emStroke,
        stroke_end: &emStrokeEnd,
        canvas_color: emColor,
    ) {
        let r = (thickness * ARROW_BASE_SIZE * 0.5 * stroke_end.width_factor).abs();
        if r <= 1e-140 { return; }
        let mut l = thickness * ARROW_BASE_SIZE * stroke_end.length_factor;
        let (nx, ny) = if l < 0.0 {
            l = -l;
            (-nx, -ny)
        } else {
            (nx, ny)
        };
        if l <= 1e-140 { return; }

        let rounded = stroke.cap == super::emStroke::LineCap::Round;

        // C++: arrowStroke = stroke; arrowStroke.DashType = SOLID;
        let mut arrow_stroke = stroke.clone();
        arrow_stroke.dash_type = super::emStroke::DashType::Solid;
        arrow_stroke.dash_pattern.clear();

        let bc = 4.0_f64 / 3.0 * (std::f64::consts::PI / 8.0).tan();

        match stroke_end.end_type {
            StrokeEndType::Butt | StrokeEndType::Cap => {}

            StrokeEndType::Arrow => {
                self.PaintPolygon(
                    &[
                        (x, y),
                        (x + l * nx + r * ny, y + l * ny - r * nx),
                        (x + (1.0 - ARROW_NOTCH) * l * nx, y + (1.0 - ARROW_NOTCH) * l * ny),
                        (x + l * nx - r * ny, y + l * ny + r * nx),
                    ],
                    stroke.color, canvas_color,
                );
            }

            StrokeEndType::ContourArrow => {
                let s = Self::contour_offset(thickness, rounded, r, l);
                let verts = [
                    (x + s * nx, y + s * ny),
                    (x + (s + l) * nx + r * ny, y + (s + l) * ny - r * nx),
                    (x + (s + (1.0 - ARROW_NOTCH) * l) * nx, y + (s + (1.0 - ARROW_NOTCH) * l) * ny),
                    (x + (s + l) * nx - r * ny, y + (s + l) * ny + r * nx),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, canvas_color);
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, true, canvas_color);
            }

            StrokeEndType::LineArrow => {
                let s = Self::contour_offset(thickness, rounded, r, l);
                let verts = [
                    (x + (s + l) * nx - r * ny, y + (s + l) * ny + r * nx),
                    (x + s * nx, y + s * ny),
                    (x + (s + l) * nx + r * ny, y + (s + l) * ny - r * nx),
                ];
                let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
                arrow_stroke.start_end = cap_end;
                arrow_stroke.finish_end = cap_end;
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, false, canvas_color);
            }

            StrokeEndType::Triangle => {
                self.PaintPolygon(
                    &[
                        (x, y),
                        (x + l * nx + r * ny, y + l * ny - r * nx),
                        (x + l * nx - r * ny, y + l * ny + r * nx),
                    ],
                    stroke.color, canvas_color,
                );
            }

            StrokeEndType::ContourTriangle => {
                let s = Self::contour_offset(thickness, rounded, r, l);
                let verts = [
                    (x + s * nx, y + s * ny),
                    (x + (s + l) * nx + r * ny, y + (s + l) * ny - r * nx),
                    (x + (s + l) * nx - r * ny, y + (s + l) * ny + r * nx),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, canvas_color);
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, true, canvas_color);
            }

            StrokeEndType::Square => {
                self.PaintPolygon(
                    &[
                        (x + r * ny, y - r * nx),
                        (x + l * nx + r * ny, y + l * ny - r * nx),
                        (x + l * nx - r * ny, y + l * ny + r * nx),
                        (x - r * ny, y + r * nx),
                    ],
                    stroke.color, canvas_color,
                );
            }

            StrokeEndType::ContourSquare => {
                let s = thickness * 0.5;
                let verts = [
                    (x + s * nx + r * ny, y + s * ny - r * nx),
                    (x + (s + l) * nx + r * ny, y + (s + l) * ny - r * nx),
                    (x + (s + l) * nx - r * ny, y + (s + l) * ny + r * nx),
                    (x + s * nx - r * ny, y + s * ny + r * nx),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, canvas_color);
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, true, canvas_color);
            }

            StrokeEndType::HalfSquare => {
                let s = thickness * 0.5;
                let l_adj = (l * 0.5 - s).max(thickness * 0.0001);
                let verts = [
                    (x + s * nx + r * ny, y + s * ny - r * nx),
                    (x + (s + l_adj) * nx + r * ny, y + (s + l_adj) * ny - r * nx),
                    (x + (s + l_adj) * nx - r * ny, y + (s + l_adj) * ny + r * nx),
                    (x + s * nx - r * ny, y + s * ny + r * nx),
                ];
                let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
                arrow_stroke.start_end = cap_end;
                arrow_stroke.finish_end = cap_end;
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, false, canvas_color);
            }

            StrokeEndType::Circle => {
                let pts = [
                    (x, y),
                    (x + bc * r * ny, y - bc * r * nx),
                    (x + (1.0 - bc) * 0.5 * l * nx + r * ny, y + (1.0 - bc) * 0.5 * l * ny - r * nx),
                    (x + 0.5 * l * nx + r * ny, y + 0.5 * l * ny - r * nx),
                    (x + (1.0 + bc) * 0.5 * l * nx + r * ny, y + (1.0 + bc) * 0.5 * l * ny - r * nx),
                    (x + l * nx + bc * r * ny, y + l * ny - bc * r * nx),
                    (x + l * nx, y + l * ny),
                    (x + l * nx - bc * r * ny, y + l * ny + bc * r * nx),
                    (x + (1.0 + bc) * 0.5 * l * nx - r * ny, y + (1.0 + bc) * 0.5 * l * ny + r * nx),
                    (x + 0.5 * l * nx - r * ny, y + 0.5 * l * ny + r * nx),
                    (x + (1.0 - bc) * 0.5 * l * nx - r * ny, y + (1.0 - bc) * 0.5 * l * ny + r * nx),
                    (x - bc * r * ny, y + bc * r * nx),
                ];
                self.PaintBezier(&pts, stroke.color, canvas_color);
            }

            StrokeEndType::ContourCircle => {
                let s = thickness * 0.5;
                let pts = [
                    (x + s * nx, y + s * ny),
                    (x + s * nx + bc * r * ny, y + s * ny - bc * r * nx),
                    (x + (s + (1.0 - bc) * 0.5 * l) * nx + r * ny, y + (s + (1.0 - bc) * 0.5 * l) * ny - r * nx),
                    (x + (s + 0.5 * l) * nx + r * ny, y + (s + 0.5 * l) * ny - r * nx),
                    (x + (s + (1.0 + bc) * 0.5 * l) * nx + r * ny, y + (s + (1.0 + bc) * 0.5 * l) * ny - r * nx),
                    (x + (s + l) * nx + bc * r * ny, y + (s + l) * ny - bc * r * nx),
                    (x + (s + l) * nx, y + (s + l) * ny),
                    (x + (s + l) * nx - bc * r * ny, y + (s + l) * ny + bc * r * nx),
                    (x + (s + (1.0 + bc) * 0.5 * l) * nx - r * ny, y + (s + (1.0 + bc) * 0.5 * l) * ny + r * nx),
                    (x + (s + 0.5 * l) * nx - r * ny, y + (s + 0.5 * l) * ny + r * nx),
                    (x + (s + (1.0 - bc) * 0.5 * l) * nx - r * ny, y + (s + (1.0 - bc) * 0.5 * l) * ny + r * nx),
                    (x + s * nx - bc * r * ny, y + s * ny + bc * r * nx),
                ];
                self.PaintBezier(&pts, stroke_end.inner_color, canvas_color);
                self.PaintBezierOutline(&pts, &arrow_stroke, canvas_color);
            }

            StrokeEndType::HalfCircle => {
                let s = if rounded { thickness * 0.5 } else { 0.0 };
                let pts = [
                    (x + s * nx + r * ny, y + s * ny - r * nx),
                    (x + (s + bc * 0.5 * l) * nx + r * ny, y + (s + bc * 0.5 * l) * ny - r * nx),
                    (x + (s + 0.5 * l) * nx + bc * r * ny, y + (s + 0.5 * l) * ny - bc * r * nx),
                    (x + (s + 0.5 * l) * nx, y + (s + 0.5 * l) * ny),
                    (x + (s + 0.5 * l) * nx - bc * r * ny, y + (s + 0.5 * l) * ny + bc * r * nx),
                    (x + (s + bc * 0.5 * l) * nx - r * ny, y + (s + bc * 0.5 * l) * ny + r * nx),
                    (x + s * nx - r * ny, y + s * ny + r * nx),
                ];
                let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
                let butt_end = emStrokeEnd::butt();
                arrow_stroke.start_end = if rounded { cap_end } else { butt_end };
                arrow_stroke.finish_end = if rounded { cap_end } else { butt_end };
                self.PaintBezierLine(&pts, &arrow_stroke, canvas_color);
            }

            StrokeEndType::Diamond => {
                self.PaintPolygon(
                    &[
                        (x, y),
                        (x + 0.5 * l * nx + r * ny, y + 0.5 * l * ny - r * nx),
                        (x + l * nx, y + l * ny),
                        (x + 0.5 * l * nx - r * ny, y + 0.5 * l * ny + r * nx),
                    ],
                    stroke.color, canvas_color,
                );
            }

            StrokeEndType::ContourDiamond => {
                let s = Self::contour_offset_diamond(thickness, rounded, r, l);
                let verts = [
                    (x + s * nx, y + s * ny),
                    (x + (s + 0.5 * l) * nx + r * ny, y + (s + 0.5 * l) * ny - r * nx),
                    (x + (s + l) * nx, y + (s + l) * ny),
                    (x + (s + 0.5 * l) * nx - r * ny, y + (s + 0.5 * l) * ny + r * nx),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, canvas_color);
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, true, canvas_color);
            }

            StrokeEndType::HalfDiamond => {
                let mut s = thickness * 0.5;
                if !rounded {
                    let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                    s *= sin_a + (1.0 - sin_a).sqrt();
                }
                let verts = [
                    (x + s * nx + r * ny, y + s * ny - r * nx),
                    (x + (s + 0.5 * l) * nx, y + (s + 0.5 * l) * ny),
                    (x + s * nx - r * ny, y + s * ny + r * nx),
                ];
                let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
                arrow_stroke.start_end = cap_end;
                arrow_stroke.finish_end = cap_end;
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, false, canvas_color);
            }

            StrokeEndType::emStroke => {
                let verts = [
                    (x + r * ny, y - r * nx),
                    (x - r * ny, y + r * nx),
                ];
                arrow_stroke.width = thickness * stroke_end.length_factor.abs();
                let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
                arrow_stroke.start_end = cap_end;
                arrow_stroke.finish_end = cap_end;
                self.PaintPolylineWithoutArrows(&verts, &arrow_stroke, false, canvas_color);
            }
        }
    }

    /// C++ contour offset: `s = thickness*0.5`, adjusted by miter if not rounded.
    fn contour_offset(thickness: f64, rounded: bool, r: f64, l: f64) -> f64 {
        let mut s = thickness * 0.5;
        if !rounded {
            let sin_a = r / (l * l + r * r).sqrt();
            if MAX_MITER * sin_a < 1.0 { s *= sin_a; } else { s /= sin_a; }
        }
        s
    }

    /// C++ contour offset for diamond shapes (uses l*l*0.25 in discriminant).
    fn contour_offset_diamond(thickness: f64, rounded: bool, r: f64, l: f64) -> f64 {
        let mut s = thickness * 0.5;
        if !rounded {
            let sin_a = r / (l * l * 0.25 + r * r).sqrt();
            if MAX_MITER * sin_a < 1.0 { s *= sin_a; } else { s /= sin_a; }
        }
        s
    }

    // --- Anti-aliased polygon fill ---

    /// Fill a polygon with anti-aliased edges using the scanline rasterizer.
    fn fill_polygon_aa(&mut self, proof: DirectProof, vertices: &[(f64, f64)], color: emColor, rule: WindingRule) {
        if vertices.len() < 3 {
            return;
        }

        let pixel_verts: Vec<(f64, f64)> = vertices
            .iter()
            .map(|&(x, y)| {
                (
                    x * self.state.scale_x + self.state.offset_x,
                    y * self.state.scale_y + self.state.offset_y,
                )
            })
            .collect();

        let rows = emPainterScanline::rasterize(&pixel_verts, self.state.clip.to_scanline_clip(), rule);

        for (y, spans) in &rows {
            for span in spans {
                self.blit_span(proof, *y, span, color);
            }
        }
    }

    /// Blit a single AA span onto the target.
    fn blit_span(&mut self, proof: DirectProof, y: i32, span: &emPainterScanline::Span, color: emColor) {
        let tw = self.target_width as i32;
        let th = self.target_height as i32;
        if y < 0 || y >= th {
            return;
        }

        let x_start = span.x_start.max(0);
        let x_end = span.x_end.min(tw);
        let span_width = x_end - x_start;
        if span_width <= 0 {
            return;
        }

        // Span opacities are 12-bit (0–0x1000). Combine with color alpha in one
        // step matching C++: alpha = (color_alpha * opacity_12bit + 0x800) >> 12.
        if span_width == 1 {
            let opacity = span.opacity_beg;
            if opacity > 0 {
                self.blend_with_coverage(proof, x_start, y, color, opacity);
            }
            return;
        }

        // First pixel (partial coverage).
        if span.opacity_beg > 0 {
            self.blend_with_coverage(proof, x_start, y, color, span.opacity_beg);
        }

        // Interior pixels — bulk scanline, no per-pixel clip/bounds checks.
        let ix1 = x_start + 1;
        let ix2 = x_end - 1;
        if ix1 < ix2 && span.opacity_mid > 0 {
            // C++ PaintScanlineCol: alpha = (color_alpha * opacity + 0x800) >> 12
            // Raw opacity can exceed 0x1000 for double-wound polygons.
            let alpha =
                ((color.GetAlpha() as i32 * span.opacity_mid + 0x800) >> 12).clamp(0, 255) as u8;
            if alpha > 0 {
                let blended = color.SetAlpha(alpha);
                self.fill_span_blended(proof, y, ix1, ix2, blended);
            }
        }

        // Last pixel (partial coverage).
        if span_width > 1 && span.opacity_end > 0 {
            let x_last = x_end - 1;
            self.blend_with_coverage(proof, x_last, y, color, span.opacity_end);
        }
    }

    /// Paint a scanline span for PaintRect, matching C++ PaintScanlineCol.
    /// `ix`: start pixel X, `iy`: pixel Y, `iw`: width in pixels.
    /// `a1`: first pixel opacity, `a`: interior opacity, `a2`: last pixel opacity.
    /// All opacities are Fixed12 (0..0x1000).
    #[allow(clippy::too_many_arguments)]
    fn paint_rect_scanline(
        &mut self, proof: DirectProof,
        ix: i32, iy: i32, iw: i32,
        a1: i32, a: i32, a2: i32,
        color: emColor,
    ) {
        // First pixel
        self.blend_with_coverage(proof, ix, iy, color, a1);
        // Interior pixels
        if iw > 2 && a > 0 {
            // C++ PaintScanlineCol: alpha = (color_alpha * opacity + 0x800) >> 12
            let combined_alpha = ((color.GetAlpha() as i32 * a + 0x800) >> 12).clamp(0, 255) as u8;
            if combined_alpha > 0 {
                let interior_color = color.SetAlpha(combined_alpha);
                self.fill_span_blended(proof, iy, ix + 1, ix + iw - 1, interior_color);
            }
        }
        // Last pixel (if width > 1)
        if iw > 1 {
            self.blend_with_coverage(proof, ix + iw - 1, iy, color, a2);
        }
    }

    fn blend_with_coverage(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor, cov: i32) {
        if cov <= 0 { return; }
        // C++ PaintScanlineCol: alpha = (color_alpha * opacity + 0x800) >> 12
        // Must use raw opacity (not capped at 0x1000) so double winding (0x2000)
        // with semi-transparent colors correctly produces alpha >= 255.
        let alpha = ((color.GetAlpha() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8;
        if alpha == 0 { return; }
        let blended = color.SetAlpha(alpha);
        self.blend_pixel(proof, x, y, blended);
    }

    /// Same as `blend_pixel` but without clip/bounds checks.
    /// Caller must guarantee x,y are within both the clip rect and the target image.
    #[inline(always)]
    fn blend_pixel_unchecked(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor) {
        // C++ PaintScanlineCol AVX2: three paths based on alpha and canvas.
        let xu = x as u32;
        let yu = y as u32;
        let alpha = color.GetAlpha();
        if alpha == 0 { return; }

        if alpha >= 255 && self.state.alpha == 255 {
            // C++ alpha>=255: direct write
            let out = self.GetImage(proof).SetPixel(xu, yu);
            out[0] = color.GetRed();
            out[1] = color.GetGreen();
            out[2] = color.GetBlue();
            out[3] = 255;
        } else if self.state.canvas_color.IsOpaque() {
            // C++ HAVE_CVC: pix = hash_color[alpha] - hash_canvas[alpha]; *p += pix
            use super::emColor::blend_hash_lookup;
            let a = if self.state.alpha == 255 {
                alpha
            } else {
                ((alpha as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if a == 0 { return; }
            let cv = self.state.canvas_color;
            let dr = blend_hash_lookup(color.GetRed(), a) as i32 - blend_hash_lookup(cv.GetRed(), a) as i32;
            let dg = blend_hash_lookup(color.GetGreen(), a) as i32 - blend_hash_lookup(cv.GetGreen(), a) as i32;
            let db = blend_hash_lookup(color.GetBlue(), a) as i32 - blend_hash_lookup(cv.GetBlue(), a) as i32;
            let px = self.read_pixel(proof, xu, yu);
            let out = self.GetImage(proof).SetPixel(xu, yu);
            out[0] = (px[0] as i32 + dr).clamp(0, 255) as u8;
            out[1] = (px[1] as i32 + dg).clamp(0, 255) as u8;
            out[2] = (px[2] as i32 + db).clamp(0, 255) as u8;
        } else {
            // C++ no canvas: fused source-over
            let ea = if self.state.alpha == 255 {
                alpha as u16
            } else {
                (alpha as u16 * self.state.alpha as u16 + 128) >> 8
            };
            if ea == 0 { return; }
            if ea >= 255 {
                let out = self.GetImage(proof).SetPixel(xu, yu);
                out[0] = color.GetRed();
                out[1] = color.GetGreen();
                out[2] = color.GetBlue();
                out[3] = 255;
                return;
            }
            use super::emColor::blend_channel_fused;
            let bg = self.read_pixel(proof, xu, yu);
            let a = ea as u8;
            let out = self.GetImage(proof).SetPixel(xu, yu);
            out[0] = blend_channel_fused(color.GetRed(), bg[0], a);
            out[1] = blend_channel_fused(color.GetGreen(), bg[1], a);
            out[2] = blend_channel_fused(color.GetBlue(), bg[2], a);
            out[3] = blend_channel_fused(255, bg[3], a);
        }
    }

    /// Same as `blend_with_coverage` but without clip/bounds checks.
    #[inline(always)]
    fn blend_with_coverage_unchecked(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor, cov: i32) {
        if cov <= 0 { return; }
        // C++ PaintScanlineCol: alpha = (color_alpha * opacity + 0x800) >> 12
        let alpha = ((color.GetAlpha() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8;
        if alpha == 0 { return; }
        let blended = color.SetAlpha(alpha);
        self.blend_pixel_unchecked(proof, x, y, blended);
    }

    /// Fill a polygon with a texture using the scanline rasterizer.
    fn fill_polygon_aa_textured(
        &mut self,
        proof: DirectProof,
        vertices: &[(f64, f64)],
        texture: &emTexture,
        rule: WindingRule,
    ) {
        if vertices.len() < 3 {
            return;
        }

        let pixel_verts: Vec<(f64, f64)> = vertices
            .iter()
            .map(|&(x, y)| {
                (
                    x * self.state.scale_x + self.state.offset_x,
                    y * self.state.scale_y + self.state.offset_y,
                )
            })
            .collect();

        let rows = emPainterScanline::rasterize(&pixel_verts, self.state.clip.to_scanline_clip(), rule);

        // Pre-transform texture coordinates to pixel space.
        // Extract state values to avoid borrowing self through the loop.
        let px_texture = Self::build_pixel_texture(texture, &self.state);

        for (y, spans) in &rows {
            for span in spans {
                self.blit_span_textured(proof, *y, span, &px_texture);
            }
        }
    }

    /// Convert a emTexture's coordinates from local space to pixel space.
    fn build_pixel_texture<'t>(texture: &'t emTexture, state: &PainterState) -> PixelTexture<'t> {
        match texture {
            emTexture::SolidColor(c) => PixelTexture::Solid(*c),
            emTexture::LinearGradient {
                color_a,
                color_b,
                start,
                end,
            } => {
                // C++ emPainter_ScTl.cpp ScanlineTool::Init LINEAR_GRADIENT (lines 174-186)
                let tx1 = start.0 * state.scale_x + state.offset_x;
                let ty1 = start.1 * state.scale_y + state.offset_y;
                let tx2 = end.0 * state.scale_x + state.offset_x;
                let ty2 = end.1 * state.scale_y + state.offset_y;
                let mut nx = tx2 - tx1;
                let mut ny = ty2 - ty1;
                let nn = nx * nx + ny * ny;
                let f = if nn < 1e-3 {
                    0.0
                } else {
                    ((255_i64 << 24) as f64) / nn
                };
                nx *= f;
                ny *= f;
                let tx = (tx1 - 0.5) * nx + (ty1 - 0.5) * ny;
                PixelTexture::LinearGradient {
                    color_a: *color_a,
                    color_b: *color_b,
                    fp_tx: (tx as i64) - 0x7fffff,
                    fp_tdx: nx as i64,
                    fp_tdy: ny as i64,
                }
            }
            emTexture::RadialGradient {
                color_inner,
                color_outer,
                center,
                radius_x,
                radius_y,
            } => {
                let pcx = center.0 * state.scale_x + state.offset_x;
                let pcy = center.1 * state.scale_y + state.offset_y;
                let prx = (radius_x * state.scale_x).max(1e-3);
                let pry = (radius_y * state.scale_y).max(1e-3);
                let nx = (255_i64 << 23) as f64 / prx;
                let ny = (255_i64 << 23) as f64 / pry;
                let _ = grad_sqrt_table();
                PixelTexture::RadialGradient {
                    color_inner: *color_inner,
                    color_outer: *color_outer,
                    fp_tx: ((pcx - 0.5) * nx) as i64,
                    fp_ty: ((pcy - 0.5) * ny) as i64,
                    fp_tdx: nx as i64,
                    fp_tdy: ny as i64,
                }
            }
            emTexture::emImage {
                image,
                x,
                y,
                w,
                h,
                alpha,
                extension,
                quality: _,
            } => {
                // C++ emPainter_ScTl.cpp ScanlineTool::Init for IMAGE (lines 228-378)
                let iw = image.GetWidth() as f64;
                let ih = image.GetHeight() as f64;
                let tw = w * state.scale_x;
                let th = h * state.scale_y;
                let tx_px = x * state.scale_x + state.offset_x;
                let ty_px = y * state.scale_y + state.offset_y;
                let tdx_f64 = ((image.GetWidth() as i64) << 24) as f64 / tw;
                let tdy_f64 = ((image.GetHeight() as i64) << 24) as f64 / th;
                let fp_tdx = tdx_f64 as i64;
                let fp_tdy = tdy_f64 as i64;

                // C++ downscale detection: TDX > 0xFFFF00 || TDY > 0xFFFF00
                let is_area_sampled = fp_tdx > 0xFFFF00 || fp_tdy > 0xFFFF00;

                // C++ pre-reduction (emPainter_ScTl.cpp:314-337)
                let channels = image.GetChannelCount() as isize;
                let mut img_w = iw as i32;
                let mut img_h = ih as i32;
                let mut img_dx = channels;
                let mut img_dy = iw as isize * channels;
                let mut fp_tdx = fp_tdx;
                let mut fp_tdy = fp_tdy;
                let mut tdx_f64 = tdx_f64;
                let mut tdy_f64 = tdy_f64;
                let mut img_map_offset: usize = 0;

                if is_area_sampled {
                    // C++ downscaleQuality defaults to 3
                    let dq = 3i64;
                    // X pre-reduction
                    let n = ((fp_tdx / dq + 0xFFFFFF) >> 24) as i32;
                    if n > 1 {
                        let t = img_w;
                        if n <= t {
                            img_w = (t + n - 1) / n;
                            let off = t - (img_w - 1) * n - 1;
                            img_map_offset += (img_dx * (off as isize >> 1)) as usize;
                            img_dx *= n as isize;
                            tdx_f64 = (img_w as f64) * ((1i64 << 24) as f64) / tw;
                            fp_tdx = tdx_f64 as i64;
                        }
                    }
                    // Y pre-reduction
                    let n = ((fp_tdy / dq + 0xFFFFFF) >> 24) as i32;
                    if n > 1 {
                        let t = img_h;
                        if n <= t {
                            img_h = (t + n - 1) / n;
                            let off = t - (img_h - 1) * n - 1;
                            img_map_offset += (img_dy * (off as isize >> 1)) as usize;
                            img_dy *= n as isize;
                            tdy_f64 = (img_h as f64) * ((1i64 << 24) as f64) / th;
                            fp_tdy = tdy_f64 as i64;
                        }
                    }
                }

                let img_sx = img_w as isize * img_dx;
                let img_sy = img_h as isize * img_dy;

                // C++ emPainter_ScTl.cpp:296-311: near-1:1 pixel-aligned → NEAREST
                // After pre-reduction, re-check if the reduced TDX/TDY qualify
                // for NEAREST sampling (like paint_image_rect_textured does).
                let is_area_sampled = if is_area_sampled {
                    let near_1_to_1 = fp_tdx < 0x10000FF && fp_tdy < 0x10000FF
                        && fp_tdx > 0x0FFFF00 && fp_tdy > 0x0FFFF00
                        && ((tx_px * tdx_f64) as i64 + 0x800) & 0xFFF000 == 0
                        && ((ty_px * tdy_f64) as i64 + 0x800) & 0xFFF000 == 0;
                    !near_1_to_1
                } else {
                    false
                };

                // C++ TX origin depends on sampling mode
                let fp_tx;
                let fp_ty;
                if is_area_sampled {
                    // Area sampling: TX = (emInt64)(tx * tdx)
                    fp_tx = (tx_px * tdx_f64) as i64;
                    fp_ty = (ty_px * tdy_f64) as i64;
                } else {
                    // Nearest/bilinear: TX = (emInt64)((tx - 0.5) * tdx)
                    fp_tx = ((tx_px - 0.5) * tdx_f64) as i64;
                    fp_ty = ((ty_px - 0.5) * tdy_f64) as i64;
                }

                PixelTexture::emImage {
                    image,
                    alpha: *alpha,
                    extension: *extension,
                    sct_alpha: *alpha as u32,
                    have_alpha: *alpha < 255,
                    fp_tdx,
                    fp_tdy,
                    fp_tx,
                    fp_ty,
                    is_area_sampled,
                    img_w,
                    img_h,
                    img_dx,
                    img_dy,
                    img_sx,
                    img_sy,
                    img_map_offset,
                    inv_scale_x: iw / tw,
                    inv_scale_y: ih / th,
                    offset_x: tx_px,
                    offset_y: ty_px,
                }
            }
            emTexture::ImageColored {
                image,
                color,
                extension,
                quality,
            } => PixelTexture::ImageColored {
                image,
                color: *color,
                extension: *extension,
                quality: *quality,
                inv_scale_x: 1.0 / state.scale_x,
                inv_scale_y: 1.0 / state.scale_y,
                offset_x: state.offset_x,
                offset_y: state.offset_y,
            },
            emTexture::ImageColoredGradient { .. } => {
                // Handled by paint_rect_textured directly, not through polygon fill.
                PixelTexture::Solid(emColor::TRANSPARENT)
            },
        }
    }

    /// Sample a color from a pixel-space texture at the given pixel coordinates.
    fn sample_pixel_texture(texture: &PixelTexture, px: f64, py: f64) -> emColor {
        match texture {
            PixelTexture::Solid(c) => *c,
            PixelTexture::LinearGradient {
                color_a,
                color_b,
                fp_tx,
                fp_tdx,
                fp_tdy,
            } => {
                // C++ InterpolateLinearGradient (emPainter_ScTlIntGra.cpp:24-39)
                let x = (px - 0.5) as i64;
                let y = (py - 0.5) as i64;
                let t = x * fp_tdx + y * fp_tdy - fp_tx;
                let mut u = t >> 24;
                if (u as u64) > 255 {
                    u = !(u >> 48);
                }
                let g = u as u32;
                let inv_g = 255 - g;
                let mix = |a: u8, b: u8| -> u8 {
                    (blend_hash_lookup(a, inv_g as u8) as u16
                        + blend_hash_lookup(b, g as u8) as u16) as u8
                };
                emColor::rgba(
                    mix(color_a.GetRed(), color_b.GetRed()),
                    mix(color_a.GetGreen(), color_b.GetGreen()),
                    mix(color_a.GetBlue(), color_b.GetBlue()),
                    mix(color_a.GetAlpha(), color_b.GetAlpha()),
                )
            }
            PixelTexture::RadialGradient {
                color_inner,
                color_outer,
                fp_tx,
                fp_ty,
                fp_tdx,
                fp_tdy,
            } => {
                // C++ integer sqrt table lookup matching emPainter_ScTlIntGra.cpp.
                // px/py are pixel-center coords (col+0.5, row+0.5).
                let col = (px - 0.5) as i64;
                let row = (py - 0.5) as i64;
                let tx = col * fp_tdx - fp_tx;
                let ty = row * fp_tdy - fp_ty;
                // C++ bounds check: (emUInt64)val+(0xFF<<23) >= (0x1FE<<23)
                // Range [-LIMIT, LIMIT) via unsigned-addition trick.
                const HALF_RANGE: u64 = 0xFF << 23;
                const FULL_RANGE: u64 = 0x1FE << 23;
                if (tx as u64).wrapping_add(HALF_RANGE) >= FULL_RANGE
                    || (ty as u64).wrapping_add(HALF_RANGE) >= FULL_RANGE
                {
                    return color_outer.GetBlended(*color_inner, 0.0);
                }
                let tyty = ty * ty + ((1i64 << 45) - 1);
                let t_idx = ((tx * tx + tyty) >> 46) as usize;
                let table = grad_sqrt_table();
                let factor = if t_idx < GRAD_SQRT_TABLE_SIZE {
                    table[t_idx]
                } else {
                    255
                };
                // factor is 0–255: 0=center (inner), 255=edge (outer).
                // C++ PaintScanlineInt G1G2: hash(Color1, 255-g) + hash(Color2, g)
                // per channel. Must use TWO separate hash lookups to match C++
                // rounding (floor(a*k) + floor(b*k) ≠ floor((a+b)*k)).
                let g = factor;
                let inv_g = 255 - g;
                let mix = |a: u8, b: u8| -> u8 {
                    (blend_hash_lookup(a, inv_g) as u16
                        + blend_hash_lookup(b, g) as u16) as u8
                };
                emColor::rgba(
                    mix(color_inner.GetRed(), color_outer.GetRed()),
                    mix(color_inner.GetGreen(), color_outer.GetGreen()),
                    mix(color_inner.GetBlue(), color_outer.GetBlue()),
                    mix(color_inner.GetAlpha(), color_outer.GetAlpha()),
                )
            }
            PixelTexture::emImage {
                image,
                alpha,
                inv_scale_x,
                inv_scale_y,
                offset_x,
                offset_y,
                extension,
                ..
            } => {
                let lx = (px - offset_x) * inv_scale_x;
                let ly = (py - offset_y) * inv_scale_y;
                // Fallback: f64 bilinear sampling (used for upscaled or non-polygon paths)
                let sampled = emPainterInterpolation::sample_bilinear(image, lx, ly, *extension);
                if *alpha == 255 {
                    sampled
                } else {
                    // C++ PSF_INT_A: multiply each channel by alpha/255
                    let a = *alpha as u32;
                    emColor::rgba(
                        ((sampled.GetRed() as u32 * a * 257 + 0x8073) >> 16) as u8,
                        ((sampled.GetGreen() as u32 * a * 257 + 0x8073) >> 16) as u8,
                        ((sampled.GetBlue() as u32 * a * 257 + 0x8073) >> 16) as u8,
                        ((sampled.GetAlpha() as u32 * a * 257 + 0x8073) >> 16) as u8,
                    )
                }
            }
            PixelTexture::ImageColored {
                image,
                color,
                extension,
                quality,
                inv_scale_x,
                inv_scale_y,
                offset_x,
                offset_y,
            } => {
                let lx = (px - offset_x) * inv_scale_x;
                let ly = (py - offset_y) * inv_scale_y;
                let sampled = Self::sample_image_at(image, lx, ly, *extension, *quality);
                emColor::rgba(
                    ((sampled.GetRed() as u32 * color.GetRed() as u32 + 128) >> 8) as u8,
                    ((sampled.GetGreen() as u32 * color.GetGreen() as u32 + 128) >> 8) as u8,
                    ((sampled.GetBlue() as u32 * color.GetBlue() as u32 + 128) >> 8) as u8,
                    ((sampled.GetAlpha() as u32 * color.GetAlpha() as u32 + 128) >> 8) as u8,
                )
            }
        }
    }

    /// Sample an image at local coordinates using the given extension and quality.
    fn sample_image_at(
        image: &emImage,
        x: f64,
        y: f64,
        extension: super::emTexture::ImageExtension,
        quality: super::emTexture::ImageQuality,
    ) -> emColor {
        match quality {
            super::emTexture::ImageQuality::Nearest => {
                emPainterInterpolation::sample_nearest(image, x, y, extension)
            }
            _ => emPainterInterpolation::sample_bilinear(image, x, y, extension),
        }
    }

    /// Blit a single textured AA span matching C++ PaintScanlineInt G1G2.
    ///
    /// C++ integrates coverage INTO the gradient-to-color mapping:
    ///   o1 = (opacity * Color1.alpha + 127) / 255
    ///   a1 = ((255-g) * o1 + 0x800) >> 12
    ///   a2 = (g * o2 + 0x800) >> 12
    ///   pix = hash_255[((c1*a1 + c2*a2)*257 + 0x8073) >> 16]
    /// Then source-over blend with combined alpha a = a1 + a2.
    fn blit_span_textured(&mut self, proof: DirectProof, y: i32, span: &emPainterScanline::Span, texture: &PixelTexture) {
        let tw = self.target_width as i32;
        let th = self.target_height as i32;
        if y < 0 || y >= th { return; }

        let x_start = span.x_start.max(0);
        let x_end = span.x_end.min(tw);
        if x_start >= x_end { return; }

        match texture {
            PixelTexture::LinearGradient { color_a, color_b, fp_tx, fp_tdx, fp_tdy } => {
                self.blit_span_linear_gradient_g1g2(
                    proof, y, span, x_start, x_end,
                    *color_a, *color_b, *fp_tx, *fp_tdx, *fp_tdy,
                );
            }
            PixelTexture::RadialGradient { color_inner, color_outer, fp_tx, fp_ty, fp_tdx, fp_tdy } => {
                self.blit_span_radial_gradient_g1g2(
                    proof, y, span, x_start, x_end,
                    *color_inner, *color_outer, *fp_tx, *fp_ty, *fp_tdx, *fp_tdy,
                );
            }
            PixelTexture::emImage {
                image,
                sct_alpha,
                extension,
                have_alpha,
                fp_tdx,
                fp_tdy,
                fp_tx,
                fp_ty,
                is_area_sampled,
                img_w, img_h, img_dx, img_dy, img_sx, img_sy,
                img_map_offset,
                ..
            } if *is_area_sampled && *extension == ImageExtension::Repeat => {
                self.blit_span_image_area_sampled_tiled(
                    proof, y, span, x_start, x_end,
                    image, *sct_alpha, *have_alpha,
                    *fp_tdx, *fp_tdy, *fp_tx, *fp_ty,
                    *img_w, *img_h, *img_dx, *img_dy, *img_sx, *img_sy,
                    *img_map_offset,
                );
            }
            _ => {
                // Fallback: per-pixel texture evaluation (used for other texture types).
                let py = y as f64 + 0.5;
                for x in x_start..x_end {
                    let opacity = span_opacity_at(span, x, x_start, x_end);
                    if opacity == 0 { continue; }
                    let color = Self::sample_pixel_texture(texture, x as f64 + 0.5, py);
                    // blend_with_coverage_unchecked handles all opacity values correctly,
                    // including > 0x1000 for double-wound polygons.
                    self.blend_with_coverage_unchecked(proof, x, y, color, opacity);
                }
            }
        }
    }

    /// Linear gradient span matching C++ PaintScanlineInt G1G2 + InterpolateLinearGradient.
    ///
    /// C++ computes gradient parameter per pixel using 24-bit fixed-point:
    ///   t = x*TDX + y*TDY - TX; u = t>>24; clamp to [0,255]
    /// Then blends: a1 = ((255-g)*o1+0x800)>>12, a2 = (g*o2+0x800)>>12
    #[allow(clippy::too_many_arguments)]
    fn blit_span_linear_gradient_g1g2(
        &mut self, proof: DirectProof, y: i32,
        span: &emPainterScanline::Span, x_start: i32, x_end: i32,
        color1: emColor, color2: emColor,
        fp_tx: i64, fp_tdx: i64, fp_tdy: i64,
    ) {
        let c1r = color1.GetRed() as u32;
        let c1g = color1.GetGreen() as u32;
        let c1b = color1.GetBlue() as u32;
        let c2r = color2.GetRed() as u32;
        let c2g = color2.GetGreen() as u32;
        let c2b = color2.GetBlue() as u32;

        // C++ InterpolateLinearGradient: t = x*TDX + y*TDY - TX (per pixel, incremental)
        let row = y as i64;
        let mut t = x_start as i64 * fp_tdx + row * fp_tdy - fp_tx;

        let tw = self.target_width as usize;

        for x in x_start..x_end {
            let opacity = span_opacity_at(span, x, x_start, x_end);
            if opacity == 0 {
                t += fp_tdx;
                continue;
            }

            // C++ emPainter_ScTlIntGra.cpp:34: u = t>>24; if ((emUInt64)u>255) u=~(u>>48);
            let mut u = t >> 24;
            if (u as u64) > 255 {
                u = !(u >> 48);
            }
            let g = (u as u8) as u32;

            // C++ PaintScanlineInt G1G2, CHANNELS=1:
            let o1 = (opacity as u32 * color1.GetAlpha() as u32 + 127) / 255;
            let o2 = (opacity as u32 * color2.GetAlpha() as u32 + 127) / 255;
            let a1 = ((255 - g) * o1 + 0x800) >> 12;
            let a2 = (g * o2 + 0x800) >> 12;
            let a = a1 + a2;
            if a == 0 {
                t += fp_tdx;
                continue;
            }

            let pr = ((c1r * a1 + c2r * a2) * 257 + 0x8073) >> 16;
            let pg = ((c1g * a1 + c2g * a2) * 257 + 0x8073) >> 16;
            let pb = ((c1b * a1 + c2b * a2) * 257 + 0x8073) >> 16;

            let pix_r = (255 * pr * 257 + 0x8073) >> 16;
            let pix_g = (255 * pg * 257 + 0x8073) >> 16;
            let pix_b = (255 * pb * 257 + 0x8073) >> 16;

            let offset = (y as usize * tw + x as usize) * 4;
            let data = self.GetImage(proof).GetWritableMap();
            let dest = &mut data[offset..offset + 4];
            if a >= 255 {
                dest[0] = pix_r as u8;
                dest[1] = pix_g as u8;
                dest[2] = pix_b as u8;
            } else {
                let tb = (255 - a) * 257;
                dest[0] = (((dest[0] as u32 * tb + 0x8073) >> 16) + pix_r) as u8;
                dest[1] = (((dest[1] as u32 * tb + 0x8073) >> 16) + pix_g) as u8;
                dest[2] = (((dest[2] as u32 * tb + 0x8073) >> 16) + pix_b) as u8;
            }

            t += fp_tdx;
        }
    }

    /// Radial gradient span matching C++ PaintScanlineInt G1G2 exactly.
    #[allow(clippy::too_many_arguments)]
    fn blit_span_radial_gradient_g1g2(
        &mut self, proof: DirectProof, y: i32,
        span: &emPainterScanline::Span, x_start: i32, x_end: i32,
        color1: emColor, color2: emColor,
        fp_tx: i64, fp_ty: i64, fp_tdx: i64, fp_tdy: i64,
    ) {
        let table = grad_sqrt_table();
        let c1r = color1.GetRed() as u32;
        let c1g = color1.GetGreen() as u32;
        let c1b = color1.GetBlue() as u32;
        let c2r = color2.GetRed() as u32;
        let c2g = color2.GetGreen() as u32;
        let c2b = color2.GetBlue() as u32;

        // C++ ScanlineTool: ty = y * TDY - TY
        let row = y as i64;
        let ty = row * fp_tdy - fp_ty;
        // C++ range check: (emUInt64)ty+(0xFF<<23) >= (0x1FE<<23)
        // Matches range [-LIMIT, LIMIT) via unsigned-addition trick.
        const HALF_RANGE: u64 = 0xFF << 23;
        const FULL_RANGE: u64 = 0x1FE << 23;

        // Precompute ty*ty + rounding constant (matching C++ line 213)
        let ty_in_range = (ty as u64).wrapping_add(HALF_RANGE) < FULL_RANGE;
        let tyty = if ty_in_range { ty * ty + ((1i64 << 45) - 1) } else { 0 };

        let tw = self.target_width as usize;

        for x in x_start..x_end {
            let opacity = span_opacity_at(span, x, x_start, x_end);
            if opacity == 0 { continue; }

            // C++ PaintScanlineInt G1G2: o1 = (opacity * Color1.alpha + 127) / 255
            let o1 = (opacity as u32 * color1.GetAlpha() as u32 + 127) / 255;
            let o2 = (opacity as u32 * color2.GetAlpha() as u32 + 127) / 255;

            // C++ InterpolateRadialGradient: tx = x * TDX - TX
            let tx = x as i64 * fp_tdx - fp_tx;

            // C++ bounds: (emUInt64)tx+(0xFF<<23) < (0x1FE<<23)
            let g = if !ty_in_range || (tx as u64).wrapping_add(HALF_RANGE) >= FULL_RANGE {
                255u32
            } else {
                let t_idx = ((tx * tx + tyty) >> 46) as usize;
                if t_idx < GRAD_SQRT_TABLE_SIZE { table[t_idx] as u32 } else { 255 }
            };

            // C++ PaintScanlineInt G1G2, CHANNELS=1:
            //   a1 = ((255-g) * o1 + 0x800) >> 12
            //   a2 = (g * o2 + 0x800) >> 12
            let a1 = ((255 - g) * o1 + 0x800) >> 12;
            let a2 = (g * o2 + 0x800) >> 12;
            let a = a1 + a2;
            if a == 0 { continue; }

            // C++ pix = hR[((c1R*a1+c2R*a2)*257+0x8073)>>16] + ...
            // where hR is hash at component=255 → hash_255[v] = (255*v*257+0x8073)>>16 ≈ v
            let pr = ((c1r * a1 + c2r * a2) * 257 + 0x8073) >> 16;
            let pg = ((c1g * a1 + c2g * a2) * 257 + 0x8073) >> 16;
            let pb = ((c1b * a1 + c2b * a2) * 257 + 0x8073) >> 16;

            // hash_255[pr] = (255 * pr * 257 + 0x8073) >> 16
            let pix_r = (255 * pr * 257 + 0x8073) >> 16;
            let pix_g = (255 * pg * 257 + 0x8073) >> 16;
            let pix_b = (255 * pb * 257 + 0x8073) >> 16;

            // Source-over blend: dest = dest * ((255-a)*257) + pix
            let offset = (y as usize * tw + x as usize) * 4;
            let data = self.GetImage(proof).GetWritableMap();
            let dest = &mut data[offset..offset + 4];
            if a >= 255 {
                dest[0] = pix_r as u8;
                dest[1] = pix_g as u8;
                dest[2] = pix_b as u8;
            } else {
                let t = (255 - a) * 257;
                dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + pix_r) as u8;
                dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + pix_g) as u8;
                dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + pix_b) as u8;
            }
        }
    }

    /// Integer tiled area-sampled image span matching C++ InterpolateImageAreaSampled
    /// (EXTEND_TILED) + PaintScanlineInt (no gradient, optional HAVE_ALPHA).
    ///
    /// C++ separates interpolation (into a buffer) from blending (PaintScanlineInt).
    /// This fuses both steps: for each dest pixel, area-sample the tiled source image
    /// using C++ integer math, then blend into the output using PaintScanlineInt logic.
    #[allow(clippy::too_many_arguments)]
    fn blit_span_image_area_sampled_tiled(
        &mut self, proof: DirectProof, y: i32,
        span: &emPainterScanline::Span, x_start: i32, x_end: i32,
        image: &emImage, sct_alpha: u32, have_alpha: bool,
        fp_tdx: i64, fp_tdy: i64, fp_tx: i64, fp_ty: i64,
        img_w: i32, img_h: i32,
        img_dx: isize, img_dy: isize, img_sx: isize, img_sy: isize,
        img_map_offset: usize,
    ) {
        let channels = image.GetChannelCount() as usize;
        if img_w <= 0 || img_h <= 0 { return; }

        let img_map = &image.GetMap()[img_map_offset..];

        // C++ ODX/ODY: rational inverse of TDX/TDY for weight computation.
        let odx: u32 = if fp_tdx <= 0x200 { 0x7FFF_FFFF } else { (((1i64 << 40) - 1) / fp_tdx + 1) as u32 };
        let ody: u32 = if fp_tdy <= 0x200 { 0x7FFF_FFFF } else { (((1i64 << 40) - 1) / fp_tdy + 1) as u32 };

        // C++ tx = x * TDX - TX (for first pixel in span)
        let tx = x_start as i64 * fp_tdx - fp_tx;

        // C++ ox1 = ((0x1000000 - (tx & 0xffffff)) * (i64)odx + 0xffffff) >> 24
        let mut ox1: u32 = (((0x100_0000i64 - (tx & 0xFF_FFFF)) * odx as i64 + 0xFF_FFFF) >> 24) as u32;
        if odx == 0x7FFF_FFFF { ox1 = 0x7FFF_FFFF; }

        // C++ DEFINE_AND_SET_IMAGE_X(imgX, tx>>24, imgDX, imgSX) for tiled:
        //   imgX = ((tx>>24) * DX) % SX; if imgX < 0 { imgX += SX; }
        let mut img_x = ((tx >> 24) * img_dx as i64).rem_euclid(img_sx as i64) as isize;

        // C++ ty = y * TDY - TY
        let ty = y as i64 * fp_tdy - fp_ty;

        // C++ oy1 = ((0x1000000 - (ty & 0xffffff)) * (i64)ody + 0xffffff) >> 24
        let mut oy1: u32 = (((0x100_0000i64 - (ty & 0xFF_FFFF)) * ody as i64 + 0xFF_FFFF) >> 24) as u32;
        if oy1 >= 0x10000 || ody == 0x7FFF_FFFF { oy1 = 0x10000; }
        let oy1n: u32 = 0x10000 - oy1;

        // C++ DEFINE_AND_SET_IMAGE_Y(imgY1, ty>>24, imgDY, imgSY) for tiled:
        let img_y1 = ((ty >> 24) * img_dy as i64).rem_euclid(img_sy as i64) as isize;

        let tw = self.target_width as usize;

        // C++ cy: column accumulator (accumulated Y samples for current source column).
        // Represented as [r, g, b, a] u32 accumulators (up to 4 channels).
        let mut cy = [0u32; 4];

        // C++ ox: remaining fractional X weight from previous dest pixel.
        let mut ox: u32 = 0;

        for x in x_start..x_end {
            let opacity = span_opacity_at(span, x, x_start, x_end);

            // C++ cyx: row accumulator for this dest pixel, initialized with rounding.
            let mut cyx = [0x7F_FFFFu32; 4];

            // C++ oxs = 0x10000 (total X budget for this dest pixel)
            let mut oxs: u32 = 0x10000;

            while ox < oxs {
                // ADD_MUL_COLOR(cyx, cy, ox): cyx += cy * ox
                for ch in 0..channels {
                    cyx[ch] = cyx[ch].wrapping_add(cy[ch].wrapping_mul(ox));
                }
                oxs -= ox;

                // Compute cy for current source column (area-sample in Y direction).
                let mut img_y = img_y1;

                // Read first source row with oy1 weight.
                // C++ READ_PREMUL_MUL_COLOR(cy, p, oy1)
                let p_offset = (img_y + img_x) as usize;
                Self::read_premul_mul_color(&mut cy, img_map, p_offset, channels, oy1);

                // Add remaining source rows.
                let mut oys = oy1n;
                if oys > 0 {
                    // INCREMENT_IMAGE_Y: imgY += imgDY; if imgY >= imgSY { imgY = 0; }
                    img_y += img_dy;
                    if img_y >= img_sy { img_y = 0; }

                    if oys > ody {
                        // Accumulate full rows.
                        let mut ctmp = [0u32; 4];
                        loop {
                            let p_off = (img_y + img_x) as usize;
                            Self::add_read_premul_color(&mut ctmp, img_map, p_off, channels);
                            img_y += img_dy;
                            if img_y >= img_sy { img_y = 0; }
                            oys -= ody;
                            if oys <= ody { break; }
                        }
                        // ADD_MUL_COLOR(cy, ctmp, ody)
                        for ch in 0..channels {
                            cy[ch] = cy[ch].wrapping_add(ctmp[ch].wrapping_mul(ody));
                        }
                    }

                    // Add final partial row.
                    // ADD_READ_PREMUL_MUL_COLOR(cy, p, oys)
                    let p_off = (img_y + img_x) as usize;
                    Self::add_read_premul_mul_color(&mut cy, img_map, p_off, channels, oys);
                }

                // FINPREMUL_SHR_COLOR(cy, 8)
                Self::finpremul_shr_color(&mut cy, channels, 8);

                // INCREMENT_IMAGE_X: imgX += imgDX; if imgX >= imgSX { imgX = 0; }
                img_x += img_dx;
                if img_x >= img_sx { img_x = 0; }

                ox = ox1;
                ox1 = odx;
            }

            // ADD_MUL_COLOR(cyx, cy, oxs)
            for ch in 0..channels {
                cyx[ch] = cyx[ch].wrapping_add(cy[ch].wrapping_mul(oxs));
            }

            // WRITE_NO_ROUND_SHR_COLOR: s[ch] = cyx[ch] >> 24
            let mut s = [0u8; 4];
            for ch in 0..channels {
                s[ch] = (cyx[ch] >> 24) as u8;
            }
            ox -= oxs;

            // Skip blend if zero opacity.
            if opacity == 0 { continue; }

            // --- PaintScanlineInt blend (no gradient) ---
            // C++ HAVE_ALPHA: o = (opacity * sct.Alpha + 127) / 255
            // sct_alpha is raw 8-bit texture alpha (C++ sct.Alpha).
            let o: u32 = if have_alpha {
                (opacity as u32 * sct_alpha + 127) / 255
            } else {
                opacity as u32
            };

            let offset = (y as usize * tw + x as usize) * 4;
            let data = self.GetImage(proof).GetWritableMap();
            let dest = &mut data[offset..offset + 4];

            match channels {
                4 => {
                    if o < 0x1000 {
                        // Low opacity: scale channels.
                        let a = (s[3] as u32 * o + 0x800) >> 12;
                        if a == 0 { continue; }
                        let pix_r = blend_hash_lookup(255, ((s[0] as u32 * o + 0x800) >> 12) as u8);
                        let pix_g = blend_hash_lookup(255, ((s[1] as u32 * o + 0x800) >> 12) as u8);
                        let pix_b = blend_hash_lookup(255, ((s[2] as u32 * o + 0x800) >> 12) as u8);
                        if a >= 255 {
                            dest[0] = pix_r;
                            dest[1] = pix_g;
                            dest[2] = pix_b;
                            dest[3] = 255;
                        } else {
                            let t = (255 - a) * 257;
                            dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + pix_r as u32) as u8;
                            dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + pix_g as u32) as u8;
                            dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + pix_b as u32) as u8;
                            dest[3] = (((dest[3] as u32 * t + 0x8073) >> 16) + blend_hash_lookup(255, a as u8) as u32) as u8;
                        }
                    } else {
                        // High opacity: direct write or blend.
                        let a = s[3] as u32;
                        if a == 0 { continue; }
                        let pix_r = blend_hash_lookup(255, s[0]);
                        let pix_g = blend_hash_lookup(255, s[1]);
                        let pix_b = blend_hash_lookup(255, s[2]);
                        if a >= 255 {
                            dest[0] = pix_r;
                            dest[1] = pix_g;
                            dest[2] = pix_b;
                            dest[3] = 255;
                        } else {
                            let t = (255 - a) * 257;
                            dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + pix_r as u32) as u8;
                            dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + pix_g as u32) as u8;
                            dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + pix_b as u32) as u8;
                            dest[3] = (((dest[3] as u32 * t + 0x8073) >> 16) + blend_hash_lookup(255, a as u8) as u32) as u8;
                        }
                    }
                }
                3 => {
                    // 3-channel (RGB, no alpha channel in source).
                    // C++ PaintScanlineInt !HAVE_GC1 && !HAVE_GC2, CHANNELS=3:
                    // a = (255 * o + 0x800) >> 12
                    let a = if o < 0x1000 {
                        (255 * o + 0x800) >> 12
                    } else {
                        255
                    };
                    if a == 0 { continue; }
                    let (pix_r, pix_g, pix_b) = if o < 0x1000 {
                        (
                            blend_hash_lookup(255, ((s[0] as u32 * o + 0x800) >> 12) as u8),
                            blend_hash_lookup(255, ((s[1] as u32 * o + 0x800) >> 12) as u8),
                            blend_hash_lookup(255, ((s[2] as u32 * o + 0x800) >> 12) as u8),
                        )
                    } else {
                        (
                            blend_hash_lookup(255, s[0]),
                            blend_hash_lookup(255, s[1]),
                            blend_hash_lookup(255, s[2]),
                        )
                    };
                    if a >= 255 {
                        dest[0] = pix_r;
                        dest[1] = pix_g;
                        dest[2] = pix_b;
                    } else {
                        let t = (255 - a) * 257;
                        dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + pix_r as u32) as u8;
                        dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + pix_g as u32) as u8;
                        dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + pix_b as u32) as u8;
                    }
                }
                2 => {
                    // 2-channel (gray + alpha).
                    if o < 0x1000 {
                        let a = (s[1] as u32 * o + 0x800) >> 12;
                        if a == 0 { continue; }
                        let g_val = blend_hash_lookup(255, ((s[0] as u32 * o + 0x800) >> 12) as u8);
                        if a >= 255 {
                            dest[0] = g_val;
                            dest[1] = g_val;
                            dest[2] = g_val;
                            dest[3] = 255;
                        } else {
                            let t = (255 - a) * 257;
                            dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                            dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                            dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                            dest[3] = (((dest[3] as u32 * t + 0x8073) >> 16) + blend_hash_lookup(255, a as u8) as u32) as u8;
                        }
                    } else {
                        let a = s[1] as u32;
                        if a == 0 { continue; }
                        let g_val = blend_hash_lookup(255, s[0]);
                        if a >= 255 {
                            dest[0] = g_val;
                            dest[1] = g_val;
                            dest[2] = g_val;
                            dest[3] = 255;
                        } else {
                            let t = (255 - a) * 257;
                            dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                            dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                            dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                            dest[3] = (((dest[3] as u32 * t + 0x8073) >> 16) + blend_hash_lookup(255, a as u8) as u32) as u8;
                        }
                    }
                }
                _ => {
                    // 1-channel (grayscale, no alpha).
                    let a = if o < 0x1000 {
                        (255 * o + 0x800) >> 12
                    } else {
                        255
                    };
                    if a == 0 { continue; }
                    let g_val = if o < 0x1000 {
                        blend_hash_lookup(255, ((s[0] as u32 * o + 0x800) >> 12) as u8)
                    } else {
                        blend_hash_lookup(255, s[0])
                    };
                    if a >= 255 {
                        dest[0] = g_val;
                        dest[1] = g_val;
                        dest[2] = g_val;
                    } else {
                        let t = (255 - a) * 257;
                        dest[0] = (((dest[0] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                        dest[1] = (((dest[1] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                        dest[2] = (((dest[2] as u32 * t + 0x8073) >> 16) + g_val as u32) as u8;
                    }
                }
            }
        }
    }

    /// C++ READ_PREMUL_MUL_COLOR: read pixel, premultiply (for 2/4ch), multiply by weight.
    #[inline(always)]
    fn read_premul_mul_color(cy: &mut [u32; 4], map: &[u8], offset: usize, channels: usize, weight: u32) {
        match channels {
            4 => {
                let a = map[offset + 3] as u32 * weight;
                cy[3] = a;
                cy[0] = map[offset] as u32 * a;
                cy[1] = map[offset + 1] as u32 * a;
                cy[2] = map[offset + 2] as u32 * a;
            }
            3 => {
                cy[0] = map[offset] as u32 * weight;
                cy[1] = map[offset + 1] as u32 * weight;
                cy[2] = map[offset + 2] as u32 * weight;
            }
            2 => {
                let a = map[offset + 1] as u32 * weight;
                cy[1] = a;
                cy[0] = map[offset] as u32 * a;
            }
            _ => {
                cy[0] = map[offset] as u32 * weight;
            }
        }
    }

    /// C++ ADD_READ_PREMUL_COLOR: read pixel, premultiply (for 2/4ch), add to accumulator.
    #[inline(always)]
    fn add_read_premul_color(acc: &mut [u32; 4], map: &[u8], offset: usize, channels: usize) {
        match channels {
            4 => {
                let a = map[offset + 3] as u32;
                acc[3] = acc[3].wrapping_add(a);
                acc[0] = acc[0].wrapping_add(map[offset] as u32 * a);
                acc[1] = acc[1].wrapping_add(map[offset + 1] as u32 * a);
                acc[2] = acc[2].wrapping_add(map[offset + 2] as u32 * a);
            }
            3 => {
                acc[0] = acc[0].wrapping_add(map[offset] as u32);
                acc[1] = acc[1].wrapping_add(map[offset + 1] as u32);
                acc[2] = acc[2].wrapping_add(map[offset + 2] as u32);
            }
            2 => {
                let a = map[offset + 1] as u32;
                acc[1] = acc[1].wrapping_add(a);
                acc[0] = acc[0].wrapping_add(map[offset] as u32 * a);
            }
            _ => {
                acc[0] = acc[0].wrapping_add(map[offset] as u32);
            }
        }
    }

    /// C++ ADD_READ_PREMUL_MUL_COLOR: read pixel, premultiply (for 2/4ch), multiply by weight, add.
    #[inline(always)]
    fn add_read_premul_mul_color(acc: &mut [u32; 4], map: &[u8], offset: usize, channels: usize, weight: u32) {
        match channels {
            4 => {
                let a = map[offset + 3] as u32 * weight;
                acc[3] = acc[3].wrapping_add(a);
                acc[0] = acc[0].wrapping_add(map[offset] as u32 * a);
                acc[1] = acc[1].wrapping_add(map[offset + 1] as u32 * a);
                acc[2] = acc[2].wrapping_add(map[offset + 2] as u32 * a);
            }
            3 => {
                acc[0] = acc[0].wrapping_add(map[offset] as u32 * weight);
                acc[1] = acc[1].wrapping_add(map[offset + 1] as u32 * weight);
                acc[2] = acc[2].wrapping_add(map[offset + 2] as u32 * weight);
            }
            2 => {
                let a = map[offset + 1] as u32 * weight;
                acc[1] = acc[1].wrapping_add(a);
                acc[0] = acc[0].wrapping_add(map[offset] as u32 * a);
            }
            _ => {
                acc[0] = acc[0].wrapping_add(map[offset] as u32 * weight);
            }
        }
    }

    /// C++ FINPREMUL_SHR_COLOR(C, S): finalize premultiplication with shift.
    /// For 4ch: RGB = (x + 0x7F7F) / 0xFF00, A = (x + 0x7F) >> 8
    /// For 3ch/1ch: all = (x + 0x7F) >> 8
    /// For 2ch: G = (x + 0x7F7F) / 0xFF00, A = (x + 0x7F) >> 8
    #[inline(always)]
    fn finpremul_shr_color(cy: &mut [u32; 4], channels: usize, s: u32) {
        // (((1<<S)>>1)-1) = (1 << (S-1)) - 1 = rounding bias
        // For S=8: rounding = 0x7F
        // (((0xff<<S)>>1)-1) = (0xFF << (S-1)) - 1 = 0x7F7F for S=8
        let round = (1u32 << (s - 1)) - 1; // 0x7F for S=8
        let round_premul = (0xFFu32 << (s - 1)) - 1; // 0x7F7F for S=8
        let div_premul = 0xFFu32 << s; // 0xFF00 for S=8
        match channels {
            4 => {
                cy[0] = (cy[0].wrapping_add(round_premul)) / div_premul;
                cy[1] = (cy[1].wrapping_add(round_premul)) / div_premul;
                cy[2] = (cy[2].wrapping_add(round_premul)) / div_premul;
                cy[3] = (cy[3].wrapping_add(round)) >> s;
            }
            3 => {
                cy[0] = (cy[0].wrapping_add(round)) >> s;
                cy[1] = (cy[1].wrapping_add(round)) >> s;
                cy[2] = (cy[2].wrapping_add(round)) >> s;
            }
            2 => {
                cy[0] = (cy[0].wrapping_add(round_premul)) / div_premul;
                cy[1] = (cy[1].wrapping_add(round)) >> s;
            }
            _ => {
                cy[0] = (cy[0].wrapping_add(round)) >> s;
            }
        }
    }

    /// Generate polygon vertices approximating an ellipse.
    fn ellipse_polygon(&self, cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<(f64, f64)> {
        let segments = adaptive_circle_segments(rx, ry, self.state.scale_x, self.state.scale_y);
        // C++ pre-computes step = 2*PI/n, then uses step*i. Must match this
        // order of operations — `(2*PI/n)*i` is not f64-identical to `2*PI*i/n`.
        let step = 2.0 * std::f64::consts::PI / segments as f64;
        let mut verts = Vec::with_capacity(segments);
        for i in 0..segments {
            let angle = step * i as f64;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        verts
    }

    // --- Coordinate transform helpers ---

    /// Build a 24-bit fixed-point scaling transform for image scaling.
    fn scale_transform_24(
        &self,
        src_w: u32,
        src_h: u32,
        dest_x: f64,
        dest_y: f64,
        dest_w: f64,
        dest_h: f64,
    ) -> emPainterInterpolation::ScaleTransform24 {
        let px = self.to_pixel_x(dest_x);
        let py = self.to_pixel_y(dest_y);
        let tw = dest_w * self.state.scale_x;
        let th = dest_h * self.state.scale_y;
        let tdx_f64 = ((src_w as i64) << 24) as f64 / tw;
        let tdy_f64 = ((src_h as i64) << 24) as f64 / th;
        let tdx = tdx_f64 as i64;
        let tdy = tdy_f64 as i64;
        let tx_sub = dest_x * self.state.scale_x + self.state.offset_x;
        let ty_sub = dest_y * self.state.scale_y + self.state.offset_y;
        let tx_origin = ((tx_sub - 0.5) * tdx_f64) as i64;
        let ty_origin = ((ty_sub - 0.5) * tdy_f64) as i64;
        emPainterInterpolation::ScaleTransform24 {
            tdx,
            tdy,
            base_x: px as i64 * tdx - tx_origin,
            base_y: py as i64 * tdy - ty_origin,
            px,
            py,
        }
    }

    /// Build a 24-bit fixed-point area sampling transform for downscaling.
    /// Matches C++ emPainter_ScTl Init (lines 323, 335, 338-341).
    ///
    /// Key difference from `scale_transform_24`: NO -0.5 pixel-center offset.
    fn area_sample_transform_24(
        &self,
        src_w: u32,
        src_h: u32,
        dest_x: f64,
        dest_y: f64,
        dest_w: f64,
        dest_h: f64,
    ) -> emPainterInterpolation::AreaSampleTransform {
        let tw = dest_w * self.state.scale_x;
        let th = dest_h * self.state.scale_y;
        let tdx_f64 = ((src_w as i64) << 24) as f64 / tw;
        let tdy_f64 = ((src_h as i64) << 24) as f64 / th;
        let tdx = tdx_f64 as i64;
        let tdy = tdy_f64 as i64;
        let tx_sub = dest_x * self.state.scale_x + self.state.offset_x;
        let ty_sub = dest_y * self.state.scale_y + self.state.offset_y;
        // NO -0.5 offset (contrast with scale_transform_24).
        let tx = (tx_sub * tdx_f64) as i64;
        let ty = (ty_sub * tdy_f64) as i64;
        let odx = if tdx <= 0x200 {
            0x7FFF_FFFF
        } else {
            (((1i64 << 40) - 1) / tdx + 1) as u32
        };
        let ody = if tdy <= 0x200 {
            0x7FFF_FFFF
        } else {
            (((1i64 << 40) - 1) / tdy + 1) as u32
        };
        emPainterInterpolation::AreaSampleTransform {
            tdx,
            tdy,
            tx,
            ty,
            odx,
            ody,
            img_w: src_w as i32,
            img_h: src_h as i32,
            stride_x: 1,
            stride_y: 1,
            off_x: 0,
            off_y: 0,
        }
    }

    fn to_pixel_x(&self, x: f64) -> i32 {
        (x * self.state.scale_x + self.state.offset_x) as i32
    }

    fn to_pixel_y(&self, y: f64) -> i32 {
        (y * self.state.scale_y + self.state.offset_y) as i32
    }

    // --- Pixel-level operations ---

    fn blend_pixel(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor) {
        let clip = self.state.clip;
        if x < clip.x1 as i32 || x >= clip.x2.ceil() as i32
            || y < clip.y1 as i32 || y >= clip.y2.ceil() as i32
        { return; }
        if x < 0 || y < 0 || x >= self.target_width as i32 || y >= self.target_height as i32 {
            return;
        }
        self.blend_pixel_unchecked(proof, x, y, color);
    }

    /// Write a horizontal span of pixels at full coverage with no per-pixel
    /// clip or bounds checks.  Caller must guarantee that `y` and `x1..x2`
    /// are within both the clip rect and the target image.
    #[inline]
    fn fill_span_blended(&mut self, proof: DirectProof, y: i32, x1: i32, x2: i32, color: emColor) {
        debug_assert!(x1 >= 0 && x2 <= self.target_width as i32);
        debug_assert!(y >= 0 && y < self.target_height as i32);
        debug_assert!(x1 < x2);

        let tw = self.target_width as usize;
        let row_base = y as usize * tw * 4;

        if color.IsOpaque() && self.state.alpha == 255 {
            let pixel = [color.GetRed(), color.GetGreen(), color.GetBlue(), 255u8];
            let data = self.GetImage(proof).GetWritableMap();
            for col in x1..x2 {
                let off = row_base + col as usize * 4;
                data[off..off + 4].copy_from_slice(&pixel);
            }
        } else if self.state.canvas_color.IsOpaque() {
            let combined_alpha = if self.state.alpha == 255 {
                color.GetAlpha()
            } else {
                ((color.GetAlpha() as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if combined_alpha == 0 {
                return;
            }
            // Use hash table lookups to match C++ PaintScanlineCol exactly.
            use super::emColor::blend_hash_lookup;
            let a = combined_alpha;
            let cv = self.state.canvas_color;
            let cr = blend_hash_lookup(cv.GetRed(), a) as i32;
            let cg = blend_hash_lookup(cv.GetGreen(), a) as i32;
            let cb = blend_hash_lookup(cv.GetBlue(), a) as i32;
            let pm_r = blend_hash_lookup(color.GetRed(), a) as i32;
            let pm_g = blend_hash_lookup(color.GetGreen(), a) as i32;
            let pm_b = blend_hash_lookup(color.GetBlue(), a) as i32;
            let delta_r = pm_r - cr;
            let delta_g = pm_g - cg;
            let delta_b = pm_b - cb;
            let data = self.GetImage(proof).GetWritableMap();
            for col in x1..x2 {
                let off = row_base + col as usize * 4;
                data[off] = (data[off] as i32 + delta_r).clamp(0, 255) as u8;
                data[off + 1] = (data[off + 1] as i32 + delta_g).clamp(0, 255) as u8;
                data[off + 2] = (data[off + 2] as i32 + delta_b).clamp(0, 255) as u8;
            }
        } else {
            let ca = color.GetAlpha() as u16;
            let ea = if self.state.alpha == 255 {
                ca
            } else {
                (ca * self.state.alpha as u16 + 128) >> 8
            };
            if ea == 0 {
                return;
            }
            if ea >= 255 {
                let pixel = [color.GetRed(), color.GetGreen(), color.GetBlue(), 255u8];
                let data = self.GetImage(proof).GetWritableMap();
                for col in x1..x2 {
                    let off = row_base + col as usize * 4;
                    data[off..off + 4].copy_from_slice(&pixel);
                }
            } else {
                // Fused source-over matching C++ AVX2 PaintScanlineCol.
                use super::emColor::blend_channel_fused;
                let alpha = ea as u8;
                let cr = color.GetRed();
                let cg = color.GetGreen();
                let cb = color.GetBlue();
                let data = self.GetImage(proof).GetWritableMap();
                for col in x1..x2 {
                    let off = row_base + col as usize * 4;
                    data[off] = blend_channel_fused(cr, data[off], alpha);
                    data[off + 1] = blend_channel_fused(cg, data[off + 1], alpha);
                    data[off + 2] = blend_channel_fused(cb, data[off + 2], alpha);
                    data[off + 3] = blend_channel_fused(255, data[off + 3], alpha);
                }
            }
        }
    }

    fn fill_rect_pixels(&mut self, proof: DirectProof, x: i32, y: i32, w: i32, h: i32, color: emColor) {
        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = x.max(cx1);
        let start_y = y.max(cy1);
        let end_x = (x + w).min(cx2);
        let end_y = (y + h).min(cy2);

        if start_x >= end_x || start_y >= end_y {
            return;
        }

        // Fast path: fully opaque fill — bulk write rows directly.
        if color.IsOpaque() && self.state.alpha == 255 {
            let pixel = [color.GetRed(), color.GetGreen(), color.GetBlue(), 255u8];
            let tw = self.target_width as usize;
            let data = self.GetImage(proof).GetWritableMap();
            for row in start_y..end_y {
                let row_base = row as usize * tw * 4;
                for col in start_x..end_x {
                    let off = row_base + col as usize * 4;
                    data[off..off + 4].copy_from_slice(&pixel);
                }
            }
            return;
        }

        for row in start_y..end_y {
            self.fill_span_blended(proof, row, start_x, end_x, color);
        }
    }

    fn draw_line_pixels(&mut self, proof: DirectProof, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: emColor) {
        // Bresenham's line algorithm
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.blend_pixel(proof, x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}

/// Expand tab characters to spaces, aligning to 8-column tab stops.
/// Return the byte length of a UTF-8 character from its leading byte.
/// Matches the skip count of C++ `emDecodeChar` continuation byte handling.
fn utf8_char_len(lead: u8) -> usize {
    if lead < 0xC0 { 1 } // ASCII or continuation byte
    else if lead < 0xE0 { 2 }
    else if lead < 0xF0 { 3 }
    else { 4 }
}

/// Choose number of polygon segments for circle approximation.
/// Matches C++ emPainter: `f = CircleQuality * sqrt(rx*ScaleX + ry*ScaleY)`,
/// clamped to [3, 256] and rounded.
fn adaptive_circle_segments(rx: f64, ry: f64, scale_x: f64, scale_y: f64) -> usize {
    let f = CIRCLE_QUALITY * (rx * scale_x + ry * scale_y).sqrt();
    if f <= 3.0 {
        3
    } else if f >= 256.0 {
        256
    } else {
        (f + 0.5) as usize
    }
}

/// Tessellate a cubic Bezier segment using the C++ algorithm: curvature-based
/// adaptive step count with uniform parametric stepping (Horner evaluation).
///
/// `s` is `ScaleX + ScaleY` for scale-aware quality.
/// `thickness` is the stroke width (0.0 for filled beziers). C++ adds
/// `thickness * 0.04` to the curvature term so thick strokes get more segments.
fn tessellate_cubic_cpp(
    out: &mut Vec<(f64, f64)>,
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    s: f64,
    thickness: f64,
) {
    let x1 = p0.0;
    let y1 = p0.1;
    // Control points relative to P0.
    let mut x2 = p1.0 - x1;
    let mut y2 = p1.1 - y1;
    let mut x3 = p2.0 - x1;
    let mut y3 = p2.1 - y1;
    let mut x4 = p3.0 - x1;
    let mut y4 = p3.1 - y1;

    // Determine segment count m.
    let m: usize = 'flat: {
        let ll = x4 * x4 + y4 * y4;
        if ll > 1e-280 {
            let l = ll.sqrt();
            if ((x2 * y4 - y2 * x4).abs() + (x3 * y4 - y3 * x4).abs()) * s <= l * 0.01 {
                break 'flat 1;
            }
        } else {
            let dx = x3 - x2;
            let dy = y3 - y2;
            let l = (dx * dx + dy * dy).sqrt();
            if l * s <= 0.01 {
                break 'flat 1;
            }
            if (x2 * dy - y2 * dx).abs() * s <= l * 0.01 {
                break 'flat 1;
            }
        }
        // Curvature-based segment count.
        let bx1 = x3 - 2.0 * x2;
        let by1 = y3 - 2.0 * y2;
        let bx2 = x2 - 2.0 * x3 + x4;
        let by2 = y2 - 2.0 * y3 + y4;
        let b = ((bx1 * bx1 + by1 * by1).sqrt() + (bx2 * bx2 + by2 * by2).sqrt()) * 3.0;
        let f = CIRCLE_QUALITY * ((b * 0.0228 + thickness * 0.04) * s).sqrt();
        if f >= 500.0 {
            500
        } else if f > 1.0 {
            (f + 0.5) as usize
        } else {
            1
        }
    };

    // Convert to power basis for Horner evaluation.
    x2 *= 3.0;
    y2 *= 3.0;
    x3 *= 3.0;
    y3 *= 3.0;
    x4 += x2 - x3;
    y4 += y2 - y3;
    x3 -= x2 + x2;
    y3 -= y2 + y2;

    let dt = 1.0 / m as f64;
    let mut t = 0.0;
    for _ in 0..m {
        let px = x1 + t * (x2 + t * (x3 + t * x4));
        let py = y1 + t * (y2 + t * (y3 + t * y4));
        out.push((px, py));
        t += dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emImage::emImage;

    fn make_painter<'a>(target: &'a mut emImage) -> emPainter<'a> {
        emPainter::new(target)
    }

    #[test]
    fn edge_correction_no_crash() {
        let mut img = emImage::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.PaintPolygon(
            &[(0.0, 0.0), (16.0, 0.0), (16.0, 16.0)],
            emColor::RED,
            emColor::TRANSPARENT,
        );
        p.PaintPolygon(
            &[(0.0, 0.0), (16.0, 16.0), (0.0, 16.0)],
            emColor::BLUE,
            emColor::TRANSPARENT,
        );
        p.PaintEdgeCorrection(0.0, 0.0, 16.0, 16.0, emColor::RED, emColor::BLUE);
    }

    #[test]
    fn edge_correction_transparent_noop() {
        let mut img = emImage::new(16, 16, 4);
        let mut p = make_painter(&mut img);
        p.PaintEdgeCorrection(0.0, 0.0, 10.0, 10.0, emColor::TRANSPARENT, emColor::RED);
        p.PaintEdgeCorrection(0.0, 0.0, 10.0, 10.0, emColor::RED, emColor::TRANSPARENT);
    }

    #[test]
    fn bezier_outline_paints_pixels() {
        let mut img = emImage::new(64, 64, 4);
        let mut p = make_painter(&mut img);
        let stroke = emStroke::new(emColor::WHITE, 2.0);
        // Stride-3 convention: 12 points = 4 cubic segments (closed path).
        let points = [
            (32.0, 10.0),
            (50.0, 10.0),
            (50.0, 32.0),
            (50.0, 32.0),
            (50.0, 54.0),
            (32.0, 54.0),
            (32.0, 54.0),
            (14.0, 54.0),
            (14.0, 32.0),
            (14.0, 32.0),
            (14.0, 10.0),
            (32.0, 10.0),
        ];
        p.PaintBezierOutline(&points, &stroke, emColor::TRANSPARENT);
        let px = img.GetPixel(32, 10);
        assert!(px[0] > 0 || px[1] > 0 || px[2] > 0);
    }

    #[test]
    fn line_radius_miter_no_arrow() {
        let stroke = emStroke::new(emColor::BLACK, 4.0);
        let butt = emStrokeEnd::butt();
        let r = emPainter::CalculateLinePointMinMaxRadius(4.0, &stroke, &butt, &butt);
        assert!((r - 10.0).abs() < 0.01, "miter: expected 10.0, got {r}");
    }

    #[test]
    fn line_radius_round_no_arrow() {
        let stroke = emStroke {
            join: super::super::emStroke::LineJoin::Round,
            ..emStroke::new(emColor::BLACK, 4.0)
        };
        let butt = emStrokeEnd::butt();
        let r = emPainter::CalculateLinePointMinMaxRadius(4.0, &stroke, &butt, &butt);
        assert!((r - 2.0).abs() < 0.01, "round: expected 2.0, got {r}");
    }

    #[test]
    fn line_radius_with_arrow() {
        let stroke = emStroke {
            join: super::super::emStroke::LineJoin::Round,
            ..emStroke::new(emColor::BLACK, 4.0)
        };
        let butt = emStrokeEnd::butt();
        let arrow = emStrokeEnd::new(StrokeEndType::Arrow);
        let r = emPainter::CalculateLinePointMinMaxRadius(4.0, &stroke, &arrow, &butt);
        let expected = (20.0f64 * 20.0 + 40.0 * 40.0).sqrt();
        assert!(
            (r - expected).abs() < 0.1,
            "arrow: expected {expected}, got {r}"
        );
    }

    #[test]
    fn polyline_without_arrows_solid() {
        let mut img = emImage::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        let stroke = emStroke::new(emColor::WHITE, 2.0);
        let verts = [(5.0, 5.0), (25.0, 5.0), (25.0, 25.0)];
        p.PaintPolylineWithoutArrows(&verts, &stroke, false, emColor::TRANSPARENT);
        let px = img.GetPixel(15, 5);
        assert!(px[0] > 0, "solid polyline should paint pixels");
    }

    #[test]
    fn paint_image_scaled_bilinear() {
        let mut src = emImage::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let v = ((x + y) * 32) as u8;
                let p = src.SetPixel(x, y);
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 255;
            }
        }
        let mut img = emImage::new(16, 16, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            16.0,
            16.0,
            &src,
            super::super::emTexture::ImageQuality::Bilinear,
            super::super::emTexture::ImageExtension::Clamp,
        );
        // Center pixel should be interpolated (non-zero).
        let px = img.GetPixel(8, 8);
        assert!(px[0] > 0, "scaled image should paint pixels");
    }

    #[test]
    fn paint_radial_gradient_fills() {
        let mut img = emImage::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_radial_gradient(
            16.0,
            16.0,
            12.0,
            12.0,
            emColor::WHITE,
            emColor::BLACK,
            emColor::TRANSPARENT,
        );
        let center = img.GetPixel(16, 16);
        assert!(center[0] > 200, "center should be near white");
    }

    fn make_gradient_src() -> emImage {
        let mut src = emImage::new(8, 8, 4);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let v = ((x + y) * 16).min(255) as u8;
                let p = src.SetPixel(x, y);
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 255;
            }
        }
        src
    }

    #[test]
    fn paint_image_scaled_bicubic() {
        let src = make_gradient_src();
        let mut img = emImage::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            32.0,
            32.0,
            &src,
            super::super::emTexture::ImageQuality::Bicubic,
            super::super::emTexture::ImageExtension::Clamp,
        );
        let px = img.GetPixel(16, 16);
        assert!(px[0] > 0, "bicubic: center pixel should be non-zero");
    }

    #[test]
    fn paint_image_scaled_lanczos() {
        let src = make_gradient_src();
        let mut img = emImage::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            32.0,
            32.0,
            &src,
            super::super::emTexture::ImageQuality::Lanczos,
            super::super::emTexture::ImageExtension::Clamp,
        );
        let px = img.GetPixel(16, 16);
        assert!(px[0] > 0, "lanczos: center pixel should be non-zero");
    }

    #[test]
    fn paint_image_scaled_adaptive() {
        let src = make_gradient_src();
        let mut img = emImage::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            32.0,
            32.0,
            &src,
            super::super::emTexture::ImageQuality::Adaptive,
            super::super::emTexture::ImageExtension::Clamp,
        );
        let px = img.GetPixel(16, 16);
        assert!(px[0] > 0, "adaptive: center pixel should be non-zero");
    }

    #[test]
    fn paint_image_scaled_area_sampled() {
        let src = make_gradient_src();
        let mut img = emImage::new(4, 4, 4);
        let mut p = make_painter(&mut img);
        // Downscale: 8x8 -> 4x4 (area sampling)
        p.paint_image_scaled(
            0.0,
            0.0,
            4.0,
            4.0,
            &src,
            super::super::emTexture::ImageQuality::AreaSampled,
            super::super::emTexture::ImageExtension::Clamp,
        );
        let px = img.GetPixel(2, 2);
        assert!(px[0] > 0, "area-sampled: center pixel should be non-zero");
    }

    /// Verify that recording + replay produces byte-identical output to
    /// direct rendering. This is the foundation for multi-threaded rendering
    /// correctness: if replay matches direct, then parallel replay also matches.
    #[test]
    fn draw_list_replay_matches_direct() {
        use crate::emPainterDrawList::DrawList;
        use crate::emRenderThreadPool::emRenderThreadPool;

        let w = 64u32;
        let h = 64u32;

        // --- Direct rendering (single-threaded, no recording) ---
        let mut direct_img = emImage::new(w, h, 4);
        direct_img.fill(crate::emColor::emColor::BLACK);
        {
            let mut p = emPainter::new(&mut direct_img);
            draw_test_scene(&mut p);
        }

        // --- Recording + single-tile replay ---
        let mut draw_list = DrawList::new();
        {
            let mut rec = emPainter::new_recording(w, h, draw_list.ops_mut());
            draw_test_scene(&mut rec);
        }
        let mut replay_img = emImage::new(w, h, 4);
        replay_img.fill(crate::emColor::emColor::BLACK);
        {
            let mut p = emPainter::new(&mut replay_img);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        assert_eq!(
            direct_img.GetMap(),
            replay_img.GetMap(),
            "recording + replay must produce byte-identical output to direct rendering"
        );

        // --- Multi-threaded replay (thread counts 1, 2, 4) ---
        for thread_count in [1, 2, 4] {
            let pool = emRenderThreadPool::new(thread_count);
            // Split into 4 tiles of 32x32
            let tile_size = 32u32;
            let cols = w / tile_size;
            let rows = h / tile_size;
            let tiles: Vec<(u32, u32)> = (0..rows)
                .flat_map(|r| (0..cols).map(move |c| (c, r)))
                .collect();
            let results: Vec<std::sync::Mutex<Option<emImage>>> = tiles
                .iter()
                .map(|_| std::sync::Mutex::new(None::<emImage>))
                .collect();
            let results_ref = &results;
            let tiles_ref = &tiles;
            let draw_list_ref = &draw_list;
            let ts = tile_size as f64;

            pool.CallParallel(
                |idx| {
                    let (col, row) = tiles_ref[idx];
                    let mut buf = emImage::new(tile_size, tile_size, 4);
                    buf.fill(crate::emColor::emColor::BLACK);
                    {
                        let mut p = emPainter::new(&mut buf);
                        draw_list_ref.replay(&mut p, (col as f64 * ts, row as f64 * ts));
                    }
                    *results_ref[idx].lock().unwrap() = Some(buf);
                },
                tiles.len(),
            );

            // Reconstruct the full image from tiles
            let mut composed = emImage::new(w, h, 4);
            composed.fill(crate::emColor::emColor::BLACK);
            for (idx, (col, row)) in tiles.iter().enumerate() {
                let tile_buf = results[idx].lock().unwrap();
                let tile = tile_buf.as_ref().unwrap();
                composed.copy_from_rect(
                    col * tile_size,
                    row * tile_size,
                    tile,
                    (0, 0, tile_size, tile_size),
                );
            }

            assert_eq!(
                direct_img.GetMap(),
                composed.GetMap(),
                "parallel replay with {} threads must match direct rendering",
                thread_count,
            );
        }
    }

    /// Draw a test scene with various primitives for recording/replay testing.
    fn draw_test_scene(p: &mut emPainter) {
        let red = emColor::rgba(255, 0, 0, 255);
        let green = emColor::rgba(0, 255, 0, 200);
        let blue = emColor::rgba(0, 0, 255, 180);
        let white = emColor::rgba(255, 255, 255, 255);
        let canvas = emColor::rgba(50, 50, 50, 255);

        // Background
        p.PaintRect(0.0, 0.0, 64.0, 64.0, canvas, emColor::BLACK);

        // Overlapping rectangles with transparency
        p.push_state();
        p.SetCanvasColor(canvas);
        p.PaintRect(5.0, 5.0, 30.0, 30.0, red, canvas);
        p.PaintRect(15.0, 15.0, 30.0, 30.0, green, canvas);

        // Ellipse
        p.PaintEllipse(48.0, 16.0, 12.0, 12.0, blue, canvas);

        // Text
        p.PaintText(2.0, 50.0, "Hi", 10.0, 1.0, white, canvas);

        // Polygon
        let verts = [(10.0, 40.0), (30.0, 35.0), (25.0, 55.0)];
        p.PaintPolygon(&verts, blue, canvas);

        p.pop_state();
    }
}

// ---------------------------------------------------------------------------
// RUST_ONLY: fixed.rs -- Fixed-point newtype for sub-pixel rasterization.
// C++ uses bare int with inline shifts (emPainter.cpp:358-374). Rust wraps
// in a newtype to prevent mixing fixed and integer values. ceil() and round()
// use i64 promotion to fix signed-overflow UB present in C++.
// ---------------------------------------------------------------------------

/// Fixed-point number with 12 fractional bits (4096 sub-pixel grid).
///
/// Used for sub-pixel anti-aliased rasterization. The 12-bit fractional
/// part gives 1/4096 precision, sufficient for high-quality AA coverage.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed12(i32);

const FRAC_BITS: i32 = 12;
const SCALE: i32 = 1 << FRAC_BITS; // 4096
const FRAC_MASK: i32 = SCALE - 1; // 0xFFF

impl Fixed12 {
    pub const ZERO: Fixed12 = Fixed12(0);
    pub const ONE: Fixed12 = Fixed12(SCALE);

    #[inline]
    pub fn from_raw(raw: i32) -> Self {
        Self(raw)
    }

    #[inline]
    pub fn raw(self) -> i32 {
        self.0
    }

    #[inline]
    pub fn from_f64(v: f64) -> Self {
        Self((v * SCALE as f64) as i32)
    }

    #[inline]
    pub fn from_i32(v: i32) -> Self {
        Self(v << FRAC_BITS)
    }

    #[inline]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }

    /// Integer part (truncates toward negative infinity).
    #[inline]
    pub fn to_i32(self) -> i32 {
        self.0 >> FRAC_BITS
    }

    /// Fractional part (lower 12 bits), always in [0, 4095].
    #[inline]
    pub fn frac(self) -> i32 {
        self.0 & FRAC_MASK
    }

    #[inline]
    pub fn floor(self) -> Self {
        Self(self.0 & !FRAC_MASK)
    }

    #[inline]
    pub fn ceil(self) -> Self {
        // DIVERGED: C++ uses `(raw + 0xFFF) & ~0xFFF` which is signed overflow UB for
        // raw > i32::MAX - 4095 (coordinates > 524,287 px). Rust promotes to i64 to
        // compute the correct answer instead of wrapping to a garbage value.
        Self(((self.0 as i64 + FRAC_MASK as i64) & !(FRAC_MASK as i64)) as i32)
    }

    #[inline]
    pub fn round(self) -> Self {
        // DIVERGED: C++ uses `(raw + 2048) & ~0xFFF` which is signed overflow UB for
        // raw > i32::MAX - 2048 (coordinates > 524,287 px). Rust promotes to i64 to
        // compute the correct answer instead of wrapping to a garbage value.
        Self(((self.0 as i64 + (SCALE >> 1) as i64) & !(FRAC_MASK as i64)) as i32)
    }
}

impl std::ops::Add for Fixed12 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Fixed12 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl std::ops::Sub for Fixed12 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::SubAssign for Fixed12 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl std::ops::Mul for Fixed12 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        // Use i64 intermediate to prevent overflow for values > 1024.
        Self(((self.0 as i64 * rhs.0 as i64) >> FRAC_BITS) as i32)
    }
}

impl std::ops::Neg for Fixed12 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl std::fmt::Display for Fixed12 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.to_f64())
    }
}

#[cfg(test)]
mod fixed12_tests {
    use super::*;

    #[test]
    fn round_trip_f64() {
        let v = Fixed12::from_f64(3.75);
        assert!((v.to_f64() - 3.75).abs() < 1e-6);
    }

    #[test]
    fn round_trip_i32() {
        let v = Fixed12::from_i32(42);
        assert_eq!(v.to_i32(), 42);
        assert_eq!(v.frac(), 0);
    }

    #[test]
    fn frac_bits() {
        let v = Fixed12::from_f64(1.5);
        assert_eq!(v.to_i32(), 1);
        assert_eq!(v.frac(), 2048); // 0.5 * 4096
    }

    #[test]
    fn arithmetic() {
        let a = Fixed12::from_f64(2.5);
        let b = Fixed12::from_f64(1.25);
        assert!((a + b).to_f64() - 3.75 < 1e-6);
        assert!((a - b).to_f64() - 1.25 < 1e-6);
        assert!(((a * b).to_f64() - 3.125).abs() < 1e-3);
    }

    #[test]
    fn negation() {
        let v = Fixed12::from_f64(3.5);
        assert!(((-v).to_f64() + 3.5).abs() < 1e-6);
    }

    #[test]
    fn floor_ceil_round() {
        let v = Fixed12::from_f64(2.7);
        assert_eq!(v.floor().to_i32(), 2);
        assert_eq!(v.ceil().to_i32(), 3);
        assert_eq!(v.round().to_i32(), 3);

        let v2 = Fixed12::from_f64(2.3);
        assert_eq!(v2.floor().to_i32(), 2);
        assert_eq!(v2.ceil().to_i32(), 3);
        assert_eq!(v2.round().to_i32(), 2);
    }

    #[test]
    fn negative_values() {
        let v = Fixed12::from_f64(-1.75);
        assert_eq!(v.to_i32(), -2); // Right-shift floors toward negative infinity
        assert!((v.to_f64() + 1.75).abs() < 1e-6);
    }

    #[test]
    fn overflow_mul_uses_i64() {
        // Values > 1024 would overflow i32 without i64 intermediate.
        let a = Fixed12::from_f64(2000.0);
        let b = Fixed12::from_f64(3.0);
        assert!(((a * b).to_f64() - 6000.0).abs() < 1.0);
    }

    #[test]
    fn constants() {
        assert_eq!(Fixed12::ZERO.to_i32(), 0);
        assert_eq!(Fixed12::ZERO.frac(), 0);
        assert_eq!(Fixed12::ONE.to_i32(), 1);
        assert_eq!(Fixed12::ONE.frac(), 0);
    }
}


#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_adaptive_circle_segments() {
        let mut p_rx: f64 = kani::any::<f64>();
        kani::assume(p_rx.is_finite());
        let mut p_ry: f64 = kani::any::<f64>();
        kani::assume(p_ry.is_finite());
        let mut p_scale_x: f64 = kani::any::<f64>();
        kani::assume(p_scale_x.is_finite());
        let mut p_scale_y: f64 = kani::any::<f64>();
        kani::assume(p_scale_y.is_finite());
        let _r = adaptive_circle_segments(p_rx, p_ry, p_scale_x, p_scale_y);
    }

    #[kani::proof]
    fn kani_private_emPainter_cut_arrow() {
        let mut p_x1: f64 = kani::any::<f64>();
        kani::assume(p_x1.is_finite());
        let mut p_y1: f64 = kani::any::<f64>();
        kani::assume(p_y1.is_finite());
        let mut p_x2: f64 = kani::any::<f64>();
        kani::assume(p_x2.is_finite());
        let mut p_y2: f64 = kani::any::<f64>();
        kani::assume(p_y2.is_finite());
        let mut p_r: f64 = kani::any::<f64>();
        kani::assume(p_r.is_finite());
        let mut p_l: f64 = kani::any::<f64>();
        kani::assume(p_l.is_finite());
        let _r = emPainter::cut_arrow(p_x1, p_y1, p_x2, p_y2, p_r, p_l);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_emPainter_cut_circle() {
        let mut p_x1: f64 = kani::any::<f64>();
        kani::assume(p_x1.is_finite());
        let mut p_y1: f64 = kani::any::<f64>();
        kani::assume(p_y1.is_finite());
        let mut p_x2: f64 = kani::any::<f64>();
        kani::assume(p_x2.is_finite());
        let mut p_y2: f64 = kani::any::<f64>();
        kani::assume(p_y2.is_finite());
        let mut p_r: f64 = kani::any::<f64>();
        kani::assume(p_r.is_finite());
        let mut p_l: f64 = kani::any::<f64>();
        kani::assume(p_l.is_finite());
        let mut p_s: f64 = kani::any::<f64>();
        kani::assume(p_s.is_finite());
        let mut p_semi: bool = kani::any::<bool>();
        let _r = emPainter::cut_circle(p_x1, p_y1, p_x2, p_y2, p_r, p_l, p_s, p_semi);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_emPainter_cut_diamond() {
        let mut p_x1: f64 = kani::any::<f64>();
        kani::assume(p_x1.is_finite());
        let mut p_y1: f64 = kani::any::<f64>();
        kani::assume(p_y1.is_finite());
        let mut p_x2: f64 = kani::any::<f64>();
        kani::assume(p_x2.is_finite());
        let mut p_y2: f64 = kani::any::<f64>();
        kani::assume(p_y2.is_finite());
        let mut p_r: f64 = kani::any::<f64>();
        kani::assume(p_r.is_finite());
        let mut p_l: f64 = kani::any::<f64>();
        kani::assume(p_l.is_finite());
        let mut p_semi: bool = kani::any::<bool>();
        let _r = emPainter::cut_diamond(p_x1, p_y1, p_x2, p_y2, p_r, p_l, p_semi);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_emPainter_cut_square() {
        let mut p_x1: f64 = kani::any::<f64>();
        kani::assume(p_x1.is_finite());
        let mut p_y1: f64 = kani::any::<f64>();
        kani::assume(p_y1.is_finite());
        let mut p_x2: f64 = kani::any::<f64>();
        kani::assume(p_x2.is_finite());
        let mut p_y2: f64 = kani::any::<f64>();
        kani::assume(p_y2.is_finite());
        let mut p_r: f64 = kani::any::<f64>();
        kani::assume(p_r.is_finite());
        let mut p_l: f64 = kani::any::<f64>();
        kani::assume(p_l.is_finite());
        let _r = emPainter::cut_square(p_x1, p_y1, p_x2, p_y2, p_r, p_l);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_emPainter_cut_triangle() {
        let mut p_x1: f64 = kani::any::<f64>();
        kani::assume(p_x1.is_finite());
        let mut p_y1: f64 = kani::any::<f64>();
        kani::assume(p_y1.is_finite());
        let mut p_x2: f64 = kani::any::<f64>();
        kani::assume(p_x2.is_finite());
        let mut p_y2: f64 = kani::any::<f64>();
        kani::assume(p_y2.is_finite());
        let mut p_r: f64 = kani::any::<f64>();
        kani::assume(p_r.is_finite());
        let mut p_l: f64 = kani::any::<f64>();
        kani::assume(p_l.is_finite());
        let _r = emPainter::cut_triangle(p_x1, p_y1, p_x2, p_y2, p_r, p_l);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_SubPixelEdges_new() {
        let mut p_dx_px: f64 = kani::any::<f64>();
        kani::assume(p_dx_px.is_finite());
        let mut p_dy_px: f64 = kani::any::<f64>();
        kani::assume(p_dy_px.is_finite());
        let mut p_dw_px: f64 = kani::any::<f64>();
        kani::assume(p_dw_px.is_finite());
        let mut p_dh_px: f64 = kani::any::<f64>();
        kani::assume(p_dh_px.is_finite());
        let _r = SubPixelEdges::new(p_dx_px, p_dy_px, p_dw_px, p_dh_px);
    }
}

#[cfg(test)]
mod tiny_rect_tests {
    use super::*;

    /// Compare composed sub-pixel PaintRect output against C++ reference values.
    /// C++ values generated by test_tiny_rect_compose.cpp with identical parameters.
    #[test]
    fn tiny_rect_compose_matches_cpp() {
        let cw = 100u32;
        let ch = 50u32;
        let mut img = crate::emImage::emImage::new(cw, ch, 4);
        let map = img.GetWritableMap();
        for i in 0..(cw * ch) as usize {
            map[i*4] = 128; map[i*4+1] = 128; map[i*4+2] = 128; map[i*4+3] = 255;
        }

        let text_color = crate::emColor::emColor::rgba(239, 240, 244, 56);
        let base_y: f64 = 24.7;
        let line_h: f64 = 0.275;
        let rects: &[(f64,f64,f64,f64)] = &[
            (10.0, base_y + 0.0*line_h, 5.2, line_h),
            (10.0, base_y + 1.0*line_h, 4.9, line_h),
            (10.0, base_y + 3.0*line_h, 10.4, line_h),
            (10.0, base_y + 4.0*line_h, 10.8, line_h),
            (10.0, base_y + 5.0*line_h, 10.4, line_h),
            (10.0, base_y + 7.0*line_h, 0.7, line_h),
            (10.0, base_y + 9.0*line_h, 10.9, line_h),
            (10.0, base_y + 10.0*line_h, 11.1, line_h),
        ];

        {
            let mut p = emPainter::new(&mut img);
            p.SetCanvasColor(crate::emColor::emColor::TRANSPARENT);
            for &(x, y, w, h) in rects {
                p.PaintRect(x, y, w, h, text_color, crate::emColor::emColor::TRANSPARENT);
            }
        }

        // Reference values matching C++ AVX2 PaintScanlineCol fused blend.
        // Original scalar C++ values differed by ±1 at sub-pixel boundaries.
        let cpp_y25: [(u8,u8,u8); 13] = [
            (144,144,144), (144,144,144), (144,144,144), (144,144,144),
            (144,144,144), (139,140,140), (139,140,140), (139,140,140),
            (139,140,140), (139,140,140), (135,135,135), (128,128,128), (128,128,128),
        ];

        let map = img.GetMap();
        let mut mismatches = Vec::new();
        for (i, &(er, eg, eb)) in cpp_y25.iter().enumerate() {
            let x = 10 + i;
            let off = (25 * cw as usize + x) * 4;
            let (ar, ag, ab) = (map[off], map[off+1], map[off+2]);
            let d = (ar as i16 - er as i16).unsigned_abs()
                .max((ag as i16 - eg as i16).unsigned_abs())
                .max((ab as i16 - eb as i16).unsigned_abs());
            if d > 0 {
                mismatches.push(format!(
                    "({},25): rust=({},{},{}) cpp=({},{},{}) diff=({:+},{:+},{:+})",
                    x, ar, ag, ab, er, eg, eb,
                    ar as i16 - er as i16, ag as i16 - eg as i16, ab as i16 - eb as i16
                ));
            }
        }

        if !mismatches.is_empty() {
            let max_diff = mismatches.len();
            panic!("Tiny rect compose: {} mismatches vs C++:\n{}", max_diff, mismatches.join("\n"));
        }
    }
}
