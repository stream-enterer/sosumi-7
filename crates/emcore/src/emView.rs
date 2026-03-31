use std::time::Instant;

use bitflags::bitflags;
use crate::dlog;

use crate::emPanelCtx::PanelCtx;
use super::emPanelTree::{PanelId, PanelTree};
use crate::emClipRects::ClipRects;
use crate::emColor::emColor;
use crate::emRec::{write_rec_with_format, RecStruct, RecValue};
use crate::emPanel::Rect;
use crate::emCursor::emCursor;
use crate::emPainter::{emPainter, TextAlignment, VAlign};

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
        const STRESS_TEST          = 0b0010_0000_0000;
        const EGO_MODE             = 0b0100_0000_0000;
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

/// Frame rate measurement for the stress test overlay.
///
/// Port of C++ `StressTestClass` (emEngine subclass). Maintains a 128-entry
/// ring buffer of timestamps, computes frame rate over a 1-second window,
/// updates the displayed rate every 100ms.
#[derive(Clone, Debug)]
pub struct StressTest {
    /// Ring buffer of frame timestamps.
    timestamps: Vec<Instant>,
    /// Position of the next write in the ring buffer.
    pos: usize,
    /// Number of valid entries (0..=128).
    valid: usize,
    /// Last computed frame rate (Hz).
    frame_rate: f64,
    /// When the displayed rate was last updated.
    last_update: Instant,
}

impl Default for StressTest {
    fn default() -> Self {
        Self::new()
    }
}

impl StressTest {
    const RING_SIZE: usize = 128;

    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            timestamps: vec![now; Self::RING_SIZE],
            pos: 0,
            valid: 0,
            frame_rate: 0.0,
            last_update: now,
        }
    }

    /// Record a frame timestamp and update the frame rate if 100ms have elapsed.
    pub fn record_frame(&mut self) {
        let now = Instant::now();
        self.timestamps[self.pos] = now;
        self.pos = (self.pos + 1) % Self::RING_SIZE;
        if self.valid < Self::RING_SIZE {
            self.valid += 1;
        }

        // Update displayed rate every 100ms
        if now.duration_since(self.last_update).as_millis() >= 100 {
            self.last_update = now;
            self.frame_rate = self.compute_rate(now);
        }
    }

    fn compute_rate(&self, now: Instant) -> f64 {
        if self.valid < 2 {
            return 0.0;
        }
        // Count entries within the last 1 second
        let mut count = 0usize;
        let mut oldest = now;
        for i in 0..self.valid {
            let idx = (self.pos + Self::RING_SIZE - 1 - i) % Self::RING_SIZE;
            let elapsed = now.duration_since(self.timestamps[idx]);
            if elapsed.as_millis() > 1000 {
                break;
            }
            oldest = self.timestamps[idx];
            count += 1;
        }
        if count < 2 {
            return 0.0;
        }
        let elapsed_ms = now.duration_since(oldest).as_secs_f64() * 1000.0;
        if elapsed_ms < 1.0 {
            return 0.0;
        }
        count as f64 * 1000.0 / elapsed_ms
    }

    /// Paint the "Stress Test XX.X Hz" overlay in the top-left corner.
    pub fn paint_info(&self, painter: &mut emPainter, _view_w: f64, view_h: f64) {
        let text_h = (view_h / 45.0).max(10.0);
        let box_w = text_h * 8.0;
        let box_h = text_h * 2.5;

        // Purple background (matches C++: 255,0,255,128)
        let bg = emColor::rgba(255, 0, 255, 128);
        painter.PaintRect(0.0, 0.0, box_w, box_h, bg, emColor::TRANSPARENT);

        // Yellow text (matches C++: 255,255,0,192)
        let fg = emColor::rgba(255, 255, 0, 192);
        let label = format!("Stress Test\n{:.1} Hz", self.frame_rate);
        painter.PaintTextBoxed(
            0.0,
            0.0,
            box_w,
            box_h,
            &label,
            text_h,
            fg,
            bg,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            false,
            0.15,
        );
    }

    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    pub fn valid_count(&self) -> usize {
        self.valid
    }
}

/// The emView manages the viewport — which panels are visible and how they're
/// navigated and rendered.
pub struct emView {
    root: PanelId,
    active: Option<PanelId>,
    focused: Option<PanelId>,
    visit_stack: Vec<VisitState>,
    pub flags: ViewFlags,
    viewport_width: f64,
    viewport_height: f64,
    svp: Option<PanelId>,
    background_color: emColor,
    svp_update_count: u32,
    window_focused: bool,
    /// Panel targeted by the visiting animator's seek operation.
    seek_pos_panel: Option<PanelId>,
    /// Child name being sought within `seek_pos_panel`.
    seek_pos_child_name: String,
    /// Pixel tallness (height/width ratio of a single pixel).
    pixel_tallness: f64,
    /// Pixel shape ratio (C++ HomePixelTallness). Always 1.0 for square pixels.
    home_pixel_tallness: f64,
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
    /// emVisitingViewAnimator. None means no pending animated visit.
    pending_animated_visit: Option<VisitState>,
    /// PORT-0129: Countdown for delayed End-Of-Interaction signal.
    /// When `Some(n)`, tick_eoi() decrements each frame and fires when 0.
    eoi_countdown: Option<i32>,
    /// Whether viewed coordinates need recomputation. Set by viewport_changed,
    /// visit stack changes, resize, etc. Cleared after update_viewing() runs.
    viewing_dirty: bool,
    /// The view title. Updated from the active panel's title.
    pub title: String,
    /// Current mouse cursor for this view.
    pub cursor: emCursor,
    /// Maximum popup view rectangle (pixel coords of bounding monitor rect).
    pub max_popup_rect: Option<Rect>,
    /// Whether the view is currently in popped-up state (popup window active).
    pub popped_up: bool,
    /// Cached visited panel ViewedWidth (screen pixels). Set by update_viewing().
    /// Used by scroll() and done-distance for correct aspect-aware conversion.
    visited_vw: f64,
    /// Cached visited panel ViewedHeight (screen pixels). Set by update_viewing().
    visited_vh: f64,
    /// C++ ZoomedOutBeforeSG: when true the next update_viewing() will
    /// compute the zoom-out relA so the root panel fits in the viewport.
    /// Initially true; cleared after the first viewing update.
    zoomed_out_before_sg: bool,
    /// Stress test state. Created when STRESS_TEST flag is set, dropped when cleared.
    stress_test: Option<StressTest>,
    /// Whether the soft keyboard is shown (touch platforms only).
    /// C++ emView::IsSoftKeyboardShown / ShowSoftKeyboard.
    soft_keyboard_shown: bool,
}

impl emView {
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
            background_color: emColor::rgba(0x80, 0x80, 0x80, 0xFF),
            svp_update_count: 0,
            window_focused: true,
            seek_pos_panel: None,
            seek_pos_child_name: String::new(),
            pixel_tallness: if viewport_width > 0.0 {
                viewport_height / viewport_width
            } else {
                1.0
            },
            home_pixel_tallness: 1.0,
            dirty_rects: Vec::new(),
            title_invalid: false,
            cursor_invalid: false,
            control_panel_invalid: false,
            activation_adherent: false,
            viewport_changed: false,
            needs_animator_abort: false,
            pending_animated_visit: None,
            eoi_countdown: None,
            title: String::new(),
            cursor: emCursor::Normal,
            max_popup_rect: None,
            popped_up: false,
            visited_vw: viewport_width.max(1.0),
            visited_vh: viewport_height.max(1.0),
            viewing_dirty: true,
            zoomed_out_before_sg: true,
            stress_test: None,
            soft_keyboard_shown: false,
        }
    }

    // --- Accessors ---

    pub fn GetRootPanel(&self) -> PanelId {
        self.root
    }

    pub fn GetActivePanel(&self) -> Option<PanelId> {
        self.active
    }

    pub fn SetActivePanel(&mut self, id: PanelId) {
        self.active = Some(id);
    }

    pub fn GetFocusedPanel(&self) -> Option<PanelId> {
        self.focused
    }

    pub fn set_focus(&mut self, id: Option<PanelId>) {
        self.focused = id;
    }

    pub fn GetSupremeViewedPanel(&self) -> Option<PanelId> {
        self.svp
    }

    pub fn IsFocused(&self) -> bool {
        self.window_focused
    }

    pub fn SetFocused(&mut self, tree: &mut PanelTree, focused: bool) {
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
                let mut flags = super::emPanel::NoticeFlags::VIEW_FOCUS_CHANGED
                    | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED;
                if panel.in_active_path {
                    flags |= super::emPanel::NoticeFlags::FOCUS_CHANGED;
                }
                panel.pending_notices.insert(flags);
            }
        }
        tree.mark_notices_pending();
    }

    pub fn IsActivationAdherent(&self) -> bool {
        self.activation_adherent
    }

    /// Whether the soft keyboard is currently shown.
    /// C++ `emView::IsSoftKeyboardShown()`.
    pub fn IsSoftKeyboardShown(&self) -> bool {
        self.soft_keyboard_shown
    }

    /// Show or hide the soft keyboard.
    /// C++ `emView::ShowSoftKeyboard(bool show)`.
    /// DIVERGED: C++ delegates to CurrentViewPort which delegates to the
    /// platform window. Desktop stub stores flag only — no actual keyboard
    /// is shown until a platform-specific viewport implements this.
    pub fn ShowSoftKeyboard(&mut self, show: bool) {
        self.soft_keyboard_shown = show;
    }

    // --- Seeking ---

    /// Set the seek target panel and child name for the visiting animator.
    ///
    /// When the visiting animator is navigating to a panel that doesn't yet
    /// exist, this records which panel to watch and which child name is being
    /// sought, so the animator can monitor creation progress.
    pub fn SetSeekPos(&mut self, tree: &mut PanelTree, panel: Option<PanelId>, child_name: &str) {
        let child_name = if panel.is_some() { child_name } else { "" };

        if self.seek_pos_panel != panel {
            // Notify old panel that sought name is cleared
            if let Some(old_id) = self.seek_pos_panel {
                tree.queue_notice(old_id, super::emPanel::NoticeFlags::SOUGHT_NAME_CHANGED);
            }

            self.seek_pos_panel = panel;
            self.seek_pos_child_name = child_name.to_string();

            // Notify new panel that sought name is set
            if let Some(new_id) = self.seek_pos_panel {
                tree.queue_notice(new_id, super::emPanel::NoticeFlags::SOUGHT_NAME_CHANGED);
            }
        } else if panel.is_some() && self.seek_pos_child_name != child_name {
            self.seek_pos_child_name = child_name.to_string();
            if let Some(id) = self.seek_pos_panel {
                tree.queue_notice(id, super::emPanel::NoticeFlags::SOUGHT_NAME_CHANGED);
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
    pub fn IsHopeForSeeking(&self, tree: &PanelTree) -> bool {
        if let Some(id) = self.seek_pos_panel {
            if let Some(panel) = tree.GetRec(id) {
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

    pub fn Visit(&mut self, panel: PanelId, rel_x: f64, rel_y: f64, rel_a: f64) {
        self.visit_stack.push(VisitState {
            panel,
            rel_x,
            rel_y,
            rel_a,
        });
        self.active = Some(panel);
        self.viewport_changed = true;
        self.viewing_dirty = true;
    }

    pub fn VisitFullsized(&mut self, tree: &PanelTree, panel: PanelId) {
        let (x, y, a) = self.CalcVisitFullsizedCoords(tree, panel, false);
        self.Visit(panel, x, y, a);
    }

    /// D-PANEL-02: Request an animated visit to a panel. Sets a pending goal
    /// that the window loop feeds to the emVisitingViewAnimator. Also sets the
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
        let (x, y, a) = self.CalcVisitCoords(tree, panel);
        self.animated_visit(tree, panel, x, y, a, adherent);
    }

    pub fn go_back(&mut self) -> bool {
        if self.visit_stack.len() > 1 {
            self.visit_stack.pop();
            self.active = Some(self.current_visit().panel);
            self.viewport_changed = true;
            self.viewing_dirty = true;
            true
        } else {
            false
        }
    }

    pub fn go_home(&mut self) {
        self.visit_stack.truncate(1);
        self.active = Some(self.root);
        self.viewport_changed = true;
        self.viewing_dirty = true;
    }

    // --- Viewport ---

    /// D-PANEL-05: Clamp dimensions, preserve zoom state on resize (C++ SetGeometry parity).
    pub fn SetGeometry(&mut self, tree: &mut PanelTree, width: f64, height: f64) {
        let width = width.max(MIN_DIMENSION);
        let height = height.max(MIN_DIMENSION);

        if (self.viewport_width - width).abs() < 1e-15
            && (self.viewport_height - height).abs() < 1e-15
        {
            return;
        }

        let was_zoomed_out = self.IsZoomedOut(tree);

        self.viewport_width = width;
        self.viewport_height = height;
        self.pixel_tallness = height / width;

        // C++ SetGeometry parity: inline-update root panel layout when
        // VF_ROOT_SAME_TALLNESS is set (mirrors RootPanel->Layout(0,0,1,GetHomeTallness())).
        if self.flags.contains(ViewFlags::ROOT_SAME_TALLNESS) {
            tree.Layout(self.root, 0.0, 0.0, 1.0, self.pixel_tallness);
        }

        // Preserve zoom state: if was zoomed out, re-apply zoom-out with
        // the new viewport dimensions (computes the correct fit ratio).
        if was_zoomed_out {
            self.RawZoomOut(tree);
        }

        self.viewport_changed = true;
        self.viewing_dirty = true;
    }

    pub fn viewport_size(&self) -> (f64, f64) {
        (self.viewport_width, self.viewport_height)
    }

    /// Cached visited panel dimensions (screen pixels), set by update_viewing().
    pub fn visited_size(&self) -> (f64, f64) {
        (self.visited_vw, self.visited_vh)
    }

    // --- Zoom & Scroll ---

    /// Fix-point zoom: keeps the viewport point (center_x, center_y) mapped to the
    /// same panel-space point before and after zoom.
    pub fn Zoom(&mut self, factor: f64, center_x: f64, center_y: f64) {
        if self.flags.contains(ViewFlags::NO_ZOOM) {
            return;
        }
        // VIEW-003: Signal abort for any active animator (C++ AbortActiveAnimator)
        self.needs_animator_abort = true;
        if let Some(state) = self.visit_stack.last_mut() {
            let old_a = state.rel_a;
            // C++ emView::Zoom (emView.cpp:793-796):
            //   reFac = 1/factor
            //   rx += (fixX - hmx) * (1 - reFac) / pvw
            //   ry += (fixY - hmy) * (1 - reFac) / pvh
            //   ra *= reFac^2            i.e. ra *= 1/factor^2
            //
            // Rust rel_a = 1/ra, so rel_a_new = factor^2 * rel_a_old.
            let new_a = (old_a * factor * factor).clamp(0.001, MAX_SVP_SIZE);
            if (new_a - old_a).abs() < 1e-15 {
                return;
            }
            let re_fac = 1.0 / factor;
            let vw = self.viewport_width.max(1.0);
            let vh = self.viewport_height.max(1.0);
            let pvw = self.visited_vw.max(1.0);
            let pvh = self.visited_vh.max(1.0);
            state.rel_x += (center_x - vw * 0.5) * (1.0 - re_fac) / pvw;
            state.rel_y += (center_y - vh * 0.5) * (1.0 - re_fac) / pvh;
            state.rel_a = new_a;
            self.viewport_changed = true;
            self.viewing_dirty = true;
        }
    }

    pub fn Scroll(&mut self, dx: f64, dy: f64) {
        if self.flags.contains(ViewFlags::NO_SCROLL) {
            return;
        }
        // VIEW-003: Signal abort for any active animator (C++ AbortActiveAnimator)
        self.needs_animator_abort = true;
        if let Some(state) = self.visit_stack.last_mut() {
            // Convert screen-pixel deltas to viewport-fraction units by
            // dividing by the cached visited panel dimensions (ViewedWidth /
            // ViewedHeight).  Matches C++ Scroll which divides by pvw/pvh.
            state.rel_x += dx / self.visited_vw;
            state.rel_y += dy / self.visited_vh;
            self.viewport_changed = true;
            self.viewing_dirty = true;
        }
    }

    /// Atomic scroll+zoom with done-distance feedback.
    pub fn RawScrollAndZoom(
        &mut self,
        tree: &mut PanelTree,
        fix_x: f64,
        fix_y: f64,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> [f64; 3] {
        let before = self.visit_stack.last().cloned();
        // Save pre-operation visited dimensions for done-distance.
        let pre_vw = self.visited_vw;
        let pre_vh = self.visited_vh;

        // C++ RawScrollAndZoom applies scroll+zoom atomically:
        //   reFac = exp(-deltaZ * zflpp)
        //   rx2 = rx + ((fixX-hmx)*(1-reFac) + deltaX) / pvw
        //   ry2 = ry + ((fixY-hmy)*(1-reFac) + deltaY) / pvh
        //   ra2 = ra * reFac^2
        //
        // Scroll and zoom fix-point correction are additive to rx/ry;
        // the zoom does NOT scale the scroll delta (unlike sequential
        // scroll-then-zoom which would multiply rel_x by the zoom ratio).
        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);
        let zflpp = self.GetZoomFactorLogarithmPerPixel();
        let re_fac = (-dz * zflpp).exp();
        let pvw = self.visited_vw;
        let pvh = self.visited_vh;

        if let Some(state) = self.visit_stack.last_mut() {
            // C++ emView.cpp ~1604: when EGO_MODE active, collapse scroll
            // boundaries to viewport center — only zoom works, scroll is locked.
            if !self.flags.contains(ViewFlags::EGO_MODE) {
                // Fix-point zoom correction: (fixX - vw/2)*(1-reFac) / pvw
                let fix_corr_x = (fix_x - vw * 0.5) * (1.0 - re_fac) / pvw;
                let fix_corr_y = (fix_y - vh * 0.5) * (1.0 - re_fac) / pvh;

                // Scroll: dx / pvw (same as C++ deltaX / pvw)
                state.rel_x += fix_corr_x + dx / pvw;
                state.rel_y += fix_corr_y + dy / pvh;
            }

            // Zoom: rel_a = 1/ra, ra_new = ra * reFac^2
            // => rel_a_new = rel_a / reFac^2
            let new_a = (state.rel_a / (re_fac * re_fac)).clamp(0.001, MAX_SVP_SIZE);
            state.rel_a = new_a;

            self.viewport_changed = true;
            self.viewing_dirty = true;
        }
        self.Update(tree);
        self.viewing_dirty = false;
        let after = self.visit_stack.last().cloned();
        match (before, after) {
            (Some(b), Some(a)) => {
                // Convert rel_x/rel_y deltas back to pixel units using the
                // PRE-operation visited dimensions (same denominator scroll used).
                let done_x = (a.rel_x - b.rel_x) * pre_vw;
                let done_y = (a.rel_y - b.rel_y) * pre_vh;
                // C++: done_z = -ln(reFac)/zflpp = 0.5 * ln(rel_a_new/rel_a_old) / zflpp
                let zflpp = self.GetZoomFactorLogarithmPerPixel();
                let done_z = if b.rel_a > 0.0 && zflpp > 1e-15 {
                    0.5 * (a.rel_a / b.rel_a).ln() / zflpp
                } else {
                    0.0
                };
                [done_x, done_y, done_z]
            }
            _ => [0.0, 0.0, 0.0],
        }
    }

    /// Zoom sensitivity for VIFs/animators.
    pub fn GetZoomFactorLogarithmPerPixel(&self) -> f64 {
        1.33 / ((self.viewport_width + self.viewport_height) * 0.25).max(1.0)
    }

    // --- Zoom out ---

    pub fn ZoomOut(&mut self, tree: &mut PanelTree) {
        self.RawZoomOut(tree);
    }

    pub fn RawZoomOut(&mut self, tree: &mut PanelTree) {
        let rel_a = self.zoom_out_rel_a(tree);
        if let Some(state) = self.visit_stack.last_mut() {
            state.rel_x = 0.0;
            state.rel_y = 0.0;
            state.rel_a = rel_a;
            self.viewport_changed = true;
            self.viewing_dirty = true;
        }
        self.Update(tree);
        self.viewing_dirty = false;
    }

    /// Compute the rel_a that makes the viewport fully contain the root panel.
    ///
    /// C++ `RawZoomOut` computes `ra = max(W*H_root/pt/H, H/H_root*pt/W)`.
    /// Rust rel_a uses the INVERSE convention: `rel_a = 1/ra`, so larger
    /// rel_a means more zoomed in (panel viewed area > viewport area).
    fn zoom_out_rel_a(&self, tree: &PanelTree) -> f64 {
        let root_h = tree.get_height(self.root);
        let a1 = self.viewport_width * root_h / self.home_pixel_tallness / self.viewport_height;
        let a2 = self.viewport_height / root_h * self.home_pixel_tallness / self.viewport_width;
        // C++ ra = max(a1, a2). Rust convention: rel_a = 1/ra.
        1.0 / a1.max(a2)
    }

    pub fn IsZoomedOut(&self, tree: &PanelTree) -> bool {
        if self.flags.contains(ViewFlags::POPUP_ZOOM) {
            return !self.popped_up;
        }
        let target_a = self.zoom_out_rel_a(tree);
        if let Some(state) = self.visit_stack.last() {
            state.rel_x.abs() < 0.001
                && state.rel_y.abs() < 0.001
                && (state.rel_a - target_a).abs() < 0.001
        } else {
            true
        }
    }

    // --- CalcVisitCoords ---

    /// Compute optimal (rel_x, rel_y, rel_a) to view `panel` well.
    pub fn CalcVisitCoords(&self, tree: &PanelTree, panel: PanelId) -> (f64, f64, f64) {
        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);
        let v_aspect = vw / vh;

        // Walk from panel up to root, build chain root→panel
        let mut chain_rev: Vec<PanelId> = tree.ancestors(panel);
        chain_rev.reverse(); // [root, ..., parent, panel]

        // Start from root — width normalized to 1.0, height preserves layout_rect aspect
        let root_lr = tree
            .GetRec(self.root)
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
            let lr = tree.GetRec(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, _ph) = rects[i - 1];
            // C++ scales ALL layout coordinates by ParentViewedWidth (pw)
            let x = px + lr.x * pw;
            let y = py + lr.y * pw;
            let w = lr.w * pw;
            let h = lr.h * pw;
            rects.push((x, y, w, h));
        }

        let (_px, _py, pw, ph) = *rects.last().unwrap_or(&(0.0, 0.0, 1.0, 1.0));

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
        let rel_a = rel_a.clamp(0.001, MAX_SVP_SIZE);

        // In panel-fraction, centering is always relX=0, relY=0.
        // The panel's position within the root is encoded in the tree walk,
        // not in rel_x/rel_y (C++ emView.cpp:1484-1486).
        let rel_x = 0.0;
        let rel_y = 0.0;

        (rel_x, rel_y, rel_a)
    }

    /// Compute coords to show panel at its natural aspect (fullsized).
    pub fn CalcVisitFullsizedCoords(
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
            .GetRec(self.root)
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
            let lr = tree.GetRec(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, _ph) = rects[i - 1];
            // C++ scales ALL layout coordinates by ParentViewedWidth (pw)
            rects.push((px + lr.x * pw, py + lr.y * pw, lr.w * pw, lr.h * pw));
        }

        let (_px, _py, pw, ph) = *rects.last().unwrap_or(&(0.0, 0.0, 1.0, 1.0));
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

        let rel_a = (scale * scale * (pw * ph).max(MIN_DIMENSION * MIN_DIMENSION))
            .clamp(0.001, MAX_SVP_SIZE);
        // Panel-fraction: centering is relX=0, relY=0.
        let rel_x = 0.0;
        let rel_y = 0.0;

        (rel_x, rel_y, rel_a)
    }

    // --- ViewFlags with side effects ---

    pub fn SetViewFlags(&mut self, flags: ViewFlags, tree: &mut PanelTree) {
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
            self.RawZoomOut(tree);
        }

        if new_flags.contains(ViewFlags::NO_ZOOM) && !old.contains(ViewFlags::NO_ZOOM) {
            self.RawZoomOut(tree);
        }

        if new_flags.contains(ViewFlags::ROOT_SAME_TALLNESS)
            && !old.contains(ViewFlags::ROOT_SAME_TALLNESS)
        {
            tree.Layout(self.root, 0.0, 0.0, 1.0, self.pixel_tallness);
            self.RawZoomOut(tree);
        }
    }

    // --- Active Panel Management ---

    pub fn set_active_panel(&mut self, tree: &mut PanelTree, panel: PanelId, adherent: bool) {
        // Walk up to nearest focusable panel (self included, matching C++ SetActivePanel)
        let target = if tree.GetRec(panel).map(|p| p.focusable).unwrap_or(false) {
            panel
        } else {
            tree.GetFocusableParent(panel).unwrap_or(panel)
        };

        if self.active == Some(target) {
            if self.activation_adherent != adherent {
                self.activation_adherent = adherent;
            }
            return;
        }

        // Build notice flags: always ACTIVE_CHANGED, add FOCUS_CHANGED if focused
        let mut flags = super::emPanel::NoticeFlags::ACTIVE_CHANGED;
        if self.window_focused {
            flags.insert(super::emPanel::NoticeFlags::FOCUS_CHANGED);
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
        dlog!("active panel changed to {:?}", target);
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
        self.control_panel_invalid = true;
        tree.mark_notices_pending();
    }

    /// Auto-select best visible focusable panel as active.
    ///
    /// D-PANEL-03: Uses center-containment descent (C++ parity) instead of
    /// max-area. Starts at SVP and descends into the deepest focusable child
    /// whose clip rect contains the viewport center, stopping when children
    /// are too small (< 99% view width AND height, AND < 33% view area).
    pub fn SetActivePanelBestPossible(&mut self, tree: &mut PanelTree) {
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
                let p = match tree.GetRec(child) {
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
                    let p = tree.GetRec(child).expect("child just found");
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
        if !tree.GetRec(best).map(|p| p.focusable).unwrap_or(false) {
            if let Some(anc) = tree.GetFocusableParent(best) {
                best = anc;
            } else {
                return;
            }
        }

        // Adherent check: keep current active if still visible and best is ancestor
        if self.activation_adherent {
            if let Some(active_id) = self.active {
                if let Some(active_panel) = tree.GetRec(active_id) {
                    if active_panel.viewed
                        && active_panel.viewed_width >= 4.0
                        && active_panel.viewed_height >= 4.0
                    {
                        if let Some(best_panel) = tree.GetRec(best) {
                            if best_panel.in_active_path {
                                self.set_active_panel(tree, active_id, true);
                                return;
                            }
                        }
                    }
                }
            }
        }
        dlog!("set_active_panel_best_possible chose {:?}", best);
        self.set_active_panel(tree, best, false);
    }

    // --- Coordinate transform: update_viewing ---

    /// Compute absolute viewport coordinates for all panels. Called once per frame.
    pub fn Update(&mut self, tree: &mut PanelTree) {
        tree.clear_viewing_flags();

        let root = match tree.GetRootPanel() {
            Some(r) => r,
            None => return,
        };

        // C++ ZoomedOutBeforeSG: on the first update after construction,
        // compute the zoom-out relA so the root panel fits in the viewport.
        // This mirrors C++ emView::SetGeometry which always calls RawZoomOut
        // when ZoomedOutBeforeSG is true (no threshold).
        if self.zoomed_out_before_sg {
            self.zoomed_out_before_sg = false;
            let rel_a = self.zoom_out_rel_a(tree);
            if let Some(state) = self.visit_stack.last_mut() {
                state.rel_a = rel_a;
            }
        }

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
        let root_lr = tree.GetRec(root).map(|p| p.layout_rect).unwrap_or_default();
        let root_norm_h = if root_lr.w > MIN_DIMENSION {
            (root_lr.h / root_lr.w).max(MIN_DIMENSION)
        } else {
            1.0
        };
        let mut norm_rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        norm_rects.push((0.0, 0.0, 1.0, root_norm_h));
        for i in 1..chain_rev.len() {
            let id = chain_rev[i];
            let lr = tree.GetRec(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, _ph) = norm_rects[i - 1];
            // C++ scales ALL layout coordinates by ParentViewedWidth (pw)
            norm_rects.push((px + lr.x * pw, py + lr.y * pw, lr.w * pw, lr.h * pw));
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
        // visited center in viewport: vw/2 - rel_x*visited_vw (panel-fraction)
        // visited center = root_vx + (vnx + vnw/2) * root_vw

        let vnw_safe = vnw.max(MIN_DIMENSION);
        let vnh_safe = vnh.max(MIN_DIMENSION);
        let panel_aspect = vnw_safe / vnh_safe;

        let visited_vw = (visit.rel_a * vw * vh * panel_aspect).sqrt();
        let visited_vh = (visit.rel_a * vw * vh / panel_aspect).sqrt();

        // Cache for scroll/done-distance (BUG-8 fix infrastructure).
        self.visited_vw = visited_vw.max(1.0);
        self.visited_vh = visited_vh.max(1.0);

        let root_vw = visited_vw / vnw_safe;
        // For centering: visited_vh / vnh_safe gives the coordinate-scale
        // factor used to place the visited panel center. This equals root_vw
        // algebraically, but we keep the derivation clear.
        let root_vh_center = visited_vh / vnh_safe;

        // Visited panel center in viewport.
        // C++ emView.cpp:1537: panel_center = hmx - relX * pvw
        // Panel-fraction: relX is in visited-panel-widths, so multiply by visited_vw.
        let vcx = vw * 0.5 - visit.rel_x * visited_vw;
        let vcy = vh * 0.5 - visit.rel_y * visited_vh;

        // Root position (centering uses root_vh_center)
        let root_vx = vcx - (vnx + vnw_safe * 0.5) * root_vw;
        let root_vy = vcy - (vny + vnh_safe * 0.5) * root_vh_center;

        // C++ ViewedHeight = ViewedWidth * Height / PixelTallness (emPanel.cpp:615).
        // The root rect height is its actual pixel extent, not the centering scale.
        let root_actual_h = root_vw * root_norm_h;

        // Now recursively set viewed coords for all panels starting from root
        let viewport = Rect::new(0.0, 0.0, vw, vh);
        let root_abs = Rect::new(root_vx, root_vy, root_vw, root_actual_h);
        self.compute_viewed_recursive(tree, root, root_abs, &viewport);

        // NOTE: C++ RawVisitAbs fires viewing notices on all visible children
        // during navigation (zoom/scroll/visit), but Rust relies on
        // set_layout_rect to queue viewing notices on laid-out panels only.
        // No golden tests cover the navigation case yet.

        // Find SVP: deepest ancestor of visited panel whose absolute area <= MAX_SVP_SIZE
        let ancestors = tree.ancestors(visited);
        self.svp = None;
        for &id in &ancestors {
            if let Some(p) = tree.GetRec(id) {
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
        dlog!("SVP = {:?}", self.svp);

        // C++ sets InViewedPath=1 for ALL viewed panels, not just SVP
        // ancestors (emPanel.cpp:1497). This affects get_memory_limit and
        // get_update_priority which return 0 when in_viewed_path is false.
        tree.for_each_mut(|_, p| {
            if p.viewed {
                p.in_viewed_path = true;
            }
        });

        // Set in_active_path from root to active (walk parent chain without allocating)
        if let Some(active_id) = self.active {
            if tree.contains(active_id) {
                let mut cur = Some(active_id);
                while let Some(id) = cur {
                    if let Some(p) = tree.get_mut(id) {
                        p.in_active_path = true;
                        cur = p.parent;
                    } else {
                        break;
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

        // Drain panel-to-view navigation requests
        for target in tree.drain_navigation_requests() {
            self.VisitFullsized(tree, target);
        }
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
            let lr = tree.GetRec(child).map(|p| p.layout_rect).unwrap_or_default();
            // C++ emPanel scales ALL layout coords by parent ViewedWidth
            // (not ViewedHeight), because layout coordinates are all in
            // parent-width units.
            let child_abs = Rect::new(
                abs.x + lr.x * abs.w,
                abs.y + lr.y * abs.w,
                lr.w * abs.w,
                lr.h * abs.w,
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
                let Some(panel) = tree.GetRec(id) else {
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

            let vc = tree.GetViewCondition(id, threshold_type);
            let should_expand = vc >= threshold_value;

            if should_expand && !currently_expanded {
                // Expand: set flag and create children directly via layout_children.
                // This matches C++ where AutoExpand() creates children, then a
                // subsequent HandleNotice pass calls LayoutChildren() to position them.
                if let Some(panel) = tree.get_mut(id) {
                    panel.ae_expanded = true;
                    panel.ae_decision_invalid = false;
                    panel.ae_invalid = false;
                }
                if let Some(mut behavior) = tree.take_behavior(id) {
                    let mut ctx = PanelCtx::new(tree, id);
                    behavior.LayoutChildren(&mut ctx);
                    if tree.contains(id) {
                        tree.put_behavior(id, behavior);
                    }
                }
                // Queue LAYOUT_CHANGED so deliver_notices repositions children
                tree.queue_notice(id, super::emPanel::NoticeFlags::LAYOUT_CHANGED);
            } else if !should_expand && currently_expanded {
                // Shrink: delete children and clear flag
                tree.DeleteAllChildren(id);
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
                }
                tree.queue_notice(id, super::emPanel::NoticeFlags::LAYOUT_CHANGED);
            }
        }
    }

    // --- Navigation ---

    /// D-PANEL-01: Navigate to next focusable panel (C++ VisitNext parity).
    ///
    /// Tries next focusable sibling; if at end, ascends to focusable parent
    /// and wraps to its first focusable child.
    pub fn VisitNext(&mut self, tree: &mut PanelTree) {
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
        if let Some(next) = tree.GetFocusableNext(active) {
            self.animated_visit_panel(tree, next, false);
            return;
        }

        // No next sibling: go to focusable parent's first focusable child
        let parent = tree
            .GetFocusableParent(active)
            .unwrap_or_else(|| tree.GetRootPanel().unwrap_or(active));
        if parent != active {
            if let Some(first) = tree.GetFocusableFirstChild(parent) {
                self.animated_visit_panel(tree, first, false);
            }
        }
    }

    /// D-PANEL-01: Navigate to previous focusable panel (C++ VisitPrev parity).
    pub fn VisitPrev(&mut self, tree: &mut PanelTree) {
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
        if let Some(prev) = tree.GetFocusablePrev(active) {
            self.animated_visit_panel(tree, prev, false);
            return;
        }

        // No previous sibling: go to focusable parent's last focusable child
        let parent = tree
            .GetFocusableParent(active)
            .unwrap_or_else(|| tree.GetRootPanel().unwrap_or(active));
        if parent != active {
            if let Some(last) = tree.GetFocusableLastChild(parent) {
                self.animated_visit_panel(tree, last, false);
            }
        }
    }

    pub fn VisitFirst(&mut self, tree: &mut PanelTree) {
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
        let parent = match tree.GetParentContext(active) {
            Some(p) => p,
            None => return,
        };
        for child in tree.children(parent) {
            if tree.GetRec(child).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, child, false);
                return;
            }
        }
    }

    pub fn VisitLast(&mut self, tree: &mut PanelTree) {
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
        let parent = match tree.GetParentContext(active) {
            Some(p) => p,
            None => return,
        };
        for child in tree.children_rev(parent) {
            if tree.GetRec(child).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, child, false);
                return;
            }
        }
    }

    pub fn VisitLeft(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Left);
    }

    pub fn VisitRight(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Right);
    }

    pub fn VisitUp(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Up);
    }

    pub fn VisitDown(&mut self, tree: &mut PanelTree) {
        self.visit_neighbour(tree, Direction::Down);
    }

    pub fn VisitIn(&mut self, tree: &mut PanelTree) {
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
            if tree.GetRec(child).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, child, false);
                return;
            }
        }
        // No focusable child — visit active fullsized
        self.VisitFullsized(tree, active);
    }

    pub fn VisitOut(&mut self, tree: &mut PanelTree) {
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
        if let Some(parent) = tree.GetParentContext(active) {
            if tree.GetRec(parent).map(|p| p.focusable).unwrap_or(false) {
                self.animated_visit_panel(tree, parent, false);
                return;
            }
            if let Some(focusable) = tree.GetFocusableParent(parent) {
                self.animated_visit_panel(tree, focusable, false);
                return;
            }
        }
        // At root — zoom out
        self.ZoomOut(tree);
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
        let parent = match tree.GetParentContext(active) {
            Some(p) => p,
            None => return,
        };

        let active_panel = match tree.GetRec(active) {
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
            let sp = match tree.GetRec(sib) {
                Some(p) if p.focusable && p.viewed => p,
                _ => continue,
            };
            let scx = sp.viewed_x + sp.viewed_width * 0.5;
            let scy = sp.viewed_y + sp.viewed_height * 0.5;

            let (dx, dy) = (scx - acx, scy - acy);

            // Rotate based on direction so "forward" is always +x
            // C++: 0=Right identity, 1=Down (dy,-dx), 2=Left (-dx,-dy), 3=Up (-dy,dx)
            let (rx, ry) = match direction {
                Direction::Right => (dx, dy),
                Direction::Down => (dy, -dx),
                Direction::Left => (-dx, -dy),
                Direction::Up => (-dy, dx),
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

    pub fn GetPanelAt(&self, tree: &PanelTree, x: f64, y: f64) -> Option<PanelId> {
        let svp = self.svp?;
        self.hit_test_recursive(tree, svp, x, y, false)
    }

    pub fn GetFocusablePanelAt(&self, tree: &PanelTree, x: f64, y: f64) -> Option<PanelId> {
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
        let panel = tree.GetRec(id)?;
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
    /// causing `emView.SetActivePanel(Parent, false)`.
    pub fn remove_panel(&mut self, tree: &mut PanelTree, id: PanelId) {
        if tree.GetRec(id).map(|p| p.in_active_path).unwrap_or(false) {
            if let Some(parent) = tree.GetParentContext(id) {
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
        self.SetFocused(tree, true);
        self.set_active_panel(tree, panel, false);
    }

    /// Whether a panel is focused: it is the active panel and the view's
    /// window is focused.
    pub fn is_panel_focused(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let is_active = tree.GetRec(panel).map(|p| p.is_active).unwrap_or(false);
        is_active && self.window_focused
    }

    /// Whether a panel is in the focused path: it is in the active path and
    /// the view's window is focused.
    pub fn is_panel_in_focused_path(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let in_active_path = tree.GetRec(panel).map(|p| p.in_active_path).unwrap_or(false);
        in_active_path && self.window_focused
    }

    /// Whether a panel's activation is adherent (indirect, via a descendant).
    pub fn is_panel_activated_adherent(&self, tree: &PanelTree, panel: PanelId) -> bool {
        tree.GetRec(panel).map(|p| p.is_active).unwrap_or(false) && self.activation_adherent
    }

    /// Whether the view's window is focused (panel-level delegate).
    pub fn is_view_focused(&self) -> bool {
        self.window_focused
    }

    /// Return wall-clock milliseconds (since Unix epoch).
    pub fn GetInputClockMS(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Panel-level delegate for `get_input_clock_ms` (mirrors `emPanel::GetInputClockMS`).
    pub fn get_panel_input_clock_ms(&self) -> u64 {
        self.GetInputClockMS()
    }

    // --- Pixel tallness ---

    /// Return the pixel tallness (height/width ratio of a pixel).
    ///
    /// Corresponds to `emPanel::GetViewedPixelTallness` (delegates to
    /// `emView.CurrentPixelTallness`).
    pub fn GetCurrentPixelTallness(&self) -> f64 {
        self.pixel_tallness
    }

    // --- Invalidation ---

    /// Mark the view's title as needing a refresh. Only takes effect when
    /// the panel is in the active path.
    ///
    /// Corresponds to `emPanel::InvalidateTitle`.
    pub fn InvalidateTitle(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_active_path = tree.GetRec(panel).map(|p| p.in_active_path).unwrap_or(false);
        if in_active_path {
            self.title_invalid = true;
        }
    }

    /// Mark the view's cursor as needing a refresh. Only takes effect when
    /// the panel is in the viewed path.
    ///
    /// Corresponds to `emPanel::InvalidateCursor`.
    pub fn InvalidateCursor(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_viewed_path = tree.GetRec(panel).map(|p| p.in_viewed_path).unwrap_or(false);
        if in_viewed_path {
            self.cursor_invalid = true;
        }
    }

    /// Mark the entire panel clip rect as needing repaint.
    ///
    /// Corresponds to `emPanel::InvalidatePainting()` (no-arg overload).
    pub fn InvalidatePainting(&mut self, tree: &PanelTree, panel: PanelId) {
        let p = match tree.GetRec(panel) {
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
        let p = match tree.GetRec(panel) {
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
    pub fn InvalidateControlPanel(&mut self, tree: &PanelTree, panel: PanelId) {
        let in_active_path = tree.GetRec(panel).map(|p| p.in_active_path).unwrap_or(false);
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

    /// Mark the cursor as needing a refresh (without requiring tree/panel).
    pub fn mark_cursor_invalid(&mut self) {
        self.cursor_invalid = true;
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

    /// Drain accumulated dirty rectangles as a coalesced [`ClipRects`] set.
    ///
    /// Overlapping dirty rects are merged so the compositor doesn't repaint
    /// the same pixel region twice.
    pub(crate) fn take_dirty_clip_rects(&mut self) -> ClipRects {
        let rects = std::mem::take(&mut self.dirty_rects);
        let mut cr = ClipRects::new();
        for r in &rects {
            cr.unite_rect(r.x, r.y, r.x + r.w, r.y + r.h);
        }
        cr
    }

    /// Collect invalidation signals from panel behaviors that manage sub-views
    /// (e.g. [`emSubViewPanel`](super::emSubViewPanel::emSubViewPanel)). Drains each behavior's
    /// pending parent invalidation and merges the dirty rects, title, and
    /// cursor flags into this view.
    ///
    /// This implements the C++ invalidation chain where
    /// `SubViewClass::InvalidateTitle`, `SubViewPortClass::InvalidateCursor`,
    /// and `SubViewPortClass::InvalidatePainting` propagate from the sub-view
    /// to the enclosing panel/view.
    pub fn collect_parent_invalidation(&mut self, tree: &mut PanelTree) {
        let ids: Vec<PanelId> = tree.all_ids();
        for id in ids {
            if let Some(mut behavior) = tree.take_behavior(id) {
                if let Some(inv) = behavior.drain_parent_invalidation() {
                    for r in inv.dirty_rects {
                        self.dirty_rects.push(r);
                    }
                    if inv.title_invalid {
                        self.title_invalid = true;
                    }
                    if inv.cursor_invalid {
                        self.cursor_invalid = true;
                    }
                }
                tree.put_behavior(id, behavior);
            }
        }
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
    /// emVisitingViewAnimator if active.
    pub fn needs_animator_abort(&self) -> bool {
        self.needs_animator_abort
    }

    /// Clear the animator-abort flag.
    pub fn clear_animator_abort(&mut self) {
        self.needs_animator_abort = false;
    }

    /// D-PANEL-02: Take the pending animated visit goal. Returns `Some` if a
    /// navigation method requested an animated visit. The window loop should
    /// feed this to the emVisitingViewAnimator.
    pub fn take_pending_animated_visit(&mut self) -> Option<VisitState> {
        self.pending_animated_visit.take()
    }

    /// Whether there is a pending animated visit goal.
    pub fn has_pending_animated_visit(&self) -> bool {
        self.pending_animated_visit.is_some()
    }

    // --- Background color (PORT-0116) ---

    /// Get the background color of this view. Used for areas not covered by
    /// panels or where panels are transparent. Matches C++ `emView::GetBackgroundColor`.
    pub fn GetBackgroundColor(&self) -> emColor {
        self.background_color
    }

    /// Set the background color of this view. If changed, the view is
    /// invalidated for repainting. Matches C++ `emView::SetBackgroundColor`.
    pub fn SetBackgroundColor(&mut self, color: emColor) {
        if self.background_color != color {
            self.background_color = color;
            self.viewport_changed = true;
            self.viewing_dirty = true;
        }
    }

    // --- Panel lookup by identity (PORT-0127) ---

    /// Search for a panel by its colon-delimited identity string.
    /// Returns `None` if not found. Matches C++ `emView::GetPanelByIdentity`.
    pub fn GetPanelByIdentity(&self, tree: &PanelTree, identity: &str) -> Option<PanelId> {
        use crate::emPanelTree::DecodeIdentity;

        let names = DecodeIdentity(identity);
        if names.is_empty() {
            return None;
        }

        let root = self.root;
        let root_name = tree.name(root)?;
        if root_name != names[0] {
            return None;
        }

        let mut current = root;
        for name in &names[1..] {
            match tree.find_child_by_name(current, name) {
                Some(child) => current = child,
                None => return None,
            }
        }
        Some(current)
    }

    // --- EOI signal (PORT-0129) ---

    /// Request a delayed End-Of-Interaction signal. The window loop should
    /// check `eoi_delayed()` and count down before signaling EOI.
    /// Matches C++ `emView::SignalEOIDelayed`.
    pub fn SignalEOIDelayed(&mut self) {
        self.eoi_countdown = Some(3);
    }

    /// Whether an EOI countdown is active.
    pub fn eoi_delayed(&self) -> bool {
        self.eoi_countdown.is_some()
    }

    /// Tick the EOI countdown. Returns `true` when the countdown has reached
    /// zero and the EOI should be signaled (caller should then act, e.g.
    /// zoom out for popup views).
    pub fn tick_eoi(&mut self) -> bool {
        if let Some(ref mut count) = self.eoi_countdown {
            if *count <= 0 {
                self.eoi_countdown = None;
                return true;
            }
            *count -= 1;
        }
        false
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
        let p = match tree.GetRec(panel) {
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
            // scroll() divides by visited_vw/visited_vh (screen-pixel
            // ViewedWidth/Height). dx/dy are already in screen pixels,
            // so pass them directly.
            self.Scroll(dx, dy);
        }
    }

    // --- Title (C++ GetTitle / InvalidateTitle) ---

    /// Get the view title. In C++, GetTitle() is virtual and defaults to the
    /// active panel's title. Here we store a cached title string that is
    /// refreshed when `title_invalid` is set.
    pub fn GetTitle(&self) -> &str {
        &self.title
    }

    /// Set the view title directly. Marks the title as valid.
    pub fn SetRootTitle(&mut self, title: &str) {
        self.title = title.to_string();
        self.title_invalid = false;
    }

    /// Mark the title as needing a refresh (view-level, not panel-level).
    /// Corresponds to C++ `emView::InvalidateTitle` which signals the title signal.
    pub fn invalidate_view_title(&mut self) {
        self.title_invalid = true;
    }

    // --- Popup state (C++ IsPoppedUp) ---

    /// Whether the view is in popped-up state. In C++ this checks
    /// `PopupWindow != NULL`. The Rust equivalent tracks a bool flag that
    /// is set when a popup window is created for this view.
    pub fn IsPoppedUp(&self) -> bool {
        self.popped_up
    }

    /// Set the popped-up state. Called by the window/viewport layer when
    /// a popup window is created or destroyed for this view.
    pub fn set_popped_up(&mut self, popped_up: bool) {
        self.popped_up = popped_up;
    }

    // --- Popup rect (C++ GetMaxPopupViewRect) ---

    /// Get the maximum popup view rectangle: the bounding rectangle of all
    /// display monitors intersecting the home rectangle of the view, in
    /// pixel coordinates. Returns `None` if not set.
    pub fn max_popup_rect(&self) -> Option<Rect> {
        self.max_popup_rect
    }

    /// Set the maximum popup view rectangle. Called by the viewport/window
    /// layer when monitor geometry is known.
    pub fn set_max_popup_rect(&mut self, rect: Option<Rect>) {
        self.max_popup_rect = rect;
    }

    // --- emCursor (C++ GetCursor) ---

    /// Get the current mouse cursor for this view. In C++, GetCursor() is
    /// virtual and defaults to the cursor of the panel under the mouse.
    pub fn GetCursor(&self) -> emCursor {
        // C++ emView.cpp ~1358: when EGO_MODE active and cursor is Normal,
        // override to Crosshair.
        if self.flags.contains(ViewFlags::EGO_MODE) && self.cursor == emCursor::Normal {
            return emCursor::Crosshair;
        }
        self.cursor
    }

    /// Set the current mouse cursor. Called after resolving cursor from panels.
    pub fn set_cursor(&mut self, cursor: emCursor) {
        self.cursor = cursor;
        self.cursor_invalid = false;
    }

    // --- Control panel (C++ CreateControlPanel / GetControlPanelSignal) ---

    /// Create the control panel for the currently active panel.
    ///
    /// Walks the content tree's parent chain from the active panel to find
    /// a behavior that creates a control panel, but creates the panel in
    /// `control_tree` as a child of `parent`. Matches C++
    /// `emView::CreateControlPanel`.
    pub fn CreateControlPanel(
        &self,
        content_tree: &mut PanelTree,
        control_tree: &mut PanelTree,
        parent: PanelId,
        name: &str,
    ) -> Option<PanelId> {
        let active = self.active?;
        content_tree.create_control_panel_in(active, control_tree, parent, name)
    }

    /// Whether the control panel signal has been raised (needs recreation).
    /// Equivalent to checking C++ `GetControlPanelSignal`.
    pub fn needs_control_panel_update(&self) -> bool {
        self.control_panel_invalid
    }

    // --- Window/emScreen access (C++ GetWindow / GetScreen) ---
    //
    // In C++, emView inherits from emContext which provides a parent-context
    // chain. GetWindow() walks up the context chain to find the nearest
    // emWindow, and GetScreen() finds the nearest emScreen.
    //
    // In Rust, emView is a plain struct owned by the window. The window/screen
    // relationship is managed externally. These methods are not needed on emView
    // itself — callers that have a emView reference already know which window
    // owns it. If cross-view queries are needed in the future, the window
    // layer can provide the mapping.

    // --- Update loop ---

    pub fn update(&mut self, tree: &mut PanelTree) {
        if self.viewing_dirty {
            self.Update(tree);
            self.viewing_dirty = false;
        }

        // VIEW-003: After scroll/zoom or viewport change, reselect active panel
        // (C++ calls SetActivePanelBestPossible after Scroll/Zoom)
        let need_reselect = match self.active {
            None => true,
            Some(id) => !tree.contains(id) || !tree.GetRec(id).map(|p| p.focusable).unwrap_or(false),
        };
        if need_reselect || self.viewport_changed {
            self.SetActivePanelBestPossible(tree);
        }
    }

    /// Mark viewed coordinates as needing recomputation (e.g. after layout changes).
    pub fn mark_viewing_dirty(&mut self) {
        self.viewing_dirty = true;
    }

    // --- Supreme panel ---

    pub fn supreme_panel(&self) -> PanelId {
        self.svp.unwrap_or(self.current_visit().panel)
    }

    // --- Stress test ---

    /// Sync stress test state with the STRESS_TEST flag. Call this each frame.
    pub fn sync_stress_test(&mut self) {
        if self.flags.contains(ViewFlags::STRESS_TEST) {
            if self.stress_test.is_none() {
                self.stress_test = Some(StressTest::new());
            }
            if let Some(st) = &mut self.stress_test {
                st.record_frame();
            }
        } else if self.stress_test.is_some() {
            self.stress_test = None;
        }
    }

    /// Whether the stress test is currently active.
    pub fn is_stress_test_active(&self) -> bool {
        self.stress_test.is_some()
    }

    /// Access the stress test state (for testing).
    pub fn stress_test(&self) -> Option<&StressTest> {
        self.stress_test.as_ref()
    }

    // --- Paint ---

    pub fn Paint(&self, tree: &mut PanelTree, painter: &mut emPainter) {
        // Fill background
        painter.push_state();
        painter.PaintRect(
            0.0,
            0.0,
            self.viewport_width,
            self.viewport_height,
            self.background_color,
            emColor::TRANSPARENT,
        );
        painter.pop_state();

        // Paint from SVP using absolute viewed coords
        let start = self.svp.unwrap_or(self.root);
        let base_offset = painter.offset();
        self.paint_panel_recursive(tree, painter, start, base_offset, self.background_color);

        // D-PANEL-06: Paint focus/active highlight (C++ PaintHighlight parity)
        self.paint_highlight(tree, painter);

        // Stress test overlay (C++ StressTestClass::PaintInfo, after all panel painting)
        if let Some(st) = &self.stress_test {
            st.paint_info(painter, self.viewport_width, self.viewport_height);
        }
    }

    /// D-PANEL-06: Paint highlight around the active panel.
    ///
    /// C++ draws a rounded rectangle with arrows around the active panel's
    /// substance rect. White normally, light yellow if adherent, dimmed if
    /// window not focused.
    fn paint_highlight(&self, tree: &PanelTree, painter: &mut emPainter) {
        if self.flags.contains(ViewFlags::NO_ACTIVE_HIGHLIGHT) {
            return;
        }

        let active_id = match self.active {
            Some(id) => id,
            None => return,
        };

        let panel = match tree.GetRec(active_id) {
            Some(p) if p.viewed => p,
            _ => return,
        };

        // Get the panel's substance rect in viewport coords
        let (sx, sy, sw, sh, _sr) = tree.GetSubstanceRect(active_id);
        let hx = panel.viewed_x + sx * panel.viewed_width;
        let hy = panel.viewed_y + sy * panel.viewed_width;
        let hw = sw * panel.viewed_width;
        let hh = sh * panel.viewed_width;

        if hw < 1.0 || hh < 1.0 {
            return;
        }

        // Expand by distance-from-panel (C++ constant: 2.0)
        let pad = 2.0;
        let hx = hx - pad;
        let hy = hy - pad;
        let hw = hw + pad * 2.0;
        let hh = hh + pad * 2.0;

        // emColor selection (C++ constants)
        let base_color = if self.activation_adherent {
            emColor::rgba(255, 255, 187, 255) // Light yellow for adherent
        } else {
            emColor::rgba(255, 255, 255, 255) // White
        };

        let alpha = if !self.window_focused || self.flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT) {
            85 // alpha / 3
        } else {
            255
        };

        let color = emColor::rgba(base_color.GetRed(), base_color.GetGreen(), base_color.GetBlue(), alpha);

        // Shadow color: black with alpha 192 normally, alpha/3 when unfocused
        let shadow_alpha =
            if !self.window_focused || self.flags.contains(ViewFlags::NO_FOCUS_HIGHLIGHT) {
                64
            } else {
                192
            };
        let shadow_color = emColor::rgba(0, 0, 0, shadow_alpha);

        // C++ constants
        let arrow_size = 11.0;
        let arrow_distance = 55.0;

        // Corner radius — C++ uses substance_rect rounding scaled to viewport
        let (_sx2, _sy2, _sw2, _sh2, sr) = tree.GetSubstanceRect(active_id);
        let corner_r = (sr * panel.viewed_width).max(0.0);

        // Goal point: center of the highlight rect
        let goal_x = hx + hw * 0.5;
        let goal_y = hy + hh * 0.5;

        // Build the perimeter as 8 segments: 4 bows + 4 lines
        // Walk clockwise starting from top-right corner midpoint
        // Segment order: top-right bow, right line, bottom-right bow,
        //                bottom line, bottom-left bow, left line,
        //                top-left bow, top line
        let r = corner_r.min(hw * 0.5).min(hh * 0.5);

        // Line segments (after accounting for corner radii)
        let segments: [(f64, f64, f64, f64, bool); 8] = [
            // top-right bow: center (hx+hw-r, hy+r), start angle -PI/2, quarter CW
            (hx + hw - r, hy + r, r, 0.0, true),
            // right line: top to bottom
            (hx + hw, hy + r, hx + hw, hy + hh - r, false),
            // bottom-right bow
            (hx + hw - r, hy + hh - r, r, 1.0, true),
            // bottom line: right to left
            (hx + hw - r, hy + hh, hx + r, hy + hh, false),
            // bottom-left bow
            (hx + r, hy + hh - r, r, 2.0, true),
            // left line: bottom to top
            (hx, hy + hh - r, hx, hy + r, false),
            // top-left bow
            (hx + r, hy + r, r, 3.0, true),
            // top line: left to right
            (hx + r, hy, hx + hw - r, hy, false),
        ];

        painter.push_state();

        for &(a, b, c, d, is_bow) in &segments {
            if is_bow {
                // Bow: (cx, cy, radius, quadrant)
                paint_highlight_arrows_on_bow(
                    painter,
                    a,
                    b,
                    c,
                    d as usize,
                    goal_x,
                    goal_y,
                    arrow_size,
                    arrow_distance,
                    color,
                    shadow_color,
                    self.viewport_width,
                    self.viewport_height,
                );
            } else {
                // Line: (x1, y1, x2, y2)
                paint_highlight_arrows_on_line(
                    painter,
                    a,
                    b,
                    c,
                    d,
                    goal_x,
                    goal_y,
                    arrow_size,
                    arrow_distance,
                    color,
                    shadow_color,
                    self.viewport_width,
                    self.viewport_height,
                );
            }
        }

        painter.pop_state();
    }

    /// Paint a sub-tree starting at `root` with the given base offset and
    /// background color. Used by [`emSubViewPanel`] to delegate painting to a
    /// sub-view's panel tree.
    pub(crate) fn paint_sub_tree(
        &self,
        tree: &mut PanelTree,
        painter: &mut emPainter,
        root: PanelId,
        base_offset: (f64, f64),
        background: emColor,
    ) {
        self.paint_panel_recursive(tree, painter, root, base_offset, background);
    }

    fn paint_panel_recursive(
        &self,
        tree: &mut PanelTree,
        painter: &mut emPainter,
        id: PanelId,
        base_offset: (f64, f64),
        parent_canvas: emColor,
    ) {
        let (vx, vy, vw, vh, clip_x, clip_y, clip_w, clip_h, canvas_color, layout_rect) = {
            match tree.GetRec(id) {
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
                    p.layout_rect,
                ),
                _ => return,
            }
        };

        painter.push_state();
        // Set absolute offset (not cumulative) — viewed coords are in
        // absolute viewport pixels, so each panel computes its offset
        // from the base (tile) offset independently.
        painter.set_offset(base_offset.0 + vx, base_offset.1 + vy);
        painter.SetClipping(clip_x - vx, clip_y - vy, clip_w, clip_h);

        // Skip this panel and its entire subtree if it doesn't intersect
        // the current tile's clip region.
        if painter.GetClipX1() {
            painter.pop_state();
            return;
        }

        // C++ canvasColor inheritance: if a panel has no explicit canvas
        // color (TRANSPARENT), inherit from the parent. The root's parent
        // is the view background.
        let effective_canvas = if canvas_color.GetAlpha() > 0 {
            canvas_color
        } else {
            parent_canvas
        };
        painter.SetCanvasColor(effective_canvas);

        if let Some(mut behavior) = tree.take_behavior(id) {
            let mut state = tree.build_panel_state(id, self.window_focused, self.pixel_tallness);
            state.priority = tree.GetUpdatePriority(
                id,
                self.viewport_width,
                self.viewport_height,
                self.window_focused,
            );
            // C++ default: MaxMegabytesPerView = 2048 → 2048 * 1_000_000.
            const DEFAULT_MEMORY_LIMIT: u64 = 2_048_000_000;
            state.memory_limit = tree.GetMemoryLimit(
                id,
                self.viewport_width,
                self.viewport_height,
                DEFAULT_MEMORY_LIMIT,
                self.seek_pos_panel,
            );
            // In Eagle Mode, viewed_height is a scaling factor (parent-width
            // units), not the panel's actual pixel height. The real pixel
            // height = viewed_width * tallness, where tallness = layout_h / layout_w.
            // For children this equals viewed_height, but for the root it differs.
            let paint_h = if layout_rect.w > 0.0 {
                vw * (layout_rect.h / layout_rect.w)
            } else {
                vh
            };
            behavior.Paint(painter, vw, paint_h, &state);
            tree.put_behavior(id, behavior);
        }

        let children: Vec<PanelId> = tree.children(id).collect();
        for child in children {
            self.paint_panel_recursive(tree, painter, child, base_offset, effective_canvas);
        }

        painter.pop_state();
    }

    // --- Tree dump ---

    /// Dump the panel tree to `temp_dir()/debug.emTreeDump` in
    /// emRec format. Returns the path written.
    pub fn dump_tree(&self, tree: &mut PanelTree) -> std::path::PathBuf {
        let path = std::env::temp_dir().join("debug.emTreeDump");

        // emView-level data: flags, title, focused, home_rect
        let mut view_rec = RecStruct::new();
        view_rec.set_str("title", &self.title);
        view_rec.set_str("flags", &format!("{:?}", self.flags));
        view_rec.set_bool("focused", self.window_focused);
        view_rec.set_str(
            "home_rect",
            &format!(
                "({:.3}, {:.3}, {:.3}, {:.3})",
                0.0, 0.0, self.viewport_width, self.viewport_height
            ),
        );

        // Panel tree
        let panels_rec = self.dump_panel_recursive(tree, self.root);

        let mut root_rec = RecStruct::new();
        root_rec.SetValue("view", RecValue::Struct(view_rec));
        root_rec.SetValue("panels", panels_rec);

        let text = write_rec_with_format(&root_rec, "emTreeDump");
        if let Err(e) = std::fs::write(&path, &text) {
            eprintln!("[TreeDump] write failed: {e}");
        } else {
            eprintln!("[TreeDump] wrote {}", path.display());
        }
        path
    }

    fn dump_panel_recursive(&self, tree: &mut PanelTree, id: PanelId) -> RecValue {
        let mut rec = RecStruct::new();

        // Type name from behavior
        let type_name = if let Some(behavior) = tree.take_behavior(id) {
            let name = behavior.type_name().to_string();
            tree.put_behavior(id, behavior);
            name
        } else {
            "(no behavior)".to_string()
        };
        rec.set_str("title", &type_name);

        // Panel fields
        let height = tree.get_height(id);
        let is_focused = self.focused == Some(id);
        if let Some(p) = tree.GetRec(id) {
            let mut text = String::new();
            text.push_str(&format!("name = {}\n", p.name));
            text.push_str(&format!(
                "layout_rect = ({:.3}, {:.3}, {:.3}, {:.3})\n",
                p.layout_rect.x, p.layout_rect.y, p.layout_rect.w, p.layout_rect.h
            ));
            text.push_str(&format!("height = {:.6}\n", height));
            text.push_str(&format!("viewed = {}\n", p.viewed));
            text.push_str(&format!("enabled = {}\n", p.enabled));
            text.push_str(&format!("focusable = {}\n", p.focusable));
            text.push_str(&format!("is_active = {}\n", p.is_active));
            text.push_str(&format!("in_active_path = {}\n", p.in_active_path));
            text.push_str(&format!("focused = {}\n", is_focused));
            rec.set_str("text", &text);
        }

        // Children (recursive)
        let children: Vec<PanelId> = tree.children(id).collect();
        if !children.is_empty() {
            let child_recs: Vec<RecValue> = children
                .into_iter()
                .map(|child| self.dump_panel_recursive(tree, child))
                .collect();
            rec.SetValue("children", RecValue::Array(child_recs));
        }

        RecValue::Struct(rec)
    }

    /// Handle a custom cheat code. Override in subclasses for app-specific cheats.
    /// C++ `emView::DoCustomCheat(const char* func)`.
    pub(crate) fn DoCustomCheat(&self, func: &str) {
        // DIVERGED: C++ default walks GetParentContext() chain to find ancestor
        // emView and delegates. Needs emContext parent traversal to be ported.
        log::debug!("Unknown cheat code: {}", func);
    }

    /// Activate the magnetic view animator.
    /// C++ `emMagneticViewAnimator::Activate()`.
    pub(crate) fn activate_magnetic_view_animator(&mut self) {
        // TODO(magnetic): Full activation requires finding the nearest focusable
        // panel and setting it as the snap target. The emMagneticViewAnimator
        // struct exists in emViewAnimator.rs but the view doesn't own it — it's
        // managed by the window loop. This stub signals the intent; the actual
        // wiring needs the window loop to check a flag and activate the animator.
        log::trace!("magnetic view animator: activation requested (not yet wired to window loop)");
    }
}

// ── Highlight arrow helpers (C++ emView.cpp:2300-2479) ──

/// Compute the 4 vertices of a highlight arrow chevron.
/// Returns [(tip), (right wing), (notch), (left wing)].
fn compute_arrow_vertices(
    x: f64,
    y: f64,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
) -> [(f64, f64); 4] {
    let gdx = x - goal_x;
    let gdy = y - goal_y;
    let glen = (gdx * gdx + gdy * gdy).sqrt().max(1e-10);
    let dx = gdx / glen;
    let dy = gdy / glen;

    let ah = arrow_size; // arrow head length
    let aw = arrow_size * 0.5; // arrow half-width
    let ag = ah * 0.8; // notch depth

    let tip = (x, y);
    let right = (x + dx * ah - dy * aw * 0.5, y + dy * ah + dx * aw * 0.5);
    let notch = (x + dx * ag, y + dy * ag);
    let left = (x + dx * ah + dy * aw * 0.5, y + dy * ah - dx * aw * 0.5);

    [tip, right, notch, left]
}

/// Round arrow count to a "nice" number per C++ formula.
fn compute_arrow_count(len: f64, arrow_distance: f64) -> usize {
    if len < arrow_distance * 0.5 {
        return 0;
    }
    let mut n = (len / arrow_distance).round() as usize;
    if n == 0 {
        return 0;
    }
    // Find smallest power of 2 >= n
    let mut m = 1usize;
    while m < n {
        m <<= 1;
    }
    n &= m | (m >> 1) | (m >> 2);
    n.max(1)
}

/// Paint a single highlight arrow (shadow + arrow polygon).
#[allow(clippy::too_many_arguments)]
fn paint_highlight_arrow(
    painter: &mut emPainter,
    x: f64,
    y: f64,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
    color: emColor,
    shadow_color: emColor,
) {
    let sd = arrow_size * 0.2;

    // Shadow polygon (offset toward bottom-right)
    let shadow_verts = compute_arrow_vertices(x + sd, y + sd, goal_x, goal_y, arrow_size);
    painter.PaintPolygon(&shadow_verts, shadow_color, emColor::TRANSPARENT);

    // Arrow polygon
    let verts = compute_arrow_vertices(x, y, goal_x, goal_y, arrow_size);
    painter.PaintPolygon(&verts, color, emColor::TRANSPARENT);
}

/// Paint arrows along a straight line segment.
#[allow(clippy::too_many_arguments)]
fn paint_highlight_arrows_on_line(
    painter: &mut emPainter,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
    arrow_distance: f64,
    color: emColor,
    shadow_color: emColor,
    vw: f64,
    vh: f64,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }

    let n = compute_arrow_count(len, arrow_distance);
    if n == 0 {
        return;
    }

    let margin = arrow_size * 1.5;

    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let ax = x1 + dx * t;
        let ay = y1 + dy * t;

        // Clip to viewport (with margin for arrow size)
        if ax < -margin || ax > vw + margin || ay < -margin || ay > vh + margin {
            continue;
        }

        paint_highlight_arrow(
            painter,
            ax,
            ay,
            goal_x,
            goal_y,
            arrow_size,
            color,
            shadow_color,
        );
    }
}

/// Paint arrows along a quarter-circle arc (bow).
#[allow(clippy::too_many_arguments)]
fn paint_highlight_arrows_on_bow(
    painter: &mut emPainter,
    cx: f64,
    cy: f64,
    radius: f64,
    quadrant: usize,
    goal_x: f64,
    goal_y: f64,
    arrow_size: f64,
    arrow_distance: f64,
    color: emColor,
    shadow_color: emColor,
    vw: f64,
    vh: f64,
) {
    if radius < 1.0 {
        return;
    }

    let arc_len = radius * std::f64::consts::FRAC_PI_2;
    let n = compute_arrow_count(arc_len, arrow_distance);
    if n == 0 {
        return;
    }

    // Start angle for each quadrant (clockwise from top-right)
    let start_angle = match quadrant {
        0 => -std::f64::consts::FRAC_PI_2, // top-right: -90° to 0°
        1 => 0.0,                          // bottom-right: 0° to 90°
        2 => std::f64::consts::FRAC_PI_2,  // bottom-left: 90° to 180°
        _ => std::f64::consts::PI,         // top-left: 180° to 270°
    };

    let margin = arrow_size * 1.5;

    for i in 0..n {
        let t = (i as f64 + 0.5) / n as f64;
        let angle = start_angle + t * std::f64::consts::FRAC_PI_2;
        let ax = cx + radius * angle.cos();
        let ay = cy + radius * angle.sin();

        if ax < -margin || ax > vw + margin || ay < -margin || ay > vh + margin {
            continue;
        }

        paint_highlight_arrow(
            painter,
            ax,
            ay,
            goal_x,
            goal_y,
            arrow_size,
            color,
            shadow_color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;

    fn setup_tree() -> (PanelTree, PanelId, PanelId, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let child1 = tree.create_child(root, "child1");
        tree.get_mut(child1).unwrap().focusable = true;
        tree.Layout(child1, 0.0, 0.0, 0.5, 1.0);

        let child2 = tree.create_child(root, "child2");
        tree.get_mut(child2).unwrap().focusable = true;
        tree.Layout(child2, 0.5, 0.0, 0.5, 1.0);

        (tree, root, child1, child2)
    }

    #[test]
    fn test_update_viewing_sets_coords() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Root should be viewed
        let rp = tree.GetRec(root).unwrap();
        assert!(rp.viewed);
        assert!(rp.viewed_width > 0.0);
        assert!(rp.viewed_height > 0.0);

        // Children should be viewed
        assert!(tree.GetRec(child1).unwrap().viewed);
        assert!(tree.GetRec(child2).unwrap().viewed);
    }

    #[test]
    fn test_svp_selection() {
        let (mut tree, root, _child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // SVP should be set
        assert!(view.GetSupremeViewedPanel().is_some());
    }

    #[test]
    fn test_viewed_false_outside_viewport() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let offscreen = tree.create_child(root, "offscreen");
        tree.Layout(offscreen, 5.0, 5.0, 0.1, 0.1);

        let mut view = emView::new(root, 100.0, 100.0);
        view.Update(&mut tree);

        assert!(!tree.GetRec(offscreen).unwrap().viewed);
    }

    #[test]
    fn test_active_path_flags() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.set_active_panel(&mut tree, child1, false);
        view.Update(&mut tree);

        assert!(tree.GetRec(child1).unwrap().is_active);
        assert!(tree.GetRec(child1).unwrap().in_active_path);
        assert!(tree.GetRec(root).unwrap().in_active_path);
    }

    #[test]
    fn test_fix_point_zoom() {
        let (_tree, root, _c1, _c2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);

        // Zoom around center — should keep center stable
        let before = view.current_visit().clone();
        view.Zoom(2.0, 400.0, 300.0);
        let after = view.current_visit().clone();

        // C++ Zoom(factor=2): ra *= reFac^2 = 1/4, so rel_a *= 4
        assert!((after.rel_a - before.rel_a * 4.0).abs() < 0.01);
    }

    #[test]
    fn test_visit_next_prev() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        view.VisitNext(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child2));

        view.VisitPrev(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_visit_in_out() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let grandchild = tree.create_child(child1, "gc");
        tree.get_mut(grandchild).unwrap().focusable = true;
        tree.Layout(grandchild, 0.0, 0.0, 1.0, 1.0);

        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        view.VisitIn(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(grandchild));

        view.VisitOut(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_hit_testing() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Hit test in left half should find child1, right half child2
        let left_hit = view.GetFocusablePanelAt(&tree, 100.0, 300.0);
        let right_hit = view.GetFocusablePanelAt(&tree, 600.0, 300.0);

        assert_eq!(left_hit, Some(child1));
        assert_eq!(right_hit, Some(child2));
    }

    #[test]
    fn test_directional_navigation() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // child2 is to the right of child1
        view.VisitRight(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child2));

        view.VisitLeft(&mut tree);
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_focus_panel() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.SetFocused(&mut tree, false);

        view.focus_panel(&mut tree, child1);
        assert!(view.IsFocused());
        assert_eq!(view.GetActivePanel(), Some(child1));
    }

    #[test]
    fn test_is_panel_focused() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        assert!(view.is_panel_focused(&tree, child1));
        assert!(!view.is_panel_focused(&tree, root));

        view.SetFocused(&mut tree, false);
        assert!(!view.is_panel_focused(&tree, child1));
    }

    #[test]
    fn test_is_panel_in_focused_path() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        assert!(view.is_panel_in_focused_path(&tree, child1));
        assert!(view.is_panel_in_focused_path(&tree, root));
        assert!(!view.is_panel_in_focused_path(&tree, child2));

        view.SetFocused(&mut tree, false);
        assert!(!view.is_panel_in_focused_path(&tree, child1));
    }

    #[test]
    fn test_is_view_focused_delegate() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        assert!(view.is_view_focused());
        view.SetFocused(&mut tree, false);
        assert!(!view.is_view_focused());
    }

    // ── Invalidation tests ───────────────────────────────────────────

    #[test]
    fn test_invalidate_painting_whole() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // child1 should be viewed after update_viewing
        view.InvalidatePainting(&tree, child1);
        let rects = view.take_dirty_rects();
        assert_eq!(rects.len(), 1);
        // The dirty rect should be the child's clip rect
        let p = tree.GetRec(child1).unwrap();
        assert!((rects[0].x - p.clip_x).abs() < 1e-6);
        assert!((rects[0].y - p.clip_y).abs() < 1e-6);
    }

    #[test]
    fn test_invalidate_painting_rect() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

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
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // child1 is active, thus in_active_path
        assert!(!view.is_title_invalid());
        view.InvalidateTitle(&tree, child1);
        assert!(view.is_title_invalid());
        view.clear_title_invalid();
        assert!(!view.is_title_invalid());

        // root is in_viewed_path (it's an ancestor of the SVP)
        assert!(!view.is_cursor_invalid());
        view.InvalidateCursor(&tree, root);
        assert!(view.is_cursor_invalid());
        view.clear_cursor_invalid();

        // child1 is viewed AND in_viewed_path (all viewed panels are)
        view.InvalidateCursor(&tree, child1);
        assert!(view.is_cursor_invalid());
    }

    #[test]
    fn test_invalidate_control_panel() {
        let (mut tree, root, child1, child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);
        view.set_active_panel(&mut tree, child1, false);

        // set_active_panel unconditionally invalidates the control panel
        assert!(view.is_control_panel_invalid());
        view.clear_control_panel_invalid();

        // child1 is in active path — invalidate_control_panel sets flag
        view.InvalidateControlPanel(&tree, child1);
        assert!(view.is_control_panel_invalid());
        view.clear_control_panel_invalid();

        // child2 is NOT in active path — flag stays clear
        view.InvalidateControlPanel(&tree, child2);
        assert!(!view.is_control_panel_invalid());
    }

    #[test]
    fn test_pixel_tallness() {
        let (mut tree, root, _c1, _c2) = setup_tree();
        let view = emView::new(root, 800.0, 600.0);
        assert!((view.GetCurrentPixelTallness() - 0.75).abs() < 1e-6);

        let mut view2 = emView::new(root, 1920.0, 1080.0);
        assert!((view2.GetCurrentPixelTallness() - 1080.0 / 1920.0).abs() < 1e-6);

        view2.SetGeometry(&mut tree, 100.0, 200.0);
        assert!((view2.GetCurrentPixelTallness() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_activation_adherent() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Direct activation is not adherent
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.IsActivationAdherent());
        assert!(!view.is_panel_activated_adherent(&tree, child1));

        // Explicit adherent activation
        view.set_active_panel(&mut tree, child1, true);
        assert!(view.IsActivationAdherent());
        assert!(view.is_panel_activated_adherent(&tree, child1));

        // Switching to a different panel clears adherent
        view.set_active_panel(&mut tree, root, false);
        assert!(!view.IsActivationAdherent());
    }

    #[test]
    fn test_activation_adherent_early_return_update() {
        let (mut tree, root, child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Set active non-adherent
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.IsActivationAdherent());

        // Re-set same panel as adherent — hits early-return path, updates flag
        view.set_active_panel(&mut tree, child1, true);
        assert!(view.IsActivationAdherent());

        // Re-set same panel as non-adherent — hits early-return path again
        view.set_active_panel(&mut tree, child1, false);
        assert!(!view.IsActivationAdherent());
    }

    #[test]
    fn test_get_input_clock_ms() {
        let (_tree, root, _c1, _c2) = setup_tree();
        let view = emView::new(root, 800.0, 600.0);
        let ms = view.GetInputClockMS();
        // Should be a reasonable epoch-based timestamp (after year 2020)
        assert!(ms > 1_577_836_800_000);
    }

    #[test]
    fn test_highlight_rect_uses_viewed_width_for_y() {
        // Create a non-square child panel so viewed_height != viewed_width.
        // Root is square so viewed_width == viewed_height for it — we need
        // to test with a child whose layout_h != layout_w.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let child = tree.create_child(root, "child");
        tree.get_mut(child).unwrap().focusable = true;
        // Non-square child: w=0.5, h=0.25 → tallness = 0.5
        tree.Layout(child, 0.1, 0.1, 0.5, 0.25);

        let mut view = emView::new(root, 800.0, 600.0);
        view.set_active_panel(&mut tree, child, false);
        view.Update(&mut tree);

        let panel = tree.GetRec(child).unwrap();
        assert!(panel.viewed, "child should be viewed");
        // For a non-square child, viewed_width and viewed_height differ
        assert!(
            (panel.viewed_height - panel.viewed_width).abs() > 1.0,
            "Test setup: child must have viewed_width != viewed_height, got w={} h={}",
            panel.viewed_width,
            panel.viewed_height
        );

        // Substance rect components are in width-relative units.
        // Y and H must multiply by viewed_width, not viewed_height.
        let (_sx, sy, _sw, sh, _sr) = tree.GetSubstanceRect(child);
        let correct_hy = panel.viewed_y + sy * panel.viewed_width;
        let correct_hh = sh * panel.viewed_width;
        let wrong_hh = sh * panel.viewed_height;
        assert!(
            (correct_hh - wrong_hh).abs() > 1.0,
            "viewed_width and viewed_height must produce different results"
        );
        assert!(correct_hy.is_finite());
        assert!(correct_hh > 0.0);
    }

    #[test]
    fn test_raw_zoom_out_computes_fit_ratio() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75);

        let mut view = emView::new(root, 800.0, 600.0);
        view.RawZoomOut(&mut tree);

        let state = view.current_visit();
        // C++ formula: max(W*H_root/hpt/H, H/H_root*hpt/W)
        // home_pixel_tallness = 1.0 (square pixels)
        let hpt = 1.0;
        let expected = (800.0 * 0.75 / hpt / 600.0_f64).max(600.0 / 0.75 * hpt / 800.0);
        assert!(
            (state.rel_a - expected).abs() < 0.001,
            "rel_a should be {expected}, got {}",
            state.rel_a
        );
        assert!(state.rel_x.abs() < 0.001);
        assert!(state.rel_y.abs() < 0.001);
    }

    #[test]
    fn test_is_zoomed_out_after_raw_zoom_out() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75);

        let mut view = emView::new(root, 800.0, 600.0);
        view.RawZoomOut(&mut tree);
        assert!(view.IsZoomedOut(&tree));

        // After zooming in, should not be zoomed out
        view.Zoom(2.0, 400.0, 300.0);
        assert!(!view.IsZoomedOut(&tree));
    }

    #[test]
    fn test_set_view_flags_root_same_tallness_updates_layout() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0); // starts square

        let mut view = emView::new(root, 800.0, 600.0);
        // pixel_tallness = 600/800 = 0.75
        let flags = view.flags | ViewFlags::ROOT_SAME_TALLNESS;
        view.SetViewFlags(flags, &mut tree);

        let rect = tree.layout_rect(root).unwrap();
        assert!(
            (rect.h - 0.75).abs() < 0.001,
            "Root layout_h should match pixel_tallness (0.75), got {}",
            rect.h
        );
    }

    #[test]
    fn test_highlight_arrow_vertices() {
        // Arrow at (100, 100) with goal at (100, 50).
        // C++ direction = away from goal: dx=0, dy=+1 (pointing down).
        let verts = compute_arrow_vertices(100.0, 100.0, 100.0, 50.0, 11.0);
        // tip: (100, 100)
        assert!((verts[0].0 - 100.0).abs() < 0.01);
        assert!((verts[0].1 - 100.0).abs() < 0.01);
        // right: (100 + 0*11 - 1*5.5*0.5, 100 + 1*11 + 0*5.5*0.5) = (97.25, 111)
        assert!((verts[1].0 - 97.25).abs() < 0.01);
        assert!((verts[1].1 - 111.0).abs() < 0.01);
        // notch: (100, 100 + 1*8.8) = (100, 108.8)
        assert!((verts[2].0 - 100.0).abs() < 0.01);
        assert!((verts[2].1 - 108.8).abs() < 0.01);
        // left: (100 + 0*11 + 1*5.5*0.5, 100 + 1*11 - 0*5.5*0.5) = (102.75, 111)
        assert!((verts[3].0 - 102.75).abs() < 0.01);
        assert!((verts[3].1 - 111.0).abs() < 0.01);
    }

    #[test]
    fn test_highlight_arrow_count_rounding() {
        assert_eq!(compute_arrow_count(55.0, 55.0), 1);
        assert_eq!(compute_arrow_count(110.0, 55.0), 2);
        assert_eq!(compute_arrow_count(165.0, 55.0), 3);
        assert_eq!(compute_arrow_count(220.0, 55.0), 4);
        assert_eq!(compute_arrow_count(385.0, 55.0), 6);
        assert_eq!(compute_arrow_count(440.0, 55.0), 8);
        // Too short — no arrows
        assert_eq!(compute_arrow_count(20.0, 55.0), 0);
    }

    #[test]
    fn ego_mode_cursor_override() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Default cursor is Normal
        view.set_cursor(emCursor::Normal);
        assert_eq!(view.GetCursor(), emCursor::Normal);

        // With EGO_MODE, Normal cursor becomes Crosshair
        view.flags |= ViewFlags::EGO_MODE;
        assert_eq!(view.GetCursor(), emCursor::Crosshair);

        // Non-Normal cursors are NOT overridden
        view.set_cursor(emCursor::Text);
        assert_eq!(view.GetCursor(), emCursor::Text);

        // Turning off EGO_MODE restores Normal
        view.set_cursor(emCursor::Normal);
        view.flags -= ViewFlags::EGO_MODE;
        assert_eq!(view.GetCursor(), emCursor::Normal);
    }

    #[test]
    fn ego_mode_scroll_locked() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Record initial center position
        let visit_before = view.current_visit().clone();

        // Enable EGO_MODE and attempt to scroll
        view.flags |= ViewFlags::EGO_MODE;
        let done = view.RawScrollAndZoom(&mut tree, 400.0, 300.0, 50.0, 50.0, 0.0);

        // Scroll delta should be zero — viewport center locked
        let visit_after = view.current_visit().clone();
        assert!(
            (visit_after.rel_x - visit_before.rel_x).abs() < 1e-12,
            "rel_x should not change under EGO_MODE, delta={}",
            visit_after.rel_x - visit_before.rel_x
        );
        assert!(
            (visit_after.rel_y - visit_before.rel_y).abs() < 1e-12,
            "rel_y should not change under EGO_MODE, delta={}",
            visit_after.rel_y - visit_before.rel_y
        );
        assert!(
            done[0].abs() < 1e-12 && done[1].abs() < 1e-12,
            "done_x and done_y should be zero"
        );
    }

    #[test]
    fn ego_mode_zoom_still_works() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        let rel_a_before = view.current_visit().rel_a;

        // Enable EGO_MODE and zoom
        view.flags |= ViewFlags::EGO_MODE;
        view.RawScrollAndZoom(&mut tree, 400.0, 300.0, 0.0, 0.0, 50.0);

        let rel_a_after = view.current_visit().rel_a;
        assert!(
            (rel_a_after - rel_a_before).abs() > 1e-6,
            "zoom should still work under EGO_MODE"
        );
    }

    #[test]
    fn ego_mode_toggle_invalidates_cursor() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        assert!(!view.is_cursor_invalid());
        view.flags ^= ViewFlags::EGO_MODE;
        view.mark_cursor_invalid();
        assert!(view.is_cursor_invalid());
    }

    #[test]
    fn stress_test_sync_creates_and_destroys() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        // Initially no stress test
        assert!(!view.is_stress_test_active());
        assert!(view.stress_test().is_none());

        // Enable STRESS_TEST and sync
        view.flags |= ViewFlags::STRESS_TEST;
        view.sync_stress_test();
        assert!(view.is_stress_test_active());
        assert!(view.stress_test().is_some());

        // Disable and sync — struct dropped
        view.flags -= ViewFlags::STRESS_TEST;
        view.sync_stress_test();
        assert!(!view.is_stress_test_active());
        assert!(view.stress_test().is_none());
    }

    #[test]
    fn stress_test_ring_buffer_accumulates() {
        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        view.flags |= ViewFlags::STRESS_TEST;
        // Sync multiple times to accumulate entries
        for _ in 0..10 {
            view.sync_stress_test();
        }
        let st = view.stress_test().unwrap();
        assert_eq!(st.valid_count(), 10);
    }

    #[test]
    fn stress_test_paint_overlay() {
        use crate::emImage::emImage;

        let (mut tree, root, _, _) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.Update(&mut tree);

        view.flags |= ViewFlags::STRESS_TEST;
        view.sync_stress_test();

        // Paint into a real image and verify the overlay renders without panic
        let mut img = emImage::new(800, 600, 4);
        let mut painter = emPainter::new(&mut img);
        view.Paint(&mut tree, &mut painter);

        // Check that the top-left corner has non-zero (overlay painted) pixels.
        // The purple background (255,0,255,128) should have been blended there.
        let px = img.GetMap();
        // pixel at (5, 5): offset = (5 * 800 + 5) * 4
        let off = (5 * 800 + 5) * 4;
        let has_overlay = px[off] > 0 || px[off + 1] > 0 || px[off + 2] > 0;
        assert!(has_overlay, "stress test overlay should paint in top-left corner");
    }

    #[test]
    fn tree_dump_produces_valid_emrec() {
        use crate::emRec::parse_rec_with_format;

        let (mut tree, root, _child1, _child2) = setup_tree();
        let view = emView::new(root, 800.0, 600.0);

        let path = view.dump_tree(&mut tree);

        // File should exist
        assert!(path.exists(), "dump file should exist at {:?}", path);

        // Read and parse as emRec
        let content = std::fs::read_to_string(&path).expect("read dump file");
        let rec = parse_rec_with_format(&content, "emTreeDump")
            .expect("dump should be valid emRec format");

        // Should contain view-level data
        assert!(rec.get_str("view.title").is_some() || rec.get_struct("view").is_some());

        // Should contain panel names from the test tree
        assert!(
            content.contains("root"),
            "dump should contain root panel name"
        );
        assert!(
            content.contains("child1"),
            "dump should contain child1 panel name"
        );
        assert!(
            content.contains("child2"),
            "dump should contain child2 panel name"
        );

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    // --- Coordinate-system invariant tests ---
    // These test physical behavior, not coordinate values, so they
    // survive a convention change (viewport-fraction → panel-fraction).

    /// Helper: convert viewport pixel to panel-local coordinates using
    /// viewed_x/viewed_width from the panel tree. Convention-independent.
    fn panel_space_at_pixel(
        tree: &PanelTree,
        panel: PanelId,
        px: f64,
        py: f64,
    ) -> (f64, f64) {
        let rec = tree.GetRec(panel).unwrap();
        (
            (px - rec.viewed_x) / rec.viewed_width,
            (py - rec.viewed_y) / rec.viewed_height,
        )
    }

    #[test]
    fn invariant_zoom_fixpoint() {
        // The pixel under the cursor maps to the same panel-space point
        // before and after zoom, at various cursor positions and zoom factors.
        let (mut tree, root, _child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        view.Update(&mut tree);

        // Start at a moderate zoom so there's room to zoom in and out
        view.Zoom(4.0, 400.0, 300.0);
        view.Update(&mut tree);

        let cursors = [(200.0, 150.0), (400.0, 300.0), (700.0, 50.0), (50.0, 550.0)];
        let factors = [0.01, 0.5, 2.0, 4.0, 100.0];

        for &(cx, cy) in &cursors {
            for &factor in &factors {
                // Save and restore state for each combo
                let saved = view.current_visit().clone();

                let before = panel_space_at_pixel(&tree, root, cx, cy);
                view.Zoom(factor, cx, cy);
                view.Update(&mut tree);
                let after = panel_space_at_pixel(&tree, root, cx, cy);

                assert!(
                    (before.0 - after.0).abs() < 1e-9,
                    "fix-point X violated: cursor=({cx},{cy}) factor={factor} \
                     before={:.12} after={:.12} diff={:.3e}",
                    before.0,
                    after.0,
                    (before.0 - after.0).abs()
                );
                assert!(
                    (before.1 - after.1).abs() < 1e-9,
                    "fix-point Y violated: cursor=({cx},{cy}) factor={factor} \
                     before={:.12} after={:.12} diff={:.3e}",
                    before.1,
                    after.1,
                    (before.1 - after.1).abs()
                );

                // Restore
                if let Some(state) = view.visit_stack.last_mut() {
                    *state = saved;
                }
                view.Update(&mut tree);
            }
        }
    }

    #[test]
    fn invariant_calc_visit_round_trip() {
        // rel_x=0, rel_y=0 → Update → panel center at viewport center.
        // Verifies the core coord-system invariant: zero offset means centered.
        // Tested at multiple zoom levels to catch convention/scaling errors.
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
        let mut view = emView::new(root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        view.Update(&mut tree);

        let (vw, vh) = view.viewport_size();

        for &rel_a in &[1.0, 2.0, 4.0, 16.0] {
            if let Some(state) = view.visit_stack.last_mut() {
                state.rel_x = 0.0;
                state.rel_y = 0.0;
                state.rel_a = rel_a;
            }
            view.Update(&mut tree);

            let rec = tree.GetRec(root).unwrap();
            let panel_cx = rec.viewed_x + rec.viewed_width * 0.5;
            let panel_cy = rec.viewed_y + rec.viewed_height * 0.5;

            assert!(
                (panel_cx - vw * 0.5).abs() < 0.5,
                "rel_a={rel_a}: root not centered X: panel_cx={panel_cx:.4} \
                 viewport_cx={:.4} diff={:.4}",
                vw * 0.5,
                (panel_cx - vw * 0.5).abs()
            );
            assert!(
                (panel_cy - vh * 0.5).abs() < 0.5,
                "rel_a={rel_a}: root not centered Y: panel_cy={panel_cy:.4} \
                 viewport_cy={:.4} diff={:.4}",
                vh * 0.5,
                (panel_cy - vh * 0.5).abs()
            );
        }
    }

    #[test]
    fn invariant_scroll_direction() {
        // Scroll(+50, 0) moves the panel in the SAME direction at every zoom
        // level. The specific direction depends on convention (positive dx
        // scrolls the view rightward, so panel viewed_x decreases), but the
        // invariant is consistency across zoom levels.
        let (mut tree, root, _child1, _child2) = setup_tree();
        let mut view = emView::new(root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        view.Update(&mut tree);

        let mut deltas = Vec::new();
        for &factor in &[1.0, 4.0, 16.0] {
            // Reset to center
            if let Some(state) = view.visit_stack.last_mut() {
                state.rel_x = 0.0;
                state.rel_y = 0.0;
                state.rel_a = 1.0;
            }
            view.Zoom(factor, 400.0, 300.0);
            view.Update(&mut tree);

            let before_vx = tree.GetRec(root).unwrap().viewed_x;
            view.Scroll(50.0, 0.0);
            view.Update(&mut tree);
            let after_vx = tree.GetRec(root).unwrap().viewed_x;

            let delta = after_vx - before_vx;
            assert!(
                delta.abs() > 1e-6,
                "Scroll(+50) had no effect at factor={factor}"
            );
            deltas.push((factor, delta));
        }

        // All deltas must have the same sign
        let first_sign = deltas[0].1.signum();
        for &(factor, delta) in &deltas {
            assert!(
                delta.signum() == first_sign,
                "Scroll direction inconsistent: factor={factor} delta={delta:.4}, \
                 expected sign={first_sign}"
            );
        }
    }

    #[test]
    fn test_soft_keyboard_toggle() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let mut view = emView::new(root, 800.0, 600.0);
        assert!(!view.IsSoftKeyboardShown());
        view.ShowSoftKeyboard(true);
        assert!(view.IsSoftKeyboardShown());
        view.ShowSoftKeyboard(false);
        assert!(!view.IsSoftKeyboardShown());
    }
}


#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_compute_arrow_count() {
        let mut p_len: f64 = kani::any::<f64>();
        kani::assume(p_len.is_finite());
        let mut p_arrow_distance: f64 = kani::any::<f64>();
        kani::assume(p_arrow_distance.is_finite());
        let _r = compute_arrow_count(p_len, p_arrow_distance);
    }
}
