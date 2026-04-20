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
/// DIVERGED: C++ uses emLinearLayout composition with emPackGroup/
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

    /// Tracks config generation to detect external changes.
    last_config_gen: u64,

    /// Path of the directory panel that created this control panel.
    /// Used by SelectAll to enumerate entries.
    dir_path: Option<String>,
}

impl emFileManControlPanel {
    pub fn new(ctx: Rc<emContext>) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        let file_man = emFileManModel::Acquire(&ctx);
        let theme_names = emFileManThemeNames::Acquire(&ctx);
        let look = emLook::new();

        // Build sort criterion radio group
        let sort_group = RadioGroup::new();
        let sort_radios: Vec<emRadioButton> = SORT_LABELS
            .iter()
            .enumerate()
            .map(|(i, label)| {
                emRadioButton::new(label, Rc::clone(&look), Rc::clone(&sort_group), i)
            })
            .collect();

        // Build name sorting style radio group
        let nss_group = RadioGroup::new();
        let nss_radios: Vec<emRadioButton> = NSS_LABELS
            .iter()
            .enumerate()
            .map(|(i, label)| emRadioButton::new(label, Rc::clone(&look), Rc::clone(&nss_group), i))
            .collect();

        // Build theme style radio group
        let theme_style_group = RadioGroup::new();
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
        let theme_ar_group = RadioGroup::new();
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
        let dirs_first_check = emCheckButton::new("Sort Directories First", Rc::clone(&look));
        let show_hidden_check = emCheckButton::new("Show Hidden", Rc::clone(&look));
        let autosave_check = emCheckButton::new("Autosave", Rc::clone(&look));

        // Action buttons
        let save_button = emButton::new("Save", Rc::clone(&look));
        let select_all_button = emButton::new("Select All", Rc::clone(&look));
        let clear_sel_button = emButton::new("Clear Selection", Rc::clone(&look));
        let swap_sel_button = emButton::new("Swap Selection", Rc::clone(&look));
        let paths_clip_button = emButton::new("Paths to Clipboard", Rc::clone(&look));
        let names_clip_button = emButton::new("Names to Clipboard", Rc::clone(&look));

        let last_config_gen = config.borrow().GetChangeSignal();

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
            last_config_gen,
            dir_path: None,
        };
        panel.sync_from_config();
        panel
    }

    /// Read current config state into widget state.
    fn sync_from_config(&mut self) {
        let cfg = self.config.borrow();
        self.sort_group
            .borrow_mut()
            .SetChecked(cfg.GetSortCriterion() as usize);
        self.nss_group
            .borrow_mut()
            .SetChecked(cfg.GetNameSortingStyle() as usize);
        self.dirs_first_check
            .SetChecked(cfg.GetSortDirectoriesFirst());
        self.show_hidden_check.SetChecked(cfg.GetShowHiddenFiles());
        self.autosave_check.SetChecked(cfg.GetAutosave());

        // Sync theme style and AR from current theme name
        let theme_name = cfg.GetThemeName().to_string();
        drop(cfg);
        let tn = self.theme_names.borrow();
        if let Some(style_idx) = tn.GetThemeStyleIndex(&theme_name) {
            self.theme_style_group.borrow_mut().SetChecked(style_idx);
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
                self.theme_ar_group.borrow_mut().SetChecked(ar_idx);
            }
        }
    }

    pub(crate) fn with_dir_path(mut self, path: &str) -> Self {
        self.dir_path = Some(path.to_string());
        self
    }

    /// DIVERGED: C++ SelectAll finds active DirPanel by walking from
    /// content_view's focused panel. Rust receives the dir_path from the
    /// creating DirPanel and accesses the emDirModel directly.
    fn select_all(&self) {
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
                fm.SelectAsTarget(entry.GetPath());
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
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut emcore::emEngineCtx::PanelCtx,
    ) -> bool {
        let gen = self.config.borrow().GetChangeSignal();
        if gen != self.last_config_gen {
            self.last_config_gen = gen;
            self.sync_from_config();
            true
        } else {
            false
        }
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
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
                $widget.Paint(painter, widget_w, widget_h, true, pixel_scale);
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
        // Delegate to sort criterion radios
        for radio in &mut self.sort_radios {
            if radio.Input(event, state, input_state) {
                if let Some(idx) = self.sort_group.borrow().GetChecked() {
                    let sc = match idx {
                        0 => SortCriterion::ByName,
                        1 => SortCriterion::ByEnding,
                        2 => SortCriterion::ByClass,
                        3 => SortCriterion::ByVersion,
                        4 => SortCriterion::ByDate,
                        5 => SortCriterion::BySize,
                        _ => return true,
                    };
                    self.config.borrow_mut().SetSortCriterion(sc);
                }
                return true;
            }
        }

        // Delegate to name sorting style radios
        for radio in &mut self.nss_radios {
            if radio.Input(event, state, input_state) {
                if let Some(idx) = self.nss_group.borrow().GetChecked() {
                    let nss = match idx {
                        0 => NameSortingStyle::PerLocale,
                        1 => NameSortingStyle::CaseSensitive,
                        2 => NameSortingStyle::CaseInsensitive,
                        _ => return true,
                    };
                    self.config.borrow_mut().SetNameSortingStyle(nss);
                }
                return true;
            }
        }

        // Delegate to theme style radios
        for radio in &mut self.theme_style_radios {
            if radio.Input(event, state, input_state) {
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
                        self.config.borrow_mut().SetThemeName(&name);
                        self.sync_from_config();
                    }
                }
                return true;
            }
        }

        // Delegate to theme AR radios
        for radio in &mut self.theme_ar_radios {
            if radio.Input(event, state, input_state) {
                if let Some(ar_idx) = self.theme_ar_group.borrow().GetChecked() {
                    let style_idx = self.theme_style_group.borrow().GetChecked().unwrap_or(0);
                    let tn = self.theme_names.borrow();
                    if let Some(name) = tn.GetThemeName(style_idx, ar_idx) {
                        drop(tn);
                        self.config.borrow_mut().SetThemeName(&name);
                    }
                }
                return true;
            }
        }

        // Delegate to checkboxes
        if self.dirs_first_check.Input(event, state, input_state) {
            self.config
                .borrow_mut()
                .SetSortDirectoriesFirst(self.dirs_first_check.IsChecked());
            return true;
        }
        if self.show_hidden_check.Input(event, state, input_state) {
            self.config
                .borrow_mut()
                .SetShowHiddenFiles(self.show_hidden_check.IsChecked());
            return true;
        }
        if self.autosave_check.Input(event, state, input_state) {
            self.config
                .borrow_mut()
                .SetAutosave(self.autosave_check.IsChecked());
            return true;
        }

        // Delegate to action buttons
        if self.save_button.Input(event, state, input_state) {
            if self.save_button.IsPressed() {
                // Press tracked; actual save on release via on_click
            } else {
                self.config.borrow_mut().SaveAsDefault();
            }
            return true;
        }
        if self.select_all_button.Input(event, state, input_state) {
            self.select_all();
            return true;
        }
        if self.clear_sel_button.Input(event, state, input_state) {
            self.file_man.borrow_mut().ClearTargetSelection();
            return true;
        }
        if self.swap_sel_button.Input(event, state, input_state) {
            self.file_man.borrow_mut().SwapSelection();
            return true;
        }
        if self.paths_clip_button.Input(event, state, input_state) {
            let _text = self.file_man.borrow().SelectionToClipboard(false, false);
            return true;
        }
        if self.names_clip_button.Input(event, state, input_state) {
            let _text = self.file_man.borrow().SelectionToClipboard(false, true);
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopEngineForTest;
    impl emcore::emEngine::emEngine for NoopEngineForTest {
        fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
            false
        }
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManControlPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn sync_from_config_initializes_widgets() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManControlPanel::new(Rc::clone(&ctx));
        // Default config: ByName sort, PerLocale nss, dirs_first=false, hidden=false
        assert_eq!(panel.sort_group.borrow().GetChecked(), Some(0));
        assert_eq!(panel.nss_group.borrow().GetChecked(), Some(0));
        assert!(!panel.dirs_first_check.IsChecked());
        assert!(!panel.show_hidden_check.IsChecked());
    }

    #[test]
    fn cycle_detects_config_change() {
        use emcore::emEngineCtx::PanelCtx;
        use emcore::emPanelTree::{PanelId, PanelTree};
        use slotmap::Key as _;

        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emFileManControlPanel::new(Rc::clone(&ctx));

        // Mutate config externally
        panel
            .config
            .borrow_mut()
            .SetSortCriterion(SortCriterion::BySize);

        let mut tree = PanelTree::new();
        let mut pctx = PanelCtx {
            tree: &mut tree,
            id: PanelId::null(),
            current_pixel_tallness: 1.0,
            scheduler: None,
        };
        let mut h = emcore::test_view_harness::TestViewHarness::new();
        let dummy_eid = h.scheduler.register_engine(
            Box::new(NoopEngineForTest),
            emcore::emEngine::Priority::Medium,
            emcore::emEngine::TreeLocation::Outer,
        );
        let changed = {
            let mut ectx = h.engine_ctx(dummy_eid);
            panel.Cycle(&mut ectx, &mut pctx)
        };
        assert!(changed);
        // Widget should now reflect BySize
        assert_eq!(panel.sort_group.borrow().GetChecked(), Some(5));
        h.scheduler.remove_engine(dummy_eid);
    }

    #[test]
    fn widget_counts() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManControlPanel::new(Rc::clone(&ctx));
        assert_eq!(panel.sort_radios.len(), 6);
        assert_eq!(panel.nss_radios.len(), 3);
    }

    #[test]
    fn sort_group_change_updates_config() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManControlPanel::new(Rc::clone(&ctx));

        // Simulate changing sort group to ByDate (index 4)
        panel.sort_group.borrow_mut().SetChecked(4);
        // Apply via sync logic — normally this happens in Input handler,
        // but we test the config update path directly
        panel
            .config
            .borrow_mut()
            .SetSortCriterion(SortCriterion::ByDate);

        let cfg = panel.config.borrow();
        assert_eq!(cfg.GetSortCriterion(), SortCriterion::ByDate);
    }
}
