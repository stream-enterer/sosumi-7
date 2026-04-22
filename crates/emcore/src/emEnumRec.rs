//! emEnumRec — concrete emRec<u32> indexing into an identifier table.
//!
//! C++ reference: `include/emCore/emRec.h:543` (`class emEnumRec : public emRec`)
//! and `src/emCore/emRec.cpp:598-748` for constructor (Init) and Set body.
//!
//! Constructor / Init (emRec.cpp:727-748): identifiers collected, then default
//! clamped:
//!   - Line 744: `if (defaultValue<0) defaultValue=0;`
//!   - Line 745: `if (defaultValue>=IdentifierCount) defaultValue=IdentifierCount-1;`
//!
//! Set (emRec.cpp:631-639):
//!   - Line 633: `if (value<0) value=0;`
//!   - Line 634: `if (value>=IdentifierCount) value=IdentifierCount-1;`
//!   - Line 635: `if (Value!=value) { Value=value; Changed(); }`
//!
//! Clamp-before-compare: value clamped to [0, IdentifierCount-1] FIRST, then
//! compared to current; mutation + signal only when clamped value differs.
//!
//! Rust uses `u32` for the index (plan-specified width; C++ uses `int`).
//!
//! TODO(phase-4b+): GetIdentifier(index) per emRec.h:579-587 and
//!   GetIdentifierOf(value), GetValueOf(identifier) per emRec.h:585-591.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emEnumRec {
    value: u32,
    default: u32,
    // Stored for future GetIdentifier/GetIdentifierOf/GetValueOf accessors
    // (phase-4b+). Unused by current API paths — prefix suppresses dead-code lint.
    _identifiers: Vec<String>,
    /// Cached `identifiers.len() - 1` for O(1) GetMaxValue; set at construction.
    max_index: u32,
    signal: SignalId,
    /// Reified aggregate-signal chain; see ADR 2026-04-21-phase-4b-listener-tree-adr.md.
    aggregate_signals: Vec<SignalId>,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emEnumRec {
    // TODO(phase-4b): emEnumRec(parent, varIdentifier, default, identifiers...)
    //   per emRec.h:551-552
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: u32, identifiers: Vec<String>) -> Self {
        assert!(
            !identifiers.is_empty(),
            "emEnumRec: identifier table must not be empty"
        );
        // Mirror C++ Init clamping (emRec.cpp:744-745).
        let max_index = (identifiers.len() - 1) as u32;
        let default = default.min(max_index);
        Self {
            value: default,
            default,
            _identifiers: identifiers,
            max_index,
            signal: ctx.create_signal(),
            aggregate_signals: Vec::new(),
        }
    }
}

impl emRecNode for emEnumRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
    }
}

impl emRec<u32> for emEnumRec {
    fn GetValue(&self) -> &u32 {
        &self.value
    }

    /// C++ emEnumRec::Set (emRec.cpp:631-639): clamp to [0, IdentifierCount-1],
    /// then skip mutation+signal when clamped value equals current value.
    fn SetValue(&mut self, value: u32, ctx: &mut SchedCtx<'_>) {
        let value = value.min(self.max_index);
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
            // DIVERGED: C++ emRec::Changed() (emRec.h:243 inline, delegates to emRec::ChildChanged at emRec.cpp:217) walks UpperNode
            // per-fire; Rust fires the reified aggregate chain. See ADR
            // 2026-04-21-phase-4b-listener-tree-adr.md.
            for sig in &self.aggregate_signals {
                ctx.fire(*sig);
            }
        }
    }

    fn GetDefaultValue(&self) -> &u32 {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
    }

    fn GetMinValue(&self) -> Option<&u32> {
        Some(&0)
    }

    fn GetMaxValue(&self) -> Option<&u32> {
        Some(&self.max_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emClipboard::emClipboard;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
    use crate::emScheduler::EngineScheduler;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_sched_ctx<'a>(
        sched: &'a mut EngineScheduler,
        actions: &'a mut Vec<DeferredAction>,
        ctx_root: &'a Rc<crate::emContext::emContext>,
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

    fn ids() -> Vec<String> {
        vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
    }

    #[test]
    fn set_value_fires_on_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emEnumRec::new(&mut sc, 0, ids());
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(1, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(*rec.GetValue(), 1);

        sc.remove_signal(sig);
    }

    #[test]
    fn aggregate_signal_fires_on_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emEnumRec::new(&mut sc, 0, ids());
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue(1, &mut sc);

        assert!(sc.is_signaled(sig));
        assert!(sc.is_signaled(agg), "aggregate signal must fire");

        sc.remove_signal(sig);
        sc.remove_signal(agg);
    }

    #[test]
    fn aggregate_signal_does_not_fire_on_no_op() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emEnumRec::new(&mut sc, 1, ids());
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue(1, &mut sc);

        assert!(!sc.is_signaled(sig));
        assert!(!sc.is_signaled(agg), "aggregate must NOT fire on no-op");
    }

    #[test]
    fn set_value_suppresses_no_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emEnumRec::new(&mut sc, 1, ids());
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(1, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(*rec.GetValue(), 1);

        sc.remove_signal(sig);
    }

    #[test]
    fn set_value_clamps_out_of_range_and_suppresses_when_clamped_equals_current() {
        // identifiers len = 3, max_index = 2; current = 2 (last).
        // SetValue(99) clamps to 2 = current → no fire.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emEnumRec::new(&mut sc, 2, ids());
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(99, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire: 99 clamps to 2 = current"
        );
        assert_eq!(*rec.GetValue(), 2);

        sc.remove_signal(sig);
    }

    #[test]
    fn set_value_clamps_out_of_range_and_fires_when_clamped_differs() {
        // identifiers len = 3, max_index = 2; current = 0.
        // SetValue(99) clamps to 2 ≠ 0 → fires.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emEnumRec::new(&mut sc, 0, ids());
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(99, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire: 99 clamps to 2 ≠ 0");
        assert_eq!(*rec.GetValue(), 2, "value must be clamped to max_index 2");

        sc.remove_signal(sig);
    }

    #[test]
    fn get_min_max_bounds_reflect_identifier_table() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        // 3 identifiers: min = 0, max = 2
        let rec = emEnumRec::new(&mut sc, 0, ids());
        let sig = rec.GetValueSignal();

        assert_eq!(rec.GetMinValue(), Some(&0u32));
        assert_eq!(rec.GetMaxValue(), Some(&2u32));

        sc.remove_signal(sig);
    }
}
