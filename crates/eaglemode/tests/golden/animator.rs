use emcore::emPanelTree::PanelTree;
use emcore::emView::{emView, ViewFlags};
use emcore::emViewAnimator::{
    emKineticViewAnimator, emMagneticViewAnimator, emSpeedingViewAnimator, emSwipingViewAnimator,
    emViewAnimator, emVisitingViewAnimator,
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

/// Create a PanelTree + emView zoomed in deeply (matching C++ AnimViewSetup).
/// Returns (tree, view) ready for animator testing.
/// Set up view zoomed in moderately (rel_a ≈ 4). Gives room for both
/// scrolling (panel larger than viewport) and further zoom-in (rel_a < 1000).
/// The velocity trajectory is view-independent as long as no boundaries are hit.
fn setup_anim_view() -> (PanelTree, emView) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.Update(&mut tree);

    // Moderate zoom in: factor=2 matches C++ VisitAnimViewSetup Zoom(400,300,2.0).
    // rel_a ≈ 4 (ra *= 1/4). Gives room to scroll and zoom further in.
    view.Zoom(2.0, 400.0, 300.0);
    view.Update(&mut tree);

    (tree, view)
}

/// Collect velocity trajectory from emKineticViewAnimator.
#[allow(clippy::too_many_arguments)]
fn run_kinetic_velocity_trajectory(
    tree: &mut PanelTree,
    view: &mut emView,
    vx: f64,
    vy: f64,
    vz: f64,
    friction: f64,
    friction_enabled: bool,
    steps: usize,
) -> Vec<TrajectoryStep> {
    let mut anim = emKineticViewAnimator::new(vx, vy, vz, friction);
    anim.SetFrictionEnabled(friction_enabled);

    let dt = 1.0 / 60.0;
    let mut trajectory = Vec::with_capacity(steps);

    for _ in 0..steps {
        anim.animate(view, tree, dt);
        let (vel_x, vel_y, vel_z) = anim.GetVelocity();
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

    compare_trajectory("animator_kinetic_fling_x", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_kinetic_fling_x: {e}"));
}

#[test]
fn animator_kinetic_fling_xy() {
    require_golden!();
    let golden = load_trajectory_golden("animator_kinetic_fling_xy");
    let (mut tree, mut view) = setup_anim_view();
    let actual =
        run_kinetic_velocity_trajectory(&mut tree, &mut view, 100.0, 50.0, 0.0, 2.0, true, 60);

    compare_trajectory("animator_kinetic_fling_xy", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_kinetic_fling_xy: {e}"));
}

#[test]
fn animator_kinetic_zoom() {
    require_golden!();
    let golden = load_trajectory_golden("animator_kinetic_zoom");
    let (mut tree, mut view) = setup_anim_view();
    let actual =
        run_kinetic_velocity_trajectory(&mut tree, &mut view, 0.0, 0.0, 5.0, 2.0, true, 60);

    compare_trajectory("animator_kinetic_zoom", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_kinetic_zoom: {e}"));
}

// ─── Speeding trajectory tests ──────────────────────────────────

#[test]
fn animator_speeding_ramp() {
    require_golden!();
    let golden = load_trajectory_golden("animator_speeding_ramp");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = emSpeedingViewAnimator::new(2.0);
    anim.inner_mut().SetFrictionEnabled(true);
    anim.SetAcceleration(500.0);
    anim.SetReverseAcceleration(1000.0);
    anim.SetTargetVelocity(200.0, 0.0, 0.0);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for _ in 0..60 {
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().GetVelocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory("animator_speeding_ramp", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_speeding_ramp: {e}"));
}

#[test]
fn animator_speeding_reverse() {
    require_golden!();
    let golden = load_trajectory_golden("animator_speeding_reverse");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = emSpeedingViewAnimator::new(2.0);
    anim.inner_mut().SetFrictionEnabled(true);
    anim.inner_mut().SetVelocity(100.0, 0.0, 0.0);
    anim.SetAcceleration(500.0);
    anim.SetReverseAcceleration(1000.0);
    anim.SetTargetVelocity(-200.0, 0.0, 0.0);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for _ in 0..60 {
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().GetVelocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory("animator_speeding_reverse", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_speeding_reverse: {e}"));
}

#[test]
fn animator_speeding_release() {
    require_golden!();
    let golden = load_trajectory_golden("animator_speeding_release");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = emSpeedingViewAnimator::new(2.0);
    anim.inner_mut().SetFrictionEnabled(true);
    anim.SetAcceleration(500.0);
    anim.SetReverseAcceleration(1000.0);
    anim.SetTargetVelocity(200.0, 0.0, 0.0);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        if i == 30 {
            anim.release();
        }
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().GetVelocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory("animator_speeding_release", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_speeding_release: {e}"));
}

// ─── Swiping trajectory tests ──────────────────────────────────

#[test]
fn animator_swiping_grip() {
    require_golden!();
    let golden = load_trajectory_golden("animator_swiping_grip");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = emSwipingViewAnimator::new(2.0);
    anim.inner_mut().SetFrictionEnabled(true);
    anim.SetSpringConstant(100.0);
    anim.SetGripped(true);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        if i < 10 {
            anim.MoveGrip(0, 5.0);
        }
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().GetVelocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory("animator_swiping_grip", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_swiping_grip: {e}"));
}

#[test]
fn animator_swiping_release() {
    require_golden!();
    let golden = load_trajectory_golden("animator_swiping_release");
    let (mut tree, mut view) = setup_anim_view();

    let mut anim = emSwipingViewAnimator::new(2.0);
    anim.inner_mut().SetFrictionEnabled(true);
    anim.SetSpringConstant(100.0);
    anim.SetGripped(true);

    let dt = 1.0 / 60.0;
    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        if i < 10 {
            anim.MoveGrip(0, 5.0);
        }
        if i == 20 {
            anim.SetGripped(false);
        }
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().GetVelocity();
        actual.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    compare_trajectory("animator_swiping_release", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("animator_swiping_release: {e}"));
}

// ─── Visiting trajectory tests ──────────────────────────────────

/// Collect GetPos trajectory from emVisitingViewAnimator.
fn run_visiting_trajectory(
    tree: &mut PanelTree,
    view: &mut emView,
    target_x: f64,
    target_y: f64,
    target_a: f64,
    steps: usize,
) -> Vec<TrajectoryStep> {
    let mut anim = emVisitingViewAnimator::new(target_x, target_y, target_a, 0.0);
    anim.set_identity("root", "");
    anim.SetAnimated(true);
    anim.SetAcceleration(5.0);
    anim.SetMaxAbsoluteSpeed(5.0);
    anim.SetMaxCuspSpeed(2.5);

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

    if dump_golden_enabled() {
        save_trajectory_golden("animator_visiting_short", &actual);
    }
    for (i, (a, g)) in actual.iter().zip(golden.iter()).enumerate() {
        let dx = (a.vel_x - g.vel_x).abs();
        let dy = (a.vel_y - g.vel_y).abs();
        let dz = (a.vel_z - g.vel_z).abs();
        let flag = if dx > 1e-4 || dy > 1e-4 || dz > 1e-4 {
            " <<< FAIL"
        } else {
            ""
        };
        eprintln!("step {i:2}: actual=({:.10e}, {:.10e}, {:.10e})  golden=({:.10e}, {:.10e}, {:.10e})  diff=({dx:.3e}, {dy:.3e}, {dz:.3e}){flag}",
            a.vel_x, a.vel_y, a.vel_z, g.vel_x, g.vel_y, g.vel_z);
    }
    // Rust rel_x now uses panel-fraction convention matching C++ relX.
    // Trajectory matches C++ to machine epsilon (~1e-12).
    compare_trajectory("animator_visiting_short", &actual, &golden, 1e-10)
        .unwrap_or_else(|e| panic!("animator_visiting_short: {e}"));
}

/// Same as setup_anim_view but with a SQUARE panel (height=1.0) on a 4:3
/// viewport. This makes panel_aspect != viewport_aspect, exercising the
/// scroll denominator fix (BUG-8) which is invisible at matching aspects.
fn setup_anim_view_square_panel() -> (PanelTree, emView) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0); // square panel

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.Update(&mut tree);

    view.Zoom(2.0, 400.0, 300.0);
    view.Update(&mut tree);

    (tree, view)
}

#[test]
fn animator_visiting_square_panel() {
    let (mut tree, mut view) = setup_anim_view_square_panel();
    let actual = run_visiting_trajectory(&mut tree, &mut view, 0.1, 0.1, 2.0, 60);

    // Rust-native golden: no C++ reference for non-matching-aspect Restore.
    // Generate with DUMP_GOLDEN=1; thereafter compare.
    if dump_golden_enabled() {
        save_trajectory_golden("animator_visiting_square_panel", &actual);
    }
    let golden = load_trajectory_golden("animator_visiting_square_panel");
    compare_trajectory("animator_visiting_square_panel", &actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("animator_visiting_square_panel: {e}"));
}

#[test]
fn animator_visiting_zoom() {
    require_golden!();
    let golden = load_trajectory_golden("animator_visiting_zoom");
    let (mut tree, mut view) = setup_anim_view();
    let actual = run_visiting_trajectory(&mut tree, &mut view, 0.0, 0.0, 16.0, 60);

    compare_trajectory("animator_visiting_zoom", &actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("animator_visiting_zoom: {e}"));
}

// ─── Magnetic trajectory tests ──────────────────────────────────

/// Run the magnetic animator for `steps` frames, recording 3D velocity trajectory.
/// Matches C++ gen_animator_magnetic_approach: AnimViewSetup (root at 0,0,1,0.75,
/// zoom factor 100), Activate, 60 frames of CycleAnimation, Deactivate.
fn run_magnetic_trajectory(steps: usize) -> Vec<TrajectoryStep> {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75);
    tree.set_focusable(root, true);

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.Update(&mut tree);

    // C++ AnimViewSetup: Zoom(400, 300, 100.0)
    view.Zoom(100.0, 400.0, 300.0);
    view.Update(&mut tree);

    let mut anim = emMagneticViewAnimator::new();
    // C++ Activate() with no prior active animator → friction=1E10
    anim.Activate(None);

    let dt = 1.0 / 60.0;
    let mut trajectory = Vec::with_capacity(steps);

    for _ in 0..steps {
        anim.animate(&mut view, &mut tree, dt);
        let (vel_x, vel_y, vel_z) = anim.inner().GetVelocity();
        trajectory.push(TrajectoryStep {
            vel_x,
            vel_y,
            vel_z,
        });
    }

    anim.Deactivate();

    trajectory
}

#[test]
fn animator_magnetic_approach() {
    require_golden!();
    let golden = load_trajectory_golden("animator_magnetic_approach");
    let actual = run_magnetic_trajectory(60);

    compare_trajectory("animator_magnetic_approach", &actual, &golden, 0.0)
        .unwrap_or_else(|e| panic!("animator_magnetic_approach: {e}"));
}
