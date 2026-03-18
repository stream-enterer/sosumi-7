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
    ScalarField, SelectionMode, TextField, Tunnel,
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
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e, _s, _is)
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
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e, _s, _is)
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
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e, _s, _is)
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
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e, _s, _is)
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
        self.widget.paint(p, w, h, _s.enabled);
    }
    fn input(&mut self, e: &InputEvent, _s: &PanelState, _is: &InputState) -> bool {
        self.widget.input(e, _s, _is)
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
        self.widget.input(e, _s, _is)
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
        self.widget.input(e, _s, _is)
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
        self.widget.input(e, _s, _is)
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
        self.widget.input(e, _s, _is)
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
    Tunnels,
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
        group.border.set_border_scaling(2.5);
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
                    let mut bt2 = Button::new("Long Desc", look.clone());
                    let long_desc = "This is a looooooooooooooooooooooooooooooooooooooooooooooooooooooong description of the button.\n".repeat(100);
                    bt2.set_description(&long_desc);
                    ctx.create_child_with("b2", Box::new(ButtonPanel { widget: bt2 }));
                    let mut bt3 = Button::new("NoEOI", look);
                    bt3.set_no_eoi(true);
                    ctx.create_child_with("b3", Box::new(ButtonPanel { widget: bt3 }));
                }
                WidgetCategory::CheckWidgets => {
                    let cb1 = CheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c1", Box::new(CheckButtonPanel { widget: cb1 }));
                    let cb2 = CheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c2", Box::new(CheckButtonPanel { widget: cb2 }));
                    let cb3 = CheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c3", Box::new(CheckButtonPanel { widget: cb3 }));
                    let cbx1 = CheckBox::new("Check Box", look.clone());
                    ctx.create_child_with("c4", Box::new(CheckBoxPanel { widget: cbx1 }));
                    let cbx2 = CheckBox::new("Check Box", look.clone());
                    ctx.create_child_with("c5", Box::new(CheckBoxPanel { widget: cbx2 }));
                    let cbx3 = CheckBox::new("Check Box", look);
                    ctx.create_child_with("c6", Box::new(CheckBoxPanel { widget: cbx3 }));
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
                    let mut sf3 = ScalarField::new(-1000.0, 1000.0, look.clone());
                    sf3.set_editable(true);
                    sf3.set_scale_mark_intervals(&[1000, 100, 10, 5, 1]);
                    ctx.create_child_with("sf3", Box::new(ScalarFieldPanel { widget: sf3 }));
                    // sf4: Level with custom formatter
                    let mut sf4 = ScalarField::new(1.0, 5.0, look.clone());
                    sf4.set_editable(true);
                    sf4.set_text_box_tallness(0.25);
                    sf4.set_value(3.0);
                    sf4.set_text_of_value_fn(Box::new(|val, _iv| format!("Level {}", val)));
                    ctx.create_child_with("sf4", Box::new(ScalarFieldPanel { widget: sf4 }));
                    // sf5: Play Length with time formatter
                    let mut sf5 = ScalarField::new(0.0, 24.0 * 3600.0 * 1000.0, look.clone());
                    sf5.set_editable(true);
                    sf5.set_value(4.0 * 3600.0 * 1000.0);
                    sf5.set_scale_mark_intervals(&[
                        3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1,
                    ]);
                    sf5.set_text_of_value_fn(Box::new(|value, mark_interval| {
                        let v = value;
                        let h = v / 3600000;
                        let m = (v / 60000) % 60;
                        let s = (v / 1000) % 60;
                        let ms = v % 1000;
                        if mark_interval < 10 {
                            format!("{:02}:{:02}:{:02}\n.{:03}", h, m, s, ms)
                        } else if mark_interval < 100 {
                            format!("{:02}:{:02}:{:02}\n.{:02}", h, m, s, ms / 10)
                        } else if mark_interval < 1000 {
                            format!("{:02}:{:02}:{:02}\n.{:01}", h, m, s, ms / 100)
                        } else if mark_interval < 60000 {
                            format!("{:02}:{:02}:{:02}", h, m, s)
                        } else {
                            format!("{:02}:{:02}", h, m)
                        }
                    }));
                    ctx.create_child_with("sf5", Box::new(ScalarFieldPanel { widget: sf5 }));
                    // sf6: Play Position (same formatter, static 4h max)
                    let mut sf6 = ScalarField::new(0.0, 4.0 * 3600.0 * 1000.0, look);
                    sf6.set_editable(true);
                    sf6.set_scale_mark_intervals(&[
                        3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1,
                    ]);
                    sf6.set_text_of_value_fn(Box::new(|value, mark_interval| {
                        let v = value;
                        let h = v / 3600000;
                        let m = (v / 60000) % 60;
                        let s = (v / 1000) % 60;
                        let ms = v % 1000;
                        if mark_interval < 10 {
                            format!("{:02}:{:02}:{:02}\n.{:03}", h, m, s, ms)
                        } else if mark_interval < 100 {
                            format!("{:02}:{:02}:{:02}\n.{:02}", h, m, s, ms / 10)
                        } else if mark_interval < 1000 {
                            format!("{:02}:{:02}:{:02}\n.{:01}", h, m, s, ms / 100)
                        } else if mark_interval < 60000 {
                            format!("{:02}:{:02}:{:02}", h, m, s)
                        } else {
                            format!("{:02}:{:02}", h, m)
                        }
                    }));
                    ctx.create_child_with("sf6", Box::new(ScalarFieldPanel { widget: sf6 }));
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
                    let mut lb5 = ListBox::new(look.clone());
                    lb5.set_selection_mode(SelectionMode::Toggle);
                    lb5.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l5", Box::new(ListBoxPanel { widget: lb5 }));
                    // l6: Single column
                    let mut lb6 = ListBox::new(look.clone());
                    lb6.set_caption("Single Column");
                    lb6.set_fixed_column_count(Some(1));
                    lb6.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    lb6.select(0, true);
                    ctx.create_child_with("l6", Box::new(ListBoxPanel { widget: lb6 }));
                    // l7: Custom List Box (Multi selection placeholder)
                    let mut lb7 = ListBox::new(look);
                    lb7.set_caption("Custom List Box");
                    lb7.set_selection_mode(SelectionMode::Multi);
                    lb7.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    lb7.select(0, true);
                    ctx.create_child_with("l7", Box::new(ListBoxPanel { widget: lb7 }));
                }
                WidgetCategory::Tunnels => {
                    // t1: default tunnel
                    let t1 = Tunnel::new(look.clone()).with_caption("Tunnel");
                    ctx.create_child_with("t1", Box::new(t1));
                    // t2: deeper tunnel
                    let mut t2 = Tunnel::new(look.clone()).with_caption("Deeper Tunnel");
                    t2.set_depth(30.0);
                    ctx.create_child_with("t2", Box::new(t2));
                    // t3: square end
                    let mut t3 = Tunnel::new(look.clone()).with_caption("Square End");
                    t3.set_child_tallness(1.0);
                    ctx.create_child_with("t3", Box::new(t3));
                    // t4: square end + zero depth
                    let mut t4 = Tunnel::new(look).with_caption("Square End, Zero Depth");
                    t4.set_child_tallness(1.0);
                    t4.set_depth(0.0);
                    ctx.create_child_with("t4", Box::new(t4));
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
    group: RasterGroup,
    look: Rc<Look>,
}

impl TkTestPanel {
    fn new(look: Rc<Look>) -> Self {
        let mut group = RasterGroup::new();
        group.border.caption = "Toolkit Test".to_string();
        group.border.set_border_scaling(2.5);
        group.layout.preferred_child_tallness = 0.3;
        group.look = (*look).clone();
        Self { group, look }
    }
}

impl PanelBehavior for TkTestPanel {
    fn is_opaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn paint(&mut self, p: &mut Painter, w: f64, h: f64, s: &PanelState) {
        self.group.paint(p, w, h, s);
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            let look = self.look.clone();

            let groups: &[(&str, &str, WidgetCategory)] = &[
                ("grp_btn", "Buttons", WidgetCategory::Buttons),
                ("grp_chk", "Check Widgets", WidgetCategory::CheckWidgets),
                ("grp_rad", "Radio Widgets", WidgetCategory::RadioWidgets),
                ("grp_txt", "Text Fields", WidgetCategory::TextFields),
                ("grp_scl", "Scalar Fields", WidgetCategory::ScalarFields),
                ("grp_clr", "Color Fields", WidgetCategory::ColorFields),
                ("grp_lst", "List Boxes", WidgetCategory::ListBoxes),
                ("grp_tun", "Tunnels", WidgetCategory::Tunnels),
            ];

            for &(name, caption, cat) in groups {
                ctx.create_child_with(
                    name,
                    Box::new(WidgetGroupPanel::new(cat, caption, look.clone())),
                );
            }
        }
        self.group.layout_children(ctx);
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
