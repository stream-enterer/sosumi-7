# Phase 1.5 — Keystone Migration (deferred Chunks 3+4 + Tasks 10/11)

> **For agentic workers:** This plan closes the keystone deferred by Phase 1's partial closeout. Five prior sessions halted at the same structural boundary while trying to decompose it. This plan accepts that the keystone is ONE atomic migration — plan for a single long session dedicated to it, not a task-by-task decomposition that will halt at the same cliff.

**Goal.** Delete the `emView::scheduler` field and the entire SchedOp deferral machinery; thread ctx through every emView method currently reading `self.scheduler`; re-narrow `App.scheduler` to a plain `EngineScheduler`; delete `sub_scheduler`, `register_pending_engines`, and `emPanelCtx.rs`; rewire the ~150 unit tests that construct bare `emView::new(...)`; and land the SP4.5-FIX-2 / SP4.5-FIX-3 cleanups (Tasks 10/11 from the original Phase 1 plan) that depend on ctx availability.

Goal stated as invariants:

- **I1.** `rg 'Rc<RefCell<EngineScheduler>>' crates/` returns zero matches.
- **I1a.** `rg -w 'SchedOp' crates/` returns zero matches.
- **I1b.** `rg 'pending_sched_ops|queue_or_apply_sched_op|register_pending_engines|close_signal_pending' crates/` returns zero matches.
- **I1c.** `rg 'sub_scheduler' crates/` returns zero matches.
- **I1d.** `rg 'try_borrow(_mut)?\(\)' crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs` returns zero matches.
- **I6 full.** `NewRootWithScheduler` / `fn GetScheduler` / `emContext::scheduler` already zero (Phase 1 Chunk 2 @ 0e68a1f); re-verify.
- **Task-10.** No pre-allocated popup signals block — `ctx.create_signal()` inline at the 4 popup-signal use sites (spec §4 D4.7).
- **Task-11.** `sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs`, `_sched_drain_baseline_slices.rs`, `_subview_reinit_baseline_slices.rs` all assert `delta == 0` (spec §4 D4.6 / D4.11).
- **I-P15-pending-inputs.** `rg -n 'pub(crate)?\s+pending_inputs' crates/emcore/src/emGUIFramework.rs` returns at least one match (field restored by Task 1 step 1g; closes W2 drift).
- **I-P15-W4-regression.** `rg 'mem::take.*framework_actions|drain_framework_actions' crates/emcore/src/emScheduler.rs` returns zero matches (catches reintroduction of the Chunk 1 scheduler-owned framework_actions workaround).

**Tech stack:** unchanged from Phase 1. No new dependencies.

**Architecture.** See Phase 1 plan's `## File Structure` and `Task 1`/`Task 5`/`Task 6`/`Task 7` sections (`docs/superpowers/plans/2026-04-19-port-rewrite-phase-1-scheduler-threading.md`). The target shape is unchanged; only the execution strategy changes.

**Companion documents:**

- Spec: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §2 P1/P6, §3.1, §3.3, §3.7, §4 D4.1–D4.11.
- Phase 1 plan: `docs/superpowers/plans/2026-04-19-port-rewrite-phase-1-scheduler-threading.md` (cite Tasks 4–11 verbatim where they describe the target shape).
- Phase 1 closeout: `docs/superpowers/notes/2026-04-19-phase-1-closeout.md`.
- Structural blocker analyses:
  - `docs/superpowers/notes/2026-04-19-phase-1-chunk-3-blocked.md` (fourth halt — full enumeration of the 364-site cascade + decomposition proposal).
  - `docs/superpowers/notes/2026-04-19-phase-1-chunk-4-blocked.md` (fifth halt — per-part cascade analysis of sub_scheduler / register_engine_for / emPanelCtx).
- Bootstrap/closeout ritual: `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md`.

**Entry precondition.** Phase 1 closeout status `PARTIAL — Chunks 1+2 complete`. Branch tagged `port-rewrite-phase-1-partial-complete`. Main at or ahead of that tag. Working tree clean. Baseline for Phase 1.5 is the *exit* state of Phase 1 partial (not the pre-Phase-1 baseline): nextest 2455/0/9, goldens 237/6, `rc_refcell_total=287`, `diverged_total=177`, `try_borrow_total=11`, `idiom_total=0`.

**JSON entries closed:** E002, E003, E004, E005, E007, E008, E009, E010, E011. (All nine carry-forward entries from Phase 1 partial.)

---

## Bootstrap (per shared ritual)

Run steps B1–B12 from `2026-04-19-port-rewrite-bootstrap-ritual.md`. Substitute `<N>` with `1-5`.

Important deviations from the standard ritual:

- **B4.** Locate `2026-04-19-phase-1-closeout.md`. Its `## Status` line reads `PARTIAL — Chunks 1+2 complete; Chunks 3+4 (keystone) deferred to Phase 1.5`. This is the single sanctioned case where PARTIAL (not COMPLETE) is accepted as a bootstrap predecessor — because Phase 1.5 exists precisely to close the PARTIAL. Record the read in the Phase 1.5 ledger; do NOT halt on the PARTIAL status.
- **B7.** Baseline is Phase 1 exit state (see Entry precondition above), not the pre-Phase-1 reference numbers in the ritual's B7 section. Capture Phase 1.5 baseline verbatim.
- **B9.** Branch: `port-rewrite/phase-1-5`.
- **B11.** Bootstrap commit message: `phase-1-5: bootstrap — baseline captured, ledger opened`.

---

## Execution shape advisory — READ BEFORE TASK 1

The keystone is ONE atomic migration. Do not decompose it into independent tasks. Five prior sessions tried; all five halted at the same boundary.

Concretely:

- Task 1 below is one cohesive operation, even though its description spans ~8 concrete steps and touches 7000+ lines across ~40 files. Its steps are internal sequencing of ONE migration, not independent tasks.
- Plan for a single long subagent session dedicated to Task 1. Allow up to two full sessions if the first halts mid-migration: the branch will carry intermediate `--no-verify` red commits from which the second session can resume.
- `--no-verify` is liberally allowed on the `port-rewrite/phase-1-5` branch during Task 1. The final commit after Task 1 must pass `cargo check --all-targets` at minimum. The full gate (fmt / clippy -D / nextest / goldens) runs at Task 5 (Closeout prep), not at each intermediate commit.
- Tasks 2–5 below are separable and fit standard task-by-task decomposition. They are structurally trivial once Task 1 lands:
  - Task 2 (sub_scheduler): one file, one struct field deletion + `Cycle` body rewrite.
  - Task 3 (register_engine_for ctx, emPanelCtx deletion): one file delete + one function signature change + caller updates. Callers all have ctx in hand post-Task-1.
  - Task 4 (Task-10 + Task-11): isolated test fixture and popup-signal site edits.
  - Task 5 (closeout): mechanical.

**Failure mode to avoid:** re-decomposing Task 1 into "sub-tasks 1.1 through 1.10" (the chunk-3-blocked proposal). That proposal has merit as a recovery path if Task 1 halts, but starting there recreates the same trap — the migration is not actually separable, and each sub-task depends on the others. Start with Task 1 as one atomic operation. Only fall back to sub-decomposition if Task 1 halts after genuine progress.

**Capacity estimate.** Prior halt analysis (chunk-3-blocked) counted 7012 lines in emView.rs with 57 SchedOp sites, 3869 lines in emPanelTree.rs with 33 SchedOp/pending-engine sites, 326 PanelCtx references across 53 files, and ~150 unit tests. The cumulative diff is on the order of 5000–8000 lines. A single subagent session has fit ~2000–3000 lines of coherent diff historically; Task 1 is likely ≥ 2x that budget in one session, so prepare for the resume-from-intermediate-commit path.

---

## Task 1: Keystone migration (ATOMIC — do not subdivide)

**This is ONE task.** The steps below are internal sequencing, not sub-tasks. The commit plan may produce several intermediate `--no-verify` red commits on the branch for checkpoint/resume purposes, but conceptually this is one atomic migration that lands as one logical change.

**Surface (from chunk-3-blocked analysis):**

- `emView.rs` 7012 lines, ~57 SchedOp call sites, 12 methods needing ctx, ~40 internal tests using bare `emView::new(...)`.
- `emPanelTree.rs` 3869 lines, ~33 SchedOp + pending-engine sites, `register_engine_for` + `register_pending_engines` + `create_child` caller chain.
- `emSubViewPanel.rs` 677 lines, `sub_scheduler` field + `Cycle` driver.
- `emGUIFramework.rs`: re-narrow `App.scheduler` from `Rc<RefCell<EngineScheduler>>` back to plain value (undoing the Chunk 2 Ch2-A carry-forward).
- `emPanel.rs` / `emFilePanel.rs` / `emFileSelectionBox.rs` / `emPanelCycleEngine.rs` / `emSubViewPanel.rs`: `PanelBehavior::Cycle` signature flips from `(&mut self, &mut PanelCtx)` to `(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx<'_>)`. ~50 impls across emcore/emmain/emfileman/emstocks/eaglemode tests.
- Unit tests across emcore/tests and the in-file `#[cfg(test)]` modules: ~150 sites that construct bare `emView::new(...)` and rely on the scheduler-None / queue_or_apply no-op path.
- External callers in emmain (`emMainWindow.rs:882` `attach_to_scheduler` call — deletable with the scheduler field), plus benches and golden-test support harnesses.

**Step sequencing (internal to Task 1 — do not commit each as a separate task):**

**1a. Land `TestViewHarness` helper first.** A single test helper in `crates/emcore/tests/common/view_harness.rs` (or similar) that owns `EngineScheduler + Vec<DeferredAction> + Rc<emContext>` and hands out `SchedCtx<'_>` / `EngineCtx<'_>` as needed. Write it before any production edits so the later test rewires use it uniformly. Verify it compiles on its own. Commit `--no-verify`.

**1b. Flip `PanelBehavior::Cycle` trait signature.**

From: `fn Cycle(&mut self, pctx: &mut PanelCtx<'_>)` (or whatever the current shape is — verify in `emPanel.rs`).

To: `fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx<'_>)` per spec §3.3.

Every `PanelBehavior` impl takes both ctx now. `pctx` continues to carry panel-tree context; `ectx` adds scheduler/ctx access. Impl bodies that don't use `ectx` get a `let _ = ectx;` to silence unused warnings (this is NOT an `#[allow]` — it's a deliberate no-op binding). Update all ~50 impls with a pass-through body (do not yet consume `ectx` except in `emSubViewPanel::Cycle` which drives sub-tree — see 1f). The `PanelCycleEngine::Cycle` driver (`emPanelCycleEngine.rs:41-67`) constructs both ectx (from its own ctx parameter) and pctx. Commit `--no-verify`.

**1c. Thread ctx through the 12 emView methods.** Add `ctx: &mut SchedCtx<'_>` (or `&mut EngineCtx<'_>` where window/tree access is needed) parameter to each. Cascade through internal helpers. Replace every `self.queue_or_apply_sched_op(SchedOp::X(…))` with `ctx.X(…)`. Replace `self.scheduler.as_ref().map(...)` patterns with direct `ctx.*` calls. Delete the `close_signal_pending` cache (use `ctx.IsSignaled(close_sig)` at the top of `Update`). Delete the SVPUpdSlice `try_borrow().ok()` fallback at `emView.rs:2080-2095` (direct `ctx.IsSignaled(svp_sig)`). This is the largest step; budget accordingly. Commit `--no-verify` at natural method-group boundaries to preserve resumable checkpoints.

**1d. Delete `SchedOp` + `queue_or_apply_sched_op` + `pending_sched_ops` + `close_signal_pending`.** After 1c, all call sites are migrated. Delete the enum, the helper fn, the fields, and their initializations. Grep-assert zero matches of `SchedOp`, `queue_or_apply_sched_op`, `pending_sched_ops`, `close_signal_pending` in `crates/`. Commit `--no-verify`.

**1e. Delete `emView::scheduler` field + `attach_to_scheduler` + `set_scheduler` + `scheduler_ref`.** With SchedOp gone, nothing reads the field. Delete it, its initialization, and the three accessors at `emView.rs:3101-3156`. Commit `--no-verify`.

**1e.1. Delete `PanelTree::sched_rc` (debt-shuffle cleanup).** The prior 1e attempt relocated the `Rc<RefCell<EngineScheduler>>` from `emView::scheduler` to `PanelTree::sched_rc` rather than deleting it; it survives as a back-channel used by `PanelTree::add_to_notice_list`, `register_engine_for`, `deregister_engine_for`, and `emPanelCtx::wake_up_panel`. Thread `ctx: &mut SchedCtx<'_>` into all four, eliminate the `try_borrow_mut` deferral + `pending_engine_wakeups` Vec (deferral only existed because the old shape couldn't obtain a scheduler borrow; with `ctx.scheduler: &mut EngineScheduler` already held it is impossible), propagate ctx through all callers across `emView`, `emPanelTree`, `emPanelCtx`, `emSubViewPanel`, tests. Grep-assert `Rc<RefCell<.*EngineScheduler` returns only the single remaining site on `App.scheduler` pending substep 1f. Commit `--no-verify`.

**Rationale for 1e.1:** Phase 1.5's whole purpose is to delete the `Rc<RefCell<EngineScheduler>>` back-channel, not relocate it. 1e's prior attempt produced zero net reduction in `rc_refcell_total` / `try_borrow_total` (actually grew by 2 each). 1e.1 makes the missing deletion a first-class step rather than silent technical debt.

**1f. Re-narrow `App.scheduler` to plain `EngineScheduler`.** Undo the Chunk 2 Ch2-A carry-forward at `emGUIFramework.rs:96`. Delete the `DIVERGED:` comment block at `emGUIFramework.rs:89-95`. Fix emmain callers (`emMainWindow.rs:882` drops the `Rc::clone(&app.scheduler)` + `attach_to_scheduler` call; the view attaches no scheduler). Fix all `self.scheduler.borrow_mut()` / `self.scheduler.borrow()` sites to direct `&mut self.scheduler`. Commit `--no-verify`.

**1g. Restore `App.pending_inputs` field.** Add `pub(crate) pending_inputs: Vec<(winit::window::WindowId, emInputEvent)>` to the App struct (spec §3.1 + §4 D4.9 mandate; Chunk 2 deleted this speculatively — W2 drift). Initialize to `Vec::new()` in the constructor. Leave the field unused at end of Phase 1.5 — Phase 3's `InputDispatchEngine` consumes it. To avoid clippy `dead_code` in the interim, mark the field `pub(crate)` and add one line in the App constructor's doc-comment acknowledging Phase 3 as the consumer. If clippy still warns, the field must still land; `--no-verify` the final Phase 1.5 commit with a ledger note that clippy's single `dead_code` warning is spec-mandated carry-forward to Phase 3. Re-verify removal of the warning when Phase 3 lands `InputDispatchEngine`.

**1h. Rewire the ~150 unit tests.** Each test that constructed `emView::new(...)` and called a scheduler-touching method now uses `TestViewHarness` (from 1a) and passes `&mut harness.sched_ctx()` (or `&mut harness.engine_ctx()`) into the method under test. Do NOT duplicate harness setup across tests. If a test needs scheduler-None semantics (for methods that have ctx-is-None branches), confirm those branches actually exist post-migration — most won't, since ctx is always present once threaded. Commit `--no-verify` at natural test-module boundaries.

**1i. Final compile + full gate.** Run `cargo check --all-targets`, then `cargo fmt`, then `cargo clippy --all-targets --all-features -- -D warnings`, then `cargo-nextest ntr`, then `cargo test --test golden -- --test-threads=1`. All must pass. Final `Task 1` commit message: `phase-1-5 task-1: keystone migration — delete emView::scheduler + SchedOp + close_signal_pending; thread ctx; re-narrow App.scheduler; rewire 150 tests`. This commit may include step 1i fixes and does NOT use `--no-verify` — the gate must be green.

**Invariants satisfied by end of Task 1:** I1 (mostly — sub_scheduler remains for Task 2), I1a, I1b, I1d. I1c still UNSAT. I6 already SAT from Phase 1.

**Append to ledger:** `Task 1 done @ <sha>. Keystone atomic migration complete. SchedOp/pending_sched_ops/close_signal_pending deleted; emView.scheduler deleted; App.scheduler plain value; ~150 tests on TestViewHarness.`

---

## Task 2: Delete `sub_scheduler`; rewire `emSubViewPanel::Cycle` via shared ctx

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs`

Post-Task-1, `PanelBehavior::Cycle` takes `EngineCtx`. The forced-divergence reasoning at `emSubViewPanel.rs:34-40` (captured in chunk-4-blocked Part A) no longer applies: the outer scheduler can drive sub-tree engines because `emSubViewPanel::Cycle` receives `ectx` and explicitly constructs an inner `PanelCtx` pointing at `self.sub_tree`. The tree-swap problem is solved by the Cycle impl choosing which tree to pass via `pctx`, not by the scheduler's `EngineCtx.tree` field.

**Steps:**

- [ ] **Step 1.** Delete the `sub_scheduler: Rc<RefCell<EngineScheduler>>` field at `emSubViewPanel.rs:43`.
- [ ] **Step 2.** Delete its construction at `emSubViewPanel.rs:65` + the `attach_to_scheduler(sub_scheduler.clone(), ...)` call at `:72`.
- [ ] **Step 3.** Rewrite `emSubViewPanel::Cycle` body to drive the sub-tree using the outer `ectx`, constructing an inner `PanelCtx` that re-borrows `self.sub_tree` disjointly from the caller's `pctx.tree`. See Phase 1 plan Task 7 Step 3 for the target shape.
- [ ] **Step 4.** Delete the SP8 DIVERGED block near the field declaration (it was `forced` classification; forcing condition is dissolved).
- [ ] **Step 5.** Run full gate: `cargo check --all-targets && cargo clippy --all-targets --all-features -- -D warnings && cargo-nextest ntr && cargo test --test golden -- --test-threads=1`.
- [ ] **Step 6.** Commit: `phase-1-5 task-2: delete sub_scheduler; emSubViewPanel::Cycle drives sub-tree via shared ctx`.

**Invariants satisfied by end of Task 2:** I1c, I1 fully.

---

## Task 3: `register_engine_for(ctx)` + `register_pending_engines` deletion + `emPanelCtx.rs` deletion

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs`
- Delete: `crates/emcore/src/emPanelCtx.rs`
- Modify: `crates/emcore/src/lib.rs` — remove `pub mod emPanelCtx;`

Post-Task-1, callers of `register_engine_for` inside `PanelBehavior::Cycle` impls have `ectx` on hand; callers during framework init have `InitCtx`. Post-Task-2, `register_pending_engines` has no writers (the only writer was `register_engine_for`'s try_borrow-fail deferral path).

**Steps per Phase 1 plan Task 6:** rewrite `register_engine_for` signature to `(panel_id, engine, priority, ctx: &mut impl ConstructCtx)`; delete `register_pending_engines` + its backing queue; delete `emPanelCtx.rs` (absorb any non-SchedOp-related panel API into `emEngineCtx.rs` only if still referenced; most of PanelCtx's code is `create_child` / tree walkers that move to `PanelTree` directly post-Task-1).

- [ ] **Step 1.** Rewrite `register_engine_for` signature + body per Phase 1 plan Task 6 Step 1.
- [ ] **Step 2.** Delete `register_pending_engines`, its backing field on `PanelTree`, and all callers (the `register_pending_engines()` sweep at `emMainWindow.rs:889` disappears; ~15 test sites in sp4_5 suite no longer call it).
- [ ] **Step 3.** Delete `crates/emcore/src/emPanelCtx.rs` + `emPanelCtx.no_rust_equivalent` marker (if any). Absorb any remaining panel-API code into `emEngineCtx.rs` with a provenance comment.
- [ ] **Step 4.** Update all callers of `register_engine_for` to pass ctx.
- [ ] **Step 5.** Full gate.
- [ ] **Step 6.** Commit: `phase-1-5 task-3: register_engine_for takes ctx; delete register_pending_engines + emPanelCtx.rs`.

---

## Task 4: Task-10 (popup signals inline) + Task-11 (timing fixtures delta=0)

**Files:**
- Modify: `crates/emcore/src/emView.rs` — the `RawVisitAbs` popup-signal allocation at `emView.rs:1828-1836` (per spec §4 D4.7).
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs`
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_sched_drain_baseline_slices.rs`
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_subview_reinit_baseline_slices.rs`

**Steps:**

- [ ] **Step 1.** Locate the popup pre-allocation block (`let (close_sig, flags_sig, focus_sig, geom_sig) = ...`). Replace with four inline `ctx.create_signal()` calls at the use sites. Per spec §4 D4.7.
- [ ] **Step 2.** Run `cargo test -p emcore sp4_5_fix_2`. Expected: PASS without panic.
- [ ] **Step 3.** Rewrite each of the three timing fixtures: `assert_eq!(delta, 1)` → `assert_eq!(delta, 0)` with a comment citing spec §4 D4.6 / D4.11.
- [ ] **Step 4.** Run `cargo test -p emcore sp4_5_fix_1_timing`. Expected: 3/3 PASS.
- [ ] **Step 5.** Full gate.
- [ ] **Step 6.** Commit: `phase-1-5 task-4: SP4.5-FIX-2 inline popup signals; SP4.5-FIX-3 delta=0 by construction`.

---

## Task 5: Full gate + Closeout prep

- [ ] **Step 1.** Run the full gate one more time to confirm nothing regressed across Task 1–4 commits:
    ```bash
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo-nextest ntr
    cargo test --test golden -- --test-threads=1
    ```
- [ ] **Step 2.** Run all Phase 1 original invariants (I1, I1a, I1b, I1c, I1d, I5, I6). Every one must be SAT.
- [ ] **Step 3.** Run Task-10 / Task-11 invariants: no `sp4_5_fix_2` panic; all three `sp4_5_fix_1_timing_*` fixtures assert `delta == 0`.
- [ ] **Step 4.** Proceed to Closeout.

---

## Closeout (per shared ritual)

Run steps C1–C11 from `2026-04-19-port-rewrite-bootstrap-ritual.md`. Substitute `<N>` with `1-5`.

Specific Phase 1.5 requirements:

- **C4.** Verify invariants I1, I1a, I1b, I1c, I1d are SAT (all carry-forward from Phase 1). Verify I5 and I6 remain SAT. Verify Task-10 / Task-11 invariants.
- **C5.** For each of E002, E003, E004, E005, E007, E008, E009, E010, E011, cite the Phase 1.5 commit that closed it and the invariant that witnesses closure.
- **C6.** Update the JSON: for each of the nine entries, change `status` from `carry-forward-phase-1.5` to `resolved-phase-1-5`, add `resolution_commit: "<sha>"`, remove the `carry_forward_reason` field. Commit: `phase-1-5: mark JSON entries E002,E003,E004,E005,E007,E008,E009,E010,E011 resolved`.
- **C7.** Closeout note status line: `COMPLETE — all C1–C11 checks passed`.
- **C10.** Tag: `port-rewrite-phase-1-5-complete`.

---

## Self-review checklist (before Closeout)

- [ ] `rg 'Rc<RefCell<EngineScheduler>>' crates/` empty (I1).
- [ ] `rg -w 'SchedOp' crates/` empty (I1a).
- [ ] `rg 'pending_sched_ops|queue_or_apply_sched_op|register_pending_engines|close_signal_pending' crates/` empty (I1b).
- [ ] `rg 'sub_scheduler' crates/` empty (I1c).
- [ ] `rg 'try_borrow' crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs` empty (I1d).
- [ ] No `IDIOM:` comment re-introduced (I5 preserved).
- [ ] `rg 'NewRootWithScheduler|fn GetScheduler' crates/` empty (I6 preserved).
- [ ] All three `sp4_5_fix_1_timing_*.rs` fixtures assert `delta == 0`.
- [ ] Goldens 237/6 (or better) preserved.
- [ ] `rg -n 'pub(crate)?\s+pending_inputs' crates/emcore/src/emGUIFramework.rs` returns at least one match (I-P15-pending-inputs; Task 1 step 1g restored).
- [ ] `rg 'mem::take.*framework_actions|drain_framework_actions' crates/emcore/src/emScheduler.rs` returns zero matches (I-P15-W4-regression; catches reintroduction of the Chunk 1 workaround).
