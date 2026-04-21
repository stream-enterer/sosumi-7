use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::emDialog::{emDialog, DialogResult};
use crate::emEngineCtx::{ConstructCtx, PanelCtx};
use crate::emFileSelectionBox::emFileSelectionBox;
use crate::emLook::emLook;
use crate::emSignal::SignalId;

/// Mode of the file dialog.
///
/// Port of C++ `emFileDialog::ModeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogMode {
    /// Select a file without validation dialogs.
    Select,
    /// Open (read) a file. Validates that the file exists.
    Open,
    /// Save (write) a file. Confirms overwrite of existing files.
    Save,
}

/// Result of a file dialog check-finish validation.
#[derive(Clone, Debug)]
pub enum FileDialogCheckResult {
    /// The dialog can finish.
    Allow,
    /// A directory was selected; enter it instead of finishing.
    EnterDirectory(String),
    /// An error occurred (show message to user).
    Error(String),
    /// An overwrite confirmation is needed.
    ConfirmOverwrite(Vec<PathBuf>),
}

/// A dialog for file open/save operations.
///
/// Port of C++ `emFileDialog`. Wraps a `emFileSelectionBox` in a `emDialog` with
/// OK/Cancel buttons and mode-dependent validation (existence checks for Open,
/// overwrite confirmation for Save).
pub struct emFileDialog {
    dialog: emDialog,
    look: Rc<emLook>,
    fsb: emFileSelectionBox,
    mode: FileDialogMode,
    dir_allowed: bool,
    /// Written by the `on_finish` closure when the overwrite dialog finishes;
    /// read in `Cycle` (Task 20). Shared via `Rc<Cell<_>>` so the closure can
    /// hold a clone. Added in Task 19; Task 20 reads it.
    pub(crate) overwrite_result: Rc<Cell<Option<DialogResult>>>,
    /// The pending overwrite-confirmation dialog (C++ `OverwriteDialog`,
    /// `emCrossPtr<emDialog>`). Owned as `Option<emDialog>` here; identical
    /// lifetime semantics — created inside `CheckFinish` when Save-mode
    /// detects existing files, observed inside `Cycle` via its
    /// `finish_signal`, dropped when Cycle handles the user's answer.
    overwrite_dialog: Option<emDialog>,
    overwrite_asked: String,
    overwrite_confirmed: String,
    /// Cached copy of `fsb.file_trigger_signal` for signal-driven Cycle.
    /// C++ `emFileDialog` constructor calls
    /// `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`; Rust stores the
    /// id so `Cycle` can query it without re-borrowing `fsb`.
    fsb_file_trigger_signal: SignalId,
}

impl emFileDialog {
    pub fn new<C: crate::emEngineCtx::ConstructCtx>(
        ctx: &mut C,
        mode: FileDialogMode,
        look: Rc<emLook>,
    ) -> Self {
        let (title, ok_label) = mode_title_and_ok(mode);
        let mut dialog = emDialog::new(ctx, title, Rc::clone(&look));
        dialog.AddCustomButton(ctx, ok_label, DialogResult::Ok);
        dialog.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
        dialog.show(ctx);

        let mut fsb = emFileSelectionBox::new(ctx, "");
        fsb.border_mut().outer = super::emBorder::OuterBorderType::None;
        fsb.border_mut().inner = super::emBorder::InnerBorderType::None;
        let fsb_file_trigger_signal = fsb.file_trigger_signal;

        Self {
            dialog,
            look,
            fsb,
            mode,
            dir_allowed: false,
            overwrite_result: Rc::new(Cell::new(None)),
            overwrite_dialog: None,
            overwrite_asked: String::new(),
            overwrite_confirmed: String::new(),
            fsb_file_trigger_signal,
        }
    }

    pub fn GetMode(&self) -> FileDialogMode {
        self.mode
    }

    pub fn is_directory_result_allowed(&self) -> bool {
        self.dir_allowed
    }

    pub fn set_directory_result_allowed(&mut self, allowed: bool) {
        self.dir_allowed = allowed;
    }

    pub fn is_multi_selection_enabled(&self) -> bool {
        self.fsb.is_multi_selection_enabled()
    }

    pub fn set_multi_selection_enabled(&mut self, enabled: bool) {
        self.fsb.set_multi_selection_enabled(enabled);
    }

    pub fn GetParentDirectory(&self) -> &Path {
        self.fsb.GetParentDirectory()
    }

    pub fn set_parent_directory(&mut self, dir: &Path) {
        self.fsb.set_parent_directory(dir);
    }

    pub fn GetSelectedName(&self) -> Option<&str> {
        self.fsb.GetSelectedName()
    }

    pub fn GetSelectedNames(&self) -> &[String] {
        self.fsb.GetSelectedNames()
    }

    pub fn set_selected_name(&mut self, name: &str) {
        self.fsb.set_selected_name(name);
    }

    pub fn set_selected_names(&mut self, names: &[String]) {
        self.fsb.set_selected_names(names);
    }

    pub fn ClearSelection(&mut self) {
        self.fsb.ClearSelection();
    }

    pub fn GetSelectedPath(&self) -> PathBuf {
        self.fsb.GetSelectedPath()
    }

    pub fn set_selected_path(&mut self, path: &Path) {
        self.fsb.set_selected_path(path);
    }

    pub fn GetFilters(&self) -> &[String] {
        self.fsb.GetFilters()
    }

    pub fn set_filters(&mut self, filters: &[String]) {
        self.fsb.set_filters(filters);
    }

    pub fn GetSelectedFilterIndex(&self) -> i32 {
        self.fsb.GetSelectedFilterIndex()
    }

    pub fn set_selected_filter_index(&mut self, index: i32) {
        self.fsb.set_selected_filter_index(index);
    }

    pub fn are_hidden_files_shown(&self) -> bool {
        self.fsb.are_hidden_files_shown()
    }

    pub fn set_hidden_files_shown(&mut self, shown: bool) {
        self.fsb.set_hidden_files_shown(shown);
    }

    pub fn dialog(&self) -> &emDialog {
        &self.dialog
    }

    pub fn file_selection_box(&self) -> &emFileSelectionBox {
        &self.fsb
    }

    pub fn file_selection_box_mut(&mut self) -> &mut emFileSelectionBox {
        &mut self.fsb
    }

    /// Check whether the dialog can finish with the given result.
    ///
    /// Port of C++ `emFileDialog::CheckFinish`. Validates the selection
    /// based on the dialog mode (Open checks existence, Save confirms
    /// overwrite).
    pub fn CheckFinish<C: crate::emEngineCtx::ConstructCtx>(
        &mut self,
        ctx: &mut C,
        result: &DialogResult,
    ) -> FileDialogCheckResult {
        if *result == DialogResult::Cancel {
            return FileDialogCheckResult::Allow;
        }

        let names = self.fsb.GetSelectedNames().to_vec();
        let parent = self.fsb.GetParentDirectory().to_path_buf();

        // Check for directory selection when not allowed.
        if !self.dir_allowed {
            if names.is_empty() {
                return FileDialogCheckResult::Error("No file selected".to_string());
            }
            for name in &names {
                let path = parent.join(name);
                if path.is_dir() {
                    if names.len() == 1 {
                        return FileDialogCheckResult::EnterDirectory(name.clone());
                    }
                    return FileDialogCheckResult::Error(format!("Directory selected: {}", name));
                }
            }
        }

        match self.mode {
            FileDialogMode::Open => {
                for name in &names {
                    let path = parent.join(name);
                    if !path.exists() {
                        return FileDialogCheckResult::Error(format!(
                            "The following file cannot be opened, because it does not exist:\n\n{}",
                            path.display()
                        ));
                    }
                }
            }
            FileDialogMode::Save => {
                let mut paths_to_overwrite = Vec::new();
                for name in &names {
                    let path = parent.join(name);
                    if path.exists() {
                        paths_to_overwrite.push(path);
                    }
                }
                if !paths_to_overwrite.is_empty() {
                    let text = if paths_to_overwrite.len() == 1 {
                        format!(
                            "Are you sure to overwrite the following already existing file?\n\n{}",
                            paths_to_overwrite[0].display()
                        )
                    } else {
                        let mut msg =
                            "Are you sure to overwrite the following already existing files?\n"
                                .to_string();
                        for p in &paths_to_overwrite {
                            msg.push('\n');
                            msg.push_str(&p.display().to_string());
                        }
                        msg
                    };
                    if text != self.overwrite_confirmed {
                        // Create the overwrite confirmation dialog, matching
                        // C++ CheckFinish lines 186-197 (new emDialog, set
                        // title, add OK/Cancel buttons). Task 19: wire
                        // set_on_finish + show per new handle API.
                        let mut dlg = emDialog::new(ctx, "File Exists", self.look.clone());
                        dlg.AddCustomButton(ctx, "OK", DialogResult::Ok);
                        dlg.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
                        let cell = Rc::clone(&self.overwrite_result);
                        dlg.set_on_finish(Box::new(move |r, _sched| cell.set(Some(*r))));
                        dlg.show(ctx);
                        self.overwrite_dialog = Some(dlg);
                        self.overwrite_asked = text;
                        return FileDialogCheckResult::ConfirmOverwrite(paths_to_overwrite);
                    }
                }
                self.overwrite_confirmed.clear();
            }
            FileDialogMode::Select => {}
        }

        FileDialogCheckResult::Allow
    }

    /// Observe the file-selection-box and overwrite-dialog signals and drive
    /// the dialog forward when either fires.
    ///
    /// Port of C++ `emFileDialog::Cycle` (emFileDialog.cpp:80-106). The C++
    /// scheduler invokes `Cycle()` whenever a subscribed wake-up signal
    /// fires; Rust exposes this as a method callers invoke when the dialog's
    /// observed signals (`fsb.file_trigger_signal`,
    /// `overwrite_dialog.finish_signal`) become pending.
    ///
    /// Behavior, beat-for-beat with C++:
    /// 1. If `Fsb->GetFileTriggerSignal()` is signaled, call `Finish(POSITIVE)`.
    /// 2. If `OverwriteDialog && IsSignaled(OverwriteDialog->GetFinishSignal())`:
    ///    - On POSITIVE: `OverwriteConfirmed = OverwriteAsked;
    ///      OverwriteAsked.Clear(); delete OverwriteDialog; Finish(POSITIVE);`
    ///    - On NEGATIVE: `OverwriteAsked.Clear(); delete OverwriteDialog;`
    ///
    /// Returns `true` if either branch executed (caller may wish to re-cycle).
    ///
    /// DIVERGED: C++ `emFileDialog::Cycle` (emFileDialog.cpp:80) is
    /// scheduler-dispatched automatically — the constructor calls
    /// `AddWakeUpSignal(Fsb->GetFileTriggerSignal())` and
    /// `AddWakeUpSignal(OverwriteDialog->GetFinishSignal())`, so the engine
    /// scheduler invokes `Cycle` as soon as either signal fires. Rust requires
    /// caller-driven invocation because `emFileDialog` is not registered as an
    /// `emEngine` — which in turn requires `emDialog` to become an `emEngine`
    /// first (it currently is not). Observable surface: the caller, not the
    /// scheduler, controls when signal-driven state transitions become visible.
    /// The signal-firing *beats* inside Cycle already match C++, so observers
    /// that connect to `finish_signal` see the result at the correct logical
    /// moment once Cycle is invoked — but the scheduler-versus-caller dispatch
    /// timing is not reproduced. Port plan: closure of this divergence is
    /// deferred to the phase that ports `emDialog` → `emEngine` with proper
    /// wake-up-signal subscription plumbing. Tracked by raw-material entry E024.
    pub fn Cycle(&mut self, ctx: &mut PanelCtx<'_>) -> bool {
        // Decide what transition to execute based on currently-pending
        // signals, then drop the SchedCtx borrow before enqueueing closures
        // via finish_post_show / pending_actions (which borrow ctx mutably).
        enum Action {
            None,
            FinishOk,
            OverwriteConfirmed,
            OverwriteCancelled,
        }
        let action = {
            let Some(sched) = ctx.as_sched_ctx() else {
                return false;
            };
            if sched.is_signaled(self.fsb_file_trigger_signal) {
                Action::FinishOk
            } else if let Some(od) = self.overwrite_dialog.as_ref() {
                if sched.is_signaled(od.finish_signal) {
                    // Read from the Cell shim populated by the overwrite
                    // dialog's on_finish closure (Task 19/20).
                    match self.overwrite_result.take() {
                        Some(DialogResult::Ok) => Action::OverwriteConfirmed,
                        Some(DialogResult::Cancel) => Action::OverwriteCancelled,
                        Some(DialogResult::Custom(_)) | None => Action::None,
                    }
                } else {
                    Action::None
                }
            } else {
                Action::None
            }
        };

        // Require pending_actions to be present (PanelCtx exposes it as an
        // Option field; Cycle's contract requires it to be Some — if it's
        // None we have no closure rail and must return false).
        if ctx.pending_actions.is_none() {
            return false;
        }

        match action {
            Action::None => false,
            Action::FinishOk => {
                self.dialog.finish_post_show(ctx, DialogResult::Ok);
                true
            }
            Action::OverwriteConfirmed => {
                self.overwrite_confirmed = std::mem::take(&mut self.overwrite_asked);
                if let Some(od) = self.overwrite_dialog.take() {
                    let did = od.dialog_id;
                    ctx.pending_actions().borrow_mut().push(Box::new(
                        move |app: &mut crate::emGUIFramework::App, _el| {
                            app.close_dialog_by_id(did);
                        },
                    ));
                }
                self.dialog.finish_post_show(ctx, DialogResult::Ok);
                true
            }
            Action::OverwriteCancelled => {
                self.overwrite_asked.clear();
                if let Some(od) = self.overwrite_dialog.take() {
                    let did = od.dialog_id;
                    ctx.pending_actions().borrow_mut().push(Box::new(
                        move |app: &mut crate::emGUIFramework::App, _el| {
                            app.close_dialog_by_id(did);
                        },
                    ));
                }
                // Outer dialog stays open; user may try save again.
                true
            }
        }
    }

    /// Signal fired when the dialog finishes (OK or Cancel). Observers
    /// connect their engines to this signal instead of polling. Alias for
    /// `self.dialog().finish_signal` — the underlying `emDialog::finish_signal`
    /// already satisfies the "result signal" role (landed in Task 3+4 bundle).
    pub fn finish_signal(&self) -> SignalId {
        self.dialog.finish_signal
    }

    /// Signal fired by the inner file-selection-box when a file is triggered
    /// (double-click / Enter). Exposed so observers/engines can wake on it,
    /// matching C++ `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`.
    pub fn file_trigger_signal(&self) -> SignalId {
        self.fsb_file_trigger_signal
    }

    /// Signal fired by the active overwrite-confirmation dialog, if any.
    /// Returns `None` while no overwrite dialog is pending. Observers may
    /// resubscribe each time `CheckFinish` produces
    /// `FileDialogCheckResult::ConfirmOverwrite` (mirrors C++
    /// `AddWakeUpSignal(OverwriteDialog->GetFinishSignal())`).
    pub fn overwrite_finish_signal(&self) -> Option<SignalId> {
        self.overwrite_dialog.as_ref().map(|d| d.finish_signal)
    }

    /// Test-only: populate the `overwrite_result` Cell as if the overwrite
    /// dialog's `on_finish` callback fired with `result`. The caller then fires
    /// `overwrite_finish_signal` manually so `Cycle` can observe both the Cell
    /// value and the signal in one shot.
    ///
    /// Ported from the `cfg(any())`-gated version (Task 21): the old body
    /// called `od.Finish` / `od.silent_cancel`, both of which are
    /// `unimplemented!` stubs in the new handle-based emDialog API. The Cell
    /// shim (`overwrite_result`) is the correct observable interface.
    #[cfg(test)]
    fn test_force_overwrite_result(&mut self, result: DialogResult) {
        self.overwrite_result.set(Some(result));
    }
}

fn mode_title_and_ok(mode: FileDialogMode) -> (&'static str, &'static str) {
    match mode {
        FileDialogMode::Select => ("Files", "OK"),
        FileDialogMode::Open => ("Open", "Open"),
        FileDialogMode::Save => ("Save As", "Save"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx};
    use crate::emScheduler::EngineScheduler;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<crate::emContext::emContext>,
        pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl Drop for TestInit {
        fn drop(&mut self) {
            // B3.4c: clear pending signals accumulated during Input-path tests
            self.sched.clear_pending_for_tests();
        }
    }

    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: crate::emContext::emContext::NewRoot(),
                pa: Rc::new(RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                pending_actions: &self.pa,
            }
        }
    }

    fn make_dialog(init: &mut TestInit, mode: FileDialogMode) -> emFileDialog {
        let look = emLook::new();
        emFileDialog::new(&mut init.ctx(), mode, look)
    }

    #[test]
    fn dialog_mode() {
        let mut __init = TestInit::new();
        let dlg = make_dialog(&mut __init, FileDialogMode::Select);
        assert_eq!(dlg.GetMode(), FileDialogMode::Select);
    }

    #[test]
    fn dialog_cancel_always_allowed() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Open);
        let result = dlg.CheckFinish(&mut __init.ctx(), &DialogResult::Cancel);
        assert!(matches!(result, FileDialogCheckResult::Allow));
    }

    #[test]
    fn dialog_open_no_selection_error() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Open);
        dlg.ClearSelection();
        let result = dlg.CheckFinish(&mut __init.ctx(), &DialogResult::Ok);
        assert!(matches!(result, FileDialogCheckResult::Error(_)));
    }

    #[test]
    fn multi_selection_forwarded() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Select);
        assert!(!dlg.is_multi_selection_enabled());
        dlg.set_multi_selection_enabled(true);
        assert!(dlg.is_multi_selection_enabled());
    }

    #[test]
    fn filters_forwarded() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Open);
        dlg.set_filters(&["All (*)".to_string()]);
        assert_eq!(dlg.GetFilters().len(), 1);
        assert_eq!(dlg.GetSelectedFilterIndex(), 0);
    }

    #[test]
    fn hidden_files_forwarded() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Select);
        assert!(!dlg.are_hidden_files_shown());
        dlg.set_hidden_files_shown(true);
        assert!(dlg.are_hidden_files_shown());
    }

    #[test]
    fn dir_result_default_disallowed() {
        let mut __init = TestInit::new();
        let dlg = make_dialog(&mut __init, FileDialogMode::Select);
        assert!(!dlg.is_directory_result_allowed());
    }

    // ─── Signal-driven Cycle tests (Phase-3 Task 6 / E024) ──────────────────
    //
    // In the new handle-based emDialog model, `Cycle` calls `finish_post_show`
    // which queues a closure onto `pending_actions` (rather than directly
    // setting a result field or firing `finish_signal`). The `finish_signal`
    // is subsequently fired by `DialogPrivateEngine` after the App processes
    // `pending_actions`. In unit tests there is no App event loop, so:
    //   - "dialog finished" is observable as `pending_actions` gaining ≥1 entry.
    //   - "dialog NOT finished" is observable as `pending_actions` staying empty.
    // The old assertions `dlg.GetResult()` and `sched.is_pending(finish)` are
    // replaced accordingly. `GetResult()` on emDialog is `unimplemented!` (dead
    // stub); `finish_signal` won't be pending until App processes the queue.

    use crate::emPanelTree::PanelTree;

    fn test_panel_tree() -> (PanelTree, crate::emPanelTree::PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    #[test]
    fn cycle_no_signals_is_no_op() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Open);
        // Drain any show-dialog actions queued by the constructor (AddCustomButton
        // / show) so the count is unambiguous after Cycle.
        __init.pa.borrow_mut().clear();
        let (mut tree, tid) = test_panel_tree();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        let mut ctx = PanelCtx::with_sched_reach(
            &mut tree,
            tid,
            1.0,
            &mut __init.sched,
            &mut __init.fw,
            &__init.root,
            &fw_cb,
            &__init.pa,
        );
        assert!(!dlg.Cycle(&mut ctx));
        // No action queued by Cycle: pending_actions stays empty.
        assert_eq!(__init.pa.borrow().len(), 0, "no pending actions expected");
    }

    #[test]
    fn cycle_file_trigger_signal_finishes_ok() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Open);
        // Drain constructor-queued show-dialog actions before counting.
        __init.pa.borrow_mut().clear();
        let trig = dlg.file_trigger_signal();
        __init.sched.fire(trig);

        let (mut tree, tid) = test_panel_tree();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            let acted = dlg.Cycle(&mut ctx);
            assert!(acted);
        }
        // finish_post_show enqueues a closure: pending_actions must be non-empty.
        // The finish_signal itself fires only after App processes the queue.
        assert!(
            !__init.pa.borrow().is_empty(),
            "finish_post_show must enqueue a pending action"
        );
    }

    #[test]
    fn cycle_overwrite_dialog_positive_confirms_and_finishes() {
        // Build a Save-mode dialog and force an overwrite_dialog via
        // CheckFinish against a path that exists.
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Save);
        let tmp = std::env::temp_dir();
        let f = tmp.join("emcore_filedialog_overwrite_test.tmp");
        std::fs::write(&f, b"x").expect("write tmp file");
        dlg.set_parent_directory(&tmp);
        dlg.set_selected_name("emcore_filedialog_overwrite_test.tmp");

        let result = dlg.CheckFinish(&mut __init.ctx(), &DialogResult::Ok);
        assert!(matches!(result, FileDialogCheckResult::ConfirmOverwrite(_)));
        let od_sig = dlg
            .overwrite_finish_signal()
            .expect("overwrite dialog created");

        // Simulate user clicking OK on the overwrite dialog: populate the Cell
        // shim and fire the overwrite dialog's finish_signal.
        dlg.test_force_overwrite_result(DialogResult::Ok);
        __init.sched.fire(od_sig);

        // Drain show-dialog actions queued by CheckFinish (AddCustomButton →
        // show) before calling Cycle, so pending_actions is clean going in.
        __init.pa.borrow_mut().clear();

        let (mut tree, tid) = test_panel_tree();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            assert!(dlg.Cycle(&mut ctx));
        }
        // Overwrite dialog must be gone from the handle.
        assert!(dlg.overwrite_finish_signal().is_none(), "od destroyed");
        // Cycle enqueues: (a) close overwrite dialog closure, (b) finish_post_show
        // closure for the outer dialog. At least one pending action required.
        assert!(
            !__init.pa.borrow().is_empty(),
            "Cycle must enqueue pending actions on overwrite-confirmed path"
        );

        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn cycle_overwrite_dialog_negative_cancels_overwrite_only() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Save);
        let tmp = std::env::temp_dir();
        let f = tmp.join("emcore_filedialog_overwrite_neg_test.tmp");
        std::fs::write(&f, b"x").expect("write tmp file");
        dlg.set_parent_directory(&tmp);
        dlg.set_selected_name("emcore_filedialog_overwrite_neg_test.tmp");

        let _ = dlg.CheckFinish(&mut __init.ctx(), &DialogResult::Ok);
        let od_sig = dlg.overwrite_finish_signal().expect("od created");
        dlg.test_force_overwrite_result(DialogResult::Cancel);
        __init.sched.fire(od_sig);

        // Drain show-dialog actions queued by CheckFinish before calling Cycle.
        __init.pa.borrow_mut().clear();

        let (mut tree, tid) = test_panel_tree();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            assert!(dlg.Cycle(&mut ctx));
        }
        // Overwrite dialog handle cleared.
        assert!(dlg.overwrite_finish_signal().is_none(), "od destroyed");
        // Cancel path: outer dialog stays open — Cycle enqueues only the
        // close-overwrite-dialog action, NOT a finish_post_show for the outer
        // dialog. The outer dialog remains live (no finish closure).
        // We verify by checking the overwrite_result Cell is empty (taken by
        // Cycle) and the outer dialog's finish_signal has NOT been scheduled
        // (finish_post_show not called).
        assert!(
            dlg.overwrite_result.get().is_none(),
            "overwrite_result Cell must be empty after Cycle takes it"
        );
        assert_eq!(
            __init.pa.borrow().len(),
            1,
            "cancel path must enqueue exactly 1 action (close-od), not a finish"
        );

        let _ = std::fs::remove_file(&f);
    }

    // ─── End-to-end overwrite-confirm test (Task 21) ─────────────────────────
    //
    // Full e2e test verifying the overwrite-confirm path from CheckFinish →
    // Cell-shim → Cycle → finish_post_show pending action.
    //
    // Infrastructure note: the heavy e2e shape described in the plan
    // (fire button click-signal → App processes pending_actions → outer dialog
    // finalized_result == Some(Ok)) requires a live App + DialogPrivateEngine
    // event loop, which is App-level integration test infrastructure not yet
    // available at this scope. Per the plan's lighter-test fallback, this test
    // verifies the observable Cell-shim path: after CheckFinish creates the
    // overwrite dialog, after we populate the Cell and fire the signal, Cycle
    // correctly reads the Cell and enqueues pending actions for both the
    // close-overwrite-dialog step and the outer finish_post_show step.
    #[test]
    fn save_existing_file_triggers_overwrite_dialog_and_confirms() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Save);

        // Create a real temp file so CheckFinish detects it as existing.
        let tmp = std::env::temp_dir();
        let f = tmp.join("emcore_e2e_overwrite_confirm.tmp");
        std::fs::write(&f, b"existing").expect("write tmp file");
        dlg.set_parent_directory(&tmp);
        dlg.set_selected_name("emcore_e2e_overwrite_confirm.tmp");

        // Step 1: CheckFinish → ConfirmOverwrite (overwrite dialog created).
        let check = dlg.CheckFinish(&mut __init.ctx(), &DialogResult::Ok);
        assert!(
            matches!(check, FileDialogCheckResult::ConfirmOverwrite(_)),
            "expected ConfirmOverwrite, got {check:?}"
        );
        let od_sig = dlg
            .overwrite_finish_signal()
            .expect("overwrite dialog must exist after ConfirmOverwrite");

        // Step 2: Simulate the user clicking OK on the overwrite dialog —
        // populate the Cell shim (mirrors what the on_finish closure does in
        // production) and fire the overwrite dialog's finish_signal.
        dlg.test_force_overwrite_result(DialogResult::Ok);
        assert_eq!(
            dlg.overwrite_result.get(),
            Some(DialogResult::Ok),
            "Cell must hold Ok before Cycle reads it"
        );
        __init.sched.fire(od_sig);

        // Drain show-dialog actions accumulated during CheckFinish so the
        // pending_actions count is unambiguous after Cycle.
        __init.pa.borrow_mut().clear();

        // Step 3: Cycle observes the signal + Cell → OverwriteConfirmed path.
        let (mut tree, tid) = test_panel_tree();
        let fw_cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        let acted = {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                tid,
                1.0,
                &mut __init.sched,
                &mut __init.fw,
                &__init.root,
                &fw_cb,
                &__init.pa,
            );
            dlg.Cycle(&mut ctx)
        };
        assert!(acted, "Cycle must return true on OverwriteConfirmed");

        // Step 4: Verify observable post-Cycle state.
        //   (a) overwrite_result Cell consumed (taken to None by Cycle).
        assert!(
            dlg.overwrite_result.get().is_none(),
            "Cell must be empty after Cycle takes it"
        );
        //   (b) overwrite dialog handle cleared.
        assert!(
            dlg.overwrite_finish_signal().is_none(),
            "overwrite dialog handle must be None after Cycle"
        );
        //   (c) pending_actions has at least 2 entries: close-overwrite-dialog
        //       closure + finish_post_show closure for the outer dialog.
        let pa_count = __init.pa.borrow().len();
        assert!(
            pa_count >= 2,
            "expected ≥2 pending actions (close od + finish outer), got {pa_count}"
        );

        let _ = std::fs::remove_file(&f);
    }
}
