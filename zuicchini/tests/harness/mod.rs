#![allow(dead_code, clippy::type_complexity)]

use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::input::{InputEvent, InputKey, InputState, InputVariant};
use zuicchini::panel::{
    KeyboardZoomScrollVIF, MouseZoomScrollVIF, NoticeFlags, PanelBehavior, PanelCtx, PanelId,
    PanelTree, View, ViewInputFilter,
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
            Box::new(MouseZoomScrollVIF::new()),
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
        self.tree.deliver_notices();
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
    /// Replicates ZuiWindow::dispatch_input without needing a ZuiWindow.
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
            if let Some(hit) =
                self.view
                    .get_focusable_panel_at(&self.tree, event.mouse_x, event.mouse_y)
            {
                self.view.set_active_panel(&mut self.tree, hit, false);
            }
        }

        // Stamp modifier keys from InputState onto the event
        let ev = event.clone().with_modifiers(&self.input_state);

        // Dispatch to active panel's behavior
        if let Some(active) = self.view.active() {
            if let Some(mut behavior) = self.tree.take_behavior(active) {
                behavior.input(&ev);
                self.tree.put_behavior(active, behavior);
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
    fn notice(&mut self, flags: NoticeFlags) {
        self.log.borrow_mut().push(format!("notice:{flags:?}"));
    }

    fn input(&mut self, event: &InputEvent) -> bool {
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
    fn notice(&mut self, flags: NoticeFlags) {
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
