# Phase 3 — Widget Signal Model + emFpPlugin API — Ledger

**Started:** 2026-04-20
**Branch:** port-rewrite/phase-3
**Baseline:** see 2026-04-19-phase-3-baseline.md
**Spec sections:** §2 P1/P3, §3.1, §3.4 (clipboard), §3.5, §6 D6.1–D6.5, §4 D4.9, §4 D4.10
**JSON entries to close:** E024, E025

## Bootstrap decisions

- **B11a (stage-only scan):** Phase 3 plan has NO stage-only tasks — every Task 1–7 has its own commit at step end. Pre-commit hook left in place. B11a skipped.
- **Phase-1.5 precondition deviation:** `_pending_inputs` field exists with underscore prefix (unused marker). Phase 3 Task 1 Step 5 wires it and drops the underscore. User approved proceeding (Option A) on 2026-04-20.

## Task log

- **Task 1 (InputDispatchEngine):** commit c7ca9971. Framework-owned engine at `Priority::VeryHigh` drains `pending_inputs` each slice; three winit callback sites migrated from immediate `win.dispatch_input(...)` to enqueue-and-wake. `EngineCtx` gained `pending_inputs` + `input_state` (latter needed by `emWindow::dispatch_input`); `DoTimeSlice` signature + ~20 call sites updated. Gate green. Baseline deltas: nextest +1 (new test → 2459 passed, 9 skipped), goldens 237/6 preserved, rc_refcell_total unchanged, rust_only_total +1 (emInputDispatchEngine.rust_only).
