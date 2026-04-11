// Debug: what exact values does C++ PaintScanline write for the divergent pixels?
// Focus on row=10, x=43-50 (the divergent region)

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

static unsigned char* load_tga_rgba(const char* path, int* out_w, int* out_h) {
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;
    unsigned char header[18];
    fread(header, 1, 18, f);
    int id_len = header[0], w = header[12] | (header[13] << 8),
        h = header[14] | (header[15] << 8), desc = header[17];
    if (id_len > 0) fseek(f, id_len, SEEK_CUR);
    int npix = w * h;
    unsigned char* pixels = (unsigned char*)malloc(npix * 4);
    int idx = 0;
    while (idx < npix) {
        unsigned char rep; fread(&rep, 1, 1, f);
        int count = (rep & 0x7F) + 1;
        if (rep & 0x80) {
            unsigned char bgra[4]; fread(bgra, 1, 4, f);
            for (int i = 0; i < count && idx < npix; i++, idx++) {
                pixels[idx*4]=bgra[2]; pixels[idx*4+1]=bgra[1];
                pixels[idx*4+2]=bgra[0]; pixels[idx*4+3]=bgra[3];
            }
        } else {
            for (int i = 0; i < count && idx < npix; i++, idx++) {
                unsigned char bgra[4]; fread(bgra, 1, 4, f);
                pixels[idx*4]=bgra[2]; pixels[idx*4+1]=bgra[1];
                pixels[idx*4+2]=bgra[0]; pixels[idx*4+3]=bgra[3];
            }
        }
    }
    fclose(f);
    if (!(desc & 0x20)) {
        for (int y = 0; y < h/2; y++) {
            unsigned char *r1=pixels+y*w*4, *r2=pixels+(h-1-y)*w*4;
            for (int x = 0; x < w*4; x++) { unsigned char t=r1[x]; r1[x]=r2[x]; r2[x]=t; }
        }
    }
    *out_w = w; *out_h = h;
    return pixels;
}

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
    int IW, IH;
    unsigned char* tga = load_tga_rgba(
        "/home/a0/git/eaglemode-0.96.4/res/emCore/toolkit/GroupBorder.tga", &IW, &IH);
    if (!tga) return 1;

    emImage srcImg; srcImg.Setup(IW, IH, 4);
    memcpy((void*)srcImg.GetMap(), tga, IW*IH*4);

    const int CW=800, CH=600;
    emImage canvas; canvas.Setup(CW, CH, 4);
    memset((void*)canvas.GetMap(), 0, CW*CH*4);

    emPainter p;
    static emPainter::SharedPixelFormat pf;
    setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap(); p.BytesPerRow = CW*4; p.PixelFormat = &pf;
    p.ClipX1=0; p.ClipY1=0; p.ClipX2=CW; p.ClipY2=CH;
    p.OriginX=0; p.OriginY=0; p.ScaleX=800.0; p.ScaleY=800.0;
    p.UserSpaceMutex=NULL; p.USMLockedByThisThread=NULL;
    static char fake_model[4096]; memset(fake_model,0,sizeof(fake_model));
    void* fm=(void*)fake_model; memcpy(&p.Model,&fm,sizeof(void*));

    emImageTexture tex(0.013026, 0.013026, 0.096974, 0.096974, srcImg, 255,
                       emTexture::EXTEND_EDGE, emTexture::DQ_3X3, emTexture::UQ_AREA_SAMPLING);

    emPainter::ScanlineTool sct(p);
    if (!sct.Init(tex, 0)) {
        printf("Init failed\n");
        void* np=nullptr; memcpy(&p.Model,&np,sizeof(void*));
        free(tga); return 1;
    }

    // Row 10: top edge row, ay1=2373
    int row = 10;
    int ax1 = 2373, ay1 = 2373, ax2 = 4096;
    int iw = 78, ix = 10;
    int a1_top = (ax1 * ay1 + 0x7ff) >> 12;
    int a2_top = (ax2 * ay1 + 0x7ff) >> 12;

    printf("Row %d: a1_top=%d ay1=%d a2_top=%d\n", row, a1_top, ay1, a2_top);

    // Get interpolation output first
    sct.CallInterpolate(ix, row, iw);
    const unsigned char* interp = sct.GetInterpolationBuffer();

    // Show interpolation values for x=43..50
    for (int x = 43; x <= 50; x++) {
        int off = (x - ix) * 4;
        printf("  Interp[x=%d]: rgba(%d,%d,%d,%d)", x,
               interp[off], interp[off+1], interp[off+2], interp[off+3]);

        // Compute what source-over would produce on transparent canvas:
        // opacity for x=43 (interior pixel, so opacity = ay1 = 2373)
        int o = ay1;
        int sr = (interp[off] * o + 0x800) >> 12;
        int sg = (interp[off+1] * o + 0x800) >> 12;
        int sb = (interp[off+2] * o + 0x800) >> 12;
        int sa = (interp[off+3] * o + 0x800) >> 12;
        printf("  scaled(o=%d): rgba(%d,%d,%d,%d)\n", o, sr, sg, sb, sa);
    }

    // Now actually paint and see what C++ writes
    sct.CallPaintScanline(ix, row, iw, a1_top, ay1, a2_top);
    unsigned char* dest = (unsigned char*)canvas.GetMap() + row * CW * 4;
    printf("\nC++ PaintScanline output:\n");
    for (int x = 40; x <= 55; x++) {
        printf("  x=%d: rgba(%d,%d,%d,%d)\n", x,
               dest[x*4], dest[x*4+1], dest[x*4+2], dest[x*4+3]);
    }

    void* np=nullptr; memcpy(&p.Model,&np,sizeof(void*));
    free(tga);
    return 0;
}
