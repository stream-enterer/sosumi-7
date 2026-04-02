// Layer 3 test (variant 2): larger image, higher downscale ratio,
// matching the checkbox border image parameters more closely.
// 286x286 image downscaled to ~77 pixel dest width (ratio ~3.7).

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
    // 286x286 RGBA image with realistic gradient
    const int IW = 286, IH = 286;
    emImage srcImg;
    srcImg.Setup(IW, IH, 4);
    unsigned char* map = (unsigned char*)srcImg.GetMap();
    for (int y = 0; y < IH; y++) {
        for (int x = 0; x < IW; x++) {
            int off = (y * IW + x) * 4;
            map[off] = (unsigned char)(x % 256);
            map[off+1] = (unsigned char)(y % 256);
            map[off+2] = (unsigned char)(((x*3 + y*7) >> 2) % 256);
            map[off+3] = (unsigned char)(200 + (x + y) % 56);
        }
    }

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

    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    void* fake_ptr = (void*)fake_model;
    memcpy(&p.Model, &fake_ptr, sizeof(void*));

    // Match a checkbox corner section: tex at ~0.013, size ~0.097
    // dest size in pixels: 0.097 * 800 ≈ 77.6
    emImageTexture tex(0.013026, 0.013026, 0.096974, 0.096974, srcImg, 255,
                       emTexture::EXTEND_EDGE,
                       emTexture::DQ_3X3,
                       emTexture::UQ_AREA_SAMPLING);

    emPainter::ScanlineTool sct(p);
    bool ok = sct.Init(tex, emColor::WHITE);
    if (!ok) {
        printf("Init returned false\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        return 1;
    }

    printf("Init: ImgW=%d ImgH=%d ImgDX=%zd ImgDY=%zd\n",
           sct.GetImgW(), sct.GetImgH(), sct.GetImgDX(), sct.GetImgDY());
    printf("  TDX=%lld TDY=%lld TX=%lld TY=%lld ODX=%u ODY=%u\n",
           (long long)sct.GetTDX(), (long long)sct.GetTDY(),
           (long long)sct.GetTX(), (long long)sct.GetTY(),
           sct.GetODX(), sct.GetODY());

    // Test multiple rows across the section
    int total_mismatches = 0;
    int total_bytes = 0;
    int max_diff = 0;

    for (int row = 11; row < 88; row += 5) {
        int x_start = 11;
        int w = 66; // most of the section width
        if (w > 256) w = 256; // buffer limit

        sct.CallInterpolate(x_start, row, w);
        const unsigned char* cpp_buf = sct.GetInterpolationBuffer();

        CAreaSampleTransform rxfm;
        rxfm.tdx = sct.GetTDX();
        rxfm.tdy = sct.GetTDY();
        rxfm.tx = sct.GetTX();
        rxfm.ty = sct.GetTY();
        rxfm.odx = sct.GetODX();
        rxfm.ody = sct.GetODY();
        rxfm.img_w = sct.GetImgW();
        rxfm.img_h = sct.GetImgH();
        rxfm.stride_x = sct.GetImgDX() / 4;
        rxfm.stride_y = sct.GetImgDY() / (IW * 4);
        if (rxfm.stride_x < 1) rxfm.stride_x = 1;
        if (rxfm.stride_y < 1) rxfm.stride_y = 1;
        rxfm.off_x = (IW - (sct.GetImgW() - 1) * (int)rxfm.stride_x - 1) / 2;
        rxfm.off_y = (IH - (sct.GetImgH() - 1) * (int)rxfm.stride_y - 1) / 2;

        unsigned char rust_buf[256 * 4];
        memset(rust_buf, 0, sizeof(rust_buf));

        rust_interpolate_area_sampled(
            map, IW, IH,
            &rxfm,
            0, 0, IW, IH,
            x_start, row,
            w,
            rust_buf
        );

        for (int i = 0; i < w * 4; i++) {
            total_bytes++;
            int d = abs((int)cpp_buf[i] - (int)rust_buf[i]);
            if (d > 0) {
                if (total_mismatches < 10) {
                    printf("  row=%d pixel=%d ch=%d cpp=%d rust=%d diff=%+d\n",
                           row, i/4, i%4, cpp_buf[i], rust_buf[i],
                           (int)rust_buf[i] - (int)cpp_buf[i]);
                }
                total_mismatches++;
                if (d > max_diff) max_diff = d;
            }
        }
    }

    if (total_mismatches == 0) {
        printf("PASS: All %d interpolation bytes match across %d rows.\n",
               total_bytes, 16);
    } else {
        printf("FAIL: %d mismatches out of %d bytes, max_diff=%d.\n",
               total_mismatches, total_bytes, max_diff);
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    return total_mismatches > 0 ? 1 : 0;
}
