use std::cell::OnceCell;
use std::rc::Rc;

use crate::emCore::emColor::emColor;
use crate::emCore::emImage::emImage;
use crate::emCore::rect::Rect;
use crate::emCore::emPanel::{NoticeFlags, PanelBehavior, PanelState};
use crate::emCore::emPanelCtx::PanelCtx;
use crate::emCore::emPainter::emPainter;

use super::emBorder::{emBorder, OuterBorderType};
use crate::emCore::emLook::emLook;

/// emTunnel image loaded once from the toolkit resources.
fn tunnel_image() -> emImage {
    thread_local! {
        static IMG: OnceCell<emImage> = const { OnceCell::new() };
    }
    IMG.with(|cell| {
        cell.get_or_init(|| {
            crate::emCore::emResTga::load_tga(include_bytes!("../../res/toolkit/Tunnel.tga"))
                .expect("failed to decode Tunnel.tga")
        })
        .clone()
    })
}

/// A panel that creates a visual tunnel/zoom corridor to a child panel.
///
/// Draws concentric rounded rectangles connecting the outer border's content
/// area to a smaller inner rectangle, creating the characteristic Eagle Mode
/// "zoom into content" visual effect. The child panel is placed at the end
/// (innermost rectangle) of the tunnel.
///
/// Ported from C++ `emTunnel`.
pub struct emTunnel {
    border: emBorder,
    look: Rc<emLook>,
    /// Tallness (height/width) for the child panel at the tunnel end.
    /// A value <= 0.0 means use the tallness of the content rectangle.
    child_tallness: f64,
    /// Depth of the tunnel. Larger values make the child panel smaller.
    /// The relationship is roughly: area_end = area_entrance / ((depth+1)^2).
    depth: f64,
    /// Set when `set_child_tallness` or `set_depth` is called at runtime.
    /// Cleared by `layout_children`, which then re-queues `LAYOUT_CHANGED` on
    /// its own children so the next deliver_notices pass repositions them and
    /// triggers a full repaint (via `had_notices = true` in the app loop).
    /// Mirrors C++ `InvalidatePainting()` + `InvalidateChildrenLayout()`.
    layout_invalid: bool,
}

/// Result of the tunnel geometry calculation.
pub struct TunnelChildRect {
    /// X coordinate of the child rect origin.
    pub x: f64,
    /// Y coordinate of the child rect origin.
    pub y: f64,
    /// Width of the child rect.
    pub w: f64,
    /// Height of the child rect.
    pub h: f64,
    /// Canvas color at the tunnel end.
    pub canvas_color: emColor,
}

impl emTunnel {
    pub fn new(look: Rc<emLook>) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::Instrument)
                .with_inner(super::emBorder::InnerBorderType::Group),
            look,
            child_tallness: 0.0,
            depth: 10.0,
            layout_invalid: false,
        }
    }

    /// Builder: set a caption.
    pub fn with_caption(mut self, caption: &str) -> Self {
        self.border.caption = caption.to_string();
        self
    }

    /// Builder: set a description.
    pub fn with_description(mut self, description: &str) -> Self {
        self.border.description = description.to_string();
        self
    }

    pub fn GetChildTallness(&self) -> f64 {
        self.child_tallness
    }

    pub fn SetChildTallness(&mut self, tallness: f64) {
        self.child_tallness = tallness;
        self.layout_invalid = true;
    }

    pub fn GetDepth(&self) -> f64 {
        self.depth
    }

    pub fn SetDepth(&mut self, depth: f64) {
        let depth = if depth < 1e-10 { 1e-10 } else { depth };
        self.depth = depth;
        self.layout_invalid = true;
    }

    pub(crate) fn border_mut(&mut self) -> &mut emBorder {
        &mut self.border
    }

    /// Compute the geometry of the tunnel's inner (child) rectangle.
    ///
    /// `parent_canvas` is the canvas color of this tunnel panel (i.e.
    /// `ctx.canvas_color()`). It is threaded through the border paint pipeline
    /// via `emBorder::content_canvas_color` to arrive at the correct color for
    /// the child panel — matching how C++ `DoTunnel` passes `cc` from
    /// `GetContentRoundRect` into `pCanvasColor`.
    pub fn GetChildRect(&self, w: f64, h: f64, parent_canvas: emColor) -> TunnelChildRect {
        let (rect, ar) = self.content_round_rect(w, h);
        let ax = rect.x;
        let ay = rect.y;
        let aw = rect.w;
        let ah = rect.h;

        let (bx, by, bw, bh, br) = self.compute_inner_rect(ax, ay, aw, ah, ar);

        // Child rect is the inner rect inset by half the corner radius.
        // Canvas color is computed by walking the border paint pipeline, matching
        // C++ GetContentRoundRect which returns `cc` after DoBorder's canvasColor
        // tracking (always enabled=true: emTunnel has no InputField/OutputField inner).
        TunnelChildRect {
            x: bx + 0.5 * br,
            y: by + 0.5 * br,
            w: bw - br,
            h: bh - br,
            canvas_color: self.border.content_canvas_color(parent_canvas, &self.look, true),
        }
    }

    /// Paint the tunnel decoration.
    pub fn paint_tunnel(&self, painter: &mut emPainter, w: f64, h: f64) {
        // Paint the border chrome first.
        self.border
            .paint_border(painter, w, h, &self.look, false, true, 1.0);

        let (rect, ar) = self.content_round_rect(w, h);
        let ax = rect.x;
        let ay = rect.y;
        let aw = rect.w;
        let ah = rect.h;

        if aw <= 0.0 || ah <= 0.0 {
            return;
        }

        let canvas_color = painter.canvas_color();
        let (bx, by, bw, bh, br) = self.compute_inner_rect(ax, ay, aw, ah, ar);

        let img = tunnel_image();
        let img_rx = img.width() as f64 * 0.5;
        let img_ry = img.height() as f64 * 0.5;

        // Determine tessellation quality based on corner radius and view scale.
        let (sx, sy) = painter.scaling();
        let circle_quality = 4.5;
        let f = circle_quality * (ar * (sx + sy)).sqrt();
        let f = f.min(256.0) * 0.25;
        let n: i32 = if f <= 1.0 {
            1
        } else if f >= 64.0 {
            64
        } else {
            (f + 0.5) as i32
        };

        let m = n * 4;

        // C++ uses a flat double xy[8] array representing 4 points:
        //   point0 = (xy[0], xy[1]), point1 = (xy[2], xy[3]),
        //   point2 = (xy[4], xy[5]), point3 = (xy[6], xy[7]).
        // ja/jb index into these 4 points. ja starts at 0, jb at 1.
        // They swap via ja ^= 3, jb ^= 3 => ja: 0,3,0,3... jb: 1,2,1,2...
        // At each step, points ja and jb are updated; the other two are kept
        // from the previous iteration, forming a quad strip.
        let mut xy = [0.0_f64; 8];
        let mut ja: usize = 0;
        let mut jb: usize = 1;

        for i in 0..=m {
            let f_mid = (i as f64 + 0.5) * std::f64::consts::TAU / m as f64;
            let dx = f_mid.cos();
            let dy = f_mid.sin();

            let quadrant = i / n;
            if (quadrant + 1) & 2 != 0 {
                xy[ja * 2] = ax + (dx + 1.0) * ar;
                xy[jb * 2] = bx + (dx + 1.0) * br;
            } else {
                xy[ja * 2] = ax + aw + (dx - 1.0) * ar;
                xy[jb * 2] = bx + bw + (dx - 1.0) * br;
            }
            if quadrant & 2 != 0 {
                xy[ja * 2 + 1] = ay + (dy + 1.0) * ar;
                xy[jb * 2 + 1] = by + (dy + 1.0) * br;
            } else {
                xy[ja * 2 + 1] = ay + ah + (dy - 1.0) * ar;
                xy[jb * 2 + 1] = by + bh + (dy - 1.0) * br;
            }

            if i > 0 {
                let f_edge = i as f64 * std::f64::consts::TAU / m as f64;
                let edge_dx = f_edge.cos();
                let edge_dy = f_edge.sin();

                // Sample color from the tunnel image at the edge angle.
                let ix = ((img_rx + (img_rx - 0.6) * edge_dx + 0.5) as u32).min(img.width() - 1);
                let iy = ((img_ry + (img_ry - 0.6) * edge_dy + 0.5) as u32).min(img.height() - 1);
                let pix = img.pixel(ix, iy);
                let color = if img.channel_count() >= 4 {
                    emColor::rgba(pix[0], pix[1], pix[2], pix[3])
                } else {
                    emColor::rgb(pix[0], pix[1], pix[2])
                };

                // Build the quad from the 4 points and paint it.
                let quad = [
                    (xy[0], xy[1]),
                    (xy[2], xy[3]),
                    (xy[4], xy[5]),
                    (xy[6], xy[7]),
                ];
                painter.paint_polygon(&quad, color, canvas_color);
            }

            ja ^= 3;
            jb ^= 3;
        }
    }

    /// Content round rect from the border.
    fn content_round_rect(&self, w: f64, h: f64) -> (Rect, f64) {
        self.border.content_round_rect(w, h, &self.look)
    }

    /// Compute the inner rectangle of the tunnel (before inset for child).
    fn compute_inner_rect(
        &self,
        ax: f64,
        ay: f64,
        aw: f64,
        ah: f64,
        ar: f64,
    ) -> (f64, f64, f64, f64, f64) {
        let d = 1.0 / (self.depth + 1.0);
        let mut bw = aw * d;
        let mut bh = ah * d;
        let mut br = ar * d;

        if self.child_tallness > 1e-100 {
            bw = ((bw - br) * (bh - br) / self.child_tallness).sqrt();
            bh = bw * self.child_tallness;
            br = ar / (aw.min(ah) - ar) * bw.min(bh);
            bw += br;
            bh += br;
            let f = aw * 0.999999 / bw;
            if f < 1.0 {
                bw *= f;
                bh *= f;
                br *= f;
            }
            let f = ah * 0.999999 / bh;
            if f < 1.0 {
                bw *= f;
                bh *= f;
                br *= f;
            }
        }

        let bx = ax + (aw - bw) * 0.5;
        let by = ay + (ah - bh) * 0.5;

        (bx, by, bw, bh, br)
    }
}

impl PanelBehavior for emTunnel {
    fn paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        self.paint_tunnel(painter, w, h);
    }

    fn auto_expand(&self) -> bool {
        true
    }

    fn layout_children(&mut self, ctx: &mut PanelCtx) {
        if !ctx.tree.is_auto_expanded(ctx.id) {
            return;
        }

        // Mirrors C++ InvalidatePainting() + InvalidateChildrenLayout(): when
        // set_child_tallness or set_depth is called at runtime, re-queue
        // LAYOUT_CHANGED on this panel's children so the next deliver_notices
        // pass repositions them. The had_notices=true path in the app loop
        // also causes a full repaint, matching InvalidatePainting().
        if self.layout_invalid {
            self.layout_invalid = false;
            ctx.tree.invalidate_children_layout(ctx.id);
            ctx.tree.queue_notice(ctx.id, NoticeFlags::LAYOUT_CHANGED);
        }

        let rect = ctx.layout_rect();
        let cr = self.GetChildRect(rect.w, rect.h, ctx.canvas_color());

        if let Some(&child) = ctx.children().first() {
            ctx.layout_child(child, cr.x, cr.y, cr.w, cr.h);
            ctx.tree.set_canvas_color(child, cr.canvas_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_default_depth() {
        let look = emLook::new();
        let tunnel = emTunnel::new(look);
        assert!((tunnel.depth() - 10.0).abs() < f64::EPSILON);
        assert!((tunnel.child_tallness() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tunnel_set_depth_clamps() {
        let look = emLook::new();
        let mut tunnel = emTunnel::new(look);
        tunnel.set_depth(-5.0);
        assert!(tunnel.depth() > 0.0);
    }

    #[test]
    fn tunnel_child_rect_is_inside_content() {
        let look = emLook::new();
        let tunnel = emTunnel::new(look);
        let cr = tunnel.child_rect(100.0, 60.0, emColor::BLACK);
        assert!(cr.x > 0.0, "child x={} should be positive", cr.x);
        assert!(cr.y > 0.0, "child y={} should be positive", cr.y);
        assert!(
            cr.x + cr.w < 100.0,
            "child right edge should be inside panel"
        );
        assert!(
            cr.y + cr.h < 60.0,
            "child bottom edge should be inside panel"
        );
        assert!(cr.w > 0.0, "child should have positive width");
        assert!(cr.h > 0.0, "child should have positive height");
        // canvas_color should be populated from the look.
        let _ = cr.canvas_color;
    }
}
