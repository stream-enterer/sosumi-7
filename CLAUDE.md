# sosumi-7

Cargo workspace with two crates:
- `zuicchini/` — UI framework library (reimplementation of Eagle Mode's emCore in Rust)
- `sosumi-7/` — game binary, depends on zuicchini via path

## Commands

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo-nextest ntr --workspace
cargo run -p sosumi-7
```

## Pre-commit hook

Runs `cargo fmt` (auto-applied) then `clippy -D warnings` then `cargo-nextest ntr`.
Do not skip with `--no-verify`. If a commit fails, fix the cause and retry.

## Code Rules

- **Types & coordinates**: `f64` logical, `i32` pixel, `u32` image dims, `u8` color channels.
- **Color**: `Color` (packed u32 RGBA) for storage. Intermediate blend math in `i32` or wider.
- **Ownership**: `Rc`/`RefCell` shared state, `Weak` parent refs.
- **Strings**: `String` owned, `&str` params. Convert with `.to_string()`.
- **Errors**: Per-module `Result` with custom error enums (`Display` + `Error`). `assert!` only for logic-error invariants.
- **Imports**: std → external → `crate::`. Explicit names. `use super::*` only in `#[cfg(test)]`.
- **Construction**: `new()` primary, builder `with_*(self) -> Self` for optional config.
- **Modules**: One primary type per file. Private `mod` + public `use` re-exports in `mod.rs`.
- **Visibility**: `pub(crate)` default. `pub` only for library API consumed by `sosumi-7`.
- **Unwrap**: `expect("reason")` unless invariant is obvious from context. Bare `unwrap()` fine in tests and same-line proofs.
- **Warnings**: Fix the cause (remove dead code, prefix `_`, apply clippy fix). Suppress only genuine false positives with a comment.

## Port Fidelity (zuicchini)

zuicchini is a port of Eagle Mode's emCore. Golden tests compare Rust pixel output against C++ reference data. The fidelity rules depend on what layer the code is in.

**Pixel arithmetic** (blend, coverage, interpolation, sampling): Reproduce C++ integer formulas exactly. Use `(x*257+0x8073)>>16` not `f64` division. Wrap in newtypes (`Fixed12`, `div255_round()`). Named constants with derivations: `const BLINN_BIAS: u32 = 0x8073; // (128 * 257 + 1) / 2`. No f64 approximations in the compositing pipeline.

**Geometry** (coordinates, rects, transforms, layout): Same algorithm and operation order on golden-tested paths. `Iterator::sum` OK (left-fold matches C++ loop). Clamp/min/max must preserve C++ boundary values.

**State logic, data structures, ownership, API surface**: Fully idiomatic Rust. Golden tests verify output, not structure. Preserve behavioral contracts (return values, side effects, ordering). Adapt syntax freely.

**When in doubt**: Check if the function's output feeds a golden test. If yes → port the C++ formula exactly. If no → write idiomatic Rust.

## Golden Tests

- Run: `MEASURE_DIVERGENCE=1 cargo test --test golden -- --test-threads=1`
- Diff images: `DUMP_GOLDEN=1 cargo test --test golden <name>`
- Generator: `make -C zuicchini/tests/golden/gen && make -C zuicchini/tests/golden/gen run`
- Comparison functions in `zuicchini/tests/golden/common.rs`: pixel (ch_tol + max_fail_pct), rect (f64 eps), behavioral/notice/input (exact), trajectory (f64 tol).

## Do NOT

- `#[allow(...)]` / `#[expect(...)]` — fix the warning instead, UNLESS warning is for too many arguments (which is allowed).
- `Arc` / `Mutex` — single-threaded UI tree
- `Cow` — use `String` / `&str`
- Glob imports (`use foo::*`) — except `use super::*` in tests
- Truncate color math to `u8` mid-calculation
- `f64` in blend/coverage/interpolation paths — use C++ integer arithmetic
- Rayon / parallel iteration on golden-tested code paths
- `assert!` for recoverable errors
- `--no-verify` on commits

## Plan Tool Rules

- **When writing plans**: Plans must be phased, gated, and hardened against LLM failure modes and anti-patterns.
