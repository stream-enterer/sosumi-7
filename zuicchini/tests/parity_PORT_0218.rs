use zuicchini::widget::{ItemPanelInterface, ListBox, Look};

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
