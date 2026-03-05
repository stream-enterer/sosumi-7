use std::rc::Rc;

use crate::render::font_cache::FontCache;
use crate::render::Painter;

use super::border::{Border, OuterBorderType};
use super::look::Look;

/// Non-focusable text display widget.
pub struct Label {
    border: Border,
    look: Rc<Look>,
}

impl Label {
    pub fn new(caption: &str, look: Rc<Look>) -> Self {
        Self {
            border: Border::new(OuterBorderType::None).with_caption(caption),
            look,
        }
    }

    pub fn set_caption(&mut self, text: &str) {
        self.border.caption = text.to_string();
    }

    pub fn caption(&self) -> &str {
        &self.border.caption
    }

    pub fn paint(&self, painter: &mut Painter, w: f64, h: f64) {
        self.border.paint_border(painter, w, h, &self.look, false);
    }

    pub fn preferred_size(&self) -> (f64, f64) {
        let tw = FontCache::measure_text(&self.border.caption).0 as f64;
        // Content is empty — the caption IS the label text, drawn by the border.
        // Add 4px to width for the 2px left/right padding the border uses when
        // painting the caption text (at ox + 2.0).
        self.border.preferred_size_for_content(tw + 4.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_caption() {
        let look = Look::new();
        let mut label = Label::new("Hello", look);
        assert_eq!(label.caption(), "Hello");
        label.set_caption("World");
        assert_eq!(label.caption(), "World");
    }

    #[test]
    fn label_preferred_size() {
        let look = Look::new();
        let label = Label::new("Test", look);
        let (w, h) = label.preferred_size();
        // "Test" = 4 chars * 6 - 1 = 23 pixels wide + 4px caption padding
        // height = 11 (caption area only, no content)
        assert_eq!(w, 27.0);
        assert_eq!(h, 11.0);
    }
}
