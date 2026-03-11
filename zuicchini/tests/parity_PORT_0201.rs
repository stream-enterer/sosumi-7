use zuicchini::widget::{Look, RadioButton, RadioGroup};

/// PORT-0201: emRadioButton::Mechanism::GetButton(int)
/// Returns the button index at the given position, or None if out of range.
#[test]
fn get_button_valid_index() {
    let group = RadioGroup::new();
    group.borrow_mut().add_all(3);

    assert_eq!(group.borrow().get_button(0), Some(0));
    assert_eq!(group.borrow().get_button(1), Some(1));
    assert_eq!(group.borrow().get_button(2), Some(2));
}

#[test]
fn get_button_out_of_range() {
    let group = RadioGroup::new();
    group.borrow_mut().add_all(2);

    assert_eq!(group.borrow().get_button(2), None);
    assert_eq!(group.borrow().get_button(100), None);
}

#[test]
fn get_button_empty_group() {
    let group = RadioGroup::new();
    assert_eq!(group.borrow().get_button(0), None);
}

#[test]
fn get_button_with_real_buttons() {
    let look = Look::new();
    let group = RadioGroup::new();
    let _r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
    let _r1 = RadioButton::new("B", look.clone(), group.clone(), 1);

    assert_eq!(group.borrow().get_button(0), Some(0));
    assert_eq!(group.borrow().get_button(1), Some(1));
    assert_eq!(group.borrow().get_button(2), None);
}
