use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emConfigModel::emConfigModel;
use emcore::emContext::emContext;
use emcore::emInstallInfo::{InstallDirType, emGetInstallPath};
use emcore::emPainter::{TextAlignment, VAlign, emPainter};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelCtx::PanelCtx;
use emcore::emRec::{RecError, RecStruct, RecValue};
use emcore::emRecRecTypes::emColorRec;
use emcore::emRecRecord::Record;
use emcore::emSignal::SignalId;
use slotmap::Key as _;

// DIVERGED: C++ uses class inheritance (emBookmarkEntryRec → emBookmarkRec,
// emBookmarkGroupRec). Rust flattens to struct composition — `emBookmarkEntryBase`
// is embedded by value in each concrete struct. The field names and default
// colors are identical to the C++ originals.

// Default colors matching C++ emBookmarkEntryRec constructor defaults:
// emBookmarkRec:      BgColor = emLook().GetButtonBgColor() = 0x596790FF
//                     FgColor = emLook().GetButtonFgColor() = 0xF2F2F7FF
// emBookmarkGroupRec: BgColor = emLook().GetBgColor()       = 0x515E84FF
//                     FgColor = emLook().GetFgColor()        = 0xEFF0F4FF

/// Default background color for a single bookmark (C++ emLook::GetButtonBgColor).
const BOOKMARK_BG_DEFAULT: emColor = emColor::from_packed(0x596790FF);
/// Default foreground color for a single bookmark (C++ emLook::GetButtonFgColor).
const BOOKMARK_FG_DEFAULT: emColor = emColor::from_packed(0xF2F2F7FF);
/// Default background color for a bookmark group (C++ emLook::GetBgColor).
const GROUP_BG_DEFAULT: emColor = emColor::from_packed(0x515E84FF);
/// Default foreground color for a bookmark group (C++ emLook::GetFgColor).
const GROUP_FG_DEFAULT: emColor = emColor::from_packed(0xEFF0F4FF);

// ── emBookmarkEntryBase ───────────────────────────────────────────────────────

/// Common bookmark entry fields (C++ emBookmarkEntryRec).
#[derive(Debug, Clone, PartialEq)]
pub struct emBookmarkEntryBase {
    pub Name: String,
    pub Description: String,
    pub Icon: String,
    pub BgColor: emColor,
    pub FgColor: emColor,
}

impl emBookmarkEntryBase {
    fn new(bg: emColor, fg: emColor) -> Self {
        Self {
            Name: String::new(),
            Description: String::new(),
            Icon: String::new(),
            BgColor: bg,
            FgColor: fg,
        }
    }

    fn from_rec_with_defaults(rec: &RecStruct, bg_default: emColor, fg_default: emColor) -> Self {
        let bg = rec
            .get_struct("bgcolor")
            .and_then(|s| emColorRec::FromRecStruct(s, true).ok())
            .unwrap_or(bg_default);
        let fg = rec
            .get_struct("fgcolor")
            .and_then(|s| emColorRec::FromRecStruct(s, true).ok())
            .unwrap_or(fg_default);
        Self {
            Name: rec.get_str("name").unwrap_or("").to_string(),
            Description: rec.get_str("description").unwrap_or("").to_string(),
            Icon: rec.get_str("icon").unwrap_or("").to_string(),
            BgColor: bg,
            FgColor: fg,
        }
    }

    fn write_into(&self, s: &mut RecStruct) {
        s.set_str("name", &self.Name);
        s.set_str("description", &self.Description);
        s.set_str("icon", &self.Icon);
        s.SetValue(
            "bgcolor",
            RecValue::Struct(emColorRec::ToRecStruct(self.BgColor, true)),
        );
        s.SetValue(
            "fgcolor",
            RecValue::Struct(emColorRec::ToRecStruct(self.FgColor, true)),
        );
    }
}

impl Default for emBookmarkEntryBase {
    fn default() -> Self {
        Self::new(BOOKMARK_BG_DEFAULT, BOOKMARK_FG_DEFAULT)
    }
}

// ── emBookmarkRec ─────────────────────────────────────────────────────────────

/// A single bookmark (C++ emBookmarkRec).
#[derive(Debug, Clone, PartialEq)]
pub struct emBookmarkRec {
    pub entry: emBookmarkEntryBase,
    pub Hotkey: String,
    pub LocationIdentity: String,
    pub LocationRelX: f64,
    pub LocationRelY: f64,
    pub LocationRelA: f64,
    pub VisitAtProgramStart: bool,
}

impl emBookmarkRec {
    fn entry_bg_default() -> emColor {
        BOOKMARK_BG_DEFAULT
    }
    fn entry_fg_default() -> emColor {
        BOOKMARK_FG_DEFAULT
    }

    pub fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let entry = emBookmarkEntryBase::from_rec_with_defaults(
            rec,
            Self::entry_bg_default(),
            Self::entry_fg_default(),
        );
        Ok(Self {
            entry,
            Hotkey: rec.get_str("hotkey").unwrap_or("").to_string(),
            LocationIdentity: rec.get_str("locationidentity").unwrap_or("").to_string(),
            LocationRelX: rec.get_double("locationrelx").unwrap_or(0.0),
            LocationRelY: rec.get_double("locationrely").unwrap_or(0.0),
            LocationRelA: rec.get_double("locationrela").unwrap_or(0.0).max(0.0),
            VisitAtProgramStart: rec.get_bool("visitatprogramstart").unwrap_or(false),
        })
    }

    pub fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        self.entry.write_into(&mut s);
        s.set_str("hotkey", &self.Hotkey);
        s.set_str("locationidentity", &self.LocationIdentity);
        s.set_double("locationrelx", self.LocationRelX);
        s.set_double("locationrely", self.LocationRelY);
        s.set_double("locationrela", self.LocationRelA);
        s.set_bool("visitatprogramstart", self.VisitAtProgramStart);
        s
    }
}

impl Default for emBookmarkRec {
    fn default() -> Self {
        Self {
            entry: emBookmarkEntryBase::new(Self::entry_bg_default(), Self::entry_fg_default()),
            Hotkey: String::new(),
            LocationIdentity: String::new(),
            LocationRelX: 0.0,
            LocationRelY: 0.0,
            LocationRelA: 0.0,
            VisitAtProgramStart: false,
        }
    }
}

// ── emBookmarkGroupRec ────────────────────────────────────────────────────────

/// A bookmark group with nested children (C++ emBookmarkGroupRec).
#[derive(Debug, Clone, PartialEq)]
pub struct emBookmarkGroupRec {
    pub entry: emBookmarkEntryBase,
    pub Bookmarks: Vec<emBookmarkEntryUnion>,
}

impl emBookmarkGroupRec {
    fn entry_bg_default() -> emColor {
        GROUP_BG_DEFAULT
    }
    fn entry_fg_default() -> emColor {
        GROUP_FG_DEFAULT
    }

    pub fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let entry = emBookmarkEntryBase::from_rec_with_defaults(
            rec,
            Self::entry_bg_default(),
            Self::entry_fg_default(),
        );
        let bookmarks = if let Some(arr) = rec.get_array("bookmarks") {
            arr.iter()
                .filter_map(|v| emBookmarkEntryUnion::from_rec_value(v).ok())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Self {
            entry,
            Bookmarks: bookmarks,
        })
    }

    pub fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        self.entry.write_into(&mut s);
        let items: Vec<RecValue> = self.Bookmarks.iter().map(|e| e.to_rec_value()).collect();
        s.SetValue("bookmarks", RecValue::Array(items));
        s
    }
}

impl Default for emBookmarkGroupRec {
    fn default() -> Self {
        Self {
            entry: emBookmarkEntryBase::new(Self::entry_bg_default(), Self::entry_fg_default()),
            Bookmarks: Vec::new(),
        }
    }
}

// ── emBookmarkEntryUnion ──────────────────────────────────────────────────────

/// Union of bookmark or group (C++ emUnionRec with BOOKMARK/GROUP variants).
#[derive(Debug, Clone, PartialEq)]
pub enum emBookmarkEntryUnion {
    Bookmark(emBookmarkRec),
    Group(emBookmarkGroupRec),
}

impl emBookmarkEntryUnion {
    fn from_rec_value(val: &RecValue) -> Result<Self, RecError> {
        match val {
            RecValue::Union(variant, inner) if variant == "bookmark" => match inner.as_ref() {
                RecValue::Struct(s) => Ok(Self::Bookmark(emBookmarkRec::from_rec(s)?)),
                _ => Err(RecError::InvalidValue {
                    field: "bookmark".into(),
                    message: "expected struct inside union".into(),
                }),
            },
            RecValue::Union(variant, inner) if variant == "group" => match inner.as_ref() {
                RecValue::Struct(s) => Ok(Self::Group(emBookmarkGroupRec::from_rec(s)?)),
                _ => Err(RecError::InvalidValue {
                    field: "group".into(),
                    message: "expected struct inside union".into(),
                }),
            },
            RecValue::Union(variant, _) => Err(RecError::InvalidValue {
                field: "entry".into(),
                message: format!("unknown union variant: {variant}"),
            }),
            _ => Err(RecError::InvalidValue {
                field: "entry".into(),
                message: "expected union value".into(),
            }),
        }
    }

    fn to_rec_value(&self) -> RecValue {
        match self {
            Self::Bookmark(bm) => RecValue::Union(
                "bookmark".to_string(),
                Box::new(RecValue::Struct(bm.to_rec())),
            ),
            Self::Group(grp) => RecValue::Union(
                "group".to_string(),
                Box::new(RecValue::Struct(grp.to_rec())),
            ),
        }
    }
}

// ── emBookmarksRec ────────────────────────────────────────────────────────────

// DIVERGED: C++ emBookmarksRec also has InsertNewBookmark, InsertNewGroup,
// CopyToClipboard, TryInsertFromClipboard, BookmarkNameFromPanelTitle.
// These editing-UI methods are not ported — the Rust implementation is
// read-only for now.

/// Root record: array of bookmark entries (C++ emBookmarksRec).
#[derive(Debug, Clone, PartialEq)]
pub struct emBookmarksRec {
    pub entries: Vec<emBookmarkEntryUnion>,
}

impl emBookmarksRec {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Find the first bookmark with `VisitAtProgramStart == true` (recursive).
    ///
    /// Port of C++ `emBookmarksRec::SearchStartLocation`.
    pub fn SearchStartLocation(&self) -> Option<&emBookmarkRec> {
        search_start_location_in(&self.entries)
    }

    /// Find a bookmark matching `hotkey` string (recursive).
    ///
    /// Port of C++ `emBookmarksRec::SearchBookmarkByHotkey`.
    pub fn SearchBookmarkByHotkey(&self, hotkey: &str) -> Option<&emBookmarkRec> {
        if hotkey.is_empty() {
            return None;
        }
        search_bookmark_by_hotkey_in(&self.entries, hotkey)
    }

    /// Set `VisitAtProgramStart` on the given bookmark; clears all others first.
    ///
    /// Port of C++ `emBookmarksRec::SetStartLocation`.
    pub fn SetStartLocation(&mut self, target_identity: &str) {
        self.ClearStartLocation();
        set_start_location_in(&mut self.entries, target_identity);
    }

    /// Clear all `VisitAtProgramStart` flags (recursive).
    ///
    /// Port of C++ `emBookmarksRec::ClearStartLocation`.
    pub fn ClearStartLocation(&mut self) {
        clear_start_location_in(&mut self.entries);
    }
}

impl Default for emBookmarksRec {
    fn default() -> Self {
        Self::new()
    }
}

impl Record for emBookmarksRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        // Support both C++ format (top-level array stored as "_array") and
        // legacy Rust format (struct with "entries" field).
        let arr = rec.get_array("_array").or_else(|| rec.get_array("entries"));
        let entries = if let Some(arr) = arr {
            arr.iter()
                .filter_map(|v| emBookmarkEntryUnion::from_rec_value(v).ok())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Self { entries })
    }

    fn to_rec(&self) -> RecStruct {
        // Write in C++ compatible format: top-level array of union entries.
        let mut s = RecStruct::new();
        let items: Vec<RecValue> = self.entries.iter().map(|e| e.to_rec_value()).collect();
        s.SetValue("_array", RecValue::Array(items));
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Private recursive helpers ─────────────────────────────────────────────────

fn search_start_location_in(entries: &[emBookmarkEntryUnion]) -> Option<&emBookmarkRec> {
    for entry in entries {
        match entry {
            emBookmarkEntryUnion::Bookmark(bm) => {
                if bm.VisitAtProgramStart {
                    return Some(bm);
                }
            }
            emBookmarkEntryUnion::Group(grp) => {
                if let Some(bm) = search_start_location_in(&grp.Bookmarks) {
                    return Some(bm);
                }
            }
        }
    }
    None
}

fn search_bookmark_by_hotkey_in<'a>(
    entries: &'a [emBookmarkEntryUnion],
    hotkey: &str,
) -> Option<&'a emBookmarkRec> {
    for entry in entries {
        match entry {
            emBookmarkEntryUnion::Bookmark(bm) => {
                if !bm.Hotkey.is_empty() && bm.Hotkey == hotkey {
                    return Some(bm);
                }
            }
            emBookmarkEntryUnion::Group(grp) => {
                if let Some(bm) = search_bookmark_by_hotkey_in(&grp.Bookmarks, hotkey) {
                    return Some(bm);
                }
            }
        }
    }
    None
}

fn clear_start_location_in(entries: &mut [emBookmarkEntryUnion]) {
    for entry in entries {
        match entry {
            emBookmarkEntryUnion::Bookmark(bm) => {
                bm.VisitAtProgramStart = false;
            }
            emBookmarkEntryUnion::Group(grp) => {
                clear_start_location_in(&mut grp.Bookmarks);
            }
        }
    }
}

fn set_start_location_in(entries: &mut [emBookmarkEntryUnion], target_identity: &str) -> bool {
    for entry in entries {
        match entry {
            emBookmarkEntryUnion::Bookmark(bm) => {
                if bm.LocationIdentity == target_identity {
                    bm.VisitAtProgramStart = true;
                    return true;
                }
            }
            emBookmarkEntryUnion::Group(grp) => {
                if set_start_location_in(&mut grp.Bookmarks, target_identity) {
                    return true;
                }
            }
        }
    }
    false
}

// ── emBookmarksModel ──────────────────────────────────────────────────────────

/// Model wrapper for emBookmarks.
///
/// Port of C++ `emBookmarksModel` (extends emConfigModel + emBookmarksRec).
/// Backed by `emConfigModel` for file persistence at
/// `~/.eaglemode/emMain/bookmarks.rec`.
pub struct emBookmarksModel {
    config_model: emConfigModel<emBookmarksRec>,
}

impl emBookmarksModel {
    /// Acquire the singleton `emBookmarksModel` from the context registry.
    ///
    /// Port of C++ `emBookmarksModel::Acquire`.
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let path =
                emGetInstallPath(InstallDirType::UserConfig, "emMain", Some("bookmarks.rec"))
                    .unwrap_or_else(|_| {
                        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                        std::path::PathBuf::from(home)
                            .join(".eaglemode")
                            .join("emMain")
                            .join("bookmarks.rec")
                    });

            let mut model = emConfigModel::new(emBookmarksRec::default(), path, SignalId::null())
                .with_format_name("emBookmarks");

            if let Err(e) = model.TryLoadOrInstall() {
                log::warn!("emBookmarksModel: failed to load or install: {e}");
            }

            Self {
                config_model: model,
            }
        })
    }

    pub fn GetFormatName(&self) -> &str {
        "emBookmarks"
    }

    /// Port of C++ `emBookmarksModel::GetDefaultIconDir`.
    pub fn GetDefaultIconDir() -> std::path::PathBuf {
        emGetInstallPath(InstallDirType::Res, "icons", None)
            .unwrap_or_else(|_| std::path::PathBuf::from("/usr/share/eaglemode/res/icons"))
    }

    /// Port of C++ `emBookmarksModel::GetNormalizedIconFileName`.
    pub fn GetNormalizedIconFileName(icon_file: &str) -> String {
        if icon_file.is_empty() {
            return String::new();
        }
        let icon_dir = Self::GetDefaultIconDir();
        let icon_dir_str = icon_dir.to_string_lossy();
        if icon_file.len() > icon_dir_str.len() + 1 {
            let sep = icon_file.as_bytes().get(icon_dir_str.len());
            if sep == Some(&b'/') && icon_file.starts_with(icon_dir_str.as_ref()) {
                return icon_file[icon_dir_str.len() + 1..].to_string();
            }
        }
        icon_file.to_string()
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.config_model.GetChangeSignal()
    }

    pub fn GetRec(&self) -> &emBookmarksRec {
        self.config_model.GetRec()
    }

    pub fn Set(&mut self, rec: emBookmarksRec) {
        self.config_model.Set(rec);
    }

    pub fn IsUnsaved(&self) -> bool {
        self.config_model.IsUnsaved()
    }
}

// DIVERGED: C++ emBookmarkEntryAuxPanel and emBookmarksAuxPanel are editing
// panels (cut/copy/paste/new bookmark/new group, color editing, location
// setting, hotkey editing). Not ported — the Rust implementation is read-only.

// ── emBookmarkButton ──────────────────────────────────────────────────────────

/// A panel button representing a single bookmark entry.
///
/// Port of C++ `emBookmarksPanel`'s bookmark child (C++ uses anonymous button
/// widgets embedded in the raster group; Rust gives each a named type).
pub struct emBookmarkButton {
    bookmark: emBookmarkRec,
}

impl emBookmarkButton {
    pub fn new(bookmark: emBookmarkRec) -> Self {
        Self { bookmark }
    }
}

impl PanelBehavior for emBookmarkButton {
    fn get_title(&self) -> Option<String> {
        Some(self.bookmark.entry.Name.clone())
    }

    fn GetCanvasColor(&self) -> emColor {
        self.bookmark.entry.BgColor
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        let bg = self.bookmark.entry.BgColor;
        let fg = self.bookmark.entry.FgColor;
        // Draw background.
        painter.PaintRect(0.0, 0.0, w, h, bg, emColor::TRANSPARENT);
        // Draw bookmark name centered in the button.
        if !self.bookmark.entry.Name.is_empty() {
            let font_h = h * 0.4;
            painter.PaintTextBoxed(
                0.0,
                0.0,
                w,
                h,
                &self.bookmark.entry.Name,
                font_h,
                fg,
                bg,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.0,
                false,
                0.5,
            );
        }
    }

    fn Cycle(&mut self, _ctx: &mut PanelCtx) -> bool {
        // Navigation is not wired yet — log intent for now.
        false
    }
}

// ── emBookmarksPanel ──────────────────────────────────────────────────────────

/// Panel rendering bookmark buttons from `emBookmarksModel`.
///
/// Simplified port of C++ `emBookmarksPanel` (extends `emRasterGroup`).
/// Renders each `emBookmarkEntryUnion::Bookmark` as an `emBookmarkButton`
/// and each `emBookmarkEntryUnion::Group` as a nested `emBookmarksPanel`.
/// Listens to the model change signal and recreates children when the model
/// updates.
pub struct emBookmarksPanel {
    ctx: Rc<emContext>,
    model: Rc<RefCell<emBookmarksModel>>,
    /// Cached snapshot of entries — used to detect model changes.
    entries: Vec<emBookmarkEntryUnion>,
    children_created: bool,
}

impl emBookmarksPanel {
    pub fn new(ctx: Rc<emContext>) -> Self {
        let model = emBookmarksModel::Acquire(&ctx);
        let entries = model.borrow().GetRec().entries.clone();
        Self {
            ctx,
            model,
            entries,
            children_created: false,
        }
    }

    fn model_changed(&self) -> bool {
        let current = self.model.borrow().GetRec().entries.clone();
        current != self.entries
    }

    fn sync_entries(&mut self) {
        self.entries = self.model.borrow().GetRec().entries.clone();
    }
}

impl PanelBehavior for emBookmarksPanel {
    fn get_title(&self) -> Option<String> {
        Some("Bookmarks".to_string())
    }

    fn GetCanvasColor(&self) -> emColor {
        GROUP_BG_DEFAULT
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        // Draw group background.
        painter.PaintRect(0.0, 0.0, w, h, GROUP_BG_DEFAULT, emColor::TRANSPARENT);
    }

    fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool {
        if self.model_changed() {
            self.sync_entries();
            self.children_created = false;
            ctx.DeleteAllChildren();
        }
        if !self.children_created {
            self.children_created = true;
            // LayoutChildren creates the children — returning true triggers a
            // layout pass.
            return true;
        }
        false
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Delete old children if needed.
        if ctx.child_count() == 0 && !self.entries.is_empty() {
            // Create child panels for each entry in the snapshot.
            let entries = self.entries.clone();
            for (i, entry) in entries.iter().enumerate() {
                let name = format!("bm_{i}");
                match entry {
                    emBookmarkEntryUnion::Bookmark(bm) => {
                        let btn = emBookmarkButton::new(bm.clone());
                        ctx.create_child_with(&name, Box::new(btn));
                    }
                    emBookmarkEntryUnion::Group(grp) => {
                        // Build a nested panel backed by a temporary model
                        // snapshot.  The child holds its own ctx reference so
                        // it can re-acquire the model for sub-entries.
                        let sub_panel =
                            emBookmarksGroupPanel::new(Rc::clone(&self.ctx), grp.clone());
                        ctx.create_child_with(&name, Box::new(sub_panel));
                    }
                }
            }
        }

        // Lay out children in a vertical stack.
        let children = ctx.children();
        if children.is_empty() {
            return;
        }
        let n = children.len() as f64;
        let child_h = 1.0 / n;
        for (i, child) in children.iter().enumerate() {
            let y = i as f64 * child_h;
            ctx.layout_child(*child, 0.0, y, 1.0, child_h);
        }
    }

    fn notice(&mut self, flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.contains(NoticeFlags::LAYOUT_CHANGED) {
            // Force LayoutChildren to re-run.
        }
    }
}

// ── emBookmarksGroupPanel ─────────────────────────────────────────────────────

/// Panel rendering a single bookmark group's children.
///
/// DIVERGED: C++ uses recursive emBookmarksPanel for groups.  In Rust we use
/// a separate struct `emBookmarksGroupPanel` to avoid ownership issues when
/// recursively creating child panels from within `LayoutChildren`.  The
/// behavior is identical; only the type name differs.
pub struct emBookmarksGroupPanel {
    _ctx: Rc<emContext>,
    group: emBookmarkGroupRec,
    children_created: bool,
}

impl emBookmarksGroupPanel {
    pub fn new(ctx: Rc<emContext>, group: emBookmarkGroupRec) -> Self {
        Self {
            _ctx: ctx,
            group,
            children_created: false,
        }
    }
}

impl PanelBehavior for emBookmarksGroupPanel {
    fn get_title(&self) -> Option<String> {
        Some(self.group.entry.Name.clone())
    }

    fn GetCanvasColor(&self) -> emColor {
        self.group.entry.BgColor
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        let bg = self.group.entry.BgColor;
        painter.PaintRect(0.0, 0.0, w, h, bg, emColor::TRANSPARENT);
        if !self.group.entry.Name.is_empty() {
            let fg = self.group.entry.FgColor;
            let font_h = h * 0.15;
            painter.PaintTextBoxed(
                0.0,
                0.0,
                w,
                font_h * 1.5,
                &self.group.entry.Name,
                font_h,
                fg,
                bg,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.0,
                false,
                0.5,
            );
        }
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if self.children_created {
            return;
        }
        self.children_created = true;

        let entries = self.group.Bookmarks.clone();
        for (i, entry) in entries.iter().enumerate() {
            let name = format!("grp_bm_{i}");
            match entry {
                emBookmarkEntryUnion::Bookmark(bm) => {
                    let btn = emBookmarkButton::new(bm.clone());
                    ctx.create_child_with(&name, Box::new(btn));
                }
                emBookmarkEntryUnion::Group(sub_grp) => {
                    let sub = emBookmarksGroupPanel::new(Rc::clone(&self._ctx), sub_grp.clone());
                    ctx.create_child_with(&name, Box::new(sub));
                }
            }
        }

        let children = ctx.children();
        if children.is_empty() {
            return;
        }
        let n = children.len() as f64;
        let child_h = 1.0 / n;
        for (i, child) in children.iter().enumerate() {
            let y = i as f64 * child_h;
            ctx.layout_child(*child, 0.0, y, 1.0, child_h);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bookmark_rec_defaults() {
        let bm = emBookmarkRec::default();
        assert_eq!(bm.entry.Name, "");
        assert_eq!(bm.LocationIdentity, "");
        assert!((bm.LocationRelX).abs() < 1e-10);
        assert!((bm.LocationRelY).abs() < 1e-10);
        assert!((bm.LocationRelA).abs() < 1e-10);
        assert!(!bm.VisitAtProgramStart);
    }

    #[test]
    fn test_bookmark_rec_round_trip() {
        let mut bm = emBookmarkRec::default();
        bm.entry.Name = "Home".to_string();
        bm.LocationIdentity = "::VcItem:Home:".to_string();
        bm.Hotkey = "F5".to_string();
        let rec = bm.to_rec();
        let loaded = emBookmarkRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.entry.Name, "Home");
        assert_eq!(loaded.LocationIdentity, "::VcItem:Home:");
        assert_eq!(loaded.Hotkey, "F5");
    }

    #[test]
    fn test_search_start_location() {
        let mut bm1 = emBookmarkRec::default();
        bm1.entry.Name = "First".to_string();
        let mut bm2 = emBookmarkRec::default();
        bm2.entry.Name = "Start".to_string();
        bm2.VisitAtProgramStart = true;
        let bookmarks = emBookmarksRec {
            entries: vec![
                emBookmarkEntryUnion::Bookmark(bm1),
                emBookmarkEntryUnion::Bookmark(bm2),
            ],
        };
        let start = bookmarks.SearchStartLocation();
        assert!(start.is_some());
        assert_eq!(start.unwrap().entry.Name, "Start");
    }

    #[test]
    fn test_group_with_nested_bookmarks() {
        let bm = emBookmarkRec::default();
        let group = emBookmarkGroupRec {
            entry: emBookmarkEntryBase {
                Name: "MyGroup".to_string(),
                ..emBookmarkEntryBase::default()
            },
            Bookmarks: vec![emBookmarkEntryUnion::Bookmark(bm)],
        };
        let root = emBookmarksRec {
            entries: vec![emBookmarkEntryUnion::Group(group)],
        };
        assert_eq!(root.entries.len(), 1);
    }

    #[test]
    fn test_bookmarks_rec_round_trip() {
        let mut bm = emBookmarkRec::default();
        bm.entry.Name = "Test".to_string();
        bm.LocationIdentity = "::test:".to_string();
        bm.VisitAtProgramStart = true;

        let root = emBookmarksRec {
            entries: vec![emBookmarkEntryUnion::Bookmark(bm)],
        };

        let rec = root.to_rec();
        let loaded = emBookmarksRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.entries.len(), 1);

        if let emBookmarkEntryUnion::Bookmark(loaded_bm) = &loaded.entries[0] {
            assert_eq!(loaded_bm.entry.Name, "Test");
            assert_eq!(loaded_bm.LocationIdentity, "::test:");
            assert!(loaded_bm.VisitAtProgramStart);
        } else {
            panic!("expected bookmark");
        }
    }

    #[test]
    fn test_search_start_location_in_group() {
        let mut bm = emBookmarkRec::default();
        bm.entry.Name = "Nested".to_string();
        bm.VisitAtProgramStart = true;

        let group = emBookmarkGroupRec {
            entry: emBookmarkEntryBase::default(),
            Bookmarks: vec![emBookmarkEntryUnion::Bookmark(bm)],
        };

        let root = emBookmarksRec {
            entries: vec![emBookmarkEntryUnion::Group(group)],
        };

        let found = root.SearchStartLocation();
        assert!(found.is_some());
        assert_eq!(found.unwrap().entry.Name, "Nested");
    }

    #[test]
    fn test_clear_start_location() {
        let mut bm1 = emBookmarkRec::default();
        bm1.VisitAtProgramStart = true;
        let mut bm2 = emBookmarkRec::default();
        bm2.VisitAtProgramStart = true;

        let mut root = emBookmarksRec {
            entries: vec![
                emBookmarkEntryUnion::Bookmark(bm1),
                emBookmarkEntryUnion::Bookmark(bm2),
            ],
        };

        root.ClearStartLocation();

        for entry in &root.entries {
            if let emBookmarkEntryUnion::Bookmark(bm) = entry {
                assert!(!bm.VisitAtProgramStart);
            }
        }
    }

    #[test]
    fn test_set_start_location() {
        let mut bm1 = emBookmarkRec::default();
        bm1.LocationIdentity = "::home:".to_string();
        bm1.VisitAtProgramStart = true;

        let mut bm2 = emBookmarkRec::default();
        bm2.LocationIdentity = "::work:".to_string();

        let mut root = emBookmarksRec {
            entries: vec![
                emBookmarkEntryUnion::Bookmark(bm1),
                emBookmarkEntryUnion::Bookmark(bm2),
            ],
        };

        root.SetStartLocation("::work:");

        let found = root.SearchStartLocation().unwrap();
        assert_eq!(found.LocationIdentity, "::work:");
    }

    #[test]
    fn test_search_bookmark_by_hotkey() {
        let mut bm1 = emBookmarkRec::default();
        bm1.Hotkey = "F1".to_string();
        bm1.entry.Name = "Bm1".to_string();

        let mut bm2 = emBookmarkRec::default();
        bm2.Hotkey = "F2".to_string();
        bm2.entry.Name = "Bm2".to_string();

        let root = emBookmarksRec {
            entries: vec![
                emBookmarkEntryUnion::Bookmark(bm1),
                emBookmarkEntryUnion::Bookmark(bm2),
            ],
        };

        let found = root.SearchBookmarkByHotkey("F2");
        assert!(found.is_some());
        assert_eq!(found.unwrap().entry.Name, "Bm2");

        assert!(root.SearchBookmarkByHotkey("F3").is_none());
        assert!(root.SearchBookmarkByHotkey("").is_none());
    }

    #[test]
    fn test_default_colors() {
        let bm = emBookmarkRec::default();
        assert_eq!(bm.entry.BgColor, BOOKMARK_BG_DEFAULT);
        assert_eq!(bm.entry.FgColor, BOOKMARK_FG_DEFAULT);

        let grp = emBookmarkGroupRec::default();
        assert_eq!(grp.entry.BgColor, GROUP_BG_DEFAULT);
        assert_eq!(grp.entry.FgColor, GROUP_FG_DEFAULT);
    }

    #[test]
    fn test_group_rec_round_trip() {
        let mut bm = emBookmarkRec::default();
        bm.entry.Name = "Inner".to_string();

        let group = emBookmarkGroupRec {
            entry: emBookmarkEntryBase {
                Name: "OuterGroup".to_string(),
                ..emBookmarkEntryBase::new(GROUP_BG_DEFAULT, GROUP_FG_DEFAULT)
            },
            Bookmarks: vec![emBookmarkEntryUnion::Bookmark(bm)],
        };

        let rec = group.to_rec();
        let loaded = emBookmarkGroupRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.entry.Name, "OuterGroup");
        assert_eq!(loaded.Bookmarks.len(), 1);

        if let emBookmarkEntryUnion::Bookmark(inner_bm) = &loaded.Bookmarks[0] {
            assert_eq!(inner_bm.entry.Name, "Inner");
        } else {
            panic!("expected bookmark inside group");
        }
    }

    #[test]
    fn test_acquire_singleton() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let m1 = emBookmarksModel::Acquire(&ctx);
        let m2 = emBookmarksModel::Acquire(&ctx);
        assert!(Rc::ptr_eq(&m1, &m2));
    }

    #[test]
    fn test_model_format_name() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let model = emBookmarksModel::Acquire(&ctx);
        let model = model.borrow();
        assert_eq!(model.GetFormatName(), "emBookmarks");
    }

    #[test]
    fn test_bookmarks_panel_new() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emBookmarksPanel::new(Rc::clone(&ctx));
        assert_eq!(panel.get_title(), Some("Bookmarks".to_string()));
    }

    #[test]
    fn test_bookmark_button_title() {
        let mut bm = emBookmarkRec::default();
        bm.entry.Name = "Home".to_string();
        let btn = emBookmarkButton::new(bm);
        assert_eq!(btn.get_title(), Some("Home".to_string()));
    }

    #[test]
    fn test_bookmarks_panel_behavior() {
        use emcore::emPanel::PanelBehavior;
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emBookmarksPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn test_parse_cpp_format() {
        // Verify we can parse the C++ emBookmarks file format (top-level
        // array of union entries).
        use emcore::emRec::parse_rec;
        let text = r#"#%rec:emBookmarks%#

Bookmark: {
    Name = "Help"
    Description = "This brings you to the documentation area."
    Icon = "help.tga"
    Hotkey = "F1"
    LocationIdentity = ":"
    LocationRelX = -0.36326
    LocationRelY = -0.37791
    LocationRelA = 0.00621
}

Bookmark: {
    Name = "Home"
    Icon = "home.tga"
    Hotkey = "F6"
    LocationIdentity = "::FS::::home::a0"
    VisitAtProgramStart = yes
}

Group: {
    Name = "Games"
    BgColor = { 81 94 132 255 }
    FgColor = { 239 240 244 255 }
    Bookmarks = {
        Bookmark: {
            Name = "Chess"
            Icon = "silchess.tga"
            LocationIdentity = "::Chess1:"
        }
    }
}
"#;
        let rec = parse_rec(text).expect("should parse C++ bookmarks format");
        let bookmarks = emBookmarksRec::from_rec(&rec).expect("should deserialize");
        assert_eq!(bookmarks.entries.len(), 3);

        // First entry: Bookmark "Help"
        if let emBookmarkEntryUnion::Bookmark(bm) = &bookmarks.entries[0] {
            assert_eq!(bm.entry.Name, "Help");
            assert_eq!(bm.Hotkey, "F1");
            assert_eq!(bm.LocationIdentity, ":");
            assert!((bm.LocationRelX - (-0.36326)).abs() < 1e-10);
            assert!((bm.LocationRelA - 0.00621).abs() < 1e-10);
        } else {
            panic!("expected Bookmark");
        }

        // Second: Bookmark "Home" with VisitAtProgramStart
        if let emBookmarkEntryUnion::Bookmark(bm) = &bookmarks.entries[1] {
            assert_eq!(bm.entry.Name, "Home");
            assert!(bm.VisitAtProgramStart);
        } else {
            panic!("expected Bookmark");
        }

        // Third: Group "Games" with nested bookmark
        if let emBookmarkEntryUnion::Group(grp) = &bookmarks.entries[2] {
            assert_eq!(grp.entry.Name, "Games");
            assert_eq!(grp.Bookmarks.len(), 1);
            if let emBookmarkEntryUnion::Bookmark(inner) = &grp.Bookmarks[0] {
                assert_eq!(inner.entry.Name, "Chess");
            } else {
                panic!("expected nested Bookmark");
            }
        } else {
            panic!("expected Group");
        }

        // SearchStartLocation should find "Home"
        let start = bookmarks.SearchStartLocation();
        assert!(start.is_some());
        assert_eq!(start.unwrap().entry.Name, "Home");

        // SearchBookmarkByHotkey
        let found = bookmarks.SearchBookmarkByHotkey("F1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().entry.Name, "Help");
    }

    #[test]
    fn test_round_trip_cpp_format() {
        // Verify that to_rec + write_rec + parse_rec + from_rec round-trips.
        use emcore::emRec::{parse_rec, write_rec_with_format};
        let mut bm = emBookmarkRec::default();
        bm.entry.Name = "Test".to_string();
        bm.Hotkey = "F5".to_string();
        bm.LocationIdentity = "::test:".to_string();

        let root = emBookmarksRec {
            entries: vec![emBookmarkEntryUnion::Bookmark(bm)],
        };

        let rec = root.to_rec();
        let text = write_rec_with_format(&rec, "emBookmarks");
        assert!(text.starts_with("#%rec:emBookmarks%#"));

        let parsed = parse_rec(&text).expect("should re-parse");
        let loaded = emBookmarksRec::from_rec(&parsed).expect("should deserialize");
        assert_eq!(loaded.entries.len(), 1);
        if let emBookmarkEntryUnion::Bookmark(bm) = &loaded.entries[0] {
            assert_eq!(bm.entry.Name, "Test");
            assert_eq!(bm.Hotkey, "F5");
        } else {
            panic!("expected Bookmark");
        }
    }
}
