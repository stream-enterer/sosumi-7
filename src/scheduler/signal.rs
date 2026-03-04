use slotmap::new_key_type;

use super::engine::EngineId;

new_key_type! {
    /// Handle to a signal in the scheduler.
    pub struct SignalId;
}

/// Internal state for a signal.
pub(crate) struct SignalData {
    pub pending: bool,
    pub connected_engines: Vec<EngineId>,
}

impl SignalData {
    pub fn new() -> Self {
        Self {
            pending: false,
            connected_engines: Vec::new(),
        }
    }
}
