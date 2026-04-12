# DrawOp State Recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make C++ DrawOp logging include painter state (transformation + clip) so `diff_draw_ops.py` state comparison produces valid results instead of always showing defaults.

**Architecture:** Add a `fprint_state` helper to C++ emPainter.cpp that emits `state_sx/sy/ox/oy/clip_x1/y1/x2/y2` fields, call it from all 19 logging blocks, then update `track_state()` in the diff script to read inline state fields when present. Both C++ and Rust already log parameters in user-space coordinates — no transformation is needed.

**Tech Stack:** C++ (emPainter.cpp in Eagle Mode 0.96.4), Python (diff_draw_ops.py)

**Premise correction:** The handoff claimed Rust logs post-transformation (pixel-space) parameters. This is wrong — Rust's `try_record()` at `emPainter.rs:590` captures user-space values BEFORE transformation at lines 607-614. Both sides log in user-space. The real problem is simply that C++ doesn't log state at all, so `track_state()` returns defaults (scale=1, offset=0, clip=None) for every C++ paint op.

---

### Task 1: Add `fprint_state` helper to C++ emPainter.cpp

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp:56` (after `fprint_json_string`)

- [ ] **Step 1: Add helper function after fprint_json_string**

After the closing `}` of `fprint_json_string` (line 56), add:

```cpp
// Helper: append painter state fields to the current JSON object.
// Called from each paint op's logging block to embed transformation and clip state.
static void fprint_state(FILE* f, double sx, double sy, double ox, double oy,
                          double cx1, double cy1, double cx2, double cy2) {
	fprintf(f,
		",\"state_sx\":%.17g,\"state_sy\":%.17g,\"state_ox\":%.17g,\"state_oy\":%.17g"
		",\"state_clip_x1\":%.17g,\"state_clip_y1\":%.17g,\"state_clip_x2\":%.17g,\"state_clip_y2\":%.17g",
		sx, sy, ox, oy, cx1, cy1, cx2, cy2);
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd ~/git/eaglemode-0.96.4 && perl make.pl build continue=yes projects=emCore`
Expected: Build succeeds (function is defined but not yet called — no unused warning since it's `static` and the compiler may or may not warn; if it does, proceed to Task 2 immediately).

---

### Task 2: Add fprint_state call to all 19 logging blocks

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`

The call to add is always the same:

```cpp
		fprint_state(g_draw_op_log, ScaleX, ScaleY, OriginX, OriginY, ClipX1, ClipY1, ClipX2, ClipY2);
```

There are two patterns:

**Pattern A (17 blocks):** The logging block ends with `fprintf(g_draw_op_log, "}\n");` on its own line. Insert the `fprint_state` call on the line BEFORE that closing fprintf.

**Pattern B (2 blocks):** PaintPolygon and PaintSolidPolyline close with `}\n` inline in the color fprintf. Split the fprintf to remove `}` from the format string, add `fprint_state`, then add `fprintf(g_draw_op_log, "}\n");`.

- [ ] **Step 1: Pattern A blocks — insert fprint_state before each closing fprintf**

Insert `fprint_state(g_draw_op_log, ScaleX, ScaleY, OriginX, OriginY, ClipX1, ClipY1, ClipX2, ClipY2);` on the line immediately before `fprintf(g_draw_op_log, "}\n");` at each of these locations:

| Line | Paint function |
|------|---------------|
| 386 | PaintRect |
| 1006 | PaintBezier |
| 1141 | PaintEllipse |
| 1194 | PaintEllipseSector |
| 1265 | PaintRoundRect |
| 1338 | PaintLine |
| 1414 | PaintPolyline |
| 1507 | PaintBezierLine |
| 1707 | PaintEllipseArc |
| 1820 | PaintRectOutline |
| 1901 | PaintEllipseOutline |
| 2000 | PaintEllipseSectorOutline |
| 2080 | PaintRoundRectOutline |
| 2224 | PaintBorderImage |
| 2346 | PaintBorderImageColored |
| 2461 | PaintText |
| 2564 | PaintTextBoxed |

Example before (PaintRect, line ~385-387):
```cpp
		fprint_hex_f64(g_draw_op_log, "h", h);
		fprintf(g_draw_op_log, "}\n");
		fflush(g_draw_op_log);
```

Example after:
```cpp
		fprint_hex_f64(g_draw_op_log, "h", h);
		fprint_state(g_draw_op_log, ScaleX, ScaleY, OriginX, OriginY, ClipX1, ClipY1, ClipX2, ClipY2);
		fprintf(g_draw_op_log, "}\n");
		fflush(g_draw_op_log);
```

- [ ] **Step 2: Pattern B — PaintPolygon (line 460)**

Current code (lines 460-462):
```cpp
		fprintf(g_draw_op_log, "],\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"}\n",
			c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
```

Change to (remove `}` from format string, add state, add closing):
```cpp
		fprintf(g_draw_op_log, "],\"color\":\"%02x%02x%02x%02x\",\"canvas_color\":\"%02x%02x%02x%02x\"",
			c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
		fprint_state(g_draw_op_log, ScaleX, ScaleY, OriginX, OriginY, ClipX1, ClipY1, ClipX2, ClipY2);
		fprintf(g_draw_op_log, "}\n");
```

- [ ] **Step 3: Pattern B — PaintSolidPolyline (line 3695-3699)**

Current code:
```cpp
		fprintf(g_draw_op_log, "],\"thickness\":%.17g,\"color\":\"%02x%02x%02x%02x\","
			"\"canvas_color\":\"%02x%02x%02x%02x\"}\n",
			thickness,
			c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
```

Change to:
```cpp
		fprintf(g_draw_op_log, "],\"thickness\":%.17g,\"color\":\"%02x%02x%02x%02x\","
			"\"canvas_color\":\"%02x%02x%02x%02x\"",
			thickness,
			c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha(),
			canvasColor.GetRed(), canvasColor.GetGreen(), canvasColor.GetBlue(), canvasColor.GetAlpha());
		fprint_state(g_draw_op_log, ScaleX, ScaleY, OriginX, OriginY, ClipX1, ClipY1, ClipX2, ClipY2);
		fprintf(g_draw_op_log, "}\n");
```

- [ ] **Step 4: Build C++ to verify compilation**

Run: `cd ~/git/eaglemode-0.96.4 && perl make.pl build continue=yes projects=emCore`
Expected: Build succeeds with no errors.

- [ ] **Step 5: Commit C++ changes**

```bash
cd ~/git/eaglemode-0.96.4
git add src/emCore/emPainter.cpp
git commit -m "feat: embed painter state (transform + clip) in DrawOp logging"
```

---

### Task 3: Update diff_draw_ops.py to read inline state

**Files:**
- Modify: `/home/a0/git/eaglemode-rs/scripts/diff_draw_ops.py`

- [ ] **Step 1: Add STATE_INLINE_KEYS constant**

After the `STATE_OPS` definition (line 18), add:

```python
# Keys embedded in C++ paint ops for state — exclude from parameter comparison.
STATE_INLINE_KEYS = {"state_sx", "state_sy", "state_ox", "state_oy",
                     "state_clip_x1", "state_clip_y1", "state_clip_x2", "state_clip_y2"}
```

- [ ] **Step 2: Exclude inline state keys from parameter comparison in diff_ops**

At line 121 (in `diff_ops`), change:

```python
        all_keys = (set(cpp.keys()) | set(rust.keys())) - SKIP_KEYS
```

To:

```python
        all_keys = (set(cpp.keys()) | set(rust.keys())) - SKIP_KEYS - STATE_INLINE_KEYS
```

- [ ] **Step 3: Same exclusion in diff_with_state parameter comparison**

At line 213 (in `diff_with_state`), change:

```python
        all_keys = (set(cpp_op.keys()) | set(rust_op.keys())) - SKIP_KEYS
```

To:

```python
        all_keys = (set(cpp_op.keys()) | set(rust_op.keys())) - SKIP_KEYS - STATE_INLINE_KEYS
```

- [ ] **Step 4: Update track_state to read inline state from C++ ops**

Replace the paint-op branch of `track_state()` (lines 181-183):

```python
        elif kind not in STATE_OPS:
            # Paint op — snapshot current state
            paint_ops.append((op, dict(state)))
```

With:

```python
        elif kind not in STATE_OPS:
            # Paint op — use inline state if present (C++), else accumulated state (Rust)
            if "state_sx" in op:
                snap = {
                    "offset_x": op.get("state_ox", 0.0),
                    "offset_y": op.get("state_oy", 0.0),
                    "scale_x": op.get("state_sx", 1.0),
                    "scale_y": op.get("state_sy", 1.0),
                    "clip_x": op.get("state_clip_x1"),
                    "clip_y": op.get("state_clip_y1"),
                    "clip_w": (op["state_clip_x2"] - op["state_clip_x1"]) if "state_clip_x1" in op else None,
                    "clip_h": (op["state_clip_y2"] - op["state_clip_y1"]) if "state_clip_y1" in op else None,
                    "canvas_color": op.get("canvas_color", "00000000"),
                }
                paint_ops.append((op, snap))
            else:
                paint_ops.append((op, dict(state)))
```

- [ ] **Step 5: Verify script syntax**

Run: `python3 -c "exec(open('scripts/diff_draw_ops.py').read())"`
Expected: No syntax errors (will fail with SystemExit from missing args, that's OK).

- [ ] **Step 6: Commit**

```bash
cd /home/a0/git/eaglemode-rs
git add scripts/diff_draw_ops.py
git commit -m "feat: read inline painter state from C++ DrawOp logs in diff tool"
```

---

### Task 4: Rebuild and verify end-to-end

**Files:**
- No new files — validation only

- [ ] **Step 1: Rebuild C++ generator**

```bash
make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen clean && \
make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen && \
make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen run
```

Expected: Build succeeds, new `.cpp_ops.jsonl` files generated in `crates/eaglemode/target/golden-divergence/`.

- [ ] **Step 2: Verify C++ ops contain state fields**

```bash
python3 -c "
import json
ops = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/widget_button_normal.cpp_ops.jsonl') if l.strip()]
paint = [o for o in ops if 'state_sx' in o]
print(f'{len(paint)} paint ops with inline state out of {len(ops)} total')
assert len(paint) > 0, 'No inline state found!'
print('First op state:', {k: v for k, v in paint[0].items() if k.startswith('state_')})
"
```

Expected: All paint ops (should be 7 for widget_button_normal) have `state_sx` etc. fields with non-default values.

- [ ] **Step 3: Generate Rust ops and run diff on widget_button_normal**

```bash
cd /home/a0/git/eaglemode-rs
DUMP_DRAW_OPS=1 cargo test --test golden widget_button_normal -- --test-threads=1
python3 scripts/diff_draw_ops.py widget_button_normal
```

Expected: The "paint ops + state" section should now show real state comparisons (not all defaults). State fields like `STATE:scale_x`, `STATE:offset_x` should either match or show meaningful divergences — not the previous `1.0 vs 800.0` type artifacts from default state.

- [ ] **Step 4: Run diff on the three zero-mismatch tests**

```bash
DUMP_DRAW_OPS=1 cargo test --test golden painter_bezier_stroked -- --test-threads=1
DUMP_DRAW_OPS=1 cargo test --test golden widget_border_roundrect_thin -- --test-threads=1
DUMP_DRAW_OPS=1 cargo test --test golden widget_splitter_v_extreme_tall -- --test-threads=1
python3 scripts/diff_draw_ops.py painter_bezier_stroked
python3 scripts/diff_draw_ops.py widget_border_roundrect_thin
python3 scripts/diff_draw_ops.py widget_splitter_v_extreme_tall
```

Expected: These tests previously showed "zero mismatches" because state comparison was broken. Now they should show STATE divergences that reveal the actual cause of their pixel diffs.

- [ ] **Step 5: Verify JSONL validity on a text-heavy test**

```bash
DUMP_DRAW_OPS=1 cargo test --test golden testpanel_root -- --test-threads=1
python3 -c "
import json
ops = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/testpanel_root.cpp_ops.jsonl') if l.strip()]
print(f'Parsed {len(ops)} C++ ops, all valid JSONL')
"
python3 scripts/diff_draw_ops.py testpanel_root
```

Expected: Valid JSONL parsing, diff tool runs without errors.

- [ ] **Step 6: Record findings**

Document what the state comparison reveals for the three previously-zero-mismatch tests. If state divergences are found, note which state fields diverge and at which paint ops — this is the input to the next debugging phase.
