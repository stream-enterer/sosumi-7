# W3 Popup Architecture Close-out (2026-04-18)

## Outcome

PopupPlaceholder stub deleted; real `emWindow` restored across the popup path with deferred OS surface materialization wired end-to-end. Popup windows now construct in the Pending state and materialize their winit/wgpu surface on the first `about_to_wait` tick via a back-channel drained in `App`.

## Phase summary

| Phase | Task | Commit | Outcome |
|---|---|---|---|
| 1 | OsSurface enum refactor | 6506900 | OK (refactor + DIVERGED comment for missing accessor methods) |
| 2 | new_popup_pending | 0b303f9 | OK |
| 3 | materialize_popup_surface | 7edec0b | OK |
| 4 | Delete PopupPlaceholder, rewrite popup branch | 483d440 + 612f2b2 | OK (483 lands, 612 fixes lazy-wire guard + signal-fire safety doc) |
| 5 | Materialization integration test | 49c385d | OK (DISPLAY-gated) |
| 5 | Cancellation test | b8da326 | OK (DISPLAY-gated) |
| 6 | popup_window.rs cleanup | 31ff689 | OK (retarget to new_popup_pending) |
| 7 | Acceptance | <this task> | OK — all 6 verification steps pass |

## PHASE-6-FOLLOWUP markers

- Cleared: 4 (struct doc, new_popup, SetViewPosSize, RawVisitAbs call site)
- Remaining: 1 (VIF-chain migration at emView.rs:3626, input-dispatch work, out of scope per W3 spec)

## Test counts

Before W3 (`main` HEAD `468f411`):
- nextest: ~2415 passed
- golden: 237 passed / 6 failed

After W3 (HEAD `31ff689`):
- nextest: 2418 passed, 9 skipped, 0 failed
- golden: 237 passed / 6 failed (matches baseline — no new regressions)
- New tests added: 3 (1 inline `new_popup_pending` test, 2 DISPLAY-gated integration tests in eaglemode crate)

## Architectural changes

- `OsSurface { Pending(Box<PendingSurface>), Materialized(Box<MaterializedSurface>) }` — boxed to satisfy `clippy::large_enum_variant`. Tuple-pattern access only.
- `App::pending_actions: Rc<RefCell<Vec<DeferredAction>>>` (was `Vec<DeferredAction>`). Drains by-move so `Rc::strong_count == 1` cancellation in `materialize_popup_surface` works.
- `emView::pending_framework_actions: Option<Rc<RefCell<...>>>` back-channel; lazily wired (is_none-guarded) at top of `App::about_to_wait`.
- `emWindow::SetViewPosSize` is Pending-tolerant (stashes pos/size into `PendingSurface.requested_pos_size`, applied on materialization). Signature changed `&self → &mut self`.
- Framework type is `App` (pre-existing project naming, file is `emGUIFramework.rs`).

## Scope expansions

- Lazy-wire location: plan said wire at every window-creation site; landed at top of `about_to_wait` (single touch-point, idempotent). Better than spec.
- `SetViewPosSize` Pending-tolerance was needed at runtime; not anticipated in the plan.

## Items deferred to follow-up

1. `materialize_popup_surface` surface-creation block duplicates `emWindow::create()`'s wgpu surface init (~50 lines). Extract `fn build_materialized_surface(gpu, winit_window) -> MaterializedSurface` helper. Plan flagged as optional; deferred.
2. Plan-listed `materialized()`/`materialized_mut()` accessor methods on `emWindow` were inlined as `match` at call sites (DIVERGED comment present); idiomatic for Rust borrow rules.

## Risk register outcomes

- R1 (missed field-access site): no occurrences; cargo check + nextest caught nothing.
- R2 (back-channel plumbing invasive): mitigated via lazy-wire approach.
- R3 (test scheduler wiring): used `SignalId::default()` fallback for scheduler-less unit tests; documented.
- R4 (windows-map iteration mid-insert): drain runs at line ~455 before window-iteration; preserved.
- R5 (winit window destruction timing): cleanup-closure on popup-exit removes from App.windows; signal fire is defensive (lookup-or-noop in emScheduler).
- R6 (first-frame popup paint timing): accepted ~16.7ms concession.
