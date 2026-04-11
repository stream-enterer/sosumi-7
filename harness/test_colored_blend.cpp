// Layer 6 test: Colored blend (IMAGE_COLORED / font glyph pipeline)
// Compares C++ PaintScanlineIntG2 against Rust blend_colored_scanline.
//
// Sets up a ScanlineTool with IMAGE_COLORED texture (color1=TRANSPARENT,
// color2=some_color), feeds it known glyph luminance values, and compares
// the blended output pixel by pixel.

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>
#include "emPainter_ScTl.h"

extern "C" void rust_blend_colored_g2(
    unsigned char* dest,
    const unsigned char* lums,
    int count,
    const int* coverage,  // null for full coverage
    unsigned char color2_r, unsigned char color2_g, unsigned char color2_b, unsigned char color2_a,
    unsigned char canvas_r, unsigned char canvas_g, unsigned char canvas_b, unsigned char canvas_a
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
    // Test parameters matching HowTo text rendering:
    // color1 = TRANSPARENT, color2 = fg_color with alpha ~166 (0.65 * 255)
    // canvas = gray background (128,128,128)
    // Source: 1-channel grayscale glyph data

    // Create a 1-channel "glyph" image with known luminance values
    const int GW = 32, GH = 32;
    emImage glyphImg;
    glyphImg.Setup(GW, GH, 1);
    unsigned char* gmap = (unsigned char*)glyphImg.GetMap();
    // Fill with gradient: luminance from 0 to 255
    for (int y = 0; y < GH; y++) {
        for (int x = 0; x < GW; x++) {
            gmap[y * GW + x] = (unsigned char)((x * 255) / (GW - 1));
        }
    }

    // Canvas setup
    const int CW = 200, CH = 100;
    emImage canvas;
    canvas.Setup(CW, CH, 4);
    // Fill with gray background
    unsigned char* cmap = (unsigned char*)canvas.GetMap();
    for (int i = 0; i < CW * CH; i++) {
        cmap[i*4+0] = 128; // R
        cmap[i*4+1] = 128; // G
        cmap[i*4+2] = 128; // B
        cmap[i*4+3] = 255; // A
    }

    // Setup painter
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

    // Create IMAGE_COLORED texture:
    // color1 = TRANSPARENT, color2 = (0, 0, 0, 166) — dark text
    // This matches: PaintScanlineIntG2 variant
    unsigned char c2r = 0, c2g = 0, c2b = 0, c2a = 166;

    // Place glyph at (0.1, 0.1) with size (0.3, 0.3)
    // In pixels: (10, 10) to (40, 40), 30x30 pixels from 32x32 source
    emImageColoredTexture tex(
        0.1, 0.1, 0.3, 0.3,
        glyphImg, 0, 0, GW, GH,
        emColor(0,0,0,0),           // color1 = transparent
        emColor(c2r, c2g, c2b, c2a), // color2 = dark with alpha
        emTexture::EXTEND_ZERO,
        emTexture::DQ_3X3,
        emTexture::UQ_AREA_SAMPLING
    );

    // Use canvas color = gray (128,128,128,255) for CVC path
    emPainter::ScanlineTool sct(p);
    if (!sct.Init(tex, emColor(128,128,128,255))) {
        printf("Init failed (CVC path)\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        return 1;
    }

    printf("Init OK. Channels=%d\n", sct.GetChannels());

    // Test a row of pixels: paint with C++, then compare against Rust
    int test_row = 20; // middle of glyph area
    int test_x = 10;
    int test_w = 30;

    // Save canvas before C++ paint
    unsigned char* canvas_backup = (unsigned char*)malloc(CW * CH * 4);
    memcpy(canvas_backup, canvas.GetMap(), CW * CH * 4);

    // C++ paint
    sct.CallPaintScanline(test_x, test_row, test_w, 0x1000, 0x1000, 0x1000);

    // Read C++ result
    unsigned char* cpp_row = (unsigned char*)canvas.GetMap() + test_row * CW * 4;

    // Now do Rust side: first get interpolation buffer to extract luminances
    sct.CallInterpolate(test_x, test_row, test_w);
    const unsigned char* interp_buf = sct.GetInterpolationBuffer();

    // Extract luminances from interpolation buffer (1-channel)
    unsigned char lums[256];
    for (int i = 0; i < test_w; i++) {
        lums[i] = interp_buf[i]; // 1-channel: just the first byte per pixel
    }

    printf("Lum samples: [0]=%d [5]=%d [15]=%d [25]=%d [29]=%d\n",
           lums[0], lums[5], lums[15], lums[25], lums[29]);

    // Prepare Rust dest (copy of canvas backup for this row)
    unsigned char rust_row[256 * 4];
    memcpy(rust_row, canvas_backup + test_row * CW * 4 + test_x * 4, test_w * 4);

    // Call Rust colored blend (G2, canvas=gray)
    rust_blend_colored_g2(
        rust_row,
        lums,
        test_w,
        NULL, // full coverage
        c2r, c2g, c2b, c2a,
        128, 128, 128, 255  // canvas = gray
    );

    // Compare
    int mismatches = 0;
    int max_diff = 0;
    for (int x = 0; x < test_w; x++) {
        for (int ch = 0; ch < 3; ch++) { // RGB only
            int ci = test_x * 4 + x * 4 + ch;
            int ri = x * 4 + ch;
            int d = abs((int)cpp_row[ci] - (int)rust_row[ri]);
            if (d > 0) {
                if (mismatches < 20) {
                    printf("  x=%d ch=%d cpp=%d rust=%d diff=%+d lum=%d\n",
                           test_x + x, ch, cpp_row[ci], rust_row[ri],
                           (int)rust_row[ri] - (int)cpp_row[ci], lums[x]);
                }
                mismatches++;
                if (d > max_diff) max_diff = d;
            }
        }
    }

    printf("\n=== COLORED BLEND (G2, CVC) ===\n");
    if (mismatches == 0) {
        printf("PASS: All %d RGB values match.\n", test_w * 3);
    } else {
        printf("FAIL: %d mismatches, max_diff=%d.\n", mismatches, max_diff);
    }

    // Also test source-over (no canvas) path
    // Restore canvas to all-zero (transparent)
    memset((void*)canvas.GetMap(), 0, CW * CH * 4);

    emPainter::ScanlineTool sct2(p);
    if (!sct2.Init(tex, 0)) {
        printf("Init failed (source-over path)\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        free(canvas_backup);
        return mismatches > 0 ? 1 : 0;
    }

    sct2.CallPaintScanline(test_x, test_row, test_w, 0x1000, 0x1000, 0x1000);
    unsigned char* cpp_row2 = (unsigned char*)canvas.GetMap() + test_row * CW * 4;

    // Rust side: source-over on transparent canvas
    unsigned char rust_row2[256 * 4];
    memset(rust_row2, 0, test_w * 4);

    // Get interp buffer from sct2
    sct2.CallInterpolate(test_x, test_row, test_w);
    const unsigned char* interp2 = sct2.GetInterpolationBuffer();
    for (int i = 0; i < test_w; i++) {
        lums[i] = interp2[i];
    }

    rust_blend_colored_g2(
        rust_row2,
        lums,
        test_w,
        NULL,
        c2r, c2g, c2b, c2a,
        0, 0, 0, 0  // canvas = transparent (source-over)
    );

    int mismatches2 = 0;
    int max_diff2 = 0;
    for (int x = 0; x < test_w; x++) {
        for (int ch = 0; ch < 3; ch++) {
            int ci = test_x * 4 + x * 4 + ch;
            int ri = x * 4 + ch;
            int d = abs((int)cpp_row2[ci] - (int)rust_row2[ri]);
            if (d > 0) {
                if (mismatches2 < 20) {
                    printf("  x=%d ch=%d cpp=%d rust=%d diff=%+d lum=%d\n",
                           test_x + x, ch, cpp_row2[ci], rust_row2[ri],
                           (int)rust_row2[ri] - (int)cpp_row2[ci], lums[x]);
                }
                mismatches2++;
                if (d > max_diff2) max_diff2 = d;
            }
        }
    }

    printf("\n=== COLORED BLEND (G2, source-over) ===\n");
    if (mismatches2 == 0) {
        printf("PASS: All %d RGB values match.\n", test_w * 3);
    } else {
        printf("FAIL: %d mismatches, max_diff=%d.\n", mismatches2, max_diff2);
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    free(canvas_backup);
    return (mismatches + mismatches2) > 0 ? 1 : 0;
}
