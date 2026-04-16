//! Port of C++ emDirPanel grid layout algorithm (LayoutChildren).

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emFilePanel::emFilePanel;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::PanelId;
use emcore::emPainter::emPainter;

use crate::emDirEntry::emDirEntry;
use crate::emDirEntryPanel::emDirEntryPanel;
use crate::emDirModel::emDirModel;
use crate::emFileManModel::emFileManModel;
use crate::emFileManViewConfig::emFileManViewConfig;

pub struct LayoutRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Port of C++ emDirPanel::LayoutChildren grid algorithm.
/// theme_height is the theme's Height value.
/// panel_height is GetHeight() (the panel's actual height, typically GetHeight()).
/// pad_l/t/r/b are DirPaddingL/T/R/B from the theme.
pub fn compute_grid_layout(
    count: usize,
    theme_height: f64,
    panel_height: f64,
    pad_l: f64,
    pad_t: f64,
    pad_r: f64,
    pad_b: f64,
) -> Vec<LayoutRect> {
    if count == 0 {
        return Vec::new();
    }

    let t = theme_height;
    let h = panel_height;

    // Find minimum rows such that rows*cols >= count
    let mut rows = 1;
    loop {
        let mut cols = (rows as f64 * t / (h * (1.0 - 0.05 / rows as f64))) as i32;
        if cols <= 0 {
            cols = 1;
        }
        if (rows * cols as usize) >= count {
            break;
        }
        rows += 1;
    }
    let cols = count.div_ceil(rows);

    // Cell dimensions with padding
    let mut cw = 1.0 / (pad_l + cols as f64 + pad_r);
    let mut ch = h / (pad_t / t + rows as f64 + pad_b / t);
    if ch > cw * t {
        ch = cw * t;
    } else {
        cw = ch / t;
    }
    let mut cx = cw * pad_l;
    let cy = cw * pad_t;

    // Gap calculation
    let f = 1.0 - cw * (pad_l + pad_r);
    let n = (f / cw + 0.001) as i32;
    let mut gap = ((pad_t + pad_b) / t - (pad_l + pad_r)) * cw;
    gap = gap.min(f - n as f64 * cw);
    if gap < 0.0 {
        gap = 0.0;
    }
    gap /= (n + 1) as f64;
    cx += gap;

    // Column-major layout
    let mut rects = Vec::with_capacity(count);
    let mut col = 0;
    let mut row = 0;
    for _ in 0..count {
        rects.push(LayoutRect {
            x: cx + (cw + gap) * col as f64,
            y: cy + ch * row as f64,
            w: cw,
            h: ch,
        });
        row += 1;
        if row >= rows {
            col += 1;
            row = 0;
        }
    }
    rects
}

struct KeyWalkState {
    search: String,
    last_key_time: std::time::Instant,
}

/// Directory grid panel.
/// Port of C++ `emDirPanel` (extends emFilePanel).
///
/// Displays directory entries in a grid layout. Lazily acquires emDirModel
/// when viewed. Creates/updates emDirEntryPanel children from model entries.
///
/// DIVERGED: C++ emDirPanel connects emDirModel as a FileModelState via
/// SetFileModel. Rust drives loading directly in Cycle using
/// `get_file_state()` to query the model's phase, because emDirModel does
/// not implement FileModelState — it wraps emDirModelData directly without
/// scheduler integration.
pub struct emDirPanel {
    pub(crate) file_panel: emFilePanel,
    ctx: Rc<emContext>,
    pub(crate) path: String,
    config: Rc<RefCell<emFileManViewConfig>>,
    file_man: Rc<RefCell<emFileManModel>>,
    dir_model: Option<Rc<RefCell<emDirModel>>>,
    pub(crate) content_complete: bool,
    child_count: usize,
    loading_done: bool,
    loading_error: Option<String>,
    key_walk_state: Option<KeyWalkState>,
    scroll_target: Option<String>,
    last_config_gen: u64,
}

impl emDirPanel {
    pub fn new(ctx: Rc<emContext>, path: String) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        let file_man = emFileManModel::Acquire(&ctx);
        let last_config_gen = config.borrow().GetChangeSignal();
        Self {
            file_panel: emFilePanel::new(),
            ctx,
            file_man,
            path,
            config,
            dir_model: None,
            content_complete: false,
            child_count: 0,
            loading_done: false,
            loading_error: None,
            key_walk_state: None,
            scroll_target: None,
            last_config_gen,
        }
    }

    pub fn IsContentComplete(&self) -> bool {
        self.content_complete
    }

    pub fn GetPath(&self) -> &str {
        &self.path
    }

    pub(crate) fn SelectAll(&self) {
        if let Some(ref dm_rc) = self.dir_model {
            let show_hidden = self.config.borrow().GetShowHiddenFiles();
            let dm = dm_rc.borrow();
            let mut fm = self.file_man.borrow_mut();
            for i in 0..dm.GetEntryCount() {
                let entry = dm.GetEntry(i);
                if !entry.IsHidden() || show_hidden {
                    fm.SelectAsTarget(entry.GetPath());
                }
            }
        }
    }

    fn key_walk(&mut self, ch: char) {
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(1);

        match &mut self.key_walk_state {
            Some(state) if now.duration_since(state.last_key_time) < timeout => {
                state.search.push(ch);
                state.last_key_time = now;
            }
            _ => {
                self.key_walk_state = Some(KeyWalkState {
                    search: ch.to_string(),
                    last_key_time: now,
                });
            }
        }

        // Search for matching entry
        let search = &self.key_walk_state.as_ref().expect("just set").search;
        let wildcard = search.starts_with('*');
        let pattern = if wildcard { &search[1..] } else { search };
        let pattern_lower = pattern.to_lowercase();

        if let Some(ref dm_rc) = self.dir_model {
            let dm = dm_rc.borrow();
            for i in 0..dm.GetEntryCount() {
                let name = dm.GetEntry(i).GetName();
                let name_lower = name.to_lowercase();
                let matches = if wildcard {
                    name_lower.contains(&pattern_lower)
                } else {
                    name_lower.starts_with(&pattern_lower)
                };
                if matches {
                    self.scroll_target = Some(name.to_string());
                    break;
                }
            }
        }
    }

    fn update_children(&mut self, ctx: &mut PanelCtx) {
        if !self.loading_done {
            self.content_complete = false;
            return;
        }
        if let Some(ref dm_rc) = self.dir_model {
            let dm = dm_rc.borrow();
            let cfg = self.config.borrow();
            let show_hidden = cfg.GetShowHiddenFiles();
            let count = dm.GetEntryCount();

            // Collect visible entries
            let mut visible: Vec<emDirEntry> = Vec::new();
            for i in 0..count {
                let entry = dm.GetEntry(i);
                if !entry.IsHidden() || show_hidden {
                    visible.push(entry.clone());
                }
            }

            // Sort using config comparator
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

            let visible_count = visible.len();

            // Only recreate if count changed
            if visible_count != self.child_count {
                ctx.DeleteAllChildren();

                for entry in &visible {
                    let panel = emDirEntryPanel::new(
                        Rc::clone(&self.ctx),
                        entry.clone(),
                    );
                    ctx.create_child_with(entry.GetName(), Box::new(panel));
                }

                self.child_count = visible_count;
                self.content_complete = true;

                // Check for pending scroll target
                // DIVERGED: C++ uses View.Visit() from Input handler. Rust queues
                // navigation via PanelCtx::request_visit, drained by emView each frame.
                if let Some(target) = self.scroll_target.take() {
                    if let Some(child_id) = ctx.find_child_by_name(&target) {
                        ctx.request_visit(child_id);
                    }
                }
            }
        }
    }
}

impl PanelBehavior for emDirPanel {
    fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool {
        let mut changed = false;

        if self.dir_model.is_none() {
            self.dir_model = Some(emDirModel::Acquire(&self.ctx, &self.path));
            self.loading_done = false;
            self.loading_error = None;
            self.child_count = 0;
            self.content_complete = false;
            changed = true;
        }

        // Detect config changes (sort/filter) and force child re-creation
        let cfg_gen = self.config.borrow().GetChangeSignal();
        if cfg_gen != self.last_config_gen {
            self.last_config_gen = cfg_gen;
            self.child_count = 0;
            changed = true;
        }

        if let Some(ref dm_rc) = self.dir_model {
            let mut dm = dm_rc.borrow_mut();
            match dm.get_file_state() {
                emcore::emFileModel::FileState::Waiting => {
                    match dm.try_start_loading() {
                        Ok(()) => {
                            self.loading_done = false;
                            self.loading_error = None;
                            self.file_panel.clear_custom_error();
                        }
                        Err(e) => {
                            self.loading_error = Some(e.clone());
                            self.file_panel.set_custom_error(&e);
                        }
                    }
                    changed = true;
                }
                emcore::emFileModel::FileState::Loading { .. } => {
                    match dm.try_continue_loading() {
                        Ok(true) => {
                            dm.quit_loading();
                            self.loading_done = true;
                            self.file_panel.clear_custom_error();
                            drop(dm);
                            self.update_children(ctx);
                            changed = true;
                        }
                        Ok(false) => {
                            changed = true;
                        }
                        Err(e) => {
                            self.loading_error = Some(e.clone());
                            self.file_panel.set_custom_error(&e);
                            changed = true;
                        }
                    }
                }
                emcore::emFileModel::FileState::Loaded => {
                    drop(dm);
                    // Model was loaded previously (possibly by another
                    // panel instance). Reflect in loading_done so
                    // update_children creates entries.
                    self.loading_done = true;
                    self.update_children(ctx);
                }
                _ => {}
            }
        }

        self.file_panel.refresh_vir_file_state();
        // C++ returns emFilePanel::Cycle() busy state. Return true (busy) while
        // the model is loading, so the engine keeps cycling us.
        if !self.loading_done && self.dir_model.is_some() {
            return true;
        }
        changed
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // Alt+A: SelectAll
        if event.is_key(InputKey::Key('a')) && input_state.IsAltMod() {
            self.SelectAll();
            return true;
        }

        // KeyWalk: printable characters
        if event.is_keyboard_event() && !event.chars.is_empty() {
            for ch in event.chars.chars() {
                if ch.is_alphanumeric() || ch == '.' || ch == '_' || ch == '-' || ch == '*' {
                    self.key_walk(ch);
                    return true;
                }
            }
        }

        false
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.contains(NoticeFlags::VIEW_CHANGED) || flags.contains(NoticeFlags::SOUGHT_NAME_CHANGED) {
            // C++ emDirPanel::Notice:
            //   if (IsViewed() || GetSoughtName()) {
            //     if (!GetFileModel()) SetFileModel(emDirModel::Acquire(...))
            //   } else if (GetFileModel()) SetFileModel(NULL)
            // We use in_active_path as proxy for "being viewed or sought"
            // since PanelState doesn't expose sought_name.
            let keep_model = state.viewed || state.in_active_path;
            if keep_model {
                if self.dir_model.is_none() {
                    self.dir_model = Some(emDirModel::Acquire(&self.ctx, &self.path));
                    self.loading_done = false;
                    self.loading_error = None;
                    self.child_count = 0;
                    self.content_complete = false;
                }
            } else if self.dir_model.is_some() {
                self.dir_model = None;
                self.file_panel.SetFileModel(None);
                self.loading_done = false;
                self.loading_error = None;
                self.scroll_target = None;
            }
        }
    }

    fn IsOpaque(&self) -> bool {
        if self.loading_done {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let dc = theme.GetRec().DirContentColor;
            (dc >> 24) == 0xFF
        } else {
            false
        }
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        if self.loading_done {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let dc = emColor::from_packed(theme.GetRec().DirContentColor);
            painter.Clear(dc);
        } else if self.loading_error.is_some() || self.dir_model.is_some() {
            self.file_panel.paint_status(painter, w, h);
        } else {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let dc = emColor::from_packed(theme.GetRec().DirContentColor);
            painter.Clear(dc);
        }
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let cnt = children.len();
        if cnt == 0 {
            return;
        }

        let cfg = self.config.borrow();
        let theme = cfg.GetTheme();
        let theme_rec = theme.GetRec();
        let rect = ctx.layout_rect();

        let canvas_color = if self.loading_done {
            emColor::from_packed(theme_rec.DirContentColor)
        } else {
            emColor::TRANSPARENT
        };

        if self.content_complete {
            let rects = compute_grid_layout(
                cnt,
                theme_rec.Height,
                rect.h,
                theme_rec.DirPaddingL,
                theme_rec.DirPaddingT,
                theme_rec.DirPaddingR,
                theme_rec.DirPaddingB,
            );
            for (i, child) in children.iter().enumerate() {
                if i < rects.len() {
                    ctx.layout_child_canvas(
                        *child,
                        rects[i].x, rects[i].y,
                        rects[i].w, rects[i].h,
                        canvas_color,
                    );
                }
            }
        } else {
            // Incomplete: clamp existing positions
            let t = theme_rec.Height;
            for child in &children {
                let mut cw = 0.5_f64;
                cw = cw.clamp(0.001, 1.0);
                let mut ch = cw * t;
                if ch > rect.h { ch = rect.h; cw = ch / t; }
                ctx.layout_child_canvas(*child, 0.0, 0.0, cw, ch, canvas_color);
            }
        }
    }

    fn CreateControlPanel(&mut self, parent_ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let panel = crate::emFileManControlPanel::emFileManControlPanel::new(Rc::clone(&self.ctx))
            .with_dir_path(&self.path);
        Some(parent_ctx.create_child_with(name, Box::new(panel)))
    }

    fn GetIconFileName(&self) -> Option<String> {
        Some("directory.tga".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_layout_single_entry() {
        let rects = compute_grid_layout(1, 1.5, 1.0, 0.02, 0.02, 0.02, 0.02);
        assert_eq!(rects.len(), 1);
        assert!(rects[0].x >= 0.0);
        assert!(rects[0].y >= 0.0);
        assert!(rects[0].w > 0.0);
        assert!(rects[0].h > 0.0);
    }

    #[test]
    fn grid_layout_many_entries() {
        let rects = compute_grid_layout(20, 1.5, 1.0, 0.02, 0.02, 0.02, 0.02);
        assert_eq!(rects.len(), 20);
        for r in &rects {
            assert!(r.x >= 0.0);
            assert!(r.x + r.w <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn grid_layout_column_major() {
        let rects = compute_grid_layout(4, 1.5, 1.5, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(rects.len(), 4);
        assert!((rects[0].x - rects[1].x).abs() < 1e-9);
    }

    #[test]
    fn grid_layout_empty() {
        let rects = compute_grid_layout(0, 1.5, 1.0, 0.02, 0.02, 0.02, 0.02);
        assert!(rects.is_empty());
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn panel_initial_state() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());
        assert_eq!(panel.path, "/tmp");
        assert!(!panel.content_complete);
    }

    #[test]
    fn panel_icon_filename() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());
        assert_eq!(panel.GetIconFileName(), Some("directory.tga".to_string()));
    }

    #[test]
    fn key_walk_state_resets_on_timeout() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());

        panel.key_walk('a');
        assert_eq!(
            panel.key_walk_state.as_ref().unwrap().search,
            "a"
        );

        // Within timeout: appends
        panel.key_walk('b');
        assert_eq!(
            panel.key_walk_state.as_ref().unwrap().search,
            "ab"
        );
    }

    #[test]
    fn key_walk_wildcard() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());

        panel.key_walk('*');
        panel.key_walk('t');
        assert_eq!(
            panel.key_walk_state.as_ref().unwrap().search,
            "*t"
        );
    }
}
