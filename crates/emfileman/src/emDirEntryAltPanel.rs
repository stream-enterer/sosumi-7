//! Alternative content view for directory entries.
//!
//! Port of C++ `emDirEntryAltPanel`. Creates content via
//! `CreateFilePanel(..., alternative)` with incrementing alternative index.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;

use crate::emDirEntry::emDirEntry;
use crate::emFileManViewConfig::emFileManViewConfig;

/// Data for an alternative content view panel.
pub struct emDirEntryAltPanelData {
    pub dir_entry: emDirEntry,
    pub alternative: i32,
}

impl emDirEntryAltPanelData {
    pub fn new(dir_entry: emDirEntry, alternative: i32) -> Self {
        Self {
            dir_entry,
            alternative,
        }
    }
}

/// Alternative content view panel.
/// Port of C++ `emDirEntryAltPanel` (extends emPanel).
pub struct emDirEntryAltPanel {
    pub(crate) data: emDirEntryAltPanelData,
    ctx: Rc<emContext>,
    config: Rc<RefCell<emFileManViewConfig>>,
    content_panel: Option<PanelId>,
    alt_panel: Option<PanelId>,
    content_dirty: bool,
    alt_dirty: bool,
    last_viewed: bool,
    last_in_active_path: bool,
    last_config_gen: u64,
}

impl emDirEntryAltPanel {
    pub fn new(ctx: Rc<emContext>, dir_entry: emDirEntry, alternative: i32) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        let last_config_gen = config.borrow().GetChangeSignal();
        Self {
            data: emDirEntryAltPanelData::new(dir_entry, alternative),
            ctx,
            config,
            content_panel: None,
            alt_panel: None,
            content_dirty: true,
            alt_dirty: true,
            last_viewed: false,
            last_in_active_path: false,
            last_config_gen,
        }
    }

    pub fn update_dir_entry(&mut self, dir_entry: emDirEntry) {
        self.data.dir_entry = dir_entry;
        self.content_dirty = true;
        self.alt_dirty = true;
    }

    fn update_content_panel(&mut self, ctx: &mut PanelCtx) {
        if !self.content_dirty {
            return;
        }
        self.content_dirty = false;

        let should_create = self.last_viewed;
        let should_delete = !self.last_in_active_path && !self.last_viewed;

        if should_delete {
            if let Some(child) = self.content_panel.take() {
                ctx.delete_child(child);
            }
        } else if should_create && self.content_panel.is_none() {
            let fppl = emcore::emFpPlugin::emFpPluginList::Acquire(&self.ctx);
            let fppl = fppl.borrow();
            let parent_arg = emcore::emFpPlugin::PanelParentArg::new(Rc::clone(&self.ctx));
            let behavior = fppl.CreateFilePanelWithStat(
                ctx,
                &parent_arg,
                crate::emDirEntryPanel::CONTENT_NAME,
                self.data.dir_entry.GetPath(),
                None,
                if self.data.dir_entry.IsDirectory() {
                    emcore::emFpPlugin::FileStatMode::Directory
                } else {
                    emcore::emFpPlugin::FileStatMode::Regular
                },
                self.data.alternative as usize,
            );
            let child_id = ctx.create_child_with(crate::emDirEntryPanel::CONTENT_NAME, behavior);
            self.content_panel = Some(child_id);
        }
    }

    fn update_alt_panel(&mut self, ctx: &mut PanelCtx) {
        if !self.alt_dirty {
            return;
        }
        self.alt_dirty = false;

        let should_create = self.last_viewed;
        let should_delete = !self.last_in_active_path && !self.last_viewed;

        if should_delete {
            if let Some(child) = self.alt_panel.take() {
                ctx.delete_child(child);
            }
        } else if should_create && self.alt_panel.is_none() {
            let next_alt = emDirEntryAltPanel::new(
                Rc::clone(&self.ctx),
                self.data.dir_entry.clone(),
                self.data.alternative + 1,
            );
            let child_id =
                ctx.create_child_with(crate::emDirEntryPanel::ALT_NAME, Box::new(next_alt));
            self.alt_panel = Some(child_id);
        }
    }
}

impl PanelBehavior for emDirEntryAltPanel {
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(
            NoticeFlags::VIEWING_CHANGED
                | NoticeFlags::SOUGHT_NAME_CHANGED
                | NoticeFlags::ACTIVE_CHANGED,
        ) {
            let viewed_changed = state.viewed != self.last_viewed;
            let active_changed = state.in_active_path != self.last_in_active_path;
            self.last_viewed = state.viewed;
            self.last_in_active_path = state.in_active_path;
            if viewed_changed || active_changed {
                self.content_dirty = true;
                self.alt_dirty = true;
            }
        }
    }

    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        let cfg = self.config.borrow();
        let gen = cfg.GetChangeSignal();
        drop(cfg);
        if gen != self.last_config_gen {
            self.last_config_gen = gen;
            self.content_dirty = true;
            self.alt_dirty = true;
            return true;
        }
        false
    }

    fn IsOpaque(&self) -> bool {
        false
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        _w: f64,
        _h: f64,
        _state: &PanelState,
    ) {
        let config = self.config.borrow();
        let theme = config.GetTheme();
        let theme_rec = theme.GetRec();

        let label = format!("Alternative Content Panel #{}", self.data.alternative);
        let label_color = emColor::from_packed(theme_rec.LabelColor);
        let canvas = emColor::TRANSPARENT;

        painter.PaintTextBoxed(
            theme_rec.AltLabelX,
            theme_rec.AltLabelY,
            theme_rec.AltLabelW,
            theme_rec.AltLabelH,
            &label,
            theme_rec.AltLabelH,
            label_color,
            canvas,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            1.0,
        );

        // Content background
        let bg = emColor::from_packed(theme_rec.BackgroundColor);
        painter.PaintRect(
            theme_rec.AltContentX,
            theme_rec.AltContentY,
            theme_rec.AltContentW,
            theme_rec.AltContentH,
            bg,
            canvas,
        );
    }

    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // Mouse events in alt content area: content panel receives
        // the event via panel tree propagation. Nothing to handle here.
        false
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        self.update_content_panel(ctx);
        self.update_alt_panel(ctx);

        // Layout content panel
        if let Some(child_id) = self.content_panel {
            let config = self.config.borrow();
            let theme = config.GetTheme();
            let theme_rec = theme.GetRec();
            let bg = emColor::from_packed(theme_rec.BackgroundColor);
            ctx.layout_child_canvas(
                child_id,
                theme_rec.AltContentX,
                theme_rec.AltContentY,
                theme_rec.AltContentW,
                theme_rec.AltContentH,
                bg,
            );
        }
        // Layout alt panel
        if let Some(child_id) = self.alt_panel {
            let config = self.config.borrow();
            let theme = config.GetTheme();
            let theme_rec = theme.GetRec();
            let canvas = ctx.GetCanvasColor();
            ctx.layout_child_canvas(
                child_id,
                theme_rec.AltAltX,
                theme_rec.AltAltY,
                theme_rec.AltAltW,
                theme_rec.AltAltH,
                canvas,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let panel = emDirEntryAltPanel::new(Rc::clone(&ctx), entry, 1);
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn panel_has_correct_alternative_index() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let panel = emDirEntryAltPanel::new(Rc::clone(&ctx), entry, 3);
        assert_eq!(panel.data.alternative, 3);
    }
}
