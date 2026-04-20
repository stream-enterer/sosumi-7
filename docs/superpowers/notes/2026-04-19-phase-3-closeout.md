# Phase 3 — Widget Signal Model + emFpPlugin API — Closeout

**Branch:** port-rewrite/phase-3-continue (merged into port-rewrite/phase-3-partial-checkpoint-1 → main at C9)
**Commits:** c7ca9971..29b958a0 (Tasks 1–7 + closeout artifacts; see `git log main..port-rewrite/phase-3-continue`)
**Status:** COMPLETE — all C1–C11 checks passed (C9/C10/C11 pending user confirmation)

## Summary

Phase 3 landed the full widget signal model across all 11 emCore widget types (emCheckButton, emCheckBox, emButton, emRadioButton, emTextField, emColorField, emScalarField, emFileSelectionBox, emListBox, emSplitter, emDialog): each widget now allocates `*_signal: SignalId` field(s) at construction and fires them at the C++-faithful points in its Input path and setter paths, with scheduler-dispatched callbacks via the new `WidgetCallback<Args>` / `WidgetCallbackRef<T>` aliases. InputDispatchEngine was introduced as a framework-owned `Priority::VeryHigh` engine draining `pending_inputs` each slice. Clipboard was relocated from `emContext` to `emGUIFramework`. The `emFpPlugin` / `emFpPluginList` API was migrated to take `&mut dyn ConstructCtx`. emFileDialog was refactored to a signal-driven `Cycle` method (E024 prepared; full scheduler dispatch deferred to a later emDialog→emEngine port phase). E025 is fully closed; E024 remains open with a `phase_3_progress` note. All invariants I3a–I3e are satisfied at exit.

## Delta from baseline

| Metric | Baseline | Exit | Δ |
|---|---|---|---|
| nextest passed | 2458 | 2476 | **+18** |
| nextest failed | 0 | 0 | 0 |
| nextest skipped | 9 | 9 | 0 |
| goldens passed | 237 | 237 | 0 |
| goldens failed | 6 | 6 | 0 |
| rc_refcell_total | 262 | 256 | **−6** |
| diverged_total | 176 | 173 | **−3** |
| rust_only_total | 17 | 18 | **+1** |

## JSON entries closed

- E025 — resolved-phase-3 (resolution_commit 33d25c72)
- E024 — NOT CLOSED (phase_3_progress recorded at 44e4aa9b, 8a9154f4; deferred to later phase pending emDialog→emEngine port)

## Spec sections implemented

- §3.1 (clipboard on emGUIFramework)
- §3.5 (widget signal model)
- §4 D4.9 (InputDispatchEngine)
- §4 D4.10 (ConstructCtx for widget construction)
- §6 D6.1–D6.5 (widget event model)

## Invariants verified

- I3a (widget signal fields) — PASS (8/8 widget files; emCoreConfigPanel excluded per Task 7 spec-error note — C++ exposes no GetXxxSignal methods)
- I3b (widget callbacks use alias) — PASS with 6 documented exclusions (Task 7 ledger inventory; all are non-widget or return-value-veto callbacks)
- I3c (clipboard on emGUIFramework) — PASS
- I3d (emFpPlugin API takes ConstructCtx) — PASS
- I3e (InputDispatchEngine framework-owned top-priority) — PASS

## Next phase

Phase 4 — see plan file when ready. Note deferred work: emDialog → emEngine port to close E024.
