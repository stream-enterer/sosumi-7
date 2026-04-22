# Phase 5 — Async Plugins + Annotations Lint + CLAUDE.md Deltas — Baseline

**Captured:** 2026-04-22
**Branch:** port-rewrite/phase-5
**Predecessor:** Phase 4e (COMPLETE — all C1–C11 checks passed)

## nextest

2685 tests run: 2685 passed, 9 skipped, 0 failed

## goldens

237 passed; 6 failed (known baseline — unchanged from phase 4e exit)

## clippy

Exit 0 (no warnings under -D warnings)

## rc_refcell_total

461

## diverged_total

253

## rust_only_total

18

## idiom_total

0

## try_borrow_total

0

## Notes

- Phase plan cites 177 DIVERGED + 16 RUST_ONLY as pre-Phase-1 baseline. Post-4e actual is 253 + 18.
  Task 3 will need to tag all 253 DIVERGED and 18 RUST_ONLY blocks.
- 9 skipped tests are the plugin_invocation tests that require `cargo build -p test_plugin`
  and `LD_LIBRARY_PATH=target/debug` (pre-existing gap from Phase 3).
