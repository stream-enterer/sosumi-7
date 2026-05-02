//! Port of C++ emDirPanel grid layout algorithm (LayoutChildren).

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::PanelCtx;
use emcore::emFileModel::{FileModelState, FileState};
use emcore::emFilePanel::{emFilePanel, VirtualFileState};
use emcore::emInput::{emInputEvent, InputKey};
use emcore::emInputState::emInputState;
use emcore::emPainter::emPainter;
use emcore::emPanel::{FileLoadStatus, NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::PanelId;
use slotmap::Key as _;

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
pub struct emDirPanel {
    pub(crate) file_panel: emFilePanel,
    ctx: Rc<emContext>,
    pub(crate) path: String,
    config: Rc<RefCell<emFileManViewConfig>>,
    file_man: Rc<RefCell<emFileManModel>>,
    dir_model: Option<Rc<RefCell<emDirModel>>>,
    pub(crate) content_complete: bool,
    child_count: usize,
    key_walk_state: Option<KeyWalkState>,
    scroll_target: Option<String>,
    /// First-Cycle init guard for D-006 subscribe shape.
    subscribed_init: bool,
}

impl emDirPanel {
    pub fn new(ctx: Rc<emContext>, path: String) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        let file_man = emFileManModel::Acquire(&ctx);
        Self {
            file_panel: emFilePanel::new(),
            ctx,
            file_man,
            path,
            config,
            dir_model: None,
            content_complete: false,
            child_count: 0,
            key_walk_state: None,
            scroll_target: None,
            subscribed_init: false,
        }
    }

    pub fn IsContentComplete(&self) -> bool {
        self.content_complete
    }

    pub fn GetPath(&self) -> &str {
        &self.path
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

    /// B-016 test mutator: drive a custom error onto the embedded `emFilePanel`.
    #[doc(hidden)]
    pub fn set_custom_error_for_test(&mut self, msg: &str) {
        self.file_panel.set_custom_error(msg);
    }

    pub(crate) fn SelectAll(&self, ectx: &mut impl emcore::emEngineCtx::SignalCtx) {
        if let Some(ref dm_rc) = self.dir_model {
            let show_hidden = self.config.borrow().GetShowHiddenFiles();
            let dm = dm_rc.borrow();
            let mut fm = self.file_man.borrow_mut();
            for i in 0..dm.GetEntryCount() {
                let entry = dm.GetEntry(i);
                if !entry.IsHidden() || show_hidden {
                    fm.SelectAsTarget(ectx, entry.GetPath());
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

    fn is_model_loaded(&self) -> bool {
        self.dir_model
            .as_ref()
            .is_some_and(|dm| matches!(dm.borrow().get_file_state(), FileState::Loaded))
    }

    fn update_children(&mut self, ctx: &mut PanelCtx) {
        if !self.is_model_loaded() {
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
    fn file_load_status(&self) -> Option<FileLoadStatus> {
        Some(emcore::emFilePanel::map_vir_state(
            &self.file_panel.GetVirFileState(),
        ))
    }

    fn dump_state(&self) -> Vec<(&'static str, String)> {
        let (loading_done, loading_error) = match self
            .dir_model
            .as_ref()
            .map(|dm| dm.borrow().get_file_state())
        {
            Some(FileState::Loaded) => ("true".to_string(), String::new()),
            Some(FileState::LoadError(e)) => ("false".to_string(), e),
            _ => ("false".to_string(), String::new()),
        };
        vec![
            ("path", self.path.clone()),
            ("loading_done", loading_done),
            ("loading_error", loading_error),
            ("content_complete", self.content_complete.to_string()),
            ("child_count", self.child_count.to_string()),
            ("has_dir_model", self.dir_model.is_some().to_string()),
            (
                "scroll_target",
                self.scroll_target.as_deref().unwrap_or("").to_string(),
            ),
        ]
    }

    fn Cycle(&mut self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        // B-016 (1) MANDATORY emFilePanel::Cycle prefix in derived panel.
        // Mirrors C++ `busy=emFilePanel::Cycle()` at emDirPanel.cpp:74.
        // The Rust composition pattern (emFilePanel as field, not base
        // class) means `<emFilePanel as PanelBehavior>::Cycle` is never
        // invoked by the engine for this panel; we must run its prefix
        // explicitly. Implementer-of-record: emImageFileImageFilePanel.rs:211-235.
        self.file_panel.ensure_vir_file_state_signal(ectx);
        self.file_panel.fire_pending_vir_state(ectx);

        // Port of C++ emFilePanel::Cycle (emFilePanel.cpp:151-161): observe
        // the file model's state and refresh the painted view; never drive
        // loading from the panel. Loading is owned by emDirModelEngine,
        // registered lazily here.
        if self.dir_model.is_none() {
            let dm_rc = emDirModel::Acquire(&self.ctx, &self.path);
            emDirModel::ensure_engine_registered(&dm_rc, ectx.scheduler);
            self.file_panel
                .SetFileModel(Some(Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>));
            self.dir_model = Some(dm_rc);
            self.child_count = 0;
            self.content_complete = false;
        }

        // B-015 (F019): subscribe the panel to the model's FileStateSignal.
        // <emFilePanel as PanelBehavior>::Cycle (which embeds B-015) is not
        // invoked for emDirPanel — emFilePanel is held as a field, not a
        // base behavior — so the subscribe must run explicitly here as part
        // of the B-016 prefix (mirrors the explicit cycle_inner suffix
        // below). Without this, the model's FileStateSignal fires would
        // have no subscribed panel and the panel would never re-cycle
        // after its initial wake.
        self.file_panel.connect_file_state_signal(ectx);

        // D-006 first-Cycle init: lazy-allocate ChangeSignal and connect.
        // Mirrors C++ emDirPanel ctor `AddWakeUpSignal(...)` (rows 37-38).
        if !self.subscribed_init {
            let eid = ectx.engine_id;
            let chg_sig = self.config.borrow().GetChangeSignal(ectx);
            ectx.connect(chg_sig, eid);
            // B-016: subscribe to emFilePanel::GetVirFileStateSignal.
            // Mirrors C++ emDirPanel.cpp:37 AddWakeUpSignal(GetVirFileStateSignal()).
            let vfs_sig = self.file_panel.GetVirFileStateSignal();
            if !vfs_sig.is_null() {
                ectx.connect(vfs_sig, eid);
            }
            self.subscribed_init = true;
        }

        // Mirrors C++ emDirPanel.cpp:75-82 — fire-driven invalidation/rebuild.
        //   if (IsSignaled(GetVirFileStateSignal()) || IsSignaled(Config->GetChangeSignal())) {
        //       InvalidatePainting(); UpdateChildren(); InvalidateChildrenLayout();
        //   }
        // Re-call combined-form accessor (B-014 precedent): idempotent.
        let chg_sig = self.config.borrow().GetChangeSignal(ectx);
        let cfg_changed = !chg_sig.is_null() && ectx.IsSignaled(chg_sig);
        let vfs_fired = ectx.IsSignaled(self.file_panel.GetVirFileStateSignal());
        if cfg_changed || vfs_fired {
            // Force rebuild on next observed_state==Loaded match-arm.
            // (Rust's `update_children` is gated on `child_count == 0`,
            // which doubles as the "needs rebuild" predicate.)
            self.child_count = 0;
        }

        // Observe the model. On Loaded, materialize children if not yet
        // built (child_count == 0 doubles as the "haven't built children
        // yet" predicate). On error, surface the message via file_panel.
        // While Loading or Waiting, return false — the panel is woken on
        // FileStateSignal fires via the D-006 subscribe path established
        // by emFilePanel::Cycle (B-015). The earlier stay_awake=true
        // workaround (F017 compensation for the never-fired signal) is
        // retired now that emFileModel<T> allocates its FileStateSignal
        // lazily (F019).
        let observed_state = self
            .dir_model
            .as_ref()
            .map(|dm| dm.borrow().get_file_state());
        match &observed_state {
            Some(FileState::Loaded) if self.child_count == 0 => {
                self.file_panel.clear_custom_error();
                self.update_children(ctx);
            }
            Some(FileState::LoadError(e)) => {
                self.file_panel.set_custom_error(e);
            }
            _ => {}
        }

        // Same-Cycle drain: set_custom_error / clear_custom_error above flip
        // pending_vir_state_fire; drain so VirFileStateSignal observers see
        // the fire this tick (mirrors C++ where the signal would have fired
        // synchronously inside emFilePanel::Cycle).
        self.file_panel.fire_pending_vir_state(ectx);

        // B-016 (3) MANDATORY emFilePanel::Cycle suffix — cycle_inner +
        // conditional fire. Mirrors emImageFileImageFilePanel.rs:232-235.
        let changed = self.file_panel.cycle_inner();
        if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
            ectx.fire(self.file_panel.GetVirFileStateSignal());
        }
        changed
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        // Alt+A: SelectAll
        if event.is_key(InputKey::Key('a')) && input_state.IsAltMod() {
            let mut sc = ctx
                .as_sched_ctx()
                .expect("emDirPanel::Input requires full PanelCtx reach for SelectAll");
            self.SelectAll(&mut sc);
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

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, ctx: &mut PanelCtx) {
        if flags.contains(NoticeFlags::VIEWING_CHANGED)
            || flags.contains(NoticeFlags::SOUGHT_NAME_CHANGED)
        {
            // C++ emDirPanel::Notice:
            //   if (IsViewed() || GetSoughtName()) {
            //     if (!GetFileModel()) SetFileModel(emDirModel::Acquire(...))
            //   } else if (GetFileModel()) SetFileModel(NULL)
            let keep_model = state.viewed || state.in_active_path;
            if keep_model {
                if self.dir_model.is_none() {
                    let dm_rc = emDirModel::Acquire(&self.ctx, &self.path);
                    if let Some(sched) = ctx.scheduler.as_deref_mut() {
                        emDirModel::ensure_engine_registered(&dm_rc, sched);
                    }
                    self.file_panel
                        .SetFileModel(Some(Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>));
                    self.dir_model = Some(dm_rc);
                    self.child_count = 0;
                    self.content_complete = false;
                }
            } else if let Some(dm_rc) = self.dir_model.take() {
                if let Some(sched) = ctx.scheduler.as_deref_mut() {
                    emDirModel::release_engine(&dm_rc, sched);
                }
                self.file_panel.SetFileModel(None);
                self.scroll_target = None;
            }
        }
    }

    fn IsOpaque(&self) -> bool {
        if self.is_model_loaded() {
            let cfg = self.config.borrow();
            let theme = cfg.GetTheme();
            let dc = theme.GetRec().DirContentColor;
            (dc >> 24) == 0xFF
        } else {
            false
        }
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        // Port of C++ emDirPanel::Paint (emDirPanel.cpp:159-170):
        //   switch (GetVirFileState()) {
        //   case VFS_LOADED:
        //   case VFS_NO_FILE_MODEL:
        //       painter.Clear(Config->GetTheme().DirContentColor.Get());
        //       break;
        //   default:
        //       emFilePanel::Paint(painter,canvasColor);
        //       break;
        //   }
        match self.file_panel.GetVirFileState() {
            VirtualFileState::Loaded | VirtualFileState::NoFileModel => {
                let cfg = self.config.borrow();
                let theme = cfg.GetTheme();
                let dc = emColor::from_packed(theme.GetRec().DirContentColor);
                painter.Clear(dc);
            }
            _ => {
                self.file_panel.Paint(painter, canvas_color, w, h, state);
            }
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

        let canvas_color = if self.is_model_loaded() {
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

    fn CreateControlPanel(
        &mut self,
        parent_ctx: &mut PanelCtx,
        name: &str,
        self_is_active: bool,
    ) -> Option<PanelId> {
        if !self_is_active {
            return None;
        } // C++: if (IsActive())
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
            Rc::clone(&dm_rc) as Rc<std::cell::RefCell<dyn FileModelState>>
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
        panel
            .file_panel
            .SetFileModel(Some(Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>));
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
            assert!(matches!(dm.get_file_state(), FileState::Loading { .. }));
        } // dm dropped here — borrow_mut released
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
    fn dump_state_reports_initial_loading_state() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emDirPanel::new(Rc::clone(&ctx), "/tmp/example".to_string());
        let pairs = PanelBehavior::dump_state(&panel);
        let map: std::collections::HashMap<&'static str, String> = pairs.into_iter().collect();
        assert_eq!(map.get("path").map(String::as_str), Some("/tmp/example"));
        assert_eq!(map.get("loading_done").map(String::as_str), Some("false"));
        assert_eq!(map.get("loading_error").map(String::as_str), Some(""));
        assert_eq!(
            map.get("content_complete").map(String::as_str),
            Some("false")
        );
        assert_eq!(map.get("child_count").map(String::as_str), Some("0"));
        assert_eq!(map.get("has_dir_model").map(String::as_str), Some("false"));
        assert_eq!(map.get("scroll_target").map(String::as_str), Some(""));
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
    // inside a notice handler (PanelCtx::with_sched_reach context, as used by
    // emView::HandleNotice → handle_notice_one) must get engine_id registered
    // and its PanelCycleEngine must fire in the same DoTimeSlice.
    //
    // This mirrors the production path that was broken: emDirEntryPanel::notice
    // → update_content_panel → create_child_with(emDirPanel) + wake_up_panel —
    // after which emDirPanel::Cycle never fired.

    /// Verifies that a child created inside a PanelCtx::with_sched_reach context
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

        // Simulate handle_notice_one's PanelCtx + update_content_panel.
        // with_sched_reach mirrors the full-reach context used in production
        // (handle_notice_one switched from with_scheduler to with_sched_reach
        // per the notice-dispatch-reach fix).
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let fw_cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let pa: std::rc::Rc<std::cell::RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                parent,
                1.0,
                &mut sched,
                &mut fw_actions,
                &emctx,
                &fw_cb,
                &pa,
            );
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
    /// Observable: after wake_up_panel + DoTimeSlice, the panel's Cycle has
    /// dispatched, registered the dir_model engine, and that model engine is
    /// awake driving the load — so sched.has_awake_engines() is true.
    /// (Pre-F019 the panel itself stayed awake via stay_awake=true polling;
    /// post-F019 the panel cycles only on FileStateSignal fires, but the
    /// model engine quiescence path keeps at least one engine awake during
    /// loading.)
    ///
    /// Failure (after notice_ctx_child_wake_up_panel_queues_engine passes) means
    /// the engine is woken but the scheduler never dispatches it:
    /// PanelScope::Toplevel window not found, or take_behavior returning None.
    #[test]
    fn notice_ctx_child_engine_fires_in_same_slice() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::{Cell, RefCell};
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
        // with_sched_reach mirrors the full-reach context used in production
        // (handle_notice_one switched from with_scheduler to with_sched_reach
        // per the notice-dispatch-reach fix).
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let fw_cb: std::cell::RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let pa: std::rc::Rc<std::cell::RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let child_id = {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                parent,
                1.0,
                &mut sched,
                &mut fw_actions,
                &emctx,
                &fw_cb,
                &pa,
            );
            let cid = ctx.create_child_with(
                "content",
                Box::new(emDirPanel::new(Rc::clone(&emctx), "/tmp".to_string())),
            );
            ctx.wake_up_panel(cid);
            cid
        };

        // F019 plan step 2.4: attach a cycle counter to the child's
        // PanelCycleEngine BEFORE the first slice, so we capture every
        // dispatch from the initial wake onward. Used below to assert
        // the panel does not re-cycle once FileStateSignal fires are
        // silenced — the polling-regression discriminator.
        let child_eid = tree
            .panel_engine_id_pub(child_id)
            .expect("emDirPanel child must have a registered PanelCycleEngine after wake_up_panel");
        let cycle_count: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        sched.attach_cycle_counter(child_eid, Rc::clone(&cycle_count));

        // Wrap tree in headless window so Toplevel(wid) dispatch finds it.
        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(RefCell::new(
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

        // After Cycle runs, it registers + wakes the dir_model engine
        // (driving the load); has_awake_engines() reflects that. If the
        // panel Cycle never ran (window scope mismatch / behavior
        // taken), no model engine got registered and has_awake_engines()
        // is false.
        assert!(
            sched.has_awake_engines(),
            "emDirPanel PanelCycleEngine must dispatch + register the dir_model engine; \
             has_awake_engines()=false means Cycle never ran — \
             check PanelScope::Toplevel wid={:?} window lookup or take_behavior returning None",
            wid
        );

        // F019 plan step 2.4 strengthening: snapshot the cycle counter
        // after the initial wake's dispatch, then silence the model
        // (release its engine + force state back to Waiting) and run a
        // generous slice budget. With FileStateSignal fires impossible
        // (no model engine), the post-fix panel must NOT re-cycle. A
        // polling regression (`Loading|Waiting => stay_awake=true` in
        // emDirPanel::Cycle) would re-queue the panel every slice while
        // state is Waiting, growing the counter by ~SLICE_BUDGET. This
        // belt-and-suspenders alongside the proof-of-fix test
        // `loading_dir_wakes_panel_via_filestatesignal_not_polling`.
        let after_initial = cycle_count.get();
        let dm = emDirModel::Acquire(&emctx, "/tmp");
        emDirModel::release_engine(&dm, &mut sched);
        dm.borrow_mut().force_state_waiting_for_test();
        const SLICE_BUDGET: u32 = 10;
        for _ in 0..SLICE_BUDGET {
            sched.DoTimeSlice(
                &mut windows,
                &emctx,
                &mut fw,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
            );
        }
        let after_silenced = cycle_count.get();
        let delta = after_silenced - after_initial;

        // Cleanup: reclaim tree from window and remove all panels.
        let mut win = windows.remove(&wid).unwrap();
        let mut reclaimed = win.take_tree();
        reclaimed.remove(root, Some(&mut sched));

        assert_eq!(
            delta, 0,
            "F019 regression: emDirPanel cycled {delta} additional times \
             across {SLICE_BUDGET} silenced slices (model engine released + \
             state forced to Waiting, so no FileStateSignal fire is possible). \
             Expected zero — any nonzero delta means the panel re-cycled \
             without a signal event, which is the exact stay_awake-while-loading \
             polling we retired. after_initial={after_initial} \
             after_silenced={after_silenced}",
        );
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
                        Box::new(emDirPanel::new(Rc::clone(&self.emctx), self.path.clone())),
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
                    view_context: None,
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
        // NoticeSpawner creates emDirPanel → wake_up_panel. Then Cycle
        // dispatches, registers + wakes the dir_model engine.
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
                // Something is still running — the model engine driving
                // the load satisfies has_awake_engines(). We can't
                // distinguish which engine is awake, but a true here
                // means at minimum the panel Cycle dispatched and
                // registered the model engine.
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
        // If emDirPanel Cycle ran, the dir_model engine got registered
        // and woken; with /tmp not yet finished loading by slice 5,
        // some engine remains awake.
        assert!(
            sched.has_awake_engines(),
            "After {n} DoTimeSlice calls through UpdateEngineClass → notice → create_child_with \
             + wake_up_panel, emDirPanel PanelCycleEngine must have dispatched (and the \
             dir_model engine it registers must still be loading). \
             has_awake_engines()=false means either:\n\
             (a) wake_up_panel found engine_id=None — engine not registered during notice\n\
             (b) PanelCycleEngine never dispatched despite being in queue (scope/window mismatch)\n\
             (c) load completed in fewer than {n} slices and all engines quiesced\n\
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
        let dm = emDirModel::Acquire(&emctx, "/tmp");
        emDirModel::release_engine(&dm, &mut sched);
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
                    view_context: None,
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
        let dm = emDirModel::Acquire(&emctx, "/tmp");
        emDirModel::release_engine(&dm, &mut sched);
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
            emctx.acquire::<emFpPluginList>("", || emFpPluginList::from_plugins(vec![dir_plugin]));
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
                    view_context: None,
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
                tree.queue_notice(root, NoticeFlags::SOUGHT_NAME_CHANGED, Some(&mut sched));
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
        // Inner emDirPanel was created via emDirFpPlugin with the entry's
        // path: emDirEntry::from_parent_and_name("", "/tmp") yields
        // GetPath() == "//tmp" (get_child_path joins "" + "/" + name).
        let dm = emDirModel::Acquire(&emctx, "//tmp");
        emDirModel::release_engine(&dm, &mut sched);
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
            emctx.acquire::<emFpPluginList>("", || emFpPluginList::from_plugins(vec![dir_plugin]));
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
                    view_context: None,
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
                tree.queue_notice(root, NoticeFlags::SOUGHT_NAME_CHANGED, Some(&mut sched));
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
                rect.w,
                rect.h,
            );
        }

        // Cleanup
        tree.remove(root, Some(&mut sched));
        for eid in view_engine_ids {
            sched.remove_engine(eid);
        }
        // Inner emDirPanel uses entry.GetPath() == "/" + path.
        let inner_path = format!("/{}", &path);
        let dm = emDirModel::Acquire(&emctx, &inner_path);
        emDirModel::release_engine(&dm, &mut sched);
        let _ = std::fs::remove_dir_all(&tmpdir);
    }

    /// F010 scaled-up test (step 2a from investigation): many entries (≥100).
    ///
    /// Mirrors the runtime failure scenario (directory with many entries that
    /// takes a long time to load then appears blank). The small 4-entry test
    /// passes; this exercises whether one-entry-per-Cycle loading interacts
    /// with child layout when the load completes after many slices.
    ///
    /// Asserts every child ends up with a non-zero, in-bounds layout_rect so
    /// an overflow or grid-compute failure at scale would surface here.
    #[test]
    fn real_stack_dir_panel_many_entries_all_have_nonzero_rects_after_load() {
        use emcore::emPanel::NoticeFlags;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::rc::Rc;

        use crate::emDirEntry::emDirEntry;
        use crate::emDirEntryPanel::emDirEntryPanel;
        use crate::emDirEntryPanel::CONTENT_NAME;

        let tmpdir = std::env::temp_dir().join("emfileman_f010_many_entries");
        let _ = std::fs::remove_dir_all(&tmpdir);
        std::fs::create_dir_all(&tmpdir).unwrap();
        let expected_entries: usize = 120;
        for i in 0..expected_entries {
            std::fs::write(tmpdir.join(format!("entry_{i:03}.txt")), b"x").unwrap();
        }
        let path = tmpdir.to_string_lossy().to_string();

        let emctx = emcore::emContext::emContext::NewRoot();
        {
            use emcore::emFpPlugin::{emFpPlugin, emFpPluginList};
            let dir_plugin = emFpPlugin::for_test_directory_handler(
                "emDir",
                crate::emDirFpPlugin::emDirFpPluginFunc,
            );
            emctx.acquire::<emFpPluginList>("", || emFpPluginList::from_plugins(vec![dir_plugin]));
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
                    view_context: None,
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
                tree.queue_notice(root, NoticeFlags::SOUGHT_NAME_CHANGED, Some(&mut sched));
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

        // ReadingNames + Sorting + LoadingEntries is O(N) slices; budget generously.
        for _ in 0..(expected_entries * 10 + 200) {
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

        let mut win = windows.remove(&wid).unwrap();
        let mut tree = win.take_tree();

        let dir_panel_id = tree
            .find_child_by_name(root, CONTENT_NAME)
            .expect("emDirPanel content child must exist after loading");

        let entry_children: Vec<_> = tree.children(dir_panel_id).collect();
        assert_eq!(
            entry_children.len(),
            expected_entries,
            "emDirPanel must have {expected_entries} entry children; got {}. \
             At scale, loading did not complete or children were not created.",
            entry_children.len(),
        );

        let mut zero_rect_count = 0usize;
        let mut out_of_bounds_count = 0usize;
        for cid in &entry_children {
            let rect = tree.layout_rect(*cid).expect("child layout rect");
            if rect.w <= 0.0 || rect.h <= 0.0 {
                zero_rect_count += 1;
            }
            // Entries live within the emDirPanel's 1x(panel_tallness) canvas.
            // rect.x, rect.y are panel-local; they must be finite and entries
            // shouldn't collapse onto (0,0) or extend beyond a plausible grid.
            if !rect.x.is_finite()
                || !rect.y.is_finite()
                || rect.x < -0.001
                || rect.y < -0.001
                || rect.x + rect.w > 2.0
            {
                out_of_bounds_count += 1;
            }
        }
        assert_eq!(
            zero_rect_count,
            0,
            "{zero_rect_count}/{} entries have zero-sized layout_rect at scale",
            entry_children.len(),
        );
        assert_eq!(
            out_of_bounds_count, 0,
            "{out_of_bounds_count}/{} entries have non-finite or out-of-bounds layout_rect at scale",
            entry_children.len(),
        );

        tree.remove(root, Some(&mut sched));
        for eid in view_engine_ids {
            sched.remove_engine(eid);
        }
        let inner_path = format!("/{}", &path);
        let dm = emDirModel::Acquire(&emctx, &inner_path);
        emDirModel::release_engine(&dm, &mut sched);
        let _ = std::fs::remove_dir_all(&tmpdir);
    }

    /// F010 Phase 1: state-gated Paint port. With a no-file-model dir panel,
    /// the C++ switch (emDirPanel.cpp:159-170) hits the `VFS_NO_FILE_MODEL`
    /// case and Clears with `DirContentColor`. Verify Paint fills the entire
    /// pixel buffer with DirContentColor.
    #[test]
    fn paint_clears_with_dir_content_color_when_no_file_model() {
        use emcore::emImage::emImage;
        use emcore::emPainter::emPainter;
        use emcore::emPanel::PanelState;

        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());

        // No SetFileModel — file_panel is in NoFileModel state.
        assert_eq!(
            panel.file_panel.GetVirFileState(),
            VirtualFileState::NoFileModel,
        );

        let dc = emColor::from_packed(panel.config.borrow().GetTheme().GetRec().DirContentColor);

        // 16x16 RGBA image filled with the canvas color so the canvas-blend
        // formula in Clear produces exact DirContentColor pixels.
        let mut img = emImage::new(16, 16, 4);
        img.fill(dc);
        {
            let mut p = emPainter::new(&mut img);
            let state = PanelState::default_for_test();
            panel.Paint(&mut p, dc, 1.0, 1.0, &state);
        }

        // Every pixel should be DirContentColor (Clear filled the whole rect).
        let map = img.GetMap();
        let r = ((dc.GetPacked() >> 24) & 0xFF) as u8;
        let g = ((dc.GetPacked() >> 16) & 0xFF) as u8;
        let b = ((dc.GetPacked() >> 8) & 0xFF) as u8;
        let a = (dc.GetPacked() & 0xFF) as u8;
        for px in map.chunks_exact(4) {
            assert_eq!(
                (px[0], px[1], px[2], px[3]),
                (r, g, b, a),
                "all pixels must equal DirContentColor after Clear",
            );
        }
    }

    /// F010 Phase 1: in a non-good state (Waiting), the C++ default arm
    /// delegates to `emFilePanel::Paint`, which calls `paint_status` — it
    /// emits status text only and does NOT Clear the full rect. Verify
    /// at least one pixel remains the sentinel color.
    #[test]
    fn paint_does_not_clear_when_waiting() {
        use emcore::emFileModel::FileModelState;
        use emcore::emImage::emImage;
        use emcore::emPainter::emPainter;
        use emcore::emPanel::PanelState;

        let ctx = emcore::emContext::emContext::NewRoot();
        let dm_rc = emDirModel::Acquire(&ctx, "/tmp");
        let mut panel = emDirPanel::new(Rc::clone(&ctx), "/tmp".to_string());
        panel
            .file_panel
            .SetFileModel(Some(Rc::clone(&dm_rc) as Rc<RefCell<dyn FileModelState>>));

        // Just-acquired model is in Waiting state.
        assert!(matches!(
            panel.file_panel.GetVirFileState(),
            VirtualFileState::Waiting,
        ));

        let dc = emColor::from_packed(panel.config.borrow().GetTheme().GetRec().DirContentColor);
        let sentinel = emColor::rgba(255, 0, 255, 255); // magenta — distinct from DirContentColor

        let mut img = emImage::new(16, 16, 4);
        img.fill(sentinel);
        {
            let mut p = emPainter::new(&mut img);
            let state = PanelState::default_for_test();
            panel.Paint(&mut p, sentinel, 1.0, 1.0, &state);
        }

        // No full-rect Clear should have happened. At least the corner pixel
        // (0,0) should remain the sentinel color (paint_status text is
        // centered, so the (0,0) corner is untouched).
        let map = img.GetMap();
        let dc_r = ((dc.GetPacked() >> 24) & 0xFF) as u8;
        let dc_g = ((dc.GetPacked() >> 16) & 0xFF) as u8;
        let dc_b = ((dc.GetPacked() >> 8) & 0xFF) as u8;
        let dc_a = (dc.GetPacked() & 0xFF) as u8;
        let corner = (map[0], map[1], map[2], map[3]);
        assert_ne!(
            corner,
            (dc_r, dc_g, dc_b, dc_a),
            "Waiting state must NOT Clear with DirContentColor (default arm delegates to paint_status)",
        );
    }

    /// F019 proof-of-fix: emDirPanel.Cycle invocations are bounded by
    /// state-change events (signal fires), not by scheduler slice count.
    ///
    /// Construction strategy: build the panel viewing /tmp, run one
    /// DoTimeSlice so the panel registers the model engine, then
    /// FORCIBLY remove that engine and reset the model state to
    /// Waiting. With the model engine gone, the model cannot transition
    /// state and cannot fire FileStateSignal. The only way the panel
    /// can be re-cycled across subsequent slices is via the
    /// `stay_awake`-while-loading polling we just retired.
    ///
    /// Pre-fix (F017): `Loading|Waiting => stay_awake=true` re-queued
    /// the panel every slice — cycle_count would grow ~1 per slice.
    /// Post-fix (F019): no signal fires → panel never re-wakes → after
    /// the initial wake's cycle, additional slices add zero.
    ///
    /// Threshold: post-fix cycle_count must be `<= initial_cycles`
    /// (the cycles run during the initial wake before counter
    /// snapshot); pre-fix would yield `initial + SLICE_BUDGET`.
    #[test]
    fn loading_dir_wakes_panel_via_filestatesignal_not_polling() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanelTree::PanelTree;
        use emcore::emScheduler::EngineScheduler;
        use std::cell::{Cell, RefCell};
        use std::collections::HashMap;
        use std::rc::Rc;

        let path = "/tmp".to_string();
        let emctx = emcore::emContext::emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", true);
        let mut sched = EngineScheduler::new();
        tree.register_engine_for_public(root, Some(&mut sched));
        let parent = tree.create_child(root, "parent", Some(&mut sched));

        // Create emDirPanel child + wake it. Mirrors
        // notice_ctx_child_engine_fires_in_same_slice fixture shape.
        // with_sched_reach mirrors the full-reach context used in production
        // (handle_notice_one switched from with_scheduler to with_sched_reach
        // per the notice-dispatch-reach fix).
        let child_id = {
            let mut notice_fw: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
            let notice_cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
                RefCell::new(None);
            let notice_pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree,
                parent,
                1.0,
                &mut sched,
                &mut notice_fw,
                &emctx,
                &notice_cb,
                &notice_pa,
            );
            let cid = ctx.create_child_with(
                "content",
                Box::new(emDirPanel::new(Rc::clone(&emctx), path.clone())),
            );
            ctx.wake_up_panel(cid);
            cid
        };

        let child_eid = tree
            .panel_engine_id_pub(child_id)
            .expect("emDirPanel child must have a registered PanelCycleEngine after wake_up_panel");
        let cycle_count: Rc<Cell<u32>> = Rc::new(Cell::new(0));
        sched.attach_cycle_counter(child_eid, Rc::clone(&cycle_count));

        let (wid, win) =
            emcore::test_view_harness::headless_emwindow_with_tree(&emctx, &mut sched, tree);
        let mut windows = HashMap::new();
        windows.insert(wid, win);

        let mut fw = Vec::new();
        let mut pending_inputs = Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let cb = RefCell::new(None::<Box<dyn emcore::emClipboard::emClipboard>>);
        let pa = Rc::new(RefCell::new(
            Vec::<emcore::emGUIFramework::DeferredAction>::new(),
        ));

        // Slice 1: initial wake — panel.Cycle creates dir_model + registers
        // its engine + connects FileStateSignal. Cycle counter records
        // every dispatch in this slice (could be multiple due to model
        // engine fires propagating back).
        sched.DoTimeSlice(
            &mut windows,
            &emctx,
            &mut fw,
            &mut pending_inputs,
            &mut input_state,
            &cb,
            &pa,
        );
        let after_initial = cycle_count.get();

        // Now silence the model: remove its engine and reset its state
        // to Waiting. With no model engine, the model cannot fire
        // FileStateSignal. The ONLY mechanism that could re-cycle the
        // panel across subsequent slices is the retired
        // stay_awake-while-loading polling.
        let dm = emDirModel::Acquire(&emctx, &path);
        emDirModel::release_engine(&dm, &mut sched);
        // Force state back to Waiting so polling-regression code path
        // (Loading|Waiting => stay_awake=true) would activate.
        dm.borrow_mut().force_state_waiting_for_test();

        // Run a generous slice budget. Without model engine fires, the
        // post-fix panel must NOT cycle again. A polling regression
        // would re-cycle the panel every slice — count would grow by
        // SLICE_BUDGET.
        const SLICE_BUDGET: u32 = 10;
        for _ in 0..SLICE_BUDGET {
            sched.DoTimeSlice(
                &mut windows,
                &emctx,
                &mut fw,
                &mut pending_inputs,
                &mut input_state,
                &cb,
                &pa,
            );
        }

        let after_silenced = cycle_count.get();
        let delta = after_silenced - after_initial;

        // Cleanup before assertion.
        let mut win = windows.remove(&wid).unwrap();
        let mut tree = win.take_tree();
        tree.remove(root, Some(&mut sched));

        // Assertion: zero additional cycles across SLICE_BUDGET silenced
        // slices. Polling regression would yield delta >= SLICE_BUDGET
        // (one re-cycle per slice while state is Waiting).
        assert_eq!(
            delta, 0,
            "F019 regression: emDirPanel cycled {delta} additional times \
             across {SLICE_BUDGET} silenced slices (no model fires \
             possible). Expected zero — any nonzero delta means the \
             panel re-cycled without a signal event, which is the \
             exact stay_awake-while-loading polling we retired. \
             after_initial={after_initial} after_silenced={after_silenced}",
        );
    }
}
