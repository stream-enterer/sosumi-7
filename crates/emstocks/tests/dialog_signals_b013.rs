//! B-013 — emstocks dialog-cells finish_signal pipeline (P-004).
//!
//! Rows covered (4 total, all in `crates/emstocks/src/emStocksListBox.rs`):
//!   `emStocksListBox-189`  — CutStocksDialog finish_signal
//!   `emStocksListBox-287`  — PasteStocksDialog finish_signal
//!   `emStocksListBox-356`  — DeleteStocksDialog finish_signal
//!   `emStocksListBox-443`  — InterestDialog finish_signal
//!
//! Decisions cited: D-002 rule-1 (convert trigger side), D-006 (per-dialog
//! first-Cycle init), D-009 (typed signal pipe — finish_signal — not Cell-as-poll).
//!
//! Each test exercises the full signal pipeline:
//!   1. mutator(ask=true) creates dialog.
//!   2. first lb.Cycle subscribes parent engine to dialog.finish_signal
//!      (D-006 first-Cycle init); IsSignaled is false → busy=true.
//!   3. simulate dialog finish: write cell + fire(finish_signal).
//!   4. flush + lb.Cycle observes IsSignaled, drains cell, runs deferred
//!      mutator (ask=false) on Ok, disconnects, clears subscribed flag.
//!
//! The cancel-old-dialog disconnect is verified by a separate test that
//! creates two dialogs in sequence and confirms the new dialog has a fresh
//! finish_signal and the subscribed flag was reset.
//!
//! RUST_ONLY: (dependency-forced) no C++ test analogue — mirrors the
//! `polling_b017` shape.

use emStocks::emStocksConfig::emStocksConfig;
use emStocks::emStocksListBox::emStocksListBox;
use emStocks::emStocksRec::{emStocksRec, Interest, StockRec};

use emcore::emDialog::DialogResult;
use emcore::emEngine::Priority;
use emcore::emLook::emLook;
use emcore::emPanelScope::PanelScope;
use emcore::test_view_harness::TestViewHarness;

struct NoopEngine;
impl emcore::emEngine::emEngine for NoopEngine {
    fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
        false
    }
}

fn make_stock(id: &str, name: &str, interest: Interest) -> StockRec {
    let mut s = StockRec::default();
    s.id = id.to_string();
    s.name = name.to_string();
    s.interest = interest;
    s
}

fn populated_rec_and_lb(
    _h: &mut TestViewHarness,
) -> (emStocksRec, emStocksListBox, emStocksConfig) {
    let mut rec = emStocksRec::default();
    rec.stocks.push(make_stock("1", "Alpha", Interest::High));
    rec.stocks.push(make_stock("2", "Beta", Interest::High));

    let config = emStocksConfig::default();
    let mut lb = emStocksListBox::new();
    // Set look directly without attach_list_box: the latter wires an emListBox
    // whose items would need separate population. The fallback `selected_indices`
    // path is sufficient to drive selection through the mutators here.
    lb.set_look_for_test(emLook::new());
    lb.UpdateItems(&rec, &config);
    lb.Select(0);
    (rec, lb, config)
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-356 — DeleteStocksDialog finish_signal pipeline
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn delete_dialog_finish_signal_drives_cycle() {
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let (mut rec, mut lb, config) = populated_rec_and_lb(&mut h);

    {
        let mut ectx = h.engine_ctx(eid);
        lb.DeleteStocks(&mut ectx, &mut rec, true);
    }
    let sig = lb
        .delete_stocks_dialog_for_test()
        .expect("delete dialog should be Some after ask=true")
        .finish_signal;

    {
        let mut ectx = h.engine_ctx(eid);
        let busy = lb.Cycle(&mut ectx, &mut rec, &config);
        assert!(busy, "first Cycle with pending dialog must report busy");
    }
    assert!(
        lb.delete_subscribed_for_test(),
        "first Cycle must set subscribed=true"
    );

    lb.delete_stocks_result_for_test()
        .set(Some(DialogResult::Ok));
    h.scheduler.fire(sig);
    h.scheduler.flush_signals_for_test();

    let initial_len = rec.stocks.len();
    {
        let mut ectx = h.engine_ctx(eid);
        let busy = lb.Cycle(&mut ectx, &mut rec, &config);
        assert!(!busy, "Cycle after finish must not report busy");
    }
    assert!(
        lb.delete_stocks_dialog_for_test().is_none(),
        "dialog must be cleared"
    );
    assert!(
        !lb.delete_subscribed_for_test(),
        "subscribed must reset to false"
    );
    assert_eq!(
        rec.stocks.len(),
        initial_len - 1,
        "Ok finish must run DeleteStocks(ask=false)"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-189 — CutStocksDialog finish_signal pipeline
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn cut_dialog_finish_signal_drives_cycle() {
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let (mut rec, mut lb, config) = populated_rec_and_lb(&mut h);

    {
        let mut ectx = h.engine_ctx(eid);
        lb.CutStocks(&mut ectx, &mut rec, true);
    }
    let sig = lb.cut_stocks_dialog_for_test().unwrap().finish_signal;

    {
        let mut ectx = h.engine_ctx(eid);
        let busy = lb.Cycle(&mut ectx, &mut rec, &config);
        assert!(busy);
    }
    assert!(lb.cut_subscribed_for_test());

    // Cancel branch: write Cancel result + fire signal. Cycle must drop the
    // dialog without performing the cut.
    lb.cut_stocks_result_for_test()
        .set(Some(DialogResult::Cancel));
    h.scheduler.fire(sig);
    h.scheduler.flush_signals_for_test();

    let initial_len = rec.stocks.len();
    {
        let mut ectx = h.engine_ctx(eid);
        let busy = lb.Cycle(&mut ectx, &mut rec, &config);
        assert!(!busy);
    }
    assert!(lb.cut_stocks_dialog_for_test().is_none());
    assert!(!lb.cut_subscribed_for_test());
    assert_eq!(rec.stocks.len(), initial_len, "Cancel must not delete");

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-287 — PasteStocksDialog finish_signal pipeline
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn paste_dialog_finish_signal_drives_cycle() {
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let (mut rec, mut lb, config) = populated_rec_and_lb(&mut h);

    let result = {
        let mut ectx = h.engine_ctx(eid);
        lb.PasteStocks(&mut ectx, &mut rec, &config, true)
    };
    assert!(result.is_ok(), "ask=true paste returns Ok(empty)");
    let sig = lb.paste_stocks_dialog_for_test().unwrap().finish_signal;

    {
        let mut ectx = h.engine_ctx(eid);
        let busy = lb.Cycle(&mut ectx, &mut rec, &config);
        assert!(busy);
    }
    assert!(lb.paste_subscribed_for_test());

    // Cancel path is sufficient to verify the pipeline (Ok would require a
    // real clipboard; not portable in CI).
    lb.paste_stocks_result_for_test()
        .set(Some(DialogResult::Cancel));
    h.scheduler.fire(sig);
    h.scheduler.flush_signals_for_test();

    {
        let mut ectx = h.engine_ctx(eid);
        let _ = lb.Cycle(&mut ectx, &mut rec, &config);
    }
    assert!(lb.paste_stocks_dialog_for_test().is_none());
    assert!(!lb.paste_subscribed_for_test());

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-443 — InterestDialog finish_signal pipeline
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn interest_dialog_finish_signal_drives_cycle_ok() {
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let (mut rec, mut lb, config) = populated_rec_and_lb(&mut h);
    rec.stocks.clear();
    rec.stocks.push(make_stock("1", "Alpha", Interest::Medium));
    rec.stocks.push(make_stock("2", "Beta", Interest::Medium));
    lb.UpdateItems(&rec, &config);
    lb.Select(0);

    {
        let mut ectx = h.engine_ctx(eid);
        lb.SetInterest(&mut ectx, &mut rec, Interest::High, true);
    }
    let sig = lb.interest_dialog_for_test().unwrap().finish_signal;
    assert_eq!(lb.interest_to_set_for_test(), Some(Interest::High));

    {
        let mut ectx = h.engine_ctx(eid);
        let busy = lb.Cycle(&mut ectx, &mut rec, &config);
        assert!(busy);
    }
    assert!(lb.interest_subscribed_for_test());

    lb.interest_result_for_test().set(Some(DialogResult::Ok));
    h.scheduler.fire(sig);
    h.scheduler.flush_signals_for_test();

    {
        let mut ectx = h.engine_ctx(eid);
        let _ = lb.Cycle(&mut ectx, &mut rec, &config);
    }
    assert!(lb.interest_dialog_for_test().is_none());
    assert!(!lb.interest_subscribed_for_test());
    assert!(
        lb.interest_to_set_for_test().is_none(),
        "interest_to_set must be taken on Ok"
    );
    assert_eq!(
        rec.stocks[lb.visible_items[0]].interest,
        Interest::High,
        "Ok finish must run SetInterest(ask=false)"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

#[test]
fn interest_dialog_cancel_resets_interest_to_set() {
    // §3.3a: Interest-block cancel-side cleanup — `interest_to_set = None;`
    // must be reset on non-Ok finish (preserves existing semantics).
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let (mut rec, mut lb, config) = populated_rec_and_lb(&mut h);
    rec.stocks.clear();
    rec.stocks.push(make_stock("1", "Alpha", Interest::Medium));
    lb.UpdateItems(&rec, &config);
    lb.Select(0);

    {
        let mut ectx = h.engine_ctx(eid);
        lb.SetInterest(&mut ectx, &mut rec, Interest::High, true);
    }
    let sig = lb.interest_dialog_for_test().unwrap().finish_signal;
    assert_eq!(lb.interest_to_set_for_test(), Some(Interest::High));

    {
        let mut ectx = h.engine_ctx(eid);
        lb.Cycle(&mut ectx, &mut rec, &config);
    }

    lb.interest_result_for_test()
        .set(Some(DialogResult::Cancel));
    h.scheduler.fire(sig);
    h.scheduler.flush_signals_for_test();

    {
        let mut ectx = h.engine_ctx(eid);
        lb.Cycle(&mut ectx, &mut rec, &config);
    }
    assert!(lb.interest_dialog_for_test().is_none());
    assert!(
        lb.interest_to_set_for_test().is_none(),
        "Cancel must reset interest_to_set to None"
    );
    assert_eq!(
        rec.stocks[lb.visible_items[0]].interest,
        Interest::Medium,
        "Cancel must not change interest"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// Cancel-old-dialog disconnect (Adversarial Review I-2)
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn cancel_old_dialog_disconnects_old_finish_signal() {
    // When a second mutator(ask=true) call replaces an in-flight dialog, the
    // old dialog's `finish_signal → engine` connection must be removed before
    // the new dialog is installed. We assert observably: after the cancel-old
    // branch runs, the new dialog has a fresh finish_signal and subscribed
    // is reset (so the next Cycle re-subscribes for the new sig).
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let (mut rec, mut lb, config) = populated_rec_and_lb(&mut h);

    {
        let mut ectx = h.engine_ctx(eid);
        lb.DeleteStocks(&mut ectx, &mut rec, true);
    }
    let old_sig = lb.delete_stocks_dialog_for_test().unwrap().finish_signal;
    {
        let mut ectx = h.engine_ctx(eid);
        lb.Cycle(&mut ectx, &mut rec, &config);
    }
    assert!(lb.delete_subscribed_for_test());

    // Create dialog #2: cancel-old branch must disconnect old_sig and reset
    // the subscribed flag.
    {
        let mut ectx = h.engine_ctx(eid);
        lb.DeleteStocks(&mut ectx, &mut rec, true);
    }
    let new_sig = lb.delete_stocks_dialog_for_test().unwrap().finish_signal;
    assert_ne!(
        old_sig, new_sig,
        "new dialog must have a fresh finish_signal"
    );
    assert!(
        !lb.delete_subscribed_for_test(),
        "cancel-old branch must reset subscribed=false (re-subscribe on next Cycle)"
    );

    h.scheduler.remove_engine(eid);
    h.scheduler.flush_signals_for_test();
}
