# SP4.5-FIX-1 follow-ups — design

**Date:** 2026-04-19
**Triggered by:** commit `85828c2` ("sp4.5-fix-1: defer panel-engine registration on re-entrant borrow")
**Status:** spec — awaiting plan
**Scope:** two independent investigations bundled into one spec; either part may ship without the other.

---

## 1. Charter

Commit `85828c2` fixed two latent re-entrant-borrow panics in `PanelTree::register_engine_for` (one on the scheduler `RefCell`, one on the view `RefCell`) by switching to `try_borrow*` and adding a post-`DoTimeSlice` `register_pending_engines()` catch-up sweep. The fix mirrored SP4's `SchedOp` defer pattern; the deregister path (`deregister_engine_for`) had used the same shape since SP4.5.

Two follow-ups remain after that commit:

- **Part A — re-entrancy audit.** SP4.5-FIX-1 fixed only the two sites that panicked production. The same shape (`borrow*()` on view or scheduler `RefCell`, reachable from inside an engine `Cycle` or `emView::Update`) may recur elsewhere; nothing was audited.
- **Part B — timing characterization.** The fix defers panel-engine registration to the next `about_to_wait` tick, so a panel created mid-`Cycle` doesn't get its own `PanelCycleEngine::Cycle` for ~one slice. C++ doesn't have this delay (panels and engines share an `emContext`, no `RefCell`). It is unmeasured whether this is observable.

This spec covers both. Either part can ship independently of the other.

---

## 2. Part A — re-entrancy audit + fix-as-found

### 2.1 Goal

Enumerate every `borrow*()` call against an `Rc<RefCell<emView>>` or `Rc<RefCell<EngineScheduler>>` in the audit scope, classify each, fix every site that matches the SP4.5-FIX-1 vulnerability shape, and commit a regression test per fix.

### 2.2 Scope

Audit boundary: `crates/emcore/src/{emPanelTree,emPanelCtx,emSubViewPanel,emView}.rs`.

Justification for boundary:
- `emPanelTree.rs` — origin file of the bug; most likely to host siblings of the same shape.
- `emPanelCtx.rs` — the way `PanelBehavior` reaches its view from inside `Cycle`; structurally identical risk surface.
- `emSubViewPanel.rs` — SP8's per-sub-view scheduler is structurally identical to the top-level scheduler the bug hit; same pattern can recur.
- `emView.rs` — the SP4 `SchedOp` drain path runs while the view is `borrow_mut`'d; any callee that re-enters the view borrow has the same risk.

Out of scope: other crates; other `RefCell` types (only the view and scheduler RefCells matter — they are the ones that the engine driver and `emView::Update` hold `borrow_mut` on).

### 2.3 Methodology

1. **Enumerate.** Single grep pass:
   ```bash
   grep -nE '\.(try_)?borrow(_mut)?\(\)' \
     crates/emcore/src/{emPanelTree,emPanelCtx,emSubViewPanel,emView}.rs
   ```
   Filter to RefCells whose backing type is `emView` or `EngineScheduler`. (Other RefCells — `emCoreConfig`, `Window`, etc. — are out of scope.)

2. **Classify.** For each surviving hit, fill in a row of the audit table with:
   - **File:line**
   - **RefCell type** — `view` or `scheduler`.
   - **Borrow kind** — `borrow` or `borrow_mut`.
   - **Caller class** — `outermost` (called only from `App::about_to_wait` / `dispatch_input` / scheduler driver entrypoints), `nested-from-Update`, `nested-from-Cycle`, `nested-from-both`, `unknown`.
   - **Verdict** — `safe` / `vulnerable` / `needs-deeper-analysis`.
   - **Evidence** — one-sentence justification with an explicit callgraph chain produced from grep, e.g. `App::about_to_wait → DoTimeSlice → SomeEngine::Cycle → X → here`.

3. **Empirical verification for vulnerable verdicts.** For each site classified `vulnerable`, write a regression test in the SP4.5-FIX-1 template (set up the contention state, exercise the path, expect no panic). The test must fail before the fix and pass after. If the test cannot be made to fail, downgrade to `needs-deeper-analysis` and capture why.

4. **Fix.** For each `vulnerable` site, apply the SP4.5-FIX-1 template:
   - Replace `borrow*()` with `try_borrow*()`.
   - On `Err`, return silently (deferring the work).
   - Ensure the deferred work has a catch-up trigger (existing `register_pending_engines` sweep, or a new equivalent if the work is not registration).

5. **Escalate.** If the SP4.5-FIX-1 template doesn't fit (e.g., the site needs the borrow's value synchronously — no defer possible), do not improvise. File a separate follow-up item, mark the audit row `needs-deeper-analysis`, and continue.

### 2.4 Deliverables

- Audit table committed in the implementation plan and reproduced in the closeout note.
- One regression test + one fix commit per `vulnerable` site (commit-per-site, not lumped).
- Closeout entry in `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.1 item 16's SP4.5-FIX-1 follow-up block, summarising what was found and what was fixed.

### 2.5 Exit criteria

- Every `borrow*()` site in scope has a verdict.
- Every `vulnerable` verdict has either landed a fix-with-test or escalated to its own follow-up.
- `cargo nextest run` is green; `cargo clippy --all-targets -- -D warnings` is clean.
- Smoke: `timeout 20 ./target/release/eaglemode` exits 143 (SIGTERM) with no panic in stderr.

---

## 3. Part B — timing characterization

### 3.1 Goal

Measure, for representative production paths, the number of scheduler slices between `create_child` returning and the spawned panel's `PanelCycleEngine::Cycle` first running. Compare Rust (post-SP4.5-FIX-1) against C++. Decide whether the difference is observable.

### 3.2 Scope

Three paths to measure:

1. **Top-level `StartupEngine`.** `App::about_to_wait → DoTimeSlice → StartupEngine::Cycle → ctx.tree.create_child(...)`. Spawn child belongs to top-level `tree` driven by the top-level `scheduler`.
2. **Top-level mid-`Update`.** `winit window_event → dispatch_input → emView::Update → emVirtualCosmosPanel::update_children → tree.create_child(...)`. Same top-level scheduler; different contention (view `borrow_mut` instead of scheduler `borrow_mut`).
3. **Sub-scheduler analog.** Same shape as path 1 but inside an `emSubViewPanel`'s `sub_scheduler`, driven from the outer scheduler's `PanelBehavior::Cycle`.

Out of scope: synthetic worst-case fixtures; multi-window timing; signal-fire propagation timing.

### 3.3 Methodology

**Rust side — test-harness probes.**

Two test fixtures (one for path 1, one for path 3; path 2 may share a fixture with path 1 with a different contention setup):

- Setup: build a `PanelTree` + `emView` + `EngineScheduler` mirror of the production caller (an engine that calls `create_child` once on its first `Cycle`, exactly as `StartupEngine` does for its overlay child).
- Instrument `PanelCycleEngine::Cycle` with a one-shot counter that captures `EngineCtx::scheduler.time_slice_counter` on its first invocation. Capture occurs only inside the test fixture, gated to `#[cfg(test)]` to keep production paths cold.
- Snapshot the scheduler's `time_slice_counter` immediately after `create_child` returns from inside the spawning engine's `Cycle`.
- Drive `DoTimeSlice` until the spawned panel's `Cycle` has fired once. The delta is the slice count.

Each fixture asserts the measured delta against a constant set by the first measurement run. The constant is the documented baseline; future regressions surface as test failures.

Fixtures live in `crates/emcore/src/emPanelTree.rs::tests` (paths 1 and 2) and `crates/emcore/src/emSubViewPanel.rs::tests` (path 3). Naming: `sp4_5_fix_1_timing_<path>_baseline_slices`.

**C++ side — one-shot live instrumentation.**

C++ source tree at `~/git/eaglemode-0.96.4/`. Add temporary probes:

- In `emPanel::emPanel(...)` (constructor): record `Scheduler.GetTimeSliceCounter()` into a per-panel field.
- In the first cycle entry for each panel (at `emPanel::HandleCycle` or the equivalent): on first invocation only, `fprintf(stderr, "PANEL_FIRST_CYCLE %p delta=%u\n", this, slice_now - slice_at_construct);`.

Build, run `eaglemode`, capture stderr for the first ~10 seconds of startup, extract the deltas for the panels created on the same callgraph chains as paths 1, 2, and 3. Paste the captured numbers into this spec as a one-time measurement table.

Revert the C++ instrumentation after capture. Do not commit changes to `~/git/eaglemode-0.96.4/`.

### 3.4 Deliverables

- Three Rust test fixtures committed, each asserting its baseline slice count.
- A measurement table appended to this spec capturing:
  - For each path: Rust delta, C++ delta, difference.
  - The exact diff applied to the C++ source for the C++ measurement (so it can be re-applied later).
  - The eaglemode version under test (0.96.4).
  - The capture date.

### 3.5 Exit criteria

If `Rust delta == C++ delta` on every path → close as "no observable drift; SP4.5-FIX-1 timing concession is a non-issue".

If `Rust delta > C++ delta` on any path → file the affected path(s) as a new follow-up item ("SP4.5-FIX-1 same-slice registration") with the measurement as evidence. Do not design the fix in this spec.

---

## 4. Risks

- **Part A — false-safe verdicts.** Classifying a site `safe` because no current caller exercises it from inside Cycle/Update, when a future caller will. Mitigation: every `safe` verdict requires an explicit callgraph chain produced by grep, not memory; when the chain is unclear, classify `needs-deeper-analysis`, not `safe`.
- **Part A — fix doesn't fit template.** A vulnerable site may need the borrow's value synchronously (no defer possible). Mitigation: per the charter, escalate as its own follow-up rather than improvise an alternative fix shape inside this spec's scope.
- **Part B — C++ measurement non-reproducible.** The C++ instrumentation is one-shot and reverted. Mitigation: spec captures the exact diff + the captured stderr + the eaglemode version, so a future engineer can re-apply and re-run.
- **Part B — fixtures don't reflect production.** Toy fixtures might miss timing characteristics of real startup (signal interleaving, multiple engines waking simultaneously). Mitigation: each fixture's setup explicitly mirrors the production caller; fixture comments cite the production callgraph chain by file:line.

---

## 5. Anti-patterns to avoid

- Reading code to *guess* whether a site is vulnerable. Per `feedback_instrument_not_read`: when in doubt, write a regression test that would panic if the site were vulnerable, and run it. Empirical > speculative.
- Re-reading C++ to *infer* slice timing instead of measuring. Per `feedback_cpp_is_ground_truth`: instrument and run the C++ binary; do not deduce.
- Lumping multiple suspect sites into one fix commit. Repo discipline: one fix = one commit + one regression test.

---

## 6. Execution order

1. **Part A audit pass.** Build the full classification table and commit it (in the implementation plan, then in the closeout note) before any fix is applied. This guards against the failure mode where a half-finished audit gets abandoned and only a subset of fixes ships.
2. **Part A fix-as-found.** One commit per vulnerable site, in any order. Each commit lands its regression test alongside the fix.
3. **Part B Rust fixtures.** Land idempotent test fixtures with the baseline constant captured in the test source.
4. **Part B C++ measurement.** One-shot instrumentation, capture, paste numbers into this spec, revert C++ tree.
5. **Part B closeout decision.** Either close (no drift) or file follow-up (drift found, with measurement as evidence).

Part A is sequenced before Part B because Part A's audit may surface additional vulnerable paths that change Part B's measurement targets. Part B's *methodology* is locked by this spec; the *targets* may grow if Part A finds something.

---

## 7. Non-goals

- No design in this spec for an alternative same-slice registration mechanism. If Part B finds drift, the redesign is a follow-up item, not a body section here.
- No `unsafe`/raw-pointer escape hatches considered for vulnerable sites in Part A. Either the SP4.5-FIX-1 template fits, or escalate.
- No expansion of the audit scope beyond the four files in §2.2.
- No multi-window or multi-monitor timing analysis in Part B.
