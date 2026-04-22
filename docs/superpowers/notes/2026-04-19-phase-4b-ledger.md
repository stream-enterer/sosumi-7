# Phase 4b — emRec Compound Types — Ledger

**Started:** 2026-04-21 16:34 PDT
**Branch:** port-rewrite/phase-4b
**Baseline:** see 2026-04-19-phase-4b-baseline.md
**Spec sections:** §7 D7.1
**JSON entries to close:** none

## Bootstrap notes

- B11a skipped: the Phase 4b plan (`docs/superpowers/plans/2026-04-19-port-rewrite-phase-4b-emrec-compound.md`) contains no stage-only tasks — every task ends with its own commit. Pre-commit hook left in place throughout the phase.

## Scope amendment (2026-04-21)

Pre-Task-1 audit found legacy `emAlignmentRec` and `emColorRec` in `crates/emcore/src/emRecRecTypes.rs` with three production consumers (`emVirtualCosmos`, `emBookmarks`, `emFileManTheme`) plus generated kani harnesses. These conflict with adding canonical files at the same logical names. Migration deferred to **Phase 4b'** (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-4b-prime-color-alignment-rec.md`); Phase 4b plan revised in commit `713b5743`. Phase 4b now ships: `emFlagsRec` (Task 1), `emStructRec` (Task 4), `emUnionRec` (Task 5), `emTArrayRec<T>` (Task 6), gate (Task 7).

## Phase 4 family renumber (2026-04-21)

To remove the collision between my newly-carved compound-types plan and the pre-existing persistence/config plans, the Phase 4 family now executes as: **4a → 4b → 4b.1 → 4c → 4d → 4e**. Renames in the same commit as scope amendment #2:
- Phase 4b' (Color/AlignmentRec migration) → Phase 4b.1 (decimal-subphase, matching project convention).
- Pre-existing Phase 4c (emRec persistence IO) → Phase 4d.
- Pre-existing Phase 4d (emCoreConfig migration; closes E026 + E027) → Phase 4e.
- New Phase 4c slot now holds the carved-out structural compounds (emStructRec, emUnionRec, emArrayRec, emTArrayRec).

Canonical execution chain documented at `docs/superpowers/plans/2026-04-21-port-rewrite-phase-4-overview.md`. Future agents must consult that overview before resuming work in the Phase 4 series.

## Scope amendment #2 (2026-04-21, post-Task-1)

Pre-Task-4 C++ audit (`emRec.h:36-246`, `:930-1006`) found the original Task 4 sketch ("struct owns children + dedicated `aggregate_signal`") contradicts the C++ design: aggregate change propagates via the `emRecNode` parent-pointer listener tree (`UpperNode` + `IsListener` + `ChildChanged` virtuals), not via owned-children forwarding. Phase 4a's closeout already anticipated this: *"Landing parent pointers will retroactively change observable behavior at every currently-isolated SchedCtx fire site — capture as a Phase 4b invariant."*

Phase 4b rescoped to ship the listener-tree machinery + emRecListener + retrofit parent-aware ctors onto all six primitives (including the already-shipped emFlagsRec). The structural compounds (emStructRec, emUnionRec, emTArrayRec) move to **Phase 4c** (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-4c-emrec-compound-types.md`), where they will be built C++-faithfully on top of the listener tree.

Plan rewritten in this commit. Bootstrap remains valid (commit `2b2bae56`); Task 1 stays shipped.

## Task log

- **Task 1 (emFlagsRec):** COMPLETE.
  - `280a23b3` — initial impl: i32 value, ≤32 dense identifiers, mask-then-compare SetValue, case-insensitive backwards GetBitOf. Mirrors `emBoolRec` structurally. 8 tests; nextest 2535 → 2543.
  - `7223846c` — code review fix: added full `check_identifier` predicate (`[A-Za-z_][A-Za-z0-9_]*`) per `emRec.cpp:173-194`. 7 grammar tests; nextest 2543 → 2550. CheckIdentifier centralization deferred to phase-4b+ when emEnumRec/emStructRec varIdentifiers need it.
- **Tasks 2-3:** REMOVED per scope amendment (deferred to Phase 4b').

