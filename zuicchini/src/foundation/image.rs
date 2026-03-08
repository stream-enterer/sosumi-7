use std::collections::BTreeSet;

use super::color::Color;
use crate::render::interpolation::sample_bilinear;
use crate::render::ImageExtension;

/// CPU bitmap image with 1–4 channels per pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    width: u32,
    height: u32,
    channel_count: u8,
    data: Vec<u8>,
}

impl Image {
    /// Create a zero-filled image.
    ///
    /// # Panics
    /// Panics if `channel_count` is not 1, 2, 3, or 4.
    pub fn new(width: u32, height: u32, channel_count: u8) -> Self {
        assert!(
            (1..=4).contains(&channel_count),
            "channel_count must be 1, 2, 3, or 4"
        );
        let len = width as usize * height as usize * channel_count as usize;
        Self {
            width,
            height,
            channel_count,
            data: vec![0; len],
        }
    }

    /// Create an image from pre-existing pixel data.
    ///
    /// # Panics
    /// Panics if `channel_count` is not 1, 2, 3, or 4, or if `data.len()`
    /// does not equal `width * height * channel_count`.
    pub fn from_raw(width: u32, height: u32, channel_count: u8, data: Vec<u8>) -> Self {
        assert!(
            (1..=4).contains(&channel_count),
            "channel_count must be 1, 2, 3, or 4"
        );
        let expected = width as usize * height as usize * channel_count as usize;
        assert_eq!(
            data.len(),
            expected,
            "data length {} does not match {}x{}x{}={}",
            data.len(),
            width,
            height,
            channel_count,
            expected,
        );
        Self {
            width,
            height,
            channel_count,
            data,
        }
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    pub fn channel_count(&self) -> u8 {
        self.channel_count
    }

    #[inline]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns `true` if either dimension is zero.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    fn pixel_offset(&self, x: u32, y: u32) -> usize {
        debug_assert!(x < self.width && y < self.height);
        (y as usize * self.width as usize + x as usize) * self.channel_count as usize
    }

    /// Access the raw channel bytes for a pixel.
    pub fn pixel(&self, x: u32, y: u32) -> &[u8] {
        let offset = self.pixel_offset(x, y);
        &self.data[offset..offset + self.channel_count as usize]
    }

    /// Mutably access the raw channel bytes for a pixel.
    pub fn pixel_mut(&mut self, x: u32, y: u32) -> &mut [u8] {
        let offset = self.pixel_offset(x, y);
        let cc = self.channel_count as usize;
        &mut self.data[offset..offset + cc]
    }

    /// Fill all pixels with the given color. Only valid for RGBA (4-channel) images.
    ///
    /// # Panics
    /// Panics if `channel_count` is not 4.
    pub fn fill(&mut self, color: Color) {
        assert_eq!(self.channel_count, 4, "fill() requires a 4-channel image");
        let bytes = [color.r(), color.g(), color.b(), color.a()];
        for chunk in self.data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bytes);
        }
    }

    /// Reinitialize in-place with new dimensions, zero-filled.
    pub fn setup(&mut self, w: u32, h: u32, cc: u8) {
        assert!((1..=4).contains(&cc), "channel_count must be 1, 2, 3, or 4");
        self.width = w;
        self.height = h;
        self.channel_count = cc;
        let len = w as usize * h as usize * cc as usize;
        self.data.clear();
        self.data.resize(len, 0);
    }

    /// Reset to 0×0 empty image.
    pub fn clear(&mut self) {
        self.width = 0;
        self.height = 0;
        self.data.clear();
    }

    /// Get a single channel value for a pixel.
    pub fn get_pixel_channel(&self, x: u32, y: u32, ch: u8) -> u8 {
        let offset = self.pixel_offset(x, y);
        self.data[offset + ch as usize]
    }

    /// Set a single channel value for a pixel.
    pub fn set_pixel_channel(&mut self, x: u32, y: u32, ch: u8, val: u8) {
        let offset = self.pixel_offset(x, y);
        self.data[offset + ch as usize] = val;
    }

    /// Fill a rectangle with a color. 4-channel images only. Clips to image bounds.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: Color) {
        assert_eq!(
            self.channel_count, 4,
            "fill_rect() requires a 4-channel image"
        );
        let x1 = x.min(self.width);
        let y1 = y.min(self.height);
        let x2 = (x.saturating_add(w)).min(self.width);
        let y2 = (y.saturating_add(h)).min(self.height);
        let bytes = [color.r(), color.g(), color.b(), color.a()];
        let stride = self.width as usize * 4;
        for row in y1..y2 {
            let row_start = row as usize * stride + x1 as usize * 4;
            for col in 0..(x2 - x1) as usize {
                let off = row_start + col * 4;
                self.data[off..off + 4].copy_from_slice(&bytes);
            }
        }
    }

    /// Fill one channel of the entire image with a value.
    pub fn fill_channel(&mut self, ch: u8, val: u8) {
        let cc = self.channel_count as usize;
        let ch = ch as usize;
        for i in (ch..self.data.len()).step_by(cc) {
            self.data[i] = val;
        }
    }

    /// Fill one channel within a rectangle. Clips to image bounds.
    pub fn fill_channel_rect(&mut self, ch: u8, x: u32, y: u32, w: u32, h: u32, val: u8) {
        let x1 = x.min(self.width);
        let y1 = y.min(self.height);
        let x2 = (x.saturating_add(w)).min(self.width);
        let y2 = (y.saturating_add(h)).min(self.height);
        let cc = self.channel_count as usize;
        let stride = self.width as usize * cc;
        for row in y1..y2 {
            let row_start = row as usize * stride;
            for col in x1..x2 {
                self.data[row_start + col as usize * cc + ch as usize] = val;
            }
        }
    }

    /// Copy entire source image into self at (dx, dy). Channel counts must match.
    pub fn copy_from(&mut self, dx: u32, dy: u32, src: &Image) {
        self.copy_from_rect(dx, dy, src, (0, 0, src.width, src.height));
    }

    /// Copy a rectangle from source image into self. Clips both sides.
    /// `src_rect` is `(x, y, w, h)` within `src`.
    pub fn copy_from_rect(
        &mut self,
        dx: u32,
        dy: u32,
        src: &Image,
        src_rect: (u32, u32, u32, u32),
    ) {
        let (sx, sy, sw, sh) = src_rect;
        assert_eq!(
            self.channel_count, src.channel_count,
            "channel count mismatch: dst={} src={}",
            self.channel_count, src.channel_count
        );
        let cc = self.channel_count as usize;

        // Clip source rect to source bounds
        let sx1 = sx.min(src.width);
        let sy1 = sy.min(src.height);
        let sx2 = (sx.saturating_add(sw)).min(src.width);
        let sy2 = (sy.saturating_add(sh)).min(src.height);

        let copy_w = sx2 - sx1;
        let copy_h = sy2 - sy1;

        // Clip destination
        let copy_w = copy_w.min(self.width.saturating_sub(dx));
        let copy_h = copy_h.min(self.height.saturating_sub(dy));

        let src_stride = src.width as usize * cc;
        let dst_stride = self.width as usize * cc;

        for row in 0..copy_h {
            let src_off = (sy1 + row) as usize * src_stride + sx1 as usize * cc;
            let dst_off = (dy + row) as usize * dst_stride + dx as usize * cc;
            let len = copy_w as usize * cc;
            self.data[dst_off..dst_off + len].copy_from_slice(&src.data[src_off..src_off + len]);
        }
    }

    /// Copy a single channel from src into a (possibly different) channel in self.
    pub fn copy_channel(&mut self, dst_ch: u8, dx: u32, dy: u32, src: &Image, src_ch: u8) {
        let copy_w = src.width.min(self.width.saturating_sub(dx));
        let copy_h = src.height.min(self.height.saturating_sub(dy));
        let scc = src.channel_count as usize;
        let dcc = self.channel_count as usize;

        for row in 0..copy_h {
            for col in 0..copy_w {
                let si = (row as usize * src.width as usize + col as usize) * scc + src_ch as usize;
                let di = ((dy + row) as usize * self.width as usize + (dx + col) as usize) * dcc
                    + dst_ch as usize;
                self.data[di] = src.data[si];
            }
        }
    }

    /// Extract a sub-image.
    pub fn get_cropped(&self, x: u32, y: u32, w: u32, h: u32) -> Image {
        let x1 = x.min(self.width);
        let y1 = y.min(self.height);
        let x2 = (x.saturating_add(w)).min(self.width);
        let y2 = (y.saturating_add(h)).min(self.height);
        let cw = x2 - x1;
        let ch = y2 - y1;
        let cc = self.channel_count as usize;
        let mut data = Vec::with_capacity(cw as usize * ch as usize * cc);
        let stride = self.width as usize * cc;
        for row in y1..y2 {
            let start = row as usize * stride + x1 as usize * cc;
            data.extend_from_slice(&self.data[start..start + cw as usize * cc]);
        }
        Image {
            width: cw,
            height: ch,
            channel_count: self.channel_count,
            data,
        }
    }

    /// Convert to a different channel count. All 12 combos (1↔2↔3↔4) supported.
    pub fn get_converted(&self, new_cc: u8) -> Image {
        assert!(
            (1..=4).contains(&new_cc),
            "channel_count must be 1, 2, 3, or 4"
        );
        let old_cc = self.channel_count;
        if old_cc == new_cc {
            return self.clone();
        }
        let mut out = Image::new(self.width, self.height, new_cc);
        for y in 0..self.height {
            for x in 0..self.width {
                let src = self.pixel(x, y);
                let dst = out.pixel_mut(x, y);
                match (old_cc, new_cc) {
                    (1, 2) => {
                        dst[0] = src[0];
                        dst[1] = 255;
                    }
                    (1, 3) => {
                        dst[0] = src[0];
                        dst[1] = src[0];
                        dst[2] = src[0];
                    }
                    (1, 4) => {
                        dst[0] = src[0];
                        dst[1] = src[0];
                        dst[2] = src[0];
                        dst[3] = 255;
                    }
                    (2, 1) => {
                        dst[0] = src[0];
                    }
                    (2, 3) => {
                        dst[0] = src[0];
                        dst[1] = src[0];
                        dst[2] = src[0];
                    }
                    (2, 4) => {
                        dst[0] = src[0];
                        dst[1] = src[0];
                        dst[2] = src[0];
                        dst[3] = src[1];
                    }
                    (3, 1) => {
                        dst[0] = ((src[0] as u16 + src[1] as u16 + src[2] as u16) / 3) as u8;
                    }
                    (3, 2) => {
                        dst[0] = ((src[0] as u16 + src[1] as u16 + src[2] as u16) / 3) as u8;
                        dst[1] = 255;
                    }
                    (3, 4) => {
                        dst[0] = src[0];
                        dst[1] = src[1];
                        dst[2] = src[2];
                        dst[3] = 255;
                    }
                    (4, 1) => {
                        dst[0] = ((src[0] as u16 + src[1] as u16 + src[2] as u16) / 3) as u8;
                    }
                    (4, 2) => {
                        dst[0] = ((src[0] as u16 + src[1] as u16 + src[2] as u16) / 3) as u8;
                        dst[1] = src[3];
                    }
                    (4, 3) => {
                        dst[0] = src[0];
                        dst[1] = src[1];
                        dst[2] = src[2];
                    }
                    _ => unreachable!(),
                }
            }
        }
        out
    }

    /// Crop to the bounding box of non-zero alpha pixels. Requires 4 or 2 channels.
    pub fn get_cropped_by_alpha(&self) -> Image {
        if let Some((x, y, w, h)) = self.calc_alpha_min_max_rect() {
            self.get_cropped(x, y, w, h)
        } else {
            Image::new(0, 0, self.channel_count)
        }
    }

    /// Returns `true` if any pixel has differing R, G, B channels. Requires ≥3 channels.
    pub fn has_any_non_grey_pixel(&self) -> bool {
        if self.channel_count < 3 {
            return false;
        }
        let cc = self.channel_count as usize;
        for chunk in self.data.chunks_exact(cc) {
            if chunk[0] != chunk[1] || chunk[1] != chunk[2] {
                return true;
            }
        }
        false
    }

    /// Returns `true` if any pixel has alpha < 255. Requires 2 or 4 channels.
    pub fn has_any_transparent_pixel(&self) -> bool {
        let alpha_ch = match self.channel_count {
            2 => 1,
            4 => 3,
            _ => return false,
        };
        let cc = self.channel_count as usize;
        for chunk in self.data.chunks_exact(cc) {
            if chunk[alpha_ch] < 255 {
                return true;
            }
        }
        false
    }

    /// Find the bounding rect of pixels differing from `bg`. Returns `(x, y, w, h)`.
    pub fn calc_min_max_rect(&self, bg: Color) -> Option<(u32, u32, u32, u32)> {
        if self.channel_count != 4 {
            return None;
        }
        let bg_bytes = [bg.r(), bg.g(), bg.b(), bg.a()];
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        for y in 0..self.height {
            for x in 0..self.width {
                let p = self.pixel(x, y);
                if p != bg_bytes {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if max_x < min_x {
            None
        } else {
            Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
        }
    }

    /// Find the bounding rect of pixels in one channel differing from `bg_val`.
    pub fn calc_channel_min_max_rect(&self, ch: u8, bg_val: u8) -> Option<(u32, u32, u32, u32)> {
        let cc = self.channel_count as usize;
        let ch = ch as usize;
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        for y in 0..self.height {
            for x in 0..self.width {
                let off = (y as usize * self.width as usize + x as usize) * cc + ch;
                if self.data[off] != bg_val {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if max_x < min_x {
            None
        } else {
            Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
        }
    }

    /// Find the bounding rect of non-zero alpha pixels. Requires 2 or 4 channels.
    pub fn calc_alpha_min_max_rect(&self) -> Option<(u32, u32, u32, u32)> {
        let alpha_ch = match self.channel_count {
            2 => 1,
            4 => 3,
            _ => return None,
        };
        self.calc_channel_min_max_rect(alpha_ch, 0)
    }

    /// Sample a pixel using bilinear interpolation, returning `bg` for
    /// out-of-bounds coordinates. `x` and `y` are in source pixel coordinates.
    /// `w` and `h` describe the sampling footprint (unused for bilinear; reserved
    /// for future area-sampling support). Requires a 4-channel image.
    pub fn get_pixel_interpolated(&self, x: f64, y: f64, _w: f64, _h: f64, bg: Color) -> Color {
        if self.is_empty() {
            return bg;
        }
        // If the sample center is entirely outside source bounds, return bg.
        if x < -0.5 || y < -0.5 || x >= self.width as f64 - 0.5 || y >= self.height as f64 - 0.5 {
            return bg;
        }
        sample_bilinear(self, x, y, ImageExtension::Clamp)
    }

    /// Apply an affine transformation from `src` into a region of `self`.
    ///
    /// `x`, `y`, `w`, `h` define the target clip rectangle (in `self` pixel
    /// coords).  `matrix` is a 2×3 affine mapping **source → target**:
    ///
    /// ```text
    /// target_x = matrix[0]*src_x + matrix[1]*src_y + matrix[2]
    /// target_y = matrix[3]*src_x + matrix[4]*src_y + matrix[5]
    /// ```
    ///
    /// The matrix is inverted internally so that each target pixel can be
    /// mapped back to source coordinates.
    ///
    /// * `interpolate` – `true` for bilinear sampling, `false` for nearest.
    /// * `bg_color` – used for source samples that fall outside the source
    ///   image bounds.
    ///
    /// Both `self` and `src` must be 4-channel RGBA images.
    pub fn copy_transformed(
        &mut self,
        clip: (i32, i32, i32, i32),
        matrix: &[f64; 6],
        src: &Image,
        interpolate: bool,
        bg_color: Color,
    ) {
        let (x, y, w, h) = clip;
        assert_eq!(
            self.channel_count, 4,
            "copy_transformed() requires a 4-channel target image"
        );
        assert_eq!(
            src.channel_count, 4,
            "copy_transformed() requires a 4-channel source image"
        );

        if w <= 0 || h <= 0 || self.is_empty() {
            return;
        }

        // Invert the source→target affine matrix to get target→source.
        //
        //   | a  b  c |        | a  b |
        //   | d  e  f |   M =  | d  e |
        //
        // inv(M) = (1/det) *  |  e  -b |
        //                     | -d   a |
        let a = matrix[0];
        let b = matrix[1];
        let c = matrix[2];
        let d = matrix[3];
        let e = matrix[4];
        let f = matrix[5];

        let det = a * e - b * d;
        if det.abs() < 1e-15 {
            // Degenerate (singular) matrix — fill with bg.
            let bg_bytes = [bg_color.r(), bg_color.g(), bg_color.b(), bg_color.a()];
            for py in y..y + h {
                for px in x..x + w {
                    if px >= 0 && py >= 0 && (px as u32) < self.width && (py as u32) < self.height {
                        self.pixel_mut(px as u32, py as u32)
                            .copy_from_slice(&bg_bytes);
                    }
                }
            }
            return;
        }

        let inv_det = 1.0 / det;
        let ia = e * inv_det;
        let ib = -b * inv_det;
        let id = -d * inv_det;
        let ie = a * inv_det;
        // Inverted translation: inv_M * (-t)
        let ic = -(ia * c + ib * f);
        let ifc = -(id * c + ie * f);

        let src_w = src.width as f64;
        let src_h = src.height as f64;

        for py in y..y + h {
            if py < 0 || (py as u32) >= self.height {
                continue;
            }
            for px in x..x + w {
                if px < 0 || (px as u32) >= self.width {
                    continue;
                }

                let tx = px as f64;
                let ty = py as f64;

                // Map target pixel back to source coordinates.
                let sx = ia * tx + ib * ty + ic;
                let sy = id * tx + ie * ty + ifc;

                let color = if interpolate {
                    // Bilinear sampling with bounds check.
                    if sx < -0.5 || sy < -0.5 || sx >= src_w - 0.5 || sy >= src_h - 0.5 {
                        bg_color
                    } else {
                        sample_bilinear(src, sx, sy, ImageExtension::Clamp)
                    }
                } else {
                    // Nearest-neighbor with bounds check.
                    let ix = sx.round() as i32;
                    let iy = sy.round() as i32;
                    if ix < 0 || iy < 0 || ix >= src.width as i32 || iy >= src.height as i32 {
                        bg_color
                    } else {
                        let p = src.pixel(ix as u32, iy as u32);
                        Color::rgba(p[0], p[1], p[2], p[3])
                    }
                };

                let dst = self.pixel_mut(px as u32, py as u32);
                dst[0] = color.r();
                dst[1] = color.g();
                dst[2] = color.b();
                dst[3] = color.a();
            }
        }
    }

    /// Collect all unique colors, sorted by packed u32 value. 4-channel only.
    pub fn determine_all_colors_sorted(&self) -> Vec<Color> {
        assert_eq!(
            self.channel_count, 4,
            "determine_all_colors_sorted() requires a 4-channel image"
        );
        let mut set = BTreeSet::new();
        for chunk in self.data.chunks_exact(4) {
            let packed = (chunk[0] as u32) << 24
                | (chunk[1] as u32) << 16
                | (chunk[2] as u32) << 8
                | chunk[3] as u32;
            set.insert(packed);
        }
        set.into_iter()
            .map(|v| Color::rgba((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zero_filled() {
        let img = Image::new(4, 4, 4);
        assert!(img.data().iter().all(|&b| b == 0));
        assert_eq!(img.data().len(), 4 * 4 * 4);
    }

    #[test]
    fn pixel_access() {
        let mut img = Image::new(2, 2, 3);
        let p = img.pixel_mut(1, 0);
        p[0] = 10;
        p[1] = 20;
        p[2] = 30;
        assert_eq!(img.pixel(1, 0), &[10, 20, 30]);
    }

    #[test]
    fn fill_rgba() {
        let mut img = Image::new(3, 2, 4);
        img.fill(Color::RED);
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(img.pixel(x, y), &[255, 0, 0, 255]);
            }
        }
    }

    #[test]
    #[should_panic(expected = "channel_count must be 1, 2, 3, or 4")]
    fn invalid_channel_count() {
        Image::new(1, 1, 0);
    }

    #[test]
    #[should_panic(expected = "fill() requires a 4-channel image")]
    fn fill_non_rgba() {
        let mut img = Image::new(1, 1, 3);
        img.fill(Color::BLACK);
    }

    #[test]
    fn from_raw_valid() {
        let data = vec![10, 20, 30, 255, 40, 50, 60, 128];
        let img = Image::from_raw(2, 1, 4, data);
        assert_eq!(img.pixel(0, 0), &[10, 20, 30, 255]);
        assert_eq!(img.pixel(1, 0), &[40, 50, 60, 128]);
    }

    #[test]
    #[should_panic(expected = "does not match")]
    fn from_raw_wrong_length() {
        Image::from_raw(2, 2, 4, vec![0; 15]);
    }

    #[test]
    fn single_channel() {
        let mut img = Image::new(2, 2, 1);
        img.pixel_mut(0, 0)[0] = 128;
        assert_eq!(img.pixel(0, 0), &[128]);
        assert_eq!(img.pixel(1, 0), &[0]);
    }

    #[test]
    fn partial_eq() {
        let a = Image::new(2, 2, 4);
        let b = Image::new(2, 2, 4);
        assert_eq!(a, b);
        let c = Image::new(3, 2, 4);
        assert_ne!(a, c);
    }

    #[test]
    fn setup_and_clear() {
        let mut img = Image::new(4, 4, 4);
        img.fill(Color::RED);
        img.setup(2, 3, 1);
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 3);
        assert_eq!(img.channel_count(), 1);
        assert!(img.data().iter().all(|&b| b == 0));

        img.clear();
        assert_eq!(img.width(), 0);
        assert_eq!(img.height(), 0);
        assert!(img.is_empty());
    }

    #[test]
    fn pixel_channel_round_trip() {
        let mut img = Image::new(3, 3, 4);
        img.set_pixel_channel(1, 2, 2, 42);
        assert_eq!(img.get_pixel_channel(1, 2, 2), 42);
        assert_eq!(img.get_pixel_channel(1, 2, 0), 0);
    }

    #[test]
    fn fill_rect_region_isolation() {
        let mut img = Image::new(4, 4, 4);
        img.fill_rect(1, 1, 2, 2, Color::RED);
        // Inside rect
        assert_eq!(img.pixel(1, 1), &[255, 0, 0, 255]);
        assert_eq!(img.pixel(2, 2), &[255, 0, 0, 255]);
        // Outside rect
        assert_eq!(img.pixel(0, 0), &[0, 0, 0, 0]);
        assert_eq!(img.pixel(3, 3), &[0, 0, 0, 0]);
    }

    #[test]
    fn fill_channel_works() {
        let mut img = Image::new(2, 2, 3);
        img.fill_channel(1, 128);
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(img.pixel(x, y), &[0, 128, 0]);
            }
        }
    }

    #[test]
    fn fill_channel_rect_clips() {
        let mut img = Image::new(3, 3, 1);
        img.fill_channel_rect(0, 1, 1, 100, 100, 77);
        assert_eq!(img.get_pixel_channel(0, 0, 0), 0);
        assert_eq!(img.get_pixel_channel(1, 1, 0), 77);
        assert_eq!(img.get_pixel_channel(2, 2, 0), 77);
    }

    #[test]
    fn copy_from_correctness() {
        let mut src = Image::new(2, 2, 4);
        src.fill(Color::rgb(10, 20, 30));
        let mut dst = Image::new(4, 4, 4);
        dst.copy_from(1, 1, &src);
        assert_eq!(dst.pixel(0, 0), &[0, 0, 0, 0]);
        assert_eq!(dst.pixel(1, 1), &[10, 20, 30, 255]);
        assert_eq!(dst.pixel(2, 2), &[10, 20, 30, 255]);
        assert_eq!(dst.pixel(3, 3), &[0, 0, 0, 0]);
    }

    #[test]
    fn get_cropped_extraction() {
        let mut img = Image::new(4, 4, 1);
        img.set_pixel_channel(2, 1, 0, 99);
        let sub = img.get_cropped(1, 1, 2, 2);
        assert_eq!(sub.width(), 2);
        assert_eq!(sub.height(), 2);
        assert_eq!(sub.get_pixel_channel(1, 0, 0), 99);
    }

    #[test]
    fn get_converted_1_to_4() {
        let mut img = Image::new(1, 1, 1);
        img.pixel_mut(0, 0)[0] = 128;
        let rgba = img.get_converted(4);
        assert_eq!(rgba.pixel(0, 0), &[128, 128, 128, 255]);
    }

    #[test]
    fn get_converted_4_to_1() {
        let mut img = Image::new(1, 1, 4);
        img.pixel_mut(0, 0).copy_from_slice(&[30, 60, 90, 255]);
        let grey = img.get_converted(1);
        assert_eq!(grey.pixel(0, 0), &[60]); // (30+60+90)/3 = 60
    }

    #[test]
    fn get_converted_3_to_4() {
        let mut img = Image::new(1, 1, 3);
        img.pixel_mut(0, 0).copy_from_slice(&[10, 20, 30]);
        let rgba = img.get_converted(4);
        assert_eq!(rgba.pixel(0, 0), &[10, 20, 30, 255]);
    }

    #[test]
    fn get_converted_4_to_3() {
        let mut img = Image::new(1, 1, 4);
        img.pixel_mut(0, 0).copy_from_slice(&[10, 20, 30, 128]);
        let rgb = img.get_converted(3);
        assert_eq!(rgb.pixel(0, 0), &[10, 20, 30]);
    }

    #[test]
    fn has_any_non_grey_pixel_detects() {
        let mut img = Image::new(2, 1, 3);
        img.pixel_mut(0, 0).copy_from_slice(&[50, 50, 50]);
        img.pixel_mut(1, 0).copy_from_slice(&[50, 50, 50]);
        assert!(!img.has_any_non_grey_pixel());
        img.pixel_mut(1, 0)[1] = 51;
        assert!(img.has_any_non_grey_pixel());
    }

    #[test]
    fn has_any_transparent_pixel_detects() {
        let mut img = Image::new(2, 1, 4);
        img.fill(Color::rgb(0, 0, 0)); // all alpha=255
        assert!(!img.has_any_transparent_pixel());
        img.set_pixel_channel(1, 0, 3, 254);
        assert!(img.has_any_transparent_pixel());
    }

    #[test]
    fn calc_min_max_rect_basic() {
        let mut img = Image::new(4, 4, 4);
        img.fill(Color::BLACK);
        img.pixel_mut(1, 1).copy_from_slice(&[255, 0, 0, 255]);
        img.pixel_mut(2, 2).copy_from_slice(&[0, 255, 0, 255]);
        let r = img.calc_min_max_rect(Color::BLACK).unwrap();
        assert_eq!(r, (1, 1, 2, 2));
    }

    #[test]
    fn calc_min_max_rect_all_bg() {
        let mut img = Image::new(3, 3, 4);
        img.fill(Color::WHITE);
        assert_eq!(img.calc_min_max_rect(Color::WHITE), None);
    }

    #[test]
    fn calc_alpha_min_max_rect_works() {
        let mut img = Image::new(4, 4, 4);
        // All alpha=0 initially
        img.set_pixel_channel(2, 1, 3, 255);
        img.set_pixel_channel(3, 3, 3, 128);
        let r = img.calc_alpha_min_max_rect().unwrap();
        assert_eq!(r, (2, 1, 2, 3));
    }

    #[test]
    fn get_cropped_by_alpha_works() {
        let mut img = Image::new(4, 4, 4);
        img.set_pixel_channel(1, 1, 3, 255);
        img.pixel_mut(1, 1).copy_from_slice(&[10, 20, 30, 255]);
        let cropped = img.get_cropped_by_alpha();
        assert_eq!(cropped.width(), 1);
        assert_eq!(cropped.height(), 1);
        assert_eq!(cropped.pixel(0, 0), &[10, 20, 30, 255]);
    }

    #[test]
    fn determine_all_colors_sorted_works() {
        let mut img = Image::new(3, 1, 4);
        img.pixel_mut(0, 0).copy_from_slice(&[0, 0, 255, 255]); // blue
        img.pixel_mut(1, 0).copy_from_slice(&[255, 0, 0, 255]); // red
        img.pixel_mut(2, 0).copy_from_slice(&[0, 0, 255, 255]); // blue dup
        let colors = img.determine_all_colors_sorted();
        assert_eq!(colors.len(), 2);
        // Red (0xFF0000FF) > Blue (0x0000FFFF) in packed u32
        assert_eq!(colors[0], Color::rgb(0, 0, 255));
        assert_eq!(colors[1], Color::rgb(255, 0, 0));
    }

    #[test]
    fn copy_channel_cross_cc() {
        let mut src = Image::new(2, 2, 1);
        src.pixel_mut(0, 0)[0] = 42;
        src.pixel_mut(1, 1)[0] = 99;
        let mut dst = Image::new(3, 3, 4);
        dst.copy_channel(2, 0, 0, &src, 0); // copy src ch0 into dst ch2 (blue)
        assert_eq!(dst.get_pixel_channel(0, 0, 2), 42);
        assert_eq!(dst.get_pixel_channel(1, 1, 2), 99);
        assert_eq!(dst.get_pixel_channel(0, 0, 0), 0); // red untouched
    }

    #[test]
    fn get_pixel_interpolated_in_bounds() {
        let mut img = Image::new(2, 2, 4);
        img.fill(Color::RED);
        let c = img.get_pixel_interpolated(0.0, 0.0, 1.0, 1.0, Color::BLUE);
        assert_eq!(c.r(), 255);
        assert_eq!(c.g(), 0);
        assert_eq!(c.b(), 0);
    }

    #[test]
    fn get_pixel_interpolated_out_of_bounds() {
        let mut img = Image::new(2, 2, 4);
        img.fill(Color::RED);
        let c = img.get_pixel_interpolated(-1.0, -1.0, 1.0, 1.0, Color::BLUE);
        assert_eq!(c, Color::BLUE);
    }

    #[test]
    fn get_pixel_interpolated_empty_image() {
        let img = Image::new(0, 0, 4);
        let c = img.get_pixel_interpolated(0.0, 0.0, 1.0, 1.0, Color::GREEN);
        assert_eq!(c, Color::GREEN);
    }

    #[test]
    fn copy_transformed_identity() {
        // Identity matrix: target == source coords
        let mut src = Image::new(4, 4, 4);
        src.fill(Color::RED);
        src.pixel_mut(1, 1).copy_from_slice(&[0, 255, 0, 255]);

        let mut dst = Image::new(4, 4, 4);
        // Identity: target_x = 1*src_x + 0*src_y + 0, target_y = 0*src_x + 1*src_y + 0
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        dst.copy_transformed((0, 0, 4, 4), &identity, &src, false, Color::BLACK);

        assert_eq!(dst.pixel(0, 0), &[255, 0, 0, 255]);
        assert_eq!(dst.pixel(1, 1), &[0, 255, 0, 255]);
        assert_eq!(dst.pixel(3, 3), &[255, 0, 0, 255]);
    }

    #[test]
    fn copy_transformed_translation() {
        // Translate source by (+2, +1)
        let mut src = Image::new(2, 2, 4);
        src.fill(Color::rgb(10, 20, 30));

        let mut dst = Image::new(6, 6, 4);
        // target_x = src_x + 2, target_y = src_y + 1
        let translate = [1.0, 0.0, 2.0, 0.0, 1.0, 1.0];
        dst.copy_transformed((0, 0, 6, 6), &translate, &src, false, Color::BLACK);

        // Source pixel (0,0) maps to target (2,1)
        assert_eq!(dst.pixel(2, 1), &[10, 20, 30, 255]);
        assert_eq!(dst.pixel(3, 2), &[10, 20, 30, 255]);
        // Outside source -> bg
        assert_eq!(dst.pixel(0, 0), &[0, 0, 0, 255]);
    }

    #[test]
    fn copy_transformed_scale() {
        // Scale 2x: target_x = 2*src_x, target_y = 2*src_y
        let mut src = Image::new(2, 2, 4);
        src.pixel_mut(0, 0).copy_from_slice(&[255, 0, 0, 255]);
        src.pixel_mut(1, 0).copy_from_slice(&[0, 255, 0, 255]);
        src.pixel_mut(0, 1).copy_from_slice(&[0, 0, 255, 255]);
        src.pixel_mut(1, 1).copy_from_slice(&[255, 255, 0, 255]);

        let mut dst = Image::new(4, 4, 4);
        let scale = [2.0, 0.0, 0.0, 0.0, 2.0, 0.0];
        dst.copy_transformed((0, 0, 4, 4), &scale, &src, false, Color::BLACK);

        // Source (0,0) maps to target (0,0); nearest for (0,0) -> src(0,0)
        assert_eq!(dst.pixel(0, 0), &[255, 0, 0, 255]);
        // Source (1,0) maps to target (2,0); nearest for (2,0) -> src(1,0)
        assert_eq!(dst.pixel(2, 0), &[0, 255, 0, 255]);
    }

    #[test]
    fn copy_transformed_interpolated() {
        // Simple identity with interpolation
        let mut src = Image::new(2, 2, 4);
        src.fill(Color::rgb(100, 100, 100));

        let mut dst = Image::new(2, 2, 4);
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        dst.copy_transformed((0, 0, 2, 2), &identity, &src, true, Color::BLACK);

        // With bilinear on a uniform image, result should be same
        assert_eq!(dst.pixel(0, 0), &[100, 100, 100, 255]);
        assert_eq!(dst.pixel(1, 1), &[100, 100, 100, 255]);
    }

    #[test]
    fn copy_transformed_clips_target() {
        // Clip rectangle smaller than target
        let mut src = Image::new(4, 4, 4);
        src.fill(Color::RED);

        let mut dst = Image::new(4, 4, 4);
        dst.fill(Color::BLACK);
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        // Only transform the top-left 2x2 region
        dst.copy_transformed((0, 0, 2, 2), &identity, &src, false, Color::BLUE);

        assert_eq!(dst.pixel(0, 0), &[255, 0, 0, 255]); // transformed
        assert_eq!(dst.pixel(1, 1), &[255, 0, 0, 255]); // transformed
        assert_eq!(dst.pixel(2, 2), &[0, 0, 0, 255]); // untouched
        assert_eq!(dst.pixel(3, 3), &[0, 0, 0, 255]); // untouched
    }

    #[test]
    fn copy_transformed_singular_matrix() {
        // Degenerate matrix (all zeros) should fill with bg color
        let src = Image::new(2, 2, 4);
        let mut dst = Image::new(4, 4, 4);
        let singular = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        dst.copy_transformed((1, 1, 2, 2), &singular, &src, false, Color::rgb(42, 42, 42));

        assert_eq!(dst.pixel(1, 1), &[42, 42, 42, 255]);
        assert_eq!(dst.pixel(2, 2), &[42, 42, 42, 255]);
        assert_eq!(dst.pixel(0, 0), &[0, 0, 0, 0]); // outside clip
    }

    #[test]
    fn copy_transformed_zero_size_noop() {
        let src = Image::new(2, 2, 4);
        let mut dst = Image::new(4, 4, 4);
        dst.fill(Color::WHITE);
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        // Zero-width clip should be a no-op
        dst.copy_transformed((0, 0, 0, 4), &identity, &src, false, Color::BLACK);
        assert_eq!(dst.pixel(0, 0), &[255, 255, 255, 255]);
    }
}
