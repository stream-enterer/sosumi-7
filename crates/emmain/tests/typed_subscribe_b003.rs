use emMain::emAutoplay::emAutoplayViewModel;
/// B-003-no-wire-autoplay behavioral tests.
///
/// Covers the 3 rows per the design doc §Verification pattern:
///
/// Row 1 (G1): ChangeSignal accessor — mutating ViewModel via setter wakes subscribers.
/// Row 2 (G2): ProgressSignal accessor — SetItemProgress fires ProgressSignal.
/// Row 3 (wire): signal wiring end-to-end — ChangeSignal fired by every C++ emit site.
///
/// Tests use `EngineScheduler::is_pending` to verify fire delivery, matching the
/// D-006-subscribe-shape pattern without needing a full panel tree.
use emcore::emEngineCtx::SignalCtx;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;

/// Minimal SignalCtx adapter wrapping `EngineScheduler`.
struct TestSignalCtx<'a> {
    sched: &'a mut EngineScheduler,
}

impl<'a> TestSignalCtx<'a> {
    fn new(sched: &'a mut EngineScheduler) -> Self {
        Self { sched }
    }
}

impl SignalCtx for TestSignalCtx<'_> {
    fn create_signal(&mut self) -> SignalId {
        self.sched.create_signal()
    }
    fn fire(&mut self, id: SignalId) {
        self.sched.fire(id);
    }
}

/// Row 1 (G1): ChangeSignal accessor returns stable SignalId; mutating ViewModel
/// via setter fires ChangeSignal (observable as `is_pending`).
///
/// Mirrors C++ `emAutoplayControlPanel::Cycle` subscribe:
///   `AddWakeUpSignal(Model->GetChangeSignal())`
#[test]
fn change_signal_wake_on_mutator() {
    let mut sched = EngineScheduler::new();
    let mut vm = emAutoplayViewModel::new();

    // G1: allocate ChangeSignal via GetChangeSignal.
    let sig = {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.GetChangeSignal(&mut sc)
    };

    use slotmap::Key as _;
    assert!(
        !sig.is_null(),
        "GetChangeSignal returned null — not allocated"
    );

    // Verify same ID returned on second call (stable accessor per D-008 A1).
    let sig2 = {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.GetChangeSignal(&mut sc)
    };
    assert_eq!(
        sig, sig2,
        "GetChangeSignal must return stable SignalId across calls"
    );

    // Mutate ViewModel via SetDurationMS — must fire ChangeSignal.
    assert!(
        !sched.is_pending(sig),
        "signal should not be pending before mutation"
    );
    {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.SetDurationMS(&mut sc, 10000);
    }
    assert!(
        sched.is_pending(sig),
        "ChangeSignal not pending after SetDurationMS"
    );

    sched.clear_pending_for_tests();
}

/// Row 2 (G2): ProgressSignal accessor returns stable SignalId; SetItemProgress fires it.
///
/// Mirrors C++ `AddWakeUpSignal(Model->GetProgressSignal())`.
#[test]
fn progress_signal_wake_on_set_item_progress() {
    let mut sched = EngineScheduler::new();
    let mut vm = emAutoplayViewModel::new();

    // Allocate ProgressSignal.
    let prog_sig = {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.GetProgressSignal(&mut sc)
    };

    use slotmap::Key as _;
    assert!(!prog_sig.is_null(), "GetProgressSignal returned null");

    // Stable accessor.
    let prog_sig2 = {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.GetProgressSignal(&mut sc)
    };
    assert_eq!(
        prog_sig, prog_sig2,
        "GetProgressSignal must return stable SignalId"
    );

    // SetItemProgress fires ProgressSignal.
    assert!(
        !sched.is_pending(prog_sig),
        "progress signal should not be pending before mutation"
    );
    {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.SetItemProgress(&mut sc, 0.5);
    }
    assert_eq!(
        vm.GetItemProgress(),
        0.5,
        "SetItemProgress must update ItemProgress"
    );
    assert!(
        sched.is_pending(prog_sig),
        "ProgressSignal must be pending after SetItemProgress"
    );

    sched.clear_pending_for_tests();
}

/// Row 3 (wire): ChangeSignal is fired by every C++ emit site's Rust counterpart.
/// Exercises SetDurationMS, SetRecursive, SetLoop, SetAutoplaying, ContinueLastAutoplay.
/// Verifies that the signal ID returned by GetChangeSignal is the one fired.
#[test]
fn change_signal_fired_by_all_mutators() {
    let mut sched = EngineScheduler::new();
    let mut vm = emAutoplayViewModel::new();

    let sig = {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.GetChangeSignal(&mut sc)
    };

    macro_rules! assert_fires {
        ($expr:expr, $msg:expr) => {{
            $expr;
            assert!(sched.is_pending(sig), $msg);
            sched.clear_pending_for_tests();
        }};
    }

    // SetDurationMS (emAutoplay.cpp:661)
    assert_fires!(
        {
            let mut sc = TestSignalCtx::new(&mut sched);
            vm.SetDurationMS(&mut sc, 3000);
        },
        "SetDurationMS must fire ChangeSignal"
    );

    // SetRecursive (emAutoplay.cpp:675)
    assert_fires!(
        {
            let mut sc = TestSignalCtx::new(&mut sched);
            vm.SetRecursive(&mut sc, true);
        },
        "SetRecursive must fire ChangeSignal"
    );

    // SetLoop (emAutoplay.cpp:688)
    assert_fires!(
        {
            let mut sc = TestSignalCtx::new(&mut sched);
            vm.SetLoop(&mut sc, true);
        },
        "SetLoop must fire ChangeSignal"
    );

    // SetAutoplaying (emAutoplay.cpp:701)
    assert_fires!(
        {
            let mut sc = TestSignalCtx::new(&mut sched);
            vm.SetAutoplaying(&mut sc, true);
        },
        "SetAutoplaying must fire ChangeSignal"
    );

    // ContinueLastAutoplay (emAutoplay.cpp:710).
    // Set up state via public methods without triggering change signal first.
    // Need Autoplaying=false and LastLocationValid=true.
    // First turn off autoplay without the sig being set up (it already is).
    // Then set location info via SetLastLocationValid/SetLastLocation.
    {
        let mut sc = TestSignalCtx::new(&mut sched);
        vm.SetAutoplaying(&mut sc, false); // back to false so ContinueLast can fire
    }
    sched.clear_pending_for_tests(); // clear the SetAutoplaying(false) signal

    vm.SetLastLocationValid(true);
    vm.SetLastLocation("test/panel");
    // Now ContinueLastAutoplay should fire ChangeSignal.
    assert_fires!(
        {
            let mut sc = TestSignalCtx::new(&mut sched);
            vm.ContinueLastAutoplay(&mut sc);
        },
        "ContinueLastAutoplay must fire ChangeSignal"
    );
}
