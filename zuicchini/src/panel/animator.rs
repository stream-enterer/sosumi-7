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

/// Kinetic view animator — applies velocity with friction for smooth deceleration.
/// Used for fling/swipe gestures. Supports 3D (scroll x, scroll y, zoom z).
pub struct KineticViewAnimator {
    velocity_x: f64,
    velocity_y: f64,
    velocity_z: f64,
    friction: f64,
    active: bool,
}

impl KineticViewAnimator {
    pub fn new(velocity_x: f64, velocity_y: f64, velocity_z: f64, friction: f64) -> Self {
        Self {
            velocity_x,
            velocity_y,
            velocity_z,
            friction: friction.clamp(0.0, 1.0),
            active: velocity_x.abs() > 0.01 || velocity_y.abs() > 0.01 || velocity_z.abs() > 0.01,
        }
    }

    pub fn set_velocity(&mut self, vx: f64, vy: f64, vz: f64) {
        self.velocity_x = vx;
        self.velocity_y = vy;
        self.velocity_z = vz;
        self.active = vx.abs() > 0.01 || vy.abs() > 0.01 || vz.abs() > 0.01;
    }
}

impl ViewAnimator for KineticViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        let (vw, vh) = view.viewport_size();
        let fix_x = vw * 0.5;
        let fix_y = vh * 0.5;
        view.raw_scroll_and_zoom(
            tree,
            fix_x,
            fix_y,
            self.velocity_x * dt,
            self.velocity_y * dt,
            self.velocity_z * dt,
        );

        let decay = (1.0 - self.friction).powf(dt * 60.0);
        self.velocity_x *= decay;
        self.velocity_y *= decay;
        self.velocity_z *= decay;

        if self.velocity_x.abs() < 0.01
            && self.velocity_y.abs() < 0.01
            && self.velocity_z.abs() < 0.01
        {
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
        self.velocity_z = 0.0;
        self.active = false;
    }
}

/// Speeding view animator — accelerates toward a target velocity.
/// Used for keyboard-driven scrolling. Supports 3D.
pub struct SpeedingViewAnimator {
    target_vx: f64,
    target_vy: f64,
    target_vz: f64,
    current_vx: f64,
    current_vy: f64,
    current_vz: f64,
    acceleration: f64,
    active: bool,
}

impl SpeedingViewAnimator {
    pub fn new(acceleration: f64) -> Self {
        Self {
            target_vx: 0.0,
            target_vy: 0.0,
            target_vz: 0.0,
            current_vx: 0.0,
            current_vy: 0.0,
            current_vz: 0.0,
            acceleration,
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
}

impl ViewAnimator for SpeedingViewAnimator {
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        let accel = self.acceleration * dt;
        self.current_vx = approach(self.current_vx, self.target_vx, accel);
        self.current_vy = approach(self.current_vy, self.target_vy, accel);
        self.current_vz = approach(self.current_vz, self.target_vz, accel);

        let (vw, vh) = view.viewport_size();
        let fix_x = vw * 0.5;
        let fix_y = vh * 0.5;
        view.raw_scroll_and_zoom(
            tree,
            fix_x,
            fix_y,
            self.current_vx * dt,
            self.current_vy * dt,
            self.current_vz * dt,
        );

        if self.target_vx.abs() < 0.01
            && self.target_vy.abs() < 0.01
            && self.target_vz.abs() < 0.01
            && self.current_vx.abs() < 0.01
            && self.current_vy.abs() < 0.01
            && self.current_vz.abs() < 0.01
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
        self.current_vz = 0.0;
        self.target_vx = 0.0;
        self.target_vy = 0.0;
        self.target_vz = 0.0;
        self.active = false;
    }
}

/// Visiting view animator — smoothly animates the camera to a target visit state.
/// Uses logarithmic interpolation for zoom dimension.
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
    fn animate(&mut self, view: &mut View, tree: &mut PanelTree, dt: f64) -> bool {
        if !self.active {
            return false;
        }

        let t = (self.speed * dt).min(1.0);

        if let Some(state) = view.visit_stack().last().cloned() {
            let new_x = lerp(state.rel_x, self.target_x, t);
            let new_y = lerp(state.rel_y, self.target_y, t);
            // Logarithmic interpolation for zoom
            let log_a = state.rel_a.ln();
            let log_target = self.target_a.max(0.001).ln();
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

            // Check convergence
            if (new_x - self.target_x).abs() < 0.001
                && (new_y - self.target_y).abs() < 0.001
                && (new_log_a - log_target).abs() < 0.01
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

        let mut anim = KineticViewAnimator::new(0.0, 0.0, 1.0, 0.1);
        anim.animate(&mut view, &mut tree, 0.1);

        // Zoom velocity should have changed rel_a
        assert!((view.current_visit().rel_a - initial_a).abs() > 0.001);
    }

    #[test]
    fn speeding_with_zoom() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = SpeedingViewAnimator::new(100.0);
        anim.set_target(0.0, 0.0, 2.0);

        for _ in 0..10 {
            anim.animate(&mut view, &mut tree, 0.016);
        }

        // Should be accelerating toward zoom
        assert!(anim.current_vz.abs() > 0.0);
    }

    #[test]
    fn visiting_converges() {
        let (mut tree, mut view) = setup();
        view.update_viewing(&mut tree);

        let mut anim = VisitingViewAnimator::new(0.1, 0.1, 2.0, 10.0);

        for _ in 0..100 {
            if !anim.animate(&mut view, &mut tree, 0.016) {
                break;
            }
        }

        assert!(!anim.is_active());
    }
}
