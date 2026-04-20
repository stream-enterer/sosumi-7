# Phase 1.5 — Keystone Migration — Closeout

**Branch:** `port-rewrite/phase-1-5`
**Commits:** `4918781..2e0af84` (bootstrap through Task 1 invariants) + `a270957` + `5449fa7` (post-crash bench migration + ledger)
**Status:** PARTIAL — Task 1 complete; Tasks 2–5 deferred to a follow-on phase (tentatively Phase 1.75 or absorbed into Phase 2 scope)

## Summary

Phase 1.5 Task 1 shipped the keystone migration of the outer scheduler
into `SchedCtx` threading end-to-end. `emView::scheduler` field, `App.scheduler`
Rc-wrapper, `PanelTree::sched_rc`, `SchedOp` enum, `queue_or_apply_sched_op`,
`pending_sched_ops`, `close_signal_pending`, and the `with_local_sched_ctx`
bridge helper are all gone from live code. Every public emView method that
previously emitted `SchedOp`s now takes `ctx: &mut SchedCtx<'_>` and dispatches
directly via the caller-provided scheduler borrow. `App.scheduler` is a plain
value; no Rc/RefCell indirection.

Task 2 (`sub_scheduler` deletion) was attempted and blocked on a structural
issue the plan didn't anticipate: `PanelCycleEngine` looks up its panel via
`ctx.tree`, so moving sub-tree panel engines onto the outer scheduler requires
a prior change to `PanelCycleEngine`'s tree-resolution that neither Task 2
nor Task 3 currently specifies. The honest move is to close Phase 1.5 partial
and re-plan rather than force a rushed fix.

## Delta from baseline

See `2026-04-19-phase-1-5-exit.md` for the full metric table. Headlines:
`rc_refcell_total` 287 → 282 (−5), `try_borrow_total` 11 → 5 (−6). Goldens
and nextest both preserved at 237/6 and 2455/9 respectively. Clippy
`--all-targets -- -D warnings` clean.

## JSON entries closed

To be enumerated via C5 sweep against `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json`. At minimum, entries tied to the outer scheduler `Rc<RefCell<>>` back-channel are candidates; entries tied to `sub_scheduler` / `emPanelCtx` / popup-signal pre-allocation / timing-fixture delta are NOT closed by this partial.

## Spec sections implemented

- §3.1 (App owns scheduler as plain value; `SchedCtx`/`EngineCtx` threading): **DONE** for the outer scheduler.
- §3.1.1 (EngineCtx current_engine → engine_id reconciliation): DONE (plan edit landed prior to Task 1).
- Ledger workaround items tied to `emView.scheduler`/`App.scheduler` back-channel: CLOSED.
- §3.1 sub-view/sub-scheduler unification: DEFERRED.
- §4 D4.7 (popup-signal inline alloc): DEFERRED.
- §4 D4.6/D4.11 (timing-fixture delta=0): DEFERRED.

## Invariants verified

| ID | Status |
|----|--------|
| I1 (outer scheduler ctx-threaded) | PARTIAL — outer SAT, sub_scheduler UNSAT |
| I1a (SchedOp eliminated) | SAT |
| I1b (close_signal_pending / pending_sched_ops eliminated) | SAT |
| I1c (sub_scheduler eliminated) | UNSAT — deferred |
| I1d (emView::scheduler field deleted) | SAT |
| I6 (NewRootWithScheduler gone) | SAT (from Phase 1) |

## What's deferred (next phase inputs)

1. **Task 2:** `emSubViewPanel::sub_scheduler` deletion. Blocked on
   `PanelCycleEngine` tree-parameterization — needs a prior substep added to
   the replanned phase. The re-plan should brainstorm whether to
   (a) parameterize `PanelCycleEngine` with `Weak<RefCell<PanelTree>>`, or
   (b) introduce a new `emView::DoSlice(ectx, inner_pctx)` API that drives the
   sub-view using the outer scheduler's wake tracking but an inner tree.
2. **Task 3:** `register_engine_for(ctx)` signature change + `register_pending_engines`
   deletion + `emPanelCtx.rs` deletion. Independent of Task 2's blocker but
   sequenced after it in the current plan.
3. **Task 4:** Popup-signal pre-allocation → inline `ctx.create_signal()` (spec §4 D4.7);
   timing-fixture `delta==1` → `delta==0` (spec §4 D4.6/D4.11).
4. **Task 5:** Closeout prep (currently being done ad-hoc for this partial).

## Next phase

Re-plan "Phase 1.75" (or fold into Phase 2 scope) to cover the four deferred
tasks above. Entry precondition: this partial's tag
`port-rewrite-phase-1-5-partial-complete`. Phase 2 as currently written assumes
I1 fully SAT; if folding Task 2 into Phase 2, Phase 2's plan text needs a
prepended substep covering the `PanelCycleEngine` rewiring.
