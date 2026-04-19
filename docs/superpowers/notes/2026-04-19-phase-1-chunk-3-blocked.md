# Phase 1 Chunk 3 — blocked at same boundary as three prior halt attempts

**Date:** 2026-04-19
**Branch:** port-rewrite/phase-1
**State at entry:** Chunks 1+2 committed; Ch2-A (App.scheduler re-wrap) and I1/I1a/I1b/I1d UNSATISFIED; 2455/0/9 nextest; goldens 237/6.
**Work attempted in this session:** scope assessment only — no code changes shipped.

## Why this is the fourth halt at the same boundary

Three prior halt notes captured the same finding:

- 2026-04-19-phase-1-task-4-5-blocked.md (Tasks 4+5)
- Tasks 6+7+8+9 mega-commit — ledger entry `@ b185d6d`
- Tasks 4+5 blocked — ledger entry `@ 55b7ce1`

The Chunk 3 driver prompt is a reissue of the Tasks 6+7+8+9 mega-commit with
scope reduced to "only the emView + App ctx threading". That reduction does
not materially shrink the surface — it only renames the bundle. The
surface:

- `emView.rs` 7026 lines, 78 self-scheduler/SchedOp sites.
- SchedOp / queue_or_apply_sched_op / pending_sched_ops / close_signal_pending:
  110 sites across 7 files (`rg -c` above).
- emView methods currently reading `self.scheduler` or calling
  `self.queue_or_apply_sched_op`: 12 distinct methods
  (`SetGeometry`, `set_active_panel`, `RawVisitAbs` popup-open,
  `RawVisitAbs` popup-close, `InvalidateControlPanel`, `WakeUpUpdateEngine`,
  `SwapViewPorts`, `SignalEOIDelayed`, plus `set_scheduler`,
  `attach_to_scheduler`, `scheduler_ref`, and the SVPUpdSlice throttle in
  `UpdateViewing`). Each takes/returns no ctx today.
- External `view.*` callers of those methods (and their transitive callers):
  364 sites across 33 files including benches, golden tests, unit tests,
  emmain production code.
- Unit tests in `emView.rs` alone that construct bare `emView::new(...)`
  and rely on the scheduler-None / queue_or_apply no-op path: ~40 visible
  in the `#[cfg(test)] mod tests` block (5074–7019). Counterparts in
  `emViewInputFilter.rs` (3977 lines) and `emViewAnimator.rs` (3879 lines)
  bring the rewire burden to the prompt's stated ~150.

## Structural finding that makes this unlike Chunks 1 and 2

Chunk 1 (old `emEngine::EngineCtx` → new) and Chunk 2 (emContext::scheduler
deletion, framework_actions migration) were additive migrations: one type
flipped, callers got a mechanical path rewrite, tests continued to work
because the old type was never held by unit tests themselves.

Chunk 3 is **not additive**. The keystone is the emView-internal
`scheduler: Option<Rc<RefCell<EngineScheduler>>>` field. Deleting it
forces every caller of every emView method that touches scheduler state
to acquire ctx — and the *test-harness construction model* must change,
because tests currently construct bare `emView::new(...)` with no
scheduler at all and rely on `queue_or_apply_sched_op` silently
no-op'ing. Under Chunk 3's target shape, every test must produce a
`SchedCtx` (which owns `&mut EngineScheduler`, `&mut Vec<DeferredAction>`,
`&Rc<emContext>`) on the stack, and every method under test must accept it.

This is the scope the prompt calls "Part E: Test-harness rewire (~150
unit tests)". The prompt's own recommendation in that section — "Do NOT
duplicate harness setup in each test. Single helper, reused across all
tests." — is structurally correct, but implementing it requires:

1. The single `TestViewHarness` helper: ~50 lines.
2. Every test-site rewrite from `view.Update(&mut tree)` to
   `view.Update(&mut tree, &mut h.sched_ctx())`: 150 sites.
3. Every `emViewInputFilter` and `emViewAnimator` test that constructs
   its own `emView` transitively: ~50 sites.
4. All 364 external-caller sites in emmain / eaglemode / benches /
   golden tests / integration tests updated to thread ctx.
5. Every `emView` method touching scheduler gains a ctx parameter: 12+
   signatures, each with cascading ripple through internal helpers
   (`RawVisitAbs` dispatches to `UpdateEngineClass`-reentrant paths
   and to `SwapViewPorts`-reentrant paths; the latter also sets signals).

The prompt asks for all five items in one session. Prior three attempts
established that this does not fit in a single context window; this
session does not change that.

## What I inspected (vs. attempted to code)

Deliberately avoided starting a mega-edit that prior evidence says will
not land. Inspected instead:

- `crates/emcore/src/emView.rs:189–260` — SchedOp enum + apply_to +
  apply_via_ctx. Both variants remain needed for the queue-drain path
  inside `UpdateEngineClass::Cycle` (lines 257–260).
- `crates/emcore/src/emView.rs:421–427` — the target fields
  (`close_signal_pending`, `pending_sched_ops`) with their
  `TODO(phase-1 task-9)` markers from Tasks 4+5 minimal.
- `crates/emcore/src/emView.rs:648–657` — the `queue_or_apply_sched_op`
  body; replacement requires ctx propagation through every caller.
- `crates/emcore/src/emView.rs:3101–3156` — `set_scheduler` /
  `attach_to_scheduler` / scheduler field initialization. This is the
  Rc<RefCell<_>> keystone; removing it requires the caller (App) to
  pass ctx instead.
- `crates/emcore/src/emGUIFramework.rs:87–145` — `App` struct with the
  Chunk 2 re-wrap at line 96 plus its DIVERGED comment.

## Recommendation: restructure Chunk 3 before reattempting

The driver prompt's Parts A–F are *already* a reasonable sequence, but
each individual Part is still within reach of one session only if the
others don't run concurrently. Concrete proposal:

### Chunk 3.1 — introduce `SchedCtx` as the actual per-call parameter type, no call-site migration yet

Today `SchedCtx` (in `emEngineCtx.rs`) exists as scaffolding. Confirm it
exposes every op SchedOp supports: `fire`, `wake_up`, `connect`,
`disconnect`, `remove_signal`, `remove_engine`. Verify tests call no
method on it that drifts from SchedOp variants. No emView edits.
One commit. Trivially landable.

### Chunk 3.2 — add `App::with_sched_ctx` helper, without calling it yet

Implementation of Part D option (b) as a dead-code helper with
`#[cfg(test)]`-gated exerciser. Cannot trigger clippy dead_code because
it's exercised. No emView edits. One commit.

### Chunk 3.3 — migrate ONE emView method (pick `WakeUpUpdateEngine` — one SchedOp site, no internal branching)

Add `ctx: &mut SchedCtx<'_>` parameter, change body to `ctx.wake_up(id)`,
update all callers with `app.with_sched_ctx(|sc| view.WakeUpUpdateEngine(sc))`
or inline equivalent. Drop the field access on `self.update_engine_id`
if the method still needs it — keep that on self. This exercises the
end-to-end path for one method and reveals the true cost per subsequent
method. Commit. Land.

### Chunk 3.4 through 3.N — migrate remaining methods one at a time

Each chunk picks one emView method, migrates it + its callers + its
specific unit tests. SchedOp stays alive during these chunks and
shrinks by one variant per method migrated. The Rc<RefCell<_>>
scheduler field stays alive until every caller has been migrated.

### Chunk 3.Final — delete SchedOp + scheduler field + attach_to_scheduler + re-narrow App.scheduler

Once zero methods remain that use `self.scheduler` or SchedOp, this
becomes the additive delete-only commit the original prompt aimed for.

### Test-harness migration

Land the `TestViewHarness` helper alongside Chunk 3.3 (first migrated
method). Each subsequent chunk updates only the tests for its migrated
method. Avoids the "150 tests one commit" cliff entirely.

## Invariant state at end of this session

Unchanged from end of Chunk 2:

- I1 (Rc<RefCell<EngineScheduler>> in crates = 0): UNSAT (still 1 site
  at `emGUIFramework.rs:96`, 1 at `emView.rs:381`, plus uses).
- I1a (SchedOp = 0): UNSAT.
- I1b (pending_sched_ops et al. = 0): UNSAT.
- I1d (try_borrow in emView.rs = 0): UNSAT.
- I5 (IDIOM: comment block deleted): SAT (from Tasks 4+5).
- I6 (NewRootWithScheduler/GetScheduler = 0): SAT (from Chunk 2).
- `emContext::scheduler` field = 0: SAT (from Chunk 2).

Tests: 2455/0/9 nextest (unchanged). Goldens: 237/6 (unchanged).
Clippy: clean (unchanged). Build: clean (unchanged).

## Commits this session

None. Tree still at `f3710c4`. No partial edits shipped.

## Escalation classification

**BLOCKED.** Fourth independent halt at the same structural boundary.
Not a capability issue of this session — a decomposition issue of the
driver prompt. Chunk 3's Parts A–F, executed as one bundle, exceeds
one-session capacity. Proposal above decomposes the same work into
~10 landable chunks that each touch one method's worth of surface.

Recommend driver accept the decomposition (or a variant of it) and
reissue Chunk 3 as Chunk 3.1, or accept the R2 path from the earlier
halt note (keep the Rc<RefCell<_>> shim through Phase 1, attack the
ctx threading in a dedicated Phase 1.5).
