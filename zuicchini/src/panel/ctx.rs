use super::behavior::PanelBehavior;
use super::tree::{PanelId, PanelTree};
use crate::foundation::{Color, Rect};

/// Panel context — provides a scoped API for a panel to interact with the tree.
///
/// The pattern is: extract behavior from tree, create PanelCtx, call behavior
/// methods passing ctx, then put behavior back. This avoids borrow conflicts.
pub struct PanelCtx<'a> {
    pub tree: &'a mut PanelTree,
    pub id: PanelId,
}

impl<'a> PanelCtx<'a> {
    /// Create a context for the given panel.
    pub fn new(tree: &'a mut PanelTree, id: PanelId) -> Self {
        Self { tree, id }
    }

    /// Create a child panel under the current panel.
    pub fn create_child(&mut self, name: &str) -> PanelId {
        self.tree.create_child(self.id, name)
    }

    /// Create a child with a behavior.
    pub fn create_child_with(&mut self, name: &str, behavior: Box<dyn PanelBehavior>) -> PanelId {
        let child_id = self.tree.create_child(self.id, name);
        self.tree.set_behavior(child_id, behavior);
        child_id
    }

    /// Remove a child panel.
    pub fn delete_child(&mut self, child: PanelId) {
        // Verify it's actually a child
        if self.tree.parent(child) == Some(self.id) {
            self.tree.remove(child);
        }
    }

    /// Delete the current panel (removes self from tree).
    pub fn delete_self(self) {
        self.tree.remove(self.id);
    }

    /// Set layout rect for a child panel.
    pub fn layout_child(&mut self, child: PanelId, x: f64, y: f64, w: f64, h: f64) {
        self.tree.set_layout_rect(child, x, y, w, h);
    }

    /// Get the parent panel ID.
    pub fn parent(&self) -> Option<PanelId> {
        self.tree.parent(self.id)
    }

    /// Iterate over children of the current panel.
    pub fn children(&self) -> Vec<PanelId> {
        self.tree.children(self.id).collect()
    }

    /// Get the name of the current panel.
    pub fn name(&self) -> &str {
        self.tree
            .get(self.id)
            .map(|p| p.name.as_str())
            .unwrap_or("")
    }

    /// Get the layout rect of the current panel.
    pub fn layout_rect(&self) -> Rect {
        self.tree
            .get(self.id)
            .map(|p| p.layout_rect)
            .unwrap_or_default()
    }

    /// Set the canvas color.
    pub fn set_canvas_color(&mut self, color: Color) {
        self.tree.set_canvas_color(self.id, color);
    }

    /// Get whether the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.tree.get(self.id).map(|p| p.visible).unwrap_or(false)
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        if let Some(panel) = self.tree.get_mut(self.id) {
            panel.visible = visible;
        }
    }

    /// Set whether the panel can receive focus.
    pub fn set_focusable(&mut self, focusable: bool) {
        if let Some(panel) = self.tree.get_mut(self.id) {
            panel.focusable = focusable;
        }
    }

    /// Get the preferred size of a child by extracting its behavior, calling
    /// `preferred_size()`, and putting the behavior back.
    pub fn child_preferred_size(&mut self, child: PanelId) -> (f64, f64) {
        if let Some(behavior) = self.tree.take_behavior(child) {
            let size = behavior.preferred_size();
            self.tree.put_behavior(child, behavior);
            size
        } else {
            (0.0, 0.0)
        }
    }

    /// Get the minimum size of a child by extracting its behavior, calling
    /// `min_size()`, and putting the behavior back.
    pub fn child_min_size(&mut self, child: PanelId) -> (f64, f64) {
        if let Some(behavior) = self.tree.take_behavior(child) {
            let size = behavior.min_size();
            self.tree.put_behavior(child, behavior);
            size
        } else {
            (0.0, 0.0)
        }
    }

    /// Remove all children of the current panel.
    pub fn delete_all_children(&mut self) {
        self.tree.delete_all_children(self.id);
    }

    /// Find a child by name.
    pub fn find_child_by_name(&self, name: &str) -> Option<PanelId> {
        self.tree.find_child_by_name(self.id, name)
    }

    /// Get the canvas color of the current panel.
    pub fn canvas_color(&self) -> Color {
        self.tree
            .get(self.id)
            .map(|p| p.canvas_color)
            .unwrap_or(Color::TRANSPARENT)
    }

    /// Get whether the panel is enabled.
    pub fn is_enabled(&self) -> bool {
        self.tree.get(self.id).map(|p| p.enabled).unwrap_or(false)
    }

    /// Set the enable switch for the current panel.
    pub fn set_enable_switch(&mut self, enable: bool) {
        self.tree.set_enable_switch(self.id, enable);
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.tree.child_count(self.id)
    }
}
