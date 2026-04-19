# Phase 1 — Scheduler Event-Loop Threading — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `Rc<RefCell<EngineScheduler>>` + SchedOp deferral + per-sub-view scheduler with event-loop-threaded `&mut EngineScheduler` + `EngineCtx`/`SchedCtx` disjoint-borrow ctx. Delete all 12 B01 workaround mechanisms.

**Architecture:** Framework owns `scheduler: EngineScheduler` as a plain value. Each `DoTimeSlice` threads `&mut` through an `EngineCtx<'a>` borrow bundle that exposes scheduler+windows+framework-actions. Views are reached via `ctx.with_view_mut(wid, |view, sched| …)` which re-borrows ctx disjointly so scheduler calls remain available inside the closure. SchedOp/pending_sched_ops/close_signal_pending/per-sub-view scheduler are deleted in this phase — no compat layer.

**Tech Stack:** Rust (stable), existing eaglemode-rs crates (emcore, tests/golden). No new dependencies.

**Companion documents:**
- Spec: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §2 P1/P6, §3.1, §4 D4.1–D4.11
- Bootstrap/closeout ritual: `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md`
- Workaround inventory: `docs/superpowers/notes/2026-04-19-scheduler-refcell-workaround-ledger.md`

**Spec sections implemented:** §3.1, §3.1.1, §3.3, §3.7 (framework_actions only — popup cancellation is Phase 2), §4 D4.1–D4.11.

**JSON entries closed:** E001, E002, E003, E004, E005, E007, E008, E009, E010, E011, E036.

**Phase-specific invariants to verify at Closeout (C4):**
- **I1.** `rg 'Rc<RefCell<EngineScheduler>>' crates/` returns zero matches.
- **I1a.** `rg -w 'SchedOp' crates/` returns zero matches.
- **I1b.** `rg 'pending_sched_ops|queue_or_apply_sched_op|register_pending_engines|close_signal_pending' crates/` returns zero matches.
- **I1c.** `rg 'sub_scheduler' crates/` returns zero matches.
- **I1d.** `rg 'try_borrow(_mut)?\(\)' crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs` returns zero matches (the scheduler-adjacent try_borrow fallbacks).
- **I5 partial.** `rg 'IDIOM:' crates/` returns zero matches (the sole occurrence at emView.rs is deleted by Task 4).
- **Delta target** (re-baselined 2026-04-19 against ground-truth measurements): `try_borrow_total` ends at **0** (baseline 11; all 11 occurrences live in `emView.rs` (4) + `emPanelTree.rs` (7), and I1d already requires zero in those two files — so the global total goes to zero by construction); `rc_refcell_total` drops by ≥ 5 from its 284 baseline (scheduler/window/view core sites: `EngineScheduler`, per-`emWindow` wrapper, per-`emView` wrapper, `sub_scheduler`, `emContext::scheduler`); `diverged_total` drops by ≥ 6 (the SP4/SP4.5/SP8 DIVERGED blocks that dissolve). Note: the original spec draft cited a "≥ 40 drop" target that was based on a stale pre-SP4.5-FIX-1 baseline; the SP4.5-FIX-1 wave already reduced the population to 11.

**Timing test invariants:**
- The SP4.5-FIX-3 timing fixtures in `crates/emcore/tests/sp4_5_fix_1_timing_*.rs` — previously asserting `delta == 1` (the +1 slice drift) — are rewritten in Task 11 to assert `delta == 0`.
- Goldens (`tests/golden/`) must preserve the 237/6 baseline (or better) throughout. A regression in any previously-passing golden is a STOP.

---

## Bootstrap (per shared ritual)

Run steps B1–B12 from `2026-04-19-port-rewrite-bootstrap-ritual.md`. Substitute `<N>` with `1`. At B7 capture the baseline values and record them verbatim in `docs/superpowers/notes/2026-04-19-phase-1-baseline.md`. Do not proceed past B12 unless all green.

---

## File Structure

**New files (all under `crates/emcore/src/`):**
- `emEngineCtx.rs` — defines `EngineCtx<'a>`, `SchedCtx<'a>`, `InitCtx<'a>`, `ConstructCtx` trait. Replaces the current ad-hoc `emPanelCtx.rs` layout.
- Marker file: `emEngineCtx.rust_only` (one line: `Rust-only: ctx structs translate C++'s raw-pointer event-loop threading to Rust &mut discipline; no direct C++ analogue`).

**Files heavily modified:**
- `crates/emcore/src/emGUIFramework.rs` — `scheduler` becomes plain `EngineScheduler`; add `windows: HashMap<WindowId, emWindow>`, `framework_actions: Vec<DeferredAction>`, `pending_inputs: Vec<(WindowId, InputEvent)>`.
- `crates/emcore/src/emScheduler.rs` — `DoTimeSlice` signature adds `windows`, `root_context`; ctx is built per engine.
- `crates/emcore/src/emView.rs` — delete `SchedOp` enum, `queue_or_apply_sched_op`, `pending_sched_ops`, `close_signal_pending`. ~12 call sites migrate to `ctx.fire/connect/disconnect/remove_signal/wake_up`.
- `crates/emcore/src/emPanelTree.rs` — `register_engine_for` (line 558–598) takes `&mut impl ConstructCtx`; `register_pending_engines` deleted; `RemoveEngine` call (line 626) becomes `ctx.remove_engine(eid)`.
- `crates/emcore/src/emPanelCtx.rs` — deleted; its content absorbed into `emEngineCtx.rs`.
- `crates/emcore/src/emSubViewPanel.rs` — delete `sub_scheduler: Option<Rc<RefCell<EngineScheduler>>>`; delete its construction and wiring; `Cycle` recurses via shared outer ctx per §3.3.
- `crates/emcore/src/emContext.rs` — delete `scheduler: Option<Rc<RefCell<EngineScheduler>>>` field, `NewRootWithScheduler`, `GetScheduler`. Callers go through framework/ctx.
- `crates/emcore/src/emEngine.rs` — `emEngine::Cycle` signature becomes `fn Cycle(&mut self, ectx: &mut EngineCtx<'_>)` (pctx added only in Phase 2 when panel-tree types migrate; during Phase 1 the engines that take pctx carry a placeholder ad-hoc parameter — see Task 9).
- `crates/emcore/src/emViewPort.rs` — no structural changes in Phase 1; SwapViewPorts ctx-threading lands in Phase 2.

**Test files modified:**
- `crates/emcore/tests/sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs` — assert delta==0.
- `crates/emcore/tests/sp4_5_fix_1_timing_sched_drain_baseline_slices.rs` — assert delta==0.
- `crates/emcore/tests/sp4_5_fix_1_timing_subview_reinit_baseline_slices.rs` — assert delta==0.
- Existing in-file tests at `emView.rs:4998–5030` — migrated from `view.queue_or_apply_sched_op(SchedOp::Fire(sig))` to `ctx.fire(sig)` pattern.

**Files annotated-deleted (DIVERGED blocks removed):**
- `emView.rs` near the `SchedOp` enum — the IDIOM block is the sole codebase occurrence and is deleted in Task 4.
- `emScheduler.rs` near any SP8 / per-sub-view-scheduler DIVERGED block.

---

## Task 1: Introduce `EngineCtx` / `SchedCtx` / `ConstructCtx` scaffolding

**Files:**
- Create: `crates/emcore/src/emEngineCtx.rs`
- Create: `crates/emcore/src/emEngineCtx.rust_only` (one line marker)
- Modify: `crates/emcore/src/lib.rs` — add `pub mod emEngineCtx;`
- Test: `crates/emcore/src/emEngineCtx.rs` (inline `#[cfg(test)]` module)

- [ ] **Step 1: Write the failing test.**

Add to end of `emEngineCtx.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::emScheduler::EngineScheduler;

    #[test]
    fn sched_ctx_exposes_full_api() {
        let mut sched = EngineScheduler::new();
        let mut actions = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            current_engine: None,
        };
        let sig = sc.create_signal();
        sc.fire(sig);
        sc.remove_signal(sig);
    }
}
```

- [ ] **Step 2: Run test to verify it fails.**

Run: `cargo test -p emcore emEngineCtx::tests::sched_ctx_exposes_full_api 2>&1 | tail -20`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write minimal implementation.**

`emEngineCtx.rs`:
```rust
//! EngineCtx, SchedCtx, InitCtx — event-loop-threaded mutable-state bundles.
//!
//! This module replaces the `Rc<RefCell<EngineScheduler>>` ownership model.
//! See `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.1.

use std::collections::HashMap;
use std::rc::Rc;

use crate::emContext::emContext;
use crate::emScheduler::{EngineId, EngineScheduler, Priority, SignalId};

pub enum DeferredAction {
    CloseWindow(winit::window::WindowId),
    MaterializePopup(winit::window::WindowId),
    // additional variants populated as migration progresses (Phase 2+)
}

pub struct EngineCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub windows: &'a mut HashMap<winit::window::WindowId, crate::emWindow::emWindow>,
    pub root_context: &'a Rc<emContext>,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub current_engine: Option<EngineId>,
}

pub struct SchedCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
    pub current_engine: Option<EngineId>,
}

pub struct InitCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
}

pub trait ConstructCtx {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(&mut self, e: Box<dyn crate::emEngine::emEngine>, pri: Priority) -> EngineId;
    fn wake_up(&mut self, eng: EngineId);
}

impl<'a> EngineCtx<'a> {
    pub fn with_view_mut<R>(
        &mut self,
        window: winit::window::WindowId,
        f: impl FnOnce(&mut crate::emView::emView, &mut SchedCtx<'_>) -> R,
    ) -> Option<R> {
        let win = self.windows.get_mut(&window)?;
        let mut sched = SchedCtx {
            scheduler: &mut *self.scheduler,
            framework_actions: &mut *self.framework_actions,
            root_context: self.root_context,
            current_engine: self.current_engine,
        };
        Some(f(&mut win.view, &mut sched))
    }

    pub fn framework_action(&mut self, a: DeferredAction) { self.framework_actions.push(a); }

    pub fn create_signal(&mut self) -> SignalId { self.scheduler.create_signal() }
    pub fn fire(&mut self, sig: SignalId) { self.scheduler.fire(sig) }
    pub fn wake_up(&mut self, e: EngineId) { self.scheduler.wake_up(e) }
    pub fn connect(&mut self, sig: SignalId, e: EngineId) { self.scheduler.connect(sig, e) }
    pub fn disconnect(&mut self, sig: SignalId, e: EngineId) { self.scheduler.disconnect(sig, e) }
    pub fn remove_signal(&mut self, sig: SignalId) { self.scheduler.remove_signal(sig) }
    pub fn remove_engine(&mut self, e: EngineId) { self.scheduler.remove_engine(e) }
    pub fn register_engine(&mut self, b: Box<dyn crate::emEngine::emEngine>, pri: Priority) -> EngineId {
        self.scheduler.register_engine(b, pri)
    }
}

impl SchedCtx<'_> {
    pub fn create_signal(&mut self) -> SignalId { self.scheduler.create_signal() }
    pub fn fire(&mut self, sig: SignalId) { self.scheduler.fire(sig) }
    pub fn wake_up(&mut self, e: EngineId) { self.scheduler.wake_up(e) }
    pub fn connect(&mut self, sig: SignalId, e: EngineId) { self.scheduler.connect(sig, e) }
    pub fn disconnect(&mut self, sig: SignalId, e: EngineId) { self.scheduler.disconnect(sig, e) }
    pub fn remove_signal(&mut self, sig: SignalId) { self.scheduler.remove_signal(sig) }
    pub fn remove_engine(&mut self, e: EngineId) { self.scheduler.remove_engine(e) }
    pub fn register_engine(&mut self, b: Box<dyn crate::emEngine::emEngine>, pri: Priority) -> EngineId {
        self.scheduler.register_engine(b, pri)
    }
    pub fn framework_action(&mut self, a: DeferredAction) { self.framework_actions.push(a); }
}

impl ConstructCtx for SchedCtx<'_> {
    fn create_signal(&mut self) -> SignalId { self.scheduler.create_signal() }
    fn register_engine(&mut self, e: Box<dyn crate::emEngine::emEngine>, p: Priority) -> EngineId {
        self.scheduler.register_engine(e, p)
    }
    fn wake_up(&mut self, e: EngineId) { self.scheduler.wake_up(e) }
}

impl ConstructCtx for InitCtx<'_> {
    fn create_signal(&mut self) -> SignalId { self.scheduler.create_signal() }
    fn register_engine(&mut self, e: Box<dyn crate::emEngine::emEngine>, p: Priority) -> EngineId {
        self.scheduler.register_engine(e, p)
    }
    fn wake_up(&mut self, e: EngineId) { self.scheduler.wake_up(e) }
}
```

`emEngineCtx.rust_only`:
```
Rust-only: ctx structs translate C++'s raw-pointer event-loop threading to Rust &mut discipline; no direct C++ analogue.
```

Add `pub mod emEngineCtx;` to `crates/emcore/src/lib.rs` in alphabetical position among existing `em*` modules.

- [ ] **Step 4: Run test to verify it passes.**

Run: `cargo test -p emcore emEngineCtx::tests::sched_ctx_exposes_full_api`
Expected: PASS.

> **Argument-order note.** The adapter methods above call `self.scheduler.register_engine(b, pri)` — the *target* signature `register_engine(behavior, priority)` matches the new spec §4 D4.x design. The current `EngineScheduler::register_engine` (in `emScheduler.rs:149`) is declared as `(priority, behavior)` (legacy order). If you choose to defer the scheduler-side flip to Task 3, write Task 1's adapters as `self.scheduler.register_engine(pri, b)` to keep Task 1 compilable standalone, then flip both adapter and scheduler in Task 3 in a single commit. Either way the final state is `(behavior, priority)`. Pick one and document the choice in the ledger.

- [ ] **Step 5: Commit.**

```bash
git add crates/emcore/src/emEngineCtx.rs crates/emcore/src/emEngineCtx.rust_only crates/emcore/src/lib.rs
git commit -m "phase-1: introduce EngineCtx/SchedCtx/InitCtx scaffolding"
```

Append to ledger: `Task 1 done @ <sha>`.

---

## Task 2: Migrate `emGUIFramework::scheduler` from `Rc<RefCell<>>` to plain value

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs:89` (field decl) and every construction/use site in the same file.
- Modify: `crates/emcore/src/lib.rs` if any pub re-exports change type.

- [ ] **Step 1: Read current usages.** Run: `rg -n 'framework\.scheduler|self\.scheduler' crates/emcore/src/emGUIFramework.rs`. Capture all hits.

- [ ] **Step 2: Write the failing test.** Add to `emGUIFramework.rs`:
```rust
#[cfg(test)]
#[test]
fn framework_scheduler_is_plain_value() {
    let framework = emGUIFramework::new_for_test();
    // Statically assert: the type of framework.scheduler is EngineScheduler, not Rc<RefCell<...>>.
    let _: &EngineScheduler = &framework.scheduler;
}
```

- [ ] **Step 3: Run test.** Expected: compile FAIL (scheduler is currently `Rc<RefCell<EngineScheduler>>`).

- [ ] **Step 4: Rewrite field and constructions.**

Change `emGUIFramework.rs:89`:
```rust
// OLD
pub scheduler: Rc<RefCell<EngineScheduler>>,
// NEW
pub scheduler: EngineScheduler,
pub windows: std::collections::HashMap<WindowId, crate::emWindow::emWindow>,
pub framework_actions: Vec<crate::emEngineCtx::DeferredAction>,
pub pending_inputs: Vec<(WindowId, crate::input::InputEvent)>,
```

In the constructor (`emGUIFramework::new` and `new_for_test`), replace:
```rust
scheduler: Rc::new(RefCell::new(EngineScheduler::new())),
// with
scheduler: EngineScheduler::new(),
windows: std::collections::HashMap::new(),
framework_actions: Vec::new(),
pending_inputs: Vec::new(),
```

All internal call sites changing `self.scheduler.borrow()` / `self.scheduler.borrow_mut()` drop the borrow:
```rust
// OLD
self.scheduler.borrow_mut().DoTimeSlice(...)
// NEW
self.scheduler.DoTimeSlice(&mut self.windows, &self.root_context)
```

(DoTimeSlice's new signature lands in Task 3.)

- [ ] **Step 5: Run test.** Expected: PASS.

- [ ] **Step 6: Run full check.** `cargo check -p emcore 2>&1 | tail -40`. Expected: many downstream breakages (Tasks 3–11 resolve them). STOP-worthy breakage: anything outside `emGUIFramework.rs`, `emScheduler.rs`, `emContext.rs` — fix in the owning-file task rather than here.

- [ ] **Step 7: Commit** (with `--allow-empty` fallback if the tree is in an in-progress compile state — but the sub-step gate is `cargo check` passing at minimum; prefer to continue to Task 3 before committing if uncompilable).

```bash
git add crates/emcore/src/emGUIFramework.rs
git commit -m "phase-1: emGUIFramework::scheduler plain value"
```

---

## Task 3: Rewrite `EngineScheduler::DoTimeSlice` signature

**Files:**
- Modify: `crates/emcore/src/emScheduler.rs` (DoTimeSlice definition ~line 349-ish per existing take/put pattern)

- [ ] **Step 1: Locate DoTimeSlice.** Run: `rg -n 'fn DoTimeSlice' crates/emcore/src/emScheduler.rs`. Note line number.

- [ ] **Step 2: Rewrite signature.** From:
```rust
pub fn DoTimeSlice(&mut self) -> bool { ... }
```
To:
```rust
pub fn DoTimeSlice(
    &mut self,
    windows: &mut std::collections::HashMap<winit::window::WindowId, crate::emWindow::emWindow>,
    root_context: &std::rc::Rc<crate::emContext::emContext>,
) -> bool { ... }
```

- [ ] **Step 3: Rewrite Cycle dispatch loop.**

The loop that picks an engine and calls its `Cycle` method now constructs an `EngineCtx`:
```rust
let mut framework_actions_local = Vec::new();
let mut ctx = crate::emEngineCtx::EngineCtx {
    scheduler: self,
    windows,
    root_context,
    framework_actions: &mut framework_actions_local,
    current_engine: Some(eng_id),
};
// behavior.Cycle(&mut ctx) — preserve take/put; behavior is .take()n before call
let mut behavior = slot.behavior.take().expect("slot populated");
behavior.Cycle(&mut ctx);
slot.behavior = Some(behavior);
```

(The `emEngine::Cycle` trait signature change lands in Task 9. During Phase 1 you will land both atomically — Task 3 prepares the call shape; Task 9 updates the trait.)

- [ ] **Step 4: Compile smoke.** `cargo check -p emcore 2>&1 | tail`. Expect: breakages in call sites of `emEngine::Cycle` — that is Task 9's domain.

- [ ] **Step 5: Commit.**
```bash
git add crates/emcore/src/emScheduler.rs
git commit -m "phase-1: DoTimeSlice takes &mut windows + &root_context"
```

---

## Task 4: Delete `SchedOp` enum, `queue_or_apply_sched_op`, and `pending_sched_ops`

**Files:**
- Modify: `crates/emcore/src/emView.rs` — delete enum + helper + field + field initialization + the `IDIOM:` comment block.
- Modify: `crates/emcore/src/emPanelTree.rs:613–626` — delete the `SchedOp::RemoveEngine` call (replaced with inline `ctx.remove_engine` in Task 6).
- Modify: `crates/emcore/src/emPanelCtx.rs:49` — delete the `SchedOp::WakeUp` call (replaced with inline `ctx.wake_up` in Task 6).

- [ ] **Step 1: Locate the enum and helper.** `rg -n 'enum SchedOp|fn queue_or_apply_sched_op|pending_sched_ops' crates/emcore/src/emView.rs`.

- [ ] **Step 2: Write assertion test.** Add at end of `emView.rs`:
```rust
#[cfg(test)]
#[test]
fn sched_op_does_not_exist() {
    // This test exists so this file fails to compile if SchedOp is ever reintroduced.
    // If someone adds `pub enum SchedOp` or `pub(crate) enum SchedOp` below, this
    // will not catch it, but `scripts/verify_no_sched_op.sh` (Task 12) covers that.
    let _ = ();
}
```
(The real enforcement is the grep assertion in Closeout C4.)

- [ ] **Step 3: Delete the IDIOM block.** Find the comment block labeled `IDIOM:` near `SchedOp`. Delete the comment lines and the `enum SchedOp { ... }` declaration.

- [ ] **Step 4: Delete the field.** In `emView` struct (line ~around the view fields), delete `pending_sched_ops: Vec<SchedOp>,` and remove its initialization from `emView::new` / any constructors.

- [ ] **Step 5: Delete the helper.** Delete `fn queue_or_apply_sched_op(&mut self, op: SchedOp)` at line ~656. Delete every `drain_pending_sched_ops` site (rg for it first to enumerate — typically 5 call sites per spec).

- [ ] **Step 6: Replace 12 call sites in emView.rs.** For each of:
- line 1126 (Fire)
- line 1584 (Fire)
- line 1866 (Connect)
- line 1953 (Disconnect)
- line 1954 (RemoveSignal)
- line 1980 (Fire)
- line 3051 (Fire)
- line 3169 (WakeUp)
- line 3355 (Fire)
- lines 5009, 5019, 5026 (test-code Fire)

Replace `self.queue_or_apply_sched_op(SchedOp::Fire(sig))` with `ctx.fire(sig)`, `SchedOp::Connect(sig, e)` → `ctx.connect(sig, e)`, etc. The containing function's signature must already take `ctx: &mut SchedCtx<'_>` (or `EngineCtx`); the call-site task chains to Task 5 which threads ctx through emView methods.

(Because Task 5 threads ctx through emView methods, completing Task 4 in isolation will not compile. Task 4's commit therefore happens *after* Task 5 lands ctx-threading; the two tasks ship in one commit. This is explicitly noted as Task 4 being a "deletion manifest" rather than a standalone compile-green step.)

- [ ] **Step 7: Delete the `in emPanelTree.rs:626` call.**
```rust
// OLD
owning_view.queue_or_apply_sched_op(crate::emView::SchedOp::RemoveEngine(eid));
// NEW
ctx.remove_engine(eid);
```
Task 6 lands ctx-threading through emPanelTree; Task 4's edits to emPanelTree are staged but not committed until Task 6.

- [ ] **Step 8: Delete the `in emPanelCtx.rs:49` call.** Since `emPanelCtx.rs` is deleted entirely in Task 6, this just means leaving no code to delete — confirm Task 6 absorbs it.

- [ ] **Step 9: Commit with Task 5.** See Task 5 Step 7.

---

## Task 5: Thread ctx through emView methods

**Files:**
- Modify: `crates/emcore/src/emView.rs` — every method body calling a `ctx.*` must receive `ctx: &mut SchedCtx<'_>` or `EngineCtx<'_>` as a parameter.

- [ ] **Step 1: Enumerate methods that now need ctx.** Run:
```bash
rg -n 'fn [A-Z][A-Za-z0-9_]*\s*\(&mut self' crates/emcore/src/emView.rs | head -60
```
Cross-reference with the 12 call sites from Task 4. Every enclosing method takes ctx.

- [ ] **Step 2: For each method, add `ctx: &mut SchedCtx<'_>` parameter.** Propagate through callers transitively.

Callers inside emView pass `&mut *ctx` or re-borrow. Callers outside emView (in emWindow, emSubViewPanel, emEngine impls) are already inside a `with_view_mut` closure and have `sched: &mut SchedCtx<'_>` bound — pass that directly.

- [ ] **Step 3: Replace all 12 SchedOp call sites** (Task 4 Step 6) with direct ctx calls inside the ctx-threaded methods.

- [ ] **Step 4: Delete `close_signal_pending: bool` field.**

Locate at `emView.rs:257–261` (per spec §4 D4.5). Delete the field, its initialization, and replace the cache-read pattern with inline `ctx.IsSignaled(close_sig)` at the top of `emView::Update`.

- [ ] **Step 5: Delete SVPUpdSlice try_borrow fallback.**

Locate with `rg -n 'try_borrow' crates/emcore/src/emView.rs`. The SVPUpdSlice fallback at the "svp_upd_slice" path becomes a direct read: `ctx.IsSignaled(svp_sig)`.

- [ ] **Step 6: Compile.**
```bash
cargo check -p emcore 2>&1 | tail -30
```
Expect: clean (Task 4 deletions + Task 5 threading align). Any remaining breakages are in Tasks 6–9.

- [ ] **Step 7: Commit (Task 4 + Task 5 together).**
```bash
git add crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs
git commit -m "phase-1: delete SchedOp and pending_sched_ops; thread ctx through emView"
```

Append to ledger: `Tasks 4+5 done @ <sha>. close_signal_pending deleted. SVPUpdSlice try_borrow deleted.`

---

## Task 6: Rewrite `register_engine_for` to take ctx; delete `register_pending_engines`; absorb emPanelCtx.rs

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` — `register_engine_for` (line 558–598) takes `&mut impl ConstructCtx`; delete `register_pending_engines`.
- Delete: `crates/emcore/src/emPanelCtx.rs` entirely.
- Modify: `crates/emcore/src/lib.rs` — remove `pub mod emPanelCtx;`.

- [ ] **Step 1: Rewrite `register_engine_for`.**

```rust
pub fn register_engine_for(
    &mut self,
    panel_id: PanelId,
    engine: Box<dyn emEngine>,
    priority: Priority,
    ctx: &mut impl ConstructCtx,
) -> EngineId {
    let eid = ctx.register_engine(engine, priority);
    ctx.wake_up(eid);  // D4.11: register-and-wake in the same call; delta=0.
    self.panels.get_mut(&panel_id)
        .expect("panel exists")
        .engines.push(eid);
    eid
}
```

- [ ] **Step 2: Delete `register_pending_engines`** and the pending-engines queue field on `PanelTree`.

- [ ] **Step 3: Delete `emPanelCtx.rs`.** `rm crates/emcore/src/emPanelCtx.rs` and its marker (if any). Remove `pub mod emPanelCtx;` from `lib.rs`. Move any leftover useful code into `emEngineCtx.rs` with a comment referencing its provenance.

- [ ] **Step 4: Fix callers of `register_engine_for`.** `rg -n 'register_engine_for' crates/` — each caller now provides ctx.

- [ ] **Step 5: Compile smoke.** `cargo check -p emcore`. Expected: green (given Task 9's trait migration lands in the same phase).

- [ ] **Step 6: Commit.**
```bash
git add -A
git commit -m "phase-1: register_engine_for takes ctx; delete register_pending_engines + emPanelCtx.rs"
```

---

## Task 7: Delete per-sub-view scheduler; rewire `emSubViewPanel::Cycle` via shared ctx

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs` — delete `sub_scheduler` field, its construction, its `DoTimeSlice` call.
- Modify: `crates/emcore/src/emSubViewPanel.rs` — `PanelBehavior::Cycle` recurses sub-tree using outer ctx.

- [ ] **Step 1: Locate the field.** `rg -n 'sub_scheduler' crates/emcore/src/emSubViewPanel.rs`.

- [ ] **Step 2: Delete field + initialization + `DoTimeSlice` driver.**

- [ ] **Step 3: Rewrite `Cycle` to use outer ctx.** Per spec §3.3:
```rust
impl PanelBehavior for emSubViewPanel {
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx<'_>) {
        // Drive inner sub-tree using the outer scheduler; do NOT construct a new scheduler.
        // The inner PanelCtx re-borrows self.sub_tree — disjoint from pctx.tree.
        let mut inner_pctx = PanelCtx {
            tree: &mut self.sub_tree,
            current_panel: self.sub_root,
        };
        self.sub_view.DoSlice(ectx, &mut inner_pctx);
    }
}
```

- [ ] **Step 4: Delete the SP8 DIVERGED block near the sub_scheduler field declaration.**

- [ ] **Step 5: Compile smoke.** `cargo check -p emcore`.

- [ ] **Step 6: Commit.**
```bash
git add crates/emcore/src/emSubViewPanel.rs
git commit -m "phase-1: delete per-sub-view scheduler; sub-tree drives via shared ctx"
```

---

## Task 8: Delete `emContext::scheduler` field

**Files:**
- Modify: `crates/emcore/src/emContext.rs:48, 65, 106` — delete field, `NewRootWithScheduler`, `GetScheduler`.

- [ ] **Step 1: Enumerate callers.** `rg -n 'GetScheduler\(\)|NewRootWithScheduler' crates/`. Each caller migrates to ctx-based scheduler access (or direct framework access in non-Cycle code paths).

- [ ] **Step 2: Delete field + methods.**

- [ ] **Step 3: Migrate callers** identified in Step 1.

- [ ] **Step 4: Compile.** `cargo check -p emcore`.

- [ ] **Step 5: Commit.**
```bash
git add crates/emcore/src/emContext.rs
git commit -m "phase-1: delete emContext::scheduler field + GetScheduler/NewRootWithScheduler"
```

---

## Task 9: Migrate `emEngine::Cycle` trait signature + 5 engine impls

**Files:**
- Modify: `crates/emcore/src/emEngine.rs` — trait signature.
- Modify: `UpdateEngineClass`, `VisitingVAEngine`, `StartupEngine`, `PanelCycleEngine`, `PriSchedAgent` implementations — each takes `ectx` (and `pctx` where tree access is needed).

- [ ] **Step 1: Update the trait.**
```rust
pub trait emEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>);
}
```

(For panel-tree-walking engines, pctx is carried in a separate trait added in Phase 2. During Phase 1, PanelCycleEngine is the only such engine; it receives the tree through ad-hoc ctx-field access since Phase 1 ships with `windows: HashMap<WindowId, emWindow>` already in EngineCtx, and PanelCycleEngine holds `window_id` + `panel_id` and resolves both through ctx.)

- [ ] **Step 2: For each of the 5 engine impls, replace `borrow_mut` patterns with ctx calls.** The previous `self.view.upgrade()?.borrow_mut()` becomes `ctx.with_view_mut(self.window_id, |view, sched| { ... })`.

- [ ] **Step 3: Run `cargo check -p emcore`.** Any remaining breakages: enumerate with `cargo check 2>&1 | grep error`. Fix until green.

- [ ] **Step 4: Run nextest.**
```bash
cargo-nextest ntr 2>&1 | tail -10
```
Expected: all green (≥ baseline count). If any test fails, fix before committing.

- [ ] **Step 5: Commit.**
```bash
git add -A
git commit -m "phase-1: migrate emEngine::Cycle trait + 5 engine impls to ctx-based"
```

---

## Task 10: Delete SP4.5-FIX-2 popup pre-allocation; replace with inline `ctx.create_signal() × 4`

**Files:**
- Modify: `crates/emcore/src/emView.rs` — the `RawVisitAbs` popup-signal allocation path (per spec §4 D4.7).

- [ ] **Step 1: Locate the pre-allocated popup signals.** `rg -n 'popup.*signal|SP4.5-FIX-2|pre.allocat' crates/emcore/src/emView.rs`.

- [ ] **Step 2: Delete pre-allocation; insert inline `ctx.create_signal()` at the four use sites.** Per spec §4 D4.7.

- [ ] **Step 3: Run the SP4.5-FIX-2 regression test.**
```bash
cargo test -p emcore sp4_5_fix_2 2>&1 | tail
```
Expected: PASS without panic.

- [ ] **Step 4: Commit.**
```bash
git add crates/emcore/src/emView.rs
git commit -m "phase-1: SP4.5-FIX-2 inline popup signal allocation via ctx"
```

---

## Task 11: Update SP4.5-FIX-3 timing fixtures to assert `delta == 0`

**Files:**
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs`
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_sched_drain_baseline_slices.rs`
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_subview_reinit_baseline_slices.rs`

- [ ] **Step 1: Read each fixture.** Note the current `assert_eq!(delta, 1)` (the +1 slice drift).

- [ ] **Step 2: Rewrite each to `assert_eq!(delta, 0)` with a comment** citing spec §4 D4.6 / D4.11.

- [ ] **Step 3: Run the three fixtures.**
```bash
cargo test -p emcore sp4_5_fix_1_timing 2>&1 | tail -10
```
Expected: 3/3 PASS with delta==0.

- [ ] **Step 4: Commit.**
```bash
git add crates/emcore/tests/sp4_5_fix_1_timing_*.rs
git commit -m "phase-1: sp4.5-fix-3 delta=0 by construction"
```

---

## Task 12: Run full gate; stage closeout

- [ ] **Step 1: Run full gate.**
```bash
cargo fmt --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo-nextest ntr && \
cargo test --test golden -- --test-threads=1
```

- [ ] **Step 2: If any check fails, fix in a new commit, then re-run the full gate.** Do not proceed with red state.

- [ ] **Step 3: Run invariant assertions I1, I1a, I1b, I1c, I1d, I5 from the phase header.**
```bash
rg 'Rc<RefCell<EngineScheduler>>' crates/ && echo "I1 FAIL" || echo "I1 PASS"
rg -w 'SchedOp' crates/ && echo "I1a FAIL" || echo "I1a PASS"
rg 'pending_sched_ops|queue_or_apply_sched_op|register_pending_engines|close_signal_pending' crates/ && echo "I1b FAIL" || echo "I1b PASS"
rg 'sub_scheduler' crates/ && echo "I1c FAIL" || echo "I1c PASS"
rg 'try_borrow' crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs && echo "I1d FAIL" || echo "I1d PASS"
rg 'IDIOM:' crates/ && echo "I5 FAIL" || echo "I5 PASS"
```

All must print `PASS`. Any FAIL: STOP, fix, re-run.

- [ ] **Step 4: Proceed to Closeout.**

---

## Closeout (per shared ritual)

Run steps C1–C11 from `2026-04-19-port-rewrite-bootstrap-ritual.md`. Substitute `<N>` with `1`.

At C4, verify invariants I1, I1a, I1b, I1c, I1d, I5 above.
At C5, for each of E001/E002/E003/E004/E005/E007/E008/E009/E010/E011/E036, cite the commit that closed it and the invariant that witnesses closure (e.g. `E002 → Task 2 commit <sha>, witnessed by I1`).
At C6, update the JSON marking the 11 entries `resolved-phase-1` with the commit SHAs.
At C7, write the closeout note.

---

## Self-review checklist (executor runs before Closeout)

Before C1, confirm:
- [ ] Every `ctx.fire/connect/disconnect/wake_up/remove_signal/remove_engine` site is reachable from a threaded `&mut SchedCtx` or `&mut EngineCtx` — no `self.scheduler.borrow_mut()` remains.
- [ ] Every `SchedOp::` literal is gone from the tree.
- [ ] The 3 timing fixtures assert `delta == 0` not `delta == 1`.
- [ ] `cargo xtask annotations` is not yet in use (Phase 5) — do not add it here.
- [ ] No `IDIOM:` comment remains in-tree.
- [ ] Goldens 237/6 (or better) preserved.
