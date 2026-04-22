# Phase 4d — emRec Persistence — Exit Metrics

**Captured:** 2026-04-22
**Branch:** port-rewrite/phase-4d
**Commits:** dafd6855..036dd6cb

## Gate

- `cargo fmt --check` — clean
- `cargo clippy --all-targets --all-features -- -D warnings` — clean
- `cargo-nextest ntr` — 2681 passed, 9 skipped, 0 failed
- `cargo test --test golden -- --test-threads=1` — 237 passed, 6 failed (baseline; no regressions)

## Counts

- nextest: 2681
- goldens passed: 237
- goldens failed: 6
- rc_refcell_total: 444
- diverged_total: 251
- rust_only_total: 18
- idiom_total: 0
- try_borrow_total: 0

## Delta from baseline

| Metric | Baseline | Exit | Δ |
|---|---|---|---|
| nextest | 2613 | 2681 | +68 |
| goldens passed | 237 | 237 | 0 |
| goldens failed | 6 | 6 | 0 |
| rc_refcell_total | 421 | 444 | +23 |
| diverged_total | 224 | 251 | +27 |
| rust_only_total | 18 | 18 | 0 |
| try_borrow_total | 0 | 0 | 0 |

**nextest**: +68 reflects round-trip coverage added across every primitive (Task 3) and every compound (Task 3 completion), file-backed I/O (Task 4), C++ fixture compat (Task 5), and emRecNodeConfigModel load-save (Task 6). Floor held: `cargo clippy --all-targets --all-features -- -D warnings` is now a gate target (Task 4 fixed the pre-existing `drop_non_drop` noise in the Task 3 round-trip suite).

**rc_refcell_total Δ +23**: emRecNodeConfigModel wires through `SchedCtx<'_>` which carries `Rc<RefCell<...>>` frames from emScheduler; test fixtures for the 9 new integration tests construct fresh scheduler contexts and account for all +23. No new Rc<RefCell> in the IO types themselves (all state lives in owned Vecs, structs, or the Lexer).

**diverged_total Δ +27**: every `TryRead`/`TryWrite` port of the C++ two-phase Start/Continue shape carries a `DIVERGED:` annotation; +27 matches the 11 concrete types × ~2.5 annotations per type (fusion note + occasional signature note).

**goldens** unchanged, as expected — Phase 4d touches no rendering pipeline.

## JSON entries closed

None. E026/E027 remain open and land at Phase 4e per plan header.

## Invariants (C4)

- **I4d-1** — Files exist: ✅
  - `crates/emcore/src/emRecReader.rs`
  - `crates/emcore/src/emRecWriter.rs`
  - `crates/emcore/src/emRecFileReader.rs`
  - `crates/emcore/src/emRecFileWriter.rs`
  - `crates/emcore/src/emRecMemReader.rs`
  - `crates/emcore/src/emRecMemWriter.rs`

- **I4d-2** — Round-trip test: serialize → parse → re-serialize produces identical bytes for every concrete type from Phases 4a-4c.
  - emBoolRec: `crates/emcore/tests/emrec_persistence_bool_roundtrip.rs` (Task 2 + fixup, write→read→write byte-stability asserted).
  - emIntRec, emDoubleRec, emStringRec, emEnumRec, emFlagsRec, emAlignmentRec, emColorRec: `crates/emcore/tests/emrec_persistence_roundtrip.rs` (Task 3 primitives, each test asserts byte-stability).
  - emUnionRec, emArrayRec, emTArrayRec, emStructRec: same file (Task 3 completion, byte-stability asserted via second-write comparison).
  - ✅

- **I4d-3** — Compatibility test reads a known C++-produced emRec file and asserts parsed values match expected.
  - `crates/emcore/tests/data/License.emVcItem` (Eagle Mode 0.96.4 verbatim) parsed by `crates/emcore/tests/emrec_persistence_cpp_compat.rs`. ✅

- **I4d-4** — `emConfigModel::LoadAndSave` method exists and wires through the IO classes.
  - Port-new decision (Drift note): `emRecNodeConfigModel` added at `crates/emcore/src/emRecNodeConfigModel.rs` alongside the legacy `emConfigModel` (which retains Record-trait callers across emmain/emfileman/emcore). `LoadAndSave` as a literal method name has no C++ counterpart (Task 6 grep of 0.96.4 source confirmed no hits); the semantics live in `TryLoad` / `TrySave(force)` / `TryLoadOrInstall(ctx)` on the new model, mirroring C++ `emConfigModel.cpp:24-114`. Round-trip test at `crates/emcore/tests/emrec_config_loadandsave.rs`. ✅ (resolved with documented name-divergence).
