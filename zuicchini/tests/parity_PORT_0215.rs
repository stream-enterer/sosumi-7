use zuicchini::widget::{DefaultItemPanel, ItemPanelInterface};

#[test]
fn default_item_panel_new() {
    let panel = DefaultItemPanel::new(3, "item 3".to_string(), true);
    assert_eq!(panel.item_index(), 3);
    assert_eq!(panel.text(), "item 3");
    assert!(panel.is_selected());
}

#[test]
fn default_item_panel_text_changed() {
    let mut panel = DefaultItemPanel::new(0, "old".to_string(), false);
    panel.item_text_changed("new");
    assert_eq!(panel.text(), "new");
}

#[test]
fn default_item_panel_selection_changed() {
    let mut panel = DefaultItemPanel::new(0, "test".to_string(), false);
    assert!(!panel.is_selected());
    panel.item_selection_changed(true);
    assert!(panel.is_selected());
    panel.item_selection_changed(false);
    assert!(!panel.is_selected());
}

#[test]
fn default_item_panel_set_index() {
    let mut panel = DefaultItemPanel::new(5, "test".to_string(), false);
    panel.set_item_index(10);
    assert_eq!(panel.item_index(), 10);
}
