use crate::input::Cursor;
use crate::render::Painter;

use super::behavior::{NoticeFlags, PanelBehavior, PanelState};
use super::tree::{PanelId, PanelTree};
use super::view::{View, ViewFlags};

/// A panel that embeds a sub-view within the parent view's panel tree.
///
/// This enables split-view or embedded-view functionality by maintaining a
/// separate [`View`] and [`PanelTree`] that are rendered within the bounds of
/// this panel. Input is forwarded from the parent to the sub-view, and
/// painting is delegated to the sub-view's own render pipeline.
///
/// Corresponds to C++ `emSubViewPanel`.
pub struct SubViewPanel {
    sub_tree: PanelTree,
    sub_view: View,
    /// Cached viewed geometry from the parent panel (absolute viewport pixels).
    viewed_x: f64,
    viewed_y: f64,
    viewed_width: f64,
    viewed_height: f64,
}

impl Default for SubViewPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SubViewPanel {
    /// Create a new SubViewPanel with an empty sub-view.
    ///
    /// The sub-tree is initialized with a root panel. Use [`sub_root`],
    /// [`sub_tree_mut`], and [`sub_view_mut`] to populate the sub-view.
    pub fn new() -> Self {
        let mut sub_tree = PanelTree::new();
        let root = sub_tree.create_root("sub_root");
        sub_tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);

        let sub_view = View::new(root, 1.0, 1.0);

        Self {
            sub_tree,
            sub_view,
            viewed_x: 0.0,
            viewed_y: 0.0,
            viewed_width: 1.0,
            viewed_height: 1.0,
        }
    }

    /// Get the root panel ID of the sub-view's panel tree.
    pub fn sub_root(&self) -> PanelId {
        self.sub_tree
            .root()
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
    pub fn sub_view(&self) -> &View {
        &self.sub_view
    }

    /// Get a mutable reference to the sub-view.
    pub fn sub_view_mut(&mut self) -> &mut View {
        &mut self.sub_view
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
                .set_viewport(&mut self.sub_tree, self.viewed_width, self.viewed_height);
        } else {
            // Not viewed — give the sub-view a default geometry.
            // C++ uses (0, 0, 1, GetHeight(), 1.0).
            self.viewed_x = 0.0;
            self.viewed_y = 0.0;
            self.viewed_width = 1.0;
            self.viewed_height = 1.0;
            self.sub_view.set_viewport(&mut self.sub_tree, 1.0, 1.0);
        }
    }
}

impl PanelBehavior for SubViewPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        // C++ NF_FOCUS_CHANGED → SetViewFocused(IsFocused())
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.sub_view
                .set_window_focused(&mut self.sub_tree, state.is_focused());
        }
        // C++ NF_VIEWING_CHANGED → SetViewGeometry(...)
        if flags.intersects(NoticeFlags::VIEW_CHANGED | NoticeFlags::LAYOUT_CHANGED) {
            self.sync_geometry(state);
        }
    }

    fn paint(&mut self, painter: &mut Painter, _w: f64, _h: f64, state: &PanelState) {
        if !state.viewed {
            return;
        }

        // Update the sub-view's viewing state so panel coordinates are current.
        self.sub_view.update_viewing(&mut self.sub_tree);

        // The parent's paint_panel_recursive set the painter's origin to
        // (base_offset.x + viewed_x, base_offset.y + viewed_y), i.e. this
        // panel's top-left in pixel space. The sub-view's panels have their
        // viewed coordinates relative to the sub-view viewport starting at
        // (0, 0). paint_panel_recursive adds each panel's viewed_x/y to the
        // base offset, so we pass the current origin as-is (the sub-view's
        // (0, 0) == this panel's top-left).
        let base_offset = painter.origin();
        let bg = self.sub_view.background_color();
        let root = self.sub_root();

        self.sub_view
            .paint_sub_tree(&mut self.sub_tree, painter, root, base_offset, bg);
    }

    fn get_cursor(&self) -> Cursor {
        self.sub_view.cursor()
    }

    fn get_title(&self) -> Option<String> {
        // C++ delegates to SubView->GetTitle(), which walks the sub-view's
        // panel tree for a title.
        let root = self.sub_root();
        Some(self.sub_tree.get_title(root))
    }
}
