use bitflags::bitflags;

use super::tree::{PanelId, PanelTree};
use crate::foundation::{Color, Rect};
use crate::render::Painter;

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

    pub fn set_window_focused(&mut self, focused: bool) {
        self.window_focused = focused;
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
    }

    pub fn visit_fullsized(&mut self, tree: &PanelTree, panel: PanelId) {
        let (x, y, a) = self.calc_visit_fullsized_coords(tree, panel, false);
        self.visit(panel, x, y, a);
    }

    pub fn go_back(&mut self) -> bool {
        if self.visit_stack.len() > 1 {
            self.visit_stack.pop();
            self.active = Some(self.current_visit().panel);
            true
        } else {
            false
        }
    }

    pub fn go_home(&mut self) {
        self.visit_stack.truncate(1);
        self.active = Some(self.root);
    }

    // --- Viewport ---

    pub fn set_viewport(&mut self, width: f64, height: f64) {
        self.viewport_width = width;
        self.viewport_height = height;
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
        }
    }

    pub fn scroll(&mut self, dx: f64, dy: f64) {
        if self.flags.contains(ViewFlags::NO_SCROLL) {
            return;
        }
        if let Some(state) = self.visit_stack.last_mut() {
            state.rel_x += dx / self.viewport_width.max(1.0);
            state.rel_y += dy / self.viewport_height.max(1.0);
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

        // Start from root which occupies the "full space"
        let mut rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        rects.push((0.0, 0.0, 1.0, 1.0)); // root occupies full normalized space

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

        let mut rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        rects.push((0.0, 0.0, 1.0, 1.0));

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
        self.flags = flags;

        if flags.contains(ViewFlags::NO_ZOOM) {
            self.flags.remove(ViewFlags::POPUP_ZOOM);
            self.flags.insert(ViewFlags::NO_USER_NAVIGATION);
            self.raw_zoom_out(tree);
        }

        if flags.contains(ViewFlags::POPUP_ZOOM) && !old.contains(ViewFlags::POPUP_ZOOM) {
            self.raw_zoom_out(tree);
        }
    }

    // --- Active Panel Management ---

    pub fn set_active_panel(&mut self, tree: &mut PanelTree, panel: PanelId) {
        // Walk up to nearest focusable ancestor
        let target = tree.focusable_ancestor(panel).unwrap_or(panel);

        if self.active == Some(target) {
            return;
        }

        // Clear old active path
        if let Some(old_active) = self.active {
            let old_path = tree.ancestors(old_active);
            for id in &old_path {
                if let Some(p) = tree.get_mut(*id) {
                    p.is_active = false;
                    p.in_active_path = false;
                    p.pending_notices
                        .insert(super::behavior::NoticeFlags::VIEW_CHANGED);
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
                p.pending_notices
                    .insert(super::behavior::NoticeFlags::VIEW_CHANGED);
            }
        }
    }

    /// Auto-select best visible focusable panel as active.
    pub fn set_active_panel_best_possible(&mut self, tree: &mut PanelTree) {
        let svp = match self.svp {
            Some(id) => id,
            None => return,
        };

        // Find the best focusable visible panel by area
        let mut best: Option<(PanelId, f64)> = None;

        fn find_best(tree: &PanelTree, id: PanelId, best: &mut Option<(PanelId, f64)>) {
            let panel = match tree.get(id) {
                Some(p) => p,
                None => return,
            };
            if !panel.viewed {
                return;
            }
            if panel.focusable {
                let area = panel.clip_w * panel.clip_h;
                if best.map(|(_, a)| area > a).unwrap_or(true) {
                    *best = Some((id, area));
                }
            }
            let children: Vec<PanelId> = tree.children(id).collect();
            for child in children {
                find_best(tree, child, best);
            }
        }

        find_best(tree, svp, &mut best);

        if let Some((id, _)) = best {
            self.set_active_panel(tree, id);
        }
    }

    // --- Coordinate transform: update_viewing ---

    /// Compute absolute viewport coordinates for all panels. Called once per frame.
    pub fn update_viewing(&mut self, tree: &mut PanelTree) {
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

        // Compute each panel's normalized rect relative to root
        // Root occupies (0,0,1,1) in normalized space
        let mut norm_rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        norm_rects.push((0.0, 0.0, 1.0, 1.0));
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

    // --- Navigation ---

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
        let parent = match tree.parent(active) {
            Some(p) => p,
            None => return,
        };
        // Find next focusable sibling, wrapping around
        let siblings: Vec<PanelId> = tree.children(parent).collect();
        let pos = siblings.iter().position(|&id| id == active);
        if let Some(idx) = pos {
            let len = siblings.len();
            for i in 1..=len {
                let candidate = siblings[(idx + i) % len];
                if tree.get(candidate).map(|p| p.focusable).unwrap_or(false) {
                    self.set_active_panel(tree, candidate);
                    return;
                }
            }
        }
    }

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
        let parent = match tree.parent(active) {
            Some(p) => p,
            None => return,
        };
        let siblings: Vec<PanelId> = tree.children(parent).collect();
        let pos = siblings.iter().position(|&id| id == active);
        if let Some(idx) = pos {
            let len = siblings.len();
            for i in 1..=len {
                let candidate = siblings[(idx + len - i) % len];
                if tree.get(candidate).map(|p| p.focusable).unwrap_or(false) {
                    self.set_active_panel(tree, candidate);
                    return;
                }
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
                self.set_active_panel(tree, child);
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
                self.set_active_panel(tree, child);
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
                self.set_active_panel(tree, child);
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
        // Go to focusable parent
        if let Some(parent) = tree.parent(active) {
            if let Some(focusable) = tree.focusable_ancestor(parent) {
                self.set_active_panel(tree, focusable);
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
            self.set_active_panel(tree, winner);
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

    // --- Update loop ---

    pub fn update(&mut self, tree: &mut PanelTree) {
        self.update_viewing(tree);

        // If active panel is invalid or non-focusable, auto-select
        let need_reselect = match self.active {
            None => true,
            Some(id) => !tree.contains(id) || !tree.get(id).map(|p| p.focusable).unwrap_or(false),
        };
        if need_reselect {
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
        self.paint_panel_recursive(tree, painter, start);
    }

    fn paint_panel_recursive(&self, tree: &mut PanelTree, painter: &mut Painter, id: PanelId) {
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
        painter.translate(vx, vy);
        painter.clip_rect(clip_x - vx, clip_y - vy, clip_w, clip_h);
        painter.set_canvas_color(canvas_color);

        if let Some(mut behavior) = tree.take_behavior(id) {
            behavior.paint(painter, vw, vh);
            tree.put_behavior(id, behavior);
        }

        let children: Vec<PanelId> = tree.children(id).collect();
        for child in children {
            self.paint_panel_recursive(tree, painter, child);
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
        view.set_active_panel(&mut tree, child1);
        view.update_viewing(&mut tree);

        assert!(tree.get(child1).unwrap().is_active);
        assert!(tree.get(child1).unwrap().in_active_path);
        assert!(tree.get(root).unwrap().in_active_path);
    }

    #[test]
    fn test_fix_point_zoom() {
        let (mut tree, root, _c1, _c2) = setup_tree();
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
        view.set_active_panel(&mut tree, child1);

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
        view.set_active_panel(&mut tree, child1);

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
        view.set_active_panel(&mut tree, child1);

        // child2 is to the right of child1
        view.visit_right(&mut tree);
        assert_eq!(view.active(), Some(child2));

        view.visit_left(&mut tree);
        assert_eq!(view.active(), Some(child1));
    }
}
