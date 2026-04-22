# ADR: emRec listener-tree representation in Rust

**Date:** 2026-04-21
**Status:** Accepted
**Owners of the decision context:** Phase 4b (which generated the question) and Phase 4c (which executes against the decision)
**Supersedes:** the original Phase 4b plan's "owned children + dedicated `aggregate_signal`" sketch (rejected) and the rewritten Phase 4b plan's three-candidate brainstorm (Task 0a)

---

## Context

C++ `emRec` (in `~/git/eaglemode-0.96.4/include/emCore/emRec.h:36-290` and `src/emCore/emRec.cpp:120-280`) propagates aggregate change via a parent-pointer back-walk:

- Every `emRecNode` carries a raw `emRecNode * UpperNode`.
- `emRec::Changed()` (one line at `emRec.h:243-246`) fires `UpperNode->ChildChanged()`.
- `emRec::ChildChanged()` (default impl at `emRec.cpp:217-221`) continues walking up.
- `emRecListener::ChildChanged()` (`emRec.cpp:275-280`) fires the user `OnRecChanged()` hook then continues.
- `emStructRec` does NOT own children — children are declared as members of a derived struct (`class Person : public emStructRec { emBoolRec X; emIntRec Y; }`); each child's parent-aware ctor calls `parent->AddMember(this, identifier)` and sets `UpperNode = parent`.
- Dynamic compounds (`emUnionRec::SetVariant`, `emTArrayRec::SetCount`) own their children through allocators and use `emRec::BeTheParentOf` (`emRec.cpp:195-208`) to splice them into the chain past existing listeners.

Porting this back-walk faithfully into Rust requires either `unsafe { &mut *parent_ptr }`, a viral lifetime parameter on every emRec type, or `Rc<RefCell<>>`/`Weak<RefCell<>>` ownership. Each option carries a cost the codebase has historically paid in some places but not others. The rest of this ADR walks the codebase's actual precedents and lands on the choice that fits.

## Constraints

The decision MUST satisfy:

- **C1.** `try_borrow_total` stays at `0` (Phase 1 invariant; cf. `docs/superpowers/notes/2026-04-19-phase-1-closeout.md`). No new `Rc<RefCell<>>` on the listener-tree path.
- **C2.** Each Phase 4a primitive's existing single-arg ctor (`emBoolRec::new(&mut sc, default)`) keeps working. Tests that don't wire a parent must compile unchanged.
- **C3.** Dynamic re-parenting must work. `emUnionRec::SetVariant(new_idx, ctx)` swaps children; observable behaviour must match C++ exactly.
- **C4.** `emRecListener::SetListenedRec(rec)` (`emRec.cpp:241-262`) — re-targeting an already-constructed listener — must work.
- **C5.** Multi-level nesting (root struct → sub-struct → sub-sub-struct → leaf) must propagate aggregate fires correctly: a leaf mutation fires every ancestor's aggregate signal.
- **C6.** Observable parity with C++. Tests assert `(mutation count) == (listener-callback fire count)` — exactly one fire per leaf mutation, propagated to every registered listener.
- **C7.** No new `Pin`, no new self-referential structs, no new `unsafe` blocks beyond the six pre-existing ones in `PanelCycleEngine`/`PanelScope`. (The codebase's `unsafe` budget is deliberately tiny; growing it for a feature that has alternatives would set a bad precedent.)

## Precedents surveyed

A thorough survey of the existing codebase (Explore-agent report, 2026-04-21) examined how 10 distinct subsystems express analogous patterns. Summary:

| Subsystem | Pattern | Applicable to emRec? |
|---|---|---|
| **emPanel** (`emPanelTree.rs`) | Arena (`SlotMap<PanelId, PanelData>`); parent stored as `Option<PanelId>`; mutation queues parent into a notice ring drained asynchronously by the scheduler | Partial — arena ownership doesn't fit emRec's "child is a member of a user struct" shape |
| **emEngine** (`emScheduler.rs`) | Engines owned by scheduler in a SlotMap; engine instances reach the scheduler via `EngineCtx`. No back-references | No — emRec mutation happens in user code, no scheduler ownership of the rec |
| **emContext** (`emContext.rs`) | `Rc<emContext>` everywhere; parent stored as `Option<Weak<emContext>>`; lookups walk via upgrade | No — would force every emRec into an Rc, defeating the embedded-field design |
| **emModel** | Models registered by `Rc<dyn Any>` in the context. No back-pointer model→context | No — registration without callback is the wrong shape |
| **emCrossPtr** (`emCrossPtr.rs`) | `Weak<RefCell<T>>` + shared `Rc<Cell<bool>>` invalidation flag for explicit "all-pointers-invalid" without walking | Partial — elegant for invalidation, useless for change-callback |
| **emSigModel / emSignal** (`emSignal.rs`) | `SignalId` is a SlotMap handle; `EngineScheduler::fire(sig)` flat-broadcasts to connected engines via `Vec<SignalConnection>` | **Yes** — this is the codebase's canonical observation mechanism, used by every Phase 4a primitive |
| **Legacy `RecListenerList`** (`emRecRecTypes.rs:23-66`) | Flat `Vec<(RecListenerId, Box<dyn Fn()>)>`. Comment by previous porter explicitly rejects the C++ linked-list chain as "unnecessarily complex in Rust" | Negative precedent — informative: the previous porter already chose to flatten the C++ tree-walk |
| **Widget signals** (`emCheckBox.rs`, Phase 3 widgets) | Each widget allocates `*_signal: SignalId` at construction; `WidgetCallback<Args> = Box<dyn FnMut(Args, &mut SchedCtx)>` for inline callbacks | **Yes** — direct precedent for the listener shape |
| **Phase 1 elimination of `Rc<RefCell<>>`** | Context-passing (`&mut SchedCtx` / `&mut EngineCtx` threaded through every method); pointer reification only inside `PanelCycleEngine` for re-entrancy | Strong negative precedent against reintroducing shared interior mutation |
| **Existing `unsafe` in emcore** | 6 blocks total, all in `PanelCycleEngine`/`PanelScope` for cycle re-entrancy. No `Pin`, no `NonNull`, no self-referential structs | Strong negative precedent against introducing self-referential machinery |

**Headline.** The codebase has *zero* precedent for synchronous parent-walking callbacks. Every analogous problem either (a) flattened the walk (emPanel notice queue, legacy RecListenerList) or (b) used flat signal observation (every Phase 3 widget, every Phase 4a primitive). The previous emRec porter (RecListenerList) explicitly chose to flatten; the Phase 1 author explicitly chose to delete shared interior mutation; the codebase's `unsafe` budget has held flat for 18 phases.

## Candidate representations (rejected)

Listed for completeness so a future reader can see what was considered.

### R1. Raw `Option<NonNull<dyn emRecNode>>` + `unsafe`

Closest to C++. Children store a pointer to their parent; `Changed()` derefs via `unsafe` and calls `child_changed`. Requires `Pin` or pinning-by-convention to prevent moving recs after parent registration.

**Why rejected.**
- Zero precedent in emcore for self-referential structs or `Pin`.
- Sets a high `unsafe` precedent for one feature whose Rust alternatives are clean.
- Caller-side ergonomics painful: `Pin<&mut Self>` viral, `Box::pin(...)` in every test.
- Any future bug in the back-pointer (e.g., a parent dropped before a child) is UB; the codebase has not built the safety culture for that.

### R2. Lifetime-parameterized borrow

`emBoolRec<'p>` carries `parent: Option<&'p dyn emRecNode<'p>>`. Construction takes `&'p mut parent`.

**Why rejected.**
- Lifetime parameter goes viral up the type system: `Vec<Box<dyn emRec<'p>>>`, `emTArrayRec<'p, T: emRec<'p>>`, `emConfigModel<'p, ...>`, etc.
- Heterogeneous containers in Phase 4c (`emStructRec`'s member registry, `emTArrayRec`'s items) become awkward — the lifetime parameter has to align across all members.
- Phase 4a's primitive ctors (`fn new<C: ConstructCtx>(...) -> Self`) don't carry a lifetime parameter today; adding one to satisfy compounds breaks every test that doesn't wire a parent.
- No precedent in emcore for lifetime-parameterized record types.

### R3. `Rc<RefCell<>>` parent (the CLAUDE.md "default")

What CLAUDE.md cites as the default convention.

**Why rejected.**
- Phase 1 deleted every `Rc<RefCell<EngineScheduler>>` workaround and brought `try_borrow_total` to 0. Reintroducing the pattern for the listener tree directly regresses Phase 1's invariant (C1).
- emContext already uses `Rc/Weak` for the context tree — but contexts are uniformly Rc-wrapped via `NewRoot`/`NewChild`. emRec primitives are constructed by-value and held as fields of user structs; Rc-wrapping them changes the ownership model fundamentally.

### R4. Scheduler-owned arena (emPanel-style)

Every emRec node lives in a `SlotMap` owned by the scheduler. Parent references are indices.

**Why rejected.**
- emRec users want to write `class Person : emStructRec { emBoolRec X; … }` (the C++ pattern). An arena ownership model breaks that.
- Forces every emRec usage through scheduler API, even tiny standalone tests.
- Loses the "rec is a value" property that makes Phase 4a primitives ergonomic.

## Decision: R5 — reified signal chain

**Choice.** Each emRec carries a `Vec<SignalId>` of "ancestor aggregate signals" alongside its own `SignalId`. On `SetValue`, the rec fires its own signal (Phase 4a behavior, unchanged) and then fires every signal in the ancestor list. There is **no parent pointer**. The "tree walk" is reified at construction time as data: when a parent registers a child, the parent pushes its `aggregate_signal: SignalId` onto the child's `aggregate_signals` vector. Multi-level registration walks the registered subtree once at `add_field` time to push the parent's signal onto every leaf's list.

### Shape

```rust
// in each Phase 4a / 4b primitive
pub struct emBoolRec {
    value: bool,
    default: bool,
    own_signal: SignalId,
    aggregate_signals: Vec<SignalId>,   // walked-up signals to also fire
}

impl emRec<bool> for emBoolRec {
    fn SetValue(&mut self, v: bool, ctx: &mut SchedCtx<'_>) {
        if v != self.value {
            self.value = v;
            ctx.fire(self.own_signal);
            for sig in &self.aggregate_signals {
                ctx.fire(*sig);
            }
        }
    }
    // ...
}

// new method on emRec trait (or a helper trait emRecRegister)
impl emBoolRec {
    pub(crate) fn register_aggregate(&mut self, parent_sig: SignalId) {
        self.aggregate_signals.push(parent_sig);
    }
}
```

```rust
// emStructRec (Phase 4c)
pub struct emStructRec {
    aggregate_signal: SignalId,
    members: Vec<MemberInfo>,    // identifier + opaque ref for serialization (Phase 4d)
}

impl emStructRec {
    pub fn add_field<R: emRecRegister>(&mut self, child: &mut R, identifier: &str) {
        // 1. Recursively walk the child's existing aggregate fan-out and push our
        //    signal into every leaf, so multi-level nesting propagates correctly.
        child.register_aggregate(self.aggregate_signal);
        // 2. Record the member for serialization + GetIndexOf lookups.
        self.members.push(MemberInfo { identifier: identifier.into(), /* … */ });
    }

    pub fn GetAggregateSignal(&self) -> SignalId {
        self.aggregate_signal
    }
}
```

```rust
// emRecListener (Phase 4c)
pub struct emRecListener {
    callback: Box<dyn FnMut(&mut SchedCtx<'_>)>,
    observed_signal: Option<SignalId>,
    engine: EngineId,           // The listener's own engine; on signal fire, scheduler wakes us
}

impl emRecListener {
    pub fn new<R: emRec<T>, T>(rec: &R, callback: impl FnMut(&mut SchedCtx<'_>) + 'static, ctx: &mut C) -> Self {
        // ... allocate engine + connect engine to rec.GetValueSignal()
        //     OR rec.GetAggregateSignal() if rec is a compound — see SetListenedRec semantics.
    }

    pub fn SetListenedRec<R: emRec<T>, T>(&mut self, rec: Option<&R>, ctx: &mut SchedCtx<'_>) {
        // disconnect old, connect new — same engine, just rewires the signal connection
    }
}
```

### Why this satisfies the constraints

- **C1.** Zero new `Rc<RefCell<>>`. `Vec<SignalId>` is plain owned data; `SignalId` is `Copy`.
- **C2.** Existing single-arg ctor unchanged. `aggregate_signals` defaults to `Vec::new()` (no parent → no extra fires). All Phase 4a tests compile and pass unchanged.
- **C3.** `emUnionRec::SetVariant(new_idx, ctx)` constructs the new child with `aggregate_signals = vec![self.aggregate_signal]`, drops the old child, fires `self.aggregate_signal`. No re-parenting needed.
- **C4.** `emRecListener::SetListenedRec(rec)` is just a scheduler-side `disconnect(old_sig, engine); connect(new_sig, engine)`. The listener's own Engine doesn't move.
- **C5.** Multi-level nesting works via recursive registration in `add_field`: when a sub-struct is added to a root struct, the root's `aggregate_signal` is recursively pushed onto every leaf in the sub-struct's already-built tree. One-time tree-walk at construction; runtime cost is the existing `for sig in aggregate_signals` loop.
- **C6.** Observable parity holds. C++ fires N times for N listeners; Rust fires N signals (one per listener) which the scheduler dispatches to each listener's engine. The mutation→callback count is identical.
- **C7.** Zero new `unsafe`, zero new `Pin`, zero self-referential structs. The "tree" exists only as data in `Vec<SignalId>`, never as borrow graphs.

### Cost: structural divergence from C++

This is **not** what C++ does. C++ walks pointers per-fire; Rust walks once at registration and stores the chain.

- **Port Ideology classification:** Forced divergence. Rust's borrow checker, the codebase's `unsafe` budget, and Phase 1's `Rc<RefCell<>>` deletion together make a faithful pointer-back-walk impossible without breaking established patterns (R1–R4 each break one constraint).
- **Required annotations:** Every emRec primitive's `SetValue` carries a `// DIVERGED: emRec::Changed() back-walk reified as Vec<SignalId>; see ADR 2026-04-21-phase-4b-listener-tree-adr.md` comment near the `for sig in &self.aggregate_signals` loop. Every compound's `add_field`/`SetVariant`/`SetCount` carries a similar comment near its registration call.
- **Observable behaviour:** Identical. Verification via composition tests (Phase 4c Task 4) that mirror the C++ `Person` example — listener fires exactly once per leaf mutation.

### What this changes vs. the rewritten Phase 4b plan

The rewritten plan treated "listener tree" as a substantial subsystem. With R5, the rep collapses to:

- One `Vec<SignalId>` field per primitive.
- One `SignalId` field per compound.
- One `register_aggregate` trait method on a small `emRecRegister` trait (or as a default method on `emRec<T>` if it can be added without breaking the trait shape).
- `emRecListener` becomes a thin wrapper around the existing scheduler `connect(signal, engine)` mechanism plus a `Box<dyn FnMut>` callback. Same shape as Phase 3 `WidgetCallback`.

Net: the listener tree is no longer a separable phase. **Phase 4b can ship `emFlagsRec` only and close.** The listener-tree fields + `register_aggregate` + `emRecListener` move into Phase 4c alongside the structural compounds that consume them.

## Implementation contract (Phase 4c)

The Phase 4c plan executes against this ADR. Specifically:

1. Add the `aggregate_signals: Vec<SignalId>` field to all six existing primitives (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`, `emFlagsRec`) and wire the per-fire loop into each `SetValue`. Default `aggregate_signals = Vec::new()`. Phase 4a tests stay green.
2. Add the `register_aggregate(&mut self, sig: SignalId)` method on the `emRec` trait (default impl: `panic!("emRec::register_aggregate must be overridden")` so any concrete that forgets to add the field fails loudly; or, if it can be made a default method that touches `self.aggregate_signals`, the field becomes part of the trait contract — to be decided at implementation time per the per-primitive code shape).
3. Add `emRecListener` as a new file. Wraps `Box<dyn FnMut(&mut SchedCtx<'_>)>`. Allocates its own engine via the standard ConstructCtx path.
4. Implement `emStructRec`, `emUnionRec`, `emArrayRec`/`emTArrayRec` per the existing Phase 4c plan, using `add_field` / `SetVariant` / `SetCount` to push the compound's `aggregate_signal` onto each registered child.
5. Composition tests covering: single-leaf mutation, multi-level nesting, dynamic re-parenting (`emUnionRec::SetVariant`), listener detach/retarget, stress (100 mutations → 100 callbacks).

The Phase 4c plan does NOT need an additional brainstorm; the rep is settled here.

## Open questions explicitly left for Phase 4c

These are implementation choices the ADR does not over-specify:

- **`emRec` trait method vs. `emRecRegister` sub-trait.** `register_aggregate` could go on the existing `emRec<T>` trait or on a new `emRecRegister` super-trait. Either works; the implementer picks based on what makes `add_field` ergonomic.
- **Recursive registration in `add_field`.** Walking a child's pre-existing `aggregate_signals` to push the new parent's signal onto every leaf needs a tree-walk method. Possibly `propagate_aggregate(&mut self, sig: SignalId)` that compounds override to recurse; primitives default-impl to `self.aggregate_signals.push(sig)`. Settle at implementation time.
- **`emRecListener` engine ownership.** Phase 3 widget callbacks construct an internal engine and connect it to the observed signal. emRecListener should follow the same pattern; verify the signature aligns at implementation time.
- **`SetListenedRec` semantics under disconnection.** When the listener detaches (rec set to `None`), the scheduler-side `disconnect` releases the signal connection. If the listener was the last connection, the signal can be removed; verify the existing scheduler API supports this without leaking.

## Future work

- **Persistence (Phase 4d).** Persistence walks the tree via `members: Vec<MemberInfo>` on each compound, not via the aggregate signal chain. The two mechanisms are disjoint. The aggregate-signal rep does not constrain persistence design.
- **`emConfigModel` migration (Phase 4e).** `emConfigModel<T: emStructRec>` will attach an `emRecListener` to the config tree for auto-save. The listener consumes `T::GetAggregateSignal()`. Standard pattern.
- **If the per-fire loop ever becomes a hot path,** the `Vec<SignalId>` could be replaced with a `SmallVec<[SignalId; 4]>` to avoid heap allocation for the typical 1–4 ancestor case. Defer until profiling data exists.
