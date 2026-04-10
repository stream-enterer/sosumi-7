# FFI Harness Extension: Layers 8–10

**Date:** 2026-04-10
**Status:** Draft
**Goal:** Mechanically diagnose remaining 36 golden test divergences by extending the FFI harness upward from layer 7.

## Context

The FFI harness at `.worktrees/harness-prototype/` has proven layers 1–7 byte-perfect (blend hash, source-over, area sampling, coverage, colored blend G2, adaptive luminance). 36 golden tests still fail. Past sessions repeatedly fell into an antipattern of reading Rust code, forming hypotheses, and trial-and-error fixing — this has never produced correct diagnoses. The FFI harness is the only proven diagnostic tool.

### Failure Distribution

| Group | Tests | Count | Suspected Layer |
|-------|-------|-------|-----------------|
| ±1 interpolation | splitter_content, image_scaled, multi_compose | 3 | Layer 8 or 10 |
| 9-slice border (A) | scalarfield ×4, button_normal, listbox ×3, checkbox, textfield ×3, radiobutton, splitter_v, colorfield ×5 | 19 | Layer 8 |
| Polygon FP | bezier_stroked, starfield_large, starfield_small | 3 | Layer 9 |
| RoundRect outline | border_roundrect_thin | 1 | Layer 9 |
| Structural | cosmos_item_border | 1 | Layer 9 |
| Composite | border_nest, testpanel ×2, tktest ×2 | 5 | Layers 8+9 |
| Wrong test setup | eagle_logo | 1 | Test rewrite needed |
| Stroke/missing | checkbox_checked, file_selection_box | 2 | Layer 9 |

### Anti-Patterns (Enforced)

1. **No code-reading diagnosis.** Every hypothesis must come from mechanical comparison.
2. **No catalog trust.** The failure catalog contains LLM guesses, many verified wrong.
3. **No fix attempts without FFI proof.** If the harness hasn't identified the root cause, don't change rendering code.

## Architecture

### Existing Harness Pattern

```
┌─────────────────┐     ┌──────────────┐
│ C++ test binary  │────▶│ C++ reference │─── compare ───▶ PASS/FAIL
│ (test_*.cpp)     │     │ function      │                    │
│                  │────▶│ Rust FFI      │────────────────────┘
│                  │     │ (libem_harness)│
└─────────────────┘     └──────────────┘
```

Each test:
1. Sets up identical inputs for both C++ and Rust
2. Calls C++ reference function, captures output
3. Calls Rust FFI function with same inputs, captures output
4. Compares byte-for-byte, reports mismatches

### Extension Strategy: Layered Bisection

For each new layer, export **two** FFI functions:
- **Intermediates function**: Returns computed intermediate values (boundary coords, span lists, interpolation buffers) — pinpoints WHERE divergence enters.
- **Pipeline function**: Runs the full rendering pipeline for that layer on a test framebuffer — confirms the intermediate divergence produces the observed pixel divergence.

When intermediates match but pixels diverge → bug is in the next layer up.
When intermediates diverge → bug is in THIS layer's computation.

## Layer 8: PaintBorderImage 9-Slice Boundaries

**Covers:** 19+ of 36 failures (Groups A, B, D, plus composites).

### Root Question

Does Rust compute the same 9-slice boundary coordinates as C++? PaintBorderImage calls `RoundX`/`RoundY` to snap inset boundaries to pixel grid, then makes 9 `PaintImage` calls. If the boundaries differ by even 1 pixel, every widget border test fails.

### FFI Functions to Export

#### 8a: `rust_border_image_boundaries`

Computes the 9-slice boundaries without painting.

```c
// Input: painter transform + border image parameters
struct CBorderImageParams {
    double x, y, w, h;          // Target rect (logical)
    double l, t, r, b;          // Inset distances (logical)
    int src_w, src_h;           // Source image dimensions
    int src_l, src_t, src_r, src_b;  // Source insets (pixels)
    double scale_x, scale_y;   // Painter transform scale
    double origin_x, origin_y; // Painter transform origin
    int canvas_opaque;          // Whether canvas_color is opaque (affects rounding)
};

// Output: 4 boundary values in logical space + 9 target rects in pixel space
struct CBorderImageBoundaries {
    double adj_l, adj_t, adj_r, adj_b;  // Adjusted insets after RoundX/RoundY
    // 9 target rects (x, y, w, h) in pixel space:
    double rects[9][4];  // UL, U, UR, L, C, R, LL, B, LR
    // 9 source rects (x, y, w, h) in image pixels:
    int src_rects[9][4];
};

int rust_border_image_boundaries(
    const CBorderImageParams* params,
    CBorderImageBoundaries* out
);
```

**Implementation:** Create a temporary `emPainter` with the given transform, call the boundary computation logic from `PaintBorderImage` (lines 2287-2346 in emPainter.rs), but instead of painting, collect and return the 9 rect coordinates.

**Why this function:** If boundaries match C++ but pixels still diverge, the bug is in paint_image_rect (already proven at layer 3/7). If boundaries differ, we've found the root cause for 19+ tests without touching rendering code.

#### 8b: `rust_paint_border_image`

Full PaintBorderImage pipeline on a test framebuffer.

```c
int rust_paint_border_image(
    const CBorderImageParams* params,
    const unsigned char* img_data,  // Source image RGBA
    int img_channels,
    unsigned char canvas_r, unsigned char canvas_g,
    unsigned char canvas_b, unsigned char canvas_a,
    unsigned char alpha,
    int which_sub_rects,
    // Output framebuffer (pre-allocated, filled with canvas color)
    unsigned char* framebuffer,
    int fb_w, int fb_h
);
```

**Implementation:** Create `emPainter` over the framebuffer, set transform, call `PaintBorderImage`. Return the modified framebuffer for pixel comparison.

### C++ Tests to Write

#### `test_border_boundaries.cpp`

Compare boundary computation for the exact parameters used by failing golden tests.

```
For each of {checkbox_unchecked, listbox_normal, textfield_normal, colorfield_hue}:
  1. Extract PaintBorderImage params from golden test setup
  2. Call C++ PaintBorderImage, instrument to capture boundary rects
  3. Call rust_border_image_boundaries with same params
  4. Compare all 9 rect coordinates (f64 epsilon 1e-10)
  5. Report: MATCH or DIVERGE with exact values
```

**Extracting C++ boundaries:** Two options (implementor chooses):
1. **Instrumented build:** Copy emPainter.cpp into the harness directory, add a `PaintBorderImage_Boundaries()` wrapper that captures the 9 rect params passed to each PaintImage call, then returns them. Link against the instrumented version.
2. **Standalone reimplementation:** Reimplement the ~20-line boundary computation (RoundX/RoundY + 9 rect formulas) directly in the test file. This is simpler since the boundary logic is pure arithmetic with no dependencies beyond ScaleX/ScaleY/OriginX/OriginY.

#### `test_border_e2e.cpp`

Full pipeline comparison for representative test cases.

```
For each of {checkbox_unchecked, scalarfield_hue}:
  1. Load actual GroupBorder.tga (or equivalent border image)
  2. Create C++ painter, paint border image, capture framebuffer
  3. Call rust_paint_border_image with same params
  4. Compare framebuffers pixel-by-pixel
  5. Report divergent pixel count, max_diff, locations
```

## Layer 9: Polygon Rasterizer

**Covers:** 6+ of 36 failures (bezier_stroked, starfield_large/small, border_roundrect_thin, cosmos_item_border, checkbox_checked, file_selection_box).

### Root Question

Does Rust's polynomial forward-difference rasterization produce the same per-scanline spans as C++? The algorithm is identical in structure (both use quadratic A0/A1/A2 accumulation), but f64 rounding in edge clipping or coverage accumulation could diverge.

### FFI Functions to Export

#### 9a: `rust_rasterize_polygon`

Rasterization only — vertices to spans.

```c
struct CPolygonVertex {
    double x, y;  // Already in pixel space
};

struct CSpan {
    int x_start, x_end;
    int opacity_beg;   // 0-4096
    int opacity_mid;
    int opacity_end;
};

struct CScanlineSpans {
    int y;
    int span_count;
    CSpan spans[64];  // Max spans per scanline (generous)
};

int rust_rasterize_polygon(
    const CPolygonVertex* vertices, int n_vertices,
    double clip_x1, double clip_y1, double clip_x2, double clip_y2,
    int winding_rule,  // 0=NonZero, 1=EvenOdd
    CScanlineSpans* out_scanlines,
    int max_scanlines,
    int* out_scanline_count
);
```

**Implementation:** Call `emPainterScanline::rasterize()` with the given vertices and clip bounds. Marshal the `Vec<(i32, Vec<Span>)>` output into C-compatible structs.

**Why this function:** If spans match C++ but pixels diverge, the bug is in blitting (already partially proven). If spans differ, we've found the polygon rasterizer divergence.

#### 9b: `rust_paint_polygon`

Full polygon paint on a test framebuffer.

```c
int rust_paint_polygon(
    const CPolygonVertex* vertices, int n_vertices,
    double scale_x, double scale_y,
    double origin_x, double origin_y,
    unsigned char color_r, unsigned char color_g,
    unsigned char color_b, unsigned char color_a,
    unsigned char canvas_r, unsigned char canvas_g,
    unsigned char canvas_b, unsigned char canvas_a,
    unsigned char* framebuffer,
    int fb_w, int fb_h
);
```

### C++ Tests to Write

#### `test_polygon_spans.cpp`

Compare rasterization output for simple and complex polygons.

```
Test cases (in pixel space, no transform needed):
  1. Unit square (4 vertices) — baseline sanity
  2. Triangle with sub-pixel vertices — tests edge clipping
  3. Star polygon (10 vertices) matching starfield_small params
  4. Round-rect outline polygon matching border_roundrect_thin params
  5. Bezier approximation polygon matching bezier_stroked params

For each:
  1. Call C++ PaintPolygon, instrument ScanlineTool::PaintScanline to capture (y, x, w, opacityBeg, opacity, opacityEnd)
  2. Call rust_rasterize_polygon with same vertices
  3. Compare scanline-by-scanline: y values, span x_start/x_end, opacity values
  4. Report: MATCH or DIVERGE with exact values per scanline
```

**Instrumenting C++ PaintScanline:** The C++ PaintPolygon calls `sct.PaintScanline(sx0, sy, w, alpha_beg, alpha_mid, alpha_end)` in the inner loop. Add a capture callback or printf wrapper to record these calls.

#### `test_polygon_e2e.cpp`

Full pipeline comparison.

```
For each of {star_5pt, roundrect_outline, bezier_curve}:
  1. Create identical framebuffers (50x50 or 100x100)
  2. Paint polygon via C++ emPainter::PaintPolygon
  3. Paint polygon via rust_paint_polygon
  4. Compare pixel-by-pixel
```

## Layer 10: Linear Gradient Integer Walk

**Covers:** eagle_logo (after test rewrite), potentially multi_compose and other ±1 failures.

### Root Question

Does Rust's f64 per-pixel gradient projection produce the same interpolation buffer as C++'s 40-bit fixed-point integer walk? The Rust port uses `(t.clamp(0.0, 1.0) * 255.0 + 0.5) as i32` while C++ uses `(x*TDX + y*TDY - TX) >> 24` with sign-extension clamping.

### FFI Functions to Export

#### 10a: `rust_interpolate_linear_gradient`

Fill an interpolation buffer matching C++ InterpolateLinearGradient.

```c
struct CGradientParams {
    double x1, y1;  // Gradient start (pixel space)
    double x2, y2;  // Gradient end (pixel space)
};

int rust_interpolate_linear_gradient(
    const CGradientParams* params,
    int scanline_x, int scanline_y, int width,
    unsigned char* out_buffer  // width bytes, gradient values 0-255
);
```

**Implementation:** For each pixel in [scanline_x, scanline_x+width), compute gradient parameter using the Rust `sample_linear_gradient` logic, store the 0-255 interpolation value.

#### 10b: `rust_paint_linear_gradient`

Full gradient paint on a framebuffer.

```c
int rust_paint_linear_gradient(
    double rect_x, double rect_y, double rect_w, double rect_h,
    double grad_x1, double grad_y1,
    double grad_x2, double grad_y2,
    unsigned char c1_r, unsigned char c1_g, unsigned char c1_b, unsigned char c1_a,
    unsigned char c2_r, unsigned char c2_g, unsigned char c2_b, unsigned char c2_a,
    unsigned char canvas_r, unsigned char canvas_g, unsigned char canvas_b, unsigned char canvas_a,
    double scale_x, double scale_y,
    double origin_x, double origin_y,
    unsigned char* framebuffer,
    int fb_w, int fb_h
);
```

### C++ Tests to Write

#### `test_gradient_interp.cpp`

Compare interpolation buffers across representative scanlines.

```
Test cases:
  1. Horizontal gradient (0,0)→(100,0), scanlines y=0..99, width=100
  2. Diagonal gradient (0,0)→(100,100), same scanlines
  3. Eagle logo gradient params (extracted from gen_golden.cpp:4630)

For each scanline:
  1. Call C++ InterpolateLinearGradient, capture buffer
  2. Call rust_interpolate_linear_gradient with same params
  3. Compare byte-by-byte
  4. Report: MATCH, or per-byte diff histogram
```

#### `test_gradient_e2e.cpp`

Full gradient paint comparison for the eagle_logo case.

## Layer 0: eagle_logo Test Rewrite

**Separate from harness extension.** The eagle_logo test has a structural mismatch: Rust delegates to `panel.Paint()` while C++ manually reimplements gradient + eagle polygons. This test must be rewritten to either:

1. **Match C++ generator structure:** Manually call `paint_linear_gradient` + `PaintPolygon` for the eagle shape, matching `gen_eagle_logo()` in `gen_golden.cpp:4630`.
2. **Rewrite C++ generator:** Make the C++ generator use `emMainContentPanel::Paint()` too.

Option 1 is simpler and doesn't require rebuilding the C++ generator.

## Implementation Order

### Phase 1: Layer 8 (PaintBorderImage) — Highest Impact

19+ of 36 failures. Implement `rust_border_image_boundaries` + `test_border_boundaries.cpp` first. If boundaries diverge, we have root cause for the majority of failures without any pixel rendering.

**Deliverables:**
- `rust_border_image_boundaries` in harness/src/lib.rs
- `rust_paint_border_image` in harness/src/lib.rs
- `test_border_boundaries.cpp` comparing 4+ widget parameter sets
- `test_border_e2e.cpp` for full pipeline
- Visibility changes: make boundary computation logic `pub` in emPainter.rs

**Success criterion:** For each of the 19 Group A/B/D tests, we can mechanically state either "boundaries diverge at rect N by X pixels" or "boundaries match, divergence is in paint_image_rect."

### Phase 2: Layer 9 (Polygon Rasterizer) — Second Priority

6+ failures. Implement `rust_rasterize_polygon` + `test_polygon_spans.cpp`.

**Deliverables:**
- `rust_rasterize_polygon` in harness/src/lib.rs
- `rust_paint_polygon` in harness/src/lib.rs
- `test_polygon_spans.cpp` comparing 5+ polygon shapes
- `test_polygon_e2e.cpp` for full pipeline
- Visibility changes: make `rasterize()` and `Span` `pub` in emPainterScanline.rs

**Success criterion:** For each polygon failure, we can state either "span at scanline Y diverges: Rust opacity=X, C++ opacity=Y" or "spans match, divergence is in blitting."

### Phase 3: Layer 10 (Gradient) + Test Rewrite — Lowest Priority

1-3 failures. Implement `rust_interpolate_linear_gradient` + `test_gradient_interp.cpp`. Rewrite eagle_logo test.

**Deliverables:**
- `rust_interpolate_linear_gradient` in harness/src/lib.rs
- `rust_paint_linear_gradient` in harness/src/lib.rs
- `test_gradient_interp.cpp`
- Rewritten eagle_logo golden test

**Success criterion:** Gradient interpolation buffer comparison shows exact match or quantified divergence pattern.

## Phase Gating

Each phase is gated on the prior phase's diagnostic output:

- **Phase 1 gate:** All 19 Group A/B/D tests have a mechanical root cause classification (boundary divergence OR paint_image_rect divergence). No proceeding to Phase 2 until this is done.
- **Phase 2 gate:** All 6 polygon tests have mechanical root cause (span divergence OR blitting divergence).
- **Phase 3 gate:** Gradient interpolation buffer compared; eagle_logo test rewritten and compared.

## Visibility Changes Required

The existing harness needs 7 `pub(crate)` → `pub` changes (documented in harness-prototype branch). New layers will need additional visibility changes:

| File | Item | Current | Needed |
|------|------|---------|--------|
| emPainter.rs | `RoundX`, `RoundY` | private | `pub` (or extract boundary logic) |
| emPainter.rs | Border image boundary computation | inline in PaintBorderImage | Extract to `pub` helper |
| emPainterScanline.rs | `rasterize` | `pub(crate)` | `pub` |
| emPainterScanline.rs | `Span` | `pub(crate)` | `pub` |
| emPainterInterpolation.rs | `sample_linear_gradient` | `pub(crate)` | `pub` |

## Build Integration

The harness builds as a cdylib (`libem_harness.so`). C++ test binaries link against it plus libemCore. The existing Makefile/build scripts in `.harness/` handle this. New tests follow the same pattern:

```bash
# Build Rust harness
cd harness && cargo build --release

# Build C++ test
g++ -o test_border_boundaries test_border_boundaries.cpp \
    -I../../include -L../../lib -lemCore \
    -L../harness/target/release -lem_harness \
    -Wl,-rpath,'$ORIGIN/../harness/target/release'

# Run
./test_border_boundaries
```

## What This Spec Does NOT Cover

- **Fixing the divergences.** This spec is diagnostic only. Fixes come after root causes are mechanically identified.
- **Hypotheses about what's wrong.** We do not speculate. The harness tells us.
- **Code-reading analysis.** The C++ and Rust implementations were read only to identify FFI boundaries, not to form theories.
