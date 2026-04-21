// Port of C++ emStocksConfig.h / emStocksConfig.cpp

use std::fmt;
use std::str::FromStr;

use emcore::emRecParser::{RecStruct, RecValue};
use emcore::emRecRecord::{RecError, Record};

use super::emStocksRec::{GetDateDifferenceParts, GetDaysOfMonth, Interest, ParseDate};

// ─── ChartPeriod ─────────────────────────────────────────────────────────────

/// Port of C++ emStocksConfig::PeriodType.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ChartPeriod {
    Week1,   // PT_1_WEEK
    Weeks2,  // PT_2_WEEKS
    Month1,  // PT_1_MONTH
    Months3, // PT_3_MONTHS
    Months6, // PT_6_MONTHS
    #[default]
    Year1, // PT_1_YEAR  (default)
    Years3,  // PT_3_YEARS
    Years5,  // PT_5_YEARS
    Years10, // PT_10_YEARS
    Years20, // PT_20_YEARS
}

impl fmt::Display for ChartPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Week1 => write!(f, "PT_1_WEEK"),
            Self::Weeks2 => write!(f, "PT_2_WEEKS"),
            Self::Month1 => write!(f, "PT_1_MONTH"),
            Self::Months3 => write!(f, "PT_3_MONTHS"),
            Self::Months6 => write!(f, "PT_6_MONTHS"),
            Self::Year1 => write!(f, "PT_1_YEAR"),
            Self::Years3 => write!(f, "PT_3_YEARS"),
            Self::Years5 => write!(f, "PT_5_YEARS"),
            Self::Years10 => write!(f, "PT_10_YEARS"),
            Self::Years20 => write!(f, "PT_20_YEARS"),
        }
    }
}

impl FromStr for ChartPeriod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PT_1_WEEK" => Ok(Self::Week1),
            "PT_2_WEEKS" => Ok(Self::Weeks2),
            "PT_1_MONTH" => Ok(Self::Month1),
            "PT_3_MONTHS" => Ok(Self::Months3),
            "PT_6_MONTHS" => Ok(Self::Months6),
            "PT_1_YEAR" => Ok(Self::Year1),
            "PT_3_YEARS" => Ok(Self::Years3),
            "PT_5_YEARS" => Ok(Self::Years5),
            "PT_10_YEARS" => Ok(Self::Years10),
            "PT_20_YEARS" => Ok(Self::Years20),
            _ => Err(format!("unknown chart period: {s}")),
        }
    }
}

// ─── Sorting ─────────────────────────────────────────────────────────────────

/// Port of C++ emStocksConfig::SortingType.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sorting {
    #[default]
    ByName, // SORT_BY_NAME (default)
    ByTradeDate,     // SORT_BY_TRADE_DATE
    ByInquiryDate,   // SORT_BY_INQUIRY_DATE
    ByAchievement,   // SORT_BY_ACHIEVEMENT
    ByOneWeekRise,   // SORT_BY_ONE_WEEK_RISE
    ByThreeWeekRise, // SORT_BY_THREE_WEEK_RISE
    ByNineWeekRise,  // SORT_BY_NINE_WEEK_RISE
    ByDividend,      // SORT_BY_DIVIDEND
    ByPurchaseValue, // SORT_BY_PURCHASE_VALUE
    ByValue,         // SORT_BY_VALUE
    ByDifference,    // SORT_BY_DIFFERENCE
}

impl fmt::Display for Sorting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByName => write!(f, "SORT_BY_NAME"),
            Self::ByTradeDate => write!(f, "SORT_BY_TRADE_DATE"),
            Self::ByInquiryDate => write!(f, "SORT_BY_INQUIRY_DATE"),
            Self::ByAchievement => write!(f, "SORT_BY_ACHIEVEMENT"),
            Self::ByOneWeekRise => write!(f, "SORT_BY_ONE_WEEK_RISE"),
            Self::ByThreeWeekRise => write!(f, "SORT_BY_THREE_WEEK_RISE"),
            Self::ByNineWeekRise => write!(f, "SORT_BY_NINE_WEEK_RISE"),
            Self::ByDividend => write!(f, "SORT_BY_DIVIDEND"),
            Self::ByPurchaseValue => write!(f, "SORT_BY_PURCHASE_VALUE"),
            Self::ByValue => write!(f, "SORT_BY_VALUE"),
            Self::ByDifference => write!(f, "SORT_BY_DIFFERENCE"),
        }
    }
}

impl FromStr for Sorting {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SORT_BY_NAME" => Ok(Self::ByName),
            "SORT_BY_TRADE_DATE" => Ok(Self::ByTradeDate),
            "SORT_BY_INQUIRY_DATE" => Ok(Self::ByInquiryDate),
            "SORT_BY_ACHIEVEMENT" => Ok(Self::ByAchievement),
            "SORT_BY_ONE_WEEK_RISE" => Ok(Self::ByOneWeekRise),
            "SORT_BY_THREE_WEEK_RISE" => Ok(Self::ByThreeWeekRise),
            "SORT_BY_NINE_WEEK_RISE" => Ok(Self::ByNineWeekRise),
            "SORT_BY_DIVIDEND" => Ok(Self::ByDividend),
            "SORT_BY_PURCHASE_VALUE" => Ok(Self::ByPurchaseValue),
            "SORT_BY_VALUE" => Ok(Self::ByValue),
            "SORT_BY_DIFFERENCE" => Ok(Self::ByDifference),
            _ => Err(format!("unknown sorting: {s}")),
        }
    }
}

// ─── emStocksConfig ──────────────────────────────────────────────────────────

/// Port of C++ emStocksConfig.
#[derive(Debug, Clone, PartialEq)]
pub struct emStocksConfig {
    pub api_script: String,
    pub api_script_interpreter: String,
    pub api_key: String,
    pub web_browser: String,
    pub auto_update_dates: bool,
    pub triggering_opens_web_page: bool,
    pub chart_period: ChartPeriod,
    pub min_visible_interest: Interest,
    pub visible_countries: Vec<String>,
    pub visible_sectors: Vec<String>,
    pub visible_collections: Vec<String>,
    pub sorting: Sorting,
    pub owned_shares_first: bool,
    pub search_text: String,
}

impl Default for emStocksConfig {
    fn default() -> Self {
        Self {
            api_script: String::new(),
            api_script_interpreter: "perl".to_string(),
            api_key: String::new(),
            web_browser: "firefox".to_string(),
            auto_update_dates: false,
            triggering_opens_web_page: false,
            chart_period: ChartPeriod::Year1,
            min_visible_interest: Interest::Low,
            visible_countries: Vec::new(),
            visible_sectors: Vec::new(),
            visible_collections: Vec::new(),
            sorting: Sorting::ByName,
            owned_shares_first: false,
            search_text: String::new(),
        }
    }
}

impl emStocksConfig {
    /// Port of C++ GetFormatName.
    pub fn GetFormatName(&self) -> &str {
        "emStocksConfig"
    }

    /// Port of C++ CalculateChartPeriodDays.
    pub fn CalculateChartPeriodDays(&self, end_date: &str) -> i32 {
        match self.chart_period {
            ChartPeriod::Week1 => return 7,
            ChartPeriod::Weeks2 => return 14,
            _ => {}
        }

        let (y2, m2, d2) = ParseDate(end_date).unwrap_or((0, 0, 0));
        let mut y1 = y2;
        let mut m1 = m2;

        match self.chart_period {
            ChartPeriod::Month1 => m1 -= 1,
            ChartPeriod::Months3 => m1 -= 3,
            ChartPeriod::Months6 => m1 -= 6,
            ChartPeriod::Year1 => y1 -= 1,
            ChartPeriod::Years3 => y1 -= 3,
            ChartPeriod::Years5 => y1 -= 5,
            ChartPeriod::Years10 => y1 -= 10,
            ChartPeriod::Years20 => y1 -= 20,
            ChartPeriod::Week1 | ChartPeriod::Weeks2 => unreachable!(),
        }

        while m1 <= 0 {
            m1 += 12;
            y1 -= 1;
        }

        let d1 = d2.min(GetDaysOfMonth(y1, m1));

        GetDateDifferenceParts(y1, m1, d1, y2, m2, d2)
    }

    /// Port of C++ IsInVisibleCategories.
    /// Binary search on sorted categories vec. Empty vec means all visible.
    pub fn IsInVisibleCategories(categories: &[String], category: &str) -> bool {
        if categories.is_empty() {
            return true;
        }
        categories
            .binary_search_by(|c| c.as_str().cmp(category))
            .is_ok()
    }
}

// ─── Helper: parse ident from rec (lowercase stored, uppercase for FromStr) ──

fn parse_ident_upper<T: FromStr>(rec: &RecStruct, name: &str) -> Option<T> {
    let ident = rec.get_ident(name)?;
    T::from_str(&ident.to_ascii_uppercase()).ok()
}

fn read_string_array(rec: &RecStruct, name: &str) -> Vec<String> {
    if let Some(arr) = rec.get_array(name) {
        arr.iter()
            .filter_map(|v| {
                if let RecValue::Str(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

fn write_string_array(rec: &mut RecStruct, name: &str, arr: &[String]) {
    rec.SetValue(
        name,
        RecValue::Array(arr.iter().map(|s| RecValue::Str(s.clone())).collect()),
    );
}

// ─── Record impl ─────────────────────────────────────────────────────────────

impl Record for emStocksConfig {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let get_str = |name: &str| -> String { rec.get_str(name).unwrap_or("").to_string() };

        let chart_period = parse_ident_upper::<ChartPeriod>(rec, "ChartPeriod").unwrap_or_default();

        let sorting = parse_ident_upper::<Sorting>(rec, "Sorting").unwrap_or_default();

        // MinVisibleInterest: try canonical first, then deprecated-normal fallback
        let min_visible_interest = if let Some(ident) = rec.get_ident("MinVisibleInterest") {
            let upper = ident.to_ascii_uppercase();
            Interest::from_str(&upper).unwrap_or_else(|_| Interest::from_deprecated_normal(&upper))
        } else {
            Interest::Low
        };

        let api_script_interpreter = {
            let s = get_str("ApiScriptInterpreter");
            if s.is_empty() {
                "perl".to_string()
            } else {
                s
            }
        };

        let web_browser = {
            let s = get_str("WebBrowser");
            if s.is_empty() {
                "firefox".to_string()
            } else {
                s
            }
        };

        Ok(Self {
            api_script: get_str("ApiScript"),
            api_script_interpreter,
            api_key: get_str("ApiKey"),
            web_browser,
            auto_update_dates: rec.get_bool("AutoUpdateDates").unwrap_or(false),
            triggering_opens_web_page: rec.get_bool("TriggeringOpensWebPage").unwrap_or(false),
            chart_period,
            min_visible_interest,
            visible_countries: read_string_array(rec, "VisibleCountries"),
            visible_sectors: read_string_array(rec, "VisibleSectors"),
            visible_collections: read_string_array(rec, "VisibleCollections"),
            sorting,
            owned_shares_first: rec.get_bool("OwnedSharesFirst").unwrap_or(false),
            search_text: get_str("SearchText"),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut rec = RecStruct::new();
        rec.set_str("ApiScript", &self.api_script);
        rec.set_str("ApiScriptInterpreter", &self.api_script_interpreter);
        rec.set_str("ApiKey", &self.api_key);
        rec.set_str("WebBrowser", &self.web_browser);
        rec.set_bool("AutoUpdateDates", self.auto_update_dates);
        rec.set_bool("TriggeringOpensWebPage", self.triggering_opens_web_page);
        rec.set_ident("ChartPeriod", &self.chart_period.to_string());
        rec.set_ident("MinVisibleInterest", &self.min_visible_interest.to_string());
        write_string_array(&mut rec, "VisibleCountries", &self.visible_countries);
        write_string_array(&mut rec, "VisibleSectors", &self.visible_sectors);
        write_string_array(&mut rec, "VisibleCollections", &self.visible_collections);
        rec.set_ident("Sorting", &self.sorting.to_string());
        rec.set_bool("OwnedSharesFirst", self.owned_shares_first);
        rec.set_str("SearchText", &self.search_text);
        rec
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_period_from_str() {
        assert_eq!(ChartPeriod::from_str("PT_1_WEEK"), Ok(ChartPeriod::Week1));
        assert_eq!(ChartPeriod::from_str("PT_1_YEAR"), Ok(ChartPeriod::Year1));
        assert_eq!(
            ChartPeriod::from_str("PT_20_YEARS"),
            Ok(ChartPeriod::Years20)
        );
    }

    #[test]
    fn sorting_from_str() {
        assert_eq!(Sorting::from_str("SORT_BY_NAME"), Ok(Sorting::ByName));
        assert_eq!(
            Sorting::from_str("SORT_BY_DIFFERENCE"),
            Ok(Sorting::ByDifference)
        );
    }

    #[test]
    fn config_record_round_trip() {
        let config = emStocksConfig::default();
        let serialized = config.to_rec();
        let deserialized = emStocksConfig::from_rec(&serialized).unwrap();
        assert_eq!(deserialized.chart_period, config.chart_period);
        assert_eq!(deserialized.sorting, config.sorting);
        assert_eq!(deserialized.web_browser, config.web_browser);
    }

    #[test]
    fn calculate_chart_period_days_fixed() {
        let config = emStocksConfig {
            chart_period: ChartPeriod::Week1,
            ..Default::default()
        };
        assert_eq!(config.CalculateChartPeriodDays("2024-06-15"), 7);
    }

    #[test]
    fn calculate_chart_period_days_month() {
        let config = emStocksConfig {
            chart_period: ChartPeriod::Month1,
            ..Default::default()
        };
        let days = config.CalculateChartPeriodDays("2024-06-15");
        assert_eq!(days, 31); // May has 31 days
    }

    #[test]
    fn is_in_visible_categories_empty_means_all_visible() {
        let categories: Vec<String> = vec![];
        assert!(emStocksConfig::IsInVisibleCategories(
            &categories,
            "anything"
        ));
    }

    #[test]
    fn is_in_visible_categories_binary_search() {
        let categories = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        assert!(emStocksConfig::IsInVisibleCategories(&categories, "B"));
        assert!(!emStocksConfig::IsInVisibleCategories(&categories, "D"));
    }
}
