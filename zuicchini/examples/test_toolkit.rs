//! Isolated toolkit test panel — exercises only the widget showcase subtree.
//!
//! All panel/widget code below is line-for-line identical to `test_panel.rs`.
//! Only the `main()` scaffolding differs (marked with "SCAFFOLDING" comments).

use std::rc::Rc;

use zuicchini::foundation::Color;
use zuicchini::input::{Cursor, InputEvent, InputState};
use zuicchini::layout::raster::RasterGroup;
use zuicchini::panel::{NoticeFlags, PanelBehavior, PanelCtx, PanelState, ViewConditionType, ViewFlags};
use zuicchini::render::Painter;
use zuicchini::widget::{
    Button, CheckBox, CheckButton, ColorField, ListBox, Look, RadioBox, RadioButton, RadioGroup,
    ScalarField, SelectionMode, TextField,
};
use zuicchini::window::{App, WindowFlags};

// ═══════════════════════════════════════════════════════════════════════
// Widget wrapper panels (identical to test_panel.rs)
// ═══════════════════════════════════════════════════════════════════════

struct ButtonPanel {
    widget: Button,
}
impl PanelBehavior for ButtonPanel {
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

struct RadioBoxPanel {
    widget: RadioBox,
}
impl PanelBehavior for RadioBoxPanel {
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
        self.widget.cycle_blink(_s.in_focused_path());
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
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.widget.on_focus_changed(state.in_focused_path());
        }
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

// ═══════════════════════════════════════════════════════════════════════
// WidgetGroupPanel — bordered group container for widget categories
// (identical to test_panel.rs)
// ═══════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
enum WidgetCategory {
    Buttons,
    CheckWidgets,
    RadioWidgets,
    TextFields,
    ScalarFields,
    ColorFields,
    ListBoxes,
}

struct WidgetGroupPanel {
    group: RasterGroup,
    category: WidgetCategory,
    look: Rc<Look>,
}

impl WidgetGroupPanel {
    fn new(category: WidgetCategory, caption: &str, look: Rc<Look>) -> Self {
        let mut group = RasterGroup::new();
        group.border.caption = caption.to_string();
        group.look = (*look).clone();
        Self {
            group,
            category,
            look,
        }
    }
}

impl PanelBehavior for WidgetGroupPanel {
    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.group.paint(p, w, h, s);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            let look = self.look.clone();
            match self.category {
                WidgetCategory::Buttons => {
                    let bt1 = Button::new("Button", look.clone());
                    ctx.create_child_with("b1", Box::new(ButtonPanel { widget: bt1 }));
                    let bt2 = Button::new("Long Desc", look);
                    ctx.create_child_with("b2", Box::new(ButtonPanel { widget: bt2 }));
                }
                WidgetCategory::CheckWidgets => {
                    let cb1 = CheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c1", Box::new(CheckButtonPanel { widget: cb1 }));
                    let cb2 = CheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c2", Box::new(CheckButtonPanel { widget: cb2 }));
                    let cbx1 = CheckBox::new("Check Box", look.clone());
                    ctx.create_child_with("c4", Box::new(CheckBoxPanel { widget: cbx1 }));
                    let cbx2 = CheckBox::new("Check Box", look);
                    ctx.create_child_with("c5", Box::new(CheckBoxPanel { widget: cbx2 }));
                }
                WidgetCategory::RadioWidgets => {
                    let rg = RadioGroup::new();
                    let rb1 = RadioButton::new("Radio Button", look.clone(), rg.clone(), 0);
                    ctx.create_child_with("r1", Box::new(RadioButtonPanel { widget: rb1 }));
                    let rb2 = RadioButton::new("Radio Button", look.clone(), rg.clone(), 1);
                    ctx.create_child_with("r2", Box::new(RadioButtonPanel { widget: rb2 }));
                    let rb3 = RadioButton::new("Radio Button", look.clone(), rg, 2);
                    ctx.create_child_with("r3", Box::new(RadioButtonPanel { widget: rb3 }));
                    let rg2 = RadioGroup::new();
                    let rbx1 = RadioBox::new("Radio Box", look.clone(), rg2.clone(), 0);
                    ctx.create_child_with("r4", Box::new(RadioBoxPanel { widget: rbx1 }));
                    let rbx2 = RadioBox::new("Radio Box", look.clone(), rg2.clone(), 1);
                    ctx.create_child_with("r5", Box::new(RadioBoxPanel { widget: rbx2 }));
                    let rbx3 = RadioBox::new("Radio Box", look, rg2, 2);
                    ctx.create_child_with("r6", Box::new(RadioBoxPanel { widget: rbx3 }));
                }
                WidgetCategory::TextFields => {
                    let mut tf1 = TextField::new(look.clone());
                    tf1.set_editable(false);
                    tf1.set_caption("Read-Only");
                    tf1.set_text("Read-Only");
                    ctx.create_child_with("tf1", Box::new(TextFieldPanel { widget: tf1 }));
                    let mut tf2 = TextField::new(look.clone());
                    tf2.set_editable(true);
                    tf2.set_caption("Editable");
                    tf2.set_text("Editable");
                    ctx.create_child_with("tf2", Box::new(TextFieldPanel { widget: tf2 }));
                    let mut tf3 = TextField::new(look.clone());
                    tf3.set_editable(true);
                    tf3.set_caption("Password");
                    tf3.set_text("Password");
                    tf3.set_password_mode(true);
                    ctx.create_child_with("tf3", Box::new(TextFieldPanel { widget: tf3 }));
                    let mut tf4 = TextField::new(look);
                    tf4.set_editable(true);
                    tf4.set_caption("Multi-Line");
                    tf4.set_multi_line(true);
                    tf4.set_text("first line\nsecond line\n...");
                    ctx.create_child_with("mltf1", Box::new(TextFieldPanel { widget: tf4 }));
                }
                WidgetCategory::ScalarFields => {
                    let sf1 = ScalarField::new(0.0, 100.0, look.clone());
                    ctx.create_child_with("sf1", Box::new(ScalarFieldPanel { widget: sf1 }));
                    let mut sf2 = ScalarField::new(0.0, 100.0, look.clone());
                    sf2.set_editable(true);
                    ctx.create_child_with("sf2", Box::new(ScalarFieldPanel { widget: sf2 }));
                    let mut sf3 = ScalarField::new(-1000.0, 1000.0, look);
                    sf3.set_editable(true);
                    sf3.set_scale_mark_intervals(&[1000, 100, 10, 5, 1]);
                    ctx.create_child_with("sf3", Box::new(ScalarFieldPanel { widget: sf3 }));
                }
                WidgetCategory::ColorFields => {
                    let mut cf1 = ColorField::new(look.clone());
                    cf1.set_color(Color::rgba(0xBB, 0x22, 0x22, 0xFF));
                    ctx.create_child_with("cf1", Box::new(ColorFieldPanel { widget: cf1 }));
                    let mut cf2 = ColorField::new(look.clone());
                    cf2.set_editable(true);
                    cf2.set_color(Color::rgba(0x22, 0xBB, 0x22, 0xFF));
                    ctx.create_child_with("cf2", Box::new(ColorFieldPanel { widget: cf2 }));
                    let mut cf3 = ColorField::new(look);
                    cf3.set_editable(true);
                    cf3.set_alpha_enabled(true);
                    cf3.set_color(Color::rgba(0x22, 0x22, 0xBB, 0xFF));
                    ctx.create_child_with("cf3", Box::new(ColorFieldPanel { widget: cf3 }));
                }
                WidgetCategory::ListBoxes => {
                    let lb1 = ListBox::new(look.clone());
                    ctx.create_child_with("l1", Box::new(ListBoxPanel { widget: lb1 }));
                    let mut lb2 = ListBox::new(look.clone());
                    lb2.set_selection_mode(SelectionMode::Single);
                    lb2.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l2", Box::new(ListBoxPanel { widget: lb2 }));
                    let mut lb3 = ListBox::new(look.clone());
                    lb3.set_selection_mode(SelectionMode::ReadOnly);
                    lb3.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l3", Box::new(ListBoxPanel { widget: lb3 }));
                    let mut lb4 = ListBox::new(look.clone());
                    lb4.set_selection_mode(SelectionMode::Multi);
                    lb4.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l4", Box::new(ListBoxPanel { widget: lb4 }));
                    let mut lb5 = ListBox::new(look);
                    lb5.set_selection_mode(SelectionMode::Toggle);
                    lb5.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l5", Box::new(ListBoxPanel { widget: lb5 }));
                }
            }
        }
        self.group.layout_children(ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TkTestPanel — widget showcase (identical to test_panel.rs)
// ═══════════════════════════════════════════════════════════════════════

struct TkTestPanel {
    look: Rc<Look>,
}

impl TkTestPanel {
    fn new(look: Rc<Look>) -> Self {
        Self { look }
    }
}

impl PanelBehavior for TkTestPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, _s: &PanelState) {
        p.paint_rect(
            0.0,
            0.0,
            w,
            h,
            Color::rgba(0x30, 0x40, 0x50, 0xFF),
            Color::TRANSPARENT,
        );
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        let children = ctx.children();

        // Grid layout for 7 groups
        let cols = 3;
        let margin = 0.02;
        let cell_w = (1.0 - margin * (cols as f64 + 1.0)) / cols as f64;
        let cell_h = cell_w * 0.8;

        if !children.is_empty() {
            for (i, child) in children.iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let x = margin + col as f64 * (cell_w + margin);
                let y = margin + row as f64 * (cell_h + margin);
                ctx.layout_child(*child, x, y, cell_w, cell_h);
            }
            return;
        }

        let look = self.look.clone();

        let groups: &[(&str, &str, WidgetCategory)] = &[
            ("grp_btn", "Buttons", WidgetCategory::Buttons),
            ("grp_chk", "Check Widgets", WidgetCategory::CheckWidgets),
            ("grp_rad", "Radio Widgets", WidgetCategory::RadioWidgets),
            ("grp_txt", "Text Fields", WidgetCategory::TextFields),
            ("grp_scl", "Scalar Fields", WidgetCategory::ScalarFields),
            ("grp_clr", "Color Fields", WidgetCategory::ColorFields),
            ("grp_lst", "List Boxes", WidgetCategory::ListBoxes),
        ];

        for &(name, caption, cat) in groups {
            ctx.create_child_with(
                name,
                Box::new(WidgetGroupPanel::new(cat, caption, look.clone())),
            );
        }

        let all = ctx.children();
        for (i, child) in all.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = margin + col as f64 * (cell_w + margin);
            let y = margin + row as f64 * (cell_h + margin);
            ctx.layout_child(*child, x, y, cell_w, cell_h);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SCAFFOLDING — Main (differs from test_panel.rs: mounts TkTestGrpPanel
// directly as root instead of TestPanel)
// ═══════════════════════════════════════════════════════════════════════

fn main() {
    let look = Look::new();
    let app = App::new(Box::new(move |app, event_loop| {
        // SCAFFOLDING: mount a single TkTestPanel as root (matches C++ standalone)
        let root = app.tree.create_root("root");
        app.tree
            .set_behavior(root, Box::new(TkTestPanel::new(look.clone())));
        app.tree.set_layout_rect(root, 0.0, 0.0, 1.0, 1.0);
        app.tree
            .set_auto_expansion_threshold(root, 900.0, ViewConditionType::Area);

        let close_sig = app.scheduler.create_signal();
        let flags_sig = app.scheduler.create_signal();
        let win = zuicchini::window::ZuiWindow::create(
            event_loop,
            app.gpu(),
            root,
            WindowFlags::AUTO_DELETE,
            close_sig,
            flags_sig,
        );
        let wid = win.winit_window.id();
        app.windows.insert(wid, win);
        {
            let win = app.windows.get_mut(&wid).unwrap();
            // SCAFFOLDING: ROOT_SAME_TALLNESS matches test_panel.rs window setup
            let flags = win.view().flags | ViewFlags::ROOT_SAME_TALLNESS;
            win.view_mut().set_view_flags(flags, &mut app.tree);
        }
    }));
    app.run();
}
