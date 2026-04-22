//! emStructRec — structured record with named members.
//!
//! C++ reference: `include/emCore/emRec.h:930-1031` (class emStructRec).
//!
//! C++ shape: a user declares a subclass of `emStructRec` with member records
//! as data fields (see `Person` example at emRec.h:78-108), then calls
//! `AddMember(&child, "identifier")` in the constructor for each one.
//! `emStructRec` stores only pointers to the externally-owned children and
//! their identifiers; ChildChanged walks up the UpperNode chain from any
//! descendant leaf.
//!
//! Rust rep (per Phase 4c Task 3 approved architecture): `emStructRec` does
//! NOT own child records either. Users write their own derived struct
//! (`struct Person { inner: emStructRec, name: emStringRec, age: emIntRec, ... }`)
//! and implement `emRecNode` by hand, forwarding `register_aggregate` to
//! every sibling field record. This matches the C++ shape: emStructRec holds
//! only the identifier registry and its own aggregate signal.
//!
//! The UpperNode/ChildChanged walk is replaced by the reified aggregate
//! chain from ADR 2026-04-21-phase-4b-listener-tree-adr.md (R5): when
//! `AddMember` is called, the struct's aggregate signal is pushed onto every
//! descendant leaf's `aggregate_signals` vector via
//! `child.register_aggregate(self.aggregate_signal)`. If the child is itself
//! a compound (another emStructRec inside a derived struct), the user's
//! derived `emRecNode::register_aggregate` impl forwards to all its sibling
//! fields recursively.
//!
//! Persistence methods (TryStartReading / TryContinueReading / QuitReading /
//! TryStartWriting / TryContinueWriting / QuitWriting / SetToDefault /
//! IsSetToDefault / CalcRecMemNeed / ShallWriteOptionalOnly) are deferred —
//! they belong to the emRecReader/emRecWriter serialization machinery that
//! ships in Phase 4d.

use crate::emEngineCtx::ConstructCtx;
use crate::emRec::CheckIdentifier;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

/// Member registry entry. Stores only the identifier — the child record
/// itself is owned by the user's derived struct as a sibling field.
///
/// DIVERGED: C++ `emStructRec::MemberType` (emRec.h:1000-1003) holds an
/// `emRec* Record` pointer alongside the identifier. Rust cannot safely
/// store a back-pointer to a sibling field without `unsafe` or interior
/// mutability, so the Rust rep keeps only the identifier; the `Get(i)`
/// accessor is provided by the user's derived struct (trivial match on
/// index). See the approved-architecture note in Phase 4c Task 3.
struct MemberInfo {
    identifier: String,
}

/// Structured record: named-member registry plus a reified aggregate signal.
pub struct emStructRec {
    /// Aggregate signal for the whole struct. Fires whenever any descendant
    /// leaf's `SetValue` observes a real change — via the reified chain, not
    /// an UpperNode walk.
    aggregate_signal: SignalId,
    /// Identifiers of registered members, in insertion order.
    members: Vec<MemberInfo>,
    /// Reified aggregate-signal chain; see ADR
    /// 2026-04-21-phase-4b-listener-tree-adr.md. Non-empty when this struct
    /// is itself nested inside an outer compound (its parent's aggregate
    /// signal is pushed here via `register_aggregate`).
    aggregate_signals: Vec<SignalId>,
}

impl emStructRec {
    /// Construct an empty struct record. Equivalent to the C++ default
    /// constructor `emStructRec::emStructRec()` (emRec.h:940).
    ///
    /// DIVERGED: C++ also exposes `emStructRec(emStructRec* parent, const
    /// char* varIdentifier)` (emRec.h:941-948) which immediately splices the
    /// new struct into a parent as a named member. Rust splits this: users
    /// construct the inner struct with `new`, then the outer struct calls
    /// `AddMember(&mut inner, "name")`. The two-step pattern matches the
    /// observable outcome (the inner is registered as a named member of the
    /// outer with the given identifier) and avoids a self-referential
    /// constructor call.
    pub fn new<C: ConstructCtx>(ctx: &mut C) -> Self {
        Self {
            aggregate_signal: ctx.create_signal(),
            members: Vec::new(),
            aggregate_signals: Vec::new(),
        }
    }

    /// Register a named member. Splices `self.aggregate_signal` into the
    /// child's reified aggregate chain so any descendant leaf mutation
    /// reaches this struct's aggregate signal.
    ///
    /// C++ reference: `emStructRec::AddMember` (emRec.h:993 private; invoked
    /// by derived-class constructors). C++ stores an `emRec*` pointer to
    /// the child; the UpperNode is set separately via the child's
    /// constructor. Here we just push the parent's aggregate signal into the
    /// child's reified chain.
    ///
    /// DIVERGED: C++ `emRec::Changed()` (emRec.h:243 inline, delegates to
    /// `emRec::ChildChanged` at emRec.cpp:217) walks `UpperNode` per-fire.
    /// Rust fires the reified aggregate chain registered here. See ADR
    /// 2026-04-21-phase-4b-listener-tree-adr.md.
    pub fn AddMember<R: emRecNode + ?Sized>(&mut self, child: &mut R, identifier: &str) {
        CheckIdentifier(identifier);
        child.register_aggregate(self.aggregate_signal);
        self.members.push(MemberInfo {
            identifier: identifier.to_string(),
        });
    }

    /// Number of registered members. C++: `emStructRec::GetCount` (emRec.h:954).
    pub fn GetCount(&self) -> i32 {
        self.members.len() as i32
    }

    /// Identifier of the member at `index`, or `None` if out of range.
    ///
    /// C++: `emStructRec::GetIdentifierOf(int index)` (emRec.h:965-967)
    /// returns `const char*` with `NULL` on out-of-range. Rust returns
    /// `Option<&str>` — same three-state API (valid string / not found)
    /// without null-pointer semantics.
    pub fn GetIdentifierOf(&self, index: i32) -> Option<&str> {
        if index < 0 {
            return None;
        }
        self.members
            .get(index as usize)
            .map(|m| m.identifier.as_str())
    }

    /// Index of the member with the given identifier, or `-1` if not found.
    ///
    /// C++: `emStructRec::GetIndexOf(const char* identifier)` (emRec.h:973-974).
    /// Returns `-1` per C++ contract.
    ///
    /// DIVERGED: C++ also exposes `GetIndexOf(const emRec* member)`
    /// (emRec.h:969-970) which linear-scans the stored `Record` pointers.
    /// Rust cannot port that overload because the Rust rep does not store
    /// back-pointers to sibling fields. Name correspondence preserved for
    /// the identifier overload.
    pub fn GetIndexOf(&self, identifier: &str) -> i32 {
        self.members
            .iter()
            .position(|m| m.identifier == identifier)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }

    /// Accessor for the reified aggregate signal. Used by outer compounds
    /// (via `AddMember`) and by `emRecListener::SetListenedRec` through
    /// `listened_signal()`.
    ///
    /// DIVERGED: no direct C++ counterpart — C++ listeners splice into the
    /// `UpperNode` chain instead. Exposed for the reified-chain rep (ADR
    /// 2026-04-21-phase-4b-listener-tree-adr.md).
    pub fn GetAggregateSignal(&self) -> SignalId {
        self.aggregate_signal
    }

    // TODO(phase-4d): ShallWriteOptionalOnly, SetToDefault, IsSetToDefault,
    // TryStartReading, TryContinueReading, QuitReading, TryStartWriting,
    // TryContinueWriting, QuitWriting, CalcRecMemNeed per emRec.h:980-991.
    //
    // DIVERGED: C++ `Get(int)` / `operator[]` (emRec.h:957-962) return
    // `emRec&` by index. Rust cannot port this overload because the struct
    // does not own the children (no back-pointers to sibling fields).
    // Callers access members through the user's derived struct fields
    // directly — the same access pattern C++ derived classes use when the
    // children are declared as named fields.
}

impl emRecNode for emStructRec {
    /// DIVERGED: parent is tracked only through the aggregate chain; no
    /// `UpperNode` pointer is stored (C1: no new `Rc<RefCell>`, C7: no new
    /// `unsafe`). Derived-struct impls returning `None` is consistent with
    /// the other emRec nodes in the current tree (all return `None` until
    /// Phase 4b port of `UpperNode`).
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    /// DIVERGED: forwards only to this struct's own `aggregate_signals`.
    /// When `emStructRec` is used inside a user's derived struct, the user's
    /// derived `emRecNode::register_aggregate` impl must additionally
    /// forward the signal to every sibling field record so descendant leaf
    /// mutations propagate through both the inner struct and the outer
    /// compound. See ADR 2026-04-21-phase-4b-listener-tree-adr.md.
    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
    }

    fn listened_signal(&self) -> SignalId {
        self.aggregate_signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emBoolRec::emBoolRec;
    use crate::emClipboard::emClipboard;
    use crate::emContext::emContext;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
    use crate::emIntRec::emIntRec;
    use crate::emRec::emRec;
    use crate::emScheduler::EngineScheduler;
    use crate::emStringRec::emStringRec;
    use std::cell::RefCell;
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

    /// User-level derived struct mirroring C++ `Person` example (emRec.h:78-108).
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

        /// DIVERGED (ADR 2026-04-21-phase-4b-listener-tree-adr.md): user's
        /// derived compound forwards to inner struct AND every sibling leaf.
        /// Matches the C++ `UpperNode` chain semantics via the reified
        /// aggregate Vec.
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

    /// Invariant I4c-4: aggregate signal fires on any field mutation.
    #[test]
    fn person_aggregate_fires_on_any_field_mutation() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut person = Person::new(&mut sc);
        let agg = person.inner.GetAggregateSignal();

        // name
        person.name.SetValue("alice".to_string(), &mut sc);
        assert!(sc.is_signaled(agg), "aggregate fires on name change");
        sc.scheduler.abort(agg);
        sc.scheduler.abort(person.name.GetValueSignal());

        // age
        person.age.SetValue(42, &mut sc);
        assert!(sc.is_signaled(agg), "aggregate fires on age change");
        sc.scheduler.abort(agg);
        sc.scheduler.abort(person.age.GetValueSignal());

        // male
        person.male.SetValue(true, &mut sc);
        assert!(sc.is_signaled(agg), "aggregate fires on male change");
        sc.scheduler.abort(agg);
        sc.scheduler.abort(person.male.GetValueSignal());

        // Clean up all created signals before drop.
        sc.remove_signal(agg);
        sc.remove_signal(person.name.GetValueSignal());
        sc.remove_signal(person.age.GetValueSignal());
        sc.remove_signal(person.male.GetValueSignal());
    }

    struct Address {
        inner: emStructRec,
        street: emStringRec,
        zip: emIntRec,
    }

    impl Address {
        fn new(ctx: &mut SchedCtx<'_>) -> Self {
            let mut inner = emStructRec::new(ctx);
            let mut street = emStringRec::new(ctx, String::new());
            let mut zip = emIntRec::new(ctx, 0, i64::MIN, i64::MAX);
            inner.AddMember(&mut street, "street");
            inner.AddMember(&mut zip, "zip");
            Self { inner, street, zip }
        }
    }

    impl emRecNode for Address {
        fn parent(&self) -> Option<&dyn emRecNode> {
            None
        }
        fn register_aggregate(&mut self, sig: SignalId) {
            self.inner.register_aggregate(sig);
            self.street.register_aggregate(sig);
            self.zip.register_aggregate(sig);
        }
        fn listened_signal(&self) -> SignalId {
            self.inner.listened_signal()
        }
    }

    struct PersonWithAddr {
        inner: emStructRec,
        name: emStringRec,
        addr: Address,
        age: emIntRec,
    }

    impl PersonWithAddr {
        fn new(ctx: &mut SchedCtx<'_>) -> Self {
            let mut inner = emStructRec::new(ctx);
            let mut name = emStringRec::new(ctx, String::new());
            let mut addr = Address::new(ctx);
            let mut age = emIntRec::new(ctx, 0, i64::MIN, i64::MAX);
            inner.AddMember(&mut name, "name");
            inner.AddMember(&mut addr, "addr");
            inner.AddMember(&mut age, "age");
            Self {
                inner,
                name,
                addr,
                age,
            }
        }
    }

    impl emRecNode for PersonWithAddr {
        fn parent(&self) -> Option<&dyn emRecNode> {
            None
        }
        fn register_aggregate(&mut self, sig: SignalId) {
            self.inner.register_aggregate(sig);
            self.name.register_aggregate(sig);
            self.addr.register_aggregate(sig);
            self.age.register_aggregate(sig);
        }
        fn listened_signal(&self) -> SignalId {
            self.inner.listened_signal()
        }
    }

    /// Invariant I4c-5: aggregate fires through sub-struct compound.
    #[test]
    fn multi_level_nesting_fires_through_sub_struct() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut pwa = PersonWithAddr::new(&mut sc);
        let outer_agg = pwa.inner.GetAggregateSignal();
        let inner_agg = pwa.addr.inner.GetAggregateSignal();

        pwa.addr.zip.SetValue(90024, &mut sc);

        assert!(
            sc.is_signaled(inner_agg),
            "inner (Address) aggregate must fire"
        );
        assert!(
            sc.is_signaled(outer_agg),
            "outer (PersonWithAddr) aggregate must fire through sub-struct"
        );

        sc.remove_signal(inner_agg);
        sc.remove_signal(outer_agg);
        sc.remove_signal(pwa.addr.zip.GetValueSignal());
    }

    #[test]
    fn add_member_returns_correct_count_and_identifier() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let person = Person::new(&mut sc);

        assert_eq!(person.inner.GetCount(), 3);
        assert_eq!(person.inner.GetIdentifierOf(0), Some("name"));
        assert_eq!(person.inner.GetIdentifierOf(1), Some("age"));
        assert_eq!(person.inner.GetIdentifierOf(2), Some("male"));
        assert_eq!(person.inner.GetIdentifierOf(3), None);
        assert_eq!(person.inner.GetIdentifierOf(-1), None);

        assert_eq!(person.inner.GetIndexOf("name"), 0);
        assert_eq!(person.inner.GetIndexOf("age"), 1);
        assert_eq!(person.inner.GetIndexOf("male"), 2);
        assert_eq!(person.inner.GetIndexOf("missing"), -1);
    }

    #[test]
    fn aggregate_signal_does_not_fire_on_no_op_mutation_through_chain() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        // Multi-level chain: PersonWithAddr { addr: Address { zip, .. }, .. }.
        // Setting zip to its existing default value must not fire the inner
        // Address aggregate nor the outer PersonWithAddr aggregate.
        let mut pwa = PersonWithAddr::new(&mut sc);
        let outer_agg = pwa.inner.GetAggregateSignal();
        let inner_agg = pwa.addr.inner.GetAggregateSignal();

        pwa.addr.zip.SetValue(0, &mut sc);

        assert!(
            !sc.is_signaled(pwa.addr.zip.GetValueSignal()),
            "leaf signal must NOT fire on no-op SetValue"
        );
        assert!(
            !sc.is_signaled(inner_agg),
            "inner (Address) aggregate must NOT fire on no-op chain propagation"
        );
        assert!(
            !sc.is_signaled(outer_agg),
            "outer (PersonWithAddr) aggregate must NOT fire on no-op chain propagation"
        );
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn add_member_rejects_invalid_identifier_with_space() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut inner = emStructRec::new(&mut sc);
        let mut leaf = emIntRec::new(&mut sc, 0, i64::MIN, i64::MAX);
        inner.AddMember(&mut leaf, "has space");
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn add_member_rejects_identifier_with_leading_digit() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut inner = emStructRec::new(&mut sc);
        let mut leaf = emIntRec::new(&mut sc, 0, i64::MIN, i64::MAX);
        inner.AddMember(&mut leaf, "1leading_digit");
    }

    #[test]
    #[should_panic(expected = "is not a valid identifier")]
    fn add_member_rejects_empty_identifier() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut inner = emStructRec::new(&mut sc);
        let mut leaf = emIntRec::new(&mut sc, 0, i64::MIN, i64::MAX);
        inner.AddMember(&mut leaf, "");
    }
}
