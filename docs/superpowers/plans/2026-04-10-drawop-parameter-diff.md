# DrawOp Parameter Diff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a mechanical diagnostic tool that compares paint-call parameters between C++ and Rust widget rendering, op-by-op, to find widget-level parameter divergence causing 33 golden test failures.

**Architecture:** Rust DrawOp recording (already exists) serializes to JSONL. C++ emPainter gets conditional fprintf logging to JSONL. A Python script diffs the two files parameter-by-parameter. First target: cosmos_item_border.

**Tech Stack:** Rust (golden test infrastructure), C++ (emPainter logging), Python (diff script)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/eaglemode/tests/golden/draw_op_dump.rs` | Create | DrawOp → JSONL serialization + dump helpers |
| `crates/eaglemode/tests/golden/cosmos_items.rs` | Modify | Add recording pass when `DUMP_DRAW_OPS=1` |
| `crates/eaglemode/tests/golden/test_panel.rs` | Modify | Add recording pass when `DUMP_DRAW_OPS=1` |
| `crates/eaglemode/tests/golden/main.rs` | Modify | Add `mod draw_op_dump;` |
| `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp` | Modify | Add conditional JSONL logging guarded by global |
| `crates/eaglemode/tests/golden/gen/gen_golden.cpp` | Modify | Set up logging file before rendering |
| `scripts/diff_draw_ops.py` | Create | JSONL diff script |

---

### Task 1: Rust DrawOp JSONL Serializer

**Files:**
- Create: `crates/eaglemode/tests/golden/draw_op_dump.rs`
- Modify: `crates/eaglemode/tests/golden/main.rs`

- [ ] **Step 1: Create draw_op_dump.rs with the serialization function**

```rust
// crates/eaglemode/tests/golden/draw_op_dump.rs

use std::io::Write;
use std::path::PathBuf;

use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPainterDrawList::DrawOp;

/// Returns true if DUMP_DRAW_OPS=1 env var is set.
pub fn dump_draw_ops_enabled() -> bool {
    std::env::var("DUMP_DRAW_OPS").map_or(false, |v| v == "1")
}

/// Directory for draw op dumps.
fn dump_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target")
        .join("golden-divergence");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Serialize a color as 8-hex-digit lowercase RGBA string.
fn color_hex(c: emColor) -> String {
    format!("{:08x}", c.GetPacked())
}

/// Serialize image metadata (dimensions only — pointer not comparable cross-language).
fn img_meta(ptr: *const emImage) -> (u32, u32, u32) {
    if ptr.is_null() {
        return (0, 0, 0);
    }
    let img = unsafe { &*ptr };
    (img.GetWidth(), img.GetHeight(), img.GetChannelCount() as u32)
}

/// Dump a DrawOp sequence to `target/golden-divergence/{name}.rust_ops.jsonl`.
pub fn dump_draw_ops(name: &str, ops: &[DrawOp]) {
    let path = dump_dir().join(format!("{name}.rust_ops.jsonl"));
    let mut f = std::fs::File::create(&path)
        .unwrap_or_else(|e| panic!("Cannot create {}: {e}", path.display()));

    for (seq, op) in ops.iter().enumerate() {
        let line = serialize_op(seq, op);
        writeln!(f, "{line}").unwrap();
    }
    eprintln!("  DrawOps dumped: {} ({} ops)", path.display(), ops.len());
}

fn serialize_op(seq: usize, op: &DrawOp) -> String {
    match op {
        // State ops
        DrawOp::PushState => {
            format!(r#"{{"seq":{seq},"op":"PushState"}}"#)
        }
        DrawOp::PopState => {
            format!(r#"{{"seq":{seq},"op":"PopState"}}"#)
        }
        DrawOp::SetOffset(dx, dy) => {
            format!(r#"{{"seq":{seq},"op":"SetOffset","dx":{dx},"dy":{dy}}}"#)
        }
        DrawOp::ClipRect { x, y, w, h } => {
            format!(r#"{{"seq":{seq},"op":"ClipRect","x":{x},"y":{y},"w":{w},"h":{h}}}"#)
        }
        DrawOp::SetCanvasColor(c) => {
            format!(r#"{{"seq":{seq},"op":"SetCanvasColor","color":"{}"}}"#, color_hex(*c))
        }
        DrawOp::SetAlpha(a) => {
            format!(r#"{{"seq":{seq},"op":"SetAlpha","alpha":{a}}}"#)
        }

        // Shapes
        DrawOp::PaintRect { x, y, w, h, color, canvas_color } => {
            format!(
                r#"{{"seq":{seq},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"{}","canvas_color":"{}"}}"#,
                color_hex(*color), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintRoundRect { x, y, w, h, radius, color, canvas_color } => {
            format!(
                r#"{{"seq":{seq},"op":"PaintRoundRect","x":{x},"y":{y},"w":{w},"h":{h},"radius":{radius},"color":"{}","canvas_color":"{}"}}"#,
                color_hex(*color), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintEllipse { cx, cy, rx, ry, color, canvas_color } => {
            format!(
                r#"{{"seq":{seq},"op":"PaintEllipse","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color":"{}","canvas_color":"{}"}}"#,
                color_hex(*color), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintPolygon { vertices, color, canvas_color } => {
            let verts = serialize_vertices(vertices);
            format!(
                r#"{{"seq":{seq},"op":"PaintPolygon","vertices":{verts},"color":"{}","canvas_color":"{}"}}"#,
                color_hex(*color), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintSolidPolyline { vertices, stroke, closed, canvas_color } => {
            let verts = serialize_vertices(vertices);
            format!(
                r#"{{"seq":{seq},"op":"PaintSolidPolyline","vertices":{verts},"stroke_color":"{}","stroke_width":{},"closed":{closed},"canvas_color":"{}"}}"#,
                color_hex(stroke.color), stroke.width, color_hex(*canvas_color)
            )
        }

        // Images
        DrawOp::PaintImageFull { x, y, w, h, image_ptr, alpha, canvas_color } => {
            let (iw, ih, ic) = img_meta(*image_ptr);
            format!(
                r#"{{"seq":{seq},"op":"PaintImageFull","x":{x},"y":{y},"w":{w},"h":{h},"img_w":{iw},"img_h":{ih},"img_ch":{ic},"alpha":{alpha},"canvas_color":"{}"}}"#,
                color_hex(*canvas_color)
            )
        }
        DrawOp::PaintImageColored { x, y, w, h, image_ptr, src_x, src_y, src_w, src_h, color1, color2, canvas_color, extension } => {
            let (iw, ih, ic) = img_meta(*image_ptr);
            format!(
                r#"{{"seq":{seq},"op":"PaintImageColored","x":{x},"y":{y},"w":{w},"h":{h},"img_w":{iw},"img_h":{ih},"img_ch":{ic},"src_x":{src_x},"src_y":{src_y},"src_w":{src_w},"src_h":{src_h},"color1":"{}","color2":"{}","canvas_color":"{}","extension":"{extension:?}"}}"#,
                color_hex(*color1), color_hex(*color2), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintBorderImage { x, y, w, h, l, t, r, b, image_ptr, src_l, src_t, src_r, src_b, alpha, canvas_color, which_sub_rects } => {
            let (iw, ih, ic) = img_meta(*image_ptr);
            format!(
                r#"{{"seq":{seq},"op":"PaintBorderImage","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{iw},"img_h":{ih},"img_ch":{ic},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"alpha":{alpha},"canvas_color":"{}","which_sub_rects":{which_sub_rects}}}"#,
                color_hex(*canvas_color)
            )
        }
        DrawOp::PaintBorderImageColored { x, y, w, h, l, t, r, b, image_ptr, src_l, src_t, src_r, src_b, color1, color2, canvas_color, which_sub_rects, alpha } => {
            let (iw, ih, ic) = img_meta(*image_ptr);
            format!(
                r#"{{"seq":{seq},"op":"PaintBorderImageColored","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{iw},"img_h":{ih},"img_ch":{ic},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"color1":"{}","color2":"{}","canvas_color":"{}","which_sub_rects":{which_sub_rects},"alpha":{alpha}}}"#,
                color_hex(*color1), color_hex(*color2), color_hex(*canvas_color)
            )
        }

        // Text
        DrawOp::PaintText { x, y, text, char_height, width_scale, color, canvas_color } => {
            let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                r#"{{"seq":{seq},"op":"PaintText","x":{x},"y":{y},"text":"{escaped}","char_height":{char_height},"width_scale":{width_scale},"color":"{}","canvas_color":"{}"}}"#,
                color_hex(*color), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintTextBoxed { x, y, w, h, text, max_char_height, color, canvas_color, .. } => {
            let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                r#"{{"seq":{seq},"op":"PaintTextBoxed","x":{x},"y":{y},"w":{w},"h":{h},"text":"{escaped}","max_char_height":{max_char_height},"color":"{}","canvas_color":"{}"}}"#,
                color_hex(*color), color_hex(*canvas_color)
            )
        }

        // Gradients
        DrawOp::PaintLinearGradient { x, y, w, h, color_a, color_b, horizontal, canvas_color } => {
            format!(
                r#"{{"seq":{seq},"op":"PaintLinearGradient","x":{x},"y":{y},"w":{w},"h":{h},"color_a":"{}","color_b":"{}","horizontal":{horizontal},"canvas_color":"{}"}}"#,
                color_hex(*color_a), color_hex(*color_b), color_hex(*canvas_color)
            )
        }
        DrawOp::PaintRadialGradient { cx, cy, rx, ry, color_inner, color_outer, canvas_color } => {
            format!(
                r#"{{"seq":{seq},"op":"PaintRadialGradient","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color_inner":"{}","color_outer":"{}","canvas_color":"{}"}}"#,
                color_hex(*color_inner), color_hex(*color_outer), color_hex(*canvas_color)
            )
        }

        // Catch-all for ops not yet serialized — log op name and seq only.
        other => {
            let name = format!("{other:?}");
            let op_name = name.split_once(|c: char| c == ' ' || c == '{' || c == '(')
                .map(|(n, _)| n)
                .unwrap_or(&name);
            format!(r#"{{"seq":{seq},"op":"{op_name}","_unserialized":true}}"#)
        }
    }
}

fn serialize_vertices(verts: &[(f64, f64)]) -> String {
    let inner: Vec<String> = verts.iter().map(|(x, y)| format!("[{x},{y}]")).collect();
    format!("[{}]", inner.join(","))
}
```

- [ ] **Step 2: Add module declaration to main.rs**

Find the module declarations in `crates/eaglemode/tests/golden/main.rs` and add:

```rust
mod draw_op_dump;
```

- [ ] **Step 3: Verify it compiles**

Run:
```bash
cargo test --test golden -- --list 2>&1 | head -5
```
Expected: no compilation errors (the module is unused so far, but should compile).

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/golden/draw_op_dump.rs crates/eaglemode/tests/golden/main.rs
git commit -m "feat(golden): add DrawOp JSONL serialization for parameter diff diagnosis"
```

---

### Task 2: Record and Dump DrawOps for cosmos_item_border

**Files:**
- Modify: `crates/eaglemode/tests/golden/cosmos_items.rs`

The `cosmos_item_border` test uses a direct painter (`emPainter::new(&mut img)`). To record DrawOps, add a second paint pass using `emPainter::new_recording()` when `DUMP_DRAW_OPS=1`.

- [ ] **Step 1: Add recording pass to cosmos_item_border**

After the existing paint block (line 74), add a recording block:

```rust
// crates/eaglemode/tests/golden/cosmos_items.rs
// Add import at top:
use super::draw_op_dump::{dump_draw_ops, dump_draw_ops_enabled};

// After the existing paint block (after line 74), add:
    if dump_draw_ops_enabled() {
        let mut ops = Vec::new();
        {
            let mut rec = emPainter::new_recording(400, 300, &mut ops);
            rec.SetCanvasColor(emColor::TRANSPARENT);
            rec.scale(sx, sy);
            panel.Paint(&mut rec, 1.0, panel_h, &state);
        }
        dump_draw_ops("cosmos_item_border", &ops);
    }
```

Note: `sx`, `sy`, `state` need to be accessible. Move variable declarations before the first paint block so they're in scope for both. The full function becomes:

```rust
#[test]
fn cosmos_item_border() {
    require_golden!();

    let (ew, eh, expected) = load_painter_golden("cosmos_item_border");
    assert_eq!(ew, 400);
    assert_eq!(eh, 300);

    let ctx = emcore::emContext::emContext::NewRoot();
    let mut panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
    let rec = test_item_rec();

    let b_val = rec.ContentTallness.min(1.0) * rec.BorderScaling;
    let bt = b_val * 0.05;
    let bb = b_val * 0.03;
    let panel_h = rec.ContentTallness + bt + bb;

    panel.SetItemRec(rec);

    let mut img = emImage::new(400, 300, 4);
    img.fill(emColor::BLACK);

    let sx = 400.0;
    let sy = 300.0 / panel_h;
    let state = PanelState::default_for_test();

    {
        let mut p = emPainter::new(&mut img);
        p.SetCanvasColor(emColor::TRANSPARENT);
        p.scale(sx, sy);
        panel.Paint(&mut p, 1.0, panel_h, &state);
    }

    if dump_draw_ops_enabled() {
        let mut ops = Vec::new();
        {
            let mut rec = emPainter::new_recording(400, 300, &mut ops);
            rec.SetCanvasColor(emColor::TRANSPARENT);
            rec.scale(sx, sy);
            panel.Paint(&mut rec, 1.0, panel_h, &state);
        }
        dump_draw_ops("cosmos_item_border", &ops);
    }

    compare_images("cosmos_item_border", img.GetMap(), &expected, ew, eh, 0, 0.0)
        .expect("cosmos_item_border golden mismatch");
}
```

- [ ] **Step 2: Add import for emPainter::new_recording**

`emPainter::new_recording` is `pub(crate)`. Since the test is in the `eaglemode` crate's test directory, it should have access. Verify that `emcore::emPainter::emPainter` re-exports or that the test can call it. If `new_recording` is only `pub(crate)` on `emcore`, the test (in the `eaglemode` crate) won't have access. In that case, add a public wrapper in `emcore`:

```rust
// In crates/emcore/src/emPainter.rs, add:
pub fn new_recording_public(width: u32, height: u32, ops: &'a mut Vec<DrawOp>) -> Self {
    Self::new_recording(width, height, ops)
}
```

Or change `new_recording` visibility to `pub`. Check the actual visibility situation before deciding.

- [ ] **Step 3: Test the recording**

Run:
```bash
DUMP_DRAW_OPS=1 cargo test --test golden cosmos_item_border -- --test-threads=1
```
Expected: Test runs (may pass or fail as before), and `crates/eaglemode/target/golden-divergence/cosmos_item_border.rust_ops.jsonl` is created with JSONL content.

Verify content:
```bash
head -5 crates/eaglemode/target/golden-divergence/cosmos_item_border.rust_ops.jsonl
```
Expected: JSON lines with `seq`, `op`, and parameters.

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/golden/cosmos_items.rs
git commit -m "feat(golden): record DrawOps for cosmos_item_border when DUMP_DRAW_OPS=1"
```

---

### Task 3: Record and Dump DrawOps for Compositor Tests (testpanel, tktest)

**Files:**
- Modify: `crates/eaglemode/tests/golden/test_panel.rs`

The compositor tests use `SoftwareCompositor::render()` which calls `view.Paint(tree, &mut painter)`. Add a recording pass using `emPainter::new_recording()`.

- [ ] **Step 1: Add a recording helper function**

In `test_panel.rs`, add a helper that records DrawOps for any compositor-rendered test:

```rust
use super::draw_op_dump::{dump_draw_ops, dump_draw_ops_enabled};

/// Record DrawOps for a compositor test if DUMP_DRAW_OPS=1.
fn maybe_record_draw_ops(name: &str, tree: &mut PanelTree, view: &emView, w: u32, h: u32) {
    if !dump_draw_ops_enabled() {
        return;
    }
    let mut ops = Vec::new();
    {
        let mut rec = emPainter::new_recording(w, h, &mut ops);
        view.Paint(tree, &mut rec);
    }
    dump_draw_ops(name, &ops);
}
```

- [ ] **Step 2: Add recording call to render_testpanel**

In the `render_testpanel` function (line ~2068), after `settle()` and before `compositor.render()`, add:

```rust
    maybe_record_draw_ops(name, tree, view, w, h);
```

The full function becomes:

```rust
fn render_testpanel(
    name: &str,
    tree: &mut PanelTree,
    view: &mut emView,
    expected: &(u32, u32, Vec<u8>),
    channel_tolerance: u8,
    max_failure_pct: f64,
    settle_rounds: usize,
) {
    let (w, h, ref expected_data) = *expected;

    settle(tree, view, settle_rounds);

    maybe_record_draw_ops(name, tree, view, w, h);

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(tree, view);
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images(
        name,
        actual,
        expected_data,
        w,
        h,
        channel_tolerance,
        max_failure_pct,
    );
    if result.is_err() && dump_golden_enabled() {
        dump_test_images(name, actual, expected_data, w, h);
        analyze_diff_distribution(actual, expected_data, w, h, channel_tolerance);
    }
    result.unwrap();
}
```

- [ ] **Step 3: Test with testpanel_root**

Run:
```bash
DUMP_DRAW_OPS=1 cargo test --test golden testpanel_root -- --test-threads=1
```
Expected: `crates/eaglemode/target/golden-divergence/testpanel_root.rust_ops.jsonl` is created.

```bash
wc -l crates/eaglemode/target/golden-divergence/testpanel_root.rust_ops.jsonl
```
Expected: Some reasonable number of ops (likely 10-100+).

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/golden/test_panel.rs
git commit -m "feat(golden): record DrawOps for compositor tests when DUMP_DRAW_OPS=1"
```

---

### Task 4: C++ emPainter Conditional Logging

**Files:**
- Modify: `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp`

Add a global log file pointer and conditional JSONL logging at the top of each paint method.

- [ ] **Step 1: Add global log state near top of emPainter.cpp**

After the existing includes (near line 20), add:

```cpp
// ── DrawOp logging for golden test parameter diff ──
#include <cstdio>
FILE* g_draw_op_log = nullptr;
int g_draw_op_seq = 0;

static void log_color(FILE* f, const char* name, emColor c) {
    fprintf(f, ",\"%s\":\"%02x%02x%02x%02x\"", name,
        c.GetRed(), c.GetGreen(), c.GetBlue(), c.GetAlpha());
}
```

- [ ] **Step 2: Add logging to PaintRect (line ~334)**

At the very top of `PaintRect`, before the `x=x*ScaleX+OriginX` line, add:

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log, "{\"seq\":%d,\"op\":\"PaintRect\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g",
            g_draw_op_seq++, x, y, w, h);
        // texture is typically a solid color for golden tests
        log_color(g_draw_op_log, "color", texture.IsColor() ? texture.GetColor() : emColor(0));
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

Note: The C++ `PaintRect` takes `const emTexture& texture`, not `emColor color`. The `emTexture` can be a solid color, image, or gradient. For diagnostic logging, log `texture.GetColor()` when `texture.IsColor()`, otherwise log a marker. Check the emTexture API:

```cpp
// In emPainter.h or emTexture.h — verify these methods exist:
// bool emTexture::IsColor() const;
// emColor emTexture::GetColor() const;
```

If `emTexture` doesn't have `IsColor()`/`GetColor()`, cast the texture to color when it's a single-color case (most common in golden tests). The `emTexture` type is actually `emColor` in most uses since `emColor` implicitly converts to `emTexture`. Check the constructor:

```cpp
// emTexture is likely just an emColor alias or has an implicit conversion.
// If emTexture IS emColor (common in Eagle Mode):
log_color(g_draw_op_log, "color", (emColor)texture);
```

- [ ] **Step 3: Add logging to PaintRoundRect (line ~1127)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log, "{\"seq\":%d,\"op\":\"PaintRoundRect\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,\"radius\":%.17g",
            g_draw_op_seq++, x, y, w, h, r);
        log_color(g_draw_op_log, "color", (emColor)texture);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

- [ ] **Step 4: Add logging to PaintBorderImage (line ~1892)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log,
            "{\"seq\":%d,\"op\":\"PaintBorderImage\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
            "\"l\":%.17g,\"t\":%.17g,\"r\":%.17g,\"b\":%.17g,"
            "\"img_w\":%d,\"img_h\":%d,\"img_ch\":%d,"
            "\"src_l\":%d,\"src_t\":%d,\"src_r\":%d,\"src_b\":%d,"
            "\"alpha\":%d",
            g_draw_op_seq++, x, y, w, h, l, t, r, b,
            img.GetWidth(), img.GetHeight(), img.GetChannelCount(),
            srcL, srcT, srcR, srcB, alpha);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, ",\"which_sub_rects\":%d}\n", whichSubRects);
    }
```

Note: The C++ PaintBorderImage has additional `srcX, srcY, srcW, srcH` parameters. Log them too:

```cpp
        fprintf(g_draw_op_log,
            "{\"seq\":%d,\"op\":\"PaintBorderImage\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
            "\"l\":%.17g,\"t\":%.17g,\"r\":%.17g,\"b\":%.17g,"
            "\"img_w\":%d,\"img_h\":%d,\"img_ch\":%d,"
            "\"src_x\":%d,\"src_y\":%d,\"src_w\":%d,\"src_h\":%d,"
            "\"src_l\":%d,\"src_t\":%d,\"src_r\":%d,\"src_b\":%d,"
            "\"alpha\":%d",
            g_draw_op_seq++, x, y, w, h, l, t, r, b,
            img.GetWidth(), img.GetHeight(), img.GetChannelCount(),
            srcX, srcY, srcW, srcH,
            srcL, srcT, srcR, srcB, alpha);
```

- [ ] **Step 5: Add logging to PaintBorderImageColored (line ~1985)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log,
            "{\"seq\":%d,\"op\":\"PaintBorderImageColored\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
            "\"l\":%.17g,\"t\":%.17g,\"r\":%.17g,\"b\":%.17g,"
            "\"img_w\":%d,\"img_h\":%d,\"img_ch\":%d,"
            "\"src_l\":%d,\"src_t\":%d,\"src_r\":%d,\"src_b\":%d",
            g_draw_op_seq++, x, y, w, h, l, t, r, b,
            img.GetWidth(), img.GetHeight(), img.GetChannelCount(),
            srcL, srcT, srcR, srcB);
        log_color(g_draw_op_log, "color1", color1);
        log_color(g_draw_op_log, "color2", color2);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, ",\"which_sub_rects\":%d,\"alpha\":%d}\n", whichSubRects, alpha);
    }
```

- [ ] **Step 6: Add logging to PaintText (line ~2078)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log,
            "{\"seq\":%d,\"op\":\"PaintText\",\"x\":%.17g,\"y\":%.17g,\"text\":\"%s\","
            "\"char_height\":%.17g,\"width_scale\":%.17g",
            g_draw_op_seq++, x, y, text ? text : "", charHeight, widthScale);
        log_color(g_draw_op_log, "color", color);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

- [ ] **Step 7: Add logging to PaintTextBoxed (line ~2158)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log,
            "{\"seq\":%d,\"op\":\"PaintTextBoxed\",\"x\":%.17g,\"y\":%.17g,\"w\":%.17g,\"h\":%.17g,"
            "\"text\":\"%s\",\"max_char_height\":%.17g",
            g_draw_op_seq++, x, y, w, h, text ? text : "", maxCharHeight);
        log_color(g_draw_op_log, "color", color);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

- [ ] **Step 8: Add logging to PaintEllipse (line ~1044)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log,
            "{\"seq\":%d,\"op\":\"PaintEllipse\",\"cx\":%.17g,\"cy\":%.17g,\"rx\":%.17g,\"ry\":%.17g",
            g_draw_op_seq++, cx, cy, rx, ry);
        log_color(g_draw_op_log, "color", (emColor)texture);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

- [ ] **Step 9: Add logging to PaintPolygon (line ~400)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log, "{\"seq\":%d,\"op\":\"PaintPolygon\",\"n\":%d,\"vertices\":[",
            g_draw_op_seq++, n);
        for (int i = 0; i < n; i++) {
            if (i > 0) fprintf(g_draw_op_log, ",");
            fprintf(g_draw_op_log, "[%.17g,%.17g]", xy[i*2], xy[i*2+1]);
        }
        fprintf(g_draw_op_log, "]");
        log_color(g_draw_op_log, "color", (emColor)texture);
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

- [ ] **Step 10: Add logging to PaintSolidPolyline (line ~3280)**

```cpp
    if (g_draw_op_log) {
        fprintf(g_draw_op_log, "{\"seq\":%d,\"op\":\"PaintSolidPolyline\",\"n\":%d,\"vertices\":[",
            g_draw_op_seq++, n);
        for (int i = 0; i < n; i++) {
            if (i > 0) fprintf(g_draw_op_log, ",");
            fprintf(g_draw_op_log, "[%.17g,%.17g]", xy[i*2], xy[i*2+1]);
        }
        fprintf(g_draw_op_log, "],\"stroke_color\":\"%02x%02x%02x%02x\",\"stroke_width\":%.17g,\"closed\":%s",
            stroke.Color.GetRed(), stroke.Color.GetGreen(),
            stroke.Color.GetBlue(), stroke.Color.GetAlpha(),
            stroke.Width,
            closed ? "true" : "false");
        log_color(g_draw_op_log, "canvas_color", canvasColor);
        fprintf(g_draw_op_log, "}\n");
    }
```

- [ ] **Step 11: Add logging to PaintImage (the internal one called by PaintBorderImage)**

The PaintBorderImage delegates to individual PaintImage calls. These are already captured by the PaintBorderImage log entry at the API boundary, which is what we want — we're comparing widget-level calls, not internal decomposition. Skip instrumenting PaintImage unless needed later.

- [ ] **Step 12: Rebuild C++ Eagle Mode library**

```bash
cd ~/git/eaglemode-0.96.4 && make
```
Expected: Compiles successfully with the added fprintf calls.

- [ ] **Step 13: Commit the C++ changes**

Note: The C++ source is in a separate repo. Document what was changed but don't commit to the eaglemode-rs repo. Instead, leave a note:

```
# Changes made to ~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp:
# - Added global FILE* g_draw_op_log and int g_draw_op_seq
# - Added conditional JSONL logging to PaintRect, PaintRoundRect, PaintBorderImage,
#   PaintBorderImageColored, PaintText, PaintTextBoxed, PaintEllipse, PaintPolygon,
#   PaintSolidPolyline
```

---

### Task 5: Wire Up C++ Golden Generator Logging

**Files:**
- Modify: `crates/eaglemode/tests/golden/gen/gen_golden.cpp`

- [ ] **Step 1: Add extern declaration and log setup helper**

Near the top of `gen_golden.cpp`, after includes:

```cpp
// ── DrawOp logging ──
extern FILE* g_draw_op_log;
extern int g_draw_op_seq;

static FILE* open_draw_op_log(const char* name) {
    char path[512];
    snprintf(path, sizeof(path),
        "%s/crates/eaglemode/target/golden-divergence/%s.cpp_ops.jsonl",
        getenv("CARGO_MANIFEST_DIR") ? getenv("CARGO_MANIFEST_DIR") : ".",
        name);
    FILE* f = fopen(path, "w");
    if (!f) {
        // Fallback: use relative path
        snprintf(path, sizeof(path), "target/golden-divergence/%s.cpp_ops.jsonl", name);
        f = fopen(path, "w");
    }
    if (f) {
        g_draw_op_log = f;
        g_draw_op_seq = 0;
    }
    return f;
}

static void close_draw_op_log() {
    if (g_draw_op_log) {
        fclose(g_draw_op_log);
        g_draw_op_log = nullptr;
        g_draw_op_seq = 0;
    }
}
```

- [ ] **Step 2: Add logging to gen_cosmos_item_border**

Wrap the paint calls in `gen_cosmos_item_border()` (line ~5121) with log setup:

```cpp
static void gen_cosmos_item_border() {
    // ... existing setup code unchanged ...

    FILE* log = open_draw_op_log("cosmos_item_border");

    // Top border strip
    p.PaintRect(0.0, 0.0, w, bt * h, borderColor);
    // Bottom border strip
    p.PaintRect(0.0, (1.0 - bb) * h, w, bb * h, borderColor);
    // Left border strip
    p.PaintRect(0.0, bt * h, bl * w, (1.0 - bt - bb) * h, borderColor);
    // Right border strip
    p.PaintRect((1.0 - br) * w, bt * h, br * w, (1.0 - bt - bb) * h, borderColor);
    // Background
    p.PaintRect(bl * w, bt * h, (1.0 - bl - br) * w, (1.0 - bt - bb) * h, bgColor);
    // Title text
    double fontH = bt * h * 0.7;
    if (fontH >= 1.0) {
        p.PaintText(bl * w, bt * h * 0.15, title, fontH, 1.0, titleColor);
    }

    close_draw_op_log();

    dump_painter("cosmos_item_border", img);
}
```

- [ ] **Step 3: Add logging to render_and_dump_sized for compositor tests**

In `render_and_dump_sized()` (line ~4199), wrap the `DoPaintView` call:

```cpp
static void render_and_dump_sized(const char* name, GoldenViewPort& vp,
                                   emRootContext& ctx, int w, int h) {
    emImage img(w, h, 4);
    emPainter p;
    if (!img.PreparePainter(&p, ctx, 0.0, 0.0, (double)w, (double)h)) {
        fprintf(stderr, "PreparePainter failed for %s\n", name);
        exit(1);
    }

    FILE* log = open_draw_op_log(name);
    vp.DoPaintView(p, 0);
    close_draw_op_log();

    dump_compositor(name, img);
}
```

- [ ] **Step 4: Rebuild golden generator**

```bash
make -C crates/eaglemode/tests/golden/gen
```
Expected: Compiles successfully.

- [ ] **Step 5: Run golden generator to produce C++ op logs**

```bash
make -C crates/eaglemode/tests/golden/gen run
```
Expected: Golden data regenerated, AND `target/golden-divergence/cosmos_item_border.cpp_ops.jsonl` (and others) created.

Verify:
```bash
head -5 crates/eaglemode/target/golden-divergence/cosmos_item_border.cpp_ops.jsonl
```
Expected: JSONL lines with matching format to Rust output.

- [ ] **Step 6: Commit gen_golden.cpp changes**

```bash
git add crates/eaglemode/tests/golden/gen/gen_golden.cpp
git commit -m "feat(golden-gen): add DrawOp JSONL logging to C++ golden generator"
```

---

### Task 6: Python Diff Script

**Files:**
- Create: `scripts/diff_draw_ops.py`

- [ ] **Step 1: Create the diff script**

```python
#!/usr/bin/env python3
"""Compare C++ and Rust DrawOp JSONL files parameter-by-parameter."""

import json
import sys
import os
from pathlib import Path


def load_ops(path):
    """Load JSONL file into list of dicts."""
    ops = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                ops.append(json.loads(line))
    return ops


def compare_value(key, cpp_val, rust_val):
    """Compare two values. Returns (matches, delta_str)."""
    if isinstance(cpp_val, float) and isinstance(rust_val, float):
        delta = abs(cpp_val - rust_val)
        if delta > 1e-10:
            return False, f"{delta:.6e}"
        return True, ""
    if cpp_val != rust_val:
        return False, str(cpp_val) + " vs " + str(rust_val)
    return True, ""


def diff_ops(cpp_ops, rust_ops, name):
    """Diff two op lists and print report."""
    divergences = []
    min_len = min(len(cpp_ops), len(rust_ops))

    for i in range(min_len):
        cpp = cpp_ops[i]
        rust = rust_ops[i]

        cpp_op = cpp.get("op", "?")
        rust_op = rust.get("op", "?")

        if cpp_op != rust_op:
            divergences.append({
                "seq": i,
                "op": f"{cpp_op} / {rust_op}",
                "param": "op",
                "cpp": cpp_op,
                "rust": rust_op,
                "delta": "TYPE MISMATCH",
            })
            # Stop on type mismatch — everything after is meaningless
            break

        # Compare all params (skip seq and op)
        all_keys = set(cpp.keys()) | set(rust.keys())
        all_keys -= {"seq", "op", "_unserialized"}

        for key in sorted(all_keys):
            cpp_val = cpp.get(key)
            rust_val = rust.get(key)

            if cpp_val is None:
                divergences.append({
                    "seq": i, "op": cpp_op, "param": key,
                    "cpp": "(missing)", "rust": str(rust_val), "delta": "EXTRA IN RUST",
                })
                continue
            if rust_val is None:
                divergences.append({
                    "seq": i, "op": cpp_op, "param": key,
                    "cpp": str(cpp_val), "rust": "(missing)", "delta": "EXTRA IN C++",
                })
                continue

            matches, delta = compare_value(key, cpp_val, rust_val)
            if not matches:
                divergences.append({
                    "seq": i, "op": cpp_op, "param": key,
                    "cpp": format_val(cpp_val), "rust": format_val(rust_val),
                    "delta": delta,
                })

    if len(cpp_ops) != len(rust_ops):
        divergences.append({
            "seq": min_len, "op": "(count)", "param": "op_count",
            "cpp": str(len(cpp_ops)), "rust": str(len(rust_ops)),
            "delta": f"C++ has {len(cpp_ops)}, Rust has {len(rust_ops)}",
        })

    # Print report
    print(f"\n=== {name}: {len(divergences)} divergence(s) in {min_len} ops ===")
    if not divergences:
        print("  IDENTICAL")
        return 0

    print(f"{'seq':>4}  {'op':<28} {'param':<20} {'C++':<24} {'Rust':<24} {'delta'}")
    print(f"{'---':>4}  {'---':<28} {'---':<20} {'---':<24} {'---':<24} {'---'}")
    for d in divergences:
        print(f"{d['seq']:>4}  {d['op']:<28} {d['param']:<20} {str(d['cpp']):<24} {str(d['rust']):<24} {d['delta']}")

    return len(divergences)


def format_val(v):
    """Format a value for display."""
    if isinstance(v, float):
        return f"{v:.15g}"
    return str(v)


def main():
    if len(sys.argv) < 2:
        print("Usage: diff_draw_ops.py <test_name> [divergence_dir]")
        print("  Compares {name}.cpp_ops.jsonl vs {name}.rust_ops.jsonl")
        sys.exit(1)

    name = sys.argv[1]
    div_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("crates/eaglemode/target/golden-divergence")

    cpp_path = div_dir / f"{name}.cpp_ops.jsonl"
    rust_path = div_dir / f"{name}.rust_ops.jsonl"

    if not cpp_path.exists():
        print(f"ERROR: {cpp_path} not found. Run golden generator with logging.")
        sys.exit(1)
    if not rust_path.exists():
        print(f"ERROR: {rust_path} not found. Run: DUMP_DRAW_OPS=1 cargo test --test golden {name}")
        sys.exit(1)

    cpp_ops = load_ops(cpp_path)
    rust_ops = load_ops(rust_path)

    n = diff_ops(cpp_ops, rust_ops, name)
    sys.exit(1 if n > 0 else 0)


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/diff_draw_ops.py
```

- [ ] **Step 3: Test with cosmos_item_border**

Prerequisites: both JSONL files must exist from Tasks 2 and 5.

```bash
python3 scripts/diff_draw_ops.py cosmos_item_border
```

Expected: A table showing parameter-level divergences between C++ and Rust for cosmos_item_border, or "IDENTICAL" if they match.

- [ ] **Step 4: Commit**

```bash
git add scripts/diff_draw_ops.py
git commit -m "feat: add DrawOp parameter diff script for golden test diagnosis"
```

---

### Task 7: Run Diff on All High-Diff Tests

**Files:** None (diagnostic run only)

- [ ] **Step 1: Generate Rust ops for all high-diff tests**

```bash
DUMP_DRAW_OPS=1 cargo test --test golden -- testpanel_root testpanel_expanded tktest_1x tktest_2x cosmos_item_border eagle_logo --test-threads=1
```

Note: `file_selection_box` and `border_nest` may be in different test files. Find them:

```bash
grep -r "fn file_selection_box\|fn border_nest" crates/eaglemode/tests/golden/
```

Run those too with `DUMP_DRAW_OPS=1`. They may need recording passes added (same pattern as Task 2/3).

- [ ] **Step 2: Generate C++ ops (already done in Task 5)**

The golden generator already logs all tests. If not, re-run:

```bash
make -C crates/eaglemode/tests/golden/gen run
```

- [ ] **Step 3: Run diff on each high-diff test**

```bash
for test in cosmos_item_border testpanel_root testpanel_expanded tktest_1x tktest_2x eagle_logo; do
    python3 scripts/diff_draw_ops.py "$test"
done
```

- [ ] **Step 4: Document findings**

Create a summary of divergence patterns. Look for:
- Same parameter diverging across multiple tests (e.g., canvas_color always different)
- Structural divergences (different number of ops)
- Coordinate offsets (consistent additive error in x/y/w/h)

This diagnostic output is the deliverable. No code changes needed — the findings inform the next phase of widget-level fixes.

- [ ] **Step 5: Commit diagnostic results (optional)**

If the JSONL files are small enough, commit them for reference:

```bash
git add scripts/diff_draw_ops.py
git commit -m "docs: DrawOp parameter diff results for high-diff golden tests"
```

---

## Self-Review Checklist

**Spec coverage:**
- Phase 1 (Rust DrawOp serialization): Tasks 1-2 ✓
- Phase 2 (C++ logging): Tasks 4-5 ✓
- Phase 3 (Diff script): Task 6 ✓
- Phase 4 (Apply to high-diff tests): Task 7 ✓
- Compositor test recording: Task 3 ✓
- JSONL format spec: Implemented in Task 1 serializer code ✓
- cosmos_item_border as first target: Task 2 ✓

**Placeholder scan:** No TBD/TODO. All code blocks are complete.

**Type consistency:** `dump_draw_ops(name, &ops)` signature consistent across Tasks 1-3. `DrawOp` enum variant names match between serializer and JSONL format. C++ fprintf format strings match Python parser expectations.
