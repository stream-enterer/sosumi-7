use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::{PanelCtx, SignalCtx};
use emcore::emFpPlugin::{FileStatMode, PanelParentArg, emFpPluginList};
use emcore::emImage::emImage;
use emcore::emInstallInfo::{InstallDirType, emGetConfigDirOverloadable, emGetInstallPath};
use emcore::emPainter::{TextAlignment, VAlign, emPainter};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::{AutoplayHandlingFlags, PanelId};
use emcore::emRecParser::{RecError, RecStruct, RecValue};
use emcore::emRecRecTypes::{em_color_from_rec_struct, em_color_to_rec_struct};
use emcore::emRecRecord::Record;
use emcore::emResTga::load_tga;
use emcore::emSignal::SignalId;
use slotmap::Key as _;

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
        let border_color = parse_color_field(rec, "bordercolor", emColor::from_packed(0xAAAAAAFF));
        let title_color = parse_color_field(rec, "titlecolor", emColor::from_packed(0x000000FF));

        Ok(Self {
            Name: rec.get_str("name").unwrap_or("").to_string(),
            Title: rec.get_str("title").unwrap_or("").to_string(),
            PosX: rec.get_double("posx").unwrap_or(0.0).clamp(0.0, 1.0),
            PosY: rec.get_double("posy").unwrap_or(0.0).clamp(0.0, 1.0),
            Width: rec.get_double("width").unwrap_or(0.1).clamp(1e-10, 1.0),
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
            FileName: rec.get_str("filename").unwrap_or("unnamed").to_string(),
            CopyToUser: rec.get_bool("copytouser").unwrap_or(false),
            Alternative: rec.get_int("alternative").unwrap_or(0).max(0),
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
            RecValue::Struct(em_color_to_rec_struct(self.BackgroundColor, true)),
        );
        s.SetValue(
            "bordercolor",
            RecValue::Struct(em_color_to_rec_struct(self.BorderColor, true)),
        );
        s.SetValue(
            "titlecolor",
            RecValue::Struct(em_color_to_rec_struct(self.TitleColor, true)),
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
        && let Ok(c) = em_color_from_rec_struct(s, true)
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
    /// Port of C++ emVirtualCosmosModel::ChangeSignal (emVirtualCosmos.h).
    /// Lazily allocated on first subscribe per D-008-signal-allocation-shape (A1).
    /// Fired by `Reload` post-Acquire (emVirtualCosmos.cpp:226 in C++; Rust
    /// callsite is benign-no-op per CALLSITE-NOTE on Reload).
    change_signal: Cell<SignalId>,
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
                change_signal: Cell::new(SignalId::null()),
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
            change_signal: Cell::new(SignalId::null()),
        };
        model.sort_item_recs();
        model
    }

    /// Port of C++ `emVirtualCosmosModel::GetChangeSignal()`.
    /// Lazily allocates the signal on first call per D-008 A1 (combined-form
    /// accessor; B-003 precedent on `emAutoplayViewModel::GetChangeSignal`).
    pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let sig = self.change_signal.get();
        if sig.is_null() {
            let new_sig = ectx.create_signal();
            self.change_signal.set(new_sig);
            new_sig
        } else {
            sig
        }
    }

    /// Reload items from disk.
    ///
    /// Port of C++ `emVirtualCosmosModel::Reload`.
    ///
    /// CALLSITE-NOTE: The single existing callsite is the Acquire-bootstrap
    /// closure (see `Acquire`), which has no `SignalCtx` and runs before any
    /// panel has subscribed. Per D-008 A1 lazy allocation, `change_signal ==
    /// SignalId::null()` at that moment, so a missing fire is benign-by-
    /// construction (the composition of D-007/D-008 absorbs it). Future
    /// post-Acquire callers (e.g., a future port of `emVirtualCosmosModel::
    /// Cycle` reacting to `FileUpdateSignalModel`) MUST thread `&mut impl
    /// SignalCtx` and fire via `ectx.fire(self.change_signal.get())` after a
    /// successful reload (skipping the fire if `change_signal.is_null()`),
    /// mirroring B-003's `signal_change` mutator-fire pattern on
    /// `emAutoplayViewModel`.
    pub fn Reload(&mut self) {
        let items_dir = emGetConfigDirOverloadable("emMain", Some("VcItems"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item_files_dir = emGetConfigDirOverloadable("emMain", Some("VcItemFiles"))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let item_files_user_dir = emGetInstallPath(
            InstallDirType::UserConfig,
            "emMain",
            Some("VcItemFiles.user"),
        )
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

        self.items_dir = items_dir.clone();
        self.item_files_dir = item_files_dir.clone();

        let dir_entries = match std::fs::read_dir(&items_dir) {
            Ok(d) => d,
            Err(e) => {
                log::warn!(
                    "emVirtualCosmosModel: cannot read dir '{}': {}",
                    items_dir,
                    e
                );
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
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        _w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        let Some(rec) = &self.item_rec else {
            return;
        };

        if rec.BorderScaling <= 1e-100 {
            painter.ClearWithCanvas(rec.BackgroundColor, canvas_color);
            return;
        }

        let bor_col = rec.BorderColor;
        let (l, t, r, b) = self.CalcBorders();

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
            painter.PaintRect(x1, y1, x2 - x1, y2 - y1, rec.BackgroundColor, canvas_color);
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
            0.0,
            0.0,
            1.0,
            h,
            d,
            d,
            d,
            d,
            &self.OuterBorderImage,
            82,
            82,
            82,
            82,
            255,
            bor_col,
            0o757,
        );

        // Inner border image
        let e = l * 0.5;
        let f = e * (23.0 / 126.0);
        painter.PaintBorderImage(
            l - e,
            t - f,
            1.0 - l - r + e * 2.0,
            h - t - b + f * 2.0,
            e,
            f,
            e,
            f,
            &self.InnerBorderImage,
            126,
            23,
            126,
            23,
            255,
            bor_col,
            0o757,
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
            ctx,
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
        ctx.wake_up_panel(child_id);
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

    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::VIEWING_CHANGED | NoticeFlags::LAYOUT_CHANGED) {
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
    /// B-014 row -575: D-006 first-Cycle init gate. True after Cycle has
    /// subscribed to the model's `ChangeSignal` (mirrors C++ ctor's
    /// `AddWakeUpSignal(Model->GetChangeSignal())`).
    subscribed_init: bool,
    /// B-008 row -104: cached `App::file_update_signal` from the scheduler.
    /// Allocated by `App::new`; null in test contexts that never set it.
    /// Subscribed-to in the first-Cycle init alongside `ChangeSignal`.
    file_update_signal: SignalId,
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
            subscribed_init: false,
            file_update_signal: SignalId::null(),
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
            ctx.tree
                .SetAutoplayHandling(child_id, AutoplayHandlingFlags::CUTOFF);
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
                    if let Some(item_panel) =
                        beh.as_any_mut().downcast_mut::<emVirtualCosmosItemPanel>()
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

    fn Cycle(&mut self, ectx: &mut emcore::emEngineCtx::EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        // B-014 row -575: D-006 first-Cycle init — subscribe to model's
        // ChangeSignal. Mirrors C++ ctor `AddWakeUpSignal(Model->GetChangeSignal())`
        // (emVirtualCosmos.cpp:575). Lazy alloc per D-008 A1.
        //
        // B-008 row -104: also subscribe to the shared file-update broadcast
        // (`App::file_update_signal`, returned by `emFileModel::AcquireUpdateSignalModel`
        // post-B-007). Mirrors C++ `emVirtualCosmosModel` ctor at
        // emVirtualCosmosPanel.cpp:104, which adds the broadcast to the
        // model's wake-up set so any file write triggers a `Reload()`.
        //
        // The C++ subscribe lives on the model itself (`emVirtualCosmosModel : emEngine`).
        // In Rust the model is not an emEngine, so we host the subscribe on
        // its sole consumer panel — open-question 4 of the B-008 design doc
        // explicitly leaves this implementer's pick (driving engine vs. panel
        // first-Cycle subscribe); panel-side preserves the observable
        // contract (broadcast wake → `Reload`) without threading a
        // ConstructCtx through `emVirtualCosmosModel::Acquire`.
        if !self.subscribed_init {
            let eid = ectx.id();
            let sig = self.model.borrow().GetChangeSignal(ectx);
            ectx.connect(sig, eid);
            let upd = emcore::emFileModel::emFileModel::<()>::AcquireUpdateSignalModel(ectx);
            if !upd.is_null() {
                ectx.connect(upd, eid);
                self.file_update_signal = upd;
            }
            self.subscribed_init = true;
        }

        // B-008 row -104: file-update broadcast reaction — mirrors C++
        // `emVirtualCosmosModel::Cycle` reacting to `FileUpdateSignalModel->Sig`
        // by calling `Reload()`. The reload mutates the model's item list,
        // which fires `ChangeSignal` (post-B-014); the panel's existing
        // ChangeSignal branch below then runs `update_children`.
        if !self.file_update_signal.is_null() && ectx.IsSignaled(self.file_update_signal) {
            // Borrow the model mutably for Reload, then fire ChangeSignal so
            // the panel's existing reaction path picks up the new items in a
            // single Cycle (matches C++ where Reload's `Signal(ChangeSignal)`
            // is observed in the same time slice).
            self.model.borrow_mut().Reload();
            let chg = self.model.borrow().GetChangeSignal(ectx);
            if !chg.is_null() {
                ectx.fire(chg);
            }
        }

        // B-014 row -575: ChangeSignal reaction — mirrors C++ Cycle's
        // `IsSignaled(Model->GetChangeSignal()) → UpdateChildren()`
        // (emVirtualCosmos.cpp:606).
        let change_sig = self.model.borrow().GetChangeSignal(ectx);
        if ectx.IsSignaled(change_sig) {
            self.update_children(ctx);
        }

        // C++ Notice(NF_VIEWING_CHANGED) → UpdateChildren (cpp:613). Independent
        // of the ChangeSignal path; both converge on update_children.
        if self.needs_update {
            self.needs_update = false;
            self.update_children(ctx);
        }
        false
    }

    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        // C++ Notice(NF_VIEWING_CHANGED) calls UpdateChildren. We defer
        // to Cycle since notice() has no PanelCtx.
        if flags.intersects(NoticeFlags::VIEWING_CHANGED) {
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

    fn Paint(
        &mut self,
        _painter: &mut emPainter,
        _canvas_color: emColor,
        _w: f64,
        _h: f64,
        _state: &PanelState,
    ) {
        // Background is drawn by the emStarFieldPanel child. Nothing to paint here.
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRecParser::RecStruct;
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
        let item = emVirtualCosmosItemRec {
            Title: "Test".to_string(),
            PosX: 0.5,
            ..emVirtualCosmosItemRec::default()
        };
        panel.SetItemRec(item);
        assert!(panel.item_rec.is_some());
        assert!(panel.update_needed);
    }

    #[test]
    fn test_item_panel_calc_borders() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emVirtualCosmosItemPanel::new(Rc::clone(&ctx));
        let item = emVirtualCosmosItemRec {
            ContentTallness: 1.0,
            BorderScaling: 1.0,
            ..emVirtualCosmosItemRec::default()
        };
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
        let item = emVirtualCosmosItemRec {
            Title: "Home".to_string(),
            PosX: 0.5,
            PosY: 0.3,
            Width: 0.2,
            FileName: "Home".to_string(),
            ..emVirtualCosmosItemRec::default()
        };
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
        let item1 = emVirtualCosmosItemRec {
            PosX: 0.8,
            PosY: 0.5,
            Name: "B".to_string(),
            ..emVirtualCosmosItemRec::default()
        };

        let item2 = emVirtualCosmosItemRec {
            PosX: 0.2,
            PosY: 0.1,
            Name: "A".to_string(),
            ..emVirtualCosmosItemRec::default()
        };

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
        let item = emVirtualCosmosItemRec {
            BackgroundColor: emColor::rgba(10, 20, 30, 200),
            TitleColor: emColor::rgba(255, 0, 0, 255),
            ..emVirtualCosmosItemRec::default()
        };
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
        let mut item = emVirtualCosmosItemRec {
            FileName: "foo.tga".to_string(),
            CopyToUser: false,
            ..emVirtualCosmosItemRec::default()
        };
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

        let mut item = emVirtualCosmosItemRec {
            FileName: "hello.txt".to_string(),
            CopyToUser: true,
            ..emVirtualCosmosItemRec::default()
        };
        item.TryPrepareItemFile(&orig.to_string_lossy(), &user.to_string_lossy());

        let expected = user.join("hello.txt");
        assert_eq!(item.ItemFilePath, expected.to_string_lossy());
        assert!(
            expected.exists(),
            "file should have been copied to user dir"
        );
        assert_eq!(std::fs::read(&expected).unwrap(), b"hello");

        // Second call should not fail (file already exists).
        item.TryPrepareItemFile(&orig.to_string_lossy(), &user.to_string_lossy());
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

        let mut item = emVirtualCosmosItemRec {
            FileName: "missing.txt".to_string(),
            CopyToUser: true,
            ..emVirtualCosmosItemRec::default()
        };
        item.TryPrepareItemFile(&orig.to_string_lossy(), &user.to_string_lossy());

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

    /// B-014 row -575: model-level signal-allocation test. Verifies the
    /// combined-form `GetChangeSignal(ectx)` accessor lazy-allocates on first
    /// call and is idempotent on subsequent calls. End-to-end fire-driven
    /// behavior is covered by `b014_row_575_change_signal_drives_update_children`.
    ///
    /// Mirrors C++ emVirtualCosmos.cpp:226 `Signal(ChangeSignal)` semantics
    /// at the allocation/identity layer.
    #[test]
    fn b014_row_575_change_signal_lazy_alloc_and_idempotence() {
        use emcore::emEngineCtx::EngineCtx;
        use emcore::emScheduler::EngineScheduler;
        use slotmap::Key as _;
        use std::collections::HashMap;

        let model = emVirtualCosmosModel::from_items(vec![]);
        // Pre-allocation: the cell is null until first GetChangeSignal call.
        assert!(
            model.change_signal.get().is_null(),
            "before first GetChangeSignal call, change_signal must be null"
        );

        let mut sched = EngineScheduler::new();
        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let fw_cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
            HashMap::new();

        // Register a stub engine so we have a valid EngineId for the EngineCtx.
        let mut tree = emcore::emPanelTree::PanelTree::new();
        let stub_id = tree.create_root_deferred_view("b014_stub");
        tree.set_panel_view(stub_id);
        tree.register_engine_for_public(stub_id, Some(&mut sched));
        let engine_id = tree.panel_engine_id_pub(stub_id).expect("engine");

        // Allocate the signal lazily.
        let sig = {
            let mut ectx = EngineCtx {
                scheduler: &mut sched,
                tree: None,
                windows: &mut windows,
                root_context: &root_ctx,
                view_context: None,
                framework_actions: &mut fw_actions,
                pending_inputs: &mut pending_inputs,
                input_state: &mut input_state,
                framework_clipboard: &fw_cb,
                engine_id,
                pending_actions: &pa,
            };
            model.GetChangeSignal(&mut ectx)
        };
        assert!(!sig.is_null(), "GetChangeSignal must return a non-null id");
        assert_eq!(
            sig,
            model.change_signal.get(),
            "stored change_signal must equal the just-allocated id"
        );
        // Idempotent: a second GetChangeSignal returns the same id.
        let sig2 = {
            let mut ectx = EngineCtx {
                scheduler: &mut sched,
                tree: None,
                windows: &mut windows,
                root_context: &root_ctx,
                view_context: None,
                framework_actions: &mut fw_actions,
                pending_inputs: &mut pending_inputs,
                input_state: &mut input_state,
                framework_clipboard: &fw_cb,
                engine_id,
                pending_actions: &pa,
            };
            model.GetChangeSignal(&mut ectx)
        };
        assert_eq!(sig2, sig, "GetChangeSignal must be idempotent");

        // Cleanup.
        let all_ids = tree.panel_ids();
        for pid in all_ids {
            if let Some(eid) = tree.panel_engine_id_pub(pid) {
                sched.remove_engine(eid);
            }
        }
        sched.abort_all_pending();
        sched.remove_signal(sig);
    }

    /// B-014 row -575 click-through: emVirtualCosmosPanel::Cycle subscribes to
    /// the model's ChangeSignal via first-Cycle init, and on a subsequent fire
    /// calls `update_children` (observable via `background_panel.is_some()`).
    ///
    /// Mirrors C++ emVirtualCosmos.cpp:575 (ctor `AddWakeUpSignal`) + cpp:606
    /// (Cycle's `IsSignaled → UpdateChildren`).
    #[test]
    fn b014_row_575_change_signal_drives_update_children() {
        use emcore::emEngineCtx::{EngineCtx, PanelCtx};
        use emcore::emScheduler::EngineScheduler;
        use std::collections::HashMap;

        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut sched = EngineScheduler::new();

        let mut tree = emcore::emPanelTree::PanelTree::new();
        let root_id = tree.create_root_deferred_view("vc_575");
        tree.set_behavior(
            root_id,
            Box::new(emVirtualCosmosPanel::new(Rc::clone(&root_ctx))),
        );
        tree.set_panel_view(root_id);
        tree.register_engine_for_public(root_id, Some(&mut sched));
        let engine_id = tree.panel_engine_id_pub(root_id).expect("engine");

        let fw_cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();
        let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
            HashMap::new();

        // Reset needs_update=false so we can observe ChangeSignal-driven updates
        // independently of the notice-driven path.
        let model_handle: Rc<RefCell<emVirtualCosmosModel>> = {
            let mut behavior = tree.take_behavior(root_id).expect("behavior");
            let panel = behavior
                .as_any_mut()
                .downcast_mut::<emVirtualCosmosPanel>()
                .expect("emVirtualCosmosPanel");
            panel.needs_update = false;
            let m = Rc::clone(&panel.model);
            tree.put_behavior(root_id, behavior);
            m
        };

        // Drive Cycle once to run first-Cycle init (subscribe to ChangeSignal).
        // No fire pending → update_children should not be called.
        let mut behavior_slot = tree.take_behavior(root_id).expect("behavior");
        {
            // SAFETY: ectx.scheduler/framework_actions and pctx.scheduler/framework_actions
            // alias the same values for the duration of Cycle. Single-threaded; mirrors how
            // the real frame-driver hands both contexts to a panel.
            let sched_ptr: *mut EngineScheduler = &mut sched;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut fw_actions;
            let mut pctx = PanelCtx::with_sched_reach(
                &mut tree,
                root_id,
                1.0,
                unsafe { &mut *sched_ptr },
                unsafe { &mut *fw_ptr },
                &root_ctx,
                &fw_cb,
                &pa,
            );
            let mut ectx = EngineCtx {
                scheduler: &mut sched,
                tree: None,
                windows: &mut windows,
                root_context: &root_ctx,
                view_context: None,
                framework_actions: &mut fw_actions,
                pending_inputs: &mut pending_inputs,
                input_state: &mut input_state,
                framework_clipboard: &fw_cb,
                engine_id,
                pending_actions: &pa,
            };
            behavior_slot.Cycle(&mut ectx, &mut pctx);
        }
        tree.put_behavior(root_id, behavior_slot);

        // Verify subscribe happened and update_children did not run.
        // After first-Cycle init, the cell is populated; read it directly
        // (the GetChangeSignal accessor needs an ectx, but we just want the id).
        let change_sig = model_handle.borrow().change_signal.get();
        assert!(
            !change_sig.is_null(),
            "after first-Cycle init, ChangeSignal must be allocated"
        );
        {
            let mut behavior = tree.take_behavior(root_id).expect("behavior");
            let panel = behavior
                .as_any_mut()
                .downcast_mut::<emVirtualCosmosPanel>()
                .expect("emVirtualCosmosPanel");
            assert!(
                panel.subscribed_init,
                "subscribed_init must be true after first Cycle"
            );
            assert!(
                panel.background_panel.is_none(),
                "background_panel must still be None: no signal fired yet"
            );
            tree.put_behavior(root_id, behavior);
        }

        // Fire the ChangeSignal and run another Cycle. update_children must run
        // (creates the starfield background panel as a side-effect).
        sched.fire(change_sig);
        sched.flush_signals_for_test();

        let mut behavior_slot = tree.take_behavior(root_id).expect("behavior");
        {
            // SAFETY: ectx.scheduler/framework_actions and pctx.scheduler/framework_actions
            // alias the same values; same aliasing rationale as the first Cycle invocation.
            let sched_ptr: *mut EngineScheduler = &mut sched;
            let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut fw_actions;
            let mut pctx = PanelCtx::with_sched_reach(
                &mut tree,
                root_id,
                1.0,
                unsafe { &mut *sched_ptr },
                unsafe { &mut *fw_ptr },
                &root_ctx,
                &fw_cb,
                &pa,
            );
            let mut ectx = EngineCtx {
                scheduler: &mut sched,
                tree: None,
                windows: &mut windows,
                root_context: &root_ctx,
                view_context: None,
                framework_actions: &mut fw_actions,
                pending_inputs: &mut pending_inputs,
                input_state: &mut input_state,
                framework_clipboard: &fw_cb,
                engine_id,
                pending_actions: &pa,
            };
            behavior_slot.Cycle(&mut ectx, &mut pctx);
        }
        tree.put_behavior(root_id, behavior_slot);

        {
            let mut behavior = tree.take_behavior(root_id).expect("behavior");
            let panel = behavior
                .as_any_mut()
                .downcast_mut::<emVirtualCosmosPanel>()
                .expect("emVirtualCosmosPanel");
            assert!(
                panel.background_panel.is_some(),
                "background_panel must be Some after ChangeSignal-driven update_children"
            );
            tree.put_behavior(root_id, behavior);
        }

        // Cleanup.
        let all_ids = tree.panel_ids();
        for pid in all_ids {
            if let Some(eid) = tree.panel_engine_id_pub(pid) {
                sched.remove_engine(eid);
            }
        }
        sched.abort_all_pending();
    }

    /// B-008 row -104 click-through.
    ///
    /// File-update broadcast arrival drives `emVirtualCosmosModel::Reload()`
    /// and re-fires `ChangeSignal`, mirroring C++
    /// `emVirtualCosmosModel::Cycle` reacting to `FileUpdateSignalModel->Sig`.
    /// Four-question audit trail:
    ///   (1) connected — first-Cycle init connects file_update_signal to engine.
    ///   (2) observes  — IsSignaled(file_update_signal) branch in panel Cycle.
    ///   (3) reaction  — model.Reload() runs and ChangeSignal becomes pending.
    ///   (4) C++ order — Reload precedes the ChangeSignal-driven update_children
    ///                   (matches C++ where Signal(ChangeSignal) inside Reload
    ///                   is observed in the same time slice).
    #[test]
    fn b008_row_104_file_update_broadcast_reloads_model() {
        use emcore::emEngineCtx::{EngineCtx, PanelCtx};
        use emcore::emScheduler::EngineScheduler;
        use std::collections::HashMap;

        let root_ctx = emcore::emContext::emContext::NewRoot();
        let mut sched = EngineScheduler::new();

        // Simulate App::new — allocate the shared broadcast and store it.
        let file_update_signal = sched.create_signal();
        sched.file_update_signal = file_update_signal;

        // Allocate a real ChangeSignal so the post-Reload fire is observable.
        let change_sig = sched.create_signal();

        let mut panel = emVirtualCosmosPanel::new(Rc::clone(&root_ctx));
        panel.model.borrow().change_signal.set(change_sig);

        // Bypass the first-Cycle init wiring (which would also subscribe
        // ChangeSignal and run AutoExpand/update_children paths that need a
        // full sub-tree). The reaction branch under test only needs
        // `subscribed_init = true` and `file_update_signal` populated.
        panel.subscribed_init = true;
        panel.file_update_signal = file_update_signal;

        let mut tree = emcore::emPanelTree::PanelTree::new();
        let root_id = tree.create_root_deferred_view("vc_104");
        tree.set_panel_view(root_id);
        tree.register_engine_for_public(root_id, Some(&mut sched));
        let engine_id = tree.panel_engine_id_pub(root_id).expect("engine");

        sched.connect(file_update_signal, engine_id);
        sched.connect(change_sig, engine_id);
        sched.fire(file_update_signal);
        sched.flush_signals_for_test();

        let mut windows: HashMap<winit::window::WindowId, emcore::emWindow::emWindow> =
            HashMap::new();
        let fw_cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> = RefCell::new(None);
        let pa: Rc<RefCell<Vec<emcore::emGUIFramework::DeferredAction>>> =
            Rc::new(RefCell::new(Vec::new()));
        let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
        let mut pending_inputs: Vec<(winit::window::WindowId, emcore::emInput::emInputEvent)> =
            Vec::new();
        let mut input_state = emcore::emInputState::emInputState::new();

        let sched_ptr: *mut EngineScheduler = &mut sched;
        let fw_ptr: *mut Vec<emcore::emEngineCtx::DeferredAction> = &mut fw_actions;
        // SAFETY: ectx.scheduler/framework_actions and pctx.scheduler/framework_actions
        // alias the same values; single-threaded, mirrors B-006 row_218 click-through pattern.
        let mut pctx = PanelCtx::with_sched_reach(
            &mut tree,
            root_id,
            1.0,
            unsafe { &mut *sched_ptr },
            unsafe { &mut *fw_ptr },
            &root_ctx,
            &fw_cb,
            &pa,
        );
        let mut ectx = EngineCtx {
            scheduler: &mut sched,
            tree: None,
            windows: &mut windows,
            root_context: &root_ctx,
            view_context: None,
            framework_actions: &mut fw_actions,
            pending_inputs: &mut pending_inputs,
            input_state: &mut input_state,
            framework_clipboard: &fw_cb,
            engine_id,
            pending_actions: &pa,
        };
        panel.Cycle(&mut ectx, &mut pctx);

        // After Cycle: the file-update reaction must have called Reload() and
        // fired ChangeSignal. ChangeSignal is observable via is_pending.
        assert!(
            sched.is_pending(change_sig),
            "Reload() must fire ChangeSignal so consumers re-render"
        );

        // Cleanup.
        let all_ids = tree.panel_ids();
        for pid in all_ids {
            if let Some(eid) = tree.panel_engine_id_pub(pid) {
                sched.remove_engine(eid);
            }
        }
        sched.disconnect(file_update_signal, engine_id);
        sched.disconnect(change_sig, engine_id);
        sched.remove_signal(file_update_signal);
        sched.remove_signal(change_sig);
        sched.abort_all_pending();
    }

    #[test]
    fn test_vcitem_struct_color_still_works() {
        // Ensure the struct-format colors (from to_rec) still parse
        let item = emVirtualCosmosItemRec {
            BackgroundColor: emColor::rgba(10, 20, 30, 255),
            ..emVirtualCosmosItemRec::default()
        };
        let rec = item.to_rec();
        let loaded = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.BackgroundColor, item.BackgroundColor);
    }
}
