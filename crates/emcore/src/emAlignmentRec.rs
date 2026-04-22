//! emAlignmentRec — concrete emRec<emAlignment>.
//!
//! C++ reference: `include/emCore/emRec.h:735` (`class emAlignmentRec : public emRec`)
//! and `src/emCore/emRec.cpp:946-952` for the Set body.
//!
//! No-change-skip contract (emRec.cpp:948): `if (Value!=value)` — mutate and
//! signal only when the new value differs from the current value. Unlike
//! emFlagsRec, emAlignmentRec does NOT mask undefined bits in Set; the raw
//! `emAlignment` byte is stored verbatim.

use crate::emAlignment::emAlignment;
use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emAlignmentRec {
    value: emAlignment,
    default: emAlignment,
    signal: SignalId,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emAlignmentRec {
    // TODO(phase-4b): emAlignmentRec(parent, varIdentifier, default) per emRec.h:742.
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: emAlignment) -> Self {
        Self {
            value: default,
            default,
            signal: ctx.create_signal(),
        }
    }
}

impl emRecNode for emAlignmentRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
}

impl emRec<emAlignment> for emAlignmentRec {
    fn GetValue(&self) -> &emAlignment {
        &self.value
    }

    /// C++ emAlignmentRec::Set (emRec.cpp:946-952): skip mutation+signal when unchanged.
    fn SetValue(&mut self, value: emAlignment, ctx: &mut SchedCtx<'_>) {
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
        }
    }

    fn GetDefaultValue(&self) -> &emAlignment {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emAlignment::{EM_ALIGN_CENTER, EM_ALIGN_TOP, EM_ALIGN_TOP_LEFT};
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
    fn set_value_fires_signal() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emAlignmentRec::new(&mut sc, EM_ALIGN_CENTER);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(EM_ALIGN_TOP, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(*rec.GetValue(), EM_ALIGN_TOP);

        sc.remove_signal(sig);
    }

    #[test]
    fn set_to_same_value_does_not_fire() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emAlignmentRec::new(&mut sc, EM_ALIGN_TOP_LEFT);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(EM_ALIGN_TOP_LEFT, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(*rec.GetValue(), EM_ALIGN_TOP_LEFT);
    }

    #[test]
    fn default_value_is_stored_and_retained() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emAlignmentRec::new(&mut sc, EM_ALIGN_TOP_LEFT);
        assert_eq!(*rec.GetDefaultValue(), EM_ALIGN_TOP_LEFT);
        assert_eq!(*rec.GetValue(), EM_ALIGN_TOP_LEFT);
    }

    #[test]
    fn set_preserves_raw_bits_no_masking() {
        // C++ emAlignmentRec::Set (emRec.cpp:946-952) does NOT mask — unlike
        // emFlagsRec::Set. Upper bits pass through verbatim.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emAlignmentRec::new(&mut sc, EM_ALIGN_CENTER);
        let sig = rec.GetValueSignal();

        rec.SetValue(0xF0, &mut sc);

        assert_eq!(*rec.GetValue(), 0xF0, "upper bits must pass through");
        assert!(sc.is_signaled(sig));

        sc.remove_signal(sig);
    }
}
