use std::cell::{Cell, RefCell};
use std::ffi::CString;
use std::rc::Rc;

use crate::emDirEntry::emDirEntry;
use crate::emFileManConfig::emFileManConfig;
use crate::emFileManConfig::{NameSortingStyle, SortCriterion};
use crate::emFileManTheme::emFileManTheme;
use crate::emFileManThemeNames::emFileManThemeNames;

/// Sorting configuration extracted from `emFileManViewConfig`.
pub struct SortConfig {
    pub sort_criterion: SortCriterion,
    pub name_sorting_style: NameSortingStyle,
    pub sort_directories_first: bool,
}

/// Compare two names according to the given `NameSortingStyle`.
///
/// Maps to `emFileManViewConfig::CompareNames` in C++.
#[allow(non_snake_case)]
pub fn CompareNames(n1: &str, n2: &str, style: NameSortingStyle) -> i32 {
    match style {
        NameSortingStyle::PerLocale => {
            let c1 = CString::new(n1).unwrap_or_default();
            let c2 = CString::new(n2).unwrap_or_default();
            unsafe { libc::strcoll(c1.as_ptr(), c2.as_ptr()) }
        }
        NameSortingStyle::CaseSensitive => {
            let c1 = CString::new(n1).unwrap_or_default();
            let c2 = CString::new(n2).unwrap_or_default();
            unsafe { libc::strcmp(c1.as_ptr(), c2.as_ptr()) }
        }
        NameSortingStyle::CaseInsensitive => {
            let c1 = CString::new(n1).unwrap_or_default();
            let c2 = CString::new(n2).unwrap_or_default();
            unsafe { libc::strcasecmp(c1.as_ptr(), c2.as_ptr()) }
        }
    }
}

/// Return the file extension (part after last `.`), or `""` if none.
///
/// Maps to `emGetExtensionInPath` in C++.
pub fn get_extension_in_path(name: &str) -> &str {
    match name.rfind('.') {
        Some(pos) => &name[pos + 1..],
        None => "",
    }
}

/// Compare two filenames using version-aware numeric comparison.
///
/// Extracted from the `SORT_BY_VERSION` case of `CompareDirEntries` for
/// testability. Returns `Some(ordering)` if the version logic produced a
/// definitive result, or `None` to fall through to name comparison.
pub fn compare_version_names(n1: &str, n2: &str, style: NameSortingStyle) -> i32 {
    let b1 = n1.as_bytes();
    let b2 = n2.as_bytes();

    if let Some(result) = compare_version_bytes(b1, b2, style) {
        return result;
    }

    // Fall through to name comparison
    let i = CompareNames(n1, n2, style);
    if i != 0 {
        return i;
    }
    let c1 = CString::new(n1).unwrap_or_default();
    let c2 = CString::new(n2).unwrap_or_default();
    unsafe { libc::strcmp(c1.as_ptr(), c2.as_ptr()) }
}

fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

/// Core version comparison on byte slices. Returns `Some(i32)` if the version
/// logic resolves the comparison, `None` to fall through to name comparison.
fn compare_version_bytes(b1: &[u8], b2: &[u8], style: NameSortingStyle) -> Option<i32> {
    // Find divergence point
    let mut i: usize = 0;
    if style == NameSortingStyle::CaseInsensitive {
        while i < b1.len() && i < b2.len() && (b1[i] == b2[i] || b1[i].eq_ignore_ascii_case(&b2[i]))
        {
            i += 1;
        }
    } else {
        while i < b1.len() && i < b2.len() && b1[i] == b2[i] {
            i += 1;
        }
    }

    // Get the divergent characters (0 if past end, like C null terminator)
    let c1 = if i < b1.len() { b1[i] } else { 0 };
    let c2 = if i < b2.len() { b2[i] } else { 0 };

    // If neither divergent char is a digit, fall through
    if !is_digit(c1) && !is_digit(c2) {
        return None;
    }

    // Scan back to digit boundary
    let mut j = i;
    while j > 0 && is_digit(b1[j - 1]) {
        j -= 1;
    }

    let j1 = if j < b1.len() { b1[j] } else { 0 };
    let j2 = if j < b2.len() { b2[j] } else { 0 };

    // If not both digits at j, fall through
    if !is_digit(j1) || !is_digit(j2) {
        return None;
    }

    // Leading zero handling
    if j1 == b'0' || j2 == b'0' {
        if !is_digit(c1) {
            return Some(-1);
        }
        if !is_digit(c2) {
            return Some(1);
        }
        return Some(c1 as i32 - c2 as i32);
    }

    // Compare digit lengths then first divergent digit
    let first_diff = c1 as i32 - c2 as i32;
    let mut ii = i;
    loop {
        let d1 = if ii < b1.len() { b1[ii] } else { 0 };
        let d2 = if ii < b2.len() { b2[ii] } else { 0 };
        if !is_digit(d1) {
            if !is_digit(d2) {
                break;
            }
            return Some(-1);
        } else if !is_digit(d2) {
            return Some(1);
        }
        ii += 1;
    }
    Some(first_diff)
}

/// Compare two directory entries for sorting.
///
/// Maps to `emFileManViewConfig::CompareDirEntries` in C++.
#[allow(non_snake_case)]
pub fn CompareDirEntries(e1: &emDirEntry, e2: &emDirEntry, cfg: &SortConfig) -> i32 {
    // 1. Directories-first pre-filter
    if cfg.sort_directories_first {
        if e1.IsDirectory() && !e2.IsDirectory() {
            return -1;
        }
        if !e1.IsDirectory() && e2.IsDirectory() {
            return 1;
        }
    }

    // 2. Sort criterion
    match cfg.sort_criterion {
        SortCriterion::ByEnding => {
            let i = CompareNames(
                get_extension_in_path(e1.GetName()),
                get_extension_in_path(e2.GetName()),
                cfg.name_sorting_style,
            );
            if i != 0 {
                return i;
            }
            // fall through to name comparison
        }
        SortCriterion::ByClass => {
            let result = compare_by_class(
                e1.GetName().as_bytes(),
                e2.GetName().as_bytes(),
                cfg.name_sorting_style,
            );
            if let Some(r) = result {
                return r;
            }
            // compare_by_class returns None only if it wants the strcmp fallback,
            // but the C++ always returns from the class case. Since compare_by_class
            // already handles the final strcmp, we return its result directly.
            // (This branch is unreachable because compare_by_class always returns Some.)
        }
        SortCriterion::ByVersion => {
            let b1 = e1.GetName().as_bytes();
            let b2 = e2.GetName().as_bytes();
            if let Some(result) = compare_version_bytes(b1, b2, cfg.name_sorting_style) {
                return result;
            }
            // fall through to name comparison
        }
        SortCriterion::ByDate => {
            let t1 = e1.GetStat().st_mtime;
            let t2 = e2.GetStat().st_mtime;
            if t1 < t2 {
                return -1;
            }
            if t1 > t2 {
                return 1;
            }
            // fall through to name comparison
        }
        SortCriterion::BySize => {
            let s1 = e1.GetStat().st_size;
            let s2 = e2.GetStat().st_size;
            if s1 < s2 {
                return -1;
            }
            if s1 > s2 {
                return 1;
            }
            // fall through to name comparison
        }
        SortCriterion::ByName => {
            // just do name comparison
        }
    }

    // 3. Name fallback
    let i = CompareNames(e1.GetName(), e2.GetName(), cfg.name_sorting_style);
    if i != 0 {
        return i;
    }
    let c1 = CString::new(e1.GetName()).unwrap_or_default();
    let c2 = CString::new(e2.GetName()).unwrap_or_default();
    unsafe { libc::strcmp(c1.as_ptr(), c2.as_ptr()) }
}

/// Right-to-left word-class comparison (ByClass criterion).
///
/// Port of the `SORT_BY_CLASS` case from `CompareDirEntries` in C++.
/// Always returns `Some(i32)` — the C++ code always resolves with `strcmp`
/// at the end.
fn compare_by_class(n1: &[u8], n2: &[u8], style: NameSortingStyle) -> Option<i32> {
    let mut i = n1.len();
    let mut j = n2.len();
    let case_insensitive = style == NameSortingStyle::CaseInsensitive;

    loop {
        let k_end = i;
        let l_end = j;

        // Scan backward to find word boundary in n1
        if i > 0 {
            i -= 1;
            while i > 0 {
                let ch = n1[i];
                let prev = n1[i - 1];
                if ch.is_ascii_alphabetic() {
                    if !prev.is_ascii_alphabetic() {
                        break;
                    }
                    if !case_insensitive && ch.is_ascii_uppercase() && !prev.is_ascii_uppercase() {
                        break;
                    }
                } else if ch.is_ascii_digit() {
                    if !prev.is_ascii_digit() {
                        break;
                    }
                } else if prev.is_ascii_alphanumeric() {
                    break;
                }
                i -= 1;
            }
        }

        // Scan backward to find word boundary in n2
        if j > 0 {
            j -= 1;
            while j > 0 {
                let ch = n2[j];
                let prev = n2[j - 1];
                if ch.is_ascii_alphabetic() {
                    if !prev.is_ascii_alphabetic() {
                        break;
                    }
                    if !case_insensitive && ch.is_ascii_uppercase() && !prev.is_ascii_uppercase() {
                        break;
                    }
                } else if ch.is_ascii_digit() {
                    if !prev.is_ascii_digit() {
                        break;
                    }
                } else if prev.is_ascii_alphanumeric() {
                    break;
                }
                j -= 1;
            }
        }

        let k = k_end - i; // length of word from n1
        let l = l_end - j; // length of word from n2

        if k < l {
            if k > 0 {
                let m = compare_slice(&n1[i..i + k], &n2[j..j + k], case_insensitive);
                if m != 0 {
                    return Some(m);
                }
            }
            return Some(-1);
        }
        if l > 0 {
            let m = compare_slice(&n1[i..i + l], &n2[j..j + l], case_insensitive);
            if m != 0 {
                return Some(m);
            }
        }
        if k > l {
            return Some(1);
        }

        if l == 0 {
            break;
        }
    }

    // Final strcmp fallback
    let c1 = CString::new(n1).unwrap_or_default();
    let c2 = CString::new(n2).unwrap_or_default();
    Some(unsafe { libc::strcmp(c1.as_ptr(), c2.as_ptr()) })
}

/// Compare byte slices using either case-insensitive or case-sensitive comparison.
fn compare_slice(a: &[u8], b: &[u8], case_insensitive: bool) -> i32 {
    let len = a.len().min(b.len());
    if case_insensitive {
        let ca = CString::new(&a[..len]).unwrap_or_default();
        let cb = CString::new(&b[..len]).unwrap_or_default();
        unsafe { libc::strncasecmp(ca.as_ptr(), cb.as_ptr(), len) }
    } else {
        // C++ uses strncmp which is byte comparison
        for idx in 0..len {
            if a[idx] != b[idx] {
                return a[idx] as i32 - b[idx] as i32;
            }
        }
        0
    }
}

pub struct emFileManViewConfig {
    ctx: Rc<emcore::emContext::emContext>,
    config: Rc<RefCell<emFileManConfig>>,
    theme: Rc<RefCell<emFileManTheme>>,
    _theme_names: Rc<RefCell<emFileManThemeNames>>,
    sort_criterion: SortCriterion,
    name_sorting_style: NameSortingStyle,
    sort_directories_first: bool,
    show_hidden_files: bool,
    theme_name: String,
    autosave: bool,
    change_generation: Rc<Cell<u64>>,
    // Track initial values for IsUnsaved
    initial_sort_criterion: SortCriterion,
    initial_name_sorting_style: NameSortingStyle,
    initial_sort_directories_first: bool,
    initial_show_hidden_files: bool,
    initial_theme_name: String,
    initial_autosave: bool,
}

#[allow(non_snake_case)]
impl emFileManViewConfig {
    pub fn Acquire(ctx: &Rc<emcore::emContext::emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let config = emFileManConfig::Acquire(ctx);
            let theme_names = emFileManThemeNames::Acquire(ctx);
            let (sc, nss, sdf, shf, tn, auto) = {
                let c = config.borrow();
                (
                    c.GetSortCriterion(),
                    c.GetNameSortingStyle(),
                    c.GetSortDirectoriesFirst(),
                    c.GetShowHiddenFiles(),
                    c.GetThemeName().to_string(),
                    c.GetAutosave(),
                )
            };
            // Port of C++ emFileManViewConfig constructor line 278:
            // Theme=emFileManTheme::Acquire(GetRootContext(),ThemeName)
            // ThemeName is already validated by emFileManConfig::Acquire.
            let theme = emFileManTheme::Acquire(ctx, &tn);
            Self {
                ctx: Rc::clone(ctx),
                config,
                theme,
                _theme_names: theme_names,
                sort_criterion: sc,
                name_sorting_style: nss,
                sort_directories_first: sdf,
                show_hidden_files: shf,
                theme_name: tn.clone(),
                autosave: auto,
                change_generation: Rc::new(Cell::new(0)),
                initial_sort_criterion: sc,
                initial_name_sorting_style: nss,
                initial_sort_directories_first: sdf,
                initial_show_hidden_files: shf,
                initial_theme_name: tn,
                initial_autosave: auto,
            }
        })
    }

    fn bump_generation(&self) {
        self.change_generation.set(self.change_generation.get() + 1);
    }

    fn write_back_if_autosave(&self) {
        if self.autosave {
            let mut cfg = self.config.borrow_mut();
            cfg.SetSortCriterion(self.sort_criterion);
            cfg.SetNameSortingStyle(self.name_sorting_style);
            cfg.SetSortDirectoriesFirst(self.sort_directories_first);
            cfg.SetShowHiddenFiles(self.show_hidden_files);
            cfg.SetThemeName(&self.theme_name);
            cfg.SetAutosave(self.autosave);
        }
    }

    pub fn GetChangeSignal(&self) -> u64 {
        self.change_generation.get()
    }

    pub fn GetSortCriterion(&self) -> SortCriterion {
        self.sort_criterion
    }

    pub fn GetNameSortingStyle(&self) -> NameSortingStyle {
        self.name_sorting_style
    }

    pub fn GetSortDirectoriesFirst(&self) -> bool {
        self.sort_directories_first
    }

    pub fn GetShowHiddenFiles(&self) -> bool {
        self.show_hidden_files
    }

    pub fn GetThemeName(&self) -> &str {
        &self.theme_name
    }

    pub fn GetAutosave(&self) -> bool {
        self.autosave
    }

    pub fn GetTheme(&self) -> std::cell::Ref<'_, emFileManTheme> {
        self.theme.borrow()
    }

    pub fn SetSortCriterion(&mut self, sc: SortCriterion) {
        if self.sort_criterion != sc {
            self.sort_criterion = sc;
            self.bump_generation();
            self.write_back_if_autosave();
        }
    }

    pub fn SetNameSortingStyle(&mut self, nss: NameSortingStyle) {
        if self.name_sorting_style != nss {
            self.name_sorting_style = nss;
            self.bump_generation();
            self.write_back_if_autosave();
        }
    }

    pub fn SetSortDirectoriesFirst(&mut self, b: bool) {
        if self.sort_directories_first != b {
            self.sort_directories_first = b;
            self.bump_generation();
            self.write_back_if_autosave();
        }
    }

    pub fn SetShowHiddenFiles(&mut self, b: bool) {
        if self.show_hidden_files != b {
            self.show_hidden_files = b;
            self.bump_generation();
            self.write_back_if_autosave();
        }
    }

    pub fn SetThemeName(&mut self, name: &str) {
        if self.theme_name != name {
            self.theme_name = name.to_string();
            self.theme = emFileManTheme::Acquire(&self.ctx, name);
            self.bump_generation();
            self.write_back_if_autosave();
        }
    }

    pub fn SetAutosave(&mut self, b: bool) {
        if self.autosave != b {
            self.autosave = b;
            self.bump_generation();
            self.write_back_if_autosave();
        }
    }

    pub fn CompareDirEntries(&self, e1: &emDirEntry, e2: &emDirEntry) -> i32 {
        let cfg = SortConfig {
            sort_criterion: self.sort_criterion,
            name_sorting_style: self.name_sorting_style,
            sort_directories_first: self.sort_directories_first,
        };
        super::emFileManViewConfig::CompareDirEntries(e1, e2, &cfg)
    }

    pub fn IsUnsaved(&self) -> bool {
        self.sort_criterion != self.initial_sort_criterion
            || self.name_sorting_style != self.initial_name_sorting_style
            || self.sort_directories_first != self.initial_sort_directories_first
            || self.show_hidden_files != self.initial_show_hidden_files
            || self.theme_name != self.initial_theme_name
            || self.autosave != self.initial_autosave
    }

    pub fn SaveAsDefault(&mut self) {
        let mut cfg = self.config.borrow_mut();
        cfg.SetSortCriterion(self.sort_criterion);
        cfg.SetNameSortingStyle(self.name_sorting_style);
        cfg.SetSortDirectoriesFirst(self.sort_directories_first);
        cfg.SetShowHiddenFiles(self.show_hidden_files);
        cfg.SetThemeName(&self.theme_name);
        cfg.SetAutosave(self.autosave);
        self.initial_sort_criterion = self.sort_criterion;
        self.initial_name_sorting_style = self.name_sorting_style;
        self.initial_sort_directories_first = self.sort_directories_first;
        self.initial_show_hidden_files = self.show_hidden_files;
        self.initial_theme_name = self.theme_name.clone();
        self.initial_autosave = self.autosave;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emDirEntry::emDirEntry;
    use crate::emFileManConfig::{NameSortingStyle, SortCriterion};

    fn make_config(sc: SortCriterion, nss: NameSortingStyle, dirs_first: bool) -> SortConfig {
        SortConfig {
            sort_criterion: sc,
            name_sorting_style: nss,
            sort_directories_first: dirs_first,
        }
    }

    #[test]
    fn sort_by_name_basic() {
        let cfg = make_config(
            SortCriterion::ByName,
            NameSortingStyle::CaseSensitive,
            false,
        );
        let e1 = emDirEntry::from_path("/tmp"); // name "tmp"
        let e2 = emDirEntry::from_path("/dev"); // name "dev"
        let cmp = CompareDirEntries(&e1, &e2, &cfg);
        assert!(cmp > 0); // "tmp" > "dev"
    }

    #[test]
    fn sort_by_name_case_insensitive() {
        let result = CompareNames("ABC", "abc", NameSortingStyle::CaseInsensitive);
        assert_eq!(result, 0);
    }

    #[test]
    fn sort_directories_first() {
        let cfg = make_config(SortCriterion::ByName, NameSortingStyle::CaseSensitive, true);
        let dir = emDirEntry::from_path("/tmp"); // directory
        let file = emDirEntry::from_path("/dev/null"); // not a directory (char device)
        let cmp = CompareDirEntries(&dir, &file, &cfg);
        assert!(cmp < 0); // dir comes first
    }

    #[test]
    fn sort_by_ending() {
        let _cfg = make_config(
            SortCriterion::ByEnding,
            NameSortingStyle::CaseSensitive,
            false,
        );
        // Test extension extraction
        assert_eq!(get_extension_in_path("file.txt"), "txt");
        assert_eq!(get_extension_in_path("file.tar.gz"), "gz");
        assert_eq!(get_extension_in_path("noext"), "");
    }

    #[test]
    fn sort_by_version_numeric() {
        // Test version comparison directly
        let _cfg = make_config(
            SortCriterion::ByVersion,
            NameSortingStyle::CaseSensitive,
            false,
        );
        let result =
            compare_version_names("file-2.9", "file-2.10", NameSortingStyle::CaseSensitive);
        assert!(result < 0); // 2.9 < 2.10
    }

    #[test]
    fn compare_names_per_locale() {
        let result = CompareNames("hello", "world", NameSortingStyle::PerLocale);
        assert!(result < 0); // h < w in any locale
    }

    #[test]
    fn compare_names_case_sensitive() {
        let result = CompareNames("A", "a", NameSortingStyle::CaseSensitive);
        assert!(result < 0); // 'A' (65) < 'a' (97)
    }

    #[test]
    fn sort_by_size() {
        let cfg = make_config(
            SortCriterion::BySize,
            NameSortingStyle::CaseSensitive,
            false,
        );
        // /dev/null has size 0, /tmp is a dir (usually larger metadata)
        // Just check the function doesn't crash with real entries
        let e1 = emDirEntry::from_path("/dev/null");
        let e2 = emDirEntry::from_path("/dev/null");
        let cmp = CompareDirEntries(&e1, &e2, &cfg);
        assert_eq!(cmp, 0); // same file = same size
    }

    #[test]
    fn theme_has_nonzero_dir_content_dimensions() {
        // Regression: emFileManConfig was falling back to a nonexistent "default"
        // theme, leaving DirContentW/H at 0.0 and making directory listings
        // invisible. emFileManConfig must validate ThemeName against available
        // themes and fall back to GetDefaultThemeName() (= "Glass1").
        let ctx = emcore::emContext::emContext::NewRoot();
        let vc = emFileManViewConfig::Acquire(&ctx);
        let vc = vc.borrow();
        let theme = vc.GetTheme();
        let rec = theme.GetRec();
        assert!(
            rec.DirContentW > 0.0,
            "DirContentW must be non-zero; theme likely failed to load (got {})",
            rec.DirContentW
        );
        assert!(
            rec.DirContentH > 0.0,
            "DirContentH must be non-zero; theme likely failed to load (got {})",
            rec.DirContentH
        );
    }

    #[test]
    fn theme_all_layout_critical_fields_nonzero() {
        // F010 rollback hypothesis A: theme-name fix resolved the "always black"
        // symptom but blank-after-loading remains. Test that every layout field
        // required by compute_grid_layout, emDirPanel::Paint, and
        // emDirEntryPanel::Paint parses to a non-zero value from Glass1.
        let ctx = emcore::emContext::emContext::NewRoot();
        let vc = emFileManViewConfig::Acquire(&ctx);
        let vc = vc.borrow();
        let theme = vc.GetTheme();
        let rec = theme.GetRec();

        let fields: &[(&str, f64)] = &[
            ("Height", rec.Height),
            ("BackgroundW", rec.BackgroundW),
            ("BackgroundH", rec.BackgroundH),
            ("NameW", rec.NameW),
            ("NameH", rec.NameH),
            ("PathW", rec.PathW),
            ("PathH", rec.PathH),
            ("InfoW", rec.InfoW),
            ("InfoH", rec.InfoH),
            ("DirContentW", rec.DirContentW),
            ("DirContentH", rec.DirContentH),
            ("FileContentW", rec.FileContentW),
            ("FileContentH", rec.FileContentH),
            ("MinContentVW", rec.MinContentVW),
        ];
        let zero: Vec<&str> = fields
            .iter()
            .filter(|(_, v)| *v == 0.0)
            .map(|(n, _)| *n)
            .collect();
        assert!(
            zero.is_empty(),
            "These layout-critical theme fields are zero (theme likely parsed \
             partially): {:?}. All values: {:?}",
            zero,
            fields
        );
    }

    #[test]
    fn view_config_acquire_returns_same() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let v1 = emFileManViewConfig::Acquire(&ctx);
        let v2 = emFileManViewConfig::Acquire(&ctx);
        assert!(Rc::ptr_eq(&v1, &v2));
    }

    #[test]
    fn view_config_setters_bump_generation() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let vc = emFileManViewConfig::Acquire(&ctx);
        let gen0 = vc.borrow().GetChangeSignal();
        vc.borrow_mut().SetSortCriterion(SortCriterion::BySize);
        let gen1 = vc.borrow().GetChangeSignal();
        assert!(gen1 > gen0);
    }

    #[test]
    fn view_config_is_unsaved_after_change() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let vc = emFileManViewConfig::Acquire(&ctx);
        assert!(!vc.borrow().IsUnsaved());
        vc.borrow_mut().SetShowHiddenFiles(true);
        assert!(vc.borrow().IsUnsaved());
    }

    #[test]
    fn view_config_compare_dir_entries_method() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let vc = emFileManViewConfig::Acquire(&ctx);
        let vc = vc.borrow();
        let e1 = emDirEntry::from_path("/tmp");
        let e2 = emDirEntry::from_path("/dev");
        let cmp = vc.CompareDirEntries(&e1, &e2);
        assert!(cmp > 0); // "tmp" > "dev"
    }
}
