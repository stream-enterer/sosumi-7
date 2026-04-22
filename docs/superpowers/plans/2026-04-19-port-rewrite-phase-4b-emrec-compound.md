# Phase 4b — emFlagsRec — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal (final, 2026-04-21).** Ship `emFlagsRec` and close Phase 4b. The listener-tree machinery, parent-aware ctors, and `emRecListener` all move into Phase 4c, where they are bundled with the structural compounds that consume them.

> **Scope evolution.** This plan changed shape twice during pre-execution audit. First (`713b5743`) Color/AlignmentRec migration was carved out into Phase 4b.1 because of legacy collisions. Second (`46efd0ad`) the structural compounds (`emStructRec`, `emUnionRec`, `emTArrayRec`) were carved out into Phase 4c after the C++ audit found the original "owned children + dedicated `aggregate_signal`" sketch contradicted the C++ design. At that point Phase 4b was rewritten to ship the listener-tree machinery as standalone infrastructure. Then a precedent survey of the codebase (Explore-agent report, see ADR `2026-04-21-phase-4b-listener-tree-adr.md`) found that the listener-tree machinery, when implemented per the ADR's chosen rep (reified `Vec<SignalId>` chain), collapses to a few fields per primitive and one trait method — small enough that bundling it with its consumers (Phase 4c compounds) is cleaner than shipping it as standalone infrastructure. Phase 4b therefore now ships only `emFlagsRec`, which is already complete, and proceeds straight to Closeout.

**Companion:** spec §7 D7.1 (continued). C++ reference: `emRec.h:643-728`, `emRec.cpp:755-915` (`emFlagsRec`).

**JSON entries closed:** none (E026 closes at Phase 4e, E027 closes at Phase 4e).

**Phase-specific invariants (C4):**
- **I4b-1.** `crates/emcore/src/emFlagsRec.rs` exists and registers in `lib.rs`.
- **I4b-2.** `emFlagsRec` mirrors the Phase 4a primitive shape exactly (struct fields, impl `emRecNode`/`emRec<i32>`, `make_sched_ctx` helper duplicated locally).
- **I4b-3.** Identifier validation is byte-faithful to C++ `emRec::CheckIdentifier` (`emRec.cpp:173-194`): `[A-Za-z_][A-Za-z0-9_]*`, `panic!` on violation. Tests cover positive and negative grammar paths.
- **I4b-4.** `SetValue` masks via `value &= (1<<count)-1` BEFORE the no-change comparison (`emRec.cpp:787-792`). Test `mask_then_compare_no_spurious_fire` pins this.
- **I4b-5.** `try_borrow_total` remains `0`. Phase 4b adds no `Rc<RefCell<>>` anywhere.
- **I4b-6.** No golden regressions.
- **I4b-7.** Listener-tree work + structural compounds + `emRecListener` are deferred to Phase 4c per ADR `2026-04-21-phase-4b-listener-tree-adr.md`. Phase 4b explicitly does NOT ship parent wiring; the `parent() -> None` stub on each Phase 4a primitive remains in place at exit.

**Entry-precondition.** Phase 4a Closeout COMPLETE.

---

## Bootstrap

Already executed on 2026-04-21 at commit `2b2bae56`. Branch: `port-rewrite/phase-4b`. Baseline captured at `docs/superpowers/notes/2026-04-19-phase-4b-baseline.md`.

---

## Task log (executed)

| # | Task | Commits | Status |
|---|---|---|---|
| 1 | `emFlagsRec` initial implementation | `280a23b3` | ✅ done |
| 2 | `emFlagsRec` `CheckIdentifier` predicate fix (code review) | `7223846c` | ✅ done |

Nextest delta: 2535 → 2550 (+15: 8 emFlagsRec primary tests + 7 grammar tests).

The original Tasks 0a, 0b, 2 (`emRecListener`), 3 (parent-aware ctors), 4 (e2e listener-tree tests) and 5 (gate) from the rewritten plan are no longer Phase 4b work. The ADR superseded them; Phase 4c now owns that work.

---

## Closeout

Run C1–C11 with `<N>` = `4b`. The phase merges back to `main` carrying the renumber commit (`c91b9fa0`) plus the ADR commit and the emFlagsRec commits. JSON resolution: none in this phase (E026/E027 still at Phase 4e per the family overview).

**Closeout-specific invariants to assert at C4:**
- I4b-1 through I4b-7 above.
- Specifically grep that no `parent_signals: Vec<SignalId>` field landed in Phase 4b primitives (it lands in Phase 4c per ADR).
- Specifically grep that `crates/emcore/src/emRecListener.rs` does NOT exist in Phase 4b (it lands in Phase 4c).

After Closeout completes, Phase 4b.1 (Color/AlignmentRec migration) becomes the next executable phase, with Phase 4c (listener tree + structural compounds) following.
