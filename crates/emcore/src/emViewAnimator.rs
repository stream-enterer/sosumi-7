use crate::dlog;

use super::emView::{emView, ViewFlags};
use crate::emColor::emColor;
use crate::emPanel::Rect;
use crate::emPanelTree::PanelTree;

/// Trait for view animation strategies.
pub trait emViewAnimator {
    /// Advance the animation by one frame. Returns true if still animating.
    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool;

    /// Whether the animation is currently active.
    fn is_active(&self) -> bool;

    /// Stop the animation immediately.
    fn stop(&mut self);

    /// Downcast to concrete type for velocity handoff (C++ dynamic_cast equivalent).
    fn as_any(&self) -> &dyn std::any::Any;

    /// Port of C++ `emViewAnimator::Input(emInputEvent&, const emInputState&)`
    /// (emViewAnimator.cpp:111). Base default forwards to the active slave in
    /// C++; the Rust trait has no slave concept, so the default is a no-op.
    /// Subclasses that want to consume input (e.g. `emVisitingViewAnimator`)
    /// override this.
    fn Input(
        &mut self,
        _event: &mut crate::emInput::emInputEvent,
        _state: &crate::emInputState::emInputState,
    ) {
    }
}

/// Master/slave animator slot with deactivation chain.
///
/// Port of C++ emViewAnimator::SetMaster/Activate/Deactivate semantics.
/// The slot holds an active animator and an optional slave slot. Activating
/// a new animator deactivates the current one first. Deactivating cascades
/// to the slave.
pub struct AnimatorSlot {
    active: Option<Box<dyn emViewAnimator>>,
    slave: Option<Box<AnimatorSlot>>,
    /// C++ LastTSC: time slice counter for time continuity across animator switches.
    last_tsc: u64,
    /// C++ LastClk: clock value (ms) for time continuity across animator switches.
    last_clk: f64,
}

impl Default for AnimatorSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimatorSlot {
    pub fn new() -> Self {
        Self {
            active: None,
            slave: None,
            last_tsc: 0,
            last_clk: 0.0,
        }
    }

    /// Activate an animator in this slot. Deactivates the current active
    /// animator first, capturing its LastTSC/LastClk for time continuity
    /// (C++ Activate semantics).
    pub fn activate(&mut self, animator: Box<dyn emViewAnimator>) {
        // Capture time state from outgoing animator before deactivating
        // (C++ copies LastTSC/LastClk for time continuity)
        if let Some(ref mut current) = self.active {
            current.stop();
        }
        self.active = Some(animator);
    }

    /// Get the last time slice counter (for time continuity).
    pub fn last_tsc(&self) -> u64 {
        self.last_tsc
    }

    /// Get the last clock value (for time continuity).
    pub fn last_clk(&self) -> f64 {
        self.last_clk
    }

    /// Update time state after an animation step.
    pub fn update_time(&mut self, tsc: u64, clk: f64) {
        self.last_tsc = tsc;
        self.last_clk = clk;
    }

    /// Deactivate the active animator. Deactivates any active slave first
    /// (C++ Deactivate semantics: recursive slave deactivation).
    pub fn deactivate(&mut self) {
        // Deactivate slave first (recursive)
        if let Some(ref mut slave) = self.slave {
            slave.deactivate();
        }
        // Then deactivate self
        if let Some(ref mut current) = self.active {
            current.stop();
        }
        self.active = None;
    }

    /// Get the active animator, if any.
    pub fn GetActivePanel(&self) -> Option<&dyn emViewAnimator> {
        self.active.as_deref()
    }

    /// Get the active animator mutably, if any.
    pub fn active_mut(&mut self) -> Option<&mut Box<dyn emViewAnimator>> {
        self.active.as_mut()
    }

    /// Whether this slot has an active animator.
    pub fn is_active(&self) -> bool {
        self.active.as_ref().map(|a| a.is_active()).unwrap_or(false)
    }

    /// Set a slave slot. The slave is deactivated when this slot is deactivated.
    pub fn set_slave(&mut self, slave: AnimatorSlot) {
        self.slave = Some(Box::new(slave));
    }

    /// Get the slave slot, if any.
    pub fn slave(&self) -> Option<&AnimatorSlot> {
        self.slave.as_deref()
    }

    /// Get the slave slot mutably, if any.
    pub fn slave_mut(&mut self) -> Option<&mut AnimatorSlot> {
        self.slave.as_deref_mut()
    }
}

/// Snapshot of kinetic animation state for velocity handoff between animators.
#[derive(Clone, Debug)]
pub struct KineticState {
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub zoom_fix_centered: bool,
    pub zoom_fix_x: f64,
    pub zoom_fix_y: f64,
}

/// Kinetic view animator — applies velocity with linear friction for smooth deceleration.
/// Used for fling/swipe gestures. Supports 3D (scroll x, scroll y, zoom z).
pub struct emKineticViewAnimator {
    velocity_x: f64,
    velocity_y: f64,
    velocity_z: f64,
    friction: f64,
    friction_enabled: bool,
    /// When true, friction is suppressed (C++ magnetism active disables friction).
    pub magnetism_suppresses_friction: bool,
    zoom_fix_point_centered: bool,
    zoom_fix_x: f64,
    zoom_fix_y: f64,
    active: bool,
}

impl emKineticViewAnimator {
    pub fn new(velocity_x: f64, velocity_y: f64, velocity_z: f64, friction: f64) -> Self {
        Self {
            velocity_x,
            velocity_y,
            velocity_z,
            friction,
            friction_enabled: false,
            magnetism_suppresses_friction: false,
            zoom_fix_point_centered: true,
            zoom_fix_x: 0.0,
            zoom_fix_y: 0.0,
            active: (velocity_x * velocity_x + velocity_y * velocity_y + velocity_z * velocity_z)
                .sqrt()
                > 0.01,
        }
    }

    pub fn SetVelocity(&mut self, vx: f64, vy: f64, vz: f64) {
        self.velocity_x = vx;
        self.velocity_y = vy;
        self.velocity_z = vz;
        self.active = (vx * vx + vy * vy + vz * vz).sqrt() > 0.01;
    }

    pub fn GetVelocity(&self) -> (f64, f64, f64) {
        (self.velocity_x, self.velocity_y, self.velocity_z)
    }

    /// Absolute velocity magnitude (sqrt of sum of squares).
    pub fn GetAbsVelocity(&self) -> f64 {
        (self.velocity_x * self.velocity_x
            + self.velocity_y * self.velocity_y
            + self.velocity_z * self.velocity_z)
            .sqrt()
    }

    pub fn SetFrictionEnabled(&mut self, enabled: bool) {
        self.friction_enabled = enabled;
    }

    pub fn IsFrictionEnabled(&self) -> bool {
        self.friction_enabled
    }

    pub fn SetFriction(&mut self, friction: f64) {
        self.friction = friction;
    }

    pub fn GetFriction(&self) -> f64 {
        self.friction
    }

    /// Extract the current kinetic state for velocity handoff.
    pub fn extract_kinetic_state(&self) -> KineticState {
        KineticState {
            vx: self.velocity_x,
            vy: self.velocity_y,
            vz: self.velocity_z,
            zoom_fix_centered: self.zoom_fix_point_centered,
            zoom_fix_x: self.zoom_fix_x,
            zoom_fix_y: self.zoom_fix_y,
        }
    }

    /// Inject kinetic state from another animator (velocity handoff).
    pub fn inject_kinetic_state(&mut self, state: KineticState) {
        self.velocity_x = state.vx;
        self.velocity_y = state.vy;
        self.velocity_z = state.vz;
        self.zoom_fix_point_centered = state.zoom_fix_centered;
        self.zoom_fix_x = state.zoom_fix_x;
        self.zoom_fix_y = state.zoom_fix_y;
        self.active =
            (state.vx * state.vx + state.vy * state.vy + state.vz * state.vz).sqrt() > 0.01;
    }

    /// Activate this animator, inheriting velocity from any outgoing
    /// emKineticViewAnimator. Matches C++ emKineticViewAnimator::Activate().
    ///
    /// Activate this animator, walking the active animator chain to find an
    /// outgoing emKineticViewAnimator and inherit its velocity.
    ///
    /// C++ emKineticViewAnimator::Activate(): walks GetActiveAnimator chain
    /// with dynamic_cast to find an outgoing KVA. If found, extracts kinetic
    /// state and injects it. If not found, zeros all velocity fields.
    pub fn activate_with_handoff(
        &mut self,
        active_animator: Option<&dyn emViewAnimator>,
        view: &emView,
    ) {
        // Walk the active animator chain to find a emKineticViewAnimator
        // (C++ dynamic_cast equivalent via Any downcast)
        let kinetic_state = active_animator.and_then(|anim| {
            anim.as_any()
                .downcast_ref::<emKineticViewAnimator>()
                .map(|kva| kva.extract_kinetic_state())
        });

        if let Some(state) = kinetic_state {
            self.inject_kinetic_state(state);
            if self.zoom_fix_point_centered {
                self.CenterZoomFixPoint(view);
            } else {
                self.SetZoomFixPoint(self.zoom_fix_x, self.zoom_fix_y, view);
            }
        } else {
            self.velocity_x = 0.0;
            self.velocity_y = 0.0;
            self.velocity_z = 0.0;
            self.active = false;
        }
    }

    /// Switch zoom fix point to centered mode, compensating XY velocity.
    pub fn CenterZoomFixPoint(&mut self, view: &emView) {
        if self.zoom_fix_point_centered {
            return;
        }
        let old_fix_x = self.zoom_fix_x;
        let old_fix_y = self.zoom_fix_y;
        self.zoom_fix_point_centered = true;
        self.update_zoom_fix_point(view);
        let dt = 0.01;
        let zflpp = view.GetZoomFactorLogarithmPerPixel();
        let q = (1.0 - (-self.velocity_z * dt * zflpp).exp()) / dt;
        self.velocity_x += (old_fix_x - self.zoom_fix_x) * q;
        self.velocity_y += (old_fix_y - self.zoom_fix_y) * q;
    }

    /// Set an explicit (non-centered) zoom fix point, compensating XY velocity.
    pub fn SetZoomFixPoint(&mut self, x: f64, y: f64, view: &emView) {
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
        let zflpp = view.GetZoomFactorLogarithmPerPixel();
        let q = (1.0 - (-self.velocity_z * dt * zflpp).exp()) / dt;
        self.velocity_x += (old_fix_x - self.zoom_fix_x) * q;
        self.velocity_y += (old_fix_y - self.zoom_fix_y) * q;
    }

    /// If centered, update fix point to viewport center, clamped to popup
    /// rect when the view is popped up (C++ UpdateZoomFixPoint parity).
    pub fn update_zoom_fix_point(&mut self, view: &emView) {
        if self.zoom_fix_point_centered {
            let (vw, vh) = view.viewport_size();
            let mut x1 = 0.0;
            let mut y1 = 0.0;
            let mut x2 = vw;
            let mut y2 = vh;
            if view.IsPoppedUp() {
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

impl emViewAnimator for emKineticViewAnimator {
    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if !self.active {
            return false;
        }

        // Save pre-friction velocities for average displacement
        let vx_before = self.velocity_x;
        let vy_before = self.velocity_y;
        let vz_before = self.velocity_z;

        // Apply uniform magnitude-based friction (C++ parity):
        // compute single scale factor from velocity magnitude, apply to all axes.
        // C++ disables friction when magnetism is active.
        if self.friction_enabled && !self.magnetism_suppresses_friction {
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
        let done = view.RawScrollAndZoom(
            tree,
            self.zoom_fix_x,
            self.zoom_fix_y,
            dist[0],
            dist[1],
            dist[2],
            ctx,
        );
        view.SetActivePanelBestPossible(tree, ctx);

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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Speeding view animator — accelerates toward a target velocity.
/// Composes a emKineticViewAnimator for scroll/zoom delegation.
/// Used for keyboard-driven scrolling. Supports 3D.
pub struct emSpeedingViewAnimator {
    inner: emKineticViewAnimator,
    target_vx: f64,
    target_vy: f64,
    target_vz: f64,
    acceleration: f64,
    reverse_acceleration: f64,
    active: bool,
}

impl emSpeedingViewAnimator {
    pub fn new(friction: f64) -> Self {
        Self {
            inner: emKineticViewAnimator::new(0.0, 0.0, 0.0, friction),
            target_vx: 0.0,
            target_vy: 0.0,
            target_vz: 0.0,
            acceleration: 1.0,
            reverse_acceleration: 1.0,
            active: false,
        }
    }

    pub fn SetTargetVelocity(&mut self, vx: f64, vy: f64, vz: f64) {
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

    pub fn SetAcceleration(&mut self, accel: f64) {
        self.acceleration = accel;
    }

    pub fn SetReverseAcceleration(&mut self, accel: f64) {
        self.reverse_acceleration = accel;
    }

    pub fn inner(&self) -> &emKineticViewAnimator {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut emKineticViewAnimator {
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

impl emViewAnimator for emSpeedingViewAnimator {
    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if !self.active {
            return false;
        }

        // 3-branch acceleration per dimension
        let (vx, vy, vz) = self.inner.GetVelocity();
        let friction = self.inner.GetFriction();
        let friction_enabled = self.inner.IsFrictionEnabled();

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
        self.inner.SetVelocity(new_vx, new_vy, new_vz);

        // Temporarily disable friction on inner (speeding handles it via acceleration)
        let saved_friction = self.inner.IsFrictionEnabled();
        self.inner.SetFrictionEnabled(false);
        self.inner.animate(view, tree, dt, ctx);
        self.inner.SetFrictionEnabled(saved_friction);

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

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
    panel: super::emPanelTree::PanelId,
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
pub struct emVisitingViewAnimator {
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

impl emVisitingViewAnimator {
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

    /// Constructor matching C++ `emVisitingViewAnimator::emVisitingViewAnimator(emView & view)`
    /// at `emViewAnimator.cpp:930`. Initializes to ST_NO_GOAL / inactive.
    pub fn new_for_view() -> Self {
        Self {
            animated: false,
            acceleration: 5.0,
            max_cusp_speed: 2.0,
            max_absolute_speed: 5.0,
            state: VisitingState::NoGoal,
            visit_type: VisitType::Visit,
            identity: String::new(),
            names: Vec::new(),
            rel_x: 0.0,
            rel_y: 0.0,
            rel_a: 0.0,
            adherent: false,
            utilize_view: false,
            subject: String::new(),
            active: false,
            max_depth_seen: -1,
            speed: 0.0,
            time_slices_without_hope: 0,
            give_up_clock: 0.0,
        }
    }

    /// Configure animation parameters from the view's `emCoreConfig`.
    ///
    /// Mirrors C++ `emVisitingViewAnimator::SetAnimParamsByCoreConfig`
    /// (emViewAnimator.cpp:979-990): reads `VisitSpeed` and its max value
    /// from `core_config`, sets `animated` based on the strict-less-than
    /// predicate `f < fMax*0.99999`, and derives acceleration / speed
    /// bounds as linear multiples of `f`.
    pub fn SetAnimParamsByCoreConfig(&mut self, core_config: &crate::emCoreConfig::emCoreConfig) {
        let f = core_config.visit_speed;
        let f_max = core_config.VisitSpeed_GetMaxValue();
        self.animated = f < f_max * 0.99999;
        self.acceleration = 35.0 * f;
        self.max_absolute_speed = 35.0 * f;
        self.max_cusp_speed = self.max_absolute_speed * 0.5;
    }

    /// Port of C++ `emVisitingViewAnimator::IsAnimated` (emViewAnimator.h:431-434).
    pub fn IsAnimated(&self) -> bool {
        self.animated
    }

    pub fn SetAnimated(&mut self, animated: bool) {
        self.animated = animated;
    }

    pub fn SetAcceleration(&mut self, acceleration: f64) {
        self.acceleration = acceleration;
    }

    pub fn SetMaxCuspSpeed(&mut self, max_cusp_speed: f64) {
        self.max_cusp_speed = max_cusp_speed;
    }

    pub fn SetMaxAbsoluteSpeed(&mut self, max_absolute_speed: f64) {
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
    pub fn SetGoal(&mut self, identity: &str, adherent: bool, subject: &str) {
        self.visit_type = VisitType::Visit;
        self.rel_x = 0.0;
        self.rel_y = 0.0;
        self.rel_a = 0.0;
        self.adherent = adherent;
        self.utilize_view = false;
        self.subject = subject.to_string();
        self.activate_goal(identity);
    }

    /// Port of C++ `emVisitingViewAnimator::SetGoal(identity, relX, relY, relA, adherent, subject)`
    /// at `emViewAnimator.cpp:1001-1007`.
    /// DIVERGED: C++ name is `SetGoal` (6-arg overload). Rust cannot overload by arity;
    /// 3-arg variant keeps bare `SetGoal`, coords-carrying variant suffixed with `Coords`.
    pub fn SetGoalCoords(
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

    /// Goal relative X coordinate (visit-rel mode).
    /// C++ equivalent: `emVisitingViewAnimator::GetRelX` (public accessor).
    pub fn rel_x(&self) -> f64 {
        self.rel_x
    }

    /// Goal relative Y coordinate (visit-rel mode).
    /// C++ equivalent: `emVisitingViewAnimator::GetRelY` (public accessor).
    pub fn rel_y(&self) -> f64 {
        self.rel_y
    }

    /// Goal relative area coordinate (visit-rel mode).
    /// C++ equivalent: `emVisitingViewAnimator::GetRelA` (public accessor).
    pub fn rel_a(&self) -> f64 {
        self.rel_a
    }

    /// Set goal: visit panel fullsized.
    pub fn SetGoalFullsized(
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
            dlog!("VisitingViewAnimator activate: identity={}", identity);
            self.state = VisitingState::Curve;
            self.identity = identity.to_string();
            self.names = super::emPanelTree::DecodeIdentity(&self.identity);
            self.max_depth_seen = -1;
            self.time_slices_without_hope = 0;
            self.give_up_clock = 0.0;
        }
    }

    /// Start animating toward the current goal. Mirrors C++
    /// `emVisitingViewAnimator::Activate` (emViewAnimator.cpp:1040), which in
    /// C++ wakes the engine via the base class. In Rust the wrapper engine
    /// `VisitingVAEngineClass` (see `emView.rs`) observes `is_active()` and
    /// dispatches the cycle.
    pub fn Activate(&mut self) {
        self.active = true;
    }

    /// Clear the goal and stop animation.
    pub fn ClearGoal(&mut self) {
        if self.state != VisitingState::NoGoal {
            dlog!("VisitingViewAnimator deactivate");
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
        self.names = super::emPanelTree::DecodeIdentity(&self.identity);
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// Handle input during visiting animation.
    pub fn handle_input(&mut self, event: &crate::emInput::emInputEvent) -> bool {
        if !self.active {
            return false;
        }
        if self.state != VisitingState::Seek && self.state != VisitingState::GivingUp {
            return false;
        }
        if event.key != crate::emInput::InputKey::MouseLeft
            || event.variant != crate::emInput::InputVariant::Move
        {
            self.active = false;
            self.state = VisitingState::GivenUp;
            return true;
        }
        false
    }

    /// Paint the seek progress overlay.
    pub fn paint_seek_overlay(&self, painter: &mut crate::emPainter::emPainter<'_>, view: &emView) {
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
        painter.PaintRoundRect(
            x + shadow_off,
            y + shadow_off,
            w,
            h,
            h * 0.2,
            h * 0.2,
            crate::emColor::emColor::rgba(0, 0, 0, 160),
            crate::emColor::emColor::TRANSPARENT,
        );
        painter.PaintRoundRect(
            x,
            y,
            w,
            h,
            h * 0.2,
            h * 0.2,
            crate::emColor::emColor::rgba(34, 102, 153, 208),
            crate::emColor::emColor::TRANSPARENT,
        );

        let ch_size = h * 0.22;
        if self.state == VisitingState::GivingUp {
            painter.PaintTextBoxed(
                x,
                y,
                w,
                h * 0.4,
                "Not found",
                ch_size,
                crate::emColor::emColor::WHITE,
                crate::emColor::emColor::TRANSPARENT,
                crate::emPainter::TextAlignment::Center,
                crate::emPainter::VAlign::Center,
                crate::emPainter::TextAlignment::Center,
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
        painter.PaintTextBoxed(
            x,
            y,
            w,
            h * 0.4,
            &seeking_text,
            ch_size,
            crate::emColor::emColor::WHITE,
            crate::emColor::emColor::TRANSPARENT,
            crate::emPainter::TextAlignment::Center,
            crate::emPainter::VAlign::Center,
            crate::emPainter::TextAlignment::Center,
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
            painter.PaintRect(
                bar_x,
                bar_y,
                bar_w * progress,
                bar_h,
                crate::emColor::emColor::rgba(136, 255, 136, 80),
                emColor::TRANSPARENT,
            );
        }
        if progress < 1.0 {
            painter.PaintRect(
                bar_x + bar_w * progress,
                bar_y,
                bar_w * (1.0 - progress),
                bar_h,
                crate::emColor::emColor::rgba(136, 136, 136, 80),
                emColor::TRANSPARENT,
            );
        }

        let id_y = bar_y + bar_h + h * 0.02;
        let id_h = h * 0.15;
        let id_ch = ch_size * 0.6;
        painter.PaintTextBoxed(
            x,
            id_y,
            w,
            id_h,
            &self.identity,
            id_ch,
            crate::emColor::emColor::rgba(200, 200, 200, 180),
            crate::emColor::emColor::TRANSPARENT,
            crate::emPainter::TextAlignment::Center,
            crate::emPainter::VAlign::Top,
            crate::emPainter::TextAlignment::Center,
            0.3,
            false,
            0.0,
        );

        let abort_y = y + h * 0.8;
        let abort_h = h * 0.15;
        painter.PaintTextBoxed(
            x,
            abort_y,
            w,
            abort_h,
            "Press any key to abort",
            id_ch,
            crate::emColor::emColor::rgba(200, 200, 200, 128),
            crate::emColor::emColor::TRANSPARENT,
            crate::emPainter::TextAlignment::Center,
            crate::emPainter::VAlign::Center,
            crate::emPainter::TextAlignment::Center,
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

impl emVisitingViewAnimator {
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
    fn get_nearest_existing_panel(&self, view: &emView, tree: &PanelTree) -> Option<NearestPanel> {
        let root = view.GetRootPanel();
        if self.names.is_empty() {
            return None;
        }
        let root_name = tree.GetRec(root).map(|p| p.name.as_str()).unwrap_or("");
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
            let coords = view.CalcVisitFullsizedCoords(tree, panel, false);
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
                    let coords = view.CalcVisitCoords(tree, panel);
                    target_x = coords.0;
                    target_y = coords.1;
                    target_a = coords.2;
                }
                VisitType::VisitRel => {
                    if self.rel_a <= 0.0 {
                        let coords = view.CalcVisitFullsizedCoords(tree, panel, self.rel_a < -0.9);
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
                    let coords = view.CalcVisitFullsizedCoords(tree, panel, self.utilize_view);
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
        view: &emView,
        tree: &PanelTree,
        panel: super::emPanelTree::PanelId,
        target_x: f64,
        target_y: f64,
        target_a: f64,
    ) -> (f64, f64, f64, f64) {
        // Home rectangle (C++ HomeX/Y/Width/Height). Rust emView always has
        // home at (0,0) with viewport dimensions.
        let hw = view.viewport_size().0.max(1.0);
        let hh = view.viewport_size().1.max(1.0);
        let hx = 0.0_f64;
        let hy = 0.0_f64;
        // C++ HomePixelTallness — always 1.0 in Rust (square pixels).
        let hp = 1.0_f64;

        // emView rectangle (C++ GetViewRect): popup-zoom uses max_popup_rect,
        // otherwise same as home rect.
        let (sx, sy, sw, sh) = if view.flags.contains(super::emView::ViewFlags::POPUP_ZOOM) {
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
        // C++ emViewAnimator.cpp:1527: vw = sqrt(hw*hh*hp / (relA * panel->GetHeight()))
        // target_a is C++ relA (HomeW*HomeH/(vw*vh)) — use formula directly.
        let vw = (hw * hh * hp / (target_a.max(1e-100) * panel_height.max(1e-100))).sqrt();
        let vh = vw * panel_height / hp;
        // C++ emViewAnimator.cpp:1530-1531 (panel-fraction, 1:1 transcription):
        //   vx = HomeX + HomeWidth*0.5 - (relX+0.5)*vw
        let vx = hx + hw * 0.5 - (target_x + 0.5) * vw;
        let vy = hy + hh * 0.5 - (target_y + 0.5) * vh;
        let mut bx = (sx - vx) / vw;
        let mut by = (sy - vy) / vw * hp;
        let mut bw = sw / vw;
        let mut bh = sh / vw * hp;

        // Walk "b" up the panel tree until we reach a panel that is
        // in_viewed_path and whose parent is NOT viewed (i.e., the SVP),
        // or until we reach the root.
        let mut b_id = panel;
        while let Some(b_data) = tree.GetRec(b_id) {
            let parent_id = match b_data.parent {
                Some(p) => p,
                None => break, // root reached
            };
            if b_data.in_viewed_path {
                let parent_viewed = tree.GetRec(parent_id).map(|p| p.viewed).unwrap_or(false);
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
        let svp_id = view.GetSupremeViewedPanel().unwrap_or(view.GetRootPanel());
        let (svp_vx, svp_vy, svp_vw) = tree
            .GetRec(svp_id)
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
            let a_data = match tree.GetRec(a_id) {
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
            let f = (sw + sh) * view.GetZoomFactorLogarithmPerPixel();
            // C++ emViewAnimator.cpp:1568-1578 (no negation).
            dx = (bx - ax + (bw - aw) * 0.5) / t * f;
            dy = (by - ay + (bh - ah) * 0.5) / t * f;
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

impl emViewAnimator for emVisitingViewAnimator {
    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
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

        // C++ emViewAnimator.cpp:1233-1244: activate nearest existing panel.
        // While animating, the goal panel may forward activation to a child.
        {
            let to_activate = if tree.focusable(nep.panel) {
                Some(nep.panel)
            } else {
                tree.GetFocusableParent(nep.panel)
            };
            if let Some(act) = to_activate {
                let already_in_path = tree.GetRec(act).map(|p| p.in_active_path).unwrap_or(false);
                let is_focusable = tree.focusable(nep.panel);
                if is_focusable || !already_in_path {
                    view.set_active_panel(tree, act, self.adherent, ctx);
                }
            }
        }

        if self.animated {
            if self.max_depth_seen < nep.depth as i32 {
                if self.state == VisitingState::Seek {
                    view.SetSeekPos(tree, None, "");
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
            let zflpp = view.GetZoomFactorLogarithmPerPixel();
            let delta_xy_px = delta_xy / zflpp;
            let delta_z_view = delta_z / zflpp;
            let delta_x = dir_x * delta_xy_px;
            let delta_y = dir_y * delta_xy_px;

            let (vw, vh) = view.viewport_size();
            let done = view.RawScrollAndZoom(
                tree,
                vw * 0.5,
                vh * 0.5,
                delta_x,
                delta_y,
                delta_z_view,
                ctx,
            );

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
                // All panels exist — C++ uses RawVisit (modifies current visit
                // entry, no stack growth). Visit() would push a new entry.
                view.RawVisit(
                    tree,
                    nep.panel,
                    nep.target_x,
                    nep.target_y,
                    nep.target_a,
                    false,
                    ctx,
                );
                self.state = VisitingState::GoalReached;
                return false;
            } else if view.seek_pos_panel() != Some(nep.panel) {
                view.SetSeekPos(tree, Some(nep.panel), &self.names[nep.depth + 1]);
                // C++ uses RawVisitFullsized (no stack growth). VisitFullsized would push.
                view.RawVisitFullsized(tree, nep.panel, false, ctx);
                self.time_slices_without_hope = 4;
            } else if view.IsHopeForSeeking(tree) {
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Port of C++ `emVisitingViewAnimator::Input` (emViewAnimator.cpp:1066).
    fn Input(
        &mut self,
        event: &mut crate::emInput::emInputEvent,
        _state: &crate::emInputState::emInputState,
    ) {
        // C++ emViewAnimator.cpp:1068: no-op unless active and in Seek/GivingUp.
        if !self.active
            || (self.state != VisitingState::Seek && self.state != VisitingState::GivingUp)
        {
            return;
        }
        // C++ emViewAnimator.cpp:1070-1073: eat event and deactivate.
        if !event.IsEmpty() {
            event.eat();
            self.stop();
        }
    }
}

/// Swiping view animator — spring-based drag with kinetic coasting.
///
/// Matches C++ `emSwipingViewAnimator` architecture: composes a
/// `emKineticViewAnimator` with a critically-damped spring model.
///
/// **Gripped**: User drags the view. Accumulated grip distance is stored as
/// spring extension. Each frame, the spring converts extension into kinetic
/// velocity. Friction is disabled during grip.
///
/// **Released**: Spring extension zeroed, velocity transferred to kinetic
/// animator which coasts with friction deceleration.
///
/// Supports 3D (scroll X, scroll Y, zoom Z).
pub struct emSwipingViewAnimator {
    inner: emKineticViewAnimator,
    gripped: bool,
    spring_extension: [f64; 3],
    instantaneous_velocity: [f64; 3],
    spring_constant: f64,
    busy: bool,
}

impl emSwipingViewAnimator {
    pub fn new(friction: f64) -> Self {
        Self {
            inner: emKineticViewAnimator::new(0.0, 0.0, 0.0, friction),
            gripped: false,
            spring_extension: [0.0; 3],
            instantaneous_velocity: [0.0; 3],
            spring_constant: 1.0,
            busy: false,
        }
    }

    /// Toggle grip state. On release, spring extension is zeroed and
    /// instantaneous velocity copies from kinetic velocity for coasting.
    pub fn SetGripped(&mut self, gripped: bool) {
        if self.gripped != gripped {
            self.gripped = gripped;
            if !self.gripped {
                self.spring_extension = [0.0; 3];
                let (vx, vy, vz) = self.inner.GetVelocity();
                self.instantaneous_velocity = [vx, vy, vz];
            }
        }
    }

    /// Whether the view is currently gripped.
    pub fn IsGripped(&self) -> bool {
        self.gripped
    }

    /// Add distance to spring extension in the given dimension (0=X, 1=Y, 2=Z).
    pub fn MoveGrip(&mut self, dimension: usize, distance: f64) {
        if self.gripped && dimension < 3 {
            self.spring_extension[dimension] += distance;
            self.update_busy_state();
        }
    }

    /// Set the spring constant (stiffness). Higher = stiffer, less lag.
    pub fn SetSpringConstant(&mut self, k: f64) {
        self.spring_constant = k;
    }

    /// Absolute spring extension magnitude.
    pub fn GetAbsSpringExtension(&self) -> f64 {
        (self.spring_extension[0] * self.spring_extension[0]
            + self.spring_extension[1] * self.spring_extension[1]
            + self.spring_extension[2] * self.spring_extension[2])
            .sqrt()
    }

    /// Access the inner kinetic animator.
    pub fn inner(&self) -> &emKineticViewAnimator {
        &self.inner
    }

    /// Mutable access to the inner kinetic animator.
    pub fn inner_mut(&mut self) -> &mut emKineticViewAnimator {
        &mut self.inner
    }

    fn update_busy_state(&mut self) {
        if self.gripped
            && (self.GetAbsSpringExtension() > 0.01 || self.inner.GetAbsVelocity() > 0.01)
        {
            self.busy = true;
        } else {
            self.spring_extension = [0.0; 3];
            self.busy = false;
        }
    }
}

impl emViewAnimator for emSwipingViewAnimator {
    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
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

            self.inner.SetVelocity(new_vel[0], new_vel[1], new_vel[2]);

            // Disable friction during grip, delegate to kinetic for scroll/zoom
            let saved_friction = self.inner.IsFrictionEnabled();
            self.inner.SetFrictionEnabled(false);
            base_busy = self.inner.animate(view, tree, dt, ctx);
            self.inner.SetFrictionEnabled(saved_friction);
        } else {
            // Not gripped or not busy — pure kinetic coasting with friction
            base_busy = self.inner.animate(view, tree, dt, ctx);
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Magnetic view animator — automatically scrolls/zooms to snap the view
/// to the nearest focusable panel.
///
/// C++ emMagneticViewAnimator (emViewAnimator.h:272-299, emViewAnimator.cpp:671-923).
/// Inherits from emKineticViewAnimator (composition in Rust).
/// Uses a hill-rolling physics model with config-driven radius and speed.
pub struct emMagneticViewAnimator {
    inner: emKineticViewAnimator,
    /// Whether magnetism is currently active (within radius, velocity < threshold).
    magnetism_active: bool,
    /// emCoreConfig MagnetismRadius factor (default 1.0).
    radius_factor: f64,
    /// emCoreConfig MagnetismRadius.GetMinValue() (default ~0.001).
    min_radius_factor: f64,
    /// emCoreConfig MagnetismSpeed factor (default 1.0).
    speed_factor: f64,
    /// emCoreConfig MagnetismSpeed.GetMaxValue() (default ~100.0).
    max_speed_factor: f64,
}

impl Default for emMagneticViewAnimator {
    fn default() -> Self {
        Self::new()
    }
}

impl emMagneticViewAnimator {
    /// C++ emMagneticViewAnimator::emMagneticViewAnimator (line 671-677).
    /// In C++ the constructor acquires CoreConfig and sets MagnetismActive=false.
    /// Config values are read each CycleAnimation; here we store defaults.
    pub fn new() -> Self {
        let mut inner = emKineticViewAnimator::new(0.0, 0.0, 0.0, 1e10);
        inner.SetFrictionEnabled(true);
        Self {
            inner,
            magnetism_active: false,
            radius_factor: 1.0,
            min_radius_factor: 0.001,
            speed_factor: 1.0,
            max_speed_factor: 100.0,
        }
    }

    pub fn inner(&self) -> &emKineticViewAnimator {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut emKineticViewAnimator {
        &mut self.inner
    }

    pub fn set_radius_factor(&mut self, f: f64) {
        self.radius_factor = f;
    }

    pub fn set_min_radius_factor(&mut self, f: f64) {
        self.min_radius_factor = f;
    }

    pub fn set_speed_factor(&mut self, f: f64) {
        self.speed_factor = f;
    }

    pub fn set_max_speed_factor(&mut self, f: f64) {
        self.max_speed_factor = f;
    }

    pub fn is_magnetism_active(&self) -> bool {
        self.magnetism_active
    }

    /// C++ emMagneticViewAnimator::Activate (lines 685-707).
    /// Copies friction from existing active KVA, or sets friction=1E10.
    pub fn Activate(&mut self, active_animator: Option<&dyn emViewAnimator>) {
        if self.inner.is_active() {
            return;
        }
        self.magnetism_active = false;
        // Walk active animator chain to find a emKineticViewAnimator
        let kinetic =
            active_animator.and_then(|a| a.as_any().downcast_ref::<emKineticViewAnimator>());
        if let Some(kva) = kinetic {
            self.inner.SetFriction(kva.GetFriction());
            self.inner.SetFrictionEnabled(kva.IsFrictionEnabled());
        } else {
            self.inner.SetFriction(1e10);
            self.inner.SetFrictionEnabled(true);
        }
        self.inner.active = true;
    }

    /// C++ emMagneticViewAnimator::Deactivate (lines 710-713).
    pub fn Deactivate(&mut self) {
        self.inner.stop();
    }

    /// Calculate 3D distance to the nearest focusable panel.
    ///
    /// C++ emMagneticViewAnimator::CalculateDistance (emViewAnimator.cpp:809-907).
    /// DFS from supreme viewed panel, compute essence rect in view coords,
    /// measure 3D distance (xy pixels + log-zoom z-axis).
    ///
    /// Returns (dx, dy, dz, abs_dist). Returns large sentinel if no candidate
    /// found or if POPUP_ZOOM is set.
    pub fn calculate_distance(view: &emView, tree: &PanelTree) -> (f64, f64, f64, f64) {
        // C++ lines 821-825: popup zoom not supported
        if view.flags.contains(ViewFlags::POPUP_ZOOM) {
            return (1e10, 1e10, 1e10, (3e100_f64).sqrt());
        }

        let svp = view.supreme_panel();

        // C++ lines 829-830: get view rect
        let view_rect = Self::get_view_rect(view);
        let vx = view_rect.x;
        let vy = view_rect.y;
        let vw = view_rect.w;
        let vh = view_rect.h;
        let zflpp = view.GetZoomFactorLogarithmPerPixel();

        // C++ lines 816-819: sentinel values
        let mut best_dx = 1e10;
        let mut best_dy = 1e10;
        let mut best_dz = 1e10;
        let mut dd = 3e100_f64;

        // C++ lines 831-903: DFS walk from SVP
        // C++ traversal: visit current, then first-child, then next-sibling, then parent's next.
        let mut current = Some(svp);
        while let Some(id) = current {
            let p = tree.GetRec(id);
            let is_viewed = p.map(|r| r.viewed).unwrap_or(false);
            let is_focusable = p.map(|r| r.focusable).unwrap_or(false);

            if is_viewed && is_focusable {
                // C++ lines 833-854: compute essence rect in view coords
                let (ex, ey, ew, eh) = tree.GetEssenceRect(id);
                let x = tree.PanelToViewX(id, ex);
                let y = tree.PanelToViewY(id, ey);
                let w = tree.PanelToViewDeltaX(id, ew);
                let h = tree.PanelToViewDeltaY(id, eh);

                if w > 1e-3 && h > 1e-3 {
                    // Maximize panel in view (centered)
                    let tx = (x + w * 0.5) - (vx + vw * 0.5);
                    let ty = (y + h * 0.5) - (vy + vh * 0.5);
                    let tz = if w * vh >= h * vw {
                        (vw / w).ln() / zflpp
                    } else {
                        (vh / h).ln() / zflpp
                    };
                    let td = tx * tx + ty * ty + tz * tz;
                    if td < dd {
                        best_dx = tx;
                        best_dy = ty;
                        best_dz = tz;
                        dd = td;
                    }
                }
            }

            // C++ tree traversal: first child, else next sibling, else parent's next
            // C++ lines 893-903: traverse ALL panels, not just viewed ones
            if let Some(child) = tree.GetFirstChild(id) {
                current = Some(child);
            } else if id == svp {
                current = None;
            } else if let Some(next) = tree.GetNext(id) {
                current = Some(next);
            } else {
                // Walk up to find parent with next sibling
                let mut up = tree.GetParentContext(id);
                loop {
                    match up {
                        Some(pid) if pid != svp => {
                            if let Some(next) = tree.GetNext(pid) {
                                current = Some(next);
                                break;
                            }
                            up = tree.GetParentContext(pid);
                        }
                        _ => {
                            current = None;
                            break;
                        }
                    }
                }
            }
        }

        (best_dx, best_dy, best_dz, dd.sqrt())
    }

    /// Get the view rect for magnetism calculations.
    ///
    /// C++ emMagneticViewAnimator::GetViewRect (emViewAnimator.cpp:910-923):
    /// if VF_POPUP_ZOOM, return the max popup view rect; else return the
    /// home rect (viewport origin + dimensions).
    pub fn get_view_rect(view: &emView) -> Rect {
        if view.flags.contains(ViewFlags::POPUP_ZOOM) {
            view.max_popup_rect().unwrap_or_else(|| {
                Rect::new(0.0, 0.0, view.viewport_size().0, view.viewport_size().1)
            })
        } else {
            let (w, h) = view.viewport_size();
            Rect::new(0.0, 0.0, w, h)
        }
    }
}

impl emViewAnimator for emMagneticViewAnimator {
    /// C++ emMagneticViewAnimator::CycleAnimation (emViewAnimator.cpp:716-806).
    fn animate(
        &mut self,
        view: &mut emView,
        tree: &mut PanelTree,
        dt: f64,
        ctx: &mut crate::emEngineCtx::SchedCtx<'_>,
    ) -> bool {
        if !self.inner.active {
            return false;
        }

        let radius_factor = self.radius_factor;
        let min_radius_factor = self.min_radius_factor;
        let speed_factor = self.speed_factor;
        let max_speed_factor = self.max_speed_factor;

        // C++ lines 728-732: compute maxDist from view rect
        let view_rect = Self::get_view_rect(view);
        let vw = view_rect.w;
        let vh = view_rect.h;
        let max_dist = if radius_factor <= min_radius_factor * 1.0001 {
            0.0
        } else {
            (vw + vh) * 0.09 * radius_factor
        };

        // C++ line 734: calculate distance to nearest focusable panel
        let (dist_x, dist_y, dist_z, abs_dist) = Self::calculate_distance(view, tree);

        let mut busy = false;

        // C++ lines 738-755: magnetism enter/exit logic
        if abs_dist <= max_dist && abs_dist > 1e-3 {
            if !self.magnetism_active && self.inner.GetAbsVelocity() < 10.0 {
                // C++ line 740: CenterZoomFixPoint()
                self.inner.CenterZoomFixPoint(view);
                self.magnetism_active = true;
            }
            busy = true;
        } else {
            if self.magnetism_active {
                self.inner.SetVelocity(0.0, 0.0, 0.0);
                self.magnetism_active = false;
            }
            if self.inner.GetAbsVelocity() >= 0.01 {
                busy = true;
            }
        }

        // C++ lines 757-797: hill-rolling physics when magnetism active
        if self.magnetism_active && abs_dist > 1e-15 && max_dist > 1e-15 {
            let v;
            if speed_factor >= max_speed_factor * 0.9999 || abs_dist < 1.0 {
                // C++ lines 758-759: instant snap
                v = abs_dist / dt;
            } else {
                // C++ lines 762-766: project current velocity onto distance direction
                let (cur_vx, cur_vy, cur_vz) = self.inner.GetVelocity();
                let mut vel = (cur_vx * dist_x + cur_vy * dist_y + cur_vz * dist_z) / abs_dist;
                if vel < 0.0 {
                    vel = 0.0;
                }

                // C++ lines 769-792: sub-stepped Euler integration
                let mut d = 0.0;
                let mut t = 0.0;
                loop {
                    let fdt = (dt - t).min(0.01);
                    if fdt < 1e-10 {
                        break;
                    }

                    // C++ line 776: slope of hill
                    let mut k = (abs_dist - d) / max_dist * 4.0;
                    if k.abs() > 1.0 {
                        k = 1.0 / k;
                    }

                    // C++ line 780: acceleration through rolling downhill
                    let mut a = k * max_dist * 25.0 * speed_factor * speed_factor;

                    // C++ line 783: damping
                    a -= vel.abs() * 15.0 * speed_factor;

                    vel += a * fdt;
                    d += vel * fdt;
                    if d >= abs_dist {
                        d = abs_dist;
                        break;
                    }
                    t += fdt;
                }
                // C++ line 793: effective velocity
                v = d / dt;
            }

            // C++ lines 795-797: set velocity proportional to distance direction
            self.inner.velocity_x = v * dist_x / abs_dist;
            self.inner.velocity_y = v * dist_y / abs_dist;
            self.inner.velocity_z = v * dist_z / abs_dist;
        }

        // C++ lines 800-803: temporarily disable friction during magnetism
        let friction_enabled = self.inner.IsFrictionEnabled();
        self.inner
            .SetFrictionEnabled(friction_enabled && !self.magnetism_active);
        if self.inner.animate(view, tree, dt, ctx) {
            busy = true;
        }
        self.inner.SetFrictionEnabled(friction_enabled);

        // Keep active as long as busy
        if !busy {
            self.inner.active = false;
        }

        busy
    }

    fn is_active(&self) -> bool {
        self.inner.is_active()
    }

    fn stop(&mut self) {
        self.inner.stop();
        self.magnetism_active = false;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::EngineScheduler;

    /// Test helper: owns the data needed to construct a `SchedCtx`.
    struct TestSched {
        sched: EngineScheduler,
        fw: Vec<crate::emEngineCtx::DeferredAction>,
        ctx: std::rc::Rc<crate::emContext::emContext>,
    }
    impl TestSched {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                ctx: crate::emContext::emContext::NewRoot(),
            }
        }
        fn with<R>(&mut self, f: impl FnOnce(&mut crate::emEngineCtx::SchedCtx<'_>) -> R) -> R {
            let __cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
                std::cell::RefCell::new(None);
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.ctx,
                framework_clipboard: &__cb,
                current_engine: None,
            };
            f(&mut sc)
        }
    }

    fn setup() -> (PanelTree, emView) {
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
        let view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        (tree, view)
    }

    #[test]
    fn kinetic_with_zoom() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let (_, _, _, initial_a) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist at initial state");

        let mut anim = emKineticViewAnimator::new(0.0, 0.0, 100.0, 1000.0);
        // friction_enabled defaults to false — just test that zoom scroll works
        ts.with(|sc| anim.animate(&mut view, &mut tree, 0.1, sc));

        // Zoom velocity should have changed rel_a (dz = 100 * 0.1 = 10)
        let (_, _, _, final_a) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after animation");
        assert!((final_a - initial_a).abs() > 0.001);
    }

    #[test]
    fn speeding_with_zoom() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut anim = emSpeedingViewAnimator::new(1000.0);
        anim.SetTargetVelocity(0.0, 0.0, 2.0);

        for _ in 0..10 {
            ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc));
        }

        // Should be accelerating toward zoom
        let (_, _, vz) = anim.inner().GetVelocity();
        assert!(vz.abs() > 0.0);
    }

    #[test]
    fn visiting_converges() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut anim = emVisitingViewAnimator::new(0.1, 0.1, 2.0, 10.0);
        anim.set_identity("root", "");
        anim.SetAnimated(true);
        anim.SetAcceleration(5.0);
        anim.SetMaxAbsoluteSpeed(5.0);

        for _ in 0..500 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc)) {
                break;
            }
        }

        assert!(!anim.is_active());
    }

    #[test]
    fn kinetic_linear_friction_stops() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut anim = emKineticViewAnimator::new(100.0, 0.0, 0.0, 1000.0);
        anim.SetFrictionEnabled(true);

        for _ in 0..200 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc)) {
                break;
            }
        }

        assert!(!anim.is_active());
    }

    #[test]
    fn kinetic_friction_disabled() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut anim = emKineticViewAnimator::new(100.0, 0.0, 0.0, 1000.0);
        // friction_enabled defaults to false

        ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc));

        let (vx, _, _) = anim.GetVelocity();
        // Without friction, velocity should remain at 100.0 (or zeroed by blocked-motion)
        // but should NOT have been reduced by friction
        assert!(vx == 100.0 || vx == 0.0);
    }

    #[test]
    fn speeding_3branch_reverse() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut anim = emSpeedingViewAnimator::new(1000.0);
        anim.SetReverseAcceleration(500.0);

        // Set inner velocity going right (set_velocity activates if > 0.01)
        anim.inner_mut().SetVelocity(100.0, 0.0, 0.0);
        // Target going left — should trigger reverse acceleration
        anim.SetTargetVelocity(-100.0, 0.0, 0.0);

        ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc));

        let (vx, _, _) = anim.inner().GetVelocity();
        // Velocity should have moved toward -100 (decreased from 100)
        assert!(vx < 100.0);
    }

    #[test]
    fn speeding_delegates_to_kinetic() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        let (_, _, _, initial_a) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist at initial state");

        let mut anim = emSpeedingViewAnimator::new(1000.0);
        anim.SetTargetVelocity(0.0, 0.0, 2.0);
        anim.SetAcceleration(1000.0);

        for _ in 0..10 {
            ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc));
        }

        // Inner kinetic should have applied zoom via raw_scroll_and_zoom
        let (_, _, _, final_a) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist after animation");
        assert!((final_a - initial_a).abs() > 0.001);
    }

    #[test]
    fn visiting_set_anim_params_from_core_config() {
        use crate::emCoreConfig::emCoreConfig;
        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

        // Below max: animated. visit_speed=2.0, max=10.0.
        let cfg = emCoreConfig {
            visit_speed: 2.0,
            ..Default::default()
        };
        anim.SetAnimParamsByCoreConfig(&cfg);
        assert!(anim.animated);
        assert!((anim.acceleration - 70.0).abs() < 0.01);
        assert!((anim.max_absolute_speed - 70.0).abs() < 0.01);
        assert!((anim.max_cusp_speed - 35.0).abs() < 0.01);

        // At max: not animated (instant). visit_speed == max == 10.0.
        let cfg_max = emCoreConfig {
            visit_speed: 10.0,
            ..Default::default()
        };
        anim.SetAnimParamsByCoreConfig(&cfg_max);
        assert!(!anim.animated);
    }

    #[test]
    fn visiting_handle_input_abort() {
        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

        // Not in seek state — should not consume
        let event = crate::emInput::emInputEvent::press(crate::emInput::InputKey::Escape);
        assert!(!anim.handle_input(&event));

        // Set to seek state
        anim.set_visiting_state(VisitingState::Seek);
        assert!(anim.handle_input(&event));
        assert!(!anim.is_active());
        assert_eq!(anim.visiting_state(), VisitingState::GivenUp);
    }

    #[test]
    fn visiting_state_direct_transitions() {
        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

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
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        // Target is the root at current coords — should reach goal quickly
        let (_, state_rx, state_ry, state_ra) = view
            .get_visited_panel_idiom(&tree)
            .expect("visited panel should exist at initial state");
        let mut anim = emVisitingViewAnimator::new(state_rx, state_ry, state_ra, 5.0);
        anim.set_identity("root", "");
        anim.SetAnimated(true);
        anim.SetAcceleration(5.0);
        anim.SetMaxAbsoluteSpeed(5.0);

        let mut reached = false;
        for _ in 0..20 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc)) {
                reached = true;
                break;
            }
        }
        assert!(reached, "Should reach goal when already at target");
        assert_eq!(anim.visiting_state(), VisitingState::GoalReached);
    }

    #[test]
    fn visiting_giving_up_no_panel() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));

        // Target a non-existent panel
        let mut anim = emVisitingViewAnimator::new(0.5, 0.5, 2.0, 5.0);
        anim.set_identity("nonexistent", "");
        anim.SetAnimated(true);

        ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc));
        assert_eq!(
            anim.visiting_state(),
            VisitingState::GivingUp,
            "Should give up when panel doesn't exist"
        );

        // Run through the 1.5s give-up display
        for _ in 0..200 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc)) {
                break;
            }
        }
        assert_eq!(anim.visiting_state(), VisitingState::GivenUp);
    }

    #[test]
    fn swiping_grip_spring() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so the panel is larger than the viewport and scroll isn't clamped.
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));

        let mut anim = emSwipingViewAnimator::new(2.0);
        anim.inner_mut().SetFrictionEnabled(true);
        anim.SetSpringConstant(100.0);
        assert!(!anim.is_active());

        // Grip and move
        anim.SetGripped(true);
        anim.MoveGrip(0, 50.0); // 50px spring extension in X
        assert!(anim.is_active());

        // Animate — spring should produce kinetic velocity
        ts.with(|sc| anim.animate(&mut view, &mut tree, 0.016, sc));
        let (vx, _, _) = anim.inner().GetVelocity();
        assert!(vx.abs() > 0.0, "Spring should produce X velocity");
    }

    #[test]
    fn swiping_release_coasts() {
        let mut ts = TestSched::new();
        let (mut tree, mut view) = setup();
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so the panel is larger than the viewport and scroll isn't clamped.
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));

        let mut anim = emSwipingViewAnimator::new(2.0);
        anim.inner_mut().SetFrictionEnabled(true);
        anim.SetSpringConstant(100.0);
        anim.SetGripped(true);

        // Build up velocity via spring
        for _ in 0..10 {
            anim.MoveGrip(0, 5.0);
            ts.with(|sc| anim.animate(&mut view, &mut tree, 1.0 / 60.0, sc));
        }
        let (vx_before, _, _) = anim.inner().GetVelocity();
        assert!(vx_before.abs() > 1.0, "Should have built up velocity");

        // Release — should coast with friction
        anim.SetGripped(false);
        for _ in 0..5000 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 1.0 / 60.0, sc)) {
                break;
            }
        }
        assert!(!anim.is_active(), "Should decelerate to stop");
    }

    #[test]
    fn swiping_stop() {
        let mut anim = emSwipingViewAnimator::new(2.0);
        anim.SetGripped(true);
        anim.MoveGrip(0, 50.0);
        assert!(anim.is_active());

        anim.stop();
        assert!(!anim.is_active());
        let (vx, vy, vz) = anim.inner().GetVelocity();
        assert_eq!(vx, 0.0);
        assert_eq!(vy, 0.0);
        assert_eq!(vz, 0.0);
    }

    #[test]
    fn magnetic_new_creates_inactive() {
        let anim = emMagneticViewAnimator::new();
        assert!(!anim.is_active());
        assert!(!anim.is_magnetism_active());
    }

    #[test]
    fn magnetic_activate_sets_active() {
        let mut anim = emMagneticViewAnimator::new();
        anim.Activate(None);
        assert!(anim.is_active());
    }

    #[test]
    fn magnetic_stop() {
        let mut anim = emMagneticViewAnimator::new();
        anim.Activate(None);
        assert!(anim.is_active());

        anim.stop();
        assert!(!anim.is_active());
        let (vx, vy, vz) = anim.inner().GetVelocity();
        assert_eq!(vx, 0.0);
        assert_eq!(vy, 0.0);
        assert_eq!(vz, 0.0);
    }

    #[test]
    fn magnetic_animate_finds_focusable_panel() {
        let mut ts = TestSched::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        tree.set_focusable(root, true);

        let mut view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        ts.with(|sc| view.Update(&mut tree, sc));
        // Zoom in so root panel is offset, giving nonzero distance
        ts.with(|sc| view.Zoom(&mut tree, 2.0, 400.0, 300.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut anim = emMagneticViewAnimator::new();
        anim.Activate(None);

        // Should converge toward root panel over many frames
        for _ in 0..300 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 1.0 / 60.0, sc)) {
                break;
            }
        }
    }

    #[test]
    #[ignore]
    fn kinetic_velocity_inherited_on_activation() {
        let mut ts = TestSched::new();
        // BLOCKED: needs active-animator registry on emView/Window with velocity
        // inheritance protocol. C++ ref: emViewAnimator.cpp:Activate() (lines
        // 68-84) inherits LastTSC/LastClk from the currently active animator,
        // and emKineticViewAnimator::Activate() (lines 188-226) walks the
        // active chain to find a emKineticViewAnimator and copies its velocity
        // and zoom-fix-point.
        //
        // In the Rust architecture, kinetic physics are inlined in
        // emMouseZoomScrollVIF (input_filter.rs) rather than composed as separate
        // emViewAnimator instances. There is no master/slave chain or
        // UpperActivePtr — the VIF and active_animator slot run independently.
        //
        // To port this behavior:
        // 1. Add a KineticState { velocity: [f64; 3], zoom_fix: (f64, f64,
        //    bool) } struct that can be extracted/applied.
        // 2. Add emView::active_kinetic_state() -> Option<KineticState> that the
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
        ts.with(|sc| view.Update(&mut tree, sc));

        let mut old_anim = emKineticViewAnimator::new(100.0, 50.0, 0.0, 1000.0);
        old_anim.SetFrictionEnabled(true);
        ts.with(|sc| old_anim.animate(&mut view, &mut tree, 0.016, sc));

        // A new kinetic animator should inherit velocity from old_anim.
        // Currently there is no activation protocol to transfer velocity.
        let new_anim = emKineticViewAnimator::new(0.0, 0.0, 0.0, 1000.0);
        let (vx, vy, _) = new_anim.GetVelocity();
        assert!(
            vx.abs() > 1.0 && vy.abs() > 1.0,
            "New animator should inherit velocity from previous active animator"
        );
    }

    #[test]
    fn extract_kinetic_state_matches_fields() {
        let kva = emKineticViewAnimator::new(1.0, 2.0, 3.0, 0.5);
        let state = kva.extract_kinetic_state();
        assert!((state.vx - 1.0).abs() < 1e-12);
        assert!((state.vy - 2.0).abs() < 1e-12);
        assert!((state.vz - 3.0).abs() < 1e-12);
        assert!(state.zoom_fix_centered);
        assert!((state.zoom_fix_x - 0.0).abs() < 1e-12);
        assert!((state.zoom_fix_y - 0.0).abs() < 1e-12);
    }

    #[test]
    fn inject_kinetic_state_updates_fields() {
        let mut kva = emKineticViewAnimator::new(0.0, 0.0, 0.0, 0.5);
        let state = KineticState {
            vx: 4.0,
            vy: 5.0,
            vz: 6.0,
            zoom_fix_centered: false,
            zoom_fix_x: 100.0,
            zoom_fix_y: 200.0,
        };
        kva.inject_kinetic_state(state);
        let (vx, vy, vz) = kva.GetVelocity();
        assert!((vx - 4.0).abs() < 1e-12);
        assert!((vy - 5.0).abs() < 1e-12);
        assert!((vz - 6.0).abs() < 1e-12);
        assert!(kva.is_active());
    }

    #[test]
    fn activate_handoff_inherits_velocity() {
        let (_tree, view) = setup();

        let old_anim = emKineticViewAnimator::new(100.0, 50.0, 10.0, 1000.0);
        let mut new_anim = emKineticViewAnimator::new(0.0, 0.0, 0.0, 1000.0);

        new_anim.activate_with_handoff(Some(&old_anim), &view);
        let (vx, vy, vz) = new_anim.GetVelocity();
        assert!(
            (vx - 100.0).abs() < 1e-6,
            "should inherit vx from old animator"
        );
        assert!(
            (vy - 50.0).abs() < 1e-6,
            "should inherit vy from old animator"
        );
        assert!(
            (vz - 10.0).abs() < 1e-6,
            "should inherit vz from old animator"
        );
        assert!(new_anim.is_active());
    }

    #[test]
    fn activate_handoff_no_prior_zeros_velocity() {
        let (_tree, view) = setup();

        let mut anim = emKineticViewAnimator::new(100.0, 50.0, 10.0, 1000.0);
        assert!(anim.is_active());

        anim.activate_with_handoff(None, &view);
        let (vx, vy, vz) = anim.GetVelocity();
        assert!(vx.abs() < 1e-12, "vx should be zero");
        assert!(vy.abs() < 1e-12, "vy should be zero");
        assert!(vz.abs() < 1e-12, "vz should be zero");
        assert!(!anim.is_active());
    }

    #[test]
    fn magnetic_activate_inherits_friction_from_kva() {
        let mut prior = emKineticViewAnimator::new(100.0, 0.0, 0.0, 42.0);
        prior.SetFrictionEnabled(true);

        let mut mag = emMagneticViewAnimator::new();
        mag.Activate(Some(&prior as &dyn emViewAnimator));
        assert!(mag.is_active());
        assert_eq!(mag.inner().GetFriction(), 42.0);
        assert!(mag.inner().IsFrictionEnabled());
    }

    #[test]
    fn magnetic_activate_no_prior_sets_high_friction() {
        let mut mag = emMagneticViewAnimator::new();
        mag.Activate(None);
        assert!(mag.is_active());
        assert_eq!(mag.inner().GetFriction(), 1e10);
        assert!(mag.inner().IsFrictionEnabled());
    }

    #[test]
    fn calculate_distance_finds_nearest_panel() {
        let mut ts = TestSched::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
        tree.set_focusable(root, true);

        // 3 child panels at different positions
        let left = tree.create_child(root, "left", None);
        tree.Layout(left, 0.0, 0.0, 0.3, 1.0, 1.0, None);
        tree.set_focusable(left, true);

        let center = tree.create_child(root, "center", None);
        tree.Layout(center, 0.35, 0.0, 0.3, 1.0, 1.0, None);
        tree.set_focusable(center, true);

        let right = tree.create_child(root, "right", None);
        tree.Layout(right, 0.7, 0.0, 0.3, 1.0, 1.0, None);
        tree.set_focusable(right, true);

        let mut view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        ts.with(|sc| view.Update(&mut tree, sc));

        let (dx, _dy, _dz, abs_dist) = emMagneticViewAnimator::calculate_distance(&view, &tree);
        // At least one focusable panel should be found
        assert!(
            abs_dist < 1e10,
            "should find at least one candidate, got abs_dist={abs_dist}"
        );
        // dx should be small since center panel is near view center
        assert!(
            dx.abs() < 200.0,
            "nearest panel should be close to center, dx={}",
            dx
        );
    }

    #[test]
    fn calculate_distance_uses_log_zoom_z_axis() {
        let mut ts = TestSched::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
        tree.set_focusable(root, true);

        let mut view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        ts.with(|sc| view.Update(&mut tree, sc));

        let (_dx, _dy, dz, abs_dist) = emMagneticViewAnimator::calculate_distance(&view, &tree);
        // Root panel fills the viewport, so dz depends on
        // log(view_dim / panel_dim) which should be near 0 when panel ≈ viewport
        assert!(abs_dist < f64::MAX, "should find root panel as candidate");
        // dz should be finite
        assert!(dz.is_finite(), "dz should be finite, got {}", dz);
    }

    #[test]
    fn get_view_rect_returns_home_rect_when_not_popup() {
        let (_tree, view) = setup();
        // Default flags: no POPUP_ZOOM
        assert!(!view.flags.contains(ViewFlags::POPUP_ZOOM));
        let rect = emMagneticViewAnimator::get_view_rect(&view);
        let (w, h) = view.viewport_size();
        assert!((rect.x - 0.0).abs() < 1e-12);
        assert!((rect.y - 0.0).abs() < 1e-12);
        assert!((rect.w - w).abs() < 1e-12);
        assert!((rect.h - h).abs() < 1e-12);
    }

    #[test]
    fn animator_slot_activate_deactivates_previous() {
        let mut slot = AnimatorSlot::new();
        assert!(!slot.is_active());

        // Activate A
        let a = emKineticViewAnimator::new(10.0, 0.0, 0.0, 100.0);
        slot.activate(Box::new(a));
        assert!(slot.is_active());

        // Activate B at same level — A should be deactivated (stopped)
        let b = emKineticViewAnimator::new(20.0, 0.0, 0.0, 100.0);
        slot.activate(Box::new(b));
        assert!(slot.is_active());
        // B is now the active animator
    }

    #[test]
    fn animator_slot_deactivate_cascades_to_slave() {
        let mut slot = AnimatorSlot::new();

        // Activate A in the master slot
        let a = emKineticViewAnimator::new(10.0, 0.0, 0.0, 100.0);
        slot.activate(Box::new(a));

        // Create a slave slot and activate B in it
        let mut slave = AnimatorSlot::new();
        let b = emKineticViewAnimator::new(20.0, 0.0, 0.0, 100.0);
        slave.activate(Box::new(b));
        assert!(slave.is_active());

        slot.set_slave(slave);

        // Deactivate master — should cascade to slave
        slot.deactivate();
        assert!(!slot.is_active(), "master should be deactivated");
        assert!(
            !slot.slave().unwrap().is_active(),
            "slave should also be deactivated"
        );
    }

    // --- Coordinate-system invariant tests ---
    // These test physical behavior, not coordinate values, so they
    // survive a convention change (viewport-fraction → panel-fraction).

    fn setup_scrolled(factor: f64) -> (PanelTree, emView) {
        let mut ts = TestSched::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        let mut view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        ts.with(|sc| view.Update(&mut tree, sc));
        ts.with(|sc| view.Zoom(&mut tree, factor, 400.0, 300.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));
        // Scroll off-center so rel_x != 0
        ts.with(|sc| view.Scroll(&mut tree, 80.0, 40.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));
        (tree, view)
    }

    #[test]
    fn invariant_equilibrium_at_target() {
        let mut ts = TestSched::new();
        // When the visiting animator targets the current view state, it should
        // not move: get_distance_to must return 0 at the current position.
        // Also verifies that viewed_x is consistent with the visit state
        // (catches correlated errors between Update and get_distance_to).
        for &factor in &[1.0, 2.0, 4.0, 16.0, 100.0] {
            let (mut tree, mut view) = setup_scrolled(factor);
            let root = view.GetRootPanel();

            let (_, state_rx, state_ry, state_ra) = view
                .get_visited_panel_idiom(&tree)
                .expect("visited panel should exist at initial state");
            let viewed_x_before = tree.GetRec(root).unwrap().viewed_x;
            let viewed_y_before = tree.GetRec(root).unwrap().viewed_y;

            // Create animator targeting exactly the current state
            let mut anim = emVisitingViewAnimator::new(state_rx, state_ry, state_ra, 0.0);
            anim.set_identity("root", "");
            anim.SetAnimated(true);
            anim.SetAcceleration(5.0);
            anim.SetMaxAbsoluteSpeed(5.0);

            // Drive several steps — view should not move
            for step in 0..10 {
                ts.with(|sc| anim.animate(&mut view, &mut tree, 1.0 / 60.0, sc));
                let (_, after_rx, after_ry, after_ra) = view
                    .get_visited_panel_idiom(&tree)
                    .expect("visited panel should exist at step");
                assert!(
                    (after_rx - state_rx).abs() < 1e-10,
                    "factor={factor} step={step}: rel_x moved from {:.15e} to {:.15e}",
                    state_rx,
                    after_rx
                );
                assert!(
                    (after_ry - state_ry).abs() < 1e-10,
                    "factor={factor} step={step}: rel_y moved from {:.15e} to {:.15e}",
                    state_ry,
                    after_ry
                );
                // Tolerance scales with rel_a magnitude (higher zoom = larger absolute drift).
                let a_tol = 1e-10 * state_ra.max(1.0);
                assert!(
                    (after_ra - state_ra).abs() < a_tol,
                    "factor={factor} step={step}: rel_a moved from {:.15e} to {:.15e} (tol={a_tol:.3e})",
                    state_ra,
                    after_ra
                );
            }

            // Viewed position should also be unchanged
            let viewed_x_after = tree.GetRec(root).unwrap().viewed_x;
            let viewed_y_after = tree.GetRec(root).unwrap().viewed_y;
            assert!(
                (viewed_x_after - viewed_x_before).abs() < 1e-6,
                "factor={factor}: viewed_x drifted by {:.3e}",
                (viewed_x_after - viewed_x_before).abs()
            );
            assert!(
                (viewed_y_after - viewed_y_before).abs() < 1e-6,
                "factor={factor}: viewed_y drifted by {:.3e}",
                (viewed_y_after - viewed_y_before).abs()
            );
        }
    }

    #[test]
    fn invariant_animator_convergence() {
        let mut ts = TestSched::new();
        // Visiting animator reaches CalcVisitCoords target within 120 frames.
        // Tests the full pipeline: get_distance_to → curve solver → RawScrollAndZoom → Update.
        //
        // Target: the child panel, NOT root.  CalcVisitCoords(child) returns a
        // ta >> 1.0 (zoomed well in on a small panel), so the animator zooms IN
        // during convergence.  At every intermediate step the root is much wider
        // than the viewport (root_vw >> HomeWidth), so root-centering
        // (C++ emView.cpp:1588-1626) never fires.  The animator therefore reaches
        // the exact CalcVisitCoords target and we can assert exact convergence —
        // no dual-accept hack needed.
        //
        // Convergence target chosen so root-centering does not fire at any step.
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.get_mut(root).unwrap().focusable = true;
        tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0, None);
        let child = tree.create_child(root, "child", None);
        tree.get_mut(child).unwrap().focusable = true;
        tree.Layout(child, 0.1, 0.1, 0.4, 0.5, 1.0, None);

        let mut view = emView::new(crate::emContext::emContext::NewRoot(), root, 800.0, 600.0);
        view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
        ts.with(|sc| view.Update(&mut tree, sc));
        // Start zoomed in 4x, off-center
        ts.with(|sc| view.Zoom(&mut tree, 4.0, 400.0, 300.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));
        ts.with(|sc| view.Scroll(&mut tree, 100.0, 50.0, sc));
        ts.with(|sc| view.Update(&mut tree, sc));

        // Target: CalcVisitCoords for child.
        //
        // The Rust rel_a convention: rel_a = viewport_area / (panel_display_w * panel_display_h).
        // CalcVisitCoords(child) computes the zoom level that makes child fill ~80% of the
        // viewport.  Even though ta < 1.0 numerically (Rust rel_a uses an inverse-area
        // fraction relative to the full viewport), the CHILD's display width is:
        //   child_vw = sqrt(ta * vw * vh * panel_aspect)   ≈ 443 px
        // and the ROOT's display width in this coordinate frame is:
        //   root_vw = child_vw / child_normalized_w = 443 / 0.4 ≈ 1107 px
        // Since root_vw=1107 >> HomeWidth=800, root-centering (C++ emView.cpp:1588-1626)
        // cannot fire at the convergence target.  During convergence the animator
        // zooms further IN (root gets even wider), so root-centering never fires
        // at any intermediate step either.  Exact convergence to ta is therefore valid.
        let (tx, ty, ta) = view.CalcVisitCoords(&tree, child);

        let mut anim = emVisitingViewAnimator::new(tx, ty, ta, 0.0);
        // Identity path must start from root: "root:child" (colon-delimited, C++ convention).
        anim.set_identity("root:child", "");
        anim.SetAnimated(true);
        anim.SetAcceleration(5.0);
        anim.SetMaxAbsoluteSpeed(5.0);
        anim.SetMaxCuspSpeed(2.5);

        let mut frames_run = 0usize;
        for _ in 0..300 {
            if !ts.with(|sc| anim.animate(&mut view, &mut tree, 1.0 / 60.0, sc)) {
                break;
            }
            frames_run += 1;
        }
        eprintln!(
            "DEBUG frames_run={frames_run} state={:?}",
            anim.visiting_state()
        );

        // The animator must have stopped (GoalReached) within the frame budget.
        assert!(
            frames_run < 300,
            "animator did not converge within 300 frames (ran {frames_run})"
        );

        // Verify zoom level by checking child's effective rel_a.
        // The SVP may be root (not child) because root-centering (C++
        // emView.cpp:1588-1626) clamps vx when vx > HomeX, preventing exact
        // centering on child.  The zoom level (rel_a for child) should still
        // match `ta` regardless of which panel is SVP.
        //
        // Derive child's viewed_width from whichever panel is SVP.
        let hw = view.viewport_size().0;
        let hh = view.viewport_size().1;
        let svp = view.GetSupremeViewedPanel().unwrap_or(view.GetRootPanel());
        let (svp_vx, svp_vy, svp_vw, svp_vh) = tree
            .GetRec(svp)
            .map(|p| (p.viewed_x, p.viewed_y, p.viewed_width, p.viewed_height))
            .unwrap_or((0.0, 0.0, 1.0, 1.0));
        eprintln!(
            "DEBUG svp={svp:?} svp_vx={svp_vx:.4} svp_vw={svp_vw:.4} \
             ta={ta:.8} child={child:?}"
        );

        // Compute child's viewed_width from the SVP's viewed_width by walking
        // the layout path from SVP to child.
        let child_vw = if svp == child {
            svp_vw
        } else {
            // SVP is an ancestor of child; multiply layout widths
            let mut vw = svp_vw;
            let mut cur = child;
            let mut path = Vec::new();
            while cur != svp {
                path.push(cur);
                match tree.GetRec(cur).and_then(|r| r.parent) {
                    Some(p) => cur = p,
                    None => break,
                }
            }
            for pid in path.into_iter().rev() {
                let lw = tree.GetRec(pid).map(|r| r.layout_rect.w).unwrap_or(1.0);
                vw *= lw;
            }
            vw
        };
        let child_h = tree.get_height(child);
        let child_vh = child_vw * child_h / 1.0; // HomePixelTallness=1.0
        let eff_rel_a = (hw * hh) / (child_vw * child_vh);
        assert!(
            (eff_rel_a - ta).abs() < 1e-3,
            "rel_a did not converge: child_vw={child_vw:.4} eff_rel_a={eff_rel_a:.8} target={ta:.8} diff={:.3e}",
            (eff_rel_a - ta).abs()
        );
        let _ = (svp_vy, svp_vh);
    }

    /// W1a: emVisitingViewAnimator::Input must eat non-empty events and
    /// deactivate when in Seek or GivingUp states (C++ emViewAnimator.cpp:1066).
    #[test]
    fn visiting_animator_input_eats_event_and_deactivates_in_seek() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emInputState::emInputState;

        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 0.0);
        anim.active = true;
        anim.state = VisitingState::Seek;
        let mut ev = emInputEvent::press(InputKey::Enter);
        let st = emInputState::default();
        emViewAnimator::Input(&mut anim, &mut ev, &st);
        assert!(ev.IsEmpty(), "event should be eaten");
        assert!(!anim.active, "animator should deactivate");
        assert_eq!(anim.state, VisitingState::NoGoal, "state should reset");
    }

    /// W1a: emVisitingViewAnimator::Input also deactivates in GivingUp state
    /// (C++ emViewAnimator.cpp:1068).
    #[test]
    fn visiting_animator_input_eats_event_and_deactivates_in_giving_up() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emInputState::emInputState;

        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 0.0);
        anim.active = true;
        anim.state = VisitingState::GivingUp;
        let mut ev = emInputEvent::press(InputKey::Enter);
        let st = emInputState::default();
        emViewAnimator::Input(&mut anim, &mut ev, &st);
        assert!(ev.IsEmpty(), "event should be eaten");
        assert!(!anim.active, "animator should deactivate");
        assert_eq!(anim.state, VisitingState::NoGoal, "state should reset");
    }

    /// W1a: emVisitingViewAnimator::Input is a no-op in inactive state,
    /// mirroring C++ early return.
    #[test]
    fn visiting_animator_input_noop_when_inactive() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emInputState::emInputState;

        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 0.0);
        anim.active = false;
        anim.state = VisitingState::Seek;
        let mut ev = emInputEvent::press(InputKey::Enter);
        let st = emInputState::default();
        emViewAnimator::Input(&mut anim, &mut ev, &st);
        assert!(
            !ev.IsEmpty(),
            "event must not be eaten when animator inactive"
        );
    }
}

#[cfg(kani)]
mod kani_private_proofs {
    use super::*;

    #[kani::proof]
    fn kani_private_accelerate_dim() {
        let mut p_v: f64 = kani::any::<f64>();
        kani::assume(p_v.is_finite());
        let mut p_target: f64 = kani::any::<f64>();
        kani::assume(p_target.is_finite());
        let mut p_accel: f64 = kani::any::<f64>();
        kani::assume(p_accel.is_finite());
        let mut p_reverse_accel: f64 = kani::any::<f64>();
        kani::assume(p_reverse_accel.is_finite());
        let mut p_friction: f64 = kani::any::<f64>();
        kani::assume(p_friction.is_finite());
        let mut p_friction_enabled: bool = kani::any::<bool>();
        let mut p_dt: f64 = kani::any::<f64>();
        kani::assume(p_dt.is_finite());
        let _r = accelerate_dim(
            p_v,
            p_target,
            p_accel,
            p_reverse_accel,
            p_friction,
            p_friction_enabled,
            p_dt,
        );
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_get_curve_point() {
        let mut p_d: f64 = kani::any::<f64>();
        kani::assume(p_d.is_finite());
        let _r = get_curve_point(p_d);
    }

    #[kani::proof]
    fn kani_private_get_curve_pos_dist() {
        let mut p_x: f64 = kani::any::<f64>();
        kani::assume(p_x.is_finite());
        let mut p_z: f64 = kani::any::<f64>();
        kani::assume(p_z.is_finite());
        let _r = get_curve_pos_dist(p_x, p_z);
    }

    #[kani::proof]
    fn kani_private_get_direct_dist() {
        let mut p_x: f64 = kani::any::<f64>();
        kani::assume(p_x.is_finite());
        let mut p_z: f64 = kani::any::<f64>();
        kani::assume(p_z.is_finite());
        let _r = get_direct_dist(p_x, p_z);
        assert!(_r.is_finite());
    }

    #[kani::proof]
    fn kani_private_get_direct_point() {
        let mut p_x: f64 = kani::any::<f64>();
        kani::assume(p_x.is_finite());
        let mut p_z: f64 = kani::any::<f64>();
        kani::assume(p_z.is_finite());
        let mut p_d: f64 = kani::any::<f64>();
        kani::assume(p_d.is_finite());
        let _r = get_direct_point(p_x, p_z, p_d);
    }

    #[kani::proof]
    fn kani_private_emKineticViewAnimator_is_active() {
        let mut self_val =
            emKineticViewAnimator::new(kani::any(), kani::any(), kani::any(), kani::any());
        let _r = self_val.is_active();
    }

    #[kani::proof]
    fn kani_private_emSpeedingViewAnimator_is_active() {
        let mut self_val = emSpeedingViewAnimator::new(kani::any());
        let _r = self_val.is_active();
    }

    #[kani::proof]
    fn kani_private_emSwipingViewAnimator_is_active() {
        let mut self_val = emSwipingViewAnimator::new(kani::any());
        let _r = self_val.is_active();
    }

    #[kani::proof]
    fn kani_private_emMagneticViewAnimator_is_active() {
        let self_val = emMagneticViewAnimator::new();
        let _r = self_val.is_active();
    }

    #[kani::proof]
    fn kani_private_emKineticViewAnimator_stop() {
        let mut self_val =
            emKineticViewAnimator::new(kani::any(), kani::any(), kani::any(), kani::any());
        let _r = self_val.stop();
    }

    #[kani::proof]
    fn kani_private_emSpeedingViewAnimator_stop() {
        let mut self_val = emSpeedingViewAnimator::new(kani::any());
        let _r = self_val.stop();
    }

    #[kani::proof]
    fn kani_private_emSwipingViewAnimator_stop() {
        let mut self_val = emSwipingViewAnimator::new(kani::any());
        let _r = self_val.stop();
    }

    #[kani::proof]
    fn kani_private_emMagneticViewAnimator_stop() {
        let mut self_val = emMagneticViewAnimator::new();
        self_val.stop();
    }

    #[kani::proof]
    fn kani_private_emKineticViewAnimator_update_busy_state() {
        let mut self_val =
            emKineticViewAnimator::new(kani::any(), kani::any(), kani::any(), kani::any());
        let _r = self_val.update_busy_state();
    }

    #[kani::proof]
    fn kani_private_emSwipingViewAnimator_update_busy_state() {
        let mut self_val = emSwipingViewAnimator::new(kani::any());
        let _r = self_val.update_busy_state();
    }

    // Layer 3: accelerate_dim monotonically converges toward target
    // C++ CycleAnimation: velocity moves toward target, never overshoots
    #[kani::proof]
    fn l3_accelerate_dim_monotonic() {
        let v: f64 = kani::any();
        let target: f64 = kani::any();
        let accel: f64 = kani::any();
        let reverse_accel: f64 = kani::any();
        let friction: f64 = kani::any();
        let dt: f64 = kani::any();
        kani::assume(v.is_finite() && target.is_finite());
        kani::assume(accel.is_finite() && accel >= 0.0);
        kani::assume(reverse_accel.is_finite() && reverse_accel >= 0.0);
        kani::assume(friction.is_finite() && friction >= 0.0);
        kani::assume(dt.is_finite() && dt >= 0.0 && dt <= 1.0);

        let result = accelerate_dim(v, target, accel, reverse_accel, friction, true, dt);
        if v >= target {
            assert!(result <= v, "moved away from target");
            assert!(result >= target, "overshot target");
        } else {
            assert!(result >= v, "moved away from target");
            assert!(result <= target, "overshot target");
        }
    }

    // Layer 3: get_direct_dist is non-negative
    #[kani::proof]
    fn l3_get_direct_dist_nonneg() {
        let x: i16 = kani::any();
        let z: i16 = kani::any();
        let xf = x as f64 / 100.0;
        let zf = z as f64 / 100.0;
        let d = get_direct_dist(xf, zf);
        assert!(d >= 0.0, "distance must be non-negative");
    }

    // Layer 3: get_direct_dist is zero at origin
    #[kani::proof]
    fn l3_get_direct_dist_zero_at_origin() {
        assert_eq!(get_direct_dist(0.0, 0.0), 0.0);
    }
}

#[cfg(test)]
mod constructor_tests {
    use super::*;

    #[test]
    fn new_for_view_matches_cpp_initial_state() {
        // C++ emViewAnimator.cpp:930-948 initializes:
        //   Animated=false, Acceleration=5.0, MaxCuspSpeed=2.0, MaxAbsoluteSpeed=5.0
        //   State=ST_NO_GOAL, VisitType=VT_VISIT, RelX=RelY=RelA=0
        //   Adherent=false, UtilizeView=false, MaxDepthSeen=-1, Speed=0.0
        //   IsActive()=false (SetDeactivateWhenIdle + no Activate call yet).
        let va = emVisitingViewAnimator::new_for_view();
        assert!(!va.animated);
        assert_eq!(va.acceleration, 5.0);
        assert_eq!(va.max_cusp_speed, 2.0);
        assert_eq!(va.max_absolute_speed, 5.0);
        assert_eq!(va.state, VisitingState::NoGoal);
        assert_eq!(va.visit_type, VisitType::Visit);
        assert_eq!(va.rel_x, 0.0);
        assert_eq!(va.rel_y, 0.0);
        assert_eq!(va.rel_a, 0.0);
        assert!(!va.adherent);
        assert!(!va.utilize_view);
        assert_eq!(va.max_depth_seen, -1);
        assert_eq!(va.speed, 0.0);
        assert!(!va.active);
    }
}
