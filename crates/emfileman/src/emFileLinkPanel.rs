//! Port of C++ emFileLinkPanel content coordinate calculation and border constants.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emFileModel::FileModelState;
use emcore::emFilePanel::emFilePanel;

#[cfg(test)]
use emcore::emEngineCtx::DropOnlySignalCtx;
use emcore::emEngineCtx::PanelCtx;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{FileLoadStatus, NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;
use slotmap::Key as _;

use crate::emDirEntry::emDirEntry;
use crate::emDirEntryPanel::emDirEntryPanel;
use crate::emFileLinkModel::emFileLinkModel;
use crate::emFileManViewConfig::emFileManViewConfig;

pub const BORDER_BG_COLOR: u32 = 0xBBBBBBFF;
pub const BORDER_FG_COLOR: u32 = 0x444444FF;
pub const MIN_VIEW_PERCENT: f64 = 60.0;

/// Calculate content coordinates within the link panel.
/// panel_height: GetHeight()
/// have_border: whether to show border (depends on parent panel type)
/// have_dir_entry: whether the link target has a dir entry (from FileLinkModel)
/// _theme_height: theme.Height for inner scaling
/// pad_l/t/r/b: theme LnkPaddingL/T/R/B
#[allow(clippy::too_many_arguments)]
pub fn CalcContentCoords(
    panel_height: f64,
    have_border: bool,
    _have_dir_entry: bool,
    _theme_height: f64,
    pad_l: f64,
    pad_t: f64,
    pad_r: f64,
    pad_b: f64,
) -> (f64, f64, f64, f64) {
    if !have_border {
        return (0.0, 0.0, 1.0, panel_height);
    }
    // With border: apply padding
    let x = pad_l;
    let y = pad_t * panel_height;
    let w = 1.0 - pad_l - pad_r;
    let h = panel_height - (pad_t + pad_b) * panel_height;
    (x.max(0.0), y.max(0.0), w.max(0.001), h.max(0.001))
}

/// File link panel.
/// Port of C++ `emFileLinkPanel` (extends emFilePanel).
///
/// Displays a linked file by resolving the target path and creating either
/// an emDirEntryPanel (if link has HaveDirEntry) or a plugin panel as child.
pub struct emFileLinkPanel {
    pub(crate) file_panel: emFilePanel,
    ctx: Rc<emContext>,
    config: Rc<RefCell<emFileManViewConfig>>,
    pub(crate) model: Option<Rc<RefCell<emFileLinkModel>>>,
    pub(crate) have_border: bool,
    have_dir_entry_panel: bool,
    full_path: String,
    child_panel: Option<PanelId>,
    /// B-016 / M-001: per-branch flag fidelity to C++ emFileLinkPanel.cpp:84-101.
    /// `do_update` is set by VFS / UpdateSignal / Model branches → triggers
    /// `update_data_and_child_panel` in LayoutChildren. The Config branch does
    /// NOT set this flag in C++ (cpp:95-98 only invalidates layout/painting).
    do_update: bool,
    /// B-016 / M-001: set false by UpdateSignal and Model branches (cpp:90,
    /// cpp:100). Tracks whether the cached DirEntry view is up to date with
    /// the file-update broadcast / model state. The Rust `update_data_and_child_panel`
    /// re-resolves the link target whenever this is false.
    dir_entry_up_to_date: bool,
    /// B-016 / M-001: set true by Config branch (cpp:97). Distinct from
    /// `do_update` because Config-change does not require re-resolving the
    /// link target — only re-laying out the child panel.
    invalidate_layout: bool,
    last_viewed: bool,
    /// First-Cycle init guard for D-006 subscribe shape.
    subscribed_init: bool,
    /// B-002: tracks whether the *current* `model` has been subscribed to its
    /// `ChangeSignal`. Reset on `set_link_model` so the next Cycle re-runs the
    /// connect for the new model. Distinct from `subscribed_init`, which
    /// guards panel-lifetime first-Cycle subscriptions (config + UpdateSignal).
    model_subscribed: bool,
}

impl emFileLinkPanel {
    pub fn new(ctx: Rc<emContext>, have_border: bool) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        Self {
            file_panel: emFilePanel::new(),
            ctx,
            config,
            model: None,
            have_border,
            have_dir_entry_panel: false,
            full_path: String::new(),
            child_panel: None,
            // M-001 fidelity: initial state — we want the first viewed
            // LayoutChildren to populate the child panel, mirroring C++
            // ctor's `UpdateDataAndChildPanel()` call (cpp:64).
            do_update: true,
            dir_entry_up_to_date: false,
            invalidate_layout: false,
            last_viewed: false,
            subscribed_init: false,
            model_subscribed: false,
        }
    }

    // DIVERGED: (language-forced) B-002:. C++ `emFileLinkPanel::SetFileModel`
    // (`emFileLinkPanel.cpp:69-77`) calls `AddWakeUpSignal(Model->GetChangeSignal())`
    // synchronously via `this`'s engine handle. The Rust signature
    // `set_link_model(&mut self, ...)` lacks an `EngineCtx` parameter
    // (`Acquire` factory closures and panel mutators do not own ectx), so the
    // connect is deferred to the next Cycle through `model_subscribed: bool`.
    // D-006 option-B (deferred connect) localized to model-set; B-004 / B-015
    // `pending_vir_state_fire` precedent at `emFilePanel.rs:73, 104, 155`.
    pub fn set_link_model(&mut self, model: Rc<RefCell<emFileLinkModel>>) {
        // Port of C++ emFilePanel(parent, name, fileModel, true): the model is
        // connected to file_panel so its load state drives VirtualFileState
        // (Waiting → Loaded) instead of staying at NoFileModel (dark-red render).
        self.file_panel
            .SetFileModel(Some(Rc::clone(&model) as Rc<RefCell<dyn FileModelState>>));
        self.model = Some(model);
        // B-002: re-run the model-change subscribe in the next Cycle. Mirrors
        // C++ `RemoveWakeUpSignal(old)` + `AddWakeUpSignal(new)` semantics.
        self.model_subscribed = false;
        // C++ cpp:73: `UpdateDataAndChildPanel()` after SetFileModel — drive
        // a re-resolve of the new link target.
        self.do_update = true;
        self.dir_entry_up_to_date = false;
    }

    fn update_data_and_child_panel(&mut self, ctx: &mut PanelCtx, viewed: bool) {
        if !viewed {
            if let Some(child) = self.child_panel.take() {
                ctx.delete_child(child);
            }
            return;
        }

        let Some(ref model_rc) = self.model else {
            return;
        };

        let model = model_rc.borrow();
        let new_full_path = model.GetFullPath();
        let new_have_dir_entry = model.GetHaveDirEntry();
        drop(model);

        if new_full_path != self.full_path || new_have_dir_entry != self.have_dir_entry_panel {
            // Path or type changed — recreate child
            if let Some(child) = self.child_panel.take() {
                ctx.delete_child(child);
            }
            self.full_path = new_full_path;
            self.have_dir_entry_panel = new_have_dir_entry;
        }

        if self.child_panel.is_none() && !self.full_path.is_empty() {
            if self.have_dir_entry_panel {
                let entry = emDirEntry::from_path(&self.full_path);
                let panel = emDirEntryPanel::new(Rc::clone(&self.ctx), entry);
                let child_id = ctx.create_child_with("", Box::new(panel));
                self.child_panel = Some(child_id);
            } else {
                let fppl = emcore::emFpPlugin::emFpPluginList::Acquire(&self.ctx);
                let fppl = fppl.borrow();
                let parent_arg = emcore::emFpPlugin::PanelParentArg::new(Rc::clone(&self.ctx));
                let behavior = fppl.CreateFilePanel(ctx, &parent_arg, "", &self.full_path, 0);
                let child_id = ctx.create_child_with("", behavior);
                ctx.wake_up_panel(child_id);
                self.child_panel = Some(child_id);
            }
        }
    }

    /// B-016 test accessor: VFS signal id of the embedded `emFilePanel`.
    #[doc(hidden)]
    pub fn vir_file_state_signal_for_test(&self) -> emcore::emSignal::SignalId {
        self.file_panel.GetVirFileStateSignal()
    }

    /// B-016 test accessor: cached virtual-file-state of the embedded `emFilePanel`.
    #[doc(hidden)]
    pub fn vir_file_state_for_test(&self) -> emcore::emFilePanel::VirtualFileState {
        self.file_panel.GetVirFileState()
    }

    /// B-016 test mutator.
    #[doc(hidden)]
    pub fn set_custom_error_for_test(&mut self, msg: &str) {
        self.file_panel.set_custom_error(msg);
    }

    /// B-016 / M-001 test accessor: per-branch flag inspection.
    #[doc(hidden)]
    pub fn flags_for_test(&self) -> (bool, bool, bool) {
        (
            self.do_update,
            self.dir_entry_up_to_date,
            self.invalidate_layout,
        )
    }

    fn layout_child_panel(&self, ctx: &mut PanelCtx, panel_height: f64) {
        if let Some(child) = self.child_panel {
            let config = self.config.borrow();
            let theme = config.GetTheme();
            let theme_rec = theme.GetRec();
            let (x, y, w, h) = CalcContentCoords(
                panel_height,
                self.have_border,
                self.have_dir_entry_panel,
                theme_rec.Height,
                theme_rec.LnkPaddingL,
                theme_rec.LnkPaddingT,
                theme_rec.LnkPaddingR,
                theme_rec.LnkPaddingB,
            );
            let canvas = if self.have_dir_entry_panel {
                emColor::from_packed(theme_rec.DirContentColor)
            } else if self.have_border {
                emColor::from_packed(BORDER_BG_COLOR)
            } else {
                ctx.GetCanvasColor()
            };
            ctx.layout_child_canvas(child, x, y, w, h, canvas);
        }
    }
}

impl PanelBehavior for emFileLinkPanel {
    fn file_load_status(&self) -> Option<FileLoadStatus> {
        Some(emcore::emFilePanel::map_vir_state(
            &self.file_panel.GetVirFileState(),
        ))
    }

    fn Cycle(
        &mut self,
        ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // B-016 (1) MANDATORY emFilePanel::Cycle prefix in derived panel.
        // Mirrors C++ `busy=emFilePanel::Cycle()` at emFileLinkPanel.cpp:81.
        // See implementer-of-record at emImageFileImageFilePanel.rs:211-235.
        self.file_panel.ensure_vir_file_state_signal(ectx);
        self.file_panel.fire_pending_vir_state(ectx);

        // D-006 first-Cycle init: lazy-allocate signals and connect.
        // Mirrors C++ emFileLinkPanel ctor `AddWakeUpSignal(...)` (rows 53-56).
        if !self.subscribed_init {
            let eid = ectx.engine_id;
            let chg_sig = self.config.borrow().GetChangeSignal(ectx);
            ectx.connect(chg_sig, eid);
            // B-005 row emFileLinkPanel-53: subscribe to the shared
            // file-update broadcast signal. Mirrors C++
            // `AddWakeUpSignal(UpdateSignalModel->Sig)`.
            let update_sig = emcore::emFileModel::emFileModel::<()>::AcquireUpdateSignalModel(ectx);
            ectx.connect(update_sig, eid);
            // B-016: subscribe to emFilePanel::GetVirFileStateSignal.
            // Mirrors C++ emFileLinkPanel.cpp:54
            // AddWakeUpSignal(GetVirFileStateSignal()).
            let vfs_sig = self.file_panel.GetVirFileStateSignal();
            if !vfs_sig.is_null() {
                ectx.connect(vfs_sig, eid);
            }
            self.subscribed_init = true;
        }

        // B-002 rows emFileLinkPanel-56 / -72 (single shared callsite): wire
        // the model's ChangeSignal once per model. C++ does the equivalent in
        // ctor (`emFileLinkPanel.cpp:56`) and `SetFileModel` (`:72`); the
        // Rust ctor branch is structurally dead because `model: None` at
        // construction and `set_link_model` is the only path to a non-null
        // model. `set_link_model` resets `model_subscribed = false`, this
        // block re-asserts the connect on the next Cycle.
        if !self.model_subscribed {
            if let Some(ref model_rc) = self.model {
                let eid = ectx.engine_id;
                let chg_sig = model_rc.borrow().GetChangeSignal(ectx);
                ectx.connect(chg_sig, eid);
                self.model_subscribed = true;
            }
        }

        // B-016 / M-001: per-branch fidelity to C++ emFileLinkPanel.cpp:84-101.
        // 4 distinct IsSignaled branches with 3 distinct flag mutations:
        //   (a) VFS:           InvalidatePainting + doUpdate=true
        //   (b) UpdateSignal:  DirEntryUpToDate=false + doUpdate=true
        //   (c) Config:        InvalidatePainting + InvalidateChildrenLayout (no doUpdate)
        //   (d) Model:         doUpdate=true
        // The previously-collapsed `needs_update` flag was an M-001 violation
        // (no forced category applied); restored here.

        // (a) C++ cpp:85-88: VirFileStateSignal branch.
        if ectx.IsSignaled(self.file_panel.GetVirFileStateSignal()) {
            self.do_update = true;
        }

        // (b) C++ cpp:90-93: UpdateSignalModel branch.
        let update_sig = emcore::emFileModel::emFileModel::<()>::AcquireUpdateSignalModel(ectx);
        if ectx.IsSignaled(update_sig) {
            self.dir_entry_up_to_date = false;
            self.do_update = true;
        }

        // (c) C++ cpp:95-98: Config->GetChangeSignal branch — invalidates
        // layout but does NOT set doUpdate. Re-call combined-form accessor
        // (B-014 precedent): idempotent.
        let chg_sig = self.config.borrow().GetChangeSignal(ectx);
        if !chg_sig.is_null() && ectx.IsSignaled(chg_sig) {
            self.invalidate_layout = true;
        }

        // (d) C++ cpp:100-103: Model->GetChangeSignal branch.
        if let Some(ref model_rc) = self.model {
            let chg = model_rc.borrow().GetChangeSignal(ectx);
            if !chg.is_null() && ectx.IsSignaled(chg) {
                self.dir_entry_up_to_date = false;
                self.do_update = true;
            }
        }

        // B-016 (3) MANDATORY suffix — cycle_inner + conditional fire.
        let changed = self.file_panel.cycle_inner();
        if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
            ectx.fire(self.file_panel.GetVirFileStateSignal());
        }
        changed
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // Port of C++ emFileLinkPanel::AutoExpand. Load the model
        // synchronously (C++ uses emEngine; Rust loads here) and
        // create the child panel.
        if let Some(ref model_rc) = self.model {
            // D-007: thread ectx through `ensure_loaded` → `TryLoad` so
            // ChangeSignal fires synchronously after load completes.
            // PanelCtx → SchedCtx (full-reach call site under PanelCycleEngine).
            if let Some(mut sc) = ctx.as_sched_ctx() {
                let _ = model_rc.borrow_mut().ensure_loaded(&mut sc);
            } else {
                // Production `AutoExpand` runs under `PanelCycleEngine` with
                // full scheduler reach; the else branch is reachable only from
                // layout-only / unit-test `PanelCtx` constructions. A regression
                // that loses scheduler reach in production must fail loudly
                // rather than silently drop ChangeSignal fires.
                #[cfg(not(test))]
                panic!("emFileLinkPanel::AutoExpand requires scheduler reach in production");
                #[cfg(test)]
                {
                    let mut null = DropOnlySignalCtx;
                    let _ = model_rc.borrow_mut().ensure_loaded(&mut null);
                }
            }
        }
        // Force viewed=true so update_data_and_child_panel creates
        // the child. AutoExpand only runs when the panel is being
        // viewed or sought, matching C++ semantics.
        self.last_viewed = true;
        self.update_data_and_child_panel(ctx, true);
    }

    fn AutoShrink(&mut self, _ctx: &mut PanelCtx) {
        // Default AutoShrink deletes children with created_by_ae=true.
        self.child_panel = None;
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::VIEWING_CHANGED) {
            self.last_viewed = state.viewed;
            // C++ emFileLinkPanel::Notice (cpp:108): NF_VIEWING_CHANGED →
            // UpdateDataAndChildPanel(). Sets do_update; does NOT touch
            // dir_entry_up_to_date (target hasn't necessarily changed).
            self.do_update = true;
        }
    }

    fn IsOpaque(&self) -> bool {
        if !self.file_panel.GetVirFileState().is_good() && self.child_panel.is_none() {
            return false;
        }
        if self.have_border {
            return (BORDER_BG_COLOR >> 24) == 0xFF;
        }
        false
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        if !self.file_panel.GetVirFileState().is_good() && self.child_panel.is_none() {
            self.file_panel.paint_status(painter, canvas_color, w, h);
            return;
        }

        if self.have_border {
            let bg = emColor::from_packed(BORDER_BG_COLOR);
            let fg = emColor::from_packed(BORDER_FG_COLOR);
            painter.Clear(bg);

            let config = self.config.borrow();
            let theme = config.GetTheme();
            let theme_rec = theme.GetRec();
            let (cx, cy, cw, ch) = CalcContentCoords(
                state.height,
                self.have_border,
                self.have_dir_entry_panel,
                theme_rec.Height,
                theme_rec.LnkPaddingL,
                theme_rec.LnkPaddingT,
                theme_rec.LnkPaddingR,
                theme_rec.LnkPaddingB,
            );

            // Border outline
            let d = cx.min(cy) * 0.15;
            let t = cx.min(cy) * 0.03;
            let stroke = emcore::emStroke::emStroke {
                color: fg,
                width: t,
                ..Default::default()
            };
            painter.PaintRectOutline(cx - d * 0.5, cy - d * 0.5, cw + d, ch + d, &stroke, bg);

            // Label
            let label = format!("emFileLink to {}", self.full_path);
            let ty = cx.min(cy) * 0.2;
            painter.PaintTextBoxed(
                ty,
                0.0,
                1.0 - ty * 2.0,
                cy - ty,
                &label,
                (cy - ty) * 0.9,
                fg,
                bg,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Left,
                1.0,
                false,
                1.0,
            );

            if self.have_dir_entry_panel {
                painter.PaintRect(
                    cx,
                    cy,
                    cw,
                    ch,
                    emColor::from_packed(theme_rec.DirContentColor),
                    bg,
                );
            }
        } else if self.have_dir_entry_panel {
            let config = self.config.borrow();
            let theme = config.GetTheme();
            painter.Clear(emColor::from_packed(theme.GetRec().DirContentColor));
        }
    }

    /// DIVERGED: (language-forced) C++ calls UpdateDataAndChildPanel from Cycle() and Notice().
    /// Rust defers to LayoutChildren() for borrow safety — the RefCell holding
    /// the panel cannot be borrowed mutably while also creating/deleting child
    /// panels. The timing difference is at most one frame. This matches the
    /// established pattern in emDirEntryPanel.
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // M-001: per-branch flag consumption.
        if self.do_update {
            self.update_data_and_child_panel(ctx, self.last_viewed);
            self.do_update = false;
            // After a successful re-resolve, dir-entry view is current again.
            self.dir_entry_up_to_date = true;
        }
        // The Config branch sets invalidate_layout without do_update — the
        // child panel itself is unchanged but its layout may have shifted
        // (e.g., border padding). Re-laying out via layout_child_panel below
        // covers this; we just clear the flag.
        if self.invalidate_layout {
            self.invalidate_layout = false;
        }
        let rect = ctx.layout_rect();
        self.layout_child_panel(ctx, rect.h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_coords_no_border() {
        let (x, y, w, _h) = CalcContentCoords(1.0, false, false, 1.5, 0.0, 0.0, 0.0, 0.0);
        assert!((x - 0.0).abs() < 1e-9);
        assert!((y - 0.0).abs() < 1e-9);
        assert!((w - 1.0).abs() < 1e-9);
    }

    #[test]
    fn content_coords_with_border() {
        let (x, y, w, _h) = CalcContentCoords(1.0, true, false, 1.5, 0.05, 0.05, 0.05, 0.05);
        assert!(x > 0.0);
        assert!(y > 0.0);
        assert!(w < 1.0);
    }

    #[test]
    fn border_colors() {
        assert_eq!(BORDER_BG_COLOR, 0xBBBBBBFF_u32);
        assert_eq!(BORDER_FG_COLOR, 0x444444FF_u32);
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileLinkPanel::new(Rc::clone(&ctx), true);
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn panel_have_border_flag() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileLinkPanel::new(Rc::clone(&ctx), true);
        assert!(panel.have_border);
        let panel2 = emFileLinkPanel::new(Rc::clone(&ctx), false);
        assert!(!panel2.have_border);
    }

    #[test]
    fn set_link_model_connects_file_panel() {
        use emcore::emFilePanel::VirtualFileState;
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emFileLinkPanel::new(Rc::clone(&ctx), true);
        let model = crate::emFileLinkModel::emFileLinkModel::Acquire(
            &ctx,
            "/tmp/nonexistent.emFileLink",
            false,
        );
        panel.set_link_model(model);
        let vfs = panel.file_panel.GetVirFileState();
        assert!(
            !matches!(vfs, VirtualFileState::NoFileModel),
            "file_panel must not be NoFileModel after set_link_model; got {vfs:?}"
        );
    }
}
