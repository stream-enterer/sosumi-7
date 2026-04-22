//! emStringRec — concrete emRec<String>.
//!
//! C++ reference: `include/emCore/emRec.h:800` (`class emStringRec : public emRec`)
//! and `src/emCore/emRec.cpp:1050-1078` for constructor and Set body.
//!
//! No-change-skip contract (emRec.cpp:1074): `if (Value!=value)` — mutate and
//! signal only when the new value differs from the current value.
//! Constructor (emRec.cpp:1050-1052): both DefaultValue and Value initialised to
//! defaultValue; no null-to-empty transformation.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emStringRec {
    value: String,
    default: String,
    signal: SignalId,
    /// Reified aggregate-signal chain; see ADR 2026-04-21-phase-4b-listener-tree-adr.md.
    aggregate_signals: Vec<SignalId>,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emStringRec {
    // TODO(phase-4b): emStringRec(parent, varIdentifier, default) per emRec.h:807
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: String) -> Self {
        Self {
            value: default.clone(),
            default,
            signal: ctx.create_signal(),
            aggregate_signals: Vec::new(),
        }
    }
}

impl emRecNode for emStringRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
    }
}

impl emRec<String> for emStringRec {
    fn GetValue(&self) -> &String {
        &self.value
    }

    /// C++ emStringRec::Set (emRec.cpp:1072-1078): skip mutation+signal when unchanged.
    fn SetValue(&mut self, value: String, ctx: &mut SchedCtx<'_>) {
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

    fn GetDefaultValue(&self) -> &String {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
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

    #[test]
    fn set_value_fires_on_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emStringRec::new(&mut sc, "a".to_string());
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue("b".to_string(), &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(rec.GetValue(), "b", "value must be updated to b");

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

        let mut rec = emStringRec::new(&mut sc, "a".to_string());
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue("b".to_string(), &mut sc);

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

        let mut rec = emStringRec::new(&mut sc, "x".to_string());
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue("x".to_string(), &mut sc);

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

        let mut rec = emStringRec::new(&mut sc, "x".to_string());
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue("x".to_string(), &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(rec.GetValue(), "x", "value must remain x");
    }

    #[test]
    fn get_default_value_returns_original_default() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emStringRec::new(&mut sc, "orig".to_string());
        let sig = rec.GetValueSignal();

        rec.SetValue("changed".to_string(), &mut sc);

        assert_eq!(
            rec.GetDefaultValue(),
            "orig",
            "default must not change on SetValue"
        );

        sc.remove_signal(sig);
    }
}
