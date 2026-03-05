use bitflags::bitflags;

use super::tree::{PanelId, PanelTree};
use crate::foundation::Color;
use crate::render::Painter;

bitflags! {
    /// Flags controlling view behavior.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct ViewFlags: u32 {
        /// Zoom popup (zoom into a child panel).
        const POPUP_ZOOM     = 0b0000_0001;
        /// Disable zooming.
        const NO_ZOOM        = 0b0000_0010;
        /// Disable scrolling.
        const NO_SCROLL      = 0b0000_0100;
        /// Disable all navigation.
        const NO_NAVIGATE    = 0b0000_1000;
        /// Show the view fullscreen (no borders).
        const FULLSCREEN     = 0b0001_0000;
    }
}

/// State for a visited panel in the view hierarchy.
#[derive(Clone, Debug)]
pub struct VisitState {
    /// The panel being visited.
    pub panel: PanelId,
    /// Relative X position of the view within the panel.
    pub rel_x: f64,
    /// Relative Y position.
    pub rel_y: f64,
    /// Relative zoom/area factor.
    pub rel_a: f64,
}

/// The View manages the viewport — which panels are visible and how they're
/// navigated and rendered.
pub struct View {
    /// The root panel of this view.
    root: PanelId,
    /// The currently active (focused) panel.
    active: Option<PanelId>,
    /// The focused panel (for input).
    focused: Option<PanelId>,
    /// Current visit state stack (the zoom path from root to current view).
    visit_stack: Vec<VisitState>,
    /// View flags.
    pub flags: ViewFlags,
    /// Viewport dimensions.
    viewport_width: f64,
    viewport_height: f64,
}

impl View {
    /// Create a new view rooted at the given panel.
    pub fn new(root: PanelId, viewport_width: f64, viewport_height: f64) -> Self {
        let initial_visit = VisitState {
            panel: root,
            rel_x: 0.0,
            rel_y: 0.0,
            rel_a: 1.0,
        };
        Self {
            root,
            active: Some(root),
            focused: None,
            visit_stack: vec![initial_visit],
            flags: ViewFlags::empty(),
            viewport_width,
            viewport_height,
        }
    }

    /// Get the root panel.
    pub fn root(&self) -> PanelId {
        self.root
    }

    /// Get the currently active panel.
    pub fn active(&self) -> Option<PanelId> {
        self.active
    }

    /// Set the active panel.
    pub fn set_active(&mut self, id: PanelId) {
        self.active = Some(id);
    }

    /// Get the focused panel.
    pub fn focused(&self) -> Option<PanelId> {
        self.focused
    }

    /// Set focus to a panel.
    pub fn set_focus(&mut self, id: Option<PanelId>) {
        self.focused = id;
    }

    /// Get the current visit state (top of stack).
    pub fn current_visit(&self) -> &VisitState {
        self.visit_stack
            .last()
            .expect("Visit stack should never be empty")
    }

    /// Get the full visit stack.
    pub fn visit_stack(&self) -> &[VisitState] {
        &self.visit_stack
    }

    /// Visit a panel — push it onto the visit stack.
    pub fn visit(&mut self, panel: PanelId, rel_x: f64, rel_y: f64, rel_a: f64) {
        self.visit_stack.push(VisitState {
            panel,
            rel_x,
            rel_y,
            rel_a,
        });
        self.active = Some(panel);
    }

    /// Visit a panel fullsized (rel_a = 1.0, centered).
    pub fn visit_fullsized(&mut self, panel: PanelId) {
        self.visit(panel, 0.0, 0.0, 1.0);
    }

    /// Go back one level in the visit stack.
    pub fn go_back(&mut self) -> bool {
        if self.visit_stack.len() > 1 {
            self.visit_stack.pop();
            self.active = Some(self.current_visit().panel);
            true
        } else {
            false
        }
    }

    /// Go back to root.
    pub fn go_home(&mut self) {
        self.visit_stack.truncate(1);
        self.active = Some(self.root);
    }

    /// Zoom by a factor around the given viewport point.
    pub fn zoom(&mut self, factor: f64, _center_x: f64, _center_y: f64) {
        if self.flags.contains(ViewFlags::NO_ZOOM) {
            return;
        }
        if let Some(state) = self.visit_stack.last_mut() {
            state.rel_a *= factor;
            state.rel_a = state.rel_a.clamp(0.001, 1000.0);
        }
    }

    /// Scroll by the given delta in viewport coordinates.
    pub fn scroll(&mut self, dx: f64, dy: f64) {
        if self.flags.contains(ViewFlags::NO_SCROLL) {
            return;
        }
        if let Some(state) = self.visit_stack.last_mut() {
            state.rel_x += dx;
            state.rel_y += dy;
        }
    }

    /// Set viewport dimensions.
    pub fn set_viewport(&mut self, width: f64, height: f64) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    /// Get viewport dimensions.
    pub fn viewport_size(&self) -> (f64, f64) {
        (self.viewport_width, self.viewport_height)
    }

    /// Determine the supreme viewed panel (the topmost panel that fills the viewport).
    pub fn supreme_panel(&self) -> PanelId {
        self.current_visit().panel
    }

    /// Paint the view by recursively painting visible panels.
    pub fn paint(&self, tree: &mut PanelTree, painter: &mut Painter) {
        let root = self.supreme_panel();
        self.paint_panel_recursive(tree, painter, root, 0.0, 0.0);
    }

    fn paint_panel_recursive(
        &self,
        tree: &mut PanelTree,
        painter: &mut Painter,
        id: PanelId,
        offset_x: f64,
        offset_y: f64,
    ) {
        let (x, y, w, h) = match tree.get(id) {
            Some(panel) if panel.visible => panel.layout_rect,
            _ => return,
        };

        let canvas_color = tree
            .get(id)
            .map(|p| p.canvas_color)
            .unwrap_or(Color::TRANSPARENT);

        painter.push_state();
        painter.translate(offset_x + x, offset_y + y);
        painter.set_canvas_color(canvas_color);

        // Paint the panel's own content
        if let Some(mut behavior) = tree.take_behavior(id) {
            behavior.paint(painter, w, h);
            tree.put_behavior(id, behavior);
        }

        // Paint children
        let children: Vec<PanelId> = tree.children(id).collect();
        for child in children {
            self.paint_panel_recursive(tree, painter, child, 0.0, 0.0);
        }

        painter.pop_state();
    }
}
