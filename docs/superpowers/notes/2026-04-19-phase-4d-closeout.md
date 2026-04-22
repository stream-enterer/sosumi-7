# Phase 4d — emRec Persistence IO — Closeout

**Branch:** port-rewrite/phase-4d
**Commits:** dafd6855..036dd6cb
**Status:** COMPLETE — all C1–C11 checks passed

## Summary

Phase 4d ported the emRec persistence stack end-to-end: the `emRecReader`/`emRecWriter`
trait surface (Task 1) modelled on C++ `emRec.h:1545-1744`; the Mem/File backing
implementations (Tasks 2, 4) carrying the lexer/emitter state that C++ holds in the
base class; atomic `TryRead`/`TryWrite` methods on every concrete emRec type from
Phases 4a through 4c (Task 3, widening `emRecNode` with forwarding vtable slots);
a committed C++-produced fixture (`License.emVcItem` verbatim) parsed via the new
pipeline with format-header (`#%rec:FormatName%`) consumption (Task 5); and a
port-new companion `emRecNodeConfigModel<T: emRecNode>` wiring TryLoad / TrySave /
TryLoadOrInstall through the new IO stack while leaving the legacy
`emConfigModel<T: Record>` and its cross-crate callers untouched (Task 6).

Byte-format fidelity against C++ was validated at three levels: per-primitive
write→read→write byte-stability (11 types), compound structural output verified
by-hand against `emRec.cpp:1361-1489` (struct) / `1610-1655` (union) / `1820-1918`
(array), and round-trip through a real Eagle Mode 0.96.4 config file.

## Delta from baseline

| Metric | Baseline | Exit | Δ |
|---|---|---|---|
| nextest | 2613 | 2681 | +68 |
| goldens passed | 237 | 237 | 0 |
| goldens failed | 6 | 6 | 0 |
| rc_refcell_total | 421 | 444 | +23 |
| diverged_total | 224 | 251 | +27 |
| try_borrow_total | 0 | 0 | 0 |

See `2026-04-19-phase-4d-exit.md` for full accounting.

## JSON entries closed

None. E026/E027 remain open — they land at Phase 4e per plan header.

## Spec sections implemented

- §7 D7.1 — emRec persistence IO.

## Invariants verified

- **I4d-1** — all 6 IO files exist at `crates/emcore/src/`. ✅
- **I4d-2** — round-trip byte-stability asserted for every concrete type (11 types × at least one test each, plus the compound alias/edge tests). ✅
- **I4d-3** — `License.emVcItem` fixture + `emrec_persistence_cpp_compat.rs` integration test. ✅
- **I4d-4** — `emRecNodeConfigModel` provides `TryLoad`/`TrySave`/`TryLoadOrInstall`; literal `LoadAndSave` method absent because C++ has no such method (grep of 0.96.4 sources empty). The plan's spelling is a shorthand for the load-if-exists-else-install + save-if-dirty flow, which is covered. ✅

## Architectural decisions (load-bearing for Phase 4e)

- **Port-new over rewrite** for emConfigModel + emRecFileModel. The legacy Record-trait machinery (emRecParser::parse_rec/write_rec + emConfigModel<T: Record>) is consumed by 5+ callers in emmain/emfileman/emcore. Phase 4d added `emRecNodeConfigModel<T: emRecNode>` alongside. Phase 4e (emCoreConfig migration) will decide per-caller whether to migrate to the new shape or retire a use-site.
- **emRecNode trait widened** with `TryRead(&mut dyn emRecReader, &mut SchedCtx) -> Result<(), RecIoError>` and `TryWrite(&dyn emRecWriter) -> Result<(), RecIoError>`. Every concrete type provides forwarding impls to its inherent methods. Compound types (emUnionRec, emArrayRec, emTArrayRec) dispatch to child records through the widened vtable; emStructRec's sibling-field architecture (Phase 4c) dispatches through caller-provided closures over `try_read_body` / `try_write_body` helpers.
- **Format-header support** is in `emRecMemReader::with_format_header` and `emRecFileReader::open_with_format`, matching C++ `emRecReader::TryStartReading` (emRec.cpp:2004-2042). Magic is `#%rec:NAME%` (no trailing `#`); the trailing `#` in the on-disk spelling is absorbed by the lexer's `#`-to-EOL comment rule.
- **TryRead is atomic** (not two-phase). C++ `TryStartReading` + `TryContinueReading` (+ `QuitReading`) fused into one call per concrete type. DIVERGED-annotated at every port site. The scheduler provides cooperative yielding at a coarser granularity; incremental streaming of reads was never load-bearing for the observable contract.
- **`format_g_9` double formatter** (emRecMemWriter) implements `%.9G` emulation with explicit uppercase `E`, `-0.0` handling, and non-finite rejection. C-reference vector tests committed.
- **`TryWriteQuoted`** preserves embedded NULs via `\000` octal (DIVERGED: C++ truncates at NUL per C-string convention; Rust strings can legitimately carry NUL).

## Follow-ups (not blocking Phase 4e)

- Observer-driven dirty tracking in `emRecNodeConfigModel` (flagged `TODO(phase-4d-followup)` in source).
- Byte-stable re-emission of C++ input spellings (short-hex colors like `"#998"`, `.10434` without leading zero) — Rust normalises on re-write. Flagged in the compat test.
- Legacy emConfigModel + emRecParser retirement path, per-caller.
- `mem::forget(fx)` in three compound round-trip tests is a scheduler-queue bookkeeping shortcut (Task 3 review verdict: not a production leak). Replace with `scheduler.drain_pending()` when a suitable helper exists.

## Next phase

Phase 4e — emCoreConfig migration. See `docs/superpowers/plans/2026-04-19-port-rewrite-phase-4e-emcoreconfig-migration.md`.
