# Rendering Parity Harness — zuicchini vs emCore

Achieve 1:1 pixel parity between Rust `zuicchini` and C++ Eagle Mode
`emCore`. This document is the sole plan.

Built from six agentic patterns:
- **Pipeline-Aware DAG Ordering**: fixes follow the rendering pipeline's
  causal chain — upstream stages before downstream
- **Structural Comparison First**: code reading is the default diagnostic
  for a known-source port; runtime instrumentation is escalation
- **Taxonomy-Based Escalation**: type-representation, formula, and
  evaluation-order differences each have distinct diagnostic procedures
- **Per-Stage Validation**: architectural fixes are validated at their
  pipeline stage, not by downstream pixel counts
- **Rolling Baselines**: re-measure and re-diagnose after each accepted
  fix; do not carry stale observations forward
- **Spec-As-Test Feedback Loop**: automated measurement is the sole
  source of truth; the three-number dashboard is the metric

---

## Project Reference

**Implementations:**
- Rust (impl_A): `src/render/painter.rs`, `src/render/scanline.rs`,
  `src/render/interpolation.rs`, `src/widget/border.rs`,
  `src/widget/color_field.rs`, `src/widget/scalar_field.rs`
- C++ (impl_B): `/home/ar/.local/git/eaglemode-0.96.4/src/emCore/`
  `emPainter.cpp`, `emPainter_ScTlIntImg.cpp`, `emBorder.cpp`,
  `emColorField.cpp`, `emScalarField.cpp`, `emPanel.cpp`

**Tests:** `tests/golden_parity/widget.rs`

**Commands:**
```bash
# Run specific test
CARGO_TARGET_DIR=rust_target cargo-nextest ntr -E 'test(widget_colorfield)' --workspace

# Run full suite with divergence log
CARGO_TARGET_DIR=rust_target DIVERGENCE_LOG=$(pwd)/state/post_rXX.jsonl \
  cargo-nextest ntr --workspace --test-threads=1

# Generate diff images
CARGO_TARGET_DIR=rust_target DUMP_GOLDEN=1 cargo-nextest ntr \
  -E 'test(widget_colorfield)' --workspace --no-capture

# Clippy + full test suite
CARGO_TARGET_DIR=rust_target cargo clippy --workspace -- -D warnings && \
  CARGO_TARGET_DIR=rust_target cargo-nextest ntr --workspace
```

**Coordinate systems:**
- C++ paints in normalized panel space (w=1.0, h=tallness). ScaleX = ViewedWidth.
- Rust paints in pixel space (w=ViewedWidth, h=ViewedWidth*tallness). scale_x = 1.0.
- Both produce equivalent pixel-space values. Confirmed by P1-P38.

**Safety invariants (do not violate):**
- **X/Y asymmetry**: C++ uses `ViewedWidth/PixelTallness` for Y but
  `ViewedWidth` for X. Never assume symmetric scaling.
- **Framebuffer independence**: paint operations do not read from the
  framebuffer during rasterization (only at compositing/blending time).
  A clip change affects WHICH pixels are written, not HOW neighboring
  pixels are computed.
- **Widget correctness**: P1-P38 confirm all widget geometry and config.
  The remaining bugs are in `src/render/`. Do not audit widgets.

---

## Proven Ground Truth (P1-P38)

These propositions eliminate specific causes. Each is confirmed and immutable.

**All widget-level geometry, configuration, and layout is verified correct.**

### Layout & Geometry (P1-P6, P17-P20, P26-P33)

All layout coordinates, content rects, border insets, and child positions match
C++ to 6+ decimal places at runtime.

Key verified values for widget_colorfield (800x600):
- content_round_rect: (46.827, 125.854, 706.347, 427.319, r=40.773)
- swatch rect: (89.559, 168.586, 620.883, 341.856)
- IO overlay rect: (43.565, 122.592, 712.870, 433.843, ir=44.035)
- content_rect_unobscured (normalized): (0.1105, 0.2093, 0.7790, 0.4302)
- RasterLayout child: (0.5, y, 0.368, ch) -- right half of inset CRU
- Grid cell(1,0): (0.509, 0.331, 0.455, 0.091) -- column-major

### Widget Configuration (P7-P10, P21-P23, P35, P37)

Look propagation, editable flag, border types, scaling, color selection,
IsEnabled dimming, HowTo pill text/alpha, outline color -- all match C++.
CustomRect border is NOT dimmed when disabled in either implementation (P35).

### Rendering Pipeline (P11-P16, P25, P34, P36, P38)

Font atlas byte-identical (P11). Area-sampling source formulas match (P12;
caveat: output not compared). Color::lerp matches GetBlended (P13).
paint_rect_outlined vertex layout matches (P38).

**P34 (code difference confirmed):** C++ `emPanel.cpp:562-577` stores clip
edges as `double`. Rust `painter.rs:281-285` truncates to `i32` via
`px + pw = floor(ViewedX) + floor(ViewedWidth)`, which can be 1 less than
C++'s `floor(ViewedX + ViewedWidth)`.

**P36 (pixel evidence):** At (682,300) in widget_colorfield, Rust=(255,0,0),
C++=(145,5,11). Child OBT_RECT fill covers this pixel but Rust clip excludes
it. A `ceil()` fix was tested: -2 px colorfield, +521 px testpanel. The fix
must match C++'s float-based clip propagation architecture, not just change
the rounding direction.

---

## Baseline (R21, 2026-03-14, post ScalarField/TextField canvas propagation)

| Test | Pixels | Total | Pct | max_diff |
|------|--------|-------|-----|----------|
| widget_colorfield | 2,664 | 480,000 | 0.56% | 185 |
| colorfield_expanded | 11,986 | 640,000 | 1.87% | 158 |
| widget_scalarfield | 98 | 480,000 | 0.020% | 56 |
| listbox_expanded | 314 | 640,000 | 0.049% | 33 |
| testpanel_expanded | 59,460 | 1,000,000 | 5.95% | 255 |
| testpanel_root | 81,418 | 1,000,000 | 8.14% | 255 |

Rolling divergence log: `state/post_r21_widget_canvas.jsonl`

### R21 Three-Number Dashboard (ScalarField/TextField canvas propagation)

| # | Metric | Value |
|---|--------|-------|
| 1 | Fix description | ScalarField side bar rects and TextField selection/text now use painter canvas_color (set by border paint) instead of TRANSPARENT, matching C++ canvasColor propagation pattern |
| 2 | Total divergent pixels | 158,188 → 157,876 (-312 net) |
| 3 | Net regression | 0 (no test regressed) |

**Acceptance:** Fix accepted. After border.paint_border() sets canvas_color on the painter,
ScalarField captures it for its two side bar rects (C++ emScalarField.cpp:419-420 pass
canvasColor, then reset to 0). TextField captures it for selection highlight and text
rendering (C++ emTextField.cpp:990 and 1013-1026). Key improvements: widget_colorfield
-203 px, colorfield_expanded -103 px (ScalarField children in ColorField).

### R20 Three-Number Dashboard (TestPanel canvas + winding fix)

| # | Metric | Value |
|---|--------|-------|
| 1 | Fix description | Fixed TestPanel test: (a) changed holed polygon from even-odd to non-zero winding matching C++ PaintPolygon, (b) passed bg as canvas_color to ~25 paint calls where C++ passes BgColor (was TRANSPARENT), (c) set painter canvas_color state before paint_round_rect/paint_round_rect_outlined calls |
| 2 | Total divergent pixels | 159,346 → 158,188 (-1,158 net) |
| 3 | Net regression | 0 (no test regressed) |

**Acceptance:** Fix accepted. Test-only code change (no library impact). Two FORMULA
differences in the TestPanel golden test: (1) C++ uses non-zero winding for both holed
polygons, Rust incorrectly used even-odd for the first; (2) C++ passes BgColor as
canvasColor for AA edge blending on polygons, ellipses, rect outlines, round rects,
beziers, and polylines. Impact is proportional to shape edge perimeter (AA quality),
not interior area.

### R18 Three-Number Dashboard (per-call canvas_color refactor)

| # | Metric | Value |
|---|--------|-------|
| 1 | Fix description | Added explicit canvas_color: Color parameter to 26 paint functions, matching C++ emPainter per-call canvasColor API (default=0). Leaf functions save/set/restore; delegators pass through. |
| 2 | Total divergent pixels | 216,896 → 159,346 (-57,550 net) |
| 3 | Net regression | 0 (no test regressed) |

**Acceptance:** Fix accepted. Architectural alignment: C++ emPainter uses per-call
canvasColor with default=0 (TRANSPARENT). Rust was using persistent painter state,
causing every outline and fill operation to use the border's opaque canvas instead
of source-over blending. Key improvements: testpanel_expanded -29,760 px (-33%),
testpanel_root -27,289 px (-25%). Compiler-driven refactor ensured zero missed call sites.

### R17 Three-Number Dashboard (canvas_blend alpha + outline canvas fix)

| # | Metric | Value |
|---|--------|-------|
| 1 | Fix description | Two fixes: (1) canvas_blend no longer modifies destination alpha (C++ HAVE_CVC only changes RGB); (2) ColorField outline uses TRANSPARENT canvas matching C++ PaintRectOutline default canvasColor=0 |
| 2 | Total divergent pixels | 220,822 → 216,896 (-3,926 net) |
| 3 | Net regression | 0 (no test regressed) |

**Acceptance:** Fix accepted. Two root causes fixed:
1. `blend_pixel` canvas-color path was modifying destination alpha via `canvas_blend`
   on the alpha channel. C++ HAVE_CVC path only modifies RGB (hash tables hcR/hcG/hcB
   don't include alpha). Now alpha is unchanged in canvas-color compositing.
2. ColorField's swatch outline (`paint_rect_outlined`) was using the border's opaque
   canvas_color for compositing, but C++ PaintRectOutline defaults to canvasColor=0
   (TRANSPARENT), using source-over. The mismatch caused canvas_blend to clamp negative
   G,B terms to 0 instead of correctly blending. Fixed by setting canvas to TRANSPARENT
   before the outline paint.

Key improvements: widget_colorfield -1,910 px (-37%), colorfield_expanded -2,016 px (-14%).

### R16 Three-Number Dashboard (canvas_color propagation fix)

| # | Metric | Value |
|---|--------|-------|
| 1 | Fix description | Propagate canvas_color from parent borders to child panels during LayoutChildren, matching C++ `child->Layout(x,y,w,h,canvasColor)` |
| 2 | Total divergent pixels | 226,386 → 220,822 (-5,564 net) |
| 3 | Net regression | colorfield_expanded +2 px (within noise), max_diff improved 191→158 |

**Acceptance:** Fix accepted. Canvas color now propagated to child panels
during layout, matching C++ `emBorder::LayoutChildren()`. Key improvements:
listbox_expanded -3,184 px (-91%), testpanel_expanded -2,382 px.
Also fixed missing `set_canvas_color` in Filled/MarginFilled outer border paint.

**Analysis:** widget_colorfield unchanged because ScalarField children are NOT
painted as separate panels (not viewed at this zoom). The remaining 5,164 px
divergence is from the ColorField's own IO overlay border image compositing.

### R15 Three-Number Dashboard (TextField enabled fix)

| # | Metric | Value |
|---|--------|-------|
| 1 | Fix description | TextField::paint used `self.editable` as border `enabled` instead of panel `IsEnabled` |
| 2 | Total divergent pixels | 19,822 → 19,479 (-343 net, all from widget_colorfield) |
| 3 | Net regression | 0 (no test regressed) |

**Acceptance:** Fix accepted. TextField border dimming now uses panel's
`state.enabled` (matching C++ `IsEnabled()`), not `self.editable`.

### R14 Three-Number Dashboard (S1-clip fix)

| # | Metric | Value |
|---|--------|-------|
| 1 | Stage-1 match count | All clip values now computed in f64 (architectural match) |
| 2 | Total divergent pixels | 19,825 → 19,822 (-3 net) |
| 3 | Net regression | 0 |

**Acceptance:** Fix accepted. Clip storage is now f64 matching C++ `double ClipX1/ClipY1/ClipX2/ClipY2`.

Original R13 baseline: `state/post_r13_baseline.jsonl`

---

## The Rendering Pipeline DAG

The rendering pipeline has a strict causal chain. Each stage feeds the
next. A bug at stage N corrupts the inputs to stages N+1 through 6.

```
[Stage 1] Clip propagation (parent clip intersected with child bounds)
    |
[Stage 2] Transform setup (offset, scale per panel)
    |
[Stage 3] Geometry computation (rect edges, polygon vertices)
    |
[Stage 4] Rasterization (polygon fill, sub-pixel coverage, area sampling)
    |
[Stage 5] Blending (alpha compositing, canvas color interaction)
    |
[Stage 6] Compositing (back-to-front panel layering into framebuffer)
```

**Critical consequence:** A 1-pixel clip error at stage 1 does not produce
1 divergent pixel. It changes which pixels EVERY child panel renders.
D1's 1-pixel clip edge difference produces ~3,147 divergent pixels because
every child paint operation in that column diverges.

**Causal independence test:** Two divergence items A and B are causally
independent if and only if fixing A would not change pixel values at B's
locations, even transitively through the DAG. Items at different pipeline
stages are assumed DEPENDENT unless they are in completely different panel
subtrees or different test cases.

---

## Structural Diff Ledger

The structural diff ledger catalogs every known algorithmic difference
between Rust and C++ rendering code. It is organized by pipeline stage
and difference type. This replaces the pixel-based classifier.

### Difference Type Taxonomy

| Type | Description | Diagnostic method | Example |
|------|-------------|-------------------|---------|
| **TYPE_REPR** | Same formula, different storage type (f64 vs i32) | Structural comparison (code reading) | D1: clip stored as i32 vs double |
| **FORMULA** | Different algorithm or expression | Structural comparison + targeted pixel probe | Missing clamp, different constant |
| **EVAL_ORDER** | Identical source formulas, different FP evaluation order | Runtime instrumentation or strict-FP compilation | Compiler FMA contraction |

**Diagnostic escalation rule:** Start with structural comparison (read
both functions, identify differences). Escalate to runtime instrumentation
ONLY when:
1. Structural comparison shows semantic equivalence but pixels diverge
2. The difference is data-dependent (control flow varies per pixel)
3. A structural fix fails validation for non-obvious reasons

### Stage 1: Clip Propagation

```
id:       S1-clip
type:     TYPE_REPR
stage:    1 (clip)
status:   RESOLVED (R14)
rust:     painter.rs -- ClipRect{f64} with x1,y1,x2,y2. clip_rect() intersects
          in f64. Each paint method converts to i32 at point of use:
          left/top = floor, right/bottom = ceil.
cpp:      emPanel.cpp:1478-1495 -- stores ClipX1/ClipX2 as double.
          emPainter.cpp:344-359 -- truncates to fixed-point at paint time.
evidence: P34, P36 confirmed. Fix verified: -2 px colorfield, -1 px expanded,
          0 testpanel regression. scanline::rasterize() now takes ClipBounds{f64}.
```

### Stage 4: Rasterization / Interpolation

```
id:       S4-interp
type:     EVAL_ORDER (confirmed)
stage:    4 (rasterization)
status:   DIAGNOSED (R19 deep investigation)
rust:     painter.rs:paint_border_image, interpolation.rs (area-sampling path)
cpp:      emPainter.cpp:PaintBorderImage, emPainter_ScTlIntImg.cpp
evidence: Post-R18: 2,867 px at tol=3 in widget_colorfield. Key patterns:
          1. Formula diff (5,079 px at tol>0): y=170 (618px), y=508 (618px),
             y=283-395 (267px each, right half). Rust G=B=0 at ~4,009 pixels.
          2. Missing paint (85 px): (556-592, 383-389) diff=185, Rust shows
             border gray (192,195,201), C++ shows dark content (7,11,24).
root:     The IO overlay border image (IOField.tga 572x572 RGBA) has a
          sharp alpha=0 boundary at source row ~220. Area sampling at dest
          y=170 maps to reduced source rows 116-119 (actual 232-238), which
          are fully transparent (alpha=0). Rust correctly returns TRANSPARENT.
          BUT C++ produces non-transparent at the same dest pixel. The bg
          before the IO overlay is (207,0,0) in Rust (modified by swatch
          outline at ~3.4px thickness). C++ result (181,4,9) implies C++
          maps to slightly different source rows that cross the alpha=0
          boundary, sampling non-transparent pixels (alpha > 0 at y < 220).
          Root cause: floating-point differences in the section destination
          Y coordinate (the 9-slice top inset height) between C++ panel-space
          coordinates (multiplied by ScaleY=ViewedWidth/PixelTallness) and
          Rust pixel-space coordinates (scale=1.0). A sub-pixel shift in
          the section start Y shifts the source row mapping by ~5 rows
          (5.3:1 downscaling ratio), crossing the alpha=0 cliff at row 220.
fix:      Requires either (a) exact coordinate match for 9-slice section
          boundaries by reproducing C++ panel→pixel transform chain, or
          (b) restructuring to use C++-style coordinate space internally.
          Neither is simple — the coordinate spaces are architecturally
          different (Rust pixel-space vs C++ panel-space). This is the
          same root cause as U-expanded and testpanel divergence.
```

```
id:       S4-subpixel
type:     EVAL_ORDER (probable)
stage:    4 (rasterization)
status:   DIAGNOSED (R19)
rust:     scanline.rs -- polynomial coverage rasterizer
cpp:      emPainter.cpp:500-625 (edge setup), 637-716 (evaluation)
evidence: 98 px at tol=3 in widget_scalarfield. 91 px at y=516-517 (bottom
          edge of border element, h=0.916px). 7 px at corners (y=18,21,128,
          132,546,550,581). Periodic pattern at y=516: high-diff pixels
          (~40 diff) every ~11px (tick mark positions), alternating with
          low-diff (~1-4). Rust consistently brighter (lower coverage).
root:     Pixel analysis shows the polygon edge crosses pixel boundaries
          at sub-pixel positions that differ by <1px between Rust and C++.
          At (125,516): Rust=(221,223,227) shows partial polygon coverage,
          C++=(181,185,191) shows background — Rust rasterizes the polygon
          1 pixel further right than C++ at this scanline. Edge iteration
          order (reverse, matching C++) and polynomial formulas (a0, a1, a2,
          va=4096) are structurally identical. The difference is in
          floating-point accumulation of x_cur during scanline stepping,
          causing floor() to produce different sx values at polygon edges
          very close to pixel boundaries.
note:     Low priority (98 px, max_diff=56). Same EVAL_ORDER class as
          S4-interp — both stem from FP differences in coordinate mapping.
```

### Stage 3/4: Geometry / Edge AA

```
id:       S34-swatch-edge
type:     SUBSUMED by S4-interp
stage:    3 or 4
status:   SUBSUMED (R16)
evidence: Swatch edge pixels are part of the IO overlay border image
          compositing divergence diagnosed in S4-interp.
```

```
id:       S34-border-corner
type:     UNKNOWN (possibly EVAL_ORDER)
stage:    3 or 4
status:   UNDIAGNOSED
evidence: D6, 7 pixels at rounded corners of 9-slice border.
note:     Independent of S1-clip (different test). Low priority (7 px).
```

### Stage 2: Canvas Color Propagation

```
id:       S2-canvas
type:     TYPE_REPR
stage:    2 (transform/compositing setup)
status:   RESOLVED (R16)
rust:     panel/ctx.rs -- layout_child() sets geometry only, not canvas_color.
          panel/view.rs:1919 -- children inherit parent_canvas if TRANSPARENT.
cpp:      emPanel.cpp:421 -- child->Layout(x,y,w,h,canvasColor) sets both.
          emBorder.cpp:307-322 -- DoBorder computes canvasColor through border
          painting stages (outer fill → inner fill → content area).
evidence: listbox_expanded -3,184 px, testpanel_expanded -2,382 px.
          widget_colorfield unchanged (ScalarField children not viewed at
          this zoom — divergence is in IO overlay border image compositing).
fix:      Added content_canvas_color() to Border, layout_child_canvas() to
          PanelCtx, set_all_children_canvas_color() after layout in all
          border-based layout_children implementations. Also fixed missing
          set_canvas_color calls for Filled/MarginFilled outer border paint.
```

### Stage 5: Blending / Compositing

```
id:       S5-canvas-alpha
type:     FORMULA
stage:    5 (blending)
status:   RESOLVED (R17)
rust:     painter.rs:blend_pixel -- canvas_blend applied to alpha channel:
          out[3] = result.a() where canvas_blend computes
          target_a + (src_a - canvas_a) * alpha / 255. For opaque canvas and
          semi-transparent source, this reduces alpha below 255.
cpp:      emPainter_ScTlPSInt.cpp:369-371 -- HAVE_CVC path uses only
          hcR/hcG/hcB hash tables (RGB only). Alpha is never modified.
evidence: bg alpha 202 instead of 255 at ColorField swatch boundary.
          Cascading alpha corruption in subsequent compositing.
fix:      Changed blend_pixel to not modify out[3] in canvas-color path,
          matching C++ behavior.
```

```
id:       S5-outline-canvas
type:     FORMULA
stage:    5 (blending)
status:   RESOLVED (R17)
rust:     color_field.rs:359 -- paint_rect_outlined uses persistent
          painter.state.canvas_color (167,169,176 from border paint).
cpp:      emColorField.cpp:400 -- PaintRectOutline defaults to
          canvasColor=0 (TRANSPARENT). C++ uses per-call canvas_color
          parameters; Rust uses persistent painter state.
evidence: At swatch outline (y=170,508): Rust canvas_blend with border
          canvas clamps negative G,B to 0 → (207,0,0). C++ source-over
          with transparent canvas → (181,4,9). 618 px per row, 1,236 total.
fix:      Set canvas_color to TRANSPARENT before outline paint in ColorField,
          matching C++ PaintRectOutline default.
note:     Systemic issue: C++ paint API uses per-call canvasColor with
          default=0, Rust uses persistent state. Other widgets may have
          similar mismatches where C++ uses the default.
```

### Unclassified

```
id:       U-expanded
type:     UNKNOWN
stage:    UNKNOWN
status:   PARTIALLY_RESOLVED (R16 reduced max_diff 191→158)
evidence: 14,211 px in colorfield_expanded (800x800, editable=true,
          alpha_enabled=true, color=0xBB2222FF). Shares root causes with
          S4-interp (border image compositing).
```

---

## Work Phases

Work proceeds in four phases. Each phase addresses one category of
difference. Phases are strictly sequential because upstream fixes
change the pixel landscape for downstream phases.

### Phase 0: Structural Enumeration (one-time)

**Goal:** Build the structural diff ledger above.

**Method:**
1. For each rendering function pair (Rust function + C++ counterpart),
   read both and record every semantic difference.
2. Classify each difference by pipeline stage and type (TYPE_REPR,
   FORMULA, EVAL_ORDER).
3. For TYPE_REPR differences, construct a witness: specific input values
   where the two types produce different results.
4. Rank by pipeline stage (earlier = higher priority), then by estimated
   fan-out within stage.

**Output:** The structural diff ledger above, populated with all known
differences.

**Status:** PARTIALLY COMPLETE. S1-clip is fully characterized. S4-interp
and others are blocked by S1-clip or need code comparison.

**Gate 0:** At least one TYPE_REPR or FORMULA difference is identified
with line refs in both implementations. (Already passed via S1-clip.)

### Phase 1: Architectural Alignment

**Goal:** Fix all TYPE_REPR differences in pipeline-DAG order.

**Method for each TYPE_REPR item, in pipeline order:**
1. Design the architectural fix (what type changes, what code changes).
2. Implement as a single atomic batch. Do not apply partial changes
   that create inconsistent intermediate states.
3. Validate with **per-stage measurement** (see Gate 1 below).
4. Run full test suite with divergence log. Record three-number dashboard.
5. If accepted: commit, update rolling baseline, update structural
   diff ledger (cross off resolved items, note newly visible divergences).

**Gate 1 (Per-Stage Validation):**

For architectural fixes, validation is at the pipeline stage being fixed,
NOT by downstream pixel count alone:

- **Stage 1 (clip):** After applying the fix, verify that clip rect values
  (x1, y1, x2, y2) match C++ at every viewed panel. Use eprintln!
  instrumentation in both implementations to dump clip values for the
  test viewport, and diff. The fix passes if all clip rects match.
- **Downstream pixel count:** Record the three-number dashboard:
  1. Stage-N match count (how many stage-N values now match C++)
  2. Total divergent pixels (sum across all tests)
  3. Net newly-visible pixels (pixels where clip changed from
     excluding to including)
- **Bounded regression tolerance:** Total pixel count may increase by up
  to the number of newly-visible pixels. A correct clip fix may expose
  downstream bugs in newly-visible child content. This is expected.
  Regression beyond the newly-visible count indicates the fix is wrong.

**Kill condition:** After 4 failed attempts for a single architectural
item, escalate to Phase D (see below). Do NOT skip the item.

### Phase 2: Algorithmic Alignment

**Goal:** Fix FORMULA differences. Entered only after Phase 1 is complete
for the relevant pipeline stage (or after confirming the item is causally
independent of any unresolved Phase 1 items).

**Method for each FORMULA item:**
1. **Structural comparison first:** Read the Rust function and C++
   counterpart. If the formula difference is visible from code reading,
   record it directly as root_cause. No runtime instrumentation needed.
2. **Escalation to runtime probing:** If the functions appear semantically
   identical but pixels still diverge, OR the algorithm is complex enough
   that hand-tracing is unreliable (e.g., polynomial coverage rasterizer),
   THEN pick ONE divergent pixel (away from clip boundaries and panel
   edges), instrument both implementations, and extract intermediate values.
3. Fix the Rust code to match C++.
4. Run full test suite. Record three-number dashboard.

**Gate 2 (Standard Pixel Validation):**
- Total divergent pixels decreased (aggregated across all tests).
- No individual test regressed by more than 20% of the improvement.
  (Soft threshold — if the aggregate improved but one test slightly
  regressed, investigate whether the regression is a classifier artifact
  before rejecting.)

**Kill condition:** After 3 failed attempts (diagnosis + fix cycles),
record failure reason. Move to next FORMULA item. Return after all
other FORMULA items in the same stage are resolved.

### Phase 3: FP Tolerance Residual

**Goal:** Determine whether remaining divergences are fixable bugs or
inherent floating-point non-determinism.

**Entry criterion:** All TYPE_REPR and FORMULA differences resolved.

**Method:**
1. Re-run full suite with divergence log.
2. If total divergent pixels = 0: declare parity achieved.
3. For remaining divergent pixels:
   a. If max_diff > 16 per channel: there is an unidentified FORMULA
      difference. Return to Phase 0 (expand structural enumeration).
   b. If max_diff 2-15: probably FORMULA. Investigate with targeted
      pixel probing. If source formulas are identical, compile both
      with strict FP semantics and check if divergence disappears.
   c. If max_diff = 1: possibly inherent FP non-determinism. Accept
      only after confirming identical source formulas AND strict-FP
      compilation does not eliminate the difference.

### Phase D: Architectural Escalation

**Goal:** Handle items where the diagnosis is correct but the fix exceeds
the standard retry budget.

**Entry criterion:** An item's Gate 1 or Gate 2 kill condition was
reached, but the root_cause is confirmed correct (the same root cause
was found on every attempt — the issue is fix strategy, not diagnosis).

**Method:**
1. The root_cause is correct. Do NOT return to diagnosis.
2. Decompose the architectural change into sub-steps. Each sub-step
   should compile and pass tests (even if pixel counts don't improve
   yet). Example for S1-clip:
   a. Add f64 clip fields alongside existing i32 clip, compute both
   b. Verify f64 clip values match C++ (stage validation)
   c. Switch paint operations to use f64 clip, one function at a time
   d. Remove old i32 clip once all consumers are migrated
3. Each sub-step is a separate commit. Gate: compiles, tests pass
   (pixel counts may not improve until the migration is complete).
4. After all sub-steps: run full suite, record three-number dashboard,
   apply Gate 1 or Gate 2 as appropriate.

**Kill condition:** After 3 sub-step failures at the same migration
point, set kill_reason with detailed notes on what was attempted and
why it failed. Move to next item.

---

## Measurement Protocol

### Rolling Baseline

The baseline updates after each accepted fix. This prevents stale-
baseline artifacts where prior fixes' effects are double-counted or
where diagnostic observations no longer match the current code state.

**Current rolling baseline:** R13 (state/post_r13_baseline.jsonl)

After each accepted fix:
1. Run full suite:
   ```bash
   CARGO_TARGET_DIR=rust_target DIVERGENCE_LOG=$(pwd)/state/post_rXX.jsonl \
     cargo-nextest ntr --workspace --test-threads=1
   ```
2. Record the new baseline in this section.
3. Periodically reconcile against R13 to verify cumulative progress.

### Three-Number Dashboard

After every fix attempt (accepted or rejected), record:

| # | Metric | Meaning |
|---|--------|---------|
| 1 | Stage-N match count | How many intermediate values at the fixed stage now match C++ |
| 2 | Total divergent pixels | Sum of failing pixels across all tests |
| 3 | Net newly-visible pixels | Pixels where the fix changed visibility (clip boundary moved, etc.) |

### Acceptance Criteria

**For TYPE_REPR (architectural) fixes:**
- Stage-N match count increased (structural correctness improved)
- Total pixels did not increase by more than net newly-visible pixels
- Stage-specific validation passed (e.g., all clip rects match)

**For FORMULA (algorithmic) fixes:**
- Total divergent pixels decreased
- No individual test regressed by more than 20% of the improvement

**For all fixes:**
- `cargo clippy --workspace -- -D warnings` passes
- Full test suite passes (tests may have tolerance thresholds)

---

## Pipeline-Ordered Work Queue

Items are ordered by pipeline stage (upstream first), then by
estimated pixel impact within stage. Items BLOCKED_BY an upstream
item must not be diagnosed until that upstream item is resolved and
the pixel landscape is re-measured.

### Active

| Priority | Item | Stage | Type | Status | Pixels |
|----------|------|-------|------|--------|--------|
| 1 | S4-interp | 4 | EVAL_ORDER | DIAGNOSED (R19) | 2,867 (widget_colorfield) |
| 2 | S4-subpixel | 4 | EVAL_ORDER | DIAGNOSED (R19) | 98 |
| 3 | U-expanded | ? | EVAL_ORDER (probable) | PARTIALLY_RESOLVED | ~12,089 |

### Resolved

| Item | Stage | Resolution |
|------|-------|------------|
| S1-clip | 1 | RESOLVED R14: f64 clip storage, -2 px colorfield |
| S2-canvas | 2 | RESOLVED R16: canvas_color propagation to children, -5,564 px total |
| S5-canvas-alpha | 5 | RESOLVED R17: canvas_blend no longer modifies dest alpha |
| S5-outline-canvas | 5 | RESOLVED R17: ColorField outline uses TRANSPARENT canvas |
| S5-per-call-canvas | 5 | RESOLVED R18: per-call canvas_color on 26 paint functions, -57,550 px |
| TP-canvas-winding | test | RESOLVED R20: TestPanel winding rule + canvas_color fix, -1,158 px |
| S5-widget-canvas | 5 | RESOLVED R21: ScalarField/TextField canvas propagation, -312 px |

### Low Priority (independent, defer until active items resolved)

| Item | Stage | Pixels |
|------|-------|--------|
| S34-border-corner | 3/4 | 7 |

### Next Steps (post R20):

**TestPanel canvas + winding fix applied (R20, -1,158 px).**

Investigation confirmed testpanel divergence is primarily EVAL_ORDER (coordinate-space
FP), same root cause as S4-interp. The R20 fix addressed the two FORMULA differences
found in the TestPanel test code itself: winding rule and canvas_color propagation.

**All known FORMULA and canvas_color differences resolved.**

Remaining options for further parity improvement:

1. **Coordinate-space alignment** (high effort, high impact): Restructure Rust
   to use C++-style normalized panel coordinates internally, applying ScaleX/Y
   at the same point in the computation as C++. This would eliminate the FP
   differences that cause all remaining S4 divergence. Estimated impact:
   ~15,000 pixels (widget_colorfield 2,664 + colorfield_expanded 11,986)
   plus significant testpanel reduction (~141k px).

2. **Accept current parity**: All TYPE_REPR, FORMULA, and canvas_color
   differences are resolved. Remaining divergence is EVAL_ORDER (identical
   source formulas, different FP accumulation due to coordinate-space
   architecture). Total: 157,876 px across all tests.

### Historical Next Steps (post R16):

1. **S4-interp diagnosis**: The widget_colorfield divergence (5,164 px) is
   from the IO overlay border image compositing. The ScalarField children
   are NOT painted as separate panels at this zoom level. The divergence is
   at scanlines y=170, y=508 (full swatch width, 618px each), and y=283-395
   (right half, ScalarField border positions in the 9-slice border image).
2. **Structural comparison of paint_border_image**: Compare Rust
   `painter.rs:paint_border_image` with C++ `emPainter::PaintBorderImage`
   compositing path. The border image is area-sampled and composited with
   canvas_color blending — formula differences here would explain the
   remaining divergence.
3. **S4-subpixel**: Independent (widget_scalarfield, different test). Can
   proceed in parallel with S4-interp.

---

## What This Harness Requires

- **Pipeline ordering.** Fix upstream stages before diagnosing downstream.
  Do not work on S4-interp while S1-clip is unresolved.
- **Structural comparison first.** For TYPE_REPR and obvious FORMULA
  differences, diagnose from code reading. Record the evidence (line refs,
  witness values). Reserve runtime instrumentation for EVAL_ORDER or
  genuinely ambiguous cases.
- **Batch architectural fixes.** TYPE_REPR fixes that span multiple
  functions must be applied as a unit. Do not apply half the type change.
- **Re-diagnose after upstream fixes.** After S1-clip, every downstream
  item's pixel count, pixel set, and possibly root cause may have changed.
  Do not carry forward stale Phase A observations.
- **Per-stage validation for architectural fixes.** Do not reject a clip
  fix because it exposed downstream bugs. Validate clip correctness by
  comparing clip rects, not by counting downstream pixels.
- **Rolling baselines.** Always measure against the current code state.
  Periodically reconcile with R13 for cumulative progress tracking.

## What This Harness Prohibits

- **No downstream work while upstream is broken.** S4-interp must not
  be diagnosed while S1-clip is unresolved (pixels will shift).
- **No fixed baseline stagnation.** Do not measure against R13 after
  the first fix changes the pixel landscape. Use rolling baselines.
- **No pixel classifier as gate.** Per-group pixel counts (D1-D7 from
  the old harness) are informational. The acceptance gate uses total
  pixel count + stage-specific validation, not per-group deltas.
- **No unlimited Phase A ceremony.** If the code difference is visible
  from reading both functions, record it and proceed. Do not mandate
  runtime instrumentation for obvious TYPE_REPR or FORMULA diffs.
- **No skipping hard items.** If an architectural fix exceeds the retry
  budget, escalate to Phase D. Do not KILL the highest-impact item.
- **No open-ended audits.** Every phase has a kill/escalation condition.
- **No widget-level audits.** P1-P38 confirm all widget geometry and
  configuration is correct. The bugs are in `src/render/`.
- **No extrapolation across pipeline stages.** Proving a root cause at
  stage 1 does not prove the downstream symptoms at stage 4 will resolve.
  Re-measure after fixing.
