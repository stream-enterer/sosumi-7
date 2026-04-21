// Port of C++ emStocksListBox.h / emStocksListBox.cpp

use std::cmp::Ordering;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emDialog::{emDialog, DialogResult};
use emcore::emListBox::emListBox;
use emcore::emLook::emLook;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emRec::{parse_rec_with_format, write_rec_with_format};
use emcore::emRecRecord::Record;

use super::emStocksConfig::{emStocksConfig, Sorting};
use super::emStocksRec::{emStocksRec, CompareDates, Interest, StockRec};

/// Port of C++ emStocksListBox.
pub struct emStocksListBox {
    selected_date: String,

    // Visible items: sorted stock indices into emStocksRec.stocks
    pub visible_items: Vec<usize>,

    /// Backing emListBox for selection state. Set by the parent panel when it
    /// provides an emLook. When None, the local fallback fields below are used.
    pub(crate) list_box: Option<emListBox>,

    // Fallback selection state used when list_box is None.
    // These are kept in sync with visible_items on mutation.
    selected_indices: Vec<usize>,

    /// Current active item index (into visible_items) for find navigation.
    active_index: Option<usize>,

    /// Look used for confirmation dialogs. Set by attach_list_box.
    pub(crate) look: Option<Rc<emLook>>,

    /// Layout rect fields set by parent panel's LayoutChildren.
    /// The ListBox occupies a rectangular area within the parent panel.
    pub(crate) layout_x: f64,
    pub(crate) layout_y: f64,
    pub(crate) layout_w: f64,
    pub(crate) layout_h: f64,

    // C++ fields: CutStocksDialog, DeleteStocksDialog, PasteStocksDialog,
    // InterestDialog, InterestToSet — persistent dialog pointers polled in Cycle.
    pub(crate) cut_stocks_dialog: Option<emDialog>,
    pub(crate) paste_stocks_dialog: Option<emDialog>,
    pub(crate) delete_stocks_dialog: Option<emDialog>,
    pub(crate) interest_dialog: Option<emDialog>,
    pub(crate) interest_to_set: Option<Interest>,
}

impl Default for emStocksListBox {
    fn default() -> Self {
        Self::new()
    }
}

impl emStocksListBox {
    pub fn new() -> Self {
        Self {
            selected_date: String::new(),
            visible_items: Vec::new(),
            list_box: None,
            selected_indices: Vec::new(),
            active_index: None,
            look: None,
            layout_x: 0.0,
            layout_y: 0.0,
            layout_w: 1.0,
            layout_h: 1.0,
            cut_stocks_dialog: None,
            paste_stocks_dialog: None,
            delete_stocks_dialog: None,
            interest_dialog: None,
            interest_to_set: None,
        }
    }

    /// Attach an emListBox backed by the given look.
    /// After this call all selection operations delegate to it.
    pub fn attach_list_box<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        look: Rc<emLook>,
    ) {
        self.list_box = Some(emListBox::new(cc, look.clone()));
        self.look = Some(look);
        // Carry over any pre-existing local selection state is not attempted:
        // the attached list box starts empty and callers re-select as needed.
        self.selected_indices.clear();
    }

    // ─── Selection helpers ──────────────────────────────────────────────

    /// Port of C++ emListBox::GetSelectionCount (via GetSelectedIndices().len()).
    pub fn GetSelectionCount(&self) -> usize {
        self.GetSelectedIndices().len()
    }

    /// Port of C++ emListBox::IsSelected.
    pub fn IsSelected(&self, visible_index: usize) -> bool {
        if let Some(lb) = &self.list_box {
            lb.IsSelected(visible_index)
        } else {
            self.selected_indices.contains(&visible_index)
        }
    }

    /// Port of C++ emListBox::Select (solely=false: add to selection).
    pub fn Select(&mut self, visible_index: usize) {
        if let Some(lb) = &mut self.list_box {
            lb.Select(visible_index, false);
        } else if !self.selected_indices.contains(&visible_index) {
            self.selected_indices.push(visible_index);
        }
    }

    /// Port of C++ emListBox::ClearSelection.
    pub fn ClearSelection(&mut self) {
        if let Some(lb) = &mut self.list_box {
            lb.ClearSelection();
        } else {
            self.selected_indices.clear();
        }
    }

    /// Port of C++ emListBox::SetSelectedIndex (solely=true: clears others).
    pub fn SetSelectedIndex(&mut self, visible_index: usize) {
        if let Some(lb) = &mut self.list_box {
            lb.Select(visible_index, true);
        } else {
            self.selected_indices.clear();
            self.selected_indices.push(visible_index);
        }
    }

    /// Port of C++ emListBox::GetSelectedIndices.
    pub fn GetSelectedIndices(&self) -> &[usize] {
        if let Some(lb) = &self.list_box {
            lb.GetSelectedIndices()
        } else {
            &self.selected_indices
        }
    }

    /// Get the stock record for a visible-item index.
    fn GetStockByItemIndex<'a>(
        &self,
        visible_index: usize,
        rec: &'a emStocksRec,
    ) -> Option<&'a StockRec> {
        self.visible_items
            .get(visible_index)
            .and_then(|&stock_idx| rec.stocks.get(stock_idx))
    }

    /// Get the visible-item index for a stock by its rec index.
    fn GetItemIndexByStockIndex(&self, stock_index: usize) -> Option<usize> {
        self.visible_items
            .iter()
            .position(|&idx| idx == stock_index)
    }

    /// Port of C++ GetSelectedDate.
    pub fn GetSelectedDate(&self) -> &str {
        &self.selected_date
    }

    /// Port of C++ SetSelectedDate.
    pub fn SetSelectedDate(&mut self, date: &str) {
        self.selected_date = date.to_string();
    }

    /// Port of C++ GoBackInHistory.
    // C++ reads from owned FileModel reference. Rust passes rec explicitly — avoids shared mutable state.
    pub fn GoBackInHistory(&mut self, rec: &emStocksRec) {
        let date = rec.GetPricesDateBefore(&self.selected_date);
        if !date.is_empty() {
            self.selected_date = date;
        }
    }

    /// Port of C++ GoForwardInHistory.
    // C++ reads from owned FileModel reference. Rust passes rec explicitly — avoids shared mutable state.
    pub fn GoForwardInHistory(&mut self, rec: &emStocksRec) {
        let date = rec.GetPricesDateAfter(&self.selected_date);
        if !date.is_empty() {
            self.selected_date = date;
        }
    }

    /// Port of C++ IsVisibleStock.
    /// Checks interest level and category visibility against config.
    pub fn IsVisibleStock(stock_rec: &StockRec, config: &emStocksConfig) -> bool {
        stock_rec.interest <= config.min_visible_interest
            && emStocksConfig::IsInVisibleCategories(&config.visible_countries, &stock_rec.country)
            && emStocksConfig::IsInVisibleCategories(&config.visible_sectors, &stock_rec.sector)
            && emStocksConfig::IsInVisibleCategories(
                &config.visible_collections,
                &stock_rec.collection,
            )
    }

    /// Port of C++ UpdateItems.
    /// Rebuilds the visible_items list based on visibility and sorts them.
    pub fn UpdateItems(&mut self, rec: &emStocksRec, config: &emStocksConfig) {
        self.visible_items.clear();
        for (i, stock) in rec.stocks.iter().enumerate() {
            if Self::IsVisibleStock(stock, config) {
                self.visible_items.push(i);
            }
        }
        let selected_date = self.selected_date.clone();
        self.visible_items.sort_by(|&a, &b| {
            Self::CompareStocks(&rec.stocks[a], &rec.stocks[b], config, &selected_date)
        });
    }

    /// Port of C++ CompareItems.
    /// Sorting comparison function.
    pub fn CompareStocks(
        s1: &StockRec,
        s2: &StockRec,
        config: &emStocksConfig,
        selected_date: &str,
    ) -> Ordering {
        // 1. OwnedSharesFirst
        if config.owned_shares_first && s1.owning_shares != s2.owning_shares {
            return if s1.owning_shares {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        // 2. Sorting-specific comparison
        let (b1, f1, b2, f2) = match config.sorting {
            Sorting::ByName => (false, 0.0, false, 0.0),
            Sorting::ByTradeDate => {
                let cmp = CompareDates(&s1.trade_date, &s2.trade_date) as f64;
                (true, cmp, true, 0.0)
            }
            Sorting::ByInquiryDate => {
                let cmp = CompareDates(&s1.inquiry_date, &s2.inquiry_date) as f64;
                (true, cmp, true, 0.0)
            }
            Sorting::ByAchievement => {
                let r1 = s1.GetAchievementOfDate(selected_date, false);
                let r2 = s2.GetAchievementOfDate(selected_date, false);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByOneWeekRise => {
                let r1 = s1.GetRiseUntilDate(selected_date, 7);
                let r2 = s2.GetRiseUntilDate(selected_date, 7);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByThreeWeekRise => {
                let r1 = s1.GetRiseUntilDate(selected_date, 21);
                let r2 = s2.GetRiseUntilDate(selected_date, 21);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByNineWeekRise => {
                let r1 = s1.GetRiseUntilDate(selected_date, 63);
                let r2 = s2.GetRiseUntilDate(selected_date, 63);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByDividend => {
                let b1 = !s1.expected_dividend.is_empty();
                let f1 = if b1 {
                    s1.expected_dividend.parse::<f64>().unwrap_or(0.0)
                } else {
                    0.0
                };
                let b2 = !s2.expected_dividend.is_empty();
                let f2 = if b2 {
                    s2.expected_dividend.parse::<f64>().unwrap_or(0.0)
                } else {
                    0.0
                };
                (b1, f1, b2, f2)
            }
            Sorting::ByPurchaseValue => {
                let r1 = s1.GetTradeValue();
                let r2 = s2.GetTradeValue();
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByValue => {
                let r1 = s1.GetValueOfDate(selected_date);
                let r2 = s2.GetValueOfDate(selected_date);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByDifference => {
                let r1 = s1.GetDifferenceValueOfDate(selected_date);
                let r2 = s2.GetDifferenceValueOfDate(selected_date);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
        };

        // 3. If validity differs: valid sorts AFTER invalid (C++: b1 ? 1 : -1)
        if b1 != b2 {
            return if b1 {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }

        // 4. If both valid, compare values
        if b1 {
            let f = f1 - f2;
            if f < 0.0 {
                return Ordering::Less;
            }
            if f > 0.0 {
                return Ordering::Greater;
            }
        }

        // 5. Tiebreaker: name (case-insensitive), name (exact), ID (numeric), ID (string)
        let cmp = s1.name.to_lowercase().cmp(&s2.name.to_lowercase());
        if cmp != Ordering::Equal {
            return cmp;
        }

        let cmp = s1.name.cmp(&s2.name);
        if cmp != Ordering::Equal {
            return cmp;
        }

        let i1: i32 = s1.id.parse().unwrap_or(0);
        let i2: i32 = s2.id.parse().unwrap_or(0);
        let cmp = i1.cmp(&i2);
        if cmp != Ordering::Equal {
            return cmp;
        }

        s1.id.cmp(&s2.id)
    }

    // ─── Paint helper ───────────────────────────────────────────────────

    /// Port of C++ Paint (empty-message path).
    /// Paints the "empty stock list" message when no stocks are visible.
    pub fn PaintEmptyMessage(&self, painter: &mut emPainter, w: f64, h: f64, bg_color: emColor) {
        if self.visible_items.is_empty() {
            painter.PaintTextBoxed(
                0.0,
                0.0,
                w,
                h,
                "empty stock list",
                h * 0.1,
                emColor::rgb(255, 255, 255),
                bg_color,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.0,
                false,
                0.0,
            );
        }
    }

    // ─── Stock operations ───────────────────────────────────────────────

    /// Port of C++ NewStock.
    /// Creates a new stock, assigns an ID, sets initial fields from config,
    /// updates items, and selects the new stock.
    // C++ reads from owned FileModel/Config references. Rust passes rec and config explicitly — avoids shared mutable state.
    pub fn NewStock(&mut self, rec: &mut emStocksRec, config: &emStocksConfig) {
        let stock_index = rec.stocks.len();
        let mut stock_rec = StockRec::default();
        stock_rec.id = rec.InventStockId();
        if stock_rec.interest > config.min_visible_interest {
            stock_rec.interest = config.min_visible_interest;
        }
        if !config.visible_countries.is_empty() {
            stock_rec.country = config.visible_countries[0].clone();
        }
        if !config.visible_sectors.is_empty() {
            stock_rec.sector = config.visible_sectors[0].clone();
        }
        if !config.visible_collections.is_empty() {
            stock_rec.collection = config.visible_collections[0].clone();
        }
        rec.stocks.push(stock_rec);

        self.UpdateItems(rec, config);
        if let Some(vis_idx) = self.GetItemIndexByStockIndex(stock_index) {
            self.SetSelectedIndex(vis_idx);
        }
    }

    /// Port of C++ CopyStocks.
    /// Copies selected stocks to system clipboard.
    pub fn CopyStocks(&self, rec: &emStocksRec) {
        if self.GetSelectionCount() == 0 {
            return;
        }
        if let Some(text) = self.copy_stocks_to_string(rec) {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(&text);
            }
        }
    }

    /// Internal helper: serialize selected stocks to a string.
    /// Returns None if nothing is selected. Used by CopyStocks and tests.
    pub(crate) fn copy_stocks_to_string(&self, rec: &emStocksRec) -> Option<String> {
        if self.GetSelectionCount() == 0 {
            return None;
        }

        let mut stocks_rec = emStocksRec::default();
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(&stock_idx) = self.visible_items.get(vis_idx) {
                if let Some(stock) = rec.stocks.get(stock_idx) {
                    stocks_rec.stocks.push(stock.clone());
                }
            }
        }

        let rec_struct = stocks_rec.to_rec();
        Some(write_rec_with_format(&rec_struct, "emStocks"))
    }

    /// Port of C++ DeleteStocks.
    /// Removes selected stocks from rec.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `delete_stocks_dialog` and returns early — Cycle() polls the result.
    /// When `ask=false`, performs deletion immediately.
    /// C++ takes no arguments (reads from owned FileModel).
    /// Rust takes `rec` and `ask` parameters.
    pub fn DeleteStocks<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        ask: bool,
    ) {
        if self.GetSelectionCount() == 0 {
            return;
        }
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                if let Some(ref mut d) = self.delete_stocks_dialog {
                    d.silent_cancel();
                }
                let count = self.GetSelectionCount();
                let mut dialog = emDialog::new(
                    cc,
                    &format!("Really delete {} stock(s)?", count),
                    look.clone(),
                );
                dialog.AddCustomButton("Delete", DialogResult::Ok);
                dialog.AddCustomButton("Cancel", DialogResult::Cancel);
                self.delete_stocks_dialog = Some(dialog);
            }
            return; // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: perform deletion immediately.
        // Collect rec-level stock indices to remove, sorted descending
        let mut indices_to_remove: Vec<usize> = self
            .GetSelectedIndices()
            .iter()
            .filter_map(|&vis_idx| self.visible_items.get(vis_idx).copied())
            .collect();
        indices_to_remove.sort_unstable();
        indices_to_remove.dedup();
        // Remove from end to preserve earlier indices
        for &idx in indices_to_remove.iter().rev() {
            rec.stocks.remove(idx);
        }

        self.ClearSelection();
    }

    /// Port of C++ CutStocks.
    /// Cuts selected stocks to clipboard.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `cut_stocks_dialog` and returns early — Cycle() polls the result.
    /// When `ask=false`, performs copy+delete immediately.
    /// C++ takes no arguments. Rust takes `rec` and `ask` parameters.
    pub fn CutStocks<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        ask: bool,
    ) {
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                if let Some(ref mut d) = self.cut_stocks_dialog {
                    d.silent_cancel();
                }
                let count = self.GetSelectionCount();
                let mut dialog =
                    emDialog::new(cc, &format!("Really cut {} stock(s)?", count), look.clone());
                dialog.AddCustomButton("Cut", DialogResult::Ok);
                dialog.AddCustomButton("Cancel", DialogResult::Cancel);
                self.cut_stocks_dialog = Some(dialog);
            }
            return; // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: perform cut immediately.
        self.CopyStocks(rec);
        if self.GetSelectionCount() > 0 {
            self.DeleteStocks(cc, rec, false); // inner delete doesn't ask again
        }
    }

    /// Port of C++ PasteStocks.
    /// Pastes stocks from clipboard.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `paste_stocks_dialog` and returns early — Cycle() polls the result.
    /// When `ask=false`, reads clipboard and inserts stocks immediately.
    /// Returns names of pasted stocks that are not visible due to filters,
    /// or an error if the clipboard data is invalid or clipboard is empty.
    /// C++ takes no arguments. Rust takes `rec`, `config`, and `ask` parameters.
    pub fn PasteStocks<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
        ask: bool,
    ) -> Result<Vec<String>, String> {
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                if let Some(ref mut d) = self.paste_stocks_dialog {
                    d.silent_cancel();
                }
                let mut dialog = emDialog::new(cc, "Really paste stocks?", look.clone());
                dialog.AddCustomButton("Paste", DialogResult::Ok);
                dialog.AddCustomButton("Cancel", DialogResult::Cancel);
                self.paste_stocks_dialog = Some(dialog);
            }
            return Ok(Vec::new()); // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: perform paste immediately.
        let clipboard_text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
            clipboard.get_text().unwrap_or_default()
        } else {
            return Err("Cannot access clipboard".to_string());
        };
        if clipboard_text.is_empty() {
            return Err("Clipboard is empty".to_string());
        }
        self.paste_stocks_from_text(rec, config, &clipboard_text)
    }

    /// Internal helper: parse stocks from a text string and insert into rec.
    /// Used by PasteStocks (public) and tests.
    pub(crate) fn paste_stocks_from_text(
        &mut self,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
        clipboard_text: &str,
    ) -> Result<Vec<String>, String> {
        let parsed = parse_rec_with_format(clipboard_text, "emStocks")
            .map_err(|e| format!("No valid stocks in clipboard ({e})"))?;
        let stocks_rec = emStocksRec::from_rec(&parsed)
            .map_err(|e| format!("No valid stocks in clipboard ({e})"))?;

        let n = rec.stocks.len();
        let m = stocks_rec.stocks.len();
        let mut invisible_names = Vec::new();

        for mut stock in stocks_rec.stocks {
            if rec.GetStockIndexById(&stock.id).is_some() {
                stock.id = rec.InventStockId();
            }
            if !Self::IsVisibleStock(&stock, config) {
                invisible_names.push(stock.name.clone());
            }
            rec.stocks.push(stock);
        }

        self.UpdateItems(rec, config);
        self.ClearSelection();
        for i in n..(n + m) {
            if let Some(vis_idx) = self.GetItemIndexByStockIndex(i) {
                self.Select(vis_idx);
            }
        }

        Ok(invisible_names)
    }

    /// Port of C++ DeleteSharePrices.
    /// Clears price data from all visible stocks.
    // C++ reads from owned FileModel reference. Rust passes rec explicitly — avoids shared mutable state.
    pub fn DeleteSharePrices(&self, rec: &mut emStocksRec) {
        for &stock_idx in &self.visible_items {
            if let Some(stock) = rec.stocks.get_mut(stock_idx) {
                stock.prices.clear();
                stock.last_price_date.clear();
            }
        }
    }

    /// Port of C++ SetInterest.
    /// Sets interest level on selected stocks.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `interest_dialog` and `interest_to_set`, then returns early — Cycle()
    /// polls the result.
    /// When `ask=false`, applies the interest change immediately.
    /// C++ takes no arguments beyond interest. Rust takes `rec` and `ask` parameters.
    pub fn SetInterest<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        interest: Interest,
        ask: bool,
    ) {
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                if let Some(ref mut d) = self.interest_dialog {
                    d.silent_cancel();
                }
                let mut dialog = emDialog::new(cc, "Really change interest?", look.clone());
                dialog.AddCustomButton("Change", DialogResult::Ok);
                dialog.AddCustomButton("Cancel", DialogResult::Cancel);
                self.interest_dialog = Some(dialog);
                self.interest_to_set = Some(interest);
            }
            return; // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: apply immediately.
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(&stock_idx) = self.visible_items.get(vis_idx) {
                if let Some(stock) = rec.stocks.get_mut(stock_idx) {
                    stock.interest = interest;
                }
            }
        }
    }

    /// Port of C++ Cycle (dialog polling portion).
    /// Polls persistent confirmation dialogs and executes deferred operations
    /// when the user confirms. Returns true while any dialog is still open
    /// (signals the parent panel to keep cycling).
    pub fn Cycle<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
    ) -> bool {
        let mut busy = false;

        // Poll delete dialog.
        if let Some(result) = self
            .delete_stocks_dialog
            .as_ref()
            .and_then(|d| d.GetResult())
        {
            let confirmed = *result == DialogResult::Ok;
            self.delete_stocks_dialog = None;
            if confirmed {
                self.DeleteStocks(cc, rec, false);
            }
        } else if self.delete_stocks_dialog.is_some() {
            busy = true;
        }

        // Poll cut dialog.
        if let Some(result) = self.cut_stocks_dialog.as_ref().and_then(|d| d.GetResult()) {
            let confirmed = *result == DialogResult::Ok;
            self.cut_stocks_dialog = None;
            if confirmed {
                self.CutStocks(cc, rec, false);
            }
        } else if self.cut_stocks_dialog.is_some() {
            busy = true;
        }

        // Poll paste dialog.
        if let Some(result) = self
            .paste_stocks_dialog
            .as_ref()
            .and_then(|d| d.GetResult())
        {
            let confirmed = *result == DialogResult::Ok;
            self.paste_stocks_dialog = None;
            if confirmed {
                let _ = self.PasteStocks(cc, rec, config, false);
            }
        } else if self.paste_stocks_dialog.is_some() {
            busy = true;
        }

        // Poll interest dialog.
        if let Some(result) = self.interest_dialog.as_ref().and_then(|d| d.GetResult()) {
            let confirmed = *result == DialogResult::Ok;
            self.interest_dialog = None;
            if confirmed {
                if let Some(interest) = self.interest_to_set.take() {
                    self.SetInterest(cc, rec, interest, false);
                }
            } else {
                self.interest_to_set = None;
            }
        } else if self.interest_dialog.is_some() {
            busy = true;
        }

        busy
    }

    /// Port of C++ ShowFirstWebPages.
    /// Opens the first web page for each selected stock in the system browser.
    pub fn ShowFirstWebPages(&self, rec: &emStocksRec) {
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(stock) = self.GetStockByItemIndex(vis_idx, rec) {
                if let Some(page) = stock.web_pages.first() {
                    if !page.is_empty() {
                        let _ = open::that(page);
                    }
                }
            }
        }
    }

    /// Port of C++ ShowAllWebPages.
    /// Opens all web pages for each selected stock in the system browser.
    pub fn ShowAllWebPages(&self, rec: &emStocksRec) {
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(stock) = self.GetStockByItemIndex(vis_idx, rec) {
                for page in &stock.web_pages {
                    if !page.is_empty() {
                        let _ = open::that(page);
                    }
                }
            }
        }
    }

    /// Port of C++ StartToFetchSharePrices (no-args overload).
    /// Returns visible stock IDs. The caller (FilePanel) creates the fetch dialog.
    pub fn GetVisibleStockIds(&self, rec: &emStocksRec) -> Vec<String> {
        self.visible_items
            .iter()
            .filter_map(|&idx| rec.stocks.get(idx).map(|s| s.id.clone()))
            .collect()
    }

    // ─── Find operations ────────────────────────────────────────────────

    /// Port of C++ FindSelected.
    /// Sets search text from clipboard (falling back to config.search_text) and calls FindNext.
    pub fn FindSelected(
        &mut self,
        rec: &emStocksRec,
        config: &mut emStocksConfig,
    ) -> Option<usize> {
        let text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
            clipboard
                .get_text()
                .unwrap_or_else(|_| config.search_text.clone())
        } else {
            config.search_text.clone()
        };
        if text.is_empty() {
            return None;
        }
        config.search_text = text;
        self.FindNext(rec, config)
    }

    /// Port of C++ FindNext.
    /// Searches forward from active item, wrapping around.
    /// Returns the visible-item index of the found stock, or None.
    /// C++ navigates view to found panel. Rust returns the index for the caller to handle.
    pub fn FindNext(&mut self, rec: &emStocksRec, config: &emStocksConfig) -> Option<usize> {
        let count = self.visible_items.len();
        if count == 0 {
            return None;
        }

        let start = self.active_index.unwrap_or(count - 1);
        let mut j = start;
        loop {
            j = (j + 1) % count;
            if let Some(stock) = self.GetStockByItemIndex(j, rec) {
                if stock.IsMatchingSearchText(&config.search_text) {
                    self.active_index = Some(j);
                    return Some(j);
                }
            }
            if j == start {
                return None;
            }
        }
    }

    /// Port of C++ FindPrevious.
    /// Searches backward from active item, wrapping around.
    /// Returns the visible-item index of the found stock, or None.
    /// C++ navigates view to found panel. Rust returns the index for the caller to handle.
    pub fn FindPrevious(&mut self, rec: &emStocksRec, config: &emStocksConfig) -> Option<usize> {
        let count = self.visible_items.len();
        if count == 0 {
            return None;
        }

        let start = self.active_index.unwrap_or(0);
        let mut j = start;
        loop {
            j = (j + count - 1) % count;
            if let Some(stock) = self.GetStockByItemIndex(j, rec) {
                if stock.IsMatchingSearchText(&config.search_text) {
                    self.active_index = Some(j);
                    return Some(j);
                }
            }
            if j == start {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::{DeferredAction, InitCtx};
    use emcore::emScheduler::EngineScheduler;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<emcore::emContext::emContext>,
        pa: Rc<std::cell::RefCell<Vec<emcore::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: emcore::emContext::emContext::NewRoot(),
                pa: Rc::new(std::cell::RefCell::new(Vec::new())),
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

    fn make_stock(id: &str, name: &str, interest: Interest) -> StockRec {
        let mut s = StockRec::default();
        s.id = id.to_string();
        s.name = name.to_string();
        s.interest = interest;
        s
    }

    #[test]
    fn listbox_new() {
        let mut __init = TestInit::new();
        let lb = emStocksListBox::new();
        assert!(lb.visible_items.is_empty());
    }

    #[test]
    fn is_visible_stock_interest_filter() {
        let mut __init = TestInit::new();
        let config = emStocksConfig {
            min_visible_interest: Interest::Medium,
            ..Default::default()
        };
        let high = make_stock("1", "High", Interest::High);
        let medium = make_stock("2", "Medium", Interest::Medium);
        let low = make_stock("3", "Low", Interest::Low);

        assert!(emStocksListBox::IsVisibleStock(&high, &config)); // High(0) <= Medium(1)
        assert!(emStocksListBox::IsVisibleStock(&medium, &config)); // Medium(1) <= Medium(1)
        assert!(!emStocksListBox::IsVisibleStock(&low, &config)); // Low(2) > Medium(1)
    }

    #[test]
    fn is_visible_stock_category_filter() {
        let mut __init = TestInit::new();
        let config = emStocksConfig {
            visible_countries: vec!["DE".to_string(), "US".to_string()],
            ..Default::default()
        };
        let mut us_stock = make_stock("1", "US Stock", Interest::High);
        us_stock.country = "US".to_string();
        let mut jp_stock = make_stock("2", "JP Stock", Interest::High);
        jp_stock.country = "JP".to_string();

        assert!(emStocksListBox::IsVisibleStock(&us_stock, &config));
        assert!(!emStocksListBox::IsVisibleStock(&jp_stock, &config));
    }

    #[test]
    fn is_visible_stock_empty_categories_means_all() {
        let mut __init = TestInit::new();
        let config = emStocksConfig::default(); // empty visible_countries
        let mut stock = make_stock("1", "Any", Interest::High);
        stock.country = "Anywhere".to_string();
        assert!(emStocksListBox::IsVisibleStock(&stock, &config));
    }

    #[test]
    fn update_items_filters_and_sorts() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Zebra Corp", Interest::High));
        rec.stocks
            .push(make_stock("2", "Alpha Inc", Interest::High));
        rec.stocks.push(make_stock("3", "Hidden", Interest::Low));

        let config = emStocksConfig {
            min_visible_interest: Interest::Medium,
            sorting: Sorting::ByName,
            ..Default::default()
        };

        let mut lb = emStocksListBox::new();
        lb.UpdateItems(&rec, &config);

        // "Hidden" (Low interest) filtered out, remaining sorted by name
        assert_eq!(lb.visible_items.len(), 2);
        // Alpha should come before Zebra
        assert_eq!(rec.stocks[lb.visible_items[0]].name, "Alpha Inc");
        assert_eq!(rec.stocks[lb.visible_items[1]].name, "Zebra Corp");
    }

    #[test]
    fn compare_stocks_owned_first() {
        let mut __init = TestInit::new();
        let mut s1 = make_stock("1", "A", Interest::High);
        s1.owning_shares = true;
        let s2 = make_stock("2", "B", Interest::High);

        let config = emStocksConfig {
            owned_shares_first: true,
            ..Default::default()
        };

        let ord = emStocksListBox::CompareStocks(&s1, &s2, &config, "2024-06-15");
        assert_eq!(ord, Ordering::Less); // owned first
    }

    #[test]
    fn selected_date_management() {
        let mut __init = TestInit::new();
        let mut lb = emStocksListBox::new();
        lb.SetSelectedDate("2024-06-15");
        assert_eq!(lb.GetSelectedDate(), "2024-06-15");
    }

    #[test]
    fn go_back_in_history() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut stock = StockRec::default();
        stock.AddPrice("2024-06-14", "100");
        stock.AddPrice("2024-06-15", "101");
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        lb.SetSelectedDate("2024-06-15");
        lb.GoBackInHistory(&rec);
        assert_eq!(lb.GetSelectedDate(), "2024-06-14");
    }

    // ─── New tests for stock operations ──────────────────────────────────

    #[test]
    fn paint_empty_message_when_no_items() {
        let mut __init = TestInit::new();
        use emcore::emImage::emImage;
        let lb = emStocksListBox::new();
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        // Smoke test: no panic when painting with empty visible_items
        lb.PaintEmptyMessage(&mut painter, 200.0, 50.0, emColor::rgb(0, 0, 0));
    }

    #[test]
    fn paint_empty_message_no_op_when_items_exist() {
        let mut __init = TestInit::new();
        use emcore::emImage::emImage;
        let mut lb = emStocksListBox::new();
        lb.visible_items.push(0);
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        // Smoke test: no panic when items exist
        lb.PaintEmptyMessage(&mut painter, 200.0, 50.0, emColor::rgb(0, 0, 0));
    }

    #[test]
    fn new_stock_assigns_id_and_selects() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Existing", Interest::High));
        let config = emStocksConfig::default();

        let mut lb = emStocksListBox::new();
        lb.NewStock(&mut rec, &config);

        assert_eq!(rec.stocks.len(), 2);
        assert_eq!(rec.stocks[1].id, "2"); // InventStockId returns max+1
        assert_eq!(lb.GetSelectionCount(), 1);
    }

    #[test]
    fn new_stock_inherits_config_categories() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let config = emStocksConfig {
            visible_countries: vec!["DE".to_string()],
            visible_sectors: vec!["Tech".to_string()],
            visible_collections: vec!["Growth".to_string()],
            ..Default::default()
        };

        let mut lb = emStocksListBox::new();
        lb.NewStock(&mut rec, &config);

        assert_eq!(rec.stocks[0].country, "DE");
        assert_eq!(rec.stocks[0].sector, "Tech");
        assert_eq!(rec.stocks[0].collection, "Growth");
    }

    #[test]
    fn new_stock_caps_interest_to_config() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let config = emStocksConfig {
            min_visible_interest: Interest::High,
            ..Default::default()
        };

        let mut lb = emStocksListBox::new();
        lb.NewStock(&mut rec, &config);

        // StockRec default interest is Medium (1), High is (0), so Medium > High
        // means interest should be capped to High
        assert_eq!(rec.stocks[0].interest, Interest::High);
    }

    #[test]
    fn copy_stocks_to_string_empty_selection_returns_none() {
        let mut __init = TestInit::new();
        let rec = emStocksRec::default();
        let lb = emStocksListBox::new();
        assert!(lb.copy_stocks_to_string(&rec).is_none());
    }

    #[test]
    fn copy_stocks_to_string_serializes_selected() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0); // select first visible item

        let text = lb.copy_stocks_to_string(&rec);
        assert!(text.is_some());
        assert!(text.unwrap().contains("emStocks"));
    }

    #[test]
    fn delete_stocks_removes_selected() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));
        rec.stocks.push(make_stock("3", "Gamma", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(1); // select second visible item (Beta)

        lb.DeleteStocks(&mut __init.ctx(), &mut rec, false);
        assert_eq!(rec.stocks.len(), 2);
        assert_eq!(rec.stocks[0].name, "Alpha");
        assert_eq!(rec.stocks[1].name, "Gamma");
        assert_eq!(lb.GetSelectionCount(), 0);
    }

    #[test]
    fn delete_stocks_empty_selection_is_noop() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        lb.DeleteStocks(&mut __init.ctx(), &mut rec, false);
        assert_eq!(rec.stocks.len(), 1);
    }

    #[test]
    fn cut_stocks_copies_then_deletes() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0); // select Alpha

        lb.CutStocks(&mut __init.ctx(), &mut rec, false);
        assert_eq!(rec.stocks.len(), 1);
        assert_eq!(rec.stocks[0].name, "Beta");
    }

    #[test]
    fn paste_stocks_round_trip() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0);

        // Copy, then paste into a fresh rec
        let clipboard = lb.copy_stocks_to_string(&rec).unwrap();

        let mut rec2 = emStocksRec::default();
        let mut lb2 = emStocksListBox::new();
        let result = lb2.paste_stocks_from_text(&mut rec2, &config, &clipboard);
        assert!(result.is_ok());
        assert_eq!(rec2.stocks.len(), 1);
        assert_eq!(rec2.stocks[0].name, "Alpha");
    }

    #[test]
    fn paste_stocks_reassigns_conflicting_ids() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Existing", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        // Create clipboard with a stock that has id "1" (conflicts)
        let mut source_rec = emStocksRec::default();
        source_rec
            .stocks
            .push(make_stock("1", "Pasted", Interest::High));
        let rec_struct = source_rec.to_rec();
        let clipboard = write_rec_with_format(&rec_struct, "emStocks");

        let result = lb.paste_stocks_from_text(&mut rec, &config, &clipboard);
        assert!(result.is_ok());
        assert_eq!(rec.stocks.len(), 2);
        // The pasted stock should have a new ID, not "1"
        assert_ne!(rec.stocks[1].id, "1");
        assert_eq!(rec.stocks[1].name, "Pasted");
    }

    #[test]
    fn paste_stocks_invalid_data_returns_error() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        let result = lb.paste_stocks_from_text(&mut rec, &config, "not valid data");
        assert!(result.is_err());
    }

    #[test]
    fn paste_stocks_reports_invisible() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let config = emStocksConfig {
            min_visible_interest: Interest::High,
            ..Default::default()
        };
        let mut lb = emStocksListBox::new();

        // Create clipboard with a Low-interest stock
        let mut source_rec = emStocksRec::default();
        source_rec
            .stocks
            .push(make_stock("1", "Hidden", Interest::Low));
        let rec_struct = source_rec.to_rec();
        let clipboard = write_rec_with_format(&rec_struct, "emStocks");

        let result = lb.paste_stocks_from_text(&mut rec, &config, &clipboard);
        assert!(result.is_ok());
        let invisible = result.unwrap();
        assert_eq!(invisible, vec!["Hidden".to_string()]);
    }

    #[test]
    fn delete_share_prices_clears_visible() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut stock = make_stock("1", "Alpha", Interest::High);
        stock.AddPrice("2024-06-15", "100");
        stock.last_price_date = "2024-06-15".to_string();
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);

        lb.DeleteSharePrices(&mut rec);
        assert!(rec.stocks[0].prices.is_empty());
        assert!(rec.stocks[0].last_price_date.is_empty());
    }

    #[test]
    fn set_interest_on_selected() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::Medium));
        rec.stocks.push(make_stock("2", "Beta", Interest::Medium));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0);

        lb.SetInterest(&mut __init.ctx(), &mut rec, Interest::High, false);
        // Only the selected stock should change
        assert_eq!(rec.stocks[lb.visible_items[0]].interest, Interest::High);
        assert_eq!(rec.stocks[lb.visible_items[1]].interest, Interest::Medium);
    }

    #[test]
    fn show_first_web_pages_empty_selection_is_noop() {
        let mut __init = TestInit::new();
        // ShowFirstWebPages with no selection launches no browser and does not panic.
        let mut rec = emStocksRec::default();
        let mut stock = make_stock("1", "Alpha", Interest::High);
        stock.web_pages = vec!["https://example.com".to_string()];
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        // No Select() call — selection is empty.
        lb.ShowFirstWebPages(&rec); // should be a no-op
    }

    #[test]
    fn show_all_web_pages_empty_selection_is_noop() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut stock = make_stock("1", "Alpha", Interest::High);
        stock.web_pages = vec![
            "https://example.com".to_string(),
            "https://other.com".to_string(),
        ];
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        // No Select() call — selection is empty.
        lb.ShowAllWebPages(&rec); // should be a no-op
    }

    #[test]
    fn get_visible_stock_ids() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);

        let ids = lb.GetVisibleStockIds(&rec);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn find_next_wraps_around() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Alpha Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Beta Inc", Interest::High));
        rec.stocks
            .push(make_stock("3", "Alpha Ltd", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig {
            search_text: "Alpha".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        // First find should find "Alpha Corp" (index 0)
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Corp");

        // Second find should find "Alpha Ltd" (index 2)
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Ltd");

        // Third find should wrap back to "Alpha Corp"
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Corp");
    }

    #[test]
    fn find_previous_wraps_around() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Alpha Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Beta Inc", Interest::High));
        rec.stocks
            .push(make_stock("3", "Alpha Ltd", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig {
            search_text: "Alpha".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        // FindPrevious from default (start=0) should go backwards and find Alpha Ltd
        let result = lb.FindPrevious(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Ltd");
    }

    #[test]
    fn find_next_returns_none_when_no_match() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig {
            search_text: "Nonexistent".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        assert!(lb.FindNext(&rec, &config).is_none());
    }

    #[test]
    fn find_next_returns_none_on_empty_list() {
        let mut __init = TestInit::new();
        let rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        assert!(lb.FindNext(&rec, &config).is_none());
    }

    #[test]
    fn find_selected_uses_config_search_text_when_clipboard_unavailable() {
        let mut __init = TestInit::new();
        // FindSelected reads clipboard; if clipboard is unavailable it falls
        // back to config.search_text.  Pre-set search_text so the fallback
        // exercises the FindNext path.
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Alpha Corp", Interest::High));

        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig {
            search_text: "Alpha".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        // The result depends on whether the clipboard is accessible in the
        // test environment, so we only check that it does not panic and that
        // config.search_text is non-empty after the call.
        let _ = lb.FindSelected(&rec, &mut config);
        assert!(!config.search_text.is_empty());
    }

    #[test]
    fn find_selected_empty_fallback_returns_none() {
        let mut __init = TestInit::new();
        // When both clipboard and config.search_text are empty, FindSelected
        // returns None.
        let rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();
        // config.search_text is "" and clipboard is expected to be empty/unavailable
        // in the headless test environment.
        // We can only verify no panic; None is the expected result when text is empty.
        let _ = lb.FindSelected(&rec, &mut config);
    }

    #[test]
    fn selection_helpers() {
        let mut __init = TestInit::new();
        let mut lb = emStocksListBox::new();
        lb.visible_items = vec![0, 1, 2];

        assert_eq!(lb.GetSelectionCount(), 0);
        assert!(!lb.IsSelected(0));

        lb.Select(1);
        assert_eq!(lb.GetSelectionCount(), 1);
        assert!(lb.IsSelected(1));
        assert!(!lb.IsSelected(0));

        lb.Select(1); // duplicate select is idempotent
        assert_eq!(lb.GetSelectionCount(), 1);

        lb.SetSelectedIndex(2);
        assert_eq!(lb.GetSelectionCount(), 1);
        assert!(lb.IsSelected(2));
        assert!(!lb.IsSelected(1));

        lb.ClearSelection();
        assert_eq!(lb.GetSelectionCount(), 0);
    }
}
