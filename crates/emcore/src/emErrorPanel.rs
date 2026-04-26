use crate::emColor::emColor;
use crate::emPainter::{emPainter, TextAlignment, VAlign};
use crate::emPanel::{PanelBehavior, PanelState};

/// A red panel that displays an error message with yellow text.
///
/// Port of C++ `emErrorPanel`. Used to show error conditions with a
/// distinctive visual style (dark red background, yellow text).
pub struct emErrorPanel {
    error_message: String,
}

const BG_COLOR: emColor = emColor::rgb(128, 0, 0);
const FG_COLOR: emColor = emColor::rgb(255, 255, 0);

impl emErrorPanel {
    pub fn new(error_message: &str) -> Self {
        Self {
            error_message: error_message.to_string(),
        }
    }

    pub fn error_message(&self) -> &str {
        &self.error_message
    }

    pub fn set_error_message(&mut self, message: &str) {
        self.error_message = message.to_string();
    }
}

impl PanelBehavior for emErrorPanel {
    // DIVERGED: (language-forced) IsOpaque — implemented as PanelBehavior::is_opaque trait method
    fn IsOpaque(&self) -> bool {
        true
    }

    fn GetCanvasColor(&self) -> emColor {
        BG_COLOR
    }

    // DIVERGED: (language-forced) Paint — implemented as PanelBehavior::paint trait method
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        painter.PaintRect(0.0, 0.0, w, h, BG_COLOR, canvas_color);

        if self.error_message.is_empty() {
            return;
        }

        painter.PaintTextBoxed(
            0.05 * w,
            0.05 * h,
            0.9 * w,
            0.9 * h,
            &self.error_message,
            h / 2.0,
            FG_COLOR,
            BG_COLOR,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Left,
            1.0,
            false,
            0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_panel_message() {
        let mut panel = emErrorPanel::new("Something went wrong");
        assert_eq!(panel.error_message(), "Something went wrong");
        panel.set_error_message("New error");
        assert_eq!(panel.error_message(), "New error");
    }

    #[test]
    fn error_panel_is_opaque() {
        let panel = emErrorPanel::new("error");
        assert!(panel.IsOpaque());
    }

    #[test]
    fn error_panel_canvas_color() {
        let panel = emErrorPanel::new("error");
        assert_eq!(panel.GetCanvasColor(), BG_COLOR);
    }
}
