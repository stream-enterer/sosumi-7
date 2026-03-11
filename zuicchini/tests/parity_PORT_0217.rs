use zuicchini::widget::{ListBox, Look};

#[test]
fn get_item_panel_interface_after_expand() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("x".to_string(), "X".to_string());
    lb.auto_expand_items();
    let iface = lb.get_item_panel_interface(0).unwrap();
    assert_eq!(iface.item_index(), 0);
}

#[test]
fn get_item_panel_interface_text_sync() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("x".to_string(), "Original".to_string());
    lb.auto_expand_items();
    lb.set_item_text(0, "Changed".to_string());
    let iface = lb.get_item_panel_interface(0).unwrap();
    assert_eq!(iface.text(), "Changed");
}
