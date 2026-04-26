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

**Status:** PARTIAL (corrected — see Notes)

**Evidence:**
- `emView::SetBackgroundColor` at `crates/emcore/src/emView.rs:3508-3513` only updates the field and sets `self.viewport_changed = true`. It does NOT call `InvalidatePainting` or otherwise mark tiles dirty.
- `viewport_changed` is consumed in the frame loop at `crates/emcore/src/emGUIFramework.rs:1419-1421` — when set, it triggers `needs_full_repaint = true` → `win.invalidate()` (line 1431), which marks all tiles dirty. (Earlier draft of this audit incorrectly stated `viewport_changed` is never read; the correct path goes through emGUIFramework, not directly through emWindow.)
- Compositor / tile cache / window have no `background_color` references outside `emWindow.rs:346, 367, 441, 449` (which only forward the constructor argument to `view.SetBackgroundColor`). The compositor's load-clear is hardcoded BLACK (`emViewRendererCompositor.rs:261`) with no view-state plumbing.

**Notes:**
- Status downgraded from full VIOLATION to PARTIAL on this layer-by-layer analysis:
  1. **Dirty-region layer: COMPLIANT.** `SetBackgroundColor` → `viewport_changed = true` → emGUIFramework triggers `win.invalidate()` → all tiles dirty → all tiles repainted with new background color through the conditional I.3 clear.
  2. **Compositor-load-clear layer: VIOLATION.** Even with all tiles correctly repainted, the load-clear is hardcoded BLACK (`emViewRendererCompositor.rs:261`); pixels showing the load-clear (per I.4) do NOT pick up the new color.
- Net **Status: PARTIAL** — see status correction. Remediation must plumb background_color from view to compositor (option I.4-b) or guarantee opaque tile coverage (option I.4-a). No fix needed on the dirty-region layer.

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

**Status:** VIOLATION

**Evidence:**
- F018 repro (spec section V.1): zooming into a directory panel during VFS_LOADING shows BLACK where C++ Eagle Mode 0.96.4 shows 0x808080 grey.
- I.3 audit (Task 10): COMPLIANT. The conditional clear correctly fires for non-opaque SVPs, writing `view.background_color` (or fallback) into the painter's clip region.
- I.4 audit (Task 11): VIOLATION. Compositor load-clear is hardcoded BLACK, alpha-blend composes through any non-opaque tile pixels.
- I.1 audit (Task 8): VIOLATION. All four production pre-fill sites use literal BLACK.
- For the F018 case, `emFilePanel` returns non-opaque `IsOpaque()` during loading, so it is the SVP and the I.3 clear DOES fire — writing `view.background_color` (= grey 0x808080 by default per `emView.rs:664`) into the tile's clip region. But the tile is still uploaded to the compositor with whatever alpha the panel paint produced. If the loading-state paint produces alpha<255 in regions, those pixels alpha-blend through the BLACK load-clear. If the panel paint produces alpha=255 grey, it should display as grey. The empirical observation (BLACK) suggests the panel paints with alpha<255 OR doesn't fully cover the tile region.

**Notes:**
- The contributory mechanism is the composite of I.4 (load-clear BLACK) and either I.1 (skipped pre-fill paint) or panel-paint alpha<255. Resolving I.1+I.4 closes III.1.
- Remediation needs to verify post-fix that V.1 (the F018 repro) actually shows grey — see V.1 audit. A successful I.4 fix (option a or b) is necessary; an I.1 fix may also be necessary depending on which root mechanism dominates.

### III.2 — Non-opaque child reveals parent

**Status:** PARTIAL — COMPLIANT for opaque-parent + non-opaque-child case; VIOLATION when parent is non-opaque (transitive from I.1).

**Evidence:**
- Rust child-paint dispatch at `crates/emcore/src/emView.rs:4777-4830` mirrors C++ `emView.cpp:1099-1135`: SVP painted first (line 4771), then iterative DFS over children (4777+). Parent-before-child order preserved.
- Parent paints into the tile/buffer. Child paints next, alpha-blending against the tile's current state (= parent's painted pixels in regions parent covered, or pre-fill BLACK in regions parent skipped).
- When parent paints opaquely over its own region, child sees parent's pixels and composes correctly — COMPLIANT.
- When parent itself is non-opaque or skips pixels (the F018 case for emFilePanel), child sees the pre-fill BLACK leaking through parent's holes — that's a transitive I.1 violation manifesting through III.2.

**Notes:** Closing I.1 closes III.2.

### III.3 — Opaque-panel skip-clear remains valid under tiles

**Status:** PARTIAL — COMPLIANT when SVP and all descendants paint opaquely; VIOLATION otherwise (transitive from I.1).

**Evidence:**
- When the I.3 conditional clear is skipped (opaque-and-covering SVP — `emView.rs:4727-4732`), the framebuffer state at SVP-paint time is the I.1 pre-fill BLACK plus, on the GPU side, whatever was in the tile texture (overwritten by the next `upload_tile`).
- If SVP and all visible descendants paint every pixel opaquely, pre-fill is invisible — COMPLIANT in that case.
- If SVP is opaque-and-covering BUT some descendant is non-opaque (e.g. an emFilePanel inside an opaque parent), the descendant's holes show the pre-fill BLACK — VIOLATION.

**Notes:** Closing I.1 closes III.3. The C++ original implicitly assumes the framebuffer pre-state equals `view.background_color` (which it does in C++ because Eagle Mode's default OS framebuffer is so-cleared). Rust violates that assumption with the BLACK pre-fill.

---

## Cluster IV — Dirty-region Soundness

### IV.1 — `InvalidatePainting` propagates to tile cache and compositor

**Status:** COMPLIANT

**Evidence:**
- Chain end-to-end:
  - `emView::InvalidatePainting` at `crates/emcore/src/emView.rs:3159-3166` pushes a `Rect(clip_x, clip_y, clip_w, clip_h)` onto `self.dirty_rects: Vec<Rect>`. (No-arg overload — uses panel's full clip rect.)
  - `emView::invalidate_painting_rect` at `crates/emcore/src/emView.rs:3173-3213` clips against panel clip rect and pushes the clipped rect.
  - `emView::take_dirty_rects` / `take_dirty_clip_rects` at `emView.rs:3425-3431` drain the queue.
  - Frame loop in `emGUIFramework.rs:1404-1416` calls `view.has_dirty_rects() / take_dirty_clip_rects()` then iterates each rect and calls `win.mark_dirty_rect(r.x1, r.y1, r.x2, r.y2)`.
  - `emWindow::invalidate_rect` at `emWindow.rs:1568-1570` and `emWindow::mark_dirty_rect` at `emWindow.rs:1575-1597` compute the tile range covered by the rect and call `tile_cache.mark_dirty(col, row)` per overlapping tile.
- The chain exists end-to-end. Tile-aligned rect computation at `emWindow.rs:1587-1590` floors x1/y1 and ceils x2/y2 — correctly covers all overlapping tiles.

**Notes:**
- Compositor invalidation is implicit: a tile marked dirty is re-uploaded next frame via `compositor.upload_tile`, which overwrites the GPU texture. No separate compositor-cache invalidation needed.
- This rule is COMPLIANT in steady state. Note that I.5 incorrectly described `viewport_changed` as never being read — actually `emGUIFramework.rs:1419-1421` reads it and triggers `needs_full_repaint`, which calls `win.invalidate()`. See I.5 audit Notes for the correction; I.5 status remains VIOLATION because the compositor load-clear (I.4) is independent and a full repaint does not fix the load-clear leak.

### IV.2 — Painted region shrinking invalidates the difference

**Status:** PARTIAL (overload exists; specific call-sites unchecked)

**Evidence:**
- Rust overloads at `crates/emcore/src/emView.rs:3159` (no-arg, full panel clip rect) and `emView.rs:3173` (rect-arg).
- Note: there is no `emPanel::InvalidatePainting` method — invalidation lives on `emView`, not `emPanel`. C++ has it on `emPanel` (emPanel.cpp:1282-1311). This is a structural divergence: in Rust, panels must call `view.InvalidatePainting(tree, self_id)` rather than `self.InvalidatePainting()`. Functionally equivalent if every C++ `InvalidatePainting()` call site has a Rust `view.InvalidatePainting(tree, id)` mirror — which is NOT exhaustively verified here.
- `grep -n 'InvalidatePainting\|invalidate_painting' crates/emcore/src/emFilePanel.rs crates/emcore/src/emFileModel.rs` returns no results — `emFilePanel` does not directly invalidate on its own state changes. Invalidation likely happens via `mark_panel_dirty` (a different path) or via the model's dirty-flag → engine cycle.

**Notes:**
- The plan's pre-condition (panels call no-arg overload on shape-changing state transitions) cannot be verified without a wider audit of every `InvalidatePainting` call site. For the F018 symptom, the relevant transition is VFS_LOADED → VFS_LOADING (which changes the loading-state overlay region). Whether `emFilePanel` invalidates correctly on that transition is OUTSIDE the immediate F018 root cause (the symptom is BLACK leaking through, not stale paint).
- Remediation should add a sweep verifying every shape-changing state transition either calls the no-arg overload or bounds the rect-arg overload to the union of old and new regions.

### IV.3 — `IsOpaque` change invalidates SVP-choice path

**Status:** VIOLATION

**Evidence:**
- `SVPChoiceByOpacityInvalid: bool` is declared on emView (`crates/emcore/src/emView.rs:528`), initialized to false (line 707), and CLEARED to false at consumption sites (`emView.rs:1883, 2624`). `grep -rn 'SVPChoiceByOpacityInvalid *=' crates/emcore/src/` returns ONLY the two false-setters and the initializer — no path SETS it to true.
- C++ `emPanel::InvalidatePainting` at `emPanel.cpp:1284-1290, 1296-1302` sets `View->SVPChoiceByOpacityInvalid = true` whenever invalidation runs on a panel that could affect SVP choice. Rust's `emView::InvalidatePainting` (line 3159-3166) does NOT set this flag.

**Notes:**
- Consequence: when a panel's `IsOpaque()` return value changes between frames (e.g., emFilePanel transitioning VFS_LOADED → VFS_LOAD_ERROR), the SVP-choice does not get re-evaluated. The view continues using the old SVP, which may now be the wrong one.
- This is independent of the F018 symptom (which is about pre-fill leaking through during a single frame), but it's a latent bug that V.5 acceptance would expose.
- Remediation: add `self.SVPChoiceByOpacityInvalid = true` inside `emView::InvalidatePainting` and `invalidate_painting_rect`, mirroring C++ `emPanel.cpp:1284-1290, 1296-1302`. Verify a behavioral test triggers re-evaluation on opacity change.

### IV.4 — All three render strategies obey the dirty contract identically

**Status:** INCONCLUSIVE — requires test harness

**Evidence:**
- Single-buffer fallback (`crates/emcore/src/emWindow.rs:632-651`): paints viewport-sized buffer once, then `for row/col` loop only copies dirty tiles (line 640: `if tile.dirty { ... }`). Clean tiles are not touched on the GPU side.
- Per-tile path (`emWindow.rs:670-686`): `for row/col` only paints dirty tiles (line 673: `if tile.dirty { ... }`). Clean tiles untouched.
- Parallel path (`emWindow.rs:760-776` `render_parallel_inner`): records once into a display list, then `render_pool.CallParallel` over `dirty_tiles.len()` (line 775). Only dirty tiles get replayed.
- All three respect `tile.dirty` for which tiles to (re-)paint. Same `view.Paint` block produces the same draw operations for the same panel-tree state.

**Notes:**
- Strict pixel-equivalence across strategies is INCONCLUSIVE without a test that exercises the same panel-tree state under each strategy and compares outputs (V.3 acceptance). The strategies have small structural differences (single-buffer vs per-tile painter clip; recording-replay vs direct paint) that could produce divergent output for non-opaque panels — see I.4 alpha-blend-through interaction.
- Remediation plan must add a strategy-parity test as part of V.3.

### IV.5 — Recording-painter ops must record the conditional clear

**Status:** COMPLIANT (per O.4)

**Evidence:** Per O.4 finding: `DrawOp` has no dedicated `Clear` variant, but `emPainter::ClearWithCanvas` at `crates/emcore/src/emPainter.rs:865-878` is implemented as a delegated `PaintRect` over the painter's clip region. `DrawOp::PaintRect` (line 462 of `emPainterDrawList.rs`) is recordable. So recording captures the conditional clear as a `PaintRect`, and replay invokes the same paint over the per-tile painter's clip-intersected region.

**Notes:**
- The clear is recorded but as a `PaintRect`, not a semantic `Clear`. Diagnostic dumps and op-count audits should account for this — there's no "Clear count" to grep for.
- If remediation introduces a dedicated `DrawOp::Clear` variant for performance (avoid full-rect raster on every replay) or for explicit semantics, it must propagate through both record and replay paths and update the recording-painter `ClearWithCanvas` implementation.

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
