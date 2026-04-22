//! Phase 4d Task 3 — round-trip tests for every concrete primitive emRec
//! type's `TryRead` / `TryWrite` pair, plus all four compound types
//! (emUnionRec, emArrayRec, emTArrayRec, emStructRec) exercised through
//! the widened `emRecNode` trait.
//!
//! Byte-stability contract: for each type, write → read → write must produce
//! identical bytes. Every test below asserts this.

use emcore::emAlignment::{EM_ALIGN_BOTTOM, EM_ALIGN_RIGHT, EM_ALIGN_TOP_LEFT};
use emcore::emAlignmentRec::emAlignmentRec;
use emcore::emArrayRec::emArrayRec;
use emcore::emBoolRec::emBoolRec;
use emcore::emClipboard::emClipboard;
use emcore::emColor::emColor;
use emcore::emColorRec::emColorRec;
use emcore::emContext::emContext;
use emcore::emDoubleRec::emDoubleRec;
use emcore::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
use emcore::emEnumRec::emEnumRec;
use emcore::emFlagsRec::emFlagsRec;
use emcore::emIntRec::emIntRec;
use emcore::emRec::emRec;
use emcore::emRecMemReader::emRecMemReader;
use emcore::emRecMemWriter::emRecMemWriter;
use emcore::emRecNode::emRecNode;
use emcore::emRecReader::RecIoError;
use emcore::emRecWriter::emRecWriter;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;
use emcore::emStringRec::emStringRec;
use emcore::emStructRec::emStructRec;
use emcore::emTArrayRec::{emTArrayRec, emTRecAllocator};
use emcore::emUnionRec::emUnionRec;
use std::cell::RefCell;
use std::rc::Rc;

fn make_sched_ctx<'a>(
    sched: &'a mut EngineScheduler,
    actions: &'a mut Vec<DeferredAction>,
    ctx_root: &'a Rc<emContext>,
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

/// Owned scheduler bundle (keeps all SchedCtx borrows alive for a whole test).
struct Fixture {
    sched: EngineScheduler,
    actions: Vec<DeferredAction>,
    ctx_root: Rc<emContext>,
    cb: RefCell<Option<Box<dyn emClipboard>>>,
    pa: Rc<RefCell<Vec<FrameworkDeferredAction>>>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            sched: EngineScheduler::new(),
            actions: Vec::new(),
            ctx_root: emContext::NewRoot(),
            cb: RefCell::new(None),
            pa: Rc::new(RefCell::new(Vec::new())),
        }
    }
    fn sc(&mut self) -> SchedCtx<'_> {
        make_sched_ctx(
            &mut self.sched,
            &mut self.actions,
            &self.ctx_root,
            &self.cb,
            &self.pa,
        )
    }
}

#[test]
fn int_rec_roundtrip() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut rec = emIntRec::new(&mut sc, 0, -100, 100);
    rec.SetValue(42, &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"42");

    let mut sc = fx.sc();
    let mut rec2 = emIntRec::new(&mut sc, 0, -100, 100);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    let s1 = rec.GetValueSignal();
    let s2 = rec2.GetValueSignal();
    sc.scheduler.abort(s1);
    sc.scheduler.abort(s2);
    sc.remove_signal(s1);
    sc.remove_signal(s2);
}

#[test]
fn int_rec_rejects_out_of_range() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let mut rec = emIntRec::new(&mut sc, 0, 0, 10);
    let mut r = emRecMemReader::new(b"999");
    let err = rec.TryRead(&mut r, &mut sc).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");

    let mut r = emRecMemReader::new(b"-1");
    let err = rec.TryRead(&mut r, &mut sc).unwrap_err().to_string();
    assert!(err.contains("too small"), "{err}");

    let s = rec.GetValueSignal();
    sc.scheduler.abort(s);
    sc.remove_signal(s);
}

#[test]
fn double_rec_roundtrip() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut rec = emDoubleRec::new(&mut sc, 0.0, -1e6, 1e6);
    // Arbitrary non-integer double — exercises the %.9G formatter on a
    // fractional value. Not a mathematical constant; the test cares only
    // about byte-stability of the round-trip, not the specific value.
    rec.SetValue(1234.5678_f64, &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();

    let mut sc = fx.sc();
    let mut rec2 = emDoubleRec::new(&mut sc, 0.0, -1e6, 1e6);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    let s1 = rec.GetValueSignal();
    let s2 = rec2.GetValueSignal();
    sc.scheduler.abort(s1);
    sc.scheduler.abort(s2);
    sc.remove_signal(s1);
    sc.remove_signal(s2);
}

#[test]
fn string_rec_roundtrip() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut rec = emStringRec::new(&mut sc, String::new());
    rec.SetValue("hello \"world\"\n".to_string(), &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();

    let mut sc = fx.sc();
    let mut rec2 = emStringRec::new(&mut sc, String::new());
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    let s1 = rec.GetValueSignal();
    let s2 = rec2.GetValueSignal();
    sc.scheduler.abort(s1);
    sc.scheduler.abort(s2);
    sc.remove_signal(s1);
    sc.remove_signal(s2);
}

#[test]
fn enum_rec_roundtrip() {
    let mut fx = Fixture::new();
    let ids = || vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];

    let mut sc = fx.sc();
    let mut rec = emEnumRec::new(&mut sc, 0, ids());
    rec.SetValue(2, &mut sc); // "gamma"
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"gamma");

    let mut sc = fx.sc();
    let mut rec2 = emEnumRec::new(&mut sc, 0, ids());
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    // Int-form read also works (C++ emRec.cpp:678-683).
    let mut sc = fx.sc();
    let mut rec3 = emEnumRec::new(&mut sc, 0, ids());
    let mut r = emRecMemReader::new(b"1");
    rec3.TryRead(&mut r, &mut sc).unwrap();
    assert_eq!(*rec3.GetValue(), 1);
    let _ = sc;

    let mut sc = fx.sc();
    for s in [
        rec.GetValueSignal(),
        rec2.GetValueSignal(),
        rec3.GetValueSignal(),
    ] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn enum_rec_rejects_unknown_identifier() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let mut rec = emEnumRec::new(&mut sc, 0, vec!["alpha".to_string(), "beta".to_string()]);
    let mut r = emRecMemReader::new(b"gamma");
    let err = rec.TryRead(&mut r, &mut sc).unwrap_err().to_string();
    assert!(err.contains("Unknown identifier"), "{err}");

    let s = rec.GetValueSignal();
    sc.scheduler.abort(s);
    sc.remove_signal(s);
}

#[test]
fn flags_rec_roundtrip_multiple_bits() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut rec = emFlagsRec::new(&mut sc, 0, &["read", "write", "exec"]);
    rec.SetValue(0b101, &mut sc); // read + exec
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"{read exec}");

    let mut sc = fx.sc();
    let mut rec2 = emFlagsRec::new(&mut sc, 0, &["read", "write", "exec"]);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    for s in [rec.GetValueSignal(), rec2.GetValueSignal()] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn flags_rec_empty_set_roundtrip() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let rec = emFlagsRec::new(&mut sc, 0, &["a", "b"]);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"{}");

    let mut sc = fx.sc();
    let mut rec2 = emFlagsRec::new(&mut sc, 0b11, &["a", "b"]);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    assert_eq!(*rec2.GetValue(), 0);
    let _ = sc;

    let mut sc = fx.sc();
    for s in [rec.GetValueSignal(), rec2.GetValueSignal()] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn alignment_rec_roundtrip_combo() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut rec = emAlignmentRec::new(&mut sc, 0);
    rec.SetValue(EM_ALIGN_TOP_LEFT, &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    // Top bit emits first, then left — joined by `-`.
    assert_eq!(bytes.as_slice(), b"top-left");

    let mut sc = fx.sc();
    let mut rec2 = emAlignmentRec::new(&mut sc, 0);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    for s in [rec.GetValueSignal(), rec2.GetValueSignal()] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn alignment_rec_center_when_no_bits_set() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let rec = emAlignmentRec::new(&mut sc, 0);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    assert_eq!(w.into_bytes().as_slice(), b"center");

    let mut sc = fx.sc();
    let mut rec2 = emAlignmentRec::new(&mut sc, EM_ALIGN_BOTTOM | EM_ALIGN_RIGHT);
    let mut r = emRecMemReader::new(b"center");
    rec2.TryRead(&mut r, &mut sc).unwrap();
    assert_eq!(*rec2.GetValue(), 0);
    let _ = sc;

    let mut sc = fx.sc();
    for s in [rec.GetValueSignal(), rec2.GetValueSignal()] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn color_rec_roundtrip_rgb() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let mut rec = emColorRec::new(&mut sc, emColor::BLACK, false);
    rec.SetValue(emColor::rgba(0x11, 0x22, 0x33, 255), &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"{17 34 51}");

    let mut sc = fx.sc();
    let mut rec2 = emColorRec::new(&mut sc, emColor::BLACK, false);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    for s in [rec.GetValueSignal(), rec2.GetValueSignal()] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn color_rec_roundtrip_rgba() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let mut rec = emColorRec::new(&mut sc, emColor::BLACK, true);
    rec.SetValue(emColor::rgba(10, 20, 30, 128), &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    rec.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"{10 20 30 128}");

    let mut sc = fx.sc();
    let mut rec2 = emColorRec::new(&mut sc, emColor::BLACK, true);
    let mut r = emRecMemReader::new(&bytes);
    rec2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(rec2.GetValue(), rec.GetValue());

    let mut w2 = emRecMemWriter::new();
    rec2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    for s in [rec.GetValueSignal(), rec2.GetValueSignal()] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}

#[test]
fn color_rec_accepts_quoted_hex_form() {
    // C++ emRec.cpp:1191-1198 reads a quoted string via emColor::TryParse.
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let mut rec = emColorRec::new(&mut sc, emColor::BLACK, true);
    let mut r = emRecMemReader::new(b"\"#112233\"");
    rec.TryRead(&mut r, &mut sc).unwrap();
    assert_eq!(*rec.GetValue(), emColor::rgba(0x11, 0x22, 0x33, 255));

    let s = rec.GetValueSignal();
    sc.scheduler.abort(s);
    sc.remove_signal(s);
}

#[test]
fn color_rec_rejects_channel_out_of_range() {
    let mut fx = Fixture::new();
    let mut sc = fx.sc();
    let mut rec = emColorRec::new(&mut sc, emColor::BLACK, false);
    let mut r = emRecMemReader::new(b"{300 0 0}");
    let err = rec.TryRead(&mut r, &mut sc).unwrap_err().to_string();
    assert!(err.contains("out of range"), "{err}");

    let s = rec.GetValueSignal();
    sc.scheduler.abort(s);
    sc.remove_signal(s);
}

// ---------------------------------------------------------------------------
// Compound round-trips — exercise the widened emRecNode::TryRead/TryWrite
// through boxed children and sibling-field dispatch.
// ---------------------------------------------------------------------------

/// Person mirrors the C++ `Person` example (emRec.h:78-108), ported in
/// Phase 4c. Duplicated from the compound integration test to keep this
/// file self-contained (CLAUDE.md test-scaffold duplication policy).
struct Person {
    inner: emStructRec,
    name: emStringRec,
    age: emIntRec,
    male: emBoolRec,
}

impl Person {
    fn new(ctx: &mut SchedCtx<'_>) -> Self {
        let mut inner = emStructRec::new(ctx);
        let mut name = emStringRec::new(ctx, String::new());
        let mut age = emIntRec::new(ctx, 0, i64::MIN, i64::MAX);
        let mut male = emBoolRec::new(ctx, false);
        inner.AddMember(&mut name, "name");
        inner.AddMember(&mut age, "age");
        inner.AddMember(&mut male, "male");
        Self {
            inner,
            name,
            age,
            male,
        }
    }
}

impl emRecNode for Person {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
    fn register_aggregate(&mut self, sig: SignalId) {
        self.inner.register_aggregate(sig);
        self.name.register_aggregate(sig);
        self.age.register_aggregate(sig);
        self.male.register_aggregate(sig);
    }
    fn listened_signal(&self) -> SignalId {
        self.inner.listened_signal()
    }
    fn TryRead(
        &mut self,
        reader: &mut dyn emcore::emRecReader::emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        let members = self.inner.member_identifiers();
        emStructRec::try_read_body(&members, reader, |idx, r| match idx {
            0 => self.name.TryRead(r, ctx),
            1 => self.age.TryRead(r, ctx),
            2 => self.male.TryRead(r, ctx),
            _ => unreachable!(),
        })
    }
    fn TryWrite(&self, writer: &mut dyn emRecWriter) -> Result<(), RecIoError> {
        let members = self.inner.member_identifiers();
        emStructRec::try_write_body(
            &members,
            writer,
            |_| true,
            |idx, w| match idx {
                0 => self.name.TryWrite(w),
                1 => self.age.TryWrite(w),
                2 => self.male.TryWrite(w),
                _ => unreachable!(),
            },
        )
    }
}

#[test]
fn union_rec_roundtrip() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut u = emUnionRec::new(&mut sc);
    u.AddVariant(
        "num",
        Box::new(|c: &mut SchedCtx<'_>| {
            Box::new(emIntRec::new(c, 0, i64::MIN, i64::MAX)) as Box<dyn emRecNode>
        }),
    );
    u.AddVariant(
        "text",
        Box::new(|c: &mut SchedCtx<'_>| {
            Box::new(emStringRec::new(c, String::new())) as Box<dyn emRecNode>
        }),
    );
    u.SetDefaultVariant(0);
    u.SetToDefaultVariant(&mut sc);
    u.SetVariant(1, &mut sc);
    // Mutate the child through the trait object.
    {
        let child = u.GetMut().expect("variant materialised");
        // Child is an emStringRec — route through trait TryRead to set value
        // atomically via byte-level input.
        let mut r = emRecMemReader::new(b"\"hello\"");
        child.TryRead(&mut r, &mut sc).unwrap();
    }
    let _ = sc;

    let mut w = emRecMemWriter::new();
    u.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"text: \"hello\"");

    // Read into a fresh union.
    let mut sc = fx.sc();
    let mut u2 = emUnionRec::new(&mut sc);
    u2.AddVariant(
        "num",
        Box::new(|c: &mut SchedCtx<'_>| {
            Box::new(emIntRec::new(c, 0, i64::MIN, i64::MAX)) as Box<dyn emRecNode>
        }),
    );
    u2.AddVariant(
        "text",
        Box::new(|c: &mut SchedCtx<'_>| {
            Box::new(emStringRec::new(c, String::new())) as Box<dyn emRecNode>
        }),
    );
    u2.SetDefaultVariant(0);
    u2.SetToDefaultVariant(&mut sc);

    let mut r = emRecMemReader::new(&bytes);
    u2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(u2.GetVariant(), 1);

    let mut w2 = emRecMemWriter::new();
    u2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    // Clear the pending-signal queue so the scheduler drops cleanly.
    // Internal signals from variant-switch have no test handle; they are
    // orphaned in the SlotMap but drop silently (no assertion).
    fx.sched.abort_all_pending();
}

#[test]
fn array_rec_roundtrip() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let alloc: emcore::emRec::emRecAllocator = Box::new(|c: &mut SchedCtx<'_>| {
        Box::new(emIntRec::new(c, 0, i64::MIN, i64::MAX)) as Box<dyn emRecNode>
    });
    let mut arr = emArrayRec::new(&mut sc, alloc, 0, 100);
    arr.SetCount(3, &mut sc);
    // Seed values 10, 20, 30 by byte-level TryRead on each element.
    for (i, v) in [10i32, 20, 30].iter().enumerate() {
        let child = arr.GetMut(i as i32).unwrap();
        let raw = format!("{}", v);
        let mut r = emRecMemReader::new(raw.as_bytes());
        child.TryRead(&mut r, &mut sc).unwrap();
    }
    let _ = sc;

    let mut w = emRecMemWriter::new();
    arr.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(bytes.as_slice(), b"{\n\t10\n\t20\n\t30\n}");

    let mut sc = fx.sc();
    let alloc2: emcore::emRec::emRecAllocator = Box::new(|c: &mut SchedCtx<'_>| {
        Box::new(emIntRec::new(c, 0, i64::MIN, i64::MAX)) as Box<dyn emRecNode>
    });
    let mut arr2 = emArrayRec::new(&mut sc, alloc2, 0, 100);
    let mut r = emRecMemReader::new(&bytes);
    arr2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(arr2.GetCount(), 3);

    let mut w2 = emRecMemWriter::new();
    arr2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    fx.sched.abort_all_pending();
}

#[test]
fn tarray_rec_roundtrip_persons() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let alloc: emTRecAllocator<Person> = Box::new(|c: &mut SchedCtx<'_>| Person::new(c));
    let mut arr = emTArrayRec::<Person>::new(&mut sc, alloc, 0, 100);
    arr.SetCount(2, &mut sc);

    arr.GetMut(0)
        .unwrap()
        .name
        .SetValue("alice".to_string(), &mut sc);
    arr.GetMut(0).unwrap().age.SetValue(30, &mut sc);
    arr.GetMut(0).unwrap().male.SetValue(false, &mut sc);
    arr.GetMut(1)
        .unwrap()
        .name
        .SetValue("bob".to_string(), &mut sc);
    arr.GetMut(1).unwrap().age.SetValue(40, &mut sc);
    arr.GetMut(1).unwrap().male.SetValue(true, &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    arr.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();

    // Read into fresh typed array.
    let mut sc = fx.sc();
    let alloc2: emTRecAllocator<Person> = Box::new(|c: &mut SchedCtx<'_>| Person::new(c));
    let mut arr2 = emTArrayRec::<Person>::new(&mut sc, alloc2, 0, 100);
    let mut r = emRecMemReader::new(&bytes);
    arr2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;

    assert_eq!(arr2.GetCount(), 2);
    assert_eq!(arr2.Get(0).unwrap().name.GetValue(), &"alice".to_string());
    assert_eq!(*arr2.Get(0).unwrap().age.GetValue(), 30);
    assert!(!*arr2.Get(0).unwrap().male.GetValue());
    assert_eq!(arr2.Get(1).unwrap().name.GetValue(), &"bob".to_string());
    assert_eq!(*arr2.Get(1).unwrap().age.GetValue(), 40);
    assert!(*arr2.Get(1).unwrap().male.GetValue());

    let mut w2 = emRecMemWriter::new();
    arr2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    fx.sched.abort_all_pending();
}

#[test]
fn struct_rec_roundtrip_person() {
    let mut fx = Fixture::new();

    let mut sc = fx.sc();
    let mut p = Person::new(&mut sc);
    p.name.SetValue("alice".to_string(), &mut sc);
    p.age.SetValue(42, &mut sc);
    p.male.SetValue(false, &mut sc);
    let _ = sc;

    let mut w = emRecMemWriter::new();
    p.TryWrite(&mut w).unwrap();
    let bytes = w.into_bytes();
    assert_eq!(
        std::str::from_utf8(&bytes).unwrap(),
        "{\n\tname = \"alice\"\n\tage = 42\n\tmale = no\n}"
    );

    let mut sc = fx.sc();
    let mut p2 = Person::new(&mut sc);
    let mut r = emRecMemReader::new(&bytes);
    p2.TryRead(&mut r, &mut sc).unwrap();
    let _ = sc;
    assert_eq!(p2.name.GetValue(), &"alice".to_string());
    assert_eq!(*p2.age.GetValue(), 42);
    assert!(!*p2.male.GetValue());

    let mut w2 = emRecMemWriter::new();
    p2.TryWrite(&mut w2).unwrap();
    assert_eq!(w2.into_bytes(), bytes);

    let mut sc = fx.sc();
    let a1 = p.inner.GetAggregateSignal();
    let a2 = p2.inner.GetAggregateSignal();
    sc.scheduler.abort(a1);
    sc.scheduler.abort(a2);
    sc.remove_signal(a1);
    sc.remove_signal(a2);
    for s in [
        p.name.GetValueSignal(),
        p.age.GetValueSignal(),
        p.male.GetValueSignal(),
        p2.name.GetValueSignal(),
        p2.age.GetValueSignal(),
        p2.male.GetValueSignal(),
    ] {
        sc.scheduler.abort(s);
        sc.remove_signal(s);
    }
}
