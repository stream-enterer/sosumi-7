//! Input-handling demo derived from C++ `InputExample.cpp`.
//!
//! Logs keyboard and mouse events to an on-screen list, demonstrating
//! modifier matching, key press/release tracking, and mouse GetPos.

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emInput::{emInputEvent, InputKey, InputVariant};
use eaglemode_rs::emCore::emInputState::emInputState;
use eaglemode_rs::emCore::emPanel::{PanelBehavior, PanelState};
use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::{emPainter, TextAlignment, VAlign};
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

const MAX_LOG: usize = 15;

struct InputPanel {
    x_key_down: bool,
    button_down: bool,
    last_mx: f64,
    last_my: f64,
    log: Vec<String>,
}

impl InputPanel {
    fn new() -> Self {
        Self {
            x_key_down: false,
            button_down: false,
            last_mx: 0.0,
            last_my: 0.0,
            log: Vec::new(),
        }
    }

    fn push_log(&mut self, msg: String) {
        if self.log.len() >= MAX_LOG {
            self.log.remove(0);
        }
        self.log.push(msg);
    }
}

impl PanelBehavior for InputPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Input(&mut self, event: &emInputEvent, _state: &PanelState, input_state: &emInputState) -> bool {
        // E with no modifiers
        if event.key == InputKey::Key('e')
            && event.variant == InputVariant::Press
            && !event.shift
            && !event.ctrl
            && !event.alt
        {
            self.push_log("E pressed (no modifiers)".into());
            return true;
        }

        // Shift+Alt+G
        if event.key == InputKey::Key('g')
            && event.variant == InputVariant::Press
            && event.shift
            && event.alt
            && !event.ctrl
        {
            self.push_log("Shift+Alt+G".into());
            return true;
        }

        // Ctrl+V
        if event.key == InputKey::Key('v')
            && event.variant == InputVariant::Press
            && event.ctrl
            && !event.shift
            && !event.alt
        {
            self.push_log("Ctrl+V hotkey".into());
            return true;
        }

        // Dollar sign character
        if event.variant == InputVariant::Press && event.chars == "$" {
            self.push_log("Dollar sign ($) pressed".into());
            return true;
        }

        // X key press/release tracking
        if event.key == InputKey::Key('x') && event.variant == InputVariant::Press {
            self.x_key_down = true;
            self.push_log("X key pressed".into());
            return true;
        }
        if self.x_key_down && !input_state.Get(InputKey::Key('x')) {
            self.x_key_down = false;
            self.push_log("X key released".into());
        }

        // Left mouse button
        if event.key == InputKey::MouseLeft && event.variant == InputVariant::Press {
            self.button_down = true;
            self.last_mx = event.mouse_x;
            self.last_my = event.mouse_y;
            self.push_log(format!(
                "Click at ({:.0}, {:.0})",
                event.mouse_x, event.mouse_y
            ));
            // Don't eat — let panel system handle focus.
            return false;
        }
        if self.button_down && !input_state.Get(InputKey::MouseLeft) {
            self.button_down = false;
            self.push_log("Left button released".into());
        }

        // Mouse drag tracking
        if self.button_down
            && event.variant == InputVariant::Move
            && (self.last_mx != event.mouse_x || self.last_my != event.mouse_y)
        {
            self.last_mx = event.mouse_x;
            self.last_my = event.mouse_y;
            self.push_log(format!(
                "Dragged to ({:.0}, {:.0})",
                event.mouse_x, event.mouse_y
            ));
        }

        false
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        p.PaintRect(0.0, 0.0, w, h, emColor::WHITE, emColor::TRANSPARENT);

        // Title
        p.PaintTextBoxed(
            0.0,
            0.0,
            w,
            h * 0.08,
            "Input Demo — press keys, click mouse",
            h * 0.05,
            emColor::BLACK,
            emColor::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );

        // Event log
        for (i, entry) in self.log.iter().enumerate() {
            p.PaintText(
                0.02 * w,
                (0.10 + i as f64 * 0.04) * h,
                entry,
                h * 0.03,
                1.0,
                emColor::BLACK,
                emColor::TRANSPARENT,
            );
        }

        // Mouse GetPos indicator
        if self.button_down {
            let sz = 0.01 * w;
            p.PaintRect(
                self.last_mx - sz,
                self.last_my - sz,
                sz * 2.0,
                sz * 2.0,
                emColor::rgba(255, 0, 0, 180),
                emColor::TRANSPARENT,
            );
        }
    }
}

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let root = app.tree.create_root("root");
        app.tree.set_behavior(root, Box::new(InputPanel::new()));
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0);

        let close_sig = app.scheduler.borrow_mut().create_signal();
        let flags_sig = app.scheduler.borrow_mut().create_signal();
        let focus_sig = app.scheduler.borrow_mut().create_signal();
        let geometry_sig = app.scheduler.borrow_mut().create_signal();
        let win = eaglemode_rs::emCore::emWindow::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
            flags_sig,
            focus_sig,
            geometry_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        app.windows.get_mut(&wid).unwrap().view_mut().flags |= ViewFlags::ROOT_SAME_TALLNESS;
    }));
    app.run();
}
