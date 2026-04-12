# Phase C2: Remaining Golden Test Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all 7 remaining golden test failures, reaching 242/243 pass (1 ignored).

**Architecture:** Delete-and-rewrite from C++ for divergent code. C++ is source of truth. Read Rust only for replacement boundaries. Gate each commit on zero regressions (all 243 tests).

**Tech Stack:** Rust (emcore crate), C++ reference at `~/git/eaglemode-0.96.4/`

**Approach constraints (steelmanned):**
1. C++ is source of truth — read C++ exhaustively for every function that produces failing output
2. Read Rust only for boundaries — function signatures, callers, integration points
3. Delete and rewrite from C++, porting ALL code paths in each touched function
4. Calibrate scope to divergence boundary — don't over-scope or under-scope
5. Gate each commit on zero regressions — run ALL golden tests after each fix
6. Subagent tasks are self-contained — include exact C++ code to port and verification commands
7. Anything unported in a touched function gets ported — but don't hunt outside replacement boundary

---

### Task 1: testpanel_root — Add Missing Paint Primitives

**Files:**
- Modify: `crates/eaglemode/tests/golden/test_panel.rs:514-897` (paint_primitives function)

**Context:** C++ `~/git/eaglemode-0.96.4/src/emTest/emTestPanel.cpp:276-479` has many paint calls that are missing from the Rust test harness. The Rust rendering code is correct; the test is under-specified. The C++ uses bounding-rect convention `(x, y, w, h)` for ellipses; the Rust API uses center-radius `(cx, cy, rx, ry)` — convert accordingly: `cx = x + w/2`, `cy = y + h/2`, `rx = w/2`, `ry = h/2`.

**Missing calls to add (C++ line → what's missing):**

**Ellipse Sectors (C++ lines 281-282):**
```
PaintEllipseSector(0.13,0.80,0.005,0.01,245,50,0xFFFFFFFF,BgColor)
PaintEllipseSector(0.14,0.80,0.01,0.01,245,-50,0xFFFFFFFF,BgColor)
```
→ Rust (center-radius): `PaintEllipseSector(0.1325, 0.805, 0.0025, 0.005, 245.0, 50.0, WHITE, bg)` and `PaintEllipseSector(0.145, 0.805, 0.005, 0.005, 245.0, -50.0, WHITE, bg)`

**Rect Outlines (C++ lines 285, 287):**
```
PaintRectOutline(0.07,0.82,0.02,0.01,0.001,emDashedStroke(0xFFFFFFFF),BgColor)
PaintRectOutline(0.13,0.82,0.01,0.01,0.011,0xFFFFFFFF,BgColor)
```
The dashed stroke: create an `emStroke` with `pattern: StrokePattern::Dashed`. The thick one (0.011 > rect h=0.01) is a degenerate case — just pass the params through.

**Round Rects (C++ lines 292-293):**
```
PaintRoundRect(0.13,0.84,0.01,0.01,0.001,0.011,0xFFFFFFFF,BgColor)
PaintRoundRect(0.15,0.84,0.01,0.01,0.000,0.00,0xFFFFFFFF,BgColor)
```
Note: Rust PaintRoundRect signature is `(x, y, w, h, radius, color, canvas_color)` — C++ has separate rx, ry. Check Rust API and pass accordingly.

**Ellipse Outline dotted (C++ line 297):**
```
PaintEllipseOutline(0.09,0.86,0.005,0.01,0.00025,emRoundedDottedStroke(0xFFFFFFFF),BgColor)
```
→ Rust center-radius with dotted rounded stroke.

**Ellipse Arcs (C++ lines 298, 300-302):**
```
PaintEllipseArc(0.10,0.86,0.01,0.01,90,225,0.001,0xFFFFFFFF,emStrokeEnd(),emStrokeEnd(),BgColor)
PaintEllipseArc(0.13,0.86,0.005,0.01,245,50,0.001,0xFFFFFFFF,emStrokeEnd(),emStrokeEnd(),BgColor)
PaintEllipseArc(0.14,0.86,0.01,0.01,245,-50,0.001,0xFFFFFFFF,emStrokeEnd(),emStrokeEnd(),BgColor)
PaintEllipseArc(0.15,0.86,0.01,0.01,0,-145,0.0001,emRoundedStroke(0xFFFFFFFF),emStrokeEnd::CAP,emStrokeEnd::LINE_ARROW,BgColor)
```

**Ellipse Sector Outline (C++ line 299):**
```
PaintEllipseSectorOutline(0.11,0.86,0.02,0.01,45,-320,0.0001,0xFFFFFFFF,BgColor)
```

**Round Rect Outlines (C++ lines 306-309):**
```
PaintRoundRectOutline(0.10,0.88,0.01,0.01,0.003,0.002,0.003,0xFFFFFFFF,BgColor)
PaintRoundRectOutline(0.12,0.88,0.01,0.01,0.001,0.011,0.0001,0xFFFFFFFF,BgColor)
PaintRoundRectOutline(0.135,0.88,0.01,0.01,0.001,0.001,0.00002,emDashDottedStroke(0xFFFFFFFF),BgColor)
PaintRoundRectOutline(0.15,0.88,0.01,0.01,-0.0004,-0.0004,0.001,0xFFFFFFFF,BgColor)
```

**Radial gradient ellipse (C++ lines 425-428) — WRONG in Rust:**
```cpp
painter.PaintEllipse(0.23,0.94,0.02,0.01,
    emRadialGradientTexture(0.23,0.94,0.02,0.01,0,0x00cc88FF));
```
Current Rust (line 878-885) paints a flat color ellipse with wrong position. Replace with radial gradient call matching C++.

**ImageColored texture rect (C++ lines 441-451) — MISSING:**
```cpp
painter.PaintRect(0.2625,0.942,0.02,0.01,
    emImageColoredTexture(1.0005,0.942,0.001,
        0.001*TestImage.GetHeight()/TestImage.GetWidth(),
        TestImage, 0x00FFFFFF, 0xFF0000FF));
```

**Image texture with extension modes (C++ lines 453-478) — MISSING:**
```cpp
// EXTEND_TILED
painter.PaintRect(0.275,0.907,0.002,0.002,
    emImageTexture(0.2755,0.9075,0.001,0.001,
        TestImage,50,10,110,110,255,emTexture::EXTEND_TILED));
// EXTEND_EDGE
painter.PaintRect(0.275,0.910,0.002,0.002,
    emImageTexture(0.2755,0.9105,0.001,0.001,
        TestImage,50,10,110,110,255,emTexture::EXTEND_EDGE));
// EXTEND_ZERO
painter.PaintRect(0.275,0.913,0.002,0.002,
    emImageTexture(0.2755,0.9135,0.001,0.001,
        TestImage,50,10,110,110,255,emTexture::EXTEND_ZERO));
```

- [ ] **Step 1: Read Rust API signatures for all paint methods being called**

Read the method signatures in `emPainter.rs` for: `PaintEllipseSector`, `PaintEllipseArc`, `PaintEllipseSectorOutline`, `PaintRoundRect`, `PaintRoundRectOutline`, `PaintRectOutline`, `PaintEllipseOutline`, `PaintImageColored`, `paint_image_scaled`. Understand parameter order and types. Also read `emStroke.rs` for stroke pattern construction (Dashed, Dotted, DashDotted, RoundedDotted, RoundedDashed).

- [ ] **Step 2: Add missing PaintEllipseSector calls after line 652**

Add the two missing sectors matching C++ lines 281-282 (angles 245,50 and 245,-50). Convert from bounding-rect to center-radius.

- [ ] **Step 3: Add missing PaintRectOutline calls**

After the existing PaintRectOutline block (around line 670), add:
- Dashed rect outline (C++ line 285): `emStroke` with `pattern: StrokePattern::Dashed`
- Thick rect outline (C++ line 287): thickness 0.011

- [ ] **Step 4: Add missing PaintRoundRect calls after line 676**

Add C++ lines 292-293:
- `PaintRoundRect(0.13, 0.84, 0.01, 0.01, ...)` with radius params matching C++ `(0.001, 0.011)`
- `PaintRoundRect(0.15, 0.84, 0.01, 0.01, ...)` with zero radii

- [ ] **Step 5: Add missing PaintEllipseOutline (dotted)**

After existing ellipse outlines (~line 694), add C++ line 297: dotted rounded stroke.

- [ ] **Step 6: Add all 5 PaintEllipseArc calls**

After the ellipse outlines section, add C++ lines 298, 300-302. Convert bounding-rect to center-radius. Handle stroke end types (CAP, LINE_ARROW).

- [ ] **Step 7: Add PaintEllipseSectorOutline**

Add C++ line 299 between the arc calls.

- [ ] **Step 8: Add 4 missing PaintRoundRectOutline calls**

After existing round rect outlines (~line 713), add C++ lines 306-309. Include the DashDotted stroke (line 308) and negative-radii case (line 309).

- [ ] **Step 9: Fix radial gradient ellipse**

Replace Rust lines 878-885 (flat color PaintEllipse) with a radial gradient call matching C++ lines 425-428. Use `paint_radial_gradient` or equivalent ellipse-with-radial-gradient API.

- [ ] **Step 10: Add ImageColored texture rect**

After the image scaled section (~line 896), add C++ lines 441-451: `PaintImageColored` or `PaintRect` with ImageColoredTexture.

- [ ] **Step 11: Add 3 image texture extension mode rects**

Add C++ lines 453-478: three `PaintRect` calls with `emImageTexture` using sub-rect `(50, 10, 110, 110)` and EXTEND_TILED, EXTEND_EDGE, EXTEND_ZERO. Use `paint_image_scaled` with appropriate extension mode parameter and sub-region support.

- [ ] **Step 12: Run testpanel_root test**

```bash
cargo test --test golden testpanel_root -- --test-threads=1
```
Expected: PASS (0 pixels failing) or significantly reduced from 4020.

- [ ] **Step 13: Run ALL golden tests, verify zero regressions**

```bash
cargo test --test golden -- --test-threads=1
```
Expected: no new failures beyond the existing 7.

- [ ] **Step 14: Commit**

```bash
git add crates/eaglemode/tests/golden/test_panel.rs
git commit -m "fix(C2-1): add missing paint primitives to test_panel paint_primitives()"
```

---

### Task 2: testpanel_expanded — Cascade Check

**Files:**
- Possibly: `crates/eaglemode/tests/golden/test_panel.rs`

**Context:** testpanel_expanded (19,954px) is partly cascading from testpanel_root's missing primitives. After Task 1, re-run to see residual. If pixels remain, investigate canvas_color propagation and glyph rendering differences.

- [ ] **Step 1: Run testpanel_expanded test**

```bash
cargo test --test golden testpanel_expanded -- --test-threads=1
```
Record new fail count. If 0, skip remaining steps.

- [ ] **Step 2: If pixels remain, dump and diff draw ops**

```bash
DUMP_DRAW_OPS=1 cargo test --test golden testpanel_expanded -- --test-threads=1
python3 scripts/diff_draw_ops.py testpanel_expanded
```

Analyze the diff. Look for canvas_color differences (00000000 vs real colors) and glyph rendering path differences (PaintImageColored vs PaintRect).

- [ ] **Step 3: Fix identified issues**

Apply fixes based on DrawOp diff analysis. Common causes:
- Sub-panel canvas_color not propagated (check SetCanvasColor calls)
- Glyph rendering using wrong method

- [ ] **Step 4: Run ALL golden tests, verify zero regressions**

```bash
cargo test --test golden -- --test-threads=1
```

- [ ] **Step 5: Commit if changes made**

```bash
git commit -m "fix(C2-2): fix testpanel_expanded cascading divergences"
```

---

### Task 3: splitter_v_extreme_tall — Match u32 Accumulator Wrapping

**Files:**
- Modify: `crates/emcore/src/emPainterInterpolation.rs:468-527` (y_accumulate_4ch)
- Modify: `crates/emcore/src/emPainterInterpolation.rs:532-570` (y_accumulate_3ch — same issue)

**Context:** C++ `emPainter_ScTlIntImg.cpp` uses `emUInt32` (u32) for ALL accumulator variables: `cy_r`, `cy_g`, `cy_b`, `cy_a`, `ctmp_r`, etc. The Rust `y_accumulate_4ch` and `y_accumulate_3ch` use `u64` everywhere. At extreme downscale ratios (splitter panel 1.6px tall), the u32 accumulators overflow and wrap, producing different results. Per Port Fidelity rules: "Reproduce C++ integer formulas exactly."

The C++ macro chain:
- `DEFINE_COLOR(C)` → `emUInt32 Cr, Cg, Cb, Ca;` (line 219)
- `READ_PREMUL_MUL_COLOR(C,PTR,S)` → `Ca=PTR[3]*S; Cr=PTR[0]*Ca; ...` (lines 268-273) — all u32 multiplies
- `ADD_MUL_COLOR(C1,C2,S)` → `C1r+=C2r*S; ...` (lines 372-377) — u32 addition of u32*u32 product
- `ADD_READ_PREMUL_COLOR(C,PTR)` → straight add (lines 382-385)
- `FINPREMUL_SHR_COLOR(C,8)` → `Cr=(Cr+0x7F7F)/0xFF00; Ca=(Ca+0x7F)>>8;` (lines 431-436) — truncated to u32 before final division

The fix: Change all accumulator types in `y_accumulate_4ch` and `y_accumulate_3ch` from u64 to u32, and use `wrapping_mul` / `wrapping_add` where C++ arithmetic would naturally wrap.

- [ ] **Step 1: Read the Rust y_accumulate_4ch and y_accumulate_3ch functions**

Read `crates/emcore/src/emPainterInterpolation.rs:468-580`. Identify every u64 that should be u32.

- [ ] **Step 2: Read C++ macro definitions for the 4-channel path**

Read `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlIntImg.cpp:211-436`. Note that `DEFINE_COLOR` uses `emUInt32` for all channels. `READ_PREMUL_MUL_COLOR` multiplies byte * u32 → u32. `ADD_MUL_COLOR` does u32 += u32 * u32.

- [ ] **Step 3: Rewrite y_accumulate_4ch with u32 accumulators**

Delete the function body and rewrite. Key changes:
- All `u64` → `u32` for accumulators (ca, cr, cg, cb, ta, tr, tg, tb)
- `as u64` casts → `as u32` 
- Multiplication: `p[3] as u32 * yw.oy1 as u32` (wraps naturally in Rust u32 since both are small enough for the non-extreme case, but WILL wrap for extreme cases matching C++)
- Wait — actually `u32 * u32` in Rust panics on overflow in debug mode. Need `.wrapping_mul()` for the products that can overflow.
- In C++, `emUInt32 * emUInt32` silently wraps. In Rust, use `.wrapping_mul()` and `.wrapping_add()`.
- Final division: `(cr.wrapping_add(0x7F7F)) / 0xFF00` — but this is `u32 / u32`.

- [ ] **Step 4: Rewrite y_accumulate_3ch with u32 accumulators**

Same treatment. 3-channel has no alpha premultiplication but same accumulator types.

- [ ] **Step 5: Update return types**

The functions currently return `(u64, u64, u64, u64)`. Change to `(u32, u32, u32, u32)`. Update callers in the same file.

- [ ] **Step 6: Run splitter_v_extreme_tall test**

```bash
cargo test --test golden golden_widget_splitter_v_extreme_tall -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 7: Run ALL golden tests, verify zero regressions**

```bash
cargo test --test golden -- --test-threads=1
```

- [ ] **Step 8: Commit**

```bash
git add crates/emcore/src/emPainterInterpolation.rs
git commit -m "fix(C2-3): match C++ u32 wrapping in area sampling Y accumulators"
```

---

### Task 4: composition_tktest_1x/2x — Area Sampling Transform Rewrite

**Files:**
- Modify: `crates/emcore/src/emPainter.rs:6076-6120` (area_sample_transform_24)
- Modify: `crates/emcore/src/emPainterInterpolation.rs` (area sample interpolation loop)

**Context:** Draw op params match bit-for-bit but pixel output differs. The divergence is in the area sampling pipeline. C++ `ScanlineTool::Init` (emPainter_ScTl.cpp:285-343) computes the transform, and `InterpolateImageAreaSampled` (emPainter_ScTlIntImg.cpp:598-828) consumes it. The Rust equivalents may compute slightly different TDX/TDY/TX/TY values due to float → int casting differences, or the pre-reduction stride logic may differ.

The C++ Init path for area-sampled downscaling (lines 296-343):
```cpp
// Pre-reduction: reduce source image by integer stride
int n = (TDX/downscaleQuality+0xFFFFFF)>>24;
if (n>1) {
    int t=ImgW;
    ImgW=(t+n-1)/n;
    t-=(ImgW-1)*n+1;
    ImgMap+=ImgDX*(t>>1);
    ImgDX*=n;
    ImgSX=ImgW*ImgDX;
    tdx=ImgW*((emInt64)1<<24)/tw;
    TDX=(emInt64)tdx;
}
// Same for Y
TX=(emInt64)(tx*tdx);  // NOTE: tx is pixel-space, no -0.5 offset
TY=(emInt64)(ty*tdy);
ODX = TDX<=0x200 ? 0x7fffffff : (((emInt64)1<<40)-1)/TDX+1;
ODY = TDY<=0x200 ? 0x7fffffff : (((emInt64)1<<40)-1)/TDY+1;
```

The Rust `area_sample_transform_24` (emPainter.rs:6076-6120) currently does NOT do pre-reduction — stride_x/stride_y are always 1. This is likely the primary divergence. At 100-200x downscale, C++ reduces the source image first, then area-samples the reduced image. Rust area-samples the full-resolution source.

- [ ] **Step 1: Read C++ ScanlineTool::Init pre-reduction logic**

Read `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTl.cpp:285-343`. Note the pre-reduction stride computation for both X and Y, and how TDX/TDY are recomputed after reduction.

- [ ] **Step 2: Read Rust area_sample_transform_24 and its callers**

Read `crates/emcore/src/emPainter.rs:6076-6120` and find all callers (grep for `area_sample_transform_24`). Understand how the transform feeds into the interpolation loop.

- [ ] **Step 3: Read C++ downscaleQuality config**

In C++, `downscaleQuality` comes from texture or config. Check what the Rust equivalent is and what value it uses for the golden test images.

- [ ] **Step 4: Implement pre-reduction in area_sample_transform_24**

Delete the function body and rewrite from C++ Init lines 296-343. Key additions:
- Compute pre-reduction stride N for X: `n = (tdx / downscale_quality + 0xFFFFFF) >> 24`
- If n > 1: adjust img_w, off_x, stride_x, recompute tdx
- Same for Y
- Set tx, ty, odx, ody matching C++ exactly

- [ ] **Step 5: Update interpolation loop to use stride**

The interpolation functions in `emPainterInterpolation.rs` must respect `stride_x` and `stride_y` when reading pixels. If they currently always step by 1 pixel, update them to step by stride.

- [ ] **Step 6: Run composition_tktest_1x and tktest_2x**

```bash
cargo test --test golden composition_tktest_1x -- --test-threads=1
cargo test --test golden composition_tktest_2x -- --test-threads=1
```
Expected: PASS (or significantly reduced from 13396 + 94).

- [ ] **Step 7: If pixels remain, build FFI harness**

If rewrite alone doesn't fix it, build an FFI harness: call both C++ and Rust area sampling with identical parameters from a failing border sub-image, compare pixel output. This isolates the exact line of divergence.

- [ ] **Step 8: Run ALL golden tests, verify zero regressions**

```bash
cargo test --test golden -- --test-threads=1
```

- [ ] **Step 9: Commit**

```bash
git commit -m "fix(C2-4): add pre-reduction stride to area sampling, matching C++ ScanlineTool::Init"
```

---

### Task 5: file_selection_box — Rewrite Layout + Cycle from C++

**Files:**
- Modify: `crates/emcore/src/emFileSelectionBox.rs` (LayoutChildren, Cycle, reload_listing)
- Modify: `crates/eaglemode/tests/golden/widget.rs` (FSB test settle function)

**Context:** The Rust FSB renders 14,123 pixels differently. The test `settle()` doesn't call `tree.run_panel_cycles()`, so Cycle never fires, reload_listing never runs, and the file list stays empty. Adding run_panel_cycles() caused a 110K pixel regression because Rust child layouts differ from C++.

The C++ FSB architecture:
- `Cycle()` (emFileSelectionBox.cpp:385-465): Handles signals, calls ReloadListing when ListingInvalid, handles selection changes
- `LayoutChildren()` (emFileSelectionBox.cpp:569-599): 3-zone layout (top: dir+checkbox, middle: files list, bottom: name+filter)
- `ReloadListing()` (emFileSelectionBox.cpp:609-700): Reads directory, sorts, filters hidden files

The Rust version (emFileSelectionBox.rs) has the same structure but the test doesn't drive cycles. The fix requires:
1. Making test settle() call cycles properly
2. Ensuring the cycle/layout code matches C++ exactly

- [ ] **Step 1: Read C++ Cycle in full**

Read `~/git/eaglemode-0.96.4/src/emCore/emFileSelectionBox.cpp:385-465`.

- [ ] **Step 2: Read C++ LayoutChildren in full**

Read `~/git/eaglemode-0.96.4/src/emCore/emFileSelectionBox.cpp:569-599`.

- [ ] **Step 3: Read C++ ReloadListing in full**

Read `~/git/eaglemode-0.96.4/src/emCore/emFileSelectionBox.cpp:609-700`.

- [ ] **Step 4: Read Rust equivalents — boundaries only**

Read Rust `emFileSelectionBox.rs` for function signatures and integration points: what fields exist, how children are created, how events flow.

- [ ] **Step 5: Rewrite Rust Cycle to match C++ exactly**

Delete Rust `Cycle` body, rewrite matching C++ `emFileSelectionBox::Cycle()` line by line. Match signal checking order, ListingInvalid logic, selection syncing.

- [ ] **Step 6: Rewrite Rust LayoutChildren to match C++ exactly**

Delete body, rewrite from C++ `LayoutChildren()`. The current Rust version looks close but verify every parameter.

- [ ] **Step 7: Verify reload_listing matches C++ ReloadListing**

Compare Rust `reload_listing()` against C++ `ReloadListing()`. The error-handling fix was already applied (fall-through to ".." insertion). Verify sort order, hidden file filtering, and ".." insertion logic.

- [ ] **Step 8: Update test settle() to drive cycles**

In `crates/eaglemode/tests/golden/widget.rs`, find the FSB test's settle function. Add cycle-driving that matches C++ `sched.Run()` — typically 30 ticks of running the scheduler. If `tree.run_panel_cycles()` exists, call it. If this causes regressions, investigate which child panel layout differs.

- [ ] **Step 9: Run file_selection_box test**

```bash
cargo test --test golden widget_file_selection_box -- --test-threads=1
```

- [ ] **Step 10: Run ALL golden tests, verify zero regressions**

```bash
cargo test --test golden -- --test-threads=1
```

- [ ] **Step 11: Commit**

```bash
git commit -m "fix(C2-5): rewrite emFileSelectionBox Cycle/Layout from C++, drive cycles in test"
```

---

### Task 6: border_roundrect_thin — Polygon Coverage Edge Case

**Files:**
- Modify: `crates/emcore/src/emPainter.rs:5426-5500` (fill_polygon_aa + blit_span)
- Possibly: `crates/emcore/src/emPainterScanline.rs` (rasterize function)

**Context:** 2 pixels at positions (0,299) and (799,299). Panel tallness=0.002 (panel ~1.6px tall). Rust renders gray(128,128,128) while C++ renders border-blend colors. The PaintRoundRect params match bit-for-bit between C++ and Rust.

The C++ polygon fill (emPainter.cpp:460-800) uses a scanline-entry approach:
- Edge processing: clips edges to min/max bounds, computes per-pixel area coverage using A0/A1/A2 accumulation
- Scanline sweep: walks entries left-to-right, accumulates coverage, calls PaintScanline with opacity values
- Key detail: the scanline accumulation uses floating-point `a0, a1, a2` with `0.5+a0` rounding for alpha

The Rust `fill_polygon_aa` (emPainter.rs:5426-5500) uses a different approach: `emPainterScanline::rasterize()` produces spans, then `blit_span` writes them. The rasterize function must match the C++ scanline-entry algorithm for edge cases at extreme thin panels.

**WARNING: HIGH REGRESSION RISK.** 235 passing tests use polygon fill. Any change here must be verified against ALL tests.

- [ ] **Step 1: Read C++ PaintPolygon scanline algorithm**

Read `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp:460-800` in full. Understand:
- Edge clipping and scan entry creation (lines 579-711)
- Scanline sweep and opacity accumulation (lines 713-792)
- The `a0, a1, a2` coverage model

- [ ] **Step 2: Read Rust rasterize implementation**

Read `crates/emcore/src/emPainterScanline.rs`. Understand how it produces spans from polygon vertices. Identify where it handles sub-pixel edges at Y pixel boundaries.

- [ ] **Step 3: Create a targeted reproduction**

Before changing anything, write a minimal test that exercises the exact failing geometry: a very thin (h=0.002 in logical coords) rounded rect at 800x600 resolution. Verify it produces the 2-pixel failure.

- [ ] **Step 4: Compare C++ vs Rust edge handling for the failing scanline**

For y=299 (the failing row), trace through both C++ and Rust scanline algorithms with the exact polygon vertices. Identify where coverage differs. The issue is likely in how sub-pixel Y boundaries are handled when a polygon spans less than 1 pixel vertically.

- [ ] **Step 5: Fix the identified divergence**

Apply the minimal fix to match C++ behavior. Do NOT rewrite the entire polygon fill unless the architectural difference is the root cause.

- [ ] **Step 6: Run border_roundrect_thin test**

```bash
cargo test --test golden golden_widget_border_roundrect_thin -- --test-threads=1
```
Expected: PASS.

- [ ] **Step 7: Run ALL golden tests, verify zero regressions**

```bash
cargo test --test golden -- --test-threads=1
```
This step is CRITICAL for this task due to high regression risk.

- [ ] **Step 8: Commit**

```bash
git commit -m "fix(C2-6): fix polygon coverage at extreme thin panel boundaries"
```

---

### Task 7: Final Verification and Cleanup

**Files:** None (verification only)

- [ ] **Step 1: Run full golden test suite**

```bash
cargo test --test golden -- --test-threads=1
```
Expected: 242 passed, 0 failed, 1 ignored.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

- [ ] **Step 3: Run full test suite**

```bash
cargo-nextest ntr
```

- [ ] **Step 4: Record final state**

If not all 242 pass, document which tests still fail and why. Update the divergence inventory memory.
