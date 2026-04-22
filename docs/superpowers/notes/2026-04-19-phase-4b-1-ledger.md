# Phase 4b.1 — emColorRec / emAlignmentRec Migration — Ledger

**Started:** 2026-04-21 18:43 local
**Branch:** port-rewrite/phase-4b-1
**Baseline:** see 2026-04-19-phase-4b-1-baseline.md
**Spec sections:** §7 D7.1 (continued from Phase 4b)
**JSON entries to close:** none (E026/E027 close at Phase 4e)

## Task log

- **Task 1** — port emAlignment u8 typedef + string conversions — `2fc019fa`
- **Task 2** — port emAlignmentRec (new concrete emRec type) — `965ec395`
- **Task 3** — port emColorRec (new concrete emRec type) — `b1f328c5`
- **Task 3b** — emColorRec: remove non-C++ HaveAlpha getter — `2e30b528`
- **Task 4** — migrate consumers (emVirtualCosmos, emBookmarks, emFileManTheme) to stopgap free helpers (`em_color_to_rec_struct`, `em_color_from_rec_struct`, `em_alignment_to_rec_value`, `em_alignment_from_rec_value`) in `emRecRecTypes.rs`; delete legacy value-typed `emColorRec` / `emAlignmentRec` structs + impls + `Default for emAlignmentRec`; remove dead `RecListenerList` (no remaining consumers); delete 7 kani harnesses + 8 provable_functions.json entries + 1 constructible_types entry targeting legacy `emColorRec`. — `71a1efdc`

## Deferred / Tracking

- **Kani regeneration (Phase 4e):** 7 legacy `emColorRec` kani harnesses were deleted in Task 4 rather than migrated. Regenerate against the new ctx-constructed `emColorRec` / `emAlignmentRec` once a `ConstructCtx` mock pattern for Kani is designed. Tracking marker: `TODO(phase-4e)` comment at the top of `crates/eaglemode/tests/kani/proofs_generated.rs` (around line 748).
- **Alignment single-axis drift (Phase 4e+):** `emAlignmentRec` (new, u8 / C++-faithful) was built in Task 2 but no consumer was migrated to it. The one alignment consumer (`emFileManTheme`) uses the pre-existing Rust-only `emTiling::Alignment` single-axis enum (Start|Center|End|Stretch) rather than the C++ `emAlignment` bitmask byte. The Phase 4b.1 stopgap `em_alignment_{to,from}_rec_value` preserves the legacy lossy single-axis serialization exactly. A systemic alignment-drift audit — replacing `emTiling::Alignment` with `emAlignment` u8 across `emFileManTheme` and any other affected sites — belongs to Phase 4e+.
- **RecListenerList:** removed in Task 4; only the legacy `emColorRec`/`emAlignmentRec` referenced it. If a future phase needs a listener-list helper for new rec types, re-introduce from this commit's diff or from C++ `emRec::ValueChanged` chain directly.
