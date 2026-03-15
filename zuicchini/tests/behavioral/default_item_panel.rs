use zuicchini::widget::{DefaultItemPanel, ItemPanelInterface, ListBox, Look};

struct CustomPanel {
    index: usize,
    label: String,
    selected: bool,
}

impl ItemPanelInterface for CustomPanel {
    fn item_text_changed(&mut self, text: &str) {
        self.label = format!("Custom: {}", text);
    }

    fn item_data_changed(&mut self) {}

    fn item_selection_changed(&mut self, selected: bool) {
        self.selected = selected;
    }

    fn item_index(&self) -> usize {
        self.index
    }

    fn set_item_index(&mut self, index: usize) {
        self.index = index;
    }

    fn text(&self) -> &str {
        &self.label
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

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

#[test]
fn custom_factory_creates_custom_panels() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.set_item_panel_factory(|index, text, selected| {
        Box::new(CustomPanel {
            index,
            label: format!("Custom: {}", text),
            selected,
        })
    });
    lb.add_item("a".to_string(), "Alpha".to_string());
    lb.auto_expand_items();
    assert!(lb.get_item_panel(0).is_some());
    assert_eq!(lb.get_item_panel(0).unwrap().text(), "Custom: Alpha");
}

#[test]
fn default_factory_creates_default_panels() {
    let look = Look::new();
    let mut lb = ListBox::new(look);
    lb.add_item("a".to_string(), "Alpha".to_string());
    lb.auto_expand_items();
    let panel = lb.get_item_panel(0).unwrap();
    assert_eq!(panel.text(), "Alpha");
}
