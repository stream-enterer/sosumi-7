//! Eagle Mode compatible grayscale font atlas.
//!
//! Loads the BasicLatin TGA glyph atlas (ASCII 0x20–0x7F) — the same
//! 128×224-per-cell grayscale data used by C++ `emFontCache`. Each pixel
//! is an 8-bit anti-aliased coverage value (0 = transparent, 255 = fully
//! opaque foreground).

use crate::emCore::emImage::emImage;
use std::sync::OnceLock;

const TGA_DATA: &[u8] =
    include_bytes!("../../res/fonts/00020-0007F_128x224_BasicLatin_original.tga");

/// Width of each glyph cell in the atlas (pixels).
pub(crate) const CHAR_WIDTH: u32 = 128;
/// Height of each glyph cell in the atlas (pixels).
pub(crate) const CHAR_HEIGHT: u32 = 224;
/// First Unicode codepoint in the atlas.
const FIRST_CODE: u32 = 0x20;
/// Last Unicode codepoint in the atlas (inclusive).
const LAST_CODE: u32 = 0x7F;
/// Number of glyph columns in the atlas image.
const COLUMN_COUNT: u32 = 16;

/// Height-to-width ratio matching C++ `emPainter::CharBoxTallness`.
pub(crate) const CHAR_BOX_TALLNESS: f64 = 1.77;

static ATLAS: OnceLock<emImage> = OnceLock::new();

// DIVERGED: Acquire — Rust uses a global OnceLock instead of emModel/emRef
/// Returns a reference to the decoded font atlas (single-channel grayscale).
pub(crate) fn atlas() -> &'static emImage {
    ATLAS.get_or_init(|| decode_tga_rle_grayscale(TGA_DATA))
}

/// Get glyph source coordinates within the atlas.
/// Returns `(src_x, src_y, src_w, src_h)`.
pub(crate) fn GetChar(ch: char) -> (u32, u32, u32, u32) {
    let cp = ch as u32;
    let index = if (FIRST_CODE..=LAST_CODE).contains(&cp) {
        cp - FIRST_CODE
    } else {
        b'?' as u32 - FIRST_CODE
    };
    let src_x = (index % COLUMN_COUNT) * CHAR_WIDTH;
    let src_y = (index / COLUMN_COUNT) * CHAR_HEIGHT;
    (src_x, src_y, CHAR_WIDTH, CHAR_HEIGHT)
}

/// Decode a TGA type 11 (RLE-compressed grayscale, 8bpp) into a
/// single-channel `emImage`.
fn decode_tga_rle_grayscale(data: &[u8]) -> emImage {
    assert!(data.len() >= 18, "TGA data too short");

    let id_len = data[0] as usize;
    let image_type = data[2];
    assert_eq!(
        image_type, 11,
        "Expected TGA type 11 (RLE grayscale), got {image_type}"
    );

    let width = u16::from_le_bytes([data[12], data[13]]) as u32;
    let height = u16::from_le_bytes([data[14], data[15]]) as u32;
    let bpp = data[16];
    assert_eq!(bpp, 8, "Expected 8bpp, got {bpp}");

    let descriptor = data[17];
    let top_to_bottom = (descriptor & 0x20) != 0;

    let total_pixels = (width * height) as usize;
    let mut pixels = Vec::with_capacity(total_pixels);
    let mut pos = 18 + id_len;

    while pixels.len() < total_pixels && pos < data.len() {
        let header = data[pos];
        pos += 1;
        let count = (header & 0x7F) as usize + 1;

        if header & 0x80 != 0 {
            // RLE packet: repeat one value.
            let value = data[pos];
            pos += 1;
            pixels.resize(pixels.len() + count, value);
        } else {
            // Raw packet: copy values.
            pixels.extend_from_slice(&data[pos..pos + count]);
            pos += count;
        }
    }

    assert_eq!(
        pixels.len(),
        total_pixels,
        "Decoded {}, expected {total_pixels}",
        pixels.len()
    );

    if !top_to_bottom {
        // Flip scanlines from bottom-to-top to top-to-bottom.
        let w = width as usize;
        let mut flipped = Vec::with_capacity(total_pixels);
        for row in (0..height as usize).rev() {
            flipped.extend_from_slice(&pixels[row * w..(row + 1) * w]);
        }
        pixels = flipped;
    }

    emImage::from_raw(width, height, 1, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_dimensions() {
        let img = atlas();
        assert_eq!(img.width(), 2048);
        assert_eq!(img.height(), 1344);
        assert_eq!(img.channel_count(), 1);
    }

    #[test]
    fn space_is_blank() {
        let img = atlas();
        let (sx, sy, sw, sh) = GetChar(' ');
        for dy in 0..sh {
            for dx in 0..sw {
                assert_eq!(
                    img.pixel(sx + dx, sy + dy)[0],
                    0,
                    "space glyph non-zero at ({dx},{dy})"
                );
            }
        }
    }

    #[test]
    fn letter_a_has_pixels() {
        let img = atlas();
        let (sx, sy, sw, sh) = GetChar('A');
        let any_set = (0..sh)
            .flat_map(|dy| (0..sw).map(move |dx| (dx, dy)))
            .any(|(dx, dy)| img.pixel(sx + dx, sy + dy)[0] > 0);
        assert!(any_set, "'A' glyph should have non-zero pixels");
    }

    #[test]
    fn fallback_to_question_mark() {
        let q = GetChar('?');
        let unknown = GetChar('\u{FFFF}');
        assert_eq!(q, unknown);
    }
}
