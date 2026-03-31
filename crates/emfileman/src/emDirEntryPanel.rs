use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::PanelId;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};

use crate::emDirEntry::emDirEntry;
use crate::emFileManModel::emFileManModel;
use crate::emFileManViewConfig::emFileManViewConfig;

pub const CONTENT_NAME: &str = "";
pub const ALT_NAME: &str = "a";

/// Port of C++ FormatTime using libc::localtime_r
pub fn FormatTime(t: libc::time_t, nl: bool) -> String {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t_val = t;
    unsafe {
        libc::localtime_r(&t_val, &mut tm);
    }
    let sep = if nl { '\n' } else { ' ' };
    format!(
        "{:04}-{:02}-{:02}{}{:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        sep,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

/// Port of C++ emDirEntryPanel::UpdateBgColor
pub fn compute_bg_color(
    sel_src: bool,
    sel_tgt: bool,
    bg_color: u32,
    source_sel_color: u32,
    target_sel_color: u32,
) -> u32 {
    match (sel_src, sel_tgt) {
        (false, false) => bg_color,
        (true, false) => source_sel_color,
        (false, true) => target_sel_color,
        (true, true) => {
            // 50% blend of source and target
            let blend = |a: u32, b: u32, shift: u32| -> u32 {
                let va = (a >> shift) & 0xFF;
                let vb = (b >> shift) & 0xFF;
                (va + vb) / 2
            };
            (blend(source_sel_color, target_sel_color, 24) << 24)
                | (blend(source_sel_color, target_sel_color, 16) << 16)
                | (blend(source_sel_color, target_sel_color, 8) << 8)
                | blend(source_sel_color, target_sel_color, 0)
        }
    }
}

/// Directory entry panel — displays a single file or directory.
/// Port of C++ `emDirEntryPanel` (extends emPanel).
///
/// The rendering workhorse of emFileMan. Draws themed background, name,
/// info, borders, and content area. Creates content panels via the plugin
/// system and alt panels for alternative views.
///
/// DIVERGED: C++ UpdateContentPanel/UpdateAltPanel are called from
/// Notice()+Cycle() with full view state. Rust uses dirty flags set in
/// notice() and defers creation/deletion to LayoutChildren() for borrow
/// safety — LayoutChildren receives PanelCtx which allows child mutation.
pub struct emDirEntryPanel {
    ctx: Rc<emContext>,
    file_man: Rc<RefCell<emFileManModel>>,
    config: Rc<RefCell<emFileManViewConfig>>,
    dir_entry: emDirEntry,
    pub(crate) bg_color: u32,
    content_panel: Option<PanelId>,
    alt_panel: Option<PanelId>,
    content_dirty: bool,
    alt_dirty: bool,
    last_viewed: bool,
    last_in_active_path: bool,
    last_viewed_width: f64,
}

impl emDirEntryPanel {
    pub fn new(ctx: Rc<emContext>, dir_entry: emDirEntry) -> Self {
        let file_man = emFileManModel::Acquire(&ctx);
        let config = emFileManViewConfig::Acquire(&ctx);

        let bg_color = {
            let fm = file_man.borrow();
            let cfg = config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            compute_bg_color(
                fm.IsSelectedAsSource(dir_entry.GetPath()),
                fm.IsSelectedAsTarget(dir_entry.GetPath()),
                theme_rec.BackgroundColor,
                theme_rec.SourceSelectionColor,
                theme_rec.TargetSelectionColor,
            )
        };

        Self {
            ctx,
            file_man,
            config,
            dir_entry,
            bg_color,
            content_panel: None,
            alt_panel: None,
            content_dirty: true,
            alt_dirty: true,
            last_viewed: false,
            last_in_active_path: false,
            last_viewed_width: 0.0,
        }
    }

    pub fn GetDirEntry(&self) -> &emDirEntry {
        &self.dir_entry
    }

    pub fn UpdateDirEntry(&mut self, dir_entry: emDirEntry) {
        if self.dir_entry == dir_entry {
            return;
        }
        let path_changed = dir_entry.GetPath() != self.dir_entry.GetPath();
        self.dir_entry = dir_entry;
        if path_changed {
            self.update_bg_color();
        }
    }

    /// DIVERGED: C++ UpdateContentPanel is called from Notice+Cycle with
    /// full view state. Rust version uses cached dirty flags set in notice()
    /// and is called from LayoutChildren() for borrow safety.
    fn update_content_panel(&mut self, ctx: &mut PanelCtx) {
        if !self.content_dirty {
            return;
        }
        self.content_dirty = false;

        let (content_w, min_content_vw) = {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            let cw = if self.dir_entry.IsDirectory() {
                theme_rec.DirContentW
            } else {
                theme_rec.FileContentW
            };
            (cw, theme_rec.MinContentVW)
        };

        let should_create = self.last_viewed
            && self.last_viewed_width * content_w >= min_content_vw;
        let should_delete = !self.last_in_active_path && !self.last_viewed;

        if should_delete && self.content_panel.is_some() {
            if let Some(child) = self.content_panel.take() {
                ctx.delete_child(child);
            }
        } else if should_create && self.content_panel.is_none() {
            let stat_mode = if self.dir_entry.IsDirectory() {
                emcore::emFpPlugin::FileStatMode::Directory
            } else {
                emcore::emFpPlugin::FileStatMode::Regular
            };
            let fppl = emcore::emFpPlugin::emFpPluginList::Acquire(&self.ctx);
            let fppl = fppl.borrow();
            let parent_arg =
                emcore::emFpPlugin::PanelParentArg::new(Rc::clone(&self.ctx));
            let behavior = fppl.CreateFilePanelWithStat(
                &parent_arg,
                CONTENT_NAME,
                self.dir_entry.GetPath(),
                None,
                stat_mode,
                0,
            );
            let child_id = ctx.create_child_with(CONTENT_NAME, behavior);
            self.content_panel = Some(child_id);
        }
    }

    /// DIVERGED: C++ UpdateAltPanel is called from Notice+Cycle.
    /// Rust version uses cached dirty flags, called from LayoutChildren().
    fn update_alt_panel(&mut self, ctx: &mut PanelCtx) {
        if !self.alt_dirty {
            return;
        }
        self.alt_dirty = false;

        let (alt_w, min_alt_vw) = {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            (theme_rec.AltW, theme_rec.MinAltVW)
        };

        let should_create = self.last_viewed
            && self.last_viewed_width * alt_w >= min_alt_vw;
        let should_delete = !self.last_in_active_path && !self.last_viewed;

        if should_delete && self.alt_panel.is_some() {
            if let Some(child) = self.alt_panel.take() {
                ctx.delete_child(child);
            }
        } else if should_create && self.alt_panel.is_none() {
            let alt = crate::emDirEntryAltPanel::emDirEntryAltPanel::new(
                Rc::clone(&self.ctx),
                self.dir_entry.clone(),
                1,
            );
            let child_id = ctx.create_child_with(ALT_NAME, Box::new(alt));
            self.alt_panel = Some(child_id);
        }
    }

    fn update_bg_color(&mut self) {
        let fm = self.file_man.borrow();
        let cfg = self.config.borrow();
        let theme = cfg.GetTheme();
        let theme_rec = theme.GetRec();
        self.bg_color = compute_bg_color(
            fm.IsSelectedAsSource(self.dir_entry.GetPath()),
            fm.IsSelectedAsTarget(self.dir_entry.GetPath()),
            theme_rec.BackgroundColor,
            theme_rec.SourceSelectionColor,
            theme_rec.TargetSelectionColor,
        );
    }

    /// Port of C++ emDirEntryPanel::Select
    /// DIVERGED: Shift-range selection is simplified — selects only the
    /// current entry instead of walking sibling panels between
    /// ShiftTgtSelPath and current. Full range selection requires panel
    /// tree sibling enumeration not yet available.
    fn select(&mut self, shift: bool, ctrl: bool) {
        let path = self.dir_entry.GetPath().to_string();
        let mut fm = self.file_man.borrow_mut();

        if ctrl {
            // Toggle target selection
            if fm.IsSelectedAsTarget(&path) {
                fm.DeselectAsTarget(&path);
            } else {
                fm.SelectAsTarget(&path);
            }
            fm.SetShiftTgtSelPath(&path);
        } else if shift {
            // Range selection — select from ShiftTgtSelPath to current
            // For now, just select this entry (range requires sibling enumeration)
            fm.SelectAsTarget(&path);
            fm.SetShiftTgtSelPath(&path);
        } else {
            // Plain click: old targets become sources, select this as target
            fm.ClearSourceSelection();
            fm.SwapSelection();
            fm.SelectAsTarget(&path);
            fm.SetShiftTgtSelPath(&path);
        }
    }

    /// Port of C++ emDirEntryPanel::SelectSolely
    fn select_solely(&mut self) {
        let path = self.dir_entry.GetPath().to_string();
        let mut fm = self.file_man.borrow_mut();
        fm.ClearSourceSelection();
        fm.ClearTargetSelection();
        fm.SelectAsTarget(&path);
    }
}

impl PanelBehavior for emDirEntryPanel {
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.intersects(
            NoticeFlags::VIEW_CHANGED
                | NoticeFlags::SOUGHT_NAME_CHANGED
                | NoticeFlags::ACTIVE_CHANGED,
        ) {
            let viewed_changed = state.viewed != self.last_viewed;
            let active_changed = state.in_active_path != self.last_in_active_path;
            self.last_viewed = state.viewed;
            self.last_in_active_path = state.in_active_path;
            self.last_viewed_width = state.viewed_rect.w;
            if viewed_changed || active_changed {
                self.content_dirty = true;
                self.alt_dirty = true;
            }
        }
    }

    fn Cycle(&mut self, _ctx: &mut PanelCtx) -> bool {
        self.update_bg_color();
        false
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        match event.key {
            InputKey::MouseLeft => {
                if event.repeat >= 2 {
                    // Double-click: select solely (RunDefaultCommand out of scope)
                    self.select_solely();
                    true
                } else {
                    self.select(input_state.GetShift(), input_state.GetCtrl());
                    true
                }
            }
            InputKey::Enter => {
                self.select_solely();
                true
            }
            InputKey::Space => {
                self.select(input_state.GetShift(), input_state.GetCtrl());
                true
            }
            _ => false,
        }
    }

    fn IsOpaque(&self) -> bool {
        let cfg = self.config.borrow();
        let theme = cfg.GetTheme();
        let theme_rec = theme.GetRec();
        (self.bg_color >> 24) == 0xFF
            && theme_rec.BackgroundX <= 0.0
            && theme_rec.BackgroundY <= 0.0
            && theme_rec.BackgroundW >= 1.0
            && theme_rec.BackgroundRX <= 0.0
            && theme_rec.BackgroundRY <= 0.0
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, state: &PanelState) {
        let cfg = self.config.borrow();
        let theme = cfg.GetTheme();
        let theme_rec = theme.GetRec();
        let bg = emColor::from_packed(self.bg_color);

        // Background rounded rect
        let r = theme_rec.BackgroundRX.min(theme_rec.BackgroundRY);
        painter.PaintRoundRect(
            theme_rec.BackgroundX, theme_rec.BackgroundY,
            theme_rec.BackgroundW, theme_rec.BackgroundH,
            r, bg,
        );

        // Name color based on file type
        let name_color = if self.dir_entry.IsRegularFile() {
            let mode = self.dir_entry.GetStat().st_mode;
            if mode & (libc::S_IXUSR | libc::S_IXGRP | libc::S_IXOTH) != 0 {
                emColor::from_packed(theme_rec.ExeNameColor)
            } else {
                emColor::from_packed(theme_rec.NormalNameColor)
            }
        } else if self.dir_entry.IsDirectory() {
            emColor::from_packed(theme_rec.DirNameColor)
        } else {
            emColor::from_packed(theme_rec.OtherNameColor)
        };

        let name = self.dir_entry.GetName();
        painter.PaintTextBoxed(
            theme_rec.NameX, theme_rec.NameY,
            theme_rec.NameW, theme_rec.NameH,
            name, theme_rec.NameH,
            name_color, bg,
            TextAlignment::Left, VAlign::Center,
            TextAlignment::Left, 0.5, false, 1.0,
        );

        // Path (shown when content area is visible)
        let content_w = if self.dir_entry.IsDirectory() {
            theme_rec.DirContentW
        } else {
            theme_rec.FileContentW
        };

        if self.content_panel.is_some() || state.viewed_rect.w * content_w >= theme_rec.MinContentVW {
            painter.PaintTextBoxed(
                theme_rec.PathX, theme_rec.PathY,
                theme_rec.PathW, theme_rec.PathH,
                self.dir_entry.GetPath(), theme_rec.PathH,
                emColor::from_packed(theme_rec.PathColor), bg,
                TextAlignment::Left, VAlign::Center,
                TextAlignment::Left, 0.5, false, 1.0,
            );

            // Content area background
            if self.dir_entry.IsDirectory() {
                painter.PaintRect(
                    theme_rec.DirContentX, theme_rec.DirContentY,
                    theme_rec.DirContentW, theme_rec.DirContentH,
                    emColor::from_packed(theme_rec.DirContentColor), bg,
                );
            } else {
                painter.PaintRect(
                    theme_rec.FileContentX, theme_rec.FileContentY,
                    theme_rec.FileContentW, theme_rec.FileContentH,
                    emColor::from_packed(theme_rec.FileContentColor), bg,
                );
            }
        }

        // Info area (permissions, owner, group, size, time)
        let info_color = emColor::from_packed(theme_rec.InfoColor);
        let time_str = FormatTime(self.dir_entry.GetStat().st_mtime, false);
        painter.PaintTextBoxed(
            theme_rec.InfoX, theme_rec.InfoY,
            theme_rec.InfoW, theme_rec.InfoH,
            &time_str, theme_rec.InfoH,
            info_color, bg,
            TextAlignment::Left, VAlign::Center,
            TextAlignment::Left, 0.5, false, 1.0,
        );
    }

    fn get_title(&self) -> Option<String> {
        Some(self.dir_entry.GetPath().to_string())
    }

    fn CreateControlPanel(&mut self, parent_ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let panel = crate::emFileManControlPanel::emFileManControlPanel::new(Rc::clone(&self.ctx));
        Some(parent_ctx.create_child_with(name, Box::new(panel)))
    }

    fn GetIconFileName(&self) -> Option<String> {
        if self.dir_entry.IsDirectory() {
            Some("directory.tga".to_string())
        } else {
            Some("file.tga".to_string())
        }
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Create/delete children based on dirty flags
        self.update_content_panel(ctx);
        self.update_alt_panel(ctx);

        if let Some(child) = self.content_panel {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            let (cx, cy, cw, ch, cc) = if self.dir_entry.IsDirectory() {
                (theme_rec.DirContentX, theme_rec.DirContentY,
                 theme_rec.DirContentW, theme_rec.DirContentH,
                 emColor::from_packed(theme_rec.DirContentColor))
            } else {
                (theme_rec.FileContentX, theme_rec.FileContentY,
                 theme_rec.FileContentW, theme_rec.FileContentH,
                 emColor::from_packed(theme_rec.FileContentColor))
            };
            ctx.layout_child_canvas(child, cx, cy, cw, ch, cc);
        }
        if let Some(child) = self.alt_panel {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            ctx.layout_child_canvas(
                child,
                theme_rec.AltX, theme_rec.AltY,
                theme_rec.AltW, theme_rec.AltH,
                emColor::from_packed(self.bg_color),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_time_inline() {
        let t: libc::time_t = 1610000000;
        let s = FormatTime(t, false);
        assert!(s.contains("2021"));
        assert_eq!(s.matches('-').count(), 2);
        assert_eq!(s.matches(':').count(), 2);
        assert!(!s.contains('\n'));
    }

    #[test]
    fn format_time_newline() {
        let t: libc::time_t = 1610000000;
        let s = FormatTime(t, true);
        assert!(s.contains('\n'));
    }

    #[test]
    fn bg_color_no_selection() {
        let result = compute_bg_color(false, false, 0x112233FF, 0xAABBCCFF, 0xDDEEFFFF);
        assert_eq!(result, 0x112233FF);
    }

    #[test]
    fn bg_color_source_selection() {
        let result = compute_bg_color(true, false, 0x112233FF, 0xAABBCCFF, 0xDDEEFFFF);
        assert_eq!(result, 0xAABBCCFF);
    }

    #[test]
    fn bg_color_target_selection() {
        let result = compute_bg_color(false, true, 0x112233FF, 0xAABBCCFF, 0xDDEEFFFF);
        assert_eq!(result, 0xDDEEFFFF);
    }

    #[test]
    fn bg_color_both_selections_blended() {
        let result = compute_bg_color(true, true, 0x112233FF, 0xAABBCCFF, 0xDDEEFFFF);
        assert_ne!(result, 0xAABBCCFF);
        assert_ne!(result, 0xDDEEFFFF);
    }

    #[test]
    fn content_name_constants() {
        assert_eq!(CONTENT_NAME, "");
        assert_eq!(ALT_NAME, "a");
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn panel_initial_bg_color() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);
        assert_ne!(panel.bg_color, 0);
    }

    #[test]
    fn panel_get_title() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);
        assert_eq!(panel.get_title(), Some("/tmp".to_string()));
    }

    #[test]
    fn select_solely_clears_and_selects() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let mut panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        panel.select_solely();

        let fm = panel.file_man.borrow();
        assert!(fm.IsSelectedAsTarget("/tmp"));
        assert_eq!(fm.GetTargetSelectionCount(), 1);
        assert_eq!(fm.GetSourceSelectionCount(), 0);
    }

    #[test]
    fn select_plain_swaps_selection() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let mut panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        // First click: selects as target
        panel.select(false, false);
        {
            let fm = panel.file_man.borrow();
            assert!(fm.IsSelectedAsTarget("/tmp"));
        }

        // Create another panel and click it
        let entry2 = crate::emDirEntry::emDirEntry::from_path("/var");
        let mut panel2 = emDirEntryPanel::new(Rc::clone(&ctx), entry2);
        panel2.select(false, false);

        let fm = panel2.file_man.borrow();
        assert!(fm.IsSelectedAsTarget("/var"));
        // /tmp should now be a source (swapped)
        assert!(fm.IsSelectedAsSource("/tmp"));
    }

    #[test]
    fn select_ctrl_toggles() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let mut panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        panel.select(false, true); // ctrl-click: select
        assert!(panel.file_man.borrow().IsSelectedAsTarget("/tmp"));

        panel.select(false, true); // ctrl-click: deselect
        assert!(!panel.file_man.borrow().IsSelectedAsTarget("/tmp"));
    }
}
