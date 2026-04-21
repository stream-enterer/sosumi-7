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
