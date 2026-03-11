use zuicchini::widget::{DefaultItemPanel, ItemPanelInterface};

#[test]
fn item_panel_interface_trait_is_object_safe() {
    let panel: Box<dyn ItemPanelInterface> =
        Box::new(DefaultItemPanel::new(0, "test".to_string(), false));
    assert_eq!(panel.item_index(), 0);
    assert_eq!(panel.text(), "test");
}

#[test]
fn item_panel_interface_notifications() {
    let mut panel = DefaultItemPanel::new(0, "original".to_string(), false);
    panel.item_text_changed("updated");
    assert_eq!(panel.text(), "updated");
    panel.item_selection_changed(true);
    assert!(panel.is_selected());
}
