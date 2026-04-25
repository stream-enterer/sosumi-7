# F010 — Directory listing loads slowly and renders blank — Root Cause

**Date:** 2026-04-24
**Issue:** Programmatic `VisitPanel`/`VisitFullsized` on the inner content sub-view never advances the view animator, so cosmos never zooms into view from `StartupEngine`'s `VisitFullsized(":")` and the agent-driven `visit` command is a silent no-op. The user-visible "blank after long Loading NN%" symptom is a downstream consequence: cosmos materialises only when the user's mouse-wheel zoom takes the synchronous `RawVisit` path, and even then any animator-driven zoom adjustments (focus follow, fullsized snap) are dropped.

## Root Cause Chain

1. **Symptom (E3.1, runtime capture):** Inner content sub-view `Current XYWH` equals `Home XYWH = 0,0,1285,700.556505` throughout the canonical capture sequence — before any visit, after `visit identity="root:content view"`, and after `visit view="root:content view" identity=":"`. `wait_idle` reports `ok` after each command. Cosmos panel: `Viewed: no, PaintCount: 0, LastPaintFrame: 0`.

2. **Direct cause (code, `crates/emcore/src/emView.rs:281-353`):** `VisitingVAEngineClass::Cycle` only runs when the wrapper engine is in a scheduler wake queue. Once running, it observes `va.is_active()` and forwards to `va.animate(...)`. The wrapper engine self-rewakes by returning the result of `animate` (per `emScheduler.rs:666-674`).

3. **Missing wake (code, `crates/emcore/src/emViewAnimator.rs:896-898`):** `emVisitingViewAnimator::Activate` is two lines and only sets `self.active = true`. It does NOT wake the wrapper engine. Because `EngineScheduler::register_engine` (`emScheduler.rs:313-327`) sets `awake_state: -1` (sleeping) and `RegisterEngines` does not call `wake_up` on `visiting_va_engine_id`, the wrapper engine is asleep at registration and stays asleep until something wakes it. Searching production code (`grep visiting_va_engine_id` minus tests/teardown) finds no path that wakes it after `Activate`.

4. **C++ reference (`~/Projects/eaglemode-0.96.4/src/emCore/emViewAnimator.cpp:68-84` and `:1040-1044`):** In C++, `emViewAnimator` derives from `emEngine`, so the animator IS the engine. `emViewAnimator::Activate` calls `WakeUp()` on line 81 to enqueue itself on the scheduler. `emVisitingViewAnimator::Activate` chains directly to it. Activation in C++ guarantees the next-slice cycle.

5. **Why outer-view zoom still works:** The user's mouse-wheel zoom on the outer view goes through `emView::Zoom` (`emView.rs:1296-…`), which calls `RawVisit` directly, synchronously, without using `VisitingVA`. So mouse-wheel zoom on the outer view is unaffected. Any *programmatic* `Visit*` call (`StartupEngine`'s `VisitFullsized(":")` on the inner view, control-channel `visit`/`visit-fullsized`/`seek-to`, focus follow-zoom) goes through the animator and is silently dropped.

6. **Why the existing regression test passes:** The test `visiting_va_cycles_when_activated` (`emView.rs:7398-7474`) at line 7454 calls `sched.borrow_mut().wake_up(visiting_id);` *manually* before `DoTimeSlice`. The manual wake_up was a workaround for the missing wake in `Activate`, not validation that `Activate` itself wakes — the test only confirmed registration + manual wake produces a Cycle.

## Fix Direction

Plumb `&mut SchedCtx<'_>` through the Visit-family methods on `emView` (`VisitByIdentity`, `VisitFullsized`, `VisitFullsizedByIdentity`, `VisitPanel`, `VisitByIdentityBare`) and through the public navigation helpers that wrap them (`VisitNext`, `VisitPrev`, `VisitFirst`, `VisitLast`, `VisitIn`, `VisitOut`, `VisitLeft`, `VisitRight`, `VisitUp`, `VisitDown`, `VisitNeighbour`). After `va.Activate()`, call a new helper `emView::wake_visiting_va_engine(&self, &mut SchedCtx)` that does `if let Some(id) = self.visiting_va_engine_id { ctx.wake_up(id); }` — mirror of `WakeUpUpdateEngine`.

Update production callers to pass `ctx`:
- `emCtrlSocket::handle_visit`, `handle_visit_fullsized`, `handle_seek_to` (close gap #3 from the F010 handoff: route `seek_to` through `VisitByIdentityBare` once the seek engine lands).
- `emViewInputFilter` mouse-zoom and focus-follow paths.
- `emWindow` keyboard navigation block (Tab/Arrow/Home/End/PageUp/PageDown).
- `emSubViewPanel::VisitByIdentity` delegation.
- Internal navigation helpers within `emView`.

Update the existing test `visiting_va_cycles_when_activated` to remove the manual `wake_up(visiting_id)` workaround once `Activate` (via the Visit* surface) handles the wake.

Rejected alternatives:
- **Always-poll the wrapper engine** (return `true` from `Cycle` even when `!is_active`): breaks `EngineScheduler::is_idle()` (line 734-737) which means `emCtrlSocket::wait_idle` in tests and the agent control channel would never return idle.
- **Wake from `UpdateEngineClass::Cycle`**: `UpdateEngine` is itself only woken on demand (notices/signals). After `Activate`, `UpdateEngine` may not run for a long time.
- **Store `engine_id` on the animator and wake from inside `Activate`**: still requires scheduler access, so callers still need to pass `SchedCtx`.

## Verification

The implementation pass should:

1. Add a unit test mirroring `visiting_va_cycles_when_activated` but **without** the manual `wake_up` line; assert that after `view.VisitByIdentityBare(..., &mut sc)`, a single `DoTimeSlice` cycles the animator (or at least removes it from the active state via `animate`'s convergence).

2. Pass `cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr`.

3. Manual verification (via `repro` field on F010): launch the app, run the canonical capture sequence:
   - Capture baseline tree dump.
   - `visit view="root:content view" identity=":"` (cosmos).
   - `wait_idle`.
   - Capture new tree dump.

   Expect: inner content view's `Current XYWH` ≠ `Home XYWH`, cosmos panel `Viewed: yes` and `PaintCount > 0`. Then zooming further should walk into a directory listing without the long "Loading NN%" / blank end-state — because animator-driven zoom adjustments (focus follow, fullsized snap) will now actually run.
