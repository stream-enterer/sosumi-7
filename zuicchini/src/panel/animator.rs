use super::tree::PanelTree;
use super::view::View;
use crate::foundation::Color;

/// Trait for view animation strategies.
pub trait ViewAnimator {
    /// Advance the animation by one frame. Returns true if still animating.
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool;

    /// Whether the animation is currently active.
    fn is_active(&self) -> bool;

    /// Stop the animation immediately.
    fn stop(&mut self);
}

/// Kinetic view animator — applies velocity with linear friction for smooth deceleration.
/// Used for fling/swipe gestures. Supports 3D (scroll x, scroll y, zoom z).
pub struct KineticViewAnimator {
    velocity_x: f64,
    velocity_y: f64,
    velocity_z: f64,
    friction: f64,
    friction_enabled: bool,
    zoom_fix_point_centered: bool,
    zoom_fix_x: f64,
    zoom_fix_y: f64,
    active: bool,
}

impl KineticViewAnimator {
    pub fn new(velocity_x: f64, velocity_y: f64, velocity_z: f64, friction: f64) -> Self {
        Self {
            velocity_x,
            velocity_y,
            velocity_z,
            friction,
            friction_enabled: false,
            zoom_fix_point_centered: true,
            zoom_fix_x: 0.0,
            zoom_fix_y: 0.0,
            active: (velocity_x * velocity_x + velocity_y * velocity_y + velocity_z * velocity_z)
                .sqrt()
                > 0.01,
        }
    }

    pub fn set_velocity(&mut self, vx: f64, vy: f64, vz: f64) {
        self.velocity_x = vx;
        self.velocity_y = vy;
        self.velocity_z = vz;
        self.active = (vx * vx + vy * vy + vz * vz).sqrt() > 0.01;
    }

    pub fn velocity(&self) -> (f64, f64, f64) {
        (self.velocity_x, self.velocity_y, self.velocity_z)
    }

    /// Absolute velocity magnitude (sqrt of sum of squares).
    pub fn abs_velocity(&self) -> f64 {
        (self.velocity_x * self.velocity_x
            + self.velocity_y * self.velocity_y
            + self.velocity_z * self.velocity_z)
            .sqrt()
    }

    pub fn set_friction_enabled(&mut self, enabled: bool) {
        self.friction_enabled = enabled;
    }

    pub fn is_friction_enabled(&self) -> bool {
        self.friction_enabled
    }

    pub fn set_friction(&mut self, friction: f64) {
        self.friction = friction;
    }

    pub fn friction(&self) -> f64 {
        self.friction
    }

    /// Switch zoom fix point to centered mode, compensating XY velocity.
    pub fn center_zoom_fix_point(&mut self, view: &View) {
        if self.zoom_fix_point_centered {
            return;
        }
        let old_fix_x = self.zoom_fix_x;
        let old_fix_y = self.zoom_fix_y;
        self.zoom_fix_point_centered = true;
        self.update_zoom_fix_point(view);
        let dt = 0.01;
        let zflpp = view.get_zoom_factor_log_per_pixel();
        let q = (1.0 - (-self.velocity_z * dt * zflpp).exp()) / dt;
        self.velocity_x += (old_fix_x - self.zoom_fix_x) * q;
        self.velocity_y += (old_fix_y - self.zoom_fix_y) * q;
    }

    /// Set an explicit (non-centered) zoom fix point, compensating XY velocity.
    pub fn set_zoom_fix_point(&mut self, x: f64, y: f64, view: &View) {
        if !self.zoom_fix_point_centered && self.zoom_fix_x == x && self.zoom_fix_y == y {
            return;
        }
        self.update_zoom_fix_point(view);
        let old_fix_x = self.zoom_fix_x;
        let old_fix_y = self.zoom_fix_y;
        self.zoom_fix_point_centered = false;
        self.zoom_fix_x = x;
        self.zoom_fix_y = y;
        let dt = 0.01;
        let zflpp = view.get_zoom_factor_log_per_pixel();
        let q = (1.0 - (-self.velocity_z * dt * zflpp).exp()) / dt;
        self.velocity_x += (old_fix_x - self.zoom_fix_x) * q;
        self.velocity_y += (old_fix_y - self.zoom_fix_y) * q;
    }

    /// If centered, update fix point to viewport center, clamped to popup
    /// rect when the view is popped up (C++ UpdateZoomFixPoint parity).
    pub fn update_zoom_fix_point(&mut self, view: &View) {
        if self.zoom_fix_point_centered {
            let (vw, vh) = view.viewport_size();
            let mut x1 = 0.0;
            let mut y1 = 0.0;
            let mut x2 = vw;
            let mut y2 = vh;
            if view.is_popped_up() {
                if let Some(pr) = view.max_popup_rect() {
                    if x1 < pr.x {
                        x1 = pr.x;
                    }
                    if y1 < pr.y {
                        y1 = pr.y;
                    }
                    if x2 > pr.x + pr.w {
                        x2 = pr.x + pr.w;
                    }
                    if y2 > pr.y + pr.h {
                        y2 = pr.y + pr.h;
                    }
                }
            }
            self.zoom_fix_x = (x1 + x2) * 0.5;
            self.zoom_fix_y = (y1 + y2) * 0.5;
        }
    }

    fn update_busy_state(&mut self) {
        let abs_vel = (self.velocity_x * self.velocity_x
            + self.velocity_y * self.velocity_y
            + self.velocity_z * self.velocity_z)
            .sqrt();
        if self.active && abs_vel > 0.01 {
            // stay active
        } else {
            self.velocity_x = 0.0;
            self.velocity_y = 0.0;
            self.velocity_z = 0.0;
            self.active = false;
        }
    }
}

impl ViewAnimator for KineticViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        // Save pre-friction velocities for average displacement
        let vx_before = self.velocity_x;
        let vy_before = self.velocity_y;
        let vz_before = self.velocity_z;

        // Apply uniform magnitude-based friction (C++ parity):
        // compute single scale factor from velocity magnitude, apply to all axes.
        if self.friction_enabled {
            let v = (self.velocity_x * self.velocity_x
                + self.velocity_y * self.velocity_y
                + self.velocity_z * self.velocity_z)
                .sqrt();
            let a = self.friction;
            let f = if v - a * dt > 0.0 {
                (v - a * dt) / v
            } else if v + a * dt < 0.0 {
                (v + a * dt) / v
            } else {
                0.0
            };
            self.velocity_x *= f;
            self.velocity_y *= f;
            self.velocity_z *= f;
        }

        // Compute distances using average of pre/post-friction velocity
        let dist = [
            (vx_before + self.velocity_x) * 0.5 * dt,
            (vy_before + self.velocity_y) * 0.5 * dt,
            (vz_before + self.velocity_z) * 0.5 * dt,
        ];

        // Skip if motion is negligible
        if dist[0].abs() < 0.01 && dist[1].abs() < 0.01 && dist[2].abs() < 0.01 {
            self.update_busy_state();
            return self.active;
        }

        // Apply scroll and zoom
        self.update_zoom_fix_point(view);
        let done = view.raw_scroll_and_zoom(
            tree,
            self.zoom_fix_x,
            self.zoom_fix_y,
            dist[0],
            dist[1],
            dist[2],
        );
        view.set_active_panel_best_possible(tree);

        // Blocked-motion feedback: zero velocity for blocked dimensions
        for i in 0..3 {
            if done[i].abs() < 0.99 * dist[i].abs() {
                match i {
                    0 => self.velocity_x = 0.0,
                    1 => self.velocity_y = 0.0,
                    2 => self.velocity_z = 0.0,
                    _ => unreachable!(),
                }
            }
        }

        self.update_busy_state();
        self.active
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
        self.velocity_z = 0.0;
        self.active = false;
    }
}

/// Speeding view animator — accelerates toward a target velocity.
/// Composes a KineticViewAnimator for scroll/zoom delegation.
/// Used for keyboard-driven scrolling. Supports 3D.
pub struct SpeedingViewAnimator {
    inner: KineticViewAnimator,
    target_vx: f64,
    target_vy: f64,
    target_vz: f64,
    acceleration: f64,
    reverse_acceleration: f64,
    active: bool,
}

impl SpeedingViewAnimator {
    pub fn new(friction: f64) -> Self {
        Self {
            inner: KineticViewAnimator::new(0.0, 0.0, 0.0, friction),
            target_vx: 0.0,
            target_vy: 0.0,
            target_vz: 0.0,
            acceleration: 1.0,
            reverse_acceleration: 1.0,
            active: false,
        }
    }

    pub fn set_target(&mut self, vx: f64, vy: f64, vz: f64) {
        self.target_vx = vx;
        self.target_vy = vy;
        self.target_vz = vz;
        self.active = true;
    }

    pub fn release(&mut self) {
        self.target_vx = 0.0;
        self.target_vy = 0.0;
        self.target_vz = 0.0;
    }

    pub fn set_acceleration(&mut self, accel: f64) {
        self.acceleration = accel;
    }

    pub fn set_reverse_acceleration(&mut self, accel: f64) {
        self.reverse_acceleration = accel;
    }

    pub fn inner(&self) -> &KineticViewAnimator {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut KineticViewAnimator {
        &mut self.inner
    }
}

/// 3-branch acceleration: reverse, forward, or friction deceleration.
fn accelerate_dim(
    v: f64,
    target: f64,
    accel: f64,
    reverse_accel: f64,
    friction: f64,
    friction_enabled: bool,
    dt: f64,
) -> f64 {
    let adt = if v * target < -0.1 {
        // Opposite direction — use reverse acceleration
        reverse_accel * dt
    } else if v.abs() < target.abs() {
        // Below target speed — use forward acceleration, clamp dt
        accel * dt.min(0.1)
    } else if friction_enabled {
        // Above target speed — use friction deceleration
        friction * dt
    } else {
        0.0
    };

    if v - adt > target {
        v - adt
    } else if v + adt < target {
        v + adt
    } else {
        target
    }
}

impl ViewAnimator for SpeedingViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        // 3-branch acceleration per dimension
        let (vx, vy, vz) = self.inner.velocity();
        let friction = self.inner.friction();
        let friction_enabled = self.inner.is_friction_enabled();

        let new_vx = accelerate_dim(
            vx,
            self.target_vx,
            self.acceleration,
            self.reverse_acceleration,
            friction,
            friction_enabled,
            dt,
        );
        let new_vy = accelerate_dim(
            vy,
            self.target_vy,
            self.acceleration,
            self.reverse_acceleration,
            friction,
            friction_enabled,
            dt,
        );
        let new_vz = accelerate_dim(
            vz,
            self.target_vz,
            self.acceleration,
            self.reverse_acceleration,
            friction,
            friction_enabled,
            dt,
        );
        self.inner.set_velocity(new_vx, new_vy, new_vz);

        // Temporarily disable friction on inner (speeding handles it via acceleration)
        let saved_friction = self.inner.is_friction_enabled();
        self.inner.set_friction_enabled(false);
        self.inner.animate(view, tree, dt);
        self.inner.set_friction_enabled(saved_friction);

        // Idle check: target near zero and inner stopped
        if self.target_vx.abs() < 0.01
            && self.target_vy.abs() < 0.01
            && self.target_vz.abs() < 0.01
            && !self.inner.is_active()
        {
            self.active = false;
        }

        self.active
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.inner.stop();
        self.target_vx = 0.0;
        self.target_vy = 0.0;
        self.target_vz = 0.0;
        self.active = false;
    }
}

/// State for the visiting animator's seek/navigation progress.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum VisitingState {
    /// No goal set.
    NoGoal,
    /// Animating along a curved path.
    Curve,
    /// Animating directly toward the target.
    Direct,
    /// Seeking: waiting for panels to be lazily created.
    Seek,
    /// Seek failed, showing "Not found" overlay briefly.
    GivingUp,
    /// Terminal: gave up after showing overlay.
    GivenUp,
    /// Terminal: goal reached.
    GoalReached,
}

/// Result of walking the panel tree to find the nearest existing panel.
struct NearestPanel {
    panel: super::tree::PanelId,
    target_x: f64,
    target_y: f64,
    target_a: f64,
    depth: usize,
    panels_after: usize,
    dist_final: f64,
}

/// How the target panel should be displayed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum VisitType {
    /// Auto-position: compute natural viewing coordinates.
    Visit,
    /// Explicit relative coordinates (rel_x, rel_y, rel_a).
    VisitRel,
    /// Fill the viewport with the panel.
    VisitFullsized,
}

/// Visiting view animator — navigates the camera along an optimal-cost curve
/// to a target panel. Matches C++ `emVisitingViewAnimator`.
///
/// The curve minimizes visual travel cost: scrolling at high zoom costs more
/// than scrolling at low zoom, so the optimal path zooms out, scrolls, then
/// zooms in — producing the characteristic Eagle Mode navigation feel.
///
/// Uses precomputed curve tables for the optimal (scroll, zoom) path, with
/// Catmull-Rom spline interpolation and speed management (acceleration,
/// cusp speed limits, distance-based deceleration).
pub struct VisitingViewAnimator {
    // Configuration
    animated: bool,
    acceleration: f64,
    max_cusp_speed: f64,
    max_absolute_speed: f64,

    // Goal
    state: VisitingState,
    visit_type: VisitType,
    identity: String,
    names: Vec<String>,
    rel_x: f64,
    rel_y: f64,
    rel_a: f64,
    adherent: bool,
    utilize_view: bool,
    subject: String,

    // Animation state
    active: bool,
    max_depth_seen: i32,
    speed: f64,
    time_slices_without_hope: u32,
    give_up_clock: f64,
}

impl VisitingViewAnimator {
    pub fn new(target_x: f64, target_y: f64, target_a: f64, _speed: f64) -> Self {
        Self {
            animated: true,
            acceleration: 5.0,
            max_cusp_speed: 2.0,
            max_absolute_speed: 5.0,
            state: VisitingState::Curve,
            visit_type: VisitType::VisitRel,
            identity: String::new(),
            names: Vec::new(),
            rel_x: target_x,
            rel_y: target_y,
            rel_a: target_a,
            adherent: false,
            utilize_view: false,
            subject: String::new(),
            active: true,
            max_depth_seen: -1,
            speed: 0.0,
            time_slices_without_hope: 0,
            give_up_clock: 0.0,
        }
    }

    /// Configure animation parameters from a speed config value.
    ///
    /// Mirrors C++ `emVisitingViewAnimator::SetAnimParamsByCoreConfig`.
    pub fn set_anim_params_by_speed_config(&mut self, speed_factor: f64, max_speed_factor: f64) {
        self.animated = speed_factor < max_speed_factor * 0.99999;
        self.acceleration = 35.0 * speed_factor;
        self.max_absolute_speed = 35.0 * speed_factor;
        self.max_cusp_speed = self.max_absolute_speed * 0.5;
    }

    pub fn set_animated(&mut self, animated: bool) {
        self.animated = animated;
    }

    pub fn set_acceleration(&mut self, acceleration: f64) {
        self.acceleration = acceleration;
    }

    pub fn set_max_cusp_speed(&mut self, max_cusp_speed: f64) {
        self.max_cusp_speed = max_cusp_speed;
    }

    pub fn set_max_absolute_speed(&mut self, max_absolute_speed: f64) {
        self.max_absolute_speed = max_absolute_speed;
    }

    #[cfg(test)]
    fn visiting_state(&self) -> VisitingState {
        self.state
    }

    #[cfg(test)]
    fn set_visiting_state(&mut self, state: VisitingState) {
        self.state = state;
    }

    /// Set goal: visit panel by identity path with auto-positioning.
    pub fn set_goal(&mut self, identity: &str, adherent: bool, subject: &str) {
        self.visit_type = VisitType::Visit;
        self.rel_x = 0.0;
        self.rel_y = 0.0;
        self.rel_a = 0.0;
        self.adherent = adherent;
        self.utilize_view = false;
        self.subject = subject.to_string();
        self.activate_goal(identity);
    }

    /// Set goal: visit panel at explicit relative coordinates.
    pub fn set_goal_rel(
        &mut self,
        identity: &str,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        adherent: bool,
        subject: &str,
    ) {
        self.visit_type = VisitType::VisitRel;
        self.rel_x = rel_x;
        self.rel_y = rel_y;
        self.rel_a = rel_a;
        self.adherent = adherent;
        self.utilize_view = false;
        self.subject = subject.to_string();
        self.activate_goal(identity);
    }

    /// Set goal: visit panel fullsized.
    pub fn set_goal_fullsized(
        &mut self,
        identity: &str,
        adherent: bool,
        utilize_view: bool,
        subject: &str,
    ) {
        self.visit_type = VisitType::VisitFullsized;
        self.rel_x = 0.0;
        self.rel_y = 0.0;
        self.rel_a = 0.0;
        self.adherent = adherent;
        self.utilize_view = utilize_view;
        self.subject = subject.to_string();
        self.activate_goal(identity);
    }

    fn activate_goal(&mut self, identity: &str) {
        if self.state == VisitingState::NoGoal || self.identity != identity {
            self.state = VisitingState::Curve;
            self.identity = identity.to_string();
            self.names = super::tree::decode_identity(&self.identity);
            self.max_depth_seen = -1;
            self.time_slices_without_hope = 0;
            self.give_up_clock = 0.0;
        }
    }

    /// Clear the goal and stop animation.
    pub fn clear_goal(&mut self) {
        if self.state != VisitingState::NoGoal {
            self.state = VisitingState::NoGoal;
            self.visit_type = VisitType::Visit;
            self.identity.clear();
            self.names.clear();
            self.rel_x = 0.0;
            self.rel_y = 0.0;
            self.rel_a = 0.0;
            self.adherent = false;
            self.utilize_view = false;
            self.subject.clear();
            self.max_depth_seen = -1;
            self.time_slices_without_hope = 0;
            self.give_up_clock = 0.0;
        }
    }

    /// Set the identity and subject for seek overlay display.
    pub fn set_identity(&mut self, identity: &str, subject: &str) {
        self.identity = identity.to_string();
        self.subject = subject.to_string();
        self.names = super::tree::decode_identity(&self.identity);
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Handle input during visiting animation.
    pub fn handle_input(&mut self, event: &crate::input::InputEvent) -> bool {
        if !self.active {
            return false;
        }
        if self.state != VisitingState::Seek && self.state != VisitingState::GivingUp {
            return false;
        }
        if event.key != crate::input::InputKey::MouseLeft
            || event.variant != crate::input::InputVariant::Move
        {
            self.active = false;
            self.state = VisitingState::GivenUp;
            return true;
        }
        false
    }

    /// Paint the seek progress overlay.
    pub fn paint_seek_overlay(&self, painter: &mut crate::render::Painter<'_>, view: &View) {
        if !self.active {
            return;
        }
        if self.state != VisitingState::Seek && self.state != VisitingState::GivingUp {
            return;
        }

        let (vw, vh) = view.viewport_size();
        let w = (vw.max(vh) * 0.6).min(vw);
        let mut h = w * 0.25;
        let f = vh * 0.8 / h;
        if f < 1.0 {
            h *= f;
        }
        let x = (vw - w) * 0.5;
        let y = (vh - h) * 0.5;

        let shadow_off = w * 0.03;
        painter.paint_round_rect(
            x + shadow_off,
            y + shadow_off,
            w,
            h,
            h * 0.2,
            crate::foundation::Color::rgba(0, 0, 0, 160),
        );
        painter.paint_round_rect(
            x,
            y,
            w,
            h,
            h * 0.2,
            crate::foundation::Color::rgba(34, 102, 153, 208),
        );

        let ch_size = h * 0.22;
        if self.state == VisitingState::GivingUp {
            painter.paint_text_boxed(
                x,
                y,
                w,
                h * 0.4,
                "Not found",
                ch_size,
                crate::foundation::Color::WHITE,
                crate::foundation::Color::TRANSPARENT,
                crate::render::TextAlignment::Center,
                crate::render::VAlign::Center,
                crate::render::TextAlignment::Center,
                0.5,
                false,
                0.0,
            );
            return;
        }

        let mut seeking_text = String::from("Seeking...");
        if !self.subject.is_empty() {
            seeking_text.push_str(" for ");
            seeking_text.push_str(&self.subject);
        }
        painter.paint_text_boxed(
            x,
            y,
            w,
            h * 0.4,
            &seeking_text,
            ch_size,
            crate::foundation::Color::WHITE,
            crate::foundation::Color::TRANSPARENT,
            crate::render::TextAlignment::Center,
            crate::render::VAlign::Center,
            crate::render::TextAlignment::Center,
            0.5,
            false,
            0.0,
        );

        let bar_x = x + w * 0.05;
        let bar_y = y + h * 0.45;
        let bar_w = w * 0.9;
        let bar_h = h * 0.15;

        let seek_id = view
            .seek_pos_panel()
            .map(|_| view.seek_pos_child_name())
            .unwrap_or("");
        let total_len = self.identity.len().max(1);
        let found_len = if !seek_id.is_empty() {
            self.identity
                .find(seek_id)
                .map(|pos| pos + seek_id.len())
                .unwrap_or(0)
                .min(total_len)
        } else {
            0
        };
        let progress = found_len as f64 / total_len as f64;

        if progress > 0.0 {
            painter.paint_rect(
                bar_x,
                bar_y,
                bar_w * progress,
                bar_h,
                crate::foundation::Color::rgba(136, 255, 136, 80),
                Color::TRANSPARENT,
            );
        }
        if progress < 1.0 {
            painter.paint_rect(
                bar_x + bar_w * progress,
                bar_y,
                bar_w * (1.0 - progress),
                bar_h,
                crate::foundation::Color::rgba(136, 136, 136, 80),
                Color::TRANSPARENT,
            );
        }

        let id_y = bar_y + bar_h + h * 0.02;
        let id_h = h * 0.15;
        let id_ch = ch_size * 0.6;
        painter.paint_text_boxed(
            x,
            id_y,
            w,
            id_h,
            &self.identity,
            id_ch,
            crate::foundation::Color::rgba(200, 200, 200, 180),
            crate::foundation::Color::TRANSPARENT,
            crate::render::TextAlignment::Center,
            crate::render::VAlign::Top,
            crate::render::TextAlignment::Center,
            0.3,
            false,
            0.0,
        );

        let abort_y = y + h * 0.8;
        let abort_h = h * 0.15;
        painter.paint_text_boxed(
            x,
            abort_y,
            w,
            abort_h,
            "Press any key to abort",
            id_ch,
            crate::foundation::Color::rgba(200, 200, 200, 128),
            crate::foundation::Color::TRANSPARENT,
            crate::render::TextAlignment::Center,
            crate::render::VAlign::Center,
            crate::render::TextAlignment::Center,
            0.5,
            false,
            0.0,
        );
    }
}

// ─── Optimal-cost curve math (C++ parity) ──────────────────────────────

/// A point on the optimal-cost curve: (x = scroll, z = log-zoom).
#[derive(Copy, Clone)]
struct CurvePoint {
    x: f64,
    z: f64,
}

/// Step size in curve-distance units between precomputed points.
const CURVE_DELTA_DIST: f64 = 0.0703125;

/// Precomputed optimal-cost curve: 128 points mapping arc-length distance
/// to (scroll_x, log_zoom_z) coordinates. The curve minimizes total cost
/// where scrolling at zoom level z costs exp(z) per unit distance.
/// Ported directly from C++ emVisitingViewAnimator::CurvePoints[].
const CURVE_POINTS: [CurvePoint; 128] = [
    CurvePoint {
        x: 0.000000000000,
        z: 0.00000000,
    },
    CurvePoint {
        x: 0.070196996568,
        z: 0.00246786,
    },
    CurvePoint {
        x: 0.139706409829,
        z: 0.00984731,
    },
    CurvePoint {
        x: 0.207867277855,
        z: 0.02206685,
    },
    CurvePoint {
        x: 0.274069721820,
        z: 0.03901038,
    },
    CurvePoint {
        x: 0.337775385698,
        z: 0.06052148,
    },
    CurvePoint {
        x: 0.398532523720,
        z: 0.08640897,
    },
    CurvePoint {
        x: 0.455985066529,
        z: 0.11645328,
    },
    CurvePoint {
        x: 0.509875667166,
        z: 0.15041328,
    },
    CurvePoint {
        x: 0.560043303236,
        z: 0.18803304,
    },
    CurvePoint {
        x: 0.606416423020,
        z: 0.22904841,
    },
    CurvePoint {
        x: 0.649002844597,
        z: 0.27319286,
    },
    CurvePoint {
        x: 0.687877659276,
        z: 0.32020270,
    },
    CurvePoint {
        x: 0.723170290571,
        z: 0.36982132,
    },
    CurvePoint {
        x: 0.755051666347,
        z: 0.42180262,
    },
    CurvePoint {
        x: 0.783722223449,
        z: 0.47591350,
    },
    CurvePoint {
        x: 0.809401221691,
        z: 0.53193559,
    },
    CurvePoint {
        x: 0.832317626300,
        z: 0.58966630,
    },
    CurvePoint {
        x: 0.852702640725,
        z: 0.64891924,
    },
    CurvePoint {
        x: 0.870783840857,
        z: 0.70952419,
    },
    CurvePoint {
        x: 0.886780775044,
        z: 0.77132669,
    },
    CurvePoint {
        x: 0.900901845307,
        z: 0.83418737,
    },
    CurvePoint {
        x: 0.913342265529,
        z: 0.89798109,
    },
    CurvePoint {
        x: 0.924282893555,
        z: 0.96259598,
    },
    CurvePoint {
        x: 0.933889748709,
        z: 1.02793239,
    },
    CurvePoint {
        x: 0.942314048229,
        z: 1.09390185,
    },
    CurvePoint {
        x: 0.949692621183,
        z: 1.16042604,
    },
    CurvePoint {
        x: 0.956148583577,
        z: 1.22743579,
    },
    CurvePoint {
        x: 0.961792181829,
        z: 1.29487016,
    },
    CurvePoint {
        x: 0.966721732461,
        z: 1.36267553,
    },
    CurvePoint {
        x: 0.971024603574,
        z: 1.43080482,
    },
    CurvePoint {
        x: 0.974778198188,
        z: 1.49921674,
    },
    CurvePoint {
        x: 0.978050911290,
        z: 1.56787514,
    },
    CurvePoint {
        x: 0.980903041613,
        z: 1.63674836,
    },
    CurvePoint {
        x: 0.983387646274,
        z: 1.70580876,
    },
    CurvePoint {
        x: 0.985551331675,
        z: 1.77503218,
    },
    CurvePoint {
        x: 0.987434978026,
        z: 1.84439754,
    },
    CurvePoint {
        x: 0.989074397556,
        z: 1.91388643,
    },
    CurvePoint {
        x: 0.990500928466,
        z: 1.98348284,
    },
    CurvePoint {
        x: 0.991741967860,
        z: 2.05317279,
    },
    CurvePoint {
        x: 0.992821447684,
        z: 2.12294410,
    },
    CurvePoint {
        x: 0.993760258098,
        z: 2.19278617,
    },
    CurvePoint {
        x: 0.994576622816,
        z: 2.26268978,
    },
    CurvePoint {
        x: 0.995286430928,
        z: 2.33264690,
    },
    CurvePoint {
        x: 0.995903529539,
        z: 2.40265055,
    },
    CurvePoint {
        x: 0.996439981325,
        z: 2.47269463,
    },
    CurvePoint {
        x: 0.996906290795,
        z: 2.54277387,
    },
    CurvePoint {
        x: 0.997311602787,
        z: 2.61288367,
    },
    CurvePoint {
        x: 0.997663876350,
        z: 2.68302002,
    },
    CurvePoint {
        x: 0.997970036920,
        z: 2.75317946,
    },
    CurvePoint {
        x: 0.998236109346,
        z: 2.82335896,
    },
    CurvePoint {
        x: 0.998467334097,
        z: 2.89355590,
    },
    CurvePoint {
        x: 0.998668268700,
        z: 2.96376798,
    },
    CurvePoint {
        x: 0.998842876218,
        z: 3.03399323,
    },
    CurvePoint {
        x: 0.998994602405,
        z: 3.10422992,
    },
    CurvePoint {
        x: 0.999126442933,
        z: 3.17447655,
    },
    CurvePoint {
        x: 0.999241001961,
        z: 3.24473182,
    },
    CurvePoint {
        x: 0.999340543132,
        z: 3.31499459,
    },
    CurvePoint {
        x: 0.999427033975,
        z: 3.38526388,
    },
    CurvePoint {
        x: 0.999502184543,
        z: 3.45553885,
    },
    CurvePoint {
        x: 0.999567481032,
        z: 3.52581873,
    },
    CurvePoint {
        x: 0.999624215036,
        z: 3.59610289,
    },
    CurvePoint {
        x: 0.999673508983,
        z: 3.66639077,
    },
    CurvePoint {
        x: 0.999716338261,
        z: 3.73668188,
    },
    CurvePoint {
        x: 0.999753550459,
        z: 3.80697579,
    },
    CurvePoint {
        x: 0.999785882091,
        z: 3.87727215,
    },
    CurvePoint {
        x: 0.999813973141,
        z: 3.94757062,
    },
    CurvePoint {
        x: 0.999838379703,
        z: 4.01787093,
    },
    CurvePoint {
        x: 0.999859584970,
        z: 4.08817284,
    },
    CurvePoint {
        x: 0.999878008788,
        z: 4.15847614,
    },
    CurvePoint {
        x: 0.999894015951,
        z: 4.22878064,
    },
    CurvePoint {
        x: 0.999907923421,
        z: 4.29908620,
    },
    CurvePoint {
        x: 0.999920006597,
        z: 4.36939266,
    },
    CurvePoint {
        x: 0.999930504759,
        z: 4.43969992,
    },
    CurvePoint {
        x: 0.999939625809,
        z: 4.51000786,
    },
    CurvePoint {
        x: 0.999947550381,
        z: 4.58031641,
    },
    CurvePoint {
        x: 0.999954435420,
        z: 4.65062547,
    },
    CurvePoint {
        x: 0.999960417283,
        z: 4.72093498,
    },
    CurvePoint {
        x: 0.999965614444,
        z: 4.79124489,
    },
    CurvePoint {
        x: 0.999970129838,
        z: 4.86155513,
    },
    CurvePoint {
        x: 0.999974052897,
        z: 4.93186567,
    },
    CurvePoint {
        x: 0.999977461322,
        z: 5.00217647,
    },
    CurvePoint {
        x: 0.999980422622,
        z: 5.07248749,
    },
    CurvePoint {
        x: 0.999982995451,
        z: 5.14279871,
    },
    CurvePoint {
        x: 0.999985230769,
        z: 5.21311009,
    },
    CurvePoint {
        x: 0.999987172851,
        z: 5.28342162,
    },
    CurvePoint {
        x: 0.999988860164,
        z: 5.35373328,
    },
    CurvePoint {
        x: 0.999990326129,
        z: 5.42404505,
    },
    CurvePoint {
        x: 0.999991599784,
        z: 5.49435691,
    },
    CurvePoint {
        x: 0.999992706355,
        z: 5.56466886,
    },
    CurvePoint {
        x: 0.999993667762,
        z: 5.63498088,
    },
    CurvePoint {
        x: 0.999994503048,
        z: 5.70529296,
    },
    CurvePoint {
        x: 0.999995228757,
        z: 5.77560510,
    },
    CurvePoint {
        x: 0.999995859265,
        z: 5.84591728,
    },
    CurvePoint {
        x: 0.999996407059,
        z: 5.91622951,
    },
    CurvePoint {
        x: 0.999996882992,
        z: 5.98654177,
    },
    CurvePoint {
        x: 0.999997296489,
        z: 6.05685406,
    },
    CurvePoint {
        x: 0.999997655742,
        z: 6.12716639,
    },
    CurvePoint {
        x: 0.999997967867,
        z: 6.19747873,
    },
    CurvePoint {
        x: 0.999998239046,
        z: 6.26779109,
    },
    CurvePoint {
        x: 0.999998474650,
        z: 6.33810348,
    },
    CurvePoint {
        x: 0.999998679346,
        z: 6.40841587,
    },
    CurvePoint {
        x: 0.999998857189,
        z: 6.47872829,
    },
    CurvePoint {
        x: 0.999999011703,
        z: 6.54904071,
    },
    CurvePoint {
        x: 0.999999145946,
        z: 6.61935314,
    },
    CurvePoint {
        x: 0.999999262578,
        z: 6.68966558,
    },
    CurvePoint {
        x: 0.999999363911,
        z: 6.75997803,
    },
    CurvePoint {
        x: 0.999999451949,
        z: 6.83029049,
    },
    CurvePoint {
        x: 0.999999528439,
        z: 6.90060295,
    },
    CurvePoint {
        x: 0.999999594894,
        z: 6.97091542,
    },
    CurvePoint {
        x: 0.999999652632,
        z: 7.04122789,
    },
    CurvePoint {
        x: 0.999999702795,
        z: 7.11154036,
    },
    CurvePoint {
        x: 0.999999746377,
        z: 7.18185284,
    },
    CurvePoint {
        x: 0.999999784242,
        z: 7.25216532,
    },
    CurvePoint {
        x: 0.999999817140,
        z: 7.32247781,
    },
    CurvePoint {
        x: 0.999999845722,
        z: 7.39279029,
    },
    CurvePoint {
        x: 0.999999870554,
        z: 7.46310278,
    },
    CurvePoint {
        x: 0.999999892129,
        z: 7.53341527,
    },
    CurvePoint {
        x: 0.999999910874,
        z: 7.60372776,
    },
    CurvePoint {
        x: 0.999999927159,
        z: 7.67404025,
    },
    CurvePoint {
        x: 0.999999941309,
        z: 7.74435274,
    },
    CurvePoint {
        x: 0.999999953602,
        z: 7.81466524,
    },
    CurvePoint {
        x: 0.999999964282,
        z: 7.88497773,
    },
    CurvePoint {
        x: 0.999999973561,
        z: 7.95529023,
    },
    CurvePoint {
        x: 0.999999981623,
        z: 8.02560272,
    },
    CurvePoint {
        x: 0.999999988627,
        z: 8.09591522,
    },
    CurvePoint {
        x: 0.999999994713,
        z: 8.16622772,
    },
    CurvePoint {
        x: 1.000000000000,
        z: 8.23654021,
    },
];

const CURVE_MAX_INDEX: usize = CURVE_POINTS.len() - 1;

/// Interpolate a point on the optimal-cost curve at arc-length distance `d`.
/// Uses quadratic Bézier interpolation with Catmull-Rom tangent estimation.
fn get_curve_point(d: f64) -> CurvePoint {
    let max_d = CURVE_MAX_INDEX as f64 * CURVE_DELTA_DIST;
    if d.abs() >= max_d {
        let mut cp = CURVE_POINTS[CURVE_MAX_INDEX];
        if d < 0.0 {
            cp.x = -cp.x;
        }
        cp.z += d.abs() - max_d;
        return cp;
    }

    let t_raw = d.abs() / CURVE_DELTA_DIST;
    let (i, t) = if t_raw.is_nan() || t_raw <= 0.0 {
        (0, 0.0)
    } else if t_raw >= CURVE_MAX_INDEX as f64 {
        (CURVE_MAX_INDEX - 1, 1.0)
    } else {
        let i = (t_raw as usize).min(CURVE_MAX_INDEX - 1);
        (i, t_raw - i as f64)
    };

    let x1 = CURVE_POINTS[i].x;
    let z1 = CURVE_POINTS[i].z;
    let x2 = CURVE_POINTS[i + 1].x;
    let z2 = CURVE_POINTS[i + 1].z;

    // Catmull-Rom tangent estimation
    let (dx1, dz1) = if i == 0 {
        (CURVE_DELTA_DIST * 0.5, 0.0)
    } else {
        (
            (x2 - CURVE_POINTS[i - 1].x) * 0.25,
            (z2 - CURVE_POINTS[i - 1].z) * 0.25,
        )
    };

    let (dx2, dz2) = if i + 2 > CURVE_MAX_INDEX {
        (0.0, CURVE_DELTA_DIST * 0.5)
    } else {
        (
            (CURVE_POINTS[i + 2].x - x1) * 0.25,
            (CURVE_POINTS[i + 2].z - z1) * 0.25,
        )
    };

    // Quadratic Bézier with mid-control point from tangents
    let x3 = (x1 + dx1 + x2 - dx2) * 0.5;
    let z3 = (z1 + dz1 + z2 - dz2) * 0.5;

    let c1 = (1.0 - t) * (1.0 - t);
    let c2 = t * t;
    let c3 = 2.0 * t * (1.0 - t);

    let mut cp = CurvePoint {
        x: x1 * c1 + x2 * c2 + x3 * c3,
        z: z1 * c1 + z2 * c2 + z3 * c3,
    };
    if d < 0.0 {
        cp.x = -cp.x;
    }
    cp
}

/// Find curve position and remaining distance for target at (x, z).
/// Returns (curve_pos, curve_dist) where curve_pos is arc-length position
/// on the curve at the "start" point, and curve_dist is the remaining
/// arc-length to the "end" point.
fn get_curve_pos_dist(mut x: f64, mut z: f64) -> (f64, f64) {
    let mut neg = false;
    let mut swap = false;

    if z < 0.0 {
        z = -z;
        x /= z.exp();
        neg = true;
        swap = true;
    }
    if x < 0.0 {
        x = -x;
        neg = !neg;
    }

    let max_curve_d = CURVE_MAX_INDEX as f64 * CURVE_DELTA_DIST;

    // Binary search for curve position `a` and end position `b`
    let mut a_min = -x;
    let mut a_max = max_curve_d;

    let mut a = 0.0_f64;
    let mut tp = CurvePoint { x: 0.0, z: 0.0 };

    for i in 0..49 {
        a = (a_min + a_max) * 0.5;
        let ap = get_curve_point(a);
        tp.x = ap.x + x / ap.z.exp();
        tp.z = ap.z + z;

        if a_max - a_min < 1e-12 || i >= 48 {
            break;
        }
        if tp.x <= 0.0 {
            a_min = a;
            continue;
        }
        if tp.x >= CURVE_POINTS[CURVE_MAX_INDEX].x {
            a_max = a;
            continue;
        }

        let mut b_min = tp.z;
        let mut b_max = tp.z + tp.x;
        for j in 0..49 {
            let b = (b_min + b_max) * 0.5;
            let bp = get_curve_point(b);
            if b_max - b_min < 1e-12 || j >= 48 {
                break;
            }
            if tp.z > bp.z {
                if tp.x <= bp.x {
                    break;
                }
                b_min = b;
            } else {
                if tp.x >= bp.x {
                    break;
                }
                b_max = b;
            }
        }
        let bp = get_curve_point((b_min + b_max) * 0.5);
        if tp.z > bp.z {
            a_min = a;
        } else {
            a_max = a;
        }
    }

    // Final binary search for b
    let mut b_min = tp.z;
    let mut b_max = tp.z + tp.x;
    if b_min < a + z {
        b_min = a + z;
    }
    if b_max < b_min {
        b_max = b_min;
    }
    for j in 0..49 {
        let b = (b_min + b_max) * 0.5;
        if b_max - b_min < 1e-12 || j >= 48 {
            break;
        }
        let bp = get_curve_point(b);
        if tp.z > bp.z {
            b_min = b;
        } else {
            b_max = b;
        }
    }
    let b = (b_min + b_max) * 0.5;

    let (mut a_out, mut b_out) = (a, b);
    if neg {
        a_out = -a_out;
        b_out = -b_out;
    }
    if swap {
        (b_out, a_out - b_out)
    } else {
        (a_out, b_out - a_out)
    }
}

/// Direct-line distance from origin to (x, z) in the cost metric.
fn get_direct_dist(x: f64, z: f64) -> f64 {
    if z.abs() < 0.1 {
        (x * x + z * z).sqrt()
    } else {
        let fix_x = x / (1.0 - (-z).exp());
        z.abs() * (fix_x * fix_x + 1.0).sqrt()
    }
}

/// Point on direct line from origin to (x, z) at distance d.
fn get_direct_point(x: f64, z: f64, d: f64) -> (f64, f64) {
    if z.abs() < 0.1 {
        let dist = (x * x + z * z).sqrt();
        let t = if dist < 1e-100 { 0.0 } else { d / dist };
        (x * t, z * t)
    } else {
        let fix_x = x / (1.0 - (-z).exp());
        let dist = z.abs() * (fix_x * fix_x + 1.0).sqrt();
        let t = d / dist;
        (fix_x * (1.0 - (-z * t).exp()), z * t)
    }
}

// ─── Speed management ──────────────────────────────────────────

impl VisitingViewAnimator {
    fn update_speed(&mut self, pos: f64, dist: f64, panels_after: usize, dist_final: f64, dt: f64) {
        self.speed += self.acceleration * dt;

        // Limit by remaining distance (avoid overshoot)
        let s = (dist + panels_after as f64 * 2.0_f64.ln() + dist_final).max(0.0);
        let v = (self.acceleration * s * 2.0).sqrt();
        if self.speed > v {
            self.speed = v;
        }

        // Limit at cusp
        if pos < 0.0 {
            let v = (self.acceleration * (-pos) * 2.0 + self.max_cusp_speed * self.max_cusp_speed)
                .sqrt();
            if self.speed > v {
                self.speed = v;
            }
        }

        if self.speed > self.max_absolute_speed {
            self.speed = self.max_absolute_speed;
        }

        if self.speed > dist / dt {
            self.speed = dist / dt;
        }
    }

    /// Walk the panel tree identity path, returning the deepest existing panel,
    /// target coordinates, depth reached, and panels remaining.
    fn get_nearest_existing_panel(&self, view: &View, tree: &PanelTree) -> Option<NearestPanel> {
        let root = view.root();
        if self.names.is_empty() {
            return None;
        }
        let root_name = tree.get(root).map(|p| p.name.as_str()).unwrap_or("");
        if self.names[0] != root_name {
            return None;
        }

        let mut panel = root;
        let mut i = 1;
        while i < self.names.len() {
            if let Some(child) = tree.find_child_by_name(panel, &self.names[i]) {
                panel = child;
                i += 1;
            } else {
                break;
            }
        }

        let depth = i - 1;
        let panels_after = self.names.len() - i;

        let (target_x, target_y, target_a, dist_final);
        if panels_after > 0 {
            // Didn't reach the goal — visit nearest existing panel fullsized
            let coords = view.calc_visit_fullsized_coords(tree, panel, false);
            target_x = coords.0;
            target_y = coords.1;
            target_a = coords.2;
            dist_final = match self.visit_type {
                VisitType::VisitRel if self.rel_a > 0.0 && self.rel_a < 1.0 => {
                    (1.0 / self.rel_a.sqrt()).ln()
                }
                _ => 0.0,
            };
        } else {
            // Reached the goal panel
            match self.visit_type {
                VisitType::Visit => {
                    let coords = view.calc_visit_coords(tree, panel);
                    target_x = coords.0;
                    target_y = coords.1;
                    target_a = coords.2;
                }
                VisitType::VisitRel => {
                    if self.rel_a <= 0.0 {
                        let coords =
                            view.calc_visit_fullsized_coords(tree, panel, self.rel_a < -0.9);
                        target_x = coords.0;
                        target_y = coords.1;
                        target_a = coords.2;
                    } else {
                        target_x = self.rel_x;
                        target_y = self.rel_y;
                        target_a = self.rel_a;
                    }
                }
                VisitType::VisitFullsized => {
                    let coords = view.calc_visit_fullsized_coords(tree, panel, self.utilize_view);
                    target_x = coords.0;
                    target_y = coords.1;
                    target_a = coords.2;
                }
            }
            dist_final = 0.0;
        }

        Some(NearestPanel {
            panel,
            target_x,
            target_y,
            target_a,
            depth,
            panels_after,
            dist_final,
        })
    }

    /// Compute 3D distance from current view state to target panel coordinates.
    ///
    /// Matches C++ `emVisitingViewAnimator::GetDistanceTo`: converts both current
    /// and target view positions into a common ancestor panel's coordinate system
    /// by walking the panel tree, then computes scroll/zoom distance in curve space.
    ///
    /// Returns (dir_x, dir_y, dist_xy, dist_z).
    fn get_distance_to(
        &self,
        view: &View,
        tree: &PanelTree,
        panel: super::tree::PanelId,
        target_x: f64,
        target_y: f64,
        target_a: f64,
    ) -> (f64, f64, f64, f64) {
        // Home rectangle (C++ HomeX/Y/Width/Height). Rust View always has
        // home at (0,0) with viewport dimensions.
        let hw = view.viewport_size().0.max(1.0);
        let hh = view.viewport_size().1.max(1.0);
        let hx = 0.0_f64;
        let hy = 0.0_f64;
        // C++ HomePixelTallness — always 1.0 in Rust (square pixels).
        let hp = 1.0_f64;

        // View rectangle (C++ GetViewRect): popup-zoom uses max_popup_rect,
        // otherwise same as home rect.
        let (sx, sy, sw, sh) = if view.flags.contains(super::view::ViewFlags::POPUP_ZOOM) {
            if let Some(r) = view.max_popup_rect() {
                (r.x, r.y, r.w, r.h)
            } else {
                (hx, hy, hw, hh)
            }
        } else {
            (hx, hy, hw, hh)
        };

        // Panel height (tallness) for the target panel.
        let panel_height = tree.get_height(panel);

        // Rectangle "b": where the view should end up, in target-panel coords.
        // C++: vw = sqrt(hw*hh*hp / (relA * panel->GetHeight()))
        // Rust rel_a = 1/C++relA, so relA = 1/target_a.
        let vw = (hw * hh * hp * target_a / panel_height).sqrt();
        let vh = vw * panel_height / hp;
        // Rust rel_x/y uses viewport-fraction convention:
        //   vcx = vw_viewport * (0.5 + rel_x)
        // C++ uses: vx = hx + hw*0.5 - (relX+0.5)*ViewedWidth
        // In Rust: the panel center is at vcx = hw*(0.5+target_x), so
        //   panel_vx = vcx - vw*0.5 = hw*(0.5+target_x) - vw*0.5
        //   vx(C++) = hx + hw*0.5 - (relX+0.5)*vw
        // For C++ relX=0 (centered): vx = hx + hw*0.5 - 0.5*vw = hw*0.5 - vw*0.5
        // For Rust rel_x=0 (centered): panel_vx = hw*0.5 - vw*0.5 ✓
        // Mapping: hw*(0.5+target_x) - vw*0.5 = hx + hw*0.5 - (relX+0.5)*vw
        //   => relX = (vw*0.5 - hw*target_x) / vw - 0.5
        //          = - hw*target_x / vw
        // But we don't need to convert: we can compute vx directly from Rust coords.
        let vx = hx + hw * (0.5 + target_x) - vw * 0.5;
        let vy = hy + hh * (0.5 + target_y) - vh * 0.5;
        let mut bx = (sx - vx) / vw;
        let mut by = (sy - vy) / vw * hp;
        let mut bw = sw / vw;
        let mut bh = sh / vw * hp;

        // Walk "b" up the panel tree until we reach a panel that is
        // in_viewed_path and whose parent is NOT viewed (i.e., the SVP),
        // or until we reach the root.
        let mut b_id = panel;
        while let Some(b_data) = tree.get(b_id) {
            let parent_id = match b_data.parent {
                Some(p) => p,
                None => break, // root reached
            };
            if b_data.in_viewed_path {
                let parent_viewed = tree
                    .get(parent_id)
                    .map(|p| p.viewed)
                    .unwrap_or(false);
                if !parent_viewed {
                    break;
                }
            }
            let lr = b_data.layout_rect;
            bx = lr.x + bx * lr.w;
            by = lr.y + by * lr.w;
            bw *= lr.w;
            bh *= lr.w;
            b_id = parent_id;
        }

        // Get SVP and rectangle "a" (current view position in SVP coords).
        let svp_id = view.svp().unwrap_or(view.root());
        let (svp_vx, svp_vy, svp_vw) = tree
            .get(svp_id)
            .map(|p| (p.viewed_x, p.viewed_y, p.viewed_width))
            .unwrap_or((0.0, 0.0, 1.0));

        let mut ax = (sx - svp_vx) / svp_vw;
        let mut ay = (sy - svp_vy) / svp_vw * hp;
        let mut aw = sw / svp_vw;
        let mut ah = sh / svp_vw * hp;

        // Walk "a" up from SVP until reaching "b_id" so both rectangles
        // are in the same panel's coordinate system.
        let mut a_id = svp_id;
        while a_id != b_id {
            let a_data = match tree.get(a_id) {
                Some(d) => d,
                None => break,
            };
            let lr = a_data.layout_rect;
            ax = lr.x + ax * lr.w;
            ay = lr.y + ay * lr.w;
            aw *= lr.w;
            ah *= lr.w;
            a_id = match a_data.parent {
                Some(p) => p,
                None => break, // shouldn't happen if tree is well-formed
            };
        }

        // Calculate 3D distance.
        let extreme_dist: f64 = 50.0;
        let dx;
        let dy;
        let dz;

        let t = aw + ah;
        if t < 1e-100 {
            dx = 0.0;
            dy = 0.0;
            dz = -extreme_dist;
        } else {
            let f = (sw + sh) * view.get_zoom_factor_log_per_pixel();
            // Negate: the tree-walk computes displacement in C++ sign convention
            // (positive = view rect to the right), but Rust raw_scroll_and_zoom
            // uses opposite rel_x sign convention (positive delta = increase rel_x
            // = panel moves right = viewport content goes left). Negating aligns
            // the distance direction with Rust scroll direction.
            dx = -(bx - ax + (bw - aw) * 0.5) / t * f;
            dy = -(by - ay + (bh - ah) * 0.5) / t * f;
            let ratio = (bw + bh) / t;
            if ratio < (-extreme_dist).exp() {
                dz = extreme_dist;
            } else if ratio > extreme_dist.exp() {
                dz = -extreme_dist;
            } else {
                dz = -ratio.ln();
            }
        }

        // Calculate 2D distance.
        let dxy = (dx * dx + dy * dy).sqrt();
        let (dir_x, dir_y) = if dxy < 1e-100 {
            (1.0, 0.0)
        } else {
            (dx / dxy, dy / dxy)
        };

        if dxy > extreme_dist.exp() {
            (dir_x, dir_y, 0.0, -extreme_dist)
        } else {
            (dir_x, dir_y, dxy, dz)
        }
    }
}

impl ViewAnimator for VisitingViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        match self.state {
            VisitingState::NoGoal | VisitingState::GivenUp | VisitingState::GoalReached => {
                return false;
            }
            VisitingState::GivingUp => {
                self.give_up_clock += dt;
                if self.give_up_clock > 1.5 {
                    self.state = VisitingState::GivenUp;
                    return false;
                }
                return true;
            }
            VisitingState::Curve | VisitingState::Direct | VisitingState::Seek => {}
        }

        // Find nearest existing panel on the identity path
        let nep = match self.get_nearest_existing_panel(view, tree) {
            Some(v) => v,
            None => {
                self.state = VisitingState::GivingUp;
                self.give_up_clock = 0.0;
                return true;
            }
        };

        if self.animated {
            if self.max_depth_seen < nep.depth as i32 {
                if self.state == VisitingState::Seek {
                    view.set_seek_pos(tree, None, "");
                    self.state = VisitingState::Curve;
                }
                self.max_depth_seen = nep.depth as i32;
            }
        } else {
            self.state = VisitingState::Seek;
            if self.max_depth_seen < nep.depth as i32 {
                self.max_depth_seen = nep.depth as i32;
            }
        }

        if self.state == VisitingState::Curve || self.state == VisitingState::Direct {
            let (dir_x, dir_y, dist_xy, dist_z) = self.get_distance_to(
                view,
                tree,
                nep.panel,
                nep.target_x,
                nep.target_y,
                nep.target_a,
            );

            let (curve_pos, curve_dist) = if self.state == VisitingState::Direct {
                (0.0, get_direct_dist(dist_xy, dist_z))
            } else {
                get_curve_pos_dist(dist_xy, dist_z)
            };

            self.update_speed(curve_pos, curve_dist, nep.panels_after, nep.dist_final, dt);

            let (delta_xy, delta_z) = if self.state == VisitingState::Direct {
                get_direct_point(dist_xy, dist_z, self.speed * dt)
            } else {
                let cp1 = get_curve_point(curve_pos);
                let cp2 = get_curve_point(curve_pos + self.speed * dt);
                ((cp2.x - cp1.x) * cp1.z.exp(), cp2.z - cp1.z)
            };

            // Convert curve deltas back to raw_scroll_and_zoom units.
            // Scroll: curve scroll distance → pixel scroll via /zflpp (matching C++).
            // Zoom: curve z-delta → deltaZ for raw_scroll_and_zoom (matching C++
            // convention: reFac = exp(-deltaZ * zflpp), ra *= reFac^2).
            let zflpp = view.get_zoom_factor_log_per_pixel();
            let delta_xy_px = delta_xy / zflpp;
            let delta_z_view = delta_z / zflpp;
            let delta_x = dir_x * delta_xy_px;
            let delta_y = dir_y * delta_xy_px;

            let (vw, vh) = view.viewport_size();
            let done =
                view.raw_scroll_and_zoom(tree, vw * 0.5, vh * 0.5, delta_x, delta_y, delta_z_view);

            let delta_mag =
                (delta_x * delta_x + delta_y * delta_y + delta_z_view * delta_z_view).sqrt();
            let done_mag = (done[0] * done[0] + done[1] * done[1] + done[2] * done[2]).sqrt();

            if curve_dist <= 1e-6 {
                if nep.panels_after > 0 {
                    self.state = VisitingState::Seek;
                } else {
                    self.state = VisitingState::GoalReached;
                    return false;
                }
            } else if done_mag < delta_mag * 0.2 {
                if self.state == VisitingState::Curve {
                    self.state = VisitingState::Direct;
                } else {
                    self.state = VisitingState::Seek;
                }
            }
        }

        if self.state == VisitingState::Seek {
            if nep.depth + 1 >= self.names.len() {
                // All panels exist — visit the target
                view.visit(nep.panel, nep.target_x, nep.target_y, nep.target_a);
                view.update_viewing(tree);
                self.state = VisitingState::GoalReached;
                return false;
            } else if view.seek_pos_panel() != Some(nep.panel) {
                view.set_seek_pos(tree, Some(nep.panel), &self.names[nep.depth + 1]);
                view.visit_fullsized(tree, nep.panel);
                view.update_viewing(tree);
                self.time_slices_without_hope = 4;
            } else if view.is_hope_for_seeking(tree) {
                self.time_slices_without_hope = 0;
            } else {
                self.time_slices_without_hope += 1;
                if self.time_slices_without_hope > 10 {
                    self.state = VisitingState::GivingUp;
                    self.give_up_clock = 0.0;
                }
            }
        }

        true
    }

    fn is_active(&self) -> bool {
        self.active
            && !matches!(
                self.state,
                VisitingState::NoGoal | VisitingState::GoalReached | VisitingState::GivenUp
            )
    }

    fn stop(&mut self) {
        self.active = false;
        self.speed = 0.0;
        self.state = VisitingState::NoGoal;
    }
}

/// Swiping view animator — spring-based drag with kinetic coasting.
///
/// Matches C++ `emSwipingViewAnimator` architecture: composes a
/// `KineticViewAnimator` with a critically-damped spring model.
///
/// **Gripped**: User drags the view. Accumulated grip distance is stored as
/// spring extension. Each frame, the spring converts extension into kinetic
/// velocity. Friction is disabled during grip.
///
/// **Released**: Spring extension zeroed, velocity transferred to kinetic
/// animator which coasts with friction deceleration.
///
/// Supports 3D (scroll X, scroll Y, zoom Z).
pub struct SwipingViewAnimator {
    inner: KineticViewAnimator,
    gripped: bool,
    spring_extension: [f64; 3],
    instantaneous_velocity: [f64; 3],
    spring_constant: f64,
    busy: bool,
}

impl SwipingViewAnimator {
    pub fn new(friction: f64) -> Self {
        Self {
            inner: KineticViewAnimator::new(0.0, 0.0, 0.0, friction),
            gripped: false,
            spring_extension: [0.0; 3],
            instantaneous_velocity: [0.0; 3],
            spring_constant: 1.0,
            busy: false,
        }
    }

    /// Toggle grip state. On release, spring extension is zeroed and
    /// instantaneous velocity copies from kinetic velocity for coasting.
    pub fn set_gripped(&mut self, gripped: bool) {
        if self.gripped != gripped {
            self.gripped = gripped;
            if !self.gripped {
                self.spring_extension = [0.0; 3];
                let (vx, vy, vz) = self.inner.velocity();
                self.instantaneous_velocity = [vx, vy, vz];
            }
        }
    }

    /// Whether the view is currently gripped.
    pub fn is_gripped(&self) -> bool {
        self.gripped
    }

    /// Add distance to spring extension in the given dimension (0=X, 1=Y, 2=Z).
    pub fn move_grip(&mut self, dimension: usize, distance: f64) {
        if self.gripped && dimension < 3 {
            self.spring_extension[dimension] += distance;
            self.update_busy_state();
        }
    }

    /// Set the spring constant (stiffness). Higher = stiffer, less lag.
    pub fn set_spring_constant(&mut self, k: f64) {
        self.spring_constant = k;
    }

    /// Absolute spring extension magnitude.
    pub fn abs_spring_extension(&self) -> f64 {
        (self.spring_extension[0] * self.spring_extension[0]
            + self.spring_extension[1] * self.spring_extension[1]
            + self.spring_extension[2] * self.spring_extension[2])
            .sqrt()
    }

    /// Access the inner kinetic animator.
    pub fn inner(&self) -> &KineticViewAnimator {
        &self.inner
    }

    /// Mutable access to the inner kinetic animator.
    pub fn inner_mut(&mut self) -> &mut KineticViewAnimator {
        &mut self.inner
    }

    fn update_busy_state(&mut self) {
        if self.gripped && (self.abs_spring_extension() > 0.01 || self.inner.abs_velocity() > 0.01)
        {
            self.busy = true;
        } else {
            self.spring_extension = [0.0; 3];
            self.busy = false;
        }
    }
}

impl ViewAnimator for SwipingViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        let base_busy;

        if self.busy && self.gripped {
            // Critically-damped spring physics per dimension.
            // Converts spring extension into kinetic velocity.
            let mut new_vel = [0.0_f64; 3];

            for ((ext, inst_vel), nv) in self
                .spring_extension
                .iter_mut()
                .zip(self.instantaneous_velocity.iter_mut())
                .zip(new_vel.iter_mut())
            {
                let e1 = *ext;
                let v1 = *inst_vel;

                let (e2, v2) = if self.spring_constant < 1e5 && (e1 / dt).abs() > 20.0 {
                    // Critically damped spring:
                    //   e(t) = (e₀ + (e₀ω - v₀)t) · exp(-ωt)
                    //   v(t) = (v₀ + (e₀ω - v₀)ωt) · exp(-ωt)
                    // (C++ sign convention: v is retraction velocity)
                    let w = self.spring_constant.sqrt();
                    let decay = (-w * dt).exp();
                    (
                        (e1 + (e1 * w - v1) * dt) * decay,
                        (v1 + (e1 * w - v1) * dt * w) * decay,
                    )
                } else {
                    // Spring is too stiff or extension rate too low — snap rigid
                    (0.0, 0.0)
                };

                *ext = e2;
                *inst_vel = v2;
                *nv = (e1 - e2) / dt;
            }

            self.inner.set_velocity(new_vel[0], new_vel[1], new_vel[2]);

            // Disable friction during grip, delegate to kinetic for scroll/zoom
            let saved_friction = self.inner.is_friction_enabled();
            self.inner.set_friction_enabled(false);
            base_busy = self.inner.animate(view, tree, dt);
            self.inner.set_friction_enabled(saved_friction);
        } else {
            // Not gripped or not busy — pure kinetic coasting with friction
            base_busy = self.inner.animate(view, tree, dt);
        }

        self.update_busy_state();
        self.busy || base_busy
    }

    fn is_active(&self) -> bool {
        self.busy || self.inner.is_active()
    }

    fn stop(&mut self) {
        self.inner.stop();
        self.gripped = false;
        self.spring_extension = [0.0; 3];
        self.instantaneous_velocity = [0.0; 3];
        self.busy = false;
    }
}

/// Magnetic view animator — snaps the view to the nearest panel boundary.
///
/// After another animation settles, this applies a spring-like force toward
/// the nearest snap point. The displacement from the snap target is multiplied
/// by a spring constant and applied as velocity, producing a smooth settle.
pub struct MagneticViewAnimator {
    /// Spring constant controlling snap strength.
    spring_constant: f64,
    /// Current snap velocity.
    velocity_x: f64,
    velocity_y: f64,
    /// Target snap position (set externally when a snap point is identified).
    snap_target_x: f64,
    snap_target_y: f64,
    /// Whether snapping is active.
    active: bool,
    /// Damping factor to prevent oscillation (0..1).
    damping: f64,
}

impl MagneticViewAnimator {
    pub fn new(spring_constant: f64) -> Self {
        Self {
            spring_constant,
            velocity_x: 0.0,
            velocity_y: 0.0,
            snap_target_x: 0.0,
            snap_target_y: 0.0,
            active: false,
            damping: 0.8,
        }
    }

    /// Set the snap target position. Activates the animator.
    pub fn set_snap_target(&mut self, x: f64, y: f64) {
        self.snap_target_x = x;
        self.snap_target_y = y;
        self.active = true;
    }

    /// Set the spring constant.
    pub fn set_spring_constant(&mut self, k: f64) {
        self.spring_constant = k;
    }

    /// Set the damping factor (0..1, lower = more damping).
    pub fn set_damping(&mut self, d: f64) {
        self.damping = d.clamp(0.0, 1.0);
    }

    /// Current velocity.
    pub fn velocity(&self) -> (f64, f64) {
        (self.velocity_x, self.velocity_y)
    }
}

impl ViewAnimator for MagneticViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        if let Some(state) = view.visit_stack().last().cloned() {
            let disp_x = self.snap_target_x - state.rel_x;
            let disp_y = self.snap_target_y - state.rel_y;

            // Spring force: F = k * displacement
            let force_x = self.spring_constant * disp_x;
            let force_y = self.spring_constant * disp_y;

            // Update velocity: v += F * dt, then apply damping
            self.velocity_x = (self.velocity_x + force_x * dt) * self.damping;
            self.velocity_y = (self.velocity_y + force_y * dt) * self.damping;

            let dx = self.velocity_x * dt * view.viewport_size().0.max(1.0);
            let dy = self.velocity_y * dt * view.viewport_size().1.max(1.0);

            if dx.abs() > 1e-8 || dy.abs() > 1e-8 {
                let (vw, vh) = view.viewport_size();
                view.raw_scroll_and_zoom(tree, vw * 0.5, vh * 0.5, dx, dy, 0.0);
            }

            // Converged when displacement and velocity are both tiny
            let dist = disp_x.abs() + disp_y.abs();
            let speed =
                (self.velocity_x * self.velocity_x + self.velocity_y * self.velocity_y).sqrt();
            if dist < 1e-6 && speed < 1e-6 {
                self.velocity_x = 0.0;
                self.velocity_y = 0.0;
                self.active = false;
                return false;
            }
        }

        self.active
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelTree;

    fn setup() -> (PanelTree, View) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
        let view = View::new(root, 800.0, 600.0);
        (tree, view)
    }

    #[test]
    fn kinetic_with_zoom() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);
        let initial_a = view.current_visit().rel_a;

        let mut anim = KineticViewAnimator::new(0.0, 0.0, 100.0, 1000.0);
        // friction_enabled defaults to false — just test that zoom scroll works
        anim.animate(&mut view, &mut tree, 0.1);

        // Zoom velocity should have changed rel_a (dz = 100 * 0.1 = 10)
        assert!((view.current_visit().rel_a - initial_a).abs() > 0.001);
    }

    #[test]
    fn speeding_with_zoom() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = SpeedingViewAnimator::new(1000.0);
        anim.set_target(0.0, 0.0, 2.0);

        for _ in 0..10 {
            anim.animate(&mut view, &mut tree, 0.016);
        }

        // Should be accelerating toward zoom
        let (_, _, vz) = anim.inner().velocity();
        assert!(vz.abs() > 0.0);
    }

    #[test]
    fn visiting_converges() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = VisitingViewAnimator::new(0.1, 0.1, 2.0, 10.0);
        anim.set_identity("root", "");
        anim.set_animated(true);
        anim.set_acceleration(5.0);
        anim.set_max_absolute_speed(5.0);

        for _ in 0..500 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                break;
            }
        }

        assert!(!anim.is_active());
    }

    #[test]
    fn kinetic_linear_friction_stops() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = KineticViewAnimator::new(100.0, 0.0, 0.0, 1000.0);
        anim.set_friction_enabled(true);

        for _ in 0..200 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                break;
            }
        }

        assert!(!anim.is_active());
    }

    #[test]
    fn kinetic_friction_disabled() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = KineticViewAnimator::new(100.0, 0.0, 0.0, 1000.0);
        // friction_enabled defaults to false

        anim.animate(&mut view, &mut tree, 0.016);

        let (vx, _, _) = anim.velocity();
        // Without friction, velocity should remain at 100.0 (or zeroed by blocked-motion)
        // but should NOT have been reduced by friction
        assert!(vx == 100.0 || vx == 0.0);
    }

    #[test]
    fn speeding_3branch_reverse() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = SpeedingViewAnimator::new(1000.0);
        anim.set_reverse_acceleration(500.0);

        // Set inner velocity going right (set_velocity activates if > 0.01)
        anim.inner_mut().set_velocity(100.0, 0.0, 0.0);
        // Target going left — should trigger reverse acceleration
        anim.set_target(-100.0, 0.0, 0.0);

        anim.animate(&mut view, &mut tree, 0.016);

        let (vx, _, _) = anim.inner().velocity();
        // Velocity should have moved toward -100 (decreased from 100)
        assert!(vx < 100.0);
    }

    #[test]
    fn speeding_delegates_to_kinetic() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);
        let initial_a = view.current_visit().rel_a;

        let mut anim = SpeedingViewAnimator::new(1000.0);
        anim.set_target(0.0, 0.0, 2.0);
        anim.set_acceleration(1000.0);

        for _ in 0..10 {
            anim.animate(&mut view, &mut tree, 0.016);
        }

        // Inner kinetic should have applied zoom via raw_scroll_and_zoom
        assert!((view.current_visit().rel_a - initial_a).abs() > 0.001);
    }

    #[test]
    fn visiting_set_anim_params() {
        let mut anim = VisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

        // Below max: animated
        anim.set_anim_params_by_speed_config(2.0, 10.0);
        assert!(anim.animated);
        assert!((anim.acceleration - 70.0).abs() < 0.01);
        assert!((anim.max_absolute_speed - 70.0).abs() < 0.01);
        assert!((anim.max_cusp_speed - 35.0).abs() < 0.01);

        // At max: not animated (instant)
        anim.set_anim_params_by_speed_config(10.0, 10.0);
        assert!(!anim.animated);
    }

    #[test]
    fn visiting_handle_input_abort() {
        let mut anim = VisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

        // Not in seek state — should not consume
        let event = crate::input::InputEvent::press(crate::input::InputKey::Escape);
        assert!(!anim.handle_input(&event));

        // Set to seek state
        anim.set_visiting_state(VisitingState::Seek);
        assert!(anim.handle_input(&event));
        assert!(!anim.is_active());
        assert_eq!(anim.visiting_state(), VisitingState::GivenUp);
    }

    #[test]
    fn visiting_state_direct_transitions() {
        let mut anim = VisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

        // Exercise all state variants to ensure they exist
        anim.set_visiting_state(VisitingState::NoGoal);
        assert_eq!(anim.visiting_state(), VisitingState::NoGoal);

        anim.set_visiting_state(VisitingState::Direct);
        assert_eq!(anim.visiting_state(), VisitingState::Direct);

        anim.set_visiting_state(VisitingState::GivingUp);
        assert_eq!(anim.visiting_state(), VisitingState::GivingUp);

        anim.set_visiting_state(VisitingState::GoalReached);
        assert_eq!(anim.visiting_state(), VisitingState::GoalReached);
    }

    #[test]
    fn visiting_goal_reached_on_target() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        // Target is the root at current coords — should reach goal quickly
        let state = view.current_visit().clone();
        let mut anim = VisitingViewAnimator::new(state.rel_x, state.rel_y, state.rel_a, 5.0);
        anim.set_identity("root", "");
        anim.set_animated(true);
        anim.set_acceleration(5.0);
        anim.set_max_absolute_speed(5.0);

        let mut reached = false;
        for _ in 0..20 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                reached = true;
                break;
            }
        }
        assert!(reached, "Should reach goal when already at target");
        assert_eq!(anim.visiting_state(), VisitingState::GoalReached);
    }

    #[test]
    fn visiting_giving_up_no_panel() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        // Target a non-existent panel
        let mut anim = VisitingViewAnimator::new(0.5, 0.5, 2.0, 5.0);
        anim.set_identity("nonexistent", "");
        anim.set_animated(true);

        anim.animate(&mut view, &mut tree, 0.016);
        assert_eq!(
            anim.visiting_state(),
            VisitingState::GivingUp,
            "Should give up when panel doesn't exist"
        );

        // Run through the 1.5s give-up display
        for _ in 0..200 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                break;
            }
        }
        assert_eq!(anim.visiting_state(), VisitingState::GivenUp);
    }

    #[test]
    fn swiping_grip_spring() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = SwipingViewAnimator::new(2.0);
        anim.inner_mut().set_friction_enabled(true);
        anim.set_spring_constant(100.0);
        assert!(!anim.is_active());

        // Grip and move
        anim.set_gripped(true);
        anim.move_grip(0, 50.0); // 50px spring extension in X
        assert!(anim.is_active());

        // Animate — spring should produce kinetic velocity
        anim.animate(&mut view, &mut tree, 0.016);
        let (vx, _, _) = anim.inner().velocity();
        assert!(vx.abs() > 0.0, "Spring should produce X velocity");
    }

    #[test]
    fn swiping_release_coasts() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = SwipingViewAnimator::new(2.0);
        anim.inner_mut().set_friction_enabled(true);
        anim.set_spring_constant(100.0);
        anim.set_gripped(true);

        // Build up velocity via spring
        for _ in 0..10 {
            anim.move_grip(0, 5.0);
            anim.animate(&mut view, &mut tree, 1.0 / 60.0);
        }
        let (vx_before, _, _) = anim.inner().velocity();
        assert!(vx_before.abs() > 1.0, "Should have built up velocity");

        // Release — should coast with friction
        anim.set_gripped(false);
        for _ in 0..5000 {
            if !anim.animate(&mut view, &mut tree, 1.0 / 60.0) {
                break;
            }
        }
        assert!(!anim.is_active(), "Should decelerate to stop");
    }

    #[test]
    fn swiping_stop() {
        let mut anim = SwipingViewAnimator::new(2.0);
        anim.set_gripped(true);
        anim.move_grip(0, 50.0);
        assert!(anim.is_active());

        anim.stop();
        assert!(!anim.is_active());
        let (vx, vy, vz) = anim.inner().velocity();
        assert_eq!(vx, 0.0);
        assert_eq!(vy, 0.0);
        assert_eq!(vz, 0.0);
    }

    #[test]
    fn magnetic_snaps_to_target() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = MagneticViewAnimator::new(50.0);
        let state = view.current_visit().clone();
        // Set a snap target slightly offset from current position
        anim.set_snap_target(state.rel_x + 0.001, state.rel_y + 0.001);

        assert!(anim.is_active());

        for _ in 0..500 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                break;
            }
        }

        assert!(!anim.is_active(), "Should converge to snap target");
    }

    #[test]
    fn magnetic_stop() {
        let mut anim = MagneticViewAnimator::new(50.0);
        anim.set_snap_target(0.5, 0.5);
        assert!(anim.is_active());

        anim.stop();
        assert!(!anim.is_active());
        let (vx, vy) = anim.velocity();
        assert_eq!(vx, 0.0);
        assert_eq!(vy, 0.0);
    }

    #[test]
    #[ignore]
    fn magnetic_panel_tree_traversal_magnetism() {
        // BLOCKED: needs panel-tree-traversal magnetism matching C++ emMagneticViewAnimator.
        // C++ ref: emViewAnimator.cpp:emMagneticViewAnimator::CalculateDistance (line 809)
        // and emMagneticViewAnimator::CycleAnimation (line 716).
        //
        // The C++ implementation:
        // 1. CalculateDistance() traverses the panel tree depth-first from
        //    GetView().GetSupremeViewedPanel(), visiting every viewed+focusable
        //    panel. For each, it gets the essence rect, converts to view coords
        //    via PanelToViewX/Y/DeltaX/DeltaY, computes the scroll+zoom distance
        //    to center-and-maximize that panel in the viewport, and keeps the
        //    nearest one.
        // 2. CycleAnimation() uses a hill-rolling physics model (not spring-damper):
        //    - Config-driven radius (CoreConfig.MagnetismRadius) controls the
        //      engagement distance: maxDist = (vw+vh)*0.09*radiusFactor
        //    - Config-driven speed (CoreConfig.MagnetismSpeed) controls
        //      acceleration and damping
        //    - Sub-stepping simulation (0.01s steps) with slope-based acceleration
        //      and velocity-proportional damping
        //    - 3D: scroll X, scroll Y, AND zoom Z (not just 2D like current Rust)
        //    - Inherits from emKineticViewAnimator for velocity/friction/scroll-zoom
        //
        // The current Rust MagneticViewAnimator instead uses:
        // - Externally-set snap_target_x/y (no auto-discovery of nearest panel)
        // - 2D only (no zoom magnetism)
        // - Simple spring-damper (F = k*disp, not hill-rolling)
        // - No CoreConfig radius/speed integration
        //
        // Infrastructure already present in Rust:
        // - PanelTree::viewed_panels_dfs() — DFS traversal of viewed panels
        // - PanelTree::focusable(id) — focusability check
        // - PanelTree::get_essence_rect(id) — essence rect
        // - PanelTree::panel_to_view_x/y/delta_x/delta_y() — coord transforms
        // - View::supreme_panel() — supreme viewed panel
        // - View::get_zoom_factor_log_per_pixel() — zflpp
        // - CoreConfig::magnetism_radius/magnetism_speed — config values
        //
        // What needs to change:
        // 1. Rewrite MagneticViewAnimator to inherit/compose KineticViewAnimator
        //    (for 3D velocity, friction, scroll-zoom delegation)
        // 2. Implement CalculateDistance: iterate viewed+focusable panels from
        //    supreme_panel downward, compute 3D (dx, dy, dz) to each, keep min
        // 3. Replace spring-damper with hill-rolling physics (slope-based accel
        //    + velocity damping, sub-stepped at 0.01s)
        // 4. Wire CoreConfig magnetism_radius and magnetism_speed
        // 5. Implement Activate() friction inheritance from active KVA
        //
        // Expected behavior: when the view is near-idle, the magnetic animator
        // automatically discovers the nearest focusable panel and smoothly
        // scrolls+zooms to center it in the viewport.
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let _anim = MagneticViewAnimator::new(50.0);
        // Currently there is no auto-discovery — the snap target must be set
        // externally. The C++ version discovers it by traversing the panel tree.
        assert!(
            false,
            "MagneticViewAnimator should auto-discover nearest focusable panel"
        );
    }

    #[test]
    #[ignore]
    fn kinetic_velocity_inherited_on_activation() {
        // BLOCKED: needs active-animator registry on View/Window with velocity
        // inheritance protocol. C++ ref: emViewAnimator.cpp:Activate() (lines
        // 68-84) inherits LastTSC/LastClk from the currently active animator,
        // and emKineticViewAnimator::Activate() (lines 188-226) walks the
        // active chain to find a KineticViewAnimator and copies its velocity
        // and zoom-fix-point.
        //
        // In the Rust architecture, kinetic physics are inlined in
        // MouseZoomScrollVIF (input_filter.rs) rather than composed as separate
        // ViewAnimator instances. There is no master/slave chain or
        // UpperActivePtr — the VIF and active_animator slot run independently.
        //
        // To port this behavior:
        // 1. Add a KineticState { velocity: [f64; 3], zoom_fix: (f64, f64,
        //    bool) } struct that can be extracted/applied.
        // 2. Add View::active_kinetic_state() -> Option<KineticState> that the
        //    VIF populates when its grip/wheel/coast animations are running.
        // 3. When a new kinetic animation activates (VIF grip start, wheel
        //    start, or active_animator swap), inherit KineticState from the
        //    previous active source and deactivate it.
        // 4. Wire needs_animator_abort in the app loop (currently set but never
        //    checked) for mutual exclusion between VIF and active_animator.
        //
        // Expected behavior: swiping animator at velocity (100, 0, 0) is
        // replaced by a new kinetic animator which inherits that velocity.
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut old_anim = KineticViewAnimator::new(100.0, 50.0, 0.0, 1000.0);
        old_anim.set_friction_enabled(true);
        old_anim.animate(&mut view, &mut tree, 0.016);

        // A new kinetic animator should inherit velocity from old_anim.
        // Currently there is no activation protocol to transfer velocity.
        let new_anim = KineticViewAnimator::new(0.0, 0.0, 0.0, 1000.0);
        let (vx, vy, _) = new_anim.velocity();
        assert!(
            vx.abs() > 1.0 && vy.abs() > 1.0,
            "New animator should inherit velocity from previous active animator"
        );
    }
}
