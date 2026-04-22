//! emRecNodeConfigModel — file-backed configuration record model for the
//! new `emRecNode` persistence stack.
//!
//! C++ reference: `include/emCore/emConfigModel.h` and
//! `src/emCore/emConfigModel.cpp` (Eagle Mode 0.96.4).
//!
//! SPLIT: there are two Rust ports sharing the C++ `emConfigModel` role in
//! this crate:
//!
//! * [`crate::emConfigModel::emConfigModel`] — legacy, generic over the
//!   internal `Record` trait with `from_rec`/`to_rec`/`SetToDefault`, wired
//!   through the tree-based [`crate::emRecParser`]. Multiple callers
//!   (`emWindowStateSaver`, `emCoreConfigPanel`, `emView`) still depend on it;
//!   those call sites will migrate incrementally.
//!
//! * `emRecNodeConfigModel<T: emRecNode>` (this file) — port-new successor
//!   wired through the Phase 4d [`crate::emRecFileReader`] /
//!   [`crate::emRecFileWriter`] IO stack. Provides the C++ TrySave /
//!   TryLoad / TryLoadOrInstall contract against an `emRecNode`-typed value.
//!
//! Once all legacy callers migrate to the new substrate, the legacy module
//! will be deleted and this type renamed to `emConfigModel` in line with the
//! CLAUDE.md File and Name Correspondence rule.
//!
//! DIVERGED: `emRecNodeConfigModel` does not derive from `emModel` — the Rust
//! port carries no engine/model runtime at this layer (the C++
//! `emModel`/`emContext` lifetime-plumbing is handled by higher-level
//! callers). What is preserved from C++ is the observable load/save/dirty
//! contract: `TrySave(force)` saves iff dirty-or-forced, `TryLoad` reads from
//! disk, `TryLoadOrInstall` installs defaults on first run.
//!
//! Dirty tracking uses [`emRecListener`]: a listener engine connects to the
//! record's `listened_signal()` at construction time and sets an
//! `Rc<Cell<bool>>` flag on the next scheduler cycle after any field mutates.
//! `modify()` additionally sets the flag synchronously for immediate
//! `IsUnsaved()` accuracy within the same call.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::emEngineCtx::SchedCtx;
use crate::emRecFileReader::emRecFileReader;
use crate::emRecFileWriter::emRecFileWriter;
use crate::emRecListener::emRecListener;
use crate::emRecNode::emRecNode;
use crate::emRecReader::RecIoError;

/// File-backed configuration model over an `emRecNode` value. See module
/// docs for the relationship to the legacy `emConfigModel<T: Record>`.
pub struct emRecNodeConfigModel<T: emRecNode> {
    value: T,
    install_path: PathBuf,
    unsaved_flag: Rc<Cell<bool>>,
    listener: emRecListener,
    format_name: Option<String>,
}

impl<T: emRecNode> emRecNodeConfigModel<T> {
    /// Construct a model wrapping `value`, with `install_path` as the disk
    /// location. `ctx` is required to register the internal listener engine
    /// that auto-marks the model dirty when any field in `value` signals a
    /// change. No IO happens here — call [`Self::TryLoad`] or
    /// [`Self::TryLoadOrInstall`] to populate.
    pub fn new(value: T, install_path: PathBuf, ctx: &mut SchedCtx<'_>) -> Self {
        let unsaved_flag = Rc::new(Cell::new(false));
        let flag_cb = Rc::clone(&unsaved_flag);
        let listener = emRecListener::new(
            Some(&value as &dyn emRecNode),
            Box::new(move |_sc| flag_cb.set(true)),
            ctx,
        );
        Self {
            value,
            install_path,
            unsaved_flag,
            listener,
            format_name: None,
        }
    }

    /// Builder: set the `#%rec:<name>%#` magic header for saved files. When
    /// set, [`Self::TryLoad`] also validates the header on load.
    ///
    /// Mirrors C++ `emRec::GetFormatName` (emRec.cpp:96-99) + the reader /
    /// writer header branches (emRec.cpp:2017-2019, 2512-2517).
    pub fn with_format_name(mut self, name: &str) -> Self {
        self.format_name = Some(name.to_string());
        self
    }

    /// Immutable access to the underlying record.
    ///
    /// C++ `emConfigModel::GetRec` (emConfigModel.h:111).
    pub fn GetRec(&self) -> &T {
        &self.value
    }

    /// Mutable access. Mutations fired through the returned reference
    /// auto-mark dirty after the next scheduler cycle via the listener.
    pub fn GetRecMut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Run `f` against the record and mark the model unsaved immediately.
    ///
    /// DIVERGED: `modify()` sets `unsaved_flag` synchronously rather than
    /// waiting for the listener engine to wake on the next scheduler cycle.
    /// This matches the observable C++ contract (IsUnsaved true immediately
    /// after mutation) even though the underlying mechanism differs.
    pub fn modify<F>(&mut self, f: F, ctx: &mut SchedCtx<'_>)
    where
        F: FnOnce(&mut T, &mut SchedCtx<'_>),
    {
        f(&mut self.value, ctx);
        self.unsaved_flag.set(true);
    }

    /// Install path of the configuration file. C++
    /// `emConfigModel::GetInstallPath` (emConfigModel.h:46,137-140).
    pub fn GetInstallPath(&self) -> &Path {
        &self.install_path
    }

    /// Whether the record has in-memory changes not yet flushed to disk.
    /// C++ `emConfigModel::IsUnsaved` (emConfigModel.h:49,142-145).
    pub fn IsUnsaved(&self) -> bool {
        self.unsaved_flag.get()
    }

    /// Load the record from [`Self::GetInstallPath`]. Mirrors C++
    /// `emConfigModel::TryLoad` (emConfigModel.cpp:77-84): delegates to
    /// `emRec::TryLoad` → opens an `emRecFileReader`, consumes the magic
    /// header when one is configured, drives `TryRead` on the root record,
    /// and clears `Unsaved` on success.
    pub fn TryLoad(&mut self, ctx: &mut SchedCtx<'_>) -> Result<(), RecIoError> {
        // SPLIT: C++ `emRec::TryLoad` opens `emRecFileReader`, calls
        // `TryStartReading(root)` which consumes the magic header when the
        // root has a FormatName, then drives `TryContinueReading` in a loop.
        // The Rust reader trait is per-element, so we compose those three
        // steps explicitly here.
        let mut reader: Box<dyn crate::emRecReader::emRecReader> =
            if let Some(ref fmt) = self.format_name {
                Box::new(emRecFileReader::open_with_format(&self.install_path, fmt)?)
            } else {
                Box::new(emRecFileReader::new(&self.install_path)?)
            };
        self.value.TryRead(reader.as_mut(), ctx)?;
        self.unsaved_flag.set(false);
        Ok(())
    }

    /// Save the record to [`Self::GetInstallPath`] iff unsaved-or-`force`.
    /// Mirrors C++ `emConfigModel::TrySave` (emConfigModel.cpp:24-33), which
    /// guards on `(Unsaved || force)` then delegates to `emRec::TrySave`.
    /// Creates missing parent directories to match the
    /// `TryLoadOrInstall`-era expectation that the first save lands even in
    /// a fresh config directory (emConfigModel.cpp:104).
    pub fn TrySave(&mut self, force: bool) -> Result<(), RecIoError> {
        if !self.unsaved_flag.get() && !force {
            return Ok(());
        }
        if let Some(parent) = self.install_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    RecIoError::with_location(
                        Some(self.install_path.display().to_string()),
                        None,
                        format!("failed to create parent directory: {}", e),
                    )
                })?;
            }
        }
        let mut writer = emRecFileWriter::new(self.install_path.clone());
        if let Some(ref fmt) = self.format_name {
            // Mirror C++ emRec.cpp:2512-2517 — `#%rec:FormatName%#\n\n` is
            // emitted before the root body. The trait exposes no raw-string
            // primitive, so we pipe each ASCII byte through
            // `TryWriteDelimiter` (which is a raw `push`; see
            // emRecMemWriter.rs `TryWriteChar`).
            use crate::emRecWriter::emRecWriter;
            for ch in format!("#%rec:{}%#\n\n", fmt).chars() {
                writer.TryWriteDelimiter(ch)?;
            }
        }
        self.value.TryWrite(&mut writer)?;
        // Trailing newline per emRec.cpp:2535 (`TryWriteNewLine` after the
        // root body, before close).
        {
            use crate::emRecWriter::emRecWriter;
            writer.TryWriteNewLine()?;
        }
        writer.finalize()?;
        self.unsaved_flag.set(false);
        Ok(())
    }

    /// Load the record if the file exists, else initialise the file with the
    /// current in-memory value. Mirrors C++ `emConfigModel::TryLoadOrInstall`
    /// without the optional `insSrcPath` copy-from-template branch
    /// (emConfigModel.cpp:98-114).
    ///
    /// DIVERGED: C++ invokes `GetRec().SetToDefault()` on the install branch
    /// so the saved file reflects canonical defaults regardless of prior
    /// mutation. The Rust port expects callers to construct the model with a
    /// default-valued record — a `SetToDefault` method on `emRecNode` is not
    /// yet ported (TODO phase-4d-followup: add `SetToDefault` to the trait so
    /// this method can mirror C++ exactly).
    pub fn TryLoadOrInstall(&mut self, ctx: &mut SchedCtx<'_>) -> Result<(), RecIoError> {
        if self.install_path.exists() {
            self.TryLoad(ctx)
        } else {
            self.unsaved_flag.set(true);
            self.TrySave(true)
        }
    }

    /// DIVERGED: no C++ counterpart. C++ `emConfigModel` destroys the
    /// `RecLink` (its embedded listener) in `~emConfigModel`; in Rust the
    /// lifetime of `emRecListener` is managed explicitly. Call before drop
    /// to remove the listener engine from the scheduler.
    ///
    /// Non-consuming: record fields remain accessible after `detach` for
    /// signal teardown (abort + remove_signal on each record field's SignalId).
    pub fn detach(&mut self, ctx: &mut SchedCtx<'_>) {
        self.listener.detach_mut(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emBoolRec::emBoolRec;
    use crate::emClipboard::emClipboard;
    use crate::emContext::emContext;
    use crate::emEngineCtx::{DeferredAction, FrameworkDeferredAction};
    use crate::emIntRec::emIntRec;
    use crate::emRec::emRec;
    use crate::emRecNode::emRecNode;
    use crate::emRecReader::emRecReader;
    use crate::emRecWriter::emRecWriter;
    use crate::emScheduler::EngineScheduler;
    use crate::emSignal::SignalId;
    use crate::emStructRec::emStructRec;
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

    /// Minimal 2-field emRecNode config: an int + a bool.
    struct MiniConfig {
        inner: emStructRec,
        count: emIntRec,
        enabled: emBoolRec,
    }

    impl MiniConfig {
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

    impl emRecNode for MiniConfig {
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

    fn teardown(cfg: &MiniConfig, sc: &mut SchedCtx<'_>) {
        for sig in cfg.signals() {
            sc.scheduler.abort(sig);
            sc.remove_signal(sig);
        }
    }

    fn run_slice(sched: &mut EngineScheduler) {
        use std::collections::HashMap;
        let mut windows = HashMap::new();
        let root = emContext::NewRoot();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fc: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        sched.DoTimeSlice(
            &mut windows,
            &root,
            &mut actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &pa,
        );
    }

    #[test]
    fn install_on_first_run_writes_header_and_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("mini.cfg");

        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let cfg = MiniConfig::new(&mut sc);
        let mut model =
            emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("MiniConfig");

        assert!(!path.exists());
        model.TryLoadOrInstall(&mut sc).unwrap();
        assert!(path.exists());
        assert!(!model.IsUnsaved());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(
            contents.starts_with("#%rec:MiniConfig%#\n\n"),
            "missing header: {contents:?}"
        );
        assert!(contents.contains("Count = 0"), "contents: {contents:?}");
        assert!(contents.contains("Enabled = no"), "contents: {contents:?}");

        model.detach(&mut sc);
        teardown(&model.value, &mut sc);
    }

    #[test]
    fn try_load_reads_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mini.cfg");

        // Hand-write a known-good file.
        std::fs::write(
            &path,
            b"#%rec:MiniConfig%#\n\n{\n\tCount = 42\n\tEnabled = yes\n}\n",
        )
        .unwrap();

        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let cfg = MiniConfig::new(&mut sc);
        let mut model =
            emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("MiniConfig");

        model.TryLoad(&mut sc).unwrap();
        assert_eq!(*model.GetRec().count.GetValue(), 42);
        assert!(*model.GetRec().enabled.GetValue());
        assert!(!model.IsUnsaved());

        model.detach(&mut sc);
        teardown(&model.value, &mut sc);
    }

    #[test]
    fn modify_marks_unsaved_and_try_save_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mini.cfg");

        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let cfg = MiniConfig::new(&mut sc);
        let mut model =
            emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("MiniConfig");
        // Install defaults.
        model.TryLoadOrInstall(&mut sc).unwrap();
        assert!(!model.IsUnsaved());

        // Mutate via modify.
        model.modify(
            |cfg, ctx| {
                cfg.count.SetValue(7, ctx);
                cfg.enabled.SetValue(true, ctx);
            },
            &mut sc,
        );
        assert!(model.IsUnsaved());

        // Non-forced TrySave when unsaved: writes.
        model.TrySave(false).unwrap();
        assert!(!model.IsUnsaved());

        // Non-forced TrySave when clean: no-op. (Touch the file first so we
        // can detect a rewrite.)
        let before = std::fs::metadata(&path).unwrap().len();
        model.TrySave(false).unwrap();
        let after = std::fs::metadata(&path).unwrap().len();
        assert_eq!(before, after);

        // Re-read from disk via a fresh model — assert values persisted.
        let cfg2 = MiniConfig::new(&mut sc);
        let mut model2 =
            emRecNodeConfigModel::new(cfg2, path.clone(), &mut sc).with_format_name("MiniConfig");
        model2.TryLoad(&mut sc).unwrap();
        assert_eq!(*model2.GetRec().count.GetValue(), 7);
        assert!(*model2.GetRec().enabled.GetValue());

        model.detach(&mut sc);
        teardown(&model.value, &mut sc);
        model2.detach(&mut sc);
        teardown(&model2.value, &mut sc);
    }

    #[test]
    fn end_to_end_default_modify_save_reopen_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mini.cfg");

        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        {
            let cfg = MiniConfig::new(&mut sc);
            let mut model = emRecNodeConfigModel::new(cfg, path.clone(), &mut sc)
                .with_format_name("MiniConfig");
            model.TryLoadOrInstall(&mut sc).unwrap();
            model.modify(
                |cfg, ctx| {
                    cfg.count.SetValue(-123, ctx);
                    cfg.enabled.SetValue(true, ctx);
                },
                &mut sc,
            );
            model.TrySave(false).unwrap();
            model.detach(&mut sc);
            teardown(&model.value, &mut sc);
        }

        {
            let cfg = MiniConfig::new(&mut sc);
            let mut model = emRecNodeConfigModel::new(cfg, path.clone(), &mut sc)
                .with_format_name("MiniConfig");
            model.TryLoadOrInstall(&mut sc).unwrap();
            assert_eq!(*model.GetRec().count.GetValue(), -123);
            assert!(*model.GetRec().enabled.GetValue());
            assert!(!model.IsUnsaved());
            model.detach(&mut sc);
            teardown(&model.value, &mut sc);
        }
    }

    /// Helper used by the integration tests in
    /// `tests/emrec_config_loadandsave.rs` — keeping the public builder API
    /// exercised in this module's own tests prevents the `TryLoadOrInstall`
    /// path rotting if the integration file is ever dropped.
    #[test]
    fn builder_set_format_name_survives() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mini.cfg");
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        let cfg = MiniConfig::new(&mut sc);
        let mut model =
            emRecNodeConfigModel::new(cfg, path.clone(), &mut sc).with_format_name("Foo");
        assert_eq!(model.GetInstallPath(), path.as_path());
        assert!(!model.IsUnsaved());
        model.detach(&mut sc);
        teardown(&model.value, &mut sc);
    }

    #[test]
    fn listener_auto_marks_dirty_after_scheduler_cycle() {
        let mut sched = EngineScheduler::new();
        let mut actions: Vec<DeferredAction> = Vec::new();
        let ctx_root = emContext::NewRoot();
        let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);

        let cfg = MiniConfig::new(&mut sc);
        let mut model = emRecNodeConfigModel::new(
            cfg,
            std::path::PathBuf::from("/tmp/unused_listener_test.cfg"),
            &mut sc,
        );
        model.GetRecMut().count.SetValue(42, &mut sc);
        assert!(
            !model.IsUnsaved(),
            "dirty not yet set before scheduler cycle"
        );
        let _ = sc;
        run_slice(&mut sched);
        assert!(model.IsUnsaved(), "dirty must be set after scheduler cycle");
        let mut sc = make_sc(&mut sched, &mut actions, &ctx_root, &cb, &pa);
        model.detach(&mut sc);
        teardown(&model.value, &mut sc);
    }
}
