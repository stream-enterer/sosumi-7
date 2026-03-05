use super::font_cache::FontCache;
use super::stroke::Stroke;
use crate::foundation::{Color, Image};

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

    /// Fill a rounded rectangle.
    pub fn paint_round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
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

    /// Draw a stroked line.
    pub fn paint_line_stroked(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, stroke: &Stroke) {
        // For width=1, just draw a simple line
        if stroke.width <= 1.0 {
            self.paint_line(x0, y0, x1, y1, stroke.color);
            return;
        }

        // For wider strokes, draw as a filled polygon
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }

        let half_w = stroke.width / 2.0;
        let nx = -dy / len * half_w;
        let ny = dx / len * half_w;

        self.paint_polygon(
            &[
                (x0 + nx, y0 + ny),
                (x1 + nx, y1 + ny),
                (x1 - nx, y1 - ny),
                (x0 - nx, y0 - ny),
            ],
            stroke.color,
        );
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
