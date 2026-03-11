use zuicchini::widget::{ListBox, Look};

#[test]
fn get_item_panel_none_before_expand() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "Item A".to_string());
    assert!(lb.get_item_panel(0).is_none());
}

#[test]
fn get_item_panel_some_after_expand() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "Item A".to_string());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).is_some());
    assert_eq!(lb.get_item_panel(0).unwrap().text(), "Item A");
}

#[test]
fn get_item_panel_out_of_range() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "Item A".to_string());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(99).is_none());
}
