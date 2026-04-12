# Compound Ops → PaintPolygon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor PaintEllipse, PaintRoundRect, and PaintBezier to delegate to PaintPolygon instead of calling fill_polygon_aa directly, matching C++ architecture.

**Architecture:** Each compound op currently duplicates PaintPolygon's logic (canvas_color save/restore + fill_polygon_aa). Refactor to match PaintEllipseSector's existing pattern: record own DrawOp, discard proof, build vertices, call self.PaintPolygon(). Single file change.

**Tech Stack:** Rust, emPainter

---

### Reference Pattern

PaintEllipseSector (line 715) already shows the target pattern. Every edit below replicates this:

```rust
// 1. Record own op, discard proof
let Some(_proof) = self.try_record(DrawOp::PaintFoo { ... }) else { return; };
// 2. Build vertices (no canvas_color manipulation)
let verts = ...;
// 3. Delegate to PaintPolygon
self.PaintPolygon(&verts, color, canvas_color);
```

---

### Task 1: Refactor PaintEllipse

**Files:**
- Modify: `crates/emcore/src/emPainter.rs:672-697`

- [ ] **Step 1: Edit PaintEllipse**

Replace lines 684-696 (from `let Some(proof)` through the canvas_color restore) with:

```rust
        let Some(_proof) = self.try_record(DrawOp::PaintEllipse {
            cx,
            cy,
            rx,
            ry,
            color,
            canvas_color,
        }) else { return; };
        let verts = self.ellipse_polygon(cx, cy, rx, ry);
        self.PaintPolygon(&verts, color, canvas_color);
```

This removes: `let saved_canvas`, `self.state.canvas_color = canvas_color`, `self.fill_polygon_aa(proof, ...)`, `self.state.canvas_color = saved_canvas`.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS (no new warnings — `_proof` suppresses unused binding)

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emPainter.rs
git commit -m "refactor: PaintEllipse delegates to PaintPolygon, matching C++"
```

---

### Task 2: Refactor PaintRoundRect

**Files:**
- Modify: `crates/emcore/src/emPainter.rs:1103-1130`

- [ ] **Step 1: Edit PaintRoundRect**

Replace lines 1113-1129 (from `let Some(proof)` through the canvas_color restore) with:

```rust
        let Some(_proof) = self.try_record(DrawOp::PaintRoundRect {
            x,
            y,
            w,
            h,
            radius,
            color,
            canvas_color,
        }) else { return; };
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let verts = self.round_rect_polygon(x, y, w, h, radius);
        self.PaintPolygon(&verts, color, canvas_color);
```

This removes: `let saved_canvas`, `self.state.canvas_color = canvas_color`, `self.fill_polygon_aa(proof, ...)`, `self.state.canvas_color = saved_canvas`.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emPainter.rs
git commit -m "refactor: PaintRoundRect delegates to PaintPolygon, matching C++"
```

---

### Task 3: Refactor PaintBezier

**Files:**
- Modify: `crates/emcore/src/emPainter.rs:2177-2205`

- [ ] **Step 1: Edit PaintBezier**

Replace lines 2178-2204 (from `let Some(proof)` through the canvas_color restore) with:

```rust
        let Some(_proof) = self.try_record(DrawOp::PaintBezier {
            points: points.to_vec(),
            color,
            canvas_color,
        }) else { return; };
        if points.len() < 3 {
            return;
        }
        // C++ convention: n -= n%3; truncate to multiple of 3.
        let n = points.len() - points.len() % 3;
        let seg_count = n / 3;
        let s = self.state.scale_x + self.state.scale_y;
        let mut verts = Vec::new();
        for i in 0..seg_count {
            let p0 = points[i * 3];
            let p1 = points[i * 3 + 1];
            let p2 = points[i * 3 + 2];
            // P3 = first point of next segment; wraps to points[0] for last segment.
            let p3 = points[((i + 1) * 3) % n];
            tessellate_cubic_cpp(&mut verts, p0, p1, p2, p3, s, 0.0);
        }
        if verts.len() >= 3 {
            self.PaintPolygon(&verts, color, canvas_color);
        }
```

This removes: `let saved_canvas`, `self.state.canvas_color = canvas_color`, `self.fill_polygon_aa(proof, ...)`, `self.state.canvas_color = saved_canvas`.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emPainter.rs
git commit -m "refactor: PaintBezier delegates to PaintPolygon, matching C++"
```

---

### Task 4: Full verification

- [ ] **Step 1: Run full test suite**

Run: `cargo-nextest ntr`
Expected: All tests pass

- [ ] **Step 2: Run golden tests for the 3 affected ops**

Run:
```bash
cargo test --test golden painter_bezier_stroked -- --test-threads=1
cargo test --test golden widget_border_roundrect_thin -- --test-threads=1
cargo test --test golden widget_splitter_v_extreme_tall -- --test-threads=1
```
Expected: Same pass/fail status and pixel diffs as before (this is pixel-neutral)

- [ ] **Step 3: Confirm divergence unchanged**

Run: `wc -l crates/eaglemode/target/golden-divergence/divergence.jsonl`
Expected: Same line count as before the refactor
