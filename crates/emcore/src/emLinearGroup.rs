use crate::emBorder::{emBorder, InnerBorderType, OuterBorderType};
use crate::emColor::emColor;
use crate::emEngineCtx::PanelCtx;
use crate::emLook::emLook;
use crate::emPainter::emPainter;
use crate::emPanel::{PanelBehavior, PanelState};

use crate::emLinearLayout::emLinearLayout;

/// emLinearGroup: a emLinearLayout that also paints a border and is focusable.
/// Replicates C++ emLinearGroup which inherits from emLinearLayout (which
/// inherits from emBorder).
pub struct emLinearGroup {
    pub layout: emLinearLayout,
    pub border: emBorder,
    pub look: emLook,
}

impl emLinearGroup {
    pub fn horizontal() -> Self {
        Self {
            layout: emLinearLayout::horizontal(),
            border: emBorder::new(OuterBorderType::Group).with_inner(InnerBorderType::Group),
            look: emLook::default(),
        }
    }

    pub fn vertical() -> Self {
        Self {
            layout: emLinearLayout::vertical(),
            border: emBorder::new(OuterBorderType::Group).with_inner(InnerBorderType::Group),
            look: emLook::default(),
        }
    }
}

impl PanelBehavior for emLinearGroup {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
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
        // C++ base-call: position aux panel first, then layout remaining children
        let aux_id = super::emTiling::position_aux_panel(ctx, &self.border);
        let r = ctx.layout_rect();
        let cr = self.border.GetContentRect(r.w, r.h, &self.look);
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
