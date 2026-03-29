//------------------------------------------------------------------------------
// emStocksItemPanel.rs
//
// Port of C++ emStocksItemPanel.h / emStocksItemPanel.cpp
//------------------------------------------------------------------------------
// DIVERGED: Data model only — widget creation and layout deferred
// until panel framework integration.

use super::emStocksRec::ParseDate;

/// Port of C++ emStocksItemPanel::CategoryType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryType {
    Country,
    Sector,
    Collection,
}

/// Port of C++ emStocksItemPanel::CategoryPanel.
/// DIVERGED: Stub — actual widget creation deferred until panel framework integration.
pub struct CategoryPanel {
    pub category_type: CategoryType,
    pub preserved_category: String,
    pub update_controls_needed: bool,
    pub have_list_box_content: bool,
}

impl CategoryPanel {
    pub fn new(category_type: CategoryType) -> Self {
        CategoryPanel {
            category_type,
            preserved_category: String::new(),
            update_controls_needed: false,
            have_list_box_content: false,
        }
    }
}

/// Port of C++ emStocksItemPanel.
/// DIVERGED: Data model only — widget creation and layout deferred
/// until panel framework integration.
pub struct emStocksItemPanel {
    stock_rec_index: Option<usize>,
    pub(crate) update_controls_needed: bool,

    pub country: CategoryPanel,
    pub sector: CategoryPanel,
    pub collection: CategoryPanel,

    // Previous values for change detection
    pub prev_own_shares: String,
    pub prev_purchase_price: String,
    pub prev_purchase_date: String,
    pub prev_sale_price: String,
    pub prev_sale_date: String,
}

impl emStocksItemPanel {
    pub fn new() -> Self {
        emStocksItemPanel {
            stock_rec_index: None,
            update_controls_needed: true,
            country: CategoryPanel::new(CategoryType::Country),
            sector: CategoryPanel::new(CategoryType::Sector),
            collection: CategoryPanel::new(CategoryType::Collection),
            prev_own_shares: String::new(),
            prev_purchase_price: String::new(),
            prev_purchase_date: String::new(),
            prev_sale_price: String::new(),
            prev_sale_date: String::new(),
        }
    }

    pub fn GetStockRecIndex(&self) -> Option<usize> {
        self.stock_rec_index
    }

    pub fn SetStockRecIndex(&mut self, index: Option<usize>) {
        if self.stock_rec_index != index {
            self.stock_rec_index = index;
            self.update_controls_needed = true;
        }
    }

    /// Port of C++ UpdateControls (logic only, no widget updates).
    /// Checks if stock data has changed and flags need to update.
    pub fn NeedsUpdate(&self) -> bool {
        self.update_controls_needed
    }

    pub fn MarkUpdated(&mut self) {
        self.update_controls_needed = false;
    }

    /// Port of C++ ValidateNumber. Returns true if the string is a valid
    /// decimal number (digits and at most one '.'), or empty.
    pub fn ValidateNumber(s: &str) -> bool {
        let mut dot_seen = false;
        for c in s.chars() {
            if c.is_ascii_digit() {
                continue;
            }
            if c == '.' {
                if dot_seen {
                    return false;
                }
                dot_seen = true;
                continue;
            }
            return false;
        }
        true
    }

    /// Port of C++ ValidateDate. Returns true if the string is a valid
    /// date of the form YYYY-MM-DD (parseable), or empty.
    pub fn ValidateDate(s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        ParseDate(s).is_some()
    }
}

impl Default for emStocksItemPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_panel_new() {
        let panel = emStocksItemPanel::new();
        assert!(panel.GetStockRecIndex().is_none());
        assert!(panel.update_controls_needed);
    }

    #[test]
    fn validate_number_valid() {
        assert!(emStocksItemPanel::ValidateNumber("123.45"));
        assert!(emStocksItemPanel::ValidateNumber("0"));
        assert!(emStocksItemPanel::ValidateNumber(""));
    }

    #[test]
    fn validate_number_invalid() {
        assert!(!emStocksItemPanel::ValidateNumber("abc"));
        assert!(!emStocksItemPanel::ValidateNumber("12.34.56"));
    }

    #[test]
    fn validate_date_valid() {
        assert!(emStocksItemPanel::ValidateDate("2024-03-15"));
        assert!(emStocksItemPanel::ValidateDate(""));
    }

    #[test]
    fn validate_date_invalid() {
        assert!(!emStocksItemPanel::ValidateDate("not-a-date"));
    }

    #[test]
    fn category_panel_types() {
        let cp = CategoryPanel::new(CategoryType::Country);
        assert_eq!(cp.category_type, CategoryType::Country);
    }
}
