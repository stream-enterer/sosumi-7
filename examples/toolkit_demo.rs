//! Toolkit demo derived from C++ `ToolkitExample.cpp`.
//!
//! Demonstrates all major widget types with closure-based callbacks that
//! update a shared message display. Uses the widget-wrapper PanelBehavior
//! pattern required by the eaglemode-rs architecture.

use std::cell::RefCell;
use std::rc::Rc;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emCursor::emCursor;
use eaglemode_rs::emCore::emInput::emInputEvent;
use eaglemode_rs::emCore::emInputState::emInputState;
use eaglemode_rs::emCore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use eaglemode_rs::emCore::emPanelCtx::PanelCtx;
use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::{emPainter, TextAlignment, VAlign};
use eaglemode_rs::emCore::emButton::emButton;

use eaglemode_rs::emCore::emCheckBox::emCheckBox;

use eaglemode_rs::emCore::emCheckButton::emCheckButton;

use eaglemode_rs::emCore::emColorField::emColorField;

use eaglemode_rs::emCore::emListBox::{emListBox, SelectionMode};

use eaglemode_rs::emCore::emLook::emLook;

use eaglemode_rs::emCore::emRadioButton::{emRadioButton, RadioGroup};

use eaglemode_rs::emCore::emScalarField::emScalarField;

use eaglemode_rs::emCore::emTextField::emTextField;
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

// ── Widget wrapper panels ──
// Each widget needs a PanelBehavior wrapper to participate in the panel tree.

struct ButtonPanel {
    button: emButton,
}
impl PanelBehavior for ButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.button.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.button.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct CheckButtonPanel {
    widget: emCheckButton,
}
impl PanelBehavior for CheckButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct CheckBoxPanel {
    widget: emCheckBox,
}
impl PanelBehavior for CheckBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct RadioButtonPanel {
    widget: emRadioButton,
}
impl PanelBehavior for RadioButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct TextFieldPanel {
    widget: emTextField,
}
impl PanelBehavior for TextFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.cycle_blink(s.in_focused_path());
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.widget.on_focus_changed(state.in_focused_path());
        }
    }
}

struct ScalarFieldPanel {
    widget: emScalarField,
}
impl PanelBehavior for ScalarFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, s.enabled, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct ColorFieldPanel {
    widget: emColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

struct ListBoxPanel {
    widget: emListBox,
}
impl PanelBehavior for ListBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

// ── Root panel ──

struct ToolkitRoot {
    msg: Rc<RefCell<String>>,
    look: Rc<emLook>,
}

impl PanelBehavior for ToolkitRoot {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        p.PaintRect(
            0.0,
            0.0,
            w,
            h,
            emColor::rgba(0x20, 0x30, 0x40, 0xFF),
            emColor::TRANSPARENT,
        );

        p.PaintTextBoxed(
            0.0,
            0.0,
            w,
            h * 0.05,
            "Toolkit Demo",
            h * 0.04,
            emColor::WHITE,
            emColor::TRANSPARENT,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Center,
            0.5,
            true,
            0.15,
        );
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();
        let rect = ctx.layout_rect();
        let h = rect.h / rect.w;

        // Grid layout constants
        let cols = 3;
        let margin = 0.02;
        let top = 0.06 * h;
        let cell_w = (1.0 - margin * (cols as f64 + 1.0)) / cols as f64;
        let cell_h = cell_w * 0.35;

        if !children.is_empty() {
            for (i, child) in children.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let x = margin + col as f64 * (cell_w + margin);
                let y = top + row as f64 * (cell_h + margin);
                ctx.layout_child(*child, x, y, cell_w, cell_h);
            }
            return;
        }

        // Create all widgets
        let msg = self.msg.clone();
        let look = self.look.clone();

        // 1. Message label (read-only text field)
        let mut tf_msg = emTextField::new(look.clone());
        tf_msg.SetText("Click a widget to see its callback here");
        let msg_id = ctx.create_child_with("msg", Box::new(TextFieldPanel { widget: tf_msg }));

        // 2. emButton
        let msg_c = msg.clone();
        let mut button = emButton::new("Button", look.clone());
        button.on_click = Some(Box::new(move || {
            *msg_c.borrow_mut() = "Button clicked".into();
        }));
        let _b = ctx.create_child_with("button", Box::new(ButtonPanel { button }));

        // 3. Check emButton
        let msg_c = msg.clone();
        let mut cb = emCheckButton::new("Check Button", look.clone());
        cb.on_check = Some(Box::new(move |checked| {
            *msg_c.borrow_mut() = format!(
                "Check Button switched {}",
                if checked { "on" } else { "off" }
            );
        }));
        let _cb_id = ctx.create_child_with("cb", Box::new(CheckButtonPanel { widget: cb }));

        // 4. Check Box
        let msg_c = msg.clone();
        let mut cbx = emCheckBox::new("Check Box", look.clone());
        cbx.on_check = Some(Box::new(move |checked| {
            *msg_c.borrow_mut() =
                format!("Check Box switched {}", if checked { "on" } else { "off" });
        }));
        let _cbx_id = ctx.create_child_with("cbx", Box::new(CheckBoxPanel { widget: cbx }));

        // 5-7. Radio Buttons
        let rg = RadioGroup::new();
        let msg_c = msg.clone();
        rg.borrow_mut().on_select = Some(Box::new(move |idx| {
            if let Some(i) = idx {
                *msg_c.borrow_mut() = format!("Radio Box {} selected", i + 1);
            }
        }));
        for i in 0..3 {
            let rb = emRadioButton::new(&format!("Radio {}", i + 1), look.clone(), rg.clone(), i);
            ctx.create_child_with(&format!("rb{i}"), Box::new(RadioButtonPanel { widget: rb }));
        }

        // 8. Editable text field
        let msg_c = msg.clone();
        let mut tf = emTextField::new(look.clone());
        tf.SetEditable(true);
        tf.SetMultiLineMode(true);
        tf.SetText("Edit me\nsecond line");
        tf.on_text = Some(Box::new(move |text| {
            let preview: String = text.chars().take(30).collect();
            *msg_c.borrow_mut() = format!("Text changed: \"{preview}\"");
        }));
        ctx.create_child_with("tf", Box::new(TextFieldPanel { widget: tf }));

        // 9. Scalar Field
        let msg_c = msg.clone();
        let mut sf = emScalarField::new(0.0, 100.0, look.clone());
        sf.SetEditable(true);
        sf.SetScaleMarkIntervals(&[50, 10, 5, 1]);
        sf.on_value = Some(Box::new(move |val| {
            *msg_c.borrow_mut() = format!("Scalar value: {val:.0}");
        }));
        ctx.create_child_with("sf", Box::new(ScalarFieldPanel { widget: sf }));

        // 10. emColor Field
        let msg_c = msg.clone();
        let mut cf = emColorField::new(look.clone());
        cf.SetEditable(true);
        cf.SetColor(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
        cf.on_color = Some(Box::new(move |color| {
            *msg_c.borrow_mut() = format!(
                "Color: #{:02X}{:02X}{:02X}",
                color.GetRed(),
                color.GetGreen(),
                color.GetBlue()
            );
        }));
        ctx.create_child_with("cf", Box::new(ColorFieldPanel { widget: cf }));

        // 11. List Box
        let msg_c = msg.clone();
        let mut lb = emListBox::new(look.clone());
        lb.SetSelectionType(SelectionMode::Single);
        lb.set_items((1..=7).map(|i| format!("Item {i}")).collect());
        lb.on_selection = Some(Box::new(move |sel| {
            if let Some(&idx) = sel.first() {
                *msg_c.borrow_mut() = format!("List item {} selected", idx + 1);
            }
        }));
        ctx.create_child_with("lb", Box::new(ListBoxPanel { widget: lb }));

        // Layout all children now that they exist
        let all = ctx.children();
        for (i, child) in all.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = margin + col as f64 * (cell_w + margin);
            let y = top + row as f64 * (cell_h + margin);
            ctx.layout_child(*child, x, y, cell_w, cell_h);
        }

        // Store msg panel id to update text from the message Rc in PaintContent
        // We can't update the emTextField from PaintContent, so the message display
        // relies on the Rc<RefCell<String>> shared state.
        let _ = msg_id;
    }
}

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let msg = Rc::new(RefCell::new(String::new()));
        let look = emLook::new();

        let root = app.tree.create_root("root");
        app.tree.set_behavior(
            root,
            Box::new(ToolkitRoot {
                msg: msg.clone(),
                look,
            }),
        );
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
