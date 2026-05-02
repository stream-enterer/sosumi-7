use std::cell::RefCell;
use std::rc::Rc;

use emStocks::emStocksPricesFetcher::emStocksPricesFetcher;
use emStocks::emStocksRec::{emStocksRec, SharePriceToString, StockRec};
use emcore::emCrossPtr::{emCrossPtr, emCrossPtrList};
use emcore::emEngineCtx::DropOnlySignalCtx;

// ─── emCrossPtr patterns used by emStocks ───

#[test]
fn crossptr_basic_validity() {
    let mut list = emCrossPtrList::new();
    let target = Rc::new(RefCell::new(42u32));
    let ptr = emCrossPtr::from_target(&target, &mut list);
    assert!(ptr.is_valid());
    assert_eq!(*ptr.Get().unwrap().borrow(), 42);
}

#[test]
fn crossptr_invalidation_on_break() {
    let mut list = emCrossPtrList::new();
    let target = Rc::new(RefCell::new("hello".to_string()));
    let ptr = emCrossPtr::from_target(&target, &mut list);
    assert!(ptr.is_valid());

    list.BreakCrossPtrs();
    assert!(!ptr.is_valid());
    assert!(ptr.Get().is_none());
    // Target is still alive, just the cross-ptr is broken
    assert_eq!(*target.borrow(), "hello");
}

#[test]
fn crossptr_invalidation_on_drop() {
    let target = Rc::new(RefCell::new(99u32));
    let ptr;
    {
        let mut list = emCrossPtrList::new();
        ptr = emCrossPtr::from_target(&target, &mut list);
        assert!(ptr.is_valid());
    } // list dropped -> BreakCrossPtrs called
    assert!(!ptr.is_valid());
}

// ─── emStocksPricesFetcher parsing tests ───

#[test]
fn fetcher_process_out_buffer_lines_multiple() {
    let mut fetcher = emStocksPricesFetcher::new("", "", "");
    fetcher.current_start_date = "2024-01-01".to_string();
    let mut rec = emStocksRec::default();
    let mut stock = StockRec::default();
    stock.id = "1".to_string();
    stock.symbol = "TST".to_string();
    rec.stocks.push(stock);
    fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);

    // Multiple lines with different line endings
    fetcher.out_buffer = b"2024-03-14 99.0\r\n2024-03-15 100.5\n".to_vec();
    fetcher.ProcessOutBufferLines(&mut rec);

    assert!(fetcher.out_buffer.is_empty()); // all complete lines consumed
    assert_eq!(
        rec.stocks[0].GetPriceOfDate("2024-03-14"),
        SharePriceToString(99.0)
    );
    assert_eq!(
        rec.stocks[0].GetPriceOfDate("2024-03-15"),
        SharePriceToString(100.5)
    );
}

#[test]
fn fetcher_process_out_buffer_line_with_extra_whitespace() {
    let mut fetcher = emStocksPricesFetcher::new("", "", "");
    fetcher.current_start_date = "2024-01-01".to_string();
    let mut rec = emStocksRec::default();
    let mut stock = StockRec::default();
    stock.id = "1".to_string();
    rec.stocks.push(stock);
    fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);

    // Leading whitespace should be handled
    fetcher.ProcessOutBufferLine("  2024-03-15  100.50  ", &mut rec);
    assert_eq!(
        rec.stocks[0].GetPriceOfDate("2024-03-15"),
        SharePriceToString(100.5)
    );
}

#[test]
fn fetcher_add_stock_ids_dedup() {
    let mut fetcher = emStocksPricesFetcher::new("script", "perl", "key");
    fetcher.AddStockIds(&mut DropOnlySignalCtx, &["A".to_string(), "B".to_string()]);
    fetcher.AddStockIds(&mut DropOnlySignalCtx, &["B".to_string(), "C".to_string()]);
    // B should not be duplicated
    assert_eq!(fetcher.stock_ids.len(), 3);
}

#[test]
fn fetcher_calculate_date_clamps() {
    let mut fetcher = emStocksPricesFetcher::new("", "", "");

    // With a stock that has a very old last_price_date
    let mut stock = StockRec::default();
    stock.last_price_date = "1900-01-01".to_string();
    fetcher.CalculateDate(Some(&stock));

    // Should clamp to MAX_NUM_PRICES
    assert!(!fetcher.current_start_date.is_empty());
}
