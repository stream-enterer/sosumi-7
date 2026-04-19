# Phase 1 — Tasks 4+5 Blocked (plan structural issue)

**Date:** 2026-04-19
**Branch:** `port-rewrite/phase-1`
**Prior state:** Tasks 1, 2, 3 committed and green (emcore builds; emcore tests 887/887; emmain expected-red awaiting Task 8).
**Halted at:** Tasks 4+5 (combined commit: delete SchedOp / thread ctx through emView).
**Halt severity:** HARD STOP per user directive "Halt the entire series on any hard STOP."

## What was attempted

Dispatched an implementer subagent to execute Tasks 4+5 combined (plan requires they ship together per Task 4 Step 9 / Task 5 Step 7). Subagent performed full enumeration without editing. Report summarized below.

## Why it is blocked (implementer's diagnosis, verified against plan text)

### 1. `with_view_mut` does not yet exist as a consumed closure

Plan Task 5 Step 2 states:

> "Callers outside emView (in emWindow, emSubViewPanel, emEngine impls) are already inside a `with_view_mut` closure and have `sched: &mut SchedCtx<'_>` bound — pass that directly."

Empirically (`rg with_view_mut crates/`), the method `EngineCtx::with_view_mut` is defined in the Task-1 scaffolding but is **not called from any caller site** in the current tree. The closure-based ctx access pattern the plan assumes is Task 9's deliverable (the `EngineCtx` construction inside `DoTimeSlice` around each `behavior.Cycle(&mut ctx)` call). Task 9 is explicitly deferred by Task 3's scoping decision (only DoTimeSlice's outer signature was updated; the inner Cycle dispatch still uses the legacy `emEngine::EngineCtx`).

### 2. Threading ctx through emView methods cascades into ~100+ non-plan call sites

The 12 SchedOp call sites in `emView.rs` sit inside methods whose callers are scattered across the crate. A partial map the subagent produced:

- `emWindow::resize` (emWindow.rs:443) calls `self.view.borrow_mut().SetGeometry(...)`. No scheduler/ctx in scope here.
- `emViewInputFilter.rs`, `emViewAnimator.rs`: ~50+ test sites constructing `emView::new(...)` with no scheduler and calling `Update`/`SetViewFlags`/etc.
- `emGUIFramework`, `emMainWindow`: production sites calling emView methods at winit-event boundary.
- `eaglemode/benches/**`, `eaglemode/tests/support/**`, `crates/emmain/src/**`: more.

Threading `ctx: &mut SchedCtx<'_>` into `SetGeometry` forces the same into `emWindow::resize`, whose callers in turn need a ctx, which only materializes when Task 9 builds `EngineCtx` inside `DoTimeSlice`.

### 3. Test-only reliance on `queue_or_apply_sched_op`'s no-op path

The helper `emView::queue_or_apply_sched_op` has a graceful path when `self.scheduler.as_ref() = None`, relied on by ~150 unit tests that construct bare `emView::new(...)` with no scheduler. Deleting the helper forces every such test to materialize an `EngineScheduler`, `Vec<DeferredAction>`, `Rc<emContext>`, and a `SchedCtx` — a several-hundred-line test-infrastructure rewrite outside Tasks 4+5's stated scope.

### 4. Missing primitive `SchedCtx::is_signaled`

Plan Task 5 Step 4 requires replacing `close_signal_pending` cache reads with inline `ctx.IsSignaled(close_sig)`. `SchedCtx` exposes no `is_signaled`/`IsSignaled` method today (only `EngineScheduler::is_pending` exists, privately). Minor fix — but symptomatic of the plan's forward-pointing assumptions.

## Why this is not a judgment-call fix

Individually each item is resolvable (add is_signaled; thread ctx through one caller). In combination, Tasks 4+5 as decomposed cannot land as a scoped commit without dragging in Tasks 6 (emPanelCtx deletion + register_engine_for ctx threading), 7 (emSubViewPanel drain sites), 8 (emContext wiring so outer sites have a real root context), and 9 (Cycle trait migration, which is where `EngineCtx` is first *constructed* and passed to views).

This matches the pre-RESUME `2026-04-19-phase-1-execution-blocked.md` Reason 2 analysis (the plan's decomposition is incoherent at the 12-task grain) and the E3 concerns (plan preconditions don't match the current tree). Tasks 1–3 landed cleanly because they are genuinely independent; Tasks 4+5 are not.

## State at halt

- **Working tree:** clean (subagent committed nothing).
- **emcore build:** clean with 1 pre-existing dead_code warning from Task 2 (`framework_actions`, `pending_inputs`) — will be consumed in Task 9.
- **emcore nextest:** 887/887 pass, 0 fail, 1 skipped.
- **Workspace:** emmain still red (25 errors), expected per Task 8.
- **Goldens:** 237/6 baseline preserved (no code changes since Task 3).
- **Branch commits since bootstrap:**
  - `0bb61f0` scaffolding
  - `39b975a`, `152460b`, `3d0649b`, `fe3b7a6`, `1e0b346` Task 1 reviews + fixes
  - `fad907f`, `ba1527c` Task 2 + ledger
  - `2ee1dfe`, `bfa3010` Task 3 + ledger

## Recommendation

Two viable paths:

**R1 — replan Phase 1 as one combined ctx-threading task.** Merge Tasks 4+5+6+8+9 into a single mega-commit that threads ctx end-to-end: build `EngineCtx` in `DoTimeSlice`, migrate `emEngine::Cycle` trait, delete SchedOp and queue_or_apply_sched_op, delete emContext::scheduler, rewrite test infrastructure, and close `emSubViewPanel`'s throwaway `NewRoot()` (Task 3 review flagged this at `emSubViewPanel.rs:~324` — a real semantic bug that Task 8 or 9 must fix). Task 7 (per-sub-view scheduler deletion) can remain separate. A `--no-verify` intermediate-red branch is necessary during the mega-task since hundreds of sites are in flight.

**R2 — retain `queue_or_apply_sched_op` as a thin wrapper over the new ctx path during transition.** Tasks 4+5 become "rename + refactor" instead of "delete": replace the enum/helper body with a direct ctx call when a scheduler is available, keep the `None` no-op path for test sites. Gate deletion to Task 9 after the Cycle-trait migration lands and tests are rewired. This makes each Phase-1 task truly independent at the cost of temporarily keeping the `pending_sched_ops` staging Vec (which is the structural debt Phase 1 is supposed to delete).

Either path preserves Phase 1's spec destination. R1 is higher-risk-higher-reward; R2 is incremental and matches the current 12-task decomposition's spirit.

**Not viable:** continuing the current Tasks 4+5 decomposition as written. The plan's assumption that ctx is already threaded at caller sites does not match the tree.

## User-facing summary

Phase 1 Tasks 1–3 are green and committed. Task 4+5 cannot land as planned — the 12-task decomposition has a structural flaw at the 4/5 boundary that only becomes visible when attempting to execute it. Per user directive, the series halts. Next steps require either replanning the remaining Phase 1 tasks (R1 or R2 above) or accepting Phase 1 partial completion at Task 3 and promoting Tasks 4–12 to a replanning cycle.
