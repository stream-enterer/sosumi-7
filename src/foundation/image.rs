use super::Color;

/// CPU bitmap image with 1–4 channels per pixel.
#[derive(Clone, Debug)]
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
    fn single_channel() {
        let mut img = Image::new(2, 2, 1);
        img.pixel_mut(0, 0)[0] = 128;
        assert_eq!(img.pixel(0, 0), &[128]);
        assert_eq!(img.pixel(1, 0), &[0]);
    }
}
