/// Self-contained scheduler parity tests.
/// These verify Rust scheduler behavior directly (no golden files).
use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::scheduler::{Engine, EngineCtx, EngineScheduler, Priority, SignalId};

// ─── Helper: engine that records cycle calls ────────────────────

struct RecordingEngine {
    label: &'static str,
    log: Rc<RefCell<Vec<&'static str>>>,
    stay_awake: bool,
}

impl Engine for RecordingEngine {
    fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
        self.log.borrow_mut().push(self.label);
        self.stay_awake
    }
}

// ─── Test 1: signal_fire_check ──────────────────────────────────

#[test]
fn signal_fire_check() {
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();

    // Initially not pending
    assert!(!sched.is_pending(sig));

    // Fire → pending
    sched.fire(sig);
    assert!(sched.is_pending(sig));

    // Time slice consumes pending signals
    sched.do_time_slice();
    assert!(!sched.is_pending(sig));

    sched.remove_signal(sig);
}

// ─── Test 2: signal_multi ───────────────────────────────────────

#[test]
fn signal_multi() {
    let mut sched = EngineScheduler::new();
    let sig0 = sched.create_signal();
    let sig1 = sched.create_signal();
    let sig2 = sched.create_signal();

    sched.fire(sig0);
    sched.fire(sig2);

    assert!(sched.is_pending(sig0));
    assert!(!sched.is_pending(sig1));
    assert!(sched.is_pending(sig2));

    sched.do_time_slice();
    assert!(!sched.is_pending(sig0));
    assert!(!sched.is_pending(sig1));
    assert!(!sched.is_pending(sig2));

    sched.remove_signal(sig0);
    sched.remove_signal(sig1);
    sched.remove_signal(sig2);
}

// ─── Test 3: signal_abort ───────────────────────────────────────

#[test]
fn signal_abort() {
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let sig = sched.create_signal();

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "target",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng);

    // Fire then abort before time slice
    sched.fire(sig);
    assert!(sched.is_pending(sig));
    sched.abort(sig);
    assert!(!sched.is_pending(sig));

    // Engine should NOT be cycled
    sched.do_time_slice();
    assert!(log.borrow().is_empty());

    sched.remove_engine(eng);
    sched.remove_signal(sig);
}

// ─── Test 4: timer_oneshot ──────────────────────────────────────

#[test]
fn timer_oneshot() {
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();
    let log = Rc::new(RefCell::new(Vec::new()));

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "timer_recv",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng);

    let timer = sched.create_timer(sig);
    // 0ms one-shot fires on the next time slice
    sched.start_timer(timer, 0, false);
    assert!(sched.is_timer_running(timer));

    sched.do_time_slice();
    assert_eq!(log.borrow().len(), 1, "One-shot should fire exactly once");

    // After firing, one-shot stops running
    log.borrow_mut().clear();
    sched.do_time_slice();
    assert!(log.borrow().is_empty(), "One-shot should not repeat");

    sched.remove_engine(eng);
    sched.remove_timer(timer);
    sched.remove_signal(sig);
}

// ─── Test 5: timer_periodic ─────────────────────────────────────

#[test]
fn timer_periodic() {
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();
    let log = Rc::new(RefCell::new(Vec::new()));

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "periodic",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng);

    let timer = sched.create_timer(sig);
    sched.start_timer(timer, 0, true); // 0ms periodic; refire clamped to 1ms

    // First slice fires immediately (initial delay=0).
    // Subsequent slices execute faster than 1ms refire, so add a small sleep.
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        sched.do_time_slice();
    }

    let count = log.borrow().len();
    assert!(
        count >= 3,
        "Periodic timer should fire multiple times, got {count}"
    );

    sched.cancel_timer(timer, false);
    sched.remove_timer(timer);
    sched.remove_engine(eng);
    sched.remove_signal(sig);
}

// ─── Test 6: timer_cancel ───────────────────────────────────────

#[test]
fn timer_cancel() {
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();
    let log = Rc::new(RefCell::new(Vec::new()));

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "no_fire",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng);

    let timer = sched.create_timer(sig);
    sched.start_timer(timer, 1000, false); // 1s — won't fire in time
    sched.cancel_timer(timer, false);
    assert!(!sched.is_timer_running(timer));

    sched.do_time_slice();
    assert!(log.borrow().is_empty(), "Cancelled timer should not fire");

    sched.remove_timer(timer);
    sched.remove_engine(eng);
    sched.remove_signal(sig);
}

// ─── Test 7: timer_cancel_abort ─────────────────────────────────

#[test]
fn timer_cancel_abort() {
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();
    let timer = sched.create_timer(sig);

    // Start and immediately fire
    sched.start_timer(timer, 0, false);
    sched.do_time_slice(); // timer fires signal

    // Signal may be pending; cancel with abort_signal=true
    sched.start_timer(timer, 0, false);
    sched.do_time_slice(); // fires
    sched.cancel_timer(timer, true);
    // After abort, signal should not be pending
    assert!(!sched.is_pending(sig));

    sched.remove_timer(timer);
    sched.remove_signal(sig);
}

// ─── Test 8: engine_basic ───────────────────────────────────────

#[test]
fn engine_basic() {
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();
    let log = Rc::new(RefCell::new(Vec::new()));

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "basic",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng);

    // Fire signal → engine should cycle
    sched.fire(sig);
    sched.do_time_slice();
    assert_eq!(*log.borrow(), vec!["basic"]);

    sched.remove_engine(eng);
    sched.remove_signal(sig);
}

// ─── Test 9: engine_priority ────────────────────────────────────

#[test]
fn engine_priority() {
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let vl = sched.register_engine(
        Priority::VeryLow,
        Box::new(RecordingEngine {
            label: "very_low",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    let med = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "medium",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    let vh = sched.register_engine(
        Priority::VeryHigh,
        Box::new(RecordingEngine {
            label: "very_high",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );

    sched.wake_up(vl);
    sched.wake_up(med);
    sched.wake_up(vh);
    sched.do_time_slice();

    assert_eq!(*log.borrow(), vec!["very_high", "medium", "very_low"]);

    sched.remove_engine(vl);
    sched.remove_engine(med);
    sched.remove_engine(vh);
}

// ─── Test 10: engine_wake_sleep ─────────────────────────────────

#[test]
fn engine_wake_sleep() {
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "ws",
            log: Rc::clone(&log),
            stay_awake: true, // stays awake each cycle
        }),
    );

    // Wake → should cycle
    sched.wake_up(eng);
    sched.do_time_slice();
    assert_eq!(log.borrow().len(), 1);

    sched.do_time_slice();
    assert_eq!(log.borrow().len(), 2);

    // Sleep → should stop cycling
    sched.sleep(eng);
    sched.do_time_slice();
    assert_eq!(log.borrow().len(), 2, "Sleeping engine should not cycle");

    sched.remove_engine(eng);
}

// ─── Test 11: engine_multi_signal ───────────────────────────────

#[test]
fn engine_multi_signal() {
    let mut sched = EngineScheduler::new();
    let sig_a = sched.create_signal();
    let sig_b = sched.create_signal();
    let sig_c = sched.create_signal();

    let a_seen = Rc::new(RefCell::new(false));
    let b_seen = Rc::new(RefCell::new(false));
    let c_seen = Rc::new(RefCell::new(false));

    struct MultiSigEngine {
        sig_a: SignalId,
        sig_b: SignalId,
        sig_c: SignalId,
        a_seen: Rc<RefCell<bool>>,
        b_seen: Rc<RefCell<bool>>,
        c_seen: Rc<RefCell<bool>>,
    }
    impl Engine for MultiSigEngine {
        fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            if ctx.is_signaled(self.sig_a) {
                *self.a_seen.borrow_mut() = true;
            }
            if ctx.is_signaled(self.sig_b) {
                *self.b_seen.borrow_mut() = true;
            }
            if ctx.is_signaled(self.sig_c) {
                *self.c_seen.borrow_mut() = true;
            }
            false
        }
    }

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(MultiSigEngine {
            sig_a,
            sig_b,
            sig_c,
            a_seen: Rc::clone(&a_seen),
            b_seen: Rc::clone(&b_seen),
            c_seen: Rc::clone(&c_seen),
        }),
    );
    sched.connect(sig_a, eng);
    sched.connect(sig_b, eng);
    sched.connect(sig_c, eng);

    // Fire only A
    sched.fire(sig_a);
    sched.do_time_slice();
    assert!(*a_seen.borrow());
    assert!(!*b_seen.borrow());
    assert!(!*c_seen.borrow());

    // Reset
    *a_seen.borrow_mut() = false;

    // Fire B and C
    sched.fire(sig_b);
    sched.fire(sig_c);
    sched.do_time_slice();
    assert!(!*a_seen.borrow());
    assert!(*b_seen.borrow());
    assert!(*c_seen.borrow());

    sched.remove_engine(eng);
    sched.remove_signal(sig_a);
    sched.remove_signal(sig_b);
    sched.remove_signal(sig_c);
}
