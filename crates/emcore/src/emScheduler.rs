use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slotmap::{SecondaryMap, SlotMap};
use winit::window::WindowId;

use super::emEngine::{emEngine, EngineData, EngineId, Priority};
use super::emEngineCtx::{DeferredAction, EngineCtx};
use super::emPanelScope::PanelScope;
#[cfg(test)]
use super::emPanelTree::PanelTree;
use super::emSignal::{SignalData, SignalId};
use super::emTimer::{TimerCentral, TimerId};
use super::emWindow::emWindow;

/// Internal mutable state owned by `EngineScheduler`. Moved here from
/// `emEngine` during Phase 1 Task 9 so that `EngineCtx` no longer needs
/// a borrow-splitting inner struct.
pub(crate) struct EngineCtxInner {
    pub signals: SlotMap<SignalId, SignalData>,
    pub engines: SlotMap<EngineId, EngineData>,
    /// Per-engine `PanelScope` — tells `DoTimeSlice` how to resolve the
    /// engine's target tree (Framework: none; Toplevel(wid): windows[wid].tree;
    /// SubView{wid,pid}: sub-view under outer panel `pid` in windows[wid].tree).
    /// Populated on `register_engine`, cleared on `remove_engine`.
    /// Phase 3.5.A Task 6.2 (replaces the old `engine_locations`).
    pub engine_scopes: SecondaryMap<EngineId, PanelScope>,
    pub pending_signals: Vec<SignalId>,
    pub wake_queues: [Vec<EngineId>; 10],
    pub time_slice: i8,
    pub clock: u64,
    pub time_slice_counter: u64,
    pub deadline: Instant,
    pub timer_central: TimerCentral,
    /// Current priority scan index during `DoTimeSlice`, or `None` outside
    /// a time slice. Mirrors C++ `Scheduler.CurrentAwakeList`. `wake_up_engine`
    /// and `set_engine_priority` bump this upward so higher-priority engines
    /// woken mid-slice are visited in the same slice.
    pub current_awake_idx: Option<usize>,
}

impl EngineCtxInner {
    pub(crate) fn connect_inner(&mut self, signal: SignalId, engine: EngineId) {
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

    pub(crate) fn disconnect_inner(&mut self, signal: SignalId, engine: EngineId) {
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

    pub(crate) fn remove_signal_inner(&mut self, id: SignalId) {
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
            return;
        }

        if eng.awake_state >= 0 {
            let old_queue_idx = (eng.priority as usize) * 2 + (eng.awake_state as usize);
            self.wake_queues[old_queue_idx].retain(|&e| e != id);
        }

        eng.awake_state = current_parity;
        let queue_idx = (eng.priority as usize) * 2 + (current_parity as usize);
        self.wake_queues[queue_idx].push(id);

        if let Some(cur) = self.current_awake_idx {
            if queue_idx > cur {
                self.current_awake_idx = Some(queue_idx);
            }
        }
    }
}

const TIME_SLICE_DURATION: Duration = Duration::from_millis(50);

/// Process-wide monotonic clock start point used by [`emGetClockMS`].
static CLOCK_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Port of C++ `emGetClockMS()` (emStd1.h). Returns a monotonic millisecond
/// clock value that increases over the process lifetime. Used by
/// `emViewPort::GetInputClockMS` dispatch.
pub fn emGetClockMS() -> u64 {
    let start = CLOCK_START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

/// The core cooperative scheduler. Manages signals, engines, and timers.
///
/// Faithfully implements the C++ emScheduler/emEngine algorithm:
/// - Clock-based `is_signaled` detection
/// - Instant signal chaining (engines woken at same or lower priority run in the same slice)
/// - Priority re-ascent (engines woken at higher priority mid-slice run in the same slice)
/// - Reference-counted signal-engine connections
/// - FIFO ordering with alternating time-slice parity for fairness
pub struct EngineScheduler {
    pub(crate) inner: EngineCtxInner,
    terminated: bool,
    /// Phase 3.5 Task 3 — monotonic counter for `allocate_dialog_id`.
    /// Relocated from `App::next_dialog_id` (Phase 3.5.A Task 9) so that
    /// construction code can allocate IDs through `ConstructCtx` without
    /// requiring `&mut App`. Permanent home per spec §8.
    pub(crate) next_dialog_id: u64,
}

impl EngineScheduler {
    pub fn new() -> Self {
        Self {
            terminated: false,
            next_dialog_id: 0,
            inner: EngineCtxInner {
                signals: SlotMap::with_key(),
                engines: SlotMap::with_key(),
                engine_scopes: SecondaryMap::new(),
                pending_signals: Vec::new(),
                wake_queues: Default::default(),
                time_slice: 0,
                clock: 1, // Start at 1 so that clock > 0 comparisons work
                time_slice_counter: 0,
                deadline: Instant::now(),
                timer_central: TimerCentral::new(),
                current_awake_idx: None,
            },
        }
    }

    // ── DialogId allocator ─────────────────────────────────────────

    /// Allocate a fresh `DialogId`. Monotonic counter; panics on u64 overflow.
    ///
    /// Phase 3.5 Task 3: permanent home (spec §8). `App::allocate_dialog_id`
    /// delegates here; `App::next_dialog_id` has been deleted.
    pub fn allocate_dialog_id(&mut self) -> crate::emGUIFramework::DialogId {
        let id = crate::emGUIFramework::DialogId(self.next_dialog_id);
        self.next_dialog_id = self
            .next_dialog_id
            .checked_add(1)
            .expect("DialogId overflow — u64 exhausted");
        id
    }

    // ── Scope query ────────────────────────────────────────────────

    /// Return all engine IDs registered under `scope`.
    ///
    /// Phase 3.5 Task 3 (spec §6): used by `App::close_dialog_by_id` to
    /// unregister the per-window engines associated with a dialog's toplevel
    /// window. Ownership-forced: `emWindow` cannot borrow the scheduler, so
    /// the query lives here.
    pub fn engines_for_scope(&self, scope: PanelScope) -> Vec<EngineId> {
        self.inner
            .engine_scopes
            .iter()
            .filter_map(|(eid, s)| if *s == scope { Some(eid) } else { None })
            .collect()
    }

    // ── Signal API ──────────────────────────────────────────────────

    /// Create a new signal and return its handle.
    pub fn create_signal(&mut self) -> SignalId {
        self.inner.signals.insert(SignalData::new())
    }

    /// Fire a signal, marking it pending for the next signal phase.
    ///
    /// Defensive: a lookup miss is a silent no-op. This is relied on by
    /// popup teardown in `emView::RawVisitAbs` — `close_signal` is removed
    /// synchronously while the matching winit window is dropped one frame
    /// later via the `App::pending_actions` drain. A late `CloseRequested`
    /// arriving in that gap dispatches into `App::window_event`, which
    /// calls `fire(close_signal)` on the already-removed key.
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

    /// Clock-based signaled check for a given engine, mirroring
    /// C++ `emEngine::IsSignaled`. Returns true if the signal was processed
    /// after the engine's last `Cycle()` call.
    ///
    /// Use outside of `Cycle` (e.g. from `emView::Update` which the
    /// `UpdateEngineClass` invokes) to ask "has this signal fired since this
    /// engine last ran?".
    pub fn is_signaled_for_engine(&self, signal: SignalId, engine: EngineId) -> bool {
        let sig_clock = match self.inner.signals.get(signal) {
            Some(s) => s.clock,
            None => return false,
        };
        let eng_clock = self.inner.engines.get(engine).map_or(0, |e| e.clock);
        sig_clock > eng_clock
    }

    /// Abort a pending signal (cancel before processing).
    pub fn abort(&mut self, id: SignalId) {
        if let Some(sig) = self.inner.signals.get_mut(id) {
            sig.pending = false;
            sig.clock = 0;
        }
        self.inner.pending_signals.retain(|&s| s != id);
    }

    /// Clear the pending-signal queue without firing the signals.
    ///
    /// Sets `pending = false` on every queued signal then empties the queue.
    /// Intended for test teardown when compound records have allocated internal
    /// signals with no external handle. Not for production use.
    pub fn abort_all_pending(&mut self) {
        for &id in &self.inner.pending_signals {
            if let Some(sig) = self.inner.signals.get_mut(id) {
                sig.pending = false;
            }
        }
        self.inner.pending_signals.clear();
    }

    /// Remove a signal entirely.
    pub fn remove_signal(&mut self, id: SignalId) {
        self.inner.remove_signal_inner(id);
    }

    /// Connect a signal to an engine so that firing the signal wakes the engine.
    ///
    /// Reference-counted: calling `connect` twice with the same signal+engine
    /// increments the refcount. `disconnect` decrements it; the connection is
    /// only severed when refcount reaches zero.
    pub fn connect(&mut self, signal: SignalId, engine: EngineId) {
        self.inner.connect_inner(signal, engine);
    }

    /// Disconnect an engine from a signal. Decrements the refcount;
    /// only removes the connection when refcount reaches zero.
    pub fn disconnect(&mut self, signal: SignalId, engine: EngineId) {
        self.inner.disconnect_inner(signal, engine);
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

    // ── emEngine API ──────────────────────────────────────────────────

    /// Register an engine with the given priority and behavior. Starts sleeping.
    ///
    /// `scope` tells `DoTimeSlice` how to resolve this engine's tree:
    /// - `PanelScope::Framework`: no tree — `ctx.tree` will be `None`.
    /// - `PanelScope::Toplevel(wid)`: `ctx.tree` is `windows[wid].tree`.
    /// - `PanelScope::SubView { window_id, outer_panel_id }`: `ctx.tree` is
    ///   the sub-tree under the outer panel `outer_panel_id` inside
    ///   `windows[window_id].tree`.
    ///
    /// Phase 3.5.A Task 6.2: replaces the Phase 1.75 `TreeLocation`-keyed
    /// dispatch with a flat per-window scheme.
    pub fn register_engine(
        &mut self,
        behavior: Box<dyn emEngine>,
        priority: Priority,
        scope: PanelScope,
    ) -> EngineId {
        let id = self.inner.engines.insert(EngineData {
            priority,
            awake_state: -1, // sleeping
            behavior: Some(behavior),
            clock: self.inner.clock,
        });
        self.inner.engine_scopes.insert(id, scope);
        id
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
        self.inner.engine_scopes.remove(id);
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

            // C++ re-ascent: bump scan pointer if this engine is in the
            // current parity and moved to a higher-priority queue.
            if awake_state == self.inner.time_slice {
                if let Some(cur) = self.inner.current_awake_idx {
                    if new_idx > cur {
                        self.inner.current_awake_idx = Some(new_idx);
                    }
                }
            }
        }
    }

    /// Get an engine's current priority.
    pub fn get_engine_priority(&self, id: EngineId) -> Option<Priority> {
        self.inner.engines.get(id).map(|eng| eng.priority)
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

    /// Create a timer in stopped state. Call `start_timer` to begin.
    pub fn create_timer(&mut self, signal: SignalId) -> TimerId {
        self.inner.timer_central.create_timer(signal)
    }

    /// Start (or restart) a timer with the given interval and periodicity.
    pub fn start_timer(&mut self, id: TimerId, interval_ms: u64, periodic: bool) {
        self.inner
            .timer_central
            .start_timer(id, interval_ms, periodic);
    }

    /// Restart an existing timer in-place with new interval and periodicity.
    pub fn restart_timer(&mut self, id: TimerId, interval_ms: u64, periodic: bool) {
        self.inner
            .timer_central
            .restart_timer(id, interval_ms, periodic);
    }

    /// Cancel a timer. If `abort_signal` is true, also abort any pending signal.
    /// Matches C++ emTimer::Stop(abortSignal).
    pub fn cancel_timer(&mut self, id: TimerId, abort_signal: bool) {
        if let Some(sig) = self.inner.timer_central.cancel_timer(id, abort_signal) {
            self.abort(sig);
        }
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
    /// 1. emProcess pending signals (wake connected engines)
    /// 2. Check timers and fire their signals
    /// 3. emProcess timer-fired signals
    /// 4. Run engines from highest to lowest priority
    /// 5. After each engine, process any signals it fired (instant chaining)
    /// 6. Priority re-ascent: higher-priority engines woken mid-slice run in the same slice
    #[allow(clippy::too_many_arguments)]
    pub fn DoTimeSlice(
        &mut self,
        windows: &mut HashMap<WindowId, emWindow>,
        root_context: &Rc<crate::emContext::emContext>,
        framework_actions: &mut Vec<DeferredAction>,
        pending_inputs: &mut Vec<(WindowId, crate::emInput::emInputEvent)>,
        input_state: &mut crate::emInputState::emInputState,
        framework_clipboard: &std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>>,
        pending_actions: &std::rc::Rc<
            std::cell::RefCell<Vec<crate::emGUIFramework::DeferredAction>>,
        >,
    ) {
        self.inner.time_slice_counter += 1;
        self.inner.deadline = Instant::now() + TIME_SLICE_DURATION;
        let next_parity = self.inner.time_slice ^ 1;

        // Drain deferred engine removals queued when remove() was called
        // without a scheduler context (e.g. from test cleanup or non-Cycle
        // paths). Phase 3.5.A Task 6.2: per-window trees live inside their
        // emWindow, so drain from every window's tree.
        let mut pending_removals: Vec<EngineId> = Vec::new();
        for win in windows.values_mut() {
            pending_removals.append(&mut win.tree.pending_engine_removals);
        }
        for eid in pending_removals {
            self.remove_engine(eid);
        }

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
        // (which may wake higher-priority engines), re-ascend via
        // current_awake_idx bumped by wake_up_engine / set_engine_priority.
        let parity = self.inner.time_slice as usize;
        self.inner.current_awake_idx = Some((Priority::COUNT - 1) * 2 + parity);

        loop {
            // Increment clock and process pending signals
            self.inner.clock += 1;
            self.process_pending_signals();

            // Find next non-empty queue at or below current priority.
            // Re-read current_awake_idx each iteration since signal processing
            // may have bumped it upward.
            loop {
                let current_priority_idx = self.inner.current_awake_idx.unwrap();
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
                    self.inner.current_awake_idx = None;
                    return;
                }
                // Step down by one priority level (skip by 2 because
                // queues are interleaved: [pri0_even, pri0_odd, pri1_even, ...])
                self.inner.current_awake_idx = Some(current_priority_idx - 2);
            }

            // Take the first engine from this queue
            let current_priority_idx = self.inner.current_awake_idx.unwrap();
            let engine_id = self.inner.wake_queues[current_priority_idx].remove(0);

            // Mark engine as sleeping (it was removed from queue)
            if let Some(eng) = self.inner.engines.get_mut(engine_id) {
                eng.awake_state = -1;
            } else {
                continue; // emEngine was removed
            }

            // Extract behavior to avoid borrow conflict
            let mut behavior = match self.inner.engines.get_mut(engine_id) {
                Some(eng) => match eng.behavior.take() {
                    Some(b) => b,
                    None => {
                        continue;
                    }
                },
                None => continue,
            };

            // Resolve the engine's tree per its registered PanelScope.
            // Phase 3.5.A Task 6.2: every engine carries a PanelScope.
            let scope = self.inner.engine_scopes.get(engine_id).copied().expect(
                "engine has no PanelScope — register_engine always populates \
                 engine_scopes; missing entry indicates a scheduler bug",
            );
            let stay_awake = match scope {
                PanelScope::Framework => {
                    // Framework-scoped engines do not resolve to a specific
                    // tree. `ctx.tree` is `None`; the engine reaches any
                    // target tree via `take_tree` / `put_tree` on the window.
                    let mut ctx = EngineCtx {
                        scheduler: self,
                        tree: None,
                        windows,
                        root_context,
                        framework_actions,
                        pending_inputs,
                        input_state,
                        framework_clipboard,
                        engine_id,
                        pending_actions,
                    };
                    behavior.Cycle(&mut ctx)
                }
                PanelScope::Toplevel(wid) => {
                    // Take the window's tree onto the stack, hand it to
                    // Cycle as `ctx.tree`, put it back after. If the
                    // window no longer exists, the engine sleeps this
                    // slice: put the behavior back so it's retried next
                    // slice. Masking a missing window with a default tree
                    // would silently drop any work the engine did in
                    // Cycle.
                    let Some(win) = windows.get_mut(&wid) else {
                        if let Some(eng) = self.inner.engines.get_mut(engine_id) {
                            eng.behavior = Some(behavior);
                            eng.clock = self.inner.clock;
                        }
                        continue;
                    };
                    let mut local_tree = win.take_tree();
                    let result = {
                        let mut ctx = EngineCtx {
                            scheduler: self,
                            tree: Some(&mut local_tree),
                            windows,
                            root_context,
                            framework_actions,
                            pending_inputs,
                            input_state,
                            framework_clipboard,
                            engine_id,
                            pending_actions,
                        };
                        behavior.Cycle(&mut ctx)
                    };
                    if let Some(win) = windows.get_mut(&wid) {
                        win.put_tree(local_tree);
                    } else {
                        drop(local_tree);
                    }
                    result
                }
                PanelScope::SubView {
                    window_id,
                    outer_panel_id: _,
                } => {
                    // Hand the target window's outer tree through unchanged;
                    // Cycle bodies reach the inner sub_view / sub_tree via
                    // `ctx.tree.panels.get_mut(outer_panel_id).behavior
                    // .as_sub_view_panel_mut()` (see
                    // `PanelScope::resolve_view` SubView arm and
                    // `UpdateEngineClass`/`VisitingVAEngineClass` SubView
                    // branches). The scheduler does NOT pre-walk because
                    // the inner sub_tree/sub_view need to be reached
                    // together: a pre-walk that takes the outer behavior
                    // off would leave the Cycle body unable to see the
                    // sub_view. If the window is gone, the engine sleeps
                    // this slice: put the behavior back and continue.
                    let Some(win) = windows.get_mut(&window_id) else {
                        if let Some(eng) = self.inner.engines.get_mut(engine_id) {
                            eng.behavior = Some(behavior);
                            eng.clock = self.inner.clock;
                        }
                        continue;
                    };
                    let mut local_tree = win.take_tree();
                    let result = {
                        let mut ctx = EngineCtx {
                            scheduler: self,
                            tree: Some(&mut local_tree),
                            windows,
                            root_context,
                            framework_actions,
                            pending_inputs,
                            input_state,
                            framework_clipboard,
                            engine_id,
                            pending_actions,
                        };
                        behavior.Cycle(&mut ctx)
                    };
                    if let Some(win) = windows.get_mut(&window_id) {
                        win.put_tree(local_tree);
                    } else {
                        drop(local_tree);
                    }
                    result
                }
            };

            // Reinsert behavior and update engine state
            if let Some(eng) = self.inner.engines.get_mut(engine_id) {
                eng.behavior = Some(behavior);
                eng.clock = self.inner.clock;

                if stay_awake && eng.awake_state < 0 {
                    // emEngine wants to stay awake and wasn't re-woken during cycle.
                    // Queue for next time slice (not current, to prevent infinite loop).
                    eng.awake_state = next_parity;
                    let queue_idx = (eng.priority as usize) * 2 + (next_parity as usize);
                    self.inner.wake_queues[queue_idx].push(engine_id);
                }
            }

            // After engine cycle, process signals it may have fired.
            // C++ increments Clock at the top of its for(;;) loop, so signals
            // fired by an engine get a new clock value in the next iteration.
            // wake_up_engine may bump current_awake_idx for re-ascent.
        }
    }

    /// Check if the current time slice has exceeded its deadline.
    pub fn IsTimeSliceAtEnd(&self) -> bool {
        Instant::now() >= self.inner.deadline
    }

    /// Current time slice counter (incremented once per `do_time_slice` call).
    pub fn GetTimeSliceCounter(&self) -> u64 {
        self.inner.time_slice_counter
    }

    /// Blocking run loop: calls `do_time_slice` repeatedly until
    /// `initiate_termination` is called.
    ///
    /// Port of C++ `emStandardScheduler::Run`.
    pub fn run(&mut self) {
        let mut windows = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let framework_clipboard: std::cell::RefCell<
            Option<Box<dyn crate::emClipboard::emClipboard>>,
        > = std::cell::RefCell::new(None);
        let pending_actions: std::rc::Rc<
            std::cell::RefCell<Vec<crate::emGUIFramework::DeferredAction>>,
        > = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        self.terminated = false;
        while !self.terminated {
            self.DoTimeSlice(
                &mut windows,
                &root_context,
                &mut framework_actions,
                &mut pending_inputs,
                &mut input_state,
                &framework_clipboard,
                &pending_actions,
            );
        }
    }

    /// Check if any engines are currently awake (queued in any wake list).
    pub fn has_awake_engines(&self) -> bool {
        self.inner.wake_queues.iter().any(|q| !q.is_empty())
    }

    /// True when the scheduler has no work pending: no queued signals,
    /// no wake queue entries. Does NOT consider view animators, pending
    /// per-panel notices, or AutoExpand work — those live outside the
    /// scheduler. Callers (e.g., emCtrlSocket's `wait_idle`) compose
    /// this check with view-side predicates.
    pub fn is_idle(&self) -> bool {
        self.inner.pending_signals.is_empty()
            && self.inner.wake_queues.iter().all(|q| q.is_empty())
    }

    /// Signal the scheduler to stop after the current time slice.
    ///
    /// Port of C++ `emScheduler::InitiateTermination`.
    pub fn InitiateTermination(&mut self) {
        self.terminated = true;
    }

    /// Whether termination has been initiated.
    pub fn is_terminated(&self) -> bool {
        self.terminated
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

#[cfg(any(test, feature = "test-support"))]
impl EngineScheduler {
    /// Test helper: mark all pending signals as processed without waking
    /// engines. Used by unit tests that fire signals but do not run a time
    /// slice, so the Drop-time debug assert does not panic.
    /// Phase-3 B3.4c.
    pub fn clear_pending_for_tests(&mut self) {
        let pending = std::mem::take(&mut self.inner.pending_signals);
        for id in pending {
            if let Some(sig) = self.inner.signals.get_mut(id) {
                sig.pending = false;
            }
        }
    }

    /// Attach a first-cycle slice probe to a registered `PanelCycleEngine`.
    /// Used by SP4.5-FIX-1 timing fixtures (Tasks 5-7).
    pub fn attach_first_cycle_probe(
        &mut self,
        eid: super::emEngine::EngineId,
        captured_slice: std::rc::Rc<std::cell::Cell<Option<u64>>>,
    ) {
        let Some(eng) = self.inner.engines.get_mut(eid) else {
            return;
        };
        let Some(behavior) = eng.behavior.as_mut() else {
            return;
        };
        let Some(pce) = (behavior.as_mut() as &mut dyn std::any::Any)
            .downcast_mut::<crate::emPanelCycleEngine::PanelCycleEngine>()
        else {
            panic!("attach_first_cycle_probe: engine {eid:?} is not a PanelCycleEngine");
        };
        pce.first_cycle_probe =
            Some(crate::emPanelCycleEngine::PanelCycleEngineFirstCycleProbe { captured_slice });
    }
}

impl Drop for EngineScheduler {
    fn drop(&mut self) {
        debug_assert!(
            self.inner.engines.is_empty(),
            "EngineScheduler dropped with remaining engines"
        );
        debug_assert!(
            self.inner.pending_signals.is_empty(),
            "EngineScheduler dropped with pending signals"
        );
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

    fn slice(sched: &mut EngineScheduler) {
        let mut windows = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fc: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let __pa: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        sched.DoTimeSlice(
            &mut windows,
            &root_context,
            &mut framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &__pa,
        );
    }

    struct CountingEngine {
        count: Rc<RefCell<u32>>,
    }

    impl emEngine for CountingEngine {
        fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
            *self.count.borrow_mut() += 1;
            false // sleep after one cycle
        }
    }

    struct PollingEngine {
        remaining: u32,
        count: Rc<RefCell<u32>>,
    }

    impl emEngine for PollingEngine {
        fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
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
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        sched.wake_up(id);
        slice(&mut sched);
        assert_eq!(*count.borrow(), 1);
        // emEngine returned false, should not run again
        slice(&mut sched);
        assert_eq!(*count.borrow(), 1);
        sched.remove_engine(id);
    }

    #[test]
    fn polling_engine_runs_multiple_slices() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let id = sched.register_engine(
            Box::new(PollingEngine {
                remaining: 3,
                count: Rc::clone(&count),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        sched.wake_up(id);
        slice(&mut sched);
        slice(&mut sched);
        slice(&mut sched);
        assert_eq!(*count.borrow(), 3);
        // Should be asleep now
        slice(&mut sched);
        assert_eq!(*count.borrow(), 3);
        sched.remove_engine(id);
    }

    #[test]
    fn signal_wakes_engine() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let sig = sched.create_signal();
        let eng = sched.register_engine(
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
            Priority::High,
            PanelScope::Framework,
        );
        sched.connect(sig, eng);
        // emEngine is sleeping, nothing should run
        slice(&mut sched);
        assert_eq!(*count.borrow(), 0);
        // Fire signal and run
        sched.fire(sig);
        slice(&mut sched);
        assert_eq!(*count.borrow(), 1);
        sched.remove_engine(eng);
    }

    #[test]
    fn signal_abort() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let sig = sched.create_signal();
        let eng = sched.register_engine(
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        sched.connect(sig, eng);
        sched.fire(sig);
        sched.abort(sig);
        slice(&mut sched);
        assert_eq!(*count.borrow(), 0);
        sched.remove_engine(eng);
    }

    #[test]
    fn priority_ordering() {
        let mut sched = EngineScheduler::new();
        let order = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct OrderEngine {
            label: &'static str,
            order: Rc<RefCell<Vec<&'static str>>>,
        }
        impl emEngine for OrderEngine {
            fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.order.borrow_mut().push(self.label);
                false
            }
        }

        let low = sched.register_engine(
            Box::new(OrderEngine {
                label: "low",
                order: Rc::clone(&order),
            }),
            Priority::Low,
            PanelScope::Framework,
        );
        let high = sched.register_engine(
            Box::new(OrderEngine {
                label: "high",
                order: Rc::clone(&order),
            }),
            Priority::VeryHigh,
            PanelScope::Framework,
        );
        sched.wake_up(low);
        sched.wake_up(high);
        slice(&mut sched);
        let executed = order.borrow();
        assert_eq!(executed[0], "high");
        assert_eq!(executed[1], "low");
        drop(executed);
        sched.remove_engine(low);
        sched.remove_engine(high);
    }

    #[test]
    fn is_signaled_distinguishes_signals() {
        // emEngine connected to two signals should be able to distinguish which fired.
        let mut sched = EngineScheduler::new();
        let sig_a = sched.create_signal();
        let sig_b = sched.create_signal();

        struct CheckSignalEngine {
            sig_a: SignalId,
            sig_b: SignalId,
            a_fired: Rc<RefCell<bool>>,
            b_fired: Rc<RefCell<bool>>,
        }
        impl emEngine for CheckSignalEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                *self.a_fired.borrow_mut() = ctx.IsSignaled(self.sig_a);
                *self.b_fired.borrow_mut() = ctx.IsSignaled(self.sig_b);
                false
            }
        }

        let a_fired = Rc::new(RefCell::new(false));
        let b_fired = Rc::new(RefCell::new(false));
        let eng = sched.register_engine(
            Box::new(CheckSignalEngine {
                sig_a,
                sig_b,
                a_fired: Rc::clone(&a_fired),
                b_fired: Rc::clone(&b_fired),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        sched.connect(sig_a, eng);
        sched.connect(sig_b, eng);

        // Fire only signal A
        sched.fire(sig_a);
        slice(&mut sched);
        assert!(*a_fired.borrow());
        assert!(!*b_fired.borrow());
        sched.remove_engine(eng);
    }

    #[test]
    fn refcounted_connections() {
        let mut sched = EngineScheduler::new();
        let sig = sched.create_signal();
        let eng = sched.register_engine(
            Box::new(CountingEngine {
                count: Rc::new(RefCell::new(0)),
            }),
            Priority::Medium,
            PanelScope::Framework,
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
        sched.remove_engine(eng);
    }

    #[test]
    fn set_engine_priority() {
        let mut sched = EngineScheduler::new();
        let order = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct OrderEngine {
            label: &'static str,
            order: Rc<RefCell<Vec<&'static str>>>,
        }
        impl emEngine for OrderEngine {
            fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.order.borrow_mut().push(self.label);
                false
            }
        }

        let eng_a = sched.register_engine(
            Box::new(OrderEngine {
                label: "A",
                order: Rc::clone(&order),
            }),
            Priority::Low,
            PanelScope::Framework,
        );
        let eng_b = sched.register_engine(
            Box::new(OrderEngine {
                label: "B",
                order: Rc::clone(&order),
            }),
            Priority::High,
            PanelScope::Framework,
        );

        // Promote A to VeryHigh — it should run before B
        sched.set_engine_priority(eng_a, Priority::VeryHigh);
        sched.wake_up(eng_a);
        sched.wake_up(eng_b);
        slice(&mut sched);

        let executed = order.borrow();
        assert_eq!(executed[0], "A");
        assert_eq!(executed[1], "B");
        drop(executed);
        sched.remove_engine(eng_a);
        sched.remove_engine(eng_b);
    }

    #[test]
    fn instant_signal_chaining() {
        // emEngine A fires a signal during its cycle that wakes emEngine B.
        // emEngine B should run in the SAME time slice.
        let mut sched = EngineScheduler::new();
        let sig = sched.create_signal();
        let log = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct FiringEngine {
            sig: SignalId,
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl emEngine for FiringEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("A");
                ctx.fire(self.sig);
                false
            }
        }

        struct ReceivingEngine {
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl emEngine for ReceivingEngine {
            fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("B");
                false
            }
        }

        let eng_b = sched.register_engine(
            Box::new(ReceivingEngine {
                log: Rc::clone(&log),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        sched.connect(sig, eng_b);

        let _eng_a = sched.register_engine(
            Box::new(FiringEngine {
                sig,
                log: Rc::clone(&log),
            }),
            Priority::High,
            PanelScope::Framework,
        );
        sched.wake_up(_eng_a);

        slice(&mut sched);

        let executed = log.borrow();
        assert_eq!(*executed, vec!["A", "B"]);
        drop(executed);
        sched.remove_engine(eng_b);
        sched.remove_engine(_eng_a);
    }

    #[test]
    fn reascent_to_higher_priority() {
        // Low-priority engine fires signal that wakes high-priority engine.
        // C++ re-ascends: high-priority engine runs in the SAME time slice.
        let mut sched = EngineScheduler::new();
        let sig = sched.create_signal();
        let log = Rc::new(RefCell::new(Vec::<&str>::new()));

        struct FiringEngine {
            sig: SignalId,
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl emEngine for FiringEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("low_fires");
                ctx.fire(self.sig);
                false
            }
        }

        struct HighEngine {
            log: Rc<RefCell<Vec<&'static str>>>,
        }
        impl emEngine for HighEngine {
            fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
                self.log.borrow_mut().push("high_runs");
                false
            }
        }

        let eng_high = sched.register_engine(
            Box::new(HighEngine {
                log: Rc::clone(&log),
            }),
            Priority::VeryHigh,
            PanelScope::Framework,
        );
        sched.connect(sig, eng_high);

        let eng_low = sched.register_engine(
            Box::new(FiringEngine {
                sig,
                log: Rc::clone(&log),
            }),
            Priority::VeryLow,
            PanelScope::Framework,
        );
        sched.wake_up(eng_low);

        slice(&mut sched);

        // Both run in the same slice: low fires, then high re-ascends and runs
        let executed = log.borrow();
        assert_eq!(*executed, vec!["low_fires", "high_runs"]);
        drop(executed);
        sched.remove_engine(eng_high);
        sched.remove_engine(eng_low);
    }

    // ── Phase 1.75 Task 2 — TreeLocation dispatch walk (REMOVED) ────
    //
    // The depth-2 `TreeLocation::SubView(outer, SubView(inner, Outer))`
    // test (task2_dispatch_walks_depth_2_subview_location) was removed
    // in Phase 3.5.A Task 6.2 when `PanelScope::SubView` became flat
    // (single level, no `rest` chain — see emPanelScope.rs). Depth-2
    // nesting has no production call-sites; `emSubViewPanel::new` only
    // ever registers a single level. If multi-level nesting becomes
    // necessary, reintroduce the test alongside a `rest` chain in
    // PanelScope.

    // ── Phase 3.5.A Task 6 — scope-based dispatch ───────────────────

    /// Framework-scoped engine dispatch via `register_engine`.
    /// Proves the scheduler's scope branch runs Cycle and does not touch
    /// the outer tree (it is passed through unchanged).
    #[test]
    fn spike_framework_dispatch_via_scope() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let id = sched.register_engine(
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        // Scope map populated for this engine.
        assert!(sched.inner.engine_scopes.contains_key(id));
        sched.wake_up(id);
        slice(&mut sched);
        assert_eq!(*count.borrow(), 1);
        sched.remove_engine(id);
    }

    /// Toplevel-scoped engine dispatch via `register_engine`.
    /// Builds a headless `emWindow` via `new_popup_pending`, registers a
    /// counting engine scoped to its `WindowId`, runs one slice, and
    /// verifies (a) Cycle ran and (b) the window's tree is restored
    /// post-Cycle (not left as the `Default` sentinel).
    #[test]
    fn spike_toplevel_dispatch_via_scope() {
        use crate::emWindow::WindowFlags;

        let mut sched = EngineScheduler::new();
        let close_sig = sched.create_signal();
        let flags_sig = sched.create_signal();
        let focus_sig = sched.create_signal();
        let geom_sig = sched.create_signal();
        let win = crate::emWindow::emWindow::new_popup_pending(
            crate::emContext::emContext::NewRoot(),
            WindowFlags::empty(),
            "spike_toplevel".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        let wid = WindowId::dummy();
        let mut windows: HashMap<WindowId, crate::emWindow::emWindow> = HashMap::new();
        windows.insert(wid, win);

        // Install a populated tree in the window so we can assert
        // that put_tree restored the same (non-sentinel) tree.
        {
            let mut populated = PanelTree::new();
            let _r = populated.create_root_deferred_view("win_root");
            let w = windows.get_mut(&wid).unwrap();
            // take_tree to drop the pending-state default tree, then put
            // our populated tree.
            let _ = w.take_tree();
            w.put_tree(populated);
        }

        let count = Rc::new(RefCell::new(0u32));
        let id = sched.register_engine(
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
            Priority::Medium,
            PanelScope::Toplevel(wid),
        );
        sched.wake_up(id);

        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fc: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let __pa: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        sched.DoTimeSlice(
            &mut windows,
            &root_context,
            &mut framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &__pa,
        );

        assert_eq!(*count.borrow(), 1, "Cycle must have run once");
        // Window's tree must be restored (the populated tree we put in).
        let w = windows.get_mut(&wid).unwrap();
        assert!(
            w.tree.GetRootPanel().is_some(),
            "window tree must be restored with the populated root after Cycle"
        );

        sched.remove_engine(id);
        // Drain the window so its tree's engines (if any) go away before
        // the scheduler's Drop-assert fires.
        let mut w = windows.remove(&wid).unwrap();
        let t = w.take_tree();
        drop(t);
    }

    /// Toplevel-scoped engine whose WindowId is NOT present in the windows
    /// HashMap. The engine must sleep this slice: Cycle must not run, and
    /// behavior must be put back so a future slice can retry once the
    /// window reappears. Mirrors the legacy "missing location" bail-out.
    #[test]
    fn spike_toplevel_dispatch_missing_window_sleeps() {
        let mut sched = EngineScheduler::new();
        let count = Rc::new(RefCell::new(0u32));
        let missing_wid = WindowId::dummy();
        let id = sched.register_engine(
            Box::new(CountingEngine {
                count: Rc::clone(&count),
            }),
            Priority::Medium,
            PanelScope::Toplevel(missing_wid),
        );
        sched.wake_up(id);

        // Empty windows map — the registered WindowId is absent.
        let mut windows: HashMap<WindowId, crate::emWindow::emWindow> = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fc: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let __pa: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        sched.DoTimeSlice(
            &mut windows,
            &root_context,
            &mut framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &__pa,
        );

        assert_eq!(
            *count.borrow(),
            0,
            "Cycle must NOT have run when Toplevel window is missing"
        );
        // Behavior must be restored in the engine slot so a future slice
        // can retry once the window reappears.
        let eng = sched
            .inner
            .engines
            .get(id)
            .expect("engine slot must still exist after missing-window bail");
        assert!(
            eng.behavior.is_some(),
            "behavior must be put back after missing-window bail"
        );

        sched.remove_engine(id);
    }

    // ── Phase 3.5 Task 3 tests ──────────────────────────────────────

    #[test]
    fn scheduler_allocates_monotonic_dialog_ids() {
        let mut s = EngineScheduler::new();
        let a = s.allocate_dialog_id();
        let b = s.allocate_dialog_id();
        let c = s.allocate_dialog_id();
        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
        assert_eq!(c.0, 2);
    }

    #[test]
    fn engines_for_scope_filters_correctly() {
        struct Noop;
        impl crate::emEngine::emEngine for Noop {
            fn Cycle(&mut self, _: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }
        let mut s = EngineScheduler::new();
        let wid = WindowId::dummy();
        let e1 = s.register_engine(Box::new(Noop), Priority::Medium, PanelScope::Framework);
        let e2 = s.register_engine(Box::new(Noop), Priority::Medium, PanelScope::Toplevel(wid));
        let e3 = s.register_engine(Box::new(Noop), Priority::Medium, PanelScope::Toplevel(wid));

        let fw = s.engines_for_scope(PanelScope::Framework);
        let tl = s.engines_for_scope(PanelScope::Toplevel(wid));
        assert_eq!(fw, vec![e1]);
        assert_eq!(tl.len(), 2);
        assert!(tl.contains(&e2));
        assert!(tl.contains(&e3));

        s.remove_engine(e1);
        s.remove_engine(e2);
        s.remove_engine(e3);
    }

    #[test]
    fn is_idle_true_for_fresh_scheduler() {
        let s = EngineScheduler::new();
        assert!(s.is_idle());
    }

    #[test]
    fn is_idle_false_with_pending_signal() {
        let mut s = EngineScheduler::new();
        let sig = s.create_signal();
        s.fire(sig);
        assert!(!s.is_idle());
        // Clean up so Drop-asserts don't panic.
        s.abort(sig);
        s.remove_signal(sig);
    }

    #[test]
    fn is_idle_false_with_wake_queue_entry() {
        let mut s = EngineScheduler::new();
        let id = s.register_engine(
            Box::new(CountingEngine {
                count: Rc::new(RefCell::new(0)),
            }),
            Priority::Medium,
            PanelScope::Framework,
        );
        s.wake_up(id);
        assert!(!s.is_idle());
        s.remove_engine(id);
    }

    #[test]
    fn abort_all_pending_clears_queue_and_allows_clean_drop() {
        let mut sched = EngineScheduler::new();
        let s1 = sched.create_signal();
        let s2 = sched.create_signal();
        sched.fire(s1);
        sched.fire(s2);
        sched.abort_all_pending();
        sched.remove_signal(s1);
        sched.remove_signal(s2);
        // sched drops here — pending_signals empty, no assertion fires.
    }
}
