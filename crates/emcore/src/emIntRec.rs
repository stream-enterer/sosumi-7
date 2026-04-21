//! emIntRec — concrete emRec<i64> with min/max bounds.
//!
//! C++ reference: `include/emCore/emRec.h:380` (`class emIntRec : public emRec`)
//! and `src/emCore/emRec.cpp:396-432` for constructor and Set body.
//!
//! Constructor (emRec.cpp:396-404): clamps maxValue to minValue if max < min,
//! then clamps defaultValue into [minValue, maxValue].
//!
//! Set / SetValue semantics (emRec.cpp:424-432):
//!   - Line 426: `if (value<MinValue) value=MinValue;`
//!   - Line 427: `if (value>MaxValue) value=MaxValue;`
//!   - Line 428: `if (Value!=value) { Value=value; Changed(); }`
//!
//! Clamp-before-compare: value is clamped to [min, max] FIRST, then compared
//! to the current value; mutation + signal only when the clamped value differs.
//!
//! DIVERGED: C++ uses `int` (32-bit). Rust uses `i64` per plan. No existing
//! Rust usage of emIntRec at this point; i64 is the plan-specified width.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emIntRec {
    value: i64,
    default: i64,
    min: i64,
    max: i64,
    signal: SignalId,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emIntRec {
    // TODO(phase-4b): emIntRec(parent, varIdentifier, default, min, max) per emRec.h:390
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: i64, min: i64, max: i64) -> Self {
        // Mirror C++ constructor clamping (emRec.cpp:398-400).
        let max = if max < min { min } else { max };
        let default = default.clamp(min, max);
        Self {
            value: default,
            default,
            min,
            max,
            signal: ctx.create_signal(),
        }
    }
}

impl emRecNode for emIntRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
}

impl emRec<i64> for emIntRec {
    fn GetValue(&self) -> &i64 {
        &self.value
    }

    /// C++ emIntRec::Set (emRec.cpp:424-432): clamp to [min, max], then skip
    /// mutation+signal when clamped value equals current value.
    fn SetValue(&mut self, value: i64, ctx: &mut SchedCtx<'_>) {
        let value = value.clamp(self.min, self.max);
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
        }
    }

    fn GetDefaultValue(&self) -> &i64 {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
    }

    fn GetMinValue(&self) -> Option<&i64> {
        Some(&self.min)
    }

    fn GetMaxValue(&self) -> Option<&i64> {
        Some(&self.max)
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
    fn set_value_fires_signal_when_changed() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emIntRec::new(&mut sc, 5, 0, 10);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(7, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(*rec.GetValue(), 7, "value must update to 7");

        sc.remove_signal(sig);
    }

    #[test]
    fn set_value_does_not_fire_when_unchanged() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emIntRec::new(&mut sc, 5, 0, 10);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(5, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(*rec.GetValue(), 5);
    }

    #[test]
    fn set_value_clamps_and_suppresses_when_clamped_equals_current() {
        // current = max = 10, SetValue(20) clamps to 10 = current → no fire.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emIntRec::new(&mut sc, 10, 0, 10);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(20, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire: 20 clamps to 10 = current"
        );
        assert_eq!(*rec.GetValue(), 10);
    }

    #[test]
    fn set_value_clamps_and_fires_when_clamped_differs() {
        // current = 5, max = 10, SetValue(20) clamps to 10, fires.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emIntRec::new(&mut sc, 5, 0, 10);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(20, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire: 20 clamps to 10 ≠ 5");
        assert_eq!(*rec.GetValue(), 10, "value must be clamped to max 10");

        sc.remove_signal(sig);
    }

    #[test]
    fn min_max_accessors_return_some() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emIntRec::new(&mut sc, 5, -100, 100);
        let sig = rec.GetValueSignal();

        assert_eq!(rec.GetMinValue(), Some(&-100i64));
        assert_eq!(rec.GetMaxValue(), Some(&100i64));

        // No signal fired; remove to avoid drop panic.
        sc.remove_signal(sig);
    }
}
