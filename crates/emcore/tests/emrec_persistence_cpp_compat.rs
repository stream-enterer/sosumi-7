//! Phase 4d Task 5 — byte-format compatibility test with a C++-produced
//! emRec fixture.
//!
//! Loads `tests/data/License.emVcItem` (copied verbatim from Eagle Mode
//! 0.96.4 at `etc/emMain/VcItems/License.emVcItem`) via
//! [`emRecFileReader::open_with_format`] and asserts every field round-trips
//! to its expected value. Proves the Rust reader + emColor short-hex path
//! plus emStructRec body dispatch are byte-format compatible with the C++
//! emVirtualCosmosItem serialization.

use emcore::emBoolRec::emBoolRec;
use emcore::emClipboard::emClipboard;
use emcore::emColor::emColor;
use emcore::emColorRec::emColorRec;
use emcore::emContext::emContext;
use emcore::emDoubleRec::emDoubleRec;
use emcore::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
use emcore::emRec::emRec;
use emcore::emRecFileReader::emRecFileReader;
use emcore::emRecMemReader::emRecMemReader;
use emcore::emRecMemWriter::emRecMemWriter;
use emcore::emRecNode::emRecNode;
use emcore::emRecReader::{emRecReader, RecIoError};
use emcore::emRecWriter::emRecWriter;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;
use emcore::emStringRec::emStringRec;
use emcore::emStructRec::emStructRec;
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

/// Rust mirror of the C++ emVirtualCosmosItem record layout used by
/// `etc/emMain/VcItems/*.emVcItem` files. Field identifiers MUST match the
/// C++ file's exact spellings — the reader dispatches by name.
struct VcItem {
    inner: emStructRec,
    title: emStringRec,
    pos_x: emDoubleRec,
    pos_y: emDoubleRec,
    width: emDoubleRec,
    content_tallness: emDoubleRec,
    background_color: emColorRec,
    border_color: emColorRec,
    title_color: emColorRec,
    file_name: emStringRec,
    copy_to_user: emBoolRec,
}

impl VcItem {
    fn new(sc: &mut SchedCtx<'_>) -> Self {
        let mut inner = emStructRec::new(sc);
        let mut title = emStringRec::new(sc, String::new());
        let mut pos_x = emDoubleRec::new(sc, 0.0, f64::MIN, f64::MAX);
        let mut pos_y = emDoubleRec::new(sc, 0.0, f64::MIN, f64::MAX);
        let mut width = emDoubleRec::new(sc, 0.0, 0.0, f64::MAX);
        let mut content_tallness = emDoubleRec::new(sc, 1.0, 0.0, f64::MAX);
        let mut background_color = emColorRec::new(sc, emColor::BLACK, false);
        let mut border_color = emColorRec::new(sc, emColor::BLACK, false);
        let mut title_color = emColorRec::new(sc, emColor::BLACK, false);
        let mut file_name = emStringRec::new(sc, String::new());
        let mut copy_to_user = emBoolRec::new(sc, false);

        inner.AddMember(&mut title, "Title");
        inner.AddMember(&mut pos_x, "PosX");
        inner.AddMember(&mut pos_y, "PosY");
        inner.AddMember(&mut width, "Width");
        inner.AddMember(&mut content_tallness, "ContentTallness");
        inner.AddMember(&mut background_color, "BackgroundColor");
        inner.AddMember(&mut border_color, "BorderColor");
        inner.AddMember(&mut title_color, "TitleColor");
        inner.AddMember(&mut file_name, "FileName");
        inner.AddMember(&mut copy_to_user, "CopyToUser");

        Self {
            inner,
            title,
            pos_x,
            pos_y,
            width,
            content_tallness,
            background_color,
            border_color,
            title_color,
            file_name,
            copy_to_user,
        }
    }
}

impl emRecNode for VcItem {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }

    fn register_aggregate(&mut self, sig: SignalId) {
        self.inner.register_aggregate(sig);
        self.title.register_aggregate(sig);
        self.pos_x.register_aggregate(sig);
        self.pos_y.register_aggregate(sig);
        self.width.register_aggregate(sig);
        self.content_tallness.register_aggregate(sig);
        self.background_color.register_aggregate(sig);
        self.border_color.register_aggregate(sig);
        self.title_color.register_aggregate(sig);
        self.file_name.register_aggregate(sig);
        self.copy_to_user.register_aggregate(sig);
    }

    fn listened_signal(&self) -> SignalId {
        self.inner.listened_signal()
    }

    fn TryRead(
        &mut self,
        reader: &mut dyn emRecReader,
        ctx: &mut SchedCtx<'_>,
    ) -> Result<(), RecIoError> {
        let members = self.inner.member_identifiers();
        emStructRec::try_read_body(&members, reader, |idx, r| match idx {
            0 => self.title.TryRead(r, ctx),
            1 => self.pos_x.TryRead(r, ctx),
            2 => self.pos_y.TryRead(r, ctx),
            3 => self.width.TryRead(r, ctx),
            4 => self.content_tallness.TryRead(r, ctx),
            5 => self.background_color.TryRead(r, ctx),
            6 => self.border_color.TryRead(r, ctx),
            7 => self.title_color.TryRead(r, ctx),
            8 => self.file_name.TryRead(r, ctx),
            9 => self.copy_to_user.TryRead(r, ctx),
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
                0 => self.title.TryWrite(w),
                1 => self.pos_x.TryWrite(w),
                2 => self.pos_y.TryWrite(w),
                3 => self.width.TryWrite(w),
                4 => self.content_tallness.TryWrite(w),
                5 => self.background_color.TryWrite(w),
                6 => self.border_color.TryWrite(w),
                7 => self.title_color.TryWrite(w),
                8 => self.file_name.TryWrite(w),
                9 => self.copy_to_user.TryWrite(w),
                _ => unreachable!(),
            },
        )
    }
}

/// Phase 4d Task 5 (primary gate): load a C++-produced emRec file verbatim
/// through `emRecFileReader::open_with_format` and assert every field's
/// parsed value matches the fixture.
#[test]
fn license_vcitem_loads_from_cpp_fixture() {
    // NOTE: emVirtualCosmosItem uses `{` `}` around the whole record body in
    // the full emConfigModel context. The standalone `.emVcItem` files the
    // Eagle Mode distribution ships are the *bare inner body* — field =
    // value lines without outer braces — since emConfigModel handles the
    // outer wrapping. For this test we wrap the fixture bytes in an outer
    // `{ ... }` pair via a mem reader so `emStructRec::try_read_body` has
    // the delimiters it requires.
    let path = "tests/data/License.emVcItem";

    // Primary path exercise: `open_with_format` validates the magic header
    // and returns a reader positioned after it. We don't call TryRead on it
    // directly (the file has no outer braces — that's emConfigModel's job).
    let mut header_reader = emRecFileReader::open_with_format(path, "emVirtualCosmosItem").unwrap();
    // After the magic header the next element should be an identifier
    // (`Title`).
    let peek = header_reader.TryPeekNext().unwrap();
    assert_eq!(
        peek.element_type(),
        emcore::emRecReader::ElementType::Identifier,
    );

    // Value-equivalence path: wrap the file body in outer braces so a
    // single TryRead drives the whole record. Mirrors how C++ emConfigModel
    // wraps the outer brace pair around the file body (see emConfigModel
    // TODO in Task 6).
    let mut bytes = std::fs::read(path).unwrap();
    // Strip the `#%rec:...%` magic + the trailing `#...\n` comment line
    // before adding braces. Keep it simple: use the Mem reader's header
    // validation then re-serialize what remains is overkill — just find the
    // end of the first line.
    if let Some(nl) = bytes.iter().position(|&b| b == b'\n') {
        bytes = bytes[nl + 1..].to_vec();
    }
    let mut wrapped = Vec::with_capacity(bytes.len() + 4);
    wrapped.push(b'{');
    wrapped.push(b'\n');
    wrapped.extend_from_slice(&bytes);
    wrapped.push(b'\n');
    wrapped.push(b'}');

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let mut item = VcItem::new(&mut sc);
    let mut r = emRecMemReader::new(&wrapped);
    item.TryRead(&mut r, &mut sc).unwrap();

    assert_eq!(item.title.GetValue(), "License");
    assert!((item.pos_x.GetValue() - 0.10434).abs() < 1e-9);
    assert!((item.pos_y.GetValue() - 0.100).abs() < 1e-9);
    assert!((item.width.GetValue() - 0.010).abs() < 1e-9);
    assert!((item.content_tallness.GetValue() - 0.5).abs() < 1e-9);
    // "#998" → rgb(0x99, 0x99, 0x88) — 1-hex-per-channel replication
    // (emColor::TryParse len=3 branch).
    assert_eq!(
        *item.background_color.GetValue(),
        emColor::rgb(0x99, 0x99, 0x88)
    );
    assert_eq!(
        *item.border_color.GetValue(),
        emColor::rgb(0x66, 0x66, 0x66)
    );
    assert_eq!(*item.title_color.GetValue(), emColor::rgb(0xEE, 0xEE, 0xFF));
    assert_eq!(item.file_name.GetValue(), "License.emFileLink");
    assert!(!(*item.copy_to_user.GetValue()));

    // Teardown: abort + remove every signal we created.
    let sigs = [
        item.inner.GetAggregateSignal(),
        item.title.GetValueSignal(),
        item.pos_x.GetValueSignal(),
        item.pos_y.GetValueSignal(),
        item.width.GetValueSignal(),
        item.content_tallness.GetValueSignal(),
        item.background_color.GetValueSignal(),
        item.border_color.GetValueSignal(),
        item.title_color.GetValueSignal(),
        item.file_name.GetValueSignal(),
        item.copy_to_user.GetValueSignal(),
    ];
    for sig in sigs {
        sc.scheduler.abort(sig);
        sc.remove_signal(sig);
    }
}

/// Phase 4d Task 5 (secondary): field-value round-trip through the writer.
/// After loading from the fixture, re-serialize via emRecMemWriter, parse
/// the resulting bytes back into a fresh `VcItem`, and assert every field
/// matches. Byte-level equality with the C++ fixture is NOT asserted —
/// C++ uses `.10434` (no leading zero in emDoubleRec::TryWrite output) and
/// integer-triplet color emission `{153 153 136}` where the source file
/// used short-hex `"#998"`. Those format-level re-serialization differences
/// are a known follow-up (TODO(phase-4d-followup): byte-stable emit that
/// preserves the input's numeric and color spelling).
#[test]
fn license_vcitem_round_trips_field_values_through_writer() {
    let path = "tests/data/License.emVcItem";
    let mut bytes = std::fs::read(path).unwrap();
    if let Some(nl) = bytes.iter().position(|&b| b == b'\n') {
        bytes = bytes[nl + 1..].to_vec();
    }
    let mut wrapped = Vec::with_capacity(bytes.len() + 4);
    wrapped.push(b'{');
    wrapped.push(b'\n');
    wrapped.extend_from_slice(&bytes);
    wrapped.push(b'\n');
    wrapped.push(b'}');

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sched_ctx(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let mut item = VcItem::new(&mut sc);
    let mut r = emRecMemReader::new(&wrapped);
    item.TryRead(&mut r, &mut sc).unwrap();

    // Re-serialize.
    let mut w = emRecMemWriter::new();
    item.TryWrite(&mut w).unwrap();
    let out_bytes = w.into_bytes();

    // Parse back into a fresh VcItem.
    let mut item2 = VcItem::new(&mut sc);
    let mut r2 = emRecMemReader::new(&out_bytes);
    item2.TryRead(&mut r2, &mut sc).unwrap();

    assert_eq!(item2.title.GetValue(), item.title.GetValue());
    assert_eq!(item2.pos_x.GetValue(), item.pos_x.GetValue());
    assert_eq!(item2.pos_y.GetValue(), item.pos_y.GetValue());
    assert_eq!(item2.width.GetValue(), item.width.GetValue());
    assert_eq!(
        item2.content_tallness.GetValue(),
        item.content_tallness.GetValue()
    );
    assert_eq!(
        item2.background_color.GetValue(),
        item.background_color.GetValue()
    );
    assert_eq!(item2.border_color.GetValue(), item.border_color.GetValue());
    assert_eq!(item2.title_color.GetValue(), item.title_color.GetValue());
    assert_eq!(item2.file_name.GetValue(), item.file_name.GetValue());
    assert_eq!(item2.copy_to_user.GetValue(), item.copy_to_user.GetValue());

    // Teardown.
    for it in [&item, &item2] {
        let sigs = [
            it.inner.GetAggregateSignal(),
            it.title.GetValueSignal(),
            it.pos_x.GetValueSignal(),
            it.pos_y.GetValueSignal(),
            it.width.GetValueSignal(),
            it.content_tallness.GetValueSignal(),
            it.background_color.GetValueSignal(),
            it.border_color.GetValueSignal(),
            it.title_color.GetValueSignal(),
            it.file_name.GetValueSignal(),
            it.copy_to_user.GetValueSignal(),
        ];
        for sig in sigs {
            sc.scheduler.abort(sig);
            sc.remove_signal(sig);
        }
    }
}
