# Phase 4b — emRec Compound Types — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Port the compound emRec concrete types: `emFlagsRec`, `emAlignmentRec`, `emColorRec`, `emStructRec`, `emUnionRec`, `emTArrayRec<T>`. Each composes change-notification from child recs (structural) or maintains its own signal (atomic).

**Architecture:** Compound types are nodes that own child `emRec`s (structural: `emStructRec`, `emUnionRec`, `emTArrayRec<T>`) or are themselves atomic values with structured representation (`emFlagsRec`, `emAlignmentRec`, `emColorRec`). Atomic compounds behave like primitives with typed value. Structural compounds propagate child-signals upward per C++ `emRec::SetValue` composite behavior.

**Companion:** spec §7 D7.1. C++ reference: `emRec.h` for each of the compound types.

**JSON entries closed:** none (E026 at Phase 4d).

**Phase-specific invariants (C4):**
- **I4b-1.** Files `emFlagsRec.rs`, `emAlignmentRec.rs`, `emColorRec.rs`, `emStructRec.rs`, `emUnionRec.rs`, `emTArrayRec.rs` exist with concrete impls.
- **I4b-2.** For each, a signal-fire test parallel to Phase 4a's pattern.
- **I4b-3.** For `emStructRec`, a composition test: setting a child value fires the child signal *and* propagates an aggregate-change signal (matching C++ behaviour — confirm against emRec.cpp).
- **I4b-4.** No golden regressions.

**Entry-precondition.** Phase 4a Closeout COMPLETE.

---

## Bootstrap

Run B1–B12 with `<N>` = `4b`.

---

## File Structure

**New files** (1:1 with C++):
- `crates/emcore/src/emFlagsRec.rs` — bitset: u32 value with identifier table.
- `crates/emcore/src/emAlignmentRec.rs` — emAlignment value (a u32 packed alignment).
- `crates/emcore/src/emColorRec.rs` — emColor value.
- `crates/emcore/src/emStructRec.rs` — named field collection of child emRecs.
- `crates/emcore/src/emUnionRec.rs` — tagged union of alternative child emRecs.
- `crates/emcore/src/emTArrayRec.rs` — `emTArrayRec<T: emRec>` dynamic list of child recs.

**Modified:** `crates/emcore/src/lib.rs`.

---

## Task 1–3: Atomic compounds (`emFlagsRec`, `emAlignmentRec`, `emColorRec`)

Each follows the Phase-4a primitive pattern.

- [ ] **For each:**
    - **Step 1:** Failing signal-fire + no-fire-on-no-change tests.
    - **Step 2:** Implement parallel to `emBoolRec`, substituting the value type:
      - `emFlagsRec`: `value: u32`, `identifiers: Vec<String>`, `GetFlagId/SetFlag` helpers.
      - `emAlignmentRec`: `value: emAlignment` (ported type from `emAlignment.rs`).
      - `emColorRec`: `value: emColor` (existing `Color` type in `emColor.rs`).
    - **Step 3:** Tests pass.
    - **Step 4:** Commit.

```bash
git add crates/emcore/src/emFlagsRec.rs crates/emcore/src/emAlignmentRec.rs crates/emcore/src/emColorRec.rs crates/emcore/src/lib.rs
git commit -m "phase-4b: atomic compound recs (Flags/Alignment/Color)"
```

---

## Task 4: `emStructRec`

**Files:**
- Create: `crates/emcore/src/emStructRec.rs`.

**Design (C++ reference):** `emStructRec` owns named children of type `Box<dyn emRec<?>>`. When any child's `SetValue` fires, the struct's aggregate `GetValueSignal` must also fire. C++ does this by chaining signals.

- [ ] **Step 1: Failing test.**
```rust
#[test]
fn struct_fires_aggregate_on_child_change() {
    let mut fixture = TestFixture::new();
    let mut s = emStructRec::new(&mut fixture.init_ctx());
    let bool_field = s.add_field::<emBoolRec>(&mut fixture.init_ctx(), "flag", false);
    let agg_sig = s.GetValueSignal();
    fixture.clear_signals();
    s.SetChildBool(bool_field, true, &mut fixture.sched_ctx());
    assert!(fixture.is_signaled(agg_sig));
}
```

- [ ] **Step 2: FAIL.**
- [ ] **Step 3: Implement.** The struct keeps a `Vec<Box<dyn emRec<?>>>` of children *plus* a dedicated `aggregate_signal: SignalId`. Child `SetValue` calls go through the struct, which forwards to the child then fires `aggregate_signal` if the child fired.

Because child rec types are heterogeneous, expose a typed-index API that returns a typed `&mut dyn emRec<T>` through downcast; or — simpler — the struct exposes typed getters/setters (`SetChildBool(idx, val, ctx)`, `GetChildDouble(idx)`, etc.) that statically match the declared field type.

Implementation choice per Q2 of spec: monomorphize — use generic `SetChild<T: emRec<V>, V>(idx, val, ctx)`. The struct stores children in a heterogeneous container keyed by `TypeId`; downcast at access.

- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit.**

---

## Task 5: `emUnionRec`

Tagged union: the active child variant changes on `SetTag(new_tag, ctx)`, which fires the aggregate signal. C++ reference: `emUnionRec`.

- [ ] **Step 1–5:** Failing test (set tag fires), implement, pass, commit.

---

## Task 6: `emTArrayRec<T>`

C++ template array of recs. Rust port: `emTArrayRec<T: emRec<V>, V>` with `Vec<T>` backing. Methods: `SetCount(n, ctx)` grows/shrinks; `GetItem(i)` / `SetItem(i, value, ctx)` proxies to child. Aggregate signal fires on any child write or on length change.

- [ ] **Step 1–5.**

---

## Task 7: Full gate + invariants

Same shape as Phase 4a Task 8.

---

## Closeout

Run C1–C11 with `<N>` = `4b`. No JSON entries close yet.
