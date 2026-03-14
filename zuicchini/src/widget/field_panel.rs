use std::rc::Rc;

use crate::panel::{PanelBehavior, PanelState};
use crate::render::Painter;

use super::border::{InnerBorderType, OuterBorderType};
use super::look::Look;
use super::scalar_field::ScalarField;
use super::text_field::TextField;

/// PanelBehavior wrapper for ScalarField — used by ColorField expansion.
pub(crate) struct ScalarFieldPanel {
    pub scalar_field: ScalarField,
}

impl ScalarFieldPanel {
    pub fn new(
        caption: &str,
        min: f64,
        max: f64,
        value: f64,
        look: Rc<Look>,
        editable: bool,
    ) -> Self {
        let mut sf = ScalarField::new(min, max, look);
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
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.scalar_field.paint(painter, w, h, state.enabled);
    }
}

/// PanelBehavior wrapper for TextField — used by ColorField expansion.
pub(crate) struct TextFieldPanel {
    pub text_field: TextField,
}

impl TextFieldPanel {
    pub fn new(caption: &str, text: &str, look: Rc<Look>, editable: bool) -> Self {
        let mut tf = TextField::new(look);
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
    fn paint(&mut self, painter: &mut Painter, w: f64, h: f64, state: &PanelState) {
        self.text_field.paint(painter, w, h, state.enabled);
    }
}
