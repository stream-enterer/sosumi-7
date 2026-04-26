use std::rc::Rc;

use crate::emButton::emButton;
use crate::emColor::emColor;
use crate::emCursor::emCursor;

/// Extension callback invoked by DialogPrivateEngine::Cycle after the base
/// cycle body. Populated by emFileDialog (Task 3) to inject file-dialog
/// Cycle logic — observation of fsb.file_trigger_signal +
/// overwrite_dialog.finish_signal — while keeping a single engine type
/// (D2 decision from the Phase 3.5 brainstorm).
pub(crate) type DialogCycleExt =
    Box<dyn FnMut(&mut DlgPanel, &mut crate::emEngineCtx::EngineCtx<'_>) -> bool>;
use crate::emEngineCtx::{ConstructCtx, PanelCtx};
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
use crate::emPanel::PanelBehavior;
use crate::emPanel::PanelState;
use crate::emPanel::Rect;
use crate::emSignal::SignalId;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emLook::emLook;

/// Result of a dialog interaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogResult {
    Ok,
    Cancel,
    Custom(u32),
}

type DialogFinishCb = crate::emEngineCtx::WidgetCallbackRef<DialogResult>;
/// DIVERGED: (language-forced) C++ `emDialog::CheckFinish` is a virtual method with no
/// extra args — subclasses reach into self's fields directly. Rust uses
/// a callback slot on `DlgPanel`; the closure needs `&mut DlgPanel` +
/// `&mut EngineCtx<'_>` to read tree state (e.g. emFileDialog's fsb
/// child panel) and spawn transient sub-dialogs. Matches `DialogCycleExt`
/// (Phase 3.6 Task 2).
pub(crate) type DialogCheckFinishCb =
    Box<dyn FnMut(&DialogResult, &mut DlgPanel, &mut crate::emEngineCtx::EngineCtx<'_>) -> bool>;

/// Modal dialog handle.
///
/// Port of C++ `emDialog` (emDialog.h). After Phase 3.5 Task 7, this struct
/// is a stable handle pointing to a pending-or-materialized top-level window.
/// Pre-show state lives in `pending`; post-show state lives in
/// `app.windows[app.dialog_windows[dialog_id]].tree`.
///
/// Spec §1 (phase-3-5-deferred-dialog-construction-design.md §1).
pub struct emDialog {
    /// Stable identity across pre-show / post-show transition.
    pub dialog_id: crate::emGUIFramework::DialogId,
    pub finish_signal: SignalId,
    pub close_signal: SignalId,
    /// PanelId of the DlgPanel root. Lives in `pending.window.tree` pre-show;
    /// in `app.windows[app.dialog_windows[dialog_id]].tree` post-show.
    pub root_panel_id: crate::emPanelTree::PanelId,
    /// Shared look held on the handle so pre-show mutators can clone without
    /// touching the pending DlgPanel. Keeps C++ `emDialog::GetLook()` semantics
    /// without re-exposing an accessor.
    pub(crate) look: Rc<emLook>,
    /// Pre-show builder state. `Some` before `show()`, `None` after.
    pub(crate) pending: Option<crate::emGUIFramework::PendingTopLevel>,
}

impl emDialog {
    /// Construction per spec §2 (phase-3-5-deferred-dialog-construction-design.md §2).
    ///
    /// Builds a DlgPanel-rooted PanelTree, wraps it in an
    /// `emWindow::new_top_level_pending`, and stashes the whole bundle as
    /// `self.pending`. Does NOT touch `&mut App` — uses only
    /// `ctx.allocate_dialog_id`, `ctx.create_signal`, and `ctx.root_context`.
    /// `DialogPrivateEngine` is built at install time (inside
    /// `install_pending_top_level`) once the winit `WindowId` is known.
    pub fn new<C: ConstructCtx>(ctx: &mut C, title: &str, look: Rc<emLook>) -> Self {
        let dialog_id = ctx.allocate_dialog_id();
        let finish_signal = ctx.create_signal();
        let close_signal = ctx.create_signal();
        let flags_signal = ctx.create_signal();
        let focus_signal = ctx.create_signal();
        let geom_signal = ctx.create_signal();

        // Build the DlgPanel-rooted PanelTree out-of-band.
        let mut tree = crate::emPanelTree::PanelTree::new();
        let root_panel_id = tree.create_root("dlg", false);
        let dlg_panel = DlgPanel::new(title, Rc::clone(&look), finish_signal);
        tree.set_behavior(root_panel_id, Box::new(dlg_panel));

        // Wrap in a pending top-level window.
        let mut window = crate::emWindow::emWindow::new_top_level_pending(
            Rc::clone(ctx.root_context()),
            crate::emWindow::WindowFlags::empty(),
            format!("emDialog-{}", dialog_id.0),
            close_signal,
            flags_signal,
            focus_signal,
            geom_signal,
            crate::emColor::emColor::TRANSPARENT,
        );
        // `new_top_level_pending` creates an empty default tree; discard it
        // and put our populated tree in its place (3.5.A headless-install
        // test precedent at emDialog.rs:1201-1202).
        let _ = window.take_tree();
        window.put_tree(tree);

        Self {
            dialog_id,
            finish_signal,
            close_signal,
            root_panel_id,
            look: Rc::clone(&look),
            pending: Some(crate::emGUIFramework::PendingTopLevel {
                dialog_id,
                window,
                close_signal,
                private_engine_root_panel_id: root_panel_id,
                wake_up_signals: Vec::new(),
            }),
        }
    }

    // ─── Pre-show mutator helper ────────────────────────────────────────────

    /// Reach into the pending DlgPanel and apply a closure. Panics if called
    /// after `show()`. Used by all pre-show mutators — spec §3
    /// (phase-3-5-deferred-dialog-construction-design.md §3).
    fn with_dlg_panel_mut<R>(
        &mut self,
        label: &'static str,
        f: impl FnOnce(&mut DlgPanel) -> R,
    ) -> R {
        let pending = self
            .pending
            .as_mut()
            .unwrap_or_else(|| panic!("{} after show", label));
        let tree = pending.window.tree_mut();
        let mut behavior = tree
            .take_behavior(self.root_panel_id)
            .expect("DlgPanel root must exist in pending tree");
        let r = f(behavior
            .as_dlg_panel_mut()
            .expect("root panel must be DlgPanel"));
        tree.put_behavior(self.root_panel_id, behavior);
        r
    }

    // ─── Pre-show mutators ───────────────────────────────────────────────────

    /// Port of C++ `emDialog::AddCustomButton` (emDialog.cpp:86-98).
    /// Allocates a DlgButton child panel under the DlgPanel root, records its
    /// click_signal + result pair on DlgPanel.button_signals so
    /// DialogPrivateEngine::Cycle observes clicks.
    ///
    /// Pre-show only. Panics if called after `show()`.
    pub fn AddCustomButton<C: ConstructCtx>(
        &mut self,
        ctx: &mut C,
        label: &str,
        result: DialogResult,
    ) {
        // Build the DlgButton (needs ctx for click_signal allocation via emButton::new).
        let look = Rc::clone(&self.look);
        let btn = DlgButton::new(ctx, label, look, result, self.root_panel_id);
        let click_signal = btn.button.click_signal;

        let pending = self.pending.as_mut().expect("AddCustomButton after show");
        let tree = pending.window.tree_mut();

        // Take DlgPanel behavior to record the (click_signal, result) pair.
        let mut behavior = tree
            .take_behavior(self.root_panel_id)
            .expect("DlgPanel root present in pending tree");
        let button_num = {
            let dlg = behavior.as_dlg_panel_mut().expect("root is DlgPanel");
            let n = dlg.button_signals.len();
            dlg.button_signals.push((click_signal, result));
            n
        };
        tree.put_behavior(self.root_panel_id, behavior);

        // Create the DlgButton child panel. C++ emDialog.cpp:63 names buttons
        // by ButtonNum: `emString::Format("%d", ButtonNum)`.
        let btn_id = tree.create_child(self.root_panel_id, &button_num.to_string(), None);
        tree.set_behavior(btn_id, Box::new(btn));
    }

    /// Port of C++ `emDialog::AddOKButton` — `AddPositiveButton("OK")`.
    pub fn AddOKButton<C: ConstructCtx>(&mut self, ctx: &mut C) {
        self.AddCustomButton(ctx, "OK", DialogResult::Ok);
    }

    /// Port of C++ `emDialog::AddCancelButton` — `AddNegativeButton("Cancel")`.
    pub fn AddCancelButton<C: ConstructCtx>(&mut self, ctx: &mut C) {
        self.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
    }

    /// Port of C++ `emDialog::AddOKCancelButtons`.
    pub fn AddOKCancelButtons<C: ConstructCtx>(&mut self, ctx: &mut C) {
        self.AddOKButton(ctx);
        self.AddCancelButton(ctx);
    }

    /// Port of C++ `emDialog::AddPositiveButton`. Generalization of `AddOKButton`.
    pub fn AddPositiveButton<C: ConstructCtx>(&mut self, ctx: &mut C, label: &str) {
        self.AddCustomButton(ctx, label, DialogResult::Ok);
    }

    /// Port of C++ `emDialog::AddNegativeButton`. Generalization of `AddCancelButton`.
    pub fn AddNegativeButton<C: ConstructCtx>(&mut self, ctx: &mut C, label: &str) {
        self.AddCustomButton(ctx, label, DialogResult::Cancel);
    }

    /// Update the dialog title.
    ///
    /// Port of C++ `emDialog::SetRootTitle` (emDialog.cpp:49-52).
    /// Pre-show: mutates DlgPanel directly via `with_dlg_panel_mut`.
    /// Post-show: routes through `pending_actions` → `App::mutate_dialog_by_id`.
    pub fn SetRootTitle<C: ConstructCtx>(&mut self, ctx: &mut C, title: &str) {
        if self.pending.is_some() {
            self.with_dlg_panel_mut("SetRootTitle", |p| p.SetTitle(title));
        } else {
            let did = self.dialog_id;
            let title = title.to_string();
            ctx.pending_actions()
                .borrow_mut()
                .push(Box::new(move |app, _el| {
                    app.mutate_dialog_by_id(did, |p, _tree| p.SetTitle(&title));
                }));
        }
    }

    /// Update label of the first button whose result matches `result`.
    ///
    /// Port of C++ `emDialog::SetButtonLabel` (emDialog.cpp:55-62). Walks the
    /// DlgButton children of the DlgPanel root to find the first button whose
    /// result payload matches, then updates its caption.
    /// Pre-show: walks DlgButton children directly in the pending tree.
    /// Post-show: routes through `pending_actions` → `App::mutate_dialog_by_id`,
    /// walking DlgButton children inline via the `&mut PanelTree` argument.
    /// Root panel is taken out of the tree during the closure — children remain
    /// accessible through the passed-in tree ref. `root_panel_id` is captured
    /// from `self` so the closure can call `tree.children(root_panel_id)`.
    pub fn set_button_label_for_result<C: ConstructCtx>(
        &mut self,
        ctx: &mut C,
        result: &DialogResult,
        label: &str,
    ) {
        if let Some(pending) = self.pending.as_mut() {
            let tree = pending.window.tree_mut();
            Self::walk_and_set_button_label(tree, self.root_panel_id, result, label);
        } else {
            let did = self.dialog_id;
            let root_panel_id = self.root_panel_id;
            let result = *result;
            let label = label.to_string();
            ctx.pending_actions()
                .borrow_mut()
                .push(Box::new(move |app, _el| {
                    app.mutate_dialog_by_id(did, move |_p, tree| {
                        // Root panel is detached from the tree during this closure;
                        // children remain in the tree and are walkable via `tree`.
                        Self::walk_and_set_button_label(tree, root_panel_id, &result, &label);
                    });
                }));
        }
    }

    /// Walk children of `root_id` in `tree` and set the caption of the first
    /// `DlgButton` whose result matches `result`. Shared by both pre-show and
    /// post-show paths of `set_button_label_for_result`.
    fn walk_and_set_button_label(
        tree: &mut crate::emPanelTree::PanelTree,
        root_id: crate::emPanelTree::PanelId,
        result: &DialogResult,
        label: &str,
    ) {
        // Collect child ids to avoid holding a reference into `tree` while
        // mutably borrowing per-child behaviors.
        let children: Vec<crate::emPanelTree::PanelId> = tree.children(root_id).collect();
        for cid in children {
            let mut behavior = match tree.take_behavior(cid) {
                Some(b) => b,
                None => continue,
            };
            let matched = if let Some(btn) = behavior.as_dlg_button_mut() {
                if *btn.result() == *result {
                    btn.SetCaption(label);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            tree.put_behavior(cid, behavior);
            if matched {
                break;
            }
        }
    }

    /// Port of C++ `emDialog::EnableAutoDeletion` (emDialog.cpp:156-159).
    /// Pre-show: mutates DlgPanel directly via `with_dlg_panel_mut`.
    /// Post-show: routes through `pending_actions` → `App::mutate_dialog_by_id`.
    pub fn EnableAutoDeletion<C: ConstructCtx>(&mut self, ctx: &mut C, enabled: bool) {
        if self.pending.is_some() {
            self.with_dlg_panel_mut("EnableAutoDeletion", |p| p.auto_delete = enabled);
        } else {
            let did = self.dialog_id;
            ctx.pending_actions()
                .borrow_mut()
                .push(Box::new(move |app, _el| {
                    app.mutate_dialog_by_id(did, |p, _tree| p.auto_delete = enabled);
                }));
        }
    }

    /// Set the finish callback — invoked once when the dialog result is
    /// finalized, from `DialogPrivateEngine::Cycle`.
    ///
    /// Pre-show only. Panics if called after `show()`.
    pub fn set_on_finish(&mut self, cb: DialogFinishCb) {
        self.with_dlg_panel_mut("set_on_finish", |p| p.on_finish = Some(cb));
    }

    /// Set the check-finish veto — invoked from `DialogPrivateEngine::Cycle`
    /// before finalizing the result; returning `false` vetoes finalization.
    ///
    /// Pre-show only. Panics if called after `show()`.
    pub fn set_on_check_finish(&mut self, cb: DialogCheckFinishCb) {
        self.with_dlg_panel_mut("set_on_check_finish", |p| p.on_check_finish = Some(cb));
    }

    // ─── Show ────────────────────────────────────────────────────────────────

    /// Enqueue this pending dialog for installation on the next
    /// `about_to_wait` tick. Consumes `self.pending`; the handle remains
    /// valid — `dialog_id` / `finish_signal` / `close_signal` / `root_panel_id`
    /// are stable across the show transition.
    ///
    /// Port: C++ `emDialog` construction implicitly shows the dialog because
    /// construction creates the X window via `emWindow` base-class ctor.
    /// Rust splits the two-phase pattern (3.5.A `new_top_level_pending` +
    /// `install_pending_top_level`); `show()` is the explicit second phase.
    pub fn show<C: ConstructCtx>(&mut self, ctx: &mut C) {
        let pending = self.pending.take().expect("show called twice");
        let queue = ctx.pending_actions().clone();
        queue.borrow_mut().push(Box::new(move |app, el| {
            app.pending_top_level.push(pending);
            app.install_pending_top_level(el);
        }));
    }

    /// Static convenience that builds an OK-only message dialog with
    /// auto-delete. Port of C++ `emDialog::ShowMessage` (emDialog.cpp:162-180).
    ///
    /// Phase 3.5: shimmed as `unimplemented!()` because no live caller exists
    /// in-tree. Phase 3.6 wires a real path: `new + AddOKButton +
    /// EnableAutoDeletion(true) + content label + show`.
    pub fn ShowMessage<C: ConstructCtx>(_ctx: &mut C, _title: &str, _message: &str) -> Self {
        unimplemented!("emDialog::ShowMessage — Phase 3.6 impl; no live caller in 3.5")
    }

    /// Stable PanelId of the DlgPanel root. Always valid (pre-show and
    /// post-show) — captured at construction in `emDialog::new`.
    pub fn root_panel_id(&self) -> crate::emPanelTree::PanelId {
        self.root_panel_id
    }

    /// Mutable access to the pre-show builder state. Panics post-show
    /// (after `show()` has consumed `self.pending`). Used by subclass-like
    /// consumers (e.g. `emFileDialog`) to reach into the pending tree and
    /// install child panels before the dialog materializes.
    pub(crate) fn pending_mut(&mut self) -> &mut crate::emGUIFramework::PendingTopLevel {
        self.pending
            .as_mut()
            .expect("pending_mut after show: emDialog pre-show state consumed")
    }

    /// Subscribe an arbitrary signal to the to-be-built `DialogPrivateEngine`
    /// at install time. Pre-show only; panics if called after `show()`.
    ///
    /// Port of C++ `emFileDialog` ctor calling
    /// `AddWakeUpSignal(Fsb->GetFileTriggerSignal())` (emFileDialog.cpp:41).
    /// In C++ the private engine exists in the ctor and takes subscriptions
    /// directly; in Rust the engine is built deferred, so subscribers stash
    /// signals on `PendingTopLevel.wake_up_signals` for the installer to
    /// drain post-register_engine.
    pub fn add_pre_show_wake_up_signal(&mut self, sig: SignalId) {
        self.pending
            .as_mut()
            .expect("add_pre_show_wake_up_signal after show")
            .wake_up_signals
            .push(sig);
    }

    /// Lazily create (or return) the content panel of this dialog.
    ///
    /// Port of C++ `emDialog::GetContentPanel` (emDialog.cpp:67-73). In C++
    /// the content panel is an `emLinearGroup` lazy-created on first call;
    /// in Rust we create a plain child panel named "content" under the
    /// DlgPanel root and record it in `DlgPanel.content_panel_id` so
    /// subsequent calls return the same id.
    ///
    /// DIVERGED: (language-forced) Rust's lazy-create is simpler (plain child) because the
    /// layout contract in `DlgPanel::LayoutChildren` already rects-out
    /// `content_panel_id` against the computed content area — the C++
    /// emLinearGroup wrapper is an idiom adaptation since the wrapper
    /// layer is not yet ported. Consumers install their own child behavior
    /// via the returned PanelId (see `emFileDialog::new`).
    ///
    /// Pre-show only. Panics post-show: content panel must be installed
    /// before `show()` because the pre-show tree-reach pattern is the only
    /// mutation path for the dialog's PanelTree before materialization.
    pub fn GetContentPanel<C: ConstructCtx>(
        &mut self,
        _ctx: &mut C,
    ) -> crate::emPanelTree::PanelId {
        let root_panel_id = self.root_panel_id;
        let pending = self.pending.as_mut().expect("GetContentPanel after show");
        let tree = pending.window.tree_mut();

        // Check existing id cached on DlgPanel.
        if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
            let existing = behavior.as_dlg_panel_mut().and_then(|p| p.content_panel_id);
            tree.put_behavior(root_panel_id, behavior);
            if let Some(id) = existing {
                return id;
            }
        }

        // Lazy-create a "content" child under the DlgPanel root.
        let content_id = tree.create_child(root_panel_id, "content", None);
        if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
            if let Some(dlg) = behavior.as_dlg_panel_mut() {
                dlg.content_panel_id = Some(content_id);
            }
            tree.put_behavior(root_panel_id, behavior);
        }
        content_id
    }

    /// Programmatically finish the dialog post-show with the given result.
    ///
    /// Port of C++ `emDialog::Finish(int)` (emDialog.cpp:146-153) for the
    /// post-show case.
    ///
    /// Thin wrapper over `App::mutate_dialog_by_id` (Prereq A). Pushes a
    /// closure that sets `DlgPanel.pending_result = Some(result)` via the
    /// general mutation rail; `mutate_dialog_by_id` handles the tree walk and
    /// engine wake-up. `DialogPrivateEngine::Cycle` picks up `pending_result`
    /// on the next tick and routes through the normal finalize →
    /// fire(finish_signal) → invoke on_finish sequence. Matches C++ `Finish`
    /// behavior: `CheckFinish` still runs via `DialogPrivateEngine`'s normal
    /// flow.
    ///
    /// Guard: only sets `pending_result` if neither `pending_result` nor
    /// `finalized_result` is `Some` (first-fire semantics, matching C++ Finish).
    ///
    /// Preserved for the 2 callers in `emFileDialog::Cycle` (Phase 3.6 Task 4
    /// deletes `Cycle` and both call sites). When Task 4 lands this method
    /// becomes vestigial; re-evaluate for deletion then.
    pub fn finish_post_show<C: ConstructCtx>(&self, ctx: &mut C, result: DialogResult) {
        let did = self.dialog_id;
        ctx.pending_actions()
            .borrow_mut()
            .push(Box::new(move |app, _el| {
                app.mutate_dialog_by_id(did, move |p, _tree| {
                    if p.pending_result.is_none() && p.finalized_result.is_none() {
                        p.pending_result = Some(result);
                    }
                });
            }));
    }
}

/// Root-panel PanelBehavior for an `emDialog`.
///
/// Port of C++ `emDialog::DlgPanel : public emBorder` (emDialog.h:186-204).
/// Lives as the root panel of the dialog's owned `emWindow`. Holds the
/// dialog's mutable state (result, buttons, finish-state, auto-delete
/// countdown) because `DialogPrivateEngine::Cycle` reaches state through
/// `tree.take_behavior(root_panel_id)` — the Rust analog of the C++
/// `PrivateEngineClass::Dlg&` back-reference (B3.5e).
///
pub struct DlgPanel {
    pub(crate) border: emBorder,
    pub(crate) look: Rc<emLook>,
    /// Set by `DlgPanel::on_finish` once `CheckFinish` permits. `DialogPrivateEngine`
    /// observes this on Cycle and fires `finish_signal`.
    pub(crate) pending_result: Option<DialogResult>,
    /// Stored after the finish signal has fired.
    pub(crate) finalized_result: Option<DialogResult>,
    /// Mirrors C++ `emDialog::FinishState` (emDialog.cpp:146-223). 0 = no
    /// finish pending; 1 = Finish has been called and accepted (the next
    /// `DialogPrivateEngine::Cycle` fires `finish_signal` and invokes
    /// `on_finish`/`on_finished`, advancing to 2); 2..3 = auto-delete
    /// countdown; at 3 the engine emits `DeferredAction::CloseWindow`
    /// (C++ `delete this`). If `auto_delete` is false, state returns to
    /// 0 after firing (C++ `!ADEnabled` branch).
    pub(crate) finish_state: u8,
    pub(crate) auto_delete: bool,
    pub(crate) finish_signal: SignalId,
    pub(crate) on_finish: Option<DialogFinishCb>,
    pub(crate) on_check_finish: Option<DialogCheckFinishCb>,
    /// Port of C++ `virtual void emDialog::Finished(int result)` (D1 — callback,
    /// not trait method). Fires from `DialogPrivateEngine::Cycle` after
    /// finish_signal fires. Default `None` matches C++ default (no-op).
    pub(crate) on_finished: Option<DialogFinishCb>,
    /// PanelId of the emLinearLayout content panel, set by Task 7.
    pub(crate) content_panel_id: Option<crate::emPanelTree::PanelId>,
    /// PanelId of the emLinearLayout button-row panel, set by Task 7.
    pub(crate) buttons_panel_id: Option<crate::emPanelTree::PanelId>,
    /// Parallel `(click_signal, result)` pairs for the dialog's buttons.
    /// Populated by Task 7 when `DlgButton` children are materialized;
    /// empty for Task 4. `DialogPrivateEngine::Cycle` iterates these to
    /// observe button clicks, mirroring C++ `emDialog::PrivateEngineClass`
    /// observing button signals via `AddWakeUpSignal` (emDialog.cpp:38).
    pub(crate) button_signals: Vec<(SignalId, DialogResult)>,
    /// DIVERGED: (language-forced) Rust mechanism for the C++ "emFileDialog::Cycle calls
    /// emDialog::Cycle() first then runs its own logic" inheritance pattern
    /// (emFileDialog.cpp:82). In C++, `emFileDialog` is a subclass and its
    /// `Cycle()` override calls `emDialog::Cycle()` then continues. In Rust
    /// there is a single engine type (`DialogPrivateEngine`) with no
    /// inheritance; this callback slot lets emFileDialog inject post-base
    /// Cycle logic without needing a separate engine or vtable dispatch.
    ///
    /// `DialogPrivateEngine::Cycle` calls this AFTER the base cycle body
    /// (close-signal observation, pending_result resolution, auto-delete
    /// countdown). Return value: whether the extension wants to keep the
    /// engine awake (OR'd with the base Cycle's return).
    ///
    /// NOTE: the extension must not call `ctx.tree.take_behavior(root_panel_id)`
    /// — that behavior is already taken by the engine's Cycle body. The
    /// extension CAN take other panels (e.g. overwrite dialog's root).
    ///
    /// Populated by emFileDialog::new in Task 3. `None` for plain emDialog.
    pub(crate) on_cycle_ext: Option<DialogCycleExt>,
    /// DIVERGED (Phase 3.6 Task 3): file-dialog overwrite-confirmation
    /// sub-dialog. Set by `emFileDialog::CheckFinish` when Save-mode detects
    /// overwrite conflicts; consumed + torn down by the file-dialog
    /// on_cycle_ext closure at next Cycle. Port of C++
    /// `emCrossPtr<emDialog> emFileDialog::OverwriteDialog`
    /// (emFileDialog.h:204); the `emCrossPtr` auto-null semantics are
    /// captured by the Option's None state combined with on_cycle_ext's
    /// take-deregister-drop pattern on finish observation.
    ///
    /// Placement rationale: this field lives on DlgPanel (not on
    /// emFileDialog) so the `'static + FnMut` `on_cycle_ext` closure can
    /// reach it through its `&mut DlgPanel` argument — avoiding
    /// `Rc<RefCell<Option<emDialog>>>` which would be a Do-NOT violation
    /// per CLAUDE.md.
    pub(crate) overwrite_dialog: Option<emDialog>,
    /// DIVERGED (Phase 3.6 Task 3): `EngineId` of this dialog's
    /// `DialogPrivateEngine`, populated at install time (after register).
    /// Lets `emFileDialog::CheckFinish` subscribe the overwrite dialog's
    /// finish signal to this engine without a scheduler-scope walk. Port
    /// of the C++ member-pointer that `emFileDialog::CheckFinish` uses via
    /// inheritance to reach its own private engine
    /// (emFileDialog.cpp:168 `AddWakeUpSignal(OverwriteDialog->GetFinishSignal())`).
    pub(crate) private_engine_id: Option<crate::emEngine::EngineId>,
    /// DIVERGED (Phase 3.6 Task 3): text being confirmed for overwrite.
    /// Matches C++ `OverwriteAsked` in emFileDialog.h:202. Placement on
    /// DlgPanel mirrors `overwrite_dialog` for the same closure-reach
    /// reason.
    pub(crate) overwrite_asked: String,
    /// DIVERGED (Phase 3.6 Task 3 fix): last-confirmed overwrite text.
    /// C++ `emFileDialog::OverwriteConfirmed` (emFileDialog.h:203).
    /// Lives on `DlgPanel` (not on `emFileDialog`) so the `on_cycle_ext`
    /// closure — which has only `&mut DlgPanel` + `&mut EngineCtx` — can
    /// promote `overwrite_asked → overwrite_confirmed` on OD POSITIVE
    /// without reaching back into `emFileDialog`. Placement rationale
    /// matches `overwrite_dialog` / `overwrite_asked`.
    pub(crate) overwrite_confirmed: String,
    /// DIVERGED (Phase 3.6.1 Task 2): file-dialog mode mirrored from
    /// `emFileDialog::mode` onto DlgPanel so the `'static + FnMut`
    /// `on_check_finish` closure — which has only `&mut DlgPanel` +
    /// `&mut EngineCtx`, not `&mut emFileDialog` — reads fresh state per
    /// fire. `None` for plain emDialogs (no file-dialog validation).
    /// Rust-only consolidation: C++ subclass `emFileDialog` stores `Mode`
    /// directly on itself and reaches it via `this` inside virtual
    /// `CheckFinish`; Rust callback slot can't reach across struct
    /// boundaries, so the read path lives on DlgPanel. Writes are mirrored
    /// from `emFileDialog` (authoritative outward API) via
    /// `with_dlg_panel_mut` pre-show / `App::mutate_dialog_by_id` post-show.
    pub(crate) file_dialog_mode: Option<crate::emFileDialog::FileDialogMode>,
    /// DIVERGED (Phase 3.6.1 Task 2): mirrors `emFileDialog::dir_allowed`
    /// onto DlgPanel for the `on_check_finish` closure's read path. Same
    /// rationale as `file_dialog_mode` above. `false` default matches
    /// `emFileDialog::new` default.
    pub(crate) file_dialog_dir_allowed: bool,
    /// DIVERGED (Phase 3.6.1 Task 2): mirrors `emFileDialog::fsb_panel_id`
    /// onto DlgPanel so the `on_check_finish` closure can reach the
    /// emFileSelectionBox child via `take_behavior(fsb_panel_id)` through
    /// its `&mut EngineCtx` tree. `None` for plain emDialogs.
    pub(crate) fsb_panel_id_for_check_finish: Option<crate::emPanelTree::PanelId>,
}

impl DlgPanel {
    pub(crate) fn new(title: &str, look: Rc<emLook>, finish_signal: SignalId) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::PopupRoot).with_caption(title),
            look,
            pending_result: None,
            finalized_result: None,
            finish_state: 0,
            auto_delete: false,
            finish_signal,
            on_finish: None,
            on_check_finish: None,
            on_finished: None,
            content_panel_id: None,
            buttons_panel_id: None,
            button_signals: Vec::new(),
            on_cycle_ext: None,
            overwrite_dialog: None,
            private_engine_id: None,
            overwrite_asked: String::new(),
            overwrite_confirmed: String::new(),
            file_dialog_mode: None,
            file_dialog_dir_allowed: false,
            fsb_panel_id_for_check_finish: None,
        }
    }

    pub(crate) fn SetTitle(&mut self, title: &str) {
        self.border.SetCaption(title);
    }
}

impl PanelBehavior for DlgPanel {
    fn as_dlg_panel_mut(&mut self) -> Option<&mut DlgPanel> {
        Some(self)
    }

    fn as_dlg_panel(&self) -> Option<&DlgPanel> {
        Some(self)
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        let pixel_scale = 1.0; // DlgPanel is the view root; no enclosing scaling
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Port of C++ DlgPanel::LayoutChildren (emDialog.cpp:302-322).
        // Same operation order as C++:
        //   GetContentRectUnobscured(&x,&y,&w,&h,&cc);
        //   bh = emMin(w*0.08, h*0.3);
        //   sp = bh * 0.25;
        //   x += sp; y += sp; w -= 2*sp; h -= 2*sp;
        //   ContentPanel->Layout(x, y, w, h-sp-bh, cc);
        //   ButtonsPanel->Layout(x, y+h-bh, w, bh, cc);
        let Rect { w: pw, h: ph, .. } = ctx.layout_rect();
        let Rect {
            mut x,
            mut y,
            mut w,
            mut h,
        } = self.border.GetContentRectUnobscured(pw, ph, &self.look);
        let bh = f64::min(w * 0.08, h * 0.3);
        let sp = bh * 0.25;
        x += sp;
        y += sp;
        w -= 2.0 * sp;
        h -= 2.0 * sp;
        if let Some(content_id) = self.content_panel_id {
            ctx.layout_child(content_id, x, y, w, h - sp - bh);
        }
        if let Some(buttons_id) = self.buttons_panel_id {
            ctx.layout_child(buttons_id, x, y + h - bh, w, bh);
        }

        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn GetCanvasColor(&self) -> crate::emColor::emColor {
        // PopupRoot border paints opaque background; canvas = content.
        self.border
            .content_canvas_color(crate::emColor::emColor::TRANSPARENT, &self.look, true)
    }

    fn IsOpaque(&self) -> bool {
        true // PopupRoot covers the whole dialog viewport
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // Port of C++ emDialog::DlgPanel::Input (emDialog.cpp:277-299).
        // DIVERGED: (language-forced) emBorder has no Input in Rust; C++ emBorder::Input called
        // here handles focus traversal. Track as latent gap — revisit if
        // emBorder gains Input.
        if event.variant != InputVariant::Press {
            return false;
        }
        // C++ uses state.IsNoMod() (emInput.h:293): treats Shift as a blocking
        // modifier too. Inline-expanded since Rust emInputState has no IsNoMod.
        if event.shift || event.ctrl || event.alt || event.meta {
            return false;
        }
        match event.key {
            InputKey::Enter => {
                // Set pending result; DialogPrivateEngine observes on next Cycle,
                // runs on_check_finish, fires finish_signal. Not a direct Finish
                // call: Finish semantics live in the private engine per C++.
                self.pending_result = Some(DialogResult::Ok);
                true
            }
            InputKey::Escape => {
                self.pending_result = Some(DialogResult::Cancel);
                true
            }
            _ => false,
        }
    }
}

/// PanelBehavior wrapping `emButton` for a dialog button.
///
/// Port of C++ `class DlgButton : public emButton` (emDialog.h:169-183).
/// Carries a `DialogResult` payload (C++: `int Result`) and a reference
/// to the owning `DlgPanel` (via `PanelId`). In C++, `Clicked()` calls
/// `((emDialog*)GetWindow())->Finish(Result)` — a direct back-edge through
/// the window pointer. In Rust, click observation is engine-side:
/// `DialogPrivateEngine` (Task 4+7) connects the button's `click_signal`
/// to its own wake-up set (`scheduler.connect(click_signal, private_engine_id)`),
/// matching C++ `emDialog::PrivateEngineClass` observing button signals via
/// `AddWakeUpSignal`. Therefore the Rust `Input` impl here is a pure
/// delegator to `emButton::Input`; it does not write `pending_result`
/// itself — the private engine does on observing the click signal.
///
/// Precedent: `ButtonPanel` adapter in `emColorFieldFieldPanel.rs:187-210`.
///
/// `DlgButton` struct is `pub` (not `pub(crate)`) because `PanelBehavior`
/// is a `pub` trait and its `as_dlg_button_mut` method names this type in
/// its return type — the same `private_interfaces` forced divergence as
/// `DlgPanel`. All methods and fields are `pub(crate)`.
pub struct DlgButton {
    pub(crate) button: emButton,
    /// Dialog result payload carried by this button. C++ parity: `int Result`
    /// in `class DlgButton` (emDialog.h:182).
    pub(crate) result: DialogResult,
    /// PanelId of the owning `DlgPanel`. The engine-side click observer
    /// (Task 4+7) uses this to reach the `DlgPanel` and write
    /// `pending_result`. Rust analog of the C++ back-edge
    /// `((emDialog*)GetWindow())->Finish(Result)` (emDialog.cpp:236).
    /// Prefixed `_` because Phase 3.5 Tasks 8–14 do not yet wire the
    /// engine-side connect; retained as design documentation for Task 15+.
    pub(crate) _dlg_panel_id: crate::emPanelTree::PanelId,
}

impl DlgButton {
    pub(crate) fn new<C: ConstructCtx>(
        ctx: &mut C,
        caption: &str,
        look: Rc<emLook>,
        result: DialogResult,
        dlg_panel_id: crate::emPanelTree::PanelId,
    ) -> Self {
        Self {
            button: emButton::new(ctx, caption, look),
            result,
            _dlg_panel_id: dlg_panel_id,
        }
    }

    /// Port of C++ `DlgButton::GetResult` (emDialog.h:249-252).
    pub(crate) fn result(&self) -> &DialogResult {
        &self.result
    }

    pub(crate) fn SetCaption(&mut self, text: &str) {
        self.button.SetCaption(text);
    }
}

impl PanelBehavior for DlgButton {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    // DIVERGED: (language-forced) DlgButton click observation — C++ emDialog.cpp:236 `DlgButton::Clicked()` walks
    // parent chain via `((emDialog*)GetWindow())->Finish(Result)`. Rust wires this engine-side
    // via `scheduler.connect(button.click_signal, private_engine_id)` at install time (Task 7),
    // so `Input` here is a pure delegator.
    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        // Pure delegator. Click observation is engine-side via
        // `scheduler.connect(button.click_signal, dialog_private_engine_id)`
        // (Task 4+7), matching C++ `PrivateEngineClass::AddWakeUpSignal`.
        self.button.Input(event, state, input_state, ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.button.GetCursor()
    }

    fn as_dlg_button_mut(&mut self) -> Option<&mut crate::emDialog::DlgButton> {
        Some(self)
    }
}

/// Port of C++ `emDialog::PrivateEngineClass` (emDialog.h:203-210,
/// emDialog.cpp:194-224). Installed at `Priority::High` and wired to
/// `close_signal` (C++: `AddWakeUpSignal(GetCloseSignal())` in the
/// `emDialog` ctor, emDialog.cpp:38). `Cycle` ports `PrivateCycle`
/// (emDialog.cpp:194-224) beat-for-beat:
///   1. Close signal observed ⇒ `pending_result = Cancel` (C++ Finish(NEGATIVE)).
///   2. Iterate button click signals ⇒ `pending_result = button.result`
///      (C++ `DlgButton::Clicked` calls `GetWindow()->Finish(Result)`; in
///      Rust the engine observes the signal — see `DlgButton` doc comment).
///   3. If `pending_result` set and not yet finalized, run `on_check_finish`
///      veto → finalize, fire `finish_signal`, invoke `on_finish`/`on_finished`
///      (C++ `Finish` + `FinishState==1` branch).
///   4. Auto-delete countdown: 3 slices after finalize, emit
///      `DeferredAction::CloseWindow` (C++ `delete this` at FinishState==4).
///
pub(crate) struct DialogPrivateEngine {
    /// Phase 3.5 Task 10: `DialogId` stored directly so the auto-delete
    /// branch can call `App::close_dialog_by_id(did)` via the closure rail
    /// without a reverse-lookup.
    pub(crate) dialog_id: crate::emGUIFramework::DialogId,
    pub(crate) root_panel_id: crate::emPanelTree::PanelId,
    pub(crate) close_signal: SignalId,
}

impl crate::emEngine::emEngine for DialogPrivateEngine {
    fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
        // Port of emDialog::PrivateCycle (emDialog.cpp:194-224).
        //
        // Step 0: detach DlgPanel behavior — Rust analog of the C++
        // `PrivateEngineClass::Dlg&` back-reference. After `take_behavior`,
        // `tree`'s borrow is returned and we may freely call `as_sched_ctx`
        // on `ctx` to invoke widget callbacks. No `unsafe` needed.
        //
        // Phase 3.5.A Task 10: DialogPrivateEngine is registered at
        // `PanelScope::Toplevel(dialog_window_id)` via
        // `App::install_pending_top_level` (production) or
        // `install_pending_top_level_headless` (tests), so `ctx.tree` is
        // always `Some` during Cycle.
        let Some(mut behavior) = ctx
            .tree
            .as_deref_mut()
            .expect("DialogPrivateEngine: tree is Some (Toplevel scope)")
            .take_behavior(self.root_panel_id)
        else {
            // Panel gone — nothing to do.
            return false;
        };

        let stay_awake = {
            let Some(dlg) = behavior.as_dlg_panel_mut() else {
                // Non-DlgPanel at root_panel_id: wiring bug. Put it back and
                // go to sleep — defensive no-op.
                ctx.tree
                    .as_deref_mut()
                    .expect("DialogPrivateEngine: tree is Some")
                    .put_behavior(self.root_panel_id, behavior);
                return false;
            };

            // Step 1: close_signal → Cancel (emDialog.cpp:196-198
            // `if (IsSignaled(GetCloseSignal())) Finish(NEGATIVE);`).
            // Guard on `pending_result.is_none() && finalized_result.is_none()`
            // matches `Finish`'s "no-op once result is set" semantics
            // (emDialog.cpp: once Result is Finalized, subsequent Finish calls
            // through PrivateCycle short-circuit via FinishState>0 branches).
            if ctx.IsSignaled(self.close_signal)
                && dlg.pending_result.is_none()
                && dlg.finalized_result.is_none()
            {
                dlg.pending_result = Some(DialogResult::Cancel);
            }

            // Step 2: button click signals (Task 7 populates button_signals).
            // Iterated by value to avoid aliasing `dlg.button_signals` with
            // `dlg.pending_result` writes.
            let button_fires: Vec<DialogResult> = dlg
                .button_signals
                .iter()
                .filter_map(|(sig, result)| {
                    if ctx.IsSignaled(*sig) {
                        Some(*result)
                    } else {
                        None
                    }
                })
                .collect();
            for result in button_fires {
                if dlg.pending_result.is_none() && dlg.finalized_result.is_none() {
                    dlg.pending_result = Some(result);
                }
            }

            // Step 3: pending_result set → check_finish veto → finalize
            // (sets finish_state=1). Ports the body of C++ emDialog::Finish
            // (emDialog.cpp:146-153): if CheckFinish accepts, Result=r and
            // FinishState=1. The signal fire + Finished invocation live in
            // the FinishState==1 branch below, matching C++ structure.
            if let Some(pending) = dlg.pending_result.take() {
                let vetoed = if let Some(mut cb) = dlg.on_check_finish.take() {
                    let vetoed = !cb(&pending, dlg, ctx);
                    dlg.on_check_finish = Some(cb);
                    vetoed
                } else {
                    false
                };
                if !vetoed && dlg.finish_state == 0 {
                    dlg.finalized_result = Some(pending);
                    dlg.finish_state = 1;
                }
            }

            // Step 4: state-machine dispatch. Ports emDialog.cpp:200-223
            // (PrivateCycle if/else chain) one-to-one.
            //
            // C++:
            //   if (FinishState<=0) return false;
            //   else if (FinishState==1) { FinishState=2; Signal(FinishSignal); Finished(Result); return true; }
            //   else if (!ADEnabled) { FinishState=0; return false; }
            //   else if (FinishState<3) { FinishState++; return true; }
            //   else { delete this; return false; }
            //
            // DIVERGED: (language-forced) `delete this` becomes a deferred
            // `DeferredAction::CloseWindow`, because emWindow lifetime is
            // owned by emGUIFramework rather than self-destructed.
            let state = dlg.finish_state;
            if state == 0 {
                false
            } else if state == 1 {
                // Advance first, then fire + invoke callbacks. Matches C++
                // ordering: FinishState=2 is observable to any code the
                // Signal/Finished call chain reaches (emDialog.cpp:204-206).
                dlg.finish_state = 2;
                let finish_signal = dlg.finish_signal;
                let result = dlg
                    .finalized_result
                    .expect("finish_state==1 implies finalized_result is set");
                // Take callbacks to avoid aliasing with ctx.as_sched_ctx();
                // leave None afterwards — C++ invokes `Finished(Result)`
                // exactly once per dialog (virtual dispatch, no re-arm).
                let mut on_finish = dlg.on_finish.take();
                let mut on_finished = dlg.on_finished.take();
                let mut sched = ctx.as_sched_ctx();
                sched.fire(finish_signal);
                if let Some(cb) = on_finish.as_mut() {
                    cb(&result, &mut sched);
                }
                if let Some(cb) = on_finished.as_mut() {
                    cb(&result, &mut sched);
                }
                true
            } else if !dlg.auto_delete {
                dlg.finish_state = 0;
                false
            } else if dlg.finish_state < 3 {
                dlg.finish_state += 1;
                true
            } else {
                // state == 3 (or greater): `delete this` in C++.
                // Phase 3.5 Task 10: closure-rail replaces the undrained
                // enum-rail `DeferredAction::CloseWindow` push. The previous
                // emission was never consumed (CloseWindow was on the
                // framework_actions enum rail which the event loop did not
                // drain for dialogs), so auto-delete was latently broken.
                // Push a `pending_actions` closure that calls
                // `App::close_dialog_by_id(did)` instead.
                let did = self.dialog_id;
                ctx.pending_actions()
                    .borrow_mut()
                    .push(Box::new(move |app, _el| app.close_dialog_by_id(did)));
                false
            }
        };

        // Step 5a: call on_cycle_ext (if set) BEFORE putting behavior back.
        // The extension runs after the base cycle body so any pending_result
        // it sets will be visible to the base on the NEXT Cycle, matching C++
        // emFileDialog::Cycle() calling emDialog::Cycle() first (emFileDialog.cpp:82).
        //
        // Swap-out pattern: take the closure from dlg_panel, call it with
        // &mut dlg_panel, put it back. This avoids a double-borrow of dlg_panel
        // (we can't hold `dlg_panel.on_cycle_ext.as_mut()` while also passing
        // `&mut dlg_panel` to the same closure).
        let ext_stay = if let Some(dlg_panel) = behavior.as_dlg_panel_mut() {
            if let Some(mut ext) = dlg_panel.on_cycle_ext.take() {
                let stay = ext(dlg_panel, ctx);
                dlg_panel.on_cycle_ext = Some(ext);
                stay
            } else {
                false
            }
        } else {
            false
        };

        // Step 5b: put DlgPanel behavior back.
        let tree = ctx
            .tree
            .as_deref_mut()
            .expect("DialogPrivateEngine: tree is Some");
        if tree.panels.contains_key(self.root_panel_id) {
            tree.put_behavior(self.root_panel_id, behavior);
        }
        stay_awake || ext_stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngineCtx::{DeferredAction, InitCtx};
    use crate::emPanel::Rect;
    use crate::emPanelTree::{PanelId, PanelTree};
    use crate::emScheduler::EngineScheduler;
    use slotmap::Key as _;
    use std::cell::RefCell;

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

    fn test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let id = tree.create_root("t", false);
        (tree, id)
    }

    // ─── New API tests (Task 7+) ─────────────────────────────────────────────

    #[test]
    fn new_builds_pending_with_populated_tree_and_identity() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let dlg = emDialog::new(&mut init.ctx(), "Test Dialog", look);

        // Identity fields are stable across show().
        let _did: u64 = dlg.dialog_id.0; // just proves the field exists and is u64
        let _: SignalId = dlg.finish_signal;
        let _: SignalId = dlg.close_signal;
        let _: crate::emPanelTree::PanelId = dlg.root_panel_id;

        // Pre-show: pending is Some.
        assert!(dlg.pending.is_some());
        let pending = dlg.pending.as_ref().unwrap();
        assert_eq!(pending.dialog_id, dlg.dialog_id);
        assert_eq!(pending.close_signal, dlg.close_signal);
        assert_eq!(pending.private_engine_root_panel_id, dlg.root_panel_id);
    }

    // ─── DlgPanel tests — not affected by legacy API removal ─────────────────

    #[test]
    fn dlg_panel_enter_sets_pending_ok() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::press(InputKey::Enter);
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(consumed, "Enter should be consumed");
        assert_eq!(panel.pending_result, Some(DialogResult::Ok));
        // Read remaining fields so dead-code doesn't fire before Task 5 wires
        // DlgPanel into emDialog. All of these are observed by
        // DialogPrivateEngine::Cycle (Task 4) per plan §B3.5e.
        assert!(panel.finalized_result.is_none());
        assert_eq!(panel.finish_state, 0);
        assert!(!panel.auto_delete);
        let _ = panel.finish_signal;
        assert!(panel.on_finish.is_none());
        assert!(panel.on_check_finish.is_none());
        assert!(panel.on_finished.is_none());
        assert!(panel.content_panel_id.is_none());
        assert!(panel.buttons_panel_id.is_none());
        assert!(panel.button_signals.is_empty());
        panel.SetTitle("New");
    }

    #[test]
    fn dlg_panel_escape_sets_pending_cancel() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::press(InputKey::Escape);
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(consumed, "Escape should be consumed");
        assert_eq!(panel.pending_result, Some(DialogResult::Cancel));
    }

    #[test]
    fn dlg_panel_modified_enter_is_ignored() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.ctrl = true;
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(!consumed, "Ctrl-Enter should not be consumed");
        assert_eq!(panel.pending_result, None);
    }

    #[test]
    fn dlg_panel_shift_enter_is_ignored() {
        // C++ parity: state.IsNoMod() rejects Shift (emInput.h:293).
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut init = TestInit::new();
        let finish_sig = init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.shift = true;
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(!consumed, "Shift-Enter should not be consumed");
        assert_eq!(panel.pending_result, None);
    }

    #[test]
    fn dlg_button_carries_result_payload() {
        let mut __init = TestInit::new();
        let (tree, tid) = test_tree();
        let btn = DlgButton::new(
            &mut __init.ctx(),
            "OK",
            emLook::new(),
            DialogResult::Ok,
            tid,
        );
        assert_eq!(btn.button.GetCaption(), "OK");
        assert_eq!(btn.result(), &DialogResult::Ok);
        assert_eq!(btn._dlg_panel_id, tid);
        // click_signal is allocated by emButton::new; engine-side connect
        // happens in Task 4+7. Prove the signal exists (non-null).
        let _ = btn.button.click_signal;
        let _ = &tree;
    }

    #[test]
    fn dlg_button_set_caption_updates_emButton() {
        let mut __init = TestInit::new();
        let (tree, tid) = test_tree();
        let mut btn = DlgButton::new(
            &mut __init.ctx(),
            "OK",
            emLook::new(),
            DialogResult::Custom(9),
            tid,
        );
        assert_eq!(btn.button.GetCaption(), "OK");
        btn.SetCaption("Accept");
        assert_eq!(btn.button.GetCaption(), "Accept");
        let _ = &tree;
    }

    #[test]
    fn private_engine_observes_close_signal_sets_pending_cancel() {
        // Ports the C++ PrivateCycle close-signal branch (emDialog.cpp:196-198):
        //   if (IsSignaled(GetCloseSignal())) Finish(NEGATIVE);
        // Expectation after one DoTimeSlice: finalized_result == Cancel,
        // finish_state == 2 (C++ FinishState==1 branch advances to 2 after
        // firing FinishSignal, emDialog.cpp:203-206), and a probe engine
        // connected to finish_signal has been awoken exactly once.
        //
        // Phase 3.5.A Task 10: registration flows through
        // `App::install_pending_top_level_headless`, which mirrors the
        // production `install_pending_top_level` path (deferred engine
        // register at `PanelScope::Toplevel(wid)` post-materialize).
        use crate::emGUIFramework::{App, PendingTopLevel};
        use crate::emPanelScope::PanelScope;
        use crate::emWindow::WindowFlags;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        // Build the dialog's populated PanelTree out-of-band, then wrap it
        // in a pending top-level emWindow (whose default empty tree we
        // discard). Matches the production shape where `emDialog::new`
        // builds the tree before enqueueing the `PendingTopLevel`.
        let mut tree = PanelTree::new();
        let root_id = tree.create_root("dlg", false);
        let finish_sig = app.scheduler.create_signal();
        let close_sig = app.scheduler.create_signal();
        let flags_sig = app.scheduler.create_signal();
        let focus_sig = app.scheduler.create_signal();
        let geom_sig = app.scheduler.create_signal();
        let dlg_panel = DlgPanel::new("Test", emLook::new(), finish_sig);
        tree.set_behavior(root_id, Box::new(dlg_panel));

        let mut window = crate::emWindow::emWindow::new_top_level_pending(
            Rc::clone(&app.context),
            WindowFlags::empty(),
            "test-dialog".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        let _discarded_internal = window.take_tree();
        window.put_tree(tree);

        // Probe engine: counts its own Cycle invocations. Connected to
        // `finish_sig`, it will be woken in the slice where the signal
        // fires — a direct observation of `Signal(FinishSignal)`.
        struct FinishProbe {
            hits: Rc<RefCell<u32>>,
        }
        impl crate::emEngine::emEngine for FinishProbe {
            fn Cycle(&mut self, _ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
                *self.hits.borrow_mut() += 1;
                false
            }
        }
        let hits: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let probe_id = app.scheduler.register_engine(
            Box::new(FinishProbe {
                hits: Rc::clone(&hits),
            }),
            crate::emEngine::Priority::Medium,
            PanelScope::Framework,
        );
        app.scheduler.connect(finish_sig, probe_id);

        // Enqueue the pending top-level entry and drive the headless install
        // path. Phase 3.5 Task 5: DialogPrivateEngine is constructed at
        // install time by `install_pending_top_level_headless`; the
        // PendingTopLevel carries only the construction inputs.
        let wid = WindowId::dummy();
        let dialog_id = app.allocate_dialog_id();
        app.pending_top_level.push(PendingTopLevel {
            dialog_id,
            window,
            close_signal: close_sig,
            private_engine_root_panel_id: root_id,
            wake_up_signals: Vec::new(),
        });
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");
        assert!(
            app.windows.contains_key(&wid),
            "install_pending_top_level_headless must move emWindow into App::windows",
        );
        assert_eq!(
            app.dialog_windows.get(&dialog_id).copied(),
            Some(wid),
            "DialogId → WindowId mapping must be recorded",
        );

        // Fire close signal and run one slice against the per-window tree.
        app.scheduler.fire(close_sig);
        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &app.pending_actions,
        );

        // Direct probe: finish_signal fired exactly once during the slice.
        assert_eq!(
            *hits.borrow(),
            1,
            "finish_signal must fire exactly once when close_signal is observed",
        );

        // Inspect DlgPanel state via the window's tree after the cycle.
        {
            let win = app.windows.get_mut(&wid).expect("window present");
            let mut tree = win.take_tree();
            let mut behavior = tree.take_behavior(root_id).expect("behavior reinstated");
            {
                let dlg = behavior.as_dlg_panel_mut().expect("is DlgPanel");
                assert_eq!(
                    dlg.finalized_result,
                    Some(DialogResult::Cancel),
                    "close_signal should finalize to Cancel"
                );
                assert_eq!(
                    dlg.finish_state, 2,
                    "FinishState==1 branch advances to 2 after firing FinishSignal",
                );
                assert!(
                    dlg.pending_result.is_none(),
                    "pending_result consumed by finalize"
                );
            }
            tree.put_behavior(root_id, behavior);
            win.put_tree(tree);
        }

        // Without auto_delete, the next Cycle hits the C++ `!ADEnabled`
        // branch: FinishState=0, return false. finish_signal must NOT
        // fire again. Re-fire close_signal too — the engine is already
        // finalized and must ignore it.
        app.scheduler.fire(close_sig);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &app.pending_actions,
        );
        assert_eq!(
            *hits.borrow(),
            1,
            "finish_signal must not re-fire on subsequent slices",
        );
        {
            let win = app.windows.get_mut(&wid).expect("window still present");
            let mut tree = win.take_tree();
            let mut behavior = tree.take_behavior(root_id).expect("still present");
            {
                let dlg = behavior.as_dlg_panel_mut().unwrap();
                assert_eq!(
                    dlg.finalized_result,
                    Some(DialogResult::Cancel),
                    "repeated close_signal must not re-finalize"
                );
                assert_eq!(
                    dlg.finish_state, 0,
                    "!ADEnabled branch resets FinishState to 0",
                );
            }
            tree.put_behavior(root_id, behavior);
            win.put_tree(tree);
        }

        // Teardown. InputDispatchEngine is removed by App::drop.
        app.scheduler.remove_engine(engine_id);
        app.scheduler.remove_engine(probe_id);
        app.scheduler.clear_pending_for_tests();
    }

    // ─── Task 8 tests ────────────────────────────────────────────────────────

    #[test]
    fn add_custom_button_appends_to_button_signals() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        dlg.AddCustomButton(&mut init.ctx(), "OK", DialogResult::Ok);
        dlg.AddCustomButton(&mut init.ctx(), "Cancel", DialogResult::Cancel);

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(dlg.root_panel_id).unwrap();
        let results: Vec<DialogResult> = behavior
            .as_dlg_panel_mut()
            .unwrap()
            .button_signals
            .iter()
            .map(|(_, r)| *r)
            .collect();
        tree.put_behavior(dlg.root_panel_id, behavior);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], DialogResult::Ok);
        assert_eq!(results[1], DialogResult::Cancel);
    }

    #[test]
    fn add_custom_button_creates_dlg_button_children() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        dlg.AddCustomButton(&mut init.ctx(), "OK", DialogResult::Ok);
        dlg.AddCustomButton(&mut init.ctx(), "Cancel", DialogResult::Cancel);

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let children: Vec<_> = tree.children(dlg.root_panel_id).collect();
        assert_eq!(children.len(), 2, "two DlgButton children created");

        // Verify first child is named "0" and has Ok result.
        let mut b0 = tree.take_behavior(children[0]).unwrap();
        let btn0_result = *b0.as_dlg_button_mut().unwrap().result();
        tree.put_behavior(children[0], b0);
        assert_eq!(btn0_result, DialogResult::Ok);

        let mut b1 = tree.take_behavior(children[1]).unwrap();
        let btn1_result = *b1.as_dlg_button_mut().unwrap().result();
        tree.put_behavior(children[1], b1);
        assert_eq!(btn1_result, DialogResult::Cancel);
    }

    #[test]
    #[should_panic(expected = "AddCustomButton after show")]
    fn add_custom_button_after_show_panics() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        // Simulate show() by taking pending.
        let _ = dlg.pending.take();
        dlg.AddCustomButton(&mut init.ctx(), "OK", DialogResult::Ok);
    }

    #[test]
    fn set_root_title_updates_dlg_panel_border_caption() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Old", look);
        dlg.SetRootTitle(&mut init.ctx(), "New");

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(dlg.root_panel_id).unwrap();
        let caption = behavior.as_dlg_panel_mut().unwrap().border.caption.clone();
        tree.put_behavior(dlg.root_panel_id, behavior);
        assert_eq!(caption, "New");
    }

    /// Post-show SetRootTitle routes through pending_actions → mutate_dialog_by_id.
    /// Prereq B replaces the former #[should_panic] test.
    #[test]
    fn set_root_title_post_show_routes_via_pending_actions() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        let (dlg_id, root_id) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Old", look);
            let dlg_id = dlg.dialog_id;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.take().unwrap());
            (dlg_id, root_id)
        };

        let wid = WindowId::dummy();
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        // Build a post-show handle (pending = None) and call SetRootTitle.
        let dummy_sig = app.scheduler.create_signal();
        let mut handle = emDialog {
            dialog_id: dlg_id,
            finish_signal: dummy_sig,
            close_signal: dummy_sig,
            root_panel_id: root_id,
            look: emLook::new(),
            pending: None,
        };
        {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            handle.SetRootTitle(&mut ctx, "Changed");
        }

        // Drain pending_actions so mutate_dialog_by_id fires.
        app.drain_pending_actions_headless();

        // Read back the title from the materialized DlgPanel.
        let win = app.windows.get_mut(&wid).expect("window present");
        let mut tree = win.take_tree();
        let mut b = tree.take_behavior(root_id).expect("DlgPanel present");
        let caption = b.as_dlg_panel_mut().unwrap().border.caption.clone();
        tree.put_behavior(root_id, b);
        win.put_tree(tree);
        assert_eq!(
            caption, "Changed",
            "SetRootTitle post-show must land on DlgPanel"
        );

        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    #[test]
    fn set_button_label_for_result_updates_dlg_button_caption() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        dlg.AddCustomButton(&mut init.ctx(), "OK", DialogResult::Ok);
        dlg.set_button_label_for_result(&mut init.ctx(), &DialogResult::Ok, "Accept");

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let children: Vec<_> = tree.children(dlg.root_panel_id).collect();
        let mut b = tree.take_behavior(children[0]).unwrap();
        let caption = b
            .as_dlg_button_mut()
            .unwrap()
            .button
            .GetCaption()
            .to_string();
        tree.put_behavior(children[0], b);
        assert_eq!(caption, "Accept");
    }

    /// Post-show set_button_label_for_result routes through pending_actions →
    /// mutate_dialog_by_id, walking DlgButton children inline via the tree arg.
    /// Prereq B rework: queue + step 0.5 removed; label lands after draining
    /// pending_actions (no DoTimeSlice needed).
    #[test]
    fn set_button_label_for_result_post_show_routes_via_pending_actions() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        let (dlg_id, root_id) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Test", look);
            dlg.AddCustomButton(&mut ctx, "OK", DialogResult::Ok);
            let dlg_id = dlg.dialog_id;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.take().unwrap());
            (dlg_id, root_id)
        };

        let wid = WindowId::dummy();
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        // Post-show handle.
        let dummy_sig = app.scheduler.create_signal();
        let mut handle = emDialog {
            dialog_id: dlg_id,
            finish_signal: dummy_sig,
            close_signal: dummy_sig,
            root_panel_id: root_id,
            look: emLook::new(),
            pending: None,
        };
        {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            handle.set_button_label_for_result(&mut ctx, &DialogResult::Ok, "Accept");
        }

        // Drain pending_actions: mutate_dialog_by_id fires and walks DlgButton
        // children inline — label lands immediately, no DoTimeSlice required.
        app.drain_pending_actions_headless();

        // Read back the button caption.
        let win = app.windows.get_mut(&wid).expect("window present");
        let mut tree = win.take_tree();
        let children: Vec<_> = tree.children(root_id).collect();
        let mut b = tree.take_behavior(children[0]).expect("DlgButton present");
        let caption = b
            .as_dlg_button_mut()
            .unwrap()
            .button
            .GetCaption()
            .to_string();
        tree.put_behavior(children[0], b);
        win.put_tree(tree);
        assert_eq!(
            caption, "Accept",
            "set_button_label_for_result post-show must land after pending_actions drain"
        );

        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    #[test]
    fn enable_auto_deletion_sets_flag() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        dlg.EnableAutoDeletion(&mut init.ctx(), true);

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(dlg.root_panel_id).unwrap();
        let flag = behavior.as_dlg_panel_mut().unwrap().auto_delete;
        tree.put_behavior(dlg.root_panel_id, behavior);
        assert!(flag);
    }

    /// Post-show EnableAutoDeletion routes through pending_actions → mutate_dialog_by_id.
    /// Prereq B replaces the former #[should_panic] test.
    #[test]
    fn enable_auto_deletion_post_show_routes_via_pending_actions() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        let (dlg_id, root_id) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Test", look);
            let dlg_id = dlg.dialog_id;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.take().unwrap());
            (dlg_id, root_id)
        };

        let wid = WindowId::dummy();
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        // Confirm auto_delete starts false.
        {
            let win = app.windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut b = tree.take_behavior(root_id).unwrap();
            let flag = b.as_dlg_panel_mut().unwrap().auto_delete;
            tree.put_behavior(root_id, b);
            win.put_tree(tree);
            assert!(!flag, "auto_delete must start false");
        }

        // Post-show handle.
        let dummy_sig = app.scheduler.create_signal();
        let mut handle = emDialog {
            dialog_id: dlg_id,
            finish_signal: dummy_sig,
            close_signal: dummy_sig,
            root_panel_id: root_id,
            look: emLook::new(),
            pending: None,
        };
        {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            handle.EnableAutoDeletion(&mut ctx, true);
        }

        // Drain pending_actions so mutate_dialog_by_id fires.
        app.drain_pending_actions_headless();

        // Read back auto_delete.
        let win = app.windows.get_mut(&wid).expect("window present");
        let mut tree = win.take_tree();
        let mut b = tree.take_behavior(root_id).expect("DlgPanel present");
        let flag = b.as_dlg_panel_mut().unwrap().auto_delete;
        tree.put_behavior(root_id, b);
        win.put_tree(tree);
        assert!(flag, "EnableAutoDeletion post-show must land on DlgPanel");

        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    #[test]
    fn set_on_finish_stores_callback() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        let called = Rc::new(RefCell::new(false));
        let called_clone = Rc::clone(&called);
        dlg.set_on_finish(Box::new(move |_r, _s| {
            *called_clone.borrow_mut() = true;
        }));

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(dlg.root_panel_id).unwrap();
        let has_cb = behavior.as_dlg_panel_mut().unwrap().on_finish.is_some();
        tree.put_behavior(dlg.root_panel_id, behavior);
        assert!(has_cb);
        // Silence unused-variable warning from `called`.
        let _ = called;
    }

    #[test]
    #[should_panic(expected = "set_on_finish after show")]
    fn set_on_finish_after_show_panics() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        let _ = dlg.pending.take();
        dlg.set_on_finish(Box::new(|_r, _s| {}));
    }

    #[test]
    fn set_on_check_finish_stores_callback() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        dlg.set_on_check_finish(Box::new(|_r, _dlg, _ctx| true));

        let pending = dlg.pending.as_mut().unwrap();
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(dlg.root_panel_id).unwrap();
        let has_cb = behavior
            .as_dlg_panel_mut()
            .unwrap()
            .on_check_finish
            .is_some();
        tree.put_behavior(dlg.root_panel_id, behavior);
        assert!(has_cb);
    }

    #[test]
    #[should_panic(expected = "set_on_check_finish after show")]
    fn set_on_check_finish_after_show_panics() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        let _ = dlg.pending.take();
        dlg.set_on_check_finish(Box::new(|_r, _dlg, _ctx| true));
    }

    // ─── Task 9 tests ────────────────────────────────────────────────────────

    #[test]
    fn show_drains_pending_into_closure_rail() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        assert!(dlg.pending.is_some());
        dlg.show(&mut init.ctx());
        assert!(dlg.pending.is_none());
        // Closure was pushed onto pending_actions.
        assert_eq!(init.pa.borrow().len(), 1);
        // Identity fields still valid.
        let _: crate::emGUIFramework::DialogId = dlg.dialog_id;
        let _: SignalId = dlg.finish_signal;
    }

    #[test]
    #[should_panic(expected = "show called twice")]
    fn show_twice_panics() {
        let mut init = TestInit::new();
        let look = emLook::new();
        let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
        dlg.show(&mut init.ctx());
        dlg.show(&mut init.ctx()); // panics
    }

    // ─── Phase 3.5 Task 10 test ──────────────────────────────────────────────

    /// Verifies that `DialogPrivateEngine::Cycle`'s auto-delete countdown
    /// (FinishState 1→2→3→emit) pushes a `close_dialog_by_id` closure onto
    /// `pending_actions` rather than emitting the old undrained
    /// `DeferredAction::CloseWindow` enum-rail action.
    ///
    /// Setup mirrors `private_engine_observes_close_signal_sets_pending_cancel`
    /// but enables `auto_delete` before install and runs 4 slices (one to
    /// finalize, three to count down).
    #[test]
    fn private_engine_with_auto_delete_emits_close_closure() {
        use crate::emGUIFramework::{App, PendingTopLevel};
        use crate::emWindow::WindowFlags;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        // Build dialog tree with auto_delete=true.
        let mut tree = PanelTree::new();
        let root_id = tree.create_root("dlg", false);
        let finish_sig = app.scheduler.create_signal();
        let close_sig = app.scheduler.create_signal();
        let flags_sig = app.scheduler.create_signal();
        let focus_sig = app.scheduler.create_signal();
        let geom_sig = app.scheduler.create_signal();
        let mut dlg_panel = DlgPanel::new("Test", emLook::new(), finish_sig);
        // Enable auto-delete before install.
        dlg_panel.auto_delete = true;
        tree.set_behavior(root_id, Box::new(dlg_panel));

        let mut window = crate::emWindow::emWindow::new_top_level_pending(
            Rc::clone(&app.context),
            WindowFlags::empty(),
            "test-auto-delete".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        let _discarded = window.take_tree();
        window.put_tree(tree);

        let wid = WindowId::dummy();
        let dialog_id = app.allocate_dialog_id();
        app.pending_top_level.push(PendingTopLevel {
            dialog_id,
            window,
            close_signal: close_sig,
            private_engine_root_panel_id: root_id,
            wake_up_signals: Vec::new(),
        });
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);

        // Helper closure to run one scheduler slice.
        let do_slice = |app: &mut App,
                        pending_inputs: &mut Vec<(WindowId, emInputEvent)>,
                        input_state: &mut emInputState| {
            app.scheduler.DoTimeSlice(
                &mut app.windows,
                &app.context,
                &mut app.framework_actions,
                pending_inputs,
                input_state,
                &fc,
                &app.pending_actions,
            );
        };

        // Slice 1: fire close_signal → FinishState 0→1 (pending_result=Cancel),
        //          then 1→2 (fire finish_signal, advance state).
        app.scheduler.fire(close_sig);
        do_slice(&mut app, &mut pending_inputs, &mut input_state);

        // Slices 2+3+4: FinishState 2→3 (count), 3→4 (count), 4→emit closure.
        // Each slice the engine is awoken because it returned true at state 1.
        do_slice(&mut app, &mut pending_inputs, &mut input_state);
        do_slice(&mut app, &mut pending_inputs, &mut input_state);
        do_slice(&mut app, &mut pending_inputs, &mut input_state);

        // After 4 slices the auto-delete branch (state==3, +1 more tick) has
        // fired. Check that exactly one closure was pushed onto pending_actions.
        assert_eq!(
            app.pending_actions.borrow().len(),
            1,
            "auto-delete must push exactly one close_dialog_by_id closure"
        );

        // Teardown — engine_id was already removed by close_dialog_by_id if
        // the closure had been drained; it hasn't yet, so remove it manually.
        // (The closure itself calls close_dialog_by_id when drained by App.)
        // Clear pending_actions to avoid running the closure against a partial
        // App in test teardown.
        app.pending_actions.borrow_mut().clear();
        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    // ─── Task 12 tests ───────────────────────────────────────────────────────

    /// Ports `release_event_is_ignored`: DlgPanel rejects key-release events.
    /// C++ parity: `DlgPanel::Input` guard at emDialog.cpp:465 checks
    /// `event.variant != InputVariant::Press`.
    #[test]
    fn dlg_panel_release_event_is_ignored() {
        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut init = TestInit::new();
        let finish_sig = init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::release(InputKey::Enter);
        let ps = PanelState {
            id: PanelId::null(),
            is_active: true,
            in_active_path: true,
            window_focused: true,
            enabled: true,
            viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0,
            memory_limit: u64::MAX,
            pixel_tallness: 1.0,
            height: 1.0,
        };
        let is = emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(!consumed, "key-release must not be consumed");
        assert_eq!(
            panel.pending_result, None,
            "release must not set pending_result"
        );
    }

    /// End-to-end: construct dialog via `emDialog::new`, add OK button,
    /// set `on_finish` callback, install headless, fire button click_signal,
    /// run one `DoTimeSlice`, assert `on_finish` was called with `Ok`.
    ///
    /// Ports `dialog_finish_fires_callback` and `dialog_fires_finish_signal_on_input_enter`.
    /// Satisfies plan §12 minimum coverage item 1.
    #[test]
    fn dialog_finish_fires_on_finish_callback_via_button_click() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        // Build emDialog via the public handle API so the full pending tree is
        // constructed by emDialog::new / AddCustomButton.
        let result_cell: Rc<RefCell<Option<DialogResult>>> = Rc::new(RefCell::new(None));
        let result_clone = Rc::clone(&result_cell);

        let (dlg, button_click_sig) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Test", look);
            dlg.AddCustomButton(&mut ctx, "OK", DialogResult::Ok);
            dlg.set_on_finish(Box::new(move |r, _sched| {
                *result_clone.borrow_mut() = Some(*r);
            }));

            // Extract button click_signal before consuming `pending`.
            let click_sig = {
                let pending = dlg.pending.as_mut().unwrap();
                let tree = pending.window.tree_mut();
                let mut beh = tree.take_behavior(dlg.root_panel_id).unwrap();
                let sig = beh.as_dlg_panel_mut().unwrap().button_signals[0].0;
                tree.put_behavior(dlg.root_panel_id, beh);
                sig
            };
            (dlg, click_sig)
        };

        // Push pending to app and install headless.
        let wid = WindowId::dummy();
        app.pending_top_level.push(dlg.pending.unwrap());
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        // Connect button click_signal to DialogPrivateEngine so firing it wakes
        // the engine (mirrors production `install_pending_top_level` behavior).
        app.scheduler.connect(button_click_sig, engine_id);

        // Fire the OK button click and run one slice.
        app.scheduler.fire(button_click_sig);
        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &app.pending_actions,
        );

        assert_eq!(
            *result_cell.borrow(),
            Some(DialogResult::Ok),
            "on_finish must be called with Ok when OK button click_signal fires"
        );

        // Teardown.
        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    /// End-to-end veto: `on_check_finish` returning `false` prevents
    /// finalization; returning `true` on the second trigger allows it.
    ///
    /// Ports `check_finish_can_veto` and `check_finish_lifecycle`.
    /// Satisfies plan §12 minimum coverage item 2.
    #[test]
    fn dialog_check_finish_veto_then_allow() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        // Veto counter: first call returns false (veto), second returns true (allow).
        let veto_count: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let veto_clone = Rc::clone(&veto_count);
        let finish_cell: Rc<RefCell<Option<DialogResult>>> = Rc::new(RefCell::new(None));
        let finish_clone = Rc::clone(&finish_cell);

        let (close_sig, root_id, wid) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Veto", look);
            dlg.set_on_check_finish(Box::new(move |_r, _dlg, _ctx| {
                let mut n = veto_clone.borrow_mut();
                *n += 1;
                *n > 1 // false on first call, true on subsequent
            }));
            dlg.set_on_finish(Box::new(move |r, _sched| {
                *finish_clone.borrow_mut() = Some(*r);
            }));

            let close_sig = dlg.close_signal;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.unwrap());
            (close_sig, root_id, WindowId::dummy())
        };

        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install");

        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);

        let do_slice =
            |app: &mut App, pi: &mut Vec<(WindowId, emInputEvent)>, is: &mut emInputState| {
                app.scheduler.DoTimeSlice(
                    &mut app.windows,
                    &app.context,
                    &mut app.framework_actions,
                    pi,
                    is,
                    &fc,
                    &app.pending_actions,
                );
            };

        // First close_signal: veto fires → finish_cell stays None.
        app.scheduler.fire(close_sig);
        do_slice(&mut app, &mut pending_inputs, &mut input_state);
        assert!(
            finish_cell.borrow().is_none(),
            "first close_signal must be vetoed"
        );

        // Inspect: pending_result was consumed by veto (not finalized).
        {
            let win = app.windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut beh = tree.take_behavior(root_id).unwrap();
            let dlg = beh.as_dlg_panel_mut().unwrap();
            assert_eq!(dlg.finish_state, 0, "veto must leave finish_state at 0");
            assert!(dlg.finalized_result.is_none(), "veto must not finalize");
            tree.put_behavior(root_id, beh);
            win.put_tree(tree);
        }

        // Second close_signal: veto allows → on_finish fires.
        app.scheduler.fire(close_sig);
        do_slice(&mut app, &mut pending_inputs, &mut input_state);
        assert_eq!(
            *finish_cell.borrow(),
            Some(DialogResult::Cancel),
            "second close_signal must finalize to Cancel"
        );

        // Teardown.
        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    // ─── Task 20 tests ───────────────────────────────────────────────────────

    /// End-to-end: build dialog, install headless, call `finish_post_show(Ok)`,
    /// drain `pending_actions`, `DoTimeSlice`, assert `finalized_result == Ok`
    /// and `on_finish` fired with `Ok`.
    ///
    /// Ports plan Step 20.5.
    #[test]
    fn finish_post_show_sets_pending_result() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        let result_cell: Rc<RefCell<Option<DialogResult>>> = Rc::new(RefCell::new(None));
        let result_clone = Rc::clone(&result_cell);

        // Build dialog, extract stable fields, push pending to app directly
        // (mirrors existing headless-install tests).
        let (dlg_id, finish_sig, root_id) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Test", look);
            dlg.AddCustomButton(&mut ctx, "OK", DialogResult::Ok);
            dlg.set_on_finish(Box::new(move |r, _sched| {
                *result_clone.borrow_mut() = Some(*r);
            }));
            let dlg_id = dlg.dialog_id;
            let finish_sig = dlg.finish_signal;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.unwrap());
            (dlg_id, finish_sig, root_id)
        };

        let wid = WindowId::dummy();
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        // Build a minimal handle to call finish_post_show. finish_post_show
        // only reads self.dialog_id and self.root_panel_id (both Copy), so
        // we construct a post-show handle directly.
        let dummy_sig = app.scheduler.create_signal();
        let handle = emDialog {
            dialog_id: dlg_id,
            finish_signal: finish_sig,
            close_signal: dummy_sig,
            root_panel_id: root_id,
            look: emLook::new(),
            pending: None, // post-show
        };
        {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            handle.finish_post_show(&mut ctx, DialogResult::Ok);
        }

        // Drain pending_actions (the finish_post_show closure + wake).
        app.drain_pending_actions_headless();

        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &app.pending_actions,
        );

        assert_eq!(
            *result_cell.borrow(),
            Some(DialogResult::Ok),
            "on_finish must fire with Ok after finish_post_show"
        );
        {
            let win = app.windows.get_mut(&wid).expect("window present");
            let mut tree = win.take_tree();
            let mut beh = tree.take_behavior(root_id).expect("behavior present");
            let dlg = beh.as_dlg_panel_mut().unwrap();
            assert_eq!(
                dlg.finalized_result,
                Some(DialogResult::Ok),
                "finalized_result must be Ok"
            );
            tree.put_behavior(root_id, beh);
            win.put_tree(tree);
        }

        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    /// Verifies first-fire guard: a second `finish_post_show` call while
    /// `pending_result` is already `Some` is a no-op (does not overwrite).
    ///
    /// Ports plan Step 20.5 double-call idempotence requirement.
    #[test]
    fn finish_post_show_double_call_is_noop() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        let (dlg_id, finish_sig, root_id) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "Test", look);
            dlg.AddCustomButton(&mut ctx, "OK", DialogResult::Ok);
            let dlg_id = dlg.dialog_id;
            let finish_sig = dlg.finish_signal;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.unwrap());
            (dlg_id, finish_sig, root_id)
        };

        let wid = WindowId::dummy();
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install");

        // Pre-set pending_result = Ok directly — simulates first call fired.
        {
            let win = app.windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut beh = tree.take_behavior(root_id).unwrap();
            beh.as_dlg_panel_mut().unwrap().pending_result = Some(DialogResult::Ok);
            tree.put_behavior(root_id, beh);
            win.put_tree(tree);
        }

        // Build a second handle with the same identity and call finish_post_show
        // with Cancel — the guard must block it.
        let dummy_sig2 = app.scheduler.create_signal();
        let handle2 = emDialog {
            dialog_id: dlg_id,
            finish_signal: finish_sig,
            close_signal: dummy_sig2,
            root_panel_id: root_id,
            look: emLook::new(),
            pending: None,
        };
        {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            handle2.finish_post_show(&mut ctx, DialogResult::Cancel);
        }

        // Drain pending_actions.
        app.drain_pending_actions_headless();

        // pending_result must still be Ok (Cancel was blocked by guard).
        {
            let win = app.windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut beh = tree.take_behavior(root_id).unwrap();
            let dlg = beh.as_dlg_panel_mut().unwrap();
            assert_eq!(
                dlg.pending_result,
                Some(DialogResult::Ok),
                "second finish_post_show must not overwrite pending_result"
            );
            tree.put_behavior(root_id, beh);
            win.put_tree(tree);
        }

        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    // ─── Phase 3.6 Task 2 test ───────────────────────────────────────────────

    /// Asserts that `DialogPrivateEngine::Cycle` invokes `DlgPanel.on_cycle_ext`
    /// exactly once per Cycle slice.
    ///
    /// Setup follows the headless-install shape used by
    /// `private_engine_observes_close_signal_sets_pending_cancel`:
    /// build DlgPanel, set `on_cycle_ext` to a closure incrementing an
    /// `Rc<Cell<u32>>` counter, install via `install_pending_top_level_headless`,
    /// wake the engine by firing close_signal, run one `DoTimeSlice`, assert
    /// counter == 1.
    ///
    /// The extension receives `&mut DlgPanel` directly — it must NOT call
    /// `ctx.tree.take_behavior(root_panel_id)` (already taken by the engine body).
    #[test]
    fn dialog_private_engine_calls_on_cycle_ext() {
        use crate::emGUIFramework::{App, PendingTopLevel};
        use crate::emWindow::WindowFlags;
        use std::cell::Cell;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        // Counter captured by the extension closure.
        let counter: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        let counter_clone = Rc::clone(&counter);

        // Build dialog tree, set on_cycle_ext before install.
        let mut tree = PanelTree::new();
        let root_id = tree.create_root("dlg", false);
        let finish_sig = app.scheduler.create_signal();
        let close_sig = app.scheduler.create_signal();
        let flags_sig = app.scheduler.create_signal();
        let focus_sig = app.scheduler.create_signal();
        let geom_sig = app.scheduler.create_signal();
        let mut dlg_panel = DlgPanel::new("Ext", emLook::new(), finish_sig);
        dlg_panel.on_cycle_ext = Some(Box::new(move |_dlg, _ctx| {
            counter_clone.set(counter_clone.get() + 1);
            false
        }));
        tree.set_behavior(root_id, Box::new(dlg_panel));

        let mut window = crate::emWindow::emWindow::new_top_level_pending(
            Rc::clone(&app.context),
            WindowFlags::empty(),
            "test-ext".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            crate::emColor::emColor::TRANSPARENT,
        );
        let _discarded = window.take_tree();
        window.put_tree(tree);

        let wid = WindowId::dummy();
        let dialog_id = app.allocate_dialog_id();
        app.pending_top_level.push(PendingTopLevel {
            dialog_id,
            window,
            close_signal: close_sig,
            private_engine_root_panel_id: root_id,
            wake_up_signals: Vec::new(),
        });
        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install registers DialogPrivateEngine");

        // Fire close_signal to wake the engine and run one slice.
        app.scheduler.fire(close_sig);
        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &app.pending_actions,
        );

        assert_eq!(
            counter.get(),
            1,
            "on_cycle_ext must be called exactly once per Cycle slice"
        );

        // Teardown.
        app.pending_actions.borrow_mut().clear();
        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }

    // ─── Phase 3.6.1 Task 1 test ─────────────────────────────────────────────

    /// Asserts that the widened `DialogCheckFinishCb` receives a live
    /// `&mut DlgPanel` and a live `&mut EngineCtx<'_>`.
    ///
    /// - Reads `ctx.engine_id` to prove `EngineCtx` is accessible.
    /// - Mutates `dlg.auto_delete` to prove `DlgPanel` is mutably reachable
    ///   (and observes the mutation after the Cycle).
    /// - Captures `Rc<Cell<bool>>` flag; asserts it is set after the Cycle.
    /// - Returns `true` (don't veto) so finalization proceeds normally.
    #[test]
    fn check_finish_widened_args_reachable() {
        use crate::emEngineCtx::InitCtx;
        use crate::emGUIFramework::App;
        use std::cell::Cell;
        use winit::window::WindowId;

        let mut app = App::new(Box::new(|_app, _el| {}));

        let ran_flag: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let ran_clone = Rc::clone(&ran_flag);
        let engine_id_cell: Rc<Cell<Option<crate::emEngine::EngineId>>> = Rc::new(Cell::new(None));
        let eid_clone = Rc::clone(&engine_id_cell);

        let (close_sig, root_id, wid) = {
            let mut ctx = InitCtx {
                scheduler: &mut app.scheduler,
                framework_actions: &mut app.framework_actions,
                root_context: &app.context,
                pending_actions: &app.pending_actions,
            };
            let look = emLook::new();
            let mut dlg = emDialog::new(&mut ctx, "WidenedArgs", look);
            // Ensure auto_delete starts true (default) so we can flip it to false
            // as the DlgPanel mutation proof.
            dlg.set_on_check_finish(Box::new(
                move |_r, dlg_panel: &mut DlgPanel, ctx: &mut crate::emEngineCtx::EngineCtx<'_>| {
                    // Read EngineCtx — proves the ctx arg is live.
                    eid_clone.set(Some(ctx.engine_id));
                    // Mutate DlgPanel — proves the dlg_panel arg is mutable.
                    dlg_panel.auto_delete = false;
                    ran_clone.set(true);
                    true // don't veto
                },
            ));

            let close_sig = dlg.close_signal;
            let root_id = dlg.root_panel_id;
            app.pending_top_level.push(dlg.pending.unwrap());
            (close_sig, root_id, WindowId::dummy())
        };

        let engine_id = app
            .install_pending_top_level_headless(wid)
            .expect("install");

        // Fire close_signal → pending_result = Cancel → step 3 → on_check_finish.
        app.scheduler.fire(close_sig);
        let mut pending_inputs: Vec<(WindowId, emInputEvent)> = Vec::new();
        let mut input_state = emInputState::new();
        let fc: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
        app.scheduler.DoTimeSlice(
            &mut app.windows,
            &app.context,
            &mut app.framework_actions,
            &mut pending_inputs,
            &mut input_state,
            &fc,
            &app.pending_actions,
        );

        // Closure must have run.
        assert!(ran_flag.get(), "on_check_finish closure must have run");

        // EngineCtx arg must have been a valid engine id (non-zero check is
        // just a sanity guard — the specific value is opaque).
        assert!(
            engine_id_cell.get().is_some(),
            "ctx.engine_id must be readable inside on_check_finish"
        );

        // DlgPanel mutation must be observable: auto_delete was flipped to false.
        {
            let win = app.windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut beh = tree.take_behavior(root_id).unwrap();
            let dlg = beh.as_dlg_panel_mut().unwrap();
            assert!(
                !dlg.auto_delete,
                "auto_delete must be false — mutation via &mut DlgPanel must persist"
            );
            tree.put_behavior(root_id, beh);
            win.put_tree(tree);
        }

        // The engine_id observed inside the closure must match the installed id.
        assert_eq!(
            engine_id_cell.get(),
            Some(engine_id),
            "ctx.engine_id inside on_check_finish must match installed engine"
        );

        // Teardown.
        app.scheduler.remove_engine(engine_id);
        app.scheduler.clear_pending_for_tests();
    }
}
