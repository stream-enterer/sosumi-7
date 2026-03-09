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

#include "golden_format.h"

// ═══════════════════════════════════════════════════════════════════
// Globals
// ═══════════════════════════════════════════════════════════════════

static emStandardScheduler* g_sched = nullptr;
static emRootContext*        g_ctx   = nullptr;

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
    p.PaintBezierLine(bezier_pts, 4, 3.0,
                      emRoundedStroke(emColor::BLACK),
                      emStrokeEnd(emStrokeEnd::ARROW, emColor::WHITE),
                      emStrokeEnd(emStrokeEnd::ARROW, emColor::WHITE));
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

    printf("Generating notice golden files...\n");
    gen_notice_active_changed();
    gen_notice_focus_changed();
    gen_notice_layout_changed();
    gen_notice_children_changed();
    gen_notice_window_focus_gained();
    gen_notice_window_focus_lost();
    gen_notice_window_resize();

    printf("Generating input golden files...\n");
    gen_input_mouse_hit();
    gen_input_key_to_focused();
    gen_input_scroll_delta();
    gen_input_drag_sequence();

    printf("Done!\n");

    delete g_ctx;
    delete g_sched;
    return 0;
}
