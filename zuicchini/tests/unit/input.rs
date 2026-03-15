use zuicchini::input::{Hotkey, InputKey, InputState};

#[test]
fn hotkey_parse_ctrl_c() {
    let hk = Hotkey::parse("Ctrl+C").unwrap();
    assert!(hk.ctrl);
    assert!(!hk.alt);
    assert!(!hk.shift);
    assert_eq!(hk.key, InputKey::Key('c'));
}

#[test]
fn hotkey_parse_ctrl_shift_s() {
    let hk = Hotkey::parse("Ctrl+Shift+S").unwrap();
    assert!(hk.ctrl);
    assert!(hk.shift);
    assert!(!hk.alt);
    assert_eq!(hk.key, InputKey::Key('s'));
}

#[test]
fn hotkey_parse_alt_f4() {
    let hk = Hotkey::parse("Alt+F4").unwrap();
    assert!(hk.alt);
    assert_eq!(hk.key, InputKey::F4);
}

#[test]
fn hotkey_parse_single_key() {
    let hk = Hotkey::parse("Escape").unwrap();
    assert!(!hk.ctrl);
    assert!(!hk.alt);
    assert!(!hk.shift);
    assert_eq!(hk.key, InputKey::Escape);
}

#[test]
fn hotkey_parse_invalid() {
    assert!(Hotkey::parse("").is_none());
    assert!(Hotkey::parse("NotAKey+X").is_none());
}

#[test]
fn hotkey_matches() {
    let hk = Hotkey::parse("Ctrl+C").unwrap();

    let mut state = InputState::new();
    state.press(InputKey::Ctrl);
    assert!(hk.matches(InputKey::Key('c'), &state));

    // Without Ctrl held, should not match
    let state2 = InputState::new();
    assert!(!hk.matches(InputKey::Key('c'), &state2));
}

#[test]
fn input_state_modifiers() {
    let mut state = InputState::new();
    assert!(!state.shift());
    assert!(!state.ctrl());
    assert!(!state.alt());
    assert!(!state.meta());

    state.press(InputKey::Shift);
    assert!(state.shift());

    state.press(InputKey::Ctrl);
    assert!(state.ctrl());

    state.release(InputKey::Shift);
    assert!(!state.shift());
    assert!(state.ctrl());
}

#[test]
fn input_state_mouse() {
    let mut state = InputState::new();
    assert_eq!(state.mouse_x, 0.0);
    state.set_mouse(100.0, 200.0);
    assert_eq!(state.mouse_x, 100.0);
    assert_eq!(state.mouse_y, 200.0);
}

#[test]
fn input_state_key_tracking() {
    let mut state = InputState::new();
    state.press(InputKey::Key('a'));
    assert!(state.is_pressed(InputKey::Key('a')));
    assert!(!state.is_pressed(InputKey::Key('b')));

    state.release(InputKey::Key('a'));
    assert!(!state.is_pressed(InputKey::Key('a')));
}

#[test]
fn input_state_touches() {
    let mut state = InputState::new();
    state.set_touch(1, 100.0, 200.0);
    state.set_touch(2, 300.0, 400.0);
    assert_eq!(state.touches().len(), 2);

    state.remove_touch(1);
    assert_eq!(state.touches().len(), 1);
    assert_eq!(state.touches()[0].0, 2);
}

#[test]
fn hotkey_parse_meta() {
    let hk = Hotkey::parse("Meta+A").unwrap();
    assert!(hk.meta);
    assert_eq!(hk.key, InputKey::Key('a'));

    let hk2 = Hotkey::parse("Cmd+A").unwrap();
    assert!(hk2.meta);
}
