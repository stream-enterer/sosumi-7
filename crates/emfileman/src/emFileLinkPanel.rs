//! Port of C++ emFileLinkPanel content coordinate calculation and border constants.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emFilePanel::emFilePanel;

use emcore::emEngineCtx::PanelCtx;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;

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
    needs_update: bool,
    last_viewed: bool,
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
            needs_update: true,
            last_viewed: false,
        }
    }

    pub fn set_link_model(&mut self, model: Rc<RefCell<emFileLinkModel>>) {
        self.model = Some(model);
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
    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.file_panel.refresh_vir_file_state();
        false
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // Port of C++ emFileLinkPanel::AutoExpand. Load the model
        // synchronously (C++ uses emEngine; Rust loads here) and
        // create the child panel.
        if let Some(ref model_rc) = self.model {
            let _ = model_rc.borrow_mut().ensure_loaded();
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
            self.needs_update = true;
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

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        if !self.file_panel.GetVirFileState().is_good() && self.child_panel.is_none() {
            self.file_panel.paint_status(painter, w, h);
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

    /// DIVERGED: C++ calls UpdateDataAndChildPanel from Cycle() and Notice().
    /// Rust defers to LayoutChildren() for borrow safety — the RefCell holding
    /// the panel cannot be borrowed mutably while also creating/deleting child
    /// panels. The timing difference is at most one frame. This matches the
    /// established pattern in emDirEntryPanel.
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if self.needs_update {
            self.update_data_and_child_panel(ctx, self.last_viewed);
            self.needs_update = false;
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
}
