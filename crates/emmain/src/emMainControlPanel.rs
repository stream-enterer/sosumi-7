// Port of C++ emMain/emMainControlPanel
// Sidebar panel containing window control buttons and bookmarks.
//
// DIVERGED: C++ emMainControlPanel extends emLinearGroup and builds a deep
// widget tree (emButton, emCheckButton, emLinearGroup, emPackGroup, etc.).
// Rust uses a simplified flat panel with manual vertical layout since the full
// toolkit widget hierarchy is not yet ported.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emPainter::emPainter;
use emcore::emPanelCtx::PanelCtx;
use emcore::emPanelTree::PanelId;

use crate::emBookmarks::emBookmarksPanel;
use crate::emMainConfig::emMainConfig;

// ── ControlButton ─────────────────────────────────────────────────────────────

/// Simple labeled button stub.
///
/// DIVERGED: C++ uses `emButton` / `emCheckButton` widgets from emToolkit.
/// Rust uses this placeholder until the full toolkit is ported.
pub(crate) struct ControlButton {
    pub(crate) label: String,
}

impl PanelBehavior for ControlButton {
    fn get_title(&self) -> Option<String> {
        Some(self.label.clone())
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        // Button background: emLook::GetButtonBgColor (C++ default 0x596790FF).
        let bg = emColor::from_packed(0x596790FF);
        // Button foreground: emLook::GetButtonFgColor (C++ default 0xF2F2F7FF).
        let fg = emColor::from_packed(0xF2F2F7FF);
        let canvas = emColor::TRANSPARENT;

        painter.PaintRect(0.0, 0.0, w, h, bg, canvas);

        let font_h = (h * 0.45).max(0.01);
        let text_y = h * 0.5 - font_h * 0.5;
        painter.PaintText(w * 0.05, text_y, &self.label, font_h, 1.0, fg, canvas);
    }
}

// ── emMainControlPanel ────────────────────────────────────────────────────────

/// Sidebar panel with window control buttons and bookmarks.
///
/// Port of C++ `emMainControlPanel` (extends `emLinearGroup`).
/// DIVERGED: C++ uses emLinearGroup layout with nested emLinearLayout /
/// emPackGroup widget trees. Rust uses a simplified flat panel with manual
/// vertical layout since the full toolkit widget hierarchy is not yet ported.
pub struct emMainControlPanel {
    ctx: Rc<emContext>,
    _config: Rc<RefCell<emMainConfig>>,
    bookmarks_panel: Option<PanelId>,
    button_panels: Vec<PanelId>,
    children_created: bool,
}

/// Button labels matching C++ `grCommands` pack group (emMainControlPanel.cpp).
const BUTTON_LABELS: &[&str] = &[
    "New Window",
    "Fullscreen",
    "Reload Files",
    "Close",
    "Quit",
];

/// Relative height weights for vertical layout.
/// Buttons each get weight 1.0; bookmarks panel gets weight 6.5 to match the
/// C++ child-weight ratio (lMain->SetChildWeight(1,6.5)).
const BUTTON_WEIGHT: f64 = 1.0;
const BOOKMARKS_WEIGHT: f64 = 6.5;

impl emMainControlPanel {
    /// Port of C++ `emMainControlPanel` constructor.
    pub fn new(ctx: Rc<emContext>) -> Self {
        let config = emMainConfig::Acquire(&ctx);
        Self {
            ctx,
            _config: config,
            bookmarks_panel: None,
            button_panels: Vec::new(),
            children_created: false,
        }
    }

    fn create_children(&mut self, ctx: &mut PanelCtx) {
        // Create a ControlButton child for each command.
        for (i, &label) in BUTTON_LABELS.iter().enumerate() {
            let name = format!("btn_{i}");
            let btn = Box::new(ControlButton {
                label: label.to_string(),
            });
            let id = ctx.create_child_with(&name, btn);
            self.button_panels.push(id);
        }

        // Create the bookmarks panel as the last child.
        let bookmarks = Box::new(emBookmarksPanel::new(Rc::clone(&self.ctx)));
        let bm_id = ctx.create_child_with("bookmarks", bookmarks);
        self.bookmarks_panel = Some(bm_id);

        self.children_created = true;
    }
}

impl PanelBehavior for emMainControlPanel {
    /// Port of C++ `emMainControlPanel::GetTitle`.
    fn get_title(&self) -> Option<String> {
        Some("Eagle Mode".to_string())
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        // Sidebar background: emLook::GetBgColor (C++ default 0x515E84FF).
        let bg = emColor::from_packed(0x515E84FF);
        let canvas = emColor::TRANSPARENT;
        painter.PaintRect(0.0, 0.0, w, h, bg, canvas);
    }

    fn Cycle(&mut self, _ctx: &mut PanelCtx) -> bool {
        // No config-change watching needed for the initial port.
        false
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // Create children lazily on the first layout pass.
        if !self.children_created {
            self.create_children(ctx);
        }

        let n_buttons = BUTTON_LABELS.len() as f64;
        let total_weight = n_buttons * BUTTON_WEIGHT + BOOKMARKS_WEIGHT;
        // Thin horizontal padding: 1% of width.
        let pad_x = 0.01_f64;
        let child_w = (1.0 - 2.0 * pad_x).max(0.0);
        // Vertical gap between children: 0.5% of height.
        let gap_frac = 0.005_f64;
        let total_gaps = (n_buttons as usize + 1) as f64 * gap_frac;
        let usable_h = (1.0 - total_gaps).max(0.0);

        let canvas = emColor::from_packed(0x515E84FF);

        let mut y = gap_frac;
        for (i, &id) in self.button_panels.iter().enumerate() {
            let _ = i;
            let ch = usable_h * (BUTTON_WEIGHT / total_weight);
            ctx.layout_child_canvas(id, pad_x, y, child_w, ch, canvas);
            y += ch + gap_frac;
        }

        if let Some(bm_id) = self.bookmarks_panel {
            let bm_h = usable_h * (BOOKMARKS_WEIGHT / total_weight);
            ctx.layout_child_canvas(bm_id, pad_x, y, child_w, bm_h, canvas);
        }
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState) {}
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_panel_new() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx));
        assert_eq!(panel.get_title(), Some("Eagle Mode".to_string()));
    }

    #[test]
    fn test_control_panel_opaque() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx));
        assert!(panel.IsOpaque());
    }

    #[test]
    fn test_control_panel_behavior() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emMainControlPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn test_control_button() {
        let btn = ControlButton {
            label: "Test".to_string(),
        };
        assert_eq!(btn.get_title(), Some("Test".to_string()));
        assert!(btn.IsOpaque());
    }
}
