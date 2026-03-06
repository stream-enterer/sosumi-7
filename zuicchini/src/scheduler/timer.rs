use std::time::{Duration, Instant};

use slotmap::{new_key_type, SlotMap};

use super::signal::SignalId;

new_key_type! {
    /// Handle to a timer managed by `TimerCentral`.
    pub struct TimerId;
}

struct TimerEntry {
    signal_id: SignalId,
    interval_ms: u64,
    periodic: bool,
    next_fire: Instant,
    active: bool,
}

/// An engine that manages timers. Each `cycle()` checks wall clock time
/// and fires signals for elapsed timers.
pub(crate) struct TimerCentral {
    timers: SlotMap<TimerId, TimerEntry>,
}

impl TimerCentral {
    pub fn new() -> Self {
        Self {
            timers: SlotMap::with_key(),
        }
    }

    /// Create a new timer that fires the given signal after `interval_ms`.
    pub fn create_timer(
        &mut self,
        signal_id: SignalId,
        interval_ms: u64,
        periodic: bool,
    ) -> TimerId {
        // C++ clamps periodic timer interval to at least 1ms to prevent spin-loop
        let clamped_ms = if periodic {
            interval_ms.max(1)
        } else {
            interval_ms
        };
        self.timers.insert(TimerEntry {
            signal_id,
            interval_ms: clamped_ms,
            periodic,
            next_fire: Instant::now() + Duration::from_millis(clamped_ms),
            active: true,
        })
    }

    /// Cancel a timer.
    pub fn cancel_timer(&mut self, id: TimerId) {
        if let Some(entry) = self.timers.get_mut(id) {
            entry.active = false;
        }
    }

    /// Remove a cancelled timer, freeing its slot.
    pub fn remove_timer(&mut self, id: TimerId) {
        self.timers.remove(id);
    }

    /// Check if a timer is still active (running).
    pub fn is_running(&self, id: TimerId) -> bool {
        self.timers.get(id).is_some_and(|t| t.active)
    }

    /// Run timer checks and collect signals to fire. Called directly
    /// by the scheduler (not as a registered engine) at VERY_HIGH priority
    /// equivalent position in the time slice.
    pub fn check_and_collect(&mut self) -> Vec<SignalId> {
        let now = Instant::now();
        let mut signals_to_fire = Vec::new();

        for (_, timer) in &mut self.timers {
            if !timer.active {
                continue;
            }
            if now >= timer.next_fire {
                signals_to_fire.push(timer.signal_id);
                if timer.periodic {
                    timer.next_fire += Duration::from_millis(timer.interval_ms);
                    // Clamp to current time to prevent burst catch-up
                    // (matches C++: `if (st<ct) st=ct;`)
                    if timer.next_fire < now {
                        timer.next_fire = now;
                    }
                } else {
                    timer.active = false;
                }
            }
        }

        // Purge inactive timers to prevent unbounded growth
        self.timers.retain(|_, t| t.active);

        signals_to_fire
    }
}

// TimerCentral is no longer used as an Engine trait object.
// It is called directly by the scheduler. This avoids the dead
// timer_engine_id pattern.

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    #[test]
    fn timer_fires_after_elapsed() {
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let sig = signals.insert(());

        let mut tc = TimerCentral::new();
        tc.create_timer(sig, 0, false); // 0ms = fires immediately

        let fired = tc.check_and_collect();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0], sig);
        assert!(tc.timers.is_empty()); // one-shot purged after fire
    }

    #[test]
    fn periodic_stays_active() {
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let sig = signals.insert(());

        let mut tc = TimerCentral::new();
        // interval_ms=0 is clamped to 1ms for periodic timers,
        // so wait briefly to ensure it fires.
        tc.create_timer(sig, 0, true);
        std::thread::sleep(Duration::from_millis(2));

        let fired = tc.check_and_collect();
        assert_eq!(fired.len(), 1);
        assert!(!tc.timers.is_empty()); // periodic stays
    }

    #[test]
    fn periodic_zero_interval_clamped_to_1ms() {
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let sig = signals.insert(());

        let mut tc = TimerCentral::new();
        tc.create_timer(sig, 0, true);

        // Should NOT fire immediately — clamped to 1ms minimum
        let fired = tc.check_and_collect();
        assert!(fired.is_empty());
    }

    #[test]
    fn cancel_timer() {
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let sig = signals.insert(());

        let mut tc = TimerCentral::new();
        let id = tc.create_timer(sig, 0, false);
        tc.cancel_timer(id);

        let fired = tc.check_and_collect();
        assert!(fired.is_empty());
    }
}
