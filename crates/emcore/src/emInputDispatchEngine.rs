// RUST_ONLY: no C++ header counterpart. The C++ reference dispatches input
// events directly from the X11 event loop into emView::Input (synchronous,
// in-thread). winit's async-callback model requires bridging into emCore's
// cycle-driven scheduler. This engine is registered at top priority during
// framework init (`emGUIFramework::App::new`) and drains
// `App::pending_inputs` each slice, routing each `(WindowId, emInputEvent)`
// through `emWindow::dispatch_input` with the framework-owned `emInputState`
// and a ctx built from the enclosing DoTimeSlice call.
//
// Spec: §3.1, §4 D4.9 in
// `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md`.

use crate::emEngine::emEngine;
use crate::emEngineCtx::EngineCtx;

pub struct InputDispatchEngine;

impl emEngine for InputDispatchEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let events = ctx.take_pending_inputs();
        if events.is_empty() {
            // Nothing to do this slice. Stay asleep until the next winit
            // callback wakes us up via `wake_up(input_dispatch_engine_id)`.
            return false;
        }

        for (wid, event) in events {
            // Resolve the target window: top-level `windows` first, then
            // popup scan (same rule as `emGUIFramework::find_window_mut`).
            // Popups live inside `emView::PopupWindow` and receive winit
            // events addressed to their own WindowId; we must reach them
            // via the parent view.
            //
            // Disjoint-field borrow: `ctx.windows` is taken for the
            // lookup; the SchedCtx built below re-borrows `ctx.scheduler`,
            // `ctx.framework_actions`, and `ctx.root_context` which do
            // not alias `ctx.windows` or `ctx.tree` / `ctx.input_state`.
            let EngineCtx {
                scheduler,
                tree,
                windows,
                root_context,
                framework_actions,
                input_state,
                framework_clipboard,
                ..
            } = ctx;

            let win = if windows.contains_key(&wid) {
                windows.get_mut(&wid)
            } else {
                // Popup path: scan for a parent window whose view holds a
                // popup whose materialized WindowId matches.
                windows.values_mut().find_map(|w| {
                    let matches = w
                        .view()
                        .PopupWindow
                        .as_ref()
                        .and_then(|p| p.winit_window_if_materialized().map(|ww| ww.id() == wid))
                        .unwrap_or(false);
                    if matches {
                        w.view_mut().PopupWindow.as_deref_mut()
                    } else {
                        None
                    }
                })
            };

            let Some(win) = win else {
                // Window closed between enqueue and drain. Silently drop;
                // matches the pre-migration `if let Some(win) = ...` gate.
                continue;
            };

            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context,
                framework_clipboard,
                current_engine: None,
            };
            win.dispatch_input(tree, &event, input_state, &mut sc);
        }

        // Sleep until next winit callback wakes us.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngine::{Priority, TreeLocation};
    use crate::emEngineCtx::DeferredAction;
    use crate::emInput::{emInputEvent, InputKey};
    use crate::emInputState::emInputState;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::EngineScheduler;
    use std::collections::HashMap;

    #[test]
    fn input_dispatch_drains_pending_inputs() {
        let mut sched = EngineScheduler::new();
        let mut tree = PanelTree::new();
        let mut windows = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let framework_clipboard: std::cell::RefCell<
            Option<Box<dyn crate::emClipboard::emClipboard>>,
        > = std::cell::RefCell::new(None);

        let eid = sched.register_engine(
            Box::new(InputDispatchEngine),
            Priority::VeryHigh,
            TreeLocation::Outer,
        );

        // Seed an event for an unknown (dummy) window. The engine must
        // still drain the queue; the unknown WindowId causes a silent
        // drop (matches the pre-migration `find_window_mut.is_some()`
        // gate), but `pending_inputs` is emptied regardless.
        let dummy_wid = winit::window::WindowId::dummy();
        let event = emInputEvent::press(InputKey::Key('a'));
        pending_inputs.push((dummy_wid, event));
        sched.wake_up(eid);

        sched.DoTimeSlice(
            &mut tree,
            &mut windows,
            &root_context,
            &mut framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &framework_clipboard,
        );

        assert!(
            pending_inputs.is_empty(),
            "InputDispatchEngine must drain pending_inputs each slice"
        );

        sched.remove_engine(eid);
    }
}
