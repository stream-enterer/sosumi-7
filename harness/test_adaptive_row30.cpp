#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>
#include "emPainter_ScTl.h"

extern "C" unsigned char rust_sample_adaptive_lum(
    const unsigned char* img_data, int img_w, int img_h,
    int ix, int iy, unsigned ox, unsigned oy,
    int sec_ox, int sec_oy, int sec_w, int sec_h
);

static void setup_pixel_format(emPainter::SharedPixelFormat& pf) {
    memset(&pf, 0, sizeof(pf));
    pf.BytesPerPixel = 4; pf.RedRange = 255; pf.GreenRange = 255; pf.BlueRange = 255;
    pf.RedShift = 0; pf.GreenShift = 8; pf.BlueShift = 16;
    pf.RedHash = malloc(256*256*4); pf.GreenHash = malloc(256*256*4); pf.BlueHash = malloc(256*256*4);
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
    const int GW = 32, GH = 32;
    emImage glyphImg; glyphImg.Setup(GW, GH, 1);
    unsigned char* gmap = (unsigned char*)glyphImg.GetMap();
    for (int y = 0; y < GH; y++)
        for (int x = 0; x < GW; x++) {
            double dx = (x - 16.0) / 16.0, dy = (y - 16.0) / 16.0;
            double d = sqrt(dx*dx + dy*dy);
            gmap[y * GW + x] = d < 0.8 ? (unsigned char)(255 * (1.0 - d / 0.8)) : 0;
        }

    const int CW = 200, CH = 100;
    emImage canvas; canvas.Setup(CW, CH, 4); canvas.Fill(emColor(128, 128, 128));
    emPainter p;
    static emPainter::SharedPixelFormat pf; setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap(); p.BytesPerRow = CW * 4; p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0; p.ClipX2 = CW; p.ClipY2 = CH;
    p.OriginX = 0; p.OriginY = 0; p.ScaleX = 100.0; p.ScaleY = 100.0;
    p.UserSpaceMutex = NULL; p.USMLockedByThisThread = NULL;
    static char fake_model[4096]; memset(fake_model, 0, sizeof(fake_model));
    void* fm = (void*)fake_model; memcpy(&p.Model, &fm, sizeof(void*));

    emImageColoredTexture tex(0.1, 0.1, 0.6, 0.6, glyphImg, 0, 0, GW, GH,
        emColor(0,0,0,0), emColor(0,0,0,166),
        emTexture::EXTEND_ZERO, emTexture::DQ_3X3, emTexture::UQ_AREA_SAMPLING);

    emPainter::ScanlineTool sct(p);
    sct.Init(tex, emColor(128,128,128,255));

    // C++ InterpolateImageAdaptive uses:
    // ty = y * TDY - TY - 0x1800000  (for 4-tap center)
    // tx = x * tdx - TX - 0x2800000  (for sliding window start)
    // Then tx = (tx & 0xffffff) + 0x3000000
    // For each pixel: while (tx >= 0) { tx -= 0x1000000; shift columns }
    //                 ox = (tx + 0x1007fff) >> 16
    
    long long TDX = sct.GetTDX(), TDY = sct.GetTDY();
    long long TX = sct.GetTX(), TY = sct.GetTY();
    
    printf("TDX=%lld TDY=%lld TX=%lld TY=%lld\n", TDX, TDY, TX, TY);

    int row = 30;
    int x_start = 20, w = 30;

    // C++ interpolation
    sct.CallInterpolate(x_start, row, w);
    const unsigned char* cpp_buf = sct.GetInterpolationBuffer();

    // Compute C++ ty for this row
    long long ty_cpp = row * TDY - TY - 0x1800000LL;
    unsigned oy_cpp = ((ty_cpp & 0xffffff) + 0x7fff) >> 16;
    int iy_cpp = (int)(ty_cpp >> 24);
    
    printf("Row %d: ty=%lld iy=%d oy=%u\n", row, ty_cpp, iy_cpp, oy_cpp);

    // Simulate C++ tx loop to get exact ix/ox for each pixel
    long long tx_init = (long long)x_start * TDX - TX - 0x2800000LL;
    long long tx_loop = (tx_init & 0xffffff) + 0x3000000LL;
    int imgX = (int)(tx_init >> 24);
    
    printf("Initial: tx_init=%lld imgX=%d tx_loop=%lld\n", tx_init, imgX, tx_loop);
    
    int total_mismatch = 0;
    int max_diff = 0;
    
    for (int i = 0; i < w; i++) {
        // C++ steps imgX forward while tx_loop >= 0
        while (tx_loop >= 0) {
            tx_loop -= 0x1000000LL;
            imgX++;
        }
        unsigned ox_cpp = (unsigned)((tx_loop + 0x1007fffLL) >> 16);
        
        // Call Rust with these exact coordinates
        unsigned char rust_val = rust_sample_adaptive_lum(
            gmap, GW, GH,
            imgX, iy_cpp, ox_cpp, oy_cpp,
            0, 0, GW, GH
        );
        
        unsigned char cpp_val = cpp_buf[i];
        int d = abs((int)cpp_val - (int)rust_val);
        
        if (d > 0 && total_mismatch < 20) {
            printf("  x=%d cpp=%d rust=%d diff=%+d imgX=%d ox=%u oy=%u\n",
                   x_start+i, cpp_val, rust_val, (int)rust_val-(int)cpp_val,
                   imgX, ox_cpp, oy_cpp);
        }
        if (d > 0) { total_mismatch++; if (d > max_diff) max_diff = d; }
        
        tx_loop += TDX;
    }
    
    printf("\n=== ADAPTIVE LUM (exact C++ coords) row=%d ===\n", row);
    if (total_mismatch == 0) {
        printf("PASS: All %d values match.\n", w);
    } else {
        printf("FAIL: %d mismatches, max_diff=%d.\n", total_mismatch, max_diff);
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    return total_mismatch > 0 ? 1 : 0;
}
