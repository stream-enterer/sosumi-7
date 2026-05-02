// Port of C++ emStocksControlPanel.h / emStocksControlPanel.cpp

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emFileSelectionBox::emFileSelectionBox;
use emcore::emLook::emLook;
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use emcore::emScalarField::emScalarField;
use emcore::emSignal::SignalId;
use emcore::emTextField::emTextField;

use crate::emStocksConfig::{emStocksConfig, ChartPeriod, Sorting};
use crate::emStocksFileModel::emStocksFileModel;
use crate::emStocksListBox::emStocksListBox;
use crate::emStocksRec::{emStocksRec, Interest, PaymentPriceToString, StockRec};

// ─── FileFieldPanel ──────────────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel::FileFieldType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileFieldType {
    Script,      // FT_SCRIPT
    Interpreter, // FT_INTERPRETER
    Browser,     // FT_BROWSER
}

/// Port of C++ emStocksControlPanel::FileFieldPanel.
/// D20: Replaced `text_value: String` with `widget: emFileSelectionBox`.
pub(crate) struct FileFieldPanel {
    pub(crate) field_type: FileFieldType,
    pub(crate) widget: emFileSelectionBox,
    pub(crate) update_controls_needed: bool,
}

impl FileFieldPanel {
    pub(crate) fn new<C: emcore::emEngineCtx::ConstructCtx>(
        cc: &mut C,
        field_type: FileFieldType,
        caption: &str,
    ) -> Self {
        Self {
            field_type,
            widget: emFileSelectionBox::new(cc, caption),
            update_controls_needed: true,
        }
    }

    /// Port of C++ FileFieldPanel::UpdateControls.
    pub(crate) fn UpdateControls(&mut self, config: &emStocksConfig) {
        self.update_controls_needed = false;
        let value = match self.field_type {
            FileFieldType::Script => &config.api_script,
            FileFieldType::Interpreter => &config.api_script_interpreter,
            FileFieldType::Browser => &config.web_browser,
        };
        use std::path::Path;
        self.widget.set_selected_path(Path::new(value.as_str()));
    }
}

// ─── CategoryType ────────────────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel::CategoryType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CategoryType {
    Country,    // CT_COUNTRY
    Sector,     // CT_SECTOR
    Collection, // CT_COLLECTION
}

// ─── ControlCategoryPanel ────────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel::CategoryPanel.
/// Uses sorted_items: Vec<String> for category items. C++ uses an emListBox widget.
/// This is a different type from emStocksItemPanel::CategoryPanel.
pub struct ControlCategoryPanel {
    pub caption: String,
    pub sorted_items: Vec<String>,
    pub(crate) category_type: CategoryType,
}

impl ControlCategoryPanel {
    pub(crate) fn new(caption: &str, category_type: CategoryType) -> Self {
        Self {
            caption: caption.to_string(),
            sorted_items: Vec::new(),
            category_type,
        }
    }

    /// Returns the extractor function for this panel's category type.
    pub(crate) fn extractor(&self) -> fn(&StockRec) -> &str {
        match self.category_type {
            CategoryType::Country => |s| &s.country,
            CategoryType::Sector => |s| &s.sector,
            CategoryType::Collection => |s| &s.collection,
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

// ─── Helpers for enum ↔ radio-group index conversions ─────────────────────────

fn interest_to_index(i: Interest) -> usize {
    match i {
        Interest::High => 0,
        Interest::Medium => 1,
        Interest::Low => 2,
    }
}

fn sorting_to_index(s: Sorting) -> usize {
    match s {
        Sorting::ByName => 0,
        Sorting::ByTradeDate => 1,
        Sorting::ByInquiryDate => 2,
        Sorting::ByAchievement => 3,
        Sorting::ByOneWeekRise => 4,
        Sorting::ByThreeWeekRise => 5,
        Sorting::ByNineWeekRise => 6,
        Sorting::ByDividend => 7,
        Sorting::ByPurchaseValue => 8,
        Sorting::ByValue => 9,
        Sorting::ByDifference => 10,
    }
}

fn chart_period_to_index(p: ChartPeriod) -> f64 {
    match p {
        ChartPeriod::Week1 => 0.0,
        ChartPeriod::Weeks2 => 1.0,
        ChartPeriod::Month1 => 2.0,
        ChartPeriod::Months3 => 3.0,
        ChartPeriod::Months6 => 4.0,
        ChartPeriod::Year1 => 5.0,
        ChartPeriod::Years3 => 6.0,
        ChartPeriod::Years5 => 7.0,
        ChartPeriod::Years10 => 8.0,
        ChartPeriod::Years20 => 9.0,
    }
}

// ─── ControlWidgets ──────────────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel widget fields.
/// D22: Replaced plain-value fields with real emcore widget instances.
/// `chart_period_text` is kept as a derived string for `emScalarField` text display.
pub(crate) struct ControlWidgets {
    // Config fields (Preferences group)
    pub(crate) api_script: FileFieldPanel,
    pub(crate) api_script_interpreter: FileFieldPanel,
    /// D22: `api_key: String` replaced with `emTextField`.
    pub(crate) api_key: emTextField,
    pub(crate) web_browser: FileFieldPanel,
    /// D22: `auto_update_dates: bool` replaced with `emCheckBox`.
    pub(crate) auto_update_dates: emCheckBox,
    /// D22: `triggering_opens_web_page: bool` replaced with `emCheckBox`.
    pub(crate) triggering_opens_web_page: emCheckBox,
    /// D22: `chart_period: ChartPeriod` replaced with `emScalarField` (range 0.0..9.0).
    /// Each integer index maps to a ChartPeriod variant via `chart_period_to_index`.
    pub(crate) chart_period: emScalarField,
    /// Display text for the current chart period (set via ChartPeriodTextOfValue).
    /// Kept alongside the widget for consumers that need a plain string.
    pub(crate) chart_period_text: &'static str,

    // Filter fields
    /// D22: `min_visible_interest: Interest` replaced with `RadioGroup` + buttons.
    /// Shared group enforces mutual exclusion across 3 interest levels.
    pub(crate) min_visible_interest_group: Rc<RefCell<RadioGroup>>,
    /// Individual interest-level radio buttons — stored for future signal wiring.
    pub(crate) _min_visible_interest_buttons: Vec<emRadioButton>,
    pub(crate) visible_countries: ControlCategoryPanel,
    pub(crate) visible_sectors: ControlCategoryPanel,
    pub(crate) visible_collections: ControlCategoryPanel,

    // Sorting
    /// D22: `sorting: Sorting` replaced with `RadioGroup` + buttons.
    /// Shared group enforces mutual exclusion across 11 sort orders.
    pub(crate) sorting_group: Rc<RefCell<RadioGroup>>,
    /// Individual sorting radio buttons — stored for future signal wiring.
    pub(crate) _sorting_buttons: Vec<emRadioButton>,
    /// D22: `owned_shares_first: bool` replaced with `emCheckBox`.
    pub(crate) owned_shares_first: emCheckBox,

    // Prices group — FetchSharePrices, DeleteSharePrices always enabled in C++.
    // B-001 Phase 2 G8 widget instances. B-001-followup Phase B drops the
    // underscore prefix as Cycle subscribes land — the fields are no longer
    // dead code.
    /// B-001 G8 row -586: Prices group `FetchSharePrices` button.
    pub(crate) fetch_share_prices: emButton,
    /// B-001 G8 row -600: Prices group `DeleteSharePrices` button.
    pub(crate) delete_share_prices: emButton,
    /// B-001 G8 row -609: Prices group `GoBackInHistory` button.
    pub(crate) go_back_in_history: emButton,
    /// B-001 G8 row -618: Prices group `GoForwardInHistory` button.
    pub(crate) go_forward_in_history: emButton,
    pub(crate) go_back_in_history_enabled: bool,
    pub(crate) go_forward_in_history_enabled: bool,
    /// B-001 G8 row -626: Prices group `SelectedDate` editable text field.
    /// The C++ widget is the editable surface; `selected_date: String` below
    /// remains the cached display value populated in `UpdateControls`.
    pub(crate) selected_date_field: emTextField,
    pub(crate) selected_date: String,
    pub(crate) total_purchase_value: String,
    pub(crate) total_current_value: String,
    pub(crate) total_difference_value: String,

    // Commands group — NewStock, PasteStocks always enabled in C++
    /// B-001 G8 row -650: Commands group `NewStock` button.
    pub(crate) new_stock: emButton,
    /// B-001 G8 row -658: Commands group `CutStocks` button.
    pub(crate) cut_stocks: emButton,
    /// B-001 G8 row -666: Commands group `CopyStocks` button.
    pub(crate) copy_stocks: emButton,
    /// B-001 G8 row -674: Commands group `PasteStocks` button.
    pub(crate) paste_stocks: emButton,
    /// B-001 G8 row -682: Commands group `DeleteStocks` button.
    pub(crate) delete_stocks: emButton,
    /// B-001 G8 row -690: Commands group `SelectAll` button.
    pub(crate) select_all: emButton,
    /// B-001 G8 row -698: Commands group `ClearSelection` button.
    pub(crate) clear_selection: emButton,
    /// B-001 G8 row -706: Commands group `SetHighInterest` button.
    pub(crate) set_high_interest: emButton,
    /// B-001 G8 row -714: Commands group `SetMediumInterest` button.
    pub(crate) set_medium_interest: emButton,
    /// B-001 G8 row -722: Commands group `SetLowInterest` button.
    pub(crate) set_low_interest: emButton,
    /// B-001 G8 row -730: Commands group `ShowFirstWebPages` button.
    pub(crate) show_first_web_pages: emButton,
    /// B-001 G8 row -738: Commands group `ShowAllWebPages` button.
    pub(crate) show_all_web_pages: emButton,
    pub(crate) cut_stocks_enabled: bool,
    pub(crate) copy_stocks_enabled: bool,
    pub(crate) delete_stocks_enabled: bool,
    pub(crate) select_all_enabled: bool,
    pub(crate) clear_selection_enabled: bool,
    pub(crate) set_high_interest_enabled: bool,
    pub(crate) set_medium_interest_enabled: bool,
    pub(crate) set_low_interest_enabled: bool,
    pub(crate) show_first_web_pages_enabled: bool,
    pub(crate) show_all_web_pages_enabled: bool,

    // Search group — FindSelected always enabled in C++
    /// B-001 G8 row -749: Search group `FindSelected` button.
    pub(crate) find_selected: emButton,
    /// D22: `search_text: String` replaced with `emTextField`.
    pub(crate) search_text: emTextField,
    /// B-001 G8 row -764: Search group `FindNext` button.
    pub(crate) find_next: emButton,
    /// B-001 G8 row -772: Search group `FindPrevious` button.
    pub(crate) find_previous: emButton,
    pub(crate) find_next_enabled: bool,
    pub(crate) find_previous_enabled: bool,
}

impl ControlWidgets {
    fn new<C: emcore::emEngineCtx::ConstructCtx>(cc: &mut C, look: Rc<emLook>) -> Self {
        // Build interest radio-button group (High / Medium / Low)
        let interest_group = RadioGroup::new(cc);
        let interest_buttons: Vec<emRadioButton> = ["High", "Medium", "Low"]
            .iter()
            .enumerate()
            .map(|(i, label)| {
                emRadioButton::new(cc, label, look.clone(), interest_group.clone(), i)
            })
            .collect();

        // Build sorting radio-button group (11 variants)
        let sorting_group = RadioGroup::new(cc);
        let sorting_captions = [
            "By Name",
            "By Trade Date",
            "By Inquiry Date",
            "By Achievement",
            "By 1-Week Rise",
            "By 3-Week Rise",
            "By 9-Week Rise",
            "By Dividend",
            "By Purchase Value",
            "By Value",
            "By Difference",
        ];
        let sorting_buttons: Vec<emRadioButton> = sorting_captions
            .iter()
            .enumerate()
            .map(|(i, label)| emRadioButton::new(cc, label, look.clone(), sorting_group.clone(), i))
            .collect();

        // Chart period scalar field: integer steps 0..9, default to Year1 (index 5)
        let mut chart_period_field = emScalarField::new(cc, 0.0, 9.0, look.clone());
        chart_period_field.set_initial_value(chart_period_to_index(ChartPeriod::default()));
        chart_period_field.SetTextOfValueFunc(Box::new(|v, _| {
            let period = match v {
                0 => "1\nweek",
                1 => "2\nweeks",
                2 => "1\nmonth",
                3 => "3\nmonths",
                4 => "6\nmonths",
                5 => "1\nyear",
                6 => "3\nyears",
                7 => "5\nyears",
                8 => "10\nyears",
                9 => "20\nyears",
                _ => "",
            };
            period.to_string()
        }));

        Self {
            api_script: FileFieldPanel::new(cc, FileFieldType::Script, "API Script"),
            api_script_interpreter: FileFieldPanel::new(
                cc,
                FileFieldType::Interpreter,
                "API Script Interpreter",
            ),
            api_key: emTextField::new(cc, look.clone()),
            web_browser: FileFieldPanel::new(cc, FileFieldType::Browser, "Web Browser"),
            auto_update_dates: emCheckBox::new(cc, "Auto Update Dates", look.clone()),
            triggering_opens_web_page: emCheckBox::new(
                cc,
                "Triggering Opens Web Page",
                look.clone(),
            ),
            chart_period: chart_period_field,
            chart_period_text: ChartPeriodTextOfValue(ChartPeriod::default()),

            min_visible_interest_group: interest_group,
            _min_visible_interest_buttons: interest_buttons,
            visible_countries: ControlCategoryPanel::new(
                "Visible Countries",
                CategoryType::Country,
            ),
            visible_sectors: ControlCategoryPanel::new("Visible Sectors", CategoryType::Sector),
            visible_collections: ControlCategoryPanel::new(
                "Visible Collections",
                CategoryType::Collection,
            ),

            sorting_group,
            _sorting_buttons: sorting_buttons,
            owned_shares_first: emCheckBox::new(cc, "Owned Shares First", look.clone()),

            // Prices group buttons (B-001 G8 rows -586..-618, -626 textfield).
            // Captions mirror C++ emStocksControlPanel.cpp:576-630.
            fetch_share_prices: emButton::new(cc, "Fetch\nPrices", look.clone()),
            delete_share_prices: emButton::new(cc, "Delete Prices", look.clone()),
            go_back_in_history: emButton::new(cc, "Go Back In History", look.clone()),
            go_forward_in_history: emButton::new(cc, "Go Forward In History", look.clone()),
            go_back_in_history_enabled: false,
            go_forward_in_history_enabled: false,
            selected_date_field: emTextField::new(cc, look.clone()),
            selected_date: String::new(),
            total_purchase_value: String::new(),
            total_current_value: String::new(),
            total_difference_value: String::new(),

            // Commands group buttons (B-001 G8 rows -650..-738).
            // Captions mirror C++ emStocksControlPanel.cpp:644-741.
            new_stock: emButton::new(cc, "New", look.clone()),
            cut_stocks: emButton::new(cc, "Cut", look.clone()),
            copy_stocks: emButton::new(cc, "Copy", look.clone()),
            paste_stocks: emButton::new(cc, "Paste", look.clone()),
            delete_stocks: emButton::new(cc, "Delete", look.clone()),
            select_all: emButton::new(cc, "Select All", look.clone()),
            clear_selection: emButton::new(cc, "Clear Selection", look.clone()),
            set_high_interest: emButton::new(cc, "Set High Interest", look.clone()),
            set_medium_interest: emButton::new(cc, "Set Medium Interest", look.clone()),
            set_low_interest: emButton::new(cc, "Set Low Interest", look.clone()),
            show_first_web_pages: emButton::new(cc, "Show First Web Pages", look.clone()),
            show_all_web_pages: emButton::new(cc, "Show All Web Pages", look.clone()),
            cut_stocks_enabled: false,
            copy_stocks_enabled: false,
            delete_stocks_enabled: false,
            select_all_enabled: false,
            clear_selection_enabled: false,
            set_high_interest_enabled: false,
            set_medium_interest_enabled: false,
            set_low_interest_enabled: false,
            show_first_web_pages_enabled: false,
            show_all_web_pages_enabled: false,

            // Search group (B-001 G8 rows -749, -764, -772).
            // Captions mirror C++ emStocksControlPanel.cpp:751-789.
            find_selected: emButton::new(cc, "Find Selected", look.clone()),
            search_text: emTextField::new(cc, look.clone()),
            find_next: emButton::new(cc, "Find Next", look.clone()),
            find_previous: emButton::new(cc, "Find Previous", look),
            find_next_enabled: false,
            find_previous_enabled: false,
        }
    }
}

// ─── ChartPeriodTextOfValue ──────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel::ChartPeriodTextOfValue.
/// Returns the display text for a chart period value.
pub(crate) fn ChartPeriodTextOfValue(period: ChartPeriod) -> &'static str {
    match period {
        ChartPeriod::Week1 => "1\nweek",
        ChartPeriod::Weeks2 => "2\nweeks",
        ChartPeriod::Month1 => "1\nmonth",
        ChartPeriod::Months3 => "3\nmonths",
        ChartPeriod::Months6 => "6\nmonths",
        ChartPeriod::Year1 => "1\nyear",
        ChartPeriod::Years3 => "3\nyears",
        ChartPeriod::Years5 => "5\nyears",
        ChartPeriod::Years10 => "10\nyears",
        ChartPeriod::Years20 => "20\nyears",
    }
}

// ─── ValidateDate ────────────────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel::ValidateDate.
/// Filters a string to contain only digits and at most 2 dashes, max 32 chars.
pub(crate) fn ValidateDate(input: &str) -> String {
    let mut result = String::new();
    let mut dash_count = 0;
    for ch in input.chars() {
        if result.len() >= 32 {
            break;
        }
        if ch.is_ascii_digit() {
            result.push(ch);
        } else if ch == '-' && dash_count < 2 {
            dash_count += 1;
            result.push(ch);
        }
    }
    result
}

// ─── emStocksControlPanel ────────────────────────────────────────────────────

/// Port of C++ emStocksControlPanel.
/// D22/D23: `look` is passed from the parent; `AutoExpand` uses it to create
/// real widget instances rather than plain-value placeholders.
/// The `widgets` field mirrors C++ AutoExpand/AutoShrink lifecycle:
/// `None` when shrunk (C++ NULL pointers), `Some` when expanded.
pub struct emStocksControlPanel {
    pub(crate) look: Rc<emLook>,
    /// C++ `emStocksFileModel & FileModel;` member reference
    /// (emStocksControlPanel.h:33). (a)-justified `Rc<RefCell<>>`: shared
    /// across `emStocksFilePanel::Cycle` (owner) and `emStocksControlPanel::Cycle`
    /// (consumer); both must read/mutate the same model.
    pub(crate) file_model: Rc<RefCell<emStocksFileModel>>,
    /// C++ `emStocksConfig & Config;` member reference. (a)-justified
    /// per Task A.1 / Phase 3 — co-borrowed with FilePanel.
    pub(crate) config: Rc<RefCell<emStocksConfig>>,
    /// C++ `emStocksListBox & ListBox;` member reference. (a)-justified —
    /// FilePanel wraps the ListBox in `Rc<RefCell<>>` (Task A.1) so the
    /// ControlPanel can hold a clone of the same handle.
    pub(crate) list_box: Rc<RefCell<emStocksListBox>>,
    pub(crate) update_controls_needed: bool,
    pub(crate) widgets: Option<ControlWidgets>,
    /// D-006 first-Cycle init flag; mirrors the B-001 ListBox pattern at
    /// emStocksListBox.rs (Phase 4.5 precedent). Flips on the first Cycle
    /// where the model/config/SelectedDate signals are wired (G1/G2/G4).
    pub(crate) subscribed_init: bool,
    /// G5 (Selection) is attach-deferred — the inner emListBox is `None`
    /// until VFS-Loaded materialises it. Tracked via a separate flag so the
    /// outer `subscribed_init` can flip on the first Cycle while G5 lazily
    /// connects whenever attach completes. Mirrors B-001 design §Sequencing
    /// two-tier note (preserved-design-intent — not DIVERGED).
    pub(crate) selection_subscribed: bool,
    /// G1 cached SignalId — `Some` after first-Cycle init.
    pub(crate) model_change_sig: Option<SignalId>,
    /// G2 cached SignalId — `Some` after first-Cycle init.
    pub(crate) config_change_sig: Option<SignalId>,
    /// G4 cached SignalId — `Some` after first-Cycle init.
    pub(crate) selected_date_sig: Option<SignalId>,
    /// G5 cached SignalId — `Some` after the inner emListBox attach delivers it.
    pub(crate) selection_sig: Option<SignalId>,
    /// Widget subscribes are deferred until `AutoExpand` materialises the
    /// `ControlWidgets`. Reset to `false` on every `AutoExpand` so a fresh
    /// expand re-subscribes. Mirrors B-001 design §Sequencing.
    pub(crate) subscribed_widgets: bool,
}

impl emStocksControlPanel {
    pub fn new(
        look: Rc<emLook>,
        file_model: Rc<RefCell<emStocksFileModel>>,
        config: Rc<RefCell<emStocksConfig>>,
        list_box: Rc<RefCell<emStocksListBox>>,
    ) -> Self {
        Self {
            look,
            file_model,
            config,
            list_box,
            update_controls_needed: true,
            widgets: None,
            subscribed_init: false,
            selection_subscribed: false,
            model_change_sig: None,
            config_change_sig: None,
            selected_date_sig: None,
            selection_sig: None,
            subscribed_widgets: false,
        }
    }

    pub fn NeedsUpdate(&self) -> bool {
        self.update_controls_needed
    }

    pub fn MarkUpdated(&mut self) {
        self.update_controls_needed = false;
    }

    /// Read current widget values back into config.
    /// Polls widget state rather than using callbacks — avoids callback ownership
    /// issues. Called from Cycle (or equivalent) after widgets may have changed.
    pub fn ReadFromWidgets(&self, config: &mut emStocksConfig) {
        let widgets = match self.widgets.as_ref() {
            Some(w) => w,
            None => return,
        };

        // Text fields
        config.api_key = widgets.api_key.GetText().to_string();
        config.search_text = widgets.search_text.GetText().to_string();

        // File selection fields
        config.api_script = widgets
            .api_script
            .widget
            .GetSelectedPath()
            .to_string_lossy()
            .to_string();
        config.api_script_interpreter = widgets
            .api_script_interpreter
            .widget
            .GetSelectedPath()
            .to_string_lossy()
            .to_string();
        config.web_browser = widgets
            .web_browser
            .widget
            .GetSelectedPath()
            .to_string_lossy()
            .to_string();

        // Checkboxes
        config.auto_update_dates = widgets.auto_update_dates.IsChecked();
        config.triggering_opens_web_page = widgets.triggering_opens_web_page.IsChecked();
        config.owned_shares_first = widgets.owned_shares_first.IsChecked();

        // Scalar field (chart period index 0..9)
        let period_idx = widgets.chart_period.GetValue() as usize;
        config.chart_period = match period_idx {
            0 => ChartPeriod::Week1,
            1 => ChartPeriod::Weeks2,
            2 => ChartPeriod::Month1,
            3 => ChartPeriod::Months3,
            4 => ChartPeriod::Months6,
            5 => ChartPeriod::Year1,
            6 => ChartPeriod::Years3,
            7 => ChartPeriod::Years5,
            8 => ChartPeriod::Years10,
            _ => ChartPeriod::Years20,
        };

        // Radio groups
        if let Some(idx) = widgets.min_visible_interest_group.borrow().GetChecked() {
            config.min_visible_interest = match idx {
                0 => Interest::High,
                1 => Interest::Medium,
                _ => Interest::Low,
            };
        }
        if let Some(idx) = widgets.sorting_group.borrow().GetChecked() {
            config.sorting = match idx {
                0 => Sorting::ByName,
                1 => Sorting::ByTradeDate,
                2 => Sorting::ByInquiryDate,
                3 => Sorting::ByAchievement,
                4 => Sorting::ByOneWeekRise,
                5 => Sorting::ByThreeWeekRise,
                6 => Sorting::ByNineWeekRise,
                7 => Sorting::ByDividend,
                8 => Sorting::ByPurchaseValue,
                9 => Sorting::ByValue,
                _ => Sorting::ByDifference,
            };
        }
    }

    /// Port of C++ AutoExpand.
    /// D23: Creates real widget instances using the stored `Rc<emLook>`.
    pub fn AutoExpand<C: emcore::emEngineCtx::ConstructCtx>(&mut self, cc: &mut C) {
        let look = self.look.clone();
        self.widgets = Some(ControlWidgets::new(cc, look));
        self.update_controls_needed = true;
        // B-001-followup B.3: re-subscribe widget signals on every expand.
        self.subscribed_widgets = false;
    }

    /// Port of C++ AutoShrink.
    /// D23: Drops all widget instances (C++ equivalent: set to NULL).
    pub fn AutoShrink(&mut self) {
        self.widgets = None;
    }

    /// Port of C++ IsAutoExpanded.
    pub fn IsAutoExpanded(&self) -> bool {
        self.widgets.is_some()
    }

    /// Port of C++ UpdateControls.
    // C++ reads from owned Config/FileModel/ListBox references. Rust passes them explicitly — avoids shared mutable state.
    pub fn UpdateControls(
        &mut self,
        config: &emStocksConfig,
        rec: &emStocksRec,
        list_box: &emStocksListBox,
        ctx: &mut emcore::emEngineCtx::PanelCtx<'_>,
    ) {
        self.update_controls_needed = false;

        let widgets = match self.widgets.as_mut() {
            Some(w) => w,
            None => return,
        };

        // Sync config values to widget state
        widgets.api_script.UpdateControls(config);
        widgets.api_script_interpreter.UpdateControls(config);
        widgets.api_key.SetText(&config.api_key);
        widgets.web_browser.UpdateControls(config);

        widgets
            .auto_update_dates
            .SetChecked(config.auto_update_dates, ctx);
        widgets
            .triggering_opens_web_page
            .SetChecked(config.triggering_opens_web_page, ctx);
        let cp_idx = chart_period_to_index(config.chart_period);
        widgets.chart_period.SetValue(cp_idx, ctx);
        widgets.chart_period_text = ChartPeriodTextOfValue(config.chart_period);

        let interest_idx = interest_to_index(config.min_visible_interest);
        widgets
            .min_visible_interest_group
            .borrow_mut()
            .SetChecked(interest_idx, ctx);

        // Update category panels with current stock data
        let countries_ext = widgets.visible_countries.extractor();
        widgets
            .visible_countries
            .UpdateItems(&rec.stocks, countries_ext);
        let sectors_ext = widgets.visible_sectors.extractor();
        widgets
            .visible_sectors
            .UpdateItems(&rec.stocks, sectors_ext);
        let collections_ext = widgets.visible_collections.extractor();
        widgets
            .visible_collections
            .UpdateItems(&rec.stocks, collections_ext);

        let sorting_idx = sorting_to_index(config.sorting);
        widgets
            .sorting_group
            .borrow_mut()
            .SetChecked(sorting_idx, ctx);
        widgets
            .owned_shares_first
            .SetChecked(config.owned_shares_first, ctx);

        // History navigation enabled state
        widgets.go_back_in_history_enabled = !rec
            .GetPricesDateBefore(list_box.GetSelectedDate())
            .is_empty();
        widgets.go_forward_in_history_enabled = !rec
            .GetPricesDateAfter(list_box.GetSelectedDate())
            .is_empty();

        widgets.selected_date = ValidateDate(list_box.GetSelectedDate());
        widgets.selected_date_field.SetText(&widgets.selected_date);

        // Calculate totals from owned visible stocks
        let mut total_purchase = 0.0_f64;
        let mut total_current = 0.0_f64;
        let mut total_purchase_valid = true;
        let mut total_current_valid = true;

        for &stock_idx in &list_box.visible_items {
            if let Some(stock_rec) = rec.stocks.get(stock_idx) {
                if !stock_rec.owning_shares {
                    continue;
                }
                match stock_rec.GetTradeValue() {
                    Some(d) => total_purchase += d,
                    None => total_purchase_valid = false,
                }
                match stock_rec.GetValueOfDate(list_box.GetSelectedDate()) {
                    Some(d) => total_current += d,
                    None => total_current_valid = false,
                }
            }
        }

        widgets.total_purchase_value = if total_purchase_valid {
            PaymentPriceToString(total_purchase)
        } else {
            String::new()
        };

        widgets.total_current_value = if total_current_valid {
            PaymentPriceToString(total_current)
        } else {
            String::new()
        };

        widgets.total_difference_value = if total_purchase_valid && total_current_valid {
            PaymentPriceToString(total_current - total_purchase)
        } else {
            String::new()
        };

        // Enable/disable buttons based on selection
        let selection_count = list_box.GetSelectionCount();
        let has_selection = selection_count > 0;

        widgets.cut_stocks_enabled = has_selection;
        widgets.copy_stocks_enabled = has_selection;
        widgets.delete_stocks_enabled = has_selection;
        widgets.select_all_enabled = selection_count < list_box.visible_items.len();
        widgets.clear_selection_enabled = has_selection;
        widgets.set_high_interest_enabled = has_selection;
        widgets.set_medium_interest_enabled = has_selection;
        widgets.set_low_interest_enabled = has_selection;
        widgets.show_first_web_pages_enabled = has_selection;
        widgets.show_all_web_pages_enabled = has_selection;

        // Search
        widgets.search_text.SetText(&config.search_text);
        let has_search_text = !config.search_text.is_empty();
        widgets.find_next_enabled = has_search_text;
        widgets.find_previous_enabled = has_search_text;
    }
}

// ─── PanelBehavior ───────────────────────────────────────────────────────────

/// Port of C++ `emStocksControlPanel::Cycle` (emStocksControlPanel.cpp:85-220).
///
/// B-001-followup Phase B: D-006 wiring. Three tiers of subscribe init:
///   - `subscribed_init`: G1 (FileModel.ChangeSignal), G2 (Config.ChangeSignal),
///     G4 (ListBox.SelectedDateSignal). Always available — the parent provides
///     the three Rc<RefCell<>> refs at construction.
///   - `selection_subscribed`: G5 (ListBox.SelectionSignal). Attach-deferred —
///     the inner emListBox is `None` until VFS-Loaded materialises it; we
///     re-attempt every Cycle until `Some(_)`.
///   - `subscribed_widgets`: G7/G8 widget signals. Reset to `false` on every
///     `AutoExpand`, run on the first Cycle observing `widgets.is_some()`.
///
/// Reactions mirror C++ `emStocksControlPanel::Cycle` (cpp:85-220) branch
/// for branch (M-001 verified). The 4 model/config/list-box signals all
/// fold into `update_controls_needed = true`. The 28 widget signals are
/// (mostly) immediate Config or ListBox mutations.
impl emcore::emPanel::PanelBehavior for emStocksControlPanel {
    fn Cycle(
        &mut self,
        ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        pctx: &mut emcore::emEngineCtx::PanelCtx,
    ) -> bool {
        // ── Tier 1: G1/G2/G4 first-Cycle init ─────────────────────────────
        if !self.subscribed_init {
            let eid = ectx.engine_id;

            let model_sig = self.file_model.borrow().GetChangeSignal(ectx);
            ectx.connect(model_sig, eid);
            self.model_change_sig = Some(model_sig);

            let cfg_sig = self.config.borrow().GetChangeSignal(ectx);
            ectx.connect(cfg_sig, eid);
            self.config_change_sig = Some(cfg_sig);

            let sd_sig = self.list_box.borrow().GetSelectedDateSignal(ectx);
            ectx.connect(sd_sig, eid);
            self.selected_date_sig = Some(sd_sig);

            self.subscribed_init = true;
        }

        // ── Tier 2: G5 attach-deferred Selection subscribe ────────────────
        if !self.selection_subscribed {
            if let Some(sel_sig) = self.list_box.borrow().GetSelectionSignal() {
                let eid = ectx.engine_id;
                ectx.connect(sel_sig, eid);
                self.selection_sig = Some(sel_sig);
                self.selection_subscribed = true;
            }
        }

        // ── Tier 3: G7/G8 widget subscribes (after AutoExpand) ────────────
        // Subscribe block scoped before any other widget access so the
        // `widgets` borrow does not collide with reaction code below.
        if !self.subscribed_widgets {
            if let Some(w) = self.widgets.as_ref() {
                let eid = ectx.engine_id;
                // C++ source order — emStocksControlPanel.cpp:111-219.
                ectx.connect(w.api_key.text_signal, eid);
                ectx.connect(w.auto_update_dates.check_signal, eid);
                ectx.connect(w.triggering_opens_web_page.check_signal, eid);
                ectx.connect(w.chart_period.value_signal, eid);
                ectx.connect(w.min_visible_interest_group.borrow().check_signal, eid);
                ectx.connect(w.sorting_group.borrow().check_signal, eid);
                // Row -566 — C++ cpp:135 wires `OwnedSharesFirst->GetClickSignal()`
                // (B-001 design C-3). The Rust `emCheckBox` exposes only
                // `check_signal` (no inherited GetClickSignal accessor); using
                // it preserves the toggle reaction observably equivalently for
                // keyboard- and click-driven toggles. TODO: when emCheckBox
                // gains a click-signal accessor (B-001 prereq miss), swap.
                ectx.connect(w.owned_shares_first.check_signal, eid);
                ectx.connect(w.fetch_share_prices.click_signal, eid);
                ectx.connect(w.delete_share_prices.click_signal, eid);
                ectx.connect(w.go_back_in_history.click_signal, eid);
                ectx.connect(w.go_forward_in_history.click_signal, eid);
                ectx.connect(w.selected_date_field.text_signal, eid);
                ectx.connect(w.new_stock.click_signal, eid);
                ectx.connect(w.cut_stocks.click_signal, eid);
                ectx.connect(w.copy_stocks.click_signal, eid);
                ectx.connect(w.paste_stocks.click_signal, eid);
                ectx.connect(w.delete_stocks.click_signal, eid);
                ectx.connect(w.select_all.click_signal, eid);
                ectx.connect(w.clear_selection.click_signal, eid);
                ectx.connect(w.set_low_interest.click_signal, eid);
                ectx.connect(w.set_medium_interest.click_signal, eid);
                ectx.connect(w.set_high_interest.click_signal, eid);
                ectx.connect(w.show_first_web_pages.click_signal, eid);
                ectx.connect(w.show_all_web_pages.click_signal, eid);
                ectx.connect(w.find_selected.click_signal, eid);
                ectx.connect(w.search_text.text_signal, eid);
                ectx.connect(w.find_next.click_signal, eid);
                ectx.connect(w.find_previous.click_signal, eid);
                self.subscribed_widgets = true;
            }
        }

        // ── Reactions, in C++ source order (cpp:93-219) ──────────────────
        // Group 1: model/config/list-box → UpdateControlsNeeded.
        if self
            .model_change_sig
            .map(|s| ectx.IsSignaled(s))
            .unwrap_or(false)
        {
            self.update_controls_needed = true;
        }
        if self
            .config_change_sig
            .map(|s| ectx.IsSignaled(s))
            .unwrap_or(false)
        {
            self.update_controls_needed = true;
        }
        if self
            .selection_sig
            .map(|s| ectx.IsSignaled(s))
            .unwrap_or(false)
        {
            self.update_controls_needed = true;
        }
        if self
            .selected_date_sig
            .map(|s| ectx.IsSignaled(s))
            .unwrap_or(false)
        {
            self.update_controls_needed = true;
        }

        // Group 2: widget signals → Config / ListBox mutations.
        // Guarded on `widgets.is_some()` because all reads/writes go through
        // the `widgets` field.
        if let Some(w) = self.widgets.as_ref() {
            // Read out which signals fired before mutating, to avoid borrow
            // conflicts between the signal queries (need ectx) and the
            // mutations (need config/list_box mut borrows).
            let api_key_fired = ectx.IsSignaled(w.api_key.text_signal);
            let api_key_text = if api_key_fired {
                Some(w.api_key.GetText().to_string())
            } else {
                None
            };
            let auto_update_fired = ectx.IsSignaled(w.auto_update_dates.check_signal);
            let auto_update_val = w.auto_update_dates.IsChecked();
            let trig_fired = ectx.IsSignaled(w.triggering_opens_web_page.check_signal);
            let trig_val = w.triggering_opens_web_page.IsChecked();
            let chart_fired = ectx.IsSignaled(w.chart_period.value_signal);
            let chart_idx = w.chart_period.GetValue() as usize;
            let interest_fired =
                ectx.IsSignaled(w.min_visible_interest_group.borrow().check_signal);
            let interest_idx = w.min_visible_interest_group.borrow().GetChecked();
            let sorting_fired = ectx.IsSignaled(w.sorting_group.borrow().check_signal);
            let sorting_idx = w.sorting_group.borrow().GetChecked();
            let owned_first_fired = ectx.IsSignaled(w.owned_shares_first.check_signal);
            let owned_first_val = w.owned_shares_first.IsChecked();
            let fetch_fired = ectx.IsSignaled(w.fetch_share_prices.click_signal);
            let delete_prices_fired = ectx.IsSignaled(w.delete_share_prices.click_signal);
            let go_back_fired = ectx.IsSignaled(w.go_back_in_history.click_signal);
            let go_forward_fired = ectx.IsSignaled(w.go_forward_in_history.click_signal);
            let sel_date_fired = ectx.IsSignaled(w.selected_date_field.text_signal);
            let sel_date_text = if sel_date_fired {
                Some(w.selected_date_field.GetText().to_string())
            } else {
                None
            };
            let new_stock_fired = ectx.IsSignaled(w.new_stock.click_signal);
            let cut_fired = ectx.IsSignaled(w.cut_stocks.click_signal);
            let copy_fired = ectx.IsSignaled(w.copy_stocks.click_signal);
            let paste_fired = ectx.IsSignaled(w.paste_stocks.click_signal);
            let delete_stocks_fired = ectx.IsSignaled(w.delete_stocks.click_signal);
            let select_all_fired = ectx.IsSignaled(w.select_all.click_signal);
            let clear_sel_fired = ectx.IsSignaled(w.clear_selection.click_signal);
            let set_low_fired = ectx.IsSignaled(w.set_low_interest.click_signal);
            let set_med_fired = ectx.IsSignaled(w.set_medium_interest.click_signal);
            let set_high_fired = ectx.IsSignaled(w.set_high_interest.click_signal);
            let show_first_fired = ectx.IsSignaled(w.show_first_web_pages.click_signal);
            let show_all_fired = ectx.IsSignaled(w.show_all_web_pages.click_signal);
            let find_sel_fired = ectx.IsSignaled(w.find_selected.click_signal);
            let search_text_fired = ectx.IsSignaled(w.search_text.text_signal);
            let search_text_val = if search_text_fired {
                Some(w.search_text.GetText().to_string())
            } else {
                None
            };
            let find_next_fired = ectx.IsSignaled(w.find_next.click_signal);
            let find_prev_fired = ectx.IsSignaled(w.find_previous.click_signal);

            // ── Config mutations ──────────────────────────────────────────
            // C++ writes Config fields directly. The Rust analogue mutates
            // `self.config.borrow_mut()`. Each setter ends with `Signal()`
            // (D-007) by virtue of the Config setter contract; here we
            // assign the field directly, which mirrors C++ but does NOT
            // fire the Config ChangeSignal. C++ uses `=` writes on a
            // Config that inherits emConfigModel::Signal indirectly. The
            // Rust port writes the field; downstream consumers re-cycle on
            // their own signals (widget → cycle → config write → fire
            // Config.Signal so listeners react). We fire `Config.Signal`
            // explicitly when any Config field is written.
            let mut config_changed = false;
            if let Some(text) = api_key_text {
                self.config.borrow_mut().api_key = text;
                config_changed = true;
            }
            if auto_update_fired {
                self.config.borrow_mut().auto_update_dates = auto_update_val;
                config_changed = true;
            }
            if trig_fired {
                self.config.borrow_mut().triggering_opens_web_page = trig_val;
                config_changed = true;
            }
            if chart_fired {
                self.config.borrow_mut().chart_period = match chart_idx {
                    0 => ChartPeriod::Week1,
                    1 => ChartPeriod::Weeks2,
                    2 => ChartPeriod::Month1,
                    3 => ChartPeriod::Months3,
                    4 => ChartPeriod::Months6,
                    5 => ChartPeriod::Year1,
                    6 => ChartPeriod::Years3,
                    7 => ChartPeriod::Years5,
                    8 => ChartPeriod::Years10,
                    _ => ChartPeriod::Years20,
                };
                config_changed = true;
            }
            if interest_fired {
                if let Some(idx) = interest_idx {
                    self.config.borrow_mut().min_visible_interest = match idx {
                        0 => Interest::High,
                        1 => Interest::Medium,
                        _ => Interest::Low,
                    };
                    config_changed = true;
                }
            }
            if sorting_fired {
                if let Some(idx) = sorting_idx {
                    self.config.borrow_mut().sorting = match idx {
                        0 => Sorting::ByName,
                        1 => Sorting::ByTradeDate,
                        2 => Sorting::ByInquiryDate,
                        3 => Sorting::ByAchievement,
                        4 => Sorting::ByOneWeekRise,
                        5 => Sorting::ByThreeWeekRise,
                        6 => Sorting::ByNineWeekRise,
                        7 => Sorting::ByDividend,
                        8 => Sorting::ByPurchaseValue,
                        9 => Sorting::ByValue,
                        _ => Sorting::ByDifference,
                    };
                    config_changed = true;
                }
            }
            if owned_first_fired {
                self.config.borrow_mut().owned_shares_first = owned_first_val;
                config_changed = true;
            }
            if let Some(text) = search_text_val {
                self.config.borrow_mut().search_text = text;
                config_changed = true;
            }

            if config_changed {
                // D-007: synchronous Signal fire on Config mutation. Reads
                // through `self.config` borrow (immut) — Signal API is
                // `&self`-shaped per emStocksConfig.
                let cfg_sig = self.config.borrow().GetChangeSignal(ectx);
                ectx.fire(cfg_sig);
            }

            // ── ListBox mutations ─────────────────────────────────────────
            // For methods that need `&mut emStocksRec` we go through
            // `self.file_model.borrow_mut().GetWritableRec(ectx)`. For
            // immutable access we use `GetRec()`. ConstructCtx-bound methods
            // accept `pctx`.
            //
            // C++ `StartToFetchSharePrices` is a parent-side action (creates
            // a fetch dialog); the Rust port has only `GetVisibleStockIds`.
            // We log the row but defer the dialog construction (B-017
            // territory). This is a known gap — the subscribe wires
            // mechanically; the reaction is a structural stub.
            let _ = fetch_fired; // TODO: wire to FetchPricesDialog when emStocksFilePanel surfaces it.

            if delete_prices_fired {
                let lb_rc = self.list_box.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                lb_rc.borrow().DeleteSharePrices(rec);
            }
            if go_back_fired {
                let model = self.file_model.borrow();
                self.list_box
                    .borrow_mut()
                    .GoBackInHistory(ectx, model.GetRec());
            }
            if go_forward_fired {
                let model = self.file_model.borrow();
                self.list_box
                    .borrow_mut()
                    .GoForwardInHistory(ectx, model.GetRec());
            }
            if let Some(text) = sel_date_text {
                self.list_box.borrow_mut().SetSelectedDate(ectx, &text);
            }
            if new_stock_fired {
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                let cfg = self.config.borrow();
                self.list_box.borrow_mut().NewStock(rec, &cfg);
            }
            if cut_fired {
                let lb_rc = self.list_box.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                lb_rc.borrow_mut().CutStocks(pctx, rec, true);
            }
            if copy_fired {
                let lb_rc = self.list_box.clone();
                let model = self.file_model.borrow();
                lb_rc.borrow().CopyStocks(model.GetRec());
            }
            if paste_fired {
                let lb_rc = self.list_box.clone();
                let cfg_rc = self.config.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                let cfg = cfg_rc.borrow();
                let _ = lb_rc.borrow_mut().PasteStocks(pctx, rec, &cfg, true);
            }
            if delete_stocks_fired {
                let lb_rc = self.list_box.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                lb_rc.borrow_mut().DeleteStocks(pctx, rec, true);
            }
            if select_all_fired {
                // C++ ListBox::SelectAll selects all visible rows. The Rust
                // emStocksListBox does not expose SelectAll; iterate
                // visible_items and Select each. UpdateControls reflects
                // the selection-count enable state next pass.
                let lb_rc = self.list_box.clone();
                let mut lb = lb_rc.borrow_mut();
                let count = lb.visible_items.len();
                for i in 0..count {
                    lb.Select(i);
                }
            }
            if clear_sel_fired {
                self.list_box.borrow_mut().ClearSelection();
            }
            if set_low_fired {
                let lb_rc = self.list_box.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                lb_rc
                    .borrow_mut()
                    .SetInterest(pctx, rec, Interest::Low, true);
            }
            if set_med_fired {
                let lb_rc = self.list_box.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                lb_rc
                    .borrow_mut()
                    .SetInterest(pctx, rec, Interest::Medium, true);
            }
            if set_high_fired {
                let lb_rc = self.list_box.clone();
                let mut model = self.file_model.borrow_mut();
                let rec = model.GetWritableRec(ectx);
                lb_rc
                    .borrow_mut()
                    .SetInterest(pctx, rec, Interest::High, true);
            }
            if show_first_fired {
                let lb_rc = self.list_box.clone();
                let model = self.file_model.borrow();
                lb_rc.borrow().ShowFirstWebPages(model.GetRec());
            }
            if show_all_fired {
                let lb_rc = self.list_box.clone();
                let model = self.file_model.borrow();
                lb_rc.borrow().ShowAllWebPages(model.GetRec());
            }
            if find_sel_fired {
                let lb_rc = self.list_box.clone();
                let cfg_rc = self.config.clone();
                let model = self.file_model.borrow();
                let mut cfg = cfg_rc.borrow_mut();
                let _ = lb_rc.borrow_mut().FindSelected(model.GetRec(), &mut cfg);
            }
            if find_next_fired {
                let lb_rc = self.list_box.clone();
                let cfg_rc = self.config.clone();
                let model = self.file_model.borrow();
                let cfg = cfg_rc.borrow();
                let _ = lb_rc.borrow_mut().FindNext(model.GetRec(), &cfg);
            }
            if find_prev_fired {
                let lb_rc = self.list_box.clone();
                let cfg_rc = self.config.clone();
                let model = self.file_model.borrow();
                let cfg = cfg_rc.borrow();
                let _ = lb_rc.borrow_mut().FindPrevious(model.GetRec(), &cfg);
            }
        }

        // ── Final: UpdateControls if needed (C++ cpp:218) ─────────────────
        // Borrow conflict avoidance: `UpdateControls` takes `&mut self`, but
        // we need immutable borrows of the three Rcs. Clone the Rc handles
        // first; the underlying RefCells survive the `&mut self` call.
        if self.update_controls_needed && self.widgets.is_some() {
            let model_rc = self.file_model.clone();
            let config_rc = self.config.clone();
            let list_box_rc = self.list_box.clone();
            let cfg = config_rc.borrow();
            let model = model_rc.borrow();
            let lb = list_box_rc.borrow();
            self.UpdateControls(&cfg, model.GetRec(), &lb, pctx);
        }
        false
    }
}

#[cfg(test)]
impl emStocksControlPanel {
    /// Test-only fixture. The production constructor requires a parent-supplied
    /// `Rc<RefCell<>>` for FileModel/Config/ListBox; tests fabricate dummies.
    pub(crate) fn for_test() -> Self {
        Self::new(
            emLook::new(),
            Rc::new(RefCell::new(emStocksFileModel::new(
                std::path::PathBuf::from("/tmp/control_panel_test.emStocks"),
            ))),
            Rc::new(RefCell::new(emStocksConfig::default())),
            Rc::new(RefCell::new(emStocksListBox::new())),
        )
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::{DeferredAction, DropOnlySignalCtx, InitCtx};
    use emcore::emScheduler::EngineScheduler;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<emcore::emContext::emContext>,
        pa: Rc<RefCell<Vec<emcore::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: emcore::emContext::emContext::NewRoot(),
                pa: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                view_context: None,
                pending_actions: &self.pa,
            }
        }
    }

    use crate::emStocksRec::StockRec;

    fn make_panel() -> emStocksControlPanel {
        emStocksControlPanel::for_test()
    }

    /// B-001-followup A.3 — first-Cycle latch flips `subscribed_init`.
    /// Phase B will replace the no-op gated body with the 37 deferred D-006
    /// row subscribes; this test pins the latch contract until then.
    #[test]
    fn control_panel_first_cycle_flips_subscribed_init() {
        use emcore::emEngine::Priority;
        use emcore::emPanelScope::PanelScope;
        use emcore::test_view_harness::TestViewHarness;

        struct NoopEngine;
        impl emcore::emEngine::emEngine for NoopEngine {
            fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }

        let mut h = TestViewHarness::new();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        let mut panel = emStocksControlPanel::for_test();
        assert!(!panel.subscribed_init);

        let mut tree = emcore::emPanelTree::PanelTree::new();
        let id = tree.create_root("cp", false);
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = <emStocksControlPanel as emcore::emPanel::PanelBehavior>::Cycle(
                &mut panel, &mut ectx, &mut pctx,
            );
        }

        assert!(
            panel.subscribed_init,
            "first Cycle must flip subscribed_init"
        );

        h.scheduler.remove_engine(eid);
        h.scheduler.flush_signals_for_test();
    }

    /// B-001-followup A.2 — verify the constructor wires the three member-ref
    /// `Rc<RefCell<>>`s through (strong_count goes to 2 once held both by the
    /// test scope and by the panel).
    #[test]
    fn control_panel_holds_member_refs() {
        let model = Rc::new(RefCell::new(emStocksFileModel::new(
            std::path::PathBuf::from("/tmp/fp_a2.emStocks"),
        )));
        let config = Rc::new(RefCell::new(emStocksConfig::default()));
        let list_box = Rc::new(RefCell::new(emStocksListBox::new()));
        let look = emLook::new();
        let _panel =
            emStocksControlPanel::new(look, model.clone(), config.clone(), list_box.clone());
        assert_eq!(Rc::strong_count(&model), 2);
        assert_eq!(Rc::strong_count(&config), 2);
        assert_eq!(Rc::strong_count(&list_box), 2);
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
    fn control_panel_new() {
        let mut __init = TestInit::new();
        let panel = make_panel();
        assert!(panel.update_controls_needed);
        assert!(!panel.IsAutoExpanded());
    }

    #[test]
    fn file_field_panel_new() {
        let mut __init = TestInit::new();
        let panel = FileFieldPanel::new(&mut __init.ctx(), FileFieldType::Script, "Script");
        assert_eq!(panel.field_type, FileFieldType::Script);
        assert!(panel.update_controls_needed);
        // widget starts with no selection (empty path)
        assert!(panel.widget.GetSelectedNames().is_empty());
    }

    #[test]
    fn category_panel_update_items() {
        let mut __init = TestInit::new();
        let mut cp = ControlCategoryPanel::new("Countries", CategoryType::Country);
        let mut stocks = vec![
            StockRec::default(),
            StockRec::default(),
            StockRec::default(),
        ];
        stocks[0].country = "US".to_string();
        stocks[1].country = "DE".to_string();
        stocks[2].country = "US".to_string(); // duplicate

        cp.UpdateItems(&stocks, |s| &s.country);
        assert_eq!(cp.sorted_items, vec!["DE", "US"]); // sorted, deduplicated
    }

    #[test]
    fn auto_expand_creates_widgets() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        assert!(!panel.IsAutoExpanded());

        panel.AutoExpand(&mut __init.ctx());
        assert!(panel.IsAutoExpanded());
        assert!(panel.update_controls_needed);

        let widgets = panel.widgets.as_ref().unwrap();
        assert_eq!(widgets.api_script.field_type, FileFieldType::Script);
        assert_eq!(
            widgets.api_script_interpreter.field_type,
            FileFieldType::Interpreter
        );
        assert_eq!(widgets.web_browser.field_type, FileFieldType::Browser);
        // chart_period starts at default index
        assert!(
            (widgets.chart_period.GetValue() - chart_period_to_index(ChartPeriod::default())).abs()
                < f64::EPSILON
        );
        // interest and sorting groups start with no selection
        assert!(!widgets.auto_update_dates.IsChecked());
        assert!(!widgets.triggering_opens_web_page.IsChecked());
        assert!(!widgets.owned_shares_first.IsChecked());
    }

    /// B-001 Phase 2 G8 — confirms 20 emButton + 1 emTextField widget instances
    /// were instantiated by AutoExpand and that their signals are allocated
    /// (non-null, distinct), making them ready for Phase 4 D-006 subscribe wiring.
    #[test]
    fn auto_expand_creates_g8_widget_instances() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());
        let w = panel.widgets.as_ref().unwrap();

        let click_sigs = [
            w.fetch_share_prices.click_signal,
            w.delete_share_prices.click_signal,
            w.go_back_in_history.click_signal,
            w.go_forward_in_history.click_signal,
            w.new_stock.click_signal,
            w.cut_stocks.click_signal,
            w.copy_stocks.click_signal,
            w.paste_stocks.click_signal,
            w.delete_stocks.click_signal,
            w.select_all.click_signal,
            w.clear_selection.click_signal,
            w.set_high_interest.click_signal,
            w.set_medium_interest.click_signal,
            w.set_low_interest.click_signal,
            w.show_first_web_pages.click_signal,
            w.show_all_web_pages.click_signal,
            w.find_selected.click_signal,
            w.find_next.click_signal,
            w.find_previous.click_signal,
        ];
        assert_eq!(click_sigs.len(), 19, "19 emButton click signals expected");
        // All click signals must be distinct (one signal per button) — distinctness
        // implies allocation succeeded for each.
        let unique: std::collections::HashSet<_> = click_sigs.iter().copied().collect();
        assert_eq!(unique.len(), 19, "click signals must be distinct");
        // SelectedDate emTextField widget signal must also be distinct from all
        // click signals — captioned widget, lives separately from the cached
        // `selected_date: String` display value.
        assert!(
            !unique.contains(&w.selected_date_field.text_signal),
            "selected_date_field text signal collides with a button click signal"
        );
        assert_eq!(w.selected_date, "");

        // Captions sanity-check a sample of buttons (mirrors C++ source-order).
        assert_eq!(w.fetch_share_prices.GetCaption(), "Fetch\nPrices");
        assert_eq!(w.delete_share_prices.GetCaption(), "Delete Prices");
        assert_eq!(w.new_stock.GetCaption(), "New");
        assert_eq!(w.find_previous.GetCaption(), "Find Previous");
    }

    #[test]
    fn auto_shrink_destroys_widgets() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());
        assert!(panel.IsAutoExpanded());

        panel.AutoShrink();
        assert!(!panel.IsAutoExpanded());
        assert!(panel.widgets.is_none());
    }

    #[test]
    fn auto_expand_shrink_cycle() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();

        // First expand
        panel.AutoExpand(&mut __init.ctx());
        assert!(panel.IsAutoExpanded());

        // Shrink
        panel.AutoShrink();
        assert!(!panel.IsAutoExpanded());

        // Re-expand
        panel.AutoExpand(&mut __init.ctx());
        assert!(panel.IsAutoExpanded());
        assert!(panel.update_controls_needed);
    }

    #[test]
    fn chart_period_text_of_value_all_variants() {
        let mut __init = TestInit::new();
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Week1), "1\nweek");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Weeks2), "2\nweeks");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Month1), "1\nmonth");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Months3), "3\nmonths");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Months6), "6\nmonths");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Year1), "1\nyear");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Years3), "3\nyears");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Years5), "5\nyears");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Years10), "10\nyears");
        assert_eq!(ChartPeriodTextOfValue(ChartPeriod::Years20), "20\nyears");
    }

    #[test]
    fn validate_date_filters_correctly() {
        let mut __init = TestInit::new();
        assert_eq!(ValidateDate("2024-06-15"), "2024-06-15");
        assert_eq!(ValidateDate("abc"), "");
        assert_eq!(ValidateDate("2024--06-15"), "2024--0615"); // only 2 dashes
        assert_eq!(ValidateDate("12-34-56-78"), "12-34-5678"); // third dash dropped
    }

    #[test]
    fn validate_date_length_limit() {
        let mut __init = TestInit::new();
        let long = "1".repeat(50);
        assert_eq!(ValidateDate(&long).len(), 32);
    }

    #[test]
    fn update_controls_syncs_config() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig {
            api_key: "test-key".to_string(),
            auto_update_dates: true,
            triggering_opens_web_page: true,
            chart_period: ChartPeriod::Months3,
            min_visible_interest: Interest::High,
            sorting: Sorting::ByTradeDate,
            owned_shares_first: true,
            search_text: "find me".to_string(),
            ..Default::default()
        };
        let rec = emStocksRec::default();
        let list_box = emStocksListBox::new();

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert_eq!(w.api_key.GetText(), "test-key");
        assert!(w.auto_update_dates.IsChecked());
        assert!(w.triggering_opens_web_page.IsChecked());
        assert!(
            (w.chart_period.GetValue() - chart_period_to_index(ChartPeriod::Months3)).abs()
                < f64::EPSILON
        );
        assert_eq!(
            w.min_visible_interest_group.borrow().GetChecked(),
            Some(interest_to_index(Interest::High))
        );
        assert_eq!(
            w.sorting_group.borrow().GetChecked(),
            Some(sorting_to_index(Sorting::ByTradeDate))
        );
        assert!(w.owned_shares_first.IsChecked());
        assert_eq!(w.search_text.GetText(), "find me");
        assert!(w.find_next_enabled);
        assert!(w.find_previous_enabled);
        assert!(!panel.update_controls_needed);
    }

    #[test]
    fn update_controls_empty_search_disables_find() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig::default(); // search_text is empty
        let rec = emStocksRec::default();
        let list_box = emStocksListBox::new();

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(!w.find_next_enabled);
        assert!(!w.find_previous_enabled);
    }

    #[test]
    fn update_controls_selection_enables_buttons() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig::default();
        let rec = emStocksRec::default();
        let list_box = emStocksListBox::new();

        // No selection
        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));
        let w = panel.widgets.as_ref().unwrap();
        assert!(!w.cut_stocks_enabled);
        assert!(!w.copy_stocks_enabled);
        assert!(!w.delete_stocks_enabled);
        assert!(!w.clear_selection_enabled);
        assert!(!w.set_high_interest_enabled);
        assert!(!w.show_first_web_pages_enabled);
    }

    #[test]
    fn update_controls_with_selection() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig::default();
        let mut rec = emStocksRec::default();
        rec.stocks.push(StockRec::default());
        rec.stocks.push(StockRec::default());

        let mut list_box = emStocksListBox::new();
        list_box.visible_items = vec![0, 1];
        list_box.Select(0);

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(w.cut_stocks_enabled);
        assert!(w.copy_stocks_enabled);
        assert!(w.delete_stocks_enabled);
        assert!(w.clear_selection_enabled);
        assert!(w.set_high_interest_enabled);
        assert!(w.set_medium_interest_enabled);
        assert!(w.set_low_interest_enabled);
        assert!(w.show_first_web_pages_enabled);
        assert!(w.show_all_web_pages_enabled);
        // Not all selected, so select_all should be enabled
        assert!(w.select_all_enabled);
    }

    #[test]
    fn update_controls_all_selected_disables_select_all() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig::default();
        let mut rec = emStocksRec::default();
        rec.stocks.push(StockRec::default());

        let mut list_box = emStocksListBox::new();
        list_box.visible_items = vec![0];
        list_box.Select(0);

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));

        let w = panel.widgets.as_ref().unwrap();
        assert!(!w.select_all_enabled); // all already selected
    }

    #[test]
    fn update_controls_total_values_with_owned_stocks() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig::default();
        let mut rec = emStocksRec::default();

        // Stock with owned shares: 10 shares at $5 trade price
        let mut stock = StockRec::default();
        stock.owning_shares = true;
        stock.own_shares = "10".to_string();
        stock.trade_price = "5.00".to_string();
        // Need a price for the selected date to compute current value
        stock.last_price_date = "2024-06-15".to_string();
        stock.prices = "A".to_string(); // price byte 'A' = 65-32 = 33 -> 3.30
        rec.stocks.push(stock);

        let mut list_box = emStocksListBox::new();
        list_box.visible_items = vec![0];
        list_box.SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-15");

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));

        let w = panel.widgets.as_ref().unwrap();
        // trade_value = 10 * 5.00 = 50.00
        assert_eq!(w.total_purchase_value, "50.00");
    }

    #[test]
    fn update_controls_no_owned_stocks_zeros() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let config = emStocksConfig::default();
        let mut rec = emStocksRec::default();

        // Stock without owned shares
        let stock = StockRec::default();
        rec.stocks.push(stock);

        let mut list_box = emStocksListBox::new();
        list_box.visible_items = vec![0];

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));

        let w = panel.widgets.as_ref().unwrap();
        // No owned stocks, so totals are valid but 0
        assert_eq!(w.total_purchase_value, "0.00");
        assert_eq!(w.total_current_value, "0.00");
        assert_eq!(w.total_difference_value, "0.00");
    }

    #[test]
    fn update_controls_not_expanded_is_noop() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        // Don't call AutoExpand

        let config = emStocksConfig::default();
        let rec = emStocksRec::default();
        let list_box = emStocksListBox::new();

        with_scratch_ctx(|ctx| panel.UpdateControls(&config, &rec, &list_box, ctx));
        // Should not panic, just returns early
        assert!(!panel.update_controls_needed);
        assert!(panel.widgets.is_none());
    }

    #[test]
    fn file_field_panel_update_controls() {
        let mut __init = TestInit::new();
        let config = emStocksConfig {
            api_script: "/path/to/script.pl".to_string(),
            api_script_interpreter: "python3".to_string(),
            web_browser: "chromium".to_string(),
            ..Default::default()
        };

        let mut script = FileFieldPanel::new(&mut __init.ctx(), FileFieldType::Script, "Script");
        script.UpdateControls(&config);
        // widget should reflect the path
        assert!(!script.update_controls_needed);

        let mut interp =
            FileFieldPanel::new(&mut __init.ctx(), FileFieldType::Interpreter, "Interpreter");
        interp.UpdateControls(&config);
        assert!(!interp.update_controls_needed);

        let mut browser = FileFieldPanel::new(&mut __init.ctx(), FileFieldType::Browser, "Browser");
        browser.UpdateControls(&config);
        assert!(!browser.update_controls_needed);
    }

    #[test]
    fn category_panel_types() {
        let mut __init = TestInit::new();
        let cp = ControlCategoryPanel::new("Countries", CategoryType::Country);
        assert_eq!(cp.category_type, CategoryType::Country);
        assert_eq!(cp.caption, "Countries");
        assert!(cp.sorted_items.is_empty());
    }

    #[test]
    fn category_panel_empty_strings_filtered() {
        let mut __init = TestInit::new();
        let mut cp = ControlCategoryPanel::new("Sectors", CategoryType::Sector);
        let mut stocks = vec![StockRec::default(), StockRec::default()];
        stocks[0].sector = "Tech".to_string();
        stocks[1].sector = String::new(); // empty — should be filtered

        cp.UpdateItems(&stocks, |s| &s.sector);
        assert_eq!(cp.sorted_items, vec!["Tech"]);
    }

    #[test]
    fn read_from_widgets_not_expanded_is_noop() {
        let mut __init = TestInit::new();
        let panel = make_panel();
        let mut config = emStocksConfig {
            api_key: "original".to_string(),
            ..emStocksConfig::default()
        };
        panel.ReadFromWidgets(&mut config);
        // Widgets absent — config unchanged
        assert_eq!(config.api_key, "original");
    }

    #[test]
    fn read_from_widgets_reflects_update_controls() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let original = emStocksConfig {
            api_key: "my-key".to_string(),
            auto_update_dates: true,
            triggering_opens_web_page: true,
            chart_period: ChartPeriod::Months6,
            min_visible_interest: Interest::Medium,
            sorting: Sorting::ByDividend,
            owned_shares_first: true,
            search_text: "hello".to_string(),
            ..Default::default()
        };
        let rec = emStocksRec::default();
        let list_box = emStocksListBox::new();

        with_scratch_ctx(|ctx| panel.UpdateControls(&original, &rec, &list_box, ctx));

        let mut readback = emStocksConfig::default();
        panel.ReadFromWidgets(&mut readback);

        assert_eq!(readback.api_key, original.api_key);
        assert_eq!(readback.auto_update_dates, original.auto_update_dates);
        assert_eq!(
            readback.triggering_opens_web_page,
            original.triggering_opens_web_page
        );
        assert_eq!(readback.chart_period, original.chart_period);
        assert_eq!(readback.min_visible_interest, original.min_visible_interest);
        assert_eq!(readback.sorting, original.sorting);
        assert_eq!(readback.owned_shares_first, original.owned_shares_first);
        assert_eq!(readback.search_text, original.search_text);
    }

    #[test]
    fn read_from_widgets_chart_period_all_indices() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let periods = [
            ChartPeriod::Week1,
            ChartPeriod::Weeks2,
            ChartPeriod::Month1,
            ChartPeriod::Months3,
            ChartPeriod::Months6,
            ChartPeriod::Year1,
            ChartPeriod::Years3,
            ChartPeriod::Years5,
            ChartPeriod::Years10,
            ChartPeriod::Years20,
        ];
        for period in periods {
            let config_in = emStocksConfig {
                chart_period: period,
                ..Default::default()
            };
            let rec = emStocksRec::default();
            let list_box = emStocksListBox::new();
            with_scratch_ctx(|ctx| panel.UpdateControls(&config_in, &rec, &list_box, ctx));

            let mut config_out = emStocksConfig::default();
            panel.ReadFromWidgets(&mut config_out);
            assert_eq!(config_out.chart_period, period);
        }
    }

    // ── B-001-followup Phase B — D-006 wiring tests ─────────────────────

    /// G1+G2+G4 — first Cycle wires the FileModel/Config/SelectedDate signals
    /// and caches their ids.
    #[test]
    fn control_panel_first_cycle_wires_g1_g2_g4_signals() {
        use emcore::emEngine::Priority;
        use emcore::emPanel::PanelBehavior;
        use emcore::emPanelScope::PanelScope;
        use emcore::test_view_harness::TestViewHarness;

        struct NoopE;
        impl emcore::emEngine::emEngine for NoopE {
            fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }

        let mut h = TestViewHarness::new();
        let eid =
            h.scheduler
                .register_engine(Box::new(NoopE), Priority::Medium, PanelScope::Framework);

        let mut panel = emStocksControlPanel::for_test();
        assert!(!panel.subscribed_init);
        assert!(panel.model_change_sig.is_none());
        assert!(panel.config_change_sig.is_none());
        assert!(panel.selected_date_sig.is_none());

        let mut tree = emcore::emPanelTree::PanelTree::new();
        let id = tree.create_root("cp", false);
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }

        assert!(panel.subscribed_init);
        assert!(panel.model_change_sig.is_some());
        assert!(panel.config_change_sig.is_some());
        assert!(panel.selected_date_sig.is_some());

        h.scheduler.remove_engine(eid);
        h.scheduler.flush_signals_for_test();
    }

    /// G1 — firing FileModel.ChangeSignal flips `update_controls_needed`.
    #[test]
    fn control_panel_reacts_to_file_model_change_signal() {
        use emcore::emEngine::Priority;
        use emcore::emPanel::PanelBehavior;
        use emcore::emPanelScope::PanelScope;
        use emcore::test_view_harness::TestViewHarness;

        struct NoopE;
        impl emcore::emEngine::emEngine for NoopE {
            fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }

        let mut h = TestViewHarness::new();
        let eid =
            h.scheduler
                .register_engine(Box::new(NoopE), Priority::Medium, PanelScope::Framework);

        let mut panel = emStocksControlPanel::for_test();
        let mut tree = emcore::emPanelTree::PanelTree::new();
        let id = tree.create_root("cp", false);

        // First Cycle wires.
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }
        panel.MarkUpdated();
        assert!(!panel.NeedsUpdate());

        // Fire model signal.
        let sig = panel.model_change_sig.expect("wired");
        h.scheduler.fire(sig);
        h.scheduler.flush_signals_for_test();

        // Second Cycle observes IsSignaled.
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }

        assert!(panel.NeedsUpdate());

        h.scheduler.remove_engine(eid);
        h.scheduler.flush_signals_for_test();
    }

    /// G7 widget reaction — firing AutoUpdateDates.check_signal writes Config.
    #[test]
    fn control_panel_reacts_to_auto_update_dates_check_signal() {
        use emcore::emEngine::Priority;
        use emcore::emPanel::PanelBehavior;
        use emcore::emPanelScope::PanelScope;
        use emcore::test_view_harness::TestViewHarness;

        struct NoopE;
        impl emcore::emEngine::emEngine for NoopE {
            fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }

        let mut h = TestViewHarness::new();
        let eid =
            h.scheduler
                .register_engine(Box::new(NoopE), Priority::Medium, PanelScope::Framework);

        // Build panel with shared Config so we can observe writes.
        let model = Rc::new(RefCell::new(emStocksFileModel::new(
            std::path::PathBuf::from("/tmp/cp_b001_g7.emStocks"),
        )));
        let config = Rc::new(RefCell::new(emStocksConfig::default()));
        let list_box = Rc::new(RefCell::new(emStocksListBox::new()));
        let mut panel = emStocksControlPanel::new(emLook::new(), model, config.clone(), list_box);

        // First Cycle wires G1/G2/G4. Then AutoExpand creates widgets.
        let mut tree = emcore::emPanelTree::PanelTree::new();
        let id = tree.create_root("cp", false);
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }
        {
            let mut sc = h.sched_ctx_for(eid);
            panel.AutoExpand(&mut sc);
        }
        // Second Cycle wires widget signals.
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }
        assert!(panel.subscribed_widgets);
        assert!(!config.borrow().auto_update_dates);

        // Fire AutoUpdateDates check_signal directly. Stage widget state
        // via the silent setter so the reaction can read the new value.
        let sig = panel
            .widgets
            .as_ref()
            .unwrap()
            .auto_update_dates
            .check_signal;
        panel
            .widgets
            .as_mut()
            .unwrap()
            .auto_update_dates
            .set_checked_silent(true);
        h.scheduler.fire(sig);
        h.scheduler.flush_signals_for_test();

        // Drive Cycle to observe IsSignaled and write Config.
        {
            let mut pctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }

        assert!(
            config.borrow().auto_update_dates,
            "AutoUpdateDates check_signal reaction must write Config"
        );

        h.scheduler.remove_engine(eid);
        h.scheduler.flush_signals_for_test();
    }

    #[test]
    fn read_from_widgets_interest_and_sorting_roundtrip() {
        let mut __init = TestInit::new();
        let mut panel = make_panel();
        panel.AutoExpand(&mut __init.ctx());

        let interests = [Interest::High, Interest::Medium, Interest::Low];
        for interest in interests {
            let config_in = emStocksConfig {
                min_visible_interest: interest,
                ..Default::default()
            };
            let rec = emStocksRec::default();
            let list_box = emStocksListBox::new();
            with_scratch_ctx(|ctx| panel.UpdateControls(&config_in, &rec, &list_box, ctx));

            let mut config_out = emStocksConfig::default();
            panel.ReadFromWidgets(&mut config_out);
            assert_eq!(config_out.min_visible_interest, interest);
        }

        let sortings = [
            Sorting::ByName,
            Sorting::ByTradeDate,
            Sorting::ByInquiryDate,
            Sorting::ByAchievement,
            Sorting::ByOneWeekRise,
            Sorting::ByThreeWeekRise,
            Sorting::ByNineWeekRise,
            Sorting::ByDividend,
            Sorting::ByPurchaseValue,
            Sorting::ByValue,
            Sorting::ByDifference,
        ];
        for sorting in sortings {
            let config_in = emStocksConfig {
                sorting,
                ..Default::default()
            };
            let rec = emStocksRec::default();
            let list_box = emStocksListBox::new();
            with_scratch_ctx(|ctx| panel.UpdateControls(&config_in, &rec, &list_box, ctx));

            let mut config_out = emStocksConfig::default();
            panel.ReadFromWidgets(&mut config_out);
            assert_eq!(config_out.sorting, sorting);
        }
    }
}
