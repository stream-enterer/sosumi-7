#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emTexture.h>
#include "emPainter_ScTl.h"

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
    for (int y = 0; y < GH; y++) {
        for (int x = 0; x < GW; x++) {
            double dx = (x - 16.0) / 16.0;
            double dy = (y - 16.0) / 16.0;
            double d = sqrt(dx*dx + dy*dy);
            gmap[y * GW + x] = d < 0.8 ? (unsigned char)(255 * (1.0 - d / 0.8)) : 0;
        }
    }

    const int CW = 200, CH = 100;
    emImage canvas; canvas.Setup(CW, CH, 4);
    canvas.Fill(emColor(128, 128, 128));

    emPainter p;
    static emPainter::SharedPixelFormat pf; setup_pixel_format(pf);
    p.Map = (void*)canvas.GetMap(); p.BytesPerRow = CW * 4; p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0; p.ClipX2 = CW; p.ClipY2 = CH;
    p.OriginX = 0; p.OriginY = 0; p.ScaleX = 100.0; p.ScaleY = 100.0;
    p.UserSpaceMutex = NULL; p.USMLockedByThisThread = NULL;
    static char fake_model[4096]; memset(fake_model, 0, sizeof(fake_model));
    void* fm = (void*)fake_model; memcpy(&p.Model, &fm, sizeof(void*));

    emImageColoredTexture tex(
        0.1, 0.1, 0.6, 0.6,
        glyphImg, 0, 0, GW, GH,
        emColor(0,0,0,0), emColor(0, 0, 0, 166),
        emTexture::EXTEND_ZERO, emTexture::DQ_3X3, emTexture::UQ_AREA_SAMPLING
    );

    emPainter::ScanlineTool sct(p);
    if (!sct.Init(tex, emColor(128,128,128,255))) {
        printf("Init failed\n");
        void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
        return 1;
    }

    printf("Channels=%d ImgW=%d ImgH=%d ImgDX=%zd ImgDY=%zd\n",
           sct.GetChannels(), sct.GetImgW(), sct.GetImgH(),
           sct.GetImgDX(), sct.GetImgDY());
    printf("TDX=%lld TDY=%lld TX=%lld TY=%lld ODX=%u ODY=%u\n",
           (long long)sct.GetTDX(), (long long)sct.GetTDY(),
           (long long)sct.GetTX(), (long long)sct.GetTY(),
           sct.GetODX(), sct.GetODY());

    // Print multiple rows of interpolation buffer
    for (int row = 10; row <= 40; row += 5) {
        sct.CallInterpolate(10, row, 60);
        const unsigned char* buf = sct.GetInterpolationBuffer();
        printf("Row %d interp: ", row);
        for (int i = 0; i < 60; i += 5) {
            printf("[%d]=%d ", 10+i, buf[i]);
        }
        printf("\n");
    }

    // Check: what does the glyph look like at the center?
    printf("\nGlyph center region (y=14..18, x=14..18):\n");
    for (int y = 14; y <= 18; y++) {
        for (int x = 14; x <= 18; x++) {
            printf("  (%d,%d)=%d", x, y, gmap[y*GW+x]);
        }
        printf("\n");
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    return 0;
}
