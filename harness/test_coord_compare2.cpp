// Compare exact coordinates used by C++ InterpolateImageAdaptive vs Rust PaintImageColored
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
            int c1=(a1*range+127)/255;
            for (int a2 = 0; a2 < 128; a2++) {
                int c2=(a2*range+127)/255; int c3=(a1*a2*range+32512)/65025;
                ((unsigned*)hash)[(a1<<8)+a2]=c3<<shift;
                ((unsigned*)hash)[(a1<<8)+(255-a2)]=(c1-c3)<<shift;
                ((unsigned*)hash)[((255-a1)<<8)+a2]=(c2-c3)<<shift;
                ((unsigned*)hash)[((255-a1)<<8)+(255-a2)]=(range+c3-c1-c2)<<shift;
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
            gmap[y*GW+x] = sqrt(dx*dx+dy*dy) < 0.8 ? (unsigned char)(255*(1.0-sqrt(dx*dx+dy*dy)/0.8)) : 0;
        }

    const int CW = 200, CH = 100;
    emImage canvas; canvas.Setup(CW, CH, 4); canvas.Fill(emColor(128,128,128));
    emPainter p;
    static emPainter::SharedPixelFormat pf; setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap(); p.BytesPerRow = CW * 4; p.PixelFormat = &pf;
    p.ClipX1=0; p.ClipY1=0; p.ClipX2=CW; p.ClipY2=CH;
    p.OriginX=0; p.OriginY=0; p.ScaleX=100.0; p.ScaleY=100.0;
    p.UserSpaceMutex=NULL; p.USMLockedByThisThread=NULL;
    static char fake_model[4096]; memset(fake_model,0,sizeof(fake_model));
    void* fm=(void*)fake_model; memcpy(&p.Model,&fm,sizeof(void*));

    emImageColoredTexture tex(0.1, 0.1, 0.6, 0.6, glyphImg, 0, 0, GW, GH,
        emColor(0,0,0,0), emColor(0,0,0,166),
        emTexture::EXTEND_ZERO, emTexture::DQ_3X3, emTexture::UQ_ADAPTIVE);

    emPainter::ScanlineTool sct(p);
    sct.Init(tex, emColor(128,128,128,255));

    long long TDX = sct.GetTDX(), TDY = sct.GetTDY();
    long long TX = sct.GetTX(), TY = sct.GetTY();
    
    // C++ Rust coordinate comparison for a specific row
    int row = 30, x_start = 25, w = 10;
    
    sct.CallInterpolate(x_start, row, w);
    const unsigned char* cpp_buf = sct.GetInterpolationBuffer();
    
    // Simulate C++ sliding window to extract exact (imgX, ox) per pixel
    long long tx_init = (long long)x_start * TDX - TX - 0x2800000LL;
    int cpp_imgX = (int)(tx_init >> 24);
    long long tx_loop = (tx_init & 0xffffff) + 0x3000000LL;
    
    // Compute Rust-equivalent transform
    // tx_sub = 0.1 * 100.0 + 0 = 10.0
    double tx_sub = 10.0;
    double tdx_f64 = ((long long)32 << 24) / (0.6 * 100.0);
    long long tx_origin = (long long)((tx_sub - 0.5) * tdx_f64);
    int px = 10;
    long long base_x = (long long)px * TDX - tx_origin;
    
    printf("C++ TDX=%lld TX=%lld\n", TDX, TX);
    printf("Rust tdx=%lld base_x=%lld tx_origin=%lld\n", TDX, base_x, tx_origin);
    printf("C++ Init TX=%lld, Rust tx_origin=%lld (should match)\n", TX, tx_origin);
    
    printf("\nPer-pixel coordinate comparison (row=%d):\n", row);
    printf("%4s %8s %8s %6s | %8s %8s %6s | %4s %4s\n",
           "x", "cpp_ix", "cpp_ox", "cpp_v", "rst_ix", "rst_ox", "rst_v", "d_ix", "d_ox");
    
    for (int i = 0; i < w; i++) {
        // C++ coordinates
        while (tx_loop >= 0) { tx_loop -= 0x1000000LL; cpp_imgX++; }
        unsigned cpp_ox = (unsigned)((tx_loop + 0x1007fffLL) >> 16);
        int cpp_ix = cpp_imgX - 3; // leftmost column of 4-tap window
        
        // Rust coordinates
        int c = x_start + i;
        long long tx64 = (long long)(c - px) * TDX + base_x - 0x1800000LL;
        int rust_ix = (int)(tx64 >> 24);
        unsigned rust_ox = (unsigned)(((unsigned)(tx64 & 0xFFFFFF) + 0x7FFF) >> 16);
        
        // C++ iy
        long long ty_cpp = (long long)row * TDY - TY - 0x1800000LL;
        int cpp_iy = (int)(ty_cpp >> 24);
        unsigned cpp_oy = (unsigned)(((unsigned)(ty_cpp & 0xffffff) + 0x7fff) >> 16);
        
        // Rust iy
        long long ty_origin = (long long)((tx_sub - 0.5) * tdx_f64); // same formula for Y
        long long base_y = (long long)px * TDY - ty_origin; // assuming square
        long long ty64 = (long long)(row - px) * TDY + base_y - 0x1800000LL;
        int rust_iy = (int)(ty64 >> 24);
        unsigned rust_oy = (unsigned)(((unsigned)(ty64 & 0xFFFFFF) + 0x7FFF) >> 16);
        
        // Get values
        unsigned char rust_val = rust_sample_adaptive_lum(
            gmap, GW, GH, rust_ix, rust_iy, rust_ox, rust_oy, 0, 0, GW, GH
        );
        
        printf("%4d %8d %8u %6d | %8d %8u %6d | %4d %4d\n",
               c, cpp_ix, cpp_ox, cpp_buf[i], rust_ix, rust_ox, rust_val,
               rust_ix - cpp_ix, (int)rust_ox - (int)cpp_ox);
        
        tx_loop += TDX;
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    return 0;
}
