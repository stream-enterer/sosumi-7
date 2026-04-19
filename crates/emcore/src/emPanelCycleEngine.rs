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

pub(crate) struct PanelCycleEngine {
    pub(crate) panel_id: PanelId,
    pub(crate) view: Weak<RefCell<emView>>,
}

impl emEngine for PanelCycleEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
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
