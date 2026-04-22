# Phase 4b — emRec Listener Tree + emFlagsRec — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal (revised 2026-04-21).** Wire the C++ `emRecNode` listener tree (parent pointers, `IsListener`, `ChildChanged`, `Changed`, `BeTheParentOf`) onto the Phase 4a primitives, plus port `emRecListener`. Ship `emFlagsRec` (atomic compound — does not need the listener tree) as part of the same phase. Compound types that NEED the listener tree (`emStructRec`, `emUnionRec`, `emTArrayRec<T>`) move to **Phase 4c**.

> **Scope amendment (2026-04-21).** This plan was originally titled "emRec Compound Types" and bundled six concrete types (Tasks 1–6). Pre-execution analysis reclassified the work:
> - **Tasks 2 (emAlignmentRec) and 3 (emColorRec)** were deferred to **Phase 4b'** (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-4b-prime-color-alignment-rec.md`) because legacy parser-era counterparts already live in `crates/emcore/src/emRecRecTypes.rs` with three production consumers, requiring a focused migration phase.
> - **Tasks 4–6 (emStructRec, emUnionRec, emTArrayRec\<T\>)** were deferred to **Phase 4c** (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-4c-emrec-compound-types.md`) once it became clear that the original sketch (struct owns children + dedicated `aggregate_signal`) contradicted the C++ design (`emRec.h:36-246`, `emRec.h:930-1006`): C++ propagates aggregate change via a parent-pointer listener tree (`emRecNode::UpperNode` + `ChildChanged` virtual), not via owned-children forwarding. Building the compound types correctly requires the listener-tree mechanism to exist first. Phase 4a's closeout note already anticipated this: *"Landing parent pointers will retroactively change observable behavior at every currently-isolated SchedCtx fire site — capture as a Phase 4b invariant."*
>
> Phase 4b therefore now ships:
> 1. The listener-tree machinery (parent pointers, `IsListener`, `ChildChanged`, `Changed`, `BeTheParentOf` on `emRecNode`/`emRec`).
> 2. `emRecListener` — the public observer class that consumes the listener tree.
> 3. Two-arg parent-aware constructor on each Phase 4a primitive (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`).
> 4. Wire `Changed()` into each primitive's `SetValue`, retiring the `parent() -> None` stub.
> 5. **Task 1 (already shipped at `7223846c`):** `emFlagsRec` (atomic compound — has its own internal value, doesn't need the listener tree but must gain the parent-aware ctor and Changed() wiring as part of (3)/(4)).

**Companion:** spec §7 D7.1 (continued). C++ reference: `emRec.h:36-246` (`emRecNode`, `emRec`, `emRecListener` declarations) and `emRec.cpp:120-280` (their implementations, especially `BeTheParentOf` at :195, `IsListener`/`ChildChanged` at :211/:217, `emRecListener::SetListenedRec` at :241).

**JSON entries closed:** none (E026 still at Phase 4d, E027 still at Phase 4d).

**Phase-specific invariants (C4):**
- **I4b-1.** `crates/emcore/src/emRecListener.rs` exists with the `emRecListener` struct + `OnRecChanged` virtual hook (Rust expression: trait method or `Box<dyn Fn>` closure — see Task 0 brainstorm output).
- **I4b-2.** `emRecNode` trait gains `is_listener() -> bool` (default `false`) and `child_changed(&mut self, ctx: &mut SchedCtx<'_>)` (default impl walks via `parent()`). `parent()` no longer returns `None` for primitives — it returns the wired `Option<&dyn emRecNode>` (or whatever the brainstorm chooses; see Task 0).
- **I4b-3.** Each Phase 4a primitive (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`) gains a parent-aware constructor whose signature mirrors C++ `emRec(emStructRec*, const char*, T defaultValue)` shape (in Phase 4b the parent slot is a generic `&mut dyn emRec` since `emStructRec` itself doesn't exist yet — verify that this slot can be tightened to `emStructRec` in Phase 4c without a breaking change).
- **I4b-4.** Each primitive's `SetValue` calls `Changed()` (the parent-walk) AFTER firing its own signal. Tests verify both: (a) own signal fires, (b) when a parent listener exists, `OnRecChanged` is invoked.
- **I4b-5.** `emFlagsRec` (already shipped) is retrofitted with the parent-aware ctor + `Changed()` wiring in this phase.
- **I4b-6.** A new test file or in-module test demonstrates: child mutation → emRecListener `OnRecChanged` invoked. Verifies the chain works end-to-end without compound types.
- **I4b-7.** `try_borrow_total` MUST remain `0` (Phase 1 deletion holds; do NOT regress to `Rc<RefCell<>>` for the parent chain).
- **I4b-8.** No golden regressions.

**Entry-precondition.** Phase 4a Closeout COMPLETE.

---

## Bootstrap

Run B1–B12 with `<N>` = `4b`. **B11a:** scan this revised plan — Tasks 0a, 1 (already shipped), 2, 3, 4, gate are independently committable, no stage-only tasks → **skip B11a**.

(Bootstrap was completed on 2026-04-21 at commit `2b2bae56`; resume at the next pending task.)

---

## File Structure

**New files (per the brainstorm output):**
- `crates/emcore/src/emRecListener.rs` — `emRecListener` struct, parallel to C++ class.
- `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` — the architecture-decision record produced by Task 0a (one of the two columns committed; the discarded option recorded as "considered alternatives").

**Modified files:**
- `crates/emcore/src/emRecNode.rs` — add `is_listener`, `child_changed`, refine `parent()` semantics per the ADR.
- `crates/emcore/src/emRec.rs` — add `Changed()` and `BeTheParentOf` (Rust expressions tbd by ADR).
- `crates/emcore/src/emBoolRec.rs`, `emIntRec.rs`, `emDoubleRec.rs`, `emEnumRec.rs`, `emStringRec.rs`, `emFlagsRec.rs` — add parent-aware ctors, store the parent reference, retire `parent() -> None`, wire `Changed()` into `SetValue`. Update existing tests where parent-wiring changes observable behavior (Phase 4a flagged this).
- `crates/emcore/src/lib.rs` — register `emRecListener`.

---

## Task 0a: Architecture decision — parent-pointer representation

**Why this is its own task.** The C++ design uses raw `emRecNode * UpperNode` back-pointers, mutated freely as parents register children (`AddMember`, `BeTheParentOf`, `SetListenedRec`). Rust has no zero-cost equivalent — the choice trades off ownership clarity, lifetime annotations, and `unsafe` against C++ shape fidelity. CLAUDE.md says "Rc/RefCell shared state, Weak parent refs" but Phase 1 deleted every `Rc<RefCell<>>` workaround in this subsystem (try_borrow_total: 11 → 0). Re-introducing them would regress Phase 1.

**Output:** an ADR file `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` that picks one of the candidate approaches below (or proposes a fourth), justifies the choice against the design constraints listed, and commits it. The ADR governs Tasks 0b–4.

**Candidate approaches (not exhaustive):**

1. **Raw `Option<NonNull<dyn emRecNode>>` + `unsafe`** — closest to C++. Children store a pointer to their parent; `Changed()` derefs via `unsafe` and calls `child_changed`. Requires `Pin` or pinning-by-convention to prevent moving recs after parent registration. Documents the invariant "parent outlives child" via type comments and a debug-build invariant check.

2. **Owning back-references via lifetime parameter** — `emBoolRec<'p>` carries `parent: Option<&'p dyn emRecNode<'p>>`. Children built with explicit reference to their parent at construction; types parameterized by lifetime up the tree. Forces the entire emRec subsystem into a borrowed style (no `Box<dyn emRec>`, no `Vec<dyn emRec>`).

3. **Pre-allocated signal chain** — each child stores `parent_signals: Vec<SignalId>` set at construction; `SetValue` fires its own signal and every listed parent signal. emRecListener holds its own SignalId and observes via the standard scheduler. No back-pointer at all; the chain is reified as data, not as a pointer walk. Diverges structurally from C++ but matches behaviorally; test as forced-divergence-vs-design-intent at ADR time.

4. **TBD by brainstormer.** If a fourth option emerges from the brainstorm, prefer it if it avoids `unsafe`, doesn't reintroduce `Rc<RefCell<>>`, and preserves the ability to register children dynamically (so Phase 4c's `emUnionRec::SetVariant` can replace its child).

**Constraints the ADR must satisfy:**
- `try_borrow_total` stays 0 after Phase 4b (`rg -c try_borrow crates/`).
- No reintroduction of `Rc<RefCell<EngineScheduler>>` or any equivalent runtime borrow check on the listener tree.
- Each Phase 4a primitive's existing single-arg ctor (`emBoolRec::new(&mut sc, default)`) keeps working for tests that don't need a parent — i.e., the parent slot must be `Option`-shaped or the parent-aware ctor must be a separate function.
- The chosen representation must support `emRecListener::SetListenedRec(rec)` — re-targeting a listener to a new record after construction (`emRec.cpp:241-262`).
- The chosen representation must support `BeTheParentOf` semantics: walk past existing listeners to find the real parent slot, then splice in (`emRec.cpp:195-208`).

**Process:** Spawn a brainstorming subagent (`superpowers:brainstorming`) seeded with this section verbatim plus the C++ source citations above. Returned ADR is committed before any implementation begins. If the brainstorm picks Option 1 (`unsafe` + `NonNull`), Task 0b adds the `Pin`-or-equivalent invariant scaffolding.

**Commit:**
```
git add docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md
git commit -m "phase-4b: ADR — parent-pointer representation for listener tree"
```

---

## Task 0b: Listener-tree skeleton

Implement the chosen representation in `emRecNode.rs` + `emRec.rs` only. No primitive changes yet. Add unit tests at the `emRecNode` level using a hand-rolled `MockListener` to exercise:
- `is_listener` defaults to `false` on emRec, `true` on the mock listener.
- `child_changed` walks past non-listener nodes to listener nodes (mirrors `emRecListener::SetListenedRec` chain-splice at `emRec.cpp:251-260`).
- `Changed()` is a no-op when `parent` is None.
- `BeTheParentOf` correctly splices through existing listeners.

Commit:
```
git add crates/emcore/src/emRecNode.rs crates/emcore/src/emRec.rs
git commit -m "phase-4b: emRecNode listener-tree skeleton (IsListener, ChildChanged, BeTheParentOf)"
```

---

## Task 1: emFlagsRec — ALREADY COMPLETE

Shipped at commits `280a23b3` and `7223846c` (CheckIdentifier fix). Will be retrofitted with parent-aware ctor + Changed() wiring as part of Task 3.

---

## Task 2: emRecListener

**File:** `crates/emcore/src/emRecListener.rs`.

C++ reference: `emRec.h:253-290`, `emRec.cpp:227-280`.

- Struct fields: per ADR. C++ stores `emRec * Rec` and inherits `UpperNode` from emRecNode.
- Methods: `new(rec: Option<...>)`, `GetListenedRec`, `SetListenedRec(rec)`, `OnRecChanged` (the user override point — express as a trait, a `Box<dyn FnMut>`, or a generic — per ADR).
- `is_listener()` returns `true`.
- `child_changed` calls `OnRecChanged` then continues walking up.
- Tests: construct two primitives; attach a listener to one; mutate; assert listener's callback fired exactly once. Detach; mutate; assert no fire.

Commit:
```
git add crates/emcore/src/emRecListener.rs crates/emcore/src/lib.rs
git commit -m "phase-4b: port emRecListener (emRec.h:253, emRec.cpp:227)"
```

---

## Task 3: Wire parent-aware ctors + Changed() into all six primitives

**Files:** `crates/emcore/src/em{Bool,Int,Double,Enum,String,Flags}Rec.rs`.

For each primitive:
1. Add a second constructor mirroring the C++ two-arg ctor (`new_with_parent(parent: ..., var_identifier: &str, default: T, ctx: &mut C) -> Self`). Internally calls `parent.add_member(self_ref, var_identifier)` (registration mechanism per ADR — for Phase 4b without emStructRec, the "parent" slot is satisfied by `dyn emRec` directly via `BeTheParentOf`; the structured `add_member` becomes a Phase 4c addition). For emFlagsRec the ctor adds the identifier list as a third argument before `default`.
2. Store the parent reference per ADR.
3. Implement `parent()` non-trivially (returns the wired parent).
4. Modify `SetValue` to call `Changed()` AFTER firing the own signal. Order matters: own signal fires first to preserve Phase 4a observable order on the leaf; `Changed()` then propagates.
5. Add a unit test for each primitive: construct with a parent (the parent can be a mock listener directly attached via `emRecListener::SetListenedRec` for Phase 4b tests); mutate; assert listener observes.

**Behavior change ack:** Existing Phase 4a tests where a primitive was constructed without a parent still pass — `Changed()` is a no-op on parentless recs. Tests that DO wire a parent must update their expectations. Phase 4a flagged this.

Run the full nextest suite locally; expect no failures.

Commit:
```
git add crates/emcore/src/em{Bool,Int,Double,Enum,String,Flags}Rec.rs
git commit -m "phase-4b: wire parent-aware ctors + Changed() into all six primitives"
```

---

## Task 4: End-to-end listener-tree test

**File:** `crates/emcore/src/emRecListener.rs` (test module) OR a new `crates/emcore/tests/listener_tree.rs` integration test if the cross-primitive setup outgrows the module.

Test scenarios:
- One listener observes one primitive; mutation fires `OnRecChanged`.
- Two stacked listeners on the same primitive (chain-splice): mutation fires both, in chain order matching C++ `emRecListener::ChildChanged` (`OnRecChanged` first, then walk up — so the listener nearer the leaf fires first).
- Listener detached mid-life via `SetListenedRec(None)`: mutation does not fire.
- Listener re-targeted to a different primitive: only the new target's mutations fire.
- Stress: 100 mutations, listener fires 100 times.

Commit:
```
git add ...
git commit -m "phase-4b: end-to-end listener-tree tests (chain, detach, retarget)"
```

---

## Task 5: Full gate + invariants

Same shape as Phase 4a Task 8. Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo-nextest ntr`, `cargo test --test golden -- --test-threads=1`. Verify:
- `try_borrow_total` is still `0`.
- `rc_refcell_total` did not increase (no new `Rc<RefCell<>>` introduced).
- All I4b-1 through I4b-8 invariants pass (verify mechanically).
- Goldens 237/6 unchanged.

Commit:
```
git commit -m "phase-4b: gate green, invariants verified"
```
(if any fixups are needed; otherwise no commit, just gate-check the existing tip).

---

## Closeout

Run C1–C11 with `<N>` = `4b`. No JSON entries close yet (E026/E027 land at Phase 4d).

Update Phase 4b' (Color/AlignmentRec migration) to inherit the listener-tree wiring from this phase — its `emColorRec.rs` and `emAlignmentRec.rs` files will use the parent-aware ctor pattern from Task 3.

Update Phase 4c plan header to confirm: emStructRec, emUnionRec, emTArrayRec build on the now-existing listener tree; their internal `aggregate_signal` is no longer in the plan — aggregate observation works via attached emRecListeners.
