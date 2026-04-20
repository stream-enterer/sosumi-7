// SPLIT: Split from emPanel.h — panel context types extracted
use super::emPanelTree::{PanelId, PanelTree};
use crate::emColor::emColor;
use crate::emPanel::PanelBehavior;
use crate::emPanel::Rect;

/// Panel context — provides a scoped API for a panel to interact with the tree.
///
/// The pattern is: extract behavior from tree, create PanelCtx, call behavior
/// methods passing ctx, then put behavior back. This avoids borrow conflicts.
pub struct PanelCtx<'a> {
    pub tree: &'a mut PanelTree,
    pub id: PanelId,
    /// Current pixel tallness (height/width ratio of a single pixel) of the
    /// owning view. Mirrors `emView::CurrentPixelTallness`. Passed at ctor
    /// time so layout / viewed-coord computations can use it without needing
    /// a `View&` reference (C++ `emPanel::Layout` reads it via `View&`).
    pub current_pixel_tallness: f64,
}

impl<'a> PanelCtx<'a> {
    /// Create a context for the given panel.
    pub fn new(tree: &'a mut PanelTree, id: PanelId, current_pixel_tallness: f64) -> Self {
        Self {
            tree,
            id,
            current_pixel_tallness,
        }
    }

    /// Wake this panel's scheduler engine.
    pub fn wake_up(&mut self) {
        self.wake_up_panel(self.id);
    }

    /// Wake another panel's scheduler engine.
    pub fn wake_up_panel(&mut self, id: crate::emPanelTree::PanelId) {
        let Some(panel) = self.tree.GetRec(id) else {
            return;
        };
        let Some(eid) = panel.engine_id else {
            return;
        };
        let Some(view_rc) = panel.View.upgrade() else {
            return;
        };
        // Reach the scheduler via an immutable view borrow.  This succeeds
        // from inside `PanelBehavior::Cycle` (the view is not borrowed there).
        // If the view is already mutably borrowed (rare — PanelCtx constructed
        // directly inside `emView::Update`) the wakeup is silently skipped;
        // the update engine is already running in that case so the panel will
        // be visited on the next notice cycle anyway.
        // C++ equivalent: panel->GetView().UpdateEngine->WakeUp().
        let sched_rc_opt: Option<std::rc::Rc<std::cell::RefCell<crate::emScheduler::EngineScheduler>>> =
            view_rc.try_borrow().ok().and_then(|v| v.scheduler.clone());
        // view_rc borrow is released above (temporary dropped before this line)
        if let Some(sched_rc) = sched_rc_opt {
            match sched_rc.try_borrow_mut() {
                Ok(mut sched) => sched.wake_up(eid),
                Err(_) => {
                    // Scheduler is borrowed by DoTimeSlice. Queue for deferred
                    // wakeup at the start of the next time slice.
                    self.tree.pending_engine_wakeups.push(eid);
                }
            }
        }
    }

    /// Returns true if this panel is the view's current seek target.
    pub fn is_seek_target(&self) -> bool {
        self.tree.is_seek_target(self.id)
    }

    /// Returns the child name being sought from this panel, or "" if not
    /// a seek target. Port of C++ `emPanel::GetSoughtName()`.
    pub fn seek_child_name(&self) -> &str {
        self.tree.sought_name(self.id).unwrap_or("")
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
        if self.tree.GetParentContext(child) == Some(self.id) {
            self.tree.remove(child);
        }
    }

    /// Delete the current panel (removes self from tree).
    pub fn delete_self(self) {
        self.tree.remove(self.id);
    }

    /// Set layout rect for a child panel.
    pub fn layout_child(&mut self, child: PanelId, x: f64, y: f64, w: f64, h: f64) {
        self.tree
            .Layout(child, x, y, w, h, self.current_pixel_tallness);
    }

    /// Set layout rect and canvas color for a child panel.
    ///
    /// C++ equivalent: `child->Layout(x, y, w, h, canvasColor)`.
    /// The canvas_color tells the child what background color it's being
    /// painted on top of, which is needed for correct canvas-color compositing.
    pub fn layout_child_canvas(
        &mut self,
        child: PanelId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        canvas_color: emColor,
    ) {
        self.tree
            .Layout(child, x, y, w, h, self.current_pixel_tallness);
        self.tree.SetCanvasColor(child, canvas_color);
    }

    /// Get the parent panel ID.
    pub fn GetParentContext(&self) -> Option<PanelId> {
        self.tree.GetParentContext(self.id)
    }

    /// Iterate over children of the current panel.
    pub fn children(&self) -> Vec<PanelId> {
        self.tree.children(self.id).collect()
    }

    /// Get the name of the current panel.
    pub fn name(&self) -> &str {
        self.tree
            .GetRec(self.id)
            .map(|p| p.name.as_str())
            .unwrap_or("")
    }

    /// Get the layout rect of the current panel in its own coordinate space.
    ///
    /// C++ emPanel behaviors operate in a normalized space where the panel's
    /// own width is always 1.0 and height = LayoutHeight / LayoutWidth
    /// (tallness). All child positions set via `layout_child` must be in
    /// this normalized space.
    pub fn layout_rect(&self) -> Rect {
        self.tree
            .GetRec(self.id)
            .map(|p| {
                let tallness = if p.layout_rect.w > 1e-100 {
                    p.layout_rect.h / p.layout_rect.w
                } else {
                    1.0
                };
                Rect::new(0.0, 0.0, 1.0, tallness)
            })
            .unwrap_or_default()
    }

    /// Set the canvas color.
    pub fn SetCanvasColor(&mut self, color: emColor) {
        self.tree.SetCanvasColor(self.id, color);
    }

    /// Get whether the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.tree
            .GetRec(self.id)
            .map(|p| p.visible)
            .unwrap_or(false)
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
    pub fn DeleteAllChildren(&mut self) {
        self.tree.DeleteAllChildren(self.id);
    }

    /// Find a child by name.
    pub fn find_child_by_name(&self, name: &str) -> Option<PanelId> {
        self.tree.find_child_by_name(self.id, name)
    }

    /// Request view navigation to a child panel.
    pub fn request_visit(&mut self, child: PanelId) {
        self.tree.request_visit(child);
    }

    /// Get the canvas color of the current panel.
    pub fn GetCanvasColor(&self) -> emColor {
        self.tree
            .GetRec(self.id)
            .map(|p| p.canvas_color)
            .unwrap_or(emColor::TRANSPARENT)
    }

    /// Get whether the panel is enabled.
    pub fn is_enabled(&self) -> bool {
        self.tree
            .GetRec(self.id)
            .map(|p| p.enabled)
            .unwrap_or(false)
    }

    /// Set the enable switch for the current panel.
    pub fn SetEnableSwitch(&mut self, enable: bool) {
        self.tree.SetEnableSwitch(self.id, enable);
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.tree.child_count(self.id)
    }

    /// Set canvas color on a child panel.
    ///
    /// C++ equivalent: the canvasColor argument of `child->Layout()`.
    pub fn set_child_canvas_color(&mut self, child: PanelId, color: emColor) {
        self.tree.SetCanvasColor(child, color);
    }

    /// Set canvas color on all children of the current panel.
    ///
    /// Used after layout_children to propagate the content area's background
    /// color to all child panels, matching C++ LayoutChildren behavior.
    pub fn set_all_children_canvas_color(&mut self, color: emColor) {
        let children: Vec<PanelId> = self.tree.children(self.id).collect();
        for child in children {
            self.tree.SetCanvasColor(child, color);
        }
    }

    /// Port of C++ `emPanel::PanelToViewX(x)`.
    /// Maps panel-space x-coordinate to view (screen) space.
    pub fn panel_to_view_x(&self, x: f64) -> f64 {
        self.tree.PanelToViewX(self.id, x)
    }

    /// Port of C++ `emPanel::PanelToViewY(y)`.
    /// Maps panel-space y-coordinate to view (screen) space.
    /// C++: ViewedY + y * ViewedWidth / CurrentPixelTallness.
    pub fn panel_to_view_y(&self, y: f64) -> f64 {
        if let Some(p) = self.tree.GetRec(self.id) {
            p.viewed_y + y * p.viewed_width / self.current_pixel_tallness
        } else {
            0.0
        }
    }

    /// Port of C++ `emPanel::GetClipX1/X2/Y1/Y2`.
    /// Returns the panel's clip rect in view (screen) space.
    pub fn clip_rect(&self) -> (f64, f64, f64, f64) {
        if let Some(p) = self.tree.GetRec(self.id) {
            (p.clip_x, p.clip_y, p.clip_x + p.clip_w, p.clip_y + p.clip_h)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Make child the first child in sibling order. Port of C++ `BeFirst()`.
    pub fn be_first_child(&mut self, child: PanelId) {
        self.tree.BeFirst(child);
    }

    /// Check if child panel is in the active path.
    pub fn child_in_active_path(&self, child: PanelId) -> bool {
        self.tree
            .GetRec(child)
            .map(|p| p.in_active_path)
            .unwrap_or(false)
    }

    /// Check if child panel is in the viewed path.
    pub fn child_in_viewed_path(&self, child: PanelId) -> bool {
        self.tree
            .GetRec(child)
            .map(|p| p.in_viewed_path)
            .unwrap_or(false)
    }
}
