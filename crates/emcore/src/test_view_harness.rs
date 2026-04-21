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

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use winit::window::WindowId;

use crate::emContext::emContext;
use crate::emEngine::EngineId;
use crate::emEngineCtx::{DeferredAction, EngineCtx, FrameworkDeferredAction, InitCtx, SchedCtx};
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
    /// Phase 3.5 Task 2: closure-rail handle threaded through ctx constructors.
    pub pending_actions: Rc<RefCell<Vec<FrameworkDeferredAction>>>,
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
            pending_actions: Rc::new(RefCell::new(Vec::new())),
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
            pending_actions: &self.pending_actions,
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
            pending_actions: &self.pending_actions,
        }
    }

    /// Construct an `EngineCtx` simulating an engine-dispatch slice.
    /// Callers pass the engine id that would be currently dispatched.
    pub fn engine_ctx(&mut self, engine_id: EngineId) -> EngineCtx<'_> {
        EngineCtx {
            scheduler: &mut self.scheduler,
            // Phase 3.5.A Task 6.2: tests using this harness simulate
            // engine dispatch; hand the tree as Some so Framework-
            // classified test engines that DO touch ctx.tree (pre-6.2
            // style) still work. Framework-true engines ignore `_ctx`.
            tree: Some(&mut self.tree),
            windows: &mut self.windows,
            root_context: &self.root_context,
            framework_actions: &mut self.framework_actions,
            pending_inputs: &mut self.pending_inputs,
            input_state: &mut self.input_state,
            framework_clipboard: &self.framework_clipboard,
            engine_id,
            pending_actions: &self.pending_actions,
        }
    }

    /// Construct an `InitCtx` (no engine currently running; framework-init
    /// scope).
    pub fn init_ctx(&mut self) -> InitCtx<'_> {
        InitCtx {
            scheduler: &mut self.scheduler,
            framework_actions: &mut self.framework_actions,
            root_context: &self.root_context,
            pending_actions: &self.pending_actions,
        }
    }
}

/// Lightweight test helper: owns the three buffers needed to construct an
/// `InitCtx`. Use `new()` to build the harness, then `ctx()` to borrow a
/// short-lived `InitCtx<'_>` from it.
///
/// This is the canonical shared fixture for plugin-invocation tests across
/// `emcore`, `eaglemode`, and `emstocks` (Phase-3 Task-5 cleanup). Tests
/// that also need a panel tree or windows should use `TestViewHarness`
/// (and its `init_ctx()` method) instead.
///
/// Introduced by Phase-3 Task-5 cleanup to eliminate four near-identical
/// local definitions.
pub struct InitHarness {
    pub scheduler: EngineScheduler,
    pub actions: Vec<DeferredAction>,
    pub root: Rc<emContext>,
    /// Phase 3.5 Task 2: closure-rail handle threaded through InitCtx.
    pub pending_actions: Rc<RefCell<Vec<FrameworkDeferredAction>>>,
}

impl Default for InitHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl InitHarness {
    pub fn new() -> Self {
        Self {
            scheduler: EngineScheduler::new(),
            actions: Vec::new(),
            root: emContext::NewRoot(),
            pending_actions: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn ctx(&mut self) -> InitCtx<'_> {
        InitCtx {
            scheduler: &mut self.scheduler,
            framework_actions: &mut self.actions,
            root_context: &self.root,
            pending_actions: &self.pending_actions,
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
    /// Phase 3.5 Task 2: closure-rail handle threaded through SchedCtx.
    pa: Rc<RefCell<Vec<FrameworkDeferredAction>>>,
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
            pa: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn with<R>(&mut self, f: impl FnOnce(&mut SchedCtx<'_>) -> R) -> R {
        let mut sc = SchedCtx {
            scheduler: &mut self.sched,
            framework_actions: &mut self.fw,
            root_context: &self.ctx,
            framework_clipboard: &self.cb,
            current_engine: None,
            pending_actions: &self.pa,
        };
        f(&mut sc)
    }
}

/// Phase 3.5.A Task 6.2 test helper: wrap a detached `PanelTree` in a
/// headless (Pending) `emWindow` so tests that register `Toplevel(wid)`
/// engines have a tree the scheduler can take/put. Returns
/// `(WindowId::dummy(), emWindow)`; caller is expected to insert into a
/// `HashMap<WindowId, emWindow>` and later drain for teardown.
pub fn headless_emwindow_with_tree(
    root_ctx: &Rc<emContext>,
    scheduler: &mut EngineScheduler,
    tree: PanelTree,
) -> (WindowId, emWindow) {
    use crate::emColor::emColor;
    use crate::emWindow::WindowFlags;
    let close_sig = scheduler.create_signal();
    let flags_sig = scheduler.create_signal();
    let focus_sig = scheduler.create_signal();
    let geom_sig = scheduler.create_signal();
    // Phase 3.5.A Task 8: `new_popup_pending` now builds its own internal
    // tree + root. For this harness we discard that internal tree and
    // install the caller's rooted tree. The view's `root: PanelId` still
    // points at the discarded internal root — acceptable for Framework and
    // Toplevel test engines that only read `ctx.tree` (not view.root) per
    // the classification sheet's engine contracts.
    let mut win = emWindow::new_popup_pending(
        Rc::clone(root_ctx),
        WindowFlags::empty(),
        "headless".to_string(),
        close_sig,
        flags_sig,
        focus_sig,
        geom_sig,
        emColor::TRANSPARENT,
    );
    let _ = win.take_tree();
    win.put_tree(tree);
    (WindowId::dummy(), win)
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
