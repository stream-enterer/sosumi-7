# Sub-Op Recording Investigation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Determine why 3 golden tests produce pixel diffs despite zero DrawOp parameter mismatches, by recording sub-ops (internal polygon vertices from compound ops) on both C++ and Rust and diffing them.

**Architecture:** Remove depth gating on C++ to expose sub-ops (PaintPolygon calls from compound ops). On Rust, add a thread-local vertex logger at `fill_polygon_aa` call sites (since Rust compound ops call `fill_polygon_aa` directly, not `PaintPolygon`). Update diff tool to handle depth field. All changes are temporary — revert after investigation.

**Tech Stack:** C++ (emPainter.cpp), Rust (emPainter.rs), Python (diff_draw_ops.py)

---

### Task 0: Identify compound ops in each failing test

**Files:**
- Read: `crates/eaglemode/target/golden-divergence/painter_bezier_stroked.rust_ops.jsonl`
- Read: `crates/eaglemode/target/golden-divergence/widget_border_roundrect_thin.rust_ops.jsonl`
- Read: `crates/eaglemode/target/golden-divergence/widget_splitter_v_extreme_tall.rust_ops.jsonl`

- [ ] **Step 0.1: List the op types in each test's existing JSONL**

```bash
for f in crates/eaglemode/target/golden-divergence/painter_bezier_stroked.rust_ops.jsonl \
         crates/eaglemode/target/golden-divergence/widget_border_roundrect_thin.rust_ops.jsonl \
         crates/eaglemode/target/golden-divergence/widget_splitter_v_extreme_tall.rust_ops.jsonl; do
  echo "=== $(basename $f) ==="
  python3 -c "
import json, sys
ops = [json.loads(l) for l in open('$f') if l.strip().startswith('{')]
from collections import Counter
c = Counter(o['op'] for o in ops)
for op, n in c.most_common():
    print(f'  {op}: {n}')
"
done
```

Expected: lists of op types. Identify which are compound ops (PaintBezier, PaintBezierLine, PaintRoundRectOutline, PaintEllipse, etc.) — these are the ones whose sub-ops we need to capture.

- [ ] **Step 0.2: Record findings**

Write down which compound ops each test uses. This determines which Rust functions need vertex logging in Task 2.

---

### Task 1: Instrument C++ to log sub-ops with depth

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`

- [ ] **Step 1.1: Add depth field to all logging sites**

In `emPainter.cpp`, every logging site has this pattern:
```cpp
if (g_draw_op_log && g_draw_op_depth == 0) {
    fprintf(g_draw_op_log, "{\"seq\":%d,\"op\":\"PaintFoo\",...", g_draw_op_seq++, ...);
    // ...
    fprintf(g_draw_op_log, "}\n");
}
g_draw_op_depth++;
```

Change ALL 19 sites to:
```cpp
if (g_draw_op_log) {  // removed: && g_draw_op_depth == 0
    fprintf(g_draw_op_log, "{\"seq\":%d,\"depth\":%d,\"op\":\"PaintFoo\",...", g_draw_op_seq++, g_draw_op_depth, ...);
    // ...
    fprintf(g_draw_op_log, "}\n");
    fflush(g_draw_op_log);
}
g_draw_op_depth++;
```

The 19 sites are at these lines (approximate, search for `g_draw_op_depth == 0`):
385 (PaintRect), 464 (PaintPolygon), 1012 (PaintBezier), 1144 (PaintEllipse),
1197 (PaintEllipseSector), 1267 (PaintRoundRect), 1342 (PaintLine),
1424 (PaintPolyline), 1518 (PaintBezierLine), 1712 (PaintEllipseArc),
1828 (PaintRectOutline), 1910 (PaintEllipseOutline),
2008 (PaintEllipseSectorOutline), 2089 (PaintRoundRectOutline),
2230 (PaintBorderImage), 2350 (PaintBorderImageColored),
2474 (PaintText), 2577 (PaintTextBoxed), 3717 (PaintSolidPolyline)

For each: (a) remove `&& g_draw_op_depth == 0`, (b) add `"depth":%d` after `"seq":%d` in the format string, (c) add `g_draw_op_depth` as the corresponding printf argument.

- [ ] **Step 1.2: Rebuild gen_golden**

```bash
make -C crates/eaglemode/tests/golden/gen clean
make -C crates/eaglemode/tests/golden/gen
```

Expected: compiles successfully.

- [ ] **Step 1.3: Re-generate C++ ops for the 3 tests**

```bash
make -C crates/eaglemode/tests/golden/gen run
```

Expected: produces updated `.cpp_ops.jsonl` files in `target/golden-divergence/`.

- [ ] **Step 1.4: Verify sub-ops appear in C++ JSONL**

```bash
for test in painter_bezier_stroked widget_border_roundrect_thin widget_splitter_v_extreme_tall; do
  echo "=== $test ==="
  python3 -c "
import json
ops = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/${test}.cpp_ops.jsonl') if l.strip().startswith('{')]
depths = set(o.get('depth', 0) for o in ops)
print(f'  Depths seen: {sorted(depths)}')
print(f'  Total ops: {len(ops)}')
for o in ops:
    if o.get('depth', 0) > 0:
        print(f'  depth={o[\"depth\"]} op={o[\"op\"]}')
"
done
```

Expected: at least some ops with `depth > 0` (sub-ops from compound ops like PaintPolygon called from PaintBezier).

- [ ] **Step 1.5: Commit C++ instrumentation**

```bash
cd ~/git/eaglemode-0.96.4
git add -A && git commit -m "tmp: log sub-ops with depth field (investigation)"
```

---

### Task 2: Instrument Rust to log sub-ops with depth

**Files:**
- Modify: `crates/emcore/src/emPainter.rs`
- Modify: `crates/eaglemode/tests/golden/draw_op_dump.rs`

The Rust compound ops call `fill_polygon_aa` (internal) not `PaintPolygon`. So we need a thread-local logger that:
1. Logs at Paint* function entries (like C++)
2. Logs at `fill_polygon_aa` calls with the vertices (to match C++ PaintPolygon sub-ops)
3. Tracks depth

- [ ] **Step 2.1: Add thread-local logger infrastructure to emPainter.rs**

At the top of `crates/emcore/src/emPainter.rs`, add:

```rust
use std::cell::RefCell;

thread_local! {
    static DRAW_OP_LOG: RefCell<Option<DrawOpLogger>> = RefCell::new(None);
}

struct DrawOpLogger {
    lines: Vec<String>,
    seq: usize,
    depth: i32,
}

impl DrawOpLogger {
    fn new() -> Self {
        Self { lines: Vec::new(), seq: 0, depth: 0 }
    }

    fn log(&mut self, json_body: String) {
        let seq = self.seq;
        let depth = self.depth;
        self.lines.push(format!(r#"{{"seq":{seq},"depth":{depth},{json_body}}}"#));
        self.seq += 1;
    }

    fn enter(&mut self) {
        self.depth += 1;
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }
}

/// Activate the thread-local draw op logger. Returns previous state.
pub fn activate_draw_op_logger() {
    DRAW_OP_LOG.with(|log| {
        *log.borrow_mut() = Some(DrawOpLogger::new());
    });
}

/// Deactivate the logger and return all logged lines.
pub fn drain_draw_op_logger() -> Vec<String> {
    DRAW_OP_LOG.with(|log| {
        log.borrow_mut().take().map(|l| l.lines).unwrap_or_default()
    })
}
```

- [ ] **Step 2.2: Add logging macro for Paint* functions**

Add a helper macro in `emPainter.rs` that logs an op if the logger is active, then increments depth. Place it after the logger infrastructure:

```rust
/// Log a draw op if the thread-local logger is active. Call at Paint* function entry.
/// After calling, use `draw_op_leave()` before every return point.
fn draw_op_log_and_enter(json_body: String) {
    DRAW_OP_LOG.with(|log| {
        if let Some(ref mut logger) = *log.borrow_mut() {
            logger.log(json_body);
            logger.enter();
        }
    });
}

fn draw_op_leave() {
    DRAW_OP_LOG.with(|log| {
        if let Some(ref mut logger) = *log.borrow_mut() {
            logger.leave();
        }
    });
}
```

- [ ] **Step 2.3: Instrument compound ops that the 3 tests use**

Based on Task 0 findings, instrument the relevant Paint* functions. For each compound op, add `draw_op_log_and_enter(...)` after `try_record` and `draw_op_leave()` before each return. Also add a sub-op log before `fill_polygon_aa` calls to log the generated vertices.

**Example for PaintBezier (line ~2177):**

After the `try_record` guard:
```rust
pub fn PaintBezier(&mut self, points: &[(f64, f64)], color: emColor, canvas_color: emColor) {
    let Some(proof) = self.try_record(DrawOp::PaintBezier { ... }) else { return; };
    // ADD: log the top-level op
    draw_op_log_and_enter(format!(
        r#""op":"PaintBezier","n":{}"#, points.len()
    ));
    // ... tessellation code ...
    // Before fill_polygon_aa, ADD: log the sub-polygon vertices
    DRAW_OP_LOG.with(|log| {
        if let Some(ref mut logger) = *log.borrow_mut() {
            let verts_json = verts.iter()
                .map(|(x, y)| format!("[{x},{y}]"))
                .collect::<Vec<_>>().join(",");
            logger.log(format!(
                r#""op":"PaintPolygon","n":{},"vertices":[{verts_json}]"#,
                verts.len()
            ));
        }
    });
    self.fill_polygon_aa(proof, &verts, color, WindingRule::NonZero);
    // ADD: leave depth
    draw_op_leave();
}
```

Do the same pattern for every compound op identified in Task 0. Key compound ops to expect:
- `PaintBezier` / `PaintBezierLine` — tessellates bezier, calls `fill_polygon_aa`
- `PaintRoundRect` / `PaintRoundRectOutline` — generates round rect polygon, calls `fill_polygon_aa` or `PaintPolygonOutline`
- `PaintEllipse` / `PaintEllipseOutline` — generates ellipse polygon, calls `fill_polygon_aa`
- `PaintPolyline` / `PaintPolylineWithArrows` — generates stroke quads, calls `PaintPolygon`
- `PaintLine` (stroked) / `paint_line_stroked` — calls `PaintPolylineWithArrows`

For ops that call other Paint* functions as sub-ops (like PaintPolyline → PaintPolygon), the sub-op will log itself at depth+1 automatically. No extra vertex logging needed.

For ops that call `fill_polygon_aa` directly, add explicit vertex logging before the call (like the PaintBezier example above).

- [ ] **Step 2.4: Verify it compiles**

```bash
cargo check
```

Expected: compiles without errors.

---

### Task 3: Wire logger into golden tests

**Files:**
- Modify: `crates/eaglemode/tests/golden/draw_op_dump.rs`
- Modify: `crates/eaglemode/tests/golden/painter.rs`
- Modify: `crates/eaglemode/tests/golden/widget.rs`

**Key insight:** The existing `record_painter_ops` / `maybe_record_draw_ops` functions create a SEPARATE painter in DrawList mode, duplicating paint calls. Sub-ops never execute in DrawList mode. Instead, we activate the thread-local logger around the ACTUAL rendering block (which runs in direct mode), so sub-ops fire and get logged.

- [ ] **Step 3.1: Add logger dump function to draw_op_dump.rs**

```rust
use emcore::emPainter::{activate_draw_op_logger, drain_draw_op_logger};

/// Dump sub-op-inclusive draw ops recorded via thread-local logger.
/// Call activate_draw_op_logger() BEFORE paint, then this AFTER paint.
pub fn dump_subop_log(name: &str) {
    let lines = drain_draw_op_logger();
    if lines.is_empty() {
        return;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("golden-divergence")
        .join(format!("{name}.rust_ops.jsonl"));
    let _ = std::fs::create_dir_all(path.parent().expect("path has parent"));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open rust_ops.jsonl");
    for line in &lines {
        writeln!(f, "{line}").expect("write line");
    }
}
```

- [ ] **Step 3.2: Wrap actual rendering with logger in painter_bezier_stroked**

In `crates/eaglemode/tests/golden/painter.rs`, in the `painter_bezier_stroked` test, wrap the existing direct-mode paint block with the logger. Do NOT use `record_painter_ops`:

```rust
#[test]
fn painter_bezier_stroked() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("bezier_stroked");
    let mut img = white_canvas(ew, eh);
    // Activate logger BEFORE actual rendering
    if dump_draw_ops_enabled() {
        activate_draw_op_logger();
    }
    {
        let mut p = white_painter(&mut img);
        let mut stroke = emStroke::new(emColor::BLACK, 3.0);
        stroke.cap = LineCap::Round;
        stroke.join = LineJoin::Round;
        stroke.start_end = emStrokeEnd::new(StrokeEndType::Arrow).with_inner_color(emColor::WHITE);
        stroke.finish_end = emStrokeEnd::new(StrokeEndType::Arrow).with_inner_color(emColor::WHITE);
        p.PaintBezierLine(&bezier_points(), &stroke, emColor::TRANSPARENT);
    }
    // Dump logger output AFTER rendering
    if dump_draw_ops_enabled() {
        dump_subop_log("bezier_stroked");
    }
    compare_images("bezier_stroked", img.GetMap(), &expected, ew, eh, 0, 0.0).unwrap();
}
```

Remove the old `record_painter_ops("bezier_stroked", ...)` call.

- [ ] **Step 3.3: Wrap actual rendering with logger in widget tests**

In `crates/eaglemode/tests/golden/widget.rs`, do the same for `widget_border_roundrect_thin` and `widget_splitter_v_extreme_tall`. Find the actual rendering block (the `SoftwareCompositor::render` or `view.Paint` call) and wrap it with `activate_draw_op_logger()` / `dump_subop_log()`.

Remove the old `maybe_record_draw_ops(...)` calls for these two tests.

- [ ] **Step 3.4: Verify it compiles**

```bash
cargo check
```

Expected: compiles without errors.

- [ ] **Step 3.5: Re-record Rust ops for all 3 tests**

```bash
DUMP_DRAW_OPS=1 cargo test --test golden painter_bezier_stroked -- --test-threads=1
DUMP_DRAW_OPS=1 cargo test --test golden widget_border_roundrect_thin -- --test-threads=1
DUMP_DRAW_OPS=1 cargo test --test golden widget_splitter_v_extreme_tall -- --test-threads=1
```

- [ ] **Step 3.6: Verify sub-ops appear in Rust JSONL**

```bash
for test in painter_bezier_stroked widget_border_roundrect_thin widget_splitter_v_extreme_tall; do
  echo "=== $test ==="
  python3 -c "
import json
ops = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/${test}.rust_ops.jsonl') if l.strip().startswith('{')]
depths = set(o.get('depth', 0) for o in ops)
print(f'  Depths seen: {sorted(depths)}')
print(f'  Total ops: {len(ops)}')
for o in ops:
    if o.get('depth', 0) > 0:
        print(f'  depth={o[\"depth\"]} op={o[\"op\"]}')
"
done
```

Expected: sub-ops at depth > 0 matching the C++ output pattern.

---

### Task 4: Update diff tool and run diffs

**Files:**
- Modify: `scripts/diff_draw_ops.py`

- [ ] **Step 4.1: Update diff tool to handle depth field**

In `scripts/diff_draw_ops.py`:

1. Don't add `"depth"` to `SKIP_KEYS` — we want it compared.
2. The LCS alignment already uses `o.get("op", "?")` for matching. With sub-ops, both sides will have more ops. The alignment should still work since it's based on op name sequences.
3. Add depth to the output table so mismatches show which depth level they're at.

Modify the `diff_ops` function to include depth in the report header:

```python
# In the output formatting, add depth column:
# seq  depth  op              param          C++              Rust             delta
```

Also add a summary line counting ops at each depth level for both sides.

- [ ] **Step 4.2: Run diff on all 3 tests**

```bash
python3 scripts/diff_draw_ops.py painter_bezier_stroked
python3 scripts/diff_draw_ops.py widget_border_roundrect_thin
python3 scripts/diff_draw_ops.py widget_splitter_v_extreme_tall
```

Expected: either sub-op mismatches appear (vertex differences, extra/missing sub-ops) or all sub-ops match.

- [ ] **Step 4.3: Record results**

For each test, document:
- Total ops at each depth level (C++ vs Rust)
- Any parameter mismatches at depth > 0
- Any unmatched ops (C++ ONLY or RUST ONLY) at any depth
- Conclusion: hypothesis A (sub-op divergence) confirmed or eliminated

---

### Task 5: Interpret results and clean up

- [ ] **Step 5.1: Summarize findings**

If sub-op mismatches found: identify which compound op's decomposition differs and what parameters diverge. This is the root cause — file a follow-up task to fix the compound op.

If sub-ops all match: hypothesis A eliminated. The divergence is in test setup (hypothesis B) or something else (hypothesis C). Next investigation: compare initial image state and painter initialization between C++ and Rust tests.

- [ ] **Step 5.2: Revert Rust instrumentation**

```bash
cd /home/a0/git/eaglemode-rs
git checkout -- crates/emcore/src/emPainter.rs crates/eaglemode/tests/golden/draw_op_dump.rs crates/eaglemode/tests/golden/painter.rs crates/eaglemode/tests/golden/widget.rs
```

- [ ] **Step 5.3: Revert C++ instrumentation**

```bash
cd ~/git/eaglemode-0.96.4
git checkout -- src/emCore/emPainter.cpp
make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen clean
make -C /home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/gen
```
