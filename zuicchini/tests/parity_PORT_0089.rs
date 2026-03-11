use std::rc::Rc;
use zuicchini::foundation::Color;
use zuicchini::widget::Look;

/// PORT-0089: emLook::Apply — apply look to target, optionally recursive.
#[test]
fn apply_replaces_target_look() {
    let mut target = Look::new(); // default look
    let mut custom = Look::default();
    custom.bg_color = Color::RED;

    custom.apply(&mut target, false);
    assert_eq!(target.bg_color, Color::RED);
}

#[test]
fn apply_preserves_all_fields() {
    let mut custom = Look::default();
    custom.bg_color = Color::rgba(0x11, 0x22, 0x33, 0xFF);
    custom.fg_color = Color::rgba(0xAA, 0xBB, 0xCC, 0xFF);
    custom.button_bg_color = Color::rgba(0x44, 0x55, 0x66, 0xFF);
    custom.input_hl_color = Color::rgba(0x77, 0x88, 0x99, 0xFF);

    let mut target = Look::new();
    custom.apply(&mut target, true);
    assert_eq!(target.bg_color, custom.bg_color);
    assert_eq!(target.fg_color, custom.fg_color);
    assert_eq!(target.button_bg_color, custom.button_bg_color);
    assert_eq!(target.input_hl_color, custom.input_hl_color);
}

#[test]
fn apply_all_updates_multiple_targets() {
    let mut custom = Look::default();
    custom.bg_color = Color::GREEN;

    let mut t1 = Look::new();
    let mut t2 = Look::new();
    custom.apply_all(&mut [&mut t1, &mut t2]);
    assert_eq!(t1.bg_color, Color::GREEN);
    assert_eq!(t2.bg_color, Color::GREEN);
}

#[test]
fn look_is_cloneable() {
    let look = Look::default();
    let cloned = look.clone();
    assert_eq!(look, cloned);
}

#[test]
fn apply_creates_independent_rc() {
    let mut custom = Look::default();
    custom.bg_color = Color::BLUE;

    let mut target = Look::new();
    let original_ptr = Rc::as_ptr(&target);
    custom.apply(&mut target, false);
    // Should be a new Rc, not the same allocation
    assert_ne!(Rc::as_ptr(&target), original_ptr);
    assert_eq!(target.bg_color, Color::BLUE);
}
