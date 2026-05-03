// DIVERGED: (language-forced) C++ emPanel inherits from emEngine directly (emPanel.h:33 —
// `class emPanel : public emEngine`). In Rust, `PanelBehavior` trait
// objects are owned by `PanelTree::panels` and are `take`n during cycling
// so the tree can lend a `PanelCtx`; a `PanelBehavior` therefore cannot
// simultaneously live in the scheduler's `Box<dyn emEngine>` slot-map.
// This adapter is the minimum concession: one adapter engine per panel,
// registered with the scheduler, whose `Cycle` drives the panel's
// `PanelBehavior::Cycle` via the standard take/put path.
//
// Observable behavior matches C++ (panel cycling runs through the
// scheduler's normal engine loop, uses the panel's own view's
// `CurrentPixelTallness`).

use super::emEngine::emEngine;
use super::emEngineCtx::EngineCtx;
use super::emEngineCtx::PanelCtx;
use super::emPanelScope::PanelScope;
use super::emPanelTree::PanelId;
use super::emScheduler::EngineScheduler;

/// Probe attached to a `PanelCycleEngine` in test/test-support builds.
/// Records the scheduler's `time_slice_counter` on the engine's first
/// `Cycle` invocation. Used by SP4.5-FIX-1 timing fixtures (Tasks 5-7)
/// to measure slices-between-create-and-first-Cycle.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub(crate) struct PanelCycleEngineFirstCycleProbe {
    pub captured_slice: std::rc::Rc<std::cell::Cell<Option<u64>>>,
}

/// F019: Counts every `Cycle` dispatch on the engine. Used by the F019
/// proof-of-fix test to assert the panel re-cycles only on observable
/// state-change events (FileStateSignal fires), not on every scheduler
/// slice — i.e., that the retired `stay_awake`-while-loading polling
/// has not regressed.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub(crate) struct PanelCycleEngineCycleCounter {
    pub count: std::rc::Rc<std::cell::Cell<u32>>,
}

pub(crate) struct PanelCycleEngine {
    pub(crate) panel_id: PanelId,
    /// Phase 2 Task 5: replaces `view: Weak<RefCell<emView>>`. The engine
    /// now identifies its owning view by scope (top-level `WindowId` or
    /// sub-view panel id), resolved at `Cycle` entry through
    /// `PanelScope::resolve_view`.
    pub(crate) scope: PanelScope,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) first_cycle_probe: Option<PanelCycleEngineFirstCycleProbe>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) cycle_counter: Option<PanelCycleEngineCycleCounter>,
}

impl emEngine for PanelCycleEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(probe) = &self.first_cycle_probe {
            if probe.captured_slice.get().is_none() {
                probe.captured_slice.set(Some(ctx.time_slice_counter()));
            }
        }
        #[cfg(any(test, feature = "test-support"))]
        if let Some(counter) = &self.cycle_counter {
            counter.count.set(counter.count.get() + 1);
        }

        // SAFETY: `ectx.scheduler` and `pctx.scheduler` alias the same
        // `EngineScheduler`. This is sound because:
        //   1. The event loop is single-threaded — no concurrent access.
        //   2. `EngineScheduler` methods (wake_up, register_engine, etc.)
        //      are safe to call re-entrantly from within a Cycle callback;
        //      this mirrors C++ where emEngine::Cycle receives a raw pointer
        //      to the scheduler and may call scheduler methods freely.
        //   3. No two `&mut` operations on distinct subfields alias each
        //      other — all scheduler mutations go through the same handle.
        let sched_ptr: *mut EngineScheduler = &mut *ctx.scheduler;
        let fw_ptr: *mut Vec<crate::emEngineCtx::DeferredAction> = &mut *ctx.framework_actions;

        match self.scope {
            PanelScope::Toplevel(_) | PanelScope::Framework => {
                // ctx.tree IS the tree that owns this panel — take/dispatch/put directly.
                let (tallness, mut behavior) = {
                    let ctx_tree = ctx
                        .tree
                        .as_deref_mut()
                        .expect("PanelCycleEngine: tree is Some for Toplevel engines");
                    let tallness = ctx_tree.cached_pixel_tallness;
                    let Some(b) = ctx_tree.take_behavior(self.panel_id) else {
                        return false;
                    };
                    (tallness, b)
                };
                let (stay_awake, invalidate_requested) = {
                    let mut ectx = crate::emEngineCtx::EngineCtx {
                        scheduler: unsafe { &mut *sched_ptr },
                        tree: None,
                        windows: &mut *ctx.windows,
                        root_context: ctx.root_context,
                        view_context: None,
                        framework_actions: unsafe { &mut *fw_ptr },
                        pending_inputs: &mut *ctx.pending_inputs,
                        input_state: &mut *ctx.input_state,
                        framework_clipboard: ctx.framework_clipboard,
                        engine_id: ctx.engine_id,
                        pending_actions: ctx.pending_actions,
                    };
                    let pctx_tree = ctx
                        .tree
                        .as_deref_mut()
                        .expect("PanelCycleEngine: tree is Some for Toplevel engines");
                    let mut pctx = PanelCtx::with_sched_reach(
                        pctx_tree,
                        self.panel_id,
                        tallness,
                        unsafe { &mut *sched_ptr },
                        unsafe { &mut *fw_ptr },
                        ctx.root_context,
                        ctx.framework_clipboard,
                        ctx.pending_actions,
                    );
                    // Phase 0 (B2.1): CYCLE_ENTRY — emit before
                    // behavior.Cycle so the analyzer can detect whether
                    // DoTimeSlice picked up the woken engine and
                    // PanelCycleEngine routed to a behavior.
                    {
                        let line = format!(
                            "CYCLE_ENTRY|wall_us={}|engine_id={:?}|panel_id={:?}|behavior_type={}\n",
                            crate::emInstr::wall_us(),
                            ctx.engine_id,
                            self.panel_id,
                            std::any::type_name_of_val(&*behavior),
                        );
                        crate::emInstr::write_line(&line);
                    }
                    let busy = behavior.Cycle(&mut ectx, &mut pctx);
                    let inval = pctx.take_invalidate_self_request();
                    {
                        let line = format!(
                            "INVAL_DRAIN|wall_us={}|engine_id={:?}|panel_id={:?}|drained={}\n",
                            crate::emInstr::wall_us(),
                            ctx.engine_id,
                            self.panel_id,
                            if inval { "t" } else { "f" },
                        );
                        crate::emInstr::write_line(&line);
                    }
                    (busy, inval)
                };
                let ctx_tree = ctx
                    .tree
                    .as_deref_mut()
                    .expect("PanelCycleEngine: tree is Some for Toplevel engines");
                if ctx_tree.panels.contains_key(self.panel_id) {
                    ctx_tree.put_behavior(self.panel_id, behavior);
                }
                if invalidate_requested {
                    // RUST_ONLY: (language-forced-utility) — Implements the
                    // drain half of the language-forced
                    // `request_invalidate_self` adapter (see RUST_ONLY in
                    // `emEngineCtx.rs`, `invalidate_self_requested`). What we
                    // mirror is the *effect* of C++
                    // `emPanel::InvalidatePainting()` (no-arg) called inline
                    // from `emTextField::Cycle` (emTextField.cpp:319/325/334)
                    // — C++ has no drain step. SAFETY: the tree raw pointer
                    // aliases `ctx.tree`, but `resolve_view` for Toplevel
                    // scopes only touches `ctx.windows` and `view.dirty_rects`
                    // — `ctx.tree` is not re-borrowed by the closure
                    // (single-threaded; same raw-ptr pattern used above for
                    // sched/framework_actions).
                    let panel_id = self.panel_id;
                    let tree_ptr: *const crate::emPanelTree::PanelTree = ctx
                        .tree
                        .as_deref()
                        .expect("PanelCycleEngine: tree is Some for Toplevel engines");
                    self.scope.resolve_view(ctx, |view, sc| {
                        view.InvalidatePainting(sc, unsafe { &*tree_ptr }, panel_id);
                    });
                }
                stay_awake
            }
            PanelScope::SubView { outer_panel_id, .. } => {
                // ctx.tree is the OUTER tree (handed by the scheduler's SubView arm).
                // self.panel_id lives in the sub-tree, not the outer tree.
                // Navigate outer tree → emSubViewPanel behavior at outer_panel_id → sub_tree,
                // then take/dispatch/put against that sub_tree.
                //
                // Phase 1: take behavior from sub_tree.
                // The borrow of outer_tree (and sub_tree through it) is dropped at end of block,
                // which is required before we re-borrow outer_tree for the dispatch phase.
                let (tallness, mut behavior) = {
                    let outer_tree = ctx
                        .tree
                        .as_deref_mut()
                        .expect("PanelCycleEngine: tree is Some for SubView engines");
                    let Some(svp) = outer_tree
                        .panels
                        .get_mut(outer_panel_id)
                        .and_then(|p| p.behavior.as_mut())
                        .and_then(|b| b.as_sub_view_panel_mut())
                    else {
                        return false;
                    };
                    let sub_tree = svp.sub_tree_mut();
                    let tallness = sub_tree.cached_pixel_tallness;
                    let Some(b) = sub_tree.take_behavior(self.panel_id) else {
                        return false;
                    };
                    (tallness, b)
                };

                // Phase 2: dispatch Cycle against sub_tree.
                // `ectx.windows` borrows `ctx.windows`; `pctx` borrows `ctx.tree` (sub_tree
                // through the outer tree chain). These are distinct fields of `ctx` so the
                // compiler accepts both borrows simultaneously — same pattern as Toplevel.
                let (stay_awake, invalidate_requested) = {
                    let mut ectx = crate::emEngineCtx::EngineCtx {
                        scheduler: unsafe { &mut *sched_ptr },
                        tree: None,
                        windows: &mut *ctx.windows,
                        root_context: ctx.root_context,
                        view_context: None,
                        framework_actions: unsafe { &mut *fw_ptr },
                        pending_inputs: &mut *ctx.pending_inputs,
                        input_state: &mut *ctx.input_state,
                        framework_clipboard: ctx.framework_clipboard,
                        engine_id: ctx.engine_id,
                        pending_actions: ctx.pending_actions,
                    };
                    let outer_tree = ctx
                        .tree
                        .as_deref_mut()
                        .expect("PanelCycleEngine: tree is Some for SubView engines");
                    let svp = outer_tree
                        .panels
                        .get_mut(outer_panel_id)
                        .and_then(|p| p.behavior.as_mut())
                        .and_then(|b| b.as_sub_view_panel_mut())
                        .expect("emSubViewPanel still present after take_behavior");
                    let sub_tree = svp.sub_tree_mut();
                    let mut pctx = PanelCtx::with_sched_reach(
                        sub_tree,
                        self.panel_id,
                        tallness,
                        unsafe { &mut *sched_ptr },
                        unsafe { &mut *fw_ptr },
                        ctx.root_context,
                        ctx.framework_clipboard,
                        ctx.pending_actions,
                    );
                    // Phase 0 (B2.1): CYCLE_ENTRY — emit before
                    // behavior.Cycle so the analyzer can detect whether
                    // DoTimeSlice picked up the woken engine and
                    // PanelCycleEngine routed to a behavior.
                    {
                        let line = format!(
                            "CYCLE_ENTRY|wall_us={}|engine_id={:?}|panel_id={:?}|behavior_type={}\n",
                            crate::emInstr::wall_us(),
                            ctx.engine_id,
                            self.panel_id,
                            std::any::type_name_of_val(&*behavior),
                        );
                        crate::emInstr::write_line(&line);
                    }
                    let busy = behavior.Cycle(&mut ectx, &mut pctx);
                    let inval = pctx.take_invalidate_self_request();
                    {
                        let line = format!(
                            "INVAL_DRAIN|wall_us={}|engine_id={:?}|panel_id={:?}|drained={}\n",
                            crate::emInstr::wall_us(),
                            ctx.engine_id,
                            self.panel_id,
                            if inval { "t" } else { "f" },
                        );
                        crate::emInstr::write_line(&line);
                    }
                    (busy, inval)
                };

                // Phase 3: put behavior back, then (if requested) invalidate.
                // Re-navigate (borrow from phase 2 dropped). Both put_behavior
                // and the InvalidatePainting drain happen inside the same
                // svp borrow window so we don't need a second raw-ptr re-derivation.
                //
                // RUST_ONLY: (language-forced-utility) — Implements the drain
                // half of the language-forced `request_invalidate_self`
                // adapter (see RUST_ONLY in `emEngineCtx.rs`,
                // `invalidate_self_requested`). What we mirror is the *effect*
                // of C++ `emPanel::InvalidatePainting()` (no-arg) called inline
                // from `emTextField::Cycle` — C++ has no drain step. The
                // disjoint-field borrow `svp.sub_view_and_tree_mut()` lets us
                // call `view.InvalidatePainting(sched, &sub_tree, panel_id)`
                // without aliasing `svp` through the outer tree twice.
                let outer_tree = ctx
                    .tree
                    .as_deref_mut()
                    .expect("PanelCycleEngine: tree is Some for SubView engines");
                if let Some(svp) = outer_tree
                    .panels
                    .get_mut(outer_panel_id)
                    .and_then(|p| p.behavior.as_mut())
                    .and_then(|b| b.as_sub_view_panel_mut())
                {
                    let (sub_view, sub_tree) = svp.sub_view_and_tree_mut();
                    if sub_tree.panels.contains_key(self.panel_id) {
                        sub_tree.put_behavior(self.panel_id, behavior);
                    }
                    if invalidate_requested {
                        let mut sched_ctx = crate::emEngineCtx::SchedCtx {
                            scheduler: unsafe { &mut *sched_ptr },
                            framework_actions: unsafe { &mut *fw_ptr },
                            root_context: ctx.root_context,
                            view_context: None,
                            framework_clipboard: ctx.framework_clipboard,
                            current_engine: Some(ctx.engine_id),
                            pending_actions: ctx.pending_actions,
                        };
                        sub_view.InvalidatePainting(&mut sched_ctx, sub_tree, self.panel_id);
                    }
                }
                stay_awake
            }
        }
    }
}
