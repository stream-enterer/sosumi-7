//! emTestPanel — plugin port of C++ emTest/emTestPanel.cpp.
//!
//! Provides TestPanel + TkTestGrp + TkTest with the full widget set
//! (Buttons, Check, Radio, Text, Scalar fields, Color, Tunnels, ListBoxes,
//! dialogs, file selection) plus the PolyDrawPanel container. BgColor is
//! persisted via emVarModel keyed on the panel identity; the teddy.tga test
//! image is loaded from embedded resources.

use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;

use emcore::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use emcore::emButton::emButton;
use emcore::emCheckBox::emCheckBox;
use emcore::emCheckButton::emCheckButton;
use emcore::emColor::emColor;
use emcore::emColorField::emColorField;
use emcore::emContext::emContext;
use emcore::emCursor::emCursor;
use emcore::emEngineCtx::{ConstructCtx, EngineCtx, PanelCtx};
use emcore::emFileDialog::{
    emFileDialog, get_selected_names_post_show, get_selected_path_post_show, FileDialogMode,
};
use emcore::emFileSelectionBox::emFileSelectionBox;
use emcore::emImage::emImage;
use emcore::emInput::{emInputEvent, InputKey, InputVariant};
use emcore::emInputState::emInputState;
use emcore::emLabel::emLabel;
use emcore::emLinearGroup::emLinearGroup;
use emcore::emLinearLayout::emLinearLayout;
use emcore::emListBox::{emListBox, SelectionMode};
use emcore::emLook::emLook;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{ItemInputResult, NoticeFlags, PanelBehavior, PanelState};
use emcore::emPanelTree::{PanelId, ViewConditionType};
use emcore::emRadioBox::emRadioBox;
use emcore::emRadioButton::{emRadioButton, RadioGroup};
use emcore::emRasterGroup::emRasterGroup;
use emcore::emRasterLayout::emRasterLayout;
use emcore::emRes::emGetInsResImage;
use emcore::emScalarField::emScalarField;
use emcore::emSignal::SignalId;
use emcore::emStroke::{emStroke, DashType, LineCap, LineJoin};
use emcore::emStrokeEnd::{emStrokeEnd, StrokeEndType};
use emcore::emTextField::emTextField;
use emcore::emTexture::{emTexture, ImageExtension, ImageQuality};
use emcore::emTiling::{AlignmentH, AlignmentV, ChildConstraint};
use emcore::emTunnel::emTunnel;
use emcore::emVarModel;
use emcore::emView::ViewFlags;
use emcore::emWindow::WindowFlags;

// ─── constants ──────────────────────────────────────────────────────

const MAX_LOG_ENTRIES: usize = 20;
const DEFAULT_BG: emColor = emColor::rgba(0x00, 0x1C, 0x38, 0xFF);

const CHILD_LAYOUT: [(&str, f64, f64, f64, f64); 7] = [
    ("TkTestGrp", 0.20, 0.15, 0.30, 0.12),
    ("1", 0.70, 0.05, 0.12, 0.12),
    ("2", 0.83, 0.05, 0.12, 0.12),
    ("3", 0.70, 0.18, 0.12, 0.12),
    ("4", 0.83, 0.18, 0.12, 0.12),
    ("BgColorField", 0.775, 0.34, 0.10, 0.02),
    ("PolyDraw", 0.05, 0.92, 0.08, 0.04),
];

// ─── widget wrapper PanelBehaviors ──────────────────────────────────

struct ButtonPanel {
    widget: emButton,
}
impl PanelBehavior for ButtonPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
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
    is_focused: bool,
}
impl PanelBehavior for TextFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Cycle(&mut self, _ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
        let r = self.widget.cycle_blink(self.is_focused);
        if r.flipped {
            pctx.request_invalidate_self();
        }
        r.busy
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.is_focused = state.in_focused_path();
            self.widget.on_focus_changed(self.is_focused);
            // Mirrors C++ emTextField::Notice (emTextField.cpp:343-350):
            // RestartCursorBlinking + WakeUp guarded by IsInFocusedPath().
            if self.is_focused {
                self.widget.RestartCursorBlinking();
                let id = ctx.id;
                ctx.wake_up_panel(id);
            }
        }
    }
}

struct ScalarFieldPanel {
    widget: emScalarField,
}
impl PanelBehavior for ScalarFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}

/// sf6 "Play Position" wrapper. Mirrors C++ `TkTest::Cycle` behavior at
/// emTestPanel.cpp:786 —
///   `if (IsSignaled(SFLen->GetValueSignal())) SFPos->SetMaxValue(SFLen->GetValue());`
/// — but relocated from `TkTest::Cycle` to sf6's own `Cycle`.
///
/// DIVERGED: (language-forced) C++ calls `SFPos->SetMaxValue(SFLen->GetValue())` from
/// `TkTest::Cycle`. In Rust, `SetMaxValue` requires `&mut PanelCtx`. Moving this logic
/// to `TkTestPanel::Cycle` would require threading `&mut PanelCtx` through a
/// `with_behavior_as` closure that already holds `&mut PanelTree` — a borrow conflict
/// the compiler rejects. Placing the logic in sf6's own `Cycle` resolves this: `Cycle`
/// receives both `&mut EngineCtx` and `&mut PanelCtx` directly, so `SetMaxValue` can
/// be called without aliasing the tree borrow. The sf5 value is read via a parent-walk
/// tree lookup — no Cell intermediary.
struct ScalarFieldWithDynamicMax {
    widget: emScalarField,
    sf5_len_signal: SignalId,
    signal_connected: bool,
}

impl PanelBehavior for ScalarFieldWithDynamicMax {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .Paint(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn GetCursor(&self) -> emCursor {
        self.widget.GetCursor()
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
        if !self.signal_connected {
            ectx.connect(self.sf5_len_signal, ectx.engine_id);
            self.signal_connected = true;
            // Trigger one wake so initial connection observes any pending fire.
            ectx.wake_up(ectx.engine_id);
        }
        if ectx.IsSignaled(self.sf5_len_signal) {
            // Read sf5's current value directly — walk to parent, find "sf5" by name.
            // C++: SFPos->SetMaxValue(SFLen->GetValue()) — direct pointer; Rust uses
            // panel-tree lookup because sibling widgets are tree-owned.
            //
            // sf5 is a sibling created in the same AutoExpand — None only if tree is
            // partially constructed. This None path is unreachable in normal operation
            // since sf6 cannot exist without sf5.
            let sf5_val = pctx
                .GetParentContext()
                .and_then(|parent| pctx.tree.find_child_by_name(parent, "sf5"))
                .and_then(|sf5_id| {
                    pctx.tree
                        .with_behavior_as::<ScalarFieldPanel, _>(sf5_id, |p| p.widget.GetValue())
                });
            if let Some(max) = sf5_val {
                self.widget.SetMaxValue(max, pctx);
            }
        }
        false
    }
}

struct ColorFieldPanel {
    widget: emColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, canvas_color, w, h, pixel_scale);
    }
    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.widget.create_expansion_children(ctx);
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.widget.LayoutChildren(ctx, rect.w, rect.h);
    }
    fn Cycle(&mut self, _ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
        self.widget.Cycle(ctx)
    }
}

/// Wraps emLabel as a control-panel child.
struct LabelPanel {
    widget: emLabel,
}
impl PanelBehavior for LabelPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget
            .PaintContent(p, canvas_color, w, h, s.enabled, pixel_scale);
    }
}

// ─── ListBoxPanel ───────────────────────────────────────────────────

/// Wraps emListBox as a PanelBehavior child. Port of C++ emListBox usage
/// pattern where the list box is a child of an emRasterGroup.
struct ListBoxPanel {
    widget: emListBox,
}

impl PanelBehavior for ListBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, canvas_color, w, h, pixel_scale);
    }

    fn Input(
        &mut self,
        e: &emInputEvent,
        s: &PanelState,
        is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.widget.Input(e, s, is, ctx)
    }

    fn GetCursor(&self) -> emCursor {
        emCursor::Normal
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        self.widget.create_item_children(ctx);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        self.widget.layout_item_children(ctx, rect.w, rect.h);
    }

    fn dispatch_item_input(
        &mut self,
        child_panel_id: PanelId,
        event: &emInputEvent,
        state: &PanelState,
        ctx: &mut PanelCtx,
    ) -> ItemInputResult {
        if let Some(idx) = self.widget.item_index_for_child(child_panel_id) {
            self.widget.process_item_input(idx, event, state, ctx)
        } else {
            ItemInputResult::default()
        }
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.widget.on_focus_changed(state.in_focused_path());
        }
        if flags.intersects(NoticeFlags::ENABLE_CHANGED) {
            self.widget.on_enable_changed(state.enabled);
        }
    }
}

/// Custom item panel behavior for l7's CustomListBox.
///
/// Port of C++ emTestPanel::CustomItemPanel::ItemSelectionChanged, which calls
/// `SetLook` with a tinted bg color (emColor(224,80,128)) when selected, or
/// restores the list box's default look when deselected. C++ lines 970-981.
struct CustomItemBehavior {
    text: String,
    selected: bool,
    look: Rc<emLook>,
}

impl PanelBehavior for CustomItemBehavior {
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        // When selected, use a rose bg (matching C++ emColor(224,80,128)).
        // When not selected, use the standard look bg color.
        let bg = if self.selected {
            emColor::rgba(224, 80, 128, 0xFF)
        } else {
            self.look.input_bg_color
        };
        p.PaintRect(0.0, 0.0, w, h, bg, canvas_color);
        // Paint item text — matches C++ caption (set by ItemTextChanged).
        let text_h = h * 0.7;
        p.PaintTextBoxed(
            0.0,
            0.0,
            w,
            h,
            &self.text,
            text_h,
            self.look.fg_color,
            emColor::TRANSPARENT,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            true,
            0.15,
        );
        let _ = pixel_scale;
    }

    fn IsOpaque(&self) -> bool {
        true
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        _is: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        let child_id = ctx.id;
        let result = ctx
            .with_parent_behavior(|parent, ctx| {
                parent.dispatch_item_input(child_id, event, state, ctx)
            })
            .unwrap_or_default();
        if result.focus_self {
            ctx.request_focus();
        }
        result.consumed
    }

    fn on_item_text_changed(&mut self, text: &str) {
        self.text = text.to_string();
    }

    /// C++ CustomItemPanel::AutoExpand (cpp:941–956): recursive label + list box.
    ///
    /// In C++ each CustomItemPanel inherits from emLinearGroup and creates:
    /// 1. An emLabel "t" with the listbox's look and the item text.
    /// 2. A recursive CustomListBox "l" with the same look, multi-selection,
    ///    items 1–7, item 0 selected.
    ///
    /// The look used here (`self.look`) is the parent listbox's look,
    /// matching `GetListBox().GetLook()` in C++.
    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // "t": label with listbox's look (cpp:943-944: new emLabel; SetLook).
        let label = emLabel::new(
            "This is a custom list\nbox item panel (it is\nrecursive...)",
            self.look.clone(),
        );
        ctx.create_child_with("t", Box::new(LabelPanel { widget: label }));

        // "l": recursive CustomListBox with parent listbox's look (cpp:945-955).
        let look = self.look.clone();
        let mut lb = emListBox::new(ctx, look.clone());
        lb.SetCaption("Child List Box");
        lb.SetSelectionType(SelectionMode::Multi);
        for i in 1..=7usize {
            lb.AddItem(format!("{i}"), format!("Item {i}"));
        }
        lb.SetSelectedIndex(0);
        lb.SetChildTallness(0.4);
        lb.SetAlignment(AlignmentH::Left, AlignmentV::Top);
        lb.SetStrictRaster();
        lb.set_item_behavior_factory(move |_idx, text, selected, _look, _sel_mode, _enabled| {
            Box::new(CustomItemBehavior {
                text: text.to_string(),
                selected,
                look: look.clone(),
            })
        });
        ctx.create_child_with("l", Box::new(ListBoxPanel { widget: lb }));
    }
}

// ─── TestPanel ──────────────────────────────────────────────────────

/// Shared bg color slot — written by TestPanel::Cycle via IsSignaled
/// (C++ emTestPanel.cpp:62–71), read by TestPanel::Paint and Drop.
type BgShared = Rc<Cell<emColor>>;

pub(crate) struct TestPanel {
    /// Root-context handle for VarModel lookups in Drop.
    root_ctx: Rc<emContext>,
    /// Per-view context for VarModel, mirroring C++ `GetView()` scope (emTestPanel.cpp:32, 47).
    /// Captured in AutoExpand when `view_context()` is available.
    /// Falls back to `root_ctx` if view context is unavailable.
    view_ctx: Option<Rc<emContext>>,
    /// `"emTestPanel - BgColor of " + identity` — populated lazily on first
    /// `AutoExpand` once the tree assigns this panel its identity path.
    /// Empty until then; Drop checks for empty before persisting.
    identity_key: String,
    bg_shared: BgShared,
    input_log: Vec<String>,
    test_image: emImage,
    /// Signal from BgColorField's color_signal — stored in AutoExpand, connected
    /// in first Cycle (C++ emTestPanel.cpp:495: AddWakeUpSignal(BgColorField->GetColorSignal())).
    /// None until AutoExpand runs.
    bg_color_signal: Option<SignalId>,
    /// True after the first Cycle has connected bg_color_signal to our engine.
    /// C++ connects via AddWakeUpSignal in AutoExpand; Rust defers to Cycle
    /// because EngineCtx::connect is not available from PanelCtx.
    bg_signal_connected: bool,
}

impl TestPanel {
    pub(crate) fn new(root_ctx: Rc<emContext>, initial_bg: emColor) -> Self {
        // DIVERGED: (dependency-forced) C++ emGetInsResImage(GetRootContext(), "icons", "teddy.tga")
        // resolves to $EM_DIR/res/icons/teddy.tga (monolithic install). Rust cdylib resources
        // live under res/emTest/; call uses ("emTest", "icons/teddy.tga") → $EM_DIR/res/emTest/icons/teddy.tga.
        let test_image = emGetInsResImage("emTest", "icons/teddy.tga");
        Self {
            root_ctx,
            view_ctx: None,
            identity_key: String::new(),
            bg_shared: Rc::new(Cell::new(initial_bg)),
            input_log: Vec::new(),
            test_image,
            bg_color_signal: None,
            bg_signal_connected: false,
        }
    }

    fn bg_color(&self) -> emColor {
        self.bg_shared.get()
    }

    fn paint_primitives(&self, p: &mut emPainter, fg: emColor, bg: emColor) {
        // Text test with tabs
        p.PaintTextBoxed(
            0.25,
            0.80,
            0.05,
            0.05,
            "Text Test\n\t<-tab\ntab->\t<-tab",
            0.1,
            fg,
            bg,
            TextAlignment::Center,
            VAlign::Center,
            TextAlignment::Left,
            0.2,
            true,
            0.1,
        );
        p.PaintRect(
            0.25,
            0.80,
            0.05,
            0.05,
            emColor::rgba(255, 0, 0, 32),
            emColor::TRANSPARENT,
        );

        p.PaintPolygon(&[(0.7, 0.6), (0.6, 0.7), (0.8, 0.8)], fg, bg);

        // Holed polygon (even-odd winding) — C++ PaintPolygon 10-pt outer+inner square.
        p.paint_polygon_even_odd(
            &[
                (0.90, 0.90),
                (0.94, 0.90),
                (0.94, 0.94),
                (0.90, 0.94),
                (0.90, 0.90),
                (0.91, 0.91),
                (0.93, 0.91),
                (0.93, 0.93),
                (0.91, 0.93),
                (0.91, 0.91),
            ],
            emColor::rgba(255, 255, 255, 128),
            bg,
        );

        // Holed polygon (non-zero winding, reversed inner).
        p.PaintPolygon(
            &[
                (0.80, 0.90),
                (0.84, 0.90),
                (0.84, 0.94),
                (0.80, 0.94),
                (0.80, 0.90),
                (0.81, 0.91),
                (0.81, 0.93),
                (0.83, 0.93),
                (0.83, 0.91),
                (0.81, 0.91),
            ],
            emColor::WHITE,
            bg,
        );

        let circle: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.65, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle, emColor::YELLOW, bg);

        // DIVERGED: (language-forced) C++ creates a sub-painter with restricted origin/scale
        // (emTestPanel.cpp:225–231) — both clips AND re-maps coordinate origin. Rust cannot
        // create a sub-painter (second &mut emImage borrow forbidden by borrow checker).
        // push_state/SetClipping/pop_state clips correctly but does not shift origin.
        // Verify in golden test.
        p.push_state();
        p.SetClipping(0.51, 0.81, 0.08, 0.08);
        let circle2: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.05 + 0.55, a.cos() * 0.05 + 0.85)
            })
            .collect();
        p.PaintPolygon(&circle2, emColor::GREEN, bg);
        p.pop_state();

        // Ellipse (polygon approximation).
        let ellipse: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let a = PI * i as f64 / 32.0;
                (a.sin() * 0.06 + 0.6, a.cos() * 0.04 + 0.86)
            })
            .collect();
        p.PaintPolygon(&ellipse, emColor::rgba(255, 0, 0, 92), bg);

        p.PaintPolygon(
            &[(0.6, 0.9), (0.5, 0.92), (0.65, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            bg,
        );
        p.PaintPolygon(&[(0.6, 0.96), (0.5, 0.92), (0.65, 0.95)], emColor::RED, bg);
        p.PaintPolygon(
            &[(0.45, 0.9), (0.35, 0.92), (0.5, 0.95)],
            emColor::rgba(187, 255, 255, 255),
            bg,
        );
        p.PaintPolygon(&[(0.45, 0.96), (0.35, 0.92), (0.5, 0.95)], emColor::RED, bg);

        // Thin triangles.
        p.PaintPolygon(
            &[(0.6, 0.6), (0.602, 0.6), (0.502, 0.7)],
            emColor::rgba(187, 136, 255, 192),
            bg,
        );
        p.PaintPolygon(
            &[(0.7, 0.55), (0.702, 0.55), (0.802, 0.9), (0.8, 0.9)],
            emColor::rgba(136, 187, 255, 192),
            bg,
        );

        // Bowtie (self-intersecting quad).
        p.PaintPolygon(
            &[(0.8, 0.55), (0.9, 0.55), (0.8, 0.8), (0.9, 0.8)],
            emColor::rgba(136, 187, 255, 192),
            bg,
        );

        // Ellipses — bounding-rect (x, y, w, h). C++ emTestPanel.cpp:276-278.
        p.PaintEllipse(0.05, 0.80, 0.01, 0.01, emColor::WHITE, bg);
        p.PaintEllipse(0.06, 0.80, 0.02, 0.01, emColor::WHITE, bg);
        p.PaintEllipse(0.09, 0.80, 0.005, 0.01, emColor::WHITE, bg);

        // Ellipse sectors — C++ emTestPanel.cpp:279-282.
        p.PaintEllipseSector(0.10, 0.80, 0.01, 0.01, 45.0, 350.0, emColor::WHITE, bg);
        p.PaintEllipseSector(0.11, 0.80, 0.02, 0.01, 45.0, -350.0, emColor::WHITE, bg);
        p.PaintEllipseSector(0.13, 0.80, 0.005, 0.01, 245.0, 50.0, emColor::WHITE, bg);
        p.PaintEllipseSector(0.14, 0.80, 0.01, 0.01, 245.0, -50.0, emColor::WHITE, bg);

        // Rect outlines — C++ emTestPanel.cpp:284-287.
        p.PaintRectOutline(
            0.05,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        {
            let mut dashed_s = emStroke::new(emColor::WHITE, 0.001);
            dashed_s.dash_type = DashType::Dashed;
            p.PaintRectOutline(0.07, 0.82, 0.02, 0.01, &dashed_s, bg);
        }
        p.PaintRectOutline(
            0.10,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.008),
            bg,
        );
        p.PaintRectOutline(
            0.13,
            0.82,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.011),
            bg,
        );

        // Round rects — C++ emTestPanel.cpp:289-293.
        p.PaintRoundRect(0.05, 0.84, 0.01, 0.01, 0.001, 0.001, emColor::WHITE, bg);
        p.PaintRoundRect(0.07, 0.84, 0.02, 0.01, 0.001, 0.002, emColor::WHITE, bg);
        p.PaintRoundRect(0.10, 0.84, 0.01, 0.01, 0.003, 0.002, emColor::WHITE, bg);
        p.PaintRoundRect(0.13, 0.84, 0.01, 0.01, 0.001, 0.011, emColor::WHITE, bg);
        p.PaintRoundRect(0.15, 0.84, 0.01, 0.01, 0.0, 0.0, emColor::WHITE, bg);

        // Ellipse outlines — C++ emTestPanel.cpp:295-302.
        p.PaintEllipseOutline(
            0.05,
            0.86,
            0.01,
            0.01,
            &emStroke::new(emColor::WHITE, 0.003),
            bg,
        );
        p.PaintEllipseOutline(
            0.065,
            0.86,
            0.02,
            0.01,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        {
            // C++ :297 emRoundedDottedStroke — rounded cap/join + dotted dash pattern.
            let mut rd_s = emStroke::new(emColor::WHITE, 0.00025);
            rd_s.cap = LineCap::Round;
            rd_s.join = LineJoin::Round;
            rd_s.dash_type = DashType::Dotted;
            p.PaintEllipseOutline(0.09, 0.86, 0.005, 0.01, &rd_s, bg);
        }
        p.PaintEllipseArc(
            0.10,
            0.86,
            0.01,
            0.01,
            90.0,
            225.0,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        p.PaintEllipseSectorOutline(
            0.11,
            0.86,
            0.02,
            0.01,
            45.0,
            -320.0,
            &emStroke::new(emColor::WHITE, 0.0001),
            bg,
        );
        p.PaintEllipseArc(
            0.13,
            0.86,
            0.005,
            0.01,
            245.0,
            50.0,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        p.PaintEllipseArc(
            0.14,
            0.86,
            0.01,
            0.01,
            245.0,
            -50.0,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        {
            // C++ :302 emRoundedStroke + LineArrow finish end.
            let mut rounded_s = emStroke::new(emColor::WHITE, 0.0001);
            rounded_s.cap = LineCap::Round;
            rounded_s.join = LineJoin::Round;
            rounded_s.start_end = emStrokeEnd::new(StrokeEndType::Cap);
            rounded_s.finish_end = emStrokeEnd::new(StrokeEndType::LineArrow);
            p.PaintEllipseArc(0.15, 0.86, 0.01, 0.01, 0.0, -145.0, &rounded_s, bg);
        }

        // Round rect outlines — C++ emTestPanel.cpp:304-309.
        p.PaintRoundRectOutline(
            0.05,
            0.88,
            0.01,
            0.01,
            0.001,
            0.001,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        p.PaintRoundRectOutline(
            0.07,
            0.88,
            0.02,
            0.01,
            0.001,
            0.002,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );
        p.PaintRoundRectOutline(
            0.10,
            0.88,
            0.01,
            0.01,
            0.003,
            0.002,
            &emStroke::new(emColor::WHITE, 0.003),
            bg,
        );
        p.PaintRoundRectOutline(
            0.12,
            0.88,
            0.01,
            0.01,
            0.001,
            0.011,
            &emStroke::new(emColor::WHITE, 0.0001),
            bg,
        );
        {
            let mut dd_s = emStroke::new(emColor::WHITE, 0.00002);
            dd_s.dash_type = DashType::DashDotted;
            p.PaintRoundRectOutline(0.135, 0.88, 0.01, 0.01, 0.001, 0.001, &dd_s, bg);
        }
        p.PaintRoundRectOutline(
            0.15,
            0.88,
            0.01,
            0.01,
            -0.0004,
            -0.0004,
            &emStroke::new(emColor::WHITE, 0.001),
            bg,
        );

        // Bezier curves.
        p.PaintBezier(
            &[(0.05, 0.90), (0.06, 0.90), (0.05, 0.91)],
            emColor::WHITE,
            bg,
        );
        p.PaintBezier(
            &[
                (0.065, 0.91),
                (0.05, 0.902),
                (0.058, 0.89),
                (0.065, 0.900),
                (0.072, 0.89),
                (0.08, 0.902),
            ],
            emColor::WHITE,
            bg,
        );

        let bezier_stroke = emStroke::new(emColor::WHITE, 0.0002);
        p.PaintBezierOutline(
            &[
                (0.085, 0.91),
                (0.07, 0.902),
                (0.078, 0.89),
                (0.085, 0.900),
                (0.092, 0.89),
                (0.10, 0.902),
            ],
            &bezier_stroke,
            bg,
        );

        let mut arrow_s = emStroke::new(emColor::WHITE, 0.0002);
        arrow_s.cap = LineCap::Round;
        arrow_s.join = LineJoin::Round;
        arrow_s.start_end =
            emStrokeEnd::new(StrokeEndType::ContourTriangle).with_inner_color(emColor::RED);
        arrow_s.finish_end = emStrokeEnd::new(StrokeEndType::Arrow);
        p.PaintBezierLine(
            &[(0.105, 0.91), (0.09, 0.902), (0.098, 0.89), (0.105, 0.900)],
            &arrow_s,
            bg,
        );

        // All StrokeEndType variants in radial pattern.
        let end_types = [
            StrokeEndType::Butt,
            StrokeEndType::Cap,
            StrokeEndType::Arrow,
            StrokeEndType::ContourArrow,
            StrokeEndType::LineArrow,
            StrokeEndType::Triangle,
            StrokeEndType::ContourTriangle,
            StrokeEndType::Square,
            StrokeEndType::ContourSquare,
            StrokeEndType::HalfSquare,
            StrokeEndType::Circle,
            StrokeEndType::ContourCircle,
            StrokeEndType::HalfCircle,
            StrokeEndType::Diamond,
            StrokeEndType::ContourDiamond,
            StrokeEndType::HalfDiamond,
            StrokeEndType::emStroke,
        ];
        let n = end_types.len();
        for i in 0..(2 * n) {
            let a = 2.0 * PI * i as f64 / (2 * n) as f64;
            let mut s = emStroke::new(emColor::WHITE, 0.0001);
            if i & 1 != 0 {
                s.cap = LineCap::Round;
                s.join = LineJoin::Round;
            }
            s.start_end = emStrokeEnd::new(StrokeEndType::Cap);
            s.finish_end = emStrokeEnd::new(end_types[i / 2])
                .with_inner_color(emColor::rgba(0xFF, 0xFF, 0xFF, 0x40));
            p.paint_line_stroked(
                0.117 + 0.002 * a.cos(),
                0.903 + 0.002 * a.sin(),
                0.117 + 0.0075 * a.cos(),
                0.903 + 0.0075 * a.sin(),
                &s,
                bg,
            );
        }

        // Polyline with contour arrow.
        let mut poly_s = emStroke::new(emColor::WHITE, 0.0005);
        poly_s.cap = LineCap::Round;
        poly_s.join = LineJoin::Round;
        poly_s.start_end = emStrokeEnd::new(StrokeEndType::ContourArrow);
        poly_s.finish_end = emStrokeEnd::new(StrokeEndType::Cap);
        p.PaintPolyline(
            &[(0.13, 0.897), (0.14, 0.902), (0.13, 0.906), (0.137, 0.909)],
            &poly_s,
            false,
            bg,
        );

        // Polygon outline.
        p.PaintPolygonOutline(
            &[(0.06, 0.80), (0.10, 0.85), (0.08, 0.91)],
            emColor::RED,
            0.0002,
            bg,
        );

        // Textured polygons — C++ emTestPanel.cpp:372–413 hand-placed vertices.
        // C++ emTestPanel.cpp:373–382
        let poly1: &[(f64, f64)] = &[
            (0.200, 0.905),
            (0.215, 0.912),
            (0.230, 0.900),
            (0.222, 0.915),
            (0.230, 0.930),
            (0.220, 0.922),
            (0.205, 0.935),
            (0.212, 0.920),
        ];
        p.paint_polygon_textured(
            poly1,
            &emTexture::LinearGradient {
                color_a: emColor::rgba(0, 0xFF, 0, 0x80),
                color_b: emColor::rgba(0xFF, 0xFF, 0, 0xFF),
                start: (0.23, 0.9),
                end: (0.2, 0.93),
            },
            bg,
        );

        // C++ emTestPanel.cpp:386–395
        let poly2: &[(f64, f64)] = &[
            (0.220, 0.905),
            (0.235, 0.912),
            (0.250, 0.900),
            (0.242, 0.915),
            (0.250, 0.930),
            (0.240, 0.922),
            (0.225, 0.935),
            (0.232, 0.920),
        ];
        p.paint_polygon_textured(
            poly2,
            &emTexture::RadialGradient {
                color_inner: emColor::rgba(0xCC, 0xCC, 0x33, 0xFF),
                color_outer: emColor::rgba(0, 0, 0xFF, 0x60),
                center: (0.21, 0.90),
                radius_x: 0.05,
                radius_y: 0.035,
            },
            bg,
        );

        let h_ratio = if self.test_image.GetWidth() > 0 {
            0.001 * self.test_image.GetHeight() as f64 / self.test_image.GetWidth() as f64
        } else {
            0.001
        };
        // C++ emTestPanel.cpp:399–408
        let poly3: &[(f64, f64)] = &[
            (0.240, 0.905),
            (0.255, 0.912),
            (0.270, 0.900),
            (0.262, 0.915),
            (0.270, 0.930),
            (0.260, 0.922),
            (0.245, 0.935),
            (0.252, 0.920),
        ];
        p.paint_polygon_textured(
            poly3,
            &emTexture::emImage {
                image: self.test_image.clone(),
                x: 0.0,
                y: 0.0,
                w: 0.002,
                h: h_ratio,
                alpha: 255,
                extension: ImageExtension::Repeat,
                quality: ImageQuality::Bilinear,
            },
            bg,
        );

        // C-18: linear gradient rect — explicit start/end points matching C++
        // emTestPanel.cpp:415–419.
        p.paint_rect_with_texture(
            0.2,
            0.94,
            0.02,
            0.01,
            &emTexture::LinearGradient {
                color_a: emColor::from_packed(0x00000080),
                color_b: emColor::from_packed(0x80808080),
                start: (0.207, 0.944),
                end: (0.213, 0.946),
            },
            bg,
        );

        // C-19: radial gradient rect — explicit origin/radii matching C++
        // emTestPanel.cpp:420–423.
        p.paint_rect_with_texture(
            0.221,
            0.94,
            0.008,
            0.01,
            &emTexture::RadialGradient {
                color_inner: emColor::from_packed(0xFF8800FF),
                color_outer: emColor::from_packed(0x005500FF),
                center: (0.223, 0.941),
                radius_x: 0.004,
                radius_y: 0.008,
            },
            bg,
        );

        // C-22: radial-gradient ellipse — matches C++ emTestPanel.cpp:425–428.
        p.paint_ellipse_with_texture(
            0.23 + 0.02 * 0.5,
            0.94 + 0.01 * 0.5,
            0.02 * 0.5,
            0.01 * 0.5,
            &emTexture::RadialGradient {
                color_inner: emColor::from_packed(0x00000000),
                color_outer: emColor::from_packed(0x00CC88FF),
                center: (0.23, 0.94),
                radius_x: 0.02,
                radius_y: 0.01,
            },
            bg,
        );

        // C-20: image tile — texture width 0.001 (not rect width 0.02), matching
        // C++ emTestPanel.cpp:430–435.
        p.PaintImageTextured(
            0.26,
            0.94,
            0.02,
            0.01,
            0.26,
            0.94,
            0.001,
            0.001 * self.test_image.GetHeight() as f64 / self.test_image.GetWidth() as f64,
            &self.test_image,
            0,
            0,
            self.test_image.GetWidth() as i32,
            self.test_image.GetHeight() as i32,
            255,
            ImageExtension::Repeat,
        );

        // C-16: emImageColoredTexture rect — matches C++ emTestPanel.cpp:441–451.
        p.PaintImageColoredTextured(
            0.2625,
            0.942,
            0.02,
            0.01,
            1.0005,
            0.942,
            0.001,
            0.001 * self.test_image.GetHeight() as f64 / self.test_image.GetWidth() as f64,
            &self.test_image,
            emColor::from_packed(0x00FFFFFF),
            emColor::from_packed(0xFF0000FF),
            bg,
            ImageExtension::Repeat,
        );

        // C-17: extend-mode rects — TILED, EDGE, ZERO — matches C++
        // emTestPanel.cpp:453–478. Rust ImageExtension variants:
        //   EXTEND_TILED → Repeat, EXTEND_EDGE → Clamp, EXTEND_ZERO → Zero.
        p.PaintImageTextured(
            0.275,
            0.907,
            0.002,
            0.002,
            0.2755,
            0.9075,
            0.001,
            0.001,
            &self.test_image,
            50,
            10,
            110,
            110,
            255,
            ImageExtension::Repeat,
        );
        p.PaintImageTextured(
            0.275,
            0.910,
            0.002,
            0.002,
            0.2755,
            0.9105,
            0.001,
            0.001,
            &self.test_image,
            50,
            10,
            110,
            110,
            255,
            ImageExtension::Clamp,
        );
        p.PaintImageTextured(
            0.275,
            0.913,
            0.002,
            0.002,
            0.2755,
            0.9135,
            0.001,
            0.001,
            &self.test_image,
            50,
            10,
            110,
            110,
            255,
            ImageExtension::Zero,
        );
    }
}

impl Drop for TestPanel {
    fn drop(&mut self) {
        if self.identity_key.is_empty() {
            return;
        }
        let bg = self.bg_shared.get();
        if bg != DEFAULT_BG {
            // C++ emTestPanel.cpp:46–50: Set(GetView(), key, BgColor, 10).
            // 10 = minLifetime seconds; ignored in Rust (no per-key model objects).
            let ctx_for_var = self.view_ctx.as_ref().unwrap_or(&self.root_ctx);
            emVarModel::Set(ctx_for_var, &self.identity_key, bg, 10);
        }
    }
}

// RUST_ONLY: (language-forced-utility) C++ uses emString::Format("%g") which has no Rust builtin equivalent.
// Replicates %g behavior: 6 significant digits, no trailing zeros.
fn format_g(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let abs = v.abs();
    let exp = abs.log10().floor() as i32;
    let prec = (5 - exp).max(0) as usize;
    let s = format!("{:.prec$}", v, prec = prec);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

impl PanelBehavior for TestPanel {
    fn IsOpaque(&self) -> bool {
        self.bg_color().IsOpaque()
    }

    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {
        // C++ emTestPanel.cpp:74–78.
        // C++ calls UpdateControlPanel() (no Rust equivalent — child panels rebuild on next
        // AutoExpand / Paint cycle) and InvalidatePainting() (Rust view repaints every frame).
        // Both are no-ops in this port; method must exist to mirror C++ override.
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn get_title(&self) -> Option<String> {
        Some("Test Panel".into())
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let bg = self.bg_color();
        let fg = if state.is_focused() {
            emColor::rgba(255, 136, 136, 255)
        } else if state.in_focused_path() {
            emColor::rgba(187, 136, 136, 255)
        } else {
            emColor::rgba(136, 136, 136, 255)
        };

        painter.push_state();
        painter.scale(w, w);
        let panel_h = h / w;

        painter.PaintRect(0.0, 0.0, 1.0, panel_h, bg, emColor::TRANSPARENT);
        painter.PaintRectOutline(
            0.01,
            0.01,
            0.98,
            panel_h - 0.02,
            &emStroke::new(fg, 0.02),
            bg,
        );

        painter.PaintTextBoxed(
            0.02,
            0.02,
            0.49,
            0.07,
            "Test Panel",
            0.1,
            fg,
            bg,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            0.5,
            true,
            0.0,
        );

        if state.viewed_rect.w < 25.0 {
            painter.pop_state();
            return;
        }

        let mut status = "State:".to_string();
        if state.is_focused() {
            status += " Focused";
        }
        if state.in_focused_path() {
            status += " InFocusedPath";
        }
        // DIVERGED: (language-forced) C++ IsViewFocused() is per-view; Rust PanelState has
        // window_focused (per-window). Observable only with multiple views per window.
        // C++ cpp:148.
        if state.window_focused {
            status += " ViewFocused";
        }
        painter.PaintTextBoxed(
            0.05,
            0.4,
            0.9,
            0.05,
            &status,
            0.05,
            fg,
            bg,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            true,
            0.0,
        );

        let pri_str = format!("Pri={:.6} MemLim={}", state.priority, state.memory_limit);
        painter.PaintTextBoxed(
            0.05,
            0.45,
            0.9,
            0.1,
            &pri_str,
            0.1,
            fg,
            bg,
            TextAlignment::Left,
            VAlign::Center,
            TextAlignment::Left,
            0.5,
            true,
            0.0,
        );

        for (i, entry) in self.input_log.iter().enumerate() {
            painter.PaintText(
                0.05,
                0.57 + i as f64 * 0.008,
                entry,
                0.008,
                1.0,
                emColor::rgba(0x88, 0x88, 0xBB, 0xFF),
                bg,
            );
        }

        self.paint_primitives(painter, fg, bg);
        painter.pop_state();
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // C++ emTestPanel.cpp:88–105. Mirrors C++ log format exactly:
        // "EVENT: key=%d chars=\"%s\" repeat=%d variant=%d STATE: pressed=k1,k2,... mouse=%g,%g"
        // C++ iterates i=0..256, printing i for each pressed key (cast from emInputKey).
        // Rust: iterate all InputKey variants in numeric-code order, collect pressed codes.
        let key_code = event.key.to_em_key_code().map_or(0i32, |c| c as i32);
        let mut pressed_parts: Vec<String> = Vec::new();
        // Collect all pressed keys sorted by C++ numeric code (0..256 scan order).
        let pressed_keys = input_state.GetKeyStates();
        let mut codes: Vec<u8> = pressed_keys
            .iter()
            .filter_map(|k| k.to_em_key_code())
            .collect();
        codes.sort_unstable();
        for code in codes {
            pressed_parts.push(format!("{}", code as i32));
        }
        let pressed_str = pressed_parts.join(",");
        let log = format!(
            "EVENT: key={key_code} chars=\"{}\" repeat={} variant={} STATE: pressed={pressed_str} mouse={},{}",
            event.chars,
            event.repeat,
            event.source_variant,
            format_g(event.mouse_x),
            format_g(event.mouse_y),
        );
        if self.input_log.len() >= MAX_LOG_ENTRIES {
            self.input_log.remove(0);
        }
        self.input_log.push(log);
        // C++ emPanel::Input forwarding call is a no-op at this level (emPanel::Input is empty).
        false
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // C++ emTestPanel constructor (cpp:39): SetAutoExpansionThreshold(900.0).
        // Rust: set here because panels lack tree access during construction.
        // First expansion fires at default 150.0 (view area >> 150 in any real view);
        // subsequent shrink/re-expand decisions use 900.0.
        ctx.tree.SetAutoExpansionThreshold(
            ctx.id,
            900.0,
            ViewConditionType::Area,
            ctx.scheduler.as_deref_mut(),
        );

        // C++ emTestPanel constructor: BgColor = emVarModel<emColor>::GetAndRemove(GetView(), ...).
        // Identity is available here (panel is in the tree before AutoExpand fires).
        // Capture view_context on first expansion — mirrors C++ GetView() scope (emTestPanel.cpp:32, 47).
        if let Some(vc) = ctx.view_context() {
            self.view_ctx = Some(vc.clone());
        }
        if self.identity_key.is_empty() {
            let identity = ctx.tree.GetIdentity(ctx.id);
            // Mirror C++ key: "emTestPanel - BgColor of " + GetIdentity().
            let key = format!("emTestPanel - BgColor of {identity}");
            // Restore persisted bg if present, using view context to match C++ GetView() scope.
            let ctx_for_var = self.view_ctx.as_ref().unwrap_or(&self.root_ctx);
            let bg = emVarModel::GetAndRemove(ctx_for_var, &key, self.bg_shared.get());
            self.bg_shared.set(bg);
            self.identity_key = key;
        }

        let root_ctx = self.root_ctx.clone();
        let bg_shared = self.bg_shared.clone();

        // C++ AutoExpand (emTestPanel.cpp:480–497): creates TkTestGrp, TP1–TP4,
        // BgColorField, PolyDraw; calls AddWakeUpSignal on BgColorField's color signal.
        let tktest_id = ctx.create_child_with("TkTestGrp", Box::new(TkTestGrpPanel::new()));
        ctx.tree.SetAutoExpansionThreshold(
            tktest_id,
            900.0,
            ViewConditionType::Area,
            ctx.scheduler.as_deref_mut(),
        );

        // Recursive child TestPanels — C++ names are "1", "2", "3", "4".
        for i in 1..=4u32 {
            let tp_id = ctx.create_child_with(
                &format!("{i}"),
                Box::new(TestPanel::new(root_ctx.clone(), DEFAULT_BG)),
            );
            ctx.tree.SetAutoExpansionThreshold(
                tp_id,
                900.0,
                ViewConditionType::Area,
                ctx.scheduler.as_deref_mut(),
            );
        }

        // Background ColorField — C++ name "BgColorField".
        // C++ emTestPanel.cpp:480–492: new emColorField(...); AddWakeUpSignal(BgColorField->GetColorSignal()).
        // Signal stored here; engine connection deferred to Cycle (EngineCtx::connect not available in PanelCtx).
        // Reset bg_signal_connected so Cycle reconnects on re-expansion.
        self.bg_signal_connected = false;
        let mut cf = emColorField::new(ctx, emLook::new());
        cf.SetCaption("Background Color");
        cf.SetEditable(true);
        cf.set_initial_alpha_enabled(true);
        cf.set_initial_color(bg_shared.get());
        self.bg_color_signal = Some(cf.color_signal);
        ctx.create_child_with("BgColorField", Box::new(ColorFieldPanel { widget: cf }));

        // PolyDraw — C++ name "PolyDraw" (emTestPanel.cpp:494).
        ctx.create_child_with("PolyDraw", Box::new(PolyDrawPanel::new()));

        // Wake engine so Cycle runs to connect bg_color_signal on the first frame after expansion.
        ctx.wake_up();
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // C++ LayoutChildren (emTestPanel.cpp:499–510): positions existing children.
        let bg = self.bg_color();
        for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
            if let Some(child) = ctx.find_child_by_name(name) {
                ctx.layout_child_canvas(child, x, y, cw, ch, bg);
            }
        }
    }

    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
        // C++ emTestPanel.cpp:62–71: Cycle checks IsSignaled(BgColorField->GetColorSignal()),
        // then reads BgColor = BgColorField->GetColor(), calls UpdateControlPanel(),
        // InvalidatePainting(), InvalidateChildrenLayout().
        //
        // Signal connection deferred to first Cycle because EngineCtx::connect is not
        // available in AutoExpand's PanelCtx. C++ uses AddWakeUpSignal in AutoExpand directly.
        if !self.bg_signal_connected {
            if let Some(sig) = self.bg_color_signal {
                ectx.connect(sig, ectx.engine_id);
            }
            self.bg_signal_connected = true;
        }

        if let Some(sig) = self.bg_color_signal {
            if ectx.IsSignaled(sig) {
                // Read new color from BgColorField widget via downcast.
                // C++: BgColor = BgColorField->GetColor().
                let new_color = pctx.find_child_by_name("BgColorField").and_then(|id| {
                    pctx.tree
                        .with_behavior_as::<ColorFieldPanel, _>(id, |cf| cf.widget.GetColor())
                });
                if let Some(color) = new_color {
                    self.bg_shared.set(color);
                    // C++: InvalidateChildrenLayout() — triggers relayout with new bg color.
                    pctx.tree
                        .InvalidateChildrenLayout(pctx.id, pctx.scheduler.as_deref_mut());
                }
            }
        }

        false
    }

    fn CreateControlPanel(
        &mut self,
        ctx: &mut PanelCtx,
        name: &str,
        _self_is_active: bool,
    ) -> Option<PanelId> {
        let identity = ctx.tree.GetIdentity(ctx.id);
        let bg = self.bg_color();
        let text = format!(
            "This is just a test\n\nPanel Identity: {identity}\nBgColor: 0x{:08X}",
            bg.GetPacked()
        );
        let label = emLabel::new(&text, emLook::new());
        Some(ctx.create_child_with(name, Box::new(LabelPanel { widget: label })))
    }
}

// ─── TkTestGrpPanel ─────────────────────────────────────────────────

struct TkTestGrpPanel {
    border: emBorder,
    look: Rc<emLook>,
}

impl TkTestGrpPanel {
    fn new() -> Self {
        let look = emLook::new();
        let border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption("Toolkit Test");
        Self { border, look }
    }
}

impl PanelBehavior for TkTestGrpPanel {
    fn IsOpaque(&self) -> bool {
        self.border.IsOpaque(&self.look)
    }
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        self.border.paint_border(
            p,
            canvas_color,
            w,
            h,
            &self.look,
            s.is_focused(),
            s.enabled,
            1.0,
        );
    }
    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // C++ TkTestGrp::AutoExpand (emTestPanel.cpp:882–910): creates sp → sp1/sp2 → t1a/t1b/t2a/t2b.
        // DIVERGED: (dependency-forced) emSplitter is not yet ported; 2×2 grid laid out manually.
        // Observable difference: panel proportions use fixed 80/20 splits instead of user-draggable splitters.
        ctx.create_child_with("t1a", Box::new(TkTestPanel::new(self.look.clone())));
        ctx.create_child_with("t1b", Box::new(TkTestPanel::new(self.look.clone())));
        ctx.create_child_with("t2a", Box::new(TkTestPanel::new(self.look.clone())));
        let t2b_id = ctx.create_child_with(
            "t2b",
            Box::new(TkTestPanel::new(self.look.clone()).with_caption("Disabled")),
        );
        ctx.tree
            .SetEnableSwitch(t2b_id, false, ctx.scheduler.as_deref_mut());
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();

        // sp->SetPos(0.8): horizontal split — sp1 left 80 %, sp2 right 20 %.
        // sp1->SetPos(0.8), sp2->SetPos(0.8): vertical split — top 80 %, bottom 20 %.
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        let left_w = cr.w * 0.8;
        let right_w = cr.w * 0.2;
        let top_h = cr.h * 0.8;
        let bot_h = cr.h * 0.2;

        if let Some(id) = ctx.find_child_by_name("t1a") {
            ctx.layout_child(id, cr.x, cr.y, left_w, top_h);
        }
        if let Some(id) = ctx.find_child_by_name("t1b") {
            ctx.layout_child(id, cr.x, cr.y + top_h, left_w, bot_h);
        }
        if let Some(id) = ctx.find_child_by_name("t2a") {
            ctx.layout_child(id, cr.x + left_w, cr.y, right_w, top_h);
        }
        if let Some(id) = ctx.find_child_by_name("t2b") {
            ctx.layout_child(id, cr.x + left_w, cr.y + top_h, right_w, bot_h);
        }

        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }
}

// ─── File dialog finish helper ───────────────────────────────────────

/// Install `set_on_finish_ext` on `fd` that mirrors C++ `TkTest::Cycle`
/// lines 824-839: when the dialog finishes positive, read the selected names
/// and show a `ShowMessage` result dialog.
///
/// Called from `TkTestPanel::Cycle` for each newly created file dialog.
/// The callback runs from inside `DialogPrivateEngine::Cycle` (the file
/// dialog's own engine) where `ectx.tree` is the dialog's tree and
/// `get_selected_names_post_show` / `get_selected_path_post_show` work correctly.
fn install_file_dialog_finish(fd: &mut emFileDialog, _ectx: &mut EngineCtx<'_>) {
    fd.dialog_mut()
        .set_on_finish_ext(Box::new(|result, dlg_panel, ectx| {
            use emcore::emDialog::{emDialog, DialogResult};
            if *result == DialogResult::Ok {
                // C++ emTestPanel.cpp:825-838: read names, build message string.
                let names = get_selected_names_post_show(dlg_panel, ectx);
                let str = if names.len() <= 1 {
                    // Single selection: show full path (joined from parent + name).
                    // C++ :831: str += FileDlg->GetSelectedPath();
                    let path = get_selected_path_post_show(dlg_panel, ectx);
                    format!(
                        "File dialog finished with positive result. Would load or save:\n{}",
                        path.display()
                    )
                } else {
                    // Multi-selection: show names indented + "From:" parent dir.
                    // C++ :833-837.
                    let mut msg = "File dialog finished with positive result. Would load or save:"
                        .to_string();
                    for name in &names {
                        msg.push_str("\n  ");
                        msg.push_str(name);
                    }
                    // DIVERGED: (language-forced) C++ `str += emString("From:\n  ")+FileDlg->GetParentDirectory()`
                    // appends the dialog's parent directory to the multi-selection summary.
                    // `get_selected_path_post_show` returns only the first selected name (filename);
                    // the FSB's parent directory is not exposed by the post-show free-function API
                    // and would require a second tree-access helper that does not yet exist. The
                    // "From:" line is omitted; observable only with multi-selection (names.len() > 1).
                    msg
                };
                // C++ :838: emDialog::ShowMessage(GetView(),"Result",str).
                // ShowMessage calls show() internally; drop the returned handle.
                let _ = emDialog::ShowMessage(ectx, "Result", &str);
            }
        }));
}

// ─── TkTest helper formatters ────────────────────────────────────────

/// C++ `TextOfTimeValue` (emTestPanel.cpp:844-870).
fn text_of_time_value(val: i64, mark_interval: u64) -> String {
    let ms = val.unsigned_abs();
    let h = ms / 3_600_000;
    let m = (ms / 60_000) % 60;
    let s = (ms / 1_000) % 60;
    let ms_r = ms % 1_000;
    match mark_interval {
        0..=9 => format!("{h:02}:{m:02}:{s:02}\n.{ms_r:03}"),
        10..=99 => format!("{h:02}:{m:02}:{s:02}\n.{:02}", ms_r / 10),
        100..=999 => format!("{h:02}:{m:02}:{s:02}\n.{}", ms_r / 100),
        1_000..=59_999 => format!("{h:02}:{m:02}:{s:02}"),
        _ => format!("{h:02}:{m:02}"),
    }
}

/// C++ `TextOfLevelValue` (emTestPanel.cpp:873-878).
fn text_of_level_value(val: i64, _mark_interval: u64) -> String {
    format!("Level {val}")
}

// ─── TkTestPanel — core widget groups ────────────────────────────────

struct TkTestPanel {
    look: Rc<emLook>,
    border: emBorder,
    layout: emRasterLayout,
    /// PlayLength value signal (sf5) — stored here, passed to sf6's
    /// `ScalarFieldWithDynamicMax` wrapper so it can watch sf5's value changes.
    sf5_len_signal: Option<SignalId>,
    // ── Dialogs group signals + checkbox state (Task 9) ──────────────
    /// BtCreateDlg click signal — None until LayoutChildren creates the button.
    btn_create_dlg_signal: Option<SignalId>,
    /// File-dialog button signals — None until LayoutChildren creates them.
    btn_open_file_signal: Option<SignalId>,
    btn_open_files_signal: Option<SignalId>,
    btn_save_file_signal: Option<SignalId>,
    /// Active file dialog — C++ `FileDlg` member (emTestPanel.cpp TkTest::FileDlg).
    /// Holds the dialog until its finish_signal fires; dropped in Cycle when finished.
    active_file_dialog: Option<emFileDialog>,
    /// True after the first Cycle has connected all wake-up signals.
    signals_connected: bool,
    // Checkbox state cells — written synchronously by on_check callbacks,
    // read in Cycle when BtCreateDlg fires.
    cb_toplev: Rc<Cell<bool>>,
    cb_pzoom: Rc<Cell<bool>>,
    cb_modal: Rc<Cell<bool>>,
    cb_undec: Rc<Cell<bool>>,
    cb_popup: Rc<Cell<bool>>,
    cb_max: Rc<Cell<bool>>,
    cb_full: Rc<Cell<bool>>,
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
            sf5_len_signal: None,
            btn_create_dlg_signal: None,
            btn_open_file_signal: None,
            btn_open_files_signal: None,
            btn_save_file_signal: None,
            active_file_dialog: None,
            signals_connected: false,
            cb_toplev: Rc::new(Cell::new(false)),
            cb_pzoom: Rc::new(Cell::new(true)),
            cb_modal: Rc::new(Cell::new(true)),
            cb_undec: Rc::new(Cell::new(false)),
            cb_popup: Rc::new(Cell::new(false)),
            cb_max: Rc::new(Cell::new(false)),
            cb_full: Rc::new(Cell::new(false)),
        }
    }

    /// Override the border caption — used to set "Disabled" on t2b
    /// (C++ emTestPanel.cpp:910: `t2b->SetCaption("Disabled")`).
    fn with_caption(mut self, caption: &str) -> Self {
        self.border = emBorder::new(OuterBorderType::Group)
            .with_inner(InnerBorderType::Group)
            .with_caption(caption);
        self
    }

    fn make_category(ctx: &mut PanelCtx, name: &str, caption: &str, pct: Option<f64>) -> PanelId {
        let mut rg = emRasterGroup::new();
        rg.border.SetBorderScaling(2.5);
        rg.border.caption = caption.to_string();
        if let Some(p) = pct {
            rg.layout.preferred_child_tallness = p;
        }
        let id = ctx.tree.create_child(ctx.id, name, None);
        ctx.tree.set_behavior(id, Box::new(rg));
        id
    }

    fn create_all_categories(&mut self, ctx: &mut PanelCtx) {
        let look = self.look.clone();

        // 1. Buttons — C++ emTestPanel.cpp:556-566.
        let gid = Self::make_category(ctx, "buttons", "Buttons", None);
        {
            let id = ctx.tree.create_child(gid, "b1", None);
            let w = emButton::new(ctx, "Button", look.clone());
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: w }));

            let mut b2 = emButton::new(ctx, "Long Desc", look.clone());
            let mut desc = String::new();
            for _ in 0..100 {
                desc.push_str("This is a looooooooooooooooooooooooooooooooooooooooooooooooooooooong description of the button.\n");
            }
            b2.SetDescription(&desc);
            let id = ctx.tree.create_child(gid, "b2", None);
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b2 }));

            let mut b3 = emButton::new(ctx, "NoEOI", look.clone());
            b3.SetNoEOI(true);
            let id = ctx.tree.create_child(gid, "b3", None);
            ctx.tree
                .set_behavior(id, Box::new(ButtonPanel { widget: b3 }));
        }

        // 2. Check Buttons and Boxes — C++ :568-575.
        let gid = Self::make_category(ctx, "checkbuttons", "Check Buttons and Boxes", None);
        {
            for i in 1..=3 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"), None);
                let w = emCheckButton::new(ctx, "Check Button", look.clone());
                ctx.tree
                    .set_behavior(id, Box::new(CheckButtonPanel { widget: w }));
            }
            for i in 4..=6 {
                let id = ctx.tree.create_child(gid, &format!("c{i}"), None);
                let w = emCheckBox::new(ctx, "Check Box", look.clone());
                ctx.tree
                    .set_behavior(id, Box::new(CheckBoxPanel { widget: w }));
            }
        }

        // 3. Radio Buttons and Boxes — C++ :577-584.
        // C++: emRadioBox extends emRadioButton; all 6 widgets (r1-r3 buttons,
        // r4-r6 boxes) share the same RasterGroup parent, so selecting any one
        // deselects the others. One RadioGroup covers all six.
        let gid = Self::make_category(ctx, "radiobuttons", "Radio Buttons and Boxes", None);
        {
            let rg = RadioGroup::new(ctx);
            for i in 1..=3usize {
                let id = ctx.tree.create_child(gid, &format!("r{i}"), None);
                let w = emRadioButton::new(ctx, "Radio Button", look.clone(), rg.clone(), i - 1);
                ctx.tree
                    .set_behavior(id, Box::new(RadioButtonPanel { widget: w }));
            }
            for i in 4..=6usize {
                let id = ctx.tree.create_child(gid, &format!("r{i}"), None);
                let w = emRadioBox::new("Radio Box", look.clone(), rg.clone(), i - 4);
                ctx.tree
                    .set_behavior(id, Box::new(RadioBoxPanel { widget: w }));
            }
        }

        // 4. Text Fields — C++ :586-609.
        let gid = Self::make_category(ctx, "textfields", "Text Fields", None);
        {
            let mut tf1 = emTextField::new(ctx, look.clone());
            tf1.SetCaption("Read-Only");
            tf1.SetDescription("This is a read-only text field.");
            tf1.SetText("Read-Only");
            let id = ctx.tree.create_child(gid, "tf1", None);
            ctx.tree.set_behavior(
                id,
                Box::new(TextFieldPanel {
                    widget: tf1,
                    is_focused: false,
                }),
            );

            let mut tf2 = emTextField::new(ctx, look.clone());
            tf2.SetCaption("Editable");
            tf2.SetDescription("This is an editable text field.");
            tf2.SetEditable(true);
            tf2.SetText("Editable");
            let id = ctx.tree.create_child(gid, "tf2", None);
            ctx.tree.set_behavior(
                id,
                Box::new(TextFieldPanel {
                    widget: tf2,
                    is_focused: false,
                }),
            );

            let mut tf3 = emTextField::new(ctx, look.clone());
            tf3.SetCaption("Password");
            tf3.SetDescription("This is an editable password text field.");
            tf3.SetEditable(true);
            tf3.SetText("Password");
            tf3.SetPasswordMode(true);
            let id = ctx.tree.create_child(gid, "tf3", None);
            ctx.tree.set_behavior(
                id,
                Box::new(TextFieldPanel {
                    widget: tf3,
                    is_focused: false,
                }),
            );

            let mut mltf1 = emTextField::new(ctx, look.clone());
            mltf1.SetCaption("Multi-Line");
            mltf1.SetDescription("This is an editable multi-line text field.");
            mltf1.SetEditable(true);
            mltf1.SetMultiLineMode(true);
            mltf1.SetText("first line\nsecond line\n...");
            let id = ctx.tree.create_child(gid, "mltf1", None);
            ctx.tree.set_behavior(
                id,
                Box::new(TextFieldPanel {
                    widget: mltf1,
                    is_focused: false,
                }),
            );
        }

        // 5. Scalar Fields (sf1–sf6) — C++ :611-660.
        let gid = Self::make_category(ctx, "scalarfields", "Scalar Fields", Some(0.1));
        {
            let mut sf1 = emScalarField::new(ctx, 0.0, 10.0, look.clone());
            sf1.SetCaption("Read-Only");
            let id = ctx.tree.create_child(gid, "sf1", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf1 }));

            let mut sf2 = emScalarField::new(ctx, 0.0, 10.0, look.clone());
            sf2.SetCaption("Editable");
            sf2.SetEditable(true);
            let id = ctx.tree.create_child(gid, "sf2", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf2 }));

            let mut sf3 = emScalarField::new(ctx, -1000.0, 1000.0, look.clone());
            sf3.SetEditable(true);
            sf3.set_initial_value(0.0);
            sf3.SetScaleMarkIntervals(&[1000, 100, 10, 5, 1]);
            let id = ctx.tree.create_child(gid, "sf3", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf3 }));

            // sf4 — Level — C++ :624-630.
            let mut sf4 = emScalarField::new(ctx, 1.0, 5.0, look.clone());
            sf4.SetCaption("Level");
            sf4.SetEditable(true);
            sf4.SetTextBoxTallness(0.25);
            sf4.set_initial_value(3.0);
            sf4.SetTextOfValueFunc(Box::new(text_of_level_value));
            let id = ctx.tree.create_child(gid, "sf4", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf4 }));

            // sf5 — Play Length — C++ :632-638.
            // AddWakeUpSignal(SFLen->GetValueSignal()) — signal stored and passed to sf6 wrapper.
            let mut sf5 = emScalarField::new(ctx, 0.0, 24.0 * 3_600_000.0, look.clone());
            sf5.SetCaption("Play Length");
            sf5.SetEditable(true);
            sf5.set_initial_value(4.0 * 3_600_000.0);
            sf5.SetScaleMarkIntervals(&[
                3_600_000, 900_000, 300_000, 60_000, 10_000, 1_000, 100, 10, 1,
            ]);
            sf5.SetTextOfValueFunc(Box::new(text_of_time_value));
            let sf5_len_signal = sf5.value_signal;
            self.sf5_len_signal = Some(sf5_len_signal);
            // C++: SFPos->SetMinMaxValues(0, SFLen->GetValue()) — read sf5's initial
            // value before moving sf5 into the tree.
            let sf6_initial_max = sf5.GetValue();
            let id = ctx.tree.create_child(gid, "sf5", None);
            ctx.tree
                .set_behavior(id, Box::new(ScalarFieldPanel { widget: sf5 }));

            // sf6 — Play Position — C++ :640-644. Dynamic max tracks sf5's value
            // via the wrapper's signal-driven Cycle (mirrors C++ TkTest::Cycle :786).
            // Initial max read from sf5.GetValue() above (matches C++ :642).
            let mut sf6 = emScalarField::new(ctx, 0.0, sf6_initial_max, look.clone());
            sf6.SetCaption("Play Position");
            sf6.SetEditable(true);
            sf6.SetScaleMarkIntervals(&[
                3_600_000, 900_000, 300_000, 60_000, 10_000, 1_000, 100, 10, 1,
            ]);
            sf6.SetTextOfValueFunc(Box::new(text_of_time_value));
            let id = ctx.tree.create_child(gid, "sf6", None);
            ctx.tree.set_behavior(
                id,
                Box::new(ScalarFieldWithDynamicMax {
                    widget: sf6,
                    sf5_len_signal,
                    signal_connected: false,
                }),
            );
        }

        // 6. Color Fields — C++ :646-660.
        let gid = Self::make_category(ctx, "colorfields", "Color Fields", Some(0.4));
        {
            let mut cf1 = emColorField::new(ctx, look.clone());
            cf1.SetCaption("Read-Only");
            cf1.set_initial_color(emColor::rgba(0xBB, 0x22, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf1", None);
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf1 }));

            let mut cf2 = emColorField::new(ctx, look.clone());
            cf2.SetCaption("Editable");
            cf2.SetEditable(true);
            cf2.set_initial_color(emColor::rgba(0x22, 0xBB, 0x22, 0xFF));
            let id = ctx.tree.create_child(gid, "cf2", None);
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf2 }));

            let mut cf3 = emColorField::new(ctx, look.clone());
            cf3.SetCaption("Editable, Alpha Enabled");
            cf3.SetEditable(true);
            cf3.set_initial_alpha_enabled(true);
            cf3.set_initial_color(emColor::rgba(0x22, 0x22, 0xBB, 0xFF));
            let id = ctx.tree.create_child(gid, "cf3", None);
            ctx.tree
                .set_behavior(id, Box::new(ColorFieldPanel { widget: cf3 }));
        }

        // 7. Tunnels — C++ :662-680.
        // `emTunnel` implements `PanelBehavior` directly; content is created as
        // a child of the tunnel panel in the tree (matching C++ parent/child hierarchy).
        let gid = Self::make_category(ctx, "tunnels", "Tunnels", Some(0.4));
        {
            // t1: default depth, emButton content — C++ :666-667.
            let t1 = emTunnel::new(look.clone()).with_caption("Tunnel");
            let t1_id = ctx.tree.create_child(gid, "t1", None);
            ctx.tree.set_behavior(t1_id, Box::new(t1));
            {
                let btn = emButton::new(ctx, "End Of Tunnel", look.clone());
                let e_id = ctx.tree.create_child(t1_id, "e", None);
                ctx.tree
                    .set_behavior(e_id, Box::new(ButtonPanel { widget: btn }));
            }

            // t2: SetDepth(30.0), emRasterGroup content — C++ :669-671.
            let mut t2 = emTunnel::new(look.clone()).with_caption("Deeper Tunnel");
            t2.SetDepth(30.0);
            let t2_id = ctx.tree.create_child(gid, "t2", None);
            ctx.tree.set_behavior(t2_id, Box::new(t2));
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                let e_id = ctx.tree.create_child(t2_id, "e", None);
                ctx.tree.set_behavior(e_id, Box::new(rg));
            }

            // t3: SetChildTallness(1.0), emRasterGroup content — C++ :673-675.
            let mut t3 = emTunnel::new(look.clone()).with_caption("Square End");
            t3.SetChildTallness(1.0);
            let t3_id = ctx.tree.create_child(gid, "t3", None);
            ctx.tree.set_behavior(t3_id, Box::new(t3));
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                let e_id = ctx.tree.create_child(t3_id, "e", None);
                ctx.tree.set_behavior(e_id, Box::new(rg));
            }

            // t4: SetChildTallness(1.0), SetDepth(0.0), emRasterGroup content — C++ :677-680.
            let mut t4 = emTunnel::new(look.clone()).with_caption("Square End, Zero Depth");
            t4.SetChildTallness(1.0);
            t4.SetDepth(0.0);
            let t4_id = ctx.tree.create_child(gid, "t4", None);
            ctx.tree.set_behavior(t4_id, Box::new(t4));
            {
                let mut rg = emRasterGroup::new();
                rg.border.caption = "End Of Tunnel".to_string();
                let e_id = ctx.tree.create_child(t4_id, "e", None);
                ctx.tree.set_behavior(e_id, Box::new(rg));
            }
        }

        // 8. List Boxes — C++ emTestPanel.cpp:682-731.
        // grp->SetBorderScaling(2.5) and grp->SetPrefChildTallness(0.4) mirror C++.
        let gid = Self::make_category(ctx, "listboxes", "List Boxes", Some(0.4));
        {
            // l1: Empty — C++ :686.
            let mut lb1 = emListBox::new(ctx, look.clone());
            lb1.SetCaption("Empty");
            let id = ctx.tree.create_child(gid, "l1", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb1 }));

            // l2: Single-Selection, 7 items, index 0 selected — C++ :688-692.
            let mut lb2 = emListBox::new(ctx, look.clone());
            lb2.SetCaption("Single-Selection");
            for i in 1..=7usize {
                lb2.AddItem(format!("{i}"), format!("Item {i}"));
            }
            lb2.SetSelectedIndex(0);
            let id = ctx.tree.create_child(gid, "l2", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb2 }));

            // l3: Read-Only, 7 items, index 2 selected — C++ :694-699.
            let mut lb3 = emListBox::new(ctx, look.clone());
            lb3.SetCaption("Read-Only");
            lb3.SetSelectionType(SelectionMode::ReadOnly);
            for i in 1..=7usize {
                lb3.AddItem(format!("{i}"), format!("Item {i}"));
            }
            lb3.SetSelectedIndex(2);
            let id = ctx.tree.create_child(gid, "l3", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb3 }));

            // l4: Multi-Selection, 7 items, indices 1-4 selected — C++ :701-709.
            let mut lb4 = emListBox::new(ctx, look.clone());
            lb4.SetCaption("Multi-Selection");
            lb4.SetSelectionType(SelectionMode::Multi);
            for i in 1..=7usize {
                lb4.AddItem(format!("{i}"), format!("Item {i}"));
            }
            lb4.Select(1, false);
            lb4.Select(2, false);
            lb4.Select(3, false);
            lb4.Select(4, false);
            let id = ctx.tree.create_child(gid, "l4", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb4 }));

            // l5: Toggle-Selection, 7 items, indices 2,4 selected — C++ :711-717.
            let mut lb5 = emListBox::new(ctx, look.clone());
            lb5.SetCaption("Toggle-Selection");
            lb5.SetSelectionType(SelectionMode::Toggle);
            for i in 1..=7usize {
                lb5.AddItem(format!("{i}"), format!("Item {i}"));
            }
            lb5.Select(2, false);
            lb5.Select(4, false);
            let id = ctx.tree.create_child(gid, "l5", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb5 }));

            // l6: Single Column, 7 items, index 0 selected — C++ :719-724.
            let mut lb6 = emListBox::new(ctx, look.clone());
            lb6.SetCaption("Single Column");
            lb6.set_fixed_column_count(Some(1));
            for i in 1..=7usize {
                lb6.AddItem(format!("{i}"), format!("Item {i}"));
            }
            lb6.SetSelectedIndex(0);
            let id = ctx.tree.create_child(gid, "l6", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb6 }));

            // l7: Custom List Box — C++ :726-731, 985-1001.
            // C++ CustomListBox constructor (cpp:992–994) sets child tallness 0.4,
            // top-left alignment, and strict raster.
            // CustomItemPanel children (emLinearGroup + ItemPanelInterface) implement
            // ItemSelectionChanged (C++ :970-981): rose bg color (emColor(224,80,128))
            // on selected items. Implemented via set_item_behavior_factory (visual layer).
            // I-13: factory receives the listbox's live look as its `look` parameter
            // (matching C++ `SetLook(GetListBox().GetLook())`); no outer capture needed.
            let mut lb7 = emListBox::new(ctx, look.clone());
            lb7.SetCaption("Custom List Box");
            lb7.SetSelectionType(SelectionMode::Multi);
            for i in 1..=7usize {
                lb7.AddItem(format!("{i}"), format!("Item {i}"));
            }
            lb7.SetSelectedIndex(0);
            // I-12: C++ CustomListBox::CustomListBox (cpp:992–994).
            lb7.SetChildTallness(0.4);
            lb7.SetAlignment(AlignmentH::Left, AlignmentV::Top);
            lb7.SetStrictRaster();
            lb7.set_item_behavior_factory(
                move |_index, text, selected, look, _sel_mode, _enabled| {
                    Box::new(CustomItemBehavior {
                        text: text.to_string(),
                        selected,
                        look,
                    })
                },
            );
            let id = ctx.tree.create_child(gid, "l7", None);
            ctx.tree
                .set_behavior(id, Box::new(ListBoxPanel { widget: lb7 }));
        }

        // 9. Dialogs group — C++ emTestPanel.cpp:733-748.
        // grp->SetBorderScaling(2.5), SetFixedColumnCount(1).
        // Inner raster layout "rl" holds checkboxes (pref_child_tallness 0.1).
        // Button "bt" is direct child of grp (alongside rl).
        {
            let mut dlgs_rg = emRasterGroup::new();
            dlgs_rg.border.caption = "Test Dialog".to_string();
            dlgs_rg.border.SetBorderScaling(2.5);
            dlgs_rg.layout.fixed_columns = Some(1);
            let dlgs_id = ctx.tree.create_child(ctx.id, "dlgs", None);
            ctx.tree.set_behavior(dlgs_id, Box::new(dlgs_rg));

            // Inner raster layout "rl" — contains the checkboxes.
            // C++ :736 `rl=new emRasterLayout(grp,"rl"); rl->SetPrefChildTallness(0.1)`.
            let mut rl_beh = emRasterLayout::new();
            rl_beh.preferred_child_tallness = 0.1;
            let rl_id = ctx.tree.create_child(dlgs_id, "rl", None);
            ctx.tree.set_behavior(rl_id, Box::new(rl_beh));

            // Checkboxes — C++ :738-746.
            // Helper: create a checkbox in rl_id, wire on_check to a Cell<bool>.
            let make_cb = |ctx: &mut PanelCtx,
                           name: &str,
                           caption: &str,
                           initial: bool,
                           state: Rc<Cell<bool>>| {
                let mut cb = emCheckBox::new(ctx, caption, look.clone());
                if initial {
                    cb.set_checked_silent(true);
                }
                cb.on_check = Some(Box::new(move |checked, _sched| {
                    state.set(checked);
                }));
                let id = ctx.tree.create_child(rl_id, name, None);
                ctx.tree
                    .set_behavior(id, Box::new(CheckBoxPanel { widget: cb }));
            };

            make_cb(ctx, "tl", "Top-Level", false, Rc::clone(&self.cb_toplev));
            make_cb(
                ctx,
                "VF_POPUP_ZOOM",
                "VF_POPUP_ZOOM",
                true,
                Rc::clone(&self.cb_pzoom),
            );
            make_cb(ctx, "WF_MODAL", "WF_MODAL", true, Rc::clone(&self.cb_modal));
            make_cb(
                ctx,
                "WF_UNDECORATED",
                "WF_UNDECORATED",
                false,
                Rc::clone(&self.cb_undec),
            );
            make_cb(
                ctx,
                "WF_POPUP",
                "WF_POPUP",
                false,
                Rc::clone(&self.cb_popup),
            );
            make_cb(
                ctx,
                "WF_MAXIMIZED",
                "WF_MAXIMIZED",
                false,
                Rc::clone(&self.cb_max),
            );
            make_cb(
                ctx,
                "WF_FULLSCREEN",
                "WF_FULLSCREEN",
                false,
                Rc::clone(&self.cb_full),
            );

            // Button "bt" — C++ :747-748.
            let bt = emButton::new(ctx, "Create Test Dialog", look.clone());
            self.btn_create_dlg_signal = Some(bt.click_signal);
            let bt_id = ctx.tree.create_child(dlgs_id, "bt", None);
            ctx.tree
                .set_behavior(bt_id, Box::new(ButtonPanel { widget: bt }));
        }

        // 10. File choosers — C++ emTestPanel.cpp:750-768.
        // grp->SetBorderScaling(2.5), grp->SetPrefChildTallness(0.3).
        // emFileSelectionBox "l8" with three filters; three buttons for
        // open, open-multi/allow-dir, and save-as file dialogs.
        {
            let mut fc_rg = emRasterGroup::new();
            fc_rg.border.caption = "File Selection".to_string();
            fc_rg.border.SetBorderScaling(2.5);
            fc_rg.layout.preferred_child_tallness = 0.3;
            let fc_id = ctx.tree.create_child(ctx.id, "fileChoosers", None);
            ctx.tree.set_behavior(fc_id, Box::new(fc_rg));

            // emFileSelectionBox "l8" with filters — C++ :751-756.
            let mut fsb = emFileSelectionBox::new(ctx, "File Selection Box");
            fsb.set_filters(&[
                "All Files (*)".to_string(),
                "Image Files (*.bmp *.gif *.jpg *.png *.tga)".to_string(),
                "HTML Files (*.htm *.html)".to_string(),
            ]);
            let fsb_id = ctx.tree.create_child(fc_id, "l8", None);
            ctx.tree.set_behavior(fsb_id, Box::new(fsb));

            // Open button — C++ :757-758.
            let bt_open = emButton::new(ctx, "Open...", look.clone());
            self.btn_open_file_signal = Some(bt_open.click_signal);
            let bt_open_id = ctx.tree.create_child(fc_id, "openFile", None);
            ctx.tree
                .set_behavior(bt_open_id, Box::new(ButtonPanel { widget: bt_open }));

            // Open Multi button — C++ :759-760.
            let bt_opens = emButton::new(ctx, "Open Multi, Allow Dir...", look.clone());
            self.btn_open_files_signal = Some(bt_opens.click_signal);
            let bt_opens_id = ctx.tree.create_child(fc_id, "openFiles", None);
            ctx.tree
                .set_behavior(bt_opens_id, Box::new(ButtonPanel { widget: bt_opens }));

            // Save As button — C++ :761-762.
            let bt_save = emButton::new(ctx, "Save As...", look.clone());
            self.btn_save_file_signal = Some(bt_save.click_signal);
            let bt_save_id = ctx.tree.create_child(fc_id, "saveFile", None);
            ctx.tree
                .set_behavior(bt_save_id, Box::new(ButtonPanel { widget: bt_save }));
        }
    }
}

impl PanelBehavior for TkTestPanel {
    fn IsOpaque(&self) -> bool {
        self.border.IsOpaque(&self.look)
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn Paint(&mut self, p: &mut emPainter, canvas_color: emColor, w: f64, h: f64, s: &PanelState) {
        self.border.paint_border(
            p,
            canvas_color,
            w,
            h,
            &self.look,
            s.is_focused(),
            s.enabled,
            1.0,
        );
    }
    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // C++ TkTest::TkTest constructor creates all widget children immediately.
        // In Rust, AutoExpand is the equivalent since tree access requires ctx.
        self.create_all_categories(ctx);
        // Wake engine so Cycle runs to connect signals on the first frame after expansion.
        ctx.wake_up();
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
        self.layout.do_layout_skip(ctx, None, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, _pctx: &mut PanelCtx) -> bool {
        // Connect wake-up signals on the first Cycle after LayoutChildren.
        // C++ `AddWakeUpSignal` wires each signal to the engine directly in the
        // constructor; in Rust we defer until Cycle because EngineCtx is required
        // for `connect` and LayoutChildren only has PanelCtx.
        if !self.signals_connected {
            let eid = ectx.engine_id;
            for sig in [
                self.btn_create_dlg_signal,
                self.btn_open_file_signal,
                self.btn_open_files_signal,
                self.btn_save_file_signal,
            ]
            .into_iter()
            .flatten()
            {
                ectx.connect(sig, eid);
            }
            self.signals_connected = true;
        }

        // File dialog finished — C++ emTestPanel.cpp:830-839:
        //   if (FileDlg && IsSignaled(FileDlg->GetFinishSignal())) { ... delete FileDlg; FileDlg=NULL; }
        // Result display is handled by the on_finish_ext callback installed at
        // dialog-creation time (below). Here we only tear down the handle.
        if let Some(ref fd) = self.active_file_dialog {
            if ectx.IsSignaled(fd.finish_signal()) {
                self.active_file_dialog = None;
            }
        }

        // Open file — C++ emTestPanel.cpp:808-811.
        if let Some(sig) = self.btn_open_file_signal {
            if ectx.IsSignaled(sig) {
                // Drop any previous active file dialog before creating a new one.
                // C++ `if (FileDlg) delete FileDlg;` — matches C++ at :808.
                self.active_file_dialog = None;
                let look = Rc::clone(&self.look);
                let mut fd = emFileDialog::new(ectx, FileDialogMode::Open, look);
                install_file_dialog_finish(&mut fd, ectx);
                fd.show(ectx);
                self.active_file_dialog = Some(fd);
            }
        }

        // Open multi / allow dir — C++ emTestPanel.cpp:812-817.
        if let Some(sig) = self.btn_open_files_signal {
            if ectx.IsSignaled(sig) {
                self.active_file_dialog = None;
                let look = Rc::clone(&self.look);
                let mut fd = emFileDialog::new(ectx, FileDialogMode::Open, look);
                fd.set_multi_selection_enabled(true);
                fd.set_directory_result_allowed(true);
                install_file_dialog_finish(&mut fd, ectx);
                fd.show(ectx);
                self.active_file_dialog = Some(fd);
            }
        }

        // Save As — C++ emTestPanel.cpp:818-821.
        if let Some(sig) = self.btn_save_file_signal {
            if ectx.IsSignaled(sig) {
                self.active_file_dialog = None;
                let look = Rc::clone(&self.look);
                let mut fd = emFileDialog::new(ectx, FileDialogMode::Save, look);
                install_file_dialog_finish(&mut fd, ectx);
                fd.show(ectx);
                self.active_file_dialog = Some(fd);
            }
        }

        // Connect new active file dialog's finish_signal to our engine so we
        // wake up when it finishes. Mirrors C++ AddWakeUpSignal(FileDlg->GetFinishSignal()).
        // Done after all three button branches so we always subscribe the most
        // recently created dialog.
        if let Some(ref fd) = self.active_file_dialog {
            let finish_sig = fd.finish_signal();
            let eid = ectx.engine_id;
            ectx.connect(finish_sig, eid);
        }

        // Create Test Dialog — C++ emTestPanel.cpp:788-803.
        if let Some(sig) = self.btn_create_dlg_signal {
            if ectx.IsSignaled(sig) {
                let look = Rc::clone(&self.look);

                let mut vflags = ViewFlags::ROOT_SAME_TALLNESS;
                if self.cb_pzoom.get() {
                    vflags |= ViewFlags::POPUP_ZOOM;
                }

                let mut wflags = WindowFlags::empty();
                if self.cb_modal.get() {
                    wflags |= WindowFlags::MODAL;
                }
                if self.cb_undec.get() {
                    wflags |= WindowFlags::UNDECORATED;
                }
                if self.cb_popup.get() {
                    wflags |= WindowFlags::POPUP;
                }
                if self.cb_max.get() {
                    wflags |= WindowFlags::MAXIMIZED;
                }
                if self.cb_full.get() {
                    wflags |= WindowFlags::FULLSCREEN;
                }

                // DIVERGED: (language-forced) C++ selects dialog context by checking
                // CbTopLev->IsChecked(): false → GetView() (attached to this view),
                // true → GetRootContext() (top-level). In Rust, emDialog::new always
                // uses root_context() internally (emDialog.rs:100); the ConstructCtx
                // trait does not expose a separate view-context construction path.
                // Both C++ paths produce a top-level dialog window; the observable
                // difference (which window the dialog is parented to) is not testable.
                // Self.cb_toplev is retained for display fidelity.
                let mut dlg = emcore::emDialog::emDialog::new(ectx, "Test Dialog", look.clone());
                // Apply view/window flags — C++ emDialog(*ctx, vFlags, wFlags) passes
                // flags to the constructor; set them before AddNegativeButton to mirror
                // C++ construction order (emTestPanel.cpp:799–803).
                dlg.set_view_window_flags(vflags, wflags);
                dlg.AddNegativeButton(ectx, "Close");
                dlg.EnableAutoDeletion(ectx, true);
                dlg.SetRootTitle(ectx, "Test Dialog");
                // C++ :803 `new TkTest(dlg->GetContentPanel(),"test")`.
                dlg.set_content_behavior(ectx, Box::new(TkTestPanel::new(look)));
                dlg.show(ectx);
            }
        }

        false
    }
}

// ─── PolyDrawPanel — emLinearGroup container ────────────────────────
//
// C++ `PolyDrawPanel : public emLinearGroup` (emTestPanel.h:132).
// AutoExpand creates a Controls raster with four sub-groups (general,
// stroke, strokeStart, strokeEnd) and their widget children, plus CanvasPanel.
// All children are created flat from PolyDrawPanel::AutoExpand using
// ctx.tree.create_child(sub_id, ...) — mirrors TkTestPanel::create_all_categories
// pattern already established in this file. The created_by_ae flag for
// grandchildren is false (same as TkTestPanel's grandchildren) since
// auto-shrink is not yet wired; this is the codebase-established pattern.

struct PolyDrawPanel {
    group: emLinearGroup,
    // True after the first Cycle has connected all 18 wake-up signals.
    // C++ calls AddWakeUpSignal inline in the constructor; Rust defers to first
    // Cycle because EngineCtx is required for `connect` and the constructor
    // only has InitCtx.
    signals_connected: bool,
    // RadioGroup handles — read selected index in Cycle via group.borrow().GetChecked().
    // Rc<RefCell<RadioGroup>> — the type is already Rc<RefCell<>> by emcore design
    // (RadioGroup::new returns Rc<RefCell<Self>>). Stored here so Cycle can read the
    // selected index after AutoExpand has built the radio button tree. Closest to (a):
    // the handle bridges two separate method invocations (AutoExpand and Cycle) that
    // cannot share stack state.
    type_group: Option<Rc<RefCell<RadioGroup>>>,
    stroke_dash_type_group: Option<Rc<RefCell<RadioGroup>>>,
    stroke_start_type_group: Option<Rc<RefCell<RadioGroup>>>,
    stroke_end_type_group: Option<Rc<RefCell<RadioGroup>>>,
    // Signal IDs — None until AutoExpand wires them. 18 signals total.
    type_signal: Option<SignalId>,
    vertex_count_signal: Option<SignalId>,
    with_canvas_color_signal: Option<SignalId>,
    fill_color_signal: Option<SignalId>,
    stroke_width_signal: Option<SignalId>,
    stroke_color_signal: Option<SignalId>,
    stroke_rounded_signal: Option<SignalId>,
    stroke_dash_type_signal: Option<SignalId>,
    dash_length_factor_signal: Option<SignalId>,
    gap_length_factor_signal: Option<SignalId>,
    stroke_start_type_signal: Option<SignalId>,
    stroke_start_inner_color_signal: Option<SignalId>,
    stroke_start_width_factor_signal: Option<SignalId>,
    stroke_start_length_factor_signal: Option<SignalId>,
    stroke_end_type_signal: Option<SignalId>,
    stroke_end_inner_color_signal: Option<SignalId>,
    stroke_end_width_factor_signal: Option<SignalId>,
    stroke_end_length_factor_signal: Option<SignalId>,
    // Panel IDs for reading widget values in Cycle via downcast.
    canvas_id: Option<PanelId>,
    vertex_count_id: Option<PanelId>,
    with_canvas_color_id: Option<PanelId>,
    fill_color_id: Option<PanelId>,
    stroke_width_id: Option<PanelId>,
    stroke_color_id: Option<PanelId>,
    stroke_rounded_id: Option<PanelId>,
    stroke_dash_type_id: Option<PanelId>,
    dash_length_factor_id: Option<PanelId>,
    gap_length_factor_id: Option<PanelId>,
    stroke_start_type_id: Option<PanelId>,
    stroke_start_inner_color_id: Option<PanelId>,
    stroke_start_width_factor_id: Option<PanelId>,
    stroke_start_length_factor_id: Option<PanelId>,
    stroke_end_type_id: Option<PanelId>,
    stroke_end_inner_color_id: Option<PanelId>,
    stroke_end_width_factor_id: Option<PanelId>,
    stroke_end_length_factor_id: Option<PanelId>,
}

impl PolyDrawPanel {
    fn new() -> Self {
        let mut group = emLinearGroup::horizontal();
        // C++ emTestPanel.cpp:1005–1009: caption and description set in constructor.
        group.border.SetCaption("Poly Draw Test");
        group.border.description =
            "This allows manual testing of various paint functions. Main focus is\n\
             on strokes an stroke ends, i.e. textures cannot be tested with this.\n"
                .to_string();
        // C++ emTestPanel.cpp:1011: SetOrientationThresholdTallness(1.0) — switches
        // horizontal/vertical layout based on aspect ratio.
        // DIVERGED: (upstream-gap-forced) emLinearGroup does not implement orientation
        // threshold switching; the method is absent from the Rust emLinearGroup API.
        // With a single CanvasPanel child the observable layout is unchanged until
        // the controls sub-tree is added.
        Self {
            group,
            signals_connected: false,
            type_group: None,
            stroke_dash_type_group: None,
            stroke_start_type_group: None,
            stroke_end_type_group: None,
            type_signal: None,
            vertex_count_signal: None,
            with_canvas_color_signal: None,
            fill_color_signal: None,
            stroke_width_signal: None,
            stroke_color_signal: None,
            stroke_rounded_signal: None,
            stroke_dash_type_signal: None,
            dash_length_factor_signal: None,
            gap_length_factor_signal: None,
            stroke_start_type_signal: None,
            stroke_start_inner_color_signal: None,
            stroke_start_width_factor_signal: None,
            stroke_start_length_factor_signal: None,
            stroke_end_type_signal: None,
            stroke_end_inner_color_signal: None,
            stroke_end_width_factor_signal: None,
            stroke_end_length_factor_signal: None,
            canvas_id: None,
            vertex_count_id: None,
            with_canvas_color_id: None,
            fill_color_id: None,
            stroke_width_id: None,
            stroke_color_id: None,
            stroke_rounded_id: None,
            stroke_dash_type_id: None,
            dash_length_factor_id: None,
            gap_length_factor_id: None,
            stroke_start_type_id: None,
            stroke_start_inner_color_id: None,
            stroke_start_width_factor_id: None,
            stroke_start_length_factor_id: None,
            stroke_end_type_id: None,
            stroke_end_inner_color_id: None,
            stroke_end_width_factor_id: None,
            stroke_end_length_factor_id: None,
        }
    }
}

impl PanelBehavior for PolyDrawPanel {
    fn IsOpaque(&self) -> bool {
        // C++ emGroup::IsOpaque → emBorder::IsOpaque; OBT_GROUP returns false.
        self.group.border.IsOpaque(&self.group.look)
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // C++ PolyDrawPanel::AutoExpand (emTestPanel.cpp:1071–1261).
        // Creates Controls raster + four sub-groups + all 22 widgets + CanvasPanel.
        // Flat creation pattern: grandchildren created via ctx.tree.create_child(sub_id, ...)
        // matching TkTestPanel::create_all_categories precedent.

        // ── Controls raster layout ───────────────────────────────────────────
        // C++: controls = new emRasterLayout(this,"Controls"); controls->SetPrefChildTallness(0.6)
        let mut controls_layout = emRasterLayout::new();
        controls_layout.preferred_child_tallness = 0.6;
        let controls_id = ctx.create_child_with("Controls", Box::new(controls_layout));

        // ── general sub-group ────────────────────────────────────────────────
        // C++: general = new emLinearGroup(controls,"general","General");
        //      general->SetBorderScaling(2.0); general->SetChildWeight(0,2.0)
        // Note: SetChildWeight(0,2.0) is set after creating child 0 (Method).
        let mut general_grp = emLinearGroup::vertical();
        general_grp.border.SetBorderScaling(2.0);
        general_grp.border.caption = "General".to_string();
        let general_id = ctx.tree.create_child(controls_id, "general", None);
        ctx.tree.set_behavior(general_id, Box::new(general_grp));

        // ── stroke sub-group ─────────────────────────────────────────────────
        // C++: stroke = new emLinearGroup(controls,"stroke","Stroke");
        //      stroke->SetBorderScaling(2.0); stroke->SetChildWeight(2,2.0)
        // Note: SetChildWeight(2,2.0) is set after creating child 2 (StrokeDashType).
        let mut stroke_grp = emLinearGroup::vertical();
        stroke_grp.border.SetBorderScaling(2.0);
        stroke_grp.border.caption = "Stroke".to_string();
        let stroke_id = ctx.tree.create_child(controls_id, "stroke", None);
        ctx.tree.set_behavior(stroke_id, Box::new(stroke_grp));

        // ── strokeStart sub-group ────────────────────────────────────────────
        // C++: strokeStart = new emLinearGroup(controls,"strokeStart","Stroke Start");
        //      strokeStart->SetBorderScaling(2.0); strokeStart->SetChildWeight(0,2.0)
        let mut stroke_start_grp = emLinearGroup::vertical();
        stroke_start_grp.border.SetBorderScaling(2.0);
        stroke_start_grp.border.caption = "Stroke Start".to_string();
        let stroke_start_id = ctx.tree.create_child(controls_id, "strokeStart", None);
        ctx.tree
            .set_behavior(stroke_start_id, Box::new(stroke_start_grp));

        // ── strokeEnd sub-group ──────────────────────────────────────────────
        // C++: strokeEnd = new emLinearGroup(controls,"strokeEnd","Stroke End");
        //      strokeEnd->SetBorderScaling(2.0); strokeEnd->SetChildWeight(0,2.0)
        let mut stroke_end_grp = emLinearGroup::vertical();
        stroke_end_grp.border.SetBorderScaling(2.0);
        stroke_end_grp.border.caption = "Stroke End".to_string();
        let stroke_end_id = ctx.tree.create_child(controls_id, "strokeEnd", None);
        ctx.tree
            .set_behavior(stroke_end_id, Box::new(stroke_end_grp));

        // ════════════════════════════════════════════════════════════════════
        // general children
        // ════════════════════════════════════════════════════════════════════

        // ── Method (Type) RadioGroup ─────────────────────────────────────────
        // C++: Type = new emRadioButton::RasterGroup(general,"Method","Method");
        //      new emRadioBox(*Type,"0",...) × 16
        //      Type->SetBorderScaling(1.5); Type->SetPrefChildTallness(0.07); Type->SetCheckIndex(0)
        //      AddWakeUpSignal(Type->GetCheckSignal())
        let type_rg = RadioGroup::new(ctx);
        let type_signal = type_rg.borrow().check_signal;
        {
            let mut rg_panel = emRasterGroup::new();
            rg_panel.border.SetBorderScaling(1.5);
            rg_panel.border.caption = "Method".to_string();
            rg_panel.layout.preferred_child_tallness = 0.07;
            let method_id = ctx.tree.create_child(general_id, "Method", None);
            ctx.tree.set_behavior(method_id, Box::new(rg_panel));
            // Set child 0 (Method) weight to 2.0 on general — mirrors C++ general->SetChildWeight(0,2.0).
            // The constraint is set after method_id is known, using with_behavior_as.
            ctx.tree
                .with_behavior_as::<emLinearGroup, _>(general_id, |g| {
                    g.layout.set_child_constraint(
                        method_id,
                        ChildConstraint {
                            weight: 2.0,
                            ..Default::default()
                        },
                    );
                });
            // Radio boxes 0–15
            let labels = [
                "PaintPolygon",
                "PaintPolygonOutline",
                "PaintPolyline",
                "PaintBezier",
                "PaintBezierOutline",
                "PaintBezierLine",
                "PaintLine",
                "PaintRect",
                "PaintRectOutline",
                "PaintEllipse",
                "PaintEllipseOutline",
                "PaintEllipseSector",
                "PaintEllipseSectorOutline",
                "PaintEllipseArc",
                "PaintRoundRect",
                "PaintRoundRectOutline",
            ];
            let look = emLook::new();
            for (i, label) in labels.iter().enumerate() {
                let rb_id = ctx.tree.create_child(method_id, &i.to_string(), None);
                let w = emRadioBox::new(label, look.clone(), type_rg.clone(), i);
                ctx.tree
                    .set_behavior(rb_id, Box::new(RadioBoxPanel { widget: w }));
            }
            // SetCheckIndex(0) — select first option
            type_rg.borrow_mut().SetCheckIndex(Some(0), ctx);
        }

        // ── ll (VertexCount + FillColor row) ─────────────────────────────────
        // C++: ll=new emLinearLayout(general,"ll"); ll->SetHorizontal()
        //      VertexCount=new emTextField(ll,...); FillColor=new emColorField(ll,...)
        let ll_id = ctx.tree.create_child(general_id, "ll", None);
        ctx.tree
            .set_behavior(ll_id, Box::new(emLinearLayout::horizontal()));

        // VertexCount
        // C++: VertexCount->SetEditable(); VertexCount->SetText("9")
        //      AddWakeUpSignal(VertexCount->GetTextSignal())
        let vertex_count_id = ctx.tree.create_child(ll_id, "VertexCount", None);
        let look = emLook::new();
        let mut vc = emTextField::new(ctx, look.clone());
        vc.SetCaption("Vertex Count");
        vc.SetEditable(true);
        vc.SetText("9");
        let vertex_count_signal = vc.text_signal;
        ctx.tree.set_behavior(
            vertex_count_id,
            Box::new(TextFieldPanel {
                widget: vc,
                is_focused: false,
            }),
        );

        // FillColor
        // C++: FillColor->SetEditable(); FillColor->SetAlphaEnabled(); FillColor->SetColor(0xFFFFFFFF)
        //      AddWakeUpSignal(FillColor->GetColorSignal())
        let fill_color_id = ctx.tree.create_child(ll_id, "FillColor", None);
        let mut fc = emColorField::new(ctx, emLook::new());
        fc.SetCaption("Fill Color");
        fc.SetEditable(true);
        fc.set_initial_alpha_enabled(true);
        fc.set_initial_color(emColor::WHITE);
        let fill_color_signal = fc.color_signal;
        ctx.tree
            .set_behavior(fill_color_id, Box::new(ColorFieldPanel { widget: fc }));

        // ── ll2 (StrokeWidth + WithCanvasColor row) ───────────────────────────
        // C++: ll=new emLinearLayout(general,"ll2"); ll->SetHorizontal()
        let ll2_id = ctx.tree.create_child(general_id, "ll2", None);
        ctx.tree
            .set_behavior(ll2_id, Box::new(emLinearLayout::horizontal()));

        // StrokeWidth
        // C++: StrokeWidth->SetEditable(); StrokeWidth->SetText("0.01")
        //      AddWakeUpSignal(StrokeWidth->GetTextSignal())
        let stroke_width_id = ctx.tree.create_child(ll2_id, "StrokeWidth", None);
        let mut sw = emTextField::new(ctx, look.clone());
        sw.SetCaption("Stroke Width");
        sw.SetEditable(true);
        sw.SetText("0.01");
        let stroke_width_signal = sw.text_signal;
        ctx.tree.set_behavior(
            stroke_width_id,
            Box::new(TextFieldPanel {
                widget: sw,
                is_focused: false,
            }),
        );

        // WithCanvasColor
        // C++: WithCanvasColor=new emCheckBox(ll,"WithCanvasColor","With Canvas Color")
        //      AddWakeUpSignal(WithCanvasColor->GetCheckSignal()); WithCanvasColor->SetChecked(false)
        let with_canvas_color_id = ctx.tree.create_child(ll2_id, "WithCanvasColor", None);
        let mut wcc = emCheckBox::new(ctx, "With Canvas Color", look.clone());
        wcc.SetChecked(false, ctx);
        let with_canvas_color_signal = wcc.check_signal;
        ctx.tree.set_behavior(
            with_canvas_color_id,
            Box::new(CheckBoxPanel { widget: wcc }),
        );

        // ════════════════════════════════════════════════════════════════════
        // stroke children
        // ════════════════════════════════════════════════════════════════════

        // StrokeColor
        // C++: StrokeColor=new emColorField(stroke,"StrokeColor","Color")
        //      SetEditable; SetAlphaEnabled; SetColor(0x000000FF)
        //      AddWakeUpSignal(StrokeColor->GetColorSignal())
        let stroke_color_id = ctx.tree.create_child(stroke_id, "StrokeColor", None);
        let mut sc = emColorField::new(ctx, emLook::new());
        sc.SetCaption("Color");
        sc.SetEditable(true);
        sc.set_initial_alpha_enabled(true);
        sc.set_initial_color(emColor::BLACK);
        let stroke_color_signal = sc.color_signal;
        ctx.tree
            .set_behavior(stroke_color_id, Box::new(ColorFieldPanel { widget: sc }));

        // StrokeRounded
        // C++: StrokeRounded=new emCheckBox(stroke,"StrokeRounded","Rounded")
        //      AddWakeUpSignal(StrokeRounded->GetCheckSignal())
        let stroke_rounded_id = ctx.tree.create_child(stroke_id, "StrokeRounded", None);
        let sr = emCheckBox::new(ctx, "Rounded", look.clone());
        let stroke_rounded_signal = sr.check_signal;
        ctx.tree
            .set_behavior(stroke_rounded_id, Box::new(CheckBoxPanel { widget: sr }));

        // StrokeDashType RadioGroup
        // C++: StrokeDashType=new emRadioButton::RasterGroup(stroke,"StrokeDashType","Dash Type")
        //      4 radios; SetBorderScaling(1.5); SetPrefChildTallness(0.08); SetCheckIndex(0)
        //      AddWakeUpSignal(StrokeDashType->GetCheckSignal())
        let stroke_dash_rg = RadioGroup::new(ctx);
        let stroke_dash_type_signal = stroke_dash_rg.borrow().check_signal;
        {
            let mut rg_panel = emRasterGroup::new();
            rg_panel.border.SetBorderScaling(1.5);
            rg_panel.border.caption = "Dash Type".to_string();
            rg_panel.layout.preferred_child_tallness = 0.08;
            let dash_type_id = ctx.tree.create_child(stroke_id, "StrokeDashType", None);
            ctx.tree.set_behavior(dash_type_id, Box::new(rg_panel));
            // C++ stroke->SetChildWeight(2,2.0) — StrokeDashType is child 2 of stroke.
            ctx.tree
                .with_behavior_as::<emLinearGroup, _>(stroke_id, |g| {
                    g.layout.set_child_constraint(
                        dash_type_id,
                        ChildConstraint {
                            weight: 2.0,
                            ..Default::default()
                        },
                    );
                });
            let dash_labels = ["SOLID", "DASHED", "DOTTED", "DASH_DOTTED"];
            for (i, label) in dash_labels.iter().enumerate() {
                let rb_id = ctx.tree.create_child(dash_type_id, &i.to_string(), None);
                let w = emRadioBox::new(label, look.clone(), stroke_dash_rg.clone(), i);
                ctx.tree
                    .set_behavior(rb_id, Box::new(RadioBoxPanel { widget: w }));
            }
            stroke_dash_rg.borrow_mut().SetCheckIndex(Some(0), ctx);
            self.stroke_dash_type_id = Some(dash_type_id);
        }

        // ll (DashLengthFactor + GapLengthFactor row)
        // C++: ll=new emLinearLayout(stroke,"ll"); ll->SetHorizontal()
        let stroke_ll_id = ctx.tree.create_child(stroke_id, "ll", None);
        ctx.tree
            .set_behavior(stroke_ll_id, Box::new(emLinearLayout::horizontal()));

        // DashLengthFactor
        let dash_length_factor_id = ctx
            .tree
            .create_child(stroke_ll_id, "DashLengthFactor", None);
        let mut dlf = emTextField::new(ctx, look.clone());
        dlf.SetCaption("Dash Length Factor");
        dlf.SetEditable(true);
        dlf.SetText("1.0");
        let dash_length_factor_signal = dlf.text_signal;
        ctx.tree.set_behavior(
            dash_length_factor_id,
            Box::new(TextFieldPanel {
                widget: dlf,
                is_focused: false,
            }),
        );

        // GapLengthFactor
        let gap_length_factor_id = ctx.tree.create_child(stroke_ll_id, "GapLengthFactor", None);
        let mut glf = emTextField::new(ctx, look.clone());
        glf.SetCaption("Gap Length Factor");
        glf.SetEditable(true);
        glf.SetText("1.0");
        let gap_length_factor_signal = glf.text_signal;
        ctx.tree.set_behavior(
            gap_length_factor_id,
            Box::new(TextFieldPanel {
                widget: glf,
                is_focused: false,
            }),
        );

        // ════════════════════════════════════════════════════════════════════
        // strokeStart children
        // ════════════════════════════════════════════════════════════════════

        // StrokeStartType RadioGroup (17 radios)
        // C++: StrokeStartType=new emRadioButton::RasterGroup(strokeStart,"StrokeStartType","Type")
        //      SetBorderScaling(1.5); SetPrefChildTallness(0.08); SetCheckIndex(0)
        //      AddWakeUpSignal(StrokeStartType->GetCheckSignal())
        //      strokeStart->SetChildWeight(0,2.0)
        let stroke_start_rg = RadioGroup::new(ctx);
        let stroke_start_type_signal = stroke_start_rg.borrow().check_signal;
        {
            let mut rg_panel = emRasterGroup::new();
            rg_panel.border.SetBorderScaling(1.5);
            rg_panel.border.caption = "Type".to_string();
            rg_panel.layout.preferred_child_tallness = 0.08;
            let start_type_id = ctx
                .tree
                .create_child(stroke_start_id, "StrokeStartType", None);
            ctx.tree.set_behavior(start_type_id, Box::new(rg_panel));
            // C++ strokeStart->SetChildWeight(0,2.0) — StrokeStartType is child 0.
            ctx.tree
                .with_behavior_as::<emLinearGroup, _>(stroke_start_id, |g| {
                    g.layout.set_child_constraint(
                        start_type_id,
                        ChildConstraint {
                            weight: 2.0,
                            ..Default::default()
                        },
                    );
                });
            let end_labels = [
                "BUTT",
                "CAP",
                "ARROW",
                "CONTOUR_ARROW",
                "LINE_ARROW",
                "TRIANGLE",
                "CONTOUR_TRIANGLE",
                "SQUARE",
                "CONTOUR_SQUARE",
                "HALF_SQUARE",
                "CIRCLE",
                "CONTOUR_CIRCLE",
                "HALF_CIRCLE",
                "DIAMOND",
                "CONTOUR_DIAMOND",
                "HALF_DIAMOND",
                "STROKE",
            ];
            for (i, label) in end_labels.iter().enumerate() {
                let rb_id = ctx.tree.create_child(start_type_id, &i.to_string(), None);
                let w = emRadioBox::new(label, look.clone(), stroke_start_rg.clone(), i);
                ctx.tree
                    .set_behavior(rb_id, Box::new(RadioBoxPanel { widget: w }));
            }
            stroke_start_rg.borrow_mut().SetCheckIndex(Some(0), ctx);
            self.stroke_start_type_id = Some(start_type_id);
        }

        // StrokeStartInnerColor
        // C++: SetEditable; SetAlphaEnabled; SetColor(0xEEEEEEFF)
        let stroke_start_inner_color_id =
            ctx.tree
                .create_child(stroke_start_id, "StrokeStartInnerColor", None);
        let mut ssic = emColorField::new(ctx, emLook::new());
        ssic.SetCaption("Inner Color");
        ssic.SetEditable(true);
        ssic.set_initial_alpha_enabled(true);
        ssic.set_initial_color(emColor::rgba(0xEE, 0xEE, 0xEE, 0xFF));
        let stroke_start_inner_color_signal = ssic.color_signal;
        ctx.tree.set_behavior(
            stroke_start_inner_color_id,
            Box::new(ColorFieldPanel { widget: ssic }),
        );

        // ll (StrokeStartWidthFactor + StrokeStartLengthFactor row)
        let stroke_start_ll_id = ctx.tree.create_child(stroke_start_id, "ll", None);
        ctx.tree
            .set_behavior(stroke_start_ll_id, Box::new(emLinearLayout::horizontal()));

        // StrokeStartWidthFactor
        let stroke_start_width_factor_id =
            ctx.tree
                .create_child(stroke_start_ll_id, "StrokeStartWidthFactor", None);
        let mut sswf = emTextField::new(ctx, look.clone());
        sswf.SetCaption("Width Factor");
        sswf.SetEditable(true);
        sswf.SetText("1.0");
        let stroke_start_width_factor_signal = sswf.text_signal;
        ctx.tree.set_behavior(
            stroke_start_width_factor_id,
            Box::new(TextFieldPanel {
                widget: sswf,
                is_focused: false,
            }),
        );

        // StrokeStartLengthFactor
        let stroke_start_length_factor_id =
            ctx.tree
                .create_child(stroke_start_ll_id, "StrokeStartLengthFactor", None);
        let mut sslf = emTextField::new(ctx, look.clone());
        sslf.SetCaption("Length Factor");
        sslf.SetEditable(true);
        sslf.SetText("1.0");
        let stroke_start_length_factor_signal = sslf.text_signal;
        ctx.tree.set_behavior(
            stroke_start_length_factor_id,
            Box::new(TextFieldPanel {
                widget: sslf,
                is_focused: false,
            }),
        );

        // ════════════════════════════════════════════════════════════════════
        // strokeEnd children
        // ════════════════════════════════════════════════════════════════════

        // StrokeEndType RadioGroup (17 radios) — same labels as Start
        // C++: strokeEnd->SetChildWeight(0,2.0)
        let stroke_end_rg = RadioGroup::new(ctx);
        let stroke_end_type_signal = stroke_end_rg.borrow().check_signal;
        {
            let mut rg_panel = emRasterGroup::new();
            rg_panel.border.SetBorderScaling(1.5);
            rg_panel.border.caption = "Type".to_string();
            rg_panel.layout.preferred_child_tallness = 0.08;
            let end_type_id = ctx.tree.create_child(stroke_end_id, "StrokeEndType", None);
            ctx.tree.set_behavior(end_type_id, Box::new(rg_panel));
            // C++ strokeEnd->SetChildWeight(0,2.0) — StrokeEndType is child 0.
            ctx.tree
                .with_behavior_as::<emLinearGroup, _>(stroke_end_id, |g| {
                    g.layout.set_child_constraint(
                        end_type_id,
                        ChildConstraint {
                            weight: 2.0,
                            ..Default::default()
                        },
                    );
                });
            let end_labels = [
                "BUTT",
                "CAP",
                "ARROW",
                "CONTOUR_ARROW",
                "LINE_ARROW",
                "TRIANGLE",
                "CONTOUR_TRIANGLE",
                "SQUARE",
                "CONTOUR_SQUARE",
                "HALF_SQUARE",
                "CIRCLE",
                "CONTOUR_CIRCLE",
                "HALF_CIRCLE",
                "DIAMOND",
                "CONTOUR_DIAMOND",
                "HALF_DIAMOND",
                "STROKE",
            ];
            for (i, label) in end_labels.iter().enumerate() {
                let rb_id = ctx.tree.create_child(end_type_id, &i.to_string(), None);
                let w = emRadioBox::new(label, look.clone(), stroke_end_rg.clone(), i);
                ctx.tree
                    .set_behavior(rb_id, Box::new(RadioBoxPanel { widget: w }));
            }
            stroke_end_rg.borrow_mut().SetCheckIndex(Some(0), ctx);
            self.stroke_end_type_id = Some(end_type_id);
        }

        // StrokeEndInnerColor
        let stroke_end_inner_color_id =
            ctx.tree
                .create_child(stroke_end_id, "StrokeEndInnerColor", None);
        let mut seic = emColorField::new(ctx, emLook::new());
        seic.SetCaption("Inner Color");
        seic.SetEditable(true);
        seic.set_initial_alpha_enabled(true);
        seic.set_initial_color(emColor::rgba(0xEE, 0xEE, 0xEE, 0xFF));
        let stroke_end_inner_color_signal = seic.color_signal;
        ctx.tree.set_behavior(
            stroke_end_inner_color_id,
            Box::new(ColorFieldPanel { widget: seic }),
        );

        // ll (StrokeEndWidthFactor + StrokeEndLengthFactor row)
        let stroke_end_ll_id = ctx.tree.create_child(stroke_end_id, "ll", None);
        ctx.tree
            .set_behavior(stroke_end_ll_id, Box::new(emLinearLayout::horizontal()));

        // StrokeEndWidthFactor
        let stroke_end_width_factor_id =
            ctx.tree
                .create_child(stroke_end_ll_id, "StrokeEndWidthFactor", None);
        let mut sewf = emTextField::new(ctx, look.clone());
        sewf.SetCaption("Width Factor");
        sewf.SetEditable(true);
        sewf.SetText("1.0");
        let stroke_end_width_factor_signal = sewf.text_signal;
        ctx.tree.set_behavior(
            stroke_end_width_factor_id,
            Box::new(TextFieldPanel {
                widget: sewf,
                is_focused: false,
            }),
        );

        // StrokeEndLengthFactor
        let stroke_end_length_factor_id =
            ctx.tree
                .create_child(stroke_end_ll_id, "StrokeEndLengthFactor", None);
        let mut self_ = emTextField::new(ctx, look.clone());
        self_.SetCaption("Length Factor");
        self_.SetEditable(true);
        self_.SetText("1.0");
        let stroke_end_length_factor_signal = self_.text_signal;
        ctx.tree.set_behavior(
            stroke_end_length_factor_id,
            Box::new(TextFieldPanel {
                widget: self_,
                is_focused: false,
            }),
        );

        // ── CanvasPanel ──────────────────────────────────────────────────────
        // C++: Canvas = new CanvasPanel(this,"CanvasPanel")
        let canvas_id = ctx.create_child_with("CanvasPanel", Box::new(CanvasPanel::new()));
        self.canvas_id = Some(canvas_id);

        // ── Wire signal fields and RadioGroup handles ────────────────────────
        // C++: AddWakeUpSignal on each widget's signal (called inline above in C++).
        // In Rust we store signal IDs here; Cycle connects them via
        // ectx.connect(sig, eid) on first Cycle after AutoExpand.
        self.type_group = Some(type_rg);
        self.stroke_dash_type_group = Some(stroke_dash_rg);
        self.stroke_start_type_group = Some(stroke_start_rg);
        self.stroke_end_type_group = Some(stroke_end_rg);

        self.type_signal = Some(type_signal);
        self.vertex_count_signal = Some(vertex_count_signal);
        self.fill_color_signal = Some(fill_color_signal);
        self.stroke_width_signal = Some(stroke_width_signal);
        self.with_canvas_color_signal = Some(with_canvas_color_signal);
        self.stroke_color_signal = Some(stroke_color_signal);
        self.stroke_rounded_signal = Some(stroke_rounded_signal);
        self.stroke_dash_type_signal = Some(stroke_dash_type_signal);
        self.dash_length_factor_signal = Some(dash_length_factor_signal);
        self.gap_length_factor_signal = Some(gap_length_factor_signal);
        self.stroke_start_type_signal = Some(stroke_start_type_signal);
        self.stroke_start_inner_color_signal = Some(stroke_start_inner_color_signal);
        self.stroke_start_width_factor_signal = Some(stroke_start_width_factor_signal);
        self.stroke_start_length_factor_signal = Some(stroke_start_length_factor_signal);
        self.stroke_end_type_signal = Some(stroke_end_type_signal);
        self.stroke_end_inner_color_signal = Some(stroke_end_inner_color_signal);
        self.stroke_end_width_factor_signal = Some(stroke_end_width_factor_signal);
        self.stroke_end_length_factor_signal = Some(stroke_end_length_factor_signal);

        self.vertex_count_id = Some(vertex_count_id);
        self.with_canvas_color_id = Some(with_canvas_color_id);
        self.fill_color_id = Some(fill_color_id);
        self.stroke_width_id = Some(stroke_width_id);
        self.stroke_color_id = Some(stroke_color_id);
        self.stroke_rounded_id = Some(stroke_rounded_id);
        self.dash_length_factor_id = Some(dash_length_factor_id);
        self.gap_length_factor_id = Some(gap_length_factor_id);
        self.stroke_start_inner_color_id = Some(stroke_start_inner_color_id);
        self.stroke_start_width_factor_id = Some(stroke_start_width_factor_id);
        self.stroke_start_length_factor_id = Some(stroke_start_length_factor_id);
        self.stroke_end_inner_color_id = Some(stroke_end_inner_color_id);
        self.stroke_end_width_factor_id = Some(stroke_end_width_factor_id);
        self.stroke_end_length_factor_id = Some(stroke_end_length_factor_id);

        // Wake engine so Cycle runs to connect signals on the first frame after expansion.
        // Mirrors C++ pattern: C++ AddWakeUpSignal fires on the next scheduler cycle;
        // here we guarantee Cycle runs immediately after AutoExpand.
        ctx.wake_up();

        // C++ first Cycle fires after all signals are signaled at construction via
        // AddWakeUpSignal + SetCheckIndex(0), so Setup() is called on the very first frame.
        // Rust defers signal connection to first Cycle, so initial signals are gone by then.
        // Call Setup directly here with default widget values to establish the initial render
        // state (C++ timing compat).
        self.apply_setup_from_widgets(ctx);
    }

    fn Paint(
        &mut self,
        p: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        self.group.Paint(p, canvas_color, w, h, state);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        self.group.LayoutChildren(ctx);
    }

    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx) -> bool {
        // C++ PolyDrawPanel::Cycle (emTestPanel.cpp:1015–1068).
        // Connect all 18 wake-up signals on first Cycle after AutoExpand.
        // C++ calls AddWakeUpSignal inline in constructor; Rust defers to first
        // Cycle because EngineCtx::connect requires EngineCtx (not available in AutoExpand).
        if !self.signals_connected {
            let eid = ectx.engine_id;
            for sig in [
                self.type_signal,
                self.vertex_count_signal,
                self.with_canvas_color_signal,
                self.fill_color_signal,
                self.stroke_width_signal,
                self.stroke_color_signal,
                self.stroke_rounded_signal,
                self.stroke_dash_type_signal,
                self.dash_length_factor_signal,
                self.gap_length_factor_signal,
                self.stroke_start_type_signal,
                self.stroke_start_inner_color_signal,
                self.stroke_start_width_factor_signal,
                self.stroke_start_length_factor_signal,
                self.stroke_end_type_signal,
                self.stroke_end_inner_color_signal,
                self.stroke_end_width_factor_signal,
                self.stroke_end_length_factor_signal,
            ]
            .into_iter()
            .flatten()
            {
                ectx.connect(sig, eid);
            }
            self.signals_connected = true;
        }

        // C++ `if (Canvas && (IsSignaled(...) || ...)) { Canvas->Setup(...); }`
        if self.canvas_id.is_none() {
            return false;
        }

        let any_signaled = [
            self.type_signal,
            self.vertex_count_signal,
            self.with_canvas_color_signal,
            self.fill_color_signal,
            self.stroke_width_signal,
            self.stroke_color_signal,
            self.stroke_rounded_signal,
            self.stroke_dash_type_signal,
            self.dash_length_factor_signal,
            self.gap_length_factor_signal,
            self.stroke_start_type_signal,
            self.stroke_start_inner_color_signal,
            self.stroke_start_width_factor_signal,
            self.stroke_start_length_factor_signal,
            self.stroke_end_type_signal,
            self.stroke_end_inner_color_signal,
            self.stroke_end_width_factor_signal,
            self.stroke_end_length_factor_signal,
        ]
        .into_iter()
        .flatten()
        .any(|sig| ectx.IsSignaled(sig));

        if !any_signaled {
            return false;
        }

        // Delegate to shared helper — same logic path used by the initial call in AutoExpand.
        self.apply_setup_from_widgets(pctx);

        false
    }
}

impl PolyDrawPanel {
    /// Read all control widget values and call `CanvasPanel::Setup`.
    /// Shared by `AutoExpand` (initial call to establish C++ first-Cycle timing)
    /// and `Cycle` (on any signal change). Single source of truth for defaults.
    ///
    /// C++ `PolyDrawPanel::Cycle` body (emTestPanel.cpp:1035–1067).
    fn apply_setup_from_widgets(&self, pctx: &mut PanelCtx) {
        let canvas_id = match self.canvas_id {
            Some(id) => id,
            None => return,
        };

        // Read Type from RadioGroup (C++ `Type->GetCheckIndex()`).
        let render_type = self
            .type_group
            .as_ref()
            .and_then(|g| g.borrow().GetChecked())
            .unwrap_or(0) as u8;

        // Read VertexCount (C++ `atoi(VertexCount->GetText())`).
        let vertex_count = self
            .vertex_count_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<usize>().unwrap_or(9)
                })
            })
            .unwrap_or(9);

        // Read WithCanvasColor (C++ `WithCanvasColor->IsChecked()`).
        let with_canvas_color = self
            .with_canvas_color_id
            .and_then(|id| {
                pctx.tree
                    .with_behavior_as::<CheckBoxPanel, _>(id, |b| b.widget.IsChecked())
            })
            .unwrap_or(false);

        // Read FillColor (C++ `emTexture(FillColor->GetColor())`).
        // Simplified to emColor (flat fill); C++ wraps in emTexture for gradient/image
        // support. Full emTexture integration deferred; follow up if golden comparison
        // reveals divergence. See task C-3.
        let fill_color = self
            .fill_color_id
            .and_then(|id| {
                pctx.tree
                    .with_behavior_as::<ColorFieldPanel, _>(id, |b| b.widget.GetColor())
            })
            .unwrap_or(emColor::WHITE);

        // Read StrokeWidth (C++ `atof(StrokeWidth->GetText())`).
        let stroke_width = self
            .stroke_width_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(0.01)
                })
            })
            .unwrap_or(0.01);

        // Read StrokeColor (C++ `StrokeColor->GetColor()`).
        let stroke_color = self
            .stroke_color_id
            .and_then(|id| {
                pctx.tree
                    .with_behavior_as::<ColorFieldPanel, _>(id, |b| b.widget.GetColor())
            })
            .unwrap_or(emColor::BLACK);

        // Read StrokeRounded (C++ `StrokeRounded->IsChecked()`).
        // C++ emStroke has a Rounded field that sets joins and caps to Round.
        // Rust emStroke has separate join and cap fields — we translate by setting
        // both to Round when checked (same observable behavior as C++ emStroke::Rounded).
        let stroke_rounded = self
            .stroke_rounded_id
            .and_then(|id| {
                pctx.tree
                    .with_behavior_as::<CheckBoxPanel, _>(id, |b| b.widget.IsChecked())
            })
            .unwrap_or(false);

        // Read StrokeDashType from RadioGroup (C++ `(emStroke::DashTypeEnum)StrokeDashType->GetCheckIndex()`).
        let dash_type_idx = self
            .stroke_dash_type_group
            .as_ref()
            .and_then(|g| g.borrow().GetChecked())
            .unwrap_or(0);
        let dash_type = match dash_type_idx {
            0 => DashType::Solid,
            1 => DashType::Dashed,
            2 => DashType::Dotted,
            _ => DashType::DashDotted,
        };

        // Read DashLengthFactor (C++ `atof(DashLengthFactor->GetText())`).
        let dash_length_factor = self
            .dash_length_factor_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(1.0)
                })
            })
            .unwrap_or(1.0);

        // Read GapLengthFactor (C++ `atof(GapLengthFactor->GetText())`).
        let gap_length_factor = self
            .gap_length_factor_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(1.0)
                })
            })
            .unwrap_or(1.0);

        // Build emStroke (C++ `emStroke(StrokeColor, StrokeRounded, DashType, DashLengthFactor, GapLengthFactor)`).
        let mut stroke = emStroke::new(stroke_color, stroke_width);
        if stroke_rounded {
            stroke.join = LineJoin::Round;
            stroke.cap = LineCap::Round;
        }
        stroke.dash_type = dash_type;
        stroke.dash_length_factor = dash_length_factor;
        stroke.gap_length_factor = gap_length_factor;

        // Read StrokeStartType from RadioGroup (C++ `(emStrokeEnd::TypeEnum)StrokeStartType->GetCheckIndex()`).
        let stroke_start_type_idx = self
            .stroke_start_type_group
            .as_ref()
            .and_then(|g| g.borrow().GetChecked())
            .unwrap_or(0);
        let stroke_start_type = idx_to_stroke_end_type(stroke_start_type_idx);

        // Read StrokeStartInnerColor (C++ `StrokeStartInnerColor->GetColor()`).
        let stroke_start_inner_color = self
            .stroke_start_inner_color_id
            .and_then(|id| {
                pctx.tree
                    .with_behavior_as::<ColorFieldPanel, _>(id, |b| b.widget.GetColor())
            })
            .unwrap_or(emColor::rgba(0xEE, 0xEE, 0xEE, 0xFF));

        // Read StrokeStartWidthFactor (C++ `atof(StrokeStartWidthFactor->GetText())`).
        let stroke_start_width_factor = self
            .stroke_start_width_factor_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(1.0)
                })
            })
            .unwrap_or(1.0);

        // Read StrokeStartLengthFactor (C++ `atof(StrokeStartLengthFactor->GetText())`).
        let stroke_start_length_factor = self
            .stroke_start_length_factor_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(1.0)
                })
            })
            .unwrap_or(1.0);

        // Build StrokeStart (C++ `emStrokeEnd(TypeEnum, InnerColor, WidthFactor, LengthFactor)`).
        let stroke_start = emStrokeEnd {
            end_type: stroke_start_type,
            inner_color: stroke_start_inner_color,
            width_factor: stroke_start_width_factor,
            length_factor: stroke_start_length_factor,
        };

        // Read StrokeEndType from RadioGroup (C++ `(emStrokeEnd::TypeEnum)StrokeEndType->GetCheckIndex()`).
        let stroke_end_type_idx = self
            .stroke_end_type_group
            .as_ref()
            .and_then(|g| g.borrow().GetChecked())
            .unwrap_or(0);
        let stroke_end_type = idx_to_stroke_end_type(stroke_end_type_idx);

        // Read StrokeEndInnerColor (C++ `StrokeEndInnerColor->GetColor()`).
        let stroke_end_inner_color = self
            .stroke_end_inner_color_id
            .and_then(|id| {
                pctx.tree
                    .with_behavior_as::<ColorFieldPanel, _>(id, |b| b.widget.GetColor())
            })
            .unwrap_or(emColor::rgba(0xEE, 0xEE, 0xEE, 0xFF));

        // Read StrokeEndWidthFactor (C++ `atof(StrokeEndWidthFactor->GetText())`).
        let stroke_end_width_factor = self
            .stroke_end_width_factor_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(1.0)
                })
            })
            .unwrap_or(1.0);

        // Read StrokeEndLengthFactor (C++ `atof(StrokeEndLengthFactor->GetText())`).
        let stroke_end_length_factor = self
            .stroke_end_length_factor_id
            .and_then(|id| {
                pctx.tree.with_behavior_as::<TextFieldPanel, _>(id, |b| {
                    b.widget.GetText().parse::<f64>().unwrap_or(1.0)
                })
            })
            .unwrap_or(1.0);

        // Build StrokeEnd (C++ `emStrokeEnd(TypeEnum, InnerColor, WidthFactor, LengthFactor)`).
        let stroke_end = emStrokeEnd {
            end_type: stroke_end_type,
            inner_color: stroke_end_inner_color,
            width_factor: stroke_end_width_factor,
            length_factor: stroke_end_length_factor,
        };

        // Call Canvas->Setup(...) (C++ emTestPanel.cpp:1035–1067).
        pctx.tree
            .with_behavior_as::<CanvasPanel, _>(canvas_id, |canvas| {
                canvas.Setup(
                    render_type,
                    vertex_count,
                    with_canvas_color,
                    fill_color,
                    stroke_width,
                    stroke,
                    stroke_start,
                    stroke_end,
                );
            });
    }
}

/// C++ `(emStrokeEnd::TypeEnum)idx` cast — converts radio button index to
/// `StrokeEndType`. C++ enumerates 17 values; indices beyond the last map to
/// the last variant (STROKE) to match C++ undefined-but-silent cast behavior.
fn idx_to_stroke_end_type(idx: usize) -> StrokeEndType {
    match idx {
        0 => StrokeEndType::Butt,
        1 => StrokeEndType::Cap,
        2 => StrokeEndType::Arrow,
        3 => StrokeEndType::ContourArrow,
        4 => StrokeEndType::LineArrow,
        5 => StrokeEndType::Triangle,
        6 => StrokeEndType::ContourTriangle,
        7 => StrokeEndType::Square,
        8 => StrokeEndType::ContourSquare,
        9 => StrokeEndType::HalfSquare,
        10 => StrokeEndType::Circle,
        11 => StrokeEndType::ContourCircle,
        12 => StrokeEndType::HalfCircle,
        13 => StrokeEndType::Diamond,
        14 => StrokeEndType::ContourDiamond,
        15 => StrokeEndType::HalfDiamond,
        _ => StrokeEndType::emStroke,
    }
}

// ─── CanvasPanel — interactive polygon drawing ───────────────────────
//
// C++ `CanvasPanel : public emPanel` (emTestPanel.h:139).
// Holds vertex positions, drag state, and render state; Paint draws gradient
// background + polygon/bezier/rect/ellipse/etc. + handles; Input handles
// vertex dragging.

struct CanvasPanel {
    // Drag state — set by Input.
    vertices: Vec<(f64, f64)>,
    drag_idx: Option<usize>,
    drag_offset: (f64, f64),
    show_handles: bool,
    // Render state — set by Setup(), driven by PolyDrawPanel::Cycle.
    // C++ emTestPanel.h:152–161.
    render_type: u8,         // C++ Type (emTestPanel.cpp:1278)
    with_canvas_color: bool, // C++ WithCanvasColor (emTestPanel.cpp:1293)
    // Simplified to emColor (flat fill) — C++ uses emTexture for gradient/image support.
    // Full emTexture integration deferred; follow up if golden comparison reveals divergence.
    texture: emColor,
    // C++ stores four separate members: StrokeWidth, Stroke, StrokeStart, StrokeEnd
    // (emTestPanel.h:163–166). Rust emStroke encodes start_end/finish_end inline, so the
    // four C++ fields (StrokeWidth + Stroke + StrokeStart + StrokeEnd) are folded into one
    // here: stroke.width mirrors StrokeWidth, stroke.start_end mirrors StrokeStart, and
    // stroke.finish_end mirrors StrokeEnd. Setup() receives all four separately and folds them.
    stroke: emStroke,
}

impl CanvasPanel {
    fn new() -> Self {
        // C++ CanvasPanel::CanvasPanel (emTestPanel.cpp:1270–1273):
        // DragIdx=-1, no vertices initially; ShowHandles(false).
        // The first Setup() call from Cycle() populates XY from vertexCount.
        // Here we pre-initialize to the default 9-vertex polygon
        // (matching the default VertexCount="9" in C++ AutoExpand:1123).
        let n = 9;
        let vertices: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / n as f64;
                (a.cos() * 0.4 + 0.5, a.sin() * 0.4 + 0.5)
            })
            .collect();
        Self {
            vertices,
            drag_idx: None,
            drag_offset: (0.0, 0.0),
            show_handles: false,
            render_type: 0,
            with_canvas_color: false,
            texture: emColor::WHITE,
            stroke: emStroke::new(emColor::BLACK, 0.01),
        }
    }

    /// C++ `CanvasPanel::Setup` (emTestPanel.cpp:1275–1299).
    /// Called from PolyDrawPanel::Cycle() when controls change.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn Setup(
        &mut self,
        render_type: u8,
        vertex_count: usize,
        with_canvas_color: bool,
        texture: emColor,
        stroke_width: f64,
        stroke: emStroke,
        stroke_start: emStrokeEnd,
        stroke_end: emStrokeEnd,
    ) {
        self.render_type = render_type;

        // C++ cpp:1285–1292: resize XY array (2 coords per vertex).
        // When shrinking, drop trailing vertices and clear drag; when growing,
        // append new vertices on a circle of radius 0.4 centred at (0.5, 0.5).
        // C++ uses GetHeight() to scale the y-axis (XY.Set(i*2+1,
        //   GetHeight()*(sin(...)*0.4+0.5))), but Setup() is called from Cycle()
        // before a Paint context is available, so height is unknown here.
        // We use 1.0 as the height placeholder; the Rust Paint path already
        // scales vertices to (w, h) space, so the observable output is identical.
        if self.vertices.len() > vertex_count {
            self.vertices.truncate(vertex_count);
            self.drag_idx = None;
        } else if self.vertices.len() < vertex_count {
            let current_len = self.vertices.len();
            for i in current_len..vertex_count {
                let angle = PI * 2.0 * i as f64 / vertex_count as f64;
                let x = angle.cos() * 0.4 + 0.5;
                let y = angle.sin() * 0.4 + 0.5;
                self.vertices.push((x, y));
            }
            self.drag_idx = None;
        }

        self.with_canvas_color = with_canvas_color;
        self.texture = texture;
        // C++ cpp:1295–1298: StrokeWidth, Stroke, StrokeStart, StrokeEnd stored separately.
        // Fold all four into self.stroke: width from stroke_width, start_end from stroke_start,
        // finish_end from stroke_end (Rust emStroke encodes these inline).
        let mut s = stroke;
        s.width = stroke_width;
        s.start_end = stroke_start;
        s.finish_end = stroke_end;
        self.stroke = s;
        // C++ cpp:1299: InvalidatePainting() — triggers repaint.
        // In Rust, painting is always recomputed from the current frame state;
        // no explicit invalidation is needed.
    }
}

impl PanelBehavior for CanvasPanel {
    fn IsOpaque(&self) -> bool {
        true
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        let mx = event.mouse_x;
        let my = event.mouse_y;
        // C++ cpp:1355: GetHeight() — panel height in panel-local coordinate system.
        let panel_h = state.height;

        // C++ cpp:1315–1317: left-press → Eat() and Focus() unconditionally BEFORE vertex search.
        // Focus() is omitted: Rust focus-on-click is handled by the window dispatch loop before Input fires.
        // DIVERGED: (language-forced) C++ calls event.Eat() via non-const reference at cpp:1315,
        // consuming the event unconditionally on any left-press. PanelBehavior::Input takes
        // &emInputEvent (not &mut), and dispatch_input discards the bool return value, so no
        // in-trait eat-equivalent exists. The observable difference: in C++, any left-click on the
        // canvas panel is consumed even if no vertex was hit; in Rust it propagates. Fixing this
        // requires an emcore-wide trait change (or Cell<bool> on emInputEvent.eaten).
        if self.drag_idx.is_none()
            && event.key == InputKey::MouseLeft
            && event.variant == InputVariant::Press
        {
            // Threshold: 12 view-pixels in panel space (ViewToPanelDeltaX(12)).
            // panel_to_view_x(1) - panel_to_view_x(0) = viewed_width in pixels.
            let viewed_w = ctx.panel_to_view_x(1.0) - ctx.panel_to_view_x(0.0);
            let threshold = if viewed_w > 0.0 {
                12.0 / viewed_w
            } else {
                0.03
            };

            let mut best_i = None;
            let mut best_r = threshold;
            for (i, &(vx, vy)) in self.vertices.iter().enumerate() {
                let dx = vx - mx;
                let dy = vy - my;
                let r = (dx * dx + dy * dy).sqrt();
                if r < best_r {
                    best_i = Some(i);
                    best_r = r;
                }
            }
            if let Some(idx) = best_i {
                self.drag_idx = Some(idx);
                self.drag_offset = (self.vertices[idx].0 - mx, self.vertices[idx].1 - my);
                // C++ cpp:1331: InvalidatePainting() after setting DragIdx — no-op in Rust (view repaints every frame).
            }
        }

        // C++ Input: left-release → stop drag
        if self.drag_idx.is_some() && !input_state.GetLeftButton() {
            self.drag_idx = None;
            // C++ cpp:1336: InvalidatePainting() — no-op in Rust.
        }

        // C++ Input: dragging → update vertex position; shift/ctrl/alt → snap to grid
        if let Some(idx) = self.drag_idx {
            // C-7: y-bounds use GetHeight() (panel_h), not hardcoded 1.0 (cpp:1340–1341).
            let raw_x = (mx + self.drag_offset.0).clamp(0.0, 1.0);
            let raw_y = (my + self.drag_offset.1).clamp(0.0, panel_h);
            let (x, y) = if input_state.GetShift() || input_state.GetCtrl() || input_state.GetAlt()
            {
                // C++ snapping: find r s.t. PanelToViewDeltaX(r) <= 20px
                let viewed_w = ctx.panel_to_view_x(1.0) - ctx.panel_to_view_x(0.0);
                let mut r = 0.1;
                while viewed_w > 0.0 && r * viewed_w > 20.0 {
                    r *= 0.5;
                }
                ((raw_x / r).round() * r, (raw_y / r).round() * r)
            } else {
                (raw_x, raw_y)
            };
            // C-9: change-guard before vertex write (cpp:1344–1347).
            if self.vertices[idx].0 != x || self.vertices[idx].1 != y {
                self.vertices[idx] = (x, y);
                // C++ cpp:1347: InvalidatePainting() — no-op in Rust (view repaints every frame).
            }
        }

        // C++ Input: ShowHandles = dragging or mouse inside panel bounds.
        // C-7: y-bound uses panel_h (GetHeight()), not 1.0 (cpp:1355).
        let inside =
            self.drag_idx.is_some() || ((0.0..1.0).contains(&mx) && (0.0..panel_h).contains(&my));
        if self.show_handles != inside {
            self.show_handles = inside;
            // C++ cpp:1358: InvalidatePainting() — no-op in Rust.
        }

        // C-26: C++ cpp:1361: emPanel::Input(event, state, mx, my) — base handles cursor + bookkeeping.
        // Rust base Input is a no-op; call preserved for fidelity.

        false
    }

    fn Paint(
        &mut self,
        p: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        // C++ cpp:1372–1386: WithCanvasColor branch for background.
        let effective_canvas_color = if self.with_canvas_color {
            let c = emColor::rgb(96, 128, 160);
            p.ClearWithCanvas(c, canvas_color);
            c
        } else {
            p.paint_linear_gradient(
                0.0,
                0.0,
                w,
                h,
                emColor::rgba(80, 80, 160, 255),
                emColor::rgba(160, 160, 80, 255),
                false,
                canvas_color,
            );
            emColor::TRANSPARENT
        };

        // C++ cpp:1388–1403: compute bounding box and arc angles from vertices.
        // Vertices are stored normalized [0..1]; scale to panel (w, h) space.
        let (x1, y1, x2, y2, bx, by, bw, bh) = if self.vertices.len() >= 2 {
            let (vx1, vy1) = (self.vertices[0].0 * w, self.vertices[0].1 * h);
            let (vx2, vy2) = (self.vertices[1].0 * w, self.vertices[1].1 * h);
            let bx = vx1.min(vx2);
            let by = vy1.min(vy2);
            let bw = (vx2 - vx1).abs();
            let bh = (vy2 - vy1).abs();
            (vx1, vy1, vx2, vy2, bx, by, bw, bh)
        } else {
            // C++ cpp:1388: initializes x1=y1=x2=y2=0.0 when XY.GetCount()<4.
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        };

        // C++ cpp:1397–1403: arc start/range angles from vertices 2 and 3.
        let (sa, ra) = if self.vertices.len() >= 4 {
            let cx = bx + bw * 0.5;
            let cy = by + bh * 0.5;
            let (v2x, v2y) = (self.vertices[2].0 * w, self.vertices[2].1 * h);
            let (v3x, v3y) = (self.vertices[3].0 * w, self.vertices[3].1 * h);
            let sa = (v2y - cy).atan2(v2x - cx);
            let mut ra = (v3y - cy).atan2(v3x - cx) - sa;
            if ra < 0.0 {
                ra += 2.0 * std::f64::consts::PI;
            }
            (
                sa * 180.0 / std::f64::consts::PI,
                ra * 180.0 / std::f64::consts::PI,
            )
        } else {
            // C++ cpp:1397: initializes sa=ra=0.0 when XY.GetCount()<8; ra=0 means arc draws nothing.
            (0.0, 0.0)
        };

        // Scale vertices to panel (w, h) space for drawing.
        let scaled: Vec<(f64, f64)> = self
            .vertices
            .iter()
            .map(|&(vx, vy)| (vx * w, vy * h))
            .collect();

        // C++ cpp:1404–1466: 16-way render type switch.
        // DIVERGED: (language-forced) Several Rust methods differ from C++ signatures:
        //   - PaintPolygonOutline takes (vertices, stroke_color, thickness, canvas_color)
        //     vs C++ (xy, n, thickness, stroke, canvas_color); Rust builds emStroke internally.
        //   - PaintPolyline/PaintBezierLine/PaintBezierOutline take &emStroke (with
        //     start_end/finish_end inline) vs C++ (xy, n, thickness, stroke, start, end, canvas).
        //   - paint_line_stroked is the stroked equivalent of C++ PaintLine with thickness/ends;
        //     plain PaintLine in Rust is 1-pixel only.
        //   - PaintRectOutline/PaintEllipseOutline/PaintEllipseSectorOutline/PaintEllipseArc/
        //     PaintRoundRectOutline take &emStroke (which encodes width) vs C++ separate args.
        match self.render_type {
            0 => p.PaintPolygon(&scaled, self.texture, effective_canvas_color),
            1 => p.PaintPolygonOutline(
                &scaled,
                self.stroke.color,
                self.stroke.width,
                effective_canvas_color,
            ),
            2 => p.PaintPolyline(&scaled, &self.stroke, false, effective_canvas_color),
            3 => p.PaintBezier(&scaled, self.texture, effective_canvas_color),
            4 => p.PaintBezierOutline(&scaled, &self.stroke, effective_canvas_color),
            5 => p.PaintBezierLine(&scaled, &self.stroke, effective_canvas_color),
            6 => p.paint_line_stroked(x1, y1, x2, y2, &self.stroke, effective_canvas_color),
            7 => p.PaintRect(bx, by, bw, bh, self.texture, effective_canvas_color),
            8 => p.PaintRectOutline(bx, by, bw, bh, &self.stroke, effective_canvas_color),
            9 => p.PaintEllipse(bx, by, bw, bh, self.texture, effective_canvas_color),
            10 => p.PaintEllipseOutline(bx, by, bw, bh, &self.stroke, effective_canvas_color),
            11 => {
                p.PaintEllipseSector(bx, by, bw, bh, sa, ra, self.texture, effective_canvas_color)
            }
            12 => p.PaintEllipseSectorOutline(
                bx,
                by,
                bw,
                bh,
                sa,
                ra,
                &self.stroke,
                effective_canvas_color,
            ),
            13 => p.PaintEllipseArc(bx, by, bw, bh, sa, ra, &self.stroke, effective_canvas_color),
            14 => p.PaintRoundRect(
                bx,
                by,
                bw,
                bh,
                bw * 0.2,
                bh * 0.2,
                self.texture,
                effective_canvas_color,
            ),
            15 => p.PaintRoundRectOutline(
                bx,
                by,
                bw,
                bh,
                bw * 0.2,
                bh * 0.2,
                &self.stroke,
                effective_canvas_color,
            ),
            _ => {}
        }

        // C++ Paint: draw vertex handles when ShowHandles (cpp:1463–1483).
        if self.show_handles {
            // C-10: C++ cpp:1464: r = emMin(ViewToPanelDeltaX(12.0), 0.05).
            // ViewToPanelDeltaX(12) = 12 * panel_width / viewed_width_in_pixels.
            // Panel width is always 1.0; viewed_rect.w is width in view (screen) pixels.
            let viewed_w = state.viewed_rect.w.max(1.0);
            let r = (12.0 / viewed_w).min(0.05);
            // Scale r to painter space (painter coords = panel coords * w).
            let r_paint = r * w;
            let n = scaled.len();
            // Compute active vertex count m based on render_type (cpp:1471–1478).
            let m = if self.render_type >= 3 && self.render_type <= 4 {
                n - n % 3
            } else if self.render_type == 5 {
                n - (n + 2) % 3
            } else if self.render_type >= 11 && self.render_type <= 13 {
                4
            } else if self.render_type >= 6 {
                2
            } else {
                n
            };
            for (i, &(vx, vy)) in scaled.iter().enumerate() {
                // C++ cpp:1467–1470: yellow for non-anchor bezier control points or extra
                // vertices on fixed-count types, green for anchor points.
                let mut c = if (self.render_type >= 3 && self.render_type <= 5 && i % 3 != 0)
                    || (self.render_type >= 6 && i > 1)
                {
                    emColor::rgba(255, 255, 0, 128) // yellow: non-anchor control points
                } else {
                    emColor::rgba(0, 255, 0, 128) // green: anchor points
                };
                // C++ cpp:1479: gray for unused vertices beyond active count m.
                if i >= m {
                    c = emColor::rgba(128, 128, 128, 128);
                }
                // C++ cpp:1480: blend with white when dragging this vertex.
                if self.drag_idx == Some(i) {
                    c = c.GetBlended(emColor::rgba(255, 255, 255, 128), 75.0);
                }
                p.PaintEllipse(
                    vx - r_paint,
                    vy - r_paint,
                    2.0 * r_paint,
                    2.0 * r_paint,
                    c,
                    emColor::TRANSPARENT,
                );
                let outline = emStroke::new(emColor::rgba(0, 0, 0, 128), r_paint * 0.15);
                p.PaintEllipseOutline(
                    vx - r_paint,
                    vy - r_paint,
                    2.0 * r_paint,
                    2.0 * r_paint,
                    &outline,
                    emColor::TRANSPARENT,
                );
            }
        }

        // C-11: C++ cpp:1485–1490: PaintTextBoxed(0.0, GetHeight()-0.03, 1.0, 0.03, ..., 0.03).
        // Fixed values in panel-local coords; h corresponds to GetHeight() (panel tallness).
        // Paint coords are panel-local * w, so multiply by w for painter space.
        p.PaintTextBoxed(
            0.0,
            (h - 0.03) * w,
            1.0 * w,
            0.03 * w,
            "The vertices can be dragged with the left mouse button!\n(Hold shift for raster)\n",
            0.03 * w,
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
}

// ─── helpers ────────────────────────────────────────────────────────

// ─── plugin entry ───────────────────────────────────────────────────

pub(crate) fn new_root_panel(ctx: &mut dyn ConstructCtx) -> Box<dyn PanelBehavior> {
    let root_ctx = ctx.root_context().clone();
    Box::new(TestPanel::new(root_ctx, DEFAULT_BG))
}

#[cfg(any(test, feature = "test-support"))]
pub fn new_poly_draw_panel() -> Box<dyn PanelBehavior> {
    Box::new(PolyDrawPanel::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emContext::emContext;
    use emcore::emPanelTree::PanelTree;
    use emcore::emView::emView;
    use emcore::test_view_harness::TestSched;
    use std::rc::Rc;

    /// Drive 5 HandleNotice + Update rounds.
    ///
    /// Tests using this helper care about structural outcomes (tree shape,
    /// threshold values), not signal observations. Pending signals fired by
    /// behaviors during the settle rounds are discarded explicitly — the
    /// intent is declared at the call site rather than relying on a silent
    /// Drop that would mask real regressions in other tests.
    fn settle(tree: &mut PanelTree, view: &mut emView, ctx: &Rc<emContext>) {
        let mut ts = TestSched::new();
        for _ in 0..5 {
            ts.with(|sc| {
                view.HandleNotice(
                    tree,
                    sc.scheduler,
                    Some(ctx),
                    None,
                    sc.framework_actions,
                    sc.framework_clipboard,
                    sc.pending_actions,
                )
            });
            ts.with(|sc| view.Update(tree, sc));
        }
        // Discard signals fired during the settle rounds — these tests check
        // structural outcomes, not which signals were emitted. Explicit call
        // required because TestSched::Drop now loud-fails on non-empty queues.
        ts.clear_pending_signals_for_test_discard();
    }

    #[test]
    fn test_panel_auto_expands_children() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(TestPanel::new(ctx.clone(), DEFAULT_BG)));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(
            tree.GetFirstChild(root).is_some(),
            "TestPanel should have children after AutoExpand fires"
        );
        assert!(
            tree.find_by_name("TkTestGrp").is_some(),
            "TkTestGrp missing"
        );
        assert!(
            tree.find_by_name("BgColorField").is_some(),
            "BgColorField missing"
        );
        assert!(tree.find_by_name("PolyDraw").is_some(), "PolyDraw missing");
        assert!(tree.find_by_name("1").is_some(), "TP1 missing");
        assert!(tree.find_by_name("4").is_some(), "TP4 missing");
    }

    #[test]
    fn tktestgrp_auto_expands_children() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(TkTestGrpPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(tree.find_by_name("t1a").is_some(), "t1a missing");
        assert!(tree.find_by_name("t1b").is_some(), "t1b missing");
        assert!(tree.find_by_name("t2a").is_some(), "t2a missing");
        assert!(tree.find_by_name("t2b").is_some(), "t2b missing");
    }

    #[test]
    fn polydrawpanel_auto_expands_canvas() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(PolyDrawPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(
            tree.find_by_name("CanvasPanel").is_some(),
            "CanvasPanel missing"
        );
    }

    /// Structural test for sf5↔sf6 wiring — no Cell intermediary.
    ///
    /// Verifies that `TkTestPanel` expands and settles without panic and that
    /// both sf5 ("Play Length") and sf6 ("Play Position") are present in the
    /// tree. The `ScalarFieldWithDynamicMax` wrapper, which replaced the removed
    /// `Cell` polling intermediary, must initialize and run through at least one
    /// `Cycle` without error.
    ///
    /// This is a smoke + structural test; golden tests verify pixel-level
    /// correctness. A regression to the Cell pattern would require adding fields
    /// and wiring that cause compilation errors (Cell<f64> removed; this test
    /// confirms the happy path compiles and runs).
    #[test]
    fn tktestpanel_expands_sf5_and_sf6() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(TkTestGrpPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        // After AutoExpand + settle, the first TkTestPanel child (t1a) should have
        // expanded its scalar-fields group, which contains sf5 and sf6.
        assert!(
            tree.find_by_name("sf5").is_some(),
            "sf5 (Play Length ScalarField) must exist after TkTestPanel AutoExpand"
        );
        assert!(
            tree.find_by_name("sf6").is_some(),
            "sf6 (Play Position ScalarFieldWithDynamicMax) must exist after TkTestPanel AutoExpand"
        );
    }

    #[test]
    fn polydrawpanel_control_tree_exists() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(PolyDrawPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(
            tree.find_by_name("Controls").is_some(),
            "Controls raster missing"
        );
        assert!(
            tree.find_by_name("CanvasPanel").is_some(),
            "CanvasPanel missing"
        );
        assert!(
            tree.find_by_name("Method").is_some(),
            "Method RadioGroup missing"
        );
        assert!(
            tree.find_by_name("VertexCount").is_some(),
            "VertexCount missing"
        );
        assert!(
            tree.find_by_name("FillColor").is_some(),
            "FillColor missing"
        );
        assert!(
            tree.find_by_name("WithCanvasColor").is_some(),
            "WithCanvasColor missing"
        );
        assert!(
            tree.find_by_name("StrokeWidth").is_some(),
            "StrokeWidth missing"
        );
        assert!(
            tree.find_by_name("StrokeColor").is_some(),
            "StrokeColor missing"
        );
        assert!(
            tree.find_by_name("StrokeRounded").is_some(),
            "StrokeRounded missing"
        );
        assert!(
            tree.find_by_name("StrokeDashType").is_some(),
            "StrokeDashType missing"
        );
        assert!(
            tree.find_by_name("DashLengthFactor").is_some(),
            "DashLengthFactor missing"
        );
        assert!(
            tree.find_by_name("GapLengthFactor").is_some(),
            "GapLengthFactor missing"
        );
        assert!(
            tree.find_by_name("StrokeStartType").is_some(),
            "StrokeStartType missing"
        );
        assert!(
            tree.find_by_name("StrokeStartInnerColor").is_some(),
            "StrokeStartInnerColor missing"
        );
        assert!(
            tree.find_by_name("StrokeStartWidthFactor").is_some(),
            "StrokeStartWidthFactor missing"
        );
        assert!(
            tree.find_by_name("StrokeStartLengthFactor").is_some(),
            "StrokeStartLengthFactor missing"
        );
        assert!(
            tree.find_by_name("StrokeEndType").is_some(),
            "StrokeEndType missing"
        );
        assert!(
            tree.find_by_name("StrokeEndInnerColor").is_some(),
            "StrokeEndInnerColor missing"
        );
        assert!(
            tree.find_by_name("StrokeEndWidthFactor").is_some(),
            "StrokeEndWidthFactor missing"
        );
        assert!(
            tree.find_by_name("StrokeEndLengthFactor").is_some(),
            "StrokeEndLengthFactor missing"
        );
    }

    #[test]
    fn test_panel_ae_threshold_is_900() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(TestPanel::new(ctx.clone(), DEFAULT_BG)));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        // After AutoExpand fires, the panel should have reset its own threshold to 900.0.
        assert_eq!(
            tree.GetAutoExpansionThresholdValue(root),
            900.0,
            "TestPanel AE threshold should be 900.0 after AutoExpand (C++ constructor value)"
        );
    }

    #[test]
    fn polydrawpanel_cycle_wires_canvas() {
        // Smoke test: PolyDrawPanel settles with a complete control tree and CanvasPanel.
        // Cycle connects 18 signals on the first post-AutoExpand frame; the initial Setup
        // call is made directly in AutoExpand (C++ first-Cycle timing compat).
        // Pixel-level correctness of the 16-way render switch is verified by golden tests.
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(PolyDrawPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);
        settle(&mut tree, &mut view, &ctx);

        assert!(
            tree.find_by_name("CanvasPanel").is_some(),
            "CanvasPanel absent"
        );
        assert!(tree.find_by_name("Method").is_some(), "Method group absent");
        assert!(
            tree.find_by_name("VertexCount").is_some(),
            "VertexCount absent"
        );
        // After settle, the tree is structurally complete and stable (no panic from Setup).
    }
}
