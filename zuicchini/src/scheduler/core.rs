use std::time::{Duration, Instant};

use slotmap::SlotMap;

use super::engine::{Engine, EngineCtx, EngineCtxInner, EngineData, EngineId, Priority};
use super::signal::{SignalConnection, SignalData, SignalId};
use super::timer::{TimerCentral, TimerId};

const TIME_SLICE_DURATION: Duration = Duration::from_millis(50);

/// The core cooperative scheduler. Manages signals, engines, and timers.
///
/// Faithfully implements the C++ emScheduler/emEngine algorithm:
/// - Clock-based `is_signaled` detection
/// - Instant signal chaining (engines woken mid-slice run in the same slice)
/// - Priority re-ascent after mid-slice signal processing
/// - Reference-counted signal-engine connections
/// - FIFO ordering with alternating time-slice parity for fairness
pub struct EngineScheduler {
    inner: EngineCtxInner,
}

impl EngineScheduler {
    pub fn new() -> Self {
        Self {
            inner: EngineCtxInner {
                signals: SlotMap::with_key(),
                engines: SlotMap::with_key(),
                pending_signals: Vec::new(),
                wake_queues: Default::default(),
                time_slice: 0,
                clock: 1, // Start at 1 so that clock > 0 comparisons work
                time_slice_counter: 0,
                deadline: Instant::now(),
                timer_central: TimerCentral::new(),
            },
        }
    }

    // ── Signal API ──────────────────────────────────────────────────

    /// Create a new signal and return its handle.
    pub fn create_signal(&mut self) -> SignalId {
        self.inner.signals.insert(SignalData::new())
    }

    /// Fire a signal, marking it pending for the next signal phase.
    pub fn fire(&mut self, id: SignalId) {
        if let Some(sig) = self.inner.signals.get_mut(id) {
            if !sig.pending {
                sig.pending = true;
                self.inner.pending_signals.push(id);
            }
        }
    }

    /// Check whether a signal is pending.
    pub fn is_pending(&self, id: SignalId) -> bool {
        self.inner.signals.get(id).is_some_and(|s| s.pending)
    }

    /// Abort a pending signal (cancel before processing).
    pub fn abort(&mut self, id: SignalId) {
        if let Some(sig) = self.inner.signals.get_mut(id) {
            sig.pending = false;
        }
        self.inner.pending_signals.retain(|&s| s != id);
    }

    /// Remove a signal entirely.
    pub fn remove_signal(&mut self, id: SignalId) {
        self.abort(id);
        self.inner.signals.remove(id);
    }

    /// Connect a signal to an engine so that firing the signal wakes the engine.
    ///
    /// Reference-counted: calling `connect` twice with the same signal+engine
    /// increments the refcount. `disconnect` decrements it; the connection is
    /// only severed when refcount reaches zero.
    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        if let Some(sig) = self.inner.signals.get_mut(signal) {
            // Check if connection already exists
            for conn in &mut sig.connected_engines {
                if conn.engine == engine {
                    conn.ref_count += 1;
                    return;
                }
            }
            sig.connected_engines.push(SignalConnection {
                engine,
                ref_count: 1,
            });
        }
    }

    /// Disconnect an engine from a signal. Decrements the refcount;
    /// only removes the connection when refcount reaches zero.
    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        if let Some(sig) = self.inner.signals.get_mut(signal) {
            let mut i = 0;
            while i < sig.connected_engines.len() {
                if sig.connected_engines[i].engine == engine {
                    sig.connected_engines[i].ref_count -= 1;
                    if sig.connected_engines[i].ref_count == 0 {
                        sig.connected_engines.swap_remove(i);
                    }
                    return;
                }
                i += 1;
            }
        }
    }

    /// Get the number of connection references for a signal-engine pair.
    pub fn get_signal_refs(&self, signal: SignalId, engine: EngineId) -> u32 {
        self.inner
            .signals
            .get(signal)
            .and_then(|sig| {
                sig.connected_engines
                    .iter()
                    .find(|c| c.engine == engine)
                    .map(|c| c.ref_count)
            })
            .unwrap_or(0)
    }

    // ── Engine API ──────────────────────────────────────────────────

    /// Register an engine with the given priority and behavior. Starts sleeping.
    pub fn register_engine(&mut self, priority: Priority, behavior: Box<dyn Engine>) -> EngineId {
        self.inner.engines.insert(EngineData {
            priority,
            awake_state: -1, // sleeping
            behavior: Some(behavior),
            clock: self.inner.clock,
        })
    }

    /// Remove an engine from the scheduler.
    pub fn remove_engine(&mut self, id: EngineId) {
        // Remove from wake queues
        for queue in &mut self.inner.wake_queues {
            queue.retain(|&e| e != id);
        }
        // Remove from all signal connections
        for (_, sig) in &mut self.inner.signals {
            sig.connected_engines.retain(|c| c.engine != id);
        }
        self.inner.engines.remove(id);
    }

    /// Wake up an engine so it runs in the current time slice.
    /// If already awake in the next-parity queue, moves it to current parity
    /// (critical for instant signal chaining).
    pub fn wake_up(&mut self, id: EngineId) {
        self.inner.wake_up_engine(id);
    }

    /// Change an engine's priority at runtime.
    /// If the engine is awake, it is moved to the correct wake queue.
    pub fn set_engine_priority(&mut self, id: EngineId, priority: Priority) {
        let Some(eng) = self.inner.engines.get_mut(id) else {
            return;
        };
        let old_priority = eng.priority;
        if old_priority == priority {
            return;
        }
        let awake_state = eng.awake_state;
        eng.priority = priority;

        if awake_state >= 0 {
            // Remove from old queue
            let old_idx = (old_priority as usize) * 2 + (awake_state as usize);
            self.inner.wake_queues[old_idx].retain(|&e| e != id);
            // Insert into new queue
            let new_idx = (priority as usize) * 2 + (awake_state as usize);
            self.inner.wake_queues[new_idx].push(id);
        }
    }

    /// Put an engine to sleep (remove from wake queues).
    pub fn sleep(&mut self, id: EngineId) {
        let Some(eng) = self.inner.engines.get_mut(id) else {
            return;
        };
        if eng.awake_state >= 0 {
            let queue_idx = (eng.priority as usize) * 2 + (eng.awake_state as usize);
            self.inner.wake_queues[queue_idx].retain(|&e| e != id);
            eng.awake_state = -1;
        }
    }

    // ── Timer API ────────────────────────────────────────────────────

    /// Create a timer that fires the given signal after `interval_ms`.
    /// If `periodic` is true, the timer repeats.
    pub fn create_timer(&mut self, signal: SignalId, interval_ms: u64, periodic: bool) -> TimerId {
        self.inner
            .timer_central
            .create_timer(signal, interval_ms, periodic)
    }

    /// Cancel a timer.
    pub fn cancel_timer(&mut self, id: TimerId) {
        self.inner.timer_central.cancel_timer(id);
    }

    /// Remove a timer entirely, freeing its slot.
    pub fn remove_timer(&mut self, id: TimerId) {
        self.inner.timer_central.remove_timer(id);
    }

    /// Check if a timer is still active (running).
    pub fn is_timer_running(&self, id: TimerId) -> bool {
        self.inner.timer_central.is_running(id)
    }

    // ── Time slice execution ────────────────────────────────────────

    /// Execute one time slice: process signals, run timers, then run engines.
    ///
    /// This implements the C++ `emScheduler::DoTimeSlice()` algorithm:
    /// 1. Process pending signals (wake connected engines)
    /// 2. Check timers and fire their signals
    /// 3. Process timer-fired signals
    /// 4. Run engines from highest to lowest priority
    /// 5. After each engine, process any signals it fired (instant chaining)
    /// 6. Re-ascend to higher priority if signals woke higher-priority engines
    pub fn do_time_slice(&mut self) {
        self.inner.deadline = Instant::now() + TIME_SLICE_DURATION;
        let next_parity = self.inner.time_slice ^ 1;

        // Timer phase: check timers and fire their signals
        let timer_signals = self.inner.timer_central.check_and_collect();
        for sig in timer_signals {
            if let Some(s) = self.inner.signals.get_mut(sig) {
                if !s.pending {
                    s.pending = true;
                    self.inner.pending_signals.push(sig);
                }
            }
        }

        // Main scheduling loop (matches C++ DoTimeSlice structure).
        // Start at highest priority and work down. After processing signals
        // (which may wake higher-priority engines), re-ascend.
        let parity = self.inner.time_slice as usize;
        let mut current_priority_idx = (Priority::COUNT - 1) * 2 + parity; // Start at VeryHigh

        loop {
            // Increment clock and process pending signals
            self.inner.clock += 1;
            self.process_pending_signals();

            // Find next non-empty queue at or below current priority
            loop {
                let queue = &self.inner.wake_queues[current_priority_idx];
                if !queue.is_empty() {
                    break;
                }
                if current_priority_idx < 2 + parity {
                    // Below VeryLow, we're done with all priorities.
                    // Move remaining awake engines to next parity for fairness.
                    for pri_val in 0..Priority::COUNT {
                        let src = pri_val * 2 + parity;
                        let dst = pri_val * 2 + (next_parity as usize);
                        let remaining: Vec<EngineId> =
                            self.inner.wake_queues[src].drain(..).collect();
                        self.inner.wake_queues[dst].extend(remaining);
                    }
                    self.inner.time_slice = next_parity;
                    self.inner.time_slice_counter += 1;
                    return;
                }
                // Step down by one priority level (skip by 2 because
                // queues are interleaved: [pri0_even, pri0_odd, pri1_even, ...])
                current_priority_idx -= 2;
            }

            // Take the first engine from this queue
            let engine_id = self.inner.wake_queues[current_priority_idx].remove(0);

            // Mark engine as sleeping (it was removed from queue)
            if let Some(eng) = self.inner.engines.get_mut(engine_id) {
                eng.awake_state = -1;
            } else {
                continue; // Engine was removed
            }

            // Extract behavior to avoid borrow conflict
            let mut behavior = match self.inner.engines.get_mut(engine_id) {
                Some(eng) => match eng.behavior.take() {
                    Some(b) => b,
                    None => continue,
                },
                None => continue,
            };

            // Call Cycle with context
            let stay_awake = {
                let mut ctx = EngineCtx {
                    engine_id,
                    scheduler: &mut self.inner,
                };
                behavior.cycle(&mut ctx)
            };

            // Reinsert behavior and update engine state
            if let Some(eng) = self.inner.engines.get_mut(engine_id) {
                eng.behavior = Some(behavior);
                eng.clock = self.inner.clock;

                if stay_awake && eng.awake_state < 0 {
                    // Engine wants to stay awake and wasn't re-woken during cycle.
                    // Queue for next time slice (not current, to prevent infinite loop).
                    eng.awake_state = next_parity;
                    let queue_idx = (eng.priority as usize) * 2 + (next_parity as usize);
                    self.inner.wake_queues[queue_idx].push(engine_id);
                }
            }

            // After engine cycle, process signals it may have fired.
            // Re-ascend if higher-priority engines were woken.
            self.process_pending_signals();

            // Re-ascend: check if any higher-priority queue (in current parity)
            // now has engines. This is the key to correct instant chaining.
            let highest = (Priority::COUNT - 1) * 2 + parity;
            for check_idx in (current_priority_idx..=highest).rev().step_by(2) {
                if !self.inner.wake_queues[check_idx].is_empty() {
                    current_priority_idx = check_idx;
                    break;
                }
            }
        }
    }

    /// Check if the current time slice has exceeded its deadline.
    pub fn is_time_slice_at_end(&self) -> bool {
        Instant::now() >= self.inner.deadline
    }

    /// Current time slice counter (incremented once per `do_time_slice` call).
    pub fn time_slice_counter(&self) -> u64 {
        self.inner.time_slice_counter
    }

    // ── Internal helpers ────────────────────────────────────────────

    /// Drain pending signals and wake their connected engines.
    fn process_pending_signals(&mut self) {
        while !self.inner.pending_signals.is_empty() {
            let pending: Vec<SignalId> = std::mem::take(&mut self.inner.pending_signals);
            for sig_id in pending {
                if let Some(sig) = self.inner.signals.get_mut(sig_id) {
                    sig.pending = false;
                    sig.clock = self.inner.clock;
                    // Collect engine IDs to wake (avoid borrow conflict)
                    let engines: Vec<EngineId> =
                        sig.connected_engines.iter().map(|c| c.engine).collect();
                    for eng_id in engines {
                        self.inner.wake_up_engine(eng_id);
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
        fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
            *self.count.borrow_mut() += 1;
            false // sleep after one cycle
        }
    }

    struct PollingEngine {
        remaining: u32,
        count: Rc<RefCell<u32>>,
    }

    impl Engine for PollingEngine {
        fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
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
            fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
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

    #[test]
    fn is_signaled_distinguishes_signals() {
        // Engine connected to two signals should be able to distinguish which fired.
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
        assert!(*a_fired.borrow());
        assert!(!*b_fired.borrow());
    }

    #[test]
    fn refcounted_connections() {
        let mut sched = EngineScheduler::new();
        let sig = sched.create_signal();
        let eng = sched.register_engine(
            Priority::Medium,
            Box::new(CountingEngine {
                count: Rc::new(RefCell::new(0)),
            }),
        );

        // Connect twice
        sched.connect(sig, eng);
        sched.connect(sig, eng);
        assert_eq!(sched.get_signal_refs(sig, eng), 2);

        // Disconnect once — still connected
        sched.disconnect(sig, eng);
        assert_eq!(sched.get_signal_refs(sig, eng), 1);

        // Disconnect again — now disconnected
        sched.disconnect(sig, eng);
        assert_eq!(sched.get_signal_refs(sig, eng), 0);
    }

    #[test]
    fn set_engine_priority() {
        let mut sched = EngineScheduler::new();
        let order = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct OrderEngine {
            label: &'static str,
            order: Rc<RefCell<Vec<&'static str>>>,
        }
        impl Engine for OrderEngine {
            fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.order.borrow_mut().push(self.label);
                false
            }
        }

        let eng_a = sched.register_engine(
            Priority::Low,
            Box::new(OrderEngine {
                label: "A",
                order: Rc::clone(&order),
            }),
        );
        let eng_b = sched.register_engine(
            Priority::High,
            Box::new(OrderEngine {
                label: "B",
                order: Rc::clone(&order),
            }),
        );

        // Promote A to VeryHigh — it should run before B
        sched.set_engine_priority(eng_a, Priority::VeryHigh);
        sched.wake_up(eng_a);
        sched.wake_up(eng_b);
        sched.do_time_slice();

        let executed = order.borrow();
        assert_eq!(executed[0], "A");
        assert_eq!(executed[1], "B");
    }

    #[test]
    fn instant_signal_chaining() {
        // Engine A fires a signal during its cycle that wakes Engine B.
        // Engine B should run in the SAME time slice.
        let mut sched = EngineScheduler::new();
        let sig = sched.create_signal();
        let log = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct FiringEngine {
            sig: SignalId,
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl Engine for FiringEngine {
            fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("A");
                ctx.fire(self.sig);
                false
            }
        }

        struct ReceivingEngine {
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl Engine for ReceivingEngine {
            fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("B");
                false
            }
        }

        let eng_b = sched.register_engine(
            Priority::Medium,
            Box::new(ReceivingEngine {
                log: Rc::clone(&log),
            }),
        );
        sched.connect(sig, eng_b);

        let _eng_a = sched.register_engine(
            Priority::High,
            Box::new(FiringEngine {
                sig,
                log: Rc::clone(&log),
            }),
        );
        sched.wake_up(_eng_a);

        sched.do_time_slice();

        let executed = log.borrow();
        assert_eq!(*executed, vec!["A", "B"]);
    }

    #[test]
    fn high_priority_reascent() {
        // Low-priority engine fires signal that wakes high-priority engine.
        // High-priority engine should still run in the same time slice.
        let mut sched = EngineScheduler::new();
        let sig = sched.create_signal();
        let log = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct FiringEngine {
            sig: SignalId,
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl Engine for FiringEngine {
            fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("low_fires");
                ctx.fire(self.sig);
                false
            }
        }

        struct HighEngine {
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl Engine for HighEngine {
            fn cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("high_runs");
                false
            }
        }

        let eng_high = sched.register_engine(
            Priority::VeryHigh,
            Box::new(HighEngine {
                log: Rc::clone(&log),
            }),
        );
        sched.connect(sig, eng_high);

        let eng_low = sched.register_engine(
            Priority::VeryLow,
            Box::new(FiringEngine {
                sig,
                log: Rc::clone(&log),
            }),
        );
        sched.wake_up(eng_low);

        sched.do_time_slice();

        let executed = log.borrow();
        // Low fires first (it was the only one awake), then high runs via signal chaining
        assert_eq!(*executed, vec!["low_fires", "high_runs"]);
    }
}
