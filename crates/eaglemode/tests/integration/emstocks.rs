use emStocks::emStocksConfig::{emStocksConfig, ChartPeriod, Sorting};
use emStocks::emStocksListBox::emStocksListBox;
use emStocks::emStocksRec::{emStocksRec, Interest, StockRec};
use emcore::emRecParser::{parse_rec_with_format, write_rec_with_format};
use emcore::emRecRecord::Record;

#[test]
fn load_emstocks_from_rec_format() {
    // Create an emStocksRec programmatically
    let mut rec = emStocksRec::default();

    let mut stock1 = StockRec::default();
    stock1.id = "1".to_string();
    stock1.name = "Acme Corp".to_string();
    stock1.symbol = "ACME".to_string();
    stock1.country = "US".to_string();
    stock1.sector = "Tech".to_string();
    stock1.interest = Interest::High;
    stock1.owning_shares = true;
    stock1.own_shares = "100".to_string();
    stock1.trade_price = "50.00".to_string();
    stock1.trade_date = "2024-01-15".to_string();
    stock1.web_pages = vec!["https://acme.com".to_string()];
    stock1.AddPrice("2024-03-14", "98.00");
    stock1.AddPrice("2024-03-15", "100.50");
    rec.stocks.push(stock1);

    let mut stock2 = StockRec::default();
    stock2.id = "2".to_string();
    stock2.name = "Beta Inc".to_string();
    stock2.symbol = "BETA".to_string();
    stock2.country = "DE".to_string();
    stock2.interest = Interest::Low;
    rec.stocks.push(stock2);

    // Serialize to emRec format
    let rec_struct = rec.to_rec();
    let text = write_rec_with_format(&rec_struct, "emStocks");

    // Parse back
    let parsed = parse_rec_with_format(&text, "emStocks").unwrap();
    let loaded = emStocksRec::from_rec(&parsed).unwrap();

    assert_eq!(loaded.stocks.len(), 2);
    assert_eq!(loaded.stocks[0].name, "Acme Corp");
    assert_eq!(loaded.stocks[0].symbol, "ACME");
    assert_eq!(loaded.stocks[0].interest, Interest::High);
    assert!(loaded.stocks[0].owning_shares);
    assert_eq!(loaded.stocks[0].web_pages, vec!["https://acme.com"]);
    assert_eq!(loaded.stocks[1].name, "Beta Inc");
    assert_eq!(loaded.stocks[1].interest, Interest::Low);
}

#[test]
fn emstocks_full_round_trip() {
    // Create initial data
    let mut rec = emStocksRec::default();
    let mut stock = StockRec::default();
    stock.id = "1".to_string();
    stock.name = "Original Name".to_string();
    stock.symbol = "ORIG".to_string();
    stock.interest = Interest::Medium;
    rec.stocks.push(stock);

    // Round-trip 1: serialize and parse
    let text1 = write_rec_with_format(&rec.to_rec(), "emStocks");
    let parsed1 = parse_rec_with_format(&text1, "emStocks").unwrap();
    let mut loaded = emStocksRec::from_rec(&parsed1).unwrap();

    // Modify
    loaded.stocks[0].name = "Modified Name".to_string();
    loaded.stocks[0].interest = Interest::High;

    // Add a new stock
    let mut new_stock = StockRec::default();
    new_stock.id = loaded.InventStockId();
    new_stock.name = "New Stock".to_string();
    loaded.stocks.push(new_stock);

    // Round-trip 2: serialize and parse
    let text2 = write_rec_with_format(&loaded.to_rec(), "emStocks");
    let parsed2 = parse_rec_with_format(&text2, "emStocks").unwrap();
    let final_rec = emStocksRec::from_rec(&parsed2).unwrap();

    assert_eq!(final_rec.stocks.len(), 2);
    assert_eq!(final_rec.stocks[0].name, "Modified Name");
    assert_eq!(final_rec.stocks[0].interest, Interest::High);
    assert_eq!(final_rec.stocks[1].name, "New Stock");
    assert_eq!(final_rec.stocks[1].id, "2");
}

#[test]
fn emstocks_price_data_round_trip() {
    let mut rec = emStocksRec::default();
    let mut stock = StockRec::default();
    stock.id = "1".to_string();
    stock.name = "Price Test".to_string();

    // Add a week of prices
    stock.AddPrice("2024-03-11", "100");
    stock.AddPrice("2024-03-12", "101");
    stock.AddPrice("2024-03-13", "99");
    stock.AddPrice("2024-03-14", "102");
    stock.AddPrice("2024-03-15", "103");

    rec.stocks.push(stock);

    // Round-trip
    let text = write_rec_with_format(&rec.to_rec(), "emStocks");
    let parsed = parse_rec_with_format(&text, "emStocks").unwrap();
    let loaded = emStocksRec::from_rec(&parsed).unwrap();

    assert_eq!(loaded.stocks[0].GetPriceOfDate("2024-03-11"), "100");
    assert_eq!(loaded.stocks[0].GetPriceOfDate("2024-03-13"), "99");
    assert_eq!(loaded.stocks[0].GetPriceOfDate("2024-03-15"), "103");
    assert_eq!(loaded.stocks[0].last_price_date, "2024-03-15");
}

#[test]
fn emstocks_config_round_trip() {
    let config = emStocksConfig {
        api_script: "/usr/local/bin/fetch_prices.pl".to_string(),
        api_key: "test_key_123".to_string(),
        chart_period: ChartPeriod::Months3,
        sorting: Sorting::ByDifference,
        visible_countries: vec!["US".to_string(), "DE".to_string()],
        owned_shares_first: true,
        ..emStocksConfig::default()
    };

    let text = write_rec_with_format(&config.to_rec(), "emStocksConfig");
    let parsed = parse_rec_with_format(&text, "emStocksConfig").unwrap();
    let loaded = emStocksConfig::from_rec(&parsed).unwrap();

    assert_eq!(loaded.api_script, "/usr/local/bin/fetch_prices.pl");
    assert_eq!(loaded.api_key, "test_key_123");
    assert_eq!(loaded.chart_period, ChartPeriod::Months3);
    assert_eq!(loaded.sorting, Sorting::ByDifference);
    assert_eq!(loaded.visible_countries, vec!["US", "DE"]);
    assert!(loaded.owned_shares_first);
}

#[test]
fn emstocks_listbox_full_pipeline() {
    let mut rec = emStocksRec::default();

    // Create stocks with different attributes
    let mut s1 = StockRec::default();
    s1.id = "1".to_string();
    s1.name = "Zebra Corp".to_string();
    s1.interest = Interest::High;
    s1.country = "US".to_string();
    rec.stocks.push(s1);

    let mut s2 = StockRec::default();
    s2.id = "2".to_string();
    s2.name = "Alpha Inc".to_string();
    s2.interest = Interest::Medium;
    s2.country = "US".to_string();
    rec.stocks.push(s2);

    let mut s3 = StockRec::default();
    s3.id = "3".to_string();
    s3.name = "Hidden Stock".to_string();
    s3.interest = Interest::Low;
    s3.country = "JP".to_string();
    rec.stocks.push(s3);

    // Config: show only High+Medium interest, only US
    let config = emStocksConfig {
        min_visible_interest: Interest::Medium,
        visible_countries: vec!["US".to_string()],
        sorting: Sorting::ByName,
        ..Default::default()
    };

    let mut lb = emStocksListBox::new();
    lb.UpdateItems(&rec, &config);

    // Only US stocks with interest <= Medium visible
    assert_eq!(lb.visible_items.len(), 2);
    // Sorted by name: Alpha before Zebra
    assert_eq!(rec.stocks[lb.visible_items[0]].name, "Alpha Inc");
    assert_eq!(rec.stocks[lb.visible_items[1]].name, "Zebra Corp");
}
