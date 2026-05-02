//! FU-005: emRecFileModel file-state-signal lifecycle and fire coverage.
//!
//! Phase 1 establishes that `GetFileStateSignal` returns a real (non-null)
//! id after `ensure_file_state_signal` is called at first Cycle. Phase 2
//! adds the fire-side coverage (separate test functions below).
//!
//! RUST_ONLY: (dependency-forced) — no C++ test analogue; same rationale as
//! `no_wire_b002_emrecfilemodel`.

use std::path::PathBuf;

use slotmap::Key as _;

use emcore::emRecFileModel::emRecFileModel;
use emcore::emRecParser::{RecError, RecStruct};
use emcore::emRecRecord::Record;
use emcore::test_view_harness::TestViewHarness;

#[derive(Clone, Default, Debug, PartialEq)]
struct DummyRec {
    a: i32,
}

impl Record for DummyRec {
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
fn get_file_state_signal_returns_null_until_ensured() {
    use emcore::emFileModel::FileModelState;

    let m = emRecFileModel::<DummyRec>::new(PathBuf::from("/tmp/fu005_null.rec"));

    // Pre-ensure: trait impl returns null per the lazy invariant.
    assert!(
        m.GetFileStateSignal().is_null(),
        "FileStateSignal must be null before ensure_file_state_signal is called"
    );
    assert!(
        m.file_state_signal_for_test().is_null(),
        "raw cell slot must be null pre-ensure"
    );

    // Promote via the lazy accessor.
    let mut h = TestViewHarness::new();
    let id1 = {
        let mut sc = h.sched_ctx();
        m.ensure_file_state_signal(&mut sc)
    };
    assert!(!id1.is_null(), "ensure_file_state_signal must return a real id");

    // Idempotent: second call returns the same id.
    let id2 = {
        let mut sc = h.sched_ctx();
        m.ensure_file_state_signal(&mut sc)
    };
    assert_eq!(id1, id2, "ensure_file_state_signal must be idempotent");

    // Trait impl now returns the real id.
    assert_eq!(
        m.GetFileStateSignal(),
        id1,
        "post-ensure, GetFileStateSignal must return the live id"
    );
}

#[test]
fn try_load_fires_file_state_signal() {
    // TryLoad on a non-existent path transitions to LoadError; per FU-005,
    // both ChangeSignal and FileStateSignal must fire on the transition.
    let mut h = TestViewHarness::new();
    let mut m = emRecFileModel::<DummyRec>::new(PathBuf::from(
        "/tmp/fu005_does_not_exist_xyz_load.rec",
    ));

    // Promote both signals (mirrors first-Cycle subscriber wiring).
    let (change_sig, state_sig) = {
        let mut sc = h.sched_ctx();
        let c = m.GetChangeSignal(&mut sc);
        let s = m.ensure_file_state_signal(&mut sc);
        (c, s)
    };
    assert!(!change_sig.is_null());
    assert!(!state_sig.is_null());

    // Drive TryLoad — fires happen synchronously via signal_change /
    // signal_file_state, marking both signals pending.
    {
        let mut sc = h.sched_ctx();
        m.TryLoad(&mut sc);
    }

    assert!(
        h.scheduler.is_pending(change_sig),
        "TryLoad must fire ChangeSignal (existing behavior)"
    );
    assert!(
        h.scheduler.is_pending(state_sig),
        "TryLoad must fire FileStateSignal (FU-005 new behavior)"
    );

    // Clean up pending signals so the scheduler drop-asserts pass.
    h.scheduler.flush_signals_for_test();
}

#[test]
fn hard_reset_fires_file_state_signal() {
    // hard_reset transitions any state → Waiting; FU-005 requires both signals fire.
    let mut h = TestViewHarness::new();
    let mut m = emRecFileModel::<DummyRec>::new(PathBuf::from("/tmp/fu005_hard_reset.rec"));

    let (change_sig, state_sig) = {
        let mut sc = h.sched_ctx();
        let c = m.GetChangeSignal(&mut sc);
        let s = m.ensure_file_state_signal(&mut sc);
        (c, s)
    };

    {
        let mut sc = h.sched_ctx();
        m.hard_reset(&mut sc);
    }

    assert!(h.scheduler.is_pending(change_sig));
    assert!(
        h.scheduler.is_pending(state_sig),
        "hard_reset must fire FileStateSignal (FU-005)"
    );

    h.scheduler.flush_signals_for_test();
}
