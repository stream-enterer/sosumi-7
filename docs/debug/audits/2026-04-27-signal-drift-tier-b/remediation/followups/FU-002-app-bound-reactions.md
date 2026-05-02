# FU-002 — App-bound reaction wiring (mainctrl)

**Pattern:** reaction body needs `&mut App` access from `Cycle` context.
**Scope:** `emmain`.
**Row count:** 3 stubs (B-012-followup) — likely more once swept.
**Prereq buckets:** **architectural decision required** — App threading model is not yet established for Cycle-side mutation.

## Pattern description

B-012 (rc-shim-mainctrl) fixed *subscription* drift by replacing closure shims with proper signal subscribes. The *reaction-body* axis was deliberately left out of B-012 scope: bodies that need to mutate or invoke the top-level `App` (Duplicate window, ToggleFullscreen, Quit) cannot do so from `Cycle(&mut self, ectx)` because `&mut App` is not threaded through. C++ takes this for granted (`MainWin->Quit()` is direct method dispatch on a long-lived parent pointer); the Rust ownership model needs an explicit decision.

## Items (current sightings)

| ID | Site | C++ call | Status |
|---|---|---|---|
| FU-002-1 | `crates/emmain/src/emMainControlPanel.rs:562` | `MainWin.Duplicate()` | `TODO(B-012-followup)` |
| FU-002-2 | `crates/emmain/src/emMainControlPanel.rs:570` | `MainWin.ToggleFullscreen()` | `TODO(B-012-followup)` |
| FU-002-3 | `crates/emmain/src/emMainControlPanel.rs:610` | `MainWin.Quit()` | `TODO(B-012-followup)` |

A tree-wide grep for closure-shim or skipped-reaction comments in emmain may surface additional sightings; first phase of this bucket should enumerate before implementing.

## Architectural decision (required first phase)

Pick one before implementation:

- **(a) Thread `&mut App` through `EngineCtx` / `SignalCtx`** — broadest reach, but widens already-busy ctx traits and potentially leaks App-level concerns into emcore.
- **(b) Pending-action queue on `App`** — Cycle pushes an enum variant (`AppAction::Duplicate { window_id }`); App drains at the top of its main loop. Mirrors a pattern already used elsewhere; keeps Cycle pure.
- **(c) `Rc<RefCell<App>>` registry lookup via `EngineCtx::app()`** — single accessor, cheapest to add, but adds a new (a)-justified `Rc<RefCell<>>` to a hot path.

C++ shape (direct method dispatch) is **language-forced** unavailable. Whichever option ships needs a `DIVERGED:` annotation with the chosen category cited; (b) is the most defensible (no new Rc, no ctx widening).

## Acceptance

- All `TODO(B-012-followup)` markers removed.
- One coherent App-threading mechanism in place; documented in `docs/superpowers/specs/`.
- Behavior matches C++ on golden tests for the affected windows.

## Notes

- Do not skip the architectural-decision phase. The three-line stub above hides a real ownership question; picking arbitrarily will create the next round of follow-ups.
- This bucket is **separate from** D-007 ectx-threading. D-007 covers signal-firing sites; FU-002 covers App mutation sites — different axis, possibly different mechanism.
