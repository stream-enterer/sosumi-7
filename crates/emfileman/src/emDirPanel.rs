//! Port of C++ emDirPanel grid layout algorithm (LayoutChildren).

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::PanelCtx;
use emcore::emFileModel::FileModelState;
use emcore::emFilePanel::emFilePanel;
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPainter::emPainter;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;

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
/// DIVERGED: (language-forced) C++ emDirPanel connects emDirModel as a FileModelState via
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
                    let panel = emDirEntryPanel::new(Rc::clone(&self.ctx), entry.clone());
                    ctx.create_child_with(entry.GetName(), Box::new(panel));
                }

                self.child_count = visible_count;
                self.content_complete = true;

                // Check for pending scroll target
                // DIVERGED: (language-forced) C++ uses View.Visit() from Input handler. Rust queues
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
    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        ctx: &mut PanelCtx,
    ) -> bool {
        let mut changed = false;

        if self.dir_model.is_none() {
            // Port of C++ emDirPanel::Notice SetFileModel path (Cycle fallback):
            // acquire model and connect it to file_panel so VirtualFileState
            // transitions from NoFileModel → Waiting → Loading → Loaded.
            let dm_rc = emDirModel::Acquire(&self.ctx, &self.path);
            self.file_panel.SetFileModel(Some(
                Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>,
            ));
            self.dir_model = Some(dm_rc);
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
                            dm.state = emcore::emFileModel::FileState::Loading { progress: 0.0 };
                            // drop dm before accessing file_panel: both share the same
                            // RefCell<emDirModel>, so holding borrow_mut() while
                            // clear_custom_error() calls borrow() would panic.
                            drop(dm);
                            self.loading_done = false;
                            self.loading_error = None;
                            self.file_panel.clear_custom_error();
                        }
                        Err(e) => {
                            drop(dm);
                            self.loading_error = Some(e.clone());
                            self.file_panel.set_custom_error(&e);
                        }
                    }
                    changed = true;
                }
                emcore::emFileModel::FileState::Loading { .. } => match dm.try_continue_loading() {
                    Ok(true) => {
                        dm.state = emcore::emFileModel::FileState::Loaded;
                        dm.quit_loading();
                        self.loading_done = true;
                        drop(dm);
                        self.file_panel.clear_custom_error();
                        self.update_children(ctx);
                        changed = true;
                    }
                    Ok(false) => {
                        dm.state = emcore::emFileModel::FileState::Loading {
                            progress: dm.calc_file_progress(),
                        };
                        changed = true;
                    }
                    Err(e) => {
                        drop(dm);
                        self.loading_error = Some(e.clone());
                        self.file_panel.set_custom_error(&e);
                        changed = true;
                    }
                },
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
        _ctx: &mut PanelCtx,
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

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.contains(NoticeFlags::VIEWING_CHANGED)
            || flags.contains(NoticeFlags::SOUGHT_NAME_CHANGED)
        {
            // C++ emDirPanel::Notice:
            //   if (IsViewed() || GetSoughtName()) {
            //     if (!GetFileModel()) SetFileModel(emDirModel::Acquire(...))
            //   } else if (GetFileModel()) SetFileModel(NULL)
            // We use in_active_path as proxy for "being viewed or sought"
            // since PanelState doesn't expose sought_name.
            let keep_model = state.viewed || state.in_active_path;
            if keep_model {
                if self.dir_model.is_none() {
                    // Port of C++ emDirPanel::Notice line:
                    // if (!GetFileModel()) SetFileModel(emDirModel::Acquire(...))
                    let dm_rc = emDirModel::Acquire(&self.ctx, &self.path);
                    self.file_panel.SetFileModel(Some(
                        Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>,
                    ));
                    self.dir_model = Some(dm_rc);
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
                        rects[i].x,
                        rects[i].y,
                        rects[i].w,
                        rects[i].h,
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
                if ch > rect.h {
                    ch = rect.h;
                    cw = ch / t;
                }
                ctx.layout_child_canvas(*child, 0.0, 0.0, cw, ch, canvas_color);
            }
        }
    }

    fn CreateControlPanel(&mut self, parent_ctx: &mut PanelCtx, name: &str) -> Option<PanelId> {
        let panel = {
            let mut sched = parent_ctx
                .as_sched_ctx()
                .expect("CreateControlPanel requires scheduler-reach PanelCtx");
            crate::emFileManControlPanel::emFileManControlPanel::new(
                &mut sched,
                Rc::clone(&self.ctx),
            )
            .with_dir_path(&self.path)
        };
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
    fn model_acquired_connects_file_panel() {
        // Regression: emDirPanel::notice acquired dir_model but never called
        // file_panel.SetFileModel(Some(...)), leaving file_panel in NoFileModel
        // state and causing "No file model" to render during directory loading.
        use emcore::emFileModel::FileModelState;
        use emcore::emFilePanel::VirtualFileState;
        let ctx = emcore::emContext::emContext::NewRoot();
        let dm_rc = emDirModel::Acquire(&ctx, "/tmp");
        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());
        panel.file_panel.SetFileModel(Some(
            Rc::clone(&dm_rc) as Rc<std::cell::RefCell<dyn FileModelState>>,
        ));
        assert_ne!(
            panel.file_panel.GetVirFileState(),
            VirtualFileState::NoFileModel,
            "file_panel must be Waiting after model acquired, not NoFileModel"
        );
    }

    #[test]
    fn cycle_waiting_to_loading_advances_vir_state() {
        // Regression: emDirPanel::Cycle called clear_custom_error() while
        // dm (RefMut<emDirModel>) was still live. Since file_panel.model and
        // dir_model share the same Rc<RefCell<emDirModel>>, compute_vir_file_state
        // tried borrow() on the same RefCell that held borrow_mut() → panic.
        // Fix: drop(dm) before any file_panel.* calls. This test verifies the
        // Waiting→Loading transition completes without panic and the VirtualFileState
        // advances from Waiting to Loading.
        use emcore::emFileModel::{FileModelState, FileState};
        use emcore::emFilePanel::VirtualFileState;

        let ctx = emcore::emContext::emContext::NewRoot();
        let dm_rc = emDirModel::Acquire(&ctx, "/tmp");

        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());
        panel.file_panel.SetFileModel(Some(
            Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>,
        ));
        panel.dir_model = Some(Rc::clone(&dm_rc));

        // Execute the Waiting→Loading transition exactly as Cycle does it
        // after the fix (drop dm before file_panel ops).
        {
            let mut dm = dm_rc.borrow_mut();
            assert!(
                matches!(dm.get_file_state(), FileState::Waiting),
                "model must start in Waiting state"
            );
            dm.try_start_loading().expect("/tmp must be readable");
            dm.state = FileState::Loading { progress: 0.0 };
        } // dm dropped here — borrow_mut released
        panel.loading_done = false;
        panel.loading_error = None;
        panel.file_panel.clear_custom_error(); // would panic before fix
        panel.file_panel.refresh_vir_file_state();

        assert!(
            matches!(
                panel.file_panel.GetVirFileState(),
                VirtualFileState::Loading { .. }
            ),
            "VirtualFileState must advance to Loading after Waiting→Loading transition"
        );
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
        assert_eq!(panel.key_walk_state.as_ref().unwrap().search, "a");

        // Within timeout: appends
        panel.key_walk('b');
        assert_eq!(panel.key_walk_state.as_ref().unwrap().search, "ab");
    }

    #[test]
    fn key_walk_wildcard() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());

        panel.key_walk('*');
        panel.key_walk('t');
        assert_eq!(panel.key_walk_state.as_ref().unwrap().search, "*t");
    }

    // ── Notice-path engine registration/firing diagnostics ──────────────────
    //
    // Regression guard: a panel created via create_child_with + wake_up_panel
    // inside a notice handler (PanelCtx::with_scheduler context, as used by
    // emView::HandleNotice → handle_notice_one) must get engine_id registered
    // and its PanelCycleEngine must fire in the same DoTimeSlice.
    //
    // This mirrors the production path that was broken: emDirEntryPanel::notice
    // → update_content_panel → create_child_with(emDirPanel) + wake_up_panel —
    // after which emDirPanel::Cycle never fired.

    /// Verifies that a child created inside a PanelCtx::with_scheduler context
    /// (identical to handle_notice_one's ctx) results in wake_up_panel leaving
    /// an awake engine in the scheduler.
    ///
    /// Failure means either:
    /// (a) register_engine_for returned early for this context (has_view=false
    ///     or no scheduler) so engine_id=None → wake_up_panel is a no-op, or
    /// (b) wake_up_panel found engine_id but wake_up() didn't queue it.
    #[test]
    fn notice_ctx_child_wake_up_panel_queues_engine() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;

        let emctx = emcore::emContext::emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", true);
        let mut sched = EngineScheduler::new();
        tree.register_engine_for_public(root, Some(&mut sched));

        let parent = tree.create_child(root, "parent", Some(&mut sched));

        // Drain any engines woken by create_child's add_to_notice_list so we
        // get a clean has_awake_engines() reading after wake_up_panel.
        // (add_to_notice_list wakes the update_engine_id, which is None here,
        // but PanelCycleEngine for root/parent is woken by INIT_NOTICE_FLAGS.)
        // We specifically want to measure whether wake_up_panel for the CHILD
        // adds to the queue.
        let awake_before = sched.has_awake_engines();

        // Simulate handle_notice_one's PanelCtx + update_content_panel
        {
            let mut ctx = PanelCtx::with_scheduler(&mut tree, parent, 1.0, &mut sched);
            let child_id = ctx.create_child_with(
                "content",
                Box::new(emDirPanel::new(Rc::clone(&emctx), "/tmp".to_string())),
            );
            ctx.wake_up_panel(child_id);
        };
        let awake_after_wake = sched.has_awake_engines();

        // Reporting: show the transition even on success to make failures obvious.
        // The key assertion: wake_up_panel must result in an awake engine.
        assert!(
            awake_after_wake,
            "wake_up_panel must queue the child's PanelCycleEngine; \
             awake_before={awake_before} awake_after_wake={awake_after_wake} — \
             if false, engine_id was None (has_view not propagated or no scheduler in ctx)"
        );
        // Cleanup: deregister all engines before scheduler is dropped.
        tree.remove(root, Some(&mut sched));
    }

    /// Verifies that the PanelCycleEngine for a child created inside a notice
    /// handler fires within the same DoTimeSlice — the full scheduling roundtrip.
    ///
    /// Observable: emDirPanel::Cycle returns stay_awake=true while loading, so
    /// the engine is re-queued; sched.has_awake_engines() must be true after
    /// DoTimeSlice if Cycle ran.
    ///
    /// Failure (after notice_ctx_child_wake_up_panel_queues_engine passes) means
    /// the engine is woken but the scheduler never dispatches it:
    /// PanelScope::Toplevel window not found, or take_behavior returning None.
    #[test]
    fn notice_ctx_child_engine_fires_in_same_slice() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::collections::HashMap;
        use std::rc::Rc;

        let emctx = emcore::emContext::emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", true);
        let mut sched = EngineScheduler::new();
        tree.register_engine_for_public(root, Some(&mut sched));

        let parent = tree.create_child(root, "parent", Some(&mut sched));

        // Simulate notice-path: create emDirPanel child + wake, exactly as
        // handle_notice_one → update_content_panel does.
        {
            let mut ctx = PanelCtx::with_scheduler(&mut tree, parent, 1.0, &mut sched);
            let child_id = ctx.create_child_with(
                "content",
                Box::new(emDirPanel::new(Rc::clone(&emctx), "/tmp".to_string())),
            );
            ctx.wake_up_panel(child_id);
        }

        // Wrap tree in headless window so Toplevel(wid) dispatch finds it.
        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = std::cell::RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(std::cell::RefCell::new(
            Vec::<emcore::emGUIFramework::DeferredAction>::new(),
        ));

        // Drain any engines already awake before our panel (root/parent engines
        // woken by INIT_NOTICE_FLAGS) so they don't muddy the has_awake signal.
        sched.DoTimeSlice(
            &mut windows,
            &emctx,
            &mut fw,
            &mut pending_inputs,
            &mut input_state,
            &cb,
            &pa,
        );

        // emDirPanel::Cycle returns stay_awake=true while loading ("/tmp").
        // If it ran, the engine is re-queued and has_awake_engines() is true.
        // If it never ran (window scope mismatch / behavior taken), it was
        // dequeued and NOT re-queued → has_awake_engines() is false.
        assert!(
            sched.has_awake_engines(),
            "emDirPanel PanelCycleEngine must fire and re-queue itself (stay_awake=true \
             while loading /tmp); has_awake_engines()=false means Cycle never ran — \
             check PanelScope::Toplevel wid={:?} window lookup or take_behavior returning None",
            wid
        );
        // Cleanup: reclaim tree from window and remove all panels.
        let mut win = windows.remove(&wid).unwrap();
        let mut reclaimed = win.take_tree();
        reclaimed.remove(root, Some(&mut sched));
    }

    /// Reproduces the full production path:
    /// RegisterEngines (High-priority UpdateEngineClass) → HandleNotice →
    /// parent::notice creates emDirPanel child → wake_up_panel.
    ///
    /// This is the path that fails in the app: UpdateEngineClass (High) fires,
    /// delivers a notice to a parent panel, the parent creates an emDirPanel
    /// child during the notice, calls wake_up_panel. Then UpdateEngineClass
    /// completes and PanelCycleEngine (Medium) should fire in the same slice.
    ///
    /// If this test fails but notice_ctx_child_engine_fires_in_same_slice
    /// passes, the bug is in the UpdateEngineClass-mediated path — something
    /// in the High-to-Medium priority scheduling when UpdateEngineClass holds
    /// the tree.
    #[test]
    fn production_path_child_engine_fires_after_update_engine_notice() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::rc::Rc;

        // A parent panel that creates an emDirPanel child on the first
        // VIEWING_CHANGED notice, then calls wake_up_panel on it.
        struct NoticeSpawner {
            emctx: Rc<emcore::emContext::emContext>,
            path: String,
            spawned: bool,
        }
        impl PanelBehavior for NoticeSpawner {
            fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, ctx: &mut PanelCtx) {
                if flags.contains(NoticeFlags::VIEWING_CHANGED) && !self.spawned {
                    let child_id = ctx.create_child_with(
                        "content",
                        Box::new(emDirPanel::new(
                            Rc::clone(&self.emctx),
                            self.path.clone(),
                        )),
                    );
                    ctx.wake_up_panel(child_id);
                    self.spawned = true;
                }
            }
        }

        let emctx = emcore::emContext::emContext::NewRoot();
        let mut tree = PanelTree::new();
        // Production: root created with has_view=false, then init_panel_view(None)
        let root = tree.create_root("root", false);
        tree.init_panel_view(root, None); // sets has_view=true, no engines yet
        let parent = tree.create_child(root, "parent", None); // has_view=true, no engine yet

        // Insert NoticeSpawner behavior for parent
        tree.set_behavior(
            parent,
            Box::new(NoticeSpawner {
                emctx: Rc::clone(&emctx),
                path: "/tmp".to_string(),
                spawned: false,
            }),
        );

        // Queue VIEWING_CHANGED on parent so UpdateEngineClass delivers it
        let mut sched = EngineScheduler::new();

        // Wrap tree in headless window BEFORE RegisterEngines (production order:
        // init_panel_view → put_tree → windows.insert → RegisterEngines).
        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        // RegisterEngines: set scope + register PanelCycleEngines for existing panels
        // + register UpdateEngineClass (High priority) + wake it.
        // Save view-engine IDs so we can deregister them during cleanup.
        let update_engine_ids: Vec<emcore::emEngine::EngineId> = {
            let win = windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut ids = Vec::new();
            {
                let scope = emcore::emPanelScope::PanelScope::Toplevel(wid);
                let mut fw = Vec::new();
                let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
                let pa = Rc::new(RefCell::new(
                    Vec::<emcore::emGUIFramework::DeferredAction>::new(),
                ));
                let mut sc = emcore::emEngineCtx::SchedCtx {
                    scheduler: &mut sched,
                    framework_actions: &mut fw,
                    root_context: &emctx,
                    framework_clipboard: &cb,
                    current_engine: None,
                    pending_actions: &pa,
                };
                win.view_mut().RegisterEngines(&mut sc, &mut tree, scope);
                // Save engine IDs registered by RegisterEngines so we can remove them.
                if let Some(eid) = win.view().update_engine_id {
                    ids.push(eid);
                }
                if let Some(eid) = win.view().visiting_va_engine_id {
                    ids.push(eid);
                }
                // Queue VIEWING_CHANGED on parent now that view is set up
                tree.queue_notice(parent, NoticeFlags::VIEWING_CHANGED, Some(&mut sched));
            }
            win.put_tree(tree);
            ids
        };

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(RefCell::new(
            Vec::<emcore::emGUIFramework::DeferredAction>::new(),
        ));

        // Run several slices: UpdateEngineClass fires → delivers notice →
        // NoticeSpawner creates emDirPanel → wake_up_panel.
        // Then PanelCycleEngine(Medium) should fire (Cycle returns stay_awake=true).
        for _ in 0..5 {
            sched.DoTimeSlice(
                &mut windows,
                &emctx,
                &mut fw,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
            );
            if sched.has_awake_engines() {
                // Something is still running — check if it's the emDirPanel engine
                // (stay_awake=true means Cycle ran and is loading /tmp).
                // We can't distinguish which engine is awake, but if after 5 slices
                // has_awake_engines() is true, the loading is in progress.
                break;
            }
        }

        // Reclaim tree for cleanup check
        let mut win = windows.remove(&wid).unwrap();
        let mut reclaimed = win.take_tree();

        // Check: did emDirPanel's behavior get modified? (i.e., did Cycle run?)
        // After one Cycle, loading_done=false, dir_model=Some. The model would
        // be in Loading state. If Cycle never ran, dir_model=None.
        // We check by querying the child panel's VirtualFileState via file_panel.
        // But we can't directly access behavior without pub(crate) methods.
        // Use the observable proxy: has_awake_engines after all slices.
        // If emDirPanel Cycle ran (stay_awake=true), it's in the awake queue.
        assert!(
            sched.has_awake_engines(),
            "After {n} DoTimeSlice calls through UpdateEngineClass → notice → create_child_with \
             + wake_up_panel, emDirPanel PanelCycleEngine must have fired (stay_awake=true). \
             has_awake_engines()=false means either:\n\
             (a) wake_up_panel found engine_id=None — engine not registered during notice\n\
             (b) PanelCycleEngine fired but Cycle returned false — unexpected\n\
             (c) PanelCycleEngine never dispatched despite being in queue (scope/window mismatch)\n\
             This is the exact production failure mode: UpdateEngineClass(High) blocks \
             PanelCycleEngine(Medium) from running.",
            n = 5,
        );

        reclaimed.remove(root, Some(&mut sched));
        // Remove view-owned engines (UpdateEngineClass, VisitingVAEngineClass)
        // that are not panel-tree engines and won't be cleaned up by remove().
        for eid in update_engine_ids {
            sched.remove_engine(eid);
        }
    }

    /// Probe-based version of `production_path_child_engine_fires_after_update_engine_notice`.
    ///
    /// The earlier test uses `has_awake_engines()` which is true whenever ANY engine
    /// is awake — including UpdateEngineClass itself. It does not prove that the
    /// emDirPanel's PanelCycleEngine was dispatched.
    ///
    /// This test attaches a `first_cycle_probe` to the child's engine BEFORE the
    /// slices run, so we can distinguish:
    ///   - `probe.get() == None`  → PanelCycleEngine was never dispatched (scope/wake bug)
    ///   - `probe.get() == Some(n)` → dispatched at time-slice n (Cycle ran)
    #[test]
    fn probe_confirms_dir_panel_cycle_dispatched_via_notice_spawner() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::{Cell, RefCell};
        use std::collections::HashMap;
        use std::rc::Rc;

        struct NoticeSpawner {
            emctx: Rc<emcore::emContext::emContext>,
            path: String,
            child_id: Option<emcore::emPanelTree::PanelId>,
        }
        impl PanelBehavior for NoticeSpawner {
            fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, ctx: &mut PanelCtx) {
                if flags.contains(NoticeFlags::VIEWING_CHANGED) && self.child_id.is_none() {
                    let id = ctx.create_child_with(
                        "content",
                        Box::new(emDirPanel::new(Rc::clone(&self.emctx), self.path.clone())),
                    );
                    ctx.wake_up_panel(id);
                    self.child_id = Some(id);
                }
            }
        }

        let emctx = emcore::emContext::emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", false);
        tree.init_panel_view(root, None);
        let parent = tree.create_child(root, "parent", None);
        tree.set_behavior(
            parent,
            Box::new(NoticeSpawner {
                emctx: Rc::clone(&emctx),
                path: "/tmp".to_string(),
                child_id: None,
            }),
        );

        let mut sched = EngineScheduler::new();
        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        let scope = emcore::emPanelScope::PanelScope::Toplevel(wid);
        let view_engine_ids: Vec<emcore::emEngine::EngineId> = {
            let win = windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut ids = Vec::new();
            {
                let mut fw = Vec::new();
                let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
                let pa = Rc::new(RefCell::new(
                    Vec::<emcore::emGUIFramework::DeferredAction>::new(),
                ));
                let mut sc = emcore::emEngineCtx::SchedCtx {
                    scheduler: &mut sched,
                    framework_actions: &mut fw,
                    root_context: &emctx,
                    framework_clipboard: &cb,
                    current_engine: None,
                    pending_actions: &pa,
                };
                win.view_mut().RegisterEngines(&mut sc, &mut tree, scope);
                if let Some(eid) = win.view().update_engine_id {
                    ids.push(eid);
                }
                if let Some(eid) = win.view().visiting_va_engine_id {
                    ids.push(eid);
                }
                tree.queue_notice(parent, NoticeFlags::VIEWING_CHANGED, Some(&mut sched));
            }
            win.put_tree(tree);
            ids
        };

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(RefCell::new(
            Vec::<emcore::emGUIFramework::DeferredAction>::new(),
        ));

        // Slice 1: UpdateEngineClass fires → notice → NoticeSpawner creates child + wakes engine
        sched.DoTimeSlice(
            &mut windows,
            &emctx,
            &mut fw,
            &mut pending_inputs,
            &mut input_state,
            &cb,
            &pa,
        );

        // After slice 1: find the child engine and attach probe BEFORE more slices run.
        let probe_cell: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        let child_engine_found = {
            let win = windows.get_mut(&wid).unwrap();
            let tree = win.take_tree();
            let child_id = tree.find_child_by_name(parent, "content");
            let found = if let Some(cid) = child_id {
                if let Some(eid) = tree.panel_engine_id_pub(cid) {
                    sched.attach_first_cycle_probe(eid, Rc::clone(&probe_cell));
                    true
                } else {
                    false
                }
            } else {
                false
            };
            win.put_tree(tree);
            found
        };

        assert!(
            child_engine_found,
            "After slice 1, content child must exist and have a registered PanelCycleEngine. \
             child_engine_found=false means either the child was not created (notice did not fire \
             or create_child_with failed) or the engine was not registered (has_view=false)."
        );

        // Slices 2-11: PanelCycleEngine (Medium) should now fire.
        for _ in 0..10 {
            sched.DoTimeSlice(
                &mut windows,
                &emctx,
                &mut fw,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
            );
            if probe_cell.get().is_some() {
                break;
            }
        }

        assert!(
            probe_cell.get().is_some(),
            "emDirPanel PanelCycleEngine must be dispatched within 10 slices after the child \
             is created and woken via wake_up_panel. probe=None means:\n\
             (a) scope mismatch — engine registered with wrong WindowId, scheduler skips it\n\
             (b) engine deregistered between wake and dispatch (panel deleted)\n\
             (c) scheduler priority inversion — UpdateEngineClass permanently blocks Medium\n\
             If this test passes but the app still shows Wait..., the issue is in the \
             real emDirEntryPanel path, not the NoticeSpawner stub."
        );

        // Cleanup
        let mut win = windows.remove(&wid).unwrap();
        let mut tree = win.take_tree();
        tree.remove(root, Some(&mut sched));
        for eid in view_engine_ids {
            sched.remove_engine(eid);
        }
    }

    /// End-to-end test using the real `emDirEntryPanel` hierarchy.
    ///
    /// Unlike `probe_confirms_dir_panel_cycle_dispatched_via_notice_spawner` (which uses a
    /// NoticeSpawner stub), this test instantiates the actual production stack:
    ///
    ///   emDirEntryPanel (parent) → update_content_panel → emDirFpPlugin → emDirPanel (child)
    ///
    /// `is_sought=true` (via `set_seek_pos_pub`) bypasses geometry conditions so the child is
    /// created in a headless environment.
    ///
    /// If this test FAILS but `probe_confirms_dir_panel_cycle_dispatched_via_notice_spawner`
    /// PASSES, the bug is in the real `emDirEntryPanel::update_content_panel` path vs the stub.
    /// If both PASS but the app shows Wait..., the issue is in the real window setup or event loop.
    #[test]
    fn real_stack_dir_entry_panel_cycle_dispatched() {
        use emcore::emPanel::NoticeFlags;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::{Cell, RefCell};
        use std::collections::HashMap;
        use std::rc::Rc;

        use crate::emDirEntry::emDirEntry;
        use crate::emDirEntryPanel::emDirEntryPanel;
        use crate::emDirEntryPanel::CONTENT_NAME;

        // Register emDirFpPlugin so CreateFilePanelWithStat returns emDirPanel.
        let emctx = emcore::emContext::emContext::NewRoot();
        {
            use emcore::emFpPlugin::{emFpPlugin, emFpPluginList};
            let dir_plugin = emFpPlugin::for_test_directory_handler(
                "emDir",
                crate::emDirFpPlugin::emDirFpPluginFunc,
            );
            emctx.acquire::<emFpPluginList>("", || {
                emFpPluginList::from_plugins(vec![dir_plugin])
            });
        }

        // Build tree: root→emDirEntryPanel for /tmp
        let entry = emDirEntry::from_parent_and_name("", "/tmp");
        let dep = emDirEntryPanel::new(Rc::clone(&emctx), entry);
        let mut tree = PanelTree::new();
        let root = tree.create_root("dep_root", false);
        tree.init_panel_view(root, None);
        tree.set_behavior(root, Box::new(dep));
        // Set seek so should_create bypasses geometry check
        tree.set_seek_pos_pub(root, CONTENT_NAME);

        let mut sched = EngineScheduler::new();
        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        let scope = emcore::emPanelScope::PanelScope::Toplevel(wid);
        let view_engine_ids: Vec<emcore::emEngine::EngineId> = {
            let win = windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut ids = Vec::new();
            {
                let mut fw = Vec::new();
                let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
                let pa = Rc::new(RefCell::new(
                    Vec::<emcore::emGUIFramework::DeferredAction>::new(),
                ));
                let mut sc = emcore::emEngineCtx::SchedCtx {
                    scheduler: &mut sched,
                    framework_actions: &mut fw,
                    root_context: &emctx,
                    framework_clipboard: &cb,
                    current_engine: None,
                    pending_actions: &pa,
                };
                win.view_mut().RegisterEngines(&mut sc, &mut tree, scope);
                if let Some(eid) = win.view().update_engine_id {
                    ids.push(eid);
                }
                if let Some(eid) = win.view().visiting_va_engine_id {
                    ids.push(eid);
                }
                // Queue SOUGHT_NAME_CHANGED so update_content_panel fires and sees is_sought=true
                tree.queue_notice(
                    root,
                    NoticeFlags::SOUGHT_NAME_CHANGED,
                    Some(&mut sched),
                );
            }
            win.put_tree(tree);
            ids
        };

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(RefCell::new(
            Vec::<emcore::emGUIFramework::DeferredAction>::new(),
        ));

        // Slice 1: UpdateEngineClass → notice → emDirEntryPanel creates emDirPanel child
        sched.DoTimeSlice(
            &mut windows,
            &emctx,
            &mut fw,
            &mut pending_inputs,
            &mut input_state,
            &cb,
            &pa,
        );

        // Find child and attach probe
        let probe_cell: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
        let child_engine_found = {
            let win = windows.get_mut(&wid).unwrap();
            let tree = win.take_tree();
            let child_id = tree.find_child_by_name(root, CONTENT_NAME);
            let found = if let Some(cid) = child_id {
                if let Some(eid) = tree.panel_engine_id_pub(cid) {
                    sched.attach_first_cycle_probe(eid, Rc::clone(&probe_cell));
                    true
                } else {
                    false
                }
            } else {
                false
            };
            win.put_tree(tree);
            found
        };

        assert!(
            child_engine_found,
            "emDirEntryPanel must create a content child with a registered PanelCycleEngine \
             when is_sought=true. child_engine_found=false means either the plugin system did \
             not return emDirPanel (emDirFpPlugin not registered) or the engine was not \
             registered (has_view=false propagation broken)."
        );

        // Run slices: emDirPanel PanelCycleEngine (Medium) must fire
        for _ in 0..10 {
            sched.DoTimeSlice(
                &mut windows,
                &emctx,
                &mut fw,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
            );
            if probe_cell.get().is_some() {
                break;
            }
        }

        assert!(
            probe_cell.get().is_some(),
            "emDirPanel PanelCycleEngine must be dispatched via the real emDirEntryPanel path. \
             probe=None means Cycle was never called. This is the production failure mode: \
             the app shows Wait... indefinitely because Cycle never advances loading state."
        );

        // Cleanup
        let mut win = windows.remove(&wid).unwrap();
        let mut tree = win.take_tree();
        tree.remove(root, Some(&mut sched));
        for eid in view_engine_ids {
            sched.remove_engine(eid);
        }
    }

    /// Hypothesis B test (F010): after loading completes, does the inner emDirPanel
    /// actually have entry children with non-zero layout rects?
    ///
    /// Drives the full real_stack pipeline (emDirEntryPanel → emDirFpPlugin → emDirPanel)
    /// with a populated temp directory, runs DoTimeSlice until loading finishes,
    /// then asserts:
    ///   (a) the emDirPanel has one emDirEntryPanel child per visible entry
    ///   (b) each child's layout_rect is non-zero (LayoutChildren ran and sized it)
    ///
    /// Failure (a) confirms Hypothesis B(i): entries never created.
    /// Failure (b) confirms Hypothesis B(ii): created but clipped/zero-sized.
    #[test]
    fn real_stack_dir_panel_children_created_with_nonzero_rects_after_load() {
        use emcore::emPanel::NoticeFlags;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::rc::Rc;

        use crate::emDirEntry::emDirEntry;
        use crate::emDirEntryPanel::emDirEntryPanel;
        use crate::emDirEntryPanel::CONTENT_NAME;

        // Populated temp directory with known entry count.
        let tmpdir = std::env::temp_dir().join("emfileman_f010_hyp_b");
        let _ = std::fs::remove_dir_all(&tmpdir);
        std::fs::create_dir_all(&tmpdir).unwrap();
        for name in &["a.txt", "b.txt", "c.txt", "d.txt"] {
            std::fs::write(tmpdir.join(name), b"x").unwrap();
        }
        let path = tmpdir.to_string_lossy().to_string();
        let expected_entries = 4;

        let emctx = emcore::emContext::emContext::NewRoot();
        {
            use emcore::emFpPlugin::{emFpPlugin, emFpPluginList};
            let dir_plugin = emFpPlugin::for_test_directory_handler(
                "emDir",
                crate::emDirFpPlugin::emDirFpPluginFunc,
            );
            emctx.acquire::<emFpPluginList>("", || {
                emFpPluginList::from_plugins(vec![dir_plugin])
            });
        }

        let entry = emDirEntry::from_parent_and_name("", &path);
        let dep = emDirEntryPanel::new(Rc::clone(&emctx), entry);
        let mut tree = PanelTree::new();
        let root = tree.create_root("dep_root", false);
        tree.init_panel_view(root, None);
        tree.set_behavior(root, Box::new(dep));
        tree.set_seek_pos_pub(root, CONTENT_NAME);

        let mut sched = EngineScheduler::new();
        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        let scope = emcore::emPanelScope::PanelScope::Toplevel(wid);
        let view_engine_ids: Vec<emcore::emEngine::EngineId> = {
            let win = windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut ids = Vec::new();
            {
                let mut fw = Vec::new();
                let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
                let pa = Rc::new(RefCell::new(
                    Vec::<emcore::emGUIFramework::DeferredAction>::new(),
                ));
                let mut sc = emcore::emEngineCtx::SchedCtx {
                    scheduler: &mut sched,
                    framework_actions: &mut fw,
                    root_context: &emctx,
                    framework_clipboard: &cb,
                    current_engine: None,
                    pending_actions: &pa,
                };
                win.view_mut().RegisterEngines(&mut sc, &mut tree, scope);
                if let Some(eid) = win.view().update_engine_id {
                    ids.push(eid);
                }
                if let Some(eid) = win.view().visiting_va_engine_id {
                    ids.push(eid);
                }
                tree.queue_notice(
                    root,
                    NoticeFlags::SOUGHT_NAME_CHANGED,
                    Some(&mut sched),
                );
            }
            win.put_tree(tree);
            ids
        };

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(RefCell::new(
            Vec::<emcore::emGUIFramework::DeferredAction>::new(),
        ));

        // Drive enough slices to:
        //   1. notice → create emDirPanel child
        //   2. emDirPanel::Cycle: acquire model → Waiting → Loading
        //   3. N ReadingNames slices (one per dir entry)
        //   4. 1 Sorting slice
        //   5. N LoadingEntries slices (one per entry)
        //   6. Final slice: Ok(true) → Loaded → update_children → create entry panels
        //   7. LayoutChildren on emDirPanel → assign rects
        // 200 slices is far more than needed for 4 entries.
        for _ in 0..200 {
            sched.DoTimeSlice(
                &mut windows,
                &emctx,
                &mut fw,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
            );
            if !sched.has_awake_engines() {
                break;
            }
        }

        // Inspect tree state.
        let mut win = windows.remove(&wid).unwrap();
        let mut tree = win.take_tree();

        let dir_panel_id = tree
            .find_child_by_name(root, CONTENT_NAME)
            .expect("emDirPanel content child must exist after loading");

        let entry_children: Vec<_> = tree.children(dir_panel_id).collect();
        assert_eq!(
            entry_children.len(),
            expected_entries,
            "emDirPanel must have {expected_entries} entry children after loading {path}; \
             got {}. Hypothesis B(i) confirmed: entries not created.",
            entry_children.len(),
        );

        for cid in &entry_children {
            let name = tree.get_panel_name(*cid);
            let rect = tree.layout_rect(*cid).expect("child layout rect");
            assert!(
                rect.w > 0.0 && rect.h > 0.0,
                "Entry child {name:?} has zero-sized layout_rect: w={} h={}. \
                 Hypothesis B(ii) confirmed: LayoutChildren never ran or produced zero rects.",
                rect.w, rect.h,
            );
        }

        // Cleanup
        tree.remove(root, Some(&mut sched));
        for eid in view_engine_ids {
            sched.remove_engine(eid);
        }
        let _ = std::fs::remove_dir_all(&tmpdir);
    }
}
