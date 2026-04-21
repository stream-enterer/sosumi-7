# Phase 3.5 / 3.6 — `emDialog → emWindow` (C++-structural port) + `emFileDialog` E024 closure — Brainstorm Output

**Created:** 2026-04-21
**Produced by:** brainstorm session per the handoff at `docs/superpowers/notes/2026-04-20-phase-3-5-emdialog-engine-brainstorm-handoff.md`.
**Supersedes handoff framing:** the handoff labelled this "emDialog → emEngine". Research showed that framing is off: C++ `emDialog` inherits `emWindow` (not `emEngine`) and owns a nested `PrivateEngineClass : public emEngine` via composition. The correct port target is `emDialog : public emWindow` with a `DialogPrivateEngine` member. E024 closure is a consequence of that port, not its own target.

---

## 1. Decision

**Phase 3.5 stands as a phase (not a fold-in), and expands into a phase chain — 3.5 and 3.6 — both of which must land before E024 can be honestly marked `resolved`.** Scope ceiling is governed by the CLAUDE.md authority order (C++ source > golden tests > Rust idiom) and the user-affirmed principle *"allow for as many sub-phases as needed to fully realise the C++ port"* — scope is not compressed to fit an arbitrary labelling.

Rationale in one paragraph: C++ `emDialog` is an `emWindow`-derived view whose Cycle/wake-up surface comes from a nested `PrivateEngineClass : emEngine` subscribing to `GetCloseSignal()` via `AddWakeUpSignal`. The Rust port today has `emDialog` as a plain owned struct with a caller-driven `Cycle` method — a structural divergence on both counts. Research (see §3) confirms every prerequisite for a faithful port already exists in Rust (emWindow, popup/modal window flags, per-panel PanelCycleEngine providing the wake-up rail, rich PanelBehavior ecosystem including emLinearLayout + button-panel wrappers). No forced divergence was surfaced. Therefore γ (full structural port) is achievable without `Rc<RefCell>`/unsafe/Any/Weak, and is the correct answer under the port ideology. Splitting into 3.5 (emDialog-as-emWindow) and 3.6 (emFileDialog consumes 3.5 + E024 closes) keeps each phase singly-focused and gate-verifiable.

## 2. Scope

### Phase 3.5 — `emDialog` as `emWindow`-derived

**In scope:**

- Reshape `emDialog` from an owned plain struct into a type modelled on C++ `class emDialog : public emWindow`:
  - Owns an `emWindow` (Rust composition since Rust has no inheritance), constructed via the existing `emWindow::new_popup_pending(parent_context, root_panel, close_signal, ...)` rail.
  - Root panel is a new `DlgPanel` PanelBehavior (C++ `emDialog::DlgPanel : emBorder`), installed as the window's root at construction.
  - Content area is an `emLinearLayout` (C++ `GetContentPanel()` returns `emLinearLayout*`).
  - Button row is a sibling `emLinearLayout` (C++ `ButtonsPanel`).
  - Dialog buttons (`AddPositiveButton`/`AddNegativeButton`/`AddCustomButton`/`AddOKButton`/`AddCancelButton`) install `DlgButton` PanelBehaviors — a new wrapper type following the `ButtonPanel`-wrapper precedent in `emColorFieldFieldPanel.rs`.
- Introduce `DialogPrivateEngine` struct + `impl emEngine` inside `emDialog.rs` (private module), named to match C++ `PrivateEngineClass`:
  - Registered with the scheduler at dialog construction; `EngineId` stored on `emDialog`.
  - `scheduler.connect(close_signal, private_engine_id)` replicates C++ `PrivateEngine.AddWakeUpSignal(GetCloseSignal())`.
  - `SetEnginePriority(High)` parity via `scheduler.set_engine_priority(private_engine_id, Priority::High)` (C++: `HIGH_PRIORITY`).
  - Cycle body drives `DlgPanel` behavior state (result, finish_state, auto-delete countdown) via the existing `tree.take_behavior(root_panel_id)` take/put pattern — the same idiom `PanelCycleEngine` uses.
- Port remaining `emDialog` surface beat-for-beat: `Finish(result) -> bool` (with `CheckFinish(result)` hook), `Finished(result)` virtual-equivalent (Rust: trait-method or callback; shape matches existing widget callback idiom), `EnableAutoDeletion` / `IsAutoDeletionEnabled` (three-time-slice self-deletion via DialogPrivateEngine), `GetFinishSignal` (already on today's `emDialog`).
- Delete the caller-invoked `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool` on `emDialog` (today's Rust surface) entirely — the dialog's Cycle now belongs to `DialogPrivateEngine`, scheduler-dispatched.
- Migrate `emDialog::ShowMessage` to the new construction rail.
- Rewrite dialog unit tests to the new shape: install dialog (produces an `emDialog` owning an `emWindow`), fire signals into scheduler, tick `DoTimeSlice`, assert state transitions. No test calls `Cycle` directly.

**Out of scope (Phase 3.5):**

- `emFileDialog` — consumes 3.5 in 3.6.
- Any change to `emFileSelectionBox` — already a PanelBehavior.
- Porting additional `emWindow` features not needed by `emDialog`.

### Phase 3.6 — `emFileDialog` rides 3.5 + E024 closes

**In scope:**

- Reshape `emFileDialog` from an owned plain struct that composes `emDialog` + `emFileSelectionBox` into a type modelled on C++ `class emFileDialog : public emDialog`:
  - Composition-over-inheritance per Rust idiom + File and Name Correspondence: `emFileDialog` owns an `emDialog` (from 3.5) and installs `emFileSelectionBox` as a child of the content panel. `DIVERGED:` comment documents the Rust inheritance→composition adaptation (this is an idiom adaptation, not an observable divergence — same behavior).
- Subscribe the `emDialog` `DialogPrivateEngine` (or, if Cycle-body specialisation requires it, a `FileDialogPrivateEngine` that replaces/extends it) to `fsb.file_trigger_signal` via `scheduler.connect` — direct port of C++ `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`.
- Port `emFileDialog::Cycle` (emFileDialog.cpp:80-106) into the private engine's Cycle body. Logic:
  1. If `is_signaled(fsb_file_trigger_signal)`: `Finish(POSITIVE)`.
  2. If `overwrite_dialog.is_some() && is_signaled(od.finish_signal)`: switch on `od.result` — POSITIVE → `OverwriteConfirmed = OverwriteAsked; clear; drop(overwrite_dialog); Finish(POSITIVE)`; NEGATIVE → `clear; drop(overwrite_dialog)`.
- `CheckFinish` continues to create the transient overwrite-confirmation `emDialog` (now a real 3.5-ported emDialog with its own emWindow); on creation, `scheduler.connect(overwrite_dialog.finish_signal, file_dialog_private_engine_id)` subscribes the outer engine to the transient's finish signal.
- Delete the caller-invoked `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool` on `emFileDialog` and its `fsb_file_trigger_signal` cached field (the engine reads via the live `fsb`).
- Delete `test_force_overwrite_result` — vestigial once the overwrite dialog is driven by real Input through its own panel tree.
- Migrate the 4 `emStocksListBox.rs` call sites (`DeleteStocks`, `CutStocks`, `PasteStocks`, `SetInterest`) that create confirmation dialogs. Today they create `emDialog::new(...)` and call `.Finish()` / `.silent_cancel()` imperatively. After 3.5/3.6, they install a dialog into a parent panel context and connect a caller-owned engine (or a deferred-action handler) to `finish_signal`.

**Out of scope (Phase 3.6):**

- Phase 4 scope (emRec, emCoreConfig migration, etc. — per the existing Phase 4a/4b/4c/4d plans).

### Blast-radius estimate

| Metric | Expected delta (both phases) |
|---|---|
| Files touched | ~15 core + ~8 tests |
| LOC delta | +800 / −350 (rough; 3.5 adds DlgPanel/DlgButton/DialogPrivateEngine; 3.6 adds engine Cycle body, deletes caller-Cycle) |
| nextest delta | +~10 (new regression tests for scheduler-driven dispatch); un-ignores of any dialog tests deferred today |
| golden tests | 237/6 preserved (no paint-path change) |
| rc_refcell_total | unchanged or down (no new Rc<RefCell>) |
| DIVERGED count | net down (delete existing DIVERGED at `emFileDialog::Cycle`; add smaller one for Rust-composition-instead-of-C++-inheritance, at the `emFileDialog` struct) |

## 3. Prerequisites

Confirmed present, no prerequisite sub-phase required **before** 3.5:

- **emWindow close_signal + popup/modal flags + lazy materialise.** `emWindow.rs:26-34` (`WindowFlags`), `:118` (`close_signal: SignalId`), `:320-375` (`new_popup_pending`), `:1535-1540` ("Matches C++ emWindow::SignalClosing: Signal(CloseSignal)").
- **Per-panel `PanelCycleEngine` with `EngineId`, engine-subscription via `scheduler.connect`, wake-on-signal via `process_pending_signals`.** `emPanelCycleEngine.rs:31-40` (per-panel engine), `emScheduler.rs:279-301` (`connect` with refcount), `emScheduler.rs:654-680` (`process_pending_signals` walks `connected_engines`, calls `wake_up_engine`).
- **`scheduler.register_engine(Box<dyn emEngine>, priority, tree_location) -> EngineId`** for standalone engines. `emScheduler.rs:313-327`. `TreeLocation::Outer | SubView { .. }` supports popup subtrees.
- **PanelBehavior ecosystem adequate for DlgPanel + DlgButton + ContentPanel/ButtonsPanel.** 38 `impl PanelBehavior for` sites. `emLinearLayout` is a PanelBehavior (`emLinearLayout.rs:486`). `ButtonPanel`/`LabelPanel`/etc. wrappers in `emColorFieldFieldPanel.rs` establish the widget-as-PanelBehavior-wrapper precedent.
- **`emBorder` is a compositional helper** used by every widget border today — DlgPanel-as-PanelBehavior wrapping `emBorder` follows the same pattern (see e.g. `emListBox.rs`).

Audit sub-tasks (one-commit each, inside Phase 3.5 Task 1) — confirm no gap; fill only what's actually missing:

- **3.5 Task 1a:** `VF_POPUP_ZOOM` / `VF_ROOT_SAME_TALLNESS` parity — C++ emDialog default ViewFlags. Check `emView` `ViewFlags` coverage. If a flag is missing that the dialog ctor needs, port it (small, localised).
- **3.5 Task 1b:** AutoDelete lifecycle (C++ emDialog deletes itself 3 slices after close). Path: `DialogPrivateEngine` counts slices post-close-signal, emits a `DeferredAction::DeleteDialog(window_id)` via `framework_actions`. Confirm `DeferredAction` enum extensibility (it already carries window-teardown variants used by popup-zoom).
- **3.5 Task 1c:** `Finished(int result)` virtual hook — C++ `emDialog::Finished` is a protected virtual, default does nothing, subclasses override. Rust shape: a `on_finished: Option<WidgetCallbackRef<DialogResult>>` field on `emDialog` (matching the existing `on_finish` / `on_check_finish` callback idiom introduced in Phase 3 B3.4a). This is lighter than a trait-method and consistent with the established widget-callback pattern. `Finished` callers inside the engine Cycle thread a `SchedCtx`. Decided now; Task 1c verifies no other obstacles.

Open audit item (not gate-blocking):

- **3.5 Task 1d:** confirm `emWindow`'s `close_signal` is fired on user-driven window-close in the winit-callback pathways (it's referenced; verify the firing sites match C++ `SignalClosing` beat-for-beat for modal windows). If a firing site is missing, port it.

## 4. Tasks

Per project plan-tool rules: phased, gated, commit-per-task, gate commands named, anti-patterns flagged.

### Phase 3.5 — `emDialog` as `emWindow`-derived

**Bootstrap decisions (B-stage):**

- **B3.5a (stage-only scan):** Tasks 2–5 each commit at step end. Task 1's audit sub-tasks (1a–1d) are each independently committable if they surface a gap; otherwise they fold into a single "audit complete" note commit. Pre-commit hook stays active throughout — no DIVERGED cascade like B3.4d.
- **B3.5b (test harness):** tests construct dialogs via a helper that provides a parent context + a parent panel. `tests/golden/common.rs::TestSched` + `InitCtx` + `with_sched_reach` already provides the scheduler reach; the new piece is a test-only `make_test_dialog_host(init: &mut TestInit) -> (emContext, PanelId)` helper in `test_view_harness.rs`.

**Task 1 — Audit + gap-fill (1 commit, or 1 per gap if any surface).**

- Run 1a / 1b / 1c / 1d above. For each, read the cited C++ source + Rust code, decide "present" or "port". Port if needed.
- Gate: `cargo check` + `cargo clippy -- -D warnings` + `cargo-nextest ntr` — all pass.
- Output: note in ledger recording which audit sub-tasks found gaps and which commits filled them.

**Task 2 — `DlgPanel` + `DlgButton` PanelBehaviors (1 commit).**

- Add `DlgPanel` struct + `impl PanelBehavior for DlgPanel` in `emDialog.rs`. Composes an `emBorder`. Layout children into content area + button row (ports C++ `DlgPanel::LayoutChildren`). Input handler (Enter→Ok, Escape→Cancel) moves off the current plain-struct `emDialog::Input` onto `DlgPanel::Input` per C++ `DlgPanel::Input`.
- Add `DlgButton` struct + `impl PanelBehavior for DlgButton` in `emDialog.rs`. Follows the `ButtonPanel` wrapper precedent (see `emColorFieldFieldPanel.rs::ButtonPanel`). Stores its `result: DialogResult` value. On `Clicked()` (C++ `DlgButton::Clicked`), reaches up to the owning `DlgPanel` / `emDialog` to call `Finish(result)` — via the dialog's `EngineId` stored in a field, or via a result-signal subscribed by the DialogPrivateEngine.
- Not yet installed — these are types declared but unused. Task 4 wires them in.
- Gate: cargo check / clippy / nextest all pass. goldens 237/6 preserved. No new `Rc<RefCell>` / unsafe / Any. New files carry proper C++ name correspondence (no renames; `SPLIT:` markers if split out).

**Task 3 — `DialogPrivateEngine` struct + `impl emEngine` (1 commit).**

- Private module in `emDialog.rs` (matching C++ nested private class): `struct DialogPrivateEngine { root_panel_id: PanelId, /* state needed for Cycle that doesn't live on DlgPanel */ }`.
- `impl emEngine for DialogPrivateEngine { fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool { ... } }`.
- Cycle body ports C++ `emDialog::PrivateCycle()` (emDialog.cpp:194+): observe `GetCloseSignal()` signaled → call `Finish(NEGATIVE)`; advance auto-delete countdown if enabled; reach `DlgPanel` via `tree.take_behavior(self.root_panel_id)` + take/put pattern (mirrors `PanelCycleEngine::Cycle`).
- Engine registration helper: `DialogPrivateEngine::install(scheduler, tree_location, root_panel_id, close_signal) -> EngineId` — registers, sets `Priority::High`, connects `close_signal`.
- Not yet consumed — this is infrastructure for Task 4.
- Gate: cargo check / clippy / nextest. Unit test: construct a dummy engine, register, fire close_signal, tick DoTimeSlice, assert engine was woken (via probe — existing `PanelCycleEngineFirstCycleProbe` pattern).

**Task 4 — `emDialog` reshaped to own `emWindow` + install DlgPanel + DialogPrivateEngine (1 commit).**

- `emDialog` struct gains: `window: emWindow`, `root_panel_id: PanelId`, `private_engine_id: EngineId`. Loses: `border: emBorder` (now inside DlgPanel), `buttons: Vec<(String, DialogResult)>` (now reflected in DlgPanel's child DlgButton panels), `result: Option<DialogResult>` (now in DlgPanel behavior state), `finish_signal: SignalId` (now fetched from DlgPanel behavior or kept on emDialog as a facade).
- `emDialog::new(parent_context, title, look)` constructs the window via `emWindow::new_popup_pending`, creates DlgPanel root, installs into window, registers DialogPrivateEngine.
- Public methods (`AddCustomButton`, `AddPositiveButton`, `AddNegativeButton`, `AddOKButton`, `AddCancelButton`, `AddOKCancelButtons`, `GetButton`, `GetButtonForResult`, `GetOKButton`, `GetCancelButton`, `Finish`, `GetResult`, `GetFinishSignal`, `SetRootTitle`, `EnableAutoDeletion`, `IsAutoDeletionEnabled`) all route through the tree to mutate DlgPanel state. `Finish` fires finish_signal + invokes DialogPrivateEngine post-processing.
- DELETE the existing `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool` entirely. Remove `silent_cancel` (the transient-replacement pattern it serves is 3.6 territory and gets a different shape there).
- `emDialog::ShowMessage` migrated.
- All `emDialog.rs` tests rewritten to the new shape: construct, fire signals, tick scheduler, assert.
- Gate: cargo check / clippy / nextest. goldens 237/6 preserved.

**Task 5 — Consumer migration (1 commit).**

- `emStocksListBox.rs` 4 dialog-creation sites (DeleteStocks, CutStocks, PasteStocks, SetInterest) migrate to the new `emDialog::new` surface. The `silent_cancel` pattern gets replaced: when replacing an in-flight dialog, tear down via the scheduler (`scheduler.remove_engine(old_private_engine_id)` + drop the window) rather than setting `result = Some(Cancel)` silently. Observable C++-parity: C++ `delete OldDialog` destroys the dialog outright.
- Any other `emDialog::new` callers (check `rg 'emDialog::new' crates/`) migrated.
- Gate: full `cargo-nextest ntr`, `cargo test --test golden -- --test-threads=1` (237/6), `cargo clippy --all-targets -- -D warnings`.

**Phase 3.5 Closeout:**

- Ledger + invariant sweep for 3.5. Invariants in §5.
- Tag `port-rewrite-phase-3-5-complete`.

### Phase 3.6 — `emFileDialog` rides 3.5; E024 closes

**Task 1 — `emFileDialog` reshaped to compose an `emDialog` (3.5) + install `emFileSelectionBox` as content (1 commit).**

- `emFileDialog` owns an `emDialog` (Rust composition-for-inheritance). Its constructor delegates to `emDialog::new` for the dialog shell, then installs `emFileSelectionBox` into the dialog's content panel (requires `emDialog::content_panel() -> PanelId` accessor added in 3.5).
- `DIVERGED:` at the struct: "C++ `emFileDialog : public emDialog`; Rust composition. Idiom adaptation — observable behavior identical."
- Gate: cargo check / clippy / nextest. Existing emFileDialog tests still run but with minimal adjustment (no Cycle changes yet — 3.6 Task 2 ports Cycle).

**Task 2 — emFileDialog's root DlgPanel gets a file-dialog cycle hook; shared `DialogPrivateEngine` drives it (1 commit).**

- emFileDialog's root panel (installed in Task 1) is a DlgPanel whose PanelBehavior provides a specialised cycle-hook body (per D2: single engine type, per-behavior hook). Hook body ports emFileDialog.cpp:80-106 beat-for-beat: IsSignaled(fsb_file_trigger) → Finish(POSITIVE); overwrite_dialog handling on its finish_signal.
- Subscribe the existing (3.5-created) `DialogPrivateEngine` to `fsb.file_trigger_signal` via `scheduler.connect(fsb.file_trigger_signal, dialog.private_engine_id)`. Matches C++ `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`. The same engine is already subscribed to close_signal from 3.5.
- DELETE `emFileDialog::Cycle(&mut self, ctx)` and `fsb_file_trigger_signal` cached field and `test_force_overwrite_result` helper.
- Gate: cargo check / clippy / nextest.

**Task 3 — Overwrite-dialog transient (1 commit).**

- `CheckFinish` creates the overwrite-confirmation dialog via `emDialog::new` (3.5-ported) + `scheduler.connect(overwrite_dialog.finish_signal, file_dialog_private_engine_id)`.
- On Cycle's overwrite-path cleanup: disconnect (`scheduler.disconnect(...)`) + drop the overwrite dialog.
- Gate: cargo check / clippy / nextest.

**Task 4 — E024 closure regression tests + gate (1 commit).**

- New tests (required; see §6 for exact shape): construct emFileDialog installed into a scheduler, fire `fsb.file_trigger_signal`, tick `DoTimeSlice`, assert `finish_signal` pending and `GetResult() == Some(Ok)`. *No test invokes any `Cycle` method.* Symmetric test for the overwrite-POSITIVE and overwrite-NEGATIVE paths.
- Delete the caller-driven Cycle tests from today (already deleted the method in Task 2; the four tests they cover get replaced by the new scheduler-driven versions).
- Invariant check (§5): `rg 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emFileDialog.rs` → 0 matches.
- Gate: full suite.

**Task 5 — Raw-material update + phase-3-5/3-6 closeout (1 commit).**

- `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json`: `E024.status` → `resolved-phase-3-6`; add `resolution_commit` field with the SHA of Task 4's commit. Remove `phase_3_progress`.
- Closeout ledger + invariant sweep.
- Tag `port-rewrite-phase-3-6-complete`.

## 5. Invariants (grep-enforceable)

Must hold at exit of Phase 3.6:

| ID | Assertion | Command |
|---|---|---|
| I5a | `emDialog` composes an `emWindow` | `rg -n 'window: emWindow\|window: crate::emWindow::emWindow' crates/emcore/src/emDialog.rs` → ≥1 match |
| I5b | `DialogPrivateEngine` exists and is an `emEngine` (one type, per D2) | `rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs` → exactly 1 match |
| I5c | dialog private engine is registered at construction with wake-up subscription | `rg -nU 'register_engine\([\s\S]*?DialogPrivateEngine\|DialogPrivateEngine::install' crates/emcore/src/emDialog.rs` → ≥1 match; `rg -n 'scheduler\.connect\|\.connect\(' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs` → ≥2 matches (close_signal + fsb_file_trigger_signal) |
| I5d | no caller-invoked dialog `Cycle` method | `rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs` → 0 matches |
| I5e | `test_force_overwrite_result` is deleted | `rg -n 'test_force_overwrite_result' crates/` → 0 matches |
| I5f | no new `Rc<RefCell<` in the touched files | `rg -n 'Rc<RefCell<' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs` → 0 matches (≤ pre-phase baseline) |
| I5g | no new `unsafe` blocks in the touched files | `rg -n 'unsafe\s*\{' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs` → 0 matches |
| I5h | E024 resolved | `jq '.entries[] \| select(.id=="E024") \| .status' docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` → `"resolved-phase-3-6"` |
| I5i | goldens preserved | `cargo test --test golden -- --test-threads=1` → 237 passed / 6 failed (same 6 pre-existing) |
| I5j | nextest no regressions | `cargo-nextest ntr` → 0 failed, passed ≥ Phase-3-exit baseline (2476) |

Phase 3.5 exit uses I5a, I5b, I5c (close_signal only), I5d (emDialog.rs only), I5f, I5g (emDialog.rs only), I5i, I5j.

## 6. E024 closure criteria — concrete evidence

E024 is `resolved-phase-3-6` when **all** of the following are true:

1. **Structural evidence in code:**
   - `emFileDialog` no longer has a `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool` method (grep I5d).
   - The `DIVERGED:` block at today's `emFileDialog::Cycle` (emFileDialog.rs:307-321) is deleted alongside the method.
   - The single `DialogPrivateEngine` (per D2) is subscribed via `scheduler.connect` to `fsb.file_trigger_signal` in addition to its 3.5-established close_signal subscription (I5c).

2. **Behavioural evidence in tests** — the following test compiles and passes without any code path invoking `*.Cycle(...)` or `dlg.Cycle(...)`:

   ```rust
   #[test]
   fn file_trigger_signal_drives_dialog_to_finish_via_scheduler() {
       let mut harness = TestDialogHarness::new();
       let mut fd = emFileDialog::install(
           &mut harness.init_ctx(),
           &harness.dialog_host_context(),
           FileDialogMode::Open,
           emLook::new(),
       );
       let finish_signal = fd.finish_signal();
       let fsb_file_trigger_signal = fd.file_selection_box().file_trigger_signal;

       // User action: fire the FSB's file-trigger signal.
       harness.sched.fire(fsb_file_trigger_signal);

       // Scheduler runs one slice. NO test-driven Cycle invocation.
       harness.do_one_time_slice();

       assert!(harness.sched.is_pending(finish_signal));
       assert_eq!(fd.GetResult(), Some(&DialogResult::Ok));
   }
   ```

   Symmetric tests for overwrite-POSITIVE (fires `overwrite_dialog.finish_signal` after setting its result to OK) and overwrite-NEGATIVE paths.

3. **Raw-material status flip:** `E024.status = "resolved-phase-3-6"` in `port-divergence-raw-material.json`, `resolution_commit` = Phase 3.6 Task 4's SHA.

4. **The name correspondence invariant** at `emFileDialog::Cycle` no longer appears on a `pub fn` — the emFileDialog-specific cycle logic is honoured by the DlgPanel behavior's cycle hook body (reached by `DialogPrivateEngine::Cycle` via `tree.take_behavior(root_panel_id)`), which *is* the C++ `emFileDialog::Cycle` virtual-override semantic in Rust form.

## 7. Decisions (resolved via CLAUDE.md authority order + audits)

Each decision cites the rule that settled it + a fallback to take if the choice fails during implementation.

**D1 — `Finished` hook shape: callback, not trait method.**
- *Decision:* `on_finished: Option<WidgetCallbackRef<DialogResult>>` on `emDialog`, mirroring the established Rust port pattern where C++ virtuals become callbacks (see `emDialog::on_check_finish` replacing C++ `virtual bool CheckFinish(int)`, `emButton::on_click` replacing C++ `virtual void Clicked()`). The callback fires at the C++-matching beat from `DialogPrivateEngine::Cycle` after `Finish` completes.
- *Why the rule lands here:* Port Ideology classifies "virtual method → Option<callback>" as **idiom adaptation**, not divergence, because the behavior is identical (subclass hook point). CLAUDE.md Authority Order: Rust idiom applies below the observable surface — `Finished`'s observable behavior is "a function runs when the dialog finishes"; caller's override-vs-callback is not observable.
- *Fallback:* if a consumer needs features a callback can't express (e.g., access to protected emDialog state the way a C++ subclass has), promote `Finished` to a trait method on a `DialogBehavior` trait. Low risk — callbacks are strictly more permissive than virtuals for non-super-calling overrides.

**D2 — Single `DialogPrivateEngine` type; per-dialog Cycle specialisation lives on the dialog's PanelBehavior.**
- *Decision:* One engine type — `DialogPrivateEngine` — registered at emDialog construction. Its `Cycle` reaches the dialog's root behavior via `tree.take_behavior(self.root_panel_id)` and invokes a PanelBehavior-side cycle hook (`DlgPanel::on_private_cycle(ctx) -> bool` or similar). emFileDialog provides its Cycle specialisation by giving its root DlgPanel a different cycle-hook body (ported from emFileDialog.cpp:80-106), not by registering a different engine type.
- *Why the rule lands here:* CLAUDE.md Authority Order. C++ `class emDialog` has a **single** `PrivateEngineClass PrivateEngine` member instance — ONE engine, not two. C++'s `emFileDialog::Cycle()` at emFileDialog.h:195 is a virtual on `emFileDialog` itself (not on `PrivateEngineClass`); the engine's Cycle reaches the dialog via `Dlg&` and virtual-dispatches to the derived dialog's override. Rust's faithful port: one engine type, reaching the dialog behavior via the tree, calling a PanelBehavior cycle hook that the specific dialog type implements. Introducing a second engine struct (`FileDialogPrivateEngine`) would be a Rust-convenience divergence from C++ with **no forced necessity** — CLAUDE.md forbids divergences that aren't strictly forced.
- *Fallback:* if the PanelBehavior cycle-hook signature can't be made to work cleanly (borrow conflicts between engine's scheduler reach and behavior's PanelCtx reach), add an internal dispatch trait `DialogCycleHook` on a marker trait all dialog behaviors implement, then `DialogPrivateEngine` holds `panel_id` only and resolves the hook through the tree. Still one engine type.

**D3 — Overwrite-dialog replacement: explicit `deregister(scheduler)` helper called before drop.**
- *Decision:* Adding `emDialog::deregister(&mut self, scheduler: &mut EngineScheduler)` which calls `scheduler.remove_engine(self.private_engine_id)` and any panel-tree cleanup required. Migrated `emStocksListBox` sites call `old_dialog.deregister(sched); drop(old_dialog);` when replacing an in-flight dialog. `emDialog::silent_cancel()` is deleted.
- *Why the rule lands here:* Drop-time auto-teardown fails on a hard constraint: Rust's `Drop` takes no arguments and has no scheduler reach, yet the registered `DialogPrivateEngine` must be removed from the scheduler (otherwise its next `Cycle` hits a stale `root_panel_id` pointing into a dropped tree — a crash, not a leak). The C++ equivalent works because `emEngine::~emEngine` has a captured scheduler handle via the emContext pointer its constructor stored; Rust's Do-NOT list forbids the `Rc<RefCell<Scheduler>>` / `Weak<Scheduler>` / raw-pointer patterns that would allow an equivalent captured handle. Explicit deregister is therefore **the only shape consistent with both C++-parity tear-down semantics and Do-NOT constraints** — the explicit-call cost is a forced-divergence concession, documented with a `DIVERGED:` at the method.
- *Fallback:* if pattern proves too easy to forget at call sites, introduce a `DialogOwnedHandle` newtype that wraps the dialog + deregisters on its Drop (the handle captures the scheduler via a `&'a mut EngineScheduler` lifetime-bound slot). This shifts the forced cost from "remember to call deregister" to "lifetime-annotate the handle" — tradeoff evaluated at 3.5 Task 4 if the explicit-call shape feels error-prone.

**D4 — Dialog is its own view (owns `emWindow` via `new_popup_pending`), not a panel installed in a parent tree.**
- *Decision:* `emDialog::new` constructs its own `emWindow` via `emWindow::new_popup_pending(parent_context, root_panel, close_signal, view_flags, window_flags, wm_res_name)` with `WindowFlags::MODAL | WindowFlags::POPUP` and `VF_POPUP_ZOOM|VF_ROOT_SAME_TALLNESS`-equivalent view flags. Matches C++ `class emDialog : public emWindow` with its C++ default `VF_POPUP_ZOOM|VF_ROOT_SAME_TALLNESS, WF_MODAL`.
- *Why the rule lands here:* CLAUDE.md Authority Order top: C++ source is ground truth for structure. emDialog IS an emWindow in C++ — any other Rust shape is a structural divergence. Rust's `emWindow::new_popup_pending` was built exactly for this pattern (popup-zoom already consumes it).
- *Fallback:* if `new_popup_pending` turns out to be insufficient for modal-dialog semantics (e.g., modal input-blocking isn't wired through for arbitrary popup owners), extend the popup path as a prerequisite sub-phase rather than abandoning dialog-as-own-view. The framing here is non-negotiable per CLAUDE.md; the implementation rail can be extended if needed.

**D5 — Separate branches per sub-phase, tags at each closeout.**
- *Decision:* `port-rewrite/phase-3-5-emdialog-as-emwindow` merges after 3.5 closeout with tag `port-rewrite-phase-3-5-complete`. `port-rewrite/phase-3-6-emfiledialog-e024` branches from main after 3.5 merge, merges with tag `port-rewrite-phase-3-6-complete`.
- *Why the rule lands here:* Project convention (Phase 3 used this exact cadence — `port-rewrite-phase-3-complete` tag at `d0f1cc7b`). Not a CLAUDE.md rule but a durable observed pattern.
- *Fallback:* if 3.6 turns out trivially small (< 200 LOC) at 3.5 closeout, inline into the 3.5 branch with a combined tag. Decision revisited at 3.5 closeout, not now.

**D6 — `Finished` subclass-override audit (RESOLVED).**
- *Audit result:* emCore contains `emDialog::Finished` default impl only (`emDialog.cpp:189`). Zero emCore-side subclass overrides. One override exists in `emShowStdDlg.cpp:99` (the `emShowStdDlg` command-line tool), which is NOT ported to Rust — it is outside emCore entirely.
- *Consequence:* Phase 3.5 Task 1c can use `on_finished` callback with no migration cost. If `emShowStdDlg` is ever ported, its `Finished` override becomes a closure installed into `on_finished`.
- *CLAUDE.md tie-in:* File and Name Correspondence is honoured — `emDialog.rs::on_finished` carries the callback-for-virtual comment naming C++ `Finished` per the idiom-adaptation convention.

**D7 — Other dialog/window E-entries dependent on this work (RESOLVED).**
- *Audit result:* Open E-entries matching `emDialog|emWindow|AddWakeUpSignal|caller-driven Cycle` in `port-divergence-raw-material.json`: **only E024.** Other partial matches (E018 — emScreen/emWindow inheritance, E020 — dummy-base-class pattern, E030 — screensaver timer) are unrelated to dialog Cycle dispatch.
- *Consequence:* Phase 3.5/3.6 closes E024 only. No co-resolution expected. If any new E-entry is created during implementation that names a dialog-or-window structural divergence surfaced by the work, flag in the phase ledger and resolve in-phase if cheap.

---

**End of brainstorm output.** All open questions resolved by CLAUDE.md authority order + cited audits; fallbacks named where the primary choice could fail. Ready for user review before proceeding to `writing-plans` for the full detailed Phase 3.5 and Phase 3.6 implementation plans.
