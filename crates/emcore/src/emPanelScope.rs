//! PanelScope — identifies where a panel-associated engine resolves its view.
//!
//! See `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.2.

use winit::window::WindowId;

use crate::emPanelTree::PanelId;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PanelScope {
    Toplevel(WindowId),
    SubView(PanelId),
}

impl PanelScope {
    /// Resolve to a `&mut emView` through `EngineCtx`. Returns `None` if
    /// the target window or sub-view panel no longer exists (e.g. the
    /// owning panel was removed between engine registration and Cycle).
    pub fn resolve_view<R>(
        self,
        ctx: &mut crate::emEngineCtx::EngineCtx<'_>,
        f: impl FnOnce(&mut crate::emView::emView, &mut crate::emEngineCtx::SchedCtx<'_>) -> R,
    ) -> Option<R> {
        match self {
            PanelScope::Toplevel(wid) => {
                let window = ctx.windows.get_mut(&wid)?;
                // Phase 2 Task 2: window.view is now a plain emView; borrow
                // it directly. The split borrow (windows vs. scheduler) is
                // OK because `as_sched_ctx` only touches fields other than
                // `windows`.
                let view: &mut crate::emView::emView = &mut window.view;
                let mut sched_ctx = crate::emEngineCtx::SchedCtx {
                    scheduler: ctx.scheduler,
                    framework_actions: ctx.framework_actions,
                    root_context: ctx.root_context,
                    framework_clipboard: ctx.framework_clipboard,
                    current_engine: Some(ctx.engine_id),
                };
                Some(f(view, &mut sched_ctx))
            }
            PanelScope::SubView(pid) => {
                // Phase 2 Task 5: wire sub-view resolution.
                //
                // `pid` is the outer-tree panel id of an `emSubViewPanel`.
                // We search the outer-most reachable `PanelTree` (via
                // `ctx.tree`) for that panel, then reach its `sub_view`
                // through the typed `as_sub_view_panel_mut` accessor (no
                // `Any` / `downcast_mut` — spec rule).
                //
                // Borrow shape: the search takes `&mut ctx.tree`, the
                // `SchedCtx` we build is disjoint (only scheduler /
                // framework_actions / root_context / engine_id). This
                // mirrors the `Toplevel` branch pattern.
                //
                // Known limitation (pre-Task-7): when `PanelCycleEngine`
                // for a panel *inside* a sub-tree cycles, `ctx.tree` is
                // already the inner sub-tree (the outer emSubViewPanel's
                // behavior is held by the scheduler's dispatch walk), so
                // this lookup returns `None` — the engine sleeps for that
                // slice. Tasks 6/7 finalize the dispatch/resolution shape.
                let engine_id = ctx.engine_id;
                let sched_ptr: *mut crate::emScheduler::EngineScheduler = &mut *ctx.scheduler;
                let fw_ptr: *mut Vec<crate::emEngineCtx::DeferredAction> =
                    &mut *ctx.framework_actions;
                let svp_opt: Option<&mut crate::emSubViewPanel::emSubViewPanel> = ctx
                    .tree
                    .panels
                    .get_mut(pid)
                    .and_then(|p| p.behavior.as_mut())
                    .and_then(|b| b.as_sub_view_panel_mut());
                let svp = svp_opt?;
                let mut sched_ctx = crate::emEngineCtx::SchedCtx {
                    // SAFETY: `ctx.scheduler` and `ctx.framework_actions`
                    // are disjoint from `ctx.tree.panels` (the borrow that
                    // produced `svp`). Reifying them as raw pointers here
                    // only avoids the compiler's overly coarse borrow
                    // check on `ctx: &mut EngineCtx`; single-threaded
                    // use, no aliasing with `svp`.
                    scheduler: unsafe { &mut *sched_ptr },
                    framework_actions: unsafe { &mut *fw_ptr },
                    root_context: ctx.root_context,
                    framework_clipboard: ctx.framework_clipboard,
                    current_engine: Some(engine_id),
                };
                Some(f(&mut svp.sub_view, &mut sched_ctx))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::Key as _;

    #[test]
    fn scope_variants_exist() {
        let _ = PanelScope::Toplevel(WindowId::dummy());
        let _ = PanelScope::SubView(PanelId::null());
    }
}
