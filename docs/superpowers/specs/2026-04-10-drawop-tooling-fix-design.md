# DrawOp Diff Tooling Fix Design

## Problem

The DrawOp diff tool (`scripts/diff_draw_ops.py`) cannot do meaningful op comparison because of three bugs:

### Bug 1: C++ depth tracking is incomplete
C++ logs every paint call at depth 0. Only PaintRoundRect, PaintBorderImage, PaintBorderImageColored increment depth. All other compound methods (PaintText, PaintTextBoxed, PaintEllipse, etc.) leak their internal decomposition into the log.

Result: widget_button_normal produces 333 C++ ops (326 leaked PaintRect from text glyph rendering) vs 7 Rust paint ops.

### Bug 2: Multi-line text breaks JSONL
Both C++ and Rust serialize PaintTextBoxed `text` fields with literal newlines, breaking the one-JSON-per-line format. The howto text spans 800+ lines, each becoming a parse error.

### Bug 3: Diff script uses positional matching
The diff script matches ops by sequence number. With different op counts this produces meaningless TYPE MISMATCH errors. Even with matching counts, structural differences (C++ PaintPolygon where Rust uses PaintImageFull for the same visual) need alignment-based matching.

## Goal

The diff tool produces a useful per-parameter comparison for any golden test. Specifically:
- Matching ops (same type at aligned position) get parameter-level comparison with ULP distances
- Structural differences (different op types) are clearly flagged
- The tool works for any of the 27 failing tests without manual intervention

## Validated data

widget_button_normal after all fixes should produce:

| Op | C++ | Rust |
|---|---|---|
| PaintRect | 1 | 1 |
| PaintRoundRect | 3 | 3 |
| PaintBorderImage | 2 | 2 |
| PaintTextBoxed | 1 | 1 |
| **Total** | **7** | **7** |

widget_checkbox_checked will have ~1 structural difference (C++ PaintPolygon vs Rust PaintImageFull for checkbox indicator). The tool should flag this and compare the remaining matching ops.

## Solution

### Part 1: C++ depth tracking

In `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`, add `g_draw_op_depth++` at method entry and `g_draw_op_depth--` at all exit points for every compound method.

Methods that ALREADY have logging blocks but need depth tracking added:

| Method | ~Line | Leaks |
|--------|-------|-------|
| PaintTextBoxed | 2328 | PaintText → PaintImageColored, PaintRect |
| PaintText | 2232 | PaintImageColored, PaintRect |
| PaintEllipse | 1088 | PaintPolygon |
| PaintEllipseSector | 1138 | PaintEllipse, PaintPolygon |
| PaintSolidPolyline | 3467 | PaintPolygon |

Methods that need BOTH logging blocks AND depth tracking:

| Method | ~Line | Leaks |
|--------|-------|-------|
| PaintBezier | 968 | PaintPolygon |
| PaintRectOutline | 1687 | PaintRoundRect, PaintPolygon |
| PaintRoundRectOutline | 1876 | PaintRectOutline, PaintPolygon |
| PaintEllipseOutline | 1744 | PaintPolygon |
| PaintEllipseSectorOutline | 1817 | PaintEllipseOutline |
| PaintEllipseArc | 1590 | PaintEllipseOutline |
| PaintLine | 1278 | PaintPolyline |
| PaintPolyline | 1338 | PaintSolidPolyline |
| PaintBezierLine | 1412 | PaintPolyline |

For methods needing new logging blocks: add `if (g_draw_op_log && g_draw_op_depth == 0) { fprintf(g_draw_op_log, "{\"seq\":%d,\"op\":\"%s\"...}\n", g_draw_op_seq++, "MethodName"); }` before the body, then `g_draw_op_depth++` after the log, and `g_draw_op_depth--` at every return point. Log at minimum: op name, x, y, w, h (or equivalent geometry params), and color. Match the existing PaintRoundRect logging pattern.

### Part 2: Fix newline escaping

**C++ (`emPainter.cpp`)**: In the PaintTextBoxed and PaintText logging blocks, escape `\n` → `\\n` and `\t` → `\\t` in the text string before writing to JSON.

**Rust (`draw_op_dump.rs`)**: In `serialize_op` for PaintTextBoxed and PaintText, use proper JSON string escaping for the text field (replace `\n` with `\\n`, or use a JSON serialization library).

### Part 3: Diff script alignment

Replace positional matching in `scripts/diff_draw_ops.py` with LCS-based alignment:

1. **Parse**: Load both JSONL files, skip Rust state ops (PushState, PopState, SetOffset, ClipRect, SetCanvasColor, SetAlpha).
2. **Align**: Use longest-common-subsequence on op type strings to find matching positions. This handles insertions/deletions (structural differences) naturally.
3. **Compare matched ops**: For each aligned pair with matching op type, compare parameters with existing ULP logic.
4. **Report structural diffs**: For unmatched ops (only in C++ or only in Rust), report them as structural differences.
5. **Summary**: Total matched ops, total structural diffs, ULP divergences in matched ops.

### Part 4: Rebuild & validate

1. Rebuild C++: `cd ~/git/eaglemode-0.96.4 && perl make.pl build continue=yes projects=emCore`
2. Rebuild generator: `make -C crates/eaglemode/tests/golden/gen`
3. Regenerate C++ ops: `make -C crates/eaglemode/tests/golden/gen run`
4. Generate Rust ops for validation tests: `DUMP_DRAW_OPS=1 cargo test --test golden widget_button_normal widget_checkbox_checked -- --test-threads=1`
5. Diff both: `python3 scripts/diff_draw_ops.py widget_button_normal` and `widget_checkbox_checked`
6. Validate:
   - widget_button_normal: 7 ops each side, all match, only ULP geometry diffs
   - widget_checkbox_checked: ~6 matched ops + 1 structural diff flagged

### Validation for all 27 failing tests

After tooling is fixed, generate ops for all 27 failing tests and run the diff. The tool should produce useful output for every test — either parameter divergences to fix, or structural differences to investigate.

## Out of scope

- Fixing the golden test failures themselves (this spec is tooling only)
- Adding state op logging to C++ (state ops are structural, not parameter-comparable)
- Changing Rust's recording mechanism (it's already correct)
- Making the C++ logging match Rust's exact JSONL schema (field names may differ; the diff script handles aliasing)

## Risks

- Some compound methods may have complex early-return paths where depth decrement is missed. Each method's exit points must be audited. Use RAII-style or goto-cleanup pattern if needed.
- LCS alignment on large op sequences (testpanel tests with thousands of ops) may be slow. If so, fall back to positional matching when counts are equal.
