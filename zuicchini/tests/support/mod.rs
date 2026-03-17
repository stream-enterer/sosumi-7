#![allow(dead_code, clippy::type_complexity)]

use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::input::{InputEvent, InputKey, InputState, InputVariant};
use zuicchini::panel::{
    KeyboardZoomScrollVIF, MouseZoomScrollVIF, NoticeFlags, PanelBehavior, PanelCtx, PanelId,
    PanelState, PanelTree, View, ViewInputFilter,
};
use zuicchini::scheduler::EngineScheduler;

/// Headless test harness that wires together PanelTree, EngineScheduler, and View
/// without needing wgpu/winit.
pub struct TestHarness {
    pub tree: PanelTree,
    pub scheduler: EngineScheduler,
    pub view: View,
    pub vif_chain: Vec<Box<dyn ViewInputFilter>>,
    pub input_state: InputState,
    root: PanelId,
}

impl TestHarness {
    /// Create a harness with root panel (focusable, layout 0,0,1,1), 800x600 view.
    pub fn new() -> Self {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);

        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        let vif_chain: Vec<Box<dyn ViewInputFilter>> = vec![
            {
                let mut mouse_vif = MouseZoomScrollVIF::new();
                let zflpp = view.get_zoom_factor_log_per_pixel();
                mouse_vif.set_mouse_anim_params(1.0, 0.25, zflpp);
                mouse_vif.set_wheel_anim_params(1.0, 0.25, zflpp);
                Box::new(mouse_vif)
            },
            Box::new(KeyboardZoomScrollVIF::new()),
        ];

        Self {
            tree,
            scheduler: EngineScheduler::new(),
            view,
            vif_chain,
            input_state: InputState::new(),
            root,
        }
    }

    pub fn root(&self) -> PanelId {
        self.root
    }

    /// Run one frame: scheduler time slice → deliver notices → update viewing.
    pub fn tick(&mut self) {
        self.scheduler.do_time_slice();
        self.tree
            .deliver_notices(self.view.window_focused(), self.view.pixel_tallness());
        self.view.update_viewing(&mut self.tree);
    }

    /// Run n frames.
    pub fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.tick();
        }
    }

    /// Create a focusable child panel with a layout rect.
    pub fn add_panel(&mut self, parent: PanelId, name: &str) -> PanelId {
        let id = self.tree.create_child(parent, name);
        self.tree.set_focusable(id, true);
        self.tree.set_layout_rect(id, 0.0, 0.0, 1.0, 1.0);
        id
    }

    /// Create a focusable child panel with a layout rect and behavior.
    pub fn add_panel_with(
        &mut self,
        parent: PanelId,
        name: &str,
        behavior: Box<dyn PanelBehavior>,
    ) -> PanelId {
        let id = self.add_panel(parent, name);
        self.tree.set_behavior(id, behavior);
        id
    }

    /// Dispatch input through VIF chain → hit-test → behavior delivery.
    /// Matches C++ emPanel::Input which broadcasts to ALL viewed panels in
    /// depth-first order (root → leaves).
    pub fn inject_input(&mut self, event: &InputEvent) {
        // Run VIF chain
        for vif in &mut self.vif_chain {
            if vif.filter(event, &self.input_state, &mut self.view) {
                return;
            }
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
                .get_focusable_panel_at(&self.tree, event.mouse_x, event.mouse_y)
                .unwrap_or_else(|| self.view.root());
            self.view.set_active_panel(&mut self.tree, panel, false);
        }

        // Stamp modifier keys from InputState onto the event
        let ev = event.clone().with_modifiers(&self.input_state);

        // Dispatch to ALL viewed panels in DFS order (matching C++ emPanel::Input
        // recursive broadcast). Each panel's behavior receives the event;
        // if any returns true (consumed), propagation stops.
        let wf = self.view.window_focused();
        let viewed = self.tree.viewed_panels_dfs();
        for panel_id in viewed {
            if let Some(mut behavior) = self.tree.take_behavior(panel_id) {
                let state = self
                    .tree
                    .build_panel_state(panel_id, wf, self.view.pixel_tallness());
                let consumed = behavior.input(&ev, &state, &self.input_state);
                self.tree.put_behavior(panel_id, behavior);
                if consumed {
                    break;
                }
            }
        }
    }
}

/// A behavior that records calls via shared log. Optional closures for custom actions.
pub struct RecordingBehavior {
    pub log: Rc<RefCell<Vec<String>>>,
    pub on_input: Option<Box<dyn FnMut(&InputEvent) -> bool>>,
    pub on_layout: Option<Box<dyn FnMut(&mut PanelCtx)>>,
}

impl RecordingBehavior {
    pub fn new(log: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            log,
            on_input: None,
            on_layout: None,
        }
    }
}

impl PanelBehavior for RecordingBehavior {
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        self.log.borrow_mut().push(format!("notice:{flags:?}"));
    }

    fn input(
        &mut self,
        event: &InputEvent,
        _state: &PanelState,
        _input_state: &InputState,
    ) -> bool {
        self.log
            .borrow_mut()
            .push(format!("input:{:?}:{:?}", event.key, event.variant));
        if let Some(ref mut f) = self.on_input {
            f(event)
        } else {
            false
        }
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        self.log.borrow_mut().push("layout_children".to_string());
        if let Some(ref mut f) = self.on_layout {
            f(ctx);
        }
    }
}

/// A behavior that accumulates notice flags into a shared bitfield.
pub struct NoticeBehavior {
    pub accumulated: Rc<RefCell<NoticeFlags>>,
}

impl NoticeBehavior {
    pub fn new(accumulated: Rc<RefCell<NoticeFlags>>) -> Self {
        Self { accumulated }
    }
}

impl PanelBehavior for NoticeBehavior {
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        self.accumulated.borrow_mut().insert(flags);
    }
}

/// A behavior that tracks whether input was received.
pub struct InputTrackingBehavior {
    pub input_received: Rc<RefCell<bool>>,
}

impl InputTrackingBehavior {
    pub fn new(input_received: Rc<RefCell<bool>>) -> Self {
        Self { input_received }
    }
}

impl PanelBehavior for InputTrackingBehavior {
    fn input(
        &mut self,
        _event: &InputEvent,
        _state: &PanelState,
        _input_state: &InputState,
    ) -> bool {
        *self.input_received.borrow_mut() = true;
        false // don't consume — let default behavior handle activation
    }
}

/// Behavior that calls closures on notice/layout_children for tree mutation tests.
pub struct MutatingBehavior {
    pub on_layout: Option<Box<dyn FnMut(&mut PanelCtx)>>,
    pub on_notice: Option<Box<dyn FnMut(NoticeFlags)>>,
}

impl MutatingBehavior {
    pub fn new() -> Self {
        Self {
            on_layout: None,
            on_notice: None,
        }
    }
}

impl PanelBehavior for MutatingBehavior {
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        if let Some(ref mut f) = self.on_notice {
            f(flags);
        }
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if let Some(ref mut f) = self.on_layout {
            f(ctx);
        }
    }
}
