use slotmap::new_key_type;

use crate::emPanelTree::PanelId;

new_key_type! {
    /// Handle to an engine in the scheduler.
    pub struct EngineId;
}

/// Location of an engine relative to the outer `PanelTree`.
///
/// Phase 1.75 unifies engine dispatch across outer and sub-view trees by
/// tagging every registered engine with a `TreeLocation` that tells the
/// scheduler how to reach the engine's tree from the outer tree.
///
/// `Outer` means the engine belongs to the outer tree directly.
/// `SubView { outer_panel_id, rest }` means: walk to `outer_panel_id` in the
/// outer tree, take its behavior, access the `emSubViewPanel::sub_tree`
/// through `as_sub_view_panel_mut`, and resolve the remaining location
/// within that sub-tree. Supports arbitrary nesting via `rest`.
///
/// See the Phase 1.75 plan §Architecture for the dispatch walk.
#[derive(Clone, Debug)]
pub enum TreeLocation {
    Outer,
    SubView {
        outer_panel_id: PanelId,
        rest: Box<TreeLocation>,
    },
}

/// A unit of cooperative work executed by the scheduler.
///
/// Engines are the primary scheduling primitive. They receive `Cycle()` calls
/// from the scheduler with an `EngineCtx` that provides access to signals,
/// timers, the panel tree, the window registry, and time-slice queries.
///
/// `std::any::Any` supertrait is required so that test-support helpers can
/// downcast `Box<dyn emEngine>` to a concrete type (e.g.
/// `PanelCycleEngine`) without a separate registry. Only `'static` types
/// may implement `emEngine`; all current implementations satisfy this.
pub trait emEngine: std::any::Any {
    /// Called when the engine is awake. Return `true` to stay awake next slice,
    /// `false` to go to sleep.
    ///
    /// The `ctx` parameter provides access to the scheduler for firing signals,
    /// checking `IsSignaled`, querying `IsTimeSliceAtEnd`, etc.
    fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool;
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
