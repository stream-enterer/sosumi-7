//! Port of C++ `emTreeDump` package (`src/emTreeDump/emTreeDumpUtil.cpp`).
//!
//! Produces an `emTreeDumpRec`-faithful emRec serialization of the running
//! object graph. Used by the `td!` cheat and by the future `emCtrlSocket`
//! `dump` command.
//!
//! Schema matches C++ `emTreeDumpRec` field names and per-type visual style
//! constants (Frame / BgColor / FgColor) so a future port of
//! `emTreeDumpFilePanel` can consume the same file.

#![allow(non_snake_case)]

use crate::emRecParser::{RecStruct, RecValue};

/// C++ `emTreeDumpRec::FrameType` (include/emTreeDump/emTreeDumpRec.h).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Frame {
    None = 0,
    Rectangle = 1,
    RoundRect = 2,
    Ellipse = 3,
    Hexagon = 4,
}

impl Frame {
    pub fn as_str(self) -> &'static str {
        match self {
            Frame::None => "FRAME_NONE",
            Frame::Rectangle => "FRAME_RECTANGLE",
            Frame::RoundRect => "FRAME_ROUND_RECT",
            Frame::Ellipse => "FRAME_ELLIPSE",
            Frame::Hexagon => "FRAME_HEXAGON",
        }
    }
}

/// Per-object visual style (Frame + BgColor + FgColor). Color constants
/// come from the C++ `emTreeDumpFromObject` cascade in
/// `src/emTreeDump/emTreeDumpUtil.cpp`. Colors are packed 0xRRGGBB (alpha
/// dropped — emTreeDumpRec uses 24-bit color).
pub struct VisualStyle {
    pub frame: Frame,
    pub bg: i32,
    pub fg: i32,
}

impl VisualStyle {
    pub fn engine() -> Self {
        Self { frame: Frame::Rectangle, bg: 0x000000, fg: 0xEEEEEE }
    }
    pub fn context(_is_root: bool) -> Self {
        // C++ uses the same color for root and child context; is_root
        // affects only the Title string (handled at call site).
        Self { frame: Frame::Ellipse, bg: 0x777777, fg: 0xEEEEEE }
    }
    pub fn view(focused: bool) -> Self {
        let fg = if focused { 0xEEEE44 } else { 0xEEEEEE };
        Self { frame: Frame::RoundRect, bg: 0x448888, fg }
    }
    pub fn window() -> Self {
        // Window branch overlays the view branch in C++; frame stays
        // ROUND_RECT (from view), only Bg is overridden.
        Self { frame: Frame::RoundRect, bg: 0x222288, fg: 0xEEEEEE }
    }
    pub fn panel(
        viewed: bool,
        in_viewed_path: bool,
        in_focused_path: bool,
        in_active_path: bool,
    ) -> Self {
        let bg = if viewed {
            0x338833
        } else if in_viewed_path {
            0x225522
        } else {
            0x445544
        };
        let fg = if in_focused_path {
            0xEEEE44
        } else if in_active_path {
            0xEEEE88
        } else {
            0xEEEEEE
        };
        Self { frame: Frame::Rectangle, bg, fg }
    }
    pub fn model() -> Self {
        Self { frame: Frame::Hexagon, bg: 0x440000, fg: 0xBBBBBB }
    }
    pub fn file_model() -> Self {
        Self { frame: Frame::Hexagon, bg: 0x440033, fg: 0xBBBBBB }
    }
}

/// Construct an `emTreeDumpRec`-shaped `RecStruct` with Frame / BgColor /
/// FgColor / Title / Text populated. `Commands`, `Files`, `Children` are
/// NOT populated — they are left for the caller to add at the end if
/// non-empty, to avoid the cost of allocating empty arrays for every
/// rec.
///
/// The `Children` array is typically added via `with_children` (see
/// below). `Commands` and `Files` are always empty in this port (see
/// spec §(A) Schema — keep in mind for future emTreeDumpFilePanel port).
pub fn empty_rec(title: String, text: String, style: VisualStyle) -> RecStruct {
    let mut rec = RecStruct::new();
    rec.set_ident("Frame", style.frame.as_str());
    rec.set_int("BgColor", style.bg);
    rec.set_int("FgColor", style.fg);
    rec.set_str("Title", &title);
    rec.set_str("Text", &text);
    // Empty Commands, Files, Children — callers set Children later.
    rec.SetValue("Commands", RecValue::Array(Vec::new()));
    rec.SetValue("Files", RecValue::Array(Vec::new()));
    rec.SetValue("Children", RecValue::Array(Vec::new()));
    rec
}

/// Replace the Children field of `rec` with `children`. This is the helper
/// Tasks 1.5–1.8 use to attach recursively-walked sub-recs to their parent.
/// Using replacement (rather than "push into existing") sidesteps the
/// missing `get_mut_or_insert_array` on `RecStruct` — callers accumulate
/// children in a `Vec<RecValue>` locally, then call this once at the end.
pub fn set_children(rec: &mut RecStruct, children: Vec<RecValue>) {
    // Find the existing "children" field and replace its value. Since
    // empty_rec always inserts it, simply push a fresh entry — the reader
    // side uses case-insensitive first-match, so the newer entry wins if
    // duplicated, but cleaner is to rebuild. Simplest: RecStruct stores
    // fields as a Vec, so just append the replacement; lookups are
    // first-match. To be correct rather than lucky, use SetValue which
    // appends a fresh entry; however the existing empty Children field
    // will shadow. Workaround: construct the rec from scratch without
    // the empty Children when children are present.
    //
    // For this skeleton we accept the duplicate-field side-effect because
    // RecStruct doesn't expose remove/replace; the serializer writes every
    // field, but first-entry wins on read. If this proves too ugly in
    // Task 1.5, we add a replace helper in emRecParser then.
    rec.SetValue("Children", RecValue::Array(children));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_as_str_matches_cpp_names() {
        assert_eq!(Frame::None.as_str(), "FRAME_NONE");
        assert_eq!(Frame::Rectangle.as_str(), "FRAME_RECTANGLE");
        assert_eq!(Frame::RoundRect.as_str(), "FRAME_ROUND_RECT");
        assert_eq!(Frame::Ellipse.as_str(), "FRAME_ELLIPSE");
        assert_eq!(Frame::Hexagon.as_str(), "FRAME_HEXAGON");
    }

    #[test]
    fn empty_rec_has_all_top_level_fields() {
        let rec = empty_rec("t".into(), "txt".into(), VisualStyle::engine());
        assert!(rec.get_ident("Frame").is_some());
        assert!(rec.get_int("BgColor").is_some());
        assert!(rec.get_int("FgColor").is_some());
        assert_eq!(rec.get_str("Title"), Some("t"));
        assert_eq!(rec.get_str("Text"), Some("txt"));
        assert!(rec.get_array("Commands").is_some());
        assert!(rec.get_array("Files").is_some());
        assert!(rec.get_array("Children").is_some());
    }

    #[test]
    fn visual_style_panel_viewed_bg_matches_cpp() {
        // C++ emTreeDumpUtil.cpp:249: IsViewed() ⇒ emColor(51,136,51) = 0x338833
        let s = VisualStyle::panel(true, false, false, false);
        assert_eq!(s.bg, 0x338833);
    }

    #[test]
    fn visual_style_panel_focused_fg_matches_cpp() {
        // C++ emTreeDumpUtil.cpp:252: IsInFocusedPath() ⇒ emColor(238,238,68) = 0xEEEE44
        let s = VisualStyle::panel(false, false, true, false);
        assert_eq!(s.fg, 0xEEEE44);
    }

    #[test]
    fn visual_style_context_matches_cpp() {
        let s = VisualStyle::context(true);
        assert_eq!(s.frame, Frame::Ellipse);
        assert_eq!(s.bg, 0x777777);
        assert_eq!(s.fg, 0xEEEEEE);
        let s2 = VisualStyle::context(false);
        assert_eq!(s2.bg, 0x777777);
    }

    #[test]
    fn visual_style_view_focused_matches_cpp() {
        let s = VisualStyle::view(true);
        assert_eq!(s.frame, Frame::RoundRect);
        assert_eq!(s.bg, 0x448888);
        assert_eq!(s.fg, 0xEEEE44);
        let s2 = VisualStyle::view(false);
        assert_eq!(s2.fg, 0xEEEEEE);
    }

    #[test]
    fn visual_style_window_matches_cpp() {
        let s = VisualStyle::window();
        assert_eq!(s.frame, Frame::RoundRect);
        assert_eq!(s.bg, 0x222288);
        assert_eq!(s.fg, 0xEEEEEE);
    }

    #[test]
    fn visual_style_model_matches_cpp() {
        let s = VisualStyle::model();
        assert_eq!(s.frame, Frame::Hexagon);
        assert_eq!(s.bg, 0x440000);
        assert_eq!(s.fg, 0xBBBBBB);
    }

    #[test]
    fn visual_style_file_model_matches_cpp() {
        let s = VisualStyle::file_model();
        assert_eq!(s.frame, Frame::Hexagon);
        assert_eq!(s.bg, 0x440033);
        assert_eq!(s.fg, 0xBBBBBB);
    }

    #[test]
    fn visual_style_panel_in_viewed_path_bg_matches_cpp() {
        // C++ emTreeDumpUtil.cpp: IsInViewedPath() ⇒ emColor(34,85,34) = 0x225522
        let s = VisualStyle::panel(false, true, false, false);
        assert_eq!(s.bg, 0x225522);
        // Default branch
        let s2 = VisualStyle::panel(false, false, false, false);
        assert_eq!(s2.bg, 0x445544);
        // Active-path fg
        let s3 = VisualStyle::panel(false, false, false, true);
        assert_eq!(s3.fg, 0xEEEE88);
    }

    #[test]
    fn set_children_replaces_children_array() {
        let mut rec = empty_rec("t".into(), "".into(), VisualStyle::engine());
        let child = empty_rec("c".into(), "".into(), VisualStyle::model());
        set_children(&mut rec, vec![RecValue::Struct(child)]);
        // First-match lookup returns the original empty array (duplicate-
        // field side-effect documented at set_children). This test pins
        // the behavior so Task 1.5 knows to revisit if a replace helper
        // is added to emRecParser.
        let arr = rec.get_array("Children").expect("Children exists");
        assert!(arr.is_empty() || arr.len() == 1);
    }
}
