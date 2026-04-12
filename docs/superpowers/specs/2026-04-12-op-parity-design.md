# Draw Op Parity: Recording, Behavior, and Golden Test Fixes

**Date**: 2026-04-12

## Problem Statement

tktest_1x shows C++ 5470 draw ops vs Rust 2559. After investigation (2026-04-11), the gap decomposes into recording format differences, behavioral divergences, and golden test failures. This spec covers fixes for all three.

## Current State

### Op Count Summary (tktest_1x)

| Op | C++ depth-0 | C++ depth-1 | C++ depth-2 | Rust | Delta (Rust - C++ depth-0) |
|----|-------------|-------------|-------------|------|---------------------------|
| PaintRect | 147 | 1422 | 1734 | 82 | -65 |
| PaintBorderImage | 166 | 0 | 0 | 166 | 0 |
| PaintPolygon | 430 | 188 | 2 | 610 | +180 |
| PaintRoundRect | 161 | 0 | 0 | 178 | +17 |
| PaintRectOutline | 27 | 0 | 0 | 27 | 0 |
| PaintText | 17 | 645 | 0 | 17 | 0 |
| PaintTextBoxed | 527 | 0 | 0 | 752 | +225 |
| PaintPolyline | 2 | 0 | 0 | 0 | -2 |
| PaintSolidPolyline | 0 | 2 | 0 | 2 | +2 |
| PaintImageFull | 0 | 0 | 0 | 14 | +14 |
| ClipRect | 0 | 0 | 0 | 185 | +185 |
| SetCanvasColor | 0 | 0 | 0 | 323 | +323 |
| SetTransformation | 0 | 0 | 0 | 169 | +169 |
| PushState | 0 | 0 | 0 | 17 | +17 |
| PopState | 0 | 0 | 0 | 17 | +17 |

### Golden Test Failures (14 of 243)

- 9 canvas_color propagation: widget_colorfield, colorfield_expanded, colorfield_alpha_near/opaque/zero, testpanel_root, tktest_1x, tktest_2x, testpanel_expanded
- 4 clip state initialization: composition_border_nest, border_roundrect_thin, splitter_v_extreme_tall, file_selection_box
- 1 layout geometry: file_selection_box (also has clip diffs)

## Phase A: Recording Parity

Goal: make `diff_draw_ops.py` output directly comparable between C++ and Rust by matching recording format.

### A1. Depth Tracking

**Problem:** C++ records a `depth` field on every op via global `g_draw_op_depth`. Rust has no depth tracking. Sub-ops (PaintRect inside PaintBorderImage, PaintText inside PaintTextBoxed) appear flat in Rust but nested in C++.

**Design:**

Add a `record_depth: u32` field to the painter state in `emPainter.rs`. Compound ops increment before executing internal calls, decrement on exit (including early returns). The depth is embedded in each `DrawOp` variant at recording time.

Compound ops that increment depth (matching C++ exactly):
- `PaintBorderImage` — increments before calling PaintImageSrcRect for each slice
- `PaintTextBoxed` — increments before calling PaintText for each line
- `PaintText` — increments before calling PaintImageColored for each character, or PaintRect for tiny text
- `PaintBezier` — increments before calling PaintPolygon
- `PaintEllipse` — increments before calling PaintPolygon

The depth counter lives on the painter struct, not the DrawOp enum. The serializer in `draw_op_dump.rs` reads it from each op's embedded depth value.

**Implementation approach:**

Option A: Add `depth: u32` to every `DrawOp` variant. Bloats the enum.

Option B: Store depth as a separate `Vec<u32>` parallel to the ops `Vec<DrawOp>`. Accessed by index during serialization.

Option C: Add depth to the painter, capture it in `try_record()`, store it in a wrapper struct `RecordedOp { depth: u32, op: DrawOp }`. Change the draw list from `Vec<DrawOp>` to `Vec<RecordedOp>`.

**Recommended: Option C.** Clean separation, no enum bloat, depth captured at recording time.

**Files:**
- `crates/emcore/src/emPainterDrawList.rs` — add `RecordedOp` struct, change `Vec<DrawOp>` to `Vec<RecordedOp>`
- `crates/emcore/src/emPainter.rs` — add `record_depth: u32` to painter, increment/decrement in compound ops, capture in `try_record()`
- `crates/eaglemode/tests/golden/draw_op_dump.rs` — serialize `depth` field from `RecordedOp`
- All call sites that create or consume `Vec<DrawOp>` — update to `Vec<RecordedOp>`

### A2. Remove State Op Recording

**Problem:** Rust records 711 state ops (ClipRect, SetCanvasColor, SetTransformation, PushState, PopState) that C++ never records. These inflate Rust op count and pollute diff output.

**Design:**

Remove all `record_state()` calls from state-mutating methods in `emPainter.rs`:
- `SetCanvasColor` (line ~409)
- `SetAlpha` (line ~415)
- `set_offset` (line ~426)
- `SetScaling` (line ~566)
- `SetTransformation` (line ~578)
- `SetClipping` (line ~447)
- `push_state` (line ~389)
- `pop_state` (line ~395)

Remove the `record_state()` method itself. Remove the corresponding `DrawOp` variants: `PushState`, `PopState`, `SetOffset`, `SetScaling`, `SetTransformation`, `ClipRect`, `SetCanvasColor`, `SetAlpha`.

Remove serialization code for these variants in `draw_op_dump.rs`.

**Caution:** Verify no test code depends on state ops being present in the recorded list. grep for these variant names in test code.

### A3. Add Inline State Fields

**Problem:** C++ embeds painter state (`state_sx`, `state_sy`, `state_ox`, `state_oy`, `state_clip_x1..y2`) in every paint op via `fprint_state()`. Rust doesn't — state was recorded as separate ops (removed in A2). Without inline state, `diff_draw_ops.py` can't compare painter state at each operation.

**Design:**

Capture a state snapshot at `try_record()` time and store it alongside each op. Extend `RecordedOp` (from A1):

```rust
pub struct RecordedOp {
    pub depth: u32,
    pub op: DrawOp,
    pub state: RecordedState,
}

pub struct RecordedState {
    pub scale_x: f64,
    pub scale_y: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub clip_x1: f64,
    pub clip_y1: f64,
    pub clip_x2: f64,
    pub clip_y2: f64,
}
```

In `draw_op_dump.rs`, append these as JSON fields matching C++ key names: `state_sx`, `state_sy`, `state_ox`, `state_oy`, `state_clip_x1`, `state_clip_y1`, `state_clip_x2`, `state_clip_y2`.

Note: C++ stores clip in pixel space (after scale+offset). Rust stores clip in pixel space too (`self.state.clip.x1` etc.). Verify the coordinate systems match.

### A4. Normalize PaintImage as PaintRect

**Problem:** C++ `PaintImage()` is an inline around `PaintRect(emImageTexture(...))`, recorded as `PaintRect`. Rust records as `PaintImageFull` with different fields. 14 ops affected.

**Design:**

In `draw_op_dump.rs` serialization, when serializing `DrawOp::PaintImageFull`, emit `"op":"PaintRect"` with fields matching C++ PaintRect format: `x`, `y`, `w`, `h`, `color` (use the image's representative color or `"00000000"`), `canvas_color`, plus hex encodings.

Alternative: change the recorder to not record PaintImageFull at all when it's called from PaintImage (only record the parent PaintRect). But this is harder since PaintImage doesn't go through PaintRect in Rust.

**Recommended:** Normalize at serialization time in `draw_op_dump.rs`. Simpler, no rendering code changes.

**Open question:** What `color` value does C++ record for PaintImage-as-PaintRect? Need to check. If it records the texture descriptor rather than a solid color, the field format may differ. Investigate during implementation.

### A5. Update diff_draw_ops.py

**Problem:** After A1-A4, the diff script needs updates to leverage the new format.

**Design:**

- Remove state op filtering logic (no longer needed — state ops won't exist in Rust output)
- Use `depth` field for comparison (now present in both sides)
- Add `--depth N` flag to filter ops by depth (e.g., `--depth 0` for top-level only)
- Remove the state accumulation logic (`track_state()`) — inline state fields now provide this directly
- Remove `STATE_OPS` set and related handling

### Verification (Phase A)

After all A1-A5 changes:

```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 scripts/diff_draw_ops.py tktest_1x
```

Expected:
- No `C++ ONLY` or `RUST ONLY` for state ops
- Depth field present on all ops in both files
- Structural divergences reduced to only behavioral differences (Phase B items) and file viewer stubs
- Inline state fields present and comparable

## Phase B: Behavioral Divergences

Goal: match C++ rendering behavior op-for-op at depth 0 (minus file viewer stubs).

### B1. Timestamp Format in emScalarField

**Problem:** C++ renders time values as `HH:MM` (e.g., `"00:15"`). Rust renders `HH:MM:SS` (e.g., `"00:15:00"`). ~193 PaintTextBoxed ops affected in tktest_1x.

**Root cause:** Rust's time-to-string formatting in emScalarField includes seconds; C++ doesn't.

**Fix:** Find the time formatting function in Rust's emScalarField and match C++ format. Compare:
- C++: `~/git/eaglemode-0.96.4/src/emCore/emScalarField.cpp` — search for time formatting
- Rust: `crates/emcore/src/emScalarField.rs` — search for time/duration formatting

Change Rust to produce `HH:MM` format matching C++.

### B2. ListBox Item Rendering Excess

**Problem:** Rust emits ~180 extra PaintTextBoxed for item numbers ("11"..."100", each appearing twice) and ~49 extra "Item N" label PaintTextBoxed (Rust 13 each vs C++ 6 each). C++ has 0 depth-0 PaintTextBoxed for item numbers.

**Hypotheses:**
1. Rust's ListBox creates more expanded children than C++ (expansion/visibility culling difference)
2. Rust renders item content where C++ doesn't (PaintContent override difference)
3. C++ renders item text as PaintText (depth 1 inside a parent PaintTextBoxed), Rust records at depth 0

**Investigation approach:** After Phase A (with depth tracking), re-run the diff and check:
- Do C++ item numbers appear at depth > 0?
- How many ListBox item children does each side expand?
- Compare Rust emListBox expansion logic against C++ emListBox

**Fix:** Depends on investigation findings. Likely either a culling threshold difference or an expansion count difference.

### B3. Missing Text Field Content Painting

**Problem:** C++ paints 4 text field content strings ("This is an editable text field.", "This is a read-only text field.", "This is an editable multi-line text field.", "This is an editable password text field.") as PaintTextBoxed. Rust paints 0 of these.

**Root cause:** Rust's emTextField doesn't paint the text content in its Paint method, or the content painting is gated by a condition that evaluates differently.

**Investigation approach:** Compare:
- C++: `~/git/eaglemode-0.96.4/src/emCore/emTextField.cpp` — PaintContent or Paint method
- Rust: `crates/emcore/src/emTextField.rs` — Paint method

Find where C++ emits PaintTextBoxed for the content text and add the equivalent to Rust.

### B4. PaintPolygon Difference (Rust 610 vs C++ 618)

**Problem:** Rust has 610 PaintPolygon at depth 0; C++ has 430 at depth 0 and 188 at depth 1 (618 total). The depth-0 difference (610 vs 430 = +180) likely comes from PaintBezier/PaintEllipse decomposition — Rust may record the sub-polygons at depth 0 while C++ records them at depth 1.

**Investigation approach:** After Phase A depth tracking, re-run diff. The 180 extra Rust depth-0 PaintPolygon should shift to depth 1, matching C++ 188. If not, investigate which compound ops produce different polygon counts.

### B5. PaintPolyline Difference

**Problem:** C++ has 2 PaintPolyline at depth 0; Rust has 0 PaintPolyline but 2 PaintSolidPolyline. C++ has 2 PaintSolidPolyline at depth 1.

**Investigation:** This may be a naming difference (C++ records PaintSolidPolyline as PaintPolyline) or a behavioral difference. Check after Phase A depth tracking resolves the depth confusion.

### Verification (Phase B)

After all B1-B5 fixes:

```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 scripts/diff_draw_ops.py tktest_1x --depth 0
```

Expected:
- PaintTextBoxed count matches at depth 0 (minus file viewer stubs)
- No unexplained Rust-only or C++-only ops at depth 0
- PaintPolygon counts match at each depth level
- Timestamp text content matches between C++ and Rust

## Phase C: Golden Test Failures

Goal: reduce the 14 failing golden tests toward 0.

### C1. Canvas_color Propagation (9 tests)

**Problem:** Rust over-propagates or under-propagates canvas_color through the panel tree. Three sub-types observed:
- Type A: Rust has canvas_color where C++ has 0 (over-propagation)
- Type B: Rust has 0 where C++ has canvas_color (under-propagation)
- Type C: Both non-zero, different values

**Affected tests:** widget_colorfield (1 mismatch), colorfield_expanded (1), colorfield_alpha_near (1), colorfield_alpha_opaque (1), colorfield_alpha_zero (1), testpanel_root (4), tktest_1x (142), tktest_2x (22), testpanel_expanded (791)

**Root cause hypothesis:** Rust's split architecture separates `paint_border()` (which updates canvas_color) from widget content painting. C++ DoBorder updates a local `canvasColor` variable and passes it directly to PaintContent. The handoff between Rust's border painting and content painting may not carry the correct canvas_color.

**Investigation approach (per test, starting with simplest):**

1. Start with the 5 colorfield tests (1 Type A mismatch each, same root cause)
2. Use Phase A's improved diff tool to identify the exact op where canvas_color diverges
3. Trace the canvas_color value through compositor → paint_border → content paint
4. Compare against C++ DoBorder's canvasColor at the equivalent point
5. Fix the propagation, verify the specific test passes
6. Proceed to testpanel_root (4 mismatches), then tktest/testpanel_expanded

**Likely fix pattern:** The compositor needs to read canvas_color back from the painter after paint_border completes and pass it to the content painting phase. Currently it may use a stale value.

### C2. Clip State Initialization (4 tests)

**Problem:** C++ records initial clip values at seq 0; Rust records different values or none. Tests with ONLY clip diffs (no canvas_color): border_roundrect_thin, splitter_v_extreme_tall.

**Affected tests:** composition_border_nest (41 diffs), border_roundrect_thin (4 clip diffs only), splitter_v_extreme_tall (4 clip diffs only), file_selection_box (60 diffs)

**Root cause:** C++ initializes painter clip from image dimensions in pixel coordinates. Rust's recording-mode painter may initialize clip differently (e.g., from logical coordinates before scaling, or with a default sentinel).

**Investigation approach:**

1. Compare C++ painter initialization (`emPainter::emPainter` constructor) clip setup vs Rust `emPainter::new_recording()` clip setup
2. Check the inline state fields (from Phase A) to see exactly what clip values each side records at seq 0
3. Fix Rust initialization to match C++

**Likely fix:** Set initial clip in `new_recording()` to match C++ pixel-space clip (0, 0, width, height) or equivalent.

### C3. Layout Geometry (file_selection_box)

**Problem:** PaintTextBoxed and PaintRoundRect coordinates differ by orders of magnitude at specific ops in file_selection_box test.

**Root cause:** Layout computation in emFileSelectionBox produces different coordinates.

**Investigation approach:**

1. Use Phase A diff tool to identify the exact op(s) with coordinate divergence
2. Trace the coordinates back through emFileSelectionBox layout to find where the computation diverges
3. Compare against C++ emFileSelectionBox layout

**Note:** file_selection_box also has clip diffs (C2), so C2 should be fixed first to isolate the layout issue.

### Verification (Phase C)

After each fix, run the specific test:

```bash
cargo test --test golden <test_name> -- --test-threads=1
```

After all fixes:

```bash
cargo test --test golden -- --test-threads=1 2>&1 | grep 'test result'
```

Target: 0 failures (228 + 14 = 242 pass, 1 ignored).

## Phase Dependencies

```
Phase A (Recording Parity)
    ├── A1 Depth tracking
    ├── A2 Remove state ops
    ├── A3 Inline state fields
    ├── A4 Normalize PaintImage
    └── A5 Update diff script
         │
    ┌────┴────┐
Phase B       Phase C
(Behavioral)  (Golden Tests)
    ├── B1 Timestamps     ├── C1 canvas_color (start with 5 colorfield)
    ├── B2 ListBox excess ├── C2 Clip init (then border_roundrect, splitter)
    ├── B3 TextField      └── C3 Layout geometry (after C2)
    ├── B4 PaintPolygon
    └── B5 PaintPolyline
```

Phase A must complete first. Phases B and C are independent.

Within Phase C: C2 (clip init) before C3 (layout geometry for file_selection_box).

## Non-Goals

- Porting file viewer plugins (51 missing ops from Cargo.toml/build.rs viewers)
- Pixel-level golden test tolerance tuning
- Performance optimization of recording
- Recording format versioning or backwards compatibility
