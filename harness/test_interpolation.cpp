// Layer 3 test: compare C++ area sampling interpolation against Rust.
// Uses actual libemCore.so ScanlineTool::Interpolate vs Rust harness.
// Same image, same transform params, compare output buffers byte-by-byte.

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>
#include "emPainter_ScTl.h"

struct CAreaSampleTransform {
    long long tdx, tdy, tx, ty;
    unsigned odx, ody;
    int img_w, img_h;
    unsigned stride_x, stride_y;
    int off_x, off_y;
};

extern "C" int rust_interpolate_area_sampled(
    const unsigned char* img_data,
    int img_w, int img_h,
    const CAreaSampleTransform* xfm,
    int sec_ox, int sec_oy, int sec_w, int sec_h,
    int dest_x, int dest_y,
    int count,
    unsigned char* out_buf
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
    // Create a test image: 64x64 RGBA with a gradient pattern
    emImage srcImg;
    srcImg.Setup(64, 64, 4);
    unsigned char* map = (unsigned char*)srcImg.GetMap();
    for (int y = 0; y < 64; y++) {
        for (int x = 0; x < 64; x++) {
            int off = (y * 64 + x) * 4;
            map[off] = (unsigned char)((x * 4) % 256);
            map[off+1] = (unsigned char)((y * 4) % 256);
            map[off+2] = (unsigned char)(((x + y) * 2) % 256);
            map[off+3] = 255;
        }
    }

    // Set up painter: 256x256 canvas, scale=1 (simple case)
    emImage canvas;
    canvas.Setup(256, 256, 4);
    canvas.Fill(emColor::WHITE);

    emPainter p;
    static emPainter::SharedPixelFormat pf;
    setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap();
    p.BytesPerRow = 256 * 4;
    p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0;
    p.ClipX2 = 256; p.ClipY2 = 256;
    p.OriginX = 0; p.OriginY = 0;
    p.ScaleX = 1.0; p.ScaleY = 1.0;
    p.UserSpaceMutex = NULL;
    p.USMLockedByThisThread = NULL;

    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    void* fake_ptr = (void*)fake_model;
    memcpy(&p.Model, &fake_ptr, sizeof(void*));

    // Texture: 64x64 image displayed at 20x20 pixels (downscaling: ratio ~3.2)
    // This forces area sampling path (TDX > 0xFFFF00)
    emImageTexture tex(10.0, 10.0, 20.0, 20.0, srcImg, 255,
                       emTexture::EXTEND_EDGE,
                       emTexture::DQ_3X3,
                       emTexture::UQ_AREA_SAMPLING);

    emPainter::ScanlineTool sct(p);
    bool ok = sct.Init(tex, emColor::WHITE);
    if (!ok) {
        printf("Init returned false\n");
        void* null_ptr = nullptr;
        memcpy(&p.Model, &null_ptr, sizeof(void*));
        return 1;
    }

    printf("Init succeeded:\n");
    printf("  ImgW=%d ImgH=%d ImgDX=%zd TDX=%lld TDY=%lld\n",
           sct.GetImgW(), sct.GetImgH(), sct.GetImgDX(),
           (long long)sct.GetTDX(), (long long)sct.GetTDY());
    printf("  TX=%lld TY=%lld ODX=%u ODY=%u\n",
           (long long)sct.GetTX(), (long long)sct.GetTY(),
           sct.GetODX(), sct.GetODY());

    // Run C++ interpolation for row 15, columns 10-29 (20 pixels)
    int test_row = 15;
    int test_x = 10;
    int test_w = 20;

    sct.CallInterpolate(test_x, test_row, test_w);
    const unsigned char* cpp_buf = sct.GetInterpolationBuffer();

    // Run Rust interpolation with the same transform
    CAreaSampleTransform rxfm;
    rxfm.tdx = sct.GetTDX();
    rxfm.tdy = sct.GetTDY();
    rxfm.tx = sct.GetTX();
    rxfm.ty = sct.GetTY();
    rxfm.odx = sct.GetODX();
    rxfm.ody = sct.GetODY();
    rxfm.img_w = sct.GetImgW();
    rxfm.img_h = sct.GetImgH();
    // Stride and offset: need to figure out from Init state
    // For now, use stride=1 if no reduction happened
    if (sct.GetImgDX() > 4) {
        // Stride reduction happened: ImgDX = channels * stride
        rxfm.stride_x = sct.GetImgDX() / 4;
    } else {
        rxfm.stride_x = 1;
    }
    if (sct.GetImgDY() > 64 * 4) {
        rxfm.stride_y = sct.GetImgDY() / (64 * 4);
    } else {
        rxfm.stride_y = 1;
    }
    // off_x/off_y: centering offset
    int orig_w = 64; // original image width
    rxfm.off_x = (orig_w - (sct.GetImgW() - 1) * (int)rxfm.stride_x - 1) / 2;
    int orig_h = 64;
    rxfm.off_y = (orig_h - (sct.GetImgH() - 1) * (int)rxfm.stride_y - 1) / 2;

    unsigned char rust_buf[256 * 4]; // max 256 pixels
    memset(rust_buf, 0, sizeof(rust_buf));

    rust_interpolate_area_sampled(
        map, 64, 64,
        &rxfm,
        0, 0, 64, 64,  // section = full image
        test_x, test_row,
        test_w,
        rust_buf
    );

    // Compare
    int mismatches = 0;
    for (int i = 0; i < test_w * 4; i++) {
        if (cpp_buf[i] != rust_buf[i]) {
            if (mismatches < 20) {
                printf("MISMATCH: pixel=%d ch=%d cpp=%d rust=%d diff=%d\n",
                       i / 4, i % 4, cpp_buf[i], rust_buf[i],
                       (int)rust_buf[i] - (int)cpp_buf[i]);
            }
            mismatches++;
        }
    }

    if (mismatches == 0) {
        printf("PASS: All %d interpolation bytes match.\n", test_w * 4);
    } else {
        printf("FAIL: %d mismatches out of %d bytes.\n", mismatches, test_w * 4);
    }

    void* null_ptr = nullptr;
    memcpy(&p.Model, &null_ptr, sizeof(void*));
    return mismatches > 0 ? 1 : 0;
}
