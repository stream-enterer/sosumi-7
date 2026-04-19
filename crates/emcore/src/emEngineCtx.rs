//! EngineCtx, SchedCtx, InitCtx — event-loop-threaded mutable-state bundles.
//!
//! This module replaces the `Rc<RefCell<EngineScheduler>>` ownership model.
//! See `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.1.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::emContext::emContext;
use crate::emEngine::{EngineId, Priority};
use crate::emPanelTree::PanelTree;
use crate::emScheduler::EngineScheduler;
use crate::emSignal::SignalId;
use crate::emWindow::emWindow;

pub enum DeferredAction {
    /// Close a winit window after the current time slice. Drained by the
    /// framework's post-cycle action pump so that window teardown does not
    /// happen inside an engine's Cycle.
    CloseWindow(winit::window::WindowId),
    /// Materialize a popup's winit window after the current time slice.
    /// Popup materialization is deferred to the framework pump (Task 10)
    /// so `emView::RawVisitAbs` can request the popup without owning winit.
    MaterializePopup(winit::window::WindowId),
}

/// Engine-cycle context — the replacement for the legacy `emEngine::EngineCtx`.
///
/// Constructed by `EngineScheduler::DoTimeSlice` immediately before each
/// engine's `Cycle` call. Provides access to the scheduler, panel tree,
/// window registry, root context, and the framework-action drain.
pub struct EngineCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub tree: &'a mut PanelTree,
    pub windows: &'a mut HashMap<winit::window::WindowId, Rc<RefCell<emWindow>>>,
    pub root_context: &'a Rc<emContext>,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    /// The ID of the engine currently being cycled. Populated by the
    /// scheduler at Cycle-dispatch time.
    pub engine_id: EngineId,
}

pub struct SchedCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
    pub current_engine: Option<EngineId>,
}

/// Construction-only ctx used before the scheduler has started its first
/// time slice. Intentionally trait-only: exposes `ConstructCtx` so engines
/// can be registered and signals created, but does NOT expose
/// fire/connect/remove.
pub struct InitCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
}

pub trait ConstructCtx {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
    ) -> EngineId;
    fn wake_up(&mut self, eng: EngineId);
}

impl EngineCtx<'_> {
    pub fn framework_action(&mut self, action: DeferredAction) {
        self.framework_actions.push(action);
    }

    pub fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    pub fn fire(&mut self, id: SignalId) {
        self.scheduler.fire(id);
    }

    pub fn remove_signal(&mut self, id: SignalId) {
        self.scheduler.remove_signal(id);
    }

    pub fn wake_up(&mut self, id: EngineId) {
        self.scheduler.wake_up(id);
    }

    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.connect(signal, engine);
    }

    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.disconnect(signal, engine);
    }

    pub fn remove_engine(&mut self, id: EngineId) {
        self.scheduler.remove_engine(id);
    }

    pub fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri)
    }

    /// Check whether a specific signal has been signaled since the last
    /// time this engine's `Cycle` was called.
    ///
    /// Rust equivalent of C++ `emEngine::IsSignaled()`.
    pub fn IsSignaled(&self, signal: SignalId) -> bool {
        self.scheduler
            .is_signaled_for_engine(signal, self.engine_id)
    }

    /// Forwarded to `EngineScheduler::is_pending`. Used by tests that want
    /// to check whether a signal is merely pending (not yet processed).
    pub fn is_signaled(&self, sig: SignalId) -> bool {
        self.scheduler.is_pending(sig)
    }

    pub fn IsTimeSliceAtEnd(&self) -> bool {
        self.scheduler.IsTimeSliceAtEnd()
    }

    /// Current scheduler time-slice counter. Used by SP4.5-FIX-1 timing
    /// fixtures to measure slices-between-create-and-first-Cycle.
    pub fn time_slice_counter(&self) -> u64 {
        self.scheduler.GetTimeSliceCounter()
    }

    /// The engine whose `Cycle` is currently executing.
    pub fn id(&self) -> EngineId {
        self.engine_id
    }
}

impl SchedCtx<'_> {
    pub fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    pub fn fire(&mut self, id: SignalId) {
        self.scheduler.fire(id);
    }

    pub fn remove_signal(&mut self, id: SignalId) {
        self.scheduler.remove_signal(id);
    }

    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.connect(signal, engine);
    }

    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        self.scheduler.disconnect(signal, engine);
    }

    pub fn remove_engine(&mut self, id: EngineId) {
        self.scheduler.remove_engine(id);
    }

    pub fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri)
    }

    pub fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }

    pub fn is_signaled(&self, sig: SignalId) -> bool {
        self.scheduler.is_pending(sig)
    }
}

impl ConstructCtx for SchedCtx<'_> {
    fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri)
    }

    fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }
}

impl ConstructCtx for InitCtx<'_> {
    fn create_signal(&mut self) -> SignalId {
        self.scheduler.create_signal()
    }

    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
    ) -> EngineId {
        self.scheduler.register_engine(behavior, pri)
    }

    fn wake_up(&mut self, eng: EngineId) {
        self.scheduler.wake_up(eng);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngine::emEngine;
    use crate::emScheduler::EngineScheduler;

    struct NoopEngine;

    impl emEngine for NoopEngine {
        fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
            false
        }
    }

    #[test]
    fn sched_ctx_exposes_full_api() {
        let mut sched = EngineScheduler::new();
        let mut actions = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            current_engine: None,
        };

        assert!(sc.current_engine.is_none());
        assert!(sc.framework_actions.is_empty());
        assert!(Rc::strong_count(sc.root_context) >= 1);

        let sig_a = sc.create_signal();
        let sig_b = sc.create_signal();
        assert_ne!(sig_a, sig_b);

        assert!(!sc.scheduler.is_pending(sig_a));
        sc.fire(sig_a);
        assert!(sc.scheduler.is_pending(sig_a));
        assert!(!sc.scheduler.is_pending(sig_b));

        sc.remove_signal(sig_a);
        sc.fire(sig_a);
        assert!(!sc.scheduler.is_pending(sig_a));
    }

    #[test]
    fn sched_ctx_connect_disconnect_and_engine_lifecycle() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            current_engine: None,
        };

        let sig = sc.create_signal();
        let eng = sc.register_engine(Box::new(NoopEngine), Priority::Medium);

        sc.connect(sig, eng);
        sc.disconnect(sig, eng);

        sc.wake_up(eng);
        sc.remove_engine(eng);
    }

    #[test]
    fn init_ctx_construct_ctx_trait() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut ic = InitCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
        };

        assert!(ic.framework_actions.is_empty());
        assert!(Rc::strong_count(ic.root_context) >= 1);

        let sig = <InitCtx as ConstructCtx>::create_signal(&mut ic);
        let eng = <InitCtx as ConstructCtx>::register_engine(
            &mut ic,
            Box::new(NoopEngine),
            Priority::High,
        );
        <InitCtx as ConstructCtx>::wake_up(&mut ic, eng);

        assert!(!ic.scheduler.is_pending(sig));

        ic.scheduler.remove_engine(eng);
    }

    #[test]
    fn sched_ctx_construct_ctx_trait() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            current_engine: None,
        };
        let cc: &mut dyn ConstructCtx = &mut sc;
        let _sig = cc.create_signal();
        let eng = cc.register_engine(Box::new(NoopEngine), Priority::VeryHigh);
        cc.wake_up(eng);

        sc.remove_engine(eng);
    }

    #[test]
    fn is_signaled_tracks_fire_and_remove() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let mut sc = SchedCtx {
            scheduler: &mut sched,
            framework_actions: &mut actions,
            root_context: &ctx_root,
            current_engine: None,
        };

        let sig = sc.create_signal();
        assert!(!sc.is_signaled(sig));
        sc.fire(sig);
        assert!(sc.is_signaled(sig));
        sc.remove_signal(sig);
        assert!(!sc.is_signaled(sig));
    }

    #[test]
    fn deferred_action_variants_constructible() {
        let wid = winit::window::WindowId::dummy();
        let actions: Vec<DeferredAction> = vec![
            DeferredAction::CloseWindow(wid),
            DeferredAction::MaterializePopup(wid),
        ];
        assert_eq!(actions.len(), 2);
        for a in &actions {
            match a {
                DeferredAction::CloseWindow(_) | DeferredAction::MaterializePopup(_) => {}
            }
        }
    }
}
