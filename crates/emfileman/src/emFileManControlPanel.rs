//! Sort/filter/theme UI control panel.
//!
//! Port of C++ `emFileManControlPanel`. Extends `emLinearLayout`.
//! Contains sort criterion radio buttons, name sorting style radio buttons,
//! directories-first and show-hidden checkboxes, theme selectors,
//! autosave checkbox, and command group buttons.
//!
//! DIVERGED: Full widget construction deferred. This version paints a
//! placeholder label. The control panel requires emLinearLayout, emPackGroup,
//! emRasterLayout, emRadioButton, emCheckButton, emButton widget integration
//! which will be ported as a follow-up.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emPainter::{emPainter, TextAlignment, VAlign};

use crate::emFileManViewConfig::emFileManViewConfig;

/// Control panel for file manager settings.
/// Port of C++ `emFileManControlPanel` (extends emLinearLayout).
pub struct emFileManControlPanel {
    _config: Rc<RefCell<emFileManViewConfig>>,
}

impl emFileManControlPanel {
    pub fn new(ctx: Rc<emContext>) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        Self { _config: config }
    }
}

impl PanelBehavior for emFileManControlPanel {
    fn IsOpaque(&self) -> bool {
        false
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        let fg = emColor::from_packed(0xCCCCCCFF);
        let bg = emColor::TRANSPARENT;
        painter.PaintTextBoxed(
            0.02, 0.02, w - 0.04, h - 0.04,
            "File Manager Control Panel\n(widget construction pending)",
            h * 0.1,
            fg, bg,
            TextAlignment::Center, VAlign::Center,
            TextAlignment::Center, 1.0, false, 1.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emFileManControlPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }
}
