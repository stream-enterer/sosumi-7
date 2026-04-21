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
- **Task 6.1 spike — scope-based dispatch compiles clean:** COMPLETE.
  Parallel engine_scopes SecondaryMap + register_engine_with_scope method
  added; dispatch branches on PanelScope when the new map has an entry;
  legacy engine_locations path retained for non-migrated callers. Clean
  borrow-checker path (windows.get_mut(&wid).map(|w| w.take_tree())
  .unwrap_or_default() onto the stack, then EngineCtx { windows,
  tree: &mut local_tree, ... }) compiles with no unsafe and no
  destructuring gymnastics — the HashMap entry borrow releases at the
  statement boundary, so `windows` is free for ctx construction. All
  three PanelScope arms (Framework / Toplevel / SubView) implemented.
  SubView arm mirrors dispatch_with_resolved_tree's take/put shape but
  rooted at the target window's tree. Two spike tests
  (spike_framework_dispatch_via_scope, spike_toplevel_dispatch_via_scope)
  green. Gate 2488/0/9.
- **Task 6.2 — atomic signature break:** COMPLETE. Keystone migration.
  TreeLocation enum DELETED (crates/emcore/src/emEngine.rs). register_engine
  signature: TreeLocation → PanelScope (crates/emcore/src/emScheduler.rs;
  ConstructCtx trait + all impls in emEngineCtx.rs). DoTimeSlice signature
  dropped `tree: &mut PanelTree` parameter; per-window trees reached via
  windows[wid].tree. EngineCtx::tree: &mut PanelTree → Option<&mut PanelTree>.
  PanelTree::new_with_location(TreeLocation) → new_with_scope(PanelScope);
  PanelTree field `tree_location` → `scope` (stored PanelScope directly).
  emSubViewPanel::new migrated — sub_tree constructed with
  PanelScope::SubView { window_id: dummy, outer_panel_id }. register_engine
  call-sites migrated across 69 sites: MiniIpcEngine / PriSchedEngine /
  InputDispatchEngine / emWindowStateSaver / MainWindowEngine /
  ControlPanelBridge / EOIEngineClass → Framework; StartupEngine →
  Toplevel(window_id); ChildSpawnEngine / SpawnEngineWithProbe test engines
  → Toplevel(dummy wid); emView Update/VisitingVA engines keep
  per-registration-site scope. DialogPrivateEngine registered as
  Framework PLACEHOLDER with its only test #[ignore]d (Task 10 will
  re-register as Toplevel post-materialize).
  Dispatch branches on PanelScope: Framework → ctx.tree = None, no detach;
  Toplevel(wid) → mem::take windows[wid].tree, pass ctx.tree = Some,
  restore on exit (or sleep-and-retry if window is missing); SubView{wid,
  pid} → mem::take windows[wid].tree, hand outer tree through unchanged —
  engine's Cycle walks `ctx.tree.as_deref_mut()?.panels[pid].behavior
  .as_sub_view_panel_mut()` to reach sub_view/sub_tree (scheduler does
  NOT pre-walk, because the take-behavior-off-outer shape would hide
  sub_view from the Cycle body; UpdateEngineClass SubView arm and
  VisitingVAEngineClass SubView arm both depend on this). emPanelScope's
  resolve_view SubView arm updated for `ctx.tree.as_deref_mut()?`.
  emWindow::dispatch_input dropped its external `tree` parameter; inner
  take/put-tree split plus private helper `dispatch_input_with_tree` on
  the legacy shape. resize/render/handle_touch/tick_vif_animations retain
  external tree param (unchanged; App::tree feeds them pre-Task-7).
  Depth-2 `task2_dispatch_walks_depth_2_subview_location` test DELETED —
  PanelScope::SubView is flat (no `rest` chain); no production call-site
  requires multi-level nesting. Test helper
  `test_view_harness::headless_emwindow_with_tree` added to wrap rooted
  trees in a pending emWindow for Toplevel(wid)-registered test engines
  (consumed by 4 emPanelTree tests + 2 emView popup tests). StartupEngine
  Cycle body uses `ctx.tree.as_deref_mut().expect("...")` inline; a doc
  comment notes that windows[wid].tree is the empty-default at Task-6
  exit (Task 7 migrates App's home tree into it) — production startup is
  expected non-functional between Task 6 and Task 7. Gate green — nextest
  2487/0/10 (baseline 2488/0/9 + one new #[ignore] on Dialog test − one
  deleted depth-2 test = net same). Clippy clean, fmt clean. Goldens not
  re-run (Task 8 is the popup risk gate).
  
  **Carry-over to Task 7:** Four emWindow dispatch-side methods retain
  external `tree: &mut PanelTree` param pending migration: resize, render,
  tick_vif_animations, handle_touch. Their callsites feed App::tree
  pre-Task-7 startup. Once App::tree is migrated into emWindow::tree
  (Task 7), these four methods' callers will switch to self.windows[home_wid].tree,
  completing the migration alongside App::tree deletion.
- **Task 7 — Home window owns its tree:** COMPLETE. App::tree field
  deleted. App::home_window_id: Option<WindowId> added (set by
  create_main_window on first home insert). emMainWindow's
  create_main_window builds a local PanelTree, populates it, then
  put_tree's it onto the emWindow before insertion into App::windows.
  create_control_window follows the same per-window pattern (its own
  tree, not home's). Carry-over emWindow methods resize, render,
  tick_vif_animations, handle_touch dropped their external `tree`
  parameter; each uses `self.tree` internally via destructure. Added
  `pub fn tree()` / `pub fn tree_mut()` accessors on emWindow for
  cross-crate reads (emmain::Duplicate). take_tree/put_tree promoted
  from pub(crate) to pub so emmain can migrate the initial home tree
  onto the window. App::home_tree / App::home_tree_mut helpers added
  for legacy App-level tree access sites (ToggleControlView,
  RecreateContentPanels, create_control_window read of MainPanel).
  In about_to_wait's per-window loop, `tree` is sourced via
  `win.take_tree()` (put back at end of iteration) so each window
  operates on its own tree — previous single App::tree borrow is gone.
  WindowEvent::Focused uses inline take/put for SetFocused;
  materialize_pending_popup uses take/put across the SetGeometry
  callsite. StartupEngine (Toplevel(home_wid)) now dispatches on the
  real home tree post-Task-7 (previously saw the empty default tree
  on windows[home_wid] while the real tree was on App::tree).
  Gate green — nextest 2487/0/10, clippy clean, fmt clean.
- **Task 8 — Popup migration:** COMPLETE. `emWindow::new_popup_pending`
  signature drops `root_panel: PanelId` and builds the popup's own
  `PanelTree` + root (`create_root("popup_root", false)`, `has_view=false`
  mirroring `emMainWindow::create_main_window`'s two-phase init). The
  built tree is placed into `emWindow::tree` via direct struct-init (not
  `PanelTree::default()` sentinel). `emView` is constructed over the
  popup-internal root. `emView::RawVisitAbs` drops the `self.root`
  pass-through.

  `App::materialize_pending_popup` no longer pulls `home.take_tree()`
  for the popup's `SetGeometry`; it now takes the popup's own tree
  (`popup.take_tree`/`popup.put_tree`) — consistent with the new
  ownership: popup panels live in popup's own tree.

  Test call-sites migrated: `emScheduler.rs` spike (dummy_tree+cleanup
  removed), `emWindow.rs` four unit tests (window_view_is_plain,
  headless_window_register_engines_registers_engines,
  new_popup_pending_constructs_without_event_loop,
  take_tree_put_tree_roundtrip — the latter now asserts the initial
  take_tree returns the popup's internal rooted tree, not the empty
  sentinel), `emView.rs` three tests (sp4, phase8, swap_view_ports —
  now build popup first, take its internal tree, extend with
  setup_children_on helper on top of the popup's internal root),
  `test_view_harness::headless_emwindow_with_tree` (ctor sig drop;
  discarded internal tree; same Framework/Toplevel harness contract).

  New test helper `setup_children_on(tree, root)` extracts sp4/phase8's
  child-hierarchy builder so popup-harness tests can extend the popup's
  internal root in place.

  This closes the popup-shares-launching-view-tree implicit divergence
  from C++'s `emWindow : emView` (emWindow.cpp:31-33 — popup ctor
  forwards to `emView::emView(parentContext, viewFlags)`, which
  constructs a fresh RootPanel). C++ parity restored at the
  window-tree ownership level; symmetric with home window (Task 7)
  and future dialog windows (Task 10).

  Golden suite re-verified: 237/6 preserved (identical failing set —
  composition_tktest_1x, composition_tktest_2x, notice_window_resize,
  testpanel_expanded, testpanel_root, widget_file_selection_box —
  confirmed pre-existing by git-stash A/B against HEAD^). No
  paint-path regression. Spec §R7 contingency NOT invoked.

  Gate green — nextest 2487/0/10, goldens 237/6 preserved, clippy
  clean, fmt clean.
