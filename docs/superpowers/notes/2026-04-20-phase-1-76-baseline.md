# Phase 1.76 — Baseline

**Captured:** 2026-04-20
**Branch:** port-rewrite/phase-1-76 (off main `458f1fe0`)
**Predecessor:** Phase 1.75 COMPLETE at tag `port-rewrite-phase-1-75-complete` (`50cba0d3`)

## nextest

```
Summary [ 15.918s] 2454 tests run: 2454 passed, 9 skipped
```

- passed: 2454
- failed: 0
- skipped: 9

## goldens

```
test result: FAILED. 237 passed; 6 failed; 0 ignored
```

- passed: 237
- failed: 6 (inherited — identical failure set as Phase 1.75 exit)

## clippy

Clean (`Finished dev profile` — no warnings).

## Code metrics

- `rc_refcell_total`: 283
- `diverged_total`: 176
- `rust_only_total`: 17
- `idiom_total`: 0
- `try_borrow_total`: 0

All metrics match Phase 1.75 exit counts verbatim.
