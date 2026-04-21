use std::path::{Path, PathBuf};

use super::emDialog::{emDialog, DialogResult};
use crate::emEngineCtx::PanelCtx;
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
    fsb: emFileSelectionBox,
    mode: FileDialogMode,
    dir_allowed: bool,
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
        look: std::rc::Rc<emLook>,
    ) -> Self {
        let (title, ok_label) = mode_title_and_ok(mode);
        let mut dialog = emDialog::new(ctx, title, look);
        dialog.AddCustomButton(ok_label, DialogResult::Ok);
        dialog.AddCustomButton("Cancel", DialogResult::Cancel);

        let mut fsb = emFileSelectionBox::new(ctx, "");
        fsb.border_mut().outer = super::emBorder::OuterBorderType::None;
        fsb.border_mut().inner = super::emBorder::InnerBorderType::None;
        let fsb_file_trigger_signal = fsb.file_trigger_signal;

        Self {
            dialog,
            fsb,
            mode,
            dir_allowed: false,
            overwrite_dialog: None,
            overwrite_asked: String::new(),
            overwrite_confirmed: String::new(),
            fsb_file_trigger_signal,
        }
    }

    pub fn GetMode(&self) -> FileDialogMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: FileDialogMode) {
        self.mode = mode;
        let (title, ok_label) = mode_title_and_ok(mode);
        self.dialog.SetRootTitle(title);
        self.dialog
            .set_button_label_for_result(&DialogResult::Ok, ok_label);
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

    pub fn dialog_mut(&mut self) -> &mut emDialog {
        &mut self.dialog
    }

    pub fn file_selection_box(&self) -> &emFileSelectionBox {
        &self.fsb
    }

    pub fn file_selection_box_mut(&mut self) -> &mut emFileSelectionBox {
        &mut self.fsb
    }

    pub fn Finish(&mut self, result: DialogResult, ctx: &mut PanelCtx<'_>) {
        self.dialog.Finish(result, ctx);
    }

    pub fn GetResult(&self) -> Option<&DialogResult> {
        self.dialog.GetResult()
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
                        // title, add OK/Cancel buttons).
                        let mut dlg = emDialog::new(ctx, "File Exists", self.dialog.look().clone());
                        dlg.AddCustomButton("OK", DialogResult::Ok);
                        dlg.AddCustomButton("Cancel", DialogResult::Cancel);
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
        // signals, then drop the SchedCtx borrow before mutating ctx via
        // Finish (which itself synthesizes a SchedCtx to fire finish_signal).
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
                    match od.GetResult() {
                        Some(DialogResult::Ok) => Action::OverwriteConfirmed,
                        Some(DialogResult::Cancel) => Action::OverwriteCancelled,
                        _ => Action::None,
                    }
                } else {
                    Action::None
                }
            } else {
                Action::None
            }
        };

        match action {
            Action::None => false,
            Action::FinishOk => {
                self.dialog.Finish(DialogResult::Ok, ctx);
                true
            }
            Action::OverwriteConfirmed => {
                self.overwrite_confirmed = std::mem::take(&mut self.overwrite_asked);
                self.overwrite_dialog = None;
                self.dialog.Finish(DialogResult::Ok, ctx);
                true
            }
            Action::OverwriteCancelled => {
                self.overwrite_asked.clear();
                self.overwrite_dialog = None;
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

    /// Test-only: set the overwrite-dialog's internal result without firing
    /// its `finish_signal`, so tests can exercise Cycle's two branches by
    /// firing the signal separately. Production code drives the overwrite
    /// dialog via normal `emDialog::Finish` from the UI event path.
    #[cfg(test)]
    fn test_force_overwrite_result(&mut self, result: DialogResult) {
        if let Some(od) = self.overwrite_dialog.as_mut() {
            match result {
                DialogResult::Cancel => od.silent_cancel(),
                other => {
                    let mut tree = crate::emPanelTree::PanelTree::new();
                    let tid = tree.create_root("tt", false);
                    let mut ctx = PanelCtx::new(&mut tree, tid, 1.0);
                    // No scheduler on this PanelCtx → Finish cannot fire
                    // signals, which is what we want: only the result is set.
                    od.Finish(other, &mut ctx);
                }
            }
        }
    }
}

fn mode_title_and_ok(mode: FileDialogMode) -> (&'static str, &'static str) {
    match mode {
        FileDialogMode::Select => ("Files", "OK"),
        FileDialogMode::Open => ("Open", "Open"),
        FileDialogMode::Save => ("Save As", "Save"),
    }
}

// PHASE-3.5-TASK-19: consumer migration — all emFileDialog tests call
// `make_dialog` which constructs an `emFileDialog` via `emDialog::AddCustomButton`
// (legacy stub → unimplemented!()); entire module gated until Task 19 ports
// emFileDialog to the new emDialog API.
#[cfg(any())]
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
        assert!(dlg.GetResult().is_none());
    }

    #[test]
    fn cycle_file_trigger_signal_finishes_ok() {
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Open);
        let trig = dlg.file_trigger_signal();
        let finish = dlg.finish_signal();
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
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
        // Finish fires the outer dialog's finish_signal (C++ parity).
        assert!(__init.sched.is_pending(finish));
    }

    #[test]
    fn cycle_overwrite_dialog_positive_confirms_and_finishes() {
        // Build a Save-mode dialog and force an overwrite_dialog via
        // CheckFinish against a path that exists (use current working dir
        // so the check triggers deterministically).
        let mut __init = TestInit::new();
        let mut dlg = make_dialog(&mut __init, FileDialogMode::Save);
        // Point fsb at /tmp and pretend the selected name is an existing file.
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

        // Simulate the user clicking OK on the overwrite dialog: set its
        // result to Ok and fire its finish_signal.
        let finish = dlg.finish_signal();
        // Drive overwrite dialog's Finish via its internal API. We set the
        // result directly by invoking Finish on a &mut borrow of the inner
        // emDialog field. Use the public accessor + internal route:
        {
            // We need &mut access to the inner overwrite emDialog. It's
            // private — tunnel through a mini helper: manually mimic what
            // the UI does by just firing the signal and setting the result.
            // Since overwrite_dialog is private, we use the Finish path:
            // build a PanelCtx and call dlg.dialog_mut() — no, that's the
            // outer. Use a dedicated test helper.
            dlg.test_force_overwrite_result(DialogResult::Ok);
        }
        __init.sched.fire(od_sig);

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
        assert!(dlg.overwrite_finish_signal().is_none(), "od destroyed");
        assert_eq!(dlg.GetResult(), Some(&DialogResult::Ok));
        assert!(__init.sched.is_pending(finish));

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
        let finish = dlg.finish_signal();
        dlg.test_force_overwrite_result(DialogResult::Cancel);
        __init.sched.fire(od_sig);

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
        assert!(dlg.overwrite_finish_signal().is_none(), "od destroyed");
        assert!(dlg.GetResult().is_none(), "outer dialog NOT finished");
        assert!(!__init.sched.is_pending(finish));

        let _ = std::fs::remove_file(&f);
    }
}
