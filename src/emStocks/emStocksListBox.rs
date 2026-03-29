// Port of C++ emStocksListBox.h / emStocksListBox.cpp

use std::cmp::Ordering;

use super::emStocksConfig::{emStocksConfig, Sorting};
use super::emStocksRec::{emStocksRec, CompareDates, StockRec};

/// Port of C++ emStocksListBox.
/// DIVERGED: Data model and sorting/filtering logic only — widget and
/// panel infrastructure deferred until panel framework integration.
pub struct emStocksListBox {
    selected_date: String,

    // Visible items: sorted stock indices into emStocksRec.stocks
    pub visible_items: Vec<usize>,
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
        }
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
    pub fn GoBackInHistory(&mut self, rec: &emStocksRec) {
        let date = rec.GetPricesDateBefore(&self.selected_date);
        if !date.is_empty() {
            self.selected_date = date;
        }
    }

    /// Port of C++ GoForwardInHistory.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::emStocksRec::Interest;

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
}
