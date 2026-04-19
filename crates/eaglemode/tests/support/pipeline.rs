#![allow(dead_code)]

use std::collections::HashMap;

use emcore::emInput::{emInputEvent, InputKey, InputVariant};
use emcore::emInputState::emInputState;
use emcore::emPanel::PanelBehavior;

use emcore::emPanelTree::{PanelId, PanelTree};

use emcore::emView::emView;

use emcore::emScheduler::EngineScheduler;
use emcore::emViewInputFilter::{
    emDefaultTouchVIF, emKeyboardZoomScrollVIF, emMouseZoomScrollVIF, emViewInputFilter,
};
use emcore::emWindow::emWindow;
use winit::window::WindowId;

/// Test harness that dispatches Input through the FULL coordinate transform
/// pipeline (VIF chain, hit test, view_to_panel_x/y transform), matching
/// the production path in `emWindow::dispatch_input`.
///
/// Unlike `TestHarness` which passes view-space coordinates directly to
/// `behavior.Input()`, this harness transforms mouse coordinates from view
/// space to panel-local space before delivery — exactly as the real window
/// does.
pub struct PipelineTestHarness {
    pub tree: PanelTree,
    pub scheduler: EngineScheduler,
    pub view: emView,
    pub vif_chain: Vec<Box<dyn emViewInputFilter>>,
    pub touch_vif: emDefaultTouchVIF,
    pub input_state: emInputState,
    root: PanelId,
}

impl PipelineTestHarness {
    /// Create a harness with root panel (focusable, layout 0,0,1,1), 800x600 view.
    pub fn new() -> Self {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_focusable(root, true);
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        let mut view = emView::new(emcore::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.Update(&mut tree);

        let vif_chain: Vec<Box<dyn emViewInputFilter>> = vec![
            {
                let mut mouse_vif = emMouseZoomScrollVIF::new();
                let zflpp = view.GetZoomFactorLogarithmPerPixel();
                mouse_vif.set_mouse_anim_params(1.0, 0.25, zflpp);
                mouse_vif.set_wheel_anim_params(1.0, 0.25, zflpp);
                Box::new(mouse_vif)
            },
            Box::new(emKeyboardZoomScrollVIF::new()),
        ];

        Self {
            tree,
            scheduler: EngineScheduler::new(),
            view,
            vif_chain,
            touch_vif: emDefaultTouchVIF::new(),
            input_state: emInputState::new(),
            root,
        }
    }

    pub fn get_root_panel(&self) -> PanelId {
        self.root
    }

    // ── Frame / tick ─────────────────────────────────────────────

    /// Run one frame: scheduler time slice, deliver notices, update viewing.
    ///
    /// Also drives `VisitingVA` to completion: the pipeline harness has no
    /// window registry, so `VisitingVAEngineClass::Cycle` (which requires
    /// `ctx.windows`) cannot advance the animator — the harness pumps it
    /// directly to observe post-convergence active-panel state.
    pub fn tick(&mut self) {
        let mut windows: HashMap<WindowId, std::rc::Rc<std::cell::RefCell<emWindow>>> =
            HashMap::new();
        let __root_ctx = emcore::emContext::emContext::NewRoot();
        self.scheduler.DoTimeSlice(&mut self.tree, &mut windows, &__root_ctx);
        self.view.pump_visiting_va(&mut self.tree);
        self.view.HandleNotice(&mut self.tree);
        self.view.Update(&mut self.tree);
    }

    /// Run n frames.
    pub fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.tick();
        }
    }

    // ── Panel management ─────────────────────────────────────────

    /// Create a focusable child panel with a layout rect.
    pub fn add_panel(&mut self, parent_context: PanelId, name: &str) -> PanelId {
        let id = self.tree.create_child(parent_context, name);
        self.tree.set_focusable(id, true);
        self.tree.Layout(id, 0.0, 0.0, 1.0, 1.0, 1.0);
        id
    }

    /// Create a focusable child panel with a layout rect and behavior.
    pub fn add_panel_with(
        &mut self,
        parent_context: PanelId,
        name: &str,
        behavior: Box<dyn PanelBehavior>,
    ) -> PanelId {
        let id = self.add_panel(parent_context, name);
        self.tree.set_behavior(id, behavior);
        id
    }

    // ── Zoom control ─────────────────────────────────────────────

    /// Set the zoom level relative to the default "fit in viewport" (1x).
    ///
    /// At `level = 1.0` the root panel just fills the viewport (same as
    /// raw_zoom_out). At `level = 2.0` the root panel appears at 2x linear
    /// magnification (4x area).
    ///
    /// The zoom is centered on the viewport center so rel_x/rel_y stay at 0.
    pub fn set_zoom(&mut self, level: f64) {
        // Step 1: HardResetFileState to the 1x baseline (raw_zoom_out sets rel_a to
        // zoom_out_rel_a and calls update_viewing internally).
        self.view.RawZoomOut(&mut self.tree, false);

        // Step 2: apply the magnification factor. Zoom(factor) squares
        // internally (ra *= 1/factor^2), so passing `level` directly gives
        // level-x linear magnification.
        if (level - 1.0).abs() > 1e-12 {
            let (vw, vh) = self.view.viewport_size();
            self.view.Zoom(&mut self.tree, level, vw * 0.5, vh * 0.5);
        }

        // Step 3: refresh viewed Restore for all panels.
        self.view.Update(&mut self.tree);
    }

    // ── Auto-expansion ─────────────────────────────────────────

    /// Set zoom to trigger auto-expansion and run enough ticks for
    /// `LayoutChildren` to execute. `update_auto_expansion` runs inside
    /// `update_viewing`, so `set_zoom` already triggers it; the extra
    /// ticks let notices propagate and child panels GetRec laid out.
    pub fn expand_to(&mut self, zoom_level: f64) {
        self.set_zoom(zoom_level);
        // Several ticks to propagate notices and execute LayoutChildren
        self.tick_n(10);
    }

    /// Query whether a panel is currently auto-expanded.
    pub fn is_expanded(&self, panel_id: PanelId) -> bool {
        self.tree.IsAutoExpanded(panel_id)
    }

    // ── Input dispatch (full pipeline) ───────────────────────────

    /// Dispatch an Input event through the full coordinate transform pipeline,
    /// matching `emWindow::dispatch_input`:
    ///
    /// 1. VIF chain filter
    /// 2. Hit test and set active panel (for mouse press)
    /// 3. Transform mouse coords from view space to panel-local space
    /// 4. Keyboard suppression for non-active-path panels
    /// 5. Deliver to behavior
    pub fn dispatch(&mut self, event: &emInputEvent) {
        // Run VIF chain
        for vif in &mut self.vif_chain {
            if vif.filter(event, &self.input_state, &mut self.view, &mut self.tree) {
                return;
            }
        }

        // Tab / Shift+Tab focus cycling (C++ emPanel.cpp FocusNext/FocusPrev).
        // User-nav gate: nav methods don't gate internally; gate here.
        if event.key == InputKey::Tab && event.variant == InputVariant::Press {
            if !self
                .view
                .flags
                .contains(emcore::emView::ViewFlags::NO_USER_NAVIGATION)
            {
                if self.input_state.GetShift() {
                    self.view.VisitPrev(&mut self.tree);
                } else {
                    self.view.VisitNext(&mut self.tree);
                }
            }
            return;
        }

        // For mouse press: hit test and set active panel
        if event.variant == InputVariant::Press
            && matches!(
                event.key,
                InputKey::MouseLeft | InputKey::MouseRight | InputKey::MouseMiddle
            )
        {
            let panel = self
                .view
                .GetFocusablePanelAt(&self.tree, event.mouse_x, event.mouse_y)
                .unwrap_or_else(|| self.view.GetRootPanel());
            self.view.set_active_panel(&mut self.tree, panel, false);
        }

        // Stamp modifier keys from emInputState onto the event
        let ev = event.clone().with_modifiers(&self.input_state);

        // Dispatch to ALL viewed panels in post-order, transforming mouse
        // coordinates to panel-local space for each panel.
        let wf = self.view.IsFocused();
        let viewed = self.tree.viewed_panels_dfs();
        let mut consumed = false;
        for panel_id in viewed {
            // Transform view-space mouse coords to panel-local coords
            let mut panel_ev = ev.clone();
            panel_ev.mouse_x = self.tree.ViewToPanelX(panel_id, ev.mouse_x);
            panel_ev.mouse_y =
                self.tree
                    .ViewToPanelY(panel_id, ev.mouse_y, self.view.GetCurrentPixelTallness());

            if let Some(mut behavior) = self.tree.take_behavior(panel_id) {
                let panel_state =
                    self.tree
                        .build_panel_state(panel_id, wf, self.view.GetCurrentPixelTallness());

                // C++ RecurseInput: keyboard events are suppressed for
                // panels not in the active path.
                if panel_ev.is_keyboard_event() && !panel_state.in_active_path {
                    self.tree.put_behavior(panel_id, behavior);
                    continue;
                }

                consumed = behavior.Input(&panel_ev, &panel_state, &self.input_state);
                self.tree.put_behavior(panel_id, behavior);
                if consumed {
                    self.view.InvalidatePainting(&self.tree, panel_id);
                    break;
                }
            }
        }

        // Arrow key sibling navigation and Home/End/PageUp/PageDown.
        // (C++ emPanel.cpp:1168-1198 routes these via emPanel::Input fallback;
        // Rust routes via this post-behavior block for architectural consistency
        // with the existing arrow-key arrangement.)
        // Only fires if no behavior consumed the event.
        // User-nav gate: C++ `emView::Visit*` nav methods do not gate internally;
        // the user-nav caller gates on `NO_USER_NAVIGATION`.
        let user_nav_blocked = self
            .view
            .flags
            .contains(emcore::emView::ViewFlags::NO_USER_NAVIGATION);
        if !consumed && !user_nav_blocked && event.variant == InputVariant::Press {
            let st = &self.input_state;
            match event.key {
                InputKey::ArrowLeft if st.IsNoMod() => self.view.VisitLeft(&mut self.tree),
                InputKey::ArrowRight if st.IsNoMod() => self.view.VisitRight(&mut self.tree),
                InputKey::ArrowUp if st.IsNoMod() => self.view.VisitUp(&mut self.tree),
                InputKey::ArrowDown if st.IsNoMod() => self.view.VisitDown(&mut self.tree),

                // C++ emPanel.cpp:1168-1180: Home with modifier variants.
                InputKey::Home if st.IsNoMod() => self.view.VisitFirst(&mut self.tree),
                InputKey::Home if st.IsAltMod() => {
                    if let Some(p) = self.view.GetActivePanel() {
                        let adherent = self.view.IsActivationAdherent();
                        self.view.VisitFullsized(&self.tree, p, adherent, false);
                    }
                }
                InputKey::Home if st.IsShiftAltMod() => {
                    if let Some(p) = self.view.GetActivePanel() {
                        let adherent = self.view.IsActivationAdherent();
                        self.view.VisitFullsized(&self.tree, p, adherent, true);
                    }
                }

                // C++ emPanel.cpp:1182-1198
                InputKey::End if st.IsNoMod() => self.view.VisitLast(&mut self.tree),
                InputKey::PageUp if st.IsNoMod() => self.view.VisitOut(&mut self.tree),
                InputKey::PageDown if st.IsNoMod() => self.view.VisitIn(&mut self.tree),

                _ => {}
            }
        }
    }

    // ── High-level Input helpers ─────────────────────────────────

    /// Click (press + release) the left mouse button at view-space coordinates.
    pub fn click(&mut self, view_x: f64, view_y: f64) {
        let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(view_x, view_y);
        let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(view_x, view_y);
        self.dispatch(&press);
        self.dispatch(&release);
    }

    /// Drag: press at `from`, move to `to`, release at `to`. All coordinates
    /// are in view space.
    pub fn drag(&mut self, from_x: f64, from_y: f64, to_x: f64, to_y: f64) {
        let press = emInputEvent::press(InputKey::MouseLeft).with_mouse(from_x, from_y);
        let move_ev = emInputEvent::mouse_move(InputKey::MouseLeft, to_x, to_y);
        let release = emInputEvent::release(InputKey::MouseLeft).with_mouse(to_x, to_y);
        self.dispatch(&press);
        self.dispatch(&move_ev);
        self.dispatch(&release);
    }

    /// Press and release a keyboard key.
    pub fn press_key(&mut self, key: InputKey) {
        let press = emInputEvent::press(key);
        let release = emInputEvent::release(key);
        self.dispatch(&press);
        self.dispatch(&release);
    }

    /// Press and release a character key, including the `chars` field so that
    /// text-Input widgets (e.g. emTextField) receive the typed character.
    pub fn press_char(&mut self, ch: char) {
        let press = emInputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string());
        let release = emInputEvent::release(InputKey::Key(ch));
        self.dispatch(&press);
        self.dispatch(&release);
    }
}
