# Phase 4a — emRec Trait + Primitive Concrete Types — Closeout

**Branch:** port-rewrite/phase-4a
**Commits:** `2a65b220..08995307` (11 commits on branch including bootstrap + Task 7.5 fixup)
**Status:** COMPLETE — all C1–C11 checks passed (with Closeout-note-documented departures for predecessor-status linkage and per-task review folding — see exit note).

## Summary

Landed the `emRec` scalar-field base layer per spec §7 D7.1, D7.3, D7.4. `emRecNode` trait hosts the parent accessor (single method; tree-walk helpers deferred to Phase 4b+). `emRec<T: Clone + PartialEq>: emRecNode` trait defines the observational contract (GetValue/SetValue/GetDefaultValue/GetValueSignal + default-None Min/Max). Five concrete primitives (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`) each allocate a `SignalId` at construction via `ConstructCtx::create_signal()` and fire it inline from `SetValue` (via `&mut SchedCtx`) only when the stored value actually changes; bounded primitives clamp before comparing. 21 new signal-fire/suppress/clamp tests all pass; goldens unchanged (237/6). No persistence, no parent wiring, no compound types — those are Phases 4b/4c/4d.

The plan's drift-note collision (existing 905-line `emRec.rs` held a textual-record parser) was resolved at Task 2 pre-step: parser moved to new `emRecParser.rs` with `SPLIT:` header citing C++ `emRecReader`/`emRecWriter`; 27 caller files updated.

## Delta from baseline

See `2026-04-19-phase-4a-exit.md` §Delta. Headline: nextest +21, goldens ±0, clippy clean, no new production `Rc<RefCell<>>`.

## JSON entries closed

None. Phase 4a plan explicitly defers E026 (persistence) and E027 (compound types) to Phase 4d. Next-phase Bootstrap must flag these as still-open inheritance items.

## Spec sections implemented

- §7 D7.1 — Phase 4a scope (scalar-field types + change notification).
- §7 D7.3 — observational contract (SetValue fires GetValueSignal on change).
- §7 D7.4 — construct-time signal allocation via `ConstructCtx`.

## Invariants verified

- I4a-1. `emRec.rs` defines `pub trait emRec` — PASS.
- I4a-2. Six primitive files + `emRecNode.rs` exist — PASS.
- I4a-3. Each concrete has `SetValue`-fires-signal-exactly-once-per-change test — PASS.
- I4a-4. No golden regressions — PASS (237/6 identical to baseline).

## Flags forwarded to Phase 4b

Per reviewer notes (see per-task ledger entries):

- **Parent wiring is the real Phase 4b lift.** All 5 concretes currently return `parent() -> None`. When parent pointers land, `ChildChanged()` propagation will change observable behavior at every currently-isolated `SchedCtx` fire site. Capture as a Phase 4b invariant.
- **`_identifiers` rename** — drop the underscore in `emEnumRec.rs:34` the moment `GetIdentifier(index)` lands. Otherwise the accessor will read a misnamed field silently.
- **`emIntRec` width** — Rust is `i64`; C++ is `int` (32-bit). Flagged in the module comment. Verify at Phase 4d that no caller relies on 32-bit wrap behavior.
- **Deferred methods marked `TODO(phase-4b+)`** in each primitive: `Invert` (emBoolRec), `SetToDefault`, `IsSetToDefault`, `TryStartReading`, serialization hooks (all 5 primitives). Grep-discoverable.
- **E026 + E027 still open** — close at Phase 4d gate.

## Next phase

Phase 4b — see plan under `docs/superpowers/plans/2026-04-19-port-rewrite-phase-4b-*.md` (not yet written; Phase 4a's closeout does not gate its creation).
