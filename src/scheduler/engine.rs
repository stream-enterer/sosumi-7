use slotmap::new_key_type;

new_key_type! {
    /// Handle to an engine in the scheduler.
    pub struct EngineId;
}

/// A unit of cooperative work executed by the scheduler.
pub trait Engine {
    /// Called when the engine is awake. Return `true` to stay awake next slice,
    /// `false` to go to sleep.
    fn cycle(&mut self) -> bool;
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
    pub awake: bool,
    pub behavior: Option<Box<dyn Engine>>,
}
