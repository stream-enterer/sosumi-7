# Sub-Op Recording Investigation

## Problem

3 golden tests produce pixel diffs despite the DrawOp diff tool reporting zero parameter mismatches, zero state mismatches, and zero unmatched ops:

| Test | Pixel diffs | max_diff |
|------|-------------|----------|
| painter_bezier_stroked | 119/65536 (0.18%) | 53 |
| widget_border_roundrect_thin | 2/480000 (0.00%) | 24 |
| widget_splitter_v_extreme_tall | 84/480000 (0.02%) | 19 |

## Root Cause Hypotheses

The DrawOp diff tool only compares top-level ops (depth-gated). The divergence must be in one of:

- **A. Sub-op decomposition**: Compound ops (PaintBezier→PaintPolygon, PaintRoundRectOutline→PaintPolygonOutline, etc.) decompose into different sub-ops with different parameters (e.g. different polygon vertices).
- **B. Test setup**: Different initial image state or painter initialization not captured in the op stream.
- **C. Something else** not captured by DrawOp recording at any depth.

## Recording Architecture (Key Constraint)

The C++ and Rust sides record DrawOps differently:

**C++** (in `emPainter.cpp`):
- Uses `g_draw_op_log` (FILE*) and `g_draw_op_depth` (int) globals
- At each paint function entry: if `g_draw_op_log && g_draw_op_depth == 0`, write JSON line
- Then `g_draw_op_depth++`, execute the function body (which calls sub-ops), then `g_draw_op_depth--`
- Sub-ops fire but don't log (depth > 0). Logging happens during actual rendering.

**Rust** (in `emPainter.rs` / `draw_op_dump.rs`):
- `record_painter_ops()` creates a separate painter in `DrawList` mode
- `try_record()` pushes the op to a `Vec<DrawOp>` and returns `None`
- The `else { return; }` pattern means the function exits — **sub-ops never execute**
- Recording is a separate pass from rendering; no logging during actual execution

**Consequence**: To record sub-ops on the Rust side, we cannot use the existing DrawList mechanism. We must add C++-style thread-local logging that fires during actual direct-mode rendering.

## Approach

Add a thread-local JSONL logger to the Rust painter that mirrors C++'s `g_draw_op_log`/`g_draw_op_depth`. This logs during actual rendering (direct mode), capturing all ops including sub-ops with depth tracking.

### C++ Changes (temporary)

In `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`:
- Remove the `g_draw_op_depth == 0` condition from all logging guards (log at all depths)
- Add `"depth"` field to each JSON line: the current value of `g_draw_op_depth` at time of logging
- Rebuild gen_golden

### Rust Changes (temporary)

In `crates/emcore/src/emPainter.rs`:
- Add `thread_local!` static: `DRAW_OP_LOG: RefCell<Option<(Vec<String>, i32)>>` (lines + depth)
- Add helper: `fn log_draw_op(json: String)` — if logger active, push json line with current depth
- Add helper: `fn enter_compound() -> bool` / `fn leave_compound()` — increment/decrement depth
- At each paint function entry, call `log_draw_op(...)` with the same JSON format as `draw_op_dump.rs` plus `"depth"` field — log at ALL depths (no depth == 0 gate)
- In compound ops (PaintBezier, PaintEllipse, PaintRoundRect, PaintLine, PaintPolyline, etc.), wrap the body with `enter_compound()`/`leave_compound()`

In `crates/eaglemode/tests/golden/draw_op_dump.rs`:
- Add `activate_logger()` / `deactivate_and_dump(name)` that toggles the thread-local and writes the JSONL file

In `crates/eaglemode/tests/golden/painter.rs` and `widget.rs`:
- Wrap the actual rendering calls (not a separate recording pass) with `activate_logger()` / `deactivate_and_dump()`

### Diff Tool Changes

In `scripts/diff_draw_ops.py`:
- Handle `"depth"` field in JSONL
- Flatten all ops (ignore depth for alignment), diff sequentially
- Report depth alongside each mismatch so we can see if it's a top-level or sub-op divergence
- If op counts differ between C++ and Rust (sub-ops differ), report the unmatched ops

## Execution Steps

1. **C++ instrumentation**: Remove depth gate, add depth field, rebuild gen_golden, re-generate ops for 3 tests.
2. **Rust instrumentation**: Add thread-local logger, instrument paint functions, wire into tests.
3. **Rust re-record**: Run 3 tests with `DUMP_DRAW_OPS=1`, producing sub-op-inclusive JSONL.
4. **Diff tool update**: Handle depth field, flatten alignment.
5. **Diff all 3 tests**: Run diff tool, report results.
6. **Interpret**: Sub-op mismatches → hypothesis A confirmed. All match → hypothesis A eliminated, proceed to B.
7. **Revert**: Remove all temporary instrumentation.

## Success Criteria

For each of the 3 tests, one of:
- Sub-op parameter mismatch identified (hypothesis A confirmed) — shows exactly which compound op decomposes differently and what parameters diverge.
- Sub-ops match perfectly (hypothesis A eliminated) — proceed to investigate test setup (hypothesis B).

## Scope

### In Scope
- Temporary instrumentation to record and compare sub-ops
- Determining root cause category (A, B, or C) for each of the 3 tests

### Out of Scope
- Fixing any identified divergence (separate task)
- Permanent sub-op recording infrastructure (this is throwaway instrumentation)
