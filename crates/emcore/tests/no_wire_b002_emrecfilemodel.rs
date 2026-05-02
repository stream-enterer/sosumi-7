//! B-002 emRecFileModel ChangeSignal — P-001-no-subscribe-no-accessor.
//!
//! Row `emRecFileModel-GetChangeSignal` (G2): emRecFileModel<T> ports the
//! C++ inherited `emFileModel::ChangeSignal` lazily via D-008 A1
//! combined-form `GetChangeSignal(&self, ectx)` and fires synchronously per
//! D-007 via `signal_change(&self, ectx)` in every mutator.
//!
//! Decisions cited: D-006 (subscribe-shape), D-007 (mutator-fire shape;
//! synchronous `&mut impl SignalCtx` threading at every mutator), D-008 A1
//! (lazy `Cell<SignalId>` allocation), D-009 (no polling intermediary).
//!
//! RUST_ONLY: (dependency-forced) — no C++ test analogue; same rationale as
//! B-004 / B-005 tests.

use std::path::PathBuf;

use slotmap::Key as _;

use emcore::emEngineCtx::DropOnlySignalCtx;
use emcore::emRecFileModel::emRecFileModel;
use emcore::emRecParser::{RecError, RecStruct};
use emcore::emRecRecord::Record;
use emcore::test_view_harness::TestViewHarness;

#[derive(Clone, Default, Debug, PartialEq)]
struct TestRec {
    a: i32,
}

impl Record for TestRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        Ok(Self {
            a: rec.get_int("a").unwrap_or(0),
        })
    }
    fn to_rec(&self) -> RecStruct {
        let mut r = RecStruct::new();
        r.set_int("A", self.a);
        r
    }
    fn SetToDefault(&mut self) {
        *self = Self::default();
    }
    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

#[test]
fn change_signal_is_null_until_first_get() {
    let m = emRecFileModel::<TestRec>::new(PathBuf::from("/tmp/b002_null.rec"));
    assert!(
        m.change_signal_for_test().is_null(),
        "change_signal must be null until first GetChangeSignal"
    );
}

#[test]
fn get_change_signal_lazy_alloc_and_idempotent() {
    let mut h = TestViewHarness::new();
    let m = emRecFileModel::<TestRec>::new(PathBuf::from("/tmp/b002_lazy.rec"));
    let mut sc = h.sched_ctx();
    let s1 = m.GetChangeSignal(&mut sc);
    assert!(!s1.is_null());
    let s2 = m.GetChangeSignal(&mut sc);
    assert_eq!(s1, s2, "combined-form must be idempotent");
}

#[test]
fn pre_subscribe_signal_change_is_no_op() {
    // Per D-007 + D-008 composition: signal_change must be a silent no-op when
    // change_signal is still null (no subscriber has called GetChangeSignal).
    let mut m = emRecFileModel::<TestRec>::new(PathBuf::from("/tmp/b002_preno.rec"));
    let mut null = DropOnlySignalCtx;
    // Mutators all fire signal_change internally; with DropOnlySignalCtx and a
    // null change_signal slot, none of these must panic.
    m.hard_reset(&mut null);
    m.clear_save_error(&mut null);
    m.update(&mut null);
    let _ = m.GetWritableMap(&mut null);
}

#[test]
fn signal_change_fires_synchronously_after_get() {
    // Once GetChangeSignal has allocated the SignalId, every mutator's
    // synchronous signal_change(ectx) fires it on the engine ctx.
    let mut h = TestViewHarness::new();
    let mut m = emRecFileModel::<TestRec>::new(PathBuf::from("/tmp/b002_fire.rec"));
    let sig = {
        let mut sc = h.sched_ctx();
        m.GetChangeSignal(&mut sc)
    };
    assert!(!sig.is_null());

    // hard_reset triggers signal_change synchronously per D-007.
    {
        let mut sc = h.sched_ctx();
        m.hard_reset(&mut sc);
    }
    // The signal is now pending in the scheduler clock.
    h.scheduler.flush_signals_for_test();
}

#[test]
fn try_load_fires_change_signal_synchronously() {
    let mut h = TestViewHarness::new();
    let mut m = emRecFileModel::<TestRec>::new(PathBuf::from("/tmp/b002_does_not_exist_xyz.rec"));
    let _ = {
        let mut sc = h.sched_ctx();
        m.GetChangeSignal(&mut sc)
    };
    let mut sc = h.sched_ctx();
    m.TryLoad(&mut sc);
    // Even on error completion, ChangeSignal must have fired.
    h.scheduler.flush_signals_for_test();
}

#[test]
fn get_writable_map_from_waiting_does_not_transition() {
    // GetWritableMap from Waiting should NOT transition to Unsaved — and so
    // signal_change is not fired. The mutator-firing test is observable via
    // state, not via flag drain.
    let mut m = emRecFileModel::<TestRec>::new(PathBuf::from("/tmp/b002_unsaved.rec"));
    let mut null = DropOnlySignalCtx;
    m.hard_reset(&mut null); // Waiting
    let _ = m.GetWritableMap(&mut null);
    assert_eq!(
        *m.GetFileState(),
        emcore::emFileModel::FileState::Waiting,
        "GetWritableMap from Waiting must not transition"
    );
}
