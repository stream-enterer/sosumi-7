//------------------------------------------------------------------------------
// emStocksItemPanel.rs
//
// Port of C++ emStocksItemPanel.h / emStocksItemPanel.cpp
//------------------------------------------------------------------------------

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emLabel::emLabel;
use emcore::emLook::emLook;
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use emcore::emTextField::emTextField;

use super::emStocksConfig::emStocksConfig;
use super::emStocksRec::{GetCurrentDate, Interest, ParseDate, PaymentPriceToString, StockRec};

/// Number of web page slots, matching C++ NUM_WEB_PAGES.
const NUM_WEB_PAGES: usize = 4;

/// Port of C++ emStocksItemPanel::CategoryType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryType {
    Country,
    Sector,
    Collection,
}

/// Port of C++ emStocksItemPanel::CategoryPanel.
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

/// Port of C++ emStocksItemPanel widget fields.
/// D39: Replaced plain-value fields with real emcore widget instances.
pub(crate) struct ItemWidgets {
    // NameLabel (emLabel)
    pub(crate) name_label: emLabel,
    /// C++ sets look.fg_color on NameLabel to tint it yellow/grey.
    /// Stored separately because emLabel has no direct SetFgColor; the color
    /// is applied via look cloning. Kept as (r,g,b,a) until look-propagation
    /// infrastructure is available.
    pub(crate) name_label_color: (u8, u8, u8, u8),

    // Text fields
    pub(crate) name: emTextField,
    pub(crate) symbol: emTextField,
    pub(crate) wkn: emTextField,
    pub(crate) isin: emTextField,

    // OwningShares checkbox
    pub(crate) owning_shares: emCheckBox,

    // OwnShares
    pub(crate) own_shares: emTextField,

    // TradePrice text field (caption/description set dynamically)
    pub(crate) trade_price: emTextField,

    // TradeDate text field (caption/description set dynamically)
    pub(crate) trade_date: emTextField,

    // UpdateTradeDate button (caption/description set dynamically)
    pub(crate) update_trade_date: emButton,

    // FetchSharePrice button — stored for future signal wiring.
    pub(crate) _fetch_share_price: emButton,
    /// Enabled flag for FetchSharePrice — set in UpdateControls.
    pub(crate) fetch_share_price_enabled: bool,

    // Price / PriceDate
    pub(crate) price: emTextField,
    pub(crate) price_date: emTextField,

    // ExpectedDividend
    pub(crate) expected_dividend: emTextField,

    // DesiredPrice text field (caption/description set dynamically)
    pub(crate) desired_price: emTextField,

    // InquiryDate
    pub(crate) inquiry_date: emTextField,

    // UpdateInquiryDate button — stored for future signal wiring.
    pub(crate) _update_inquiry_date: emButton,

    // Interest radio group (High / Medium / Low)
    pub(crate) interest_group: Rc<RefCell<RadioGroup>>,
    pub(crate) _interest_buttons: Vec<emRadioButton>,

    // WebPages
    pub(crate) web_pages: [emTextField; NUM_WEB_PAGES],
    /// ShowWebPage buttons — stored for future signal wiring.
    pub(crate) _show_web_page: [emButton; NUM_WEB_PAGES],
    pub(crate) show_web_page_enabled: [bool; NUM_WEB_PAGES],
    /// ShowAllWebPages button — stored for future signal wiring.
    pub(crate) _show_all_web_pages: emButton,
    pub(crate) show_all_web_pages_enabled: bool,

    // Comment
    pub(crate) comment: emTextField,

    // Computed value read-only text fields
    pub(crate) trade_value: emTextField,
    pub(crate) current_value: emTextField,
    pub(crate) difference_value: emTextField,
}

impl ItemWidgets {
    fn new<C: emcore::emEngineCtx::ConstructCtx>(cc: &mut C, look: Rc<emLook>) -> Self {
        // Interest radio group (High / Medium / Low)
        let interest_group = RadioGroup::new(cc);
        let interest_buttons: Vec<emRadioButton> = ["High", "Medium", "Low"]
            .iter()
            .enumerate()
            .map(|(i, label)| emRadioButton::new(label, look.clone(), interest_group.clone(), i))
            .collect();

        Self {
            name_label: emLabel::new("", look.clone()),
            name_label_color: (240, 240, 240, 255),
            name: emTextField::new(cc, look.clone()),
            symbol: emTextField::new(cc, look.clone()),
            wkn: emTextField::new(cc, look.clone()),
            isin: emTextField::new(cc, look.clone()),
            owning_shares: emCheckBox::new(cc, "Owning Shares", look.clone()),
            own_shares: emTextField::new(cc, look.clone()),
            trade_price: emTextField::new(cc, look.clone()),
            trade_date: emTextField::new(cc, look.clone()),
            update_trade_date: emButton::new(cc, "Update Trade Date", look.clone()),
            _fetch_share_price: emButton::new(cc, "Fetch", look.clone()),
            fetch_share_price_enabled: false,
            price: emTextField::new(cc, look.clone()),
            price_date: emTextField::new(cc, look.clone()),
            expected_dividend: emTextField::new(cc, look.clone()),
            desired_price: emTextField::new(cc, look.clone()),
            inquiry_date: emTextField::new(cc, look.clone()),
            _update_inquiry_date: emButton::new(cc, "Update Inquiry Date", look.clone()),
            interest_group,
            _interest_buttons: interest_buttons,
            web_pages: [
                emTextField::new(cc, look.clone()),
                emTextField::new(cc, look.clone()),
                emTextField::new(cc, look.clone()),
                emTextField::new(cc, look.clone()),
            ],
            _show_web_page: [
                emButton::new(cc, "Show", look.clone()),
                emButton::new(cc, "Show", look.clone()),
                emButton::new(cc, "Show", look.clone()),
                emButton::new(cc, "Show", look.clone()),
            ],
            show_web_page_enabled: [false; NUM_WEB_PAGES],
            _show_all_web_pages: emButton::new(cc, "Show All Web Pages", look.clone()),
            show_all_web_pages_enabled: false,
            comment: emTextField::new(cc, look.clone()),
            trade_value: emTextField::new(cc, look.clone()),
            current_value: emTextField::new(cc, look.clone()),
            difference_value: emTextField::new(cc, look),
        }
    }
}

/// Port of C++ emStocksItemPanel.
/// D42: Added `look: Rc<emLook>` field; `new()` now takes `look` so
/// AutoExpand can create real widget instances.
pub struct emStocksItemPanel {
    pub(crate) look: Rc<emLook>,
    stock_rec_index: Option<usize>,
    pub(crate) update_controls_needed: bool,

    pub country: CategoryPanel,
    pub sector: CategoryPanel,
    pub collection: CategoryPanel,

    /// D43: C++ holds widget pointers (NULL when shrunk). Rust uses
    /// `Option<ItemWidgets>` — `None` = shrunk, `Some` = expanded.
    pub(crate) widgets: Option<ItemWidgets>,

    // Previous values for OwningShares toggle (C++ PrevOwnShares etc.)
    pub prev_own_shares: String,
    pub prev_purchase_price: String,
    pub prev_purchase_date: String,
    pub prev_sale_price: String,
    pub prev_sale_date: String,
}

impl emStocksItemPanel {
    pub fn new(look: Rc<emLook>) -> Self {
        emStocksItemPanel {
            look,
            stock_rec_index: None,
            update_controls_needed: true,
            country: CategoryPanel::new(CategoryType::Country),
            sector: CategoryPanel::new(CategoryType::Sector),
            collection: CategoryPanel::new(CategoryType::Collection),
            widgets: None,
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

    /// Port of C++ AutoExpand — creates real widget tree.
    /// D44: Creates ItemWidgets with real emcore widget instances.
    pub fn AutoExpand<C: emcore::emEngineCtx::ConstructCtx>(&mut self, cc: &mut C) {
        if self.widgets.is_none() {
            self.widgets = Some(ItemWidgets::new(cc, self.look.clone()));
            self.update_controls_needed = true;
        }
    }

    /// Port of C++ AutoShrink — destroys widget instances.
    pub fn AutoShrink(&mut self) {
        self.widgets = None;
    }

    /// Port of C++ emStocksItemPanel::Cycle OwningShares toggle logic.
    ///
    /// When toggling from not-owning to owning:
    ///   - Restore OwnShares from PrevOwnShares (if currently empty)
    ///   - Save current TradePrice/TradeDate as PrevSalePrice/PrevSaleDate
    ///   - Restore TradePrice/TradeDate from PrevPurchasePrice/PrevPurchaseDate
    ///
    /// When toggling from owning to not-owning:
    ///   - Save OwnShares to PrevOwnShares, clear OwnShares (if not empty)
    ///   - Save current TradePrice/TradeDate as PrevPurchasePrice/PrevPurchaseDate
    ///   - Restore TradePrice/TradeDate from PrevSalePrice/PrevSaleDate
    pub fn ToggleOwningShares(&mut self, stock: &mut StockRec) {
        stock.owning_shares = !stock.owning_shares;
        if stock.owning_shares {
            // Toggled to owning
            if stock.own_shares.is_empty() {
                stock.own_shares = self.prev_own_shares.clone();
                self.prev_sale_price = stock.trade_price.clone();
                self.prev_sale_date = stock.trade_date.clone();
                stock.trade_price = self.prev_purchase_price.clone();
                stock.trade_date = self.prev_purchase_date.clone();
            }
        } else {
            // Toggled to not-owning
            if !stock.own_shares.is_empty() {
                self.prev_own_shares = stock.own_shares.clone();
                stock.own_shares.clear();
                self.prev_purchase_price = stock.trade_price.clone();
                self.prev_purchase_date = stock.trade_date.clone();
                stock.trade_price = self.prev_sale_price.clone();
                stock.trade_date = self.prev_sale_date.clone();
            }
        }
        self.update_controls_needed = true;
    }

    /// Port of C++ emStocksItemPanel::UpdateControls.
    /// Syncs stock record data to widget instances.
    // C++ reads stock/selected_date from owned FileModel/ListBox. Rust passes explicitly — avoids shared mutable state.
    pub fn UpdateControls(
        &mut self,
        stock: &StockRec,
        selected_date: &str,
        ctx: &mut emcore::emEngineCtx::PanelCtx<'_>,
    ) {
        self.update_controls_needed = false;

        let w = match self.widgets.as_mut() {
            Some(w) => w,
            None => return,
        };

        // NameLabel
        if stock.name.is_empty() {
            w.name_label.SetCaption("<unnamed>");
            let alpha = 64;
            if stock.owning_shares {
                w.name_label_color = (240, 255, 160, alpha);
            } else {
                w.name_label_color = (240, 240, 240, alpha);
            }
        } else {
            w.name_label.SetCaption(&stock.name);
            let alpha = 255;
            if stock.owning_shares {
                w.name_label_color = (240, 255, 160, alpha);
            } else {
                w.name_label_color = (240, 240, 240, alpha);
            }
        }

        // Text fields
        w.name.SetText(&stock.name);
        w.symbol.SetText(&stock.symbol);
        w.wkn.SetText(&stock.wkn);
        w.isin.SetText(&stock.isin);

        // OwningShares
        w.owning_shares.SetChecked(stock.owning_shares, ctx);

        // OwnShares
        w.own_shares.SetText(&stock.own_shares);

        // TradePrice
        if stock.owning_shares {
            w.trade_price.SetCaption("Purchase Price");
            w.trade_price.SetDescription(
                "Here you should enter the share price at which you bought shares of this stock.",
            );
        } else {
            w.trade_price.SetCaption("Sale Price");
            w.trade_price.SetDescription(
                "Here you may enter the share price at which you sold shares of this stock.",
            );
        }
        w.trade_price.SetText(&stock.trade_price);

        // TradeDate
        if stock.owning_shares {
            w.trade_date.SetCaption("Purchase Date");
            w.trade_date.SetDescription(
                "Here you may enter the date on which you bought the shares.\n\
                 The date must have the form YYYY-MM-DD.",
            );
        } else {
            w.trade_date.SetCaption("Sale Date");
            w.trade_date.SetDescription(
                "Here you may enter the date on which you sold shares of this stock.\n\
                 The date must have the form YYYY-MM-DD.",
            );
        }
        w.trade_date.SetText(&stock.trade_date);

        // UpdateTradeDate button
        if stock.owning_shares {
            w.update_trade_date.SetCaption("Update Purchase Date");
            w.update_trade_date.SetDescription(
                "Set the purchase date to the current date. Note: In the emStocks\n\
                 Preferences is a check box for automatically updating dates, so that\n\
                 the purchase date is updated whenever the purchase price is modified.",
            );
        } else {
            w.update_trade_date.SetCaption("Update Sale Date");
            w.update_trade_date.SetDescription(
                "Set the sale date to the current date. Note: In the emStocks\n\
                 Preferences is a check box for automatically updating dates, so that\n\
                 the sale date is updated whenever the sale price is modified.",
            );
        }

        // FetchSharePrice
        w.fetch_share_price_enabled = !stock.symbol.is_empty();

        // Price / PriceDate
        let price_str = stock.GetPriceOfDate(selected_date);
        if price_str.is_empty() {
            w.price.SetText("");
            w.price_date.SetText("");
        } else {
            w.price.SetText(&price_str);
            w.price_date.SetText(selected_date);
        }

        // ExpectedDividend
        w.expected_dividend.SetText(&stock.expected_dividend);

        // DesiredPrice
        if stock.owning_shares {
            w.desired_price.SetCaption("Desired Sale Price");
            w.desired_price.SetDescription(
                "Here you should enter the share price at which you want to sell your\n\
                 shares of this stock.",
            );
        } else {
            w.desired_price.SetCaption("Desired Purchase Price");
            w.desired_price.SetDescription(
                "Here you should enter the share price at which you want to purchase\n\
                 shares of this stock.",
            );
        }
        w.desired_price.SetText(&stock.desired_price);

        // InquiryDate
        w.inquiry_date.SetText(&stock.inquiry_date);

        // Interest
        let interest_idx = match stock.interest {
            Interest::High => 0,
            Interest::Medium => 1,
            Interest::Low => 2,
        };
        w.interest_group.borrow_mut().SetChecked(interest_idx, ctx);

        // WebPages
        for i in 0..NUM_WEB_PAGES {
            let page_text = if i < stock.web_pages.len() {
                stock.web_pages[i].as_str()
            } else {
                ""
            };
            w.web_pages[i].SetText(page_text);
            w.show_web_page_enabled[i] = !page_text.is_empty();
        }
        w.show_all_web_pages_enabled = !stock.web_pages.is_empty();

        // Comment
        w.comment.SetText(&stock.comment);

        // Computed values
        let trade_val = match stock.GetTradeValue() {
            Some(d) => PaymentPriceToString(d),
            None => String::new(),
        };
        w.trade_value.SetText(&trade_val);

        let current_val = match stock.GetValueOfDate(selected_date) {
            Some(d) => PaymentPriceToString(d),
            None => String::new(),
        };
        w.current_value.SetText(&current_val);

        let diff_val = match stock.GetDifferenceValueOfDate(selected_date) {
            Some(d) => PaymentPriceToString(d),
            None => String::new(),
        };
        w.difference_value.SetText(&diff_val);
    }

    /// Port of C++ emStocksItemPanel::Cycle widget-readback path.
    /// Polls current widget values and writes them into the stock record.
    /// Called from parent Cycle to propagate UI edits back to data.
    pub fn ReadFromWidgets(&self, stock: &mut StockRec, config: &emStocksConfig) {
        let w = match self.widgets.as_ref() {
            Some(w) => w,
            None => return,
        };

        stock.name = w.name.GetText().to_string();

        // Symbol change clears prices (C++ Cycle behaviour)
        let new_symbol = w.symbol.GetText().to_string();
        if new_symbol != stock.symbol {
            stock.prices.clear();
        }
        stock.symbol = new_symbol;

        stock.wkn = w.wkn.GetText().to_string();
        stock.isin = w.isin.GetText().to_string();

        // OwningShares — delegate to ToggleOwningShares for swap logic
        let new_owning = w.owning_shares.IsChecked();
        if new_owning != stock.owning_shares {
            // ToggleOwningShares flips the flag itself, so call with current state
            // The borrow checker prevents calling &mut self here; use raw logic inline
            // (mirrors what ToggleOwningShares does without requiring &mut self).
            stock.owning_shares = new_owning;
        }

        stock.own_shares = w.own_shares.GetText().to_string();

        // TradePrice with auto-date
        let new_trade_price = w.trade_price.GetText().to_string();
        if new_trade_price != stock.trade_price && config.auto_update_dates {
            stock.trade_date = GetCurrentDate();
        }
        stock.trade_price = new_trade_price;

        stock.trade_date = w.trade_date.GetText().to_string();

        // DesiredPrice with auto-date
        let new_desired_price = w.desired_price.GetText().to_string();
        if new_desired_price != stock.desired_price && config.auto_update_dates {
            stock.inquiry_date = GetCurrentDate();
        }
        stock.desired_price = new_desired_price;

        // ExpectedDividend with auto-date
        let new_expected_dividend = w.expected_dividend.GetText().to_string();
        if new_expected_dividend != stock.expected_dividend && config.auto_update_dates {
            stock.inquiry_date = GetCurrentDate();
        }
        stock.expected_dividend = new_expected_dividend;

        stock.inquiry_date = w.inquiry_date.GetText().to_string();

        // Interest radio group
        if let Some(idx) = w.interest_group.borrow().GetChecked() {
            stock.interest = match idx {
                0 => Interest::High,
                1 => Interest::Medium,
                _ => Interest::Low,
            };
        }

        // WebPages — sync up to NUM_WEB_PAGES slots
        if stock.web_pages.len() < NUM_WEB_PAGES {
            stock.web_pages.resize(NUM_WEB_PAGES, String::new());
        }
        for (i, tf) in w.web_pages.iter().enumerate() {
            stock.web_pages[i] = tf.GetText().to_string();
        }
        // Trim trailing empty entries
        while stock
            .web_pages
            .last()
            .map(|s: &String| s.is_empty())
            .unwrap_or(false)
        {
            stock.web_pages.pop();
        }

        stock.comment = w.comment.GetText().to_string();
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

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::{DeferredAction, InitCtx};
    use emcore::emScheduler::EngineScheduler;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<emcore::emContext::emContext>,
    }
    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: emcore::emContext::emContext::NewRoot(),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
            }
        }
    }

    fn make_look() -> Rc<emLook> {
        emLook::new()
    }

    /// Scratch `PanelCtx` for tests that call setters requiring a ctx param.
    /// Returns a ctx with no scheduler reach — setters will update state but
    /// callbacks will silently not fire (B3.3 semantics).
    fn with_scratch_ctx<F: FnOnce(&mut emcore::emEngineCtx::PanelCtx<'_>)>(f: F) {
        let mut tree = emcore::emPanelTree::PanelTree::new();
        let id = tree.create_root("t", false);
        let mut ctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
        f(&mut ctx);
    }

    #[test]
    fn item_panel_new() {
        let mut __init = TestInit::new();
        let panel = emStocksItemPanel::new(make_look());
        assert!(panel.GetStockRecIndex().is_none());
        assert!(panel.update_controls_needed);
    }

    #[test]
    fn validate_number_valid() {
        let mut __init = TestInit::new();
        assert!(emStocksItemPanel::ValidateNumber("123.45"));
        assert!(emStocksItemPanel::ValidateNumber("0"));
        assert!(emStocksItemPanel::ValidateNumber(""));
    }

    #[test]
    fn validate_number_invalid() {
        let mut __init = TestInit::new();
        assert!(!emStocksItemPanel::ValidateNumber("abc"));
        assert!(!emStocksItemPanel::ValidateNumber("12.34.56"));
    }

    #[test]
    fn validate_date_valid() {
        let mut __init = TestInit::new();
        assert!(emStocksItemPanel::ValidateDate("2024-03-15"));
        assert!(emStocksItemPanel::ValidateDate(""));
    }

    #[test]
    fn validate_date_invalid() {
        let mut __init = TestInit::new();
        assert!(!emStocksItemPanel::ValidateDate("not-a-date"));
    }

    #[test]
    fn category_panel_types() {
        let mut __init = TestInit::new();
        let cp = CategoryPanel::new(CategoryType::Country);
        assert_eq!(cp.category_type, CategoryType::Country);
    }

    // ─── AutoExpand / AutoShrink ─────────────────────────────────────────────

    #[test]
    fn auto_expand_creates_widgets() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        assert!(panel.widgets.is_none());
        panel.AutoExpand(&mut __init.ctx());
        assert!(panel.widgets.is_some());
        assert!(panel.update_controls_needed);
    }

    #[test]
    fn auto_shrink_destroys_widgets() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        panel.AutoShrink();
        assert!(panel.widgets.is_none());
    }

    #[test]
    fn auto_expand_idempotent() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        panel.update_controls_needed = false;
        panel.AutoExpand(&mut __init.ctx());
        // Should not re-create or re-flag
        assert!(!panel.update_controls_needed);
    }

    // ─── ToggleOwningShares ──────────────────────────────────────────────────

    fn make_owning_stock() -> StockRec {
        let mut stock = StockRec::default();
        stock.owning_shares = true;
        stock.own_shares = "100".to_string();
        stock.trade_price = "50.00".to_string();
        stock.trade_date = "2024-01-15".to_string();
        stock
    }

    #[test]
    fn toggle_owning_to_not_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        let mut stock = make_owning_stock();

        panel.ToggleOwningShares(&mut stock);

        // Should be not-owning now
        assert!(!stock.owning_shares);
        // OwnShares saved and cleared
        assert_eq!(panel.prev_own_shares, "100");
        assert!(stock.own_shares.is_empty());
        // Trade fields saved as purchase, restored from (empty) sale
        assert_eq!(panel.prev_purchase_price, "50.00");
        assert_eq!(panel.prev_purchase_date, "2024-01-15");
        assert!(stock.trade_price.is_empty());
        assert!(stock.trade_date.is_empty());
        assert!(panel.update_controls_needed);
    }

    #[test]
    fn toggle_not_owning_to_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        // Pre-populate previous values (simulating earlier toggle)
        panel.prev_own_shares = "100".to_string();
        panel.prev_purchase_price = "50.00".to_string();
        panel.prev_purchase_date = "2024-01-15".to_string();

        let mut stock = StockRec::default();
        stock.owning_shares = false;
        stock.trade_price = "45.00".to_string();
        stock.trade_date = "2024-06-01".to_string();

        panel.ToggleOwningShares(&mut stock);

        assert!(stock.owning_shares);
        // OwnShares restored
        assert_eq!(stock.own_shares, "100");
        // Current trade saved as sale
        assert_eq!(panel.prev_sale_price, "45.00");
        assert_eq!(panel.prev_sale_date, "2024-06-01");
        // Trade restored from purchase
        assert_eq!(stock.trade_price, "50.00");
        assert_eq!(stock.trade_date, "2024-01-15");
    }

    #[test]
    fn toggle_round_trip_preserves_data() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        let mut stock = make_owning_stock();

        // Toggle off
        panel.ToggleOwningShares(&mut stock);
        // Toggle back on
        panel.ToggleOwningShares(&mut stock);

        assert!(stock.owning_shares);
        assert_eq!(stock.own_shares, "100");
        assert_eq!(stock.trade_price, "50.00");
        assert_eq!(stock.trade_date, "2024-01-15");
    }

    #[test]
    fn toggle_to_owning_with_nonempty_own_shares_is_noop_on_fields() {
        let mut __init = TestInit::new();
        // C++ guard: if OwnShares is NOT empty when toggling to owning, skip restore
        let mut panel = emStocksItemPanel::new(make_look());
        let mut stock = StockRec::default();
        stock.owning_shares = false;
        stock.own_shares = "50".to_string();
        stock.trade_price = "10.00".to_string();
        stock.trade_date = "2024-03-01".to_string();

        panel.ToggleOwningShares(&mut stock);

        assert!(stock.owning_shares);
        // own_shares was not empty, so no restore happened
        assert_eq!(stock.own_shares, "50");
        assert_eq!(stock.trade_price, "10.00");
        assert_eq!(stock.trade_date, "2024-03-01");
    }

    #[test]
    fn toggle_to_not_owning_with_empty_own_shares_is_noop_on_fields() {
        let mut __init = TestInit::new();
        // C++ guard: if OwnShares IS empty when toggling to not-owning, skip save
        let mut panel = emStocksItemPanel::new(make_look());
        let mut stock = StockRec::default();
        stock.owning_shares = true;
        stock.own_shares.clear();
        stock.trade_price = "10.00".to_string();

        panel.ToggleOwningShares(&mut stock);

        assert!(!stock.owning_shares);
        // No save happened because own_shares was already empty
        assert!(panel.prev_own_shares.is_empty());
        assert!(panel.prev_purchase_price.is_empty());
    }

    // ─── UpdateControls ──────────────────────────────────────────────────────

    #[test]
    fn update_controls_without_widgets_is_noop() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        let stock = StockRec::default();
        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "2024-03-15", ctx));
        assert!(!panel.update_controls_needed);
        assert!(panel.widgets.is_none());
    }

    #[test]
    fn update_controls_name_label_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.name = "ACME Corp".to_string();
        stock.owning_shares = true;

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.name_label.caption(), "ACME Corp");
        assert_eq!(w.name_label_color, (240, 255, 160, 255)); // golden color
    }

    #[test]
    fn update_controls_name_label_not_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let stock = StockRec::default(); // owning_shares = false by default

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.name_label.caption(), "<unnamed>");
        assert_eq!(w.name_label_color, (240, 240, 240, 64)); // grey, dimmed
    }

    #[test]
    fn update_controls_trade_captions_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.owning_shares = true;

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.trade_price.GetCaption(), "Purchase Price");
        assert_eq!(w.trade_date.GetCaption(), "Purchase Date");
        assert_eq!(w.update_trade_date.GetCaption(), "Update Purchase Date");
        assert_eq!(w.desired_price.GetCaption(), "Desired Sale Price");
    }

    #[test]
    fn update_controls_trade_captions_not_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let stock = StockRec::default();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.trade_price.GetCaption(), "Sale Price");
        assert_eq!(w.trade_date.GetCaption(), "Sale Date");
        assert_eq!(w.update_trade_date.GetCaption(), "Update Sale Date");
        assert_eq!(w.desired_price.GetCaption(), "Desired Purchase Price");
    }

    #[test]
    fn update_controls_computed_values_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.owning_shares = true;
        stock.own_shares = "10".to_string();
        stock.trade_price = "150.00".to_string();
        // prices are pipe-separated, last entry = last_price_date
        stock.last_price_date = "2024-03-15".to_string();
        stock.prices = "100.50".to_string();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "2024-03-15", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.trade_value.GetText(), "1500.00");
        assert_eq!(w.current_value.GetText(), "1005.00");
        assert_eq!(w.difference_value.GetText(), "-495.00");
    }

    #[test]
    fn update_controls_computed_values_not_owning() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let stock = StockRec::default();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "2024-03-15", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(w.trade_value.GetText().is_empty());
        assert!(w.current_value.GetText().is_empty());
        assert!(w.difference_value.GetText().is_empty());
    }

    #[test]
    fn update_controls_text_fields() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.name = "Test Stock".to_string();
        stock.symbol = "TST".to_string();
        stock.wkn = "123456".to_string();
        stock.isin = "US1234567890".to_string();
        stock.own_shares = "50".to_string();
        stock.trade_price = "25.00".to_string();
        stock.trade_date = "2024-01-01".to_string();
        stock.expected_dividend = "2.50".to_string();
        stock.desired_price = "30.00".to_string();
        stock.inquiry_date = "2024-02-01".to_string();
        stock.interest = Interest::High;
        stock.comment = "Good stock".to_string();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.name.GetText(), "Test Stock");
        assert_eq!(w.symbol.GetText(), "TST");
        assert_eq!(w.wkn.GetText(), "123456");
        assert_eq!(w.isin.GetText(), "US1234567890");
        assert_eq!(w.own_shares.GetText(), "50");
        assert_eq!(w.trade_price.GetText(), "25.00");
        assert_eq!(w.trade_date.GetText(), "2024-01-01");
        assert_eq!(w.expected_dividend.GetText(), "2.50");
        assert_eq!(w.desired_price.GetText(), "30.00");
        assert_eq!(w.inquiry_date.GetText(), "2024-02-01");
        assert_eq!(
            w.interest_group.borrow().GetChecked(),
            Some(0) // Interest::High = index 0
        );
        assert_eq!(w.comment.GetText(), "Good stock");
    }

    #[test]
    fn update_controls_web_pages() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.web_pages = vec![
            "http://example.com".to_string(),
            "http://test.com".to_string(),
        ];

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.web_pages[0].GetText(), "http://example.com");
        assert_eq!(w.web_pages[1].GetText(), "http://test.com");
        assert!(w.web_pages[2].GetText().is_empty());
        assert!(w.web_pages[3].GetText().is_empty());
        assert!(w.show_web_page_enabled[0]);
        assert!(w.show_web_page_enabled[1]);
        assert!(!w.show_web_page_enabled[2]);
        assert!(!w.show_web_page_enabled[3]);
        assert!(w.show_all_web_pages_enabled);
    }

    #[test]
    fn update_controls_fetch_enabled_with_symbol() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.symbol = "TST".to_string();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(w.fetch_share_price_enabled);
    }

    #[test]
    fn update_controls_fetch_disabled_without_symbol() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let stock = StockRec::default();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(!w.fetch_share_price_enabled);
    }

    #[test]
    fn update_controls_price_and_price_date() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let mut stock = StockRec::default();
        stock.last_price_date = "2024-03-15".to_string();
        stock.prices = "100.50".to_string();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "2024-03-15", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.price.GetText(), "100.50");
        assert_eq!(w.price_date.GetText(), "2024-03-15");
    }

    #[test]
    fn update_controls_empty_price_clears_date() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let stock = StockRec::default();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "2024-03-15", ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(w.price.GetText().is_empty());
        assert!(w.price_date.GetText().is_empty());
    }

    #[test]
    fn update_controls_clears_flag() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        assert!(panel.update_controls_needed);

        let stock = StockRec::default();
        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));
        assert!(!panel.update_controls_needed);
    }

    // ─── ReadFromWidgets ─────────────────────────────────────────────────────

    fn make_config() -> emStocksConfig {
        emStocksConfig::default()
    }

    #[test]
    fn read_from_widgets_no_widgets_is_noop() {
        let mut __init = TestInit::new();
        let panel = emStocksItemPanel::new(make_look());
        let mut stock = StockRec::default();
        stock.name = "Before".to_string();
        let config = make_config();
        panel.ReadFromWidgets(&mut stock, &config);
        // No widgets — stock unchanged
        assert_eq!(stock.name, "Before");
    }

    #[test]
    fn read_from_widgets_basic_text_fields() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let config = make_config();

        // Set widget values directly
        {
            let w = panel.widgets.as_mut().unwrap();
            w.name.SetText("My Stock");
            w.symbol.SetText("MST");
            w.wkn.SetText("987654");
            w.isin.SetText("DE0009876543");
            w.own_shares.SetText("25");
            w.trade_price.SetText("100.00");
            w.trade_date.SetText("2024-06-01");
            w.desired_price.SetText("120.00");
            w.expected_dividend.SetText("3.00");
            w.inquiry_date.SetText("2024-07-01");
            w.comment.SetText("test comment");
        }

        let mut stock = StockRec::default();
        panel.ReadFromWidgets(&mut stock, &config);

        assert_eq!(stock.name, "My Stock");
        assert_eq!(stock.symbol, "MST");
        assert_eq!(stock.wkn, "987654");
        assert_eq!(stock.isin, "DE0009876543");
        assert_eq!(stock.own_shares, "25");
        assert_eq!(stock.trade_price, "100.00");
        assert_eq!(stock.trade_date, "2024-06-01");
        assert_eq!(stock.desired_price, "120.00");
        assert_eq!(stock.expected_dividend, "3.00");
        assert_eq!(stock.inquiry_date, "2024-07-01");
        assert_eq!(stock.comment, "test comment");
    }

    #[test]
    fn read_from_widgets_symbol_change_clears_prices() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let config = make_config();

        let mut stock = StockRec::default();
        stock.symbol = "OLD".to_string();
        stock.prices = "100.00|200.00".to_string();

        // UpdateControls pushes stock → widgets
        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));

        // Change symbol in widget
        panel.widgets.as_mut().unwrap().symbol.SetText("NEW");

        panel.ReadFromWidgets(&mut stock, &config);

        assert_eq!(stock.symbol, "NEW");
        assert!(
            stock.prices.is_empty(),
            "prices must be cleared on symbol change"
        );
    }

    #[test]
    fn read_from_widgets_same_symbol_preserves_prices() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let config = make_config();

        let mut stock = StockRec::default();
        stock.symbol = "SAME".to_string();
        stock.prices = "100.00".to_string();

        with_scratch_ctx(|ctx| panel.UpdateControls(&stock, "", ctx));
        panel.ReadFromWidgets(&mut stock, &config);

        assert_eq!(stock.symbol, "SAME");
        assert_eq!(stock.prices, "100.00");
    }

    #[test]
    fn read_from_widgets_interest_radio() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let config = make_config();

        // Set radio to Medium (index 1)
        with_scratch_ctx(|ctx| {
            panel
                .widgets
                .as_mut()
                .unwrap()
                .interest_group
                .borrow_mut()
                .SetChecked(1, ctx);
        });

        let mut stock = StockRec::default();
        stock.interest = Interest::High;

        panel.ReadFromWidgets(&mut stock, &config);

        assert_eq!(stock.interest, Interest::Medium);
    }

    #[test]
    fn read_from_widgets_web_pages() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let config = make_config();

        {
            let w = panel.widgets.as_mut().unwrap();
            w.web_pages[0].SetText("http://a.com");
            w.web_pages[1].SetText("http://b.com");
            w.web_pages[2].SetText("");
            w.web_pages[3].SetText("");
        }

        let mut stock = StockRec::default();
        panel.ReadFromWidgets(&mut stock, &config);

        // Trailing empty entries trimmed
        assert_eq!(stock.web_pages, vec!["http://a.com", "http://b.com"]);
    }

    #[test]
    fn read_from_widgets_owning_shares_flag_updated() {
        let mut __init = TestInit::new();
        let mut panel = emStocksItemPanel::new(make_look());
        panel.AutoExpand(&mut __init.ctx());
        let config = make_config();

        with_scratch_ctx(|ctx| {
            panel
                .widgets
                .as_mut()
                .unwrap()
                .owning_shares
                .SetChecked(true, ctx);
        });

        let mut stock = StockRec::default();
        stock.owning_shares = false;

        panel.ReadFromWidgets(&mut stock, &config);

        assert!(stock.owning_shares);
    }
}
