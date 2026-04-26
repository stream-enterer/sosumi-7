// SPLIT: Split from emColorField.h — inner panel type extracted
use std::rc::Rc;

use crate::emColor::emColor;
use crate::emCursor::emCursor;
use crate::emEngineCtx::PanelCtx;
use crate::emInput::emInputEvent;
use crate::emInputState::emInputState;
use crate::emPainter::emPainter;
use crate::emPanel::{NoticeFlags, PanelBehavior, PanelState};

use super::emBorder::{InnerBorderType, OuterBorderType};
use crate::emButton::emButton;
use crate::emCheckBox::emCheckBox;
use crate::emLabel::emLabel;
use crate::emListBox::emListBox;
use crate::emLook::emLook;
use crate::emScalarField::emScalarField;
use crate::emTextField::emTextField;

/// PanelBehavior wrapper for emScalarField — used by emColorField expansion.
pub(crate) struct ScalarFieldPanel {
    pub scalar_field: emScalarField,
}

impl ScalarFieldPanel {
    pub fn new<C: crate::emEngineCtx::ConstructCtx>(
        ctx: &mut C,
        caption: &str,
        min: f64,
        max: f64,
        value: f64,
        look: Rc<emLook>,
        editable: bool,
    ) -> Self {
        let mut sf = emScalarField::new(ctx, min, max, look);
        sf.SetCaption(caption);
        sf.set_initial_value(value);
        sf.SetEditable(editable);
        sf.border_mut().outer = OuterBorderType::Rect;
        sf.border_mut().inner = InnerBorderType::CustomRect;
        sf.border_mut().SetBorderScaling(2.0);
        Self { scalar_field: sf }
    }
}

impl PanelBehavior for ScalarFieldPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.scalar_field
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.scalar_field.Input(event, _state, _input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.scalar_field.GetCursor()
    }
}

/// PanelBehavior wrapper for emTextField — used by emColorField expansion.
pub(crate) struct TextFieldPanel {
    pub text_field: emTextField,
}

impl TextFieldPanel {
    pub fn new<C: crate::emEngineCtx::ConstructCtx>(
        ctx: &mut C,
        caption: &str,
        text: &str,
        look: Rc<emLook>,
        editable: bool,
    ) -> Self {
        let mut tf = emTextField::new(ctx, look);
        tf.SetCaption(caption);
        tf.SetText(text);
        tf.SetEditable(editable);
        tf.border_mut().outer = OuterBorderType::Rect;
        tf.border_mut().inner = InnerBorderType::CustomRect;
        tf.border_mut().SetBorderScaling(2.0);
        Self { text_field: tf }
    }
}

impl PanelBehavior for TextFieldPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.text_field.cycle_blink(state.in_focused_path());
        self.text_field
            .Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.text_field.on_focus_changed(state.in_focused_path());
        }
    }
}

/// PanelBehavior wrapper for emCheckBox.
pub(crate) struct CheckBoxPanel {
    pub check_box: emCheckBox,
}

impl PanelBehavior for CheckBoxPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.check_box
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.check_box.Input(event, _state, _input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.check_box.GetCursor()
    }
}

/// PanelBehavior wrapper for emListBox.
pub(crate) struct ListBoxPanel {
    pub list_box: emListBox,
}

impl PanelBehavior for ListBoxPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.list_box.Paint(painter, w, h, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.list_box.Input(event, _state, _input_state, _ctx)
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState, _ctx: &mut PanelCtx) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.list_box.on_focus_changed(state.in_active_path);
        }
        if flags.intersects(NoticeFlags::ENABLE_CHANGED) {
            self.list_box.on_enable_changed(state.enabled);
        }
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
        // Port of C++ emListBox::AutoExpand(): create item child panels.
        self.list_box.create_item_children(ctx);
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let rect = ctx.layout_rect();
        // C++ always calls layout in the panel's own coordinate space (w=1.0 normalized).
        // Pass tallness = h/w so GetContentRectUnobscured gets the same aspect ratio as C++.
        let tallness = if rect.w > 1e-100 {
            rect.h / rect.w
        } else {
            1.0
        };
        self.list_box.layout_item_children(ctx, 1.0, tallness);
    }
}

/// PanelBehavior wrapper for emButton.
pub(crate) struct ButtonPanel {
    pub button: emButton,
}

impl PanelBehavior for ButtonPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.button
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.button.Input(event, _state, _input_state, _ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.button.GetCursor()
    }
}

/// PanelBehavior wrapper for emLabel (non-focusable text display).
pub(crate) struct LabelPanel {
    pub label: emLabel,
}

impl PanelBehavior for LabelPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.label
            .PaintContent(painter, w, h, state.enabled, pixel_scale);
    }
}
