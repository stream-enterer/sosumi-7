use super::tree::PanelTree;
use super::view::View;

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
            active: velocity_x.abs() > 0.01 || velocity_y.abs() > 0.01 || velocity_z.abs() > 0.01,
        }
    }

    pub fn set_velocity(&mut self, vx: f64, vy: f64, vz: f64) {
        self.velocity_x = vx;
        self.velocity_y = vy;
        self.velocity_z = vz;
        self.active = vx.abs() > 0.01 || vy.abs() > 0.01 || vz.abs() > 0.01;
    }

    pub fn velocity(&self) -> (f64, f64, f64) {
        (self.velocity_x, self.velocity_y, self.velocity_z)
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

    /// If centered, update fix point to viewport center.
    pub fn update_zoom_fix_point(&mut self, view: &View) {
        if self.zoom_fix_point_centered {
            let (vw, vh) = view.viewport_size();
            self.zoom_fix_x = vw * 0.5;
            self.zoom_fix_y = vh * 0.5;
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

        // Apply linear friction per-dimension independently
        if self.friction_enabled {
            let a = self.friction;
            self.velocity_x = apply_friction_1d(self.velocity_x, a, dt);
            self.velocity_y = apply_friction_1d(self.velocity_y, a, dt);
            self.velocity_z = apply_friction_1d(self.velocity_z, a, dt);
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

/// Visiting view animator — smoothly animates the camera to a target visit state.
/// Uses S-curve velocity profile with acceleration/deceleration ramps and
/// logarithmic interpolation for zoom dimension.
pub struct VisitingViewAnimator {
    target_x: f64,
    target_y: f64,
    target_a: f64,
    speed: f64,
    active: bool,
    /// Whether smooth animation is enabled (false = instant visit).
    animated: bool,
    /// Acceleration for speed ramping (units/s^2).
    acceleration: f64,
    /// Maximum speed at zoom cusp (zoom-out-then-in transition).
    max_cusp_speed: f64,
    /// Maximum absolute animation speed.
    max_absolute_speed: f64,
    /// Current state in the seek/navigation state machine.
    state: VisitingState,
    /// Target panel identity string (slash-separated path).
    identity: String,
    /// Human-readable subject description for seek overlay.
    subject: String,
    /// Current animation speed (ramps up then down via S-curve).
    current_speed: f64,
    /// Total elapsed animation time in seconds.
    elapsed: f64,
    /// Frames with no significant progress (blocked-movement detection).
    stall_frames: u32,
    /// Previous position for blocked-movement detection.
    prev_x: f64,
    prev_y: f64,
    prev_log_a: f64,
}

impl VisitingViewAnimator {
    pub fn new(target_x: f64, target_y: f64, target_a: f64, speed: f64) -> Self {
        Self {
            target_x,
            target_y,
            target_a,
            speed,
            active: true,
            animated: false,
            acceleration: 5.0,
            max_cusp_speed: 2.0,
            max_absolute_speed: 5.0,
            state: VisitingState::Curve,
            identity: String::new(),
            subject: String::new(),
            current_speed: 0.0,
            elapsed: 0.0,
            stall_frames: 0,
            prev_x: f64::NAN,
            prev_y: f64::NAN,
            prev_log_a: f64::NAN,
        }
    }

    /// Configure animation parameters from a speed config value.
    ///
    /// Mirrors C++ `emVisitingViewAnimator::SetAnimParamsByCoreConfig`.
    /// `speed_factor` is the user's configured visit speed (typically 0..max).
    /// `max_speed_factor` is the maximum value of that config range.
    ///
    /// When `speed_factor` is near `max_speed_factor`, animation is disabled
    /// (instant visit). Otherwise, acceleration and max speeds are scaled by
    /// `35.0 * speed_factor`, and cusp speed is half of max absolute speed.
    pub fn set_anim_params_by_speed_config(&mut self, speed_factor: f64, max_speed_factor: f64) {
        self.animated = speed_factor < max_speed_factor * 0.99999;
        self.acceleration = 35.0 * speed_factor;
        self.max_absolute_speed = 35.0 * speed_factor;
        self.max_cusp_speed = self.max_absolute_speed * 0.5;
    }

    /// Returns the current visiting state.
    pub(crate) fn visiting_state(&self) -> VisitingState {
        self.state
    }

    /// Set state (used by seek logic / tests).
    pub(crate) fn set_visiting_state(&mut self, state: VisitingState) {
        self.state = state;
    }

    /// Set the identity and subject for seek overlay display.
    pub fn set_identity(&mut self, identity: &str, subject: &str) {
        self.identity = identity.to_string();
        self.subject = subject.to_string();
    }

    /// Returns the identity string being visited.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Handle input during visiting animation.
    ///
    /// Mirrors C++ `emVisitingViewAnimator::Input`.
    /// During seek or giving-up states, any key/mouse event aborts the
    /// seek and deactivates the animator. Returns true if the event was
    /// consumed (eaten).
    pub fn handle_input(&mut self, event: &crate::input::InputEvent) -> bool {
        if !self.active {
            return false;
        }
        if self.state != VisitingState::Seek && self.state != VisitingState::GivingUp {
            return false;
        }
        // Any non-empty event aborts the seek
        if event.key != crate::input::InputKey::MouseLeft
            || event.variant != crate::input::InputVariant::Move
        {
            // An actual key/button event (not just mouse move) — abort
            self.active = false;
            self.state = VisitingState::GivenUp;
            return true;
        }
        false
    }

    /// Paint the seek progress overlay.
    ///
    /// Mirrors C++ `emVisitingViewAnimator::Paint`.
    /// Shows a semi-transparent overlay with the target identity and a
    /// progress bar when in Seek or GivingUp state.
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

        // Shadow
        let shadow_off = w * 0.03;
        painter.paint_round_rect(
            x + shadow_off,
            y + shadow_off,
            w,
            h,
            h * 0.2,
            crate::foundation::Color::rgba(0, 0, 0, 160),
        );

        // Background box
        painter.paint_round_rect(
            x,
            y,
            w,
            h,
            h * 0.2,
            crate::foundation::Color::rgba(34, 102, 153, 208),
        );

        let _ch = h * 0.22;

        if self.state == VisitingState::GivingUp {
            // TODO(font): paint text here ("Not found")
            return;
        }

        // "Seeking..." text
        let mut _seeking_text = String::from("Seeking...");
        if !self.subject.is_empty() {
            _seeking_text.push_str(" for ");
            _seeking_text.push_str(&self.subject);
        }
        // TODO(font): paint text here (seeking status)

        // Progress bar background
        let bar_x = x + w * 0.05;
        let bar_y = y + h * 0.45;
        let bar_w = w * 0.9;
        let bar_h = h * 0.15;

        // Compute progress from identity match
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

        // Found portion (green)
        if progress > 0.0 {
            painter.paint_rect(
                bar_x,
                bar_y,
                bar_w * progress,
                bar_h,
                crate::foundation::Color::rgba(136, 255, 136, 80),
            );
        }
        // Remaining portion (gray)
        if progress < 1.0 {
            painter.paint_rect(
                bar_x + bar_w * progress,
                bar_y,
                bar_w * (1.0 - progress),
                bar_h,
                crate::foundation::Color::rgba(136, 136, 136, 80),
            );
        }

        // TODO(font): paint text here (identity label)

        // TODO(font): paint text here ("Press any key to abort")
    }
}

impl VisitingViewAnimator {
    /// Compute the S-curve speed factor for the current animation progress.
    ///
    /// Uses a smoothstep-like profile: accelerate during the first half of
    /// the remaining distance, then decelerate during the second half.
    /// `remaining` is the normalised distance to target (0..inf),
    /// returns a speed multiplier in 0..1.
    fn s_curve_speed(&self, remaining: f64) -> f64 {
        // Ramp up from 0 to 1 over the range 0..1, stay at 1 for 1..large,
        // then decelerate as remaining shrinks below a threshold.
        // We blend acceleration (near start) with deceleration (near end).
        let accel_factor = (self.elapsed * self.acceleration).min(1.0);
        // Deceleration: as remaining distance shrinks, slow down proportionally
        let decel_factor = remaining.min(1.0);
        // S-curve: the effective factor is the minimum of both ramps,
        // smoothed with a cubic Hermite (smoothstep).
        let raw = accel_factor.min(decel_factor);
        // Smoothstep for a nicer S-curve profile
        raw * raw * (3.0 - 2.0 * raw)
    }
}

impl ViewAnimator for VisitingViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        match self.visiting_state() {
            VisitingState::NoGoal | VisitingState::GivenUp | VisitingState::GoalReached => {
                return false;
            }
            VisitingState::GivingUp => {
                // Still showing "Not found" — just stay active
                return true;
            }
            VisitingState::Seek => {
                // Seek state — track elapsed for give-up timeout
                self.elapsed += dt;
                if self.elapsed > 3.0 {
                    self.state = VisitingState::GivingUp;
                }
                return true;
            }
            VisitingState::Curve | VisitingState::Direct => {
                // Continue with animation below
            }
        }

        self.elapsed += dt;

        // Give-up timeout: after 3 seconds, snap to target
        if self.elapsed > 3.0 {
            if let Some(state) = view.visit_stack().last().cloned() {
                let log_target = self.target_a.max(0.001).ln();
                let dx = (self.target_x - state.rel_x) * view.viewport_size().0.max(1.0);
                let dy = (self.target_y - state.rel_y) * view.viewport_size().1.max(1.0);
                let dz = if state.rel_a > 0.0 {
                    log_target - state.rel_a.ln()
                } else {
                    0.0
                };
                let (vw, vh) = view.viewport_size();
                view.raw_scroll_and_zoom(tree, vw * 0.5, vh * 0.5, dx, dy, dz);
            }
            self.active = false;
            self.set_visiting_state(VisitingState::GoalReached);
            return false;
        }

        if let Some(state) = view.visit_stack().last().cloned() {
            let log_a = state.rel_a.ln();
            let log_target = self.target_a.max(0.001).ln();

            // Compute remaining distance (normalised)
            let dx_rem = (self.target_x - state.rel_x).abs();
            let dy_rem = (self.target_y - state.rel_y).abs();
            let dz_rem = (log_target - log_a).abs();
            let remaining = dx_rem + dy_rem + dz_rem;

            // S-curve speed: ramp up then decelerate
            let s_factor = self.s_curve_speed(remaining);
            // Effective speed blends base speed with S-curve modulation
            let effective_speed = self.speed * (0.1 + 0.9 * s_factor);
            self.current_speed = effective_speed;

            // Exponential decay interpolation (framerate-independent)
            // t approaches 1 as effective_speed * dt grows
            let t = (1.0 - (-effective_speed * dt).exp()).min(1.0);

            let new_x = lerp(state.rel_x, self.target_x, t);
            let new_y = lerp(state.rel_y, self.target_y, t);
            let new_log_a = lerp(log_a, log_target, t);
            let new_a = new_log_a.exp();

            let dx = (new_x - state.rel_x) * view.viewport_size().0.max(1.0);
            let dy = (new_y - state.rel_y) * view.viewport_size().1.max(1.0);
            let dz = if state.rel_a > 0.0 {
                (new_a / state.rel_a).ln()
            } else {
                0.0
            };

            let (vw, vh) = view.viewport_size();
            view.raw_scroll_and_zoom(tree, vw * 0.5, vh * 0.5, dx, dy, dz);

            // Transition from Curve to Direct when close enough
            if self.visiting_state() == VisitingState::Curve {
                let dist = (new_x - self.target_x).abs()
                    + (new_y - self.target_y).abs()
                    + (new_log_a - log_target).abs();
                if dist < 0.1 {
                    self.set_visiting_state(VisitingState::Direct);
                }
            }

            // Blocked-movement detection: if position barely changed for 3+ frames, give up
            if self.prev_x.is_finite() {
                let progress = (new_x - self.prev_x).abs()
                    + (new_y - self.prev_y).abs()
                    + (new_log_a - self.prev_log_a).abs();
                if progress < 1e-10 {
                    self.stall_frames += 1;
                } else {
                    self.stall_frames = 0;
                }
                if self.stall_frames >= 3 {
                    self.active = false;
                    self.set_visiting_state(VisitingState::GoalReached);
                    return false;
                }
            }
            self.prev_x = new_x;
            self.prev_y = new_y;
            self.prev_log_a = new_log_a;

            // Tight convergence thresholds (C++ parity: pos < 1e-6, area < 1e-12)
            if (new_x - self.target_x).abs() < 1e-6
                && (new_y - self.target_y).abs() < 1e-6
                && (new_log_a - log_target).abs() < 1e-12
            {
                self.active = false;
                self.set_visiting_state(VisitingState::GoalReached);
            }
        }

        self.active
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.active = false;
        self.current_speed = 0.0;
        self.set_visiting_state(VisitingState::NoGoal);
    }
}

/// Per-dimension linear friction: reduces signed velocity toward zero by `a * dt`.
fn apply_friction_1d(v: f64, a: f64, dt: f64) -> f64 {
    if v - a * dt > 0.0 {
        v - a * dt
    } else if v + a * dt < 0.0 {
        v + a * dt
    } else {
        0.0
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// State for the swiping (touch/mouse drag) animator.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SwipingState {
    /// No active swipe.
    Inactive,
    /// Finger/mouse is down, following the drag.
    Tracking,
    /// Released with velocity, coasting to a stop.
    Coasting,
}

/// Swiping view animator — handles touch/mouse drag with kinetic coasting.
///
/// During tracking, accumulates velocity from position deltas.
/// On release, coasts with friction deceleration until velocity drops below threshold.
pub struct SwipingViewAnimator {
    state: SwipingState,
    velocity_x: f64,
    velocity_y: f64,
    /// Friction factor applied per frame (typical: 0.95).
    friction_factor: f64,
    /// Velocity threshold below which coasting stops.
    stop_threshold: f64,
    /// Previous tracking position for delta computation.
    last_x: f64,
    last_y: f64,
    /// Smoothed velocity accumulator (exponential moving average).
    smoothed_vx: f64,
    smoothed_vy: f64,
}

impl SwipingViewAnimator {
    pub fn new() -> Self {
        Self {
            state: SwipingState::Inactive,
            velocity_x: 0.0,
            velocity_y: 0.0,
            friction_factor: 0.95,
            stop_threshold: 0.5,
            last_x: 0.0,
            last_y: 0.0,
            smoothed_vx: 0.0,
            smoothed_vy: 0.0,
        }
    }

    /// Current swiping state.
    pub fn state(&self) -> SwipingState {
        self.state
    }

    /// Set the friction factor (0..1, higher = less friction).
    pub fn set_friction_factor(&mut self, factor: f64) {
        self.friction_factor = factor.clamp(0.0, 1.0);
    }

    /// Set the velocity threshold below which coasting stops.
    pub fn set_stop_threshold(&mut self, threshold: f64) {
        self.stop_threshold = threshold;
    }

    /// Begin tracking a drag at the given position.
    pub fn begin_tracking(&mut self, x: f64, y: f64) {
        self.state = SwipingState::Tracking;
        self.last_x = x;
        self.last_y = y;
        self.smoothed_vx = 0.0;
        self.smoothed_vy = 0.0;
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
    }

    /// Update tracking position. Call each frame while dragging.
    /// `dt` is the frame delta in seconds (must be > 0).
    pub fn update_tracking(&mut self, x: f64, y: f64, dt: f64) {
        if self.state != SwipingState::Tracking {
            return;
        }
        let dt_safe = dt.max(1e-6);
        let instant_vx = (x - self.last_x) / dt_safe;
        let instant_vy = (y - self.last_y) / dt_safe;
        // Exponential moving average for smooth velocity
        let alpha = 0.3;
        self.smoothed_vx = lerp(self.smoothed_vx, instant_vx, alpha);
        self.smoothed_vy = lerp(self.smoothed_vy, instant_vy, alpha);
        self.last_x = x;
        self.last_y = y;
    }

    /// End tracking and begin coasting with the accumulated velocity.
    pub fn end_tracking(&mut self) {
        if self.state != SwipingState::Tracking {
            return;
        }
        self.velocity_x = self.smoothed_vx;
        self.velocity_y = self.smoothed_vy;
        let speed = (self.velocity_x * self.velocity_x + self.velocity_y * self.velocity_y).sqrt();
        if speed < self.stop_threshold {
            self.state = SwipingState::Inactive;
        } else {
            self.state = SwipingState::Coasting;
        }
    }

    /// Current velocity.
    pub fn velocity(&self) -> (f64, f64) {
        (self.velocity_x, self.velocity_y)
    }
}

impl Default for SwipingViewAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewAnimator for SwipingViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        match self.state {
            SwipingState::Inactive => false,
            SwipingState::Tracking => {
                // During tracking, the caller drives position via update_tracking.
                // Scroll is applied externally. We just stay active.
                true
            }
            SwipingState::Coasting => {
                // Apply friction each frame
                self.velocity_x *= self.friction_factor;
                self.velocity_y *= self.friction_factor;

                let dx = self.velocity_x * dt;
                let dy = self.velocity_y * dt;

                if dx.abs() > 0.001 || dy.abs() > 0.001 {
                    let (vw, vh) = view.viewport_size();
                    let done = view.raw_scroll_and_zoom(tree, vw * 0.5, vh * 0.5, dx, dy, 0.0);
                    // Zero blocked dimensions
                    if done[0].abs() < 0.99 * dx.abs() {
                        self.velocity_x = 0.0;
                    }
                    if done[1].abs() < 0.99 * dy.abs() {
                        self.velocity_y = 0.0;
                    }
                }

                let speed =
                    (self.velocity_x * self.velocity_x + self.velocity_y * self.velocity_y).sqrt();
                if speed < self.stop_threshold {
                    self.velocity_x = 0.0;
                    self.velocity_y = 0.0;
                    self.state = SwipingState::Inactive;
                    return false;
                }
                true
            }
        }
    }

    fn is_active(&self) -> bool {
        self.state != SwipingState::Inactive
    }

    fn stop(&mut self) {
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
        self.state = SwipingState::Inactive;
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

        let mut anim = KineticViewAnimator::new(0.0, 0.0, 1.0, 1000.0);
        // friction_enabled defaults to false — just test that zoom scroll works
        anim.animate(&mut view, &mut tree, 0.1);

        // Zoom velocity should have changed rel_a
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

        for _ in 0..300 {
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
    fn visiting_give_up_timeout() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        // Target a position that cannot converge quickly with very low speed
        let mut anim = VisitingViewAnimator::new(0.5, 0.5, 5.0, 0.001);

        // Simulate 4 seconds of frames — should hit the 3s give-up timeout
        let mut converged = false;
        for _ in 0..250 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                converged = true;
                break;
            }
        }
        assert!(converged, "Should give up after 3s timeout");
        assert_eq!(anim.visiting_state(), VisitingState::GoalReached);
    }

    #[test]
    fn visiting_blocked_movement_gives_up() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        // Start at the target — should detect stall and stop
        let state = view.current_visit().clone();
        let mut anim = VisitingViewAnimator::new(state.rel_x, state.rel_y, state.rel_a, 10.0);

        let mut stopped = false;
        for _ in 0..20 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                stopped = true;
                break;
            }
        }
        assert!(stopped, "Should stop when blocked (already at target)");
    }

    #[test]
    fn visiting_s_curve_speed_ramps() {
        let anim = VisitingViewAnimator::new(0.5, 0.5, 2.0, 5.0);
        // At elapsed=0 (start), s-curve factor should be very small
        let s0 = anim.s_curve_speed(1.0);
        assert!(s0 < 0.01, "S-curve should start near zero");
    }

    #[test]
    fn visiting_seek_timeout() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = VisitingViewAnimator::new(0.5, 0.5, 2.0, 5.0);
        anim.set_visiting_state(VisitingState::Seek);

        // Run for 4 seconds — should transition to GivingUp
        for _ in 0..250 {
            anim.animate(&mut view, &mut tree, 0.016);
        }
        assert_eq!(anim.visiting_state(), VisitingState::GivingUp);
    }

    #[test]
    fn swiping_tracking_and_coasting() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = SwipingViewAnimator::new();
        assert_eq!(anim.state(), SwipingState::Inactive);
        assert!(!anim.is_active());

        // Begin tracking
        anim.begin_tracking(100.0, 100.0);
        assert_eq!(anim.state(), SwipingState::Tracking);
        assert!(anim.is_active());

        // Simulate drag with velocity
        anim.update_tracking(120.0, 100.0, 0.016);
        anim.update_tracking(140.0, 100.0, 0.016);
        anim.update_tracking(160.0, 100.0, 0.016);

        // Release — should enter coasting
        anim.end_tracking();
        assert_eq!(anim.state(), SwipingState::Coasting);
        let (vx, _vy) = anim.velocity();
        assert!(
            vx > 0.0,
            "Should have positive X velocity after rightward drag"
        );

        // Run coasting frames until stopped
        for _ in 0..500 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                break;
            }
        }
        assert!(!anim.is_active(), "Should decelerate to stop");
    }

    #[test]
    fn swiping_slow_release_stays_inactive() {
        let mut anim = SwipingViewAnimator::new();
        anim.begin_tracking(100.0, 100.0);
        // No significant movement — velocity stays near zero
        anim.update_tracking(100.001, 100.0, 0.016);
        anim.end_tracking();
        // Should go directly to inactive since velocity is below threshold
        assert_eq!(anim.state(), SwipingState::Inactive);
    }

    #[test]
    fn swiping_stop() {
        let mut anim = SwipingViewAnimator::new();
        anim.begin_tracking(100.0, 100.0);
        anim.update_tracking(200.0, 100.0, 0.016);
        anim.end_tracking();
        assert!(anim.is_active());

        anim.stop();
        assert!(!anim.is_active());
        let (vx, vy) = anim.velocity();
        assert_eq!(vx, 0.0);
        assert_eq!(vy, 0.0);
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
}
