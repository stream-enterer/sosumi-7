use crate::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use crate::emLook::emLook;
use crate::emPainter::emPainter;
use crate::emPanel::{PanelBehavior, PanelState};
use crate::emEngineCtx::PanelCtx;

use crate::emPackLayout::emPackLayout;

/// emPackGroup wraps emPackLayout with border painting and focusable support.
pub struct emPackGroup {
    pub layout: emPackLayout,
    pub border: emBorder,
    pub look: emLook,
}

impl emPackGroup {
    pub fn new() -> Self {
        Self {
            layout: emPackLayout::new(),
            border: emBorder::new(OuterBorderType::Group).with_inner(InnerBorderType::Group),
            look: emLook::default(),
        }
    }
}

impl Default for emPackGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelBehavior for emPackGroup {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, state: &PanelState) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.border.paint_border(
            painter,
            w,
            h,
            &self.look,
            state.is_focused(),
            state.enabled,
            pixel_scale,
        );
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        let aux_id = super::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRectUnobscured(r.w, r.h, &self.look);
        self.layout.do_layout_skip(ctx, aux_id, Some(cr));
        let cc =
            self.border
                .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn auto_expand(&self) -> bool {
        true
    }
}
