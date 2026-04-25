# F018 — Compositor ↔ Paint-Model Integration Contract

**Status:** Design (contract only — audit and remediation are out of scope).
**Issue:** F018 (`docs/debug/ISSUES.json`).
**Date:** 2026-04-25.
**Authority model:** Each rule is an observable invariant anchored to specific C++ source. Compositor internals are unconstrained as long as the observable matches.

---

## 1. Background

Eagle Mode (C++) paints in one pass: the OS window driver hands `emView::Paint` a framebuffer-backed `emPainter`, and `emView::Paint` walks the panel tree once, writing pixels directly into the framebuffer. There is no tile cache, no record/replay, no compositor, and no per-frame buffer pre-fill. The framebuffer state at any moment during the walk is fully determined by C++ source: see `~/Projects/eaglemode-0.96.4/src/emCore/emView.cpp:1048-1146` for the dispatch and `src/emCore/emPainter.cpp:364-374` for `Clear`.

The Rust port adds a tiled compositor (`crates/emcore/src/emViewRendererCompositor.rs`, `emViewRendererTileCache.rs`) and three rendering strategies in `crates/emcore/src/emWindow.rs:567-701`:

1. **Single-buffer fallback** (line 628-652) — used when more than 50% of tiles are dirty. Paints once into a viewport-sized buffer, then copies tile-sized chunks.
2. **Parallel record/replay** (line 653-667) — used when multi-threaded and several tiles are dirty. `render_parallel_inner` records ops single-threaded then replays per tile in parallel.
3. **Per-tile single-threaded** (line 668-687) — default path. Paints each dirty tile by re-walking the tree with a translated painter.

The compositor is performance-load-bearing (~10-20× release-mode speedup, per commits `beb83ccf`, `b970ac25`, `1bdf5cc2`, `8c4dff80`, `2145e710`) and stays. It was added without writing down the contract by which it must preserve C++ paint semantics. F018 is that contract.

The visible symptom that surfaced this gap is F018's repro: directory panels render BLACK during `VFS_WAITING`/`VFS_LOADING` instead of the C++-equivalent grey. The contract is not the fix for that symptom — it is the framework against which the symptom (and any other compositor-induced divergence) is judged.

## 2. Scope

**In scope.** Observable paint-pipeline invariants the compositor and per-tile renderer must preserve relative to C++ `emView::Paint`. Concretely: framebuffer pixel state at frame boundaries, canvas-color threading through paint dispatch, opaque/non-opaque parent-child composition, dirty-region soundness across content changes, runtime behavior on `view.background_color` changes, and the acceptance test for F018's repro.

**Out of scope.**
- The audit of where current Rust code complies with vs. violates each rule. (Belongs in the implementation plan.)
- The fix for the black-during-loading symptom. (Implementation plan.)
- Compositor performance design — tile size, eviction policy, parallel strategy thresholds. The contract takes the existing performance design as given.
- The recording-painter / display-list mechanism (`emPainterDrawList`). Constrained only insofar as its replay output must obey the contract.
- Cursor, focus, input dispatch — orthogonal pipelines.

## 3. Definitions

- **Framebuffer.** The pixel buffer presented to the user this frame. In C++: the `emPainter::Map` passed by the OS driver. In Rust: the wgpu surface texture written by `WgpuCompositor::render_frame` (`emViewRendererCompositor.rs:238-286`).
- **Tile.** One 256×256 RGBA8 cell of the Rust tile cache. Backing store is `emImage::new(256, 256, 4)` (`emViewRendererTileCache.rs:21`); GPU texture is `Rgba8UnormSrgb` (`emViewRendererCompositor.rs:163`).
- **Tile backing store.** The CPU-side `emImage` of a tile, before upload to GPU.
- **Dirty region.** The set of framebuffer pixels that may differ from the previous frame's presented framebuffer. C++: `emView::InvalidatePainting` accumulates rects. Rust: per-tile `dirty: bool` (`emViewRendererTileCache.rs:13`) plus `emWindow::mark_dirty_rect` (`emWindow.rs:1575+`).
- **Canvas color.** The opaque background color that a paint operation can assume already exists at every pixel of its target region. Used for the optimized blend formula `target += (source − canvasColor) × alpha` (see `include/emCore/emPainter.h:103-114`). In C++ it is an explicit argument on every `emPainter::Paint*` and `emPanel::Paint` method, never a member of `emPainter`.
- **Opaque panel.** A panel for which `IsOpaque() == true` (default `false` per `emPanel.cpp:1211`). Signals that the framework may skip clearing under it. `emFilePanel::IsOpaque` returns `true` only for error states (`emFilePanel.cpp:187-197`); during `VFS_WAITING`/`VFS_LOADING`/`VFS_LOADED`/`VFS_UNSAVED`/`VFS_SAVING`/`VFS_TOO_COSTLY`/`VFS_NO_FILE_MODEL` it returns `false`.
- **SVP.** Supreme Viewed Panel — the topmost panel covering the visible viewport (`emView::SupremeViewedPanel`). C++ uses it to decide whether the framebuffer must be cleared before paint dispatch (`emView.cpp:1062-1084`).
- **Single-buffer fallback.** The `>50%-dirty` branch in `emWindow::render` (`emWindow.rs:628-652`).
- **Parallel replay.** The `render_parallel_inner` branch in `emWindow::render` (`emWindow.rs:653-667`, `emWindow.rs:713+`).
- **Per-tile path.** The default branch in `emWindow::render` (`emWindow.rs:668-687`).

## 4. Contract

The contract is grouped into five clusters. Each cluster opens with a one-paragraph invariant and lists sub-rules. Each sub-rule states the invariant, cites the C++ reference, and notes why the rule exists.

### Cluster I — Pixel equivalence

**Invariant.** At the moment `WgpuCompositor::render_frame` returns and the surface is presented, every framebuffer pixel must equal the value that C++ `emView::Paint` would have written to that pixel given the same panel tree, view state, viewport, and `view.background_color`. No pixel may carry a color that originates in a Rust-only init, fill, or load-clear that has no C++ analogue.

#### I.1 — Framebuffer pre-state must not be observable

**Rule.** No Rust-only buffer pre-fill (tile init color, `viewport_buffer.fill(BLACK)`, per-tile `tile.image.fill(BLACK)`, `WgpuCompositor` `LoadOp::Clear(BLACK)`, `framebuffer.fill(BLACK)` in `SoftwareCompositor`) may survive into the presented framebuffer for any pixel in the dirty region.

**C++ reference.** `emView::Paint` at `src/emCore/emView.cpp:1062-1084`. C++ has no pre-fill: the framebuffer either gets cleared by `painter.Clear(BackgroundColor, canvasColor)` (line 1063) when there is no SVP, or `painter.Clear(ncc, canvasColor)` (line 1082) when SVP is non-opaque or doesn't cover the rect, or no clear at all when the SVP is opaque and covers the rect. After that, panel paint ops write the final pixels.

**Why.** The current Rust pre-fills are a performance shortcut on the assumption that "downstream paint ops will cover them." That assumption is wrong whenever (a) `emView::Paint`'s conditional clear is skipped because the SVP is opaque-and-covering, AND (b) some descendant paints non-opaquely and reveals the buffer underneath. The pre-fill color then leaks through. C++ never reveals a pre-fill because there isn't one.

#### I.2 — Tile backing-store init color is not observable

**Rule.** The initial pixel value of a freshly-allocated tile (currently `vec![0; len]` via `emImage::new` at `emImage.rs:123-135`, i.e. RGBA `(0,0,0,0)`) must not be observable in the presented framebuffer. Any tile that becomes part of the dirty region must be fully overwritten — by `view.Paint` output, by the C++-equivalent clear-to-`BackgroundColor`, or by both — before composition.

**C++ reference.** No analogue. C++ allocates no tiles. The rule exists to neutralize a Rust-only allocation event.

**Why.** A tile that is allocated but never painted (out-of-tree region, race between dirty marking and paint, etc.) would otherwise show transparent black at composition time, blended onto the compositor load-clear (see I.4) — visible as black.

#### I.3 — Conditional framebuffer clear must mirror C++

**Rule.** Whether `emView::Paint` clears the framebuffer (and to what color, with what canvas color) is a function of `SupremeViewedPanel`, `SVP.IsOpaque()`, `SVP.ViewedX/Y/Width/Height`, `SVP.CanvasColor`, the painter's clip rect, and `view.BackgroundColor`. The Rust port must compute this clear identically — same condition, same color, same canvas-color argument — for every render strategy (per-tile, parallel, single-buffer fallback). The clear must occur in user space within `view.Paint`, not in the strategy wrapper.

**C++ reference.** `emView.cpp:1062-1084`:
```cpp
if (!SupremeViewedPanel) {
    painter.Clear(BackgroundColor, canvasColor);
}
else {
    // ...
    if (!p->IsOpaque() ||
        p->ViewedX > rx1 || p->ViewedX+p->ViewedWidth  < rx2 ||
        p->ViewedY > ry1 || p->ViewedY+p->ViewedHeight < ry2)
    {
        ncc = p->CanvasColor;
        if (!ncc.IsOpaque()) ncc = BackgroundColor;
        painter.Clear(ncc, canvasColor);
        canvasColor = ncc;
    }
}
```
Rust mirror: `emView.rs:4688-4738`.

**Why.** This conditional is the *only* mechanism by which C++ guarantees the framebuffer is in a known opaque state before non-opaque panels paint. Any divergence — clear skipped where C++ would clear, clear performed where C++ would not, wrong color, wrong canvas-color argument — opens a hole through which the pre-fill state of I.1/I.2 (or the load-clear of I.4) becomes visible.

#### I.4 — Compositor load-clear color must not be observable

**Rule.** The `LoadOp::Clear(wgpu::Color::BLACK)` on the wgpu render pass (`emViewRendererCompositor.rs:261`) must not produce any visible black pixel. Either (a) every viewport pixel must be covered by an opaque tile fragment in the same render pass, or (b) the load-clear color must be set to `view.background_color` (or any color equivalent in observable effect to C++ behavior — currently identical to grey-everywhere, since C++ would have cleared to `BackgroundColor` for any pixel not covered by an opaque SVP).

**C++ reference.** No analogue. C++ has no compositor pass. The rule exists to neutralize a Rust-only render-pass clear.

**Why.** If any tile texture has alpha < 255 anywhere (e.g., tile init color RGBA `(0,0,0,0)` per I.2, or a partially-painted tile), that pixel composites the tile RGB over the load-clear black, producing a visible black contribution proportional to `(1 − tile_alpha)`. The rule has two acceptable resolutions; the implementation plan picks one.

#### I.5 — Runtime `view.background_color` changes propagate

**Rule.** Setting `view.background_color` to a new value at runtime (`emView::SetBackgroundColor`, `emView.rs:3508-3516`) must cause the next presented frame to use the new color anywhere C++ `emView::Paint` would write `BackgroundColor` (the `Clear` calls of I.3, and the `ncc = BackgroundColor` fallback at `emView.cpp:1081`). The change must propagate through any cached intermediate (tile cache, single-buffer viewport buffer, GPU textures of empty regions) within one frame of the change.

**C++ reference.** `emView.cpp:1063, 1081-1082`. C++ reads `BackgroundColor` from the view state on every paint; there is no cache to invalidate.

**Why.** The compositor introduces caches that C++ does not have. A change to `background_color` must invalidate enough state that the new color reaches the framebuffer next frame. (If `SetBackgroundColor` already calls `InvalidatePainting`, this rule is trivially satisfied at the dirty-region layer; the rule still has to hold for the compositor load-clear of I.4 if that path is the resolution chosen.)

### Cluster II — Canvas-color propagation

**Invariant.** Every `emPanel::Paint` call (and every `emPainter::Paint*` call inside it) must receive exactly the canvas-color value that C++ `emView::Paint` would pass at the same point in the panel tree walk. Canvas color is a per-call argument, not a per-painter member; it changes value during dispatch and must be threaded explicitly. The compositor's tile boundaries are not visible to canvas color — a panel's canvas-color value is the same regardless of which tile its pixels land in.

#### II.1 — `view.Paint` receives the OS-driver canvas color

**Rule.** The `canvas_color` argument passed into `emView::Paint` by the renderer must equal the value C++'s OS driver would pass. For Rust the renderer currently passes `emColor::TRANSPARENT` from all three render strategies (`emWindow.rs:635, 679, 736` and `emViewRenderer.rs:41, 64`). This must equal the C++ initial value, which is the OS driver's choice; current understanding is that this is non-opaque (zero or transparent) on the platforms targeted. **Open question O.1 — confirm against C++ X11 driver.**

**C++ reference.** Caller of `emView::Paint`: outside emCore, in OS driver code (e.g., `src/emCore/emWindow.cpp` or platform-specific subclass). Default is non-opaque.

**Why.** The initial canvas-color is the seed of the propagation chain in II.2-II.4. If it diverges, every subsequent rule fires from a wrong starting state.

#### II.2 — SVP receives the conditionally-updated canvas color

**Rule.** When `emView::Paint` performs the conditional clear of I.3 with color `ncc`, it must update `canvas_color = ncc` *before* dispatching to the SVP, so the SVP's `Paint` call receives `ncc` as its `canvasColor` argument. When no clear is performed, the SVP receives the original `canvas_color` unchanged.

**C++ reference.** `emView.cpp:1083` (`canvasColor=ncc;` immediately after the clear, before `p->Paint(pnt,canvasColor);` at line 1098). Rust mirror: `emView.rs:4738`.

**Why.** The SVP is told what canvas color it can rely on. If the clear painted `BackgroundColor` underneath, the SVP must know — otherwise its `PaintTextBoxed` etc. fall back to the slow non-opaque blend formula and may pick wrong anti-aliased pixel values at edges (see emPainter.h canvas-color discussion).

#### II.3 — Children receive their own `CanvasColor`

**Rule.** When `emView::Paint` recursively dispatches to non-SVP panels, each child must be passed `child.CanvasColor` (its own per-panel canvas color), not the parent's canvas color, not the view background color, and not whatever was used for the SVP.

**C++ reference.** `emView.cpp:1118` — `p->Paint(pnt, p->CanvasColor);`. Rust mirror: see the analogous line in `emView.rs` (the per-child paint dispatch loop following the SVP paint).

**Why.** Child panels declare their own canvas color separately from their parent — a panel painted on a known-blue background announces `CanvasColor = blue` so its own `PaintTextBoxed` calls can use the optimized formula. Threading the parent's color instead would silently misrender any child whose declared canvas color differs.

#### II.4 — Tile boundaries do not perturb canvas color

**Rule.** A panel that straddles two tiles must have its `Paint` method called once per tile (or once total, in the parallel record/replay path) with the same `canvas_color` argument in both calls. The argument value must be the C++-equivalent value (II.2 or II.3 as appropriate). The tile decomposition is not visible to the panel — the panel does not know it is being painted into a tile.

**C++ reference.** No tiles in C++; canvas color is determined entirely by panel-tree position. The rule preserves that "panel-tree position determines canvas color" invariant under tiling.

**Why.** Per-tile renderers (the default and single-buffer-fallback paths) call `view.Paint` once per dirty tile with a translated painter. Each such call re-derives the canvas-color chain from scratch through `emView::Paint`'s SVP logic. Both calls must produce the same canvas color for the same panel; otherwise the panel's pixels in tile A and tile B would blend differently, and the visible seam would betray the tile boundary.

#### II.5 — `emPainter` is not a canvas-color carrier

**Rule.** Canvas color is not stored on `emPainter` and is not retrieved via a getter from inside a `Paint` method. It is read from the explicit `canvasColor` parameter the panel received and passed (unchanged or replaced with one of the panel's own choosing) to each `painter.Paint*` call.

**C++ reference.** `include/emCore/emPainter.h:40-200` defines no `GetCanvasColor`/`SetCanvasColor` member. `include/emCore/emPanel.h:626, 645-654` documents canvas color as a `Paint` parameter. `src/emCore/emFilePanel.cpp:200-410` consistently passes the parameter through to `painter.PaintTextBoxed(...,canvasColor,...)`.

**Why.** Initial scan suggests the Rust port has a `painter.GetCanvasColor()` accessor and at least one panel paint method (`emFilePanel::paint_status`) reads canvas color from the painter rather than from a method parameter — implying canvas color may be carried as a painter member set by the caller. If true, this is a structural divergence: any panel that reads `GetCanvasColor()` would receive whatever was last set on the painter, not the value the dispatcher meant for *this* panel. If painter-as-carrier is preserved, there must be a mechanism that updates the painter's stored canvas color exactly at the points where C++ would have changed the parameter (see II.2, II.3). **Open question O.2 — confirm the carrier mechanism and verify dispatcher updates.**

### Cluster III — Non-opaque composition

**Invariant.** When a panel's `IsOpaque()` returns `false` and its `Paint` method writes pixels with alpha < 255 (or skips pixel writes entirely), what shows through must be exactly what C++ would show through: the panel's parent's pixels, painted by the parent's `Paint` method (or `view.background_color` cleared by `emView::Paint`'s conditional). The compositor's tile decomposition, parallel replay, and GPU composition pass must not introduce any other source of "showing through."

#### III.1 — Non-opaque SVP reveals view background

**Rule.** When the SVP is non-opaque (the `IsOpaque()==false` branch of I.3's conditional), the framebuffer has been cleared to `view.background_color` (or the SVP's own `CanvasColor` if opaque, per `emView.cpp:1080-1082`) before the SVP paints. Any pixel the SVP does not write opaquely must end up showing that clear color, not the tile init color, not the compositor load-clear color, not stale prior-frame content.

**C++ reference.** `emView.cpp:1073-1083` (the conditional clear), `emFilePanel.cpp:187-197` (`IsOpaque` returns `false` during loading), `emFilePanel.cpp:200-235` (`Paint` during `VFS_WAITING`/`VFS_LOADING` calls only `PaintTextBoxed`, leaving most pixels untouched).

**Why.** This rule is the direct mechanical statement of F018's repro: when an emFilePanel is the SVP during loading, C++ shows grey (background color) under the centered "Loading: NN%" text. The contract is satisfied iff the Rust pipeline produces the same grey.

#### III.2 — Non-opaque child reveals parent

**Rule.** When `emView::Paint`'s recursive walk dispatches a non-opaque child panel after its parent has been painted, the child's non-written pixels must show the parent's pixels (which are already in the framebuffer at that point), not any tile init / pre-fill / load-clear color introduced by the compositor.

**C++ reference.** `emView.cpp:1100-1135`. The recursive walk paints parent first (line 1098 for SVP, line 1118 for descendants), then descends into children. The framebuffer state when `child->Paint` is called contains the parent's pixels.

**Why.** The compositor's per-tile path re-walks the tree from scratch per tile. As long as the tree walk per tile is identical (visits parent before child, paints both), the parent's pixels are present before the child paints, regardless of which tile they're in. The rule confirms that this invariant survives the tile decomposition.

#### III.3 — Opaque-panel skip-clear remains valid under tiles

**Rule.** When `emView::Paint` skips the conditional clear because the SVP is opaque-and-covering, the framebuffer's prior state is irrelevant in C++ because the SVP overwrites it entirely. Under tiles, "irrelevant" must remain true: no Rust-only color (tile init, pre-fill, load-clear) may peek through a fully-painted opaque panel.

**C++ reference.** `emView.cpp:1073-1084` (the `if !p->IsOpaque() || ...` condition — the entire clear block is skipped when the panel is opaque AND covers the rect).

**Why.** This rule is what makes the I.1 pre-fill rule load-bearing in the worst case: when the C++ fast path (skip clear) applies, Rust must still produce zero observable Rust-only color. This is the case where the `viewport_buffer.fill(BLACK)` / `tile.image.fill(BLACK)` / `LoadOp::Clear(BLACK)` is most likely to leak through, because the contract that "downstream will overpaint" depends on the panel actually being opaque-and-covering AND having no non-opaque descendants whose holes expose the buffer beneath.

### Cluster IV — Dirty-region soundness

**Invariant.** A pixel is in the dirty region for frame N iff its presented value in frame N may differ from its presented value in frame N−1. Every cause of a possible difference (panel content change, panel opacity change, panel painted region shrinking, canvas-color change, view-background change, panel layout change, focus/highlight change) must mark a containing rect dirty. No clean (non-dirty) tile may reuse stale GPU content if its pixels would have differed under C++ semantics.

#### IV.1 — `InvalidatePainting` propagates to tile cache and compositor

**Rule.** When `emPanel::InvalidatePainting()` (or its rect-arg form) is called, every tile whose pixel coverage intersects the panel's clip rect (in viewport coordinates, after panel-to-viewport transform) must have its `dirty: bool` set to `true`, and the next render cycle must re-paint those tiles and re-upload them via `WgpuCompositor::upload_tile` before `render_frame` draws.

**C++ reference.** `src/emCore/emPanel.cpp:1282-1311`. C++ accumulates rects on the view; the OS driver consumes them on next paint. Rust analogue: `emWindow::mark_dirty_rect` (`emWindow.rs:1575+`) translates a viewport rect into tile dirty bits.

**Why.** Without this rule, content changes are invisible until something else triggers a re-upload of the affected tiles.

#### IV.2 — Painted region shrinking invalidates the difference

**Rule.** When a panel transitions from a state where it paints a region R₁ to a state where it paints a smaller region R₂ ⊊ R₁ (e.g., `VFS_LOADED` → `VFS_LOADING` may shrink the painted area), the panel must invalidate R₁, not R₂ — i.e., the difference R₁ \ R₂ must be marked dirty so the framework can repaint it (with parent / view-background / `Clear` content per Cluster III).

**C++ reference.** Same `emPanel::InvalidatePainting` mechanism; the panel itself is responsible for calling it when its painted output changes. C++ relies on the panel knowing its prior painted region; framework simply repaints from scratch within the dirty rect.

**Why.** If the panel only invalidates its current (smaller) painted region, the prior-frame pixels in R₁ \ R₂ stay in the framebuffer (or in the tile texture) as ghost content. This is the kind of failure mode the contract has to surface even though it doesn't manifest in F018's specific repro.

#### IV.3 — `IsOpaque` change invalidates SVP-choice path

**Rule.** When a panel's `IsOpaque()` return value changes, the framework must re-evaluate the SVP choice (the C++ `SVPChoiceByOpacityInvalid` flag in `emView`) and re-paint enough of the framebuffer that the new opacity status is reflected — including the conditional clear of I.3, which depends on `SVP.IsOpaque()`.

**C++ reference.** `src/emCore/emPanel.cpp:1284-1290` and `1296-1302` — both `InvalidatePainting` overloads set `View.SVPChoiceByOpacityInvalid = true`. `emView.cpp:1073` reads `IsOpaque()` on every paint.

**Why.** A panel that becomes non-opaque opens up the conditional clear path. If the tile cache caches the prior-frame "opaque, no clear needed" pixels, the next frame would still skip the clear and the un-painted region would show stale content. The dirty propagation has to invalidate not just the panel's own pixels but the framebuffer state that the panel's opacity gates.

#### IV.4 — All three render strategies obey the dirty contract identically

**Rule.** The single-buffer fallback (`emWindow.rs:628-652`), the parallel replay (`emWindow.rs:653-667`, `emWindow.rs:713+`), and the per-tile path (`emWindow.rs:668-687`) must all repaint exactly the dirty tiles, leave clean tiles untouched (their GPU textures retain prior-frame content), and produce identical observable output for the same dirty set.

**C++ reference.** No analogue. The rule preserves C++'s "one paint per frame produces all visible pixels" invariant under three Rust-only paint strategies.

**Why.** The strategy choice is a Rust-only optimization (selection in `emWindow.rs:628, 653, 668`). If the strategies disagree — e.g., the single-buffer fallback re-walks every panel including ones the per-tile path would consider clean — the visible output may differ frame-to-frame depending only on dirty count, not on actual content. That divergence violates port fidelity.

#### IV.5 — Recording-painter ops must record the conditional clear

**Rule.** The display list recorded in `render_parallel_inner` (`emWindow.rs:732-736`) must include the conditional clear of I.3, so that the per-tile replay produces identical pixel output whether or not the SVP is opaque-and-covering.

**C++ reference.** `emPainter::Clear` (`emPainter.cpp:364-374`) is one of the paint primitives. Rust recording painter must record `Clear` ops with the same color and canvas-color arguments.

**Why.** If the recording painter omits `Clear` from its op log (e.g., if `Clear` is implemented as a special path that bypasses the recording channel), per-tile replay never sees the clear, and every tile's per-tile pre-fill BLACK becomes the visible background. This is one specific way I.1 can be violated in the parallel path even if the per-tile path is fine.

### Cluster V — Acceptance criteria

#### V.1 — F018 repro: `VFS_WAITING`/`VFS_LOADING` background is grey

**Acceptance.** Launch the application, zoom into a directory panel, observe during the `VFS_WAITING` and `VFS_LOADING` phases. The area outside the centered "Wait..." / "Loading: NN%" text must render as `view.background_color` (currently `0x808080` grey, set at `emView.rs:664`), not black. Visual comparison against C++ Eagle Mode 0.96.4 zooming into the same path must show no perceptible color difference in the loading background. This is the symptom that surfaced F018; satisfying every cluster I-IV rule is necessary for V.1, but V.1 alone is not sufficient evidence the contract is fully obeyed (other rule violations may not visibly manifest).

#### V.2 — Background-color change visibly propagates

**Acceptance.** Setting `view.background_color` at runtime to a distinct test color (e.g., `0xFF0000FF` red) and triggering a repaint must cause the next presented frame to show the new color anywhere C++ would have used `BackgroundColor` — including in regions exposed by non-opaque panels. Verifies I.5.

#### V.3 — Strategy parity

**Acceptance.** Forcing each render strategy in turn (single-buffer fallback by marking >50% tiles dirty, parallel replay by enabling multi-thread + multi-dirty, per-tile by single-thread or single-dirty) must produce pixel-identical (within established golden tolerances per `tests/golden/common.rs`) presented framebuffers for the same panel-tree state. Verifies IV.4.

#### V.4 — Painted-region shrink shows no ghost

**Acceptance.** Constructing a panel that paints opaquely in region R₁ for frame N then paints opaquely in R₂ ⊊ R₁ for frame N+1 (with the parent's pixels visible under R₁ \ R₂) must show parent pixels in R₁ \ R₂ at frame N+1, not ghost content from frame N. Verifies IV.2.

#### V.5 — Opacity transition rebuilds framebuffer

**Acceptance.** A panel that returns `IsOpaque() == true` at frame N and `false` at frame N+1, with non-trivial parent content underneath, must render the parent content under the now-non-opaque panel's holes at frame N+1. Verifies III.3 + IV.3.

Additional acceptance criteria may emerge during the audit (out of scope here) and should be appended to this section in a follow-up rather than discovered ad-hoc during implementation.

## 5. Out of Scope / Follow-ups

- **Audit.** Per-rule comparison of current Rust code to the contract. Belongs in the implementation plan that follows this spec.
- **Remediation.** Code changes to bring violations into compliance. Implementation plan.
- **Compositor performance redesign.** Tile size, eviction, parallel threshold, recording-painter mechanism — all preserved as-is.
- **Other RUST_ONLY architectural additions** (engine scheduler, `emRenderThreadPool`, `emPanelTree` arena) — separate concerns; if any of them perturbs paint output, file as a sibling F-issue.
- **Display-list semantics for non-paint operations** (input dispatch, focus, cursor) — orthogonal pipelines.

## 6. Open Questions

These are points where the contract requires a C++ behavior that the spec author could not pin down with confidence from the current source read. They are parked here for the implementation plan to resolve, not guessed.

- **O.1.** What value does the C++ OS driver pass as `canvasColor` to the top-level `emView::Paint` call? (Rule II.1.) Likely transparent / non-opaque on X11, but needs confirmation in `src/emCore/emWindow.cpp` and platform-specific subclasses.
- **O.2.** Does the Rust `emPainter` carry canvas color as a member (the `paint_status` pattern in `emFilePanel.rs` and the `painter.GetCanvasColor()` accessor suggest it does), and if so where is it set/updated relative to C++'s explicit-argument threading? (Rule II.5.) Affects whether II.5 needs a "remove the carrier" remediation or a "update the carrier at the right points" remediation.
- **O.3.** In the per-tile single-threaded path (`emWindow.rs:668-687`), does `painter.translate(-(col*ts), -(row*ts))` produce a painter whose clip rect is the tile bounds (so `Clear` covers exactly the tile), or the viewport bounds (so `Clear` covers viewport-sized area but is masked by tile target)? Affects how Rule I.3's clear interacts with the per-tile pre-fill.
- **O.4.** Does the recording painter in `render_parallel_inner` already record `Clear` ops, or does it bypass them? (Rule IV.5.) A grep in `emPainterDrawList.rs` will answer this.
- **O.5.** What does `WgpuCompositor` do for tiles that are out of the active grid (resized smaller, or never allocated)? Does the load-clear black show through, or is the alpha-blend pipeline configured to mask it? (Rule I.4 / I.2.)
