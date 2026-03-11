use zuicchini::widget::{ListBox, Look};

#[test]
fn auto_expand_creates_all_panels() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.add_item("b".to_string(), "B".to_string());
    lb.add_item("c".to_string(), "C".to_string());

    assert!(lb.get_item_panel(0).is_none());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).is_some());
    assert!(lb.get_item_panel(1).is_some());
    assert!(lb.get_item_panel(2).is_some());
}

#[test]
fn auto_shrink_destroys_all_panels() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).is_some());

    lb.auto_shrink_items();
    assert!(lb.get_item_panel(0).is_none());
}

#[test]
fn auto_expand_idempotent() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.auto_expand_items();
    lb.auto_expand_items(); // second call should not create duplicates
    assert!(lb.get_item_panel(0).is_some());
}

#[test]
fn auto_expand_preserves_selection() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "A".to_string());
    lb.select(0, true);
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).unwrap().is_selected());
}
