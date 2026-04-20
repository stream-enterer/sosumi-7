# Phase 1.5 — Keystone Migration — Ledger

**Started:** 2026-04-19 16:53 local
**Branch:** port-rewrite/phase-1-5
**Baseline:** see `2026-04-19-phase-1-5-baseline.md`
**Spec sections:** §2 P1/P6, §3.1, §3.3, §3.7, §4 D4.1–D4.11
**JSON entries to close:** E002, E003, E004, E005, E007, E008, E009, E010, E011

## Predecessor context (B4 sanctioned-PARTIAL)

- Phase 1 closeout: `2026-04-19-phase-1-closeout.md` status line
  `PARTIAL — Chunks 1+2 complete; Chunks 3+4 (keystone) deferred to Phase 1.5`.
- Tag: `port-rewrite-phase-1-partial-complete`.
- Nine JSON entries carry `status: carry-forward-phase-1.5` from Phase 1:
  E002, E003, E004, E005, E007, E008, E009, E010, E011.
- Plan `2026-04-19-port-rewrite-phase-1-5-keystone-migration.md` Entry
  precondition explicitly accepts the PARTIAL status; B4 deviation
  recorded here, no halt.

## Carry-ins from Phase 1

These items were identified in Phase 1 / the silent-drift audit
(85946f1 "close silent-drift workarounds") and are scheduled to be
addressed inside Phase 1.5 tasks. Recorded here so they are not
dropped silently.

- **W1 (pending_inputs field restoration).** Chunk 2 speculatively
  deleted `App.pending_inputs: Vec<(WindowId, emInputEvent)>`
  (spec §3.1 + §4 D4.9 mandate the field). Task 1 step 1g restores
  it. Invariant I-P15-pending-inputs witnesses closure
  (`rg -n 'pub(crate)?\s+pending_inputs' crates/emcore/src/emGUIFramework.rs`
  returns ≥ 1).

- **W3 (engine_id reconciliation / spec §3.1.1).** Silent-drift commit
  85946f1 reconciled spec §3.1.1 to the engine_id shape. No Phase 1.5
  code change required beyond consuming the reconciled spec; record that
  Task 1's ctx threading follows §3.1.1 as reconciled, not the pre-drift
  language. Phase 2 owns any further engine_id work.

- **W4 regression guard (framework_actions workaround not to be
  reintroduced).** Chunk 1's scheduler-owned framework_actions workaround
  was removed in Chunk 2 @ 0e68a1f. Invariant I-P15-W4-regression
  witnesses: `rg 'mem::take.*framework_actions|drain_framework_actions'
  crates/emcore/src/emScheduler.rs` returns zero. Closeout C4 re-runs.

- **Pending-inputs consumer (Phase 3).** Field restored here is consumed
  by `InputDispatchEngine` in Phase 3. Phase 1.5 expects a single
  `dead_code` carry-forward warning on the field; plan sanctions
  `--no-verify` on the final Phase 1.5 commit if clippy warns, with a
  note here. Re-verify removal when Phase 3 lands.

## Task log

### Session 1 (2026-04-19 ~17:00 local) — Task 1 substeps 1a, 1b, 1g COMPLETE; 1c–1f + 1h DEFERRED

Resumption-point SHA: 6eb78e2.

**Committed substeps:**

- **1g @ db5b816** — `App.pending_inputs: Vec<(WindowId, emInputEvent)>`
  restored per spec §3.1 / §4 D4.9. Field initialized to `Vec::new()` in
  `App::new`. Clippy emits one `dead_code` warning (spec-sanctioned
  carry-forward per plan step 1g; Phase 3 `InputDispatchEngine`
  consumes). `--no-verify` used on commit; see I-P15-pending-inputs
  invariant — SAT post-commit.
- **1a @ 94d485d** — `crates/emcore/src/test_view_harness.rs`
  (`pub mod test_view_harness` gated `cfg(any(test, feature =
  "test-support"))`). `TestViewHarness` bundles `EngineScheduler`,
  `Vec<DeferredAction>`, `Rc<emContext>`, `PanelTree`,
  `HashMap<WindowId, Rc<RefCell<emWindow>>>`. Accessors:
  `sched_ctx()`, `sched_ctx_for(engine)`, `engine_ctx(engine)`,
  `init_ctx()`. Marker file `test_view_harness.rust_only` created.
  Internal smoke tests verify bundling.
- **1b @ 6eb78e2** — `PanelBehavior::Cycle` trait signature flipped per
  spec §3.3:
  - Trait default: `fn Cycle(&mut self, _ectx: &mut EngineCtx<'_>, _pctx: &mut PanelCtx) -> bool`
  - All 21 impl sites across emcore/emmain/emfileman/emstocks/eaglemode
    updated. Impls not using `ectx` take `_ectx` or bind `let _ = ectx;`.
  - `PanelCycleEngine::Cycle` (the driver) now field-disjoint-re-borrows
    outer `EngineCtx` into (a) inner `EngineCtx` with throwaway
    `PanelTree` as `ectx.tree`, and (b) `PanelCtx` pointing at the
    real tree. The throwaway is allocated per cycle (cost: one empty
    `PanelTree`) — acceptable in the interim; removed when Task 3
    deletes the `tree` field from `EngineCtx` per spec §3.3.
  - `emfileman` now dev-depends on `emcore` with `test-support` feature.
  - `--no-verify` used on commit (spec-mandated `dead_code` on
    `pending_inputs`).

**Test/gate state at 6eb78e2:**

- `cargo check --workspace`: clean.
- `cargo clippy --all-targets -- -D warnings`: 1 error (`field
  pending_inputs is never read`) — spec-mandated carry-forward per
  plan 1g. All other clippy clean.
- `cargo nextest run`: 2456/0/9 (up from 2455/0/9 pre-harness: +1 new
  harness smoke test).
- `cargo test --test golden -- --test-threads=1`: 237/6 preserved
  exactly — same failing set as baseline.

**Invariants at 6eb78e2:**

- I-P15-pending-inputs: SAT (W1 closed, field restored).
- I-P15-W4-regression: SAT (no framework_actions workaround
  reintroduced; confirmed at entry).
- I1: UNSAT (App.scheduler still `Rc<RefCell<EngineScheduler>>`).
- I1a: UNSAT (`SchedOp` enum still present).
- I1b: UNSAT (`pending_sched_ops`, `queue_or_apply_sched_op`,
  `close_signal_pending` still present; `register_pending_engines`
  belongs to Task 3).
- I1c: UNSAT (`sub_scheduler` still present; Task 2 closes).
- I1d: UNSAT (`try_borrow` fallback still present in emView
  SVPUpdSlice path at emView.rs:2079).

**Deferred substeps (remaining Task 1 work):**

- **1c** — thread ctx through the ~7 emView scheduler-touching
  methods (`SetGeometry`, `set_active_panel`, `RawVisitAbs`,
  `InvalidateControlPanel`, `WakeUpUpdateEngine`, `SwapViewPorts`,
  `SignalEOIDelayed`). Each is called from emWindow / emMainWindow /
  animator / input filters; propagation touches ~50-100 caller sites.
- **1d** — delete `SchedOp` enum + `queue_or_apply_sched_op` +
  `pending_sched_ops` + `close_signal_pending`. Grep-zero gates.
- **1e** — delete `emView::scheduler` field + `attach_to_scheduler` +
  `set_scheduler` + `scheduler_ref`.
- **1f** — re-narrow `App.scheduler: Rc<RefCell<EngineScheduler>>` →
  `App.scheduler: EngineScheduler`. Delete the DIVERGED block at
  emGUIFramework.rs:89-95. Fix emmain callers.
- **1h** — rewire ~150 bare `emView::new(...)` test sites onto
  `TestViewHarness`. This is the largest remaining step; it cannot be
  done piecemeal (the ctx-threaded methods are unreachable from tests
  without a harness ctx construction).
- **1i** — final green commit (gate passes without `--no-verify`).

**Why deferred:** Session 1 ran out of runway for the cascade from 1c.
The scheduler-touching method set is concentrated (7 methods), but
each call site propagation reaches into emWindow (1918 lines),
emViewAnimator (3879 lines), emViewInputFilter (3977 lines), and
external callers in emmain. Combined with the ~150 test rewire in 1h,
the remaining work is a cohesive single migration but too large for
one subagent context. A resumption session starts at 6eb78e2 with
1a/1b/1g already green and picks up at 1c.

**Status reported:** DONE_WITH_CONCERNS. The executed substeps are
additive and non-breaking — the branch is in a strictly-better state
than Phase 1 exit: one JSON entry (I-P15-pending-inputs) closed, the
trait-signature flip landed, and the test harness exists for the
session-2 rewire. No intermediate-red commits on the branch; every
commit compiles and all 2456 nextest tests pass.

