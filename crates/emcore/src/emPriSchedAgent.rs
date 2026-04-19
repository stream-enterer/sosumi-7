use std::cell::RefCell;
use std::rc::Rc;

use super::emEngine::{emEngine, EngineCtx, EngineId, Priority};
use super::emScheduler::EngineScheduler;

/// Unique identifier for a priority-scheduled agent within a `PriSchedModel`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PriSchedAgentId(usize);

/// Shared state for a priority scheduling resource (e.g., "cpu").
///
/// This is the Rust equivalent of the C++ `emPriSchedAgent::PriSchedModel`.
/// It manages a set of agents that compete for exclusive access to a shared
/// resource, granting access to the highest-priority waiting agent each cycle.
pub struct PriSchedModel {
    inner: Rc<RefCell<PriSchedModelInner>>,
    engine_id: EngineId,
}

struct AgentEntry {
    priority: f64,
    waiting: bool,
    callback: Option<Box<dyn FnMut()>>,
}

struct PriSchedModelInner {
    agents: Vec<AgentEntry>,
    /// Index of the agent that currently has access, or `None`.
    active: Option<PriSchedAgentId>,
    /// The engine ID, needed to wake the engine when agents request access.
    engine_id: Option<EngineId>,
}

struct PriSchedEngine {
    inner: Rc<RefCell<PriSchedModelInner>>,
}

impl emEngine for PriSchedEngine {
    fn Cycle(&mut self, _ctx: &mut EngineCtx<'_>) -> bool {
        let mut model = self.inner.borrow_mut();

        // If there's already an active agent, or no one is waiting, nothing to do.
        if model.active.is_some() {
            return false;
        }

        // Find the waiting agent with the highest priority.
        let mut best: Option<(PriSchedAgentId, f64)> = None;
        for (i, entry) in model.agents.iter().enumerate() {
            if entry.waiting {
                let id = PriSchedAgentId(i);
                match best {
                    None => best = Some((id, entry.priority)),
                    Some((_, best_pri)) if entry.priority >= best_pri => {
                        best = Some((id, entry.priority));
                    }
                    _ => {}
                }
            }
        }

        let Some((winner_id, _)) = best else {
            return false;
        };

        // Grant access: remove from waiting, set active.
        model.agents[winner_id.0].waiting = false;
        model.active = Some(winner_id);

        // Call the GotAccess callback.
        if let Some(cb) = model.agents[winner_id.0].callback.as_mut() {
            cb();
        }

        false
    }
}

impl PriSchedModel {
    /// Create a new priority scheduling model and register its engine.
    pub fn new(scheduler: &mut EngineScheduler) -> Self {
        let inner = Rc::new(RefCell::new(PriSchedModelInner {
            agents: Vec::new(),
            active: None,
            engine_id: None,
        }));

        let engine = PriSchedEngine {
            inner: Rc::clone(&inner),
        };
        let engine_id = scheduler.register_engine( Box::new(engine),Priority::Low);
        inner.borrow_mut().engine_id = Some(engine_id);

        Self { inner, engine_id }
    }

    /// Register a new agent with the given priority and callback.
    /// Returns an ID that can be used to interact with this agent.
    pub fn add_agent(&mut self, priority: f64, got_access: Box<dyn FnMut()>) -> PriSchedAgentId {
        let mut model = self.inner.borrow_mut();
        let id = PriSchedAgentId(model.agents.len());
        model.agents.push(AgentEntry {
            priority,
            waiting: false,
            callback: Some(got_access),
        });
        id
    }

    /// Set the access priority for an agent.
    pub fn SetAccessPriority(&self, agent: PriSchedAgentId, priority: f64) {
        let mut model = self.inner.borrow_mut();
        if let Some(entry) = model.agents.get_mut(agent.0) {
            entry.priority = priority;
        }
    }

    /// Start waiting for access. Wakes the scheduling engine.
    pub fn RequestAccess(&self, agent: PriSchedAgentId, scheduler: &mut EngineScheduler) {
        let mut model = self.inner.borrow_mut();
        if let Some(entry) = model.agents.get_mut(agent.0) {
            entry.waiting = true;
        }
        // If this agent was active, release that status.
        if model.active == Some(agent) {
            model.active = None;
        }
        // Wake the engine if no one is currently active.
        if model.active.is_none() {
            if let Some(eid) = model.engine_id {
                drop(model);
                scheduler.wake_up(eid);
            }
        }
    }

    /// Whether the agent is currently waiting for access.
    pub fn IsWaitingForAccess(&self, agent: PriSchedAgentId) -> bool {
        let model = self.inner.borrow();
        model.agents.get(agent.0).is_some_and(|entry| entry.waiting)
    }

    /// Whether the agent currently has access.
    pub fn HasAccess(&self, agent: PriSchedAgentId) -> bool {
        let model = self.inner.borrow();
        model.active == Some(agent)
    }

    /// Release access (or stop waiting).
    pub fn ReleaseAccess(&self, agent: PriSchedAgentId, scheduler: &mut EngineScheduler) {
        let mut model = self.inner.borrow_mut();
        if let Some(entry) = model.agents.get_mut(agent.0) {
            entry.waiting = false;
        }
        if model.active == Some(agent) {
            model.active = None;
            // Wake the engine so the next waiting agent can get access.
            if let Some(eid) = model.engine_id {
                drop(model);
                scheduler.wake_up(eid);
            }
        }
    }

    /// Remove the scheduling engine from the scheduler.
    /// Call this before dropping the model.
    pub fn remove(self, scheduler: &mut EngineScheduler) {
        scheduler.remove_engine(self.engine_id);
    }

    /// Get the engine ID for this model (useful for testing).
    pub fn engine_id(&self) -> EngineId {
        self.engine_id
    }
}

#[cfg(test)]
mod tests {
    use super::super::emPanelTree::PanelTree;
    use super::super::emWindow::emWindow;
    use super::*;
    use std::collections::HashMap;
    use winit::window::WindowId;

    fn slice(sched: &mut EngineScheduler) {
        let mut tree = PanelTree::new();
        let mut windows: HashMap<WindowId, std::rc::Rc<std::cell::RefCell<emWindow>>> =
            HashMap::new();
        let __root_ctx = crate::emContext::emContext::NewRoot();
        sched.DoTimeSlice(&mut tree, &mut windows, &__root_ctx);
    }

    #[test]
    fn highest_priority_gets_access() {
        let mut sched = EngineScheduler::new();
        let mut model = PriSchedModel::new(&mut sched);

        let got_a = Rc::new(RefCell::new(false));
        let got_b = Rc::new(RefCell::new(false));

        let ga = Rc::clone(&got_a);
        let agent_a = model.add_agent(1.0, Box::new(move || *ga.borrow_mut() = true));
        let gb = Rc::clone(&got_b);
        let agent_b = model.add_agent(2.0, Box::new(move || *gb.borrow_mut() = true));

        model.RequestAccess(agent_a, &mut sched);
        model.RequestAccess(agent_b, &mut sched);

        slice(&mut sched);

        // Agent B has higher priority, should get access first.
        assert!(!*got_a.borrow());
        assert!(*got_b.borrow());
        assert!(model.HasAccess(agent_b));
        assert!(!model.HasAccess(agent_a));

        // Release B, then A should get access on next cycle.
        model.ReleaseAccess(agent_b, &mut sched);
        slice(&mut sched);

        assert!(*got_a.borrow());
        assert!(model.HasAccess(agent_a));

        model.ReleaseAccess(agent_a, &mut sched);
        model.remove(&mut sched);
    }

    #[test]
    fn request_while_active_requeues() {
        let mut sched = EngineScheduler::new();
        let mut model = PriSchedModel::new(&mut sched);

        let count = Rc::new(RefCell::new(0u32));
        let c = Rc::clone(&count);
        let agent = model.add_agent(1.0, Box::new(move || *c.borrow_mut() += 1));

        model.RequestAccess(agent, &mut sched);
        slice(&mut sched);
        assert_eq!(*count.borrow(), 1);
        assert!(model.HasAccess(agent));

        // Re-request while active — should clear active and requeue.
        model.RequestAccess(agent, &mut sched);
        assert!(!model.HasAccess(agent));
        assert!(model.IsWaitingForAccess(agent));

        slice(&mut sched);
        assert_eq!(*count.borrow(), 2);
        assert!(model.HasAccess(agent));

        model.ReleaseAccess(agent, &mut sched);
        model.remove(&mut sched);
    }
}
