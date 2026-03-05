use std::rc::Rc;

use crate::input::{Cursor, InputEvent, InputKey, InputVariant};
use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::look::Look;

const CHAR_WIDTH: f64 = 6.0; // GLYPH_WIDTH + 1
const TEXT_PADDING: f64 = 2.0;

type TextChangeCb = Box<dyn FnMut(&str)>;

/// Single-line text input widget.
pub struct TextField {
    border: Border,
    look: Rc<Look>,
    text: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    scroll_x: f64,
    password_mode: bool,
    max_length: usize,
    pub on_text: Option<TextChangeCb>,
}

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
            on_text: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.text.len();
        self.selection_anchor = None;
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor
    }

    pub fn set_password_mode(&mut self, enabled: bool) {
        self.password_mode = enabled;
    }

    pub fn set_max_length(&mut self, max: usize) {
        self.max_length = max;
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border.paint_border(painter, w, h, &self.look, false);

        let (cx, cy, cw, ch) = self.border.content_rect(w, h, &self.look);

        painter.push_state();
        painter.clip_rect(cx, cy, cw, ch);

        let display_text = if self.password_mode {
            "*".repeat(self.text.chars().count())
        } else {
            self.text.clone()
        };

        // Selection highlight
        if let Some(anchor) = self.selection_anchor {
            let sel_start = anchor.min(self.cursor);
            let sel_end = anchor.max(self.cursor);
            let start_chars = self.text[..sel_start].chars().count();
            let end_chars = self.text[..sel_end].chars().count();
            let sx = cx + TEXT_PADDING + start_chars as f64 * CHAR_WIDTH - self.scroll_x;
            let sw = (end_chars - start_chars) as f64 * CHAR_WIDTH;
            painter.paint_rect(sx, cy, sw, ch, self.look.selection_color);
        }

        // Text
        let text_x = cx + TEXT_PADDING - self.scroll_x;
        let text_y = cy + (ch - FontCache::GLYPH_HEIGHT as f64) / 2.0;
        painter.paint_text(text_x, text_y, &display_text, self.look.fg_color);

        // Cursor line
        let cursor_chars = self.text[..self.cursor].chars().count();
        let cursor_x = cx + TEXT_PADDING + cursor_chars as f64 * CHAR_WIDTH - self.scroll_x;
        painter.paint_rect(cursor_x, cy + 1.0, 1.0, ch - 2.0, self.look.cursor_color);

        painter.pop_state();
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        match event.variant {
            InputVariant::Press | InputVariant::Repeat => {}
            InputVariant::Release | InputVariant::Move => return false,
        }

        match event.key {
            InputKey::ArrowLeft => {
                if self.cursor > 0 {
                    self.cursor = self.prev_char_boundary(self.cursor);
                }
                self.selection_anchor = None;
                true
            }
            InputKey::ArrowRight => {
                if self.cursor < self.text.len() {
                    self.cursor = self.next_char_boundary(self.cursor);
                }
                self.selection_anchor = None;
                true
            }
            InputKey::Home => {
                self.cursor = 0;
                self.selection_anchor = None;
                true
            }
            InputKey::End => {
                self.cursor = self.text.len();
                self.selection_anchor = None;
                true
            }
            InputKey::Backspace => {
                if self.delete_selection() {
                    self.fire_change();
                    return true;
                }
                if self.cursor > 0 {
                    let prev = self.prev_char_boundary(self.cursor);
                    self.text.drain(prev..self.cursor);
                    self.cursor = prev;
                    self.fire_change();
                }
                true
            }
            InputKey::Delete => {
                if self.delete_selection() {
                    self.fire_change();
                    return true;
                }
                if self.cursor < self.text.len() {
                    let next = self.next_char_boundary(self.cursor);
                    self.text.drain(self.cursor..next);
                    self.fire_change();
                }
                true
            }
            _ => {
                if !event.chars.is_empty() {
                    self.delete_selection();
                    for ch in event.chars.chars() {
                        if ch.is_control() {
                            continue;
                        }
                        if self.text.chars().count() >= self.max_length {
                            break;
                        }
                        self.text.insert(self.cursor, ch);
                        self.cursor += ch.len_utf8();
                    }
                    self.fire_change();
                    return true;
                }
                false
            }
        }
    }

    pub fn get_cursor(&self) -> Cursor {
        Cursor::Text
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let cw = 120.0; // default width
        let ch = FontCache::GLYPH_HEIGHT as f64 + 4.0;
        self.border.preferred_size_for_content(cw, ch)
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

    fn delete_selection(&mut self) -> bool {
        if let Some(anchor) = self.selection_anchor.take() {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            if start != end {
                self.text.drain(start..end);
                self.cursor = start;
                return true;
            }
        }
        false
    }

    fn fire_change(&mut self) {
        if let Some(cb) = &mut self.on_text {
            cb(&self.text);
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
        // Internal state preserved
        assert_eq!(tf.text(), "secret");
    }

    #[test]
    fn cursor_type() {
        let look = Look::new();
        let tf = TextField::new(look);
        assert_eq!(tf.get_cursor(), Cursor::Text);
    }
}
