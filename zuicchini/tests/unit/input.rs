use zuicchini::emCore::emInput::InputKey;
use zuicchini::emCore::emInputHotkey::Hotkey;
use zuicchini::emCore::emInputState::emInputState;

#[test]
fn hotkey_parse_ctrl_c() {
    let hk = Hotkey::TryParse("Ctrl+C").unwrap();
    assert!(hk.ctrl);
    assert!(!hk.alt);
    assert!(!hk.shift);
    assert_eq!(hk.key, InputKey::Key('c'));
}

#[test]
fn hotkey_parse_ctrl_shift_s() {
    let hk = Hotkey::TryParse("Ctrl+Shift+S").unwrap();
    assert!(hk.ctrl);
    assert!(hk.shift);
    assert!(!hk.alt);
    assert_eq!(hk.key, InputKey::Key('s'));
}

#[test]
fn hotkey_parse_alt_f4() {
    let hk = Hotkey::TryParse("Alt+F4").unwrap();
    assert!(hk.alt);
    assert_eq!(hk.key, InputKey::F4);
}

#[test]
fn hotkey_parse_single_key() {
    let hk = Hotkey::TryParse("Escape").unwrap();
    assert!(!hk.ctrl);
    assert!(!hk.alt);
    assert!(!hk.shift);
    assert_eq!(hk.key, InputKey::Escape);
}

#[test]
fn hotkey_parse_invalid() {
    assert!(Hotkey::TryParse("").is_none());
    assert!(Hotkey::TryParse("NotAKey+X").is_none());
}

#[test]
fn hotkey_matches() {
    let hk = Hotkey::TryParse("Ctrl+C").unwrap();

    let mut state = emInputState::new();
    state.press(InputKey::Ctrl);
    assert!(hk.Match(InputKey::Key('c'), &state));

    // Without Ctrl held, should not match
    let state2 = emInputState::new();
    assert!(!hk.Match(InputKey::Key('c'), &state2));
}

#[test]
fn input_state_modifiers() {
    let mut state = emInputState::new();
    assert!(!state.GetShift());
    assert!(!state.GetCtrl());
    assert!(!state.GetAlt());
    assert!(!state.GetMeta());

    state.press(InputKey::Shift);
    assert!(state.GetShift());

    state.press(InputKey::Ctrl);
    assert!(state.GetCtrl());

    state.release(InputKey::Shift);
    assert!(!state.GetShift());
    assert!(state.GetCtrl());
}

#[test]
fn input_state_mouse() {
    let mut state = emInputState::new();
    assert_eq!(state.mouse_x, 0.0);
    state.set_mouse(100.0, 200.0);
    assert_eq!(state.mouse_x, 100.0);
    assert_eq!(state.mouse_y, 200.0);
}

#[test]
fn input_state_key_tracking() {
    let mut state = emInputState::new();
    state.press(InputKey::Key('a'));
    assert!(state.Get(InputKey::Key('a')));
    assert!(!state.Get(InputKey::Key('b')));

    state.release(InputKey::Key('a'));
    assert!(!state.Get(InputKey::Key('a')));
}

#[test]
fn input_state_touches() {
    let mut state = emInputState::new();
    state.SetTouch(1, 100.0, 200.0);
    state.SetTouch(2, 300.0, 400.0);
    assert_eq!(state.GetTouchCount().len(), 2);

    state.RemoveTouch(1);
    assert_eq!(state.GetTouchCount().len(), 1);
    assert_eq!(state.GetTouchCount()[0].0, 2);
}

#[test]
fn hotkey_parse_meta() {
    let hk = Hotkey::TryParse("Meta+A").unwrap();
    assert!(hk.meta);
    assert_eq!(hk.key, InputKey::Key('a'));

    let hk2 = Hotkey::TryParse("Cmd+A").unwrap();
    assert!(hk2.meta);
}

#[test]
fn hotkey_parse_case_insensitive_ctrl() {
    for s in &["CTRL+A", "control+A", "Control+a"] {
        let hk = Hotkey::TryParse(s).unwrap();
        assert!(hk.ctrl, "failed for {s}");
        assert!(!hk.alt);
        assert!(!hk.shift);
        assert!(!hk.meta);
        assert_eq!(hk.key, InputKey::Key('a'), "failed for {s}");
    }
}

#[test]
fn hotkey_parse_all_meta_aliases() {
    for s in &["Win+X", "Super+X", "cmd+X"] {
        let hk = Hotkey::TryParse(s).unwrap();
        assert!(hk.meta, "failed for {s}");
        assert!(!hk.ctrl);
        assert!(!hk.alt);
        assert!(!hk.shift);
        assert_eq!(hk.key, InputKey::Key('x'), "failed for {s}");
    }
}

#[test]
fn hotkey_display_roundtrip() {
    for s in &["Ctrl+Shift+F1", "Alt+F4", "Escape"] {
        let hk1 = Hotkey::TryParse(s).unwrap();
        let display = hk1.to_string();
        let hk2 = Hotkey::TryParse(&display).unwrap();
        assert_eq!(hk1.ctrl, hk2.ctrl, "ctrl mismatch for {s}");
        assert_eq!(hk1.alt, hk2.alt, "alt mismatch for {s}");
        assert_eq!(hk1.shift, hk2.shift, "shift mismatch for {s}");
        assert_eq!(hk1.meta, hk2.meta, "meta mismatch for {s}");
        assert_eq!(hk1.key, hk2.key, "key mismatch for {s}");
    }
}

#[test]
fn hotkey_modifier_mutation() {
    let mut hk = Hotkey::new(InputKey::Key('c'));
    assert!(!hk.ctrl);
    hk.AddModifier(InputKey::Ctrl);
    assert!(hk.ctrl);
    assert_eq!(hk.key, InputKey::Key('c'));

    hk.ClearModifiers();
    assert!(!hk.ctrl);
    assert!(!hk.alt);
    assert!(!hk.shift);
    assert!(!hk.meta);
    assert_eq!(hk.key, InputKey::Key('c'));
}

#[test]
fn hotkey_match_rejects_extra_modifiers() {
    let hk = Hotkey::TryParse("C").unwrap();
    assert!(!hk.shift);

    let mut state = emInputState::new();
    state.press(InputKey::Shift);
    assert!(state.GetShift());

    assert!(!hk.Match(InputKey::Key('c'), &state));
}

#[test]
fn hotkey_parse_special_keys() {
    let cases: &[(&str, InputKey)] = &[
        ("esc", InputKey::Escape),
        ("return", InputKey::Enter),
        ("del", InputKey::Delete),
        ("pgup", InputKey::PageUp),
        ("arrowup", InputKey::ArrowUp),
    ];
    for (s, expected) in cases {
        let hk = Hotkey::TryParse(s).unwrap();
        assert_eq!(hk.key, *expected, "failed for {s}");
    }
}

#[test]
fn hotkey_set_key() {
    let mut hk = Hotkey::new(InputKey::Key('a'));
    assert_eq!(hk.key, InputKey::Key('a'));
    hk.SetKey(InputKey::F12);
    assert_eq!(hk.key, InputKey::F12);
}
