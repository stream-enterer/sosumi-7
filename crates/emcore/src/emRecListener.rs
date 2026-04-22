//! emRecListener — closure-based, scheduler-dispatched listener on a rec.
//!
//! C++ reference: `include/emCore/emRec.h:253-290` (class) and
//! `src/emCore/emRec.cpp:227-280` (impl).
//!
//! DIVERGED: C++ `emRecListener` is an abstract base with a pure-virtual
//! `OnRecChanged()` and splices itself into the `UpperNode` chain synchronously
//! (`ChildChanged` walks up and calls `OnRecChanged` inline). Rust replaces
//! inheritance + synchronous walk with:
//!   1. A closure `Box<dyn FnMut(&mut SchedCtx<'_>)>` (the Phase 3 widget-
//!      callback shape, payload-free).
//!   2. A dedicated engine dispatched by `EngineScheduler` — the callback
//!      fires on the *next* scheduler cycle after the observed signal fires,
//!      not synchronously from inside `SetValue`. This preserves the
//!      per-fire-aggregate-signal rep chosen in ADR
//!      2026-04-21-phase-4b-listener-tree-adr.md (R5) without requiring
//!      a second mutable-graph walk during mutation.
//!
//! DIVERGED: `Drop` cannot access the scheduler to `disconnect` the signal or
//! `remove_engine` the owned engine. Callers must invoke
//! `detach(self, &mut SchedCtx<'_>)` before drop. If `detach` is not called,
//! the engine remains registered until the `EngineScheduler` itself is
//! dropped; the listener's callback will still fire for any still-live
//! connected signal. This is a leak but not a safety issue. C++ relies on
//! deterministic `~emRecListener()` running `SetListenedRec(NULL)` — not
//! available in Rust safe code without scheduler access in `Drop`.
//!
//! Design choice (Phase 4c Task 2): Option A from the task plan — added
//! `emRecNode::listened_signal()` so `SetListenedRec` can accept
//! `Option<&dyn emRecNode>` and extract the observed `SignalId`
//! polymorphically. Primitives return their value signal; compounds
//! (Phase 4c Tasks 3-5) will return their aggregate signal.

use crate::emEngine::{emEngine, EngineId, Priority};
use crate::emEngineCtx::{EngineCtx, SchedCtx};
use crate::emPanelScope::PanelScope;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

/// Internal engine wrapping the listener's callback. Its sole job is to
/// invoke the callback when Cycle is called (i.e. a connected signal fired).
struct ListenerEngine {
    callback: Box<dyn FnMut(&mut SchedCtx<'_>)>,
}

impl emEngine for ListenerEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let mut sc = ctx.as_sched_ctx();
        (self.callback)(&mut sc);
        false // go back to sleep until the next signal fires
    }
}

pub struct emRecListener {
    engine_id: EngineId,
    /// Signal currently being observed (the rec's `listened_signal`). `None`
    /// means detached.
    signal: Option<SignalId>,
}

impl emRecListener {
    /// Construct a listener attached to `rec` (or unattached if `rec` is
    /// `None`). The callback fires on the scheduler cycle following any
    /// `SetValue` mutation on a primitive, or on any aggregate-signal fire
    /// for a compound.
    ///
    /// DIVERGED: C++ `emRecListener::emRecListener(emRec*)` (emRec.cpp:228-233)
    /// takes a raw `emRec*`. Rust takes `Option<&dyn emRecNode>` so the same
    /// API handles primitives and (Phase 4c Tasks 3-5) compounds without a
    /// value-type generic. Also takes `&mut SchedCtx` rather than constructing
    /// at an arbitrary point — the listener needs both `register_engine` and
    /// `connect`, and only `SchedCtx` / `EngineCtx` expose the latter.
    pub fn new(
        rec: Option<&dyn emRecNode>,
        callback: Box<dyn FnMut(&mut SchedCtx<'_>)>,
        ctx: &mut SchedCtx<'_>,
    ) -> Self {
        // Framework scope: the listener isn't bound to a panel tree.
        // Priority::Low: user callbacks shouldn't preempt rendering / input dispatch.
        let engine_id = ctx.register_engine(
            Box::new(ListenerEngine { callback }),
            Priority::Low,
            PanelScope::Framework,
        );
        let mut this = Self {
            engine_id,
            signal: None,
        };
        if let Some(r) = rec {
            let sig = r.listened_signal();
            ctx.connect(sig, engine_id);
            this.signal = Some(sig);
        }
        this
    }

    /// Re-target (or detach) the listener. Mirrors C++
    /// `emRecListener::SetListenedRec` (emRec.cpp:242-268): no-op when the
    /// target is the same rec; otherwise disconnect the old signal and
    /// connect the new.
    pub fn SetListenedRec(&mut self, rec: Option<&dyn emRecNode>, ctx: &mut SchedCtx<'_>) {
        let new_sig = rec.map(|r| r.listened_signal());
        if new_sig == self.signal {
            return;
        }
        if let Some(old) = self.signal.take() {
            ctx.disconnect(old, self.engine_id);
        }
        if let Some(new) = new_sig {
            ctx.connect(new, self.engine_id);
            self.signal = Some(new);
        }
    }

    /// Accessor paralleling C++ `emRecListener::GetListenedRec`
    /// (emRec.h:262-264). Returns the currently observed `SignalId` — not
    /// the `emRec*` itself, since Rust does not store a back-pointer (the
    /// rec's ownership lifetime is not tied to the listener in our rep).
    ///
    /// DIVERGED: C++ returns `emRec*`; Rust returns `Option<SignalId>`
    /// because the listener only retains the observed signal, not a
    /// pointer back to the rec. Callers that need the rec track it
    /// externally (idiomatic in our ownership model).
    pub fn GetListenedSignal(&self) -> Option<SignalId> {
        self.signal
    }

    /// Non-consuming teardown: disconnects the currently observed signal (if
    /// any) and removes the internal engine from the scheduler. The listener
    /// is left in a "detached zombie" state — safe to drop, but calling
    /// `SetListenedRec` afterwards is a logic bug because `engine_id` is now
    /// stale and points at a removed engine. Intended for struct-field
    /// listeners (e.g. compound records in Tasks 3-5) that must survive
    /// their owner's lifetime cycles without an `Option<_>` + `.take()` dance.
    pub fn detach_mut(&mut self, ctx: &mut SchedCtx<'_>) {
        if let Some(sig) = self.signal.take() {
            ctx.disconnect(sig, self.engine_id);
        }
        ctx.remove_engine(self.engine_id);
        // engine_id is now stale — any further SetListenedRec would be a logic bug.
    }

    /// Explicit teardown: disconnects the signal and removes the internal
    /// engine. Must be called before drop to avoid leaking the engine in
    /// the scheduler. See module-level DIVERGED note on `Drop`.
    pub fn detach(mut self, ctx: &mut SchedCtx<'_>) {
        self.detach_mut(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emBoolRec::emBoolRec;
    use crate::emClipboard::emClipboard;
    use crate::emContext::emContext;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction};
    use crate::emRec::emRec;
    use crate::emScheduler::EngineScheduler;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
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

    /// Run one full scheduler time slice using empty window/input state.
    /// Duplicated from tests in emScheduler / primitive tests — anti-scope
    /// to hoist per CLAUDE.md Code Rules.
    fn run_slice(sched: &mut EngineScheduler) {
        let mut windows = HashMap::new();
        let root = emContext::NewRoot();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fc: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        sched.DoTimeSlice(
            &mut windows,
            &root,
            &mut actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &pa,
        );
    }

    #[test]
    fn callback_fires_on_primitive_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emBoolRec::new(&mut sc, false);
        let hits = Rc::new(Cell::new(0u32));
        let hits_cb = Rc::clone(&hits);
        let listener = emRecListener::new(
            Some(&rec),
            Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
            &mut sc,
        );

        rec.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 1, "callback must fire after SetValue");

        // Clean up (without this, EngineScheduler drop panics on residual state).
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        listener.detach(&mut sc);
        sc.remove_signal(rec.GetValueSignal());
    }

    #[test]
    fn detach_stops_firing() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emBoolRec::new(&mut sc, false);
        let hits = Rc::new(Cell::new(0u32));
        let hits_cb = Rc::clone(&hits);
        let mut listener = emRecListener::new(
            Some(&rec),
            Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
            &mut sc,
        );

        // Detach.
        listener.SetListenedRec(None, &mut sc);
        assert!(listener.GetListenedSignal().is_none());

        // Mutate — no callback should fire.
        rec.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 0, "detached listener must not fire");

        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        listener.detach(&mut sc);
        sc.remove_signal(rec.GetValueSignal());
    }

    #[test]
    fn retarget_switches_observed_rec() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec_a = emBoolRec::new(&mut sc, false);
        let mut rec_b = emBoolRec::new(&mut sc, false);
        let hits = Rc::new(Cell::new(0u32));
        let hits_cb = Rc::clone(&hits);
        let mut listener = emRecListener::new(
            Some(&rec_a),
            Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
            &mut sc,
        );

        // Attached to A: mutating A fires.
        rec_a.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 1);

        // Re-target to B.
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        listener.SetListenedRec(Some(&rec_b), &mut sc);

        // Mutating A should NOT fire.
        rec_a.SetValue(false, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 1, "after re-target, old rec must not fire");

        // Mutating B SHOULD fire.
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        rec_b.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 2, "after re-target, new rec must fire");

        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        listener.detach(&mut sc);
        sc.remove_signal(rec_a.GetValueSignal());
        sc.remove_signal(rec_b.GetValueSignal());
    }

    #[test]
    fn attach_via_set_listened_rec_after_none_construction() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let hits = Rc::new(Cell::new(0u32));
        let hits_cb = Rc::clone(&hits);
        let mut listener = emRecListener::new(
            None,
            Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
            &mut sc,
        );
        assert!(
            listener.GetListenedSignal().is_none(),
            "None-construction leaves listener detached"
        );

        // Mutating an unrelated rec must not fire the callback.
        let mut unrelated = emBoolRec::new(&mut sc, false);
        unrelated.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 0, "unattached listener must not fire");

        // Attach after construction.
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let mut rec = emBoolRec::new(&mut sc, false);
        listener.SetListenedRec(Some(&rec), &mut sc);
        assert_eq!(listener.GetListenedSignal(), Some(rec.GetValueSignal()));

        rec.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 1, "after attach, mutation must fire callback");

        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        listener.detach(&mut sc);
        sc.remove_signal(rec.GetValueSignal());
        sc.remove_signal(unrelated.GetValueSignal());
    }

    #[test]
    fn same_rec_is_idempotent() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emBoolRec::new(&mut sc, false);
        let hits = Rc::new(Cell::new(0u32));
        let hits_cb = Rc::clone(&hits);
        let mut listener = emRecListener::new(
            Some(&rec),
            Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
            &mut sc,
        );

        // Re-set to same rec: should be a no-op (no disconnect/reconnect).
        listener.SetListenedRec(Some(&rec), &mut sc);

        rec.SetValue(true, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
        assert_eq!(hits.get(), 1, "callback still fires exactly once");

        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        listener.detach(&mut sc);
        sc.remove_signal(rec.GetValueSignal());
    }
}
