use crate::emCursor::emCursor;
use crate::emInput::emInputEvent;
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
use crate::emViewAnimator::emViewAnimator;

use super::emPanel::{NoticeFlags, PanelBehavior, PanelState, ParentInvalidation};
use super::emPanelTree::{PanelId, PanelTree};
use super::emView::{emView, ViewFlags};

/// A panel that embeds a sub-view within the parent view's panel tree.
///
/// This enables split-view or embedded-view functionality by maintaining a
/// separate [`emView`] and [`PanelTree`] that are rendered within the bounds of
/// this panel. Input is forwarded from the parent to the sub-view, and
/// painting is delegated to the sub-view's own render pipeline.
///
/// Corresponds to C++ `emSubViewPanel`.
pub struct emSubViewPanel {
    sub_tree: PanelTree,
    sub_view: emView,
    /// Cached viewed geometry from the parent panel (absolute viewport pixels).
    viewed_x: f64,
    viewed_y: f64,
    viewed_width: f64,
    viewed_height: f64,
    /// C++ emView has ActiveAnimator — an animator that drives zoom/scroll
    /// within this sub-view. Ticked during Paint alongside sub-tree lifecycle.
    pub active_animator: Option<Box<dyn emViewAnimator>>,
}

impl Default for emSubViewPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl emSubViewPanel {
    /// Create a new emSubViewPanel with an empty sub-view.
    ///
    /// The sub-tree is initialized with a root panel. Use [`sub_root`],
    /// [`sub_tree_mut`], and [`sub_view_mut`] to populate the sub-view.
    pub fn new() -> Self {
        let mut sub_tree = PanelTree::new();
        // C++ view root has an empty name; identity ":" decodes to [""].
        let root = sub_tree.create_root("");
        sub_tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let sub_view = emView::new(root, 1.0, 1.0);

        Self {
            sub_tree,
            sub_view,
            viewed_x: 0.0,
            viewed_y: 0.0,
            viewed_width: 1.0,
            viewed_height: 1.0,
            active_animator: None,
        }
    }

    /// Get the root panel ID of the sub-view's panel tree.
    pub fn sub_root(&self) -> PanelId {
        self.sub_tree
            .GetRootPanel()
            .expect("SubViewPanel sub-tree always has a root")
    }

    /// Get a reference to the sub-view's panel tree.
    pub fn sub_tree(&self) -> &PanelTree {
        &self.sub_tree
    }

    /// Get a mutable reference to the sub-view's panel tree.
    pub fn sub_tree_mut(&mut self) -> &mut PanelTree {
        &mut self.sub_tree
    }

    /// Get a reference to the sub-view.
    pub fn GetSubView(&self) -> &emView {
        &self.sub_view
    }

    /// Get a mutable reference to the sub-view.
    pub fn sub_view_mut(&mut self) -> &mut emView {
        &mut self.sub_view
    }

    /// Borrow sub_view and sub_tree mutably at the same time.
    /// Needed when a method on the view requires a mutable tree reference.
    pub fn view_and_tree_mut(&mut self) -> (&mut emView, &mut PanelTree) {
        (&mut self.sub_view, &mut self.sub_tree)
    }

    /// Visit a panel in the sub-view by identity string.
    ///
    /// This method borrows `sub_view` and `sub_tree` simultaneously, which
    /// requires access to `self` rather than separate `sub_view_mut()` and
    /// `sub_tree_mut()` calls (which would conflict on `&mut self`).
    pub fn visit_by_identity(
        &mut self,
        identity: &str,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
    ) {
        self.sub_view
            .VisitByIdentity(&mut self.sub_tree, identity, rel_x, rel_y, rel_a);
    }

    /// Set the view flags on the sub-view.
    pub fn set_sub_view_flags(&mut self, flags: ViewFlags) {
        self.sub_view.flags = flags;
    }

    /// Update the sub-view geometry to match the parent panel's viewed area.
    ///
    /// In C++ this delegates to `emViewPort::SetViewGeometry`. The sub-view's
    /// viewport size is set to match the parent panel's pixel dimensions.
    fn sync_geometry(&mut self, state: &PanelState) {
        if state.viewed {
            self.viewed_x = state.viewed_rect.x;
            self.viewed_y = state.viewed_rect.y;
            self.viewed_width = state.viewed_rect.w;
            self.viewed_height = state.viewed_rect.h;
            self.sub_view
                .SetGeometry(&mut self.sub_tree, self.viewed_width, self.viewed_height);
        } else {
            // Not viewed — give the sub-view a default geometry.
            // C++ uses (0, 0, 1, GetHeight(), 1.0).
            self.viewed_x = 0.0;
            self.viewed_y = 0.0;
            self.viewed_width = 1.0;
            self.viewed_height = state.height;
            self.sub_view
                .SetGeometry(&mut self.sub_tree, 1.0, state.height);
        }
    }
}

impl PanelBehavior for emSubViewPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // C++ emSubViewPanel::Input:
        //   if (IsFocusable() && (event.IsMouseEvent() || event.IsTouchEvent())) {
        //       Focus();
        //       SubViewPort->SetViewFocused(IsFocused());
        //   }
        //   SubViewPort->InputToView(event, state);
        //
        // Focus() on mouse/touch is already handled by the parent window's
        // dispatch loop (zui_window.rs hit-test + set_active_panel). We still
        // propagate focus state to the sub-view here, matching C++.
        if event.is_mouse_event() || event.is_touch_event() {
            self.sub_view
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }

        // Forward input to the sub-view's panels (C++ InputToView).
        // The event mouse coords are in panel-local space (x: 0..1, y: 0..h).
        // Convert to sub-view viewport pixel coords for the sub-tree dispatch.
        let sub_vx = event.mouse_x * self.viewed_width;
        let sub_vy = event.mouse_y * self.viewed_width / state.pixel_tallness;

        // Hit-test and set active panel on mouse press (mirrors parent window logic).
        if event.is_mouse_event()
            && event.variant == crate::emInput::InputVariant::Press
        {
            let panel = self
                .sub_view
                .GetFocusablePanelAt(&self.sub_tree, sub_vx, sub_vy)
                .unwrap_or_else(|| self.sub_view.GetRootPanel());
            self.sub_view
                .set_active_panel(&mut self.sub_tree, panel, false);
        }

        // Ensure sub-view viewing state is current for coordinate transforms.
        self.sub_view.Update(&mut self.sub_tree);

        // Dispatch to sub-tree panels (DFS order, matching C++ RecurseInput).
        let wf = self.sub_view.IsFocused();
        let viewed = self.sub_tree.viewed_panels_dfs();
        for panel_id in viewed {
            let mut panel_ev = event.clone();
            panel_ev.mouse_x = self.sub_tree.ViewToPanelX(panel_id, sub_vx);
            panel_ev.mouse_y = self.sub_tree.ViewToPanelY(
                panel_id,
                sub_vy,
                self.sub_view.GetCurrentPixelTallness(),
            );
            if let Some(mut behavior) = self.sub_tree.take_behavior(panel_id) {
                let panel_state = self
                    .sub_tree
                    .build_panel_state(panel_id, wf, self.sub_view.GetCurrentPixelTallness());
                // Suppress keyboard events for panels not in the active path.
                if panel_ev.is_keyboard_event() && !panel_state.in_active_path {
                    self.sub_tree.put_behavior(panel_id, behavior);
                    continue;
                }
                let consumed = behavior.Input(&panel_ev, &panel_state, input_state);
                self.sub_tree.put_behavior(panel_id, behavior);
                if consumed {
                    self.sub_view
                        .InvalidatePainting(&self.sub_tree, panel_id);
                    return true;
                }
            }
        }

        false
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        // C++ NF_FOCUS_CHANGED → SetViewFocused(IsFocused())
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.sub_view
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }
        // C++ NF_VIEWING_CHANGED → SetViewGeometry(...)
        if flags.intersects(NoticeFlags::VIEW_CHANGED | NoticeFlags::LAYOUT_CHANGED) {
            self.sync_geometry(state);
        }
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, state: &PanelState) {
        if !state.viewed {
            return;
        }

        // Drive sub-tree lifecycle. C++ panels participate in a global
        // scheduler that calls HandleNotice continuously. Rust sub-trees are
        // driven here. We interleave animator ticks, view updates, and notice
        // delivery to allow the seek mechanism to expand panels layer by layer
        // within a single frame (C++ does this across scheduler cycles).
        self.sub_tree.run_panel_cycles();
        self.sub_tree.HandleNotice(state.is_focused(), state.pixel_tallness);
        self.sub_view.Update(&mut self.sub_tree);

        // Run animator + expand loop: animator seeks deeper, view updates
        // reveal new panels, HandleNotice triggers their LayoutChildren.
        for _ in 0..50 {
            let anim_active = if let Some(mut anim) = self.active_animator.take() {
                let cont = anim.animate(&mut self.sub_view, &mut self.sub_tree, 0.016);
                if cont {
                    self.active_animator = Some(anim);
                }
                cont
            } else {
                false
            };

            self.sub_view.Update(&mut self.sub_tree);
            let had_notices = self.sub_tree.HandleNotice(state.is_focused(), state.pixel_tallness);

            if !anim_active && !had_notices {
                break;
            }
        }

        // Update the sub-view's viewing state so panel coordinates are current.
        self.sub_view.Update(&mut self.sub_tree);

        // The parent's paint_panel_recursive set the painter's origin to
        // (base_offset.x + viewed_x, base_offset.y + viewed_y), i.e. this
        // panel's top-left in pixel space. The sub-view's panels have their
        // viewed coordinates relative to the sub-view viewport starting at
        // (0, 0). paint_panel_recursive adds each panel's viewed_x/y to the
        // base offset, so we pass the current origin as-is (the sub-view's
        // (0, 0) == this panel's top-left).
        let base_offset = painter.origin();
        let bg = self.sub_view.GetBackgroundColor();
        let root = self.sub_root();

        self.sub_view
            .paint_sub_tree(&mut self.sub_tree, painter, root, base_offset, bg);
    }

    fn GetCursor(&self) -> emCursor {
        self.sub_view.GetCursor()
    }

    fn get_title(&self) -> Option<String> {
        // C++ delegates to SubView->GetTitle(), which walks the sub-view's
        // panel tree for a title.
        let root = self.sub_root();
        Some(self.sub_tree.get_title(root))
    }

    fn drain_parent_invalidation(&mut self) -> Option<ParentInvalidation> {
        let title = self.sub_view.is_title_invalid();
        let cursor = self.sub_view.is_cursor_invalid();
        let has_dirty = self.sub_view.has_dirty_rects();

        if !title && !cursor && !has_dirty {
            return None;
        }

        if title {
            self.sub_view.clear_title_invalid();
        }
        if cursor {
            self.sub_view.clear_cursor_invalid();
        }

        // C++ SubViewPortClass::InvalidatePainting calls
        // SuperPanel.InvalidatePaintingOnView(x,y,w,h) which forwards
        // directly to GetView().InvalidatePainting(x,y,w,h). The dirty
        // rects are already in absolute view (pixel) coordinates, so we
        // pass them through unchanged.
        let dirty_rects = if has_dirty {
            self.sub_view.take_dirty_rects()
        } else {
            Vec::new()
        };

        Some(ParentInvalidation {
            dirty_rects,
            title_invalid: title,
            cursor_invalid: cursor,
        })
    }
}
