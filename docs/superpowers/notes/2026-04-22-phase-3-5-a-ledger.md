# Phase 3.5.A — Runtime Top-Level Windows + Per-emWindow PanelTree — Ledger

**Started:** 2026-04-22
**Branch:** port-rewrite/phase-3-5-a-runtime-toplevel-windows
**Parent:** port-rewrite/phase-3-5-emdialog-as-emwindow at 1e393d2f (tagged port-rewrite/phase-3-5-partial-checkpoint-before-3-5-a)
**Baseline:** nextest 2483/0/9; goldens 237/6; clippy clean. Measured at 1e393d2f.
**Spec:** docs/superpowers/specs/2026-04-21-phase-3-5-a-runtime-toplevel-windows-design.md (a7678e22)
**Plan:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-a-runtime-toplevel-windows.md
**JSON entries:** none opened/closed directly; unblocks E024 via Phase 3.5. E026 opened only on spec §R7 contingency (popup migration split — avoid).

## Bootstrap decisions

See plan §"Bootstrap decisions" (B3.5a.a–B3.5a.g).

## Task log

(Entries appended by each task's commit.)
- **Task 1 — Entry audit:** COMPLETE.
  - Baseline 2483/0/9; fmt + clippy green.
  - Precondition A (App::tree singular) confirmed.
  - Precondition B (create_root asserts single-root) confirmed.
  - Precondition C (PopupWindow single-slot) confirmed.
  - Precondition D (no runtime top-level install path) confirmed.
  - Spec matches current code state — no drift correction needed.
- **Task 2 — Engine classification audit:** COMPLETE.
  - Deliverable: docs/superpowers/notes/2026-04-22-phase-3-5-a-engine-classification.md
  - Production engines classified: 12 (plus DialogPrivateEngine — impl currently `#[cfg(test)]`, production registration deferred per §B3.5a.f).
  - Test engines classified: 27 (16 emcore src + 1 emfileman + 4 eaglemode integration + 3 eaglemode unit + 2 eaglemode golden + 1 examples).
  - Framework count: 32 (8 production + 24 test).
  - Toplevel count: 5 (2 production [StartupEngine, DialogPrivateEngine] + 3 test [ProbePointerEngine, ChildSpawnEngine, SpawnEngineWithProbe]).
  - scope-based (Toplevel or SubView per registered scope) count: 3 production (PanelCycleEngine, UpdateEngineClass, VisitingVAEngineClass).
  - Total `impl emEngine for` sites: 39 (matches `rg` at HEAD 75ae0428).
  - Total `register_engine` call-sites counted: 69 (code + tests).
  - Deviations from plan's starting-point tables: EOIEngineClass reclassified Toplevel→Framework (no tree access in Cycle); ProbePointerEngine reclassified Framework→Toplevel (Cycle captures `ctx.tree as *mut PanelTree`); added StartupEngine / MainWindowEngine / ControlPanelBridge / emStocksPricesFetcher to production table; added 11 test engines outside emcore. See sheet §Deviations for full rationale.
- **Task 3 — PanelTree::Default:** COMPLETE. impl Default for PanelTree
  returns PanelTree::new() (empty tree). Used by Task 6's scheduler dispatch
  as the mem::take sentinel. One unit test (default_produces_empty_tree —
  populate, mem::take, assert source empty + dest populated). Gate green —
  nextest 2484/0/9.
- **Task 4 — emWindow::tree field + take/put:** COMPLETE. Added
  tree: PanelTree field to emWindow struct; all ctors construct
  PanelTree::default() (empty, unused). take_tree (mem::take) / put_tree
  helpers added with dispatch-invariant doc. Field not yet consumed —
  Task 6 wires into scheduler dispatch, Task 7 migrates home tree into it,
  Task 8 migrates popup tree into it. One roundtrip unit test. Gate
  green — nextest 2485/0/9.
- **Task 5 — PanelScope extension:** COMPLETE. Added Framework variant;
  SubView gains window_id field (struct variant, flat — no `rest` chain).
  Added window_id() accessor. resolve_view updated: Framework → None;
  SubView walk now WindowId-aware (ctx.tree still &mut PanelTree at this
  task — Option migration in Task 6). One new unit test
  (window_id_extraction); pre-existing scope_variants_exist expanded to
  cover all three variants including struct-variant SubView. Migrated
  existing SubView call-sites to new struct shape (emSubViewPanel.rs,
  emPanelTree.rs, emView.rs — UpdateEngine + VisitingVA match arms plus
  defensive Framework no-op arms since those engines are panel-bound).
  Call-sites pass WindowId::dummy() for now; Task 7 backfills real
  WindowIds through window/tree construction. Framework variant not
  yet dispatched (Task 6). Gate green — nextest 2486/0/9.
