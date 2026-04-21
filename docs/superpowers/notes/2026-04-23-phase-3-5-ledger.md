# Phase 3.5 — Deferred emDialog Construction + Consumer Migration — Ledger

**Spec:** `docs/superpowers/specs/2026-04-21-phase-3-5-deferred-dialog-construction-design.md` (commit `55b3a76d`).

**Base:** `port-rewrite/phase-3-5-a-runtime-toplevel-windows` @ `586d6af5` (tagged `port-rewrite-phase-3-5-a-complete`). Baseline nextest 2492/0/9, goldens 237/6.

## Entry audit

- `App::pending_actions` closure rail present at `emGUIFramework.rs:188`.
- `PendingTopLevel` shape: `{dialog_id, window, close_signal, pending_private_engine: Option<Box<dyn emEngine>>}`. Phase 3.5 Task 5 replaces `pending_private_engine` with `private_engine_root_panel_id: PanelId`.
- `ConstructCtx` trait exposes `create_signal` / `register_engine` / `wake_up`. Phase 3.5 adds `pending_actions` / `allocate_dialog_id` / `root_context`.
- `DlgPanel` / `DlgButton` / `DialogPrivateEngine` `#[cfg(test)]`-gated. Phase 3.5 un-gates.
- `DialogPrivateEngine::window_id: Option<WindowId>` — Phase 3.5 narrows to `WindowId`.
- Legacy `emDialog` API on the struct itself; Phase 3.5 deletes.
- Consumer polling: `emStocksListBox` at 4 Cycle sites, `emFileDialog::Cycle` at the overwrite branch. Phase 3.5 replaces with `Rc<Cell>`.
- Dead API: `emFileDialog::{set_mode, dialog_mut}` — zero live callers. Phase 3.5 deletes.

### Audit corrections vs plan

- Plan predicted "15+ `#[test]` functions" in `emDialog.rs` test module; actual count is 20. Ledger text uses "15+" as specified — correction noted here for record.

## Task ledger

- **Task 1 — Entry audit + ledger open:** COMPLETE. Branch created off port-rewrite-phase-3-5-a-complete (586d6af5). Entry audit recorded. Gate green — nextest 2492/0/9.
- **Task 2 — Extend ConstructCtx + pending_actions closure rail:** COMPLETE. Added `FrameworkDeferredAction` re-export, extended `ConstructCtx` trait with `pending_actions()` / `root_context()` / `allocate_dialog_id()`, added `pending_actions` required field to `InitCtx`/`EngineCtx`/`SchedCtx` and `Option` field to `PanelCtx`, added `EngineScheduler::allocate_dialog_id()` stub, threaded `pending_actions` through `DoTimeSlice` and all engine dispatch paths, updated `with_sched_reach` to 8 args, updated all test harnesses and 47 call-site files. Gate green — nextest 2493/0/9.
- **Task 3 — scheduler engines_for_scope + App::allocate_dialog_id delegation:** COMPLETE. `App::next_dialog_id` field + init deleted; `App::allocate_dialog_id` delegates to `self.scheduler.allocate_dialog_id()`. `engines_for_scope(PanelScope) -> Vec<EngineId>` added to `EngineScheduler`. Note: `EngineScheduler::next_dialog_id` + `allocate_dialog_id` were pre-emptively landed by Task 2's implementer. Gate green — nextest 2495.
- **Task 4 — pending_actions threaded through construction sites:** COMPLETE. Work landed inside Task 2's compile-driven plumbing (commit fc2fe40e): DoTimeSlice signature, every EngineCtx/SchedCtx site, TestViewHarness/InitHarness/TestSched. Audit grep of EngineCtx/SchedCtx/InitCtx construction sites (6 EngineCtx, 50+ SchedCtx, 23 InitCtx) → all plumbed. Gate green — nextest 2495/0/9.
- **Task 5 — PendingTopLevel reshape + install-time engine construction:** COMPLETE. `pending_private_engine` replaced with `private_engine_root_panel_id: PanelId`. `DialogPrivateEngine::window_id` narrowed `Option<WindowId>` → `WindowId`. `install_pending_top_level` + `install_pending_top_level_headless` build the engine from stored inputs at install time (`#[cfg(test)]`-gated in `install_pending_top_level` until Task 6 un-gates `DialogPrivateEngine`). 3.5.A Task-10 test migrated to new shape. New test `pending_top_level_carries_private_engine_root_panel_id` added. Gate green — nextest 2496/0/9.
