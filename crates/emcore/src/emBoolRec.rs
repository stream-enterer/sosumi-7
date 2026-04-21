//! emBoolRec — concrete emRec<bool>.
//!
//! C++ reference: `include/emCore/emRec.h:316` (`class emBoolRec : public emRec`)
//! and `src/emCore/emRec.cpp:306-312` for the Set (SetValue) body.
//!
//! No-change-skip contract (emRec.cpp:308): `if (Value!=value)` — mutate and
//! signal only when the new value differs from the current value.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emBoolRec {
    value: bool,
    default: bool,
    signal: SignalId,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emBoolRec {
    // TODO(phase-4b): emBoolRec(parent, varIdentifier, default) per emRec.h:323
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: bool) -> Self {
        Self {
            value: default,
            default,
            signal: ctx.create_signal(),
        }
    }
}

impl emRecNode for emBoolRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
}

impl emRec<bool> for emBoolRec {
    fn GetValue(&self) -> &bool {
        &self.value
    }

    /// C++ emBoolRec::Set (emRec.cpp:306-312): skip mutation+signal when unchanged.
    fn SetValue(&mut self, value: bool, ctx: &mut SchedCtx<'_>) {
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
        }
    }

    fn GetDefaultValue(&self) -> &bool {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
    }

    // TODO(phase-4b+): Invert() per emRec.cpp:315-319.
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
    fn set_value_fires_signal() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emBoolRec::new(&mut sc, false);
        let sig = rec.GetValueSignal();

        // Signal must not be pending at construction.
        assert!(!sc.is_signaled(sig));

        rec.SetValue(true, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert!(*rec.GetValue(), "value must be updated to true");

        // Clean up: remove pending signal so EngineScheduler doesn't panic on drop.
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

        let mut rec = emBoolRec::new(&mut sc, true);
        let sig = rec.GetValueSignal();

        // Signal must not be pending at construction.
        assert!(!sc.is_signaled(sig));

        rec.SetValue(true, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert!(*rec.GetValue(), "value must remain true");
    }
}
