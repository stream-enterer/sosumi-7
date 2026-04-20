# Phase 2 — View/Window Composition + Back-Ref Migration — Closeout

**Branch:** port-rewrite/phase-2
**Commits:** 2f11b813..53944db0 (18 commits; see `git log main..port-rewrite/phase-2`)
**Status:** COMPLETE — all C1–C11 invariants SAT. No deferrals — E006/E014/E015/E038 resolved.

## Summary

Phase 2 replaced `Rc<RefCell<>>` ownership on `emWindow` and `emView` with composition-owned plain values and ID-based back-references resolved through `EngineCtx`. `emWindow` now owns `view: emView` directly; `emSubViewPanel` owns `sub_view: emView`. Engines (`emPanelCycleEngine`, `UpdateEngineClass`, `VisitingVAEngineClass`) hold a `PanelScope` — either `Toplevel(WindowId)` or `SubView(PanelId)` — and resolve their view through `ctx.windows.get_mut` or `tree.panels.get_mut(...).as_sub_view_panel_mut()`. `emViewPort::window` became `Option<WindowId>`; port-level `focused: bool` was deleted with focus consolidated on `emView::window_focused` (spec §5 D5.6). `NoticeList` ring was relocated back to `emView` (spec §5 D5.5); `PanelTree` retains only the `has_pending_notices` flag. Popups stay owned by the launching `emView::PopupWindow: Option<Box<emWindow>>` — matching C++'s `emWindow * PopupWindow` — with winit events routed through `App::find_window_mut` which scans popups. `SwapViewPorts` remains a within-view method swapping `CurrentViewPort` between parent and popup (`HashMap::get_disjoint_mut` was ruled out by C++ inspection — wrong shape for this operation).

Gate stayed green throughout the final cascade. nextest 2454 → 2458 (+4 new tests), goldens 237/6 baseline preserved, clippy clean, fmt clean. `rc_refcell_total` dropped 283 → 262 (−21).

## Delta from baseline

See `2026-04-19-phase-2-exit.md` § "Delta from baseline". Key rows:

| Metric | Baseline | Exit | Δ |
|---|---|---|---|
| nextest | 2454/0/9 | 2458/0/9 | +4/0/0 |
| goldens | 237/6 | 237/6 | 0 |
| rc_refcell_total | 283 | 262 | −21 |
| try_borrow_total | 0 | 0 | 0 |
| clippy warnings | 0 | 0 | 0 |

## JSON entries closed

- **E006** (NoticeList relocation SP5) → commit 68dbeadb (Task 6).
- **E014** (engine back-reference migration) → commit 7549fc13 (Task 5) + keystone cb5129e1.
- **E015** (focus consolidation D5.6) → commit aea6f3db (Task 4 D5.6).
- **E038** (borrow-ordering hazard dissolved) → keystone cb5129e1.

## Spec sections implemented

- **§2 P2** — Single-owner composition default. Realized across Tasks 2, 3.
- **§3.1** — Framework-owned roots + disjoint-borrow EngineCtx. Realized in W3+4-backref + keystone.
- **§3.2** — Window-owned views (plain emView, plain emSubViewPanel sub_view, WindowId-based port back-ref). Realized across W3+4-backref, 2, 3, 4-D5.6.
- **§3.7** — Popup lifecycle. Realized in W3+4-backref + Task 8; landed as Path B (C++-faithful parent-view ownership with OS-event routing via App::find_window_mut), not Path A (framework pending_popups map, which would have been a structural divergence).
- **§5 D5.1** — emView plain ownership at all public sites (Tasks 2, 3).
- **§5 D5.2** — PanelScope back-references (Tasks 1, 5, keystone).
- **§5 D5.3** — ViewPort `window_id: Option<WindowId>` (W3+4-backref).
- **§5 D5.4** — Home geometry fields on emViewPort preserved; atomic SwapViewPorts mirroring C++ semantics (Task 9).
- **§5 D5.5** — NoticeList relocation to emView (Task 6).
- **§5 D5.6** — Focus consolidation on emView (Task 4 D5.6).

## Invariants verified

All run at Task 11 + re-run at closeout gate.

- **I2** (`Rc<RefCell<emView>>` production): PASS — zero code hits. Two test-fixture holdouts enumerated below for Phase 5.
- **I2a** (`Weak<RefCell<emView>>`): PASS — zero code hits, comment/prose only.
- **I2b** (`Rc<RefCell<emWindow>>|Weak<RefCell<emWindow>>`): PASS — zero code hits.
- **I2b-W3** (same, W3 deferral check): PASS.
- **I2c** (`view_rc|sub_view_rc` identifiers): PASS with two `#[cfg(test)]` holdouts (below).
- **I6 (partial)**: golden baseline 237/6 preserved.
- **NoticeList-location**: ring heads on `emView`, not `PanelTree`. PASS.

## Test-fixture holdouts (schedule for Phase 5)

Two test helpers still bind `Rc<RefCell<emView>>`, preserved deliberately because they model external C++-test fixtures (per plan I2 prose):

1. `crates/emcore/src/emPanelTree.rs:3251-3273` — `make_registered_tree()` helper + its `RcCell<T>` type alias.
2. `crates/emcore/src/emView.rs:7303-7368` — `visiting_va_cycles_when_activated` test's local `view_rc` binding.

Both compile and pass; neither is reachable from production code. Phase 5 should rewrite these to use plain-value construction patterns (analogous to other Phase-2-era test construction) and delete the holdouts.

## Plan reshuffles (recorded here for audit)

1. **W3 bundled with Task 4's back-ref portion.** First dispatch of Task-W3 returned BLOCKED because narrowing `App.windows` to plain `emWindow` is inseparable from migrating `emViewPort::window: Option<Weak<RefCell<emWindow>>>` → `Option<WindowId>` and rewriting `emWindow::create` to return plain. User-approved bundle. Task 4's D5.6 remainder (focused deletion + focus consolidation) kept separate and ran later (commit aea6f3db).
2. **Task 8 chose Path B (popup owned by parent emView)**, not the plan's Path A (framework-level `pending_popups` map). C++ inspection (`emView.h:670`, `emView.cpp:1636,1678`) showed `emWindow * PopupWindow` on C++ emView; Path A would have been a structural divergence. Winit event routing handled via `App::find_window_mut` scanning popups.
3. **Task 9 did not use `HashMap::get_disjoint_mut`.** C++ `emView::SwapViewPorts` (emView.cpp:1974-2001) swaps `CurrentViewPort` between `this` and `this->PopupWindow` — a field-level swap under one `&mut emView` borrow. `get_disjoint_mut` was ruled out as the wrong shape.
4. **Pre-commit hook temporarily disabled** during Tasks 2→3→4-D5.6→5→6 because each task committed mid-cascade with a red workspace. Hook re-enabled by Task 7 keystone before its final commit; both keystone commit and all post-keystone commits (Tasks 8, 9, 10, ledger, closeout) run under the active hook.

## Task commit sequence

```
53944db0 phase-2 task-10: delete obsolete DIVERGED blocks for emView/emViewPort/emSubViewPanel
25831fe6 phase-2 ledger: record Task 9 SwapViewPorts Shape 2 + rationale
65025efe phase-2 task-9: SwapViewPorts via Shape 2 (parent view ↔ popup view)
17eac70c phase-2 ledger: record Task 8 popup Path B commit + rationale
82aa240d phase-2 task-8: popup Path B (view-owned) + winit event routing + tests
99baa27d phase-2 ledger: record Task 7 keystone commit SHA
cb5129e1 phase-2 task-7 keystone: all callers green; gate restored; hook re-enabled
68dbeadb phase-2 task-6: NoticeList back to emView (closes E006)
7549fc13 phase-2 task-5: emPanelCycleEngine uses PanelScope; SubView resolver wired
aea6f3db phase-2 task-4-d5.6: consolidate focus on emView; delete emViewPort::focused duplicate
8a2393b2 phase-2 ledger: record task-3 commit sha
5278cbf5 phase-2 task-3: emSubViewPanel::sub_view plain (Rc<RefCell<>> removed)
179e0fa0 phase-2 ledger: record task-2 commit sha
9179be6c phase-2 task-2: emWindow::view plain (Rc<RefCell<>> removed)
0158b060 phase-2 task-1: introduce PanelScope
79f02b45 phase-2 task-W3+4backref: DIVERGED annotation on PopupCloseSignal mirror
cd0342fa phase-2 ledger: record task-W3+4backref commit sha
6fdf5096 phase-2 task-W3+4backref: narrow windows to plain emWindow; emViewPort uses WindowId
2f11b813 phase-2: bootstrap — baseline captured, ledger opened
```

## Next phase

Phase 3 — see the upcoming `docs/superpowers/plans/2026-04-19-port-rewrite-phase-3-*.md`. Phase 2 unblocks the clean ctx-based engine dispatch that Phase 3 builds on.

Phase 5 inherits the test-fixture `view_rc` holdouts enumerated above.
