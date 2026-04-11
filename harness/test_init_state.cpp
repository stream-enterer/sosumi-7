// Test: construct ScanlineTool from scratch, call Init, dump derived state.
// Bypasses emContext by setting emPainter fields directly.

#include <cstdio>
#include <cstring>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>

// Private header for ScanlineTool definition
#include "emPainter_ScTl.h"

int main() {
    // Create a canvas image (256x256 RGBA)
    emImage canvas;
    canvas.Setup(256, 256, 4);
    canvas.Fill(emColor::WHITE);

    // Create an emPainter and set its fields directly.
    // Normally this is done by PreparePainter, but we bypass emContext.
    emPainter p;
    p.Map = (void*)canvas.GetMap();
    p.BytesPerRow = 256 * 4;
    p.ClipX1 = 0; p.ClipY1 = 0;
    p.ClipX2 = 256; p.ClipY2 = 256;
    p.OriginX = 0; p.OriginY = 0;
    p.ScaleX = 1.0; p.ScaleY = 1.0;
    p.UserSpaceMutex = NULL;
    p.USMLockedByThisThread = NULL;

    // PixelFormat: allocate manually for RGBA (4bpp, range=255, shifts 24/16/8)
    // Matching the standard 0RGB layout that emImage uses.
    static emPainter::SharedPixelFormat pf;
    static bool pf_inited = false;
    if (!pf_inited) {
        memset(&pf, 0, sizeof(pf));
        pf.BytesPerPixel = 4;
        pf.RedRange = 255; pf.GreenRange = 255; pf.BlueRange = 255;
        // emImage RGBA layout: R at byte 0 (shift 0), G at byte 1 (shift 8), B at byte 2 (shift 16)
        // Actually need to check emImage's actual layout...
        // For now use shifts that match a common layout:
        pf.RedShift = 0; pf.GreenShift = 8; pf.BlueShift = 16;
        // Allocate hash tables (256*256 entries of 4 bytes each)
        pf.RedHash = malloc(256 * 256 * 4);
        pf.GreenHash = malloc(256 * 256 * 4);
        pf.BlueHash = malloc(256 * 256 * 4);
        // Fill hash tables using the same formula from emPainter.cpp:209-234
        for (int ch = 0; ch < 3; ch++) {
            void* hash = ch == 0 ? pf.RedHash : ch == 1 ? pf.GreenHash : pf.BlueHash;
            int range = 255;
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
        pf.OPFIndex = (emPainter::OptimizedPixelFormatIndex)0; // OPFI_8888_0BGR
        pf_inited = true;
    }
    p.PixelFormat = &pf;

    // Model stub: Init reads Model->CanCpuDoAvx2 (and if true, Model->CoreConfig).
    // We need a fake SharedModel where CanCpuDoAvx2 = false.
    // SharedModel inherits from emModel (deep hierarchy), but we only need the
    // memory to be dereferenceable with CanCpuDoAvx2 = false.
    // Allocate a zero-filled buffer large enough and set the emRef to point to it.
    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    // emRef<SharedModel> is essentially a pointer. Set it via memcpy.
    // emRef stores a T* internally. On this platform, that's 8 bytes.
    {
        void* fake_ptr = (void*)fake_model;
        memcpy(&p.Model, &fake_ptr, sizeof(void*));
    }

    // Create a source image (64x64 RGBA, simulating a border section)
    emImage srcImg;
    srcImg.Setup(64, 64, 4);
    srcImg.Fill(emColor(128, 64, 32, 255));

    // Create texture: PaintImage(x, y, w, h, img, alpha) internally creates
    // emImageTexture(img, x, y, w, h, alpha).
    // Let's use the inline PaintImage signature from emPainter.h to understand
    // how texture is constructed.
    //
    // Actually, we need the emTexture with IMAGE type. Let me check how
    // PaintImage constructs it.

    printf("emPainter fields set.\n");
    printf("  Map=%p PixelFormat=%p Model=%p\n", p.Map, (void*)p.PixelFormat, *(void**)&p.Model);
    fflush(stdout);

    printf("Constructing ScanlineTool...\n"); fflush(stdout);
    emPainter::ScanlineTool sct(p);
    printf("ScanlineTool constructed.\n"); fflush(stdout);

    // We need a valid emTexture. PaintImage creates an emImageTexture:
    // emImageTexture inherits emTexture, sets Type=IMAGE, stores image ref.
    // Let's try with the emImageTexture wrapper from emPainter.h.

    // From emPainter.h line 1026:
    // inline void emPainter::PaintImage(
    //   double x, double y, double w, double h,
    //   const emImage & img, int alpha, emColor canvasColor
    // ) const {
    //   PaintRect(x, y, w, h, emImageTexture(img, x, y, w, h, alpha), canvasColor);
    // }
    //
    // emImageTexture sets up srcX/Y/W/H from the full image dimensions.

    printf("Creating texture...\n"); fflush(stdout);
    // Explicit quality settings to avoid Model->CoreConfig reads:
    // DQ_AREA_SAMPLING and UQ_AREA_SAMPLING avoid BY_CONFIG paths.
    emImageTexture tex(10.0, 10.0, 100.0, 100.0, srcImg, 255,
                       emTexture::EXTEND_EDGE,
                       emTexture::DQ_4X4,
                       emTexture::UQ_AREA_SAMPLING);
    printf("Texture created. Calling Init...\n"); fflush(stdout);

    bool ok = sct.Init(tex, emColor::WHITE);

    if (!ok) {
        printf("Init returned false.\n");
        printf("This is expected if PixelFormat is NULL.\n");
        printf("Need to initialize PixelFormat to proceed.\n");
        return 1;
    }

    printf("Init succeeded! Derived state:\n");
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

    // Null out the fake Model ref before p's destructor runs,
    // to prevent emRef::~emRef from calling Free() on the fake buffer.
    {
        void* null_ptr = nullptr;
        memcpy(&p.Model, &null_ptr, sizeof(void*));
    }
    return 0;
}
