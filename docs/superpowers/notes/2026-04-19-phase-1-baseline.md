# Phase 1 — Baseline (captured at B7, after RESUME)

**Captured:** 2026-04-19 (resume session)
**Branch:** `port-rewrite/phase-1` (fresh from main + CLAUDE.md amendment merge)

## nextest

```
2451 tests run: 2451 passed, 9 skipped, 0 failed
```

## goldens

```
237 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

Matches the plan's documented baseline (B8 green).

## clippy

```
Finished `dev` profile [unoptimized + debuginfo] target(s)
```

No warnings emitted (clean).

## rc_refcell_total

```
284
```

(Plan §3.6 states "current: 155" — plan figure is stale; actual is 284. Delta target for Phase 1 is ≥ −1, which is unchanged.)

## diverged_total

```
177
```

Phase 1 delta target: ≥ −6 (SP4/SP4.5/SP8 DIVERGED blocks that dissolve).

## rust_only_total

```
16
```

## idiom_total

```
1
```

Sole occurrence at `crates/emcore/src/emView.rs` (the `IDIOM:` block near `SchedOp`). Phase 1 invariant I5 requires this to drop to 0.

## try_borrow_total

```
11
```

Phase 1 delta target: ≥ −40 (per plan, via SchedOp/try_borrow removals). NOTE: the baseline of 11 is smaller than the −40 target; see ledger task-4/5 notes for reconciliation. [Phase-1 plan header line 27: "try_borrow_total drops by ≥ 40 from baseline".] At 11 starting occurrences, the deletion can at most drop to 0, i.e. delta = −11. This is a plan-figure staleness, not a blocker — the operational target is "invariant I1d: zero try_borrow in emView.rs and emPanelTree.rs", which is graspable regardless of the aggregate count.
