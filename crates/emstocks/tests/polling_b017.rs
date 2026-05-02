//! B-017 — emstocks polling-no-accessor cluster (P-007).
//!
//! Rows covered (rows 2 + 3; row 1 deferred per spec coordination on B-001 G3):
//!   `emStocksFilePanel-34`  — first-Cycle subscribe to
//!     `emFilePanel::GetVirFileStateSignal()`; signal-gated
//!     `refresh_vir_file_state` + lazy `list_box` materialisation.
//!   `emStocksFileModel-41`  — model-owned `SaveTimer` signal, allocated by
//!     the owning panel in its first-Cycle init (proxy-engine pattern per
//!     spec I-3 by-value resolution); signal-gated `Save`.
//!
//! Decisions cited: D-005 (poll-replacement), D-006 (subscribe-shape),
//! D-009 (no polling intermediary — `Option<Instant>` shim removed).
//!
//! RUST_ONLY: (dependency-forced) no C++ test analogue. Mirrors the
//! `typed_subscribe_b016` shape used by the sibling fileman cluster.

use emStocks::emStocksFilePanel::emStocksFilePanel;
use emcore::emEngine::Priority;
use emcore::emPanel::PanelBehavior;
use emcore::emPanelScope::PanelScope;
use emcore::emPanelTree::PanelTree;
use emcore::test_view_harness::TestViewHarness;

struct NoopEngine;
impl emcore::emEngine::emEngine for NoopEngine {
    fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
        false
    }
}

fn empty_panel_ctx<'a>(tree: &'a mut PanelTree) -> emcore::emEngineCtx::PanelCtx<'a> {
    let id = tree.create_root("b017-stub", false);
    emcore::emEngineCtx::PanelCtx::new(tree, id, 1.0)
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

fn cycle_panel(
    h: &mut TestViewHarness,
    eid: emcore::emEngine::EngineId,
    panel: &mut dyn PanelBehavior,
) -> bool {
    let mut tree = PanelTree::new();
    let mut pctx = empty_panel_ctx(&mut tree);
    let mut ectx = h.engine_ctx(eid);
    panel.Cycle(&mut ectx, &mut pctx)
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksFilePanel-34 — VirFileStateSignal subscription
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn stocks_file_panel_allocates_vir_file_state_signal_on_first_cycle() {
    // Q1 (signal connected): first Cycle runs the D-006 init block, which
    // allocates `VirFileStateSignal` (via `ensure_vir_file_state_signal`)
    // and connects the panel's engine to it.
    let mut h = TestViewHarness::new();
    let mut panel = emStocksFilePanel::default();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    assert!(
        panel.vir_file_state_signal_for_test().is_none(),
        "vir_file_state_sig must be None before first Cycle"
    );

    let _ = cycle_panel(&mut h, eid, &mut panel);

    assert!(
        panel.vir_file_state_signal_for_test().is_some(),
        "vir_file_state_sig must be Some after first Cycle"
    );

    h.scheduler.remove_engine(eid);
    drain_all_engines(&mut h);
    h.scheduler.flush_signals_for_test();
}

// ───────────────────────────────────────────────────────────────────────────
// emStocksFileModel-41 — SaveTimer signal allocated + dirty/save round-trip
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn stocks_file_panel_allocates_save_timer_signal_on_first_cycle() {
    // Q1 (signal connected, row 3): the panel's first-Cycle init allocates
    // the model's `SaveTimer` signal + timer and connects the panel's engine
    // to it (proxy-engine pattern per spec I-3).
    let mut h = TestViewHarness::new();
    let mut panel = emStocksFilePanel::default();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    let pre = panel.save_timer_signal_for_test();
    assert_eq!(
        pre,
        emcore::emSignal::SignalId::default(),
        "model.save_timer_signal must be the null key before first Cycle"
    );

    let _ = cycle_panel(&mut h, eid, &mut panel);

    let post = panel.save_timer_signal_for_test();
    assert_ne!(
        post,
        emcore::emSignal::SignalId::default(),
        "model.save_timer_signal must be a valid id after first Cycle"
    );

    h.scheduler.remove_engine(eid);
    drain_all_engines(&mut h);
    h.scheduler.flush_signals_for_test();
}

#[test]
fn stocks_file_panel_save_timer_fires_drives_save() {
    // Q2 + Q3 (Cycle observes & fires; reaction runs documented mutator):
    // mutate the rec via GetWritableRec → after panel Cycle, dirty is set
    // and the timer is armed. Force the timer to fire (zero-delay restart),
    // run the next time slice so the scheduler dispatches the timer signal,
    // then run the panel's Cycle: the panel observes `IsSignaled(save_timer)`
    // and calls `save_on_timer_fire` which clears dirty.
    let mut h = TestViewHarness::new();
    let mut panel = emStocksFilePanel::default();
    let eid = h.scheduler.register_engine(
        Box::new(NoopEngine),
        Priority::Medium,
        PanelScope::Framework,
    );

    // First Cycle — allocates SaveTimer signal/timer and subscribes.
    let _ = cycle_panel(&mut h, eid, &mut panel);
    assert!(
        !panel.model_dirty_for_test(),
        "fresh panel must not be dirty"
    );

    // Mutate via GetWritableRec: sets dirty, latches dirty_unobserved.
    {
        let mut sc = h.sched_ctx_for(eid);
        panel.mark_rec_dirty_for_test(&mut sc);
    }
    assert!(
        panel.model_dirty_for_test(),
        "GetWritableRec must mark the model dirty"
    );

    // Second Cycle — drains dirty_unobserved latch, arms the SaveTimer.
    let _ = cycle_panel(&mut h, eid, &mut panel);

    // Force the SaveTimer to fire on the next slice by directly firing its
    // signal through the scheduler. (Equivalent to advancing time past
    // 15s — the goal is to verify the IsSignaled gate, not the timer
    // semantics.)
    let save_sig = panel.save_timer_signal_for_test();
    h.scheduler.fire(save_sig);
    // Process pending signals so the IsSignaled clock-comparison observes
    // the fire on the next Cycle.
    h.scheduler.flush_signals_for_test();

    // Third Cycle — observes IsSignaled(save_sig), calls save_on_timer_fire.
    let _ = cycle_panel(&mut h, eid, &mut panel);

    assert!(
        !panel.model_dirty_for_test(),
        "save_on_timer_fire must clear dirty on signal-driven Save"
    );

    h.scheduler.remove_engine(eid);
    drain_all_engines(&mut h);
    h.scheduler.flush_signals_for_test();
}
