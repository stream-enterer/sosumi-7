use std::collections::HashMap;

use slotmap::{new_key_type, SlotMap};

use super::behavior::{NoticeFlags, PanelBehavior};
use super::ctx::PanelCtx;
use crate::foundation::{Color, Rect};

new_key_type! {
    /// Unique handle for a panel in the panel tree.
    pub struct PanelId;
}

/// Data stored for each panel in the arena.
pub struct PanelData {
    /// Parent panel (None for root).
    pub parent: Option<PanelId>,
    /// First child panel.
    pub first_child: Option<PanelId>,
    /// Last child panel.
    pub last_child: Option<PanelId>,
    /// Next sibling.
    pub next_sibling: Option<PanelId>,
    /// Previous sibling.
    pub prev_sibling: Option<PanelId>,
    /// Panel name (for lookup).
    pub name: String,
    /// Layout rectangle relative to parent.
    pub layout_rect: Rect,
    /// Canvas color for this panel.
    pub canvas_color: Color,
    /// Whether the panel is visible.
    pub visible: bool,
    /// Whether the panel can receive input focus.
    pub focusable: bool,
    /// Per-panel enable switch (ANDed with ancestors to compute enabled).
    pub enable_switch: bool,
    /// Computed: true if this panel and all ancestors have enable_switch=true.
    pub enabled: bool,
    /// Pending notice flags.
    pub pending_notices: NoticeFlags,
    /// The behavior implementation (extracted for mutation).
    pub behavior: Option<Box<dyn PanelBehavior>>,
}

impl PanelData {
    fn new(name: String) -> Self {
        Self {
            parent: None,
            first_child: None,
            last_child: None,
            next_sibling: None,
            prev_sibling: None,
            name,
            layout_rect: Rect::default(),
            canvas_color: Color::TRANSPARENT,
            visible: true,
            focusable: false,
            enable_switch: true,
            enabled: true,
            pending_notices: NoticeFlags::empty(),
            behavior: None,
        }
    }
}

/// Arena-based panel tree using SlotMap for stable handles.
pub struct PanelTree {
    panels: SlotMap<PanelId, PanelData>,
    root: Option<PanelId>,
    /// Per-parent name index: (parent, child_name) → child_id.
    /// Root panels use their own id as the "parent" key.
    name_index: HashMap<(PanelId, String), PanelId>,
}

impl PanelTree {
    pub fn new() -> Self {
        Self {
            panels: SlotMap::with_key(),
            root: None,
            name_index: HashMap::new(),
        }
    }

    /// Create the root panel.
    pub fn create_root(&mut self, name: &str) -> PanelId {
        let id = self.panels.insert(PanelData::new(name.to_string()));
        // Root uses its own id as the parent key
        self.name_index.insert((id, name.to_string()), id);
        self.root = Some(id);
        id
    }

    /// Create a child panel under the given parent.
    pub fn create_child(&mut self, parent: PanelId, name: &str) -> PanelId {
        let id = self.panels.insert(PanelData::new(name.to_string()));
        self.name_index.insert((parent, name.to_string()), id);

        // Link into parent's child list
        self.panels[id].parent = Some(parent);

        let prev_last = self.panels[parent].last_child;
        if let Some(prev) = prev_last {
            self.panels[prev].next_sibling = Some(id);
            self.panels[id].prev_sibling = Some(prev);
        } else {
            self.panels[parent].first_child = Some(id);
        }
        self.panels[parent].last_child = Some(id);

        // Notify parent
        self.panels[parent]
            .pending_notices
            .insert(NoticeFlags::CHILDREN_CHANGED);

        id
    }

    /// Remove a panel and all its descendants.
    pub fn remove(&mut self, id: PanelId) {
        // Collect all descendants first
        let descendants = self.collect_descendants(id);

        // Unlink from parent's child list
        if let Some(parent_id) = self.panels[id].parent {
            let prev = self.panels[id].prev_sibling;
            let next = self.panels[id].next_sibling;

            if let Some(prev_id) = prev {
                self.panels[prev_id].next_sibling = next;
            } else {
                self.panels[parent_id].first_child = next;
            }

            if let Some(next_id) = next {
                self.panels[next_id].prev_sibling = prev;
            } else {
                self.panels[parent_id].last_child = prev;
            }

            self.panels[parent_id]
                .pending_notices
                .insert(NoticeFlags::CHILDREN_CHANGED);
        }

        // Remove root reference if needed
        if self.root == Some(id) {
            self.root = None;
        }

        // Remove from arena and name index
        for desc_id in descendants {
            if let Some(data) = self.panels.remove(desc_id) {
                if let Some(parent_id) = data.parent {
                    self.name_index.remove(&(parent_id, data.name));
                }
            }
        }
        if let Some(data) = self.panels.remove(id) {
            if let Some(parent_id) = data.parent {
                self.name_index.remove(&(parent_id, data.name));
            } else {
                // Root panel uses itself as key
                self.name_index.remove(&(id, data.name));
            }
        }
    }

    /// Get the root panel ID.
    pub fn root(&self) -> Option<PanelId> {
        self.root
    }

    /// Get a panel's data.
    pub fn get(&self, id: PanelId) -> Option<&PanelData> {
        self.panels.get(id)
    }

    /// Get a panel's data mutably.
    pub fn get_mut(&mut self, id: PanelId) -> Option<&mut PanelData> {
        self.panels.get_mut(id)
    }

    /// Look up a child panel by parent and name.
    pub fn find_child_by_name(&self, parent: PanelId, name: &str) -> Option<PanelId> {
        self.name_index.get(&(parent, name.to_string())).copied()
    }

    /// Look up a panel by name (searches all panels).
    pub fn find_by_name(&self, name: &str) -> Option<PanelId> {
        self.panels
            .iter()
            .find(|(_, data)| data.name == name)
            .map(|(id, _)| id)
    }

    /// Check if a panel exists.
    pub fn contains(&self, id: PanelId) -> bool {
        self.panels.contains_key(id)
    }

    /// Get the total number of panels.
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    /// Check if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }

    /// Iterate over children of a panel.
    pub fn children(&self, parent: PanelId) -> ChildIter<'_> {
        let first = self.panels.get(parent).and_then(|p| p.first_child);
        ChildIter {
            tree: self,
            current: first,
        }
    }

    /// Get the number of children.
    pub fn child_count(&self, parent: PanelId) -> usize {
        self.children(parent).count()
    }

    /// Get the parent of a panel.
    pub fn parent(&self, id: PanelId) -> Option<PanelId> {
        self.panels.get(id).and_then(|p| p.parent)
    }

    /// Remove all children of a panel.
    pub fn delete_all_children(&mut self, parent: PanelId) {
        let children: Vec<PanelId> = self.children(parent).collect();
        for child in children {
            self.remove(child);
        }
    }

    /// Set the layout rectangle for a panel.
    pub fn set_layout_rect(&mut self, id: PanelId, x: f64, y: f64, w: f64, h: f64) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.layout_rect = Rect { x, y, w, h };
            panel.pending_notices.insert(NoticeFlags::LAYOUT_CHANGED);
        }
    }

    /// Set the canvas color for a panel.
    pub fn set_canvas_color(&mut self, id: PanelId, color: Color) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.canvas_color = color;
            panel.pending_notices.insert(NoticeFlags::CANVAS_CHANGED);
        }
    }

    /// Set the enable switch for a panel and recompute enabled state for descendants.
    pub fn set_enable_switch(&mut self, id: PanelId, enable: bool) {
        if let Some(panel) = self.panels.get_mut(id) {
            if panel.enable_switch == enable {
                return;
            }
            panel.enable_switch = enable;
        }
        self.recompute_enabled(id);
    }

    /// Recompute the `enabled` field for a panel and its descendants.
    fn recompute_enabled(&mut self, id: PanelId) {
        let parent_enabled = self
            .panels
            .get(id)
            .and_then(|p| p.parent)
            .and_then(|pid| self.panels.get(pid))
            .map(|p| p.enabled)
            .unwrap_or(true);

        if let Some(panel) = self.panels.get_mut(id) {
            let new_enabled = panel.enable_switch && parent_enabled;
            if panel.enabled != new_enabled {
                panel.enabled = new_enabled;
                panel.pending_notices.insert(NoticeFlags::ENABLE_CHANGED);
            }
        }

        // Recurse into children
        let child_ids: Vec<PanelId> = self.children(id).collect();
        for child_id in child_ids {
            self.recompute_enabled(child_id);
        }
    }

    /// Set the behavior for a panel.
    pub fn set_behavior(&mut self, id: PanelId, behavior: Box<dyn PanelBehavior>) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.behavior = Some(behavior);
        }
    }

    /// Extract the behavior from a panel (for calling methods that need &mut self on tree).
    pub fn take_behavior(&mut self, id: PanelId) -> Option<Box<dyn PanelBehavior>> {
        self.panels.get_mut(id).and_then(|p| p.behavior.take())
    }

    /// Put the behavior back after extraction.
    pub fn put_behavior(&mut self, id: PanelId, behavior: Box<dyn PanelBehavior>) {
        if let Some(panel) = self.panels.get_mut(id) {
            panel.behavior = Some(behavior);
        }
    }

    /// Deliver pending notices to all panels with behaviors.
    pub fn deliver_notices(&mut self) {
        let ids: Vec<PanelId> = self.panels.keys().collect();
        for id in ids {
            let flags = self.panels[id].pending_notices;
            if flags.is_empty() {
                continue;
            }
            self.panels[id].pending_notices = NoticeFlags::empty();
            if let Some(mut behavior) = self.take_behavior(id) {
                behavior.notice(flags);
                if flags.contains(NoticeFlags::LAYOUT_CHANGED) {
                    let mut ctx = PanelCtx::new(self, id);
                    behavior.layout_children(&mut ctx);
                }
                self.put_behavior(id, behavior);
            }
        }
    }

    fn collect_descendants(&self, id: PanelId) -> Vec<PanelId> {
        let mut result = Vec::new();
        let mut stack = Vec::new();
        if let Some(panel) = self.panels.get(id) {
            if let Some(child) = panel.first_child {
                stack.push(child);
            }
        }
        while let Some(current) = stack.pop() {
            result.push(current);
            if let Some(panel) = self.panels.get(current) {
                if let Some(child) = panel.first_child {
                    stack.push(child);
                }
                if let Some(next) = panel.next_sibling {
                    stack.push(next);
                }
            }
        }
        result
    }
}

impl Default for PanelTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over children of a panel.
pub struct ChildIter<'a> {
    tree: &'a PanelTree,
    current: Option<PanelId>,
}

impl<'a> Iterator for ChildIter<'a> {
    type Item = PanelId;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        self.current = self.tree.panels.get(id).and_then(|p| p.next_sibling);
        Some(id)
    }
}
