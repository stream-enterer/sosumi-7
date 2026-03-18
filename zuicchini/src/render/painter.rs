use std::sync::OnceLock;

use super::bitmap_font;
use super::draw_list::DrawOp;
use super::em_font;
use super::interpolation;
use super::scanline::{self, WindingRule};
use super::scanline_tool::{blend_scanline, blend_scanline_premul, BlendMode, InterpolationBuffer};
use super::stroke::{Stroke, StrokeEnd, StrokeEndType};
use super::texture::{ImageExtension, ImageQuality, Texture};
use crate::foundation::{Color, Fixed12, Image};

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

/// Pre-transformed texture with coordinates in pixel space.
/// Used internally by the textured polygon rasterizer.
enum PixelTexture<'t> {
    Solid(Color),
    LinearGradient {
        color_a: Color,
        color_b: Color,
        start: (f64, f64),
        end: (f64, f64),
    },
    RadialGradient {
        color_inner: Color,
        color_outer: Color,
        /// Fixed-point base: `(center_px - 0.5) * tdx`, cast to i64.
        fp_tx: i64,
        /// Fixed-point base: `(center_py - 0.5) * tdy`, cast to i64.
        fp_ty: i64,
        /// Fixed-point X step: `(255 << 23) / prx`, cast to i64.
        fp_tdx: i64,
        /// Fixed-point Y step: `(255 << 23) / pry`, cast to i64.
        fp_tdy: i64,
    },
    Image {
        image: &'t Image,
        extension: ImageExtension,
        quality: ImageQuality,
        inv_scale_x: f64,
        inv_scale_y: f64,
        offset_x: f64,
        offset_y: f64,
    },
    ImageColored {
        image: &'t Image,
        color: Color,
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
    fn is_empty(&self) -> bool {
        self.x1 >= self.x2 || self.y1 >= self.y2
    }

    fn to_scanline_clip(self) -> scanline::ClipBounds {
        scanline::ClipBounds {
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
    canvas_color: Color,
    /// Global alpha multiplier (0–255).
    alpha: u8,
}

/// Sub-pixel rectangle edges for 12-bit fractional coverage.
/// Matches C++ emPainter PaintRect sub-pixel model (emPainter.cpp:334-397).
struct SubPixelEdges {
    ix1: i32,
    iy1: i32,
    ix2: i32,
    iy2: i32,
    frac_left: i32,
    frac_right: i32,
    frac_top: i32,
    frac_bottom: i32,
    raw_w: i32,
    raw_h: i32,
}

impl SubPixelEdges {
    /// Compute sub-pixel edges from pixel-space float coordinates.
    fn new(dx_px: f64, dy_px: f64, dw_px: f64, dh_px: f64) -> Self {
        let fx1 = Fixed12::from_f64(dx_px);
        let fy1 = Fixed12::from_f64(dy_px);
        let fx2 = Fixed12::from_f64(dx_px + dw_px);
        let fy2 = Fixed12::from_f64(dy_px + dh_px);
        Self {
            ix1: fx1.to_i32(),
            iy1: fy1.to_i32(),
            ix2: fx2.ceil().to_i32(),
            iy2: fy2.ceil().to_i32(),
            frac_left: 0x1000 - fx1.frac(),
            frac_right: fx2.frac(),
            frac_top: 0x1000 - fy1.frac(),
            frac_bottom: fy2.frac(),
            raw_w: (fx2 - fx1).raw(),
            raw_h: (fy2 - fy1).raw(),
        }
    }

    /// Compute coverages for a batch of consecutive columns at `row`.
    /// Returns `true` if all coverages are full (0x1000).
    fn batch_coverages(&self, row: i32, col_start: i32, out: &mut [i32]) -> bool {
        let mut all_full = true;
        for (i, cov) in out.iter_mut().enumerate() {
            *cov = self.coverage(col_start + i as i32, row);
            if *cov < 0x1000 {
                all_full = false;
            }
        }
        all_full
    }

    /// Per-pixel combined coverage (0..=0x1000).
    /// Mirrors paint_rect() lines 339-364 logic exactly.
    #[inline]
    fn coverage(&self, px: i32, py: i32) -> i32 {
        let alpha_y = if py == self.iy1 && py == self.iy2 - 1 {
            (self.frac_top + self.frac_bottom).min(0x1000) - 0x1000 + self.raw_h.min(0x1000)
        } else if py == self.iy1 {
            self.frac_top
        } else if py == self.iy2 - 1 && self.frac_bottom != 0 {
            self.frac_bottom
        } else {
            0x1000
        };
        if alpha_y <= 0 {
            return 0;
        }

        let alpha_x = if px == self.ix1 && px == self.ix2 - 1 {
            (self.frac_left + self.frac_right).min(0x1000) - 0x1000 + self.raw_w.min(0x1000)
        } else if px == self.ix1 {
            self.frac_left
        } else if px == self.ix2 - 1 && self.frac_right != 0 {
            self.frac_right
        } else {
            0x1000
        };
        if alpha_x <= 0 {
            return 0;
        }

        ((alpha_x as i64 * alpha_y as i64) >> 12) as i32
    }
}

/// Paint target: either a real image or a draw list for recording.
pub(crate) enum PaintTarget<'a> {
    /// Direct pixel rendering to an image buffer.
    Image(&'a mut Image),
    /// Recording mode: draw operations are captured for parallel replay.
    DrawList(&'a mut Vec<DrawOp>),
}

/// CPU software rasterizer that paints into an Image buffer.
pub struct Painter<'a> {
    target: PaintTarget<'a>,
    target_width: u32,
    target_height: u32,
    state: PainterState,
    state_stack: Vec<PainterState>,
}

impl<'a> Painter<'a> {
    /// Create a new painter targeting the given RGBA image.
    ///
    /// # Panics
    /// Panics if the image is not 4-channel RGBA.
    pub fn new(target: &'a mut Image) -> Self {
        assert_eq!(
            target.channel_count(),
            4,
            "Painter requires a 4-channel RGBA image"
        );
        let w = target.width();
        let h = target.height();
        Self {
            target: PaintTarget::Image(target),
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
                canvas_color: Color::BLACK,
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
    pub(crate) fn new_recording(width: u32, height: u32, ops: &'a mut Vec<DrawOp>) -> Self {
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
                canvas_color: Color::BLACK,
                alpha: 255,
            },
            state_stack: Vec::new(),
        }
    }

    /// Get a mutable reference to the target image.
    /// Panics if in recording mode — callers must check `is_recording()` first.
    fn image(&mut self) -> &mut Image {
        match &mut self.target {
            PaintTarget::Image(img) => img,
            PaintTarget::DrawList(_) => unreachable!("pixel access in recording mode"),
        }
    }

    /// Get an immutable reference to the target image.
    fn image_ref(&self) -> &Image {
        match &self.target {
            PaintTarget::Image(img) => img,
            PaintTarget::DrawList(_) => unreachable!("pixel access in recording mode"),
        }
    }

    /// Push a draw op when in recording mode. Returns true if recording.
    fn record(&mut self, op: DrawOp) -> bool {
        if let PaintTarget::DrawList(ops) = &mut self.target {
            ops.push(op);
            true
        } else {
            false
        }
    }

    /// Read a pixel from the target image, returning an owned copy.
    /// Avoids borrow issues when both reading and writing pixels.
    #[inline]
    fn read_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let p = self.image_ref().pixel(x, y);
        [p[0], p[1], p[2], p[3]]
    }

    /// Push the current state onto the stack.
    pub fn push_state(&mut self) {
        self.record(DrawOp::PushState);
        self.state_stack.push(self.state.clone());
    }

    /// Pop and restore the previous state.
    ///
    /// # Panics
    /// Panics if the state stack is empty.
    pub fn pop_state(&mut self) {
        self.record(DrawOp::PopState);
        self.state = self.state_stack.pop().expect("State stack underflow");
    }

    /// Get the current canvas color.
    pub fn canvas_color(&self) -> Color {
        self.state.canvas_color
    }

    /// Set the canvas color used for canvas_blend operations.
    pub fn set_canvas_color(&mut self, color: Color) {
        self.record(DrawOp::SetCanvasColor(color));
        self.state.canvas_color = color;
    }

    /// Set the global alpha multiplier.
    pub fn set_alpha(&mut self, alpha: u8) {
        self.record(DrawOp::SetAlpha(alpha));
        self.state.alpha = alpha;
    }

    /// Get the current offset (for computing absolute panel transforms).
    pub fn offset(&self) -> (f64, f64) {
        (self.state.offset_x, self.state.offset_y)
    }

    /// Set the offset absolutely (not cumulative).
    pub fn set_offset(&mut self, ox: f64, oy: f64) {
        self.record(DrawOp::SetOffset(ox, oy));
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
    pub fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.record(DrawOp::ClipRect { x, y, w, h });
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
    pub fn clip_is_empty(&self) -> bool {
        self.state.clip.is_empty()
    }

    /// Set origin (absolute offset, replaces current offset).
    pub fn set_origin(&mut self, x: f64, y: f64) {
        self.state.offset_x = x;
        self.state.offset_y = y;
    }

    /// Set scaling (absolute, replaces current scale).
    pub fn set_scaling(&mut self, sx: f64, sy: f64) {
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
    pub fn round_x(&self, x: f64) -> f64 {
        ((x * self.state.scale_x + self.state.offset_x).round() - self.state.offset_x)
            / self.state.scale_x
    }

    /// Round y coordinate to nearest pixel.
    pub fn round_y(&self, y: f64) -> f64 {
        ((y * self.state.scale_y + self.state.offset_y).round() - self.state.offset_y)
            / self.state.scale_y
    }

    /// Round x coordinate down to pixel boundary.
    pub fn round_down_x(&self, x: f64) -> f64 {
        ((x * self.state.scale_x + self.state.offset_x).floor() - self.state.offset_x)
            / self.state.scale_x
    }

    /// Round y coordinate down to pixel boundary.
    pub fn round_down_y(&self, y: f64) -> f64 {
        ((y * self.state.scale_y + self.state.offset_y).floor() - self.state.offset_y)
            / self.state.scale_y
    }

    /// Round x coordinate up to pixel boundary.
    pub fn round_up_x(&self, x: f64) -> f64 {
        ((x * self.state.scale_x + self.state.offset_x).ceil() - self.state.offset_x)
            / self.state.scale_x
    }

    /// Round y coordinate up to pixel boundary.
    pub fn round_up_y(&self, y: f64) -> f64 {
        ((y * self.state.scale_y + self.state.offset_y).ceil() - self.state.offset_y)
            / self.state.scale_y
    }

    /// Get the left edge of the clip rectangle in user coordinates.
    pub fn get_user_clip_x1(&self) -> f64 {
        (self.state.clip.x1 - self.state.offset_x) / self.state.scale_x
    }

    /// Get the top edge of the clip rectangle in user coordinates.
    pub fn get_user_clip_y1(&self) -> f64 {
        (self.state.clip.y1 - self.state.offset_y) / self.state.scale_y
    }

    /// Get the right edge of the clip rectangle in user coordinates.
    pub fn get_user_clip_x2(&self) -> f64 {
        (self.state.clip.x2 - self.state.offset_x) / self.state.scale_x
    }

    /// Get the bottom edge of the clip rectangle in user coordinates.
    pub fn get_user_clip_y2(&self) -> f64 {
        (self.state.clip.y2 - self.state.offset_y) / self.state.scale_y
    }

    // --- Drawing API ---

    /// Fill a rectangle with a solid color using sub-pixel anti-aliasing.
    /// Uses 12-bit fixed-point for fractional edge coverage matching C++ emPainter.
    pub fn paint_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        canvas_color: Color,
    ) {
        if w <= 0.0 || h <= 0.0 || color.a() == 0 {
            return;
        }
        if self.record(DrawOp::PaintRect {
            x,
            y,
            w,
            h,
            color,
            canvas_color,
        }) {
            return;
        }
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let dx_px = x * self.state.scale_x + self.state.offset_x;
        let dy_px = y * self.state.scale_y + self.state.offset_y;
        let dw_px = w * self.state.scale_x;
        let dh_px = h * self.state.scale_y;
        let sp = SubPixelEdges::new(dx_px, dy_px, dw_px, dh_px);

        let tw = self.target_width as i32;
        let th = self.target_height as i32;
        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(tw);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(th);

        let start_x = sp.ix1.max(cx1);
        let start_y = sp.iy1.max(cy1);
        let end_x = sp.ix2.min(cx2);
        let end_y = sp.iy2.min(cy2);

        if start_x >= end_x || start_y >= end_y {
            return;
        }

        // Interior pixels have full coverage — avoid per-pixel clip/bounds
        // checks and coverage computation by splitting into edge and interior
        // regions.  Edge pixels (on the sub-pixel boundary of the rect) still
        // need per-pixel coverage; interior pixels can be bulk-written.

        // Interior region: rows/cols that are fully inside the rect (full coverage).
        let inner_x1 = if sp.frac_left < 0x1000 {
            (sp.ix1 + 1).max(start_x)
        } else {
            start_x
        };
        let inner_x2 = if sp.frac_right > 0 && sp.frac_right < 0x1000 {
            (sp.ix2 - 1).min(end_x)
        } else {
            end_x
        };
        let inner_y1 = if sp.frac_top < 0x1000 {
            (sp.iy1 + 1).max(start_y)
        } else {
            start_y
        };
        let inner_y2 = if sp.frac_bottom > 0 && sp.frac_bottom < 0x1000 {
            (sp.iy2 - 1).min(end_y)
        } else {
            end_y
        };

        // Top edge row (sub-pixel coverage in Y)
        if start_y < inner_y1 {
            for px in start_x..end_x {
                self.blend_with_coverage(px, start_y, color, sp.coverage(px, start_y));
            }
        }

        // Middle rows
        for py in inner_y1..inner_y2 {
            // Left edge pixel (sub-pixel coverage in X)
            if start_x < inner_x1 {
                self.blend_with_coverage(start_x, py, color, sp.coverage(start_x, py));
            }

            // Interior span: full coverage, no per-pixel clip/bounds checks.
            if inner_x1 < inner_x2 {
                self.fill_span_blended(py, inner_x1, inner_x2, color);
            }

            // Right edge pixel (sub-pixel coverage in X)
            if inner_x2 < end_x {
                let rx = end_x - 1;
                self.blend_with_coverage(rx, py, color, sp.coverage(rx, py));
            }
        }

        // Bottom edge row (sub-pixel coverage in Y)
        if inner_y2 < end_y {
            let by = end_y - 1;
            for px in start_x..end_x {
                self.blend_with_coverage(px, by, color, sp.coverage(px, by));
            }
        }

        self.state.canvas_color = saved_canvas;
    }

    /// Fill an ellipse with a solid color using AA polygon approximation.
    pub fn paint_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        color: Color,
        canvas_color: Color,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        if self.record(DrawOp::PaintEllipse {
            cx,
            cy,
            rx,
            ry,
            color,
            canvas_color,
        }) {
            return;
        }
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let verts = self.ellipse_polygon(cx, cy, rx, ry);
        self.fill_polygon_aa(&verts, color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill an ellipse sector (pie slice) defined by center, radii, and angle range.
    /// Angles are in **degrees**, matching C++ emPainter convention.
    /// `start_angle` is the start in degrees from +X axis; `sweep_angle` is the
    /// arc length in degrees from start.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_ellipse_sector(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        sweep_angle: f64,
        color: Color,
        canvas_color: Color,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        if sweep_angle == 0.0 {
            return;
        }
        // Normalize negative sweep.
        if sweep_angle < 0.0 {
            return self.paint_ellipse_sector(
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
            return self.paint_ellipse(cx, cy, rx, ry, color, canvas_color);
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
        self.paint_polygon(&verts, color, canvas_color);
    }

    /// Fill a rectangle with a linear gradient between two colors.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_linear_gradient(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color_a: Color,
        color_b: Color,
        horizontal: bool,
        canvas_color: Color,
    ) {
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

        for row in start_y..end_y {
            for col in start_x..end_x {
                let color = interpolation::sample_linear_gradient(
                    start,
                    end,
                    color_a,
                    color_b,
                    (col as f64 + 0.5, row as f64 + 0.5),
                );
                self.blend_pixel_unchecked(col, row, color);
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
        color_inner: Color,
        color_outer: Color,
        canvas_color: Color,
    ) {
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

        let rows = scanline::rasterize(
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
                self.blit_span_textured(*y, span, &px_texture);
            }
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a line between two points.
    pub fn paint_line(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: Color,
        canvas_color: Color,
    ) {
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        let px0 = self.to_pixel_x(x0);
        let py0 = self.to_pixel_y(y0);
        let px1 = self.to_pixel_x(x1);
        let py1 = self.to_pixel_y(y1);
        self.draw_line_pixels(px0, py0, px1, py1, color);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon defined by a list of (x, y) vertices.
    /// Uses anti-aliased scanline rasterization with NonZero winding rule.
    pub fn paint_polygon(&mut self, vertices: &[(f64, f64)], color: Color, canvas_color: Color) {
        if self.record(DrawOp::PaintPolygon {
            vertices: vertices.to_vec(),
            color,
            canvas_color,
        }) {
            return;
        }
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        self.fill_polygon_aa(vertices, color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon using even-odd winding rule (for polygon rings with holes).
    pub fn paint_polygon_even_odd(
        &mut self,
        vertices: &[(f64, f64)],
        color: Color,
        canvas_color: Color,
    ) {
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        self.fill_polygon_aa(vertices, color, WindingRule::EvenOdd);
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon with a texture (gradient, image, or solid color).
    /// Uses anti-aliased scanline rasterization with NonZero winding rule.
    pub fn paint_polygon_textured(
        &mut self,
        vertices: &[(f64, f64)],
        texture: &Texture,
        canvas_color: Color,
    ) {
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if let Texture::SolidColor(color) = texture {
            self.fill_polygon_aa(vertices, *color, WindingRule::NonZero);
        } else {
            self.fill_polygon_aa_textured(vertices, texture, WindingRule::NonZero);
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Fill a polygon with a texture using even-odd winding rule.
    pub fn paint_polygon_textured_even_odd(
        &mut self,
        vertices: &[(f64, f64)],
        texture: &Texture,
        canvas_color: Color,
    ) {
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if let Texture::SolidColor(color) = texture {
            self.fill_polygon_aa(vertices, *color, WindingRule::EvenOdd);
        } else {
            self.fill_polygon_aa_textured(vertices, texture, WindingRule::EvenOdd);
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a polygon outline by stroking as a closed polyline with proper joins.
    pub fn paint_polygon_outlined(
        &mut self,
        vertices: &[(f64, f64)],
        stroke_color: Color,
        thickness: f64,
        canvas_color: Color,
    ) {
        if vertices.len() < 2 {
            return;
        }
        let stroke = Stroke::new(stroke_color, thickness);
        self.paint_polyline_without_arrows(vertices, &stroke, true, canvas_color);
    }

    /// Draw a polyline (open path) outline by stroking each segment.
    pub fn paint_polyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke_color: Color,
        thickness: f64,
        canvas_color: Color,
    ) {
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
            self.paint_polygon(
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
    /// Reads canvas_color from painter state (set by caller).
    pub fn paint_round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
        if self.record(DrawOp::PaintRoundRect {
            x,
            y,
            w,
            h,
            radius,
            color,
        }) {
            return;
        }
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let verts = self.round_rect_polygon(x, y, w, h, radius);
        self.fill_polygon_aa(&verts, color, WindingRule::NonZero);
    }

    /// Draw a source image at the given position (convenience wrapper).
    /// Draws at 1:1 scale with full opacity and no canvas color.
    pub fn paint_image(&mut self, x: f64, y: f64, image: &Image) {
        let iw = image.width() as f64 / self.state.scale_x;
        let ih = image.height() as f64 / self.state.scale_y;
        self.paint_image_full(x, y, iw, ih, image, 255, Color::TRANSPARENT);
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
        image: &Image,
        alpha: u8,
        canvas_color: Color,
    ) {
        if image.channel_count() != 4 || w <= 0.0 || h <= 0.0 || alpha == 0 {
            return;
        }
        if self.record(DrawOp::PaintImageFull {
            x,
            y,
            w,
            h,
            image_ptr: image as *const Image,
            alpha,
            canvas_color,
        }) {
            return;
        }

        let dx_px = x * self.state.scale_x + self.state.offset_x;
        let dy_px = y * self.state.scale_y + self.state.offset_y;
        let dw_px = w * self.state.scale_x;
        let dh_px = h * self.state.scale_y;
        let sp = SubPixelEdges::new(dx_px, dy_px, dw_px, dh_px);
        let px = sp.ix1;
        let py = sp.iy1;
        let pw = sp.ix2 - sp.ix1;
        let ph = sp.iy2 - sp.iy1;
        if pw <= 0 || ph <= 0 {
            return;
        }

        let src_w = image.width() as f64;
        let src_h = image.height() as f64;

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = py.max(cy1);
        let end_x = sp.ix2.min(cx2);
        let end_y = sp.iy2.min(cy2);

        // Save and temporarily override canvas color and alpha if specified.
        let saved_canvas = self.state.canvas_color;
        let saved_alpha = self.state.alpha;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // Match C++ emPainter_ScTl coordinate and interpolation conventions:
        // - EXTEND_EDGE_OR_ZERO: images with even channel count (incl. 4-ch RGBA)
        //   use EXTEND_ZERO; odd channel count uses EXTEND_EDGE.
        // - Upscaling uses adaptive (bicubic-like) with pixel-center offset (-0.5).
        // - Area sampling for downscaling (no pixel-center offset).
        // - 1:1 scale uses nearest-neighbor.
        let upscaling = (pw as f64) > src_w || (ph as f64) > src_h;
        let downscaling = (pw as f64) < src_w || (ph as f64) < src_h;

        let ext = if image.channel_count().is_multiple_of(2) {
            super::texture::ImageExtension::Zero
        } else {
            super::texture::ImageExtension::Clamp
        };

        // 24-bit fixed-point scaling transform matching C++ emPainter_ScTl.
        let sxfm = self.scale_transform_24(image.width(), image.height(), x, y, w, h);

        // Scanline batch pipeline: interpolate into buffer, then blend.
        // Borrow-split: extract state into locals before taking &mut data.
        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4);
        let max_batch = ibuf.max_pixels();

        if downscaling {
            // 24fp area sampling matching C++ ScanlineTool InterpolateImageAreaSampled.
            let iw = image.width();
            let ih = image.height();
            let tdx_init = ((iw as i64) << 24) as f64 / pw as f64;
            let tdy_init = ((ih as i64) << 24) as f64 / ph as f64;
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
            let red_w = iw.div_ceil(stride_x);
            let red_h = ih.div_ceil(stride_y);
            let off_x = (iw as i32 - (red_w as i32 - 1) * stride_x as i32 - 1) / 2;
            let off_y = (ih as i32 - (red_h as i32 - 1) * stride_y as i32 - 1) / 2;
            let mut xfm = self.area_sample_transform_24(red_w, red_h, x, y, w, h);
            xfm.stride_x = stride_x;
            xfm.stride_y = stride_y;
            xfm.off_x = off_x;
            xfm.off_y = off_y;
            let sec = interpolation::SectionBounds {
                ox: 0,
                oy: 0,
                w: iw as i32,
                h: ih as i32,
            };
            let mut coverages = vec![0i32; max_batch];
            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    interpolation::interpolate_scanline_area_sampled(
                        image, col, row, batch, &xfm, &sec, ext, &mut ibuf,
                    );
                    let all_full =
                        sp.batch_coverages(row, col, &mut coverages[..batch]);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.image().data_mut();
                    let dest = &mut data[dest_offset..];
                    if all_full {
                        blend_scanline(dest, &ibuf, batch, None, &mode);
                    } else {
                        blend_scanline(dest, &ibuf, batch, Some(&coverages[..batch]), &mode);
                    }
                    col += batch as i32;
                }
            }
        } else {
            let mut coverages = vec![0i32; max_batch];
            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    if upscaling {
                        interpolation::interpolate_scanline_adaptive_premul(
                            image, px, py, col, row, batch, &sxfm, ext, &mut ibuf,
                        );
                        let all_full =
                            sp.batch_coverages(row, col, &mut coverages[..batch]);
                        let dest_offset = (row as usize * tw + col as usize) * 4;
                        let data = self.image().data_mut();
                        let dest = &mut data[dest_offset..];
                        if all_full {
                            blend_scanline_premul(dest, &ibuf, batch, None, &mode);
                        } else {
                            blend_scanline_premul(
                                dest,
                                &ibuf,
                                batch,
                                Some(&coverages[..batch]),
                                &mode,
                            );
                        }
                    } else {
                        interpolation::interpolate_scanline_nearest(
                            image, px, py, col, row, batch, &sxfm, ext, &mut ibuf,
                        );
                        let all_full =
                            sp.batch_coverages(row, col, &mut coverages[..batch]);
                        let dest_offset = (row as usize * tw + col as usize) * 4;
                        let data = self.image().data_mut();
                        let dest = &mut data[dest_offset..];
                        if all_full {
                            blend_scanline(dest, &ibuf, batch, None, &mode);
                        } else {
                            blend_scanline(
                                dest,
                                &ibuf,
                                batch,
                                Some(&coverages[..batch]),
                                &mode,
                            );
                        }
                    }
                    col += batch as i32;
                }
            }
        }

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Draw an image with two-color mapping and canvas color support.
    /// Pixel luminance maps linearly from `color1` (at 0) to `color2` (at 255).
    /// For single-color alpha mask behavior, pass `Color::TRANSPARENT` as color1.
    /// Source region is (src_x, src_y, src_w, src_h) within the image.
    /// Matches C++ `PaintImageColored(x, y, w, h, img, color1, color2, canvasColor)`.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_image_colored(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &Image,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        color1: Color,
        color2: Color,
        canvas_color: Color,
        extension: ImageExtension,
    ) {
        if self.record(DrawOp::PaintImageColored {
            x,
            y,
            w,
            h,
            image_ptr: image as *const Image,
            src_x,
            src_y,
            src_w,
            src_h,
            color1,
            color2,
            canvas_color,
            extension,
        }) {
            return;
        }
        // Floating-point dest rect in pixel space (sub-pixel precision).
        let dx = x * self.state.scale_x + self.state.offset_x;
        let dy = y * self.state.scale_y + self.state.offset_y;
        let dw = w * self.state.scale_x;
        let dh = h * self.state.scale_y;

        let sp = SubPixelEdges::new(dx, dy, dw, dh);
        let px = sp.ix1;
        let py = sp.iy1;
        let px2 = sp.ix2;
        let py2 = sp.iy2;
        let pw = px2 - px;
        let ph = py2 - py;

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = py.max(cy1);
        let end_x = px2.min(cx2);
        let end_y = py2.min(cy2);

        if pw <= 0 || ph <= 0 || src_w == 0 || src_h == 0 {
            return;
        }

        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;

        let ch = image.channel_count();

        // C++ emPainter uses area sampling for downscaling (DQ_3X3 default),
        // nearest-neighbor for upscaling/1:1 with pixel-center offset.
        let src_w_f = src_w as f64;
        let src_h_f = src_h as f64;
        let ratio_x = src_w_f / dw;
        let ratio_y = src_h_f / dh;
        let downscaling = ratio_x > 1.0 || ratio_y > 1.0;

        let ext = extension.resolve_for_colored(color1, color2);

        // Helper: lum -> color mapping (shared between downscaling and non-downscaling paths).
        let lum_to_color = |lum: u8| -> Color {
            if color1.is_transparent() {
                let a = (lum as u32 * color2.a() as u32 + 127) / 255;
                Color::rgba(color2.r(), color2.g(), color2.b(), a as u8)
            } else if color2.is_transparent() {
                let inv = 255 - lum;
                let a = (inv as u32 * color1.a() as u32 + 127) / 255;
                Color::rgba(color1.r(), color1.g(), color1.b(), a as u8)
            } else {
                let t = lum as f64 / 255.0;
                color1.lerp(color2, t)
            }
        };

        // Scanline batch pipeline for colored images.
        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4); // output is always RGBA after lum mapping
        let max_batch = ibuf.max_pixels();
        let mut coverages = vec![0i32; max_batch];

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
            let sec = interpolation::SectionBounds {
                ox: src_x as i32,
                oy: src_y as i32,
                w: src_w as i32,
                h: src_h as i32,
            };

            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    // Interpolate, then apply lum->color mapping in-place
                    interpolation::interpolate_scanline_area_sampled(
                        image, col, row, batch, &xfm, &sec, ext, &mut ibuf,
                    );
                    for i in 0..batch {
                        let p = ibuf.pixel_rgba(i);
                        let lum = if ch == 1 {
                            p[0]
                        } else {
                            ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8)
                                as u8
                        };
                        let c = lum_to_color(lum);
                        ibuf.set_pixel(i, [c.r(), c.g(), c.b(), c.a()]);
                    }
                    let all_full =
                        sp.batch_coverages(row, col, &mut coverages[..batch]);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.image().data_mut();
                    let dest = &mut data[dest_offset..];
                    if all_full {
                        blend_scanline(dest, &ibuf, batch, None, &mode);
                    } else {
                        blend_scanline(dest, &ibuf, batch, Some(&coverages[..batch]), &mode);
                    }
                    col += batch as i32;
                }
            }
        } else {
            // Bilinear upscaling with lum mapping
            let sec = interpolation::SectionBounds {
                ox: src_x as i32,
                oy: src_y as i32,
                w: src_w as i32,
                h: src_h as i32,
            };
            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    for i in 0..batch {
                        let c = col + i as i32;
                        let fx = (c as f64 - dx + 0.5) * ratio_x - 0.5;
                        let fy = (row as f64 - dy + 0.5) * ratio_y - 0.5;
                        let ix = fx.floor() as i32;
                        let iy = fy.floor() as i32;
                        let tx = ((fx - fx.floor()) * 256.0) as u32;
                        let ty = ((fy - fy.floor()) * 256.0) as u32;
                        let itx = 256 - tx;
                        let ity = 256 - ty;

                        let p00 =
                            interpolation::sample_section_pixel(image, ix, iy, &sec, ext);
                        let p10 =
                            interpolation::sample_section_pixel(image, ix + 1, iy, &sec, ext);
                        let p01 =
                            interpolation::sample_section_pixel(image, ix, iy + 1, &sec, ext);
                        let p11 = interpolation::sample_section_pixel(
                            image,
                            ix + 1,
                            iy + 1,
                            &sec,
                            ext,
                        );

                        let bilinear_ch = |ch_idx: usize| -> u8 {
                            let top = p00[ch_idx] as u32 * itx + p10[ch_idx] as u32 * tx;
                            let bot = p01[ch_idx] as u32 * itx + p11[ch_idx] as u32 * tx;
                            ((top * ity + bot * ty + 0x8000) >> 16) as u8
                        };

                        let lum = if ch == 1 {
                            bilinear_ch(0)
                        } else {
                            let r = bilinear_ch(0) as u32;
                            let g = bilinear_ch(1) as u32;
                            let b = bilinear_ch(2) as u32;
                            ((r * 77 + g * 150 + b * 29) >> 8) as u8
                        };
                        let mapped = lum_to_color(lum);
                        ibuf.set_pixel(i, [mapped.r(), mapped.g(), mapped.b(), mapped.a()]);
                    }
                    ibuf.set_len(batch);
                    let all_full =
                        sp.batch_coverages(row, col, &mut coverages[..batch]);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.image().data_mut();
                    let dest = &mut data[dest_offset..];
                    if all_full {
                        blend_scanline(dest, &ibuf, batch, None, &mode);
                    } else {
                        blend_scanline(dest, &ibuf, batch, Some(&coverages[..batch]), &mode);
                    }
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
    ///   - If `formatted`: interprets `\n`, `\r\n`, `\t` (tabs align to 8).
    ///     Counts columns per line, tracks max columns and row count.
    ///   - If not formatted: character count = columns, 1 row.
    ///   - Width  = `char_height * columns / CHAR_BOX_TALLNESS`
    ///   - Height = `char_height * (1.0 + rel_line_space) * rows`
    pub fn get_text_size(
        text: &str,
        char_height: f64,
        formatted: bool,
        rel_line_space: f64,
    ) -> (f64, f64) {
        let (columns, rows) = if formatted {
            bitmap_font::measure_formatted(text)
        } else {
            (text.chars().count(), 1)
        };
        let w = char_height * columns as f64 / em_font::CHAR_BOX_TALLNESS;
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
    pub fn paint_text(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        char_height: f64,
        width_scale: f64,
        color: Color,
        canvas_color: Color,
    ) {
        if text.is_empty() || char_height <= 0.0 || color.a() == 0 {
            return;
        }
        if self.record(DrawOp::PaintText {
            x,
            y,
            text: text.to_string(),
            char_height,
            width_scale,
            color,
            canvas_color,
        }) {
            return;
        }

        let rcw = char_height / em_font::CHAR_BOX_TALLNESS;
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
                self.state.canvas_color,
            );
            return;
        }

        let clip_x1 = self.get_user_clip_x1();
        let clip_x2 = self.get_user_clip_x2();
        let clip_y1 = self.get_user_clip_y1();
        let clip_y2 = self.get_user_clip_y2();

        if y >= clip_y2 || y + char_height <= clip_y1 {
            return;
        }

        let gw = em_font::CHAR_WIDTH as f64;
        let gh = em_font::CHAR_HEIGHT as f64;
        let show_height = (rcw * gh / gw).min(char_height);
        let y_offset = (char_height - show_height) * 0.5;

        let saved_canvas = self.state.canvas_color;
        if canvas_color.is_opaque() {
            self.state.canvas_color = canvas_color;
        }

        let font_atlas = em_font::atlas();

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
            let (src_x, src_y, src_w, src_h) = em_font::get_glyph(ch);
            // C++ emPainter.cpp:2125 passes EXTEND_ZERO explicitly for font glyphs.
            self.paint_image_colored(
                cx,
                y + y_offset,
                char_width,
                show_height,
                font_atlas,
                src_x,
                src_y,
                src_w,
                src_h,
                Color::TRANSPARENT,
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
    fn paint_text_tiny(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        char_width: f64,
        char_height: f64,
        color: Color,
        canvas_color: Color,
    ) {
        let reduced_alpha = (color.a() as u32).div_ceil(3) as u8;
        let rc = color.with_alpha(reduced_alpha);
        let mut cx = x;
        let mut run_start: Option<f64> = None;

        for ch in text.chars() {
            if ch == ' ' {
                // Flush non-space run.
                if let Some(start) = run_start.take() {
                    self.paint_rect(start, y, cx - start, char_height, rc, canvas_color);
                }
            } else if run_start.is_none() {
                run_start = Some(cx);
            }
            cx += char_width;
        }
        // Flush final run.
        if let Some(start) = run_start {
            self.paint_rect(start, y, cx - start, char_height, rc, canvas_color);
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
    pub fn paint_text_boxed(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        text: &str,
        max_char_height: f64,
        color: Color,
        canvas_color: Color,
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
        if self.record(DrawOp::PaintTextBoxed {
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
        }) {
            return;
        }

        let (mut tw, mut th) =
            Self::get_text_size(text, max_char_height, formatted, rel_line_space);
        if tw <= 0.0 || th <= 0.0 {
            return;
        }

        // C++ PaintTextBoxed mutation-based fitting (emPainter.cpp lines 2175-2208).
        // All of ch, tw, th, ws are mutated together to maintain consistency.
        let mut ch = max_char_height;

        // Step 1: if text is taller than box, scale everything down proportionally.
        if th > h {
            ch *= h / th;
            tw *= h / th;
            th = h;
        }

        // Step 2: compute width scale and handle squeeze/expand.
        let mut ws = w / tw;
        if ws < 1.0 {
            // Text is wider than box — squeeze horizontally.
            tw = w;
            if ws < min_width_scale {
                // Can't squeeze enough — shrink char height to compensate.
                th *= ws / min_width_scale;
                ch *= ws / min_width_scale;
                ws = min_width_scale;
            }
        } else {
            // Text fits — don't stretch beyond 1.0 unless minWidthScale demands it.
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

        // Box alignment — position of text block within the box.
        // C++ compensates for trailing relLineSpace in vertical alignment.
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
            self.paint_text_formatted(
                bx,
                by,
                tw,
                text,
                ch,
                ws,
                color,
                canvas_color,
                text_alignment,
                rel_line_space,
            );
        } else {
            // C++ non-formatted: PaintText(x, y, text, ch, ws, color, canvasColor)
            // No per-line text alignment in non-formatted mode.
            self.paint_text(bx, by, text, ch, ws, color, self.state.canvas_color);
        }
    }

    /// Render formatted text (handles `\n`, `\r\n`, `\t`) line by line.
    #[allow(clippy::too_many_arguments)]
    fn paint_text_formatted(
        &mut self,
        bx: f64,
        by: f64,
        block_w: f64,
        text: &str,
        char_height: f64,
        width_scale: f64,
        color: Color,
        canvas_color: Color,
        text_alignment: TextAlignment,
        rel_line_space: f64,
    ) {
        let line_step = char_height * (1.0 + rel_line_space);
        let rcw = char_height / em_font::CHAR_BOX_TALLNESS * width_scale;
        let mut line_y = by;

        // Split on newlines, handling \r\n
        let normalized = text.replace("\r\n", "\n");
        for line in normalized.split('\n') {
            // Expand tabs to spaces (align to 8).
            let expanded = expand_tabs(line);
            let line_w = expanded.chars().count() as f64 * rcw;
            let lx = match text_alignment {
                TextAlignment::Left => bx,
                TextAlignment::Center => bx + (block_w - line_w) * 0.5,
                TextAlignment::Right => bx + block_w - line_w,
            };
            self.paint_text(
                lx,
                line_y,
                &expanded,
                char_height,
                width_scale,
                color,
                canvas_color,
            );
            line_y += line_step;
        }
    }

    /// Convenience: measure text width for a single un-formatted line.
    /// Returns the width in the same coordinate space as the painter.
    pub fn measure_text_width(text: &str, char_height: f64) -> f64 {
        char_height * text.chars().count() as f64 / em_font::CHAR_BOX_TALLNESS
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
        image: &Image,
        quality: super::texture::ImageQuality,
        extension: super::texture::ImageExtension,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        if self.record(DrawOp::PaintImageScaled {
            x,
            y,
            w,
            h,
            image_ptr: image as *const Image,
            quality,
            extension,
        }) {
            return;
        }

        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;
        if pw <= 0 || ph <= 0 {
            return;
        }

        let src_w = image.width() as f64;
        let src_h = image.height() as f64;

        // Auto-select area sampling for downscaling.
        let interp_quality = match quality {
            super::texture::ImageQuality::Nearest => interpolation::InterpolationQuality::Nearest,
            super::texture::ImageQuality::Bilinear => {
                if src_w > pw as f64 || src_h > ph as f64 {
                    interpolation::InterpolationQuality::AreaSampled
                } else {
                    interpolation::InterpolationQuality::Bilinear
                }
            }
            super::texture::ImageQuality::AreaSampled => {
                interpolation::InterpolationQuality::AreaSampled
            }
            super::texture::ImageQuality::Bicubic => interpolation::InterpolationQuality::Bicubic,
            super::texture::ImageQuality::Lanczos => interpolation::InterpolationQuality::Lanczos,
            super::texture::ImageQuality::Adaptive => interpolation::InterpolationQuality::Adaptive,
        };

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = py.max(cy1);
        let end_x = (px + pw).min(cx2);
        let end_y = (py + ph).min(cy2);

        let ctx = interpolation::ScaleContext {
            src_w,
            src_h,
            dest_w: pw as f64,
            dest_h: ph as f64,
        };

        for row in start_y..end_y {
            for col in start_x..end_x {
                let src_x = (col - px) as f64 * src_w / pw as f64;
                let src_y = (row - py) as f64 * src_h / ph as f64;
                let color =
                    interpolation::sample(image, src_x, src_y, interp_quality, extension, &ctx);
                self.blend_pixel_unchecked(col, row, color);
            }
        }
    }

    // --- Bezier curves ---

    /// Fill a cubic Bezier curve region (tessellated to polygon).
    /// `points` length must be a multiple of 3. Uses stride-3 convention:
    /// segment i uses points[i*3], points[i*3+1], points[i*3+2], points[((i+1)*3) % n].
    /// The path is implicitly closed.
    pub fn paint_bezier(&mut self, points: &[(f64, f64)], color: Color, canvas_color: Color) {
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
            self.fill_polygon_aa(&verts, color, WindingRule::NonZero);
        }
        self.state.canvas_color = saved_canvas;
    }

    /// Stroke a closed Bezier path outline (tessellated to polyline, then stroked).
    /// Corresponds to C++ `PaintBezierOutline`: tessellates + strokes as closed path.
    pub fn paint_bezier_outline(
        &mut self,
        points: &[(f64, f64)],
        stroke: &Stroke,
        canvas_color: Color,
    ) {
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
            self.paint_polyline_without_arrows(&verts, stroke, true, canvas_color);
        }
    }

    /// Stroke a cubic Bezier curve (tessellated to polyline).
    /// For open paths, `points` length must be 3k+1. For closed paths, 3k.
    /// Uses stride-3 convention.
    pub fn paint_bezier_line(
        &mut self,
        points: &[(f64, f64)],
        stroke: &Stroke,
        canvas_color: Color,
    ) {
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
        if verts.len() >= 2 {
            self.paint_polyline_with_arrows(&verts, stroke, closed, canvas_color);
        }
    }

    // --- 9-slice border images ---

    /// Draw a 9-slice border image stretched to fill a rectangle.
    ///
    /// `l,t,r,b` are **target** insets (logical coordinates).
    /// `src_l,src_t,src_r,src_b` are **source** insets (image pixel coordinates).
    /// `which_sub_rects` bitmask: `BORDER_EDGES_ONLY` (0o757) draws all except center.
    /// `canvas_color`: when not opaque, target inset boundaries are pixel-rounded.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_border_image(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        image: &Image,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        alpha: u8,
        canvas_color: Color,
        which_sub_rects: u16,
    ) {
        if alpha == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        if self.record(DrawOp::PaintBorderImage {
            x,
            y,
            w,
            h,
            l,
            t,
            r,
            b,
            image_ptr: image as *const Image,
            src_l,
            src_t,
            src_r,
            src_b,
            alpha,
            canvas_color,
            which_sub_rects,
        }) {
            return;
        }
        let iw = image.width() as f64;
        let ih = image.height() as f64;
        let quality = super::texture::ImageQuality::Bilinear;
        let ext = super::texture::ImageExtension::Clamp;

        // Target insets (logical).
        let mut l = l.min(w / 2.0);
        let mut r = r.min(w / 2.0);
        let mut t = t.min(h / 2.0);
        let mut b = b.min(h / 2.0);

        // R-6: pixel-round inset boundaries when canvas_color is not opaque.
        if !canvas_color.is_opaque() {
            let f = self.round_x(x + l) - x;
            if f > 0.0 && f < w - r {
                l = f;
            }
            let f = x + w - self.round_x(x + w - r);
            if f > 0.0 && f < w - l {
                r = f;
            }
            let f = self.round_y(y + t) - y;
            if f > 0.0 && f < h - b {
                t = f;
            }
            let f = y + h - self.round_y(y + h - b);
            if f > 0.0 && f < h - t {
                b = f;
            }
        }

        // Source insets (pixel coords).
        let sl = src_l as f64;
        let st = src_t as f64;
        let sr = src_r as f64;
        let sb = src_b as f64;

        // Source center region.
        let src_cx = iw - sl - sr;
        let src_cy = ih - st - sb;

        // Destination center region.
        let dst_cx = w - l - r;
        let dst_cy = h - t - b;

        let saved_alpha = self.state.alpha;
        let saved_canvas = self.state.canvas_color;
        self.state.canvas_color = canvas_color;
        if alpha < 255 {
            self.state.alpha = ((self.state.alpha as u16 * alpha as u16 + 128) >> 8) as u8;
        }

        // Bit layout (octal digit positions):
        //  8=UL  5=U   2=UR
        //  7=L   4=C   1=R
        //  6=LL  3=B   0=LR

        // Corners.
        if which_sub_rects & (1 << 8) != 0 {
            self.paint_9slice_section(x, y, l, t, image, 0.0, 0.0, sl, st, quality, ext);
        }
        if which_sub_rects & (1 << 2) != 0 {
            self.paint_9slice_section(
                x + w - r,
                y,
                r,
                t,
                image,
                iw - sr,
                0.0,
                sr,
                st,
                quality,
                ext,
            );
        }
        if which_sub_rects & (1 << 6) != 0 {
            self.paint_9slice_section(
                x,
                y + h - b,
                l,
                b,
                image,
                0.0,
                ih - sb,
                sl,
                sb,
                quality,
                ext,
            );
        }
        if which_sub_rects & (1 << 0) != 0 {
            self.paint_9slice_section(
                x + w - r,
                y + h - b,
                r,
                b,
                image,
                iw - sr,
                ih - sb,
                sr,
                sb,
                quality,
                ext,
            );
        }

        // Edges.
        if dst_cx > 0.0 {
            if which_sub_rects & (1 << 5) != 0 {
                self.paint_9slice_section(
                    x + l,
                    y,
                    dst_cx,
                    t,
                    image,
                    sl,
                    0.0,
                    src_cx,
                    st,
                    quality,
                    ext,
                );
            }
            if which_sub_rects & (1 << 3) != 0 {
                self.paint_9slice_section(
                    x + l,
                    y + h - b,
                    dst_cx,
                    b,
                    image,
                    sl,
                    ih - sb,
                    src_cx,
                    sb,
                    quality,
                    ext,
                );
            }
        }
        if dst_cy > 0.0 {
            if which_sub_rects & (1 << 7) != 0 {
                self.paint_9slice_section(
                    x,
                    y + t,
                    l,
                    dst_cy,
                    image,
                    0.0,
                    st,
                    sl,
                    src_cy,
                    quality,
                    ext,
                );
            }
            if which_sub_rects & (1 << 1) != 0 {
                self.paint_9slice_section(
                    x + w - r,
                    y + t,
                    r,
                    dst_cy,
                    image,
                    iw - sr,
                    st,
                    sr,
                    src_cy,
                    quality,
                    ext,
                );
            }
        }

        // Center.
        if which_sub_rects & (1 << 4) != 0 && dst_cx > 0.0 && dst_cy > 0.0 {
            self.paint_9slice_section(
                x + l,
                y + t,
                dst_cx,
                dst_cy,
                image,
                sl,
                st,
                src_cx,
                src_cy,
                quality,
                ext,
            );
        }

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Draw a 9-slice border image with two-color tinting.
    ///
    /// `l,t,r,b` are **target** insets (logical coordinates).
    /// `src_l,src_t,src_r,src_b` are **source** insets (image pixel coordinates).
    #[allow(clippy::too_many_arguments)]
    pub fn paint_border_image_colored(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        image: &Image,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        color1: Color,
        color2: Color,
        canvas_color: Color,
        which_sub_rects: u16,
        alpha: u8,
    ) {
        if alpha == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let iw = image.width() as f64;
        let ih = image.height() as f64;

        let mut l = l.min(w / 2.0);
        let mut r = r.min(w / 2.0);
        let mut t = t.min(h / 2.0);
        let mut b = b.min(h / 2.0);

        if !canvas_color.is_opaque() {
            let f = self.round_x(x + l) - x;
            if f > 0.0 && f < w - r {
                l = f;
            }
            let f = x + w - self.round_x(x + w - r);
            if f > 0.0 && f < w - l {
                r = f;
            }
            let f = self.round_y(y + t) - y;
            if f > 0.0 && f < h - b {
                t = f;
            }
            let f = y + h - self.round_y(y + h - b);
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
            self.paint_image_colored(
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
                ImageExtension::EdgeOrZero,
            );
        }
        if which_sub_rects & (1 << 2) != 0 {
            self.paint_image_colored(
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
                ImageExtension::EdgeOrZero,
            );
        }
        if which_sub_rects & (1 << 6) != 0 {
            self.paint_image_colored(
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
                ImageExtension::EdgeOrZero,
            );
        }
        if which_sub_rects & (1 << 0) != 0 {
            self.paint_image_colored(
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
                ImageExtension::EdgeOrZero,
            );
        }

        // Edges.
        if dst_cx > 0.0 {
            if which_sub_rects & (1 << 5) != 0 {
                self.paint_image_colored(
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
                    ImageExtension::EdgeOrZero,
                );
            }
            if which_sub_rects & (1 << 3) != 0 {
                self.paint_image_colored(
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
                    ImageExtension::EdgeOrZero,
                );
            }
        }
        if dst_cy > 0.0 {
            if which_sub_rects & (1 << 7) != 0 {
                self.paint_image_colored(
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
                    ImageExtension::EdgeOrZero,
                );
            }
            if which_sub_rects & (1 << 1) != 0 {
                self.paint_image_colored(
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
                    ImageExtension::EdgeOrZero,
                );
            }
        }

        // Center.
        if which_sub_rects & (1 << 4) != 0 && dst_cx > 0.0 && dst_cy > 0.0 {
            self.paint_image_colored(
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
                ImageExtension::EdgeOrZero,
            );
        }

        self.state.canvas_color = saved_canvas;
        self.state.alpha = saved_alpha;
    }

    /// Helper for 9-slice: draw a sub-region of an image scaled to a destination rect.
    #[allow(clippy::too_many_arguments)]
    fn paint_9slice_section(
        &mut self,
        dx: f64,
        dy: f64,
        dw: f64,
        dh: f64,
        image: &Image,
        sx: f64,
        sy: f64,
        sw: f64,
        sh: f64,
        _quality: super::texture::ImageQuality,
        extension: super::texture::ImageExtension,
    ) {
        if dw <= 0.0 || dh <= 0.0 || sw <= 0.0 || sh <= 0.0 {
            return;
        }

        let dx_px = dx * self.state.scale_x + self.state.offset_x;
        let dy_px = dy * self.state.scale_y + self.state.offset_y;
        let dw_px = dw * self.state.scale_x;
        let dh_px = dh * self.state.scale_y;
        let sp = SubPixelEdges::new(dx_px, dy_px, dw_px, dh_px);
        let px = sp.ix1;
        let py = sp.iy1;
        let px2 = sp.ix2;
        let py2 = sp.iy2;
        let pw = px2 - px;
        let ph = py2 - py;
        if pw <= 0 || ph <= 0 {
            return;
        }

        let cx1 = (self.state.clip.x1 as i32).max(0);
        let cy1 = (self.state.clip.y1 as i32).max(0);
        let cx2 = (self.state.clip.x2.ceil() as i32).min(self.target_width as i32);
        let cy2 = (self.state.clip.y2.ceil() as i32).min(self.target_height as i32);
        let start_x = px.max(cx1);
        let start_y = py.max(cy1);
        let end_x = px2.min(cx2);
        let end_y = py2.min(cy2);

        // Match C++ emPainter scaling: pre-reduced area sampling for downscaling,
        // adaptive for upscaling (UQ_ADAPTIVE default).
        let ratio_x = sw / dw_px;
        let ratio_y = sh / dh_px;
        let downscaling = ratio_x > 1.0 || ratio_y > 1.0;

        // Scanline batch pipeline for 9-slice sections.
        let tw = self.target_width as usize;
        let mode = BlendMode::from_state(self.state.canvas_color, self.state.alpha);
        let mut ibuf = InterpolationBuffer::new(4);
        let max_batch = ibuf.max_pixels();
        let mut coverages = vec![0i32; max_batch];

        if downscaling {
            // 24-bit fixed-point area sampling matching C++ emPainter_ScTl + InterpolateImageAreaSampled.
            let sw_u = sw as u32;
            let sh_u = sh as u32;

            let tdx_init = ((sw_u as i64) << 24) as f64 / dw_px;
            let tdy_init = ((sh_u as i64) << 24) as f64 / dh_px;
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

            let red_w = sw_u.div_ceil(stride_x);
            let red_h = sh_u.div_ceil(stride_y);

            let off_x = (sw_u as i32 - (red_w as i32 - 1) * stride_x as i32 - 1) / 2;
            let off_y = (sh_u as i32 - (red_h as i32 - 1) * stride_y as i32 - 1) / 2;

            let mut xfm = self.area_sample_transform_24(red_w, red_h, dx, dy, dw, dh);
            xfm.stride_x = stride_x;
            xfm.stride_y = stride_y;
            xfm.off_x = off_x;
            xfm.off_y = off_y;
            let sec = interpolation::SectionBounds {
                ox: sx as i32,
                oy: sy as i32,
                w: sw as i32,
                h: sh as i32,
            };

            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    interpolation::interpolate_scanline_area_sampled(
                        image,
                        col,
                        row,
                        batch,
                        &xfm,
                        &sec,
                        super::texture::ImageExtension::Clamp,
                        &mut ibuf,
                    );
                    let all_full =
                        sp.batch_coverages(row, col, &mut coverages[..batch]);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.image().data_mut();
                    let dest = &mut data[dest_offset..];
                    if all_full {
                        blend_scanline(dest, &ibuf, batch, None, &mode);
                    } else {
                        blend_scanline(dest, &ibuf, batch, Some(&coverages[..batch]), &mode);
                    }
                    col += batch as i32;
                }
            }
        } else {
            // Upscaling or 1:1: adaptive interpolation matching C++ UQ_ADAPTIVE (default).
            let sxfm = self.scale_transform_24(sw as u32, sh as u32, dx, dy, dw, dh);
            let sec = interpolation::SectionBounds {
                ox: sx as i32,
                oy: sy as i32,
                w: sw as i32,
                h: sh as i32,
            };

            for row in start_y..end_y {
                let mut col = start_x;
                while col < end_x {
                    let batch = ((end_x - col) as usize).min(max_batch);
                    interpolation::interpolate_scanline_adaptive_premul_section(
                        image, px, py, col, row, batch, &sxfm, &sec, extension, &mut ibuf,
                    );
                    let all_full =
                        sp.batch_coverages(row, col, &mut coverages[..batch]);
                    let dest_offset = (row as usize * tw + col as usize) * 4;
                    let data = self.image().data_mut();
                    let dest = &mut data[dest_offset..];
                    if all_full {
                        blend_scanline_premul(dest, &ibuf, batch, None, &mode);
                    } else {
                        blend_scanline_premul(
                            dest,
                            &ibuf,
                            batch,
                            Some(&coverages[..batch]),
                            &mode,
                        );
                    }
                    col += batch as i32;
                }
            }
        }
    }

    // --- Ellipse/sector outline utilities ---

    /// Stroke an arc of an ellipse (no radii, just the curved portion).
    #[allow(clippy::too_many_arguments)]
    pub fn paint_ellipse_arc(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        range_angle: f64,
        stroke: &Stroke,
        canvas_color: Color,
    ) {
        if rx <= 0.0 || ry <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        if range_angle == 0.0 {
            return;
        }
        let abs_range = range_angle.abs();
        if abs_range >= 2.0 * std::f64::consts::PI {
            self.paint_ellipse_outlined(cx, cy, rx, ry, stroke, canvas_color);
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
        self.paint_solid_polyline(&verts, stroke, false, canvas_color);
    }

    /// Draw an ellipse sector outline. Routes through polyline if dashed.
    #[allow(clippy::too_many_arguments)]
    /// Outline an ellipse sector. Angles in **degrees** (start + sweep), matching C++.
    pub fn paint_ellipse_sector_outlined(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        sweep_angle: f64,
        stroke: &Stroke,
        canvas_color: Color,
    ) {
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
            self.paint_polyline_without_arrows(&verts, stroke, true, canvas_color);
        } else {
            self.paint_polygon_outlined(&verts, stroke.color, stroke.width, canvas_color);
        }
    }

    /// Draw a rectangle outline. Stroke is centered on the shape boundary.
    ///
    /// Matches C++ `PaintRectOutline`: for solid non-rounded strokes, builds a
    /// 10-vertex polygon (outer rect + bridge + reversed inner rect). For
    /// dashed/rounded strokes, routes through `PaintPolylineWithoutArrows`.
    pub fn paint_rect_outlined(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stroke: &Stroke,
        canvas_color: Color,
    ) {
        if self.record(DrawOp::PaintRectOutlined {
            x,
            y,
            w,
            h,
            stroke: stroke.clone(),
            canvas_color,
        }) {
            return;
        }
        let sw = stroke.width;
        let w = w.max(0.0);
        let h = h.max(0.0);
        if sw <= 0.0 {
            return;
        }
        let t2 = sw * 0.5;
        let rounded = stroke.join == super::stroke::LineJoin::Round
            || stroke.cap == super::stroke::LineCap::Round;

        if rounded || stroke.is_dashed() {
            if (w <= sw || h <= sw) && !stroke.is_dashed() {
                self.paint_round_rect(x - t2, y - t2, w + sw, h + sw, t2, stroke.color);
                return;
            }
            let verts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
            self.paint_polyline_without_arrows(&verts, stroke, true, canvas_color);
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
            // Stroke fills entire rect.
            self.paint_polygon(
                &[(ox1, oy1), (ox2, oy1), (ox2, oy2), (ox1, oy2)],
                stroke.color,
                self.state.canvas_color,
            );
            return;
        }

        // 10-vertex polygon: outer CW, bridge, inner CCW, bridge back.
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
        self.fill_polygon_aa(&poly, stroke.color, WindingRule::NonZero);
    }

    /// Draw a rounded rectangle outline. Stroke is centered on the shape boundary.
    ///
    /// Matches C++ `PaintRoundRectOutline`: for solid strokes, builds outer +
    /// inner round-rect polygons with a bridge for NonZero winding hole.
    /// For dashed, routes through polyline.
    /// Reads canvas_color from painter state (set by caller).
    pub fn paint_round_rect_outlined(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        stroke: &Stroke,
    ) {
        if w <= 0.0 || h <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        if self.record(DrawOp::PaintRoundRectOutlined {
            x,
            y,
            w,
            h,
            radius,
            stroke: stroke.clone(),
        }) {
            return;
        }
        let sw = stroke.width;
        let t2 = sw * 0.5;

        if stroke.is_dashed() {
            let verts = self.round_rect_polygon(x, y, w, h, radius);
            self.paint_polyline_without_arrows(&verts, stroke, true, self.state.canvas_color);
            return;
        }

        // Outer round-rect expanded by t2 on each side.
        let ox = x - t2;
        let oy = y - t2;
        let ow = w + sw;
        let oh = h + sw;
        let or = radius + t2;

        if sw * 2.0 >= w || sw * 2.0 >= h {
            self.paint_round_rect(ox, oy, ow, oh, or, stroke.color);
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
        self.fill_polygon_aa(&outer, stroke.color, WindingRule::NonZero);
    }

    /// Draw an ellipse outline. Stroke is centered on the shape boundary.
    ///
    /// Matches C++ `PaintEllipseOutline`: for solid strokes, builds
    /// outer + inner ellipse polygons with adaptive segment counts and a
    /// bridge for NonZero winding hole. For dashed, routes through polyline.
    pub fn paint_ellipse_outlined(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        stroke: &Stroke,
        canvas_color: Color,
    ) {
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
            self.paint_polyline_without_arrows(&verts, stroke, true, canvas_color);
            return;
        }

        // Inner radii contracted by t2 from shape boundary.
        let irx = orx - sw;
        let iry = ory - sw;
        if irx <= 0.0 || iry <= 0.0 {
            self.paint_ellipse(cx, cy, orx, ory, stroke.color, canvas_color);
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
        self.fill_polygon_aa(&outer, stroke.color, WindingRule::NonZero);
    }

    /// Correct blending artifacts along a shared edge between two adjacent polygons.
    ///
    /// Walks along the edge using DDA stepping, computes area coverage for both
    /// sides, and blends a correction pixel. Corresponds to C++ `PaintEdgeCorrection`.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_edge_correction(
        &mut self,
        mut x1: f64,
        mut y1: f64,
        mut x2: f64,
        mut y2: f64,
        mut color1: Color,
        mut color2: Color,
    ) {
        // Transform to pixel coordinates.
        x1 = x1 * self.state.scale_x + self.state.offset_x;
        y1 = y1 * self.state.scale_y + self.state.offset_y;
        x2 = x2 * self.state.scale_x + self.state.offset_x;
        y2 = y2 * self.state.scale_y + self.state.offset_y;

        // Ensure y1 <= y2.
        if y1 > y2 {
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y1, &mut y2);
            std::mem::swap(&mut color1, &mut color2);
        }

        if color1.a() == 0 || color2.a() == 0 {
            return;
        }

        let dx = x2 - x1;
        let dy = y2 - y1;
        let adx = dx.abs();

        if dy < 0.0001 && adx < 0.0001 {
            return;
        }

        let gx = if dy >= 0.0001 { dx / dy } else { 0.0 };
        let gy = if adx >= 0.0001 { dy / adx } else { 0.0 };

        let clip_x1f = self.state.clip.x1;
        let clip_y1f = self.state.clip.y1;
        let clip_x2f = self.state.clip.x2;
        let clip_y2f = self.state.clip.y2;

        if y1 < clip_y1f {
            x1 += gx * (clip_y1f - y1);
            y1 = clip_y1f;
        }
        if y2 > clip_y2f {
            x2 += gx * (clip_y2f - y2);
            y2 = clip_y2f;
        }
        if y1 >= y2 {
            return;
        }

        if dx >= 0.0 {
            if x1 < clip_x1f {
                y1 += gy * (clip_x1f - x1);
                x1 = clip_x1f;
            }
            if x2 > clip_x2f {
                y2 += gy * (clip_x2f - x2);
                x2 = clip_x2f;
            }
        } else {
            if x2 < clip_x1f {
                y2 -= gy * (clip_x1f - x2);
                x2 = clip_x1f;
            }
            if x1 > clip_x2f {
                y1 -= gy * (x1 - clip_x2f);
                x1 = clip_x2f;
            }
        }

        if y1 >= y2 {
            return;
        }

        let cy1 = y1.floor() as i32;
        let cy2 = y2.ceil() as i32;
        let (cx1, cx2) = if dx >= 0.0 {
            (x1.floor() as i32, x2.ceil() as i32)
        } else {
            (x2.floor() as i32, x1.ceil() as i32)
        };

        let mut sx = if dx >= 0.0 {
            x1.floor() as i32
        } else {
            x1.ceil() as i32 - 1
        };
        let mut sy = y1.floor() as i32;

        let tw = self.target_width as i32;
        let th = self.target_height as i32;

        loop {
            if sy >= cy2 {
                break;
            }
            if sx < cx1 || sx >= cx2 || sy < cy1 {
                sy += 1;
                continue;
            }

            if sx >= 0 && sx < tw && sy >= 0 && sy < th {
                let px_left = sx as f64;
                let px_right = (sx + 1) as f64;
                let ey_top = (sy as f64).max(y1);
                let ey_bot = ((sy + 1) as f64).min(y2);

                if ey_top < ey_bot {
                    let ex_top = x1 + gx * (ey_top - y1);
                    let ex_bot = x1 + gx * (ey_bot - y1);
                    let edge_x_min = ex_top.min(ex_bot).max(px_left);
                    let edge_x_max = ex_top.max(ex_bot).min(px_right);
                    let edge_y_span = ey_bot - ey_top;
                    let mid_x = (edge_x_min + edge_x_max) * 0.5;
                    let a1 = ((mid_x - px_left) * edge_y_span).clamp(0.0, 1.0);
                    let a2 = ((px_right - mid_x) * edge_y_span).clamp(0.0, 1.0);

                    if a1 >= 0.001 && a2 >= 0.001 {
                        let inv = 1.0 / ((1.0 - a1) * (1.0 - a2));
                        let t = (255.0 * (1.0 - inv.min(1.0))).max(0.0);
                        let alpha3 = (t * a1 * a2) as i32;

                        if alpha3 > 0 {
                            let total_a = a1 + a2;
                            let w1 = a1 / total_a;
                            let w2 = a2 / total_a;
                            let cr =
                                (color1.r() as f64 * w1 + color2.r() as f64 * w2).round() as u8;
                            let cg =
                                (color1.g() as f64 * w1 + color2.g() as f64 * w2).round() as u8;
                            let cb =
                                (color1.b() as f64 * w1 + color2.b() as f64 * w2).round() as u8;
                            let ca = alpha3.min(255) as u8;
                            let correction = Color::rgba(cr, cg, cb, ca);
                            self.blend_pixel(sx, sy, correction);
                        }
                    }
                }
            }

            if dx >= 0.0 {
                if (sy as f64 + 1.0 - y1) * dx > (sx as f64 + 1.0 - x1) * dy {
                    sx += 1;
                    if sx >= cx2 {
                        break;
                    }
                } else {
                    sy += 1;
                    if sy >= cy2 {
                        break;
                    }
                }
            } else if (sy as f64 + 1.0 - y1) * dx < (sx as f64 - x1) * dy {
                sx -= 1;
                if sx < cx1 {
                    break;
                }
            } else {
                sy += 1;
                if sy >= cy2 {
                    break;
                }
            }
        }
    }

    /// Fill the current clip rect with a solid color.
    pub fn clear(&mut self, color: Color) {
        let x = self.state.clip.x1 as i32;
        let y = self.state.clip.y1 as i32;
        let w = self.state.clip.x2.ceil() as i32 - x;
        let h = self.state.clip.y2.ceil() as i32 - y;
        self.fill_rect_pixels(x, y, w, h, color);
    }

    /// Draw a dashed polyline by splitting the path into dash/gap segments
    /// and painting each dash as a solid sub-polyline.
    pub fn paint_dashed_polyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &Stroke,
        closed: bool,
        canvas_color: Color,
    ) {
        use super::stroke::DashType;

        if vertices.len() < 2 || stroke.width <= 0.0 {
            self.paint_solid_polyline(vertices, stroke, closed, canvas_color);
            return;
        }

        // Route: if C++ dash_type API is set, use the fitted algorithm.
        if stroke.dash_type != DashType::Solid {
            self.paint_dashed_polyline_fitted(vertices, stroke, closed);
            return;
        }

        // Legacy pattern-based dashes.
        if stroke.dash_pattern.is_empty() {
            self.paint_solid_polyline(vertices, stroke, closed, canvas_color);
            return;
        }
        let pattern = &stroke.dash_pattern;
        let total_pattern_len: f64 = pattern.iter().sum();
        if total_pattern_len <= 0.0 {
            self.paint_solid_polyline(vertices, stroke, closed, canvas_color);
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
        let dash_stroke = Stroke {
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
                        self.paint_solid_polyline(
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
            self.paint_solid_polyline(&current_segment, &dash_stroke, false, canvas_color);
        }
    }

    /// C++ `PaintDashedPolyline` port: fits dashes to total path length.
    fn paint_dashed_polyline_fitted(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &Stroke,
        closed: bool,
    ) {
        use super::stroke::DashType;

        const MAX_DASHES: f64 = 1e5;

        let n = vertices.len();
        if n < 2 {
            self.paint_solid_polyline(vertices, stroke, closed, self.state.canvas_color);
            return;
        }

        let thickness = stroke.width;
        let rounded = stroke.join == super::stroke::LineJoin::Round
            || stroke.cap == super::stroke::LineCap::Round;
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
                self.paint_solid_polyline(vertices, stroke, closed, self.state.canvas_color);
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
                self.paint_solid_polyline(vertices, stroke, closed, self.state.canvas_color);
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
            let a = (stroke.color.a() as f64 * t_solid + 0.5) as u8;
            solid_stroke.color = solid_stroke.color.with_alpha(a);
            self.paint_solid_polyline(vertices, &solid_stroke, closed, self.state.canvas_color);
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

        let cap_end = StrokeEnd::new(StrokeEndType::Cap);
        let butt_end = StrokeEnd::butt();

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
                self.paint_solid_polyline(&xy_out, &solid_stroke, false, self.state.canvas_color);
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
    pub fn paint_polyline_without_arrows(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &Stroke,
        closed: bool,
        canvas_color: Color,
    ) {
        if stroke.is_dashed() {
            self.paint_dashed_polyline(vertices, stroke, closed, canvas_color);
        } else {
            self.paint_solid_polyline(vertices, stroke, closed, canvas_color);
        }
    }

    /// Dispatch polyline rendering with arrow support.
    /// Corresponds to C++ `PaintPolyline`: checks for arrow decorations,
    /// computes direction vectors, shortens endpoints, then paints arrows.
    pub fn paint_polyline_with_arrows(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &Stroke,
        closed: bool,
        canvas_color: Color,
    ) {
        if vertices.len() < 2 {
            return;
        }
        let has_start_arrow = !closed && stroke.start_end.is_decorated();
        let has_end_arrow = !closed && stroke.finish_end.is_decorated();

        if !has_start_arrow && !has_end_arrow {
            self.paint_polyline_without_arrows(vertices, stroke, closed, canvas_color);
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

        let rounded = stroke.join == super::stroke::LineJoin::Round
            || stroke.cap == super::stroke::LineCap::Round;

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
            self.paint_polyline_without_arrows(body, stroke, closed, canvas_color);
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
    pub fn paint_solid_polyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke: &Stroke,
        closed: bool,
        canvas_color: Color,
    ) {
        if vertices.is_empty() || stroke.width <= 0.0 {
            return;
        }
        if self.record(DrawOp::PaintSolidPolyline {
            vertices: vertices.to_vec(),
            stroke: stroke.clone(),
            closed,
            canvas_color,
        }) {
            return;
        }

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
        let rounded = stroke.join == super::stroke::LineJoin::Round
            || stroke.cap == super::stroke::LineCap::Round;

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

        self.fill_polygon_aa(&poly, stroke.color, WindingRule::NonZero);
        self.state.canvas_color = saved_canvas;
    }

    /// Draw a stroked line with optional end decorations.
    pub fn paint_line_stroked(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        stroke: &Stroke,
        canvas_color: Color,
    ) {
        // For width=1 with no decorations and no rounding, simple line.
        if stroke.width <= 1.0
            && !stroke.start_end.is_decorated()
            && !stroke.finish_end.is_decorated()
            && stroke.join != super::stroke::LineJoin::Round
        {
            self.paint_line(x0, y0, x1, y1, stroke.color, canvas_color);
            return;
        }

        // Route through the polyline system which handles caps, joins,
        // decorations, and dashes correctly — matching C++ PaintLine.
        let verts = [(x0, y0), (x1, y1)];
        self.paint_polyline_with_arrows(&verts, stroke, false, canvas_color);
    }

    /// Calculate the maximum radius that a line point (including any arrow
    /// decorations) can extend from the vertex. Used for clip-rectangle
    /// expansion when testing visibility.
    /// Corresponds to C++ `CalculateLinePointMinMaxRadius`.
    pub fn calculate_line_point_min_max_radius(
        thickness: f64,
        stroke: &Stroke,
        stroke_start: &StrokeEnd,
        stroke_end: &StrokeEnd,
    ) -> f64 {
        let mut r = thickness * 0.5;
        if stroke.join != super::stroke::LineJoin::Round {
            r *= MAX_MITER.max(1.415);
        }
        if stroke_start.is_decorated() {
            let w = thickness * ARROW_BASE_SIZE * 0.5 * stroke_start.width_factor;
            let l = thickness * ARROW_BASE_SIZE * stroke_start.length_factor;
            r = r.max((w * w + l * l).sqrt());
        }
        if stroke_end.is_decorated() {
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
        stroke: &Stroke,
        end: &StrokeEnd,
    ) -> f64 {
        let mut r = (thickness * ARROW_BASE_SIZE * 0.5 * end.width_factor).abs();
        if r <= 1e-140 {
            return 0.0;
        }
        let mut l = thickness * ARROW_BASE_SIZE * end.length_factor;
        if l <= 1e-140 {
            return 0.0;
        }
        let rounded = stroke.join == super::stroke::LineJoin::Round
            || stroke.cap == super::stroke::LineCap::Round;

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
            StrokeEndType::Stroke => {
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
        stroke_color: Color,
        stroke_end: &StrokeEnd,
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

        // Stroke for sub-drawing (outlines, open polylines).
        // Matches C++ `arrowStroke = stroke; arrowStroke.DashType = SOLID;`.
        let arrow_stroke = {
            let mut s = Stroke::new(stroke_color, thickness);
            if rounded {
                s.join = super::stroke::LineJoin::Round;
                s.cap = super::stroke::LineCap::Round;
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
                self.paint_polygon(
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
                self.paint_polygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.paint_polyline_without_arrows(
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
                line_stroke.start_end = StrokeEnd::new(StrokeEndType::Cap);
                line_stroke.finish_end = StrokeEnd::new(StrokeEndType::Cap);
                self.paint_polyline_without_arrows(
                    &verts,
                    &line_stroke,
                    false,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::Triangle => {
                self.paint_polygon(
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
                self.paint_polygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.paint_polyline_without_arrows(
                    &verts,
                    &arrow_stroke,
                    true,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::Square => {
                self.paint_polygon(
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
                self.paint_polygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.paint_polyline_without_arrows(
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
                hs_stroke.start_end = StrokeEnd::new(StrokeEndType::Cap);
                hs_stroke.finish_end = StrokeEnd::new(StrokeEndType::Cap);
                self.paint_polyline_without_arrows(
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
                self.paint_bezier(&bezier_pts, stroke_color, self.state.canvas_color);
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
                self.paint_bezier(&bezier_pts, stroke_end.inner_color, self.state.canvas_color);
                self.paint_bezier_outline(&bezier_pts, &arrow_stroke, self.state.canvas_color);
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
                    hc_stroke.start_end = StrokeEnd::new(StrokeEndType::Cap);
                    hc_stroke.finish_end = StrokeEnd::new(StrokeEndType::Cap);
                }
                self.paint_bezier_line(&bezier_pts, &hc_stroke, self.state.canvas_color);
            }

            StrokeEndType::Diamond => {
                self.paint_polygon(
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
                self.paint_polygon(&verts, stroke_end.inner_color, self.state.canvas_color);
                self.paint_polyline_without_arrows(
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
                hd_stroke.start_end = StrokeEnd::new(StrokeEndType::Cap);
                hd_stroke.finish_end = StrokeEnd::new(StrokeEndType::Cap);
                self.paint_polyline_without_arrows(
                    &verts,
                    &hd_stroke,
                    false,
                    self.state.canvas_color,
                );
            }

            StrokeEndType::Stroke => {
                let stroke_thickness = thickness * stroke_end.length_factor.abs();
                let verts = [(x + r * nx, y + r * ny), (x - r * nx, y - r * ny)];
                let mut st_stroke = arrow_stroke.clone();
                st_stroke.width = stroke_thickness;
                st_stroke.start_end = StrokeEnd::new(StrokeEndType::Cap);
                st_stroke.finish_end = StrokeEnd::new(StrokeEndType::Cap);
                self.paint_polyline_without_arrows(
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
    fn fill_polygon_aa(&mut self, vertices: &[(f64, f64)], color: Color, rule: WindingRule) {
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

        let rows = scanline::rasterize(&pixel_verts, self.state.clip.to_scanline_clip(), rule);

        for (y, spans) in &rows {
            for span in spans {
                self.blit_span(*y, span, color);
            }
        }
    }

    /// Blit a single AA span onto the target.
    fn blit_span(&mut self, y: i32, span: &scanline::Span, color: Color) {
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
                self.blend_with_coverage(x_start, y, color, opacity);
            }
            return;
        }

        // First pixel (partial coverage).
        if span.opacity_beg > 0 {
            self.blend_with_coverage(x_start, y, color, span.opacity_beg);
        }

        // Interior pixels — bulk scanline, no per-pixel clip/bounds checks.
        let ix1 = x_start + 1;
        let ix2 = x_end - 1;
        if ix1 < ix2 {
            if span.opacity_mid >= 0x1000 {
                self.fill_span_blended(y, ix1, ix2, color);
            } else if span.opacity_mid > 0 {
                // Uniform partial coverage: pre-compute alpha-adjusted color once.
                let alpha =
                    ((color.a() as i32 * span.opacity_mid + 0x800) >> 12).clamp(0, 255) as u8;
                let blended = Color::rgba(color.r(), color.g(), color.b(), alpha);
                self.fill_span_blended(y, ix1, ix2, blended);
            }
        }

        // Last pixel (partial coverage).
        if span_width > 1 && span.opacity_end > 0 {
            let x_last = x_end - 1;
            self.blend_with_coverage(x_last, y, color, span.opacity_end);
        }
    }

    /// Blend a sampled color with sub-pixel edge coverage (0..=0x1000).
    #[inline]
    fn blend_with_coverage(&mut self, x: i32, y: i32, color: Color, cov: i32) {
        if cov >= 0x1000 {
            self.blend_pixel(x, y, color);
        } else if cov > 0 {
            // C++ single-step: alpha = (color_alpha * opacity_12bit + 0x800) >> 12
            let alpha = ((color.a() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8;
            let blended = Color::rgba(color.r(), color.g(), color.b(), alpha);
            self.blend_pixel(x, y, blended);
        }
    }

    /// Same as `blend_pixel` but without clip/bounds checks.
    /// Caller must guarantee x,y are within both the clip rect and the target image.
    #[inline(always)]
    fn blend_pixel_unchecked(&mut self, x: i32, y: i32, color: Color) {
        let xu = x as u32;
        let yu = y as u32;
        if color.is_opaque() && self.state.alpha == 255 {
            let out = self.image().pixel_mut(xu, yu);
            out[0] = color.r();
            out[1] = color.g();
            out[2] = color.b();
            out[3] = 255;
        } else if self.state.canvas_color.is_opaque() {
            let combined_alpha = if self.state.alpha == 255 {
                color.a()
            } else {
                ((color.a() as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if combined_alpha == 0 {
                return;
            }
            let px = self.read_pixel(xu, yu);
            let existing = Color::rgba(px[0], px[1], px[2], px[3]);
            let result = existing.canvas_blend(color, self.state.canvas_color, combined_alpha);
            let out = self.image().pixel_mut(xu, yu);
            out[0] = result.r();
            out[1] = result.g();
            out[2] = result.b();
        } else {
            let ca = color.a() as u16;
            let ea = if self.state.alpha == 255 {
                ca
            } else {
                (ca * self.state.alpha as u16 + 128) >> 8
            };
            if ea == 0 {
                return;
            }
            if ea >= 255 {
                let out = self.image().pixel_mut(xu, yu);
                out[0] = color.r();
                out[1] = color.g();
                out[2] = color.b();
                out[3] = 255;
                return;
            }
            let bg = self.read_pixel(xu, yu);
            let alpha = ea as u32;
            let t = (255 - alpha) * 257;
            let r = ((bg[0] as u32 * t + 0x8073) >> 16)
                + ((color.r() as u32 * alpha * 257 + 0x8073) >> 16);
            let g = ((bg[1] as u32 * t + 0x8073) >> 16)
                + ((color.g() as u32 * alpha * 257 + 0x8073) >> 16);
            let b = ((bg[2] as u32 * t + 0x8073) >> 16)
                + ((color.b() as u32 * alpha * 257 + 0x8073) >> 16);
            let a =
                ((bg[3] as u32 * t + 0x8073) >> 16) + ((255u32 * alpha * 257 + 0x8073) >> 16);
            let out = self.image().pixel_mut(xu, yu);
            out[0] = r as u8;
            out[1] = g as u8;
            out[2] = b as u8;
            out[3] = a as u8;
        }
    }

    /// Same as `blend_with_coverage` but without clip/bounds checks.
    #[inline(always)]
    fn blend_with_coverage_unchecked(&mut self, x: i32, y: i32, color: Color, cov: i32) {
        if cov >= 0x1000 {
            self.blend_pixel_unchecked(x, y, color);
        } else if cov > 0 {
            let alpha = ((color.a() as i32 * cov + 0x800) >> 12).clamp(0, 255) as u8;
            let blended = Color::rgba(color.r(), color.g(), color.b(), alpha);
            self.blend_pixel_unchecked(x, y, blended);
        }
    }

    /// Fill a polygon with a texture using the scanline rasterizer.
    fn fill_polygon_aa_textured(
        &mut self,
        vertices: &[(f64, f64)],
        texture: &Texture,
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

        let rows = scanline::rasterize(&pixel_verts, self.state.clip.to_scanline_clip(), rule);

        // Pre-transform texture coordinates to pixel space.
        // Extract state values to avoid borrowing self through the loop.
        let px_texture = Self::build_pixel_texture(texture, &self.state);

        for (y, spans) in &rows {
            for span in spans {
                self.blit_span_textured(*y, span, &px_texture);
            }
        }
    }

    /// Convert a Texture's coordinates from local space to pixel space.
    fn build_pixel_texture<'t>(texture: &'t Texture, state: &PainterState) -> PixelTexture<'t> {
        match texture {
            Texture::SolidColor(c) => PixelTexture::Solid(*c),
            Texture::LinearGradient {
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
            Texture::RadialGradient {
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
            Texture::Image {
                image,
                extension,
                quality,
            } => PixelTexture::Image {
                image,
                extension: *extension,
                quality: *quality,
                inv_scale_x: 1.0 / state.scale_x,
                inv_scale_y: 1.0 / state.scale_y,
                offset_x: state.offset_x,
                offset_y: state.offset_y,
            },
            Texture::ImageColored {
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
    fn sample_pixel_texture(texture: &PixelTexture, px: f64, py: f64) -> Color {
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
                color_a.lerp(*color_b, t.clamp(0.0, 1.0))
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
                    return color_outer.lerp(*color_inner, 0.0);
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
                color_inner.lerp(*color_outer, factor as f64 / 255.0)
            }
            PixelTexture::Image {
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
                Color::rgba(
                    ((sampled.r() as u32 * color.r() as u32 + 128) >> 8) as u8,
                    ((sampled.g() as u32 * color.g() as u32 + 128) >> 8) as u8,
                    ((sampled.b() as u32 * color.b() as u32 + 128) >> 8) as u8,
                    ((sampled.a() as u32 * color.a() as u32 + 128) >> 8) as u8,
                )
            }
        }
    }

    /// Sample an image at local coordinates using the given extension and quality.
    fn sample_image_at(
        image: &Image,
        x: f64,
        y: f64,
        extension: super::texture::ImageExtension,
        quality: super::texture::ImageQuality,
    ) -> Color {
        match quality {
            super::texture::ImageQuality::Nearest => {
                interpolation::sample_nearest(image, x, y, extension)
            }
            _ => interpolation::sample_bilinear(image, x, y, extension),
        }
    }

    /// Blit a single textured AA span onto the target.
    fn blit_span_textured(&mut self, y: i32, span: &scanline::Span, texture: &PixelTexture) {
        let tw = self.target_width as i32;
        let th = self.target_height as i32;
        if y < 0 || y >= th {
            return;
        }

        let x_start = span.x_start.max(0);
        let x_end = span.x_end.min(tw);
        if x_start >= x_end {
            return;
        }

        let py = y as f64 + 0.5;
        for x in x_start..x_end {
            let opacity = if x == span.x_start && x_end - x_start > 1 {
                span.opacity_beg
            } else if x == x_end - 1 && x_end - x_start > 1 {
                span.opacity_end
            } else if x_end - x_start == 1 {
                span.opacity_beg
            } else {
                span.opacity_mid
            };
            if opacity == 0 {
                continue;
            }

            let color = Self::sample_pixel_texture(texture, x as f64 + 0.5, py);

            if opacity >= 0x1000 {
                self.blend_pixel_unchecked(x, y, color);
            } else {
                self.blend_with_coverage_unchecked(x, y, color, opacity);
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
        if r < 0.5 {
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
    ) -> interpolation::ScaleTransform24 {
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
        interpolation::ScaleTransform24 {
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
    ) -> interpolation::AreaSampleTransform {
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
        interpolation::AreaSampleTransform {
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

    fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
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

        if color.is_opaque() && self.state.alpha == 255 {
            // Fully opaque: direct write, no blending needed.
            let out = self.image().pixel_mut(x as u32, y as u32);
            out[0] = color.r();
            out[1] = color.g();
            out[2] = color.b();
            out[3] = 255;
        } else if self.state.canvas_color.is_opaque() {
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
                color.a()
            } else {
                ((color.a() as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if combined_alpha == 0 {
                return;
            }
            let px = self.read_pixel(x as u32, y as u32);
            let existing = Color::rgba(px[0], px[1], px[2], px[3]);
            let result = existing.canvas_blend(color, self.state.canvas_color, combined_alpha);
            let out = self.image().pixel_mut(x as u32, y as u32);
            out[0] = result.r();
            out[1] = result.g();
            out[2] = result.b();
            // out[3] unchanged — C++ HAVE_CVC never modifies destination alpha.
        } else {
            // Standard source-over alpha compositing when canvas color is
            // unknown (non-opaque). Avoids the additive artifacts that
            // canvas_blend produces with TRANSPARENT canvas.
            let ca = color.a() as u16;
            let ea = if self.state.alpha == 255 {
                ca
            } else {
                (ca * self.state.alpha as u16 + 128) >> 8
            };
            if ea == 0 {
                return;
            }
            let bg = self.read_pixel(x as u32, y as u32);
            if ea >= 255 {
                let out = self.image().pixel_mut(x as u32, y as u32);
                out[0] = color.r();
                out[1] = color.g();
                out[2] = color.b();
                out[3] = 255;
            } else {
                // C++ emPainter non-CVC blend: per-channel round(x/255) via
                // (x * 257 + 0x8073) >> 16.  Background uses t = (255-alpha)*257,
                // source uses hash[color][alpha] ≈ round(color*alpha/255).
                let alpha = ea as u32;
                let inv = 255 - alpha;
                let t = inv * 257; // background attenuation factor
                let r = ((bg[0] as u32 * t + 0x8073) >> 16)
                    + ((color.r() as u32 * alpha * 257 + 0x8073) >> 16);
                let g = ((bg[1] as u32 * t + 0x8073) >> 16)
                    + ((color.g() as u32 * alpha * 257 + 0x8073) >> 16);
                let b = ((bg[2] as u32 * t + 0x8073) >> 16)
                    + ((color.b() as u32 * alpha * 257 + 0x8073) >> 16);
                let a =
                    ((bg[3] as u32 * t + 0x8073) >> 16) + ((255u32 * alpha * 257 + 0x8073) >> 16);
                let out = self.image().pixel_mut(x as u32, y as u32);
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
    fn fill_span_blended(&mut self, y: i32, x1: i32, x2: i32, color: Color) {
        debug_assert!(x1 >= 0 && x2 <= self.target_width as i32);
        debug_assert!(y >= 0 && y < self.target_height as i32);
        debug_assert!(x1 < x2);

        let tw = self.target_width as usize;
        let row_base = y as usize * tw * 4;

        if color.is_opaque() && self.state.alpha == 255 {
            let pixel = [color.r(), color.g(), color.b(), 255u8];
            let data = self.image().data_mut();
            for col in x1..x2 {
                let off = row_base + col as usize * 4;
                data[off..off + 4].copy_from_slice(&pixel);
            }
        } else if self.state.canvas_color.is_opaque() {
            let combined_alpha = if self.state.alpha == 255 {
                color.a()
            } else {
                ((color.a() as u16 * self.state.alpha as u16 + 128) >> 8) as u8
            };
            if combined_alpha == 0 {
                return;
            }
            let cv = self.state.canvas_color;
            let a = combined_alpha as u32;
            let cr = (cv.r() as u32 * a + 127) / 255;
            let cg = (cv.g() as u32 * a + 127) / 255;
            let cb = (cv.b() as u32 * a + 127) / 255;
            let pm_r = ((color.r() as u32 * a * 257 + 0x8073) >> 16) as i32;
            let pm_g = ((color.g() as u32 * a * 257 + 0x8073) >> 16) as i32;
            let pm_b = ((color.b() as u32 * a * 257 + 0x8073) >> 16) as i32;
            let delta_r = pm_r - cr as i32;
            let delta_g = pm_g - cg as i32;
            let delta_b = pm_b - cb as i32;
            let data = self.image().data_mut();
            for col in x1..x2 {
                let off = row_base + col as usize * 4;
                data[off] = (data[off] as i32 + delta_r).clamp(0, 255) as u8;
                data[off + 1] = (data[off + 1] as i32 + delta_g).clamp(0, 255) as u8;
                data[off + 2] = (data[off + 2] as i32 + delta_b).clamp(0, 255) as u8;
            }
        } else {
            let ca = color.a() as u16;
            let ea = if self.state.alpha == 255 {
                ca
            } else {
                (ca * self.state.alpha as u16 + 128) >> 8
            };
            if ea == 0 {
                return;
            }
            if ea >= 255 {
                let pixel = [color.r(), color.g(), color.b(), 255u8];
                let data = self.image().data_mut();
                for col in x1..x2 {
                    let off = row_base + col as usize * 4;
                    data[off..off + 4].copy_from_slice(&pixel);
                }
            } else {
                let alpha = ea as u32;
                let t = (255 - alpha) * 257;
                let sr = (color.r() as u32 * alpha * 257 + 0x8073) >> 16;
                let sg = (color.g() as u32 * alpha * 257 + 0x8073) >> 16;
                let sb = (color.b() as u32 * alpha * 257 + 0x8073) >> 16;
                let sa = (255u32 * alpha * 257 + 0x8073) >> 16;
                let data = self.image().data_mut();
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

    fn fill_rect_pixels(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
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
        if color.is_opaque() && self.state.alpha == 255 {
            let pixel = [color.r(), color.g(), color.b(), 255u8];
            let tw = self.target_width as usize;
            let data = self.image().data_mut();
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
            self.fill_span_blended(row, start_x, end_x, color);
        }
    }

    fn draw_line_pixels(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Color) {
        // Bresenham's line algorithm
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.blend_pixel(x0, y0, color);
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
fn expand_tabs(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut col = 0usize;
    for ch in line.chars() {
        if ch == '\t' {
            let next_tab = (col / 8 + 1) * 8;
            for _ in col..next_tab {
                result.push(' ');
            }
            col = next_tab;
        } else {
            result.push(ch);
            col += 1;
        }
    }
    result
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
    use crate::foundation::Image;

    fn make_painter<'a>(target: &'a mut Image) -> Painter<'a> {
        Painter::new(target)
    }

    #[test]
    fn edge_correction_no_crash() {
        let mut img = Image::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_polygon(
            &[(0.0, 0.0), (16.0, 0.0), (16.0, 16.0)],
            Color::RED,
            Color::TRANSPARENT,
        );
        p.paint_polygon(
            &[(0.0, 0.0), (16.0, 16.0), (0.0, 16.0)],
            Color::BLUE,
            Color::TRANSPARENT,
        );
        p.paint_edge_correction(0.0, 0.0, 16.0, 16.0, Color::RED, Color::BLUE);
    }

    #[test]
    fn edge_correction_transparent_noop() {
        let mut img = Image::new(16, 16, 4);
        let mut p = make_painter(&mut img);
        p.paint_edge_correction(0.0, 0.0, 10.0, 10.0, Color::TRANSPARENT, Color::RED);
        p.paint_edge_correction(0.0, 0.0, 10.0, 10.0, Color::RED, Color::TRANSPARENT);
    }

    #[test]
    fn bezier_outline_paints_pixels() {
        let mut img = Image::new(64, 64, 4);
        let mut p = make_painter(&mut img);
        let stroke = Stroke::new(Color::WHITE, 2.0);
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
        p.paint_bezier_outline(&points, &stroke, Color::TRANSPARENT);
        let px = img.pixel(32, 10);
        assert!(px[0] > 0 || px[1] > 0 || px[2] > 0);
    }

    #[test]
    fn line_radius_miter_no_arrow() {
        let stroke = Stroke::new(Color::BLACK, 4.0);
        let butt = StrokeEnd::butt();
        let r = Painter::calculate_line_point_min_max_radius(4.0, &stroke, &butt, &butt);
        assert!((r - 10.0).abs() < 0.01, "miter: expected 10.0, got {r}");
    }

    #[test]
    fn line_radius_round_no_arrow() {
        let stroke = Stroke {
            join: super::super::stroke::LineJoin::Round,
            ..Stroke::new(Color::BLACK, 4.0)
        };
        let butt = StrokeEnd::butt();
        let r = Painter::calculate_line_point_min_max_radius(4.0, &stroke, &butt, &butt);
        assert!((r - 2.0).abs() < 0.01, "round: expected 2.0, got {r}");
    }

    #[test]
    fn line_radius_with_arrow() {
        let stroke = Stroke {
            join: super::super::stroke::LineJoin::Round,
            ..Stroke::new(Color::BLACK, 4.0)
        };
        let butt = StrokeEnd::butt();
        let arrow = StrokeEnd::new(StrokeEndType::Arrow);
        let r = Painter::calculate_line_point_min_max_radius(4.0, &stroke, &arrow, &butt);
        let expected = (20.0f64 * 20.0 + 40.0 * 40.0).sqrt();
        assert!(
            (r - expected).abs() < 0.1,
            "arrow: expected {expected}, got {r}"
        );
    }

    #[test]
    fn polyline_without_arrows_solid() {
        let mut img = Image::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        let stroke = Stroke::new(Color::WHITE, 2.0);
        let verts = [(5.0, 5.0), (25.0, 5.0), (25.0, 25.0)];
        p.paint_polyline_without_arrows(&verts, &stroke, false, Color::TRANSPARENT);
        let px = img.pixel(15, 5);
        assert!(px[0] > 0, "solid polyline should paint pixels");
    }

    #[test]
    fn paint_image_scaled_bilinear() {
        let mut src = Image::new(4, 4, 4);
        for y in 0..4u32 {
            for x in 0..4u32 {
                let v = ((x + y) * 32) as u8;
                let p = src.pixel_mut(x, y);
                p[0] = v;
                p[1] = v;
                p[2] = v;
                p[3] = 255;
            }
        }
        let mut img = Image::new(16, 16, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            16.0,
            16.0,
            &src,
            super::super::texture::ImageQuality::Bilinear,
            super::super::texture::ImageExtension::Clamp,
        );
        // Center pixel should be interpolated (non-zero).
        let px = img.pixel(8, 8);
        assert!(px[0] > 0, "scaled image should paint pixels");
    }

    #[test]
    fn paint_radial_gradient_fills() {
        let mut img = Image::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_radial_gradient(
            16.0,
            16.0,
            12.0,
            12.0,
            Color::WHITE,
            Color::BLACK,
            Color::TRANSPARENT,
        );
        let center = img.pixel(16, 16);
        assert!(center[0] > 200, "center should be near white");
    }

    fn make_gradient_src() -> Image {
        let mut src = Image::new(8, 8, 4);
        for y in 0..8u32 {
            for x in 0..8u32 {
                let v = ((x + y) * 16).min(255) as u8;
                let p = src.pixel_mut(x, y);
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
        let mut img = Image::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            32.0,
            32.0,
            &src,
            super::super::texture::ImageQuality::Bicubic,
            super::super::texture::ImageExtension::Clamp,
        );
        let px = img.pixel(16, 16);
        assert!(px[0] > 0, "bicubic: center pixel should be non-zero");
    }

    #[test]
    fn paint_image_scaled_lanczos() {
        let src = make_gradient_src();
        let mut img = Image::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            32.0,
            32.0,
            &src,
            super::super::texture::ImageQuality::Lanczos,
            super::super::texture::ImageExtension::Clamp,
        );
        let px = img.pixel(16, 16);
        assert!(px[0] > 0, "lanczos: center pixel should be non-zero");
    }

    #[test]
    fn paint_image_scaled_adaptive() {
        let src = make_gradient_src();
        let mut img = Image::new(32, 32, 4);
        let mut p = make_painter(&mut img);
        p.paint_image_scaled(
            0.0,
            0.0,
            32.0,
            32.0,
            &src,
            super::super::texture::ImageQuality::Adaptive,
            super::super::texture::ImageExtension::Clamp,
        );
        let px = img.pixel(16, 16);
        assert!(px[0] > 0, "adaptive: center pixel should be non-zero");
    }

    #[test]
    fn paint_image_scaled_area_sampled() {
        let src = make_gradient_src();
        let mut img = Image::new(4, 4, 4);
        let mut p = make_painter(&mut img);
        // Downscale: 8x8 -> 4x4 (area sampling)
        p.paint_image_scaled(
            0.0,
            0.0,
            4.0,
            4.0,
            &src,
            super::super::texture::ImageQuality::AreaSampled,
            super::super::texture::ImageExtension::Clamp,
        );
        let px = img.pixel(2, 2);
        assert!(px[0] > 0, "area-sampled: center pixel should be non-zero");
    }

    /// Verify that recording + replay produces byte-identical output to
    /// direct rendering. This is the foundation for multi-threaded rendering
    /// correctness: if replay matches direct, then parallel replay also matches.
    #[test]
    fn draw_list_replay_matches_direct() {
        use super::super::draw_list::DrawList;
        use super::super::thread_pool::RenderThreadPool;

        let w = 64u32;
        let h = 64u32;

        // --- Direct rendering (single-threaded, no recording) ---
        let mut direct_img = Image::new(w, h, 4);
        direct_img.fill(crate::foundation::Color::BLACK);
        {
            let mut p = Painter::new(&mut direct_img);
            draw_test_scene(&mut p);
        }

        // --- Recording + single-tile replay ---
        let mut draw_list = DrawList::new();
        {
            let mut rec = Painter::new_recording(w, h, draw_list.ops_mut());
            draw_test_scene(&mut rec);
        }
        let mut replay_img = Image::new(w, h, 4);
        replay_img.fill(crate::foundation::Color::BLACK);
        {
            let mut p = Painter::new(&mut replay_img);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        assert_eq!(
            direct_img.data(),
            replay_img.data(),
            "recording + replay must produce byte-identical output to direct rendering"
        );

        // --- Multi-threaded replay (thread counts 1, 2, 4) ---
        for thread_count in [1, 2, 4] {
            let pool = RenderThreadPool::new(thread_count);
            // Split into 4 tiles of 32x32
            let tile_size = 32u32;
            let cols = w / tile_size;
            let rows = h / tile_size;
            let tiles: Vec<(u32, u32)> = (0..rows)
                .flat_map(|r| (0..cols).map(move |c| (c, r)))
                .collect();
            let results: Vec<std::sync::Mutex<Option<Image>>> = tiles
                .iter()
                .map(|_| std::sync::Mutex::new(None::<Image>))
                .collect();
            let results_ref = &results;
            let tiles_ref = &tiles;
            let draw_list_ref = &draw_list;
            let ts = tile_size as f64;

            pool.call_parallel(
                |idx| {
                    let (col, row) = tiles_ref[idx];
                    let mut buf = Image::new(tile_size, tile_size, 4);
                    buf.fill(crate::foundation::Color::BLACK);
                    {
                        let mut p = Painter::new(&mut buf);
                        draw_list_ref.replay(&mut p, (col as f64 * ts, row as f64 * ts));
                    }
                    *results_ref[idx].lock().unwrap() = Some(buf);
                },
                tiles.len(),
            );

            // Reconstruct the full image from tiles
            let mut composed = Image::new(w, h, 4);
            composed.fill(crate::foundation::Color::BLACK);
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
                direct_img.data(),
                composed.data(),
                "parallel replay with {} threads must match direct rendering",
                thread_count,
            );
        }
    }

    /// Draw a test scene with various primitives for recording/replay testing.
    fn draw_test_scene(p: &mut Painter) {
        let red = Color::rgba(255, 0, 0, 255);
        let green = Color::rgba(0, 255, 0, 200);
        let blue = Color::rgba(0, 0, 255, 180);
        let white = Color::rgba(255, 255, 255, 255);
        let canvas = Color::rgba(50, 50, 50, 255);

        // Background
        p.paint_rect(0.0, 0.0, 64.0, 64.0, canvas, Color::BLACK);

        // Overlapping rectangles with transparency
        p.push_state();
        p.set_canvas_color(canvas);
        p.paint_rect(5.0, 5.0, 30.0, 30.0, red, canvas);
        p.paint_rect(15.0, 15.0, 30.0, 30.0, green, canvas);

        // Ellipse
        p.paint_ellipse(48.0, 16.0, 12.0, 12.0, blue, canvas);

        // Text
        p.paint_text(2.0, 50.0, "Hi", 10.0, 1.0, white, canvas);

        // Polygon
        let verts = [(10.0, 40.0), (30.0, 35.0), (25.0, 55.0)];
        p.paint_polygon(&verts, blue, canvas);

        p.pop_state();
    }
}
