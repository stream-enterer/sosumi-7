//! B-001 — emstocks signal-wiring (P-001), partial coverage.
//!
//! Rows covered (4 of 71 — Phase 4 partial: Tasks 4.4 + 4.5):
//!   `emStocksFilePanel-255` — first-Cycle subscribe to
//!     `emStocksListBox::GetSelectedDateSignal()` after the ListBox is
//!     materialised on VFS-Loaded (deferred-attach init per the design's
//!     §Sequencing two-tier note).
//!   `emStocksListBox-51`    — first-Cycle init of FileModel.ChangeSignal
//!     (allocated & connected from the parent panel — see
//!     `emStocksListBox::wire_change_signals` doc); reaction = UpdateItems.
//!   `emStocksListBox-52`    — first-Cycle init of Config.ChangeSignal;
//!     reaction = UpdateItems.
//!   `emStocksListBox-53`    — self-subscribe to inherited
//!     `emListBox::GetItemTriggerSignal()`; reaction = open first WebPage when
//!     `Config.TriggeringOpensWebPage` is set.
//!
//! Decisions cited: D-006 (first-Cycle subscribe-shape), D-007 (synchronous
//! mutator-fire), D-008 A1 (lazy-allocated SignalId combined-form accessor),
//! D-009 (no polling intermediary).
//!
//! RUST_ONLY: (dependency-forced) no C++ test analogue. Mirrors the
//! `polling_b017` / `dialog_signals_b013` shapes used by the sibling clusters.
//!
//! Other Phase 4 sub-tasks (ControlPanel-37, ItemPanel-29, ItemChart-2) are
//! deferred — those panels lack a production instantiation path / Cycle in
//! the current Rust codebase, so wiring D-006 there would be cargo-cult.

use slotmap::Key as _;

use emStocks::emStocksListBox::emStocksListBox;
use emStocks::emStocksRec::{emStocksRec, Interest, StockRec};

use emcore::emEngine::Priority;
use emcore::emPanelScope::PanelScope;
use emcore::test_view_harness::TestViewHarness;

struct NoopEngine;
impl emcore::emEngine::emEngine for NoopEngine {
    fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
        false
    }
}

fn drain_all_engines(h: &mut TestViewHarness) {
    let mut eids: Vec<emcore::emEngine::EngineId> =
        h.scheduler.engines_for_scope(PanelScope::Framework);
    for wid in h.windows.keys().copied().collect::<Vec<_>>() {
        eids.extend(h.scheduler.engines_for_scope(PanelScope::Toplevel(wid)));
    }
    for eid in eids {
        h.scheduler.remove_engine(eid);
    }
}

fn make_stock(id: &str, name: &str, interest: Interest) -> StockRec {
    let mut s = StockRec::default();
    s.id = id.to_string();
    s.name = name.to_string();
    s.interest = interest;
    s
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksFilePanel-255 — coverage note.
//
// The deferred-attach contract for this row (allocate + connect
// `ListBox::SelectedDateSignal` once VFS-Loaded materialises the inner
// list_box) is verified by an in-crate `#[cfg(test)]` test inside
// `emStocksFilePanel.rs` (see
// `emStocksFilePanel::tests::cycle_wires_selected_date_subscribe_after_vfs_good`).
// Driving the contract from this integration crate would require the
// `pub(crate)`-scoped `set_vfs_good_for_test` helper, which is intentionally
// kept crate-private (it pokes inner emFilePanel state directly). The
// in-crate test exercises the same Cycle code path through the same
// public `Cycle` entry point.
// ───────────────────────────────────────────────────────────────────────────

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-51 / -52 — FileModel + Config ChangeSignal pipeline
// ───────────────────────────────────────────────────────────────────────────

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-51 — direct lb.Cycle round-trip: FileModel.ChangeSignal
// fires → UpdateItems runs (visible_items reflects the rec).
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn listbox_reacts_to_file_model_change_signal_with_update_items() {
    use emStocks::emStocksConfig::emStocksConfig;
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let mut rec = emStocksRec::default();
    rec.stocks.push(make_stock("1", "Alpha", Interest::High));

    let config = emStocksConfig::default();
    let mut lb = emStocksListBox::new();

    // Allocate a synthetic "FileModel ChangeSignal" id and wire it.
    let model_sig = {
        let mut sc = h.sched_ctx_for(eid);
        sc.create_signal()
    };
    let cfg_sig = {
        let mut sc = h.sched_ctx_for(eid);
        sc.create_signal()
    };
    // Connect the engine before firing so IsSignaled observes it.
    {
        let mut ectx = h.engine_ctx(eid);
        ectx.connect(model_sig, eid);
        ectx.connect(cfg_sig, eid);
    }
    lb.wire_change_signals(model_sig, cfg_sig);

    // Pre-condition: visible_items empty (no UpdateItems run yet).
    assert!(lb.visible_items.is_empty());

    // Fire FileModel.ChangeSignal.
    h.scheduler.fire(model_sig);
    h.scheduler.flush_signals_for_test();

    {
        let mut ectx = h.engine_ctx(eid);
        let _busy = lb.Cycle(&mut ectx, &mut rec, &config);
    }

    assert_eq!(
        lb.visible_items.len(),
        1,
        "Cycle's IsSignaled(FileModel.ChangeSignal) branch must call UpdateItems"
    );

    h.scheduler.remove_engine(eid);
    drain_all_engines(&mut h);
    h.scheduler.flush_signals_for_test();
}

#[test]
fn listbox_reacts_to_config_change_signal_with_update_items() {
    use emStocks::emStocksConfig::emStocksConfig;
    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let mut rec = emStocksRec::default();
    rec.stocks.push(make_stock("1", "Alpha", Interest::High));
    rec.stocks.push(make_stock("2", "Beta", Interest::Medium));

    let config = emStocksConfig::default();
    let mut lb = emStocksListBox::new();

    let model_sig = {
        let mut sc = h.sched_ctx_for(eid);
        sc.create_signal()
    };
    let cfg_sig = {
        let mut sc = h.sched_ctx_for(eid);
        sc.create_signal()
    };
    {
        let mut ectx = h.engine_ctx(eid);
        ectx.connect(model_sig, eid);
        ectx.connect(cfg_sig, eid);
    }
    lb.wire_change_signals(model_sig, cfg_sig);

    assert!(lb.visible_items.is_empty());

    h.scheduler.fire(cfg_sig);
    h.scheduler.flush_signals_for_test();

    {
        let mut ectx = h.engine_ctx(eid);
        let _busy = lb.Cycle(&mut ectx, &mut rec, &config);
    }

    assert_eq!(
        lb.visible_items.len(),
        2,
        "Cycle's IsSignaled(Config.ChangeSignal) branch must call UpdateItems"
    );

    h.scheduler.remove_engine(eid);
    drain_all_engines(&mut h);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksListBox-53 — ItemTriggerSignal connect after attach
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn listbox_connects_item_trigger_signal_after_attach() {
    use emStocks::emStocksConfig::emStocksConfig;
    use emcore::emLook::emLook;

    let mut h = TestViewHarness::new();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let mut rec = emStocksRec::default();
    let config = emStocksConfig::default();
    let mut lb = emStocksListBox::new();

    // Pre-attach: GetItemTriggerSignal is None; no cached id.
    assert!(lb.GetItemTriggerSignal().is_none());
    assert!(lb.item_trigger_signal_for_test().is_none());

    // Attach the inner emListBox. Drives `attach_list_box`, which sets
    // `lb.list_box = Some(emListBox::new(...))`.
    {
        let mut sc = h.sched_ctx_for(eid);
        lb.attach_list_box(&mut sc, emLook::new());
    }

    // Wire model/config signals so the gate-of-three doesn't trip on `None`s.
    let model_sig = {
        let mut sc = h.sched_ctx_for(eid);
        sc.create_signal()
    };
    let cfg_sig = {
        let mut sc = h.sched_ctx_for(eid);
        sc.create_signal()
    };
    {
        let mut ectx = h.engine_ctx(eid);
        ectx.connect(model_sig, eid);
        ectx.connect(cfg_sig, eid);
    }
    lb.wire_change_signals(model_sig, cfg_sig);

    // Drive Cycle once: the attach-deferred init connects the
    // ItemTriggerSignal.
    {
        let mut ectx = h.engine_ctx(eid);
        let _ = lb.Cycle(&mut ectx, &mut rec, &config);
    }

    let cached = lb.item_trigger_signal_for_test();
    assert!(
        cached.is_some(),
        "Cycle must connect+cache GetItemTriggerSignal once the inner emListBox is attached"
    );
    assert_eq!(
        cached,
        lb.GetItemTriggerSignal(),
        "cached id must match the inner emListBox's item_trigger_signal"
    );

    h.scheduler.remove_engine(eid);
    drain_all_engines(&mut h);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// Sanity: `wire_change_signals` is idempotent w.r.t. subscribed_init.
// ───────────────────────────────────────────────────────────────────────────

// ───────────────────────────────────────────────────────────────────────────
// B-001-followup Phase B — emStocksControlPanel D-006 wiring (37 rows).
//
// The representative-coverage tests for ControlPanel's first-Cycle subscribe
// + signal-driven reactions live as in-crate `#[cfg(test)]` tests inside
// `emStocksControlPanel.rs` because they exercise `pub(crate)` fields
// (`subscribed_init`, `model_change_sig`, `selection_subscribed`,
// `subscribed_widgets`, `widgets`) intentionally kept crate-private. See
// `emStocksControlPanel::tests::control_panel_first_cycle_wires_g1_g2_g4_signals`
// and siblings.
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn wire_change_signals_flips_subscribed_init_true() {
    let mut lb = emStocksListBox::new();
    assert!(!lb.subscribed_init_for_test());

    // Use null SignalIds — the API only stores them; the ectx.connect is
    // performed by the caller.
    let s1: emcore::emSignal::SignalId = emcore::emSignal::SignalId::null();
    let s2: emcore::emSignal::SignalId = emcore::emSignal::SignalId::null();
    lb.wire_change_signals(s1, s2);

    assert!(lb.subscribed_init_for_test());
}
