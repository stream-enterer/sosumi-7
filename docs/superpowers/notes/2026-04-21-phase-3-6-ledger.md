# Phase 3.6 — emFileDialog rides 3.5; E024 closure — Ledger

**Started:** 2026-04-21
**Branch:** port-rewrite/phase-3-6-emfiledialog-e024 (off `port-rewrite-phase-3-5-complete`, SHA `74bb14ce`)
**Baseline:** nextest 2510/0/9, clippy clean, goldens 237/6 (pre-existing).
**Plan:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-emfiledialog-e024.md
**Spec:** docs/superpowers/specs/2026-04-21-phase-3-5-deferred-dialog-construction-design.md §Deferred to Phase 3.6
**JSON entry to close:** E024 (open → resolved-phase-3-6).

## Baseline audit (pre-dispatch)

Confirmed current state of `main` post-3.5-merge against Phase 3.6 plan expectations:

### Prereq infrastructure present from 3.5
- `emGUIFramework.rs`: `DialogId`, `dialog_windows: HashMap<DialogId, WindowId>`, `dialog_window_mut()`, `close_dialog_by_id(did)`, `allocate_dialog_id()`, `DialogWindow<'_>` all present. No `mutate_dialog_by_id` yet — Prereq Task A adds it.
- `emScheduler.rs`: `engines_for_scope(scope) -> Vec<EngineId>` + `wake_up(id)` present.
- `emDialog.rs`: `dialog_id`, `root_panel_id`, `ConstructCtx::pending_actions()` all present.
- `emDialog::finish_post_show` (emDialog.rs:318) manually inlines `dialog_windows → windows → take_tree → take_behavior → apply → put → put`. Prereq Task C folds that walk into `App::mutate_dialog_by_id`.

### Prereq Task B call-site survey (pre-show-only mutators)
- `SetRootTitle` callers: 4 in emDialog.rs tests (lines 1299, 1316, two `should_panic` post-show tests).
- `set_button_label_for_result` callers: 4 in emDialog.rs tests (1325, 1348, two `should_panic`).
- `EnableAutoDeletion` callers: 4 in emDialog.rs tests (1356, 1373, two `should_panic`).
- No production callers outside tests — all ctx threading lives inside the test-only call sites.

### Prereq Task C scope
- `finish_post_show` production callers: `emFileDialog.rs:359, 372` (the two overwrite-path Cycle branches, to be deleted in Phase 3.6 Task 4).
- Test callers: `emDialog.rs:1851, 1955` (the two `finish_post_show_*` unit tests).
- Decision point: retire or thin-wrap — defer to Task C design.

### Phase 3.6 Task 4 deletion targets — all present
- `emFileDialog.rs:316` — `pub fn Cycle(&mut self, ctx: &mut PanelCtx<'_>) -> bool`. Deleted at Task 4.
- `emFileDialog.rs:64, 82, 94` — `fsb_file_trigger_signal` cached field. Deleted at Task 4.
- `emFileDialog.rs:425` — `test_force_overwrite_result` test helper. Deleted at Task 4.
- Cycle-path tests at `:639, :685, :766` all still rely on these helpers.

### Stale line-number drift in plan
Plan written pre-3.5-merge; a few line references have drifted:
- Plan Task 3.1 cites emFileDialog.rs:40-58 for struct; now closer to the beginning of the file.
- Plan Task 4.3 cites emFileDialog.rs:401-416 for `test_force_overwrite_result`; actual at :425.
- Plan Task 4.1 cites emFileDialog.rs:322-371 for Cycle; actual at :316.
Implementers must grep rather than trust line numbers.

### Invariants pre-dispatch
- I5b (`impl emEngine for DialogPrivateEngine`) — confirmed 1 match.
- I5d (`pub fn Cycle.*PanelCtx` in emDialog.rs) — confirmed 0 matches.
- I5e (`silent_cancel` in crates/) — confirmed 0 matches.
- I5h (E024.status) — confirmed still `open`.

## Bootstrap decisions

See plan §"Bootstrap decisions" (B3.6a–B3.6d).

## Task log

- **Prereq Task A — App::mutate_dialog_by_id:** COMPLETE. Added `pub fn
  mutate_dialog_by_id(&mut self, did: DialogId, f: impl FnOnce(&mut DlgPanel))`
  that walks (wid, root_panel_id) → take_tree → take_behavior → apply closure
  → put_behavior → put_tree → wake all engines at Toplevel(wid). Root-panel id
  bookkeeping via `dialog_roots: HashMap<DialogId, PanelId>` (parallel to
  `dialog_windows`) populated in both `install_pending_top_level` and
  `install_pending_top_level_headless`, cleared in `close_dialog_by_id`.
  Consolidates the inlined walk that `emDialog::finish_post_show` currently
  duplicates; `finish_post_show` not yet retired — Prereq C does that. Two
  unit tests added: `mutate_dialog_by_id_applies_closure_and_wakes_engines`
  (title mutation lands + engine still registered after wake) and
  `mutate_dialog_by_id_unknown_id_is_noop` (silent no-op guard).
  Gate green — nextest 2512/0/9.

- **Prereq Task B — post-show mutator routing:** COMPLETE. SetRootTitle,
  EnableAutoDeletion, set_button_label_for_result gain a ctx parameter
  and branch on self.pending.is_some(): pre-show stays direct via
  with_dlg_panel_mut; post-show routes through pending_actions →
  App::mutate_dialog_by_id. For set_button_label_for_result, post-show
  queues a (DialogResult, String) pair on DlgPanel::pending_label_updates
  via mutate_dialog_by_id; DialogPrivateEngine::Cycle step 0.5 drains the
  queue by walking DlgButton children while holding simultaneous DlgPanel
  + tree access. Three #[should_panic] post-show tests replaced by
  positive routing tests (title/flag read back via materialized tree;
  button label read back after one DoTimeSlice). AddCustomButton /
  set_on_finish / set_on_check_finish untouched per plan. Gate green —
  nextest 2512/0/9.

- **Prereq Task B rework — single-rail post-show:** COMPLETE. Widened
  `App::mutate_dialog_by_id` closure to `FnOnce(&mut DlgPanel, &mut PanelTree)`.
  Dropped DlgPanel::pending_label_updates queue + DialogPrivateEngine::Cycle
  step 0.5 drain. `set_button_label_for_result` post-show walks DlgButton
  children inline through the tree arg. Single post-show rail restored.
  Gate green — nextest 2512/0/9.
