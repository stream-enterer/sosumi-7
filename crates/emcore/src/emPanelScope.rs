//! PanelScope — identifies where a panel-associated engine resolves its view.
//!
//! See `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.2
//! and `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-a-runtime-toplevel-windows.md`
//! §Task 5.

use winit::window::WindowId;

use crate::emPanelTree::PanelId;

/// Scope classification for an engine registered with the scheduler.
///
/// Phase 3.5.A extends the pre-3.5.A `{Toplevel, SubView}` pair with a
/// `Framework` variant for engines that span windows (e.g. the top-level
/// input-dispatch engine, mini-IPC engine, clipboard engine — anything the
/// scheduler's per-window tree detach/attach dance must NOT try to resolve
/// a tree for).
///
/// ## Flat `SubView` (no `rest` chain)
///
/// The spec originally sketched a recursive `rest: Box<PanelScope>` chain
/// to express nested sub-views. The current codebase only ever registers a
/// single level of sub-view (outer `emView` → `emSubViewPanel` → inner
/// `emView`), so `SubView` is a flat struct variant. Keeping it flat
/// preserves `Copy` on the enum and keeps the scheduler dispatch path
/// uniform. If multi-level nesting becomes necessary, revisit the `rest`
/// chain — but do NOT introduce it speculatively.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PanelScope {
    /// Engine spans windows / is not associated with any single panel tree.
    ///
    /// At Cycle entry the scheduler leaves `ctx.tree == None` — the engine
    /// resolves its own targets (usually through `ctx.windows`, the
    /// framework-action queue, or the root context). `resolve_view`
    /// returns `None` for this variant: there is no single view to hand
    /// back.
    Framework,

    /// Engine is bound to the top-level `emView` of a specific window.
    ///
    /// Resolution walks `ctx.windows.get_mut(&wid)` and hands the
    /// window's `view` to the callback.
    Toplevel(WindowId),

    /// Engine is bound to an `emSubViewPanel`'s inner `sub_view`, hosted
    /// inside the outer panel with id `outer_panel_id` in the tree of
    /// window `window_id`.
    ///
    /// Flat by design (see enum doc). `window_id` is stored so that once
    /// the scheduler's scope-directed walk lands in the right window's
    /// tree (Task 6), this variant's resolution starts from the correct
    /// panel set without guessing.
    SubView {
        window_id: WindowId,
        outer_panel_id: PanelId,
    },
}

impl PanelScope {
    /// Window this scope belongs to, or `None` for `Framework`.
    pub fn window_id(&self) -> Option<WindowId> {
        match self {
            PanelScope::Framework => None,
            PanelScope::Toplevel(wid) => Some(*wid),
            PanelScope::SubView { window_id, .. } => Some(*window_id),
        }
    }

    /// Resolve to a `&mut emView` through `EngineCtx`. Returns `None` if
    /// the scope has no view (Framework) or the target window or sub-view
    /// panel no longer exists (e.g. the owning panel was removed between
    /// engine registration and Cycle).
    pub fn resolve_view<R>(
        self,
        ctx: &mut crate::emEngineCtx::EngineCtx<'_>,
        f: impl FnOnce(&mut crate::emView::emView, &mut crate::emEngineCtx::SchedCtx<'_>) -> R,
    ) -> Option<R> {
        match self {
            PanelScope::Framework => {
                // Framework-scoped engines do not resolve to a single view;
                // the scheduler's Task-6 dispatch leaves `ctx.tree == None`
                // for these. Nothing to hand back.
                None
            }
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
            PanelScope::SubView {
                window_id: _,
                outer_panel_id,
            } => {
                // Phase 2 Task 5: sub-view resolution.
                //
                // `outer_panel_id` is the outer-tree panel id of an
                // `emSubViewPanel`. We search the outer-most reachable
                // `PanelTree` (via `ctx.tree`) for that panel, then reach
                // its `sub_view` through the typed
                // `as_sub_view_panel_mut` accessor (no `Any` /
                // `downcast_mut` — spec rule).
                //
                // Phase 3.5.A Task 5: `window_id` is stored but not yet
                // consumed — at this task the scheduler still hands a
                // single outer App tree via `ctx.tree: &mut PanelTree`.
                // Task 6 migrates to `Option<&mut PanelTree>` + a
                // scope-directed walk that uses `window_id` to pick the
                // right tree first.
                //
                // Borrow shape: the search takes `&mut ctx.tree`, the
                // `SchedCtx` we build is disjoint (only scheduler /
                // framework_actions / root_context / engine_id). This
                // mirrors the `Toplevel` branch pattern.
                let engine_id = ctx.engine_id;
                let sched_ptr: *mut crate::emScheduler::EngineScheduler = &mut *ctx.scheduler;
                let fw_ptr: *mut Vec<crate::emEngineCtx::DeferredAction> =
                    &mut *ctx.framework_actions;
                let svp_opt: Option<&mut crate::emSubViewPanel::emSubViewPanel> = ctx
                    .tree
                    .panels
                    .get_mut(outer_panel_id)
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
        let _ = PanelScope::Framework;
        let _ = PanelScope::Toplevel(WindowId::dummy());
        let _ = PanelScope::SubView {
            window_id: WindowId::dummy(),
            outer_panel_id: PanelId::null(),
        };
    }

    #[test]
    fn window_id_extraction() {
        let wid = WindowId::dummy();
        assert_eq!(PanelScope::Framework.window_id(), None);
        assert_eq!(PanelScope::Toplevel(wid).window_id(), Some(wid));
        assert_eq!(
            PanelScope::SubView {
                window_id: wid,
                outer_panel_id: PanelId::null()
            }
            .window_id(),
            Some(wid)
        );
    }
}
