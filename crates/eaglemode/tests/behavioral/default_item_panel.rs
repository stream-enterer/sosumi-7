use super::support::TestHarness;
use emcore::emListBox::{emListBox, DefaultItemPanel, ItemPanelInterface};
use emcore::emLook::emLook;

struct CustomPanel {
    index: usize,
    label: String,
    checked: bool,
}

impl ItemPanelInterface for CustomPanel {
    fn item_text_changed(&mut self, text: &str) {
        self.label = format!("Custom: {}", text);
    }

    fn item_data_changed(&mut self) {}

    fn item_selection_changed(&mut self, checked: bool) {
        self.checked = checked;
    }

    fn item_index(&self) -> usize {
        self.index
    }

    fn set_item_index(&mut self, index: usize) {
        self.index = index;
    }

    fn GetText(&self) -> &str {
        &self.label
    }

    fn IsSelected(&self) -> bool {
        self.checked
    }
}

#[test]
fn item_panel_interface_trait_is_object_safe() {
    let panel: Box<dyn ItemPanelInterface> =
        Box::new(DefaultItemPanel::new(0, "test".to_string(), false));
    assert_eq!(panel.item_index(), 0);
    assert_eq!(panel.GetText(), "test");
}

#[test]
fn item_panel_interface_notifications() {
    let mut panel = DefaultItemPanel::new(0, "original".to_string(), false);
    panel.item_text_changed("updated");
    assert_eq!(panel.GetText(), "updated");
    panel.item_selection_changed(true);
    assert!(panel.IsSelected());
}

#[test]
fn default_item_panel_new() {
    let panel = DefaultItemPanel::new(3, "item 3".to_string(), true);
    assert_eq!(panel.item_index(), 3);
    assert_eq!(panel.GetText(), "item 3");
    assert!(panel.IsSelected());
}

#[test]
fn default_item_panel_text_changed() {
    let mut panel = DefaultItemPanel::new(0, "old".to_string(), false);
    panel.item_text_changed("new");
    assert_eq!(panel.GetText(), "new");
}

#[test]
fn default_item_panel_selection_changed() {
    let mut panel = DefaultItemPanel::new(0, "test".to_string(), false);
    assert!(!panel.IsSelected());
    panel.item_selection_changed(true);
    assert!(panel.IsSelected());
    panel.item_selection_changed(false);
    assert!(!panel.IsSelected());
}

#[test]
fn default_item_panel_set_index() {
    let mut panel = DefaultItemPanel::new(5, "test".to_string(), false);
    panel.set_item_index(10);
    assert_eq!(panel.item_index(), 10);
}

#[test]
fn get_item_panel_none_before_expand() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.AddItem("a".to_string(), "Item A".to_string());
    assert!(lb.GetItemPanel(0).is_none());
}

#[test]
fn get_item_panel_some_after_expand() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.AddItem("a".to_string(), "Item A".to_string());
    lb.auto_expand_items();
    assert!(lb.GetItemPanel(0).is_some());
    assert_eq!(lb.GetItemPanel(0).unwrap().GetText(), "Item A");
}

#[test]
fn get_item_panel_out_of_range() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.AddItem("a".to_string(), "Item A".to_string());
    lb.auto_expand_items();
    assert!(lb.GetItemPanel(99).is_none());
}

#[test]
fn get_item_panel_interface_after_expand() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.AddItem("x".to_string(), "X".to_string());
    lb.auto_expand_items();
    let iface = lb.GetItemPanelInterface(0).unwrap();
    assert_eq!(iface.item_index(), 0);
}

#[test]
fn get_item_panel_interface_text_sync() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.AddItem("x".to_string(), "Original".to_string());
    lb.auto_expand_items();
    lb.SetItemText(0, "Changed".to_string());
    let iface = lb.GetItemPanelInterface(0).unwrap();
    assert_eq!(iface.GetText(), "Changed");
}

#[test]
fn custom_factory_creates_custom_panels() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.set_item_panel_factory(|index, text, checked| {
        Box::new(CustomPanel {
            index,
            label: format!("Custom: {}", text),
            checked,
        })
    });
    lb.AddItem("a".to_string(), "Alpha".to_string());
    lb.auto_expand_items();
    assert!(lb.GetItemPanel(0).is_some());
    assert_eq!(lb.GetItemPanel(0).unwrap().GetText(), "Custom: Alpha");
}

#[test]
fn default_factory_creates_default_panels() {
    let mut h = TestHarness::new();
    let look = emLook::new();
    let mut lb = emListBox::new(&mut h.sched_ctx(), look);
    lb.AddItem("a".to_string(), "Alpha".to_string());
    lb.auto_expand_items();
    let panel = lb.GetItemPanel(0).unwrap();
    assert_eq!(panel.GetText(), "Alpha");
}
