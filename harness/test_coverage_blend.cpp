// Test: compare C++ vs Rust coverage+blend for a single pixel.
// C++ applies opacity to each channel: hR[(s[0]*o+0x800)>>12]
// Rust applies coverage to alpha only: adjusted_a = (src_a * cov + 0x800) >> 12
// Then blends with adjusted_a.
//
// If these differ, we've found the Group A+B root cause.

#include <cstdio>
#include <cstdlib>

// Rust function
extern "C" void rust_blend_source_over_simple(
    unsigned char* dest, const unsigned char* src, int count
);

// C++ blend hash (unshifted, range=255)
static unsigned char cpp_hash[65536];
static void init_hash() {
    int range = 255;
    for (int a1 = 0; a1 < 128; a1++) {
        int c1 = (a1 * range + 127) / 255;
        for (int a2 = 0; a2 < 128; a2++) {
            int c2 = (a2 * range + 127) / 255;
            int c3 = (a1 * a2 * range + 32512) / 65025;
            cpp_hash[(a1 << 8) + a2] = (unsigned char)c3;
            cpp_hash[(a1 << 8) + (255 - a2)] = (unsigned char)(c1 - c3);
            cpp_hash[((255 - a1) << 8) + a2] = (unsigned char)(c2 - c3);
            cpp_hash[((255 - a1) << 8) + (255 - a2)] = (unsigned char)(range + c3 - c1 - c2);
        }
    }
}
static inline unsigned char h(unsigned char color, unsigned char alpha) {
    return cpp_hash[(color << 8) + alpha];
}

// C++ PaintScanlineInt blend for non-GC, non-CVC, CHANNELS=4, PIXEL_SIZE=4.
// opacity `o` applied to each channel and alpha separately.
// Lines 313-322 of emPainter_ScTlPSInt.cpp (no HAVE_GC, no HAVE_CVC, CHANNELS=4).
static void cpp_blend_with_opacity(
    unsigned char* dest,        // 4 bytes RGBA
    const unsigned char* src,   // 4 bytes: interpolated premul RGBA
    int o                       // opacity 0..0x1000
) {
    // C++ line 315: a = (a * o + 0x800) >> 12
    unsigned a = (src[3] * o + 0x800) >> 12;
    if (!a) return;

    // C++ lines 318-321: pix = hR[(s[0]*o+0x800)>>12] + hG[...] + hB[...]
    // For unshifted range=255: hR[x] = x (identity), so pix_r = (s[0]*o+0x800)>>12
    unsigned pr = (src[0] * o + 0x800) >> 12;
    unsigned pg = (src[1] * o + 0x800) >> 12;
    unsigned pb = (src[2] * o + 0x800) >> 12;

    if (a >= 255) {
        // C++ line 355: *p = pix (direct write)
        dest[0] = (unsigned char)pr;
        dest[1] = (unsigned char)pg;
        dest[2] = (unsigned char)pb;
        // C++ writes alpha=0 here but golden tests don't check alpha
        return;
    }

    // C++ lines 369-378: source-over with Blinn div255
    unsigned t = (255 - a) * 257;
    dest[0] = (unsigned char)(((dest[0] * t + 0x8073) >> 16) + pr);
    dest[1] = (unsigned char)(((dest[1] * t + 0x8073) >> 16) + pg);
    dest[2] = (unsigned char)(((dest[2] * t + 0x8073) >> 16) + pb);
}

// Rust premul path: scale ALL channels by o_eff, then blend.
// This matches blend_scanline_premul_source_over (lines 377-410).
static void rust_premul_blend_with_coverage(
    unsigned char* dest,
    const unsigned char* src,
    int o  // coverage/opacity 0..0x1000
) {
    unsigned char pm[4];
    if (o >= 0x1000) {
        pm[0] = src[0]; pm[1] = src[1]; pm[2] = src[2]; pm[3] = src[3];
    } else if (o > 0) {
        pm[0] = (unsigned char)((src[0] * o + 0x800) >> 12);
        pm[1] = (unsigned char)((src[1] * o + 0x800) >> 12);
        pm[2] = (unsigned char)((src[2] * o + 0x800) >> 12);
        pm[3] = (unsigned char)((src[3] * o + 0x800) >> 12);
    } else {
        return;
    }

    unsigned a = pm[3];
    if (a == 0) return;

    if (a >= 255) {
        dest[0] = pm[0]; dest[1] = pm[1]; dest[2] = pm[2]; dest[3] = 255;
        return;
    }

    unsigned t = (255 - a) * 257;
    dest[0] = (unsigned char)(((dest[0] * t + 0x8073) >> 16) + pm[0]);
    dest[1] = (unsigned char)(((dest[1] * t + 0x8073) >> 16) + pm[1]);
    dest[2] = (unsigned char)(((dest[2] * t + 0x8073) >> 16) + pm[2]);
    dest[3] = (unsigned char)(((dest[3] * t + 0x8073) >> 16) + a);
}

int main() {
    init_hash();

    int mismatches = 0;
    int total = 0;

    // Test: sweep opacity/coverage from 1 to 0xFFF, with various src pixels
    unsigned char test_pixels[][4] = {
        {128, 64, 32, 200},   // typical premul RGBA
        {255, 128, 0, 255},   // fully opaque
        {50, 100, 150, 180},  // mid-range
        {10, 10, 10, 12},     // very transparent
        {200, 200, 200, 220}, // near-white
        {0, 0, 0, 128},       // black with half alpha
    };

    for (auto& src : test_pixels) {
        for (int o = 1; o <= 0x1000; o++) {
            unsigned char cpp_dest[4] = {180, 180, 180, 255};
            unsigned char rust_dest[4] = {180, 180, 180, 255};

            cpp_blend_with_opacity(cpp_dest, src, o);
            rust_premul_blend_with_coverage(rust_dest, src, o);

            total++;
            int max_diff = 0;
            for (int ch = 0; ch < 3; ch++) {
                int d = abs(cpp_dest[ch] - rust_dest[ch]);
                if (d > max_diff) max_diff = d;
            }
            if (max_diff > 0) {
                if (mismatches < 20) {
                    printf("MISMATCH: src=(%d,%d,%d,%d) o=%d -> cpp=(%d,%d,%d) rust=(%d,%d,%d) diff=%d\n",
                           src[0], src[1], src[2], src[3], o,
                           cpp_dest[0], cpp_dest[1], cpp_dest[2],
                           rust_dest[0], rust_dest[1], rust_dest[2],
                           max_diff);
                }
                mismatches++;
            }
        }
    }

    if (mismatches == 0) {
        printf("PASS: All %d coverage+blend values match.\n", total);
    } else {
        printf("FAIL: %d mismatches out of %d.\n", mismatches, total);
    }
    return mismatches > 0 ? 1 : 0;
}
