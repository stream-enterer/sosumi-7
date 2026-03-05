use super::font_cache::FontCache;
use super::stroke::{Stroke, StrokeEnd, StrokeEndType};
use crate::foundation::{Color, Image};

/// Base multiplier for decoration size.
const ARROW_BASE_SIZE: f64 = 10.0;
/// Notch depth ratio for Arrow type.
const ARROW_NOTCH: f64 = 0.3;
/// Number of segments for circle approximation.
const CIRCLE_SEGMENTS: usize = 32;

/// Coordinate transform state.
#[derive(Clone, Debug)]
struct PainterState {
    /// Translation offset.
    offset_x: f64,
    offset_y: f64,
    /// Scale factor.
    scale_x: f64,
    scale_y: f64,
    /// Clip rectangle in pixel coordinates (x, y, w, h).
    clip: (i32, i32, i32, i32),
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
    font_cache: FontCache,
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
        let w = target.width() as i32;
        let h = target.height() as i32;
        Self {
            target,
            state: PainterState {
                offset_x: 0.0,
                offset_y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                clip: (0, 0, w, h),
                canvas_color: Color::BLACK,
                alpha: 255,
            },
            state_stack: Vec::new(),
            font_cache: FontCache::new(),
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

        let (cx, cy, cw, ch) = self.state.clip;
        // Intersect
        let nx = px.max(cx);
        let ny = py.max(cy);
        let nx2 = (px + pw).min(cx + cw);
        let ny2 = (py + ph).min(cy + ch);
        self.state.clip = (nx, ny, (nx2 - nx).max(0), (ny2 - ny).max(0));
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

    /// Fill an ellipse with a solid color.
    pub fn paint_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, color: Color) {
        let pcx = self.to_pixel_x(cx);
        let pcy = self.to_pixel_y(cy);
        let prx = (rx * self.state.scale_x) as i32;
        let pry = (ry * self.state.scale_y) as i32;

        if prx <= 0 || pry <= 0 {
            return;
        }

        let x0 = pcx - prx;
        let y0 = pcy - pry;
        let x1 = pcx + prx;
        let y1 = pcy + pry;

        let (clip_x, clip_y, clip_w, clip_h) = self.state.clip;
        let start_y = y0.max(clip_y);
        let end_y = y1.min(clip_y + clip_h);
        let start_x = x0.max(clip_x);
        let end_x = x1.min(clip_x + clip_w);

        let rx_sq = (prx as f64) * (prx as f64);
        let ry_sq = (pry as f64) * (pry as f64);

        for py in start_y..end_y {
            let dy = py as f64 - pcy as f64 + 0.5;
            for px in start_x..end_x {
                let dx = px as f64 - pcx as f64 + 0.5;
                if (dx * dx) / rx_sq + (dy * dy) / ry_sq <= 1.0 {
                    self.blend_pixel(px, py, color);
                }
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
    pub fn paint_polygon(&mut self, vertices: &[(f64, f64)], color: Color) {
        if vertices.len() < 3 {
            return;
        }

        let pixels: Vec<(i32, i32)> = vertices
            .iter()
            .map(|&(x, y)| (self.to_pixel_x(x), self.to_pixel_y(y)))
            .collect();

        let min_y = pixels.iter().map(|p| p.1).min().unwrap();
        let max_y = pixels.iter().map(|p| p.1).max().unwrap();

        let (clip_x, clip_y, clip_w, clip_h) = self.state.clip;
        let start_y = min_y.max(clip_y);
        let end_y = (max_y + 1).min(clip_y + clip_h);

        for y in start_y..end_y {
            let mut intersections = Vec::new();
            let n = pixels.len();
            for i in 0..n {
                let (x0, y0) = pixels[i];
                let (x1, y1) = pixels[(i + 1) % n];
                if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                    let t = (y - y0) as f64 / (y1 - y0) as f64;
                    intersections.push(x0 + (t * (x1 - x0) as f64) as i32);
                }
            }
            intersections.sort();
            for pair in intersections.chunks(2) {
                if pair.len() == 2 {
                    let sx = pair[0].max(clip_x);
                    let ex = pair[1].min(clip_x + clip_w);
                    for px in sx..ex {
                        self.blend_pixel(px, y, color);
                    }
                }
            }
        }
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
    pub fn paint_polyline(
        &mut self,
        vertices: &[(f64, f64)],
        stroke_color: Color,
        thickness: f64,
    ) {
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

    /// Fill a rounded rectangle.
    pub fn paint_round_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
        color: Color,
    ) {
        let px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);
        let pw = (w * self.state.scale_x) as i32;
        let ph = (h * self.state.scale_y) as i32;
        let pr = (radius * self.state.scale_x.min(self.state.scale_y)) as i32;
        let pr = pr.min(pw / 2).min(ph / 2);

        if pw <= 0 || ph <= 0 {
            return;
        }

        let (clip_x, clip_y, clip_w, clip_h) = self.state.clip;
        let start_y = py.max(clip_y);
        let end_y = (py + ph).min(clip_y + clip_h);

        let r_sq = (pr as f64) * (pr as f64);

        for row in start_y..end_y {
            let ry = row - py;
            let mut sx = px;
            let mut ex = px + pw;

            // Check if in corner region
            if ry < pr {
                // Top corners
                let dy = pr as f64 - ry as f64 - 0.5;
                let dx = (r_sq - dy * dy).max(0.0).sqrt();
                sx = sx.max(px + pr - dx as i32);
                ex = ex.min(px + pw - pr + dx as i32);
            } else if ry >= ph - pr {
                // Bottom corners
                let dy = ry as f64 - (ph - pr) as f64 + 0.5;
                let dx = (r_sq - dy * dy).max(0.0).sqrt();
                sx = sx.max(px + pr - dx as i32);
                ex = ex.min(px + pw - pr + dx as i32);
            }

            sx = sx.max(clip_x);
            ex = ex.min(clip_x + clip_w);

            for col in sx..ex {
                self.blend_pixel(col, row, color);
            }
        }
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

        let (clip_x, clip_y, clip_w, clip_h) = self.state.clip;
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

    /// Draw text at the given position using the built-in bitmap font.
    pub fn paint_text(&mut self, x: f64, y: f64, text: &str, color: Color) {
        let mut px = self.to_pixel_x(x);
        let py = self.to_pixel_y(y);

        for ch in text.chars() {
            let glyph_data = self.font_cache.get_glyph(ch).copied();
            if let Some(glyph) = glyph_data {
                let gw = FontCache::GLYPH_WIDTH as i32;
                let gh = FontCache::GLYPH_HEIGHT as i32;
                for gy in 0..gh {
                    for gx in 0..gw {
                        if glyph[gy as usize] & (1 << (gw - 1 - gx)) != 0 {
                            self.blend_pixel(px + gx, py + gy, color);
                        }
                    }
                }
                px += gw + 1;
            } else {
                px += FontCache::GLYPH_WIDTH as i32 + 1;
            }
        }
    }

    /// Get the size of text in user coordinates at the given char height.
    /// Returns (width, height) scaled to match the requested char_height.
    pub fn get_text_size(&self, text: &str, char_height: f64) -> (f64, f64) {
        let (gw, gh) = FontCache::measure_text(text);
        if gh == 0 {
            return (0.0, 0.0);
        }
        let scale = char_height / gh as f64;
        (gw as f64 * scale, char_height)
    }

    /// Draw text at the given position scaled to the specified character height.
    pub fn paint_text_scaled(
        &mut self,
        x: f64,
        y: f64,
        text: &str,
        char_height: f64,
        color: Color,
    ) {
        let gh = FontCache::GLYPH_HEIGHT as f64;
        let scale = char_height / gh;
        self.push_state();
        self.translate(x, y);
        self.scale(scale, scale);

        let mut px = 0.0;
        for ch in text.chars() {
            let glyph_data = self.font_cache.get_glyph(ch).copied();
            if let Some(glyph) = glyph_data {
                let gw = FontCache::GLYPH_WIDTH as i32;
                let gh_i = FontCache::GLYPH_HEIGHT as i32;
                for gy in 0..gh_i {
                    for gx in 0..gw {
                        if glyph[gy as usize] & (1 << (gw - 1 - gx)) != 0 {
                            self.paint_rect(px + gx as f64, gy as f64, 1.0, 1.0, color);
                        }
                    }
                }
                px += (gw + 1) as f64;
            } else {
                px += (FontCache::GLYPH_WIDTH + 1) as f64;
            }
        }
        self.pop_state();
    }

    /// Draw a rectangle outline with a stroke.
    pub fn paint_rect_outlined(&mut self, x: f64, y: f64, w: f64, h: f64, stroke: &Stroke) {
        let sw = stroke.width;
        // Top
        self.paint_rect(x, y, w, sw, stroke.color);
        // Bottom
        self.paint_rect(x, y + h - sw, w, sw, stroke.color);
        // Left
        self.paint_rect(x, y + sw, sw, h - 2.0 * sw, stroke.color);
        // Right
        self.paint_rect(x + w - sw, y + sw, sw, h - 2.0 * sw, stroke.color);
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
        let (ax0, ay0) =
            Self::cut_line_at_end(x0, y0, -udx, -udy, stroke.width, &stroke.start_end);
        let (ax1, ay1) =
            Self::cut_line_at_end(x1, y1, udx, udy, stroke.width, &stroke.finish_end);

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
                x1, y1, unx, uny, udx, udy, stroke.width, stroke.color, &stroke.finish_end,
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

        match stroke_end.end_type {
            StrokeEndType::Butt => {} // Nothing

            StrokeEndType::Cap => {
                if rounded {
                    // Semicircle cap
                    let half_t = thickness * 0.5;
                    let verts = Self::half_ellipse_vertices(
                        (x, y), (half_t, half_t), (nx, ny), (dx, dy), CIRCLE_SEGMENTS,
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
                let verts = [
                    (tip_x, tip_y),
                    (wing_px, wing_py),
                    (notch_x, notch_y),
                    (wing_mx, wing_my),
                ];
                // Fill inner
                self.paint_polygon(&verts, stroke_end.inner_color);
                // Stroke outline
                self.paint_polygon_outlined(&verts, stroke_color, thickness);
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
                let verts = [(tip_x, tip_y), (base_px, base_py), (base_mx, base_my)];
                self.paint_polygon(&verts, stroke_end.inner_color);
                self.paint_polygon_outlined(&verts, stroke_color, thickness);
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
                let verts = [
                    (x + r * nx, y + r * ny),
                    (x + l * dx + r * nx, y + l * dy + r * ny),
                    (x + l * dx - r * nx, y + l * dy - r * ny),
                    (x - r * nx, y - r * ny),
                ];
                self.paint_polygon(&verts, stroke_end.inner_color);
                self.paint_polygon_outlined(&verts, stroke_color, thickness);
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
                    center, (r, l * 0.5), (nx, ny), (dx, dy), CIRCLE_SEGMENTS,
                );
                self.paint_polygon(&verts, stroke_color);
            }

            StrokeEndType::ContourCircle => {
                let center = (x + l * 0.5 * dx, y + l * 0.5 * dy);
                let verts = Self::ellipse_vertices(
                    center, (r, l * 0.5), (nx, ny), (dx, dy), CIRCLE_SEGMENTS,
                );
                self.paint_polygon(&verts, stroke_end.inner_color);
                self.paint_polygon_outlined(&verts, stroke_color, thickness);
            }

            StrokeEndType::HalfCircle => {
                let verts = Self::half_ellipse_vertices(
                    (x, y), (r, l * 0.5), (nx, ny), (dx, dy), CIRCLE_SEGMENTS,
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
                let verts = [
                    (tip_x, tip_y),
                    (mid_px, mid_py),
                    (back_x, back_y),
                    (mid_mx, mid_my),
                ];
                self.paint_polygon(&verts, stroke_end.inner_color);
                self.paint_polygon_outlined(&verts, stroke_color, thickness);
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

    // --- Coordinate transform helpers ---

    fn to_pixel_x(&self, x: f64) -> i32 {
        (x * self.state.scale_x + self.state.offset_x) as i32
    }

    fn to_pixel_y(&self, y: f64) -> i32 {
        (y * self.state.scale_y + self.state.offset_y) as i32
    }

    // --- Pixel-level operations ---

    fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        let (cx, cy, cw, ch) = self.state.clip;
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
        let (cx, cy, cw, ch) = self.state.clip;
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
