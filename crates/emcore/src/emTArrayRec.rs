//! emTArrayRec<T> — template version of emArrayRec with typed element access.
//!
//! C++ reference: `include/emCore/emRec.h:1271-1530` (`template <class REC>
//! class emTArrayRec : public emArrayRec`). The C++ template inherits from
//! `emArrayRec` and supplies the allocator via `EM_DEFAULT_REC_ALLOCATOR(REC)`;
//! `Get(int)` / `operator[]` simply cast the `emRec&` back to `REC&` —
//! statically sound because every element was produced by the typed
//! allocator.
//!
//! Rust rep (Phase 4c Task 5):
//!   - `items: Vec<T>` — typed storage. No downcast machinery required.
//!   - `allocator: Box<dyn FnMut(&mut SchedCtx<'_>) -> T>` — typed allocator
//!     closure. Parallels the C++ `EM_DEFAULT_REC_ALLOCATOR(REC)` macro that
//!     expands to `emRec* (*)() { return new REC; }`.
//!   - `aggregate_signal: SignalId` + `aggregate_signals: Vec<SignalId>` —
//!     reified chain rep from ADR 2026-04-21-phase-4b-listener-tree-adr.md.
//!
//! DIVERGED from C++: the C++ template *is-a* `emArrayRec` via public
//! inheritance and shares its storage. Rust lacks inheritance, so the
//! typed array has its own typed `Vec<T>` and duplicates the SetCount /
//! register_aggregate logic. The alternative — wrapping a raw
//! `emArrayRec` and downcasting via `Any` — would require adding an
//! `Any` supertrait to `emRecNode` and `as_any_mut` impls to every
//! primitive + compound; the cost of that trait widening outweighs a
//! ~30-line duplication here. Observable behaviour (SetCount semantics,
//! aggregate firing, chain forwarding) is identical to `emArrayRec`.
//!
//! TODO(revisit): if a future caller needs to recover `&mut dyn emArrayRec`
//! from an erased `&mut dyn emRecNode` (e.g., polymorphic compound walks),
//! migrate to an `Any` supertrait on `emRecNode` + `as_any_mut` impls and
//! collapse `emTArrayRec<T>` onto a wrapped `emArrayRec`.
//!
//! Persistence methods (SetToDefault, IsSetToDefault, serialization) are
//! deferred to Phase 4d alongside emArrayRec's equivalents.
//!
//! TODO(phase-4d): C++ exposes an STL-style iterator surface on this
//! template (emRec.h:1289-1326):
//!   - `class ConstIterator` — immutable forward iterator over elements.
//!   - `class Iterator` — mutable forward iterator.
//!   - `begin() / end()` (const + mutable overloads) — range accessors.
//!
//! All deferred. Current Rust consumers use indexed `Get`/`GetMut`;
//! iterator support will arrive when Phase 4d needs it.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRecNode::emRecNode;
use crate::emRecReader::{emRecReader, ElementType, RecIoError};
use crate::emRecWriter::emRecWriter;
use crate::emSignal::SignalId;

/// Typed allocator closure — produces a fresh `T` on each call. Parallels
/// the C++ `EM_DEFAULT_REC_ALLOCATOR(REC)` macro.
pub type emTRecAllocator<T> = Box<dyn FnMut(&mut SchedCtx<'_>) -> T>;

/// Dynamic homogeneous array of typed child records.
pub struct emTArrayRec<T: emRecNode + 'static> {
    aggregate_signal: SignalId,
    aggregate_signals: Vec<SignalId>,
    allocator: emTRecAllocator<T>,
    min_count: i32,
    max_count: i32,
    items: Vec<T>,
}

impl<T: emRecNode + 'static> emTArrayRec<T> {
    /// Construct an empty typed array record.
    ///
    /// C++ reference: `emTArrayRec<REC>::emTArrayRec(int minCount, int
    /// maxCount)` (emRec.h:1331-1335) which forwards to the base
    /// `emArrayRec` ctor with `EM_DEFAULT_REC_ALLOCATOR(REC)`. Rust takes
    /// the typed allocator explicitly — the macro-expanded default-
    /// constructor shape has no portable Rust equivalent when `T`'s
    /// constructor needs a `SchedCtx`.
    ///
    /// DIVERGED: (language-forced) C++ constructor immediately materialises `MinCount`
    /// elements via `SetToDefault`; Rust defers that to an explicit
    /// `SetCount(min_count)` call (same staged pattern as `emArrayRec`).
    pub fn new<C: ConstructCtx>(
        ctx: &mut C,
        allocator: emTRecAllocator<T>,
        min_count: i32,
        max_count: i32,
    ) -> Self {
        Self {
            aggregate_signal: ctx.create_signal(),
            aggregate_signals: Vec::new(),
            allocator,
            min_count,
            max_count,
            items: Vec::new(),
        }
    }

    /// Current element count. C++: inherited `emArrayRec::GetCount`.
    pub fn GetCount(&self) -> i32 {
        self.items.len() as i32
    }

    pub fn GetMinCount(&self) -> i32 {
        self.min_count
    }

    pub fn GetMaxCount(&self) -> i32 {
        self.max_count
    }

    /// Resize to `count` elements, clipping to `[min_count, max_count]`.
    /// Mirrors `emArrayRec::SetCount`; see that module for the C++ refs.
    ///
    /// DIVERGED: (language-forced) C++ `emRec::Changed()` walks
    /// `UpperNode`; Rust fires the reified chain. See ADR
    /// 2026-04-21-phase-4b-listener-tree-adr.md.
    //
    // MIRROR: `emArrayRec::SetCount` holds the erased-Box counterpart of this
    // body; keep the two in lockstep.
    pub fn SetCount(&mut self, count: i32, ctx: &mut SchedCtx<'_>) {
        let target = count.clamp(self.min_count, self.max_count);
        let current = self.items.len() as i32;
        if target == current {
            return;
        }
        if target > current {
            for _ in current..target {
                let mut child = (self.allocator)(ctx);
                child.register_aggregate(self.aggregate_signal);
                for sig in &self.aggregate_signals {
                    child.register_aggregate(*sig);
                }
                self.items.push(child);
            }
        } else {
            self.items.truncate(target as usize);
        }
        ctx.fire(self.aggregate_signal);
        for sig in &self.aggregate_signals {
            ctx.fire(*sig);
        }
    }

    /// Typed immutable access to element `i`, or `None` if out of range.
    ///
    /// C++: `emTArrayRec<REC>::Get(int)` (emRec.h:1343-1346) returns
    /// `REC&` by casting the base `emArrayRec::Get(int)` result. Rust
    /// returns `Option<&T>` directly from the typed storage — no cast
    /// required.
    pub fn Get(&self, i: i32) -> Option<&T> {
        if i < 0 {
            return None;
        }
        self.items.get(i as usize)
    }

    /// Typed mutable access. See `emArrayRec::GetMut`.
    pub fn GetMut(&mut self, i: i32) -> Option<&mut T> {
        if i < 0 {
            return None;
        }
        self.items.get_mut(i as usize)
    }

    /// Reified aggregate signal accessor. Mirrors
    /// `emArrayRec::GetAggregateSignal`.
    pub fn GetAggregateSignal(&self) -> SignalId {
        self.aggregate_signal
    }
}

impl<T: emRecNode + 'static> emRecNode for emTArrayRec<T> {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
        for item in self.items.iter_mut() {
            item.register_aggregate(sig);
        }
    }

    fn listened_signal(&self) -> SignalId {
        self.aggregate_signal
    }

    // MIRROR: emArrayRec::TryRead / TryWrite — same wire format, same body,
    // typed storage (see module docs for the duplication rationale).
    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        self.SetCount(self.min_count, ctx);
        reader.TryReadCertainDelimiter('{')?;

        let mut pos: i32 = 0;
        loop {
            let peek = reader.TryPeekNext()?;
            if let crate::emRecReader::PeekResult::Delimiter(c) = peek {
                if c == '}' {
                    reader.TryReadCertainDelimiter('}')?;
                    if pos < self.min_count {
                        return Err(reader.ThrowElemError("Too few elements."));
                    }
                    return Ok(());
                }
            }
            if peek.element_type() == ElementType::End {
                return Err(reader.ThrowSyntaxError());
            }
            if pos >= self.max_count {
                return Err(reader.ThrowElemError("Too many elements."));
            }
            if pos >= self.items.len() as i32 {
                self.SetCount(pos + 1, ctx);
            }
            self.items[pos as usize].TryRead(reader, ctx)?;
            pos += 1;
        }
    }

    fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        writer.TryWriteDelimiter('{')?;
        writer.IncIndent();
        for item in self.items.iter() {
            writer.TryWriteNewLine()?;
            writer.TryWriteIndent()?;
            item.TryWrite(writer)?;
        }
        writer.DecIndent();
        if !self.items.is_empty() {
            writer.TryWriteNewLine()?;
            writer.TryWriteIndent()?;
        }
        writer.TryWriteDelimiter('}')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emClipboard::emClipboard;
    use crate::emContext::emContext;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction};
    use crate::emIntRec::emIntRec;
    use crate::emRec::emRec;
    use crate::emScheduler::EngineScheduler;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_sched_ctx<'a>(
        sched: &'a mut EngineScheduler,
        actions: &'a mut Vec<DeferredAction>,
        ctx_root: &'a Rc<emContext>,
        cb: &'a RefCell<Option<Box<dyn emClipboard>>>,
        pa: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
    ) -> SchedCtx<'a> {
        SchedCtx {
            scheduler: sched,
            framework_actions: actions,
            root_context: ctx_root,
            framework_clipboard: cb,
            current_engine: None,
            pending_actions: pa,
        }
    }

    fn int_allocator() -> emTRecAllocator<emIntRec> {
        Box::new(|c: &mut SchedCtx<'_>| emIntRec::new(c, 0, i64::MIN, i64::MAX))
    }

    #[test]
    fn typed_get_returns_downcast_ref() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emTArrayRec::<emIntRec>::new(&mut sc, int_allocator(), 0, 100);
        arr.SetCount(2, &mut sc);

        let item0 = arr.Get(0).expect("item 0 present");
        // Typed access — `GetValue` lives on emRec<i64>, callable without
        // a downcast because the storage is typed.
        assert_eq!(*item0.GetValue(), 0);

        let agg = arr.GetAggregateSignal();
        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    #[test]
    fn typed_get_out_of_bounds_returns_none() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emTArrayRec::<emIntRec>::new(&mut sc, int_allocator(), 0, 100);
        arr.SetCount(2, &mut sc);

        assert!(arr.Get(-1).is_none());
        assert!(arr.Get(2).is_none());
        assert!(arr.Get(0).is_some());

        let agg = arr.GetAggregateSignal();
        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    /// Typed mutation through `GetMut` fires the value signal of the
    /// mutated leaf AND the array's aggregate signal — proves the
    /// register_aggregate splice worked end-to-end with real typed items.
    #[test]
    fn typed_mutation_fires_leaf_and_aggregate() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emTArrayRec::<emIntRec>::new(&mut sc, int_allocator(), 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(2, &mut sc);
        sc.scheduler.abort(agg);

        let leaf_sig = arr.Get(1).expect("item 1").GetValueSignal();

        arr.GetMut(1).expect("item 1 mut").SetValue(42, &mut sc);

        assert!(sc.is_signaled(leaf_sig), "leaf value signal must fire");
        assert!(
            sc.is_signaled(agg),
            "array aggregate must fire via reified chain"
        );

        sc.scheduler.abort(agg);
        sc.scheduler.abort(leaf_sig);
        sc.remove_signal(agg);
        sc.remove_signal(leaf_sig);
    }
}
