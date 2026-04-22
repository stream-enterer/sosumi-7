# Phase 4c — emRec Compound Types — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal.** Port the structural compound emRec types: `emStructRec`, `emUnionRec`, `emTArrayRec<T>`. Each composes change-notification via the parent-pointer listener tree shipped in Phase 4b — they do NOT carry their own aggregate signal.

> **Origin (2026-04-21).** This phase was carved out of the original Phase 4b ("emRec Compound Types") after pre-execution audit found the compound types depend on a listener-tree mechanism that did not yet exist. Phase 4b was rewritten to ship that mechanism (`emRecNode::IsListener`/`ChildChanged`/`Changed`/`BeTheParentOf` plus `emRecListener`); Phase 4c builds the structural compounds on top.

**Companion:** spec §7 D7.1 (continued). C++ reference:
- `emRec.h:930-1031` (`emStructRec`).
- `emRec.h:1038-1100` (`emUnionRec`).
- `emRec.h:1271+` (`emTArrayRec<T>`) — note this is a template extending `emArrayRec`, not directly `emRec`.

**JSON entries closed:** none yet (E026 persistence at Phase 4d).

**Phase-specific invariants (C4):**
- **I4c-1.** Files `emStructRec.rs`, `emUnionRec.rs`, `emArrayRec.rs`, `emTArrayRec.rs` exist with concrete impls. (`emArrayRec` is the dynamic-size base; `emTArrayRec<T>` parameterizes over element type per `emRec.h:1271`.)
- **I4c-2.** Each compound is a valid emRecNode (children walk up to it via `child_changed`). None of them carries its own aggregate signal — aggregate observation is via attached `emRecListener`. Tests assert this end-to-end.
- **I4c-3.** Composition test (`Person : emStructRec` style): three primitive children registered, parent-aware ctor used, listener observes mutation of any child via the standard listener-tree mechanism.
- **I4c-4.** `emUnionRec::SetVariant` correctly destroys the old child and constructs the new one via the variant's allocator (`emRec.h:1073-1077`); listener attached to the union sees one fire, listener attached to the (now-deleted) old child stops firing.
- **I4c-5.** `emTArrayRec<T>::SetCount` grows/shrinks correctly; the per-item observation API matches C++ `Get(i) -> emRec&`.
- **I4c-6.** `try_borrow_total` remains `0` (Phase 1 invariant; Phase 4b's listener-tree representation must already satisfy this).
- **I4c-7.** No golden regressions.

**Entry-precondition.** Phase 4b Closeout COMPLETE.

---

## Bootstrap

Run B1–B12 with `<N>` = `4c`. **B11a:** scan this plan — Tasks each end with their own commit, no stage-only tasks → **skip B11a**.

---

## Pre-execution audit at Bootstrap

Before Task 1, verify these assumptions inherited from Phase 4b:

1. The Phase 4b ADR (`docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md`) chose a parent-pointer representation that supports **dynamic re-parenting** (so `emUnionRec::SetVariant` can swap the active child). If it didn't, the union task needs a divergence note.
2. Each Phase 4a primitive now has a parent-aware ctor with a stable signature. The compound types will call it to add fields.
3. `emRecListener` is the canonical observer; compound types do NOT need their own `aggregate_signal: SignalId`.

If any of these is false, STOP and write a `phase-4c-bootstrap-blocked.md` halt note before proceeding.

---

## File Structure

**New files (1:1 with C++):**
- `crates/emcore/src/emStructRec.rs` — named-field collection, registry of child `emRec` back-references.
- `crates/emcore/src/emUnionRec.rs` — tagged union; owns its current child via the variant's allocator.
- `crates/emcore/src/emArrayRec.rs` — dynamic-size base for `emTArrayRec<T>` (C++ has this as a separate base class — verify by reading `emRec.h:1100-1270`).
- `crates/emcore/src/emTArrayRec.rs` — generic typed wrapper over `emArrayRec`.

**Modified:** `crates/emcore/src/lib.rs`.

---

## Task 1: emStructRec

C++ reference: `emRec.h:930-1031`, `emRec.cpp` for `AddMember` and the rest.

The struct does NOT own its children — children are held externally (typically as fields of a derived class). The struct stores `Vec<MemberType { identifier, record_back_ref }>` for iteration/serialization. Children register via the parent-aware ctor's `AddMember` call.

Implementation steps:
1. Failing test: derived-struct pattern (Rust analogue of C++ `class Person : public emStructRec { emBoolRec X; emIntRec Y; }`). Listener attached to the struct must observe each child's mutation.
2. Implement `emStructRec` with `Vec<(String, ???)>` member registry. The `???` shape comes from the Phase 4b ADR — it must be the same kind of back-reference the listener-tree uses.
3. Implement `GetCount`, `Get(i)`, `GetIdentifierOf(i)`, `GetIndexOf(name)`, `GetIndexOf(rec_ref)`.
4. Tests pass.
5. Commit:
   ```
   git commit -m "phase-4c: emStructRec — named-field registry on listener tree"
   ```

## Task 2: emUnionRec

C++ reference: `emRec.h:1038-1100`. The union owns its current child via the variant's allocator function. `SetVariant(new_index)` deletes the old child, allocates a new one, and registers it under the union's parent slot via `BeTheParentOf` (Phase 4b primitive).

Rust expression of the allocator: each variant carries a `fn() -> Box<dyn emRec<???>>` factory (or an enum closure form — TBD by implementer; `BoxFn` is fine if the factory is one-shot). The owned-child storage uses `Box<dyn ...>` — note this is not `Rc<RefCell<>>` and does not regress the Phase 1 invariant.

Steps: failing test (set tag → listener observes one fire and the active child has new type), implement, pass, commit.

## Task 3: emArrayRec + emTArrayRec\<T\>

C++ reference: `emRec.h:1100-1270` (emArrayRec) and `emRec.h:1271+` (emTArrayRec template).

`emArrayRec` is the dynamic-size base; `emTArrayRec<T>` provides typed access. Methods: `SetCount(n)` grows/shrinks; `Get(i)` returns the i-th child; `BeTheParentOf` is called when items are constructed.

Steps: failing test (`SetCount(2)` → two listener fires? confirm against C++ behavior — likely one fire after the resize, since the resize is the user-visible change), implement, pass, commit.

## Task 4: Composition tests

A consolidated integration test demonstrating the C++ `Person` example (`emRec.h:78-108`):
```rust
struct Person {
    inner: emStructRec,
    name: emStringRec,   // constructed with parent=&mut inner
    age: emIntRec,
    male: emBoolRec,
}
```
Listener attached to a `Person`; mutate any field; assert single listener fire.

Plus an `emTArrayRec<Person>` test mirroring the C++ array example (`emRec.h:99-108`).

Commit: `phase-4c: end-to-end composition tests (Person, TArray<Person>)`.

## Task 5: Full gate + invariants

Same shape as Phase 4b Task 5. Verify all I4c invariants.

---

## Closeout

Run C1–C11 with `<N>` = `4c`. No JSON entries close yet.
