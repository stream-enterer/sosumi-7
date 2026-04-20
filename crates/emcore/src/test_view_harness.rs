// RUST_ONLY: test harness for Phase 1.5 keystone migration.
//
// Bundles `EngineScheduler`, `Vec<DeferredAction>`, `Rc<emContext>`,
// `PanelTree`, and `HashMap<WindowId, emWindow>` so unit tests can
// construct the four ctx types (`EngineCtx`, `SchedCtx`, `InitCtx`)
// without duplicating setup across ~150 test sites.
//
// Introduced by Phase 1.5 Task 1 step 1a. Consumed by step 1h's test
// rewire pass.

#![cfg(any(test, feature = "test-support"))]

use std::collections::HashMap;
use std::rc::Rc;

use winit::window::WindowId;

use crate::emContext::emContext;
use crate::emEngine::EngineId;
use crate::emEngineCtx::{DeferredAction, EngineCtx, InitCtx, SchedCtx};
use crate::emPanelTree::PanelTree;
use crate::emScheduler::EngineScheduler;
use crate::emWindow::emWindow;

/// Bundle of the framework-owned state needed to construct `EngineCtx`,
/// `SchedCtx`, and `InitCtx` in unit tests.
pub struct TestViewHarness {
    pub scheduler: EngineScheduler,
    pub framework_actions: Vec<DeferredAction>,
    pub root_context: Rc<emContext>,
    pub framework_clipboard: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>>,
    pub tree: PanelTree,
    pub windows: HashMap<WindowId, emWindow>,
    pub pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)>,
    pub input_state: crate::emInputState::emInputState,
}

impl Default for TestViewHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl TestViewHarness {
    pub fn new() -> Self {
        Self {
            scheduler: EngineScheduler::new(),
            framework_actions: Vec::new(),
            root_context: emContext::NewRoot(),
            framework_clipboard: std::cell::RefCell::new(None),
            tree: PanelTree::new(),
            windows: HashMap::new(),
            pending_inputs: Vec::new(),
            input_state: crate::emInputState::emInputState::new(),
        }
    }

    /// Construct a `SchedCtx` covering the harness's scheduler / actions /
    /// root context. `current_engine` is `None` (InitCtx-equivalent scope);
    /// use `sched_ctx_for(engine_id)` to thread a specific engine.
    pub fn sched_ctx(&mut self) -> SchedCtx<'_> {
        SchedCtx {
            scheduler: &mut self.scheduler,
            framework_actions: &mut self.framework_actions,
            root_context: &self.root_context,
            framework_clipboard: &self.framework_clipboard,
            current_engine: None,
        }
    }

    /// Construct a `SchedCtx` whose `current_engine` is pre-populated.
    pub fn sched_ctx_for(&mut self, engine: EngineId) -> SchedCtx<'_> {
        SchedCtx {
            scheduler: &mut self.scheduler,
            framework_actions: &mut self.framework_actions,
            root_context: &self.root_context,
            framework_clipboard: &self.framework_clipboard,
            current_engine: Some(engine),
        }
    }

    /// Construct an `EngineCtx` simulating an engine-dispatch slice.
    /// Callers pass the engine id that would be currently dispatched.
    pub fn engine_ctx(&mut self, engine_id: EngineId) -> EngineCtx<'_> {
        EngineCtx {
            scheduler: &mut self.scheduler,
            tree: &mut self.tree,
            windows: &mut self.windows,
            root_context: &self.root_context,
            framework_actions: &mut self.framework_actions,
            pending_inputs: &mut self.pending_inputs,
            input_state: &mut self.input_state,
            framework_clipboard: &self.framework_clipboard,
            engine_id,
        }
    }

    /// Construct an `InitCtx` (no engine currently running; framework-init
    /// scope).
    pub fn init_ctx(&mut self) -> InitCtx<'_> {
        InitCtx {
            scheduler: &mut self.scheduler,
            framework_actions: &mut self.framework_actions,
            root_context: &self.root_context,
        }
    }
}

/// Lightweight test helper: owns the data needed to construct a `SchedCtx`.
/// Use `.with(|sc| ...)` to call ctx-taking emView / emViewAnimator methods in
/// tests that don't need a full `TestViewHarness`.
pub struct TestSched {
    sched: EngineScheduler,
    fw: Vec<DeferredAction>,
    ctx: Rc<emContext>,
    cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>>,
}

impl Default for TestSched {
    fn default() -> Self {
        Self::new()
    }
}

impl TestSched {
    pub fn new() -> Self {
        Self {
            sched: EngineScheduler::new(),
            fw: Vec::new(),
            ctx: emContext::NewRoot(),
            cb: std::cell::RefCell::new(None),
        }
    }

    pub fn with<R>(&mut self, f: impl FnOnce(&mut SchedCtx<'_>) -> R) -> R {
        let mut sc = SchedCtx {
            scheduler: &mut self.sched,
            framework_actions: &mut self.fw,
            root_context: &self.ctx,
            framework_clipboard: &self.cb,
            current_engine: None,
        };
        f(&mut sc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_bundles_all_four_pieces() {
        let mut h = TestViewHarness::new();
        assert!(h.framework_actions.is_empty());
        assert!(h.windows.is_empty());

        // sched_ctx — no engine
        {
            let sc = h.sched_ctx();
            assert!(sc.current_engine.is_none());
        }

        // init_ctx — construct-only
        {
            let _ic = h.init_ctx();
        }
    }
}
