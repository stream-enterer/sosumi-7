//! Toolkit demo derived from C++ `ToolkitExample.cpp`.
//!
//! Demonstrates all major widget types with closure-based callbacks that
//! update a shared message display. Uses the widget-wrapper PanelBehavior
//! pattern required by the zuicchini architecture.

use std::cell::RefCell;
use std::rc::Rc;

use zuicchini::foundation::Color;
use zuicchini::input::{Cursor, InputEvent, InputState};
use zuicchini::panel::{PanelBehavior, PanelCtx, PanelState, ViewFlags};
use zuicchini::render::{Painter, TextAlignment, VAlign};
use zuicchini::widget::{
    Button, CheckBox, CheckButton, ColorField, ListBox, Look, RadioButton, RadioGroup, ScalarField,
    SelectionMode, TextField,
};
use zuicchini::window::{App, WindowFlags};

// ── Widget wrapper panels ──
// Each widget needs a PanelBehavior wrapper to participate in the panel tree.

struct ButtonPanel {
    button: Button,
}
impl PanelBehavior for ButtonPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.button.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.button.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.button.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct CheckButtonPanel {
    widget: CheckButton,
}
impl PanelBehavior for CheckButtonPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct CheckBoxPanel {
    widget: CheckBox,
}
impl PanelBehavior for CheckBoxPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct RadioButtonPanel {
    widget: RadioButton,
}
impl PanelBehavior for RadioButtonPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct TextFieldPanel {
    widget: TextField,
}
impl PanelBehavior for TextFieldPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ScalarFieldPanel {
    widget: ScalarField,
}
impl PanelBehavior for ScalarFieldPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.widget.paint(p, w, h, s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn get_cursor(&self) -> Cursor {
        self.widget.get_cursor()
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ColorFieldPanel {
    widget: ColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

struct ListBoxPanel {
    widget: ListBox,
}
impl PanelBehavior for ListBoxPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        self.widget.paint(p, w, h);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e)
    }
    fn is_opaque(&self) -> bool {
        true
    }
}

// ── Root panel ──

struct ToolkitRoot {
    msg: Rc<RefCell<String>>,
    look: Rc<Look>,
}

impl PanelBehavior for ToolkitRoot {
    fn is_opaque(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _state: &PanelState) {
        p.paint_rect(0.0, 0.0, w, h, Color::rgba(0x20, 0x30, 0x40, 0xFF));

        p.paint_text_boxed(
            0.0,
            0.0,
            w,
            h * 0.05,
            "Toolkit Demo",
            h * 0.04,
            Color::WHITE,
            Color::TRANSPARENT,
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

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
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
        let mut tf_msg = TextField::new(look.clone());
        tf_msg.set_text("Click a widget to see its callback here");
        let msg_id = ctx.create_child_with("msg", Box::new(TextFieldPanel { widget: tf_msg }));

        // 2. Button
        let msg_c = msg.clone();
        let mut button = Button::new("Button", look.clone());
        button.on_click = Some(Box::new(move || {
            *msg_c.borrow_mut() = "Button clicked".into();
        }));
        let _b = ctx.create_child_with("button", Box::new(ButtonPanel { button }));

        // 3. Check Button
        let msg_c = msg.clone();
        let mut cb = CheckButton::new("Check Button", look.clone());
        cb.on_check = Some(Box::new(move |checked| {
            *msg_c.borrow_mut() = format!(
                "Check Button switched {}",
                if checked { "on" } else { "off" }
            );
        }));
        let _cb_id = ctx.create_child_with("cb", Box::new(CheckButtonPanel { widget: cb }));

        // 4. Check Box
        let msg_c = msg.clone();
        let mut cbx = CheckBox::new("Check Box", look.clone());
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
            let rb = RadioButton::new(&format!("Radio {}", i + 1), look.clone(), rg.clone(), i);
            ctx.create_child_with(&format!("rb{i}"), Box::new(RadioButtonPanel { widget: rb }));
        }

        // 8. Editable text field
        let msg_c = msg.clone();
        let mut tf = TextField::new(look.clone());
        tf.set_editable(true);
        tf.set_multi_line(true);
        tf.set_text("Edit me\nsecond line");
        tf.on_text = Some(Box::new(move |text| {
            let preview: String = text.chars().take(30).collect();
            *msg_c.borrow_mut() = format!("Text changed: \"{preview}\"");
        }));
        ctx.create_child_with("tf", Box::new(TextFieldPanel { widget: tf }));

        // 9. Scalar Field
        let msg_c = msg.clone();
        let mut sf = ScalarField::new(0.0, 100.0, look.clone());
        sf.set_editable(true);
        sf.set_scale_mark_intervals(&[50, 10, 5, 1]);
        sf.on_value = Some(Box::new(move |val| {
            *msg_c.borrow_mut() = format!("Scalar value: {val:.0}");
        }));
        ctx.create_child_with("sf", Box::new(ScalarFieldPanel { widget: sf }));

        // 10. Color Field
        let msg_c = msg.clone();
        let mut cf = ColorField::new(look.clone());
        cf.set_editable(true);
        cf.set_color(Color::rgba(0x22, 0xBB, 0x22, 0xFF));
        cf.on_color = Some(Box::new(move |color| {
            *msg_c.borrow_mut() = format!(
                "Color: #{:02X}{:02X}{:02X}",
                color.r(),
                color.g(),
                color.b()
            );
        }));
        ctx.create_child_with("cf", Box::new(ColorFieldPanel { widget: cf }));

        // 11. List Box
        let msg_c = msg.clone();
        let mut lb = ListBox::new(look.clone());
        lb.set_selection_mode(SelectionMode::Single);
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

        // Store msg panel id to update text from the message Rc in paint
        // We can't update the TextField from paint, so the message display
        // relies on the Rc<RefCell<String>> shared state.
        let _ = msg_id;
    }
}

fn main() {
    let app = App::new(Box::new(|app, event_loop| {
        let msg = Rc::new(RefCell::new(String::new()));
        let look = Look::new();

        let root = app.tree.create_root("root");
        app.tree.set_behavior(
            root,
            Box::new(ToolkitRoot {
                msg: msg.clone(),
                look,
            }),
        );
        app.tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);

        let close_sig = app.scheduler.create_signal();
        let win = zuicchini::window::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        app.windows.get_mut(&wid).unwrap().view_mut().flags |= ViewFlags::ROOT_SAME_TALLNESS;
    }));
    app.run();
}
