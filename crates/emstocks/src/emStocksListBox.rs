// Port of C++ emStocksListBox.h / emStocksListBox.cpp

use std::cmp::Ordering;

use emcore::emRec::{parse_rec_with_format, write_rec_with_format};
use emcore::emRecRecord::Record;

use super::emStocksConfig::{emStocksConfig, Sorting};
use super::emStocksRec::{emStocksRec, CompareDates, Interest, StockRec};

/// Port of C++ emStocksListBox.
/// DIVERGED: Data model, sorting/filtering, and stock operations only — widget
/// and panel infrastructure deferred until panel framework integration.
pub struct emStocksListBox {
    selected_date: String,

    // Visible items: sorted stock indices into emStocksRec.stocks
    pub visible_items: Vec<usize>,

    // Selected visible item indices (indices into visible_items)
    /// DIVERGED: C++ uses emListBox selection API. Rust tracks selection locally
    /// since emListBox widget is not yet integrated.
    pub selected_indices: Vec<usize>,

    /// Current active item index (into visible_items) for find navigation.
    /// DIVERGED: C++ uses panel active-path. Rust tracks locally.
    pub active_index: Option<usize>,
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
            selected_indices: Vec::new(),
            active_index: None,
        }
    }

    // ─── Selection helpers ──────────────────────────────────────────────

    /// Number of currently selected items.
    /// DIVERGED: C++ uses emListBox::GetSelectionCount(). Rust tracks locally.
    pub fn GetSelectionCount(&self) -> usize {
        self.selected_indices.len()
    }

    /// Whether the given visible-item index is selected.
    /// DIVERGED: C++ uses emListBox::IsSelected(). Rust tracks locally.
    pub fn IsSelected(&self, visible_index: usize) -> bool {
        self.selected_indices.contains(&visible_index)
    }

    /// Select a visible-item index.
    /// DIVERGED: C++ uses emListBox::Select(). Rust tracks locally.
    pub fn Select(&mut self, visible_index: usize) {
        if !self.selected_indices.contains(&visible_index) {
            self.selected_indices.push(visible_index);
        }
    }

    /// Clear all selections.
    /// DIVERGED: C++ uses emListBox::ClearSelection(). Rust tracks locally.
    pub fn ClearSelection(&mut self) {
        self.selected_indices.clear();
    }

    /// Set a single item as selected (clears previous selection).
    /// DIVERGED: C++ uses emListBox::SetSelectedIndex(). Rust tracks locally.
    pub fn SetSelectedIndex(&mut self, visible_index: usize) {
        self.selected_indices.clear();
        self.selected_indices.push(visible_index);
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
    /// DIVERGED: C++ takes no arguments (reads from owned FileModel reference).
    /// Rust takes `rec` parameter since emStocksFileModel is not yet integrated.
    pub fn GoBackInHistory(&mut self, rec: &emStocksRec) {
        let date = rec.GetPricesDateBefore(&self.selected_date);
        if !date.is_empty() {
            self.selected_date = date;
        }
    }

    /// Port of C++ GoForwardInHistory.
    /// DIVERGED: C++ takes no arguments (reads from owned FileModel reference).
    /// Rust takes `rec` parameter since emStocksFileModel is not yet integrated.
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
            && emStocksConfig::IsInVisibleCategories(
                &config.visible_countries,
                &stock_rec.country,
            )
            && emStocksConfig::IsInVisibleCategories(
                &config.visible_sectors,
                &stock_rec.sector,
            )
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
                let cmp =
                    CompareDates(&s1.trade_date, &s2.trade_date) as f64;
                (true, cmp, true, 0.0)
            }
            Sorting::ByInquiryDate => {
                let cmp = CompareDates(&s1.inquiry_date, &s2.inquiry_date)
                    as f64;
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

    /// Port of C++ Paint.
    /// Returns the "empty stock list" message text when item count is zero,
    /// or None when there are items to display.
    /// DIVERGED: C++ calls emPainter::PaintTextBoxed. Rust returns the text
    /// since the painter infrastructure is deferred.
    pub fn GetEmptyMessage(&self) -> Option<&'static str> {
        if self.visible_items.is_empty() {
            Some("empty stock list")
        } else {
            None
        }
    }

    // ─── Stock operations ───────────────────────────────────────────────

    /// Port of C++ NewStock.
    /// Creates a new stock, assigns an ID, sets initial fields from config,
    /// updates items, and selects the new stock.
    /// DIVERGED: C++ takes no arguments (reads from owned FileModel/Config).
    /// Rust takes `rec` and `config` parameters.
    pub fn NewStock(
        &mut self,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
    ) {
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
    /// Serializes selected stocks to emStocksRec format string.
    /// Returns the serialized string, or None if nothing is selected.
    /// DIVERGED: C++ copies to system clipboard. Rust returns the string
    /// since clipboard integration is deferred.
    pub fn CopyStocks(&self, rec: &emStocksRec) -> Option<String> {
        if self.GetSelectionCount() == 0 {
            return None;
        }

        let mut stocks_rec = emStocksRec::default();
        for &vis_idx in &self.selected_indices {
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
    /// DIVERGED: C++ has `ask` parameter for dialog confirmation. Rust
    /// performs the operation directly since dialog system is deferred.
    /// C++ takes no arguments (reads from owned FileModel).
    /// Rust takes `rec` parameter.
    pub fn DeleteStocks(&mut self, rec: &mut emStocksRec) {
        if self.GetSelectionCount() == 0 {
            return;
        }

        // Collect rec-level stock indices to remove, sorted descending
        let mut indices_to_remove: Vec<usize> = self
            .selected_indices
            .iter()
            .filter_map(|&vis_idx| self.visible_items.get(vis_idx).copied())
            .collect();
        indices_to_remove.sort_unstable();
        indices_to_remove.dedup();
        // Remove from end to preserve earlier indices
        for &idx in indices_to_remove.iter().rev() {
            rec.stocks.remove(idx);
        }

        self.selected_indices.clear();
    }

    /// Port of C++ CutStocks.
    /// Copies selected stocks, then deletes them.
    /// DIVERGED: C++ has `ask` parameter for dialog confirmation. Rust
    /// performs the operation directly since dialog system is deferred.
    /// C++ takes no arguments. Rust takes `rec` parameter and returns
    /// the serialized clipboard string.
    pub fn CutStocks(&mut self, rec: &mut emStocksRec) -> Option<String> {
        let clipboard = self.CopyStocks(rec);
        if clipboard.is_some() {
            self.DeleteStocks(rec);
        }
        clipboard
    }

    /// Port of C++ PasteStocks.
    /// Deserializes stocks from clipboard format, assigns new IDs where
    /// conflicts exist, adds to rec.
    /// Returns names of pasted stocks that are not visible due to filters,
    /// or an error if the clipboard data is invalid.
    /// DIVERGED: C++ reads from system clipboard and has `ask` dialog.
    /// Rust takes clipboard text as parameter and performs directly.
    /// C++ takes no arguments.
    pub fn PasteStocks(
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
    /// DIVERGED: C++ takes no arguments (reads from owned FileModel).
    /// Rust takes `rec` parameter.
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
    /// DIVERGED: C++ has `ask` parameter for dialog confirmation. Rust
    /// performs directly. C++ takes no arguments beyond interest.
    /// Rust takes `rec` parameter.
    pub fn SetInterest(
        &self,
        rec: &mut emStocksRec,
        interest: Interest,
    ) {
        for &vis_idx in &self.selected_indices {
            if let Some(&stock_idx) = self.visible_items.get(vis_idx) {
                if let Some(stock) = rec.stocks.get_mut(stock_idx) {
                    stock.interest = interest;
                }
            }
        }
    }

    /// Port of C++ ShowFirstWebPages.
    /// Collects the first web page URL from each selected stock.
    /// DIVERGED: C++ launches web browser. Rust returns URLs since
    /// process launch is deferred.
    pub fn ShowFirstWebPages(&self, rec: &emStocksRec) -> Vec<String> {
        let mut pages = Vec::new();
        for &vis_idx in &self.selected_indices {
            if let Some(stock) = self.GetStockByItemIndex(vis_idx, rec) {
                if let Some(page) = stock.web_pages.first() {
                    if !page.is_empty() {
                        pages.push(page.clone());
                    }
                }
            }
        }
        pages
    }

    /// Port of C++ ShowAllWebPages.
    /// Collects all web page URLs from selected stocks.
    /// DIVERGED: C++ launches web browser. Rust returns URLs since
    /// process launch is deferred.
    pub fn ShowAllWebPages(&self, rec: &emStocksRec) -> Vec<String> {
        let mut pages = Vec::new();
        for &vis_idx in &self.selected_indices {
            if let Some(stock) = self.GetStockByItemIndex(vis_idx, rec) {
                for page in &stock.web_pages {
                    if !page.is_empty() {
                        pages.push(page.clone());
                    }
                }
            }
        }
        pages
    }

    /// Port of C++ StartToFetchSharePrices (no-args overload).
    /// Collects stock IDs of all visible items.
    /// DIVERGED: C++ creates the fetch dialog. Rust returns the IDs since
    /// dialog/process integration is deferred.
    pub fn GetVisibleStockIds(&self, rec: &emStocksRec) -> Vec<String> {
        self.visible_items
            .iter()
            .filter_map(|&idx| rec.stocks.get(idx).map(|s| s.id.clone()))
            .collect()
    }

    // ─── Find operations ────────────────────────────────────────────────

    /// Port of C++ FindSelected.
    /// Sets search text and calls FindNext.
    /// DIVERGED: C++ reads from system clipboard. Rust takes search text
    /// as parameter. C++ modifies Config.SearchText; Rust takes config
    /// mutably.
    pub fn FindSelected(
        &mut self,
        rec: &emStocksRec,
        config: &mut emStocksConfig,
        search_text: &str,
    ) -> Option<usize> {
        if search_text.is_empty() {
            return None;
        }
        config.search_text = search_text.to_string();
        self.FindNext(rec, config)
    }

    /// Port of C++ FindNext.
    /// Searches forward from active item, wrapping around.
    /// Returns the visible-item index of the found stock, or None.
    /// DIVERGED: C++ navigates view to found panel. Rust returns index
    /// since view navigation is deferred.
    pub fn FindNext(
        &mut self,
        rec: &emStocksRec,
        config: &emStocksConfig,
    ) -> Option<usize> {
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
    /// DIVERGED: C++ navigates view to found panel. Rust returns index
    /// since view navigation is deferred.
    pub fn FindPrevious(
        &mut self,
        rec: &emStocksRec,
        config: &emStocksConfig,
    ) -> Option<usize> {
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

    fn make_stock(id: &str, name: &str, interest: Interest) -> StockRec {
        let mut s = StockRec::default();
        s.id = id.to_string();
        s.name = name.to_string();
        s.interest = interest;
        s
    }

    #[test]
    fn listbox_new() {
        let lb = emStocksListBox::new();
        assert!(lb.visible_items.is_empty());
    }

    #[test]
    fn is_visible_stock_interest_filter() {
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
        let config = emStocksConfig::default(); // empty visible_countries
        let mut stock = make_stock("1", "Any", Interest::High);
        stock.country = "Anywhere".to_string();
        assert!(emStocksListBox::IsVisibleStock(&stock, &config));
    }

    #[test]
    fn update_items_filters_and_sorts() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Zebra Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Alpha Inc", Interest::High));
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
        let mut lb = emStocksListBox::new();
        lb.SetSelectedDate("2024-06-15");
        assert_eq!(lb.GetSelectedDate(), "2024-06-15");
    }

    #[test]
    fn go_back_in_history() {
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
    fn empty_message_when_no_items() {
        let lb = emStocksListBox::new();
        assert_eq!(lb.GetEmptyMessage(), Some("empty stock list"));
    }

    #[test]
    fn no_empty_message_when_items_exist() {
        let mut lb = emStocksListBox::new();
        lb.visible_items.push(0);
        assert_eq!(lb.GetEmptyMessage(), None);
    }

    #[test]
    fn new_stock_assigns_id_and_selects() {
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
    fn copy_stocks_empty_selection_returns_none() {
        let rec = emStocksRec::default();
        let lb = emStocksListBox::new();
        assert!(lb.CopyStocks(&rec).is_none());
    }

    #[test]
    fn copy_stocks_serializes_selected() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0); // select first visible item

        let clipboard = lb.CopyStocks(&rec);
        assert!(clipboard.is_some());
        let text = clipboard.unwrap();
        assert!(text.contains("emStocks"));
    }

    #[test]
    fn delete_stocks_removes_selected() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));
        rec.stocks.push(make_stock("3", "Gamma", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(1); // select second visible item (Beta)

        lb.DeleteStocks(&mut rec);
        assert_eq!(rec.stocks.len(), 2);
        assert_eq!(rec.stocks[0].name, "Alpha");
        assert_eq!(rec.stocks[1].name, "Gamma");
        assert_eq!(lb.GetSelectionCount(), 0);
    }

    #[test]
    fn delete_stocks_empty_selection_is_noop() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        lb.DeleteStocks(&mut rec);
        assert_eq!(rec.stocks.len(), 1);
    }

    #[test]
    fn cut_stocks_copies_then_deletes() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0); // select Alpha

        let clipboard = lb.CutStocks(&mut rec);
        assert!(clipboard.is_some());
        assert_eq!(rec.stocks.len(), 1);
        assert_eq!(rec.stocks[0].name, "Beta");
    }

    #[test]
    fn paste_stocks_round_trip() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0);

        // Copy, then paste into a fresh rec
        let clipboard = lb.CopyStocks(&rec).unwrap();

        let mut rec2 = emStocksRec::default();
        let mut lb2 = emStocksListBox::new();
        let result = lb2.PasteStocks(&mut rec2, &config, &clipboard);
        assert!(result.is_ok());
        assert_eq!(rec2.stocks.len(), 1);
        assert_eq!(rec2.stocks[0].name, "Alpha");
    }

    #[test]
    fn paste_stocks_reassigns_conflicting_ids() {
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

        let result = lb.PasteStocks(&mut rec, &config, &clipboard);
        assert!(result.is_ok());
        assert_eq!(rec.stocks.len(), 2);
        // The pasted stock should have a new ID, not "1"
        assert_ne!(rec.stocks[1].id, "1");
        assert_eq!(rec.stocks[1].name, "Pasted");
    }

    #[test]
    fn paste_stocks_invalid_data_returns_error() {
        let mut rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        let result = lb.PasteStocks(&mut rec, &config, "not valid data");
        assert!(result.is_err());
    }

    #[test]
    fn paste_stocks_reports_invisible() {
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

        let result = lb.PasteStocks(&mut rec, &config, &clipboard);
        assert!(result.is_ok());
        let invisible = result.unwrap();
        assert_eq!(invisible, vec!["Hidden".to_string()]);
    }

    #[test]
    fn delete_share_prices_clears_visible() {
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
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::Medium));
        rec.stocks.push(make_stock("2", "Beta", Interest::Medium));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0);

        lb.SetInterest(&mut rec, Interest::High);
        // Only the selected stock should change
        assert_eq!(
            rec.stocks[lb.visible_items[0]].interest,
            Interest::High
        );
        assert_eq!(
            rec.stocks[lb.visible_items[1]].interest,
            Interest::Medium
        );
    }

    #[test]
    fn show_first_web_pages() {
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
        lb.Select(0);

        let pages = lb.ShowFirstWebPages(&rec);
        assert_eq!(pages, vec!["https://example.com".to_string()]);
    }

    #[test]
    fn show_all_web_pages() {
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
        lb.Select(0);

        let pages = lb.ShowAllWebPages(&rec);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn get_visible_stock_ids() {
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
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Beta Inc", Interest::High));
        rec.stocks.push(make_stock("3", "Alpha Ltd", Interest::High));

        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();
        config.search_text = "Alpha".to_string();
        lb.UpdateItems(&rec, &config);

        // First find should find "Alpha Corp" (index 0)
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(
            rec.stocks[lb.visible_items[idx]].name,
            "Alpha Corp"
        );

        // Second find should find "Alpha Ltd" (index 2)
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(
            rec.stocks[lb.visible_items[idx]].name,
            "Alpha Ltd"
        );

        // Third find should wrap back to "Alpha Corp"
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(
            rec.stocks[lb.visible_items[idx]].name,
            "Alpha Corp"
        );
    }

    #[test]
    fn find_previous_wraps_around() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Beta Inc", Interest::High));
        rec.stocks.push(make_stock("3", "Alpha Ltd", Interest::High));

        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();
        config.search_text = "Alpha".to_string();
        lb.UpdateItems(&rec, &config);

        // FindPrevious from default (start=0) should go backwards and find Alpha Ltd
        let result = lb.FindPrevious(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(
            rec.stocks[lb.visible_items[idx]].name,
            "Alpha Ltd"
        );
    }

    #[test]
    fn find_next_returns_none_when_no_match() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();
        config.search_text = "Nonexistent".to_string();
        lb.UpdateItems(&rec, &config);

        assert!(lb.FindNext(&rec, &config).is_none());
    }

    #[test]
    fn find_next_returns_none_on_empty_list() {
        let rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        assert!(lb.FindNext(&rec, &config).is_none());
    }

    #[test]
    fn find_selected_sets_search_text() {
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha Corp", Interest::High));

        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);

        let result = lb.FindSelected(&rec, &mut config, "Alpha");
        assert!(result.is_some());
        assert_eq!(config.search_text, "Alpha");
    }

    #[test]
    fn find_selected_empty_text_returns_none() {
        let rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();

        assert!(lb.FindSelected(&rec, &mut config, "").is_none());
    }

    #[test]
    fn selection_helpers() {
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
