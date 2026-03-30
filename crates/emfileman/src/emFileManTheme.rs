use std::cell::{Ref, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emConfigModel::emConfigModel;
use emcore::emContext::emContext;
use emcore::emImage::emImage;
use emcore::emImageFile::load_image_from_file;
use emcore::emInstallInfo::{emGetInstallPath, InstallDirType, InstallInfoError};
use emcore::emRec::{RecError, RecStruct, RecValue};
use emcore::emRecRecord::Record;
use emcore::emRecRecTypes::{emAlignmentRec, emColorRec};
use emcore::emSignal::SignalId;
use emcore::emTiling::Alignment;

pub const THEME_FILE_ENDING: &str = ".emFileManTheme";

pub fn GetThemesDirPath() -> Result<PathBuf, InstallInfoError> {
    emGetInstallPath(InstallDirType::Res, "emFileMan", Some("themes"))
}

/// Helper: read an alignment field from a RecStruct via get_ident.
fn read_alignment(rec: &RecStruct, name: &str) -> Alignment {
    match rec.get_ident(name) {
        Some(s) => {
            let val = RecValue::Ident(s.to_string());
            emAlignmentRec::FromRecValue(&val).unwrap_or(Alignment::Center)
        }
        None => Alignment::Center,
    }
}

/// Helper: read a color (packed u32) from a RecStruct sub-struct.
fn read_color(rec: &RecStruct, name: &str) -> Result<u32, RecError> {
    let sub = rec
        .get_struct(name)
        .ok_or_else(|| RecError::MissingField(name.into()))?;
    let c = emColorRec::FromRecStruct(sub, true)?;
    Ok(c.GetPacked())
}

/// Helper: write a color (packed u32) into a RecStruct as a sub-struct.
fn write_color(rec: &mut RecStruct, name: &str, packed: u32) {
    let r = ((packed >> 24) & 0xFF) as u8;
    let g = ((packed >> 16) & 0xFF) as u8;
    let b = ((packed >> 8) & 0xFF) as u8;
    let a = (packed & 0xFF) as u8;
    let c = emColor::rgba(r, g, b, a);
    let sub = emColorRec::ToRecStruct(c, true);
    rec.SetValue(name, RecValue::Struct(sub));
}

/// Helper: write an alignment field into a RecStruct.
fn write_alignment(rec: &mut RecStruct, name: &str, alignment: Alignment) {
    let val = emAlignmentRec::ToRecValue(alignment);
    rec.SetValue(name, val);
}


#[derive(Clone, Debug, PartialEq)]
pub struct emFileManThemeData {
    // Strings
    pub DisplayName: String,
    pub DisplayIcon: String,

    // Colors (packed RGBA u32)
    pub BackgroundColor: u32,
    pub SourceSelectionColor: u32,
    pub TargetSelectionColor: u32,
    pub NormalNameColor: u32,
    pub ExeNameColor: u32,
    pub DirNameColor: u32,
    pub FifoNameColor: u32,
    pub BlkNameColor: u32,
    pub ChrNameColor: u32,
    pub SockNameColor: u32,
    pub OtherNameColor: u32,
    pub PathColor: u32,
    pub SymLinkColor: u32,
    pub LabelColor: u32,
    pub InfoColor: u32,
    pub FileContentColor: u32,
    pub DirContentColor: u32,

    // f64 dimension fields (via DoubleFields)
    pub Height: f64,
    pub BackgroundX: f64,
    pub BackgroundY: f64,
    pub BackgroundW: f64,
    pub BackgroundH: f64,
    pub BackgroundRX: f64,
    pub BackgroundRY: f64,
    pub OuterBorderX: f64,
    pub OuterBorderY: f64,
    pub OuterBorderW: f64,
    pub OuterBorderH: f64,
    pub OuterBorderL: f64,
    pub OuterBorderT: f64,
    pub OuterBorderR: f64,
    pub OuterBorderB: f64,
    pub NameX: f64,
    pub NameY: f64,
    pub NameW: f64,
    pub NameH: f64,
    pub PathX: f64,
    pub PathY: f64,
    pub PathW: f64,
    pub PathH: f64,
    pub InfoX: f64,
    pub InfoY: f64,
    pub InfoW: f64,
    pub InfoH: f64,
    pub FileInnerBorderX: f64,
    pub FileInnerBorderY: f64,
    pub FileInnerBorderW: f64,
    pub FileInnerBorderH: f64,
    pub FileInnerBorderL: f64,
    pub FileInnerBorderT: f64,
    pub FileInnerBorderR: f64,
    pub FileInnerBorderB: f64,
    pub FileContentX: f64,
    pub FileContentY: f64,
    pub FileContentW: f64,
    pub FileContentH: f64,
    pub DirInnerBorderX: f64,
    pub DirInnerBorderY: f64,
    pub DirInnerBorderW: f64,
    pub DirInnerBorderH: f64,
    pub DirInnerBorderL: f64,
    pub DirInnerBorderT: f64,
    pub DirInnerBorderR: f64,
    pub DirInnerBorderB: f64,
    pub DirContentX: f64,
    pub DirContentY: f64,
    pub DirContentW: f64,
    pub DirContentH: f64,
    pub AltX: f64,
    pub AltY: f64,
    pub AltW: f64,
    pub AltH: f64,
    pub AltLabelX: f64,
    pub AltLabelY: f64,
    pub AltLabelW: f64,
    pub AltLabelH: f64,
    pub AltPathX: f64,
    pub AltPathY: f64,
    pub AltPathW: f64,
    pub AltPathH: f64,
    pub AltAltX: f64,
    pub AltAltY: f64,
    pub AltAltW: f64,
    pub AltAltH: f64,
    pub AltInnerBorderX: f64,
    pub AltInnerBorderY: f64,
    pub AltInnerBorderW: f64,
    pub AltInnerBorderH: f64,
    pub AltInnerBorderL: f64,
    pub AltInnerBorderT: f64,
    pub AltInnerBorderR: f64,
    pub AltInnerBorderB: f64,
    pub AltContentX: f64,
    pub AltContentY: f64,
    pub AltContentW: f64,
    pub AltContentH: f64,
    pub MinContentVW: f64,
    pub MinAltVW: f64,
    pub DirPaddingL: f64,
    pub DirPaddingT: f64,
    pub DirPaddingR: f64,
    pub DirPaddingB: f64,
    pub LnkPaddingL: f64,
    pub LnkPaddingT: f64,
    pub LnkPaddingR: f64,
    pub LnkPaddingB: f64,

    // Alignment fields
    pub NameAlignment: Alignment,
    pub PathAlignment: Alignment,
    pub InfoAlignment: Alignment,
    pub AltLabelAlignment: Alignment,
    pub AltPathAlignment: Alignment,

    // Image border fields — path string + 4 i32 values each
    pub OuterBorderImg: String,
    pub OuterBorderImgL: i32,
    pub OuterBorderImgT: i32,
    pub OuterBorderImgR: i32,
    pub OuterBorderImgB: i32,

    pub FileInnerBorderImg: String,
    pub FileInnerBorderImgL: i32,
    pub FileInnerBorderImgT: i32,
    pub FileInnerBorderImgR: i32,
    pub FileInnerBorderImgB: i32,

    pub DirInnerBorderImg: String,
    pub DirInnerBorderImgL: i32,
    pub DirInnerBorderImgT: i32,
    pub DirInnerBorderImgR: i32,
    pub DirInnerBorderImgB: i32,

    pub AltInnerBorderImg: String,
    pub AltInnerBorderImgL: i32,
    pub AltInnerBorderImgT: i32,
    pub AltInnerBorderImgR: i32,
    pub AltInnerBorderImgB: i32,
}

impl Default for emFileManThemeData {
    fn default() -> Self {
        Self {
            DisplayName: String::new(),
            DisplayIcon: String::new(),

            BackgroundColor: 0x000000FF,
            SourceSelectionColor: 0x000000FF,
            TargetSelectionColor: 0x000000FF,
            NormalNameColor: 0x000000FF,
            ExeNameColor: 0x000000FF,
            DirNameColor: 0x000000FF,
            FifoNameColor: 0x000000FF,
            BlkNameColor: 0x000000FF,
            ChrNameColor: 0x000000FF,
            SockNameColor: 0x000000FF,
            OtherNameColor: 0x000000FF,
            PathColor: 0x000000FF,
            SymLinkColor: 0x000000FF,
            LabelColor: 0x000000FF,
            InfoColor: 0x000000FF,
            FileContentColor: 0x000000FF,
            DirContentColor: 0x000000FF,

            Height: 0.0,
            BackgroundX: 0.0,
            BackgroundY: 0.0,
            BackgroundW: 0.0,
            BackgroundH: 0.0,
            BackgroundRX: 0.0,
            BackgroundRY: 0.0,
            OuterBorderX: 0.0,
            OuterBorderY: 0.0,
            OuterBorderW: 0.0,
            OuterBorderH: 0.0,
            OuterBorderL: 0.0,
            OuterBorderT: 0.0,
            OuterBorderR: 0.0,
            OuterBorderB: 0.0,
            NameX: 0.0,
            NameY: 0.0,
            NameW: 0.0,
            NameH: 0.0,
            PathX: 0.0,
            PathY: 0.0,
            PathW: 0.0,
            PathH: 0.0,
            InfoX: 0.0,
            InfoY: 0.0,
            InfoW: 0.0,
            InfoH: 0.0,
            FileInnerBorderX: 0.0,
            FileInnerBorderY: 0.0,
            FileInnerBorderW: 0.0,
            FileInnerBorderH: 0.0,
            FileInnerBorderL: 0.0,
            FileInnerBorderT: 0.0,
            FileInnerBorderR: 0.0,
            FileInnerBorderB: 0.0,
            FileContentX: 0.0,
            FileContentY: 0.0,
            FileContentW: 0.0,
            FileContentH: 0.0,
            DirInnerBorderX: 0.0,
            DirInnerBorderY: 0.0,
            DirInnerBorderW: 0.0,
            DirInnerBorderH: 0.0,
            DirInnerBorderL: 0.0,
            DirInnerBorderT: 0.0,
            DirInnerBorderR: 0.0,
            DirInnerBorderB: 0.0,
            DirContentX: 0.0,
            DirContentY: 0.0,
            DirContentW: 0.0,
            DirContentH: 0.0,
            AltX: 0.0,
            AltY: 0.0,
            AltW: 0.0,
            AltH: 0.0,
            AltLabelX: 0.0,
            AltLabelY: 0.0,
            AltLabelW: 0.0,
            AltLabelH: 0.0,
            AltPathX: 0.0,
            AltPathY: 0.0,
            AltPathW: 0.0,
            AltPathH: 0.0,
            AltAltX: 0.0,
            AltAltY: 0.0,
            AltAltW: 0.0,
            AltAltH: 0.0,
            AltInnerBorderX: 0.0,
            AltInnerBorderY: 0.0,
            AltInnerBorderW: 0.0,
            AltInnerBorderH: 0.0,
            AltInnerBorderL: 0.0,
            AltInnerBorderT: 0.0,
            AltInnerBorderR: 0.0,
            AltInnerBorderB: 0.0,
            AltContentX: 0.0,
            AltContentY: 0.0,
            AltContentW: 0.0,
            AltContentH: 0.0,
            MinContentVW: 0.0,
            MinAltVW: 0.0,
            DirPaddingL: 0.0,
            DirPaddingT: 0.0,
            DirPaddingR: 0.0,
            DirPaddingB: 0.0,
            LnkPaddingL: 0.0,
            LnkPaddingT: 0.0,
            LnkPaddingR: 0.0,
            LnkPaddingB: 0.0,

            NameAlignment: Alignment::Center,
            PathAlignment: Alignment::Center,
            InfoAlignment: Alignment::Center,
            AltLabelAlignment: Alignment::Center,
            AltPathAlignment: Alignment::Center,

            OuterBorderImg: String::new(),
            OuterBorderImgL: 0,
            OuterBorderImgT: 0,
            OuterBorderImgR: 0,
            OuterBorderImgB: 0,

            FileInnerBorderImg: String::new(),
            FileInnerBorderImgL: 0,
            FileInnerBorderImgT: 0,
            FileInnerBorderImgR: 0,
            FileInnerBorderImgB: 0,

            DirInnerBorderImg: String::new(),
            DirInnerBorderImgL: 0,
            DirInnerBorderImgT: 0,
            DirInnerBorderImgR: 0,
            DirInnerBorderImgB: 0,

            AltInnerBorderImg: String::new(),
            AltInnerBorderImgL: 0,
            AltInnerBorderImgT: 0,
            AltInnerBorderImgR: 0,
            AltInnerBorderImgB: 0,
        }
    }
}

/// Macro to reduce repetition for reading/writing f64 fields.
macro_rules! rec_doubles {
    ($rec:expr, $self:expr, read, $($field:ident),* $(,)?) => {
        $($self.$field = $rec.get_double(stringify!($field)).unwrap_or(0.0);)*
    };
    ($rec:expr, $self:expr, write, $($field:ident),* $(,)?) => {
        $($rec.set_double(stringify!($field), $self.$field);)*
    };
}

/// Macro to reduce repetition for reading/writing color fields.
macro_rules! rec_colors {
    ($rec:expr, $self:expr, read, $($field:ident),* $(,)?) => {
        $($self.$field = read_color($rec, stringify!($field)).unwrap_or(0x000000FF);)*
    };
    ($rec:expr, $self:expr, write, $($field:ident),* $(,)?) => {
        $(write_color(&mut $rec, stringify!($field), $self.$field);)*
    };
}

impl Record for emFileManThemeData {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let mut t = Self::default();

        // Strings
        t.DisplayName = rec.get_str("DisplayName").unwrap_or("").to_string();
        t.DisplayIcon = rec.get_str("DisplayIcon").unwrap_or("").to_string();

        // Colors
        rec_colors!(rec, t, read,
            BackgroundColor, SourceSelectionColor, TargetSelectionColor,
            NormalNameColor, ExeNameColor, DirNameColor, FifoNameColor,
            BlkNameColor, ChrNameColor, SockNameColor, OtherNameColor,
            PathColor, SymLinkColor, LabelColor, InfoColor,
            FileContentColor, DirContentColor,
        );

        // f64 dimensions
        rec_doubles!(rec, t, read,
            Height,
            BackgroundX, BackgroundY, BackgroundW, BackgroundH, BackgroundRX, BackgroundRY,
            OuterBorderX, OuterBorderY, OuterBorderW, OuterBorderH,
            OuterBorderL, OuterBorderT, OuterBorderR, OuterBorderB,
            NameX, NameY, NameW, NameH,
            PathX, PathY, PathW, PathH,
            InfoX, InfoY, InfoW, InfoH,
            FileInnerBorderX, FileInnerBorderY, FileInnerBorderW, FileInnerBorderH,
            FileInnerBorderL, FileInnerBorderT, FileInnerBorderR, FileInnerBorderB,
            FileContentX, FileContentY, FileContentW, FileContentH,
            DirInnerBorderX, DirInnerBorderY, DirInnerBorderW, DirInnerBorderH,
            DirInnerBorderL, DirInnerBorderT, DirInnerBorderR, DirInnerBorderB,
            DirContentX, DirContentY, DirContentW, DirContentH,
            AltX, AltY, AltW, AltH,
            AltLabelX, AltLabelY, AltLabelW, AltLabelH,
            AltPathX, AltPathY, AltPathW, AltPathH,
            AltAltX, AltAltY, AltAltW, AltAltH,
            AltInnerBorderX, AltInnerBorderY, AltInnerBorderW, AltInnerBorderH,
            AltInnerBorderL, AltInnerBorderT, AltInnerBorderR, AltInnerBorderB,
            AltContentX, AltContentY, AltContentW, AltContentH,
            MinContentVW, MinAltVW,
            DirPaddingL, DirPaddingT, DirPaddingR, DirPaddingB,
            LnkPaddingL, LnkPaddingT, LnkPaddingR, LnkPaddingB,
        );

        // Alignments
        t.NameAlignment = read_alignment(rec, "NameAlignment");
        t.PathAlignment = read_alignment(rec, "PathAlignment");
        t.InfoAlignment = read_alignment(rec, "InfoAlignment");
        t.AltLabelAlignment = read_alignment(rec, "AltLabelAlignment");
        t.AltPathAlignment = read_alignment(rec, "AltPathAlignment");

        // Image border groups
        t.OuterBorderImg = rec.get_str("OuterBorderImg").unwrap_or("").to_string();
        t.OuterBorderImgL = rec.get_int("OuterBorderImgL").unwrap_or(0);
        t.OuterBorderImgT = rec.get_int("OuterBorderImgT").unwrap_or(0);
        t.OuterBorderImgR = rec.get_int("OuterBorderImgR").unwrap_or(0);
        t.OuterBorderImgB = rec.get_int("OuterBorderImgB").unwrap_or(0);

        t.FileInnerBorderImg = rec.get_str("FileInnerBorderImg").unwrap_or("").to_string();
        t.FileInnerBorderImgL = rec.get_int("FileInnerBorderImgL").unwrap_or(0);
        t.FileInnerBorderImgT = rec.get_int("FileInnerBorderImgT").unwrap_or(0);
        t.FileInnerBorderImgR = rec.get_int("FileInnerBorderImgR").unwrap_or(0);
        t.FileInnerBorderImgB = rec.get_int("FileInnerBorderImgB").unwrap_or(0);

        t.DirInnerBorderImg = rec.get_str("DirInnerBorderImg").unwrap_or("").to_string();
        t.DirInnerBorderImgL = rec.get_int("DirInnerBorderImgL").unwrap_or(0);
        t.DirInnerBorderImgT = rec.get_int("DirInnerBorderImgT").unwrap_or(0);
        t.DirInnerBorderImgR = rec.get_int("DirInnerBorderImgR").unwrap_or(0);
        t.DirInnerBorderImgB = rec.get_int("DirInnerBorderImgB").unwrap_or(0);

        t.AltInnerBorderImg = rec.get_str("AltInnerBorderImg").unwrap_or("").to_string();
        t.AltInnerBorderImgL = rec.get_int("AltInnerBorderImgL").unwrap_or(0);
        t.AltInnerBorderImgT = rec.get_int("AltInnerBorderImgT").unwrap_or(0);
        t.AltInnerBorderImgR = rec.get_int("AltInnerBorderImgR").unwrap_or(0);
        t.AltInnerBorderImgB = rec.get_int("AltInnerBorderImgB").unwrap_or(0);

        Ok(t)
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();

        // Strings
        s.set_str("DisplayName", &self.DisplayName);
        s.set_str("DisplayIcon", &self.DisplayIcon);

        // Colors
        rec_colors!(s, self, write,
            BackgroundColor, SourceSelectionColor, TargetSelectionColor,
            NormalNameColor, ExeNameColor, DirNameColor, FifoNameColor,
            BlkNameColor, ChrNameColor, SockNameColor, OtherNameColor,
            PathColor, SymLinkColor, LabelColor, InfoColor,
            FileContentColor, DirContentColor,
        );

        // f64 dimensions
        rec_doubles!(s, self, write,
            Height,
            BackgroundX, BackgroundY, BackgroundW, BackgroundH, BackgroundRX, BackgroundRY,
            OuterBorderX, OuterBorderY, OuterBorderW, OuterBorderH,
            OuterBorderL, OuterBorderT, OuterBorderR, OuterBorderB,
            NameX, NameY, NameW, NameH,
            PathX, PathY, PathW, PathH,
            InfoX, InfoY, InfoW, InfoH,
            FileInnerBorderX, FileInnerBorderY, FileInnerBorderW, FileInnerBorderH,
            FileInnerBorderL, FileInnerBorderT, FileInnerBorderR, FileInnerBorderB,
            FileContentX, FileContentY, FileContentW, FileContentH,
            DirInnerBorderX, DirInnerBorderY, DirInnerBorderW, DirInnerBorderH,
            DirInnerBorderL, DirInnerBorderT, DirInnerBorderR, DirInnerBorderB,
            DirContentX, DirContentY, DirContentW, DirContentH,
            AltX, AltY, AltW, AltH,
            AltLabelX, AltLabelY, AltLabelW, AltLabelH,
            AltPathX, AltPathY, AltPathW, AltPathH,
            AltAltX, AltAltY, AltAltW, AltAltH,
            AltInnerBorderX, AltInnerBorderY, AltInnerBorderW, AltInnerBorderH,
            AltInnerBorderL, AltInnerBorderT, AltInnerBorderR, AltInnerBorderB,
            AltContentX, AltContentY, AltContentW, AltContentH,
            MinContentVW, MinAltVW,
            DirPaddingL, DirPaddingT, DirPaddingR, DirPaddingB,
            LnkPaddingL, LnkPaddingT, LnkPaddingR, LnkPaddingB,
        );

        // Alignments
        write_alignment(&mut s, "NameAlignment", self.NameAlignment);
        write_alignment(&mut s, "PathAlignment", self.PathAlignment);
        write_alignment(&mut s, "InfoAlignment", self.InfoAlignment);
        write_alignment(&mut s, "AltLabelAlignment", self.AltLabelAlignment);
        write_alignment(&mut s, "AltPathAlignment", self.AltPathAlignment);

        // Image border groups
        s.set_str("OuterBorderImg", &self.OuterBorderImg);
        s.set_int("OuterBorderImgL", self.OuterBorderImgL);
        s.set_int("OuterBorderImgT", self.OuterBorderImgT);
        s.set_int("OuterBorderImgR", self.OuterBorderImgR);
        s.set_int("OuterBorderImgB", self.OuterBorderImgB);

        s.set_str("FileInnerBorderImg", &self.FileInnerBorderImg);
        s.set_int("FileInnerBorderImgL", self.FileInnerBorderImgL);
        s.set_int("FileInnerBorderImgT", self.FileInnerBorderImgT);
        s.set_int("FileInnerBorderImgR", self.FileInnerBorderImgR);
        s.set_int("FileInnerBorderImgB", self.FileInnerBorderImgB);

        s.set_str("DirInnerBorderImg", &self.DirInnerBorderImg);
        s.set_int("DirInnerBorderImgL", self.DirInnerBorderImgL);
        s.set_int("DirInnerBorderImgT", self.DirInnerBorderImgT);
        s.set_int("DirInnerBorderImgR", self.DirInnerBorderImgR);
        s.set_int("DirInnerBorderImgB", self.DirInnerBorderImgB);

        s.set_str("AltInnerBorderImg", &self.AltInnerBorderImg);
        s.set_int("AltInnerBorderImgL", self.AltInnerBorderImgL);
        s.set_int("AltInnerBorderImgT", self.AltInnerBorderImgT);
        s.set_int("AltInnerBorderImgR", self.AltInnerBorderImgR);
        s.set_int("AltInnerBorderImgB", self.AltInnerBorderImgB);

        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

/// Lazy-loading image record. Stores a path and caches the loaded image.
/// DIVERGED: C++ ImageFileRec extends emStringRec + emRecListener with async
/// emImageFileModel loading. This loads synchronously via load_image_from_file.
pub struct ImageFileRec {
    path: String,
    theme_dir: PathBuf,
    cached: RefCell<Option<emImage>>,
}

impl ImageFileRec {
    pub fn new(path: String, theme_dir: PathBuf) -> Self {
        Self {
            path,
            theme_dir,
            cached: RefCell::new(None),
        }
    }

    pub fn GetImage(&self) -> Ref<'_, emImage> {
        // Check if already cached without taking a mutable borrow.
        let needs_load = self.cached.borrow().is_none();
        if needs_load {
            let image = if self.path.is_empty() {
                None
            } else {
                load_image_from_file(&self.theme_dir.join(&self.path))
            };
            *self.cached.borrow_mut() = Some(image.unwrap_or_else(|| emImage::new(1, 1, 4)));
        }
        Ref::map(self.cached.borrow(), |opt| opt.as_ref().expect("just set"))
    }

    pub fn GetPath(&self) -> &str {
        &self.path
    }
}

pub struct emFileManTheme {
    config_model: emConfigModel<emFileManThemeData>,
    outer_border_img: ImageFileRec,
    file_inner_border_img: ImageFileRec,
    dir_inner_border_img: ImageFileRec,
    alt_inner_border_img: ImageFileRec,
}

impl emFileManTheme {
    pub fn Acquire(ctx: &Rc<emContext>, name: &str) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>(name, || {
            let signal_id = SignalId::default();
            let theme_dir = GetThemesDirPath().unwrap_or_default();
            let path = theme_dir.join(format!("{}{}", name, THEME_FILE_ENDING));
            let data = emFileManThemeData::default();
            let outer = ImageFileRec::new(data.OuterBorderImg.clone(), theme_dir.clone());
            let file_inner =
                ImageFileRec::new(data.FileInnerBorderImg.clone(), theme_dir.clone());
            let dir_inner =
                ImageFileRec::new(data.DirInnerBorderImg.clone(), theme_dir.clone());
            let alt_inner = ImageFileRec::new(data.AltInnerBorderImg.clone(), theme_dir);
            Self {
                config_model: emConfigModel::new(data, path, signal_id),
                outer_border_img: outer,
                file_inner_border_img: file_inner,
                dir_inner_border_img: dir_inner,
                alt_inner_border_img: alt_inner,
            }
        })
    }

    pub fn GetFormatName(&self) -> &str {
        "emFileManTheme"
    }

    pub fn GetRec(&self) -> &emFileManThemeData {
        self.config_model.GetRec()
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.config_model.GetChangeSignal()
    }

    pub fn GetOuterBorderImage(&self) -> Ref<'_, emImage> {
        self.outer_border_img.GetImage()
    }

    pub fn GetFileInnerBorderImage(&self) -> Ref<'_, emImage> {
        self.file_inner_border_img.GetImage()
    }

    pub fn GetDirInnerBorderImage(&self) -> Ref<'_, emImage> {
        self.dir_inner_border_img.GetImage()
    }

    pub fn GetAltInnerBorderImage(&self) -> Ref<'_, emImage> {
        self.alt_inner_border_img.GetImage()
    }

    pub(crate) fn _refresh_image_recs(&mut self) {
        let theme_dir = self
            .config_model
            .GetInstallPath()
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
            .to_path_buf();
        let data = self.config_model.GetRec();
        self.outer_border_img = ImageFileRec::new(data.OuterBorderImg.clone(), theme_dir.clone());
        self.file_inner_border_img =
            ImageFileRec::new(data.FileInnerBorderImg.clone(), theme_dir.clone());
        self.dir_inner_border_img =
            ImageFileRec::new(data.DirInnerBorderImg.clone(), theme_dir.clone());
        self.alt_inner_border_img =
            ImageFileRec::new(data.AltInnerBorderImg.clone(), theme_dir);
    }

    pub(crate) fn _config_model_mut(&mut self) -> &mut emConfigModel<emFileManThemeData> {
        &mut self.config_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRecRecord::Record;

    #[test]
    fn default_has_reasonable_height() {
        let t = emFileManThemeData::default();
        assert!((t.Height - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn record_round_trip_preserves_colors() {
        let mut t = emFileManThemeData::default();
        t.BackgroundColor = 0xAABBCCFF;
        t.SourceSelectionColor = 0x11223344;
        t.Height = 1.5;
        t.DisplayName = "TestTheme".to_string();

        let rec = t.to_rec();
        let t2 = emFileManThemeData::from_rec(&rec).unwrap();

        assert_eq!(t2.BackgroundColor, 0xAABBCCFF);
        assert_eq!(t2.SourceSelectionColor, 0x11223344);
        assert!((t2.Height - 1.5).abs() < f64::EPSILON);
        assert_eq!(t2.DisplayName, "TestTheme");
    }

    #[test]
    fn all_dimension_fields_exist() {
        let t = emFileManThemeData::default();
        let _ = t.BackgroundX;
        let _ = t.BackgroundY;
        let _ = t.BackgroundW;
        let _ = t.BackgroundH;
        let _ = t.BackgroundRX;
        let _ = t.BackgroundRY;
        let _ = t.NameX;
        let _ = t.NameY;
        let _ = t.NameW;
        let _ = t.NameH;
        let _ = t.MinContentVW;
        let _ = t.MinAltVW;
        let _ = t.DirPaddingL;
        let _ = t.LnkPaddingL;
    }

    #[test]
    fn alignment_round_trip() {
        let mut t = emFileManThemeData::default();
        t.NameAlignment = Alignment::Start;
        t.PathAlignment = Alignment::End;
        t.InfoAlignment = Alignment::Stretch;

        let rec = t.to_rec();
        let t2 = emFileManThemeData::from_rec(&rec).unwrap();

        assert_eq!(t2.NameAlignment, Alignment::Start);
        assert_eq!(t2.PathAlignment, Alignment::End);
        assert_eq!(t2.InfoAlignment, Alignment::Stretch);
    }

    #[test]
    fn image_border_round_trip() {
        let mut t = emFileManThemeData::default();
        t.OuterBorderImg = "border.png".to_string();
        t.OuterBorderImgL = 10;
        t.OuterBorderImgT = 20;
        t.OuterBorderImgR = 30;
        t.OuterBorderImgB = 40;

        let rec = t.to_rec();
        let t2 = emFileManThemeData::from_rec(&rec).unwrap();

        assert_eq!(t2.OuterBorderImg, "border.png");
        assert_eq!(t2.OuterBorderImgL, 10);
        assert_eq!(t2.OuterBorderImgT, 20);
        assert_eq!(t2.OuterBorderImgR, 30);
        assert_eq!(t2.OuterBorderImgB, 40);
    }

    #[test]
    fn all_colors_round_trip() {
        let mut t = emFileManThemeData::default();
        t.DirNameColor = 0xFF0000FF;
        t.FileContentColor = 0x00FF00AA;
        t.DirContentColor = 0x0000FFBB;

        let rec = t.to_rec();
        let t2 = emFileManThemeData::from_rec(&rec).unwrap();

        assert_eq!(t2.DirNameColor, 0xFF0000FF);
        assert_eq!(t2.FileContentColor, 0x00FF00AA);
        assert_eq!(t2.DirContentColor, 0x0000FFBB);
    }

    #[test]
    fn image_file_rec_empty_path_returns_fallback() {
        let rec = ImageFileRec::new("".to_string(), PathBuf::new());
        let img = rec.GetImage();
        assert_eq!(img.GetWidth(), 1);
        assert_eq!(img.GetHeight(), 1);
    }

    #[test]
    fn image_file_rec_caches_result() {
        let rec = ImageFileRec::new("".to_string(), PathBuf::new());
        let _img1 = rec.GetImage();
        let _img2 = rec.GetImage();
    }

    #[test]
    fn theme_model_acquire_same_name() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let t1 = emFileManTheme::Acquire(&ctx, "test_theme");
        let t2 = emFileManTheme::Acquire(&ctx, "test_theme");
        assert!(Rc::ptr_eq(&t1, &t2));
    }

    #[test]
    fn theme_model_field_access() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let theme = emFileManTheme::Acquire(&ctx, "default");
        let theme = theme.borrow();
        assert_eq!(theme.GetRec().Height, 0.0);
    }
}
