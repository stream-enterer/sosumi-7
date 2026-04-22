# Phase 4c — emRec Listener Tree + Structural Compounds — Ledger

**Started:** 2026-04-21 23:05 local
**Branch:** port-rewrite/phase-4c
**Baseline:** see 2026-04-19-phase-4c-baseline.md
**Spec sections:** §7 D7.1 (continued) — listener tree + structural compounds
**JSON entries to close:** none (E026/E027 land at Phase 4e; persistence at 4d)
**ADR:** 2026-04-21-phase-4b-listener-tree-adr.md (R5 reified signal chain — Accepted)

## Pre-execution audit (per phase plan)

- ADR Status: Accepted ✅
- Phase 4b primitives unchanged (single-arg ctors, parent() -> None stubs): pending Task 2 verification
- Phase 4b.1 landed (emColorRec / emAlignmentRec present) ✅ — I4c-1 covers 8 primitives, not 6

## B11a decision

**Skipped** — per phase plan "Tasks each end with their own commit, no stage-only tasks."
Pre-commit hook remains active throughout Phase 4c.

## Task log

- **Task 1** (sha fc6566cf) — retrofit aggregate_signals on 8 primitives. +16 tests (fire + no-op per primitive). `register_aggregate` placed on `emRecNode` trait (not `emRec<T>`) — generic-free + dyn-compatible so compounds can forward via `&mut dyn emRecNode`.
- **Task 1 fixup** (sha 63146184) — corrected DIVERGED citation across 8 files from `emRec.cpp:245` (wrong — inside `SetListenedRec`) to `emRec.h:243 inline + emRec.cpp:217 (ChildChanged)`. Dropped vestigial mid-edit comment in emIntRec no-op test. Code-review issue #1 (Important).
- **Task 2** (sha f38df937) — emRecListener ported. Option A: added `emRecNode::listened_signal()` so `SetListenedRec(Option<&dyn emRecNode>)` stays non-generic; primitives return value signal, compounds (Tasks 3-5) will return aggregate. Listener wraps `Box<dyn FnMut(&mut SchedCtx<'_>)>` in a dedicated engine (Priority::Low, Framework scope); callback dispatch is async via scheduler (DIVERGED from C++ synchronous ChildChanged walk). Explicit `detach(self, ctx)` replaces C++ dtor disconnect. +4 tests.
- **Task 2 fixup** (sha 6b445fa2) — added detach_mut, None-attach test, engine-priority rationale. Code-review I1/I2/M4.
- **Task 3** (sha 42d1b4cb) — emStructRec ported: `new`, `AddMember`, `GetCount`, `GetIdentifierOf`, `GetIndexOf`, `GetAggregateSignal`. Struct does not own child records — user's derived struct declares them as sibling fields and forwards `register_aggregate` (mirrors C++ `Person` example at emRec.h:78-108). AddMember splices `self.aggregate_signal` into each child's reified aggregate chain (ADR R5). Persistence methods deferred to phase-4d. +4 tests covering I4c-4 (aggregate fires on any field mutation), I4c-5 (fires through sub-struct compound), identifier registry, and aggregate no-op suppression. 2587 tests pass.
