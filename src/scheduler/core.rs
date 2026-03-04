use std::time::{Duration, Instant};

use slotmap::SlotMap;

use super::engine::{Engine, EngineData, EngineId, Priority};
use super::signal::{SignalData, SignalId};
use super::timer::{TimerCentral, TimerId};

const TIME_SLICE_DURATION: Duration = Duration::from_millis(50);

/// The core cooperative scheduler. Manages signals and engines, executes
/// engines by priority within time slices.
pub struct EngineScheduler {
    signals: SlotMap<SignalId, SignalData>,
    engines: SlotMap<EngineId, EngineData>,
    /// Signals fired but not yet processed.
    pending_signals: Vec<SignalId>,
    /// 10 wake queues: 5 priorities × 2 parities.
    wake_queues: [Vec<EngineId>; 10],
    time_slice_counter: u64,
    deadline: Instant,
    timer_central: TimerCentral,
    timer_engine_id: Option<EngineId>,
}

impl EngineScheduler {
    pub fn new() -> Self {
        Self {
            signals: SlotMap::with_key(),
            engines: SlotMap::with_key(),
            pending_signals: Vec::new(),
            wake_queues: Default::default(),
            time_slice_counter: 0,
            deadline: Instant::now(),
            timer_central: TimerCentral::new(),
            timer_engine_id: None,
        }
    }

    // ── Signal API ──────────────────────────────────────────────────

    /// Create a new signal and return its handle.
    pub fn create_signal(&mut self) -> SignalId {
        self.signals.insert(SignalData::new())
    }

    /// Fire a signal, marking it pending for the next signal phase.
    pub fn fire(&mut self, id: SignalId) {
        if let Some(sig) = self.signals.get_mut(id) {
            if !sig.pending {
                sig.pending = true;
                self.pending_signals.push(id);
            }
        }
    }

    /// Check whether a signal is pending.
    pub fn is_pending(&self, id: SignalId) -> bool {
        self.signals.get(id).is_some_and(|s| s.pending)
    }

    /// Abort a pending signal (cancel before processing).
    pub fn abort(&mut self, id: SignalId) {
        if let Some(sig) = self.signals.get_mut(id) {
            sig.pending = false;
        }
        self.pending_signals.retain(|&s| s != id);
    }

    /// Remove a signal entirely.
    pub fn remove_signal(&mut self, id: SignalId) {
        self.abort(id);
        self.signals.remove(id);
    }

    /// Connect a signal to an engine so that firing the signal wakes the engine.
    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        if let Some(sig) = self.signals.get_mut(signal) {
            if !sig.connected_engines.contains(&engine) {
                sig.connected_engines.push(engine);
            }
        }
    }

    /// Disconnect an engine from a signal.
    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        if let Some(sig) = self.signals.get_mut(signal) {
            sig.connected_engines.retain(|&e| e != engine);
        }
    }

    // ── Engine API ──────────────────────────────────────────────────

    /// Register an engine with the given priority and behavior. Starts sleeping.
    pub fn register_engine(
        &mut self,
        priority: Priority,
        behavior: Box<dyn Engine>,
    ) -> EngineId {
        self.engines.insert(EngineData {
            priority,
            awake: false,
            behavior: Some(behavior),
        })
    }

    /// Remove an engine from the scheduler.
    pub fn remove_engine(&mut self, id: EngineId) {
        // Remove from wake queues
        for queue in &mut self.wake_queues {
            queue.retain(|&e| e != id);
        }
        // Remove from all signal connections
        for (_, sig) in &mut self.signals {
            sig.connected_engines.retain(|&e| e != id);
        }
        self.engines.remove(id);
    }

    /// Wake up an engine so it runs in the next (or current) engine phase.
    pub fn wake_up(&mut self, id: EngineId) {
        if let Some(eng) = self.engines.get_mut(id) {
            if !eng.awake {
                eng.awake = true;
                let parity = (self.time_slice_counter as usize) % 2;
                let queue_idx = (eng.priority as usize) * 2 + parity;
                self.wake_queues[queue_idx].push(id);
            }
        }
    }

    // ── Timer API ────────────────────────────────────────────────────

    /// Create a timer that fires the given signal after `interval_ms`.
    /// If `periodic` is true, the timer repeats.
    pub fn create_timer(
        &mut self,
        signal: SignalId,
        interval_ms: u64,
        periodic: bool,
    ) -> TimerId {
        let id = self.timer_central.create_timer(signal, interval_ms, periodic);
        // Ensure the timer engine is awake
        self.ensure_timer_engine_awake();
        id
    }

    /// Cancel a timer.
    pub fn cancel_timer(&mut self, id: TimerId) {
        self.timer_central.cancel_timer(id);
    }

    fn ensure_timer_engine_awake(&mut self) {
        if let Some(eng_id) = self.timer_engine_id {
            self.wake_up(eng_id);
        }
    }

    // ── Time slice execution ────────────────────────────────────────

    /// Execute one time slice: process signals, then run awake engines by priority.
    pub fn do_time_slice(&mut self) {
        self.deadline = Instant::now() + TIME_SLICE_DURATION;

        // Signal phase
        self.process_pending_signals();

        // Timer phase: check timers and fire their signals
        let stay_awake = self.timer_central.cycle();
        let fired: Vec<SignalId> = self.timer_central.signals_to_fire.drain(..).collect();
        for sig in fired {
            self.fire(sig);
        }
        if stay_awake {
            self.ensure_timer_engine_awake();
        }
        self.process_pending_signals();

        // Engine phase: iterate from VeryHigh down to VeryLow
        let parity = (self.time_slice_counter as usize) % 2;
        for pri_val in (0..Priority::COUNT).rev() {
            let queue_idx = pri_val * 2 + parity;
            let mut i = 0;
            while i < self.wake_queues[queue_idx].len() {
                let engine_id = self.wake_queues[queue_idx][i];

                // Extract behavior to avoid borrow conflict
                let mut behavior = match self.engines.get_mut(engine_id) {
                    Some(eng) => match eng.behavior.take() {
                        Some(b) => b,
                        None => {
                            i += 1;
                            continue;
                        }
                    },
                    None => {
                        // Engine was removed; clean up queue entry
                        self.wake_queues[queue_idx].swap_remove(i);
                        continue;
                    }
                };

                let stay_awake = behavior.cycle();

                // Reinsert behavior
                if let Some(eng) = self.engines.get_mut(engine_id) {
                    eng.behavior = Some(behavior);
                    if !stay_awake {
                        eng.awake = false;
                        self.wake_queues[queue_idx].swap_remove(i);
                        // Don't increment i since swap_remove moved last element here
                        // Process any signals fired during engine cycle (instant chaining)
                        self.process_pending_signals();
                        continue;
                    }
                }

                // Process any signals fired during engine cycle (instant chaining)
                self.process_pending_signals();
                i += 1;
            }
        }

        // Move remaining awake engines to the next parity's queues for fairness
        let next_parity = 1 - parity;
        for pri_val in 0..Priority::COUNT {
            let src = pri_val * 2 + parity;
            let dst = pri_val * 2 + next_parity;
            let remaining: Vec<EngineId> = self.wake_queues[src].drain(..).collect();
            self.wake_queues[dst].extend(remaining);
        }

        self.time_slice_counter += 1;
    }

    /// Check if the current time slice has exceeded its deadline.
    pub fn is_time_slice_at_end(&self) -> bool {
        Instant::now() >= self.deadline
    }

    /// Current time slice counter.
    pub fn time_slice_counter(&self) -> u64 {
        self.time_slice_counter
    }

    // ── Internal helpers ────────────────────────────────────────────

    /// Drain pending signals and wake their connected engines.
    fn process_pending_signals(&mut self) {
        while !self.pending_signals.is_empty() {
            // Take the pending list to avoid borrow issues
            let pending: Vec<SignalId> = std::mem::take(&mut self.pending_signals);
            for sig_id in pending {
                if let Some(sig) = self.signals.get_mut(sig_id) {
                    sig.pending = false;
                    // Clone to avoid borrow conflict with wake_up
                    let engines = sig.connected_engines.clone();
                    for eng_id in engines {
                        self.wake_up(eng_id);
                    }
                }
            }
        }
    }
}

impl Default for EngineScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct CountingEngine {
        count: Rc<RefCell<u32>>,
    }

    impl Engine for CountingEngine {
        fn cycle(&mut self) -> bool {
            *self.count.borrow_mut() += 1;
            false // sleep after one cycle
        }
    }

    struct PollingEngine {
        remaining: u32,
        count: Rc<RefCell<u32>>,
    }

    impl Engine for PollingEngine {
        fn cycle(&mut self) -> bool {
            *self.count.borrow_mut() += 1;
            self.remaining -= 1;
            self.remaining > 0
        }
    }

    #[test]
    fn engine_wakes_and_runs() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let id = sched.register_engine(
            Priority::Medium,
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
        );
        sched.wake_up(id);
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 1);
        // Engine returned false, should not run again
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn polling_engine_runs_multiple_slices() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let id = sched.register_engine(
            Priority::Medium,
            Box::new(PollingEngine {
                remaining: 3,
                count: Rc::clone(&count),
            }),
        );
        sched.wake_up(id);
        sched.do_time_slice();
        sched.do_time_slice();
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 3);
        // Should be asleep now
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 3);
    }

    #[test]
    fn signal_wakes_engine() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let sig = sched.create_signal();
        let eng = sched.register_engine(
            Priority::High,
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
        );
        sched.connect(sig, eng);
        // Engine is sleeping, nothing should run
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 0);
        // Fire signal and run
        sched.fire(sig);
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn signal_abort() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let sig = sched.create_signal();
        let eng = sched.register_engine(
            Priority::Medium,
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
        );
        sched.connect(sig, eng);
        sched.fire(sig);
        sched.abort(sig);
        sched.do_time_slice();
        assert_eq!(*count.borrow(), 0);
    }

    #[test]
    fn priority_ordering() {
        let mut sched = EngineScheduler::new();
        let order = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct OrderEngine {
            label: &'static str,
            order: Rc<RefCell<Vec<&'static str>>>,
        }
        impl Engine for OrderEngine {
            fn cycle(&mut self) -> bool {
                self.order.borrow_mut().push(self.label);
                false
            }
        }

        let low = sched.register_engine(
            Priority::Low,
            Box::new(OrderEngine {
                label: "low",
                order: Rc::clone(&order),
            }),
        );
        let high = sched.register_engine(
            Priority::VeryHigh,
            Box::new(OrderEngine {
                label: "high",
                order: Rc::clone(&order),
            }),
        );
        sched.wake_up(low);
        sched.wake_up(high);
        sched.do_time_slice();
        let executed = order.borrow();
        assert_eq!(executed[0], "high");
        assert_eq!(executed[1], "low");
    }
}
