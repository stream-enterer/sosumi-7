# Phase 3.5.A — Runtime Top-Level Windows + Per-emWindow PanelTree

**Date:** 2026-04-21
**Branch:** `port-rewrite/phase-3-5-a-runtime-toplevel-windows` (off `1e393d2f` on Phase 3.5, tagged `port-rewrite/phase-3-5-partial-checkpoint-before-3-5-a`)
**Prereq for:** Phase 3.5 Task 5 (`emDialog` reshape).
**JSON entries:** none opened/closed directly; unblocks E024 path via Phase 3.5.

## Problem

Phase 3.5 Task 5's `emDialog = emWindow + DlgPanel-root + DialogPrivateEngine` shape requires three capabilities the codebase currently lacks:

1. **Multiple PanelTrees.** `App::tree: PanelTree` is a single tree; `PanelTree::create_root` asserts exactly one root (emPanelTree.rs:479-483). Each dialog needs its own root.
2. **Runtime top-level window install.** `App::windows: HashMap<WindowId, emWindow>` has no runtime-add path; insertions happen only at startup via `App::run` + setup callback. `DeferredAction` at emEngineCtx.rs:33-42 has `CloseWindow` and `MaterializePopup` only.
3. **Top-level-window constructor callable mid-Cycle.** `emWindow::new_popup_pending` exists for popups; no analog for top-level dialogs. `emWindow::create` requires `&ActiveEventLoop`, unavailable inside engine Cycles.

Investigation at commit `1e393d2f` confirmed all three constraints are tight.

## Scope

Unblock Phase 3.5 Task 5 in one atomic sub-phase. Migrate home, popup, and dialog windows to a uniform per-emWindow tree model. `App::tree` removed entirely. No 3.5.B follow-up; popup migration is included.

Explicitly out of scope: `emDialog` reshape itself (that's the Task 5 consumer), multi-monitor work, `emSubViewPanel` changes (already correct).

## Architecture

### Per-emWindow PanelTree

Each `emWindow` owns its `PanelTree` as a struct field, built via `PanelTree::new_with_location(...)` in every ctor. `root_panel: PanelId` remains, now referencing a root inside `self.tree`.

Precedent: `emSubViewPanel` already owns its `sub_tree: PanelTree` (emSubViewPanel.rs:23), lifted here from "sub-view container" to "window container." Matches C++ `emView::RootPanel` ownership.

`App::tree` deleted. `App::windows` becomes the sole tree storage, keyed by WindowId.

### Scheduler dispatch — take/put tree swap

Engines are registered with a `WindowId` identifying their owning window (stored alongside or inside `TreeLocation`). At Cycle dispatch time, the scheduler:

1. Takes the owning window's tree out of `windows[wid].tree` via `mem::take` (requires `PanelTree: Default`).
2. Walks the detached tree via existing `dispatch_with_resolved_tree` logic (`Outer` → tree directly; `SubView` → take/put behavior chain, unchanged).
3. Calls `behavior.Cycle(&mut ctx)` with `ctx.tree = resolved` and `ctx.windows = windows`. During Cycle, `windows[own_wid].tree` holds a sentinel (empty `Default` tree).
4. After Cycle, restores the real tree via `windows[own_wid].put_tree(tree)`.

**Invariant:** during a Cycle, code does not read `ctx.windows[own_wid].tree`. The engine's tree is `ctx.tree`. Other windows' trees (`ctx.windows[other_wid].tree`) remain intact.

This is the exact shape of the existing SubView dispatch (emScheduler.rs:138-169): the behavior-slot of the owner `emSubViewPanel` is detached during recursion; code doesn't re-enter it. Lifted one level to per-window trees.

`PanelTree: Default` added — trivial (empty SlotMaps). Used only as the sentinel during dispatch swap; never surfaced to callers.

### `TreeLocation` extension

`TreeLocation` itself is unchanged:

```rust
pub enum TreeLocation {
    Outer,  // "root of the engine's registered window's tree"
    SubView { outer_panel_id: PanelId, rest: Box<TreeLocation> },
}
```

Semantic shift: `Outer` previously meant "root of App::tree." Post-3.5.A it means "root of the engine's owning window's tree." `SubView` recurses identically to today.

Scheduler bookkeeping: new `engine_windows: SecondaryMap<EngineId, WindowId>` parallel to existing `engine_locations`, populated at `register_engine` time. Kept separate from `TreeLocation` to avoid reshaping a widely-used type; the pair `(engine_windows[id], engine_locations[id])` identifies the engine's tree + position.

No `TreeLocation::Window` cross-window variant introduced — no consumer identified for 3.5.A. If cross-window engine placement is ever needed, it can be added later without migration cost.

### `EngineCtx` changes

`EngineCtx::tree: &mut PanelTree` remains — resolved by scheduler dispatch as today. `EngineCtx::windows` remains. `DoTimeSlice` signature drops the standalone `tree: &mut PanelTree` parameter; windows carry the trees.

Engine Cycle code that uses `ctx.tree` compiles unchanged. Engine code that previously did nothing problematic with `ctx.windows` stays fine. Only rule: during a Cycle, don't reach into your own window's `.tree` through the windows map — use `ctx.tree`.

### Popup migration

`emWindow::new_popup_pending` stops taking `root_panel: PanelId` from caller. Instead, constructs its own `PanelTree::new_with_location(...)` and calls `create_root` on it. The popup's panels live in the popup's tree, not in the launching view's tree.

`emView::RawVisitAbs` (emView.rs:1936-1949) popup-enter code drops the `self.root` argument pass-through. The popup becomes fully independent.

### Runtime top-level install path

New constructor `emWindow::new_top_level_pending(parent_context, title, flags, signals, look_bg_color) -> Self`, mirroring `new_popup_pending`'s shape but with top-level `WindowFlags`. Callable anywhere; returns `emWindow` in `OsSurface::Pending` with its own `PanelTree`.

New App fields:

- `pending_top_level: Vec<PendingTopLevel>` where `struct PendingTopLevel { dialog_id: DialogId, window: emWindow }`.
- `dialog_windows: HashMap<DialogId, WindowId>`.
- `next_dialog_id: u64` counter.

New type:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DialogId(u64);
```

Allocated by an `App::allocate_dialog_id()` helper (or exposed via a threaded mutable reference — TBD during implementation, whichever preserves borrow hygiene).

Framework drain closure `App::install_pending_top_level(event_loop)` — analogous to `materialize_pending_popup`. Enqueued via existing `pending_framework_actions` closure queue on the caller side (same mechanism popup uses). Drain:

1. For each `PendingTopLevel`: create winit window + wgpu surface, extract WindowId.
2. Set `OsSurface::Materialized` on the emWindow in place.
3. Move `window` into `App::windows[wid]`.
4. Record `dialog_windows[did] = wid`.
5. Wire viewport, geometry, focus as `materialize_pending_popup` does.

### Dialog handle resolution

Consumer (`emDialog` in Phase 3.5 Task 5) holds a `DialogId`. All ops resolve via:

- `dialog_windows.get(&did).and_then(|wid| windows.get_mut(wid))` — matured path.
- If miss: scan `pending_top_level` for the matching entry. Pending dialogs still have their emWindow + tree in the pending vec.

Helper on `App`:

```rust
pub(crate) fn dialog_window_mut(&mut self, did: DialogId)
    -> Option<DialogWindowRef<'_>>;
```

where `DialogWindowRef` is an enum `Pending(&mut emWindow) | Materialized(WindowId, &mut emWindow)` or similar. Implementation detail.

### Teardown

`emDialog::deregister` routes to one of:

- **Pending:** find index in `pending_top_level`, remove it. Deregister the private engine (signals free). Tree + emWindow drop locally.
- **Materialized:** push `DeferredAction::CloseWindow(wid)`. Deregister the private engine. On next framework drain, the emWindow (with its tree) is removed from `App::windows` and `App::dialog_windows`.

Auto-delete flow unchanged: `DialogPrivateEngine::Cycle` countdown pushes `CloseWindow(wid)`. Framework drain handles removal.

## Data flow

### Dialog open (mid-Cycle, e.g., button-click callback)

1. Caller invokes `emDialog::new(...)` with threaded references (scheduler, pending queue, pending_framework_actions, root_context).
2. Ctor allocates signals, builds `emWindow::new_top_level_pending(...)` with its own tree, installs DlgPanel as root behavior, registers `DialogPrivateEngine` at `Priority::High` connected to `close_signal`.
3. Ctor pushes `PendingTopLevel { dialog_id, window }` onto `pending_top_level`.
4. Ctor enqueues closure `fw.install_pending_top_level(el)` onto `pending_framework_actions`.
5. Returns façade `{ dialog_id, root_panel_id, private_engine_id, finish_signal, close_signal, ... }`.
6. Current time slice completes. `App::about_to_wait` fires; closure drains pending queue → materializes winit surface → populates `App::windows` and `App::dialog_windows`.
7. Subsequent winit events for `wid` route normally.

### Dialog close (user clicks X)

1. winit `CloseRequested` → `close_signal` fires on the dialog's emWindow.
2. Next slice: `DialogPrivateEngine::Cycle` observes `close_signal`, sets `pending_result = Cancel`, finalizes, fires `finish_signal`, invokes `on_finish` / `on_finished`.
3. If `auto_delete`: countdown machine runs per C++ `PrivateCycle` (already implemented in Phase 3.5 Task 4). At the final step, pushes `DeferredAction::CloseWindow(wid)`.
4. Framework drain removes emWindow; `dialog_windows.remove(&did)`.

### Popup open (existing path, post-migration)

1. `emView::RawVisitAbs` popup-enter detects outside_home, no existing popup.
2. Builds `emWindow::new_popup_pending(...)` — now constructs own tree + root internally.
3. Stores in `launching_view.PopupWindow`.
4. Enqueues existing popup materialization closure. Unchanged thereafter.

## Error handling

- `emDialog::new` requires scheduler access at compile time (function signature).
- Ops on a deregistered `DialogId`: lookup miss → return default (mirrors current emDialog semantics).
- Pending dialog whose caller drops it before materialize: façade's `deregister` must run (analogous to popup's same-frame-enter-exit handling). If the caller leaks the façade, the pending vec retains the emWindow; we document but don't enforce — matches current popup cancellation semantics.

## Testing

Unit tests:
- `emWindow::new_top_level_pending` constructs in `OsSurface::Pending`, has a valid non-empty tree with exactly one root.
- `App::install_pending_top_level` drain: pending queue empties, `App::windows` grows by one, `dialog_windows` populated with the returned WindowId.
- `TreeLocation::Window` or per-engine WindowId registration resolves to correct tree during scheduler dispatch.
- Scheduler take/put swap: a Cycle that accesses `ctx.tree` sees the engine's own tree; after Cycle, `windows[own_wid].tree` is restored with no data loss.
- `PanelTree::default()` produces an empty tree; constructing + taking + putting preserves content (`mem::take` roundtrip test).

Integration:
- Popup path unchanged behaviorally: existing popup tests stay green after `new_popup_pending` owns its tree.
- Two concurrent dialogs: open two via `emDialog::new`, verify independent WindowIds, independent trees, independent engines, independent close lifecycles.
- Cross-window dispatch: an engine in window A can read `ctx.windows[B]` (including `B.tree` if it wanted to, though doing so during its own Cycle is permitted since only A's tree is detached). Regression-guard test for this invariant.

Regression:
- All Phase 3.5 Tasks 2-4 tests (DlgPanel/DlgButton/DialogPrivateEngine) stay green. They use test-local trees and don't route through App-level dispatch — unaffected.
- Goldens 237/6 preserved (no paint-path changes).

## Migration task outline (writing-plans will formalize)

1. `PanelTree: Default` — implement empty-state ctor alongside existing `new` / `new_with_location`.
2. `emWindow::tree: PanelTree` field + take/put helpers. Every ctor builds its own tree.
3. Scheduler migration: `DoTimeSlice` drops `tree` param; `dispatch_with_resolved_tree` extended; engine registration records WindowId; per-engine tree resolution via take/put swap.
4. Home window: startup code in `App::new` / setup-callback path migrates from `App::tree` to constructing the home `emWindow`'s tree directly.
5. Popup migration: `new_popup_pending` owns tree; `emView::RawVisitAbs` drops `self.root` arg.
6. Top-level install: `new_top_level_pending` ctor + `App::pending_top_level` + drain closure + `DialogId` type + resolution helpers.
7. Test-site migration: every `EngineScheduler::DoTimeSlice` call-site (~20 sites in emScheduler.rs tests plus app-level) updates signature.
8. Full gate: clippy + nextest + goldens.

## Exit criteria

- `rg -n 'pub tree: PanelTree' crates/emcore/src/emGUIFramework.rs` → 0 (App::tree deleted).
- `rg -n 'tree: PanelTree' crates/emcore/src/emWindow.rs` → ≥1 (field present on emWindow).
- `rg -n 'pending_top_level' crates/emcore/src/emGUIFramework.rs` → ≥1 (install path present).
- `rg -n 'impl Default for PanelTree' crates/emcore/src/emPanelTree.rs` → 1.
- nextest: baseline (2483 at branch start) +N for new tests; 0 failed; 9 skipped.
- Goldens 237/6 preserved.

## Risks

- **Scheduler dispatch rewrite is central infrastructure.** Every test that constructs a `PanelTree` and registers an engine (~20 sites) migrates. Mechanical; low conceptual risk since take/put shape is established.
- **`PanelTree: Default` surprises.** If `PanelTree::new` does load-bearing initialization beyond empty SlotMaps, `Default` must not replicate that (sentinel must be cheap + never surfaced to callers). Verified during implementation's first step.
- **Popup behavioral regression.** Popup migration is contained but touches live code paths. Mitigation: existing popup tests must stay green at every step; bisectable task breakdown.
- **Borrow-hygiene around `DialogId` allocation.** Threading a counter through ctor args vs exposing via `&mut App` vs centralizing on `App::allocate_dialog_id()`. Chosen during implementation per site-specific borrow constraints.

## Precedent compliance

- Tree-per-view-like-container: matches `emSubViewPanel.sub_tree`.
- Take/put dispatch: matches existing `dispatch_with_resolved_tree` for SubView.
- Stable-id handles (DialogId): matches EngineId/PanelId/SignalId patterns throughout.
- Pending→Materialize via `pending_framework_actions` closure queue: matches popup path (`materialize_pending_popup`).
- No `Rc<RefCell>` proliferation, no `Any`/downcast, no `unsafe`-for-convenience. `mem::take` usage matches 17+ existing sites.
