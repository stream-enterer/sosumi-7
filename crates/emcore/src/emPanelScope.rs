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
                    current_engine: Some(ctx.engine_id),
                };
                Some(f(view, &mut sched_ctx))
            }
            PanelScope::SubView(_pid) => {
                // Sub-view resolution threads through the owning panel's sub_view.
                // Phase 2 Task 5 wires this; stubbed here so callers compile.
                None
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
