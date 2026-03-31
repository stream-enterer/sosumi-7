// Port of C++ emStocksPricesFetcher.h / emStocksPricesFetcher.cpp
// DIVERGED(Phase 4): FileModel/FileStateSignal/ChangeSignal integration pending.
// emEngine trait is implemented but the trait Cycle cannot drive the fetch loop
// until FileModel integration provides access to the rec.
// DIVERGED: Uses BTreeMap<String, Option<usize>> mapping stock ID to index in emStocksRec.stocks,
// instead of emAvlTreeMap<String, emCrossPtr<StockRec>>. The cross-pointer approach
// doesn't work well when StockRecs are stored in a Vec.

use std::collections::{BTreeMap, HashMap};

use emcore::emEngine::{emEngine, EngineCtx};
use emcore::emProcess::{emProcess, PipeResult, StartFlags};

use super::emStocksRec::{
    emStocksRec, AddDaysToDate, CompareDates, GetCurrentDate, GetDateDifference,
    SharePriceToString, StockRec,
};

/// Placeholder for emStocksListBox (not yet ported).
pub struct emStocksListBoxPlaceholder;

/// Port of C++ emStocksPricesFetcher.
pub struct emStocksPricesFetcher {
    pub api_script: String,
    pub api_script_interpreter: String,
    pub api_key: String,
    pub stock_ids: Vec<String>,
    stock_index_map: BTreeMap<String, Option<usize>>,
    pub current_index: i32,
    pub current_symbol: String,
    pub current_start_date: String,
    pub current_process_active: bool,
    pub current_stock_updated: bool,
    pub out_buffer: Vec<u8>,
    pub err_buffer: Vec<u8>,
    pub no_data_stocks: String,
    error: String,
    current_process: emProcess,
}

impl emStocksPricesFetcher {
    pub fn new(
        api_script: &str,
        api_script_interpreter: &str,
        api_key: &str,
    ) -> Self {
        Self {
            api_script: api_script.to_string(),
            api_script_interpreter: api_script_interpreter.to_string(),
            api_key: api_key.to_string(),
            stock_ids: Vec::new(),
            stock_index_map: BTreeMap::new(),
            current_index: 0,
            current_symbol: String::new(),
            current_start_date: String::new(),
            current_process_active: false,
            current_stock_updated: false,
            out_buffer: Vec::new(),
            err_buffer: Vec::new(),
            no_data_stocks: String::new(),
            error: String::new(),
            current_process: emProcess::new(),
        }
    }

    /// Port of C++ AddStockIds.
    pub fn AddStockIds(&mut self, stock_ids: &[String]) {
        for id in stock_ids {
            if !self.stock_index_map.contains_key(id) {
                self.stock_ids.push(id.clone());
                self.stock_index_map.insert(id.clone(), None);
            }
        }
        self.error.clear();
    }

    /// Port of C++ GetCurrentStockId.
    pub fn GetCurrentStockId(&self) -> Option<&str> {
        if self.current_index < 0
            || self.current_index as usize >= self.stock_ids.len()
        {
            return None;
        }
        Some(&self.stock_ids[self.current_index as usize])
    }

    /// Port of C++ GetProgressInPercent.
    pub fn GetProgressInPercent(&self) -> f64 {
        if self.current_index < 0
            || self.current_index as usize >= self.stock_ids.len()
        {
            return 100.0;
        }
        (self.current_index as f64 + 0.5) * 100.0 / self.stock_ids.len() as f64
    }

    /// Port of C++ HasFinished.
    pub fn HasFinished(&self) -> bool {
        self.current_index < 0
            || self.current_index as usize >= self.stock_ids.len()
    }

    /// Port of C++ GetError.
    pub fn GetError(&self) -> &str {
        &self.error
    }

    /// Port of C++ SetFailed.
    pub fn SetFailed(&mut self, error: &str) {
        self.Clear();
        self.error = error.to_string();
    }

    /// Port of C++ Clear.
    pub fn Clear(&mut self) {
        self.stock_ids.clear();
        self.stock_index_map.clear();
        self.current_index = 0;
        self.current_symbol.clear();
        self.current_start_date.clear();
        self.current_process
            .Terminate(std::time::Duration::from_secs(20));
        self.current_process_active = false;
        self.current_stock_updated = false;
        self.out_buffer.clear();
        self.err_buffer.clear();
        self.no_data_stocks.clear();
        self.error.clear();
    }

    /// Port of C++ CalculateDate. Takes the stock rec to calculate start date.
    pub fn CalculateDate(&mut self, stock_rec: Option<&StockRec>) {
        let current_date = GetCurrentDate();

        let d = match stock_rec {
            Some(sr) if !sr.last_price_date.is_empty() => {
                let (diff, _) =
                    GetDateDifference(&sr.last_price_date, &current_date);
                let d = diff + 1;
                d.max(1).min(StockRec::MAX_NUM_PRICES as i32)
            }
            _ => StockRec::MAX_NUM_PRICES as i32,
        };

        self.current_start_date = AddDaysToDate(1 - d, &current_date);
    }

    /// Port of C++ ProcessOutBufferLines.
    /// Processes complete lines in out_buffer, leaving partial last line.
    pub fn ProcessOutBufferLines(&mut self, rec: &mut emStocksRec) {
        let mut pos = 0;
        let len = self.out_buffer.len();

        loop {
            // Find line break (0x0d or 0x0a)
            let mut brk = pos;
            while brk < len
                && self.out_buffer[brk] != 0x0d
                && self.out_buffer[brk] != 0x0a
            {
                brk += 1;
            }
            if brk >= len {
                break;
            }

            // Extract line as string (replacing the break char with nul like C++)
            let line =
                String::from_utf8_lossy(&self.out_buffer[pos..brk]).to_string();
            self.ProcessOutBufferLine(&line, rec);

            // Skip consecutive line break chars
            brk += 1;
            while brk < len
                && (self.out_buffer[brk] == 0x0d
                    || self.out_buffer[brk] == 0x0a)
            {
                brk += 1;
            }
            pos = brk;
        }

        if pos > 0 {
            self.out_buffer.drain(..pos);
        }
    }

    /// Port of C++ ProcessOutBufferLine.
    /// Parses a single line "YYYY-MM-DD price" and calls AddPrice.
    pub fn ProcessOutBufferLine(
        &mut self,
        line: &str,
        rec: &mut emStocksRec,
    ) {
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut pos = 0;

        // Skip leading whitespace (unsigned char <= 0x20)
        while pos < len && bytes[pos] <= 0x20 {
            pos += 1;
        }

        let mut ymd = [0i32; 3];
        for (i, slot) in ymd.iter_mut().enumerate() {
            // Need at least one digit
            if pos >= len || bytes[pos] < b'0' || bytes[pos] > b'9' {
                return;
            }
            let mut d = (bytes[pos] - b'0') as i32;
            pos += 1;
            while pos < len && bytes[pos] >= b'0' && bytes[pos] <= b'9' {
                d = d * 10 + (bytes[pos] - b'0') as i32;
                pos += 1;
            }
            *slot = d;
            if i < 2 {
                if pos >= len || bytes[pos] != b'-' {
                    return;
                }
                pos += 1;
            }
        }

        let date = format!("{:04}-{:02}-{:02}", ymd[0], ymd[1], ymd[2]);

        if CompareDates(&date, &self.current_start_date) < 0 {
            return;
        }

        // Skip to price number
        while pos < len
            && (bytes[pos] < b'0' || bytes[pos] > b'9')
            && bytes[pos] != b'-'
            && bytes[pos] != b'.'
        {
            pos += 1;
        }
        if pos >= len {
            return;
        }

        // Parse the price as f64 (port of C++ atof(str))
        let price_str = &line[pos..];
        let price_f64: f64 = match price_str
            .split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
            .next()
        {
            Some(s) => s.parse().unwrap_or(0.0),
            None => return,
        };
        let price = SharePriceToString(price_f64);

        self.AddPriceToStock(&date, &price, rec);
    }

    /// Port of C++ AddPrice (the fetcher's version, which updates StockRec and ListBoxes).
    fn AddPriceToStock(
        &mut self,
        date: &str,
        price: &str,
        rec: &mut emStocksRec,
    ) {
        let idx = match self.GetCurrentStockRecIndex(rec) {
            Some(i) => i,
            None => return,
        };

        // DIVERGED: ListBox date-selection update skipped — emStocksListBox not yet ported (Task 13).
        // C++ checks if date > last_price_date and date > latest_prices_date, then updates ListBox
        // selected dates. That logic will be added when emStocksListBox is implemented.

        rec.stocks[idx].AddPrice(date, price);
        self.current_stock_updated = true;
    }

    /// Build the argv vector for the API script process.
    /// Port of the argv construction in C++ StartProcess.
    pub(crate) fn BuildProcessArgv(&self) -> Vec<String> {
        let mut args = Vec::new();
        if !self.api_script_interpreter.is_empty() {
            args.push(self.api_script_interpreter.clone());
        }
        args.push(self.api_script.clone());
        args.push(self.current_symbol.clone());
        args.push(self.current_start_date.clone());
        args.push(self.api_key.clone());
        args
    }

    /// Port of C++ Cycle.
    /// DIVERGED: No FileModel file-state check — caller is responsible for ensuring
    /// the model is in a loaded/unsaved state before calling Cycle.
    pub fn Cycle(&mut self, rec: &mut emStocksRec) -> bool {
        if self.current_process_active {
            self.PollProcess(rec);
        }
        if !self.current_process_active {
            self.StartProcess(rec);
        }
        self.current_process_active
    }

    /// Port of C++ StartProcess.
    pub fn StartProcess(&mut self, rec: &mut emStocksRec) {
        if self.current_process_active {
            return;
        }

        loop {
            if self.current_index as usize >= self.stock_ids.len() {
                return;
            }

            let stock_rec_idx = self.GetCurrentStockRecIndex(rec);
            match stock_rec_idx {
                Some(idx) if !rec.stocks[idx].symbol.is_empty() => {
                    self.current_symbol = rec.stocks[idx].symbol.clone();
                    break;
                }
                _ => {
                    self.current_index += 1;
                    continue;
                }
            }
        }

        self.out_buffer.clear();
        self.err_buffer.clear();
        self.current_process_active = true;

        let stock_rec_idx = self.GetCurrentStockRecIndex(rec);
        let stock_rec = stock_rec_idx.map(|i| &rec.stocks[i]);
        self.CalculateDate(stock_rec);

        if self.api_script.is_empty() {
            self.SetFailed("API script is not set.");
            return;
        }

        let argv = self.BuildProcessArgv();
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        let env = HashMap::new();
        let flags = StartFlags::PIPE_STDOUT | StartFlags::PIPE_STDERR;
        if let Err(e) = self.current_process.TryStart(&argv_refs, &env, None, flags) {
            self.SetFailed(&e.to_string());
        }
    }

    /// Port of C++ PollProcess.
    pub fn PollProcess(&mut self, rec: &mut emStocksRec) {
        if !self.current_process_active {
            return;
        }

        // Read stdout
        let mut stdout_closed = false;
        loop {
            let mut tmp = [0u8; 128];
            let result = match self.current_process.TryRead(&mut tmp) {
                Ok(r) => r,
                Err(e) => {
                    self.SetFailed(&e.to_string());
                    return;
                }
            };
            match result {
                PipeResult::Bytes(n) => {
                    self.out_buffer.extend_from_slice(&tmp[..n]);
                    self.ProcessOutBufferLines(rec);
                    if self.out_buffer.len() > 100000 {
                        self.SetFailed("API script printed a too long line.");
                        return;
                    }
                }
                PipeResult::WouldBlock => break,
                PipeResult::Closed => {
                    stdout_closed = true;
                    break;
                }
            }
        }

        // Read stderr
        let mut stderr_closed = false;
        loop {
            let mut tmp = [0u8; 128];
            let result = match self.current_process.TryReadErr(&mut tmp) {
                Ok(r) => r,
                Err(e) => {
                    self.SetFailed(&e.to_string());
                    return;
                }
            };
            match result {
                PipeResult::Bytes(n) => {
                    self.err_buffer.extend_from_slice(&tmp[..n]);
                    if self.err_buffer.len() > 100000 {
                        self.SetFailed(
                            "API script printed too much data on stderr.",
                        );
                        return;
                    }
                }
                PipeResult::WouldBlock => break,
                PipeResult::Closed => {
                    stderr_closed = true;
                    break;
                }
            }
        }

        // If either pipe is still open, keep polling
        if !stdout_closed || !stderr_closed {
            return;
        }

        // Both pipes closed — check if process is still running
        if self.current_process.IsRunning() {
            return;
        }

        // Process has exited
        let exit_status = self.current_process.GetExitStatus().unwrap_or(-1);
        if exit_status != 0 {
            let err_str = String::from_utf8_lossy(&self.err_buffer);
            self.SetFailed(&format!(
                "API script failed for \"{}\":\n{}",
                self.current_symbol, err_str
            ));
            return;
        }

        if !self.current_stock_updated {
            if let Some(idx) = self.GetCurrentStockRecIndex(rec) {
                let symbol = &rec.stocks[idx].symbol;
                let name = &rec.stocks[idx].name;
                self.no_data_stocks +=
                    &format!("  {} - {}\n", symbol, name);
            }
        }

        if !self.no_data_stocks.is_empty()
            && self.current_index + 1 >= self.stock_ids.len() as i32
        {
            self.SetFailed(&format!(
                "Could not fetch any new data for:\n{}",
                self.no_data_stocks
            ));
            return;
        }

        self.current_index += 1;
        self.current_symbol.clear();
        self.current_start_date.clear();
        self.current_process_active = false;
        self.current_stock_updated = false;
        self.out_buffer.clear();
        self.err_buffer.clear();
        if self.current_index as usize >= self.stock_ids.len() {
            self.Clear();
        }
    }

    /// Resolve stock ID to StockRec index in the emStocksRec.
    fn GetStockRecIndex(
        &self,
        stock_id: &str,
        rec: &emStocksRec,
    ) -> Option<usize> {
        // Check cached index first
        if let Some(Some(idx)) = self.stock_index_map.get(stock_id) {
            if *idx < rec.stocks.len() && rec.stocks[*idx].id == stock_id {
                return Some(*idx);
            }
        }

        // Linear scan to find matching stock
        for (i, stock) in rec.stocks.iter().enumerate() {
            if stock.id == stock_id {
                return Some(i);
            }
        }
        None
    }

    /// Resolve current stock to StockRec index, updating the cache.
    fn GetCurrentStockRecIndex(&mut self, rec: &emStocksRec) -> Option<usize> {
        let stock_id = match self.GetCurrentStockId() {
            Some(id) => id.to_string(),
            None => return None,
        };
        let idx = self.GetStockRecIndex(&stock_id, rec);
        // Update cache
        if let Some(entry) = self.stock_index_map.get_mut(&stock_id) {
            *entry = idx;
        }
        idx
    }
}

impl emEngine for emStocksPricesFetcher {
    fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
        // DIVERGED(Phase 4): FileModel/FileStateSignal/ChangeSignal integration pending.
        // Once FileModel is integrated, this will read rec from the model and check file state.
        // For now, this trait impl cannot call the internal Cycle because it needs
        // a &mut emStocksRec. The caller must use the direct Cycle(&mut rec) method.
        self.current_process_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetcher_initially_finished() {
        let fetcher = emStocksPricesFetcher::new("", "", "");
        assert!(fetcher.HasFinished());
    }

    #[test]
    fn fetcher_add_stock_ids() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.pl", "perl", "key123");
        fetcher.AddStockIds(&["1".to_string(), "2".to_string()]);
        assert!(!fetcher.HasFinished());
        assert_eq!(fetcher.GetCurrentStockId(), Some("1"));
    }

    #[test]
    fn fetcher_progress() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.AddStockIds(&["1".to_string(), "2".to_string()]);
        // At index 0 of 2: (0 + 0.5) * 100 / 2 = 25.0
        assert_eq!(fetcher.GetProgressInPercent(), 25.0);
    }

    #[test]
    fn fetcher_no_duplicate_stock_ids() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.AddStockIds(&["1".to_string(), "2".to_string()]);
        fetcher.AddStockIds(&["2".to_string(), "3".to_string()]);
        assert_eq!(fetcher.stock_ids.len(), 3); // 1, 2, 3
    }

    #[test]
    fn process_out_buffer_line_valid() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.current_start_date = "2024-01-01".to_string();
        let mut rec = emStocksRec::default();
        let mut stock = StockRec::default();
        stock.id = "1".to_string();
        stock.symbol = "TST".to_string();
        rec.stocks.push(stock);
        fetcher.AddStockIds(&["1".to_string()]);

        fetcher.ProcessOutBufferLine("2024-03-15 100.50", &mut rec);
        assert_eq!(
            rec.stocks[0].GetPriceOfDate("2024-03-15"),
            SharePriceToString(100.50)
        );
    }

    #[test]
    fn process_out_buffer_line_before_start_date() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.current_start_date = "2024-06-01".to_string();
        let mut rec = emStocksRec::default();
        let mut stock = StockRec::default();
        stock.id = "1".to_string();
        rec.stocks.push(stock);
        fetcher.AddStockIds(&["1".to_string()]);

        fetcher.ProcessOutBufferLine("2024-03-15 100.50", &mut rec);
        assert_eq!(rec.stocks[0].GetPriceOfDate("2024-03-15"), "");
    }

    #[test]
    fn process_out_buffer_line_invalid() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.current_start_date = "2024-01-01".to_string();
        let mut rec = emStocksRec::default();

        // No crash on invalid input
        fetcher.ProcessOutBufferLine("not a date", &mut rec);
        fetcher.ProcessOutBufferLine("", &mut rec);
        fetcher.ProcessOutBufferLine("2024-03-15", &mut rec);
    }

    #[test]
    fn process_out_buffer_lines_splits_correctly() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.current_start_date = "2024-01-01".to_string();
        let mut rec = emStocksRec::default();
        let mut stock = StockRec::default();
        stock.id = "1".to_string();
        rec.stocks.push(stock);
        fetcher.AddStockIds(&["1".to_string()]);

        fetcher.out_buffer =
            b"2024-03-14 99.0\n2024-03-15 100.5\n2024-03-".to_vec();
        fetcher.ProcessOutBufferLines(&mut rec);
        assert_eq!(fetcher.out_buffer, b"2024-03-");
        assert_eq!(rec.stocks[0].last_price_date, "2024-03-15");
    }

    #[test]
    fn calculate_date_no_existing_prices() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        let stock = StockRec::default();
        fetcher.CalculateDate(Some(&stock));
        assert!(!fetcher.current_start_date.is_empty());
    }

    #[test]
    fn calculate_date_with_existing_prices() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        let mut stock = StockRec::default();
        stock.last_price_date = GetCurrentDate();
        fetcher.CalculateDate(Some(&stock));
        assert!(!fetcher.current_start_date.is_empty());
    }

    #[test]
    fn build_argv_with_interpreter() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.pl", "perl", "key123");
        fetcher.current_symbol = "AAPL".to_string();
        fetcher.current_start_date = "2024-01-01".to_string();
        let argv = fetcher.BuildProcessArgv();
        assert_eq!(
            argv,
            vec!["perl", "script.pl", "AAPL", "2024-01-01", "key123"]
        );
    }

    #[test]
    fn build_argv_without_interpreter() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.py", "", "mykey");
        fetcher.current_symbol = "GOOG".to_string();
        fetcher.current_start_date = "2024-06-01".to_string();
        let argv = fetcher.BuildProcessArgv();
        assert_eq!(
            argv,
            vec!["script.py", "GOOG", "2024-06-01", "mykey"]
        );
    }

    #[test]
    fn start_process_skips_stocks_without_symbol() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.py", "", "key");
        let mut rec = emStocksRec::default();

        // Stock 1: no symbol (should be skipped)
        let mut s1 = StockRec::default();
        s1.id = "1".to_string();
        s1.symbol = String::new();
        rec.stocks.push(s1);

        // Stock 2: has symbol but use a real command so TryStart succeeds
        let mut s2 = StockRec::default();
        s2.id = "2".to_string();
        s2.symbol = "TST".to_string();
        rec.stocks.push(s2);

        // Use /bin/echo as the "script" so the process actually starts
        fetcher.api_script = "/bin/echo".to_string();
        fetcher.AddStockIds(&["1".to_string(), "2".to_string()]);

        fetcher.StartProcess(&mut rec);
        // current_index should have advanced past stock 1
        assert_eq!(fetcher.current_index, 1);
        assert_eq!(fetcher.current_symbol, "TST");
        assert!(fetcher.current_process_active);
    }

    #[test]
    fn start_process_fails_when_api_script_empty() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "key");
        let mut rec = emStocksRec::default();

        let mut s = StockRec::default();
        s.id = "1".to_string();
        s.symbol = "TST".to_string();
        rec.stocks.push(s);

        fetcher.AddStockIds(&["1".to_string()]);
        fetcher.StartProcess(&mut rec);

        assert_eq!(fetcher.GetError(), "API script is not set.");
        assert!(fetcher.HasFinished());
    }

    #[test]
    fn cycle_returns_false_when_finished() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        let mut rec = emStocksRec::default();
        assert!(!fetcher.Cycle(&mut rec));
    }

    #[test]
    fn poll_process_handles_exit_with_error() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.py", "", "key");
        // Simulate an active process state without real process
        // (PollProcess will see pipes closed and process not running)
        fetcher.current_process_active = true;
        fetcher.current_symbol = "TST".to_string();

        let mut rec = emStocksRec::default();
        let mut s = StockRec::default();
        s.id = "1".to_string();
        s.symbol = "TST".to_string();
        rec.stocks.push(s);
        fetcher.AddStockIds(&["1".to_string()]);

        // PollProcess with no real child: pipes return Closed, process not running,
        // exit status is None (maps to -1), so it should fail.
        fetcher.PollProcess(&mut rec);

        // Should have set error since exit status != 0
        assert!(!fetcher.GetError().is_empty());
        assert!(fetcher.GetError().contains("API script failed"));
    }

    #[test]
    fn start_process_returns_early_when_all_done() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.py", "", "key");
        let mut rec = emStocksRec::default();
        // No stocks added — StartProcess should return immediately
        fetcher.StartProcess(&mut rec);
        assert!(!fetcher.current_process_active);
    }

    #[test]
    fn start_process_skips_missing_stock_rec() {
        let mut fetcher =
            emStocksPricesFetcher::new("script.py", "", "key");
        let mut rec = emStocksRec::default();
        // Add stock ID "1" but no matching StockRec in rec
        fetcher.AddStockIds(&["1".to_string()]);
        fetcher.StartProcess(&mut rec);
        // Should have skipped past it without activating
        assert!(!fetcher.current_process_active);
        assert!(fetcher.HasFinished());
    }
}
