# Phase 1.75 — Sub-view Scheduler Narrowing + ctx Cleanup (Phase-1.5 deferreds)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. This phase is the re-plan of the Tasks 2–5 deferred by Phase 1.5's PARTIAL closeout. The prior drafts assumed `emSubViewPanel::sub_scheduler` could be deleted outright; this plan corrects that premise and keeps `sub_scheduler` as a plain-owned field, chartering it in the spec.

**Goal.** Close the three residual cleanup goals carried forward from Phase 1.5 PARTIAL:
1. Narrow `emSubViewPanel::sub_scheduler: Rc<RefCell<EngineScheduler>>` → plain-owned `EngineScheduler` (eliminating the last `Rc<RefCell<EngineScheduler>>` declaration site and the `try_borrow_mut` patterns around it).
2. Re-signature `PanelTree::register_engine_for` to take ctx; delete `register_pending_engines` + its backing queue; delete `crates/emcore/src/emPanelCtx.rs` (absorb `PanelCtx` into `emEngineCtx.rs`).
3. Inline popup-signal allocation (spec §4 D4.7) and rewrite the three `sp4_5_fix_1_timing_*` fixtures to `delta==0` (spec §4 D4.6 / D4.11).

Goal stated as invariants:

- **I1c'.** `rg 'Rc<RefCell<EngineScheduler>>' crates/` returns zero matches. (Strengthened from "sub_scheduler eliminated": the *Rc/RefCell wrapper* is what Phase 1.5's goal was about; the `sub_scheduler: EngineScheduler` field survives as a plain value, chartered in spec §3.3.)
- **I1c''.** `rg -w 'sub_scheduler' crates/` may return matches (chartered), but every match is on a plain `EngineScheduler` (no `Rc<RefCell<>>` around it). Grep shape: `rg 'sub_scheduler: Rc<' crates/` returns zero.
- **I1d'.** `rg 'try_borrow(_mut)?\(\)' crates/emcore/src/emSubViewPanel.rs` returns zero matches (the post-1e.1 residual at the four sub_scheduler sites is gone).
- **I-T3a.** `rg -w 'register_pending_engines' crates/` returns zero matches.
- **I-T3b.** `test -e crates/emcore/src/emPanelCtx.rs` returns non-zero (file is deleted). `rg 'pub mod emPanelCtx' crates/emcore/src/lib.rs` returns zero.
- **I-T3c.** All `PanelCtx` references in `crates/` resolve through `crate::emEngineCtx::PanelCtx` (a single import path).
- **Task-10.** No pre-allocated popup signals block in `emView::RawVisitAbs` or its helpers — `ctx.create_signal()` inline at the 4 popup-signal use sites (spec §4 D4.7).
- **Task-11.** `sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs`, `_sched_drain_baseline_slices.rs`, `_subview_reinit_baseline_slices.rs` all assert `delta == 0`.
- **I-Spec-3.3-amended.** `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.3 explicitly charters `emSubViewPanel::sub_scheduler: EngineScheduler` as a preserved per-sub-view scheduler (reason: `PanelCycleEngine::Cycle` resolves panels via `ctx.tree.take_behavior(panel_id)` using slab-local `PanelId`s, which cannot be routed across trees from a shared outer scheduler). §4 and §10/Phase-1 invariant language revised accordingly.

**Tech stack:** unchanged. No new dependencies.

**Architecture.** The sub-view scheduler remains per-`emSubViewPanel`. Outer scheduler drives it once per outer slice from `emSubViewPanel::Cycle`, passing `&mut self.sub_scheduler` directly (no Rc/RefCell). `PanelTree::register_engine_for` takes a `ConstructCtx` trait object implemented by both `SchedCtx<'_>` (outer cycle path) and a new `BareSchedCtx<'a>` wrapper around `&'a mut EngineScheduler` (init path, including `emSubViewPanel::new`). `emPanelCtx.rs` is deleted; `PanelCtx` lives in `emEngineCtx.rs` alongside the other ctx shapes.

**Companion documents:**
- Spec: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.3 (amended by this phase), §3.6, §4 D4.6/D4.7/D4.11.
- Phase 1.5 plan (superseded for Tasks 2–5): `docs/superpowers/plans/2026-04-19-port-rewrite-phase-1-5-keystone-migration.md`.
- Phase 1.5 closeout: `docs/superpowers/notes/2026-04-19-phase-1-5-closeout.md`.
- Phase 1.5 ledger: `docs/superpowers/notes/2026-04-19-phase-1-5-ledger.md` (read the BLOCKED dispatch for Task 2; this plan's Option-G rationale is informed by it).
- Bootstrap/closeout ritual: `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md`.

**Entry precondition.** Phase 1.5 closeout note (`docs/superpowers/notes/2026-04-19-phase-1-5-closeout.md`) has `Status: PARTIAL — Task 1 complete; Tasks 2–5 deferred`. Branch tagged `port-rewrite-phase-1-5-partial-complete`. Main at or ahead of that tag (currently `5060b9b`). Working tree clean.

This is a sanctioned PARTIAL predecessor — Phase 1.75 exists precisely to close those deferred tasks. At Bootstrap step B4, record the PARTIAL read in the Phase 1.75 ledger and do NOT halt; cite this plan's Entry precondition as the sanctioning record.

**Baseline.** Phase 1.5 exit metrics, from `docs/superpowers/notes/2026-04-19-phase-1-5-exit.md`: nextest 2455/0/9, goldens 237/6, `rc_refcell_total=282`, `diverged_total=177`, `rust_only_total=17`, `idiom_total=0`, `try_borrow_total=5`.

**JSON entries closed:** to be enumerated at Closeout C5 by sweeping `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` for any entries tied to `sub_scheduler`, `register_pending_engines`, `emPanelCtx.rs`, popup-signal pre-allocation, or timing-fixture deltas whose `status` is still `carry-forward-*`.

---

## Bootstrap (per shared ritual)

Run steps B1–B12 from `2026-04-19-port-rewrite-bootstrap-ritual.md`. Substitute `<N>` with `1-75`.

Important deviations from the standard ritual:

- **B4.** Locate `2026-04-19-phase-1-5-closeout.md`. Its `## Status` line reads `PARTIAL — Task 1 complete; Tasks 2–5 deferred to a follow-on phase`. This is the **second** sanctioned case where PARTIAL (not COMPLETE) is accepted as a bootstrap predecessor — because Phase 1.75 exists precisely to close Phase 1.5's deferred tasks. Record the read in the Phase 1.75 ledger; do NOT halt on the PARTIAL status.
- **B7.** Baseline is Phase 1.5 exit state (see Entry precondition above). Capture verbatim.
- **B9.** Branch: `port-rewrite/phase-1-75`.
- **B11.** Bootstrap commit message: `phase-1-75: bootstrap — baseline captured, ledger opened`.

---

## File Structure

**Files heavily modified:**
- `crates/emcore/src/emSubViewPanel.rs` — field `sub_scheduler: Rc<RefCell<EngineScheduler>>` → `sub_scheduler: EngineScheduler`; construction drops Rc/RefCell wrap; `Cycle`/`new` sites use `&mut self.sub_scheduler`; `try_borrow_mut` patterns deleted; `DIVERGED:` block at `:34-42` rewritten to reflect chartered per-sub-view scheduler (not forced-via-missing-alternative).
- `crates/emcore/src/emPanelTree.rs` — `register_engine_for` signature `fn(&mut self, PanelId, Option<&mut EngineScheduler>)` → `fn<C: ConstructCtx>(&mut self, PanelId, &mut C)`; `register_pending_engines` + its backing state (if any survives) deleted; `init_panel_view` + `create_child` signatures updated to match.
- `crates/emcore/src/emEngineCtx.rs` — absorb `PanelCtx` struct + its impl block; introduce `trait ConstructCtx` with blanket impls for `SchedCtx<'_>`, `EngineCtx<'_>`, and new `BareSchedCtx<'_>`; optionally `InitCtx<'_>` if it exists post-Phase-1.
- `crates/emcore/src/emView.rs` — `RawVisitAbs` popup pre-allocation (the `let (close_sig, flags_sig, focus_sig, geom_sig) = ...` block) replaced by inline `ctx.create_signal()` at the four use sites.
- `crates/emcore/src/emGUIFramework.rs` — if any `app.sub_scheduler`-adjacent code exists (there shouldn't post-Phase-1.5), migrate to new shape.
- `crates/emcore/src/emPanel.rs` — `PanelBehavior` callers passing `pctx.scheduler: Option<&mut EngineScheduler>` adjusted if the `ConstructCtx` refactor bleeds into PanelCtx construction.

**Files deleted:**
- `crates/emcore/src/emPanelCtx.rs` — `PanelCtx` moves to `emEngineCtx.rs`. Any filesystem marker (`emPanelCtx.no_rust_equivalent`, etc.) deleted alongside.

**Files heavily touched by import updates:**
- Every file in `crates/` that has `use crate::emPanelCtx::PanelCtx;` (from the earlier grep: at least ~40 sites across `emmain/`, `eaglemode/tests/`, `emstocks/`, `emcore/tests/`, `emcore/src/`). Bulk sed replacement to `use crate::emEngineCtx::PanelCtx;`.
- `crates/emcore/src/lib.rs` — `pub mod emPanelCtx;` removed; existing `pub use emPanelCtx::PanelCtx;` re-export (if present) relocated to re-export from `emEngineCtx`.

**Test files modified:**
- `crates/emcore/tests/sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs` — `assert_eq!(delta, 1)` → `assert_eq!(delta, 0)`, with a comment citing spec §4 D4.6/D4.11.
- `crates/emcore/tests/sp4_5_fix_1_timing_sched_drain_baseline_slices.rs` — same treatment.
- `crates/emcore/tests/sp4_5_fix_1_timing_subview_reinit_baseline_slices.rs` — same treatment.
- `crates/emcore/tests/sp4_5_fix_2_popup_signal_*` (if any) — expect PASS without panic once popup signals are inline.
- Any in-file `#[cfg(test)]` module in `emPanelTree.rs` that constructs a bare `EngineScheduler` + calls `register_pending_engines` directly — rewritten to use the new `BareSchedCtx` shape.

**Files where DIVERGED blocks change:**
- `emSubViewPanel.rs:34-42` — the existing block classifies per-sub-view scheduler as **forced** ("EngineCtx::tree is singular, so a single scheduler cannot cycle engines across two trees"). Keep the forced-divergence classification (the constraint is real — slab-local PanelIds), but rewrite the text to:
  - Name the constraint precisely: **"`PanelCycleEngine::Cycle` resolves its panel via `ctx.tree.take_behavior(panel_id)`; `PanelId` is slab-local to a single `PanelTree`, so one scheduler's awake-engine queue cannot index panels across two trees."**
  - Reference spec §3.3 (post-amendment) as the canonical charter.
  - Drop the `Rc<RefCell<>>` descriptor — the wrapper is gone.

---

## Task 1: Spec §3.3 amendment + Phase-1 invariant re-statement

**Files:**
- Modify: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md`

This task lands as a docs-only commit *before* any code edit, so subsequent review of code changes has an updated spec to compare against.

- [ ] **Step 1.** In spec §3.3, replace the paragraph beginning "`emSubViewPanel` owns `sub_view: emView` and `sub_tree: PanelTree` as plain values." with:

    > `emSubViewPanel` owns `sub_view: emView`, `sub_tree: PanelTree`, and `sub_scheduler: EngineScheduler` as plain values. Its `PanelBehavior::Cycle` receives `(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx<'_>)` and drives one slice of the sub-scheduler against the sub-tree using `ectx.root_context` / `ectx.framework_actions`.
    >
    > **Why a per-sub-view scheduler is preserved.** `PanelCycleEngine::Cycle` (the adapter that drives a panel's `PanelBehavior::Cycle`) resolves its panel via `ctx.tree.take_behavior(panel_id)`. `PanelId` is a `slab::Key` scoped to a single `PanelTree` instance; two trees share no id-space. A shared outer scheduler's awake-engine queue therefore cannot dispatch panel cycles across both the outer tree and a sub-tree — the adapter would look up the wrong tree. This is a forced Rust consequence of slab-local keys; C++ avoided it by tree-rooted pointer-stable `emPanel*`. Sub-view scheduler is chartered under §3.6(d) **tree-scoped scheduler** (new category below).

- [ ] **Step 2.** In spec §3.3, rewrite the "Observational argument" paragraph (currently ends "shared scheduler's clock") to match reality:

    > **Observational argument.** C++ has one process-wide `emScheduler` indexed by pointer-stable `emPanel*`. Rust's slab-local `PanelId` forces structural separation: each `PanelTree` carries its own scheduler. Observable equivalence is preserved by (a) driving each sub-scheduler exactly once per owning outer slice inside `emSubViewPanel::Cycle`, and (b) ensuring sub-scheduler deadlines/wake-flags are derived from the outer slice's clock rather than set independently. Tests for this: the `sp4_5_fix_1_timing_*` fixtures (post-Task 3) assert `delta == 0` between outer wake and sub-view engine first Cycle, confirming the single-slice drive preserves the C++ observable "engines fire in the slice that wakes them" invariant.

- [ ] **Step 3.** In spec §3.6, add a new chartered category and row:

    > (d) **Tree-scoped scheduler:** a scheduler whose awake-engine queue indexes `PanelId` keys scoped to one `PanelTree`. One declaration per `emSubViewPanel` instance.

    Chartered-sites table add:

    | Category | Site | Rationale |
    |---|---|---|
    | (d) | `emSubViewPanel::sub_scheduler: EngineScheduler` (1 decl per sub-view-panel instance; typically 0–2 active) | `PanelId` is slab-local; one scheduler cannot route awake-engine dispatch across two trees. |

- [ ] **Step 4.** In spec §4 D4.1 through D4.11, where any bullet still reads "the only scheduler" or "the single scheduler" as if one exists globally, qualify to "the outer scheduler" or "each tree's scheduler" as appropriate. (Grep the §4 subsection for "single scheduler" / "one scheduler" and adjust.)

- [ ] **Step 5.** In spec §10 Phase 1 invariant list, rewrite the bullet that reads "`sub_scheduler` eliminated" to "`sub_scheduler` narrowed from `Rc<RefCell<EngineScheduler>>` to plain `EngineScheduler` (Phase 1.75 Task 2)". Cross-reference Phase 1.75 plan.

- [ ] **Step 6.** Commit: `phase-1-75 task-1: amend spec §3.3 — charter per-sub-view scheduler; rewrite observational argument`.

**Invariant satisfied by end of Task 1:** I-Spec-3.3-amended.

---

## Task 2: Narrow `emSubViewPanel::sub_scheduler` to plain `EngineScheduler`

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs`
- Possibly touch: `crates/emcore/src/emPanelTree.rs` (only if the `register_pending_engines` call in `emSubViewPanel::new` at line 83 survives into Task 3's surface; this task leaves that call alone and lets Task 3 delete it).

- [ ] **Step 1.** Change field declaration at `emSubViewPanel.rs:43`:

    From: `pub(crate) sub_scheduler: std::rc::Rc<std::cell::RefCell<crate::emScheduler::EngineScheduler>>,`
    To: `pub(crate) sub_scheduler: crate::emScheduler::EngineScheduler,`

- [ ] **Step 2.** Rewrite construction at `emSubViewPanel::new` (`:65-83`):
    - Drop the `Rc::new(RefCell::new(...))` wrap around `EngineScheduler::new()`.
    - `sub_scheduler.borrow_mut()` call sites collapse to direct `&mut sub_scheduler`.
    - At the `RegisterEngines` call (`:73-80`), `SchedCtx { scheduler: &mut s, ... }` now borrows `&mut sub_scheduler` directly (no `.borrow_mut()` intermediate).
    - The `register_pending_engines(&mut sub_scheduler.borrow_mut())` call at `:83` becomes `register_pending_engines(&mut sub_scheduler)` for this task; Task 3 will delete the call entirely.

- [ ] **Step 3.** Rewrite the body of `PanelBehavior::Cycle` at `:344-407`:
    - The animator tick block (`:362-381`) currently does `self.sub_scheduler.borrow_mut()`; collapse to `&mut self.sub_scheduler`. The `drop(sched_anim)` line is deleted (no guard to drop).
    - The `self.sub_scheduler.borrow_mut().DoTimeSlice(...)` call at `:392` becomes `self.sub_scheduler.DoTimeSlice(...)`.
    - The `self.sub_tree.register_pending_engines(&mut self.sub_scheduler.borrow_mut())` at `:402-403` becomes `self.sub_tree.register_pending_engines(&mut self.sub_scheduler)` (Task 3 will delete this entire line).
    - The `self.sub_scheduler.borrow().has_awake_engines()` at `:406` becomes `self.sub_scheduler.has_awake_engines()`.

- [ ] **Step 4.** Grep-verify zero matches in `emSubViewPanel.rs`:
    ```
    rg 'sub_scheduler\.borrow' crates/emcore/src/emSubViewPanel.rs
    rg 'Rc<RefCell<.*EngineScheduler' crates/emcore/src/emSubViewPanel.rs
    rg 'try_borrow' crates/emcore/src/emSubViewPanel.rs
    ```
    All three return zero.

- [ ] **Step 5.** Rewrite the `DIVERGED:` block at `emSubViewPanel.rs:34-42` per File Structure above. Keep the `DIVERGED:` classification (forced), but update the rationale to cite slab-local PanelIds and reference spec §3.3 (post-amendment).

- [ ] **Step 6.** Run `cargo check --all-targets`. Fix any caller that reaches the field expecting `Rc<RefCell<>>` — most likely tests constructing bare `emSubViewPanel` or touching `sub_scheduler` externally. If any production code outside `emSubViewPanel.rs` matches `sub_scheduler` (grep-verify), the rewire moves with this task.

- [ ] **Step 7.** Full gate: `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings && cargo-nextest ntr && cargo test --test golden -- --test-threads=1`. All must pass.

- [ ] **Step 8.** Commit: `phase-1-75 task-2: narrow emSubViewPanel::sub_scheduler to plain EngineScheduler`.

**Invariants satisfied by end of Task 2:** I1c', I1c'', I1d'.

**Metric target:** `rc_refcell_total` −1 (the `Rc<RefCell<EngineScheduler>>` decl at `:43`). `try_borrow_total` −4 (the four `.borrow_mut()`/`.borrow()` sites in the file — not `try_borrow`, so this mostly does not move `try_borrow_total` directly; the metric that moves is the file-local `.borrow` count which is tracked separately as a cleanup signal, not a grep-assertion).

---

## Task 3: `register_engine_for(ctx)` + `register_pending_engines` deletion + `emPanelCtx.rs` deletion

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs`
- Modify: `crates/emcore/src/emEngineCtx.rs` (absorb `PanelCtx` + add `ConstructCtx` trait)
- Modify: `crates/emcore/src/emView.rs`, `crates/emcore/src/emSubViewPanel.rs`, `crates/emcore/src/emPanel.rs` (and all other callers, per bulk import update)
- Modify: `crates/emcore/src/lib.rs` (remove `pub mod emPanelCtx;`; relocate `PanelCtx` re-export)
- Delete: `crates/emcore/src/emPanelCtx.rs`

**The `ConstructCtx` trait.** A disjoint-borrow-friendly abstraction that lets both outer-cycle and init-time callers register engines uniformly without duplicating `register_engine_for` signatures.

```rust
// crates/emcore/src/emEngineCtx.rs
pub trait ConstructCtx {
    fn scheduler_mut(&mut self) -> &mut EngineScheduler;
}

impl ConstructCtx for SchedCtx<'_> { fn scheduler_mut(&mut self) -> &mut EngineScheduler { &mut *self.scheduler } }
impl ConstructCtx for EngineCtx<'_> { fn scheduler_mut(&mut self) -> &mut EngineScheduler { &mut *self.scheduler } }

pub struct BareSchedCtx<'a> { pub scheduler: &'a mut EngineScheduler }
impl ConstructCtx for BareSchedCtx<'_> { fn scheduler_mut(&mut self) -> &mut EngineScheduler { &mut *self.scheduler } }
```

`BareSchedCtx` is the init-time wrapper used by `emSubViewPanel::new` to pass `&mut sub_scheduler` through `register_engine_for`.

- [ ] **Step 1.** Move the `PanelCtx` struct + impl block from `emPanelCtx.rs` into `emEngineCtx.rs`. Preserve the public API verbatim. Add a `// SPLIT-MERGED: formerly emPanelCtx.rs; absorbed into emEngineCtx.rs per Phase-1.75` comment at the top of the PanelCtx block.

- [ ] **Step 2.** Introduce the `ConstructCtx` trait + `BareSchedCtx` wrapper in `emEngineCtx.rs` per the sketch above.

- [ ] **Step 3.** Delete `crates/emcore/src/emPanelCtx.rs`. Remove `pub mod emPanelCtx;` from `crates/emcore/src/lib.rs`. Add `pub use emEngineCtx::PanelCtx;` (or adjust the existing re-export) so the public symbol `emcore::PanelCtx` continues to resolve.

- [ ] **Step 4.** Rewrite `PanelTree::register_engine_for` signature (`emPanelTree.rs:576`):

    From: `fn register_engine_for(&mut self, id: PanelId, sched: Option<&mut EngineScheduler>)`
    To: `fn register_engine_for<C: ConstructCtx>(&mut self, id: PanelId, ctx: &mut C)`

    Body: replace `let Some(sched) = sched else { return; };` with direct use of `ctx.scheduler_mut()`. The `None`-scheduler early-return goes away — the caller is now required to provide a scheduler (the prior `None` branch existed only to support pre-`register_pending_engines` timing).

- [ ] **Step 5.** Delete `PanelTree::register_pending_engines` (`emPanelTree.rs:617-622`) and any backing pending-engines queue/state field on `PanelTree`. Delete the test-scoped `register_pending_engines` callers or rewrite them to use `BareSchedCtx { scheduler: &mut sched }`. Grep-verify `rg -w 'register_pending_engines' crates/` returns zero.

- [ ] **Step 6.** Update `init_panel_view` and `create_child` signatures on `PanelTree` to take `&mut C: ConstructCtx` (or remove the `Option<&mut EngineScheduler>` parameter entirely). Cascade through callers.

- [ ] **Step 7.** Update `emSubViewPanel::new` (`emSubViewPanel.rs:53-96`):
    - The `sub_tree.register_pending_engines(&mut sub_scheduler)` call at `:83` is deleted.
    - Engine registration now happens inline via the `RegisterEngines` + `create_root`/`init_panel_view` paths, each of which takes `&mut BareSchedCtx { scheduler: &mut sub_scheduler }`.
    - The `SchedCtx { scheduler: &mut s, ... }` block at `:73-80` remains (it's for `RegisterEngines`, not `register_pending_engines`).

- [ ] **Step 8.** Update `emSubViewPanel::Cycle` (`emSubViewPanel.rs:402-403`): delete the `self.sub_tree.register_pending_engines(&mut self.sub_scheduler)` line (inherited from Task 2's interim rewrite). No replacement — `register_engine_for` is now called synchronously at `create_child` time with ctx in hand, so the catch-up sweep is unnecessary.

- [ ] **Step 9.** Bulk import update across `crates/`: `sed -i 's|use crate::emPanelCtx::PanelCtx|use crate::emEngineCtx::PanelCtx|' <files>`. Verify with `rg 'emPanelCtx::' crates/` returns zero (outside git-history noise).

- [ ] **Step 10.** Any filesystem marker (`crates/emcore/src/emPanelCtx.no_rust_equivalent`, `.rust_only`, etc.) adjacent to the deleted file is deleted. (The Rust file deletion itself follows File and Name Correspondence: C++ has no `emPanelCtx.h`, so no `emPanelCtx.no_rust_equivalent` marker should appear post-deletion — confirm or add.)

- [ ] **Step 11.** Full gate. All must pass.

- [ ] **Step 12.** Commit: `phase-1-75 task-3: register_engine_for takes ctx; delete register_pending_engines + emPanelCtx.rs`.

**Invariants satisfied by end of Task 3:** I-T3a, I-T3b, I-T3c.

---

## Task 4: Task-10 (popup signals inline) + Task-11 (timing fixtures delta=0)

**Files:**
- Modify: `crates/emcore/src/emView.rs` (popup-signal pre-allocation block in/near `RawVisitAbs`)
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_panel_reinit_baseline_slices.rs`
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_sched_drain_baseline_slices.rs`
- Modify: `crates/emcore/tests/sp4_5_fix_1_timing_subview_reinit_baseline_slices.rs`

- [ ] **Step 1.** In `emView.rs`, locate the popup pre-allocation block (grep for `let (close_sig, flags_sig, focus_sig, geom_sig)` or similar — the four pre-allocated `SignalId`s for popup lifecycle). Replace each with an inline `ctx.create_signal()` at the use site per spec §4 D4.7. If any pre-allocated block member is unused post-inlining, delete the block entirely.

- [ ] **Step 2.** Verify `sp4_5_fix_2_*` (if present) passes without panic. If no `sp4_5_fix_2_*` tests exist, grep for any test that asserts "no pre-allocated popup signals" or similar; otherwise the invariant is the grep-assertion in Task-10.

- [ ] **Step 3.** In each of the three `sp4_5_fix_1_timing_*_baseline_slices.rs` fixtures, rewrite `assert_eq!(delta, 1, ...)` to `assert_eq!(delta, 0, ...)` with an adjacent comment citing spec §4 D4.6/D4.11.

- [ ] **Step 4.** Run `cargo test -p emcore sp4_5_fix_1_timing` — expected 3/3 PASS. If any fixture now passes at `delta==0` because underlying code already fires in the right slice, the change is a pure assertion update. If any fails, the underlying scheduler/view timing needs adjustment per §4 D4.6 — record in ledger and address as a scoped follow-up; do NOT lower the assertion back to `delta==1`.

- [ ] **Step 5.** Full gate.

- [ ] **Step 6.** Commit: `phase-1-75 task-4: inline popup signals; SP4.5-FIX-3 delta=0 by construction`.

**Invariants satisfied by end of Task 4:** Task-10, Task-11.

---

## Task 5: Full gate + Closeout prep

- [ ] **Step 1.** Run the full gate one more time:
    ```bash
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo-nextest ntr
    cargo test --test golden -- --test-threads=1
    ```

- [ ] **Step 2.** Verify all Phase 1.75 invariants (I1c', I1c'', I1d', I-T3a, I-T3b, I-T3c, I-Spec-3.3-amended, Task-10, Task-11). Record pass/fail in the ledger.

- [ ] **Step 3.** Verify all Phase-1 carry-forward invariants still hold (I1, I1a, I1b, I1d, I6 must remain SAT from Phase 1.5 exit). Explicit grep assertions:
    ```
    rg 'Rc<RefCell<EngineScheduler>>' crates/          # must be 0 (I1 + I1c')
    rg -w 'SchedOp' crates/                            # must be 0 (I1a)
    rg 'pending_sched_ops|queue_or_apply_sched_op|close_signal_pending' crates/  # must be 0 (I1b)
    rg 'try_borrow(_mut)?\(\)' crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs  # must be 0 (I1d)
    rg 'NewRootWithScheduler|fn GetScheduler' crates/  # must be 0 (I6)
    ```

- [ ] **Step 4.** Proceed to Closeout.

---

## Closeout (per shared ritual)

Run steps C1–C11 from `2026-04-19-port-rewrite-bootstrap-ritual.md`. Substitute `<N>` with `1-75`.

Specific Phase 1.75 requirements:

- **C4.** Verify invariants I1c', I1c'', I1d', I-T3a, I-T3b, I-T3c, I-Spec-3.3-amended, Task-10, Task-11. Verify Phase-1 carry-forward invariants (I1, I1a, I1b, I1d, I6) remain SAT.
- **C5.** Enumerate any carry-forward JSON entries in `2026-04-19-port-divergence-raw-material.json` that this phase closes. Candidates: entries tied to `sub_scheduler`, `register_pending_engines`, `emPanelCtx.rs`, popup-signal pre-allocation, timing-fixture deltas. For each, cite the Phase 1.75 commit that closed it.
- **C6.** Update the JSON: mark each closed entry `status: resolved-phase-1-75` + add `resolution_commit`. Commit: `phase-1-75: mark JSON entries <list> resolved`.
- **C7.** Closeout note status line: `COMPLETE — all C1–C11 checks passed`.
- **C10.** Tag: `port-rewrite-phase-1-75-complete`.

---

## Next-phase unblock

At end of Phase 1.75 Closeout, Phase 2 (`docs/superpowers/plans/2026-04-19-port-rewrite-phase-2-view-window-composition.md`) Entry precondition is satisfied *except* that Phase 2's header reads "Phase 1 Closeout COMPLETE" — which was never achieved (Phase 1 closed PARTIAL, Phase 1.5 closed PARTIAL). Phase 2's B4 deviation must be added before Phase 2 runs, analogous to Phase 1.5's B4 deviation for Phase 1 and Phase 1.75's B4 deviation for Phase 1.5. Specifically: Phase 2's Entry precondition should read "Phase 1.75 COMPLETE" (the first COMPLETE predecessor in the chain), and its spec-section list should note that §3.3 has been amended by Phase 1.75.

This adjustment is landed as a one-line edit to Phase 2's plan at the start of Phase 1.75 Task 5 (Closeout prep), not as a standalone step in this plan. Ledger entry: "Phase 2 B4 deviation appended @ <sha>: accept `port-rewrite-phase-1-75-complete` as predecessor tag."

---

## Self-review checklist (before Closeout)

- [ ] `rg 'Rc<RefCell<EngineScheduler>>' crates/` empty (I1 + I1c').
- [ ] `rg 'sub_scheduler: Rc<' crates/` empty (I1c'').
- [ ] `rg 'try_borrow(_mut)?\(\)' crates/emcore/src/emSubViewPanel.rs` empty (I1d').
- [ ] `rg -w 'register_pending_engines' crates/` empty (I-T3a).
- [ ] `! test -e crates/emcore/src/emPanelCtx.rs` (I-T3b).
- [ ] `rg 'pub mod emPanelCtx' crates/emcore/src/lib.rs` empty (I-T3b).
- [ ] `rg 'emPanelCtx::' crates/` empty (I-T3c).
- [ ] Spec §3.3 updated; §3.6 chartered category (d) added; §10 Phase-1 invariant bullet rewritten (I-Spec-3.3-amended).
- [ ] Popup-signal pre-allocation block gone from emView.rs (Task-10).
- [ ] All three `sp4_5_fix_1_timing_*.rs` fixtures assert `delta == 0` (Task-11).
- [ ] Goldens 237/6 (or better) preserved.
- [ ] Phase-1 carry-forward invariants (I1, I1a, I1b, I1d, I6) remain SAT.
- [ ] No new `#[allow(...)]` introduced outside CLAUDE.md's narrow whitelist.
- [ ] No `Rc<RefCell<PanelTree>>` introduced anywhere (rejection of Option A from the brainstorm).
- [ ] No nested `DoTimeSlice` call (rejection of Options B/C/D).
