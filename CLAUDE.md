# eaglemode-rs

A zoomable UI framework — reimplementation of Eagle Mode's emCore in Rust.

## Commands

```bash
cargo check
cargo clippy -- -D warnings
cargo-nextest ntr
```

## Pre-commit hook

Runs `cargo fmt` (auto-applied) then `clippy -D warnings` then `cargo-nextest ntr`.
Do not skip with `--no-verify`. If a commit fails, fix the cause and retry.

## Code Rules

For any task of the form "identify/filter/classify code by property P": Is P a property that an existing tool computes exactly? If yes, your approach is: generate inputs → invoke tool on all inputs → parse structured output. Do not write a custom filter.
- Abstract example: To find which functions in a codebase satisfy a type-level constraint, don't scan source text for signatures — generate a file that attempts to use each function in the constrained context, compile it, and parse the errors. The compiler is the arbiter of type-level properties.
- Concrete example: To find which functions can have Kani harnesses generated, don't grep for "pure" functions or filter by name/signature patterns. Generate a harness for every function, compile them all, and let rustc and Kani tell you which ones are valid. Parse their structured error output into your inventory.

- **Types & coordinates**: `f64` logical, `i32` pixel, `u32` image dims, `u8` color channels.
- **Color**: `Color` (packed u32 RGBA) for storage. Intermediate blend math in `i32` or wider.
- **Ownership**: `Rc`/`RefCell` shared state, `Weak` parent refs.
- **Strings**: `String` owned, `&str` params. Convert with `.to_string()`.
- **Errors**: Per-module `Result` with custom error enums (`Display` + `Error`). `assert!` only for logic-error invariants.
- **Imports**: std → external → `crate::`. Explicit names. `use super::*` only in `#[cfg(test)]`.
- **Construction**: `new()` primary, builder `with_*(self) -> Self` for optional config.
- **Modules**: One primary type per file. Private `mod` + public `use` re-exports in `mod.rs`.
- **Visibility**: `pub(crate)` default. `pub` only for the library's public API.
- **Unwrap**: `expect("reason")` unless invariant is obvious from context. Bare `unwrap()` fine in tests and same-line proofs.
- **Warnings**: Fix the cause (remove dead code, prefix `_`, apply clippy fix). Suppress only genuine false positives with a comment.

## C++ Reference Source

Eagle Mode 0.96.4 source is at `~/git/eaglemode-0.96.4/` (headers in `include/emCore/`, implementation in `src/emCore/`).

## File and Name Correspondence

The codebase should be a transparent overlay on the C++ original. A developer holding `emFoo.h` open should be able to find `emFoo.rs`, scan for the method name, and land on the corresponding code without guessing, searching, or asking. Where the overlay can't be 1:1, the exception is marked at the exact point of divergence with the C++ name and the reason, so the developer never has to wonder whether something was missed or renamed silently.

Every C++ header in `include/emCore/` has exactly one Rust file in `src/emCore/` with the same name (`emFoo.h` → `emFoo.rs`), containing all types and methods from that header with identical names (`class emColor` → `struct emColor`, `GetRed` → `GetRed`). Where Rust requires splitting one C++ file into multiple Rust files (e.g., the Modules rule "one primary type per file" requires it), the primary file keeps the C++ name and the splits are named `emFoo{Suffix}.rs` where the suffix is derived from the existing Rust filename, with a `SPLIT:` comment at the top explaining why. Where a Rust method or type cannot keep the C++ name, it has a `DIVERGED:` comment at its definition with the C++ name and the reason. Everything not annotated `SPLIT:` or `DIVERGED:` is 1:1 by name.

Filesystem markers in `src/emCore/`:
- C++ headers with no Rust equivalent (e.g., `emArray.h` → `Vec<T>`) get an empty marker file: `emArray.no_rust_equivalent`. Lists all exempt headers visibly on the filesystem.
- Rust files with no C++ header get an empty marker file alongside them: `rect.rust_only`. Identifies Rust-only code visibly on the filesystem.

## Port Fidelity

eaglemode-rs is a port of Eagle Mode's emCore. Golden tests compare Rust pixel output against C++ reference data. The fidelity rules depend on what layer the code is in.

**Pixel arithmetic** (blend, coverage, interpolation, sampling): Reproduce C++ integer formulas exactly. Use `(x*257+0x8073)>>16` not `f64` division. Wrap in newtypes (`Fixed12`, `div255_round()`). Named constants with derivations: `const BLINN_BIAS: u32 = 0x8073; // (128 * 257 + 1) / 2`. No f64 approximations in the compositing pipeline.

**Geometry** (coordinates, rects, transforms, layout): Same algorithm and operation order on golden-tested paths. `Iterator::sum` OK (left-fold matches C++ loop). Clamp/min/max must preserve C++ boundary values.

**State logic, data structures, ownership, API surface**: Fully idiomatic Rust, subject to File and Name Correspondence (names and file structure match C++ even when Rust idiom would differ). Golden tests verify output, not structure. Preserve behavioral contracts (return values, side effects, ordering). Adapt syntax freely where it does not break name correspondence.

**When in doubt**: Check if the function's output feeds a golden test. If yes → port the C++ formula exactly. If no → write idiomatic Rust.

## Golden Tests

- Run: `cargo test --test golden -- --test-threads=1`
- Comparison functions in `tests/golden/common.rs`: pixel (ch_tol + max_fail_pct), rect (f64 eps), behavioral/notice/input (exact), trajectory (f64 tol).

### Verification tooling

- **Full single-test workflow**: `scripts/verify_golden.sh <name>` — dumps Rust ops, diffs against C++ ops, generates debug PPM images on failure, prints divergence report. Add `--regions` for spatial breakdown.
- **Analyze all failures**: `scripts/verify_golden.sh --all` — runs full suite with DUMP_DRAW_OPS, diffs every failing test.
- **Quick status**: `scripts/verify_golden.sh --report` — runs full suite + prints divergence table.
- **Regenerate C++ baseline**: `scripts/verify_golden.sh --regen` — rebuilds gen_golden and regenerates golden data + C++ ops. Use only when intentionally updating the baseline; overwrites golden data files.

### DrawOp diff (`scripts/diff_draw_ops.py`)

Compare C++ vs Rust paint call parameters op-by-op. Diagnose golden failures mechanically — do not form hypotheses from code reading.

- **Summary only**: `python3 scripts/diff_draw_ops.py <name> --no-table`
- **With spatial regions**: `python3 scripts/diff_draw_ops.py <name> --regions`
- **Machine-readable summary**: `python3 scripts/diff_draw_ops.py <name> --summary-json`
- **Machine-readable divergences**: `python3 scripts/diff_draw_ops.py <name> --json`
- **Include sub-ops**: `python3 scripts/diff_draw_ops.py <name> --all-depths` (default: depth 0 only — sub-ops are implementation details that legitimately differ between C++ and Rust)
- **Include format noise**: `python3 scripts/diff_draw_ops.py <name> --verbose` (shows `_hex` fields, `state_alpha`, PaintTextBoxed format-only fields)
- Requires ops JSONL files in `target/golden-divergence/`. Generate C++ ops: `make -C tests/golden/gen && make -C tests/golden/gen run`. Generate Rust ops: `DUMP_DRAW_OPS=1 cargo test --test golden <name> -- --test-threads=1`.

### Divergence report (`scripts/divergence_report.py`)

Parse `target/golden-divergence/divergence.jsonl` (auto-generated at zero tolerance by every golden test run).

- **Status table**: `python3 scripts/divergence_report.py`
- **Compare against previous run**: `python3 scripts/divergence_report.py --diff`
- **Machine-readable**: `python3 scripts/divergence_report.py --json`
- **Divergent tests only**: `python3 scripts/divergence_report.py --failing`

### Debug images

- `DUMP_GOLDEN=1 cargo test --test golden <name>` — writes `target/golden-debug/{actual,expected,diff}_<name>.ppm`

## Do NOT

- `#[allow(...)]` / `#[expect(...)]` — fix the warning instead, UNLESS warning is for too many arguments (which is allowed), `non_snake_case` on the `emCore` module, or `non_camel_case_types` on `em`-prefixed types (both required by File and Name Correspondence).
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
