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

## Baseline (R15, 2026-03-14, post TextField enabled fix)

| Test | Pixels | Total | Pct | max_diff |
|------|--------|-------|-----|----------|
| widget_colorfield | 5,164 | 480,000 | 1.08% | 185 |
| colorfield_expanded | 14,209 | 640,000 | 2.22% | 191 |
| widget_scalarfield | 106 | 480,000 | 0.022% | 56 |

Rolling divergence log: `state/post_r15.jsonl`

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
type:     FORMULA (probable) or EVAL_ORDER
stage:    4 (rasterization)
status:   BLOCKED_BY_S1
rust:     scanline.rs, interpolation.rs
cpp:      emPainter.cpp:637-716, emPainter_ScTlIntImg.cpp
evidence: D2 pixels (x=412-682, y=284-395) overlap D1's clip-affected region.
          Cannot diagnose until S1-clip is resolved and pixel set re-measured.
note:     After S1-clip fix, pick a probe pixel well inside the child panel
          (x < 680) to avoid clip boundary contamination.
```

```
id:       S4-subpixel
type:     FORMULA (probable)
stage:    4 (rasterization)
status:   UNDIAGNOSED
rust:     scanline.rs -- polynomial coverage rasterizer
cpp:      emPainter.cpp:637-716
evidence: D5 pixels (y=516-517), h=0.916px. Vertices match (P24). max_diff=56.
note:     Independent of S1-clip (different test, different widget).
          Can proceed in parallel.
```

### Stage 3/4: Geometry / Edge AA

```
id:       S34-swatch-edge
type:     UNKNOWN
stage:    3 or 4
status:   BLOCKED_BY_S1
evidence: D4 pixels at swatch rect edges. May change after S1-clip fix.
```

```
id:       S34-border-corner
type:     UNKNOWN (possibly EVAL_ORDER)
stage:    3 or 4
status:   UNDIAGNOSED
evidence: D6, 7 pixels at rounded corners of 9-slice border.
note:     Independent of S1-clip (different test). Low priority (7 px).
```

### Unclassified

```
id:       U-expanded
type:     UNKNOWN
stage:    UNKNOWN
status:   BLOCKED_BY_S1
evidence: D7, 14,210 px in colorfield_expanded (800x800, editable=true,
          alpha_enabled=true, color=0xBB2222FF). Expected to share root
          causes with S1-clip and S4-interp at different scale.
triage:   Before deferring fully, generate diff images (DUMP_GOLDEN=1) to
          estimate what fraction of divergence comes from alpha/editable-
          specific code paths vs shared clip/interp paths.
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
| 1 | S4-interp | 4 | FORMULA | UNBLOCKED | ~5,507 (post-R14) |
| 2 | S4-subpixel | 4 | FORMULA | UNDIAGNOSED | 106 |
| 3 | S34-swatch-edge | 3/4 | UNKNOWN | UNBLOCKED | TBD (re-measure) |
| 4 | U-expanded | ? | UNKNOWN | UNBLOCKED | ~14,209 |

### Resolved

| Item | Stage | Resolution |
|------|-------|------------|
| S1-clip | 1 | RESOLVED R14: f64 clip storage, -2 px colorfield |

### Low Priority (independent, defer until active items resolved)

| Item | Stage | Pixels |
|------|-------|--------|
| S34-border-corner | 3/4 | 7 |

### Next Steps (post S1-clip):

1. Generate diff images with `DUMP_GOLDEN=1` for widget_colorfield and
   colorfield_expanded to re-classify remaining divergent pixels.
2. Diagnose S4-interp: structural comparison of scanline.rs vs
   emPainter.cpp:637-716 for rasterization formula differences.
3. Pick a probe pixel well inside child panels (away from clip boundaries)
   to isolate interpolation-specific divergence from clip residuals.

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
