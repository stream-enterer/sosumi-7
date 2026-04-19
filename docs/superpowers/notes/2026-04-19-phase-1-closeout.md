# Phase 1 — Scheduler Event-Loop Threading — Closeout

**Branch:** port-rewrite/phase-1
**Commit range:** `61f2042..b7364eb` (bootstrap halt note through JSON-update commit; C8 closeout commit appended after this note is written).
**Status:** PARTIAL — Chunks 1+2 complete; Chunks 3+4 (keystone) deferred to Phase 1.5

## Summary

Phase 1 shipped the additive half of the scheduler-threading migration:
ctx scaffolding (`EngineCtx`/`SchedCtx`/`InitCtx`/`ConstructCtx` in `emEngineCtx.rs`),
`emEngine::Cycle` trait flip from the legacy `emEngine::EngineCtx` to the new
`emEngineCtx::EngineCtx`, engine-impl migration (5 production impls plus ~7
test/harness engines), `DoTimeSlice` signature to spec §3.1
(`&mut windows, &root_context, &mut framework_actions`), `framework_actions`
ownership correctly seated on `App` (spec §3.1 / §3.7), and
`emContext::scheduler` field / `NewRootWithScheduler` / `GetScheduler`
deletion.

The keystone migration — deleting `emView::scheduler: Option<Rc<RefCell<EngineScheduler>>>`,
deleting `SchedOp` / `queue_or_apply_sched_op` / `pending_sched_ops` /
`close_signal_pending`, threading ctx through the ~12 emView methods that
currently read `self.scheduler`, re-narrowing `App.scheduler` to plain
`EngineScheduler`, and rewiring the ~150 unit tests that construct bare
`emView::new(...)` — was deferred after five independent single-session
halts at the same structural boundary. The cascade shape (one atomic
keystone + ~150-test rewire + 364 external caller sites) exceeds one
subagent's context capacity; it requires one long session dedicated to
the atomic migration, not a task-by-task decomposition that keeps
halting at the same cliff.

A dedicated Phase 1.5 plan captures the keystone with
execution-shape-appropriate grain: one cohesive "keystone" task (the
atomic migration) plus four post-keystone cleanup tasks that fit
standard decomposition.

## Delta from baseline

See `2026-04-19-phase-1-exit.md` for the metric table. Salient:

- nextest: 2451 → 2455 passed (+4 new Chunk-1/2 unit tests), 0 failed throughout.
- goldens: 237 / 6 preserved.
- clippy: clean throughout.
- `idiom_total`: 1 → 0 (IDIOM block deleted by Tasks 4+5 @ d3a6643).
- `rc_refcell_total`: 284 → 287 (+3 from Chunk 2 App.scheduler re-wrap; goes negative in Phase 1.5).
- `try_borrow_total`: 11 unchanged (Phase 1.5 closes).
- `diverged_total`: 177 unchanged (SP4/SP4.5/SP8 DIVERGED blocks move in Phase 1.5).

## Invariants verified

| ID  | Status | Notes |
|-----|--------|-------|
| I1  | UNSAT  | CARRY-FORWARD to Phase 1.5 |
| I1a | UNSAT  | CARRY-FORWARD to Phase 1.5 |
| I1b | UNSAT  | CARRY-FORWARD to Phase 1.5 |
| I1c | UNSAT  | CARRY-FORWARD to Phase 1.5 |
| I1d | UNSAT  | CARRY-FORWARD to Phase 1.5 |
| I5  | SAT    | `rg 'IDIOM:' crates/` empty. Closed @ d3a6643. |
| I6  | SAT    | `NewRootWithScheduler`, `fn GetScheduler`, `emContext::scheduler` all deleted @ 0e68a1f. |

Full evidence in `2026-04-19-phase-1-exit.md`.

## JSON entries closed

- **E001** — IDIOM comment block at emView.rs:186–194. Deleted @ d3a6643. Witnessed by I5.
- **E036** — spec-doc reference to E001's IDIOM classification. Obsolete with E001 @ d3a6643.

## JSON entries carried forward to Phase 1.5

- **E002** — `App.scheduler: Rc<RefCell<EngineScheduler>>` at emGUIFramework.rs:96. Chunk 2 re-wrapped this (Ch2-A carry-forward); re-narrows when the emView scheduler field is deleted.
- **E003** — `SchedOp` enum at emView.rs:196–206.
- **E004** — `pending_sched_ops: Vec<SchedOp>` field at emView.rs:427.
- **E005** — `sub_scheduler` DIVERGED block at emSubViewPanel.rs:34–40 (forced divergence today; becomes deletable once PanelBehavior::Cycle takes EngineCtx).
- **E007** — `queue_or_apply_sched_op` body at emView.rs:650.
- **E008** — `register_engine_for` + `register_pending_engines` try_borrow path at emPanelTree.rs:558–611.
- **E009** — SVPUpdSlice throttle try_borrow fallback at emView.rs:2080–2095.
- **E010** — `close_signal_pending` cache at emView.rs:421.
- **E011** — pre-allocated popup signals via `sched.borrow_mut()` at emView.rs:1828–1836.

All nine are structurally entangled with the keystone; resolving any one
in isolation cascades into the full migration per the chunk-3-blocked
and chunk-4-blocked analyses.

## Spec sections — implemented vs deferred

- §3.1 ctx scaffolding (`EngineCtx`/`SchedCtx`/`InitCtx`/`ConstructCtx`) — SHIPPED @ 0bb61f0.
- §3.1 DoTimeSlice signature (`&mut windows, &root_context, &mut framework_actions`) — SHIPPED @ 2ee1dfe (windows, root_context) + 0e68a1f (framework_actions).
- §3.1 `App.scheduler` as plain `EngineScheduler` — DEFERRED (re-wrapped to Rc<RefCell<>> in Chunk 2 Ch2-A carry-forward; Phase 1.5 closes).
- §3.3 sub-view cycle via shared ctx — DEFERRED (requires PanelBehavior::Cycle signature flip; Phase 1.5).
- §3.7 framework_actions ownership on App — SHIPPED @ 0e68a1f (Chunk 2 C2 fix).
- §4 D4.1 through D4.5 (EngineCtx existence + SchedCtx exposure) — SHIPPED.
- §4 D4.6 / D4.7 / D4.11 (SchedOp deletion + popup signals inline + register-and-wake delta=0) — DEFERRED to Phase 1.5.
- Timing fixtures (`sp4_5_fix_1_timing_*`) — still assert `delta == 1`; Phase 1.5 Task 4 rewrites to `delta == 0`.

## Commits in this phase

- `cebe5cf` — bootstrap resumed
- `0bb61f0` — Task 1 ctx scaffolding
- `152460b`, `fe3b7a6` — Task 1 review fixes
- `fad907f` — Task 2 (intermediate red — reverted in Chunk 2 Ch2-A)
- `2ee1dfe` — Task 3 DoTimeSlice signature
- `d3a6643` — Tasks 4+5 Option B minimal (IDIOM deletion)
- `c402cf6` — Chunk 1: emEngine::Cycle trait flip + 5+ engine impls
- `0e68a1f` — Chunk 2: emContext::scheduler deletion, framework_actions on App, emmain unblocked
- `138825a` — Chunk 2 review findings (Phase 3 plan CI.1 added)
- `b7364eb` — JSON entries status update

## Carry-forward to Phase 1.5

See `docs/superpowers/plans/2026-04-19-port-rewrite-phase-1-5-keystone-migration.md`.

## Next phase

Two options open in parallel:

- **Phase 1.5** (keystone migration) — gating requirement for Phase 3,
  which introduces `InputDispatchEngine` that consumes `App.pending_inputs`
  (deleted speculatively in Chunk 2; Phase 3 plan CI.1 requires restoration).
- **Phase 2** (view/window composition) — does not depend on the
  keystone and can proceed on a separate branch in parallel.

Phase 1.5 must land before Phase 3. Phase 2 may begin immediately.
