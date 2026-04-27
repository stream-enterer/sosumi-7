/// B-006-typed-subscribe-mainctrl behavioral tests.
///
/// Covers the two wired subscription rows (218 and 219) from the
/// B-006 design doc. The full click-through tests live in
/// `emMainControlPanel::tests` (internal `mod tests`) because they require
/// access to private types and the emMainWindow thread-local. This file
/// covers the signal-allocation / stable-accessor properties that are
/// observable from outside the crate, following the B-003 test pattern.
///
/// Row 217 (ContentView.GetControlPanelSignal) is handled by
/// ControlPanelBridge and is verified by existing emMainWindow tests.
///
/// RUST_ONLY: (dependency-forced) no C++ test analogue — C++ emMainControlPanel
/// is an in-process class with no separate test harness. Mirrors B-003's
/// `typed_subscribe_b003.rs` rationale.
use std::rc::Rc;

use emcore::emScheduler::EngineScheduler;
use slotmap::Key as _;

/// Row 218 signal accessor: `GetWindowFlagsSignal` on `emWindow` returns a
/// stable, non-null `SignalId` after window construction.
///
/// Mirrors C++ row 218: `AddWakeUpSignal(MainWin.GetWindowFlagsSignal())`.
#[test]
fn row_218_window_flags_signal_stable() {
    let ctx = emcore::emContext::emContext::NewRoot();
    let mut sched = EngineScheduler::new();
    let close_sig = sched.create_signal();
    let flags_sig = sched.create_signal();
    let focus_sig = sched.create_signal();
    let geom_sig = sched.create_signal();

    let win = emcore::emWindow::emWindow::new_popup_pending(
        Rc::clone(&ctx),
        emcore::emWindow::WindowFlags::empty(),
        "b006_218".to_string(),
        close_sig,
        flags_sig,
        focus_sig,
        geom_sig,
        emcore::emColor::emColor::TRANSPARENT,
    );

    // GetWindowFlagsSignal returns the same SignalId passed at construction.
    let retrieved_sig = win.GetWindowFlagsSignal();
    assert_eq!(
        retrieved_sig, flags_sig,
        "GetWindowFlagsSignal must return the flags_signal passed at construction"
    );
    assert!(
        !retrieved_sig.is_null(),
        "GetWindowFlagsSignal must not return null"
    );

    // Firing the signal is observable via is_pending.
    sched.fire(flags_sig);
    assert!(
        sched.is_pending(flags_sig),
        "flags_signal must be pending after fire"
    );

    sched.clear_pending_for_tests();
    sched.remove_signal(close_sig);
    sched.remove_signal(flags_sig);
    sched.remove_signal(focus_sig);
    sched.remove_signal(geom_sig);
}

/// Row 219 signal accessor: `emMainConfig::GetChangeSignal` is addressable
/// and initially `SignalId::null()` (no scheduler at registration time).
/// After a real scheduler is wired (which happens in production via the
/// engine-registration path), the signal fires and is stable.
///
/// Mirrors C++ row 219: `AddWakeUpSignal(MainConfig->GetChangeSignal())`.
#[test]
fn row_219_config_change_signal_addressable() {
    let ctx = emcore::emContext::emContext::NewRoot();

    // Acquire the config singleton.
    let cfg = emMain::emMainConfig::emMainConfig::Acquire(&ctx);

    // GetChangeSignal is callable; initially null because no scheduler is
    // available at Acquire time (production engines wire this at startup).
    let sig = cfg.borrow().GetChangeSignal();
    // The signal is either null (test context) or a valid SignalId.
    // We only assert that GetChangeSignal doesn't panic and is stable.
    let sig2 = cfg.borrow().GetChangeSignal();
    assert_eq!(sig, sig2, "GetChangeSignal must be stable across calls");

    // The config fields are readable.
    assert!(
        !cfg.borrow().GetAutoHideControlView(),
        "AutoHideControlView defaults to false"
    );
    assert!(
        !cfg.borrow().GetAutoHideSlider(),
        "AutoHideSlider defaults to false"
    );
}
