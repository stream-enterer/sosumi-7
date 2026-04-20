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

### Session 2 (2026-04-19 later) — Task 1 substeps 1c–1i BLOCKED

Resumption-point SHA: e2109af (unchanged). No commits made.

**Outcome:** BLOCKED — on entry, the subagent reviewed all required
reading, enumerated the actual call-site surface against the plan's
per-substep budgets, and concluded the cascade as written is
structurally too large for a single session.

**Evidence collected (mechanical grep, not speculation):**

- `queue_or_apply_sched_op` call sites in production: **9** in
  `emView.rs` (lines 1120, 1578, 1860, 1947, 1948, 1974, 3045, 3163,
  3349) + **1** in `emPanelCtx.rs` + **1** in `emPanelTree.rs` +
  **drain sites** in `emSubViewPanel.rs` (1), `emPanelTree.rs` (5) ×
  both forward + cleanup. Total ~17 production sites.
- **The 7 emView methods** named in 1c are called from outside emView
  at these external locations: `SetGeometry` from emGUIFramework.rs,
  emSubViewPanel.rs (×2), emWindow.rs, notice.rs test, plus 6 internal
  emView test sites; `set_active_panel` from emViewAnimator.rs,
  emViewInputFilter.rs, emSubViewPanel.rs, emWindow.rs, plus **~180
  test sites** across integration/, golden/, pipeline/, unit/. Full
  grep: `grep -rcE '\.(SetGeometry|set_active_panel|RawVisitAbs|...)\('`
  returns **152** direct call sites (excluding emView.rs internals).
- `WakeUpUpdateEngine` is called from `RawVisitAbs` and `Input` (4104)
  — `Input` is NOT one of the 7 methods but propagation into it is
  mandatory, and `Input` itself is called from emWindow input
  dispatch + animator + IVF, cascading transitively.
- `attach_to_scheduler` has **~31** call sites (plan cites this); each
  call site's surrounding test/setup code must rework to use
  `TestViewHarness` or direct `sched.register_engine(UpdateEngineClass)`
  wiring.

**Why this is BLOCKED, not DONE_WITH_CONCERNS-with-partial-progress:**

The plan substep 1c is atomic in the sense that it deletes SchedOp
and `queue_or_apply_sched_op`. Partial execution (threading ctx
through some methods but not others) leaves the codebase in a
non-compiling state because `SchedOp` and its helpers must remain
live until ALL 9 production sites migrate. Test rewires in 1h also
must land with 1c/1d for the crate to build — every `emView::new()`
without a scheduler field but with ctx-threaded `Update` needs a
harness even for tests that don't use signals.

Put differently: substeps 1c through 1h form a single connected
change front. The session-1 subagent correctly identified this and
reported DONE_WITH_CONCERNS; the present session confirms the same
diagnosis with concrete grep counts.

**Specific over-budget areas vs plan estimates:**

- Plan says "~50-100 call sites" for emView propagation. Actual: 152
  direct + transitive cascade through `Input`, `RawVisit`, `RawZoomOut`,
  `SetActivePanelBestPossible`, `FindBestSVP`, the animator, the IVF,
  window input dispatch. The transitive surface is the bulk of the
  emView public API (~200+ methods).
- Plan says "~150 test rewire". Actual ~180 unique call sites for
  `set_active_panel` alone, plus ~20–40 each for `SetGeometry` /
  `RawVisitAbs` / `SignalEOIDelayed`, times the fact that each test
  file has its own harness idiom (`h.view`, `view` + separate `tree`,
  pipeline helpers) that needs adapting.
- Plan sanctions `--no-verify` on the final 1i commit for dead_code
  on `pending_inputs`. No mechanism is proposed for committing
  intermediate RED states (mandatory with a multi-commit cascade
  when substeps land one-at-a-time). `CLAUDE.md` allows `--no-verify`
  on intermediate-red commits for long-running phase branches — the
  plan should explicitly authorize this pattern for 1c/1d/1e/1f if
  those are to land separately, which they must.

**Recommended re-plan (for whoever picks this up):**

1. **Subdivide 1c** into 7 substeps, one per method, each threading
   ctx through that method's transitive caller graph. Substep ordering
   (least → most invasive):
   - 1c.1 `SwapViewPorts` (4 callers, all inside `RawVisitAbs`)
   - 1c.2 `WakeUpUpdateEngine` (4 internal + 1 PanelTree + tests)
   - 1c.3 `InvalidateControlPanel` (2 test sites; external callers?)
   - 1c.4 `SignalEOIDelayed` (1 test site; called from behaviors?)
   - 1c.5 `SetGeometry` (~10 sites)
   - 1c.6 `set_active_panel` (~180 sites — own session)
   - 1c.7 `RawVisitAbs` (deep propagation via `Visit` chain)
2. **Gate each substep** on a green intermediate commit with
   `--no-verify` sanctioned for dead_code on the still-alive
   `SchedOp` (becomes dead as call sites migrate off). The final
   cleanup substep (formerly 1d) deletes `SchedOp` when use-count
   hits zero.
3. **Keep 1h distributed**: rewire test sites as each method migrates,
   not all-at-once. Each 1c.N test sites that call the N'th method
   get rewired in the same commit as that method's signature change.
4. **Defer 1e/1f** until all of 1c.1–1c.7 land: those are post-cleanup
   steps whose preconditions are "zero uses of emView.scheduler".

This restructure makes the cascade tractable in 3–5 sessions instead
of 1 nominal + N escalations.

**No code changes.** Working tree clean. Branch unchanged at e2109af.

