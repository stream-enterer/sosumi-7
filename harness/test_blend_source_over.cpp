// Test: compare C++ source-over compositing against Rust for synthetic pixel data.
// C++ formula from emPainter_ScTlPSInt.cpp, non-GC non-CVC CHANNELS=4 PIXEL_SIZE=4 path.
// Full opacity (o >= 0x1000), so opacity scaling is identity.

#include <cstdio>
#include <cstring>

extern "C" void rust_blend_source_over_simple(
    unsigned char* dest, const unsigned char* src, int count
);

// C++ blend hash table (unshifted, range=255)
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

static inline unsigned char hash_lookup(unsigned char color, unsigned char alpha) {
    return cpp_hash[(color << 8) + alpha];
}

// C++ source-over blend for RGBA, full coverage, no canvas.
// From emPainter_ScTlPSInt.cpp with HAVE_GC1=0, HAVE_GC2=0, HAVE_CVC=0,
// CHANNELS=4, PIXEL_SIZE=4, o >= 0x1000 (full coverage).
static void cpp_blend_source_over(
    unsigned char* dest, const unsigned char* src, int count
) {
    for (int i = 0; i < count; i++) {
        int s_off = i * 4;
        int d_off = i * 4;

        unsigned char sr = src[s_off];
        unsigned char sg = src[s_off + 1];
        unsigned char sb = src[s_off + 2];
        unsigned char sa = src[s_off + 3];

        if (sa == 0) continue;

        if (sa >= 255) {
            dest[d_off] = sr;
            dest[d_off + 1] = sg;
            dest[d_off + 2] = sb;
            dest[d_off + 3] = 255;
            continue;
        }

        // C++: Pixel pix = hR[s[0]] + hG[s[1]] + hB[s[2]];
        // But since range=255 and shift=0 for our test, hR[x] = hash_lookup(255, x)
        // Wait — the C++ hash is indexed by (color*256 + alpha). For the non-GC case,
        // hR points to hash row 255 (hR = hash + 255*256), so hR[s[0]] = hash[255*256 + s[0]].
        // This is hash_lookup(255, s[0]) — which for range=255 is just s[0].
        //
        // Actually, this is the IMAGE (non-colored) path. The interpolated buffer
        // already contains the final RGBA. The paint scanline just composites.
        //
        // For CHANNELS=4 non-GC: a = s[3] (alpha from interpolated pixel)
        // Since o >= 0x1000 (full coverage), no opacity scaling.
        // pix = hR[s[0]] + hG[s[1]] + hB[s[2]]  (premul color)
        // But hR/hG/hB for non-GC point to row 255, and range=255, so hash(255, x) = x.
        //
        // Hmm, that means the C++ just reads s[0..3] directly as a packed pixel when
        // the source is already premul RGBA. The compositing is:
        //   if (a >= 255) { *p = pix; }  // opaque fast path
        //   else {
        //     t = (255 - a) * 257;
        //     *p = (dest_r * t + 0x8073) >> 16 << rsh + ... + pix;
        //   }
        //
        // For RGBA with no shifts, this simplifies to per-channel:
        //   dest[ch] = (dest[ch] * t + 0x8073) >> 16 + src[ch]

        unsigned alpha = sa;
        unsigned t = (255 - alpha) * 257;
        dest[d_off]     = (unsigned char)(((dest[d_off]     * t + 0x8073) >> 16) + hash_lookup(sr, alpha));
        dest[d_off + 1] = (unsigned char)(((dest[d_off + 1] * t + 0x8073) >> 16) + hash_lookup(sg, alpha));
        dest[d_off + 2] = (unsigned char)(((dest[d_off + 2] * t + 0x8073) >> 16) + hash_lookup(sb, alpha));
        dest[d_off + 3] = (unsigned char)(((dest[d_off + 3] * t + 0x8073) >> 16) + hash_lookup(255, alpha));
    }
}

int main() {
    init_hash();

    // Test with a variety of src/dest combinations
    const int N = 256;
    unsigned char cpp_dest[N * 4];
    unsigned char rust_dest[N * 4];
    unsigned char src[N * 4];

    int total_tests = 0;
    int total_mismatches = 0;

    // Test: sweep alpha from 0-255, with various RGB values
    for (int test_rgb = 0; test_rgb < 4; test_rgb++) {
        unsigned char r, g, b;
        switch (test_rgb) {
            case 0: r = 128; g = 64;  b = 32;  break;
            case 1: r = 255; g = 0;   b = 0;   break;
            case 2: r = 0;   g = 255; b = 128; break;
            case 3: r = 1;   g = 1;   b = 1;   break;
        }

        // Initialize src with premultiplied values
        for (int i = 0; i < N; i++) {
            unsigned char a = (unsigned char)i;
            // Premultiply
            src[i * 4]     = (unsigned char)((r * a + 127) / 255);
            src[i * 4 + 1] = (unsigned char)((g * a + 127) / 255);
            src[i * 4 + 2] = (unsigned char)((b * a + 127) / 255);
            src[i * 4 + 3] = a;
        }

        // Initialize dest with some background
        for (int i = 0; i < N * 4; i++) {
            cpp_dest[i] = rust_dest[i] = (unsigned char)(200 - (i % 7) * 20);
        }

        cpp_blend_source_over(cpp_dest, src, N);
        rust_blend_source_over_simple(rust_dest, src, N);

        for (int i = 0; i < N * 4; i++) {
            total_tests++;
            if (cpp_dest[i] != rust_dest[i]) {
                if (total_mismatches < 10) {
                    printf("MISMATCH: test_rgb=%d pixel=%d ch=%d cpp=%d rust=%d\n",
                           test_rgb, i / 4, i % 4, cpp_dest[i], rust_dest[i]);
                }
                total_mismatches++;
            }
        }
    }

    if (total_mismatches == 0) {
        printf("PASS: All %d compositing values match across %d test configs.\n",
               total_tests, 4);
    } else {
        printf("FAIL: %d mismatches out of %d.\n", total_mismatches, total_tests);
    }
    return total_mismatches > 0 ? 1 : 0;
}
