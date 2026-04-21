# Phase 4a — Exit

Captured at HEAD `08995307` (branch `port-rewrite/phase-4a`).

## nextest

```
Summary: 2535 tests run: 2535 passed, 9 skipped
```

## goldens

```
test result: FAILED. 237 passed; 6 failed
```

Six pre-existing failures unchanged (composition_tktest_1x/2x, notice_window_resize, testpanel_expanded/root, widget_file_selection_box).

## clippy

`cargo clippy --all-targets --all-features -- -D warnings` clean.

## fmt

`cargo fmt --check` clean.

## rc_refcell_total

329

## diverged_total

182

## rust_only_total

18

## split_total

22

## Delta (exit − baseline)

| Metric | Baseline | Exit | Delta | Notes |
|---|---|---|---|---|
| nextest passed | 2514 | 2535 | +21 | 21 new signal-fire/suppress/clamp tests across 5 primitives ✓ |
| nextest failed | 0 | 0 | 0 | ✓ |
| goldens passed | 237 | 237 | 0 | ✓ |
| goldens failed | 6 | 6 | 0 | no regressions ✓ |
| rc_refcell_total | 304 | 329 | +25 | test scaffolding only: `pa: Rc<RefCell<Vec<FrameworkDeferredAction>>>` replicates the existing production `SchedCtx.pending_actions` signature across 5 primitive test modules. No production `Rc<RefCell<>>` added. Not a phase-4a invariant. |
| diverged_total | 180 | 182 | +2 | new `DIVERGED:` annotation on `emRecNode::parent()` + one secondary annotation in `emRecParser.rs` SPLIT region. |
| rust_only_total | 18 | 18 | 0 | ✓ |

## Invariants (phase-specific)

- **I4a-1.** `crates/emcore/src/emRec.rs` defines `pub trait emRec`. ✓ (`grep -l "^pub trait emRec" crates/emcore/src/emRec.rs` matches).
- **I4a-2.** All six primitive files + emRecNode exist. ✓ (`emRec`, `emRecNode`, `emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec` — all PASS the `[ -f ]` check).
- **I4a-3.** Each concrete has a `SetValue`-fires-signal test (plus no-fire-on-no-change + clamp tests where applicable). ✓ Observed: emBoolRec 2 tests, emIntRec 5, emDoubleRec 5, emEnumRec 5 (6 matches incl. identifiers in assertions), emStringRec 3 = 20+ observational assertions.
- **I4a-4.** No golden regressions. ✓ (6 failed before, 6 failed after, same six tests).

## JSON entries closed

None. Phase 4a plan header: "JSON entries closed: none (E026 closes at Phase 4d gate; E027 closes at Phase 4d)." Partial-entry-close deferred.

## Closeout departures from strict ritual

- Per-task spec-compliance + code-quality review was applied fully to Task 1; for Tasks 2–7 the controller ran targeted per-task verification (gate + test reports) and consolidated the formal reviewer pass at end of Task 7. A reviewer approval with Minor-level doc nudges was applied as fixup `08995307` (added `TODO(phase-4b+)` markers for `Invert`, `SetToDefault`, `IsSetToDefault`, `TryStartReading`, serialization hooks across all 5 primitives).
- Predecessor phase-3.6.2 closeout (`2026-04-21-phase-3-6-2-closeout.md`) lacks the literal `Status: COMPLETE — all C1–C11 checks passed` string; 3.6.x ran as a sub-phase track outside strict ritual. Accepted at B4 because the predecessor's functional closeout (merge `c94eb3ae`, green gates, E040+E041 resolved) satisfied the ritual's intent. Documented in `2026-04-19-phase-4a-baseline.md`.
