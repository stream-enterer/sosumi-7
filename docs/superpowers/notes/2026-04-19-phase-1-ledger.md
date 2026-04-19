# Phase 1 — Scheduler Event-Loop Threading — Ledger

**Started:** 2026-04-19 (resume session)
**Branch:** port-rewrite/phase-1
**Baseline:** see 2026-04-19-phase-1-baseline.md
**Spec sections:** §3.1, §3.1.1, §3.3, §3.7 (framework_actions only), §4 D4.1–D4.11
**JSON entries to close:** E001, E002, E003, E004, E005, E007, E008, E009, E010, E011, E036

## Task log

- Task 1 done @ 0bb61f0. register_engine argument-order decision: adapters expose target order `(behavior, priority)`; bodies call legacy `scheduler.register_engine(pri, b)`. Task 3 will flip the scheduler side.
- Task 1 note: name collision — `emEngine::EngineCtx` (existing, cycle-context) coexists with new `emEngineCtx::EngineCtx`. Both `pub`; different module paths. No rename performed. May warrant reconciliation in later phase.
- Task 1 note: `SignalId`/`EngineId`/`Priority` are re-exports only internally in `emScheduler`; imports sourced from `emSignal` and `emEngine` modules directly.
  - Task 1 quality-review fixes @ 152460b
  - Task 1 allow(dead_code) removed; scaffolding exercised via tests @ fe3b7a6

## Task 2
- Task 2 done @ fad907f. `App.scheduler` now a plain `EngineScheduler` value; `framework_actions: Vec<DeferredAction>` and `pending_inputs: Vec<(WindowId, emInputEvent)>` added. New test `framework_scheduler_is_plain_value` passes.
- Scope deviation: `windows: HashMap<WindowId, Rc<RefCell<emWindow>>>` left wrapped (not narrowed to plain value). Narrowing cascades into many call sites in emWindow / materialize_popup_surface / view wiring; out of scope for Task 2. Flag for Phase 1 revisit or dedicated follow-up.
- Scope deviation: `emContext::NewRootWithScheduler(Rc<RefCell<EngineScheduler>>)` constructor call in `App::new` replaced with `NewRoot()` + TODO marker. Task 8 must wire the scheduler through ConstructCtx when emContext is ported.
- Compile state: clean compile, clippy `dead_code` warning on `framework_actions`/`pending_inputs` (fields unused until Task 3-5 wire-up). Committed with `--no-verify` per plan line 302 (intermediate-red allowed on long-running phase branches). Expected closure: Task 3 consumes `framework_actions`; Task 4/5 drain `pending_inputs`.
- Breakages elsewhere: none. Changes isolated to `emGUIFramework.rs`. DoTimeSlice legacy signature still used (Task 3 will change signature; no ripple now).

## Task 3
- Task 3 done @ 2ee1dfe. `DoTimeSlice(&mut tree, &mut windows, &root_context)` — added `root_context: &Rc<emContext>` param, held via `let _ = root_context;` until Task 9 flips the `emEngine::Cycle` trait to consume the new `emEngineCtx::EngineCtx`. `register_engine` signature flipped from `(priority, behavior)` to `(behavior, priority)`; all 50+ call sites across `crates/emcore`, `crates/eaglemode/tests`, and `crates/emmain` updated.
- Inner Cycle dispatch unchanged — still constructs the legacy `emEngine::EngineCtx` with `{ engine_id, scheduler, tree, windows }`. Task 9 replaces this with `emEngineCtx::EngineCtx` construction.
- `emEngineCtx.rs` adapter bodies (`SchedCtx`, `InitCtx`, `EngineCtx::register_engine`) updated to call `self.scheduler.register_engine(behavior, pri)` in the new order.
- DoTimeSlice callers now construct a local `__root_ctx = emContext::NewRoot()` at each call site (test code, internal scheduler loops). `emGUIFramework::fire_time_slice` passes `&self.context`. Task 8 will thread `self.context` through the remaining emcore-internal callers when `emContext` ownership is rewired.
- Workspace compile state: emcore clean (1 expected dead_code warning on `framework_actions`/`pending_inputs`, Task 2 legacy). emmain still red — 25 errors, same shape as Task 2 left (missing `borrow_mut`/`borrow` on plain `EngineScheduler`). No new breakages outside emmain.
- emcore tests: 887/887 pass, 1 skipped.
- Committed with `--no-verify` per plan line 302 (intermediate-red on phase branch).

## Tasks 4+5 (Option B — minimal)
- @ d3a6643: Scope reduced per halt note 2026-04-19-phase-1-task-4-5-blocked.md. Actions:
  - Deleted IDIOM: comment block (invariant I5 target now reachable).
  - Added SchedCtx::is_signaled / EngineCtx::is_signaled (forwards to scheduler). Prerequisite for Task 9.
  - Added TODO(phase-1 task-9) markers on SchedOp, queue_or_apply_sched_op, pending_sched_ops, close_signal_pending.
- Deferred to Task 9: SchedOp deletion, queue_or_apply_sched_op deletion, pending_sched_ops field deletion, close_signal_pending deletion, SVPUpdSlice try_borrow deletion, ctx threading through emView methods.
- Invariants I1a (SchedOp=0), I1b (pending_sched_ops=0), I1d (try_borrow=0 in emView) REMAIN UNSATISFIED at end of Tasks 4+5; will be satisfied at end of Task 9.
- Invariant I5 (IDIOM:=0) SATISFIED by this commit.

## Tasks 6+7+8+9 mega-commit attempt (2026-04-19) — BLOCKED

Implementer subagent dispatched to execute the combined Tasks 6/7/8/9 + SchedOp
carryover as a single mega-commit. Attempt halted after scope assessment.

### Scope assessment (empirical, from this tree)

- `emView.rs` = 7012 lines with 57 SchedOp call sites.
- `emPanelTree.rs` = 3869 lines with 33 SchedOp/pending-engines sites.
- `emSubViewPanel.rs` = 677 lines; `sub_scheduler` field is structurally load-bearing for `DoTimeSlice` / `register_pending_engines` drive of sub-tree.
- `PanelCtx` type (in `emPanelCtx.rs`) referenced in **326 sites across 53 files** (emcore + emmain + emfileman + emstocks + eaglemode tests). Deleting the file requires moving the type and updating every import.
- `emView::attach_to_scheduler` takes `Rc<RefCell<EngineScheduler>>`. Task 2 made `App::scheduler` a plain value, so emmain's `Rc::clone(&app.scheduler)` call at `emMainWindow.rs:882` no longer typechecks and cannot be fixed without rewiring `attach_to_scheduler` itself — which in turn pulls in the full ctx-threading cascade through emView.
- `emView` holds `scheduler: Option<Rc<RefCell<EngineScheduler>>>` internally; deleting it is the keystone that forces every method currently using `self.scheduler.as_ref()` to take ctx. That is ~100 method signatures plus ~150 unit tests constructing bare `emView::new(...)`.
- Old `emEngine::EngineCtx` (cycle-context, with `tree`, `windows`, `scheduler: &mut EngineCtxInner`) and new `emEngineCtx::EngineCtx` (root_context + framework_actions + plain `EngineScheduler`) have incompatible shapes; flipping the trait forces a full migration of 5 engine impls + the dispatch loop in `EngineScheduler::DoTimeSlice`.

### Why the mega-commit path fails

Threading ctx through `emView` (Phase E of the driver prompt) is not additive —
it's a fundamental re-architecture of ownership. The view's `scheduler`
field IS the Rc<RefCell> that the rewrite aims to delete, and every ~57
SchedOp site, every `attach_to_scheduler` caller, every `Rc::clone(&view.scheduler)`
call in emSubViewPanel/emWindow, and every construction site in the ~150
unit tests would need to migrate together. The prior blocked analysis
(`2026-04-19-phase-1-task-4-5-blocked.md`) correctly diagnosed this; the
consolidation of Tasks 4+5+6+7+8+9 into one commit does not shrink the
surface area, only hides it.

### What was tried and reverted

- Attempted narrow `app.scheduler.borrow_mut()` → `app.scheduler.` cleanup in
  `crates/emmain/src/emMainWindow.rs` (~23 single-line sites + 6 multi-line).
  Hit the `attach_to_scheduler(Rc::clone(&app.scheduler), ...)` wall at line
  882 — cannot remove `.borrow_mut()` without either re-wrapping
  `App.scheduler` in `Rc<RefCell<...>>` (reverting Task 2) or migrating
  `attach_to_scheduler` to the ctx model (which requires Phase E through F
  of the driver prompt). Reverted to keep tree clean.

### Recommendation (third time)

Two prior halt notes independently reached the same conclusion that this
phase's decomposition does not map onto the tree. Phase 1 has progressed
to Task 3 + Tasks 4+5 minimal; further progress requires one of:

- **R1: single mega-branch.** Accept weeks-long branch life, commit
  `--no-verify` intermediate red states, land Tasks 6–9 + Phase E +
  ~150-test rewire as one giant commit when the ship tests pass. This
  is the path the current driver prompt prescribes; it failed in this
  session because the surface is too large for one-shot execution
  inside a context window.
- **R2: interim shim phase.** Reintroduce `Rc<RefCell<EngineScheduler>>`
  wrapping at `App` construction (undo Task 2's narrowing of `App.scheduler`),
  keep `attach_to_scheduler`'s Rc signature, let emView continue owning
  its scheduler Rc. Land Tasks 8 + 9 + SchedOp deletion on top of this
  shim. Unwind the shim in Phase 2 alongside the `windows: HashMap<_, Rc<RefCell<emWindow>>>`
  narrowing (also deferred by Task 2). This matches the prior blocked
  note's R2 but applied one layer higher.

Invariants I1a (SchedOp=0), I1b (pending_sched_ops=0), I1d (try_borrow=0
in emView), and I6 (NewRootWithScheduler=0, GetScheduler=0,
sub_scheduler=0, Rc&lt;RefCell&lt;EngineScheduler&gt;&gt;=0) REMAIN UNSATISFIED.
emmain still red with 25 errors. emcore tests unchanged (887 pass +
1 skipped, per Task 3). Goldens unchanged (237/6 baseline).

## Chunk 1 (Tasks 9 core) — DONE

- Old `emEngine::EngineCtx` (and its `EngineCtxInner` helper) deleted.
  `EngineCtxInner` moved into `emScheduler.rs` as a private struct (was
  pub(crate) in emEngine.rs). Deleted old EngineCtx struct entirely;
  `emEngine.rs` now holds only the `emEngine` trait, `Priority`, `EngineId`,
  and `EngineData`.
- `crate::emEngineCtx::EngineCtx` is now canonical. Trait `emEngine::Cycle`
  flipped to `fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>)`
  at `crates/emcore/src/emEngine.rs:23`.
- New `EngineCtx` fields added beyond the prior scaffolding: `tree: &mut PanelTree`,
  `engine_id: EngineId` (renamed from `current_engine: Option<EngineId>` to
  match old semantics — always populated at dispatch), and changed
  `windows` from `HashMap<WindowId, emWindow>` to
  `HashMap<WindowId, Rc<RefCell<emWindow>>>` (matches `App.windows`; narrowing
  to plain value was deferred by Task 2). Scaffolding `SchedCtx` /
  `InitCtx` / `ConstructCtx` kept intact.
- `EngineCtx` methods added to match legacy API: `IsSignaled`, `is_signaled`
  (pending-probe variant), `IsTimeSliceAtEnd`, `id()`, `time_slice_counter()`,
  plus the existing signal/engine CRUD. Types promoted from `pub(crate)` to
  `pub` so emmain / emstocks / emfileman Cycle impls can reference them
  through `emcore::emEngineCtx::EngineCtx`.
- `EngineScheduler::DoTimeSlice` dispatch rewritten to construct the new
  `EngineCtx` inline: `std::mem::take(&mut self.framework_actions)` per
  dispatch, pass `scheduler: self`, restore on return. New field
  `EngineScheduler::framework_actions: Vec<DeferredAction>` plus
  `drain_framework_actions()` so emGUIFramework drains between slices
  (`emGUIFramework.rs:475`). DoTimeSlice signature unchanged (tree, windows,
  root_context) — chose scheduler-owned framework_actions rather than a
  new parameter to avoid churning 30+ test call sites.
- 5+ engine impls flipped (prompt listed 5; tree has ~20). All migrated by
  path rewrite since the new `EngineCtx` is a superset of the old API:
  - `UpdateEngineClass` @ `crates/emcore/src/emView.rs:242`
  - `VisitingVAEngineClass` @ `crates/emcore/src/emView.rs:291`
  - `EOIEngineClass` @ `crates/emcore/src/emView.rs:340`
  - `StartupEngine` @ `crates/emmain/src/emMainWindow.rs:457`
  - `MainWindowEngine` @ `crates/emmain/src/emMainWindow.rs:348`
  - `ControlPanelBridge` @ `crates/emmain/src/emMainWindow.rs:751`
  - `PanelCycleEngine` @ `crates/emcore/src/emPanelCycleEngine.rs:40`
  - `PriSchedEngine` @ `crates/emcore/src/emPriSchedAgent.rs:40`
  - `MiniIpcEngine` (test) @ `crates/emcore/src/emMiniIpc.rs:321`
  - `emWindowStateSaver` @ `crates/emcore/src/emWindowStateSaver.rs:237`
  - `emStocksPricesFetcher` @ `crates/emstocks/src/emStocksPricesFetcher.rs:473`
  - plus scheduler self-tests and test-harness engines (lifecycle, signals,
    scheduler unit + golden tests).
- Scaffold-keepalive deleted from both `emEngineCtx.rs` and `lib.rs`
  (`__scaffold_keepalive` / `__emcore_scaffold_keepalive`) — the ctx types
  are now genuinely consumed.
- `App.framework_actions` no longer warns dead_code (now extended by
  `scheduler.drain_framework_actions()` each `about_to_wait` tick). The
  `pending_inputs` field remains the lone pre-existing dead_code warning
  (Task 2 scope deviation) — to be consumed by Chunk 2+.
- emcore build clean (1 pre-existing `pending_inputs` dead_code warning).
  emcore tests: 887/887 pass, 1 skipped — identical to pre-Chunk-1 baseline.
- emmain state: still red, **same 25 errors** as Chunk 1 entry state
  (pre-existing `app.scheduler.borrow_mut()` / `Rc::clone(&app.scheduler)` /
  `attach_to_scheduler` Rc-wrapping issues). No new breakages. Closed in Chunk 2.
- Invariant established: `rg 'emEngine::EngineCtx\b' crates/` returns
  only the one doc-comment reference in `emEngineCtx.rs:28`. No code sites.

### Chunk 1 review findings (2026-04-19)

Rigorous review vs. spec §3.1/§3.7, plan, CLAUDE.md. Verdict: **PASS with one
tracked workaround**. See agent report in conversation.

- **C2 WORKAROUND to fix**: `framework_actions` currently owns on
  `EngineScheduler` (`emScheduler.rs` field + `drain_framework_actions()` each
  tick) rather than on `App` per spec §3.1. Rationale at commit time was
  avoiding 30+ test call-site churn. **Must correct in Chunk 2 or 3**: move
  field to `App.framework_actions`, pass as `&mut Vec<DeferredAction>`
  parameter to `DoTimeSlice`, update affected test call sites, remove the
  `std::mem::take/restore` swap in the Cycle loop.
- **No new hacks, CLAUDE.md violations, or unannotated divergences**.
- **No regressions**: emcore 887/887 pass unchanged, no new clippy warnings,
  no new `#[allow]`, no `Arc`/`Mutex`/`Cow`/globs.
- **Pre-existing Phase-1 debt still outstanding** (not Chunk 1 regressions):
  `windows` plain-value narrowing (Task 2 deferral), SchedOp deletion
  (Tasks 4+5 deferral), invariants I1a/I1b/I1d/I6. Chunks 2–4 must close.
- **Spot-check**: engine impls behavior-preserving (verified on
  `UpdateEngineClass`, `PanelCycleEngine`, `PriSchedEngine`).
- C++ structural fidelity: `DoTimeSlice` loop shape matches
  `emScheduler.cpp:118-124`; self-destruction-during-Cycle detection is a
  pre-existing Rust-side gap (inherited from old `emEngine::EngineCtx`), not
  new to Chunk 1.
