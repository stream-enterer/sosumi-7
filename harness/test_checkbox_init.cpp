// Compare C++ ScanlineTool::Init derived state against Rust computation
// for widget_checkbox_unchecked border image sections.
//
// Uses the actual libemCore.so Init via the harness-modified emPainter.

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>
#include "emPainter_ScTl.h"

// From the EM_DUMP_INIT output, a representative border section for the
// checkbox test (first ImgW=143 entry at ScaleX=800):
//
// INIT_STATE: Ch=4 ImgW=143 ImgH=143 ImgDX=8 ImgDY=4736 ImgSX=1144 ImgSY=677248
//   TX=322272790 TY=322272790 TDX=30925166 TDY=30925166 ODX=35554 ODY=35554
//   Alpha=255
//   tex_x=0.013026 tex_y=0.013026 tex_w=0.096974 tex_h=0.096974
//   ScaleX=800.000000 ScaleY=800.000000 OriginX=0.000000 OriginY=0.000000
//
// This section uses a 286x286 image (group_border.tga, 4ch RGBA).
// The stride reduction halved it: 286->143, DX: 4->8.

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
        void* hash = ch == 0 ? pf.RedHash : ch == 1 ? pf.GreenHash : pf.BlueHash;
        int shift = ch == 0 ? pf.RedShift : ch == 1 ? pf.GreenShift : pf.BlueShift;
        for (int a1 = 0; a1 < 128; a1++) {
            int c1 = (a1 * range + 127) / 255;
            for (int a2 = 0; a2 < 128; a2++) {
                int c2 = (a2 * range + 127) / 255;
                int c3 = (a1 * a2 * range + 32512) / 65025;
                ((unsigned*)hash)[(a1 << 8) + a2] = c3 << shift;
                ((unsigned*)hash)[(a1 << 8) + (255 - a2)] = (c1 - c3) << shift;
                ((unsigned*)hash)[((255 - a1) << 8) + a2] = (c2 - c3) << shift;
                ((unsigned*)hash)[((255 - a1) << 8) + (255 - a2)] = (range + c3 - c1 - c2) << shift;
            }
        }
    }
    pf.OPFIndex = emPainter::OPFI_8888_0BGR;
}

int main() {
    // Create a 286x286 RGBA image (simulating group_border.tga)
    emImage srcImg;
    srcImg.Setup(286, 286, 4);
    // Fill with a gradient pattern so pixels are non-trivial
    unsigned char* map = (unsigned char*)srcImg.GetMap();
    for (int y = 0; y < 286; y++) {
        for (int x = 0; x < 286; x++) {
            int off = (y * 286 + x) * 4;
            map[off] = (unsigned char)(x % 256);
            map[off+1] = (unsigned char)(y % 256);
            map[off+2] = (unsigned char)((x + y) % 256);
            map[off+3] = 255;
        }
    }

    // Set up painter matching golden test: 800x600 canvas
    emImage canvas;
    canvas.Setup(800, 600, 4);
    canvas.Fill(emColor::WHITE);

    emPainter p;
    static emPainter::SharedPixelFormat pf;
    setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap();
    p.BytesPerRow = 800 * 4;
    p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0;
    p.ClipX2 = 800; p.ClipY2 = 600;
    p.OriginX = 0; p.OriginY = 0;
    p.ScaleX = 800.0; p.ScaleY = 800.0;
    p.UserSpaceMutex = NULL;
    p.USMLockedByThisThread = NULL;

    // Fake model (zero-filled, CanCpuDoAvx2=false)
    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    void* fake_ptr = (void*)fake_model;
    memcpy(&p.Model, &fake_ptr, sizeof(void*));

    // Test each of the 9-slice corner sections from the dump.
    // Top-left corner: tex_x=0.013026, tex_y=0.013026, tex_w=0.096974, tex_h=0.096974
    // Full image source rect: srcX=0, srcY=0, srcW=286, srcH=286
    // For the corner section: srcX=0, srcY=0, srcW=143, srcH=143 (left half)
    // But C++ computes this from the 9-slice subdivision, not directly.
    // Let's use the texture params from the dump directly.

    struct TestCase {
        const char* name;
        double tex_x, tex_y, tex_w, tex_h;
        int srcX, srcY, srcW, srcH;
    };

    // From the dump, the first few entries with ImgW=143 at ScaleX=800:
    TestCase cases[] = {
        {"corner_TL", 0.013026, 0.013026, 0.096974, 0.096974, 0, 0, 286, 286},
        {"corner_TR", 0.890000, 0.013026, 0.096974, 0.096974, 0, 0, 286, 286},
        {"edge_L",    0.013026, 0.110000, 0.096974, 0.530000, 0, 0, 286, 286},
    };

    for (auto& tc : cases) {
        printf("=== %s ===\n", tc.name);

        emImageTexture tex(tc.tex_x, tc.tex_y, tc.tex_w, tc.tex_h,
                           srcImg, tc.srcX, tc.srcY, tc.srcW, tc.srcH, 255,
                           emTexture::EXTEND_EDGE,
                           emTexture::DQ_3X3,
                           emTexture::UQ_AREA_SAMPLING);

        emPainter::ScanlineTool sct(p);
        bool ok = sct.Init(tex, emColor::WHITE);

        if (!ok) {
            printf("  Init returned false\n");
            continue;
        }

        printf("  Channels = %d\n", sct.GetChannels());
        printf("  ImgW     = %d\n", sct.GetImgW());
        printf("  ImgH     = %d\n", sct.GetImgH());
        printf("  ImgDX    = %zd\n", sct.GetImgDX());
        printf("  ImgDY    = %zd\n", sct.GetImgDY());
        printf("  ImgSX    = %zd\n", sct.GetImgSX());
        printf("  ImgSY    = %zd\n", sct.GetImgSY());
        printf("  TX       = %lld\n", (long long)sct.GetTX());
        printf("  TY       = %lld\n", (long long)sct.GetTY());
        printf("  TDX      = %lld\n", (long long)sct.GetTDX());
        printf("  TDY      = %lld\n", (long long)sct.GetTDY());
        printf("  ODX      = %u\n", sct.GetODX());
        printf("  ODY      = %u\n", sct.GetODY());
        printf("  Alpha    = %d\n", sct.GetAlpha());
    }

    // Clean up fake model ref
    void* null_ptr = nullptr;
    memcpy(&p.Model, &null_ptr, sizeof(void*));
    return 0;
}
