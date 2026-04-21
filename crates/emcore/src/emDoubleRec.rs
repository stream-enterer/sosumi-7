//! emDoubleRec — concrete emRec<f64> with min/max bounds.
//!
//! C++ reference: `include/emCore/emRec.h:460` (`class emDoubleRec : public emRec`)
//! and `src/emCore/emRec.cpp:496-537` for constructor and Set body.
//!
//! Constructor (emRec.cpp:496-505): clamps maxValue to minValue if max < min,
//! then clamps defaultValue into [minValue, maxValue].
//!
//! Set / SetValue semantics (emRec.cpp:529-537):
//!   - Line 531: `if (value<MinValue) value=MinValue;`
//!   - Line 532: `if (value>MaxValue) value=MaxValue;`
//!   - Line 533: `if (Value!=value) { Value=value; Changed(); }`
//!
//! Clamp-before-compare: value is clamped to [min, max] FIRST, then compared
//! to the current value; mutation + signal only when the clamped value differs.
//!
//! NaN behavior: C++ uses `<` / `>` comparisons which return false for NaN,
//! so NaN passes through both clamp guards unchanged. Then `Value != value`
//! with NaN (IEEE 754: NaN != NaN is true) would fire Changed() on every call.
//! We replicate this by using explicit `<` / `>` guards rather than f64::clamp.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emDoubleRec {
    value: f64,
    default: f64,
    min: f64,
    max: f64,
    signal: SignalId,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emDoubleRec {
    // TODO(phase-4b): emDoubleRec(parent, varIdentifier, default, min, max) per emRec.h:468
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: f64, min: f64, max: f64) -> Self {
        // Mirror C++ constructor clamping (emRec.cpp:498-500).
        let max = if max < min { min } else { max };
        let default = if default < min {
            min
        } else if default > max {
            max
        } else {
            default
        };
        Self {
            value: default,
            default,
            min,
            max,
            signal: ctx.create_signal(),
        }
    }
}

impl emRecNode for emDoubleRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
}

impl emRec<f64> for emDoubleRec {
    fn GetValue(&self) -> &f64 {
        &self.value
    }

    /// C++ emDoubleRec::Set (emRec.cpp:529-537): clamp to [min, max] via
    /// explicit `<`/`>` guards (not f64::clamp), then skip mutation+signal
    /// when clamped value equals current value.
    fn SetValue(&mut self, mut value: f64, ctx: &mut SchedCtx<'_>) {
        // Replicate C++ lines 531-532 exactly: `<` / `>` guards, not clamp().
        if value < self.min {
            value = self.min;
        }
        if value > self.max {
            value = self.max;
        }
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
        }
    }

    fn GetDefaultValue(&self) -> &f64 {
        &self.default
    }

    fn GetValueSignal(&self) -> SignalId {
        self.signal
    }

    fn GetMinValue(&self) -> Option<&f64> {
        Some(&self.min)
    }

    fn GetMaxValue(&self) -> Option<&f64> {
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

        let mut rec = emDoubleRec::new(&mut sc, 5.0, 0.0, 10.0);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(7.5, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(*rec.GetValue(), 7.5, "value must update to 7.5");

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

        let mut rec = emDoubleRec::new(&mut sc, 5.0, 0.0, 10.0);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(5.0, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(*rec.GetValue(), 5.0);
    }

    #[test]
    fn set_value_clamps_and_suppresses_when_clamped_equals_current() {
        // current = max = 10.0, SetValue(20.0) clamps to 10.0 = current → no fire.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emDoubleRec::new(&mut sc, 10.0, 0.0, 10.0);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(20.0, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire: 20.0 clamps to 10.0 = current"
        );
        assert_eq!(*rec.GetValue(), 10.0);
    }

    #[test]
    fn set_value_clamps_and_fires_when_clamped_differs() {
        // current = 5.0, max = 10.0, SetValue(20.0) clamps to 10.0, fires.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emDoubleRec::new(&mut sc, 5.0, 0.0, 10.0);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(20.0, &mut sc);

        assert!(
            sc.is_signaled(sig),
            "signal must fire: 20.0 clamps to 10.0 ≠ 5.0"
        );
        assert_eq!(*rec.GetValue(), 10.0, "value must be clamped to max 10.0");

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

        let rec = emDoubleRec::new(&mut sc, 5.0, -100.0, 100.0);
        let sig = rec.GetValueSignal();

        assert_eq!(rec.GetMinValue(), Some(&-100.0f64));
        assert_eq!(rec.GetMaxValue(), Some(&100.0f64));

        // No signal fired; remove to avoid drop panic.
        sc.remove_signal(sig);
    }
}
