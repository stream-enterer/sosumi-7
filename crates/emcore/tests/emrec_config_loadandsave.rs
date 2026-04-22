//! Phase 4d Task 6 — integration tests for `emRecNodeConfigModel` against a
//! config-shaped `emStructRec` (Count / Enabled), exercising the full
//! install → modify → save → re-open cycle mirroring C++ `emConfigModel`
//! `TryLoadOrInstall` + `TrySave` + `TryLoad` (emConfigModel.cpp:77-114).
//!
//! The inline unit tests in `src/emRecNodeConfigModel.rs` cover the same
//! cases against an in-crate `MiniConfig`; this file proves the public API
//! is complete and usable from a downstream crate.

use emcore::emBoolRec::emBoolRec;
use emcore::emClipboard::emClipboard;
use emcore::emContext::emContext;
use emcore::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
use emcore::emIntRec::emIntRec;
use emcore::emRec::emRec;
use emcore::emRecNode::emRecNode;
use emcore::emRecNodeConfigModel::emRecNodeConfigModel;
use emcore::emRecReader::{emRecReader, RecIoError};
use emcore::emRecWriter::emRecWriter;
use emcore::emScheduler::EngineScheduler;
use emcore::emSignal::SignalId;
use emcore::emStructRec::emStructRec;
use std::cell::RefCell;
use std::rc::Rc;

fn make_sc<'a>(
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

struct AppConfig {
    inner: emStructRec,
    count: emIntRec,
    enabled: emBoolRec,
}

impl AppConfig {
    fn new(sc: &mut SchedCtx<'_>) -> Self {
        let mut inner = emStructRec::new(sc);
        let mut count = emIntRec::new(sc, 0, i64::MIN, i64::MAX);
        let mut enabled = emBoolRec::new(sc, false);
        inner.AddMember(&mut count, "Count");
        inner.AddMember(&mut enabled, "Enabled");
        Self {
            inner,
            count,
            enabled,
        }
    }

    fn signals(&self) -> [SignalId; 3] {
        [
            self.inner.GetAggregateSignal(),
            self.count.GetValueSignal(),
            self.enabled.GetValueSignal(),
        ]
    }
}

impl emRecNode for AppConfig {
    fn parent(&self) -> Option<&dyn emRecNode> {
        None
    }
    fn register_aggregate(&mut self, sig: SignalId) {
        self.inner.register_aggregate(sig);
        self.count.register_aggregate(sig);
        self.enabled.register_aggregate(sig);
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
            0 => self.count.TryRead(r, ctx),
            1 => self.enabled.TryRead(r, ctx),
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
                0 => self.count.TryWrite(w),
                1 => self.enabled.TryWrite(w),
                _ => unreachable!(),
            },
        )
    }
}

fn teardown(cfg: &AppConfig, sc: &mut SchedCtx<'_>) {
    for sig in cfg.signals() {
        sc.scheduler.abort(sig);
        sc.remove_signal(sig);
    }
}

/// Test 1 — first-run install: path does not exist; `TryLoadOrInstall`
/// creates parent dirs + writes the format header + body.
#[test]
fn install_on_first_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("app.cfg");

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let cfg = AppConfig::new(&mut sc);
    let mut model = emRecNodeConfigModel::new(cfg, path.clone()).with_format_name("AppConfig");

    assert!(!path.exists());
    model.TryLoadOrInstall(&mut sc).unwrap();
    assert!(path.exists());
    assert!(!model.IsUnsaved());

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(
        contents.starts_with("#%rec:AppConfig%#\n\n"),
        "{contents:?}"
    );

    let sigs = model.GetRec().signals();
    for sig in sigs {
        sc.scheduler.abort(sig);
        sc.remove_signal(sig);
    }
}

/// Test 2 — load existing file: write known bytes, construct fresh model,
/// `TryLoad`, assert values.
#[test]
fn load_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.cfg");
    std::fs::write(
        &path,
        b"#%rec:AppConfig%#\n\n{\n\tCount = -17\n\tEnabled = yes\n}\n",
    )
    .unwrap();

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let cfg = AppConfig::new(&mut sc);
    let mut model = emRecNodeConfigModel::new(cfg, path.clone()).with_format_name("AppConfig");

    model.TryLoad(&mut sc).unwrap();
    assert_eq!(*model.GetRec().count.GetValue(), -17);
    assert!(*model.GetRec().enabled.GetValue());
    assert!(!model.IsUnsaved());

    teardown(model.GetRec(), &mut sc);
}

/// Test 3 — modify flips dirty; TrySave persists and clears dirty.
#[test]
fn modify_marks_dirty_and_try_save_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.cfg");

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    let cfg = AppConfig::new(&mut sc);
    let mut model = emRecNodeConfigModel::new(cfg, path.clone()).with_format_name("AppConfig");
    model.TryLoadOrInstall(&mut sc).unwrap();

    model.modify(
        |cfg, ctx| {
            cfg.count.SetValue(99, ctx);
        },
        &mut sc,
    );
    assert!(model.IsUnsaved());
    model.TrySave(false).unwrap();
    assert!(!model.IsUnsaved());

    let reread = std::fs::read_to_string(&path).unwrap();
    assert!(reread.contains("Count = 99"), "{reread:?}");

    teardown(model.GetRec(), &mut sc);
}

/// Test 4 — end-to-end round-trip: default → modify → save → re-open →
/// modified values observed.
#[test]
fn end_to_end_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.cfg");

    let mut sched = EngineScheduler::new();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let ctx_root = emContext::NewRoot();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

    {
        let cfg = AppConfig::new(&mut sc);
        let mut model = emRecNodeConfigModel::new(cfg, path.clone()).with_format_name("AppConfig");
        model.TryLoadOrInstall(&mut sc).unwrap();
        model.modify(
            |cfg, ctx| {
                cfg.count.SetValue(12345, ctx);
                cfg.enabled.SetValue(true, ctx);
            },
            &mut sc,
        );
        model.TrySave(false).unwrap();
        teardown(model.GetRec(), &mut sc);
    }

    {
        let cfg = AppConfig::new(&mut sc);
        let mut model = emRecNodeConfigModel::new(cfg, path.clone()).with_format_name("AppConfig");
        model.TryLoadOrInstall(&mut sc).unwrap();
        assert_eq!(*model.GetRec().count.GetValue(), 12345);
        assert!(*model.GetRec().enabled.GetValue());
        teardown(model.GetRec(), &mut sc);
    }
}
