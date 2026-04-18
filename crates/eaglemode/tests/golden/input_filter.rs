use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPanelTree::PanelTree;

use emcore::emView::{emView, ViewFlags};

use emcore::emViewInputFilter::{emKeyboardZoomScrollVIF, emMouseZoomScrollVIF, emViewInputFilter};

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
fn setup_vif_view() -> (PanelTree, emView) {
    let mut tree = PanelTree::new();
    let root = tree.create_root("root");
    tree.Layout(root, 0.0, 0.0, 1.0, 0.75, 1.0);
    let mut view = emView::new_for_test(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::ROOT_SAME_TALLNESS);
    view.Update(&mut tree);
    // C++ Zoom(400,300,100.0) -> ra *= 1/100^2 -> rel_a *= 100^2 = 10000
    view.Zoom(&mut tree, 100.0, 400.0, 300.0);
    view.Update(&mut tree);
    view.SetFocused(&mut tree, true);
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

/// Initial fake clock GetValue matching C++ TimedGoldenViewPort.
const CLOCK_INIT: u64 = 1_000_000;
/// Clock step per frame (16ms).
const CLOCK_STEP: u64 = 16;

/// Create a emMouseZoomScrollVIF with C++ default config parameters.
fn setup_mouse_vif(view: &emView) -> emMouseZoomScrollVIF {
    let mut vif = emMouseZoomScrollVIF::new();
    let zflpp = view.GetZoomFactorLogarithmPerPixel();
    // C++ default config: KineticZoomingAndScrolling=1.0, MinKZAS=0.25
    vif.set_mouse_anim_params(1.0, 0.25, zflpp);
    vif.set_wheel_anim_params(1.0, 0.25, zflpp);
    vif.set_test_clock(CLOCK_INIT);
    vif
}

/// Create a emKeyboardZoomScrollVIF with C++ default config parameters.
fn setup_keyboard_vif(view: &emView) -> emKeyboardZoomScrollVIF {
    let mut vif = emKeyboardZoomScrollVIF::new();
    let zflpp = view.GetZoomFactorLogarithmPerPixel();
    // C++ default config: kinetic=1.0, min=0.25, scroll/zoom speed=1.0
    vif.set_animator_params(1.0, 0.25, 1.0, 1.0, zflpp);
    vif
}

/// Record view state trajectory as (rel_x, rel_y, 1/rel_a).
///
/// The C++ gen_golden stores `1/ra` (i.e., `vw*vh/(HomeW*HomeH)`, the old
/// "Rust scale-factor convention"). Rust now uses C++ convention internally
/// (`rel_a = HomeW*HomeH/(vw*vh)`), so invert here to keep golden data stable.
fn read_view_state(view: &emView, tree: &PanelTree) -> TrajectoryStep {
    let (_, rx, ry, ra) = view
        .get_visited_panel_idiom(tree)
        .expect("visited panel should exist at observation point");
    TrajectoryStep {
        vel_x: rx,
        vel_y: ry,
        vel_z: if ra > 1e-100 { 1.0 / ra } else { 1000.0 },
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
    let event = emInputEvent::press(InputKey::WheelUp).with_mouse(400.0, 300.0);
    let mut state = emInputState::new();
    state.set_mouse(400.0, 300.0);
    vif.filter(&event, &state, &mut view, &mut tree);

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.set_test_clock(CLOCK_INIT + (i as u64 + 1) * CLOCK_STEP);
        vif.animate_wheel(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_wheel_zoom_in", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_wheel_zoom_in: {e}"));
}

#[test]
fn filter_wheel_zoom_out() {
    require_golden!();
    let golden = load_trajectory_golden("filter_wheel_zoom_out");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    let event = emInputEvent::press(InputKey::WheelDown).with_mouse(400.0, 300.0);
    let mut state = emInputState::new();
    state.set_mouse(400.0, 300.0);
    vif.filter(&event, &state, &mut view, &mut tree);

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.set_test_clock(CLOCK_INIT + (i as u64 + 1) * CLOCK_STEP);
        vif.animate_wheel(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_wheel_zoom_out", &actual, &golden, 1e-6)
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
            let event = emInputEvent::press(InputKey::WheelUp).with_mouse(400.0, 300.0);
            let mut state = emInputState::new();
            state.set_mouse(400.0, 300.0);
            vif.filter(&event, &state, &mut view, &mut tree);
        }

        vif.set_test_clock(CLOCK_INIT + (i as u64 + 1) * CLOCK_STEP);
        vif.animate_wheel(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_wheel_acceleration", &actual, &golden, 1e-6)
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
        let event = emInputEvent::press(InputKey::MouseMiddle).with_mouse(400.0, 300.0);
        let mut state = emInputState::new();
        state.set_mouse(400.0, 300.0);
        state.press(InputKey::MouseMiddle);
        vif.filter(&event, &state, &mut view, &mut tree);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // Frames 1-10: move mouse 10px/frame
        if (1..=10).contains(&i) {
            let mx = 400.0 + i as f64 * 10.0;
            let my = 300.0 + i as f64 * 10.0;
            let event = emInputEvent::mouse_move(InputKey::MouseMiddle, mx, my);
            let mut state = emInputState::new();
            state.set_mouse(mx, my);
            state.press(InputKey::MouseMiddle);
            vif.filter(&event, &state, &mut view, &mut tree);
        }

        vif.animate_grip(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_middle_pan", &actual, &golden, 1e-6)
        .unwrap_or_else(|e| panic!("filter_middle_pan: {e}"));
}

#[test]
fn filter_middle_fling() {
    require_golden!();
    let golden = load_trajectory_golden("filter_middle_fling");
    let (mut tree, mut view) = setup_vif_view();
    let mut vif = setup_mouse_vif(&view);

    // Frame 0: middle press at (400, 300)
    {
        let event = emInputEvent::press(InputKey::MouseMiddle).with_mouse(400.0, 300.0);
        let mut state = emInputState::new();
        state.set_mouse(400.0, 300.0);
        state.press(InputKey::MouseMiddle);
        vif.filter(&event, &state, &mut view, &mut tree);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // Frames 1-10: move mouse 10px/frame
        if (1..=10).contains(&i) {
            let mx = 400.0 + i as f64 * 10.0;
            let my = 300.0 + i as f64 * 10.0;
            let event = emInputEvent::mouse_move(InputKey::MouseMiddle, mx, my);
            let mut state = emInputState::new();
            state.set_mouse(mx, my);
            state.press(InputKey::MouseMiddle);
            vif.filter(&event, &state, &mut view, &mut tree);
        }

        // Frame 10: release middle button (after move event)
        if i == 10 {
            let event = emInputEvent::release(InputKey::MouseMiddle).with_mouse(500.0, 400.0);
            let mut state = emInputState::new();
            state.set_mouse(500.0, 400.0);
            vif.filter(&event, &state, &mut view, &mut tree);
        }

        vif.animate_grip(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_middle_fling", &actual, &golden, 1e-6)
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
        let mut event = emInputEvent::press(InputKey::ArrowRight);
        event.alt = true;
        let mut state = emInputState::new();
        state.press(InputKey::Alt);
        state.press(InputKey::ArrowRight);
        vif.filter(&event, &state, &mut view, &mut tree);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.animate(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_keyboard_scroll", &actual, &golden, 1e-6)
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
        let mut event = emInputEvent::press(InputKey::PageUp);
        event.alt = true;
        let mut state = emInputState::new();
        state.press(InputKey::Alt);
        state.press(InputKey::PageUp);
        vif.filter(&event, &state, &mut view, &mut tree);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        vif.animate(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_keyboard_zoom", &actual, &golden, 1e-4)
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
        let mut event = emInputEvent::press(InputKey::ArrowRight);
        event.alt = true;
        let mut state = emInputState::new();
        state.press(InputKey::Alt);
        state.press(InputKey::ArrowRight);
        vif.filter(&event, &state, &mut view, &mut tree);
    }

    let mut actual = Vec::with_capacity(60);
    for i in 0..60 {
        // Frame 30: release Right (Alt still held)
        if i == 30 {
            let mut event = emInputEvent::release(InputKey::ArrowRight);
            event.alt = true;
            let mut state = emInputState::new();
            state.press(InputKey::Alt);
            vif.filter(&event, &state, &mut view, &mut tree);
        }

        vif.animate(&mut view, &mut tree, dt_for_frame(i));
        actual.push(read_view_state(&view, &tree));
    }

    compare_trajectory("filter_keyboard_release", &actual, &golden, 1e-4)
        .unwrap_or_else(|e| panic!("filter_keyboard_release: {e}"));
}
