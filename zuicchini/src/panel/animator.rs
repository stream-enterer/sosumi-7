use super::view::View;

/// Trait for view animation strategies.
pub trait ViewAnimator {
    /// Advance the animation by one frame. Returns true if still animating.
    fn animate(&mut self, view: &mut View, dt: f64) -> bool;

    /// Whether the animation is currently active.
    fn is_active(&self) -> bool;

    /// Stop the animation immediately.
    fn stop(&mut self);
}

/// Kinetic view animator — applies velocity with friction for smooth deceleration.
/// Used for fling/swipe gestures.
pub struct KineticViewAnimator {
    velocity_x: f64,
    velocity_y: f64,
    friction: f64,
    active: bool,
}

impl KineticViewAnimator {
    /// Create a kinetic animator with the given initial velocity and friction coefficient.
    pub fn new(velocity_x: f64, velocity_y: f64, friction: f64) -> Self {
        Self {
            velocity_x,
            velocity_y,
            friction: friction.clamp(0.0, 1.0),
            active: velocity_x.abs() > 0.01 || velocity_y.abs() > 0.01,
        }
    }

    /// Set new velocity (e.g., from a fling gesture).
    pub fn set_velocity(&mut self, vx: f64, vy: f64) {
        self.velocity_x = vx;
        self.velocity_y = vy;
        self.active = vx.abs() > 0.01 || vy.abs() > 0.01;
    }
}

impl ViewAnimator for KineticViewAnimator {
    fn animate(&mut self, view: &mut View, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        view.scroll(self.velocity_x * dt, self.velocity_y * dt);

        let decay = (1.0 - self.friction).powf(dt * 60.0);
        self.velocity_x *= decay;
        self.velocity_y *= decay;

        if self.velocity_x.abs() < 0.01 && self.velocity_y.abs() < 0.01 {
            self.active = false;
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

/// Speeding view animator — accelerates toward a target velocity.
/// Used for keyboard-driven scrolling.
pub struct SpeedingViewAnimator {
    target_vx: f64,
    target_vy: f64,
    current_vx: f64,
    current_vy: f64,
    acceleration: f64,
    active: bool,
}

impl SpeedingViewAnimator {
    pub fn new(acceleration: f64) -> Self {
        Self {
            target_vx: 0.0,
            target_vy: 0.0,
            current_vx: 0.0,
            current_vy: 0.0,
            acceleration,
            active: false,
        }
    }

    /// Set the target velocity.
    pub fn set_target(&mut self, vx: f64, vy: f64) {
        self.target_vx = vx;
        self.target_vy = vy;
        self.active = true;
    }

    /// Stop targeting (decelerate to zero).
    pub fn release(&mut self) {
        self.target_vx = 0.0;
        self.target_vy = 0.0;
    }
}

impl ViewAnimator for SpeedingViewAnimator {
    fn animate(&mut self, view: &mut View, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        let accel = self.acceleration * dt;

        // Accelerate toward target
        self.current_vx = approach(self.current_vx, self.target_vx, accel);
        self.current_vy = approach(self.current_vy, self.target_vy, accel);

        view.scroll(self.current_vx * dt, self.current_vy * dt);

        // Stop if target velocity is near zero and current velocity has decayed
        if self.target_vx.abs() < 0.01
            && self.target_vy.abs() < 0.01
            && self.current_vx.abs() < 0.01
            && self.current_vy.abs() < 0.01
        {
            self.active = false;
        }

        self.active
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.current_vx = 0.0;
        self.current_vy = 0.0;
        self.target_vx = 0.0;
        self.target_vy = 0.0;
        self.active = false;
    }
}

/// Visiting view animator — smoothly animates the camera to a target visit state.
pub struct VisitingViewAnimator {
    target_x: f64,
    target_y: f64,
    target_a: f64,
    speed: f64,
    active: bool,
}

impl VisitingViewAnimator {
    pub fn new(target_x: f64, target_y: f64, target_a: f64, speed: f64) -> Self {
        Self {
            target_x,
            target_y,
            target_a,
            speed,
            active: true,
        }
    }
}

impl ViewAnimator for VisitingViewAnimator {
    fn animate(&mut self, view: &mut View, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        let t = (self.speed * dt).min(1.0);

        if let Some(state) = view.visit_stack().last().cloned() {
            let new_x = lerp(state.rel_x, self.target_x, t);
            let new_y = lerp(state.rel_y, self.target_y, t);
            let new_a = lerp(state.rel_a, self.target_a, t);

            let dx = new_x - state.rel_x;
            let dy = new_y - state.rel_y;
            view.scroll(dx, dy);
            let zoom_factor = if state.rel_a > 0.0 {
                new_a / state.rel_a
            } else {
                1.0
            };
            view.zoom(zoom_factor, 0.0, 0.0);

            // Check if arrived (compare new interpolated values, not stale state)
            if (new_x - self.target_x).abs() < 0.01
                && (new_y - self.target_y).abs() < 0.01
                && (new_a - self.target_a).abs() < 0.001
            {
                self.active = false;
            }
        }

        self.active
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.active = false;
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn approach(current: f64, target: f64, step: f64) -> f64 {
    if (target - current).abs() < step {
        target
    } else if target > current {
        current + step
    } else {
        current - step
    }
}
