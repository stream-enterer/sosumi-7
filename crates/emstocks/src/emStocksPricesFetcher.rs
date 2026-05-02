// Port of C++ emStocksPricesFetcher.h / emStocksPricesFetcher.cpp
// emEngine trait Cycle cannot drive the fetch loop because it needs a
// &mut emStocksRec parameter that the trait signature doesn't provide. This is a structural
// limitation of the trait pattern. The direct Cycle(&mut rec) method is used instead.
// Uses BTreeMap<String, Option<usize>> instead of C++ emAvlTreeMap<String, emCrossPtr<StockRec>> — BTreeMap is Rust's idiomatic ordered map; cross-pointers don't apply when StockRecs live in a Vec.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use emcore::emEngine::{emEngine, EngineId};
use emcore::emEngineCtx::{EngineCtx, SignalCtx};
use emcore::emFileModel::FileState;
use emcore::emProcess::{emProcess, PipeResult, StartFlags};
use emcore::emSignal::SignalId;
use slotmap::Key as _;

use super::emStocksFileModel::emStocksFileModel;
use super::emStocksRec::{
    emStocksRec, AddDaysToDate, CompareDates, GetCurrentDate, GetDateDifference,
    SharePriceToString, StockRec,
};

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
    pub(crate) latest_prices_date: String,
    error: String,
    current_process: emProcess,
    /// Lazy-allocated per D-008 A1. Null until first subscriber.
    /// Mirrors C++ `emSignal ChangeSignal` (emStocksPricesFetcher.h:103).
    /// RUST_ONLY: (language-forced-utility) — Rust struct does not inherit from
    /// `emConfigModel`; the SignalId lives directly per design Option B
    /// (mirrors G2 in `2026-04-27-B-001-no-wire-emstocks-design.md` §G3).
    change_signal: Cell<SignalId>,
    /// Optional reference to the owning FileModel. Mirrors C++
    /// `emRef<emStocksFileModel> FileModel;` (emStocksPricesFetcher.h:85).
    /// `None` is permitted at construction time (legacy ctor flow) and for
    /// the bare unit tests that exercise the fetch state machine without an
    /// owning model; `Some(_)` is required for the engine-mirror `cycle()`
    /// path (B-001-followup Phase E).
    ///
    /// RUST_ONLY: (language-forced-utility) — Rust has no by-reference
    /// member field equivalent to C++ `emRef<>`; `Rc<RefCell<>>` is the
    /// canonical codebase shape for cross-Cycle-shared model references
    /// (CLAUDE.md §Ownership rule (a) — engine-callback-held). The C++
    /// inline `emRef<emStocksFileModel> FileModel;` is provided implicitly
    /// by the language; the Rust wrapper makes the same shape explicit.
    /// Justification: the proxy-engine driver (the dialog's `Cycle`) holds
    /// the fetcher across scheduler ticks and reaches into the model
    /// through this ref.
    file_model: Option<Rc<RefCell<emStocksFileModel>>>,
    /// First-Cycle init latch for the FileModel signal subscribes (B-001-followup
    /// Phase E.1). Mirrors C++ ctor `AddWakeUpSignal(...)` at
    /// `emStocksPricesFetcher.cpp:38-39`, deferred to first `cycle()` per
    /// D-006 (the `new()` ctor has no `EngineCtx`/`engine_id` reach).
    subscribed_init: bool,
    /// Cached `emStocksFileModel::GetChangeSignal` id captured at first
    /// `cycle()`. `None` until `subscribed_init` flips. Mirrors C++ ctor
    /// `AddWakeUpSignal(FileModel->GetChangeSignal())`
    /// (emStocksPricesFetcher.cpp:38).
    file_model_change_sig: Option<SignalId>,
    /// Cached `emStocksFileModel::GetFileStateSignal` id captured at first
    /// `cycle()`. `None` until `subscribed_init` flips. Mirrors C++ ctor
    /// `AddWakeUpSignal(FileModel->GetFileStateSignal())`
    /// (emStocksPricesFetcher.cpp:39). UPSTREAM-GAP: the underlying signal
    /// id is `SignalId::default()` (null) in the standalone-port
    /// `emRecFileModel`; the connect call below is a no-op for null but the
    /// subscribe site is preserved per the upstream-gap convention so a
    /// future emRecFileModel promotion plugs in without callsite changes.
    file_model_state_sig: Option<SignalId>,
}

impl emStocksPricesFetcher {
    pub fn new(api_script: &str, api_script_interpreter: &str, api_key: &str) -> Self {
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
            latest_prices_date: String::new(),
            error: String::new(),
            current_process: emProcess::new(),
            change_signal: Cell::new(SignalId::null()),
            file_model: None,
            subscribed_init: false,
            file_model_change_sig: None,
            file_model_state_sig: None,
        }
    }

    /// Builder-style attach for the owning `emStocksFileModel`. Mirrors C++
    /// ctor parameter `emStocksFileModel & fileModel` at
    /// `emStocksPricesFetcher.cpp:24`. Required for the engine-mirror
    /// `cycle()` path; B-001-followup Phase E.1 wires this from
    /// `emStocksFetchPricesDialog::new` so the dialog's proxy-engine
    /// `Cycle` can drive the fetcher with full FileModel reach.
    pub fn with_file_model(mut self, file_model: Rc<RefCell<emStocksFileModel>>) -> Self {
        self.file_model = Some(file_model);
        self
    }

    /// Port of inherited C++ `emConfigModel::GetChangeSignal` (the C++ emStocksPricesFetcher
    /// owns the `ChangeSignal` directly). D-008 A1 combined-form: lazy alloc.
    pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let cur = self.change_signal.get();
        if cur.is_null() {
            let new_id = ectx.create_signal();
            self.change_signal.set(new_id);
            new_id
        } else {
            cur
        }
    }

    /// Port of C++ `emSignal::Signal()` on `ChangeSignal`. Synchronous fire per
    /// D-007 (`&mut impl SignalCtx`). No-op when `change_signal` is null
    /// (matches C++ `emSignal::Signal()` with zero subscribers).
    ///
    /// Mirrors C++ `Signal(ChangeSignal)` callsites at
    /// `emStocksPricesFetcher.cpp:70, 134, 264, 272`. The Rust analogues are
    /// `AddStockIds`, `CalculateDate` (post-success path), and the two
    /// `PollProcess` end-of-cycle paths. CALLSITE-NOTE: Rust mutator-fire is
    /// wired here via `pub fn Signal`; the consumer-side subscribe lives in
    /// B-017 row 1 (`emStocksFetchPricesDialog-62`), not B-001 — accessor is
    /// added now to unblock that bucket.
    pub fn Signal(&self, ectx: &mut impl SignalCtx) {
        let s = self.change_signal.get();
        if !s.is_null() {
            ectx.fire(s);
        }
    }

    /// Test-only accessor for the raw SignalId slot without allocating.
    #[doc(hidden)]
    pub fn change_signal_for_test(&self) -> SignalId {
        self.change_signal.get()
    }

    /// Port of C++ AddStockIds. Mirrors C++ fire site at
    /// `emStocksPricesFetcher.cpp:70` (`Signal(ChangeSignal)` after Error.Clear).
    pub fn AddStockIds(&mut self, ectx: &mut impl SignalCtx, stock_ids: &[String]) {
        for id in stock_ids {
            if !self.stock_index_map.contains_key(id) {
                self.stock_ids.push(id.clone());
                self.stock_index_map.insert(id.clone(), None);
            }
        }
        self.error.clear();
        self.Signal(ectx);
    }

    /// Port of C++ GetCurrentStockId.
    pub fn GetCurrentStockId(&self) -> Option<&str> {
        if self.current_index < 0 || self.current_index as usize >= self.stock_ids.len() {
            return None;
        }
        Some(&self.stock_ids[self.current_index as usize])
    }

    /// Port of C++ GetProgressInPercent.
    pub fn GetProgressInPercent(&self) -> f64 {
        if self.current_index < 0 || self.current_index as usize >= self.stock_ids.len() {
            return 100.0;
        }
        (self.current_index as f64 + 0.5) * 100.0 / self.stock_ids.len() as f64
    }

    /// Port of C++ HasFinished.
    pub fn HasFinished(&self) -> bool {
        self.current_index < 0 || self.current_index as usize >= self.stock_ids.len()
    }

    /// Port of C++ GetError.
    pub fn GetError(&self) -> &str {
        &self.error
    }

    /// Port of C++ SetFailed. Mirrors C++ fire site at
    /// `emStocksPricesFetcher.cpp:272` (`Signal(ChangeSignal)` after error set).
    pub fn SetFailed(&mut self, ectx: &mut impl SignalCtx, error: &str) {
        self.Clear();
        self.error = error.to_string();
        self.Signal(ectx);
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
        self.latest_prices_date.clear();
        self.error.clear();
    }

    /// Port of C++ CalculateDate. Takes the stock rec to calculate start date.
    pub fn CalculateDate(&mut self, stock_rec: Option<&StockRec>) {
        let current_date = GetCurrentDate();

        let d = match stock_rec {
            Some(sr) if !sr.last_price_date.is_empty() => {
                let (diff, _) = GetDateDifference(&sr.last_price_date, &current_date);
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
            while brk < len && self.out_buffer[brk] != 0x0d && self.out_buffer[brk] != 0x0a {
                brk += 1;
            }
            if brk >= len {
                break;
            }

            // Extract line as string (replacing the break char with nul like C++)
            let line = String::from_utf8_lossy(&self.out_buffer[pos..brk]).to_string();
            self.ProcessOutBufferLine(&line, rec);

            // Skip consecutive line break chars
            brk += 1;
            while brk < len && (self.out_buffer[brk] == 0x0d || self.out_buffer[brk] == 0x0a) {
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
    pub fn ProcessOutBufferLine(&mut self, line: &str, rec: &mut emStocksRec) {
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
    fn AddPriceToStock(&mut self, date: &str, price: &str, rec: &mut emStocksRec) {
        let idx = match self.GetCurrentStockRecIndex(rec) {
            Some(i) => i,
            None => return,
        };

        // C++ updates ListBox selected date when date > last_price_date.
        // Rust: ListBox date-selection is managed by the caller (FilePanel/Dialog)
        // after Cycle returns, following the explicit-parameter pattern.
        // Track the latest date seen during this fetch cycle (mirrors C++ LatestPricesDate).
        if CompareDates(date, &rec.stocks[idx].last_price_date) > 0
            && CompareDates(date, &self.latest_prices_date) > 0
        {
            self.latest_prices_date = date.to_string();
        }

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
    /// File-state guard: caller ensures model is in loaded/unsaved state before calling Cycle.
    /// This matches the explicit-parameter pattern (no shared FileModel reference).
    pub fn Cycle(&mut self, ectx: &mut impl SignalCtx, rec: &mut emStocksRec) -> bool {
        if self.current_process_active {
            self.PollProcess(ectx, rec);
        }
        if !self.current_process_active {
            self.StartProcess(ectx, rec);
        }
        self.current_process_active
    }

    /// Engine-mirror Cycle entry point (B-001-followup Phase E.1). Port of
    /// C++ `emStocksPricesFetcher::Cycle()` at `emStocksPricesFetcher.cpp:102-116`:
    ///
    /// ```cpp
    /// bool emStocksPricesFetcher::Cycle()
    /// {
    ///     switch (FileModel->GetFileState()) {
    ///         case emFileModel::FS_LOADED:
    ///         case emFileModel::FS_UNSAVED:
    ///             break;
    ///         default:
    ///             return false;
    ///     }
    ///     if (CurrentProcessActive) PollProcess();
    ///     if (!CurrentProcessActive) StartProcess();
    ///     return CurrentProcessActive;
    /// }
    /// ```
    ///
    /// The dialog's proxy `Cycle` invokes this on every slice; on the first
    /// call this method performs the deferred upstream-subscribe step (C++
    /// ctor `AddWakeUpSignal(FileModel->GetChangeSignal())` +
    /// `AddWakeUpSignal(FileModel->GetFileStateSignal())` at cpp:38-39),
    /// using the dialog's `engine_id` per the panel-as-proxy-engine pattern
    /// (B-017 SaveTimer precedent at `emStocksFilePanel.cpp:454-...`).
    ///
    /// Returns `false` (idle) if no FileModel has been attached or if the
    /// model is not in `Loaded`/`Unsaved` — mirrors C++ `default: return false`.
    pub fn cycle(&mut self, ectx: &mut EngineCtx<'_>, eid: EngineId) -> bool {
        let Some(file_model) = self.file_model.clone() else {
            return false;
        };

        // First-Cycle subscribe — D-006 deferred init.
        if !self.subscribed_init {
            // GetChangeSignal lazily allocates if needed; capture into the
            // option slot. GetFileStateSignal currently delegates to a null
            // SignalId per the UPSTREAM-GAP on emRecFileModel; connect is
            // null-safe so we still preserve the subscribe site for future
            // promotion.
            let change_sig = file_model.borrow().GetChangeSignal(ectx);
            let state_sig = file_model.borrow().GetFileStateSignal();
            ectx.connect(change_sig, eid);
            ectx.connect(state_sig, eid);
            self.file_model_change_sig = Some(change_sig);
            self.file_model_state_sig = Some(state_sig);
            self.subscribed_init = true;
        }

        // C++ Cycle body: file-state guard, then drive PollProcess /
        // StartProcess. The `IsSignaled` checks are NOT used to gate the
        // body — C++ `Cycle()` always evaluates the switch and runs the
        // body when state permits; the wakeup signals only ensure the
        // engine is woken when upstream changes. We mirror that exactly.
        let file_state_ok = matches!(
            file_model.borrow().GetFileState(),
            FileState::Loaded | FileState::Unsaved
        );
        if !file_state_ok {
            return false;
        }

        // Borrow rec mutably from the model; the existing PollProcess /
        // StartProcess paths consume `&mut emStocksRec`. We hold the borrow
        // only for the duration of the body — no nested model borrows.
        let mut model = file_model.borrow_mut();
        let rec = model.GetWritableRec(ectx);
        if self.current_process_active {
            self.PollProcess(ectx, rec);
        }
        if !self.current_process_active {
            self.StartProcess(ectx, rec);
        }
        self.current_process_active
    }

    /// Test/internal accessor for the upstream-subscribe latch.
    #[doc(hidden)]
    pub fn subscribed_init_for_test(&self) -> bool {
        self.subscribed_init
    }

    /// Test/internal accessor for the cached upstream-change SignalId.
    #[doc(hidden)]
    pub fn file_model_change_sig_for_test(&self) -> Option<SignalId> {
        self.file_model_change_sig
    }

    /// Test/internal accessor for the cached upstream-state SignalId.
    #[doc(hidden)]
    pub fn file_model_state_sig_for_test(&self) -> Option<SignalId> {
        self.file_model_state_sig
    }

    /// Port of C++ StartProcess. Mirrors C++ fire site at
    /// `emStocksPricesFetcher.cpp:134` (`Signal(ChangeSignal)` inside the
    /// skip-loop after `CurrentIndex++`).
    pub fn StartProcess(&mut self, ectx: &mut impl SignalCtx, rec: &mut emStocksRec) {
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
                    self.Signal(ectx);
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
            self.SetFailed(ectx, "API script is not set.");
            return;
        }

        let argv = self.BuildProcessArgv();
        let argv_refs: Vec<&str> = argv.iter().map(|s| s.as_str()).collect();
        let env = HashMap::new();
        let flags = StartFlags::PIPE_STDOUT | StartFlags::PIPE_STDERR;
        if let Err(e) = self.current_process.TryStart(&argv_refs, &env, None, flags) {
            self.SetFailed(ectx, &e.to_string());
        }
    }

    /// Port of C++ PollProcess. Mirrors C++ fire site at
    /// `emStocksPricesFetcher.cpp:264` (`Signal(ChangeSignal)` at end of
    /// per-stock completion before returning).
    pub fn PollProcess(&mut self, ectx: &mut impl SignalCtx, rec: &mut emStocksRec) {
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
                    self.SetFailed(ectx, &e.to_string());
                    return;
                }
            };
            match result {
                PipeResult::Bytes(n) => {
                    self.out_buffer.extend_from_slice(&tmp[..n]);
                    self.ProcessOutBufferLines(rec);
                    if self.out_buffer.len() > 100000 {
                        self.SetFailed(ectx, "API script printed a too long line.");
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
                    self.SetFailed(ectx, &e.to_string());
                    return;
                }
            };
            match result {
                PipeResult::Bytes(n) => {
                    self.err_buffer.extend_from_slice(&tmp[..n]);
                    if self.err_buffer.len() > 100000 {
                        self.SetFailed(ectx, "API script printed too much data on stderr.");
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
            self.SetFailed(
                ectx,
                &format!(
                    "API script failed for \"{}\":\n{}",
                    self.current_symbol, err_str
                ),
            );
            return;
        }

        if !self.current_stock_updated {
            if let Some(idx) = self.GetCurrentStockRecIndex(rec) {
                let symbol = &rec.stocks[idx].symbol;
                let name = &rec.stocks[idx].name;
                self.no_data_stocks += &format!("  {} - {}\n", symbol, name);
            }
        }

        if !self.no_data_stocks.is_empty() && self.current_index + 1 >= self.stock_ids.len() as i32
        {
            self.SetFailed(
                ectx,
                &format!("Could not fetch any new data for:\n{}", self.no_data_stocks),
            );
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
        self.Signal(ectx);
    }

    /// Resolve stock ID to StockRec index in the emStocksRec.
    fn GetStockRecIndex(&self, stock_id: &str, rec: &emStocksRec) -> Option<usize> {
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
        // emEngine trait Cycle provides scheduling hints (returns active state).
        // The actual fetch driving happens through the direct Cycle(&mut rec) method
        // because the emEngine trait signature doesn't carry &mut emStocksRec.
        self.current_process_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::DropOnlySignalCtx;
    use emcore::emScheduler::EngineScheduler;

    /// Minimal SignalCtx adapter wrapping `EngineScheduler` for unit tests.
    struct TestSignalCtx<'a> {
        sched: &'a mut EngineScheduler,
    }

    impl SignalCtx for TestSignalCtx<'_> {
        fn create_signal(&mut self) -> SignalId {
            self.sched.create_signal()
        }
        fn fire(&mut self, id: SignalId) {
            self.sched.fire(id);
        }
    }

    #[test]
    fn fetcher_initially_finished() {
        let fetcher = emStocksPricesFetcher::new("", "", "");
        assert!(fetcher.HasFinished());
    }

    #[test]
    fn get_change_signal_lazy_alloc_is_stable() {
        // G3: D-008 A1 — first call allocates, subsequent calls return same id.
        let fetcher = emStocksPricesFetcher::new("", "", "");
        assert!(fetcher.change_signal_for_test().is_null());
        let mut sched = EngineScheduler::new();
        let sig_a = {
            let mut sc = TestSignalCtx { sched: &mut sched };
            fetcher.GetChangeSignal(&mut sc)
        };
        assert!(!sig_a.is_null());
        let sig_b = {
            let mut sc = TestSignalCtx { sched: &mut sched };
            fetcher.GetChangeSignal(&mut sc)
        };
        assert_eq!(sig_a, sig_b);
    }

    #[test]
    fn fetcher_add_stock_ids() {
        let mut fetcher = emStocksPricesFetcher::new("script.pl", "perl", "key123");
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string(), "2".to_string()]);
        assert!(!fetcher.HasFinished());
        assert_eq!(fetcher.GetCurrentStockId(), Some("1"));
    }

    #[test]
    fn fetcher_progress() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string(), "2".to_string()]);
        // At index 0 of 2: (0 + 0.5) * 100 / 2 = 25.0
        assert_eq!(fetcher.GetProgressInPercent(), 25.0);
    }

    #[test]
    fn fetcher_no_duplicate_stock_ids() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string(), "2".to_string()]);
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["2".to_string(), "3".to_string()]);
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
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);

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
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);

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
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);

        fetcher.out_buffer = b"2024-03-14 99.0\n2024-03-15 100.5\n2024-03-".to_vec();
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
        let mut fetcher = emStocksPricesFetcher::new("script.pl", "perl", "key123");
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
        let mut fetcher = emStocksPricesFetcher::new("script.py", "", "mykey");
        fetcher.current_symbol = "GOOG".to_string();
        fetcher.current_start_date = "2024-06-01".to_string();
        let argv = fetcher.BuildProcessArgv();
        assert_eq!(argv, vec!["script.py", "GOOG", "2024-06-01", "mykey"]);
    }

    #[test]
    fn start_process_skips_stocks_without_symbol() {
        let mut fetcher = emStocksPricesFetcher::new("script.py", "", "key");
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
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string(), "2".to_string()]);

        fetcher.StartProcess(&mut DropOnlySignalCtx, &mut rec);
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

        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);
        fetcher.StartProcess(&mut DropOnlySignalCtx, &mut rec);

        assert_eq!(fetcher.GetError(), "API script is not set.");
        assert!(fetcher.HasFinished());
    }

    #[test]
    fn cycle_returns_false_when_finished() {
        let mut fetcher = emStocksPricesFetcher::new("", "", "");
        let mut rec = emStocksRec::default();
        assert!(!fetcher.Cycle(&mut DropOnlySignalCtx, &mut rec));
    }

    #[test]
    fn poll_process_handles_exit_with_error() {
        let mut fetcher = emStocksPricesFetcher::new("script.py", "", "key");
        // Simulate an active process state without real process
        // (PollProcess will see pipes closed and process not running)
        fetcher.current_process_active = true;
        fetcher.current_symbol = "TST".to_string();

        let mut rec = emStocksRec::default();
        let mut s = StockRec::default();
        s.id = "1".to_string();
        s.symbol = "TST".to_string();
        rec.stocks.push(s);
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);

        // PollProcess with no real child: pipes return Closed, process not running,
        // exit status is None (maps to -1), so it should fail.
        fetcher.PollProcess(&mut DropOnlySignalCtx, &mut rec);

        // Should have set error since exit status != 0
        assert!(!fetcher.GetError().is_empty());
        assert!(fetcher.GetError().contains("API script failed"));
    }

    #[test]
    fn start_process_returns_early_when_all_done() {
        let mut fetcher = emStocksPricesFetcher::new("script.py", "", "key");
        let mut rec = emStocksRec::default();
        // No stocks added — StartProcess should return immediately
        fetcher.StartProcess(&mut DropOnlySignalCtx, &mut rec);
        assert!(!fetcher.current_process_active);
    }

    #[test]
    fn start_process_skips_missing_stock_rec() {
        let mut fetcher = emStocksPricesFetcher::new("script.py", "", "key");
        let mut rec = emStocksRec::default();
        // Add stock ID "1" but no matching StockRec in rec
        fetcher.AddStockIds(&mut DropOnlySignalCtx, &["1".to_string()]);
        fetcher.StartProcess(&mut DropOnlySignalCtx, &mut rec);
        // Should have skipped past it without activating
        assert!(!fetcher.current_process_active);
        assert!(fetcher.HasFinished());
    }
}
