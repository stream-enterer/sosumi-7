// Port of C++ emStocksControlPanel.h / emStocksControlPanel.cpp

use crate::emStocks::emStocksRec::StockRec;

/// Port of C++ emStocksControlPanel::FileFieldPanel.
/// DIVERGED: Stub — actual widget creation deferred.
pub struct FileFieldPanel {
    pub label: String,
    pub description: String,
}

impl FileFieldPanel {
    pub fn new(label: &str, description: &str) -> Self {
        Self {
            label: label.to_string(),
            description: description.to_string(),
        }
    }
}

/// Port of C++ emStocksControlPanel::CategoryPanel.
/// DIVERGED: Stub — actual widget creation deferred.
/// This is a different type from emStocksItemPanel::CategoryPanel.
pub struct ControlCategoryPanel {
    pub caption: String,
    pub sorted_items: Vec<String>,
}

impl ControlCategoryPanel {
    pub fn new(caption: &str) -> Self {
        Self {
            caption: caption.to_string(),
            sorted_items: Vec::new(),
        }
    }

    /// Port of C++ CategoryPanel::UpdateItems.
    /// Rebuilds the sorted item list from all stocks.
    pub fn UpdateItems(&mut self, stocks: &[StockRec], extract: fn(&StockRec) -> &str) {
        let mut items: Vec<String> = stocks
            .iter()
            .map(|s| extract(s).to_string())
            .filter(|s| !s.is_empty())
            .collect();
        items.sort();
        items.dedup();
        self.sorted_items = items;
    }
}

/// Port of C++ emStocksControlPanel.
/// DIVERGED: Data model only — widget layout deferred.
pub struct emStocksControlPanel {
    // These would reference widgets in the full implementation
    pub update_controls_needed: bool,
}

impl emStocksControlPanel {
    pub fn new() -> Self {
        Self {
            update_controls_needed: true,
        }
    }

    pub fn NeedsUpdate(&self) -> bool {
        self.update_controls_needed
    }

    pub fn MarkUpdated(&mut self) {
        self.update_controls_needed = false;
    }
}

impl Default for emStocksControlPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emStocks::emStocksRec::StockRec;

    #[test]
    fn control_panel_new() {
        let panel = emStocksControlPanel::new();
        assert!(panel.update_controls_needed);
    }

    #[test]
    fn file_field_panel_new() {
        let panel = FileFieldPanel::new("API Script", "Path to script");
        assert_eq!(panel.label, "API Script");
    }

    #[test]
    fn category_panel_update_items() {
        let mut cp = ControlCategoryPanel::new("Countries");
        let mut stocks = vec![StockRec::default(), StockRec::default(), StockRec::default()];
        stocks[0].country = "US".to_string();
        stocks[1].country = "DE".to_string();
        stocks[2].country = "US".to_string(); // duplicate

        cp.UpdateItems(&stocks, |s| &s.country);
        assert_eq!(cp.sorted_items, vec!["DE", "US"]); // sorted, deduplicated
    }
}
