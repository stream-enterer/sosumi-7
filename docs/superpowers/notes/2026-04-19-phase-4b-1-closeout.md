# Phase 4b-1 — emColorRec / emAlignmentRec Migration — Closeout

**Branch:** port-rewrite/phase-4b-1
**Commits:** `233ebca0..5afd08bc` (bootstrap → final ledger entry; 7 commits)
**Status:** COMPLETE — all C1–C11 checks passed

## Summary

Phase 4b.1 ported `emAlignment` (u8 typedef + string conversions) and reified
`emAlignmentRec` / `emColorRec` as concrete `impl emRec<…>` types living in their
own `emAlignmentRec.rs` / `emColorRec.rs` files. The legacy hand-rolled versions
in `emRecRecTypes.rs` were deleted along with the now-unused `RecListenerList`
scaffolding; all three consumers migrated in a single step. Persistence for
both types is currently routed through free helpers (`em_alignment_{to,from}_rec_value`,
`em_color_{to,from}_rec_struct`) as a stopgap that retires when Phase 4d's
`TryRead` / `TryWrite` lands on the new `emRec`-shaped types. Goldens held at
237 passing / 6 failing (unchanged from baseline); nextest grew +12 with new
unit tests.

## Delta from baseline

- nextest: 2550 → 2562 (+12 passed, 0 failed)
- goldens: 237 passed / 6 failed — unchanged (meets >= / <= gate)
- rc_refcell_total: 339 → 351 (+12, new emRec-backed shared state)
- diverged_total: 182 (unchanged)
- rust_only_total: 18 (unchanged)
- idiom_total: 0 (unchanged)
- try_borrow_total: 0 (unchanged)

## JSON entries closed

none (E026/E027 close at Phase 4e)

## Spec sections implemented

§7 D7.1 (continued from Phase 4b)

## Invariants verified

- **I4b1-1** PASS — concrete `impl emRec<…>` for both types at canonical file paths.
- **I4b1-2** PASS — `emAlignment.rs` with `emAlignment=u8`, `EM_ALIGN_*`, to/from string.
- **I4b1-3** PASS — signal fire + no-fire-on-no-change tests in both *Rec.rs files.
- **I4b1-4** PASS — no consumer references to legacy `emRecRecTypes::em{Color,Alignment}Rec`.
- **I4b1-5** PASS — legacy structs removed; `RecListenerList` also removed crate-wide.
- **I4b1-6** PASS — TODO(phase-4e) marker present in `proofs_generated.rs`.
- **I4b1-7** PASS — no golden regressions (goldens held at baseline 237/6).

## Deferred / Tracking items for downstream phases

- **Kani regeneration (Phase 4e):** 7 legacy `emColorRec` harnesses deleted from `proofs_generated.rs`. New harnesses require `ConstructCtx` mock infrastructure. Regenerate alongside persistence work when the mock pattern is designed.
- **Alignment single-axis drift audit (Phase 4d or 4e):** New `emAlignmentRec` ships C++-faithful (u8 bitmask) but no consumer migrated. `emfileman::emFileManTheme` uses pre-existing Rust `emTiling::Alignment` single-axis enum (Start|Center|End|Stretch). Legacy `FromRecValue`/`ToRecValue` (now free functions `em_alignment_to_rec_value`/`em_alignment_from_rec_value` in `emRecRecTypes.rs`) retain the lossy C++-bitmask → Rust-enum mapping as a temporary stopgap. Phase 4d (persistence) or Phase 4e (emCoreConfig) must audit this drift against C++ and decide whether emFileManTheme migrates to `emAlignment` u8 or whether the Rust single-axis simplification becomes a chartered `DIVERGED:` annotation.
- **Stale .kani/provable_functions.json entries:** 10 `emAlignmentRec` entries + 1 `emColorRec::FromRecStruct` entry remain in the generated analysis cache. Will wash at next regeneration; do not hand-edit.
- **Persistence stopgap retirement:** Free helpers in `emRecRecTypes.rs` (`em_color_{to,from}_rec_struct`, `em_alignment_{to,from}_rec_value`) retire when Phase 4d's `TryRead`/`TryWrite` lands on the new `emColorRec`/`emAlignmentRec`.

## Next phase

Phase 4c — see `docs/superpowers/plans/2026-04-21-port-rewrite-phase-4c-emrec-compound-types.md`.
