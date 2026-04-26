# F018 — Compositor Remediation Design

**Status:** Design (remediation only — implementation plan and acceptance-test harness for V.2–V.5 are sibling deliverables).
**Issue:** F018 (`docs/debug/ISSUES.json`).
**Date:** 2026-04-25.
**Inputs:**
- Contract spec — `docs/superpowers/specs/2026-04-25-F018-compositor-integration-contract-design.md`.
- Audit — `docs/debug/F018-audit.md`.

---

## 1. Background and scope

The F018 audit found four root-cause violations of the compositor↔paint-model integration contract:

- **I.1** — Framebuffer pre-fills use literal BLACK at four production sites.
- **I.4** — Compositor wgpu render pass clears to opaque BLACK and alpha-blends through any non-opaque tile pixel.
- **IV.3** — `SVPChoiceByOpacityInvalid` is never set to `true` in the Rust port; opacity transitions do not trigger SVP re-evaluation.
- **II.5** — `emPainter` carries `canvas_color` as a member field; C++ threads it as an explicit parameter to every panel paint.

This spec is the design for closing those four violations. The transitive violations identified by the audit (I.2, I.5 compositor layer, III.1, III.2, III.3, plus V.1) close as a side effect; their closure is documented per fix.

**In scope.**
- Fix shape (algorithm, file boundaries, target code shape, contract-rule mapping) for each of the four root-cause violations.
- A minimal V.1 acceptance harness sufficient to gate "F018 user-visible symptom is closed."
- Dependency graph and phase ordering for the implementation plan.

**Out of scope.**
- Step-by-step implementation tasks. Belongs in the implementation plan that follows this spec.
- V.2 / V.3 / V.4 / V.5 acceptance harness design. Sibling spec — separate brainstorm.
- Compositor performance design (tile size, eviction policy, parallel threshold, recording-painter mechanism). Preserved as-is per the contract spec.
- The structural divergence "`emPanel::InvalidatePainting` lives on `emView` instead of on `emPanel`" surfaced by audit IV.2 newly-discovered concerns. Out of this spec; if a panel call site is missing a Rust mirror it surfaces as a separate issue.

## 2. Decisions locked from brainstorm

The brainstorming session resolved five scope questions before this spec was written. Recorded here so the implementation plan inherits them without re-litigation:

1. **Test harness scope (Q1):** **Hybrid.** This spec includes a *minimal V.1 harness* sufficient to gate the F018 visible symptom. V.2 / V.3 / V.4 / V.5 are explicitly deferred to a sibling spec.
2. **I.4 resolution (Q2):** **Plumb `view.background_color` to the compositor.** The wgpu `LoadOp::Clear` color is parameterized on the view's background color. The compositor gains a single piece of view state. The alternative (force every render strategy to produce alpha=255 for every painted pixel inside the panel-tree footprint) was rejected as pushing fidelity bugs across the panel codebase to satisfy a compositor invariant.
3. **I.1 resolution (Q3):** **Replace BLACK with `view.background_color`** at the three pre-fill sites. Restores the C++ implicit precondition that the framebuffer pre-state equals `view.background_color` (which `emView::Paint`'s conditional clear logic was written against).
4. **II.5 resolution (Q4):** **Remove the painter canvas-color carrier; thread canvas color as an explicit parameter** to every panel paint method. Matches C++ exactly. The audit notes that no forced-divergence category fits the carrier, so by Port Ideology it is a fidelity bug by default.
5. **Granularity and ordering (Q5):** **Single spec, four phases.** Phase 1 (I.1 + I.4 paired) → Phase 2 (V.1 harness, built alongside Phase 1, run as gate) → Phase 3 (IV.3) → Phase 4 (II.5). II.5 is last because it has the widest blast radius; isolating it after F018's symptom-closing fixes means a refactor regression cannot reopen F018.

## 3. Fix shape per root-cause violation

### 3.1 I.1 + I.4 — Plumb `view.background_color` through the render path (paired)

These two fixes travel together. They share the same data path (view → compositor) and resolve the same observable defect (BLACK leakage in non-opaque-SVP scenarios). Splitting them buys nothing and risks shipping half a fix.

**Rules closed.**
- I.1 — Framebuffer pre-state must not be observable.
- I.4 — Compositor load-clear color must not be observable.

**Current shape (per audit).**
- Three CPU-side pre-fills using literal BLACK:
  - `crates/emcore/src/emWindow.rs:632` — single-buffer fallback `viewport_buffer.fill(emColor::BLACK)`.
  - `crates/emcore/src/emWindow.rs:674` — per-tile single-threaded path `tile.image.fill(emColor::BLACK)`.
  - `crates/emcore/src/emWindow.rs:767` — parallel-replay per-thread tile buffer `fill(emColor::BLACK)`.
- One GPU-side load-clear using opaque BLACK:
  - `crates/emcore/src/emViewRendererCompositor.rs:261` — `LoadOp::Clear(wgpu::Color::BLACK)` on the wgpu render pass that composites tile textures onto the surface.
- No data path exists between the view's `background_color` field and the compositor.

**Target shape.**

1. **CPU pre-fills read live `background_color`.** At the top of each render-strategy block in `emWindow::render`, read `view.background_color` once into a local. The three pre-fill sites replace the literal `emColor::BLACK` with that local. The pre-fill now matches what C++'s OS-supplied framebuffer would have been (the implicit precondition `emView::Paint`'s conditional clear logic relies on).

2. **Compositor load-clear is parameterized on the view's background color.** `WgpuCompositor` gains a per-frame `background_color: Color` input. The mechanism choice (parameter vs. setter) is an implementation-plan question; the design constraint is that the value used by the next `render_frame` reflects the most recent `view.background_color` at frame start. The hardcoded `wgpu::Color::BLACK` at `:261` is replaced with a conversion of that input. `SoftwareCompositor` (used by tests and the headless render path at `crates/emcore/src/emViewRenderer.rs:37, 87, 99`) gets the equivalent plumbing for parity.

3. **Invalidation on `SetBackgroundColor` is already correct.** Audit I.5 confirmed: `emView::SetBackgroundColor` sets `viewport_changed = true`; `emGUIFramework.rs:1419-1421` consumes that flag and triggers `win.invalidate()`, marking every tile dirty. Once the compositor reads live `background_color` per frame (target shape #2), no additional invalidation is required: every tile is repainted with the new color, and the load-clear of any uncovered region uses the new color.

**Files affected.**
- `crates/emcore/src/emWindow.rs` — three pre-fill sites; pass `background_color` into the compositor at the call site of `render_frame`.
- `crates/emcore/src/emViewRendererCompositor.rs` — accept the per-frame `background_color`; replace `LoadOp::Clear(wgpu::Color::BLACK)` at `:261`.
- `crates/emcore/src/emViewRenderer.rs` — `SoftwareCompositor` parity (same plumbing for the CPU-side composition path).

**Contract-rule mapping.**
- I.1 closed: no Rust-only BLACK pre-fill remains in production paths.
- I.4 closed: load-clear is now `view.background_color`. Alpha-blend-through still happens for any tile pixel with alpha < 255, but the bleed-through color is now `background_color` — observably equivalent to C++ for non-opaque-SVP scenarios.
- I.2 closed transitively: tile init RGBA(0,0,0,0) is overwritten by the new pre-fill before paint dispatch, same as today; the observable color is no longer BLACK.
- I.5 fully closed: dirty-region layer was already COMPLIANT (audit correction); compositor layer now reads live `background_color` per frame.
- III.1 closed: non-opaque SVP holes now reveal `background_color` (or whatever the I.3 conditional clear writes), not BLACK.
- III.2 closed transitively: non-opaque child holes reveal parent paint or `background_color`, not BLACK pre-fill.
- III.3 closed transitively: opaque-and-covering SVP fast path with non-opaque descendants now exposes `background_color`, not BLACK.
- V.1 unblocked: the F018 user-visible symptom (grey loading background) is now achievable; gated by Phase 2 harness.

**Open questions for implementation plan.**
- Plumbing mechanism for the compositor `background_color` input — per-frame parameter to `render_frame`, a setter called before `render_frame`, or a field on the compositor written by `emWindow` at frame start. All three are observably equivalent; the choice is local code-shape preference.
- Whether the parallel-replay path's per-thread tile buffer (`emWindow.rs:767`) is reachable from the same `view.background_color` read, or whether the value must be captured into the closure / thread context. Code-reading question.

### 3.2 IV.3 — Set `SVPChoiceByOpacityInvalid` on invalidation

**Rule closed.**
- IV.3 — `IsOpaque` change invalidates SVP-choice path.

**Current shape (per audit).**
- `SVPChoiceByOpacityInvalid: bool` is declared on `emView` (`crates/emcore/src/emView.rs:528`), initialized `false` (`:707`), and cleared at consumption sites (`:1883, :2624`).
- No path sets it to `true`. C++ `emPanel.cpp:1284-1290, 1296-1302` sets it inside the two `emPanel::InvalidatePainting` overloads; the Rust port has no `emPanel::InvalidatePainting` (invalidation lives on `emView`), and the Rust `emView::InvalidatePainting` overloads do not set the flag.

**Target shape.**

Add `self.SVPChoiceByOpacityInvalid = true;` to both `emView` invalidation overloads:
- `emView::InvalidatePainting` at `crates/emcore/src/emView.rs:3159-3166` (no-arg overload — full panel clip rect).
- `emView::invalidate_painting_rect` at `crates/emcore/src/emView.rs:3173-3213` (rect-arg overload).

Mirrors C++ `emPanel.cpp:1284-1290` and `:1296-1302`. The Rust mechanism diverges from C++ only in *where* the flag-setting code lives (on `emView` instead of on `emPanel`); the *behavior* — that every invalidation triggers SVP re-evaluation — is identical.

**Files affected.**
- `crates/emcore/src/emView.rs` — two overloads.

**Contract-rule mapping.**
- IV.3 closed.
- V.5 prerequisite mechanism unblocked. V.5 itself remains INCONCLUSIVE until the V.5 harness lands (sibling spec scope), but the underlying logic is now present.

**Open questions for implementation plan.**
- Whether existing tests exercise an opacity transition densely enough to surface a regression if the flag-setting is wrong (e.g., set on the wrong overload, set in the wrong order relative to the rect-clip). The implementation plan should consider whether a smoke test covers the transition before merging.

### 3.3 II.5 — Remove painter canvas-color carrier

**Rule closed.**
- II.5 — `emPainter` is not a canvas-color carrier.

**Current shape (per audit O.2).**
- Carrier field: `canvas_color: emColor` on `PainterState` at `crates/emcore/src/emPainter.rs:200`. Initialized `emColor::TRANSPARENT` in `new` (`:547`) and `new_recording` (`:581`).
- Accessors: `GetCanvasColor` (`:720`), `SetCanvasColor` (`:725`).
- Carrier writers (dispatcher updates that mirror C++ parameter changes):
  - `crates/emcore/src/emView.rs:4770` — post-conditional-clear, before SVP paint (mirrors C++ `emView.cpp:1083`).
  - `crates/emcore/src/emView.rs:4812` — per-child loop (mirrors C++ `emView.cpp:1118`).
  - `crates/emcore/src/emButton.rs:210`, `emCheckButton.rs:116`, `emRadioBox.rs:166`, `emRadioButton.rs:353`, `emBorder.rs:2038, :2246` — intra-panel sub-content paints with a different canvas color.
- Carrier readers (panel paint code that reads `painter.GetCanvasColor()` instead of accepting a parameter):
  - `crates/emcore/src/emFilePanel.rs:166`, `emTunnel.rs:145`, `emButton.rs:191`, `emSplitter.rs:140`, `emCheckButton.rs:97`, `emFileSelectionBox.rs:129, :633`, `emScalarField.rs:344`.

**Target shape.**

1. **Add an explicit `canvas_color: Color` parameter to the panel paint dispatch chain.** The Rust analogue of C++ `emPanel::Paint(emPainter&, emColor canvasColor)`. The audit identifies `paint_one_panel(tree, painter, id, layout)` at `emView.rs:4771` as the dispatcher entrypoint; that signature gains a `canvas_color: Color` parameter, propagating to whatever per-panel paint method it invokes (`emFilePanel::paint_status`, `emButton::paint`, etc.).

2. **Carrier readers switch to the parameter.** The 8 reader sites above replace `painter.GetCanvasColor()` with the parameter received from the dispatcher. No semantic change — the parameter holds the same value the carrier held under the current shape, because dispatcher writes already match C++ parameter changes.

3. **Intra-panel canvas-color updates become local parameter passing.** A panel that today calls `painter.SetCanvasColor(new); ...sub-paint...; painter.SetCanvasColor(old);` instead passes `new` directly to whatever sub-paint helper it invokes. Helper signatures gain the parameter. No mutation on the painter.

4. **Dispatcher updates drop `SetCanvasColor`.** `emView.rs:4770` and `:4812` stop calling `painter.SetCanvasColor(...)`; they pass the locally-computed canvas color (`ncc` after conditional clear, `panel.canvas_color` per child) directly as the new parameter to `paint_one_panel`.

5. **Remove `canvas_color` field, `GetCanvasColor`, `SetCanvasColor` from `emPainter`.** Delete the field, the accessors, and the `new` / `new_recording` initializers. C++'s `emPainter` has no such field (audit confirmed via `include/emCore/emPainter.h:40-200`); the Rust port matches.

6. **Recording painter is unaffected.** `DrawOp::PaintRect` and other recordable ops already capture canvas color per-op (audit O.4 / IV.5). The parameter routes through the panel call into the op record verbatim, same as today.

**Files affected.**
- `crates/emcore/src/emPainter.rs` — remove field, accessors, initializers.
- `crates/emcore/src/emView.rs` — dispatcher: drop two `SetCanvasColor` call sites, add parameter to `paint_one_panel` invocation.
- `crates/emcore/src/emFilePanel.rs`, `emTunnel.rs`, `emButton.rs`, `emSplitter.rs`, `emCheckButton.rs`, `emFileSelectionBox.rs`, `emScalarField.rs`, `emRadioBox.rs`, `emRadioButton.rs`, `emBorder.rs` — panel paint signatures gain the parameter; readers switch from accessor to parameter; intra-panel updates switch from `SetCanvasColor` to parameter passing.
- Any panel paint method not enumerated above that participates in the dispatch chain. The implementation plan discovers these via a `cargo check` cycle as the parameter is threaded.

**Contract-rule mapping.**
- II.5 closed (carrier removed; canvas color is now an explicit parameter at every panel paint).
- II.1, II.2, II.3, II.4 remain COMPLIANT — semantically equivalent to today (parameter replaces carrier; carrier was already updated at C++-equivalent points).

**Risk callout.**
- Widest blast radius in this spec — every panel paint method touched. The implementation plan must phase the migration carefully (likely: introduce the parameter alongside the carrier first, migrate readers to the parameter, then remove the carrier in a second commit). Phase ordering in §5 places II.5 last for exactly this reason: a refactor regression cannot reopen F018 if F018 is already closed and gated.

**Open questions for implementation plan.**
- Exact `paint_one_panel` signature and the immediate callees that need the parameter. Code-reading question.
- Whether helper paint methods (e.g., `emBorder::paint_inner_content`) should take the parameter or capture it via closure / context. Local code-shape preference.
- Whether closures captured by the recording painter need adjustment for the new parameter. Code-reading question.

## 4. V.1 minimal acceptance harness

**Rule gated.**
- V.1 — `VFS_WAITING` / `VFS_LOADING` background renders as `view.background_color`, not BLACK.

**Shape: behavioral pixel-inspection test (β).**

Construct a Rust integration test that:
1. Builds a minimal `emView` + `emFilePanel` + `emWindow` + compositor stack.
2. Sets `view.background_color` to a distinct test color (e.g., `0xFF0000FF` red — *not* the default `0x808080` grey, so the assertion proves we're observing the configured value rather than a default that coincidentally matches the C++ baseline).
3. Forces the panel into `VFS_LOADING` state.
4. Runs one `emWindow::render` cycle.
5. Samples a pixel inside the panel's clip rect, outside the centered "Loading…" text region.
6. Asserts the sampled pixel equals the test color within the established golden pixel tolerance (`tests/golden/common.rs` channel tolerance).

**Why behavioral, not golden-PPM.**
1. The contract is the authority. Once the contract spec is locked, the assertion that V.1 holds is "this pixel equals `view.background_color`," derivable from contract rule III.1. No C++ baseline pixel value is required.
2. Driving `gen_golden` (the C++ baseline generator) into VFS_LOADING for a directory panel is plumbing that may not exist today. The behavioral test bypasses that dependency.
3. The harness pattern this test establishes (construct view, force panel state, inspect pixel) is the template the V.2 / V.3 / V.4 / V.5 sibling spec will extend. Doing V.1 this way bootstraps the sibling.

**Files affected.**
- A new test file under `crates/emcore/tests/` or `crates/eaglemode/tests/` — location depends on where integration tests for full render currently live.

**Contract-rule mapping.**
- V.1 closed when the test passes after Phase 1 lands.

**Open questions for implementation plan.**
- Whether `crates/emcore/tests/` can already construct a fully-wired `emView` + `emWindow` + compositor for an integration-style render, or whether a new test harness module is required.
- Headless-render mechanism: `SoftwareCompositor` is simpler but does not exercise the wgpu `LoadOp::Clear` path; a wgpu headless device exercises the full Phase 1 fix. The implementation plan picks one (likely `SoftwareCompositor` — exercises both pre-fill sites and is sufficient to prove the BLACK→`background_color` fix; full GPU path can be a sibling-spec concern).
- Whether a sanity visual screenshot comparison against C++ Eagle Mode 0.96.4 is also required as a one-time human gate before declaring Phase 1+2 complete.

## 5. Dependency graph and phase ordering

```
                           ┌─────────────────────────────────────┐
                           │ Phase 1: I.1 + I.4 (paired)         │
                           │ - emWindow.rs three pre-fill sites  │
                           │ - emViewRendererCompositor load-    │
                           │   clear parameterized               │
                           │ - emViewRenderer SoftwareCompositor │
                           │   parity                            │
                           └────────────────┬────────────────────┘
                                            │
                                            ▼
                           ┌─────────────────────────────────────┐
                           │ Phase 2: V.1 minimal harness (gate) │
                           │ - new behavioral test               │
                           │ - asserts view.background_color is  │
                           │   visible in VFS_LOADING            │
                           └────────────────┬────────────────────┘
                                            │ V.1 passes
                                            ▼
                           ┌─────────────────────────────────────┐
                           │ Phase 3: IV.3                       │
                           │ - emView InvalidatePainting both    │
                           │   overloads set                     │
                           │   SVPChoiceByOpacityInvalid = true  │
                           └────────────────┬────────────────────┘
                                            │ existing tests pass
                                            ▼
                           ┌─────────────────────────────────────┐
                           │ Phase 4: II.5                       │
                           │ - thread canvas_color parameter     │
                           │ - migrate 8 reader sites + 6 intra- │
                           │   panel update sites + dispatcher   │
                           │ - remove painter carrier field +    │
                           │   accessors                         │
                           └─────────────────────────────────────┘
```

**Inter-phase gates** (the implementation plan lifts these into explicit checkpoints):
- After Phase 1+2: V.1 test passes. One-time human visual sanity check against C++ Eagle Mode 0.96.4 (zoom into a directory panel during loading, confirm grey).
- After Phase 3: existing test suite passes; no behavioral regression in panels with shape-changing state transitions.
- After Phase 4: existing test suite passes; `grep -n 'GetCanvasColor\|SetCanvasColor' crates/emcore/src/` returns zero matches in production code; canvas_color field is removed from `PainterState`.

**Rationale recap.**
- I.1 + I.4 ship together because they share the view→compositor data path and are co-load-bearing for the F018 visible symptom.
- V.1 harness ships in the same phase boundary because the harness has no assertion-target without the fix and the fix has no gate without the harness.
- IV.3 runs after V.1 passes so a regression in IV.3 cannot be confused with an F018 regression. The fix itself is independent of I.1/I.4.
- II.5 is last because it has the widest file blast radius; isolating it after the F018-closing fixes means a refactor regression cannot reopen F018 or IV.3.

## 6. Open questions for implementation plan

These are points where the spec author could not pin down a fact without code reading. They are parked here for the implementation-plan author to resolve, not guessed.

- **OP.1.** Plumbing mechanism for the compositor `background_color` input — per-frame parameter to `WgpuCompositor::render_frame`, setter on the compositor, or field written by `emWindow` at frame start. (§3.1.) Local code-shape preference; observable behavior is identical across the three.
- **OP.2.** In the parallel-replay path, whether `view.background_color` flows naturally into the per-thread tile buffer fill at `emWindow.rs:767`, or must be captured into the closure / thread context. (§3.1.)
- **OP.3.** Whether existing tests cover an opacity transition densely enough to surface a regression on the IV.3 fix (e.g., flag set on the wrong overload). If not, decide whether to add a smoke test in Phase 3 or punt to the sibling V.5 harness. (§3.2.)
- **OP.4.** Exact `paint_one_panel` signature and its immediate callees. The brainstorm lists 10 panel files affected but cannot enumerate every helper paint method without code reading. (§3.3.)
- **OP.5.** Whether helper paint methods inside individual panels (e.g., `emBorder::paint_inner_content`) should take `canvas_color` as a parameter or capture it from the enclosing scope. (§3.3.) Local code-shape preference.
- **OP.6.** Whether the V.1 behavioral test runs against `SoftwareCompositor` only, against a wgpu headless device, or both. (§4.) Trade-off: `SoftwareCompositor` exercises both pre-fill sites and is sufficient to prove the BLACK→`background_color` fix; wgpu headless exercises the full Phase 1 surface including the load-clear.
- **OP.7.** Whether a one-time human visual screenshot comparison against C++ Eagle Mode 0.96.4 should be a documented gate after Phase 1+2, or if the V.1 behavioral assertion alone is sufficient. (§4 / §5.)

## 7. Out of scope and follow-ups

- **V.2 / V.3 / V.4 / V.5 acceptance harness.** Sibling spec, separate brainstorm. Will reuse the harness pattern bootstrapped by V.1.
- **`emPanel::InvalidatePainting` structural divergence.** Audit IV.2 newly-discovered concern: invalidation lives on `emView` in Rust, not on `emPanel` as in C++. Whether every C++ call site has a Rust mirror is not exhaustively verified. If a missing mirror surfaces, file as a separate F-issue.
- **Compositor performance design.** Tile size, eviction, parallel threshold, recording-painter mechanism — all preserved as-is per the contract spec.
- **Other RUST_ONLY architectural additions** (engine scheduler, `emRenderThreadPool`, `emPanelTree` arena) — separate concerns; if any perturbs paint output, file as a sibling F-issue.
