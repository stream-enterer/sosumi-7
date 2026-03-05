use std::rc::Rc;

use crate::input::{InputEvent, InputKey, InputVariant};
use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, InnerBorderType, OuterBorderType};
use super::look::Look;

const ROW_HEIGHT: f64 = 11.0;

type SelectionCb = Box<dyn FnMut(&[usize])>;

/// Selection mode for list box items.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    Single,
    Multi,
    None,
}

/// Selectable item list widget.
pub struct ListBox {
    border: Border,
    look: Rc<Look>,
    items: Vec<String>,
    selected: Vec<usize>,
    focus_index: usize,
    scroll_y: f64,
    selection_mode: SelectionMode,
    pub on_selection: Option<SelectionCb>,
    pub on_trigger: Option<Box<dyn FnMut(usize)>>,
}

impl ListBox {
    pub fn new(look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::Rect).with_inner(InnerBorderType::InputField),
            look,
            items: Vec::new(),
            selected: Vec::new(),
            focus_index: 0,
            scroll_y: 0.0,
            selection_mode: SelectionMode::Single,
            on_selection: None,
            on_trigger: None,
        }
    }

    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection_mode = mode;
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.selected.clear();
        self.focus_index = 0;
        self.scroll_y = 0.0;
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn selected(&self) -> &[usize] {
        &self.selected
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border.paint_border(painter, w, h, &self.look, false);

        let (cx, cy, cw, ch) = self.border.content_rect(w, h, &self.look);

        painter.push_state();
        painter.clip_rect(cx, cy, cw, ch);

        for (i, item) in self.items.iter().enumerate() {
            let y = cy + i as f64 * ROW_HEIGHT - self.scroll_y;
            if y + ROW_HEIGHT < cy || y > cy + ch {
                continue;
            }

            if self.selected.contains(&i) {
                painter.paint_rect(cx, y, cw, ROW_HEIGHT, self.look.selection_color);
            }

            let text_y = y + 2.0;
            painter.paint_text(cx + 2.0, text_y, item, self.look.fg_color);
        }

        painter.pop_state();
    }

    pub fn input(&mut self, event: &InputEvent) -> bool {
        if self.items.is_empty() {
            return false;
        }

        match event.key {
            InputKey::ArrowDown if event.variant == InputVariant::Press => {
                if self.focus_index + 1 < self.items.len() {
                    self.focus_index += 1;
                    if self.selection_mode == SelectionMode::Single {
                        self.selected = vec![self.focus_index];
                        self.fire_selection();
                    }
                }
                true
            }
            InputKey::ArrowUp if event.variant == InputVariant::Press => {
                if self.focus_index > 0 {
                    self.focus_index -= 1;
                    if self.selection_mode == SelectionMode::Single {
                        self.selected = vec![self.focus_index];
                        self.fire_selection();
                    }
                }
                true
            }
            InputKey::Space if event.variant == InputVariant::Press => {
                if self.selection_mode == SelectionMode::Multi {
                    let idx = self.focus_index;
                    if let Some(pos) = self.selected.iter().position(|&s| s == idx) {
                        self.selected.remove(pos);
                    } else {
                        self.selected.push(idx);
                    }
                    self.fire_selection();
                }
                true
            }
            InputKey::Enter if event.variant == InputVariant::Press => {
                if let Some(cb) = &mut self.on_trigger {
                    cb(self.focus_index);
                }
                true
            }
            InputKey::MouseLeft if event.variant == InputVariant::Press => {
                let (_, cy, _, _ch) = self.border.content_rect(0.0, 0.0, &self.look);
                let rel_y = event.mouse_y - cy + self.scroll_y;
                let clicked_idx = (rel_y / ROW_HEIGHT) as usize;
                if clicked_idx < self.items.len() {
                    self.focus_index = clicked_idx;
                    match self.selection_mode {
                        SelectionMode::Single => {
                            self.selected = vec![clicked_idx];
                            self.fire_selection();
                        }
                        SelectionMode::Multi => {
                            if let Some(pos) = self.selected.iter().position(|&s| s == clicked_idx)
                            {
                                self.selected.remove(pos);
                            } else {
                                self.selected.push(clicked_idx);
                            }
                            self.fire_selection();
                        }
                        SelectionMode::None => {}
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let max_w = self
            .items
            .iter()
            .map(|s| FontCache::measure_text(s) as f64)
            .fold(0.0f64, f64::max);
        let h = self.items.len() as f64 * ROW_HEIGHT;
        self.border.preferred_size_for_content(max_w + 4.0, h)
    }

    fn fire_selection(&mut self) {
        if let Some(cb) = &mut self.on_selection {
            cb(&self.selected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn single_selection_arrow_keys() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_items(vec!["A".into(), "B".into(), "C".into()]);

        assert_eq!(lb.focus_index(), 0);

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 1);
        assert_eq!(lb.selected(), &[1]);

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 2);
        assert_eq!(lb.selected(), &[2]);

        // Won't go past end
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 2);

        lb.input(&InputEvent::press(InputKey::ArrowUp));
        assert_eq!(lb.focus_index(), 1);
        assert_eq!(lb.selected(), &[1]);
    }

    #[test]
    fn multi_selection_toggle() {
        let look = Look::new();
        let mut lb = ListBox::new(look);
        lb.set_selection_mode(SelectionMode::Multi);
        lb.set_items(vec!["X".into(), "Y".into(), "Z".into()]);

        // In multi mode, ArrowDown doesn't auto-select
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(lb.focus_index(), 1);
        assert!(lb.selected().is_empty());

        // Space toggles selection
        lb.input(&InputEvent::press(InputKey::Space));
        assert_eq!(lb.selected(), &[1]);

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        lb.input(&InputEvent::press(InputKey::Space));
        assert_eq!(lb.selected(), &[1, 2]);

        // Toggle off
        lb.focus_index = 1;
        lb.input(&InputEvent::press(InputKey::Space));
        assert_eq!(lb.selected(), &[2]);
    }

    #[test]
    fn trigger_callback() {
        let look = Look::new();
        let triggered = Rc::new(RefCell::new(None));
        let trig_clone = triggered.clone();

        let mut lb = ListBox::new(look);
        lb.set_items(vec!["A".into(), "B".into()]);
        lb.on_trigger = Some(Box::new(move |idx| {
            *trig_clone.borrow_mut() = Some(idx);
        }));

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        lb.input(&InputEvent::press(InputKey::Enter));
        assert_eq!(*triggered.borrow(), Some(1));
    }

    #[test]
    fn selection_callback() {
        let look = Look::new();
        let selections = Rc::new(RefCell::new(Vec::new()));
        let sel_clone = selections.clone();

        let mut lb = ListBox::new(look);
        lb.set_items(vec!["A".into(), "B".into(), "C".into()]);
        lb.on_selection = Some(Box::new(move |sel| {
            sel_clone.borrow_mut().push(sel.to_vec());
        }));

        lb.input(&InputEvent::press(InputKey::ArrowDown));
        lb.input(&InputEvent::press(InputKey::ArrowDown));
        assert_eq!(selections.borrow().len(), 2);
        assert_eq!(selections.borrow()[1], vec![2]);
    }
}
