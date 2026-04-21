# Phase 3.5 — emDialog as emWindow — Ledger

**Started:** 2026-04-22
**Branch:** port-rewrite/phase-3-5-emdialog-as-emwindow
**Baseline:** see docs/superpowers/notes/2026-04-19-phase-3-closeout.md (nextest 2476/0/9; goldens 237/6; rc_refcell_total 256).
**Plan:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow.md
**Source brainstorm:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow-plan.md
**JSON entries:** E024 remains open (closed in Phase 3.6, not here).

## Bootstrap decisions

See plan §"Bootstrap decisions" (B3.5a–B3.5e).

## Task log

- **Task 1 — Audit:** COMPLETE.
  - 1a: ViewFlags::POPUP_ZOOM and ROOT_SAME_TALLNESS present at emView.rs:22 bitflags block (POPUP_ZOOM line 23, ROOT_SAME_TALLNESS line 28). PASS (no gap-fill).
  - 1b: DeferredAction::CloseWindow(winit::window::WindowId) present at emEngineCtx.rs:37. PASS.
  - 1c: `on_finished: Option<WidgetCallbackRef<DialogResult>>` per D1 (brainstorm decision). Matches Rust port's existing virtual-to-callback rendition (emButton.on_click, emDialog.on_check_finish). No audit gap. Task 5 (emDialog reshape) adds the field.
  - 1d: close_signal firing confirmed on WindowEvent::CloseRequested in emGUIFramework.rs:399-420. Modal (WF_MODAL) top-level windows traverse the same `self.windows` branch (lines 406-411), so close_signal fires identically for modal and non-modal on user-requested close. PASS.
- **Task 2 — DlgPanel:** this commit. DlgPanel struct + impl
  PanelBehavior for DlgPanel added to emDialog.rs. Paint delegates to emBorder;
  LayoutChildren positions content_panel above buttons_panel; Input consumes
  Enter→pending_ok and Escape→pending_cancel per C++ DlgPanel::Input. Four
  unit tests added. Gate green — nextest 2480/0/9.
  - Adaptation from plan: `ctx.panel_size()` not present on PanelCtx; used
    existing `ctx.layout_rect()` and destructured `Rect { w, h, .. }`. Avoids
    adding a one-off helper for Task 2 when a near-identical one exists.
  - DlgPanel and its impls are `#[cfg(test)]`-gated in Task 2; Task 5 removes
    the cfg gate when wiring in the real consumer. No emDialog reshape in
    Task 2.
  - Port-fidelity fixes folded into this commit (code-review):
    - C1: DlgPanel::Input now rejects Shift too (C++ state.IsNoMod() per
      emInput.h:293). New test `dlg_panel_shift_enter_is_ignored` proves it.
    - C2: LayoutChildren rewritten beat-for-beat from C++ emDialog.cpp:302-322
      (bh = min(w*0.08, h*0.3); sp = bh*0.25; inset all four sides; content
      above buttons of height bh). Discarded prior flat BUTTON_HEIGHT/
      BOTTOM_MARGIN drift (those constants stay — still used by outer
      emDialog placeholder; removed in Task 5).
    - C3: emBorder has no Input in Rust; DIVERGED comment annotates the gap
      (C++ emBorder::Input handles focus traversal at emDialog.cpp:279).
    - I1: switched from GetContentRect to GetContentRectUnobscured per
      emDialog.cpp:308.
- **Task 3 — DlgButton:** this commit. `DlgButton` PanelBehavior added to
  emDialog.rs (`#[cfg(test)]`-gated alongside DlgPanel). Wraps `emButton`;
  carries `result: DialogResult` (C++ `int Result`) and `dlg_panel_id:
  PanelId` (Rust analog of C++ `((emDialog*)GetWindow())->Finish(Result)`
  back-edge at emDialog.cpp:236). `PanelBehavior::Paint`/`Input`/`GetCursor`
  are pure delegators to `emButton` — `Input` does NOT write
  `pending_result`; click observation is engine-side via
  `scheduler.connect(button.click_signal, private_engine_id)` (wired in
  Task 4+7, matching C++ `PrivateEngineClass::AddWakeUpSignal`). Two unit
  tests added (`dlg_button_carries_result_payload`,
  `dlg_button_set_caption_updates_emButton`). Gate green — nextest
  2482/0/9.
  - Follows `ButtonPanel` adapter precedent at
    `emColorFieldFieldPanel.rs:187-210`.
  - `DlgButton::new` takes `(ctx, caption, look, result, dlg_panel_id)` —
    `look` is required by `emButton::new`'s actual signature
    (`emButton.rs:46`). Plan text (line 674) predicted only `ctx`; adjusted
    to match real signature.
