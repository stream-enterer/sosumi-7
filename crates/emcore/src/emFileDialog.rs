use std::path::{Path, PathBuf};

use super::emDialog::{emDialog, DialogResult};
use crate::emEngineCtx::PanelCtx;
use crate::emFileSelectionBox::emFileSelectionBox;
use crate::emLook::emLook;

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
    // DIVERGED: C++ `OverwriteDialog` is `emCrossPtr<emDialog>` — a cross-pointer
    // to a dynamically-created child dialog in the panel tree, polled each Cycle().
    // Rust has no signal/cycle infrastructure and emDialog is not wrapped in
    // Rc<RefCell<...>>, so we use `Option<emDialog>` to track the pending overwrite
    // confirmation dialog. The caller retrieves it via `overwrite_dialog()` and feeds
    // back the user's choice via `confirm_overwrite()` / `cancel_overwrite()`.
    overwrite_dialog: Option<emDialog>,
    overwrite_asked: String,
    overwrite_confirmed: String,
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

        Self {
            dialog,
            fsb,
            mode,
            dir_allowed: false,
            overwrite_dialog: None,
            overwrite_asked: String::new(),
            overwrite_confirmed: String::new(),
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

    /// Access the pending overwrite confirmation dialog, if any.
    ///
    /// DIVERGED: C++ `OverwriteDialog` is `emCrossPtr<emDialog>` polled in
    /// `Cycle()`. Rust exposes `Option<&emDialog>` for the caller to present
    /// and relay the user's answer via `confirm_overwrite()` /
    /// `cancel_overwrite()`.
    pub fn overwrite_dialog(&self) -> Option<&emDialog> {
        self.overwrite_dialog.as_ref()
    }

    /// Confirm overwrite (called after user confirms in the overwrite dialog).
    ///
    /// Port of C++ `Cycle()` POSITIVE branch (lines 92-96): copies
    /// `OverwriteAsked` into `OverwriteConfirmed`, clears `OverwriteAsked`,
    /// and destroys the dialog.
    pub fn confirm_overwrite(&mut self) {
        self.overwrite_confirmed = self.overwrite_asked.clone();
        self.overwrite_asked.clear();
        self.overwrite_dialog = None;
    }

    /// Cancel overwrite request (called after user cancels the overwrite
    /// dialog).
    ///
    /// Port of C++ `Cycle()` NEGATIVE branch (lines 98-100): clears
    /// `OverwriteAsked` and destroys the dialog.
    pub fn cancel_overwrite(&mut self) {
        self.overwrite_asked.clear();
        self.overwrite_dialog = None;
    }

    /// Handle triggering a file (double-click / enter). Returns true if dialog
    /// should finish with a positive result.
    pub fn handle_file_trigger(&mut self) -> bool {
        // File trigger means the user wants to confirm the selection.
        true
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

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: std::rc::Rc<crate::emContext::emContext>,
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
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
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
