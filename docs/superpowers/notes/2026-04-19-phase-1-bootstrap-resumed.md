# Phase 1 — Bootstrap Resumed (supersedes halt note)

**Date:** 2026-04-19
**Branch:** `port-rewrite/phase-1`
**Supersedes:** `2026-04-19-phase-1-bootstrap-blocked.md` (commit `61f2042`)

## Why the halt was lifted

The halt note raised two reasons:

**Reason 1 — per-task green gate incompatible with intermediate-red plan.** Resolved by the human's CLAUDE.md amendment (commit `8da45a1` on main, merged here): implementer subagents may now use `git commit --no-verify` for intermediate-red commits on this phase branch. Closeout C1 still enforces the full gate at the phase cliff.

**Reason 2 — scope/single-session budget.** The estimate was ex-ante and untested. Directive: proceed with Task 1 and see what actually happens. If budget exhaustion manifests concretely (e.g. "cannot fit dispatches for Task N"), halt at that point with evidence. Speculative halts on untested capacity are not a valid BLOCKED outcome.

## What carries forward from the halt note

- Baseline captured at the halt (still-valid numbers; no commits to main between halt and resume other than CLAUDE.md docs-only):
  - `cargo-nextest ntr`: 2451 passed / 9 skipped / 0 failed.
  - `cargo test --test golden -- --test-threads=1`: 237 passed / 6 failed (matches plan's expected baseline).
  - `cargo clippy --all-targets --all-features`: clean.
  - `rc_refcell_total`: 284.
  - `diverged_total`: 177.
  - `rust_only_total`: 16.
  - `idiom_total`: 1.
  - `try_borrow_total`: 11.
- Branch `port-rewrite/phase-1` exists from main.

## B7 recapture and B10–B12 execution

This resume supersedes the halt note's "B11/B12 not executed." Per the RESUME directive I re-run B7 in this session (numbers must match above within test-run noise), write the baseline.md, ledger.md, then B11 commits both, B12 announces, and Task 1 begins.

## Implementer subagent mandate (Phase 1 only)

Every implementer dispatch on this phase must include the line:

> You may use `git commit --no-verify` for intermediate-red commits on this phase branch (`port-rewrite/phase-1`); the CLAUDE.md prohibition has been lifted for this phase. Closeout C1 enforces the full gate at the phase cliff.

Without this, a spec-following implementer will STOP at a failed pre-commit hook.
