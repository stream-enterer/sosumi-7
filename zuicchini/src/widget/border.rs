use crate::foundation::Rect;
use crate::render::font_cache::FontCache;
use crate::render::{Painter, Stroke};

use super::look::Look;

/// Height allocated for caption and description text, derived from font metrics.
/// Uses glyph height + 4px padding.
const TEXT_ROW_HEIGHT: f64 = FontCache::DEFAULT_SIZE_PX + 4.0;

/// Outer border style.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OuterBorderType {
    None,
    Filled,
    Margin,
    MarginFilled,
    Rect,
    RoundRect,
    Group,
    Instrument,
    InstrumentMoreRound,
    PopupRoot,
}

/// Inner border style.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InnerBorderType {
    None,
    Group,
    InputField,
    OutputField,
    CustomRect,
}

/// Border chrome helper. Embedded in widgets to draw surrounding decoration.
pub struct Border {
    pub outer: OuterBorderType,
    pub inner: InnerBorderType,
    pub caption: String,
    pub description: String,
}

impl Border {
    pub fn new(outer: OuterBorderType) -> Self {
        Self {
            outer,
            inner: InnerBorderType::None,
            caption: String::new(),
            description: String::new(),
        }
    }

    pub fn with_caption(mut self, caption: &str) -> Self {
        self.caption = caption.to_string();
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn with_inner(mut self, inner: InnerBorderType) -> Self {
        self.inner = inner;
        self
    }

    /// Compute the content area after border and label insets.
    pub fn content_rect(&self, w: f64, h: f64, _look: &Look) -> Rect {
        let (ox, oy, ow, oh) = self.outer_insets();
        let caption_h = if self.caption.is_empty() {
            0.0
        } else {
            TEXT_ROW_HEIGHT
        };
        let desc_h = if self.description.is_empty() {
            0.0
        } else {
            TEXT_ROW_HEIGHT
        };
        let (ix, iy, iw, ih) = self.inner_insets();

        Rect {
            x: ox + ix,
            y: oy + caption_h + iy,
            w: (w - ow - iw).max(0.0),
            h: (h - oh - caption_h - desc_h - ih).max(0.0),
        }
    }

    /// Preferred size to fit the given content size.
    pub fn preferred_size_for_content(&self, cw: f64, ch: f64) -> (f64, f64) {
        let (_, _, ow, oh) = self.outer_insets();
        let caption_h = if self.caption.is_empty() {
            0.0
        } else {
            TEXT_ROW_HEIGHT
        };
        let desc_h = if self.description.is_empty() {
            0.0
        } else {
            TEXT_ROW_HEIGHT
        };
        let (_, _, iw, ih) = self.inner_insets();
        (cw + ow + iw, ch + oh + caption_h + desc_h + ih)
    }

    /// Minimum size to fit any content.
    pub fn min_size_for_content(&self, min_cw: f64, min_ch: f64) -> (f64, f64) {
        self.preferred_size_for_content(min_cw, min_ch)
    }

    /// Paint the border chrome.
    pub fn paint_border(&self, painter: &mut Painter, w: f64, h: f64, look: &Look, focused: bool) {
        // Outer border
        match self.outer {
            OuterBorderType::None => {}
            OuterBorderType::Filled => {
                painter.paint_rect(0.0, 0.0, w, h, look.bg_color);
            }
            OuterBorderType::Margin => {}
            OuterBorderType::MarginFilled => {
                painter.paint_rect(2.0, 2.0, w - 4.0, h - 4.0, look.bg_color);
            }
            OuterBorderType::Rect => {
                let color = if focused {
                    look.focus_tint()
                } else {
                    look.border_tint()
                };
                painter.paint_rect_outlined(0.0, 0.0, w, h, &Stroke::new(color, 1.0));
            }
            OuterBorderType::RoundRect => {
                let color = if focused {
                    look.focus_tint()
                } else {
                    look.border_tint()
                };
                painter.paint_round_rect(0.0, 0.0, w, h, 3.0, look.bg_color);
                painter.paint_rect_outlined(0.0, 0.0, w, h, &Stroke::new(color, 1.0));
            }
            OuterBorderType::Group => {
                painter.paint_rect_outlined(0.0, 0.0, w, h, &Stroke::new(look.border_tint(), 1.0));
            }
            OuterBorderType::Instrument => {
                painter.paint_round_rect(0.0, 0.0, w, h, 4.0, look.bg_color);
                let color = if focused {
                    look.focus_tint()
                } else {
                    look.border_tint()
                };
                painter.paint_rect_outlined(0.0, 0.0, w, h, &Stroke::new(color, 1.0));
            }
            OuterBorderType::InstrumentMoreRound => {
                painter.paint_round_rect(0.0, 0.0, w, h, 6.0, look.bg_color);
                let color = if focused {
                    look.focus_tint()
                } else {
                    look.border_tint()
                };
                painter.paint_rect_outlined(0.0, 0.0, w, h, &Stroke::new(color, 1.0));
            }
            OuterBorderType::PopupRoot => {
                painter.paint_rect(0.0, 0.0, w, h, look.bg_color);
                painter.paint_rect_outlined(0.0, 0.0, w, h, &Stroke::new(look.border_tint(), 2.0));
            }
        }

        // Inner border
        let (ox, oy, _, _) = self.outer_insets();
        let caption_h = if self.caption.is_empty() {
            0.0
        } else {
            TEXT_ROW_HEIGHT
        };
        let ix = ox;
        let iy = oy + caption_h;
        let iw = w - ox * 2.0;
        let ih = h
            - oy * 2.0
            - caption_h
            - if self.description.is_empty() {
                0.0
            } else {
                TEXT_ROW_HEIGHT
            };

        match self.inner {
            InnerBorderType::None => {}
            InnerBorderType::Group => {
                painter.paint_rect_outlined(ix, iy, iw, ih, &Stroke::new(look.border_tint(), 1.0));
            }
            InnerBorderType::InputField => {
                painter.paint_rect(ix, iy, iw, ih, look.input_bg_color);
                painter.paint_rect_outlined(ix, iy, iw, ih, &Stroke::new(look.border_tint(), 1.0));
            }
            InnerBorderType::OutputField => {
                painter.paint_rect(ix, iy, iw, ih, look.output_bg_color);
                painter.paint_rect_outlined(ix, iy, iw, ih, &Stroke::new(look.border_tint(), 1.0));
            }
            InnerBorderType::CustomRect => {
                painter.paint_rect_outlined(ix, iy, iw, ih, &Stroke::new(look.border_tint(), 1.0));
            }
        }

        // Caption
        if !self.caption.is_empty() {
            painter.paint_text(
                ox + 2.0,
                oy + 2.0,
                &self.caption,
                FontCache::DEFAULT_SIZE_PX,
                look.fg_color,
            );
        }

        // Description
        if !self.description.is_empty() {
            let desc_y = h - oy - 9.0;
            painter.paint_text(
                ox + 2.0,
                desc_y,
                &self.description,
                FontCache::DEFAULT_SIZE_PX,
                look.disabled_fg(),
            );
        }
    }

    fn outer_insets(&self) -> (f64, f64, f64, f64) {
        match self.outer {
            OuterBorderType::None => (0.0, 0.0, 0.0, 0.0),
            OuterBorderType::Filled => (0.0, 0.0, 0.0, 0.0),
            OuterBorderType::Margin => (2.0, 2.0, 4.0, 4.0),
            OuterBorderType::MarginFilled => (2.0, 2.0, 4.0, 4.0),
            OuterBorderType::Rect => (1.0, 1.0, 2.0, 2.0),
            OuterBorderType::RoundRect => (2.0, 2.0, 4.0, 4.0),
            OuterBorderType::Group => (1.0, 1.0, 2.0, 2.0),
            OuterBorderType::Instrument => (3.0, 3.0, 6.0, 6.0),
            OuterBorderType::InstrumentMoreRound => (4.0, 4.0, 8.0, 8.0),
            OuterBorderType::PopupRoot => (3.0, 3.0, 6.0, 6.0),
        }
    }

    fn inner_insets(&self) -> (f64, f64, f64, f64) {
        match self.inner {
            InnerBorderType::None => (0.0, 0.0, 0.0, 0.0),
            InnerBorderType::Group => (1.0, 1.0, 2.0, 2.0),
            InnerBorderType::InputField => (2.0, 2.0, 4.0, 4.0),
            InnerBorderType::OutputField => (2.0, 2.0, 4.0, 4.0),
            InnerBorderType::CustomRect => (1.0, 1.0, 2.0, 2.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_look() -> Look {
        Look::default()
    }

    #[test]
    fn content_rect_none_border() {
        let border = Border::new(OuterBorderType::None);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        assert_eq!((x, y), (0.0, 0.0));
        assert_eq!((cw, ch), (100.0, 50.0));
    }

    #[test]
    fn content_rect_rect_border() {
        let border = Border::new(OuterBorderType::Rect);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        assert_eq!((x, y), (1.0, 1.0));
        assert_eq!((cw, ch), (98.0, 48.0));
    }

    #[test]
    fn content_rect_with_caption() {
        let border = Border::new(OuterBorderType::Rect).with_caption("Test");
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        assert_eq!(x, 1.0);
        assert_eq!(y, 1.0 + TEXT_ROW_HEIGHT); // outer + caption
        assert_eq!(cw, 98.0);
        assert_eq!(ch, 50.0 - 2.0 - TEXT_ROW_HEIGHT); // total - outer - caption
    }

    #[test]
    fn content_rect_with_inner_input_field() {
        let border = Border::new(OuterBorderType::None).with_inner(InnerBorderType::InputField);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 50.0, &test_look());
        assert_eq!((x, y), (2.0, 2.0));
        assert_eq!((cw, ch), (96.0, 46.0));
    }

    #[test]
    fn content_rect_instrument_with_caption_and_inner() {
        let border = Border::new(OuterBorderType::Instrument)
            .with_caption("Cap")
            .with_inner(InnerBorderType::InputField);
        let Rect { x, y, w: cw, h: ch } = border.content_rect(100.0, 80.0, &test_look());
        assert_eq!(x, 3.0 + 2.0); // outer + inner
        assert_eq!(y, 3.0 + TEXT_ROW_HEIGHT + 2.0); // outer + caption + inner
        assert_eq!(cw, 100.0 - 6.0 - 4.0); // w - outer_w - inner_w
        assert_eq!(ch, 80.0 - 6.0 - TEXT_ROW_HEIGHT - 4.0); // h - outer_h - caption - inner_h
    }

    #[test]
    fn preferred_size_round_trips() {
        let border = Border::new(OuterBorderType::RoundRect)
            .with_caption("Title")
            .with_inner(InnerBorderType::Group);
        let (pw, ph) = border.preferred_size_for_content(50.0, 30.0);
        let Rect { w: cw, h: ch, .. } = border.content_rect(pw, ph, &test_look());
        assert!((cw - 50.0).abs() < 0.01);
        assert!((ch - 30.0).abs() < 0.01);
    }
}
