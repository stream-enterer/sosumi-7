# Phase 3.6 — emFileDialog + E024 closure — Closeout

**Branch:** port-rewrite/phase-3-6-emfiledialog-e024
**Status:** COMPLETE
**E024 closed at:** 9eb7ff03 (phase-3.6 task 5)

## Commit range

```
git log --oneline port-rewrite-phase-3-5-complete..HEAD
```

```
7dd820ca phase-3.6 task 5 cleanup: doc comments on fsb_trigger_sig cache + on_cycle_ext stay=true; raw-material entry for deferred POSITIVE end-to-end
cffd213b phase-3.6 task 5 ledger: E024 closure tests complete
9eb7ff03 phase-3.6 task 5: E024 closure tests — scheduler-driven, zero Cycle calls
55a3f53a phase-3.6 task 4 ledger: no-op — deletions absorbed into Task 3
d99cedea phase-3.6 task 3 fix: OD POSITIVE promotes overwrite_confirmed + pending_result = Ok; closes livelock
0e00a10f phase-3.6 task 3 ledger: emFileDialog reshape complete
ab336382 phase-3.6 task 3: emFileDialog reshaped — composes 3.5 emDialog + Fsb child panel
6eb118d0 phase-3.6 task 2 ledger: on_cycle_ext landed; ready for Task 3
eec497e0 phase-3.6 task 2: DlgPanel gains on_cycle_ext slot; DialogPrivateEngine::Cycle calls it post-base
845aefc3 phase-3.6 prereq C ledger: finish_post_show retired to wrapper complete
0aa09b5f phase-3.6 prereq C: retire finish_post_show's inlined walk in favor of mutate_dialog_by_id
ef6f70dd phase-3.6 prereq B rework: widen mutate_dialog_by_id to (DlgPanel, &mut PanelTree); drop queue
f001fcd3 phase-3.6 prereq B: route post-show SetRootTitle/EnableAutoDeletion/set_button_label_for_result via pending_actions
0af67f08 phase-3.6 prereq A: App::mutate_dialog_by_id — closure mutator over (wid, root_panel)
763d4bca phase-3.6 task 1: entry-gate verified; baseline-audit ledger created
```

## Summary

Phase 3.6 reshaped `emFileDialog` from a plain owned struct with a caller-invoked
`Cycle` into a composition over the 3.5-ported `emDialog` + `emFileSelectionBox`
installed as a child panel under `content_panel`. Wake-up-signal subscription via
`scheduler.connect(fsb.file_trigger_signal, dialog.private_engine_id())` ports
C++ `emFileDialog.cpp:41` `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`. The
`on_cycle_ext` callback on `DlgPanel` ports `emFileDialog.cpp:80-106` `Cycle`
body, running as a post-amble to the base `DialogPrivateEngine::Cycle` per
the single-engine design decision (D2).

The transient overwrite-confirmation `emDialog` (OD) is a separate top-level dialog
(its own emWindow + DialogPrivateEngine); the outer emFileDialog's engine subscribes
to its `finish_signal`. `on_cycle_ext` tears down the OD when its finish is observed
and promotes `overwrite_asked → overwrite_confirmed` on POSITIVE.

E024 status: resolved-phase-3-6. The mechanical arbiter — 4 scheduler-driven tests
at `emFileDialog.rs::e024_closure_tests` — asserts that signals fire into the
scheduler and assertions pass with zero caller `Cycle` invocation.

## Invariant table (Task 6 Step 6.1)

| ID | Check | Result |
|----|-------|--------|
| I5a | `window_id:` in emDialog.rs | PASS (≥1 hit) |
| I5b | `impl.*emEngine.*for DialogPrivateEngine` | PASS (1 hit, line 816) |
| I5c | `scheduler.connect` (wiring in emGUIFramework: close_signal + wake_up_signals drain) | PASS (4 hits in emGUIFramework.rs: 590, 597, 688, 692) |
| I5d | `pub fn Cycle.*PanelCtx` in emDialog.rs + emFileDialog.rs | PASS (0 matches) |
| I5e | `silent_cancel` in crates/ (live code) | PASS (1 doc-comment ref in emGUIFramework.rs only — no live use) |
| I5f | `Rc<RefCell<` in emDialog.rs + emFileDialog.rs | PASS (all hits are comments or test-only; 0 new production uses) |
| I5g | `unsafe {` in emDialog.rs + emFileDialog.rs | PASS (0 matches — no unsafe blocks) |
| I5h | E024.status before flip | PASS (was open; flipped to resolved-phase-3-6 in Step 6.2) |
| I5i | Goldens preserved | PASS (237/6 — pre-existing baseline) |
| I5j | Nextest delta | PASS (2512/0/9) |
| E024.closure | `.Cycle(` in emFileDialog.rs | PASS (0 matches) |
| wake_up_signals | refs in emGUIFramework.rs | PASS (8 hits — ≥4) |

## JSON entries closed

- **E024** — `resolved-phase-3-6` (resolution_commit `9eb7ff03`, phase_3_progress cleared)

## JSON entries opened

- **E040** — `open` — deferred POSITIVE overwrite-confirm end-to-end test. Blocked on
  parameterizing `install_pending_top_level_headless` with a caller-supplied `WindowId`
  to avoid collision when outer FD + OD both install headlessly. Tracked for follow-up.

## Notable divergences introduced (Rust-specific)

- **`PendingTopLevel.wake_up_signals` rail** — pre-show signal subscriptions queued on
  the pending struct, drained into `scheduler.connect` at both install paths (prod +
  headless). Ports `AddWakeUpSignal` deferred subscription without a live
  `emScheduler&` at construction time.

- **`on_cycle_ext` callback slot on `DlgPanel`** — ports C++ `emFileDialog::Cycle`
  post-base logic into Rust via single-engine + callback instead of subclass.
  `on_cycle_ext` is `Option<Box<dyn FnMut(&mut DlgPanel, &mut EngineCtx) -> bool>>`;
  installed by `emFileDialog::new`.

- **`DlgPanel.overwrite_dialog` / `overwrite_asked` / `overwrite_confirmed` / `private_engine_id`
  fields** — moved from `emFileDialog` struct so the `'static FnMut` closure can reach
  them without `Rc<RefCell>` (CLAUDE.md Do-NOT).

- **`on_cycle_ext` returns `true` on mutation** — forces re-wake because base body runs
  pre-ext; any state set by ext requires another Cycle to finalize. C++ does not have
  this gap because `Finish(POSITIVE)` finalizes in-cycle via `FinishState=1`.

- **File-trigger-path validation deferral** — CheckFinish not re-entered on file_trigger
  (authorized P3 divergence). `DialogCheckFinishCb` signature lacks ctx needed for OD
  spawn; deferred to a later phase that widens the check-finish callback.

- **`fsb_trigger_sig` cached `SignalId` field** — test-only exposure; no C++ accessor.
  Avoids a pending-tree walk in test harness.

## Gate (Step 6.5)

- `cargo clippy --all-targets --all-features -- -D warnings`: clean
- `cargo-nextest ntr`: 2512/0/9
- `cargo test --test golden -- --test-threads=1`: 237 passed / 6 failed (pre-existing baseline preserved)

## Next phase

Phase 4 (emRec migration) per `docs/superpowers/plans/2026-04-19-port-rewrite-phase-4*.md`.
E024 was the last open entry in the scope of the Phase 3 + 3.5 + 3.6 chain.
