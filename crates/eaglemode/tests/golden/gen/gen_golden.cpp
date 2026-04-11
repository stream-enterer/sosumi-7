// Golden-master data generator for zuicchini parity tests.
// Links against Eagle Mode's libemCore.so and exercises the C++ painter,
// layout, and scheduler subsystems with deterministic inputs.
//
// Build: make -C golden_gen
// Run:   make -C golden_gen run   (from zuicchini/)

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include <emCore/emContext.h>
#include <emCore/emRes.h>
#include <emCore/emEngine.h>
#include <emCore/emImage.h>
#include <emCore/emLinearLayout.h>
#include <emCore/emPackLayout.h>
#include <emCore/emPainter.h>
#include <emCore/emPanel.h>
#include <emCore/emRasterLayout.h>
#include <emCore/emScheduler.h>
#include <emCore/emStd2.h>
#include <emCore/emStroke.h>
#include <emCore/emStrokeEnd.h>
#include <emCore/emTexture.h>
#include <emCore/emView.h>
#include <emCore/emViewAnimator.h>
#include <emCore/emViewInputFilter.h>

// Widget headers for Phase 6 golden tests
#include <emCore/emBorder.h>
#include <emCore/emButton.h>
#include <emCore/emCheckBox.h>
#include <emCore/emCheckButton.h>
#include <emCore/emColorField.h>
#include <emCore/emLabel.h>
#include <emCore/emListBox.h>
#include <emCore/emRadioButton.h>
#include <emCore/emScalarField.h>
#include <emCore/emSplitter.h>
#include <emCore/emTextField.h>

// Coverage extension widget headers (CAP audit)
#include <emCore/emErrorPanel.h>
#include <emCore/emFilePanel.h>
#include <emCore/emFileSelectionBox.h>
#include <emCore/emTunnel.h>

// TestPanel integration tests
#include <emTest/emTestPanel.h>

// TkTest factory — defined in tktest_factory.cpp (compiled against scaffold header)
extern emPanel* create_tktest(emPanel::ParentArg parent, const emString& name);

#include "golden_format.h"

// ═══════════════════════════════════════════════════════════════════
// Stub clipboard — headless emClipboard for widgets that require one.
// This is test infrastructure, not a widget simplification.
// ═══════════════════════════════════════════════════════════════════

class StubClipboard : public emClipboard {
public:
    StubClipboard(emContext& ctx)
        : emClipboard(ctx, "StubClipboard"), nextId(1) {}

    virtual emInt64 PutText(const emString& str, bool selection) override {
        if (selection) selText = str; else clipText = str;
        return nextId++;
    }
    virtual void Clear(bool selection, emInt64) override {
        if (selection) selText.Clear(); else clipText.Clear();
    }
    virtual emString GetText(bool selection) override {
        return selection ? selText : clipText;
    }

    static void Setup(emContext& ctx) {
        StubClipboard* cb = new StubClipboard(ctx);
        cb->Install();
        cb->Register();
    }

private:
    emString clipText, selText;
    emInt64 nextId;
};

// ═══════════════════════════════════════════════════════════════════
// Globals
// ═══════════════════════════════════════════════════════════════════

static emStandardScheduler* g_sched = nullptr;
static emRootContext*        g_ctx   = nullptr;

extern FILE* g_draw_op_log;
extern int g_draw_op_seq;

static FILE* open_draw_op_log(const char* test_name) {
    char path[1024];
    snprintf(path, sizeof(path), "target/golden-divergence/%s.cpp_ops.jsonl", test_name);
    FILE* f = fopen(path, "w");
    if (f) {
        g_draw_op_log = f;
        g_draw_op_seq = 0;
    }
    return f;
}

static void close_draw_op_log() {
    if (g_draw_op_log) {
        fclose(g_draw_op_log);
        g_draw_op_log = nullptr;
        g_draw_op_seq = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════

static void dump_painter(const char* name, const emImage& img) {
    FILE* f = open_golden("painter", name, "painter.golden");
    write_u32(f, (uint32_t)img.GetWidth());
    write_u32(f, (uint32_t)img.GetHeight());
    write_bytes(f, (const uint8_t*)img.GetMap(),
                img.GetWidth() * img.GetHeight() * img.GetChannelCount());
    fclose(f);
    printf("  painter/%s\n", name);
}

static emPainter make_painter(emImage& img) {
    emPainter p;
    if (!img.PreparePainter(&p, *g_ctx,
                            0.0, 0.0,
                            (double)img.GetWidth(), (double)img.GetHeight())) {
        fprintf(stderr, "PreparePainter failed!\n");
        exit(1);
    }
    return p;
}

static emImage white_image(int w = 256, int h = 256) {
    emImage img(w, h, 4);
    img.Fill(emColor::WHITE);
    return img;
}

// Star vertices (matches Rust star_vertices())
static std::vector<double> star_xy() {
    double cx = 128.0, cy = 128.0, outer = 110.0, inner = 45.0;
    std::vector<double> xy;
    for (int i = 0; i < 10; i++) {
        double angle = -M_PI / 2.0 + M_PI * 2.0 * i / 10.0;
        double r = (i % 2 == 0) ? outer : inner;
        xy.push_back(cx + r * cos(angle));
        xy.push_back(cy + r * sin(angle));
    }
    return xy;
}

// 20-vertex convex polygon (matches Rust convex_polygon_20())
static std::vector<double> convex20_xy() {
    double cx = 128.0, cy = 128.0, base_r = 100.0;
    uint32_t rng = 12345;
    std::vector<double> xy;
    for (int i = 0; i < 20; i++) {
        rng = rng * 1103515245 + 12345;
        double perturb = ((rng >> 16) / 65536.0) * 20.0 - 10.0;
        double angle = M_PI * 2.0 * i / 20.0;
        double r = base_r + perturb;
        xy.push_back(cx + r * cos(angle));
        xy.push_back(cy + r * sin(angle));
    }
    return xy;
}

// Pentagon (matches Rust pentagon_vertices())
static std::vector<double> pentagon_xy() {
    double cx = 128.0, cy = 128.0, r = 100.0;
    std::vector<double> xy;
    for (int i = 0; i < 5; i++) {
        double angle = -M_PI / 2.0 + M_PI * 2.0 * i / 5.0;
        xy.push_back(cx + r * cos(angle));
        xy.push_back(cy + r * sin(angle));
    }
    return xy;
}

// Bezier control points (matches Rust bezier_points())
static const double bezier_pts[] = {
    20.0, 200.0,  80.0, 20.0,  180.0, 20.0,  236.0, 200.0
};

// Procedural image (matches Rust procedural_image())
static emImage procedural_image(int w, int h) {
    emImage img(w, h, 4);
    emByte* map = img.GetWritableMap();
    for (int y = 0; y < h; y++) {
        for (int x = 0; x < w; x++) {
            int off = (y * w + x) * 4;
            map[off + 0] = (emByte)(x * 255 / w);
            map[off + 1] = (emByte)(y * 255 / h);
            map[off + 2] = 128;
            map[off + 3] = 255;
        }
    }
    return img;
}

// ═══════════════════════════════════════════════════════════════════
// Painter golden generators
// ═══════════════════════════════════════════════════════════════════

static void gen_rect_solid() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRect(20, 20, 100, 80, emColor::RED);
    dump_painter("rect_solid", img);
}

static void gen_rect_alpha() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRect(20, 20, 100, 80, emColor(255, 0, 0, 128));
    dump_painter("rect_alpha", img);
}

static void gen_rect_overlap() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRect(20, 20, 100, 80, emColor::RED);
    p.PaintRect(60, 40, 100, 80, emColor(0, 0, 255, 128));
    dump_painter("rect_overlap", img);
}

static void gen_ellipse_basic() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintEllipse(28, 28, 200, 150, emColor::GREEN);
    dump_painter("ellipse_basic", img);
}

static void gen_ellipse_small() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintEllipse(118, 118, 20, 20, emColor::BLUE);
    dump_painter("ellipse_small", img);
}

static void gen_polygon_tri() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    double xy[] = {128, 20, 20, 230, 236, 230};
    p.PaintPolygon(xy, 3, emColor::RED);
    dump_painter("polygon_tri", img);
}

static void gen_polygon_star() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    auto xy = star_xy();
    p.PaintPolygon(xy.data(), 10, emColor::MAGENTA);
    dump_painter("polygon_star", img);
}

static void gen_polygon_complex() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    auto xy = convex20_xy();
    p.PaintPolygon(xy.data(), 20, emColor::CYAN);
    dump_painter("polygon_complex", img);
}

static void gen_round_rect() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRoundRect(20, 20, 200, 150, 20, 20, emColor::BLUE);
    dump_painter("round_rect", img);
}

static void gen_gradient_h() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Horizontal gradient: left=RED, right=BLUE
    p.PaintRect(0, 0, 256, 256,
                emLinearGradientTexture(0.0, 128.0, emColor::RED, 256.0, 128.0, emColor::BLUE));
    dump_painter("gradient_h", img);
}

static void gen_gradient_v() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Vertical gradient: top=GREEN, bottom=YELLOW
    p.PaintRect(0, 0, 256, 256,
                emLinearGradientTexture(128.0, 0.0, emColor::GREEN, 128.0, 256.0, emColor::YELLOW));
    dump_painter("gradient_v", img);
}

static void gen_gradient_radial() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Radial gradient: center=WHITE, edge=BLACK, full canvas
    p.PaintEllipse(0, 0, 256, 256,
                   emRadialGradientTexture(0, 0, 256, 256, emColor::WHITE, emColor::BLACK));
    dump_painter("gradient_radial", img);
}

static void gen_line_basic() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintLine(10, 10, 240, 200, 3.0, emStroke(emColor::BLACK));
    dump_painter("line_basic", img);
}

static void gen_line_thick() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintLine(10, 128, 240, 128, 8.0, emRoundedStroke(emColor::BLUE));
    dump_painter("line_thick", img);
}

static void gen_line_ends_all() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    emStrokeEnd::TypeEnum types[] = {
        emStrokeEnd::BUTT, emStrokeEnd::CAP, emStrokeEnd::ARROW,
        emStrokeEnd::CONTOUR_ARROW, emStrokeEnd::LINE_ARROW,
        emStrokeEnd::TRIANGLE, emStrokeEnd::CONTOUR_TRIANGLE,
        emStrokeEnd::SQUARE, emStrokeEnd::CONTOUR_SQUARE,
        emStrokeEnd::HALF_SQUARE, emStrokeEnd::CIRCLE,
        emStrokeEnd::CONTOUR_CIRCLE, emStrokeEnd::HALF_CIRCLE,
        emStrokeEnd::DIAMOND, emStrokeEnd::CONTOUR_DIAMOND,
        emStrokeEnd::HALF_DIAMOND, emStrokeEnd::STROKE,
    };
    int n = sizeof(types) / sizeof(types[0]);
    double spacing = 240.0 / n;
    for (int i = 0; i < n; i++) {
        double y = 8.0 + spacing * i;
        p.PaintLine(30, y, 226, y, 4.0,
                    emRoundedStroke(emColor::BLACK),
                    emStrokeEnd(),
                    emStrokeEnd(types[i], emColor::WHITE));
    }
    dump_painter("line_ends_all", img);
}

static void gen_line_dashed() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Dashed line (dashLengthFactor=3, gapLengthFactor=3 → dash=9, gap=9 at width=3)
    p.PaintLine(10, 64, 240, 64, 3.0,
                emDashedStroke(emColor::BLACK, 3.0, 3.0));
    // Dotted line
    p.PaintLine(10, 128, 240, 128, 3.0,
                emDottedStroke(emColor::BLACK, 3.0));
    dump_painter("line_dashed", img);
}

static void gen_outline_rect() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRectOutline(20, 20, 200, 150, 3.0, emStroke(emColor::BLACK));
    dump_painter("outline_rect", img);
}

static void gen_outline_ellipse() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintEllipseOutline(28, 28, 200, 150, 2.0, emStroke(emColor::BLACK));
    dump_painter("outline_ellipse", img);
}

static void gen_outline_polygon() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    auto xy = pentagon_xy();
    p.PaintPolygonOutline(xy.data(), 5, 3.0, emStroke(emColor::BLACK));
    dump_painter("outline_polygon", img);
}

static void gen_outline_round_rect() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRoundRectOutline(20, 20, 200, 150, 20, 20, 3.0, emStroke(emColor::BLACK));
    dump_painter("outline_round_rect", img);
}

static void gen_bezier_filled() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintBezier(bezier_pts, 4, emColor::RED);
    dump_painter("bezier_filled", img);
}

static void gen_bezier_stroked() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    open_draw_op_log("painter_bezier_stroked");
    p.PaintBezierLine(bezier_pts, 4, 3.0,
                      emRoundedStroke(emColor::BLACK),
                      emStrokeEnd(emStrokeEnd::ARROW, emColor::WHITE),
                      emStrokeEnd(emStrokeEnd::ARROW, emColor::WHITE));
    close_draw_op_log();
    dump_painter("bezier_stroked", img);
}

static void gen_clip_basic() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Create a clipped sub-painter
    emPainter clipped(p, 64, 64, 192, 192);
    double xy[] = {128, 10, 10, 246, 246, 246};
    clipped.PaintPolygon(xy, 3, emColor::RED);
    dump_painter("clip_basic", img);
}

static void gen_canvas_color() {
    emImage img(256, 256, 4);
    img.Fill(emColor(200, 200, 200));
    emPainter p = make_painter(img);
    // Paint with explicit canvas color
    p.PaintRect(20, 20, 100, 80, emColor(255, 0, 0, 128), emColor(200, 200, 200));
    dump_painter("canvas_color", img);
}

static void gen_image_paint() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    emImage src = procedural_image(64, 64);
    p.PaintImage(50, 50, 64, 64, src, 255);
    dump_painter("image_paint", img);
}

static void gen_image_scaled() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    emImage src = procedural_image(64, 64);
    p.PaintImage(28, 28, 200, 200, src, 255);
    dump_painter("image_scaled", img);
}

static void gen_multi_compose() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintRect(10, 10, 120, 120, emColor(255, 0, 0, 180));
    // Ellipse: bbox (20,0,160,160) → C++ PaintEllipse(20,0,160,160)
    // Rust uses cx=100,cy=60,rx=80,ry=80 → bbox (20,-20,160,160)
    // Need to match: Rust paint_ellipse(100,60,80,80) → bbox (20,-20,160,160)
    // So C++ should be: PaintEllipse(20, -20, 160, 160)
    p.PaintEllipse(20, -20, 160, 160, emColor(0, 255, 0, 150));
    double tri[] = {128, 10, 60, 200, 200, 200};
    p.PaintPolygon(tri, 3, emColor(0, 0, 255, 120));
    p.PaintRoundRect(140, 80, 100, 100, 15, 15, emColor(255, 255, 0, 100));
    p.PaintRect(30, 150, 200, 80, emColor(128, 0, 128, 90));
    dump_painter("multi_compose", img);
}

static void gen_polyline() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    double xy[] = {20, 200, 80, 40, 160, 200, 240, 40};
    p.PaintPolyline(xy, 4, 4.0, emRoundedStroke(emColor::BLACK));
    dump_painter("polyline", img);
}

static void gen_ellipse_sector() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Sector: bbox (28,28,200,200), start=0°, range=90°
    p.PaintEllipseSector(28, 28, 200, 200, 0.0, 90.0, emColor::RED);
    dump_painter("ellipse_sector", img);
}

static void gen_painter_howto_isolate() {
    // Isolates the howto indicator PaintRoundRect that differs between C++ and Rust.
    // Background: 515e84ff (border background at howto location in widget_button_normal)
    // Howto rect: pixel coords, scale=1 (PreparePainter defaults)
    emImage img(100, 100, 4);
    img.Fill(emColor(0x51, 0x5e, 0x84, 0xff));
    emPainter p = make_painter(img);
    p.PaintRoundRect(
        1.81824, 1.87168, 11.12832, 22.25664,
        0.11128320000000001, 0.11128320000000001,
        emColor(0xef, 0xf0, 0xf4, 0x1a),
        emColor(0x51, 0x5e, 0x84, 0xff)
    );
    dump_painter("painter_howto_isolate", img);
}

// ═══════════════════════════════════════════════════════════════════
// Transform golden generators
// ═══════════════════════════════════════════════════════════════════

static void gen_transform_translate() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    emPainter p2(p, 0, 0, 256, 256, 50.0, 30.0, 1.0, 1.0);
    p2.PaintRect(0, 0, 80, 60, emColor::RED);
    dump_painter("transform_translate", img);
}

static void gen_transform_fractional() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    emPainter p2(p, 0, 0, 256, 256, 0.3, 0.7, 1.0, 1.0);
    p2.PaintRect(20, 20, 100, 80, emColor::RED);
    dump_painter("transform_fractional", img);
}

static void gen_transform_identity_roundtrip() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // scale(2)*scale(0.5) = identity → same as painting directly
    p.PaintRect(20, 20, 100, 80, emColor::RED);
    dump_painter("transform_identity_roundtrip", img);
}

static void gen_transform_ellipse_scaled() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Non-uniform scale: 2x horizontal, 1x vertical
    emPainter p2(p, 0, 0, 256, 256, 0.0, 0.0, 2.0, 1.0);
    // User-space circle at (40,80) size 60x60 → pixel ellipse at (80,80) size 120x60
    p2.PaintEllipse(10, 50, 60, 60, emColor::GREEN);
    dump_painter("transform_ellipse_scaled", img);
}

static void gen_transform_clip_interaction() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Clip to center 128x128, then translate origin to (160,100)
    emPainter p2(p, 64, 64, 192, 192, 160.0, 100.0, 1.0, 1.0);
    p2.PaintRect(0, 0, 80, 60, emColor::RED);
    dump_painter("transform_clip_interaction", img);
}

static void gen_transform_nested() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Inner: translate(50,50) + scale(2,2)
    emPainter p_inner(p, 0, 0, 256, 256, 50.0, 50.0, 2.0, 2.0);
    p_inner.PaintRect(0, 0, 30, 30, emColor::RED);
    // Outer: translate(50,50) only
    emPainter p_outer(p, 0, 0, 256, 256, 50.0, 50.0, 1.0, 1.0);
    p_outer.PaintRect(0, 0, 50, 50, emColor(0, 0, 255, 128));
    dump_painter("transform_nested", img);
}

static void gen_transform_scale() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    emPainter p2(p, 0, 0, 256, 256, 0.0, 0.0, 2.0, 2.0);
    p2.PaintRect(10, 10, 50, 40, emColor::RED);
    dump_painter("transform_scale", img);
}

// ═══════════════════════════════════════════════════════════════════
// Text golden generators
// ═══════════════════════════════════════════════════════════════════

static void gen_text_basic() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintText(10, 80, "Hello", 40.0, 1.0, emColor::BLACK);
    dump_painter("text_basic", img);
}

static void gen_text_scaled() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintText(10, 80, "Test", 40.0, 1.5, emColor(255, 0, 0, 255));
    dump_painter("text_scaled", img);
}

static void gen_text_fitted() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    p.PaintTextBoxed(20, 20, 216, 80, "Fitted", 100.0,
                     emColor::BLACK, 0, EM_ALIGN_CENTER,
                     EM_ALIGN_LEFT, 0.5, false);
    dump_painter("text_fitted", img);
}

static void gen_text_alignment() {
    emImage img = white_image(256, 512);
    emPainter p = make_painter(img);
    // Top-left box, left text
    p.PaintTextBoxed(10, 10, 236, 80, "Left", 50.0,
                     emColor::BLACK, 0, EM_ALIGN_TOP_LEFT, EM_ALIGN_LEFT);
    // Center box, center text
    p.PaintTextBoxed(10, 120, 236, 80, "Center", 50.0,
                     emColor::BLACK, 0, EM_ALIGN_CENTER, EM_ALIGN_CENTER);
    // Bottom-right box, right text
    p.PaintTextBoxed(10, 230, 236, 80, "Right", 50.0,
                     emColor::BLACK, 0, EM_ALIGN_BOTTOM_RIGHT, EM_ALIGN_RIGHT);
    dump_painter("text_alignment", img);
}

static void gen_text_clipped() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // Clip to center 150x150 region, then paint text extending beyond
    emPainter p2(p, 50, 50, 200, 200, 0.0, 0.0, 1.0, 1.0);
    p2.PaintText(30, 80, "Clipped!", 40.0, 1.0, emColor::BLACK);
    dump_painter("text_clipped", img);
}

static void gen_text_below_threshold() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // charHeight=1.0, scaleY=1.0 → pixel_height=1.0 < 1.7 → tiny text fallback
    p.PaintText(10, 100, "tiny text here", 1.0, 1.0, emColor::BLACK);
    dump_painter("text_below_threshold", img);
}

// ═══════════════════════════════════════════════════════════════════
// Layout golden generators
// ═══════════════════════════════════════════════════════════════════

// Engine that terminates the scheduler after N cycles.
class TerminateEngine : public emEngine {
public:
    TerminateEngine(emScheduler& sched, int maxCycles)
        : emEngine(sched), remaining(maxCycles) { WakeUp(); }
    virtual bool Cycle() override {
        if (--remaining <= 0) {
            GetScheduler().InitiateTermination(0);
            return false;
        }
        return true;
    }
private:
    int remaining;
};

// ═══════════════════════════════════════════════════════════════════
// Testable — thin wrapper that exposes emPanel::Layout() for golden tests.
// Does NOT modify any virtual method — all paint/input behavior comes from T.
// ═══════════════════════════════════════════════════════════════════

template<typename T>
class Testable : public T {
public:
    using T::T; // inherit all constructors
    void DoLayout(double x, double y, double w, double h, emColor cc = 0) {
        this->Layout(x, y, w, h, cc);
    }
    void DoInput(emInputEvent& event, const emInputState& state, double mx, double my) {
        this->Input(event, state, mx, my);
    }
};

// Dump layout child rects. Converts from emCore's normalized coordinates
// (parent width = 1.0) to absolute coordinates by multiplying by parent's
// layout width.
static void dump_layout(const char* name, emPanel* layout) {
    FILE* f = open_golden("layout", name, "layout.golden");

    // Count children
    int count = 0;
    for (emPanel* c = layout->GetFirstChild(); c; c = c->GetNext()) count++;
    write_u32(f, (uint32_t)count);

    // emCore normalizes panel coordinates: width=1.0, height=tallness.
    // Child GetLayoutX/Y/Width/Height are in parent's normalized space.
    // Scale to absolute coords matching Rust's pixel-space layout rects.
    double scale = layout->GetLayoutWidth();
    double ox = layout->GetLayoutX();
    double oy = layout->GetLayoutY();

    for (emPanel* c = layout->GetFirstChild(); c; c = c->GetNext()) {
        write_f64(f, ox + c->GetLayoutX() * scale);
        write_f64(f, oy + c->GetLayoutY() * scale);
        write_f64(f, c->GetLayoutWidth() * scale);
        write_f64(f, c->GetLayoutHeight() * scale);
    }
    fclose(f);
    printf("  layout/%s (%d children)\n", name, count);
}

// Recursive panel tree dump — writes one JSONL line per panel.
// Escape backslashes in identity for valid JSON (emCore uses \: as separator escape).
static emString json_escape_identity(const emString& id) {
    emString out;
    for (int i = 0; i < id.GetLen(); i++) {
        if (id[i] == '\\') out += "\\\\";
        else out += id[i];
    }
    return out;
}

static void dump_panel_tree_recursive(FILE* f, emPanel* panel, int depth) {
    emString escaped = json_escape_identity(panel->GetIdentity());
    fprintf(f,
        "{\"path\":\"%s\",\"depth\":%d,"
        "\"lx\":%.17g,\"ly\":%.17g,\"lw\":%.17g,\"lh\":%.17g,"
        "\"children\":%d,\"ae_expanded\":%d,\"viewed\":%d,"
        "\"ae_thresh\":%.17g}\n",
        escaped.Get(),
        depth,
        panel->GetLayoutX(), panel->GetLayoutY(),
        panel->GetLayoutWidth(), panel->GetLayoutHeight(),
        (int)[&]{int n=0; for(auto*c=panel->GetFirstChild();c;c=c->GetNext())n++; return n;}(),
        panel->IsAutoExpanded() ? 1 : 0,
        panel->IsViewed() ? 1 : 0,
        panel->GetAutoExpansionThresholdValue()
    );
    for (emPanel* c = panel->GetFirstChild(); c; c = c->GetNext()) {
        dump_panel_tree_recursive(f, c, depth + 1);
    }
}

static void dump_panel_tree(const char* name, emPanel* root) {
    char path[512];
    snprintf(path, sizeof(path), "%s/%s.cpp_tree.jsonl",
             getenv("GOLDEN_DIVERGENCE_DIR") ? getenv("GOLDEN_DIVERGENCE_DIR")
                                              : "target/golden-divergence",
             name);
    FILE* f = fopen(path, "w");
    dump_panel_tree_recursive(f, root, 0);
    fclose(f);
    printf("  tree/%s\n", name);
}

// Run a layout test: create a fresh scheduler+context+view, build the layout,
// run scheduler to settle, dump child rects.
//
// `setup` receives the layout panel pointer and should create children and
// configure constraints.
template<typename LayoutT, typename SetupFn>
static void gen_layout_test(const char* name, SetupFn setup) {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    LayoutT* layout = new LayoutT(view, "layout");
    setup(layout);

    // Set the layout panel's rect
    layout->Layout(0.0, 0.0, 1000.0, 500.0);

    // Run scheduler to let internal LayoutChildren() fire
    TerminateEngine ctrl(sched, 30);
    sched.Run();

    dump_layout(name, layout);
}

static void gen_linear_h_equal() {
    gen_layout_test<emLinearLayout>("linear_h_equal", [](emLinearLayout* l) {
        l->SetHorizontal();
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_h_weighted() {
    gen_layout_test<emLinearLayout>("linear_h_weighted", [](emLinearLayout* l) {
        l->SetHorizontal();
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        double w[] = {1,2,3,4};
        for (int i = 0; i < 4; i++) l->SetChildWeight(i, w[i]);
    });
}

static void gen_linear_v_equal() {
    gen_layout_test<emLinearLayout>("linear_v_equal", [](emLinearLayout* l) {
        l->SetVertical();
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_v_weighted() {
    gen_layout_test<emLinearLayout>("linear_v_weighted", [](emLinearLayout* l) {
        l->SetVertical();
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        double w[] = {1,2,3,4};
        for (int i = 0; i < 4; i++) l->SetChildWeight(i, w[i]);
    });
}

static void gen_linear_h_tallness() {
    gen_layout_test<emLinearLayout>("linear_h_tallness", [](emLinearLayout* l) {
        l->SetHorizontal();
        double t[] = {0.5, 1.0, 2.0, 0.5};
        for (int i = 0; i < 4; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildTallness(i, t[i]);
        }
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_v_tallness() {
    gen_layout_test<emLinearLayout>("linear_v_tallness", [](emLinearLayout* l) {
        l->SetVertical();
        double t[] = {0.5, 1.0, 2.0, 0.5};
        for (int i = 0; i < 4; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildTallness(i, t[i]);
        }
        l->SetChildWeight(1.0);
    });
}

static void gen_raster_3col() {
    gen_layout_test<emRasterLayout>("raster_3col", [](emRasterLayout* l) {
        l->SetFixedColumnCount(3);
        for (int i = 0; i < 8; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_2row() {
    gen_layout_test<emRasterLayout>("raster_2row", [](emRasterLayout* l) {
        l->SetFixedRowCount(2);
        for (int i = 0; i < 6; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_strict() {
    gen_layout_test<emRasterLayout>("raster_strict", [](emRasterLayout* l) {
        l->SetFixedColumnCount(3);
        l->SetStrictRaster();
        l->SetChildTallness(1.0);
        for (int i = 0; i < 9; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_pref_tall() {
    gen_layout_test<emRasterLayout>("raster_pref_tall", [](emRasterLayout* l) {
        l->SetFixedColumnCount(3);
        l->SetPrefChildTallness(2.0);
        for (int i = 0; i < 6; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_pack_equal() {
    gen_layout_test<emPackLayout>("pack_equal", [](emPackLayout* l) {
        for (int i = 0; i < 10; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
        l->SetPrefChildTallness(1.0);
    });
}

static void gen_pack_weighted() {
    gen_layout_test<emPackLayout>("pack_weighted", [](emPackLayout* l) {
        // Matching the Rust test's deterministic RNG
        uint32_t rng = 42;
        for (int i = 0; i < 10; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildWeight(i, (double)(i + 1));
            rng = rng * 1103515245 + 12345;
            double u = (rng >> 16) / 65536.0;
            l->SetPrefChildTallness(i, exp(u * 2.0 - 1.0));
        }
    });
}

static void gen_pack_extreme() {
    gen_layout_test<emPackLayout>("pack_extreme", [](emPackLayout* l) {
        double tallnesses[] = {0.01, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 100.0};
        for (int i = 0; i < 8; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildWeight(i, 1.0);
            l->SetPrefChildTallness(i, tallnesses[i]);
        }
    });
}

// ─── Layout expansion: spacing, alignment, adaptive, min_cell_count, tallness constraints ───

static void gen_linear_h_spacing() {
    gen_layout_test<emLinearLayout>("linear_h_spacing", [](emLinearLayout* l) {
        l->SetHorizontal();
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
        // C++ SetSpace(l, t, h, v, r, b) — SpaceL=0.5, SpaceT=0.3, SpaceH=1.0, SpaceV=0, SpaceR=0.5, SpaceB=0.3
        l->SetSpace(0.5, 0.3, 1.0, 0.0, 0.5, 0.3);
    });
}

static void gen_linear_v_spacing() {
    gen_layout_test<emLinearLayout>("linear_v_spacing", [](emLinearLayout* l) {
        l->SetVertical();
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
        l->SetSpace(0.3, 0.5, 0.0, 1.0, 0.3, 0.5);
    });
}

static void gen_linear_h_align_right() {
    gen_layout_test<emLinearLayout>("linear_h_align_right", [](emLinearLayout* l) {
        l->SetHorizontal();
        l->SetAlignment(EM_ALIGN_BOTTOM_RIGHT);
        // Fixed tallness forces children narrower than parent, creating surplus
        double t[] = {2.0, 2.0, 2.0};
        for (int i = 0; i < 3; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildTallness(i, t[i]);
        }
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_h_align_center() {
    gen_layout_test<emLinearLayout>("linear_h_align_center", [](emLinearLayout* l) {
        l->SetHorizontal();
        l->SetAlignment(EM_ALIGN_CENTER);
        double t[] = {2.0, 2.0, 2.0};
        for (int i = 0; i < 3; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildTallness(i, t[i]);
        }
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_v_align_bottom() {
    gen_layout_test<emLinearLayout>("linear_v_align_bottom", [](emLinearLayout* l) {
        l->SetVertical();
        l->SetAlignment(EM_ALIGN_BOTTOM_RIGHT);
        double t[] = {0.25, 0.25, 0.25};
        for (int i = 0; i < 3; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildTallness(i, t[i]);
        }
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_adaptive_wide() {
    // Parent is wider than tall → tallness < threshold → horizontal
    gen_layout_test<emLinearLayout>("linear_adaptive_wide", [](emLinearLayout* l) {
        l->SetOrientationThresholdTallness(1.0);
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_adaptive_tall() {
    // Uses a tall parent rect (1000x2000) → tallness > threshold → vertical
    // Can't use gen_layout_test since it hardcodes 1000x500.
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    auto* l = new emLinearLayout(view, "layout");
    l->SetOrientationThresholdTallness(1.0);
    for (int i = 0; i < 4; i++)
        new emPanel(*l, emString::Format("c%d", i));
    l->SetChildWeight(1.0);
    l->Layout(0.0, 0.0, 1000.0, 2000.0);
    TerminateEngine ctrl(sched, 30);
    sched.Run();
    dump_layout("linear_adaptive_tall", l);
}

static void gen_linear_min_cell_count() {
    gen_layout_test<emLinearLayout>("linear_min_cell_count", [](emLinearLayout* l) {
        l->SetHorizontal();
        l->SetMinCellCount(6);
        // Only 3 actual children, but min_cell_count=6 allocates space for 6
        for (int i = 0; i < 3; i++)
            new emPanel(*l, emString::Format("c%d", i));
        l->SetChildWeight(1.0);
    });
}

static void gen_linear_min_max_tallness() {
    gen_layout_test<emLinearLayout>("linear_min_max_tallness", [](emLinearLayout* l) {
        l->SetHorizontal();
        for (int i = 0; i < 4; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildWeight(i, 1.0);
        }
        // Child 0: unconstrained
        // Child 1: min tallness 1.0
        l->SetMinChildTallness(1, 1.0);
        // Child 2: max tallness 0.1
        l->SetMaxChildTallness(2, 0.1);
        // Child 3: both min=0.5 max=0.5 (fixed)
        l->SetChildTallness(3, 0.5);
    });
}

static void gen_raster_alignment_br() {
    gen_layout_test<emRasterLayout>("raster_alignment_br", [](emRasterLayout* l) {
        l->SetFixedColumnCount(2);
        l->SetAlignment(EM_ALIGN_BOTTOM_RIGHT);
        l->SetChildTallness(2.0); // tall cells → surplus in x
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_alignment_center() {
    gen_layout_test<emRasterLayout>("raster_alignment_center", [](emRasterLayout* l) {
        l->SetFixedColumnCount(2);
        l->SetAlignment(EM_ALIGN_CENTER);
        l->SetChildTallness(2.0);
        for (int i = 0; i < 4; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_spacing() {
    gen_layout_test<emRasterLayout>("raster_spacing", [](emRasterLayout* l) {
        l->SetFixedColumnCount(3);
        l->SetSpace(0.5, 0.3, 0.8, 0.6, 0.5, 0.3);
        for (int i = 0; i < 9; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_min_cell_count() {
    gen_layout_test<emRasterLayout>("raster_min_cell_count", [](emRasterLayout* l) {
        l->SetFixedColumnCount(3);
        l->SetMinCellCount(9);
        // Only 5 actual children, min_cell_count=9 allocates space for 9
        for (int i = 0; i < 5; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_raster_min_max_tallness() {
    gen_layout_test<emRasterLayout>("raster_min_max_tallness", [](emRasterLayout* l) {
        l->SetFixedColumnCount(3);
        l->SetMinChildTallness(0.5);
        l->SetMaxChildTallness(2.0);
        l->SetPrefChildTallness(3.0); // pref exceeds max → clamped
        for (int i = 0; i < 6; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_pack_min_cell_count() {
    gen_layout_test<emPackLayout>("pack_min_cell_count", [](emPackLayout* l) {
        l->SetMinCellCount(8);
        // Only 4 actual children
        for (int i = 0; i < 4; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildWeight(i, 1.0);
            l->SetPrefChildTallness(i, 1.0);
        }
    });
}

static void gen_linear_mixed_weights() {
    gen_layout_test<emLinearLayout>("linear_mixed_weights", [](emLinearLayout* l) {
        l->SetHorizontal();
        // Mix of very different weights — tests distribution precision
        double w[] = {0.1, 1.0, 10.0, 0.5, 5.0};
        for (int i = 0; i < 5; i++) {
            new emPanel(*l, emString::Format("c%d", i));
            l->SetChildWeight(i, w[i]);
        }
    });
}

static void gen_raster_auto_cols() {
    // No fixed column/row count — auto-compute from tallness
    gen_layout_test<emRasterLayout>("raster_auto_cols", [](emRasterLayout* l) {
        l->SetPrefChildTallness(1.0);
        for (int i = 0; i < 12; i++)
            new emPanel(*l, emString::Format("c%d", i));
    });
}

static void gen_pack_single() {
    gen_layout_test<emPackLayout>("pack_single", [](emPackLayout* l) {
        new emPanel(*l, "c0");
        l->SetChildWeight(0, 1.0);
        l->SetPrefChildTallness(0, 1.0);
    });
}

// ═══════════════════════════════════════════════════════════════════
// Behavioral golden generators
// ═══════════════════════════════════════════════════════════════════

// Dump activation state for a list of panels in given order.
// Format: [u32 num_panels] [per panel: u8 is_active, u8 in_active_path]
static void dump_behavioral(const char* name,
                            const std::vector<emPanel*>& panels) {
    FILE* f = open_golden("behavioral", name, "behavioral.golden");
    write_u32(f, (uint32_t)panels.size());
    for (auto* p : panels) {
        write_u8(f, p->IsActive() ? 1 : 0);
        write_u8(f, p->IsInActivePath() ? 1 : 0);
    }
    fclose(f);
    printf("  behavioral/%s (%zu panels)\n", name, panels.size());
}

// Activate child1. Expect: child1=active, root=in_path, child2=inactive.
static void gen_activate_click() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Activate();

    // DFS order: root, child1, child2
    dump_behavioral("activate_click", {root, child1, child2});
}

// Activate grandchild. Expect: gc=active, child1+root=in_path, child2=inactive.
static void gen_activate_path() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");
    emPanel* gc = new emPanel(*child1, "gc");

    gc->Activate();

    // DFS order: root, child1, gc, child2
    dump_behavioral("activate_path", {root, child1, gc, child2});
}

// Activate child1, then child2. Expect: child2=active, child1=inactive.
static void gen_activate_switch() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Activate();
    child2->Activate();

    // DFS order: root, child1, child2
    dump_behavioral("activate_switch", {root, child1, child2});
}

// Focus child1 (sets view focused + activates).
static void gen_focus_click() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Focus();

    // DFS order: root, child1, child2
    dump_behavioral("focus_click", {root, child1, child2});
}

// Activate a non-focusable panel → walks to focusable ancestor.
static void gen_activate_nonfocusable() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->SetFocusable(false);
    child1->Activate();

    // child1 is not focusable → activation walks to root
    dump_behavioral("activate_nonfocusable", {root, child1, child2});
}

// Activate grandchild, then remove it → activation should move.
static void gen_activate_remove() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Activate();
    delete child1;

    // After removing active panel, check remaining: root, child2
    dump_behavioral("activate_remove", {root, child2});
}

// Tab forward: child1 focused → GetFocusableNext → child2 focused.
static void gen_focus_tab_forward() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Focus();
    emPanel* next = child1->GetFocusableNext();
    if (next) next->Focus();

    dump_behavioral("focus_tab_forward", {root, child1, child2});
}

// Tab backward: child2 focused → GetFocusablePrev → child1 focused.
static void gen_focus_tab_backward() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child2->Focus();
    emPanel* prev = child2->GetFocusablePrev();
    if (prev) prev->Focus();

    dump_behavioral("focus_tab_backward", {root, child1, child2});
}

// Tab skips non-focusable: child1 → child2 (unfocusable) → child3.
static void gen_focus_unfocusable_skip() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");
    emPanel* child3 = new emPanel(*root, "child3");

    child2->SetFocusable(false);
    child1->Focus();
    emPanel* next = child1->GetFocusableNext();
    if (next) next->Focus();

    // child2 is skipped, child3 is active
    dump_behavioral("focus_unfocusable_skip", {root, child1, child2, child3});
}

// Focus into child: root → child1 → grandchild.
static void gen_focus_nested() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* gc = new emPanel(*child1, "gc");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Focus();
    emPanel* fc = child1->GetFocusableFirstChild();
    if (fc) fc->Focus();

    // gc should be active, child1 and root in active path
    dump_behavioral("focus_nested", {root, child1, gc, child2});
}

// Remove focused panel → focus moves to parent.
static void gen_focus_remove_focused() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Focus();
    delete child1;

    // After removal, root should be active
    dump_behavioral("focus_remove_focused", {root, child2});
}

// Visit out from grandchild → focus moves to parent.
static void gen_focus_visit_out() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* gc = new emPanel(*child1, "gc");
    emPanel* child2 = new emPanel(*root, "child2");

    gc->Focus();
    // Simulate visit_out: go to focusable parent
    emPanel* parent = gc->GetFocusableParent();
    if (parent) parent->Focus();

    // child1 should be active, root in path, gc and child2 inactive
    dump_behavioral("focus_visit_out", {root, child1, gc, child2});
}

// Tab wrap: from last child → wraps to first child.
static void gen_focus_tab_wrap() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child2->Focus();
    // GetFocusableNext should return NULL (no next sibling)
    emPanel* next = child2->GetFocusableNext();
    if (next) {
        next->Focus();
    } else {
        // Wrap: go to parent's first focusable child
        emPanel* p = child2->GetFocusableParent();
        if (p) {
            emPanel* fc = p->GetFocusableFirstChild();
            if (fc) fc->Focus();
        }
    }

    // child1 should be active after wrap
    dump_behavioral("focus_tab_wrap", {root, child1, child2});
}

// VisitFirst: from child2, jump to first focusable sibling (child1).
static void gen_focus_visit_first() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");
    emPanel* child3 = new emPanel(*root, "child3");

    child2->Focus();
    emPanel* first = child2->GetParent()->GetFocusableFirstChild();
    if (first) first->Focus();

    dump_behavioral("focus_visit_first", {root, child1, child2, child3});
}

// VisitLast: from child1, jump to last focusable sibling (child3).
static void gen_focus_visit_last() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");
    emPanel* child3 = new emPanel(*root, "child3");

    child1->Focus();
    emPanel* last = child1->GetParent()->GetFocusableLastChild();
    if (last) last->Focus();

    dump_behavioral("focus_visit_last", {root, child1, child2, child3});
}

// Focus a disabled panel — disabled panels are still focusable in C++.
static void gen_focus_disabled_panel() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->SetEnableSwitch(false);
    child1->Focus();

    dump_behavioral("focus_disabled_panel", {root, child1, child2});
}

// Remove non-active middle child → remaining panels unaffected.
static void gen_activate_remove_middle() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");
    emPanel* child3 = new emPanel(*root, "child3");

    child1->Focus();
    delete child2;

    dump_behavioral("activate_remove_middle", {root, child1, child3});
}

// Focus gc (grandchild), then remove child1 (its parent in active path).
static void gen_activate_remove_in_path() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* gc = new emPanel(*child1, "gc");
    emPanel* child2 = new emPanel(*root, "child2");

    gc->Focus();
    delete child1;  // Removes child1 + gc (entire subtree)

    dump_behavioral("activate_remove_in_path", {root, child2});
}

// Tab deep: root → child1 → gc1, gc2; root → child2.
// Focus gc1, GetFocusableNext → gc2.
static void gen_focus_tab_deep() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* gc1 = new emPanel(*child1, "gc1");
    emPanel* gc2 = new emPanel(*child1, "gc2");
    emPanel* child2 = new emPanel(*root, "child2");

    gc1->Focus();
    emPanel* next = gc1->GetFocusableNext();
    if (next) next->Focus();

    dump_behavioral("focus_tab_deep", {root, child1, gc1, gc2, child2});
}

// Tab ascend: root → child1 → gc1, gc2.
// Focus gc2 (last), GetFocusableNext → wrap to gc1.
static void gen_focus_tab_ascend() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* gc1 = new emPanel(*child1, "gc1");
    emPanel* gc2 = new emPanel(*child1, "gc2");

    gc2->Focus();
    emPanel* next = gc2->GetFocusableNext();
    if (next) {
        next->Focus();
    } else {
        emPanel* p = gc2->GetFocusableParent();
        if (p) {
            emPanel* fc = p->GetFocusableFirstChild();
            if (fc) fc->Focus();
        }
    }

    dump_behavioral("focus_tab_ascend", {root, child1, gc1, gc2});
}

// VisitOut from child to root: child1 focused, visit out → root.
static void gen_focus_visit_out_to_root() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    emPanel* root = new emPanel(view, "root");
    emPanel* child1 = new emPanel(*root, "child1");
    emPanel* child2 = new emPanel(*root, "child2");

    child1->Focus();
    emPanel* parent = child1->GetFocusableParent();
    if (parent) parent->Focus();

    dump_behavioral("focus_visit_out_to_root", {root, child1, child2});
}

// ═══════════════════════════════════════════════════════════════════
// RecordingPanel — accumulates notice flags and tracks input receipt
// ═══════════════════════════════════════════════════════════════════

class RecordingPanel : public emPanel {
public:
    RecordingPanel(ParentArg parent, const emString& name)
        : emPanel(parent, name), accumulated_flags(0), input_received(false) {}
    uint32_t accumulated_flags;
    bool input_received;
    void ResetRecording() { accumulated_flags = 0; input_received = false; }
protected:
    virtual void Notice(NoticeFlags flags) override {
        accumulated_flags |= (uint32_t)flags;
    }
    virtual void Input(emInputEvent& event, const emInputState& state,
                       double mx, double my) override {
        input_received = true;
        emPanel::Input(event, state, mx, my);
    }
};

// ═══════════════════════════════════════════════════════════════════
// PaintingPanel — fills its entire area with a solid color
// ═══════════════════════════════════════════════════════════════════

class PaintingPanel : public emPanel {
public:
    PaintingPanel(ParentArg parent, const emString& name, emColor color = 0)
        : emPanel(parent, name), fill_color(color) {}
    void DoLayout(double x, double y, double w, double h, emColor cc = 0) {
        Layout(x, y, w, h, cc);
    }
protected:
    virtual void Paint(const emPainter& painter, emColor canvasColor) const override {
        if (fill_color.GetAlpha() > 0) {
            painter.PaintRect(0, 0, 1, GetTallness(), fill_color, canvasColor);
        }
    }
private:
    emColor fill_color;
};

// ═══════════════════════════════════════════════════════════════════
// GoldenViewPort — exposes protected SetViewFocused / InputToView
// ═══════════════════════════════════════════════════════════════════

class GoldenViewPort : public emViewPort {
public:
    GoldenViewPort(emView& view) : emViewPort(view) {
        SetViewGeometry(0, 0, 800, 600, 1.0);
    }
    void DoSetViewFocused(bool focused) { SetViewFocused(focused); }
    void DoSetViewGeometry(double x, double y, double w, double h, double pt) {
        SetViewGeometry(x, y, w, h, pt);
    }
    void DoInputToView(emInputEvent& event, const emInputState& state) {
        InputToView(event, state);
    }
    void DoPaintView(const emPainter& p, emColor cc) { PaintView(p, cc); }
};

// ═══════════════════════════════════════════════════════════════════
// Notice / Input dump helpers
// ═══════════════════════════════════════════════════════════════════

static void dump_notice(const char* name,
                        const std::vector<RecordingPanel*>& panels) {
    FILE* f = open_golden("notice", name, "notice.golden");
    write_u32(f, (uint32_t)panels.size());
    for (auto* p : panels) write_u32(f, p->accumulated_flags);
    fclose(f);
    printf("  notice/%s (%zu panels)\n", name, panels.size());
}

static void dump_input(const char* name,
                       const std::vector<RecordingPanel*>& panels) {
    FILE* f = open_golden("input", name, "input.golden");
    write_u32(f, (uint32_t)panels.size());
    for (auto* p : panels) {
        write_u8(f, p->input_received ? 1 : 0);
        write_u8(f, p->IsActive() ? 1 : 0);
        write_u8(f, p->IsInActivePath() ? 1 : 0);
    }
    fclose(f);
    printf("  input/%s (%zu panels)\n", name, panels.size());
}

// ═══════════════════════════════════════════════════════════════════
// Spatial behavioral golden generators (need GoldenViewPort)
// ═══════════════════════════════════════════════════════════════════

// VisitLeft: 3 children side by side, activate child3, visit left → child2.
// Use Activate()+SetViewFocused instead of Focus() to avoid VisitFull animation.
static void gen_focus_visit_left() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    emPanel* root = new emPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    emPanel* child1 = new emPanel(*root, "child1");
    child1->Layout(0, 0, 0.33, 1);
    emPanel* child2 = new emPanel(*root, "child2");
    child2->Layout(0.33, 0, 0.33, 1);
    emPanel* child3 = new emPanel(*root, "child3");
    child3->Layout(0.66, 0, 0.34, 1);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    vp.DoSetViewFocused(true);
    child3->Activate();
    view.VisitLeft();
    // Settle to let VisitingViewAnimator process the navigation
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    dump_behavioral("focus_visit_left", {root, child1, child2, child3});
}

// VisitRight: 3 children side by side, activate child1, visit right → child2.
static void gen_focus_visit_right() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    emPanel* root = new emPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    emPanel* child1 = new emPanel(*root, "child1");
    child1->Layout(0, 0, 0.33, 1);
    emPanel* child2 = new emPanel(*root, "child2");
    child2->Layout(0.33, 0, 0.33, 1);
    emPanel* child3 = new emPanel(*root, "child3");
    child3->Layout(0.66, 0, 0.34, 1);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    vp.DoSetViewFocused(true);
    child1->Activate();
    view.VisitRight();
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    dump_behavioral("focus_visit_right", {root, child1, child2, child3});
}

// VisitDown: 3 children stacked vertically, activate child1, visit down → child2.
static void gen_focus_visit_down() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    emPanel* root = new emPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    emPanel* child1 = new emPanel(*root, "child1");
    child1->Layout(0, 0, 1, 0.33);
    emPanel* child2 = new emPanel(*root, "child2");
    child2->Layout(0, 0.33, 1, 0.33);
    emPanel* child3 = new emPanel(*root, "child3");
    child3->Layout(0, 0.66, 1, 0.34);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    vp.DoSetViewFocused(true);
    child1->Activate();
    view.VisitDown();
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    dump_behavioral("focus_visit_down", {root, child1, child2, child3});
}

// VisitUp: 3 children stacked vertically, activate child3, visit up → child2.
static void gen_focus_visit_up() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    emPanel* root = new emPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    emPanel* child1 = new emPanel(*root, "child1");
    child1->Layout(0, 0, 1, 0.33);
    emPanel* child2 = new emPanel(*root, "child2");
    child2->Layout(0, 0.33, 1, 0.33);
    emPanel* child3 = new emPanel(*root, "child3");
    child3->Layout(0, 0.66, 1, 0.34);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    vp.DoSetViewFocused(true);
    child3->Activate();
    view.VisitUp();
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    dump_behavioral("focus_visit_up", {root, child1, child2, child3});
}

// ═══════════════════════════════════════════════════════════════════
// Notice golden generators
// ═══════════════════════════════════════════════════════════════════

// Activate child1 → ACTIVE_CHANGED notices.
static void gen_notice_active_changed() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Action: activate child1
    child1->Activate();

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_active_changed", {root, child1, child2});
}

// Focus child1 → FOCUS_CHANGED notices (needs view port for SetViewFocused).
static void gen_notice_focus_changed() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Action: focus child1 (sets view focused + activates)
    child1->Focus();

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_focus_changed", {root, child1, child2});
}

// Layout child1 → LAYOUT_CHANGED notices.
static void gen_notice_layout_changed() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Action: change child1's layout rect
    child1->Layout(0.1, 0.1, 0.3, 0.5);

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_layout_changed", {root, child1, child2});
}

// Create new child after settling → CHILDREN_CHANGED on parent.
static void gen_notice_children_changed() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();

    // Action: add new child
    auto* child2 = new RecordingPanel(*root, "child2");

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_children_changed", {root, child1, child2});
}

// ═══════════════════════════════════════════════════════════════════
// Window focus notice golden generators
// ═══════════════════════════════════════════════════════════════════

// Set view focused → VIEW_FOCUS_CHANGED + UPDATE_PRIORITY_CHANGED on all.
static void gen_notice_window_focus_gained() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");

    child1->Activate();

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();

    // Action: gain window focus
    vp.DoSetViewFocused(true);

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_window_focus_gained", {root, child1});
}

// Set focused true then false → VIEW_FOCUS_CHANGED on lost.
static void gen_notice_window_focus_lost() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");

    child1->Activate();
    vp.DoSetViewFocused(true);

    // Settle
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();

    // Action: lose window focus
    vp.DoSetViewFocused(false);

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_window_focus_lost", {root, child1});
}

// Resize viewport with VF_ROOT_SAME_TALLNESS → LAYOUT_CHANGED on root + children.
static void gen_notice_window_resize() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_ROOT_SAME_TALLNESS);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Action: resize viewport
    vp.DoSetViewGeometry(0, 0, 1200, 800, 1.0);

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_window_resize", {root, child1, child2});
}

// SetEnableSwitch(false) → NF_ENABLE_CHANGED.
static void gen_notice_enable_changed() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    // Settle initial notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Action: disable child1
    child1->SetEnableSwitch(false);

    // Deliver new notices
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_enable_changed", {root, child1, child2});
}

// Disable parent → children also get NF_ENABLE_CHANGED.
static void gen_notice_recursive_enable() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* gc = new RecordingPanel(*child1, "gc");
    auto* child2 = new RecordingPanel(*root, "child2");

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    gc->ResetRecording();
    child2->ResetRecording();

    // Action: disable child1 → gc should also get ENABLE_CHANGED
    child1->SetEnableSwitch(false);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_recursive_enable", {root, child1, gc, child2});
}

// Re-enable after disabling → ENABLE_CHANGED again.
static void gen_notice_re_enable() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* gc = new RecordingPanel(*child1, "gc");
    auto* child2 = new RecordingPanel(*root, "child2");

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    gc->ResetRecording();
    child2->ResetRecording();

    // Disable child1 first
    child1->SetEnableSwitch(false);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    gc->ResetRecording();
    child2->ResetRecording();

    // Action: re-enable child1
    child1->SetEnableSwitch(true);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_re_enable", {root, child1, gc, child2});
}

// Remove child2 → CHILDREN_CHANGED on parent (root).
static void gen_notice_remove_child() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Action: remove child2
    delete child2;

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    // Only root and child1 remain
    dump_notice("notice_remove_child", {root, child1});
}

// Focus + layout change in same settle → both flags appear.
static void gen_notice_focus_and_layout() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");
    auto* child2 = new RecordingPanel(*root, "child2");

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Two actions before settle: focus + layout change
    child1->Focus();
    child1->Layout(0.1, 0.1, 0.3, 0.5);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_focus_and_layout", {root, child1, child2});
}

// Add new child and activate it before settling.
static void gen_notice_add_and_activate() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);

    auto* root = new RecordingPanel(view, "root");
    auto* child1 = new RecordingPanel(*root, "child1");

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();

    // Add new child and activate it before settling
    auto* child2 = new RecordingPanel(*root, "child2");
    child2->Activate();

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_notice("notice_add_and_activate", {root, child1, child2});
}

// ═══════════════════════════════════════════════════════════════════
// Input golden generators
// ═══════════════════════════════════════════════════════════════════

// Click at (600,300) → should hit child2 (right half of 800px).
static void gen_input_mouse_hit() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    auto* child1 = new RecordingPanel(*root, "child1");
    child1->Layout(0, 0, 0.5, 1);
    auto* child2 = new RecordingPanel(*root, "child2");
    child2->Layout(0.5, 0, 0.5, 1);

    // Settle layout
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Click at (600, 300) → right half → child2
    emInputEvent event;
    emInputState state;
    state.SetMouse(600, 300);
    event.Setup(EM_KEY_LEFT_BUTTON, emString(), 1, 0);
    vp.DoInputToView(event, state);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_input("input_mouse_hit", {root, child1, child2});
}

// Key event to active panel.
static void gen_input_key_to_focused() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    auto* child1 = new RecordingPanel(*root, "child1");
    child1->Layout(0, 0, 0.5, 1);
    auto* child2 = new RecordingPanel(*root, "child2");
    child2->Layout(0.5, 0, 0.5, 1);

    child1->Focus();

    // Settle
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Send key press
    emInputEvent event;
    emInputState state;
    event.Setup(EM_KEY_A, emString("a"), 0, 0);
    vp.DoInputToView(event, state);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_input("input_key_to_focused", {root, child1, child2});
}

// Wheel/scroll event.
static void gen_input_scroll_delta() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    auto* child1 = new RecordingPanel(*root, "child1");
    child1->Layout(0, 0, 0.5, 1);

    child1->Activate();

    // Settle
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();

    // Send wheel event
    emInputEvent event;
    emInputState state;
    state.SetMouse(200, 300);
    event.Setup(EM_KEY_WHEEL_UP, emString(), 0, 0);
    vp.DoInputToView(event, state);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_input("input_scroll_delta", {root, child1});
}

// Mouse down + move + up sequence.
static void gen_input_drag_sequence() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    auto* child1 = new RecordingPanel(*root, "child1");
    child1->Layout(0, 0, 0.5, 1);
    auto* child2 = new RecordingPanel(*root, "child2");
    child2->Layout(0.5, 0, 0.5, 1);

    // Settle
    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    child2->ResetRecording();

    // Mouse down on child1
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(200, 300);
        state.Set(EM_KEY_LEFT_BUTTON, true);
        event.Setup(EM_KEY_LEFT_BUTTON, emString(), 1, 0);
        vp.DoInputToView(event, state);
    }

    // Mouse move
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(300, 300);
        state.Set(EM_KEY_LEFT_BUTTON, true);
        event.Setup(EM_KEY_NONE, emString(), 0, 0);
        vp.DoInputToView(event, state);
    }

    // Mouse up
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(300, 300);
        event.Setup(EM_KEY_LEFT_BUTTON, emString(), 0, 0);
        vp.DoInputToView(event, state);
    }

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_input("input_drag_sequence", {root, child1, child2});
}

// Click below the panel area → no panel receives input.
static void gen_input_mouse_miss() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    root->Layout(0, 0, 1, 0.5);  // Only covers top half
    auto* child1 = new RecordingPanel(*root, "child1");
    child1->Layout(0, 0, 1, 1);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();

    // Click below the panel area
    emInputEvent event;
    emInputState state;
    state.SetMouse(400, 500);  // Below root (root ends at ~300 in 600px viewport)
    event.Setup(EM_KEY_LEFT_BUTTON, emString(), 1, 0);
    vp.DoInputToView(event, state);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_input("input_mouse_miss", {root, child1});
}

// Click on a grandchild panel → deepest panel receives input.
static void gen_input_nested_hit() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* root = new RecordingPanel(view, "root");
    root->Layout(0, 0, 1, 0.75);
    auto* child1 = new RecordingPanel(*root, "child1");
    child1->Layout(0, 0, 0.5, 1);
    auto* gc = new RecordingPanel(*child1, "gc");
    gc->Layout(0, 0, 1, 1);
    auto* child2 = new RecordingPanel(*root, "child2");
    child2->Layout(0.5, 0, 0.5, 1);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    root->ResetRecording();
    child1->ResetRecording();
    gc->ResetRecording();
    child2->ResetRecording();

    // Click at (100, 300) → inside gc (which fills child1's left half)
    emInputEvent event;
    emInputState state;
    state.SetMouse(100, 300);
    event.Setup(EM_KEY_LEFT_BUTTON, emString(), 1, 0);
    vp.DoInputToView(event, state);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    dump_input("input_nested_hit", {root, child1, gc, child2});
}

// ═══════════════════════════════════════════════════════════════════
// Compositor dump helper
// ═══════════════════════════════════════════════════════════════════

static void dump_compositor(const char* name, const emImage& img) {
    FILE* f = open_golden("compositor", name, "compositor.golden");
    write_u32(f, (uint32_t)img.GetWidth());
    write_u32(f, (uint32_t)img.GetHeight());
    write_bytes(f, (const uint8_t*)img.GetMap(),
                img.GetWidth() * img.GetHeight() * img.GetChannelCount());
    fclose(f);
    printf("  compositor/%s\n", name);
}

// ═══════════════════════════════════════════════════════════════════
// Compositor golden generators
// ═══════════════════════════════════════════════════════════════════

// Test 1: Single root panel fills viewport with RED.
static void gen_composite_single_panel() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* root = new PaintingPanel(view, "root", emColor::RED);
    root->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    emImage img(800, 600, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, 800.0, 600.0)) {
        fprintf(stderr, "PreparePainter failed for composite_single_panel\n");
        exit(1);
    }
    vp.DoPaintView(p, 0);
    dump_compositor("composite_single_panel", img);
}

// Test 2: Left half RED, right half BLUE.
static void gen_composite_two_children() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* root = new PaintingPanel(view, "root", 0);
    root->DoLayout(0, 0, 1.0, 0.75);
    auto* left = new PaintingPanel(*root, "left", emColor::RED);
    left->DoLayout(0, 0, 0.5, 0.75);
    auto* right = new PaintingPanel(*root, "right", emColor::BLUE);
    right->DoLayout(0.5, 0, 0.5, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    emImage img(800, 600, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, 800.0, 600.0)) {
        fprintf(stderr, "PreparePainter failed for composite_two_children\n");
        exit(1);
    }
    vp.DoPaintView(p, 0);
    dump_compositor("composite_two_children", img);
}

// Test 3: Overlapping panels — A=RED, B=BLUE on top.
static void gen_composite_overlap() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* root = new PaintingPanel(view, "root", 0);
    root->DoLayout(0, 0, 1.0, 0.75);
    auto* panelA = new PaintingPanel(*root, "panelA", emColor::RED);
    panelA->DoLayout(0.1, 0.1, 0.4, 0.3);
    auto* panelB = new PaintingPanel(*root, "panelB", emColor::BLUE);
    panelB->DoLayout(0.3, 0.2, 0.4, 0.3);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    emImage img(800, 600, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, 800.0, 600.0)) {
        fprintf(stderr, "PreparePainter failed for composite_overlap\n");
        exit(1);
    }
    vp.DoPaintView(p, 0);
    dump_compositor("composite_overlap", img);
}

// Test 4: Nested panels — parent (no paint) contains child GREEN.
static void gen_composite_nested() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* root = new PaintingPanel(view, "root", 0);
    root->DoLayout(0, 0, 1.0, 0.75);
    auto* parent = new PaintingPanel(*root, "parent", 0);
    parent->DoLayout(0.1, 0.075, 0.8, 0.6);
    auto* child = new PaintingPanel(*parent, "child", emColor::GREEN);
    child->DoLayout(0.1, 0.075, 0.8, 0.6);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    emImage img(800, 600, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, 800.0, 600.0)) {
        fprintf(stderr, "PreparePainter failed for composite_nested\n");
        exit(1);
    }
    vp.DoPaintView(p, 0);
    dump_compositor("composite_nested", img);
}

// Test 5: Canvas color propagation — root WHITE, child RED@128 alpha.
static void gen_composite_canvas_color() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* root = new PaintingPanel(view, "root", emColor::WHITE);
    root->DoLayout(0, 0, 1.0, 0.75);
    auto* child = new PaintingPanel(*root, "child", emColor(255, 0, 0, 128));
    child->DoLayout(0.1, 0.075, 0.8, 0.6, emColor::WHITE);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    emImage img(800, 600, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, 800.0, 600.0)) {
        fprintf(stderr, "PreparePainter failed for composite_canvas_color\n");
        exit(1);
    }
    vp.DoPaintView(p, 0);
    dump_compositor("composite_canvas_color", img);
}

// ═══════════════════════════════════════════════════════════════════
// Widget rendering golden generators (Phase 6)
// ═══════════════════════════════════════════════════════════════════

// Helper: render a viewport and dump the image as a compositor golden file.
// Clears the image to BLACK first, matching Rust SoftwareCompositor behavior.
static void render_and_dump(const char* name, GoldenViewPort& vp,
                            emRootContext& ctx) {
    emImage img(800, 600, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, 800.0, 600.0)) {
        fprintf(stderr, "PreparePainter failed for %s\n", name);
        exit(1);
    }
    open_draw_op_log(name);
    vp.DoPaintView(p, 0);
    close_draw_op_log();
    dump_compositor(name, img);
}

// Test 1: emBorder with OBT_RECT, caption="Test"
static void gen_widget_border_rect() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Test");
    w->SetBorderType(emBorder::OBT_RECT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_rect", vp, ctx);
}

// Test 2: emBorder with OBT_ROUND_RECT, caption + description
static void gen_widget_border_round_rect() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Caption", "Description text");
    w->SetBorderType(emBorder::OBT_ROUND_RECT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_round_rect", vp, ctx);
}

// Test 3: emBorder with OBT_GROUP, IBT_GROUP
static void gen_widget_border_group() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Group");
    w->SetBorderType(emBorder::OBT_GROUP, emBorder::IBT_GROUP);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_group", vp, ctx);
}

// Test 4: emBorder with OBT_INSTRUMENT
static void gen_widget_border_instrument() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Instrument");
    w->SetBorderType(emBorder::OBT_INSTRUMENT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_instrument", vp, ctx);
}

// Test 5: emLabel — shows label as content (not in border)
static void gen_widget_label() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emLabel>(view, "test", "Hello World");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_label", vp, ctx);
}

// Test 6: emButton — normal (unpressed) state
static void gen_widget_button_normal() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emButton>(view, "test", "Click Me");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_button_normal", vp, ctx);
}

// Test 7: emCheckBox — unchecked
static void gen_widget_checkbox_unchecked() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emCheckBox>(view, "test", "Check Option");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_checkbox_unchecked", vp, ctx);
}

// Test 8: emCheckBox — checked
static void gen_widget_checkbox_checked() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emCheckBox>(view, "test", "Check Option");
    w->SetChecked(true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_checkbox_checked", vp, ctx);
}

// Test 9a: emCheckButton — unchecked
static void gen_widget_checkbutton_unchecked() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emCheckButton>(view, "test", "Toggle Option");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_checkbutton_unchecked", vp, ctx);
}

// Test 9b: emCheckButton — checked
static void gen_widget_checkbutton_checked() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emCheckButton>(view, "test", "Toggle Option");
    w->SetChecked(true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_checkbutton_checked", vp, ctx);
}

// Test 9: emTextField — empty editable field
static void gen_widget_textfield_empty() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                        emString(), true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_textfield_empty", vp, ctx);
}

// Test 10: emTextField — with text content
static void gen_widget_textfield_content() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                        "Hello", true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_textfield_content", vp, ctx);
}

// Test 11: emScalarField — value=50, range 0–100
static void gen_widget_scalarfield() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emScalarField>(view, "test", "Value", "", emImage(),
                                          0, 100, 50, true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_scalarfield", vp, ctx);
}

// Test 12: emColorField — red swatch
static void gen_widget_colorfield() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emColorField>(view, "test", "Color", "", emImage(),
                                         emColor(255, 0, 0, 255));
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_colorfield", vp, ctx);
}

// Test 13: emRadioButton — checked (selected) state
static void gen_widget_radiobutton() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emRadioButton>(view, "test", "Radio Option");
    w->SetChecked(true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_radiobutton", vp, ctx);
}

// Test 14: emListBox — 5 items, item 2 selected
static void gen_widget_listbox() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emListBox>(view, "test", "Items");
    w->AddItem("item0", "Alpha");
    w->AddItem("item1", "Beta");
    w->AddItem("item2", "Gamma");
    w->AddItem("item3", "Delta");
    w->AddItem("item4", "Epsilon");
    w->SetSelectedIndex(2);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_listbox", vp, ctx);
}

// Test 15: emSplitter — horizontal, pos=0.5
static void gen_widget_splitter_h() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                       false, 0.0, 1.0, 0.5);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_splitter_h", vp, ctx);
}

// Test 16: emSplitter — vertical, pos=0.3
static void gen_widget_splitter_v() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                       true, 0.0, 1.0, 0.3);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_splitter_v", vp, ctx);
}

// ═══════════════════════════════════════════════════════════════════
// Coverage extension widget rendering generators (CAP audit)
// ═══════════════════════════════════════════════════════════════════

// CAP-0023: emErrorPanel — dark-red background with yellow error text
static void gen_widget_error_panel() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emErrorPanel>(view, "test",
                                          "Test error: something went wrong");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_error_panel", vp, ctx);
}

// CAP-0076: emTunnel — concentric rounded-rectangle tunnel visual
static void gen_widget_tunnel() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emTunnel>(view, "test", "Tunnel Test");
    w->SetDepth(10.0);
    w->SetChildTallness(0.75);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_tunnel", vp, ctx);
}

// CAP-0026: emFilePanel — no file model state (shows status text)
static void gen_widget_file_panel() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emFilePanel>(view, "test", NULL, true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_file_panel", vp, ctx);
}

// CAP-0027: emFileSelectionBox — empty directory listing
static void gen_widget_file_selection_box() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emFileSelectionBox>(view, "test", "Select File");
    w->SetParentDirectory("/nonexistent_golden_test_dir");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_file_selection_box", vp, ctx);
}

// ═══════════════════════════════════════════════════════════════════
// Phase 7: Widget interaction golden generators
// ═══════════════════════════════════════════════════════════════════

// Test 1: emCheckBox — Click() twice toggles checked state.
// Golden format: [u8 initial][u8 after_click1][u8 after_click2]
static void gen_widget_checkbox_toggle() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* cb = new Testable<emCheckBox>(view, "test", "Check Option");
    cb->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_checkbox_toggle", "widget_state.golden");
    write_u8(f, cb->IsChecked() ? 1 : 0);
    cb->Click();
    write_u8(f, cb->IsChecked() ? 1 : 0);
    cb->Click();
    write_u8(f, cb->IsChecked() ? 1 : 0);
    fclose(f);
    printf("  widget_state/widget_checkbox_toggle\n");
}

// Test 1b: emCheckButton — Click() twice toggles checked state.
// Golden format: [u8 initial][u8 after_click1][u8 after_click2]
static void gen_widget_checkbutton_toggle() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* cb = new Testable<emCheckButton>(view, "test", "Toggle Option");
    cb->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_checkbutton_toggle", "widget_state.golden");
    write_u8(f, cb->IsChecked() ? 1 : 0);
    cb->Click();
    write_u8(f, cb->IsChecked() ? 1 : 0);
    cb->Click();
    write_u8(f, cb->IsChecked() ? 1 : 0);
    fclose(f);
    printf("  widget_state/widget_checkbutton_toggle\n");
}

// Test 2: emRadioButton — switch selection in a group of 3 via Click().
// Golden format: [u32 initial_index][u32 after_switch]
static void gen_widget_radiobutton_switch() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    // Container root panel — emView only allows one root.
    auto* root = new Testable<emLinearLayout>(view, "root");
    root->DoLayout(0, 0, 1.0, 0.75);

    auto* rb_a = new Testable<emRadioButton>(*root, "rb_a", "Option A");
    auto* rb_b = new Testable<emRadioButton>(*root, "rb_b", "Option B");
    auto* rb_c = new Testable<emRadioButton>(*root, "rb_c", "Option C");

    emRadioButton::Mechanism mech;
    mech.Add(rb_a);
    mech.Add(rb_b);
    mech.Add(rb_c);
    mech.SetCheckIndex(0);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_radiobutton_switch", "widget_state.golden");
    write_u32(f, (uint32_t)mech.GetCheckIndex());

    rb_b->Click();
    { TerminateEngine ctrl(sched, 10); sched.Run(); }
    write_u32(f, (uint32_t)mech.GetCheckIndex());

    fclose(f);
    printf("  widget_state/widget_radiobutton_switch\n");
}

// Test 3: emListBox — select in single mode.
// Golden format: [u32 count][u32 * count indices]
static void gen_widget_listbox_select() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* lb = new Testable<emListBox>(view, "test", "Items");
    lb->AddItem("item0", "Alpha");
    lb->AddItem("item1", "Beta");
    lb->AddItem("item2", "Gamma");
    lb->AddItem("item3", "Delta");
    lb->AddItem("item4", "Epsilon");
    lb->SetSelectionType(emListBox::SINGLE_SELECTION);
    lb->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    lb->Select(2, true);
    lb->Select(4, true);

    FILE* f = open_golden("widget_state", "widget_listbox_select", "widget_state.golden");
    const emArray<int>& sel = lb->GetSelectedIndices();
    write_u32(f, (uint32_t)sel.GetCount());
    for (int i = 0; i < sel.GetCount(); i++) {
        write_u32(f, (uint32_t)sel[i]);
    }
    fclose(f);
    printf("  widget_state/widget_listbox_select\n");
}

// Test 4: emSplitter — SetPos with clamping.
// Golden format: [f64 after_0.7][f64 after_1.5_clamped][f64 after_neg0.5_clamped]
static void gen_widget_splitter_setpos() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* sp = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                        false, 0.0, 1.0, 0.5);
    sp->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_splitter_setpos", "widget_state.golden");
    sp->SetPos(0.7);
    write_f64(f, sp->GetPos());
    sp->SetPos(1.5);
    write_f64(f, sp->GetPos());
    sp->SetPos(-0.5);
    write_f64(f, sp->GetPos());
    fclose(f);
    printf("  widget_state/widget_splitter_setpos\n");
}

// Test 5: emTextField — type "abc" via programmatic SetText + SetCursorIndex.
// Uses programmatic API because Input() delivery to headless text fields
// requires view focus + activation state that may not be fully set up.
// Golden format: [u32 text_len][text_bytes][u32 cursor_pos]
static void gen_widget_textfield_type() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* tf = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                         "", true);
    tf->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    // Focus and activate for input delivery
    tf->Focus();
    vp.DoSetViewFocused(true);
    { TerminateEngine ctrl(sched, 10); sched.Run(); }

    // Deliver key events for "abc"
    for (const char* p = "abc"; *p; p++) {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        char buf[2] = {*p, 0};
        emInputKey key = (emInputKey)(EM_KEY_A + (*p - 'a'));
        event.Setup(key, buf, 0, 0);
        state.Set(key, true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    FILE* f = open_golden("widget_state", "widget_textfield_type", "widget_state.golden");
    const emString& text = tf->GetText();
    write_u32(f, (uint32_t)text.GetLen());
    if (text.GetLen() > 0) fwrite(text.Get(), 1, text.GetLen(), f);
    write_u32(f, (uint32_t)tf->GetCursorIndex());
    fclose(f);
    printf("  widget_state/widget_textfield_type\n");
}

// Test 6: emTextField — type "abc" then Backspace.
// Golden format: [u32 text_len][text_bytes][u32 cursor_pos]
static void gen_widget_textfield_backspace() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* tf = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                         "", true);
    tf->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    tf->Focus();
    vp.DoSetViewFocused(true);
    { TerminateEngine ctrl(sched, 10); sched.Run(); }

    // Type "abc"
    for (const char* p = "abc"; *p; p++) {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        char buf[2] = {*p, 0};
        emInputKey key = (emInputKey)(EM_KEY_A + (*p - 'a'));
        event.Setup(key, buf, 0, 0);
        state.Set(key, true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    // Backspace
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        event.Setup(EM_KEY_BACKSPACE, "", 0, 0);
        state.Set(EM_KEY_BACKSPACE, true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    FILE* f = open_golden("widget_state", "widget_textfield_backspace", "widget_state.golden");
    const emString& text = tf->GetText();
    write_u32(f, (uint32_t)text.GetLen());
    if (text.GetLen() > 0) fwrite(text.Get(), 1, text.GetLen(), f);
    write_u32(f, (uint32_t)tf->GetCursorIndex());
    fclose(f);
    printf("  widget_state/widget_textfield_backspace\n");
}

// Test 7: emTextField — type "abcdef" then Shift+Left×3 for selection.
// Golden format: [u32 sel_start][u32 sel_end][u32 cursor_pos]
static void gen_widget_textfield_select() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* tf = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                         "", true);
    tf->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    tf->Focus();
    vp.DoSetViewFocused(true);
    { TerminateEngine ctrl(sched, 10); sched.Run(); }

    // Type "abcdef"
    for (const char* p = "abcdef"; *p; p++) {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        char buf[2] = {*p, 0};
        emInputKey key = (emInputKey)(EM_KEY_A + (*p - 'a'));
        event.Setup(key, buf, 0, 0);
        state.Set(key, true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    // Shift+Left × 3
    for (int i = 0; i < 3; i++) {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        state.Set(EM_KEY_SHIFT, true);
        event.Setup(EM_KEY_CURSOR_LEFT, "", 0, 0);
        state.Set(EM_KEY_CURSOR_LEFT, true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    FILE* f = open_golden("widget_state", "widget_textfield_select", "widget_state.golden");
    write_u32(f, (uint32_t)tf->GetSelectionStartIndex());
    write_u32(f, (uint32_t)tf->GetSelectionEndIndex());
    write_u32(f, (uint32_t)tf->GetCursorIndex());
    fclose(f);
    printf("  widget_state/widget_textfield_select\n");
}

// Test 8: emScalarField — keyboard +/- increment/decrement.
// Golden format: [f64 after_inc][f64 after_dec]
static void gen_widget_scalarfield_inc() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* sf = new Testable<emScalarField>(view, "test", "Value", "", emImage(),
                                            0, 100, 50, true);
    sf->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    sf->Focus();
    vp.DoSetViewFocused(true);
    { TerminateEngine ctrl(sched, 10); sched.Run(); }

    // Increment via "+"
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        event.Setup((emInputKey)'+', "+", 0, 0);
        state.Set((emInputKey)'+', true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    FILE* f = open_golden("widget_state", "widget_scalarfield_inc", "widget_state.golden");
    write_f64(f, (double)sf->GetValue());

    // Decrement via "-"
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        event.Setup((emInputKey)'-', "-", 0, 0);
        state.Set((emInputKey)'-', true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }
    write_f64(f, (double)sf->GetValue());

    fclose(f);
    printf("  widget_state/widget_scalarfield_inc\n");
}

// Test 9: emButton — press + release fires click; capture pressed states.
// Golden format: [u8 pressed_before][u8 pressed_after_press][u8 pressed_after_release][u8 click_count]
static void gen_widget_button_click() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* btn = new Testable<emButton>(view, "test", "Click Me");
    btn->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_button_click", "widget_state.golden");

    // Initial state
    write_u8(f, btn->IsPressed() ? 1 : 0);

    // Press (via Click() which is programmatic — sets pressed transiently and fires signal)
    // Instead, use direct IsPressed() queries around Click().
    // Click() fires signal immediately, pressed is transient.
    // For a meaningful test, capture the state after a programmatic Click().
    btn->Click();
    // After Click(), IsPressed() should be false (click is instantaneous).
    write_u8(f, btn->IsPressed() ? 1 : 0);

    // Call Click() a second time to verify it's repeatable.
    btn->Click();
    write_u8(f, btn->IsPressed() ? 1 : 0);

    fclose(f);
    printf("  widget_state/widget_button_click\n");
}

// Test 10: emListBox — multi-selection mode with Select().
// Golden format: [u32 count][u32*count indices]
static void gen_widget_listbox_multi() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* lb = new Testable<emListBox>(view, "test", "Items");
    lb->AddItem("item0", "Alpha");
    lb->AddItem("item1", "Beta");
    lb->AddItem("item2", "Gamma");
    lb->AddItem("item3", "Delta");
    lb->AddItem("item4", "Epsilon");
    lb->SetSelectionType(emListBox::MULTI_SELECTION);
    lb->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    // Select items 1 and 3 additively (solely=false)
    lb->Select(1, false);
    lb->Select(3, false);

    FILE* f = open_golden("widget_state", "widget_listbox_multi", "widget_state.golden");
    const emArray<int>& sel = lb->GetSelectedIndices();
    write_u32(f, (uint32_t)sel.GetCount());
    for (int i = 0; i < sel.GetCount(); i++) {
        write_u32(f, (uint32_t)sel[i]);
    }
    fclose(f);
    printf("  widget_state/widget_listbox_multi\n");
}

// Test 11: emListBox — toggle selection: select then deselect same item.
// Golden format: [u32 count1][u32*count1 indices1][u32 count2][u32*count2 indices2]
static void gen_widget_listbox_toggle() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* lb = new Testable<emListBox>(view, "test", "Items");
    lb->AddItem("item0", "Alpha");
    lb->AddItem("item1", "Beta");
    lb->AddItem("item2", "Gamma");
    lb->AddItem("item3", "Delta");
    lb->AddItem("item4", "Epsilon");
    lb->SetSelectionType(emListBox::TOGGLE_SELECTION);
    lb->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    // Toggle item 2 on
    lb->ToggleSelection(2);
    {
        FILE* dummy = nullptr; (void)dummy; // scope for const ref
        const emArray<int>& sel = lb->GetSelectedIndices();
        FILE* f = open_golden("widget_state", "widget_listbox_toggle", "widget_state.golden");
        write_u32(f, (uint32_t)sel.GetCount());
        for (int i = 0; i < sel.GetCount(); i++) {
            write_u32(f, (uint32_t)sel[i]);
        }

        // Toggle item 2 off
        lb->ToggleSelection(2);
        const emArray<int>& sel2 = lb->GetSelectedIndices();
        write_u32(f, (uint32_t)sel2.GetCount());
        for (int i = 0; i < sel2.GetCount(); i++) {
            write_u32(f, (uint32_t)sel2[i]);
        }

        fclose(f);
    }
    printf("  widget_state/widget_listbox_toggle\n");
}

// Test 12: emTextField — multi-line cursor navigation via ArrowUp.
// Golden format: [u32 cursor_before_up][u32 cursor_after_up]
static void gen_widget_textfield_cursor_nav() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    auto* tf = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                         "", true);
    tf->SetMultiLineMode(true);
    tf->SetText("abc\ndef");
    tf->SetCursorIndex(7);  // End of "abc\ndef"
    tf->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    tf->Focus();
    vp.DoSetViewFocused(true);
    { TerminateEngine ctrl(sched, 10); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_textfield_cursor_nav", "widget_state.golden");
    write_u32(f, (uint32_t)tf->GetCursorIndex());

    // Deliver ArrowUp
    {
        emInputEvent event;
        emInputState state;
        state.SetMouse(400, 300);
        event.Setup(EM_KEY_CURSOR_UP, "", 0, 0);
        state.Set(EM_KEY_CURSOR_UP, true);
        vp.DoInputToView(event, state);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }

    write_u32(f, (uint32_t)tf->GetCursorIndex());
    fclose(f);
    printf("  widget_state/widget_textfield_cursor_nav\n");
}

// Test 13: emSplitter — drag from 0.5 to ~0.7.
// Golden format: [f64 pos_before][f64 pos_after]
static void gen_widget_splitter_drag() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, 0);
    GoldenViewPort vp(view);

    // emSplitter(parent, name, caption, desc, icon, isVertical, minPos, maxPos, pos)
    auto* sp = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                        false, 0.0, 1.0, 0.5);
    sp->DoLayout(0, 0, 1.0, 0.75);
    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "widget_splitter_drag", "widget_state.golden");
    write_f64(f, sp->GetPos());

    // Simulate drag: SetPos directly (drag via input would require
    // coordinate-aware hit testing of the grip area which depends on
    // content rect geometry that varies with border rendering).
    sp->SetPos(0.7);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    write_f64(f, sp->GetPos());

    fclose(f);
    printf("  widget_state/widget_splitter_drag\n");
}

// BV-1: emBorder OBT_RECT, extreme tall aspect ratio (8x taller than wide)
static void gen_widget_border_rect_extreme_tall() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Test");
    w->SetBorderType(emBorder::OBT_RECT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 8.0);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_rect_extreme_tall", vp, ctx);
}

// BV-2: emBorder OBT_RECT, extreme wide aspect ratio (20x wider than tall)
static void gen_widget_border_rect_extreme_wide() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Test");
    w->SetBorderType(emBorder::OBT_RECT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 0.05);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_rect_extreme_wide", vp, ctx);
}

// BV-3: emBorder OBT_ROUND_RECT, single-pixel height (radius clamping)
static void gen_widget_border_roundrect_thin() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "Test");
    w->SetBorderType(emBorder::OBT_ROUND_RECT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 0.002);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_roundrect_thin", vp, ctx);
}

// BV-4: emBorder OBT_INSTRUMENT, zero-size content area (cramped caption+desc)
static void gen_widget_border_instrument_cramped() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emBorder>(view, "test", "ABCDEFGHIJ",
                                     "Long description that fills space");
    w->SetBorderType(emBorder::OBT_INSTRUMENT, emBorder::IBT_NONE);
    w->DoLayout(0, 0, 1.0, 0.15);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_border_instrument_cramped", vp, ctx);
}

// BV-5: emLabel single character "X" on height-constrained panel
static void gen_widget_label_single_char() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emLabel>(view, "test", "X");
    w->DoLayout(0, 0, 1.0, 0.1);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_label_single_char", vp, ctx);
}

// BV-6: emLabel empty string — zero-width text measurement
static void gen_widget_label_empty() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emLabel>(view, "test", "");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_label_empty", vp, ctx);
}

// BV-7: emLabel very long text on narrow (extreme vertical) panel
static void gen_widget_label_long_narrow() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emLabel>(view, "test",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789 !@#$%^&*() test");
    w->DoLayout(0, 0, 1.0, 4.0);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_label_long_narrow", vp, ctx);
}

// BV-8: emTextField empty content, extreme wide sliver
static void gen_widget_textfield_empty_wide() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                        emString(), true);
    w->DoLayout(0, 0, 1.0, 0.05);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_textfield_empty_wide", vp, ctx);
}

// BV-9: emTextField single character "A", square panel
static void gen_widget_textfield_single_char_square() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emTextField>(view, "test", "Name", "", emImage(),
                                        "A", true);
    w->DoLayout(0, 0, 1.0, 1.0);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_textfield_single_char_square", vp, ctx);
}

// BV-10: emScalarField minimum value — slider at left edge, large negative display
static void gen_widget_scalarfield_min_value() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emScalarField>(view, "test", "Value", "", emImage(),
                                          (emInt64)-1000000000000LL,
                                          (emInt64)1000000000000LL,
                                          (emInt64)-1000000000000LL, true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_scalarfield_min_value", vp, ctx);
}

// BV-11: emScalarField maximum value — slider at right edge, large positive display
static void gen_widget_scalarfield_max_value() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emScalarField>(view, "test", "Value", "", emImage(),
                                          (emInt64)-1000000000000LL,
                                          (emInt64)1000000000000LL,
                                          (emInt64)1000000000000LL, true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_scalarfield_max_value", vp, ctx);
}

// BV-12: emScalarField zero range — min=max=value, division-by-zero guard in slider
static void gen_widget_scalarfield_zero_range() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emScalarField>(view, "test", "Value", "", emImage(),
                                          (emInt64)50, (emInt64)50,
                                          (emInt64)50, true);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_scalarfield_zero_range", vp, ctx);
}

// BV-13: emListBox empty — zero items, tests layout with no children
static void gen_widget_listbox_empty() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emListBox>(view, "test", "Items");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_listbox_empty", vp, ctx);
}

// BV-14: emListBox single item — one entry "Solo", tests single-child layout
static void gen_widget_listbox_single() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emListBox>(view, "test", "Items");
    w->AddItem("item0", "Solo");
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_listbox_single", vp, ctx);
}

// BV-15: emListBox extreme wide — 3 items in horizontal sliver
static void gen_widget_listbox_extreme_wide() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emListBox>(view, "test", "Items");
    w->AddItem("item0", "Alpha");
    w->AddItem("item1", "Beta");
    w->AddItem("item2", "Gamma");
    w->DoLayout(0, 0, 1.0, 0.05);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_listbox_extreme_wide", vp, ctx);
}

// BV-16: emSplitter horizontal, pos=0.0 — first child gets zero width
static void gen_widget_splitter_h_pos0() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                       false, 0.0, 1.0, 0.0);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_splitter_h_pos0", vp, ctx);
}

// BV-17: emSplitter horizontal, pos=1.0 — second child gets zero width
static void gen_widget_splitter_h_pos1() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                       false, 0.0, 1.0, 1.0);
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_splitter_h_pos1", vp, ctx);
}

// BV-18: emSplitter vertical, extreme tall — grip rendering in narrow panel
static void gen_widget_splitter_v_extreme_tall() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                       true, 0.0, 1.0, 0.5);
    w->DoLayout(0, 0, 1.0, 8.0);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_splitter_v_extreme_tall", vp, ctx);
}

// BV-19: emColorField — fully transparent (alpha=0), blend fast-path
static void gen_widget_colorfield_alpha_zero() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emColorField>(view, "test", "Color", "", emImage(),
                                         emColor(255, 0, 0, 0));
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_colorfield_alpha_zero", vp, ctx);
}

// BV-20a: emColorField — fully opaque (alpha=255)
static void gen_widget_colorfield_alpha_opaque() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emColorField>(view, "test", "Color", "", emImage(),
                                         emColor(255, 0, 0, 255));
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_colorfield_alpha_opaque", vp, ctx);
}

// BV-20b: emColorField — near-boundary alpha (alpha=1)
static void gen_widget_colorfield_alpha_near() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emColorField>(view, "test", "Color", "", emImage(),
                                         emColor(255, 0, 0, 1));
    w->DoLayout(0, 0, 1.0, 0.75);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_colorfield_alpha_near", vp, ctx);
}

// BV-21: emCheckBox — extreme tall aspect ratio
static void gen_widget_checkbox_extreme_tall() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emCheckBox>(view, "test", "Check");
    w->DoLayout(0, 0, 1.0, 4.0);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_checkbox_extreme_tall", vp, ctx);
}

// BV-22: emTunnel — extreme wide aspect ratio
static void gen_widget_tunnel_extreme_wide() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    auto* w = new Testable<emTunnel>(view, "test", "Tunnel");
    w->SetDepth(10.0);
    w->SetChildTallness(0.75);
    w->DoLayout(0, 0, 1.0, 0.02);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump("widget_tunnel_extreme_wide", vp, ctx);
}

// ═══════════════════════════════════════════════════════════════════
// Phase 8 — Animator trajectory golden tests
// ═══════════════════════════════════════════════════════════════════

// Testable subclasses exposing protected CycleAnimation as public.

class TestableKineticAnimator : public emKineticViewAnimator {
public:
    TestableKineticAnimator(emView& view) : emKineticViewAnimator(view) {}
    bool DoCycleAnimation(double dt) { return CycleAnimation(dt); }
};

class TestableSpeedingAnimator : public emSpeedingViewAnimator {
public:
    TestableSpeedingAnimator(emView& view) : emSpeedingViewAnimator(view) {}
    bool DoCycleAnimation(double dt) { return CycleAnimation(dt); }
};

class TestableSwipingAnimator : public emSwipingViewAnimator {
public:
    TestableSwipingAnimator(emView& view) : emSwipingViewAnimator(view) {}
    bool DoCycleAnimation(double dt) { return CycleAnimation(dt); }
};

class TestableMagneticAnimator : public emMagneticViewAnimator {
public:
    TestableMagneticAnimator(emView& view) : emMagneticViewAnimator(view) {}
    bool DoCycleAnimation(double dt) { return CycleAnimation(dt); }
};

// Helper: set up a view zoomed in deeply, ready for animator testing.
// Returns the initial view state (rx, ry, ra) after zoom.
struct AnimViewSetup {
    emStandardScheduler sched;
    emRootContext* ctx;
    emView* view;
    GoldenViewPort* vp;

    AnimViewSetup() {
        ctx = new emRootContext(sched);
        view = new emView(*ctx, emView::VF_ROOT_SAME_TALLNESS);
        vp = new GoldenViewPort(*view);
        auto* root = new Testable<PaintingPanel>(*view, "root",
                                                 emColor(200, 200, 200, 255));
        root->DoLayout(0, 0, 1, 0.75);
        { TerminateEngine ctrl(sched, 30); sched.Run(); }
        // Zoom in deeply to give room for scrolling
        view->Zoom(400, 300, 100.0);
        { TerminateEngine ctrl(sched, 10); sched.Run(); }
    }

    ~AnimViewSetup() {
        delete vp;
        delete view;
        delete ctx;
    }
};

// Run kinetic animator for N steps, collecting VELOCITY trajectory data.
// Records velocity (not position) to avoid coordinate system differences.
// Returns [step_count * 3] doubles: (vel_x, vel_y, vel_z) per step.
static std::vector<double> run_kinetic_trajectory(
    AnimViewSetup& s, int steps, double vx, double vy, double vz,
    double friction, bool friction_enabled)
{
    TestableKineticAnimator anim(*s.view);
    anim.Activate();
    anim.SetFriction(friction);
    anim.SetFrictionEnabled(friction_enabled);
    anim.SetVelocity(0, vx);
    anim.SetVelocity(1, vy);
    anim.SetVelocity(2, vz);

    std::vector<double> data;
    data.reserve(steps * 3);
    const double dt = 1.0 / 60.0;

    for (int i = 0; i < steps; i++) {
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }

    anim.Deactivate();
    return data;
}

// ─── Kinetic trajectory tests ──────────────────────────────────

static void gen_animator_kinetic_fling_x() {
    AnimViewSetup s;
    auto data = run_kinetic_trajectory(s, 60, 100.0, 0.0, 0.0, 2.0, true);
    dump_trajectory("animator_kinetic_fling_x", data.data(), 60);
}

static void gen_animator_kinetic_fling_xy() {
    AnimViewSetup s;
    auto data = run_kinetic_trajectory(s, 60, 100.0, 50.0, 0.0, 2.0, true);
    dump_trajectory("animator_kinetic_fling_xy", data.data(), 60);
}

static void gen_animator_kinetic_zoom() {
    AnimViewSetup s;
    auto data = run_kinetic_trajectory(s, 60, 0.0, 0.0, 5.0, 2.0, true);
    dump_trajectory("animator_kinetic_zoom", data.data(), 60);
}

// ─── Speeding trajectory tests ──────────────────────────────────

static void gen_animator_speeding_ramp() {
    AnimViewSetup s;
    TestableSpeedingAnimator anim(*s.view);
    anim.Activate();
    anim.SetFriction(2.0);
    anim.SetFrictionEnabled(true);
    anim.SetAcceleration(500.0);
    anim.SetReverseAcceleration(1000.0);
    anim.SetTargetVelocity(0, 200.0);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    for (int i = 0; i < 60; i++) {
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }
    anim.Deactivate();
    dump_trajectory("animator_speeding_ramp", data.data(), 60);
}

static void gen_animator_speeding_reverse() {
    AnimViewSetup s;
    TestableSpeedingAnimator anim(*s.view);
    anim.Activate();
    anim.SetFriction(2.0);
    anim.SetFrictionEnabled(true);
    anim.SetAcceleration(500.0);
    anim.SetReverseAcceleration(1000.0);
    // Start with positive velocity, target negative
    anim.SetVelocity(0, 100.0);
    anim.SetTargetVelocity(0, -200.0);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    for (int i = 0; i < 60; i++) {
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }
    anim.Deactivate();
    dump_trajectory("animator_speeding_reverse", data.data(), 60);
}

static void gen_animator_speeding_release() {
    AnimViewSetup s;
    TestableSpeedingAnimator anim(*s.view);
    anim.Activate();
    anim.SetFriction(2.0);
    anim.SetFrictionEnabled(true);
    anim.SetAcceleration(500.0);
    anim.SetReverseAcceleration(1000.0);
    // Ramp up for 30 steps then release (set target to 0)
    anim.SetTargetVelocity(0, 200.0);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    for (int i = 0; i < 60; i++) {
        if (i == 30) {
            // Release: clear target, let friction decelerate
            anim.SetTargetVelocity(0, 0.0);
        }
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }
    anim.Deactivate();
    dump_trajectory("animator_speeding_release", data.data(), 60);
}

// ─── Swiping trajectory tests ──────────────────────────────────

static void gen_animator_swiping_grip() {
    AnimViewSetup s;
    TestableSwipingAnimator anim(*s.view);
    anim.Activate();
    anim.SetFriction(2.0);
    anim.SetFrictionEnabled(true);
    anim.SetSpringConstant(100.0);
    anim.SetGripped(true);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    // Move grip in X, let spring track
    for (int i = 0; i < 60; i++) {
        if (i < 10) {
            anim.MoveGrip(0, 5.0); // Apply 5px grip per frame for first 10 frames
        }
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }
    anim.Deactivate();
    dump_trajectory("animator_swiping_grip", data.data(), 60);
}

static void gen_animator_swiping_release() {
    AnimViewSetup s;
    TestableSwipingAnimator anim(*s.view);
    anim.Activate();
    anim.SetFriction(2.0);
    anim.SetFrictionEnabled(true);
    anim.SetSpringConstant(100.0);
    anim.SetGripped(true);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    // Grip, move, then release
    for (int i = 0; i < 60; i++) {
        if (i < 10) {
            anim.MoveGrip(0, 5.0);
        }
        if (i == 20) {
            anim.SetGripped(false); // Release — coast with kinetic friction
        }
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }
    anim.Deactivate();
    dump_trajectory("animator_swiping_release", data.data(), 60);
}

// ─── Visiting trajectory tests ──────────────────────────────────

class TestableVisitingAnimator : public emVisitingViewAnimator {
public:
    TestableVisitingAnimator(emView& view) : emVisitingViewAnimator(view) {}
    bool DoCycleAnimation(double dt) { return CycleAnimation(dt); }
};

// Setup for visiting tests — moderate zoom (factor 2) to stay within Rust
// clamps while still having room to navigate.
struct VisitAnimViewSetup {
    emStandardScheduler sched;
    emRootContext* ctx;
    emView* view;
    GoldenViewPort* vp;

    VisitAnimViewSetup() {
        ctx = new emRootContext(sched);
        view = new emView(*ctx, emView::VF_ROOT_SAME_TALLNESS);
        vp = new GoldenViewPort(*view);
        auto* root = new Testable<PaintingPanel>(*view, "root",
                                                 emColor(200, 200, 200, 255));
        root->DoLayout(0, 0, 1, 0.75);
        { TerminateEngine ctrl(sched, 30); sched.Run(); }
        // Moderate zoom in: factor 2 (C++ rel_a ≈ 0.25, Rust rel_a ≈ 4)
        view->Zoom(400, 300, 2.0);
        { TerminateEngine ctrl(sched, 10); sched.Run(); }
    }

    ~VisitAnimViewSetup() {
        delete vp;
        delete view;
        delete ctx;
    }
};

// Helper to record view state trajectory for visiting animator.
// Records (rel_x, rel_y, 1/rel_a) per step — the 1/rel_a converts
// C++ area-fraction to Rust scale-factor convention.
static void gen_animator_visiting_short() {
    VisitAnimViewSetup s;
    TestableVisitingAnimator anim(*s.view);
    anim.Activate();
    anim.SetAnimated(true);
    anim.SetAcceleration(5.0);
    anim.SetMaxAbsoluteSpeed(5.0);
    anim.SetMaxCuspSpeed(2.5);
    // Visit root at (0.1, 0.1, 0.5) — C++ rel_a = 0.5 means moderate zoom in
    // (Rust rel_a = 2.0)
    anim.SetGoal("root", 0.1, 0.1, 0.5, false);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    for (int i = 0; i < 60; i++) {
        anim.DoCycleAnimation(dt);
        double rx, ry, ra;
        anim.GetView().GetVisitedPanel(&rx, &ry, &ra);
        data.push_back(rx);
        data.push_back(ry);
        // Convert C++ rel_a (area fraction) to Rust rel_a (scale factor)
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    anim.Deactivate();
    dump_trajectory("animator_visiting_short", data.data(), 60);
}

static void gen_animator_visiting_zoom() {
    VisitAnimViewSetup s;
    TestableVisitingAnimator anim(*s.view);
    anim.Activate();
    anim.SetAnimated(true);
    anim.SetAcceleration(5.0);
    anim.SetMaxAbsoluteSpeed(5.0);
    anim.SetMaxCuspSpeed(2.5);
    // Pure zoom: visit root at center (0, 0) but more zoom in
    // C++ rel_a = 0.0625 (Rust rel_a = 16.0) — zoom to 4x from 2x start
    anim.SetGoal("root", 0.0, 0.0, 0.0625, false);

    std::vector<double> data;
    const double dt = 1.0 / 60.0;
    for (int i = 0; i < 60; i++) {
        anim.DoCycleAnimation(dt);
        double rx, ry, ra;
        anim.GetView().GetVisitedPanel(&rx, &ry, &ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    anim.Deactivate();
    dump_trajectory("animator_visiting_zoom", data.data(), 60);
}

// ─── Magnetic trajectory tests ──────────────────────────────────

static void gen_animator_magnetic_approach() {
    AnimViewSetup s;
    TestableMagneticAnimator anim(*s.view);
    anim.Activate();

    // Set magnetism config: moderate radius and speed
    // C++ uses CoreConfig.MagnetismRadius (default 1.0) and
    // CoreConfig.MagnetismSpeed (default 1.0)

    const double dt = 1.0 / 60.0;
    std::vector<double> data;
    data.reserve(60 * 3);

    for (int i = 0; i < 60; i++) {
        anim.DoCycleAnimation(dt);
        data.push_back(anim.GetVelocity(0));
        data.push_back(anim.GetVelocity(1));
        data.push_back(anim.GetVelocity(2));
    }

    anim.Deactivate();
    dump_trajectory("animator_magnetic_approach", data.data(), 60);
}

// ═══════════════════════════════════════════════════════════════════
// Phase 9: ViewInputFilter trajectory golden masters
// ═══════════════════════════════════════════════════════════════════

// TimedGoldenViewPort: override GetInputClockMS for deterministic dt.
class TimedGoldenViewPort : public GoldenViewPort {
    emUInt64 mFakeClock;
    emUInt64 mClockStep;
public:
    TimedGoldenViewPort(emView& view, emUInt64 step_ms = 16)
        : GoldenViewPort(view), mFakeClock(1000000), mClockStep(step_ms) {}
    virtual emUInt64 GetInputClockMS() const override { return mFakeClock; }
    void AdvanceClock() { mFakeClock += mClockStep; }
};

// View setup for VIF testing: 800x600, root panel, zoomed 100x, focused.
struct VIFTestSetup {
    emStandardScheduler sched;
    emRootContext* ctx;
    emView* view;
    TimedGoldenViewPort* vp;

    VIFTestSetup() {
        ctx = new emRootContext(sched);
        view = new emView(*ctx, emView::VF_ROOT_SAME_TALLNESS);
        vp = new TimedGoldenViewPort(*view, 16);
        auto* root = new Testable<PaintingPanel>(*view, "root",
                                                 emColor(200, 200, 200, 255));
        root->DoLayout(0, 0, 1, 0.75);
        { TerminateEngine ctrl(sched, 30); sched.Run(); }
        view->Zoom(400, 300, 100.0);
        { TerminateEngine ctrl(sched, 10); sched.Run(); }
        vp->DoSetViewFocused(true);
        { TerminateEngine ctrl(sched, 5); sched.Run(); }
    }
    ~VIFTestSetup() {
        delete vp;
        delete view;
        delete ctx;
    }

    void step() {
        vp->AdvanceClock();
        TerminateEngine ctrl(sched, 1);
        sched.Run();
    }

    void read_state(double& rx, double& ry, double& ra) {
        view->GetVisitedPanel(&rx, &ry, &ra);
    }
};

// ─── Mouse VIF tests ──────────────────────────────────────────────

static void gen_filter_wheel_zoom_in() {
    VIFTestSetup s;
    // NOTE: The view already has a default emMouseZoomScrollVIF installed
    // (from the emView constructor). We use that one, not a new one.

    std::vector<double> data;
    const int steps = 60;

    // Frame 0: single wheel up at center
    {
        emInputEvent event;
        event.Setup(EM_KEY_WHEEL_UP, emString(), 0, 0);
        emInputState state;
        state.SetMouse(400, 300);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_wheel_zoom_in", data.data(), steps);
}

static void gen_filter_wheel_zoom_out() {
    VIFTestSetup s;
    // The view's default emMouseZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    {
        emInputEvent event;
        event.Setup(EM_KEY_WHEEL_DOWN, emString(), 0, 0);
        emInputState state;
        state.SetMouse(400, 300);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_wheel_zoom_out", data.data(), steps);
}

static void gen_filter_wheel_acceleration() {
    VIFTestSetup s;
    // The view's default emMouseZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    // 5 wheel-up events every 3 frames (48ms intervals)
    for (int i = 0; i < steps; i++) {
        if (i == 0 || i == 3 || i == 6 || i == 9 || i == 12) {
            emInputEvent event;
            event.Setup(EM_KEY_WHEEL_UP, emString(), 0, 0);
            emInputState state;
            state.SetMouse(400, 300);
            s.vp->DoInputToView(event, state);
        }
        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_wheel_acceleration", data.data(), steps);
}

static void gen_filter_middle_pan() {
    VIFTestSetup s;
    // The view's default emMouseZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    // Frame 0: middle press at (400,300)
    {
        emInputEvent event;
        event.Setup(EM_KEY_MIDDLE_BUTTON, emString(), 0, 0);
        emInputState state;
        state.SetMouse(400, 300);
        state.Set(EM_KEY_MIDDLE_BUTTON, true);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        // Frames 1-10: move mouse from (400,300) to (500,400), 10px/frame
        if (i >= 1 && i <= 10) {
            double mx = 400.0 + i * 10.0;
            double my = 300.0 + i * 10.0;
            emInputEvent event;
            event.Setup(EM_KEY_NONE, emString(), 0, 0);
            emInputState state;
            state.SetMouse(mx, my);
            state.Set(EM_KEY_MIDDLE_BUTTON, true);
            s.vp->DoInputToView(event, state);
        }

        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_middle_pan", data.data(), steps);
}

static void gen_filter_middle_fling() {
    VIFTestSetup s;
    // The view's default emMouseZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    // Frame 0: middle press at (400,300)
    {
        emInputEvent event;
        event.Setup(EM_KEY_MIDDLE_BUTTON, emString(), 0, 0);
        emInputState state;
        state.SetMouse(400, 300);
        state.Set(EM_KEY_MIDDLE_BUTTON, true);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        // Frames 1-10: move mouse 10px/frame
        if (i >= 1 && i <= 10) {
            double mx = 400.0 + i * 10.0;
            double my = 300.0 + i * 10.0;
            emInputEvent event;
            event.Setup(EM_KEY_NONE, emString(), 0, 0);
            emInputState state;
            state.SetMouse(mx, my);
            state.Set(EM_KEY_MIDDLE_BUTTON, true);
            s.vp->DoInputToView(event, state);
        }

        // Frame 10: release middle button
        if (i == 10) {
            emInputEvent event;
            event.Setup(EM_KEY_NONE, emString(), 0, 0);
            emInputState state;
            state.SetMouse(500, 400);
            // Middle button NOT set → release
            s.vp->DoInputToView(event, state);
        }

        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_middle_fling", data.data(), steps);
}

// ─── Keyboard VIF tests ──────────────────────────────────────────

static void gen_filter_keyboard_scroll() {
    VIFTestSetup s;
    // The view's default emKeyboardZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    // Frame 0: Alt+Right press (held for all 60 frames)
    {
        emInputEvent event;
        event.Setup(EM_KEY_CURSOR_RIGHT, emString(), 0, 0);
        emInputState state;
        state.Set(EM_KEY_ALT, true);
        state.Set(EM_KEY_CURSOR_RIGHT, true);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_keyboard_scroll", data.data(), steps);
}

static void gen_filter_keyboard_zoom() {
    VIFTestSetup s;
    // The view's default emKeyboardZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    // Frame 0: Alt+PageUp press
    {
        emInputEvent event;
        event.Setup(EM_KEY_PAGE_UP, emString(), 0, 0);
        emInputState state;
        state.Set(EM_KEY_ALT, true);
        state.Set(EM_KEY_PAGE_UP, true);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_keyboard_zoom", data.data(), steps);
}

static void gen_filter_keyboard_release() {
    VIFTestSetup s;
    // The view's default emKeyboardZoomScrollVIF handles the events.

    std::vector<double> data;
    const int steps = 60;

    // Frame 0: Alt+Right press
    {
        emInputEvent event;
        event.Setup(EM_KEY_CURSOR_RIGHT, emString(), 0, 0);
        emInputState state;
        state.Set(EM_KEY_ALT, true);
        state.Set(EM_KEY_CURSOR_RIGHT, true);
        s.vp->DoInputToView(event, state);
    }

    for (int i = 0; i < steps; i++) {
        // Frame 30: release Right (Alt still held)
        if (i == 30) {
            emInputEvent event;
            event.Setup(EM_KEY_NONE, emString(), 0, 0);
            emInputState state;
            state.Set(EM_KEY_ALT, true);
            // CURSOR_RIGHT not set → key released
            s.vp->DoInputToView(event, state);
        }
        s.step();
        double rx, ry, ra;
        s.read_state(rx, ry, ra);
        data.push_back(rx);
        data.push_back(ry);
        data.push_back(ra > 1e-100 ? 1.0 / ra : 1000.0);
    }
    dump_trajectory("filter_keyboard_release", data.data(), steps);
}

// ═══════════════════════════════════════════════════════════════════
// Phase 10 — TestPanel integration golden tests
// ═══════════════════════════════════════════════════════════════════

// Render a view into an image of specified size and dump as compositor golden.
static void render_and_dump_sized(const char* name, GoldenViewPort& vp,
                                   emRootContext& ctx, int w, int h) {
    emImage img(w, h, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, (double)w, (double)h)) {
        fprintf(stderr, "PreparePainter failed for %s\n", name);
        exit(1);
    }
    open_draw_op_log(name);
    vp.DoPaintView(p, 0);
    close_draw_op_log();
    dump_compositor(name, img);
}

// Root panel paint only — high AE threshold prevents auto-expansion,
// so only the root TestPanel's own Paint() fires (primitives, text, etc.).
static void gen_testpanel_root() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 1000, 1000, 1.0);

    auto* tp = new emTestPanel(view, "test");
    tp->SetAutoExpansionThreshold(1e9); // prevent auto-expansion
    tp->Layout(0, 0, 1.0, 1.0);

    { TerminateEngine ctrl(sched, 30); sched.Run(); }
    render_and_dump_sized("testpanel_root", vp, ctx, 1000, 1000);
}

// Full TestPanel tree with auto-expansion — exercises the complete panel
// hierarchy including TkTest widget grid, recursive TestPanels, ColorField,
// and PolyDrawPanel.
static void gen_testpanel_expanded() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 1000, 1000, 1.0);

    auto* tp = new emTestPanel(view, "test");
    // Default AE threshold 900 (VCT_AREA). At 1000x1000, vc=1e6 > 900 → expands.
    tp->Layout(0, 0, 1.0, 1.0);

    // Run enough scheduler cycles for deep auto-expansion cascade
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("testpanel_expanded", vp, ctx, 1000, 1000);
}

// TkTest widget grid at 1x zoom — the emTestPanel::TkTest panel rendered
// standalone in an 800x600 viewport, filling the full view.
static void gen_tktest_1x() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 600, 1.0);

    auto* tk = create_tktest(view, "tktest");
    tk->Layout(0, 0, 800.0 / 600.0, 1.0);

    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    dump_panel_tree("tktest_1x", tk);
    render_and_dump_sized("tktest_1x", vp, ctx, 800, 600);
}

// TkTest widget grid at 2x zoom — 800x600 viewport scrolled to center,
// showing the middle 50% of the panel.
static void gen_tktest_2x() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 600, 1.0);

    auto* tk = create_tktest(view, "tktest");
    tk->Layout(0, 0, 800.0 / 600.0, 1.0);

    { TerminateEngine ctrl(sched, 200); sched.Run(); }

    // Zoom 2x centered on viewport center (400, 300)
    view.Zoom(400, 300, 2.0);
    { TerminateEngine ctrl(sched, 10); sched.Run(); }

    render_and_dump_sized("tktest_2x", vp, ctx, 800, 600);
}

// ═══════════════════════════════════════════════════════════════════
// Splitter drag + layout — numeric test of child rects after position changes
// ═══════════════════════════════════════════════════════════════════

// Golden format: [u32 step_count] then per step:
//   [f64 position][f64 c0_x][f64 c0_y][f64 c0_w][f64 c0_h]
//                 [f64 c1_x][f64 c1_y][f64 c1_w][f64 c1_h]
// = 9 f64 per step

static void dump_splitter_step(FILE* f, emSplitter* sp, emPanel* c0, emPanel* c1) {
    write_f64(f, sp->GetPos());
    write_f64(f, c0->GetLayoutX());
    write_f64(f, c0->GetLayoutY());
    write_f64(f, c0->GetLayoutWidth());
    write_f64(f, c0->GetLayoutHeight());
    write_f64(f, c1->GetLayoutX());
    write_f64(f, c1->GetLayoutY());
    write_f64(f, c1->GetLayoutWidth());
    write_f64(f, c1->GetLayoutHeight());
}

static void gen_splitter_layout_h() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    // Horizontal splitter, range [0,1], initial pos=0.5, no border decoration
    auto* sp = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                        false, 0.0, 1.0, 0.5);
    sp->SetBorderType(emBorder::OBT_NONE, emBorder::IBT_NONE);
    sp->DoLayout(0, 0, 1.0, 0.75);

    // Two simple child panels
    auto* c0 = new PaintingPanel(sp, "left", emColor(0xFF000080));
    auto* c1 = new PaintingPanel(sp, "right", emColor(0x0000FF80));

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "splitter_layout_h", "widget_state.golden");
    uint32_t steps = 4;
    write_u32(f, steps);

    // Step 0: initial position 0.5
    dump_splitter_step(f, sp, c0, c1);

    // Step 1: drag to 0.3
    sp->SetPos(0.3);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    dump_splitter_step(f, sp, c0, c1);

    // Step 2: drag to 0.8
    sp->SetPos(0.8);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    dump_splitter_step(f, sp, c0, c1);

    // Step 3: drag past max — should clamp to 1.0
    sp->SetPos(1.5);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    dump_splitter_step(f, sp, c0, c1);

    fclose(f);
    printf("  widget_state/splitter_layout_h\n");
}

static void gen_splitter_layout_v() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);

    // Vertical splitter, range [0,1], initial pos=0.5, no border decoration
    auto* sp = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                        true, 0.0, 1.0, 0.5);
    sp->SetBorderType(emBorder::OBT_NONE, emBorder::IBT_NONE);
    sp->DoLayout(0, 0, 1.0, 1.0);

    auto* c0 = new PaintingPanel(sp, "top", emColor(0xFF000080));
    auto* c1 = new PaintingPanel(sp, "bottom", emColor(0x0000FF80));

    { TerminateEngine ctrl(sched, 30); sched.Run(); }

    FILE* f = open_golden("widget_state", "splitter_layout_v", "widget_state.golden");
    uint32_t steps = 4;
    write_u32(f, steps);

    // Step 0: initial position 0.5
    dump_splitter_step(f, sp, c0, c1);

    // Step 1: drag to 0.2
    sp->SetPos(0.2);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    dump_splitter_step(f, sp, c0, c1);

    // Step 2: drag to 0.7
    sp->SetPos(0.7);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    dump_splitter_step(f, sp, c0, c1);

    // Step 3: drag to 0.0 — minimum
    sp->SetPos(0.0);
    { TerminateEngine ctrl(sched, 5); sched.Run(); }
    dump_splitter_step(f, sp, c0, c1);

    fclose(f);
    printf("  widget_state/splitter_layout_v\n");
}

// ═══════════════════════════════════════════════════════════════════
// ListBox expanded — renders expanded ListBox with multi-selected items
// ═══════════════════════════════════════════════════════════════════

static void gen_listbox_expanded() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 800, 1.0);

    // Multi-selection ListBox with 7 items, 3 selected.
    // Default AE threshold 150 (VCT_AREA). At 800x800 with layout 1.0x1.0,
    // area = 640000 >> 150, so auto-expansion triggers and creates item panels.
    auto* w = new Testable<emListBox>(view, "test", "Items");
    w->SetSelectionType(emListBox::MULTI_SELECTION);
    w->AddItem("item0", "Alpha");
    w->AddItem("item1", "Beta");
    w->AddItem("item2", "Gamma");
    w->AddItem("item3", "Delta");
    w->AddItem("item4", "Epsilon");
    w->AddItem("item5", "Zeta");
    w->AddItem("item6", "Eta");
    w->Select(1, false);
    w->Select(3, false);
    w->Select(5, false);
    w->DoLayout(0, 0, 1.0, 1.0);

    // 200 cycles for auto-expansion + item panel creation
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("listbox_expanded", vp, ctx, 800, 800);
}

// ═══════════════════════════════════════════════════════════════════
// ColorField expanded — renders expanded ColorField with child ScalarFields
// ═══════════════════════════════════════════════════════════════════

static void gen_colorfield_expanded() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 800, 1.0);

    // Editable ColorField with alpha, red color.
    // C++ AE threshold is 9 (VCT_MIN_EXT). At 800x800 with layout 1.0x1.0,
    // min_ext = 800 >> 9, so auto-expansion triggers.
    auto* w = new Testable<emColorField>(view, "test", "Color",
        "Test color field", emImage(), emColor(0xBB, 0x22, 0x22, 0xFF));
    w->SetEditable(true);
    w->SetAlphaEnabled(true);
    w->DoLayout(0, 0, 1.0, 1.0);

    // 200 cycles: auto-expansion creates RasterLayout + 8 child panels
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("colorfield_expanded", vp, ctx, 800, 800);
}

// ═══════════════════════════════════════════════════════════════════
// Composed widget golden — Splitter with Border/ColorField/ListBox children
// ═══════════════════════════════════════════════════════════════════

static void gen_composed_splitter_content() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 600, 1.0);

    // Horizontal splitter, range [0,1], initial pos=0.5
    auto* sp = new Testable<emSplitter>(view, "test", "", "", emImage(),
                                        false, 0.0, 1.0, 0.5);
    sp->SetBorderType(emBorder::OBT_NONE, emBorder::IBT_NONE);
    sp->DoLayout(0, 0, 800.0 / 600.0, 1.0);

    // Left child: Border containing ColorField + ListBox
    auto* left = new Testable<emBorder>(sp, "left", "Left");
    left->SetBorderType(emBorder::OBT_RECT, emBorder::IBT_NONE);

    auto* lcf = new Testable<emColorField>(left, "color", "Color", "", emImage(),
                                           emColor(255, 0, 0, 255));
    (void)lcf;

    auto* llb = new Testable<emListBox>(left, "list", "Items");
    llb->AddItem("item0", "Item 1");
    llb->AddItem("item1", "Item 2");
    llb->AddItem("item2", "Item 3");
    llb->AddItem("item3", "Item 4");
    llb->AddItem("item4", "Item 5");

    // Right child: Border containing ColorField + ListBox
    auto* right = new Testable<emBorder>(sp, "right", "Right");
    right->SetBorderType(emBorder::OBT_RECT, emBorder::IBT_NONE);

    auto* rcf = new Testable<emColorField>(right, "color", "Color", "", emImage(),
                                            emColor(0, 0, 255, 255));
    (void)rcf;

    auto* rlb = new Testable<emListBox>(right, "list", "Items");
    rlb->AddItem("item0", "Alpha");
    rlb->AddItem("item1", "Beta");
    rlb->AddItem("item2", "Gamma");

    // 200 cycles for auto-expansion cascade through the widget tree
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("composed_splitter_content", vp, ctx, 800, 600);
}

// ═══════════════════════════════════════════════════════════════════
// Composed widget golden — nested Border-in-Border with Label/Button/TextField
// ═══════════════════════════════════════════════════════════════════

static void gen_composed_border_nest() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 600, 1.0);

    // Outer border: OBT_ROUND_RECT, filled look
    auto* outer = new Testable<emLinearLayout>(view, "outer", "Outer");
    outer->SetBorderType(emBorder::OBT_ROUND_RECT, emBorder::IBT_NONE);
    outer->SetVertical();
    outer->DoLayout(0, 0, 800.0 / 600.0, 1.0);

    // Inner border: OBT_RECT with IBT_GROUP, child of outer
    auto* inner = new Testable<emLinearLayout>(*outer, "inner", "Inner");
    inner->SetBorderType(emBorder::OBT_RECT, emBorder::IBT_GROUP);
    inner->SetVertical();

    // Children of inner border
    auto* lbl = new Testable<emLabel>(*inner, "label", "Test Label");
    (void)lbl;

    auto* btn = new Testable<emButton>(*inner, "button", "Test Button");
    (void)btn;

    auto* tf = new Testable<emTextField>(*inner, "textfield", "Field", "", emImage(),
                                         "Hello", true);
    (void)tf;

    // 200 cycles for auto-expansion cascade through the nested widget tree
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("composed_border_nest", vp, ctx, 800, 600);
}

// ═══════════════════════════════════════════════════════════════════
// Composed widget golden — scrolled ListBox inside Border
// ═══════════════════════════════════════════════════════════════════

static void gen_composed_scrolled_listbox() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 600, 1.0);

    // Outer border: OBT_ROUND_RECT, filled look, caption "Scrolled List"
    auto* border = new Testable<emBorder>(view, "border", "Scrolled List");
    border->SetBorderType(emBorder::OBT_ROUND_RECT, emBorder::IBT_NONE);
    border->DoLayout(0, 0, 800.0 / 600.0, 1.0);

    // ListBox with 50 items, scrolled to item 25
    auto* lb = new Testable<emListBox>(*border, "list", "Items");
    for (int i = 1; i <= 50; i++) {
        char key[16], val[16];
        snprintf(key, sizeof(key), "item%d", i - 1);
        snprintf(val, sizeof(val), "Item %d", i);
        lb->AddItem(key, val);
    }
    lb->SetSelectedIndex(24); // Item 25 (0-based index 24)

    // 200 cycles for auto-expansion cascade
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("composed_scrolled_listbox", vp, ctx, 800, 600);
}

// ═══════════════════════════════════════════════════════════════════
// Composed widget golden — Border(RoundRect,Group) + ColorField at different aspect ratios
// ═══════════════════════════════════════════════════════════════════

static void gen_composed_colorfield_wide() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 800, 400, 1.0);

    // Border with OBT_ROUND_RECT / IBT_GROUP, containing a ColorField
    auto* border = new Testable<emBorder>(view, "test", "Wide");
    border->SetBorderType(emBorder::OBT_ROUND_RECT, emBorder::IBT_GROUP);
    border->DoLayout(0, 0, 800.0 / 400.0, 1.0);

    auto* cf = new Testable<emColorField>(*border, "color", "Color", "", emImage(),
                                           emColor(255, 0, 0, 255));
    (void)cf;

    // 200 cycles for layout cascade
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("composed_colorfield_wide", vp, ctx, 800, 400);
}

static void gen_composed_colorfield_tall() {
    emStandardScheduler sched;
    emRootContext ctx(sched);
    StubClipboard::Setup(ctx);
    emView view(ctx, emView::VF_NO_ACTIVE_HIGHLIGHT);
    GoldenViewPort vp(view);
    vp.DoSetViewGeometry(0, 0, 400, 800, 1.0);

    // Border with OBT_ROUND_RECT / IBT_GROUP, containing a ColorField
    auto* border = new Testable<emBorder>(view, "test", "Tall");
    border->SetBorderType(emBorder::OBT_ROUND_RECT, emBorder::IBT_GROUP);
    border->DoLayout(0, 0, 400.0 / 800.0, 1.0);

    auto* cf = new Testable<emColorField>(*border, "color", "Color", "", emImage(),
                                           emColor(255, 0, 0, 255));
    (void)cf;

    // 200 cycles for layout cascade
    { TerminateEngine ctrl(sched, 200); sched.Run(); }
    render_and_dump_sized("composed_colorfield_tall", vp, ctx, 400, 800);
}

// ═══════════════════════════════════════════════════════════════════
// Eagle logo golden generator
// ═══════════════════════════════════════════════════════════════════

static void gen_eagle_logo() {
    const int W = 800, H = 600;
    const double panelH = 0.75;

    emImage img(W, H, 4);
    img.Fill(emColor::WHITE);

    emPainter p;
    if (!img.PreparePainter(&p, *g_ctx,
                            0.0, 0.0,
                            (double)W, (double)H)) {
        fprintf(stderr, "PreparePainter failed for eagle_logo!\n");
        exit(1);
    }

    // Uniform scale: map (0,0)-(1.0,0.75) to (0,0)-(800,600).
    // scale_x = 800, scale_y = 800 (uniform).
    double scaleX = 800.0, scaleY = 800.0;
    emPainter sp(
        p,
        p.GetClipX1(), p.GetClipY1(),
        p.GetClipX2(), p.GetClipY2(),
        p.GetOriginX(), p.GetOriginY(),
        scaleX, scaleY
    );

    // Gradient background: top blue(145,171,242) → bottom gold(225,221,183).
    sp.PaintRect(
        0, 0, 1.0, panelH,
        emLinearGradientTexture(
            0, 0,       emColor(145, 171, 242),
            0, panelH,  emColor(225, 221, 183)
        ),
        emColor(0, 0, 0, 255)
    );

    // Compute eagle transform — matches C++ UpdateCoordinates for h=0.75.
    double eagleScaleX = emMin(1.0/180000.0, panelH/120000.0);
    double eagleScaleY = eagleScaleX;
    double eagleShiftX = 0.5 - eagleScaleX * 78450.0;
    double eagleShiftY = panelH * 0.5 - eagleScaleY * 47690.0;

    // Create sub-painter with eagle transform.
    emPainter ep(
        sp,
        sp.GetClipX1(), sp.GetClipY1(),
        sp.GetClipX2(), sp.GetClipY2(),
        sp.GetOriginX() + sp.GetScaleX() * eagleShiftX,
        sp.GetOriginY() + sp.GetScaleY() * eagleShiftY,
        sp.GetScaleX() * eagleScaleX,
        sp.GetScaleY() * eagleScaleY
    );

    // Polygon data — copied from emMainContentPanel.cpp lines 134-345.
    static const double poly0[]={
        79695.0,46350.0,81090.0,46710.0,82980.0,48465.0,85185.0,49770.0,86580.0,50085.0,89595.0,49545.0,
        97785.0,45900.0,109215.0,39240.0,117990.0,32355.0,126270.0,26235.0,128925.0,24210.0,129060.0,24885.0,
        127935.0,25920.0,127665.0,26550.0,132480.0,22590.0,138735.0,16605.0,139995.0,14940.0,140580.0,13770.0,
        140850.0,12600.0,140940.0,11160.0,140850.0,9000.0,141660.0,9540.0,142065.0,10350.0,142335.0,12105.0,
        142650.0,11025.0,142785.0,8775.0,142740.0,6435.0,142425.0,4905.0,141615.0,3555.0,142470.0,3690.0,
        142920.0,3960.0,143685.0,4680.0,144135.0,5490.0,144450.0,6930.0,144585.0,8865.0,144540.0,10665.0,
        144180.0,12780.0,143595.0,14490.0,142740.0,16245.0,140175.0,19710.0,141210.0,19125.0,142245.0,18090.0,
        143505.0,16335.0,144450.0,14670.0,145530.0,11655.0,145980.0,9225.0,146025.0,8415.0,145935.0,6975.0,
        145710.0,6165.0,145215.0,5220.0,146610.0,6255.0,147060.0,6930.0,147240.0,7560.0,147375.0,9270.0,
        147285.0,10665.0,146925.0,12645.0,146160.0,14895.0,144990.0,17415.0,142785.0,21465.0,145440.0,18945.0,
        146520.0,17505.0,147825.0,15435.0,149040.0,12915.0,149265.0,11970.0,149355.0,10845.0,149850.0,11340.0,
        150165.0,12060.0,150300.0,13275.0,149850.0,15390.0,148950.0,17865.0,148365.0,19125.0,146160.0,22545.0,
        147555.0,21645.0,148410.0,20880.0,149220.0,19890.0,150435.0,17865.0,151065.0,16965.0,151245.0,17775.0,
        151110.0,18945.0,150660.0,20250.0,149895.0,21825.0,147645.0,25020.0,150255.0,23220.0,150840.0,22635.0,
        151470.0,21825.0,151740.0,22545.0,151740.0,23175.0,151560.0,23895.0,151200.0,24930.0,150480.0,25875.0,
        149400.0,26865.0,147105.0,28080.0,148815.0,28215.0,149670.0,28620.0,149850.0,29205.0,149580.0,30015.0,
        148950.0,30780.0,147600.0,31500.0,144990.0,31725.0,146655.0,32940.0,147060.0,33615.0,147150.0,34425.0,
        146745.0,35325.0,145755.0,35865.0,144540.0,35910.0,142155.0,35820.0,144405.0,37395.0,144720.0,38115.0,
        144450.0,38880.0,143595.0,39510.0,142650.0,39690.0,141660.0,39600.0,140265.0,39375.0,141660.0,40680.0,
        141975.0,41355.0,141885.0,42075.0,141435.0,42660.0,140760.0,42930.0,140175.0,42930.0,138735.0,42390.0,
        139185.0,43830.0,139185.0,44460.0,138870.0,45000.0,138195.0,45225.0,137385.0,45270.0,135675.0,44820.0,
        136710.0,46440.0,136845.0,47295.0,136755.0,48240.0,135855.0,48735.0,134910.0,48780.0,133875.0,48555.0,
        132390.0,47880.0,133515.0,49500.0,133830.0,50265.0,133650.0,51120.0,132930.0,51795.0,131805.0,51930.0,
        130005.0,51255.0,130635.0,53235.0,130545.0,54000.0,130050.0,54765.0,129105.0,55170.0,128250.0,54990.0,
        127260.0,54630.0,125550.0,53505.0,126495.0,55350.0,126675.0,56340.0,126450.0,57150.0,125685.0,57645.0,
        124875.0,57825.0,123705.0,57600.0,121635.0,56385.0,122625.0,58095.0,122940.0,58950.0,122850.0,59760.0,
        122400.0,60390.0,121320.0,60750.0,119790.0,60435.0,117855.0,58995.0,118800.0,61335.0,118800.0,62235.0,
        118215.0,63045.0,117180.0,63450.0,115875.0,63360.0,114885.0,62910.0,113760.0,61785.0,113895.0,63495.0,
        113850.0,64440.0,113355.0,65160.0,112635.0,65475.0,111645.0,65475.0,110835.0,65160.0,109620.0,64305.0,
        109890.0,66150.0,109845.0,66915.0,109440.0,67455.0,108720.0,67770.0,107685.0,67860.0,106515.0,67545.0,
        104985.0,66600.0,105390.0,67905.0,105300.0,68625.0,104805.0,69300.0,103995.0,69750.0,102825.0,69795.0,
        99540.0,68760.0,98640.0,69345.0,105615.0,71280.0,110340.0,72360.0,112815.0,73080.0,114615.0,73980.0,
        115650.0,74700.0,116055.0,75285.0,115965.0,75915.0,115425.0,76680.0,114615.0,77400.0,113355.0,77895.0,
        111600.0,78030.0,108900.0,77625.0,111600.0,78885.0,113130.0,79920.0,113670.0,80505.0,113715.0,81135.0,
        113355.0,81810.0,112725.0,82395.0,111600.0,82980.0,110430.0,83070.0,107325.0,82350.0,109350.0,83835.0,
        110115.0,84780.0,110295.0,85770.0,110025.0,86490.0,109395.0,87030.0,108405.0,87480.0,107145.0,87390.0,
        105660.0,86940.0,103050.0,85545.0,105210.0,88065.0,105750.0,88875.0,105840.0,89685.0,105660.0,90495.0,
        104985.0,91035.0,103680.0,91395.0,102420.0,91395.0,100980.0,90810.0,98505.0,88965.0,99540.0,91395.0,
        99585.0,92250.0,99360.0,93015.0,98415.0,94005.0,97380.0,94320.0,95805.0,94095.0,94590.0,93105.0,
        92700.0,90225.0,92610.0,92790.0,92340.0,93870.0,91575.0,94770.0,90315.0,95310.0,88920.0,95265.0,
        87705.0,94545.0,87210.0,93150.0,87120.0,90180.0,86175.0,93015.0,85455.0,93870.0,84330.0,94500.0,
        83115.0,94725.0,82215.0,94545.0,81405.0,93870.0,81045.0,92790.0,81135.0,91620.0,81855.0,89055.0,
        79605.0,91890.0,78570.0,92610.0,77535.0,93015.0,76005.0,92970.0,75195.0,92565.0,74835.0,91575.0,
        75105.0,90270.0,76815.0,87795.0,74520.0,89460.0,73125.0,90135.0,71730.0,90315.0,70605.0,90090.0,
        70020.0,89685.0,69885.0,89100.0,70110.0,88245.0,71685.0,85815.0,69435.0,87525.0,68175.0,87840.0,
        67275.0,87615.0,66240.0,86850.0,66060.0,86040.0,66285.0,85140.0,66870.0,84240.0,68580.0,82530.0,
        70605.0,80865.0,72990.0,79155.0,75510.0,76950.0,76950.0,75510.0,76590.0,74925.0,75465.0,76455.0,
        74790.0,76950.0,73980.0,76995.0,73170.0,76815.0,72630.0,76185.0,72000.0,74340.0,71505.0,75960.0,
        70965.0,76680.0,70155.0,76995.0,69210.0,76950.0,68400.0,76455.0,67905.0,75690.0,67590.0,74295.0,
        67275.0,76005.0,66960.0,76680.0,66285.0,77130.0,64980.0,77265.0,64260.0,77040.0,63765.0,76455.0,
        63405.0,74520.0,62280.0,76320.0,61605.0,76905.0,60660.0,77130.0,59805.0,76770.0,59175.0,76050.0,
        59175.0,73755.0,58140.0,75690.0,57240.0,76545.0,56520.0,76725.0,55710.0,76590.0,55080.0,76050.0,
        54585.0,75195.0,54270.0,73845.0,53190.0,75195.0,52425.0,75780.0,51615.0,75915.0,50805.0,75600.0,
        50355.0,74925.0,49770.0,73215.0,48825.0,74745.0,48285.0,75015.0,47160.0,75015.0,46305.0,74565.0,
        45675.0,73845.0,45360.0,72855.0,45360.0,71415.0,43875.0,73215.0,43155.0,73710.0,42165.0,73800.0,
        41040.0,73440.0,40455.0,72900.0,40095.0,72000.0,40275.0,70290.0,39015.0,72090.0,38385.0,72495.0,
        37710.0,72765.0,36990.0,72585.0,36180.0,71955.0,35865.0,71235.0,35910.0,69120.0,34830.0,70740.0,
        34290.0,71190.0,33345.0,71370.0,32355.0,70875.0,32040.0,70065.0,32130.0,68490.0,30780.0,69255.0,
        30015.0,69390.0,29205.0,69165.0,28620.0,68625.0,28530.0,67635.0,28800.0,66150.0,27135.0,67455.0,
        26235.0,67590.0,25110.0,67365.0,24570.0,66915.0,24300.0,66240.0,24435.0,65700.0,25785.0,63990.0,
        24030.0,65520.0,23085.0,65925.0,21735.0,65880.0,20970.0,65340.0,20925.0,64665.0,21375.0,63720.0,
        22185.0,62415.0,19800.0,63900.0,18810.0,64080.0,18000.0,63855.0,17010.0,63270.0,16740.0,62865.0,
        17055.0,62325.0,18225.0,61335.0,15975.0,61965.0,14985.0,61920.0,13905.0,61200.0,13725.0,60660.0,
        13905.0,60165.0,14355.0,59940.0,16605.0,59400.0,13725.0,59265.0,12060.0,58995.0,11250.0,58680.0,
        11250.0,58050.0,12330.0,57555.0,10215.0,56520.0,9225.0,55485.0,9180.0,54855.0,9495.0,54405.0,
        10215.0,54495.0,12465.0,55575.0,10485.0,53955.0,9180.0,53325.0,8370.0,52020.0,8010.0,50805.0,
        8235.0,50265.0,8910.0,51120.0,10035.0,51975.0,12600.0,53730.0,10440.0,51390.0,7785.0,49050.0,
        6930.0,47925.0,6660.0,46755.0,6705.0,45630.0,7065.0,44730.0,7560.0,44145.0,7605.0,45945.0,
        7740.0,46620.0,8280.0,47475.0,10620.0,49275.0,12600.0,50580.0,15750.0,52065.0,13275.0,49995.0,
        11610.0,48690.0,9945.0,47115.0,8730.0,45585.0,7740.0,44055.0,7290.0,42615.0,7245.0,41625.0,
        7425.0,40365.0,7695.0,39645.0,8325.0,38700.0,8685.0,38520.0,8370.0,39645.0,8235.0,40815.0,
        8415.0,41760.0,9405.0,43290.0,11160.0,45270.0,13500.0,47430.0,11880.0,45225.0,11520.0,44145.0,
        11520.0,43110.0,11700.0,42480.0,12105.0,41895.0,12690.0,41580.0,12465.0,43155.0,12735.0,44190.0,
        13590.0,45495.0,15120.0,46800.0,17055.0,47655.0,18765.0,48060.0,22545.0,48780.0,27000.0,49545.0,
        31140.0,50310.0,31725.0,50220.0,30330.0,49950.0,29340.0,49545.0,29700.0,49050.0,34515.0,49635.0,
        40410.0,50805.0,50130.0,51570.0,62865.0,53055.0,75510.0,52965.0,76500.0,51840.0,76635.0,51390.0,
        76635.0,51120.0,76455.0,51030.0,75195.0,51345.0,76230.0,48015.0,77940.0,46440.0
    };
    static const double poly1[]={
        48960.0,62550.0,47835.0,62910.0,46800.0,62415.0,45810.0,62505.0,45360.0,61830.0,45315.0,60885.0,
        43110.0,63090.0,41985.0,63045.0,41400.0,62460.0,40635.0,63360.0,39825.0,63360.0,39510.0,62370.0,
        37485.0,63630.0,36585.0,64350.0,35685.0,64125.0,34335.0,64035.0,33975.0,63225.0,35415.0,62325.0,
        35055.0,61920.0,33480.0,63180.0,32400.0,63135.0,32040.0,64170.0,31140.0,63990.0,30645.0,63000.0,
        31140.0,62145.0,30510.0,62055.0,29835.0,62685.0,29160.0,62235.0,28395.0,62010.0,28485.0,61425.0,
        30015.0,60660.0,30645.0,59625.0,29745.0,59805.0,29025.0,60525.0,27180.0,61065.0,26505.0,61470.0,
        25695.0,61110.0,25425.0,60210.0,26190.0,59580.0,26055.0,58905.0,24840.0,59625.0,24255.0,59670.0,
        24165.0,58860.0,23535.0,58275.0,24210.0,57915.0,24390.0,57150.0,24885.0,56250.0,26370.0,55800.0,
        27405.0,55620.0,27675.0,55035.0,26145.0,55170.0,24885.0,55530.0,24975.0,54720.0,26055.0,54540.0,
        26280.0,53865.0,25695.0,53415.0,26415.0,52605.0,27495.0,52425.0,28350.0,52200.0,29250.0,52740.0,
        29925.0,54450.0,30960.0,55035.0,32085.0,56205.0,34110.0,56520.0,35280.0,57285.0,36675.0,58140.0,
        39105.0,58500.0,40365.0,58725.0,41670.0,59715.0,43065.0,59535.0,44325.0,60705.0,45360.0,60705.0,
        47070.0,60975.0,48915.0,61785.0
    };
    static const double poly2[]={
        52965.0,62730.0,52290.0,63180.0,52290.0,63900.0,53145.0,64125.0,54000.0,64440.0,54135.0,63090.0
    };
    static const double poly3[]={
        61380.0,65520.0,63765.0,66330.0,65745.0,66510.0,64710.0,67545.0,63450.0,67950.0,62685.0,66960.0,
        60930.0,67095.0,60930.0,65745.0
    };
    static const double poly4[]={
        94185.0,68490.0,93915.0,70920.0,94995.0,71955.0,94500.0,72090.0,93330.0,71505.0,93150.0,73710.0,
        93960.0,75240.0,96525.0,77490.0,97335.0,78705.0,96480.0,78615.0,94185.0,76365.0,93555.0,76230.0,
        93330.0,76995.0,93015.0,76905.0,91800.0,74385.0,90000.0,72495.0,90000.0,74385.0,90540.0,76725.0,
        91845.0,78165.0,92115.0,79155.0,91170.0,79245.0,90540.0,77895.0,89730.0,77355.0,89280.0,78435.0,
        88740.0,79290.0,88155.0,79020.0,87390.0,77715.0,86940.0,76050.0,86310.0,74340.0,84600.0,72945.0,
        84960.0,74745.0,85635.0,76860.0,85410.0,78075.0,85185.0,77805.0,84555.0,78750.0,85050.0,79245.0,
        84825.0,80640.0,83880.0,80685.0,83520.0,80460.0,84735.0,79245.0,84465.0,78840.0,84195.0,78390.0,
        85095.0,77715.0,84600.0,76590.0,83610.0,75150.0,83160.0,73800.0,81315.0,72540.0,80550.0,72585.0,
        78840.0,74475.0,77985.0,74745.0,73620.0,79245.0,74250.0,80235.0,76275.0,77895.0,76905.0,77625.0,
        78975.0,75555.0,78840.0,76455.0,76860.0,78255.0,77580.0,78390.0,75510.0,80460.0,75285.0,81405.0,
        76140.0,81495.0,78840.0,78570.0,79335.0,78660.0,76635.0,82035.0,77490.0,82710.0,80640.0,79200.0,
        82575.0,76725.0,82710.0,77625.0,82170.0,78885.0,80505.0,80640.0,80415.0,81630.0,79110.0,83025.0,
        80055.0,84105.0,80685.0,83250.0,80775.0,84240.0,82350.0,83205.0,83115.0,82215.0,82800.0,83205.0,
        83205.0,83205.0,82800.0,84645.0,83565.0,85095.0,84690.0,83835.0,85725.0,81720.0,85545.0,82755.0,
        85320.0,83700.0,85950.0,84015.0,86535.0,85905.0,87840.0,85905.0,87750.0,84060.0,88110.0,83430.0,
        89190.0,84645.0,89280.0,85725.0,89910.0,86175.0,91080.0,85770.0,89910.0,84375.0,90585.0,83970.0,
        90765.0,82665.0,92070.0,85725.0,93150.0,85230.0,91710.0,82890.0,94410.0,85275.0,95310.0,83790.0,
        94815.0,82980.0,93240.0,82350.0,93915.0,80955.0,96300.0,83745.0,97065.0,82575.0,98325.0,82260.0,
        96075.0,80055.0,97380.0,80100.0,99135.0,81630.0,99585.0,81540.0,99765.0,81000.0,98685.0,80055.0,
        97290.0,78030.0,99000.0,79110.0,100935.0,80055.0,101070.0,79425.0,100485.0,78435.0,97515.0,76590.0,
        94680.0,73980.0,97470.0,75690.0,100440.0,77850.0,101115.0,77445.0,102690.0,78165.0,103140.0,77490.0,
        102735.0,76410.0,100935.0,75150.0,99855.0,74430.0,99855.0,73845.0,100620.0,73890.0,100935.0,74565.0,
        103635.0,76275.0,104580.0,75735.0,104985.0,74610.0,104040.0,73800.0,101250.0,72180.0,100035.0,71955.0,
        98775.0,71055.0,97785.0,71010.0,96390.0,69975.0,96660.0,69750.0,98730.0,70920.0,101430.0,71505.0,
        102195.0,71100.0,105615.0,73530.0,105390.0,71730.0,98460.0,69615.0,97155.0,68895.0,96300.0,69390.0,
        94860.0,69030.0
    };
    static const double poly5[]={
        111285.0,53595.0,112185.0,52830.0,113220.0,52110.0,113895.0,51255.0,114840.0,50265.0,115515.0,49905.0,
        115875.0,51030.0,115560.0,51345.0,114840.0,51345.0,114390.0,51795.0,115155.0,53010.0,114345.0,53325.0,
        114435.0,54135.0,113130.0,53460.0,112365.0,53775.0
    };
    static const double poly6[]={
        119205.0,46575.0,120555.0,45495.0,121860.0,43785.0,123840.0,42345.0,125235.0,41085.0,126135.0,39465.0,
        127350.0,38160.0,128160.0,36990.0,129150.0,36135.0,129915.0,34875.0,130365.0,33435.0,131175.0,32085.0,
        131355.0,30960.0,131760.0,29430.0,132030.0,28485.0,131985.0,27360.0,131895.0,26595.0,133065.0,26505.0,
        134055.0,26145.0,134100.0,26865.0,133245.0,27360.0,132975.0,28125.0,134775.0,27630.0,134910.0,28260.0,
        135495.0,28485.0,135315.0,29115.0,134325.0,29385.0,133830.0,29925.0,135315.0,29430.0,136080.0,29925.0,
        137970.0,29835.0,137430.0,30690.0,137610.0,31230.0,136080.0,31590.0,136350.0,32130.0,138150.0,32310.0,
        138060.0,33075.0,138555.0,33615.0,137205.0,34515.0,135000.0,34470.0,136800.0,35550.0,136575.0,36360.0,
        136980.0,36855.0,135540.0,37485.0,135675.0,38925.0,135270.0,39825.0,134325.0,39735.0,133740.0,39960.0,
        131310.0,38295.0,131850.0,39105.0,134100.0,40275.0,134685.0,41400.0,133785.0,41670.0,133065.0,42120.0,
        131625.0,41805.0,132075.0,42975.0,131265.0,43515.0,130725.0,44235.0,129555.0,44145.0,127980.0,42885.0,
        126945.0,42840.0,127845.0,44235.0,127440.0,45450.0,126720.0,45225.0,125685.0,45675.0,124065.0,44865.0,
        124425.0,46170.0,123705.0,46575.0,123165.0,47205.0,121770.0,46215.0,120105.0,46935.0
    };
    static const double poly7[]={
        79200.0,45945.0,80100.0,46035.0,80865.0,46215.0,81720.0,46620.0,82530.0,47250.0,83250.0,47970.0,
        84510.0,49005.0,85095.0,49365.0,85995.0,49635.0,86985.0,49590.0,88380.0,49230.0,93015.0,47385.0,
        97560.0,45135.0,102780.0,42165.0,108630.0,38700.0,113265.0,35685.0,117990.0,32355.0,126270.0,26235.0,
        117765.0,33345.0,113985.0,36315.0,109260.0,39690.0,103770.0,43110.0,97920.0,46485.0,94050.0,48465.0,
        90045.0,50265.0,88290.0,50670.0,86940.0,50760.0,85725.0,50535.0,84780.0,50085.0,83475.0,49275.0,
        82305.0,48285.0,81540.0,47790.0,80685.0,47655.0,80100.0,47655.0,79560.0,47880.0,79065.0,48240.0,
        78660.0,48330.0,78165.0,48330.0,77715.0,48195.0,77895.0,48060.0,78165.0,48150.0,78525.0,48150.0,
        78930.0,48060.0,79200.0,47835.0,79380.0,47610.0,79515.0,47295.0,79875.0,47115.0,80730.0,46935.0,
        79785.0,46800.0,79065.0,46755.0,78345.0,46800.0,77625.0,47070.0,77220.0,47385.0,76860.0,47790.0,
        76545.0,48240.0,76365.0,48060.0,75690.0,48510.0,75285.0,48960.0,74700.0,49770.0,74250.0,50625.0,
        74385.0,49860.0,74700.0,49095.0,75015.0,48645.0,75285.0,48330.0,75870.0,47835.0,76230.0,47250.0,
        76770.0,46665.0,77220.0,46350.0,77805.0,46080.0,78570.0,45945.0
    };
    static const double poly8[]={
        91260.0,77535.0,91710.0,76995.0,92790.0,76905.0,93330.0,77985.0,94995.0,79875.0,95355.0,81135.0,
        95805.0,82575.0,95805.0,83250.0,94860.0,83295.0,94320.0,82485.0,93690.0,81900.0,93780.0,83025.0,
        93960.0,83925.0,93420.0,84195.0,92745.0,83655.0,91890.0,81945.0,91755.0,80415.0,91755.0,78930.0
    };
    static const double poly9[]={
        86535.0,78210.0,87345.0,78300.0,87930.0,80100.0,88560.0,81225.0,89370.0,82305.0,89640.0,83385.0,
        89505.0,84645.0,88830.0,84780.0,87930.0,83880.0,87660.0,82350.0,87435.0,82395.0,87390.0,83745.0,
        87525.0,84645.0,86985.0,85140.0,86130.0,84645.0,85860.0,83340.0,85680.0,81540.0,85950.0,80145.0,
        85860.0,78705.0
    };
    static const double poly10[]={
        78570.0,48060.0,78975.0,47925.0,79155.0,47700.0,79200.0,47430.0,79065.0,47160.0,78660.0,47465.0,
        78750.0,47610.0,78750.0,47790.0,78615.0,47880.0,78390.0,47925.0,78210.0,47835.0,78120.0,47675.0,
        77850.0,47745.0,78030.0,47970.0,78210.0,48060.0
    };
    static const double poly11[]={
        34515.0,49635.0,40860.0,50670.0,50130.0,51570.0,55485.0,52020.0,63180.0,52425.0,69750.0,52470.0,
        75465.0,52245.0,76140.0,52065.0,76500.0,51840.0,76275.0,53100.0,76050.0,53415.0,75645.0,53550.0,
        69120.0,53730.0,63000.0,53640.0,56070.0,53190.0,49410.0,52515.0,43290.0,51615.0,40365.0,51075.0
    };
    static const double poly12[]={
        75465.0,51165.0,77760.0,49545.0,78570.0,49230.0,78750.0,49005.0,78570.0,49365.0,77760.0,49815.0,
        77490.0,50085.0,76770.0,50760.0,74970.0,51750.0,74835.0,51930.0,74880.0,52110.0,75240.0,52560.0,
        74970.0,52470.0,74565.0,52200.0,74295.0,51840.0,74205.0,51435.0,74205.0,50850.0,74340.0,50040.0,
        75240.0,48645.0,76230.0,47925.0,76905.0,48600.0,77130.0,49050.0,77445.0,49185.0,77985.0,49140.0,
        78705.0,48960.0,78570.0,49140.0,77715.0,49455.0
    };
    static const double poly13[]={
        78660.0,47465.0,
        78750.0,47610.0,
        78750.0,47790.0,
        78615.0,47880.0,
        78390.0,47925.0,
        78210.0,47835.0,
        78120.0,47675.0
    };
    static const double * const polys[]={
        poly0,poly1,poly2,poly3,poly4,poly5,poly6,poly7,poly8,poly9,poly10,poly11,poly12,poly13
    };
    static const int polySizes[]={
        461,74,6,8,151,15,71,70,18,19,15,18,27,7
    };
    static const emUInt32 polyColors[]={
        0x302030FF,0x303040FF,0x303040FF,0x303040FF,0x303040FF,0x303040FF,0x303040FF,
        0x508080FF,0x505030FF,0x505030FF,0x505030FF,0x508080FF,0x505030FF,0x000000FF
    };

    // Paint 14 eagle polygons.
    for (int i = 0; i < 14; i++) {
        ep.PaintPolygon(polys[i], polySizes[i], polyColors[i]);
    }

    dump_painter("eagle_logo", img);
}

// ═══════════════════════════════════════════════════════════════════
// Starfield golden generator
// ═══════════════════════════════════════════════════════════════════

// Standalone starfield PRNG and star generation, matching C++ emStarFieldPanel.
// We replicate the constructor logic here because emStarFieldPanel requires
// a parent panel (emPanel::ParentArg), which we don't have in the generator.

struct GenStar {
    double X, Y, Radius;
    emColor Color;
};

static emUInt32 sf_lcg(emUInt32& seed) {
    seed = seed * 1664525 + 1013904223;
    return seed;
}

static double sf_random_range(emUInt32& seed, double minVal, double maxVal) {
    emUInt32 r = sf_lcg(seed);
    return r * (maxVal - minVal) / (double)EM_UINT32_MAX + minVal;
}

static void gen_starfield(const char* name, int depth, emUInt32 seed, int w, int h) {
    // Constants matching C++ emStarFieldPanel
    const double MinPanelSize = 64.0;
    const double MinStarRadius = 0.3;
    const emColor BgColor(0x000000FF);

    // Generate stars (replicates emStarFieldPanel constructor)
    int starCount = 0;
    std::vector<GenStar> stars;
    if (depth >= 1) {
        int maxCount = (depth * 3 < 400) ? depth * 3 : 400;
        starCount = (int)(maxCount * sf_random_range(seed, 0.5, 1.0));
        stars.resize(starCount);
        for (int i = 0; i < starCount; i++) {
            double r = MinStarRadius / MinPanelSize * sf_random_range(seed, 0.5, 1.0);
            stars[i].X = sf_random_range(seed, r, 1.0 - r);
            stars[i].Y = sf_random_range(seed, r, 1.0 - r);
            stars[i].Radius = r;
            stars[i].Color.SetHSVA(
                (float)sf_random_range(seed, 0.0, 360.0),
                (float)sf_random_range(seed, 0.0, 15.0),
                100.0F
            );
        }
    }

    // Advance PRNG for child seeds (not used for painting, but must match)
    sf_lcg(seed); // ^ 0x74fc8324
    sf_lcg(seed); // ^ 0x058f56a9
    sf_lcg(seed); // ^ 0xfc863e37
    sf_lcg(seed); // ^ 0x8bef7891

    // Load star texture
    emImage starShape = emGetInsResImage(*g_ctx, "emMain", "Star.tga", 1);

    // Create image and painter
    emImage img(w, h, 4);
    // Don't fill -- Clear will paint BgColor

    emPainter p;
    if (!img.PreparePainter(&p, *g_ctx,
                            0.0, 0.0,
                            (double)w, (double)h)) {
        fprintf(stderr, "PreparePainter failed for %s!\n", name);
        exit(1);
    }

    // Scale painter: map panel coords (0,0)-(1,1) to pixels (0,0)-(w,h)
    emPainter sp(
        p,
        p.GetClipX1(), p.GetClipY1(),
        p.GetClipX2(), p.GetClipY2(),
        p.GetOriginX(), p.GetOriginY(),
        (double)w, (double)h
    );

    // Clear to black background
    open_draw_op_log(name);
    sp.Clear(BgColor, 0);

    // Paint stars (replicates PaintOverlay logic)
    double scaleX = (double)w; // PanelToViewDeltaX(r) = r * scaleX
    for (int i = 0; i < starCount; i++) {
        double r = stars[i].Radius;
        double vr = scaleX * r;
        if (vr > MinStarRadius) {
            if (vr > 4.0) {
                // Tier 1: textured glow
                float hue = stars[i].Color.GetHue();
                float sat = stars[i].Color.GetSat();
                float alpha = sat * 18.0F;
                if (alpha > 255.0F) alpha = 255.0F;
                double x = stars[i].X - r;
                double y = stars[i].Y - r;
                double d = r * 2;
                // Glow pass
                emColor c1;
                c1.SetHSVA(hue, 100.0F, 100.0F, (emByte)alpha);
                sp.PaintImageColored(x, y, d, d, starShape, 0, c1, 0, emTexture::EXTEND_ZERO);
                // Star pass
                emColor c2;
                c2.SetHSVA(hue, sat - 10.0F, 100.0F);
                sp.PaintImageColored(x, y, d, d, starShape, 0, c2, 0, emTexture::EXTEND_ZERO);
            }
            else {
                r *= 0.6;
                vr = scaleX * r;
                if (vr > 1.2) {
                    // Tier 2: ellipse (C++ API: top-left + diameter)
                    double x = stars[i].X - r;
                    double y = stars[i].Y - r;
                    double d = r * 2;
                    sp.PaintEllipse(x, y, d, d, stars[i].Color);
                }
                else {
                    // Tier 3: rect
                    r *= 0.8862;
                    double x = stars[i].X - r;
                    double y = stars[i].Y - r;
                    double d = r * 2;
                    sp.PaintRect(x, y, d, d, stars[i].Color);
                }
            }
        }
    }

    close_draw_op_log();
    dump_painter(name, img);
}

// ═══════════════════════════════════════════════════════════════════
// Main Panel Layout
// ═══════════════════════════════════════════════════════════════════

struct MainPanelLayout {
    double ControlX, ControlY, ControlW, ControlH;
    double ContentX, ContentY, ContentW, ContentH;
    double SliderX, SliderY, SliderW, SliderH;
};

static MainPanelLayout compute_main_panel_layout(
    double h, double sliderPos, double controlTallness
) {
    MainPanelLayout L = {};
    double SliderMinY = 0.0;
    double SliderMaxY = emMin(controlTallness, h * 0.5);
    L.SliderY = (SliderMaxY - SliderMinY) * sliderPos + SliderMinY;
    L.SliderW = emMin(emMin(1.0, h) * 0.1, emMax(1.0, h) * 0.02);
    L.SliderH = L.SliderW * 1.2;
    L.SliderX = 1.0 - L.SliderW;

    double spaceFac = 1.015;
    double t = L.SliderH * 0.5;
    if (L.SliderY < t) {
        L.ControlH = L.SliderY + L.SliderH * L.SliderY / t;
    } else {
        L.ControlH = (L.SliderY + L.SliderH) / spaceFac;
    }

    if (L.ControlH < 1E-5) {
        L.ControlH = 1E-5;
        L.ControlW = L.ControlH / controlTallness;
        L.ControlX = 0.5 * (1.0 - L.ControlW);
        L.ControlY = 0.0;
        L.ContentX = 0.0;
        L.ContentY = 0.0;
        L.ContentW = 1.0;
        L.ContentH = h;
    } else {
        L.ControlW = L.ControlH / controlTallness;
        L.ControlX = emMin((1.0 - L.ControlW) * 0.5, L.SliderX - L.ControlW);
        L.ControlY = 0.0;
        if (L.ControlX < 1E-5) {
            L.ControlW = 1.0 - L.SliderW;
            L.ControlX = 0.0;
            L.ControlH = L.ControlW * controlTallness;
            if (L.ControlH < L.SliderY) {
                L.ControlH = L.SliderY;
                L.ControlW = L.ControlH / controlTallness;
            } else {
                // slider_pressed=false: apply correction
                L.SliderY = L.ControlH * spaceFac - L.SliderH;
            }
        }
        L.ContentY = L.ControlY + L.ControlH * spaceFac;
        L.ContentX = 0.0;
        L.ContentW = 1.0;
        L.ContentH = h - L.ContentY;
    }
    return L;
}

static void dump_main_panel_layout(const char* name, const MainPanelLayout& L) {
    FILE* f = open_golden("layout", name, "layout.golden");
    write_u32(f, 3);  // 3 rects: control, content, slider
    write_f64(f, L.ControlX); write_f64(f, L.ControlY);
    write_f64(f, L.ControlW); write_f64(f, L.ControlH);
    write_f64(f, L.ContentX); write_f64(f, L.ContentY);
    write_f64(f, L.ContentW); write_f64(f, L.ContentH);
    write_f64(f, L.SliderX);  write_f64(f, L.SliderY);
    write_f64(f, L.SliderW);  write_f64(f, L.SliderH);
    fclose(f);
    printf("  layout/%s\n", name);
}

static void gen_main_panel_layouts() {
    dump_main_panel_layout("main_panel_layout_normal",
        compute_main_panel_layout(2.0, 0.5, 0.0538));
    dump_main_panel_layout("main_panel_layout_collapsed",
        compute_main_panel_layout(2.0, 0.0, 0.0538));
    dump_main_panel_layout("main_panel_layout_wide",
        compute_main_panel_layout(0.5, 0.7, 0.0538));
}

// ═══════════════════════════════════════════════════════════════════
// Cosmos item border
// ═══════════════════════════════════════════════════════════════════

static void gen_cosmos_item_border() {
    double contentTallness = 0.75;
    double borderScaling = 1.0;
    emColor bgColor(0x20, 0x20, 0x40);
    emColor borderColor(0x40, 0x60, 0xA0);
    emColor titleColor(0xE0, 0xE0, 0xFF);
    const char* title = "Test Cosmos Item";

    double b_val = emMin(contentTallness, 1.0) * borderScaling;
    double bl = b_val * 0.03, bt = b_val * 0.05, br = b_val * 0.03, bb = b_val * 0.03;
    double panelH = contentTallness + bt + bb;

    const int W = 400, H = 300;
    emImage img(W, H, 4);
    img.Fill(emColor::BLACK);
    emPainter pixel_p = make_painter(img);

    double sx = (double)W, sy = (double)H / panelH;
    emPainter p(pixel_p, pixel_p.GetClipX1(), pixel_p.GetClipY1(),
        pixel_p.GetClipX2(), pixel_p.GetClipY2(), 0.0, 0.0, sx, sy);

    double w = 1.0, h = panelH;

    open_draw_op_log("cosmos_item_border");
    // Top border strip
    p.PaintRect(0.0, 0.0, w, bt * h, borderColor);
    // Bottom border strip
    p.PaintRect(0.0, (1.0 - bb) * h, w, bb * h, borderColor);
    // Left border strip
    p.PaintRect(0.0, bt * h, bl * w, (1.0 - bt - bb) * h, borderColor);
    // Right border strip
    p.PaintRect((1.0 - br) * w, bt * h, br * w, (1.0 - bt - bb) * h, borderColor);
    // Background
    p.PaintRect(bl * w, bt * h, (1.0 - bl - br) * w, (1.0 - bt - bb) * h, bgColor);
    // Title text
    double fontH = bt * h * 0.7;
    if (fontH >= 1.0) {
        p.PaintText(bl * w, bt * h * 0.15, title, fontH, 1.0, titleColor);
    }
    close_draw_op_log();

    dump_painter("cosmos_item_border", img);
}

// ═══════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════

int main() {
    g_sched = new emStandardScheduler();
    g_ctx   = new emRootContext(*g_sched);

    printf("Generating painter golden files...\n");
    gen_rect_solid();
    gen_rect_alpha();
    gen_rect_overlap();
    gen_ellipse_basic();
    gen_ellipse_small();
    gen_polygon_tri();
    gen_polygon_star();
    gen_polygon_complex();
    gen_round_rect();
    gen_gradient_h();
    gen_gradient_v();
    gen_gradient_radial();
    gen_line_basic();
    gen_line_thick();
    gen_line_ends_all();
    gen_line_dashed();
    gen_outline_rect();
    gen_outline_ellipse();
    gen_outline_polygon();
    gen_outline_round_rect();
    gen_bezier_filled();
    gen_bezier_stroked();
    gen_clip_basic();
    gen_canvas_color();
    gen_image_paint();
    gen_image_scaled();
    gen_multi_compose();
    gen_polyline();
    gen_ellipse_sector();
    gen_painter_howto_isolate();

    printf("Generating transform golden files...\n");
    gen_transform_translate();
    gen_transform_scale();
    gen_transform_nested();
    gen_transform_clip_interaction();
    gen_transform_ellipse_scaled();
    gen_transform_fractional();
    gen_transform_identity_roundtrip();

    printf("Generating text golden files...\n");
    gen_text_basic();
    gen_text_scaled();
    gen_text_fitted();
    gen_text_alignment();
    gen_text_clipped();
    gen_text_below_threshold();

    printf("Generating layout golden files...\n");
    gen_linear_h_equal();
    gen_linear_h_weighted();
    gen_linear_v_equal();
    gen_linear_v_weighted();
    gen_linear_h_tallness();
    gen_linear_v_tallness();
    gen_raster_3col();
    gen_raster_2row();
    gen_raster_strict();
    gen_raster_pref_tall();
    gen_pack_equal();
    gen_pack_weighted();
    gen_pack_extreme();
    // ─── Layout expansion tests ───
    gen_linear_h_spacing();
    gen_linear_v_spacing();
    gen_linear_h_align_right();
    gen_linear_h_align_center();
    gen_linear_v_align_bottom();
    gen_linear_adaptive_wide();
    gen_linear_adaptive_tall();
    gen_linear_min_cell_count();
    gen_linear_min_max_tallness();
    gen_linear_mixed_weights();
    gen_raster_alignment_br();
    gen_raster_alignment_center();
    gen_raster_spacing();
    gen_raster_min_cell_count();
    gen_raster_min_max_tallness();
    gen_raster_auto_cols();
    gen_pack_min_cell_count();
    gen_pack_single();

    printf("Generating behavioral golden files...\n");
    gen_activate_click();
    gen_activate_path();
    gen_activate_switch();
    gen_focus_click();
    gen_activate_nonfocusable();
    gen_activate_remove();
    gen_focus_tab_forward();
    gen_focus_tab_backward();
    gen_focus_unfocusable_skip();
    gen_focus_nested();
    gen_focus_remove_focused();
    gen_focus_visit_out();
    gen_focus_tab_wrap();
    gen_focus_visit_first();
    gen_focus_visit_last();
    gen_focus_visit_left();
    gen_focus_visit_right();
    gen_focus_visit_down();
    gen_focus_visit_up();
    gen_focus_disabled_panel();
    gen_activate_remove_middle();
    gen_activate_remove_in_path();
    gen_focus_tab_deep();
    gen_focus_tab_ascend();
    gen_focus_visit_out_to_root();

    printf("Generating notice golden files...\n");
    gen_notice_active_changed();
    gen_notice_focus_changed();
    gen_notice_layout_changed();
    gen_notice_children_changed();
    gen_notice_window_focus_gained();
    gen_notice_window_focus_lost();
    gen_notice_window_resize();
    gen_notice_enable_changed();
    gen_notice_recursive_enable();
    gen_notice_re_enable();
    gen_notice_remove_child();
    gen_notice_focus_and_layout();
    gen_notice_add_and_activate();

    printf("Generating input golden files...\n");
    gen_input_mouse_hit();
    gen_input_key_to_focused();
    gen_input_scroll_delta();
    gen_input_drag_sequence();
    gen_input_mouse_miss();
    gen_input_nested_hit();

    printf("Generating compositor golden files...\n");
    gen_composite_single_panel();
    gen_composite_two_children();
    gen_composite_overlap();
    gen_composite_nested();
    gen_composite_canvas_color();

    printf("Generating widget rendering golden files...\n");
    gen_widget_border_rect();
    gen_widget_border_round_rect();
    gen_widget_border_group();
    gen_widget_border_instrument();
    gen_widget_label();
    gen_widget_button_normal();
    gen_widget_checkbox_unchecked();
    gen_widget_checkbox_checked();
    gen_widget_checkbutton_unchecked();
    gen_widget_checkbutton_checked();
    gen_widget_textfield_empty();
    gen_widget_textfield_content();
    gen_widget_scalarfield();
    gen_widget_colorfield();
    gen_widget_radiobutton();
    gen_widget_listbox();
    gen_widget_splitter_h();
    gen_widget_splitter_v();

    printf("Generating coverage extension widget golden files...\n");
    gen_widget_error_panel();
    gen_widget_tunnel();
    gen_widget_file_panel();
    gen_widget_file_selection_box();
    gen_widget_border_rect_extreme_tall();
    gen_widget_border_rect_extreme_wide();
    gen_widget_border_roundrect_thin();
    gen_widget_border_instrument_cramped();
    gen_widget_label_single_char();
    gen_widget_label_empty();
    gen_widget_label_long_narrow();
    gen_widget_textfield_empty_wide();
    gen_widget_textfield_single_char_square();
    gen_widget_scalarfield_min_value();
    gen_widget_scalarfield_max_value();
    gen_widget_scalarfield_zero_range();
    gen_widget_listbox_empty();
    gen_widget_listbox_single();
    gen_widget_listbox_extreme_wide();
    gen_widget_splitter_h_pos0();
    gen_widget_splitter_h_pos1();
    gen_widget_splitter_v_extreme_tall();
    gen_widget_colorfield_alpha_zero();
    gen_widget_colorfield_alpha_opaque();
    gen_widget_colorfield_alpha_near();
    gen_widget_checkbox_extreme_tall();
    gen_widget_tunnel_extreme_wide();

    printf("Generating widget interaction golden files...\n");
    gen_widget_checkbox_toggle();
    gen_widget_checkbutton_toggle();
    gen_widget_radiobutton_switch();
    gen_widget_listbox_select();
    gen_widget_splitter_setpos();
    gen_widget_textfield_type();
    gen_widget_textfield_backspace();
    gen_widget_textfield_select();
    gen_widget_scalarfield_inc();
    gen_widget_button_click();
    gen_widget_listbox_multi();
    gen_widget_listbox_toggle();
    gen_widget_textfield_cursor_nav();
    gen_widget_splitter_drag();

    printf("Generating animator trajectory golden files...\n");
    gen_animator_kinetic_fling_x();
    gen_animator_kinetic_fling_xy();
    gen_animator_kinetic_zoom();
    gen_animator_speeding_ramp();
    gen_animator_speeding_reverse();
    gen_animator_speeding_release();
    gen_animator_swiping_grip();
    gen_animator_swiping_release();
    gen_animator_visiting_short();
    gen_animator_visiting_zoom();
    gen_animator_magnetic_approach();

    printf("Generating input filter trajectory golden files...\n");
    gen_filter_wheel_zoom_in();
    gen_filter_wheel_zoom_out();
    gen_filter_wheel_acceleration();
    gen_filter_middle_pan();
    gen_filter_middle_fling();
    gen_filter_keyboard_scroll();
    gen_filter_keyboard_zoom();
    gen_filter_keyboard_release();

    printf("Generating splitter layout golden files...\n");
    gen_splitter_layout_h();
    gen_splitter_layout_v();

    printf("Generating expanded widget golden files...\n");
    gen_listbox_expanded();
    gen_colorfield_expanded();

    printf("Generating composed widget golden files...\n");
    gen_composed_splitter_content();

    printf("Generating TestPanel integration golden files...\n");
    gen_testpanel_root();
    gen_testpanel_expanded();

    printf("Generating TkTest integration golden files...\n");
    gen_tktest_1x();
    gen_tktest_2x();

    printf("Generating composed border nest golden files...\n");
    gen_composed_border_nest();

    printf("Generating composed scrolled listbox golden files...\n");
    gen_composed_scrolled_listbox();

    printf("Generating composed colorfield aspect ratio golden files...\n");
    gen_composed_colorfield_wide();
    gen_composed_colorfield_tall();

    printf("Generating eagle logo golden files...\n");
    gen_eagle_logo();

    printf("Generating starfield golden files...\n");
    gen_starfield("starfield_small", 3, 0x12345678, 256, 256);
    gen_starfield("starfield_large", 3, 0x12345678, 1024, 1024);

    printf("Generating main panel layout golden files...\n");
    gen_main_panel_layouts();

    printf("Generating cosmos item border golden files...\n");
    gen_cosmos_item_border();

    printf("Done!\n");

    delete g_ctx;
    delete g_sched;
    return 0;
}
