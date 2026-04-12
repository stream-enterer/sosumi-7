# FFI Border Geometry Harness: Mechanically Fix 31 Golden Failures

## Objective

Fix 31 remaining golden test failures by mechanically comparing C++ DoBorder and Rust paint_border paint-call parameters at f64-bit granularity, then fixing each divergent computation without reading code to form hypotheses.

## Status

211 pass, 31 fail on branch `ffi-harness-layer8`.

## Proven Ground Truth

1. ALL paint primitives byte-identical via FFI (layers 1-17)
2. Isolated howto PaintRoundRect passes tol=0 (8eff009)
3. Divergence is in PAINT CALL PARAMETERS (1+ ULP f64 differences)
4. Scale change is NEUTRAL (proven, do not retry)
5. Reading code to form hypotheses has failed repeatedly

## Root Cause

C++ DoBorder computes border geometry in panel-local coordinates (w=1.0). Rust paint_border computes in pixel coordinates (w=800). Same formulas at different magnitudes produce different f64 intermediates. Even when Rust runs at w=1.0 (scale=800), the pixels are identical to w=800 (scale=1), meaning the divergence is a code difference, not just magnitude.

## Architecture

### Three components, fully scripted

```
C++ DoBorder ──→ depth-0 logging ──→ hex-f64 JSONL ──→ {name}.cpp_ops.jsonl
                                                              │
Rust paint_border ──→ DrawOp recording ──→ hex-f64 JSONL ──→ {name}.rust_ops.jsonl
                                                              │
                                                    diff_draw_ops.py (upgraded)
                                                              │
                                              structured divergence report (JSONL)
                                                              │
                                              instrument_intermediates.py
                                                              │
                                              "first divergent intermediate: varname at line N"
```

### Component 1: C++ depth-tracked logging

**Problem:** C++ currently logs 384 ops for widget_button_normal because PaintRoundRect internally calls PaintPolygon, and both get logged. Rust logs 17 high-level ops. Alignment is impossible.

**Fix:** Add `g_draw_op_depth` counter to `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`:
- At entry of PaintRoundRect, PaintBorderImage, PaintBorderImageColored: if depth==0, log. Increment depth.
- At exit: decrement depth.
- Internal calls (PaintPolygon from PaintRoundRect, sub-PaintRect from PaintBorderImage) happen at depth>0 and are NOT logged.

**Hex-f64:** Add `"x_hex":"4001234567890abc"` fields alongside every decimal float parameter. Uses `snprintf` with `%016llx` on the f64 bits via `memcpy(&bits, &val, 8)`.

**Location:** Modify `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp` (already has `g_draw_op_log` infrastructure).

### Component 2: Rust hex-f64 serialization

**Modify** `crates/eaglemode/tests/golden/draw_op_dump.rs` to add `_hex` suffix fields for every float parameter, containing the `{:016x}` representation of `f64::to_bits()`.

### Component 3: Upgraded diff script

**Modify** `scripts/diff_draw_ops.py`:
- Compare `_hex` fields when present (exact bit comparison, ULP distance)
- Fall back to decimal comparison when hex not available
- Output structured JSONL to stdout: `{"param":"tx","ulp_dist":1,"cpp_hex":"...","rust_hex":"..."}`
- Handle coordinate normalization: if C++ w~1.0 and Rust w~800, divide Rust floats by scale before comparing
- Filter state ops separately from paint ops

### Component 4: Intermediate instrumentation script

**Create** `scripts/instrument_intermediates.sh`:
- Given a border type and divergent parameter, greps `paint_border()` for every `let varname =` assignment
- Generates a Rust test snippet that logs every intermediate as hex-f64
- Generates equivalent C++ `fprintf` statements for DoBorder
- Runs both, diffs all intermediates
- Outputs: first divergent intermediate variable name and both values

This script makes the LLM's job purely mechanical: run script, apply one-line fix at identified location, re-run script to verify.

## Workflow Loop (LLM executes this)

```
for each border_type in [Group, InstrumentMoreRound, Rect, RoundRect, ...]:
    1. Generate C++ JSONL (make -C tests/golden/gen run)
    2. Generate Rust JSONL (DUMP_DRAW_OPS=1 cargo test --test golden border_type)
    3. Run diff script → get divergent params
    4. If no divergences: skip this type
    5. For each divergent param:
        a. Run instrument script → get first divergent intermediate
        b. Fix the one Rust line to match C++
        c. Re-run diff → verify param matches
        d. If still diverges after 3 attempts: write diagnostic, move on
    6. Run golden tests → log progress (expect failure count to decrease)
    7. Commit fixes
```

## Hardening Gates (Mechanical, Not Policy)

### Gate 1: Script-driven only
Every action is determined by script output. The diff script outputs the work queue. The instrument script identifies the fix location. There is no step that requires reading emBorder.rs to understand anything.

### Gate 2: Fix-or-revert-3
Maximum 3 fix attempts per divergent parameter. Each attempt must be validated by re-running the diff. If 3 fail: revert, add more intermediate logging, binary-search further. Never try a 4th fix on the same information.

### Gate 3: No tolerance increases
Never. Goal is bit-identical output.

### Gate 4: Scale change is dead
Do not attempt changing painter scale/coordinate space. Proven neutral.

### Gate 5: Escalation on stall
If a border type has >5 divergent parameters unfixable after instrumentation, STOP and write a diagnostic report with all the data collected. Do not spiral.

## Failure Scenarios and Responses

| Scenario | Response |
|----------|----------|
| Fix produces bit-identical params but golden still fails | Divergence is in a DIFFERENT paint call. Continue fixing remaining divergent params. |
| All params match but golden still fails | Problem upstream of border (emView, panel setup). Add full-widget DrawOp diff. |
| Fix breaks other tests | Revert immediately. Fix was wrong. |
| Diff shows 0 divergences from start | Border type isn't the source. Skip to next type. |
| C++ and Rust op counts still differ after depth tracking | Structural difference in border code (different branches taken). Report which ops are extra/missing. |

## Border Type Progression

1. OBT_GROUP + IBT_GROUP (simplest)
2. OBT_INSTRUMENT_MORE_ROUND (widget_button_normal, most Category A failures)
3. OBT_RECT
4. OBT_ROUND_RECT
5. Remaining types as needed

## 31 Remaining Failures

### Category A: Border geometry (25 tests, max 5-33)
widget_button_normal, widget_scalarfield, widget_checkbox_*, widget_radiobutton, widget_textfield_*, widget_listbox, widget_colorfield, widget_splitter_v_extreme_tall, listbox_expanded, colorfield_expanded, golden_widget_border_roundrect_thin, golden_widget_colorfield_alpha_*, golden_widget_listbox_*, golden_widget_scalarfield_*, golden_widget_textfield_single_char_square, widget_file_selection_box

### Category B: Structural/high-diff (4 tests, max 153-255)
testpanel_root, testpanel_expanded, composition_tktest_1x/2x, composition_border_nest

### Category C: Primitive-level (3 tests, max 25-53)
painter_bezier_stroked, starfield_large, starfield_small

## Files Created/Modified

| File | Action | Purpose |
|------|--------|---------|
| `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp` | Modify | Add depth tracking + hex-f64 logging |
| `crates/eaglemode/tests/golden/draw_op_dump.rs` | Modify | Add hex-f64 fields |
| `scripts/diff_draw_ops.py` | Modify | Hex comparison, ULP distance, normalization |
| `scripts/instrument_intermediates.sh` | Create | Mechanical intermediate bisection |
| `crates/emcore/src/emBorder.rs` | Modify | Fix divergent computations (identified by scripts) |

## Non-Goals

- Fixing Category C failures (separate primitive-level fixes)
- Supporting SIMD builds (diagnostics run SIMD-off)
- Creating a LoggingPainter wrapper class (depth tracking in emPainter.cpp is simpler)
