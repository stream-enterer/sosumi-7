use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::scheduler::{Engine, EngineScheduler, Priority};

struct RecordingEngine {
    label: &'static str,
    log: Rc<RefCell<Vec<&'static str>>>,
    stay_awake: bool,
}

impl Engine for RecordingEngine {
    fn cycle(&mut self) -> bool {
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

    // Create a timer with 0ms interval (fires immediately)
    sched.create_timer(sig, 0, false);
    sched.do_time_slice();

    let executed = log.borrow();
    assert_eq!(*executed, vec!["timer_target"]);
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
