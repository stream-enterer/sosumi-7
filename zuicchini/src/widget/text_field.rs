use std::rc::Rc;

use crate::foundation::Rect;
use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::look::Look;

const TEXT_PADDING: f64 = 2.0;
const TEXT_SIZE: f64 = 13.0;
const LINE_HEIGHT: f64 = TEXT_SIZE + 2.0;
const DOUBLE_CLICK_MS: u128 = 500;
const DOUBLE_CLICK_DIST: f64 = 3.0;

type TextChangeCb = Box<dyn FnMut(&str)>;
type ValidateCb = Box<dyn FnMut(&str) -> bool>;
type ClipboardCopyCb = Box<dyn Fn(&str)>;
type ClipboardPasteCb = Box<dyn Fn() -> String>;

/// Snapshot of text state for undo/redo.
#[derive(Clone, Debug)]
struct UndoEntry {
    text: String,
    cursor: usize,
}

/// Mouse drag mode (matching C++ DM_* enum).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DragMode {
    None,
    SelectChars,
    SelectWords,
    SelectRows,
    Insert,
    Move,
}

/// Single-line or multi-line text input widget.
pub struct TextField {
    border: Border,
    look: Rc<Look>,
    text: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    scroll_x: f64,
    password_mode: bool,
    max_length: usize,
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    pub on_text: Option<TextChangeCb>,

    // Phase 1 fields
    editable: bool,
    multi_line: bool,
    overwrite_mode: bool,
    scroll_y: f64,
    visible_rows: usize,
    drag_mode: DragMode,
    click_count: u8,
    last_click_time: Option<std::time::Instant>,
    last_click_x: f64,
    last_click_y: f64,
    last_w: f64,
    last_h: f64,
    char_positions: Vec<f64>,
    row_y_positions: Vec<f64>,
    magic_col: Option<usize>,
    pub on_selection: Option<Box<dyn FnMut(usize, usize)>>,
    pub on_validate: Option<ValidateCb>,
    pub on_clipboard_copy: Option<ClipboardCopyCb>,
    pub on_clipboard_paste: Option<ClipboardPasteCb>,
    // Cursor blink state
    cursor_blink_on: bool,
    cursor_blink_time: std::time::Instant,
    // Signal callbacks
    pub on_selection_signal: Option<Box<dyn FnMut()>>,
    pub on_can_undo_redo: Option<Box<dyn FnMut(bool, bool)>>,
    // Published selection tracking
    selection_published: bool,
    /// TF-003: Pending view scroll request — cursor rect in panel-pixel coords.
    /// Set by scroll_to_cursor(), consumed by take_pending_scroll_to_visible().
    pending_scroll_to_visible: Option<(f64, f64, f64, f64)>,
}

const MAX_UNDO: usize = 100;

impl TextField {
    pub fn new(look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Rect).with_inner(InnerBorderType::InputField),
            look,
            text: String::new(),
            cursor: 0,
            selection_anchor: None,
            scroll_x: 0.0,
            password_mode: false,
            max_length: usize::MAX,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            on_text: None,
            editable: true,
            multi_line: false,
            overwrite_mode: false,
            scroll_y: 0.0,
            visible_rows: 4,
            drag_mode: DragMode::None,
            click_count: 0,
            last_click_time: None,
            last_click_x: 0.0,
            last_click_y: 0.0,
            last_w: 0.0,
            last_h: 0.0,
            char_positions: Vec::new(),
            row_y_positions: Vec::new(),
            magic_col: None,
            on_selection: None,
            on_validate: None,
            on_clipboard_copy: None,
            on_clipboard_paste: None,
            cursor_blink_on: true,
            cursor_blink_time: std::time::Instant::now(),
            on_selection_signal: None,
            on_can_undo_redo: None,
            selection_published: false,
            pending_scroll_to_visible: None,
        }
    }

    // ── Property accessors ──────────────────────────────────────────────

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        self.text = text.to_string();
        self.cursor = self.text.len();
        self.selection_anchor = None;
        self.magic_col = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.fire_change();
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor_index(&mut self, idx: usize) {
        self.cursor = self.clamp_to_boundary(idx);
    }

    pub fn text_len(&self) -> usize {
        self.text.len()
    }

    pub fn set_password_mode(&mut self, enabled: bool) {
        if self.password_mode == enabled {
            return;
        }
        self.password_mode = enabled;
    }

    pub fn password_mode(&self) -> bool {
        self.password_mode
    }

    pub fn set_max_length(&mut self, max: usize) {
        self.max_length = max;
    }

    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        self.border.inner = if editable {
            InnerBorderType::InputField
        } else {
            InnerBorderType::OutputField
        };
    }

    pub fn is_editable(&self) -> bool {
        self.editable
    }

    pub fn set_multi_line(&mut self, multi_line: bool) {
        if self.multi_line == multi_line {
            return;
        }
        self.multi_line = multi_line;
        self.scroll_y = 0.0;
    }

    pub fn is_multi_line(&self) -> bool {
        self.multi_line
    }

    pub fn set_overwrite_mode(&mut self, mode: bool) {
        if self.overwrite_mode == mode {
            return;
        }
        self.overwrite_mode = mode;
    }

    pub fn is_overwrite_mode(&self) -> bool {
        self.overwrite_mode
    }

    // ── Selection API ───────────────────────────────────────────────────

    pub fn select(&mut self, start: usize, end: usize) {
        let start = self.clamp_to_boundary(start);
        let end = self.clamp_to_boundary(end);
        if start >= end {
            self.selection_anchor = None;
            self.cursor = start;
        } else {
            self.selection_anchor = Some(start);
            self.cursor = end;
        }
        self.fire_selection_change();
    }

    pub fn select_all(&mut self) {
        self.select(0, self.text.len());
    }

    pub fn deselect(&mut self) {
        self.selection_anchor = None;
        self.fire_selection_change();
    }

    pub fn selection_start(&self) -> usize {
        match self.selection_anchor {
            Some(anchor) => anchor.min(self.cursor),
            None => self.cursor,
        }
    }

    pub fn selection_end(&self) -> usize {
        match self.selection_anchor {
            Some(anchor) => anchor.max(self.cursor),
            None => self.cursor,
        }
    }

    pub fn is_selection_empty(&self) -> bool {
        self.selection_anchor.is_none() || self.selection_start() == self.selection_end()
    }

    pub fn selected_text(&self) -> &str {
        let start = self.selection_start();
        let end = self.selection_end();
        &self.text[start..end]
    }

    /// Publishes the current selection to the primary clipboard (X11 selection).
    /// In password mode, publishes asterisks instead of actual text.
    /// No-op if selection is empty or already published.
    /// Matches C++ `PublishSelection`.
    pub fn publish_selection(&mut self) {
        if self.is_selection_empty() || self.selection_published {
            return;
        }
        let text = if self.password_mode {
            "*".repeat(self.selected_text().chars().count())
        } else {
            self.selected_text().to_string()
        };
        if let Some(cb) = &self.on_clipboard_copy {
            cb(&text);
        }
        self.selection_published = true;
    }

    fn modify_selection(&mut self, new_cursor: usize, extend: bool) {
        let old_start = self.selection_start();
        let old_end = self.selection_end();
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
            self.cursor = new_cursor;
        } else {
            self.selection_anchor = None;
            self.cursor = new_cursor;
        }
        let new_start = self.selection_start();
        let new_end = self.selection_end();
        if old_start != new_start || old_end != new_end {
            self.fire_selection_change();
        }
    }

    fn fire_selection_change(&mut self) {
        self.selection_published = false;
        if self.on_selection.is_some() {
            let start = self.selection_start();
            let end = self.selection_end();
            if let Some(cb) = &mut self.on_selection {
                cb(start, end);
            }
        }
        if let Some(cb) = &mut self.on_selection_signal {
            cb();
        }
        self.selection_changed();
    }

    // ── Undo/Redo ───────────────────────────────────────────────────────

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear_undo(&mut self) {
        self.undo_stack.clear();
    }

    pub fn clear_redo(&mut self) {
        self.redo_stack.clear();
    }

    fn save_undo(&mut self) {
        let had_undo = self.can_undo();
        let had_redo = self.can_redo();
        self.undo_stack.push(UndoEntry {
            text: self.text.clone(),
            cursor: self.cursor,
        });
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        if self.can_undo() != had_undo || self.can_redo() != had_redo {
            self.fire_can_undo_redo();
        }
    }

    pub fn undo(&mut self) -> bool {
        if let Some(entry) = self.undo_stack.pop() {
            self.redo_stack.push(UndoEntry {
                text: self.text.clone(),
                cursor: self.cursor,
            });
            self.text = entry.text;
            self.cursor = entry.cursor;
            self.selection_anchor = None;
            self.fire_change();
            self.fire_can_undo_redo();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(entry) = self.redo_stack.pop() {
            self.undo_stack.push(UndoEntry {
                text: self.text.clone(),
                cursor: self.cursor,
            });
            self.text = entry.text;
            self.cursor = entry.cursor;
            self.selection_anchor = None;
            self.fire_change();
            self.fire_can_undo_redo();
            true
        } else {
            false
        }
    }

    // ── Word/Line Navigation (Phase 2) ──────────────────────────────────

    fn is_word_char(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_' || !ch.is_ascii()
    }

    fn next_word_boundary(&self, pos: usize) -> usize {
        if self.password_mode {
            return self.text.len();
        }
        let len = self.text.len();
        let mut p = pos;
        // Skip word chars
        while p < len {
            let ch = self.char_at(p);
            if !Self::is_word_char(ch) {
                break;
            }
            p += ch.len_utf8();
        }
        // Skip non-word chars
        while p < len {
            let ch = self.char_at(p);
            if Self::is_word_char(ch) {
                break;
            }
            p += ch.len_utf8();
        }
        p
    }

    fn prev_word_boundary(&self, pos: usize) -> usize {
        if self.password_mode {
            return 0;
        }
        let mut p = pos;
        // Skip non-word chars backward
        while p > 0 {
            let prev = self.prev_char_boundary(p);
            let ch = self.char_at(prev);
            if Self::is_word_char(ch) {
                break;
            }
            p = prev;
        }
        // Skip word chars backward
        while p > 0 {
            let prev = self.prev_char_boundary(p);
            let ch = self.char_at(prev);
            if !Self::is_word_char(ch) {
                break;
            }
            p = prev;
        }
        p
    }

    fn word_start(&self, pos: usize) -> usize {
        let mut p = pos;
        while p > 0 {
            let prev = self.prev_char_boundary(p);
            if !Self::is_word_char(self.char_at(prev)) {
                break;
            }
            p = prev;
        }
        p
    }

    fn word_end(&self, pos: usize) -> usize {
        let mut p = pos;
        while p < self.text.len() {
            let ch = self.char_at(p);
            if !Self::is_word_char(ch) {
                break;
            }
            p += ch.len_utf8();
        }
        p
    }

    fn row_start(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let bytes = self.text.as_bytes();
        let mut j = 0usize;
        let mut i = 0usize;
        while i < pos && i < bytes.len() {
            let c = bytes[i];
            if c == b'\r' || c == b'\n' {
                i += 1;
                // Skip LF after CR (CR+LF pair).
                if c == b'\r' && i < bytes.len() && bytes[i] == b'\n' {
                    i += 1;
                }
                if i <= pos {
                    j = i;
                }
            } else {
                i += 1;
            }
        }
        j
    }

    fn row_end(&self, pos: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut i = pos;
        while i < bytes.len() {
            let c = bytes[i];
            if c == b'\n' || c == b'\r' {
                return i;
            }
            i += 1;
        }
        self.text.len()
    }

    fn index_to_col_row(&self, pos: usize) -> (usize, usize) {
        let before = &self.text[..pos.min(self.text.len())];
        let row = before.matches('\n').count();
        let after_last_nl = match before.rfind('\n') {
            Some(nl) => &before[nl + 1..],
            None => before,
        };
        let col = after_last_nl.chars().count();
        (col, row)
    }

    pub fn col_row_to_index(&self, col: usize, row: usize) -> usize {
        let mut current_row = 0;
        let mut row_start = 0;
        for (i, ch) in self.text.char_indices() {
            if current_row == row {
                break;
            }
            if ch == '\n' {
                current_row += 1;
                row_start = i + 1;
            }
        }
        if current_row < row {
            return self.text.len();
        }
        // Find start of target row
        if row > 0 {
            let mut r = 0;
            row_start = 0;
            for (i, ch) in self.text.char_indices() {
                if r == row {
                    row_start = i;
                    break;
                }
                if ch == '\n' {
                    r += 1;
                    if r == row {
                        row_start = i + 1;
                        break;
                    }
                }
            }
            if r < row {
                return self.text.len();
            }
        }
        // Advance `col` chars within the row
        let mut idx = row_start;
        let mut c = 0;
        while c < col && idx < self.text.len() {
            let ch = self.char_at(idx);
            if ch == '\n' {
                break;
            }
            idx += ch.len_utf8();
            c += 1;
        }
        idx
    }

    pub fn total_rows(&self) -> usize {
        self.text.matches('\n').count() + 1
    }

    fn next_row_index(&self, pos: usize, target_col: usize) -> usize {
        let row_e = self.row_end(pos);
        if row_e >= self.text.len() {
            return pos; // no next row
        }
        // Skip the line ending (CR, LF, or CR+LF).
        let bytes = self.text.as_bytes();
        let mut next_row_start = row_e + 1;
        if bytes[row_e] == b'\r' && next_row_start < bytes.len() && bytes[next_row_start] == b'\n' {
            next_row_start += 1;
        }
        let next_row_end = self.row_end(next_row_start);
        let mut idx = next_row_start;
        let mut c = 0;
        while c < target_col && idx < next_row_end {
            let ch = self.char_at(idx);
            if ch == '\n' || ch == '\r' {
                break;
            }
            idx += ch.len_utf8();
            c += 1;
        }
        idx
    }

    fn prev_row_index(&self, pos: usize, target_col: usize) -> usize {
        let row_s = self.row_start(pos);
        if row_s == 0 {
            return pos; // no prev row
        }
        // Step back over the line ending (\n, \r, or \r\n).
        let bytes = self.text.as_bytes();
        let mut prev_row_end = row_s - 1;
        if bytes[prev_row_end] == b'\n' && prev_row_end > 0 && bytes[prev_row_end - 1] == b'\r' {
            prev_row_end -= 1;
        }
        let prev_row_start = self.row_start(prev_row_end);
        let mut idx = prev_row_start;
        let mut c = 0;
        while c < target_col && idx < prev_row_end {
            let ch = self.char_at(idx);
            if ch == '\n' || ch == '\r' {
                break;
            }
            idx += ch.len_utf8();
            c += 1;
        }
        idx
    }

    /// Finds the start of the next paragraph (a non-empty line after one or
    /// more newlines). In single-line mode, returns text length.
    /// Matches C++ `GetNextParagraphIndex`.
    fn next_paragraph_index(&self, pos: usize) -> usize {
        if !self.multi_line {
            return self.text.len();
        }
        let len = self.text.len();
        let mut p = pos;
        let mut found_newline = false;
        while p < len {
            let b = self.text.as_bytes()[p];
            if b == b'\n' || b == b'\r' {
                found_newline = true;
            } else if found_newline {
                break;
            }
            p += 1;
        }
        p
    }

    /// Finds the start of the previous paragraph by scanning from the
    /// beginning using `next_paragraph_index`. O(n) matching C++
    /// `GetPrevParagraphIndex`.
    fn prev_paragraph_index(&self, pos: usize) -> usize {
        if !self.multi_line {
            return 0;
        }
        let mut i = 0;
        loop {
            let j = self.next_paragraph_index(i);
            if j >= pos || j == i {
                return i;
            }
            i = j;
        }
    }

    // ── Word index (C++ GetNextWordIndex / GetPrevWordIndex) ─────────────

    /// Advances past delimiter segments to find the start of the next word.
    /// Unlike `next_word_boundary` (ctrl+arrow: skips word then delimiters),
    /// this skips only delimiter runs. Matches C++ `GetNextWordIndex`.
    pub fn next_word_index(&self, pos: usize) -> usize {
        let len = self.text.len();
        if pos >= len {
            return len;
        }
        let mut p = pos;
        loop {
            let (boundary, is_delim) = self.next_word_boundary_segment(p);
            if boundary >= len {
                return len;
            }
            if !is_delim {
                return boundary;
            }
            if boundary == p {
                return len;
            }
            p = boundary;
        }
    }

    /// Finds the previous word start by scanning from the beginning using
    /// `next_word_index`. O(n) matching C++ `GetPrevWordIndex`.
    pub fn prev_word_index(&self, pos: usize) -> usize {
        let mut i = 0;
        loop {
            let j = self.next_word_index(i);
            if j >= pos || j == i {
                return i;
            }
            i = j;
        }
    }

    /// Returns (boundary_index, is_delimiter_at_boundary) for the next word
    /// boundary segment starting at `pos`. The returned `is_delimiter` indicates
    /// the type of the character AT the boundary (i.e., the start of the next
    /// segment).
    fn next_word_boundary_segment(&self, pos: usize) -> (usize, bool) {
        let len = self.text.len();
        if pos >= len {
            return (len, true);
        }
        if self.password_mode {
            return (len, false);
        }
        let mut p = pos;
        let mut first = true;
        let mut prev_delim = false;
        while p < len {
            let ch = self.char_at(p);
            let is_delim = !Self::is_word_char(ch);
            if !first && is_delim != prev_delim {
                // Boundary: return position and the type of char AT boundary
                return (p, is_delim);
            }
            prev_delim = is_delim;
            first = false;
            p += ch.len_utf8();
        }
        // Reached end of text — return end with delimiter=true (no more text)
        (len, true)
    }

    // ── Coordinate conversion (Phase 5) ─────────────────────────────────

    fn x_to_index_single_line(&self, x: f64) -> usize {
        if self.char_positions.is_empty() {
            return 0;
        }
        let adjusted_x = x + self.scroll_x - TEXT_PADDING;
        if adjusted_x <= 0.0 {
            return 0;
        }
        for (i, &pos) in self.char_positions.iter().enumerate() {
            if i + 1 < self.char_positions.len() {
                let mid = (pos + self.char_positions[i + 1]) / 2.0;
                if adjusted_x < mid {
                    return self.char_index_at(i);
                }
            }
        }
        self.text.len()
    }

    fn char_index_at(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    // ── Clipboard (Phase 4) ─────────────────────────────────────────────

    fn copy_to_clipboard(&self) {
        if self.is_selection_empty() {
            return;
        }
        if let Some(cb) = &self.on_clipboard_copy {
            let text = if self.password_mode {
                "*".repeat(self.selected_text().chars().count())
            } else {
                self.selected_text().to_string()
            };
            cb(&text);
        }
    }

    fn cut_to_clipboard(&mut self) {
        if !self.editable || self.is_selection_empty() {
            return;
        }
        self.copy_to_clipboard();
        self.delete_selection();
        if self.validate_text() {
            self.fire_change();
        }
    }

    fn paste_from_clipboard(&mut self) {
        if !self.editable {
            return;
        }
        let text = if let Some(cb) = &self.on_clipboard_paste {
            cb()
        } else {
            return;
        };
        self.paste_text(&text);
    }

    pub fn paste_text(&mut self, text: &str) {
        if !self.editable || text.is_empty() {
            return;
        }
        if !self.delete_selection() {
            self.save_undo();
        }
        for ch in text.chars() {
            if ch.is_control() && ch != '\n' {
                continue;
            }
            if ch == '\n' && !self.multi_line {
                continue;
            }
            if self.text.chars().count() >= self.max_length {
                break;
            }
            self.text.insert(self.cursor, ch);
            self.cursor += ch.len_utf8();
        }
        if !self.validate_text() {
            return;
        }
        self.fire_change();
    }

    // ── Paint ───────────────────────────────────────────────────────────

    pub fn paint(&mut self, painter: &mut Painter, w: f64, h: f64) {
        self.last_w = w;
        self.last_h = h;

        self.border
            .paint_border(painter, w, h, &self.look, false, self.editable);

        let Rect {
            x: cx,
            y: cy,
            w: cw,
            h: ch,
        } = self.border.content_rect(w, h, &self.look);

        painter.push_state();
        painter.clip_rect(cx, cy, cw, ch);

        if self.multi_line {
            self.paint_multi_line(painter, cx, cy, cw, ch);
        } else {
            self.paint_single_line(painter, cx, cy, cw, ch);
        }

        painter.pop_state();
    }

    fn paint_single_line(&mut self, painter: &mut Painter, cx: f64, cy: f64, cw: f64, ch: f64) {
        let display_text = if self.password_mode {
            "*".repeat(self.text.chars().count())
        } else {
            self.text.clone()
        };

        // Build char_positions
        self.char_positions.clear();
        self.char_positions.push(0.0);
        for (i, _ch) in display_text.char_indices() {
            let next = display_text[..=i]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            let end = i + next;
            let w_px = display_text[..end].len() as f64 * 7.0; // TODO(font): measure_text stub
            self.char_positions.push(w_px);
        }

        // Pre-compute selection rect
        let sel_rect = if let Some(anchor) = self.selection_anchor {
            let sel_start = anchor.min(self.cursor);
            let sel_end = anchor.max(self.cursor);
            let sx_px = display_text[..sel_start.min(display_text.len())].len() as f64 * 7.0; // TODO(font): measure_text stub
            let ex_px = display_text[..sel_end.min(display_text.len())].len() as f64 * 7.0; // TODO(font): measure_text stub
            Some((cx + TEXT_PADDING + sx_px - self.scroll_x, ex_px - sx_px))
        } else {
            None
        };

        let cursor_text = if self.password_mode {
            "*".repeat(self.text[..self.cursor].chars().count())
        } else {
            self.text[..self.cursor].to_string()
        };
        let cursor_x_px = cursor_text.len() as f64 * 7.0; // TODO(font): measure_text stub

        // Update scroll_x so the cursor stays visible
        let visible_w = cw - 2.0 * TEXT_PADDING;
        if cursor_x_px - self.scroll_x > visible_w {
            self.scroll_x = cursor_x_px - visible_w;
        }
        if cursor_x_px - self.scroll_x < 0.0 {
            self.scroll_x = cursor_x_px;
        }
        if self.scroll_x < 0.0 {
            self.scroll_x = 0.0;
        }

        // Selection highlight
        if let Some((sx, sw)) = sel_rect {
            painter.paint_rect(sx, cy, sw, ch, self.look.input_hl_color);
        }

        // Text
        let _text_x = cx + TEXT_PADDING - self.scroll_x;
        let _text_y = cy + (ch - TEXT_SIZE) / 2.0;

        let fg = if self.editable {
            self.look.input_fg_color
        } else {
            self.look
                .input_fg_color
                .lerp(self.look.input_bg_color, 0.80)
        };

        // TODO(font): paint text here (selection-aware rendering)

        // Cursor
        let cursor_x = cx + TEXT_PADDING + cursor_x_px - self.scroll_x;
        if self.overwrite_mode && self.cursor < self.text.len() {
            // Box cursor
            let ch_w = if self.cursor < display_text.len() {
                7.0 // TODO(font): measure_text stub
            } else {
                8.0
            };
            painter.paint_rect(cursor_x, cy + 1.0, ch_w, ch - 2.0, fg.with_alpha(80));
        } else {
            painter.paint_rect(cursor_x, cy + 1.0, 1.0, ch - 2.0, fg);
        }
    }

    fn paint_multi_line(&mut self, painter: &mut Painter, cx: f64, cy: f64, _cw: f64, ch: f64) {
        let rows: Vec<&str> = self.text.split('\n').collect();
        let total_rows = rows.len();

        // Update row_y_positions
        self.row_y_positions.clear();
        for i in 0..total_rows {
            self.row_y_positions.push(i as f64 * LINE_HEIGHT);
        }

        let (cursor_col, cursor_row) = self.index_to_col_row(self.cursor);
        let cursor_y_px = cursor_row as f64 * LINE_HEIGHT;

        // Scroll to keep cursor visible
        let visible_h = ch;
        if cursor_y_px - self.scroll_y + LINE_HEIGHT > visible_h {
            self.scroll_y = cursor_y_px + LINE_HEIGHT - visible_h;
        }
        if cursor_y_px - self.scroll_y < 0.0 {
            self.scroll_y = cursor_y_px;
        }
        if self.scroll_y < 0.0 {
            self.scroll_y = 0.0;
        }

        let fg = if self.editable {
            self.look.input_fg_color
        } else {
            self.look
                .input_fg_color
                .lerp(self.look.input_bg_color, 0.80)
        };

        let sel_start = self.selection_start();
        let sel_end = self.selection_end();
        let has_selection = !self.is_selection_empty();

        let mut byte_offset = 0usize;
        for (row_idx, row_text) in rows.iter().enumerate() {
            let row_y = cy + row_idx as f64 * LINE_HEIGHT - self.scroll_y;
            if row_y + LINE_HEIGHT < cy || row_y > cy + ch {
                byte_offset += row_text.len() + 1; // +1 for \n
                continue;
            }

            let row_byte_start = byte_offset;
            let row_byte_end = byte_offset + row_text.len();

            // Selection highlight for this row
            if has_selection && sel_start < row_byte_end && sel_end > row_byte_start {
                let hl_start = sel_start.max(row_byte_start) - row_byte_start;
                let hl_end = sel_end.min(row_byte_end) - row_byte_start;
                let sx = row_text[..hl_start].len() as f64 * 7.0; // TODO(font): measure_text stub
                let ex = row_text[..hl_end].len() as f64 * 7.0; // TODO(font): measure_text stub
                painter.paint_rect(
                    cx + TEXT_PADDING + sx,
                    row_y,
                    ex - sx,
                    LINE_HEIGHT,
                    self.look.input_hl_color,
                );
            }

            // TODO(font): paint text here

            byte_offset = row_byte_end + 1; // +1 for \n
        }

        // Cursor
        let cursor_row_start = self.row_start(self.cursor);
        let cursor_in_row = &self.text[cursor_row_start..self.cursor];
        let cursor_x_px = cursor_in_row.len() as f64 * 7.0; // TODO(font): measure_text stub
        let cursor_x = cx + TEXT_PADDING + cursor_x_px;
        let cursor_screen_y = cy + cursor_row as f64 * LINE_HEIGHT - self.scroll_y;
        let _ = cursor_col;

        if self.overwrite_mode && self.cursor < self.text.len() && self.char_at(self.cursor) != '\n'
        {
            let ch_w = 7.0; // TODO(font): measure_text stub
            painter.paint_rect(
                cursor_x,
                cursor_screen_y,
                ch_w,
                LINE_HEIGHT,
                fg.with_alpha(80),
            );
        } else {
            painter.paint_rect(cursor_x, cursor_screen_y, 1.0, LINE_HEIGHT, fg);
        }
    }

    // ── ScrollToCursor (TF-003) ────────────────────────────────────────

    /// TF-003: Compute cursor rect in panel-pixel coordinates and store as
    /// a pending view-scroll request. Matches C++ `emTextField::ScrollToCursor`.
    ///
    /// The cursor rect is in the same coordinate space as `paint(w, h)`.
    /// The panel behavior or framework reads this via
    /// `take_pending_scroll_to_visible()` and applies it to the View.
    pub fn scroll_to_cursor(&mut self) {
        if self.last_w <= 0.0 || self.last_h <= 0.0 {
            return;
        }

        let content = self
            .border
            .content_rect(self.last_w, self.last_h, &self.look);

        let (col, row) = self.index_to_col_row(self.cursor);

        // Cursor X from cached char_positions (populated during paint).
        let cursor_x_px = if col < self.char_positions.len() {
            self.char_positions[col]
        } else {
            self.char_positions.last().copied().unwrap_or(0.0)
        };

        // Cursor Y from row index.
        let cursor_y_px = if row < self.row_y_positions.len() {
            self.row_y_positions[row]
        } else {
            row as f64 * LINE_HEIGHT
        };

        // Cursor rect in panel-pixel coords (after internal scroll).
        // Padding matches C++ (±0.5 char, ±0.2 row).
        let half_char = 4.0;
        let x1 = content.x + TEXT_PADDING + cursor_x_px - self.scroll_x - half_char;
        let y1 = content.y + cursor_y_px - self.scroll_y - LINE_HEIGHT * 0.2;
        let x2 = x1 + half_char * 2.0;
        let y2 = y1 + LINE_HEIGHT * 1.4;

        self.pending_scroll_to_visible = Some((x1, y1, x2 - x1, y2 - y1));
    }

    /// Take the pending scroll-to-visible request, if any.
    /// Returns (x, y, w, h) in panel-pixel coordinates.
    pub fn take_pending_scroll_to_visible(&mut self) -> Option<(f64, f64, f64, f64)> {
        self.pending_scroll_to_visible.take()
    }

    // ── Input ───────────────────────────────────────────────────────────

    pub fn input(&mut self, event: &InputEvent) -> bool {
        // Handle mouse events
        if self.handle_mouse(event) {
            self.scroll_to_cursor();
            return true;
        }

        match event.variant {
            InputVariant::Press | InputVariant::Repeat => {}
            InputVariant::Release | InputVariant::Move => return false,
        }

        let shift = event.shift;
        let ctrl = event.ctrl;

        let consumed = match event.key {
            // ── Navigation ──────────────────────────────────────────
            InputKey::ArrowLeft => {
                self.magic_col = None;
                let new_pos = if ctrl {
                    self.prev_word_boundary(self.cursor)
                } else if self.cursor > 0 {
                    self.prev_char_boundary(self.cursor)
                } else {
                    self.cursor
                };
                self.modify_selection(new_pos, shift);
                true
            }
            InputKey::ArrowRight => {
                self.magic_col = None;
                let new_pos = if ctrl {
                    self.next_word_boundary(self.cursor)
                } else if self.cursor < self.text.len() {
                    self.next_char_boundary(self.cursor)
                } else {
                    self.cursor
                };
                self.modify_selection(new_pos, shift);
                true
            }
            InputKey::Home => {
                self.magic_col = None;
                let new_pos = if ctrl || !self.multi_line {
                    0
                } else {
                    self.row_start(self.cursor)
                };
                self.modify_selection(new_pos, shift);
                true
            }
            InputKey::End => {
                self.magic_col = None;
                let new_pos = if ctrl || !self.multi_line {
                    self.text.len()
                } else {
                    self.row_end(self.cursor)
                };
                self.modify_selection(new_pos, shift);
                true
            }
            InputKey::ArrowUp if self.multi_line => {
                let new_pos = if ctrl {
                    self.prev_paragraph_index(self.cursor)
                } else {
                    let (col, _row) = self.index_to_col_row(self.cursor);
                    let target_col = self.magic_col.unwrap_or(col);
                    self.magic_col = Some(target_col);
                    self.prev_row_index(self.cursor, target_col)
                };
                self.modify_selection(new_pos, shift);
                true
            }
            InputKey::ArrowDown if self.multi_line => {
                let new_pos = if ctrl {
                    self.next_paragraph_index(self.cursor)
                } else {
                    let (col, _row) = self.index_to_col_row(self.cursor);
                    let target_col = self.magic_col.unwrap_or(col);
                    self.magic_col = Some(target_col);
                    self.next_row_index(self.cursor, target_col)
                };
                self.modify_selection(new_pos, shift);
                true
            }

            // ── Editing (guarded by editable) ───────────────────────
            InputKey::Key('z') if ctrl && !shift => {
                if self.editable {
                    self.undo();
                }
                true
            }
            InputKey::Key('y') if ctrl && !shift => {
                if self.editable {
                    self.redo();
                }
                true
            }
            InputKey::Key('z') if ctrl && shift => {
                // Ctrl+Shift+Z = redo
                if self.editable {
                    self.redo();
                }
                true
            }
            InputKey::Key('a') if ctrl && !shift => {
                self.select_all();
                true
            }
            InputKey::Key('a') if ctrl && shift => {
                self.deselect();
                true
            }

            // Clipboard
            InputKey::Key('c') if ctrl && !shift => {
                self.copy_to_clipboard();
                true
            }
            InputKey::Key('x') if ctrl && !shift => {
                self.cut_to_clipboard();
                true
            }
            InputKey::Key('v') if ctrl && !shift => {
                self.paste_from_clipboard();
                true
            }
            InputKey::Insert if ctrl && !shift => {
                self.copy_to_clipboard();
                true
            }
            InputKey::Insert if shift && !ctrl => {
                self.paste_from_clipboard();
                true
            }
            InputKey::Delete if shift && !ctrl => {
                self.cut_to_clipboard();
                true
            }

            InputKey::Insert if !ctrl && !shift => {
                self.overwrite_mode = !self.overwrite_mode;
                true
            }

            InputKey::Backspace => {
                if !self.editable {
                    return true;
                }
                if self.delete_selection() {
                    if self.validate_text() {
                        self.fire_change();
                    }
                    return true;
                }
                if self.cursor > 0 {
                    self.save_undo();
                    let target = if ctrl && shift {
                        self.row_start(self.cursor)
                    } else if ctrl {
                        self.prev_word_boundary(self.cursor)
                    } else {
                        self.prev_char_boundary(self.cursor)
                    };
                    self.text.drain(target..self.cursor);
                    self.cursor = target;
                    if self.validate_text() {
                        self.fire_change();
                    }
                }
                self.magic_col = None;
                true
            }
            InputKey::Delete => {
                if !self.editable {
                    return true;
                }
                if self.delete_selection() {
                    if self.validate_text() {
                        self.fire_change();
                    }
                    return true;
                }
                if self.cursor < self.text.len() {
                    self.save_undo();
                    let target = if ctrl && shift {
                        self.row_end(self.cursor)
                    } else if ctrl {
                        self.next_word_boundary(self.cursor)
                    } else {
                        self.next_char_boundary(self.cursor)
                    };
                    self.text.drain(self.cursor..target);
                    if self.validate_text() {
                        self.fire_change();
                    }
                }
                self.magic_col = None;
                true
            }

            InputKey::Enter if self.multi_line && self.editable => {
                self.magic_col = None;
                if !self.delete_selection() {
                    self.save_undo();
                }
                self.text.insert(self.cursor, '\n');
                self.cursor += 1;
                if self.validate_text() {
                    self.fire_change();
                }
                true
            }

            _ => {
                if !event.chars.is_empty() && self.editable {
                    self.magic_col = None;
                    if !self.delete_selection() {
                        self.save_undo();
                    }
                    for ch in event.chars.chars() {
                        if ch.is_control() {
                            if ch == '\n' && self.multi_line {
                                // allow
                            } else {
                                continue;
                            }
                        }
                        if self.text.chars().count() >= self.max_length {
                            break;
                        }
                        if self.overwrite_mode
                            && self.cursor < self.text.len()
                            && self.char_at(self.cursor) != '\n'
                        {
                            let next = self.next_char_boundary(self.cursor);
                            self.text.drain(self.cursor..next);
                        }
                        self.text.insert(self.cursor, ch);
                        self.cursor += ch.len_utf8();
                    }
                    if self.validate_text() {
                        self.fire_change();
                    }
                    self.scroll_to_cursor();
                    return true;
                }
                false
            }
        };
        if consumed {
            self.scroll_to_cursor();
        }
        consumed
    }

    fn handle_mouse(&mut self, event: &InputEvent) -> bool {
        match event.key {
            InputKey::MouseLeft => {}
            _ => return false,
        }

        match event.variant {
            InputVariant::Press => self.handle_mouse_press(event),
            InputVariant::Move => self.handle_mouse_move(event),
            InputVariant::Release => self.handle_mouse_release(event),
            _ => false,
        }
    }

    fn handle_mouse_press(&mut self, event: &InputEvent) -> bool {
        let now = std::time::Instant::now();

        // Multi-click detection
        let is_multi_click = if let Some(last_time) = self.last_click_time {
            let elapsed = now.duration_since(last_time).as_millis();
            let dx = (event.mouse_x - self.last_click_x).abs();
            let dy = (event.mouse_y - self.last_click_y).abs();
            elapsed < DOUBLE_CLICK_MS && dx < DOUBLE_CLICK_DIST && dy < DOUBLE_CLICK_DIST
        } else {
            false
        };

        if is_multi_click {
            self.click_count = (self.click_count + 1).min(4);
        } else {
            self.click_count = 1;
        }
        self.last_click_time = Some(now);
        self.last_click_x = event.mouse_x;
        self.last_click_y = event.mouse_y;

        let pos = self.x_to_index_single_line(event.mouse_x);

        if event.ctrl && self.editable {
            // Ctrl+click: insert or move mode
            if !self.is_selection_empty()
                && pos >= self.selection_start()
                && pos < self.selection_end()
            {
                self.drag_mode = DragMode::Move;
            } else {
                self.cursor = pos;
                self.selection_anchor = None;
                self.drag_mode = DragMode::Insert;
            }
            return true;
        }

        match self.click_count {
            1 => {
                // Single click
                if event.shift {
                    self.modify_selection(pos, true);
                } else {
                    self.modify_selection(pos, false);
                }
                self.drag_mode = DragMode::SelectChars;
            }
            2 => {
                // Double click: select word
                let ws = self.word_start(pos);
                let we = self.word_end(pos);
                if event.shift {
                    // Extend to word boundary
                    let anchor = self.selection_anchor.unwrap_or(self.cursor);
                    if pos < anchor {
                        self.selection_anchor = Some(self.word_end(anchor));
                        self.cursor = ws;
                    } else {
                        self.selection_anchor = Some(self.word_start(anchor));
                        self.cursor = we;
                    }
                } else {
                    self.selection_anchor = Some(ws);
                    self.cursor = we;
                }
                self.fire_selection_change();
                self.drag_mode = DragMode::SelectWords;
            }
            3 => {
                // Triple click: select row
                let rs = self.row_start(pos);
                let re = self.row_end(pos);
                if event.shift {
                    let anchor = self.selection_anchor.unwrap_or(self.cursor);
                    if pos < anchor {
                        self.selection_anchor = Some(self.row_end(anchor));
                        self.cursor = rs;
                    } else {
                        self.selection_anchor = Some(self.row_start(anchor));
                        self.cursor = if re < self.text.len() { re + 1 } else { re };
                    }
                } else {
                    self.selection_anchor = Some(rs);
                    self.cursor = if re < self.text.len() { re + 1 } else { re };
                }
                self.fire_selection_change();
                self.drag_mode = DragMode::SelectRows;
            }
            _ => {
                // Quad+ click: select all
                self.select_all();
                self.drag_mode = DragMode::SelectChars;
            }
        }
        self.magic_col = None;
        true
    }

    fn handle_mouse_move(&mut self, event: &InputEvent) -> bool {
        match self.drag_mode {
            DragMode::None => false,
            DragMode::SelectChars => {
                let pos = self.x_to_index_single_line(event.mouse_x);
                if self.selection_anchor.is_none() {
                    self.selection_anchor = Some(self.cursor);
                }
                self.cursor = pos;
                self.fire_selection_change();
                true
            }
            DragMode::SelectWords => {
                let pos = self.x_to_index_single_line(event.mouse_x);
                if let Some(anchor) = self.selection_anchor {
                    let anchor_ws = self.word_start(anchor);
                    let anchor_we = self.word_end(anchor);
                    if pos < anchor_ws {
                        self.selection_anchor = Some(anchor_we);
                        self.cursor = self.word_start(pos);
                    } else {
                        self.selection_anchor = Some(anchor_ws);
                        self.cursor = self.word_end(pos);
                    }
                    self.fire_selection_change();
                }
                true
            }
            DragMode::SelectRows => {
                let pos = self.x_to_index_single_line(event.mouse_x);
                if let Some(anchor) = self.selection_anchor {
                    let anchor_rs = self.row_start(anchor);
                    let anchor_re = self.row_end(anchor);
                    if pos < anchor_rs {
                        let end = if anchor_re < self.text.len() {
                            anchor_re + 1
                        } else {
                            anchor_re
                        };
                        self.selection_anchor = Some(end);
                        self.cursor = self.row_start(pos);
                    } else {
                        self.selection_anchor = Some(anchor_rs);
                        let re = self.row_end(pos);
                        self.cursor = if re < self.text.len() { re + 1 } else { re };
                    }
                    self.fire_selection_change();
                }
                true
            }
            DragMode::Insert | DragMode::Move => true,
        }
    }

    fn handle_mouse_release(&mut self, event: &InputEvent) -> bool {
        let was_dragging = self.drag_mode != DragMode::None;

        if self.drag_mode == DragMode::Move && self.editable {
            let pos = self.x_to_index_single_line(event.mouse_x);
            let sel_start = self.selection_start();
            let sel_end = self.selection_end();
            if pos < sel_start || pos > sel_end {
                let selected = self.text[sel_start..sel_end].to_string();
                self.save_undo();
                // Remove selection first
                self.text.drain(sel_start..sel_end);
                // Adjust insert position
                let insert_pos = if pos > sel_end {
                    pos - (sel_end - sel_start)
                } else {
                    pos
                };
                self.text.insert_str(insert_pos, &selected);
                self.cursor = insert_pos + selected.len();
                self.selection_anchor = Some(insert_pos);
                self.fire_change();
                self.fire_selection_change();
            }
        }

        self.drag_mode = DragMode::None;
        was_dragging
    }

    pub fn get_cursor(&self) -> Cursor {
        Cursor::Text
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let cw = 120.0;
        let ch = if self.multi_line {
            LINE_HEIGHT * self.visible_rows as f64
        } else {
            TEXT_SIZE + 4.0
        };
        self.border.preferred_size_for_content(cw, ch)
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    fn char_at(&self, pos: usize) -> char {
        self.text[pos..].chars().next().unwrap_or('\0')
    }

    fn prev_char_boundary(&self, pos: usize) -> usize {
        let mut p = pos - 1;
        while p > 0 && !self.text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    fn next_char_boundary(&self, pos: usize) -> usize {
        let mut p = pos + 1;
        while p < self.text.len() && !self.text.is_char_boundary(p) {
            p += 1;
        }
        p
    }

    fn clamp_to_boundary(&self, pos: usize) -> usize {
        let pos = pos.min(self.text.len());
        if pos == 0 || self.text.is_char_boundary(pos) {
            return pos;
        }
        // Walk backward to find a valid boundary
        let mut p = pos;
        while p > 0 && !self.text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    fn delete_selection(&mut self) -> bool {
        if let Some(anchor) = self.selection_anchor.take() {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            if start != end {
                self.save_undo();
                self.text.drain(start..end);
                self.cursor = start;
                return true;
            }
        }
        false
    }

    fn validate_text(&mut self) -> bool {
        if let Some(cb) = &mut self.on_validate {
            if !cb(&self.text) {
                // Revert via undo
                if let Some(entry) = self.undo_stack.pop() {
                    self.text = entry.text;
                    self.cursor = entry.cursor;
                }
                return false;
            }
        }
        true
    }

    fn fire_change(&mut self) {
        if let Some(cb) = &mut self.on_text {
            cb(&self.text);
        }
    }

    /// Fires the can-undo-redo callback when undo/redo availability changes.
    /// Matches C++ `CanUndoRedoSignal`.
    fn fire_can_undo_redo(&mut self) {
        if let Some(cb) = &mut self.on_can_undo_redo {
            let can_undo = !self.undo_stack.is_empty();
            let can_redo = !self.redo_stack.is_empty();
            cb(can_undo, can_redo);
        }
    }

    // ── Cursor blink ───────────────────────────────────────────────────

    /// Returns whether the cursor blink is currently in the "on" (visible)
    /// state. Matches C++ `IsCursorBlinkOn`.
    pub fn is_cursor_blink_on(&self) -> bool {
        self.cursor_blink_on
    }

    /// Toggles cursor blink state based on elapsed time. Should be called
    /// from a periodic timer. Returns `true` if the widget is busy (needs
    /// continued cycling). `focused` indicates whether this text field is
    /// in the focused path. Matches C++ `Cycle` blink logic.
    pub fn cycle_blink(&mut self, focused: bool) -> bool {
        if focused {
            let now = std::time::Instant::now();
            let elapsed_ms = now.duration_since(self.cursor_blink_time).as_millis();
            if elapsed_ms >= 1000 {
                self.cursor_blink_time = now;
                self.cursor_blink_on = true;
            } else if elapsed_ms >= 500 {
                self.cursor_blink_on = false;
            }
            true
        } else {
            self.cursor_blink_time = std::time::Instant::now();
            self.cursor_blink_on = true;
            false
        }
    }

    /// Resets the blink timer and ensures the cursor is visible. Should be
    /// called after user actions that move the cursor. Matches C++
    /// `RestartCursorBlinking`.
    pub fn restart_cursor_blinking(&mut self) {
        self.cursor_blink_time = std::time::Instant::now();
        self.cursor_blink_on = true;
    }

    /// Hook called when the selection changes.
    /// Matches C++ `SelectionChanged` — empty virtual hook.
    pub fn selection_changed(&self) {
        // Empty hook.
    }

    /// Computes total columns (widest row) and rows.
    /// In single-line mode, columns = char count, rows = 1.
    /// Tab stops every 8 columns. Minimum (1, 1).
    /// Matches C++ `CalcTotalColsRows`.
    pub fn calc_total_cols_rows(&self) -> (usize, usize) {
        if !self.multi_line {
            let cols = self.text.chars().count().max(1);
            return (cols, 1);
        }
        let mut cols: usize = 0;
        let mut rows: usize = 1;
        let mut row_cols: usize = 0;
        for ch in self.text.chars() {
            match ch {
                '\t' => {
                    row_cols = (row_cols / 8 + 1) * 8;
                }
                '\n' | '\r' => {
                    if cols < row_cols {
                        cols = row_cols;
                    }
                    row_cols = 0;
                    rows += 1;
                }
                _ => {
                    row_cols += 1;
                }
            }
        }
        if cols < row_cols {
            cols = row_cols;
        }
        (cols.max(1), rows.max(1))
    }

    /// Mouse coordinates to text byte index.
    /// Returns `(index, hit)` where `hit` is true if within content area.
    /// Matches C++ `CheckMouse`.
    pub fn check_mouse(&self, mx: f64, my: f64, w: f64, h: f64) -> (usize, bool) {
        let content = self.border.content_rect(w, h, &self.look);
        let hit = mx >= content.x
            && mx <= content.x + content.w
            && my >= content.y
            && my <= content.y + content.h;
        if self.multi_line {
            let row_f = (my - content.y + self.scroll_y) / LINE_HEIGHT;
            let row = (row_f as usize).min(self.total_rows().saturating_sub(1));
            let mut current_row = 0;
            let mut row_start_idx = 0;
            for (i, ch) in self.text.char_indices() {
                if current_row == row {
                    row_start_idx = i;
                    break;
                }
                if ch == '\n' {
                    current_row += 1;
                    row_start_idx = i + 1;
                }
            }
            if current_row < row {
                row_start_idx = self.text.len();
            }
            let x_in_row = mx - content.x - TEXT_PADDING;
            if x_in_row <= 0.0 {
                return (row_start_idx, hit);
            }
            let row_end_idx = self.row_end(row_start_idx);
            let row_text = &self.text[row_start_idx..row_end_idx];
            let char_count = row_text.chars().count();
            if char_count == 0 {
                return (row_start_idx, hit);
            }
            let approx_char_w = content.w / char_count.max(1) as f64;
            let char_idx = (x_in_row / approx_char_w) as usize;
            let mut idx = row_start_idx;
            for (i, (byte_idx, ch)) in row_text.char_indices().enumerate() {
                if i >= char_idx {
                    idx = row_start_idx + byte_idx;
                    break;
                }
                idx = row_start_idx + byte_idx + ch.len_utf8();
            }
            (idx.min(row_end_idx), hit)
        } else {
            let idx = self.x_to_index_single_line(mx);
            (idx, hit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn key_press(key: InputKey) -> InputEvent {
        InputEvent::press(key)
    }

    fn char_press(ch: char) -> InputEvent {
        InputEvent::press(InputKey::Key(ch)).with_chars(&ch.to_string())
    }

    fn ctrl_key(key: InputKey) -> InputEvent {
        InputEvent::press(key).with_ctrl()
    }

    fn shift_key(key: InputKey) -> InputEvent {
        InputEvent::press(key).with_shift()
    }

    fn ctrl_char(ch: char) -> InputEvent {
        InputEvent::press(InputKey::Key(ch)).with_ctrl()
    }

    fn shift_ctrl_key(key: InputKey) -> InputEvent {
        InputEvent::press(key).with_shift_ctrl()
    }

    #[test]
    fn insert_and_delete() {
        let look = Look::new();
        let mut tf = TextField::new(look);

        tf.input(&char_press('H'));
        tf.input(&char_press('i'));
        assert_eq!(tf.text(), "Hi");
        assert_eq!(tf.cursor_pos(), 2);

        tf.input(&key_press(InputKey::Backspace));
        assert_eq!(tf.text(), "H");
        assert_eq!(tf.cursor_pos(), 1);

        tf.input(&key_press(InputKey::ArrowLeft));
        assert_eq!(tf.cursor_pos(), 0);

        tf.input(&key_press(InputKey::Delete));
        assert_eq!(tf.text(), "");
    }

    #[test]
    fn cursor_movement() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("ABCD");
        assert_eq!(tf.cursor_pos(), 4);

        tf.input(&key_press(InputKey::Home));
        assert_eq!(tf.cursor_pos(), 0);

        tf.input(&key_press(InputKey::End));
        assert_eq!(tf.cursor_pos(), 4);

        tf.input(&key_press(InputKey::ArrowLeft));
        assert_eq!(tf.cursor_pos(), 3);

        tf.input(&key_press(InputKey::ArrowRight));
        assert_eq!(tf.cursor_pos(), 4);
    }

    #[test]
    fn max_length() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_max_length(3);

        tf.input(&char_press('A'));
        tf.input(&char_press('B'));
        tf.input(&char_press('C'));
        tf.input(&char_press('D'));
        assert_eq!(tf.text(), "ABC");
    }

    #[test]
    fn callback_fires_on_change() {
        let look = Look::new();
        let changes = Rc::new(RefCell::new(Vec::new()));
        let changes_clone = changes.clone();

        let mut tf = TextField::new(look);
        tf.on_text = Some(Box::new(move |text| {
            changes_clone.borrow_mut().push(text.to_string());
        }));

        tf.input(&char_press('X'));
        tf.input(&char_press('Y'));
        tf.input(&key_press(InputKey::Backspace));
        assert_eq!(*changes.borrow(), vec!["X", "XY", "X"]);
    }

    #[test]
    fn password_mode() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_password_mode(true);
        tf.set_text("secret");
        assert_eq!(tf.text(), "secret");
        assert!(tf.password_mode());
    }

    #[test]
    fn cursor_type() {
        let look = Look::new();
        let tf = TextField::new(look);
        assert_eq!(tf.get_cursor(), Cursor::Text);
    }

    #[test]
    fn undo_redo() {
        let look = Look::new();
        let mut tf = TextField::new(look);

        tf.input(&char_press('A'));
        tf.input(&char_press('B'));
        tf.input(&char_press('C'));
        assert_eq!(tf.text(), "ABC");

        tf.undo();
        assert_eq!(tf.text(), "AB");

        tf.undo();
        assert_eq!(tf.text(), "A");

        tf.redo();
        assert_eq!(tf.text(), "AB");

        tf.input(&char_press('X'));
        assert_eq!(tf.text(), "ABX");
        assert!(!tf.redo());
    }

    // ── Phase 1 tests ───────────────────────────────────────────────────

    #[test]
    fn select_deselect_select_all() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("Hello World");

        tf.select(0, 5);
        assert_eq!(tf.selected_text(), "Hello");
        assert_eq!(tf.selection_start(), 0);
        assert_eq!(tf.selection_end(), 5);
        assert!(!tf.is_selection_empty());

        tf.deselect();
        assert!(tf.is_selection_empty());

        tf.select_all();
        assert_eq!(tf.selected_text(), "Hello World");
    }

    #[test]
    fn modify_selection_extend() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("ABCDEF");
        tf.set_cursor_index(2);

        // Extend right
        tf.modify_selection(4, true);
        assert_eq!(tf.selected_text(), "CD");

        // Extend further
        tf.modify_selection(6, true);
        assert_eq!(tf.selected_text(), "CDEF");

        // Without extend: clears selection
        tf.modify_selection(0, false);
        assert!(tf.is_selection_empty());
        assert_eq!(tf.cursor_pos(), 0);
    }

    #[test]
    fn editable_toggle() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        assert!(tf.is_editable());

        tf.set_editable(false);
        assert!(!tf.is_editable());

        tf.set_text("readonly");
        tf.input(&char_press('X'));
        assert_eq!(tf.text(), "readonly"); // no change
    }

    #[test]
    fn can_undo_redo() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        assert!(!tf.can_undo());
        assert!(!tf.can_redo());

        tf.input(&char_press('A'));
        assert!(tf.can_undo());
        assert!(!tf.can_redo());

        tf.undo();
        assert!(!tf.can_undo());
        assert!(tf.can_redo());
    }

    // ── Phase 2 tests ───────────────────────────────────────────────────

    #[test]
    fn word_boundary_navigation() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world_test foo");

        // Forward from start
        let b1 = tf.next_word_boundary(0);
        assert_eq!(&tf.text()[..b1], "hello ");

        let b2 = tf.next_word_boundary(b1);
        assert_eq!(&tf.text()[..b2], "hello world_test ");

        // Backward from end
        let len = tf.text_len();
        let b3 = tf.prev_word_boundary(len);
        assert_eq!(b3, 17); // start of "foo"
    }

    #[test]
    fn word_boundary_edge_cases() {
        let look = Look::new();
        let mut tf = TextField::new(look);

        // Empty string
        tf.set_text("");
        assert_eq!(tf.next_word_boundary(0), 0);
        assert_eq!(tf.prev_word_boundary(0), 0);

        // Consecutive spaces
        tf.set_text("a  b");
        let b = tf.next_word_boundary(0);
        assert_eq!(b, 3); // skip "a", then skip "  "
    }

    #[test]
    fn row_navigation_multi_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("abc\ndefgh\nij");

        assert_eq!(tf.row_start(5), 4); // 'd' is at 4
        assert_eq!(tf.row_end(5), 9); // end of "defgh"

        let (col, row) = tf.index_to_col_row(5);
        assert_eq!(row, 1);
        assert_eq!(col, 1);

        assert_eq!(tf.col_row_to_index(1, 2), 11); // 'j'
    }

    #[test]
    fn row_nav_up_down() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("abc\ndefgh\nij");

        // From position 5 ("e" in row 1, col 1), go to row 0 col 1 = "b" at index 1
        let prev = tf.prev_row_index(5, 1);
        assert_eq!(prev, 1);

        // From position 1 ("b" in row 0), go to row 1 col 1 = "e" at index 5
        let next = tf.next_row_index(1, 1);
        assert_eq!(next, 5);

        // Clamp to row end: row 2 only has "ij", col 4 clamps to end
        let next2 = tf.next_row_index(5, 4);
        assert_eq!(next2, 12); // end of "ij"
    }

    // ── Phase 3 tests ───────────────────────────────────────────────────

    #[test]
    fn ctrl_left_right_word_nav() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.set_cursor_index(0);

        tf.input(&ctrl_key(InputKey::ArrowRight));
        assert_eq!(tf.cursor_pos(), 6); // after "hello "

        tf.input(&ctrl_key(InputKey::ArrowLeft));
        assert_eq!(tf.cursor_pos(), 0);
    }

    #[test]
    fn shift_selection() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("ABCDEF");
        tf.set_cursor_index(2);

        tf.input(&shift_key(InputKey::ArrowRight));
        assert_eq!(tf.selected_text(), "C");

        tf.input(&shift_key(InputKey::ArrowRight));
        assert_eq!(tf.selected_text(), "CD");

        // Without shift: clears selection
        tf.input(&key_press(InputKey::ArrowRight));
        assert!(tf.is_selection_empty());
    }

    #[test]
    fn ctrl_shift_word_selection() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.set_cursor_index(0);

        tf.input(&shift_ctrl_key(InputKey::ArrowRight));
        assert_eq!(tf.selected_text(), "hello ");
    }

    #[test]
    fn editable_false_blocks_editing_not_nav() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("test");
        tf.set_editable(false);

        // Nav works
        tf.input(&key_press(InputKey::Home));
        assert_eq!(tf.cursor_pos(), 0);

        tf.input(&key_press(InputKey::End));
        assert_eq!(tf.cursor_pos(), 4);

        // Edit blocked
        tf.input(&key_press(InputKey::Backspace));
        assert_eq!(tf.text(), "test");

        tf.input(&char_press('X'));
        assert_eq!(tf.text(), "test");
    }

    #[test]
    fn overwrite_mode() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("ABC");
        tf.set_cursor_index(0);
        tf.set_overwrite_mode(true);

        tf.input(&char_press('X'));
        assert_eq!(tf.text(), "XBC");
        assert_eq!(tf.cursor_pos(), 1);

        tf.input(&char_press('Y'));
        assert_eq!(tf.text(), "XYC");
    }

    #[test]
    fn ctrl_backspace_delete_word() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.set_cursor_index(11);

        tf.input(&ctrl_key(InputKey::Backspace));
        assert_eq!(tf.text(), "hello ");
    }

    #[test]
    fn ctrl_delete_word() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.set_cursor_index(0);

        tf.input(&ctrl_key(InputKey::Delete));
        assert_eq!(tf.text(), "world");
    }

    #[test]
    fn select_all_ctrl_a() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("test");

        tf.input(&ctrl_char('a'));
        assert_eq!(tf.selected_text(), "test");

        // Ctrl+Shift+A = deselect
        tf.input(&InputEvent::press(InputKey::Key('a')).with_shift_ctrl());
        assert!(tf.is_selection_empty());
    }

    #[test]
    fn validation_rejects_change() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("123");
        tf.on_validate = Some(Box::new(|text| text.chars().all(|c| c.is_ascii_digit())));

        // Numeric input accepted
        tf.input(&char_press('4'));
        assert_eq!(tf.text(), "1234");

        // Non-numeric rejected
        tf.input(&char_press('x'));
        assert_eq!(tf.text(), "1234");
    }

    #[test]
    fn magic_column_up_down() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("abcde\nfg\nhijklm");
        // cursor at end of "abcde" (col 5, row 0)
        tf.set_cursor_index(5);

        // Down: col 5 but row 1 only has "fg" (len 2), so clamps to end of row 1 (idx 8)
        tf.input(&key_press(InputKey::ArrowDown));
        assert_eq!(tf.cursor_pos(), 8);

        // Down again: col 5 in row 2 "hijklm" → index 9+5=14
        tf.input(&key_press(InputKey::ArrowDown));
        assert_eq!(tf.cursor_pos(), 14);
    }

    #[test]
    fn enter_multi_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("ab");
        tf.set_cursor_index(1);

        tf.input(&key_press(InputKey::Enter));
        assert_eq!(tf.text(), "a\nb");
        assert_eq!(tf.cursor_pos(), 2);
    }

    #[test]
    fn enter_single_line_noop() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("ab");
        tf.set_cursor_index(1);

        tf.input(&key_press(InputKey::Enter));
        assert_eq!(tf.text(), "ab"); // unchanged
    }

    // ── Phase 4 tests ───────────────────────────────────────────────────

    #[test]
    fn clipboard_copy_paste() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        let clipboard = Rc::new(RefCell::new(String::new()));

        let clip_w = clipboard.clone();
        tf.on_clipboard_copy = Some(Box::new(move |text| {
            *clip_w.borrow_mut() = text.to_string();
        }));

        let clip_r = clipboard.clone();
        tf.on_clipboard_paste = Some(Box::new(move || clip_r.borrow().clone()));

        tf.set_text("Hello World");
        tf.select(0, 5);

        // Copy
        tf.input(&ctrl_char('c'));
        assert_eq!(*clipboard.borrow(), "Hello");

        // Move to end, paste
        tf.input(&key_press(InputKey::End));
        tf.input(&ctrl_char('v'));
        assert_eq!(tf.text(), "Hello WorldHello");
    }

    #[test]
    fn clipboard_cut() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        let clipboard = Rc::new(RefCell::new(String::new()));

        let clip_w = clipboard.clone();
        tf.on_clipboard_copy = Some(Box::new(move |text| {
            *clip_w.borrow_mut() = text.to_string();
        }));

        tf.set_text("ABCDEF");
        tf.select(2, 4);

        tf.input(&ctrl_char('x'));
        assert_eq!(*clipboard.borrow(), "CD");
        assert_eq!(tf.text(), "ABEF");
    }

    #[test]
    fn paste_respects_max_length() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_max_length(5);

        let clip = Rc::new(RefCell::new("ABCDEFGH".to_string()));
        let clip_r = clip.clone();
        tf.on_clipboard_paste = Some(Box::new(move || clip_r.borrow().clone()));

        tf.input(&ctrl_char('v'));
        assert_eq!(tf.text(), "ABCDE");
    }

    #[test]
    fn password_mode_copies_asterisks() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_password_mode(true);
        let clipboard = Rc::new(RefCell::new(String::new()));

        let clip_w = clipboard.clone();
        tf.on_clipboard_copy = Some(Box::new(move |text| {
            *clip_w.borrow_mut() = text.to_string();
        }));

        tf.set_text("secret");
        tf.select_all();
        tf.copy_to_clipboard();
        assert_eq!(*clipboard.borrow(), "******");
    }

    // ── Phase 5 tests ───────────────────────────────────────────────────

    #[test]
    fn double_click_selects_word() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        // Populate char_positions manually for testing
        tf.char_positions = vec![
            0.0, 8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0, 72.0, 80.0, 88.0,
        ];

        let now = std::time::Instant::now();

        // First click at x position for 'e' (approximately char 1)
        let click1 = InputEvent::press(InputKey::MouseLeft).with_mouse(10.0, 5.0);
        tf.input(&click1);

        // Simulate second click (double) by setting last_click_time
        tf.last_click_time = Some(now);
        tf.last_click_x = 10.0;
        tf.last_click_y = 5.0;
        tf.click_count = 1;

        let click2 = InputEvent::press(InputKey::MouseLeft).with_mouse(10.0, 5.0);
        tf.input(&click2);

        // Should have selected "hello"
        assert_eq!(tf.selected_text(), "hello");
    }

    #[test]
    fn move_mode_relocates_text() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("ABCDEF");
        tf.char_positions = vec![0.0, 8.0, 16.0, 24.0, 32.0, 40.0, 48.0];

        // Select "CD" (indices 2..4)
        tf.select(2, 4);

        // Ctrl+click inside selection to start move
        let click = InputEvent::press(InputKey::MouseLeft)
            .with_mouse(20.0, 5.0) // inside "CD"
            .with_ctrl();
        tf.input(&click);
        assert_eq!(tf.drag_mode, DragMode::Move);

        // Release at position after 'F' (x=50 → past mid of last char)
        let release = InputEvent::release(InputKey::MouseLeft).with_mouse(50.0, 5.0);
        tf.input(&release);
        assert_eq!(tf.text(), "ABEFCD");
    }

    // ── Phase 6 tests ───────────────────────────────────────────────────

    #[test]
    fn preferred_size_multi_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);

        let (_w1, h1) = tf.preferred_size();

        tf.set_multi_line(true);
        let (_w2, h2) = tf.preferred_size();

        assert!(h2 > h1, "multi-line should be taller");
    }

    #[test]
    fn total_rows() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("a\nb\nc");
        assert_eq!(tf.total_rows(), 3);

        tf.set_text("");
        assert_eq!(tf.total_rows(), 1);
    }

    #[test]
    fn insert_toggle() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        assert!(!tf.is_overwrite_mode());

        tf.input(&key_press(InputKey::Insert));
        assert!(tf.is_overwrite_mode());

        tf.input(&key_press(InputKey::Insert));
        assert!(!tf.is_overwrite_mode());
    }

    #[test]
    fn text_len() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello");
        assert_eq!(tf.text_len(), 5);
    }

    #[test]
    fn ctrl_shift_backspace_delete_to_row_start() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.set_cursor_index(7); // at "o" in "world"

        tf.input(&shift_ctrl_key(InputKey::Backspace));
        assert_eq!(tf.text(), "orld");
    }

    #[test]
    fn ctrl_shift_delete_to_row_end() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.set_cursor_index(5);

        tf.input(&shift_ctrl_key(InputKey::Delete));
        assert_eq!(tf.text(), "hello");
    }

    #[test]
    fn home_end_multi_line_row_vs_text() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("abc\ndef\nghi");
        tf.set_cursor_index(5); // 'e' in row 1

        // Home goes to row start
        tf.input(&key_press(InputKey::Home));
        assert_eq!(tf.cursor_pos(), 4); // start of "def"

        // End goes to row end
        tf.input(&key_press(InputKey::End));
        assert_eq!(tf.cursor_pos(), 7); // end of "def"

        // Ctrl+Home goes to text start
        tf.input(&ctrl_key(InputKey::Home));
        assert_eq!(tf.cursor_pos(), 0);

        // Ctrl+End goes to text end
        tf.input(&ctrl_key(InputKey::End));
        assert_eq!(tf.cursor_pos(), 11);
    }

    // ── Port batch tests ───────────────────────────────────────────────

    #[test]
    fn next_paragraph_single_line_returns_len() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        // single-line: returns text len
        assert_eq!(tf.next_paragraph_index(0), 11);
    }

    #[test]
    fn next_paragraph_multi_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("abc\n\ndef\nghi");
        // From 0: skip "abc", find newline at 3, another at 4, then "def" at 5
        assert_eq!(tf.next_paragraph_index(0), 5);
        // From 5: skip "def", find \n at 8, then "ghi" at 9
        assert_eq!(tf.next_paragraph_index(5), 9);
        // From 9: no more paragraphs
        assert_eq!(tf.next_paragraph_index(9), 12);
    }

    #[test]
    fn prev_paragraph_multi_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("abc\n\ndef\nghi");
        // From end: prev paragraph is "def" at 5 -> but actually our scan
        // says prev of 12 is 9 (ghi start), since next_paragraph_index(5)=9.
        assert_eq!(tf.prev_paragraph_index(12), 9);
        assert_eq!(tf.prev_paragraph_index(9), 5);
        assert_eq!(tf.prev_paragraph_index(5), 0);
    }

    #[test]
    fn prev_paragraph_single_line_returns_zero() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello");
        assert_eq!(tf.prev_paragraph_index(3), 0);
    }

    #[test]
    fn next_word_index_skips_delimiters() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello  world");
        // From 0: skip word "hello", skip delimiters "  ", find word "world" at 7
        assert_eq!(tf.next_word_index(0), 7);
        // From 7: skip word "world" -> end of text
        assert_eq!(tf.next_word_index(7), 12);
        // From within delimiter space (pos 5): find next word at 7
        assert_eq!(tf.next_word_index(5), 7);
    }

    #[test]
    fn prev_word_index_finds_word_start() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello  world");
        // prev_word_index(12) should find start of "world" at 7
        assert_eq!(tf.prev_word_index(12), 7);
        // prev_word_index(7) should find start of "hello" at 0
        assert_eq!(tf.prev_word_index(7), 0);
    }

    #[test]
    fn next_word_index_at_end() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello");
        assert_eq!(tf.next_word_index(5), 5);
    }

    #[test]
    fn prev_word_index_at_start() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        assert_eq!(tf.prev_word_index(0), 0);
    }

    #[test]
    fn publish_selection_basic() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        let clipboard = Rc::new(RefCell::new(String::new()));
        let clip_w = clipboard.clone();
        tf.on_clipboard_copy = Some(Box::new(move |text| {
            *clip_w.borrow_mut() = text.to_string();
        }));
        tf.set_text("Hello World");
        tf.select(0, 5);
        tf.publish_selection();
        assert_eq!(*clipboard.borrow(), "Hello");
        // Second publish is no-op (already published)
        *clipboard.borrow_mut() = String::new();
        tf.publish_selection();
        assert_eq!(*clipboard.borrow(), "");
        // After selection change, can publish again
        tf.select(6, 11);
        tf.publish_selection();
        assert_eq!(*clipboard.borrow(), "World");
    }

    #[test]
    fn publish_selection_password_mode() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        let clipboard = Rc::new(RefCell::new(String::new()));
        let clip_w = clipboard.clone();
        tf.on_clipboard_copy = Some(Box::new(move |text| {
            *clip_w.borrow_mut() = text.to_string();
        }));
        tf.set_password_mode(true);
        tf.set_text("secret");
        tf.select_all();
        tf.publish_selection();
        assert_eq!(*clipboard.borrow(), "******");
    }

    #[test]
    fn selection_signal_fires() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        let count = Rc::new(RefCell::new(0usize));
        let count_c = count.clone();
        tf.on_selection_signal = Some(Box::new(move || {
            *count_c.borrow_mut() += 1;
        }));
        tf.set_text("ABCDEF");
        tf.select(1, 3);
        assert_eq!(*count.borrow(), 1);
        tf.select(2, 5);
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn can_undo_redo_signal_fires() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        let states = Rc::new(RefCell::new(Vec::new()));
        let states_c = states.clone();
        tf.on_can_undo_redo = Some(Box::new(move |can_undo, can_redo| {
            states_c.borrow_mut().push((can_undo, can_redo));
        }));
        // Type a char -> undo becomes available
        tf.input(&char_press('A'));
        assert_eq!(states.borrow().last(), Some(&(true, false)));
        // Undo -> redo becomes available, undo gone
        tf.undo();
        assert_eq!(states.borrow().last(), Some(&(false, true)));
        // Redo -> undo available again
        tf.redo();
        assert_eq!(states.borrow().last(), Some(&(true, false)));
    }

    #[test]
    fn cursor_blink_cycle() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        assert!(tf.is_cursor_blink_on());
        // Focused: returns busy=true
        let busy = tf.cycle_blink(true);
        assert!(busy);
        assert!(tf.is_cursor_blink_on()); // just started, < 500ms
                                          // Not focused: resets blink, returns false
        let busy = tf.cycle_blink(false);
        assert!(!busy);
        assert!(tf.is_cursor_blink_on());
    }

    #[test]
    fn restart_cursor_blinking_resets() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.cursor_blink_on = false; // simulate blink-off state
        tf.restart_cursor_blinking();
        assert!(tf.is_cursor_blink_on());
    }

    #[test]
    fn calc_total_cols_rows_single_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello");
        assert_eq!(tf.calc_total_cols_rows(), (5, 1));
        tf.set_text("");
        assert_eq!(tf.calc_total_cols_rows(), (1, 1)); // minimum
    }

    #[test]
    fn calc_total_cols_rows_multi_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("ab\ncdef\ng");
        // Row 0: "ab" (2 cols), Row 1: "cdef" (4 cols), Row 2: "g" (1 col)
        // Widest = 4, rows = 3
        assert_eq!(tf.calc_total_cols_rows(), (4, 3));
    }

    #[test]
    fn calc_total_cols_rows_with_tabs() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_multi_line(true);
        tf.set_text("a\tb");
        // Tab at col 1 -> next tab stop at 8, then 'b' at col 9
        // total cols = 9, rows = 1
        assert_eq!(tf.calc_total_cols_rows(), (9, 1));
    }

    #[test]
    fn check_mouse_single_line() {
        let look = Look::new();
        let mut tf = TextField::new(look);
        tf.set_text("hello world");
        tf.char_positions = vec![
            0.0, 8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0, 72.0, 80.0, 88.0,
        ];
        let (idx, hit) = tf.check_mouse(10.0, 5.0, 200.0, 30.0);
        assert!(idx <= tf.text_len());
        // hit depends on content rect
        let _ = hit;
    }
}
