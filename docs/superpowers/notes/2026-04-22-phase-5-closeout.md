# Phase 5 — Closeout Note

**Date:** 2026-04-22
**Branch:** port-rewrite/phase-5
**Predecessor:** Phase 4e (complete)

## Gate Results

| Gate | Result |
|------|--------|
| cargo fmt --check | PASS |
| cargo clippy -D warnings | PASS |
| cargo nextest | 2685 passed, 9 skipped, 0 failed |
| cargo test --test golden | 237 passed, 6 failed (known baseline — unchanged) |
| cargo xtask annotations | PASS (exit 0) |

## Invariants

| Invariant | Status |
|-----------|--------|
| I5a: `load_image_from_file` absent, LoaderEngine present | PASS |
| I5b: `cargo xtask annotations` exists and passes | PASS |
| I5c: All DIVERGED: blocks have forced-category tags | PASS |
| I5d: All RUST_ONLY: blocks have charter category tags | PASS |
| I5e: CLAUDE.md has §9.1/§9.2/§9.3 | PASS |
| I5f: Pre-commit hook runs `cargo xtask annotations` | PASS |

## Exit Metrics vs Baseline

| Metric | Baseline (4e exit) | Phase 5 exit | Delta |
|--------|--------------------|--------------|-------|
| nextest passed | 2685 | 2685 | 0 |
| nextest skipped | 9 | 9 | 0 |
| golden passed | 237 | 237 | 0 |
| golden failed | 6 | 6 | 0 |
| clippy warnings | 0 | 0 | 0 |
| DIVERGED: total | 253 | 239 | −14 |
| RUST_ONLY: total | 18 | 19 | +1 |
| Rc<RefCell< total | 461 | 455 | −6 |
| try_borrow total | 0 | 0 | 0 |
| IDIOM: total | 0 | 0 | 0 |

## Deliverables

1. **`cargo xtask annotations` lint binary** — walks `crates/**/*.rs`, checks DIVERGED: and RUST_ONLY: blocks for valid category tags in a 4-line window. Wired into pre-commit hook.
2. **All annotation blocks retrofitted** — 239 DIVERGED: blocks carry `(language-forced)`, `(dependency-forced)`, `(upstream-gap-forced)`, or `(performance-forced)`. 19 RUST_ONLY: blocks carry `(language-forced-utility)`, `(performance-forced-alternative)`, or `(dependency-forced)`. 14 mislabeled DIVERGED: blocks converted to `BLOCKED:` or plain comments.
3. **`emImageFileModel` async loading** — `LoaderEngine` (implements `emEngine`) loads TGA on scheduler time-slice, fires `FileStateSignal` + `load_complete_signal`, removes itself. `register()` factory method added. Full behavioral test coverage including async success, async failure, and all setter invariants.
4. **CLAUDE.md §9.1/§9.2/§9.3** — Ownership section updated; new `## Annotation Vocabulary` section; Forced divergence test expanded to four named categories.

## JSON Entries Closed

- E028 (Task 5: emImageFileModel async) — RESOLVED
- E035 (Tasks 2–4+7: annotation lint + retrofit) — RESOLVED
- E037 (Tasks 2–4+7: annotation lint + retrofit) — RESOLVED

## Open Items (non-blocking)

- 4 blocks in `emProcess.rs:539`, `emMiniIpc.rs:438`, `emLabel.rs:118`, `emListBox.rs:1113` may warrant `RUST_ONLY:` reclassification. Deferred to follow-up.
