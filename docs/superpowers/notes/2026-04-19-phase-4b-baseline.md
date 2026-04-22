# Phase 4b — Baseline (entry)

Captured 2026-04-21, on `main` at HEAD `9986b5ef` (immediately before creating `port-rewrite/phase-4b`).

## nextest

```
Summary [  15.182s] 2535 tests run: 2535 passed, 9 skipped
```

0 failed.

## goldens

```
test result: FAILED. 237 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.13s
```

237 passed / 6 failed — matches the known 2026-04-18 emview-closeout baseline.

## clippy

```
Checking test_plugin v0.1.0 (/home/a0/git/eaglemode-rs/crates/test_plugin)
Checking em-harness v0.1.0 (/home/a0/git/eaglemode-rs/harness)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.61s
```

Exit 0, no warnings.

## rc_refcell_total

```
rc_refcell_total: 329
```

## diverged_total

```
diverged_total: 182
```

## rust_only_total

```
rust_only_total: 18
```

## idiom_total

```
idiom_total: 0
```

(rg returned no matches; awk emitted an empty value, interpreted as 0.)

## try_borrow_total

```
try_borrow_total: 0
```

(rg returned no matches; awk emitted an empty value, interpreted as 0. Note: Phase 1 eliminated `try_borrow` from the codebase; subsequent phases inherit the cleaned state.)

## Notes on drift from B7 ground-truth reference (2026-04-19, pre-Phase-1)

Reference: `rc_refcell_total=284`, `try_borrow_total=11`, `diverged_total=177`, `rust_only_total=16`, `idiom_total=1`.

- `rc_refcell_total`: 329 vs 284 (+45). Later phases legitimately add Rc<RefCell<...>> in scoped places (notably 4a's emRec primitives and signal scaffolding); not regressed.
- `try_borrow_total`: 0 vs 11 (−11). Phase 1 eliminated `try_borrow` per spec.
- `diverged_total`: 182 vs 177 (+5). Expected as per-phase port work adds DIVERGED markers.
- `rust_only_total`: 18 vs 16 (+2). Expected.
- `idiom_total`: 0 vs 1 (−1). Marker eliminated by a prior phase's cleanup.

All deltas explainable by intervening phases (1, 2, 3, 4a). Nextest green, goldens at known baseline, clippy clean — no halt.
