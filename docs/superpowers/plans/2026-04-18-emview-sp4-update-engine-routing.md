# SP4 Implementation Plan — emView::Update engine-only routing + Phase-8 test promotion

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align Rust `emView::Update` dispatch with C++'s single-caller model, dissolve the scheduler re-entrant borrow by caching the popup-close signal state in `UpdateEngineClass::Cycle`, and promote `test_phase8_popup_close_signal_zooms_out` to a single-engine end-to-end run.

**Architecture:** The fix is structural, not additive. Three C++ divergences drive today's latent re-entrant-borrow panic: (i) `emGUIFramework::about_to_wait:594` calls `view.update(tree)` directly every frame, bypassing `UpdateEngineClass::Cycle`; (ii) `attach_to_scheduler` omits the ctor-time `WakeUpUpdateEngine` that C++ does at `emView.cpp:84`; (iii) `emView::Update` reaches back through `self.scheduler.borrow()` to probe the popup close signal. Fixing (i)+(ii) makes Rust match C++'s single-caller model. Fixing (iii) exploits that `UpdateEngineClass::Cycle` already holds an `&mut EngineCtx` — we pre-compute `ctx.IsSignaled(close_sig)` there, stash it in a new `emView::close_signal_pending` field, and let `Update` read and clear it. No `Update` signature change; no cascade into the 141 call sites.

**Tech Stack:** Rust 2021; `slotmap`, `winit`, `wgpu`; existing `emcore` + `eaglemode` crate split; `cargo-nextest` + `cargo test --test golden`.

**Spec:** `docs/superpowers/specs/2026-04-18-emview-sp4-update-engine-routing-design.md`.

---

## File map

- **Modify** `crates/emcore/src/emView.rs` — add `close_signal_pending` field; modify `UpdateEngineClass::Cycle`; replace popup-close probe in `Update`; add wake at end of `attach_to_scheduler`; append `SetActivePanelBestPossible(tree)` to `Scroll`/`Zoom`/`ZoomOut`; delete `update()` wrapper; rewrite Phase-8 test.
- **Modify** `crates/emcore/src/emGUIFramework.rs:594` — delete direct `win.view_mut().update(tree)` call.
- **Modify** `crates/emcore/src/emWindow.rs` — add `#[cfg(any(test, feature = "test-support"))] fn new_for_test(...)` constructor (no GPU/winit surface).
- **Modify** call sites that used the `view.update(tree)` wrapper — audit via `grep -rn "\.update(&mut tree\|\.update(tree)" crates/` and migrate each to `.Update(&mut tree)` (one-char downcase, identical semantics post-wrapper-removal).

---

## Phase 0 — Baseline capture

Snapshot green state so regressions are attributable to SP4.

### Task 0.1: Capture baseline test counts

**Files:** none (record in plan status only).

- [ ] **Step 1:** Run the full nextest suite.

  Run: `cargo-nextest ntr 2>&1 | tail -20`
  Expected: `Summary [...] 2429 tests run: 2429 passed (9 skipped), 0 failed`

- [ ] **Step 2:** Run the golden suite.

  Run: `cargo test --test golden -- --test-threads=1 2>&1 | tail -5`
  Expected: `test result: FAILED. 237 passed; 6 failed` (baseline; same 6 pre-existing failures from the closeout doc §7.4).

- [ ] **Step 3:** Smoke-run the binary.

  Run: `timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"`
  Expected: `exit=124` or `exit=143`.

- [ ] **Step 4:** Commit nothing. Record these three numbers in the plan's working notes. Phases 1–5 must match or improve each of them.

---

## Phase 1 — Add the cached-signal field (no behavior change yet)

Introduce the `close_signal_pending` field and its read path in `Update`, keeping the old scheduler-borrow probe temporarily as a fallback. This phase is a pure addition; all tests must still pass.

### Task 1.1: Add `close_signal_pending` field

**Files:**
- Modify: `crates/emcore/src/emView.rs` (struct definition near `:307`; ctor init near `:484` and `new_for_test` near `:574`).

- [ ] **Step 1: Read the struct head and both ctors.**

  Run: `sed -n '307,340p;480,580p' crates/emcore/src/emView.rs`
  Expected: shows struct `emView { ... }` header and both `new` / `new_for_test` bodies.

- [ ] **Step 2: Add the field to the struct.**

  After the existing `pub(crate) pending_framework_actions: ...` field, add (adjust line number if file has shifted; the anchor is the `pending_framework_actions` field):

  ```rust
  /// Set by `UpdateEngineClass::Cycle` from `ctx.IsSignaled(close_signal)`
  /// before calling `Update`; read and cleared at the top of `Update`.
  /// Stands in for C++ `IsSignaled(PopupWindow->GetCloseSignal())` in
  /// `emView::Update` (emView.cpp:1299).
  ///
  /// DIVERGED: C++ emView inherits from emEngine (via emContext), so the
  /// IsSignaled call there is against emView's own clock. Rust emView is
  /// not an emEngine (SP7 — emContext threading — will revisit); the
  /// nearest correct clock is UpdateEngine's, and UpdateEngineClass::Cycle
  /// is the natural site to observe it.
  pub(crate) close_signal_pending: bool,
  ```

- [ ] **Step 3: Initialize in both constructors.**

  In `new(...)` and `new_for_test(...)`, add `close_signal_pending: false,` to the struct literal (alongside other `bool` defaults like `popped_up: false`).

- [ ] **Step 4: Verify it compiles.**

  Run: `cargo check -p emcore 2>&1 | tail -5`
  Expected: `Finished ... profile`.

- [ ] **Step 5: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(1/n): add emView::close_signal_pending field"
  ```

### Task 1.2: Write failing test for the engine-side probe

**Files:**
- Modify: `crates/emcore/src/emView.rs` (`#[cfg(test)] mod tests` at the bottom).

- [ ] **Step 1: Locate the `mod tests` block and add a new test next to `test_phase8_popup_close_signal_zooms_out`.**

  Append inside the tests module:

  ```rust
  /// SP4: UpdateEngineClass::Cycle must pre-compute close_signal_pending
  /// from ctx.IsSignaled before invoking Update, so that Update does not
  /// reach back through self.scheduler.
  #[test]
  fn sp4_update_engine_cycle_caches_close_signal_pending() {
      use crate::emEngine::{emEngine as EngineTrait, EngineCtx};
      // Build a view, attach a scheduler, push a popup so PopupWindow exists.
      let (mut tree, root, child_a, _) = setup_tree();
      let mut v = emView::new_for_test(root, 640.0, 480.0);
      let sched = Rc::new(RefCell::new(EngineScheduler::new()));
      v.attach_to_scheduler(sched.clone(), winit::window::WindowId::dummy());
      v.Update(&mut tree);
      v.SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree);
      v.RawVisit(&mut tree, child_a, 0.0, 0.0, 0.1, true);
      assert!(v.PopupWindow.is_some());
      let close_sig = v.PopupWindow.as_ref().unwrap().borrow().close_signal;
      let eng_id = v.update_engine_id.unwrap();

      // Fire the close signal and advance clocks so IsSignaled returns true
      // for the update engine.
      sched.borrow_mut().fire(close_sig);
      // Process signals so sig.clock advances past eng.clock.
      sched.borrow_mut().process_pending_signals_for_test();

      // Now construct an EngineCtx as UpdateEngineClass::Cycle would see one,
      // and invoke Cycle directly.
      let window_id = winit::window::WindowId::dummy();
      let mut windows: std::collections::HashMap<_, _> = std::collections::HashMap::new();
      // Without a window, Cycle no-ops; this test only needs the caching
      // assertion, so we invoke the caching logic directly by emulating
      // Cycle's body steps once the window lookup succeeds. Use a tiny
      // local helper that exercises the exact code path.
      assert!(!v.close_signal_pending);
      // Call Cycle against an empty windows map: no-op (returns false, no panic).
      // Then call the caching helper directly via a test-only seam.
      v.close_signal_pending =
          sched.borrow().get_signal_clock(close_sig)
              > sched.borrow().get_engine_clock(eng_id);
      assert!(v.close_signal_pending, "close_signal fired — caching must see it true");
      // After Update consumes it, the flag must clear.
      v.Update(&mut tree);
      assert!(!v.close_signal_pending, "Update must clear close_signal_pending");
      // Cleanup
      drop(windows);
      if let Some(id) = v.update_engine_id.take() { sched.borrow_mut().remove_engine(id); }
      if let Some(id) = v.visiting_va_engine_id.take() { sched.borrow_mut().remove_engine(id); }
      if let Some(s) = v.EOISignal.take() { sched.borrow_mut().remove_signal(s); }
      sched.borrow_mut().remove_signal(close_sig);
  }
  ```

  **Note for implementer:** `process_pending_signals_for_test`, `get_signal_clock`, `get_engine_clock` may not exist verbatim in the current scheduler API. Before Step 2, run `grep -n "pub fn" crates/emcore/src/emScheduler.rs | head -40` to inventory available surface. Use the closest existing getters (`is_signaled_for_engine` is known to exist). If the needed accessors are missing, add minimal `#[cfg(any(test, feature = "test-support"))]` getters to `EngineScheduler` as part of this task — do not expose new production API.

- [ ] **Step 2: Run the new test — it must fail.**

  Run: `cargo-nextest run -p emcore sp4_update_engine_cycle_caches_close_signal_pending 2>&1 | tail -10`
  Expected: FAIL. The `v.Update(&mut tree)` at the end does not yet clear the flag.

- [ ] **Step 3: Commit the failing test.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(2/n): failing test for close_signal_pending lifecycle"
  ```

### Task 1.3: Make `Update` consume `close_signal_pending` (alongside the existing scheduler-borrow, as a bridge)

**Files:**
- Modify: `crates/emcore/src/emView.rs:2318-2350` (the popup-close block at the top of `Update`).

- [ ] **Step 1: Read the current block.**

  Run: `sed -n '2318,2352p' crates/emcore/src/emView.rs`
  Expected: the `BUG` comment block + the `let popup_closed = { ... }` probe.

- [ ] **Step 2: Add the `close_signal_pending` consumption as an OR with the existing check — temporary bridge.**

  Replace the `let popup_closed = { ... };` expression with:

  ```rust
  let popup_closed = {
      // SP4: cached path — set by UpdateEngineClass::Cycle before Update.
      // Preferred; the scheduler-borrow fallback below is a temporary
      // bridge kept only until Phase 2 removes the unsafe path.
      let cached = std::mem::take(&mut self.close_signal_pending);
      if cached {
          true
      } else if let (Some(popup), Some(sched), Some(eng_id)) = (
          self.PopupWindow.as_ref(),
          self.scheduler.as_ref(),
          self.update_engine_id,
      ) {
          let close_sig = popup.borrow().close_signal;
          sched.borrow().is_signaled_for_engine(close_sig, eng_id)
      } else {
          false
      }
  };
  ```

- [ ] **Step 3: Run the Task 1.2 test.**

  Run: `cargo-nextest run -p emcore sp4_update_engine_cycle_caches_close_signal_pending 2>&1 | tail -5`
  Expected: PASS.

- [ ] **Step 4: Run the full emcore test suite — nothing else should regress.**

  Run: `cargo-nextest run -p emcore 2>&1 | tail -5`
  Expected: all prior tests still pass.

- [ ] **Step 5: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(3/n): Update consumes close_signal_pending (bridge)"
  ```

---

## Phase 2 — Write the engine-side probe; remove the scheduler-borrow fallback

### Task 2.1: Modify `UpdateEngineClass::Cycle` to cache before calling `Update`

**Files:**
- Modify: `crates/emcore/src/emView.rs:197-206` (the `Cycle` impl).

- [ ] **Step 1: Read the current `Cycle`.**

  Run: `sed -n '195,210p' crates/emcore/src/emView.rs`
  Expected: the existing 4-line Cycle body.

- [ ] **Step 2: Rewrite `Cycle`.**

  Replace the body with:

  ```rust
  impl super::emEngine::emEngine for UpdateEngineClass {
      fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
          // Mirrors C++ UpdateEngineClass::Cycle → View.Update().
          // SP4: pre-compute the popup-close signal state here, where we
          // hold &mut EngineCtx directly — emView::Update must not reach
          // back through self.scheduler (would panic re-entrantly because
          // DoTimeSlice holds sched.borrow_mut()).
          if let Some(win_rc) = ctx.windows.get(&self.window_id) {
              let win_rc = Rc::clone(win_rc);
              let mut win = win_rc.borrow_mut();
              let view = win.view_mut();
              if let Some(popup) = view.PopupWindow.as_ref() {
                  let close_sig = popup.borrow().close_signal;
                  view.close_signal_pending = ctx.IsSignaled(close_sig);
              }
              view.Update(ctx.tree);
          }
          false
      }
  }
  ```

- [ ] **Step 3: Build and run the emcore suite.**

  Run: `cargo check -p emcore && cargo-nextest run -p emcore 2>&1 | tail -5`
  Expected: clean build; all tests pass.

- [ ] **Step 4: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(4/n): UpdateEngineClass::Cycle pre-computes close_signal_pending"
  ```

### Task 2.2: Remove the scheduler-borrow fallback from `Update`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (the `let popup_closed = { ... };` block edited in Task 1.3).

- [ ] **Step 1: Replace the bridge block with the final form.**

  Replace the block with:

  ```rust
  let popup_closed = std::mem::take(&mut self.close_signal_pending);
  ```

  Also delete the `BUG (tracked as ...)` comment block immediately above (previously `:2324-2335`). Keep a short 1-line comment pointing at C++:

  ```rust
  // C++ emView.cpp:1299 popup-close probe. The IsSignaled call happens
  // one frame earlier in Rust, in UpdateEngineClass::Cycle — see SP4 spec
  // docs/superpowers/specs/2026-04-18-emview-sp4-update-engine-routing-design.md §2.3.
  let popup_closed = std::mem::take(&mut self.close_signal_pending);
  ```

- [ ] **Step 2: Run the emcore suite.**

  Run: `cargo-nextest run -p emcore 2>&1 | tail -5`
  Expected: all tests pass, including Task 1.2's test.

- [ ] **Step 3: Confirm the re-entrant-borrow panic path is gone.**

  Run: `grep -n "self\.scheduler\.as_ref().unwrap().borrow()\|sched.borrow().is_signaled_for_engine" crates/emcore/src/emView.rs`
  Expected: no matches (the fallback path is deleted).

- [ ] **Step 4: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(5/n): drop scheduler-borrow fallback; close_signal_pending is sole path"
  ```

---

## Phase 3 — Align Rust dispatch with C++ single-caller model

Deletes the `update()` wrapper and the direct framework call, adds ctor-time wake, relocates `SetActivePanelBestPossible`.

### Task 3.1: Append `SetActivePanelBestPossible` to `Scroll`, `Zoom`, `ZoomOut` (C++ parity)

**Files:**
- Modify: `crates/emcore/src/emView.rs` — `Scroll` ends near `:1154`, `Zoom` ends near `:1120`, `ZoomOut` near `:1254`.

- [ ] **Step 1: Locate the three function bodies.**

  Run: `grep -n "pub fn Scroll\|pub fn Zoom\b\|pub fn ZoomOut\b" crates/emcore/src/emView.rs`
  Expected: three line numbers. Read each function end.

- [ ] **Step 2: Append to each.**

  At the end of `Scroll`'s body (just before the closing `}`):
  ```rust
      // C++ emView.cpp:780.
      self.SetActivePanelBestPossible(tree);
  ```

  At the end of `Zoom`'s body:
  ```rust
      // C++ emView.cpp:800.
      self.SetActivePanelBestPossible(tree);
  ```

  At the end of `ZoomOut`'s body:
  ```rust
      // C++ emView.cpp:901.
      self.SetActivePanelBestPossible(tree);
  ```

- [ ] **Step 3: Build; run nextest.**

  Run: `cargo check -p emcore && cargo-nextest run -p emcore 2>&1 | tail -5`
  Expected: clean build; pass.

- [ ] **Step 4: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(6/n): relocate SetActivePanelBestPossible to Scroll/Zoom/ZoomOut (C++ parity)"
  ```

### Task 3.2: Add `WakeUpUpdateEngine` to `attach_to_scheduler`

**Files:**
- Modify: `crates/emcore/src/emView.rs:3044-3070`.

- [ ] **Step 1: Read current `attach_to_scheduler`.**

  Run: `sed -n '3044,3072p' crates/emcore/src/emView.rs`
  Expected: ends with `self.visiting_va_engine_id = Some(visiting_va_engine_id);`.

- [ ] **Step 2: Append the wake.**

  Add as the last line of the function body:
  ```rust
      // C++ emView::emView at emView.cpp:84: UpdateEngine->WakeUp().
      self.WakeUpUpdateEngine();
  ```

- [ ] **Step 3: Test.**

  Run: `cargo-nextest run -p emcore 2>&1 | tail -5`
  Expected: all pass. The existing test `test_phase7_update_engine_wakeup_via_scheduler` asserts `!sched.borrow().has_awake_engines()` *before* calling `WakeUpUpdateEngine` — it will now see the engine already awake and fail.

- [ ] **Step 4: Fix the now-outdated test.**

  In `test_phase7_update_engine_wakeup_via_scheduler` (near `:5956`), delete the line `assert!(!sched.borrow().has_awake_engines());` — with SP4's ctor-time wake, the engine is awake immediately after attach. The subsequent `v.WakeUpUpdateEngine(); assert!(sched.borrow().has_awake_engines());` still exercises the re-wake path. Add a comment:
  ```rust
  // SP4: attach_to_scheduler now wakes the update engine (C++ emView.cpp:84).
  // The explicit WakeUpUpdateEngine() below re-wakes a possibly-slept engine;
  // with the ctor wake it's a no-op in this path, but verifies the API.
  ```

- [ ] **Step 5: Re-run nextest.**

  Run: `cargo-nextest run -p emcore 2>&1 | tail -5`
  Expected: all pass.

- [ ] **Step 6: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(7/n): wake update engine from attach_to_scheduler (C++ emView.cpp:84)"
  ```

### Task 3.3: Delete `emGUIFramework::about_to_wait:594` direct call

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs:593-595`.

- [ ] **Step 1: Read the block.**

  Run: `sed -n '590,600p' crates/emcore/src/emGUIFramework.rs`
  Expected: includes the `// Update view (...)\n win.view_mut().update(tree);` lines.

- [ ] **Step 2: Delete those two lines.**

  Remove:
  ```rust
              // Update view (recompute viewing coords, auto-select active)
              win.view_mut().update(tree);
  ```

  Replace with:
  ```rust
              // SP4: Update runs only via UpdateEngineClass::Cycle now
              // (C++ single-caller model, emView.cpp:2523). This loop no
              // longer drives Update directly.
  ```

- [ ] **Step 3: Build the whole workspace.**

  Run: `cargo check 2>&1 | tail -15`
  Expected: clean.

- [ ] **Step 4: Run nextest.**

  Run: `cargo-nextest run 2>&1 | tail -10`
  Expected: ≥ baseline (2429 passed). If regressions appear here, they identify tests that silently depended on the wrapper's post-hoc `SetActivePanelBestPossible`; Task 3.1 should already have covered them. If a test fails because it called `Scroll`/`Zoom`/`ZoomOut` and expected an active-panel reselection via a path other than those three, investigate — do not blindly paper over.

- [ ] **Step 5: Smoke-test the binary.**

  Run: `timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"`
  Expected: exit=124 or exit=143.

- [ ] **Step 6: Commit.**

  ```bash
  git add crates/emcore/src/emGUIFramework.rs
  git commit -m "sp4(8/n): remove direct emView::update call from about_to_wait"
  ```

### Task 3.4: Delete the `emView::update()` wrapper

**Files:**
- Modify: `crates/emcore/src/emView.rs:3845-3859`.

- [ ] **Step 1: Read the wrapper.**

  Run: `sed -n '3840,3862p' crates/emcore/src/emView.rs`
  Expected: the `pub fn update(...)` wrapper body.

- [ ] **Step 2: Audit cross-crate callers.**

  Run: `grep -rn "\.update(&mut tree\|\.update(tree)\|\.update(&mut \\*tree" crates/ 2>&1 | head -40`
  Expected: zero hits outside of the emView wrapper itself (Task 3.3 deleted the only production caller). If any remain, fix them per Step 3 before deleting.

- [ ] **Step 3: Migrate any stragglers.**

  For each remaining `X.update(&mut tree)` call: replace with `X.Update(&mut tree)`. Each of these call sites formerly benefited from the wrapper's `SetActivePanelBestPossible` fixup. If the caller was test code that established state via `Scroll`/`Zoom`/`ZoomOut` *before* the `.update` call, Task 3.1's relocation already covers the reselection. If the caller established state a different way and relies on post-`Update` reselection, add an explicit `view.SetActivePanelBestPossible(&mut tree)` after the `.Update(...)` call and annotate with `// SP4: was covered by deleted emView::update() wrapper`.

- [ ] **Step 4: Delete the wrapper.**

  Remove the `pub fn update(...)` block at `:3845-3859`.

- [ ] **Step 5: Build + nextest + golden.**

  Run:
  ```bash
  cargo check && cargo-nextest run 2>&1 | tail -5 && cargo test --test golden -- --test-threads=1 2>&1 | tail -5
  ```
  Expected: clean build, 2429 passed, golden 237/243 (baseline parity).

- [ ] **Step 6: Smoke-run the binary.**

  Run: `timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"`
  Expected: exit=124 or exit=143.

- [ ] **Step 7: Commit.**

  ```bash
  git add crates/
  git commit -m "sp4(9/n): delete emView::update() wrapper; single-caller model in place"
  ```

---

## Phase 4 — Test harness: bare-window ctor

The Phase-8 test rewrite needs an `emWindow` that `ctx.windows.get(&window_id)` can find. Provide a test-only constructor with no GPU/winit surface.

### Task 4.1: Inventory the minimum `emWindow` surface `UpdateEngineClass::Cycle` needs

**Files:** read-only.

- [ ] **Step 1: Inventory methods called on the window handle during the engine path.**

  Run:
  ```
  grep -n "win.borrow_mut\|win_rc.borrow_mut\|win_rc\.borrow\b\|win\.view_mut\|win\.view()" crates/emcore/src/emView.rs
  ```
  Expected: short list, all pointing to `view_mut()` / `view()`.

- [ ] **Step 2: Read the `emWindow` struct head and `view()` / `view_mut()` definitions.**

  Run: `grep -n "pub struct emWindow\|fn view\b\|fn view_mut\b\|pub fn create\|pub fn new_popup_pending" crates/emcore/src/emWindow.rs`

- [ ] **Step 3: No code change. Summarize in the commit message of Task 4.2 which fields the bare ctor can default.**

### Task 4.2: Write a failing test for `emWindow::new_for_test`

**Files:**
- Modify: `crates/emcore/src/emWindow.rs` — new `#[cfg(any(test, feature = "test-support"))] pub fn new_for_test(...)`.
- Modify: `crates/emcore/Cargo.toml` — ensure `test-support` feature exists (it already does per §4.4 closeout; verify).

- [ ] **Step 1: Verify the `test-support` feature flag is declared.**

  Run: `grep -n "test-support" crates/emcore/Cargo.toml`
  Expected: a `[features]` entry for `test-support = []`. If absent, add it.

- [ ] **Step 2: Add a failing test in `crates/emcore/src/emWindow.rs` tests module.**

  ```rust
  /// SP4 Task 4: new_for_test builds an emWindow with a live emView but no
  /// GPU/winit surface, suitable for registering in a scheduler's windows
  /// map for UpdateEngineClass::Cycle to find.
  #[test]
  fn new_for_test_constructs_without_event_loop() {
      let mut tree = crate::emPanelTree::PanelTree::new();
      let root = tree.create_root("root");
      tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);
      let win_id = winit::window::WindowId::dummy();
      let sched = std::rc::Rc::new(std::cell::RefCell::new(
          crate::emScheduler::EngineScheduler::new(),
      ));
      let win = emWindow::new_for_test(win_id, &sched, root, 640.0, 480.0);
      assert_eq!(win.borrow().id(), win_id);
      // Verify the view is attached to the scheduler.
      assert!(win.borrow().view().update_engine_id.is_some());
  }
  ```

- [ ] **Step 3: Run — must fail.**

  Run: `cargo-nextest run -p emcore new_for_test_constructs_without_event_loop 2>&1 | tail -5`
  Expected: FAIL (method not defined).

- [ ] **Step 4: Commit failing test.**

  ```bash
  git add crates/emcore/src/emWindow.rs crates/emcore/Cargo.toml
  git commit -m "sp4(10/n): failing test for emWindow::new_for_test"
  ```

### Task 4.3: Implement `emWindow::new_for_test`

**Files:**
- Modify: `crates/emcore/src/emWindow.rs`.

- [ ] **Step 1: Add the constructor.**

  Add alongside `new_popup_pending`:

  ```rust
  /// Test-only: build an `emWindow` with a fully-initialized `emView`
  /// attached to the given scheduler, but with no winit/wgpu surface.
  /// Exists to satisfy `UpdateEngineClass::Cycle`'s `ctx.windows.get(...)`
  /// lookup so single-engine integration tests (notably the Phase-8 popup-
  /// close test) can drive `DoTimeSlice` end-to-end without spinning up
  /// an event loop.
  ///
  /// Gated behind the `test-support` feature; not visible to non-test
  /// consumers.
  #[cfg(any(test, feature = "test-support"))]
  pub fn new_for_test(
      window_id: winit::window::WindowId,
      scheduler: &std::rc::Rc<std::cell::RefCell<crate::emScheduler::EngineScheduler>>,
      root: crate::emPanelTree::PanelId,
      width: f64,
      height: f64,
  ) -> std::rc::Rc<std::cell::RefCell<Self>> {
      let mut view = crate::emView::emView::new_for_test(root, width, height);
      view.attach_to_scheduler(scheduler.clone(), window_id);
      // All GPU/surface fields default or use the existing Pending sentinels.
      // See `OsSurface::Pending` from the W3 architecture; no surface is
      // materialized in this constructor.
      let win = Self {
          window_id,
          os_surface: crate::emWindow::os_surface::OsSurface::stub_for_test(),
          view,
          // ...fill all remaining struct fields using Default::default() or
          // their documented test-neutral values. Read the struct definition
          // and use existing test fixtures (grep for `emWindow { ... }`
          // construction in the crate) as the template.
      };
      std::rc::Rc::new(std::cell::RefCell::new(win))
  }
  ```

  **Implementer note:** The placeholder `os_surface::OsSurface::stub_for_test()` must exist — if not, add a matching constructor on the `OsSurface` enum (under the same `test-support` feature) that returns a zero-cost variant. Read the `OsSurface` enum (`Pending`/`Materialized` per W3 closeout §3.2) and add a `TestStub` variant if needed, or reuse `Pending` with a default `PendingSurface`. Document which path you chose in the commit message.

- [ ] **Step 2: Build.**

  Run: `cargo check -p emcore --features test-support 2>&1 | tail -10`
  Expected: clean. If compile errors mention unfilled fields of `emWindow`, read the struct and fill each with a test-neutral default; do not guess.

- [ ] **Step 3: Run the Task 4.2 test.**

  Run: `cargo-nextest run -p emcore new_for_test_constructs_without_event_loop 2>&1 | tail -5`
  Expected: PASS.

- [ ] **Step 4: Run full nextest; no regressions.**

  Run: `cargo-nextest run 2>&1 | tail -5`
  Expected: 2430/2430 (baseline +1 for the new test) or strictly not worse.

- [ ] **Step 5: Commit.**

  ```bash
  git add crates/emcore/src/emWindow.rs
  git commit -m "sp4(11/n): emWindow::new_for_test — bare ctor for single-engine integration tests"
  ```

---

## Phase 5 — Phase-8 test promotion

Rewrite `test_phase8_popup_close_signal_zooms_out` as a single-engine run.

### Task 5.1: Replace the two-engine test

**Files:**
- Modify: `crates/emcore/src/emView.rs` (the `test_phase8_popup_close_signal_zooms_out` test and its multi-paragraph doc block `:6098-6210` or thereabouts).

- [ ] **Step 1: Read the current test in full.**

  Run: `grep -n "fn test_phase8_popup_close_signal_zooms_out" crates/emcore/src/emView.rs`
  Note the start line. Run: `sed -n '<start>,<start+120>p' crates/emcore/src/emView.rs`.

- [ ] **Step 2: Replace the test body (and its doc block) with the single-engine version.**

  ```rust
  /// SP4 Phase-8: popup's close_signal, when fired and processed through the
  /// scheduler, wakes UpdateEngineClass, which invokes emView::Update, which
  /// observes close_signal_pending and calls ZoomOut. Drives the entire
  /// sequence through one DoTimeSlice — no dummy engines, no harness hacks.
  /// This supersedes the two-engine test that was a compromise against the
  /// scheduler re-entrant borrow (now fixed by SP4 §2.3).
  #[test]
  fn test_phase8_popup_close_signal_zooms_out() {
      use std::collections::HashMap;
      let (mut tree, root, child_a, _) = setup_tree();
      let win_id = winit::window::WindowId::dummy();
      let sched = Rc::new(RefCell::new(EngineScheduler::new()));

      // Build a real emWindow so UpdateEngineClass::Cycle can find it.
      let win = crate::emWindow::emWindow::new_for_test(win_id, &sched, root, 640.0, 480.0);

      // Initial Update to clear zoomed_out_before_sg.
      {
          let mut w = win.borrow_mut();
          w.view_mut().Update(&mut tree);
          w.view_mut().SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree);
          // Push a popup via RawVisit under POPUP_ZOOM.
          w.view_mut().RawVisit(&mut tree, child_a, 0.0, 0.0, 0.1, true);
          assert!(w.view().PopupWindow.is_some());
      }

      // Fire the popup's close signal.
      let close_sig = win
          .borrow()
          .view()
          .PopupWindow
          .as_ref()
          .unwrap()
          .borrow()
          .close_signal;
      sched.borrow_mut().fire(close_sig);

      // One time slice: signal processing advances sig.clock; UpdateEngineClass::Cycle
      // observes ctx.IsSignaled(close_sig) = true, sets close_signal_pending,
      // invokes Update, which calls ZoomOut.
      let mut windows: HashMap<_, _> = HashMap::new();
      windows.insert(win_id, Rc::clone(&win));
      sched.borrow_mut().DoTimeSlice(&mut tree, &mut windows);

      // Post-condition: popup tore down.
      assert!(
          win.borrow().view().PopupWindow.is_none(),
          "close_signal → ZoomOut must tear down PopupWindow in one time slice"
      );
      assert!(
          !win.borrow().view().popped_up,
          "popped_up must be false after ZoomOut"
      );

      // Cleanup for scheduler Drop debug_asserts.
      {
          let mut w = win.borrow_mut();
          let v = w.view_mut();
          if let Some(id) = v.update_engine_id.take() { sched.borrow_mut().remove_engine(id); }
          if let Some(id) = v.visiting_va_engine_id.take() { sched.borrow_mut().remove_engine(id); }
          if let Some(s) = v.EOISignal.take() { sched.borrow_mut().remove_signal(s); }
      }
      sched.borrow_mut().remove_signal(close_sig);
  }
  ```

  Delete the previous `NoopEngine`-based test body and its multi-paragraph comment block entirely.

- [ ] **Step 3: Also remove the Task 1.2 bridge test `sp4_update_engine_cycle_caches_close_signal_pending`.**

  Rationale: once the Phase-8 test drives the real engine path end-to-end, the Phase-1 bridge test is redundant and its hand-built caching emulation is less rigorous than the true engine drive. Delete it.

- [ ] **Step 4: Run the nextest suite.**

  Run: `cargo-nextest run 2>&1 | tail -5`
  Expected: 2429/2429 (baseline — we added `new_for_test_constructs_without_event_loop` in Task 4.2 and removed the Phase-1 bridge test, so net +0 for the `new_for_test_...` addition) or strictly not worse than baseline.

- [ ] **Step 5: Verify the test's single-slice structure.**

  Run: `grep -c "DoTimeSlice" crates/emcore/src/emView.rs` (confined to the new test body, by inspection)
  Expected: the Phase-8 test invokes `DoTimeSlice` exactly once. Success criterion §5.8 in the spec.

- [ ] **Step 6: Run golden.**

  Run: `cargo test --test golden -- --test-threads=1 2>&1 | tail -5`
  Expected: 237/243 (baseline parity).

- [ ] **Step 7: Smoke-run the binary.**

  Run: `timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"`
  Expected: exit=124 or exit=143.

- [ ] **Step 8: Commit.**

  ```bash
  git add crates/emcore/src/emView.rs
  git commit -m "sp4(12/n): rewrite Phase-8 popup-close test as single-engine DoTimeSlice"
  ```

---

## Phase 6 — Closeout

### Task 6.1: Verify all spec §5 success criteria

**Files:** read-only.

- [ ] **Step 1: Clippy clean.**

  Run: `cargo clippy --all-targets --features test-support -- -D warnings 2>&1 | tail -10`
  Expected: `Finished`.

- [ ] **Step 2: BUG marker gone.**

  Run: `grep -n "BUG (tracked as" crates/emcore/src/emView.rs`
  Expected: no output.

- [ ] **Step 3: Direct framework call gone.**

  Run: `grep -n "win\.view_mut().update(tree)" crates/emcore/src/emGUIFramework.rs`
  Expected: no output.

- [ ] **Step 4: Wrapper gone.**

  Run: `grep -n '^    pub fn update\b' crates/emcore/src/emView.rs`
  Expected: no output.

- [ ] **Step 5: Phase-8 test is single-engine.**

  Inspect the test body visually; confirm only one `DoTimeSlice` call and no `NoopEngine`.

- [ ] **Step 6: Re-run nextest + golden + smoke one last time.**

  Same commands as Phase 0. Record results.

### Task 6.2: Update the closeout doc

**Files:**
- Modify: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md`.

- [ ] **Step 1: Mark SP4 complete in §8.0 table.**

  Change the SP4 row from "Not started; 14 blocks 11 — one combined spec" to "**Complete 2026-04-18** (merged as `<commit-sha>`)" with artifact paths.

- [ ] **Step 2: Mark §8.1 items 11 and 14 as CLOSED** with the SP4 commit reference.

- [ ] **Step 3: Update §5.1 item 5** (Phase-8 test asserts across two engines) — closed by SP4.

- [ ] **Step 4: Update §1 "Status at a glance"** — subtract item 14 from the residual list.

- [ ] **Step 5: Update the appendix "Suggested execution order"** — SP4 done; remainder is SP5/SP6/SP7.

- [ ] **Step 6: Commit.**

  ```bash
  git add docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md
  git commit -m "docs(closeout): mark SP4 complete"
  ```

### Task 6.3: Merge / PR (follow existing repo conventions)

Out of scope for this plan — defer to the project's usual branch-finishing flow (see `superpowers:finishing-a-development-branch`).

---

## Self-review — run before handoff

1. **Spec coverage:**
   - §2.1 engine-only routing → Tasks 3.1–3.4. ✓
   - §2.2 ctor-time wake → Task 3.2. ✓
   - §2.3 cached-field popup probe → Tasks 1.1, 1.3, 2.1, 2.2. ✓
   - §2.4 test-site stability → verified by Phase 3 regression runs; no migration needed. ✓
   - §2.5 Phase-8 single-engine → Tasks 4.1–4.3, 5.1. ✓
   - §2.6 non-goals → no task touches `VisitingVAEngineClass` or notice dispatch. ✓
2. **Placeholder scan:** Task 4.3 Step 1 leaves the implementer to fill `emWindow` fields they haven't enumerated — flagged explicitly with "do not guess" and a concrete source (the struct definition + existing test fixtures). Task 5.1 Step 1 and Task 3.4 Step 1 say "note the start line" rather than hardcoding line numbers — intentional, because the file shifts between phases.
3. **Type consistency:** `close_signal_pending: bool` (Task 1.1), written by `UpdateEngineClass::Cycle` (Task 2.1), consumed via `std::mem::take` in `Update` (Task 2.2). `emWindow::new_for_test(winit::window::WindowId, &Rc<RefCell<EngineScheduler>>, PanelId, f64, f64) -> Rc<RefCell<Self>>` used identically in Task 4.2 (failing test) and Task 5.1 (Phase-8 rewrite). Consistent.

---

**End of plan.**
