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
- **Task 3 fixup** (sha ee3cc1dd) — ported CheckIdentifier (C++ emRec.cpp:173) as free fn in emRec.rs, called from AddMember; deleted dead "Internal hook" doc block; strengthened no-op test to cover multi-level chain propagation (PersonWithAddr.addr.zip); +3 #[should_panic] tests for invalid identifiers (space, leading digit, empty). I1 rejected — `use crate::emRec::emRec` is not dead (brings trait into test scope for GetValueSignal calls). Code-review I2/M3/M5. 2590 tests.
- **Task 4** (sha 5a291925) — emUnionRec ported: `new`, `AddVariant`, `SetDefaultVariant`, `SetToDefaultVariant`, `SetVariant`, `GetVariant`, `GetVariantCount`, `GetIdentifierOf`, `GetVariantOf`, `Get`/`GetMut`, `GetAggregateSignal`. Union owns its current child as `Option<Box<dyn emRecNode>>`; each variant carries a `Box<dyn FnMut(&mut SchedCtx<'_>) -> Box<dyn emRecNode>>` allocator closure (replaces C++ `emRecAllocator` fn pointer). SetVariant drops old child, calls new variant's allocator, then splices `self.aggregate_signal` + every outer `aggregate_signals` entry into the new child's reified chain (ADR R5); fires aggregate on variant change. C++ variadic constructor split into `new` + `AddVariant` + `SetToDefaultVariant` (DIVERGED — variadics not portable). Persistence methods deferred to phase-4d. +7 tests: variant-change fires agg once, return-to-zero fires, no-op doesn't fire, child-mutation propagates via register_aggregate mechanism, old-child signal inert after swap, nested-union propagates through outer emStructRec, identifier/count API smoke. 2597 tests.
- **Task 4 fixup** (sha 9302706d) — strengthened child_mutation test via SpyRec (proves SetVariant splice on owned child), DIVERGED prefixes on Rust-only methods. Spec-review S1/S2.
- **Task 4 fixup 2** (sha 6a0a5789) — SetVariant clamps out-of-range (C++ emRec.cpp:1567-1568 parity), dropped self-admitting listener-on-old-child test (I4c-6 covered by SpyRec splice-targets-new-instance assertion). Code-review C1/I2.
- **Task 5** (sha 4912f4f4) — emArrayRec + emTArrayRec<T> ported. `emArrayRec::new(ctx, allocator, min_count, max_count)` + `SetCount`, `GetCount`, `GetMinCount`, `GetMaxCount`, `Get`, `GetMut`, `GetAggregateSignal`. SetCount clips to `[min_count, max_count]`, grows by calling the single allocator closure per new slot and splicing `aggregate_signal` + every `aggregate_signals` entry onto each new item, shrinks via `Vec::truncate`, and fires the aggregate chain ONCE per resize (matches C++ Insert/Remove's single trailing `Changed()` call at emRec.cpp:1757, 1796 — not once per item). `register_aggregate` pushes onto the own vec AND forwards to every current item so post-populate listener attachment reaches existing descendants. `emRecAllocator` type alias promoted from emUnionRec to emRec.rs for shared use (emArrayRec uses the same type). emTArrayRec<T: emRecNode + 'static> is a typed companion with `Vec<T>` + `Box<dyn FnMut(&mut SchedCtx<'_>) -> T>` — DIVERGED from C++ template inheritance: Rust lacks inheritance, so SetCount / register_aggregate is duplicated (~30 lines) rather than widening `emRecNode` with an `Any` supertrait across 15+ impls just to support the typed `Get` cast. Persistence (Insert/Remove by index, SetToDefault, IsSetToDefault, serialization) deferred to phase-4d. +12 tests (2597 → 2609): SetCount grow/shrink fires once, no-op suppression, max clip, SpyRec-backed per-item splice proof, register_aggregate forwards to existing items, nested-array propagates through outer struct aggregate, Get OOB, typed Get/GetMut, typed mutation fires leaf + aggregate end-to-end.
- **Task 5 fixup** (sha 1ff0a008) — added explicit emArrayRec leaf-mutation tests (spec items 2+3), marked C++ Get()/iterators as TODO(phase-4d). Spec-review G1/G2.
- **Task 5 doc polish** (sha 9eb9ecd7) — MIRROR cross-refs on both SetCount bodies, TODO(revisit) for Any-supertrait migration trigger, GetMut docstring routes typed-mutation callers to emTArrayRec<T>. Code-review important-but-doc-only items.
