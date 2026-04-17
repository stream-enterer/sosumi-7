# RawVisitAbs Rewrite — Follow-ups

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Context:** The RawVisitAbs rewrite landed in commits `430e7a7`, `96f78be`, `7e7b99e`, `28fad62`, `3770b00` (2026-04-17). Code review found five semantic-parity gaps that goldens don't catch. This plan closes them.

**Parent plan / spec:**
- `docs/superpowers/plans/2026-04-17-rawvisitabs-rewrite.md`
- `docs/superpowers/specs/2026-04-17-rawvisitabs-rewrite-design.md`

**C++ reference:** `~/git/eaglemode-0.96.4/src/emCore/emView.cpp`, `emPanel.cpp`.

**Ordering:** Task 1 (change-block side effects) and Task 3 (force_viewing_update triggers) are semantic-parity fixes and should land first. Task 2 (pt plumbing) is the most invasive and is gated behind a test-count recheck. Task 4 (surgical active-path) is a pure optimization. Task 5 (VISIBILITY vs VIEW_CHANGED) is a cross-cutting cleanup. Each task is one commit unless noted.

---

## Task 1 — Port missing change-block side effects (C++ emView.cpp:1803-1806)

**Problem:** The RawVisitAbs change block in `emView::Update` ends in C++ with:
```cpp
RestartInputRecursion = true;
CursorInvalid         = true;
UpdateEngine->WakeUp();
InvalidatePainting();
```
The Rust port omits all four. `cursor_invalid` is the only one with an obvious Rust analog already on `emView`. The omission means that when the SVP changes, the frame isn't marked dirty — the old painted pixels stay until some unrelated path invalidates. Goldens render from scratch per test and don't detect this.

**Files:**
- Modify: `crates/emcore/src/emView.rs` (change block at ~line 1432; add hook at end)

**Steps:**

- [ ] **Step 1: Audit the four side effects in Rust.**

  For each of the four C++ statements, find the Rust equivalent:
  - `CursorInvalid = true` → `self.cursor_invalid = true` (grep `cursor_invalid` in emView.rs to confirm field exists).
  - `InvalidatePainting()` → grep for `invalidate_painting\|InvalidatePainting` in emView.rs. If it exists, call it. If not, set whatever dirty flag the paint path consumes (likely a field like `painting_invalid` or `dirty`). If no such flag exists, note this as a BLOCKED sub-step and report — the whole-view invalidate mechanism may need to be added separately.
  - `RestartInputRecursion = true` → grep `restart_input_recursion\|RestartInputRecursion`. Likely does not yet exist in Rust (input recursion is C++ model-specific). If absent, document the omission with a `// NOT PORTED:` comment referencing emView.cpp:1803 and move on — it's acceptable if Rust's input path doesn't need this.
  - `UpdateEngine->WakeUp()` → grep for wake-up/schedule mechanism on the Rust event loop. `em-harness` or similar may expose it. If nothing equivalent exists, document and skip.

- [ ] **Step 2: Wire the surviving side effects into the change block.**

  At the end of the change block (after the new-SVP parent-chain walk-up finishes; just before the active-path propagation), add:
  ```rust
  self.cursor_invalid = true;
  // <invalidate-painting call here>
  // <wakeup call here if applicable>
  ```
  Guard each assignment with a comment citing the C++ line it ports.

- [ ] **Step 3: Verify.**

  ```
  cargo check -p emcore
  cargo test --release --test golden -- --test-threads=1 2>&1 | tail -5
  cargo test -p emcore --lib --release 2>&1 | tail -3
  cargo test --release --test pipeline --test behavioral 2>&1 | tail -5
  ```
  Baselines must not regress. If a pre-existing test now passes (e.g., a notice test that was firing because something else invalidated), note it in the commit message.

- [ ] **Step 4: Runtime smoke test.**

  Same script as parent plan Task 5 Step 4. Focus: cursor/paint refresh on zoom (press keys that change SVP and confirm the display doesn't show stale pixels). If you can't diff screenshots mechanically, at least confirm `ALIVE` and no core dump for 15s.

- [ ] **Step 5: Commit.**
  ```
  git commit -m "fix(emView): port missing RawVisitAbs change-block side effects"
  ```
  Message body: cite emView.cpp:1803-1806 lines, enumerate which of the four were ported and which omitted (with the reason why — e.g., "RestartInputRecursion not applicable to Rust input path").

---

## Task 2 — Plumb pixel_tallness correctly; remove pt=1.0 band-aid

**Problem:** `emView::Update` currently forces `tree.set_pixel_tallness(1.0)` at its top (emView.rs:~1181) as a DIVERGED band-aid. Root cause: `emView::new` auto-derives `pixel_tallness = height / width` (treating pt as a viewport aspect property), while C++ treats pt as a hardware property set via `SetViewGeometry(x, y, w, h, pt)`. Golden data was generated with C++ pt=1.0; the pre-rewrite `compute_viewed_recursive` ignored pt (implicit pt=1.0); the new port honors pt properly, exposing the Rust bug.

The band-aid leaves `self.pixel_tallness` (the emView field) and `tree.current_pixel_tallness` (forced to 1.0) permanently desynced. Per CLAUDE.md `feedback_rust_is_defective.md`, the right fix is to rewrite from C++, not preserve Rust's broken structure.

**Scope warning:** This task touches the construction path, viewport-geometry path, and anywhere that reads `self.pixel_tallness`. It is the most invasive follow-up. Test it with both golden baselines AND a runtime smoke test before landing.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — `new()`, `SetGeometry`, any `pixel_tallness = ...` writes, the top-of-Update band-aid.
- Possibly modify: `crates/emcore/tests/*.rs`, `crates/eaglemode/tests/golden/common.rs`, anywhere that constructs `emView` and relies on the auto-derived pt.

**Steps:**

- [ ] **Step 1: Inventory every writer and reader of `pixel_tallness`.**

  ```
  grep -n 'pixel_tallness' crates/emcore/src/emView.rs
  grep -rn 'pixel_tallness\|CurrentPixelTallness' crates/
  ```
  Record which call sites construct `emView` and what pt they implicitly rely on.

- [ ] **Step 2: Change `emView::new` and `SetGeometry` to default pt=1.0.**

  - In `new()`: initialize `pixel_tallness: 1.0` (not derived).
  - In `SetGeometry(x, y, w, h)`: do NOT assign `self.pixel_tallness = h / w`. Leave pt alone.
  - Add `pub fn SetViewPortTallness(&mut self, tallness: f64)` if not present. Grep confirms whether it exists; C++ has `SetViewPortTallness` at emView.cpp:375.

- [ ] **Step 3: Delete the band-aid at the top of `Update`.**

  Remove `tree.set_pixel_tallness(1.0)` at emView.rs:~1181 and its DIVERGED comment. Replace with `tree.set_pixel_tallness(self.pixel_tallness)` — the normal pt sync that was there pre-rewrite (line 1166).

- [ ] **Step 4: Fix any test fixtures that relied on auto-derived pt.**

  Run the full suite. Any test that expected pt = h/w must now explicitly call `SetViewPortTallness`. If many tests break, extract a `make_view(w, h, pt)` helper rather than scattering calls.

- [ ] **Step 5: Add a debug assertion of the invariant.**

  In `Update`, after the `set_pixel_tallness` call, add:
  ```rust
  debug_assert!((self.pixel_tallness - tree.current_pixel_tallness).abs() < 1e-12);
  ```
  This prevents future drift.

- [ ] **Step 6: Verify.**

  ```
  cargo check -p emcore
  cargo fmt
  cargo clippy -- -D warnings
  cargo test --release --test golden -- --test-threads=1 2>&1 | tail -10
  cargo test -p emcore --lib --release 2>&1 | tail -3
  cargo test --release --test pipeline --test behavioral 2>&1 | tail -5
  ```
  Baselines must not regress. Expected: 235 passed / 8 failed (same 8).

  If non-unit-pt tests regress that previously passed, the SVP-area formula at emView.rs:~1287 (`cvw * cvh <= MAX_SVP_SIZE`) may need to be reconciled with C++'s two separate checks (emView.cpp:1579, 1694: `w > MaxSVPSize || w*p->GetHeight() > MaxSVPSize`). Fix that in the same commit.

- [ ] **Step 7: Runtime smoke test.**

  Full parent-plan Task 5 Step 4 script. `ALIVE` + no core dump + screenshot is the bar.

- [ ] **Step 8: Commit.**
  ```
  git commit -m "fix(emView): plumb pixel_tallness correctly; remove pt=1.0 band-aid"
  ```
  Body: cite the DIVERGED comment's removal, note which callers were updated, and confirm the invariant assert.

---

## Task 3 — Audit and plumb `force_viewing_update` triggers

**Problem:** `force_viewing_update` is currently set in only two places: `emView::new` and `SetGeometry`. C++ sets `forceViewingUpdate = true` at multiple additional call sites (emView.cpp:777, 797, 1276 and the `RawVisit(..., true)` convention). Rust's `RawZoomOut` notably does not set the flag. If `RawZoomOut` is invoked without a prior `SetGeometry`, and the resulting SVP/rect change stays under the 0.001 thresholds, Update's no-op branch fires and the recomputation never happens.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — various callers of Update/Visit/Zoom.

**Steps:**

- [ ] **Step 1: Enumerate C++ forceViewingUpdate setters.**

  ```
  grep -n 'forceViewingUpdate\s*=\s*true\|RawVisit[A-Za-z]*(.*,\s*true)' ~/git/eaglemode-0.96.4/src/emCore/emView.cpp
  ```
  Record every site. Typical hits: `SetViewFlags`, `SetGeometry`, `SetViewPortTallness`, `RawZoomOut`, `InvalidateViewing`, some animation entry points.

- [ ] **Step 2: Find the Rust analog for each.**

  ```
  grep -n 'RawZoomOut\|SetViewFlags\|SetViewPortTallness\|InvalidateViewing' crates/emcore/src/emView.rs
  ```
  Build a C++ → Rust mapping table. For each Rust method, confirm whether it already assigns `self.force_viewing_update = true` before returning.

- [ ] **Step 3: Add missing assignments.**

  Each Rust method that mirrors a C++ `forceViewingUpdate = true` site gets `self.force_viewing_update = true;` before it returns (or before calling Update). Add a one-line `// C++ emView.cpp:LINE sets forceViewingUpdate` comment.

- [ ] **Step 4: Verify.**

  Standard baseline re-run. No regressions expected; this is additive.

  If a formerly-silent bug starts firing notices (e.g., a zoom test that previously had stale viewing state), golden tests may regress in a *good* way — investigate per-test before dismissing.

- [ ] **Step 5: Commit.**
  ```
  git commit -m "fix(emView): set force_viewing_update at all C++ forceViewingUpdate sites"
  ```

---

## Task 4 — Surgical active-path propagation

**Problem:** `emView::Update` clears `in_active_path` and `is_active` on every panel on every frame (emView.rs:~1306-1311), in both the no-op and change branches. This is O(tree) every tick and defeats the no-op branch's purpose. C++ mutates `ActivePath` only when `SetActivePanel` runs — it doesn't rebuild every frame.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — `Update`, maybe `SetActivePanel` (if active-path propagation moves there).

**Steps:**

- [ ] **Step 1: Add `prev_active: Option<PanelId>` to emView.**

  Initialize to `None` in `new()`. This tracks the active panel whose chain currently has `in_active_path=true`.

- [ ] **Step 2: Replace the full-tree clear with surgical walks.**

  In `Update`, replace the `for id in tree.all_ids()` clear + propagation with:
  ```rust
  if self.prev_active != self.active {
      // Clear old chain.
      if let Some(old) = self.prev_active {
          if tree.contains(old) {
              if let Some(p) = tree.get_mut(old) { p.is_active = false; }
              let mut cur = tree.GetRec(old).and_then(|p| p.parent);
              while let Some(id) = cur {
                  let parent = tree.get_mut(id).map(|p| {
                      p.in_active_path = false;
                      p.parent
                  });
                  cur = parent.unwrap_or(None);
              }
          }
      }
      // Set new chain (existing logic).
      if let Some(active_id) = self.active { … }
      self.prev_active = self.active;
  }
  ```

  Do this in BOTH the no-op and change branches (or, better, lift it above the branch split since it's independent of SVP change).

- [ ] **Step 3: Audit other places that mutate is_active / in_active_path.**

  ```
  grep -rn 'is_active\s*=\s*\|in_active_path\s*=\s*' crates/emcore/
  ```
  Make sure no other path silently changes these fields without updating `prev_active`. If `SetActivePanel` or similar does, either route them through a single helper or update `prev_active` there too.

- [ ] **Step 4: Verify.**

  Baselines re-run. The behavioral/notice tests are the ones most likely to detect regressions here — active-path changes interact with focus and input routing.

- [ ] **Step 5: Commit.**

---

## Task 5 — Reconcile VISIBILITY vs VIEW_CHANGED notice flags

**Problem:** Two different `NoticeFlags` bits appear to represent "viewing changed":
- `NoticeFlags::VISIBILITY` — used in the Layout eager-compute path at `emPanelTree.rs:~1192`, commented as "VISIBILITY = C++ NF_VIEWING_CHANGED".
- `NoticeFlags::VIEW_CHANGED` — used in the new `UpdateChildrenViewing` (emPanelTree.rs:~2396) and the RawVisitAbs port (emView.rs:~1352).

Consumers of one bit won't see notices fired with the other bit. This is a pre-existing inconsistency exposed by the rewrite firing on a different path than the Layout fast-path.

**Files:**
- Modify: `crates/emcore/src/emPanel.rs` (NoticeFlags definition).
- Modify: all call sites that queue either flag.
- Modify: all consumers (panel `notice()` handlers) that filter on either flag.

**Steps:**

- [ ] **Step 1: Map C++ flag names to Rust bits.**

  Read C++ `emPanel.h` for `NF_VIEWING_CHANGED`, `NF_VISIBILITY_POSSIBLY_CHANGED`, `NF_MEMORY_LIMIT_CHANGED`, `NF_UPDATE_PRIORITY_CHANGED`. Check whether C++ has both VISIBILITY and VIEWING_CHANGED as separate bits or one unified bit.

- [ ] **Step 2: Decide which Rust bit to keep.**

  If C++ has both: keep both, but make sure every mutation site fires the right one (and both if appropriate).
  If C++ has only one: delete the Rust duplicate, rename call sites to the surviving bit.

- [ ] **Step 3: Apply the consolidation.**

  ```
  grep -rn 'NoticeFlags::VISIBILITY\|NoticeFlags::VIEW_CHANGED' crates/
  ```
  Rewrite each site to match the decision from Step 2.

- [ ] **Step 4: Verify.**

  Standard baseline. Notice tests are the signal here; if any notice test's expected bitmask changes, decide whether the old expected was wrong (likely — the duplication meant they were reading half the story).

- [ ] **Step 5: Commit.**

---

## Verification checklist (all tasks)

After each task commit, the pre-commit hook (`.git/hooks/pre-commit`) runs `cargo fmt` + `cargo clippy -- -D warnings` + `cargo nextest run`. The nextest skip-list at `.config/nextest.toml` excludes 8 known-red goldens. No task is done until:

- [ ] Pre-commit hook passes without `--no-verify`.
- [ ] Golden: 235 passed / 8 failed (same 8).
- [ ] emcore lib: ≥ 819 passed, 0 failed.
- [ ] pipeline: 312 passed, 0 failed.
- [ ] behavioral: 378 passed, 0 failed.
- [ ] Runtime smoke: eaglemode ALIVE 15s, no core dump (run on Tasks 1, 2, 4 at minimum).

## Post-implementation

- [ ] Close the "concerns" section of the original plan by referencing the commits from this follow-up plan.
- [ ] If any of Tasks 1–5 uncover pre-existing bugs unrelated to the rewrite (e.g., non-unit-pt SVP formula in Task 2 Step 6), file as separate ticket rather than bundling.
