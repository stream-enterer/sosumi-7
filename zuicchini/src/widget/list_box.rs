use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use crate::foundation::Rect;
use crate::input::{InputEvent, InputKey, InputVariant};
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::look::Look;

const ROW_HEIGHT: f64 = 17.0;

/// Timeout in milliseconds for keywalk type-to-search accumulation.
const KEYWALK_TIMEOUT_MS: u128 = 1000;

type SelectionCb = Box<dyn FnMut(&[usize])>;
type TriggerCb = Box<dyn FnMut(usize)>;
type ItemPanelFactory = Box<dyn Fn(usize, String, bool) -> Box<dyn ItemPanelInterface>>;

/// Selection mode for list box items.
///
/// Matches C++ emListBox::SelectionType.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    /// Items are displayed but cannot be selected by the user.
    ReadOnly,
    /// Exactly one item can be selected at a time.
    Single,
    /// Multiple items can be selected using Shift/Ctrl modifiers.
    Multi,
    /// Each click toggles the item's selection state.
    Toggle,
}

/// Interface for custom item panel implementations.
/// Port of C++ emListBox::ItemPanelInterface.
///
/// Implementors receive notifications when item properties change.
pub trait ItemPanelInterface {
    /// Called when the item's display text changes.
    fn item_text_changed(&mut self, text: &str);

    /// Called when the item's data changes.
    fn item_data_changed(&mut self);

    /// Called when the item's selection state changes.
    fn item_selection_changed(&mut self, selected: bool);

    /// Get the item's index within the list box.
    fn item_index(&self) -> usize;

    /// Set the item's index (called after reindexing).
    fn set_item_index(&mut self, index: usize);

    /// Get the display text.
    fn text(&self) -> &str;

    /// Whether selected.
    fn is_selected(&self) -> bool;
}

/// Default item panel that displays item text with selection highlight.
/// Port of C++ emListBox::DefaultItemPanel.
pub struct DefaultItemPanel {
    index: usize,
    text: String,
    selected: bool,
}

impl DefaultItemPanel {
    pub fn new(index: usize, text: String, selected: bool) -> Self {
        Self {
            index,
            text,
            selected,
        }
    }

    /// Get the display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether selected.
    pub fn is_selected(&self) -> bool {
        self.selected
    }
}

impl ItemPanelInterface for DefaultItemPanel {
    fn item_text_changed(&mut self, text: &str) {
        self.text = text.to_string();
    }

    fn item_data_changed(&mut self) {
        // Default panel doesn't use data
    }

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
        &self.text
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

/// Internal item representation for the list box.
struct Item {
    /// Unique identifier for lookup.
    name: String,
    /// Display text.
    text: String,
    /// Arbitrary user data.
    data: Option<Box<dyn Any>>,
    /// Whether this item is currently selected.
    selected: bool,
    /// Item panel interface, created during auto-expand.
    /// Port of C++ Item::Interface.
    interface: Option<Box<dyn ItemPanelInterface>>,
}

/// Selectable item list widget.
///
/// Implements the full emListBox API: individual item manipulation, multi-mode
/// selection, trigger/double-click, and type-to-search (keywalk).
pub struct ListBox {
    border: Border,
    look: Rc<Look>,
    items: Vec<Item>,
    /// O(1) lookup from item name to index.
    name_index: HashMap<String, usize>,
    /// Sorted list of currently selected item indices.
    selected_indices: Vec<usize>,
    focus_index: usize,
    scroll_y: f64,
    selection_mode: SelectionMode,
    /// Index of the last item that received input (for shift-range selection).
    prev_input_index: Option<usize>,
    /// Index of the last triggered item.
    triggered_index: Option<usize>,
    /// Accumulated characters for type-to-search.
    keywalk_chars: String,
    /// Timestamp of last keywalk input.
    keywalk_time: Option<std::time::Instant>,

    pub on_selection: Option<SelectionCb>,
    pub on_trigger: Option<TriggerCb>,

    /// Custom item panel factory. Port of C++ virtual CreateItemPanel.
    item_panel_factory: Option<ItemPanelFactory>,
    /// Whether item panels are currently expanded.
    expanded: bool,
}

impl ListBox {
    pub fn new(look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Instrument)
                .with_inner(InnerBorderType::InputField),
            look,
            items: Vec::new(),
            name_index: HashMap::new(),
            selected_indices: Vec::new(),
            focus_index: 0,
            scroll_y: 0.0,
            selection_mode: SelectionMode::Single,
            prev_input_index: None,
            triggered_index: None,
            keywalk_chars: String::new(),
            keywalk_time: None,
            on_selection: None,
            on_trigger: None,
            item_panel_factory: None,
            expanded: false,
        }
    }

    pub fn set_caption(&mut self, caption: &str) {
        self.border.caption = caption.to_string();
    }

    // ── Selection mode ──────────────────────────────────────────────

    pub fn selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }

    /// Set the selection type. Updates the inner border type to match:
    /// ReadOnly uses OutputField, all others use InputField.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        if self.selection_mode == mode {
            return;
        }
        let old = self.selection_mode;
        self.selection_mode = mode;
        // Swap inner border type to match C++ SetSelectionType behavior.
        if old == SelectionMode::ReadOnly && self.border.inner == InnerBorderType::OutputField {
            self.border.inner = InnerBorderType::InputField;
        } else if mode == SelectionMode::ReadOnly
            && self.border.inner == InnerBorderType::InputField
        {
            self.border.inner = InnerBorderType::OutputField;
        }
    }

    // ── Bulk item replacement (backward compat) ─────────────────────

    /// Replace all items with plain strings. Names are auto-generated as the
    /// string values themselves. Selection and scroll position are reset.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.clear_items();
        for text in items {
            self.add_item(text.clone(), text);
        }
        self.focus_index = 0;
        self.scroll_y = 0.0;
    }

    // ── Item manipulation APIs ──────────────────────────────────────

    /// Number of items in the list.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Add an item at the end. Panics if `name` is not unique.
    pub fn add_item(&mut self, name: String, text: String) {
        let idx = self.items.len();
        self.insert_item(idx, name, text);
    }

    /// Insert an item at `index`. Index is clamped to `[0, item_count()]`.
    /// Panics if `name` is not unique.
    pub fn insert_item(&mut self, index: usize, name: String, text: String) {
        let index = index.min(self.items.len());

        assert!(
            !self.name_index.contains_key(&name),
            "ListBox: item name '{}' is not unique",
            name
        );

        let item = Item {
            name: name.clone(),
            text,
            data: None,
            selected: false,
            interface: None,
        };

        self.items.insert(index, item);

        // Rebuild name_index for shifted items.
        self.rebuild_name_index_from(index);

        // Adjust selected_indices: increment any index >= insertion point.
        let mut selection_changed = false;
        for si in &mut self.selected_indices {
            if *si >= index {
                *si += 1;
                selection_changed = true;
            }
        }

        self.keywalk_chars.clear();

        if selection_changed {
            self.fire_selection();
        }
    }

    /// Remove an item at `index`. No-op if out of range.
    pub fn remove_item(&mut self, index: usize) {
        if index >= self.items.len() {
            return;
        }

        // Clear prev_input/triggered if they pointed to this item.
        if self.prev_input_index == Some(index) {
            self.prev_input_index = None;
        }
        if self.triggered_index == Some(index) {
            self.triggered_index = None;
        }

        let was_selected = self.items[index].selected;
        self.items.remove(index);

        // Rebuild name_index from the removal point.
        // First remove the old name (it was already removed from items).
        // We need to rebuild from scratch since we removed.
        self.rebuild_name_index_from(0);

        // Adjust selected_indices.
        let mut selection_changed = was_selected;
        self.selected_indices.retain(|&si| si != index);
        for si in &mut self.selected_indices {
            if *si > index {
                *si -= 1;
                selection_changed = true;
            }
        }

        // Adjust prev_input_index and triggered_index references.
        if let Some(ref mut pi) = self.prev_input_index {
            if *pi > index {
                *pi -= 1;
            }
        }
        if let Some(ref mut ti) = self.triggered_index {
            if *ti > index {
                *ti -= 1;
            }
        }

        self.keywalk_chars.clear();

        if selection_changed {
            self.fire_selection();
        }
    }

    /// Move an item from `from` to `to`. No-op if `from` is out of range or
    /// equals `to`. `to` is clamped to valid range.
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.items.len() {
            return;
        }
        let to = to.min(self.items.len() - 1);
        if from == to {
            return;
        }

        let item = self.items.remove(from);
        self.items.insert(to, item);

        // Rebuild name_index for the affected range.
        let lo = from.min(to);
        self.rebuild_name_index_from(lo);

        // Rebuild selected_indices for the affected range.
        let old_selected: Vec<usize> = self.selected_indices.clone();
        self.selected_indices.clear();
        for (i, item) in self.items.iter().enumerate() {
            if item.selected {
                self.selected_indices.push(i);
            }
        }

        let selection_changed = self.selected_indices != old_selected;
        self.keywalk_chars.clear();

        // Adjust prev_input/triggered to follow the moved item.
        // The item that was at `from` is now at `to`.
        if let Some(ref mut pi) = self.prev_input_index {
            if *pi == from {
                *pi = to;
            } else if from < to && *pi > from && *pi <= to {
                *pi -= 1;
            } else if from > to && *pi >= to && *pi < from {
                *pi += 1;
            }
        }
        if let Some(ref mut ti) = self.triggered_index {
            if *ti == from {
                *ti = to;
            } else if from < to && *ti > from && *ti <= to {
                *ti -= 1;
            } else if from > to && *ti >= to && *ti < from {
                *ti += 1;
            }
        }

        if selection_changed {
            self.fire_selection();
        }
    }

    /// Sort items using a custom comparison function. Returns `true` if the
    /// order changed.
    pub fn sort_items<F>(&mut self, compare: F) -> bool
    where
        F: FnMut(&str, &str, &str, &str) -> std::cmp::Ordering,
    {
        self.sort_items_impl(compare)
    }

    fn sort_items_impl<F>(&mut self, mut compare: F) -> bool
    where
        F: FnMut(&str, &str, &str, &str) -> std::cmp::Ordering,
    {
        // Check if already sorted.
        let old_order: Vec<String> = self.items.iter().map(|it| it.name.clone()).collect();

        self.items
            .sort_by(|a, b| compare(&a.name, &a.text, &b.name, &b.text));

        let new_order: Vec<String> = self.items.iter().map(|it| it.name.clone()).collect();
        if old_order == new_order {
            return false;
        }

        // Rebuild name_index.
        self.rebuild_name_index_from(0);

        // Rebuild selected_indices from item flags.
        let old_selected = self.selected_indices.clone();
        self.selected_indices.clear();
        for (i, item) in self.items.iter().enumerate() {
            if item.selected {
                self.selected_indices.push(i);
            }
        }

        self.keywalk_chars.clear();

        if self.selected_indices != old_selected {
            self.fire_selection();
        }

        true
    }

    /// Remove all items.
    pub fn clear_items(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let had_selection = !self.selected_indices.is_empty();

        self.items.clear();
        self.name_index.clear();
        self.prev_input_index = None;
        self.triggered_index = None;
        self.selected_indices.clear();
        self.keywalk_chars.clear();

        if had_selection {
            self.fire_selection();
        }
    }

    // ── Item accessors ──────────────────────────────────────────────

    /// Get the name of the item at `index`, or `""` if out of range.
    pub fn get_item_name(&self, index: usize) -> &str {
        self.items.get(index).map_or("", |it| &it.name)
    }

    /// Get the display text of the item at `index`, or `""` if out of range.
    pub fn get_item_text(&self, index: usize) -> &str {
        self.items.get(index).map_or("", |it| &it.text)
    }

    /// Set the display text of the item at `index`. No-op if out of range or
    /// text unchanged.
    pub fn set_item_text(&mut self, index: usize, text: String) {
        if let Some(item) = self.items.get_mut(index) {
            if item.text != text {
                item.text = text.clone();
                if let Some(iface) = &mut item.interface {
                    iface.item_text_changed(&text);
                }
                self.keywalk_chars.clear();
            }
        }
    }

    /// Get the item data at `index`, or `None` if out of range or no data set.
    pub fn get_item_data(&self, index: usize) -> Option<&dyn Any> {
        self.items.get(index).and_then(|it| it.data.as_deref())
    }

    /// Set the item data at `index`. No-op if out of range.
    pub fn set_item_data(&mut self, index: usize, data: Option<Box<dyn Any>>) {
        if let Some(item) = self.items.get_mut(index) {
            item.data = data;
            if let Some(iface) = &mut item.interface {
                iface.item_data_changed();
            }
            // Note: does NOT clear keywalk_chars (data doesn't affect search).
        }
    }

    /// Find an item's index by name. Returns `None` if not found.
    pub fn get_item_index(&self, name: &str) -> Option<usize> {
        self.name_index.get(name).copied()
    }

    // ── Legacy accessors (backward compat) ──────────────────────────

    /// Get item display texts as string slices. This allocates a Vec.
    pub fn items(&self) -> Vec<&str> {
        self.items.iter().map(|it| it.text.as_str()).collect()
    }

    /// Get the sorted selected indices.
    pub fn selected(&self) -> &[usize] {
        &self.selected_indices
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    // ── Selection APIs ──────────────────────────────────────────────

    /// Index of the first selected item, or `None` if nothing is selected.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_indices.first().copied()
    }

    /// Select a single item solely (deselecting all others).
    pub fn set_selected_index(&mut self, index: usize) {
        self.select(index, true);
    }

    /// Check whether an item is selected. Returns `false` for out-of-range.
    pub fn is_selected(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|it| it.selected)
    }

    /// Select an item. If `solely` is true, deselects all others first.
    /// Out-of-range `index` with `solely` clears the selection.
    pub fn select(&mut self, index: usize, solely: bool) {
        if index < self.items.len() {
            if solely {
                // Deselect all others.
                let indices: Vec<usize> = self
                    .selected_indices
                    .iter()
                    .copied()
                    .filter(|&i| i != index)
                    .collect();
                for i in indices {
                    self.deselect(i);
                }
            }
            if !self.items[index].selected {
                self.items[index].selected = true;
                if let Some(iface) = &mut self.items[index].interface {
                    iface.item_selection_changed(true);
                }
                // Binary insert into sorted selected_indices.
                let pos = self
                    .selected_indices
                    .binary_search(&index)
                    .unwrap_or_else(|p| p);
                self.selected_indices.insert(pos, index);
                self.prev_input_index = None;
                self.fire_selection();
            }
        } else if solely {
            self.clear_selection();
        }
    }

    /// Deselect an item. No-op if out of range or not selected.
    pub fn deselect(&mut self, index: usize) {
        if index < self.items.len() && self.items[index].selected {
            self.items[index].selected = false;
            if let Some(iface) = &mut self.items[index].interface {
                iface.item_selection_changed(false);
            }
            if let Ok(pos) = self.selected_indices.binary_search(&index) {
                self.selected_indices.remove(pos);
            }
            self.prev_input_index = None;
            self.fire_selection();
        }
    }

    /// Toggle the selection state of an item.
    pub fn toggle_selection(&mut self, index: usize) {
        if self.is_selected(index) {
            self.deselect(index);
        } else {
            self.select(index, false);
        }
    }

    /// Select all items.
    pub fn select_all(&mut self) {
        for i in 0..self.items.len() {
            self.select(i, false);
        }
    }

    /// Deselect all items.
    pub fn clear_selection(&mut self) {
        while let Some(&idx) = self.selected_indices.first() {
            self.deselect(idx);
        }
    }

    /// Get the sorted list of selected indices.
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    /// Set the selected items by index.
    ///
    /// Replaces the current selection with exactly the given indices.
    /// Out-of-range indices are silently ignored. The resulting selection
    /// is always sorted. Matches C++ `emListBox::SetSelectedIndices`.
    pub fn set_selected_indices(&mut self, indices: &[usize]) {
        // Clear current selection state on items.
        for &idx in &self.selected_indices {
            if idx < self.items.len() {
                self.items[idx].selected = false;
                if let Some(iface) = &mut self.items[idx].interface {
                    iface.item_selection_changed(false);
                }
            }
        }
        // Build new sorted selection.
        let mut new_sel: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| i < self.items.len())
            .collect();
        new_sel.sort_unstable();
        new_sel.dedup();
        for &idx in &new_sel {
            self.items[idx].selected = true;
            if let Some(iface) = &mut self.items[idx].interface {
                iface.item_selection_changed(true);
            }
        }
        if self.selected_indices != new_sel {
            self.selected_indices = new_sel;
            self.fire_selection();
        }
    }

    // ── Trigger ─────────────────────────────────────────────────────

    /// Trigger an item (fires the on_trigger callback). No-op if out of range.
    pub fn trigger_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.triggered_index = Some(index);
            if let Some(cb) = &mut self.on_trigger {
                cb(index);
            }
        }
    }

    /// Index of the last triggered item, or `None`.
    pub fn triggered_index(&self) -> Option<usize> {
        self.triggered_index
    }

    // ── Item Panel Interface ─────────────────────────────────────────

    /// Get the item panel interface at the given index.
    /// Port of C++ emListBox::GetItemPanelInterface(int).
    pub fn get_item_panel_interface(&self, index: usize) -> Option<&dyn ItemPanelInterface> {
        self.items
            .get(index)
            .and_then(|item| item.interface.as_deref())
    }

    /// Get a mutable reference to the item panel interface at the given index.
    pub fn get_item_panel_interface_mut(
        &mut self,
        index: usize,
    ) -> Option<&mut dyn ItemPanelInterface> {
        match self.items.get_mut(index) {
            Some(item) => match &mut item.interface {
                Some(iface) => Some(iface.as_mut()),
                None => None,
            },
            None => None,
        }
    }

    /// Get the item panel at the given index (returns the interface).
    /// Port of C++ emListBox::GetItemPanel(int).
    pub fn get_item_panel(&self, index: usize) -> Option<&dyn ItemPanelInterface> {
        self.get_item_panel_interface(index)
    }

    /// Create an item panel for the item at the given index.
    /// Port of C++ emListBox::CreateItemPanel(name, itemIndex).
    ///
    /// Override point: the default creates a DefaultItemPanel.
    /// Custom ListBox implementations can override this by setting
    /// a factory function.
    pub fn create_item_panel(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            let panel = if let Some(factory) = &self.item_panel_factory {
                factory(index, item.text.clone(), item.selected)
            } else {
                Box::new(DefaultItemPanel::new(
                    index,
                    item.text.clone(),
                    item.selected,
                ))
            };
            item.interface = Some(panel);
        }
    }

    /// Set a custom factory for creating item panels.
    /// Port of C++ virtual CreateItemPanel override mechanism.
    pub fn set_item_panel_factory<F>(&mut self, factory: F)
    where
        F: Fn(usize, String, bool) -> Box<dyn ItemPanelInterface> + 'static,
    {
        self.item_panel_factory = Some(Box::new(factory));
    }

    /// Create item panels for all items. Called when the list box expands.
    /// Port of C++ emListBox::AutoExpand().
    pub fn auto_expand_items(&mut self) {
        self.expanded = true;
        for i in 0..self.items.len() {
            if self.items[i].interface.is_none() {
                self.create_item_panel(i);
            }
        }
    }

    /// Destroy all item panels. Called when the list box shrinks.
    /// Port of C++ emListBox::AutoShrink().
    pub fn auto_shrink_items(&mut self) {
        self.expanded = false;
        for item in &mut self.items {
            item.interface = None;
        }
    }

    // ── Paint ───────────────────────────────────────────────────────

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border
            .paint_border(painter, w, h, &self.look, false, true);

        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.content_rect_unobscured(w, h, &self.look);

        painter.push_state();
        painter.clip_rect(cx, cy, cw, ch);

        // C++ emListBox lays out items as child panels that fill the content
        // area.  Compute row height dynamically so items scale with the widget.
        let row_h = if self.items.is_empty() {
            ROW_HEIGHT
        } else {
            ch / self.items.len() as f64
        };

        for (i, item) in self.items.iter().enumerate() {
            let iy = cy + i as f64 * row_h - self.scroll_y;
            if iy + row_h < cy || iy > cy + ch {
                continue;
            }

            let item_w = cw;
            let item_h = row_h;
            let s = item_w.min(item_h);

            // C++ DefaultItemPanel::Paint: canvasColor starts as parent canvas
            // (InputField bg). After painting selection highlight, canvasColor
            // changes to hlColor for selected items.
            let mut item_canvas = self.look.input_bg_color;

            if item.selected {
                let rdx = s * 0.015;
                let rdy = s * 0.015;
                let r = s * 0.15;
                painter.paint_round_rect(
                    cx + rdx,
                    iy + rdy,
                    item_w - 2.0 * rdx,
                    item_h - 2.0 * rdy,
                    r,
                    self.look.input_hl_color,
                );
                item_canvas = self.look.input_hl_color;
            }

            let dx = s * 0.15;
            let dy = s * 0.03;
            let text_color = if item.selected {
                self.look.input_bg_color
            } else {
                self.look.input_fg_color
            };
            painter.paint_text_boxed(
                cx + dx,
                iy + dy,
                (item_w - 2.0 * dx).max(0.0),
                (item_h - 2.0 * dy).max(0.0),
                &item.text,
                item_h,
                text_color,
                item_canvas,
                crate::render::TextAlignment::Left,
                crate::render::VAlign::Top,
                crate::render::TextAlignment::Left,
                0.5,
                true,
                0.0,
            );
        }

        painter.pop_state();
    }

    // ── Input ───────────────────────────────────────────────────────

    pub fn input(&mut self, event: &InputEvent) -> bool {
        if self.items.is_empty() {
            return false;
        }

        match event.key {
            InputKey::ArrowDown if event.variant == InputVariant::Press => {
                if self.focus_index + 1 < self.items.len() {
                    self.focus_index += 1;
                    if self.selection_mode == SelectionMode::Single {
                        self.select(self.focus_index, true);
                    }
                }
                true
            }
            InputKey::ArrowUp if event.variant == InputVariant::Press => {
                if self.focus_index > 0 {
                    self.focus_index -= 1;
                    if self.selection_mode == SelectionMode::Single {
                        self.select(self.focus_index, true);
                    }
                }
                true
            }
            InputKey::MouseLeft if event.variant == InputVariant::Press => {
                let Rect { y: cy, .. } = self.border.content_rect(0.0, 0.0, &self.look);
                let rel_y = event.mouse_y - cy + self.scroll_y;
                let clicked_idx = (rel_y / ROW_HEIGHT) as usize;
                if clicked_idx < self.items.len() && !event.alt && !event.meta {
                    self.focus_index = clicked_idx;
                    let trigger = event.is_repeat(); // double-click
                    self.select_by_input(clicked_idx, event.shift, event.ctrl, trigger);
                }
                true
            }
            InputKey::Space if event.variant == InputVariant::Press => {
                if !event.alt && !event.meta {
                    self.select_by_input(
                        self.focus_index,
                        event.shift,
                        event.ctrl,
                        false, // space never triggers
                    );
                }
                true
            }
            InputKey::Enter if event.variant == InputVariant::Press => {
                if !event.alt && !event.meta {
                    self.select_by_input(
                        self.focus_index,
                        event.shift,
                        event.ctrl,
                        true, // enter triggers
                    );
                }
                true
            }
            InputKey::Key('a') | InputKey::Key('A') if event.variant == InputVariant::Press => {
                if event.ctrl {
                    if self.selection_mode == SelectionMode::Multi
                        || self.selection_mode == SelectionMode::Toggle
                    {
                        if event.shift {
                            self.clear_selection();
                        } else {
                            self.select_all();
                        }
                    }
                    return true;
                }
                // Fall through to keywalk for plain 'a'.
                self.keywalk(event);
                true
            }
            _ => {
                if event.variant == InputVariant::Press && self.keywalk(event) {
                    return true;
                }
                false
            }
        }
    }

    // ── Preferred size ──────────────────────────────────────────────

    pub fn preferred_size(&self) -> (f64, f64) {
        let max_w = self
            .items
            .iter()
            .map(|it| Painter::measure_text_width(&it.text, ROW_HEIGHT - 2.0))
            .fold(0.0f64, f64::max);
        let h = self.items.len() as f64 * ROW_HEIGHT;
        self.border.preferred_size_for_content(max_w + 4.0, h)
    }

    /// Whether this list box provides how-to help text.
    /// Matches C++ `emListBox::HasHowTo` (always true).
    pub fn has_how_to(&self) -> bool {
        true
    }

    /// Help text describing how to use this list box.
    ///
    /// Chains the border's base how-to with list-box-specific sections.
    /// Matches C++ `emListBox::GetHowTo`.
    pub fn get_how_to(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.get_howto(enabled, focusable);
        text.push_str(HOWTO_LIST_BOX);
        match self.selection_mode {
            SelectionMode::ReadOnly => text.push_str(HOWTO_READ_ONLY_SELECTION),
            SelectionMode::Single => text.push_str(HOWTO_SINGLE_SELECTION),
            SelectionMode::Multi => text.push_str(HOWTO_MULTI_SELECTION),
            SelectionMode::Toggle => text.push_str(HOWTO_TOGGLE_SELECTION),
        }
        text
    }

    // ── Private helpers ─────────────────────────────────────────────

    fn fire_selection(&mut self) {
        if let Some(cb) = &mut self.on_selection {
            cb(&self.selected_indices);
        }
    }

    /// Rebuild the name_index HashMap from `from` onward.
    fn rebuild_name_index_from(&mut self, from: usize) {
        // For simplicity and correctness, rebuild the entire map when items
        // shift. This is O(n) but keeps the map consistent.
        self.name_index.clear();
        for (i, item) in self.items.iter().enumerate() {
            self.name_index.insert(item.name.clone(), i);
        }
        let _ = from; // Suppress unused warning; full rebuild is simpler.
    }

    /// Core selection-by-input logic matching C++ SelectByInput.
    fn select_by_input(&mut self, item_index: usize, shift: bool, ctrl: bool, trigger: bool) {
        if item_index >= self.items.len() {
            return;
        }

        match self.selection_mode {
            SelectionMode::ReadOnly => {
                // No selection action.
            }
            SelectionMode::Single => {
                self.select(item_index, true);
                if trigger {
                    self.trigger_item(item_index);
                }
            }
            SelectionMode::Multi => {
                if shift {
                    // Range selection from prev_input_index to item_index.
                    if let Some(prev) = self.prev_input_index {
                        if prev != item_index && prev < self.items.len() {
                            let (i1, i2) = if prev < item_index {
                                (prev + 1, item_index)
                            } else {
                                (item_index, prev - 1)
                            };
                            for i in i1..=i2 {
                                if ctrl {
                                    self.toggle_selection(i);
                                } else {
                                    self.select(i, false);
                                }
                            }
                        } else {
                            // prev == item_index: just select/toggle self
                            if ctrl {
                                self.toggle_selection(item_index);
                            } else {
                                self.select(item_index, false);
                            }
                        }
                    } else {
                        // No prev: just select/toggle self
                        if ctrl {
                            self.toggle_selection(item_index);
                        } else {
                            self.select(item_index, false);
                        }
                    }
                } else if ctrl {
                    self.toggle_selection(item_index);
                } else {
                    self.select(item_index, true);
                }
                if trigger {
                    self.trigger_item(item_index);
                }
            }
            SelectionMode::Toggle => {
                if shift {
                    // Range toggle from prev_input_index to item_index.
                    if let Some(prev) = self.prev_input_index {
                        if prev != item_index && prev < self.items.len() {
                            let (i1, i2) = if prev < item_index {
                                (prev + 1, item_index)
                            } else {
                                (item_index, prev - 1)
                            };
                            for i in i1..=i2 {
                                self.toggle_selection(i);
                            }
                        } else {
                            self.toggle_selection(item_index);
                        }
                    } else {
                        self.toggle_selection(item_index);
                    }
                } else {
                    self.toggle_selection(item_index);
                }
                if trigger {
                    self.trigger_item(item_index);
                }
            }
        }

        // Always set prev_input_index, even for ReadOnly.
        self.prev_input_index = Some(item_index);
    }

    /// Type-to-search (keywalk). Returns `true` if the event was consumed.
    fn keywalk(&mut self, event: &InputEvent) -> bool {
        if event.chars.is_empty() {
            return false;
        }
        if event.ctrl || event.alt || event.meta {
            return false;
        }
        // Reject control characters.
        for ch in event.chars.chars() {
            if ch <= ' ' || ch as u32 == 127 {
                return false;
            }
        }

        let now = std::time::Instant::now();

        // Check timeout.
        if let Some(prev_time) = self.keywalk_time {
            if now.duration_since(prev_time).as_millis() > KEYWALK_TIMEOUT_MS {
                self.keywalk_chars.clear();
            }
        } else {
            self.keywalk_chars.clear();
        }

        self.keywalk_chars.push_str(&event.chars);
        self.keywalk_time = Some(now);

        let search = self.keywalk_chars.clone();
        let matched_index = self.keywalk_search(&search);

        if let Some(idx) = matched_index {
            // Select the matched item if not read-only.
            if self.selection_mode != SelectionMode::ReadOnly {
                self.select(idx, true);
            }
            self.focus_index = idx;
            // Scroll to make the item visible.
            self.scroll_to_index(idx);
        } else {
            self.keywalk_chars.clear();
        }

        true
    }

    /// Search items for a keywalk match. Three strategies:
    /// 1. '*'-prefixed substring search (case insensitive)
    /// 2. Prefix match (case insensitive)
    /// 3. Fuzzy match (skips spaces/hyphens/underscores in item text)
    fn keywalk_search(&self, search: &str) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }

        if let Some(needle) = search.strip_prefix('*') {
            // Substring search (case insensitive).
            if needle.is_empty() {
                // '*' alone: match the first item (C++ strstr("", ...) always matches).
                return if self.items.is_empty() { None } else { Some(0) };
            }
            let needle_lower = needle.to_lowercase();
            for (i, item) in self.items.iter().enumerate() {
                if item.text.to_lowercase().contains(&needle_lower) {
                    return Some(i);
                }
            }
            return None;
        }

        // Strategy 2: prefix match (case insensitive).
        let search_lower = search.to_lowercase();
        for (i, item) in self.items.iter().enumerate() {
            let text_lower = item.text.to_lowercase();
            if text_lower.starts_with(&search_lower) {
                return Some(i);
            }
        }

        // Strategy 3: fuzzy match (skip spaces/hyphens/underscores in item text).
        for (i, item) in self.items.iter().enumerate() {
            if Self::fuzzy_match(&search_lower, &item.text) {
                return Some(i);
            }
        }

        None
    }

    /// Fuzzy match: for each character in `needle`, find the next matching
    /// character in `haystack`, skipping spaces/hyphens/underscores.
    fn fuzzy_match(needle: &str, haystack: &str) -> bool {
        let mut hay_chars = haystack.chars().peekable();
        for nc in needle.chars() {
            let nc_lower = nc.to_lowercase().next().unwrap_or(nc);
            loop {
                match hay_chars.next() {
                    None => return false,
                    Some(hc) => {
                        // Skip separators in haystack.
                        if hc == ' ' || hc == '-' || hc == '_' {
                            continue;
                        }
                        let hc_lower = hc.to_lowercase().next().unwrap_or(hc);
                        if hc_lower == nc_lower {
                            break;
                        }
                        // Mismatch on non-separator: this item doesn't match.
                        return false;
                    }
                }
            }
        }
        true
    }

    fn scroll_to_index(&mut self, index: usize) {
        // Ensure the item at `index` is visible within the content area.
        let item_top = index as f64 * ROW_HEIGHT;
        let item_bottom = item_top + ROW_HEIGHT;
        if item_top < self.scroll_y {
            self.scroll_y = item_top;
        } else {
            // We don't know the viewport height here, but we can at least
            // ensure the top of the item is in view.
            // A full implementation would use the widget's actual height.
            let _ = item_bottom;
        }
    }
}

/// C++ `emListBox::HowToListBox`.
const HOWTO_LIST_BOX: &str = "\n\n\
    LIST BOX\n\n\
    This is a list box. It may show any number of items from which one or more may\n\
    be selected (by program or by user). Selected items are shown highlighted.\n";

/// C++ `emListBox::HowToReadOnlySelection`.
const HOWTO_READ_ONLY_SELECTION: &str = "\n\n\
    READ-ONLY\n\n\
    This list box is read-only. You cannot modify the selection.\n\n\
    Keyboard control:\n\n\
      Any normal key               - To find and focus an item, you can simply\n\
                                     enter the first characters of its caption.\n";

/// C++ `emListBox::HowToSingleSelection`.
const HOWTO_SINGLE_SELECTION: &str = "\n\n\
    SINGLE-SELECTION\n\n\
    This is a single-selection list box. You can select only one item.\n\n\
    Mouse control:\n\n\
      Left-Button-Click            - Select the clicked item.\n\n\
      Left-Button-Double-Click     - Trigger the clicked item (application-defined\n\
                                     function).\n\n\
    Keyboard control:\n\n\
      Space                        - Select the focused item.\n\n\
      Enter                        - Trigger the focused item (application-defined\n\
                                     function).\n\n\
      Any normal key               - To find and focus an item, you can simply\n\
                                     enter the first characters of its caption.\n";

/// C++ `emListBox::HowToMultiSelection`.
const HOWTO_MULTI_SELECTION: &str = "\n\n\
    MULTI-SELECTION\n\n\
    This list box supports multi-selection. You can select one or more items.\n\n\
    Mouse control:\n\n\
      Left-Button-Click            - Select the clicked item.\n\n\
      Shift+Left-Button-Click      - Select the range of items from the previously\n\
                                     clicked item to this clicked item.\n\n\
      Ctrl+Left-Button-Click       - Invert the selection of the clicked item.\n\n\
      Shift+Ctrl+Left-Button-Click - Invert the selection of a range of items or\n\
                                     select an additional range.\n\n\
      Left-Button-Double-Click     - Trigger the clicked item (application-defined\n\
                                     function).\n";

/// C++ `emListBox::HowToToggleSelection`.
const HOWTO_TOGGLE_SELECTION: &str = "\n\n\
    TOGGLE-SELECTION\n\n\
    This is a toggle-selection list box. You can select or deselect\n\
    individual items independently from other items.\n\n\
    Mouse control:\n\n\
      Left-Button-Click            - Invert the selection of the clicked item.\n\n\
      Shift+Left-Button-Click      - Invert the selection of the range of items from\n\
                                     the previously clicked item to this clicked\n\
                                     item.\n\n\
      Left-Button-Double-Click     - Trigger the clicked item (application-defined\n\
                                     function).\n";

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn make_items(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn single_selection_arrow_keys() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_items(make_items(&["A", "B", "C"]));

        assert_eq!(lb.focus_index(), 0);

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 1);
        assert_eq!(lb.selected(), &[1]);

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 2);
        assert_eq!(lb.selected(), &[2]);

        // Won't go past end
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 2);

        lb.input(&InputEvent::press(InputKey::ArrowUp));
        assert_eq!(lb.focus_index(), 1);
        assert_eq!(lb.selected(), &[1]);
    }

    #[test]
    fn multi_selection_toggle() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Multi);
        lb.set_items(make_items(&["X", "Y", "Z"]));

        // In multi mode, ArrowDown doesn't auto-select
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 1);
        assert!(lb.selected().is_empty());

        // Space toggles selection (via select_by_input with no modifiers -> select solely)
        // In Multi mode without modifiers, space selects solely
        lb.input(&InputEvent::press(InputKey::Space));
        assert_eq!(lb.selected(), &[1]);

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        // Space without ctrl in multi mode selects solely (replaces)
        lb.input(&InputEvent::press(InputKey::Space));
        assert_eq!(lb.selected(), &[2]);

        // With ctrl, toggle
        lb.focus_index = 1;
        lb.input(&InputEvent::press(InputKey::Space).with_ctrl());
        assert!(lb.selected().contains(&1));
        assert!(lb.selected().contains(&2));

        // Toggle off with ctrl
        lb.input(&InputEvent::press(InputKey::Space).with_ctrl());
        assert_eq!(lb.selected(), &[2]);
    }

    #[test]
    fn trigger_callback() {
        let look = Look::new();
        let triggered = Rc::new(RefCell::new(None));
        let trig_clone = triggered.clone();

        let mut lb = ListBox::new(look);
        lb.set_items(make_items(&["A", "B"]));
        lb.on_trigger = Some(Box::new(move |idx| {
            *trig_clone.borrow_mut() = Some(idx);
        }));

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        lb.input(&InputEvent::press(InputKey::Enter));
        assert_eq!(*triggered.borrow(), Some(1));
    }

    #[test]
    fn selection_callback() {
        let look = Look::new();
        let selections = Rc::new(RefCell::new(Vec::new()));
        let sel_clone = selections.clone();

        let mut lb = ListBox::new(look);
        lb.set_items(make_items(&["A", "B", "C"]));
        lb.on_selection = Some(Box::new(move |sel| {
            sel_clone.borrow_mut().push(sel.to_vec());
        }));

        // First ArrowDown: selects item 1 solely. No prior selection to deselect.
        // Fires 1 callback (select).
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(selections.borrow().len(), 1);
        assert_eq!(selections.borrow()[0], vec![1]);

        // Second ArrowDown: selects item 2 solely. Deselects item 1 first (1 cb),
        // then selects item 2 (1 cb). Total: 3 callbacks.
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(selections.borrow().len(), 3);
        // Last callback should have item 2 selected.
        assert_eq!(selections.borrow()[2], vec![2]);
    }

    // ── New tests for added APIs ────────────────────────────────────

    #[test]
    fn add_and_insert_items() {
        let look = Look::new();
        let mut lb = ListBox::new(look);

        lb.add_item("a".into(), "Alpha".into());
        lb.add_item("b".into(), "Beta".into());
        assert_eq!(lb.item_count(), 2);
        assert_eq!(lb.get_item_text(0), "Alpha");
        assert_eq!(lb.get_item_text(1), "Beta");

        lb.insert_item(1, "g".into(), "Gamma".into());
        assert_eq!(lb.item_count(), 3);
        assert_eq!(lb.get_item_text(0), "Alpha");
        assert_eq!(lb.get_item_text(1), "Gamma");
        assert_eq!(lb.get_item_text(2), "Beta");

        // Name lookup
        assert_eq!(lb.get_item_index("g"), Some(1));
        assert_eq!(lb.get_item_index("z"), None);
    }

    #[test]
    fn remove_item_adjusts_selection() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());
        lb.add_item("c".into(), "C".into());

        lb.select(0, false);
        lb.select(2, false);
        assert_eq!(lb.selected(), &[0, 2]);

        // Remove item 1 ("B") — item 2 shifts to index 1.
        lb.remove_item(1);
        assert_eq!(lb.item_count(), 2);
        assert_eq!(lb.selected(), &[0, 1]);
        assert_eq!(lb.get_item_text(1), "C");
    }

    #[test]
    fn move_item_reorders() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());
        lb.add_item("c".into(), "C".into());

        lb.move_item(0, 2);
        assert_eq!(lb.get_item_text(0), "B");
        assert_eq!(lb.get_item_text(1), "C");
        assert_eq!(lb.get_item_text(2), "A");

        assert_eq!(lb.get_item_index("a"), Some(2));
        assert_eq!(lb.get_item_index("b"), Some(0));
    }

    #[test]
    fn sort_items_reorders() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("c".into(), "Cherry".into());
        lb.add_item("a".into(), "Apple".into());
        lb.add_item("b".into(), "Banana".into());

        lb.select(0, false); // Cherry at index 0

        let changed = lb.sort_items(|_n1, t1, _n2, t2| t1.cmp(t2));
        assert!(changed);
        assert_eq!(lb.get_item_text(0), "Apple");
        assert_eq!(lb.get_item_text(1), "Banana");
        assert_eq!(lb.get_item_text(2), "Cherry");

        // Cherry moved from 0 to 2; selection should follow.
        assert_eq!(lb.selected(), &[2]);
    }

    #[test]
    fn clear_items_resets_all() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.select(0, true);
        assert_eq!(lb.selected(), &[0]);

        lb.clear_items();
        assert_eq!(lb.item_count(), 0);
        assert!(lb.selected().is_empty());
    }

    #[test]
    fn selection_apis() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Multi);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());
        lb.add_item("c".into(), "C".into());

        assert_eq!(lb.selected_index(), None);

        lb.select(1, true);
        assert_eq!(lb.selected_index(), Some(1));
        assert!(lb.is_selected(1));
        assert!(!lb.is_selected(0));

        lb.select(0, false);
        assert_eq!(lb.selected_indices(), &[0, 1]);

        lb.deselect(1);
        assert_eq!(lb.selected_indices(), &[0]);

        lb.toggle_selection(0);
        assert!(lb.selected_indices().is_empty());

        lb.select_all();
        assert_eq!(lb.selected_indices(), &[0, 1, 2]);

        lb.clear_selection();
        assert!(lb.selected_indices().is_empty());
    }

    #[test]
    fn set_item_text_clears_keywalk() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "Alpha".into());
        lb.keywalk_chars = "al".into();

        lb.set_item_text(0, "Altered".into());
        assert!(lb.keywalk_chars.is_empty());
        assert_eq!(lb.get_item_text(0), "Altered");
    }

    #[test]
    fn read_only_mode_no_selection() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::ReadOnly);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());

        // Mouse click: select_by_input is called, but ReadOnly blocks selection.
        let ev = InputEvent::press(InputKey::MouseLeft).with_mouse(0.0, 5.0);
        lb.input(&ev);
        assert!(lb.selected().is_empty());
    }

    #[test]
    fn toggle_mode_selection() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Toggle);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());

        // Space toggles
        lb.input(&InputEvent::press(InputKey::Space));
        assert_eq!(lb.selected(), &[0]);

        lb.input(&InputEvent::press(InputKey::Space));
        assert!(lb.selected().is_empty());
    }

    #[test]
    fn ctrl_a_selects_all_in_multi_mode() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Multi);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());
        lb.add_item("c".into(), "C".into());

        // Ctrl+A selects all
        lb.input(&InputEvent::press(InputKey::Key('a')).with_ctrl());
        assert_eq!(lb.selected_indices(), &[0, 1, 2]);

        // Shift+Ctrl+A clears
        lb.input(&InputEvent::press(InputKey::Key('a')).with_shift_ctrl());
        assert!(lb.selected_indices().is_empty());
    }

    #[test]
    fn trigger_item_fires_callback() {
        let look = Look::new();
        let triggered = Rc::new(RefCell::new(Vec::new()));
        let trig_clone = triggered.clone();

        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());
        lb.on_trigger = Some(Box::new(move |idx| {
            trig_clone.borrow_mut().push(idx);
        }));

        lb.trigger_item(1);
        assert_eq!(*triggered.borrow(), vec![1]);
        assert_eq!(lb.triggered_index(), Some(1));
    }

    #[test]
    fn insert_adjusts_selected_indices() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());

        lb.select(1, true);
        assert_eq!(lb.selected(), &[1]);

        // Insert before the selected item.
        lb.insert_item(0, "z".into(), "Z".into());
        // Selected index should have shifted from 1 to 2.
        assert_eq!(lb.selected(), &[2]);
        assert!(lb.is_selected(2));
        assert_eq!(lb.get_item_text(2), "B");
    }

    #[test]
    fn keywalk_prefix_match() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "Apple".into());
        lb.add_item("b".into(), "Banana".into());
        lb.add_item("c".into(), "Cherry".into());

        let ev = InputEvent::press(InputKey::Key('b')).with_chars("b");
        lb.input(&ev);
        assert_eq!(lb.selected_index(), Some(1));
    }

    #[test]
    fn keywalk_substring_search() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "Apple".into());
        lb.add_item("b".into(), "Banana".into());

        // Type '*nan' to do substring search — "nan" is unique to "Banana".
        let ev1 = InputEvent::press(InputKey::Key('*')).with_chars("*");
        lb.input(&ev1);
        let ev2 = InputEvent::press(InputKey::Key('n')).with_chars("n");
        lb.input(&ev2);
        let ev3 = InputEvent::press(InputKey::Key('a')).with_chars("a");
        lb.input(&ev3);
        let ev4 = InputEvent::press(InputKey::Key('n')).with_chars("n");
        lb.input(&ev4);
        assert_eq!(lb.selected_index(), Some(1)); // "Banana" contains "nan"
    }

    #[test]
    fn keywalk_fuzzy_match() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "Red-Apple".into());
        lb.add_item("b".into(), "Banana".into());

        // Type "ra" — fuzzy matches "Red-Apple" (skips '-')
        // 'r' -> 'R' match, 'a' -> skip '-', match 'A'
        let ev = InputEvent::press(InputKey::Key('r')).with_chars("r");
        lb.input(&ev);
        let ev2 = InputEvent::press(InputKey::Key('a')).with_chars("a");
        lb.input(&ev2);
        assert_eq!(lb.selected_index(), Some(0));
    }

    #[test]
    fn out_of_range_operations_are_noop() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());

        // Out of range accessors return defaults.
        assert_eq!(lb.get_item_name(99), "");
        assert_eq!(lb.get_item_text(99), "");
        assert!(!lb.is_selected(99));

        // Out of range operations are no-ops.
        lb.remove_item(99);
        lb.move_item(99, 0);
        lb.trigger_item(99);
        lb.deselect(99);
        assert_eq!(lb.item_count(), 1);
    }

    #[test]
    #[should_panic(expected = "not unique")]
    fn duplicate_name_panics() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.add_item("a".into(), "Also A".into());
    }

    #[test]
    fn item_data_round_trip() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());

        lb.set_item_data(0, Some(Box::new(42_i32)));
        let data = lb.get_item_data(0).expect("data should be set");
        assert_eq!(data.downcast_ref::<i32>(), Some(&42));
    }

    #[test]
    fn select_solely_out_of_range_clears() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.add_item("a".into(), "A".into());
        lb.add_item("b".into(), "B".into());
        lb.select(0, false);
        lb.select(1, false);
        assert_eq!(lb.selected(), &[0, 1]);

        // Select out-of-range with solely=true should clear everything.
        lb.select(99, true);
        assert!(lb.selected().is_empty());
    }

    #[test]
    fn multi_shift_range_selection() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Multi);
        for i in 0..5 {
            lb.add_item(format!("{}", i), format!("Item {}", i));
        }

        // Click item 1 (sets prev_input_index)
        lb.select_by_input(1, false, false, false);
        assert_eq!(lb.selected(), &[1]);

        // Shift-click item 3 (range 2..=3 selected, keeping 1)
        lb.select_by_input(3, true, false, false);
        assert!(lb.is_selected(1));
        assert!(lb.is_selected(2));
        assert!(lb.is_selected(3));
    }

    #[test]
    fn toggle_mode_shift_range() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Toggle);
        for i in 0..5 {
            lb.add_item(format!("{}", i), format!("Item {}", i));
        }

        // Click item 1 (toggles on)
        lb.select_by_input(1, false, false, false);
        assert_eq!(lb.selected(), &[1]);

        // Shift-click item 3 (toggle range 2..=3)
        lb.select_by_input(3, true, false, false);
        assert!(lb.is_selected(1));
        assert!(lb.is_selected(2));
        assert!(lb.is_selected(3));
    }

    #[test]
    fn move_item_preserves_selection() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        for i in 0..4 {
            lb.add_item(format!("{}", i), format!("Item {}", i));
        }

        lb.select(0, false); // "Item 0"
        lb.select(3, false); // "Item 3"

        // Move item 0 to position 2
        lb.move_item(0, 2);
        // Items: 1, 2, 0, 3
        assert_eq!(lb.get_item_text(0), "Item 1");
        assert_eq!(lb.get_item_text(2), "Item 0");
        // "Item 0" (now at 2) and "Item 3" (still at 3) should be selected.
        assert!(lb.is_selected(2));
        assert!(lb.is_selected(3));
        assert!(!lb.is_selected(0));
    }
}
