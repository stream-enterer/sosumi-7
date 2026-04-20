# Phase 3 — Widget Signal Model + emFpPlugin API — Baseline

**Captured:** 2026-04-20
**Branch at capture:** main @ `a0640e75`
**Predecessor:** Phase 2 COMPLETE at `port-rewrite-phase-2-complete` (`047414f1`)

## nextest

`2458 tests run: 2458 passed, 9 skipped` (2458/0/9). 0 failed. Matches Phase 2 exit.

## goldens

`237 passed; 6 failed; 0 ignored; 0 measured`. Matches Phase 2 exit baseline (known 237/6 from 2026-04-18 emview closeout).

## clippy

Exit 0, no warnings. Clean.

## rc_refcell_total

`262`. Matches Phase 2 exit.

## diverged_total

`176`. Matches Phase 2 exit.

## rust_only_total

`17`. Matches Phase 2 exit.

## idiom_total

`0`. Matches Phase 2 exit (grep returned empty; no IDIOM: markers in tree).

## try_borrow_total

`0`. Matches Phase 2 exit (grep returned empty).

## Phase-1.5 precondition note

`rg -n 'pub(crate)?\s+pending_inputs' crates/emcore/src/emGUIFramework.rs` returned empty because the field is named `_pending_inputs` (underscore-prefix, unused) at `crates/emcore/src/emGUIFramework.rs:114,145`. Substance of the precondition is met — the field exists; it is just not yet wired. Phase 3 Task 1 Step 5 wires it (enqueue from winit callback) and the rename `_pending_inputs → pending_inputs` happens as part of that wiring. User confirmed Option A (proceed with deviation noted) on 2026-04-20.
