use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slotmap::{SecondaryMap, SlotMap};
use winit::window::WindowId;

use super::emEngine::{emEngine, EngineData, EngineId, Priority, TreeLocation};
use super::emEngineCtx::{DeferredAction, EngineCtx};
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
    /// Per-engine `TreeLocation` — tells `DoTimeSlice` how to reach the
    /// engine's panel tree from the outer tree. Populated on
    /// `register_engine`, cleared on `remove_engine`. Phase 1.75 Task 2.
    pub engine_locations: SecondaryMap<EngineId, TreeLocation>,
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

/// Resolve an engine's `TreeLocation` to a `&mut PanelTree` and invoke `f`
/// with it. For `Outer`, `f` runs on the outer tree directly. For `SubView`,
/// the owner's `emSubViewPanel` behavior is taken from the outer tree, the
/// walk recurses through `emSubViewPanel::sub_tree`, then behaviors are put
/// back in reverse order after `f` returns.
///
/// Mirrors the plan's `resolve`/`ResolvedTree` pseudocode. Uses recursion
/// over the `TreeLocation` chain rather than an explicit stack — the
/// take/put pairing is naturally lexically scoped by each recursive call.
///
/// Invariants (tightened in Phase 1.75 Task 3 when `SubView` dispatch went
/// live): every `TreeLocation::SubView { outer_panel_id, .. }` in the
/// `engine_locations` map resolves to a live outer-tree panel whose behavior
/// is an `emSubViewPanel`. Both `take_behavior` and `as_sub_view_panel_mut`
/// are therefore required to succeed; failure is a hard bug (stale
/// registration vs. tree mutation) and is reported via `panic!`/`expect`.
///
/// Phase 1.75 Task 2 (added); Task 3 (invariant tightened).
///
/// Known unaddressed concern (deferred to post-phase cleanup): if `f`
/// itself panics, the `behavior` local is not restored to `outer_panel_id`
/// before unwind — the panel is left behavior-less. The codebase treats
/// engine `Cycle` panics as fatal, so this leak is benign in practice; a
/// `scopeguard`-style RAII fix would make the helper unwind-safe but is
/// orthogonal to the scheduler-dispatch rewrite and is filed in the ledger.
fn dispatch_with_resolved_tree<R>(
    tree: &mut PanelTree,
    loc: &TreeLocation,
    f: impl FnOnce(&mut PanelTree) -> R,
) -> R {
    match loc {
        TreeLocation::Outer => f(tree),
        TreeLocation::SubView {
            outer_panel_id,
            rest,
        } => {
            // Take the owner's behavior — it must downcast to `emSubViewPanel`.
            // Both steps are required invariants (see helper-level doc); a
            // failure means `engine_locations` has a stale entry inconsistent
            // with the outer tree, which is a hard bug.
            let Some(mut behavior) = tree.take_behavior(*outer_panel_id) else {
                panic!(
                    "dispatch_with_resolved_tree: outer panel {:?} missing from outer tree",
                    outer_panel_id
                );
            };
            let result = {
                let sv = behavior.as_sub_view_panel_mut().expect(
                    "dispatch_with_resolved_tree: outer panel behavior is not an emSubViewPanel",
                );
                dispatch_with_resolved_tree(sv.sub_tree_mut(), rest, f)
            };
            tree.put_behavior(*outer_panel_id, behavior);
            result
        }
    }
}

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
}

impl EngineScheduler {
    pub fn new() -> Self {
        Self {
            terminated: false,
            inner: EngineCtxInner {
                signals: SlotMap::with_key(),
                engines: SlotMap::with_key(),
                engine_locations: SecondaryMap::new(),
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
    /// `tree_location` tells `DoTimeSlice` how to reach this engine's tree from
    /// the outer `PanelTree` (Phase 1.75 Task 2). Engines that live directly in
    /// the outer tree pass `TreeLocation::Outer`; engines that live inside an
    /// `emSubViewPanel::sub_tree` pass
    /// `TreeLocation::SubView { outer_panel_id, rest }` — `rest` supports
    /// arbitrary nesting.
    pub fn register_engine(
        &mut self,
        behavior: Box<dyn emEngine>,
        priority: Priority,
        tree_location: TreeLocation,
    ) -> EngineId {
        let id = self.inner.engines.insert(EngineData {
            priority,
            awake_state: -1, // sleeping
            behavior: Some(behavior),
            clock: self.inner.clock,
        });
        self.inner.engine_locations.insert(id, tree_location);
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
        self.inner.engine_locations.remove(id);
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
        tree: &mut PanelTree,
        windows: &mut HashMap<WindowId, emWindow>,
        root_context: &Rc<crate::emContext::emContext>,
        framework_actions: &mut Vec<DeferredAction>,
        pending_inputs: &mut Vec<(WindowId, crate::emInput::emInputEvent)>,
        input_state: &mut crate::emInputState::emInputState,
        framework_clipboard: &std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>>,
    ) {
        self.inner.time_slice_counter += 1;
        self.inner.deadline = Instant::now() + TIME_SLICE_DURATION;
        let next_parity = self.inner.time_slice ^ 1;

        // Drain deferred engine removals queued when remove() was called without
        // a scheduler context (e.g. from test cleanup or non-Cycle paths).
        let pending_removals: Vec<_> = tree.pending_engine_removals.drain(..).collect();
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
                    None => continue,
                },
                None => continue,
            };

            // Resolve the engine's tree location — the outer tree for
            // `TreeLocation::Outer`, or a nested `sub_tree` reached by
            // taking panel behaviors along the `SubView` chain. See
            // `dispatch_with_resolved_tree` for the take/put walk.
            //
            // Clone the TreeLocation (shallow chain of PanelId+Box) so we
            // don't alias `self.inner` during the walk.
            let tree_location = self.inner.engine_locations.get(engine_id).cloned().expect(
                "engine has no TreeLocation — register_engine always populates \
                     engine_locations; missing entry indicates a scheduler bug",
            );

            // Call Cycle with context. `behavior` has been detached from the
            // engine slot, so `self` is re-borrowable for ctx construction.
            // `framework_actions` comes in from the caller (`App` owns it per
            // spec §3.1), so ctx borrows that parameter directly — no
            // take/restore dance needed.
            let stay_awake = dispatch_with_resolved_tree(tree, &tree_location, |resolved_tree| {
                let mut ctx = EngineCtx {
                    scheduler: self,
                    tree: resolved_tree,
                    windows,
                    root_context,
                    framework_actions,
                    pending_inputs,
                    input_state,
                    framework_clipboard,
                    engine_id,
                };
                behavior.Cycle(&mut ctx)
            });

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
        let mut tree = PanelTree::new();
        let mut windows = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let framework_clipboard: std::cell::RefCell<
            Option<Box<dyn crate::emClipboard::emClipboard>>,
        > = std::cell::RefCell::new(None);
        self.terminated = false;
        while !self.terminated {
            self.DoTimeSlice(
                &mut tree,
                &mut windows,
                &root_context,
                &mut framework_actions,
                &mut pending_inputs,
                &mut input_state,
                &framework_clipboard,
            );
        }
    }

    /// Check if any engines are currently awake (queued in any wake list).
    pub fn has_awake_engines(&self) -> bool {
        self.inner.wake_queues.iter().any(|q| !q.is_empty())
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
        let mut tree = PanelTree::new();
        let mut windows = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fc: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        sched.DoTimeSlice(
            &mut tree,
            &mut windows,
            &root_context,
            &mut framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
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
            TreeLocation::Outer,
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
            TreeLocation::Outer,
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
            TreeLocation::Outer,
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
            TreeLocation::Outer,
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
            TreeLocation::Outer,
        );
        let high = sched.register_engine(
            Box::new(OrderEngine {
                label: "high",
                order: Rc::clone(&order),
            }),
            Priority::VeryHigh,
            TreeLocation::Outer,
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
            TreeLocation::Outer,
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
            TreeLocation::Outer,
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
            TreeLocation::Outer,
        );
        let eng_b = sched.register_engine(
            Box::new(OrderEngine {
                label: "B",
                order: Rc::clone(&order),
            }),
            Priority::High,
            TreeLocation::Outer,
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
            TreeLocation::Outer,
        );
        sched.connect(sig, eng_b);

        let _eng_a = sched.register_engine(
            Box::new(FiringEngine {
                sig,
                log: Rc::clone(&log),
            }),
            Priority::High,
            TreeLocation::Outer,
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
            TreeLocation::Outer,
        );
        sched.connect(sig, eng_high);

        let eng_low = sched.register_engine(
            Box::new(FiringEngine {
                sig,
                log: Rc::clone(&log),
            }),
            Priority::VeryLow,
            TreeLocation::Outer,
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

    // ── Phase 1.75 Task 2 — TreeLocation dispatch walk ──────────────

    /// Depth-2 nested `TreeLocation::SubView` dispatch.
    ///
    /// Builds: outer tree → `emSubViewPanel` behavior installed at a child
    /// panel → that panel's sub_tree contains another child panel with a
    /// second `emSubViewPanel` behavior → innermost sub_tree.
    ///
    /// Registers a probe engine on the outer scheduler tagged with
    /// `TreeLocation::SubView(outer_child, SubView(inner_child, Outer))`,
    /// wakes it, and runs one slice. The probe's `Cycle` receives
    /// `ctx.tree` and records its raw pointer; the test compares that
    /// pointer to a snapshot of the innermost `sub_tree_mut` pointer
    /// captured before dispatch. Equality proves the take/put walk
    /// resolved through both `as_sub_view_panel_mut` hops.
    ///
    /// Scope: shape-only. Does NOT yet migrate the sub-views' own engines
    /// to the outer scheduler — that is Task 3. Here we only prove the
    /// dispatch walk reaches the correct tree.
    #[test]
    fn task2_dispatch_walks_depth_2_subview_location() {
        use crate::emSubViewPanel::emSubViewPanel;

        // ── Outer scheduler + tree ──
        let mut sched = EngineScheduler::new();
        let mut tree = PanelTree::new();
        let mut windows = HashMap::new();
        let root_context = crate::emContext::emContext::NewRoot();
        let mut framework_actions: Vec<DeferredAction> = Vec::new();
        let framework_clipboard: std::cell::RefCell<
            Option<Box<dyn crate::emClipboard::emClipboard>>,
        > = std::cell::RefCell::new(None);

        let outer_root = tree.create_root("outer_root", false);
        let outer_child = tree.create_child(outer_root, "outer_sv", None);

        // Phase 1.75 Task 3: `emSubViewPanel::new` needs outer_panel_id + a
        // SchedCtx so it can register its sub-view engines on the OUTER
        // scheduler with `SubView(outer_panel_id, Outer)`.
        let mut outer_sv = {
            let mut fw: Vec<DeferredAction> = Vec::new();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut sched,
                framework_actions: &mut fw,
                root_context: &root_context,
                framework_clipboard: &framework_clipboard,
                current_engine: None,
            };
            emSubViewPanel::new(root_context.clone(), outer_child, &mut sc)
        };
        // Create the inner sub-view inside outer_sv.sub_tree.
        let outer_sub_root = outer_sv.sub_root();
        let inner_child = outer_sv
            .sub_tree_mut()
            .create_child(outer_sub_root, "inner_sv", None);
        let inner_sv = {
            let mut fw: Vec<DeferredAction> = Vec::new();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler: &mut sched,
                framework_actions: &mut fw,
                root_context: &root_context,
                framework_clipboard: &framework_clipboard,
                current_engine: None,
            };
            emSubViewPanel::new(root_context.clone(), inner_child, &mut sc)
        };
        outer_sv
            .sub_tree_mut()
            .set_behavior(inner_child, Box::new(inner_sv));

        // Snapshot innermost sub_tree pointer BEFORE handing outer_sv to the
        // outer tree. Traverse: outer_sv.sub_tree → inner_child.behavior →
        // inner_sv.sub_tree. We extract via the same take/put dance the
        // dispatcher performs.
        let innermost_ptr: *mut PanelTree = {
            let mut inner_beh = outer_sv
                .sub_tree_mut()
                .take_behavior(inner_child)
                .expect("inner_child has behavior");
            let ptr = inner_beh
                .as_sub_view_panel_mut()
                .expect("inner behavior is emSubViewPanel")
                .sub_tree_mut() as *mut PanelTree;
            outer_sv.sub_tree_mut().put_behavior(inner_child, inner_beh);
            ptr
        };

        tree.set_behavior(outer_child, Box::new(outer_sv));

        // ── Probe engine that records the `ctx.tree` pointer it receives. ──
        struct ProbePointerEngine {
            captured: Rc<RefCell<Option<*mut PanelTree>>>,
        }
        impl emEngine for ProbePointerEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                *self.captured.borrow_mut() = Some(ctx.tree as *mut PanelTree);
                false
            }
        }

        let captured: Rc<RefCell<Option<*mut PanelTree>>> = Rc::new(RefCell::new(None));
        let probe = sched.register_engine(
            Box::new(ProbePointerEngine {
                captured: Rc::clone(&captured),
            }),
            Priority::Medium,
            TreeLocation::SubView {
                outer_panel_id: outer_child,
                rest: Box::new(TreeLocation::SubView {
                    outer_panel_id: inner_child,
                    rest: Box::new(TreeLocation::Outer),
                }),
            },
        );
        sched.wake_up(probe);

        let mut pending_inputs: Vec<(WindowId, crate::emInput::emInputEvent)> = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        sched.DoTimeSlice(
            &mut tree,
            &mut windows,
            &root_context,
            &mut framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &framework_clipboard,
        );

        let got = captured.borrow().expect("probe ran");
        assert_eq!(
            got, innermost_ptr,
            "depth-2 SubView dispatch must resolve to the innermost sub_tree"
        );

        // Teardown: remove the probe engine so the scheduler Drop assert is
        // satisfied.
        sched.remove_engine(probe);

        // Phase 1.75 Task 3: nested sub-view engines (update_engine,
        // visiting_va, PanelCycleEngine adapters for both sub_trees' root
        // panels) all live on `sched` now. Walk the nested structure and
        // deregister each: the outer sub-view's sub_tree and view engines,
        // then the inner sub-view's. This replaces the Task-2
        // `std::mem::forget` test-only concession.
        let mut outer_beh = tree.take_behavior(outer_child).unwrap();
        {
            let outer_sv_ref = outer_beh.as_sub_view_panel_mut().unwrap();
            // Drain the inner sub-view first.
            if let Some(mut inner_beh) = outer_sv_ref.sub_tree_mut().take_behavior(inner_child) {
                {
                    let inner_sv = inner_beh.as_sub_view_panel_mut().unwrap();
                    let inner_sub_root = inner_sv.sub_root();
                    inner_sv
                        .sub_tree_mut()
                        .remove(inner_sub_root, Some(&mut sched));
                    let v = inner_sv.sub_view_mut();
                    if let Some(eid) = v.update_engine_id.take() {
                        sched.remove_engine(eid);
                    }
                    if let Some(eid) = v.visiting_va_engine_id.take() {
                        sched.remove_engine(eid);
                    }
                    if let Some(sig) = v.EOISignal.take() {
                        sched.remove_signal(sig);
                    }
                }
                drop(inner_beh);
            }
            // Now drain the outer sub-view.
            let outer_sub_root = outer_sv_ref.sub_root();
            outer_sv_ref
                .sub_tree_mut()
                .remove(outer_sub_root, Some(&mut sched));
            let v = outer_sv_ref.sub_view_mut();
            if let Some(eid) = v.update_engine_id.take() {
                sched.remove_engine(eid);
            }
            if let Some(eid) = v.visiting_va_engine_id.take() {
                sched.remove_engine(eid);
            }
            if let Some(sig) = v.EOISignal.take() {
                sched.remove_signal(sig);
            }
        }
        drop(outer_beh);
        // Remove outer panels' adapter engines.
        tree.remove(outer_child, Some(&mut sched));
        tree.remove(outer_root, Some(&mut sched));
    }
}
