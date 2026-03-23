use std::rc::Rc;

use crate::emCore::emCursor::emCursor;
use crate::emCore::emInput::emInputEvent;
use crate::emCore::emInputState::emInputState;
use crate::emCore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use crate::emCore::emPainter::emPainter;

use super::emBorder::{InnerBorderType, OuterBorderType};
use crate::emCore::emButton::emButton;
use crate::emCore::emCheckBox::emCheckBox;
use crate::emCore::emLabel::emLabel;
use crate::emCore::emListBox::emListBox;
use crate::emCore::emLook::emLook;
use crate::emCore::emScalarField::emScalarField;
use crate::emCore::emTextField::emTextField;

/// PanelBehavior wrapper for emScalarField — used by emColorField expansion.
pub(crate) struct ScalarFieldPanel {
    pub scalar_field: emScalarField,
}

impl ScalarFieldPanel {
    pub fn new(
        caption: &str,
        min: f64,
        max: f64,
        value: f64,
        look: Rc<emLook>,
        editable: bool,
    ) -> Self {
        let mut sf = emScalarField::new(min, max, look);
        sf.set_caption(caption);
        sf.set_value(value);
        sf.set_editable(editable);
        sf.border_mut().outer = OuterBorderType::Rect;
        sf.border_mut().inner = InnerBorderType::CustomRect;
        sf.border_mut().set_border_scaling(2.0);
        Self { scalar_field: sf }
    }
}

impl PanelBehavior for ScalarFieldPanel {
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.scalar_field.paint(painter, w, h, state.enabled);
    }

    fn input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        self.scalar_field.input(event, _state, _input_state)
    }

    fn get_cursor(&self) -> emCursor {
        self.scalar_field.get_cursor()
    }
}

/// PanelBehavior wrapper for emTextField — used by emColorField expansion.
pub(crate) struct TextFieldPanel {
    pub text_field: emTextField,
}

impl TextFieldPanel {
    pub fn new(caption: &str, text: &str, look: Rc<emLook>, editable: bool) -> Self {
        let mut tf = emTextField::new(look);
        tf.set_caption(caption);
        tf.set_text(text);
        tf.set_editable(editable);
        tf.border_mut().outer = OuterBorderType::Rect;
        tf.border_mut().inner = InnerBorderType::CustomRect;
        tf.border_mut().set_border_scaling(2.0);
        Self { text_field: tf }
    }
}

impl PanelBehavior for TextFieldPanel {
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.text_field.cycle_blink(state.in_focused_path());
        self.text_field.paint(painter, w, h, state.enabled);
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
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
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.check_box.paint(painter, w, h, state.enabled);
    }

    fn input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        self.check_box.input(event, _state, _input_state)
    }

    fn get_cursor(&self) -> emCursor {
        self.check_box.get_cursor()
    }
}

/// PanelBehavior wrapper for emListBox.
pub(crate) struct ListBoxPanel {
    pub list_box: emListBox,
}

impl PanelBehavior for ListBoxPanel {
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        self.list_box.paint(painter, w, h);
    }

    fn input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        self.list_box.input(event, _state, _input_state)
    }

    fn notice(&mut self, flags: NoticeFlags, state: &PanelState) {
        if flags.intersects(NoticeFlags::FOCUS_CHANGED) {
            self.list_box.on_focus_changed(state.in_active_path);
        }
        if flags.intersects(NoticeFlags::ENABLE_CHANGED) {
            self.list_box.on_enable_changed(state.enabled);
        }
    }
}

/// PanelBehavior wrapper for emButton.
pub(crate) struct ButtonPanel {
    pub button: emButton,
}

impl PanelBehavior for ButtonPanel {
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        self.button.paint(painter, w, h, state.enabled);
    }

    fn input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool {
        self.button.input(event, _state, _input_state)
    }

    fn get_cursor(&self) -> emCursor {
        self.button.get_cursor()
    }
}

/// PanelBehavior wrapper for emLabel (non-focusable text display).
pub(crate) struct LabelPanel {
    pub label: emLabel,
}

impl PanelBehavior for LabelPanel {
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        self.label.PaintContent(painter, w, h, _state.enabled);
    }
}
