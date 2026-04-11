// Layer 7 test: Adaptive luminance interpolation for font glyphs.
// Compares C++ InterpolateImageAdaptive (CHANNELS=1) against
// Rust sample_adaptive_lum_section.
//
// Uses a ScanlineTool with IMAGE_COLORED texture to get C++ interpolation,
// then calls the Rust function with the same transform parameters.

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>
#include "emPainter_ScTl.h"

extern "C" unsigned char rust_sample_adaptive_lum(
    const unsigned char* img_data,
    int img_w, int img_h,
    int ix, int iy,
    unsigned ox, unsigned oy,
    int sec_ox, int sec_oy, int sec_w, int sec_h
);

static void setup_pixel_format(emPainter::SharedPixelFormat& pf) {
    memset(&pf, 0, sizeof(pf));
    pf.BytesPerPixel = 4;
    pf.RedRange = 255; pf.GreenRange = 255; pf.BlueRange = 255;
    pf.RedShift = 0; pf.GreenShift = 8; pf.BlueShift = 16;
    pf.RedHash = malloc(256 * 256 * 4);
    pf.GreenHash = malloc(256 * 256 * 4);
    pf.BlueHash = malloc(256 * 256 * 4);
    int range = 255;
    for (int ch = 0; ch < 3; ch++) {
        void* hash = ch==0?pf.RedHash:ch==1?pf.GreenHash:pf.BlueHash;
        int shift = ch==0?pf.RedShift:ch==1?pf.GreenShift:pf.BlueShift;
        for (int a1 = 0; a1 < 128; a1++) {
            int c1 = (a1*range+127)/255;
            for (int a2 = 0; a2 < 128; a2++) {
                int c2 = (a2*range+127)/255;
                int c3 = (a1*a2*range+32512)/65025;
                ((unsigned*)hash)[(a1<<8)+a2] = c3<<shift;
                ((unsigned*)hash)[(a1<<8)+(255-a2)] = (c1-c3)<<shift;
                ((unsigned*)hash)[((255-a1)<<8)+a2] = (c2-c3)<<shift;
                ((unsigned*)hash)[((255-a1)<<8)+(255-a2)] = (range+c3-c1-c2)<<shift;
            }
        }
    }
    pf.OPFIndex = emPainter::OPFI_8888_0BGR;
}

int main() {
    // Create a 1-channel glyph image with realistic patterns
    const int GW = 32, GH = 32;
    emImage glyphImg;
    glyphImg.Setup(GW, GH, 1);
    unsigned char* gmap = (unsigned char*)glyphImg.GetMap();

    // Pattern: circle-like shape (simulates a glyph)
    for (int y = 0; y < GH; y++) {
        for (int x = 0; x < GW; x++) {
            double dx = (x - 16.0) / 16.0;
            double dy = (y - 16.0) / 16.0;
            double d = sqrt(dx*dx + dy*dy);
            gmap[y * GW + x] = d < 0.8 ? (unsigned char)(255 * (1.0 - d / 0.8)) : 0;
        }
    }

    const int CW = 200, CH = 100;
    emImage canvas;
    canvas.Setup(CW, CH, 4);
    canvas.Fill(emColor(128, 128, 128));

    emPainter p;
    static emPainter::SharedPixelFormat pf;
    setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap();
    p.BytesPerRow = CW * 4;
    p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0;
    p.ClipX2 = CW; p.ClipY2 = CH;
    p.OriginX = 0; p.OriginY = 0;
    p.ScaleX = 100.0; p.ScaleY = 100.0;
    p.UserSpaceMutex = NULL;
    p.USMLockedByThisThread = NULL;
    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    void* fm = (void*)fake_model;
    memcpy(&p.Model, &fm, sizeof(void*));

    // IMAGE_COLORED texture: upscaling 32x32 → ~30x30 pixel dest
    // (ratio ~0.94, but with bilinear/adaptive upscaling)
    // Actually make it definitely upscaling: 32x32 → 60x60 pixels
    emImageColoredTexture tex(
        0.1, 0.1, 0.6, 0.6,  // dest: (10,10) to (70,70) = 60x60 pixels
        glyphImg, 0, 0, GW, GH,
        emColor(0,0,0,0),
        emColor(0, 0, 0, 166),
        emTexture::EXTEND_ZERO,
        emTexture::DQ_3X3,
        emTexture::UQ_AREA_SAMPLING
    );

    emPainter::ScanlineTool sct(p);
    if (!sct.Init(tex, emColor(128,128,128,255))) {
        printf("Init failed\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        return 1;
    }

    printf("Init OK. Channels=%d ImgW=%d ImgH=%d\n",
           sct.GetChannels(), sct.GetImgW(), sct.GetImgH());
    printf("TDX=%lld TDY=%lld TX=%lld TY=%lld\n",
           (long long)sct.GetTDX(), (long long)sct.GetTDY(),
           (long long)sct.GetTX(), (long long)sct.GetTY());

    // C++ interpolation of a row
    int px = 10, py = 10; // SubPixelEdges ix1, iy1
    int total_mismatches = 0;
    int total_bytes = 0;
    int max_diff = 0;

    for (int row = 12; row < 68; row += 4) {
        int x_start = 12;
        int w = 56;

        sct.CallInterpolate(x_start, row, w);
        const unsigned char* cpp_buf = sct.GetInterpolationBuffer();

        // For each pixel, compute the Rust adaptive lum
        for (int i = 0; i < w; i++) {
            int dest_x = x_start + i;

            // Compute source coordinates matching C++ PaintImageColored upscaling:
            // tx64 = (dest_x - px) * tdx + base_x - 0x180_0000
            // ty64 = (row - py) * tdy + base_y - 0x180_0000
            long long tdx = sct.GetTDX();
            long long tdy = sct.GetTDY();
            long long tx_base = sct.GetTX();
            long long ty_base = sct.GetTY();

            long long tx64 = (long long)(dest_x - px) * tdx + tx_base - 0x1800000LL;
            long long ty64 = (long long)(row - py) * tdy + ty_base - 0x1800000LL;

            int src_ix = (int)(tx64 >> 24);
            int src_iy = (int)(ty64 >> 24);
            unsigned ox = (unsigned)(((unsigned)(tx64 & 0xFFFFFF) + 0x7FFF) >> 16);
            unsigned oy = (unsigned)(((unsigned)(ty64 & 0xFFFFFF) + 0x7FFF) >> 16);

            unsigned char rust_lum = rust_sample_adaptive_lum(
                gmap, GW, GH,
                src_ix, src_iy,
                ox, oy,
                0, 0, GW, GH
            );

            unsigned char cpp_lum = cpp_buf[i]; // 1-channel: luminance is byte 0

            total_bytes++;
            int d = abs((int)cpp_lum - (int)rust_lum);
            if (d > 0) {
                if (total_mismatches < 20) {
                    printf("  row=%d x=%d cpp=%d rust=%d diff=%+d ix=%d iy=%d ox=%u oy=%u\n",
                           row, dest_x, cpp_lum, rust_lum,
                           (int)rust_lum - (int)cpp_lum,
                           src_ix, src_iy, ox, oy);
                }
                total_mismatches++;
                if (d > max_diff) max_diff = d;
            }
        }
    }

    printf("\n=== ADAPTIVE LUM INTERPOLATION ===\n");
    if (total_mismatches == 0) {
        printf("PASS: All %d luminance values match.\n", total_bytes);
    } else {
        printf("FAIL: %d mismatches out of %d, max_diff=%d.\n",
               total_mismatches, total_bytes, max_diff);
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    return total_mismatches > 0 ? 1 : 0;
}
