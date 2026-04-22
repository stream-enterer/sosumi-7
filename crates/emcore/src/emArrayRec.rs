//! emArrayRec — dynamic-length record holding a homogeneous array of child
//! records produced by a single allocator closure.
//!
//! C++ reference: `include/emCore/emRec.h:1100-1270` (class emArrayRec) and
//! `src/emCore/emRec.cpp:1702-1940` (impl). Key body:
//!   - `Insert` (emRec.cpp:1736-1759): clips to `MaxCount`, allocates each
//!     new slot via `Allocator()`, calls `BeTheParentOf` on each, and fires
//!     `Changed()` ONCE at the end.
//!   - `Remove` (emRec.cpp:1761-1797): clips to `MinCount`, deletes each
//!     dropped child, and fires `Changed()` ONCE at the end.
//!   - `SetCount` (emRec.cpp:1729-1733): delegates to `Insert` / `Remove`.
//!
//! So the aggregate fires once per resize, not once per added/removed item.
//!
//! Rust rep (per Phase 4c Task 5 approved architecture):
//!   - `items: Vec<Box<dyn emRecNode>>` — owned dynamic children.
//!   - `allocator: emRecAllocator` — shared `crate::emRec::emRecAllocator`
//!     type alias (one allocator for all elements; contrast with emUnionRec's
//!     per-variant allocators).
//!   - `aggregate_signal: SignalId` + `aggregate_signals: Vec<SignalId>` —
//!     reified chain rep from ADR 2026-04-21-phase-4b-listener-tree-adr.md.
//!
//! Persistence methods (SetToDefault, IsSetToDefault, TryStartReading,
//! TryContinueReading, QuitReading, TryStartWriting, TryContinueWriting,
//! QuitWriting, CalcRecMemNeed per emRec.h:1199-1208) are deferred — they
//! belong to the serialization machinery that ships in Phase 4d.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRecAllocator;
use crate::emRecNode::emRecNode;
use crate::emRecReader::{emRecReader, ElementType, RecIoError};
use crate::emRecWriter::emRecWriter;
use crate::emSignal::SignalId;

/// Dynamic homogeneous array of child records.
pub struct emArrayRec {
    /// Reified aggregate signal. Fires once per resize (Insert/Remove/
    /// SetCount) AND on any descendant-leaf mutation (via items' own
    /// aggregate chains).
    aggregate_signal: SignalId,
    /// Outer-compound aggregate chain; non-empty when this array is nested
    /// inside another compound. Parallels emStructRec / emUnionRec rep.
    aggregate_signals: Vec<SignalId>,
    /// Element allocator (C++: `Allocator` field, emRec.h:1213). One shared
    /// allocator — contrast with `emUnionRec`'s per-variant allocators.
    allocator: emRecAllocator,
    /// Minimum element count. C++: `MinCount` (emRec.h:1214).
    min_count: i32,
    /// Maximum element count. C++: `MaxCount` (emRec.h:1214).
    max_count: i32,
    /// Owned child records. C++: `emRec** Array` + `Count` (emRec.h:1215-1217).
    ///
    /// DIVERGED: (language-forced) `Vec<Box<dyn emRecNode>>` replaces `emRec**` + manual
    /// capacity/count bookkeeping. `Count` is `items.len()`; `Capacity` is
    /// elided (Vec manages it). RWPos / RWChildReady belong to the
    /// persistence state machine, deferred to Phase 4d.
    items: Vec<Box<dyn emRecNode>>,
}

impl emArrayRec {
    /// Construct an empty array record with the given allocator and
    /// `[min_count, max_count]` bounds. Equivalent to the C++ constructor
    /// `emArrayRec(emRecAllocator, int minCount, int maxCount)`
    /// (emRec.h:1152, emRec.cpp:1702-1705).
    ///
    /// DIVERGED: (language-forced) C++ constructor immediately calls
    /// `SetToDefault()` to materialise `MinCount` elements. Rust defers that
    /// to `SetToDefault` / `SetCount(min_count)` called explicitly by the
    /// user, matching the staged construction pattern used by emUnionRec
    /// (`new` → `AddVariant` → `SetToDefaultVariant`).
    pub fn new<C: ConstructCtx>(
        ctx: &mut C,
        allocator: emRecAllocator,
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

    /// Current element count. C++: `emArrayRec::GetCount` (emRec.h:1161,
    /// inline at emRec.h:1222-1225).
    pub fn GetCount(&self) -> i32 {
        self.items.len() as i32
    }

    /// Minimum allowed element count. C++: `emArrayRec::GetMinCount`
    /// (emRec.h:1177, inline at emRec.h:1227-1230).
    pub fn GetMinCount(&self) -> i32 {
        self.min_count
    }

    /// Maximum allowed element count. C++: `emArrayRec::GetMaxCount`
    /// (emRec.h:1178, inline at emRec.h:1232-1235).
    pub fn GetMaxCount(&self) -> i32 {
        self.max_count
    }

    /// Resize to `count` elements, clipping to `[min_count, max_count]`.
    ///
    /// C++ reference: `emArrayRec::SetCount` (emRec.cpp:1729-1733) delegates
    /// to `Remove` / `Insert`. Both of those fire `Changed()` once at the
    /// end, so the aggregate fires exactly once per resize — not once per
    /// added/removed item.
    ///
    /// DIVERGED: (language-forced) (near `register_aggregate` loop): C++ `emRec::Changed()`
    /// (emRec.h:243 inline, delegates to `emRec::ChildChanged` at
    /// emRec.cpp:217) walks `UpperNode`. Rust fires the reified aggregate
    /// chain. See ADR 2026-04-21-phase-4b-listener-tree-adr.md.
    //
    // MIRROR: `emTArrayRec::SetCount` duplicates this body against typed `Vec<T>`
    // storage; keep the two in lockstep (no Any supertrait — see emTArrayRec.rs
    // module docs for the tradeoff).
    pub fn SetCount(&mut self, count: i32, ctx: &mut SchedCtx<'_>) {
        // Clip to [min_count, max_count]. C++ `Insert`/`Remove` clip with
        // `MaxCount-Count` / `Count-MinCount` arithmetic; mirroring the
        // outcome directly.
        let target = count.clamp(self.min_count, self.max_count);
        let current = self.items.len() as i32;
        if target == current {
            return;
        }
        if target > current {
            // Grow. C++ `Insert` loop (emRec.cpp:1753-1756) calls
            // `Allocator()` + `BeTheParentOf` for each new slot.
            for _ in current..target {
                let mut child = (self.allocator)(ctx);
                // Splice reified chain onto the new child: this array's
                // own aggregate signal, then every outer-compound signal
                // already registered on this array.
                //
                // DIVERGED: (language-forced) C++ `BeTheParentOf` (emRec.cpp:217 `ChildChanged`)
                // walks `UpperNode`. Rust registers every ancestor aggregate
                // signal directly on the new child. See ADR
                // 2026-04-21-phase-4b-listener-tree-adr.md.
                child.register_aggregate(self.aggregate_signal);
                for sig in &self.aggregate_signals {
                    child.register_aggregate(*sig);
                }
                self.items.push(child);
            }
        } else {
            // Shrink. C++ `Remove` drops trailing elements (after the
            // memmove of survivors). Here there's no index argument so we
            // just truncate.
            self.items.truncate(target as usize);
        }

        // Fire the aggregate once for the whole resize. Matches C++
        // Insert/Remove's single trailing `Changed()` call.
        ctx.fire(self.aggregate_signal);
        for sig in &self.aggregate_signals {
            ctx.fire(*sig);
        }
    }

    /// Immutable access to element `i`, or `None` if out of range.
    ///
    /// DIVERGED: (language-forced) C++ `Get(int)` / `operator[]` (emRec.h:1184-1191, inline at
    /// emRec.h:1237-1252) panic on out-of-range via raw pointer indexing.
    /// Rust returns `Option` to stay safe without `unsafe`; callers that
    /// preserve the C++ precondition (`0 <= i < GetCount()`) see `Some(_)`
    /// deterministically.
    ///
    /// TODO(phase-4d): C++ also exposes a no-arg `Get()` form (emRec.h:1192-
    /// 1194) returning the internal array (`emRec * const *`) for bulk
    /// inspection. Deferred — current consumers access by index.
    pub fn Get(&self, i: i32) -> Option<&dyn emRecNode> {
        if i < 0 {
            return None;
        }
        self.items.get(i as usize).map(|b| &**b as &dyn emRecNode)
    }

    /// DIVERGED: (language-forced) no C++ counterpart. C++ `Get(int)` returns `emRec&` which
    /// callers freely mutate through virtual methods. Rust's borrow checker
    /// needs a distinct `&mut` accessor. Kept minimal for tests and typed
    /// access through user-derived arrays. Callers that need typed mutation
    /// of a concrete child should use `emTArrayRec<T>`; this accessor only
    /// exposes the `emRecNode` surface (parent walks, register_aggregate).
    pub fn GetMut(&mut self, i: i32) -> Option<&mut (dyn emRecNode + 'static)> {
        if i < 0 {
            return None;
        }
        self.items
            .get_mut(i as usize)
            .map(|b| &mut **b as &mut (dyn emRecNode + 'static))
    }

    /// Reified aggregate signal accessor.
    ///
    /// DIVERGED: (language-forced) no direct C++ counterpart — C++ listeners splice into the
    /// `UpperNode` chain instead of observing a named signal. Introduced by
    /// the reified-chain rep (ADR 2026-04-21-phase-4b-listener-tree-adr.md —
    /// R5). Mirrors `emStructRec::GetAggregateSignal` / `emUnionRec::GetAggregateSignal`.
    pub fn GetAggregateSignal(&self) -> SignalId {
        self.aggregate_signal
    }

    // TODO(phase-4d): Insert(index, count), Remove(index, count),
    // SetToDefault, IsSetToDefault, TryStartReading, TryContinueReading,
    // QuitReading, TryStartWriting, TryContinueWriting, QuitWriting,
    // CalcRecMemNeed per emRec.h:1165-1208. SetCount-with-index and the
    // persistence state machine are serialization concerns.
}

impl emRecNode for emArrayRec {
    /// DIVERGED: (language-forced) parent tracked only through the aggregate chain (no
    /// `UpperNode`). Matches emStructRec / emUnionRec / primitive rep.
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    /// Push `sig` onto this array's own aggregate chain AND forward it to
    /// every current item, so descendant-leaf mutations in existing items
    /// reach outer compounds. Future `SetCount` growths replay
    /// `self.aggregate_signals` onto each new item.
    ///
    /// DIVERGED: (language-forced) C++ `emRec::Changed()` (emRec.h:243, delegates to
    /// emRec.cpp:217 `ChildChanged`) walks `UpperNode`. Rust fires the
    /// reified aggregate chain. See ADR
    /// 2026-04-21-phase-4b-listener-tree-adr.md.
    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
        for item in self.items.iter_mut() {
            item.register_aggregate(sig);
        }
    }

    fn listened_signal(&self) -> SignalId {
        self.aggregate_signal
    }

    /// Port of C++ `emArrayRec::TryStartReading` + `TryContinueReading`
    /// (emRec.cpp:1820-1874). Reset to min_count, consume `{`, loop reading
    /// each child body until `}`, enforcing min/max element count.
    ///
    // DIVERGED: (language-forced) fusion into one atomic call. The C++ driver yields between
    // elements; Rust runs the whole body synchronously.
    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        // C++ emRec.cpp:1821 — SetCount(MinCount) before reading.
        self.SetCount(self.min_count, ctx);
        reader.TryReadCertainDelimiter('{')?;

        let mut pos: i32 = 0;
        loop {
            // Check for `}` delimiter. C++ emRec.cpp:1842-1849.
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
            // Grow to accommodate this element if needed.
            if pos >= self.items.len() as i32 {
                self.SetCount(pos + 1, ctx);
            }
            self.items[pos as usize].TryRead(reader, ctx)?;
            pos += 1;
        }
    }

    /// Port of C++ `emArrayRec::TryStartWriting` + `TryContinueWriting`
    /// (emRec.cpp:1886-1918). Emit `{ \n\t<body>\n\t<body>\n }`.
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
    use crate::emBoolRec::emBoolRec;
    use crate::emClipboard::emClipboard;
    use crate::emContext::emContext;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction};
    use crate::emIntRec::emIntRec;
    use crate::emScheduler::EngineScheduler;
    use std::cell::{Cell, RefCell};
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

    /// Allocator producing emIntRec (0, i64::MIN, i64::MAX) children.
    fn int_allocator() -> emRecAllocator {
        Box::new(|c: &mut SchedCtx<'_>| {
            Box::new(emIntRec::new(c, 0, i64::MIN, i64::MAX)) as Box<dyn emRecNode>
        })
    }

    #[test]
    fn set_count_0_to_2_fires_once() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emArrayRec::new(&mut sc, int_allocator(), 0, 100);
        let agg = arr.GetAggregateSignal();

        arr.SetCount(2, &mut sc);
        assert_eq!(arr.GetCount(), 2);
        assert!(sc.is_signaled(agg), "aggregate must fire once on grow");

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    #[test]
    fn set_count_2_to_1_fires_once_and_drops_tail() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emArrayRec::new(&mut sc, int_allocator(), 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(2, &mut sc);
        sc.scheduler.abort(agg);

        arr.SetCount(1, &mut sc);
        assert_eq!(arr.GetCount(), 1);
        assert!(sc.is_signaled(agg), "aggregate must fire on shrink");

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    #[test]
    fn set_count_1_to_0_fires_once() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emArrayRec::new(&mut sc, int_allocator(), 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(1, &mut sc);
        sc.scheduler.abort(agg);

        arr.SetCount(0, &mut sc);
        assert_eq!(arr.GetCount(), 0);
        assert!(
            sc.is_signaled(agg),
            "aggregate must fire on shrink to empty"
        );

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    #[test]
    fn set_count_same_noop_does_not_fire() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emArrayRec::new(&mut sc, int_allocator(), 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(2, &mut sc);
        sc.scheduler.abort(agg);

        arr.SetCount(2, &mut sc);
        assert!(
            !sc.is_signaled(agg),
            "aggregate must NOT fire on SetCount no-op"
        );

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    #[test]
    fn set_count_clips_to_max() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emArrayRec::new(&mut sc, int_allocator(), 0, 3);
        let agg = arr.GetAggregateSignal();

        arr.SetCount(100, &mut sc);
        assert_eq!(arr.GetCount(), 3, "count clipped to max_count");

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    /// `SpyRec` mirrors the emUnionRec test pattern — records every
    /// `register_aggregate(sig)` call into a shared log so the test can
    /// verify exactly which signals `SetCount` splices onto new items.
    struct SpyRec {
        own_signal: SignalId,
        instance_id: u32,
        log: Rc<RefCell<Vec<(u32, SignalId)>>>,
    }

    impl emRecNode for SpyRec {
        fn parent(&self) -> Option<&dyn emRecNode> {
            None
        }
        fn register_aggregate(&mut self, sig: SignalId) {
            self.log.borrow_mut().push((self.instance_id, sig));
        }
        fn listened_signal(&self) -> SignalId {
            self.own_signal
        }
        fn TryRead(
            &mut self,
            reader: &mut dyn emRecReader,
            _ctx: &mut SchedCtx<'_>,
        ) -> Result<(), RecIoError> {
            Err(reader.ThrowSyntaxError())
        }
        fn TryWrite(&self, _writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
            Ok(())
        }
    }

    /// Prove `SetCount` splices the aggregate signal onto each freshly-
    /// allocated item — once per item, no extras.
    #[test]
    fn set_count_splices_aggregate_onto_each_new_item() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let log: Rc<RefCell<Vec<(u32, SignalId)>>> = Rc::new(RefCell::new(Vec::new()));
        let next_id: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        let log_c = Rc::clone(&log);
        let id_c = Rc::clone(&next_id);

        let allocator: emRecAllocator = Box::new(move |c: &mut SchedCtx<'_>| {
            let instance_id = id_c.get();
            id_c.set(instance_id + 1);
            Box::new(SpyRec {
                own_signal: c.create_signal(),
                instance_id,
                log: Rc::clone(&log_c),
            }) as Box<dyn emRecNode>
        });

        let mut arr = emArrayRec::new(&mut sc, allocator, 0, 100);
        let agg = arr.GetAggregateSignal();

        arr.SetCount(2, &mut sc);

        assert_eq!(
            log.borrow().as_slice(),
            &[(0u32, agg), (1u32, agg)],
            "SetCount(2) must splice agg onto instances 0 and 1 exactly once each"
        );

        // Grow further — instance 2 appears, old items not re-registered.
        arr.SetCount(3, &mut sc);
        assert_eq!(
            log.borrow().as_slice(),
            &[(0u32, agg), (1u32, agg), (2u32, agg)],
            "SetCount(3) must splice agg onto only the fresh instance 2"
        );

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }

    /// Register-aggregate on a non-empty array forwards the new signal to
    /// every existing item — so listeners added after items already exist
    /// still observe descendant mutations. Uses the SpyRec log to confirm
    /// each existing item receives the new signal exactly once.
    #[test]
    fn register_aggregate_forwards_to_existing_items() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let log: Rc<RefCell<Vec<(u32, SignalId)>>> = Rc::new(RefCell::new(Vec::new()));
        let next_id: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        let log_c = Rc::clone(&log);
        let id_c = Rc::clone(&next_id);

        let allocator: emRecAllocator = Box::new(move |c: &mut SchedCtx<'_>| {
            let instance_id = id_c.get();
            id_c.set(instance_id + 1);
            Box::new(SpyRec {
                own_signal: c.create_signal(),
                instance_id,
                log: Rc::clone(&log_c),
            }) as Box<dyn emRecNode>
        });

        let mut arr = emArrayRec::new(&mut sc, allocator, 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(2, &mut sc);

        // Baseline: both items have agg registered once each.
        assert_eq!(log.borrow().as_slice(), &[(0u32, agg), (1u32, agg)]);

        // Register a fresh outer signal — it must be forwarded to both
        // existing items.
        let outer = sc.create_signal();
        arr.register_aggregate(outer);

        assert_eq!(
            log.borrow().as_slice(),
            &[(0u32, agg), (1u32, agg), (0u32, outer), (1u32, outer)],
            "register_aggregate must forward new signal to every existing item"
        );

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
        sc.remove_signal(outer);
    }

    /// Outer struct holding an emArrayRec as a member. Any descendant-leaf
    /// mutation must propagate through the array's aggregate AND the
    /// outer struct's aggregate.
    struct ArrayHolder {
        inner: crate::emStructRec::emStructRec,
        flag: emBoolRec,
        items: emArrayRec,
    }

    impl ArrayHolder {
        fn new(ctx: &mut SchedCtx<'_>) -> Self {
            let mut inner = crate::emStructRec::emStructRec::new(ctx);
            let mut flag = emBoolRec::new(ctx, false);
            let mut items = emArrayRec::new(ctx, int_allocator(), 0, 100);
            inner.AddMember(&mut flag, "flag");
            inner.AddMember(&mut items, "items");
            Self { inner, flag, items }
        }
    }

    impl emRecNode for ArrayHolder {
        fn parent(&self) -> Option<&dyn emRecNode> {
            None
        }
        fn register_aggregate(&mut self, sig: SignalId) {
            self.inner.register_aggregate(sig);
            self.flag.register_aggregate(sig);
            self.items.register_aggregate(sig);
        }
        fn listened_signal(&self) -> SignalId {
            self.inner.listened_signal()
        }
        fn TryRead(
            &mut self,
            reader: &mut dyn emRecReader,
            ctx: &mut SchedCtx<'_>,
        ) -> Result<(), RecIoError> {
            let members = self.inner.member_identifiers();
            crate::emStructRec::emStructRec::try_read_body(&members, reader, |idx, r| match idx {
                0 => self.flag.TryRead(r, ctx),
                1 => self.items.TryRead(r, ctx),
                _ => unreachable!(),
            })
        }
        fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
            let members = self.inner.member_identifiers();
            crate::emStructRec::emStructRec::try_write_body(
                &members,
                writer,
                |_| true,
                |idx, w| match idx {
                    0 => self.flag.TryWrite(w),
                    1 => self.items.TryWrite(w),
                    _ => unreachable!(),
                },
            )
        }
    }

    #[test]
    fn nested_array_propagates_through_ancestor() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut holder = ArrayHolder::new(&mut sc);
        let outer_agg = holder.inner.GetAggregateSignal();
        let arr_agg = holder.items.GetAggregateSignal();
        sc.scheduler.abort(outer_agg);
        sc.scheduler.abort(arr_agg);

        // SetCount fires arr_agg AND outer_agg (array is a member of the
        // outer struct; `AddMember` spliced outer's agg onto the array,
        // which in turn forwards onto each new item — so resize fires
        // both chains).
        holder.items.SetCount(2, &mut sc);

        assert!(
            sc.is_signaled(arr_agg),
            "array aggregate must fire on resize"
        );
        assert!(
            sc.is_signaled(outer_agg),
            "outer struct aggregate must fire through nested array on resize"
        );

        sc.scheduler.abort(outer_agg);
        sc.scheduler.abort(arr_agg);
        sc.remove_signal(outer_agg);
        sc.remove_signal(arr_agg);
    }

    /// Shared signal bundle for `MutableSpyRec` — own signal + every
    /// aggregate signal spliced onto the rec. The test keeps an `Rc`
    /// handle so it can drive a leaf mutation without downcasting the
    /// `Box<dyn emRecNode>` returned by the allocator.
    #[derive(Default)]
    struct SpySignals {
        own_signal: Option<SignalId>,
        aggregate_signals: Vec<SignalId>,
    }

    /// SpyRec variant whose signal set is shared with the test harness so
    /// the test can call `trigger` to simulate a leaf mutation (mirrors the
    /// body of a primitive `SetValue`: fire own_signal + every aggregate
    /// signal in order).
    struct MutableSpyRec {
        signals: Rc<RefCell<SpySignals>>,
    }

    impl emRecNode for MutableSpyRec {
        fn parent(&self) -> Option<&dyn emRecNode> {
            None
        }
        fn register_aggregate(&mut self, sig: SignalId) {
            self.signals.borrow_mut().aggregate_signals.push(sig);
        }
        fn listened_signal(&self) -> SignalId {
            self.signals
                .borrow()
                .own_signal
                .expect("own_signal set at construction")
        }
        fn TryRead(
            &mut self,
            reader: &mut dyn emRecReader,
            _ctx: &mut SchedCtx<'_>,
        ) -> Result<(), RecIoError> {
            Err(reader.ThrowSyntaxError())
        }
        fn TryWrite(&self, _writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
            Ok(())
        }
    }

    /// Fire the rec's own signal + its aggregate chain, mirroring the
    /// primitive `SetValue` body.
    fn spy_trigger(signals: &Rc<RefCell<SpySignals>>, sc: &mut SchedCtx<'_>) {
        let s = signals.borrow();
        if let Some(own) = s.own_signal {
            sc.fire(own);
        }
        for sig in &s.aggregate_signals {
            sc.fire(*sig);
        }
    }

    /// Allocator builder shared by the leaf-mutation tests. The returned
    /// allocator pushes each fresh `MutableSpyRec`'s signal bundle into
    /// `handles` in construction order, so the test can trigger item N
    /// via `handles[N]`.
    fn spy_allocator(handles: Rc<RefCell<Vec<Rc<RefCell<SpySignals>>>>>) -> emRecAllocator {
        Box::new(move |c: &mut SchedCtx<'_>| {
            let signals = Rc::new(RefCell::new(SpySignals {
                own_signal: Some(c.create_signal()),
                aggregate_signals: Vec::new(),
            }));
            handles.borrow_mut().push(Rc::clone(&signals));
            Box::new(MutableSpyRec { signals }) as Box<dyn emRecNode>
        })
    }

    fn cleanup_spy_signals(
        handles: &Rc<RefCell<Vec<Rc<RefCell<SpySignals>>>>>,
        sc: &mut SchedCtx<'_>,
    ) {
        for s in handles.borrow().iter() {
            if let Some(own) = s.borrow().own_signal {
                sc.scheduler.abort(own);
                sc.remove_signal(own);
            }
        }
    }

    /// Spec item 2 — direct end-to-end leaf-mutation proof on
    /// `emArrayRec`. Driving a mutation on item 0 must fire the array's
    /// aggregate signal, exactly as a primitive `SetValue` would through
    /// the reified aggregate chain spliced by `SetCount`.
    #[test]
    fn mutate_item_0_fires_aggregate() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let handles: Rc<RefCell<Vec<Rc<RefCell<SpySignals>>>>> = Rc::new(RefCell::new(Vec::new()));

        let mut arr = emArrayRec::new(&mut sc, spy_allocator(Rc::clone(&handles)), 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(2, &mut sc);
        // Drain the SetCount fire so the leaf-mutation fire is observable
        // on its own.
        sc.scheduler.abort(agg);
        assert!(!sc.is_signaled(agg));

        let h = Rc::clone(&handles.borrow()[0]);
        spy_trigger(&h, &mut sc);

        assert!(
            sc.is_signaled(agg),
            "leaf mutation on item 0 must fire the array aggregate"
        );

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
        cleanup_spy_signals(&handles, &mut sc);
    }

    /// Spec item 3 — same as item 2 but driving a mutation on item 1.
    /// Confirms per-item splice coverage, not just item 0.
    #[test]
    fn mutate_item_1_fires_aggregate() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let handles: Rc<RefCell<Vec<Rc<RefCell<SpySignals>>>>> = Rc::new(RefCell::new(Vec::new()));

        let mut arr = emArrayRec::new(&mut sc, spy_allocator(Rc::clone(&handles)), 0, 100);
        let agg = arr.GetAggregateSignal();
        arr.SetCount(2, &mut sc);
        sc.scheduler.abort(agg);
        assert!(!sc.is_signaled(agg));

        let h = Rc::clone(&handles.borrow()[1]);
        spy_trigger(&h, &mut sc);

        assert!(
            sc.is_signaled(agg),
            "leaf mutation on item 1 must fire the array aggregate"
        );

        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
        cleanup_spy_signals(&handles, &mut sc);
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut arr = emArrayRec::new(&mut sc, int_allocator(), 0, 100);
        arr.SetCount(2, &mut sc);

        assert!(arr.Get(-1).is_none());
        assert!(arr.Get(2).is_none());
        assert!(arr.Get(0).is_some());
        assert!(arr.Get(1).is_some());

        let agg = arr.GetAggregateSignal();
        sc.scheduler.abort(agg);
        sc.remove_signal(agg);
    }
}
