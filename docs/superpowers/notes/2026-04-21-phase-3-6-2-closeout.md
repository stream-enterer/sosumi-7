# Phase 3.6.2 Closeout — E040 + E041

Branch: `port-rewrite/phase-3-6-2-e040-e041` (off main `d0d111c8`).

## Scope

Close two open Phase-3.6 divergences as a single small phase:
- **E040** — Phase 3.6 deferred POSITIVE overwrite-confirm end-to-end test. Blocked on `winit::window::WindowId::dummy()` being the only stable headless id.
- **E041** — Phase 3.6.1 `emFileDialog::CheckFinish` / `run_file_dialog_check_finish` duplication.

## Scope decision (E040)

The Phase 3.6.1 follow-up memory proposed a 181-site, 18-file refactor (introduce `WindowKey { Real(WindowId), Headless(u64) }` and rekey `App.windows`, `PanelScope::Toplevel`, `EngineCtx.windows`, etc.). Pre-implementation audit:

- 181 `WindowId` mentions across 18 files under `crates/emcore/src/`.
- Phase 4a–4d (emRec / emCoreConfig) plans: zero mentions of dialog / popup / multi-window / WindowId.
- Phase 5 (async-lint): zero mentions.
- Ownership spec §3.7 `pending_popups: HashMap<WindowId, emWindow>` handwaves "framework allocates WindowId" — same blocker, popup-multiplicity tests not queued.

Conclusion: "A1 unlocks stacked-modal tests broadly" is speculative; no queued phase cashes in. One POSITIVE test covers 8 lines of linear assignment (emFileDialog.rs POSITIVE arm). Test 2b (scheduler-driven on_cycle_ext observation) + the NEGATIVE drain test already cover 90% of the interesting paths.

**Chosen path (option 2):** narrow test-only sidecar. 15 LOC of infra, zero production path changes.

## Changes

### E040 — sidecar (emGUIFramework.rs + emFileDialog.rs)

- New `#[cfg(test)] pub(crate) App::headless_dialog_results: HashMap<DialogId, DialogResult>`.
- New `pub(crate) App::read_dialog_finalized_result(DialogId) -> Option<DialogResult>`: tests path consults sidecar first; production path (`#[cfg(not(test))]`) walks `dialog_windows → windows → tree` via `mutate_dialog_by_id` as before.
- `emFileDialog.rs` OD POSITIVE pending-action closure now calls `app.read_dialog_finalized_result(od_did)` instead of inline `mutate_dialog_by_id`.
- New test `save_mode_overwrite_positive_finishes_outer_dialog_via_scheduler` in `e024_closure_tests`:
  - Build Save-mode FD on pre-existing `doc.txt`.
  - Drive `CheckFinish(Ok)` → OD parked on outer DlgPanel; capture `od_did`, `od_finish_sig`, `asked_text`.
  - Install outer headless. Pre-seed `headless_dialog_results.insert(od_did, DialogResult::Ok)`.
  - Fire OD.finish_signal → 1 slice → on_cycle_ext pushes pending_action → drain → POSITIVE branch.
  - Assert: `overwrite_confirmed == asked_text`, `pending_result == Some(Ok)`, `overwrite_dialog.is_none()`.
  - 1 more slice → outer engine commits `pending_result → finalized_result`; assert `finalized_result == Some(Ok)`.

Production path is identical to before; only the tree-reach that reads `finalized_result` now funnels through the helper. The sidecar is compiled out entirely in release.

### E041 — CheckFinish dedup (emFileDialog.rs)

Factored three shared helpers:

- `check_finish_dir_and_open(mode, dir_allowed, names, parent) -> Result<(), FileDialogCheckResult>` — C++ emFileDialog.cpp:119-163.
- `enum SaveOverwriteOutcome { NoConflict, AlreadyConfirmed, NeedOverwriteDialog { paths, text } }` + `classify_save_overwrite(names, parent, overwrite_confirmed)` — C++ :165-185.
- `build_overwrite_dialog<C: ConstructCtx>(ctx, look) -> emDialog` — C++ :186-191.

Both `emFileDialog::CheckFinish` (pre-show, public method — test-only caller surface) and `run_file_dialog_check_finish` (post-show, free fn — production callback path) dispatch through these helpers. The remaining per-wrapper code is only the pre/post-show tree-access shape plus OD subscription routing (`self.dialog.add_pre_show_wake_up_signal` vs `ctx.scheduler.connect`) and OD parking (pre-show tree-take on `self.dialog.pending.window` vs direct `outer_dlg` mutation).

**Behavioral convergence correction:** prior `CheckFinish` cleared `overwrite_confirmed` even when `text == confirmed` (Rust-only over-zealous clear). New shared classifier returns `AlreadyConfirmed` on equality and the CheckFinish wrapper maps it to `Allow` without clearing — matches C++ :185 `if (text==OverwriteConfirmed) return;`.

## Gate

- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — clean.
- `cargo nextest run --workspace` — 2514 passed / 9 skipped. Previous baseline 2513; +1 is the new E040 POSITIVE test.
- Goldens: 237 passed / 6 failed — identical to Phase 3.6.1 baseline; six failures pre-existing.

## Invariant sweep

- `rg -n 'impl emEngine for DialogPrivateEngine'` → 1.
- `rg -n 'pub fn Cycle.*PanelCtx' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs` → 0.
- `rg -n 'DIVERGED:.*P3' crates/emcore/src/emFileDialog.rs` → 0.
- E040.status == resolved-phase-3-6-2, E041.status == resolved-phase-3-6-2.
- No new `#[allow]` outside whitelist. No new `Rc<RefCell>` / `Arc` / `Mutex` / `Cow` / `Any`. No new `unsafe`.

## Memory

Delete `memory/project_phase36_followup_e040.md` — resolved.
