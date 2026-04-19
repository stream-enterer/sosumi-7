// DIVERGED: C++ emPanel inherits from emEngine directly (emPanel.h:33 —
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

use std::cell::RefCell;
use std::rc::Weak;

use super::emEngine::{emEngine, EngineCtx};
use super::emPanelCtx::PanelCtx;
use super::emPanelTree::PanelId;
use super::emView::emView;

/// Probe attached to a `PanelCycleEngine` in test/test-support builds.
/// Records the scheduler's `time_slice_counter` on the engine's first
/// `Cycle` invocation. Used by SP4.5-FIX-1 timing fixtures (Tasks 5-7)
/// to measure slices-between-create-and-first-Cycle.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub(crate) struct PanelCycleEngineFirstCycleProbe {
    pub captured_slice: std::rc::Rc<std::cell::Cell<Option<u64>>>,
}

pub(crate) struct PanelCycleEngine {
    pub(crate) panel_id: PanelId,
    pub(crate) view: Weak<RefCell<emView>>,
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) first_cycle_probe: Option<PanelCycleEngineFirstCycleProbe>,
}

impl emEngine for PanelCycleEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        #[cfg(any(test, feature = "test-support"))]
        if let Some(probe) = &self.first_cycle_probe {
            if probe.captured_slice.get().is_none() {
                probe.captured_slice.set(Some(ctx.time_slice_counter()));
            }
        }

        // View gone (test teardown / window closed) → sleep.
        let Some(view_rc) = self.view.upgrade() else {
            return false;
        };
        let tallness = view_rc.borrow().GetCurrentPixelTallness();

        // Take the behavior off the tree, build a PanelCtx, drive Cycle,
        // put it back (if the panel still exists — behavior may have called
        // delete_self via ctx).
        let Some(mut behavior) = ctx.tree.take_behavior(self.panel_id) else {
            return false;
        };
        let mut pctx = PanelCtx::new(ctx.tree, self.panel_id, tallness);
        let stay_awake = behavior.Cycle(&mut pctx);
        if ctx.tree.panels.contains_key(self.panel_id) {
            ctx.tree.put_behavior(self.panel_id, behavior);
        }
        stay_awake
    }
}
