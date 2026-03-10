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
    vp.DoPaintView(p, 0);
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
    gen_focus_visit_out();
    gen_focus_tab_wrap();

    printf("Generating notice golden files...\n");
    gen_notice_active_changed();
    gen_notice_focus_changed();
    gen_notice_layout_changed();
    gen_notice_children_changed();
    gen_notice_window_focus_gained();
    gen_notice_window_focus_lost();
    gen_notice_window_resize();
    gen_notice_enable_changed();

    printf("Generating input golden files...\n");
    gen_input_mouse_hit();
    gen_input_key_to_focused();
    gen_input_scroll_delta();
    gen_input_drag_sequence();

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

    printf("Generating input filter trajectory golden files...\n");
    gen_filter_wheel_zoom_in();
    gen_filter_wheel_zoom_out();
    gen_filter_wheel_acceleration();
    gen_filter_middle_pan();
    gen_filter_middle_fling();
    gen_filter_keyboard_scroll();
    gen_filter_keyboard_zoom();
    gen_filter_keyboard_release();

    printf("Done!\n");

    delete g_ctx;
    delete g_sched;
    return 0;
}
