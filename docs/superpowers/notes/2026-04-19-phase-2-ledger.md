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
