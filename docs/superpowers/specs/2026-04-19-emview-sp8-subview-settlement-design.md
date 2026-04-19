# SP8 — Sub-view synchronous-settlement divergence: Design Spec

**Date:** 2026-04-19
**Predecessor:** SP4.5 (empanel-engine-registration); closed out 2026-04-19.
**Closeout reference:** `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.1 item 17.

## 1. Problem

`emSubViewPanel::Paint` (Rust) drives a 50-iteration synchronous settlement loop — `PanelTree::run_panel_cycles` + `emView::HandleNotice` + `emView::Update` + animator ticks — inside the sub-tree's Paint call. C++ `emSubViewPanel::Paint` (`src/emCore/emSubViewPanel.cpp:94-110`) does no such thing: it just delegates to `SubViewPort->PaintView(...)`. In C++, settlement is a cross-frame concern driven by the shared scheduler via `emContext`.

Rust keeps this divergence alive because:
- `emSubViewPanel` owns a nested `PanelTree` (`sub_tree`) distinct from the parent view's tree; `EngineCtx::tree` is singular, so one scheduler cannot cycle engines across two trees.
- The sub-view has no `winit::WindowId`, so existing `UpdateEngineClass` / `VisitingVAEngineClass` (both key on `window_id` via `ctx.windows`) cannot locate the sub-view's `emView`.
- `PanelTree::cycle_list` / `Cycle` / `cancel_cycle` / `run_panel_cycles` remain as the only settlement mechanism for the sub-tree.
- The golden `tests/golden/composition.rs::settle()` helper depends on the same API.

## 2. Goal

Bring sub-view settlement into line with C++ observable behavior — settlement occurs across frames, driven by a scheduler — while respecting the forced structural divergence of nested `PanelTree` ownership in Rust.

Deletions on success: `PanelTree::cycle_list`, `Cycle`, `cancel_cycle`, `run_panel_cycles`. Synchronous interleave inside `emSubViewPanel::Paint`: gone. New `DIVERGED:` blocks documenting per-sub-view scheduler and ActiveAnimator placement.

## 3. Design

### 3.1 Per-sub-view scheduler (forced divergence)

`emSubViewPanel` gains `sub_scheduler: Rc<RefCell<EngineScheduler>>`. The sub-view attaches to `sub_scheduler` at `emSubViewPanel::new`; panels in `sub_tree` register `PanelCycleEngine` adapters on `sub_scheduler` via the existing `init_panel_view` + `register_pending_engines` path (SP4.5, already works once the sub-view has a scheduler).

This is a forced divergence from C++'s single shared scheduler. Documented with a `DIVERGED:` block at `emSubViewPanel::new` citing:
- Rust `EngineCtx::tree` is singular.
- Sub-view's panels live in a separate `PanelTree` from the parent view's.
- C++ `emContext` chain is not threaded through `emView` in Rust (SP7 scope, explicitly out of scope for SP8).

### 3.2 Sub-scheduler tick from outer PanelCycleEngine

SP4.5 already registers a `PanelCycleEngine` for the `emSubViewPanel` on the parent view's main scheduler. After SP8, `emSubViewPanel` implements `PanelBehavior::Cycle` — which that engine drives. `Cycle` does:

1. Tick `active_animator` (if any) with wall-clock `dt` (pattern borrowed from `VisitingVAEngineClass`).
2. Call `sub_scheduler.borrow_mut().DoTimeSlice(&mut sub_tree, &mut empty_windows)`.
3. Return `true` iff `sub_scheduler.has_awake_engines()` or `active_animator.is_some()` — so the outer scheduler keeps the `emSubViewPanel` engine awake as long as settlement work remains.

`empty_windows` is a `HashMap<WindowId, Rc<RefCell<emWindow>>>::new()` constructed locally per `Cycle`. Sub-view engines do not look up windows (see §3.3).

### 3.3 UpdateEngineClass / VisitingVAEngineClass refactor to view-direct lookup

Both engines currently hold a `window_id: WindowId` and look the view up via `ctx.windows.get(&window_id).view_mut()`. Refactor to hold `view: Weak<RefCell<emView>>` directly (matching `PanelCycleEngine`'s shape, SP4.5). Implementation:

- `UpdateEngineClass { view: Weak<RefCell<emView>> }` — `Cycle` upgrades the weak, calls `view.Update(ctx.tree)`, drains `pending_sched_ops`. Observable behavior identical.
- `VisitingVAEngineClass { view: Weak<RefCell<emView>>, last_cycle: Option<Instant> }` — same pattern; animator ticked via `view.VisitingVA`.

`emView::attach_to_scheduler` loses its `window_id` parameter (it becomes purely a `(view_weak, scheduler)` wiring call). Callers in `emMainWindow`, `emView::new_for_test` paths, and test harness helpers migrate mechanically.

Rationale (CLAUDE.md alignment): C++ `UpdateEngineClass` is an inner class of `emView` that has a direct pointer to the view (no window indirection). The Rust `window_id`-keyed variant was a Rust-only routing choice; converging on `Weak<RefCell<emView>>` moves toward C++ structure *and* unblocks sub-view registration. Not a new divergence — a pre-existing Rust-only accident being corrected.

### 3.4 `emSubViewPanel::Paint` simplification

After SP8, `Paint` matches C++ literally:
1. If `!state.viewed`, return.
2. Let `base_offset = painter.origin()`; get `bg` from sub-view.
3. Call `self.sub_view.borrow_mut().paint_sub_tree(&mut self.sub_tree, painter, self.sub_root(), base_offset, bg)`.

The `run_panel_cycles` + `HandleNotice` + `Update` + animator loop is removed.

### 3.5 Deletions

After §3.1–3.4:
- `PanelTree::cycle_list: Vec<PanelId>` field.
- `PanelTree::Cycle(&mut self, id: PanelId)`.
- `PanelTree::cancel_cycle(&mut self, id: PanelId)`.
- `PanelTree::run_panel_cycles(&mut self, current_pixel_tallness: f64)`.

These have no remaining callers in production (`emSubViewPanel::Paint` migrated) or tests (`composition.rs::settle` migrated — §3.6). Kani JSON inventories regenerate on next build.

### 3.6 Golden `settle()` helper rewrite

`tests/golden/composition.rs::settle` currently drives synchronous `HandleNotice + run_panel_cycles + Update`. C++ `gen_golden.cpp` uses `TerminateEngine ctrl(sched, 200)` — a real scheduler. Mirror that:

```rust
fn settle(tree: &mut PanelTree, view: &mut emView, rounds: usize) {
    // First call: attach scheduler if not already attached, register pending panel engines.
    if view.scheduler_ref().is_none() {
        let sched = Rc::new(RefCell::new(EngineScheduler::new()));
        view.attach_to_scheduler(sched.clone());
        tree.register_pending_engines();
    }
    let sched = view.scheduler_ref().unwrap().clone();
    let mut empty_windows = HashMap::new();
    for _ in 0..rounds {
        sched.borrow_mut().DoTimeSlice(tree, &mut empty_windows);
    }
}
```

(Exact shape may differ — the plan pins it.) Each caller's current `settle(&mut tree, &mut view, N)` signature stays the same; the implementation is what changes.

### 3.7 Active animator ticking

`emSubViewPanel::active_animator: Option<Box<dyn emViewAnimator>>` (structural divergence from C++ `emView::ActiveAnimator` already documented by SP1 §5.1 item 3) is ticked from `emSubViewPanel::Cycle` using a wall-clock dt (store `last_cycle: Option<Instant>` on the struct). Ticking inside Paint is removed.

## 4. What does NOT change

- `PanelTree::cycle_list` semantics as a "wake set" — replaced by scheduler's own awake-engines tracking (already in SP4.5).
- Notice dispatch — per-view `HandleNotice` inside `emView::Update` (SP5).
- `UpdateEngineClass::Cycle` body (SchedOp drain, popup-close-probe, etc.) — only the view-lookup mechanism changes.
- `emSubViewPanel::Input` / `notice` / `sync_geometry` / `drain_parent_invalidation` — unchanged.
- `emContext` threading — explicitly out of scope; handled by SP7 if/when motivated.

## 5. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Settle helper change breaks golden baseline due to different settlement timing | Re-run golden suite; same 237/6 baseline is the acceptance gate. Any new diffs require investigation and are not accepted silently. |
| UpdateEngine view-direct migration touches many test sites | Mechanical (one parameter removed). Covered by existing tests. |
| Sub-scheduler's `empty_windows` map surprises an engine that expects windows | Only `UpdateEngineClass` / `VisitingVAEngineClass` / `PanelCycleEngine` register on sub-scheduler; none use `ctx.windows` after §3.3 refactor. Assert nothing else registers by default. |
| `emSubViewPanel::Cycle` returning the wrong stay-awake decision stalls settlement or burns CPU | Stay-awake rule: `sub_scheduler.has_awake_engines() \|\| active_animator.is_some()`. Test: after sub-scheduler quiesces and animator finishes, outer engine must return `false` and go to sleep. |
| Deferred engine registration (`register_pending_engines`) needs to run for sub-tree panels added after `init_panel_view` | SP4.5 already handles this at the main tree; sub_tree uses the same mechanism. Verify no sub-tree code path creates panels without triggering `register_engine_for`. |

## 6. Acceptance criteria

1. `cargo-nextest ntr` — baseline +N new SP8 tests, 0 regressions.
2. Golden suite — 237/6 baseline unchanged (no new failures, no "false pass" from timing drift).
3. Smoke (`timeout 20 cargo run --release --bin eaglemode`) — exits 143 or 124; program stays alive.
4. `grep -rn "run_panel_cycles\|cycle_list\|cancel_cycle\|fn Cycle\b" crates/emcore/` returns only `PanelCycleEngine::Cycle`, `emPanel*::Cycle` trait impls, and `UpdateEngineClass/VisitingVAEngineClass/EOIEngineClass::Cycle` — no `PanelTree::` entries.
5. One new `DIVERGED:` at `emSubViewPanel::new` documenting per-sub-view scheduler.
6. `emSubViewPanel::Paint` body ≤ ~10 lines of rendering delegation (no settlement).
7. Closeout doc §8.1 item 17 marked closed; §8.0 SP8 row marked complete.

## 7. Phase outline (for plan)

- **Phase 1** — UpdateEngineClass / VisitingVAEngineClass view-direct migration. Gated: `cargo check`, full nextest, golden baseline.
- **Phase 2** — `emSubViewPanel` gets `sub_scheduler`; sub-view attaches; sub-tree panels register engines. Gated: sub-scheduler registration tests.
- **Phase 3** — `emSubViewPanel::Cycle` impl + `active_animator` tick migration. Gated: sub-view settlement test (cross-frame, not in Paint).
- **Phase 4** — `emSubViewPanel::Paint` simplification. Gated: golden baseline (237/6).
- **Phase 5** — `composition.rs::settle()` rewrite via scheduler. Gated: golden baseline.
- **Phase 6** — Delete `PanelTree::cycle_list` / `Cycle` / `cancel_cycle` / `run_panel_cycles`. Gated: `cargo check` finds zero callers.
- **Phase 7** — Closeout-doc update.

## 8. Out of scope

- `emContext` threading (SP7).
- W3 surface-creation de-duplication (SP6).
- Any visual/rendering change in `paint_sub_tree` itself.
- Multi-window scheduler unification (not surfaced).

End of design.
