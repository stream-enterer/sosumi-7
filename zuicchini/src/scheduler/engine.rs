use slotmap::new_key_type;

new_key_type! {
    /// Handle to an engine in the scheduler.
    pub struct EngineId;
}

/// A unit of cooperative work executed by the scheduler.
///
/// Engines are the primary scheduling primitive. They receive `cycle()` calls
/// from the scheduler with an `EngineCtx` that provides access to signals,
/// timers, and time-slice queries.
pub trait Engine {
    /// Called when the engine is awake. Return `true` to stay awake next slice,
    /// `false` to go to sleep.
    ///
    /// The `ctx` parameter provides access to the scheduler for firing signals,
    /// checking `is_signaled`, querying `is_time_slice_at_end`, etc.
    fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool;
}

/// Engine execution priority. Higher priority engines run first within a time slice.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    VeryLow = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    VeryHigh = 4,
}

impl Priority {
    pub const COUNT: usize = 5;
}

/// Internal state for a registered engine.
pub(crate) struct EngineData {
    pub priority: Priority,
    /// -1 = sleeping, 0 or 1 = awake in that parity's queue.
    pub awake_state: i8,
    pub behavior: Option<Box<dyn Engine>>,
    /// Clock value after last Cycle() call. Used by `is_signaled`.
    pub clock: u64,
}

/// Context passed to `Engine::cycle()`, providing scheduler operations.
///
/// This is a limited borrow of the scheduler that lets engines fire signals,
/// check which signals woke them, query the time-slice deadline, and wake
/// other engines.
pub struct EngineCtx<'a> {
    /// The ID of the engine currently being cycled.
    pub(crate) engine_id: EngineId,
    pub(crate) scheduler: &'a mut EngineCtxInner,
}

/// The scheduler fields accessible through EngineCtx.
/// Separated from EngineData to allow simultaneous borrows.
pub(crate) struct EngineCtxInner {
    pub signals: slotmap::SlotMap<super::signal::SignalId, super::signal::SignalData>,
    pub engines: slotmap::SlotMap<EngineId, EngineData>,
    pub pending_signals: Vec<super::signal::SignalId>,
    pub wake_queues: [Vec<EngineId>; 10],
    pub time_slice: i8,
    pub clock: u64,
    pub time_slice_counter: u64,
    pub deadline: std::time::Instant,
    pub timer_central: super::timer::TimerCentral,
}

impl EngineCtx<'_> {
    /// Fire a signal, marking it pending.
    pub fn fire(&mut self, id: super::signal::SignalId) {
        if let Some(sig) = self.scheduler.signals.get_mut(id) {
            if !sig.pending {
                sig.pending = true;
                self.scheduler.pending_signals.push(id);
            }
        }
    }

    /// Check whether a specific signal has been signaled since the last
    /// time this engine's `cycle()` was called.
    ///
    /// This is the Rust equivalent of the C++ `emEngine::IsSignaled()`.
    pub fn is_signaled(&self, signal: super::signal::SignalId) -> bool {
        if let Some(sig) = self.scheduler.signals.get(signal) {
            sig.clock
                > self
                    .scheduler
                    .engines
                    .get(self.engine_id)
                    .map_or(0, |e| e.clock)
        } else {
            false
        }
    }

    /// Check if the current time slice has exceeded its deadline.
    pub fn is_time_slice_at_end(&self) -> bool {
        std::time::Instant::now() >= self.scheduler.deadline
    }

    /// Wake up another engine so it runs in the current time slice.
    pub fn wake_up(&mut self, id: EngineId) {
        self.scheduler.wake_up_engine(id);
    }

    /// Get the current engine's ID.
    pub fn id(&self) -> EngineId {
        self.engine_id
    }
}

impl EngineCtxInner {
    /// Wake up an engine, moving it to the current time slice if needed.
    /// Matches C++ `WakeUpImp()` semantics.
    pub(crate) fn wake_up_engine(&mut self, id: EngineId) {
        let Some(eng) = self.engines.get_mut(id) else {
            return;
        };

        let current_parity = self.time_slice;

        if eng.awake_state == current_parity {
            // Already awake in current time slice — nothing to do.
            return;
        }

        if eng.awake_state >= 0 {
            // Awake but in the *next* parity's queue — remove from there.
            let old_queue_idx = (eng.priority as usize) * 2 + (eng.awake_state as usize);
            self.wake_queues[old_queue_idx].retain(|&e| e != id);
        }

        // Insert into current time slice's queue.
        eng.awake_state = current_parity;
        let queue_idx = (eng.priority as usize) * 2 + (current_parity as usize);
        self.wake_queues[queue_idx].push(id);
    }
}
