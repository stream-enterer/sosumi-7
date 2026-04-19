use std::cell::RefCell;
use std::rc::Rc;

use emcore::emEngine::{emEngine, Priority};
use emcore::emEngineCtx::EngineCtx;
use emcore::emSignal::SignalId;

use crate::support::TestHarness;

/// emEngine that fires a signal on first Cycle and logs its label.
struct SignalFiringEngine {
    label: &'static str,
    log: Rc<RefCell<Vec<String>>>,
    signal: Option<SignalId>,
    fired: bool,
}

impl emEngine for SignalFiringEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        self.log.borrow_mut().push(format!("cycle:{}", self.label));
        if !self.fired {
            if let Some(sig) = self.signal {
                ctx.fire(sig);
            }
            self.fired = true;
        }
        false
    }
}

/// emEngine that modifies a shared counter when it runs.
struct CounterEngine {
    counter: Rc<RefCell<i32>>,
    delta: i32,
}

impl emEngine for CounterEngine {
    fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
        *self.counter.borrow_mut() += self.delta;
        false
    }
}

#[test]
fn engine_modifies_panel_on_signal() {
    let mut h = TestHarness::new();
    let counter = Rc::new(RefCell::new(0i32));

    let sig = h.scheduler.create_signal();

    // emEngine increments counter when signal fires
    let eng = h.scheduler.register_engine(
        Box::new(CounterEngine {
            counter: Rc::clone(&counter),
            delta: 1,
        }),
        Priority::Medium,
    );
    h.scheduler.connect(sig, eng);

    // Fire signal
    h.scheduler.fire(sig);
    h.tick();

    assert_eq!(*counter.borrow(), 1);
    h.scheduler.remove_engine(eng);
}

#[test]
fn timer_drives_engine_wake() {
    // Timer fires → signal → engine wakes and runs.
    // In a real app, the engine would set a flag and the main loop would
    // call tree.set_layout_rect (engines can't access the tree directly).
    let mut h = TestHarness::new();
    let engine_ran = Rc::new(RefCell::new(false));
    let engine_ran_clone = Rc::clone(&engine_ran);

    let sig = h.scheduler.create_signal();
    // 0ms one-shot timer fires immediately on next check_and_collect
    let timer = h.scheduler.create_timer(sig);
    h.scheduler.start_timer(timer, 0, false);

    let eng = h.scheduler.register_engine(
        Box::new(FlagEngine {
            flag: engine_ran_clone,
        }),
        Priority::Medium,
    );
    h.scheduler.connect(sig, eng);

    // Timer needs clock advancement — tick until it fires
    h.tick_n(3);

    assert!(
        *engine_ran.borrow(),
        "Timer should fire signal which wakes engine"
    );
    h.scheduler.remove_engine(eng);
}

struct FlagEngine {
    flag: Rc<RefCell<bool>>,
}

impl emEngine for FlagEngine {
    fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
        *self.flag.borrow_mut() = true;
        false
    }
}

#[test]
fn signal_removal_while_pending() {
    let mut h = TestHarness::new();
    let counter = Rc::new(RefCell::new(0i32));

    let sig = h.scheduler.create_signal();
    let eng = h.scheduler.register_engine(
        Box::new(CounterEngine {
            counter: Rc::clone(&counter),
            delta: 1,
        }),
        Priority::Medium,
    );
    h.scheduler.connect(sig, eng);

    // Fire then abort before time slice
    h.scheduler.fire(sig);
    h.scheduler.abort(sig);
    h.tick();

    // emEngine should NOT have run (signal was aborted)
    assert_eq!(*counter.borrow(), 0);
    h.scheduler.remove_engine(eng);
}

#[test]
fn engine_fires_signal_waking_sibling() {
    let mut h = TestHarness::new();
    let log = Rc::new(RefCell::new(Vec::new()));

    let sig = h.scheduler.create_signal();

    // emEngine A: high priority, fires signal on first Cycle
    let _eng_a = h.scheduler.register_engine(
        Box::new(SignalFiringEngine {
            label: "A",
            log: Rc::clone(&log),
            signal: Some(sig),
            fired: false,
        }),
        Priority::High,
    );
    h.scheduler.wake_up(_eng_a);

    // emEngine B: low Getpriorityoken by signal
    let eng_b = h.scheduler.register_engine(
        Box::new(SignalFiringEngine {
            label: "B",
            log: Rc::clone(&log),
            signal: None,
            fired: false,
        }),
        Priority::Low,
    );
    h.scheduler.connect(sig, eng_b);

    h.tick();

    let entries = log.borrow();
    // A runs first (higher GetPriority), fires signal, B wakes and runs in same slice
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0], "cycle:A");
    assert_eq!(entries[1], "cycle:B");
    drop(entries);
    h.scheduler.remove_engine(_eng_a);
    h.scheduler.remove_engine(eng_b);
}
