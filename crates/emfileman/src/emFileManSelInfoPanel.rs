//! Port of C++ emFileManSelInfoPanel selection statistics state machine.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::PanelCtx;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};

use crate::emDirEntry::emDirEntry;
use crate::emFileManModel::emFileManModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanState {
    Costly,
    Wait,
    Scanning,
    Error,
    Success,
}

#[derive(Clone, Debug)]
pub struct ScanDetails {
    pub state: ScanState,
    pub error_message: String,
    pub entries: i32,
    pub hidden_entries: i32,
    pub symbolic_links: i32,
    pub regular_files: i32,
    pub subdirectories: i32,
    pub other_types: i32,
    pub size: u64,
    pub disk_usage: u64,
    pub disk_usage_unknown: bool,
}

impl ScanDetails {
    pub fn new() -> Self {
        Self {
            state: ScanState::Costly,
            error_message: String::new(),
            entries: 0,
            hidden_entries: 0,
            symbolic_links: 0,
            regular_files: 0,
            subdirectories: 0,
            other_types: 0,
            size: 0,
            disk_usage: 0,
            disk_usage_unknown: false,
        }
    }
}

impl Default for ScanDetails {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SelInfoState {
    pub direct: ScanDetails,
    pub recursive: ScanDetails,
}

impl SelInfoState {
    pub fn new() -> Self {
        Self {
            direct: ScanDetails::new(),
            recursive: ScanDetails::new(),
        }
    }
}

impl Default for SelInfoState {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a single entry, updating scan details.
pub fn work_on_detail_entry(details: &mut ScanDetails, entry: &crate::emDirEntry::emDirEntry) {
    details.entries += 1;
    if entry.IsHidden() {
        details.hidden_entries += 1;
    }
    if entry.IsSymbolicLink() {
        details.symbolic_links += 1;
    }
    if entry.IsRegularFile() {
        details.regular_files += 1;
        // Size is accumulated by the caller using GetLStat (for all entry
        // types, not just regular files), matching C++ WorkOnDetailEntry.
    } else if entry.IsDirectory() {
        details.subdirectories += 1;
    } else {
        details.other_types += 1;
    }
}

/// Process a single entry for recursive scanning (pushes dirs onto stack).
pub fn work_on_detail_entry_with_stack(
    details: &mut ScanDetails,
    entry: &crate::emDirEntry::emDirEntry,
    dir_stack: &mut Vec<String>,
) {
    work_on_detail_entry(details, entry);
    if entry.IsDirectory() {
        dir_stack.push(entry.GetPath().to_string());
    }
}

pub struct emFileManSelInfoPanel {
    file_man: Rc<RefCell<emFileManModel>>,
    pub(crate) state: SelInfoState,
    pub(crate) allow_business: bool,
    pub(crate) last_selection_gen: u64,
    dir_stack: Vec<String>,
    initial_dir_stack: Vec<String>,
    sel_list: Vec<String>,
    sel_index: usize,
    dir_path: String,
    dir_handle: Option<std::fs::ReadDir>,
    // Layout rectangles
    pub(crate) text_x: f64,
    pub(crate) text_y: f64,
    pub(crate) text_w: f64,
    pub(crate) text_h: f64,
    details_frame_x: f64,
    details_frame_y: f64,
    details_frame_w: f64,
    details_frame_h: f64,
    pub(crate) details_x: f64,
    details_y: f64,
    pub(crate) details_w: f64,
    details_h: f64,
}

impl emFileManSelInfoPanel {
    pub fn new(ctx: Rc<emContext>) -> Self {
        let file_man = emFileManModel::Acquire(&ctx);
        let last_selection_gen = file_man.borrow().GetSelectionSignal();
        let mut panel = Self {
            file_man,
            state: SelInfoState::new(),
            allow_business: false,
            last_selection_gen,
            dir_stack: Vec::new(),
            initial_dir_stack: Vec::new(),
            sel_list: Vec::new(),
            sel_index: 0,
            dir_path: String::new(),
            dir_handle: None,
            text_x: 0.0,
            text_y: 0.0,
            text_w: 0.0,
            text_h: 0.0,
            details_frame_x: 0.0,
            details_frame_y: 0.0,
            details_frame_w: 0.0,
            details_frame_h: 0.0,
            details_x: 0.0,
            details_y: 0.0,
            details_w: 0.0,
            details_h: 0.0,
        };
        panel.set_rectangles(1.0);
        panel
    }

    pub(crate) fn set_rectangles(&mut self, h: f64) {
        if h < 0.3 {
            // Wide layout
            let mut use_w = 1.0_f64;
            let mut use_h = 0.17_f64;
            if use_h > h {
                use_w *= h / use_h;
                use_h = h;
            }
            use_w -= use_h * 0.05;
            use_w -= use_h * 0.05;

            self.text_h = use_h;
            self.text_w = self.text_h / 0.29;
            self.text_x = (1.0 - use_w) * 0.5;
            self.text_y = (h - use_h) * 0.5;

            self.details_frame_h = use_h;
            self.details_frame_w = self.details_frame_h / 0.56;
            self.details_frame_x = self.text_x + use_w - self.details_frame_w;
            self.details_frame_y = self.text_y;
        } else {
            // Tall layout
            let mut use_w = 1.0_f64;
            let mut use_h = 0.76_f64;
            if use_h > h {
                use_w *= h / use_h;
                use_h = h;
            }
            use_w -= use_w * 0.05;
            use_h -= use_h * 0.05;

            self.text_w = use_w;
            self.text_h = self.text_w * 0.29;
            self.text_x = (1.0 - use_w) * 0.5;
            self.text_y = (h - use_h) * 0.5;

            self.details_frame_w = use_w;
            self.details_frame_h = self.details_frame_w * 0.44;
            self.details_frame_x = self.text_x;
            self.details_frame_y = self.text_y + use_h - self.details_frame_h;
        }

        self.details_w = self.details_frame_w * 0.3;
        self.details_h = self.details_w * 0.4667;
        self.details_x = self.details_frame_x + (self.details_frame_w - self.details_w) * 0.5;
        self.details_y = self.details_frame_y + (self.details_frame_h - self.details_h) * 0.5;
    }

    /// DIVERGED: (language-forced) C++ name is ResetDetails (private). Renamed to
    /// reset_details with pub(crate) visibility for test access.
    pub(crate) fn reset_details(&mut self) {
        self.state = SelInfoState::new();
        self.dir_stack.clear();
        self.initial_dir_stack.clear();
        self.sel_list.clear();
        self.sel_index = 0;
        self.dir_path.clear();
        self.dir_handle = None;
    }

    /// Port of C++ WorkOnDetails. Returns true if busy (should be called again).
    fn work_on_details(&mut self) -> bool {
        if !self.allow_business {
            match self.state.direct.state {
                ScanState::Wait => {
                    self.state.direct.state = ScanState::Costly;
                }
                ScanState::Scanning => {
                    self.state.direct.state = ScanState::Costly;
                    self.dir_stack.clear();
                    self.sel_list.clear();
                }
                _ => {}
            }
            match self.state.recursive.state {
                ScanState::Wait => {
                    self.state.recursive.state = ScanState::Costly;
                }
                ScanState::Scanning => {
                    self.state.recursive.state = ScanState::Costly;
                    self.dir_stack.clear();
                    self.dir_path.clear();
                    self.dir_handle = None;
                }
                _ => {}
            }
            return false;
        }

        // Direct scanning
        match self.state.direct.state {
            ScanState::Costly | ScanState::Wait => {
                self.state.direct = ScanDetails::new();
                self.state.direct.state = ScanState::Scanning;
                self.state.recursive.state = ScanState::Wait;
                let fm = self.file_man.borrow();
                let cnt = fm.GetTargetSelectionCount();
                self.sel_list.clear();
                for i in 0..cnt {
                    self.sel_list.push(fm.GetTargetSelection(i).to_string());
                }
                self.dir_stack.clear();
                self.sel_index = 0;
                return true;
            }
            ScanState::Scanning => {
                if self.sel_index >= self.sel_list.len() {
                    self.state.direct.state = ScanState::Success;
                    self.initial_dir_stack = self.dir_stack.clone();
                    self.dir_stack.clear();
                    self.sel_list.clear();
                    return true;
                }
                let path = self.sel_list[self.sel_index].clone();
                let entry = emDirEntry::from_path(&path);
                if entry.GetLStatErrNo() != 0 {
                    self.state.direct.state = ScanState::Error;
                    self.state.direct.error_message = format!(
                        "Failed to lstat \"{}\": errno {}",
                        entry.GetPath(),
                        entry.GetLStatErrNo()
                    );
                    self.state.recursive.state = ScanState::Error;
                    self.state.recursive.error_message = self.state.direct.error_message.clone();
                    self.sel_list.clear();
                    self.dir_stack.clear();
                    return false;
                }
                work_on_detail_entry_with_stack(
                    &mut self.state.direct,
                    &entry,
                    &mut self.dir_stack,
                );
                self.state.direct.size += entry.GetLStat().st_size as u64;
                #[cfg(target_os = "linux")]
                {
                    self.state.direct.disk_usage += (entry.GetLStat().st_blocks as u64) * 512;
                }
                #[cfg(not(target_os = "linux"))]
                {
                    self.state.direct.disk_usage_unknown = true;
                }
                self.sel_index += 1;
                return true;
            }
            _ => {}
        }

        // Recursive scanning
        match self.state.recursive.state {
            ScanState::Costly | ScanState::Wait => {
                self.state.recursive = self.state.direct.clone();
                self.state.recursive.state = ScanState::Scanning;
                self.dir_stack = self.initial_dir_stack.clone();
                return true;
            }
            ScanState::Scanning => {
                if self.dir_handle.is_none() {
                    if self.dir_stack.is_empty() {
                        self.state.recursive.state = ScanState::Success;
                        self.initial_dir_stack.clear();
                        return false;
                    }
                    self.dir_path = self.dir_stack.pop().unwrap();
                    match std::fs::read_dir(&self.dir_path) {
                        Ok(rd) => {
                            self.dir_handle = Some(rd);
                        }
                        Err(e) => {
                            self.state.recursive.state = ScanState::Error;
                            self.state.recursive.error_message =
                                format!("Failed to read dir \"{}\": {}", self.dir_path, e);
                            self.dir_stack.clear();
                            self.initial_dir_stack.clear();
                            self.dir_path.clear();
                            return false;
                        }
                    }
                    return true;
                }
                let dir_handle = self.dir_handle.as_mut().unwrap();
                match dir_handle.next() {
                    Some(Ok(de)) => {
                        let name = de.file_name().to_string_lossy().to_string();
                        let entry = emDirEntry::from_parent_and_name(&self.dir_path, &name);
                        if entry.GetLStatErrNo() != 0 {
                            self.state.recursive.state = ScanState::Error;
                            self.state.recursive.error_message = format!(
                                "Failed to lstat \"{}\": errno {}",
                                entry.GetPath(),
                                entry.GetLStatErrNo()
                            );
                            self.dir_stack.clear();
                            self.initial_dir_stack.clear();
                            self.dir_path.clear();
                            self.dir_handle = None;
                            return false;
                        }
                        work_on_detail_entry_with_stack(
                            &mut self.state.recursive,
                            &entry,
                            &mut self.dir_stack,
                        );
                        self.state.recursive.size += entry.GetLStat().st_size as u64;
                        #[cfg(target_os = "linux")]
                        {
                            self.state.recursive.disk_usage +=
                                (entry.GetLStat().st_blocks as u64) * 512;
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            self.state.recursive.disk_usage_unknown = true;
                        }
                        return true;
                    }
                    Some(Err(e)) => {
                        self.state.recursive.state = ScanState::Error;
                        self.state.recursive.error_message =
                            format!("Error reading dir \"{}\": {}", self.dir_path, e);
                        self.dir_stack.clear();
                        self.initial_dir_stack.clear();
                        self.dir_path.clear();
                        self.dir_handle = None;
                        return false;
                    }
                    None => {
                        self.dir_path.clear();
                        self.dir_handle = None;
                        return true;
                    }
                }
            }
            _ => {}
        }

        false
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_details(
        painter: &mut emPainter,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        caption: &str,
        details: &ScanDetails,
        color: emColor,
        canvas_color: emColor,
    ) {
        painter.PaintTextBoxed(
            x,
            y,
            w,
            h * 0.3,
            caption,
            h * 0.3,
            color,
            canvas_color,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );
        let y = y + h * 0.3;
        let h = h - h * 0.3;

        if details.state != ScanState::Success {
            let (msg, blend_color) = match details.state {
                ScanState::Costly => ("Costly", emColor::from_packed(0x886666FF)),
                ScanState::Wait => ("Wait...", emColor::from_packed(0x888800FF)),
                ScanState::Scanning => ("Scanning...", emColor::from_packed(0x008800FF)),
                _ => {
                    let msg = if details.error_message.is_empty() {
                        "ERROR"
                    } else {
                        &details.error_message
                    };
                    (msg, emColor::from_packed(0xFF0000FF))
                }
            };
            let blended = color.GetBlended(blend_color, 50.0);
            painter.PaintTextBoxed(
                x,
                y,
                w,
                h,
                msg,
                h * 0.1,
                blended,
                canvas_color,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Left,
                1.0,
                false,
                1.0,
            );
            return;
        }

        let d = h / 32.0;
        let text = format!("Entries: {}", details.entries);
        painter.PaintTextBoxed(
            x,
            y,
            w,
            d * 8.0,
            &text,
            d * 8.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        let text = format!("Hidden Entries: {}", details.hidden_entries);
        painter.PaintTextBoxed(
            x,
            y + d * 9.0,
            w,
            d * 2.0,
            &text,
            d * 2.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        let text = format!("Symbolic Links: {}", details.symbolic_links);
        painter.PaintTextBoxed(
            x,
            y + d * 12.0,
            w,
            d * 2.0,
            &text,
            d * 2.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        let text = format!("Regular Files : {}", details.regular_files);
        painter.PaintTextBoxed(
            x,
            y + d * 14.0,
            w,
            d * 2.0,
            &text,
            d * 2.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        let text = format!("Subdirectories: {}", details.subdirectories);
        painter.PaintTextBoxed(
            x,
            y + d * 16.0,
            w,
            d * 2.0,
            &text,
            d * 2.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        let text = format!("Other Types   : {}", details.other_types);
        painter.PaintTextBoxed(
            x,
            y + d * 18.0,
            w,
            d * 2.0,
            &text,
            d * 2.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        let text = format!("Size: {}", details.size);
        painter.PaintTextBoxed(
            x,
            y + d * 21.0,
            w,
            d * 8.0,
            &text,
            d * 8.0,
            color,
            canvas_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        if details.disk_usage_unknown {
            let text = "Disk Usage: unknown";
            painter.PaintTextBoxed(
                x,
                y + d * 30.0,
                w,
                d * 2.0,
                text,
                d * 2.0,
                color,
                canvas_color,
                TextAlignment::Left,
                VAlign::Top,
                TextAlignment::Left,
                1.0,
                false,
                1.0,
            );
        } else {
            let text = format!("Disk Usage: {}", details.disk_usage);
            painter.PaintTextBoxed(
                x,
                y + d * 30.0,
                w,
                d * 2.0,
                &text,
                d * 2.0,
                color,
                canvas_color,
                TextAlignment::Left,
                VAlign::Top,
                TextAlignment::Left,
                1.0,
                false,
                1.0,
            );
        }
    }
}

impl PanelBehavior for emFileManSelInfoPanel {
    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        let gen = self.file_man.borrow().GetSelectionSignal();
        if gen != self.last_selection_gen {
            self.last_selection_gen = gen;
            self.reset_details();
        }
        self.work_on_details()
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.contains(NoticeFlags::LAYOUT_CHANGED) {
            self.set_rectangles(state.height);
        }
        if flags.contains(NoticeFlags::VIEWING_CHANGED) {
            self.allow_business = state.viewed;
        }
    }

    fn IsOpaque(&self) -> bool {
        false
    }

    fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, _state: &PanelState) {
        let fm = self.file_man.borrow();
        let fg_src = emColor::from_packed(0x80E080FF);
        let text = format!("Sources:{:4}", fm.GetSourceSelectionCount());
        painter.PaintTextBoxed(
            self.text_x,
            self.text_y,
            self.text_w,
            self.text_h * 0.5,
            &text,
            self.text_h * 0.5,
            fg_src,
            emColor::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );
        let fg_tgt = emColor::from_packed(0xE08080FF);
        let text = format!("Targets:{:4}", fm.GetTargetSelectionCount());
        painter.PaintTextBoxed(
            self.text_x,
            self.text_y + self.text_h * 0.5,
            self.text_w,
            self.text_h * 0.5,
            &text,
            self.text_h * 0.5,
            fg_tgt,
            emColor::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );
        drop(fm);

        // 3D frame background
        let canvas = emColor::TRANSPARENT;
        painter.PaintRect(
            self.details_frame_x,
            self.details_frame_y,
            self.details_frame_w,
            self.details_frame_h,
            emColor::from_packed(0x00000030),
            canvas,
        );

        // Details area
        let s = self.details_w;
        let x = self.details_x;
        let y = self.details_y;
        let _h = s * 0.48;

        let bg1 = emColor::from_packed(0x880000FF);
        let fg1 = emColor::from_packed(0xE0E0E0FF);
        let bg2 = fg1;
        let fg2 = emColor::from_packed(0x000000FF);

        painter.PaintTextBoxed(
            x,
            y,
            s,
            s * 0.1,
            "Target Selection Details",
            s * 0.1,
            bg1,
            canvas,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Left,
            1.0,
            false,
            1.0,
        );

        painter.PaintRoundRect(
            x + s * 0.15,
            y + s * 0.13,
            s * 0.84,
            s * 0.34,
            s * 0.03,
            s * 0.03,
            bg2,
            emColor::TRANSPARENT,
        );

        painter.PaintRoundRect(
            x,
            y + s * 0.22,
            s * 0.28,
            s * 0.16,
            s * 0.02,
            s * 0.02,
            bg1,
            emColor::TRANSPARENT,
        );

        Self::paint_details(
            painter,
            x + s * 0.01,
            y + s * 0.23,
            s * 0.26,
            s * 0.14,
            "Direct",
            &self.state.direct,
            fg1,
            bg1,
        );
        Self::paint_details(
            painter,
            x + s * 0.33,
            y + s * 0.15,
            s * 0.52,
            s * 0.28,
            "Recursive",
            &self.state.recursive,
            fg2,
            bg2,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::rc::Rc;

    #[test]
    fn initial_state_is_costly() {
        let info = SelInfoState::new();
        assert_eq!(info.direct.state, ScanState::Costly);
        assert_eq!(info.recursive.state, ScanState::Costly);
    }

    #[test]
    fn work_on_detail_entry_counts_file() {
        let mut details = ScanDetails::new();
        let e = crate::emDirEntry::emDirEntry::from_path("/dev/null");
        work_on_detail_entry(&mut details, &e);
        assert_eq!(details.entries, 1);
    }

    #[test]
    fn work_on_detail_entry_counts_directory() {
        let mut details = ScanDetails::new();
        let e = crate::emDirEntry::emDirEntry::from_path("/tmp");
        let mut dir_stack = Vec::new();
        work_on_detail_entry_with_stack(&mut details, &e, &mut dir_stack);
        assert_eq!(details.subdirectories, 1);
        assert_eq!(dir_stack.len(), 1);
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManSelInfoPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn panel_initial_state() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManSelInfoPanel::new(Rc::clone(&ctx));
        assert_eq!(panel.state.direct.state, ScanState::Costly);
        assert!(!panel.allow_business);
    }

    #[test]
    fn set_rectangles_wide() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emFileManSelInfoPanel::new(Rc::clone(&ctx));
        panel.set_rectangles(0.2); // wide panel (height < 0.3)
        assert!(panel.text_w > 0.0);
        assert!(panel.details_w > 0.0);
    }

    #[test]
    fn reset_details_clears_state() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let mut panel = emFileManSelInfoPanel::new(Rc::clone(&ctx));
        // Simulate scanning state
        panel.state.direct.state = ScanState::Scanning;
        panel.state.direct.entries = 5;
        panel.sel_list.push("/tmp".to_string());
        panel.sel_index = 3;

        panel.reset_details();

        assert_eq!(panel.state.direct.state, ScanState::Costly);
        assert_eq!(panel.state.direct.entries, 0);
        assert!(panel.sel_list.is_empty());
        assert_eq!(panel.sel_index, 0);
    }

    #[test]
    fn generation_tracking_detects_selection_change() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManSelInfoPanel::new(Rc::clone(&ctx));
        let initial_gen = panel.last_selection_gen;

        // Change selection
        panel.file_man.borrow_mut().SelectAsTarget("/tmp");

        // Generation should have changed
        let new_gen = panel.file_man.borrow().GetSelectionSignal();
        assert_ne!(new_gen, initial_gen);
    }
}
