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
//! DIVERGED: (language-forced) C++ uses `int` (32-bit). Rust uses `i64` per plan. No existing
//! Rust usage of emIntRec at this point; i64 is the plan-specified width.

use crate::emEngineCtx::{ConstructCtx, SchedCtx};
use crate::emRec::emRec;
use crate::emRecNode::emRecNode;
use crate::emRecReader::{emRecReader, RecIoError};
use crate::emRecWriter::emRecWriter;
use crate::emSignal::SignalId;

pub struct emIntRec {
    value: i64,
    default: i64,
    min: i64,
    max: i64,
    signal: SignalId,
    /// Reified aggregate-signal chain; see ADR 2026-04-21-phase-4b-listener-tree-adr.md.
    aggregate_signals: Vec<SignalId>,
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
            aggregate_signals: Vec::new(),
        }
    }

    /// Port of C++ `emIntRec::TryStartWriting` (emRec.cpp:469-472).
    ///
    // DIVERGED: (language-forced) C++ splits persistence into the protected virtual pair
    // `TryStartWriting` + `TryContinueWriting`. Rust collapses into one
    // atomic call — same rationale as `emBoolRec::TryWrite`.
    // DIVERGED: (language-forced) Rust stores `i64` while C++ stores `int` (32-bit) and the
    // emRecWriter primitive writes `i32`. Values outside `i32` are clamped
    // to `i32::MIN..=i32::MAX` on write. In the current tree no emIntRec is
    // constructed with bounds outside the i32 range, so this clamp is
    // observable only for pathological callers.
    pub fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        let v = self.value.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        writer.TryWriteInt(v)
    }

    /// Port of C++ `emIntRec::TryStartReading` (emRec.cpp:447-455).
    ///
    // DIVERGED: (language-forced) name + fusion with TryContinueReading; see `TryWrite` above
    // for rationale. C++ bounds-checks the read value against `[MinValue,
    // MaxValue]` and calls `ThrowElemError("Number too small.")` /
    // `"Number too large."`; Rust mirrors those two error paths.
    pub fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        let i = reader.TryReadInt()? as i64;
        if i < self.min {
            return Err(reader.ThrowElemError("Number too small."));
        }
        if i > self.max {
            return Err(reader.ThrowElemError("Number too large."));
        }
        self.SetValue(i, ctx);
        Ok(())
    }
}

impl emRecNode for emIntRec {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    fn register_aggregate(&mut self, sig: SignalId) {
        self.aggregate_signals.push(sig);
    }

    fn listened_signal(&self) -> SignalId {
        self.signal
    }

    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        emIntRec::TryRead(self, reader, ctx)
    }

    fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        emIntRec::TryWrite(self, writer)
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
            // DIVERGED: (language-forced) C++ emRec::Changed() (emRec.h:243 inline, delegates to emRec::ChildChanged at emRec.cpp:217) walks UpperNode
            // per-fire; Rust fires the reified aggregate chain. See ADR
            // 2026-04-21-phase-4b-listener-tree-adr.md.
            for sig in &self.aggregate_signals {
                ctx.fire(*sig);
            }
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
    fn aggregate_signal_fires_on_change() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = crate::emContext::emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let mut rec = emIntRec::new(&mut sc, 5, 0, 10);
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue(7, &mut sc);

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

        let mut rec = emIntRec::new(&mut sc, 5, 0, 10);
        let sig = rec.GetValueSignal();
        let agg = sc.create_signal();
        rec.register_aggregate(agg);

        rec.SetValue(5, &mut sc);

        assert!(!sc.is_signaled(sig));
        assert!(!sc.is_signaled(agg), "aggregate must NOT fire on no-op");
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
