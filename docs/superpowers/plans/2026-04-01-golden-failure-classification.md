# Golden Failure Classification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Classify all 42 golden test failures by shared rendering code path, producing a prioritized catalog of fix groups.

**Architecture:** Generate diff images for all failures, examine divergent pixel coordinates from test output, trace each failure from test → widget → paint method → blend function, record the divergent Rust code path and corresponding C++ reference, then group tests that share a code path. No production code changes — output is a catalog document.

**Tech Stack:** Rust golden tests, C++ reference at `~/git/eaglemode-0.96.4/`, PPM diff images

**Key files:**
- Test files: `crates/eaglemode/tests/golden/{painter,widget,composition,compositor,eagle_logo,starfield,cosmos_items,test_panel}.rs`
- Common test infra: `crates/eaglemode/tests/golden/common.rs`
- Rendering pipeline: `crates/emcore/src/emPainter.rs`, `crates/emcore/src/emPainterScanlineTool.rs`, `crates/emcore/src/emPainterInterpolation.rs`, `crates/emcore/src/emColor.rs`
- C++ references: `~/git/eaglemode-0.96.4/src/emCore/emPainter*.cpp`, `~/git/eaglemode-0.96.4/src/emCore/emColor.cpp`
- Output: `docs/superpowers/specs/2026-04-01-golden-failure-catalog.md`

**Critical constraint:** This is a read-only investigation. Do NOT modify any production code (anything under `crates/`). The only files created or modified are documentation under `docs/`.

---

### Task 1: Generate diff images and capture structured error data for all 42 failures

Generate PPM diff images for every failing test and capture the full error output (divergent pixel coordinates, actual vs expected values, max_diff). This raw data drives all subsequent classification.

**Files:**
- Read: `crates/eaglemode/tests/golden/common.rs` (understand dump format)
- Output: `target/golden-debug/diff_*.ppm` (42 diff images), `target/golden-debug/errors.txt` (captured error output)

- [ ] **Step 1: Run all golden tests with DUMP_GOLDEN=1 and capture output**

```bash
DUMP_GOLDEN=1 cargo test --test golden -- --test-threads=1 2>&1 | tee target/golden-debug/errors.txt
```

This creates PPM files at `target/golden-debug/{actual,expected,diff}_<name>.ppm` for each failing test. The error output contains the first 10 divergent pixel coordinates per test with actual/expected RGB values.

Note: PPM files can be viewed with `display`, `feh`, `gimp`, or converted with `convert file.ppm file.png` (ImageMagick). On headless systems, use `file` to verify they exist and check dimensions.

- [ ] **Step 2: Verify all 42 diff images were generated**

```bash
ls target/golden-debug/diff_*.ppm | wc -l
```

Expected: 42 (one per failing test). If fewer, check `errors.txt` for tests that failed before reaching the dump step.

- [ ] **Step 3: Extract a summary table from the error output**

```bash
grep -E 'max_diff=|Dumped:' target/golden-debug/errors.txt | head -100
```

This gives a quick cross-reference: test name → max_diff → dump file path.

---

### Task 2: Classify the 8 max_diff=1 tests

These tests have the smallest divergence (±1 per channel) and likely share a single arithmetic root cause. Classify them as a group.

**The 8 tests:** gradient_radial, image_scaled, multi_compose, composed_splitter_content, widget_splitter_h, widget_splitter_h_pos0, widget_splitter_h_pos1, widget_error_panel

**Files:**
- Read: `crates/eaglemode/tests/golden/painter.rs` (gradient_radial, image_scaled, multi_compose)
- Read: `crates/eaglemode/tests/golden/widget.rs` (splitter tests, error_panel)
- Read: `crates/eaglemode/tests/golden/composition.rs` (composed_splitter_content)
- Read: `crates/emcore/src/emPainter.rs` (paint methods called by these tests)
- Read: `crates/emcore/src/emPainterScanlineTool.rs` (blend functions)
- Read: `crates/emcore/src/emColor.rs` (canvas_blend, blend_hash_lookup)
- Read C++: `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlPSInt.cpp` (hash table blending)
- Read C++: `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlPSCol.cpp` (solid color blending)
- Output: Notes in catalog document

- [ ] **Step 1: Examine the error output for all 8 tests**

From `target/golden-debug/errors.txt`, extract the divergent pixel coordinates and actual/expected values for each of the 8 tests. Look for patterns:
- Are the ±1 differences always in the same direction (Rust always +1 or always -1)?
- Are the divergent pixels at edges (coverage/AA boundaries) or in flat regions?
- Do multiple tests diverge at the same screen coordinates?

Key data from the test run (already captured):
- `gradient_radial`: max_diff=1, 0.05%, 32 pixels — radial gradient edge pixels
- `image_scaled`: max_diff=1, 0.75%, 493 pixels — scaled image pixels
- `multi_compose`: max_diff=1, 7.18%, 4703 pixels — large area of composed content
- `composed_splitter_content`: max_diff=1, 0.002%, 9 pixels — splitter decoration pixels
- `widget_splitter_h`: max_diff=1, 0.0002%, 1 pixel at (402,596)
- `widget_splitter_h_pos0`: max_diff=1, 0.0002%, 1 pixel at (8,596)
- `widget_splitter_h_pos1`: max_diff=1, 0.0002%, 1 pixel at (796,596)
- `widget_error_panel`: max_diff=1, 0.0006%, 3 pixels

- [ ] **Step 2: Examine the diff images**

View the diff PPMs for these 8 tests. Note: the diff visualization in `common.rs` shows max_ch_diff > 1 as red and ≤ 1 as green. Since all these tests have max_diff=1, the diff images will show ALL pixels as green (matching). The diff images alone won't locate the divergent pixels — use the error output coordinates from Step 1 instead.

- [ ] **Step 3: Trace each test to its paint calls**

For each test, read the test source and identify:
1. What widget/panel is rendered
2. What `Paint` method is called
3. What canvas color is set (TRANSPARENT = source-over, opaque = canvas-blend)
4. What primitives the widget paints (rects, ellipses, gradients, borders, images)

Record the call chain for each test. Focus on which blend path is taken: `blend_scanline_canvas` (canvas-blend) vs `blend_scanline_source_over`.

- [ ] **Step 4: Identify the divergent rendering primitive**

Using the divergent pixel coordinates from Step 1, determine which rendering primitive produced each divergent pixel. For the splitter tests, the single pixel at y=596 in an 800×600 image is near the bottom edge — likely a border or separator line. For multi_compose with 7.18% failures, the divergence covers a large area — likely a gradient or filled region.

Read the Rust code for the identified primitives and find the exact function that computes the pixel value.

- [ ] **Step 5: Find the corresponding C++ code path**

For each identified Rust function, find the equivalent C++ function in `~/git/eaglemode-0.96.4/src/emCore/`. The key C++ files:
- Solid color painting: `emPainter_ScTlPSCol.cpp`
- Gradient/image painting with interpolation: `emPainter_ScTlPSInt.cpp`
- Canvas blending hash table: `emPainter.cpp:190-234`

Compare the Rust and C++ code paths. For ±1 differences, look for:
- Hash table lookup vs direct computation
- Different rounding (`+127)/255` vs `+0x8073)>>16`)
- Coverage computation differences (`>>12` vs `/4096`)

- [ ] **Step 6: Record the group entry**

Write the group entry for the catalog. Example format:

```markdown
### G1: ±1 arithmetic (blend/coverage rounding)

**Tests (8):** gradient_radial, image_scaled, multi_compose, composed_splitter_content, widget_splitter_h, widget_splitter_h_pos0, widget_splitter_h_pos1, widget_error_panel

**max_diff range:** 1

**Divergent code path:** [identified Rust function] → [blend function]

**C++ reference:** [file:line]

**Spatial pattern:** [description from diff images and coordinates]

**Hypothesis:** [identified difference between Rust and C++ code]
```

Note: These 8 tests may split into multiple sub-groups if they diverge at different code paths. If so, create separate group entries.

---

### Task 3: Classify the widget_scalarfield group (4 tests, max_diff=12)

**The 4 tests:** widget_scalarfield, widget_scalarfield_zero_range, widget_scalarfield_min_value, widget_scalarfield_max_value

These all render `emScalarField` variants. They likely share a single code path.

**Files:**
- Read: `crates/eaglemode/tests/golden/widget.rs` (scalarfield test functions)
- Read: `crates/emcore/src/emScalarField.rs` (Paint method)
- Read: `crates/emcore/src/emPainter.rs` (whatever paint methods emScalarField calls)
- Read C++: `~/git/eaglemode-0.96.4/src/emCore/emScalarField.cpp` or equivalent
- Output: Notes in catalog document

- [ ] **Step 1: Examine error output and diff images**

From the test run output:
- All 4 tests have max_diff=12
- Divergent pixels are at symmetric x-coordinates (e.g., x=342 and x=457, or x=67 and x=732) — this suggests a symmetric widget with divergence at the same relative position on both sides
- Differences are small (±1 per channel at most pixels, up to ±12 at specific locations)

View the diff images for these 4 tests. With max_diff=12, some pixels will show as red (diff > 1) in the diff visualization.

- [ ] **Step 2: Trace the rendering path**

Read the `widget_scalarfield` test in `widget.rs` to find how `emScalarField` is constructed and rendered. Then read `emScalarField`'s `Paint` method to identify what primitives it paints:
- Border (likely `PaintBorderImage` or `emBorder::Paint`)
- Scale markings (likely `PaintRect` or `PaintLine`)
- Slider knob (likely `PaintEllipse` or `PaintRoundRect`)
- Value display (likely text)

The divergent pixels at x=342/457 with y~146-160 are likely at the slider knob or scale markings.

- [ ] **Step 3: Identify the divergent primitive and C++ equivalent**

Using the divergent pixel coordinates, determine which primitive produces them. Read the Rust implementation and find the corresponding C++ code. Compare the two.

- [ ] **Step 4: Record the group entry**

Write the group entry for the catalog with the same format as Task 2 Step 6.

---

### Task 4: Classify the widget border/decoration group

Many widget tests share the `emBorder` rendering code. Classify tests that diverge at border/decoration painting.

**Candidate tests (by visual pattern — borders, edges, decorations):** widget_checkbox_checked (236), widget_checkbox_unchecked (22), widget_button_normal (31), widget_radiobutton (31), widget_textfield_content (26), widget_textfield_empty (26), widget_textfield_single_char_square (26), widget_listbox (136), widget_listbox_single (25), widget_listbox_empty (25), widget_colorfield (24), widget_colorfield_alpha_near (24), widget_colorfield_alpha_opaque (24), widget_colorfield_alpha_zero (24), golden_widget_border_roundrect_thin (24), widget_border_round_rect (79), widget_splitter_v_extreme_tall (19), listbox_expanded (33), colorfield_expanded (54)

This is the largest candidate group (19 tests). It will likely split into sub-groups based on which specific border/decoration code path diverges.

**Files:**
- Read: `crates/eaglemode/tests/golden/widget.rs` (all widget test functions)
- Read: `crates/emcore/src/emBorder.rs` (Paint method — this is the primary suspect)
- Read: `crates/emcore/src/emPainter.rs` (PaintBorderImage, PaintRoundRect, PaintEllipse)
- Read C++: `~/git/eaglemode-0.96.4/src/emCore/emBorder.cpp`
- Read C++: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp` (PaintBorderImage, PaintRoundRect)
- Output: Notes in catalog document

- [ ] **Step 1: Examine error output for all 19 candidate tests**

Extract divergent pixel coordinates from `errors.txt`. Look for:
- Do divergent pixels cluster at widget borders (edges of the rendered area)?
- Do multiple tests share the same divergent coordinates relative to the widget (e.g., all at the top-left border corner)?
- Are there tests where the divergent pixels are NOT at borders?

Key observation from the test run: many widget tests show divergent pixels at coordinates like (31,288)-(39,289) — these are likely at a specific border element. Multiple tests (button, radiobutton, textfield variants, listbox variants) share these exact coordinates, confirming a shared code path.

- [ ] **Step 2: View diff images for representative tests**

View diff PPMs for a few representative tests with different max_diff values:
- widget_checkbox_checked (max_diff=236) — highest in this group
- widget_colorfield (max_diff=24) — mid-range
- widget_checkbox_unchecked (max_diff=22) — lower
- widget_border_round_rect (max_diff=79) — borderline primitive

Compare the spatial patterns. Tests with high max_diff likely have structural differences (missing/shifted border elements), while low max_diff may be arithmetic (rounding at border edges).

- [ ] **Step 3: Trace emBorder::Paint to its primitives**

Read `emBorder.rs` to understand what paint calls `emBorder::Paint` makes. It likely calls some combination of:
- `PaintBorderImage` (the main border rendering — 9-slice image scaling)
- `PaintRect` (background fills)
- `PaintRoundRect` (rounded corners)
- `PaintText` (labels/captions)
- `PaintImage` (icons)

Identify which of these calls produce pixels at the divergent coordinates.

- [ ] **Step 4: Sub-group by divergent code path**

Based on Steps 1-3, split the 19 candidates into sub-groups:
- Tests where the divergence is in `PaintBorderImage` (9-slice rendering)
- Tests where the divergence is in `PaintRoundRect` (corner rendering)
- Tests where the divergence is in the widget's own content (not the border)
- Tests where the divergence is at the colorfield gradient (separate code path)

For each sub-group, identify the exact Rust function and C++ equivalent.

- [ ] **Step 5: Record group entries**

Write one group entry per sub-group in the catalog. Some of these 19 tests may actually belong to other groups (e.g., colorfield tests might diverge at gradient painting, not border painting).

---

### Task 5: Classify the composite/panel tests

These tests render full panels with multiple widgets. Their divergences are likely caused by the same code paths identified in Tasks 2-4, amplified through compositing.

**The tests:** testpanel_expanded (255), testpanel_root (255), composition_tktest_1x (239), composition_tktest_2x (239), widget_file_selection_box (237), composed_border_nest (153)

**Files:**
- Read: `crates/eaglemode/tests/golden/test_panel.rs`
- Read: `crates/eaglemode/tests/golden/composition.rs`
- Read: `crates/eaglemode/tests/golden/widget.rs` (widget_file_selection_box)
- Output: Notes in catalog document

- [ ] **Step 1: Examine error output and diff images**

These tests have max_diff=153-255, which means large contiguous regions of wrong pixels. View the diff images — they should clearly show which regions diverge.

From the test run output:
- testpanel_expanded/root: divergent at (22,26) onwards — actual=rgb(24,47,70) vs expected=rgb(0,28,56), max_diff=255
- composition_tktest_1x: divergent at (31,69) — actual=rgb(143,152,180) vs expected=rgb(94,107,147), max_diff=239
- widget_file_selection_box: divergent at (747,45) — flat regions where actual=rgb(81,94,132) vs various expected values

- [ ] **Step 2: Determine if divergences are composite or primary**

For each test, check whether the divergent pixels come from:
1. A rendering primitive that's already classified in Tasks 2-4 (composite divergence — the panel just combines widgets that individually diverge)
2. A panel-level rendering issue (panel layout, compositor blending, etc.)

This is done by cross-referencing: if testpanel_expanded diverges at the same widget location as widget_checkbox_checked, it's the same root cause.

- [ ] **Step 3: Record group assignments**

Assign each composite test to the group whose root cause produces its divergence. If a composite test has multiple divergent regions from different root causes, note all of them — but assign the test to the group of its highest-max_diff divergence (the dominant cause).

---

### Task 6: Classify the remaining special-case tests

**The tests:** eagle_logo (175), cosmos_item_border (130), starfield_small (69), starfield_large (53), bezier_stroked (53)

These tests render specialized content that may not share code paths with widget tests.

**Files:**
- Read: `crates/eaglemode/tests/golden/eagle_logo.rs`
- Read: `crates/eaglemode/tests/golden/cosmos_items.rs`
- Read: `crates/eaglemode/tests/golden/starfield.rs`
- Read: `crates/eaglemode/tests/golden/painter.rs` (bezier_stroked)
- Read: `crates/emcore/src/emPainter.rs` (PaintEllipse, PaintBezier, paint_linear_gradient)
- Read: relevant C++ source files
- Output: Notes in catalog document

- [ ] **Step 1: Classify eagle_logo**

From the test run: max_diff=175, 55.23% of pixels fail. First divergent pixel: actual=rgb(145,171,242) vs expected=rgb(144,171,242) — a ±1 difference affecting 55% of pixels. But max_diff=175 means some pixels have large differences (the pixel at (0,1): actual=rgb(145,171,242) vs expected=rgb(192,228,67) — completely different).

The ±1 gradient differences (e.g., R=145 vs R=144) and the large differences (e.g., the (0,1) pixel) are likely separate issues:
- ±1 gradient: canvas-blend hash table rounding (related to the architectural mismatch thesis)
- Large differences: possibly the eagle logo shape rendering itself

Read `eagle_logo.rs` and `emMainContentPanel`'s Paint method. The test sets `canvas=TRANSPARENT` but fills with BLACK. The C++ generator uses a different canvas setup. Trace the gradient and shape rendering paths.

- [ ] **Step 2: Classify cosmos_item_border**

From the test run: max_diff=130, divergent at (0,11) where actual=rgb(0,0,0) vs expected=rgb(52,78,130). The actual is BLACK where the expected has color — this is a structural issue (missing rendering at those coordinates).

Read the cosmos_items test and `emVirtualCosmosItemPanel` Paint method. The divergence is at the border — likely a `PaintBorderImage` or border-ellipse issue.

- [ ] **Step 3: Classify starfield_small and starfield_large**

From the test run: starfield_small max_diff=69 (star edge pixels), starfield_large max_diff=53 (star edge pixels). The divergent pixels have RGB values that differ significantly — actual and expected have different color distributions (e.g., actual=rgb(62,61,70) vs expected=rgb(69,63,70)).

The starfield renders stars using `PaintEllipse` (small viewport) and `PaintImageColored` (large viewport). Both use sub-pixel rendering where the Rust and C++ rasterizers may differ. This is likely a `PaintEllipse` polygon vertex count or `PaintImageColored` sampling difference.

Read the starfield panel Paint method and trace which paint calls produce the divergent star pixels.

- [ ] **Step 4: Classify bezier_stroked**

From the test run: max_diff=53, 0.18%, 119 pixels. Divergent pixels at bezier curve edges — likely a `PaintBezier` or stroke rasterization difference.

Read the bezier_stroked test in `painter.rs` and the `PaintBezier`/stroke implementation.

- [ ] **Step 5: Record group entries**

Each of these 5 tests may form its own group or join an existing group. Record entries accordingly.

---

### Task 7: Assemble the catalog, priority-order groups, and commit

Combine all group entries from Tasks 2-6 into the final catalog document. Priority-order the groups and commit.

**Files:**
- Create: `docs/superpowers/specs/2026-04-01-golden-failure-catalog.md`
- Modify: `/home/a0/.claude/projects/-home-a0-git-eaglemode-rs/memory/divergence_inventory.md`

- [ ] **Step 1: Assemble the summary table**

Create the catalog with a summary table listing all groups:

```markdown
# Golden Failure Catalog

## Summary

| Group | Code Path | C++ Reference | Tests | max_diff range | Likely cause |
|-------|-----------|---------------|-------|----------------|--------------|
| G1 | ... | ... | ... | ... | ... |
| G2 | ... | ... | ... | ... | ... |
```

- [ ] **Step 2: Priority-order the groups**

Order by:
1. Number of tests affected (descending)
2. Likely difficulty (ascending — arithmetic before structural)
3. max_diff (ascending — ±1 fixes are likely simpler)

- [ ] **Step 3: Add per-group detail sections**

For each group, include:
- Diff image description (spatial pattern)
- Traced call chain (test → paint method → blend function)
- Corresponding C++ code path and file/line
- Hypothesis for the divergence

- [ ] **Step 4: Verify coverage**

Count the tests in the catalog and verify exactly 42 tests are covered, each in exactly one group:

```bash
# After writing the catalog, count test names mentioned in group sections
grep -c 'test_name_pattern' docs/superpowers/specs/2026-04-01-golden-failure-catalog.md
```

- [ ] **Step 5: Update memory**

Replace the content of `/home/a0/.claude/projects/-home-a0-git-eaglemode-rs/memory/divergence_inventory.md` with a pointer to the catalog and a brief summary of the groups found. Remove stale divergence data.

- [ ] **Step 6: Commit the catalog**

```bash
git add docs/superpowers/specs/2026-04-01-golden-failure-catalog.md
git commit -m "docs: add golden failure classification catalog

Classifies all 42 golden test failures into groups by shared rendering
code path. Each group identifies the divergent Rust function, the
corresponding C++ reference, and a hypothesis for the root cause.
Groups are priority-ordered for fix planning."
```

---

## Critical Rules

1. **No production code changes.** This is classification only. Do not modify anything under `crates/`.
2. **C++ source is truth.** When tracing C++ code paths, read the actual source at `~/git/eaglemode-0.96.4/` — do not trust Rust code comments or prior documentation.
3. **Group aggressively.** Tests that diverge at the same function get grouped together, even if max_diff differs.
4. **Every test must appear in exactly one group.** If a test has divergences from multiple code paths, assign it to the group of the dominant (highest max_diff) divergence and note the secondary divergence.
5. **Diff images first.** Always examine the diff image or error coordinates before reading code. The spatial pattern tells you where to look.
