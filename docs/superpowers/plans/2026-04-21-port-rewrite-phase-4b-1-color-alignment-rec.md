# Phase 4b.1 — emColorRec / emAlignmentRec Migration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Port `emColorRec` and `emAlignmentRec` to the new signal-based emRec trait at canonical paths (`crates/emcore/src/emColorRec.rs`, `crates/emcore/src/emAlignmentRec.rs`), migrate all live consumers, and delete the legacy parser-era types from `crates/emcore/src/emRecRecTypes.rs`.

**Why this is its own phase.** The Phase 4b plan originally bundled these two atomic compounds into Tasks 2–3 alongside `emFlagsRec`. Pre-execution audit (2026-04-21) found:

- Legacy `emAlignmentRec` and `emColorRec` already live in `emRecRecTypes.rs` as parser-era types built on `RecListenerList` (callback list) and serialize to/from `RecValue`.
- Three production consumers depend on the legacy API:
  - `crates/emmain/src/emVirtualCosmos.rs` — uses `emColorRec` for theming + reads `emRecFileReader`.
  - `crates/emmain/src/emBookmarks.rs` — uses `emColorRec` for bookmark colors.
  - `crates/emfileman/src/emFileManTheme.rs` — uses `emAlignmentRec` + `emColorRec` for filemanager theme.
- Generated kani harnesses in `crates/eaglemode/tests/kani/proofs_generated.rs` exercise the legacy `emColorRec` constructor and `ToRecStruct`/`FromRecStruct` serialization paths.
- Legacy `emAlignmentRec` stores a Rust `emTiling::Alignment` enum, NOT C++'s `emByte`-typedef `emAlignment` (a u8 bitset of `EM_ALIGN_*` flags — `emStd1.h:478`). The new port must store the C++ type. So the migration is not a syntactic rename — value semantics change.

The no-backcompat-shims rule (CLAUDE.md feedback) forbids parking a "legacy" namespace alongside a "new" one. The only correct sequence is: build canonical → port consumers → delete legacy, all in a single phase.

**Companion:** spec §7 D7.1 (continued from Phase 4b). C++ reference: `emRec.h:735` (`emAlignmentRec`), `emRec.h:864` (`emColorRec`), `emRec.cpp:920` ff., `emStd1.h:478` (`typedef emByte emAlignment`).

**JSON entries closed:** none (E026 still at Phase 4e).

**Phase-specific invariants (C4):**
- **I4b1-1.** Files `emAlignmentRec.rs` and `emColorRec.rs` exist in `crates/emcore/src/` with concrete `impl emRec<…>`.
- **I4b1-2.** `crates/emcore/src/emAlignment.rs` exists, holding a Rust port of C++ `emAlignment` (newtype around `u8`, with const `EM_ALIGN_*` discriminants and the bitwise OR/AND idioms preserved). `emAlignmentToString`/`emStringToAlignment` (`emStd1.h:483-485`) ported alongside.
- **I4b1-3.** Signal-fire + no-fire-on-no-change tests for both, parallel to Phase 4a primitives.
- **I4b1-4.** All three production consumers compile and pass against the new types; no `emRecRecTypes::emAlignmentRec` or `emRecRecTypes::emColorRec` references remain anywhere in `crates/`.
- **I4b1-5.** Legacy `emAlignmentRec` and `emColorRec` stub-types removed from `emRecRecTypes.rs`. `RecListenerList` may stay if other types still use it; if it becomes unused, remove it too.
- **I4b1-6.** `proofs_generated.rs` regenerated against the new `emColorRec` (or, if regeneration is out-of-band, the old harnesses are deleted and a tracking note added — verify with the user before deleting kani coverage).
- **I4b1-7.** No golden regressions.

**Entry-precondition.** Phase 4b Closeout COMPLETE.

---

## Bootstrap

Run B1–B12 with `<N>` = `4b-1`. **B11a:** scan this plan — Tasks 1–4 each end with their own commit, no stage-only tasks, **skip B11a**.

---

## File Structure

**New files:**
- `crates/emcore/src/emAlignment.rs` — port of C++ `emAlignment` typedef + `EM_ALIGN_*` constants + `emAlignmentToString`/`emStringToAlignment`.
- `crates/emcore/src/emAlignmentRec.rs` — signal-based `emAlignmentRec` parallel to Phase 4a primitives, value type `emAlignment`.
- `crates/emcore/src/emColorRec.rs` — signal-based `emColorRec`, value type `emColor`, with the `HaveAlpha` flag forcing alpha=255 on Set when false (per `emRec.cpp:1051` ff.).

**Modified:**
- `crates/emcore/src/lib.rs` — register the three new modules; remove no-longer-needed legacy registrations only after Task 4.
- `crates/emcore/src/emRecRecTypes.rs` — delete legacy `emAlignmentRec`, `emColorRec`, and any helpers (e.g. `Default for emAlignmentRec`) only used by them.
- `crates/emmain/src/emVirtualCosmos.rs` — switch import + adapt API.
- `crates/emmain/src/emBookmarks.rs` — switch import + adapt API.
- `crates/emfileman/src/emFileManTheme.rs` — switch import + adapt API.
- `crates/eaglemode/tests/kani/proofs_generated.rs` — regenerate or delete affected harnesses (consult user).

---

## Task 1: `emAlignment` Rust type

**Step 1:** Write a small unit test that verifies bitwise OR composition (`EM_ALIGN_TOP | EM_ALIGN_LEFT == EM_ALIGN_TOP_LEFT`) and round-trip `emAlignmentToString` / `emStringToAlignment` for every named value (matching the C++ string table).

**Step 2:** Implement. Newtype around `u8` is the obvious shape; preserve C++ name-correspondence on the constants (`EM_ALIGN_CENTER`, `EM_ALIGN_TOP`, `EM_ALIGN_BOTTOM`, `EM_ALIGN_LEFT`, `EM_ALIGN_RIGHT`, plus the four corner aliases). Read C++ `emRec.cpp` to find the string table used by `emAlignmentToString` and port it byte-for-byte.

**Step 3:** Tests pass.

**Step 4:** Commit:
```
phase-4b-1: port emAlignment u8 typedef + string conversions
```

## Task 2: `emAlignmentRec`

Parallel to `emBoolRec`. Value type `emAlignment`. Failing signal-fire + no-fire-on-no-change tests first; implement; pass; commit.

## Task 3: `emColorRec`

Parallel to `emBoolRec`. Value type `emColor`. Adds the `have_alpha: bool` field stored at construction; `SetValue` forces `value.a = 255` when `have_alpha == false` (port the exact C++ branch at `emRec.cpp` `emColorRec::Set`). Failing test must cover the alpha-clamp path. Commit.

## Task 4: Migrate consumers + delete legacy

**Step 1:** Switch each consumer's `use emcore::emRecRecTypes::{emColorRec, emAlignmentRec}` import to the new canonical paths. Adapt call sites:
- Constructors now take `&mut C: ConstructCtx`.
- Mutation now takes `&mut SchedCtx`.
- Listener registration replaced with signal observation via the standard `WidgetCallback` / engine-level `Observe(signal)` pattern (consult Phase 3 widget code for the canonical adoption shape).
- For consumers that currently re-serialize via `ToRecStruct` / `FromRecStruct`, the persistence path is **NOT** ported in 4b.1 — those call sites must be temporarily routed through a stopgap. Acceptable stopgap: keep the read/write paths against the parser API by retaining the *free functions* in `emRecRecTypes.rs` (operating on `emColor` and `emAlignment` directly, not on the legacy rec types) until persistence lands at Phase 4e.

**Step 2:** Decide kani harness fate with the user. Options:
- (a) Regenerate `proofs_generated.rs` from the new `emColorRec` (preferred if the kani generator is in tree and runnable).
- (b) Delete affected harnesses + record a tracking item to regenerate at Phase 4e.

**Step 3:** Delete legacy `emAlignmentRec` and `emColorRec` from `emRecRecTypes.rs`. Run `rg "emRecRecTypes::em(Alignment|Color)Rec\b"` — must return empty.

**Step 4:** Full gate: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo-nextest ntr`, `cargo test --test golden -- --test-threads=1`. All green.

**Step 5:** Commit:
```
phase-4b-1: migrate consumers; delete legacy Color/AlignmentRec
```

---

## Closeout

Run C1–C11 with `<N>` = `4b-1`. No JSON entries close yet.

---

## Open questions for the executor

- **Persistence stopgap shape.** Task 4 Step 1's "free functions on `emColor`/`emAlignment`" stopgap is a structural choice that bridges Phase 4b.1 and Phase 4e. Confirm at Bootstrap that no better option emerged from Phase 4b.1's experience.
- **Kani regeneration tooling.** Confirm whether `tests/kani/` has a regeneration script in tree before committing to option (a) above.
