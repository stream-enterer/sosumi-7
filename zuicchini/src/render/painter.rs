use super::font_cache::{FontCache, GlyphCacheKey};
use super::interpolation;
use super::scanline::{self, WindingRule};
use super::stroke::{Stroke, StrokeEnd, StrokeEndType};
use crate::foundation::{Color, Fixed12, Image, PixelRect};

/// Base multiplier for decoration size.
const ARROW_BASE_SIZE: f64 = 10.0;
/// Notch depth ratio for Arrow type.
const ARROW_NOTCH: f64 = 0.3;
/// Number of segments for circle approximation.
const CIRCLE_SEGMENTS: usize = 32;
/// Bezier subdivision flatness threshold (pixels).
const BEZIER_FLATNESS: f64 = 0.5;
/// Maximum Bezier subdivision depth.
const BEZIER_MAX_DEPTH: u8 = 10;

/// Text alignment for boxed text rendering.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
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
    /// Clip rectangle in pixel coordinates.
    clip: PixelRect,
    /// Canvas color for canvas_blend operations.
    canvas_color: Color,
    /// Global alpha multiplier (0–255).
    alpha: u8,
}

/// CPU software rasterizer that paints into an Image buffer.
pub struct Painter<'a> {
    target: &'a mut Image,
    state: PainterState,
    state_stack: Vec<PainterState>,
    font_cache: &'a mut FontCache,
}

impl<'a> Painter<'a> {
    /// Create a new painter targeting the given RGBA image.
    ///
    /// # Panics
    /// Panics if the image is not 4-channel RGBA.
    pub fn new(target: &'a mut Image, font_cache: &'a mut FontCache) -> Self {
        assert_eq!(
            target.channel_count(),
            4,
            "Painter requires a 4-channel RGBA image"
        );
        let w = target.width() as i32;
        let h = target.height() as i32;
        Self {
            target,
            state: PainterState {
                offset_x: 0.0,
                offset_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                clip: PixelRect { x: 0, y: 0, w, h },
                canvas_color: Color::BLACK,
                alpha: 255,
            },
            state_stack: Vec::new(),
            font_cache,
        }
    }

    /// Push the current state onto the stack.
    pub fn push_state(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    /// Pop and restore the previous state.
    ///
    /// # Panics
    /// Panics if the state stack is empty.
    pub fn pop_state(&mut self) {
        self.state = self.state_stack.pop().expect("State stack underflow");
    }

    /// Set the canvas color used for canvas_blend operations.
    pub fn set_canvas_color(&mut self, color: Color) {
        self.state.canvas_color = color;
    }

    /// Set the global alpha multiplier.
    pub fn set_alpha(&mut self, alpha: u8) {
        self.state.alpha = alpha;
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
    pub fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;

        let PixelRect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.state.clip;
        // Intersect
        let nx = px.max(cx);
        let ny = py.max(cy);
        let nx2 = (px + pw).min(cx + cw);
        let ny2 = (py + ph).min(cy + ch);
        self.state.clip = PixelRect {
            x: nx,
            y: ny,
            w: (nx2 - nx).max(0),
            h: (ny2 - ny).max(0),
        };
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

    /// Get the current clip rectangle.
    pub fn clip(&self) -> PixelRect {
        self.state.clip
    }

    /// Immutable access to the font cache (for measurement).
    pub fn font_cache(&self) -> &FontCache {
        self.font_cache
    }

    // --- Drawing API ---

    /// Fill a rectangle with a solid color.
    pub fn paint_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;
        self.fill_rect_pixels(px, py, pw, ph, color);
    }

    /// Fill an ellipse with a solid color using AA polygon approximation.
    pub fn paint_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, color: Color) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let verts = Self::ellipse_polygon(cx, cy, rx, ry);
        self.fill_polygon_aa(&verts, color, WindingRule::NonZero);
    }

    /// Fill an ellipse sector (pie slice) defined by center, radii, and angle range.
    /// Angles are in radians, measured counter-clockwise from the +X axis.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_ellipse_sector(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        end_angle: f64,
        color: Color,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let sweep = end_angle - start_angle;
        // Normalize negative sweep.
        if sweep < 0.0 {
            return self.paint_ellipse_sector(cx, cy, rx, ry, end_angle, start_angle, color);
        }
        // Full circle or more — delegate to paint_ellipse.
        if sweep >= 2.0 * std::f64::consts::PI {
            return self.paint_ellipse(cx, cy, rx, ry, color);
        }
        let segments = adaptive_circle_segments(rx.max(ry));
        // Scale segments proportional to sweep.
        let arc_segments =
            ((segments as f64 * sweep / (2.0 * std::f64::consts::PI)).ceil() as usize).max(2);
        let mut verts = Vec::with_capacity(arc_segments + 2);
        verts.push((cx, cy));
        for i in 0..=arc_segments {
            let t = i as f64 / arc_segments as f64;
            let angle = start_angle + t * sweep;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        self.paint_polygon(&verts, color);
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
    ) {
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = px.max(clip_x).max(0);
        let start_y = py.max(clip_y).max(0);
        let end_x = (px + pw)
            .min(clip_x + clip_w)
            .min(self.target.width() as i32);
        let end_y = (py + ph)
            .min(clip_y + clip_h)
            .min(self.target.height() as i32);

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
                    (col as f64, row as f64),
                );
                self.blend_pixel(col, row, color);
            }
        }
    }

    /// Fill an elliptical region with a radial gradient.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_radial_gradient(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        color_inner: Color,
        color_outer: Color,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }

        let pcx = self.to_pixel_x(cx);
        let pcy = self.to_pixel_y(cy);
        let prx = (rx * self.state.scale_x) as i32;
        let pry = (ry * self.state.scale_y) as i32;

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = (pcx - prx).max(clip_x).max(0);
        let start_y = (pcy - pry).max(clip_y).max(0);
        let end_x = (pcx + prx)
            .min(clip_x + clip_w)
            .min(self.target.width() as i32);
        let end_y = (pcy + pry)
            .min(clip_y + clip_h)
            .min(self.target.height() as i32);

        for row in start_y..end_y {
            for col in start_x..end_x {
                let color = interpolation::sample_radial_gradient(
                    pcx as f64,
                    pcy as f64,
                    prx as f64,
                    pry as f64,
                    color_inner,
                    color_outer,
                    (col as f64, row as f64),
                );
                self.blend_pixel(col, row, color);
            }
        }
    }

    /// Draw a line between two points.
    pub fn paint_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, color: Color) {
        let px0 = self.to_pixel_x(x0);
        let py0 = self.to_pixel_y(y0);
        let px1 = self.to_pixel_x(x1);
        let py1 = self.to_pixel_y(y1);
        self.draw_line_pixels(px0, py0, px1, py1, color);
    }

    /// Fill a polygon defined by a list of (x, y) vertices.
    /// Uses anti-aliased scanline rasterization with NonZero winding rule.
    pub fn paint_polygon(&mut self, vertices: &[(f64, f64)], color: Color) {
        self.fill_polygon_aa(vertices, color, WindingRule::NonZero);
    }

    /// Fill a polygon using even-odd winding rule (for polygon rings with holes).
    pub fn paint_polygon_even_odd(&mut self, vertices: &[(f64, f64)], color: Color) {
        self.fill_polygon_aa(vertices, color, WindingRule::EvenOdd);
    }

    /// Draw a polygon outline by stroking each edge as a thick line.
    pub fn paint_polygon_outlined(
        &mut self,
        vertices: &[(f64, f64)],
        stroke_color: Color,
        thickness: f64,
    ) {
        let n = vertices.len();
        if n < 2 {
            return;
        }
        for i in 0..n {
            let (x0, y0) = vertices[i];
            let (x1, y1) = vertices[(i + 1) % n];
            self.paint_thick_line(x0, y0, x1, y1, thickness, stroke_color);
        }
    }

    /// Draw a polyline (open path) outline by stroking each segment.
    pub fn paint_polyline(&mut self, vertices: &[(f64, f64)], stroke_color: Color, thickness: f64) {
        if vertices.len() < 2 {
            return;
        }
        for i in 0..vertices.len() - 1 {
            let (x0, y0) = vertices[i];
            let (x1, y1) = vertices[i + 1];
            self.paint_thick_line(x0, y0, x1, y1, thickness, stroke_color);
        }
    }

    /// Draw a thick line as a filled polygon.
    fn paint_thick_line(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        thickness: f64,
        color: Color,
    ) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }
        let half_w = thickness / 2.0;
        let nx = -dy / len * half_w;
        let ny = dx / len * half_w;
        self.paint_polygon(
            &[
                (x0 + nx, y0 + ny),
                (x1 + nx, y1 + ny),
                (x1 - nx, y1 - ny),
                (x0 - nx, y0 - ny),
            ],
            color,
        );
    }

    /// Fill a rounded rectangle using AA polygon approximation.
    pub fn paint_round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let verts = Self::round_rect_polygon(x, y, w, h, radius);
        self.fill_polygon_aa(&verts, color, WindingRule::NonZero);
    }

    /// Draw a source image at the given position.
    pub fn paint_image(&mut self, x: f64, y: f64, image: &Image) {
        if image.channel_count() != 4 {
            return;
        }

        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let iw = image.width() as i32;
        let ih = image.height() as i32;

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = px.max(clip_x);
        let start_y = py.max(clip_y);
        let end_x = (px + iw).min(clip_x + clip_w);
        let end_y = (py + ih).min(clip_y + clip_h);

        for row in start_y..end_y {
            for col in start_x..end_x {
                let ix = (col - px) as u32;
                let iy = (row - py) as u32;
                let src = image.pixel(ix, iy);
                let src_color = Color::rgba(src[0], src[1], src[2], src[3]);
                self.blend_pixel(col, row, src_color);
            }
        }
    }

    /// Blit a 1-channel greyscale alpha mask using the given color.
    /// Source region is (src_x, src_y, src_w, src_h) within the image.
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
        color: Color,
    ) {
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = px.max(clip_x).max(0);
        let start_y = py.max(clip_y).max(0);
        let end_x = (px + pw)
            .min(clip_x + clip_w)
            .min(self.target.width() as i32);
        let end_y = (py + ph)
            .min(clip_y + clip_h)
            .min(self.target.height() as i32);

        if pw <= 0 || ph <= 0 || src_w == 0 || src_h == 0 {
            return;
        }

        for row in start_y..end_y {
            for col in start_x..end_x {
                // Map dest pixel to source coords (nearest neighbor)
                let sx = src_x + ((col - px) as u32 * src_w / pw as u32).min(src_w - 1);
                let sy = src_y + ((row - py) as u32 * src_h / ph as u32).min(src_h - 1);
                let g = image.pixel(sx, sy)[0];
                if g == 0 {
                    continue;
                }
                let blended = Color::rgba(
                    color.r(),
                    color.g(),
                    color.b(),
                    (color.a() as u16 * g as u16 / 255) as u8,
                );
                self.blend_pixel(col, row, blended);
            }
        }
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

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = px.max(clip_x).max(0);
        let start_y = py.max(clip_y).max(0);
        let end_x = (px + pw)
            .min(clip_x + clip_w)
            .min(self.target.width() as i32);
        let end_y = (py + ph)
            .min(clip_y + clip_h)
            .min(self.target.height() as i32);

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
                self.blend_pixel(col, row, color);
            }
        }
    }

    /// Draw an image with two-color mapping based on luminance.
    /// Pixel luminance maps linearly between `color1` (dark) and `color2` (bright).
    #[allow(clippy::too_many_arguments)]
    pub fn paint_image_colored_2(
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
    ) {
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = px.max(clip_x).max(0);
        let start_y = py.max(clip_y).max(0);
        let end_x = (px + pw)
            .min(clip_x + clip_w)
            .min(self.target.width() as i32);
        let end_y = (py + ph)
            .min(clip_y + clip_h)
            .min(self.target.height() as i32);

        if pw <= 0 || ph <= 0 || src_w == 0 || src_h == 0 {
            return;
        }

        let ch = image.channel_count();

        for row in start_y..end_y {
            for col in start_x..end_x {
                let sx = src_x + ((col - px) as u32 * src_w / pw as u32).min(src_w - 1);
                let sy = src_y + ((row - py) as u32 * src_h / ph as u32).min(src_h - 1);
                let p = image.pixel(sx, sy);
                let lum = if ch == 1 {
                    p[0]
                } else {
                    // ITU-R BT.601 luminance.
                    ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8
                };
                let t = lum as f64 / 255.0;
                let blended = color1.lerp(color2, t);
                self.blend_pixel(col, row, blended);
            }
        }
    }

    // --- Bezier curves ---

    /// Fill a cubic Bezier curve region (tessellated to polygon).
    /// `points` should contain 4*N control points (groups of [P0, P1, P2, P3]).
    pub fn paint_bezier(&mut self, points: &[(f64, f64)], color: Color) {
        if points.len() < 4 {
            return;
        }
        let mut verts = Vec::new();
        for chunk in points.chunks(4) {
            if chunk.len() == 4 {
                tessellate_cubic(
                    &mut verts,
                    chunk[0],
                    chunk[1],
                    chunk[2],
                    chunk[3],
                    BEZIER_FLATNESS,
                    0,
                );
            }
        }
        if verts.len() >= 3 {
            self.fill_polygon_aa(&verts, color, WindingRule::NonZero);
        }
    }

    /// Stroke a cubic Bezier curve (tessellated to polyline).
    pub fn paint_bezier_line(&mut self, points: &[(f64, f64)], stroke: &Stroke) {
        if points.len() < 4 {
            return;
        }
        let mut verts = Vec::new();
        for chunk in points.chunks(4) {
            if chunk.len() == 4 {
                tessellate_cubic(
                    &mut verts,
                    chunk[0],
                    chunk[1],
                    chunk[2],
                    chunk[3],
                    BEZIER_FLATNESS,
                    0,
                );
            }
        }
        if verts.len() >= 2 {
            self.paint_solid_polyline(&verts, stroke, false);
        }
    }

    // --- Formatted text ---

    /// Draw text inside a bounding box with alignment and line wrapping.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_text_boxed(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        text: &str,
        size_px: f64,
        color: Color,
        alignment: TextAlignment,
    ) {
        let quantized = FontCache::quantize_size(size_px * self.state.scale_y.abs());
        if quantized < 2 {
            return;
        }

        let line_height = self.font_cache.line_height(0, quantized);

        let mut cursor_y = y;
        for line in text.split('\n') {
            if cursor_y + line_height > y + h {
                break;
            }
            // Expand tabs.
            let expanded = expand_tabs(line);
            let (tw, _th) = self.font_cache.measure_text(&expanded, 0, quantized);
            let line_x = match alignment {
                TextAlignment::Left => x,
                TextAlignment::Center => x + (w - tw) / 2.0,
                TextAlignment::Right => x + w - tw,
            };
            self.paint_text(line_x, cursor_y, &expanded, size_px, color);
            cursor_y += line_height;
        }
    }

    // --- 9-slice border images ---

    /// Draw a 9-slice border image stretched to fill a rectangle.
    /// `border_insets` are (left, top, right, bottom) in source pixel units.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_border_image(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &Image,
        border_insets: (f64, f64, f64, f64),
    ) {
        let (bl, bt, br, bb) = border_insets;
        let iw = image.width() as f64;
        let ih = image.height() as f64;
        let quality = super::texture::ImageQuality::Bilinear;
        let ext = super::texture::ImageExtension::Clamp;

        // Clamp border insets to destination size.
        let bl = bl.min(w / 2.0);
        let br = br.min(w / 2.0);
        let bt = bt.min(h / 2.0);
        let bb = bb.min(h / 2.0);

        // Source regions (in source pixel space).
        let src_cx = iw - bl - br; // center width in source
        let src_cy = ih - bt - bb; // center height in source

        // Destination regions.
        let dst_cx = w - bl - br;
        let dst_cy = h - bt - bb;

        // Helper to draw a sub-region. We create sub-images from the source.
        // For simplicity, we draw each slice using paint_image_scaled.

        // Corners (no scaling needed if borders match).
        self.paint_9slice_section(x, y, bl, bt, image, 0.0, 0.0, bl, bt, quality, ext);
        self.paint_9slice_section(
            x + w - br,
            y,
            br,
            bt,
            image,
            iw - br,
            0.0,
            br,
            bt,
            quality,
            ext,
        );
        self.paint_9slice_section(
            x,
            y + h - bb,
            bl,
            bb,
            image,
            0.0,
            ih - bb,
            bl,
            bb,
            quality,
            ext,
        );
        self.paint_9slice_section(
            x + w - br,
            y + h - bb,
            br,
            bb,
            image,
            iw - br,
            ih - bb,
            br,
            bb,
            quality,
            ext,
        );

        // Edges.
        if dst_cx > 0.0 {
            self.paint_9slice_section(
                x + bl,
                y,
                dst_cx,
                bt,
                image,
                bl,
                0.0,
                src_cx,
                bt,
                quality,
                ext,
            );
            self.paint_9slice_section(
                x + bl,
                y + h - bb,
                dst_cx,
                bb,
                image,
                bl,
                ih - bb,
                src_cx,
                bb,
                quality,
                ext,
            );
        }
        if dst_cy > 0.0 {
            self.paint_9slice_section(
                x,
                y + bt,
                bl,
                dst_cy,
                image,
                0.0,
                bt,
                bl,
                src_cy,
                quality,
                ext,
            );
            self.paint_9slice_section(
                x + w - br,
                y + bt,
                br,
                dst_cy,
                image,
                iw - br,
                bt,
                br,
                src_cy,
                quality,
                ext,
            );
        }

        // Center.
        if dst_cx > 0.0 && dst_cy > 0.0 {
            self.paint_9slice_section(
                x + bl,
                y + bt,
                dst_cx,
                dst_cy,
                image,
                bl,
                bt,
                src_cx,
                src_cy,
                quality,
                ext,
            );
        }
    }

    /// Draw a 9-slice border image with two-color tinting.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_border_image_colored(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image: &Image,
        border_insets: (f64, f64, f64, f64),
        color1: Color,
        color2: Color,
    ) {
        // Draw the 9-slice first, then apply two-color mapping.
        // For simplicity, draw directly with the two-color method per section.
        let (bl, bt, br, bb) = border_insets;
        let iw = image.width() as f64;
        let ih = image.height() as f64;

        let bl = bl.min(w / 2.0);
        let br = br.min(w / 2.0);
        let bt = bt.min(h / 2.0);
        let bb = bb.min(h / 2.0);

        let src_cx = iw - bl - br;
        let src_cy = ih - bt - bb;
        let dst_cx = w - bl - br;
        let dst_cy = h - bt - bb;

        // Corners.
        self.paint_image_colored_2(
            x, y, bl, bt, image, 0, 0, bl as u32, bt as u32, color1, color2,
        );
        self.paint_image_colored_2(
            x + w - br,
            y,
            br,
            bt,
            image,
            (iw - br) as u32,
            0,
            br as u32,
            bt as u32,
            color1,
            color2,
        );
        self.paint_image_colored_2(
            x,
            y + h - bb,
            bl,
            bb,
            image,
            0,
            (ih - bb) as u32,
            bl as u32,
            bb as u32,
            color1,
            color2,
        );
        self.paint_image_colored_2(
            x + w - br,
            y + h - bb,
            br,
            bb,
            image,
            (iw - br) as u32,
            (ih - bb) as u32,
            br as u32,
            bb as u32,
            color1,
            color2,
        );

        // Edges.
        if dst_cx > 0.0 {
            self.paint_image_colored_2(
                x + bl,
                y,
                dst_cx,
                bt,
                image,
                bl as u32,
                0,
                src_cx as u32,
                bt as u32,
                color1,
                color2,
            );
            self.paint_image_colored_2(
                x + bl,
                y + h - bb,
                dst_cx,
                bb,
                image,
                bl as u32,
                (ih - bb) as u32,
                src_cx as u32,
                bb as u32,
                color1,
                color2,
            );
        }
        if dst_cy > 0.0 {
            self.paint_image_colored_2(
                x,
                y + bt,
                bl,
                dst_cy,
                image,
                0,
                bt as u32,
                bl as u32,
                src_cy as u32,
                color1,
                color2,
            );
            self.paint_image_colored_2(
                x + w - br,
                y + bt,
                br,
                dst_cy,
                image,
                (iw - br) as u32,
                bt as u32,
                br as u32,
                src_cy as u32,
                color1,
                color2,
            );
        }

        // Center.
        if dst_cx > 0.0 && dst_cy > 0.0 {
            self.paint_image_colored_2(
                x + bl,
                y + bt,
                dst_cx,
                dst_cy,
                image,
                bl as u32,
                bt as u32,
                src_cx as u32,
                src_cy as u32,
                color1,
                color2,
            );
        }
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
        quality: super::texture::ImageQuality,
        extension: super::texture::ImageExtension,
    ) {
        if dw <= 0.0 || dh <= 0.0 || sw <= 0.0 || sh <= 0.0 {
            return;
        }

        let px = self.to_pixel_x(dx);
        let py = self.to_pixel_y(dy);
        let pw = (dw * self.state.scale_x) as i32;
        let ph = (dh * self.state.scale_y) as i32;
        if pw <= 0 || ph <= 0 {
            return;
        }

        let PixelRect {
            x: clip_x,
            y: clip_y,
            w: clip_w,
            h: clip_h,
        } = self.state.clip;
        let start_x = px.max(clip_x).max(0);
        let start_y = py.max(clip_y).max(0);
        let end_x = (px + pw)
            .min(clip_x + clip_w)
            .min(self.target.width() as i32);
        let end_y = (py + ph)
            .min(clip_y + clip_h)
            .min(self.target.height() as i32);

        let interp_quality = match quality {
            super::texture::ImageQuality::Nearest => interpolation::InterpolationQuality::Nearest,
            _ => interpolation::InterpolationQuality::Bilinear,
        };
        let ctx = interpolation::ScaleContext {
            src_w: sw,
            src_h: sh,
            dest_w: pw as f64,
            dest_h: ph as f64,
        };

        for row in start_y..end_y {
            for col in start_x..end_x {
                let src_x = sx + (col - px) as f64 * sw / pw as f64;
                let src_y = sy + (row - py) as f64 * sh / ph as f64;
                let color =
                    interpolation::sample(image, src_x, src_y, interp_quality, extension, &ctx);
                self.blend_pixel(col, row, color);
            }
        }
    }

    // --- Ellipse/sector outline utilities ---

    /// Draw an ellipse sector outline.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_ellipse_sector_outlined(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        end_angle: f64,
        stroke: &Stroke,
    ) {
        if rx <= 0.0 || ry <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        let sweep = end_angle - start_angle;
        if sweep.abs() < 1e-10 {
            return;
        }
        let segments = adaptive_circle_segments(rx.max(ry));
        let arc_segs =
            ((segments as f64 * sweep.abs() / (2.0 * std::f64::consts::PI)).ceil() as usize).max(2);
        let mut verts = Vec::with_capacity(arc_segs + 2);
        verts.push((cx, cy));
        for i in 0..=arc_segs {
            let t = i as f64 / arc_segs as f64;
            let angle = start_angle + t * sweep;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        self.paint_polygon_outlined(&verts, stroke.color, stroke.width);
    }

    /// Draw text at the given position using the font system.
    /// `size_px` is the text size in user coordinates.
    pub fn paint_text(&mut self, x: f64, y: f64, text: &str, size_px: f64, color: Color) {
        let quantized = FontCache::quantize_size(size_px * self.state.scale_y.abs());
        if quantized < 2 {
            // Too small — draw a solid rectangle as placeholder.
            let (tw, th) = self.font_cache.measure_text(text, 0, 2);
            let scale = size_px / 2.0;
            self.paint_rect(x, y, tw * scale, th * scale, color);
            return;
        }

        // Phase 1: shape and ensure all glyphs are cached (mutates font_cache).
        let shaped = self.font_cache.shape_text(text, 0, quantized);
        for sg in &shaped {
            self.font_cache.ensure_glyph(0, quantized, sg.glyph_id);
        }
        let ascent = self.font_cache.ascent(0, quantized);

        // Phase 2: render glyphs using disjoint field borrows.
        // We borrow self.font_cache immutably and self.target/self.state mutably.
        let px_x = (x * self.state.scale_x + self.state.offset_x) as i32;
        let baseline_y = (y * self.state.scale_y + self.state.offset_y) as i32 + ascent;

        let PixelRect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.state.clip;
        let canvas_color = self.state.canvas_color;
        let global_alpha = self.state.alpha;
        let tw = self.target.width() as i32;
        let th = self.target.height() as i32;

        let mut pen_x = 0i32;
        for sg in &shaped {
            let key = GlyphCacheKey {
                font_id: 0,
                size_px: quantized,
                glyph_id: sg.glyph_id,
            };
            if let Some(glyph) = self.font_cache.get_cached_glyph(&key) {
                if glyph.width > 0 && glyph.height > 0 {
                    let gx = px_x + pen_x + sg.x_offset.round() as i32 + glyph.bearing_x;
                    let gy = baseline_y - sg.y_offset.round() as i32 - glyph.bearing_y;

                    let gw = glyph.width as i32;
                    let gh = glyph.height as i32;

                    // Compute visible bounds (clip early).
                    let row_start = (cy - gy).max(0);
                    let row_end = ((cy + ch) - gy).min(gh);
                    let col_start = (cx - gx).max(0);
                    let col_end = ((cx + cw) - gx).min(gw);

                    for row in row_start..row_end {
                        let py = gy + row;
                        if py < 0 || py >= th {
                            continue;
                        }
                        for col in col_start..col_end {
                            let px = gx + col;
                            if px < 0 || px >= tw {
                                continue;
                            }
                            let a = glyph.bitmap[(row as u32 * glyph.width + col as u32) as usize];
                            if a == 0 {
                                continue;
                            }
                            let c = Color::rgba(
                                color.r(),
                                color.g(),
                                color.b(),
                                (color.a() as u16 * a as u16 / 255) as u8,
                            );
                            let existing = self.target.pixel(px as u32, py as u32);
                            let bg =
                                Color::rgba(existing[0], existing[1], existing[2], existing[3]);
                            let result = bg.canvas_blend(c, canvas_color, global_alpha);
                            let out = self.target.pixel_mut(px as u32, py as u32);
                            out[0] = result.r();
                            out[1] = result.g();
                            out[2] = result.b();
                            out[3] = result.a();
                        }
                    }
                }
            }
            pen_x += sg.x_advance.round() as i32;
        }
    }

    /// Get the size of text in user coordinates at the given size.
    /// Returns (width, height).
    pub fn get_text_size(&self, text: &str, size_px: f64) -> (f64, f64) {
        let quantized = FontCache::quantize_size(size_px);
        self.font_cache.measure_text(text, 0, quantized)
    }

    /// Draw a rectangle outline. Uses four rects for axis-aligned precision.
    pub fn paint_rect_outlined(&mut self, x: f64, y: f64, w: f64, h: f64, stroke: &Stroke) {
        let sw = stroke.width;
        if w <= 0.0 || h <= 0.0 || sw <= 0.0 {
            return;
        }
        if sw * 2.0 >= w || sw * 2.0 >= h {
            self.paint_rect(x, y, w, h, stroke.color);
            return;
        }
        // Top
        self.paint_rect(x, y, w, sw, stroke.color);
        // Bottom
        self.paint_rect(x, y + h - sw, w, sw, stroke.color);
        // Left
        self.paint_rect(x, y + sw, sw, h - 2.0 * sw, stroke.color);
        // Right
        self.paint_rect(x + w - sw, y + sw, sw, h - 2.0 * sw, stroke.color);
    }

    /// Draw a rounded rectangle outline.
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
        let sw = stroke.width;
        if sw * 2.0 >= w || sw * 2.0 >= h {
            self.paint_round_rect(x, y, w, h, radius, stroke.color);
            return;
        }
        let mut outer = Self::round_rect_polygon(x, y, w, h, radius);
        let inner_r = (radius - sw).max(0.0);
        let inner = Self::round_rect_polygon(x + sw, y + sw, w - 2.0 * sw, h - 2.0 * sw, inner_r);
        // Bridge + reversed inner for NonZero winding hole.
        outer.push(outer[0]);
        let first_inner = inner[0];
        outer.push(first_inner);
        outer.extend(inner.iter().rev());
        outer.push(first_inner);
        self.fill_polygon_aa(&outer, stroke.color, WindingRule::NonZero);
    }

    /// Draw an ellipse outline using polygon ring.
    pub fn paint_ellipse_outlined(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, stroke: &Stroke) {
        if rx <= 0.0 || ry <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        let sw = stroke.width;
        let irx = (rx - sw).max(0.0);
        let iry = (ry - sw).max(0.0);
        if irx <= 0.0 || iry <= 0.0 {
            self.paint_ellipse(cx, cy, rx, ry, stroke.color);
            return;
        }
        let mut outer = Self::ellipse_polygon(cx, cy, rx, ry);
        let inner = Self::ellipse_polygon(cx, cy, irx, iry);
        outer.push(outer[0]);
        let first_inner = inner[0];
        outer.push(first_inner);
        outer.extend(inner.iter().rev());
        outer.push(first_inner);
        self.fill_polygon_aa(&outer, stroke.color, WindingRule::NonZero);
    }

    /// Fill the current clip rect with a solid color.
    pub fn clear(&mut self, color: Color) {
        let clip = self.state.clip;
        self.fill_rect_pixels(clip.x, clip.y, clip.w, clip.h, color);
    }

    /// Draw a stroked polyline with proper joins and caps.
    ///
    /// Uses two-pass polygon tracing: forward (right side), backward (left side).
    /// Produces a single filled polygon for proper join rendering.
    pub fn paint_solid_polyline(&mut self, vertices: &[(f64, f64)], stroke: &Stroke, closed: bool) {
        if vertices.len() < 2 {
            return;
        }
        if stroke.width <= 0.0 {
            return;
        }

        let half_w = stroke.width / 2.0;
        let n = vertices.len();
        let max_miter = 5.0;

        // Compute segment directions and normals.
        let mut dirs: Vec<(f64, f64)> = Vec::with_capacity(n - 1);
        let mut normals: Vec<(f64, f64)> = Vec::with_capacity(n - 1);
        let seg_count = if closed { n } else { n - 1 };

        for i in 0..seg_count {
            let j = (i + 1) % n;
            let dx = vertices[j].0 - vertices[i].0;
            let dy = vertices[j].1 - vertices[i].1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-10 {
                dirs.push((1.0, 0.0));
                normals.push((0.0, -1.0));
            } else {
                dirs.push((dx / len, dy / len));
                normals.push((-dy / len, dx / len));
            }
        }

        if dirs.is_empty() {
            return;
        }

        // Build outline polygon: right side forward, left side backward.
        let mut outline: Vec<(f64, f64)> = Vec::with_capacity(n * 4);

        // Forward pass (right side).
        for i in 0..n {
            if !closed && i == 0 {
                // Start cap.
                let cap_pts =
                    self.cap_vertices(vertices[0], dirs[0], normals[0], half_w, &stroke.cap, true);
                outline.extend_from_slice(&cap_pts);
            } else if !closed && i == n - 1 {
                // End cap.
                let cap_pts = self.cap_vertices(
                    vertices[n - 1],
                    dirs[seg_count - 1],
                    normals[seg_count - 1],
                    half_w,
                    &stroke.cap,
                    false,
                );
                outline.extend_from_slice(&cap_pts);
            } else {
                // Joint between segments.
                let seg_a = if closed {
                    (i + seg_count - 1) % seg_count
                } else {
                    (i - 1).min(seg_count - 1)
                };
                let seg_b = if closed {
                    i % seg_count
                } else {
                    i.min(seg_count - 1)
                };
                let join_pts = Self::join_vertices(
                    vertices[i],
                    normals[seg_a],
                    normals[seg_b],
                    half_w,
                    max_miter,
                    &stroke.join,
                    true,
                );
                outline.extend_from_slice(&join_pts);
            }
        }

        // Backward pass (left side).
        for i in (0..n).rev() {
            if !closed && i == n - 1 {
                // Already handled end cap in forward pass.
                continue;
            } else if !closed && i == 0 {
                // Already handled start cap in forward pass.
                continue;
            } else {
                let seg_a = if closed {
                    (i + seg_count - 1) % seg_count
                } else {
                    (i - 1).min(seg_count - 1)
                };
                let seg_b = if closed {
                    i % seg_count
                } else {
                    i.min(seg_count - 1)
                };
                let join_pts = Self::join_vertices(
                    vertices[i],
                    normals[seg_a],
                    normals[seg_b],
                    half_w,
                    max_miter,
                    &stroke.join,
                    false,
                );
                outline.extend_from_slice(&join_pts);
            }
        }

        self.fill_polygon_aa(&outline, stroke.color, WindingRule::NonZero);
    }

    /// Compute cap vertices at a polyline endpoint.
    fn cap_vertices(
        &self,
        point: (f64, f64),
        dir: (f64, f64),
        normal: (f64, f64),
        half_w: f64,
        cap: &super::stroke::LineCap,
        is_start: bool,
    ) -> Vec<(f64, f64)> {
        let (px, py) = point;
        let (nx, ny) = normal;
        let (dx, dy) = dir;
        let sign = if is_start { -1.0 } else { 1.0 };

        match cap {
            super::stroke::LineCap::Butt => {
                // Right side, then left side will be added in backward pass.
                vec![
                    (
                        px + nx * half_w + sign * dx * 0.0,
                        py + ny * half_w + sign * dy * 0.0,
                    ),
                    (
                        px - nx * half_w + sign * dx * 0.0,
                        py - ny * half_w + sign * dy * 0.0,
                    ),
                ]
            }
            super::stroke::LineCap::Square => {
                vec![
                    (
                        px + nx * half_w - sign * dx * half_w,
                        py + ny * half_w - sign * dy * half_w,
                    ),
                    (
                        px - nx * half_w - sign * dx * half_w,
                        py - ny * half_w - sign * dy * half_w,
                    ),
                ]
            }
            super::stroke::LineCap::Round => {
                let segments = 8;
                let mut pts = Vec::with_capacity(segments + 1);
                let start_angle = if is_start {
                    std::f64::consts::FRAC_PI_2
                } else {
                    -std::f64::consts::FRAC_PI_2
                };
                for i in 0..=segments {
                    let t = i as f64 / segments as f64;
                    let angle = start_angle + std::f64::consts::PI * t;
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();
                    pts.push((
                        px + half_w * (cos_a * nx + sin_a * dx * sign),
                        py + half_w * (cos_a * ny + sin_a * dy * sign),
                    ));
                }
                pts
            }
        }
    }

    /// Compute join vertices at a polyline joint.
    /// `right_side` selects which side of the stroke to generate vertices for.
    fn join_vertices(
        point: (f64, f64),
        normal_a: (f64, f64),
        normal_b: (f64, f64),
        half_w: f64,
        max_miter: f64,
        join: &super::stroke::LineJoin,
        right_side: bool,
    ) -> Vec<(f64, f64)> {
        let (px, py) = point;
        let sign = if right_side { 1.0 } else { -1.0 };
        let (na_x, na_y) = (normal_a.0 * sign, normal_a.1 * sign);
        let (nb_x, nb_y) = (normal_b.0 * sign, normal_b.1 * sign);

        match join {
            super::stroke::LineJoin::Bevel => {
                vec![
                    (px + na_x * half_w, py + na_y * half_w),
                    (px + nb_x * half_w, py + nb_y * half_w),
                ]
            }
            super::stroke::LineJoin::Miter => {
                // Compute miter point.
                let mx = na_x + nb_x;
                let my = na_y + nb_y;
                let d = mx * na_x + my * na_y;
                if d.abs() < 1e-10 || (1.0 / d).abs() > max_miter {
                    // Fall back to bevel.
                    vec![
                        (px + na_x * half_w, py + na_y * half_w),
                        (px + nb_x * half_w, py + nb_y * half_w),
                    ]
                } else {
                    let scale = half_w / d;
                    vec![(px + mx * scale, py + my * scale)]
                }
            }
            super::stroke::LineJoin::Round => {
                let angle_a = na_y.atan2(na_x);
                let angle_b = nb_y.atan2(nb_x);
                let mut sweep = angle_b - angle_a;
                if sweep > std::f64::consts::PI {
                    sweep -= 2.0 * std::f64::consts::PI;
                }
                if sweep < -std::f64::consts::PI {
                    sweep += 2.0 * std::f64::consts::PI;
                }
                let segments = (sweep.abs() * 4.0 / std::f64::consts::PI).ceil() as usize;
                let segments = segments.clamp(1, 16);
                let mut pts = Vec::with_capacity(segments + 1);
                for i in 0..=segments {
                    let t = i as f64 / segments as f64;
                    let angle = angle_a + t * sweep;
                    pts.push((px + half_w * angle.cos(), py + half_w * angle.sin()));
                }
                pts
            }
        }
    }

    /// Draw a stroked line with optional end decorations.
    pub fn paint_line_stroked(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, stroke: &Stroke) {
        // For width=1, just draw a simple line (no decorations)
        if stroke.width <= 1.0
            && !stroke.start_end.is_decorated()
            && !stroke.finish_end.is_decorated()
        {
            self.paint_line(x0, y0, x1, y1, stroke.color);
            return;
        }

        // Compute direction and normal vectors
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }

        // Unit direction along line
        let udx = dx / len;
        let udy = dy / len;
        // Unit normal (perpendicular)
        let unx = -udy;
        let uny = udx;

        // Cut line at ends for decorations
        let (ax0, ay0) = Self::cut_line_at_end(x0, y0, -udx, -udy, stroke.width, &stroke.start_end);
        let (ax1, ay1) = Self::cut_line_at_end(x1, y1, udx, udy, stroke.width, &stroke.finish_end);

        // Draw the line body as a filled polygon
        let half_w = stroke.width / 2.0;
        let nx = unx * half_w;
        let ny = uny * half_w;

        self.paint_polygon(
            &[
                (ax0 + nx, ay0 + ny),
                (ax1 + nx, ay1 + ny),
                (ax1 - nx, ay1 - ny),
                (ax0 - nx, ay0 - ny),
            ],
            stroke.color,
        );

        let rounded = stroke.join == super::stroke::LineJoin::Round;

        // Draw start end (direction reversed — points away from the line)
        if stroke.start_end.is_decorated() {
            self.paint_stroke_end(
                x0,
                y0,
                unx,
                uny,
                -udx,
                -udy,
                stroke.width,
                stroke.color,
                &stroke.start_end,
                rounded,
            );
        }

        // Draw finish end
        if stroke.finish_end.is_decorated() {
            self.paint_stroke_end(
                x1,
                y1,
                unx,
                uny,
                udx,
                udy,
                stroke.width,
                stroke.color,
                &stroke.finish_end,
                rounded,
            );
        }
    }

    /// Calculate how far to shorten a line end so decorations don't overlap the stroke body.
    /// Returns the adjusted endpoint.
    fn cut_line_at_end(
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        thickness: f64,
        end: &StrokeEnd,
    ) -> (f64, f64) {
        let cut = match end.end_type {
            StrokeEndType::Butt => 0.0,
            StrokeEndType::Cap => thickness * 0.5,
            StrokeEndType::Arrow => {
                let l = thickness * ARROW_BASE_SIZE * end.length_factor;
                l * (1.0 - ARROW_NOTCH)
            }
            StrokeEndType::ContourArrow => {
                let l = thickness * ARROW_BASE_SIZE * end.length_factor;
                l * (1.0 - ARROW_NOTCH)
            }
            StrokeEndType::LineArrow => 0.0, // open arrow, no cut needed
            StrokeEndType::Triangle | StrokeEndType::ContourTriangle => 0.0,
            StrokeEndType::Square | StrokeEndType::ContourSquare => {
                thickness * ARROW_BASE_SIZE * end.length_factor
            }
            StrokeEndType::HalfSquare => 0.0,
            StrokeEndType::Circle | StrokeEndType::ContourCircle => {
                let l = thickness * ARROW_BASE_SIZE * end.length_factor;
                l * 0.5
            }
            StrokeEndType::HalfCircle => 0.0,
            StrokeEndType::Diamond | StrokeEndType::ContourDiamond => {
                let l = thickness * ARROW_BASE_SIZE * end.length_factor;
                l * 0.5
            }
            StrokeEndType::HalfDiamond => 0.0,
            StrokeEndType::Stroke => 0.0,
        };

        // Move the endpoint inward (opposite to dx,dy which points outward)
        (x + dx * cut, y + dy * cut)
    }

    /// Generate vertices for an approximate ellipse/circle using line segments.
    ///
    /// `center` is the ellipse center, `radii` are (normal_radius, direction_radius),
    /// `normal` and `direction` are the oriented basis vectors.
    fn ellipse_vertices(
        center: (f64, f64),
        radii: (f64, f64),
        normal: (f64, f64),
        direction: (f64, f64),
        segments: usize,
    ) -> Vec<(f64, f64)> {
        let (cx, cy) = center;
        let (rx, ry) = radii;
        let (nx, ny) = normal;
        let (dx, dy) = direction;
        let mut verts = Vec::with_capacity(segments);
        for i in 0..segments {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let px = cx + rx * cos_a * nx + ry * sin_a * dx;
            let py = cy + rx * cos_a * ny + ry * sin_a * dy;
            verts.push((px, py));
        }
        verts
    }

    /// Generate vertices for a half-ellipse (semicircle) as an open polyline.
    ///
    /// `center` is the arc center, `radii` are (normal_radius, direction_radius),
    /// `normal` and `direction` are the oriented basis vectors.
    fn half_ellipse_vertices(
        center: (f64, f64),
        radii: (f64, f64),
        normal: (f64, f64),
        direction: (f64, f64),
        segments: usize,
    ) -> Vec<(f64, f64)> {
        let (cx, cy) = center;
        let (rx, ry) = radii;
        let (nx, ny) = normal;
        let (dx, dy) = direction;
        let mut verts = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            // Half circle: from -PI/2 to PI/2 (the half facing away from the line)
            let angle =
                -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / segments as f64;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let px = cx + rx * cos_a * nx + ry * sin_a * dx;
            let py = cy + rx * cos_a * ny + ry * sin_a * dy;
            verts.push((px, py));
        }
        verts
    }

    /// Paint a stroke end decoration at an endpoint.
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
        let r = thickness * ARROW_BASE_SIZE * 0.5 * stroke_end.width_factor;
        let l = thickness * ARROW_BASE_SIZE * stroke_end.length_factor;
        let half_width = thickness * 0.5;

        match stroke_end.end_type {
            StrokeEndType::Butt => {} // Nothing

            StrokeEndType::Cap => {
                if rounded {
                    // Semicircle cap
                    let half_t = thickness * 0.5;
                    let verts = Self::half_ellipse_vertices(
                        (x, y),
                        (half_t, half_t),
                        (nx, ny),
                        (dx, dy),
                        CIRCLE_SEGMENTS,
                    );
                    self.paint_polygon(&verts, stroke_color);
                } else {
                    // Rectangular cap extension
                    let half_t = thickness * 0.5;
                    self.paint_polygon(
                        &[
                            (x + half_t * nx, y + half_t * ny),
                            (x + half_t * nx + half_t * dx, y + half_t * ny + half_t * dy),
                            (x - half_t * nx + half_t * dx, y - half_t * ny + half_t * dy),
                            (x - half_t * nx, y - half_t * ny),
                        ],
                        stroke_color,
                    );
                }
            }

            StrokeEndType::Arrow => {
                // 4-vertex: tip, wing+, notch, wing-
                let tip_x = x;
                let tip_y = y;
                let wing_px = x + l * dx + r * nx;
                let wing_py = y + l * dy + r * ny;
                let wing_mx = x + l * dx - r * nx;
                let wing_my = y + l * dy - r * ny;
                let notch_x = x + (1.0 - ARROW_NOTCH) * l * dx;
                let notch_y = y + (1.0 - ARROW_NOTCH) * l * dy;
                self.paint_polygon(
                    &[
                        (tip_x, tip_y),
                        (wing_px, wing_py),
                        (notch_x, notch_y),
                        (wing_mx, wing_my),
                    ],
                    stroke_color,
                );
            }

            StrokeEndType::ContourArrow => {
                let tip_x = x;
                let tip_y = y;
                let wing_px = x + l * dx + r * nx;
                let wing_py = y + l * dy + r * ny;
                let wing_mx = x + l * dx - r * nx;
                let wing_my = y + l * dy - r * ny;
                let notch_x = x + (1.0 - ARROW_NOTCH) * l * dx;
                let notch_y = y + (1.0 - ARROW_NOTCH) * l * dy;
                let outer = [
                    (tip_x, tip_y),
                    (wing_px, wing_py),
                    (notch_x, notch_y),
                    (wing_mx, wing_my),
                ];
                let inner = inset_polygon(&outer, half_width);
                self.paint_polygon(&outer, stroke_color);
                self.paint_polygon(&inner, stroke_end.inner_color);
            }

            StrokeEndType::LineArrow => {
                // Open arrow: two lines from wings to tip
                let tip_x = x;
                let tip_y = y;
                let wing_px = x + l * dx + r * nx;
                let wing_py = y + l * dy + r * ny;
                let wing_mx = x + l * dx - r * nx;
                let wing_my = y + l * dy - r * ny;
                self.paint_thick_line(wing_px, wing_py, tip_x, tip_y, thickness, stroke_color);
                self.paint_thick_line(tip_x, tip_y, wing_mx, wing_my, thickness, stroke_color);
            }

            StrokeEndType::Triangle => {
                let tip_x = x;
                let tip_y = y;
                let base_px = x + l * dx + r * nx;
                let base_py = y + l * dy + r * ny;
                let base_mx = x + l * dx - r * nx;
                let base_my = y + l * dy - r * ny;
                self.paint_polygon(
                    &[(tip_x, tip_y), (base_px, base_py), (base_mx, base_my)],
                    stroke_color,
                );
            }

            StrokeEndType::ContourTriangle => {
                let tip_x = x;
                let tip_y = y;
                let base_px = x + l * dx + r * nx;
                let base_py = y + l * dy + r * ny;
                let base_mx = x + l * dx - r * nx;
                let base_my = y + l * dy - r * ny;
                let outer = [(tip_x, tip_y), (base_px, base_py), (base_mx, base_my)];
                let inner = inset_polygon(&outer, half_width);
                self.paint_polygon(&outer, stroke_color);
                self.paint_polygon(&inner, stroke_end.inner_color);
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
                );
            }

            StrokeEndType::ContourSquare => {
                let outer = [
                    (x + r * nx, y + r * ny),
                    (x + l * dx + r * nx, y + l * dy + r * ny),
                    (x + l * dx - r * nx, y + l * dy - r * ny),
                    (x - r * nx, y - r * ny),
                ];
                let inner = inset_polygon(&outer, half_width);
                self.paint_polygon(&outer, stroke_color);
                self.paint_polygon(&inner, stroke_end.inner_color);
            }

            StrokeEndType::HalfSquare => {
                // 3 sides of rectangle (open toward line)
                let p0 = (x + r * nx, y + r * ny);
                let p1 = (x + l * dx + r * nx, y + l * dy + r * ny);
                let p2 = (x + l * dx - r * nx, y + l * dy - r * ny);
                let p3 = (x - r * nx, y - r * ny);
                self.paint_polyline(&[p0, p1, p2, p3], stroke_color, thickness);
            }

            StrokeEndType::Circle => {
                let center = (x + l * 0.5 * dx, y + l * 0.5 * dy);
                let verts = Self::ellipse_vertices(
                    center,
                    (r, l * 0.5),
                    (nx, ny),
                    (dx, dy),
                    CIRCLE_SEGMENTS,
                );
                self.paint_polygon(&verts, stroke_color);
            }

            StrokeEndType::ContourCircle => {
                let center = (x + l * 0.5 * dx, y + l * 0.5 * dy);
                let outer = Self::ellipse_vertices(
                    center,
                    (r, l * 0.5),
                    (nx, ny),
                    (dx, dy),
                    CIRCLE_SEGMENTS,
                );
                let inner = Self::ellipse_vertices(
                    center,
                    ((r - half_width).max(0.0), (l * 0.5 - half_width).max(0.0)),
                    (nx, ny),
                    (dx, dy),
                    CIRCLE_SEGMENTS,
                );
                self.paint_polygon(&outer, stroke_color);
                self.paint_polygon(&inner, stroke_end.inner_color);
            }

            StrokeEndType::HalfCircle => {
                let verts = Self::half_ellipse_vertices(
                    (x, y),
                    (r, l * 0.5),
                    (nx, ny),
                    (dx, dy),
                    CIRCLE_SEGMENTS,
                );
                self.paint_polyline(&verts, stroke_color, thickness);
            }

            StrokeEndType::Diamond => {
                let tip_x = x;
                let tip_y = y;
                let mid_px = x + l * 0.5 * dx + r * nx;
                let mid_py = y + l * 0.5 * dy + r * ny;
                let back_x = x + l * dx;
                let back_y = y + l * dy;
                let mid_mx = x + l * 0.5 * dx - r * nx;
                let mid_my = y + l * 0.5 * dy - r * ny;
                self.paint_polygon(
                    &[
                        (tip_x, tip_y),
                        (mid_px, mid_py),
                        (back_x, back_y),
                        (mid_mx, mid_my),
                    ],
                    stroke_color,
                );
            }

            StrokeEndType::ContourDiamond => {
                let tip_x = x;
                let tip_y = y;
                let mid_px = x + l * 0.5 * dx + r * nx;
                let mid_py = y + l * 0.5 * dy + r * ny;
                let back_x = x + l * dx;
                let back_y = y + l * dy;
                let mid_mx = x + l * 0.5 * dx - r * nx;
                let mid_my = y + l * 0.5 * dy - r * ny;
                let outer = [
                    (tip_x, tip_y),
                    (mid_px, mid_py),
                    (back_x, back_y),
                    (mid_mx, mid_my),
                ];
                let inner = inset_polygon(&outer, half_width);
                self.paint_polygon(&outer, stroke_color);
                self.paint_polygon(&inner, stroke_end.inner_color);
            }

            StrokeEndType::HalfDiamond => {
                // Half diamond as open polyline
                let tip_x = x;
                let tip_y = y;
                let mid_px = x + l * 0.5 * dx + r * nx;
                let mid_py = y + l * 0.5 * dy + r * ny;
                let back_x = x + l * dx;
                let back_y = y + l * dy;
                self.paint_polyline(
                    &[(tip_x, tip_y), (mid_px, mid_py), (back_x, back_y)],
                    stroke_color,
                    thickness,
                );
            }

            StrokeEndType::Stroke => {
                // Perpendicular line at endpoint
                let stroke_thickness = thickness * stroke_end.length_factor;
                let p0x = x + r * nx;
                let p0y = y + r * ny;
                let p1x = x - r * nx;
                let p1y = y - r * ny;
                self.paint_thick_line(p0x, p0y, p1x, p1y, stroke_thickness, stroke_color);
            }
        }
    }

    // --- Anti-aliased polygon fill ---

    /// Fill a polygon with anti-aliased edges using the scanline rasterizer.
    fn fill_polygon_aa(&mut self, vertices: &[(f64, f64)], color: Color, rule: WindingRule) {
        if vertices.len() < 3 {
            return;
        }

        let fixed_verts: Vec<(Fixed12, Fixed12)> = vertices
            .iter()
            .map(|&(x, y)| {
                (
                    Fixed12::from_f64(x * self.state.scale_x + self.state.offset_x),
                    Fixed12::from_f64(y * self.state.scale_y + self.state.offset_y),
                )
            })
            .collect();

        let rows = scanline::rasterize(&fixed_verts, self.state.clip, rule);

        for (y, spans) in &rows {
            for span in spans {
                self.blit_span(*y, span, color);
            }
        }
    }

    /// Blit a single AA span onto the target.
    fn blit_span(&mut self, y: i32, span: &scanline::Span, color: Color) {
        let tw = self.target.width() as i32;
        let th = self.target.height() as i32;
        if y < 0 || y >= th {
            return;
        }

        let x_start = span.x_start.max(0);
        let x_end = span.x_end.min(tw);
        let span_width = x_end - x_start;
        if span_width <= 0 {
            return;
        }

        // Fast path: fully opaque, no global alpha modulation needed beyond what blend_pixel does.
        if span_width == 1 {
            let opacity = span.opacity_beg;
            if opacity > 0 {
                self.blend_pixel_alpha(x_start, y, color, opacity);
            }
            return;
        }

        // First pixel (partial coverage).
        if span.opacity_beg > 0 {
            if span.opacity_beg == 255 {
                self.blend_pixel(x_start, y, color);
            } else {
                self.blend_pixel_alpha(x_start, y, color, span.opacity_beg);
            }
        }

        // Interior pixels (full coverage) — fast path.
        if span.opacity_mid == 255 {
            for x in (x_start + 1)..(x_end - 1) {
                self.blend_pixel(x, y, color);
            }
        } else if span.opacity_mid > 0 {
            for x in (x_start + 1)..(x_end - 1) {
                self.blend_pixel_alpha(x, y, color, span.opacity_mid);
            }
        }

        // Last pixel (partial coverage).
        if span_width > 1 && span.opacity_end > 0 {
            let x_last = x_end - 1;
            if span.opacity_end == 255 {
                self.blend_pixel(x_last, y, color);
            } else {
                self.blend_pixel_alpha(x_last, y, color, span.opacity_end);
            }
        }
    }

    /// Blend a pixel with an additional alpha coverage factor (0-255).
    fn blend_pixel_alpha(&mut self, x: i32, y: i32, color: Color, coverage: u8) {
        let modulated = Color::rgba(
            color.r(),
            color.g(),
            color.b(),
            (color.a() as u16 * coverage as u16 / 255) as u8,
        );
        self.blend_pixel(x, y, modulated);
    }

    /// Generate polygon vertices approximating an ellipse.
    fn ellipse_polygon(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<(f64, f64)> {
        let segments = adaptive_circle_segments(rx.max(ry));
        let mut verts = Vec::with_capacity(segments);
        for i in 0..segments {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
            verts.push((cx + rx * angle.cos(), cy + ry * angle.sin()));
        }
        verts
    }

    /// Generate polygon vertices for a rounded rectangle.
    fn round_rect_polygon(x: f64, y: f64, w: f64, h: f64, r: f64) -> Vec<(f64, f64)> {
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        if r < 0.5 {
            return vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        }
        let corner_segments = (adaptive_circle_segments(r) / 4).max(2);
        let mut verts = Vec::with_capacity(corner_segments * 4 + 4);

        // Top-right corner
        for i in 0..=corner_segments {
            let angle = -std::f64::consts::FRAC_PI_2
                + std::f64::consts::FRAC_PI_2 * i as f64 / corner_segments as f64;
            verts.push((x + w - r + r * angle.cos(), y + r + r * angle.sin()));
        }
        // Bottom-right corner
        for i in 0..=corner_segments {
            let angle = std::f64::consts::FRAC_PI_2 * i as f64 / corner_segments as f64;
            verts.push((x + w - r + r * angle.cos(), y + h - r + r * angle.sin()));
        }
        // Bottom-left corner
        for i in 0..=corner_segments {
            let angle = std::f64::consts::FRAC_PI_2
                + std::f64::consts::FRAC_PI_2 * i as f64 / corner_segments as f64;
            verts.push((x + r + r * angle.cos(), y + h - r + r * angle.sin()));
        }
        // Top-left corner
        for i in 0..=corner_segments {
            let angle = std::f64::consts::PI
                + std::f64::consts::FRAC_PI_2 * i as f64 / corner_segments as f64;
            verts.push((x + r + r * angle.cos(), y + r + r * angle.sin()));
        }

        verts
    }

    // --- Coordinate transform helpers ---

    fn to_pixel_x(&self, x: f64) -> i32 {
        (x * self.state.scale_x + self.state.offset_x) as i32
    }

    fn to_pixel_y(&self, y: f64) -> i32 {
        (y * self.state.scale_y + self.state.offset_y) as i32
    }

    // --- Pixel-level operations ---

    fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        let PixelRect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.state.clip;
        if x < cx || x >= cx + cw || y < cy || y >= cy + ch {
            return;
        }
        if x < 0 || y < 0 || x >= self.target.width() as i32 || y >= self.target.height() as i32 {
            return;
        }

        let px = self.target.pixel(x as u32, y as u32);
        let existing = Color::rgba(px[0], px[1], px[2], px[3]);
        let result = existing.canvas_blend(color, self.state.canvas_color, self.state.alpha);
        let out = self.target.pixel_mut(x as u32, y as u32);
        out[0] = result.r();
        out[1] = result.g();
        out[2] = result.b();
        out[3] = result.a();
    }

    fn fill_rect_pixels(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let PixelRect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.state.clip;
        let start_x = x.max(cx).max(0);
        let start_y = y.max(cy).max(0);
        let end_x = (x + w).min(cx + cw).min(self.target.width() as i32);
        let end_y = (y + h).min(cy + ch).min(self.target.height() as i32);

        for row in start_y..end_y {
            for col in start_x..end_x {
                self.blend_pixel(col, row, color);
            }
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

/// Choose number of polygon segments for circle approximation based on radius.
fn adaptive_circle_segments(radius: f64) -> usize {
    if radius < 4.0 {
        8
    } else if radius < 16.0 {
        16
    } else if radius < 64.0 {
        32
    } else if radius < 256.0 {
        64
    } else {
        128
    }
}

/// Inset a convex polygon by `distance` toward its centroid.
/// For each vertex, compute the offset along the bisector of adjacent edges,
/// clamped to `distance * 5.0` to prevent self-intersection.
fn inset_polygon(vertices: &[(f64, f64)], distance: f64) -> Vec<(f64, f64)> {
    let n = vertices.len();
    if n < 3 {
        return vertices.to_vec();
    }

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let prev = vertices[(i + n - 1) % n];
        let curr = vertices[i];
        let next = vertices[(i + 1) % n];

        // Edge normals (pointing inward for CW polygon).
        let e0 = (curr.0 - prev.0, curr.1 - prev.1);
        let e1 = (next.0 - curr.0, next.1 - curr.1);
        let len0 = (e0.0 * e0.0 + e0.1 * e0.1).sqrt();
        let len1 = (e1.0 * e1.0 + e1.1 * e1.1).sqrt();
        if len0 < 1e-10 || len1 < 1e-10 {
            result.push(curr);
            continue;
        }
        // Inward normals.
        let n0 = (e0.1 / len0, -e0.0 / len0);
        let n1 = (e1.1 / len1, -e1.0 / len1);

        // Bisector direction.
        let bx = n0.0 + n1.0;
        let by = n0.1 + n1.1;
        let bl = (bx * bx + by * by).sqrt();
        if bl < 1e-10 {
            result.push(curr);
            continue;
        }

        // sin(half_angle) = cross product magnitude.
        let dot = n0.0 * n1.0 + n0.1 * n1.1;
        let half_angle_sin = ((1.0 - dot) / 2.0).sqrt().max(1e-10);
        let offset = (distance / half_angle_sin).min(distance * 5.0);

        result.push((curr.0 + bx / bl * offset, curr.1 + by / bl * offset));
    }

    result
}

/// Adaptive subdivision of a cubic Bezier into line segments.
fn tessellate_cubic(
    out: &mut Vec<(f64, f64)>,
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    flatness: f64,
    depth: u8,
) {
    if depth >= BEZIER_MAX_DEPTH {
        if out.is_empty() || *out.last().unwrap() != p0 {
            out.push(p0);
        }
        out.push(p3);
        return;
    }

    // Flatness test: max distance of control points from the line P0-P3.
    let dx = p3.0 - p0.0;
    let dy = p3.1 - p0.1;
    let len_sq = dx * dx + dy * dy;

    let flat = if len_sq < 1e-10 {
        let d1 = (p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2);
        let d2 = (p2.0 - p0.0).powi(2) + (p2.1 - p0.1).powi(2);
        d1.max(d2).sqrt()
    } else {
        let inv_len = 1.0 / len_sq.sqrt();
        let d1 = ((p1.0 - p0.0) * dy - (p1.1 - p0.1) * dx).abs() * inv_len;
        let d2 = ((p2.0 - p0.0) * dy - (p2.1 - p0.1) * dx).abs() * inv_len;
        d1.max(d2)
    };

    if flat <= flatness {
        if out.is_empty() || *out.last().unwrap() != p0 {
            out.push(p0);
        }
        out.push(p3);
        return;
    }

    // De Casteljau subdivision at t=0.5.
    let m01 = mid(p0, p1);
    let m12 = mid(p1, p2);
    let m23 = mid(p2, p3);
    let m012 = mid(m01, m12);
    let m123 = mid(m12, m23);
    let m0123 = mid(m012, m123);

    tessellate_cubic(out, p0, m01, m012, m0123, flatness, depth + 1);
    tessellate_cubic(out, m0123, m123, m23, p3, flatness, depth + 1);
}

fn mid(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

/// Expand tab characters to 8-column tab stops.
fn expand_tabs(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut col = 0;
    for ch in s.chars() {
        if ch == '\t' {
            let spaces = 8 - (col % 8);
            for _ in 0..spaces {
                result.push(' ');
            }
            col += spaces;
        } else {
            result.push(ch);
            col += 1;
        }
    }
    result
}
