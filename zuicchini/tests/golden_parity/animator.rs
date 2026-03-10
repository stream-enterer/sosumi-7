use zuicchini::panel::{
    KineticViewAnimator, PanelTree, SpeedingViewAnimator, SwipingViewAnimator, View, ViewAnimator,
    ViewFlags, VisitingViewAnimator,
};

use super::common::*;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Create a PanelTree + View zoomed in deeply (matching C++ AnimViewSetup).
/// Returns (tree, view) ready for animator testing.
/// Set up view zoomed in moderately (rel_a ≈ 4). Gives room for both
/// scrolling (panel larger than viewport) and further zoom-in (rel_a < 1000).
/// The velocity trajectory is view-independent as long as no boundaries are hit.
fn setup_anim_view() -> (PanelTree, View) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);

    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.update_viewing(&mut tree);

    // Moderate zoom in: rel_a ≈ 4. Gives room to scroll (sqrt(4)=2x panel size)
    // and room for 60 zoom-in steps at vz=5 (max rel_a ≈ 4 * exp(5/60*60) ≈ 593).
    view.zoom(4.0, 400.0, 300.0);
    view.update_viewing(&mut tree);

    (tree, view)
}

/// Collect velocity trajectory from KineticViewAnimator.
fn run_kinetic_velocity_trajectory(
    tree: &mut PanelTree,
    view: &mut View,
    vx: f64,
    vy: f64,
    vz: f64,
    friction: f64,
    friction_enabled: bool,
    steps: usize,
) -> Vec<TrajectoryStep> {
    let mut anim = KineticViewAnimator::new(vx, vy, vz, friction);
    anim.set_friction_enabled(friction_enabled);

    let dt = 1.0 / 60.0;
    let mut trajectory = Vec::with_capacity(steps);

    for _ in 0..steps {
        anim.animate(view, tree, dt);
        let (vel_x, vel_y, vel_z) = anim.velocity();
        trajectory.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    trajectory
}

// ─── Kinetic trajectory tests ──────────────────────────────────

#[test]
fn animator_kinetic_fling_x() {
    require_golden!();
    let golden = load_trajectory_golden("animator_kinetic_fling_x");
    let (mut tree, mut view) = setup_anim_view();
    let actual =
        run_kinetic_velocity_trajectory(&mut tree, &mut view, 100.0, 0.0, 0.0, 2.0, true, 60);

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_kinetic_fling_x: {e}"));
}

#[test]
fn animator_kinetic_fling_xy() {
    require_golden!();
    let golden = load_trajectory_golden("animator_kinetic_fling_xy");
    let (mut tree, mut view) = setup_anim_view();
    let actual =
        run_kinetic_velocity_trajectory(&mut tree, &mut view, 100.0, 50.0, 0.0, 2.0, true, 60);

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_kinetic_fling_xy: {e}"));
}

#[test]
fn animator_kinetic_zoom() {
    require_golden!();
    let golden = load_trajectory_golden("animator_kinetic_zoom");
    let (mut tree, mut view) = setup_anim_view();
    let actual =
        run_kinetic_velocity_trajectory(&mut tree, &mut view, 0.0, 0.0, 5.0, 2.0, true, 60);

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_kinetic_zoom: {e}"));
}

// ─── Speeding trajectory tests ──────────────────────────────────

#[test]
fn animator_speeding_ramp() {
    require_golden!();
    let golden = load_trajectory_golden("animator_speeding_ramp");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = SpeedingViewAnimator::new(2.0);
    anim.inner_mut().set_friction_enabled(true);
    anim.set_acceleration(500.0);
    anim.set_reverse_acceleration(1000.0);
    anim.set_target(200.0, 0.0, 0.0);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for _ in 0..60 {
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().velocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_speeding_ramp: {e}"));
}

#[test]
fn animator_speeding_reverse() {
    require_golden!();
    let golden = load_trajectory_golden("animator_speeding_reverse");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = SpeedingViewAnimator::new(2.0);
    anim.inner_mut().set_friction_enabled(true);
    anim.inner_mut().set_velocity(100.0, 0.0, 0.0);
    anim.set_acceleration(500.0);
    anim.set_reverse_acceleration(1000.0);
    anim.set_target(-200.0, 0.0, 0.0);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for _ in 0..60 {
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().velocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_speeding_reverse: {e}"));
}

#[test]
fn animator_speeding_release() {
    require_golden!();
    let golden = load_trajectory_golden("animator_speeding_release");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = SpeedingViewAnimator::new(2.0);
    anim.inner_mut().set_friction_enabled(true);
    anim.set_acceleration(500.0);
    anim.set_reverse_acceleration(1000.0);
    anim.set_target(200.0, 0.0, 0.0);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        if i == 30 {
            anim.release();
        }
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().velocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_speeding_release: {e}"));
}

// ─── Swiping trajectory tests ──────────────────────────────────

#[test]
fn animator_swiping_grip() {
    require_golden!();
    let golden = load_trajectory_golden("animator_swiping_grip");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = SwipingViewAnimator::new(2.0);
    anim.inner_mut().set_friction_enabled(true);
    anim.set_spring_constant(100.0);
    anim.set_gripped(true);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        if i < 10 {
            anim.move_grip(0, 5.0);
        }
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().velocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_swiping_grip: {e}"));
}

#[test]
fn animator_swiping_release() {
    require_golden!();
    let golden = load_trajectory_golden("animator_swiping_release");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = SwipingViewAnimator::new(2.0);
    anim.inner_mut().set_friction_enabled(true);
    anim.set_spring_constant(100.0);
    anim.set_gripped(true);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        if i < 10 {
            anim.move_grip(0, 5.0);
        }
        if i == 20 {
            anim.set_gripped(false);
        }
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().velocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_swiping_release: {e}"));
}

// ─── Visiting trajectory tests ──────────────────────────────────

/// Collect position trajectory from VisitingViewAnimator.
fn run_visiting_trajectory(
    tree: &mut PanelTree,
    view: &mut View,
    target_x: f64,
    target_y: f64,
    target_a: f64,
    steps: usize,
) -> Vec<TrajectoryStep> {
    let mut anim = VisitingViewAnimator::new(target_x, target_y, target_a, 0.0);
    anim.set_identity("root", "");
    anim.set_animated(true);
    anim.set_acceleration(5.0);
    anim.set_max_absolute_speed(5.0);
    anim.set_max_cusp_speed(2.5);

    let dt = 1.0 / 60.0;
    let mut trajectory = Vec::with_capacity(steps);

    for _ in 0..steps {
        anim.animate(view, tree, dt);
        let visit = view.current_visit();
        // Golden data stores (rel_x, rel_y, rel_a) in Rust convention
        trajectory.push(TrajectoryStep {
            vel_x: visit.rel_x,
            vel_y: visit.rel_y,
            vel_z: visit.rel_a,
        });
    }

    trajectory
}

#[test]
fn animator_visiting_short() {
    require_golden!();
    let golden = load_trajectory_golden("animator_visiting_short");
    let (mut tree, mut view) = setup_anim_view();
    let actual = run_visiting_trajectory(&mut tree, &mut view, 0.1, 0.1, 2.0, 60);

    compare_trajectory(&actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("animator_visiting_short: {e}"));
}

#[test]
fn animator_visiting_zoom() {
    require_golden!();
    let golden = load_trajectory_golden("animator_visiting_zoom");
    let (mut tree, mut view) = setup_anim_view();
    let actual = run_visiting_trajectory(&mut tree, &mut view, 0.0, 0.0, 16.0, 60);

    compare_trajectory(&actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("animator_visiting_zoom: {e}"));
}
