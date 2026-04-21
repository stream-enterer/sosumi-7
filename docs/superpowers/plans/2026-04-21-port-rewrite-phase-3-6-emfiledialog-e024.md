# Phase 3.6 — `emFileDialog` rides 3.5 infrastructure + E024 closes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `emFileDialog`'s current caller-invoked `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool` method with scheduler-driven dispatch by composing the Phase 3.5 `emDialog` (which owns its emWindow + DlgPanel + DialogPrivateEngine) and subscribing the dialog's `DialogPrivateEngine` to `fsb.file_trigger_signal` + the transient `overwrite_dialog.finish_signal`. E024 (raw-material divergence entry: "emFileDialog OverwriteDialog: caller-pulled Cycle") closes as a consequence.

**Architecture:** `emFileDialog` owns an `emDialog` (Rust composition for C++ inheritance) + an `emFileSelectionBox` installed as a child of the dialog's content panel. File-dialog-specific Cycle logic is injected into the shared `DialogPrivateEngine::Cycle` via a `DlgPanel.on_cycle_ext: Option<Box<dyn FnMut(...) -> bool>>` callback-slot added in this phase. The overwrite-confirmation sub-dialog (a transient `emDialog` spawned in `CheckFinish`) subscribes its `finish_signal` to the outer emFileDialog's `DialogPrivateEngine`, and its lifecycle is driven by the same Cycle. **No caller invokes any `Cycle` method anywhere.** Single engine type across base dialog and file dialog (per brainstorm D2).

**Tech Stack:** Rust 1.82+. All changes in `crates/emcore/src/emDialog.rs` (DlgPanel extension) and `crates/emcore/src/emFileDialog.rs`. No new external crates.

**Authority:** CLAUDE.md Port Ideology. Brainstorm decisions D1–D7 at `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow-plan.md`. Phase 3.5 plan at `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow.md` — **MUST be merged and tagged `port-rewrite-phase-3-5-complete` before this phase starts.**

**Branch:** `port-rewrite/phase-3-6-emfiledialog-e024` branching from `main` at the Phase 3.5 merge commit. Exit tag: `port-rewrite-phase-3-6-complete`.

**Baseline (at Phase 3.5 exit):**
- nextest: 2482 passed (approximate — Phase 3.5's net +6 tests over Phase 3 closeout's 2476; subject to minor drift from Phase 3.5.A if it lands).
- goldens: 237 passed / 6 failed (pre-existing).
- clippy: clean.
- E024: open, `phase_3_progress` unchanged from Phase 3.

**Gate commands** (end of each committed task):
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo-nextest ntr`
- `cargo test --test golden -- --test-threads=1` (phase closeout only)

---

## File structure

**Files created:** none (File and Name Correspondence).

**Files modified:**
- `crates/emcore/src/emFileDialog.rs` — the keystone. Reshape `emFileDialog` from a plain owned struct with caller-invoked `Cycle` to a composition over the 3.5-ported `emDialog` + inner `emFileSelectionBox` installed as content. DELETE `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool`, its `DIVERGED:` block, the `fsb_file_trigger_signal` cached field, and the `test_force_overwrite_result` helper.
- `crates/emcore/src/emDialog.rs` — add `DlgPanel.on_cycle_ext: Option<Box<dyn FnMut(...)>>` extension slot; extend `DialogPrivateEngine::Cycle` to call the extension after the base cycle completes.
- `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` — flip `E024.status` to `resolved-phase-3-6`, populate `resolution_commit` with Task 5's SHA, clear `phase_3_progress`.
- `docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md` — new. Phase ledger.
- `docs/superpowers/notes/2026-04-23-phase-3-6-closeout.md` — new. Phase closeout note.

---

## Bootstrap decisions

- **B3.6a (entry gate):** Phase 3.5 (and 3.5.A if required) must be merged to main with `port-rewrite-phase-3-5-complete` tagged AND all 3.5 invariants I5a–I5g+I5i+I5j green BEFORE this phase starts. Verify at Task 1.
- **B3.6b (engine composition decision):** File-dialog cycle logic extends the shared `DialogPrivateEngine` via a callback slot on `DlgPanel`, NOT a new engine type. Matches D2: single engine, per-behavior Cycle specialisation. C++ parity: `emFileDialog::Cycle()` calls `emDialog::Cycle()` first (base) then runs file-dialog logic — the Rust shape is `DialogPrivateEngine::Cycle` runs base logic then invokes `on_cycle_ext` if Some.
- **B3.6c (overwrite dialog as a second top-level dialog):** The overwrite-confirmation dialog is a transient `emDialog` (using the 3.5 ctor) — spawned in `CheckFinish`, torn down via `deregister` + drop when its finish is observed. It is a SEPARATE top-level window with its own DlgPanel + DialogPrivateEngine. The outer emFileDialog subscribes its engine to `overwrite_dialog.finish_signal` via `scheduler.connect` at spawn time. Requires Phase 3.5.A's multi-top-level support (assumed present).
- **B3.6d (test harness addition):** extend `DialogTestHarness` from 3.5 to `FileDialogTestHarness` that additionally arranges: a temp-dir fsb parent directory, pre-populated test files for overwrite scenarios, and a `do_n_slices(n)` helper.

---

## Prerequisite: post-show dialog mutation infrastructure (MUST land before Task 2)

**Context:** Phase 3.5 (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-deferred-dialog-construction.md`) shipped a narrowly-scoped `emDialog::finish_post_show(ctx, result)` as a one-off post-show mutation path — `emFileDialog::Cycle`'s live `Finish(Ok)` calls forced that minimal helper into 3.5 against the spec's original intent. The spec at §Deferred to Phase 3.6 calls for a general `App::mutate_dialog_by_id(did, f)` + `DialogMutation` enum (or equivalent closure-based routing) that covers the other mutator surface (`SetRootTitle`, `set_button_label_for_result`, `AddCustomButton`, `EnableAutoDeletion`, `Finish`).

Phase 3.6's engine-migration work (Task 2 onward) assumes post-show mutators exist — e.g. mode-change-induced title updates on a visible file dialog. **Land the general infrastructure FIRST**, then replace `finish_post_show` with a wrapper over the general path, then proceed to engine migration.

### Prereq Task A: Add `App::mutate_dialog_by_id` + closure-based mutation

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs`
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Per Phase 3.5 spec §Deferred post-show dialog mutation. Add:
```rust
impl App {
    pub fn mutate_dialog_by_id(
        &mut self,
        did: DialogId,
        f: impl FnOnce(&mut crate::emDialog::DlgPanel),
    ) {
        if let Some(&wid) = self.dialog_windows.get(&did) {
            if let Some(win) = self.windows.get_mut(&wid) {
                let mut tree = win.take_tree();
                // Root panel id is stable per-dialog; lookup via a convention:
                // each dialog's DlgPanel lives at the tree's single root.
                if let Some(root_id) = tree.root_id() {
                    if let Some(mut b) = tree.take_behavior(root_id) {
                        if let Some(dlg) = b.as_dlg_panel_mut() {
                            f(dlg);
                        }
                        tree.put_behavior(root_id, b);
                    }
                }
                win.put_tree(tree);
                // Wake DialogPrivateEngine (exactly one engine at Toplevel(wid)).
                for eid in self
                    .scheduler
                    .engines_for_scope(crate::emPanelScope::PanelScope::Toplevel(wid))
                {
                    self.scheduler.wake_up(eid);
                }
            }
        }
    }
}
```

### Prereq Task B: Route post-show `emDialog` mutators through `pending_actions`

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Extend each pre-show-only mutator to handle the post-show case via `ctx.pending_actions().borrow_mut().push(...)` calling `app.mutate_dialog_by_id(did, |p| ...)`. Pattern:

```rust
impl emDialog {
    pub fn SetRootTitle<C: ConstructCtx>(&mut self, ctx: &mut C, title: &str) {
        if self.pending.is_some() {
            self.with_dlg_panel_mut("SetRootTitle", |p| p.SetTitle(title));
        } else {
            let did = self.dialog_id;
            let title = title.to_string();
            ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
                app.mutate_dialog_by_id(did, |p| p.SetTitle(&title));
            }));
        }
    }
}
```

Apply to: `SetRootTitle`, `set_button_label_for_result`, `EnableAutoDeletion`. `AddCustomButton` is trickier because adding a button post-show requires tree mutation beyond `DlgPanel` (child panel creation); defer until a live caller exists. `set_on_finish` / `set_on_check_finish` should stay pre-show-only (callback registration post-finish is a latent bug).

**API break:** `SetRootTitle` / `EnableAutoDeletion` / `set_button_label_for_result` gain a `ctx` parameter. Update callers (Phase 3.5 Tasks 8 / 18 / 19 may have sites without ctx — they're all pre-show, so `ctx` is always in scope). Grep every call-site and thread ctx.

### Prereq Task C: Retire `emDialog::finish_post_show` in favor of the general path

Replace the body of `finish_post_show` with a one-liner over the general path:

```rust
pub fn finish_post_show<C: ConstructCtx>(&self, ctx: &mut C, result: DialogResult) {
    let did = self.dialog_id;
    ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
        app.mutate_dialog_by_id(did, move |p| {
            if p.pending_result.is_none() && p.finalized_result.is_none() {
                p.pending_result = Some(result);
            }
        });
    }));
}
```

(The scheduler wake-up moves inside `mutate_dialog_by_id`; callers no longer do it themselves.)

### Prereq gate

Run Phase 3.5's full emDialog + emStocksListBox + emFileDialog test suites. Expectation: all green, no new failures. The `emFileDialog::Cycle` path through `finish_post_show` now routes via `mutate_dialog_by_id` — semantically identical. Commit each Prereq task separately for reviewer cadence; do not bundle.

**Only after Prereq Tasks A/B/C land does Task 1 begin.**

---

## Task 1: Entry-gate verification + ledger setup

**Files:**
- Create: `docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md`

- [ ] **Step 1.1: Verify Phase 3.5 entry state.**

```bash
git tag -l 'port-rewrite-phase-3-5-complete'
# Expected: port-rewrite-phase-3-5-complete
git log --oneline main | head -3
# Expected: most recent commit is the merge of 3.5 into main
git status
# Expected: clean
```

If the tag is missing, Phase 3.5 is not complete. STOP and complete Phase 3.5 first.

- [ ] **Step 1.2: Confirm Phase 3.5 invariants hold at entry.**

```bash
rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emDialog.rs
# Expected: 0 matches (I5d for emDialog.rs — 3.5 already removed it)

rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs
# Expected: 1 match (I5b)

rg -n 'silent_cancel' crates/
# Expected: 0 matches (I5e)

# Sanity: the emFileDialog Cycle method (still present — not yet deleted; this phase deletes it):
rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emFileDialog.rs
# Expected: 1 match (to be deleted in Task 4)
```

- [ ] **Step 1.3: Check out the Phase 3.6 branch and create the ledger.**

```bash
git checkout -b port-rewrite/phase-3-6-emfiledialog-e024

cat > docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md <<'EOF'
# Phase 3.6 — emFileDialog rides 3.5; E024 closure — Ledger

**Started:** 2026-04-23
**Branch:** port-rewrite/phase-3-6-emfiledialog-e024
**Baseline:** Phase 3.5 closeout at <SHA>. nextest 2482/0/9 (approximate), goldens 237/6.
**Plan:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-emfiledialog-e024.md
**Brainstorm:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow-plan.md
**JSON entry to close:** E024 (open → resolved-phase-3-6).

## Bootstrap decisions

See plan §"Bootstrap decisions" (B3.6a–B3.6d).

## Task log

(Entries appended by each task's commit.)
EOF

git add docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md
git commit -m "phase-3.6 task 1: entry-gate verified, phase ledger created

3.5 complete at <SHA>. Invariants I5a–I5g confirmed. emFileDialog::Cycle
still present (deleted in Task 4). Ready for 3.6."
```

**Task 1 exit condition:** ledger exists; branch is checked out; entry invariants confirmed.

---

## Task 2: Add `on_cycle_ext` callback slot to `DlgPanel` + extend `DialogPrivateEngine::Cycle`

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Scope:** Prepare the DlgPanel + engine for file-dialog Cycle composition. File-dialog code lands in Task 3.

- [ ] **Step 2.1: Add the callback slot field to `DlgPanel`.**

In `emDialog.rs`, inside the `DlgPanel` struct (added in Phase 3.5 Task 2), append a new field:

```rust
    /// File-dialog-style Cycle extension. DialogPrivateEngine::Cycle calls
    /// this AFTER the base cycle body (close-signal observation, pending_result
    /// resolution, auto-delete countdown), mirroring C++ `emFileDialog::Cycle`
    /// calling `emDialog::Cycle()` first then running file-dialog logic
    /// (emFileDialog.cpp:82). Populated by emFileDialog::new.
    ///
    /// Return value: whether the extension wants to keep the engine awake
    /// (OR'd with the base Cycle's return).
    ///
    /// Signature notes:
    /// - Takes `&mut self` (the DlgPanel) so the extension can mutate
    ///   dialog state (e.g. set pending_result).
    /// - Takes `&mut EngineCtx<'_>` so the extension can observe signals
    ///   (`ctx.scheduler.is_signaled_for_engine(sig, ctx.engine_id)`),
    ///   fire signals, enqueue DeferredActions.
    /// - Return bool: true = stay awake, false = sleep.
    pub(crate) on_cycle_ext: Option<DialogCycleExt>,
```

Add a type alias near the top of `emDialog.rs` (above the `DlgPanel` struct):

```rust
/// Extension callback invoked by DialogPrivateEngine::Cycle after the base
/// cycle body. Used by emFileDialog to add Fsb and overwrite-dialog observation.
pub(crate) type DialogCycleExt =
    Box<dyn FnMut(&mut DlgPanel, &mut crate::emEngineCtx::EngineCtx<'_>) -> bool>;
```

In `DlgPanel::new`, initialise to None:

```rust
            on_cycle_ext: None,
```

- [ ] **Step 2.2: Extend `DialogPrivateEngine::Cycle` to call the extension.**

Locate the end of `DialogPrivateEngine::Cycle`'s body (the `stay_awake` computation inside the behavior borrow). After the auto-delete countdown branch, add a post-amble:

```rust
            // File-dialog extension (B3.6b). Runs AFTER base logic so
            // pending_result from the extension is visible to the base on
            // the NEXT Cycle — matching C++ where emFileDialog::Cycle
            // observes pending_result via emDialog::Cycle() on a subsequent
            // slice when fsb fires during this slice.
            //
            // NOTE: the extension receives &mut EngineCtx, but we're already
            // inside the behavior take/put scope — ctx.tree is the shared
            // tree, and self.root_panel_id is taken. The extension must NOT
            // call tree.take_behavior(self.root_panel_id) (it's already taken).
            // It CAN take other panels (e.g. the overwrite dialog's root).
            let ext_stay = dlg_panel
                .on_cycle_ext
                .as_mut()
                .map(|ext| ext(dlg_panel, ctx))
                .unwrap_or(false);
            // XXX: `dlg_panel.on_cycle_ext.as_mut().map(|ext| ext(dlg_panel, ctx))`
            // is a double borrow of dlg_panel. Rust rejects. Fix: swap the
            // callback out, call it with dlg_panel, swap back.
```

The double-borrow note above flags a real Rust-borrow-checker issue. The fix is the swap-out pattern:

```rust
            let ext_stay = {
                if let Some(mut ext) = dlg_panel.on_cycle_ext.take() {
                    let stay = ext(dlg_panel, ctx);
                    dlg_panel.on_cycle_ext = Some(ext);
                    stay
                } else {
                    false
                }
            };
            stay_awake || ext_stay
```

Also update the final return from `false` (for the non-auto-delete branch) to `ext_stay` so the extension's stay-awake flag propagates.

Reshape the whole stay_awake computation:

```rust
        let stay_awake = {
            let Some(dlg_panel) = behavior.as_dlg_panel_mut() else {
                panic!("DialogPrivateEngine::Cycle: root_panel behavior is not DlgPanel");
            };

            // --- Base cycle body (from Phase 3.5 Task 4) ---
            // Step 1: close_signal observation
            // Step 2: button click_signals
            // Step 3: pending_result → finalized + callbacks
            // Step 4: auto-delete countdown
            // [existing code from Phase 3.5 Task 4.1 — unchanged]

            let base_auto_delete_stay = /* the bool from step 4 */;

            // --- Extension (this phase) ---
            let ext_stay = {
                if let Some(mut ext) = dlg_panel.on_cycle_ext.take() {
                    let stay = ext(dlg_panel, ctx);
                    dlg_panel.on_cycle_ext = Some(ext);
                    stay
                } else {
                    false
                }
            };

            base_auto_delete_stay || ext_stay
        };

        ctx.tree.put_behavior(self.root_panel_id, behavior);
        stay_awake
```

If the existing Phase-3.5 Cycle body has different internal control flow, preserve its semantics and splice the extension in at the equivalent position.

- [ ] **Step 2.3: Add a unit test for the extension slot.**

Append to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn dialog_private_engine_calls_on_cycle_ext() {
        use crate::emPanel::PanelBehavior;
        use crate::emPanelTree::PanelTree;
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut __init = TestInit::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("dlg", true);

        let finish_sig = __init.sched.create_signal();
        let close_sig = __init.sched.create_signal();
        let mut dlg_panel = DlgPanel::new("T", emLook::new(), finish_sig);

        // Extension records its call count in a shared cell.
        let call_count = Rc::new(RefCell::new(0u32));
        let call_count_ext = call_count.clone();
        dlg_panel.on_cycle_ext = Some(Box::new(
            move |_dlg: &mut DlgPanel, _ctx: &mut crate::emEngineCtx::EngineCtx<'_>| {
                *call_count_ext.borrow_mut() += 1;
                false
            },
        ));

        tree.set_behavior(root, Box::new(dlg_panel));
        tree.init_panel_view(root, Some(&mut __init.sched));

        let eng = DialogPrivateEngine::new(root, close_sig);
        let _eid = eng.install(&mut __init.sched, crate::emEngine::TreeLocation::Outer);

        // Wake the engine once (by firing close_signal, which the engine
        // subscribes to — simplest way to force a Cycle).
        __init.sched.fire(close_sig);
        let mut pending_inputs = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            RefCell::new(None);
        let mut windows = std::collections::HashMap::new();
        __init.sched.DoTimeSlice(
            &mut tree,
            &mut windows,
            &__init.root,
            &mut __init.fw,
            &mut pending_inputs,
            &mut input_state,
            &fw_cb,
            std::time::Instant::now() + std::time::Duration::from_millis(50),
        );

        assert_eq!(*call_count.borrow(), 1, "extension called exactly once per Cycle");

        // Cleanup.
        let _ = tree.take_behavior(root);
    }
```

Adjust `DoTimeSlice` parameters to match the actual signature.

- [ ] **Step 2.4: Run new test + full gate.**

```bash
cargo-nextest run -p emcore --lib emDialog::tests::dialog_private_engine_calls_on_cycle_ext
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: one new passing test. Baseline + 1.

- [ ] **Step 2.5: Commit Task 2.**

```
- **Task 2 — Cycle extension slot:** commit <SHA>. DlgPanel gains
  `on_cycle_ext: Option<DialogCycleExt>` callback field (type alias added
  at emDialog.rs top). DialogPrivateEngine::Cycle takes+calls+puts-back the
  extension after base logic. Swap-out pattern avoids double &mut dlg_panel
  borrow. One unit test asserting the extension is called per Cycle.
  emFileDialog not yet using the slot (Task 3). Gate green.
```

```bash
git add crates/emcore/src/emDialog.rs docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md
git commit -m "phase-3.6 task 2: DlgPanel gains on_cycle_ext slot; engine calls it post-base

Prepares the Cycle composition shape per B3.6b (D2 single-engine design).
Extension callback is a Box<dyn FnMut(&mut DlgPanel, &mut EngineCtx) -> bool>
taken from DlgPanel, called, then put back. Matches C++ emFileDialog::Cycle()
calling emDialog::Cycle() first then adding file-dialog logic.

Extension is None today — Task 3 populates it for emFileDialog.
Gate green."
```

**Task 2 exit condition:** `rg -n 'on_cycle_ext' crates/emcore/src/emDialog.rs` → ≥3 matches (struct field + type alias + engine call).

---

## Task 3: Reshape `emFileDialog` to compose 3.5 `emDialog` + install Fsb as content; populate `on_cycle_ext`

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`

**Scope:** The keystone. emFileDialog goes from "plain owned struct with its own polled Cycle method" to "composition over 3.5 emDialog + fsb installed under content_panel + on_cycle_ext callback that ports emFileDialog.cpp:80-106".

- [ ] **Step 3.1: Rewrite the `emFileDialog` struct — composition over 3.5 emDialog.**

**Design decision locked upfront:** overwrite-dialog transient state lives on **DlgPanel** (not on emFileDialog) so the `on_cycle_ext: Box<dyn FnMut(&mut DlgPanel, ...)>` closure can reach it through its `&mut DlgPanel` argument — avoiding `Rc<RefCell<Option<emDialog>>>` which would be a Do-NOT violation per CLAUDE.md. This decision is codified in the Task 2 DlgPanel field set (see Step 3.2 below which adds those fields as part of this Task, not as a retroactive 3.5 edit).

Replace the current struct (emFileDialog.rs:40-58) with:

```rust
/// File-open / file-save dialog composing an `emDialog` + `emFileSelectionBox`.
///
/// Port of C++ `class emFileDialog : public emDialog` (emFileDialog.h:37).
/// DIVERGED: Rust uses composition for C++ inheritance (idiom adaptation —
/// observable behavior identical). The owned `dialog: emDialog` provides the
/// window/root-panel/private-engine infrastructure from Phase 3.5; the
/// emFileSelectionBox is installed as a child panel under
/// `dialog.GetContentPanel()` at construction.
///
/// Scheduler-driven Cycle: on construction, the outer emDialog's
/// DialogPrivateEngine subscribes to `fsb.file_trigger_signal` via
/// `scheduler.connect(...)` (port of emFileDialog.cpp:41
/// `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`). The file-dialog
/// per-cycle logic (emFileDialog.cpp:80-106) lives in the `on_cycle_ext`
/// callback installed on the dialog's DlgPanel.
///
/// Overwrite-confirmation transient state (the popped-up "File Exists"
/// sub-dialog + its asked-text) lives on `DlgPanel` (see `on_cycle_ext`
/// closure access pattern) — NOT on this struct. This avoids
/// `Rc<RefCell<...>>` because the `'static + FnMut` closure cannot borrow
/// from emFileDialog.
pub struct emFileDialog {
    dialog: crate::emDialog::emDialog,
    /// PanelId of the emFileSelectionBox installed under content panel.
    fsb_panel_id: crate::emPanelTree::PanelId,
    mode: FileDialogMode,
    dir_allowed: bool,
    /// Last-confirmed overwrite text (matches C++ `OverwriteConfirmed`).
    /// Used by CheckFinish to short-circuit a re-prompt when the user
    /// has already confirmed overwriting the same set of paths.
    overwrite_confirmed: String,
}
```

- [ ] **Step 3.2: Extend `DlgPanel` with file-dialog transient fields (this phase, not retroactive to 3.5).**

Edit `DlgPanel` struct (originally added in Phase 3.5 Task 2, extended with `on_cycle_ext` in Phase 3.6 Task 2, now extended again). Add:

```rust
    /// File-dialog overwrite-confirmation sub-dialog. Set by
    /// emFileDialog::CheckFinish when Save-mode detects overwrite
    /// conflicts; consumed + torn down by the file-dialog on_cycle_ext
    /// closure at next Cycle. Port of C++ `emCrossPtr<emDialog>
    /// emFileDialog::OverwriteDialog` (emFileDialog.h:204); the
    /// `emCrossPtr` auto-null semantics are captured by the Option's
    /// None state combined with on_cycle_ext's take-deregister-drop
    /// pattern on finish observation.
    pub(crate) overwrite_dialog: Option<crate::emDialog::emDialog>,
    /// Text being confirmed for overwrite. Matches C++ `OverwriteAsked`
    /// in emFileDialog.h:202.
    pub(crate) overwrite_asked: String,
```

Update `DlgPanel::new` initialiser:

```rust
            overwrite_dialog: None,
            overwrite_asked: String::new(),
```

Verify:

```bash
rg -n 'overwrite_dialog: Option<crate::emDialog' crates/emcore/src/emDialog.rs
# Expected: 1 match (the field declaration)
```

This extension is part of the Task 3 commit (not a separate commit) since the fields only exist to serve the closure added later in Task 3. One logical unit.

- [ ] **Step 3.3: Write the new `emFileDialog::new` ctor.**

Replace the existing ctor (emFileDialog.rs:60-86) with:

```rust
impl emFileDialog {
    pub fn new(
        parent_context: std::rc::Rc<crate::emContext::emContext>,
        mode: FileDialogMode,
        look: std::rc::Rc<crate::emLook::emLook>,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        framework_actions: &mut Vec<crate::emEngineCtx::DeferredAction>,
        root_context: &std::rc::Rc<crate::emContext::emContext>,
        tree: &mut crate::emPanelTree::PanelTree,
        // whatever pending_windows handle 3.5.A landed on:
        pending_windows: &mut /* PendingWindowQueue-or-equiv */,
    ) -> Self {
        let (title, ok_label) = mode_title_and_ok(mode);

        // 1. Construct the outer dialog (Phase 3.5).
        let mut dialog = crate::emDialog::emDialog::new(
            std::rc::Rc::clone(&parent_context),
            title,
            std::rc::Rc::clone(&look),
            scheduler,
            pending_windows,
        );

        // 2. Add OK/Cancel buttons.
        dialog.AddCustomButton(
            tree, scheduler, framework_actions, root_context,
            ok_label, crate::emDialog::DialogResult::Ok,
        );
        dialog.AddCustomButton(
            tree, scheduler, framework_actions, root_context,
            "Cancel", crate::emDialog::DialogResult::Cancel,
        );

        // 3. Install emFileSelectionBox as a child of content panel.
        let content_id = dialog.GetContentPanel(tree, scheduler);
        let fsb_panel_id = tree.create_child(content_id, "fsb", Some(scheduler));
        let mut init = crate::emEngineCtx::InitCtx {
            scheduler,
            framework_actions,
            root_context,
        };
        let mut fsb = crate::emFileSelectionBox::emFileSelectionBox::new(&mut init, "");
        fsb.border_mut().outer = crate::emBorder::OuterBorderType::None;
        fsb.border_mut().inner = crate::emBorder::InnerBorderType::None;
        let fsb_file_trigger_signal = fsb.file_trigger_signal;
        tree.set_behavior(fsb_panel_id, Box::new(fsb));

        // 4. Subscribe dialog's DialogPrivateEngine to fsb.file_trigger_signal.
        //    Port of C++ emFileDialog.cpp:41 AddWakeUpSignal(Fsb->GetFileTriggerSignal()).
        scheduler.connect(fsb_file_trigger_signal, dialog.private_engine_id());

        // 5. Install on_cycle_ext on DlgPanel — the file-dialog Cycle logic.
        //    Capture fsb_file_trigger_signal and the private_engine_id for
        //    is_signaled_for_engine probes inside the closure.
        let private_engine_id = dialog.private_engine_id();
        let closure_fsb_sig = fsb_file_trigger_signal;
        let on_cycle_ext: crate::emDialog::DialogCycleExt = Box::new(
            move |dlg: &mut crate::emDialog::DlgPanel,
                  ctx: &mut crate::emEngineCtx::EngineCtx<'_>|
            -> bool {
                // Port of emFileDialog.cpp:80-106 emFileDialog::Cycle body:
                //   if (IsSignaled(Fsb->GetFileTriggerSignal())) Finish(POSITIVE);
                //   if (OverwriteDialog && IsSignaled(OverwriteDialog->GetFinishSignal())) {
                //       switch (OverwriteDialog->GetResult()) {
                //         case POSITIVE: OverwriteConfirmed=OverwriteAsked;
                //                        OverwriteAsked.Clear();
                //                        delete OverwriteDialog.Get();
                //                        Finish(POSITIVE); break;
                //         case NEGATIVE: OverwriteAsked.Clear();
                //                        delete OverwriteDialog.Get(); break;
                //       }
                //   }
                let mut stay_awake = false;

                // fsb_file_trigger signalled?
                if ctx
                    .scheduler
                    .is_signaled_for_engine(closure_fsb_sig, private_engine_id)
                {
                    dlg.pending_result = Some(crate::emDialog::DialogResult::Ok);
                }

                // overwrite_dialog finished?
                if let Some(ref od) = dlg.overwrite_dialog {
                    let od_finish_sig = od.GetFinishSignal();
                    if ctx
                        .scheduler
                        .is_signaled_for_engine(od_finish_sig, private_engine_id)
                    {
                        // Observe OD's result. GetResult needs &mut tree — but
                        // we're inside DialogPrivateEngine::Cycle which already
                        // has ctx.tree. We can take/put OD's root panel via
                        // ctx.tree.take_behavior(od.root_panel_id) safely since
                        // it's a different panel id from self.
                        let od_root = od.root_panel_id();
                        let od_result = ctx.tree.take_behavior(od_root)
                            .and_then(|mut b| {
                                let r = b.as_dlg_panel_mut()
                                    .and_then(|p| p.finalized_result.clone());
                                ctx.tree.put_behavior(od_root, b);
                                r
                            });
                        match od_result {
                            Some(crate::emDialog::DialogResult::Ok) => {
                                // Promote OverwriteAsked → OverwriteConfirmed.
                                // OverwriteConfirmed lives on emFileDialog, but
                                // here we're in DlgPanel. Park the status by
                                // clearing OverwriteAsked and setting
                                // pending_result; emFileDialog external-state
                                // sync happens at a higher layer.
                                //
                                // Since emFileDialog.overwrite_confirmed is
                                // outside the closure's reach, we stash the
                                // asked-string on DlgPanel for emFileDialog to
                                // pick up via a peek method after the slice.
                                dlg.overwrite_asked.clear();
                                // Tear down OD. deregister requires
                                // &mut scheduler + &mut framework_actions —
                                // we have both via ctx.
                                let mut od = dlg.overwrite_dialog.take().expect("od present");
                                od.deregister(ctx.tree, ctx.scheduler, ctx.framework_actions);
                                drop(od);
                                dlg.pending_result = Some(crate::emDialog::DialogResult::Ok);
                            }
                            Some(crate::emDialog::DialogResult::Cancel) => {
                                dlg.overwrite_asked.clear();
                                let mut od = dlg.overwrite_dialog.take().expect("od present");
                                od.deregister(ctx.tree, ctx.scheduler, ctx.framework_actions);
                                drop(od);
                                // Note: no pending_result — outer dialog stays
                                // open, user can try a different filename.
                            }
                            _ => {}
                        }
                    } else {
                        stay_awake = true; // OD still pending — keep engine awake
                    }
                }

                stay_awake
            },
        );

        // Install the extension on DlgPanel.
        let root_panel_id = dialog.root_panel_id();
        if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
            if let Some(dlg_panel) = behavior.as_dlg_panel_mut() {
                dlg_panel.on_cycle_ext = Some(on_cycle_ext);
            }
            tree.put_behavior(root_panel_id, behavior);
        }

        Self {
            dialog,
            fsb_panel_id,
            mode,
            dir_allowed: false,
            overwrite_confirmed: String::new(),
        }
    }
}
```

**Required accessors on `emDialog` (add if missing):**

```rust
// in emDialog.rs impl emDialog:
impl emDialog {
    pub fn private_engine_id(&self) -> crate::emEngine::EngineId {
        self.private_engine_id
    }

    pub fn root_panel_id(&self) -> crate::emPanelTree::PanelId {
        self.root_panel_id
    }
}
```

Verify these exist (Phase 3.5 may or may not have added them). If missing, add with the above signatures.

- [ ] **Step 3.4: Port the remaining emFileDialog public API.**

All the accessor/mutator methods on emFileDialog currently take `&self` / `&mut self` and read/write its plain-struct fields. After reshape, they either delegate to `self.dialog.METHOD(tree, scheduler, ...)` or reach the fsb via `tree.take_behavior(self.fsb_panel_id)`.

Methods to port (from current emFileDialog.rs):

- `GetMode(&self)` → unchanged, reads `self.mode`.
- `set_mode(&mut self, tree, mode)` — mutates self.mode + delegates to `self.dialog.SetRootTitle` + `self.dialog.set_button_label_for_result`.
- `is_directory_result_allowed(&self)` → reads `self.dir_allowed`.
- `set_directory_result_allowed(&mut self, allowed)` → writes `self.dir_allowed`.
- `is_multi_selection_enabled(&self, tree)` → take fsb behavior, read, put back.
- `set_multi_selection_enabled(&mut self, tree, enabled)` → take fsb, mutate, put back.
- `GetParentDirectory(&self, tree) -> PathBuf` (return owned value since &Path from take/put isn't possible).
- `set_parent_directory(&mut self, tree, dir)`.
- `GetSelectedName(&self, tree) -> Option<String>`.
- `GetSelectedNames(&self, tree) -> Vec<String>`.
- `set_selected_name`, `set_selected_names`, `ClearSelection`, `GetSelectedPath`, `set_selected_path`, `GetFilters`, `set_filters`, `GetSelectedFilterIndex`, `set_selected_filter_index`, `are_hidden_files_shown`, `set_hidden_files_shown` — all take-fsb-mutate-put-back.
- `dialog(&self) -> &emDialog` → returns `&self.dialog`.
- `dialog_mut(&mut self) -> &mut emDialog` → returns `&mut self.dialog`.
- `file_selection_box(&self)` / `_mut` — DELETE these accessors. External code should no longer reach into the embedded fsb directly now that it lives in the tree. (If any caller needs a getter, provide a by-tree accessor.)
- `Finish(&mut self, result, tree, scheduler)` → delegates to `self.dialog.Finish(result, tree, scheduler)`.
- `GetResult(&self, tree)` → `self.dialog.GetResult(tree)`.
- `finish_signal(&self) -> SignalId` → `self.dialog.GetFinishSignal()`.
- `file_trigger_signal(&self, tree) -> SignalId` → take fsb, read `fsb.file_trigger_signal`, put back.
- `overwrite_finish_signal(&self, tree) -> Option<SignalId>` — peek DlgPanel's `overwrite_dialog` field. Returns `None` if no overwrite dialog active.
- `CheckFinish<C: ConstructCtx>(&mut self, ctx, result)` — ports emFileDialog.cpp:110-185. Now takes `&mut tree` + `&mut scheduler` + `&mut framework_actions` + `root_context` + `parent_context` + `pending_windows` so it can construct a transient overwrite emDialog when needed.

For each method: write the new signature + body. Use the `let Some(mut b) = tree.take_behavior(self.fsb_panel_id) else { return /* default */ }; ... tree.put_behavior(self.fsb_panel_id, b); result` pattern.

Given this is ~30 methods, break into logical commits of 5-8 methods each (e.g., "accessors group", "mutators group", "CheckFinish", "signal accessors").

- [ ] **Step 3.5: Port `CheckFinish` with transient overwrite-dialog spawn.**

Most complex of Task 3. Current body at emFileDialog.rs:205-287. Rewrite:

```rust
impl emFileDialog {
    pub fn CheckFinish(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        framework_actions: &mut Vec<crate::emEngineCtx::DeferredAction>,
        root_context: &std::rc::Rc<crate::emContext::emContext>,
        parent_context: std::rc::Rc<crate::emContext::emContext>,
        look: std::rc::Rc<crate::emLook::emLook>,
        pending_windows: &mut /* PendingWindowQueue */,
        result: &crate::emDialog::DialogResult,
    ) -> FileDialogCheckResult {
        if *result == crate::emDialog::DialogResult::Cancel {
            return FileDialogCheckResult::Allow;
        }

        // Read fsb state via take/put.
        let (names, parent, mode_copy) = {
            let Some(mut fsb_behavior) = tree.take_behavior(self.fsb_panel_id) else {
                return FileDialogCheckResult::Error("fsb missing".to_string());
            };
            let names;
            let parent;
            {
                // Assuming FSB has a downcast accessor; if not, add one.
                let fsb = fsb_behavior
                    .as_file_selection_box_mut()
                    .expect("fsb is emFileSelectionBox");
                names = fsb.GetSelectedNames().to_vec();
                parent = fsb.GetParentDirectory().to_path_buf();
            }
            tree.put_behavior(self.fsb_panel_id, fsb_behavior);
            (names, parent, self.mode)
        };

        if !self.dir_allowed {
            if names.is_empty() {
                return FileDialogCheckResult::Error("No file selected".to_string());
            }
            for name in &names {
                let path = parent.join(name);
                if path.is_dir() {
                    if names.len() == 1 {
                        return FileDialogCheckResult::EnterDirectory(name.clone());
                    }
                    return FileDialogCheckResult::Error(format!(
                        "Directory selected: {}",
                        name
                    ));
                }
            }
        }

        match mode_copy {
            FileDialogMode::Open => {
                for name in &names {
                    let path = parent.join(name);
                    if !path.exists() {
                        return FileDialogCheckResult::Error(format!(
                            "The following file cannot be opened, because it does not exist:\n\n{}",
                            path.display()
                        ));
                    }
                }
            }
            FileDialogMode::Save => {
                let mut paths_to_overwrite = Vec::new();
                for name in &names {
                    let path = parent.join(name);
                    if path.exists() {
                        paths_to_overwrite.push(path);
                    }
                }
                if !paths_to_overwrite.is_empty() {
                    let text = if paths_to_overwrite.len() == 1 {
                        format!(
                            "Are you sure to overwrite the following already existing file?\n\n{}",
                            paths_to_overwrite[0].display()
                        )
                    } else {
                        let mut msg = "Are you sure to overwrite the following already existing files?\n"
                            .to_string();
                        for p in &paths_to_overwrite {
                            msg.push('\n');
                            msg.push_str(&p.display().to_string());
                        }
                        msg
                    };

                    if text != self.overwrite_confirmed {
                        // Spawn a transient overwrite emDialog. Store on the
                        // outer dialog's DlgPanel (for the on_cycle_ext closure
                        // to reach).
                        let mut od = crate::emDialog::emDialog::new(
                            parent_context,
                            "File Exists",
                            look,
                            scheduler,
                            pending_windows,
                        );
                        od.AddCustomButton(
                            tree, scheduler, framework_actions, root_context,
                            "OK", crate::emDialog::DialogResult::Ok,
                        );
                        od.AddCustomButton(
                            tree, scheduler, framework_actions, root_context,
                            "Cancel", crate::emDialog::DialogResult::Cancel,
                        );

                        // Subscribe outer DialogPrivateEngine to OD's finish_signal.
                        scheduler.connect(od.GetFinishSignal(), self.dialog.private_engine_id());

                        // Install OD + asked text on outer DlgPanel.
                        let root_id = self.dialog.root_panel_id();
                        if let Some(mut behavior) = tree.take_behavior(root_id) {
                            if let Some(dlg_panel) = behavior.as_dlg_panel_mut() {
                                dlg_panel.overwrite_dialog = Some(od);
                                dlg_panel.overwrite_asked = text;
                            }
                            tree.put_behavior(root_id, behavior);
                        }

                        return FileDialogCheckResult::ConfirmOverwrite(paths_to_overwrite);
                    }
                }
                self.overwrite_confirmed.clear();
            }
            FileDialogMode::Select => {}
        }

        FileDialogCheckResult::Allow
    }
}
```

Note the new `as_file_selection_box_mut` PanelBehavior accessor — mirrors `as_dlg_panel_mut` from Phase 3.5. Add to the `PanelBehavior` trait in `emPanel.rs` + override on `emFileSelectionBox`:

```rust
// in emPanel.rs:
    fn as_file_selection_box_mut(
        &mut self,
    ) -> Option<&mut crate::emFileSelectionBox::emFileSelectionBox> {
        None
    }

// in emFileSelectionBox.rs impl PanelBehavior for emFileSelectionBox:
    fn as_file_selection_box_mut(&mut self) -> Option<&mut emFileSelectionBox> {
        Some(self)
    }
```

- [ ] **Step 3.6: Run cargo check + iterative fixes.**

```bash
cargo check -p emcore
```

Expected: compile errors in emFileDialog.rs for the ~30 ported methods. Fix each. Common issues:
- Missing fsb-accessor downcast — add via pattern above.
- `dialog.GetContentPanel` signature mismatch — align with Phase 3.5's actual signature.
- Lifetime issues on `on_cycle_ext` closure — the closure is `'static + FnMut`, can't borrow self. Captures must be `Copy` (like SignalId, EngineId, PanelId) or owned.

- [ ] **Step 3.7: Full gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: existing emFileDialog tests still pass (they were the 4 Cycle-path tests from Phase 3 Task 6). They still use the `pub fn Cycle(&mut self, ctx)` method, which is still present — it's deleted in Task 4. If the tests break at Task 3 due to API change (e.g., `emFileDialog::new` signature changed), mark them `#[ignore = "phase-3.6 task 4: deferred; Cycle deleted in task 4"]` temporarily.

- [ ] **Step 3.8: Commit Task 3.**

```
- **Task 3 — emFileDialog reshape:** commit <SHA>. emFileDialog composes
  3.5 emDialog + installs emFileSelectionBox as child of content_panel.
  Subscribes outer DialogPrivateEngine to fsb.file_trigger_signal via
  scheduler.connect (AddWakeUpSignal port). on_cycle_ext closure ports
  emFileDialog.cpp:80-106 — observes fsb signal + overwrite_dialog's
  finish signal, tears down OD + promotes asked→confirmed + sets
  pending_result for the outer dialog.
  CheckFinish spawns transient overwrite emDialog via 3.5 ctor, parks
  it on outer DlgPanel.overwrite_dialog for the closure to reach.
  Added as_file_selection_box_mut PanelBehavior trait method (pattern
  mirrors as_dlg_panel_mut).
  4 existing Cycle-path tests temporarily #[ignore] — deleted in Task 4.
  pub fn Cycle(&mut self, ctx) STILL PRESENT — deletion in Task 4.
  Gate green.
```

```bash
git add crates/emcore/src/emFileDialog.rs crates/emcore/src/emDialog.rs \
        crates/emcore/src/emPanel.rs crates/emcore/src/emFileSelectionBox.rs \
        docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md
git commit -m "phase-3.6 task 3: emFileDialog reshaped — composes 3.5 emDialog + Fsb child panel

Ports C++ class emFileDialog : public emDialog via composition.
AddWakeUpSignal equivalent via scheduler.connect(fsb.file_trigger_signal,
dialog.private_engine_id()).

File-dialog Cycle logic ported into on_cycle_ext closure on DlgPanel,
running after base DialogPrivateEngine::Cycle. Beats match
emFileDialog.cpp:80-106 exactly.

CheckFinish spawns transient overwrite emDialog via 3.5 ctor. Outer
DialogPrivateEngine subscribes to OD's finish_signal; on_cycle_ext
tears down OD on resolution.

pub fn Cycle still present — Task 4 deletes it.
Gate green."
```

**Task 3 exit condition:** `rg -n 'on_cycle_ext' crates/emcore/src/emFileDialog.rs` → 1 (the closure); `rg -n 'scheduler\.connect' crates/emcore/src/emFileDialog.rs` → ≥2 (fsb + overwrite subscriptions).

---

## Task 4: Delete `emFileDialog::Cycle` + `test_force_overwrite_result` + the `DIVERGED:` block

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`

- [ ] **Step 4.1: Delete the caller-invoked `pub fn Cycle(&mut self, ctx: &mut PanelCtx<'_>) -> bool`.**

Target: emFileDialog.rs:322-371 (the whole `pub fn Cycle` method, including the `DIVERGED:` block above it at lines 307-321).

After deletion, verify:

```bash
rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emFileDialog.rs
# Expected: 0

rg -n 'DIVERGED:' crates/emcore/src/emFileDialog.rs
# Expected: 0 or whatever remains if other DIVERGED comments exist (check the full file).
```

- [ ] **Step 4.2: Delete `fsb_file_trigger_signal: SignalId` cached field.**

This field was added in Phase 3 Task 6 to cache the fsb's trigger signal for the caller-invoked Cycle to probe. With scheduler-driven dispatch, the closure captures `fsb_file_trigger_signal` directly at construction. The field is vestigial.

Find + remove:

```bash
rg -n 'fsb_file_trigger_signal' crates/emcore/src/emFileDialog.rs
```

Remove the field declaration, the ctor initialisation, and any remaining reads (there should be none after the closure captures it).

- [ ] **Step 4.3: Delete `test_force_overwrite_result` helper.**

Target: emFileDialog.rs:401-416. This helper was a test-only back door for forcing the overwrite dialog's internal result without firing its finish_signal. With scheduler-driven dispatch, tests drive the overwrite dialog via normal `Finish(result)` + `DoTimeSlice`.

```bash
rg -n 'test_force_overwrite_result' crates/
# Expected after delete: 0
```

If any tests still call it, migrate the tests first (see Task 5 test migrations).

- [ ] **Step 4.4: Delete the 4 pre-port Cycle-path tests OR migrate them.**

Tests at the bottom of emFileDialog.rs:
- `cycle_no_signals_is_no_op` (line 536)
- `cycle_file_trigger_signal_finishes_ok` (line 555)
- `cycle_overwrite_dialog_positive_confirms_and_finishes` (line 583)
- `cycle_overwrite_dialog_negative_cancels_overwrite_only` (line 641)

These all call `dlg.Cycle(&mut ctx)` directly — the API being deleted. They get DELETED here because Task 5 adds NEW scheduler-driven tests that cover the same matrix at the correct observation boundary.

- [ ] **Step 4.5: Full gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: everything green. Nextest count = baseline - 4 (deleted tests).

- [ ] **Step 4.6: Commit Task 4.**

```
- **Task 4 — Delete caller-invoked Cycle + vestigial helpers:** commit <SHA>.
  - `emFileDialog::Cycle(&mut self, ctx: &mut PanelCtx)` DELETED.
  - DIVERGED: block at Cycle DELETED.
  - `fsb_file_trigger_signal: SignalId` cached field DELETED.
  - `test_force_overwrite_result` test helper DELETED.
  - 4 pre-port Cycle-path tests DELETED — replaced by scheduler-driven
    tests in Task 5.
  Gate green — nextest (baseline-4)/0/9.
```

```bash
git add crates/emcore/src/emFileDialog.rs docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md
git commit -m "phase-3.6 task 4: delete emFileDialog::Cycle + vestigial helpers

Removes the caller-invoked Cycle method, the DIVERGED: block naming the
pre-closure dispatch-timing divergence, the fsb_file_trigger_signal
cached field (replaced by closure capture in Task 3), and the
test_force_overwrite_result test helper. 4 caller-Cycle tests deleted —
replaced by scheduler-driven tests in Task 5.

Gate green."
```

**Task 4 exit condition:**
- `rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emFileDialog.rs` → 0
- `rg -n 'test_force_overwrite_result' crates/` → 0
- `rg -n 'fsb_file_trigger_signal' crates/emcore/src/emFileDialog.rs` → 0

---

## Task 5: E024 closure tests (scheduler-driven; no caller Cycle invocation)

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs` (new test module at bottom)

**Scope:** Write the tests that demonstrate E024 has closed structurally. These are the MECHANICAL ARBITERS per CLAUDE.md Authority Order §2 (golden tests — but these are behavioural rather than pixel).

- [ ] **Step 5.1: Build the `FileDialogTestHarness` helper.**

In `crates/emcore/src/test_view_harness.rs`, add (adjacent to the `DialogTestHarness` from Phase 3.5):

```rust
#[cfg(any(test, feature = "test-support"))]
pub struct FileDialogTestHarness {
    pub dialog_harness: DialogTestHarness,
    pub tmp_dir: std::path::PathBuf,
}

#[cfg(any(test, feature = "test-support"))]
impl FileDialogTestHarness {
    pub fn new() -> Self {
        let tmp_dir = std::env::temp_dir().join(format!(
            "emcore_filedialog_test_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
        Self {
            dialog_harness: DialogTestHarness::new(),
            tmp_dir,
        }
    }

    pub fn write_test_file(&self, name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = self.tmp_dir.join(name);
        std::fs::write(&path, content).expect("write test file");
        path
    }

    pub fn run_n_slices(&mut self, n: usize) {
        for _ in 0..n {
            self.dialog_harness.run_one_slice();
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for FileDialogTestHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp_dir);
    }
}
```

- [ ] **Step 5.2: Write test 1 — fsb file_trigger_signal drives dialog to finish via scheduler.**

At the bottom of `emFileDialog.rs` under a fresh `#[cfg(test)] mod e024_closure_tests`:

```rust
#[cfg(test)]
mod e024_closure_tests {
    use super::*;
    use crate::test_view_harness::FileDialogTestHarness;

    /// E024 closure: the scheduler drives emFileDialog's finish when
    /// fsb.file_trigger_signal fires, with no caller invocation of Cycle
    /// anywhere in the test.
    #[test]
    fn fsb_file_trigger_drives_dialog_to_finish_via_scheduler() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("hello.txt", b"hi");

        let look = crate::emLook::emLook::new();
        let mut fd = emFileDialog::new(
            std::rc::Rc::clone(&h.dialog_harness.root),
            FileDialogMode::Open,
            look,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.pending_windows,
        );
        fd.set_parent_directory(&mut h.dialog_harness.tree, &h.tmp_dir);
        fd.set_selected_name(&mut h.dialog_harness.tree, "hello.txt");

        let finish_sig = fd.finish_signal();
        let fsb_trigger_sig = fd.file_trigger_signal(&mut h.dialog_harness.tree);

        // USER ACTION: fire fsb's file-trigger signal (simulating double-click
        // or Enter on a file in the fsb). From this point forward, the test
        // NEVER calls any Cycle method.
        h.dialog_harness.sched.fire(fsb_trigger_sig);

        // Scheduler runs one slice. The dialog's DialogPrivateEngine,
        // subscribed to fsb_trigger_sig via scheduler.connect, is woken.
        // Its Cycle body runs the on_cycle_ext closure (Task 3), which
        // observes the signal and sets DlgPanel.pending_result = Some(Ok).
        // On the same Cycle (step 3 of the engine's base body), pending_result
        // is resolved into finalized_result + fires finish_signal.
        h.run_n_slices(1);

        assert!(
            h.dialog_harness.sched.is_pending(finish_sig),
            "E024 closure: finish_signal must be pending after scheduler-driven dispatch",
        );
        assert_eq!(
            fd.GetResult(&mut h.dialog_harness.tree),
            Some(crate::emDialog::DialogResult::Ok),
            "E024 closure: GetResult must be Ok after fsb trigger",
        );

        // Cleanup.
        fd.deregister(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
        );
    }
}
```

- [ ] **Step 5.3: Write test 2 — overwrite-POSITIVE path.**

```rust
    #[test]
    fn save_mode_overwrite_positive_finishes_outer_dialog_via_scheduler() {
        let mut h = FileDialogTestHarness::new();
        let existing = h.write_test_file("doc.txt", b"existing content");

        let look = crate::emLook::emLook::new();
        let mut fd = emFileDialog::new(
            std::rc::Rc::clone(&h.dialog_harness.root),
            FileDialogMode::Save,
            look.clone(),
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.pending_windows,
        );
        fd.set_parent_directory(&mut h.dialog_harness.tree, &h.tmp_dir);
        fd.set_selected_name(&mut h.dialog_harness.tree, "doc.txt");

        // User clicks OK (fires dialog's pending_result → CheckFinish runs).
        // CheckFinish detects overwrite, spawns transient OD, subscribes OD's
        // finish_signal to outer engine, returns ConfirmOverwrite.
        let check = fd.CheckFinish(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            std::rc::Rc::clone(&h.dialog_harness.root),
            look,
            &mut h.dialog_harness.pending_windows,
            &crate::emDialog::DialogResult::Ok,
        );
        assert!(matches!(check, FileDialogCheckResult::ConfirmOverwrite(_)));

        let od_finish_sig = fd
            .overwrite_finish_signal(&h.dialog_harness.tree)
            .expect("OD present after CheckFinish confirm");
        let outer_finish_sig = fd.finish_signal();

        // USER ACTION: user clicks OK on OD → its finish_signal fires after
        // its own engine's Cycle resolves pending_result. Simulate by driving
        // OD.Finish via its tree-routed API.
        // To reach OD: read DlgPanel.overwrite_dialog; call Finish on it.
        {
            let root_id = fd.dialog().root_panel_id();
            let mut behavior = h.dialog_harness.tree.take_behavior(root_id).unwrap();
            let dlg = behavior
                .as_dlg_panel_mut()
                .expect("DlgPanel");
            let od = dlg.overwrite_dialog.as_mut().expect("OD present");
            od.Finish(
                crate::emDialog::DialogResult::Ok,
                /* need &mut tree — but we hold dlg_panel. Uses its own
                 * tree-take for OD's own root_panel. Since OD is a separate
                 * top-level window, its root_panel_id is different from
                 * fd.dialog.root_panel_id. */
                // ... this gets awkward. Simplest: release behavior first,
                // call od.Finish through fd's accessor API.
                todo!("see next line"),
                &mut h.dialog_harness.sched,
            );
            h.dialog_harness.tree.put_behavior(root_id, behavior);
        }

        // Cleaner shape — expose a helper on emFileDialog:
        //   pub(crate) fn finish_overwrite_dialog_for_test(&mut self, tree, scheduler, result)
        // that takes the outer DlgPanel, calls Finish on the embedded OD,
        // puts back. Tests call that.
        //
        // Add to emFileDialog:
        //
        // #[cfg(test)]
        // pub(crate) fn finish_overwrite_dialog_for_test(
        //     &mut self,
        //     tree: &mut crate::emPanelTree::PanelTree,
        //     scheduler: &mut crate::emScheduler::EngineScheduler,
        //     result: crate::emDialog::DialogResult,
        // ) {
        //     let root_id = self.dialog.root_panel_id();
        //     let Some(mut behavior) = tree.take_behavior(root_id) else { return };
        //     if let Some(dlg) = behavior.as_dlg_panel_mut() {
        //         if let Some(od) = dlg.overwrite_dialog.as_mut() {
        //             // Releasing dlg here so we can reach tree again —
        //             // but dlg borrows behavior which holds tree via
        //             // take_behavior scope. This requires a different shape:
        //             // call od.Finish with a SECOND tree argument? No —
        //             // there's only one tree.
        //             //
        //             // Since od has its own root_panel_id (distinct), od.Finish
        //             // only takes/puts od.root_panel_id, which is fine.
        //             // But od.Finish requires &mut tree — which is currently
        //             // consumed by our outer take_behavior.
        //             //
        //             // Resolution: set pending_result on OD's DlgPanel directly
        //             // via a sub-take. OD's Finish() does exactly that +
        //             // wake_up. We can do it inline:
        //             let od_root = od.root_panel_id();
        //             let Some(mut od_behavior) = tree.take_behavior(od_root) else {
        //                 /* tree already taken by outer — this shows the bug */
        //                 return;
        //             };
        //             // ...
        //         }
        //     }
        //     tree.put_behavior(root_id, behavior);
        // }
        //
        // The recursive take_behavior problem above shows the test needs a
        // different approach. See Step 5.4 for resolution.
    }
```

**Stop.** Step 5.3 has surfaced a real problem: to reach the overwrite dialog's inner state, we need nested `tree.take_behavior` calls on two different panel ids (outer root + OD root). The tree does support this (take_behavior just removes the behavior from its slotmap slot; two different slots don't conflict).

The *actual* issue is the `emDialog::Finish(result, tree, scheduler)` signature: it takes `&mut tree`, but during the outer take_behavior scope we already hold `&mut tree`. The fix: release outer behavior first, call od.Finish, re-take outer. Let me restructure:

- [ ] **Step 5.4: Rewrite test 2 cleanly.**

```rust
    #[test]
    fn save_mode_overwrite_positive_finishes_outer_dialog_via_scheduler() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("doc.txt", b"existing content");

        let look = crate::emLook::emLook::new();
        let mut fd = emFileDialog::new(
            std::rc::Rc::clone(&h.dialog_harness.root),
            FileDialogMode::Save,
            look.clone(),
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.pending_windows,
        );
        fd.set_parent_directory(&mut h.dialog_harness.tree, &h.tmp_dir);
        fd.set_selected_name(&mut h.dialog_harness.tree, "doc.txt");

        let check = fd.CheckFinish(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            std::rc::Rc::clone(&h.dialog_harness.root),
            look,
            &mut h.dialog_harness.pending_windows,
            &crate::emDialog::DialogResult::Ok,
        );
        assert!(matches!(check, FileDialogCheckResult::ConfirmOverwrite(_)));
        let od_finish_sig = fd
            .overwrite_finish_signal(&h.dialog_harness.tree)
            .expect("OD present");
        let outer_finish_sig = fd.finish_signal();

        // USER ACTION: drive OD Finish via the harness helper.
        fd.finish_overwrite_dialog_for_test(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            crate::emDialog::DialogResult::Ok,
        );

        // Two slices: first slice OD's engine finalizes + fires OD finish_signal
        // (connected to outer engine which wakes); second slice outer on_cycle_ext
        // observes OD.finish_signal pending + result=Ok → clears OD, sets
        // outer pending_result=Ok; and base cycle resolves pending → outer
        // finish_signal fires. Depending on instant-signal-chaining in the
        // scheduler, one slice may suffice.
        h.run_n_slices(2);

        assert!(
            h.dialog_harness.sched.is_pending(outer_finish_sig),
            "outer finish signal must be pending after OD→Ok",
        );
        assert!(
            fd.overwrite_finish_signal(&h.dialog_harness.tree).is_none(),
            "OD torn down after confirmation",
        );
        assert_eq!(
            fd.GetResult(&mut h.dialog_harness.tree),
            Some(crate::emDialog::DialogResult::Ok),
        );

        fd.deregister(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
        );
    }
```

Add the helper on emFileDialog, gated `#[cfg(test)]`:

```rust
#[cfg(test)]
impl emFileDialog {
    /// Test-only: drive OD's Finish via the tree. Used by e024_closure_tests
    /// to simulate user click on the overwrite dialog's buttons without
    /// going through a winit Input pipeline.
    pub(crate) fn finish_overwrite_dialog_for_test(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        result: crate::emDialog::DialogResult,
    ) {
        // Take outer DlgPanel, extract od, release outer, call od.Finish, re-store od.
        let root_id = self.dialog.root_panel_id();
        let mut od: Option<crate::emDialog::emDialog> = {
            let Some(mut behavior) = tree.take_behavior(root_id) else { return };
            let ov = behavior
                .as_dlg_panel_mut()
                .and_then(|dlg| dlg.overwrite_dialog.take());
            tree.put_behavior(root_id, behavior);
            ov
        };
        if let Some(ref mut od_) = od {
            od_.Finish(result, tree, scheduler);
        }
        // Restore OD onto DlgPanel.
        if let Some(od_) = od {
            if let Some(mut behavior) = tree.take_behavior(root_id) {
                if let Some(dlg) = behavior.as_dlg_panel_mut() {
                    dlg.overwrite_dialog = Some(od_);
                }
                tree.put_behavior(root_id, behavior);
            }
        }
    }
}
```

- [ ] **Step 5.5: Write test 3 — overwrite-NEGATIVE path.**

Symmetric to test 2 but with `DialogResult::Cancel`. Assertions:
- `outer_finish_sig` is NOT pending.
- `fd.GetResult()` is still None (outer dialog stays open).
- `fd.overwrite_finish_signal()` is None (OD torn down).

```rust
    #[test]
    fn save_mode_overwrite_negative_tears_down_od_outer_stays_open() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("doc.txt", b"existing");

        let look = crate::emLook::emLook::new();
        let mut fd = emFileDialog::new(
            std::rc::Rc::clone(&h.dialog_harness.root),
            FileDialogMode::Save,
            look.clone(),
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.pending_windows,
        );
        fd.set_parent_directory(&mut h.dialog_harness.tree, &h.tmp_dir);
        fd.set_selected_name(&mut h.dialog_harness.tree, "doc.txt");

        let _ = fd.CheckFinish(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            std::rc::Rc::clone(&h.dialog_harness.root),
            look,
            &mut h.dialog_harness.pending_windows,
            &crate::emDialog::DialogResult::Ok,
        );
        let outer_finish_sig = fd.finish_signal();

        fd.finish_overwrite_dialog_for_test(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            crate::emDialog::DialogResult::Cancel,
        );

        h.run_n_slices(2);

        assert!(
            !h.dialog_harness.sched.is_pending(outer_finish_sig),
            "outer finish must NOT be pending on OD→Cancel",
        );
        assert!(
            fd.overwrite_finish_signal(&h.dialog_harness.tree).is_none(),
            "OD torn down on Cancel too",
        );
        assert_eq!(
            fd.GetResult(&mut h.dialog_harness.tree),
            None,
            "outer dialog stays open after overwrite-cancel",
        );

        fd.deregister(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
        );
    }
```

- [ ] **Step 5.6: Write test 4 — no-signal Cycle is a no-op.**

Sanity check: if no signals are pending, running one slice does NOT finish the dialog. Counterpart to the old `cycle_no_signals_is_no_op`.

```rust
    #[test]
    fn no_signals_one_slice_is_no_op() {
        let mut h = FileDialogTestHarness::new();
        let look = crate::emLook::emLook::new();
        let mut fd = emFileDialog::new(
            std::rc::Rc::clone(&h.dialog_harness.root),
            FileDialogMode::Open,
            look,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
            &h.dialog_harness.root,
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.pending_windows,
        );
        let finish_sig = fd.finish_signal();

        h.run_n_slices(1);

        assert!(!h.dialog_harness.sched.is_pending(finish_sig));
        assert_eq!(fd.GetResult(&mut h.dialog_harness.tree), None);

        fd.deregister(
            &mut h.dialog_harness.tree,
            &mut h.dialog_harness.sched,
            &mut h.dialog_harness.framework_actions,
        );
    }
```

- [ ] **Step 5.7: Run the 4 new tests + invariant check.**

```bash
cargo-nextest run -p emcore --lib emFileDialog::e024_closure_tests
```

Expected: 4 passed.

```bash
rg -n '\.Cycle\(' crates/emcore/src/emFileDialog.rs
# Expected: ZERO matches inside test bodies. Any match is a bug — the test
# is cheating on the E024 closure criterion.
```

Also:

```bash
rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emFileDialog.rs
# Expected: 0 (Task 4 deleted it)
```

- [ ] **Step 5.8: Full gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```

Expected: full suite passes. goldens 237/6 preserved.

- [ ] **Step 5.9: Commit Task 5.**

```
- **Task 5 — E024 closure tests:** commit <SHA>. Four scheduler-driven tests
  covering: fsb_file_trigger→finish (the core E024 closure observation),
  save-mode overwrite-POSITIVE, save-mode overwrite-NEGATIVE, no-signals
  no-op. None of the tests invoke any Cycle method — signals are fired into
  the scheduler, DoTimeSlice runs, assertions on pending-signals and
  finalized_result. This IS the E024 closure proof.
  Added FileDialogTestHarness in test_view_harness.rs.
  Added cfg(test) emFileDialog::finish_overwrite_dialog_for_test helper
  to drive OD.Finish via tree take/put pattern without caller-Cycle
  invocation.
  Gate green — nextest +4, goldens 237/6 preserved.
```

```bash
git add crates/emcore/src/emFileDialog.rs crates/emcore/src/test_view_harness.rs \
        docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md
git commit -m "phase-3.6 task 5: E024 closure tests — 4 scheduler-driven regression tests

Each test fires signals into the scheduler and runs DoTimeSlice(s),
asserting on pending-signals and finalized_result. ZERO tests invoke
any Cycle method — this is the E024 closure mechanical arbiter.

Test coverage:
  - fsb_file_trigger_drives_dialog_to_finish_via_scheduler
  - save_mode_overwrite_positive_finishes_outer_dialog_via_scheduler
  - save_mode_overwrite_negative_tears_down_od_outer_stays_open
  - no_signals_one_slice_is_no_op

Gate green — nextest +4, goldens 237/6 preserved."
```

**Task 5 exit condition:** `rg -n '\.Cycle\(' crates/emcore/src/emFileDialog.rs` inside any `#[cfg(test)] mod e024_closure_tests` block → 0 matches.

---

## Task 6: Flip `E024.status` in raw-material JSON + phase closeout

**Files:**
- Modify: `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json`
- Create: `docs/superpowers/notes/2026-04-23-phase-3-6-closeout.md`
- Modify: `docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md`

- [ ] **Step 6.1: Run all 3.6 invariants from the brainstorm §5 (updated with single-engine decision).**

```bash
# I5a (from brainstorm) — dialog composes emWindow
rg -n 'window_id:' crates/emcore/src/emDialog.rs | head -3
# Expected: ≥1

# I5b — single DialogPrivateEngine
rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs
# Expected: exactly 1

# I5c — wake-up subscriptions
rg -nU 'register_engine\([\s\S]*?DialogPrivateEngine\|DialogPrivateEngine::install' crates/emcore/src/emDialog.rs
# Expected: ≥1
rg -n 'scheduler\.connect' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs
# Expected: ≥3 (close_signal in emDialog + fsb_file_trigger_signal + OD.finish_signal in emFileDialog)

# I5d — no caller-invoked Cycle on dialog or file dialog
rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs
# Expected: 0

# I5e — no silent_cancel
rg -n 'silent_cancel' crates/
# Expected: 0

# I5f — no new Rc<RefCell< on emDialog.rs / emFileDialog.rs
rg -n 'Rc<RefCell<' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs
# Expected: 0 (or pre-existing baseline — this plan introduces none)

# I5g — no new unsafe
rg -n 'unsafe\s*\{' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs
# Expected: the SchedCtx aliased-borrow unsafe block from Phase 3.5 Task 4.1 (if kept)
# — any other must be justified

# I5h — E024 status
python3 -c "
import json
with open('docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json') as f:
    d = json.load(f)
e = next(x for x in d['entries'] if x['id']=='E024')
print('status:', e.get('status','MISSING'))
print('resolution_commit:', e.get('resolution_commit','MISSING'))
"
# Expected at this point (before 6.2): status='open', phase_3_progress=<phase-3 progress>
# After 6.2: status='resolved-phase-3-6'

# I5i — goldens preserved
cargo test --test golden -- --test-threads=1
# Expected: 237 passed / 6 failed

# I5j — nextest delta
cargo-nextest ntr
# Expected: baseline (3.5 exit) + 5 (one in Task 2, four in Task 5) - 4 (deleted old Cycle tests) - some from test_force_overwrite_result deletion = net delta.
```

Record all invariant results in the ledger.

- [ ] **Step 6.2: Update `E024` in raw-material JSON.**

Open `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json`. Find the E024 entry (at `entries[]` index around 893 per the earlier grep). Mutate:

- Set `"status": "resolved-phase-3-6"`.
- Add `"resolution_commit": "<SHA of Task 5 commit>"`.
- Remove the `"phase_3_progress"` field.

Use jq or a small python script (not a manual edit — easy to drift):

```bash
RESOLUTION_SHA=$(git log --format=%H -1 --grep='phase-3.6 task 5')
python3 <<EOF
import json
with open('docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json') as f:
    d = json.load(f)
for e in d['entries']:
    if e['id'] == 'E024':
        e['status'] = 'resolved-phase-3-6'
        e['resolution_commit'] = '$RESOLUTION_SHA'
        e.pop('phase_3_progress', None)
        break
with open('docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json', 'w') as f:
    json.dump(d, f, indent=2, ensure_ascii=False)
EOF
```

Verify:

```bash
python3 -c "
import json
with open('docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json') as f:
    d = json.load(f)
e = next(x for x in d['entries'] if x['id']=='E024')
print('status:', e['status'])
print('resolution_commit:', e.get('resolution_commit'))
print('phase_3_progress present:', 'phase_3_progress' in e)
"
# Expected:
#   status: resolved-phase-3-6
#   resolution_commit: <40-char SHA>
#   phase_3_progress present: False
```

- [ ] **Step 6.3: Write phase closeout note.**

Create `docs/superpowers/notes/2026-04-23-phase-3-6-closeout.md`:

```markdown
# Phase 3.6 — emFileDialog + E024 closure — Closeout

**Branch:** port-rewrite/phase-3-6-emfiledialog-e024
**Commits:** <SHA-range>
**Status:** COMPLETE
**E024 closed at:** <Task-5 SHA>

## Summary

Phase 3.6 reshaped emFileDialog from a plain owned struct with a caller-invoked
Cycle into a composition over the 3.5-ported emDialog + emFileSelectionBox
installed as a child panel under content_panel. Wake-up-signal subscription via
scheduler.connect(fsb.file_trigger_signal, dialog.private_engine_id()) ports C++
emFileDialog.cpp:41 AddWakeUpSignal(Fsb->GetFileTriggerSignal()). The
on_cycle_ext callback on DlgPanel ports emFileDialog.cpp:80-106 Cycle body,
running as a post-amble to the base DialogPrivateEngine::Cycle per D2.

The transient overwrite-confirmation emDialog is a separate top-level dialog
(its own emWindow + DialogPrivateEngine); the outer emFileDialog's engine
subscribes to its finish_signal. on_cycle_ext tears down the OD when its finish
is observed + promotes overwrite_asked→overwrite_confirmed on POSITIVE.

E024 status: resolved-phase-3-6. The mechanical arbiter — 4 scheduler-driven
tests at emFileDialog.rs::e024_closure_tests — asserts that signals fire into
the scheduler and assertions pass with ZERO caller Cycle invocation.

## Delta from Phase 3.5 baseline

<from ledger>

## Invariants

I5a–I5j all green (see ledger table).

## JSON entries closed

- E024 — resolved-phase-3-6 (resolution_commit <SHA>)

## Next

E024 was the last open entry in the scope of the Phase 3 + 3.5 + 3.6 chain.
Phase 4 remains as planned (per docs/superpowers/plans/2026-04-19-port-rewrite-phase-4*.md).
```

- [ ] **Step 6.4: Commit Task 6 + tag.**

```bash
git add docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json \
        docs/superpowers/notes/2026-04-23-phase-3-6-ledger.md \
        docs/superpowers/notes/2026-04-23-phase-3-6-closeout.md
git commit -m "phase-3.6 closeout — E024 resolved-phase-3-6; invariants green; tag applied

Raw-material JSON flipped: E024.status = resolved-phase-3-6,
resolution_commit = <Task-5 SHA>, phase_3_progress cleared.

Invariants I5a–I5j all green per ledger. Goldens 237/6 preserved."

git tag port-rewrite-phase-3-6-complete
```

- [ ] **Step 6.5: Merge to main.**

```bash
git checkout main
git merge --no-ff port-rewrite/phase-3-6-emfiledialog-e024
```

Expected: clean merge. 3.5 is already on main; 3.6 branched from that tip.

---

## Self-review — guardrails against drift + implicit assumptions check

### Implicit assumptions (Phase 3.6-specific) — audit status

| # | Assumption | Verified? | Risk if false | Mitigation |
|---|---|---|---|---|
| B1 | `on_cycle_ext: Option<Box<dyn FnMut(&mut DlgPanel, &mut EngineCtx) -> bool>>` — the `'static + FnMut` bound is achievable, i.e. the file-dialog closure's captures can all be `Copy` (SignalId, EngineId) or owned. | **VERIFIED** — SignalId and EngineId are `Copy` by design (slotmap keys). The closure captures `fsb_file_trigger_signal: SignalId` and `private_engine_id: EngineId`, both `Copy`. No lifetime issues. | — | — |
| B2 | Nested `tree.take_behavior` on different panel ids is sound — the outer DlgPanel is taken, and inside the closure we take OD's root_panel (different id). | **VERIFIED** — PanelTree::take_behavior removes one behavior at one id from a slotmap; two different ids don't alias. | — | — |
| B3 | Phase 3.5.A (runtime top-level window install) supports multiple simultaneous top-level dialogs (outer emFileDialog + transient OD). | **CONTINGENT** — depends on how Phase 3.5.A is implemented. If 3.5.A supports arbitrary multiple pending-window enqueue + materialization, OD as a second top-level dialog works directly. | If 3.5.A limits top-level install to 1-at-a-time: overwrite dialog can't spawn as a new window. | **3.5.A plan MUST explicitly support N simultaneous top-level windows** as a requirement. Call this out when 3.5.A is written. If 3.5.A doesn't, Phase 3.6 can't land without a 3.5.A revision. |
| B4 | `emDialog::private_engine_id()` and `emDialog::root_panel_id()` accessors exist (`pub(crate)`). | **RESOLVED** — Phase 3.5 plan Task 5 Step 5.4 now explicitly lists both accessors (cross-plan edit applied 2026-04-21). | — | At Phase 3.6 Task 3 Step 3.3, verify with `rg -n 'pub\(crate\) fn private_engine_id\|pub\(crate\) fn root_panel_id' crates/emcore/src/emDialog.rs` → 2 matches. If missing, Phase 3.5 execution drifted — add them inline during Phase 3.6 Task 3 before Step 3.3. |
| B5 | `emFileSelectionBox` has no breaking API change between Phase 3 closeout and Phase 3.6 — the accessors this plan uses (`GetSelectedNames`, `GetParentDirectory`, `GetSelectedPath`, `set_selected_name`, etc.) match emFileDialog.rs's current usage. | **VERIFIED** — Phase 3 didn't touch emFileSelectionBox's accessor surface. | — | — |
| B6 | The `on_cycle_ext` closure can safely invoke `od.deregister(ctx.tree, ctx.scheduler, ctx.framework_actions)` — i.e., ctx.tree has the OD's root_panel installed + scheduler has the OD's private_engine_id. | **CONTINGENT** — requires OD to be installed in `ctx.tree` at OD ctor time. Since OD uses `emDialog::new(..., tree, scheduler, ...)` and installs DlgPanel in tree, it's there. deregister will find it. | — | — |
| B7 | The instant-signal-chaining behavior of the scheduler means fsb_file_trigger firing in slice N is visible to the engine's Cycle in slice N (not slice N+1). | **VERIFIED** — emScheduler.rs instant-signal-chaining (Priority re-ascent) processes pending_signals and wakes connected engines within the same slice. | — | — |
| B8 | Test 1 in Task 5 expects exactly 1 slice to suffice for fsb_trigger → finish_signal. | **VERIFIED, with a 2-slice fallback** — if signal chaining requires 2 slices on some timing edge, tests use `h.run_n_slices(2)` which is safe. Step 5.4 already uses 2 slices for the overwrite-path test (OD's engine must finalize first, then outer observes). Test 1 can use 1 OR 2 — either works. | — | Use 2 slices defensively. |
| B9 | The `save_mode_overwrite_*` tests create an actual file on disk in the tmp_dir and verify that `path.exists()` path inside CheckFinish triggers the overwrite-confirmation spawn. | **VERIFIED** — FileDialogTestHarness::new creates tmp_dir; write_test_file places a real file; tests set fsb's parent_directory to tmp_dir + selected_name to that file. CheckFinish's `path.exists()` returns true → overwrite spawn. | — | — |
| B10 | emFileDialog::CheckFinish's new signature (~8 args) doesn't produce clippy `too_many_arguments` noise blocking the gate. | **MITIGATION AVAILABLE** — `#[allow(clippy::too_many_arguments)]` is on the CLAUDE.md whitelist. If triggered, allow. | — | Allow if triggered. |
| B11 | Deleting the Cycle method doesn't leave dangling references elsewhere in the codebase (other crates, examples). | **UNVERIFIED.** | Cross-crate build might fail. | Before Task 4 step 4.5 gate, run `rg -n 'emFileDialog.*\.Cycle\(' crates/` to find any external caller. Expected: 0. If non-zero, migrate before Task 4's gate runs. |

### Drift risks (Phase 3.6-specific)

| Risk | Guardrail |
|---|---|
| Implementor stores overwrite_dialog on emFileDialog (NOT on DlgPanel) and reaches for `Rc<RefCell>` to share with the closure. | Step 3.1's commentary + back-edit to Task 2 place the state on DlgPanel explicitly. The closure reaches via its `&mut DlgPanel` arg — zero shared state. I5f grep catches on commit. |
| Implementor writes a test that calls `fd.Cycle(&mut ctx)` to "save time" on the closure path. | The E024 closure criterion is violated the moment any test calls Cycle. Step 5.7's grep (`rg -n '\.Cycle\(' crates/emcore/src/emFileDialog.rs`) catches in any test body. |
| Implementor preserves `fsb_file_trigger_signal: SignalId` cached field on emFileDialog because the old code had it, even though the closure captures it directly. | Task 4 Step 4.2 explicitly deletes. Verify grep returns 0. |
| Implementor preserves the `DIVERGED:` block at the old Cycle method because "it's descriptive" — but the divergence is resolved now. | Task 4 Step 4.1 explicitly deletes the block alongside the method. |
| Implementor forgets to deregister the OD when it's torn down (memory leak + scheduler leak). | on_cycle_ext closure calls `od.deregister(...)` in both POSITIVE and NEGATIVE branches. Test 2 and 3 both assert `overwrite_finish_signal` is None after teardown. |
| Implementor subscribes outer engine to OD.finish_signal but forgets to disconnect on teardown. | `scheduler.remove_engine(od.private_engine_id)` inside `od.deregister` removes the engine + its connections (emScheduler.rs:330-341 iterates signals and removes the engine from connected_engines). Any stale connection would surface as a spurious wake-up — caught by test 4 (no_signals_one_slice_is_no_op) if it regresses after a prior test in the same process leaks. |
| Implementor changes on_cycle_ext's return value semantics (e.g. always returns true to keep engine awake). | Engine will never sleep → CPU burn. Test harness's `do_n_slices(n)` caps iteration, so tests won't hang, but the cycle count will be wrong. Covered by B7 asserting correct slice behavior. |
| Implementor implements the overwrite result observation by calling `od.GetResult()` through a path that requires `&mut tree` while outer DlgPanel is taken. | Step 3.3's closure uses `ctx.tree.take_behavior(od_root)` directly (different panel id) — sound. Step 5.4's helper `finish_overwrite_dialog_for_test` uses a take-release-call-retake pattern — also sound. |
| Implementor extends `DialogPrivateEngine` instead of using the callback slot (reverting to the D2-rejected "two engine types" shape). | Task 2 writes the slot + extension call in the shared engine. Task 3 populates it. Any new `impl emEngine for FileDialogPrivateEngine` is a drift — clippy wouldn't catch it but a code review would. Explicit invariant I5b (`impl emEngine for DialogPrivateEngine` → exactly 1) catches. |
| Implementor doesn't clear `overwrite_confirmed` when Save-mode's CheckFinish succeeds without prompt. | Pre-port behavior preserved (current emFileDialog.rs:281). Step 3.5's ported CheckFinish includes `self.overwrite_confirmed.clear()` in the no-overwrite-needed path. |
| Subscribe OD.finish_signal without disconnecting when a SECOND Save click on the same text spawns a second OD (edge case). | In the ported CheckFinish, the check `if text != self.overwrite_confirmed` short-circuits a re-spawn if the same text was already confirmed. No second OD. If user changes the name and triggers Save again on a new existing file: first OD should have been torn down by its finish. If not (edge: user clicks X on OD without clicking OK/Cancel → close_signal fires → base engine sets pending=Cancel → finalize → OD.finish_signal fires → outer on_cycle_ext tears down OD). Covered. |

### Gotchas that didn't make it into the plan body

- Between Task 3 (closure installed) and Task 4 (old Cycle deleted), the OLD Cycle-path tests may double-fire logic (closure runs AND old Cycle is callable). This is why Task 3 Step 3.7 allows them to be `#[ignore]`-marked temporarily. Task 4 deletes them. If the interim state breaks the gate, mark-ignore is the right response.
- `emFileDialog::dialog()` and `emFileDialog::dialog_mut()` accessors return the inner `emDialog` — but `emDialog::new` (3.5 ctor) returned a value-type, and ownership moved into emFileDialog. If external callers (tests, emStocksListBox's usage — wait, that's using `emDialog::new` directly, not emFileDialog::new) dereference `fd.dialog()` to mutate, that still works because `dialog_mut()` returns `&mut emDialog`. No issue.
- The `#[cfg(test)]` `finish_overwrite_dialog_for_test` helper uses a 3-phase take/release/retake pattern. It's not the most elegant but it's the shortest correct shape given Rust's tree-as-single-&mut-owner discipline. A cleaner alternative would be to add a `&mut tree + &mut scheduler` argument to a method on DlgPanel that does the work, but that couples DlgPanel to emFileDialog's OD state pattern. Leave as-is unless the helper grows.

### What this plan does NOT do

- It does NOT add new examples or demos showing dialog usage.
- It does NOT refactor emFileSelectionBox's internal API.
- It does NOT touch Phase 4's scope (emRec migration, etc.).
- It does NOT implement a generalized dialog-stacking mechanism — the overwrite-dialog case is a fixed 2-deep stack (outer file dialog + 1 transient overwrite dialog). Deeper stacks would need Phase 3.5.A support (may already be there if top-level-install is general).

---

## Execution handoff

Plan complete. Combined with Phase 3.5 plan, this is the full work package for resolving E024 via the C++-structural emDialog→emWindow port.

Execution: `superpowers:subagent-driven-development` for Phase 3.5 (with explicit pause at 5.1a for the 3.5.A split if the forced prereq materialises), then Phase 3.6. Each phase independently gate-green at merge + tag.
