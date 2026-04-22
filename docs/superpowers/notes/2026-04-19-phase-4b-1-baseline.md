# Phase 4b.1 — Baseline

Captured at entry of phase-4b-1 bootstrap, from branch `main` at commit `2b7d3b18` prior to branch creation.

## nextest

```
Summary [  15.204s] 2550 tests run: 2550 passed, 9 skipped
```

0 failed.

## goldens

```
test result: FAILED. 237 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.09s
```

237 passed; 6 failed (matches Phase 4b closeout exit baseline).

## clippy

```
cargo clippy --all-targets --all-features
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.62s
```

Exit code 0.

## rc_refcell_total

339

## diverged_total

182

## rust_only_total

18

## idiom_total

0

## try_borrow_total

0
