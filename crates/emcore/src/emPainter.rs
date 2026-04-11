use std::sync::OnceLock;

use crate::emPainterDrawList::DrawOp;
use super::emFontCache;
use super::emPainterInterpolation;
use super::emPainterScanline::{self, WindingRule};
use super::emPainterScanlineTool::{blend_colored_scanline, blend_scanline, blend_scanline_premul, BlendMode, InterpolationBuffer, MAX_INTERP_BYTES};
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
        start: (f64, f64),
        end: (f64, f64),
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
        extension: ImageExtension,
        quality: ImageQuality,
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
    DrawList(&'a mut Vec<DrawOp>),
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
        }
    }

    /// Create a painter in recording mode for the given viewport dimensions.
    ///
    /// Draw operations are captured into `ops` instead of rasterized.
    /// State management (push/pop, offset, clip) is tracked locally so
    /// that getters like `clip_is_empty()` and `canvas_color()` return
    /// correct values during the recording phase.
    pub fn new_recording(width: u32, height: u32, ops: &'a mut Vec<DrawOp>) -> Self {
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
        }
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
            ops.push(op);
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
            ops.push(op);
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
    pub fn GetClipX1(&self) -> bool {
        self.state.clip.IsEmpty()
    }

    /// Set origin (absolute offset, replaces current offset).
    pub fn SetOrigin(&mut self, x: f64, y: f64) {
        self.state.offset_x = x;
        self.state.offset_y = y;
    }

    /// Set scaling (absolute, replaces current scale).
    pub fn SetScaling(&mut self, sx: f64, sy: f64) {
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
        if w <= 0.0 || h <= 0.0 || color.GetAlpha() == 0 {
            return;
        }
        let Some(proof) = self.try_record(DrawOp::PaintRect {
            x,
            y,
            w,
            h,
            color,
            canvas_color,
        }) else { return; };
        // Literal port of C++ emPainter::PaintRect (emPainter.cpp:339-397).
        // Uses C++ ix/ixe/iy/iy2/ax1/ax2/ay1/ay2 formulas directly instead of
        // SubPixelEdges, because C++ uses truncation for iy2 while SubPixelEdges
        // uses ceil (and changing SubPixelEdges breaks 21 other tests).
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

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

        // C++ Fixed12 arithmetic (emPainter.cpp:358-379)
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
        let iy2 = iy2_raw >> 12; // C++ TRUNCATES (not ceil)
        if iy >= iy2 {
            ay1 += ay2 - 0x1000;
            ay2 = 0;
            if ay1 <= 0 {
                self.state.canvas_color = saved_canvas;
                return;
            }
        }

        // Top edge row (partial Y coverage)
        if ay1 < 0x1000 {
            let a1 = ((ax1 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay1 as i64 + 0x7ff) >> 12) as i32;
            self.paint_rect_scanline(proof, ix, iy, iw, a1, ay1, a2, color);
            iy += 1;
        }
        // Interior rows (full Y coverage)
        while iy < iy2 {
            self.paint_rect_scanline(proof, ix, iy, iw, ax1, 0x1000, ax2, color);
            iy += 1;
        }
        // Bottom edge row (partial Y coverage)
        if ay2 > 0 {
            let a1 = ((ax1 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            let a2 = ((ax2 as i64 * ay2 as i64 + 0x7ff) >> 12) as i32;
            self.paint_rect_scanline(proof, ix, iy, iw, a1, ay2, a2, color);
        }

        self.state.canvas_color = saved_canvas;
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
        let Some(_proof) = self.try_record(DrawOp::PaintEllipse {
            cx,
            cy,
            rx,
            ry,
            color,
            canvas_color,
        }) else { return; };
        let verts = self.ellipse_polygon(cx, cy, rx, ry);
        self.PaintPolygon(&verts, color, canvas_color);
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

    /// Draw a polyline (open path) outline by stroking each segment.
    pub fn PaintPolyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke_color: emColor,
        thickness: f64,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintPolyline {
            vertices: vertices.to_vec(),
            stroke_color,
            thickness,
            canvas_color,
        }) else { return; };
        if vertices.len() < 2 {
            return;
        }
        let half_w = thickness / 2.0;
        for i in 0..vertices.len() - 1 {
            let (x0, y0) = vertices[i];
            let (x1, y1) = vertices[i + 1];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                continue;
            }
            let nx = -dy / len * half_w;
            let ny = dx / len * half_w;
            self.PaintPolygon(
                &[
                    (x0 + nx, y0 + ny),
                    (x1 + nx, y1 + ny),
                    (x1 - nx, y1 - ny),
                    (x0 - nx, y0 - ny),
                ],
                stroke_color,
                canvas_color,
            );
        }
    }

    /// Fill a rounded rectangle using AA polygon approximation.
    /// Matches C++ `PaintRoundRect(x, y, w, h, rx, ry, texture, canvasColor)`.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintRoundRect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: emColor,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintRoundRect {
            x,
            y,
            w,
            h,
            radius,
            color,
            canvas_color,
        }) else { return; };
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let verts = self.round_rect_polygon(x, y, w, h, radius);
        self.fill_polygon_aa(proof, &verts, color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
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
        if image.GetChannelCount() != 4 || w <= 0.0 || h <= 0.0 || alpha == 0 {
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
        if image.GetChannelCount() != 4 || w <= 0.0 || h <= 0.0 || alpha == 0 {
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
        if w <= 0.0 || h <= 0.0 || src_w <= 0 || src_h <= 0 {
            return;
        }

        // --- C++ PaintRect boundary computation (emPainter.cpp:342-380) ---
        let dx_px = x * self.state.scale_x + self.state.offset_x;
        let dy_px = y * self.state.scale_y + self.state.offset_y;
        let dw_px = w * self.state.scale_x;
        let dh_px = h * self.state.scale_y;

        // X boundaries: ix with ceil for ixe (matching C++ ixe = ((int)(x2*0x1000))+0xfff)
        let sp = SubPixelEdges::new(dx_px, dy_px, dw_px, dh_px);
        let px = sp.ix1;
        let py = sp.iy1;
        let pw = sp.ix2 - sp.ix1;
        if pw <= 0 { return; }

        // Y boundaries: C++ PaintRect iy2=TRUNCATE (NOT ceil)
        let iy_raw = (dy_px * 4096.0) as i32;
        let iy2_raw = ((dy_px + dh_px) * 4096.0) as i32;
        let mut cpp_ay1 = 0x1000 - (iy_raw & 0xfff);
        let mut cpp_ay2 = iy2_raw & 0xfff;
        let cpp_iy1 = iy_raw >> 12;
        let cpp_iy2 = iy2_raw >> 12;
        if cpp_iy1 >= cpp_iy2 {
            cpp_ay1 += cpp_ay2 - 0x1000;
            cpp_ay2 = 0;
            if cpp_ay1 <= 0 { return; }
        }

        // Clip to viewport
        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = cpp_iy1.max(cy1);
        let end_x = sp.ix2.min(cx2);
        let end_y = if cpp_ay2 > 0 { (cpp_iy2 + 1).min(cy2) } else { cpp_iy2.min(cy2) };
        if start_x >= end_x || start_y >= end_y { return; }

        let src_w_f = src_w as f64;
        let src_h_f = src_h as f64;
        let ph = end_y - start_y;
        let upscaling = (pw as f64) > src_w_f || (ph as f64) > src_h_f;
        let downscaling = (pw as f64) < src_w_f || (ph as f64) < src_h_f;

        // --- C++ ScanlineTool::Init (emPainter_ScTl.cpp:228-378) ---
        let sec = emPainterInterpolation::SectionBounds {
            ox: src_x, oy: src_y, w: src_w, h: src_h,
        };

        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4);
        let max_batch = ibuf.max_pixels();
        let mut coverages = vec![0i32; max_batch];

        if downscaling {
            // C++ ScanlineTool downscale: pre-reduce + area sampling
            let sw_u = src_w as u32;
            let sh_u = src_h as u32;
            let tdx_init = ((sw_u as i64) << 24) as f64 / dw_px;
            let tdy_init = ((sh_u as i64) << 24) as f64 / dh_px;
            let tdx_i = tdx_init as i64;
            let tdy_i = tdy_init as i64;
            let stride_x = if tdx_i > 0xFFFF00 { ((tdx_i / 3 + 0xFFFFFF) >> 24) as u32 } else { 1 }.max(1);
            let stride_y = if tdy_i > 0xFFFF00 { ((tdy_i / 3 + 0xFFFFFF) >> 24) as u32 } else { 1 }.max(1);
            let red_w = sw_u.div_ceil(stride_x);
            let red_h = sh_u.div_ceil(stride_y);
            let off_x = (sw_u as i32 - (red_w as i32 - 1) * stride_x as i32 - 1) / 2;
            let off_y = (sh_u as i32 - (red_h as i32 - 1) * stride_y as i32 - 1) / 2;
            let mut xfm = self.area_sample_transform_24(red_w, red_h, x, y, w, h);
            xfm.stride_x = stride_x;
            xfm.stride_y = stride_y;
            xfm.off_x = off_x;
            xfm.off_y = off_y;

            for row in start_y..end_y {
                let mut carry = emPainterInterpolation::AreaSampleCarryState::new();
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    emPainterInterpolation::interpolate_scanline_area_sampled(
                        image, col, row, batch, &xfm, &sec, ext, &mut ibuf, &mut carry,
                    );
                    let all_full = sp.batch_coverages_cpp_y(
                        row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2,
                    );
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    if all_full { blend_scanline_premul(dest, &ibuf, batch, None, &mode); }
                    else { blend_scanline_premul(dest, &ibuf, batch, Some(&coverages[..batch]), &mode); }
                    col += batch as i32;
                }
            }
        } else {
            // C++ ScanlineTool upscale: adaptive interpolation with -0.5 pixel center
            let sxfm = self.scale_transform_24(src_w as u32, src_h as u32, x, y, w, h);

            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    if upscaling {
                        emPainterInterpolation::interpolate_scanline_adaptive_premul_section(
                            image, px, py, col, row, batch, &sxfm, &sec, ext, &mut ibuf,
                        );
                        let all_full = sp.batch_coverages_cpp_y(
                            row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2,
                        );
                        let dest_offset = (row as usize * tw + col as usize) * 4;
                        let data = self.GetImage(proof).GetWritableMap();
                        let dest = &mut data[dest_offset..];
                        if all_full { blend_scanline_premul(dest, &ibuf, batch, None, &mode); }
                        else { blend_scanline_premul(dest, &ibuf, batch, Some(&coverages[..batch]), &mode); }
                    } else {
                        emPainterInterpolation::interpolate_scanline_nearest(
                            image, px, py, col, row, batch, &sxfm, ext, &mut ibuf,
                        );
                        let all_full = sp.batch_coverages_cpp_y(
                            row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2,
                        );
                        let dest_offset = (row as usize * tw + col as usize) * 4;
                        let data = self.GetImage(proof).GetWritableMap();
                        let dest = &mut data[dest_offset..];
                        if all_full { blend_scanline(dest, &ibuf, batch, None, &mode); }
                        else { blend_scanline(dest, &ibuf, batch, Some(&coverages[..batch]), &mode); }
                    }
                    col += batch as i32;
                }
            }
        }
    }

    /// Draw an image with two-color mapping and canvas color support.
    /// Pixel luminance maps linearly from `color1` (at 0) to `color2` (at 255).
    /// For single-color alpha mask behavior, pass `emColor::TRANSPARENT` as color1.
    /// Source region is (src_x, src_y, src_w, src_h) within the image.
    /// Matches C++ `PaintImageColored(x, y, w, h, img, color1, color2, canvasColor)`.
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
        let Some(proof) = self.try_record(DrawOp::PaintImageColored {
            x,
            y,
            w,
            h,
            image_ptr: image as *const emImage,
            src_x,
            src_y,
            src_w,
            src_h,
            color1,
            color2,
            canvas_color,
            extension,
        }) else { return; };
        // Floating-point dest rect in pixel space (sub-pixel precision).
        let dx = x * self.state.scale_x + self.state.offset_x;
        let dy = y * self.state.scale_y + self.state.offset_y;
        let dw = w * self.state.scale_x;
        let dh = h * self.state.scale_y;

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
        let end_y = if cpp_ay2 > 0 { (cpp_iy2 + 1).min(cy2) } else { cpp_iy2.min(cy2) };
        let ph = end_y - start_y;

        if pw <= 0 || ph <= 0 || src_w == 0 || src_h == 0 {
            return;
        }

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

        let ch = image.GetChannelCount();

        // C++ emPainter uses area sampling for downscaling (DQ_3X3 default),
        // nearest-neighbor for upscaling/1:1 with pixel-center offset.
        let src_w_f = src_w as f64;
        let src_h_f = src_h as f64;
        let ratio_x = src_w_f / dw;
        let ratio_y = src_h_f / dh;
        let downscaling = ratio_x > 1.0 || ratio_y > 1.0;

        let ext = extension.resolve_for_colored(color1, color2);

        // Scanline batch pipeline for colored images.
        // Uses fused color-mapping + compositing (C++ PaintScanlineIntG1/G2/G1G2).
        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(ch);
        let max_batch = ibuf.max_pixels();
        let mut coverages = vec![0i32; max_batch];
        let mut lums = [0u8; MAX_INTERP_BYTES]; // max possible pixels when ch=1

        if downscaling {
            // 24fp area sampling matching C++ ScanlineTool InterpolateImageAreaSampled.
            let tdx_init = ((src_w as i64) << 24) as f64 / dw;
            let tdy_init = ((src_h as i64) << 24) as f64 / dh;
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
            let mut xfm = self.area_sample_transform_24(red_w, red_h, x, y, w, h);
            xfm.stride_x = stride_x;
            xfm.stride_y = stride_y;
            xfm.off_x = off_x;
            xfm.off_y = off_y;
            let sec = emPainterInterpolation::SectionBounds {
                ox: src_x as i32,
                oy: src_y as i32,
                w: src_w as i32,
                h: src_h as i32,
            };

            for row in start_y..end_y {
                let mut carry = emPainterInterpolation::AreaSampleCarryState::new();
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    emPainterInterpolation::interpolate_scanline_area_sampled(
                        image, col, row, batch, &xfm, &sec, ext, &mut ibuf, &mut carry,
                    );
                    // Extract luminance from interpolated data
                    for (i, lum) in lums[..batch].iter_mut().enumerate() {
                        let p = ibuf.pixel_rgba(i);
                        *lum = if ch == 1 {
                            p[0]
                        } else {
                            ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8)
                                as u8
                        };
                    }
                    let all_full =
                        sp.batch_coverages_cpp_y(row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    blend_colored_scanline(
                        dest,
                        &lums[..batch],
                        batch,
                        if all_full { None } else { Some(&coverages[..batch]) },
                        color1,
                        color2,
                        &mode,
                    );
                    col += batch as i32;
                }
            }
        } else {
            // Adaptive upscaling with lum extraction — matches C++ UQ_ADAPTIVE
            // for font glyph rendering (PaintImageColored with upscaling).
            let sxfm =
                self.scale_transform_24(src_w, src_h, x, y, w, h);
            let sec = emPainterInterpolation::SectionBounds {
                ox: src_x as i32,
                oy: src_y as i32,
                w: src_w as i32,
                h: src_h as i32,
            };
            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    for (i, lum) in lums[..batch].iter_mut().enumerate() {
                        let c = col + i as i32;
                        let tx64 = (c - px) as i64 * sxfm.tdx
                            + sxfm.base_x
                            - 0x180_0000;
                        let ty64 = (row - py) as i64 * sxfm.tdy
                            + sxfm.base_y
                            - 0x180_0000;
                        let src_ix = (tx64 >> 24) as i32;
                        let src_iy = (ty64 >> 24) as i32;
                        let ox =
                            (((tx64 & 0xFF_FFFF) as u32).wrapping_add(0x7FFF)) >> 16;
                        let oy =
                            (((ty64 & 0xFF_FFFF) as u32).wrapping_add(0x7FFF)) >> 16;

                        *lum = emPainterInterpolation::sample_adaptive_lum_section(
                            image, src_ix, src_iy, ox, oy, &sec, ext,
                        );
                    }
                    let all_full =
                        sp.batch_coverages_cpp_y(row, col, &mut coverages[..batch], cpp_iy1, cpp_iy2, cpp_ay1, cpp_ay2);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.GetImage(proof).GetWritableMap();
                    let dest = &mut data[dest_offset..];
                    blend_colored_scanline(
                        dest,
                        &lums[..batch],
                        batch,
                        if all_full { None } else { Some(&coverages[..batch]) },
                        color1,
                        color2,
                        &mode,
                    );
                    col += batch as i32;
                }
            }
        }

        self.state.canvas_color = saved_canvas;
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
        let Some(_proof) = self.try_record(DrawOp::PaintText {
            x,
            y,
            text: text.to_string(),
            char_height,
            width_scale,
            color,
            canvas_color,
        }) else { return; };

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
            return;
        }

        let clip_x1 = self.GetUserClipX1();
        let clip_x2 = self.GetUserClipX2();
        let clip_y1 = self.GetUserClipY1();
        let clip_y2 = self.GetUserClipY2();

        if y >= clip_y2 || y + char_height <= clip_y1 {
            return;
        }

        let gw = emFontCache::CHAR_WIDTH as f64;
        let gh = emFontCache::CHAR_HEIGHT as f64;
        let show_height = (rcw * gh / gw).min(char_height);
        let y_offset = (char_height - show_height) * 0.5;

        let saved_canvas = self.state.canvas_color;
        if canvas_color.IsOpaque() {
            self.state.canvas_color = canvas_color;
        }

        let font_atlas = emFontCache::atlas();

        let mut cx = x;
        for ch in text.chars() {
            if cx >= clip_x2 {
                break;
            }
            if cx + char_width <= clip_x1 {
                cx += char_width;
                continue;
            }

            // C++ PaintText renders ALL characters including space — no skip guard.
            let (src_x, src_y, src_w, src_h) = emFontCache::GetChar(ch);
            // C++ emPainter.cpp:2125 passes EXTEND_ZERO explicitly for font glyphs.
            self.PaintImageColored(
                cx,
                y + y_offset,
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
            cx += char_width;
        }

        self.state.canvas_color = saved_canvas;
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
    ///   - `min_width_scale`: minimum width squeeze factor (default 0.5).
    ///   - `formatted`: interpret `\n`, `\r\n`, `\t` (default true).
    ///   - `rel_line_space`: vertical space between lines in units of char_height.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintTextBoxed(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        text: &str,
        max_char_height: f64,
        color: emColor,
        canvas_color: emColor,
        box_h_align: TextAlignment,
        box_v_align: VAlign,
        text_alignment: TextAlignment,
        min_width_scale: f64,
        formatted: bool,
        rel_line_space: f64,
    ) {
        if text.is_empty() || w <= 0.0 || h <= 0.0 || max_char_height <= 0.0 {
            return;
        }
        let Some(_proof) = self.try_record(DrawOp::PaintTextBoxed {
            x,
            y,
            w,
            h,
            text: text.to_string(),
            max_char_height,
            color,
            canvas_color,
            box_h_align,
            box_v_align,
            text_alignment,
            min_width_scale,
            formatted,
            rel_line_space,
        }) else { return; };

        // Literal port of C++ PaintTextBoxed (emPainter.cpp:2174-2284).
        let (mut tw, mut th) =
            Self::GetTextSize(text, max_char_height, formatted, rel_line_space);
        if tw <= 0.0 {
            return;
        }

        let mut ch = max_char_height;

        if th > h {
            ch *= h / th;
            tw *= h / th;
            th = h;
        }
        let mut ws = w / tw;
        if ws < 1.0 {
            tw = w;
            if ws < min_width_scale {
                th *= ws / min_width_scale;
                ch *= ws / min_width_scale;
                ws = min_width_scale;
            }
        } else {
            ws = 1.0;
            if ws < min_width_scale {
                ws = min_width_scale;
                tw *= ws;
                if tw > w {
                    th *= w / tw;
                    ch *= w / tw;
                    tw = w;
                }
            }
        }

        let mut bx = x;
        if box_h_align != TextAlignment::Left {
            if box_h_align == TextAlignment::Right {
                bx += w - tw;
            } else {
                bx += (w - tw) * 0.5;
            }
        }
        let mut by = y;
        if box_v_align != VAlign::Top {
            if box_v_align == VAlign::Bottom {
                by += h - th + ch * rel_line_space;
            } else {
                by += (h - th + ch * rel_line_space) * 0.5;
            }
        }

        if formatted {
            // Literal port of C++ formatted rendering (emPainter.cpp:2222-2279).
            // Operates on raw bytes to match C++ byte-index column arithmetic.
            let cw = ch * ws / emFontCache::CHAR_BOX_TALLNESS;
            let bytes = text.as_bytes();
            let text_len = bytes.len();
            let mut ty = by;
            let mut i = 0usize;
            loop {
                let mut tx = bx;
                // Per-line text alignment: measure line width first.
                if text_alignment != TextAlignment::Left {
                    let mut j2 = i;
                    let mut cols2 = -(j2 as i32);
                    while j2 < text_len {
                        let c = bytes[j2];
                        if c <= 0x0d {
                            if c == 0x09 {
                                cols2 = ((cols2 + j2 as i32 + 8) & !7) - j2 as i32;
                            } else if c == 0x0a || c == 0x0d {
                                break;
                            }
                        } else if c >= 0x80 {
                            let n = utf8_char_len(c);
                            if n > 1 {
                                j2 += n - 1;
                                cols2 -= (n - 1) as i32;
                            }
                        }
                        j2 += 1;
                    }
                    cols2 += j2 as i32;
                    if text_alignment == TextAlignment::Right {
                        tx += tw - cols2 as f64 * cw;
                    } else {
                        tx += (tw - cols2 as f64 * cw) * 0.5;
                    }
                }

                // Render line segments (split by tabs).
                let mut cols = 0i32;
                let mut j = i;
                let mut k = -(i as i32);
                while i < text_len {
                    let c = bytes[i];
                    if c <= 0x0d {
                        if c == 0x09 {
                            // Tab: flush preceding text segment, advance to tab stop.
                            if j < i {
                                let seg = &text[j..i];
                                self.PaintText(
                                    tx + cols as f64 * cw,
                                    ty, seg, ch, ws, color, canvas_color,
                                );
                                cols += k + i as i32;
                            }
                            cols = (cols + 8) & !7;
                            j = i + 1;
                            k = -(j as i32);
                        } else if c == 0x0a || c == 0x0d {
                            break;
                        }
                    } else if c >= 0x80 {
                        let n = utf8_char_len(c);
                        if n > 1 {
                            i += n - 1;
                            k -= (n - 1) as i32;
                        }
                    }
                    i += 1;
                }
                // Flush remaining segment on this line.
                if j < i {
                    let seg = &text[j..i];
                    self.PaintText(
                        tx + cols as f64 * cw,
                        ty, seg, ch, ws, color, canvas_color,
                    );
                }

                // End of text?
                if i >= text_len {
                    break;
                }
                // Handle \r\n
                if bytes[i] == 0x0d && i + 1 < text_len && bytes[i + 1] == 0x0a {
                    i += 1;
                }
                i += 1;
                ty += ch * (1.0 + rel_line_space);
            }
        } else {
            self.PaintText(bx, by, text, ch, ws, color, canvas_color);
        }
    }

    /// Convenience: measure text width for a single un-formatted line.
    /// Returns the width in the same coordinate space as the painter.
    pub fn measure_text_width(text: &str, char_height: f64) -> f64 {
        char_height * text.chars().count() as f64 / emFontCache::CHAR_BOX_TALLNESS
    }

    /// Draw an image scaled to fill a destination rectangle.
    /// Auto-selects AreaSampled for downscaling.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_image_scaled(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &emImage,
        quality: super::emTexture::ImageQuality,
        extension: super::emTexture::ImageExtension,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let Some(proof) = self.try_record(DrawOp::PaintImageScaled {
            x,
            y,
            w,
            h,
            image_ptr: image as *const emImage,
            quality,
            extension,
        }) else { return; };

        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;
        if pw <= 0 || ph <= 0 {
            return;
        }

        let src_w = image.GetWidth() as f64;
        let src_h = image.GetHeight() as f64;

        // Auto-select area sampling for downscaling.
        let interp_quality = match quality {
            super::emTexture::ImageQuality::Nearest => emPainterInterpolation::InterpolationQuality::Nearest,
            super::emTexture::ImageQuality::Bilinear => {
                if src_w > pw as f64 || src_h > ph as f64 {
                    emPainterInterpolation::InterpolationQuality::AreaSampled
                } else {
                    emPainterInterpolation::InterpolationQuality::Bilinear
                }
            }
            super::emTexture::ImageQuality::AreaSampled => {
                emPainterInterpolation::InterpolationQuality::AreaSampled
            }
            super::emTexture::ImageQuality::Bicubic => emPainterInterpolation::InterpolationQuality::Bicubic,
            super::emTexture::ImageQuality::Lanczos => emPainterInterpolation::InterpolationQuality::Lanczos,
            super::emTexture::ImageQuality::Adaptive => emPainterInterpolation::InterpolationQuality::Adaptive,
        };

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = py.max(cy1);
        let end_x = (px + pw).min(cx2);
        let end_y = (py + ph).min(cy2);

        let ctx = emPainterInterpolation::ScaleContext {
            src_w,
            src_h,
            dest_w: pw as f64,
            dest_h: ph as f64,
        };

        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4);
        let max_batch = ibuf.max_pixels();

        for row in start_y..end_y {
            let mut col = start_x;
            while col < end_x {
                let batch = ((end_x - col) as usize).min(max_batch);
                for i in 0..batch {
                    let c = col + i as i32;
                    let src_x = (c - px) as f64 * src_w / pw as f64;
                    let src_y = (row - py) as f64 * src_h / ph as f64;
                    let color = emPainterInterpolation::sample(
                        image,
                        src_x,
                        src_y,
                        interp_quality,
                        extension,
                        &ctx,
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
    }

    // --- Bezier curves ---

    /// Fill a cubic Bezier curve region (tessellated to polygon).
    /// `points` length must be a multiple of 3. Uses stride-3 convention:
    /// segment i uses points[i*3], points[i*3+1], points[i*3+2], points[((i+1)*3) % n].
    /// The path is implicitly closed.
    pub fn PaintBezier(&mut self, points: &[(f64, f64)], color: emColor, canvas_color: emColor) {
        let Some(proof) = self.try_record(DrawOp::PaintBezier {
            points: points.to_vec(),
            color,
            canvas_color,
        }) else { return; };
        if points.len() < 3 {
            return;
        }
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
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
            self.fill_polygon_aa(proof, &verts, color, WindingRule::NonZero);
        }
        self.state.canvas_color = saved_canvas;
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

        // C++ PaintBezierLine (emPainter.cpp:1444-1479): compute arrow direction
        // vectors from the ORIGINAL control points, not from tessellated vertices.
        // Then replace the first/last tessellated segment direction to match.
        if !closed {
            let nv = verts.len();
            // Start direction: first control point → next non-coincident control point
            if stroke.start_end.IsDecorated() {
                for j in 1..n {
                    let dx = points[j].0 - points[0].0;
                    let dy = points[j].1 - points[0].1;
                    let ll = dx * dx + dy * dy;
                    if ll > 1e-280 {
                        let l = ll.sqrt();
                        let nx = dx / l;
                        let ny = dy / l;
                        // Replace verts[1] so the first segment has the correct direction.
                        // Use a tiny offset from verts[0] along (nx, ny) to preserve
                        // the segment while encoding the control-point direction.
                        let eps = 1e-10;
                        verts[1] = (verts[0].0 + nx * eps, verts[0].1 + ny * eps);
                        break;
                    }
                }
            }
            // End direction: last control point ← prev non-coincident control point
            if stroke.finish_end.IsDecorated() {
                let last = points[n - 1];
                for j in (0..n - 1).rev() {
                    let dx = points[j].0 - last.0;
                    let dy = points[j].1 - last.1;
                    let ll = dx * dx + dy * dy;
                    if ll > 1e-280 {
                        let l = ll.sqrt();
                        let nx = dx / l;
                        let ny = dy / l;
                        // Replace verts[nv-2] so the last segment has the correct direction.
                        let eps = 1e-10;
                        verts[nv - 2] = (last.0 + nx * eps, last.1 + ny * eps);
                        break;
                    }
                }
            }
        }

        self.PaintPolylineWithArrows(&verts, stroke, closed, canvas_color);
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
        let Some(proof) = self.try_record(DrawOp::PaintBorderImage {
            x,
            y,
            w,
            h,
            l,
            t,
            r,
            b,
            image_ptr: image as *const emImage,
            src_l,
            src_t,
            src_r,
            src_b,
            alpha,
            canvas_color,
            which_sub_rects,
        }) else { return; };

        // C++ PaintBorderImage (emPainter.cpp:1892-1982):
        // RoundX/RoundY adjustment, then 9 PaintImage calls (each = PaintRect).
        let iw_i = image.GetWidth() as i32;
        let ih_i = image.GetHeight() as i32;

        let Some(slices) = self.compute_border_image_slices(
            x, y, w, h, l, t, r, b,
            0, 0, iw_i, ih_i,
            src_l, src_t, src_r, src_b,
            canvas_color,
        ) else { return; };

        let saved_alpha = self.state.alpha;
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // C++ PaintBorderImage passes EXTEND_EDGE to each PaintImage call.
        let ext = super::emTexture::ImageExtension::Clamp;

        // C++ bit layout:  8=UL 5=U 2=UR / 7=L 4=C 1=R / 6=LL 3=B 0=LR
        // C++ order: 8, 5, 2, 7, 4, 1, 6, 3, 0 (matching emPainter.cpp:1910-1981)
        const BIT_ORDER: [u16; 9] = [1 << 8, 1 << 5, 1 << 2, 1 << 7, 1 << 4, 1 << 1, 1 << 6, 1 << 3, 1 << 0];
        // Map from C++ order to slice index: UL=0, U=1, UR=2, L=3, C=4, R=5, LL=6, B=7, LR=8
        const SLICE_ORDER: [usize; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        for i in 0..9 {
            let bit = BIT_ORDER[i];
            let si = SLICE_ORDER[i];
            if which_sub_rects & bit == 0 { continue; }
            // Center-row slices need dst_cy > 0; center-col slices need dst_cx > 0.
            let needs_cx = si == 1 || si == 4 || si == 7; // U, C, B
            let needs_cy = si == 3 || si == 4 || si == 5; // L, C, R
            if needs_cx && slices.dst_cx <= 0.0 { continue; }
            if needs_cy && slices.dst_cy <= 0.0 { continue; }
            let (dx, dy, dw, dh) = slices.target_rects[si];
            let (sx, sy, sw, sh) = slices.source_rects[si];
            self.paint_image_rect(proof, dx, dy, dw, dh, image, sx, sy, sw, sh, ext);
        }

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
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
        // Record uses the short-form DrawOp (src_l/src_t/src_r/src_b only).
        // This is acceptable because the draw list replay goes through the same
        // painter methods and the recording is for debugging/testing only.
        let Some(proof) = self.try_record(DrawOp::PaintBorderImage {
            x,
            y,
            w,
            h,
            l,
            t,
            r,
            b,
            image_ptr: image as *const emImage,
            src_l,
            src_t,
            src_r,
            src_b,
            alpha,
            canvas_color,
            which_sub_rects,
        }) else { return; };

        let Some(slices) = self.compute_border_image_slices(
            x, y, w, h, l, t, r, b,
            src_x, src_y, src_w, src_h,
            src_l, src_t, src_r, src_b,
            canvas_color,
        ) else { return; };

        let saved_alpha = self.state.alpha;
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        let ext = super::emTexture::ImageExtension::Clamp;

        const BIT_ORDER: [u16; 9] = [1 << 8, 1 << 5, 1 << 2, 1 << 7, 1 << 4, 1 << 1, 1 << 6, 1 << 3, 1 << 0];
        const SLICE_ORDER: [usize; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

        for i in 0..9 {
            let bit = BIT_ORDER[i];
            let si = SLICE_ORDER[i];
            if which_sub_rects & bit == 0 { continue; }
            let needs_cx = si == 1 || si == 4 || si == 7;
            let needs_cy = si == 3 || si == 4 || si == 5;
            if needs_cx && slices.dst_cx <= 0.0 { continue; }
            if needs_cy && slices.dst_cy <= 0.0 { continue; }
            let (dx, dy, dw, dh) = slices.target_rects[si];
            let (sx, sy, sw, sh) = slices.source_rects[si];
            self.paint_image_rect(proof, dx, dy, dw, dh, image, sx, sy, sw, sh, ext);
        }

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Draw a 9-slice border image with two-color tinting.
    ///
    /// `l,t,r,b` are **target** insets (logical coordinates).
    /// `src_l,src_t,src_r,src_b` are **source** insets (image pixel coordinates).
    #[allow(clippy::too_many_arguments)]
    pub fn PaintBorderImageColored(
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
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        which_sub_rects: u16,
        alpha: u8,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintBorderImageColored {
            x,
            y,
            w,
            h,
            l,
            t,
            r,
            b,
            image_ptr: image as *const emImage,
            src_l,
            src_t,
            src_r,
            src_b,
            color1,
            color2,
            canvas_color,
            which_sub_rects,
            alpha,
        }) else { return; };
        if alpha == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let iw = image.GetWidth() as f64;
        let ih = image.GetHeight() as f64;

        let mut l = l.min(w / 2.0);
        let mut r = r.min(w / 2.0);
        let mut t = t.min(h / 2.0);
        let mut b = b.min(h / 2.0);

        if !canvas_color.IsOpaque() {
            let f = self.RoundX(x + l) - x;
            if f > 0.0 && f < w - r {
                l = f;
            }
            let f = x + w - self.RoundX(x + w - r);
            if f > 0.0 && f < w - l {
                r = f;
            }
            let f = self.RoundY(y + t) - y;
            if f > 0.0 && f < h - b {
                t = f;
            }
            let f = y + h - self.RoundY(y + h - b);
            if f > 0.0 && f < h - t {
                b = f;
            }
        }

        let sl = src_l as f64;
        let st = src_t as f64;
        let sr = src_r as f64;
        let sb = src_b as f64;
        let src_cx = iw - sl - sr;
        let src_cy = ih - st - sb;
        let dst_cx = w - l - r;
        let dst_cy = h - t - b;

        let saved_alpha = self.state.alpha;
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // Corners.
        if which_sub_rects & (1 << 8) != 0 {
            self.PaintImageColored(
                x,
                y,
                l,
                t,
                image,
                0,
                0,
                sl as u32,
                st as u32,
                color1,
                color2,
                canvas_color,
                ImageExtension::Clamp,
            );
        }
        if which_sub_rects & (1 << 2) != 0 {
            self.PaintImageColored(
                x + w - r,
                y,
                r,
                t,
                image,
                (iw - sr) as u32,
                0,
                sr as u32,
                st as u32,
                color1,
                color2,
                canvas_color,
                ImageExtension::Clamp,
            );
        }
        if which_sub_rects & (1 << 6) != 0 {
            self.PaintImageColored(
                x,
                y + h - b,
                l,
                b,
                image,
                0,
                (ih - sb) as u32,
                sl as u32,
                sb as u32,
                color1,
                color2,
                canvas_color,
                ImageExtension::Clamp,
            );
        }
        if which_sub_rects & (1 << 0) != 0 {
            self.PaintImageColored(
                x + w - r,
                y + h - b,
                r,
                b,
                image,
                (iw - sr) as u32,
                (ih - sb) as u32,
                sr as u32,
                sb as u32,
                color1,
                color2,
                canvas_color,
                ImageExtension::Clamp,
            );
        }

        // Edges.
        if dst_cx > 0.0 {
            if which_sub_rects & (1 << 5) != 0 {
                self.PaintImageColored(
                    x + l,
                    y,
                    dst_cx,
                    t,
                    image,
                    sl as u32,
                    0,
                    src_cx as u32,
                    st as u32,
                    color1,
                    color2,
                    canvas_color,
                    ImageExtension::Clamp,
                );
            }
            if which_sub_rects & (1 << 3) != 0 {
                self.PaintImageColored(
                    x + l,
                    y + h - b,
                    dst_cx,
                    b,
                    image,
                    sl as u32,
                    (ih - sb) as u32,
                    src_cx as u32,
                    sb as u32,
                    color1,
                    color2,
                    canvas_color,
                    ImageExtension::Clamp,
                );
            }
        }
        if dst_cy > 0.0 {
            if which_sub_rects & (1 << 7) != 0 {
                self.PaintImageColored(
                    x,
                    y + t,
                    l,
                    dst_cy,
                    image,
                    0,
                    st as u32,
                    sl as u32,
                    src_cy as u32,
                    color1,
                    color2,
                    canvas_color,
                    ImageExtension::Clamp,
                );
            }
            if which_sub_rects & (1 << 1) != 0 {
                self.PaintImageColored(
                    x + w - r,
                    y + t,
                    r,
                    dst_cy,
                    image,
                    (iw - sr) as u32,
                    st as u32,
                    sr as u32,
                    src_cy as u32,
                    color1,
                    color2,
                    canvas_color,
                    ImageExtension::Clamp,
                );
            }
        }

        // Center.
        if which_sub_rects & (1 << 4) != 0 && dst_cx > 0.0 && dst_cy > 0.0 {
            self.PaintImageColored(
                x + l,
                y + t,
                dst_cx,
                dst_cy,
                image,
                sl as u32,
                st as u32,
                src_cx as u32,
                src_cy as u32,
                color1,
                color2,
                canvas_color,
                ImageExtension::Clamp,
            );
        }

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
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
        let segments = adaptive_circle_segments(rx, ry, self.state.scale_x, self.state.scale_y);
        let arc_segs =
            ((segments as f64 * abs_range / (2.0 * std::f64::consts::PI)).ceil() as usize).max(3);
        let mut verts = Vec::with_capacity(arc_segs + 1);
        for i in 0..=arc_segs {
            let t = i as f64 / arc_segs as f64;
            let angle = start_angle + t * range_angle;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        self.PaintSolidPolyline(&verts, stroke, false, canvas_color);
    }

    /// Draw an ellipse sector outline. Routes through polyline if dashed.
    #[allow(clippy::too_many_arguments)]
    /// Outline an ellipse sector. Angles in **degrees** (start + sweep), matching C++.
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
        if rx <= 0.0 || ry <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        if sweep_angle.abs() < 1e-10 {
            return;
        }
        // Convert degrees to radians.
        let start_rad = start_angle * std::f64::consts::PI / 180.0;
        let sweep_rad = sweep_angle * std::f64::consts::PI / 180.0;
        let segments = adaptive_circle_segments(rx, ry, self.state.scale_x, self.state.scale_y);
        let arc_segs = ((segments as f64 * sweep_rad.abs() / (2.0 * std::f64::consts::PI)).ceil()
            as usize)
            .max(2);
        let mut verts = Vec::with_capacity(arc_segs + 2);
        verts.push((cx, cy));
        for i in 0..=arc_segs {
            let t = i as f64 / arc_segs as f64;
            let angle = start_rad + t * sweep_rad;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        if stroke.is_dashed() {
            self.PaintPolylineWithoutArrows(&verts, stroke, true, canvas_color);
        } else {
            self.PaintPolygonOutline(&verts, stroke.color, stroke.width, canvas_color);
        }
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
                self.PaintRoundRect(x - t2, y - t2, w + sw, h + sw, t2, stroke.color, canvas_color);
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
    ///
    /// Matches C++ `PaintRoundRectOutline`: for solid strokes, builds outer +
    /// inner round-rect polygons with a bridge for NonZero winding hole.
    /// For dashed, routes through polyline.
    /// Reads canvas_color from painter state (set by caller).
    pub fn PaintRoundRectOutline(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        stroke: &emStroke,
    ) {
        if w <= 0.0 || h <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        let Some(proof) = self.try_record(DrawOp::PaintRoundRectOutline {
            x,
            y,
            w,
            h,
            radius,
            stroke: stroke.clone(),
        }) else { return; };
        let sw = stroke.width;
        let t2 = sw * 0.5;

        if stroke.is_dashed() {
            let verts = self.round_rect_polygon(x, y, w, h, radius);
            self.PaintPolylineWithoutArrows(&verts, stroke, true, self.state.canvas_color);
            return;
        }

        // Outer round-rect expanded by t2 on each side.
        let ox = x - t2;
        let oy = y - t2;
        let ow = w + sw;
        let oh = h + sw;
        let or = radius + t2;

        if sw * 2.0 >= w || sw * 2.0 >= h {
            self.PaintRoundRect(ox, oy, ow, oh, or, stroke.color, self.state.canvas_color);
            return;
        }

        // Inner round-rect contracted by t2 from shape boundary.
        let ix = ox + sw;
        let iy = oy + sw;
        let iw = ow - 2.0 * sw;
        let ih = oh - 2.0 * sw;
        let ir = (or - sw).max(0.0);

        let mut outer = self.round_rect_polygon(ox, oy, ow, oh, or);
        let inner = self.round_rect_polygon(ix, iy, iw, ih, ir);

        // Bridge + reversed inner for NonZero winding hole.
        // C++ vertex order: outer[0..n-1], outer[0], inner[0], inner[m-1..1], inner[0]
        outer.push(outer[0]);
        outer.push(inner[0]);
        outer.extend(inner.iter().rev());
        self.fill_polygon_aa(proof, &outer, stroke.color, WindingRule::NonZero);
    }

    /// Draw an ellipse outline. emStroke is centered on the shape boundary.
    ///
    /// Matches C++ `PaintEllipseOutline`: for solid strokes, builds
    /// outer + inner ellipse polygons with adaptive segment counts and a
    /// bridge for NonZero winding hole. For dashed, routes through polyline.
    pub fn PaintEllipseOutline(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        stroke: &emStroke,
        canvas_color: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintEllipseOutline {
            cx,
            cy,
            rx,
            ry,
            stroke: stroke.clone(),
            canvas_color,
        }) else { return; };
        if rx <= 0.0 || ry <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        let sw = stroke.width;
        let t2 = sw * 0.5;
        // Outer radii expanded by t2 (stroke centered on boundary).
        let orx = rx + t2;
        let ory = ry + t2;

        if stroke.is_dashed() {
            // Dashed: use centerline radii for the polyline.
            let verts = self.ellipse_polygon(cx, cy, rx, ry);
            self.PaintPolylineWithoutArrows(&verts, stroke, true, canvas_color);
            return;
        }

        // Inner radii contracted by t2 from shape boundary.
        let irx = orx - sw;
        let iry = ory - sw;
        if irx <= 0.0 || iry <= 0.0 {
            self.PaintEllipse(cx, cy, orx, ory, stroke.color, canvas_color);
            return;
        }

        // Build outer polygon with adaptive segment count.
        let mut outer = self.ellipse_polygon(cx, cy, orx, ory);

        // Build inner polygon (may have different segment count).
        let inner = self.ellipse_polygon(cx, cy, irx, iry);

        // Bridge + reversed inner for NonZero winding hole.
        // C++ vertex order: outer[0..n-1], outer[0], inner[0], inner[m-1..1], inner[0]
        outer.push(outer[0]);
        outer.push(inner[0]);
        outer.extend(inner.iter().rev());
        self.fill_polygon_aa(proof, &outer, stroke.color, WindingRule::NonZero);
    }

    /// Correct blending artifacts along a shared edge between two adjacent polygons.
    ///
    /// Walks along the edge using DDA stepping, computes area coverage for both
    /// sides, and blends a correction pixel. Corresponds to C++ `PaintEdgeCorrection`.
    #[allow(clippy::too_many_arguments)]
    pub fn PaintEdgeCorrection(
        &mut self,
        mut x1: f64,
        mut y1: f64,
        mut x2: f64,
        mut y2: f64,
        mut color1: emColor,
        mut color2: emColor,
    ) {
        let Some(proof) = self.try_record(DrawOp::PaintEdgeCorrection {
            x1,
            y1,
            x2,
            y2,
            color1,
            color2,
        }) else { return; };
        // Transform to pixel coordinates.
        x1 = x1 * self.state.scale_x + self.state.offset_x;
        y1 = y1 * self.state.scale_y + self.state.offset_y;
        x2 = x2 * self.state.scale_x + self.state.offset_x;
        y2 = y2 * self.state.scale_y + self.state.offset_y;

        // Ensure y1 <= y2 (C++ emPainter.cpp:740-744).
        if y1 > y2 {
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y1, &mut y2);
            std::mem::swap(&mut color1, &mut color2);
        }

        let dx = x2 - x1;
        let dy = y2 - y1;
        let adx = dx.abs();

        // C++ emPainter.cpp:750-753
        let gx = if dy >= 0.0001 { dx / dy } else { 0.0 };
        // C++ uses gy = dy/dx (signed), NOT dy/adx
        let gy = if adx >= 0.0001 { dy / dx } else { 0.0 };

        // Clip to viewport (C++ emPainter.cpp:755-790)
        let clip_x1f = self.state.clip.x1;
        let clip_y1f = self.state.clip.y1;
        let clip_x2f = self.state.clip.x2;
        let clip_y2f = self.state.clip.y2;

        if y1 < clip_y1f {
            if y2 <= clip_y1f { return; }
            x1 += (clip_y1f - y1) * gx;
            y1 = clip_y1f;
        }
        if y2 > clip_y2f {
            if y1 >= clip_y2f { return; }
            x2 += (clip_y2f - y2) * gx;
            y2 = clip_y2f;
        }

        let mut cx1;
        let mut cx2;
        let mut sx;
        if dx >= 0.0 {
            if x1 < clip_x1f {
                if x2 <= clip_x1f { return; }
                y1 += (clip_x1f - x1) * gy;
                x1 = clip_x1f;
            }
            if x2 > clip_x2f {
                if x1 >= clip_x2f { return; }
                y2 += (clip_x2f - x2) * gy;
                x2 = clip_x2f;
            }
            sx = x1 as i32;
            cx1 = x1;
            cx2 = x2;
        } else {
            if x2 < clip_x1f {
                if x1 <= clip_x1f { return; }
                y2 += (clip_x1f - x2) * gy;
                x2 = clip_x1f;
            }
            if x1 > clip_x2f {
                if x2 >= clip_x2f { return; }
                y1 += (clip_x2f - x1) * gy;
                x1 = clip_x2f;
            }
            sx = x1.ceil() as i32 - 1;
            cx1 = x2;
            cx2 = x1;
        }
        let mut sy = y1 as i32;
        let mut cy1 = y1;
        let mut cy2 = y2;

        // C++ emPainter.cpp:800-807: floor/ceil based on slope
        if adx > dy {
            cy1 = cy1.floor();
            cy2 = cy2.ceil();
        } else {
            cx1 = cx1.floor();
            cx2 = cx2.ceil();
        }

        if color1.IsTotallyTransparent() || color2.IsTotallyTransparent() {
            return;
        }

        // Pre-compute alpha channels (C++ emPainter.cpp:813-814)
        let ac1 = color1.GetAlpha() as f64 * (1.0 / 255.0);
        let ac2 = color2.GetAlpha() as f64 * (1.0 / 255.0);

        let tw = self.target_width as i32;
        let th = self.target_height as i32;

        // C++ hash tables map (color_channel, alpha) → premultiplied value.
        let h1 = [color1.GetRed(), color1.GetGreen(), color1.GetBlue()];
        let h2 = [color2.GetRed(), color2.GetGreen(), color2.GetBlue()];

        loop {
            // Pixel cell [px1,px2) x [py1,py2) clipped to coverage area
            let mut px1 = sx as f64;
            let mut py1 = sy as f64;
            let mut px2 = px1 + 1.0;
            let mut py2 = py1 + 1.0;
            if px1 < cx1 { px1 = cx1; }
            if py1 < cy1 { py1 = cy1; }
            if px2 > cx2 { px2 = cx2; }
            if py2 > cy2 { py2 = cy2; }

            // Clip line segment to this pixel cell (C++ emPainter.cpp:841-856)
            let mut qx1 = x1;
            let mut qy1 = y1;
            let mut qx2 = x2;
            let mut qy2 = y2;
            if qy1 < py1 { qx1 += (py1 - qy1) * gx; qy1 = py1; }
            if qy2 > py2 { qx2 += (py2 - qy2) * gx; qy2 = py2; }
            let mut a2;
            if dx >= 0.0 {
                if qx1 < px1 { qy1 += (px1 - qx1) * gy; qx1 = px1; }
                if qx2 > px2 { qy2 += (px2 - qx2) * gy; qx2 = px2; }
                a2 = py2 - qy2;
            } else {
                if qx2 < px1 { qy2 += (px1 - qx2) * gy; qx2 = px1; }
                if qx1 > px2 { qy1 += (px2 - qx1) * gy; qx1 = px2; }
                a2 = qy1 - py1;
            }

            // Exact trapezoid area (C++ emPainter.cpp:857-862)
            a2 = a2 * (px2 - px1) + (qy2 - qy1) * ((qx1 + qx2) * 0.5 - px1);
            let mut a1 = (py2 - py1) * (px2 - px1) - a2;
            a1 *= ac1;
            a2 *= ac2;

            if a1 >= 0.001 && a2 >= 0.001 {
                let t = 255.0 / ((1.0 - a1) * (1.0 - a2));
                let alpha1 = (a1 * a2 * (1.0 - a2) * t) as i32;
                let alpha2 = (a1 * a2 * a2 * t) as i32;
                let alpha3 = ((1.0 - a1 - a2) * t) as i32;

                if sx >= 0 && sx < tw && sy >= 0 && sy < th {
                    let bg = self.read_pixel(proof, sx as u32, sy as u32);
                    let out = self.GetImage(proof).SetPixel(sx as u32, sy as u32);
                    // C++ PEC_TEMPLATE: pixel = bg * hash[alpha3] + h1[alpha1] + h2[alpha2]
                    for ch in 0..3 {
                        let bg_term = if alpha3 > 0 {
                            blend_hash_lookup(bg[ch], alpha3 as u8) as i32
                        } else {
                            0
                        };
                        let c1_term = blend_hash_lookup(h1[ch], alpha1 as u8) as i32;
                        let c2_term = blend_hash_lookup(h2[ch], alpha2 as u8) as i32;
                        out[ch] = (bg_term + c1_term + c2_term).clamp(0, 255) as u8;
                    }
                }
            }

            // Step to next pixel cell (C++ emPainter.cpp:897-921)
            if dx >= 0.0 {
                if (sy as f64 + 1.0 - y1) * dx > (sx as f64 + 1.0 - x1) * dy {
                    sx += 1;
                    if (sx as f64) < cx2 { continue; }
                    break;
                }
            } else if (sy as f64 + 1.0 - y1) * dx < (sx as f64 - x1) * dy {
                sx -= 1;
                if sx as f64 + 1.0 > cx1 { continue; }
                break;
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

        if vertices.len() < 2 || stroke.width <= 0.0 {
            self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
            return;
        }

        // Route: if C++ dash_type API is set, use the fitted algorithm.
        if stroke.dash_type != DashType::Solid {
            self.paint_dashed_polyline_fitted(vertices, stroke, closed);
            return;
        }

        // Legacy pattern-based dashes.
        if stroke.dash_pattern.is_empty() {
            self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
            return;
        }
        let pattern = &stroke.dash_pattern;
        let total_pattern_len: f64 = pattern.iter().sum();
        if total_pattern_len <= 0.0 {
            self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
            return;
        }

        let n = vertices.len();
        let seg_count = if closed { n } else { n - 1 };
        let mut pat_idx = 0usize;
        let mut remaining_in_pat = pattern[0];
        let mut is_dash = true;
        let mut offset = stroke.dash_offset % total_pattern_len;

        while offset > 0.0 {
            if offset >= remaining_in_pat {
                offset -= remaining_in_pat;
                pat_idx = (pat_idx + 1) % pattern.len();
                remaining_in_pat = pattern[pat_idx];
                is_dash = pat_idx.is_multiple_of(2);
            } else {
                remaining_in_pat -= offset;
                offset = 0.0;
            }
        }

        let mut current_segment: Vec<(f64, f64)> = Vec::new();
        let dash_stroke = emStroke {
            dash_pattern: Vec::new(),
            dash_offset: 0.0,
            dash_type: DashType::Solid,
            ..stroke.clone()
        };

        for seg_i in 0..seg_count {
            let (x0, y0) = vertices[seg_i];
            let (x1, y1) = vertices[(seg_i + 1) % n];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let edge_len = (dx * dx + dy * dy).sqrt();
            if edge_len < 1e-10 {
                continue;
            }
            let ux = dx / edge_len;
            let uy = dy / edge_len;

            let mut consumed = 0.0;
            while consumed < edge_len {
                let available = edge_len - consumed;
                let step = remaining_in_pat.min(available);
                let px = x0 + ux * (consumed + step);
                let py = y0 + uy * (consumed + step);

                if is_dash {
                    if current_segment.is_empty() {
                        current_segment.push((x0 + ux * consumed, y0 + uy * consumed));
                    }
                    current_segment.push((px, py));
                }

                consumed += step;
                remaining_in_pat -= step;

                if remaining_in_pat <= 1e-10 {
                    if is_dash && current_segment.len() >= 2 {
                        self.PaintSolidPolyline(
                            &current_segment,
                            &dash_stroke,
                            false,
                            canvas_color,
                        );
                        current_segment.clear();
                    } else {
                        current_segment.clear();
                    }
                    pat_idx = (pat_idx + 1) % pattern.len();
                    remaining_in_pat = pattern[pat_idx];
                    is_dash = pat_idx.is_multiple_of(2);
                }
            }
        }

        if is_dash && current_segment.len() >= 2 {
            self.PaintSolidPolyline(&current_segment, &dash_stroke, false, canvas_color);
        }
    }

    /// C++ `PaintDashedPolyline` port: fits dashes to total path length.
    fn paint_dashed_polyline_fitted(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
    ) {
        use crate::emStroke::DashType;

        const MAX_DASHES: f64 = 1e5;

        let n = vertices.len();
        if n < 2 {
            self.PaintSolidPolyline(vertices, stroke, closed, self.state.canvas_color);
            return;
        }

        let thickness = stroke.width;
        let rounded = stroke.join == super::emStroke::LineJoin::Round
            || stroke.cap == super::emStroke::LineCap::Round;
        let have_dashes = stroke.dash_type != DashType::Dotted;
        let have_dots = stroke.dash_type != DashType::Dashed;
        let have_dashes_and_dots = have_dashes && have_dots;
        let is_endless = closed;

        let min_dash_len = if have_dashes {
            thickness
                * if rounded {
                    1.0 + MIN_REL_SEG_LEN
                } else {
                    MIN_REL_SEG_LEN
                }
        } else {
            0.0
        };
        let pref_dash_len = if have_dashes {
            min_dash_len.max(thickness * 5.0 * stroke.dash_length_factor)
        } else {
            0.0
        };
        let mut dot_len = if have_dots {
            thickness * (1.0 + MIN_REL_SEG_LEN)
        } else {
            0.0
        };
        let pref_gap_len = (thickness * 5.0 * stroke.gap_length_factor).max(0.0);
        let min_phase_len = min_dash_len + dot_len;
        let pref_phase_len = pref_dash_len + dot_len + pref_gap_len;

        // Compute total path length.
        let num_edges = if is_endless { n } else { n - 1 };
        let mut total_len = 0.0;
        let mut x2 = vertices[0].0;
        let mut y2 = vertices[0].1;
        for i in 1..=num_edges {
            let x1 = x2;
            let y1 = y2;
            let vi = vertices[i % n];
            x2 = vi.0;
            y2 = vi.1;
            let dx = x2 - x1;
            let dy = y2 - y1;
            total_len += (dx * dx + dy * dy).sqrt();
        }

        // Compute fitted dash/gap/stroke counts.
        let stroke_count: i32;
        let mut dash_len: f64;
        let mut gap_len: f64;
        let mut end_extra: f64;

        if is_endless {
            let max_stroke_count = MAX_DASHES.min(total_len / min_phase_len) as i32;
            if max_stroke_count < 1 {
                self.PaintSolidPolyline(vertices, stroke, closed, self.state.canvas_color);
                return;
            }
            stroke_count = (MAX_DASHES.min(total_len / pref_phase_len + 0.5) as i32)
                .max(1)
                .min(max_stroke_count);
            end_extra = 0.0;
            let t = total_len / stroke_count as f64 - dot_len;
            dash_len = min_dash_len.max(t / (pref_phase_len - dot_len) * pref_dash_len);
            gap_len = t - dash_len;
        } else {
            let mut t = total_len;
            if have_dashes {
                t += thickness.min(min_dash_len);
            } else {
                t += thickness;
            }
            if have_dashes_and_dots {
                t += dot_len;
            }
            let max_stroke_count = (MAX_DASHES.min(t / min_phase_len)) as i32;
            if max_stroke_count < 2 {
                self.PaintSolidPolyline(vertices, stroke, closed, self.state.canvas_color);
                return;
            }
            t = total_len + pref_gap_len;
            if have_dashes {
                t += thickness.min(pref_dash_len);
            } else {
                t += thickness;
            }
            if have_dashes_and_dots {
                t += dot_len;
            }
            stroke_count = (MAX_DASHES.min(t / pref_phase_len + 0.5) as i32)
                .max(2)
                .min(max_stroke_count);
            end_extra = thickness;
            if have_dashes {
                t = total_len + end_extra;
                if have_dots {
                    t -= (stroke_count - 1) as f64 * dot_len;
                }
                let u =
                    stroke_count as f64 * pref_dash_len + (stroke_count - 1) as f64 * pref_gap_len;
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
            if have_dashes_and_dots {
                t += dot_len;
            }
            gap_len = t / (stroke_count - 1) as f64;
            end_extra *= 0.5;
        }

        // Check if gap is too small at screen scale → render as solid with alpha.
        let t_gap = if rounded {
            gap_len + thickness * 0.215
        } else {
            gap_len
        };
        let s = self.state.scale_x + self.state.scale_y;
        if t_gap * s * 0.5 < 1.2 {
            let phase_len = dash_len + dot_len + gap_len;
            let t_solid = ((phase_len - t_gap) / phase_len).clamp(0.0, 1.0);
            if t_solid <= 0.0 {
                return;
            }
            let mut solid_stroke = stroke.clone();
            solid_stroke.dash_type = DashType::Solid;
            solid_stroke.dash_pattern.clear();
            let a = (stroke.color.GetAlpha() as f64 * t_solid + 0.5) as u8;
            solid_stroke.color = solid_stroke.color.SetAlpha(a);
            self.PaintSolidPolyline(vertices, &solid_stroke, closed, self.state.canvas_color);
            return;
        }

        let mut stroke_count = stroke_count;
        if have_dashes_and_dots {
            gap_len *= 0.5;
            stroke_count *= 2;
            if !is_endless {
                stroke_count -= 1;
            }
        }

        if rounded {
            end_extra = 0.0;
            if have_dashes {
                dash_len -= thickness;
            }
            if have_dots {
                dot_len -= thickness;
            }
            gap_len += thickness;
        }

        // Make a solid stroke for sub-segments.
        let mut solid_stroke = stroke.clone();
        solid_stroke.dash_type = DashType::Solid;
        solid_stroke.dash_pattern.clear();

        let cap_end = emStrokeEnd::new(StrokeEndType::Cap);
        let butt_end = emStrokeEnd::butt();

        // Walk the path, emitting dash sub-polylines.
        let mut is_in_stroke = false;
        let mut end_of_stroke_reached;
        let mut stroke_number = 1i32;
        let mut remaining_segment_len = 0.0f64;
        let mut remaining_edge_len = 0.0f64;
        let mut i: i32 = 0;
        x2 = vertices[0].0;
        y2 = vertices[0].1;
        let mut nx = 1.0f64;
        let mut ny = 0.0f64;
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
                remaining_edge_len = l.min(if have_dashes { dash_len } else { dot_len } * 0.5);
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
                let vi = vertices[i as usize % n];
                x2 = vi.0;
                y2 = vi.1;
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
                    if !is_in_stroke {
                        break;
                    }
                    end_of_stroke_reached = true;
                } else {
                    if !is_in_stroke {
                        continue;
                    }
                    end_of_stroke_reached = false;
                }
            }

            let x = x2 - nx * remaining_edge_len;
            let y = y2 - ny * remaining_edge_len;
            xy_out.push((x, y));

            if !is_in_stroke {
                is_in_stroke = true;
                remaining_segment_len = if have_dashes && (!have_dots || (stroke_number & 1) != 0) {
                    dash_len
                } else {
                    dot_len
                };
                if stroke_number == 1 {
                    remaining_segment_len -= end_extra;
                }
                continue;
            }

            if !end_of_stroke_reached {
                continue;
            }

            // Emit this dash sub-polyline.
            if xy_out.len() >= 2 {
                solid_stroke.start_end = if !is_endless && stroke_number == 1 {
                    stroke.start_end
                } else if rounded {
                    cap_end
                } else {
                    butt_end
                };
                solid_stroke.finish_end = if !is_endless && stroke_number == stroke_count {
                    stroke.finish_end
                } else if rounded {
                    cap_end
                } else {
                    butt_end
                };
                self.PaintSolidPolyline(&xy_out, &solid_stroke, false, self.state.canvas_color);
            }

            if stroke_number >= stroke_count {
                break;
            }
            stroke_number += 1;
            is_in_stroke = false;
            remaining_segment_len = gap_len;
            xy_out.clear();
        }
    }

    /// Dispatch polyline rendering: if dashed call dashed, else call solid.
    /// Corresponds to C++ `PaintPolylineWithoutArrows`.
    pub fn PaintPolylineWithoutArrows(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintPolylineWithoutArrows {
            vertices: vertices.to_vec(),
            stroke: stroke.clone(),
            closed,
            canvas_color,
        }) else { return; };
        if stroke.is_dashed() {
            self.PaintDashedPolyline(vertices, stroke, closed, canvas_color);
        } else {
            self.PaintSolidPolyline(vertices, stroke, closed, canvas_color);
        }
    }

    /// Dispatch polyline rendering with arrow support.
    /// Corresponds to C++ `PaintPolyline`: checks for arrow decorations,
    /// computes direction vectors, shortens endpoints, then paints arrows.
    pub fn PaintPolylineWithArrows(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &emStroke,
        closed: bool,
        canvas_color: emColor,
    ) {
        let Some(_proof) = self.try_record(DrawOp::PaintPolylineWithArrows {
            vertices: vertices.to_vec(),
            stroke: stroke.clone(),
            closed,
            canvas_color,
        }) else { return; };
        if vertices.len() < 2 {
            return;
        }
        let has_start_arrow = !closed && stroke.start_end.IsDecorated();
        let has_end_arrow = !closed && stroke.finish_end.IsDecorated();

        if !has_start_arrow && !has_end_arrow {
            self.PaintPolylineWithoutArrows(vertices, stroke, closed, canvas_color);
            return;
        }

        let n = vertices.len();

        // Find the first non-degenerate segment direction from the start.
        let (start_dx, start_dy) = {
            let mut dx = 0.0;
            let mut dy = 0.0;
            for i in 0..n - 1 {
                dx = vertices[i + 1].0 - vertices[i].0;
                dy = vertices[i + 1].1 - vertices[i].1;
                if dx * dx + dy * dy > 1e-20 {
                    break;
                }
            }
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-10 {
                (1.0, 0.0)
            } else {
                (dx / len, dy / len)
            }
        };

        // Find the last non-degenerate segment direction from the end.
        let (end_dx, end_dy) = {
            let mut dx = 0.0;
            let mut dy = 0.0;
            for i in (0..n - 1).rev() {
                dx = vertices[i + 1].0 - vertices[i].0;
                dy = vertices[i + 1].1 - vertices[i].1;
                if dx * dx + dy * dy > 1e-20 {
                    break;
                }
            }
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-10 {
                (1.0, 0.0)
            } else {
                (dx / len, dy / len)
            }
        };

        let rounded = stroke.join == super::emStroke::LineJoin::Round
            || stroke.cap == super::emStroke::LineCap::Round;

        // C++ PaintPolylineWithArrowsAlterBuf: iterate segments from each end,
        // test each segment against the decoration shape boundary via CutLineAtArrow.
        // When t < 1.0, interpolate the cut point on that segment and break.
        // When t >= 1.0, the entire segment is inside the decoration — skip it.
        let mut work_verts = vertices.to_vec();
        let mut p1: usize = 0;
        let mut p2: usize = n - 1;

        if has_start_arrow {
            let x1 = work_verts[0].0;
            let y1 = work_verts[0].1;
            while p1 < p2 {
                let ex1 = work_verts[p1].0 - x1;
                let ey1 = work_verts[p1].1 - y1;
                let ex2 = work_verts[p1 + 1].0 - x1;
                let ey2 = work_verts[p1 + 1].1 - y1;
                // Transform to decoration-local coords: rotate by (nx1, ny1)
                let t = Self::cut_line_at_arrow(
                    ex1 * start_dx + ey1 * start_dy,
                    ey1 * start_dx - ex1 * start_dy,
                    ex2 * start_dx + ey2 * start_dy,
                    ey2 * start_dx - ex2 * start_dy,
                    stroke.width,
                    stroke,
                    &stroke.start_end,
                );
                if t < 1.0 {
                    work_verts[p1].0 = (1.0 - t) * work_verts[p1].0 + t * work_verts[p1 + 1].0;
                    work_verts[p1].1 = (1.0 - t) * work_verts[p1].1 + t * work_verts[p1 + 1].1;
                    break;
                }
                p1 += 1;
            }
        }

        if has_end_arrow {
            let x2 = work_verts[p2].0;
            let y2 = work_verts[p2].1;
            while p2 > p1 {
                let ex1 = work_verts[p2].0 - x2;
                let ey1 = work_verts[p2].1 - y2;
                let ex2 = work_verts[p2 - 1].0 - x2;
                let ey2 = work_verts[p2 - 1].1 - y2;
                // Direction for end is negated (nx2, ny2 point into the line)
                let t = Self::cut_line_at_arrow(
                    ex1 * (-end_dx) + ey1 * (-end_dy),
                    ey1 * (-end_dx) - ex1 * (-end_dy),
                    ex2 * (-end_dx) + ey2 * (-end_dy),
                    ey2 * (-end_dx) - ex2 * (-end_dy),
                    stroke.width,
                    stroke,
                    &stroke.finish_end,
                );
                if t < 1.0 {
                    work_verts[p2].0 = (1.0 - t) * work_verts[p2].0 + t * work_verts[p2 - 1].0;
                    work_verts[p2].1 = (1.0 - t) * work_verts[p2].1 + t * work_verts[p2 - 1].1;
                    break;
                }
                p2 -= 1;
            }
        }

        // Paint the polyline body (only the non-skipped segment range).
        let body = &work_verts[p1..=p2];
        if body.len() >= 2 {
            self.PaintPolylineWithoutArrows(body, stroke, closed, canvas_color);
        }

        // Direction vectors point INTO the line (toward the interior).
        // Perpendicular = (dy, -dx) of the into-line direction, matching C++ convention.
        if has_start_arrow {
            let (x, y) = vertices[0];
            let nx = start_dy;
            let ny = -start_dx;
            self.paint_stroke_end(
                x,
                y,
                nx,
                ny,
                start_dx,
                start_dy,
                stroke.width,
                stroke.color,
                &stroke.start_end,
                rounded,
            );
        }

        if has_end_arrow {
            let (x, y) = vertices[n - 1];
            let nx = -end_dy;
            let ny = end_dx;
            self.paint_stroke_end(
                x,
                y,
                nx,
                ny,
                -end_dx,
                -end_dy,
                stroke.width,
                stroke.color,
                &stroke.finish_end,
                rounded,
            );
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
        // For width=1 with no decorations and no rounding, simple line.
        if stroke.width <= 1.0
            && !stroke.start_end.IsDecorated()
            && !stroke.finish_end.IsDecorated()
            && stroke.join != super::emStroke::LineJoin::Round
        {
            self.PaintLine(x0, y0, x1, y1, stroke.color, canvas_color);
            return;
        }

        // Route through the polyline system which handles caps, joins,
        // decorations, and dashes correctly — matching C++ PaintLine.
        let verts = [(x0, y0), (x1, y1)];
        self.PaintPolylineWithArrows(&verts, stroke, false, canvas_color);
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
    /// (x = along main direction, y = perpendicular). Returns parametric t
    /// (0.0–1.0) for where the segment exits the decoration shape. t >= 1.0
    /// means the entire segment is inside the decoration.
    #[allow(clippy::excessive_precision, clippy::needless_return)]
    fn cut_line_at_arrow(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        thickness: f64,
        stroke: &emStroke,
        end: &emStrokeEnd,
    ) -> f64 {
        let mut r = (thickness * ARROW_BASE_SIZE * 0.5 * end.width_factor).abs();
        if r <= 1e-140 {
            return 0.0;
        }
        let mut l = thickness * ARROW_BASE_SIZE * end.length_factor;
        if l <= 1e-140 {
            return 0.0;
        }
        let rounded = stroke.join == super::emStroke::LineJoin::Round
            || stroke.cap == super::emStroke::LineCap::Round;

        let s;
        match end.end_type {
            StrokeEndType::Butt | StrokeEndType::Cap => return 0.0,
            StrokeEndType::Arrow => {
                let d = thickness * 0.5;
                let b = l / r;
                s = (1.0 + b * b).sqrt() * d;
                let b2 = b * ARROW_NOTCH;
                let u = (1.0 + b2 * b2).sqrt() * d;
                let l2 = l - (s + u) / (1.0 - ARROW_NOTCH);
                r *= l2 / l;
                l = l2;
                return Self::cut_arrow(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::ContourArrow => {
                s = if rounded {
                    thickness * 0.5
                } else {
                    let d = thickness * 0.5;
                    let sin_a = r / (l * l + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 {
                        d * sin_a
                    } else {
                        d / sin_a
                    }
                };
                return Self::cut_arrow(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::LineArrow => {
                s = if rounded {
                    thickness * 0.5
                } else {
                    let d = thickness * 0.5;
                    let sin_a = r / (l * l + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 {
                        d * sin_a
                    } else {
                        d / sin_a
                    }
                };
                let l2 = s * 1.5;
                r *= l2 / l;
                l = l2;
                return Self::cut_triangle(x1 - 0.0, y1, x2 - 0.0, y2, r, l);
            }
            StrokeEndType::Triangle => {
                let d = thickness * 0.5;
                let b = l / r;
                s = (1.0 + b * b).sqrt() * d;
                let l2 = l - s - d;
                r *= l2 / l;
                l = l2;
                return Self::cut_triangle(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::ContourTriangle => {
                s = if rounded {
                    thickness * 0.5
                } else {
                    let d = thickness * 0.5;
                    let sin_a = r / (l * l + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 {
                        d * sin_a
                    } else {
                        d / sin_a
                    }
                };
                return Self::cut_triangle(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::Square => {
                s = thickness * 0.5;
                r = (r - s).max(0.0);
                l = (l - thickness).max(0.0);
                return Self::cut_square(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::ContourSquare => {
                s = thickness * 0.5;
                return Self::cut_square(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::HalfSquare => {
                s = thickness * 0.5;
                l = (l * 0.5 - s).max(thickness * 0.0001);
                return Self::cut_square(x1 - s, y1, x2 - s, y2, r, l);
            }
            StrokeEndType::Circle => {
                s = thickness * 0.5;
                r = (r - s).max(0.0);
                l = (l - thickness).max(0.0);
                return Self::cut_circle(x1, y1, x2, y2, r, l, s, false);
            }
            StrokeEndType::ContourCircle => {
                s = thickness * 0.5;
                return Self::cut_circle(x1, y1, x2, y2, r, l, s, false);
            }
            StrokeEndType::HalfCircle => {
                s = if rounded { thickness * 0.5 } else { 0.0 } - l * 0.5;
                return Self::cut_circle(x1, y1, x2, y2, r, l, s, true);
            }
            StrokeEndType::Diamond => {
                s = (r * r + l * l * 0.25).sqrt() / r * thickness * 0.5;
                let l2 = l - s - s;
                r *= l2 / l;
                l = l2;
                return Self::cut_diamond(x1 - s, y1, x2 - s, y2, r, l, false);
            }
            StrokeEndType::ContourDiamond => {
                s = if rounded {
                    thickness * 0.5
                } else {
                    let d = thickness * 0.5;
                    let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                    if MAX_MITER * sin_a < 1.0 {
                        d * sin_a
                    } else {
                        d / sin_a
                    }
                };
                return Self::cut_diamond(x1 - s, y1, x2 - s, y2, r, l, false);
            }
            StrokeEndType::HalfDiamond => {
                let d = thickness * 0.5;
                s = if rounded {
                    d
                } else {
                    let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                    d * (sin_a + (1.0 - sin_a).sqrt())
                } - l * 0.5;
                return Self::cut_diamond(x1 - s, y1, x2 - s, y2, r, l, true);
            }
            StrokeEndType::emStroke => {
                l = thickness * (end.length_factor.abs() - 1.0);
                if l < 0.0 {
                    l = 0.0;
                }
                s = -l * 0.5;
                return Self::cut_square(x1 - s, y1, x2 - s, y2, r, l);
            }
        }
    }

    // --- CutLineAtArrow shape intersection helpers (C++ L_ARROW, L_TRIANGLE, etc.) ---

    fn cut_arrow(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dr = r / l;
        let l2 = (1.0 - ARROW_NOTCH) * l;
        let d2 = r / (l - l2);
        let mut t = 1.0;
        if dy - d2 * dx < -1e-140 {
            if y1 <= d2 * (x1 - l2) {
                t = 0.0;
            } else if y2 < (x2 - l2) * d2 {
                t = (d2 * (x1 - l2) - y1) / (dy - d2 * dx);
            }
        }
        let mut u = 1.0;
        if dy + d2 * dx > 1e-140 {
            if y1 >= -d2 * (x1 - l2) {
                u = 0.0;
            } else if y2 > -(x2 - l2) * d2 {
                u = (-d2 * (x1 - l2) - y1) / (dy + d2 * dx);
            }
        }
        if t < u {
            t = u;
        }
        if dy - dr * dx > 1e-140 {
            if y1 >= dr * x1 {
                return 0.0;
            }
            if y2 > x2 * dr {
                t = t.min((dr * x1 - y1) / (dy - dr * dx));
            }
        }
        if dy + dr * dx < -1e-140 {
            if y1 <= -dr * x1 {
                return 0.0;
            }
            if y2 < -x2 * dr {
                t = t.min((-dr * x1 - y1) / (dy + dr * dx));
            }
        }
        t
    }

    fn cut_triangle(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dr = r / l;
        let mut t = 1.0;
        if dx > 1e-140 {
            if x1 >= l {
                return 0.0;
            }
            if x2 > l {
                t = (l - x1) / dx;
            }
        }
        if dy - dr * dx > 1e-140 {
            if y1 >= dr * x1 {
                return 0.0;
            }
            if y2 > x2 * dr {
                t = t.min((dr * x1 - y1) / (dy - dr * dx));
            }
        }
        if dy + dr * dx < -1e-140 {
            if y1 <= -dr * x1 {
                return 0.0;
            }
            if y2 < -x2 * dr {
                t = t.min((-dr * x1 - y1) / (dy + dr * dx));
            }
        }
        t
    }

    fn cut_square(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let mut t = 1.0;
        if dx > 1e-140 {
            if x1 >= l {
                return 0.0;
            }
            if x2 > l {
                t = (l - x1) / dx;
            }
        } else if dx < -1e-140 {
            if x1 <= 0.0 {
                return 0.0;
            }
            if x2 < 0.0 {
                t = -x1 / dx;
            }
        }
        if dy > 1e-140 {
            if y1 >= r {
                return 0.0;
            }
            if y2 > r {
                t = t.min((r - y1) / dy);
            }
        } else if dy < -1e-140 {
            if y1 <= -r {
                return 0.0;
            }
            if y2 < -r {
                t = t.min((-r - y1) / dy);
            }
        }
        t
    }

    #[allow(clippy::too_many_arguments)]
    fn cut_circle(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64, s: f64, semi: bool) -> f64 {
        let x1 = (x1 - s) * 2.0 / l - 1.0;
        let x2 = (x2 - s) * 2.0 / l - 1.0;
        let y1 = y1 / r;
        let y2 = y2 / r;
        let dx = x2 - x1;
        let dy = y2 - y1;
        let d = dx * dx + dy * dy;
        if d <= 1e-140 {
            return 1.0;
        }
        let d1 = x1 * x1 + y1 * y1;
        let d2 = x2 * x2 + y2 * y2;
        let u = (x1 * dx + y1 * dy) / d;
        let disc = (1.0 - d1) / d + u * u;
        if disc < 0.0 {
            return if d1 < d2 { 0.0 } else { 1.0 };
        }
        let mut t = (disc.sqrt() - u).clamp(0.0, 1.0);
        if semi && dx < -1e-140 {
            if x1 <= 0.0 {
                return 0.0;
            }
            if x2 < 0.0 {
                t = t.min(-x1 / dx);
            }
        }
        t
    }

    fn cut_diamond(x1: f64, y1: f64, x2: f64, y2: f64, r: f64, l: f64, semi: bool) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let dr = 2.0 * r / l;
        let mut t = 1.0;
        if dy - dr * dx > 1e-140 {
            if y1 >= dr * x1 {
                return 0.0;
            }
            if y2 > x2 * dr {
                t = (dr * x1 - y1) / (dy - dr * dx);
            }
        }
        if dy + dr * dx < -1e-140 {
            if y1 <= -dr * x1 {
                return 0.0;
            }
            if y2 < -x2 * dr {
                t = t.min((-dr * x1 - y1) / (dy + dr * dx));
            }
        }
        if dy - dr * dx < -1e-140 {
            if y1 <= dr * (x1 - l) {
                return 0.0;
            }
            if y2 < (x2 - l) * dr {
                t = t.min((dr * (x1 - l) - y1) / (dy - dr * dx));
            }
        }
        if dy + dr * dx > 1e-140 {
            if y1 >= -dr * (x1 - l) {
                return 0.0;
            }
            if y2 > -(x2 - l) * dr {
                t = t.min((-dr * (x1 - l) - y1) / (dy + dr * dx));
            }
        }
        if semi && dx < -1e-140 {
            if x1 <= l * 0.5 {
                return 0.0;
            }
            if x2 < l * 0.5 {
                t = t.min((l * 0.5 - x1) / dx);
            }
        }
        t
    }

    /// Paint a stroke end decoration at an endpoint.
    /// Structural port of C++ `emPainter::PaintArrow`.
    ///
    /// Parameters:
    /// - `(x, y)`: endpoint position
    /// - `(nx, ny)`: perpendicular to line direction = `(dy, -dx)` of into-line direction
    /// - `(dx, dy)`: along-line direction pointing INTO the line body
    /// - `thickness`: stroke width
    /// - `stroke_color`: line body color
    /// - `stroke_end`: decoration specification
    /// - `rounded`: whether the parent stroke uses round joins/caps
    #[allow(clippy::too_many_arguments)]
    fn paint_stroke_end(
        &mut self,
        x: f64,
        y: f64,
        nx: f64,
        ny: f64,
        dx: f64,
        dy: f64,
        thickness: f64,
        stroke_color: emColor,
        stroke_end: &emStrokeEnd,
        rounded: bool,
    ) {
        // C++ uses fabs for r and handles negative l by flipping direction.
        let r = (thickness * ARROW_BASE_SIZE * 0.5 * stroke_end.width_factor).abs();
        if r <= 1e-140 {
            return;
        }
        let mut l = thickness * ARROW_BASE_SIZE * stroke_end.length_factor;
        // Handle negative length: flip direction (matches C++).
        let (dx, dy, nx, ny) = if l < 0.0 {
            l = -l;
            (-dx, -dy, -nx, -ny)
        } else {
            (dx, dy, nx, ny)
        };
        if l <= 1e-140 {
            return;
        }

        // emStroke for sub-drawing (outlines, open polylines).
        // Matches C++ `arrowStroke = stroke; arrowStroke.DashType = SOLID;`.
        let arrow_stroke = {
            let mut s = emStroke::new(stroke_color, thickness);
            if rounded {
                s.join = super::emStroke::LineJoin::Round;
                s.cap = super::emStroke::LineCap::Round;
            }
            s
        };

        // Contour offset helper: C++ `s = thickness*0.5` with miter adjustment.
        let contour_s = |r_val: f64, l_val: f64| -> f64 {
            let mut s = thickness * 0.5;
            if !rounded {
                let sin_a = r_val / (l_val * l_val + r_val * r_val).sqrt();
                if MAX_MITER * sin_a < 1.0 {
                    s *= sin_a;
                } else {
                    s /= sin_a;
                }
            }
            s
        };

        // Bezier circle constant: 4/3 * tan(PI/8).
        let bc = 4.0_f64 / 3.0 * (std::f64::consts::PI / 8.0).tan();

        match stroke_end.end_type {
            StrokeEndType::Butt | StrokeEndType::Cap => {}

            StrokeEndType::Arrow => {
                self.PaintPolygon(
                    &[
                        (x, y),
                        (x + l * dx + r * nx, y + l * dy + r * ny),
                        (
                            x + (1.0 - ARROW_NOTCH) * l * dx,
                            y + (1.0 - ARROW_NOTCH) * l * dy,
                        ),
                        (x + l * dx - r * nx, y + l * dy - r * ny),
                    ],
                    stroke_color,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::ContourArrow => {
                let s = contour_s(r, l);
                let verts = [
                    (x + s * dx, y + s * dy),
                    (x + (s + l) * dx + r * nx, y + (s + l) * dy + r * ny),
                    (
                        x + (s + (1.0 - ARROW_NOTCH) * l) * dx,
                        y + (s + (1.0 - ARROW_NOTCH) * l) * dy,
                    ),
                    (x + (s + l) * dx - r * nx, y + (s + l) * dy - r * ny),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &arrow_stroke,
                    true,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::LineArrow => {
                let s = contour_s(r, l);
                let verts = [
                    (x + (s + l) * dx - r * nx, y + (s + l) * dy - r * ny),
                    (x + s * dx, y + s * dy),
                    (x + (s + l) * dx + r * nx, y + (s + l) * dy + r * ny),
                ];
                let mut line_stroke = arrow_stroke.clone();
                line_stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
                line_stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &line_stroke,
                    false,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::Triangle => {
                self.PaintPolygon(
                    &[
                        (x, y),
                        (x + l * dx + r * nx, y + l * dy + r * ny),
                        (x + l * dx - r * nx, y + l * dy - r * ny),
                    ],
                    stroke_color,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::ContourTriangle => {
                let s = contour_s(r, l);
                let verts = [
                    (x + s * dx, y + s * dy),
                    (x + (s + l) * dx + r * nx, y + (s + l) * dy + r * ny),
                    (x + (s + l) * dx - r * nx, y + (s + l) * dy - r * ny),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &arrow_stroke,
                    true,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::Square => {
                self.PaintPolygon(
                    &[
                        (x + r * nx, y + r * ny),
                        (x + l * dx + r * nx, y + l * dy + r * ny),
                        (x + l * dx - r * nx, y + l * dy - r * ny),
                        (x - r * nx, y - r * ny),
                    ],
                    stroke_color,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::ContourSquare => {
                let s = thickness * 0.5;
                let verts = [
                    (x + s * dx + r * nx, y + s * dy + r * ny),
                    (x + (s + l) * dx + r * nx, y + (s + l) * dy + r * ny),
                    (x + (s + l) * dx - r * nx, y + (s + l) * dy - r * ny),
                    (x + s * dx - r * nx, y + s * dy - r * ny),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &arrow_stroke,
                    true,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::HalfSquare => {
                let s = thickness * 0.5;
                let l_adj = (l * 0.5 - s).max(thickness * 0.0001);
                let verts = [
                    (x + s * dx + r * nx, y + s * dy + r * ny),
                    (x + (s + l_adj) * dx + r * nx, y + (s + l_adj) * dy + r * ny),
                    (x + (s + l_adj) * dx - r * nx, y + (s + l_adj) * dy - r * ny),
                    (x + s * dx - r * nx, y + s * dy - r * ny),
                ];
                let mut hs_stroke = arrow_stroke.clone();
                hs_stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
                hs_stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &hs_stroke,
                    false,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::Circle => {
                // C++ uses 12-point Bezier (4 cubic segments) for exact ellipse.
                let bezier_pts = [
                    (x, y),
                    (x + bc * r * nx, y + bc * r * ny),
                    (
                        x + (1.0 - bc) * 0.5 * l * dx + r * nx,
                        y + (1.0 - bc) * 0.5 * l * dy + r * ny,
                    ),
                    (x + 0.5 * l * dx + r * nx, y + 0.5 * l * dy + r * ny),
                    (
                        x + (1.0 + bc) * 0.5 * l * dx + r * nx,
                        y + (1.0 + bc) * 0.5 * l * dy + r * ny,
                    ),
                    (x + l * dx + bc * r * nx, y + l * dy + bc * r * ny),
                    (x + l * dx, y + l * dy),
                    (x + l * dx - bc * r * nx, y + l * dy - bc * r * ny),
                    (
                        x + (1.0 + bc) * 0.5 * l * dx - r * nx,
                        y + (1.0 + bc) * 0.5 * l * dy - r * ny,
                    ),
                    (x + 0.5 * l * dx - r * nx, y + 0.5 * l * dy - r * ny),
                    (
                        x + (1.0 - bc) * 0.5 * l * dx - r * nx,
                        y + (1.0 - bc) * 0.5 * l * dy - r * ny,
                    ),
                    (x - bc * r * nx, y - bc * r * ny),
                ];
                self.PaintBezier(&bezier_pts, stroke_color, self.state.canvas_color);
            }

            StrokeEndType::ContourCircle => {
                let s = thickness * 0.5;
                let bezier_pts = [
                    (x + s * dx, y + s * dy),
                    (x + s * dx + bc * r * nx, y + s * dy + bc * r * ny),
                    (
                        x + (s + (1.0 - bc) * 0.5 * l) * dx + r * nx,
                        y + (s + (1.0 - bc) * 0.5 * l) * dy + r * ny,
                    ),
                    (
                        x + (s + 0.5 * l) * dx + r * nx,
                        y + (s + 0.5 * l) * dy + r * ny,
                    ),
                    (
                        x + (s + (1.0 + bc) * 0.5 * l) * dx + r * nx,
                        y + (s + (1.0 + bc) * 0.5 * l) * dy + r * ny,
                    ),
                    (
                        x + (s + l) * dx + bc * r * nx,
                        y + (s + l) * dy + bc * r * ny,
                    ),
                    (x + (s + l) * dx, y + (s + l) * dy),
                    (
                        x + (s + l) * dx - bc * r * nx,
                        y + (s + l) * dy - bc * r * ny,
                    ),
                    (
                        x + (s + (1.0 + bc) * 0.5 * l) * dx - r * nx,
                        y + (s + (1.0 + bc) * 0.5 * l) * dy - r * ny,
                    ),
                    (
                        x + (s + 0.5 * l) * dx - r * nx,
                        y + (s + 0.5 * l) * dy - r * ny,
                    ),
                    (
                        x + (s + (1.0 - bc) * 0.5 * l) * dx - r * nx,
                        y + (s + (1.0 - bc) * 0.5 * l) * dy - r * ny,
                    ),
                    (x + s * dx - bc * r * nx, y + s * dy - bc * r * ny),
                ];
                self.PaintBezier(&bezier_pts, stroke_end.inner_color, self.state.canvas_color);
                self.PaintBezierOutline(&bezier_pts, &arrow_stroke, self.state.canvas_color);
            }

            StrokeEndType::HalfCircle => {
                // C++ uses 7-point BezierLine.
                let s = if rounded { thickness * 0.5 } else { 0.0 };
                let bezier_pts = [
                    (x + s * dx + r * nx, y + s * dy + r * ny),
                    (
                        x + (s + bc * 0.5 * l) * dx + r * nx,
                        y + (s + bc * 0.5 * l) * dy + r * ny,
                    ),
                    (
                        x + (s + 0.5 * l) * dx + bc * r * nx,
                        y + (s + 0.5 * l) * dy + bc * r * ny,
                    ),
                    (x + (s + 0.5 * l) * dx, y + (s + 0.5 * l) * dy),
                    (
                        x + (s + 0.5 * l) * dx - bc * r * nx,
                        y + (s + 0.5 * l) * dy - bc * r * ny,
                    ),
                    (
                        x + (s + bc * 0.5 * l) * dx - r * nx,
                        y + (s + bc * 0.5 * l) * dy - r * ny,
                    ),
                    (x + s * dx - r * nx, y + s * dy - r * ny),
                ];
                let mut hc_stroke = arrow_stroke.clone();
                if rounded {
                    hc_stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
                    hc_stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
                }
                self.PaintBezierLine(&bezier_pts, &hc_stroke, self.state.canvas_color);
            }

            StrokeEndType::Diamond => {
                self.PaintPolygon(
                    &[
                        (x, y),
                        (x + 0.5 * l * dx + r * nx, y + 0.5 * l * dy + r * ny),
                        (x + l * dx, y + l * dy),
                        (x + 0.5 * l * dx - r * nx, y + 0.5 * l * dy - r * ny),
                    ],
                    stroke_color,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::ContourDiamond => {
                let s = {
                    let mut s = thickness * 0.5;
                    if !rounded {
                        let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                        if MAX_MITER * sin_a < 1.0 {
                            s *= sin_a;
                        } else {
                            s /= sin_a;
                        }
                    }
                    s
                };
                let verts = [
                    (x + s * dx, y + s * dy),
                    (
                        x + (s + 0.5 * l) * dx + r * nx,
                        y + (s + 0.5 * l) * dy + r * ny,
                    ),
                    (x + (s + l) * dx, y + (s + l) * dy),
                    (
                        x + (s + 0.5 * l) * dx - r * nx,
                        y + (s + 0.5 * l) * dy - r * ny,
                    ),
                ];
                self.PaintPolygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &arrow_stroke,
                    true,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::HalfDiamond => {
                let s = {
                    let mut s = thickness * 0.5;
                    if !rounded {
                        let sin_a = r / (l * l * 0.25 + r * r).sqrt();
                        s *= sin_a + (1.0 - sin_a).sqrt();
                    }
                    s
                };
                let verts = [
                    (x + s * dx + r * nx, y + s * dy + r * ny),
                    (x + (s + 0.5 * l) * dx, y + (s + 0.5 * l) * dy),
                    (x + s * dx - r * nx, y + s * dy - r * ny),
                ];
                let mut hd_stroke = arrow_stroke.clone();
                hd_stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
                hd_stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &hd_stroke,
                    false,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::emStroke => {
                let stroke_thickness = thickness * stroke_end.length_factor.abs();
                let verts = [(x + r * nx, y + r * ny), (x - r * nx, y - r * ny)];
                let mut st_stroke = arrow_stroke.clone();
                st_stroke.width = stroke_thickness;
                st_stroke.start_end = emStrokeEnd::new(StrokeEndType::Cap);
                st_stroke.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
                self.PaintPolylineWithoutArrows(
                    &verts,
                    &st_stroke,
                    false,
                    self.state.canvas_color,
                );
            }
        }
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
        if ix1 < ix2 {
            if span.opacity_mid >= 0x1000 {
                self.fill_span_blended(proof, y, ix1, ix2, color);
            } else if span.opacity_mid > 0 {
                // Uniform partial coverage: pre-compute alpha-adjusted color once.
                let alpha =
                    ((color.GetAlpha() as i32 * span.opacity_mid + 0x800) >> 12).clamp(0, 255) as u8;
                let blended = emColor::rgba(color.GetRed(), color.GetGreen(), color.GetBlue(), alpha);
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
        if iw > 2 {
            let combined_alpha = ((color.GetAlpha() as i32 * a + 0x800) >> 12).clamp(0, 255) as u8;
            if combined_alpha > 0 {
                let interior_color = color.SetAlpha(combined_alpha);
                if combined_alpha == 255 {
                    self.fill_span_blended(proof, iy, ix + 1, ix + iw - 1, color);
                } else {
                    self.fill_span_blended(proof, iy, ix + 1, ix + iw - 1, interior_color);
                }
            }
        }
        // Last pixel (if width > 1)
        if iw > 1 {
            self.blend_with_coverage(proof, ix + iw - 1, iy, color, a2);
        }
    }

    fn blend_with_coverage(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor, cov: i32) {
        if cov >= 0x1000 {
            self.blend_pixel(proof, x, y, color);
        } else if cov > 0 {
            // C++ single-step: alpha = (color_alpha * opacity_12bit + 0x800) >> 12
            let alpha = ((color.GetAlpha() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8;
            let blended = emColor::rgba(color.GetRed(), color.GetGreen(), color.GetBlue(), alpha);
            self.blend_pixel(proof, x, y, blended);
        }
    }

    /// Same as `blend_pixel` but without clip/bounds checks.
    /// Caller must guarantee x,y are within both the clip rect and the target image.
    #[inline(always)]
    fn blend_pixel_unchecked(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor) {
        let xu = x as u32;
        let yu = y as u32;
        if color.IsOpaque() && self.state.alpha == 255 {
            let out = self.GetImage(proof).SetPixel(xu, yu);
            out[0] = color.GetRed();
            out[1] = color.GetGreen();
            out[2] = color.GetBlue();
            out[3] = 255;
        } else if self.state.canvas_color.IsOpaque() {
            let combined_alpha = if self.state.alpha == 255 {
                color.GetAlpha()
            } else {
                ((color.GetAlpha() as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if combined_alpha == 0 {
                return;
            }
            let px = self.read_pixel(proof, xu, yu);
            let existing = emColor::rgba(px[0], px[1], px[2], px[3]);
            let result = existing.canvas_blend(color, self.state.canvas_color, combined_alpha);
            let out = self.GetImage(proof).SetPixel(xu, yu);
            out[0] = result.GetRed();
            out[1] = result.GetGreen();
            out[2] = result.GetBlue();
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
                let out = self.GetImage(proof).SetPixel(xu, yu);
                out[0] = color.GetRed();
                out[1] = color.GetGreen();
                out[2] = color.GetBlue();
                out[3] = 255;
                return;
            }
            // Background: Blinn div255. Source: C++ hash table.
            let bg = self.read_pixel(proof, xu, yu);
            let alpha = ea as u8;
            let t = (255 - alpha as u32) * 257;
            let r = ((bg[0] as u32 * t + 0x8073) >> 16)
                + blend_hash_lookup(color.GetRed(), alpha) as u32;
            let g = ((bg[1] as u32 * t + 0x8073) >> 16)
                + blend_hash_lookup(color.GetGreen(), alpha) as u32;
            let b = ((bg[2] as u32 * t + 0x8073) >> 16)
                + blend_hash_lookup(color.GetBlue(), alpha) as u32;
            let a =
                ((bg[3] as u32 * t + 0x8073) >> 16) + blend_hash_lookup(255, alpha) as u32;
            let out = self.GetImage(proof).SetPixel(xu, yu);
            out[0] = r as u8;
            out[1] = g as u8;
            out[2] = b as u8;
            out[3] = a as u8;
        }
    }

    /// Same as `blend_with_coverage` but without clip/bounds checks.
    #[inline(always)]
    fn blend_with_coverage_unchecked(&mut self, proof: DirectProof, x: i32, y: i32, color: emColor, cov: i32) {
        if cov >= 0x1000 {
            self.blend_pixel_unchecked(proof, x, y, color);
        } else if cov > 0 {
            let alpha = ((color.GetAlpha() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8;
            let blended = emColor::rgba(color.GetRed(), color.GetGreen(), color.GetBlue(), alpha);
            self.blend_pixel_unchecked(proof, x, y, blended);
        }
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
            } => PixelTexture::LinearGradient {
                color_a: *color_a,
                color_b: *color_b,
                start: (
                    start.0 * state.scale_x + state.offset_x,
                    start.1 * state.scale_y + state.offset_y,
                ),
                end: (
                    end.0 * state.scale_x + state.offset_x,
                    end.1 * state.scale_y + state.offset_y,
                ),
            },
            emTexture::RadialGradient {
                color_inner,
                color_outer,
                center,
                radius,
            } => {
                let pcx = center.0 * state.scale_x + state.offset_x;
                let pcy = center.1 * state.scale_y + state.offset_y;
                let prx = (radius * state.scale_x).max(1e-3);
                let pry = (radius * state.scale_y).max(1e-3);
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
                extension,
                quality,
            } => PixelTexture::emImage {
                image,
                extension: *extension,
                quality: *quality,
                inv_scale_x: 1.0 / state.scale_x,
                inv_scale_y: 1.0 / state.scale_y,
                offset_x: state.offset_x,
                offset_y: state.offset_y,
            },
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
        }
    }

    /// Sample a color from a pixel-space texture at the given pixel coordinates.
    fn sample_pixel_texture(texture: &PixelTexture, px: f64, py: f64) -> emColor {
        match texture {
            PixelTexture::Solid(c) => *c,
            PixelTexture::LinearGradient {
                color_a,
                color_b,
                start,
                end,
            } => {
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let len_sq = dx * dx + dy * dy;
                if len_sq < 1e-12 {
                    return *color_a;
                }
                let t = ((px - start.0) * dx + (py - start.1) * dy) / len_sq;
                let g = (t.clamp(0.0, 1.0) * 255.0 + 0.5) as i32;
                let mix = |a: i32, b: i32| -> u8 {
                    (((a * (255 - g) + b * g) * 257 + 0x8073) >> 16) as u8
                };
                emColor::rgba(
                    mix(color_a.GetRed() as i32, color_b.GetRed() as i32),
                    mix(color_a.GetGreen() as i32, color_b.GetGreen() as i32),
                    mix(color_a.GetBlue() as i32, color_b.GetBlue() as i32),
                    mix(color_a.GetAlpha() as i32, color_b.GetAlpha() as i32),
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
                // C++ bounds check: (emUInt64)tx+(0xFF<<23) < (0x1FE<<23)
                // Equivalent: -0xFF_0000_00 <= tx < 0xFF_0000_00 (and same for ty).
                const LIMIT: i64 = 0xFF << 23; // 255 * 2^23
                if tx.unsigned_abs() >= LIMIT as u64 || ty.unsigned_abs() >= LIMIT as u64 {
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
                extension,
                quality,
                inv_scale_x,
                inv_scale_y,
                offset_x,
                offset_y,
            } => {
                let lx = (px - offset_x) * inv_scale_x;
                let ly = (py - offset_y) * inv_scale_y;
                Self::sample_image_at(image, lx, ly, *extension, *quality)
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
            PixelTexture::RadialGradient { color_inner, color_outer, fp_tx, fp_ty, fp_tdx, fp_tdy } => {
                self.blit_span_radial_gradient_g1g2(
                    proof, y, span, x_start, x_end,
                    *color_inner, *color_outer, *fp_tx, *fp_ty, *fp_tdx, *fp_tdy,
                );
            }
            _ => {
                // Fallback: per-pixel texture evaluation (used for other texture types).
                let py = y as f64 + 0.5;
                for x in x_start..x_end {
                    let opacity = span_opacity_at(span, x, x_start, x_end);
                    if opacity == 0 { continue; }
                    let color = Self::sample_pixel_texture(texture, x as f64 + 0.5, py);
                    if opacity >= 0x1000 {
                        self.blend_pixel_unchecked(proof, x, y, color);
                    } else {
                        self.blend_with_coverage_unchecked(proof, x, y, color, opacity);
                    }
                }
            }
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
        const LIMIT: i64 = 0xFF << 23;

        // Precompute ty*ty + rounding constant (matching C++ line 213)
        let ty_in_range = ty.unsigned_abs() < LIMIT as u64;
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

            let g = if !ty_in_range || tx.unsigned_abs() >= LIMIT as u64 {
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

    /// Generate polygon vertices for a rounded rectangle.
    fn round_rect_polygon(&self, x: f64, y: f64, w: f64, h: f64, r: f64) -> Vec<(f64, f64)> {
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        // C++: if (rx<=0.0 || ry<=0.0) { PaintRect(...); return; }
        // Must match C++ threshold exactly — r is in user-space coordinates,
        // not pixels, so any positive radius needs polygon vertices.
        if r <= 0.0 {
            return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        }
        // C++ PaintRoundRect: f = CQ * sqrt(rx*SX + ry*SY), clamp 256,
        // f *= 0.25, then n = round(f) clamped to [1, 64].
        // Must multiply by 0.25 BEFORE rounding (not round then divide by 4).
        let mut f = CIRCLE_QUALITY * (r * self.state.scale_x + r * self.state.scale_y).sqrt();
        if f > 256.0 {
            f = 256.0;
        }
        f *= 0.25;
        let corner_segments = if f <= 1.0 {
            1
        } else if f >= 64.0 {
            64
        } else {
            (f + 0.5) as usize
        };
        // C++ PaintRoundRect: single loop, 4 vertices per i.
        // step = PI/2 / n. Corners stored sequentially:
        //   [0..n] = top-left, [n+1..2n+1] = top-right,
        //   [2n+2..3n+2] = bottom-right, [3n+3..4n+3] = bottom-left.
        let n = corner_segments;
        let step = std::f64::consts::FRAC_PI_2 / n as f64;
        let cx1 = x + r; // left corner centers
        let cy1 = y + r; // top corner centers
        let cx2 = x + w - r; // right corner centers
        let cy2 = y + h - r; // bottom corner centers
        let total = 4 * (n + 1);
        let mut verts = vec![(0.0, 0.0); total];
        for i in 0..=n {
            let dx = (step * i as f64).cos();
            let dy = (step * i as f64).sin();
            verts[i] = (cx1 - dx * r, cy1 - dy * r); // top-left
            verts[n + 1 + i] = (cx2 + dy * r, cy1 - dx * r); // top-right
            verts[2 * n + 2 + i] = (cx2 + dx * r, cy2 + dy * r); // bottom-right
            verts[3 * n + 3 + i] = (cx1 - dy * r, cy2 + dx * r); // bottom-left
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
        if (x as f64) < clip.x1
            || (x as f64) >= clip.x2
            || (y as f64) < clip.y1
            || (y as f64) >= clip.y2
        {
            return;
        }
        if x < 0 || y < 0 || x >= self.target_width as i32 || y >= self.target_height as i32 {
            return;
        }

        if color.IsOpaque() && self.state.alpha == 255 {
            // Fully opaque: direct write, no blending needed.
            let out = self.GetImage(proof).SetPixel(x as u32, y as u32);
            out[0] = color.GetRed();
            out[1] = color.GetGreen();
            out[2] = color.GetBlue();
            out[3] = 255;
        } else if self.state.canvas_color.IsOpaque() {
            // Canvas blend: target += (source - canvas) * alpha / 256
            // Used when the background color is known (opaque canvas), giving
            // better anti-aliasing at shape edges. Matches Eagle Mode's emPainter.
            // Alpha must combine both the source color's alpha and the painter's
            // global alpha, matching Eagle Mode where opacity = color_alpha * coverage.
            //
            // C++ HAVE_CVC path only modifies RGB — alpha is unchanged.
            // The hash tables hcR/hcG/hcB only cover RGB, and `*p += pix`
            // leaves the alpha channel untouched.
            let combined_alpha = if self.state.alpha == 255 {
                color.GetAlpha()
            } else {
                ((color.GetAlpha() as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if combined_alpha == 0 {
                return;
            }
            let px = self.read_pixel(proof, x as u32, y as u32);
            let existing = emColor::rgba(px[0], px[1], px[2], px[3]);
            let result = existing.canvas_blend(color, self.state.canvas_color, combined_alpha);
            let out = self.GetImage(proof).SetPixel(x as u32, y as u32);
            out[0] = result.GetRed();
            out[1] = result.GetGreen();
            out[2] = result.GetBlue();
            // out[3] unchanged — C++ HAVE_CVC never modifies destination alpha.
        } else {
            // Standard source-over alpha compositing when canvas color is
            // unknown (non-opaque). Avoids the additive artifacts that
            // canvas_blend produces with TRANSPARENT canvas.
            let ca = color.GetAlpha() as u16;
            let ea = if self.state.alpha == 255 {
                ca
            } else {
                (ca * self.state.alpha as u16 + 128) >> 8
            };
            if ea == 0 {
                return;
            }
            let bg = self.read_pixel(proof, x as u32, y as u32);
            if ea >= 255 {
                let out = self.GetImage(proof).SetPixel(x as u32, y as u32);
                out[0] = color.GetRed();
                out[1] = color.GetGreen();
                out[2] = color.GetBlue();
                out[3] = 255;
            } else {
                // Background: Blinn div255. Source: C++ hash table.
                let alpha = ea as u8;
                let t = (255 - alpha as u32) * 257;
                let r = ((bg[0] as u32 * t + 0x8073) >> 16)
                    + blend_hash_lookup(color.GetRed(), alpha) as u32;
                let g = ((bg[1] as u32 * t + 0x8073) >> 16)
                    + blend_hash_lookup(color.GetGreen(), alpha) as u32;
                let b = ((bg[2] as u32 * t + 0x8073) >> 16)
                    + blend_hash_lookup(color.GetBlue(), alpha) as u32;
                let a =
                    ((bg[3] as u32 * t + 0x8073) >> 16)
                        + blend_hash_lookup(255, alpha) as u32;
                let out = self.GetImage(proof).SetPixel(x as u32, y as u32);
                out[0] = r as u8;
                out[1] = g as u8;
                out[2] = b as u8;
                out[3] = a as u8;
            }
        }
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
                // Use hash table to match C++ PaintScanlineCol exactly.
                use super::emColor::blend_hash_lookup;
                let alpha = ea as u8;
                let t = (255 - alpha as u32) * 257;
                let sr = blend_hash_lookup(color.GetRed(), alpha) as u32;
                let sg = blend_hash_lookup(color.GetGreen(), alpha) as u32;
                let sb = blend_hash_lookup(color.GetBlue(), alpha) as u32;
                let sa = blend_hash_lookup(255, alpha) as u32;
                let data = self.GetImage(proof).GetWritableMap();
                for col in x1..x2 {
                    let off = row_base + col as usize * 4;
                    data[off] = (((data[off] as u32 * t + 0x8073) >> 16) + sr) as u8;
                    data[off + 1] = (((data[off + 1] as u32 * t + 0x8073) >> 16) + sg) as u8;
                    data[off + 2] = (((data[off + 2] as u32 * t + 0x8073) >> 16) + sb) as u8;
                    data[off + 3] = (((data[off + 3] as u32 * t + 0x8073) >> 16) + sa) as u8;
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

        // C++ reference values at y=25, x=10..22 (from test_tiny_rect_compose.cpp)
        let cpp_y25: [(u8,u8,u8); 13] = [
            (144,144,145), (144,144,145), (144,144,145), (144,144,145),
            (143,143,144), (138,138,139), (138,138,139), (138,138,139),
            (138,138,139), (138,138,139), (134,134,135), (128,128,128), (128,128,128),
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
