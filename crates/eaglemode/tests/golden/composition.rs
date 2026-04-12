//! Composition golden tests — full widget trees rendered through the compositor.
//!
//! These tests render complex multi-panel hierarchies (e.g., the TkTestPanel
//! widget showcase grid) and compare the composited output against C++ golden data.

use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emCursor::emCursor;
use emcore::emInput::emInputEvent;
use emcore::emInputState::emInputState;
use emcore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use emcore::emRasterGroup::emRasterGroup;
use emcore::emRasterLayout::emRasterLayout;

use emcore::emPanelCtx::PanelCtx;

use emcore::emPanelTree::{PanelId, PanelTree, ViewConditionType};

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emPainter::emPainter;
use emcore::emView::{emView, ViewFlags};
use emcore::emPainterDrawList::DrawOp;
use emcore::emViewRenderer::SoftwareCompositor;

use super::draw_op_dump::{dump_draw_ops, dump_draw_ops_enabled};

use emcore::emButton::emButton;

use emcore::emCheckBox::emCheckBox;

use emcore::emCheckButton::emCheckButton;

use emcore::emColorField::emColorField;

use emcore::emListBox::{emListBox, SelectionMode};

use emcore::emLook::emLook;

use emcore::emRadioBox::emRadioBox;

use emcore::emRadioButton::{emRadioButton, RadioGroup};

use emcore::emScalarField::emScalarField;

use emcore::emTextField::emTextField;

use emcore::emTunnel::emTunnel;

use emcore::emFileSelectionBox::emFileSelectionBox;

use emcore::emLabel::emLabel;
use emcore::emLinearGroup::emLinearGroup;

use super::common::*;

/// Skip test if golden data hasn't been generated yet.
macro_rules! require_golden {
    () => {
        if !golden_available() {
            eprintln!("SKIP: golden/ directory not found — run `make -C golden_gen run` first");
            return;
        }
    };
}

/// Settle: deliver notices, run panel cycles, and update viewing until stable.
fn settle(tree: &mut PanelTree, view: &mut emView, rounds: usize) {
    for _ in 0..rounds {
        tree.HandleNotice(view.IsFocused(), view.GetCurrentPixelTallness());
        tree.run_panel_cycles();
        view.Update(tree);
    }
}

/// Returns true if DUMP_PANEL_TREE=1 is set in the environment.
fn dump_panel_tree_enabled() -> bool {
    std::env::var("DUMP_PANEL_TREE").as_deref() == Ok("1")
}

/// Dump the full panel tree as JSONL — one line per panel.
fn dump_panel_tree(name: &str, tree: &PanelTree, root: PanelId) {
    let dir = format!(
        "{}/target/golden-divergence",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::create_dir_all(&dir).unwrap();
    let path = format!("{dir}/{name}.rust_tree.jsonl");
    let mut lines = Vec::new();
    dump_panel_recursive(tree, root, 0, &mut lines);
    std::fs::write(&path, lines.join("")).unwrap();
    eprintln!("  tree/{name} ({} panels)", lines.len());
}

fn dump_panel_recursive(
    tree: &PanelTree,
    id: PanelId,
    depth: usize,
    out: &mut Vec<String>,
) {
    let path = tree.GetIdentity(id).replace('\\', "\\\\");
    let lr = tree.layout_rect(id).unwrap();
    let child_count = tree.child_count(id);
    let ae_expanded = tree.IsAutoExpanded(id);
    let viewed = tree.IsViewed(id);
    let ae_thresh = tree.GetAutoExpansionThresholdValue(id);
    out.push(format!(
        "{{\"path\":\"{path}\",\"depth\":{depth},\
         \"lx\":{:.17},\"ly\":{:.17},\"lw\":{:.17},\"lh\":{:.17},\
         \"children\":{child_count},\"ae_expanded\":{},\"viewed\":{},\
         \"ae_thresh\":{:.17}}}\n",
        lr.x, lr.y, lr.w, lr.h,
        ae_expanded as u8,
        viewed as u8,
        ae_thresh,
    ));
    for child in tree.children(id) {
        dump_panel_recursive(tree, child, depth + 1, out);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Widget wrapper panels (same as test_panel.rs — needed for TkTestPanel)
// ═══════════════════════════════════════════════════════════════════

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
    fn auto_expand(&self) -> bool {
        true
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            self.widget.create_expansion_children(ctx);
        }
        let rect = ctx.layout_rect();
        self.widget.LayoutChildren(ctx, rect.w, rect.h);
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
    fn auto_expand(&self) -> bool {
        true
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            self.widget.create_item_children(ctx);
        }
        let rect = ctx.layout_rect();
        self.widget.layout_item_children(ctx, rect.w, rect.h);
    }
}

// ═══════════════════════════════════════════════════════════════════
/// Wraps emLabel as a PanelBehavior for use as a child panel.
struct LabelPanel {
    widget: emLabel,
}
impl PanelBehavior for LabelPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.PaintContent(p, w, h, s.enabled, pixel_scale);
    }
}

/// Custom list box item panel — matches C++ emTestPanel::CustomItemPanel.
///
/// C++ CustomItemPanel inherits emLinearGroup (horizontal, border_scaling=5.0).
/// On expand, creates "t" (label) and "l" (recursive child CustomListBox).
/// When selected, changes look bg_color to (224,80,128).
struct CustomItemPanelBehavior {
    group: emLinearGroup,
    look: Rc<emLook>,
    children_created: bool,
}

impl CustomItemPanelBehavior {
    fn new(text: String, selected: bool, look: Rc<emLook>) -> Self {
        let mut group = emLinearGroup::horizontal();
        group.border.SetBorderScaling(5.0);
        group.border.caption = text.clone();
        // C++ ItemSelectionChanged: if selected, set look bg to (224,80,128)
        if selected {
            let mut item_look = (*look).clone();
            item_look.bg_color = emColor::rgb(224, 80, 128);
            group.look = item_look;
        } else {
            group.look = (*look).clone();
        }
        Self {
            group,
            look,
            children_created: false,
        }
    }
}

impl PanelBehavior for CustomItemPanelBehavior {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        self.group.Paint(p, w, h, s);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if !self.children_created {
            self.children_created = true;

            // C++: label = new emLabel(this, "t", "This is a custom list\n...")
            let label = emLabel::new(
                "This is a custom list\nbox item panel (it is\nrecursive...)",
                self.look.clone(),
            );
            ctx.create_child_with("t", Box::new(LabelPanel { widget: label }));

            // C++: listBox = new CustomListBox(this, "l", "Child List Box")
            let mut child_lb = emListBox::new(self.look.clone());
            child_lb.SetCaption("Child List Box");
            child_lb.SetSelectionType(SelectionMode::Multi);
            for i in 1..=7 {
                child_lb.AddItem(format!("{i}"), format!("Item {i}"));
            }
            child_lb.SetSelectedIndex(0);
            // Recursive: child listbox items also use CustomItemPanelBehavior
            child_lb.set_item_behavior_factory(
                move |_i, text, selected, look, _sel_mode, _enabled| {
                    Box::new(CustomItemPanelBehavior::new(
                        text.to_string(),
                        selected,
                        look,
                    ))
                },
            );
            ctx.create_child_with("l", Box::new(ListBoxPanel { widget: child_lb }));
        }
        // Delegate layout to the emLinearGroup
        self.group.LayoutChildren(ctx);
    }
}

// TkTestPanel — widget showcase grid (from test_panel.rs)
// ═══════════════════════════════════════════════════════════════════

struct TkTestPanel {
    look: Rc<emLook>,
    border: emBorder,
    layout: emRasterLayout,
    children_created: bool,
}

impl TkTestPanel {
    fn new(look: Rc<emLook>) -> Self {
        let border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Toolkit Test");
        let mut layout = emRasterLayout::new();
        layout.preferred_child_tallness = 0.3;
        Self {
            look,
            border,
            layout,
            children_created: false,
        }
    }

    /// Helper: create a emRasterGroup category under `parent_context`.
    fn make_category(
        tree: &mut PanelTree,
        parent_context: PanelId,
        name: &str,
        caption: &str,
        pct: Option<f64>,
        fixed_cols: Option<usize>,
    ) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(2.5);
        rg.border.caption = caption.to_string();
        if let Some(p) = pct {
            rg.layout.preferred_child_tallness = p;
        }
        if let Some(c) = fixed_cols {
            rg.layout.fixed_columns = Some(c);
        }
        let id = tree.create_child(parent_context, name);
        tree.set_behavior(id, Box::new(rg));
        id
    }

    fn create_all_categories(&self, ctx: &mut PanelCtx) {
        let look = self.look.clone();

        // 1. Buttons (C++ emTestPanel.cpp:558-576)
        let gid = Self::make_category(ctx.tree, ctx.id, "buttons", "Buttons", None, None);
        {
            let id = ctx.tree.create_child(gid, "b1");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Button", look.clone()),
                }),
            );

            let mut b2 = emButton::new("Long Desc", look.clone());
            {
                // C++ emTestPanel.cpp:560-563 — 100 repetitions of a long line
                let mut desc = String::new();
                for _ in 0..100 {
                    desc.push_str("This is a looooooooooooooooooooooooooooooooooooooooooooooooooooooong description of the button.\n");
                }
                b2.SetDescription(&desc);
            }
            let id = ctx.tree.create_child(gid, "b2");
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b2 }));

            let mut b3 = emButton::new("NoEOI", look.clone());
            b3.SetNoEOI(true);
            let id = ctx.tree.create_child(gid, "b3");
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b3 }));
        }

        // 2. Check Buttons and Boxes (C++ :578-598)
        let gid = Self::make_category(
            ctx.tree,
            ctx.id,
            "checkbuttons",
            "Check Buttons and Boxes",
            None,
            None,
        );
        {
            for i in 1..=3 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(CheckButtonPanel {
                        widget: emCheckButton::new("Check Button", look.clone()),
                    }),
                );
            }
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(CheckBoxPanel {
                        widget: emCheckBox::new("Check Box", look.clone()),
                    }),
                );
            }
        }

        // 3. Radio Buttons and Boxes (C++ :600-624)
        let gid = Self::make_category(
            ctx.tree,
            ctx.id,
            "radiobuttons",
            "Radio Buttons and Boxes",
            None,
            None,
        );
        {
            let rg = RadioGroup::new();
            for i in 1..=3 {
                let id = ctx.tree.create_child(gid, &format!("r{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(RadioButtonPanel {
                        widget: emRadioButton::new("Radio Button", look.clone(), rg.clone(), i - 1),
                    }),
                );
            }
            let rg2 = RadioGroup::new();
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("r{i}"));
                ctx.tree.set_behavior(
                    id,
                    Box::new(RadioBoxPanel {
                        widget: emRadioBox::new("Radio Box", look.clone(), rg2.clone(), i - 4),
                    }),
                );
            }
        }

        // 4. Text Fields (C++ :586-609)
        let gid = Self::make_category(ctx.tree, ctx.id, "textfields", "Text Fields", None, None);
        {
            let mut tf1 = emTextField::new(look.clone());
            tf1.SetCaption("Read-Only");

            tf1.SetText("Read-Only");
            let id = ctx.tree.create_child(gid, "tf1");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf1 }));

            let mut tf2 = emTextField::new(look.clone());
            tf2.SetCaption("Editable");

            tf2.SetEditable(true);
            tf2.SetText("Editable");
            let id = ctx.tree.create_child(gid, "tf2");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf2 }));

            let mut tf3 = emTextField::new(look.clone());
            tf3.SetCaption("Password");

            tf3.SetEditable(true);
            tf3.SetText("Password");
            tf3.SetPasswordMode(true);
            let id = ctx.tree.create_child(gid, "tf3");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: tf3 }));

            let mut mltf1 = emTextField::new(look.clone());
            mltf1.SetCaption("Multi-Line");

            mltf1.SetEditable(true);
            mltf1.SetMultiLineMode(true);
            mltf1.SetText("first line\nsecond line\n...");
            let id = ctx.tree.create_child(gid, "mltf1");
            ctx.tree
                .set_behavior(id, Box::new(TextFieldPanel { widget: mltf1 }));
        }

        // 5. Scalar Fields (C++ :658-712)
        let gid = Self::make_category(
            ctx.tree,
            ctx.id,
            "scalarfields",
            "Scalar Fields",
            Some(0.1),
            None,
        );
        {
            let mut sf1 = emScalarField::new(0.0, 100.0, look.clone());
            sf1.SetCaption("Read-Only");
            let id = ctx.tree.create_child(gid, "sf1");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf1 }));

            let mut sf2 = emScalarField::new(0.0, 100.0, look.clone());
            sf2.SetCaption("Editable");
            sf2.SetEditable(true);
            let id = ctx.tree.create_child(gid, "sf2");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf2 }));

            let mut sf3 = emScalarField::new(-1000.0, 1000.0, look.clone());
            sf3.SetEditable(true);
            sf3.SetScaleMarkIntervals(&[1000, 100, 10, 5, 1]);
            let id = ctx.tree.create_child(gid, "sf3");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf3 }));

            let mut sf4 = emScalarField::new(1.0, 5.0, look.clone());
            sf4.SetCaption("Level");
            sf4.SetEditable(true);
            sf4.SetValue(3.0);
            sf4.SetTextBoxTallness(0.25);
            sf4.SetTextOfValueFunc(Box::new(|val, _interval| format!("Level {val}")));
            let id = ctx.tree.create_child(gid, "sf4");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf4 }));

            let mut sf5 = emScalarField::new(0.0, 86400000.0, look.clone());
            sf5.SetCaption("Play Length");
            sf5.SetEditable(true);
            sf5.SetValue(14400000.0);
            // C++ emTestPanel.cpp:636
            sf5.SetScaleMarkIntervals(&[3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1]);
            sf5.SetTextOfValueFunc(Box::new(|val, _interval| {
                let ms = val.unsigned_abs();
                let s = ms / 1000;
                let m = s / 60;
                let h = m / 60;
                format!("{:02}:{:02}:{:02}", h, m % 60, s % 60)
            }));
            let id = ctx.tree.create_child(gid, "sf5");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf5 }));

            let mut sf6 = emScalarField::new(0.0, 14400000.0, look.clone());
            sf6.SetCaption("Play Position");
            sf6.SetEditable(true);
            // C++ emTestPanel.cpp:643
            sf6.SetScaleMarkIntervals(&[3600000, 900000, 300000, 60000, 10000, 1000, 100, 10, 1]);
            sf6.SetTextOfValueFunc(Box::new(|val, _interval| {
                let ms = val.unsigned_abs();
                let s = ms / 1000;
                let m = s / 60;
                let h = m / 60;
                format!("{:02}:{:02}:{:02}", h, m % 60, s % 60)
            }));
            let id = ctx.tree.create_child(gid, "sf6");
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf6 }));
        }

        // 6. emColor Fields (C++ :714-733)
        let gid = Self::make_category(
            ctx.tree,
            ctx.id,
            "colorfields",
            "Color Fields",
            Some(0.4),
            None,
        );
        {
            let mut cf1 = emColorField::new(look.clone());
            cf1.SetCaption("Read-Only");
            cf1.SetColor(emColor::rgba(0xBB, 0x22, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf1");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf1 }));
            // C++ emColorField.cpp:36: SetAutoExpansionThreshold(9,VCT_MIN_EXT)
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);

            let mut cf2 = emColorField::new(look.clone());
            cf2.SetCaption("Editable");
            cf2.SetEditable(true);
            cf2.SetColor(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf2");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf2 }));
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);

            let mut cf3 = emColorField::new(look.clone());
            cf3.SetCaption("Editable, Alpha Enabled");
            cf3.SetEditable(true);
            cf3.SetAlphaEnabled(true);
            cf3.SetColor(emColor::rgba(0x22, 0x22, 0xBB, 0xFF));
            let id = ctx.tree.create_child(gid, "cf3");
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf3 }));
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);
        }

        // 7. Tunnels (C++ emTestPanel.cpp:662-680)
        let gid = Self::make_category(ctx.tree, ctx.id, "tunnels", "Tunnels", Some(0.4), None);
        {
            // t1: default tunnel (depth=10, childTallness=0)
            let tid = ctx.tree.create_child(gid, "t1");
            let t1 = emTunnel::new(look.clone()).with_caption("Tunnel");
            ctx.tree.set_behavior(tid, Box::new(t1));
            let child = ctx.tree.create_child(tid, "e");
            ctx.tree.set_behavior(
                child,
                Box::new(ButtonPanel {
                    widget: emButton::new("End Of Tunnel", look.clone()),
                }),
            );

            // t2: deeper tunnel (depth=30)
            let tid = ctx.tree.create_child(gid, "t2");
            let mut t2 = emTunnel::new(look.clone()).with_caption("Deeper Tunnel");
            t2.SetDepth(30.0);
            ctx.tree.set_behavior(tid, Box::new(t2));
            let child = ctx.tree.create_child(tid, "e");
            ctx.tree.set_behavior(child, {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                Box::new(rg)
            });

            // t3: square end (childTallness=1.0)
            let tid = ctx.tree.create_child(gid, "t3");
            let mut t3 = emTunnel::new(look.clone()).with_caption("Square End");
            t3.SetChildTallness(1.0);
            ctx.tree.set_behavior(tid, Box::new(t3));
            let child = ctx.tree.create_child(tid, "e");
            ctx.tree.set_behavior(child, {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                Box::new(rg)
            });

            // t4: square end, zero depth
            let tid = ctx.tree.create_child(gid, "t4");
            let mut t4 = emTunnel::new(look.clone()).with_caption("Square End, Zero Depth");
            t4.SetChildTallness(1.0);
            t4.SetDepth(0.0);
            ctx.tree.set_behavior(tid, Box::new(t4));
            let child = ctx.tree.create_child(tid, "e");
            ctx.tree.set_behavior(child, {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                Box::new(rg)
            });
        }

        // 8. List Boxes (C++ :756-798)
        let gid = Self::make_category(ctx.tree, ctx.id, "listboxes", "List Boxes", Some(0.4), None);
        {
            // Helper: add items with numeric names matching C++
            // AddItem(Format("%d",i), Format("Item %d",i))
            fn add_items_1_to_7(lb: &mut emListBox) {
                for i in 1..=7 {
                    lb.AddItem(format!("{i}"), format!("Item {i}"));
                }
            }

            // C++ emTestPanel.cpp:686-731
            let mut lb1 = emListBox::new(look.clone());
            lb1.SetCaption("Empty");
            let id = ctx.tree.create_child(gid, "l1");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb1 }));

            let mut lb2 = emListBox::new(look.clone());
            lb2.SetCaption("Single-Selection");
            lb2.SetSelectionType(SelectionMode::Single);
            add_items_1_to_7(&mut lb2);
            lb2.SetSelectedIndex(0);
            let id = ctx.tree.create_child(gid, "l2");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb2 }));

            let mut lb3 = emListBox::new(look.clone());
            lb3.SetCaption("Read-Only");
            lb3.SetSelectionType(SelectionMode::ReadOnly);
            add_items_1_to_7(&mut lb3);
            lb3.SetSelectedIndex(2);
            let id = ctx.tree.create_child(gid, "l3");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb3 }));

            let mut lb4 = emListBox::new(look.clone());
            lb4.SetCaption("Multi-Selection");
            lb4.SetSelectionType(SelectionMode::Multi);
            add_items_1_to_7(&mut lb4);
            lb4.Select(1, false);
            lb4.Select(2, false);
            lb4.Select(3, false);
            lb4.Select(4, false);
            let id = ctx.tree.create_child(gid, "l4");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb4 }));

            let mut lb5 = emListBox::new(look.clone());
            lb5.SetCaption("Toggle-Selection");
            lb5.SetSelectionType(SelectionMode::Toggle);
            add_items_1_to_7(&mut lb5);
            lb5.Select(2, false);
            lb5.Select(4, false);
            let id = ctx.tree.create_child(gid, "l5");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb5 }));

            let mut lb6 = emListBox::new(look.clone());
            lb6.SetCaption("Single Column");
            lb6.SetSelectionType(SelectionMode::Single);
            add_items_1_to_7(&mut lb6);
            lb6.set_fixed_column_count(Some(1));
            lb6.SetSelectedIndex(0);
            let id = ctx.tree.create_child(gid, "l6");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb6 }));

            // l7: custom list box — C++ CustomListBox with CustomItemPanel items
            let mut lb7 = emListBox::new(look.clone());
            lb7.SetCaption("Custom List Box");
            lb7.SetSelectionType(SelectionMode::Multi);
            add_items_1_to_7(&mut lb7);
            lb7.SetSelectedIndex(0);
            lb7.set_item_behavior_factory(
                move |_i, text, selected, look, _sel_mode, _enabled| {
                    Box::new(CustomItemPanelBehavior::new(
                        text.to_string(),
                        selected,
                        look,
                    ))
                },
            );
            let id = ctx.tree.create_child(gid, "l7");
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb7 }));
        }

        // 9. Test emDialog (C++ :800-831)
        let gid = Self::make_category(ctx.tree, ctx.id, "dlgs", "Test Dialog", None, Some(1));
        {
            let mut rl = emRasterLayout::new();
            rl.preferred_child_tallness = 0.1;
            let rl_id = ctx.tree.create_child(gid, "rl");

            // C++ emTestPanel.cpp:738-747
            let cb_items: &[(&str, &str, bool)] = &[
                ("tl", "Top-Level", false),
                ("VF_POPUP_ZOOM", "VF_POPUP_ZOOM", true),
                ("WF_MODAL", "WF_MODAL", true),
                ("WF_UNDECORATED", "WF_UNDECORATED", false),
                ("WF_POPUP", "WF_POPUP", false),
                ("WF_MAXIMIZED", "WF_MAXIMIZED", false),
                ("WF_FULLSCREEN", "WF_FULLSCREEN", false),
            ];
            for &(name, caption, checked) in cb_items {
                let id = ctx.tree.create_child(rl_id, name);
                let mut cb = emCheckBox::new(caption, look.clone());
                if checked {
                    cb.SetChecked(true);
                }
                ctx.tree
                    .set_behavior(id, Box::new(CheckBoxPanel { widget: cb }));
            }
            ctx.tree.set_behavior(rl_id, Box::new(rl));

            let id = ctx.tree.create_child(gid, "bt");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Create Test Dialog", look.clone()),
                }),
            );
        }

        // 10. File Selection (C++ :750-764)
        let gid = Self::make_category(
            ctx.tree,
            ctx.id,
            "fileChoosers",
            "File Selection",
            Some(0.3),
            None,
        );
        {
            let id = ctx.tree.create_child(gid, "l8");
            let mut fsb = emFileSelectionBox::new("File Selection Box");
            fsb.set_filters(&[
                "All Files (*)".to_string(),
                "Image Files (*.bmp *.gif *.jpg *.png *.tga)".to_string(),
                "HTML Files (*.htm *.html)".to_string(),
            ]);
            // C++ gen_golden runs with CWD=crates/eaglemode/ — match that.
            fsb.set_parent_directory(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
            ctx.tree.set_behavior(id, Box::new(fsb));

            // C++ emTestPanel.cpp:759-763
            let id = ctx.tree.create_child(gid, "openFile");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Open...", look.clone()),
                }),
            );

            let id = ctx.tree.create_child(gid, "openFiles");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Open Multi, Allow Dir...", look.clone()),
                }),
            );

            let id = ctx.tree.create_child(gid, "saveFile");
            ctx.tree.set_behavior(
                id,
                Box::new(ButtonPanel {
                    widget: emButton::new("Save As...", look.clone()),
                }),
            );
        }
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
        self.border
            .paint_border(p, w, h, &self.look, s.is_focused(), s.enabled, 1.0);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        if !self.children_created {
            self.children_created = true;
            // Create all 10 category groups as direct children (C++ TkTest
            // inherits from emRasterGroup, so categories are direct children)
            self.create_all_categories(ctx);
        }

        // Position children using raster layout in border content rect
        // (matches C++ emRasterGroup::LayoutChildren → emRasterLayout)
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        self.layout.do_layout_skip(ctx, None, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

/// TkTestPanel (all widget types in emRasterGroup grid) at 1x zoom (800x600).
/// Matches C++ gen_tktest_1x(): TkTest panel with layout (0, 0, 800/600, 1.0),
/// viewport 800x600, 200 settle rounds, unfocused window.
#[test]
fn composition_tktest_1x() {
    require_golden!();
    let expected = load_compositor_golden("tktest_1x");
    let (w, h, ref expected_data) = expected;

    let look = emLook::new();
    let mut tree = PanelTree::new();
    let root = tree.create_root("tktest");
    tree.set_behavior(root, Box::new(TkTestPanel::new(look)));
    // C++ gen: tk->Layout(0, 0, 800.0/600.0, 1.0)
    tree.Layout(root, 0.0, 0.0, 800.0 / 600.0, 1.0);
    // C++ default auto-expansion threshold for TkTest
    tree.SetAutoExpansionThreshold(root, 900.0, ViewConditionType::Area);

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    settle(&mut tree, &mut view, 200);

    if dump_panel_tree_enabled() {
        dump_panel_tree("tktest_1x", &tree, root);
    }

    if dump_draw_ops_enabled() {
        let mut ops: Vec<DrawOp> = Vec::new();
        {
            let mut rec = emPainter::new_recording(w, h, &mut ops);
            view.Paint(&mut tree, &mut rec, emColor::TRANSPARENT);
        }
        dump_draw_ops("tktest_1x", &ops);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    let result = compare_images("tktest_1x", actual, expected_data, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("tktest_1x", actual, expected_data, w, h);
        analyze_diff_distribution(actual, expected_data, w, h, 3);
    }
    result.unwrap();
}

/// TkTestPanel (all widget types in emRasterGroup grid) at 2x zoom (800x600).
/// Matches C++ gen_tktest_2x(): same TkTest panel as 1x, then Zoom(400, 300, 2.0)
/// to show the middle 50% of the panel. Catches Restore rounding at non-1x zoom.
#[test]
fn composition_tktest_2x() {
    require_golden!();
    let expected = load_compositor_golden("tktest_2x");
    let (w, h, ref expected_data) = expected;

    let look = emLook::new();
    let mut tree = PanelTree::new();
    let root = tree.create_root("tktest");
    tree.set_behavior(root, Box::new(TkTestPanel::new(look)));
    // C++ gen: tk->Layout(0, 0, 800.0/600.0, 1.0)
    tree.Layout(root, 0.0, 0.0, 800.0 / 600.0, 1.0);
    // C++ default auto-expansion threshold for TkTest
    tree.SetAutoExpansionThreshold(root, 900.0, ViewConditionType::Area);

    let mut view = emView::new(root, 800.0, 600.0);
    view.flags.insert(ViewFlags::NO_ACTIVE_HIGHLIGHT);
    // C++ golden gen doesn't focus the window
    view.SetFocused(&mut tree, false);

    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 200)
    settle(&mut tree, &mut view, 200);

    // C++ gen_golden.cpp: view.Zoom(400, 300, 2.0)
    // Rust emView::Zoom(factor, center_x, center_y)
    view.Zoom(2.0, 400.0, 300.0);
    // C++ gen_golden.cpp: TerminateEngine ctrl(sched, 10)
    settle(&mut tree, &mut view, 10);

    if dump_draw_ops_enabled() {
        let mut ops: Vec<DrawOp> = Vec::new();
        {
            let mut rec = emPainter::new_recording(w, h, &mut ops);
            view.Paint(&mut tree, &mut rec, emColor::TRANSPARENT);
        }
        dump_draw_ops("tktest_2x", &ops);
    }

    let mut compositor = SoftwareCompositor::new(w, h);
    compositor.render(&mut tree, &view);
    let actual = compositor.framebuffer().GetMap();

    // TkTestPanel at 2x zoom amplifies layout GetPos differences.
    // Zoom shifts expose border-rounding rects that differ from C++ at sub-pixel level.
    let result = compare_images("tktest_2x", actual, expected_data, w, h, 0, 0.0);
    if result.is_err() && dump_golden_enabled() {
        dump_test_images("tktest_2x", actual, expected_data, w, h);
        analyze_diff_distribution(actual, expected_data, w, h, 3);
    }
    result.unwrap();
}
