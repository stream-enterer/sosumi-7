# Phase 4c — emRec Listener Tree + Structural Compounds — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal.** Ship the listener-tree mechanism (per ADR `2026-04-21-phase-4b-listener-tree-adr.md`) plus the structural compound emRec types (`emStructRec`, `emUnionRec`, `emArrayRec`, `emTArrayRec<T>`) plus `emRecListener`. The listener tree and the compounds are bundled here because the rep chosen by the ADR (reified `Vec<SignalId>` chain) is small enough — a few fields per primitive — that splitting it from its consumers would be artificial phase-fragmentation.

> **Origin.** This phase was carved from the original Phase 4b plan after a C++ audit found the original "owned children + dedicated `aggregate_signal`" sketch contradicted the C++ design (which propagates aggregate change via the parent-pointer listener tree at `emRecNode::UpperNode` + `ChildChanged` virtual). For two days the listener tree was scheduled to ship in Phase 4b as standalone infrastructure. A precedent survey of the codebase (Explore-agent report, 2026-04-21) then surfaced that no existing emcore subsystem uses synchronous parent-walking callbacks — every analogous problem (emPanel, emContext, widgets, legacy RecListenerList) flattens the walk into either an arena handle or a flat signal/callback list. The ADR (`docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md`) reads that precedent and chooses **R5 — reified signal chain**: each rec carries a `Vec<SignalId>` of ancestor aggregate signals, and parent registration walks the subtree once at construction to push the parent's signal onto every leaf. That collapses the listener tree to ~one field per primitive + one method per compound, at which point bundling it with the compounds (Phase 4c) is cleaner than shipping it standalone (former Phase 4b plan).

**Companion:** spec §7 D7.1 (continued). C++ reference:
- `emRec.h:36-290` + `emRec.cpp:120-280` — listener-tree machinery (`UpperNode`, `IsListener`, `ChildChanged`, `Changed`, `BeTheParentOf`, `emRecListener`).
- `emRec.h:930-1031` — `emStructRec`.
- `emRec.h:1038-1100` — `emUnionRec`.
- `emRec.h:1100-1270+` — `emArrayRec` and `emTArrayRec<T>` template.

**Authoritative reference:** ADR `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` defines the rep, the constraints it satisfies (C1–C7), the divergence classification (forced, structural), and the implementation contract. This plan executes against it.

**JSON entries closed:** none. Persistence stack ships at Phase 4d; E026 and E027 close at Phase 4e.

**Phase-specific invariants (C4):**
- **I4c-1.** All six existing primitives (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`, `emFlagsRec`) carry a `aggregate_signals: Vec<SignalId>` field, default `Vec::new()`. Each primitive's `SetValue` (after firing its own signal) iterates the vec and fires every signal in it. Each primitive's existing single-arg ctor (`emBoolRec::new(&mut sc, default)`) is unchanged in signature.
- **I4c-2.** A `register_aggregate(&mut self, sig: SignalId)` method is available on every emRec concrete type — either as a default method on the `emRec<T>` trait (preferred, if it can be expressed without breaking trait shape) or as a member of a small `emRecRegister` super-trait (fallback). Implementer chooses at execution time.
- **I4c-3.** Files exist:
  - `crates/emcore/src/emRecListener.rs` — owns its own engine, holds a `Box<dyn FnMut(&mut SchedCtx<'_>)>` callback, observes a single `SignalId` via the standard scheduler `connect`/`disconnect` API. Supports `SetListenedRec(Option<&R>)` re-targeting.
  - `crates/emcore/src/emStructRec.rs` — named-field registry. `add_field<R>(child, identifier)` recursively pushes `self.aggregate_signal` onto every leaf in the child's subtree.
  - `crates/emcore/src/emUnionRec.rs` — tagged union. Owns its current child via `Box<dyn ...>`. `SetVariant(idx, ctx)` drops old child, allocates new one, calls `child.register_aggregate(self.aggregate_signal)`, fires `self.aggregate_signal`.
  - `crates/emcore/src/emArrayRec.rs` and `crates/emcore/src/emTArrayRec.rs` — dynamic-size container. `SetCount(n)` grows or shrinks; on grow, each new item is constructed and registered.
- **I4c-4.** Composition tests (Rust analogue of C++ `Person` example at `emRec.h:78-108`): build a struct-of-three-primitives, attach `emRecListener` to the struct's aggregate signal, mutate any field → callback invoked exactly once. Same for `emTArrayRec<Person>`.
- **I4c-5.** Multi-level nesting test: root `emStructRec` contains a sub-`emStructRec` containing primitives. Listener attached to the root fires on a deep-leaf mutation.
- **I4c-6.** `emUnionRec::SetVariant` test: listener observes one fire on tag change; listener attached to the (now-replaced) old child stops firing afterward.
- **I4c-7.** `emRecListener::SetListenedRec(None)` test: detached listener does not fire on subsequent mutations of the previously-observed rec. `SetListenedRec(Some(other))` re-targets cleanly.
- **I4c-8.** `try_borrow_total` remains `0`. `rc_refcell_total` does not increase. No new `unsafe` blocks anywhere in the diff.
- **I4c-9.** No golden regressions.
- **I4c-10.** Each compound's `add_field` / `SetVariant` / `SetCount` carries a `// DIVERGED:` comment near the registration call citing C++ `emRec::Changed()` and pointing to the ADR. Each primitive's `SetValue` carries a `// DIVERGED:` comment near the `for sig in &self.aggregate_signals` loop.

**Entry-precondition.** Phase 4b Closeout COMPLETE. Phase 4b.1 may run in parallel or in sequence — Phase 4c does not depend on it directly, but if 4b.1 runs first, the new `emColorRec` and `emAlignmentRec` from 4b.1 must also gain the `aggregate_signals` field as part of Phase 4c's I4c-1.

---

## Bootstrap

Run B1–B12 with `<N>` = `4c`. **B11a:** scan this plan — Tasks each end with their own commit, no stage-only tasks → **skip B11a**.

### Pre-execution audit at Bootstrap

Before Task 1, verify:

1. The ADR `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md` exists and reads `Status: Accepted`.
2. The Phase 4b primitives still have their unchanged single-arg ctors and `parent() -> None` stubs. (If something else landed retroactive parent wiring, the I4c-1 design needs revisiting.)
3. Phase 4b.1 has either landed (then `emColorRec`/`emAlignmentRec` get the `aggregate_signals` retrofit too) or has not (then they're out of scope for I4c-1).

If any of these is false, STOP and write `phase-4c-bootstrap-blocked.md` halt note before proceeding.

---

## File Structure

**New files:**
- `crates/emcore/src/emRecListener.rs`
- `crates/emcore/src/emStructRec.rs`
- `crates/emcore/src/emUnionRec.rs`
- `crates/emcore/src/emArrayRec.rs` (read `emRec.h:1100-1270` to confirm whether this should be its own file or merged into `emTArrayRec.rs`; C++ has it as a separate base class, so 1:1 file correspondence says separate file).
- `crates/emcore/src/emTArrayRec.rs`

**Modified:**
- `crates/emcore/src/emRec.rs` — add `register_aggregate` method (location per implementation choice between trait default vs `emRecRegister` super-trait).
- `crates/emcore/src/emBoolRec.rs`, `emIntRec.rs`, `emDoubleRec.rs`, `emEnumRec.rs`, `emStringRec.rs`, `emFlagsRec.rs` — add `aggregate_signals: Vec<SignalId>` field, wire the per-fire loop into `SetValue`. (Plus `emColorRec.rs`/`emAlignmentRec.rs` if Phase 4b.1 has landed.)
- `crates/emcore/src/lib.rs` — register the new files.

---

## Task 1: Listener-tree retrofit on Phase 4a/4b primitives

The smallest possible step: add `aggregate_signals: Vec<SignalId>` and the per-fire loop to all six existing primitives, plus `register_aggregate`. No new types, no compounds. Verifies that the ADR's R5 rep compiles and that the existing Phase 4a tests still pass.

**TDD:**
1. Failing test: construct a primitive, push a `SignalId` via `register_aggregate(sig)`, mutate, assert the registered signal fires alongside the primitive's own signal.
2. Implement the field + loop on `emBoolRec` first (canonical pattern). Then mechanically apply to the other five.
3. Verify all existing Phase 4a/4b tests still pass (the `Vec::new()` default means no behavioural change for tests that don't register an aggregate).
4. Commit:
   ```
   git commit -m "phase-4c: retrofit aggregate_signals: Vec<SignalId> on six primitives (per ADR)"
   ```

## Task 2: emRecListener

`crates/emcore/src/emRecListener.rs`. C++ reference: `emRec.h:253-290`, `emRec.cpp:227-280`.

Design (per ADR §"Implementation contract" item 3):
- Wraps `Box<dyn FnMut(&mut SchedCtx<'_>)>` (Phase 3 widget-callback shape).
- Allocates its own engine via the standard ConstructCtx path.
- `new(rec, callback, ctx)` connects the engine to the rec's value signal (or aggregate signal if rec is a compound).
- `SetListenedRec(Option<&R>, ctx)` disconnects from the old signal and connects to the new (or just disconnects if `None`).
- Drop disconnects.

TDD: failing test (attach listener to a primitive, mutate, callback fires; detach via `SetListenedRec(None)`, mutate again, callback does NOT fire). Implement. Pass. Commit.

```
phase-4c: port emRecListener (closure-based, scheduler-dispatched)
```

## Task 3: emStructRec

`crates/emcore/src/emStructRec.rs`. C++ reference: `emRec.h:930-1031`. Children are NOT owned — held externally as fields of a derived struct.

Design:
- Fields: `aggregate_signal: SignalId`, `members: Vec<MemberInfo>`.
- `MemberInfo` carries `identifier: String` plus whatever opaque ref Phase 4d needs for serialization (TBD; for Phase 4c, identifier alone may suffice).
- `add_field<R: emRec<T>>(&mut self, child: &mut R, identifier: &str)`:
  1. `child.register_aggregate(self.aggregate_signal)` — if R is itself a compound, R's own `register_aggregate` recursively walks its members and pushes the new signal onto every leaf.
  2. Push `MemberInfo` onto `members`.
- `GetCount`, `Get(i)`, `GetIdentifierOf(i)`, `GetIndexOf(name)` per C++ surface.
- `GetAggregateSignal() -> SignalId` for external listeners.

TDD:
1. Composition test mirroring C++ `Person`: struct with three primitive fields. Attach listener to the struct. Mutate each field; assert listener fires once per mutation.
2. Multi-level test: `Address { street: emStringRec, zip: emIntRec }` nested in `Person { name, addr: Address, age }`. Listener on `Person` fires when `Person.addr.zip` mutates.
3. Implement.
4. Commit:
   ```
   phase-4c: emStructRec — named-field registry on reified signal chain
   ```

## Task 4: emUnionRec

`crates/emcore/src/emUnionRec.rs`. C++ reference: `emRec.h:1038-1100`.

Design:
- Owns its current child via `Box<dyn ???>` (the trait-object form of `emRec<T>` may need a sub-trait — settle at impl time, similar to the `emRecRegister` choice in Task 1).
- Each variant carries an allocator: `Box<dyn FnMut() -> Box<dyn ...>>`.
- `SetVariant(new_idx, ctx)`:
  1. If `new_idx == current_idx`, no-op.
  2. Drop old child.
  3. Allocate new child via the variant's allocator.
  4. Call `new_child.register_aggregate(self.aggregate_signal)`.
  5. Fire `self.aggregate_signal`.

TDD:
1. Two-variant union; set tag from 0 to 1; listener observes one fire and `Get()` returns the new child type.
2. Set tag back to 0; listener observes another fire.
3. Set tag to current value; listener does NOT fire.
4. Listener attached to the old child via its own `emRecListener` stops firing after the variant change (because the old child was dropped).
5. Commit.

## Task 5: emArrayRec + emTArrayRec\<T\>

`crates/emcore/src/emArrayRec.rs` and `crates/emcore/src/emTArrayRec.rs`. C++ reference: `emRec.h:1100-1270` (emArrayRec) and `emRec.h:1271+` (emTArrayRec template).

Design:
- `emArrayRec` owns a `Vec<Box<dyn ...>>` of items.
- `SetCount(n, ctx)` grows or shrinks. On grow, each new item is allocated via the array's allocator, registered with the aggregate signal, and fires the aggregate signal once at the end (single fire for the whole resize, matching C++ `Changed()` being called once after the resize completes — verify this in C++ source).
- `Get(i)` returns the i-th item.
- `emTArrayRec<T>` is a typed wrapper that downcasts items to `T`.

TDD:
1. `SetCount(0 → 2)` → listener fires once.
2. Mutate item 0 → listener fires.
3. Mutate item 1 → listener fires.
4. `SetCount(2 → 1)` → listener fires once; item 1 dropped (no longer reachable).
5. `SetCount(1 → 0)` → listener fires once.
6. Commit.

## Task 6: Phase 4c composition + stress tests

A consolidated integration test demonstrating the full C++ `Person` + `emTArrayRec<Person>` example (`emRec.h:78-108`):

```rust
struct Person {
    inner: emStructRec,
    name: emStringRec,
    age:  emIntRec,
    male: emBoolRec,
}
// ... with construction wiring and add_field calls

let mut persons: emTArrayRec<Person> = ...;
persons.SetCount(2, &mut sc);
persons.Get(0).name.SetValue("Fred".into(), &mut sc);
// listener attached at the array level fires
```

Plus a stress test: 1000 mutations across a multi-level tree, listener fires exactly 1000 times.

Commit: `phase-4c: end-to-end composition + stress tests`.

## Task 7: Full gate + invariants

Same shape as Phase 4b Task 5. `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo-nextest ntr`, `cargo test --test golden -- --test-threads=1`. Verify all I4c invariants. Capture exit metrics.

Commit only if fixups needed; otherwise gate-check the existing tip.

---

## Closeout

Run C1–C11 with `<N>` = `4c`. No JSON entries close yet (E026/E027 land at Phase 4e).
