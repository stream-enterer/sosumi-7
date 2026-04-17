use emStocks::emStocksConfig::{emStocksConfig, ChartPeriod, Sorting};
use emStocks::emStocksRec::{emStocksRec, AddDaysToDate, GetDateDifference, Interest, StockRec};
use emcore::emRecRecord::Record;

// ─── Record round-trip tests ───

#[test]
fn emstocks_rec_file_round_trip() {
    // Create an emStocksRec with 2 stocks with various data
    // Serialize via to_rec(), verify from_rec() produces identical data
    let mut rec = emStocksRec::default();

    let mut stock1 = StockRec::default();
    stock1.id = "1".to_string();
    stock1.name = "Test Corp".to_string();
    stock1.symbol = "TST".to_string();
    stock1.prices = "100|101|102".to_string();
    stock1.last_price_date = "2024-03-15".to_string();
    stock1.interest = Interest::High;
    stock1.web_pages = vec!["https://example.com".to_string()];
    rec.stocks.push(stock1);

    let mut stock2 = StockRec::default();
    stock2.id = "2".to_string();
    stock2.name = "Other Corp".to_string();
    stock2.owning_shares = true;
    stock2.own_shares = "100".to_string();
    stock2.trade_price = "50.00".to_string();
    stock2.interest = Interest::Low;
    rec.stocks.push(stock2);

    let rec_struct = rec.to_rec();
    let deserialized = emStocksRec::from_rec(&rec_struct).unwrap();

    assert_eq!(deserialized.stocks.len(), 2);
    assert_eq!(deserialized.stocks[0].id, "1");
    assert_eq!(deserialized.stocks[0].name, "Test Corp");
    assert_eq!(deserialized.stocks[0].prices, "100|101|102");
    assert_eq!(deserialized.stocks[0].interest, Interest::High);
    assert_eq!(
        deserialized.stocks[0].web_pages,
        vec!["https://example.com"]
    );
    assert_eq!(deserialized.stocks[1].id, "2");
    assert!(deserialized.stocks[1].owning_shares);
    assert_eq!(deserialized.stocks[1].interest, Interest::Low);
}

#[test]
fn emstocks_config_round_trip() {
    let config = emStocksConfig {
        chart_period: ChartPeriod::Months3,
        sorting: Sorting::ByDifference,
        visible_countries: vec!["US".to_string(), "DE".to_string()],
        min_visible_interest: Interest::Medium,
        ..emStocksConfig::default()
    };

    let rec_struct = config.to_rec();
    let deserialized = emStocksConfig::from_rec(&rec_struct).unwrap();

    assert_eq!(deserialized.chart_period, ChartPeriod::Months3);
    assert_eq!(deserialized.sorting, Sorting::ByDifference);
    assert_eq!(deserialized.visible_countries, vec!["US", "DE"]);
    assert_eq!(deserialized.min_visible_interest, Interest::Medium);
}

// ─── Date arithmetic consistency tests ───

#[test]
fn date_round_trip_add_subtract() {
    // Adding N days and subtracting N days should return to original date
    let original = "2024-06-15";
    for n in [1, 7, 28, 30, 31, 90, 365, 366, 730] {
        let forward = AddDaysToDate(n, original);
        let back = AddDaysToDate(-n, &forward);
        assert_eq!(back, original, "round-trip failed for n={n}");
    }
}

#[test]
fn date_difference_matches_add() {
    // GetDateDifference(a, b) should equal the number of days you'd add to a to reach b
    let a = "2023-01-01";
    let b = "2025-12-31";
    let (diff, valid) = GetDateDifference(a, b);
    assert!(valid);
    let result = AddDaysToDate(diff, a);
    assert_eq!(result, b);
}

#[test]
fn date_leap_year_boundaries() {
    // Feb 29 in a leap year
    assert_eq!(AddDaysToDate(1, "2024-02-28"), "2024-02-29");
    assert_eq!(AddDaysToDate(1, "2024-02-29"), "2024-03-01");
    // Non-leap year
    assert_eq!(AddDaysToDate(1, "2023-02-28"), "2023-03-01");
}

// ─── Price management tests ───

#[test]
fn stock_add_price_overwrites_existing() {
    let mut stock = StockRec::default();
    stock.AddPrice("2024-03-15", "100.00");
    stock.AddPrice("2024-03-15", "105.00"); // overwrite same date
    assert_eq!(stock.GetPriceOfDate("2024-03-15"), "105.00");
}

#[test]
fn stock_add_price_gap_days() {
    let mut stock = StockRec::default();
    stock.AddPrice("2024-03-15", "100");
    stock.AddPrice("2024-03-18", "103"); // 3 days later

    assert_eq!(stock.GetPriceOfDate("2024-03-15"), "100");
    assert_eq!(stock.GetPriceOfDate("2024-03-16"), ""); // gap
    assert_eq!(stock.GetPriceOfDate("2024-03-17"), ""); // gap
    assert_eq!(stock.GetPriceOfDate("2024-03-18"), "103");
}

#[test]
fn stock_add_price_before_existing() {
    let mut stock = StockRec::default();
    stock.AddPrice("2024-03-15", "100");
    stock.AddPrice("2024-03-13", "98"); // earlier date

    assert_eq!(stock.GetPriceOfDate("2024-03-13"), "98");
    assert_eq!(stock.GetPriceOfDate("2024-03-15"), "100");
}

// ─── Financial calculation tests ───

#[test]
fn stock_value_of_date() {
    let mut stock = StockRec::default();
    stock.owning_shares = true;
    stock.own_shares = "10".to_string();
    stock.AddPrice("2024-03-15", "150");

    let val = stock.GetValueOfDate("2024-03-15");
    assert_eq!(val, Some(1500.0));
}

#[test]
fn stock_difference_value_of_date() {
    let mut stock = StockRec::default();
    stock.owning_shares = true;
    stock.own_shares = "10".to_string();
    stock.trade_price = "100".to_string();
    stock.AddPrice("2024-03-15", "150");

    let diff = stock.GetDifferenceValueOfDate("2024-03-15");
    assert_eq!(diff, Some(500.0)); // (150 - 100) * 10
}
