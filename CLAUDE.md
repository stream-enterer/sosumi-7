# eaglemode-rs

A zoomable UI framework — reimplementation of Eagle Mode's emCore in Rust.

## Port Ideology

eaglemode-rs is an **observational port** of emCore. From a user's seat — visible behavior, event order, timing, signals, focus, input, pixel output — Rust and C++ must be indistinguishable. Not "same output": same what-fires-when, same what-is-true-at-each-observable-moment.

**Authority order** (higher wins):
1. C++ source — ground truth for behavior, algorithms, and design. Eagle Mode 0.96.4 at `~/Projects/eaglemode-0.96.4/` (headers in `include/emCore/`, implementation in `src/emCore/`).
2. Golden tests — the mechanical arbiter of observable equivalence.
3. Rust idiom — applies only below the observable surface, and only where C++ structure is not load-bearing.
4. LLM judgment / convenience — lowest. Never outranks the above.

**Classify every difference from C++** (the first two are *divergences* — observable or structural departures. The third is not):
- **Forced divergence** — one of the following four categories applies:
  1. **Language-forced.** Try writing the C++ shape in Rust under the project's canonical ownership model (CLAUDE.md §Ownership). If it does not compile, language-forced.
  2. **Dependency-forced.** A required dependency (wgpu, winit) cannot be made to admit the C++ shape through its public API.
  3. **Upstream-gap-forced.** C++ emCore itself ships the shape as a no-op.
  4. **Performance-forced.** Benchmark demonstrates the C++-mirrored shape crossing a documented degradation threshold; the alternative must ship the benchmark and threshold.

  "Idiom adaptation *forced by* a project-internal ownership choice" is not a valid framing. If a Rust choice makes a C++ shape impossible, revisit the Rust choice before marking forced.
- **Preserved design intent** — a deliberate architectural choice by the C++ author. Preserve; it is load-bearing.
- **Idiom adaptation** — below the observable surface, outside name correspondence. Adapt syntax and ownership freely; behavior stays identical. Not a divergence.

**Failure modes this prevents:**
- Collapsing *preserved design intent* into *forced divergence*: using one real constraint as license to redesign surrounding C++ structure.
- Treating Rust convenience as a reason to diverge (observably or structurally). Convenience is never a reason.
- Speculating about author intent instead of reading C++ to confirm.
- Preserving Rust out of inertia when it diverges from C++. Rust is defective by default; C++ is the reference.

**When unsure whether a difference is forced or design intent:** assume design intent, match C++ exactly, and mark the point of departure explicitly. Silent drift is worse than verbose preservation.

## Commands

```bash
cargo check
cargo clippy -- -D warnings
cargo-nextest ntr
```

## Pre-commit hook

Runs `cargo fmt` (auto-applied) then `clippy -D warnings` then `cargo-nextest ntr`.

## Code Rules

For "identify/filter/classify code by property P" tasks where P is computed exactly by an existing tool: generate inputs → run the tool on all inputs → parse structured output. Don't write a custom filter. Example: to find which functions can have Kani harnesses generated, don't grep for "pure" signatures — generate a harness for every function, compile them all, and parse rustc/Kani's errors.

- **Types & coordinates**: `f64` logical, `i32` pixel, `u32` image dims, `u8` color channels.
- **Color**: `Color` (packed u32 RGBA) for storage. Intermediate blend math in `i32` or wider.
- **Ownership**: Plain owned values are the default. `Rc<T>` (immutable) where the value is shared-read after init. `Rc<RefCell<T>>` requires a justification comment citing one of: (a) cross-closure reference held by winit/wgpu callbacks, or (b) context-registry typed singleton. `Weak<RefCell<T>>` is acceptable only as the pair of an (a)-justified `Rc<RefCell<T>>`. Engine and panel back-references to their owning view or window are IDs (`WindowId`, `PanelId`) resolved through `EngineCtx`, not `Weak<>`.
- **Strings**: `String` owned, `&str` params. Convert with `.to_string()`.
- **Errors**: Per-module `Result` with custom error enums (`Display` + `Error`). `assert!` only for logic-error invariants.
- **Imports**: std → external → `crate::`. Explicit names. `use super::*` only in `#[cfg(test)]`.
- **Construction**: `new()` primary, builder `with_*(self) -> Self` for optional config.
- **Modules**: One primary type per file. Private `mod` + public `use` re-exports in `mod.rs`.
- **Visibility**: `pub(crate)` default. `pub` only for the library's public API.
- **Unwrap**: `expect("reason")` unless invariant is obvious from context. Bare `unwrap()` fine in tests and same-line proofs.
- **Warnings**: Fix the cause (remove dead code, prefix `_`, apply clippy fix). Suppress only genuine false positives with a comment.

## File and Name Correspondence

The codebase is a transparent overlay on the C++ original: a developer with `emFoo.h` open finds `emFoo.rs`, scans for the method name, and lands on the corresponding code without guessing. Every exception is marked at the point of divergence with the C++ name and the reason — nothing is renamed silently.

- **1:1 by default**: each `include/emCore/emFoo.h` → exactly one `src/emCore/emFoo.rs` with identical type and method names (`class emColor` → `struct emColor`, `GetRed` → `GetRed`).
- **Splits**: when Modules' "one primary type per file" forces splitting, the primary file keeps the C++ name; splits are named `emFoo{Suffix}.rs` with a `SPLIT:` comment at the top explaining why.
- **Renames**: any Rust method or type that can't keep the C++ name carries a `DIVERGED:` comment at its definition with the C++ name and the reason.
- Anything not annotated `SPLIT:` or `DIVERGED:` is 1:1 by name.

Filesystem markers in `src/emCore/`:
- C++ headers with no Rust equivalent (e.g., `emArray.h` → `Vec<T>`) get an empty marker file: `emArray.no_rust_equivalent`. Lists all exempt headers visibly on the filesystem.
- Rust files with no C++ header get an empty marker file alongside them: `rect.rust_only`. Identifies Rust-only code visibly on the filesystem.

## Annotation Vocabulary

- `DIVERGED:` marks a forced divergence per Port Ideology §"Forced divergence". Every block must name which forced category applies (language-forced, dependency-forced, upstream-gap-forced, performance-forced) and cite the test result. Blocks without a category are treated as fidelity-bugs and are fixed, not annotated.
- `RUST_ONLY:` marks code with no C++ analogue with a chartered justification: language-forced utility (typed wrapper that C++ inline code implicitly provides), dependency-forced alternative, or performance-forced alternative (with benchmark).
- `IDIOM:` is retired. Below-surface adaptations that preserve observable behavior and introduce no structural commitment are unannotated. If the adaptation needs a comment, write prose explaining the rationale without the tag.
- `UPSTREAM-GAP:` marks code that mirrors a C++ no-op/stub because upstream itself is a no-op. Preserves upstream semantics.
- `SPLIT:` marks file splits forced by "one primary type per file". Unchanged.

Annotation lint runs as a standalone `cargo xtask annotations` binary (stable-rustc compatible; text-scan over `rg -n 'DIVERGED:'` / `RUST_ONLY:` matches, validating each hit carries a required category tag). Invoked from the pre-commit hook and from CI. Not a clippy lint — stable Rust does not admit custom clippy lints without switching to nightly, which is out of scope.

## Port Fidelity

Concretization of the Port Ideology by code layer. Golden tests compare Rust pixel output against C++ reference data; the fidelity rules depend on what layer the code is in.

**Pixel arithmetic** (blend, coverage, interpolation, sampling): Reproduce C++ integer formulas exactly. Use `(x*257+0x8073)>>16` not `f64` division. Wrap in newtypes (`Fixed12`, `div255_round()`). Named constants with derivations: `const BLINN_BIAS: u32 = 0x8073; // (128 * 257 + 1) / 2`. No f64 approximations in the compositing pipeline.

**Geometry** (coordinates, rects, transforms, layout): Same algorithm and operation order on golden-tested paths. `Iterator::sum` OK (left-fold matches C++ loop). Clamp/min/max must preserve C++ boundary values.

**State logic, data structures, ownership, API surface**: Fully idiomatic Rust, subject to File and Name Correspondence (names and file structure match C++ even when Rust idiom would differ). Golden tests verify output, not structure. Preserve behavioral contracts (return values, side effects, ordering). Adapt syntax freely where it does not break name correspondence.

**Layer test**: if the function's output feeds a golden test → port the C++ formula exactly. Otherwise → idiomatic Rust is permitted (subject to Port Ideology).

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

## Plan Tool Rules

- **When writing plans**: Plans must be phased, gated, and hardened against LLM failure modes and anti-patterns.
