# SP4.5-FIX-1 Follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Audit the four files in scope for re-entrant-borrow bugs matching the SP4.5-FIX-1 shape, fix any found, and characterize the post-fix timing concession against C++ baseline.

**Architecture:** Two parallel investigations bundled. Part A is grep-then-classify-then-fix-as-found, with each fix using the proven SP4.5-FIX-1 template (`try_borrow*` + post-slice catch-up sweep). Part B is test-fixture instrumentation in Rust + one-shot live instrumentation of `~/git/eaglemode-0.96.4/` for C++.

**Tech Stack:** Rust + cargo-nextest; bash for grep/audit; C++ for the C++ measurement (Eagle Mode 0.96.4).

**Spec:** [`docs/superpowers/specs/2026-04-19-sp4-5-fix-1-followups-design.md`](../specs/2026-04-19-sp4-5-fix-1-followups-design.md)

**Reference commit:** `85828c2` ("sp4.5-fix-1: defer panel-engine registration on re-entrant borrow") — the template for both the fix shape and the regression-test shape used in Part A.

---

## File Structure

**New files:**
- `docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md` — Part A audit table (file:line → verdict). Reused as evidence in the closeout-note update.
- `docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md` — Part B captured C++ deltas + the C++ diff applied to `~/git/eaglemode-0.96.4/` for reproduction.

**Modified files (Part A — only if vulnerabilities are found):**
- `crates/emcore/src/{emPanelTree,emPanelCtx,emSubViewPanel,emView}.rs` — fixes per vulnerable site, one commit each.

**Modified files (Part B Rust fixtures):**
- `crates/emcore/src/emPanelTree.rs` — Tasks 5 + 6 fixtures (top-level paths).
- `crates/emcore/src/emSubViewPanel.rs` — Task 7 fixture (sub-scheduler path).

**Modified files (closeout):**
- `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` — Task 9 updates §8.1 item 16's SP4.5-FIX-1 follow-up block with audit + measurement summary.

**Untouched:** `~/git/eaglemode-0.96.4/` is instrumented temporarily and reverted in Task 8; nothing committed to that tree.

---

## Part A — re-entrancy audit + fix-as-found

### Task 1: Enumerate borrow sites in scope

**Files:**
- Create: `docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md`

- [ ] **Step 1: Run the enumeration grep**

```bash
grep -nE '\.(try_)?borrow(_mut)?\(\)' \
  crates/emcore/src/emPanelTree.rs \
  crates/emcore/src/emPanelCtx.rs \
  crates/emcore/src/emSubViewPanel.rs \
  crates/emcore/src/emView.rs > /tmp/sp4_5_fix_1_raw_hits.txt
```

Expected: a list of every `borrow*()` / `try_borrow*()` call site across the four files, with `file:line:source` format.

- [ ] **Step 2: Filter to view/scheduler RefCell hits only**

For each line in `/tmp/sp4_5_fix_1_raw_hits.txt`, open the file at the line number and read 5 lines of context. Keep the entry only if the receiver of the `.borrow*()` call is one of:
- An `Rc<RefCell<emView>>` or `Weak<RefCell<emView>>::upgrade()` result.
- An `Rc<RefCell<EngineScheduler>>`.

Discard hits on `RefCell<emCoreConfig>`, `RefCell<emWindow>`, `RefCell<PanelTree>`, `RefCell<emEngineModel>`, or any other backing type. Other RefCells are explicitly out of scope per spec §2.2.

- [ ] **Step 3: Write the audit table skeleton**

Create `docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md` with this header and one row per surviving filtered hit:

```markdown
# SP4.5-FIX-1 Audit Table

**Date:** 2026-04-19
**Scope:** `crates/emcore/src/{emPanelTree,emPanelCtx,emSubViewPanel,emView}.rs`
**Filter:** `borrow*()` calls on `RefCell<emView>` or `RefCell<EngineScheduler>` only.
**Verdict legend:** `safe` / `vulnerable` / `needs-deeper-analysis`.

| File:line | RefCell type | Borrow kind | Caller class | Verdict | Evidence |
|---|---|---|---|---|---|
| emPanelTree.rs:573 | view | borrow | nested-from-Update | (TBD Task 2) | (TBD Task 2) |
| ... | ... | ... | ... | ... | ... |
```

Leave `Caller class`, `Verdict`, and `Evidence` columns blank (`(TBD Task 2)`); they are filled in Task 2.

- [ ] **Step 4: Commit the table skeleton**

```bash
git add docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md
git commit -m "sp4.5-fix-1: audit table skeleton (Part A Task 1)

Enumerate every borrow*() site in scope (emPanelTree, emPanelCtx,
emSubViewPanel, emView) targeting RefCell<emView> or
RefCell<EngineScheduler>. One row per surviving site; verdicts and
evidence are filled in Task 2.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Classify each site

**Files:**
- Modify: `docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md`

- [ ] **Step 1: For each row, build the callgraph chain**

For each row in the table, find every caller of the function containing the borrow call. Use:

```bash
grep -nrE '\b<function-name>\b' crates/emcore/src/ | grep -v '<file-of-definition>'
```

Trace upward until each chain reaches one of these terminals:
- `App::about_to_wait` (top-level scheduler driver entry)
- `dispatch_input` (winit `window_event` → input dispatch)
- `EngineScheduler::DoTimeSlice` direct call (test code)
- `emView::Update` / `emEngine::Cycle` (re-entrancy hazards)

The `Caller class` field is the union of which terminals the chains hit:
- `outermost` — only reachable from `App::about_to_wait` / `dispatch_input` / direct `DoTimeSlice` test calls, never from inside `Cycle` or `Update`.
- `nested-from-Update` — at least one chain passes through `emView::Update`.
- `nested-from-Cycle` — at least one chain passes through `emEngine::Cycle` (any engine).
- `nested-from-both` — chains exist for both nested cases.
- `unknown` — chain analysis incomplete; default to this when in doubt.

- [ ] **Step 2: Assign verdict per row**

Verdict rules:
- `Caller class = outermost` AND borrow kind ≠ a parent that holds borrow_mut → `safe`.
- `Caller class` includes `nested-from-Update` AND borrow kind on `RefCell<emView>` → `vulnerable`. (View is `borrow_mut`'d for Update's full duration.)
- `Caller class` includes `nested-from-Cycle` AND borrow kind on `RefCell<EngineScheduler>` → `vulnerable`. (Scheduler is `borrow_mut`'d for `DoTimeSlice`'s full duration.)
- `Caller class` includes `nested-from-Cycle` AND borrow kind on `RefCell<emView>` → `vulnerable` IFF the engine borrows the view (e.g., `UpdateEngineClass::Cycle` does at `emView.rs:255`). If unsure, `needs-deeper-analysis`.
- `Caller class = unknown` → `needs-deeper-analysis`.

Fill the `Verdict` column.

- [ ] **Step 3: Fill in evidence column**

For each row, write a one-sentence justification with the most-load-bearing callgraph chain (file:line at each hop). Example:

> Reachable from `App::about_to_wait` (`emGUIFramework.rs:455`) → `DoTimeSlice` → `<Engine>::Cycle` → `register_engine_for` → here. Scheduler held `borrow_mut` for the entire `DoTimeSlice` window.

- [ ] **Step 4: Commit the completed audit table**

```bash
git add docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md
git commit -m "sp4.5-fix-1: complete audit table with verdicts (Part A Task 2)

Each row classified safe / vulnerable / needs-deeper-analysis with an
explicit callgraph chain in the evidence column. Vulnerable rows
become Task 3 fix instances.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 5: Tally vulnerable count and report**

Count rows with `Verdict = vulnerable`. Emit one line:

> Audit complete. Vulnerable: N. Needs-deeper-analysis: M. Safe: K.

If `N + M = 0`: skip Task 3 entirely; jump to Task 4.

If `M > 0`: each `needs-deeper-analysis` row is escalated as its own follow-up item — file separately, do not block this plan on them.

If `N > 0`: proceed to Task 3 and run it once per vulnerable row.

---

### Task 3: Apply the SP4.5-FIX-1 template per vulnerable site

**RUN ONCE PER VULNERABLE ROW from Task 2's table.** This task is a recipe; the engineer instantiates it with the row's specifics each time.

**Files (placeholders — replace with actual values per row):**
- Modify: `<file-from-row>` — apply the fix.
- Modify: `crates/emcore/src/emPanelTree.rs` (or the appropriate `tests` module location) — add the regression test.

- [ ] **Step 1: Write the failing regression test**

Use the `sp4_5_create_child_with_view_already_borrow_mut_does_not_panic` test in `crates/emcore/src/emPanelTree.rs::tests` (added by commit `85828c2`) as the template. Adapt:
- Set up the *exact* contention state for the row's borrow kind. For `RefCell<emView>` → `let _view_borrow = view.borrow_mut();` before exercising the path. For `RefCell<EngineScheduler>` → register a custom engine that exercises the path inside its `Cycle` and drive `DoTimeSlice` from the test.
- Exercise the code path that reaches the row's `borrow*()` call.
- Assert no panic and (if applicable) that the deferred work completes after the catch-up trigger.

Test name convention: `sp4_5_fix_1_followup_<short_site_descriptor>_does_not_panic`.

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo nextest run -p emcore sp4_5_fix_1_followup_<short_site_descriptor>_does_not_panic 2>&1 | tail -30
```

Expected: `FAIL` with `RefCell already borrowed` or `RefCell already mutably borrowed` panic message. If the test passes here, the row was misclassified — return to Task 2 and downgrade to `safe`.

- [ ] **Step 3: Apply the fix at the row's site**

Two sub-cases:

**Case A: `RefCell<emView>` borrow.** Replace:
```rust
let view_borrow = view_rc.borrow();
// ... uses view_borrow ...
```
with:
```rust
let Ok(view_borrow) = view_rc.try_borrow() else {
    return; // Defer to post-slice catch-up sweep.
};
// ... uses view_borrow ...
drop(view_borrow); // explicit drop before any subsequent borrow_mut on the same RefCell
```

If the function's contract requires a value, return a `None` / sentinel that signals "deferred" to the caller. If no caller can tolerate that, downgrade the row to `needs-deeper-analysis` and escalate.

**Case B: `RefCell<EngineScheduler>` borrow_mut.** Replace:
```rust
let mut sched = sched_rc.borrow_mut();
sched.<some-mutation>(...);
```
with:
```rust
let Ok(mut sched) = sched_rc.try_borrow_mut() else {
    return; // Defer to post-slice catch-up sweep.
};
sched.<some-mutation>(...);
```

For both cases: confirm a catch-up trigger exists for the deferred work. The existing post-`DoTimeSlice` `tree.register_pending_engines()` sweep (in `App::about_to_wait` at `emGUIFramework.rs:~462` and `emSubViewPanel::Cycle` at `emSubViewPanel.rs:~329`) handles deferred engine registration. If the deferred work is something else, add an analogous sweep in the same two locations and document why.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo nextest run -p emcore sp4_5_fix_1_followup_<short_site_descriptor>_does_not_panic 2>&1 | tail -10
```

Expected: `PASS`.

- [ ] **Step 5: Run the full nextest suite to verify no regressions**

```bash
cargo nextest run 2>&1 | tail -5
```

Expected: all tests pass (count = previous count + 1 for the new regression test).

- [ ] **Step 6: Commit (one commit per vulnerable site)**

```bash
git add -A
git commit -m "sp4.5-fix-1-followup: defer <short-description> on re-entrant borrow

Audit row <file:line> from
docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md flagged
this site vulnerable: <RefCell-type> borrowed while a caller above
holds borrow_mut. Same pattern, same fix shape, same catch-up trigger
as commit 85828c2.

Regression test: sp4_5_fix_1_followup_<short_site_descriptor>_does_not_panic.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 7: Smoke test the binary**

```bash
cargo build --release --bin eaglemode 2>&1 | tail -3
RUST_BACKTRACE=1 timeout 20 ./target/release/eaglemode 2>&1 | grep -E "panic|Terminated" | head -5
```

Expected: only `Terminated` output (exit 143), no `panic` line. If a panic appears, a different vulnerable site triggered — log the new panic path, return to Task 2 to add the row, and re-run Task 3 for it.

---

## Part B — timing characterization

### Task 4: Build a measurement-instrumented PanelCycleEngine for tests

**Files:**
- Modify: `crates/emcore/src/emPanelCycleEngine.rs` (or wherever `PanelCycleEngine` lives — confirm with `grep -rn 'struct PanelCycleEngine' crates/emcore/src/`).

- [ ] **Step 1: Locate `PanelCycleEngine`**

```bash
grep -rn 'struct PanelCycleEngine' crates/emcore/src/
```

Note the file:line for the struct definition. Subsequent steps reference it as `<pce_file>`.

- [ ] **Step 2: Expose `time_slice_counter` on `EngineCtx`**

Read `crates/emcore/src/emEngine.rs:57-82` to confirm: `EngineCtx` holds `scheduler: &mut EngineCtxInner`, and `EngineCtxInner` has `pub time_slice_counter: u64`. Add an accessor on `EngineCtx`:

```rust
impl EngineCtx<'_> {
    /// Current scheduler time-slice counter. Used by SP4.5-FIX-1 timing
    /// fixtures to measure slices-between-create-and-first-Cycle.
    pub fn time_slice_counter(&self) -> u64 {
        self.scheduler.time_slice_counter
    }
}
```

- [ ] **Step 3: Add the first-cycle probe to `PanelCycleEngine`**

Edit `<pce_file>`:

```rust
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub(crate) struct PanelCycleEngineFirstCycleProbe {
    pub captured_slice: std::rc::Rc<std::cell::Cell<Option<u64>>>,
}

// On the existing PanelCycleEngine struct, add:
#[cfg(any(test, feature = "test-support"))]
pub(crate) first_cycle_probe: Option<PanelCycleEngineFirstCycleProbe>,
```

Initialize `first_cycle_probe: None` in every existing `PanelCycleEngine { ... }` constructor (grep for them: they're in `emPanelTree.rs::register_engine_for` and possibly tests).

In `impl emEngine::Cycle for PanelCycleEngine`, prepend:

```rust
#[cfg(any(test, feature = "test-support"))]
if let Some(probe) = &self.first_cycle_probe {
    if probe.captured_slice.get().is_none() {
        probe.captured_slice.set(Some(ctx.time_slice_counter()));
    }
}
```

- [ ] **Step 4: Add `EngineScheduler::attach_first_cycle_probe` test-support helper**

In `crates/emcore/src/emScheduler.rs`, add:

```rust
#[cfg(any(test, feature = "test-support"))]
impl EngineScheduler {
    /// Attach a first-cycle slice probe to a registered `PanelCycleEngine`.
    /// Used by SP4.5-FIX-1 timing fixtures (Tasks 5-7).
    pub fn attach_first_cycle_probe(
        &mut self,
        eid: super::emEngine::EngineId,
        captured_slice: std::rc::Rc<std::cell::Cell<Option<u64>>>,
    ) {
        let Some(eng) = self.inner.engines.get_mut(eid) else { return };
        let Some(behavior) = eng.behavior.as_mut() else { return };
        let Some(pce) = (behavior.as_mut() as &mut dyn std::any::Any)
            .downcast_mut::<crate::emPanelCycleEngine::PanelCycleEngine>()
        else {
            panic!("attach_first_cycle_probe: engine {eid:?} is not a PanelCycleEngine");
        };
        pce.first_cycle_probe = Some(crate::emPanelCycleEngine::PanelCycleEngineFirstCycleProbe {
            captured_slice,
        });
    }
}
```

(Note: `Box<dyn emEngine>` downcasting requires `emEngine: std::any::Any` — if the trait doesn't already require `Any`, add `: std::any::Any` to its supertrait list. Verify with `cargo check --tests -p emcore` after Step 5.)

- [ ] **Step 5: Verify it compiles under cfg(test)**

```bash
cargo check --tests -p emcore 2>&1 | tail -10
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "sp4.5-fix-1: add test-only first-cycle slice probe to PanelCycleEngine

Cfg-gated probe records the scheduler's time_slice_counter on a
PanelCycleEngine's first Cycle invocation. Used by Tasks 5-7 to
measure slices-between-create-and-first-Cycle for SP4.5-FIX-1's
deferred-registration timing concession.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Rust fixture — top-level StartupEngine path

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (`tests` module, alongside existing SP4.5 tests at `~3300`).

- [ ] **Step 1: Write the fixture**

Append after the Task 3 regression tests:

```rust
/// Part B Path 1 — top-level scheduler.
///
/// Mirrors `emMainWindow::StartupEngine::Cycle`'s production shape:
/// an engine that, on its first Cycle, calls `ctx.tree.create_child`
/// on a panel with a live View+scheduler. Records how many slices pass
/// between create_child returning and the spawned panel's
/// PanelCycleEngine::Cycle first running.
///
/// Baseline locked in below; if this assertion fires, SP4.5-FIX-1's
/// deferred-registration timing has shifted and Part B's measurement
/// table needs re-capture.
#[test]
fn sp4_5_fix_1_timing_top_level_startup_baseline_slices() {
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::rc::Rc;

    let (mut tree, _view, sched, root) = make_registered_tree();

    let create_slice: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
    let spawned_id: Rc<Cell<Option<PanelId>>> = Rc::new(Cell::new(None));

    struct StartupShapeEngine {
        parent: PanelId,
        spawned_out: Rc<Cell<Option<PanelId>>>,
        create_slice_out: Rc<Cell<Option<u64>>>,
        done: bool,
    }
    impl crate::emEngine::emEngine for StartupShapeEngine {
        fn Cycle(&mut self, ctx: &mut crate::emEngine::EngineCtx<'_>) -> bool {
            if !self.done {
                let child = ctx.tree.create_child(self.parent, "spawned");
                self.spawned_out.set(Some(child));
                self.create_slice_out.set(Some(ctx.time_slice_counter()));
                self.done = true;
            }
            false
        }
    }

    let eid = sched.borrow_mut().register_engine(
        crate::emEngine::Priority::Medium,
        Box::new(StartupShapeEngine {
            parent: root,
            spawned_out: spawned_id.clone(),
            create_slice_out: create_slice.clone(),
            done: false,
        }),
    );
    sched.borrow_mut().wake_up(eid);

    let mut empty_windows: HashMap<
        winit::window::WindowId,
        Rc<std::cell::RefCell<crate::emWindow::emWindow>>,
    > = HashMap::new();

    // Drive slices until the spawned panel's PanelCycleEngine has cycled.
    // Mirror the production catch-up sweep that lives in App::about_to_wait
    // (emGUIFramework.rs ~462).
    let mut slices = 0u64;
    let probe_captured: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));

    loop {
        sched.borrow_mut().DoTimeSlice(&mut tree, &mut empty_windows);
        tree.register_pending_engines();

        // After the first slice, attach the probe to the spawned panel's
        // PanelCycleEngine so we can capture its first cycle slice.
        if slices == 0 {
            let spawned = spawned_id.get().expect("StartupShapeEngine should have spawned");
            let pce_eid = tree.GetRec(spawned).and_then(|p| p.engine_id)
                .expect("spawned panel must be registered after catch-up");
            // Attach probe by reaching into the scheduler's engine slot for pce_eid
            // and inserting a PanelCycleEngineFirstCycleProbe with shared captured_slice.
            sched.borrow_mut().attach_first_cycle_probe(pce_eid, probe_captured.clone());
        }

        if probe_captured.get().is_some() { break; }
        slices += 1;
        assert!(slices < 10, "spawned panel should cycle within a few slices");
    }

    let create_at = create_slice.get().expect("create_slice must be captured");
    let cycled_at = probe_captured.get().expect("cycled_at must be captured");
    let delta = cycled_at - create_at;

    // Baseline: set on first measurement run; lock in here. If this fires,
    // see docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md.
    assert_eq!(delta, /* TASK 5 STEP 3 — replace with measured value */ 1,
        "SP4.5-FIX-1 top-level-startup slice delta drifted; re-run Part B measurement");

    // Cleanup.
    tree.remove(root);
    sched.borrow_mut().remove_engine(eid);
}
```

(`attach_first_cycle_probe` is a thin helper added to `EngineScheduler` for tests; if it doesn't exist, add it as part of this step. Keep it `#[cfg(any(test, feature = "test-support"))]`.)

- [ ] **Step 2: Run the test once with `delta` assertion set to a sentinel to capture the actual value**

Replace the assertion with:
```rust
panic!("MEASURED_DELTA={}", delta);
```

```bash
cargo nextest run -p emcore sp4_5_fix_1_timing_top_level_startup_baseline_slices 2>&1 | grep MEASURED_DELTA
```

Expected: a single line like `MEASURED_DELTA=1` (most likely 1, given the catch-up sweep runs at the end of the spawning slice and the next slice runs the new engine).

- [ ] **Step 3: Lock in the baseline**

Replace the `panic!` with `assert_eq!(delta, <captured value>, ...)`. Re-run; expect PASS.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "sp4.5-fix-1: lock baseline for top-level startup slice delta (Part B Path 1)

Test fixture mirrors StartupEngine::Cycle → create_child production
shape and captures the slice count between create_child and the
spawned panel's first PanelCycleEngine::Cycle. Baseline locked at
<value>; documented in
docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md
(written in Task 8).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Rust fixture — top-level mid-Update path

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (`tests` module).

- [ ] **Step 1: Write the fixture**

Same shape as Task 5 but with the contention being a held `view.borrow_mut()` (mirroring `emView::Update` running). The spawning happens via direct `tree.create_child` while the view borrow is held; capture the spawning slice as the scheduler's current `time_slice_counter` at the point of the call.

```rust
#[test]
fn sp4_5_fix_1_timing_top_level_mid_update_baseline_slices() {
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::rc::Rc;

    let (mut tree, view, sched, root) = make_registered_tree();
    let mut empty_windows: HashMap<
        winit::window::WindowId,
        Rc<std::cell::RefCell<crate::emWindow::emWindow>>,
    > = HashMap::new();

    // Capture the slice at which create_child runs, with the view
    // borrow_mut held (mirroring "inside emView::Update").
    let create_at = sched.borrow().GetTimeSliceCounter();
    let spawned = {
        let _view_borrow = view.borrow_mut();
        tree.create_child(root, "spawned")
    };
    tree.register_pending_engines();
    let pce_eid = tree.GetRec(spawned).and_then(|p| p.engine_id)
        .expect("spawned must be registered after catch-up");

    let probe_captured: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
    sched.borrow_mut().attach_first_cycle_probe(pce_eid, probe_captured.clone());
    sched.borrow_mut().wake_up(pce_eid);

    let mut slices = 0u64;
    while probe_captured.get().is_none() {
        sched.borrow_mut().DoTimeSlice(&mut tree, &mut empty_windows);
        tree.register_pending_engines();
        slices += 1;
        assert!(slices < 10, "spawned panel should cycle within a few slices");
    }
    let cycled_at = probe_captured.get().unwrap();
    let delta = cycled_at - create_at;
    assert_eq!(delta, /* TASK 6 STEP 2 — replace with measured value */ 1,
        "SP4.5-FIX-1 top-level-mid-update slice delta drifted; re-run measurement");

    tree.remove(root);
}
```

- [ ] **Step 2: Capture and lock the baseline (same procedure as Task 5 steps 2-3)**

`panic!("MEASURED_DELTA={}", delta);` → run → extract → replace assertion. Expect 1 (catch-up runs immediately after the borrow drops, well before the next DoTimeSlice).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "sp4.5-fix-1: lock baseline for top-level mid-Update slice delta (Part B Path 2)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Rust fixture — sub-scheduler path

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs` (`tests` module).

- [ ] **Step 1: Write the fixture**

Mirror Task 5 but inside an `emSubViewPanel`'s `sub_scheduler`. Drive one outer `PanelBehavior::Cycle` invocation per outer slice; the inner spawn happens inside an engine registered on `sub_scheduler`. Capture the inner-slice delta.

The exact setup depends on `emSubViewPanel`'s test scaffolding; if it's not already present, build the smallest standalone fixture (a `PanelTree` containing a single `emSubViewPanel`, the sub-tree with a "spawning" engine registered on the sub-scheduler, drive both schedulers).

Use `EngineScheduler::attach_first_cycle_probe` (added in Task 4 Step 4) against the sub-scheduler instance.

- [ ] **Step 2: Capture and lock the baseline (same procedure as Task 5)**

Expected delta: 1 (sub-tree catch-up sweep runs at the end of `emSubViewPanel::Cycle`'s sub-`DoTimeSlice` call at `emSubViewPanel.rs:~329`; spawned panel cycles on the next outer slice's inner `DoTimeSlice`).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "sp4.5-fix-1: lock baseline for sub-scheduler slice delta (Part B Path 3)

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: C++ measurement and notes-file capture

**Files:**
- Create: `docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md`
- Temporarily modify (not committed): files under `~/git/eaglemode-0.96.4/src/emCore/`.

- [ ] **Step 1: Locate the C++ probe insertion points**

```bash
grep -n 'emPanel::emPanel' ~/git/eaglemode-0.96.4/src/emCore/emPanel.cpp | head -3
grep -n 'emPanel::HandleCycle\|class emPanel' ~/git/eaglemode-0.96.4/include/emCore/emPanel.h | head -5
```

Identify:
- The `emPanel` constructor (most likely in `emPanel.cpp` near top).
- The first method called per panel each cycle (likely `HandleCycle` or one of the `Cycle*` overrides).
- The scheduler accessor for `GetTimeSliceCounter` on `emScheduler` (`include/emCore/emScheduler.h`).

- [ ] **Step 2: Apply the probes**

In `emPanel.cpp`:

```cpp
// In emPanel constructor body (after Scheduler is initialized):
if (Scheduler) {
    SP4_5_FIX_1_create_slice = Scheduler->GetTimeSliceCounter();
} else {
    SP4_5_FIX_1_create_slice = 0;
}
SP4_5_FIX_1_first_cycle_logged = false;
```

In `emPanel.h` (private members):
```cpp
emUInt64 SP4_5_FIX_1_create_slice;
bool SP4_5_FIX_1_first_cycle_logged;
```

In whichever method is the first per-cycle entry (verify with grep — likely `HandleCycle`):
```cpp
if (!SP4_5_FIX_1_first_cycle_logged && Scheduler) {
    fprintf(stderr,
        "SP4_5_FIX_1_PANEL_FIRST_CYCLE name=%s delta=%llu\n",
        (const char *)GetIdentity(),
        (unsigned long long)(Scheduler->GetTimeSliceCounter() - SP4_5_FIX_1_create_slice));
    SP4_5_FIX_1_first_cycle_logged = true;
}
```

(Adjust types to match emCore's typedefs; `emUInt64` and `GetIdentity()` are emCore conventions — confirm by reading the headers.)

- [ ] **Step 3: Build and run**

```bash
cd ~/git/eaglemode-0.96.4
perl make.pl build continue=yes 2>&1 | tail -20
timeout 15 ./bin/eaglemode 2>&1 | grep SP4_5_FIX_1 > /tmp/sp4_5_fix_1_cpp_capture.txt
```

Expected: `/tmp/sp4_5_fix_1_cpp_capture.txt` contains lines like `SP4_5_FIX_1_PANEL_FIRST_CYCLE name=... delta=N`.

- [ ] **Step 4: Extract deltas for the three measured paths**

Identify which captured panels correspond to:
- Path 1 (StartupEngine spawn) — likely `name=startupOverlay` (per `emMainWindow.cpp:~376` and Rust `emMainWindow.rs:470`).
- Path 2 (mid-Update create) — match against `update_children` callers (e.g., `emVirtualCosmosPanel`'s child names).
- Path 3 (sub-scheduler spawn) — match against panels created inside an `emSubViewPanel`'s sub-tree (e.g., `ctrl` from `emMainWindow.rs:520`).

Record the deltas.

- [ ] **Step 5: Write the measurements notes file**

```bash
cat > docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md <<'EOF'
# SP4.5-FIX-1 Timing Measurements

**Date:** 2026-04-19
**Context:** Spec §3, Task 8 of `docs/superpowers/plans/2026-04-19-sp4-5-fix-1-followups.md`.
**Eagle Mode version measured:** 0.96.4 at `~/git/eaglemode-0.96.4/`.

## Results

| Path | Panel name (C++) | Rust delta (slices) | C++ delta (slices) | Difference |
|---|---|---|---|---|
| 1 — top-level StartupEngine | <name> | <Task 5 baseline> | <Task 8 capture> | <diff> |
| 2 — top-level mid-Update | <name> | <Task 6 baseline> | <Task 8 capture> | <diff> |
| 3 — sub-scheduler | <name> | <Task 7 baseline> | <Task 8 capture> | <diff> |

## C++ instrumentation diff (one-shot, reverted)

```diff
<paste the full diff applied to ~/git/eaglemode-0.96.4/ here>
```

## Captured stderr

```
<paste the relevant grep'd lines from /tmp/sp4_5_fix_1_cpp_capture.txt>
```

## Decision

<one of:>
- "All three deltas equal Rust baseline → no observable drift; SP4.5-FIX-1 timing concession is a non-issue. Closed."
- "Delta on path <N> exceeds Rust by <X> slices → filed as follow-up SP4.5-FIX-2 (same-slice registration). Spec/plan to follow."
EOF
```

Fill in the placeholders with actual captured values.

- [ ] **Step 6: Revert C++ instrumentation**

```bash
cd ~/git/eaglemode-0.96.4
git diff > /tmp/sp4_5_fix_1_cpp_diff.patch  # save for the notes file's diff section
git checkout -- .
```

(If the C++ tree isn't a git repo, use `cp -a` of the touched files to `/tmp/` before editing in Step 2 and restore from there now.)

- [ ] **Step 7: Commit the measurements notes**

```bash
cd ~/git/eaglemode-rs
git add docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md
git commit -m "sp4.5-fix-1: capture C++ baseline timing measurements (Part B Task 8)

One-shot instrumentation of ~/git/eaglemode-0.96.4 captured slice
deltas for the three measured paths. Diff applied to the C++ tree is
reproduced in the notes file; tree was reverted after capture.

<Decision: closed as no-drift / filed as follow-up SP4.5-FIX-2>.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: Update closeout note

**Files:**
- Modify: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md`

- [ ] **Step 1: Locate the SP4.5-FIX-1 follow-up block**

```bash
grep -n 'Follow-up SP4.5-FIX-1' docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md
```

Note the line number. The block is currently a single paragraph in §8.1 item 16.

- [ ] **Step 2: Append a closeout summary**

Edit the file to append, immediately after the existing SP4.5-FIX-1 paragraph:

```markdown

**SP4.5-FIX-1 follow-ups closeout** (2026-04-19). Spec
[`docs/superpowers/specs/2026-04-19-sp4-5-fix-1-followups-design.md`];
plan [`docs/superpowers/plans/2026-04-19-sp4-5-fix-1-followups.md`].

- **Part A (audit):** N rows audited across emPanelTree, emPanelCtx,
  emSubViewPanel, emView. Vulnerable: <count>; needs-deeper-analysis:
  <count>; safe: <count>. Audit table at
  [`docs/superpowers/notes/2026-04-19-sp4-5-fix-1-audit-table.md`].
  <If vulnerable > 0:> Each vulnerable site landed its own commit and
  regression test using the SP4.5-FIX-1 template.
- **Part B (timing):** three baseline tests added (Tasks 5-7); C++
  measurement at
  [`docs/superpowers/notes/2026-04-19-sp4-5-fix-1-timing-measurements.md`].
  <Decision line from Task 8.>
- **Tests:** 2448 → <new total>. Golden 237/6 unchanged.
```

Fill in the placeholders with actual values from prior tasks.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md
git commit -m "sp4.5-fix-1: closeout — Part A + Part B summary in subsystem closeout note

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Final verification

- [ ] **Step 1: Full nextest pass**

```bash
cargo nextest run 2>&1 | tail -5
```

Expected: all green; count = 2448 + (Part A regression tests) + 3 (Part B fixtures).

- [ ] **Step 2: Clippy clean**

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

Expected: clean.

- [ ] **Step 3: Smoke test**

```bash
cargo build --release --bin eaglemode 2>&1 | tail -3
RUST_BACKTRACE=1 timeout 20 ./target/release/eaglemode 2>&1 | grep -E "panic|Terminated" | head -5
```

Expected: only `Terminated` (exit 143), no panic.

- [ ] **Step 4: Confirm no untracked files**

```bash
git status
```

Expected: clean working tree.
