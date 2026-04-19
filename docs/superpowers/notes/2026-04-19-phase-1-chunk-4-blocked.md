# Phase 1 Chunk 4 — BLOCKED: all three parts cascade into deferred Chunk 3 territory

**Date:** 2026-04-19
**Branch:** port-rewrite/phase-1
**State at entry:** Chunks 1+2 committed, Chunk 3 deferred to Phase 1.5 per user R2. Tree at f3710c4. Tests 2455/0/9. Goldens 237/6. Clippy clean.
**Work attempted this session:** scope assessment only; no code changes shipped.

## Summary

Chunk 4's three parts — (A) delete `sub_scheduler`, (B) rewrite
`register_engine_for` to take ctx, (C) delete `emPanelCtx.rs` — each
depend on machinery that Chunk 3 (now deferred) was supposed to land.
Proceeding with any of them in isolation either (i) produces
cosmetic-only changes that mask the real blocker, or (ii) cascades into
the same 364-site ctx-threading migration that caused four prior halts
at the same boundary.

Escalation: BLOCKED. Same structural boundary, fifth halt. Not a
capability issue of this session — a decomposition issue. Chunk 4
presumes a post-Chunk-3 world (PanelBehavior::Cycle takes EngineCtx;
PanelCtx evaporated; callers of `create_child` have ctx on hand);
none of that is true yet.

## Part A — delete `sub_scheduler` — BLOCKED

### The structural problem

`sub_scheduler` carries a `DIVERGED:` block at
`crates/emcore/src/emSubViewPanel.rs:34-42` that is load-bearing, not
stale. It documents a **forced divergence**: C++ shares one scheduler
across the whole context chain via `emContext::GetScheduler` walking up
parents. Rust can't do this because `EngineCtx::tree` is a single
`&mut PanelTree` pointer — one scheduler can only cycle engines against
one tree. Sub-view panels own a nested `PanelTree` (the `sub_tree`
field), so their engines cycle against a different tree than the outer
scheduler's engines.

Today:
- Outer scheduler cycles outer-tree engines. `EngineCtx.tree` points at
  outer tree.
- `sub_scheduler` cycles sub-tree engines. `EngineCtx.tree` points at
  sub_tree. Driven once per outer slice from
  `emSubViewPanel::Cycle` (line 325).

Deleting `sub_scheduler` = registering sub-tree engines on the outer
scheduler. That requires the outer scheduler's Cycle dispatch to swap
`EngineCtx.tree` from outer → sub_tree before dispatching each sub-tree
engine, and back after. There is no mechanism for this today. The spec
§3.3 sketch in plan line 513-525 proposes it via a hypothetical
`PanelBehavior::Cycle(&mut self, ectx: &mut EngineCtx, pctx: &mut PanelCtx)`
signature — but `PanelBehavior::Cycle` currently takes only
`&mut PanelCtx` (confirmed at `emPanel.rs:336`, `emSubViewPanel.rs:291`,
`emFilePanel.rs:403`, `emFileSelectionBox.rs:1432`,
`emPanelTree.rs:3447`). Flipping that signature is a 53-file migration
(every `PanelBehavior::Cycle` impl across emcore, emmain, emfileman,
emstocks, eaglemode tests + ~150 unit-test sites that synthesize bare
`PanelCtx`). That is Chunk 3 territory (see chunk-3-blocked note).

### Cascade if attempted

Plan Task 7 step 3 snippet requires `fn Cycle(&mut self, ectx: &mut EngineCtx, pctx: &mut PanelCtx)`.
Changing `PanelBehavior::Cycle` forces:
- `PanelCycleEngine::Cycle` (`emPanelCycleEngine.rs:41-67`) to pass
  both ectx and pctx through take/put.
- Every `impl PanelBehavior` across 53 files updated to the new
  signature.
- Every unit test constructing a `PanelCtx` manually (`emFilePanel.rs:669,680,691`
  plus ~60 sites in tests/ trees) updated to synthesize ectx too.
- The test harness chunk-3-blocked.md lines 57-72 enumerates (~150 sites).

This is the same migration that caused Chunk 3 to halt four times.

### Specific cascade classification

- **Forced divergence preserved**: `sub_scheduler` is correctly
  classified. Not candidate for deletion within Phase 1 constraints.
- Part A step 4 ("close the throwaway `NewRoot()`"): also blocked —
  production `emSubViewPanel::Cycle` needs `ctx.root_context` but the
  Cycle signature doesn't carry EngineCtx yet.
- Part A step 5 ("delete SP8 DIVERGED block"): requires step 1-3 to
  land first.

## Part B — rewrite `register_engine_for` to take ctx — BLOCKED

### Structural problem

Current `register_engine_for(&mut self, id: PanelId)` at
`emPanelTree.rs:558-598`:
- Takes NO engine/priority — constructs a `PanelCycleEngine` internally.
- Uses `sched_rc.try_borrow_mut()` to re-entrantly register from inside
  a Cycle (guarded by the try_borrow path).
- Registration on busy scheduler is deferred to
  `register_pending_engines()` catch-up sweep post-slice.

Plan Task 6 step 1 target signature:
```rust
pub(crate) fn register_engine_for(
    &mut self,
    panel_id: PanelId,
    engine: Box<dyn emEngine>,
    priority: Priority,
    ctx: &mut impl ConstructCtx,
) -> EngineId {
    let eid = ctx.register_engine(engine, priority);
    ctx.wake_up(eid);
    self.panels.get_mut(&panel_id).expect(..).engines.push(eid);
    eid
}
```

This is not a refactor — it's a rewrite: `engine`/`priority` move from
the callee's responsibility to the caller's, and `ctx` is new. Callers
must supply both.

### Caller inventory (6 call sites)

- `emPanelTree.rs:533` — `init_panel_view`, called from
  `emSubViewPanel::new` + `emMainWindow::create_main_window`.
  Framework-init: **has no ctx**. Could construct ad-hoc `InitCtx` at
  the call site if plumbed through `init_panel_view` → ... all the way
  up the chain. That chain includes `emSubViewPanel::new(parent_context)`
  which is called from panel `create_child_with` sites in user code.
- `emPanelTree.rs:539` — sibling/descendant walk inside `init_panel_view`.
  Same ctx requirement.
- `emPanelTree.rs:608` — `register_pending_engines` which Part B step 4
  says to delete.
- `emPanelTree.rs:654` — `create_child`. **This is the blocker.**
  `create_child` is called from `PanelCtx::create_child` (`emPanelCtx.rs:64`),
  which is called from inside `PanelBehavior::Cycle` impls (holding
  `&mut PanelCtx`, not ctx). To supply ctx, PanelCtx must carry a
  `&mut impl ConstructCtx` — that's Chunk 3's cascade (PanelCtx carrying
  ctx = PanelBehavior::Cycle taking ctx).

### Test-call-site inventory

`register_pending_engines` appears in ~15 test sites across
`emPanelTree.rs` (sp4_5_* tests) and `composition.rs`. Deleting it
(step 4) forces each test to rewire through ctx. Same cascade.

### Specific cascade classification

Plan prompt: "Call sites outside Cycle paths need a ctx constructed —
ad-hoc `SchedCtx` or `InitCtx` is fine." — true in isolation for the
framework-init sites, but `create_child`'s inside-Cycle callers have
no ctx-construction path because PanelCtx doesn't carry ctx. That is
the prerequisite Chunk 3 would deliver.

## Part C — delete `emPanelCtx.rs` — BLOCKED (semantic) / ALSO-MECHANICAL

### Structural problem

`PanelCtx` is live and referenced in **~60 sites** (see grep at top of
blocked note). It is the parameter type of every `PanelBehavior::Cycle`
and `PanelBehavior::notice` impl, and the API surface for every panel
implementation to interact with the tree.

The spec ("If any functionality is still referenced, absorb it into
`emEngineCtx.rs` with a provenance comment") presumes a post-Chunk-3
world where `PanelCtx` has been replaced by EngineCtx-direct access
from inside Cycle impls. In the current tree, ~300 lines of panel-API
code would need to move into `emEngineCtx.rs`, conflating two unrelated
concerns (scheduler-context vs. panel-tree-API). Doing so:
- Violates the port-ideology rule that `emEngineCtx.rs` is the SPLIT
  target for scheduler/ctx threading.
- Is a cosmetic rename with no behavioral change (all imports rewrite
  mechanically) and no invariant satisfied beyond
  `ls crates/emcore/src/emPanelCtx.rs` failing.
- Creates churn in ~60 callers that will need un-doing when Chunk 3
  actually lands and `PanelCtx` evaporates.

### Classification

Cosmetic-only; satisfies no real invariant (PanelCtx still alive, just
renamed). Deferring until Chunk 3 is a correctness call, not a capacity
call. Running it as a mechanical rename without behavioral benefit
would be net-negative (churn now, churn again later).

## Invariant state at end of session

Unchanged from end of Chunk 2:

- I1 (Rc<RefCell<EngineScheduler>> in crates = 0): UNSAT.
- I1a (SchedOp = 0): UNSAT.
- I1b (pending_sched_ops et al. = 0): UNSAT.
- I1d (try_borrow in emView.rs = 0): UNSAT.
- I5 (IDIOM: comment block deleted): SAT.
- I6 (NewRootWithScheduler/GetScheduler = 0): SAT.
- emContext::scheduler field = 0: SAT.
- Chunk 4's own grep-targets (sub_scheduler = 0, register_pending_engines = 0,
  emPanelCtx = 0): **all UNSAT** — structurally blocked.

## Tests / clippy / goldens

Unchanged from baseline: 2455/0/9 nextest, clippy clean, goldens 237/6.

## Commits this session

One: this blocked-note + ledger entry. No code changes.

## Recommendation for driver

Either:
- **R1 (preferred)**: reissue Chunk 3 as the decomposed sub-chunks 3.1–3.Final
  per chunk-3-blocked note. Chunk 4's Parts A/B/C become trivial after
  Chunk 3.Final — Part A is the `sub_scheduler` delete that's possible
  once `PanelBehavior::Cycle` takes `EngineCtx`; Part B is possible once
  PanelCtx carries ctx; Part C is possible because PanelCtx has
  evaporated.
- **R2 (minimal)**: accept all Chunk 4 invariants as carried to Phase 1.5
  alongside Chunk 3. Close Phase 1 at current state with the
  `sub_scheduler`/PanelCtx/register_pending_engines constructs all
  surviving. Document in Phase 1 close-out that Phase 1.5 must do
  Chunks 3+4 together.

## What was NOT attempted

- No mechanical rename of emPanelCtx.rs → emEngineCtx.rs absorb. Would
  have produced 60-site import churn with no invariant satisfied.
- No PanelBehavior::Cycle signature flip. Would have cascaded into
  Chunk 3's 364-site migration.
- No `sub_scheduler` deletion. Would have broken sub-view engine
  dispatch (engines register on wrong tree).
- No `register_engine_for` signature rewrite. Would have left
  `create_child` callers in PanelBehavior::Cycle without ctx.
