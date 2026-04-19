# Phase 4a — emRec Trait + Primitive Concrete Types — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Port the C++ `emRec` scalar-field infrastructure base layer: `emRecNode`, `emRec`, and the five primitive concrete types (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`). Change-notification via scheduler signals. No persistence, no compound types.

**Architecture:** `emRec<T>` is a Rust trait with `GetValue`, `SetValue(value, ctx)`, `Get[Min|Max|Default]Value`, `GetValueSignal`. Concrete types hold a `value: T`, a `default: T`, and a `value_signal: SignalId` allocated at construction via `ConstructCtx`. `SetValue` mutates then fires the signal via the ctx it receives. This phase lands types and change-notification only; persistence wiring is Phase 4c.

**Companion:** spec §7 D7.1 (Phase 4a scope), §7 D7.3, §7 D7.4. C++ reference: `/home/a0/git/eaglemode-0.96.4/include/emCore/emRec.h` and `src/emCore/emRec.cpp` — read the base-class section plus the five primitive concrete classes only.

**JSON entries closed:** none (E026 closes at Phase 4d gate; E027 closes at Phase 4d).

**Phase-specific invariants (C4):**
- **I4a-1.** `crates/emcore/src/emRec.rs` defines `pub trait emRec`.
- **I4a-2.** `crates/emcore/src/emBoolRec.rs`, `emIntRec.rs`, `emDoubleRec.rs`, `emEnumRec.rs`, `emStringRec.rs` each exist with a concrete struct implementing the trait.
- **I4a-3.** Each concrete type has a test demonstrating `SetValue` fires `GetValueSignal` exactly once per write.
- **I4a-4.** No golden regressions.

**Entry-precondition.** Phase 3 Closeout COMPLETE.

---

## Bootstrap (per shared ritual)

Run B1–B12 with `<N>` = `4a`.

At B3 also read the C++ reference sections listed in Companion.

---

## File Structure

**New files** (one primary type per file, per CLAUDE.md):
- `crates/emcore/src/emRecNode.rs` — base trait for nodes in the emRec tree (parent ref, child list, tree-walk helpers).
- `crates/emcore/src/emRec.rs` — `emRec<T>` trait.
- `crates/emcore/src/emBoolRec.rs`
- `crates/emcore/src/emIntRec.rs`
- `crates/emcore/src/emDoubleRec.rs`
- `crates/emcore/src/emEnumRec.rs`
- `crates/emcore/src/emStringRec.rs`

Each has its corresponding header-correspondence from C++ `emRec.h`; no `_rust_only` marker since these are 1:1 ports. No marker files because the C++ types exist 1:1.

**Modified files:** `crates/emcore/src/lib.rs` to register modules.

---

## Task 1: `emRecNode` base trait

**Files:**
- Create: `crates/emcore/src/emRecNode.rs`.

- [ ] **Step 1: Write failing test.**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rec_node_has_parent_accessor() {
        // A trait-object holder satisfies the trait shape.
        struct Fake;
        impl emRecNode for Fake {
            fn parent(&self) -> Option<&dyn emRecNode> { None }
        }
        let f = Fake;
        assert!(f.parent().is_none());
    }
}
```
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3: Implement.**
```rust
//! emRecNode — base trait for the emRec hierarchy.
//!
//! C++ reference: emCore/emRec.h lines for `emRecNode`.

pub trait emRecNode {
    fn parent(&self) -> Option<&dyn emRecNode>;
    // additional tree-walk methods ported as callers need them in Phase 4b+
}
```
- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit.**
```bash
git add crates/emcore/src/emRecNode.rs crates/emcore/src/lib.rs
git commit -m "phase-4a: emRecNode base trait"
```

---

## Task 2: `emRec<T>` trait

**Files:**
- Create: `crates/emcore/src/emRec.rs`.

- [ ] **Step 1: Write failing test.**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // compile-time only: ensure trait shape.
    fn _assert_trait_shape<T: emRec<i64>>(_: &T) {}
}
```
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3: Implement.**
```rust
//! emRec<T> — abstract scalar-field trait.
//!
//! C++ reference: emCore/emRec.h `emRec<ValueType>`.
//!
//! Observational contract: `SetValue` mutates the stored value and fires
//! `GetValueSignal`. Callers receive `&mut SchedCtx` so the fire happens
//! inline. See spec §7 D7.1.

use crate::emEngineCtx::SchedCtx;
use crate::emRecNode::emRecNode;
use crate::emScheduler::SignalId;

pub trait emRec<T: Clone + PartialEq>: emRecNode {
    fn GetValue(&self) -> &T;
    fn SetValue(&mut self, value: T, ctx: &mut SchedCtx<'_>);
    fn GetDefaultValue(&self) -> &T;
    fn GetValueSignal(&self) -> SignalId;

    /// Default no-bound impl; types with explicit ranges override.
    fn GetMinValue(&self) -> Option<&T> { None }
    fn GetMaxValue(&self) -> Option<&T> { None }
}
```
- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit.**

---

## Task 3: `emBoolRec`

**Files:**
- Create: `crates/emcore/src/emBoolRec.rs`.

- [ ] **Step 1: Write failing test.**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_value_fires_signal() {
        let mut fixture = TestFixture::new();
        let mut rec = emBoolRec::new(&mut fixture.init_ctx(), false);
        let sig = rec.GetValueSignal();
        fixture.clear_signals();
        rec.SetValue(true, &mut fixture.sched_ctx());
        assert!(fixture.is_signaled(sig));
        assert_eq!(*rec.GetValue(), true);
    }

    #[test]
    fn set_to_same_value_does_not_fire() {
        let mut fixture = TestFixture::new();
        let mut rec = emBoolRec::new(&mut fixture.init_ctx(), true);
        let sig = rec.GetValueSignal();
        fixture.clear_signals();
        rec.SetValue(true, &mut fixture.sched_ctx());
        assert!(!fixture.is_signaled(sig));
    }
}
```
(Confirm C++ behavior of no-fire-on-no-change before cementing — `emRec::SetValue` in emRec.cpp skips on `new==old`. Read the C++ source.)

- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3: Implement.**
```rust
use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emScheduler::SignalId;

pub struct emBoolRec {
    value: bool,
    default: bool,
    signal: SignalId,
}

impl emBoolRec {
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: bool) -> Self {
        Self { value: default, default, signal: ctx.create_signal() }
    }
}

impl emRecNode for emBoolRec {
    fn parent(&self) -> Option<&dyn emRecNode> { None }
}

impl emRec<bool> for emBoolRec {
    fn GetValue(&self) -> &bool { &self.value }
    fn SetValue(&mut self, value: bool, ctx: &mut SchedCtx<'_>) {
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
        }
    }
    fn GetDefaultValue(&self) -> &bool { &self.default }
    fn GetValueSignal(&self) -> SignalId { self.signal }
}
```
- [ ] **Step 4: PASS.**
- [ ] **Step 5: Commit.**

---

## Task 4: `emIntRec`

Analogous to Task 3 but with `i64` value + `min`, `max` bounds taken as constructor parameters. C++ reference: `emIntRec` in emRec.h.

- [ ] **Step 1–5** parallel to Task 3. Min/max bounds fields exist; `GetMinValue`/`GetMaxValue` return `Some(&self.min)`/`Some(&self.max)`. `SetValue` clamps to bounds before comparing for no-change-suppression.
- [ ] **Step 6: Commit.**

---

## Task 5: `emDoubleRec`

Analogous. `f64` value + bounds. Reference C++ `emDoubleRec`.

---

## Task 6: `emEnumRec`

C++ `emEnumRec` stores an `int` index into an identifier table. Rust port:
```rust
pub struct emEnumRec {
    value: u32,            // index into identifier table
    default: u32,
    identifiers: Vec<String>,
    signal: SignalId,
}
```
`SetValue` takes `u32`, clamps to `0..identifiers.len()`, fires on change.

- [ ] **Step 1–6** analogous.

---

## Task 7: `emStringRec`

C++ `emStringRec` stores a string; `SetValue` fires on change. Rust port:
```rust
pub struct emStringRec {
    value: String,
    default: String,
    signal: SignalId,
}
```
`SetValue(String, ctx)` — check-by-equality, fire on change.

- [ ] **Step 1–6** analogous.

---

## Task 8: Full gate + invariants

- [ ] **Step 1: Gate.**
```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo-nextest ntr && cargo test --test golden -- --test-threads=1
```

- [ ] **Step 2: Invariants.**
```bash
for f in emRec emRecNode emBoolRec emIntRec emDoubleRec emEnumRec emStringRec; do
    [ -f "crates/emcore/src/$f.rs" ] && echo "$f PASS" || echo "$f FAIL"
done
# Signal-fire tests
cargo test -p emcore emBoolRec emIntRec emDoubleRec emEnumRec emStringRec 2>&1 | grep -c ' PASS' | xargs -I {} test {} -ge 10 && echo "signal-fire PASS" || echo "signal-fire FAIL"
```

- [ ] **Step 3: Proceed to Closeout.**

---

## Closeout (per shared ritual)

Run C1–C11 with `<N>` = `4a`. At C5 note: "Phase 4a ships infrastructure; JSON entries E026/E027 remain open until Phase 4d." Closeout note says `Status: COMPLETE` for gate purposes but flags the partial-entry-close to the next phase's Bootstrap.
