use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emcore::emPanelTree::PanelTree;
use emcore::emPriSchedAgent::PriSchedModel;
use emcore::emScheduler::EngineScheduler;
use emcore::emWindow::emWindow;
use winit::window::WindowId;

fn slice(sched: &mut EngineScheduler) {
    let mut tree = PanelTree::new();
    let mut windows: HashMap<WindowId, std::rc::Rc<std::cell::RefCell<emWindow>>> = HashMap::new();
    let __root_ctx = emcore::emContext::emContext::NewRoot();
    let mut __fw: Vec<_> = Vec::new();
    sched.DoTimeSlice(&mut tree, &mut windows, &__root_ctx, &mut __fw);
}

#[test]
fn highest_priority_gets_access_first() {
    let mut sched = EngineScheduler::new();
    let mut model = PriSchedModel::new(&mut sched);

    let got_a = Rc::new(RefCell::new(false));
    let got_b = Rc::new(RefCell::new(false));
    let got_c = Rc::new(RefCell::new(false));

    let ga = Rc::clone(&got_a);
    let agent_a = model.add_agent(1.0, Box::new(move || *ga.borrow_mut() = true));
    let gb = Rc::clone(&got_b);
    let agent_b = model.add_agent(5.0, Box::new(move || *gb.borrow_mut() = true));
    let gc = Rc::clone(&got_c);
    let agent_c = model.add_agent(10.0, Box::new(move || *gc.borrow_mut() = true));

    model.RequestAccess(agent_a, &mut sched);
    model.RequestAccess(agent_b, &mut sched);
    model.RequestAccess(agent_c, &mut sched);

    slice(&mut sched);

    // Agent C (GetPriority 10) should GetRec access
    assert!(!*got_a.borrow());
    assert!(!*got_b.borrow());
    assert!(*got_c.borrow());
    assert!(model.HasAccess(agent_c));

    // Release C, B should GetRec access next
    model.ReleaseAccess(agent_c, &mut sched);
    slice(&mut sched);

    assert!(!*got_a.borrow());
    assert!(*got_b.borrow());
    assert!(model.HasAccess(agent_b));

    // Release B, A should GetRec access
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

    // Re-request while active clears active and requeues
    model.RequestAccess(agent, &mut sched);
    assert!(!model.HasAccess(agent));
    assert!(model.IsWaitingForAccess(agent));

    slice(&mut sched);
    assert_eq!(*count.borrow(), 2);
    assert!(model.HasAccess(agent));

    model.ReleaseAccess(agent, &mut sched);
    model.remove(&mut sched);
}

#[test]
fn release_without_access_is_noop() {
    let mut sched = EngineScheduler::new();
    let mut model = PriSchedModel::new(&mut sched);

    let agent = model.add_agent(1.0, Box::new(|| {}));

    // Release when not active and not waiting — should not panic
    model.ReleaseAccess(agent, &mut sched);
    assert!(!model.HasAccess(agent));
    assert!(!model.IsWaitingForAccess(agent));

    model.remove(&mut sched);
}

#[test]
fn no_grant_when_active_exists() {
    let mut sched = EngineScheduler::new();
    let mut model = PriSchedModel::new(&mut sched);

    let got_a = Rc::new(RefCell::new(false));
    let got_b = Rc::new(RefCell::new(false));

    let ga = Rc::clone(&got_a);
    let agent_a = model.add_agent(1.0, Box::new(move || *ga.borrow_mut() = true));
    let gb = Rc::clone(&got_b);
    let agent_b = model.add_agent(2.0, Box::new(move || *gb.borrow_mut() = true));

    // A gets access
    model.RequestAccess(agent_a, &mut sched);
    slice(&mut sched);
    assert!(model.HasAccess(agent_a));

    // B requests but A is still active — B should not GetRec access
    model.RequestAccess(agent_b, &mut sched);
    slice(&mut sched);
    assert!(!*got_b.borrow());
    assert!(!model.HasAccess(agent_b));
    assert!(model.IsWaitingForAccess(agent_b));

    model.ReleaseAccess(agent_a, &mut sched);
    slice(&mut sched);
    assert!(*got_b.borrow());
    assert!(model.HasAccess(agent_b));

    model.ReleaseAccess(agent_b, &mut sched);
    model.remove(&mut sched);
}

#[test]
fn set_access_priority_changes_grant_order() {
    let mut sched = EngineScheduler::new();
    let mut model = PriSchedModel::new(&mut sched);

    let got_a = Rc::new(RefCell::new(false));
    let got_b = Rc::new(RefCell::new(false));

    let ga = Rc::clone(&got_a);
    let agent_a = model.add_agent(1.0, Box::new(move || *ga.borrow_mut() = true));
    let gb = Rc::clone(&got_b);
    let agent_b = model.add_agent(10.0, Box::new(move || *gb.borrow_mut() = true));

    // Boost A above B
    model.SetAccessPriority(agent_a, 20.0);

    model.RequestAccess(agent_a, &mut sched);
    model.RequestAccess(agent_b, &mut sched);
    slice(&mut sched);

    // A should win now despite originally being lower
    assert!(*got_a.borrow());
    assert!(!*got_b.borrow());
    assert!(model.HasAccess(agent_a));

    model.ReleaseAccess(agent_a, &mut sched);
    model.ReleaseAccess(agent_b, &mut sched);
    model.remove(&mut sched);
}

#[test]
fn is_waiting_tracks_state() {
    let mut sched = EngineScheduler::new();
    let mut model = PriSchedModel::new(&mut sched);

    let agent = model.add_agent(1.0, Box::new(|| {}));

    assert!(!model.IsWaitingForAccess(agent));

    model.RequestAccess(agent, &mut sched);
    assert!(model.IsWaitingForAccess(agent));

    slice(&mut sched);
    // After grant, no longer waiting
    assert!(!model.IsWaitingForAccess(agent));
    assert!(model.HasAccess(agent));

    model.ReleaseAccess(agent, &mut sched);
    model.remove(&mut sched);
}
