# Phase 4a — emRec Trait + Primitive Concrete Types — Ledger

**Started:** 2026-04-21
**Branch:** port-rewrite/phase-4a
**Baseline:** see `2026-04-19-phase-4a-baseline.md`
**Spec sections:** §7 D7.1, §7 D7.3, §7 D7.4
**JSON entries to close:** none (E026 / E027 remain open until Phase 4d)

## Drift-note decision (plan line 21)

**Chosen:** option (a) — move existing `crates/emcore/src/emRec.rs` contents (905 lines: `RecStruct`, `RecValue`, `parse_rec`, `write_rec`) to a new file `crates/emcore/src/emRecParser.rs` with a `SPLIT:` comment citing `emCore/emRec.h` (C++ `emRecReader` / `emRecWriter`). `emRec.rs` is then free for the `pub trait emRec<T>` per I4a-1.

**Rationale:**
- CLAUDE.md File and Name Correspondence: "primary file keeps the C++ name". `class emRec` is the header's primary class; the trait belongs in `emRec.rs`.
- Existing `RecStruct`/`RecValue`/parser content does not correspond to `class emRec`; closer to `emRecReader` / `emRecWriter` (emRec.h lines 32–33). Splitting it out preserves correspondence.
- Applied as Task 2 pre-step, before the trait is introduced.

## B11a pre-commit hook

Phase 4a plan has no stage-only tasks — every task has its own Step 5 commit boundary. Hook left in place.

## Task log

- **Task 1 — emRecNode base trait** — COMPLETE. Commits `24839ea3` (initial), `3027e4af` (fixup: expanded doc, DIVERGED annotation on `parent()` citing `UpperNode` field + no-public-`GetParent`-on-emRecNode in C++). Spec review ✅; code-quality review flagged 3 Important items; 2 addressed in fixup, 1 (non_camel_case_types) already handled by crate-level attribute in `lib.rs`.
- **Task 2 pre-step — split emRec.rs → emRecParser.rs** — COMPLETE. Commit `5c6e6ff6`. Parser/serializer body (905 lines: RecStruct/RecValue/RecError/parse_rec*/write_rec*) relocated with SPLIT header citing emRec.h:32-33 (emRecReader/emRecWriter). 27 caller files updated; `git grep 'crate::emRec::\|emcore::emRec::'` = 0 matches post-move.
- **Task 2 — emRec<T> trait** — COMPLETE. Commit `da01f999`. Implementer caught and fixed plan's buggy `use crate::emScheduler::SignalId` (correct path: `crate::emSignal::SignalId`). Trait: `pub trait emRec<T: Clone + PartialEq>: emRecNode` with 4 required + 2 default methods.
- **Task 3 — emBoolRec** — COMPLETE. Commit `58c98f86`. C++ verification: `emRec.cpp:306-312` (`emBoolRec::Set` own no-change skip at :308). Test-scaffold adapted from the `is_signaled_tracks_fire_and_remove` pattern; `remove_signal` cleanup required to avoid EngineScheduler drop-panic.
- **Task 4 — emIntRec** — COMPLETE. Commit `d5e15228`. C++ verification: constructor clamp emRec.cpp:398-400; Set clamp+compare emRec.cpp:426-431. 5 tests (change, no-change, clamp-suppress, clamp-fire, min/max accessors).
- **Task 5 — emDoubleRec** — COMPLETE. Commit `28920e4a`. C++ verification: constructor emRec.cpp:498-500; Set emRec.cpp:531-536. NaN handling: explicit `<`/`>` guards (not `f64::clamp`) to preserve C++ IEEE 754 behavior — NaN passes guards and fires every time.
- **Task 6 — emEnumRec** — COMPLETE. Commit `60cd80bb`. C++ verification: Init emRec.cpp:727-748; Set emRec.cpp:631-639. Stored `max_index: u32` so `GetMaxValue` can return `&u32`. `_identifiers` field preserves underscore to suppress dead-code lint until Phase 4b+ `GetIdentifier` accessor lands.
- **Task 7 — emStringRec** — COMPLETE. Commit `b4b14b4a`. C++ verification: constructor emRec.cpp:1050-1052; Set emRec.cpp:1074. 3 tests (change, no-change, default-preserved-across-SetValue).
- **Task 7.5 (reviewer fixup) — deferred-methods TODOs** — COMPLETE. Commit `08995307`. Added `TODO(phase-4b+): Invert()` on emBoolRec and `TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks` on all 5 primitives (grep-discoverable from the next phase).
- **Task 8 — full gate + invariants + closeout** — COMPLETE. Gate clean (fmt/clippy/nextest 2535/goldens 237/6); I4a-1..I4a-4 all PASS. See `2026-04-19-phase-4a-exit.md` and `2026-04-19-phase-4a-closeout.md`.
