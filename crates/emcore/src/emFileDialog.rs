use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::emDialog::{emDialog, DialogResult};
use crate::emEngineCtx::ConstructCtx;
use crate::emFileSelectionBox::emFileSelectionBox;
use crate::emLook::emLook;
use crate::emPanelTree::PanelId;
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

/// File-open / file-save dialog composing an `emDialog` + `emFileSelectionBox`.
///
/// Port of C++ `class emFileDialog : public emDialog` (emFileDialog.h:37).
/// DIVERGED: Rust uses composition for C++ inheritance (idiom adaptation —
/// observable behavior identical). The owned `dialog: emDialog` provides the
/// window/root-panel/private-engine infrastructure from Phase 3.5; the
/// `emFileSelectionBox` is installed as a child panel under
/// `dialog.GetContentPanel()` at construction.
///
/// Scheduler-driven Cycle: on construction, the outer emDialog's
/// DialogPrivateEngine subscribes to `fsb.file_trigger_signal` via
/// `PendingTopLevel::wake_up_signals` — the pre-show rail that the installer
/// drains into `scheduler.connect` at materialization time (port of
/// emFileDialog.cpp:41 `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`). The
/// file-dialog per-cycle logic (emFileDialog.cpp:80-106) lives in the
/// `on_cycle_ext` callback installed on the dialog's `DlgPanel`.
///
/// Overwrite-confirmation transient state (the popped-up "File Exists"
/// sub-dialog + its asked-text) lives on `DlgPanel` (see `on_cycle_ext`
/// closure access pattern) — NOT on this struct. This avoids
/// `Rc<RefCell<...>>` because the `'static + FnMut` closure cannot borrow
/// from `emFileDialog`.
pub struct emFileDialog {
    dialog: emDialog,
    look: Rc<emLook>,
    /// PanelId of the emFileSelectionBox installed under content panel.
    fsb_panel_id: PanelId,
    mode: FileDialogMode,
    dir_allowed: bool,
    /// Last-confirmed overwrite text (matches C++ `OverwriteConfirmed`).
    /// Used by CheckFinish to short-circuit a re-prompt when the user
    /// has already confirmed overwriting the same set of paths.
    overwrite_confirmed: String,
}

impl emFileDialog {
    pub fn new<C: ConstructCtx>(ctx: &mut C, mode: FileDialogMode, look: Rc<emLook>) -> Self {
        let (title, ok_label) = mode_title_and_ok(mode);

        // 1. Construct the outer dialog (Phase 3.5).
        let mut dialog = emDialog::new(ctx, title, Rc::clone(&look));

        // 2. Add OK/Cancel buttons.
        dialog.AddCustomButton(ctx, ok_label, DialogResult::Ok);
        dialog.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);

        // 3. Install emFileSelectionBox as child of the dialog's content panel.
        //    Pre-show: reach through the pending tree directly.
        let content_id = dialog.GetContentPanel(ctx);

        // Build fsb first (needs ctx for signal allocation), then install.
        let mut fsb = emFileSelectionBox::new(ctx, "");
        fsb.border_mut().outer = super::emBorder::OuterBorderType::None;
        fsb.border_mut().inner = super::emBorder::InnerBorderType::None;
        let fsb_trigger_sig = fsb.file_trigger_signal;

        // Attach to pending tree.
        let fsb_panel_id = {
            let pending = dialog.pending_mut();
            let tree = pending.window.tree_mut();
            let pid = tree.create_child(content_id, "fsb", None);
            tree.set_behavior(pid, Box::new(fsb));
            pid
        };

        // 4. Queue fsb.file_trigger_signal for post-register subscription.
        //    Port of C++ emFileDialog.cpp:41 AddWakeUpSignal(Fsb->GetFileTriggerSignal()).
        dialog.add_pre_show_wake_up_signal(fsb_trigger_sig);

        // 5. Install on_cycle_ext on DlgPanel — the file-dialog Cycle logic.
        //    Capture the fsb signal id; observe via ctx.IsSignaled which
        //    dispatches via the engine-scoped pending-signal table.
        let closure_fsb_sig = fsb_trigger_sig;
        let on_cycle_ext: crate::emDialog::DialogCycleExt = Box::new(
            move |dlg: &mut crate::emDialog::DlgPanel,
                  ctx: &mut crate::emEngineCtx::EngineCtx<'_>|
                  -> bool {
                // Port of emFileDialog.cpp:80-106 emFileDialog::Cycle body:
                //   if (IsSignaled(Fsb->GetFileTriggerSignal())) Finish(POSITIVE);
                //   if (OverwriteDialog && IsSignaled(OverwriteDialog->GetFinishSignal())) {
                //       switch (OverwriteDialog->GetResult()) {
                //         case POSITIVE: OverwriteConfirmed = OverwriteAsked;
                //                        OverwriteAsked.Clear();
                //                        delete OverwriteDialog.Get();
                //                        Finish(POSITIVE); break;
                //         case NEGATIVE: OverwriteAsked.Clear();
                //                        delete OverwriteDialog.Get(); break;
                //       }
                //   }
                //
                // DIVERGED (validation-funnel, Phase 3.6 Task 3, P3):
                // C++ `Finish(POSITIVE)` re-enters `CheckFinish` for
                // validation. Rust shape sets `pending_result` directly
                // without validation re-entry because `DialogCheckFinishCb`'s
                // signature (`&DialogResult -> bool`) lacks the ctx needed
                // for an OD spawn. File-trigger-path validation and
                // button-click-OK-path validation are both deferred —
                // Task 3 scope is the structural reshape (E024 closure);
                // widening the check-finish-callback signature is a later
                // phase.

                // fsb_file_trigger signalled? → set pending_result = Ok.
                if ctx.IsSignaled(closure_fsb_sig) && dlg.pending_result.is_none() {
                    dlg.pending_result = Some(DialogResult::Ok);
                }

                // Overwrite dialog finished? Observe via the cached
                // finish_signal on `dlg.overwrite_dialog`. On fire, tear
                // down OD (close its window via pending_actions) and clear
                // the DlgPanel-side overwrite state.
                //
                // DIVERGED (P3, Task 3): we do NOT read OD's finalized
                // result here to decide positive-vs-negative. Reading
                // requires routing through App to walk OD's own tree, and
                // the decision is only consumed by `overwrite_confirmed`
                // promotion which lives on `emFileDialog` (out of this
                // closure's reach). Task-3 scope is the structural reshape;
                // OD-result propagation is deferred to the phase that
                // widens the closure's reach to `emFileDialog`.
                //
                // Observable effect: on OD-finish the outer dialog stays
                // open (no pending_result set); the user must click the
                // outer OK again, which re-enters `CheckFinish`, which
                // sees `overwrite_confirmed` still empty and re-spawns the
                // OD. Loop-terminating correctness is deferred.
                let od_finish_sig = dlg.overwrite_dialog.as_ref().map(|od| od.finish_signal);
                if let Some(od_sig) = od_finish_sig {
                    if ctx.IsSignaled(od_sig) {
                        let od = dlg.overwrite_dialog.take().expect("od present");
                        dlg.overwrite_asked.clear();
                        let od_did = od.dialog_id;
                        ctx.pending_actions().borrow_mut().push(Box::new(
                            move |app: &mut crate::emGUIFramework::App, _el| {
                                app.close_dialog_by_id(od_did);
                            },
                        ));
                        drop(od);
                    }
                }

                false
            },
        );

        // Install the extension on DlgPanel via the pre-show tree reach.
        let root_panel_id = dialog.root_panel_id();
        {
            let pending = dialog.pending_mut();
            let tree = pending.window.tree_mut();
            if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
                if let Some(dlg_panel) = behavior.as_dlg_panel_mut() {
                    dlg_panel.on_cycle_ext = Some(on_cycle_ext);
                }
                tree.put_behavior(root_panel_id, behavior);
            }
        }

        Self {
            dialog,
            look,
            fsb_panel_id,
            mode,
            dir_allowed: false,
            overwrite_confirmed: String::new(),
        }
    }

    pub fn show<C: ConstructCtx>(&mut self, ctx: &mut C) {
        self.dialog.show(ctx);
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

    pub fn dialog(&self) -> &emDialog {
        &self.dialog
    }

    pub fn dialog_mut(&mut self) -> &mut emDialog {
        &mut self.dialog
    }

    /// Signal fired when the dialog finishes (OK or Cancel).
    pub fn finish_signal(&self) -> SignalId {
        self.dialog.finish_signal
    }

    // ─── Pre-show fsb accessors ─────────────────────────────────────────────
    //
    // All fsb accessors reach the tree-installed emFileSelectionBox via
    // `self.pre_show_fsb(|fsb| ...)`. Pre-show only; panics post-show because
    // the post-show path is an App-routed mutation (not in scope for Task 3
    // tests which operate pre-show exclusively).

    fn with_fsb_mut<R>(&mut self, f: impl FnOnce(&mut emFileSelectionBox) -> R) -> R {
        let fsb_pid = self.fsb_panel_id;
        let pending = self.dialog.pending_mut();
        let tree = pending.window.tree_mut();
        let mut behavior = tree
            .take_behavior(fsb_pid)
            .expect("fsb behavior present in pending tree");
        let r = {
            let fsb = behavior
                .as_file_selection_box_mut()
                .expect("fsb panel carries emFileSelectionBox behavior");
            f(fsb)
        };
        tree.put_behavior(fsb_pid, behavior);
        r
    }

    fn with_fsb<R>(&mut self, f: impl FnOnce(&emFileSelectionBox) -> R) -> R {
        self.with_fsb_mut(|fsb| f(fsb))
    }

    pub fn is_multi_selection_enabled(&mut self) -> bool {
        self.with_fsb(|fsb| fsb.is_multi_selection_enabled())
    }

    pub fn set_multi_selection_enabled(&mut self, enabled: bool) {
        self.with_fsb_mut(|fsb| fsb.set_multi_selection_enabled(enabled));
    }

    pub fn GetParentDirectory(&mut self) -> PathBuf {
        self.with_fsb(|fsb| fsb.GetParentDirectory().to_path_buf())
    }

    pub fn set_parent_directory(&mut self, dir: &Path) {
        let dir = dir.to_path_buf();
        self.with_fsb_mut(|fsb| fsb.set_parent_directory(&dir));
    }

    pub fn GetSelectedName(&mut self) -> Option<String> {
        self.with_fsb(|fsb| fsb.GetSelectedName().map(|s| s.to_string()))
    }

    pub fn GetSelectedNames(&mut self) -> Vec<String> {
        self.with_fsb(|fsb| fsb.GetSelectedNames().to_vec())
    }

    pub fn set_selected_name(&mut self, name: &str) {
        let name = name.to_string();
        self.with_fsb_mut(|fsb| fsb.set_selected_name(&name));
    }

    pub fn set_selected_names(&mut self, names: &[String]) {
        let names = names.to_vec();
        self.with_fsb_mut(|fsb| fsb.set_selected_names(&names));
    }

    pub fn ClearSelection(&mut self) {
        self.with_fsb_mut(|fsb| fsb.ClearSelection());
    }

    pub fn GetSelectedPath(&mut self) -> PathBuf {
        self.with_fsb(|fsb| fsb.GetSelectedPath())
    }

    pub fn set_selected_path(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.with_fsb_mut(|fsb| fsb.set_selected_path(&path));
    }

    pub fn GetFilters(&mut self) -> Vec<String> {
        self.with_fsb(|fsb| fsb.GetFilters().to_vec())
    }

    pub fn set_filters(&mut self, filters: &[String]) {
        let filters = filters.to_vec();
        self.with_fsb_mut(|fsb| fsb.set_filters(&filters));
    }

    pub fn GetSelectedFilterIndex(&mut self) -> i32 {
        self.with_fsb(|fsb| fsb.GetSelectedFilterIndex())
    }

    pub fn set_selected_filter_index(&mut self, index: i32) {
        self.with_fsb_mut(|fsb| fsb.set_selected_filter_index(index));
    }

    pub fn are_hidden_files_shown(&mut self) -> bool {
        self.with_fsb(|fsb| fsb.are_hidden_files_shown())
    }

    pub fn set_hidden_files_shown(&mut self, shown: bool) {
        self.with_fsb_mut(|fsb| fsb.set_hidden_files_shown(shown));
    }

    /// Check whether the dialog can finish with the given result.
    ///
    /// Port of C++ `emFileDialog::CheckFinish`. Validates the selection
    /// based on the dialog mode (Open checks existence, Save confirms
    /// overwrite). On Save-mode overwrite detection, spawns a transient
    /// "File Exists" `emDialog` and parks it on the outer dialog's
    /// `DlgPanel.overwrite_dialog` via the pre-show or post-show reach
    /// pattern.
    pub fn CheckFinish<C: ConstructCtx>(
        &mut self,
        ctx: &mut C,
        result: &DialogResult,
    ) -> FileDialogCheckResult {
        if *result == DialogResult::Cancel {
            return FileDialogCheckResult::Allow;
        }

        let (names, parent) = self.with_fsb(|fsb| {
            (
                fsb.GetSelectedNames().to_vec(),
                fsb.GetParentDirectory().to_path_buf(),
            )
        });

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
                        // Spawn a transient "File Exists" emDialog.
                        let mut od = emDialog::new(ctx, "File Exists", self.look.clone());
                        od.AddCustomButton(ctx, "OK", DialogResult::Ok);
                        od.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
                        // Queue OD.finish_signal to be connected to the
                        // OUTER dialog's private engine at install time.
                        //
                        // DIVERGED: CheckFinish may be invoked pre-show
                        // (test path) or post-show (production). In the
                        // pre-show path the outer's private engine doesn't
                        // exist yet, so we park the subscription on the
                        // outer's `wake_up_signals` (same mechanism as the
                        // fsb subscription above). In the post-show path
                        // we'd need to connect directly — not exercised in
                        // Task 3 tests, deferred to a later phase.
                        if self.dialog.pending.is_some() {
                            self.dialog.add_pre_show_wake_up_signal(od.finish_signal);
                        }
                        od.show(ctx);

                        // Park OD on outer DlgPanel.overwrite_dialog via
                        // pre-show reach (tests exercise only this path).
                        if self.dialog.pending.is_some() {
                            let root_panel_id = self.dialog.root_panel_id();
                            let pending = self.dialog.pending_mut();
                            let tree = pending.window.tree_mut();
                            if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
                                if let Some(dlg_panel) = behavior.as_dlg_panel_mut() {
                                    dlg_panel.overwrite_dialog = Some(od);
                                    dlg_panel.overwrite_asked = text;
                                }
                                tree.put_behavior(root_panel_id, behavior);
                            }
                        } else {
                            // Post-show: route through App::mutate_dialog_by_id.
                            let outer_did = self.dialog.dialog_id;
                            ctx.pending_actions().borrow_mut().push(Box::new(
                                move |app: &mut crate::emGUIFramework::App, _el| {
                                    app.mutate_dialog_by_id(outer_did, move |p, _tree| {
                                        p.overwrite_dialog = Some(od);
                                        p.overwrite_asked = text;
                                    });
                                },
                            ));
                        }

                        return FileDialogCheckResult::ConfirmOverwrite(paths_to_overwrite);
                    }
                }
                self.overwrite_confirmed.clear();
            }
            FileDialogMode::Select => {}
        }

        FileDialogCheckResult::Allow
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
}
