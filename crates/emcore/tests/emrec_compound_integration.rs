//! Phase 4c Task 6 — end-to-end composition + stress tests.
//!
//! Consolidated integration test that exercises the full C++
//! `Person` + `emTArrayRec<Person>` example (emRec.h:78-108) and a
//! 1000× stress loop against a multi-level tree.
//!
//! Scope:
//!   1. `person_array_listener_fires_on_nested_mutation` — build
//!      `emTArrayRec<Person>`, SetCount(2), attach `emRecListener` at the
//!      array level, mutate `persons.GetMut(0).unwrap().name.SetValue(...)`,
//!      drive scheduler, assert listener callback invoked exactly once.
//!      Proves the reified aggregate chain traverses user-derived compound
//!      (`Person::register_aggregate` -> `name`) -> `emTArrayRec` -> listener
//!      end-to-end.
//!   2. `stress_1000_mutations_fires_1000_times` — run a 1000-iteration
//!      loop mutating a leaf inside a 3-level tree (array -> Person ->
//!      leaf), driving the scheduler between iterations so signals do
//!      not coalesce. Asserts listener fires exactly 1000 times.
//!
//! Co-located pattern: `make_sched_ctx` / `run_slice` helpers inlined
//! per CLAUDE.md anti-hoist policy (duplicate rather than share test
//! scaffolding across modules).

use emcore::emBoolRec::emBoolRec;
use emcore::emClipboard::emClipboard;
use emcore::emContext::emContext;
use emcore::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
use emcore::emIntRec::emIntRec;
use emcore::emRec::emRec;
use emcore::emRecListener::emRecListener;
use emcore::emRecNode::emRecNode;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;
use emcore::emStringRec::emStringRec;
use emcore::emStructRec::emStructRec;
use emcore::emTArrayRec::{emTArrayRec, emTRecAllocator};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

fn make_sched_ctx<'a>(
    sched: &'a mut EngineScheduler,
    actions: &'a mut Vec<DeferredAction>,
    ctx_root: &'a Rc<emContext>,
    cb: &'a RefCell<Option<Box<dyn emClipboard>>>,
    pa: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
) -> SchedCtx<'a> {
    SchedCtx {
        scheduler: sched,
        framework_actions: actions,
        root_context: ctx_root,
        framework_clipboard: cb,
        current_engine: None,
        pending_actions: pa,
    }
}

/// Run one full scheduler time slice with empty window/input state. Matches
/// the pattern used in `emRecListener` unit tests.
fn run_slice(sched: &mut EngineScheduler) {
    let mut windows = HashMap::new();
    let root = emContext::NewRoot();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
        Vec::new();
    let mut input_state = emcore::emInputState::emInputState::new();
    let fc: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    sched.DoTimeSlice(
        &mut windows,
        &root,
        &mut actions,
        &mut pending_inputs,
        &mut input_state,
        &fc,
        &pa,
    );
}

/// User-level derived compound mirroring the C++ `Person` example
/// (emRec.h:78-108): a struct with a name, age, and sex field grouped
/// under an `emStructRec` member registry.
struct Person {
    inner: emStructRec,
    name: emStringRec,
    age: emIntRec,
    male: emBoolRec,
}

impl Person {
    fn new(ctx: &mut SchedCtx<'_>) -> Self {
        let mut inner = emStructRec::new(ctx);
        let mut name = emStringRec::new(ctx, String::new());
        let mut age = emIntRec::new(ctx, 0, i64::MIN, i64::MAX);
        let mut male = emBoolRec::new(ctx, false);
        inner.AddMember(&mut name, "name");
        inner.AddMember(&mut age, "age");
        inner.AddMember(&mut male, "male");
        Self {
            inner,
            name,
            age,
            male,
        }
    }
}

impl emRecNode for Person {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    /// Forward to inner struct AND every sibling leaf so register_aggregate
    /// splices reach the whole compound. Matches the pattern documented in
    /// emStructRec's module docs.
    fn register_aggregate(&mut self, sig: SignalId) {
        self.inner.register_aggregate(sig);
        self.name.register_aggregate(sig);
        self.age.register_aggregate(sig);
        self.male.register_aggregate(sig);
    }

    fn listened_signal(&self) -> SignalId {
        self.inner.listened_signal()
    }
}

fn person_allocator() -> emTRecAllocator<Person> {
    Box::new(|ctx: &mut SchedCtx<'_>| Person::new(ctx))
}

/// End-to-end: `emTArrayRec<Person>` with a listener attached at the array
/// level fires on a nested leaf mutation through the user's derived
/// compound.
#[test]
fn person_array_listener_fires_on_nested_mutation() {
    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let mut persons = emTArrayRec::<Person>::new(&mut sc, person_allocator(), 0, 100);
    persons.SetCount(2, &mut sc);

    // SetCount fires the array's aggregate signal (grow path). Drain it so
    // the listener's first pending cycle doesn't contaminate the count.
    let agg = persons.GetAggregateSignal();
    sc.scheduler.abort(agg);

    let hits = Rc::new(Cell::new(0u32));
    let hits_cb = Rc::clone(&hits);
    let listener = emRecListener::new(
        Some(&persons),
        Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
        &mut sc,
    );

    // Nested leaf mutation through the user's derived compound.
    persons
        .GetMut(0)
        .expect("slot 0 exists")
        .name
        .SetValue("Fred".to_string(), &mut sc);

    // Drive the scheduler — listener callback dispatches asynchronously.
    let _ = sc;
    run_slice(&mut sched);
    assert_eq!(
        hits.get(),
        1,
        "array-level listener must fire exactly once on nested Person.name change",
    );

    // Teardown — collect every signal created by this test (array agg,
    // per-Person inner agg, per-leaf value signals) and remove each.
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
    listener.detach(&mut sc);
    // Snapshot signal ids before moving persons apart.
    let p0_inner = persons.Get(0).unwrap().inner.GetAggregateSignal();
    let p0_name = persons.Get(0).unwrap().name.GetValueSignal();
    let p0_age = persons.Get(0).unwrap().age.GetValueSignal();
    let p0_male = persons.Get(0).unwrap().male.GetValueSignal();
    let p1_inner = persons.Get(1).unwrap().inner.GetAggregateSignal();
    let p1_name = persons.Get(1).unwrap().name.GetValueSignal();
    let p1_age = persons.Get(1).unwrap().age.GetValueSignal();
    let p1_male = persons.Get(1).unwrap().male.GetValueSignal();
    for sig in [
        agg, p0_inner, p0_name, p0_age, p0_male, p1_inner, p1_name, p1_age, p1_male,
    ] {
        sc.scheduler.abort(sig);
        sc.remove_signal(sig);
    }
}

/// Stress test: 1000 mutations across a 3-level tree (array -> Person ->
/// leaf). Listener fires exactly 1000 times when the scheduler is driven
/// between each mutation.
///
/// Each iteration mutates `persons[0].age` with a fresh value (i as i64) so
/// no-op suppression (emIntRec returns early when SetValue receives the
/// current value) never short-circuits. After each SetValue we call
/// `run_slice` to dispatch the pending listener callback before the next
/// mutation fires its signal — without this, the scheduler would coalesce
/// multiple signals into a single Cycle call (Priority::Low /
/// Framework-scope engine fires once per cycle when signaled, regardless
/// of how many fires happened between cycles).
///
/// Secondary purpose (per plan): exercise the reified-chain representation
/// under load — chain forwarding is O(depth) per mutation, not O(n²).
#[test]
fn stress_1000_mutations_fires_1000_times() {
    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let mut persons = emTArrayRec::<Person>::new(&mut sc, person_allocator(), 0, 8);
    persons.SetCount(2, &mut sc);
    let agg = persons.GetAggregateSignal();
    sc.scheduler.abort(agg);

    let hits = Rc::new(Cell::new(0u32));
    let hits_cb = Rc::clone(&hits);
    let listener = emRecListener::new(
        Some(&persons),
        Box::new(move |_sc| hits_cb.set(hits_cb.get() + 1)),
        &mut sc,
    );

    let _ = sc;

    for i in 0..1000i64 {
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        // New value each iteration — avoids no-op suppression (emIntRec
        // short-circuits SetValue when the value is unchanged). Start at 1
        // so the first write differs from the default (0).
        persons
            .GetMut(0)
            .expect("slot 0 exists")
            .age
            .SetValue(i + 1, &mut sc);
        let _ = sc;
        run_slice(&mut sched);
    }

    assert_eq!(
        hits.get(),
        1000,
        "listener must fire exactly 1000 times across 1000 mutations",
    );

    // Teardown.
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);
    listener.detach(&mut sc);
    let p0_inner = persons.Get(0).unwrap().inner.GetAggregateSignal();
    let p0_name = persons.Get(0).unwrap().name.GetValueSignal();
    let p0_age = persons.Get(0).unwrap().age.GetValueSignal();
    let p0_male = persons.Get(0).unwrap().male.GetValueSignal();
    let p1_inner = persons.Get(1).unwrap().inner.GetAggregateSignal();
    let p1_name = persons.Get(1).unwrap().name.GetValueSignal();
    let p1_age = persons.Get(1).unwrap().age.GetValueSignal();
    let p1_male = persons.Get(1).unwrap().male.GetValueSignal();
    for sig in [
        agg, p0_inner, p0_name, p0_age, p0_male, p1_inner, p1_name, p1_age, p1_male,
    ] {
        sc.scheduler.abort(sig);
        sc.remove_signal(sig);
    }
}
