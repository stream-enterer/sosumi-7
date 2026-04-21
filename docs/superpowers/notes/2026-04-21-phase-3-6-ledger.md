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

- **Prereq Task C — finish_post_show retired to wrapper:** COMPLETE.
  emDialog::finish_post_show is now a thin wrapper over the Prereq A
  App::mutate_dialog_by_id rail. Old inlined dialog_windows → windows →
  take_tree → ... walk deleted. 2 production callers in emFileDialog::Cycle
  unchanged — they die in Task 4. 2 unit tests unchanged.
  Gate green — nextest 2512/0/9.

- **Task 2 — on_cycle_ext callback slot:** COMPLETE. Added
  `pub(crate) type DialogCycleExt = Box<dyn FnMut(&mut DlgPanel,
  &mut EngineCtx) -> bool>` and `DlgPanel.on_cycle_ext:
  Option<DialogCycleExt>` field. DialogPrivateEngine::Cycle
  takes-calls-puts the extension after the base cycle body using the
  swap-out pattern (avoids double &mut dlg_panel borrow). One unit
  test asserts the extension is called exactly once per Cycle slice.
  Extension is None today — Task 3 installs it for emFileDialog.
  Gate green — nextest 2513/0/9.

- **Task 3 — emFileDialog reshape (keystone):** COMPLETE — commit
  ab336382. emFileDialog now composes a 3.5 emDialog handle and
  installs emFileSelectionBox as a child of the dialog's content
  panel (lazy-created via new emDialog::GetContentPanel).

  AddWakeUpSignal port (emFileDialog.cpp:41): new
  PendingTopLevel.wake_up_signals rail + emDialog::add_pre_show_wake_up_signal
  queue pre-show subscriptions; both installers (prod + headless) drain
  the queue immediately after register_engine + connect(close_signal).
  Both installers also stamp DlgPanel.private_engine_id for the
  post-show reach pattern.

  File-dialog Cycle logic ported into the on_cycle_ext closure on
  DlgPanel, running after DialogPrivateEngine::Cycle's base body. The
  closure observes fsb.file_trigger_signal → pending_result=Ok and
  tears down a pending overwrite dialog on its finish_signal via
  pending_actions close_dialog_by_id. Validation-funnel P3 divergence
  (authorized (f)): file-trigger-path and button-click-OK-path both
  skip CheckFinish re-entry because DialogCheckFinishCb's signature
  lacks the ctx needed for OD spawn — deferred to a later phase that
  widens the check-finish callback. OD-result-driven overwrite_confirmed
  promotion also deferred (outer stays open on OD-finish).

  Infrastructure added:
    * emDialog::GetContentPanel, root_panel_id, pending_mut,
      add_pre_show_wake_up_signal.
    * DlgPanel.overwrite_dialog + overwrite_asked + private_engine_id
      (DIVERGED markers; placement on DlgPanel avoids Rc<RefCell<Option<emDialog>>>
      per CLAUDE.md Do-NOT).
    * PanelBehavior::as_file_selection_box_mut + emFileSelectionBox
      override.

  Task 4 deletions absorbed into Task 3: pub fn Cycle(&mut self, ctx)
  deleted along with fsb_file_trigger_signal field, overwrite_result
  Rc<Cell>, on-struct overwrite_dialog/overwrite_asked (migrated to
  DlgPanel), test_force_overwrite_result helper, and 5 Cycle-path
  tests (the 4 named in the controller's adaptation list plus
  save_existing_file_triggers_overwrite_dialog_and_confirms, which
  depended on test_force_overwrite_result).

  fsb accessors migrated to with_fsb / with_fsb_mut take/put pattern
  (~18 methods; signatures grew to &mut self). Tests exercise
  pre-show path exclusively — acceptable at Task 3 scope; post-show
  routing deferred.

  Verification:
    * pub fn Cycle.*PanelCtx in emFileDialog.rs = 0
    * test_force_overwrite_result in crates/ = 0
    * fsb_file_trigger_signal in emFileDialog.rs = 0
    * wake_up_signals in emGUIFramework.rs ≥ 4 (8 hits)
    * No new #[allow], no new unsafe, no new Rc<RefCell>/Arc/Mutex/Cow/Any
      in production code.

  Gate green — nextest 2508/0/9 (was 2513; 5 tests deleted per plan).

- **Task 3 fix — OD-POSITIVE livelock closed:** COMPLETE. Moved
  `overwrite_confirmed` from `emFileDialog` struct to `DlgPanel`
  alongside `overwrite_dialog` / `overwrite_asked`, unblocking the
  `on_cycle_ext` closure (which has only `&mut DlgPanel + &mut EngineCtx`)
  to perform POSITIVE promotion. Closure now reads OD's
  `finalized_result` via a deferred `pending_actions` closure
  (using `App::mutate_dialog_by_id` on both OD and outer DlgPanel
  since OD lives in a separate window's PanelTree — SlotMap keys
  cannot cross trees). On POSITIVE: promotes `overwrite_asked →
  overwrite_confirmed` and sets `pending_result = Ok` on outer so the
  outer dialog finalizes (matches C++ emFileDialog.cpp:93-96). On
  NEGATIVE / Custom / None: outer stays open, OverwriteAsked already
  cleared inline via `mem::take`, OD torn down via
  `close_dialog_by_id` (matches C++ emFileDialog.cpp:98-101).
  `CheckFinish` Save-mode path reads/writes `overwrite_confirmed` via
  the same pre-show tree-take pattern used for `overwrite_dialog`;
  post-show write routed via `mutate_dialog_by_id` (read conservatively
  empty — post-show read has no sync path; re-prompt then confirms,
  which is correct albeit with an extra OD flash — Task 5 end-to-end).
  Added `PanelBehavior::as_dlg_panel` (immutable downcast) +
  `DlgPanel` override. No new `#[allow]`, `unsafe`, or `Rc<RefCell>`.
  Gate green — nextest 2508/0/9, clippy clean.

- **Task 4 — delete emFileDialog::Cycle + vestigial helpers:** COMPLETE
  (no-op). All Task-4 deletions were absorbed into Task 3 by controller
  authorization (adaptation (e)): `pub fn Cycle(&mut self, ctx: &mut
  PanelCtx<'_>)` + its `DIVERGED:` block, `fsb_file_trigger_signal`
  cached field, `test_force_overwrite_result` test helper, and the 4
  Cycle-path tests are all gone. Verified:
    - `rg -n 'pub fn Cycle.*PanelCtx' crates/emcore/src/emFileDialog.rs` → 0
    - `rg -n 'test_force_overwrite_result' crates/` → 0
    - `rg -n 'fsb_file_trigger_signal' crates/emcore/src/emFileDialog.rs` → 0
  Task 4's exit condition (all three greps = 0) is met by the Task 3
  landing. No code commit required — this ledger entry is the Task 4
  closeout on record.

- **Task 5 — E024 closure tests:** commit 9eb7ff03. Four scheduler-driven
  tests in emFileDialog.rs::e024_closure_tests: (1)
  fsb.file_trigger → outer finalized_result=Ok via 2 DoTimeSlice slices;
  (2) OD.finish_signal → on_cycle_ext pushes pending_action, OD torn
  down from DlgPanel, overwrite_asked cleared (Task 3 fix's observation
  path under scheduler control); (3) OD NEGATIVE path → pending_action
  drains, outer stays open, promotion NOT applied, outer pending_result
  NOT set; (4) no-signals 1-slice no-op baseline. NO test invokes any
  Cycle method — verified via `rg -n '\.Cycle\('
  crates/emcore/src/emFileDialog.rs` == 0.

  Production change revealed by the mechanical arbiter: `on_cycle_ext`
  now returns `true` when it mutates `pending_result` or pushes a
  pending_action, because on_cycle_ext runs AFTER the base Cycle body
  and any state it sets requires another Cycle to finalize. Without
  this the engine sleeps with pending_result orphaned. C++
  emFileDialog.cpp:82-84 doesn't have this gap because Finish(POSITIVE)
  finalizes in-cycle via FinishState=1.

  Added: `emFileDialog::file_trigger_signal()` pub accessor + cached
  `fsb_trigger_sig` field (prior: only reachable via pending-tree
  walk). FileDialogTestHarness inlined in the test module (static UID
  counter for parallel-safe tmp dirs; Drop cleans up engines via
  close_dialog_by_id + clear_pending_for_tests + rm tmp_dir).

  Deferred: full scheduler-driven POSITIVE livelock regression test
  requires a second test-only WindowId distinct from
  WindowId::dummy(). Follow-up to parameterize
  install_pending_top_level_headless with a caller-supplied id.
  Documented inline at the Test 2 placeholder in
  `e024_closure_tests`.

  Gate green — nextest 2512/0/9, clippy clean, no new `#[allow]`,
  `unsafe`, or `Rc<RefCell>`.
