//! emColorRec — concrete emRec<emColor>.
//!
//! C++ reference: `include/emCore/emRec.h:864` (`class emColorRec : public emRec`)
//! and `src/emCore/emRec.cpp:1162-1169` for the Set body.
//!
//! Alpha-forcing contract (emRec.cpp:1164): `if (!HaveAlpha) value.SetAlpha(255);`
//! runs BEFORE the `if (Value!=value)` equality check — so setting a color that
//! differs from the stored value only in alpha, when have_alpha==false, is a
//! no-op after normalization and does NOT fire the signal.

use crate::emColor::emColor;
use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emSignal::SignalId;

pub struct emColorRec {
    value: emColor,
    default: emColor,
    have_alpha: bool,
    signal: SignalId,
    // TODO(phase-4b+): SetToDefault, IsSetToDefault, TryStartReading, serialization hooks per emRec.h.
}

impl emColorRec {
    // TODO(phase-4b): emColorRec(parent, varIdentifier, default, haveAlpha) per emRec.h:872.
    /// C++ emColorRec ctor (emRec.cpp:1140-1146): if !haveAlpha, force default's alpha to 255 before storing.
    pub fn new<C: ConstructCtx>(ctx: &mut C, default: emColor, have_alpha: bool) -> Self {
        let default = if !have_alpha {
            default.SetAlpha(255)
        } else {
            default
        };
        Self {
            value: default,
            default,
            have_alpha,
            signal: ctx.create_signal(),
        }
    }

    pub fn HaveAlpha(&self) -> bool {
        self.have_alpha
    }
}

impl emRecNode for emColorRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
}

impl emRec<emColor> for emColorRec {
    fn GetValue(&self) -> &emColor {
        &self.value
    }

    /// C++ emColorRec::Set (emRec.cpp:1162-1169): force alpha to 255 when
    /// !HaveAlpha BEFORE the equality check, then skip mutation+signal when unchanged.
    fn SetValue(&mut self, value: emColor, ctx: &mut SchedCtx<'_>) {
        let value = if !self.have_alpha {
            value.SetAlpha(255)
        } else {
            value
        };
        if value != self.value {
            self.value = value;
            ctx.fire(self.signal);
        }
    }

    fn GetDefaultValue(&self) -> &emColor {
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
    fn set_value_fires_signal() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emColorRec::new(&mut sc, emColor::BLACK, true);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(emColor::RED, &mut sc);

        assert!(sc.is_signaled(sig), "signal must fire when value changes");
        assert_eq!(*rec.GetValue(), emColor::RED);

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

        let mut rec = emColorRec::new(&mut sc, emColor::BLUE, true);
        let sig = rec.GetValueSignal();

        assert!(!sc.is_signaled(sig));

        rec.SetValue(emColor::BLUE, &mut sc);

        assert!(
            !sc.is_signaled(sig),
            "signal must NOT fire when value is unchanged"
        );
        assert_eq!(*rec.GetValue(), emColor::BLUE);
    }

    #[test]
    fn ctor_forces_default_alpha_to_255_when_no_alpha() {
        // C++ emColorRec ctor (emRec.cpp:1142): if (!haveAlpha) defaultValue.SetAlpha(255);
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let rec = emColorRec::new(&mut sc, emColor::rgba(10, 20, 30, 0x80), false);
        assert_eq!(rec.GetValue().GetAlpha(), 255);
        assert_eq!(rec.GetDefaultValue().GetAlpha(), 255);
    }

    #[test]
    fn set_value_forces_alpha_to_255_when_no_alpha() {
        // C++ emColorRec::Set (emRec.cpp:1164): if (!HaveAlpha) value.SetAlpha(255);
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emColorRec::new(&mut sc, emColor::BLACK, false);
        let sig = rec.GetValueSignal();

        rec.SetValue(emColor::rgba(10, 20, 30, 0x80), &mut sc);

        assert_eq!(
            rec.GetValue().GetAlpha(),
            255,
            "alpha must be forced to 255"
        );
        assert_eq!(*rec.GetValue(), emColor::rgba(10, 20, 30, 255));
        assert!(sc.is_signaled(sig));

        sc.remove_signal(sig);
    }

    #[test]
    fn alpha_only_diff_does_not_fire_when_no_alpha() {
        // C++ ordering: alpha-force happens BEFORE equality check. Two colors
        // with the same RGB but different alpha both normalize to alpha=255,
        // so the second SetValue is a no-op.
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emColorRec::new(&mut sc, emColor::BLACK, false);
        let sig = rec.GetValueSignal();

        rec.SetValue(emColor::rgba(10, 20, 30, 0x80), &mut sc);
        assert!(sc.is_signaled(sig), "first change must fire");
        sc.remove_signal(sig);
        assert!(!sc.is_signaled(sig));

        rec.SetValue(emColor::rgba(10, 20, 30, 0x40), &mut sc);
        assert!(
            !sc.is_signaled(sig),
            "alpha-only diff must not fire when have_alpha=false"
        );
        assert_eq!(rec.GetValue().GetAlpha(), 255);
    }

    #[test]
    fn have_alpha_true_preserves_alpha() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emColorRec::new(&mut sc, emColor::TRANSPARENT, true);
        let sig = rec.GetValueSignal();

        rec.SetValue(emColor::rgba(10, 20, 30, 0x80), &mut sc);

        assert_eq!(rec.GetValue().GetAlpha(), 0x80);
        assert_eq!(*rec.GetValue(), emColor::rgba(10, 20, 30, 0x80));
        assert!(sc.is_signaled(sig));

        sc.remove_signal(sig);
    }
}
