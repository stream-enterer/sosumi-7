use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::PanelCtx;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;

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
/// Children are created in Notice() (NF_VIEWING_CHANGED|NF_SOUGHT_NAME_CHANGED|NF_ACTIVE_CHANGED),
/// mirroring C++ emDirEntryPanel::Notice() which calls UpdateContentPanel()/UpdateAltPanel() directly.
pub struct emDirEntryPanel {
    ctx: Rc<emContext>,
    file_man: Rc<RefCell<emFileManModel>>,
    config: Rc<RefCell<emFileManViewConfig>>,
    dir_entry: emDirEntry,
    pub(crate) bg_color: u32,
    content_panel: Option<PanelId>,
    alt_panel: Option<PanelId>,
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

    /// Port of C++ emDirEntryPanel::UpdateContentPanel(forceRecreation, forceRelayout).
    /// Called from notice() and Cycle() with current panel state.
    fn update_content_panel(
        &mut self,
        ctx: &mut PanelCtx,
        state: &PanelState,
        force_recreation: bool,
        force_relayout: bool,
    ) {
        let (cx, cy, cw, ch, cc, min_content_vw) = {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            if self.dir_entry.IsDirectory() {
                (
                    theme_rec.DirContentX,
                    theme_rec.DirContentY,
                    theme_rec.DirContentW,
                    theme_rec.DirContentH,
                    emColor::from_packed(theme_rec.DirContentColor),
                    theme_rec.MinContentVW,
                )
            } else {
                (
                    theme_rec.FileContentX,
                    theme_rec.FileContentY,
                    theme_rec.FileContentW,
                    theme_rec.FileContentH,
                    emColor::from_packed(theme_rec.FileContentColor),
                    theme_rec.MinContentVW,
                )
            }
        };

        // Look up existing content child.
        let existing = ctx.find_child_by_name(CONTENT_NAME);
        if force_recreation {
            if let Some(child) = existing {
                ctx.delete_child(child);
                self.content_panel = None;
            }
        }
        let mut force_relayout = force_relayout || force_recreation;
        let existing = ctx.find_child_by_name(CONTENT_NAME);

        // C++ emDirEntryPanel.cpp:758-771: create when sought OR viewed+size+clip.
        let is_sought = ctx.is_seek_target() && ctx.seek_child_name() == CONTENT_NAME;
        let (clip_x1, clip_y1, clip_x2, clip_y2) = ctx.clip_rect();
        let should_create = is_sought
            || (state.viewed
                && state.viewed_rect.w * cw >= min_content_vw
                && ctx.panel_to_view_x(cx) < clip_x2
                && ctx.panel_to_view_x(cx + cw) > clip_x1
                && ctx.panel_to_view_y(cy) < clip_y2
                && ctx.panel_to_view_y(cy + ch) > clip_y1);

        if should_create {
            if existing.is_none() {
                let stat_mode = if self.dir_entry.IsDirectory() {
                    emcore::emFpPlugin::FileStatMode::Directory
                } else {
                    emcore::emFpPlugin::FileStatMode::Regular
                };
                let fppl = emcore::emFpPlugin::emFpPluginList::Acquire(&self.ctx);
                let fppl = fppl.borrow();
                let parent_arg = emcore::emFpPlugin::PanelParentArg::new(Rc::clone(&self.ctx));
                let behavior = fppl.CreateFilePanelWithStat(
                    ctx,
                    &parent_arg,
                    CONTENT_NAME,
                    self.dir_entry.GetPath(),
                    None,
                    stat_mode,
                    0,
                );
                let child_id = ctx.create_child_with(CONTENT_NAME, behavior);
                ctx.be_first_child(child_id);
                // Register for cycling so the file panel's model loads.
                ctx.wake_up_panel(child_id);
                self.content_panel = Some(child_id);
                force_relayout = true;
            }
        } else if let Some(child) = existing {
            // C++ line 785: delete if !InActivePath && (!InViewedPath || IsViewed())
            let in_active = ctx.child_in_active_path(child);
            let in_viewed = ctx.child_in_viewed_path(child);
            if !in_active && (!in_viewed || state.viewed) {
                ctx.delete_child(child);
                self.content_panel = None;
            }
        }

        if force_relayout {
            if let Some(child) = self.content_panel {
                ctx.layout_child_canvas(child, cx, cy, cw, ch, cc);
            }
        }
    }

    /// Port of C++ emDirEntryPanel::UpdateAltPanel(forceRecreation, forceRelayout).
    fn update_alt_panel(
        &mut self,
        ctx: &mut PanelCtx,
        state: &PanelState,
        force_recreation: bool,
        force_relayout: bool,
    ) {
        let (ax, ay, aw, ah, min_alt_vw) = {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            (
                theme_rec.AltX,
                theme_rec.AltY,
                theme_rec.AltW,
                theme_rec.AltH,
                theme_rec.MinAltVW,
            )
        };

        let existing = ctx.find_child_by_name(ALT_NAME);
        if force_recreation {
            if let Some(child) = existing {
                ctx.delete_child(child);
                self.alt_panel = None;
            }
        }
        let mut force_relayout = force_relayout || force_recreation;
        let existing = ctx.find_child_by_name(ALT_NAME);

        // C++ emDirEntryPanel.cpp:804-816: create when sought OR viewed+size+clip.
        let is_sought = ctx.is_seek_target() && ctx.seek_child_name() == ALT_NAME;
        let (clip_x1, clip_y1, clip_x2, clip_y2) = ctx.clip_rect();
        let should_create = is_sought
            || (state.viewed
                && state.viewed_rect.w * aw >= min_alt_vw
                && ctx.panel_to_view_x(ax) < clip_x2
                && ctx.panel_to_view_x(ax + aw) > clip_x1
                && ctx.panel_to_view_y(ay) < clip_y2
                && ctx.panel_to_view_y(ay + ah) > clip_y1);

        if should_create {
            if existing.is_none() {
                let alt = crate::emDirEntryAltPanel::emDirEntryAltPanel::new(
                    Rc::clone(&self.ctx),
                    self.dir_entry.clone(),
                    1,
                );
                let child_id = ctx.create_child_with(ALT_NAME, Box::new(alt));
                self.alt_panel = Some(child_id);
                force_relayout = true;
            }
        } else if let Some(child) = existing {
            let in_active = ctx.child_in_active_path(child);
            let in_viewed = ctx.child_in_viewed_path(child);
            if !in_active && (!in_viewed || state.viewed) {
                ctx.delete_child(child);
                self.alt_panel = None;
            }
        }

        if force_relayout {
            if let Some(child) = self.alt_panel {
                ctx.layout_child_canvas(child, ax, ay, aw, ah, emColor::from_packed(self.bg_color));
            }
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
    /// DIVERGED: (language-forced) C++ walks sibling panels via parent panel tree traversal.
    /// Rust accesses the emDirModel directly to enumerate entries in display
    /// order, since panel tree parent traversal is not available.
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
            // Range selection — select all entries between anchor and current
            let anchor_path = fm.GetShiftTgtSelPath().to_string();
            drop(fm); // Release borrow before acquiring model

            if !anchor_path.is_empty() {
                // Derive parent directory from path
                let parent_dir = std::path::Path::new(&path)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("");

                if !parent_dir.is_empty() {
                    let dm = crate::emDirModel::emDirModel::Acquire(&self.ctx, parent_dir);
                    let dm = dm.borrow();
                    let cfg = self.config.borrow();
                    let show_hidden = cfg.GetShowHiddenFiles();

                    // Collect visible entries in display order
                    let mut visible: Vec<crate::emDirEntry::emDirEntry> = Vec::new();
                    for i in 0..dm.GetEntryCount() {
                        let entry = dm.GetEntry(i);
                        if !entry.IsHidden() || show_hidden {
                            visible.push(entry.clone());
                        }
                    }

                    // Sort by config comparator to match display order
                    visible.sort_by(|a, b| {
                        let cmp = cfg.CompareDirEntries(a, b);
                        if cmp < 0 {
                            std::cmp::Ordering::Less
                        } else if cmp > 0 {
                            std::cmp::Ordering::Greater
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    });

                    // Find anchor and target indices
                    let anchor_idx = visible.iter().position(|e| e.GetPath() == anchor_path);
                    let target_idx = visible.iter().position(|e| e.GetPath() == path);
                    drop(cfg);
                    drop(dm);

                    if let (Some(a), Some(t)) = (anchor_idx, target_idx) {
                        let min = a.min(t);
                        let max = a.max(t);
                        let mut fm = self.file_man.borrow_mut();
                        for entry in &visible[min..=max] {
                            fm.SelectAsTarget(entry.GetPath());
                        }
                    } else {
                        // Fallback: just select this entry
                        let mut fm = self.file_man.borrow_mut();
                        fm.SelectAsTarget(&path);
                    }
                } else {
                    let mut fm = self.file_man.borrow_mut();
                    fm.SelectAsTarget(&path);
                }
            } else {
                // No anchor — just select this entry and set anchor
                let mut fm = self.file_man.borrow_mut();
                fm.SelectAsTarget(&path);
                fm.SetShiftTgtSelPath(&path);
            }
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
    /// Port of C++ emDirEntryPanel::Notice(flags):
    ///   if (flags & (NF_VIEWING_CHANGED|NF_SOUGHT_NAME_CHANGED|NF_ACTIVE_CHANGED))
    ///     UpdateContentPanel(); UpdateAltPanel();
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, ctx: &mut PanelCtx) {
        if flags.intersects(
            NoticeFlags::VIEWING_CHANGED
                | NoticeFlags::SOUGHT_NAME_CHANGED
                | NoticeFlags::ACTIVE_CHANGED,
        ) {
            self.update_content_panel(ctx, state, false, false);
            self.update_alt_panel(ctx, state, false, false);
        }
    }

    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        ctx: &mut PanelCtx,
    ) -> bool {
        // C++ Cycle: update bg on selection signal; update panels on config change.
        // Rust models don't have IsSignaled() yet — always update bg (conservative).
        self.update_bg_color();

        // C++ Cycle calls UpdateContentPanel(false,true)/UpdateAltPanel(false,true)
        // on config change. Build state from tree for forceRelayout pass.
        let pt = ctx.current_pixel_tallness;
        if ctx.tree.contains(ctx.id) {
            let state = ctx.tree.build_panel_state(ctx.id, false, pt);
            self.update_content_panel(ctx, &state, false, true);
            self.update_alt_panel(ctx, &state, false, true);
        }
        false
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
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
            theme_rec.BackgroundX,
            theme_rec.BackgroundY,
            theme_rec.BackgroundW,
            theme_rec.BackgroundH,
            r,
            r,
            bg,
            emColor::TRANSPARENT,
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
            theme_rec.NameX,
            theme_rec.NameY,
            theme_rec.NameW,
            theme_rec.NameH,
            name,
            theme_rec.NameH,
            name_color,
            bg,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            1.0,
        );

        // Path (shown when content area is visible)
        let content_w = if self.dir_entry.IsDirectory() {
            theme_rec.DirContentW
        } else {
            theme_rec.FileContentW
        };

        if self.content_panel.is_some() || state.viewed_rect.w * content_w >= theme_rec.MinContentVW
        {
            painter.PaintTextBoxed(
                theme_rec.PathX,
                theme_rec.PathY,
                theme_rec.PathW,
                theme_rec.PathH,
                self.dir_entry.GetPath(),
                theme_rec.PathH,
                emColor::from_packed(theme_rec.PathColor),
                bg,
                TextAlignment::Left,
                VAlign::Center,
                TextAlignment::Left,
                0.5,
                false,
                1.0,
            );

            // Content area background
            if self.dir_entry.IsDirectory() {
                painter.PaintRect(
                    theme_rec.DirContentX,
                    theme_rec.DirContentY,
                    theme_rec.DirContentW,
                    theme_rec.DirContentH,
                    emColor::from_packed(theme_rec.DirContentColor),
                    bg,
                );
            } else {
                painter.PaintRect(
                    theme_rec.FileContentX,
                    theme_rec.FileContentY,
                    theme_rec.FileContentW,
                    theme_rec.FileContentH,
                    emColor::from_packed(theme_rec.FileContentColor),
                    bg,
                );
            }
        }

        // Info area (permissions, owner, group, size, time)
        let info_color = emColor::from_packed(theme_rec.InfoColor);
        let time_str = FormatTime(self.dir_entry.GetStat().st_mtime, false);
        painter.PaintTextBoxed(
            theme_rec.InfoX,
            theme_rec.InfoY,
            theme_rec.InfoW,
            theme_rec.InfoH,
            &time_str,
            theme_rec.InfoH,
            info_color,
            bg,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            1.0,
        );
    }

    fn get_title(&self) -> Option<String> {
        Some(self.dir_entry.GetPath().to_string())
    }

    fn CreateControlPanel(&mut self, parent_ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let parent_dir = std::path::Path::new(self.dir_entry.GetPath())
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        let mut panel = {
            let mut sched = parent_ctx
                .as_sched_ctx()
                .expect("CreateControlPanel requires scheduler-reach PanelCtx");
            crate::emFileManControlPanel::emFileManControlPanel::new(
                &mut sched,
                Rc::clone(&self.ctx),
            )
        };
        if !parent_dir.is_empty() {
            panel = panel.with_dir_path(parent_dir);
        }
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
        // Children are created in notice(); LayoutChildren only positions them.
        // C++ UpdateContentPanel/UpdateAltPanel called with forceRelayout=true
        // when the layout rect changes.
        if let Some(child) = self.content_panel {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            let (cx, cy, cw, ch, cc) = if self.dir_entry.IsDirectory() {
                (
                    theme_rec.DirContentX,
                    theme_rec.DirContentY,
                    theme_rec.DirContentW,
                    theme_rec.DirContentH,
                    emColor::from_packed(theme_rec.DirContentColor),
                )
            } else {
                (
                    theme_rec.FileContentX,
                    theme_rec.FileContentY,
                    theme_rec.FileContentW,
                    theme_rec.FileContentH,
                    emColor::from_packed(theme_rec.FileContentColor),
                )
            };
            ctx.layout_child_canvas(child, cx, cy, cw, ch, cc);
        }
        if let Some(child) = self.alt_panel {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let theme_rec = theme.GetRec();
            ctx.layout_child_canvas(
                child,
                theme_rec.AltX,
                theme_rec.AltY,
                theme_rec.AltW,
                theme_rec.AltH,
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

    #[test]
    fn select_shift_range_selects_between_anchor_and_target() {
        // Tests the selection model behavior for shift-range selection.
        // When the emDirModel for /tmp is not loaded, the fallback path
        // should at minimum select the clicked entry.
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry1 = crate::emDirEntry::emDirEntry::from_path("/tmp/a.txt");
        let entry2 = crate::emDirEntry::emDirEntry::from_path("/tmp/c.txt");
        let mut panel1 = emDirEntryPanel::new(Rc::clone(&ctx), entry1);
        let mut panel2 = emDirEntryPanel::new(Rc::clone(&ctx), entry2);

        // Plain click on entry1 — sets anchor
        panel1.select(false, false);
        assert!(panel1.file_man.borrow().IsSelectedAsTarget("/tmp/a.txt"));
        assert_eq!(panel1.file_man.borrow().GetShiftTgtSelPath(), "/tmp/a.txt");

        // Shift click on entry2 — should attempt range selection
        // (Model for /tmp needs to be loaded for full range; fallback selects entry)
        panel2.select(true, false);
        assert!(panel2.file_man.borrow().IsSelectedAsTarget("/tmp/c.txt"));
    }
}
