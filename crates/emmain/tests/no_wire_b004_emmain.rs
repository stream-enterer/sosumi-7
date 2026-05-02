/// B-004-no-wire-misc emmain-slice behavioral tests.
///
/// Covers row `emBookmarks-1479` (the only emmain row remaining after the
/// emcore slice merged at 9b8ee012 and B-014's merge of
/// `emVirtualCosmosModel-accessor-model-change` at c2871547).
///
/// `emBookmarks-1479` mirrors C++ `emBookmarks.cpp:1479`:
///   `AddWakeUpSignal(GetClickSignal())`
/// and the click reaction at `emBookmarks.cpp:1523-1535`.
///
/// The accessor tested here (`emBookmarkButton::GetClickSignal`) follows
/// D-008-signal-allocation-shape (A1 combined-form): lazy allocation on
/// first call. The Cycle reaction follows D-006-subscribe-shape (first-
/// Cycle init + IsSignaled). Click *detection* (button input handling) is
/// deferred to B-013-rc-shim-emcore per the B-004 design's recommended
/// option (a); these tests use the `*_for_test` hooks as the trigger
/// surface that B-013 will replace with a real input handler.
///
/// RUST_ONLY: (dependency-forced) — no C++ test analogue. Mirrors
/// `typed_subscribe_b003.rs` / `typed_subscribe_b008.rs` rationale.
use emMain::emBookmarks::{emBookmarkButton, emBookmarkRec};
use emcore::emEngineCtx::SignalCtx;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;
use slotmap::Key as _;

/// Minimal SignalCtx adapter wrapping `EngineScheduler`.
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

fn make_bookmark(identity: &str) -> emBookmarkRec {
    let mut bm = emBookmarkRec {
        LocationIdentity: identity.to_string(),
        ..emBookmarkRec::default()
    };
    bm.entry.Name = "Test".to_string();
    bm
}

/// D-008 A1: `GetClickSignal` lazily allocates a non-null SignalId on first
/// call and returns the same ID on subsequent calls.
#[test]
fn click_signal_lazy_alloc_is_stable() {
    let mut sched = EngineScheduler::new();
    let btn = emBookmarkButton::new(make_bookmark("::test:"));

    let sig_a = {
        let mut sc = TestSignalCtx { sched: &mut sched };
        btn.GetClickSignal(&mut sc)
    };
    assert!(
        !sig_a.is_null(),
        "GetClickSignal should return a non-null SignalId after lazy alloc"
    );

    let sig_b = {
        let mut sc = TestSignalCtx { sched: &mut sched };
        btn.GetClickSignal(&mut sc)
    };
    assert_eq!(
        sig_a, sig_b,
        "GetClickSignal must return a stable SignalId across calls (D-008 A1)"
    );

    sched.remove_signal(sig_a);
}

/// D-007 deferred-fire shape: setting `pending_click_fire` (the input-
/// handler stand-in until B-013 lands) makes the click signal observable
/// as `is_pending` once `Cycle`'s drain-half runs.
///
/// This is the observable contract the consumer-side reaction at C++
/// `emBookmarks.cpp:1523` rides on: `IsSignaled(GetClickSignal())`.
#[test]
fn pending_click_fire_drains_to_observable_signal() {
    let mut sched = EngineScheduler::new();
    let btn = emBookmarkButton::new(make_bookmark("::test:"));

    // First-Cycle accessor allocation (mirrors what Cycle's init block
    // will do on the first tick).
    let sig = {
        let mut sc = TestSignalCtx { sched: &mut sched };
        btn.GetClickSignal(&mut sc)
    };
    assert!(
        !sched.is_pending(sig),
        "click signal must not be pending before any click trigger"
    );

    // Simulate the input handler that B-013 will wire.
    btn.set_pending_click_fire_for_test();

    // Mirror Cycle's drain-half (first thing Cycle does).
    let drained = {
        let mut sc = TestSignalCtx { sched: &mut sched };
        btn.fire_pending_click_for_test(&mut sc)
    };
    assert!(drained, "fire_pending_click_for_test should report a drain");
    assert!(
        sched.is_pending(sig),
        "click signal must be pending after pending-click drain → fire"
    );

    sched.remove_signal(sig);
}

/// Without an input trigger, `Cycle`'s drain is a no-op and the click
/// signal stays quiescent — the C++ contract is "fire only when clicked".
#[test]
fn no_pending_click_no_signal_fire() {
    let mut sched = EngineScheduler::new();
    let btn = emBookmarkButton::new(make_bookmark("::test:"));

    let sig = {
        let mut sc = TestSignalCtx { sched: &mut sched };
        btn.GetClickSignal(&mut sc)
    };

    let drained = {
        let mut sc = TestSignalCtx { sched: &mut sched };
        btn.fire_pending_click_for_test(&mut sc)
    };
    assert!(!drained, "drain must report no-op when no click is pending");
    assert!(
        !sched.is_pending(sig),
        "click signal must remain quiescent without an input trigger"
    );

    sched.remove_signal(sig);
}
