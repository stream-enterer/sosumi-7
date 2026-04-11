// End-to-end test with ACTUAL GroupBorder.tga:
// C++ CallPaintScanline vs Rust interpolation + source-over blend.
// Tests the full pipeline with real image data.

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

extern "C" void rust_blend_source_over_simple(
    unsigned char* dest, const unsigned char* src, int count, int opacity
);

// Blend using the same per-pixel opacity logic as C++ PaintScanline:
// first pixel gets a1, interior gets a, last pixel gets a2.
static void rust_blend_row(unsigned char* dest, const unsigned char* interp,
                            int ix, int iw, int a1, int a, int a2) {
    // First pixel
    rust_blend_source_over_simple(dest + ix * 4, interp, 1, a1);
    // Interior pixels
    if (iw > 2) {
        rust_blend_source_over_simple(dest + (ix + 1) * 4, interp + 4, iw - 2, a);
    }
    // Last pixel
    if (iw > 1) {
        rust_blend_source_over_simple(dest + (ix + iw - 1) * 4,
                                      interp + (iw - 1) * 4, 1, a2);
    }
}

// Minimal TGA RLE decoder for type 10, 32bpp
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
    if (id_len > 0) fseek(f, id_len, SEEK_CUR);
    int npix = w * h;
    unsigned char* pixels = (unsigned char*)malloc(npix * 4);
    int idx = 0;
    while (idx < npix) {
        unsigned char rep;
        if (fread(&rep, 1, 1, f) != 1) break;
        int count = (rep & 0x7F) + 1;
        if (rep & 0x80) {
            unsigned char bgra[4];
            if (fread(bgra, 1, 4, f) != 4) break;
            for (int i = 0; i < count && idx < npix; i++, idx++) {
                pixels[idx*4+0] = bgra[2];
                pixels[idx*4+1] = bgra[1];
                pixels[idx*4+2] = bgra[0];
                pixels[idx*4+3] = bgra[3];
            }
        } else {
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
        for (int y = 0; y < h / 2; y++) {
            unsigned char* r1 = pixels + y * w * 4;
            unsigned char* r2 = pixels + (h - 1 - y) * w * 4;
            for (int x = 0; x < w * 4; x++) {
                unsigned char tmp = r1[x]; r1[x] = r2[x]; r2[x] = tmp;
            }
        }
    }
    *out_w = w; *out_h = h;
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
    const char* tga_path = "/home/a0/git/eaglemode-0.96.4/res/emCore/toolkit/GroupBorder.tga";
    int IW, IH;
    unsigned char* tga_pixels = load_tga_rgba(tga_path, &IW, &IH);
    if (!tga_pixels) return 1;
    printf("Loaded TGA: %dx%d\n", IW, IH);

    emImage srcImg;
    srcImg.Setup(IW, IH, 4);
    memcpy((void*)srcImg.GetMap(), tga_pixels, IW * IH * 4);

    const int CW = 800, CH = 600;

    // C++ canvas
    emImage cpp_canvas;
    cpp_canvas.Setup(CW, CH, 4);
    memset((void*)cpp_canvas.GetMap(), 0, CW * CH * 4); // transparent

    // Rust canvas (copy)
    unsigned char* rust_canvas = (unsigned char*)calloc(CW * CH, 4);

    emPainter p;
    static emPainter::SharedPixelFormat pf;
    setup_pixel_format(pf);
    p.Map = (void*)cpp_canvas.GetMap();
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

    // Checkbox corner section params
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
        free(tga_pixels); free(rust_canvas); return 1;
    }

    // Compute pixel extents with sub-pixel fractions (PaintRect logic)
    double dpx1 = tex_x * 800.0, dpy1 = tex_y * 800.0;
    double dpx2 = dpx1 + tex_w * 800.0, dpy2 = dpy1 + tex_h * 800.0;

    int ix_raw = (int)(dpx1 * 0x1000);
    int ixe_raw = (int)(dpx2 * 0x1000) + 0xfff;
    int ax1 = 0x1000 - (ix_raw & 0xfff);
    int ax2 = (ixe_raw & 0xfff) + 1;
    int ix = ix_raw >> 12;
    int ixe = ixe_raw >> 12;
    int iw = ixe - ix;
    if (iw <= 1 && iw > 0) ax1 += ax2 - 0x1000;

    int iy_raw = (int)(dpy1 * 0x1000);
    int iy2_raw = (int)(dpy2 * 0x1000);
    int ay1 = 0x1000 - (iy_raw & 0xfff);
    int ay2 = iy2_raw & 0xfff;
    int iy = iy_raw >> 12;
    int iy2 = iy2_raw >> 12;
    if (iy >= iy2) { ay1 += ay2 - 0x1000; ay2 = 0; }

    printf("Pixel range: x=[%d,%d) y=[%d,%d) iw=%d\n", ix, ixe, iy, iy2, iw);
    printf("Frac: ax1=%d ax2=%d ay1=%d ay2=%d\n", ax1, ax2, ay1, ay2);

    // Extract transform for Rust interpolation
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

    int total_mismatches = 0;
    int total_bytes = 0;
    int max_diff = 0;

    // Process each row like PaintRect does (matching emPainter.cpp lines 382-396)
    // Top edge row
    if (ay1 < 0x1000 && iy <= iy2) {
        int a1_top = (ax1 * ay1 + 0x7ff) >> 12;
        int a2_top = (ax2 * ay1 + 0x7ff) >> 12;
        sct.CallPaintScanline(ix, iy, iw, a1_top, ay1, a2_top);

        unsigned char rust_interp[512 * 4];
        memset(rust_interp, 0, sizeof(rust_interp));
        rust_interpolate_area_sampled(tga_pixels, IW, IH, &rxfm,
                                       0, 0, IW, IH, ix, iy, iw, rust_interp);
        rust_blend_row(rust_canvas + iy * CW * 4, rust_interp, ix, iw, a1_top, ay1, a2_top);
        iy++;
    }
    // Interior rows
    for (int row = iy; row < iy2; row++) {
        sct.CallPaintScanline(ix, row, iw, ax1, 0x1000, ax2);

        unsigned char rust_interp[512 * 4];
        memset(rust_interp, 0, sizeof(rust_interp));
        rust_interpolate_area_sampled(tga_pixels, IW, IH, &rxfm,
                                       0, 0, IW, IH, ix, row, iw, rust_interp);
        rust_blend_row(rust_canvas + row * CW * 4, rust_interp, ix, iw, ax1, 0x1000, ax2);
    }
    // Bottom edge row
    if (ay2 > 0) {
        int a1_bot = (ax1 * ay2 + 0x7ff) >> 12;
        int a2_bot = (ax2 * ay2 + 0x7ff) >> 12;
        sct.CallPaintScanline(ix, iy2, iw, a1_bot, ay2, a2_bot);

        unsigned char rust_interp[512 * 4];
        memset(rust_interp, 0, sizeof(rust_interp));
        rust_interpolate_area_sampled(tga_pixels, IW, IH, &rxfm,
                                       0, 0, IW, IH, ix, iy2, iw, rust_interp);
        rust_blend_row(rust_canvas + iy2 * CW * 4, rust_interp, ix, iw, a1_bot, ay2, a2_bot);
    }

    // Compare all rows in the painted region
    int cmp_iy = (int)(dpy1 * 0x1000) >> 12;  // recompute since iy was modified
    int cmp_iy2 = iy2;
    if (ay2 > 0) cmp_iy2++;  // include bottom edge row
    for (int row = cmp_iy; row < cmp_iy2; row++) {
        unsigned char* cpp_row = (unsigned char*)cpp_canvas.GetMap() + row * CW * 4;
        unsigned char* rust_row = rust_canvas + row * CW * 4;
        for (int x = ix; x < ix + iw; x++) {
            for (int ch = 0; ch < 4; ch++) {
                int ci = x * 4 + ch;
                int d = abs((int)cpp_row[ci] - (int)rust_row[ci]);
                if (d > 0) {
                    if (total_mismatches < 30) {
                        printf("  row=%d x=%d ch=%d cpp=%d rust=%d diff=%+d\n",
                               row, x, ch, cpp_row[ci], rust_row[ci],
                               (int)rust_row[ci] - (int)cpp_row[ci]);
                    }
                    total_mismatches++;
                    if (d > max_diff) max_diff = d;
                }
                total_bytes++;
            }
        }
    }

    printf("\n=== E2E RESULT (real TGA, source-over) ===\n");
    if (total_mismatches == 0) {
        printf("PASS: All %d bytes match across %d rows.\n",
               total_bytes, iy2 - iy + 1);
    } else {
        printf("FAIL: %d mismatches out of %d bytes, max_diff=%d.\n",
               total_mismatches, total_bytes, max_diff);
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    free(tga_pixels); free(rust_canvas);
    return total_mismatches > 0 ? 1 : 0;
}
