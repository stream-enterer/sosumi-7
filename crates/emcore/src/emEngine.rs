use std::collections::HashMap;

use slotmap::new_key_type;
use winit::window::WindowId;

use super::emPanelTree::PanelTree;
use super::emWindow::emWindow;

new_key_type! {
    /// Handle to an engine in the scheduler.
    pub struct EngineId;
}

/// A unit of cooperative work executed by the scheduler.
///
/// Engines are the primary scheduling primitive. They receive `cycle()` calls
/// from the scheduler with an `EngineCtx` that provides access to signals,
/// timers, and time-slice queries.
pub trait emEngine {
    /// Called when the engine is awake. Return `true` to stay awake next slice,
    /// `false` to go to sleep.
    ///
    /// The `ctx` parameter provides access to the scheduler for firing signals,
    /// checking `is_signaled`, querying `is_time_slice_at_end`, etc.
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool;
}

/// emEngine execution priority. Higher priority engines run first within a time slice.
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
    pub behavior: Option<Box<dyn emEngine>>,
    /// Clock value after last Cycle() call. Used by `is_signaled`.
    pub clock: u64,
}

/// emContext passed to `emEngine::cycle()`, providing scheduler operations.
///
/// This is a limited borrow of the scheduler that lets engines fire signals,
/// check which signals woke them, query the time-slice deadline, and wake
/// other engines.
pub struct EngineCtx<'a> {
    /// The ID of the engine currently being cycled.
    pub(crate) engine_id: EngineId,
    pub(crate) scheduler: &'a mut EngineCtxInner,
    pub tree: &'a mut PanelTree,
    pub windows: &'a mut HashMap<WindowId, std::rc::Rc<std::cell::RefCell<emWindow>>>,
}

/// The scheduler fields accessible through EngineCtx.
/// Separated from EngineData to allow simultaneous borrows.
pub(crate) struct EngineCtxInner {
    pub signals: slotmap::SlotMap<super::emSignal::SignalId, super::emSignal::SignalData>,
    pub engines: slotmap::SlotMap<EngineId, EngineData>,
    pub pending_signals: Vec<super::emSignal::SignalId>,
    pub wake_queues: [Vec<EngineId>; 10],
    pub time_slice: i8,
    pub clock: u64,
    pub time_slice_counter: u64,
    pub deadline: std::time::Instant,
    pub timer_central: super::emTimer::TimerCentral,
    /// Current priority scan index during `do_time_slice`, or `None` outside
    /// a time slice.  Mirrors C++ `Scheduler.CurrentAwakeList`.  `wake_up_engine`
    /// and `set_engine_priority` bump this upward so higher-priority engines
    /// woken mid-slice are visited in the same slice.
    pub current_awake_idx: Option<usize>,
}

impl EngineCtx<'_> {
    /// Fire a signal, marking it pending.
    pub fn fire(&mut self, id: super::emSignal::SignalId) {
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
    pub fn IsSignaled(&self, signal: super::emSignal::SignalId) -> bool {
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
    pub fn IsTimeSliceAtEnd(&self) -> bool {
        std::time::Instant::now() >= self.scheduler.deadline
    }

    /// Wake up another engine so it runs in the current time slice.
    pub fn wake_up(&mut self, id: EngineId) {
        self.scheduler.wake_up_engine(id);
    }

    /// Connect a signal to an engine so the engine wakes whenever the
    /// signal is fired. Forwards to `EngineScheduler::connect`.
    pub fn connect(&mut self, signal: super::emSignal::SignalId, engine: EngineId) {
        self.scheduler.connect_inner(signal, engine);
    }

    /// Disconnect a signal→engine wake link. Forwards to
    /// `EngineScheduler::disconnect`.
    pub fn disconnect(&mut self, signal: super::emSignal::SignalId, engine: EngineId) {
        self.scheduler.disconnect_inner(signal, engine);
    }

    /// Remove a signal from the scheduler. Forwards to
    /// `EngineScheduler::remove_signal`.
    pub fn remove_signal(&mut self, signal: super::emSignal::SignalId) {
        self.scheduler.remove_signal_inner(signal);
    }

    /// Get the current engine's ID.
    pub fn id(&self) -> EngineId {
        self.engine_id
    }
}

impl EngineCtxInner {
    /// Connect a signal to an engine (refcounted). Body of
    /// `EngineScheduler::connect`; lives here so `EngineCtx` can forward
    /// to it while holding `&mut EngineCtxInner`.
    pub(crate) fn connect_inner(&mut self, signal: super::emSignal::SignalId, engine: EngineId) {
        if let Some(sig) = self.signals.get_mut(signal) {
            for conn in &mut sig.connected_engines {
                if conn.engine == engine {
                    conn.ref_count += 1;
                    return;
                }
            }
            sig.connected_engines
                .push(super::emSignal::SignalConnection {
                    engine,
                    ref_count: 1,
                });
        }
    }

    /// Disconnect a signal↔engine pair (refcounted). Body of
    /// `EngineScheduler::disconnect`.
    pub(crate) fn disconnect_inner(&mut self, signal: super::emSignal::SignalId, engine: EngineId) {
        if let Some(sig) = self.signals.get_mut(signal) {
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

    /// Remove a signal entirely (aborts pending state first). Body of
    /// `EngineScheduler::remove_signal`.
    pub(crate) fn remove_signal_inner(&mut self, id: super::emSignal::SignalId) {
        if let Some(sig) = self.signals.get_mut(id) {
            sig.pending = false;
            sig.clock = 0;
        }
        self.pending_signals.retain(|&s| s != id);
        self.signals.remove(id);
    }

    /// Wake up an engine, moving it to the current time slice if needed.
    /// Matches C++ `WakeUpImp()` semantics, including priority re-ascent.
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

        // C++ re-ascent: if this queue is above the current scan position,
        // bump the scan pointer so the main loop will visit this priority.
        if let Some(cur) = self.current_awake_idx {
            if queue_idx > cur {
                self.current_awake_idx = Some(queue_idx);
            }
        }
    }
}
