use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emFpPlugin::{emFpPluginList, FileStatMode, PanelParentArg};
use emcore::emImage::emImage;
use emcore::emInstallInfo::{emGetConfigDirOverloadable, emGetInstallPath, InstallDirType};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::{AutoplayHandlingFlags, PanelId};
use emcore::emRec::{RecError, RecStruct, RecValue};
use emcore::emRecRecord::Record;
use emcore::emRecRecTypes::emColorRec;
use emcore::emResTga::load_tga;

// ── emVirtualCosmosItemRec ────────────────────────────────────────────────────

/// A single VirtualCosmos item record.
///
/// Port of C++ `emVirtualCosmosItemRec`.
#[derive(Debug, Clone, PartialEq)]
pub struct emVirtualCosmosItemRec {
    /// Item name (derived from filename without extension).
    pub Name: String,
    pub Title: String,
    pub PosX: f64,
    pub PosY: f64,
    pub Width: f64,
    pub ContentTallness: f64,
    pub BorderScaling: f64,
    pub BackgroundColor: emColor,
    pub BorderColor: emColor,
    pub TitleColor: emColor,
    pub Focusable: bool,
    pub FileName: String,
    pub CopyToUser: bool,
    pub Alternative: i32,
    /// Resolved path to the content file (set by TryPrepareItemFile).
    pub(crate) ItemFilePath: String,
}

impl emVirtualCosmosItemRec {
    /// Resolve and store the item file path.
    ///
    /// Port of C++ `emVirtualCosmosItemRec::TryPrepareItemFile`.
    ///
    /// If `CopyToUser` is false the path is `orig_dir/FileName`. If
    /// `CopyToUser` is true the path is `user_dir/FileName`, copying from
    /// `orig_dir` if the user copy does not yet exist. On error, falls back
    /// to the `orig_dir` path with a warning.
    pub fn TryPrepareItemFile(&mut self, orig_dir: &str, user_dir: &str) {
        let src_path = PathBuf::from(orig_dir).join(&self.FileName);

        if !self.CopyToUser {
            self.ItemFilePath = src_path.to_string_lossy().into_owned();
            return;
        }

        let tgt_path = PathBuf::from(user_dir).join(&self.FileName);
        self.ItemFilePath = tgt_path.to_string_lossy().into_owned();

        if !tgt_path.exists() {
            if let Err(e) = std::fs::create_dir_all(user_dir) {
                log::warn!(
                    "emVirtualCosmosItemRec: failed to create dir '{}': {}; \
                     falling back to orig path",
                    user_dir,
                    e,
                );
                self.ItemFilePath = src_path.to_string_lossy().into_owned();
                return;
            }
            if let Err(e) = std::fs::copy(&src_path, &tgt_path) {
                log::warn!(
                    "emVirtualCosmosItemRec: failed to copy '{}' to '{}': {}; \
                     falling back to orig path",
                    src_path.display(),
                    tgt_path.display(),
                    e,
                );
                self.ItemFilePath = src_path.to_string_lossy().into_owned();
            }
        }
    }
}

impl Default for emVirtualCosmosItemRec {
    fn default() -> Self {
        Self {
            Name: String::new(),
            Title: String::new(),
            PosX: 0.0,
            PosY: 0.0,
            Width: 0.1,
            ContentTallness: 1.0,
            BorderScaling: 1.0,
            BackgroundColor: emColor::from_packed(0xAAAAAAFF),
            BorderColor: emColor::from_packed(0xAAAAAAFF),
            TitleColor: emColor::from_packed(0x000000FF),
            Focusable: true,
            FileName: "unnamed".to_string(),
            CopyToUser: false,
            Alternative: 0,
            ItemFilePath: String::new(),
        }
    }
}

impl Record for emVirtualCosmosItemRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let bg = parse_color_field(rec, "backgroundcolor", emColor::from_packed(0xAAAAAAFF));
        let border_color =
            parse_color_field(rec, "bordercolor", emColor::from_packed(0xAAAAAAFF));
        let title_color =
            parse_color_field(rec, "titlecolor", emColor::from_packed(0x000000FF));

        Ok(Self {
            Name: rec.get_str("name").unwrap_or("").to_string(),
            Title: rec.get_str("title").unwrap_or("").to_string(),
            PosX: rec
                .get_double("posx")
                .unwrap_or(0.0)
                .clamp(0.0, 1.0),
            PosY: rec
                .get_double("posy")
                .unwrap_or(0.0)
                .clamp(0.0, 1.0),
            Width: rec
                .get_double("width")
                .unwrap_or(0.1)
                .clamp(1e-10, 1.0),
            ContentTallness: rec
                .get_double("contenttallness")
                .unwrap_or(1.0)
                .clamp(1e-10, 1e10),
            BorderScaling: rec
                .get_double("borderscaling")
                .unwrap_or(1.0)
                .clamp(0.0, 1e10),
            BackgroundColor: bg,
            BorderColor: border_color,
            TitleColor: title_color,
            Focusable: rec.get_bool("focusable").unwrap_or(true),
            FileName: rec
                .get_str("filename")
                .unwrap_or("unnamed")
                .to_string(),
            CopyToUser: rec.get_bool("copytouser").unwrap_or(false),
            Alternative: rec
                .get_int("alternative")
                .unwrap_or(0)
                .max(0),
            ItemFilePath: String::new(),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_str("name", &self.Name);
        s.set_str("title", &self.Title);
        s.set_double("posx", self.PosX);
        s.set_double("posy", self.PosY);
        s.set_double("width", self.Width);
        s.set_double("contenttallness", self.ContentTallness);
        s.set_double("borderscaling", self.BorderScaling);
        s.SetValue(
            "backgroundcolor",
            RecValue::Struct(emColorRec::ToRecStruct(self.BackgroundColor, true)),
        );
        s.SetValue(
            "bordercolor",
            RecValue::Struct(emColorRec::ToRecStruct(self.BorderColor, true)),
        );
        s.SetValue(
            "titlecolor",
            RecValue::Struct(emColorRec::ToRecStruct(self.TitleColor, true)),
        );
        s.set_bool("focusable", self.Focusable);
        s.set_str("filename", &self.FileName);
        s.set_bool("copytouser", self.CopyToUser);
        s.set_int("alternative", self.Alternative);
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

/// Parse a color field that may be either a hex string or `{R G B A}` struct.
///
/// C++ `.emVcItem` files use `"#BBB"` format (hex strings).
/// The Rust `to_rec` writes `{R G B A}` struct format.
/// Support both for round-trip compatibility.
fn parse_color_field(rec: &RecStruct, field: &str, default: emColor) -> emColor {
    // Try hex string first (C++ .emVcItem files use "#BBB" format)
    if let Some(s) = rec.get_str(field)
        && let Some(c) = emColor::TryParse(s)
    {
        return c;
    }
    // Fall back to struct format {R G B A}
    if let Some(s) = rec.get_struct(field)
        && let Ok(c) = emColorRec::FromRecStruct(s, true)
    {
        return c;
    }
    default
}

// ── emVirtualCosmosModel ──────────────────────────────────────────────────────

/// A loaded VirtualCosmos item (file name, modification time, parsed record).
pub struct LoadedItem {
    pub file_name: String,
    pub mtime: std::time::SystemTime,
    pub item_rec: emVirtualCosmosItemRec,
}

/// Model that loads `.emVcItem` files from the VcItems config directory.
///
/// Port of C++ `emVirtualCosmosModel`.
pub struct emVirtualCosmosModel {
    items_dir: String,
    item_files_dir: String,
    items: Vec<LoadedItem>,
    /// Indices into `items`, sorted by position (PosY then PosX).
    item_recs: Vec<usize>,
}

impl emVirtualCosmosModel {
    /// Acquire the singleton `emVirtualCosmosModel` from the context registry.
    ///
    /// Port of C++ `emVirtualCosmosModel::Acquire`.
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let mut model = Self {
                items_dir: String::new(),
                item_files_dir: String::new(),
                items: Vec::new(),
                item_recs: Vec::new(),
            };
            model.Reload();
            model
        })
    }

    /// Build a model directly from a list of pre-loaded items (for tests).
    pub fn from_items(items: Vec<LoadedItem>) -> Self {
        let mut model = Self {
            items_dir: String::new(),
            item_files_dir: String::new(),
            items,
            item_recs: Vec::new(),
        };
        model.sort_item_recs();
        model
    }

    /// Reload items from disk.
    ///
    /// Port of C++ `emVirtualCosmosModel::Reload`.
    pub fn Reload(&mut self) {
        let items_dir = emGetConfigDirOverloadable("emMain", Some("VcItems"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item_files_dir = emGetConfigDirOverloadable("emMain", Some("VcItemFiles"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item_files_user_dir =
            emGetInstallPath(InstallDirType::UserConfig, "emMain", Some("VcItemFiles.user"))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();

        self.items_dir = items_dir.clone();
        self.item_files_dir = item_files_dir.clone();

        let dir_entries = match std::fs::read_dir(&items_dir) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("emVirtualCosmosModel: cannot read dir '{}': {}", items_dir, e);
                self.items.clear();
                self.item_recs.clear();
                return;
            }
        };

        let mut new_items: Vec<LoadedItem> = Vec::new();

        for entry in dir_entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !file_name.to_lowercase().ends_with(".emvcitem") {
                continue;
            }

            let path = entry.path();
            let mtime = match std::fs::metadata(&path).and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!(
                        "emVirtualCosmosModel: cannot stat '{}': {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            // Derive item name: filename without extension.
            let name = std::path::Path::new(&file_name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();

            let rec = match emcore::emRecRecTypes::emRecFileReader::read(&path) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!(
                        "emVirtualCosmosModel: failed to read '{}': {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            let mut item_rec = match emVirtualCosmosItemRec::from_rec(&rec) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!(
                        "emVirtualCosmosModel: failed to parse '{}': {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            item_rec.Name = name;
            item_rec.TryPrepareItemFile(&item_files_dir, &item_files_user_dir);

            new_items.push(LoadedItem {
                file_name,
                mtime,
                item_rec,
            });
        }

        self.items = new_items;
        self.sort_item_recs();
    }

    /// Sort the `item_recs` index by PosY then PosX (matching C++ CompareItemRecs).
    fn sort_item_recs(&mut self) {
        let mut indices: Vec<usize> = (0..self.items.len()).collect();
        indices.sort_by(|&a, &b| {
            let ra = &self.items[a].item_rec;
            let rb = &self.items[b].item_rec;
            ra.PosY
                .partial_cmp(&rb.PosY)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    ra.PosX
                        .partial_cmp(&rb.PosX)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        self.item_recs = indices;
    }

    /// Return an iterator over item records in position order.
    ///
    /// Port of C++ `emVirtualCosmosModel::GetItemRec`.
    pub fn GetItemRecs(&self) -> impl Iterator<Item = &emVirtualCosmosItemRec> {
        self.item_recs.iter().map(|&i| &self.items[i].item_rec)
    }

    /// Return the number of loaded items.
    ///
    /// Port of C++ `emVirtualCosmosModel::GetItemCount`.
    pub fn GetItemCount(&self) -> usize {
        self.items.len()
    }

    /// Return the items directory path used during the last `Reload`.
    pub fn GetItemsDir(&self) -> &str {
        &self.items_dir
    }

    /// Return the item-files directory path used during the last `Reload`.
    pub fn GetItemFilesDir(&self) -> &str {
        &self.item_files_dir
    }
}

// ── emVirtualCosmosItemPanel ──────────────────────────────────────────────────

/// Panel for a single VirtualCosmos item: renders border, title, and content.
///
/// Port of C++ `emVirtualCosmosItemPanel` from `emMain/emVirtualCosmosPanel.cpp`.
pub struct emVirtualCosmosItemPanel {
    ctx: Rc<emContext>,
    item_rec: Option<emVirtualCosmosItemRec>,
    content_panel: Option<PanelId>,
    path: String,
    alt: i32,
    item_focusable: bool,
    update_needed: bool,
    OuterBorderImage: emImage,
    InnerBorderImage: emImage,
}

impl emVirtualCosmosItemPanel {
    /// Create a new item panel with no associated item record.
    ///
    /// Port of C++ `emVirtualCosmosItemPanel` constructor.
    pub fn new(ctx: Rc<emContext>) -> Self {
        let outer_border_image =
            load_tga(include_bytes!("../../../res/emMain/VcItemOuterBorder.tga"))
                .expect("failed to load VcItemOuterBorder.tga");
        let inner_border_image =
            load_tga(include_bytes!("../../../res/emMain/VcItemInnerBorder.tga"))
                .expect("failed to load VcItemInnerBorder.tga");
        Self {
            ctx,
            item_rec: None,
            content_panel: None,
            path: String::new(),
            alt: 0,
            item_focusable: true,
            update_needed: false,
            OuterBorderImage: outer_border_image,
            InnerBorderImage: inner_border_image,
        }
    }

    /// Update the item record, triggering a re-layout on the next cycle.
    ///
    /// Port of C++ `emVirtualCosmosItemPanel::SetItemRec`.
    pub fn SetItemRec(&mut self, rec: emVirtualCosmosItemRec) {
        let new_path = rec.ItemFilePath.clone();
        let new_alt = rec.Alternative;
        let new_focusable = rec.Focusable;
        // If path or alt changed the content panel must be recreated.
        if new_path != self.path || new_alt != self.alt {
            self.path = new_path;
            self.alt = new_alt;
            if self.content_panel.is_some() {
                self.update_needed = true; // will destroy+recreate in LayoutChildren
            }
        }
        self.item_focusable = new_focusable;
        self.item_rec = Some(rec);
        self.update_needed = true;
    }

    /// Compute the four border fractions: (left, top, right, bottom).
    ///
    /// Port of C++ `emVirtualCosmosItemPanel::CalcBorders`.
    ///
    /// `b = min(1.0, tallness) * border_scaling`
    /// then `l = b*0.03, t = b*0.05, r = b*0.03, bottom = b*0.03`.
    pub fn CalcBorders(&self) -> (f64, f64, f64, f64) {
        let (t, bs) = match &self.item_rec {
            Some(rec) => (rec.ContentTallness, rec.BorderScaling),
            None => (1.0, 1.0),
        };
        let b = t.min(1.0) * bs;
        (b * 0.03, b * 0.05, b * 0.03, b * 0.03)
    }
}

impl PanelBehavior for emVirtualCosmosItemPanel {
    // DIVERGED: C++ Paint(const emPainter&, emColor canvasColor)
    // Rust PanelBehavior::Paint doesn't receive canvasColor; use painter.GetCanvasColor().
    fn Paint(&mut self, painter: &mut emPainter, _w: f64, h: f64, _state: &PanelState) {
        let Some(rec) = &self.item_rec else {
            return;
        };

        if rec.BorderScaling <= 1e-100 {
            let canvas_color = painter.GetCanvasColor();
            painter.ClearWithCanvas(rec.BackgroundColor, canvas_color);
            return;
        }

        let bor_col = rec.BorderColor;
        let (l, t, r, b) = self.CalcBorders();
        let canvas_color = painter.GetCanvasColor();

        if bor_col == rec.BackgroundColor {
            painter.ClearWithCanvas(rec.BackgroundColor, canvas_color);
        } else {
            let mut x1 = l;
            let mut x2 = 1.0 - r;
            let mut y1 = t;
            let mut y2 = h - b;
            if bor_col.IsOpaque() {
                x1 = painter.RoundDownX(x1);
                y1 = painter.RoundDownY(y1);
                x2 = painter.RoundUpX(x2);
                y2 = painter.RoundUpY(y2);
            }
            painter.PaintRect(
                x1, y1, x2 - x1, y2 - y1,
                rec.BackgroundColor, canvas_color,
            );
            // Hollow border polygon: outer CW, inner CCW (10 vertices)
            let verts: [(f64, f64); 10] = [
                (0.0, 0.0),
                (1.0, 0.0),
                (1.0, h),
                (0.0, h),
                (0.0, 0.0),
                (l, t),
                (l, h - b),
                (1.0 - r, h - b),
                (1.0 - r, t),
                (l, t),
            ];
            painter.PaintPolygon(&verts, bor_col, emColor::TRANSPARENT);
        }

        // Outer border image
        let d = l * 0.4;
        painter.PaintBorderImage(
            0.0, 0.0, 1.0, h,
            d, d, d, d,
            &self.OuterBorderImage,
            82, 82, 82, 82,
            255, bor_col, 0o757,
        );

        // Inner border image
        let e = l * 0.5;
        let f = e * (23.0 / 126.0);
        painter.PaintBorderImage(
            l - e, t - f, 1.0 - l - r + e * 2.0, h - t - b + f * 2.0,
            e, f, e, f,
            &self.InnerBorderImage,
            126, 23, 126, 23,
            255, bor_col, 0o757,
        );

        // Title text
        let title = rec.Title.clone();
        let title_color = rec.TitleColor;
        painter.PaintTextBoxed(
            d,
            d + (t - d - f) * 0.07,
            1.0 - d * 2.0,
            (t - d - f) * 0.8,
            &title,
            h,
            title_color,
            bor_col,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.0,
        );
    }

    fn IsOpaque(&self) -> bool {
        let Some(rec) = &self.item_rec else {
            return false;
        };
        rec.BackgroundColor.IsOpaque()
            && (rec.BorderColor.IsOpaque() || rec.BorderScaling <= 1e-200)
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // Port of C++ emVirtualCosmosItemPanel::AutoExpand — creates
        // the content panel via the file plugin system.
        let Some(rec) = &self.item_rec else {
            return;
        };
        let (l, t, r, b) = self.CalcBorders();
        let content_w = 1.0 - l - r;
        let content_h = content_w * rec.ContentTallness;
        let total_h = content_h + t + b;
        if total_h < 1e-100 || content_w < 1e-100 {
            return;
        }
        if self.path.is_empty() || self.content_panel.is_some() {
            return;
        }
        let stat_mode = match std::fs::metadata(&self.path) {
            Ok(m) if m.is_dir() => FileStatMode::Directory,
            _ => FileStatMode::Regular,
        };
        let fppl = emFpPluginList::Acquire(&self.ctx);
        let fppl = fppl.borrow();
        let parent_arg = PanelParentArg::new(Rc::clone(&self.ctx));
        let behavior = fppl.CreateFilePanelWithStat(
            &parent_arg,
            "content",
            &self.path,
            None,
            stat_mode,
            self.alt as usize,
        );
        // C++ uses name "" for the content panel (matches identity path).
        let child_id = ctx.create_child_with("", behavior);
        // Register for cycling so the file panel drives its model loading.
        ctx.tree.Cycle(child_id);
        self.content_panel = Some(child_id);
    }

    fn AutoShrink(&mut self, _ctx: &mut PanelCtx) {
        // Default AutoShrink deletes children with created_by_ae=true.
        // Clear our reference since the panel will be deleted.
        self.content_panel = None;
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Position content panel within border (C++ LayoutContentPanel).
        let Some(rec) = &self.item_rec else {
            return;
        };
        let (l, t, r, b) = self.CalcBorders();
        let content_w = 1.0 - l - r;
        let content_h = content_w * rec.ContentTallness;
        let total_h = content_h + t + b;
        if total_h < 1e-100 || content_w < 1e-100 {
            return;
        }
        if let Some(child) = self.content_panel {
            let canvas = self
                .item_rec
                .as_ref()
                .map(|r| r.BackgroundColor)
                .unwrap_or(emColor::TRANSPARENT);
            ctx.layout_child_canvas(child, l, t, content_w, content_h, canvas);
        }
    }

    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        if flags.intersects(NoticeFlags::VIEW_CHANGED | NoticeFlags::LAYOUT_CHANGED) {
            self.update_needed = true;
        }
    }

    fn get_title(&self) -> Option<String> {
        self.item_rec.as_ref().map(|r| r.Title.clone())
    }

    fn IsHopeForSeeking(&self) -> bool {
        self.item_focusable
    }
}

// ── emVirtualCosmosPanel ──────────────────────────────────────────────────────

/// The container panel that renders the starfield background and all cosmos
/// item panels from the `emVirtualCosmosModel`.
///
/// Port of C++ `emVirtualCosmosPanel` from `emMain/emVirtualCosmosPanel.cpp`.
pub struct emVirtualCosmosPanel {
    ctx: Rc<emContext>,
    model: Rc<RefCell<emVirtualCosmosModel>>,
    background_panel: Option<PanelId>,
    /// (item name, panel_id) — one entry per item from the model.
    item_panels: Vec<(String, PanelId)>,
    needs_update: bool,
}

impl emVirtualCosmosPanel {
    /// Create a new cosmos panel, acquiring the model from the context.
    ///
    /// Port of C++ `emVirtualCosmosPanel` constructor.
    pub fn new(ctx: Rc<emContext>) -> Self {
        let model = emVirtualCosmosModel::Acquire(&ctx);
        Self {
            ctx,
            model,
            background_panel: None,
            item_panels: Vec::new(),
            needs_update: true,
        }
    }

    /// Port of C++ `emVirtualCosmosPanel::UpdateChildren`.
    /// Creates/updates/removes child panels to match the model.
    fn update_children(&mut self, ctx: &mut PanelCtx) {
        // ── 1. Starfield background ──────────────────────────────────────
        if self.background_panel.is_none() {
            let seed: u32 = 0x7f3a_19c0;
            let bg = crate::emStarFieldPanel::emStarFieldPanel::new(50, seed);
            let child_id = ctx.create_child_with("_StarField", Box::new(bg));
            ctx.tree.set_focusable(child_id, false);
            ctx.tree.SetAutoplayHandling(child_id, AutoplayHandlingFlags::CUTOFF);
            self.background_panel = Some(child_id);
        }

        // Layout background to cover the entire panel.
        if let Some(bg_id) = self.background_panel {
            let lr = ctx.layout_rect();
            let tallness = if lr.w > 1e-100 { lr.h / lr.w } else { 1.0 };
            ctx.layout_child(bg_id, 0.0, 0.0, 1.0, tallness);
        }

        // ── 2. Item panels ──────────────────────────────────────────────
        let desired: Vec<emVirtualCosmosItemRec> = {
            let m = self.model.borrow();
            m.GetItemRecs().cloned().collect()
        };

        let desired_names: std::collections::HashSet<String> =
            desired.iter().map(|r| r.Name.clone()).collect();

        // Remove item panels no longer in the model.
        let mut to_remove = Vec::new();
        self.item_panels.retain(|(name, panel_id)| {
            if desired_names.contains(name) {
                true
            } else {
                to_remove.push(*panel_id);
                false
            }
        });
        for panel_id in to_remove {
            ctx.delete_child(panel_id);
        }

        // Build lookup: item name → existing panel_id.
        let existing: std::collections::HashMap<String, PanelId> = self
            .item_panels
            .iter()
            .map(|(n, id)| (n.clone(), *id))
            .collect();

        // Create or update item panels.
        let mut new_item_panels: Vec<(String, PanelId)> = Vec::new();

        for rec in &desired {
            let child_name = rec.Name.clone();

            let child_id = if let Some(&existing_id) = existing.get(&rec.Name) {
                if let Some(mut beh) = ctx.tree.take_behavior(existing_id) {
                    if let Some(item_panel) = beh
                        .as_any_mut()
                        .downcast_mut::<emVirtualCosmosItemPanel>()
                    {
                        item_panel.SetItemRec(rec.clone());
                    }
                    ctx.tree.put_behavior(existing_id, beh);
                }
                existing_id
            } else {
                let mut item_panel = emVirtualCosmosItemPanel::new(Rc::clone(&self.ctx));
                item_panel.SetItemRec(rec.clone());
                ctx.create_child_with(&child_name, Box::new(item_panel))
            };

            new_item_panels.push((rec.Name.clone(), child_id));
        }

        self.item_panels = new_item_panels;

        // Layout each item panel.
        self.layout_items(ctx);
    }

    fn layout_items(&self, ctx: &mut PanelCtx) {
        let desired: Vec<emVirtualCosmosItemRec> = {
            let m = self.model.borrow();
            m.GetItemRecs().cloned().collect()
        };
        for (name, child_id) in &self.item_panels {
            if let Some(rec) = desired.iter().find(|r| r.Name == *name) {
                let b_frac = rec.ContentTallness.min(1.0) * rec.BorderScaling;
                let l = b_frac * 0.03;
                let t = b_frac * 0.05;
                let r = b_frac * 0.03;
                let b = b_frac * 0.03;
                let content_w = 1.0 - l - r;
                let content_h = content_w * rec.ContentTallness;
                let item_height = (content_h + t + b) * rec.Width;
                if rec.Width < 1e-100 || item_height < 1e-100 {
                    continue;
                }
                ctx.layout_child(*child_id, rec.PosX, rec.PosY, rec.Width, item_height);
            }
        }
    }
}

impl PanelBehavior for emVirtualCosmosPanel {
    fn IsOpaque(&self) -> bool {
        // Starfield background is fully opaque (black).
        true
    }

    fn get_title(&self) -> Option<String> {
        Some("Virtual Cosmos".to_string())
    }

    fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool {
        // C++ emVirtualCosmosPanel::Cycle polls model change signal and
        // calls UpdateChildren on change. We drain the notice flag here.
        if self.needs_update {
            self.needs_update = false;
            self.update_children(ctx);
        }
        false
    }

    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState) {
        // C++ Notice(NF_VIEWING_CHANGED) calls UpdateChildren. We defer
        // to Cycle since notice() has no PanelCtx.
        if flags.intersects(NoticeFlags::VIEW_CHANGED) {
            self.needs_update = true;
        }
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // C++ emVirtualCosmosPanel doesn't override AutoExpand — it
        // creates children in Notice(NF_VIEWING_CHANGED) via
        // UpdateChildren. We hook AutoExpand to eagerly create items
        // when the panel becomes viewed, matching the C++ behavior.
        self.update_children(ctx);
    }

    fn AutoShrink(&mut self, _ctx: &mut PanelCtx) {
        // Default AutoShrink deletes children with created_by_ae=true.
        // Clear our internal references so AutoExpand recreates.
        self.background_panel = None;
        self.item_panels.clear();
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Layout background.
        if let Some(bg_id) = self.background_panel {
            let lr = ctx.layout_rect();
            let tallness = if lr.w > 1e-100 { lr.h / lr.w } else { 1.0 };
            ctx.layout_child(bg_id, 0.0, 0.0, 1.0, tallness);
        }
        // Layout items.
        self.layout_items(ctx);
    }

    fn Paint(&mut self, _painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {
        // Background is drawn by the emStarFieldPanel child. Nothing to paint here.
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRec::RecStruct;
    use emcore::emRecRecord::Record;

    #[test]
    fn test_item_panel_new() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
        assert!(panel.item_rec.is_none());
        assert!(panel.content_panel.is_none());
    }

    #[test]
    fn test_item_panel_set_item_rec() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
        let mut item = emVirtualCosmosItemRec::default();
        item.Title = "Test".to_string();
        item.PosX = 0.5;
        panel.SetItemRec(item);
        assert!(panel.item_rec.is_some());
        assert!(panel.update_needed);
    }

    #[test]
    fn test_item_panel_calc_borders() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
        let mut item = emVirtualCosmosItemRec::default();
        item.ContentTallness = 1.0;
        item.BorderScaling = 1.0;
        panel.SetItemRec(item);
        let (l, t, r, b) = panel.CalcBorders();
        // b_frac = min(1.0, 1.0) * 1.0 = 1.0
        assert!((l - 0.03).abs() < 1e-10);
        assert!((t - 0.05).abs() < 1e-10);
        assert!((r - 0.03).abs() < 1e-10);
        assert!((b - 0.03).abs() < 1e-10);
    }

    #[test]
    fn test_cosmos_panel_new() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emVirtualCosmosPanel::new(Rc::clone(&ctx));
        assert_eq!(panel.get_title(), Some("Virtual Cosmos".to_string()));
    }

    #[test]
    fn test_item_panel_behavior() {
        use emcore::emPanel::PanelBehavior;
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn test_cosmos_panel_behavior() {
        use emcore::emPanel::PanelBehavior;
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emVirtualCosmosPanel::new(Rc::clone(&ctx));
        assert!(panel.IsOpaque());
    }

    #[test]
    fn test_item_rec_defaults() {
        let item = emVirtualCosmosItemRec::default();
        assert_eq!(item.Title, "");
        assert!((item.PosX).abs() < 1e-10);
        assert!((item.PosY).abs() < 1e-10);
        assert!((item.Width - 0.1).abs() < 1e-10);
        assert!((item.ContentTallness - 1.0).abs() < 1e-10);
        assert!((item.BorderScaling - 1.0).abs() < 1e-10);
        assert!(item.Focusable);
        assert_eq!(item.FileName, "unnamed");
        assert!(!item.CopyToUser);
        assert_eq!(item.Alternative, 0);
    }

    #[test]
    fn test_item_rec_round_trip() {
        let mut item = emVirtualCosmosItemRec::default();
        item.Title = "Home".to_string();
        item.PosX = 0.5;
        item.PosY = 0.3;
        item.Width = 0.2;
        item.FileName = "Home".to_string();
        let rec = item.to_rec();
        let loaded = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.Title, "Home");
        assert!((loaded.PosX - 0.5).abs() < 1e-10);
        assert_eq!(loaded.FileName, "Home");
    }

    #[test]
    fn test_item_rec_clamp_width() {
        let mut rec = RecStruct::new();
        rec.set_double("Width", 5.0); // above max 1.0
        let item = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert!((item.Width - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_model_empty_dir() {
        let model = emVirtualCosmosModel::from_items(vec![]);
        assert_eq!(model.GetItemCount(), 0);
    }

    #[test]
    fn test_model_sorted_by_position() {
        let mut item1 = emVirtualCosmosItemRec::default();
        item1.PosX = 0.8;
        item1.PosY = 0.5;
        item1.Name = "B".to_string();

        let mut item2 = emVirtualCosmosItemRec::default();
        item2.PosX = 0.2;
        item2.PosY = 0.1;
        item2.Name = "A".to_string();

        let model = emVirtualCosmosModel::from_items(vec![
            LoadedItem {
                file_name: "B.emVcItem".to_string(),
                mtime: std::time::SystemTime::UNIX_EPOCH,
                item_rec: item1,
            },
            LoadedItem {
                file_name: "A.emVcItem".to_string(),
                mtime: std::time::SystemTime::UNIX_EPOCH,
                item_rec: item2,
            },
        ]);

        let sorted: Vec<_> = model.GetItemRecs().collect();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].Name, "A"); // PosY=0.1 comes first
        assert_eq!(sorted[1].Name, "B"); // PosY=0.5 comes second
    }

    #[test]
    fn test_item_rec_color_defaults() {
        let item = emVirtualCosmosItemRec::default();
        assert_eq!(item.BackgroundColor, emColor::from_packed(0xAAAAAAFF));
        assert_eq!(item.BorderColor, emColor::from_packed(0xAAAAAAFF));
        assert_eq!(item.TitleColor, emColor::from_packed(0x000000FF));
    }

    #[test]
    fn test_item_rec_color_round_trip() {
        let mut item = emVirtualCosmosItemRec::default();
        item.BackgroundColor = emColor::rgba(10, 20, 30, 200);
        item.TitleColor = emColor::rgba(255, 0, 0, 255);
        let rec = item.to_rec();
        let loaded = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.BackgroundColor, item.BackgroundColor);
        assert_eq!(loaded.TitleColor, item.TitleColor);
    }

    #[test]
    fn test_item_rec_clamp_posx() {
        let mut rec = RecStruct::new();
        rec.set_double("posx", 1.5); // above max 1.0
        let item = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert!((item.PosX - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_try_prepare_item_file_no_copy() {
        let mut item = emVirtualCosmosItemRec::default();
        item.FileName = "foo.tga".to_string();
        item.CopyToUser = false;
        item.TryPrepareItemFile("/orig", "/user");
        assert_eq!(item.ItemFilePath, "/orig/foo.tga");
    }

    #[test]
    fn test_try_prepare_item_file_copy_to_user() {
        let tmp = std::env::temp_dir().join("eaglemode_test_copy_to_user");
        let orig = tmp.join("orig");
        let user = tmp.join("user");
        // Clean up from any prior run.
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&orig).unwrap();

        let src_file = orig.join("hello.txt");
        std::fs::write(&src_file, b"hello").unwrap();

        let mut item = emVirtualCosmosItemRec::default();
        item.FileName = "hello.txt".to_string();
        item.CopyToUser = true;
        item.TryPrepareItemFile(
            &orig.to_string_lossy(),
            &user.to_string_lossy(),
        );

        let expected = user.join("hello.txt");
        assert_eq!(item.ItemFilePath, expected.to_string_lossy());
        assert!(expected.exists(), "file should have been copied to user dir");
        assert_eq!(std::fs::read(&expected).unwrap(), b"hello");

        // Second call should not fail (file already exists).
        item.TryPrepareItemFile(
            &orig.to_string_lossy(),
            &user.to_string_lossy(),
        );
        assert_eq!(item.ItemFilePath, expected.to_string_lossy());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_try_prepare_item_file_copy_fallback_on_missing_src() {
        let tmp = std::env::temp_dir().join("eaglemode_test_copy_fallback");
        let orig = tmp.join("orig");
        let user = tmp.join("user");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&orig).unwrap();
        // Do NOT create the source file — copy should fail and fall back.

        let mut item = emVirtualCosmosItemRec::default();
        item.FileName = "missing.txt".to_string();
        item.CopyToUser = true;
        item.TryPrepareItemFile(
            &orig.to_string_lossy(),
            &user.to_string_lossy(),
        );

        // Should fall back to orig path.
        let expected_fallback = orig.join("missing.txt");
        assert_eq!(item.ItemFilePath, expected_fallback.to_string_lossy());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_vcitem_hex_color_parsing() {
        // C++ .emVcItem files use hex string colors like "#BBB"
        let mut rec = RecStruct::new();
        rec.set_str("backgroundcolor", "#BBB");
        rec.set_str("bordercolor", "#333");
        rec.set_str("titlecolor", "#EEF");
        rec.set_str("filename", "test.emFileLink");

        let item = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        // #BBB = #BBBBBB = RGB(187, 187, 187)
        assert_eq!(
            item.BackgroundColor.GetRed(),
            187,
            "BackgroundColor red should be 187, got {}",
            item.BackgroundColor.GetRed()
        );
        assert_eq!(item.BackgroundColor.GetGreen(), 187);
        assert_eq!(item.BackgroundColor.GetBlue(), 187);
        // #333 = #333333 = RGB(51, 51, 51)
        assert_eq!(item.BorderColor.GetRed(), 51);
        assert_eq!(item.BorderColor.GetGreen(), 51);
        assert_eq!(item.BorderColor.GetBlue(), 51);
        // #EEF = #EEEEFF = RGB(238, 238, 255)
        assert_eq!(item.TitleColor.GetRed(), 238);
        assert_eq!(item.TitleColor.GetGreen(), 238);
        assert_eq!(item.TitleColor.GetBlue(), 255);
    }

    #[test]
    fn test_vcitem_struct_color_still_works() {
        // Ensure the struct-format colors (from to_rec) still parse
        let mut item = emVirtualCosmosItemRec::default();
        item.BackgroundColor = emColor::rgba(10, 20, 30, 255);
        let rec = item.to_rec();
        let loaded = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.BackgroundColor, item.BackgroundColor);
    }
}
