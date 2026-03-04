use std::time::Instant;

use super::engine::Engine;
use super::signal::SignalId;

/// Handle to a timer managed by `TimerCentral`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TimerId(pub(crate) usize);

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
    timers: Vec<TimerEntry>,
    /// Signal IDs that need to be fired — collected during cycle, applied by scheduler.
    pub(crate) signals_to_fire: Vec<SignalId>,
}

impl TimerCentral {
    pub fn new() -> Self {
        Self {
            timers: Vec::new(),
            signals_to_fire: Vec::new(),
        }
    }

    /// Create a new timer that fires the given signal after `interval_ms`.
    pub fn create_timer(
        &mut self,
        signal_id: SignalId,
        interval_ms: u64,
        periodic: bool,
    ) -> TimerId {
        let id = TimerId(self.timers.len());
        self.timers.push(TimerEntry {
            signal_id,
            interval_ms,
            periodic,
            next_fire: Instant::now()
                + std::time::Duration::from_millis(interval_ms),
            active: true,
        });
        id
    }

    /// Cancel a timer.
    pub fn cancel_timer(&mut self, id: TimerId) {
        if let Some(entry) = self.timers.get_mut(id.0) {
            entry.active = false;
        }
    }

    /// Check if any timers are still armed.
    pub fn has_active_timers(&self) -> bool {
        self.timers.iter().any(|t| t.active)
    }
}

impl Engine for TimerCentral {
    fn cycle(&mut self) -> bool {
        let now = Instant::now();
        self.signals_to_fire.clear();

        for timer in &mut self.timers {
            if !timer.active {
                continue;
            }
            if now >= timer.next_fire {
                self.signals_to_fire.push(timer.signal_id);
                if timer.periodic {
                    timer.next_fire += std::time::Duration::from_millis(timer.interval_ms);
                } else {
                    timer.active = false;
                }
            }
        }

        self.has_active_timers()
    }
}

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

        let stay_awake = tc.cycle();
        assert!(!stay_awake); // one-shot, now inactive
        assert_eq!(tc.signals_to_fire.len(), 1);
        assert_eq!(tc.signals_to_fire[0], sig);
    }

    #[test]
    fn periodic_stays_active() {
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let sig = signals.insert(());

        let mut tc = TimerCentral::new();
        tc.create_timer(sig, 0, true); // periodic, fires immediately

        let stay_awake = tc.cycle();
        assert!(stay_awake); // periodic stays active
        assert_eq!(tc.signals_to_fire.len(), 1);
    }

    #[test]
    fn cancel_timer() {
        let mut signals: SlotMap<SignalId, ()> = SlotMap::with_key();
        let sig = signals.insert(());

        let mut tc = TimerCentral::new();
        let id = tc.create_timer(sig, 0, false);
        tc.cancel_timer(id);

        let stay_awake = tc.cycle();
        assert!(!stay_awake);
        assert!(tc.signals_to_fire.is_empty());
    }
}
