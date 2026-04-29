use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::emDialog::{emDialog, DialogResult, DlgPanel};
use crate::emEngineCtx::{ConstructCtx, EngineCtx};
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
/// DIVERGED: (language-forced) Rust uses composition for C++ inheritance (idiom adaptation —
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
/// sub-dialog, its asked-text, AND its confirmed-text) lives on `DlgPanel`
/// (see `on_cycle_ext` closure access pattern) — NOT on this struct. This
/// avoids `Rc<RefCell<...>>` because the `'static + FnMut` closure cannot
/// borrow from `emFileDialog`. Phase 3.6 Task 3 fix: `overwrite_confirmed`
/// moved to `DlgPanel` alongside `overwrite_asked` so the closure can
/// promote `asked → confirmed` on OD POSITIVE (C++ emFileDialog.cpp:93).
pub struct emFileDialog {
    dialog: emDialog,
    look: Rc<emLook>,
    /// PanelId of the emFileSelectionBox installed under content panel.
    fsb_panel_id: PanelId,
    /// Cached `fsb.file_trigger_signal` — `SignalId` is `Copy`, stable
    /// across fsb lifetime. Test-only cache: backs the
    /// `file_trigger_signal()` accessor used by Rust unit tests to fire the
    /// signal externally without walking the tree. The closure in
    /// `on_cycle_ext` captures the same SignalId via move-capture at
    /// construction and does NOT read this field; production code paths
    /// never touch it. C++ `emFileDialog` has no equivalent accessor
    /// because C++ tests reach the signal through different mechanics.
    fsb_trigger_sig: SignalId,
    mode: FileDialogMode,
    dir_allowed: bool,
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
        //    Capture the fsb signal id + outer DialogId; observe via
        //    ctx.IsSignaled which dispatches via the engine-scoped
        //    pending-signal table.
        let closure_fsb_sig = fsb_trigger_sig;
        let outer_did = dialog.dialog_id;
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
                // Phase 3.6.1 Task 2: P3 divergence closed. Setting
                // `pending_result = Some(Ok)` on file-trigger now re-enters
                // the widened `on_check_finish` closure via base cycle
                // step 3 — single funnel, matches C++.

                // Re-wake protocol: the base `DialogPrivateEngine::Cycle` body
                // runs BEFORE on_cycle_ext (Phase-3.6 Task 2 ordering), so
                // state mutations this closure makes (`dlg.pending_result`,
                // `pending_actions`) are not visible to the base body this
                // Cycle. Return `true` on any mutation to keep the engine
                // awake so the next Cycle's base body observes them. C++'s
                // same-call-stack `Finish(POSITIVE)` re-entry achieves the
                // same finalize ordering directly.
                let mut stay = false;
                if ctx.IsSignaled(closure_fsb_sig) && dlg.pending_result.is_none() {
                    dlg.pending_result = Some(DialogResult::Ok);
                    stay = true;
                }

                // Overwrite dialog finished? Observe via the cached
                // finish_signal on `dlg.overwrite_dialog`.
                //
                // Reading OD's `finalized_result` requires crossing
                // windows (OD is a separate top-level). We defer the
                // read + promotion + teardown to a single pending_actions
                // closure (runs with `&mut App` after this time slice),
                // which uses `App::mutate_dialog_by_id` for both OD and
                // outer. Promotion is committed in that closure via
                // mutate_dialog_by_id(outer_did, ...), matching C++
                // emFileDialog.cpp:93-96 (POSITIVE) / :98-101 (NEGATIVE).
                let od_finish_sig = dlg.overwrite_dialog.as_ref().map(|od| od.finish_signal);
                if let Some(od_sig) = od_finish_sig {
                    if ctx.IsSignaled(od_sig) {
                        let od = dlg.overwrite_dialog.take().expect("od present");
                        let od_did = od.dialog_id;
                        // Take asked out of DlgPanel — it is consumed by
                        // the deferred closure regardless of branch.
                        let asked = std::mem::take(&mut dlg.overwrite_asked);
                        // Drop the OD handle here; the winit surface is
                        // torn down by `close_dialog_by_id` below.
                        drop(od);

                        ctx.pending_actions().borrow_mut().push(Box::new(
                            move |app: &mut crate::emGUIFramework::App, _el| {
                                // Read OD's finalized_result via the
                                // App helper: production walks the OD's
                                // window tree; #[cfg(test)] first consults
                                // `headless_dialog_results` so a test that
                                // could not install OD as a second
                                // headless top-level (winit WindowId::dummy()
                                // collision) can pre-seed a result.
                                let od_result = app.read_dialog_finalized_result(od_did);
                                match od_result {
                                    Some(DialogResult::Ok) => {
                                        // C++ emFileDialog.cpp:93-96:
                                        //   OverwriteConfirmed = OverwriteAsked;
                                        //   OverwriteAsked.Clear();
                                        //   delete OverwriteDialog;
                                        //   Finish(POSITIVE);
                                        app.mutate_dialog_by_id(outer_did, |outer_dlg, _tree| {
                                            outer_dlg.overwrite_confirmed = asked;
                                            // overwrite_asked was
                                            // already cleared by
                                            // mem::take in the closure.
                                            if outer_dlg.pending_result.is_none()
                                                && outer_dlg.finalized_result.is_none()
                                            {
                                                outer_dlg.pending_result = Some(DialogResult::Ok);
                                            }
                                        });
                                    }
                                    _ => {
                                        // C++ emFileDialog.cpp:98-101:
                                        //   OverwriteAsked.Clear();
                                        //   delete OverwriteDialog;
                                        // (outer stays open — user can
                                        // change filename). overwrite_asked
                                        // already cleared above. NEGATIVE
                                        // (Cancel), None (OD still not
                                        // finalized — shouldn't happen
                                        // given finish_signal fired), and
                                        // any Custom result all fall here.
                                    }
                                }
                                app.close_dialog_by_id(od_did);
                            },
                        ));
                        stay = true;
                    }
                }

                stay
            },
        );

        // Phase 3.6.1 Task 2: install on_check_finish closure — the
        // validation funnel for both fsb-trigger and button-click OK
        // paths. Captures nothing; reads fsb_panel_id + mode +
        // dir_allowed fresh from `outer_dlg` each fire (fields mirrored
        // onto DlgPanel below). Delegates to `run_file_dialog_check_finish`
        // which performs the dir-check + Open-existence + Save-overwrite
        // logic and spawns the overwrite-confirmation OD on demand.
        let on_check_finish: crate::emDialog::DialogCheckFinishCb = Box::new(
            move |result: &DialogResult,
                  outer_dlg: &mut DlgPanel,
                  ctx: &mut EngineCtx<'_>|
                  -> bool {
                let fsb_id = outer_dlg
                    .fsb_panel_id_for_check_finish
                    .expect("emFileDialog fsb_panel_id_for_check_finish set");
                let mode = outer_dlg
                    .file_dialog_mode
                    .expect("emFileDialog file_dialog_mode set");
                let dir_allowed = outer_dlg.file_dialog_dir_allowed;
                let look_rc = outer_dlg.look.clone();
                run_file_dialog_check_finish(
                    ctx,
                    outer_dlg,
                    fsb_id,
                    mode,
                    dir_allowed,
                    look_rc,
                    result,
                )
                .is_ok()
            },
        );

        // Install extension + check-finish + mirror DlgPanel fields via
        // one pre-show tree reach.
        let root_panel_id = dialog.root_panel_id();
        {
            let pending = dialog.pending_mut();
            let tree = pending.window.tree_mut();
            if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
                if let Some(dlg_panel) = behavior.as_dlg_panel_mut() {
                    dlg_panel.on_cycle_ext = Some(on_cycle_ext);
                    dlg_panel.on_check_finish = Some(on_check_finish);
                    dlg_panel.file_dialog_mode = Some(mode);
                    dlg_panel.file_dialog_dir_allowed = false;
                    dlg_panel.fsb_panel_id_for_check_finish = Some(fsb_panel_id);
                }
                tree.put_behavior(root_panel_id, behavior);
            }
        }

        Self {
            dialog,
            look,
            fsb_panel_id,
            fsb_trigger_sig,
            mode,
            dir_allowed: false,
        }
    }

    /// `#[cfg(test)]`-intent accessor for E024 closure tests. No C++
    /// counterpart. Callers outside tests should not rely on this —
    /// fsb's signals are implementation detail.
    pub fn file_trigger_signal(&self) -> SignalId {
        self.fsb_trigger_sig
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
        // Phase 3.6.1 Task 2: mirror onto DlgPanel for the on_check_finish
        // closure's read path. Pre-show: tree reach; post-show: deferred
        // via pending_actions → mutate_dialog_by_id.
        if self.dialog.pending.is_some() {
            let root_panel_id = self.dialog.root_panel_id();
            let pending = self.dialog.pending_mut();
            let tree = pending.window.tree_mut();
            if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
                if let Some(p) = behavior.as_dlg_panel_mut() {
                    p.file_dialog_dir_allowed = allowed;
                }
                tree.put_behavior(root_panel_id, behavior);
            }
        }
        // NOTE: post-show branch omitted — requires `&mut App` / ctx which
        // this setter doesn't take. No in-tree caller flips dir_allowed
        // post-show today. Scope-expand if/when such a caller arrives.
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
    /// `DlgPanel.overwrite_dialog`.
    ///
    /// Phase 3.6.2 (E041) dedup: the core validation logic (dir-check,
    /// Open existence, Save classification, OD build) is shared with
    /// [`run_file_dialog_check_finish`] via the `check_finish_*` helpers
    /// below. This wrapper only handles the pre-show tree-reach shape
    /// (reads `overwrite_confirmed` and parks OD via
    /// `self.dialog.pending.window.tree_mut()`). Post-show calls go
    /// directly through `run_file_dialog_check_finish`.
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

        if let Err(early) = check_finish_dir_and_open(self.mode, self.dir_allowed, &names, &parent)
        {
            return early;
        }

        if self.mode != FileDialogMode::Save {
            return FileDialogCheckResult::Allow;
        }

        // Save-mode: read overwrite_confirmed via pre-show tree-take.
        let confirmed = if self.dialog.pending.is_some() {
            let root_panel_id = self.dialog.root_panel_id();
            let pending = self.dialog.pending_mut();
            let tree = pending.window.tree_mut();
            let mut s = String::new();
            if let Some(behavior) = tree.take_behavior(root_panel_id) {
                if let Some(p) = behavior.as_dlg_panel() {
                    s = p.overwrite_confirmed.clone();
                }
                tree.put_behavior(root_panel_id, behavior);
            }
            s
        } else {
            // Post-show sync read is unavailable here (no &mut App).
            // Conservative: treat as empty so OD respawns; the
            // OD-POSITIVE closure path will promote on confirm.
            String::new()
        };

        match classify_save_overwrite(&names, &parent, &confirmed) {
            SaveOverwriteOutcome::NoConflict | SaveOverwriteOutcome::AlreadyConfirmed => {
                FileDialogCheckResult::Allow
            }
            SaveOverwriteOutcome::NeedOverwriteDialog { paths, text } => {
                let mut od = build_overwrite_dialog(ctx, self.look.clone());
                // Pre-show: the outer's private engine doesn't exist yet,
                // so park OD.finish_signal on the outer's wake_up_signals.
                if self.dialog.pending.is_some() {
                    self.dialog.add_pre_show_wake_up_signal(od.finish_signal);
                }
                od.show(ctx);

                // Park OD on outer DlgPanel via the pre-show tree reach.
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

                FileDialogCheckResult::ConfirmOverwrite(paths)
            }
        }
    }
}

/// Shared with [`emFileDialog::CheckFinish`] (Phase 3.6.2 E041): dir-check
/// (C++ emFileDialog.cpp:119-146) and Open-mode existence check
/// (C++ :148-163). Returns `Ok(())` on pass, `Err(reason)` otherwise.
/// Does not touch Save-mode overwrite logic — that's handled by
/// [`classify_save_overwrite`].
fn check_finish_dir_and_open(
    mode: FileDialogMode,
    dir_allowed: bool,
    names: &[String],
    parent: &Path,
) -> Result<(), FileDialogCheckResult> {
    if !dir_allowed {
        if names.is_empty() {
            return Err(FileDialogCheckResult::Error("No file selected".to_string()));
        }
        for name in names {
            let path = parent.join(name);
            if path.is_dir() {
                if names.len() == 1 {
                    return Err(FileDialogCheckResult::EnterDirectory(name.clone()));
                }
                return Err(FileDialogCheckResult::Error(format!(
                    "Directory selected: {}",
                    name
                )));
            }
        }
    }
    if mode == FileDialogMode::Open {
        for name in names {
            let path = parent.join(name);
            if !path.exists() {
                return Err(FileDialogCheckResult::Error(format!(
                    "The following file cannot be opened, because it does not exist:\n\n{}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

/// Shared with [`emFileDialog::CheckFinish`] (Phase 3.6.2 E041): Save-mode
/// overwrite classification (C++ emFileDialog.cpp:165-185). Outcome tells
/// the caller whether to allow, allow-because-already-confirmed, or build
/// a new overwrite dialog and park it.
enum SaveOverwriteOutcome {
    /// No existing files collide with the selection.
    NoConflict,
    /// Collisions exist but the asked text equals `overwrite_confirmed`
    /// — the user already confirmed this exact prompt. Allow. Ports
    /// C++ emFileDialog.cpp:185 (`if (text==OverwriteConfirmed) return;`).
    AlreadyConfirmed,
    /// Collisions exist and text differs from `overwrite_confirmed` —
    /// caller must build the OD and park it. Ports C++ :186-197.
    NeedOverwriteDialog { paths: Vec<PathBuf>, text: String },
}

fn classify_save_overwrite(
    names: &[String],
    parent: &Path,
    overwrite_confirmed: &str,
) -> SaveOverwriteOutcome {
    let paths: Vec<PathBuf> = names
        .iter()
        .map(|n| parent.join(n))
        .filter(|p| p.exists())
        .collect();
    if paths.is_empty() {
        return SaveOverwriteOutcome::NoConflict;
    }
    let text = if paths.len() == 1 {
        format!(
            "Are you sure to overwrite the following already existing file?\n\n{}",
            paths[0].display()
        )
    } else {
        let mut msg =
            "Are you sure to overwrite the following already existing files?\n".to_string();
        for p in &paths {
            msg.push('\n');
            msg.push_str(&p.display().to_string());
        }
        msg
    };
    if text == overwrite_confirmed {
        SaveOverwriteOutcome::AlreadyConfirmed
    } else {
        SaveOverwriteOutcome::NeedOverwriteDialog { paths, text }
    }
}

/// Shared with [`emFileDialog::CheckFinish`] (Phase 3.6.2 E041): build
/// the "File Exists" `emDialog` with OK/Cancel buttons. Caller is
/// responsible for subscribing `od.finish_signal` (pre-show wake-up
/// rail vs post-show `scheduler.connect`) and for parking the dialog
/// on `DlgPanel.overwrite_dialog` (along with the asked text). Ports
/// C++ emFileDialog.cpp:186-191.
fn build_overwrite_dialog<C: ConstructCtx>(ctx: &mut C, look: Rc<emLook>) -> emDialog {
    let mut od = emDialog::new(ctx, "File Exists", look);
    od.AddCustomButton(ctx, "OK", DialogResult::Ok);
    od.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
    od
}

/// DIVERGED: (language-forced) shared validation body for the
/// widened `DialogCheckFinishCb` closure installed by `emFileDialog::new`.
/// Post-show-only. Ports C++ `emFileDialog::CheckFinish`
/// (emFileDialog.cpp:110-199): dir-check, Open existence, Save overwrite
/// detection + OD spawn.
///
/// C++'s `CheckFinish` is a virtual method that reaches into `this`
/// directly; Rust's `'static + FnMut` closure can't capture `&mut emFileDialog`,
/// so the read path (fsb_panel_id + mode + dir_allowed + look) is passed
/// as explicit arguments or reached through `&mut DlgPanel`.
///
/// Returns `Ok(())` to allow finalization; `Err(reason)` to veto. On a
/// Save-mode overwrite conflict the function spawns the "File Exists"
/// sub-dialog inline (parks it on `outer_dlg.overwrite_dialog`, subscribes
/// its finish_signal to the caller engine) before returning
/// `Err(ConfirmOverwrite(..))`.
///
/// Phase 3.6.2 (E041): core logic shared with [`emFileDialog::CheckFinish`]
/// via `check_finish_dir_and_open`, `classify_save_overwrite`, and
/// `build_overwrite_dialog`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_file_dialog_check_finish(
    ctx: &mut EngineCtx<'_>,
    outer_dlg: &mut DlgPanel,
    fsb_panel_id: PanelId,
    mode: FileDialogMode,
    dir_allowed: bool,
    look: Rc<emLook>,
    result: &DialogResult,
) -> Result<(), FileDialogCheckResult> {
    // Cancel always allowed (C++ emFileDialog.cpp:117).
    if *result == DialogResult::Cancel {
        return Ok(());
    }

    // Read fsb state via ctx.tree take/put.
    let (names, parent) = {
        let tree = ctx
            .tree
            .as_deref_mut()
            .expect("run_file_dialog_check_finish: tree present");
        let mut fsb_behavior = tree
            .take_behavior(fsb_panel_id)
            .expect("fsb panel behavior present");
        let (n, p) = {
            let fsb = fsb_behavior
                .as_file_selection_box_mut()
                .expect("fsb behavior is emFileSelectionBox");
            (
                fsb.GetSelectedNames().to_vec(),
                fsb.GetParentDirectory().to_path_buf(),
            )
        };
        tree.put_behavior(fsb_panel_id, fsb_behavior);
        (n, p)
    };

    check_finish_dir_and_open(mode, dir_allowed, &names, &parent)?;

    if mode != FileDialogMode::Save {
        return Ok(());
    }

    match classify_save_overwrite(&names, &parent, &outer_dlg.overwrite_confirmed) {
        SaveOverwriteOutcome::NoConflict => {
            // C++ does not explicitly clear OverwriteConfirmed here, but
            // Rust keeps the clear as a safety belt against stale state
            // from a prior save session (legacy write site).
            outer_dlg.overwrite_confirmed.clear();
            Ok(())
        }
        SaveOverwriteOutcome::AlreadyConfirmed => Ok(()),
        SaveOverwriteOutcome::NeedOverwriteDialog { paths, text } => {
            let mut od = build_overwrite_dialog(ctx, look);
            let od_finish_sig = od.finish_signal;
            // Subscribe outer engine to OD's finish_signal.
            let outer_engine_id = ctx.engine_id;
            ctx.scheduler.connect(od_finish_sig, outer_engine_id);
            od.show(ctx);
            outer_dlg.overwrite_dialog = Some(od);
            outer_dlg.overwrite_asked = text;
            Err(FileDialogCheckResult::ConfirmOverwrite(paths))
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

// ─── Phase 3.6 Task 5: E024 closure tests ──────────────────────────────────
//
// Mechanical arbiter that demonstrates E024 is STRUCTURALLY CLOSED: the
// scheduler drives emFileDialog to finish WITH ZERO CALLER INVOCATION OF
// ANY `Cycle` METHOD. Signals are fired into the scheduler, `DoTimeSlice`
// runs, assertions are made on pending-signals and finalized_result.
//
// Invariant (enforced by CI grep — see plan Task 5 Step 5.7):
//     rg -n '\.Cycle\(' crates/emcore/src/emFileDialog.rs == 0
#[cfg(test)]
mod e024_closure_tests {
    use super::*;
    use crate::emDialog::DialogResult;
    use crate::emGUIFramework::App;
    use crate::emInput::emInputEvent;
    use crate::emInputState::emInputState;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use winit::window::WindowId;

    /// Test-only fixture: owns an `App` + a tmp dir for file-system scratch.
    /// Drops the tmp dir on teardown.
    struct FileDialogTestHarness {
        app: App,
        tmp_dir: PathBuf,
    }

    static UID: AtomicU64 = AtomicU64::new(0);

    impl FileDialogTestHarness {
        fn new() -> Self {
            let uid = UID.fetch_add(1, Ordering::Relaxed);
            let tmp_dir = std::env::temp_dir().join(format!(
                "emcore_filedialog_test_{}_{}",
                std::process::id(),
                uid
            ));
            std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
            Self {
                app: App::new(Box::new(|_app, _el| {})),
                tmp_dir,
            }
        }

        fn write_test_file(&self, name: &str, content: &[u8]) -> PathBuf {
            let path = self.tmp_dir.join(name);
            std::fs::write(&path, content).expect("write test file");
            path
        }

        /// Run `n` scheduler slices. Does NOT call any `Cycle` method —
        /// `DoTimeSlice` dispatches engines internally. This is the
        /// E024-closure invariant: tests never pull `Cycle` manually.
        fn run_n_slices(&mut self, n: usize) {
            let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
            let mut input_state = emInputState::new();
            let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
            for _ in 0..n {
                self.app.scheduler.DoTimeSlice(
                    &mut self.app.windows,
                    &self.app.context,
                    &mut self.app.framework_actions,
                    &mut pending_inputs,
                    &mut input_state,
                    &fc,
                    &self.app.pending_actions,
                );
            }
        }
    }

    impl Drop for FileDialogTestHarness {
        fn drop(&mut self) {
            // Clean scheduler state so other tests don't see leaked pending.
            // EngineScheduler's Drop debug_asserts `engines.is_empty()` so
            // we must close every dialog (removes engines + windows) before
            // dropping the scheduler.
            self.app.pending_actions.borrow_mut().clear();
            let dids: Vec<crate::emGUIFramework::DialogId> =
                self.app.dialog_windows.keys().copied().collect();
            for did in dids {
                self.app.close_dialog_by_id(did);
            }
            self.app.scheduler.clear_pending_for_tests();
            let _ = std::fs::remove_dir_all(&self.tmp_dir);
        }
    }

    /// Build an emFileDialog on the harness's App and install headless.
    /// Returns `(fd, wid)`. After this helper, `fd.dialog.pending` is `None`
    /// (consumed by install).
    fn build_and_install(
        h: &mut FileDialogTestHarness,
        mode: FileDialogMode,
    ) -> (emFileDialog, WindowId) {
        let look = emLook::new();
        let mut fd = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            emFileDialog::new(&mut ctx, mode, look)
        };
        // Set a default directory so post-install CheckFinish has a valid path.
        fd.set_parent_directory(&h.tmp_dir);

        // Push pending into App and install headless. Mirrors the production
        // `show()` path without needing an ActiveEventLoop.
        let pending = fd
            .dialog
            .pending
            .take()
            .expect("fd.dialog.pending present pre-install");
        h.app.pending_top_level.push(pending);
        let wid = WindowId::dummy();
        h.app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");
        (fd, wid)
    }

    /// Walk into app.windows[wid].tree, take DlgPanel behavior, apply `f`,
    /// restore. Only legal post-install.
    fn with_dlg_panel<R>(
        app: &mut App,
        wid: WindowId,
        root_id: PanelId,
        f: impl FnOnce(&mut crate::emDialog::DlgPanel) -> R,
    ) -> R {
        let win = app
            .windows
            .get_mut(&wid)
            .expect("window present post-install");
        let mut tree = win.take_tree();
        let mut beh = tree.take_behavior(root_id).expect("root behavior present");
        let r = f(beh.as_dlg_panel_mut().expect("root is DlgPanel"));
        tree.put_behavior(root_id, beh);
        win.put_tree(tree);
        r
    }

    /// Read `finalized_result` from outer DlgPanel — tests' view of
    /// `GetResult`. (`emDialog` has no direct post-show `GetResult` helper;
    /// we read DlgPanel directly via the window's tree.)
    fn read_result(app: &mut App, wid: WindowId, root_id: PanelId) -> Option<DialogResult> {
        with_dlg_panel(app, wid, root_id, |p| p.finalized_result)
    }

    // ─── Test 1: fsb file_trigger_signal drives dialog to finish ───────────

    /// E024 closure core observation: firing fsb.file_trigger_signal into
    /// the scheduler drives the outer dialog's DialogPrivateEngine to
    /// finalize the result as Ok. Test does NOT call any Cycle method.
    ///
    /// Cycle-count rationale:
    ///   slice 1 — engine awakens via wake_up_signals subscription;
    ///             base body has no pending_result; on_cycle_ext runs AFTER
    ///             base (per emDialog.rs:962), sets pending_result = Ok;
    ///             returns true so engine stays awake.
    ///   slice 2 — engine runs base body: pending_result → finalized_result;
    ///             finish_state 1 → fires finish_signal.
    #[test]
    fn fsb_file_trigger_drives_dialog_to_finish_via_scheduler() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("hello.txt", b"hi");

        let look = emLook::new();
        let mut fd = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            emFileDialog::new(&mut ctx, FileDialogMode::Open, look)
        };
        fd.set_parent_directory(&h.tmp_dir);
        fd.set_selected_name("hello.txt");

        let finish_sig = fd.finish_signal();
        let fsb_trigger_sig = fd.file_trigger_signal();
        let outer_root = fd.dialog.root_panel_id;

        // Install headless.
        let pending = fd.dialog.pending.take().expect("pending present");
        h.app.pending_top_level.push(pending);
        let wid = WindowId::dummy();
        h.app
            .install_pending_top_level_headless(wid)
            .expect("install");

        // USER ACTION: fire fsb.file_trigger_signal. From here forward,
        // the test NEVER invokes any Cycle method. The scheduler dispatches
        // DialogPrivateEngine.Cycle via DoTimeSlice internally.
        h.app.scheduler.fire(fsb_trigger_sig);

        // Two slices: see Cycle-count rationale in doc comment above.
        h.run_n_slices(2);

        // Observable post-slice: finalized_result is set, finish_state
        // advanced past 1. `is_pending(finish_sig)` is NOT a reliable
        // post-DoTimeSlice check — the scheduler clears pending after
        // processing. finalized_result is the durable observable.
        let (result, state) = with_dlg_panel(&mut h.app, wid, outer_root, |p| {
            (p.finalized_result, p.finish_state)
        });
        assert_eq!(
            result,
            Some(DialogResult::Ok),
            "E024 closure: finalized_result must be Ok after fsb trigger drives the scheduler"
        );
        assert!(
            state >= 2,
            "finish_state must have advanced past 1 (==2, fire + reset); got {state}"
        );
        // finish_sig was subscribed by the scheduler; it has already fired
        // (finish_state >= 2 proves that). Bind explicitly to document intent.
        assert_ne!(
            finish_sig, fsb_trigger_sig,
            "finish and fsb signals are distinct"
        );
    }

    // ─── Test 2: on_cycle_ext-push verification (reduced-scope POSITIVE) ───
    //
    // Full scheduler-driven POSITIVE (livelock regression) is DEFERRED —
    // tracked as E040 in docs/superpowers/notes/2026-04-19-port-divergence-
    // raw-material.json. Blocked on a second headless WindowId: currently
    // `WindowId::dummy()` is the sole stable headless id; installing OD
    // as a second top-level overwrites outer's entry in `App::windows`.
    // Follow-up: parameterize `install_pending_top_level_headless` with a
    // caller-supplied id.

    /// Task 5 pragmatic POSITIVE test: scheduler-driven proof that when
    /// OD.finish_signal fires, outer's `on_cycle_ext` closure observes it
    /// AND pushes a deferred pending_action. This proves the subscription
    /// wiring (wake_up_signals → scheduler.connect) and the observation
    /// path of the Task 3 fix without requiring a second WindowId for a
    /// full installed OD.
    ///
    /// The POSITIVE promotion (overwrite_asked → overwrite_confirmed +
    /// pending_result = Ok) is tested at source-read level; the full
    /// scheduler-driven POSITIVE path is deferred to a follow-up task
    /// requiring a second headless WindowId in the test infrastructure.
    /// See the long comment in the previous test.
    #[test]
    fn save_mode_overwrite_od_finish_signal_schedules_pending_action() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("doc.txt", b"existing");

        let look = emLook::new();
        let mut fd = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            emFileDialog::new(&mut ctx, FileDialogMode::Save, look)
        };
        fd.set_parent_directory(&h.tmp_dir);
        fd.set_selected_name("doc.txt");

        // CheckFinish: parks OD, adds OD.finish_signal to outer's
        // wake_up_signals, queues `od.show(ctx)` pending_action.
        let check = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            fd.CheckFinish(&mut ctx, &DialogResult::Ok)
        };
        assert!(matches!(check, FileDialogCheckResult::ConfirmOverwrite(_)));

        // Discard the OD.show(ctx) closure: would crash on headless drain.
        h.app.pending_actions.borrow_mut().clear();

        // Capture OD's finish_signal from the parked OD, then install outer.
        let outer_root = fd.dialog.root_panel_id;
        let od_finish_sig = {
            let pending = fd.dialog.pending_mut();
            let tree = pending.window.tree_mut();
            let beh = tree.take_behavior(outer_root).expect("outer root");
            let sig = beh
                .as_dlg_panel()
                .and_then(|p| p.overwrite_dialog.as_ref())
                .map(|od| od.finish_signal)
                .expect("OD parked with finish_signal");
            tree.put_behavior(outer_root, beh);
            sig
        };

        let outer_pending = fd.dialog.pending.take().expect("outer pending");
        h.app.pending_top_level.push(outer_pending);
        let outer_wid = WindowId::dummy();
        h.app
            .install_pending_top_level_headless(outer_wid)
            .expect("outer install");

        // USER ACTION: fire OD.finish_signal. Scheduler wakes outer engine
        // (OD.finish_signal was connected via wake_up_signals by install).
        h.app.scheduler.fire(od_finish_sig);

        // One slice: base body does nothing (no buttons, no close, no
        // pending_result); on_cycle_ext runs post-base, observes od_finish_sig
        // pending, takes OD out of DlgPanel, pushes a pending_action.
        h.run_n_slices(1);

        // E024 closure observation: on_cycle_ext pushed the pending_action
        // that processes OD's result. This is the scheduler-driven evidence
        // that the Task 3 fix's observation path is correctly wired.
        assert_eq!(
            h.app.pending_actions.borrow().len(),
            1,
            "on_cycle_ext must push exactly one pending_action on OD.finish_signal"
        );

        // Also verify OD was taken out of DlgPanel (C++ emFileDialog.cpp:95
        // `delete OverwriteDialog.Get();` — OD handle is dropped).
        with_dlg_panel(&mut h.app, outer_wid, outer_root, |p| {
            assert!(
                p.overwrite_dialog.is_none(),
                "OD must be removed from DlgPanel after on_cycle_ext observation"
            );
            assert_eq!(
                p.overwrite_asked, "",
                "overwrite_asked must be cleared by mem::take in closure"
            );
        });
    }

    // ─── Test 3: overwrite-NEGATIVE path ──────────────────────────────────

    /// Save-mode overwrite NEGATIVE: user cancels OD → outer dialog stays
    /// open; OD is torn down; overwrite_asked cleared; outer.finish_signal
    /// NOT pending.
    ///
    /// Same WindowId constraint as test 2b: OD is not installed as a
    /// second top-level window. To drive the NEGATIVE branch, we fire
    /// OD.finish_signal and drain the resulting pending_action; the
    /// action calls `mutate_dialog_by_id(od_did, ...)` which is a no-op
    /// (OD never installed → od_did not in dialog_windows). The NEGATIVE
    /// branch's code path runs (since `od_result` is `None`), matching
    /// the code path for a user-cancelled OD at the behavioral level.
    #[test]
    fn save_mode_overwrite_negative_tears_down_od_outer_stays_open() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("doc.txt", b"existing");

        let look = emLook::new();
        let mut fd = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            emFileDialog::new(&mut ctx, FileDialogMode::Save, look)
        };
        fd.set_parent_directory(&h.tmp_dir);
        fd.set_selected_name("doc.txt");

        let _check = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            fd.CheckFinish(&mut ctx, &DialogResult::Ok)
        };
        h.app.pending_actions.borrow_mut().clear();

        let outer_root = fd.dialog.root_panel_id;
        let outer_finish_sig = fd.finish_signal();
        let od_finish_sig = {
            let pending = fd.dialog.pending_mut();
            let tree = pending.window.tree_mut();
            let beh = tree.take_behavior(outer_root).expect("outer root");
            let sig = beh
                .as_dlg_panel()
                .and_then(|p| p.overwrite_dialog.as_ref())
                .map(|od| od.finish_signal)
                .expect("OD parked");
            tree.put_behavior(outer_root, beh);
            sig
        };

        let outer_pending = fd.dialog.pending.take().expect("outer pending");
        h.app.pending_top_level.push(outer_pending);
        let outer_wid = WindowId::dummy();
        h.app
            .install_pending_top_level_headless(outer_wid)
            .expect("outer install");

        // Fire OD.finish_signal → on_cycle_ext pushes action → drain → NEGATIVE branch.
        h.app.scheduler.fire(od_finish_sig);
        h.run_n_slices(1);
        // Drain pending_actions to run the on_cycle_ext-pushed closure.
        h.app.drain_pending_actions_headless();

        // Assertions on outer state after NEGATIVE resolution:
        assert!(
            !h.app.scheduler.is_pending(outer_finish_sig),
            "NEGATIVE: outer finish_signal must NOT be pending"
        );
        assert_eq!(
            read_result(&mut h.app, outer_wid, outer_root),
            None,
            "NEGATIVE: outer finalized_result must be None (outer stays open)"
        );
        with_dlg_panel(&mut h.app, outer_wid, outer_root, |p| {
            assert!(p.overwrite_dialog.is_none(), "OD torn down after NEGATIVE");
            assert_eq!(
                p.overwrite_asked, "",
                "overwrite_asked cleared after NEGATIVE"
            );
            assert_eq!(
                p.overwrite_confirmed, "",
                "NEGATIVE: overwrite_confirmed NOT promoted"
            );
            assert_eq!(
                p.pending_result, None,
                "NEGATIVE: outer pending_result NOT set"
            );
        });
    }

    // ─── Test 3b: overwrite-POSITIVE path (E040, Phase 3.6.2) ─────────────

    /// Save-mode overwrite POSITIVE: user confirms OD → outer dialog
    /// promotes `overwrite_confirmed`, sets `pending_result = Some(Ok)`,
    /// and the outer engine commits it to `finalized_result` on the
    /// next slice.
    ///
    /// Historical context: this test was deferred in Phase 3.6 as E040
    /// because OD cannot be installed as a second headless top-level —
    /// `winit::window::WindowId::dummy()` is the only stable headless id
    /// and would collide with outer's entry in `App::windows`. Phase 3.6.2
    /// closes the hole with a narrow test-only sidecar:
    /// `App::headless_dialog_results` pre-seeds the OD's
    /// `DialogResult::Ok`; the pending_action closure reads it through
    /// `App::read_dialog_finalized_result` and drives the POSITIVE branch
    /// in the closure at emFileDialog.rs (see the `Some(DialogResult::Ok)`
    /// arm that maps to C++ emFileDialog.cpp:93-96).
    #[test]
    fn save_mode_overwrite_positive_finishes_outer_dialog_via_scheduler() {
        let mut h = FileDialogTestHarness::new();
        h.write_test_file("doc.txt", b"existing");

        let look = emLook::new();
        let mut fd = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            emFileDialog::new(&mut ctx, FileDialogMode::Save, look)
        };
        fd.set_parent_directory(&h.tmp_dir);
        fd.set_selected_name("doc.txt");

        // CheckFinish parks OD on outer DlgPanel, sets overwrite_asked,
        // and queues OD.show as a pending_action (which would crash
        // headless). Discard the pending_action after capturing OD state.
        let _check = {
            let mut ctx = crate::emEngineCtx::InitCtx {
                scheduler: &mut h.app.scheduler,
                framework_actions: &mut h.app.framework_actions,
                root_context: &h.app.context,
                pending_actions: &h.app.pending_actions,
            };
            fd.CheckFinish(&mut ctx, &DialogResult::Ok)
        };
        h.app.pending_actions.borrow_mut().clear();

        // Capture OD's dialog_id + finish_signal from the parked OD,
        // and record the expected overwrite_asked text.
        let outer_root = fd.dialog.root_panel_id;
        let outer_finish_sig = fd.finish_signal();
        let (od_did, od_finish_sig, asked_text) = {
            let pending = fd.dialog.pending_mut();
            let tree = pending.window.tree_mut();
            let beh = tree.take_behavior(outer_root).expect("outer root");
            let tuple = beh
                .as_dlg_panel()
                .and_then(|p| {
                    p.overwrite_dialog
                        .as_ref()
                        .map(|od| (od.dialog_id, od.finish_signal, p.overwrite_asked.clone()))
                })
                .expect("OD parked");
            tree.put_behavior(outer_root, beh);
            tuple
        };

        // Install outer headless.
        let outer_pending = fd.dialog.pending.take().expect("outer pending");
        h.app.pending_top_level.push(outer_pending);
        let outer_wid = WindowId::dummy();
        h.app
            .install_pending_top_level_headless(outer_wid)
            .expect("outer install");

        // Pre-seed OD's finalized_result via the E040 sidecar. The
        // pending_action drained below calls
        // `read_dialog_finalized_result(od_did)` which returns this
        // value directly, bypassing the WindowId::dummy() collision.
        h.app
            .headless_dialog_results
            .insert(od_did, DialogResult::Ok);

        // USER ACTION: fire OD.finish_signal. Scheduler wakes outer
        // engine via wake_up_signals subscription; on_cycle_ext observes
        // od_finish_sig pending, takes OD out of DlgPanel, pushes a
        // pending_action.
        h.app.scheduler.fire(od_finish_sig);
        h.run_n_slices(1);
        assert_eq!(
            h.app.pending_actions.borrow().len(),
            1,
            "on_cycle_ext must push exactly one pending_action on OD.finish_signal"
        );

        // Drain: the closure reads od_result via the sidecar, runs the
        // POSITIVE branch — `overwrite_confirmed = asked`,
        // `pending_result = Some(Ok)` — and calls close_dialog_by_id(od_did)
        // which is a no-op (OD was never installed).
        h.app.drain_pending_actions_headless();

        // Post-drain: outer DlgPanel must reflect POSITIVE commit.
        with_dlg_panel(&mut h.app, outer_wid, outer_root, |p| {
            assert!(
                p.overwrite_dialog.is_none(),
                "POSITIVE: OD handle dropped from DlgPanel"
            );
            assert_eq!(
                p.overwrite_asked, "",
                "POSITIVE: overwrite_asked cleared by mem::take"
            );
            assert_eq!(
                p.overwrite_confirmed, asked_text,
                "POSITIVE: overwrite_confirmed promoted from overwrite_asked"
            );
            assert_eq!(
                p.pending_result,
                Some(DialogResult::Ok),
                "POSITIVE: pending_result = Ok (commits on next base slice)"
            );
        });

        // One more slice: outer engine base body commits pending_result
        // → finalized_result, fires outer.finish_signal.
        h.run_n_slices(1);
        assert_eq!(
            read_result(&mut h.app, outer_wid, outer_root),
            Some(DialogResult::Ok),
            "POSITIVE: outer finalized_result = Ok after engine commit slice"
        );
        // Bind outer_finish_sig to document that it exists in the
        // scheduler; firing state has already been consumed by the
        // engine's internal processing.
        let _ = outer_finish_sig;
    }

    // ─── Test 4: no-signals one-slice is no-op ────────────────────────────

    /// Baseline sanity: constructing + installing a dialog, then running
    /// a single slice with no signals fired, must NOT finish the dialog.
    /// Counterpart to the legacy `cycle_no_signals_is_no_op`.
    #[test]
    fn no_signals_one_slice_is_no_op() {
        let mut h = FileDialogTestHarness::new();
        let (fd, wid) = build_and_install(&mut h, FileDialogMode::Open);
        let finish_sig = fd.finish_signal();
        let outer_root = fd.dialog.root_panel_id;
        let _ = fd;

        h.run_n_slices(1);

        assert!(
            !h.app.scheduler.is_pending(finish_sig),
            "no-signals slice must not fire finish_signal"
        );
        assert_eq!(
            read_result(&mut h.app, wid, outer_root),
            None,
            "no-signals slice must not finalize a result"
        );
    }
}
