//! B-001-followup Phase E — `emStocksPricesFetcher` engine promotion +
//! B-017 row 1 fetcher-side regression.
//!
//! Closes I-1 (silent-undertest) from the B-017 polling-no-acc-emstocks
//! design: with the fetcher's proxy-engine `cycle()` wired through the
//! dialog's `Cycle`, FileModel transitions now drive the fetcher's reaction
//! body and (when stocks are queued) the fetcher's own `ChangeSignal`.
//!
//! Mirrors C++ `emStocksPricesFetcher.cpp:38-39` upstream subscribes
//! (`AddWakeUpSignal(FileModel->GetChangeSignal())` +
//! `AddWakeUpSignal(FileModel->GetFileStateSignal())`) and the
//! `Cycle()`-body switch on `FileModel->GetFileState()` at cpp:102-116.
//!
//! RUST_ONLY: (dependency-forced) — no C++ test analogue. Mirrors the
//! `polling_b017` test shape used by the sibling B-017 rows.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emStocks::emStocksFetchPricesDialog::emStocksFetchPricesDialog;
use emStocks::emStocksFileModel::emStocksFileModel;
use emcore::emEngine::Priority;
use emcore::emPanelScope::PanelScope;
use emcore::test_view_harness::TestViewHarness;

struct NoopEngine;
impl emcore::emEngine::emEngine for NoopEngine {
    fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
        false
    }
}

fn fresh_model() -> Rc<RefCell<emStocksFileModel>> {
    // Path is irrelevant for these tests — we never call TryLoad. The model
    // sits in `FileState::Waiting`, which means the fetcher's `cycle()`
    // performs the upstream subscribe but takes the early-return branch
    // (matches C++ `default: return false` at cpp:108-109).
    Rc::new(RefCell::new(emStocksFileModel::new(PathBuf::from(
        "/tmp/em-b001-followup-phase-e-stub.emstocks",
    ))))
}

#[test]
fn fetcher_subscribes_to_file_model_signals_on_first_cycle() {
    // Q1 (signal connected): the dialog's first Cycle drives
    // `fetcher.cycle(ectx, eid)`, which performs the deferred upstream
    // subscribe to `FileModel.GetChangeSignal` + `GetFileStateSignal`
    // (cpp:38-39) and connects the dialog's engine to each.
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let model = fresh_model();
    let mut dialog = emStocksFetchPricesDialog::new_with_model("", "", "", model.clone());

    assert!(
        !dialog.fetcher_subscribed_init_for_test(),
        "fetcher must not be subscribed before first Cycle"
    );

    {
        let mut ectx = h.engine_ctx(eid);
        let _ = dialog.Cycle(&mut ectx);
    }

    assert!(
        dialog.fetcher_subscribed_init_for_test(),
        "fetcher subscribed_init must flip true after first Cycle"
    );
    assert!(
        dialog.fetcher_file_model_change_sig_for_test().is_some(),
        "fetcher must cache FileModel.GetChangeSignal id"
    );
    // Post-FU-005: the fetcher subscribes via `ensure_file_state_signal`
    // which lazy-promotes the cell to a real (non-null) id. The cached
    // Option<SignalId> carries that real id.
    assert!(
        dialog.fetcher_file_model_state_sig_for_test().is_some(),
        "fetcher must cache FileModel.GetFileStateSignal id"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

#[test]
fn fetcher_cycle_idempotent_under_repeated_drive() {
    // Second Cycle must not re-subscribe (idempotent first-Cycle latch).
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let model = fresh_model();
    let mut dialog = emStocksFetchPricesDialog::new_with_model("", "", "", model.clone());

    {
        let mut ectx = h.engine_ctx(eid);
        let _ = dialog.Cycle(&mut ectx);
    }
    let sig_after_first = dialog.fetcher_file_model_change_sig_for_test();

    {
        let mut ectx = h.engine_ctx(eid);
        let _ = dialog.Cycle(&mut ectx);
    }
    let sig_after_second = dialog.fetcher_file_model_change_sig_for_test();

    assert_eq!(
        sig_after_first, sig_after_second,
        "first-Cycle subscribe must be idempotent across slices"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

#[test]
fn fetcher_cycle_returns_false_when_file_state_not_loaded() {
    // C++ `default: return false` at cpp:108-109. The fresh model is in
    // `FileState::Waiting`, so `cycle()` must early-return without driving
    // PollProcess/StartProcess.
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let model = fresh_model();
    let mut dialog = emStocksFetchPricesDialog::new_with_model("", "", "", model.clone());
    {
        let mut ectx = h.engine_ctx(eid);
        let _ = dialog.Cycle(&mut ectx);
    }

    // current_process_active must remain false — StartProcess never ran.
    assert!(
        !dialog.fetcher_current_process_active_for_test(),
        "fetcher must not start a process when FileState != Loaded/Unsaved"
    );
    assert!(
        dialog.fetcher_get_error_for_test().is_empty(),
        "fetcher must not record an error on the early-return path"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

#[test]
fn fetcher_subscribe_skipped_when_no_file_model_attached() {
    // Legacy `new()` callers (no FileModel) must not panic and must not
    // attempt to subscribe — the fetcher's `cycle()` returns false and
    // leaves `subscribed_init` false.
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let mut dialog = emStocksFetchPricesDialog::new("", "", "");
    {
        let mut ectx = h.engine_ctx(eid);
        let _ = dialog.Cycle(&mut ectx);
    }

    assert!(
        !dialog.fetcher_subscribed_init_for_test(),
        "fetcher must not subscribe when no FileModel was attached"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}
