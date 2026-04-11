// End-to-end test: C++ PaintScanline (interpolation+blend in one call)
// vs Rust interpolation + premul blend (two steps).
// Same image, same transform, same dest buffer, compare final pixels.

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

// C++ blend hash (unshifted)
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

// Rust-style premul canvas blend (matches blend_scanline_premul_canvas)
// canvas_r/g/b = canvas color channel values
static void rust_premul_canvas_blend(unsigned char* dest, const unsigned char* interp_buf,
                                      int count, int opacity,
                                      unsigned char canvas_r, unsigned char canvas_g, unsigned char canvas_b) {
    for (int i = 0; i < count; i++) {
        const unsigned char* s = interp_buf + i * 4;
        unsigned char* d = dest + i * 4;

        int o = opacity;
        unsigned char pm[4];
        if (o >= 0x1000) {
            pm[0] = s[0]; pm[1] = s[1]; pm[2] = s[2]; pm[3] = s[3];
        } else if (o > 0) {
            pm[0] = (unsigned char)((s[0] * o + 0x800) >> 12);
            pm[1] = (unsigned char)((s[1] * o + 0x800) >> 12);
            pm[2] = (unsigned char)((s[2] * o + 0x800) >> 12);
            pm[3] = (unsigned char)((s[3] * o + 0x800) >> 12);
        } else {
            continue;
        }

        unsigned char a = pm[3];
        if (a == 0) continue;

        // Canvas blend: dest += src_premul - blend_hash(canvas, alpha)
        int cr = cpp_hash[(canvas_r << 8) + a];
        int cg = cpp_hash[(canvas_g << 8) + a];
        int cb = cpp_hash[(canvas_b << 8) + a];

        int r = d[0] + pm[0] - cr;
        int g = d[1] + pm[1] - cg;
        int b = d[2] + pm[2] - cb;
        d[0] = (unsigned char)(r < 0 ? 0 : r > 255 ? 255 : r);
        d[1] = (unsigned char)(g < 0 ? 0 : g > 255 ? 255 : g);
        d[2] = (unsigned char)(b < 0 ? 0 : b > 255 ? 255 : b);
        // dest alpha unchanged
    }
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
    init_hash();

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

    const int CW = 800, CH = 600;
    emImage canvas;
    canvas.Setup(CW, CH, 4);
    canvas.Fill(emColor(128, 128, 128)); // gray background

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

    // Checkbox corner section params
    double tex_x = 0.013026, tex_y = 0.013026;
    double tex_w = 0.096974, tex_h = 0.096974;

    emImageTexture tex(tex_x, tex_y, tex_w, tex_h, srcImg, 255,
                       emTexture::EXTEND_EDGE,
                       emTexture::DQ_3X3,
                       emTexture::UQ_AREA_SAMPLING);

    emPainter::ScanlineTool sct(p);
    // Use WHITE canvas to test the CVC (canvas-blend) path
    if (!sct.Init(tex, emColor::WHITE)) {
        printf("Init failed\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        return 1;
    }

    // Compute pixel extents (matching PaintRect logic)
    double px1 = tex_x * 800.0, py1 = tex_y * 800.0;
    double px2 = px1 + tex_w * 800.0, py2 = py1 + tex_h * 800.0;
    int ix = (int)(px1 * 0x1000);
    int ixe = (int)(px2 * 0x1000) + 0xfff;
    int ax1 = 0x1000 - (ix & 0xfff);
    int ax2 = (ixe & 0xfff) + 1;
    ix >>= 12; ixe >>= 12;
    int iw = ixe - ix;
    if (iw <= 1 && iw > 0) ax1 += ax2 - 0x1000;

    int iy = (int)(py1 * 0x1000);
    int iy2_raw = (int)(py2 * 0x1000);
    int ay1 = 0x1000 - (iy & 0xfff);
    int ay2 = iy2_raw & 0xfff;
    iy >>= 12;
    int iy2 = iy2_raw >> 12;
    if (iy >= iy2) { ay1 += ay2 - 0x1000; ay2 = 0; }

    printf("Rect: ix=%d iy=%d ixe=%d iy2=%d iw=%d\n", ix, iy, ixe, iy2, iw);
    printf("Frac: ax1=%d ax2=%d ay1=%d ay2=%d\n", ax1, ax2, ay1, ay2);

    // Save a copy of the canvas for Rust-side rendering
    unsigned char* canvas_copy = (unsigned char*)malloc(CW * CH * 4);
    memcpy(canvas_copy, canvas.GetMap(), CW * CH * 4);

    // ── C++ side: call PaintScanline for an interior row ──
    int test_row = (iy + iy2) / 2; // middle row

    // Print first interpolation pixel + expected blend
    sct.CallInterpolate(ix, test_row, iw);
    const unsigned char* ib = sct.GetInterpolationBuffer();
    printf("Interp[0]: rgba(%d,%d,%d,%d)\n", ib[0], ib[1], ib[2], ib[3]);
    // Manual blend for first pixel with opacity=ax1=2373:
    int o = ax1;
    int a = (ib[3] * o + 0x800) >> 12;
    int pr = (ib[0] * o + 0x800) >> 12;
    int pg = (ib[1] * o + 0x800) >> 12;
    int pb = (ib[2] * o + 0x800) >> 12;
    printf("Scaled (o=%d): r=%d g=%d b=%d a=%d\n", o, pr, pg, pb, a);
    // Background is (128,128,128)
    int t = (255 - a) * 257;
    int er = ((128 * t + 0x8073) >> 16) + pr;
    int eg = ((128 * t + 0x8073) >> 16) + pg;
    int eb = ((128 * t + 0x8073) >> 16) + pb;
    printf("Expected blend: rgb(%d,%d,%d)\n", er, eg, eb);

    // Print canvas background before PaintScanline
    unsigned char* pre = (unsigned char*)canvas.GetMap() + test_row * CW * 4;
    printf("Background at x=%d row=%d: rgba(%d,%d,%d,%d)\n",
           ix, test_row, pre[ix*4], pre[ix*4+1], pre[ix*4+2], pre[ix*4+3]);

    sct.CallPaintScanline(ix, test_row, iw, ax1, 0x1000, ax2);

    // Read the C++ output from the canvas
    unsigned char* cpp_row = (unsigned char*)canvas.GetMap() + test_row * CW * 4;
    printf("C++ output at x=%d: rgba(%d,%d,%d,%d)\n",
           ix, cpp_row[ix*4], cpp_row[ix*4+1], cpp_row[ix*4+2], cpp_row[ix*4+3]);
    printf("C++ output at x=%d: rgba(%d,%d,%d,%d)\n",
           ix+1, cpp_row[(ix+1)*4], cpp_row[(ix+1)*4+1], cpp_row[(ix+1)*4+2], cpp_row[(ix+1)*4+3]);

    // ── Rust side: interpolate + blend for the same row ──
    // First, get Rust interpolation output
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

    unsigned char rust_interp[256 * 4];
    memset(rust_interp, 0, sizeof(rust_interp));
    rust_interpolate_area_sampled(map, IW, IH, &rxfm,
                                   0, 0, IW, IH,
                                   ix, test_row, iw,
                                   rust_interp);

    // First, verify interpolation buffers match
    sct.CallInterpolate(ix, test_row, iw);
    const unsigned char* cpp_interp = sct.GetInterpolationBuffer();
    int interp_mismatches = 0;
    for (int i = 0; i < iw * 4; i++) {
        if (cpp_interp[i] != rust_interp[i]) {
            if (interp_mismatches < 5)
                printf("INTERP MISMATCH: byte %d cpp=%d rust=%d\n", i, cpp_interp[i], rust_interp[i]);
            interp_mismatches++;
        }
    }
    if (interp_mismatches == 0) {
        printf("Interpolation buffers match (%d bytes).\n", iw * 4);
    } else {
        printf("INTERP FAIL: %d mismatches in %d bytes.\n", interp_mismatches, iw * 4);
    }

    // Now blend Rust interpolation output onto the canvas copy
    unsigned char* rust_row = canvas_copy + test_row * CW * 4;

    // Canvas = WHITE (255,255,255)
    // First pixel: opacity = ax1
    rust_premul_canvas_blend(rust_row + ix * 4, rust_interp, 1, ax1, 255, 255, 255);
    // Interior pixels: opacity = 0x1000
    if (iw > 2) {
        rust_premul_canvas_blend(rust_row + (ix + 1) * 4, rust_interp + 4, iw - 2, 0x1000, 255, 255, 255);
    }
    // Last pixel: opacity = ax2
    if (iw > 1) {
        rust_premul_canvas_blend(rust_row + (ix + iw - 1) * 4,
                          rust_interp + (iw - 1) * 4, 1, ax2, 255, 255, 255);
    }

    // Compare
    int mismatches = 0;
    int max_diff = 0;
    for (int x = ix; x < ix + iw; x++) {
        for (int ch = 0; ch < 3; ch++) { // RGB only (C++ writes alpha=0)
            int ci = test_row * CW * 4 + x * 4 + ch;
            int d = abs((int)cpp_row[x * 4 + ch] - (int)rust_row[x * 4 + ch]);
            if (d > 0) {
                if (mismatches < 20) {
                    printf("MISMATCH: x=%d ch=%d cpp=%d rust=%d diff=%+d\n",
                           x, ch, cpp_row[x*4+ch], rust_row[x*4+ch],
                           (int)rust_row[x*4+ch] - (int)cpp_row[x*4+ch]);
                }
                mismatches++;
                if (d > max_diff) max_diff = d;
            }
        }
    }

    if (mismatches == 0) {
        printf("PASS: All %d RGB bytes match for row %d.\n", iw * 3, test_row);
    } else {
        printf("FAIL: %d mismatches out of %d RGB bytes, max_diff=%d.\n",
               mismatches, iw * 3, max_diff);
    }

    free(canvas_copy);
    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    return mismatches > 0 ? 1 : 0;
}
