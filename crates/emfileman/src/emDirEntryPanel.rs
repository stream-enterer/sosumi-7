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

    /// Port of C++ `emDirEntryPanel::PaintInfo`
    /// (emDirEntryPanel.cpp:484-725). Linux-only port: the Windows attribute
    /// branch (cpp:642-648) and the Windows `IsDrive`/`IsDirectory` guards
    /// around the Size and Time fields are intentionally omitted.
    ///
    /// DIVERGED: (upstream-gap-forced) Linux-only port; Windows attribute
    /// branch (emDirEntryPanel.cpp:642-648), the Windows
    /// `IsDrive`/`IsDirectory` guards around the Size and Time fields
    /// (cpp:686-688, 707-709, 711-713, 722-724), and the Windows drive-type
    /// extension of the Type field (cpp:611-626) are intentionally omitted.
    /// The C++ source gates these on `#if defined(_WIN32)`, which is never
    /// defined for our Linux-only target build, so the corresponding code
    /// path is an upstream no-op for this configuration.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_info(
        &self,
        painter: &mut emPainter,
        info_x_in: f64,
        info_y_in: f64,
        info_w_in: f64,
        info_h_in: f64,
        canvas_color: emColor,
        state: &PanelState,
    ) {
        // C++ label[6] (cpp:489-500). Linux-only: the second entry is the
        // POSIX permissions label (the Windows alternative
        // "File Attributes" is omitted; see DIVERGED note).
        let label: [&str; 6] = [
            "Type",
            "Permissions of Owner, Group and Others",
            "Owner",
            "Group",
            "Size in Bytes",
            "Time of Last Modification",
        ];

        let cfg = self.config.borrow();
        let theme = cfg.GetTheme();
        let theme_rec = theme.GetRec();
        let label_color = emColor::from_packed(theme_rec.LabelColor);
        let info_color = emColor::from_packed(theme_rec.InfoColor);
        let symlink_color = emColor::from_packed(theme_rec.SymLinkColor);

        // GetViewedWidth() in C++ returns the panel's viewed-width.
        // PanelState exposes this as `state.viewed_rect.w`.
        let viewed_width = state.viewed_rect.w;

        let mut info_x = info_x_in;
        let mut info_y = info_y_in;
        let mut info_w = info_w_in;
        let mut info_h = info_h_in;

        let mut bx = [0.0f64; 6];
        let mut by = [0.0f64; 6];
        let mut bw = [0.0f64; 6];
        let mut bh = [0.0f64; 6];
        let lh: f64;

        // C++: t = infoH / infoW; then three layout branches.
        let t = info_h / info_w;
        // C++ emDirEntryPanel.cpp:512 (tall threshold)
        if t > 0.9 {
            // Tall layout: cpp:512-529.
            // C++ emDirEntryPanel.cpp:513
            let mut th = info_w * 1.4;
            if info_h > th {
                // No alignment flags propagate here — emPanel paints at the
                // panel's own coordinates and the C++ alignment argument is
                // EM_ALIGN_CENTER by default. Match C++ "else if" branch
                // (vertical center) since neither TOP nor BOTTOM is set.
                //
                // C++ takes an `alignment` parameter (emDirEntryPanel.cpp:514-516)
                // and switches between top/bottom/center positioning based on
                // EM_ALIGN_TOP / EM_ALIGN_BOTTOM bits. This Rust port is
                // hardcoded to the center branch because the only caller
                // (`Paint`, see self.paint_info call near rs:1028) passes the
                // C++ default `EM_ALIGN_CENTER`. If a non-center caller is
                // ever added, thread an `alignment: emAlignment` parameter
                // through paint_info and restore the three-way switch here
                // and in the wide branch below.
                info_y += (info_h - th) * 0.5;
                info_h = th;
            }
            // C++ emDirEntryPanel.cpp:519: th = infoH/(7+(7-2)*0.087)
            th = info_h / (7.0 + (7.0 - 2.0) * 0.087);
            if th * viewed_width <= 1.15 {
                return;
            }
            // C++ emDirEntryPanel.cpp:521: spy = (infoH-7*th)/(7-2)
            let spy = (info_h - 7.0 * th) / (7.0 - 2.0);
            for i in 0..6 {
                bx[i] = info_x;
                by[i] = info_y + (i as f64) * (th + spy);
                bw[i] = info_w;
                bh[i] = th;
            }
            // C++ emDirEntryPanel.cpp:528: bh[5] *= 2  (Time row doubled)
            bh[5] *= 2.0;
            // C++ emDirEntryPanel.cpp:529: lh = th/7.6666
            lh = th / 7.6666;
        } else if t > 0.04 {
            // Medium layout: cpp:531-544.
            // C++ emDirEntryPanel.cpp:532: infoH *= 1.03 (timestamp adjust)
            info_h *= 1.03;
            // C++ emDirEntryPanel.cpp:533: th = infoH/(4+(4-1)*0.087)
            let th = info_h / (4.0 + (4.0 - 1.0) * 0.087);
            if th * viewed_width <= 1.15 {
                return;
            }
            let spy = (info_h - 4.0 * th) / (4.0 - 1.0);
            // C++ emDirEntryPanel.cpp:536: spx = th*0.483
            let spx = th * 0.483;
            let tw = (info_w - spx) / 2.0;
            // C++ emDirEntryPanel.cpp:538-543
            bx[0] = info_x;
            by[0] = info_y;
            bw[0] = info_w;
            bh[0] = th;
            bx[1] = info_x;
            by[1] = info_y + th + spy;
            bw[1] = tw;
            bh[1] = th;
            bx[2] = info_x;
            by[2] = info_y + 2.0 * (th + spy);
            bw[2] = tw;
            bh[2] = th;
            bx[3] = info_x + tw + spx;
            by[3] = info_y + 2.0 * (th + spy);
            bw[3] = tw;
            bh[3] = th;
            bx[4] = info_x + tw + spx;
            by[4] = info_y + th + spy;
            bw[4] = tw;
            bh[4] = th;
            bx[5] = info_x;
            by[5] = info_y + 3.0 * (th + spy);
            bw[5] = info_w;
            bh[5] = th;
            lh = th / 7.6666;
        } else {
            // Wide layout: cpp:546-562.
            if info_h * viewed_width <= 1.15 {
                return;
            }
            // C++ emDirEntryPanel.cpp:548: tw = infoH/0.025
            let mut tw = info_h / 0.025;
            if info_w > tw {
                // Same alignment fallback as tall branch (default center).
                //
                // C++ takes an `alignment` parameter (emDirEntryPanel.cpp:549-551)
                // and switches between left/right/center positioning based on
                // EM_ALIGN_LEFT / EM_ALIGN_RIGHT bits. This Rust port is
                // hardcoded to the center branch because the only caller
                // (`Paint`, see self.paint_info call near rs:1028) passes the
                // C++ default `EM_ALIGN_CENTER`. If a non-center caller is
                // ever added, thread an `alignment: emAlignment` parameter
                // through paint_info and restore the three-way switch.
                info_x += (info_w - tw) * 0.5;
                info_w = tw;
            }
            // C++ emDirEntryPanel.cpp:554: tw = infoW/(6+(6-1)*0.087)
            tw = info_w / (6.0 + (6.0 - 1.0) * 0.087);
            let spx = (info_w - 6.0 * tw) / (6.0 - 1.0);
            for i in 0..6 {
                bx[i] = info_x + (i as f64) * (tw + spx);
                by[i] = info_y;
                bw[i] = tw;
                bh[i] = info_h;
            }
            lh = info_h / 7.6666;
        }

        // C++ emDirEntryPanel.cpp:565-574: paint all six labels if visible.
        if lh * viewed_width > 1.0 {
            for i in 0..6 {
                painter.PaintTextBoxed(
                    bx[i],
                    by[i],
                    bw[i],
                    bh[i],
                    label[i],
                    lh,
                    label_color,
                    canvas_color,
                    TextAlignment::Left,
                    VAlign::Top,
                    TextAlignment::Left,
                    0.5,
                    true,
                    0.0,
                );
            }
        }

        // C++ emDirEntryPanel.cpp:576: shift fields below labels.
        for i in 0..6 {
            by[i] += lh;
            bh[i] -= lh;
        }

        // C++ emDirEntryPanel.cpp:578-586: select Type string.
        let stat = self.dir_entry.GetStat();
        let mode = stat.st_mode & libc::S_IFMT;
        let p: &str = if self.dir_entry.IsRegularFile() {
            "File"
        } else if self.dir_entry.IsDirectory() {
            "Directory"
        } else if mode == libc::S_IFIFO {
            "FIFO"
        } else if mode == libc::S_IFBLK {
            "Block Device"
        } else if mode == libc::S_IFCHR {
            "Char Device"
        } else if mode == libc::S_IFSOCK {
            "Socket"
        } else {
            "Unknown Type"
        };

        // C++ emDirEntryPanel.cpp:587-634: Type field paint (with symlink
        // sub-branch).
        if self.dir_entry.IsSymbolicLink() {
            // C++ emDirEntryPanel.cpp:588: "Symbolic Link to %s:"
            let header = format!("Symbolic Link to {}:", p);
            painter.PaintTextBoxed(
                bx[0],
                by[0],
                bw[0],
                bh[0] / 2.0,
                &header,
                bh[0] / 2.0,
                symlink_color,
                canvas_color,
                TextAlignment::Left,
                VAlign::Center,
                TextAlignment::Left,
                0.5,
                false,
                0.0,
            );
            // C++ emDirEntryPanel.cpp:596-601: target text or error string.
            let errno = self.dir_entry.GetTargetPathErrNo();
            let target_str: String = if errno != 0 {
                std::io::Error::from_raw_os_error(errno).to_string()
            } else {
                self.dir_entry.GetTargetPath().to_string()
            };
            painter.PaintTextBoxed(
                bx[0],
                by[0] + bh[0] / 2.0,
                bw[0],
                bh[0] / 2.0,
                &target_str,
                bh[0] / 2.0,
                symlink_color,
                canvas_color,
                TextAlignment::Left,
                VAlign::Center,
                TextAlignment::Left,
                0.5,
                false,
                0.0,
            );
        } else {
            painter.PaintTextBoxed(
                bx[0],
                by[0],
                bw[0],
                bh[0],
                p,
                bh[0],
                info_color,
                canvas_color,
                TextAlignment::Left,
                VAlign::Center,
                TextAlignment::Left,
                0.5,
                false,
                0.0,
            );
        }

        // C++ emDirEntryPanel.cpp:650-668: Permissions field — Unix branch.
        // (Windows branch cpp:642-648 omitted; see DIVERGED note above.)
        let cw1 = emPainter::GetTextSize("X", bh[1], false, 0.0).0;
        let mut ws = bw[1] / (cw1 * 10.0);
        if ws > 1.0 {
            ws = 1.0;
        }
        let st_mode = stat.st_mode;
        let perm_group = |r_bit, w_bit, x_bit| -> String {
            let mut s = String::with_capacity(3);
            s.push(if st_mode & r_bit != 0 { 'r' } else { '-' });
            s.push(if st_mode & w_bit != 0 { 'w' } else { '-' });
            s.push(if st_mode & x_bit != 0 { 'x' } else { '-' });
            s
        };
        let owner_perm = perm_group(libc::S_IRUSR, libc::S_IWUSR, libc::S_IXUSR);
        painter.PaintText(
            bx[1],
            by[1],
            &owner_perm,
            bh[1],
            ws,
            info_color,
            canvas_color,
        );
        let group_perm = perm_group(libc::S_IRGRP, libc::S_IWGRP, libc::S_IXGRP);
        painter.PaintText(
            bx[1] + cw1 * 3.5 * ws,
            by[1],
            &group_perm,
            bh[1],
            ws,
            info_color,
            canvas_color,
        );
        let other_perm = perm_group(libc::S_IROTH, libc::S_IWOTH, libc::S_IXOTH);
        painter.PaintText(
            bx[1] + cw1 * 7.0 * ws,
            by[1],
            &other_perm,
            bh[1],
            ws,
            info_color,
            canvas_color,
        );

        // C++ emDirEntryPanel.cpp:670-676: Owner.
        painter.PaintTextBoxed(
            bx[2],
            by[2],
            bw[2],
            bh[2],
            self.dir_entry.GetOwner(),
            bh[2],
            info_color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            0.0,
        );

        // C++ emDirEntryPanel.cpp:678-684: Group.
        painter.PaintTextBoxed(
            bx[3],
            by[3],
            bw[3],
            bh[3],
            self.dir_entry.GetGroup(),
            bh[3],
            info_color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            0.0,
        );

        // C++ emDirEntryPanel.cpp:689-706: Size with thousands separator and
        // magnitude suffix (k/M/G/T/P/E/Z/Y).
        let size_str = em_uint64_to_str(stat.st_size as u64);
        let size_bytes = size_str.as_bytes();
        let len = size_bytes.len() as i32;
        let cw4 = emPainter::GetTextSize("X", bh[4], false, 0.0).0;
        // C++ emDirEntryPanel.cpp:691: ws = bw[4]/(cw*len*16/15)
        let mut ws4 = bw[4] / (cw4 * (len as f64) * 16.0 / 15.0);
        if ws4 > 1.0 {
            ws4 = 1.0;
        }
        let mag = b"kMGTPEZY";
        let mut x = bx[4];
        let mut i: i32 = 0;
        while i < len {
            // C++ emDirEntryPanel.cpp:695: j = (len-i) - (len-i-1)/3*3
            let j = (len - i) - (len - i - 1) / 3 * 3;
            let chunk = std::str::from_utf8(&size_bytes[i as usize..(i + j) as usize])
                .expect("digits are ASCII");
            painter.PaintText(x, by[4], chunk, bh[4], ws4, info_color, canvas_color);
            x += cw4 * (j as f64) * ws4;
            // C++ emDirEntryPanel.cpp:698: k = (len-i-j)/3 - 1
            let k = (len - i - j) / 3 - 1;
            if k >= 0 {
                let suffix_byte = mag[k as usize];
                let suffix = std::str::from_utf8(std::slice::from_ref(&suffix_byte))
                    .expect("magnitude letters are ASCII");
                // C++ emDirEntryPanel.cpp:700-703: PaintText at
                // (x, by[4]+bh[4]*0.75) with charHeight bh[4]/5.
                painter.PaintText(
                    x,
                    by[4] + bh[4] * 0.75,
                    suffix,
                    bh[4] / 5.0,
                    ws4,
                    info_color,
                    canvas_color,
                );
            }
            // C++ emDirEntryPanel.cpp:705: x += cw/5*ws
            x += cw4 / 5.0 * ws4;
            i += j;
        }

        // C++ emDirEntryPanel.cpp:714-721: Time field.
        let nl = bw[5] / bh[5] < 6.0;
        let time_str = FormatTime(stat.st_mtime, nl);
        painter.PaintTextBoxed(
            bx[5],
            by[5],
            bw[5],
            bh[5],
            &time_str,
            bh[5],
            info_color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            true,
            0.0,
        );
    }
}

/// Port of C++ `emUInt64ToStr` (emStd1.cpp:200-214).
/// Returns a decimal-digit string (no thousands separators).
fn em_uint64_to_str(val: u64) -> String {
    if val == 0 {
        return "0".to_string();
    }
    let mut tmp = [0u8; 32];
    let mut l = 0usize;
    let mut v = val;
    while v != 0 {
        tmp[31 - l] = b'0' + ((v % 10) as u8);
        v /= 10;
        l += 1;
    }
    std::str::from_utf8(&tmp[32 - l..32])
        .expect("digits are ASCII")
        .to_string()
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

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        _w: f64,
        _h: f64,
        state: &PanelState,
    ) {
        let cfg = self.config.borrow();
        let theme = cfg.GetTheme();
        let theme_rec = theme.GetRec();
        let bg = emColor::from_packed(self.bg_color);

        // C++ emDirEntryPanel.cpp:283-301: PaintRoundRect(Background...).
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

        // C++ emDirEntryPanel.cpp:303-335: outer-border canvasColor selection
        // and PaintBorderImage(OuterBorder...).
        //
        // C++ uses local `canvasColor` initialized to the parent's canvas color
        // (the painter's current canvas color). Then: if BgColor==canvasColor
        // (the rounded-rect background didn't change pixels — parent canvas
        // already matches bg), or if the outer-border rect lies strictly inside
        // the rounded-rect inner area (so border draws over actual BgColor
        // pixels), set canvasColor=BgColor; else canvasColor=0.
        let parent_canvas = canvas_color;
        let canvas_color = if parent_canvas == bg
            || (theme_rec.OuterBorderX >= theme_rec.BackgroundX + theme_rec.BackgroundRX * 0.3
                && theme_rec.OuterBorderY >= theme_rec.BackgroundY + theme_rec.BackgroundRY * 0.3
                && theme_rec.OuterBorderW
                    <= theme_rec.BackgroundX + theme_rec.BackgroundW
                        - theme_rec.BackgroundRX * 0.3
                        - theme_rec.OuterBorderX
                && theme_rec.OuterBorderH
                    <= theme_rec.BackgroundY + theme_rec.BackgroundH
                        - theme_rec.BackgroundRY * 0.3
                        - theme_rec.OuterBorderY)
        {
            bg
        } else {
            emColor::TRANSPARENT
        };
        // Outer border: borrow image via theme accessor (returns Ref<emImage>).
        {
            let img = theme.GetOuterBorderImage();
            painter.PaintBorderImage(
                theme_rec.OuterBorderX,
                theme_rec.OuterBorderY,
                theme_rec.OuterBorderW,
                theme_rec.OuterBorderH,
                theme_rec.OuterBorderL,
                theme_rec.OuterBorderT,
                theme_rec.OuterBorderR,
                theme_rec.OuterBorderB,
                &img,
                theme_rec.OuterBorderImgL,
                theme_rec.OuterBorderImgT,
                theme_rec.OuterBorderImgR,
                theme_rec.OuterBorderImgB,
                255,
                canvas_color,
                emcore::emPainter::BORDER_EDGES_ONLY,
            );
        }

        // C++ line 337: canvasColor=BgColor — restored before name paint.
        let canvas_color = bg;

        // C++ lines 339-353: name color based on file type.
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

        // C++ lines 362-375: PaintTextBoxed(Name...).
        let name = self.dir_entry.GetName();
        painter.PaintTextBoxed(
            theme_rec.NameX,
            theme_rec.NameY,
            theme_rec.NameW,
            theme_rec.NameH,
            name,
            theme_rec.NameH,
            name_color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            false,
            1.0,
        );

        // C++ lines 377-385: PaintInfo(InfoX, InfoY, InfoW, InfoH, ...).
        // Inlined call to the body of emDirEntryPanel::PaintInfo
        // (emDirEntryPanel.cpp:484-725).
        self.paint_info(
            painter,
            theme_rec.InfoX,
            theme_rec.InfoY,
            theme_rec.InfoW,
            theme_rec.InfoH,
            canvas_color,
            state,
        );

        // C++ lines 387-466: path text, inner border (Dir or File), and
        // content-rect background, gated on content visibility.
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
                canvas_color,
                TextAlignment::Left,
                VAlign::Center,
                TextAlignment::Left,
                0.5,
                false,
                1.0,
            );

            if self.dir_entry.IsDirectory() {
                // C++ lines 404-419: PaintBorderImage(DirInnerBorder...).
                {
                    let img = theme.GetDirInnerBorderImage();
                    painter.PaintBorderImage(
                        theme_rec.DirInnerBorderX,
                        theme_rec.DirInnerBorderY,
                        theme_rec.DirInnerBorderW,
                        theme_rec.DirInnerBorderH,
                        theme_rec.DirInnerBorderL,
                        theme_rec.DirInnerBorderT,
                        theme_rec.DirInnerBorderR,
                        theme_rec.DirInnerBorderB,
                        &img,
                        theme_rec.DirInnerBorderImgL,
                        theme_rec.DirInnerBorderImgT,
                        theme_rec.DirInnerBorderImgR,
                        theme_rec.DirInnerBorderImgB,
                        255,
                        canvas_color,
                        emcore::emPainter::BORDER_EDGES_ONLY,
                    );
                }
                // C++ lines 420-427: PaintRect(DirContent...).
                painter.PaintRect(
                    theme_rec.DirContentX,
                    theme_rec.DirContentY,
                    theme_rec.DirContentW,
                    theme_rec.DirContentH,
                    emColor::from_packed(theme_rec.DirContentColor),
                    canvas_color,
                );
            } else {
                // C++ lines 430-445: PaintBorderImage(FileInnerBorder...).
                {
                    let img = theme.GetFileInnerBorderImage();
                    painter.PaintBorderImage(
                        theme_rec.FileInnerBorderX,
                        theme_rec.FileInnerBorderY,
                        theme_rec.FileInnerBorderW,
                        theme_rec.FileInnerBorderH,
                        theme_rec.FileInnerBorderL,
                        theme_rec.FileInnerBorderT,
                        theme_rec.FileInnerBorderR,
                        theme_rec.FileInnerBorderB,
                        &img,
                        theme_rec.FileInnerBorderImgL,
                        theme_rec.FileInnerBorderImgT,
                        theme_rec.FileInnerBorderImgR,
                        theme_rec.FileInnerBorderImgB,
                        255,
                        canvas_color,
                        emcore::emPainter::BORDER_EDGES_ONLY,
                    );
                }
                // C++ lines 446-457: containment check — if content rect
                // extends beyond inner-border inner edges, force canvas=0.
                let content_canvas = if theme_rec.FileContentX + 1e-10
                    < theme_rec.FileInnerBorderX + theme_rec.FileInnerBorderL
                    || theme_rec.FileContentY + 1e-10
                        < theme_rec.FileInnerBorderY + theme_rec.FileInnerBorderT
                    || theme_rec.FileContentX + theme_rec.FileContentW - 1e-10
                        > theme_rec.FileInnerBorderX + theme_rec.FileInnerBorderW
                            - theme_rec.FileInnerBorderR
                    || theme_rec.FileContentY + theme_rec.FileContentH - 1e-10
                        > theme_rec.FileInnerBorderY + theme_rec.FileInnerBorderH
                            - theme_rec.FileInnerBorderB
                {
                    emColor::TRANSPARENT
                } else {
                    canvas_color
                };
                // C++ lines 458-465: PaintRect(FileContent...).
                painter.PaintRect(
                    theme_rec.FileContentX,
                    theme_rec.FileContentY,
                    theme_rec.FileContentW,
                    theme_rec.FileContentH,
                    emColor::from_packed(theme_rec.FileContentColor),
                    content_canvas,
                );
            }
        }
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

    /// F010 Phase 2 (HYPOTHESIS Y): emDirEntryPanel::Paint must call
    /// `painter.PaintBorderImage` for the outer border. Mirrors C++
    /// emDirEntryPanel.cpp:318-335 (unconditional outer-border paint).
    ///
    /// Uses the painter's op-log callback to record DrawOp variants and asserts
    /// at least one `PaintBorderImage` op was emitted.
    #[test]
    fn paint_emits_outer_border_image() {
        use emcore::emImage::emImage;
        use emcore::emPainter::emPainter;
        use emcore::emPainterDrawList::DrawOp;
        use emcore::emPanel::PanelState;

        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let mut panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        let mut img = emImage::new(64, 64, 4);
        img.fill(emColor::BLACK);

        // Shared op-counter; the closure must be 'static so we use Rc<Cell<_>>.
        let border_image_count = Rc::new(std::cell::Cell::new(0u32));
        let total_ops = Rc::new(std::cell::Cell::new(0u32));
        {
            let bic = Rc::clone(&border_image_count);
            let tot = Rc::clone(&total_ops);
            let mut p = emPainter::new(&mut img);
            p.SetCanvasColor(emColor::TRANSPARENT);
            p.set_op_log(move |op, _depth, _state| {
                tot.set(tot.get() + 1);
                if matches!(op, DrawOp::PaintBorderImage { .. }) {
                    bic.set(bic.get() + 1);
                }
            });
            let state = PanelState::default_for_test();
            panel.Paint(&mut p, emColor::TRANSPARENT, 1.0, 1.0, &state);
        }

        // Confirm the recorder fired at all (sanity).
        assert!(
            total_ops.get() > 0,
            "op-log callback must observe at least one DrawOp",
        );
        // Phase 2 acceptance: PaintBorderImage emitted at least once
        // (outer border is unconditional in C++).
        assert!(
            border_image_count.get() >= 1,
            "Paint must emit at least one PaintBorderImage (outer border); got {}",
            border_image_count.get(),
        );
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

    /// Helper: create a real regular file under /tmp and return an
    /// emDirEntry for it. The caller's test fixture path is unique per
    /// test so concurrent runs don't collide.
    fn make_regular_file_entry(fixture: &str) -> crate::emDirEntry::emDirEntry {
        let path = std::env::temp_dir().join(fixture);
        // Ignore errors — the previous test run may have left no file.
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, b"hello world content").expect("write fixture");
        let s = path.to_string_lossy().to_string();
        let entry = crate::emDirEntry::emDirEntry::from_path(&s);
        // Sanity: the fixture should have loaded as a regular file.
        assert!(
            entry.IsRegularFile(),
            "fixture at {s} is not a regular file (got stat_errno={})",
            entry.GetStatErrNo()
        );
        entry
    }

    /// Helper: build a recording emPainter, install an op-log that pushes
    /// each DrawOp variant tag onto a Vec, and run `f` against the painter.
    /// Returns the recorded variant tags so callers can count by `kind`.
    fn collect_paint_info_ops<F>(
        panel: &emDirEntryPanel,
        info: (f64, f64, f64, f64),
        f: F,
    ) -> Vec<&'static str>
    where
        F: FnOnce(&emDirEntryPanel, &mut emcore::emPainter::emPainter, (f64, f64, f64, f64)),
    {
        use emcore::emImage::emImage;
        use emcore::emPainter::emPainter;
        use emcore::emPainterDrawList::DrawOp;

        let mut img = emImage::new(64, 64, 4);
        img.fill(emColor::BLACK);

        let kinds: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        {
            let kinds_cb = Rc::clone(&kinds);
            let mut p = emPainter::new(&mut img);
            p.SetCanvasColor(emColor::TRANSPARENT);
            p.set_op_log(move |op, _depth, _state| {
                let tag: &'static str = match op {
                    DrawOp::PaintTextBoxed { .. } => "PaintTextBoxed",
                    DrawOp::PaintText { .. } => "PaintText",
                    _ => "Other",
                };
                kinds_cb.borrow_mut().push(tag);
            });
            f(panel, &mut p, info);
        }
        let v = kinds.borrow().clone();
        v
    }

    /// F010 Phase 3 verification item 3a: tall layout regime
    /// (t = info_h / info_w > 0.9). C++ ref: emDirEntryPanel.cpp:512-529.
    /// Asserts paint_info emits at least 12 text-class ops:
    ///   6 labels (PaintTextBoxed) when lh*viewed_width > 1.0,
    ///   plus Type (1 PaintTextBoxed), Permissions (3 PaintText),
    ///   Owner (1 PaintTextBoxed), Group (1 PaintTextBoxed),
    ///   Size loop (>=1 PaintText), Time (1 PaintTextBoxed).
    #[test]
    fn paint_info_tall_layout_emits_text_ops() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = make_regular_file_entry("emfileman_paint_info_tall.txt");
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        // info_h/info_w = 200/100 = 2.0 → tall branch.
        let info = (0.0f64, 0.0f64, 100.0f64, 200.0f64);
        let kinds = collect_paint_info_ops(&panel, info, |panel, p, (x, y, w, h)| {
            let state = PanelState::default_for_test();
            // viewed_rect default = (0,0,200,100); width=200 keeps
            // lh*viewed_width well above the 1.0 label gate.
            panel.paint_info(p, x, y, w, h, emColor::TRANSPARENT, &state);
        });

        let text_boxed = kinds.iter().filter(|&&k| k == "PaintTextBoxed").count();
        let text = kinds.iter().filter(|&&k| k == "PaintText").count();
        let total_text = text_boxed + text;
        assert!(
            total_text >= 12,
            "tall layout: expected >=12 text ops (6 labels + Type + 3 perm + Owner + Group + Size + Time), got {total_text} (PaintTextBoxed={text_boxed}, PaintText={text})"
        );
        // Labels (6) + Type/Owner/Group/Time (4) → at least 10 PaintTextBoxed
        // for a non-symlink regular file.
        assert!(
            text_boxed >= 10,
            "tall layout: expected >=10 PaintTextBoxed (6 labels + 4 boxed fields), got {text_boxed}"
        );
        // Permissions emit 3 PaintText, plus >=1 from the Size loop.
        assert!(
            text >= 4,
            "tall layout: expected >=4 PaintText (3 perm + Size), got {text}"
        );
    }

    /// F010 Phase 3 verification item 3b: medium layout regime
    /// (0.04 < t ≤ 0.9). C++ ref: emDirEntryPanel.cpp:531-544.
    #[test]
    fn paint_info_medium_layout_emits_text_ops() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = make_regular_file_entry("emfileman_paint_info_medium.txt");
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        // info_h/info_w = 100/200 = 0.5 → medium branch.
        let info = (0.0f64, 0.0f64, 200.0f64, 100.0f64);
        let kinds = collect_paint_info_ops(&panel, info, |panel, p, (x, y, w, h)| {
            let state = PanelState::default_for_test();
            panel.paint_info(p, x, y, w, h, emColor::TRANSPARENT, &state);
        });

        let text_boxed = kinds.iter().filter(|&&k| k == "PaintTextBoxed").count();
        let text = kinds.iter().filter(|&&k| k == "PaintText").count();
        let total_text = text_boxed + text;
        assert!(
            total_text >= 12,
            "medium layout: expected >=12 text ops, got {total_text} (PaintTextBoxed={text_boxed}, PaintText={text})"
        );
        assert!(
            text_boxed >= 10,
            "medium layout: expected >=10 PaintTextBoxed, got {text_boxed}"
        );
        assert!(
            text >= 4,
            "medium layout: expected >=4 PaintText, got {text}"
        );
    }

    /// F010 Phase 3 verification item 3c: wide layout regime
    /// (t ≤ 0.04). C++ ref: emDirEntryPanel.cpp:546-562.
    #[test]
    fn paint_info_wide_layout_emits_text_ops() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = make_regular_file_entry("emfileman_paint_info_wide.txt");
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        // info_h/info_w = 20/1000 = 0.02 → wide branch.
        let info = (0.0f64, 0.0f64, 1000.0f64, 20.0f64);
        let kinds = collect_paint_info_ops(&panel, info, |panel, p, (x, y, w, h)| {
            // Need a viewed_rect.w large enough to satisfy info_h*viewed_width > 1.15
            // (20 * 200 = 4000, fine) and lh*viewed_width > 1.0
            // (lh = 20/7.6666 ≈ 2.6; 2.6 * 200 = 520, fine).
            let state = PanelState::default_for_test();
            panel.paint_info(p, x, y, w, h, emColor::TRANSPARENT, &state);
        });

        let text_boxed = kinds.iter().filter(|&&k| k == "PaintTextBoxed").count();
        let text = kinds.iter().filter(|&&k| k == "PaintText").count();
        let total_text = text_boxed + text;
        assert!(
            total_text >= 12,
            "wide layout: expected >=12 text ops, got {total_text} (PaintTextBoxed={text_boxed}, PaintText={text})"
        );
        assert!(
            text_boxed >= 10,
            "wide layout: expected >=10 PaintTextBoxed, got {text_boxed}"
        );
        assert!(text >= 4, "wide layout: expected >=4 PaintText, got {text}");
    }

    /// F010 Phase 3 verification item 4: field content matches expected
    /// strings for Type / Owner / Group / Size / Time. Permissions
    /// rendering is verified by the cross-language diff tool, not here
    /// (the plan defers that). C++ refs: emDirEntryPanel.cpp:582 (Type),
    /// 670-684 (Owner/Group), 689-706 (Size), 714-721 (Time).
    #[test]
    fn paint_info_field_content() {
        use emcore::emImage::emImage;
        use emcore::emPainter::emPainter;
        use emcore::emPainterDrawList::DrawOp;

        let ctx = emcore::emContext::emContext::NewRoot();
        let entry = make_regular_file_entry("emfileman_paint_info_content.txt");
        // Capture stat for assertion comparisons.
        let owner_expected = entry.GetOwner().to_string();
        let group_expected = entry.GetGroup().to_string();
        let stat = *entry.GetStat();
        let panel = emDirEntryPanel::new(Rc::clone(&ctx), entry);

        let mut img = emImage::new(64, 64, 4);
        img.fill(emColor::BLACK);
        let texts: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        {
            let texts_cb = Rc::clone(&texts);
            let mut p = emPainter::new(&mut img);
            p.SetCanvasColor(emColor::TRANSPARENT);
            p.set_op_log(move |op, _depth, _state| match op {
                DrawOp::PaintText { text, .. } => texts_cb.borrow_mut().push(text.clone()),
                DrawOp::PaintTextBoxed { text, .. } => texts_cb.borrow_mut().push(text.clone()),
                _ => {}
            });
            // Use the medium layout: largest set of guaranteed labels and
            // values without timestamp-row doubling complications.
            let state = PanelState::default_for_test();
            panel.paint_info(&mut p, 0.0, 0.0, 200.0, 100.0, emColor::TRANSPARENT, &state);
        }
        let texts = texts.borrow().clone();

        // Type label (cpp:489) and Type value "File" (cpp:579-583,
        // non-symlink branch cpp:660-674).
        assert!(
            texts.iter().any(|t| t == "Type"),
            "missing 'Type' label, got: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t == "File"),
            "missing 'File' Type value, got: {texts:?}"
        );

        // Owner / Group: must match what emDirEntry loaded from the OS.
        // Owner string is paint at cpp:670-676.
        if !owner_expected.is_empty() {
            assert!(
                texts.iter().any(|t| t == &owner_expected),
                "missing owner '{owner_expected}', got: {texts:?}"
            );
        }
        if !group_expected.is_empty() {
            assert!(
                texts.iter().any(|t| t == &group_expected),
                "missing group '{group_expected}', got: {texts:?}"
            );
        }

        // Size: digits-only chunk(s) (no thousands separator inserted by
        // em_uint64_to_str — chunking is a paint-time visual layout, not a
        // string transform). For a "hello world content" file (19 bytes),
        // the entire size string is "19".
        let size_str = format!("{}", stat.st_size);
        assert!(
            texts.iter().any(|t| t == &size_str),
            "missing size '{size_str}', got: {texts:?}"
        );

        // Time: FormatTime of stat.st_mtime. The medium layout's Time row
        // has bw[5]/bh[5] = info_w/th — inspect the painted Time string and
        // verify it matches one of the two FormatTime variants.
        let time_no_nl = FormatTime(stat.st_mtime, false);
        let time_nl = FormatTime(stat.st_mtime, true);
        assert!(
            texts.iter().any(|t| t == &time_no_nl || t == &time_nl),
            "missing Time field; expected one of '{time_no_nl}' or '{time_nl}', got: {texts:?}"
        );
    }
}
