use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::scheduler::{Engine, EngineCtx, EngineScheduler, Priority, SignalId};

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

#[test]
fn engines_execute_in_priority_order() {
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let low = sched.register_engine(
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
    let high = sched.register_engine(
        Priority::VeryHigh,
        Box::new(RecordingEngine {
            label: "very_high",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );

    sched.wake_up(low);
    sched.wake_up(med);
    sched.wake_up(high);
    sched.do_time_slice();

    let executed = log.borrow();
    assert_eq!(*executed, vec!["very_high", "medium", "very_low"]);
    drop(executed);
    sched.remove_engine(low);
    sched.remove_engine(med);
    sched.remove_engine(high);
}

#[test]
fn signal_chaining_within_time_slice() {
    // Engine A fires a signal that wakes Engine B during the same time slice.
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let sig = sched.create_signal();

    // Engine B: low priority, woken by signal
    let eng_b = sched.register_engine(
        Priority::Low,
        Box::new(RecordingEngine {
            label: "B",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng_b);

    // Fire signal before time slice — the signal phase wakes Engine B
    sched.fire(sig);
    sched.do_time_slice();

    let executed = log.borrow();
    assert_eq!(*executed, vec!["B"]);
    drop(executed);
    sched.remove_engine(eng_b);
}

#[test]
fn timer_fires_signal() {
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let sig = sched.create_signal();
    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "timer_target",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng);

    // Create a timer and start it with 0ms interval (fires immediately)
    let timer = sched.create_timer(sig);
    sched.start_timer(timer, 0, false);
    sched.do_time_slice();

    let executed = log.borrow();
    assert_eq!(*executed, vec!["timer_target"]);
    drop(executed);
    sched.remove_engine(eng);
}

#[test]
fn remove_engine_cleans_up() {
    let mut sched = EngineScheduler::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "removed",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.wake_up(eng);
    sched.remove_engine(eng);
    sched.do_time_slice();

    assert!(log.borrow().is_empty());
}

#[test]
fn instant_signal_chaining_via_engine() {
    // Engine A fires a signal during its cycle. Engine B (connected to that signal)
    // must run within the SAME time slice.
    let mut sched = EngineScheduler::new();
    let sig = sched.create_signal();
    let log = Rc::new(RefCell::new(Vec::<&str>::new()));

    struct FiringEngine {
        sig: SignalId,
        log: Rc<RefCell<Vec<&'static str>>>,
    }
    impl Engine for FiringEngine {
        fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            self.log.borrow_mut().push("A_fires");
            ctx.fire(self.sig);
            false
        }
    }

    let eng_b = sched.register_engine(
        Priority::Medium,
        Box::new(RecordingEngine {
            label: "B_runs",
            log: Rc::clone(&log),
            stay_awake: false,
        }),
    );
    sched.connect(sig, eng_b);

    let eng_a = sched.register_engine(
        Priority::High,
        Box::new(FiringEngine {
            sig,
            log: Rc::clone(&log),
        }),
    );
    sched.wake_up(eng_a);

    sched.do_time_slice();

    let executed = log.borrow();
    assert_eq!(*executed, vec!["A_fires", "B_runs"]);
    drop(executed);
    sched.remove_engine(eng_a);
    sched.remove_engine(eng_b);
}

#[test]
fn is_signaled_distinguishes_signals() {
    let mut sched = EngineScheduler::new();
    let sig_a = sched.create_signal();
    let sig_b = sched.create_signal();

    struct CheckSignalEngine {
        sig_a: SignalId,
        sig_b: SignalId,
        a_fired: Rc<RefCell<bool>>,
        b_fired: Rc<RefCell<bool>>,
    }
    impl Engine for CheckSignalEngine {
        fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            *self.a_fired.borrow_mut() = ctx.is_signaled(self.sig_a);
            *self.b_fired.borrow_mut() = ctx.is_signaled(self.sig_b);
            false
        }
    }

    let a_fired = Rc::new(RefCell::new(false));
    let b_fired = Rc::new(RefCell::new(false));
    let eng = sched.register_engine(
        Priority::Medium,
        Box::new(CheckSignalEngine {
            sig_a,
            sig_b,
            a_fired: Rc::clone(&a_fired),
            b_fired: Rc::clone(&b_fired),
        }),
    );
    sched.connect(sig_a, eng);
    sched.connect(sig_b, eng);

    // Fire only signal A
    sched.fire(sig_a);
    sched.do_time_slice();
    assert!(*a_fired.borrow(), "Signal A should have been detected");
    assert!(!*b_fired.borrow(), "Signal B should NOT have been detected");
    sched.remove_engine(eng);
}
