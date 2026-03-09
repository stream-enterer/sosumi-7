use bitflags::bitflags;

use super::tree::{PanelId, PanelTree};
use crate::foundation::{Color, Rect};
use crate::render::{Painter, Stroke};

bitflags! {
    /// Flags controlling view behavior.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct ViewFlags: u32 {
        const POPUP_ZOOM           = 0b0000_0001;
        const NO_ZOOM              = 0b0000_0010;
        const NO_SCROLL            = 0b0000_0100;
        const NO_NAVIGATE          = 0b0000_1000;
        const FULLSCREEN           = 0b0001_0000;
        const ROOT_SAME_TALLNESS   = 0b0010_0000;
        const NO_USER_NAVIGATION   = 0b0100_0000;
        const NO_FOCUS_HIGHLIGHT   = 0b1000_0000;
        const NO_ACTIVE_HIGHLIGHT  = 0b0001_0000_0000;
    }
}

/// State for a visited panel in the view hierarchy.
#[derive(Clone, Debug)]
pub struct VisitState {
    pub panel: PanelId,
    /// Relative X position of the view within the panel.
    pub rel_x: f64,
    /// Relative Y position.
    pub rel_y: f64,
    /// Relative zoom/area factor.
    pub rel_a: f64,
}

const MAX_SVP_SIZE: f64 = 1.0e12;
const MIN_DIMENSION: f64 = 0.0001;

/// Direction for neighbor navigation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    Right,
    Down,
    Left,
    Up,
}

/// The View manages the viewport — which panels are visible and how they're
/// navigated and rendered.
pub struct View {
    root: PanelId,
    active: Option<PanelId>,
    focused: Option<PanelId>,
    visit_stack: Vec<VisitState>,
    pub flags: ViewFlags,
    viewport_width: f64,
    viewport_height: f64,
    svp: Option<PanelId>,
    background_color: Color,
    svp_update_count: u32,
    window_focused: bool,
    /// Panel targeted by the visiting animator's seek operation.
    seek_pos_panel: Option<PanelId>,
    /// Child name being sought within `seek_pos_panel`.
    seek_pos_child_name: String,
    /// Pixel tallness (height/width ratio of a single pixel).
    pixel_tallness: f64,
    /// Dirty rectangles accumulated by invalidate_painting calls.
    dirty_rects: Vec<Rect>,
    /// Whether the view title needs to be refreshed.
    title_invalid: bool,
    /// Whether the cursor display needs to be refreshed.
    cursor_invalid: bool,
    /// Whether the control panel needs to be refreshed.
    control_panel_invalid: bool,
    /// Whether the current activation is adherent (indirect, via a descendant).
    activation_adherent: bool,
    /// Set by scroll/zoom/navigate operations that change the viewport and need
    /// a repaint, but don't go through the notice or dirty_rects systems.
    viewport_changed: bool,
    /// VIEW-003: Set by scroll/zoom to signal that any active animator should be
    /// aborted. Consumers (window loop) should check and clear this flag.
    needs_animator_abort: bool,
    /// D-PANEL-02: Pending animated visit goal. Navigation methods set this
    /// instead of doing an instant jump. The window loop feeds this to the
    /// VisitingViewAnimator. None means no pending animated visit.
    pending_animated_visit: Option<VisitState>,
}

impl View {
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
            svp: None,
            background_color: Color::rgba(0x80, 0x80, 0x80, 0xFF),
            svp_update_count: 0,
            window_focused: true,
            seek_pos_panel: None,
            seek_pos_child_name: String::new(),
            pixel_tallness: if viewport_width > 0.0 {
                viewport_height / viewport_width
            } else {
                1.0
            },
            dirty_rects: Vec::new(),
            title_invalid: false,
            cursor_invalid: false,
            control_panel_invalid: false,
            activation_adherent: false,
            viewport_changed: false,
            needs_animator_abort: false,
            pending_animated_visit: None,
        }
    }

    // --- Accessors ---

    pub fn root(&self) -> PanelId {
        self.root
    }

    pub fn active(&self) -> Option<PanelId> {
        self.active
    }

    pub fn set_active(&mut self, id: PanelId) {
        self.active = Some(id);
    }

    pub fn focused(&self) -> Option<PanelId> {
        self.focused
    }

    pub fn set_focus(&mut self, id: Option<PanelId>) {
        self.focused = id;
    }

    pub fn svp(&self) -> Option<PanelId> {
        self.svp
    }

    pub fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub fn set_window_focused(&mut self, tree: &mut PanelTree, focused: bool) {
        if self.window_focused == focused {
            return;
        }
        self.window_focused = focused;
        // C++ emView::SetFocused iterates ALL panels and queues:
        //   NF_VIEW_FOCUS_CHANGED | NF_UPDATE_PRIORITY_CHANGED on every panel
        //   NF_FOCUS_CHANGED additionally on panels in the active path
        let ids: Vec<_> = tree.panel_ids();
        for id in ids {
            if let Some(panel) = tree.get_mut(id) {
                let mut flags = super::behavior::NoticeFlags::VIEW_FOCUS_CHANGED
                    | super::behavior::NoticeFlags::UPDATE_PRIORITY_CHANGED;
                if panel.in_active_path {
                    flags |= super::behavior::NoticeFlags::FOCUS_CHANGED;
                }
                panel.pending_notices.insert(flags);
            }
        }
    }

    pub fn is_activation_adherent(&self) -> bool {
        self.activation_adherent
    }

    // --- Seeking ---

    /// Set the seek target panel and child name for the visiting animator.
    ///
    /// When the visiting animator is navigating to a panel that doesn't yet
    /// exist, this records which panel to watch and which child name is being
    /// sought, so the animator can monitor creation progress.
    pub fn set_seek_pos(&mut self, tree: &mut PanelTree, panel: Option<PanelId>, child_name: &str) {
        let child_name = if panel.is_some() { child_name } else { "" };

        if self.seek_pos_panel != panel {
            // Notify old panel that sought name is cleared
            if let Some(old_id) = self.seek_pos_panel {
                if let Some(p) = tree.get_mut(old_id) {
                    p.pending_notices
                        .insert(super::behavior::NoticeFlags::SOUGHT_NAME_CHANGED);
                }
            }

            self.seek_pos_panel = panel;
            self.seek_pos_child_name = child_name.to_string();

            // Notify new panel that sought name is set
            if let Some(new_id) = self.seek_pos_panel {
                if let Some(p) = tree.get_mut(new_id) {
                    p.pending_notices
                        .insert(super::behavior::NoticeFlags::SOUGHT_NAME_CHANGED);
                }
            }
        } else if panel.is_some() && self.seek_pos_child_name != child_name {
            self.seek_pos_child_name = child_name.to_string();
            if let Some(id) = self.seek_pos_panel {
                if let Some(p) = tree.get_mut(id) {
                    p.pending_notices
                        .insert(super::behavior::NoticeFlags::SOUGHT_NAME_CHANGED);
                }
            }
        }
    }

    /// Returns the current seek target panel, if any.
    pub fn seek_pos_panel(&self) -> Option<PanelId> {
        self.seek_pos_panel
    }

    /// Returns the child name being sought.
    pub fn seek_pos_child_name(&self) -> &str {
        &self.seek_pos_child_name
    }

    /// Returns true if seeking can still succeed — the seek panel exists in
    /// the tree and has the potential to create the sought child.
    pub fn is_hope_for_seeking(&self, tree: &PanelTree) -> bool {
        if let Some(id) = self.seek_pos_panel {
            if let Some(panel) = tree.get(id) {
                if self.seek_pos_child_name.is_empty() {
                    return true;
                }
                if tree
                    .find_child_by_name(id, &self.seek_pos_child_name)
                    .is_some()
                {
                    return true;
                }
                return panel.behavior.is_some();
            }
        }
        false
    }

    pub fn current_visit(&self) -> &VisitState {
        self.visit_stack
            .last()
            .expect("visit stack should never be empty")
    }

    pub fn visit_stack(&self) -> &[VisitState] {
        &self.visit_stack
    }

    pub fn visit_stack_mut(&mut self) -> &mut Vec<VisitState> {
        &mut self.visit_stack
    }

    // --- Navigation primitives ---

    pub fn visit(&mut self, panel: PanelId, rel_x: f64, rel_y: f64, rel_a: f64) {
        self.visit_stack.push(VisitState {
            panel,
            rel_x,
            rel_y,
            rel_a,
        });
        self.active = Some(panel);
        self.viewport_changed = true;
    }

    pub fn visit_fullsized(&mut self, tree: &PanelTree, panel: PanelId) {
        let (x, y, a) = self.calc_visit_fullsized_coords(tree, panel, false);
        self.visit(panel, x, y, a);
    }

    /// D-PANEL-02: Request an animated visit to a panel. Sets a pending goal
    /// that the window loop feeds to the VisitingViewAnimator. Also sets the
    /// active panel immediately for UI responsiveness.
    pub fn animated_visit(
        &mut self,
        tree: &mut PanelTree,
        panel: PanelId,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        adherent: bool,
    ) {
        self.set_active_panel(tree, panel, adherent);
        self.pending_animated_visit = Some(VisitState {
            panel,
            rel_x,
            rel_y,
            rel_a,
        });
    }

    /// D-PANEL-02: Request an animated visit to a panel at its natural size.
    pub fn animated_visit_panel(&mut self, tree: &mut PanelTree, panel: PanelId, adherent: bool) {
        let (x, y, a) = self.calc_visit_coords(tree, panel);
        self.animated_visit(tree, panel, x, y, a, adherent);
    }

    pub fn go_back(&mut self) -> bool {
        if self.visit_stack.len() > 1 {
            self.visit_stack.pop();
            self.active = Some(self.current_visit().panel);
            self.viewport_changed = true;
            true
        } else {
            false
        }
    }

    pub fn go_home(&mut self) {
        self.visit_stack.truncate(1);
        self.active = Some(self.root);
        self.viewport_changed = true;
    }

    // --- Viewport ---

    /// D-PANEL-05: Clamp dimensions, preserve zoom state on resize (C++ SetGeometry parity).
    pub fn set_viewport(&mut self, tree: &mut PanelTree, width: f64, height: f64) {
        let width = width.max(MIN_DIMENSION);
        let height = height.max(MIN_DIMENSION);

        if (self.viewport_width - width).abs() < 1e-15
            && (self.viewport_height - height).abs() < 1e-15
        {
            return;
        }

        // Save zoom state before change
        let was_zoomed_out = self.visit_stack.last().is_none_or(|s| {
            s.rel_x.abs() < 0.001 && s.rel_y.abs() < 0.001 && (s.rel_a - 1.0).abs() < 0.001
        });

        self.viewport_width = width;
        self.viewport_height = height;
        self.pixel_tallness = height / width;

        // Preserve zoom state: if was zoomed out, re-apply zoom-out.
        // Otherwise, keep current visit coords (panel stays at same relative position).
        if was_zoomed_out {
            if let Some(state) = self.visit_stack.last_mut() {
                state.rel_x = 0.0;
                state.rel_y = 0.0;
                state.rel_a = 1.0;
            }
        }

        // C++ SetGeometry parity: inline-update root panel layout when
        // VF_ROOT_SAME_TALLNESS is set (mirrors RootPanel->Layout(0,0,1,GetHomeTallness())).
        if self.flags.contains(ViewFlags::ROOT_SAME_TALLNESS) {
            tree.set_layout_rect(self.root, 0.0, 0.0, 1.0, self.pixel_tallness);
        }

        self.viewport_changed = true;
    }

    pub fn viewport_size(&self) -> (f64, f64) {
        (self.viewport_width, self.viewport_height)
    }

    // --- Zoom & Scroll ---

    /// Fix-point zoom: keeps the viewport point (center_x, center_y) mapped to the
    /// same panel-space point before and after zoom.
    pub fn zoom(&mut self, factor: f64, center_x: f64, center_y: f64) {
        if self.flags.contains(ViewFlags::NO_ZOOM) {
            return;
        }
        // VIEW-003: Signal abort for any active animator (C++ AbortActiveAnimator)
        self.needs_animator_abort = true;
        if let Some(state) = self.visit_stack.last_mut() {
            let old_a = state.rel_a;
            let new_a = (old_a * factor).clamp(0.001, 1000.0);
            if (new_a - old_a).abs() < 1e-15 {
                return;
            }
            // Fix-point: before zoom, viewport point (center_x, center_y) maps to
            // panel_point = (center_x - rel_x * vw) / (rel_a * vw)
            // After zoom with new_a, we want the same panel_point, so solve for new rel_x:
            // new_rel_x = rel_x + center_x/vw * (1.0 - old_a/new_a) ... simplified:
            // The key relation: rel_x_new = center_x/vw + (rel_x - center_x/vw) * (old_a / new_a)
            // But our coords use: panel_abs_x = viewport_origin_x + rel_x * viewport_w
            // and scale = rel_a (area). Width scale = sqrt(rel_a * aspect_ratio) ... actually
            // let's use a simpler model: rel_x/rel_y are viewport-relative scroll offsets,
            // rel_a is an area factor. The actual mapping is that the visited panel's
            // viewport rect has center at (rel_x, rel_y) offset and area factor rel_a.
            //
            // For fix-point zoom around (cx, cy) in viewport [0..vw, 0..vh]:
            // normalize center to [0..1]: ncx = cx / vw, ncy = cy / vh
            // The visited panel has viewport-space position based on (rel_x, rel_y, rel_a).
            // After changing rel_a, we adjust rel_x/rel_y to keep the same point under (cx,cy).
            //
            // In the C++ code, the transform is: abs_x = view_x + rel_x * view_w, etc.
            // So the point at viewport fraction f maps to panel space via the inverse.
            // After zoom, we need: f = (panel_point - new_abs_x) / new_abs_w
            // which should equal: f = (panel_point - old_abs_x) / old_abs_w
            //
            // Simplest correct formulation using rel_a as linear scale on each axis:
            let sqrt_old = old_a.sqrt();
            let sqrt_new = new_a.sqrt();
            let ratio = sqrt_old / sqrt_new;
            let ncx = if self.viewport_width > 0.0 {
                center_x / self.viewport_width
            } else {
                0.5
            };
            let ncy = if self.viewport_height > 0.0 {
                center_y / self.viewport_height
            } else {
                0.5
            };
            state.rel_x = ncx + (state.rel_x - ncx) * ratio;
            state.rel_y = ncy + (state.rel_y - ncy) * ratio;
            state.rel_a = new_a;
            self.viewport_changed = true;
        }
    }

    pub fn scroll(&mut self, dx: f64, dy: f64) {
        if self.flags.contains(ViewFlags::NO_SCROLL) {
            return;
        }
        // VIEW-003: Signal abort for any active animator (C++ AbortActiveAnimator)
        self.needs_animator_abort = true;
        if let Some(state) = self.visit_stack.last_mut() {
            // Convert pixel deltas to view-coordinate space by dividing by the
            // panel's viewed size (viewport * sqrt(rel_a)), matching C++ Scroll.
            let scale = state.rel_a.sqrt().max(1e-10);
            state.rel_x += dx / (self.viewport_width.max(1.0) * scale);
            state.rel_y += dy / (self.viewport_height.max(1.0) * scale);
            self.viewport_changed = true;
        }
    }

    /// Atomic scroll+zoom with done-distance feedback.
    pub fn raw_scroll_and_zoom(
        &mut self,
        tree: &mut PanelTree,
        fix_x: f64,
        fix_y: f64,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> [f64; 3] {
        let before = self.visit_stack.last().cloned();
        if dx != 0.0 || dy != 0.0 {
            self.scroll(dx, dy);
        }
        if dz != 0.0 {
            let factor = dz.exp();
            self.zoom(factor, fix_x, fix_y);
        }
        self.update_viewing(tree);
        let after = self.visit_stack.last().cloned();
        match (before, after) {
            (Some(b), Some(a)) => {
                let done_x = (a.rel_x - b.rel_x) * self.viewport_width.max(1.0);
                let done_y = (a.rel_y - b.rel_y) * self.viewport_height.max(1.0);
                let done_z = if b.rel_a > 0.0 {
                    (a.rel_a / b.rel_a).ln()
                } else {
                    0.0
                };
                [done_x, done_y, done_z]
            }
            _ => [0.0, 0.0, 0.0],
        }
    }

    /// Zoom sensitivity for VIFs/animators.
    pub fn get_zoom_factor_log_per_pixel(&self) -> f64 {
        1.33 / ((self.viewport_width + self.viewport_height) * 0.25).max(1.0)
    }

    // --- Zoom out ---

    pub fn zoom_out(&mut self, tree: &mut PanelTree) {
        self.raw_zoom_out(tree);
    }

    pub fn raw_zoom_out(&mut self, tree: &mut PanelTree) {
        if let Some(state) = self.visit_stack.last_mut() {
            state.rel_x = 0.0;
            state.rel_y = 0.0;
            state.rel_a = 1.0;
            self.viewport_changed = true;
        }
        self.update_viewing(tree);
    }

    pub fn is_zoomed_out(&self, _tree: &PanelTree) -> bool {
        if let Some(state) = self.visit_stack.last() {
            (state.rel_x.abs() < 0.001)
                && (state.rel_y.abs() < 0.001)
                && ((state.rel_a - 1.0).abs() < 0.001)
        } else {
            true
        }
    }

    // --- CalcVisitCoords ---

    /// Compute optimal (rel_x, rel_y, rel_a) to view `panel` well.
    pub fn calc_visit_coords(&self, tree: &PanelTree, panel: PanelId) -> (f64, f64, f64) {
        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);
        let v_aspect = vw / vh;

        // Walk from panel up to root, build chain root→panel
        let mut chain_rev: Vec<PanelId> = tree.ancestors(panel);
        chain_rev.reverse(); // [root, ..., parent, panel]

        // Start from root — width normalized to 1.0, height preserves layout_rect aspect
        let root_lr = tree
            .get(self.root)
            .map(|p| p.layout_rect)
            .unwrap_or_default();
        let root_norm_h = if root_lr.w > MIN_DIMENSION {
            (root_lr.h / root_lr.w).max(MIN_DIMENSION)
        } else {
            1.0
        };
        let mut rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        rects.push((0.0, 0.0, 1.0, root_norm_h));

        for i in 1..chain_rev.len() {
            let id = chain_rev[i];
            let lr = tree.get(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, ph) = rects[i - 1];
            let x = px + lr.x * pw;
            let y = py + lr.y * ph;
            let w = lr.w * pw;
            let h = lr.h * ph;
            rects.push((x, y, w, h));
        }

        let (px, py, pw, ph) = *rects.last().unwrap_or(&(0.0, 0.0, 1.0, 1.0));

        // We want the panel to be nicely visible. Compute rel_a so panel fills
        // a good portion of the viewport.
        let panel_aspect = if ph > 0.0 { pw / ph } else { 1.0 };

        // Target: panel should occupy ~80% of viewport in its constraining dimension
        let target_fraction = 0.8;
        let rel_a = if panel_aspect > v_aspect {
            // Panel is wider than viewport → constrain by width
            let scale = target_fraction / pw.max(MIN_DIMENSION);
            scale * scale * (pw * ph).max(MIN_DIMENSION * MIN_DIMENSION)
        } else {
            // Panel is taller → constrain by height
            let scale = target_fraction / ph.max(MIN_DIMENSION);
            scale * scale * (pw * ph).max(MIN_DIMENSION * MIN_DIMENSION)
        };
        let rel_a = rel_a.clamp(0.001, 1000.0);

        // Center the panel in the viewport
        let scale = rel_a.sqrt();
        let rel_x = 0.5 - (px + pw * 0.5) * scale;
        let rel_y = 0.5 - (py + ph * 0.5) * scale;

        (rel_x, rel_y, rel_a)
    }

    /// Compute coords to show panel at its natural aspect (fullsized).
    pub fn calc_visit_fullsized_coords(
        &self,
        tree: &PanelTree,
        panel: PanelId,
        fill: bool,
    ) -> (f64, f64, f64) {
        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);
        let v_aspect = vw / vh;

        // Compute panel's normalized rect from root
        let chain = tree.ancestors(panel);
        let mut chain_rev: Vec<PanelId> = chain;
        chain_rev.reverse();

        let root_lr = tree
            .get(self.root)
            .map(|p| p.layout_rect)
            .unwrap_or_default();
        let root_norm_h = if root_lr.w > MIN_DIMENSION {
            (root_lr.h / root_lr.w).max(MIN_DIMENSION)
        } else {
            1.0
        };
        let mut rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        rects.push((0.0, 0.0, 1.0, root_norm_h));

        for i in 1..chain_rev.len() {
            let id = chain_rev[i];
            let lr = tree.get(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, ph) = rects[i - 1];
            rects.push((px + lr.x * pw, py + lr.y * ph, lr.w * pw, lr.h * ph));
        }

        let (px, py, pw, ph) = *rects.last().unwrap_or(&(0.0, 0.0, 1.0, 1.0));
        let panel_aspect = if ph > 0.0 { pw / ph } else { 1.0 };

        // Fill: panel covers viewport. Fit: panel fits inside viewport.
        let scale = if fill {
            if panel_aspect > v_aspect {
                1.0 / ph.max(MIN_DIMENSION)
            } else {
                1.0 / pw.max(MIN_DIMENSION)
            }
        } else if panel_aspect > v_aspect {
            1.0 / pw.max(MIN_DIMENSION)
        } else {
            1.0 / ph.max(MIN_DIMENSION)
        };

        let rel_a =
            (scale * scale * (pw * ph).max(MIN_DIMENSION * MIN_DIMENSION)).clamp(0.001, 1000.0);
        let s = rel_a.sqrt();
        let rel_x = 0.5 - (px + pw * 0.5) * s;
        let rel_y = 0.5 - (py + ph * 0.5) * s;

        (rel_x, rel_y, rel_a)
    }

    // --- ViewFlags with side effects ---

    pub fn set_view_flags(&mut self, flags: ViewFlags, tree: &mut PanelTree) {
        let old = self.flags;
        let mut new_flags = flags;

        if new_flags.contains(ViewFlags::NO_ZOOM) {
            new_flags.remove(ViewFlags::POPUP_ZOOM);
            new_flags.insert(ViewFlags::NO_USER_NAVIGATION);
        }

        if new_flags == old {
            return;
        }

        self.flags = new_flags;

        if new_flags.contains(ViewFlags::POPUP_ZOOM) && !old.contains(ViewFlags::POPUP_ZOOM) {
            self.raw_zoom_out(tree);
        }

        if new_flags.contains(ViewFlags::NO_ZOOM) && !old.contains(ViewFlags::NO_ZOOM) {
            self.raw_zoom_out(tree);
        }
    }

    // --- Active Panel Management ---

    pub fn set_active_panel(&mut self, tree: &mut PanelTree, panel: PanelId, adherent: bool) {
        // Walk up to nearest focusable panel (self included, matching C++ SetActivePanel)
        let target = if tree.get(panel).map(|p| p.focusable).unwrap_or(false) {
            panel
        } else {
            tree.focusable_ancestor(panel).unwrap_or(panel)
        };

        if self.active == Some(target) {
            if self.activation_adherent != adherent {
                self.activation_adherent = adherent;
            }
            return;
        }

        // Build notice flags: always ACTIVE_CHANGED, add FOCUS_CHANGED if focused
        let mut flags = super::behavior::NoticeFlags::ACTIVE_CHANGED;
        if self.window_focused {
            flags.insert(super::behavior::NoticeFlags::FOCUS_CHANGED);
        }

        // Clear old active path
        if let Some(old_active) = self.active {
            let old_path = tree.ancestors(old_active);
            for id in &old_path {
                if let Some(p) = tree.get_mut(*id) {
                    p.is_active = false;
                    p.in_active_path = false;
                    p.pending_notices.insert(flags);
                }
            }
        }

        // Set new active path
        self.active = Some(target);
        if let Some(p) = tree.get_mut(target) {
            p.is_active = true;
        }
        let new_path = tree.ancestors(target);
        for id in &new_path {
            if let Some(p) = tree.get_mut(*id) {
                p.in_active_path = true;
                p.pending_notices.insert(flags);
            }
        }
        self.activation_adherent = adherent;
    }

    /// Auto-select best visible focusable panel as active.
    ///
    /// D-PANEL-03: Uses center-containment descent (C++ parity) instead of
    /// max-area. Starts at SVP and descends into the deepest focusable child
    /// whose clip rect contains the viewport center, stopping when children
    /// are too small (< 99% view width AND height, AND < 33% view area).
    pub fn set_active_panel_best_possible(&mut self, tree: &mut PanelTree) {
        let svp = match self.svp {
            Some(id) => id,
            None => return,
        };

        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);
        let cx = vw * 0.5;
        let cy = vh * 0.5;
        let min_w = vw * 0.99;
        let min_h = vh * 0.99;
        let min_a = vw * vh * 0.33;

        let mut best = svp;

        // Center-containment descent
        loop {
            let children: Vec<PanelId> = tree.children_rev(best).collect();
            let mut found = None;
            for child in children {
                let p = match tree.get(child) {
                    Some(p) if p.viewed && p.focusable => p,
                    _ => continue,
                };
                // Check if child's clip rect contains view center
                if p.clip_x <= cx
                    && (p.clip_x + p.clip_w) > cx
                    && p.clip_y <= cy
                    && (p.clip_y + p.clip_h) > cy
                {
                    found = Some(child);
                    break;
                }
            }

            match found {
                Some(child) => {
                    let p = tree.get(child).expect("child just found");
                    // Don't descend into panels smaller than thresholds
                    if p.clip_w < min_w && p.clip_h < min_h && (p.clip_w * p.clip_h) < min_a {
                        break;
                    }
                    best = child;
                }
                None => break,
            }
        }

        // Ensure best is focusable (ascend if needed)
        if !tree.get(best).map(|p| p.focusable).unwrap_or(false) {
            if let Some(anc) = tree.focusable_ancestor(best) {
                best = anc;
            } else {
                return;
            }
        }

        // Adherent check: keep current active if still visible and best is ancestor
        if self.activation_adherent {
            if let Some(active_id) = self.active {
                if let Some(active_panel) = tree.get(active_id) {
                    if active_panel.viewed
                        && active_panel.viewed_width >= 4.0
                        && active_panel.viewed_height >= 4.0
                    {
                        if let Some(best_panel) = tree.get(best) {
                            if best_panel.in_active_path {
                                self.set_active_panel(tree, active_id, true);
                                return;
                            }
                        }
                    }
                }
            }
        }
        self.set_active_panel(tree, best, false);
    }

    // --- Coordinate transform: update_viewing ---

    /// Compute absolute viewport coordinates for all panels. Called once per frame.
    pub fn update_viewing(&mut self, tree: &mut PanelTree) {
        // Save old viewing state before clearing, so we can detect changes and
        // queue C++-parity notices (NF_VIEWING_CHANGED, NF_UPDATE_PRIORITY_CHANGED,
        // NF_MEMORY_LIMIT_CHANGED) for panels whose viewed rect changed.
        let old_viewing: Vec<(PanelId, bool, f64, f64, f64, f64)> = tree
            .all_ids()
            .into_iter()
            .filter_map(|id| {
                tree.get(id).map(|p| {
                    (
                        id,
                        p.viewed,
                        p.viewed_x,
                        p.viewed_y,
                        p.viewed_width,
                        p.viewed_height,
                    )
                })
            })
            .collect();

        tree.clear_viewing_flags();

        let root = match tree.root() {
            Some(r) => r,
            None => return,
        };

        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);

        // Get the visited panel and its view state
        let visit = self.current_visit().clone();
        let visited = visit.panel;

        // If NO_ZOOM, force visited = root
        let visited = if self.flags.contains(ViewFlags::NO_ZOOM) {
            root
        } else {
            visited
        };

        // Compute visited panel's absolute viewport rect
        // visit state: (rel_x, rel_y, rel_a) define where the visited panel appears
        // rel_a is area factor (visited_area / viewport_area)
        // visited panel width in viewport: sqrt(rel_a * visited_aspect) * vw...
        // Actually, let's use a simpler model:
        // The visited panel's viewport rect is centered at (rel_x * vw, rel_y * vh) with
        // area = rel_a * vw * vh, preserving the panel's natural aspect ratio.

        // First compute the visited panel's natural aspect from its layout chain
        let chain = tree.ancestors(visited);
        let mut chain_rev: Vec<PanelId> = chain;
        chain_rev.reverse(); // root .. visited

        // Compute each panel's normalized rect relative to root.
        // Root width is normalized to 1.0; height preserves the layout_rect aspect.
        let root_lr = tree.get(root).map(|p| p.layout_rect).unwrap_or_default();
        let root_norm_h = if root_lr.w > MIN_DIMENSION {
            (root_lr.h / root_lr.w).max(MIN_DIMENSION)
        } else {
            1.0
        };
        let mut norm_rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        norm_rects.push((0.0, 0.0, 1.0, root_norm_h));
        for i in 1..chain_rev.len() {
            let id = chain_rev[i];
            let lr = tree.get(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, ph) = norm_rects[i - 1];
            norm_rects.push((px + lr.x * pw, py + lr.y * ph, lr.w * pw, lr.h * ph));
        }

        let (vnx, vny, vnw, vnh) = *norm_rects.last().unwrap_or(&(0.0, 0.0, 1.0, 1.0));

        // The root's viewport rect is computed from the visited panel's desired position.
        // The visited panel should appear at viewport position defined by (rel_x, rel_y, rel_a).
        //
        // visited viewport width = sqrt(rel_a) * vw (for square viewport/panel)
        // More precisely: visited occupies rel_a fraction of viewport area.
        // visited_vw * visited_vh = rel_a * vw * vh
        // visited_vw / visited_vh = vnw / vnh (preserve aspect)
        // So: visited_vw = sqrt(rel_a * vw * vh * vnw / vnh)
        //     visited_vh = sqrt(rel_a * vw * vh * vnh / vnw)
        // But that's relative to the root's size. Actually:
        //
        // Root viewport rect: root_vw, root_vh
        // visited_vx = root_vx + vnx * root_vw
        // visited_vy = root_vy + vny * root_vh
        // visited_vw = vnw * root_vw
        // visited_vh = vnh * root_vh
        //
        // We want: visited panel centered at (rel_x * vw + vw/2, rel_y * vh + vh/2)?
        // No, let's use: rel_x, rel_y are the offset of the visited panel center from
        // viewport center, in viewport-fraction units. rel_a is area ratio.
        //
        // visited_vw * visited_vh = rel_a * vw * vh
        // visited_vw / visited_vh = vnw / vnh (panel aspect)
        // => visited_vw = sqrt(rel_a * vw * vh * (vnw / vnh))
        //    visited_vh = sqrt(rel_a * vw * vh * (vnh / vnw))
        //
        // But visited_vw = vnw * root_vw and visited_vh = vnh * root_vh
        // => root_vw = visited_vw / vnw, root_vh = visited_vh / vnh
        //
        // visited center in viewport: (vw * (0.5 + rel_x), vh * (0.5 + rel_y))
        // visited center = root_vx + (vnx + vnw/2) * root_vw

        let vnw_safe = vnw.max(MIN_DIMENSION);
        let vnh_safe = vnh.max(MIN_DIMENSION);
        let panel_aspect = vnw_safe / vnh_safe;

        let visited_vw = (visit.rel_a * vw * vh * panel_aspect).sqrt();
        let visited_vh = (visit.rel_a * vw * vh / panel_aspect).sqrt();

        let root_vw = visited_vw / vnw_safe;
        let root_vh = visited_vh / vnh_safe;

        // Visited center in viewport
        let vcx = vw * (0.5 + visit.rel_x);
        let vcy = vh * (0.5 + visit.rel_y);

        // Root position
        let root_vx = vcx - (vnx + vnw_safe * 0.5) * root_vw;
        let root_vy = vcy - (vny + vnh_safe * 0.5) * root_vh;

        // Now recursively set viewed coords for all panels starting from root
        let viewport = Rect::new(0.0, 0.0, vw, vh);
        let root_abs = Rect::new(root_vx, root_vy, root_vw, root_vh);
        self.compute_viewed_recursive(tree, root, root_abs, &viewport);

        // Queue notices for panels whose viewing state changed (C++ parity:
        // NF_VIEWING_CHANGED, NF_UPDATE_PRIORITY_CHANGED, NF_MEMORY_LIMIT_CHANGED).
        for &(id, old_viewed, old_vx, old_vy, old_vw, old_vh) in &old_viewing {
            let changed = if let Some(p) = tree.get(id) {
                old_viewed != p.viewed
                    || (old_vx - p.viewed_x).abs() > f64::EPSILON
                    || (old_vy - p.viewed_y).abs() > f64::EPSILON
                    || (old_vw - p.viewed_width).abs() > f64::EPSILON
                    || (old_vh - p.viewed_height).abs() > f64::EPSILON
            } else {
                false
            };
            if changed {
                if let Some(p) = tree.get_mut(id) {
                    p.pending_notices.insert(
                        super::behavior::NoticeFlags::VISIBILITY
                            | super::behavior::NoticeFlags::UPDATE_PRIORITY_CHANGED
                            | super::behavior::NoticeFlags::MEMORY_LIMIT_CHANGED,
                    );
                }
            }
        }

        // Find SVP: deepest ancestor of visited panel whose absolute area <= MAX_SVP_SIZE
        let ancestors = tree.ancestors(visited);
        self.svp = None;
        for &id in &ancestors {
            if let Some(p) = tree.get(id) {
                let area = p.viewed_width * p.viewed_height;
                if area <= MAX_SVP_SIZE {
                    self.svp = Some(id);
                    break;
                }
            }
        }
        if self.svp.is_none() {
            self.svp = Some(root);
        }

        // Set in_viewed_path from root to SVP
        if let Some(svp_id) = self.svp {
            let svp_ancestors = tree.ancestors(svp_id);
            for &id in &svp_ancestors {
                if let Some(p) = tree.get_mut(id) {
                    p.in_viewed_path = true;
                }
            }
        }

        // Set in_active_path from root to active
        if let Some(active_id) = self.active {
            if tree.contains(active_id) {
                let active_ancestors = tree.ancestors(active_id);
                for &id in &active_ancestors {
                    if let Some(p) = tree.get_mut(id) {
                        p.in_active_path = true;
                    }
                }
                if let Some(p) = tree.get_mut(active_id) {
                    p.is_active = true;
                }
            }
        }

        // SVP jitter prevention
        self.svp_update_count += 1;

        // Auto-expansion dispatch
        self.update_auto_expansion(tree);
    }

    fn compute_viewed_recursive(
        &self,
        tree: &mut PanelTree,
        id: PanelId,
        abs: Rect,
        viewport: &Rect,
    ) {
        let clip = viewport.intersection(&abs);

        {
            let panel = match tree.get_mut(id) {
                Some(p) => p,
                None => return,
            };

            if !panel.visible {
                return;
            }

            panel.viewed_x = abs.x;
            panel.viewed_y = abs.y;
            panel.viewed_width = abs.w;
            panel.viewed_height = abs.h;

            if let Some(c) = clip {
                panel.clip_x = c.x;
                panel.clip_y = c.y;
                panel.clip_w = c.w;
                panel.clip_h = c.h;
                panel.viewed = true;
            }
        }

        // Recurse into children
        let children: Vec<PanelId> = tree.children(id).collect();
        for child in children {
            let lr = tree.get(child).map(|p| p.layout_rect).unwrap_or_default();
            let child_abs = Rect::new(
                abs.x + lr.x * abs.w,
                abs.y + lr.y * abs.h,
                lr.w * abs.w,
                lr.h * abs.h,
            );
            self.compute_viewed_recursive(tree, child, child_abs, viewport);
        }
    }

    /// Check auto-expansion thresholds for all panels and trigger
    /// expansion or shrinking as needed. Called at the end of
    /// `update_viewing()` after all viewed coordinates are computed.
    fn update_auto_expansion(&self, tree: &mut PanelTree) {
        let panel_ids = tree.all_ids();

        for id in panel_ids {
            let (threshold_value, threshold_type, currently_expanded, decision_invalid) = {
                let Some(panel) = tree.get(id) else {
                    continue;
                };
                (
                    panel.ae_threshold_value,
                    panel.ae_threshold_type,
                    panel.ae_expanded,
                    panel.ae_decision_invalid,
                )
            };

            // Skip panels with no threshold set (default 0.0 with default auto_expand() = false)
            // A panel must have explicitly set a threshold > 0.0 to participate
            if threshold_value <= 0.0 && !currently_expanded {
                continue;
            }

            let vc = tree.get_view_condition(id, threshold_type);
            let should_expand = vc >= threshold_value;

            if should_expand && !currently_expanded {
                // Expand: set flag and trigger layout_children via notice
                if let Some(panel) = tree.get_mut(id) {
                    panel.ae_expanded = true;
                    panel.ae_decision_invalid = false;
                    panel.ae_invalid = false;
                    panel
                        .pending_notices
                        .insert(super::behavior::NoticeFlags::LAYOUT_CHANGED);
                }
            } else if !should_expand && currently_expanded {
                // Shrink: delete children and clear flag
                tree.delete_all_children(id);
                if let Some(panel) = tree.get_mut(id) {
                    panel.ae_expanded = false;
                    panel.ae_decision_invalid = false;
                    panel.ae_invalid = false;
                }
            } else if currently_expanded && decision_invalid {
                // Re-evaluate: panel requested re-check
                if let Some(panel) = tree.get_mut(id) {
                    panel.ae_decision_invalid = false;
                    panel.ae_invalid = false;
                    panel
                        .pending_notices
                        .insert(super::behavior::NoticeFlags::LAYOUT_CHANGED);
                }
            }
        }
    }

    // --- Navigation ---

    /// D-PANEL-01: Navigate to next focusable panel (C++ VisitNext parity).
    ///
    /// Tries next focusable sibling; if at end, ascends to focusable parent
    /// and wraps to its first focusable child.
    pub fn visit_next(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };

        // Try next focusable sibling (no wrap)
        if let Some(next) = tree.focusable_next(active) {
            self.animated_visit_panel(tree, next, false);
            return;
        }

        // No next sibling: go to focusable parent's first focusable child
        let parent = tree
            .focusable_ancestor(active)
            .unwrap_or_else(|| tree.root().unwrap_or(active));
        if parent != active {
            if let Some(first) = tree.focusable_first_child(parent) {
                self.animated_visit_panel(tree, first, false);
            }
        }
    }

    /// D-PANEL-01: Navigate to previous focusable panel (C++ VisitPrev parity).
    pub fn visit_prev(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };

        // Try previous focusable sibling (no wrap)
        if let Some(prev) = tree.focusable_prev(active) {
            self.animated_visit_panel(tree, prev, false);
            return;
        }

        // No previous sibling: go to focusable parent's last focusable child
        let parent = tree
            .focusable_ancestor(active)
            .unwrap_or_else(|| tree.root().unwrap_or(active));
        if parent != active {
            if let Some(last) = tree.focusable_last_child(parent) {
                self.animated_visit_panel(tree, last, false);
            }
        }
    }

    pub fn visit_first(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };
        let parent = match tree.parent(active) {
            Some(p) => p,
            None => return,
        };
        for child in tree.children(parent) {
            if tree.get(child).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, child, false);
                return;
            }
        }
    }

    pub fn visit_last(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };
        let parent = match tree.parent(active) {
            Some(p) => p,
            None => return,
        };
        for child in tree.children_rev(parent) {
            if tree.get(child).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, child, false);
                return;
            }
        }
    }

    pub fn visit_left(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Left);
    }

    pub fn visit_right(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Right);
    }

    pub fn visit_up(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Up);
    }

    pub fn visit_down(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Down);
    }

    pub fn visit_in(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };
        // Find first focusable child
        for child in tree.children(active) {
            if tree.get(child).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, child, false);
                return;
            }
        }
        // No focusable child — visit active fullsized
        self.visit_fullsized(tree, active);
    }

    pub fn visit_out(&mut self, tree: &mut PanelTree) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };
        // Go to focusable parent — check parent itself first, then walk up
        if let Some(parent) = tree.parent(active) {
            if tree.get(parent).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, parent, false);
                return;
            }
            if let Some(focusable) = tree.focusable_ancestor(parent) {
                self.animated_visit_panel(tree, focusable, false);
                return;
            }
        }
        // At root — zoom out
        self.zoom_out(tree);
    }

    fn visit_neighbour(&mut self, tree: &mut PanelTree, direction: Direction) {
        if self
            .flags
            .intersects(ViewFlags::NO_NAVIGATE | ViewFlags::NO_USER_NAVIGATION)
        {
            return;
        }
        let active = match self.active {
            Some(id) => id,
            None => return,
        };
        let parent = match tree.parent(active) {
            Some(p) => p,
            None => return,
        };

        let active_panel = match tree.get(active) {
            Some(p) => p,
            None => return,
        };
        let ax = active_panel.viewed_x;
        let ay = active_panel.viewed_y;
        let aw = active_panel.viewed_width;
        let ah = active_panel.viewed_height;
        let acx = ax + aw * 0.5;
        let acy = ay + ah * 0.5;

        let siblings: Vec<PanelId> = tree.children(parent).collect();
        let mut best: Option<(PanelId, f64)> = None;

        for &sib in &siblings {
            if sib == active {
                continue;
            }
            let sp = match tree.get(sib) {
                Some(p) if p.focusable && p.viewed => p,
                _ => continue,
            };
            let scx = sp.viewed_x + sp.viewed_width * 0.5;
            let scy = sp.viewed_y + sp.viewed_height * 0.5;

            let (dx, dy) = (scx - acx, scy - acy);

            // Rotate based on direction so "forward" is always +x
            let (rx, ry) = match direction {
                Direction::Right => (dx, dy),
                Direction::Down => (-dy, dx),
                Direction::Left => (-dx, -dy),
                Direction::Up => (dy, -dx),
            };

            if rx <= 1e-12 {
                continue;
            }

            let dist = (rx * rx + ry * ry).sqrt();
            let penalty = if ry.abs() > rx * 0.707 { 10.0 } else { 1.0 };
            let score = dist * penalty;

            if best.map(|(_, s)| score < s).unwrap_or(true) {
                best = Some((sib, score));
            }
        }

        if let Some((winner, _)) = best {
            self.animated_visit_panel(tree, winner, false);
        }
    }

    // --- Hit testing ---

    pub fn get_panel_at(&self, tree: &PanelTree, x: f64, y: f64) -> Option<PanelId> {
        let svp = self.svp?;
        self.hit_test_recursive(tree, svp, x, y, false)
    }

    pub fn get_focusable_panel_at(&self, tree: &PanelTree, x: f64, y: f64) -> Option<PanelId> {
        let svp = self.svp?;
        self.hit_test_recursive(tree, svp, x, y, true)
    }

    fn hit_test_recursive(
        &self,
        tree: &PanelTree,
        id: PanelId,
        x: f64,
        y: f64,
        focusable_only: bool,
    ) -> Option<PanelId> {
        let panel = tree.get(id)?;
        if !panel.viewed {
            return None;
        }

        let clip = Rect::new(panel.clip_x, panel.clip_y, panel.clip_w, panel.clip_h);
        if !clip.contains_point(x, y) {
            return None;
        }

        // Check children in reverse Z-order (last = topmost)
        let children: Vec<PanelId> = tree.children_rev(id).collect();
        for child in children {
            if let Some(hit) = self.hit_test_recursive(tree, child, x, y, focusable_only) {
                return Some(hit);
            }
        }

        // No child hit — check self
        if focusable_only && !panel.focusable {
            return None;
        }

        Some(id)
    }

    /// Remove a panel from the tree, moving activation to its parent if needed.
    /// Matches C++ `~emPanel` which calls `SetFocusable(false)` before unlinking,
    /// causing `View.SetActivePanel(Parent, false)`.
    pub fn remove_panel(&mut self, tree: &mut PanelTree, id: PanelId) {
        if tree.get(id).map(|p| p.in_active_path).unwrap_or(false) {
            if let Some(parent) = tree.parent(id) {
                self.set_active_panel(tree, parent, false);
            } else {
                self.active = None;
            }
        }
        tree.remove(id);
    }

    // --- Panel-level wrappers ---

    /// Activate a panel (delegate to set_active_panel).
    pub fn activate_panel(&mut self, tree: &mut PanelTree, panel: PanelId) {
        self.set_active_panel(tree, panel, false);
    }

    /// Focus the view and activate a panel.
    pub fn focus_panel(&mut self, tree: &mut PanelTree, panel: PanelId) {
        self.set_window_focused(tree, true);
        self.set_active_panel(tree, panel, false);
    }

    /// Whether a panel is focused: it is the active panel and the view's
    /// window is focused.
    pub fn is_panel_focused(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let is_active = tree.get(panel).map(|p| p.is_active).unwrap_or(false);
        is_active && self.window_focused
    }

    /// Whether a panel is in the focused path: it is in the active path and
    /// the view's window is focused.
    pub fn is_panel_in_focused_path(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let in_active_path = tree.get(panel).map(|p| p.in_active_path).unwrap_or(false);
        in_active_path && self.window_focused
    }

    /// Whether a panel's activation is adherent (indirect, via a descendant).
    pub fn is_panel_activated_adherent(&self, tree: &PanelTree, panel: PanelId) -> bool {
        tree.get(panel).map(|p| p.is_active).unwrap_or(false) && self.activation_adherent
    }

    /// Whether the view's window is focused (panel-level delegate).
    pub fn is_view_focused(&self) -> bool {
        self.window_focused
    }

    /// Return wall-clock milliseconds (since Unix epoch).
    pub fn get_input_clock_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Panel-level delegate for `get_input_clock_ms` (mirrors `emPanel::GetInputClockMS`).
    pub fn get_panel_input_clock_ms(&self) -> u64 {
        self.get_input_clock_ms()
    }

    // --- Pixel tallness ---

    /// Return the pixel tallness (height/width ratio of a pixel).
    ///
    /// Corresponds to `emPanel::GetViewedPixelTallness` (delegates to
    /// `View.CurrentPixelTallness`).
    pub fn pixel_tallness(&self) -> f64 {
        self.pixel_tallness
    }

    // --- Invalidation ---

    /// Mark the view's title as needing a refresh. Only takes effect when
    /// the panel is in the active path.
    ///
    /// Corresponds to `emPanel::InvalidateTitle`.
    pub fn invalidate_title(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_active_path = tree.get(panel).map(|p| p.in_active_path).unwrap_or(false);
        if in_active_path {
            self.title_invalid = true;
        }
    }

    /// Mark the view's cursor as needing a refresh. Only takes effect when
    /// the panel is in the viewed path.
    ///
    /// Corresponds to `emPanel::InvalidateCursor`.
    pub fn invalidate_cursor(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_viewed_path = tree.get(panel).map(|p| p.in_viewed_path).unwrap_or(false);
        if in_viewed_path {
            self.cursor_invalid = true;
        }
    }

    /// Mark the entire panel clip rect as needing repaint.
    ///
    /// Corresponds to `emPanel::InvalidatePainting()` (no-arg overload).
    pub fn invalidate_painting(&mut self, tree: &PanelTree, panel: PanelId) {
        let p = match tree.get(panel) {
            Some(p) if p.viewed => p,
            _ => return,
        };
        self.dirty_rects
            .push(Rect::new(p.clip_x, p.clip_y, p.clip_w, p.clip_h));
    }

    /// Mark a sub-rectangle of the panel as needing repaint. The rectangle
    /// is specified in panel coordinates and is transformed to view
    /// coordinates, then clipped against the panel's clip rect.
    ///
    /// Corresponds to `emPanel::InvalidatePainting(x, y, w, h)`.
    pub fn invalidate_painting_rect(
        &mut self,
        tree: &PanelTree,
        panel: PanelId,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        let p = match tree.get(panel) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        // Transform from panel space to view space
        let mut vx = p.viewed_x + x * p.viewed_width;
        let mut vy = p.viewed_y + y * p.viewed_height;
        let mut vw = w * p.viewed_width;
        let mut vh = h * p.viewed_height;

        // Clip against the panel's clip rect
        let clip_x2 = p.clip_x + p.clip_w;
        let clip_y2 = p.clip_y + p.clip_h;

        if vx < p.clip_x {
            vw -= p.clip_x - vx;
            vx = p.clip_x;
        }
        if vy < p.clip_y {
            vh -= p.clip_y - vy;
            vy = p.clip_y;
        }
        if vw > clip_x2 - vx {
            vw = clip_x2 - vx;
        }
        if vh > clip_y2 - vy {
            vh = clip_y2 - vy;
        }

        if vw > 0.0 && vh > 0.0 {
            self.dirty_rects.push(Rect::new(vx, vy, vw, vh));
        }
    }

    /// Signal that the control panel needs refreshing. Only takes effect
    /// when the panel is in the active path.
    ///
    /// Corresponds to `emPanel::InvalidateControlPanel`.
    pub fn invalidate_control_panel(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_active_path = tree.get(panel).map(|p| p.in_active_path).unwrap_or(false);
        if in_active_path {
            self.control_panel_invalid = true;
        }
    }

    /// Whether the title has been invalidated since the last clear.
    pub fn is_title_invalid(&self) -> bool {
        self.title_invalid
    }

    /// Clear the title-invalid flag.
    pub fn clear_title_invalid(&mut self) {
        self.title_invalid = false;
    }

    /// Whether the cursor has been invalidated since the last clear.
    pub fn is_cursor_invalid(&self) -> bool {
        self.cursor_invalid
    }

    /// Clear the cursor-invalid flag.
    pub fn clear_cursor_invalid(&mut self) {
        self.cursor_invalid = false;
    }

    /// Whether the control panel has been invalidated since the last clear.
    pub fn is_control_panel_invalid(&self) -> bool {
        self.control_panel_invalid
    }

    /// Clear the control-panel-invalid flag.
    pub fn clear_control_panel_invalid(&mut self) {
        self.control_panel_invalid = false;
    }

    /// Check whether any dirty rectangles have been accumulated.
    pub fn has_dirty_rects(&self) -> bool {
        !self.dirty_rects.is_empty()
    }

    /// Drain accumulated dirty rectangles.
    pub fn take_dirty_rects(&mut self) -> Vec<Rect> {
        std::mem::take(&mut self.dirty_rects)
    }

    /// Whether the viewport has changed (scroll/zoom/visit) since last reset.
    pub fn viewport_changed(&self) -> bool {
        self.viewport_changed
    }

    /// Clear the viewport-changed flag after processing.
    pub fn clear_viewport_changed(&mut self) {
        self.viewport_changed = false;
    }

    /// VIEW-003: Whether scroll/zoom was called and any active animator should
    /// be aborted. The window loop should check this and abort the
    /// VisitingViewAnimator if active.
    pub fn needs_animator_abort(&self) -> bool {
        self.needs_animator_abort
    }

    /// Clear the animator-abort flag.
    pub fn clear_animator_abort(&mut self) {
        self.needs_animator_abort = false;
    }

    /// D-PANEL-02: Take the pending animated visit goal. Returns `Some` if a
    /// navigation method requested an animated visit. The window loop should
    /// feed this to the VisitingViewAnimator.
    pub fn take_pending_animated_visit(&mut self) -> Option<VisitState> {
        self.pending_animated_visit.take()
    }

    /// Whether there is a pending animated visit goal.
    pub fn has_pending_animated_visit(&self) -> bool {
        self.pending_animated_visit.is_some()
    }

    /// TF-003: Scroll the viewport to make a panel-pixel rect visible.
    ///
    /// `rect` is `(x, y, w, h)` in the panel's paint coordinate space
    /// (same space as `paint(w, h)`). The method converts to viewport
    /// coordinates, checks visibility, and scrolls the minimum amount
    /// needed.
    ///
    /// Matches C++ `emTextField::ScrollToCursor` → `emView::Scroll` path.
    pub fn scroll_to_panel_rect(
        &mut self,
        tree: &PanelTree,
        panel: PanelId,
        rect: (f64, f64, f64, f64),
    ) {
        let p = match tree.get(panel) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        let (rx, ry, rw, rh) = rect;

        // Convert panel-pixel rect to viewport coords.
        // Paint coord (px, py) maps to viewport (viewed_x + px, viewed_y + py)
        // because there is no per-panel scaling in the paint pipeline.
        let vx1 = p.viewed_x + rx;
        let vy1 = p.viewed_y + ry;
        let vx2 = vx1 + rw;
        let vy2 = vy1 + rh;

        let mut dx = 0.0_f64;
        let mut dy = 0.0_f64;
        let mut need = false;

        // Horizontal: bring cursor into viewport [0, viewport_width]
        if vx1 < 0.0 {
            dx = -vx1; // shift content right
            need = true;
        } else if vx2 > self.viewport_width {
            dx = self.viewport_width - vx2; // shift content left
            need = true;
        }

        // Vertical: bring cursor into viewport [0, viewport_height]
        if vy1 < 0.0 {
            dy = -vy1;
            need = true;
        } else if vy2 > self.viewport_height {
            dy = self.viewport_height - vy2;
            need = true;
        }

        if need {
            // scroll() divides by scale internally. To achieve a viewport
            // shift of exactly (dx, dy) pixels, pre-multiply by scale.
            let scale = self
                .visit_stack
                .last()
                .map(|s| s.rel_a.sqrt().max(1e-10))
                .unwrap_or(1.0);
            self.scroll(dx * scale, dy * scale);
        }
    }

    // --- Update loop ---

    pub fn update(&mut self, tree: &mut PanelTree) {
        self.update_viewing(tree);

        // VIEW-003: After scroll/zoom or viewport change, reselect active panel
        // (C++ calls SetActivePanelBestPossible after Scroll/Zoom)
        let need_reselect = match self.active {
            None => true,
            Some(id) => !tree.contains(id) || !tree.get(id).map(|p| p.focusable).unwrap_or(false),
        };
        if need_reselect || self.viewport_changed {
            self.set_active_panel_best_possible(tree);
        }
    }

    // --- Supreme panel ---

    pub fn supreme_panel(&self) -> PanelId {
        self.svp.unwrap_or(self.current_visit().panel)
    }

    // --- Paint ---

    pub fn paint(&self, tree: &mut PanelTree, painter: &mut Painter) {
        // Fill background
        painter.push_state();
        painter.paint_rect(
            0.0,
            0.0,
            self.viewport_width,
            self.viewport_height,
            self.background_color,
        );
        painter.pop_state();

        // Paint from SVP using absolute viewed coords
        let start = self.svp.unwrap_or(self.root);
        let base_offset = painter.offset();
        self.paint_panel_recursive(tree, painter, start, base_offset);

        // D-PANEL-06: Paint focus/active highlight (C++ PaintHighlight parity)
        self.paint_highlight(tree, painter);
    }

    /// D-PANEL-06: Paint highlight around the active panel.
    ///
    /// C++ draws a rounded rectangle with arrows around the active panel's
    /// substance rect. White normally, light yellow if adherent, dimmed if
    /// window not focused.
    fn paint_highlight(&self, tree: &PanelTree, painter: &mut Painter) {
        if self.flags.contains(ViewFlags::NO_ACTIVE_HIGHLIGHT) {
            return;
        }

        let active_id = match self.active {
            Some(id) => id,
            None => return,
        };

        let panel = match tree.get(active_id) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        // Get the panel's substance rect in viewport coords
        let (sx, sy, sw, sh, _sr) = tree.get_substance_rect(active_id);
        let hx = panel.viewed_x + sx * panel.viewed_width;
        let hy = panel.viewed_y + sy * panel.viewed_height;
        let hw = sw * panel.viewed_width;
        let hh = sh * panel.viewed_height;

        if hw < 1.0 || hh < 1.0 {
            return;
        }

        // Expand by distance-from-panel (C++ constant: 2.0)
        let pad = 2.0;
        let hx = hx - pad;
        let hy = hy - pad;
        let hw = hw + pad * 2.0;
        let hh = hh + pad * 2.0;

        // Color selection (C++ constants)
        let base_color = if self.activation_adherent {
            Color::rgba(255, 255, 187, 255) // Light yellow for adherent
        } else {
            Color::rgba(255, 255, 255, 255) // White
        };

        let alpha = if !self.window_focused || self.flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT) {
            85 // alpha / 3
        } else {
            255
        };

        let color = Color::rgba(base_color.r(), base_color.g(), base_color.b(), alpha);

        // Stroke width scales with the viewport size for visibility
        let stroke_w = 2.0_f64
            .max((self.viewport_width + self.viewport_height) * 0.002)
            .min(4.0);

        painter.push_state();
        painter.paint_rect_outlined(hx, hy, hw, hh, &Stroke::new(color, stroke_w));
        painter.pop_state();
    }

    fn paint_panel_recursive(
        &self,
        tree: &mut PanelTree,
        painter: &mut Painter,
        id: PanelId,
        base_offset: (f64, f64),
    ) {
        let (vx, vy, vw, vh, clip_x, clip_y, clip_w, clip_h, canvas_color) = {
            match tree.get(id) {
                Some(p) if p.viewed && p.visible => (
                    p.viewed_x,
                    p.viewed_y,
                    p.viewed_width,
                    p.viewed_height,
                    p.clip_x,
                    p.clip_y,
                    p.clip_w,
                    p.clip_h,
                    p.canvas_color,
                ),
                _ => return,
            }
        };

        painter.push_state();
        // Set absolute offset (not cumulative) — viewed coords are in
        // absolute viewport pixels, so each panel computes its offset
        // from the base (tile) offset independently.
        painter.set_offset(base_offset.0 + vx, base_offset.1 + vy);
        painter.clip_rect(clip_x - vx, clip_y - vy, clip_w, clip_h);

        // Skip this panel and its entire subtree if it doesn't intersect
        // the current tile's clip region.
        if painter.clip_is_empty() {
            painter.pop_state();
            return;
        }

        painter.set_canvas_color(canvas_color);

        if let Some(mut behavior) = tree.take_behavior(id) {
            let mut state = tree.build_panel_state(id, self.window_focused);
            state.priority = tree.get_update_priority(
                id,
                self.viewport_width,
                self.viewport_height,
                self.window_focused,
            );
            const DEFAULT_MEMORY_LIMIT: u64 = 1_073_741_824;
            state.memory_limit = tree.get_memory_limit(
                id,
                self.viewport_width,
                self.viewport_height,
                DEFAULT_MEMORY_LIMIT,
                self.seek_pos_panel,
            );
            behavior.paint(painter, vw, vh, &state);
            tree.put_behavior(id, behavior);
        }

        let children: Vec<PanelId> = tree.children(id).collect();
        for child in children {
            self.paint_panel_recursive(tree, painter, child, base_offset);
        }

        painter.pop_state();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelTree;

    fn setup_tree() -> (PanelTree, PanelId, PanelId, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);

        let child1 = tree.create_child(root, "child1");
        tree.get_mut(child1).unwrap().focusable = true;
        tree.set_layout_rect(child1, 0.0, 0.0, 0.5, 1.0);

        let child2 = tree.create_child(root, "child2");
        tree.get_mut(child2).unwrap().focusable = true;
        tree.set_layout_rect(child2, 0.5, 0.0, 0.5, 1.0);

        (tree, root, child1, child2)
    }

    #[test]
    fn test_update_viewing_sets_coords() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // Root should be viewed
        let rp = tree.get(root).unwrap();
        assert!(rp.viewed);
        assert!(rp.viewed_width > 0.0);
        assert!(rp.viewed_height > 0.0);

        // Children should be viewed
        assert!(tree.get(child1).unwrap().viewed);
        assert!(tree.get(child2).unwrap().viewed);
    }

    #[test]
    fn test_svp_selection() {
        let (mut tree, root, _child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // SVP should be set
        assert!(view.svp().is_some());
    }

    #[test]
    fn test_viewed_false_outside_viewport() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);

        let offscreen = tree.create_child(root, "offscreen");
        tree.set_layout_rect(offscreen, 5.0, 5.0, 0.1, 0.1);

        let mut view = View::new(root, 100.0, 100.0);
        view.update_viewing(&mut tree);

        assert!(!tree.get(offscreen).unwrap().viewed);
    }

    #[test]
    fn test_active_path_flags() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.set_active_panel(&mut tree, child1, false);
        view.update_viewing(&mut tree);

        assert!(tree.get(child1).unwrap().is_active);
        assert!(tree.get(child1).unwrap().in_active_path);
        assert!(tree.get(root).unwrap().in_active_path);
    }

    #[test]
    fn test_fix_point_zoom() {
        let (_tree, root, _c1, _c2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);

        // Zoom around center — should keep center stable
        let before = view.current_visit().clone();
        view.zoom(2.0, 400.0, 300.0);
        let after = view.current_visit().clone();

        // rel_a should have doubled
        assert!((after.rel_a - before.rel_a * 2.0).abs() < 0.01);
    }

    #[test]
    fn test_visit_next_prev() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        view.visit_next(&mut tree);
        assert_eq!(view.active(), Some(child2));

        view.visit_prev(&mut tree);
        assert_eq!(view.active(), Some(child1));
    }

    #[test]
    fn test_visit_in_out() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let grandchild = tree.create_child(child1, "gc");
        tree.get_mut(grandchild).unwrap().focusable = true;
        tree.set_layout_rect(grandchild, 0.0, 0.0, 1.0, 1.0);

        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        view.visit_in(&mut tree);
        assert_eq!(view.active(), Some(grandchild));

        view.visit_out(&mut tree);
        assert_eq!(view.active(), Some(child1));
    }

    #[test]
    fn test_hit_testing() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // Hit test in left half should find child1, right half child2
        let left_hit = view.get_focusable_panel_at(&tree, 100.0, 300.0);
        let right_hit = view.get_focusable_panel_at(&tree, 600.0, 300.0);

        assert_eq!(left_hit, Some(child1));
        assert_eq!(right_hit, Some(child2));
    }

    #[test]
    fn test_directional_navigation() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // child2 is to the right of child1
        view.visit_right(&mut tree);
        assert_eq!(view.active(), Some(child2));

        view.visit_left(&mut tree);
        assert_eq!(view.active(), Some(child1));
    }

    #[test]
    fn test_focus_panel() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.set_window_focused(&mut tree, false);

        view.focus_panel(&mut tree, child1);
        assert!(view.window_focused());
        assert_eq!(view.active(), Some(child1));
    }

    #[test]
    fn test_is_panel_focused() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        assert!(view.is_panel_focused(&tree, child1));
        assert!(!view.is_panel_focused(&tree, root));

        view.set_window_focused(&mut tree, false);
        assert!(!view.is_panel_focused(&tree, child1));
    }

    #[test]
    fn test_is_panel_in_focused_path() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        assert!(view.is_panel_in_focused_path(&tree, child1));
        assert!(view.is_panel_in_focused_path(&tree, root));
        assert!(!view.is_panel_in_focused_path(&tree, child2));

        view.set_window_focused(&mut tree, false);
        assert!(!view.is_panel_in_focused_path(&tree, child1));
    }

    #[test]
    fn test_is_view_focused_delegate() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        assert!(view.is_view_focused());
        view.set_window_focused(&mut tree, false);
        assert!(!view.is_view_focused());
    }

    // ── Invalidation tests ───────────────────────────────────────────

    #[test]
    fn test_invalidate_painting_whole() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // child1 should be viewed after update_viewing
        view.invalidate_painting(&tree, child1);
        let rects = view.take_dirty_rects();
        assert_eq!(rects.len(), 1);
        // The dirty rect should be the child's clip rect
        let p = tree.get(child1).unwrap();
        assert!((rects[0].x - p.clip_x).abs() < 1e-6);
        assert!((rects[0].y - p.clip_y).abs() < 1e-6);
    }

    #[test]
    fn test_invalidate_painting_rect() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // Invalidate a sub-rect of child1 in panel coordinates
        view.invalidate_painting_rect(&tree, child1, 0.0, 0.0, 0.5, 0.5);
        let rects = view.take_dirty_rects();
        assert_eq!(rects.len(), 1);
        assert!(rects[0].w > 0.0);
        assert!(rects[0].h > 0.0);

        // Not viewed => no dirty rect
        tree.get_mut(child1).unwrap().viewed = false;
        view.invalidate_painting_rect(&tree, child1, 0.0, 0.0, 1.0, 1.0);
        let rects = view.take_dirty_rects();
        assert!(rects.is_empty());
    }

    #[test]
    fn test_invalidate_title_and_cursor() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // child1 is active, thus in_active_path
        assert!(!view.is_title_invalid());
        view.invalidate_title(&tree, child1);
        assert!(view.is_title_invalid());
        view.clear_title_invalid();
        assert!(!view.is_title_invalid());

        // root is in_viewed_path (it's an ancestor of the SVP)
        assert!(!view.is_cursor_invalid());
        view.invalidate_cursor(&tree, root);
        assert!(view.is_cursor_invalid());
        view.clear_cursor_invalid();

        // child1 is viewed but NOT in_viewed_path => no-op
        view.invalidate_cursor(&tree, child1);
        assert!(!view.is_cursor_invalid());
    }

    #[test]
    fn test_invalidate_control_panel() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // child1 is in active path
        assert!(!view.is_control_panel_invalid());
        view.invalidate_control_panel(&tree, child1);
        assert!(view.is_control_panel_invalid());
        view.clear_control_panel_invalid();

        // child2 is NOT in active path
        view.invalidate_control_panel(&tree, child2);
        assert!(!view.is_control_panel_invalid());
    }

    #[test]
    fn test_pixel_tallness() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        let view = View::new(root, 800.0, 600.0);
        assert!((view.pixel_tallness() - 0.75).abs() < 1e-6);

        let mut view2 = View::new(root, 1920.0, 1080.0);
        assert!((view2.pixel_tallness() - 1080.0 / 1920.0).abs() < 1e-6);

        view2.set_viewport(&mut tree, 100.0, 200.0);
        assert!((view2.pixel_tallness() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_activation_adherent() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // Direct activation is not adherent
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.is_activation_adherent());
        assert!(!view.is_panel_activated_adherent(&tree, child1));

        // Explicit adherent activation
        view.set_active_panel(&mut tree, child1, true);
        assert!(view.is_activation_adherent());
        assert!(view.is_panel_activated_adherent(&tree, child1));

        // Switching to a different panel clears adherent
        view.set_active_panel(&mut tree, root, false);
        assert!(!view.is_activation_adherent());
    }

    #[test]
    fn test_activation_adherent_early_return_update() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = View::new(root, 800.0, 600.0);
        view.update_viewing(&mut tree);

        // Set active non-adherent
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.is_activation_adherent());

        // Re-set same panel as adherent — hits early-return path, updates flag
        view.set_active_panel(&mut tree, child1, true);
        assert!(view.is_activation_adherent());

        // Re-set same panel as non-adherent — hits early-return path again
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.is_activation_adherent());
    }

    #[test]
    fn test_get_input_clock_ms() {
        let (_tree, root, _c1, _c2) = setup_tree();
        let view = View::new(root, 800.0, 600.0);
        let ms = view.get_input_clock_ms();
        // Should be a reasonable epoch-based timestamp (after year 2020)
        assert!(ms > 1_577_836_800_000);
    }
}
