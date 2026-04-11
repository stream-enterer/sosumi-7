// Test with the ACTUAL GroupBorder.tga image (592x592, 4ch, RLE)
// instead of synthetic gradients. This tests whether specific pixel patterns
// in the real border image expose interpolation or blend divergences.

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
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
    const unsigned char* img_data, int img_w, int img_h,
    const CAreaSampleTransform* xfm,
    int sec_ox, int sec_oy, int sec_w, int sec_h,
    int dest_x, int dest_y, int count,
    unsigned char* out_buf
);

// Minimal TGA RLE decoder for type 10 (RLE true-color), 32bpp, top-origin
static unsigned char* load_tga_rgba(const char* path, int* out_w, int* out_h) {
    FILE* f = fopen(path, "rb");
    if (!f) { fprintf(stderr, "Cannot open %s\n", path); return NULL; }

    unsigned char header[18];
    if (fread(header, 1, 18, f) != 18) { fclose(f); return NULL; }

    int id_len = header[0];
    int img_type = header[2];
    int w = header[12] | (header[13] << 8);
    int h = header[14] | (header[15] << 8);
    int bpp = header[16];
    int desc = header[17];

    if (img_type != 10 || bpp != 32) {
        fprintf(stderr, "Unsupported TGA: type=%d bpp=%d\n", img_type, bpp);
        fclose(f); return NULL;
    }

    // Skip ID field
    if (id_len > 0) fseek(f, id_len, SEEK_CUR);

    int npix = w * h;
    // Allocate as RGBA (we'll convert from BGRA)
    unsigned char* pixels = (unsigned char*)malloc(npix * 4);
    int idx = 0;

    while (idx < npix) {
        unsigned char rep;
        if (fread(&rep, 1, 1, f) != 1) break;
        int count = (rep & 0x7F) + 1;
        if (rep & 0x80) {
            // RLE packet: one pixel repeated
            unsigned char bgra[4];
            if (fread(bgra, 1, 4, f) != 4) break;
            for (int i = 0; i < count && idx < npix; i++, idx++) {
                pixels[idx*4+0] = bgra[2]; // R
                pixels[idx*4+1] = bgra[1]; // G
                pixels[idx*4+2] = bgra[0]; // B
                pixels[idx*4+3] = bgra[3]; // A
            }
        } else {
            // Raw packet
            for (int i = 0; i < count && idx < npix; i++, idx++) {
                unsigned char bgra[4];
                if (fread(bgra, 1, 4, f) != 4) break;
                pixels[idx*4+0] = bgra[2];
                pixels[idx*4+1] = bgra[1];
                pixels[idx*4+2] = bgra[0];
                pixels[idx*4+3] = bgra[3];
            }
        }
    }
    fclose(f);

    bool top_origin = (desc & 0x20) != 0;
    if (!top_origin) {
        // Flip vertically
        for (int y = 0; y < h / 2; y++) {
            unsigned char* row1 = pixels + y * w * 4;
            unsigned char* row2 = pixels + (h - 1 - y) * w * 4;
            for (int x = 0; x < w * 4; x++) {
                unsigned char tmp = row1[x]; row1[x] = row2[x]; row2[x] = tmp;
            }
        }
    }

    *out_w = w;
    *out_h = h;
    return pixels;
}

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
    // Load the actual border TGA
    const char* tga_path = "/home/a0/git/eaglemode-0.96.4/res/emCore/toolkit/GroupBorder.tga";
    int IW, IH;
    unsigned char* tga_pixels = load_tga_rgba(tga_path, &IW, &IH);
    if (!tga_pixels) return 1;
    printf("Loaded TGA: %dx%d\n", IW, IH);

    // Copy into emImage for C++ ScanlineTool
    emImage srcImg;
    srcImg.Setup(IW, IH, 4);
    memcpy((void*)srcImg.GetMap(), tga_pixels, IW * IH * 4);

    // Print some pixel samples to verify loading
    printf("Pixel (0,0): rgba(%d,%d,%d,%d)\n",
           tga_pixels[0], tga_pixels[1], tga_pixels[2], tga_pixels[3]);
    printf("Pixel (296,296): rgba(%d,%d,%d,%d)\n",
           tga_pixels[(296*IW+296)*4], tga_pixels[(296*IW+296)*4+1],
           tga_pixels[(296*IW+296)*4+2], tga_pixels[(296*IW+296)*4+3]);

    const int CW = 800, CH = 600;
    emImage canvas;
    canvas.Setup(CW, CH, 4);
    canvas.Fill(emColor(0, 0, 0, 0)); // transparent canvas (source-over path)

    emPainter p;
    static emPainter::SharedPixelFormat pf;
    setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap();
    p.BytesPerRow = CW * 4;
    p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0;
    p.ClipX2 = CW; p.ClipY2 = CH;
    p.OriginX = 0; p.OriginY = 0;
    p.ScaleX = 800.0; p.ScaleY = 800.0;
    p.UserSpaceMutex = NULL;
    p.USMLockedByThisThread = NULL;
    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    void* fm = (void*)fake_model;
    memcpy(&p.Model, &fm, sizeof(void*));

    // Checkbox corner section params (matching golden test)
    double tex_x = 0.013026, tex_y = 0.013026;
    double tex_w = 0.096974, tex_h = 0.096974;

    emImageTexture tex(tex_x, tex_y, tex_w, tex_h, srcImg, 255,
                       emTexture::EXTEND_EDGE,
                       emTexture::DQ_3X3,
                       emTexture::UQ_AREA_SAMPLING);

    emPainter::ScanlineTool sct(p);
    if (!sct.Init(tex, 0)) {
        printf("Init failed\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        free(tga_pixels); return 1;
    }

    printf("Init: ImgW=%d ImgH=%d ImgDX=%zd ImgDY=%zd\n",
           sct.GetImgW(), sct.GetImgH(), sct.GetImgDX(), sct.GetImgDY());
    printf("  TDX=%lld TDY=%lld TX=%lld TY=%lld ODX=%u ODY=%u\n",
           (long long)sct.GetTDX(), (long long)sct.GetTDY(),
           (long long)sct.GetTX(), (long long)sct.GetTY(),
           sct.GetODX(), sct.GetODY());

    // Compute pixel extents
    double px1 = tex_x * 800.0, py1 = tex_y * 800.0;
    double px2 = px1 + tex_w * 800.0, py2 = py1 + tex_h * 800.0;
    int ix = (int)(px1 * 0x1000) >> 12;
    int ixe_raw = (int)(px2 * 0x1000) + 0xfff;
    int ixe = ixe_raw >> 12;
    int iw = ixe - ix;
    int iy = (int)(py1 * 0x1000) >> 12;
    int iy2 = (int)(py2 * 0x1000) >> 12;

    printf("Pixel range: x=[%d,%d) y=[%d,%d) iw=%d\n", ix, ixe, iy, iy2, iw);

    // Test interpolation across multiple rows
    int total_mismatches = 0;
    int total_bytes = 0;
    int max_diff = 0;

    // Test every row in the section
    for (int row = iy; row < iy2; row++) {
        sct.CallInterpolate(ix, row, iw);
        const unsigned char* cpp_buf = sct.GetInterpolationBuffer();

        CAreaSampleTransform rxfm;
        rxfm.tdx = sct.GetTDX(); rxfm.tdy = sct.GetTDY();
        rxfm.tx = sct.GetTX(); rxfm.ty = sct.GetTY();
        rxfm.odx = sct.GetODX(); rxfm.ody = sct.GetODY();
        rxfm.img_w = sct.GetImgW(); rxfm.img_h = sct.GetImgH();
        rxfm.stride_x = sct.GetImgDX() / 4;
        rxfm.stride_y = sct.GetImgDY() / (IW * 4);
        if (rxfm.stride_x < 1) rxfm.stride_x = 1;
        if (rxfm.stride_y < 1) rxfm.stride_y = 1;
        rxfm.off_x = (IW - (sct.GetImgW() - 1) * (int)rxfm.stride_x - 1) / 2;
        rxfm.off_y = (IH - (sct.GetImgH() - 1) * (int)rxfm.stride_y - 1) / 2;

        unsigned char rust_buf[512 * 4];
        memset(rust_buf, 0, sizeof(rust_buf));
        rust_interpolate_area_sampled(tga_pixels, IW, IH, &rxfm,
                                       0, 0, IW, IH,
                                       ix, row, iw, rust_buf);

        for (int i = 0; i < iw * 4; i++) {
            total_bytes++;
            int d = abs((int)cpp_buf[i] - (int)rust_buf[i]);
            if (d > 0) {
                if (total_mismatches < 20) {
                    printf("  row=%d pixel=%d ch=%d cpp=%d rust=%d diff=%+d\n",
                           row, i/4, i%4, cpp_buf[i], rust_buf[i],
                           (int)rust_buf[i] - (int)cpp_buf[i]);
                }
                total_mismatches++;
                if (d > max_diff) max_diff = d;
            }
        }
    }

    printf("\n=== INTERPOLATION RESULT ===\n");
    if (total_mismatches == 0) {
        printf("PASS: All %d interpolation bytes match across %d rows.\n",
               total_bytes, iy2 - iy);
    } else {
        printf("FAIL: %d mismatches out of %d bytes, max_diff=%d.\n",
               total_mismatches, total_bytes, max_diff);
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    free(tga_pixels);
    return total_mismatches > 0 ? 1 : 0;
}
