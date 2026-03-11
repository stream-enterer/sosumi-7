use zuicchini::widget::{Look, RadioButton, RadioGroup};

/// PORT-0200: emRadioButton::Mechanism::AddAll(emPanel*)
/// Adds multiple button slots to the group at once.
#[test]
fn add_all_increases_count() {
    let group = RadioGroup::new();
    assert_eq!(group.borrow().count(), 0);
    group.borrow_mut().add_all(5);
    assert_eq!(group.borrow().count(), 5);
}

#[test]
fn add_all_zero_is_noop() {
    let group = RadioGroup::new();
    group.borrow_mut().add_all(0);
    assert_eq!(group.borrow().count(), 0);
}

#[test]
fn add_all_preserves_existing_buttons() {
    let look = Look::new();
    let group = RadioGroup::new();
    let _r0 = RadioButton::new("A", look.clone(), group.clone(), 0);
    assert_eq!(group.borrow().count(), 1);

    // AddAll adds 3 more slots
    group.borrow_mut().add_all(3);
    assert_eq!(group.borrow().count(), 4);
}

#[test]
fn add_all_preserves_selection() {
    let group = RadioGroup::new();
    group.borrow_mut().add_all(3);
    group.borrow_mut().select(1);
    assert_eq!(group.borrow().selected(), Some(1));

    // Adding more doesn't change selection
    group.borrow_mut().add_all(2);
    assert_eq!(group.borrow().selected(), Some(1));
    assert_eq!(group.borrow().count(), 5);
}
