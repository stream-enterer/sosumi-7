use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::dlog;
use crate::emColor::emColor;
use crate::emPanel::{PanelBehavior, PanelState};
use crate::emPanelCtx::PanelCtx;
use crate::emPanel::NoticeFlags;
use crate::emPanelTree::PanelId;
use crate::emPainter::emPainter;
use crate::emStroke::emStroke;

use super::emBorder::{emBorder, InnerBorderType, OuterBorderType, with_toolkit_images};
use crate::emCheckBox::emCheckBox;
use super::emColorFieldFieldPanel::{CheckBoxPanel, ListBoxPanel, TextFieldPanel};
use super::emListBox::{emListBox, SelectionMode};
use crate::emLook::emLook;
use crate::emTextField::emTextField;
use crate::emTexture::ImageExtension;

/// Data associated with each file entry in the listing.
#[derive(Clone, Debug)]
pub struct FileItemData {
    pub is_directory: bool,
    pub is_readable: bool,
    pub is_hidden: bool,
}

/// Panel behavior for a single file/directory item inside the file list.
///
/// Port of C++ `emFileSelectionBox::FileItemPanel::Paint` (lines 958-1062).
/// Renders:
/// 1. Selection highlight (round rect) when selected
/// 2. Filename text (center-aligned, bottom region)
/// 3. Directory icon (colored rectangle) or nothing for regular files
/// 4. "Parent Directory" label for ".." entries
/// 5. Not-readable indicator (circle + diagonal line)
struct FileItemPanelBehavior {
    name: String,
    is_directory: bool,
    is_readable: bool,
    is_selected: bool,
    look: Rc<emLook>,
    selection_mode: SelectionMode,
    enabled: bool,
}

impl FileItemPanelBehavior {
    fn new(
        name: String,
        is_directory: bool,
        is_readable: bool,
        is_selected: bool,
        look: Rc<emLook>,
        selection_mode: SelectionMode,
        enabled: bool,
    ) -> Self {
        Self {
            name,
            is_directory,
            is_readable,
            is_selected,
            look,
            selection_mode,
            enabled,
        }
    }
}

impl PanelBehavior for FileItemPanelBehavior {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        // C++ emFileSelectionBox::FileItemPanel::Paint (lines 958-1062).
        // Panel coordinates: (0,0)-(w,h) where w is normalized to 1.0 in C++.
        // Here w and h are the actual panel dimensions.

        let panel_h = h / w.max(1e-100); // normalized height (C++ GetHeight())
        let nh = panel_h.max(1e-3);

        // emColor setup matching C++ GetFgColor/GetBgColor via emLook.
        let (bg, fg, hl) = if self.selection_mode == SelectionMode::ReadOnly {
            (
                self.look.output_bg_color,
                self.look.output_fg_color,
                self.look.output_hl_color,
            )
        } else {
            (
                self.look.input_bg_color,
                self.look.input_fg_color,
                self.look.input_hl_color,
            )
        };
        let (bg, fg, hl) = if !self.enabled {
            let base = self.look.bg_color;
            (
                bg.GetBlended(base, 80.0),
                fg.GetBlended(base, 80.0),
                hl.GetBlended(base, 80.0),
            )
        } else {
            (bg, fg, hl)
        };

        let fg_color = fg;
        let mut canvas_color = bg;

        // 1. Selection highlight (C++ lines 973-985).
        if self.is_selected {
            let s = 1.0_f64.min(nh);
            let fx = s * 0.015;
            let fw = 1.0 - 2.0 * fx;
            let fy = fx;
            let fh = nh - 2.0 * fy;
            let r = s * 0.1;
            painter.PaintRoundRect(
                fx * w,
                fy * w,
                fw * w,
                fh * w,
                r * w,
                r * w,
                hl,
                painter.GetCanvasColor(),
            );
            canvas_color = hl;
        }

        // 2. Filename text (C++ lines 987-999).
        {
            let fx = 0.06;
            let fw = 1.0 - 2.0 * fx;
            let fy = nh * 0.77;
            let fh = nh - fy - nh * 0.05;
            let text_color = if self.is_selected { bg } else { fg_color };
            painter.PaintTextBoxed(
                fx * w,
                fy * w,
                fw * w,
                fh * w,
                &self.name,
                nh * w,
                text_color,
                canvas_color,
                crate::emPainter::TextAlignment::Center,
                crate::emPainter::VAlign::Center,
                crate::emPainter::TextAlignment::Center,
                0.5,
                true,
                0.0,
            );
        }

        // 3. Directory icon area (C++ lines 1001-1061).
        if self.is_directory {
            // C++ selects Dir.tga or DirUp.tga based on item text.
            let is_parent = self.name == "..";

            with_toolkit_images(|imgs| {
                let img = if is_parent { &imgs.dir_up } else { &imgs.dir };
                let img_w = img.GetWidth();
                let img_h = img.GetHeight();
                let img_aspect = img_h as f64 / img_w as f64; // height/width

                let mut fx = 0.06;
                let mut fw = 1.0 - 2.0 * fx;
                let mut fy = nh * 0.1;
                let mut fh = nh * 0.62;

                // Aspect ratio preservation (C++ lines 1019-1026).
                if fh / fw < img_aspect {
                    fw = fh / img_aspect;
                    fx = (1.0 - fw) * 0.5;
                } else {
                    fy += (fh - fw * img_aspect) * 0.5;
                    fh = fw * img_aspect;
                }

                // C++ PaintImageColored(fx,fy,fw,fh, *img, 0, fgCol, canvasColor, EXTEND_ZERO)
                // color1=0 (transparent), color2=fgCol
                painter.PaintImageColored(
                    fx * w,
                    fy * w,
                    fw * w,
                    fh * w,
                    img,
                    0,
                    0,
                    img_w,
                    img_h,
                    emColor::TRANSPARENT,
                    fg_color,
                    canvas_color,
                    ImageExtension::Zero,
                );

                // 4. "Parent Directory" overlay for ".." (C++ lines 1031-1044).
                if is_parent {
                    let pd_color = fg_color.GetTransparented(40.0);
                    let pdx = (fx + fw * 115.0 / 310.0) * w;
                    let pdy = (fy + fh * 168.0 / 216.0) * w;
                    let pdw = (fw * 150.0 / 310.0) * w;
                    let pdh = (fh * 23.0 / 216.0) * w;
                    painter.PaintTextBoxed(
                        pdx,
                        pdy,
                        pdw,
                        pdh,
                        "Parent Directory",
                        fh * w,
                        pd_color,
                        emColor::TRANSPARENT,
                        crate::emPainter::TextAlignment::Center,
                        crate::emPainter::VAlign::Center,
                        crate::emPainter::TextAlignment::Center,
                        0.5,
                        true,
                        0.0,
                    );
                }

                // 5. Not-readable indicator (C++ lines 1045-1059).
                if !self.is_readable {
                    let r = fw.min(fh) * 0.35;
                    let cx = (fx + fw * 0.5) * w;
                    let cy = (fy + fh * 0.5) * w;
                    let rw = r * w;

                    // Circle outline via ellipse outlined.
                    let stroke = emStroke::new(fg_color, rw * 0.26);
                    painter.PaintEllipseOutline(cx, cy, rw, rw, &stroke, canvas_color);

                    // Diagonal line.
                    let t = rw * std::f64::consts::FRAC_1_SQRT_2;
                    let line_stroke = emStroke::new(fg_color, rw * 0.22);
                    painter.paint_line_stroked(
                        cx - t,
                        cy - t,
                        cx + t,
                        cy + t,
                        &line_stroke,
                        canvas_color,
                    );
                }
            });
        }
    }

    fn GetCanvasColor(&self) -> emColor {
        if self.selection_mode == SelectionMode::ReadOnly {
            self.look.output_bg_color
        } else {
            self.look.input_bg_color
        }
    }

    fn auto_expand(&self) -> bool {
        // C++ FileItemPanel::AutoExpand: only non-directory, enabled files expand.
        self.enabled && !self.is_directory
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() && self.enabled && !self.is_directory {
            // C++ creates FilePanel("content") + FileOverlayPanel("overlay").
            // DIVERGED: C++ uses emFpPluginList::CreateFilePanel for content;
            // we create a stub that paints an opaque white background
            // (matching C++ file panels' default appearance).
            let content_id = ctx.create_child("content");
            ctx.tree.set_behavior(content_id, Box::new(FilePanelStub));
            let _overlay_id = ctx.create_child("overlay");
        }
        // C++ FileItemPanel::LayoutChildren (emFileSelectionBox.cpp:1095-1112):
        // Content panel is inset within the icon area; overlay fills the full panel.
        let lr = ctx.layout_rect();
        let h = lr.h.max(1e-3);
        let children = ctx.children();
        let bg = ctx.GetCanvasColor();
        for &child in &children {
            let is_overlay = ctx.tree.name(child) == Some("overlay");
            if is_overlay {
                ctx.layout_child(child, 0.0, 0.0, 1.0, h);
            } else {
                // "content": C++ insets fx=0.06, fw=0.88, fy=h*0.1, fh=h*0.62,
                // then clamps fw to fh*16/9 and re-centers.
                let mut fx: f64 = 0.06;
                let mut fw: f64 = 1.0 - 2.0 * fx;
                let fy: f64 = h * 0.1;
                let fh: f64 = h * 0.62;
                fw = fw.min(fh * 16.0 / 9.0);
                fx = (1.0 - fw) * 0.5;
                ctx.layout_child_canvas(child, fx, fy, fw, fh, bg);
            }
        }
    }
}

/// Stub file content panel: paints an opaque white background matching
/// the default appearance of C++ file viewer panels.
struct FilePanelStub;

impl PanelBehavior for FilePanelStub {
    fn IsOpaque(&self) -> bool {
        true
    }
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _s: &PanelState) {
        p.PaintRect(0.0, 0.0, w, h, emColor::WHITE, p.GetCanvasColor());
    }
}

type SelectionChangedCb = Box<dyn FnMut()>;
type FileTriggerCb = Box<dyn FnMut(&str)>;

/// Shared event state collected by child-panel callbacks and drained in `cycle()`.
#[derive(Default)]
struct FsbEvents {
    selection_changed: bool,
    selection_indices: Vec<usize>,
    triggered_index: Option<usize>,
    name_text_changed: Option<String>,
    dir_text_changed: Option<String>,
    hidden_toggled: Option<bool>,
    filter_index_changed: Option<usize>,
}

/// A file selection box widget for browsing and selecting files.
///
/// Port of C++ `emFileSelectionBox`. Provides a file browser with:
/// - A text field showing the current directory path
/// - A checkbox to toggle showing hidden files
/// - A list of files/directories in the current directory
/// - A text field for entering/editing the file name
/// - A filter list for file type filtering
pub struct emFileSelectionBox {
    border: emBorder,
    look: Rc<emLook>,
    multi_selection_enabled: bool,
    parent_dir: PathBuf,
    selected_names: Vec<String>,
    filters: Vec<String>,
    selected_filter_index: i32,
    hidden_files_shown: bool,
    triggered_file_name: String,
    parent_dir_field_hidden: bool,
    hidden_check_box_hidden: bool,
    name_field_hidden: bool,
    filter_hidden: bool,
    listing_invalid: bool,
    listing: Vec<(String, FileItemData)>,
    // Child panel IDs (populated on auto-expand)
    dir_field_id: Option<PanelId>,
    hidden_cb_id: Option<PanelId>,
    files_lb_id: Option<PanelId>,
    name_field_id: Option<PanelId>,
    filter_lb_id: Option<PanelId>,
    /// True when children must be torn down and rebuilt on next layout pass.
    children_dirty: bool,
    /// Shared event state collected by child-panel callbacks.
    events: Rc<RefCell<FsbEvents>>,
    /// Shared listing metadata for the item behavior factory closure.
    listing_data: Rc<RefCell<Vec<FileItemData>>>,
    /// Consumer callback: selection changed.
    pub on_selection: Option<SelectionChangedCb>,
    /// Consumer callback: file triggered (double-click / Enter on a file).
    pub on_trigger: Option<FileTriggerCb>,
}

impl emFileSelectionBox {
    pub fn new(caption: &str) -> Self {
        let parent_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        Self {
            border: emBorder::new(OuterBorderType::Group)
                .with_inner(InnerBorderType::Group)
                .with_caption(caption),
            look: emLook::new(),
            multi_selection_enabled: false,
            parent_dir,
            selected_names: Vec::new(),
            filters: Vec::new(),
            selected_filter_index: -1,
            hidden_files_shown: false,
            triggered_file_name: String::new(),
            parent_dir_field_hidden: false,
            hidden_check_box_hidden: false,
            name_field_hidden: false,
            filter_hidden: false,
            listing_invalid: true,
            listing: Vec::new(),
            dir_field_id: None,
            hidden_cb_id: None,
            files_lb_id: None,
            name_field_id: None,
            filter_lb_id: None,
            children_dirty: false,
            events: Rc::new(RefCell::new(FsbEvents::default())),
            listing_data: Rc::new(RefCell::new(Vec::new())),
            on_selection: None,
            on_trigger: None,
        }
    }

    pub fn is_multi_selection_enabled(&self) -> bool {
        self.multi_selection_enabled
    }

    pub fn set_multi_selection_enabled(&mut self, enabled: bool) {
        if self.multi_selection_enabled != enabled {
            if !enabled && self.selected_names.len() > 1 {
                let first = self.selected_names[0].clone();
                self.set_selected_name(&first);
            }
            self.multi_selection_enabled = enabled;
            self.children_dirty = true;
        }
    }

    pub fn GetParentDirectory(&self) -> &Path {
        &self.parent_dir
    }

    pub fn set_parent_directory(&mut self, parent_directory: &Path) {
        let abs_path = if parent_directory.is_absolute() {
            parent_directory.to_path_buf()
        } else {
            std::fs::canonicalize(parent_directory)
                .unwrap_or_else(|_| parent_directory.to_path_buf())
        };

        if self.parent_dir != abs_path {
            self.parent_dir = abs_path;
            self.triggered_file_name.clear();
            self.invalidate_listing();
        }
    }

    pub fn GetSelectedName(&self) -> Option<&str> {
        self.selected_names.first().map(|s| s.as_str())
    }

    pub fn GetSelectedNames(&self) -> &[String] {
        &self.selected_names
    }

    pub fn set_selected_name(&mut self, name: &str) {
        if name.is_empty() {
            if !self.selected_names.is_empty() {
                self.selected_names.clear();
            }
        } else if self.selected_names.len() != 1 || self.selected_names[0] != name {
            self.selected_names = vec![name.to_string()];
        }
    }

    pub fn set_selected_names(&mut self, names: &[String]) {
        let mut sorted = names.to_vec();
        sorted.sort();

        if sorted != self.selected_names {
            self.selected_names = sorted;
        }
    }

    pub fn ClearSelection(&mut self) {
        self.set_selected_name("");
    }

    pub fn GetSelectedPath(&self) -> PathBuf {
        if let Some(name) = self.selected_names.first() {
            self.parent_dir.join(name)
        } else {
            self.parent_dir.clone()
        }
    }

    pub fn set_selected_path(&mut self, selected_path: &Path) {
        let abs_path = if selected_path.is_absolute() {
            selected_path.to_path_buf()
        } else {
            std::fs::canonicalize(selected_path).unwrap_or_else(|_| selected_path.to_path_buf())
        };

        if abs_path.is_dir() {
            self.set_parent_directory(&abs_path);
            self.ClearSelection();
        } else {
            if let Some(parent) = abs_path.parent() {
                self.set_parent_directory(parent);
            }
            if let Some(name) = abs_path.file_name() {
                self.set_selected_name(&name.to_string_lossy());
            }
        }
    }

    pub fn GetFilters(&self) -> &[String] {
        &self.filters
    }

    pub fn set_filters(&mut self, filters: &[String]) {
        if self.filters == filters {
            return;
        }

        self.filters = filters.to_vec();
        let count = self.filters.len() as i32;
        if self.selected_filter_index >= count {
            self.selected_filter_index = count - 1;
        } else if self.selected_filter_index < 0 && count > 0 {
            self.selected_filter_index = 0;
        }
        self.children_dirty = true;
        self.invalidate_listing();
    }

    pub fn GetSelectedFilterIndex(&self) -> i32 {
        self.selected_filter_index
    }

    pub fn set_selected_filter_index(&mut self, index: i32) {
        let clamped = if index < 0 || index >= self.filters.len() as i32 {
            -1
        } else {
            index
        };
        if self.selected_filter_index != clamped {
            self.selected_filter_index = clamped;
            self.invalidate_listing();
        }
    }

    pub fn are_hidden_files_shown(&self) -> bool {
        self.hidden_files_shown
    }

    pub fn set_hidden_files_shown(&mut self, shown: bool) {
        if self.hidden_files_shown != shown {
            self.hidden_files_shown = shown;
            self.invalidate_listing();
        }
    }

    pub fn GetTriggeredFileName(&self) -> &str {
        &self.triggered_file_name
    }

    pub fn trigger_file(&mut self, name: &str) {
        dlog!("FileSelectionBox trigger_file: {}", name);
        self.triggered_file_name = name.to_string();
    }

    /// Enter a sub-directory by name.
    pub fn enter_sub_dir(&mut self, name: &str) {
        dlog!("FileSelectionBox enter_sub_dir: {}", name);
        let path = self.parent_dir.join(name);
        if name == ".." {
            self.set_parent_directory(&path);
            self.ClearSelection();
        } else if path.is_dir() {
            // Check readability by attempting to read the directory.
            if std::fs::read_dir(&path).is_ok() {
                self.set_parent_directory(&path);
                self.ClearSelection();
            }
        }
    }

    pub fn is_parent_dir_field_hidden(&self) -> bool {
        self.parent_dir_field_hidden
    }

    pub fn set_parent_dir_field_hidden(&mut self, hidden: bool) {
        self.parent_dir_field_hidden = hidden;
    }

    pub fn is_hidden_check_box_hidden(&self) -> bool {
        self.hidden_check_box_hidden
    }

    pub fn set_hidden_check_box_hidden(&mut self, hidden: bool) {
        self.hidden_check_box_hidden = hidden;
    }

    pub fn is_name_field_hidden(&self) -> bool {
        self.name_field_hidden
    }

    pub fn set_name_field_hidden(&mut self, hidden: bool) {
        self.name_field_hidden = hidden;
    }

    pub fn is_filter_hidden(&self) -> bool {
        self.filter_hidden
    }

    pub fn set_filter_hidden(&mut self, hidden: bool) {
        self.filter_hidden = hidden;
    }

    /// Reload the directory listing, applying filters and hidden-file settings.
    pub fn reload_listing(&mut self) {
        let mut entries = Vec::new();

        // C++ catches the exception from emTryLoadDir, clears names, then falls
        // through to the sort and ".." insertion.  Match that: on error, skip the
        // directory iteration but continue to the sort + ".." logic below.
        if let Ok(dir_entries) = std::fs::read_dir(&self.parent_dir) {
            for entry in dir_entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                let is_directory = path.is_dir();
                let is_readable =
                    std::fs::read_dir(&path).is_ok() || std::fs::File::open(&path).is_ok();
                let is_hidden = name.starts_with('.');

                let data = FileItemData {
                    is_directory,
                    is_readable,
                    is_hidden,
                };

                // Filter hidden files.
                if !self.hidden_files_shown && is_hidden {
                    continue;
                }

                // Apply file type filter (directories pass through).
                if self.selected_filter_index >= 0
                    && (self.selected_filter_index as usize) < self.filters.len()
                    && !is_directory
                    && !match_file_name_filter(
                        &name,
                        &self.filters[self.selected_filter_index as usize],
                    )
                {
                    continue;
                }

                entries.push((name, data));
            }
        }

        // Sort by name only (locale-aware strcoll), matching C++ CompareNames.
        // C++ does NOT sort directories first — pure alphabetical.
        entries.sort_by(|(a_name, _), (b_name, _)| strcoll_compare(a_name, b_name));

        // Add ".." entry at the beginning if not at root.
        if self.parent_dir != Path::new("/") {
            entries.insert(
                0,
                (
                    "..".to_string(),
                    FileItemData {
                        is_directory: true,
                        is_readable: true,
                        is_hidden: false,
                    },
                ),
            );
        }

        // Update shared listing data for the item behavior factory.
        *self.listing_data.borrow_mut() = entries.iter().map(|(_, d)| d.clone()).collect();

        self.listing = entries;
        self.listing_invalid = false;
    }

    /// Get the current directory listing.
    pub fn GetListing(&self) -> &[(String, FileItemData)] {
        &self.listing
    }

    /// Whether the listing needs to be reloaded.
    pub fn is_listing_invalid(&self) -> bool {
        self.listing_invalid
    }

    fn invalidate_listing(&mut self) {
        self.listing_invalid = true;
    }

    pub fn border(&self) -> &emBorder {
        &self.border
    }

    pub fn border_mut(&mut self) -> &mut emBorder {
        &mut self.border
    }

    /// Create child panels matching C++ AutoExpand().
    fn create_children(&mut self, ctx: &mut PanelCtx) {
        // Pre-calculate border scaling for FilesLB (C++ sets this dynamically,
        // but we set it at creation time to avoid downcasting).
        let rect = ctx.layout_rect();
        let cr = self
            .border
            .GetContentRectUnobscured(rect.w, rect.h, &self.look);
        let hs = (cr.w * 0.05).min(cr.h * 0.15);
        let has_top = !self.parent_dir_field_hidden || !self.hidden_check_box_hidden;
        let has_bottom = !self.name_field_hidden || !self.filter_hidden;
        let h1 = if has_top { hs } else { 0.0 };
        let h3 = if has_bottom { hs } else { 0.0 };
        let h2 = cr.h - h1 - h3;

        // 1. ParentDirField
        if !self.parent_dir_field_hidden {
            let mut tf = emTextField::new(self.look.clone());
            tf.SetCaption("Directory");
            tf.SetEditable(true);
            tf.SetText(&self.parent_dir.to_string_lossy());
            let events = self.events.clone();
            tf.on_text = Some(Box::new(move |text: &str| {
                let mut e = events.borrow_mut();
                e.dir_text_changed = Some(text.to_string());
            }));
            let id = ctx.create_child_with(
                "directory",
                Box::new(TextFieldPanel { text_field: tf }),
            );
            self.dir_field_id = Some(id);
        }

        // 2. HiddenCheckBox
        if !self.hidden_check_box_hidden {
            let mut cb = emCheckBox::new("Show\nHidden\nFiles", self.look.clone());
            cb.SetChecked(self.hidden_files_shown);
            let events = self.events.clone();
            cb.on_check = Some(Box::new(move |checked: bool| {
                let mut e = events.borrow_mut();
                e.hidden_toggled = Some(checked);
            }));
            let id =
                ctx.create_child_with("showHiddenFiles", Box::new(CheckBoxPanel { check_box: cb }));
            self.hidden_cb_id = Some(id);
        }

        // 3. FilesLB (always created)
        // Matches C++ FilesListBox constructor: SetMinCellCount(4), SetChildTallness(0.6),
        // SetAlignment(EM_ALIGN_TOP_LEFT).
        {
            let mut lb = emListBox::new(self.look.clone());
            lb.SetCaption("Files");
            lb.SetMinCellCount(4);
            lb.SetChildTallness(0.6);
            lb.SetAlignment(
                crate::emTiling::AlignmentH::Left,
                crate::emTiling::AlignmentV::Top,
            );
            lb.SetSelectionType(if self.multi_selection_enabled {
                SelectionMode::Multi
            } else {
                SelectionMode::Single
            });
            if h2 > 1e-100 {
                lb.border_mut().SetBorderScaling(hs / h2);
            }
            let events = self.events.clone();
            lb.on_selection = Some(Box::new(move |indices: &[usize]| {
                let mut e = events.borrow_mut();
                e.selection_changed = true;
                e.selection_indices = indices.to_vec();
            }));
            let events = self.events.clone();
            lb.on_trigger = Some(Box::new(move |index: usize| {
                let mut e = events.borrow_mut();
                e.triggered_index = Some(index);
            }));
            // Set custom item behavior factory for FileItemPanel rendering.
            let listing_data = self.listing_data.clone();
            lb.set_item_behavior_factory(
                move |index, text, selected, look, sel_mode, enabled| {
                    let data = listing_data.borrow();
                    let (is_dir, is_readable) = data
                        .get(index)
                        .map(|d| (d.is_directory, d.is_readable))
                        .unwrap_or((false, true));
                    Box::new(FileItemPanelBehavior::new(
                        text.to_string(),
                        is_dir,
                        is_readable,
                        selected,
                        look,
                        sel_mode,
                        enabled,
                    ))
                },
            );
            let id = ctx.create_child_with("files", Box::new(ListBoxPanel { list_box: lb }));
            self.files_lb_id = Some(id);
        }

        // 4. NameField
        if !self.name_field_hidden {
            let mut tf = emTextField::new(self.look.clone());
            tf.SetCaption("Name");
            tf.SetEditable(true);
            if let Some(name) = self.selected_names.first() {
                tf.SetText(name);
            }
            let events = self.events.clone();
            tf.on_text = Some(Box::new(move |text: &str| {
                let mut e = events.borrow_mut();
                e.name_text_changed = Some(text.to_string());
            }));
            let id = ctx.create_child_with(
                "name",
                Box::new(TextFieldPanel { text_field: tf }),
            );
            self.name_field_id = Some(id);
        }

        // 5. FiltersLB
        if !self.filter_hidden {
            let mut lb = emListBox::new(self.look.clone());
            lb.SetCaption("Filter");
            for (i, filter) in self.filters.iter().enumerate() {
                lb.AddItem(format!("{}", i), filter.clone());
            }
            if self.selected_filter_index >= 0 {
                lb.SetSelectedIndex(self.selected_filter_index as usize);
            }
            let events = self.events.clone();
            lb.on_selection = Some(Box::new(move |indices: &[usize]| {
                let mut e = events.borrow_mut();
                e.filter_index_changed = indices.first().copied();
            }));
            let id = ctx.create_child_with("filter", Box::new(ListBoxPanel { list_box: lb }));
            self.filter_lb_id = Some(id);
        }

        // Eagerly reload the listing now so that item panels can be created in the
        // first LayoutChildren pass (matching C++ where the scheduler calls Cycle()
        // before the first paint, ensuring ReloadListing runs before items are shown).
        if self.listing_invalid {
            self.reload_listing();
            self.selection_to_list_box(ctx);
        }

        // Register for per-frame cycling so we can process events.
        ctx.tree.Cycle(ctx.id);
    }

    /// Sync FSB internal selection state FROM emListBox selection indices.
    fn selection_from_list_box(&mut self, indices: &[usize]) {
        let names: Vec<String> = indices
            .iter()
            .filter_map(|&i| self.listing.get(i).map(|(name, _)| name.clone()))
            .collect();
        self.selected_names = names;
    }

    /// Sync FSB selection state TO the emListBox widget after a listing reload.
    fn selection_to_list_box(&self, ctx: &mut PanelCtx) {
        if let Some(lb_id) = self.files_lb_id {
            // Build item data outside the tree borrow.
            // C++ uses file name as both item name and text.
            let items: Vec<(String, String)> = self
                .listing
                .iter()
                .map(|(name, _)| (name.clone(), name.clone()))
                .collect();
            let selected_indices: Vec<usize> = self
                .listing
                .iter()
                .enumerate()
                .filter(|(_, (name, _))| self.selected_names.contains(name))
                .map(|(i, _)| i)
                .collect();

            ctx.tree.with_behavior_as::<ListBoxPanel, _>(lb_id, |lbp| {
                lbp.list_box.ClearItems();
                for (key, text) in &items {
                    lbp.list_box.AddItem(key.clone(), text.clone());
                }
                lbp.list_box.SetSelectedIndices(&selected_indices);
            });
            // Items changed — notify the listbox panel to re-run LayoutChildren
            // so it creates/updates item panels. C++ does this implicitly via
            // CreateItemPanel() in InsertItem() when IsAutoExpanded().
            ctx.tree.queue_notice(lb_id, super::emPanel::NoticeFlags::LAYOUT_CHANGED);
        }
    }

    /// Update name field text to match current selection.
    fn sync_name_field(&self, ctx: &mut PanelCtx) {
        if let Some(nf_id) = self.name_field_id {
            let text = self
                .selected_names
                .first()
                .cloned()
                .unwrap_or_default();
            ctx.tree
                .with_behavior_as::<TextFieldPanel, _>(nf_id, |tfp| {
                    tfp.text_field.SetText(&text);
                });
        }
    }

    /// Update directory field text to match current parent_dir.
    fn sync_dir_field(&self, ctx: &mut PanelCtx) {
        if let Some(df_id) = self.dir_field_id {
            let text = self.parent_dir.to_string_lossy().into_owned();
            ctx.tree
                .with_behavior_as::<TextFieldPanel, _>(df_id, |tfp| {
                    tfp.text_field.SetText(&text);
                });
        }
    }
}

/// Match a filename against a filter string of the form `Description (*.ext1 *.ext2)`.
///
/// Port of C++ `emFileSelectionBox::MatchFileNameFilter`.
pub(crate) fn match_file_name_filter(file_name: &str, filter: &str) -> bool {
    // Find the patterns between the last '(' and last ')'.
    let pattern_range = match (filter.rfind('('), filter.rfind(')')) {
        (Some(start), Some(end)) if start < end => &filter[start + 1..end],
        _ => filter,
    };

    // Split patterns by whitespace, semicolons, commas, or pipes.
    for pattern in
        pattern_range.split(|c: char| c.is_whitespace() || c == ';' || c == ',' || c == '|')
    {
        let pattern = pattern.trim();
        if !pattern.is_empty() && match_file_name_pattern(file_name, pattern) {
            return true;
        }
    }
    false
}

/// Match a filename against a glob-like pattern with `*` wildcards.
/// Matching is case-insensitive.
///
/// Port of C++ `emFileSelectionBox::MatchFileNamePattern`.
fn match_file_name_pattern(file_name: &str, pattern: &str) -> bool {
    let fname_bytes = file_name.as_bytes();
    let pat_bytes = pattern.as_bytes();
    match_pattern_recursive(fname_bytes, pat_bytes)
}

fn match_pattern_recursive(fname: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return fname.is_empty();
    }
    if pattern[0] == b'*' {
        // Try matching the rest of the pattern at each position.
        for i in 0..=fname.len() {
            if match_pattern_recursive(&fname[i..], &pattern[1..]) {
                return true;
            }
        }
        return false;
    }
    if fname.is_empty() {
        return pattern.is_empty();
    }
    if !fname[0].eq_ignore_ascii_case(&pattern[0]) {
        return false;
    }
    match_pattern_recursive(&fname[1..], &pattern[1..])
}

impl PanelBehavior for emFileSelectionBox {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.border
            .paint_border(painter, w, h, &self.look, state.enabled, true, state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100));
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.IsAutoExpanded(ctx.id) {
            return;
        }

        // If state that affects child structure changed after initial creation,
        // tear down and rebuild all children.
        if self.children_dirty && ctx.child_count() > 0 {
            ctx.DeleteAllChildren();
            self.dir_field_id = None;
            self.hidden_cb_id = None;
            self.files_lb_id = None;
            self.name_field_id = None;
            self.filter_lb_id = None;
        }
        self.children_dirty = false;

        if ctx.child_count() == 0 {
            self.create_children(ctx);
        }

        let rect = ctx.layout_rect();
        let (w, h) = (rect.w, rect.h);

        let cr = self.border.GetContentRectUnobscured(w, h, &self.look);
        let (x, y, cw, ch) = (cr.x, cr.y, cr.w, cr.h);

        let cc = self
            .border
            .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());

        // 3-zone geometry matching C++ LayoutChildren
        let hs = (cw * 0.05).min(ch * 0.15);
        let has_top = self.dir_field_id.is_some() || self.hidden_cb_id.is_some();
        let has_bottom = self.name_field_id.is_some() || self.filter_lb_id.is_some();
        let h1 = if has_top { hs } else { 0.0 };
        let h3 = if has_bottom { hs } else { 0.0 };
        let h2 = ch - h1 - h3;

        // Top row: directory field + checkbox
        if let Some(cb_id) = self.hidden_cb_id {
            let w2 = (cw * 0.5).min(h1 * 2.0);
            let w1 = cw - w2;
            if let Some(df_id) = self.dir_field_id {
                ctx.layout_child_canvas(df_id, x, y, w1, h1, cc);
            }
            ctx.layout_child_canvas(cb_id, x + w1, y, w2, h1, cc);
        } else if let Some(df_id) = self.dir_field_id {
            ctx.layout_child_canvas(df_id, x, y, cw, h1, cc);
        }

        // Middle: files list
        // C++ LayoutChildren calls SetBorderScaling(hs/h2) on FilesLB AFTER layout.
        // Rust must do the same so the content rect matches C++ at every layout call.
        if let Some(fl_id) = self.files_lb_id {
            ctx.layout_child_canvas(fl_id, x, y + h1, cw, h2, cc);
            if h2 > 1e-100 {
                let new_bs = hs / h2;
                let changed = ctx
                    .tree
                    .with_behavior_as::<ListBoxPanel, _>(fl_id, |lbp| {
                        let old = lbp.list_box.border_mut().border_scaling;
                        lbp.list_box.border_mut().SetBorderScaling(new_bs);
                        (old - new_bs).abs() > 1e-12
                    })
                    .unwrap_or(false);
                // Border scaling changes the content rect, so the list box
                // must re-layout its children (C++ triggers InvalidateLayout).
                if changed {
                    ctx.tree.queue_notice(fl_id, NoticeFlags::LAYOUT_CHANGED);
                }
            }
        }

        // Bottom row: name field + filter list
        if let Some(flb_id) = self.filter_lb_id {
            let w2 = (cw * 0.5).min(h3 * 10.0);
            let w1 = cw - w2;
            if let Some(nf_id) = self.name_field_id {
                ctx.layout_child_canvas(nf_id, x, y + h1 + h2, w1, h3, cc);
            }
            ctx.layout_child_canvas(flb_id, x + w1, y + h1 + h2, w2, h3, cc);
        } else if let Some(nf_id) = self.name_field_id {
            ctx.layout_child_canvas(nf_id, x, y + h1 + h2, cw, h3, cc);
        }
    }

    fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool {
        // Take all pending events.
        let events = {
            let mut e = self.events.borrow_mut();
            std::mem::take(&mut *e)
        };

        // Step 2 (C++): Directory field changed.
        if let Some(dir_text) = events.dir_text_changed {
            let new_dir = PathBuf::from(&dir_text);
            if self.parent_dir != new_dir {
                self.parent_dir = new_dir;
                self.triggered_file_name.clear();
                self.invalidate_listing();
                if let Some(ref mut cb) = self.on_selection {
                    cb();
                }
            }
        }

        // Step 4 (C++): Hidden files checkbox toggled.
        if let Some(shown) = events.hidden_toggled {
            self.set_hidden_files_shown(shown);
        }

        // Step 5 (C++): Reload listing if invalid.
        if self.listing_invalid && self.files_lb_id.is_some() {
            self.reload_listing();
            // Sync selection TO emListBox after reload.
            self.selection_to_list_box(ctx);
        }

        // Step 6 (C++): emListBox selection changed -> update FSB state.
        if events.selection_changed && !self.listing_invalid {
            self.selection_from_list_box(&events.selection_indices);
            // Update name field.
            self.sync_name_field(ctx);
            if let Some(ref mut cb) = self.on_selection {
                cb();
            }
        }

        // Step 7 (C++): emListBox trigger (double-click).
        if let Some(index) = events.triggered_index {
            if !self.listing_invalid {
                // First sync selection.
                self.selection_from_list_box(&events.selection_indices);

                if let Some((name, data)) = self.listing.get(index) {
                    let name = name.clone();
                    let is_dir = data.is_directory;
                    if name == ".." || is_dir {
                        self.enter_sub_dir(&name);
                        // After entering subdir, update dir field.
                        self.sync_dir_field(ctx);
                    } else {
                        self.triggered_file_name = name.clone();
                        if let Some(ref mut cb) = self.on_trigger {
                            cb(&name);
                        }
                    }
                }
            }
        }

        // Step 8 (C++): Name field text changed.
        if let Some(name_text) = events.name_text_changed {
            if name_text.is_empty() {
                if self.selected_names.len() == 1 {
                    self.set_selected_name("");
                }
            } else if name_text.contains('/') || name_text.contains('\\') {
                // User typed a path -- resolve it.
                let abs = if Path::new(&name_text).is_absolute() {
                    PathBuf::from(&name_text)
                } else {
                    self.parent_dir.join(&name_text)
                };
                self.set_selected_path(&abs);
                // Sync name field back to just the filename.
                self.sync_name_field(ctx);
                self.sync_dir_field(ctx);
                if let Some(ref mut cb) = self.on_selection {
                    cb();
                }
            } else {
                self.set_selected_name(&name_text);
            }
        }

        // Step 9 (C++): Filter selection changed.
        if let Some(filter_idx) = events.filter_index_changed {
            self.set_selected_filter_index(filter_idx as i32);
        }

        // Stay awake as long as we have children (panel is expanded).
        self.files_lb_id.is_some()
    }
}

fn strcoll_compare(a: &str, b: &str) -> std::cmp::Ordering {
    use std::ffi::CString;
    let a_c = CString::new(a).unwrap_or_default();
    let b_c = CString::new(b).unwrap_or_default();
    let result = unsafe { libc::strcoll(a_c.as_ptr(), b_c.as_ptr()) };
    result.cmp(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matching_all_files() {
        assert!(match_file_name_filter("anything.txt", "All files (*)"));
        assert!(match_file_name_filter("", "All files (*)"));
    }

    #[test]
    fn filter_matching_extension() {
        assert!(match_file_name_filter("image.tga", "Targa files (*.tga)"));
        assert!(!match_file_name_filter("image.png", "Targa files (*.tga)"));
    }

    #[test]
    fn filter_matching_case_insensitive() {
        assert!(match_file_name_filter("FILE.TGA", "Targa files (*.tga)"));
        assert!(match_file_name_filter("file.Tga", "Targa files (*.tga)"));
    }

    #[test]
    fn filter_matching_multiple_patterns() {
        assert!(match_file_name_filter(
            "page.htm",
            "HTML files (*.htm *.html)"
        ));
        assert!(match_file_name_filter(
            "page.html",
            "HTML files (*.htm *.html)"
        ));
        assert!(!match_file_name_filter(
            "page.txt",
            "HTML files (*.htm *.html)"
        ));
    }

    #[test]
    fn new_file_selection_box() {
        let fsb = emFileSelectionBox::new("Files");
        assert!(!fsb.is_multi_selection_enabled());
        assert!(fsb.GetSelectedNames().is_empty());
        assert_eq!(fsb.GetSelectedFilterIndex(), -1);
        assert!(!fsb.are_hidden_files_shown());
    }

    #[test]
    fn set_selected_name() {
        let mut fsb = emFileSelectionBox::new("Files");
        fsb.set_selected_name("test.txt");
        assert_eq!(fsb.GetSelectedName(), Some("test.txt"));

        fsb.set_selected_name("");
        assert_eq!(fsb.GetSelectedName(), None);
    }

    #[test]
    fn set_filters() {
        let mut fsb = emFileSelectionBox::new("Files");
        fsb.set_filters(&[
            "All files (*)".to_string(),
            "Images (*.png *.jpg)".to_string(),
        ]);
        assert_eq!(fsb.GetFilters().len(), 2);
        assert_eq!(fsb.GetSelectedFilterIndex(), 0);
    }

    #[test]
    fn enter_parent_dir() {
        let mut fsb = emFileSelectionBox::new("Files");
        fsb.set_parent_directory(Path::new("/tmp"));
        fsb.set_selected_name("foo");
        fsb.enter_sub_dir("..");
        assert!(fsb.GetSelectedNames().is_empty());
    }

    #[test]
    fn visibility_toggles() {
        let mut fsb = emFileSelectionBox::new("Files");
        assert!(!fsb.is_parent_dir_field_hidden());
        fsb.set_parent_dir_field_hidden(true);
        assert!(fsb.is_parent_dir_field_hidden());

        assert!(!fsb.is_name_field_hidden());
        fsb.set_name_field_hidden(true);
        assert!(fsb.is_name_field_hidden());

        assert!(!fsb.is_filter_hidden());
        fsb.set_filter_hidden(true);
        assert!(fsb.is_filter_hidden());
    }
}
