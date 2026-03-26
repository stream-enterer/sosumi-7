// SPLIT: Split from emPainter.h — AVX2 scanline blend extracted
//! AVX2-accelerated scanline blend functions.
//!
//! Processes 4 RGBA pixels per iteration using 256-bit SIMD.
//! Each unsafe fn is annotated with `#[target_feature(enable = "avx2")]`.
//! Callers gate on `is_x86_feature_detected!("avx2")` (cached by stdlib).
//!
//! Blend math uses the SIMD-friendly div255 identity:
//!   div255(x) = (x + 128 + ((x + 128) >> 8)) >> 8
//! which is equivalent to the Blinn formula `(x * 257 + 0x8073) >> 16`
//! for x in [0, 65025] and keeps all arithmetic in u16.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SIMD div255: `(x + 128 + ((x + 128) >> 8)) >> 8` for 16 packed u16 values.
/// Input x must be in [0, 65025] (i.e., product of two u8 values).
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn div255_epi16(x: __m256i) -> __m256i {
    let c128 = _mm256_set1_epi16(128);
    let tmp = _mm256_add_epi16(x, c128);
    let tmp_hi = _mm256_srli_epi16(tmp, 8);
    _mm256_srli_epi16(_mm256_add_epi16(tmp, tmp_hi), 8)
}

/// Shuffle mask: broadcast alpha word to all 4 RGBA positions within each pixel.
/// After cvtepu8_epi16, 128-bit lane layout is [R0 G0 B0 A0 R1 G1 B1 A1].
/// A0 is at byte offsets 6-7, A1 is at byte offsets 14-15.
/// Output: [A0 A0 A0 A0 A1 A1 A1 A1] — same mask works for both lanes.
#[cfg(target_arch = "x86_64")]
const ALPHA_BROADCAST_MASK: [i8; 32] = [
    6, 7, 6, 7, 6, 7, 6, 7, 14, 15, 14, 15, 14, 15, 14, 15, // lane 0
    6, 7, 6, 7, 6, 7, 6, 7, 14, 15, 14, 15, 14, 15, 14, 15, // lane 1
];

/// AVX2 premul source-over blend for 4 premultiplied RGBA pixels.
///
/// For RGB: `out[c] = div255(dst[c] * (255-pm_a)) + pm[c]`
/// For A:   `out[a] = div255(dst[a] * (255-pm_a)) + pm_a`
///
/// # Safety
/// Same as `blend_source_over_avx2`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn blend_premul_source_over_avx2(dest: &mut [u8], src: &[u8], count: usize) {
    let c255 = _mm256_set1_epi16(255);
    let alpha_shuf = _mm256_loadu_si256(ALPHA_BROADCAST_MASK.as_ptr() as *const __m256i);

    let chunks = count / 4;
    let remainder = count % 4;

    for chunk in 0..chunks {
        let off = chunk * 16;
        let src_ptr = src.as_ptr().add(off);
        let dest_ptr = dest.as_mut_ptr().add(off);

        let src_128 = _mm_loadu_si128(src_ptr as *const __m128i);
        let src_16 = _mm256_cvtepu8_epi16(src_128);
        let alpha_16 = _mm256_shuffle_epi8(src_16, alpha_shuf);

        // Opaque fast path
        let opaque_cmp = _mm256_cmpeq_epi16(alpha_16, c255);
        if _mm256_movemask_epi8(opaque_cmp) == -1i32 {
            _mm_storeu_si128(dest_ptr as *mut __m128i, src_128);
            continue;
        }

        // Transparent fast path
        let zero = _mm256_setzero_si256();
        let zero_cmp = _mm256_cmpeq_epi16(alpha_16, zero);
        if _mm256_movemask_epi8(zero_cmp) == -1i32 {
            continue;
        }

        // Full blend
        let dst_128 = _mm_loadu_si128(dest_ptr as *const __m128i);
        let dst_16 = _mm256_cvtepu8_epi16(dst_128);

        let inv_alpha = _mm256_sub_epi16(c255, alpha_16);

        // dest_term = div255(dst_16 * inv_alpha)
        let d_prod = _mm256_mullo_epi16(dst_16, inv_alpha);
        let d_term = div255_epi16(d_prod);

        // result = dest_term + src (premultiplied, so just add directly)
        let result_16 = _mm256_add_epi16(d_term, src_16);

        let packed = _mm256_packus_epi16(result_16, zero);
        let lo = _mm256_castsi256_si128(packed);
        let hi = _mm256_extracti128_si256(packed, 1);
        let result_128 = _mm_unpacklo_epi64(lo, hi);

        _mm_storeu_si128(dest_ptr as *mut __m128i, result_128);
    }

    let base = chunks * 4;
    blend_remainder_premul_source_over(&mut dest[base * 4..], &src[base * 4..], remainder);
}

/// Scalar blend for 0-3 remainder pixels (premul source-over).
#[inline]
fn blend_remainder_premul_source_over(dest: &mut [u8], src: &[u8], count: usize) {
    for p in 0..count {
        let po = p * 4;
        let a = src[po + 3] as u32;
        if a == 0 {
            continue;
        }
        if a >= 255 {
            dest[po..po + 4].copy_from_slice(&src[po..po + 4]);
            continue;
        }
        let t = (255 - a) * 257;
        dest[po] = (((dest[po] as u32 * t + 0x8073) >> 16) + src[po] as u32) as u8;
        dest[po + 1] = (((dest[po + 1] as u32 * t + 0x8073) >> 16) + src[po + 1] as u32) as u8;
        dest[po + 2] = (((dest[po + 2] as u32 * t + 0x8073) >> 16) + src[po + 2] as u32) as u8;
        dest[po + 3] = (((dest[po + 3] as u32 * t + 0x8073) >> 16) + a) as u8;
    }
}

#[cfg(test)]
#[cfg(target_arch = "x86_64")]
mod tests {
    use super::*;

    fn ref_blend_premul_source_over(dest: &mut [u8], src: &[u8], count: usize) {
        blend_remainder_premul_source_over(dest, src, count);
    }

    #[test]
    fn avx2_premul_source_over_matches_scalar() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        // Premul pixels
        let src = vec![
            200, 0, 0, 200,   // premul red
            0, 100, 0, 128,   // premul green
            0, 0, 0, 0,       // transparent
            255, 255, 255, 255, // opaque white
            50, 25, 10, 64,   // low alpha premul
            0, 0, 0, 0,       // transparent
            128, 128, 128, 128, // half premul
            10, 20, 30, 40,   // low alpha
        ];
        let bg = vec![100u8; 32];

        let mut dest_avx = bg.clone();
        let mut dest_ref = bg;
        unsafe { blend_premul_source_over_avx2(&mut dest_avx, &src, 8) };
        ref_blend_premul_source_over(&mut dest_ref, &src, 8);
        for (i, (a, r)) in dest_avx.iter().zip(dest_ref.iter()).enumerate() {
            assert!(
                (*a as i16 - *r as i16).abs() <= 1,
                "byte {} mismatch: avx={} ref={}",
                i,
                a,
                r
            );
        }
    }

}
