//! Isolated toolkit test panel — exercises only the widget showcase subtree.
//!
//! All panel/widget code below is line-for-line identical to `test_panel.rs`.
//! Only the `main()` scaffolding differs (marked with "SCAFFOLDING" comments).

use std::rc::Rc;

use eaglemode_rs::emCore::emColor::emColor;
use eaglemode_rs::emCore::emCursor::emCursor;
use eaglemode_rs::emCore::emInput::emInputEvent;
use eaglemode_rs::emCore::emInputState::emInputState;
use eaglemode_rs::emCore::emRasterGroup::emRasterGroup;
use eaglemode_rs::emCore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use eaglemode_rs::emCore::emPanelCtx::PanelCtx;
use eaglemode_rs::emCore::emPanelTree::ViewConditionType;
use eaglemode_rs::emCore::emView::ViewFlags;
use eaglemode_rs::emCore::emPainter::emPainter;
use eaglemode_rs::emCore::emButton::emButton;

use eaglemode_rs::emCore::emCheckBox::emCheckBox;

use eaglemode_rs::emCore::emCheckButton::emCheckButton;

use eaglemode_rs::emCore::emColorField::emColorField;

use eaglemode_rs::emCore::emListBox::{emListBox, SelectionMode};

use eaglemode_rs::emCore::emLook::emLook;

use eaglemode_rs::emCore::emRadioBox::emRadioBox;

use eaglemode_rs::emCore::emRadioButton::{emRadioButton, RadioGroup};

use eaglemode_rs::emCore::emScalarField::emScalarField;

use eaglemode_rs::emCore::emTextField::emTextField;

use eaglemode_rs::emCore::emTunnel::emTunnel;
use eaglemode_rs::emCore::emGUIFramework::App;
use eaglemode_rs::emCore::emWindow::WindowFlags;

// ═══════════════════════════════════════════════════════════════════════
// Widget wrapper panels (identical to test_panel.rs)
// ═══════════════════════════════════════════════════════════════════════

struct ButtonPanel {
    widget: emButton,
}
impl PanelBehavior for ButtonPanel {
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

struct RadioBoxPanel {
    widget: emRadioBox,
}
impl PanelBehavior for RadioBoxPanel {
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
    group: emRasterGroup,
    category: WidgetCategory,
    look: Rc<emLook>,
}

impl WidgetGroupPanel {
    fn new(category: WidgetCategory, caption: &str, look: Rc<emLook>) -> Self {
        let mut group = emRasterGroup::new();
        group.border.caption = caption.to_string();
        group.border.SetBorderScaling(2.5);
        group.look = (*look).clone();
        Self {
            group,
            category,
            look,
        }
    }
}

impl PanelBehavior for WidgetGroupPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.group.Paint(p, w, h, s);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            let look = self.look.clone();
            match self.category {
                WidgetCategory::Buttons => {
                    let bt1 = emButton::new("Button", look.clone());
                    ctx.create_child_with("b1", Box::new(ButtonPanel { widget: bt1 }));
                    let mut bt2 = emButton::new("Long Desc", look.clone());
                    let long_desc = "This is a looooooooooooooooooooooooooooooooooooooooooooooooooooooong description of the button.\n".repeat(100);
                    bt2.SetDescription(&long_desc);
                    ctx.create_child_with("b2", Box::new(ButtonPanel { widget: bt2 }));
                    let mut bt3 = emButton::new("NoEOI", look);
                    bt3.SetNoEOI(true);
                    ctx.create_child_with("b3", Box::new(ButtonPanel { widget: bt3 }));
                }
                WidgetCategory::CheckWidgets => {
                    let cb1 = emCheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c1", Box::new(CheckButtonPanel { widget: cb1 }));
                    let cb2 = emCheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c2", Box::new(CheckButtonPanel { widget: cb2 }));
                    let cb3 = emCheckButton::new("Check Button", look.clone());
                    ctx.create_child_with("c3", Box::new(CheckButtonPanel { widget: cb3 }));
                    let cbx1 = emCheckBox::new("Check Box", look.clone());
                    ctx.create_child_with("c4", Box::new(CheckBoxPanel { widget: cbx1 }));
                    let cbx2 = emCheckBox::new("Check Box", look.clone());
                    ctx.create_child_with("c5", Box::new(CheckBoxPanel { widget: cbx2 }));
                    let cbx3 = emCheckBox::new("Check Box", look);
                    ctx.create_child_with("c6", Box::new(CheckBoxPanel { widget: cbx3 }));
                }
                WidgetCategory::RadioWidgets => {
                    let rg = RadioGroup::new();
                    let rb1 = emRadioButton::new("Radio Button", look.clone(), rg.clone(), 0);
                    ctx.create_child_with("r1", Box::new(RadioButtonPanel { widget: rb1 }));
                    let rb2 = emRadioButton::new("Radio Button", look.clone(), rg.clone(), 1);
                    ctx.create_child_with("r2", Box::new(RadioButtonPanel { widget: rb2 }));
                    let rb3 = emRadioButton::new("Radio Button", look.clone(), rg, 2);
                    ctx.create_child_with("r3", Box::new(RadioButtonPanel { widget: rb3 }));
                    let rg2 = RadioGroup::new();
                    let rbx1 = emRadioBox::new("Radio Box", look.clone(), rg2.clone(), 0);
                    ctx.create_child_with("r4", Box::new(RadioBoxPanel { widget: rbx1 }));
                    let rbx2 = emRadioBox::new("Radio Box", look.clone(), rg2.clone(), 1);
                    ctx.create_child_with("r5", Box::new(RadioBoxPanel { widget: rbx2 }));
                    let rbx3 = emRadioBox::new("Radio Box", look, rg2, 2);
                    ctx.create_child_with("r6", Box::new(RadioBoxPanel { widget: rbx3 }));
                }
                WidgetCategory::TextFields => {
                    let mut tf1 = emTextField::new(look.clone());
                    tf1.SetEditable(false);
                    tf1.SetCaption("Read-Only");
                    tf1.SetText("Read-Only");
                    ctx.create_child_with("tf1", Box::new(TextFieldPanel { widget: tf1 }));
                    let mut tf2 = emTextField::new(look.clone());
                    tf2.SetEditable(true);
                    tf2.SetCaption("Editable");
                    tf2.SetText("Editable");
                    ctx.create_child_with("tf2", Box::new(TextFieldPanel { widget: tf2 }));
                    let mut tf3 = emTextField::new(look.clone());
                    tf3.SetEditable(true);
                    tf3.SetCaption("Password");
                    tf3.SetText("Password");
                    tf3.SetPasswordMode(true);
                    ctx.create_child_with("tf3", Box::new(TextFieldPanel { widget: tf3 }));
                    let mut tf4 = emTextField::new(look);
                    tf4.SetEditable(true);
                    tf4.SetCaption("Multi-Line");
                    tf4.SetMultiLineMode(true);
                    tf4.SetText("first line\nsecond line\n...");
                    ctx.create_child_with("mltf1", Box::new(TextFieldPanel { widget: tf4 }));
                }
                WidgetCategory::ScalarFields => {
                    let sf1 = emScalarField::new(0.0, 100.0, look.clone());
                    ctx.create_child_with("sf1", Box::new(ScalarFieldPanel { widget: sf1 }));
                    let mut sf2 = emScalarField::new(0.0, 100.0, look.clone());
                    sf2.SetEditable(true);
                    ctx.create_child_with("sf2", Box::new(ScalarFieldPanel { widget: sf2 }));
                    let mut sf3 = emScalarField::new(-1000.0, 1000.0, look.clone());
                    sf3.SetEditable(true);
                    sf3.SetScaleMarkIntervals(&[1000, 100, 10, 5, 1]);
                    ctx.create_child_with("sf3", Box::new(ScalarFieldPanel { widget: sf3 }));
                    // sf4: Level with custom formatter
                    let mut sf4 = emScalarField::new(1.0, 5.0, look.clone());
                    sf4.SetEditable(true);
                    sf4.SetTextBoxTallness(0.25);
                    sf4.SetValue(3.0);
                    sf4.SetTextOfValueFunc(Box::new(|val, _iv| format!("Level {}", val)));
                    ctx.create_child_with("sf4", Box::new(ScalarFieldPanel { widget: sf4 }));
                    // sf5: Play Length with time formatter
                    let mut sf5 = emScalarField::new(0.0, 24.0 * 3600.0 * 1000.0, look.clone());
                    sf5.SetEditable(true);
                    sf5.SetValue(4.0 * 3600.0 * 1000.0);
                    sf5.SetScaleMarkIntervals(&[
                        3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1,
                    ]);
                    sf5.SetTextOfValueFunc(Box::new(|GetValue, mark_interval| {
                        let v = GetValue;
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
                    let mut sf6 = emScalarField::new(0.0, 4.0 * 3600.0 * 1000.0, look);
                    sf6.SetEditable(true);
                    sf6.SetScaleMarkIntervals(&[
                        3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1,
                    ]);
                    sf6.SetTextOfValueFunc(Box::new(|GetValue, mark_interval| {
                        let v = GetValue;
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
                    let mut cf1 = emColorField::new(look.clone());
                    cf1.SetColor(emColor::rgba(0xBB, 0x22, 0x22, 0xFF));
                    ctx.create_child_with("cf1", Box::new(ColorFieldPanel { widget: cf1 }));
                    let mut cf2 = emColorField::new(look.clone());
                    cf2.SetEditable(true);
                    cf2.SetColor(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
                    ctx.create_child_with("cf2", Box::new(ColorFieldPanel { widget: cf2 }));
                    let mut cf3 = emColorField::new(look);
                    cf3.SetEditable(true);
                    cf3.SetAlphaEnabled(true);
                    cf3.SetColor(emColor::rgba(0x22, 0x22, 0xBB, 0xFF));
                    ctx.create_child_with("cf3", Box::new(ColorFieldPanel { widget: cf3 }));
                }
                WidgetCategory::ListBoxes => {
                    let lb1 = emListBox::new(look.clone());
                    ctx.create_child_with("l1", Box::new(ListBoxPanel { widget: lb1 }));
                    let mut lb2 = emListBox::new(look.clone());
                    lb2.SetSelectionType(SelectionMode::Single);
                    lb2.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l2", Box::new(ListBoxPanel { widget: lb2 }));
                    let mut lb3 = emListBox::new(look.clone());
                    lb3.SetSelectionType(SelectionMode::ReadOnly);
                    lb3.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l3", Box::new(ListBoxPanel { widget: lb3 }));
                    let mut lb4 = emListBox::new(look.clone());
                    lb4.SetSelectionType(SelectionMode::Multi);
                    lb4.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l4", Box::new(ListBoxPanel { widget: lb4 }));
                    let mut lb5 = emListBox::new(look.clone());
                    lb5.SetSelectionType(SelectionMode::Toggle);
                    lb5.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    ctx.create_child_with("l5", Box::new(ListBoxPanel { widget: lb5 }));
                    // l6: Single column
                    let mut lb6 = emListBox::new(look.clone());
                    lb6.SetCaption("Single Column");
                    lb6.set_fixed_column_count(Some(1));
                    lb6.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    lb6.Select(0, true);
                    ctx.create_child_with("l6", Box::new(ListBoxPanel { widget: lb6 }));
                    // l7: Custom List Box (Multi selection placeholder)
                    let mut lb7 = emListBox::new(look);
                    lb7.SetCaption("Custom List Box");
                    lb7.SetSelectionType(SelectionMode::Multi);
                    lb7.set_items((1..=7).map(|i| format!("Item {i}")).collect());
                    lb7.Select(0, true);
                    ctx.create_child_with("l7", Box::new(ListBoxPanel { widget: lb7 }));
                }
                WidgetCategory::Tunnels => {
                    // t1: default tunnel
                    let t1 = emTunnel::new(look.clone()).with_caption("Tunnel");
                    ctx.create_child_with("t1", Box::new(t1));
                    // t2: deeper tunnel
                    let mut t2 = emTunnel::new(look.clone()).with_caption("Deeper Tunnel");
                    t2.SetDepth(30.0);
                    ctx.create_child_with("t2", Box::new(t2));
                    // t3: square end
                    let mut t3 = emTunnel::new(look.clone()).with_caption("Square End");
                    t3.SetChildTallness(1.0);
                    ctx.create_child_with("t3", Box::new(t3));
                    // t4: square end + zero GetDepth
                    let mut t4 = emTunnel::new(look).with_caption("Square End, Zero Depth");
                    t4.SetChildTallness(1.0);
                    t4.SetDepth(0.0);
                    ctx.create_child_with("t4", Box::new(t4));
                }
            }
        }
        self.group.LayoutChildren(ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TkTestPanel — widget showcase (identical to test_panel.rs)
// ═══════════════════════════════════════════════════════════════════════

struct TkTestPanel {
    group: emRasterGroup,
    look: Rc<emLook>,
}

impl TkTestPanel {
    fn new(look: Rc<emLook>) -> Self {
        let mut group = emRasterGroup::new();
        group.border.caption = "Toolkit Test".to_string();
        group.border.SetBorderScaling(2.5);
        group.layout.preferred_child_tallness = 0.3;
        group.look = (*look).clone();
        Self { group, look }
    }
}

impl PanelBehavior for TkTestPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.group.Paint(p, w, h, s);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
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
        self.group.LayoutChildren(ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// SCAFFOLDING — Main (differs from test_panel.rs: mounts TkTestGrpPanel
// directly as root instead of TestPanel)
// ═══════════════════════════════════════════════════════════════════════

fn main() {
    let look = emLook::new();
    let app = App::new(Box::new(move |app, event_loop| {
        // SCAFFOLDING: mount a single TkTestPanel as root (Match C++ standalone)
        let root = app.tree.create_root("root");
        app.tree
            .set_behavior(root, Box::new(TkTestPanel::new(look.clone())));
        app.tree.Layout(root, 0.0, 0.0, 1.0, 1.0);
        app.tree
            .SetAutoExpansionThreshold(root, 900.0, ViewConditionType::Area);

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
        {
            let win = app.windows.get_mut(&wid).unwrap();
            // SCAFFOLDING: ROOT_SAME_TALLNESS Match test_panel.rs window setup
            let flags = win.view().flags | ViewFlags::ROOT_SAME_TALLNESS;
            win.view_mut().SetViewFlags(flags, &mut app.tree);
        }
    }));
    app.run();
}
