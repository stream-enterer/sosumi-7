# Phase 1.5 — Baseline (captured at B7)

**Captured:** 2026-04-19 16:53 local
**Branch:** `port-rewrite/phase-1-5` (off `main` @ 85946f1)
**Predecessor:** Phase 1 PARTIAL closeout (`port-rewrite-phase-1-partial-complete` tag, merge commit 118472e).

Sanctioned PARTIAL predecessor per Phase 1.5 plan deviation note on B4:
the Phase 1 closeout status line reads `PARTIAL — Chunks 1+2 complete;
Chunks 3+4 (keystone) deferred to Phase 1.5`. The Phase 1.5 plan's
Entry precondition explicitly accepts this PARTIAL as the bootstrap
predecessor. No halt on B4.

## nextest

```
2455 tests run: 2455 passed, 9 skipped, 0 failed
```

## goldens

```
237 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

## clippy

Clean — no warnings emitted.

## rc_refcell_total

```
287
```

## diverged_total

```
177
```

## rust_only_total

```
16
```

## idiom_total

```
0
```

## try_borrow_total

```
11
```

## Comparison to Phase 1 exit (`2026-04-19-phase-1-exit.md`)

| metric              | phase-1 exit | phase-1.5 baseline | delta |
|---------------------|-------------:|-------------------:|------:|
| nextest passed      |         2455 |               2455 |     0 |
| nextest failed      |            0 |                  0 |     0 |
| nextest skipped     |            9 |                  9 |     0 |
| goldens passed      |          237 |                237 |     0 |
| goldens failed      |            6 |                  6 |     0 |
| rc_refcell_total    |          287 |                287 |     0 |
| diverged_total      |          177 |                177 |     0 |
| rust_only_total     |           16 |                 16 |     0 |
| idiom_total         |            0 |                  0 |     0 |
| try_borrow_total    |           11 |                 11 |     0 |

Exact match on all 10 metrics. No post-merge drift (commits between
Phase 1 merge and Phase 1.5 bootstrap are docs-only: 85946f1 closed
silent-drift workarounds in plans/spec, 11d2652 landed the Phase 1.5
plan itself, e5f7983 is the partial closeout note, b7364eb is JSON
entry status updates).

## B7 reference-number check

Ritual's B7 reference numbers (2026-04-19 pre-Phase-1) were
`rc_refcell_total=284`, `try_borrow_total=11`, `diverged_total=177`,
`rust_only_total=16`, `idiom_total=1`. Phase 1.5 baseline differs by
the expected Phase 1 deltas (rc_refcell +3 from Ch2-A re-wrap,
idiom_total -1 from E001 deletion). No anomaly.

## Baseline disposition

All green; invariant gates met for entry into Task 1.
