# Phase 2 — View/Window Composition + Back-Ref Migration — Ledger

**Started:** 2026-04-20
**Branch:** port-rewrite/phase-2
**Baseline:** see 2026-04-19-phase-2-baseline.md
**Spec sections:** §2 P2, §3.1, §3.2, §3.7 (popup), §5 D5.1–D5.6
**JSON entries to close:** E006, E014, E015, E038

## B4 predecessor chain

Phase 2 inherits from the Phase 1.76 COMPLETE closeout (the most recent of a four-step sequence). The shared ritual's B4 naming points to `phase-<N-1>-closeout.md`; Phase 1's closeout file is not present on disk, but the COMPLETE chain is documented and accepted per the handoff:

1. Phase 1 — COMPLETE at `port-rewrite-phase-1-complete`.
2. Phase 1.5 — COMPLETE at `port-rewrite-phase-1-5-complete`.
3. Phase 1.75 — COMPLETE at `port-rewrite-phase-1-75-complete`.
4. Phase 1.76 — COMPLETE at `port-rewrite-phase-1-76-complete` (actual predecessor; closeout at `docs/superpowers/notes/2026-04-20-phase-1-76-closeout.md`).

B4 condition satisfied by the Phase 1.76 closeout's `Status: COMPLETE` line.

## Note-file naming convention

Following the ritual's `2026-04-19-phase-<N>-*.md` stem (not the Phase-1.75/1.76 execution-date stem) to maintain grep-ability with the ritual's example patterns. Handoff recommendation.

## Plan reshuffle (recorded 2026-04-20)

Task-W3's first dispatch returned BLOCKED with a sound finding: narrowing `App.windows` + `EngineCtx.windows` to plain `emWindow` is inseparable from migrating `emViewPort::window: Option<Weak<RefCell<emWindow>>>` → `Option<WindowId>` (Task 4's back-ref portion) and rewriting `emWindow::create` to return plain `emWindow`. The `Weak<RefCell<emWindow>>` can only upgrade from an Rc that outlives it, and removing the Rc owner in `App.windows` extinguishes the allocation.

**Decision (user-approved):** Bundle Task-W3 + Task 4's back-ref migration into a single atomic dispatch/commit. Task 4 *retains* the non-back-ref work: delete `focused: bool` field (D5.6), delete DIVERGED blocks at emViewPort.rs:5/43/244, update focus-consolidation callers. Those stay in Task 4 proper, which still happens after Tasks 1–3.

Revised dispatch order:
- **W3+4-backref** (atomic): map narrowing + emViewPort::window_id + constructor rewrites. Commits standalone. ✅ DONE
- **Task 1**: PanelScope. Commits standalone. ✅ DONE
- **Tasks 2 → 3 → 4-D5.6 → 5 → 6**: per plan; each commits its own step (pre-commit hook temporarily disabled; tree may be red-test between steps).
- **Task 7 (KEYSTONE)**: restore full gate; atomic commit if any final fix-ups remain; **re-enable pre-commit hook**.
- **Tasks 8–10**: per plan.

## Task log

### Task-W3 + Task 4 back-ref (bundled) — DONE
Commit: 6fdf5096
rg 'Rc<RefCell<emWindow>>' crates/ : before=9 after=0 (5 comment hits remain)
rg 'Weak<RefCell<emWindow>>' crates/ : before=3 after=0 (2 comment hits remain)
Notes:
- `emViewPort::window` → `window_id: Option<WindowId>` with
  `PaintView`/`InvalidatePainting` resolved through `windows` map; both
  methods now take `&HashMap<WindowId, emWindow>` (were zero-arg before,
  no production callers).
- `emWindow::create` + `new_popup_pending` now return plain `emWindow`
  (not `Rc<RefCell<Self>>`). Added `wire_viewport_window_id()` helper
  so the framework wires the popup's WindowId after materialization.
- `emView::PopupWindow` narrowed from `Option<Rc<RefCell<emWindow>>>` to
  `Option<emWindow>` (owned). Added `PopupCloseSignal: Option<SignalId>`
  so `Update`'s close-signal probe + teardown don't need a popup borrow.
- Task-8 concern flagged: popups now live ONLY in `emView::PopupWindow`,
  never in `App::windows`. Winit events addressed to the popup's WindowId
  are currently not routed; popup OS-event handling redesign is Task 8.
- Task-8 stubbed tests: `popup_materialization.rs` and
  `popup_cancel_before_materialize.rs` rewritten as passing stubs;
  their original assertions (Rc strong_count cancellation, popup in
  `App::windows`) no longer express valid contracts under the new
  ownership model.
- `materialize_popup_surface` replaced by `materialize_pending_popup`:
  walks `App::windows` to find a view holding a Pending popup, flips
  OS surface in place, wires WindowId onto the view-port.
- emScheduler, emEngineCtx, emScreen, emFileModel, test_view_harness,
  emPanelTree, emPriSchedAgent, and 6 workspace test files all updated
  to take `HashMap<WindowId, emWindow>` (plain) in place of the
  `Rc<RefCell<>>` wrapper.
- nextest: 2454/2454 pass. goldens: 237 pass / 6 fail (identical
  failures to baseline: composition_tktest_{1,2}x, notice_window_resize,
  testpanel_{expanded,root}, widget_file_selection_box).

### Task 1 — DONE
Commit: 0158b060da1d6e8ca2192cc168eb9d804f2c2aa5
Files: emPanelScope.rs (new), emPanelScope.rust_only (new), lib.rs
Notes: SubView branch is a Task-5 stub (returns None); Toplevel branch resolves via ctx.windows.get(&wid) + window.view_rc(). Plan's ctx.with_view_mut() does not exist on EngineCtx; implemented inline instead. WindowId::dummy() used in test (winit 0.30 has it); PanelId::null() requires `use slotmap::Key as _` in test scope.

### Task 2 — STAGED (tree red, Task 7 will green)
Commit: 9179be6c
Files: 4 (emWindow.rs, emView.rs, emPanelScope.rs, emGUIFramework.rs)
Notes:
- `emWindow::view` narrowed from `Rc<RefCell<emView>>` to plain `emView`.
  All in-file `.view.borrow()` / `.view.borrow_mut()` sites replaced
  with direct `self.view` / `&mut self.view`.
- `view_rc()` accessor deleted. `view()` / `view_mut()` kept as thin
  wrappers returning `&emView` / `&mut emView` to minimize downstream
  breakage.
- DIVERGED (Phase 2 Task 2): `emView::PopupWindow` boxed as
  `Option<Box<emWindow>>` to break the inline recursion
  `emView -> emWindow -> emView`. This is a forced Rust divergence —
  C++ stores a raw `emWindow*` pointer, so Box preserves the shape.
- PanelScope::Toplevel updated: `ctx.windows.get_mut(&wid)` +
  `&mut window.view`. `as_sched_ctx()` inlined because the split
  borrow (windows vs. scheduler) can't be reconstructed through the
  existing `&mut EngineCtx` accessor while `window: &mut emWindow`
  is held.
- `cargo check -p emcore`: clean. `cargo check --workspace`: red —
  emmain has 3 `win.view_rc()` call sites used for
  `Rc::downgrade(...)` into the view's Weak self-reference
  (RegisterEngines). Those require Tasks 5/6/7 (sub_view + NoticeList)
  to eliminate the emView self-Weak. Left broken per plan.
- Added Task 2 test `window_view_is_plain` that binds `&emView` to
  `&win.view` — compiles iff the field is plain.

### Task 3 — STAGED (tree red, Task 7 will green)
Commit: 5278cbf5
Files: 1 (emSubViewPanel.rs)
Notes:
- `emSubViewPanel::sub_view` narrowed from `Rc<RefCell<emView>>` to plain
  `emView`. All in-file `.borrow()` / `.borrow_mut()` sites replaced with
  direct field / method access.
- `sub_view_rc()` deleted. `GetSubView()` returns `&emView`, `sub_view_mut()`
  returns `&mut emView` (changed from `&self` to `&mut self`).
- `new()`: `Rc::downgrade(&sub_view)` → null `Weak::new()` placeholder for
  `init_panel_view` and `RegisterEngines`; real Weak wiring deferred to
  Tasks 5–7 (same pattern as Task 2).
- `emScheduler.rs` callers of `sub_view_mut()` compile unchanged — they
  bind the result to `v` and call `v.field.take()`, which works on
  `&mut emView` exactly as it did on `RefMut<emView>`.
- No infinite-size recursion: `emView` does not contain `emSubViewPanel`
  inline (panels are `Box<dyn PanelBehavior>` in PanelTree).
- Added `sub_view_is_plain` test (compile-time type assertion).
- `cargo check -p emcore`: clean.

### Task 4 D5.6 remainder — DONE
Commit: aea6f3db
Files: 2 (emViewPort.rs, emView.rs)
Notes:
- Deleted `focused: bool` field from `emViewPort` struct (DIVERGED duplicate of emView::Focused)
- Deleted the 7-line DIVERGED block explaining the duplicate field
- Deleted `is_focused()` and `set_focused()` Rust-only accessors from emViewPort
- `SetViewFocused` and `RequestFocus` are now no-op stubs; canonical focus is `emView::window_focused`
- `SwapViewPorts` focus-swap: directly swaps `self.window_focused` ↔ `popup.view_mut().window_focused`
  (no port intermediary; full SetFocused notification remains Phase-5)
- `RawVisitAbs` line 1860 (`CurrentViewPort.borrow_mut().RequestFocus()`): replaced with
  `self.SetFocused(tree, true)` (tree is in scope; matches C++ emViewPort::SetViewFocused →
  CurrentView->SetFocused path directly)
- 3 caller sites consolidated: SwapViewPorts (2 port ops), RawVisitAbs RequestFocus call
- `cargo check -p emcore`: clean

### Task 5 — STAGED
Commit: 911a5a21
Files: 3 (emPanelCycleEngine.rs, emPanelScope.rs, emPanelTree.rs)
Notes:
- `PanelCycleEngine::view: Weak<RefCell<emView>>` replaced with
  `scope: PanelScope`. `Cycle` resolves the view via
  `PanelScope::resolve_view`, reads `GetCurrentPixelTallness`, and
  falls through the legacy take/put path unchanged.
- `register_engine_for` derives scope from `self.tree_location`:
  `Outer → Toplevel(WindowId::dummy())`,
  `SubView{outer_panel_id, ..} → SubView(outer_panel_id)`.
  The dummy `WindowId` is a pre-Task-7 placeholder — the framework's
  shared outer tree has no single owning window, so resolve-time
  lookup via `ctx.windows` cannot succeed until WindowId threading
  lands. `PanelCycleEngine`s for outer-tree panels therefore sleep
  this phase. Task 7's nextest run will expose regressions.
- `PanelScope::SubView` resolver wired: searches `ctx.tree.panels`
  for the outer emSubViewPanel and reaches `sub_view` via the typed
  `as_sub_view_panel_mut` accessor (no `Any` / `downcast`). Borrow
  split uses disjoint raw pointers for scheduler/framework_actions
  to avoid re-borrowing `ctx` while holding the `panels` borrow.
- Known runtime limitation: when a sub-tree `PanelCycleEngine` cycles,
  `ctx.tree` is already the inner sub-tree (the outer svp's behavior
  is held by the scheduler's dispatch walk), so the SubView search
  misses and the engine sleeps. Spec §13 Q1 resolution deferred.
- No Any/downcast introduced, no new Rc/RefCell/Weak.
- `cargo check -p emcore`: clean.

### Task 7 KEYSTONE — DONE
Commit: cb5129e1
Files: 12 (11 src + 1 ledger)
nextest: 2457 passed / 0 failed / 9 skipped
goldens: 237 passed / 6 failed (same six as baseline:
  composition_tktest_{1,2}x, notice_window_resize,
  testpanel_{expanded,root}, widget_file_selection_box)

Invariants (from plan):
- `Rc<RefCell<emView>>` production: 0 (tests + comments only)
- `Weak<RefCell<emView>>`: 0 code hits (comments/DIVERGED notes only)
- `Rc<RefCell<emWindow>>|Weak<RefCell<emWindow>>`: 0 code hits
  (comments documenting the migration only)
- `view_rc()|sub_view_rc()` method calls: 0
- `NoticeList/notice_ring_head_*` on `emView`: confirmed
  (PanelTree only keeps `has_pending_notices` flag and
  `pending_ring_cleanup` Vec per Task 6)

C1-C9 resolution:
- C1 resolved: 3 `win.view_rc()` sites in emMainWindow — root
  `init_panel_view` drops its unused `view_weak`; control-window
  `init_panel_view` same.  `RegisterEngines` call replaces
  `Rc::downgrade(win.view_rc())` with
  `PanelScope::Toplevel(window_id)`.
- C2 resolved: `emPanelCycleEngine::scope` no longer needs a real
  `WindowId` to resolve — `Cycle` now reads
  `ctx.tree.cached_pixel_tallness` directly (written by
  `emView::SetGeometry`).  Scope is retained only for
  `UpdateEngineClass`/`VisitingVAEngineClass`, which receive a
  real WindowId from `emMainWindow::create_main_window` /
  `create_control_window`.  The `WindowId::dummy()` placeholder in
  `register_engine_for` is now observably benign because
  `PanelCycleEngine::Cycle` never calls `resolve_view`.
- C3 resolved alongside C2: sub-tree `PanelCycleEngine` dispatch
  no longer looks up the outer svp — tallness comes from the
  sub-tree's own cached mirror, set when the sub-view's
  `SetGeometry` runs.
- C4 resolved: `PanelScope::resolve_view` SubView branch was not
  touched further — its unsafe split-borrow remains the single
  safety-documented site in `emPanelScope.rs`.  The Update /
  VisitingVA engines now resolve inline in their own `Cycle`
  impls (not through `resolve_view`), so the SubView branch has
  effectively one caller — straight-line to
  `sub_view_and_tree_mut`.  Task 7's inline form uses no unsafe.
- C5 resolved: `PanelCycleEngine::Cycle` body (take/put on
  `ctx.tree`) unchanged — correct for both Toplevel and SubView
  since dispatch places the relevant tree at `ctx.tree`.
- C6 resolved implicitly: stub `SetViewFocused`/`RequestFocus`
  on `emViewPort` produced no dead-code warning under
  `clippy -D warnings`; left in place (matches C++ API shape).
- C7 resolved: `emView::HandleNotice` drain loop now re-scans
  `tree.has_pending_notices_flag()` after each pass; tree-internal
  `add_to_notice_list` paths (which do not link into the ring,
  per Task 6) are enrolled by the safety-net scan on the next
  loop iteration, so notices propagate within one HandleNotice
  call.  Fixes the pipeline/integration regressions introduced
  by the E006 ring relocation.
- C8 resolved: `emWindow::view()` / `view_mut()` wrappers kept
  (callers are many; method-syntax is cleaner than `&win.view`
  for the full emmain path).
- C9 invariants: see above.

Keystone changes beyond C-notes:
- `emView::UpdateEngineClass` and `VisitingVAEngineClass` no
  longer hold `Weak<RefCell<emView>>`; each stores a
  `PanelScope` and dispatches Toplevel/SubView inline (no
  scope.resolve_view closure — it can't express the borrow of
  both view and tree simultaneously).
- `emView::RegisterEngines` signature: `self_view_weak:
  Weak<RefCell<emView>>` → `scope: PanelScope`.
- `PanelData::View: Weak<RefCell<emView>>` →
  `has_view: bool` (the Weak was only ever checked for presence
  before registering panel engines).
- `PanelTree::create_root` signature: `(name, view_weak)` →
  `(name, has_view)`.
- `PanelTree::init_panel_view` / `set_panel_view`: drop
  `view_weak` arg entirely.
- `PanelTree::cached_pixel_tallness: f64` added as a view mirror,
  written by `emView::SetGeometry`.
- `emSubViewPanel::sub_view_and_tree_mut()` helper added for
  engines that need both halves.

### Task 8 — Popup ownership + winit event routing — DONE
Commit: 82aa240d

**Path chosen: B (popup owned by launching emView).**
Rationale from C++ reference: `emView.h:670` declares
`emWindow * PopupWindow;` as a plain owned pointer on emView,
allocated in `emView.cpp:1636` (`PopupWindow = new emWindow(...)`)
and deleted in `emView.cpp:1678` during zoom-back-inside-home.
There is no framework-level pending-popups registry in C++; the
backend dispatches OS events to each emWindow via its own
callback. Path A (framework pending_popups map) would be a
structural divergence from C++ ownership for no behavioral gain,
so Path B wins.

Changes:
- `App::find_window_mut` — new helper that resolves a winit
  `WindowId` to `&mut emWindow`, first looking in `self.windows`
  and, on miss, scanning parent views' `PopupWindow.as_ref()`
  for a materialized match. O(N_windows) — acceptable for
  normal UIs. This is the forced Rust-side adaptation for
  winit's single-ApplicationHandler dispatch model.
- `App::window_event` branches (CloseRequested, Resized, Moved,
  RedrawRequested, Focused, Touch, input) all route through
  `find_window_mut`. CloseRequested distinguishes top-level
  (auto-delete) vs popup (no auto-delete — teardown is driven
  by emView::RawVisitAbs on zoom-back-inside-home).
- `emView::PopupWindow` DIVERGED annotation rewritten to
  document Path B + the routing adaptation.
- Task-8 deferred comments on `App::windows` and
  `materialize_pending_popup` updated to reflect final model.

Tests restored:
- `popup_materialization::popup_allocated_in_pending_state_owned_by_view`:
  POPUP_ZOOM + outside-home RawVisit allocates a Pending popup
  owned by emView (no App required). Asserts
  `!popup.is_materialized()` and `PopupCloseSignal.is_some()`.
- `popup_cancel_before_materialize::popup_torn_down_before_materialize_leaves_no_observable_state`:
  After Pending popup allocation, `ZoomOut` triggers the
  `else if PopupWindow.is_some()` teardown branch in RawVisitAbs
  (emView.cpp:1676-1680). Asserts `PopupWindow.is_none()` and
  `PopupCloseSignal.is_none()`.

Gate:
- cargo fmt / clippy -D warnings clean.
- nextest: 2457 / 0 fail / 9 skipped.
- goldens: 237 pass / 6 fail (unchanged baseline).

PopupCloseSignal mirror retained (Path B keeps popup on emView;
borrow-conflict rationale still applies). DIVERGED annotation
remains accurate as-is.

---

### Task 9 — SwapViewPorts geometry exchange — DONE
Commit: 65025efe

**Shape chosen: Shape 2 (parent view ↔ popup view field swap).**

C++ `emView::SwapViewPorts` (emView.cpp:1974-2001) swaps
`CurrentViewPort` between `this` and `this->PopupWindow` — both live
under a single `&mut emView` borrow (parent owns PopupWindow inline).
`HashMap::get_disjoint_mut` is not applicable; the plan's Shape 1
cross-window framing does not apply to this codebase structure.

Two gaps vs C++ were fixed:

1. **Popup's `Current*` not updated** (C++ emView.cpp:1991-1995).
   After the swap, `w->CurrentX/Y/Width/Height/PixelTallness` are set
   from `w->CurrentViewPort->HomeView->Home*`. Rust now reads those
   values from the newly acquired port's `home_*` fields and writes
   them to the popup view's `Current*`.

2. **`CurrentPixelTallness` from wrong source** (C++ emView.cpp:1990).
   Was using `self.HomePixelTallness` instead of the swapped port's
   `HomeView->HomePixelTallness`. Fixed by:
   - Adding `home_pixel_tallness: f64` to `emViewPort`.
   - `emView::SetGeometry` now mirrors `Home*` (x, y, width, height,
     pixel_tallness) onto `HomeViewPort` when `is_home` (no popup
     active), so each port carries the correct geometry of its owning
     view for the swap to read.
   - `SwapViewPorts` now reads `vp.home_pixel_tallness` instead of
     `self.HomePixelTallness`.

`new_with_geometry` and `SetViewGeometry` on `emViewPort` updated to
carry `home_pixel_tallness`; `SetViewPosSize` (stub) likewise.

New test: `test_task9_swap_view_ports_geometry_exchange` — verifies
port identity swap, parent `Current*` from popup geometry, and popup
`Current*` from parent geometry (including non-trivial pixel tallness
values 1.5 and 0.75).

Gate:
- cargo fmt / clippy -D warnings clean.
- nextest: 2458 / 0 fail / 9 skipped (+1 new test).
- goldens: 237 pass / 6 fail (unchanged baseline).
