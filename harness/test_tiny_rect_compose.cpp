// Test: composed sub-pixel PaintRect calls (tiny-text pattern).
// Compares C++ PaintRect with Rust PaintRect for many overlapping
// sub-pixel-height rectangles, matching the HowTo text rendering pattern.
//
// C++ uses ScanlineTool (PaintScanlineCol) with integer opacity triple.
// Rust uses SubPixelEdges + blend_with_coverage/fill_span_blended.
// This test isolates whether the coverage models produce different
// cumulative results.

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>

int main() {
    // Setup: 100x50 canvas, gray background (matching checkbox view bg).
    const int CW = 100, CH = 50;

    // C++ canvas
    emImage cpp_canvas;
    cpp_canvas.Setup(CW, CH, 4);
    unsigned char* cmap = (unsigned char*)cpp_canvas.GetMap();
    for (int i = 0; i < CW * CH; i++) {
        cmap[i*4] = 128; cmap[i*4+1] = 128; cmap[i*4+2] = 128; cmap[i*4+3] = 255;
    }

    // Rust canvas (copy)
    unsigned char* rust_canvas = (unsigned char*)malloc(CW * CH * 4);
    memcpy(rust_canvas, cmap, CW * CH * 4);

    // Create C++ painter
    emPainter p;
    p.Map = (void*)cpp_canvas.GetMap();
    p.BytesPerRow = CW * 4;

    // Setup pixel format
    emPainter::SharedPixelFormat pf;
    memset(&pf, 0, sizeof(pf));
    pf.BytesPerPixel = 4;
    pf.RedRange = 255; pf.GreenRange = 255; pf.BlueRange = 255;
    pf.RedShift = 0; pf.GreenShift = 8; pf.BlueShift = 16;
    pf.RedHash = malloc(256*256*4);
    pf.GreenHash = malloc(256*256*4);
    pf.BlueHash = malloc(256*256*4);
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
    p.PixelFormat = &pf;
    p.ClipX1 = 0; p.ClipY1 = 0;
    p.ClipX2 = CW; p.ClipY2 = CH;
    p.OriginX = 0; p.OriginY = 0;
    p.ScaleX = 1.0; p.ScaleY = 1.0; // 1:1 scale (coords ARE pixels)
    p.UserSpaceMutex = NULL;
    p.USMLockedByThisThread = NULL;
    static char fake_model[4096];
    memset(fake_model, 0, sizeof(fake_model));
    void* fm = (void*)fake_model;
    memcpy(&p.Model, &fm, sizeof(void*));

    // Paint parameters matching HowTo tiny text:
    // - color: rgb(239,240,244) alpha=166 reduced to alpha/3 = 56
    // - canvas_color: TRANSPARENT (source-over path)
    // - Each rect: x varies, y increments by ~0.275, w varies, h = 0.275
    emColor text_color(239, 240, 244, 56);

    // Simulate 8 lines of tiny text overlapping at pixel y=25
    // Each line is at a different y, h=0.275, covering different x ranges
    double base_y = 24.7;
    double line_h = 0.275;
    double rects[][4] = {
        // {x, y, w, h} — word-sized runs at various positions
        {10.0, base_y + 0*line_h, 5.2, line_h},  // line 0: "How to"
        {10.0, base_y + 1*line_h, 4.9, line_h},  // line 1: "####"
        {10.0, base_y + 3*line_h, 10.4, line_h},  // line 3: "Here is some text..."
        {10.0, base_y + 4*line_h, 10.8, line_h},  // line 4: "multiple sections..."
        {10.0, base_y + 5*line_h, 10.4, line_h},  // line 5: "each other..."
        {10.0, base_y + 7*line_h, 0.7, line_h},   // line 7: "FOCUS"
        {10.0, base_y + 9*line_h, 10.9, line_h},  // line 9: "This panel..."
        {10.0, base_y + 10*line_h, 11.1, line_h}, // line 10: "indicated..."
    };
    int nrects = sizeof(rects) / sizeof(rects[0]);

    // C++ rendering: PaintRect with canvasColor=0 (TRANSPARENT)
    for (int i = 0; i < nrects; i++) {
        p.PaintRect(rects[i][0], rects[i][1], rects[i][2], rects[i][3],
                    text_color, 0);
    }

    // Print C++ results at key pixels
    unsigned char* cpp_row = (unsigned char*)cpp_canvas.GetMap();
    printf("C++ results at y=25 (overlapping tiny rects):\n");
    for (int x = 10; x <= 22; x++) {
        int off = (25 * CW + x) * 4;
        printf("  (%d,25): rgb(%d,%d,%d)\n", x, cpp_row[off], cpp_row[off+1], cpp_row[off+2]);
    }
    printf("\nC++ results at y=24:\n");
    for (int x = 10; x <= 15; x++) {
        int off = (24 * CW + x) * 4;
        printf("  (%d,24): rgb(%d,%d,%d)\n", x, cpp_row[off], cpp_row[off+1], cpp_row[off+2]);
    }
    printf("\nC++ results at y=26:\n");
    for (int x = 10; x <= 15; x++) {
        int off = (26 * CW + x) * 4;
        printf("  (%d,26): rgb(%d,%d,%d)\n", x, cpp_row[off], cpp_row[off+1], cpp_row[off+2]);
    }

    // Also dump the raw opacity values C++ computes for each rect at pixel (12,25)
    printf("\n--- Per-rect analysis at pixel (12,25) ---\n");
    for (int i = 0; i < nrects; i++) {
        double rx = rects[i][0], ry = rects[i][1], rw = rects[i][2], rh = rects[i][3];
        // SubPixelEdges equivalent
        int fx1 = (int)(rx * 0x1000);
        int fy1 = (int)(ry * 0x1000);
        int fx2 = (int)((rx+rw) * 0x1000);
        int fy2 = (int)((ry+rh) * 0x1000);

        int ix1 = fx1 >> 12;
        int iy1 = fy1 >> 12;
        int ixe = (fx2 + 0xfff) >> 12;
        int iy2 = fy2 >> 12; // C++ truncate

        int ax1 = 0x1000 - (fx1 & 0xfff);
        int ax2 = (fx2 + 0xfff) & 0xfff;
        ax2 = (ax2 + 1);
        int ay1 = 0x1000 - (fy1 & 0xfff);
        int ay2 = fy2 & 0xfff;

        if (iy1 >= iy2) {
            ay1 += ay2 - 0x1000;
            ay2 = 0;
        }

        // Does pixel (12,25) fall within this rect?
        int px = 12, py = 25;
        if (px >= ix1 && px < ixe && py >= iy1 && py <= iy2) {
            int alpha_y;
            if (py == iy1 && ay1 < 0x1000) alpha_y = ay1;
            else if (py == iy2 && ay2 > 0) alpha_y = ay2;
            else alpha_y = 0x1000;

            int alpha_x;
            if (px == ix1) alpha_x = ax1;
            else if (px == ixe - 1) alpha_x = ax2;
            else alpha_x = 0x1000;

            int combined = (alpha_x * alpha_y + 0x7ff) >> 12;
            // C++ opacity: (color.alpha * combined + 0x800) >> 12
            int opacity = (56 * combined + 0x800) >> 12;
            printf("  rect %d: y=%.3f..%.3f iy1=%d iy2=%d ay1=%d ay2=%d alpha_y=%d alpha_x=%d cov=%d opacity=%d\n",
                   i, ry, ry+rh, iy1, iy2, ay1, ay2, alpha_y, alpha_x, combined, opacity);
        } else {
            printf("  rect %d: y=%.3f..%.3f -- pixel (12,25) outside\n", i, ry, ry+rh);
        }
    }

    void* np = nullptr; memcpy(&p.Model, &np, sizeof(void*));
    free(rust_canvas);
    return 0;
}
