#![allow(dead_code, clippy::type_complexity)]

pub mod pipeline;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emcore::emInput::{emInputEvent, InputKey, InputVariant};
use emcore::emInputState::emInputState;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};

use emcore::emPanelCtx::PanelCtx;

use emcore::emPanelTree::{PanelId, PanelTree};

use emcore::emView::emView;

use emcore::emScheduler::EngineScheduler;
use emcore::emViewInputFilter::{
    emDefaultTouchVIF, emKeyboardZoomScrollVIF, emMouseZoomScrollVIF, emViewInputFilter,
};
use emcore::emWindow::emWindow;
use winit::window::WindowId;

/// Headless test harness that wires together PanelTree, EngineScheduler, and emView
/// without needing wgpu/winit.
pub struct TestHarness {
    pub tree: PanelTree,
    pub scheduler: EngineScheduler,
    pub view: emView,
    pub vif_chain: Vec<Box<dyn emViewInputFilter>>,
    pub touch_vif: emDefaultTouchVIF,
    pub input_state: emInputState,
    root: PanelId,
}

impl TestHarness {
    /// Create a harness with root panel (focusable, layout 0,0,1,1), 800x600 view.
    pub fn new() -> Self {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0);

        let mut view = emView::new_for_test(root, 800.0, 600.0);
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

    /// Run one frame: scheduler time slice → deliver notices → update viewing.
    pub fn tick(&mut self) {
        let mut windows: HashMap<WindowId, std::rc::Rc<std::cell::RefCell<emWindow>>> =
            HashMap::new();
        self.scheduler.DoTimeSlice(&mut self.tree, &mut windows);
        self.tree
            .HandleNotice(self.view.IsFocused(), self.view.GetCurrentPixelTallness());
        self.view.Update(&mut self.tree);
    }

    /// Run n frames.
    pub fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.tick();
        }
    }

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

    /// Dispatch Input through VIF chain → hit-test → behavior delivery.
    /// Matches C++ emPanel::Input which broadcasts to ALL viewed panels in
    /// post-order (children → parents, last → first).
    pub fn inject_input(&mut self, event: &emInputEvent) {
        // Run VIF chain
        for vif in &mut self.vif_chain {
            if vif.filter(event, &self.input_state, &mut self.view, &mut self.tree) {
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
                .GetFocusablePanelAt(&self.tree, event.mouse_x, event.mouse_y)
                .unwrap_or_else(|| self.view.GetRootPanel());
            self.view.set_active_panel(&mut self.tree, panel, false);
        }

        // Stamp modifier keys from emInputState onto the event
        let ev = event.clone().with_modifiers(&self.input_state);

        // Dispatch to ALL viewed panels in post-order (matching C++ emPanel::Input
        // recursive broadcast: children before parents, last-child first).
        // If any returns true (consumed), propagation stops.
        let wf = self.view.IsFocused();
        let viewed = self.tree.viewed_panels_dfs();
        for panel_id in viewed {
            if let Some(mut behavior) = self.tree.take_behavior(panel_id) {
                let state =
                    self.tree
                        .build_panel_state(panel_id, wf, self.view.GetCurrentPixelTallness());
                let consumed = behavior.Input(&ev, &state, &self.input_state);
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
    pub on_input: Option<Box<dyn FnMut(&emInputEvent) -> bool>>,
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
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        self.log.borrow_mut().push(format!("notice:{flags:?}"));
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
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

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
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
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        self.accumulated.borrow_mut().insert(flags);
    }
}

/// A behavior that tracks whether Input was received.
pub struct InputTrackingBehavior {
    pub input_received: Rc<RefCell<bool>>,
}

impl InputTrackingBehavior {
    pub fn new(input_received: Rc<RefCell<bool>>) -> Self {
        Self { input_received }
    }
}

impl PanelBehavior for InputTrackingBehavior {
    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        *self.input_received.borrow_mut() = true;
        false // don't consume — let default behavior handle activation
    }
}

/// Behavior that calls closures on notice/LayoutChildren for tree mutation tests.
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
    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        if let Some(ref mut f) = self.on_notice {
            f(flags);
        }
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if let Some(ref mut f) = self.on_layout {
            f(ctx);
        }
    }
}
