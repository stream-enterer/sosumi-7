# F018 Audit — Compositor Integration Contract Compliance

Audit findings for the contract spec at
`docs/superpowers/specs/2026-04-25-F018-compositor-integration-contract-design.md`.
Each section below maps to one open question or contract rule and records the
current Rust port's compliance status, evidence, and notes for the remediation
plan.

## Status legend

- **COMPLIANT** — current code provably satisfies the rule under all scenarios in scope.
- **VIOLATION** — current code provably fails the rule in at least one observable scenario.
- **PARTIAL** — code satisfies some scenarios but provably fails others; both noted.
- **INCONCLUSIVE** — verification deferred to remediation phase (requires test harness, visual check, or downstream open question).

---

## Open Questions

### O.1 — OS-driver canvas color initial value

**Question:** What value does the C++ OS driver pass as `canvasColor` to the top-level `emView::Paint` call? (Spec rule II.1.)

**Investigation:** The OS-driver entry point is `emViewRenderer::ThreadRun` and the single-threaded fallback in `emViewRenderer.cpp`. Both invoke the view through `emViewPort::PaintView`:
- `~/Projects/eaglemode-0.96.4/src/emCore/emViewRenderer.cpp:109` — `CurrentViewPort->PaintView(painter,0);`
- `~/Projects/eaglemode-0.96.4/src/emCore/emViewRenderer.cpp:140` — `CurrentViewPort->PaintView(painter,0);`

`PaintView` forwards to `emView::Paint(painter, canvasColor)`. The literal second argument is `0`, i.e. `emColor(0)` = RGBA(0,0,0,0) = transparent black.

**Finding:** C++ passes `emColor(0)` (transparent black) as the canvas-color argument to `emView::Paint`. This is the same value Rust uses (`emColor::TRANSPARENT`).

**Implication:** Rule II.1 audit (Task 14) is COMPLIANT. The Rust top-level canvas-color choice matches C++ exactly; no remediation needed at the OS-driver entry point.

### O.2 — Rust emPainter canvas-color carrier

**Question:** Does the Rust `emPainter` carry canvas color as a member, and if so where is it set/updated? (Spec rule II.5.)

**Investigation:** Read `emPainter` field declarations and grepped for SetCanvasColor/GetCanvasColor across `crates/emcore/src/`.
- Carrier field: `crates/emcore/src/emPainter.rs:200` — `canvas_color: emColor` inside `PainterState`. Initialized to `emColor::TRANSPARENT` in both `new` (line 547) and `new_recording` (line 581). Accessors `GetCanvasColor` (720) / `SetCanvasColor` (725).
- External SetCanvasColor call sites (panel paint code that updates the painter carrier mid-paint):
  - `crates/emcore/src/emView.rs:4770` — after the conditional clear, before SVP paint (mirrors C++ `emView.cpp:1083` `canvasColor=ncc`).
  - `crates/emcore/src/emView.rs:4812` — per-child loop, before each `paint_one_panel` (mirrors C++ `emView.cpp:1118` `p->Paint(pnt, p->CanvasColor)`).
  - `crates/emcore/src/emButton.rs:210`, `emCheckButton.rs:116`, `emRadioBox.rs:166`, `emRadioButton.rs:353`, `emBorder.rs:2038, 2246` — panels updating the carrier when they paint sub-content with a different canvas color (matches C++ practice of passing canvas color to nested paint calls).
- External GetCanvasColor readers (panel paint code reading the carrier instead of accepting a parameter): `emFilePanel.rs:166`, `emTunnel.rs:145`, `emButton.rs:191`, `emSplitter.rs:140`, `emCheckButton.rs:97`, `emFileSelectionBox.rs:129, 633`, `emScalarField.rs:344`. These are panels that read canvas color from the painter carrier rather than from a `Paint(canvasColor)` parameter.

**Finding:** The Rust painter DOES carry canvas color as a member field. The carrier is set/updated at the C++-equivalent points (after the conditional clear and per-child) — alignment is correct. The structural divergence is that C++ threads canvas color as an explicit parameter to every `emPanel::Paint` call, while Rust calls `SetCanvasColor` on the painter and lets panels read it via `GetCanvasColor`.

**Implication:** Rule II.5 audit (Task 18) records the carrier as a structural divergence (preserved-design-intent in C++ being expressed as carrier-state in Rust). Functionally the carrier is updated at every C++ update point that the spec investigation identified, so the canvas-color value seen by each panel matches C++. The carrier itself is not a F018 root cause; remediation may keep it but should document the divergence.

### O.3 — Per-tile painter clip rect

**Question:** In the per-tile single-threaded path (`emWindow.rs:668-687`), does the painter's clip rect cover the tile bounds or the viewport bounds? (Spec rule I.3.)

**Investigation:** Read `emPainter::new` constructor and the per-tile path in `emWindow.rs`.
- `emPainter::new(target: &mut emImage)` at `crates/emcore/src/emPainter.rs:524-557` sets `clip = ClipRect { x1: 0, y1: 0, x2: w as f64, y2: h as f64 }` where `w, h` are the target image's dimensions.
- Per-tile path at `crates/emcore/src/emWindow.rs:672-680` calls `emPainter::new(&mut tile.image)` with a 256×256 tile image, then `painter.translate(-(col*ts), -(row*ts))`. `translate` is an offset/scaling change that does not modify the clip rect (the clip rect lives in pixel space of the target image).

**Finding:** In the per-tile path, the painter's clip rect is `0..256 × 0..256` — TILE BOUNDS, not viewport bounds. `painter.ClearWithCanvas(...)` writes the full tile (the clip determines `PaintRect` extent — see `ClearWithCanvas` at `emPainter.rs:865-878`).

**Implication:** Rule I.3 audit (Task 10) treats the per-tile clear as writing the full tile when it fires. The conditional in `emView.rs:4727-4738` evaluates against viewport-rect bounds (rx1..rx2, ry1..ry2), independent of the painter's clip — so the conditional fires identically across tiles. When it fires in the per-tile path, the per-tile pre-fill BLACK at `emWindow.rs:674` is overwritten by the clear color (background or canvas). When it does NOT fire, the pre-fill remains visible in regions not subsequently overpainted by the panel — that's the I.1/III.3 cross-cut.

### O.4 — Recording painter records `Clear`

**Question:** Does the recording painter in `render_parallel_inner` record `Clear` ops, or bypass them? (Spec rule IV.5.)

**Investigation:** Searched `DrawOp` variants and read `ClearWithCanvas` impl.
- `DrawOp` enum at `crates/emcore/src/emPainterDrawList.rs:14` lists 37+ variants. `grep -n 'fn Clear\|DrawOp::Clear\b' emPainterDrawList.rs` returns no results — there is no dedicated `DrawOp::Clear` variant.
- `emPainter::ClearWithCanvas` at `crates/emcore/src/emPainter.rs:865-878` is implemented as a delegated call to `self.PaintRect(x, y, w, h, color, canvas_color)` over the current clip rect. `PaintRect` is recordable as `DrawOp::PaintRect` (`emPainterDrawList.rs:462`).
- `emView.rs:4737` calls `painter.ClearWithCanvas(ncc, canvas_color)` from the conditional-clear block. When invoked on a recording painter, this lowers to a `DrawOp::PaintRect` over the painter's full clip region with the clear's color and canvas-color arguments.

**Finding:** The recording painter DOES record the conditional clear — not as a `Clear` op (no such variant exists) but as a `PaintRect` covering the painter's clip rect. On replay into a per-tile painter, that `PaintRect` is replayed with the tile's current transform/clip, painting the clear color over the appropriate region of the tile.

**Implication:** Rule IV.5 audit (Task 28) is COMPLIANT — the parallel-replay path does see the clear. However, the replay's effective region for the recorded `PaintRect` depends on the painter clip at *record time*, which is the recording painter's viewport-sized clip. Replay into a per-tile painter then re-clips against the tile bounds. The pixel result should match the per-tile single-threaded path — but verifying this end-to-end is V.3 (strategy parity), which is INCONCLUSIVE without a test harness.

### O.5 — Compositor unallocated-tile behavior

**Question:** What does `WgpuCompositor::render_frame` do for tiles that are out of the active grid (resized smaller, or never allocated)? (Spec rules I.2, I.4.)

**Investigation:** Re-read `WgpuCompositor::render_frame`, `new`, `resize`, and the upload path.
- `WgpuCompositor::render_frame` at `crates/emcore/src/emViewRendererCompositor.rs:238-303` opens a render pass with `LoadOp::Clear(wgpu::Color::BLACK)` (line 261) covering the surface, then loops over `self.tiles: Vec<Option<TileGpuData>>` (line 17) drawing only `Some` slots (lines 272-280).
- Slots are `None` after `WgpuCompositor::new` (line 122-123) and after `resize` (line 295). They become `Some` on first call to `upload_tile` (line 152-211).
- `emWindow::render` uploads every dirty tile each frame (`emWindow.rs:683` inside the per-tile path; equivalent in the other strategies). Tiles start dirty (`Tile::new` at `emViewRendererTileCache.rs:22`), so on the first frame after construction or resize every visible tile gets uploaded — slots are `Some` from frame 1 onward in steady state.

**Finding:** For tile slots that are `None`, the wgpu render pass clears them to opaque black via `LoadOp::Clear(wgpu::Color::BLACK)` and does not draw over them. In practice this happens for at most one frame after construction or resize.

**Implication:** Rule I.4 audit (Task 11) treats the load-clear as visible during initial-frame transients AND, more importantly, as the alpha-blend background for any `Some` tile pixels with alpha < 255 (`BlendState::ALPHA_BLENDING` at line 97). Rule I.2 audit (Task 9) treats the tile init color (RGBA 0,0,0,0) as observable in the same way. The dominant load-bearing path for the F018 symptom is NOT the unallocated-tile case but the alpha-blend-through case where tiles ARE uploaded but contain non-opaque pixels.

---

## Cluster I — Pixel Equivalence

### I.1 — Framebuffer pre-state must not be observable

**Status:** VIOLATION

**Evidence:**
- Pre-fill sites in production paths (greppable as `fill(emColor::BLACK)` / `LoadOp::Clear(wgpu::Color::BLACK)`):
  - `crates/emcore/src/emWindow.rs:632` — single-buffer fallback (>50% dirty tiles).
  - `crates/emcore/src/emWindow.rs:674` — per-tile single-threaded path.
  - `crates/emcore/src/emWindow.rs:767` — parallel-replay path (per-thread tile buffer).
  - `crates/emcore/src/emViewRendererCompositor.rs:261` — wgpu render-pass `LoadOp::Clear(wgpu::Color::BLACK)`.
- Test-only pre-fills (confirmed by `grep -rn 'SoftwareCompositor::new' crates/` — only matches in `crates/eaglemode/tests/golden/`): `crates/emcore/src/emViewRenderer.rs:37, 87, 99` — `SoftwareCompositor`. Not in production path; still subject to the same contract for tests.
- The conditional clear at `crates/emcore/src/emView.rs:4727-4738` mirrors C++ `emView.cpp:1073-1084` (per I.3 audit). When the SVP is opaque-and-covering the clear is skipped, leaving the BLACK pre-fill as the framebuffer state at panel-paint time. Any non-opaque descendant of an opaque parent then exposes the pre-fill.

**Notes:**
- All four production pre-fill sites use literal BLACK rather than `view.background_color`. Even when the conditional clear fires, the pre-fill is observably wrong for the sub-frame moment between fill and clear (not user-visible, but semantically unconstrained). The user-visible failure is the skip-clear case.
- Remediation must eliminate observable pre-fill in all four sites. Options per spec:
  - (a) replace BLACK with `view.background_color` at every pre-fill site, AND threaded through the wgpu LoadOp (cross-cuts with I.4 and I.5);
  - (b) skip pre-fill entirely and rely on emView's clear semantics to cover every pixel — requires guaranteeing every viewport pixel is overwritten before composition (cross-cuts with III.1, III.2, III.3);
  - (c) introduce a Rust-only pre-clear-to-background-color step before `view.Paint`, mirroring the C++ default-framebuffer assumption.
- Cross-cuts with I.2 (tile init color), I.4 (compositor load-clear), I.5 (background_color propagation), III.1/III.2/III.3 (transitive non-opaque exposure).

### I.2 — Tile backing-store init color is not observable

**Status:** VIOLATION (transitive from I.1)

**Evidence:**
- `emImage::new` at `crates/emcore/src/emImage.rs:118-135` initializes pixel data to `vec![0; len]`. For 4-channel (RGBA) tiles the per-pixel value is `(0, 0, 0, 0)` — transparent black.
- `Tile::new` constructs tiles via `emImage::new(TILE_SIZE, TILE_SIZE, 4)`, then `emWindow.rs:674, 767` immediately overwrites with `tile.image.fill(emColor::BLACK)` = opaque black `(0, 0, 0, 255)`.
- Per O.5: `None` compositor slots show solid LoadOp::Clear opaque black. `Some` slots with alpha < 255 pixels alpha-blend RGB onto that black via `BlendState::ALPHA_BLENDING`.
- Newly-allocated tiles are immediately dirty (`Tile::new` sets `dirty: true`) and painted within the same render call (`emWindow.rs:622, 668-687`). But paint may not cover the full tile — when the conditional I.3 clear is skipped (opaque-and-covering SVP), any tile pixels not subsequently overpainted by panel ops keep the BLACK pre-fill.

**Notes:**
- The tile init color itself (RGBA 0,0,0,0) is not directly observable in production because the pre-fill at line 674/767 immediately overwrites it. The observable color is the pre-fill BLACK — which is the I.1 violation. So I.2 reduces to I.1 in practice.
- Remediation: eliminating the I.1 pre-fill (option a, b, or c) closes I.2 too. If option (b) is chosen (skip pre-fill), the raw `emImage::new` zero-fill becomes observable for un-painted regions — that's also wrong, so option (b) requires changing `emImage::new` for tiles or guaranteeing full-tile paint coverage.

### I.3 — Conditional framebuffer clear must mirror C++

**Status:** COMPLIANT

**Evidence:**
- C++ `emView.cpp:1062-1084` and Rust `emView.rs:4725-4739` are line-for-line equivalent:
  - `if (!SupremeViewedPanel)` → handled by the outer `match self.supreme_viewed_panel` (early branch unconditionally clears).
  - `if (!p->IsOpaque() || p->ViewedX > rx1 || ...)` → `if !tree.IsOpaque(svp_id) || svp_vx > rx1 || svp_vx + svp_vw < rx2 || svp_vy > ry1 || svp_vy + svp_vh < ry2` (line 4727-4732).
  - `ncc = p->CanvasColor; if (!ncc.IsOpaque()) ncc = BackgroundColor;` → `let mut ncc = svp_canvas; if !ncc.IsOpaque() { ncc = self.background_color; }` (line 4733-4736).
  - `painter.Clear(ncc, canvasColor); canvasColor = ncc;` → `painter.ClearWithCanvas(ncc, canvas_color); canvas_color = ncc;` (line 4737-4738).
- All three render strategies in `emWindow.rs:632-700, 760-776` invoke the panel walk through `view.Paint`/`render_parallel_inner` → display-list replay; both ultimately route through the same `emView::Paint` block for direct paint, and the recording painter records the clear as a `PaintRect` (per O.4) for replay.

**Notes:**
- The clear-condition mirror is correct in isolation. The black-during-loading symptom does NOT come from this conditional being wrong — it comes from I.1/I.2/I.4 (pre-fills and load-clear) being observable in scenarios where this conditional skips the clear. Remediation for the symptom does not need to touch this block.

### I.4 — Compositor load-clear color must not be observable

**Status:** VIOLATION

**Evidence:**
- `crates/emcore/src/emViewRendererCompositor.rs:261` — `load: wgpu::LoadOp::Clear(wgpu::Color::BLACK)`.
- `crates/emcore/src/emViewRendererCompositor.rs:97` — `BlendState::ALPHA_BLENDING`. Tiles composite as `out_RGB = tile_RGB * tile_A + load_clear_RGB * (1 - tile_A)`.
- Per O.5: `None` tile slots show the load-clear directly (initial frame / post-resize transient). `Some` slots whose tile pixels have alpha < 255 blend the load-clear black through.

**Notes:**
- Remediation per spec rule I.4 has two options: (a) cover every viewport pixel with an opaque tile fragment so alpha is always 255 — couples to I.1/I.2 fixes and to per-panel paint-coverage guarantees; (b) parameterize the load-clear color on `view.background_color` and update it whenever the view's background changes — cross-cuts with I.5 (currently no plumbing exists between view and compositor for this).
- Option (b) is mechanically simplest but introduces a view→compositor data-flow that the current architecture does not have (compositor knows nothing about the view).
- Option (a) is structurally cleaner (compositor stays view-agnostic) but requires that every render strategy emit alpha=255 for every painted pixel inside the visible panel-tree footprint. Currently both pre-fill BLACK (alpha=255) and the conditional clear (which may have alpha<255 for non-opaque ncc) violate that.

### I.5 — Runtime `view.background_color` changes propagate

**Status:** VIOLATION

**Evidence:**
- `emView::SetBackgroundColor` at `crates/emcore/src/emView.rs:3508-3513` only updates the field and sets `self.viewport_changed = true`. It does NOT call `InvalidatePainting` or otherwise mark tiles dirty.
- `viewport_changed` is consumed only by `viewport_changed()` getter (line 3477) and `clear_viewport_changed()` setter (line 3482). `grep -rn 'viewport_changed' crates/emcore/src/emWindow.rs` returns NO matches — the window/render loop never reads the flag, so it never triggers a tile-cache invalidation.
- Compositor / tile cache / window have no `background_color` references outside `emWindow.rs:346, 367, 441, 449` (which only forward the constructor argument to `view.SetBackgroundColor`). The compositor's load-clear is hardcoded BLACK (`emViewRendererCompositor.rs:261`) with no view-state plumbing.

**Notes:**
- Two layers fail simultaneously:
  1. **Dirty-region layer:** `SetBackgroundColor` does not invalidate tiles. After a runtime call, tiles keep their previously-painted contents until something else dirties them. New tiles painted with the new color (after some other invalidation) would coexist with stale tiles painted with the old color.
  2. **Compositor-load-clear layer:** Even if every tile is repainted with the new color, the load-clear is hardcoded BLACK; the I.4 violation still leaks the old wrong color (BLACK) where alpha<255.
- Remediation must (a) add a full-viewport InvalidatePainting (or equivalent dirty-mark) inside SetBackgroundColor, AND (b) plumb background_color from view to compositor (option I.4-b) or guarantee opaque tile coverage (option I.4-a).

---

## Cluster II — Canvas-color Propagation

### II.1 — `view.Paint` receives the OS-driver canvas color

**Status:** COMPLIANT

**Evidence:**
- Rust render strategies pass `emColor::TRANSPARENT` to `view.Paint`:
  - `crates/emcore/src/emWindow.rs:635` (single-buffer fallback).
  - `crates/emcore/src/emWindow.rs:679` (per-tile path).
  - `crates/emcore/src/emWindow.rs:736` (parallel record path — verified inline by reading 730+).
  - Test-only: `crates/emcore/src/emViewRenderer.rs:41, 64`.
- Per O.1: C++ passes literal `0` = `emColor(0)` = transparent black at `~/Projects/eaglemode-0.96.4/src/emCore/emViewRenderer.cpp:109, 140`. Rust's `emColor::TRANSPARENT` is the same value (RGBA 0,0,0,0).

**Notes:** No remediation needed at the entry point.

### II.2 — SVP receives the conditionally-updated canvas color

**Status:** COMPLIANT

**Evidence:**
- `crates/emcore/src/emView.rs:4738` — `canvas_color = ncc;` immediately after the conditional `ClearWithCanvas`. Mirrors C++ `emView.cpp:1083`.
- SVP paint dispatch at `crates/emcore/src/emView.rs:4770-4771`:
  - `painter.SetCanvasColor(canvas_color);` propagates the (possibly updated) value to the painter carrier.
  - `self.paint_one_panel(tree, painter, svp_id, svp_layout);` invokes the SVP's paint via the carrier.
- Mirrors C++ `emView.cpp:1098`: `p->Paint(pnt, canvasColor)` where `canvasColor` was just updated on line 1083.

**Notes:** Per O.2, the divergence is parameter-vs-carrier (preserved-design-intent in C++ is expressed as carrier-state in Rust). Functionally equivalent at this update point.

### II.3 — Children receive their own `CanvasColor`

**Status:** COMPLIANT

**Evidence:**
- C++ `emView.cpp:1118` — `p->Paint(pnt, p->CanvasColor);` — child receives its own panel's `CanvasColor`, not the parent's or the SVP's.
- Rust child-paint dispatch at `crates/emcore/src/emView.rs:4805, 4812`:
  - `let p_canvas = panel.canvas_color;` — fetches the child panel's own canvas color from its `PanelRecord`.
  - `painter.SetCanvasColor(p_canvas);` — updates the painter carrier with the child's canvas color before invoking `paint_one_panel`.

**Notes:** Per-child carrier update mirrors C++ per-child parameter pass. No remediation.

### II.4 — Tile boundaries do not perturb canvas color

**Status:** COMPLIANT (transitive from II.1, II.2, II.3)

**Evidence:**
- Per-tile path (`crates/emcore/src/emWindow.rs:668-687`): each dirty tile invokes `view.Paint(tree, &mut painter, emColor::TRANSPARENT)` with a fresh painter (carrier reset to `TRANSPARENT` at `emPainter::new`, line 547). Each call re-derives canvas color through the same `emView::Paint` block. Identical input → identical canvas-color output for the same panel across tiles.
- Single-buffer fallback (`emWindow.rs:632-636`): one `view.Paint` call writes the whole viewport-sized buffer; canvas color is derived once and applied during the same walk.
- Parallel path (`emWindow.rs:735-736`): one `view.Paint` records into a display list with each op carrying its own canvas-color argument (the recording painter captures `canvas_color` as part of `DrawOp::PaintRect` etc.). Replay into per-tile painters does not re-derive canvas color — it replays the recorded values verbatim.

**Notes:** Tile boundaries do not affect canvas-color computation under any of the three strategies. Compliance is transitive from II.2/II.3.

### II.5 — `emPainter` is not a canvas-color carrier

**Status:** VIOLATION (structural — preserved-design-intent divergence)

**Evidence:** Per O.2 finding: Rust `emPainter` has a `canvas_color: emColor` member field (`crates/emcore/src/emPainter.rs:200`), accessed via `GetCanvasColor` / `SetCanvasColor` (lines 720, 725). C++ passes canvas color as an explicit parameter to every `emPanel::Paint(emPainter, emColor canvasColor)` call. Rust panels read canvas color from the painter carrier (e.g., `emFilePanel.rs:166`, `emTunnel.rs:145`, `emButton.rs:191`).

**Notes:**
- This is the spec's preserved-design-intent rule (II.5). The carrier is a structural divergence from C++. The spec admits this divergence only if updates at the carrier are aligned with C++'s parameter changes — which O.2 confirmed at the canonical update points (after conditional clear `emView.rs:4770`, per child `emView.rs:4812`).
- Functionally equivalent today, but the divergence is load-bearing: any future panel that forgets to call `SetCanvasColor` before delegating to a sub-paint will silently use stale canvas color. C++ cannot make this mistake (parameter is required).
- Remediation options:
  - (a) accept the divergence and document it as an annotated `DIVERGED:` block citing the C++ shape and the rationale (matches CLAUDE.md `IDIOM:`-retired norm; would actually need a forced-divergence category — no such category obviously fits, suggesting the divergence is either an idiom adaptation or a hidden fidelity bug);
  - (b) thread canvas color as an explicit parameter to every panel's paint method, removing the carrier and matching C++ exactly.
- Decision deferred to remediation. F018 root cause does NOT require resolving II.5; the symptom is in I.1/I.4.

---

## Cluster III — Non-opaque Composition

### III.1 — Non-opaque SVP reveals view background

**Status:**
**Evidence:**
**Notes:**

### III.2 — Non-opaque child reveals parent

**Status:**
**Evidence:**
**Notes:**

### III.3 — Opaque-panel skip-clear remains valid under tiles

**Status:**
**Evidence:**
**Notes:**

---

## Cluster IV — Dirty-region Soundness

### IV.1 — `InvalidatePainting` propagates to tile cache and compositor

**Status:**
**Evidence:**
**Notes:**

### IV.2 — Painted region shrinking invalidates the difference

**Status:**
**Evidence:**
**Notes:**

### IV.3 — `IsOpaque` change invalidates SVP-choice path

**Status:**
**Evidence:**
**Notes:**

### IV.4 — All three render strategies obey the dirty contract identically

**Status:**
**Evidence:**
**Notes:**

### IV.5 — Recording-painter ops must record the conditional clear

**Status:**
**Evidence:**
**Notes:**

---

## Cluster V — Acceptance Criteria

### V.1 — F018 repro: `VFS_WAITING`/`VFS_LOADING` background is grey

**Status:**
**Evidence:**
**Notes:**

### V.2 — Background-color change visibly propagates

**Status:**
**Evidence:**
**Notes:**

### V.3 — Strategy parity

**Status:**
**Evidence:**
**Notes:**

### V.4 — Painted-region shrink shows no ghost

**Status:**
**Evidence:**
**Notes:**

### V.5 — Opacity transition rebuilds framebuffer

**Status:**
**Evidence:**
**Notes:**

---

## Summary

(Filled in by Task 36: total compliant / violation / partial / inconclusive counts, ordered list of violations to address, and any newly-discovered acceptance criteria.)
