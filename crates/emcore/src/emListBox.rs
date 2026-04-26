use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emEngineCtx::{ConstructCtx, PanelCtx};
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
use crate::emPanel::Rect;
use crate::emPanel::{PanelBehavior, PanelState};
use crate::emRasterLayout::emRasterLayout;
use crate::emSignal::SignalId;

use super::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use crate::emLook::emLook;

const ROW_HEIGHT: f64 = 17.0;

/// Timeout in milliseconds for keywalk type-to-search accumulation.
const KEYWALK_TIMEOUT_MS: u128 = 1000;

type SelectionCb = crate::emEngineCtx::WidgetCallbackRef<[usize]>;
type TriggerCb = crate::emEngineCtx::WidgetCallback<usize>;
type ItemPanelFactory = Box<dyn Fn(usize, String, bool) -> Box<dyn ItemPanelInterface>>;
type ItemBehaviorFactory =
    Box<dyn Fn(usize, &str, bool, Rc<emLook>, SelectionMode, bool) -> Box<dyn PanelBehavior>>;

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
    fn GetText(&self) -> &str;

    /// Whether selected.
    fn IsSelected(&self) -> bool;
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
    pub fn GetText(&self) -> &str {
        &self.text
    }

    /// Whether selected.
    pub fn IsSelected(&self) -> bool {
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

    fn GetText(&self) -> &str {
        &self.text
    }

    fn IsSelected(&self) -> bool {
        self.selected
    }
}

/// PanelBehavior implementation for DefaultItemPanel.
///
/// Each item becomes a real child panel in the tree, painting its own
/// selection highlight and text. Matches C++ `emListBox::DefaultItemPanel`.
pub(crate) struct DefaultItemPanelBehavior {
    text: String,
    selected: bool,
    look: Rc<emLook>,
    selection_mode: SelectionMode,
    enabled: bool,
}

impl DefaultItemPanelBehavior {
    pub fn new(
        text: String,
        selected: bool,
        look: Rc<emLook>,
        selection_mode: SelectionMode,
        enabled: bool,
    ) -> Self {
        Self {
            text,
            selected,
            look,
            selection_mode,
            enabled,
        }
    }
}

impl PanelBehavior for DefaultItemPanelBehavior {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        // C++ emListBox::DefaultItemPanel::Paint — emListBox.cpp:554-608

        let (mut bg_col, mut fg_col, mut hl_col) = if self.selection_mode != SelectionMode::ReadOnly
        {
            (
                self.look.input_bg_color,
                self.look.input_fg_color,
                self.look.input_hl_color,
            )
        } else {
            (
                self.look.output_bg_color,
                self.look.output_fg_color,
                self.look.output_hl_color,
            )
        };
        if !self.enabled {
            let base = self.look.bg_color;
            bg_col = bg_col.GetBlended(base, 80.0);
            fg_col = fg_col.GetBlended(base, 80.0);
            hl_col = hl_col.GetBlended(base, 80.0);
        }

        let x = 0.0;
        let y = 0.0;
        let cw = w;
        let ch = h;
        let mut canvas_color = canvas_color;

        if self.selected {
            let dx = cw.min(ch) * 0.015;
            let dy = cw.min(ch) * 0.015;
            let r = cw.min(ch) * 0.15;
            painter.PaintRoundRect(
                x + dx,
                y + dy,
                cw - 2.0 * dx,
                ch - 2.0 * dy,
                r,
                r,
                hl_col,
                canvas_color,
            );
            canvas_color = hl_col;
        }

        let dx = cw.min(ch) * 0.15;
        let dy = cw.min(ch) * 0.03;
        painter.PaintTextBoxed(
            x + dx,
            y + dy,
            cw - 2.0 * dx,
            ch - 2.0 * dy,
            &self.text,
            h,
            if self.selected { bg_col } else { fg_col },
            canvas_color,
            crate::emPainter::TextAlignment::Left,
            crate::emPainter::VAlign::Center,
            crate::emPainter::TextAlignment::Left,
            0.5,
            true,
            0.0,
        );
    }

    fn GetCanvasColor(&self) -> emColor {
        if self.selection_mode == SelectionMode::ReadOnly {
            self.look.output_bg_color
        } else {
            self.look.input_bg_color
        }
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
pub struct emListBox {
    border: emBorder,
    look: Rc<emLook>,
    last_w: f64,
    last_h: f64,
    items: Vec<Item>,
    /// O(1) lookup from item name to index.
    name_index: HashMap<String, usize>,
    /// Sorted list of currently selected item indices.
    selected_indices: Vec<usize>,
    focus_index: usize,
    scroll_y: f64,
    selection_mode: SelectionMode,
    /// Raster layout config used to position item children directly.
    /// C++ emListBox inherits from emRasterGroup; we use composition instead.
    raster_layout: emRasterLayout,
    /// Index of the last item that received input (for shift-range selection).
    prev_input_index: Option<usize>,
    /// Index of the last triggered item.
    triggered_index: Option<usize>,
    /// Accumulated characters for type-to-search.
    keywalk_chars: String,
    /// Timestamp of last keywalk input.
    keywalk_time: Option<std::time::Instant>,
    /// Injectable clock for keywalk timeout (testing support).
    /// When `None`, uses `std::time::Instant::now()`.
    keywalk_clock: Option<fn() -> std::time::Instant>,

    pub on_selection: Option<SelectionCb>,
    pub on_trigger: Option<TriggerCb>,

    /// Custom item panel factory. Port of C++ virtual CreateItemPanel.
    item_panel_factory: Option<ItemPanelFactory>,
    /// Custom item *behavior* factory. When set, `create_item_children` uses
    /// this instead of `DefaultItemPanelBehavior` for child panel painting.
    item_behavior_factory: Option<ItemBehaviorFactory>,
    /// Whether item panels are currently expanded.
    expanded: bool,
    /// Fixed number of columns for the item grid layout.
    /// Port of C++ `emListBox::SetFixedColumnCount`.
    fixed_column_count: Option<usize>,
    /// Last known viewport height (set during paint). Used by scroll_to_index.
    visible_height: f64,
    /// Whether the list box is enabled. Updated from PanelState.
    enabled: bool,
    /// Whether the list box is in the focused panel path.
    in_focused_path: bool,
    /// Allocated per C++ `emListBox::GetSelectionSignal()`. B3.4b: alloc only.
    pub selection_signal: SignalId,
    /// Allocated per C++ `emListBox::GetItemTriggerSignal()`.
    pub item_trigger_signal: SignalId,
    /// B3.4c: per-mutation snapshots drained at end of Input by
    /// `drain_pending_fires`. Each `fire_selection` push a snapshot of the
    /// selection; drain invokes `on_selection` once per snapshot — matching
    /// C++ per-SelectionChanged callback cadence. `selection_signal` is
    /// scheduler-coalesced (one pending fire regardless).
    pending_selection_snapshots: Vec<Vec<usize>>,
    pending_item_trigger_fire: bool,
    pending_trigger_index: Option<usize>,
}

impl emListBox {
    pub fn new<C: ConstructCtx>(ctx: &mut C, look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::Instrument)
                .with_inner(InnerBorderType::InputField)
                .with_how_to(true),
            look,
            last_w: 0.0,
            last_h: 0.0,
            items: Vec::new(),
            name_index: HashMap::new(),
            selected_indices: Vec::new(),
            focus_index: 0,
            scroll_y: 0.0,
            selection_mode: SelectionMode::Single,
            raster_layout: emRasterLayout::new(),
            prev_input_index: None,
            triggered_index: None,
            keywalk_chars: String::new(),
            keywalk_time: None,
            keywalk_clock: None,
            on_selection: None,
            on_trigger: None,
            item_panel_factory: None,
            item_behavior_factory: None,
            expanded: false,
            fixed_column_count: None,
            visible_height: 0.0,
            enabled: true,
            in_focused_path: false,
            selection_signal: ctx.create_signal(),
            item_trigger_signal: ctx.create_signal(),
            pending_selection_snapshots: Vec::new(),
            pending_item_trigger_fire: false,
            pending_trigger_index: None,
        }
    }

    /// Drain pending signal fires accumulated during Input. Phase-3 B3.4c.
    fn drain_pending_fires(&mut self, ctx: &mut PanelCtx<'_>) {
        let snapshots = std::mem::take(&mut self.pending_selection_snapshots);
        let trig = std::mem::replace(&mut self.pending_item_trigger_fire, false);
        let trig_index = self.pending_trigger_index.take();
        if snapshots.is_empty() && !trig {
            return;
        }
        let Some(mut sched) = ctx.as_sched_ctx() else {
            return;
        };
        if !snapshots.is_empty() {
            sched.fire(self.selection_signal);
            if let Some(cb) = self.on_selection.as_mut() {
                for snap in &snapshots {
                    cb(snap, &mut sched);
                }
            }
        }
        if trig {
            sched.fire(self.item_trigger_signal);
            if let (Some(idx), Some(cb)) = (trig_index, self.on_trigger.as_mut()) {
                cb(idx, &mut sched);
            }
        }
    }

    /// Set a fixed number of columns for the item grid layout.
    /// `None` uses auto-computed columns.
    /// Port of C++ `emListBox::SetFixedColumnCount`.
    pub fn set_fixed_column_count(&mut self, count: Option<usize>) {
        self.fixed_column_count = count;
        self.raster_layout.fixed_columns = count;
    }

    pub fn SetCaption(&mut self, caption: &str) {
        self.border.caption = caption.to_string();
    }

    /// Set minimum number of cells in the raster grid (padding with empty cells).
    /// Port of C++ `emRasterGroup::SetMinCellCount` (inherited by emListBox).
    pub fn SetMinCellCount(&mut self, count: usize) {
        self.raster_layout.min_cell_count = count;
    }

    /// Set preferred child tallness (h/w ratio) for items.
    /// Port of C++ `emRasterLayout::SetChildTallness` (inherited by emListBox via emRasterGroup).
    /// C++ sets PrefCT, MinCT, and MaxCT all to `tallness`.
    pub fn SetChildTallness(&mut self, tallness: f64) {
        self.raster_layout.preferred_child_tallness = tallness;
        self.raster_layout.min_child_tallness = tallness;
        self.raster_layout.max_child_tallness = tallness;
    }

    /// Set maximum child tallness only (not min/preferred).
    /// Port of C++ `emRasterLayout::SetMaxChildTallness`.
    pub fn SetMaxChildTallness(&mut self, max_ct: f64) {
        self.raster_layout.max_child_tallness = max_ct;
    }

    /// Enable strict raster mode.
    /// Port of C++ `emRasterGroup::SetStrictRaster` (inherited by emListBox).
    pub fn SetStrictRaster(&mut self) {
        self.raster_layout.strict_raster = true;
    }

    /// Set horizontal and vertical alignment.
    /// Port of C++ `emRasterGroup::SetAlignment` (inherited by emListBox).
    pub fn SetAlignment(&mut self, h: crate::emTiling::AlignmentH, v: crate::emTiling::AlignmentV) {
        self.raster_layout.alignment_h = h;
        self.raster_layout.alignment_v = v;
    }

    pub fn border_mut(&mut self) -> &mut emBorder {
        &mut self.border
    }

    // ── Selection mode ──────────────────────────────────────────────

    pub fn GetSelectionType(&self) -> SelectionMode {
        self.selection_mode
    }

    /// Set the selection type. Updates the inner border type to match:
    /// ReadOnly uses OutputField, all others use InputField.
    pub fn SetSelectionType(&mut self, mode: SelectionMode) {
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
        self.ClearItems();
        for text in items {
            self.AddItem(text.clone(), text);
        }
        self.focus_index = 0;
        self.scroll_y = 0.0;
    }

    // ── Item manipulation APIs ──────────────────────────────────────

    /// Number of items in the list.
    pub fn GetItemCount(&self) -> usize {
        self.items.len()
    }

    /// Add an item at the end. Panics if `name` is not unique.
    pub fn AddItem(&mut self, name: String, text: String) {
        self.add_item_with_data(name, text, None);
    }

    /// Add an item at the end with associated user data.
    /// Matches C++ `emListBox::AddItem(name, text, data)`.
    pub fn add_item_with_data(&mut self, name: String, text: String, data: Option<Box<dyn Any>>) {
        let idx = self.items.len();
        self.insert_item_with_data(idx, name, text, data);
    }

    /// Insert an item at `index`. Index is clamped to `[0, item_count()]`.
    /// Panics if `name` is not unique.
    pub fn InsertItem(&mut self, index: usize, name: String, text: String) {
        self.insert_item_with_data(index, name, text, None);
    }

    /// Insert an item at `index` with associated user data.
    /// Matches C++ `emListBox::InsertItem(index, name, text, data)`.
    pub fn insert_item_with_data(
        &mut self,
        index: usize,
        name: String,
        text: String,
        data: Option<Box<dyn Any>>,
    ) {
        let index = index.min(self.items.len());

        assert!(
            !self.name_index.contains_key(&name),
            "ListBox: item name '{}' is not unique",
            name
        );

        let item = Item {
            name: name.clone(),
            text,
            data,
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
    pub fn RemoveItem(&mut self, index: usize) {
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
    pub fn MoveItem(&mut self, from: usize, to: usize) {
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
    pub fn SortItems<F>(&mut self, mut compare: F) -> bool
    where
        F: FnMut(&str, &str, &str, &str) -> std::cmp::Ordering,
    {
        self.sort_items_with_data(|n1, t1, _d1, n2, t2, _d2| compare(n1, t1, n2, t2))
    }

    /// Sort items with access to user data.
    /// Matches C++ `emListBox::SortItems(compare, context)` where the
    /// comparator receives (name1, text1, data1, name2, text2, data2).
    pub fn sort_items_with_data<F>(&mut self, mut compare: F) -> bool
    where
        F: FnMut(&str, &str, Option<&dyn Any>, &str, &str, Option<&dyn Any>) -> std::cmp::Ordering,
    {
        // Check if already sorted.
        let old_order: Vec<String> = self.items.iter().map(|it| it.name.clone()).collect();

        self.items.sort_by(|a, b| {
            compare(
                &a.name,
                &a.text,
                a.data.as_deref(),
                &b.name,
                &b.text,
                b.data.as_deref(),
            )
        });

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
    pub fn ClearItems(&mut self) {
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
    pub fn GetItemName(&self, index: usize) -> &str {
        self.items.get(index).map_or("", |it| &it.name)
    }

    /// Get the display text of the item at `index`, or `""` if out of range.
    pub fn GetItemText(&self, index: usize) -> &str {
        self.items.get(index).map_or("", |it| &it.text)
    }

    /// Set the display text of the item at `index`. No-op if out of range or
    /// text unchanged.
    pub fn SetItemText(&mut self, index: usize, text: String) {
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
    pub fn GetItemData(&self, index: usize) -> Option<&dyn Any> {
        self.items.get(index).and_then(|it| it.data.as_deref())
    }

    /// Set the item data at `index`. No-op if out of range.
    pub fn SetItemData(&mut self, index: usize, data: Option<Box<dyn Any>>) {
        if let Some(item) = self.items.get_mut(index) {
            item.data = data;
            if let Some(iface) = &mut item.interface {
                iface.item_data_changed();
            }
            // Note: does NOT clear keywalk_chars (data doesn't affect search).
        }
    }

    /// Find an item's index by name. Returns `None` if not found.
    pub fn GetItemIndex(&self, name: &str) -> Option<usize> {
        self.name_index.get(name).copied()
    }

    // ── Legacy accessors (backward compat) ──────────────────────────

    /// Get item display texts as string slices. This allocates a Vec.
    pub fn items(&self) -> Vec<&str> {
        self.items.iter().map(|it| it.text.as_str()).collect()
    }

    /// Get the sorted selected indices.
    pub fn GetChecked(&self) -> &[usize] {
        &self.selected_indices
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    // ── Selection APIs ──────────────────────────────────────────────

    /// Index of the first selected item, or `None` if nothing is selected.
    pub fn GetSelectedIndex(&self) -> Option<usize> {
        self.selected_indices.first().copied()
    }

    /// Select a single item solely (deselecting all others).
    pub fn SetSelectedIndex(&mut self, index: usize) {
        self.Select(index, true);
    }

    /// Check whether an item is selected. Returns `false` for out-of-range.
    pub fn IsSelected(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|it| it.selected)
    }

    /// Select an item. If `solely` is true, deselects all others first.
    /// Out-of-range `index` with `solely` clears the selection.
    pub fn Select(&mut self, index: usize, solely: bool) {
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
                    self.Deselect(i);
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
            self.ClearSelection();
        }
    }

    /// Deselect an item. No-op if out of range or not selected.
    pub fn Deselect(&mut self, index: usize) {
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
    pub fn ToggleSelection(&mut self, index: usize) {
        if self.IsSelected(index) {
            self.Deselect(index);
        } else {
            self.Select(index, false);
        }
    }

    /// Select all items.
    pub fn SelectAll(&mut self) {
        for i in 0..self.items.len() {
            self.Select(i, false);
        }
    }

    /// Deselect all items.
    pub fn ClearSelection(&mut self) {
        while let Some(&idx) = self.selected_indices.first() {
            self.Deselect(idx);
        }
    }

    /// Get the sorted list of selected indices.
    pub fn GetSelectedIndices(&self) -> &[usize] {
        &self.selected_indices
    }

    /// Set the selected items by index.
    ///
    /// Replaces the current selection with exactly the given indices.
    /// Out-of-range indices are silently ignored. The resulting selection
    /// is always sorted. Matches C++ `emListBox::SetSelectedIndices`.
    pub fn SetSelectedIndices(&mut self, indices: &[usize]) {
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

    /// Trigger an item. Mirrors C++ `emListBox::TriggerItem`: updates state,
    /// fires `ItemTriggerSignal` + `on_trigger` callback.
    pub fn TriggerItem(&mut self, index: usize, ctx: &mut PanelCtx<'_>) {
        self.trigger_item_internal(index);
        self.drain_pending_fires(ctx);
    }

    /// Internal trigger update that only latches — used by input-path helpers
    /// whose surrounding `Input` call will drain the latches.
    fn trigger_item_internal(&mut self, index: usize) {
        if index < self.items.len() {
            self.triggered_index = Some(index);
            self.pending_item_trigger_fire = true;
            self.pending_trigger_index = Some(index);
        }
    }

    /// Index of the last triggered item, or `None`.
    pub fn GetTriggeredItemIndex(&self) -> Option<usize> {
        self.triggered_index
    }

    // ── Item Panel Interface ─────────────────────────────────────────

    /// Get the item panel interface at the given index.
    /// Port of C++ emListBox::GetItemPanelInterface(int).
    pub fn GetItemPanelInterface(&self, index: usize) -> Option<&dyn ItemPanelInterface> {
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
    pub fn GetItemPanel(&self, index: usize) -> Option<&dyn ItemPanelInterface> {
        self.GetItemPanelInterface(index)
    }

    /// Create an item panel for the item at the given index.
    /// Port of C++ emListBox::CreateItemPanel(name, itemIndex).
    ///
    /// Override point: the default creates a DefaultItemPanel.
    /// Custom emListBox implementations can override this by setting
    /// a factory function.
    pub fn CreateItemPanel(&mut self, index: usize) {
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

    /// Set a custom factory for creating item panel *behaviors* (visual layer).
    ///
    /// When set, `create_item_children` calls this factory instead of
    /// creating `DefaultItemPanelBehavior` for each item.
    /// Signature: `(index, text, selected, look, selection_mode, enabled) -> Box<dyn PanelBehavior>`
    pub fn set_item_behavior_factory<F>(&mut self, factory: F)
    where
        F: Fn(usize, &str, bool, Rc<emLook>, SelectionMode, bool) -> Box<dyn PanelBehavior>
            + 'static,
    {
        self.item_behavior_factory = Some(Box::new(factory));
    }

    /// Create item panels for all items. Called when the list box expands.
    /// Port of C++ emListBox::AutoExpand().
    pub fn auto_expand_items(&mut self) {
        self.expanded = true;
        for i in 0..self.items.len() {
            if self.items[i].interface.is_none() {
                self.CreateItemPanel(i);
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

    /// Create child panels for all items as direct children of the listbox.
    ///
    /// Called when the emListBox expands. Each item becomes a
    /// `DefaultItemPanelBehavior` direct child, matching C++ `emListBox`
    /// (which inherits from `emRasterGroup` — items are direct children).
    pub fn create_item_children(&mut self, ctx: &mut PanelCtx) {
        if !self.expanded {
            self.auto_expand_items();
        }

        // Configure raster layout settings for later use in layout_item_children.
        if let Some(cols) = self.fixed_column_count {
            self.raster_layout.fixed_columns = Some(cols);
        }

        // Create item panels as DIRECT children of the listbox (matching C++).
        let look = self.look.clone();
        let sel_mode = self.selection_mode;
        let enabled = self.enabled;
        for (i, item) in self.items.iter().enumerate() {
            let child = ctx.create_child(&item.name);
            let behavior: Box<dyn PanelBehavior> =
                if let Some(factory) = &self.item_behavior_factory {
                    factory(
                        i,
                        &item.text,
                        item.selected,
                        look.clone(),
                        sel_mode,
                        enabled,
                    )
                } else {
                    Box::new(DefaultItemPanelBehavior::new(
                        item.text.clone(),
                        item.selected,
                        look.clone(),
                        sel_mode,
                        enabled,
                    ))
                };
            ctx.tree.set_behavior(child, behavior);
        }
    }

    /// Layout item children directly using the raster layout engine.
    ///
    /// C++ emListBox inherits from emRasterGroup, which positions children
    /// in a grid. We use `do_layout_skip` to achieve the same layout on
    /// direct children without an intermediate panel.
    pub fn layout_item_children(&mut self, ctx: &mut PanelCtx, w: f64, h: f64) {
        let children = ctx.children();
        if children.is_empty() {
            return;
        }

        let cr = self.border.GetContentRectUnobscured(w, h, &self.look);
        self.raster_layout.do_layout_skip(ctx, None, Some(cr));

        // Propagate content canvas color to children.
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    // ── Paint ───────────────────────────────────────────────────────

    pub fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, pixel_scale: f64) {
        self.last_w = w;
        self.last_h = h;
        self.border.how_to_text = self.GetHowTo(true, true);
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);

        // When expanded with child panels, items are painted by their own
        // panel behaviors — skip inline painting (border only).
        if self.expanded {
            self.border.paint_inner_overlay(painter, w, h, &self.look);
            return;
        }

        // DIVERGED: (language-forced) C++ emListBox::PaintContent — no C++ equivalent; inline
        // painting is Rust-only. This guard reproduces the C++ effect where
        // tiny (non-expanded) listboxes show only the border frame.
        let pixel_area = pixel_scale * w * h;
        if pixel_area < 150.0 {
            self.border.paint_inner_overlay(painter, w, h, &self.look);
            return;
        }

        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.GetContentRectUnobscured(w, h, &self.look);

        // Store visible height for scroll_to_index
        self.visible_height = ch;

        painter.push_state();
        painter.SetClipping(cx, cy, cw, ch);

        // Determine colors based on selection mode and enabled state (C++ lines 562-577)
        let (bg, fg, hl) = if self.selection_mode == SelectionMode::ReadOnly {
            (
                self.look.output_bg_color,
                self.look.output_fg_color,
                self.look.output_hl_color,
            )
        } else {
            (
                self.look.input_bg_color,
                self.look.input_fg_color,
                self.look.input_hl_color,
            )
        };
        let (bg, fg, hl) = if !self.enabled {
            let base = self.look.bg_color;
            (
                bg.GetBlended(base, 80.0),
                fg.GetBlended(base, 80.0),
                hl.GetBlended(base, 80.0),
            )
        } else {
            (bg, fg, hl)
        };

        // C++ emListBox inherits from emRasterGroup (PrefChildTallness=0.2).
        // Compute grid dimensions matching emRasterLayout::auto_grid_clamped.
        let n = self.items.len();
        let pref_ct = 0.2_f64; // emRasterGroup default
        let (cols, rows) = if n == 0 || cw <= 0.0 || ch <= 0.0 {
            (1usize, n.max(1))
        } else {
            let mut rows_best = 1usize;
            let mut err_best = 0.0_f64;
            let mut r = 1usize;
            loop {
                let c = n.div_ceil(r);
                let ct = ch * c as f64 / (cw * r as f64);
                let err = (pref_ct / ct).ln().abs();
                if r == 1 || err < err_best {
                    rows_best = r;
                    err_best = err;
                }
                if c == 1 {
                    break;
                }
                r = (n + c - 2) / (c - 1);
            }
            (n.div_ceil(rows_best), rows_best)
        };
        let cell_w = cw / cols as f64;
        let cell_h = ch / rows as f64;

        for (i, item) in self.items.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let ix = cx + col as f64 * cell_w;
            let iy = cy + row as f64 * cell_h - self.scroll_y;
            if iy + cell_h < cy || iy > cy + ch {
                continue;
            }

            let item_w = cell_w;
            let item_h = cell_h;
            let s = item_w.min(item_h);

            let mut item_canvas = bg;

            if item.selected {
                let rdx = s * 0.015;
                let rdy = s * 0.015;
                let r = s * 0.15;
                painter.PaintRoundRect(
                    ix + rdx,
                    iy + rdy,
                    item_w - 2.0 * rdx,
                    item_h - 2.0 * rdy,
                    r,
                    r,
                    hl,
                    painter.GetCanvasColor(),
                );
                item_canvas = hl;
            }

            let dx = s * 0.15;
            let dy = s * 0.03;
            let text_color = if item.selected { bg } else { fg };
            painter.PaintTextBoxed(
                ix + dx,
                iy + dy,
                (item_w - 2.0 * dx).max(0.0),
                (item_h - 2.0 * dy).max(0.0),
                &item.text,
                item_h,
                text_color,
                item_canvas,
                crate::emPainter::TextAlignment::Left,
                crate::emPainter::VAlign::Center,
                crate::emPainter::TextAlignment::Left,
                0.5,
                true,
                0.0,
            );
        }

        painter.pop_state();

        // C++ paints content, THEN overlays the IO field border image.
        self.border.paint_inner_overlay(painter, w, h, &self.look);
    }

    /// Row height matching the paint path: `visible_height / items.len()`.
    /// Falls back to `ROW_HEIGHT` when there are no items or before first paint.
    fn row_height(&self) -> f64 {
        if self.items.is_empty() || self.visible_height <= 0.0 {
            ROW_HEIGHT
        } else {
            self.visible_height / self.items.len() as f64
        }
    }

    // ── Input ───────────────────────────────────────────────────────

    fn hit_test(&self, mx: f64, my: f64, pixel_tallness: f64) -> bool {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return false;
        }
        let tallness = self.last_h / self.last_w * pixel_tallness;
        let (rect, r) = self.border.GetContentRoundRect(1.0, tallness, &self.look);
        // RUST_ONLY: (language-forced-utility) widget_utils.rs -- C++ inlines this formula per widget
        let dx = ((rect.x - mx).max(mx - rect.x - rect.w) + r).max(0.0);
        let dy = ((rect.y - my).max(my - rect.y - rect.h) + r).max(0.0);
        dx * dx + dy * dy <= r * r
    }

    pub fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        let consumed = self.input_impl(event, state, input_state);
        self.drain_pending_fires(ctx);
        consumed
    }

    fn input_impl(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        if self.items.is_empty() {
            return false;
        }

        match event.key {
            InputKey::ArrowDown if event.variant == InputVariant::Press => {
                if self.focus_index + 1 < self.items.len() {
                    self.focus_index += 1;
                    if self.selection_mode == SelectionMode::Single {
                        self.Select(self.focus_index, true);
                    }
                }
                true
            }
            InputKey::ArrowUp if event.variant == InputVariant::Press => {
                if self.focus_index > 0 {
                    self.focus_index -= 1;
                    if self.selection_mode == SelectionMode::Single {
                        self.Select(self.focus_index, true);
                    }
                }
                true
            }
            InputKey::Home if event.variant == InputVariant::Press => {
                self.focus_index = 0;
                if self.selection_mode == SelectionMode::Single {
                    self.Select(self.focus_index, true);
                }
                true
            }
            InputKey::End if event.variant == InputVariant::Press => {
                self.focus_index = self.items.len() - 1;
                if self.selection_mode == SelectionMode::Single {
                    self.Select(self.focus_index, true);
                }
                true
            }
            InputKey::MouseLeft if event.variant == InputVariant::Press => {
                if !self.hit_test(event.mouse_x, event.mouse_y, state.pixel_tallness) {
                    return false;
                }
                // Mouse coordinates are in normalized panel-local space
                // (x in [0,1], y in [0,tallness]). The paint dimensions
                // (last_w, last_h) use the layout aspect ratio, but the
                // input coordinate Y-axis is additionally scaled by
                // pixel_tallness, so the effective tallness for input is
                // (last_h / last_w) * pixel_tallness.
                let tallness = if self.last_w > 0.0 {
                    self.last_h / self.last_w * state.pixel_tallness
                } else {
                    1.0
                };
                let cr = self
                    .border
                    .GetContentRectUnobscured(1.0, tallness, &self.look);
                let row_h = if self.items.is_empty() {
                    cr.h
                } else {
                    cr.h / self.items.len() as f64
                };
                let rel_y = event.mouse_y - cr.y + self.scroll_y;
                let clicked_idx = (rel_y / row_h) as usize;
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
                            self.ClearSelection();
                        } else {
                            self.SelectAll();
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
            .map(|it| emPainter::measure_text_width(&it.text, ROW_HEIGHT - 2.0))
            .fold(0.0f64, f64::max);
        let h = self.items.len() as f64 * ROW_HEIGHT;
        self.border.preferred_size_for_content(max_w + 4.0, h)
    }

    /// Whether this list box provides how-to help text.
    /// Matches C++ `emListBox::HasHowTo` (always true).
    pub fn HasHowTo(&self) -> bool {
        true
    }

    /// Help text describing how to use this list box.
    ///
    /// Chains the border's base how-to with list-box-specific sections.
    /// Matches C++ `emListBox::GetHowTo`.
    pub fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        let mut text = self.border.GetHowTo(enabled, focusable);
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
        // B3.4c/d latch: snapshot current selection; `drain_pending_fires`
        // invokes `on_selection` once per snapshot at end of Input (or from
        // ctx-bearing setter paths).
        self.pending_selection_snapshots
            .push(self.selected_indices.clone());
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
                self.Select(item_index, true);
                if trigger {
                    self.trigger_item_internal(item_index);
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
                                    self.ToggleSelection(i);
                                } else {
                                    self.Select(i, false);
                                }
                            }
                        } else {
                            // prev == item_index: just select/toggle self
                            if ctrl {
                                self.ToggleSelection(item_index);
                            } else {
                                self.Select(item_index, false);
                            }
                        }
                    } else {
                        // No prev: just select/toggle self
                        if ctrl {
                            self.ToggleSelection(item_index);
                        } else {
                            self.Select(item_index, false);
                        }
                    }
                } else if ctrl {
                    self.ToggleSelection(item_index);
                } else {
                    self.Select(item_index, true);
                }
                if trigger {
                    self.trigger_item_internal(item_index);
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
                                self.ToggleSelection(i);
                            }
                        } else {
                            self.ToggleSelection(item_index);
                        }
                    } else {
                        self.ToggleSelection(item_index);
                    }
                } else {
                    self.ToggleSelection(item_index);
                }
                if trigger {
                    self.trigger_item_internal(item_index);
                }
            }
        }

        // Always set prev_input_index, even for ReadOnly.
        self.prev_input_index = Some(item_index);
    }

    /// Type-to-search (keywalk). Returns `true` if the event was consumed.
    fn keywalk(&mut self, event: &emInputEvent) -> bool {
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

        let now = self
            .keywalk_clock
            .map_or_else(std::time::Instant::now, |f| f());

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
                self.Select(idx, true);
            }
            self.focus_index = idx;
            // Scroll to make the item visible.
            self.scroll_to_index(idx);
        } else {
            self.keywalk_chars.clear();
            crate::emWindowPlatform::system_beep();
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

    /// Notify the list box of a focus path change.
    /// Clears keywalk state when focus is lost. Matches C++ emListBox::Notice
    /// `NF_FOCUS_CHANGED` handler (emListBox.cpp:647-656).
    pub fn on_focus_changed(&mut self, in_focused_path: bool) {
        self.in_focused_path = in_focused_path;
        if !in_focused_path {
            self.keywalk_chars.clear();
        }
    }

    /// Notify the list box of an enabled state change.
    pub fn on_enable_changed(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn scroll_to_index(&mut self, index: usize) {
        let rh = self.row_height();
        let item_top = index as f64 * rh;
        let item_bottom = item_top + rh;
        if item_top < self.scroll_y {
            self.scroll_y = item_top;
        } else if self.visible_height > 0.0 && item_bottom > self.scroll_y + self.visible_height {
            self.scroll_y = item_bottom - self.visible_height;
        }
    }

    /// Set a custom clock function for keywalk timeout testing.
    pub fn set_keywalk_clock(&mut self, clock: fn() -> std::time::Instant) {
        self.keywalk_clock = Some(clock);
    }
}

/// C++ `emListBox::HowToListBox`.
const HOWTO_LIST_BOX: &str = concat!(
    "\n\n",
    "LIST BOX\n\n",
    "This is a list box. It may show any number of items from which one or more may\n",
    "be selected (by program or by user). Selected items are shown highlighted.\n",
);

/// C++ `emListBox::HowToReadOnlySelection`.
const HOWTO_READ_ONLY_SELECTION: &str = concat!(
    "\n\n",
    "READ-ONLY\n\n",
    "This list box is read-only. You cannot modify the selection.\n\n",
    "Keyboard control:\n\n",
    "  Any normal key               - To find and focus an item, you can simply\n",
    "                                 enter the first characters of its caption.\n",
);

/// C++ `emListBox::HowToSingleSelection`.
const HOWTO_SINGLE_SELECTION: &str = concat!(
    "\n\n",
    "SINGLE-SELECTION\n\n",
    "This is a single-selection list box. You can select only one item.\n\n",
    "Mouse control:\n\n",
    "  Left-Button-Click            - Select the clicked item.\n\n",
    "  Left-Button-Double-Click     - Trigger the clicked item (application-defined\n",
    "                                 function).\n\n",
    "Keyboard control:\n\n",
    "  Space                        - Select the focused item.\n\n",
    "  Enter                        - Trigger the focused item (application-defined\n",
    "                                 function).\n\n",
    "  Any normal key               - To find, focus and select an item, you can simply\n",
    "                                 enter the first characters of its caption.\n",
);

/// C++ `emListBox::HowToMultiSelection`.
const HOWTO_MULTI_SELECTION: &str = concat!(
    "\n\n",
    "MULTI-SELECTION\n\n",
    "This list box supports multi-selection. You can select one or more items.\n\n",
    "Mouse control:\n\n",
    "  Left-Button-Click            - Select the clicked item.\n\n",
    "  Shift+Left-Button-Click      - Select the range of items from the previously\n",
    "                                 clicked item to this clicked item.\n\n",
    "  Ctrl+Left-Button-Click       - Invert the selection of the clicked item.\n\n",
    "  Shift+Ctrl+Left-Button-Click - Invert the selection of a range of items or\n",
    "                                 select an additional range.\n\n",
    "  Left-Button-Double-Click     - Trigger the clicked item (application-defined\n",
    "                                 function).\n\n",
    "Keyboard control:\n\n",
    "  Space                        - Select the focused item.\n\n",
    "  Shift+Space                  - Select the range of items from the previously\n",
    "                                 selected item to the focused item.\n\n",
    "  Ctrl+Space                   - Invert the selection of the focused item.\n\n",
    "  Shift+Ctrl+Space             - Invert the selection of a range of items or\n",
    "                                 select an additional range.\n\n",
    "  Ctrl+A                       - Select all items.\n\n",
    "  Shift+Ctrl+A                 - Clear the selection.\n\n",
    "  Enter                        - Trigger the focused item (application-defined\n",
    "                                 function).\n\n",
    "  Any normal key               - To find, focus and select an item, you can simply\n",
    "                                 enter the first characters of its caption.\n",
);

/// C++ `emListBox::HowToToggleSelection`.
const HOWTO_TOGGLE_SELECTION: &str = concat!(
    "\n\n",
    "TOGGLE-SELECTION\n\n",
    "This is a toggle-selection list box. You can select or deselect\n",
    "individual items independently from other items.\n\n",
    "Mouse control:\n\n",
    "  Left-Button-Click            - Invert the selection of the clicked item.\n\n",
    "  Shift+Left-Button-Click      - Invert the selection of the range of items from\n",
    "                                 the previously clicked item to this clicked\n",
    "                                 item.\n\n",
    "  Left-Button-Double-Click     - Trigger the clicked item (application-defined\n",
    "                                 function).\n\n",
    "Keyboard control:\n\n",
    "  Space                        - Invert the selection of the focused item.\n\n",
    "  Shift+Space                  - Invert the selection of the range of items from\n",
    "                                 the previously selected item to the focused\n",
    "                                 item.\n\n",
    "  Ctrl+A                       - Select all items.\n\n",
    "  Shift+Ctrl+A                 - Deselect all items.\n\n",
    "  Enter                        - Trigger the focused item (application-defined\n",
    "                                 function).\n\n",
    "  Any normal key               - To find, focus and select an item, you can simply\n",
    "                                 enter the first characters of its caption.\n",
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx};
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;
    use slotmap::Key as _;
    use std::cell::RefCell;

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    fn default_panel_state() -> PanelState {
        PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        }
    }

    fn default_input_state() -> emInputState {
        emInputState::new()
    }

    fn make_items(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|s| s.to_string()).collect()
    }

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
        pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl Drop for TestInit {
        fn drop(&mut self) {
            // B3.4c: clear pending signals accumulated during Input-path tests
            self.sched.clear_pending_for_tests();
        }
    }

    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: crate::emContext::emContext::NewRoot(),
                pa: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                pending_actions: &self.pa,
            }
        }
    }

    #[test]
    fn single_selection_arrow_keys() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.set_items(make_items(&["A", "B", "C"]));

        assert_eq!(lb.focus_index(), 0);

        lb.Input(
            &emInputEvent::press(InputKey::ArrowDown),
            &ps,
            &is,
            &mut ctx,
        );
        assert_eq!(lb.focus_index(), 1);
        assert_eq!(lb.GetChecked(), &[1]);

        lb.Input(
            &emInputEvent::press(InputKey::ArrowDown),
            &ps,
            &is,
            &mut ctx,
        );
        assert_eq!(lb.focus_index(), 2);
        assert_eq!(lb.GetChecked(), &[2]);

        // Won't go past end
        lb.Input(
            &emInputEvent::press(InputKey::ArrowDown),
            &ps,
            &is,
            &mut ctx,
        );
        assert_eq!(lb.focus_index(), 2);

        lb.Input(&emInputEvent::press(InputKey::ArrowUp), &ps, &is, &mut ctx);
        assert_eq!(lb.focus_index(), 1);
        assert_eq!(lb.GetChecked(), &[1]);
    }

    #[test]
    fn multi_selection_toggle() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.SetSelectionType(SelectionMode::Multi);
        lb.set_items(make_items(&["X", "Y", "Z"]));

        // In multi mode, ArrowDown doesn't auto-select
        lb.Input(
            &emInputEvent::press(InputKey::ArrowDown),
            &ps,
            &is,
            &mut ctx,
        );
        assert_eq!(lb.focus_index(), 1);
        assert!(lb.GetChecked().is_empty());

        // Space toggles selection (via select_by_input with no modifiers -> select solely)
        // In Multi mode without modifiers, space selects solely
        lb.Input(&emInputEvent::press(InputKey::Space), &ps, &is, &mut ctx);
        assert_eq!(lb.GetChecked(), &[1]);

        lb.Input(
            &emInputEvent::press(InputKey::ArrowDown),
            &ps,
            &is,
            &mut ctx,
        );
        // Space without ctrl in multi mode selects solely (replaces)
        lb.Input(&emInputEvent::press(InputKey::Space), &ps, &is, &mut ctx);
        assert_eq!(lb.GetChecked(), &[2]);

        // With ctrl, toggle
        lb.focus_index = 1;
        lb.Input(
            &emInputEvent::press(InputKey::Space).with_ctrl(),
            &ps,
            &is,
            &mut ctx,
        );
        assert!(lb.GetChecked().contains(&1));
        assert!(lb.GetChecked().contains(&2));

        // Toggle off with ctrl
        lb.Input(
            &emInputEvent::press(InputKey::Space).with_ctrl(),
            &ps,
            &is,
            &mut ctx,
        );
        assert_eq!(lb.GetChecked(), &[2]);
    }

    #[test]
    fn trigger_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let triggered = Rc::new(RefCell::new(None));
        let trig_clone = triggered.clone();

        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.set_items(make_items(&["A", "B"]));
        lb.on_trigger = Some(Box::new(
            move |idx: usize, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                *trig_clone.borrow_mut() = Some(idx);
            },
        ));

        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            lb.Input(
                &emInputEvent::press(InputKey::ArrowDown),
                &ps,
                &is,
                &mut ctx,
            );
            lb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        }
        assert_eq!(*triggered.borrow(), Some(1));
    }

    #[test]
    fn selection_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let selections = Rc::new(RefCell::new(Vec::new()));
        let sel_clone = selections.clone();

        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.set_items(make_items(&["A", "B", "C"]));
        lb.on_selection = Some(Box::new(
            move |sel: &[usize], _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                sel_clone.borrow_mut().push(sel.to_vec());
            },
        ));

        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );

            // First ArrowDown: selects item 1 solely. No prior selection to deselect.
            // Fires 1 callback (select).
            lb.Input(
                &emInputEvent::press(InputKey::ArrowDown),
                &ps,
                &is,
                &mut ctx,
            );
            assert_eq!(selections.borrow().len(), 1);
            assert_eq!(selections.borrow()[0], vec![1]);

            // Second ArrowDown: selects item 2 solely. Deselects item 1 first (1 cb),
            // then selects item 2 (1 cb). Total: 3 callbacks.
            lb.Input(
                &emInputEvent::press(InputKey::ArrowDown),
                &ps,
                &is,
                &mut ctx,
            );
        }
        assert_eq!(selections.borrow().len(), 3);
        // Last callback should have item 2 selected.
        assert_eq!(selections.borrow()[2], vec![2]);
    }

    #[test]
    fn list_box_fires_selection_signal_on_input_arrow_down() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.set_items(make_items(&["A", "B"]));
        let sig = lb.selection_signal;
        let ps = default_panel_state();
        let is = default_input_state();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            lb.Input(
                &emInputEvent::press(InputKey::ArrowDown),
                &ps,
                &is,
                &mut ctx,
            );
        }
        assert!(__init.sched.is_pending(sig));
    }

    #[test]
    fn list_box_fires_item_trigger_signal_on_enter() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.set_items(make_items(&["A", "B"]));
        let sig = lb.item_trigger_signal;
        let ps = default_panel_state();
        let is = default_input_state();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            lb.Input(
                &emInputEvent::press(InputKey::ArrowDown),
                &ps,
                &is,
                &mut ctx,
            );
            lb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        }
        assert!(__init.sched.is_pending(sig));
    }

    // ── New tests for added APIs ────────────────────────────────────

    #[test]
    fn add_and_insert_items() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);

        lb.AddItem("a".into(), "Alpha".into());
        lb.AddItem("b".into(), "Beta".into());
        assert_eq!(lb.GetItemCount(), 2);
        assert_eq!(lb.GetItemText(0), "Alpha");
        assert_eq!(lb.GetItemText(1), "Beta");

        lb.InsertItem(1, "g".into(), "Gamma".into());
        assert_eq!(lb.GetItemCount(), 3);
        assert_eq!(lb.GetItemText(0), "Alpha");
        assert_eq!(lb.GetItemText(1), "Gamma");
        assert_eq!(lb.GetItemText(2), "Beta");

        // Name lookup
        assert_eq!(lb.GetItemIndex("g"), Some(1));
        assert_eq!(lb.GetItemIndex("z"), None);
    }

    #[test]
    fn remove_item_adjusts_selection() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());
        lb.AddItem("c".into(), "C".into());

        lb.Select(0, false);
        lb.Select(2, false);
        assert_eq!(lb.GetChecked(), &[0, 2]);

        // Remove item 1 ("B") — item 2 shifts to index 1.
        lb.RemoveItem(1);
        assert_eq!(lb.GetItemCount(), 2);
        assert_eq!(lb.GetChecked(), &[0, 1]);
        assert_eq!(lb.GetItemText(1), "C");
    }

    #[test]
    fn move_item_reorders() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());
        lb.AddItem("c".into(), "C".into());

        lb.MoveItem(0, 2);
        assert_eq!(lb.GetItemText(0), "B");
        assert_eq!(lb.GetItemText(1), "C");
        assert_eq!(lb.GetItemText(2), "A");

        assert_eq!(lb.GetItemIndex("a"), Some(2));
        assert_eq!(lb.GetItemIndex("b"), Some(0));
    }

    #[test]
    fn sort_items_reorders() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("c".into(), "Cherry".into());
        lb.AddItem("a".into(), "Apple".into());
        lb.AddItem("b".into(), "Banana".into());

        lb.Select(0, false); // Cherry at index 0

        let changed = lb.SortItems(|_n1, t1, _n2, t2| t1.cmp(t2));
        assert!(changed);
        assert_eq!(lb.GetItemText(0), "Apple");
        assert_eq!(lb.GetItemText(1), "Banana");
        assert_eq!(lb.GetItemText(2), "Cherry");

        // Cherry moved from 0 to 2; selection should follow.
        assert_eq!(lb.GetChecked(), &[2]);
    }

    #[test]
    fn clear_items_resets_all() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.Select(0, true);
        assert_eq!(lb.GetChecked(), &[0]);

        lb.ClearItems();
        assert_eq!(lb.GetItemCount(), 0);
        assert!(lb.GetChecked().is_empty());
    }

    #[test]
    fn selection_apis() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.SetSelectionType(SelectionMode::Multi);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());
        lb.AddItem("c".into(), "C".into());

        assert_eq!(lb.GetSelectedIndex(), None);

        lb.Select(1, true);
        assert_eq!(lb.GetSelectedIndex(), Some(1));
        assert!(lb.IsSelected(1));
        assert!(!lb.IsSelected(0));

        lb.Select(0, false);
        assert_eq!(lb.GetSelectedIndices(), &[0, 1]);

        lb.Deselect(1);
        assert_eq!(lb.GetSelectedIndices(), &[0]);

        lb.ToggleSelection(0);
        assert!(lb.GetSelectedIndices().is_empty());

        lb.SelectAll();
        assert_eq!(lb.GetSelectedIndices(), &[0, 1, 2]);

        lb.ClearSelection();
        assert!(lb.GetSelectedIndices().is_empty());
    }

    #[test]
    fn set_item_text_clears_keywalk() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "Alpha".into());
        lb.keywalk_chars = "al".into();

        lb.SetItemText(0, "Altered".into());
        assert!(lb.keywalk_chars.is_empty());
        assert_eq!(lb.GetItemText(0), "Altered");
    }

    #[test]
    fn read_only_mode_no_selection() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.SetSelectionType(SelectionMode::ReadOnly);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());

        // Mouse click: select_by_input is called, but ReadOnly blocks selection.
        let ev = emInputEvent::press(InputKey::MouseLeft).with_mouse(0.0, 5.0);
        lb.Input(&ev, &ps, &is, &mut ctx);
        assert!(lb.GetChecked().is_empty());
    }

    #[test]
    fn toggle_mode_selection() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.SetSelectionType(SelectionMode::Toggle);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());

        // Space toggles
        lb.Input(&emInputEvent::press(InputKey::Space), &ps, &is, &mut ctx);
        assert_eq!(lb.GetChecked(), &[0]);

        lb.Input(&emInputEvent::press(InputKey::Space), &ps, &is, &mut ctx);
        assert!(lb.GetChecked().is_empty());
    }

    #[test]
    fn ctrl_a_selects_all_in_multi_mode() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.SetSelectionType(SelectionMode::Multi);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());
        lb.AddItem("c".into(), "C".into());

        // Ctrl+A selects all
        lb.Input(
            &emInputEvent::press(InputKey::Key('a')).with_ctrl(),
            &ps,
            &is,
            &mut ctx,
        );
        assert_eq!(lb.GetSelectedIndices(), &[0, 1, 2]);

        // Shift+Ctrl+A clears
        lb.Input(
            &emInputEvent::press(InputKey::Key('a')).with_shift_ctrl(),
            &ps,
            &is,
            &mut ctx,
        );
        assert!(lb.GetSelectedIndices().is_empty());
    }

    #[test]
    fn trigger_item_fires_callback() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let look = emLook::new();
        let triggered = Rc::new(RefCell::new(Vec::new()));
        let trig_clone = triggered.clone();

        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());
        lb.on_trigger = Some(Box::new(
            move |idx: usize, _sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
                trig_clone.borrow_mut().push(idx);
            },
        ));

        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        let mut ctx = PanelCtx::with_sched_reach(
            &mut tree,
            tid,
            1.0,
            &mut __init.sched,
            &mut __init.fw,
            &__init.root,
            &fw_cb,
            &__init.pa,
        );
        lb.TriggerItem(1, &mut ctx);
        assert_eq!(*triggered.borrow(), vec![1]);
        assert_eq!(lb.GetTriggeredItemIndex(), Some(1));
    }

    #[test]
    fn insert_adjusts_selected_indices() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());

        lb.Select(1, true);
        assert_eq!(lb.GetChecked(), &[1]);

        // Insert before the selected item.
        lb.InsertItem(0, "z".into(), "Z".into());
        // Selected index should have shifted from 1 to 2.
        assert_eq!(lb.GetChecked(), &[2]);
        assert!(lb.IsSelected(2));
        assert_eq!(lb.GetItemText(2), "B");
    }

    #[test]
    fn keywalk_prefix_match() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.AddItem("a".into(), "Apple".into());
        lb.AddItem("b".into(), "Banana".into());
        lb.AddItem("c".into(), "Cherry".into());

        let ev = emInputEvent::press(InputKey::Key('b')).with_chars("b");
        lb.Input(&ev, &ps, &is, &mut ctx);
        assert_eq!(lb.GetSelectedIndex(), Some(1));
    }

    #[test]
    fn keywalk_substring_search() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.AddItem("a".into(), "Apple".into());
        lb.AddItem("b".into(), "Banana".into());

        // Type '*nan' to do substring search — "nan" is unique to "Banana".
        let ev1 = emInputEvent::press(InputKey::Key('*')).with_chars("*");
        lb.Input(&ev1, &ps, &is, &mut ctx);
        let ev2 = emInputEvent::press(InputKey::Key('n')).with_chars("n");
        lb.Input(&ev2, &ps, &is, &mut ctx);
        let ev3 = emInputEvent::press(InputKey::Key('a')).with_chars("a");
        lb.Input(&ev3, &ps, &is, &mut ctx);
        let ev4 = emInputEvent::press(InputKey::Key('n')).with_chars("n");
        lb.Input(&ev4, &ps, &is, &mut ctx);
        assert_eq!(lb.GetSelectedIndex(), Some(1)); // "Banana" contains "nan"
    }

    #[test]
    fn keywalk_fuzzy_match() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        let ps = default_panel_state();
        let is = default_input_state();
        lb.AddItem("a".into(), "Red-Apple".into());
        lb.AddItem("b".into(), "Banana".into());

        // Type "ra" — fuzzy matches "Red-Apple" (skips '-')
        // 'r' -> 'R' match, 'a' -> skip '-', match 'A'
        let ev = emInputEvent::press(InputKey::Key('r')).with_chars("r");
        lb.Input(&ev, &ps, &is, &mut ctx);
        let ev2 = emInputEvent::press(InputKey::Key('a')).with_chars("a");
        lb.Input(&ev2, &ps, &is, &mut ctx);
        assert_eq!(lb.GetSelectedIndex(), Some(0));
    }

    #[test]
    fn out_of_range_operations_are_noop() {
        let mut __init = TestInit::new();
        let (mut tree, tid) = test_tree();
        let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());

        // Out of range accessors return defaults.
        assert_eq!(lb.GetItemName(99), "");
        assert_eq!(lb.GetItemText(99), "");
        assert!(!lb.IsSelected(99));

        // Out of range operations are no-ops.
        lb.RemoveItem(99);
        lb.MoveItem(99, 0);
        lb.TriggerItem(99, &mut ctx);
        lb.Deselect(99);
        assert_eq!(lb.GetItemCount(), 1);
    }

    #[test]
    #[should_panic(expected = "not unique")]
    fn duplicate_name_panics() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("a".into(), "Also A".into());
    }

    #[test]
    fn item_data_round_trip() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());

        lb.SetItemData(0, Some(Box::new(42_i32)));
        let data = lb.GetItemData(0).expect("data should be set");
        assert_eq!(data.downcast_ref::<i32>(), Some(&42));
    }

    #[test]
    fn select_solely_out_of_range_clears() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.AddItem("a".into(), "A".into());
        lb.AddItem("b".into(), "B".into());
        lb.Select(0, false);
        lb.Select(1, false);
        assert_eq!(lb.GetChecked(), &[0, 1]);

        // Select out-of-range with solely=true should clear everything.
        lb.Select(99, true);
        assert!(lb.GetChecked().is_empty());
    }

    #[test]
    fn multi_shift_range_selection() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.SetSelectionType(SelectionMode::Multi);
        for i in 0..5 {
            lb.AddItem(format!("{}", i), format!("Item {}", i));
        }

        // Click item 1 (sets prev_input_index)
        lb.select_by_input(1, false, false, false);
        assert_eq!(lb.GetChecked(), &[1]);

        // Shift-click item 3 (range 2..=3 selected, keeping 1)
        lb.select_by_input(3, true, false, false);
        assert!(lb.IsSelected(1));
        assert!(lb.IsSelected(2));
        assert!(lb.IsSelected(3));
    }

    #[test]
    fn toggle_mode_shift_range() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        lb.SetSelectionType(SelectionMode::Toggle);
        for i in 0..5 {
            lb.AddItem(format!("{}", i), format!("Item {}", i));
        }

        // Click item 1 (toggles on)
        lb.select_by_input(1, false, false, false);
        assert_eq!(lb.GetChecked(), &[1]);

        // Shift-click item 3 (toggle range 2..=3)
        lb.select_by_input(3, true, false, false);
        assert!(lb.IsSelected(1));
        assert!(lb.IsSelected(2));
        assert!(lb.IsSelected(3));
    }

    #[test]
    fn move_item_preserves_selection() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let mut lb = emListBox::new(&mut __init.ctx(), look);
        for i in 0..4 {
            lb.AddItem(format!("{}", i), format!("Item {}", i));
        }

        lb.Select(0, false); // "Item 0"
        lb.Select(3, false); // "Item 3"

        // Move item 0 to position 2
        lb.MoveItem(0, 2);
        // Items: 1, 2, 0, 3
        assert_eq!(lb.GetItemText(0), "Item 1");
        assert_eq!(lb.GetItemText(2), "Item 0");
        // "Item 0" (now at 2) and "Item 3" (still at 3) should be selected.
        assert!(lb.IsSelected(2));
        assert!(lb.IsSelected(3));
        assert!(!lb.IsSelected(0));
    }
}
