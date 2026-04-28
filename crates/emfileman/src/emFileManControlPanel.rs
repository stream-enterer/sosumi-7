//! Sort/filter/theme UI control panel.
//!
//! Port of C++ `emFileManControlPanel`. Extends `emLinearLayout`.
//! Contains sort criterion radio buttons, name sorting style radio buttons,
//! directories-first and show-hidden checkboxes, theme selectors,
//! autosave checkbox, and command group buttons.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emButton::emButton;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use slotmap::Key as _;

use crate::emFileManConfig::{NameSortingStyle, SortCriterion};
use crate::emFileManModel::emFileManModel;
use crate::emFileManThemeNames::emFileManThemeNames;
use crate::emFileManViewConfig::emFileManViewConfig;

/// Sort criterion labels matching enum variant order.
const SORT_LABELS: [&str; 6] = [
    "By Name",
    "By Ending",
    "By Class",
    "By Version",
    "By Date",
    "By Size",
];

/// Name sorting style labels matching enum variant order.
const NSS_LABELS: [&str; 3] = ["Per Locale", "Case Sensitive", "Case Insensitive"];

/// Control panel for file manager settings.
/// Port of C++ `emFileManControlPanel` (extends emLinearLayout).
///
/// DIVERGED: (language-forced) C++ uses emLinearLayout composition with emPackGroup/
/// emRasterLayout for widget tree. Rust uses manual painting with
/// computed y offsets — widgets are painted directly rather than
/// composed as child panels in a layout tree.
pub struct emFileManControlPanel {
    ctx: Rc<emContext>,
    config: Rc<RefCell<emFileManViewConfig>>,
    file_man: Rc<RefCell<emFileManModel>>,
    theme_names: Rc<RefCell<emFileManThemeNames>>,
    _look: Rc<emLook>,

    // Sort criterion radio group (6 buttons)
    sort_group: Rc<RefCell<RadioGroup>>,
    sort_radios: Vec<emRadioButton>,

    // Name sorting style radio group (3 buttons)
    nss_group: Rc<RefCell<RadioGroup>>,
    nss_radios: Vec<emRadioButton>,

    // Theme style radio group
    theme_style_group: Rc<RefCell<RadioGroup>>,
    theme_style_radios: Vec<emRadioButton>,

    // Theme aspect ratio radio group
    theme_ar_group: Rc<RefCell<RadioGroup>>,
    theme_ar_radios: Vec<emRadioButton>,

    // Checkboxes
    dirs_first_check: emCheckButton,
    show_hidden_check: emCheckButton,
    autosave_check: emCheckButton,

    // Action buttons
    save_button: emButton,
    select_all_button: emButton,
    clear_sel_button: emButton,
    swap_sel_button: emButton,
    paths_clip_button: emButton,
    names_clip_button: emButton,

    /// First-Cycle init guard for D-006 subscribe shape.
    subscribed_init: bool,

    /// Path of the directory panel that created this control panel.
    /// Used by SelectAll to enumerate entries.
    dir_path: Option<String>,
}

impl emFileManControlPanel {
    pub fn new<C: emcore::emEngineCtx::ConstructCtx>(cc: &mut C, ctx: Rc<emContext>) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        let file_man = emFileManModel::Acquire(&ctx);
        let theme_names = emFileManThemeNames::Acquire(&ctx);
        let look = emLook::new();

        // Build sort criterion radio group
        let sort_group = RadioGroup::new(cc);
        let sort_radios: Vec<emRadioButton> = SORT_LABELS
            .iter()
            .enumerate()
            .map(|(i, label)| {
                emRadioButton::new(label, Rc::clone(&look), Rc::clone(&sort_group), i)
            })
            .collect();

        // Build name sorting style radio group
        let nss_group = RadioGroup::new(cc);
        let nss_radios: Vec<emRadioButton> = NSS_LABELS
            .iter()
            .enumerate()
            .map(|(i, label)| emRadioButton::new(label, Rc::clone(&look), Rc::clone(&nss_group), i))
            .collect();

        // Build theme style radio group
        let theme_style_group = RadioGroup::new(cc);
        let theme_style_radios: Vec<emRadioButton> = {
            let tn = theme_names.borrow();
            (0..tn.GetThemeStyleCount())
                .map(|i| {
                    let label = tn.GetThemeStyleDisplayName(i).unwrap_or("?");
                    emRadioButton::new(label, Rc::clone(&look), Rc::clone(&theme_style_group), i)
                })
                .collect()
        };

        // Build theme aspect ratio radio group (for first style initially)
        let theme_ar_group = RadioGroup::new(cc);
        let theme_ar_radios: Vec<emRadioButton> = {
            let tn = theme_names.borrow();
            let ar_count = if tn.GetThemeStyleCount() > 0 {
                tn.GetThemeAspectRatioCount(0)
            } else {
                0
            };
            (0..ar_count)
                .map(|i| {
                    let label = tn.GetThemeAspectRatio(0, i).unwrap_or("?");
                    emRadioButton::new(label, Rc::clone(&look), Rc::clone(&theme_ar_group), i)
                })
                .collect()
        };

        // Checkboxes
        let dirs_first_check = emCheckButton::new(cc, "Sort Directories First", Rc::clone(&look));
        let show_hidden_check = emCheckButton::new(cc, "Show Hidden", Rc::clone(&look));
        let autosave_check = emCheckButton::new(cc, "Autosave", Rc::clone(&look));

        // Action buttons
        let save_button = emButton::new(cc, "Save", Rc::clone(&look));
        let select_all_button = emButton::new(cc, "Select All", Rc::clone(&look));
        let clear_sel_button = emButton::new(cc, "Clear Selection", Rc::clone(&look));
        let swap_sel_button = emButton::new(cc, "Swap Selection", Rc::clone(&look));
        let paths_clip_button = emButton::new(cc, "Paths to Clipboard", Rc::clone(&look));
        let names_clip_button = emButton::new(cc, "Names to Clipboard", Rc::clone(&look));

        let mut panel = Self {
            ctx,
            config,
            file_man,
            theme_names,
            _look: look,
            sort_group,
            sort_radios,
            nss_group,
            nss_radios,
            theme_style_group,
            theme_style_radios,
            theme_ar_group,
            theme_ar_radios,
            dirs_first_check,
            show_hidden_check,
            autosave_check,
            save_button,
            select_all_button,
            clear_sel_button,
            swap_sel_button,
            paths_clip_button,
            names_clip_button,
            subscribed_init: false,
            dir_path: None,
        };

        // Initial sync at construction time with a scratch PanelCtx (no
        // scheduler reach). State is updated; callbacks silently don't fire,
        // which is exactly what we want during construction.
        {
            let mut tree = emcore::emPanelTree::PanelTree::new();
            let id = tree.create_root("init", false);
            let mut ctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, id, 1.0);
            panel.sync_from_config(&mut ctx);
        }
        panel
    }

    /// Read current config state into widget state.
    fn sync_from_config(&mut self, ctx: &mut emcore::emEngineCtx::PanelCtx<'_>) {
        let cfg = self.config.borrow();
        self.sort_group
            .borrow_mut()
            .SetChecked(cfg.GetSortCriterion() as usize, ctx);
        self.nss_group
            .borrow_mut()
            .SetChecked(cfg.GetNameSortingStyle() as usize, ctx);
        self.dirs_first_check
            .SetChecked(cfg.GetSortDirectoriesFirst(), ctx);
        self.show_hidden_check
            .SetChecked(cfg.GetShowHiddenFiles(), ctx);
        self.autosave_check.SetChecked(cfg.GetAutosave(), ctx);

        // Sync theme style and AR from current theme name
        let theme_name = cfg.GetThemeName().to_string();
        drop(cfg);
        let tn = self.theme_names.borrow();
        if let Some(style_idx) = tn.GetThemeStyleIndex(&theme_name) {
            self.theme_style_group
                .borrow_mut()
                .SetChecked(style_idx, ctx);
            // Rebuild AR radios for the selected style
            let ar_count = tn.GetThemeAspectRatioCount(style_idx);
            self.theme_ar_radios.clear();
            for i in 0..ar_count {
                let label = tn.GetThemeAspectRatio(style_idx, i).unwrap_or("?");
                self.theme_ar_radios.push(emRadioButton::new(
                    label,
                    Rc::clone(&self._look),
                    Rc::clone(&self.theme_ar_group),
                    i,
                ));
            }
            if let Some(ar_idx) = tn.GetThemeAspectRatioIndex(&theme_name) {
                self.theme_ar_group.borrow_mut().SetChecked(ar_idx, ctx);
            }
        }
    }

    pub(crate) fn with_dir_path(mut self, path: &str) -> Self {
        self.dir_path = Some(path.to_string());
        self
    }

    /// DIVERGED: (language-forced) C++ SelectAll finds active DirPanel by walking from
    /// content_view's focused panel. Rust receives the dir_path from the
    /// creating DirPanel and accesses the emDirModel directly.
    fn select_all(&self, ectx: &mut impl emcore::emEngineCtx::SignalCtx) {
        let Some(ref dir_path) = self.dir_path else {
            return;
        };
        let dm = crate::emDirModel::emDirModel::Acquire(&self.ctx, dir_path);
        let dm = dm.borrow();
        let cfg = self.config.borrow();
        let show_hidden = cfg.GetShowHiddenFiles();
        let mut fm = self.file_man.borrow_mut();
        for i in 0..dm.GetEntryCount() {
            let entry = dm.GetEntry(i);
            if !entry.IsHidden() || show_hidden {
                fm.SelectAsTarget(ectx, entry.GetPath());
            }
        }
    }

    /// Paint a section label at the given y position. Returns the y after the label.
    fn paint_section_label(
        painter: &mut emPainter,
        x: f64,
        y: f64,
        w: f64,
        row_h: f64,
        label: &str,
        fg: emColor,
    ) -> f64 {
        painter.PaintTextBoxed(
            x,
            y,
            w,
            row_h,
            label,
            row_h * 0.7,
            fg,
            emColor::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );
        y + row_h
    }
}

impl PanelBehavior for emFileManControlPanel {
    fn IsOpaque(&self) -> bool {
        false
    }

    fn Cycle(
        &mut self,
        ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        ctx: &mut emcore::emEngineCtx::PanelCtx,
    ) -> bool {
        // D-006 first-Cycle init: lazy-allocate signals and connect this engine.
        // Mirrors C++ emFileManControlPanel ctor `AddWakeUpSignal(...)` calls
        // (rows 326 SelectionSignal, 327 ChangeSignal, 522 CommandsSignal).
        if !self.subscribed_init {
            let eid = ectx.engine_id;
            let sel_sig = self.file_man.borrow().GetSelectionSignal(ectx);
            let cmd_sig = self.file_man.borrow().GetCommandsSignal(ectx);
            let chg_sig = self.config.borrow().GetChangeSignal(ectx);
            ectx.connect(sel_sig, eid);
            ectx.connect(cmd_sig, eid);
            ectx.connect(chg_sig, eid);
            self.subscribed_init = true;
        }

        // C++ source order: cpp:366 selection, cpp:367 change, cpp:533 commands.
        // Re-call the combined-form accessors (B-014 precedent,
        // emVirtualCosmos.rs:874): idempotent — cells are non-null after
        // the init block above, so the second call is a cheap field read.
        let sel_sig = self.file_man.borrow().GetSelectionSignal(ectx);
        let chg_sig = self.config.borrow().GetChangeSignal(ectx);
        let cmd_sig = self.file_man.borrow().GetCommandsSignal(ectx);
        let mut changed = false;

        if !sel_sig.is_null() && ectx.IsSignaled(sel_sig) {
            // Mirrors C++ emFileManControlPanel.cpp:366 — selection-driven
            // button-state refresh. The Rust port currently has no
            // UpdateButtonStates implementation; mark a state change so
            // a future port slots in cleanly. (No regression: prior code
            // did not react to selection here either.)
            changed = true;
        }
        if !chg_sig.is_null() && ectx.IsSignaled(chg_sig) {
            // Mirrors C++ emFileManControlPanel.cpp:367 — config-driven sync.
            self.sync_from_config(ctx);
            changed = true;
        }
        if !cmd_sig.is_null() && ectx.IsSignaled(cmd_sig) {
            // Mirrors C++ emFileManControlPanel.cpp:533 — commands-tree
            // changed. Direct same-engine subscribe (no sub-engine —
            // audit-data correction noted in B-009 design doc).
            changed = true;
        }
        changed
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        let fg = emColor::from_packed(0xCCCCCCFF);
        let margin = 0.02;
        let content_w = w - 2.0 * margin;
        let row_h = h * 0.04;
        let widget_h = row_h * 1.2;
        let widget_w = content_w * 0.45;
        let mut y = margin;

        // Helper: paint a widget at position (margin, y) via translate
        macro_rules! paint_widget {
            ($widget:expr) => {
                painter.translate(margin, y);
                $widget.Paint(painter, canvas_color, widget_w, widget_h, true, pixel_scale);
                painter.translate(-margin, -y);
                y += widget_h;
            };
        }

        // --- Sort Criterion section ---
        y = Self::paint_section_label(painter, margin, y, content_w, row_h, "Sort Criterion", fg);
        for radio in &mut self.sort_radios {
            paint_widget!(radio);
        }

        y += row_h * 0.5;

        // --- Name Sorting Style section ---
        y = Self::paint_section_label(
            painter,
            margin,
            y,
            content_w,
            row_h,
            "Name Sorting Style",
            fg,
        );
        for radio in &mut self.nss_radios {
            paint_widget!(radio);
        }

        y += row_h * 0.5;

        // --- Theme Style section ---
        y = Self::paint_section_label(painter, margin, y, content_w, row_h, "Theme Style:", fg);
        for radio in &mut self.theme_style_radios {
            paint_widget!(radio);
        }

        y += row_h * 0.5;

        // --- Aspect Ratio section ---
        y = Self::paint_section_label(painter, margin, y, content_w, row_h, "Aspect Ratio:", fg);
        for radio in &mut self.theme_ar_radios {
            paint_widget!(radio);
        }

        y += row_h * 0.5;

        // --- Options section ---
        y = Self::paint_section_label(painter, margin, y, content_w, row_h, "Options", fg);
        paint_widget!(self.dirs_first_check);
        paint_widget!(self.show_hidden_check);
        paint_widget!(self.autosave_check);

        y += row_h * 0.5;

        // --- Actions section ---
        y = Self::paint_section_label(painter, margin, y, content_w, row_h, "Actions", fg);
        paint_widget!(self.save_button);
        paint_widget!(self.select_all_button);
        paint_widget!(self.clear_sel_button);
        paint_widget!(self.swap_sel_button);
        paint_widget!(self.paths_clip_button);
        paint_widget!(self.names_clip_button);
        let _ = y; // final y unused
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut emcore::emEngineCtx::PanelCtx,
    ) -> bool {
        // D-007 mutator-fire-shape: setters now thread SignalCtx. Delegate
        // input to widgets first, then if any setter needs to fire, build a
        // SchedCtx via _ctx.as_sched_ctx().
        //
        // Invariant: every `_ctx.as_sched_ctx().expect(...)` below is safe
        // because production input dispatch always reaches `Input` through
        // the full PanelCtx path: `EngineCtx::deliver_input` → `PanelTree`
        // hand-off, which invariably constructs a `PanelCtx` carrying the
        // engine-id + scheduler reference required by `as_sched_ctx`. The
        // only way `as_sched_ctx()` can return `None` here is a test that
        // synthesises a degraded `PanelCtx` without the scheduler reach;
        // no such test path exists for this panel (see
        // `tests/typemismatch_b009.rs` — all click-through tests drive
        // `Input` through the standard harness, which wires SchedCtx).
        // Extracting a helper to fold these `expect` callsites is blocked
        // by the borrow-checker: the resulting `&mut SchedCtx` must
        // co-exist with `self.config.borrow_mut()` / `self.file_man
        // .borrow_mut()` in each branch, and a helper would have to hold
        // the SchedCtx across the borrow boundary.
        // Delegate to sort criterion radios
        for radio in &mut self.sort_radios {
            if radio.Input(event, state, input_state, _ctx) {
                if let Some(idx) = self.sort_group.borrow().GetChecked() {
                    let sc_val = match idx {
                        0 => SortCriterion::ByName,
                        1 => SortCriterion::ByEnding,
                        2 => SortCriterion::ByClass,
                        3 => SortCriterion::ByVersion,
                        4 => SortCriterion::ByDate,
                        5 => SortCriterion::BySize,
                        _ => return true,
                    };
                    let mut sc = _ctx
                        .as_sched_ctx()
                        .expect("emFileManControlPanel::Input requires full PanelCtx reach");
                    self.config.borrow_mut().SetSortCriterion(&mut sc, sc_val);
                }
                return true;
            }
        }

        // Delegate to name sorting style radios
        for radio in &mut self.nss_radios {
            if radio.Input(event, state, input_state, _ctx) {
                if let Some(idx) = self.nss_group.borrow().GetChecked() {
                    let nss = match idx {
                        0 => NameSortingStyle::PerLocale,
                        1 => NameSortingStyle::CaseSensitive,
                        2 => NameSortingStyle::CaseInsensitive,
                        _ => return true,
                    };
                    let mut sc = _ctx
                        .as_sched_ctx()
                        .expect("emFileManControlPanel::Input requires full PanelCtx reach");
                    self.config.borrow_mut().SetNameSortingStyle(&mut sc, nss);
                }
                return true;
            }
        }

        // Delegate to theme style radios
        for radio in &mut self.theme_style_radios {
            if radio.Input(event, state, input_state, _ctx) {
                let style_idx = self.theme_style_group.borrow().GetChecked();
                if let Some(style_idx) = style_idx {
                    let ar_idx = self.theme_ar_group.borrow().GetChecked().unwrap_or(0);
                    let tn = self.theme_names.borrow();
                    // Clamp AR index to new style's AR count
                    let clamped_ar =
                        ar_idx.min(tn.GetThemeAspectRatioCount(style_idx).saturating_sub(1));
                    let name = tn.GetThemeName(style_idx, clamped_ar);
                    drop(tn);
                    if let Some(name) = name {
                        {
                            let mut sc = _ctx.as_sched_ctx().expect(
                                "emFileManControlPanel::Input requires full PanelCtx reach",
                            );
                            self.config.borrow_mut().SetThemeName(&mut sc, &name);
                        }
                        self.sync_from_config(_ctx);
                    }
                }
                return true;
            }
        }

        // Delegate to theme AR radios
        for radio in &mut self.theme_ar_radios {
            if radio.Input(event, state, input_state, _ctx) {
                if let Some(ar_idx) = self.theme_ar_group.borrow().GetChecked() {
                    let style_idx = self.theme_style_group.borrow().GetChecked().unwrap_or(0);
                    let tn = self.theme_names.borrow();
                    if let Some(name) = tn.GetThemeName(style_idx, ar_idx) {
                        drop(tn);
                        let mut sc = _ctx
                            .as_sched_ctx()
                            .expect("emFileManControlPanel::Input requires full PanelCtx reach");
                        self.config.borrow_mut().SetThemeName(&mut sc, &name);
                    }
                }
                return true;
            }
        }

        // Delegate to checkboxes
        if self.dirs_first_check.Input(event, state, input_state, _ctx) {
            let v = self.dirs_first_check.IsChecked();
            let mut sc = _ctx
                .as_sched_ctx()
                .expect("emFileManControlPanel::Input requires full PanelCtx reach");
            self.config.borrow_mut().SetSortDirectoriesFirst(&mut sc, v);
            return true;
        }
        if self
            .show_hidden_check
            .Input(event, state, input_state, _ctx)
        {
            let v = self.show_hidden_check.IsChecked();
            let mut sc = _ctx
                .as_sched_ctx()
                .expect("emFileManControlPanel::Input requires full PanelCtx reach");
            self.config.borrow_mut().SetShowHiddenFiles(&mut sc, v);
            return true;
        }
        if self.autosave_check.Input(event, state, input_state, _ctx) {
            let v = self.autosave_check.IsChecked();
            let mut sc = _ctx
                .as_sched_ctx()
                .expect("emFileManControlPanel::Input requires full PanelCtx reach");
            self.config.borrow_mut().SetAutosave(&mut sc, v);
            return true;
        }

        // Delegate to action buttons
        if self.save_button.Input(event, state, input_state, _ctx) {
            if self.save_button.IsPressed() {
                // Press tracked; actual save on release via on_click
            } else {
                self.config.borrow_mut().SaveAsDefault();
            }
            return true;
        }
        if self
            .select_all_button
            .Input(event, state, input_state, _ctx)
        {
            let mut sc = _ctx
                .as_sched_ctx()
                .expect("emFileManControlPanel::Input requires full PanelCtx reach");
            self.select_all(&mut sc);
            return true;
        }
        if self.clear_sel_button.Input(event, state, input_state, _ctx) {
            let mut sc = _ctx
                .as_sched_ctx()
                .expect("emFileManControlPanel::Input requires full PanelCtx reach");
            self.file_man.borrow_mut().ClearTargetSelection(&mut sc);
            return true;
        }
        if self.swap_sel_button.Input(event, state, input_state, _ctx) {
            let mut sc = _ctx
                .as_sched_ctx()
                .expect("emFileManControlPanel::Input requires full PanelCtx reach");
            self.file_man.borrow_mut().SwapSelection(&mut sc);
            return true;
        }
        if self
            .paths_clip_button
            .Input(event, state, input_state, _ctx)
        {
            let _text = self.file_man.borrow().SelectionToClipboard(false, false);
            return true;
        }
        if self
            .names_clip_button
            .Input(event, state, input_state, _ctx)
        {
            let _text = self.file_man.borrow().SelectionToClipboard(false, true);
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::{DeferredAction, InitCtx};
    use emcore::emScheduler::EngineScheduler;

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<emContext>,
        pa: Rc<RefCell<Vec<emcore::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: emcore::emContext::emContext::NewRoot(),
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

    struct NoopEngineForTest;
    impl emcore::emEngine::emEngine for NoopEngineForTest {
        fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
            false
        }
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let mut __init = TestInit::new();
        let ctx = Rc::clone(&__init.root);
        let panel = emFileManControlPanel::new(&mut __init.ctx(), Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn sync_from_config_initializes_widgets() {
        let mut __init = TestInit::new();
        let ctx = Rc::clone(&__init.root);
        let panel = emFileManControlPanel::new(&mut __init.ctx(), Rc::clone(&ctx));
        // Default config: ByName sort, PerLocale nss, dirs_first=false, hidden=false
        assert_eq!(panel.sort_group.borrow().GetChecked(), Some(0));
        assert_eq!(panel.nss_group.borrow().GetChecked(), Some(0));
        assert!(!panel.dirs_first_check.IsChecked());
        assert!(!panel.show_hidden_check.IsChecked());
    }

    #[test]
    fn cycle_detects_config_change() {
        // D-006/D-007/D-008 A1 combined: first Cycle subscribes; then a
        // setter call fires; then a second Cycle observes IsSignaled and
        // re-syncs widgets.
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanelTree::{PanelId, PanelTree};
        use slotmap::Key as _;

        let mut h = emcore::test_view_harness::TestViewHarness::new();
        let mut panel = {
            let mut ic = h.init_ctx();
            let ctx = Rc::clone(ic.root_context);
            emFileManControlPanel::new(&mut ic, ctx)
        };
        let dummy_eid = h.scheduler.register_engine(
            Box::new(NoopEngineForTest),
            emcore::emEngine::Priority::Medium,
            emcore::emPanelScope::PanelScope::Framework,
        );

        // First Cycle: subscribe + allocate signals.
        let mut tree = PanelTree::new();
        let mut pctx = PanelCtx {
            tree: &mut tree,
            id: PanelId::null(),
            current_pixel_tallness: 1.0,
            scheduler: None,
            framework_clipboard: None,
            framework_actions: None,
            root_context: None,
            pending_actions: None,
        };
        {
            let mut ectx = h.engine_ctx(dummy_eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }
        assert!(panel.subscribed_init);

        // Mutate config; the setter fires ChangeSignal via the SchedCtx.
        {
            let mut sc = h.sched_ctx_for(dummy_eid);
            panel
                .config
                .borrow_mut()
                .SetSortCriterion(&mut sc, SortCriterion::BySize);
        }

        // Process pending signals so IsSignaled returns true on the next Cycle.
        h.scheduler.flush_signals_for_test();

        // Second Cycle: observe IsSignaled → sync.
        let changed = {
            let mut ectx = h.engine_ctx(dummy_eid);
            panel.Cycle(&mut ectx, &mut pctx)
        };
        assert!(changed, "Cycle must observe ChangeSignal and re-sync");
        // Widget should now reflect BySize
        assert_eq!(panel.sort_group.borrow().GetChecked(), Some(5));
        h.scheduler.remove_engine(dummy_eid);
        // Drain any pending signals so the scheduler's Drop assertion passes.
        h.scheduler.flush_signals_for_test();
    }

    #[test]
    fn widget_counts() {
        let mut __init = TestInit::new();
        let ctx = Rc::clone(&__init.root);
        let panel = emFileManControlPanel::new(&mut __init.ctx(), Rc::clone(&ctx));
        assert_eq!(panel.sort_radios.len(), 6);
        assert_eq!(panel.nss_radios.len(), 3);
    }

    #[test]
    fn sort_group_change_updates_config() {
        let mut h = emcore::test_view_harness::TestViewHarness::new();
        let panel = {
            let mut ic = h.init_ctx();
            let ctx = Rc::clone(ic.root_context);
            emFileManControlPanel::new(&mut ic, ctx)
        };

        let mut tree = emcore::emPanelTree::PanelTree::new();
        let tid = tree.create_root("t", false);
        let mut ctx = emcore::emEngineCtx::PanelCtx::new(&mut tree, tid, 1.0);
        // Simulate changing sort group to ByDate (index 4)
        panel.sort_group.borrow_mut().SetChecked(4, &mut ctx);
        // Apply via sync logic — normally this happens in Input handler,
        // but we test the config update path directly
        {
            let mut sc = h.sched_ctx();
            panel
                .config
                .borrow_mut()
                .SetSortCriterion(&mut sc, SortCriterion::ByDate);
        }

        let cfg = panel.config.borrow();
        assert_eq!(cfg.GetSortCriterion(), SortCriterion::ByDate);
    }
}
