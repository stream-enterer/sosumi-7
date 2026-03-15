use zuicchini::input::{InputEvent, InputKey, InputState};
use zuicchini::panel::{
    KeyboardZoomScrollVIF, MouseZoomScrollVIF, PanelTree, View, ViewFlags, ViewInputFilter,
};

use super::common::*;

macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found -- run `make -C golden_gen run` first");
            return;
        }
    };
}

/// C++ VIFTestSetup equivalent: 800x600, root panel (0,0,1,0.75),
/// ROOT_SAME_TALLNESS, zoom 10000x (matching C++ Zoom(400,300,100.0)),
/// window focused.
fn setup_vif_view() -> (PanelTree, View) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.set_layout_rect(root, 0.0, 0.0, 1.0, 0.75);
    let mut view = View::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.update_viewing(&mut tree);
    // C++ Zoom(400,300,100.0) -> ra = ra/(100^2) -> rel_a = 100^2 = 10000
    view.zoom(10000.0, 400.0, 300.0);
    view.update_viewing(&mut tree);
    view.set_window_focused(&mut tree, true);
    (tree, view)
}

/// C++ first frame uses dt=0.01 (TSC gap fallback), rest use dt=0.016.
const DT_FIRST: f64 = 0.01;
const DT_REST: f64 = 0.016;

fn dt_for_frame(i: usize) -> f64 {
    if i == 0 {
        DT_FIRST
    } else {
        DT_REST
    }
}

/// Initial fake clock value matching C++ TimedGoldenViewPort.
const CLOCK_INIT: u64 = 1_000_000;
/// Clock step per frame (16ms).
const CLOCK_STEP: u64 = 16;

/// Create a MouseZoomScrollVIF with C++ default config parameters.
fn setup_mouse_vif(view: &View) -> MouseZoomScrollVIF {
    let mut vif = MouseZoomScrollVIF::new();
    let zflpp = view.get_zoom_factor_log_per_pixel();
    // C++ default config: KineticZoomingAndScrolling=1.0, MinKZAS=0.25
    vif.set_mouse_anim_params(1.0, 0.25, zflpp);
    vif.set_wheel_anim_params(1.0, 0.25, zflpp);
    vif.set_test_clock(CLOCK_INIT);
    vif
}

/// Create a KeyboardZoomScrollVIF with C++ default config parameters.
fn setup_keyboard_vif(view: &View) -> KeyboardZoomScrollVIF {
    let mut vif = KeyboardZoomScrollVIF::new();
    let zflpp = view.get_zoom_factor_log_per_pixel();
    // C++ default config: kinetic=1.0, min=0.25, scroll/zoom speed=1.0
    vif.set_animator_params(1.0, 0.25, 1.0, 1.0, zflpp);
    vif
}

/// Record view state trajectory as (rel_x, rel_y, rel_a).
fn read_view_state(view: &View) -> TrajectoryStep {
    let visit = view.current_visit();
    TrajectoryStep {
        vel_x: visit.rel_x,
        vel_y: visit.rel_y,
        vel_z: visit.rel_a,
    }
}

// -- Wheel zoom tests --

#[test]
fn filter_wheel_zoom_in() {
    require_golden!();
    let golden = load_trajectory_golden("filter_wheel_zoom_in");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    // Frame 0: single wheel up at center
    let event = InputEvent::press(InputKey::WheelUp).with_mouse(400.0, 300.0);
    let mut state = InputState::new();
    state.set_mouse(400.0, 300.0);
    vif.filter(&event, &state, &mut view);

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.set_test_clock(CLOCK_INIT + (i as u64 + 1) * CLOCK_STEP);
        vif.animate_wheel(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_wheel_zoom_in: {e}"));
}

#[test]
fn filter_wheel_zoom_out() {
    require_golden!();
    let golden = load_trajectory_golden("filter_wheel_zoom_out");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    let event = InputEvent::press(InputKey::WheelDown).with_mouse(400.0, 300.0);
    let mut state = InputState::new();
    state.set_mouse(400.0, 300.0);
    vif.filter(&event, &state, &mut view);

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.set_test_clock(CLOCK_INIT + (i as u64 + 1) * CLOCK_STEP);
        vif.animate_wheel(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_wheel_zoom_out: {e}"));
}

#[test]
fn filter_wheel_acceleration() {
    require_golden!();
    let golden = load_trajectory_golden("filter_wheel_acceleration");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // 5 wheel-up events at frames 0, 3, 6, 9, 12 (before step)
        if i == 0 || i == 3 || i == 6 || i == 9 || i == 12 {
            vif.set_test_clock(CLOCK_INIT + i as u64 * CLOCK_STEP);
            let event = InputEvent::press(InputKey::WheelUp).with_mouse(400.0, 300.0);
            let mut state = InputState::new();
            state.set_mouse(400.0, 300.0);
            vif.filter(&event, &state, &mut view);
        }

        vif.set_test_clock(CLOCK_INIT + (i as u64 + 1) * CLOCK_STEP);
        vif.animate_wheel(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_wheel_acceleration: {e}"));
}

// -- Middle button pan/fling tests --

#[test]
fn filter_middle_pan() {
    require_golden!();
    let golden = load_trajectory_golden("filter_middle_pan");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    // Frame 0: middle press at (400, 300)
    {
        let event = InputEvent::press(InputKey::MouseMiddle).with_mouse(400.0, 300.0);
        let mut state = InputState::new();
        state.set_mouse(400.0, 300.0);
        state.press(InputKey::MouseMiddle);
        vif.filter(&event, &state, &mut view);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // Frames 1-10: move mouse 10px/frame
        if (1..=10).contains(&i) {
            let mx = 400.0 + i as f64 * 10.0;
            let my = 300.0 + i as f64 * 10.0;
            let event = InputEvent::mouse_move(InputKey::MouseMiddle, mx, my);
            let mut state = InputState::new();
            state.set_mouse(mx, my);
            state.press(InputKey::MouseMiddle);
            vif.filter(&event, &state, &mut view);
        }

        vif.animate_grip(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-6).unwrap_or_else(|e| panic!("filter_middle_pan: {e}"));
}

#[test]
fn filter_middle_fling() {
    require_golden!();
    let golden = load_trajectory_golden("filter_middle_fling");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    // Frame 0: middle press at (400, 300)
    {
        let event = InputEvent::press(InputKey::MouseMiddle).with_mouse(400.0, 300.0);
        let mut state = InputState::new();
        state.set_mouse(400.0, 300.0);
        state.press(InputKey::MouseMiddle);
        vif.filter(&event, &state, &mut view);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // Frames 1-10: move mouse 10px/frame
        if (1..=10).contains(&i) {
            let mx = 400.0 + i as f64 * 10.0;
            let my = 300.0 + i as f64 * 10.0;
            let event = InputEvent::mouse_move(InputKey::MouseMiddle, mx, my);
            let mut state = InputState::new();
            state.set_mouse(mx, my);
            state.press(InputKey::MouseMiddle);
            vif.filter(&event, &state, &mut view);
        }

        // Frame 10: release middle button (after move event)
        if i == 10 {
            let event = InputEvent::release(InputKey::MouseMiddle).with_mouse(500.0, 400.0);
            let mut state = InputState::new();
            state.set_mouse(500.0, 400.0);
            vif.filter(&event, &state, &mut view);
        }

        vif.animate_grip(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_middle_fling: {e}"));
}

// -- Keyboard VIF tests --

#[test]
fn filter_keyboard_scroll() {
    require_golden!();
    let golden = load_trajectory_golden("filter_keyboard_scroll");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_keyboard_vif(&view);

    // Frame 0: Alt+Right press (held for all 60 frames)
    {
        let mut event = InputEvent::press(InputKey::ArrowRight);
        event.alt = true;
        let mut state = InputState::new();
        state.press(InputKey::Alt);
        state.press(InputKey::ArrowRight);
        vif.filter(&event, &state, &mut view);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.animate(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_keyboard_scroll: {e}"));
}

#[test]
fn filter_keyboard_zoom() {
    require_golden!();
    let golden = load_trajectory_golden("filter_keyboard_zoom");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_keyboard_vif(&view);

    // Frame 0: Alt+PageUp press
    {
        let mut event = InputEvent::press(InputKey::PageUp);
        event.alt = true;
        let mut state = InputState::new();
        state.press(InputKey::Alt);
        state.press(InputKey::PageUp);
        vif.filter(&event, &state, &mut view);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.animate(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("filter_keyboard_zoom: {e}"));
}

#[test]
fn filter_keyboard_release() {
    require_golden!();
    let golden = load_trajectory_golden("filter_keyboard_release");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_keyboard_vif(&view);

    // Frame 0: Alt+Right press
    {
        let mut event = InputEvent::press(InputKey::ArrowRight);
        event.alt = true;
        let mut state = InputState::new();
        state.press(InputKey::Alt);
        state.press(InputKey::ArrowRight);
        vif.filter(&event, &state, &mut view);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // Frame 30: release Right (Alt still held)
        if i == 30 {
            let mut event = InputEvent::release(InputKey::ArrowRight);
            event.alt = true;
            let mut state = InputState::new();
            state.press(InputKey::Alt);
            vif.filter(&event, &state, &mut view);
        }

        vif.animate(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view));
    }

    compare_trajectory(&actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("filter_keyboard_release: {e}"));
}
