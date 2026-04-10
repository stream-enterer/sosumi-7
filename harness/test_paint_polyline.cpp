// Compare C++ emPainter::PaintPolyline vs Rust rust_paint_polyline.
//
// Build:
//   cargo build -p em-harness
//   g++ -std=c++11 -O2 \
//     -I ~/git/eaglemode-0.96.4/include \
//     -L ~/git/eaglemode-0.96.4/lib \
//     -L target/debug \
//     -o harness/test_paint_polyline \
//     harness/test_paint_polyline.cpp \
//     -lemCore -lem_harness \
//     -Wl,-rpath,$HOME/git/eaglemode-0.96.4/lib \
//     -Wl,-rpath,target/debug

#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <cstdint>

#include <emCore/emScheduler.h>
#include <emCore/emContext.h>
#include <emCore/emPainter.h>
#include <emCore/emImage.h>
#include <emCore/emStroke.h>
#include <emCore/emStrokeEnd.h>

extern "C" int rust_paint_polyline(
    uint8_t *canvas, int canvas_w, int canvas_h,
    double scale_x, double scale_y,
    double offset_x, double offset_y,
    const double* vertices, int n_vertices,
    double thickness,
    uint32_t stroke_color,
    int rounded,
    uint32_t canvas_color
);

struct TestCase {
    const char* name;
    int canvas_w, canvas_h;
    double scale_x, scale_y, offset_x, offset_y;
    const double* vertices;
    int n_vertices;
    double thickness;
    uint32_t stroke_color; // packed RGBA
    bool rounded;
    uint32_t canvas_color; // packed RGBA (0 = transparent)
    uint32_t bg_color;     // pre-fill color (0 = black)
};

static int compare_buffers(const char* name,
                           const uint8_t* cpp_buf, const uint8_t* rust_buf,
                           int w, int h, bool verbose = false) {
    int diffs = 0;
    int first_x = -1, first_y = -1;
    uint8_t first_cpp[4] = {}, first_rust[4] = {};
    int max_diff = 0;
    for (int y = 0; y < h; y++) {
        for (int x = 0; x < w; x++) {
            int off = (y * w + x) * 4;
            if (memcmp(cpp_buf + off, rust_buf + off, 3) != 0) {
                if (diffs == 0) {
                    first_x = x; first_y = y;
                    memcpy(first_cpp, cpp_buf + off, 4);
                    memcpy(first_rust, rust_buf + off, 4);
                }
                for (int c = 0; c < 3; c++) {
                    int d = abs((int)cpp_buf[off+c] - (int)rust_buf[off+c]);
                    if (d > max_diff) max_diff = d;
                }
                if (verbose && diffs < 20) {
                    printf("    diff at (%d,%d): C++=[%d,%d,%d] Rust=[%d,%d,%d]\n",
                           x, y,
                           cpp_buf[off], cpp_buf[off+1], cpp_buf[off+2],
                           rust_buf[off], rust_buf[off+1], rust_buf[off+2]);
                }
                diffs++;
            }
        }
    }
    if (diffs == 0) {
        printf("  [PASS] %s: RGB-identical (%dx%d)\n", name, w, h);
    } else {
        printf("  [FAIL] %s: %d divergent pixels (max_diff=%d) out of %d\n",
               name, diffs, max_diff, w * h);
        if (!verbose)
            printf("    first at (%d,%d): C++=[%d,%d,%d,%d] Rust=[%d,%d,%d,%d]\n",
                   first_x, first_y,
                   first_cpp[0], first_cpp[1], first_cpp[2], first_cpp[3],
                   first_rust[0], first_rust[1], first_rust[2], first_rust[3]);
    }
    return diffs == 0 ? 0 : 1;
}

static void fill_bg(uint8_t* buf, int fb_size, uint32_t bg_color) {
    if (bg_color) {
        uint8_t r = (bg_color >> 24) & 0xFF;
        uint8_t g = (bg_color >> 16) & 0xFF;
        uint8_t b = (bg_color >> 8) & 0xFF;
        uint8_t a = bg_color & 0xFF;
        for (int i = 0; i < fb_size; i += 4) {
            buf[i+0] = r; buf[i+1] = g; buf[i+2] = b; buf[i+3] = a;
        }
    } else {
        memset(buf, 0, fb_size);
    }
}

int main() {
    emStandardScheduler scheduler;
    emRootContext rootContext(scheduler);

    printf("=== PaintPolyline FFI comparison ===\n");

    // Test 1: Simple horizontal line, butt caps, source-over
    double verts1[] = { 10.0, 25.0, 90.0, 25.0 };

    // Test 2: Horizontal line, round caps
    // (same vertices as test 1)

    // Test 3: 3-vertex checkmark shape, rounded, source-over
    double verts3[] = { 20.0, 60.0, 40.0, 80.0, 80.0, 20.0 };

    // Test 4: Same checkmark but with canvas_color
    // (same vertices as test 3)

    // Test 5: Scaled checkmark (like checkbox golden test)
    double verts5[] = { 0.2, 0.6, 0.4, 0.8, 0.8, 0.2 };

    // Test 6: Diagonal line, butt
    double verts6[] = { 10.0, 10.0, 90.0, 90.0 };

    // Test 7: Diagonal line, round
    // (same vertices as test 6)

    // Test 8: Sub-pixel positioned line
    double verts8[] = { 0.5, 0.5, 9.5, 0.5 };

    TestCase cases[] = {
        {"horizontal_butt", 100, 50, 1.0, 1.0, 0.0, 0.0,
         verts1, 2, 4.0, 0x406080FF, false, 0, 0},

        {"horizontal_round", 100, 50, 1.0, 1.0, 0.0, 0.0,
         verts1, 2, 4.0, 0x406080FF, true, 0, 0},

        {"checkmark_rounded_srcover", 100, 100, 1.0, 1.0, 0.0, 0.0,
         verts3, 3, 8.0, 0x203040FF, true, 0, 0},

        {"checkmark_rounded_canvas", 100, 100, 1.0, 1.0, 0.0, 0.0,
         verts3, 3, 8.0, 0x203040FF, true, 0xD8DAE0FF, 0xD8DAE0FF},

        {"checkmark_scaled_canvas", 100, 100, 100.0, 100.0, 0.0, 0.0,
         verts5, 3, 0.16, 0x203040FF, true, 0xD8DAE0FF, 0xD8DAE0FF},

        {"diagonal_butt", 100, 100, 1.0, 1.0, 0.0, 0.0,
         verts6, 2, 6.0, 0xFF0000FF, false, 0, 0},

        {"diagonal_round", 100, 100, 1.0, 1.0, 0.0, 0.0,
         verts6, 2, 6.0, 0xFF0000FF, true, 0, 0},

        {"subpixel_offset", 10, 4, 1.0, 1.0, 0.0, 0.0,
         verts8, 2, 1.5, 0x808080FF, false, 0, 0},
    };

    int n_cases = (int)(sizeof(cases) / sizeof(cases[0]));
    int failures = 0;

    for (int c = 0; c < n_cases; c++) {
        TestCase& tc = cases[c];
        printf("Test: %s\n", tc.name);

        int fb_size = tc.canvas_w * tc.canvas_h * 4;

        // --- C++ render ---
        emImage cpp_canvas;
        cpp_canvas.Setup(tc.canvas_w, tc.canvas_h, 4);
        fill_bg((uint8_t*)cpp_canvas.GetMap(), fb_size, tc.bg_color);

        emPainter painter(
            rootContext,
            (void*)cpp_canvas.GetMap(),
            tc.canvas_w * 4, 4,
            0xFF, 0xFF00, 0xFF0000,
            0.0, 0.0, (double)tc.canvas_w, (double)tc.canvas_h,
            tc.offset_x, tc.offset_y,
            tc.scale_x, tc.scale_y
        );

        emColor stroke_color = (emUInt32)tc.stroke_color;
        emColor canvas_color = (emUInt32)tc.canvas_color;
        emStroke stroke(stroke_color, tc.rounded);

        // Use CAP ends when rounded (matching checkbox usage), BUTT otherwise
        emStrokeEnd se = tc.rounded ? emStrokeEnd(emStrokeEnd::CAP) : emStrokeEnd();
        painter.PaintPolyline(
            tc.vertices, tc.n_vertices, tc.thickness,
            stroke,
            se, se,
            canvas_color
        );

        // --- Rust render ---
        uint8_t* rust_canvas = (uint8_t*)malloc(fb_size);
        fill_bg(rust_canvas, fb_size, tc.bg_color);

        rust_paint_polyline(
            rust_canvas, tc.canvas_w, tc.canvas_h,
            tc.scale_x, tc.scale_y, tc.offset_x, tc.offset_y,
            tc.vertices, tc.n_vertices, tc.thickness,
            tc.stroke_color, tc.rounded ? 1 : 0, tc.canvas_color
        );

        // --- Compare ---
        failures += compare_buffers(tc.name,
            (const uint8_t*)cpp_canvas.GetMap(), rust_canvas,
            tc.canvas_w, tc.canvas_h, true);

        free(rust_canvas);
    }

    printf("\n%d/%d tests passed.\n", n_cases - failures, n_cases);
    return failures > 0 ? 1 : 0;
}
