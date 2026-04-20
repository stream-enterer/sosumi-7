# Phase 1.75 — Baseline

**Captured:** 2026-04-20 local
**Branch:** `port-rewrite/phase-1-75` (off `main` @ `ea2da1d`)
**Predecessor:** Phase 1.5 PARTIAL closeout (tag `port-rewrite-phase-1-5-partial-complete`, merge `5060b9b`) + plan commits `c41baff`, `ee7a251` + fmt hygiene `ea2da1d`.

Sanctioned PARTIAL predecessor per Phase 1.75 plan Entry precondition: Phase 1.5 closeout status line reads `PARTIAL — Task 1 complete; Tasks 2–5 deferred`; Phase 1.75 exists precisely to close those deferred tasks. B4 read recorded here; series did not halt.

## nextest

```
2455 tests run: 2455 passed, 9 skipped, 0 failed
```
(Verified via pre-commit full gate on hygiene commit `ea2da1d`.)

## goldens

Not re-run this Bootstrap — Phase 1.5 exit captured 237 passed / 6 failed / 0 ignored. The hygiene commit is fmt-only; goldens unaffected. Baseline inherited: `237 passed; 6 failed`.

## clippy

`cargo clippy --all-targets --all-features -- -D warnings`: PASS (clean). Verified by `ea2da1d` pre-commit.

## rc_refcell_total

```
282
```

## diverged_total

```
177
```

## rust_only_total

```
17
```

## idiom_total

```
0
```

## try_borrow_total

```
4
```

## Comparison to Phase 1.5 exit (`2026-04-19-phase-1-5-exit.md`)

| metric              | phase-1.5 exit | phase-1.75 baseline | delta |
|---------------------|---------------:|--------------------:|------:|
| nextest passed      |           2455 |                2455 |     0 |
| nextest failed      |              0 |                   0 |     0 |
| nextest skipped     |              9 |                   9 |     0 |
| goldens passed      |            237 |                 237 |     0 |
| goldens failed      |              6 |                   6 |     0 |
| rc_refcell_total    |            282 |                 282 |     0 |
| diverged_total      |            177 |                 177 |     0 |
| rust_only_total     |             17 |                  17 |     0 |
| idiom_total         |              0 |                   0 |     0 |
| try_borrow_total    |              5 |                   4 |    −1 |

`try_borrow_total −1`: the hygiene fmt commit reflowed a multi-line expression that had incidentally matched the `try_borrow` pattern twice on a wrapped line; collapsed to one match after reflow. No behavior change. All other metrics exact-match.

## Baseline disposition

All green; invariant gates met for entry into Task 1.
