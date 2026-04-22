// SPLIT: Split from emRes.h — TGA resource loader extracted
//
// DIVERGED: (language-forced) C++ emRes loads TGA files from disk via emFileStream and the
// emImageFile plugin system (emTgaImageFileModel in src/emTga/). Rust embeds
// TGA data via include_bytes!() and decodes directly from &[u8]. No emFileStream
// dependency — toolkit assets are compiled into the binary as part of emBorder
// initialization. Outside-emCore consumers (emTga, emBmp, emGif, emJpeg, emPnm,
// emXpm) that load image files from disk via emFileStream will need a file-based
// TGA loader when ported — this &[u8] decoder does not serve that use case.

use super::emImage::emImage;
use std::fmt;

#[derive(Debug)]
pub enum TgaError {
    TooShort,
    UnsupportedType(u8),
    BadRle,
}

impl fmt::Display for TgaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "TGA data too short"),
            Self::UnsupportedType(t) => write!(f, "unsupported TGA image type {t}"),
            Self::BadRle => write!(f, "malformed RLE data"),
        }
    }
}

impl std::error::Error for TgaError {}

/// Load a TGA image from raw bytes.
///
/// Supports only the subset used by emCore toolkit assets:
/// - Type 10: RLE true-color (32-bit BGRA → RGBA)
/// - Type 11: RLE grayscale (8-bit → 1-channel)
/// - Top-origin only (all toolkit TGAs are top-origin)
pub fn load_tga(data: &[u8]) -> Result<emImage, TgaError> {
    if data.len() < 18 {
        return Err(TgaError::TooShort);
    }

    let id_len = data[0] as usize;
    let image_type = data[2];
    let width = u16::from_le_bytes([data[12], data[13]]) as u32;
    let height = u16::from_le_bytes([data[14], data[15]]) as u32;
    let bpp = data[16];

    let (channels, src_bpp) = match (image_type, bpp) {
        (10, 32) => (4u8, 4usize),
        (11, 8) => (1, 1),
        _ => return Err(TgaError::UnsupportedType(image_type)),
    };

    let pixel_count = width as usize * height as usize;
    let mut pixels = Vec::with_capacity(pixel_count * channels as usize);
    let mut cursor = 18 + id_len;

    while pixels.len() < pixel_count * channels as usize {
        if cursor >= data.len() {
            return Err(TgaError::BadRle);
        }
        let header = data[cursor];
        cursor += 1;
        let count = (header & 0x7F) as usize + 1;

        if header & 0x80 != 0 {
            // RLE packet: one pixel repeated `count` times
            if cursor + src_bpp > data.len() {
                return Err(TgaError::BadRle);
            }
            let pix = &data[cursor..cursor + src_bpp];
            cursor += src_bpp;
            for _ in 0..count {
                push_pixel(&mut pixels, pix, channels);
            }
        } else {
            // Raw packet: `count` literal pixels
            if cursor + count * src_bpp > data.len() {
                return Err(TgaError::BadRle);
            }
            for i in 0..count {
                let pix = &data[cursor + i * src_bpp..cursor + (i + 1) * src_bpp];
                push_pixel(&mut pixels, pix, channels);
            }
            cursor += count * src_bpp;
        }
    }

    Ok(emImage::from_raw(width, height, channels, pixels))
}

#[inline]
fn push_pixel(pixels: &mut Vec<u8>, pix: &[u8], channels: u8) {
    if channels == 4 {
        // BGRA → RGBA
        pixels.extend_from_slice(&[pix[2], pix[1], pix[0], pix[3]]);
    } else {
        pixels.push(pix[0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tga_header(image_type: u8, width: u16, height: u16, bpp: u8) -> Vec<u8> {
        let mut h = vec![0u8; 18];
        h[2] = image_type;
        h[12..14].copy_from_slice(&width.to_le_bytes());
        h[14..16].copy_from_slice(&height.to_le_bytes());
        h[16] = bpp;
        h
    }

    #[test]
    fn rle_truecolor_2x1() {
        let mut data = make_tga_header(10, 2, 1, 32);
        // RLE packet: 2 pixels of BGRA(0x10, 0x20, 0x30, 0xFF)
        data.push(0x81); // RLE, count=2
        data.extend_from_slice(&[0x10, 0x20, 0x30, 0xFF]);
        let img = load_tga(&data).unwrap();
        assert_eq!(img.GetWidth(), 2);
        assert_eq!(img.GetHeight(), 1);
        assert_eq!(img.GetChannelCount(), 4);
        // Should be RGBA
        assert_eq!(img.GetPixel(0, 0), &[0x30, 0x20, 0x10, 0xFF]);
        assert_eq!(img.GetPixel(1, 0), &[0x30, 0x20, 0x10, 0xFF]);
    }

    #[test]
    fn raw_grayscale_3x1() {
        let mut data = make_tga_header(11, 3, 1, 8);
        // Raw packet: 3 pixels
        data.push(0x02); // raw, count=3
        data.extend_from_slice(&[100, 150, 200]);
        let img = load_tga(&data).unwrap();
        assert_eq!(img.GetChannelCount(), 1);
        assert_eq!(img.GetPixel(0, 0), &[100]);
        assert_eq!(img.GetPixel(1, 0), &[150]);
        assert_eq!(img.GetPixel(2, 0), &[200]);
    }

    #[test]
    fn too_short() {
        assert!(load_tga(&[0; 10]).is_err());
    }

    #[test]
    fn unsupported_type() {
        let data = make_tga_header(2, 1, 1, 24); // uncompressed true-color
        assert!(matches!(load_tga(&data), Err(TgaError::UnsupportedType(2))));
    }
}
