# Phase 4b.1 Handoff Prompt

Paste the block below into a fresh session.

---

You are resuming the eaglemode-rs port-ownership rewrite at **Phase 4b.1 — emColorRec / emAlignmentRec migration**.

## State at handoff

- Repo: `/home/a0/git/eaglemode-rs`. Branch: `main` (clean tree expected; verify with `git status --porcelain`).
- Phase 4b merged at `30a75f76` (tag `port-rewrite-phase-4b-complete`). Phase 4b shipped only `emFlagsRec`; the listener-tree machinery + structural compounds were carved into Phase 4c per ADR.
- **Not pushed yet** — `origin/main` is ~9 commits behind local `main`. Decide with user whether to push before starting 4b.1.
- Gate at main HEAD: fmt clean, clippy clean, **2550 nextest pass**, goldens **237 pass / 6 fail** (the six longstanding pre-existing failures unchanged since Phase 4a baseline).

## Phase 4b inheritance (what's live, what's deferred)

**Live on main:**
- All five Phase 4a primitives unchanged (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`).
- `emFlagsRec` (Phase 4b): i32 value, ≤32 dense identifiers, full `check_identifier` predicate (`[A-Za-z_][A-Za-z0-9_]*`).
- `emRecParser.rs` for textual parsing.
- Legacy `emRecRecTypes.rs` still contains the parser-era `emAlignmentRec` and `emColorRec` types that Phase 4b.1 replaces.

**ADR governs the rep choice (do not relitigate):** `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` chose **R5 reified signal chain** (`Vec<SignalId>` of ancestor signals on each rec). Phase 4b.1 is BEFORE Phase 4c's listener-tree retrofit — the new `emColorRec`/`emAlignmentRec` ship without `aggregate_signals` (matching the current Phase 4a/4b primitive shape); the field gets retrofitted in Phase 4c alongside all other primitives.

**Deferred to Phase 4c+:**
- Listener tree (`UpperNode`/`ChildChanged`/`Changed`/`BeTheParentOf`).
- `emRecListener`.
- Parent-aware ctors on any primitive.
- `SetToDefault`, `IsSetToDefault`, `TryStartReading`, serialization hooks (TODO(phase-4b+) markers in every primitive).
- Centralization of `check_identifier` (currently private in `emFlagsRec.rs:78-95`); move to a shared `emRec` helper when emEnumRec/emStructRec varIdentifiers need it (Phase 4c).

**JSON entries E026 + E027 close at Phase 4e**, not Phase 4b.1.

## How to proceed

1. Run the **shared Bootstrap ritual** at `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md` with `<N>` = `4b-1`. Scan the Phase 4b.1 plan for stage-only tasks at B11a — there are none, so skip B11a.
2. Phase 4b.1 plan: `docs/superpowers/plans/2026-04-21-port-rewrite-phase-4b-1-color-alignment-rec.md`.
3. Use the `superpowers:subagent-driven-development` skill for task execution. TDD per task; pre-commit hook runs fmt+clippy+nextest automatically.

## Known operational details to carry forward

- **`emAlignment` Rust type does not exist yet** — Task 1 creates it. C++ `typedef emByte emAlignment` (`emStd1.h:478`) is a u8 bitset of `EM_ALIGN_*` flags. Plan calls for a `crates/emcore/src/emAlignment.rs` newtype around `u8` with the eight EM_ALIGN_* constants and `emAlignmentToString` / `emStringToAlignment` (port the C++ string table from `emStd1.cpp` byte-for-byte).
- **Three production consumers to migrate** (Task 4): `crates/emmain/src/emVirtualCosmos.rs`, `crates/emmain/src/emBookmarks.rs`, `crates/emfileman/src/emFileManTheme.rs`. Each currently imports `emcore::emRecRecTypes::emColorRec` (or `emAlignmentRec`); switch to the new canonical paths.
- **Kani harness handling** (Task 4 Step 2): `crates/eaglemode/tests/kani/proofs_generated.rs` has 6+ generated harnesses against the legacy `emColorRec` constructor + `ToRecStruct`/`FromRecStruct`. Confirm with the user whether to (a) regenerate against the new types, or (b) delete the affected harnesses + record a tracking item to regenerate at Phase 4d. Do not silently delete kani coverage.
- **Persistence stopgap** (Task 4 Step 1): the consumers need `ToRecStruct`/`FromRecStruct`-shaped serialization that the new emRec trait does not yet expose. Acceptable stopgap: retain *free functions* in `emRecRecTypes.rs` operating on `emColor` and `emAlignment` directly (NOT on the legacy rec types) until Phase 4d delivers the real persistence stack. Confirm at Bootstrap that no better option emerged.
- **Test scaffolding pattern**: every primitive duplicates a `make_sched_ctx` helper plus an Rc<RefCell<...>> setup ritual; `emFlagsRec.rs` also added a `SchedCtxParts` named-struct helper inside its tests. Phase 4a closeout flagged hoisting these to a shared `crates/emcore/src/test_support` helper. Don't do that hoist in Phase 4b.1 — it's pure scope creep. The hoist belongs in a dedicated cleanup phase or as a Phase 4c warmup.
- **`SignalId` lives in `crate::emSignal`**, not `crate::emScheduler` — the original Phase 4a plan had this wrong; any subsequent plan code using `crate::emScheduler::SignalId` is similarly bogus and must be fixed at the import site.
- **Scheduler `EngineScheduler` panics on drop with pending signals** — tests that fire signals must `sc.remove_signal(sig)` after asserting the fire.
- **`emDoubleRec` uses explicit `<`/`>` guards (not `f64::clamp`)** to preserve C++ IEEE 754 NaN behavior — do NOT "simplify" if you happen to read it. This rule applies broadly to any future Double-shaped rec.
- **`.claude/` is in `.gitignore`** as of Phase 4a fix-up `6b9e8a4f`; subagents may not notice. Sanity-check `git status` before every commit to catch stray harness leaks.

## References

- `docs/superpowers/notes/2026-04-19-phase-4b-closeout.md` — Phase 4b summary with invariants verified.
- `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` — listener-tree rep ADR. Phase 4b.1 doesn't implement the rep but should not contradict it.
- `docs/superpowers/plans/2026-04-21-port-rewrite-phase-4-overview.md` — canonical execution chain (4a → 4b → 4b.1 → 4c → 4d → 4e). Read this BEFORE Bootstrap.
- `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` — authoritative spec; §7 D7.1 for the emRec layer.
- `CLAUDE.md` — Port Ideology, File and Name Correspondence, `DIVERGED:` / `SPLIT:` rules.

Begin Phase 4b.1.
