# Phase 2 — View/Window Composition — Entry Baseline

**Captured:** 2026-04-20
**Branch at capture:** main @ 810481ad (pre-B9)
**Predecessor:** `port-rewrite-phase-1-76-complete` @ 253bfe97 (merged at a7d33e55)

## nextest

```
Summary [20.745s] 2454 tests run: 2454 passed, 9 skipped
```

## goldens

```
test result: FAILED. 237 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

(6 known-failing goldens inherited from pre-Phase-1 state; baseline-equal.)

## clippy

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.57s
```

(no warnings; exit 0.)

## rc_refcell_total

283

## diverged_total

176

## rust_only_total

17

## idiom_total

0

## try_borrow_total

0

## Toolchain

rustc 1.93.1 — satisfies the Rust 1.86+ requirement for `HashMap::get_disjoint_mut` (Task 9).

## Notes

- Inherited-equal with Phase 1.76 closeout. No drift.
- Metrics targets for Phase 2 exit:
  - `rc_refcell_total`: substantial decrease (handoff estimate −20 to −40).
  - `diverged_total`: decrease (DIVERGED blocks deleted in Task 10).
  - `rust_only_total`: +1 (new `emPanelScope.rust_only`).
  - `try_borrow_total`: remains 0.
