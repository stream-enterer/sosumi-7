use crate::emColor::emColor;
use crate::emImage::emImage;

use super::emPainter::{TextAlignment, VAlign};
use crate::emStroke::emStroke;
use super::emTexture::{ImageExtension, ImageQuality, emTexture};

/// A recorded drawing operation for parallel tile rendering.
///
/// During the recording phase, a single-threaded tree walk captures all
/// draw operations into a `DrawList`. During the replay phase, multiple
/// threads independently replay the list into their own tile buffers.
#[derive(Debug)]
pub enum DrawOp {
    // State management
    PushState,
    PopState,
    SetOffset(f64, f64),
    SetScaling(f64, f64),
    SetTransformation { ox: f64, oy: f64, sx: f64, sy: f64 },
    ClipRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
    SetCanvasColor(emColor),
    SetAlpha(u8),

    // Shapes
    PaintRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: emColor,
        canvas_color: emColor,
    },
    PaintRoundRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rx: f64,
        ry: f64,
        color: emColor,
        canvas_color: emColor,
    },
    PaintRectOutline {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stroke: emStroke,
        canvas_color: emColor,
    },
    PaintRoundRectOutline {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rx: f64,
        ry: f64,
        stroke: emStroke,
    },
    PaintEllipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        color: emColor,
        canvas_color: emColor,
    },
    PaintPolygon {
        vertices: Vec<(f64, f64)>,
        color: emColor,
        canvas_color: emColor,
    },
    PaintSolidPolyline {
        vertices: Vec<(f64, f64)>,
        stroke: emStroke,
        closed: bool,
        canvas_color: emColor,
    },

    // Images — raw pointers to images owned by panel behaviors.
    PaintImageFull {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image_ptr: *const emImage,
        alpha: u8,
        canvas_color: emColor,
    },
    PaintImageColored {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image_ptr: *const emImage,
        src_x: u32,
        src_y: u32,
        src_w: u32,
        src_h: u32,
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        extension: ImageExtension,
    },
    PaintImageScaled {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        image_ptr: *const emImage,
        quality: ImageQuality,
        extension: ImageExtension,
    },
    PaintImageTextured {
        rect_x: f64,
        rect_y: f64,
        rect_w: f64,
        rect_h: f64,
        tex_x: f64,
        tex_y: f64,
        tex_w: f64,
        tex_h: f64,
        image_ptr: *const emImage,
        src_x: i32,
        src_y: i32,
        src_w: i32,
        src_h: i32,
        alpha: u8,
        extension: ImageExtension,
    },
    PaintImageColoredTextured {
        rect_x: f64,
        rect_y: f64,
        rect_w: f64,
        rect_h: f64,
        tex_x: f64,
        tex_y: f64,
        tex_w: f64,
        tex_h: f64,
        image_ptr: *const emImage,
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        extension: ImageExtension,
    },
    PaintBorderImage {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        image_ptr: *const emImage,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        alpha: u8,
        canvas_color: emColor,
        which_sub_rects: u16,
    },

    // Text
    PaintText {
        x: f64,
        y: f64,
        text: String,
        char_height: f64,
        width_scale: f64,
        color: emColor,
        canvas_color: emColor,
    },
    PaintTextBoxed {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        text: String,
        max_char_height: f64,
        color: emColor,
        canvas_color: emColor,
        box_h_align: TextAlignment,
        box_v_align: VAlign,
        text_alignment: TextAlignment,
        min_width_scale: f64,
        formatted: bool,
        rel_line_space: f64,
    },

    // Ellipse sector / arc / outlines
    PaintEllipseSector {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        sweep_angle: f64,
        color: emColor,
        canvas_color: emColor,
    },
    PaintEllipseArc {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        range_angle: f64,
        stroke: emStroke,
        canvas_color: emColor,
    },
    PaintEllipseSectorOutline {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        start_angle: f64,
        sweep_angle: f64,
        stroke: emStroke,
        canvas_color: emColor,
    },
    PaintEllipseOutline {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        stroke: emStroke,
        canvas_color: emColor,
    },

    // Gradients
    PaintLinearGradient {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color_a: emColor,
        color_b: emColor,
        horizontal: bool,
        canvas_color: emColor,
    },
    PaintRadialGradient {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        color_inner: emColor,
        color_outer: emColor,
        canvas_color: emColor,
    },

    // Lines
    PaintLine {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        color: emColor,
        canvas_color: emColor,
    },
    PaintLineStroked {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        stroke: emStroke,
        canvas_color: emColor,
    },

    // Polygons
    PaintPolygonEvenOdd {
        vertices: Vec<(f64, f64)>,
        color: emColor,
        canvas_color: emColor,
    },
    PaintPolygonTextured {
        vertices: Vec<(f64, f64)>,
        texture: emTexture,
        canvas_color: emColor,
    },
    PaintPolygonTexturedEvenOdd {
        vertices: Vec<(f64, f64)>,
        texture: emTexture,
        canvas_color: emColor,
    },
    PaintPolygonOutline {
        vertices: Vec<(f64, f64)>,
        stroke_color: emColor,
        thickness: f64,
        canvas_color: emColor,
    },

    // Polylines
    PaintPolyline {
        vertices: Vec<(f64, f64)>,
        stroke: emStroke,
        closed: bool,
        canvas_color: emColor,
    },
    PaintDashedPolyline {
        vertices: Vec<(f64, f64)>,
        stroke: emStroke,
        closed: bool,
        canvas_color: emColor,
    },

    // Bezier
    PaintBezier {
        points: Vec<(f64, f64)>,
        color: emColor,
        canvas_color: emColor,
    },
    PaintBezierOutline {
        points: Vec<(f64, f64)>,
        stroke: emStroke,
        canvas_color: emColor,
    },
    PaintBezierLine {
        points: Vec<(f64, f64)>,
        stroke: emStroke,
        canvas_color: emColor,
    },

    // Images
    PaintImageSimple {
        x: f64,
        y: f64,
        image_ptr: *const emImage,
    },
    PaintBorderImageColored {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        l: f64,
        t: f64,
        r: f64,
        b: f64,
        image_ptr: *const emImage,
        src_l: i32,
        src_t: i32,
        src_r: i32,
        src_b: i32,
        color1: emColor,
        color2: emColor,
        canvas_color: emColor,
        which_sub_rects: u16,
        alpha: u8,
    },

    // Edge correction
    PaintEdgeCorrection {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color1: emColor,
        color2: emColor,
    },
}

// SAFETY: DrawOp contains *const emImage raw pointers to images owned by
// panel behaviors in the PanelTree. These pointers remain valid during
// the parallel replay phase because:
// 1. Images are owned by behaviors stored in PanelTree
// 2. The tree is not modified between recording and replay
// 3. std::thread::scope ensures all replay threads complete before returning
// All other fields are owned values (f64, emColor, String, Vec, emStroke).
unsafe impl Send for DrawOp {}
unsafe impl Sync for DrawOp {}

/// Painter state snapshot captured alongside each recorded paint op.
/// Matches C++ inline state fields: state_sx, state_sy, state_ox, state_oy,
/// state_clip_x1, state_clip_y1, state_clip_x2, state_clip_y2.
#[derive(Debug, Clone, Copy)]
pub struct RecordedState {
    pub scale_x: f64,
    pub scale_y: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub clip_x1: f64,
    pub clip_y1: f64,
    pub clip_x2: f64,
    pub clip_y2: f64,
    pub alpha: u8,
}

// SAFETY: RecordedState is f64 + u8 — trivially Send/Sync.
// Manual impls needed because RecordedOp has manual Send/Sync impls.
unsafe impl Send for RecordedState {}
unsafe impl Sync for RecordedState {}

/// A recorded op paired with the painter's nesting depth at recording time.
/// Matches C++'s `g_draw_op_depth` field in the JSONL output.
#[derive(Debug)]
pub struct RecordedOp {
    pub depth: u32,
    pub op: DrawOp,
    pub state: RecordedState,
}

// SAFETY: same reasoning as DrawOp — RecordedOp just wraps DrawOp + u32 + RecordedState.
unsafe impl Send for RecordedOp {}
unsafe impl Sync for RecordedOp {}

/// A list of recorded drawing operations for a frame.
pub(crate) struct DrawList {
    ops: Vec<RecordedOp>,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            ops: Vec::with_capacity(256),
        }
    }

    pub fn ops_mut(&mut self) -> &mut Vec<RecordedOp> {
        &mut self.ops
    }


    /// Replay all recorded operations into the given painter.
    ///
    /// `tile_offset` is subtracted from all absolute `SetOffset` calls to
    /// convert viewport coordinates to tile-local coordinates.
    pub fn replay(&self, painter: &mut super::emPainter::emPainter, tile_offset: (f64, f64)) {
        // Initialize the painter's offset to account for the tile position.
        // Draw operations recorded at viewport coordinates are shifted so that
        // viewport pixels in the tile's region map to tile-local coordinates.
        painter.set_offset(-tile_offset.0, -tile_offset.1);

        for recorded in &self.ops {
            match &recorded.op {
                DrawOp::PushState => painter.push_state(),
                DrawOp::PopState => painter.pop_state(),
                DrawOp::SetOffset(x, y) => {
                    painter.set_offset(x - tile_offset.0, y - tile_offset.1);
                }
                DrawOp::SetScaling(sx, sy) => {
                    painter.SetScaling(*sx, *sy);
                }
                DrawOp::SetTransformation { ox, oy, sx, sy } => {
                    painter.SetTransformation(
                        ox - tile_offset.0,
                        oy - tile_offset.1,
                        *sx,
                        *sy,
                    );
                }
                DrawOp::ClipRect { x, y, w, h } => painter.SetClipping(*x, *y, *w, *h),
                DrawOp::SetCanvasColor(c) => painter.SetCanvasColor(*c),
                DrawOp::SetAlpha(a) => painter.SetAlpha(*a),

                DrawOp::PaintRect {
                    x,
                    y,
                    w,
                    h,
                    color,
                    canvas_color,
                } => painter.PaintRect(*x, *y, *w, *h, *color, *canvas_color),

                DrawOp::PaintRoundRect {
                    x,
                    y,
                    w,
                    h,
                    rx,
                    ry,
                    color,
                    canvas_color,
                } => painter.PaintRoundRect(*x, *y, *w, *h, *rx, *ry, *color, *canvas_color),

                DrawOp::PaintRectOutline {
                    x,
                    y,
                    w,
                    h,
                    stroke,
                    canvas_color,
                } => painter.PaintRectOutline(*x, *y, *w, *h, stroke, *canvas_color),

                DrawOp::PaintRoundRectOutline {
                    x,
                    y,
                    w,
                    h,
                    rx,
                    ry,
                    stroke,
                } => painter.PaintRoundRectOutline(*x, *y, *w, *h, *rx, *ry, stroke),

                DrawOp::PaintEllipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    color,
                    canvas_color,
                } => painter.PaintEllipse(*cx, *cy, *rx, *ry, *color, *canvas_color),

                DrawOp::PaintPolygon {
                    vertices,
                    color,
                    canvas_color,
                } => painter.PaintPolygon(vertices, *color, *canvas_color),

                DrawOp::PaintSolidPolyline {
                    vertices,
                    stroke,
                    closed,
                    canvas_color,
                } => painter.PaintSolidPolyline(vertices, stroke, *closed, *canvas_color),

                DrawOp::PaintImageFull {
                    x,
                    y,
                    w,
                    h,
                    image_ptr,
                    alpha,
                    canvas_color,
                } => {
                    // SAFETY: see DrawOp Send/Sync impl
                    let image = unsafe { &**image_ptr };
                    painter.paint_image_full(*x, *y, *w, *h, image, *alpha, *canvas_color);
                }

                DrawOp::PaintImageColored {
                    x,
                    y,
                    w,
                    h,
                    image_ptr,
                    src_x,
                    src_y,
                    src_w,
                    src_h,
                    color1,
                    color2,
                    canvas_color,
                    extension,
                } => {
                    let image = unsafe { &**image_ptr };
                    painter.PaintImageColored(
                        *x,
                        *y,
                        *w,
                        *h,
                        image,
                        *src_x,
                        *src_y,
                        *src_w,
                        *src_h,
                        *color1,
                        *color2,
                        *canvas_color,
                        *extension,
                    );
                }

                DrawOp::PaintImageScaled {
                    x,
                    y,
                    w,
                    h,
                    image_ptr,
                    quality,
                    extension,
                } => {
                    let image = unsafe { &**image_ptr };
                    painter.paint_image_scaled(*x, *y, *w, *h, image, *quality, *extension);
                }

                DrawOp::PaintImageTextured {
                    rect_x, rect_y, rect_w, rect_h,
                    tex_x, tex_y, tex_w, tex_h,
                    image_ptr,
                    src_x, src_y, src_w, src_h,
                    alpha,
                    extension,
                } => {
                    let image = unsafe { &**image_ptr };
                    painter.PaintImageTextured(
                        *rect_x, *rect_y, *rect_w, *rect_h,
                        *tex_x, *tex_y, *tex_w, *tex_h,
                        image, *src_x, *src_y, *src_w, *src_h,
                        *alpha, *extension,
                    );
                }

                DrawOp::PaintImageColoredTextured {
                    rect_x, rect_y, rect_w, rect_h,
                    tex_x, tex_y, tex_w, tex_h,
                    image_ptr,
                    color1, color2,
                    canvas_color,
                    extension,
                } => {
                    let image = unsafe { &**image_ptr };
                    painter.PaintImageColoredTextured(
                        *rect_x, *rect_y, *rect_w, *rect_h,
                        *tex_x, *tex_y, *tex_w, *tex_h,
                        image, *color1, *color2, *canvas_color, *extension,
                    );
                }

                DrawOp::PaintBorderImage {
                    x,
                    y,
                    w,
                    h,
                    l,
                    t,
                    r,
                    b,
                    image_ptr,
                    src_l,
                    src_t,
                    src_r,
                    src_b,
                    alpha,
                    canvas_color,
                    which_sub_rects,
                } => {
                    let image = unsafe { &**image_ptr };
                    painter.PaintBorderImage(
                        *x,
                        *y,
                        *w,
                        *h,
                        *l,
                        *t,
                        *r,
                        *b,
                        image,
                        *src_l,
                        *src_t,
                        *src_r,
                        *src_b,
                        *alpha,
                        *canvas_color,
                        *which_sub_rects,
                    );
                }

                DrawOp::PaintText {
                    x,
                    y,
                    text,
                    char_height,
                    width_scale,
                    color,
                    canvas_color,
                } => painter.PaintText(
                    *x,
                    *y,
                    text,
                    *char_height,
                    *width_scale,
                    *color,
                    *canvas_color,
                ),

                DrawOp::PaintTextBoxed {
                    x,
                    y,
                    w,
                    h,
                    text,
                    max_char_height,
                    color,
                    canvas_color,
                    box_h_align,
                    box_v_align,
                    text_alignment,
                    min_width_scale,
                    formatted,
                    rel_line_space,
                } => painter.PaintTextBoxed(
                    *x,
                    *y,
                    *w,
                    *h,
                    text,
                    *max_char_height,
                    *color,
                    *canvas_color,
                    *box_h_align,
                    *box_v_align,
                    *text_alignment,
                    *min_width_scale,
                    *formatted,
                    *rel_line_space,
                ),

                DrawOp::PaintEllipseSector {
                    cx,
                    cy,
                    rx,
                    ry,
                    start_angle,
                    sweep_angle,
                    color,
                    canvas_color,
                } => painter.PaintEllipseSector(
                    *cx,
                    *cy,
                    *rx,
                    *ry,
                    *start_angle,
                    *sweep_angle,
                    *color,
                    *canvas_color,
                ),

                DrawOp::PaintEllipseArc {
                    cx,
                    cy,
                    rx,
                    ry,
                    start_angle,
                    range_angle,
                    stroke,
                    canvas_color,
                } => painter.PaintEllipseArc(
                    *cx, *cy, *rx, *ry, *start_angle, *range_angle, stroke, *canvas_color,
                ),

                DrawOp::PaintEllipseSectorOutline {
                    cx,
                    cy,
                    rx,
                    ry,
                    start_angle,
                    sweep_angle,
                    stroke,
                    canvas_color,
                } => painter.PaintEllipseSectorOutline(
                    *cx,
                    *cy,
                    *rx,
                    *ry,
                    *start_angle,
                    *sweep_angle,
                    stroke,
                    *canvas_color,
                ),

                DrawOp::PaintEllipseOutline {
                    cx,
                    cy,
                    rx,
                    ry,
                    stroke,
                    canvas_color,
                } => painter.PaintEllipseOutline(*cx, *cy, *rx, *ry, stroke, *canvas_color),

                DrawOp::PaintLinearGradient {
                    x,
                    y,
                    w,
                    h,
                    color_a,
                    color_b,
                    horizontal,
                    canvas_color,
                } => painter.paint_linear_gradient(
                    *x,
                    *y,
                    *w,
                    *h,
                    *color_a,
                    *color_b,
                    *horizontal,
                    *canvas_color,
                ),

                DrawOp::PaintRadialGradient {
                    cx,
                    cy,
                    rx,
                    ry,
                    color_inner,
                    color_outer,
                    canvas_color,
                } => painter.paint_radial_gradient(
                    *cx,
                    *cy,
                    *rx,
                    *ry,
                    *color_inner,
                    *color_outer,
                    *canvas_color,
                ),

                DrawOp::PaintLine {
                    x0,
                    y0,
                    x1,
                    y1,
                    color,
                    canvas_color,
                } => painter.PaintLine(*x0, *y0, *x1, *y1, *color, *canvas_color),

                DrawOp::PaintLineStroked {
                    x0,
                    y0,
                    x1,
                    y1,
                    stroke,
                    canvas_color,
                } => painter.paint_line_stroked(*x0, *y0, *x1, *y1, stroke, *canvas_color),

                DrawOp::PaintPolygonEvenOdd {
                    vertices,
                    color,
                    canvas_color,
                } => painter.paint_polygon_even_odd(vertices, *color, *canvas_color),

                DrawOp::PaintPolygonTextured {
                    vertices,
                    texture,
                    canvas_color,
                } => painter.paint_polygon_textured(vertices, texture, *canvas_color),

                DrawOp::PaintPolygonTexturedEvenOdd {
                    vertices,
                    texture,
                    canvas_color,
                } => painter.paint_polygon_textured_even_odd(vertices, texture, *canvas_color),

                DrawOp::PaintPolygonOutline {
                    vertices,
                    stroke_color,
                    thickness,
                    canvas_color,
                } => painter.PaintPolygonOutline(vertices, *stroke_color, *thickness, *canvas_color),

                DrawOp::PaintPolyline {
                    vertices,
                    stroke,
                    closed,
                    canvas_color,
                } => painter.PaintPolyline(vertices, stroke, *closed, *canvas_color),

                DrawOp::PaintDashedPolyline {
                    vertices,
                    stroke,
                    closed,
                    canvas_color,
                } => painter.PaintDashedPolyline(vertices, stroke, *closed, *canvas_color),

                DrawOp::PaintBezier {
                    points,
                    color,
                    canvas_color,
                } => painter.PaintBezier(points, *color, *canvas_color),

                DrawOp::PaintBezierOutline {
                    points,
                    stroke,
                    canvas_color,
                } => painter.PaintBezierOutline(points, stroke, *canvas_color),

                DrawOp::PaintBezierLine {
                    points,
                    stroke,
                    canvas_color,
                } => painter.PaintBezierLine(points, stroke, *canvas_color),

                DrawOp::PaintImageSimple { x, y, image_ptr } => {
                    // SAFETY: see DrawOp Send/Sync impl
                    let image = unsafe { &**image_ptr };
                    painter.PaintImage(*x, *y, image);
                }

                DrawOp::PaintBorderImageColored {
                    x,
                    y,
                    w,
                    h,
                    l,
                    t,
                    r,
                    b,
                    image_ptr,
                    src_l,
                    src_t,
                    src_r,
                    src_b,
                    color1,
                    color2,
                    canvas_color,
                    which_sub_rects,
                    alpha,
                } => {
                    let image = unsafe { &**image_ptr };
                    painter.PaintBorderImageColored(
                        *x,
                        *y,
                        *w,
                        *h,
                        *l,
                        *t,
                        *r,
                        *b,
                        image,
                        *src_l,
                        *src_t,
                        *src_r,
                        *src_b,
                        *color1,
                        *color2,
                        *canvas_color,
                        *which_sub_rects,
                        *alpha,
                    );
                }

                DrawOp::PaintEdgeCorrection {
                    x1,
                    y1,
                    x2,
                    y2,
                    color1,
                    color2,
                } => painter.PaintEdgeCorrection(*x1, *y1, *x2, *y2, *color1, *color2),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emColor::emColor;
    use crate::emImage::emImage;
    use crate::emPainter::emPainter;

    fn images_equal(a: &[u8], b: &[u8]) -> bool {
        a == b
    }

    fn pixel_at(map: &[u8], x: usize, y: usize, width: usize) -> [u8; 4] {
        let off = (y * width + x) * 4;
        [map[off], map[off + 1], map[off + 2], map[off + 3]]
    }

    const WHITE: [u8; 4] = [255, 255, 255, 255];

    #[test]
    fn replay_rect_matches_direct() {
        // Direct paint
        let mut img_a = emImage::new(64, 64, 4);
        img_a.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_a);
            p.SetCanvasColor(emColor::WHITE);
            p.PaintRect(10.0, 10.0, 40.0, 30.0, emColor::RED, emColor::WHITE);
        }

        // Record + replay
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 64, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.PaintRect(10.0, 10.0, 40.0, 30.0, emColor::RED, emColor::WHITE);
            *draw_list.ops_mut() = ops;
        }
        let mut img_b = emImage::new(64, 64, 4);
        img_b.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_b);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        assert!(
            images_equal(img_a.GetMap(), img_b.GetMap()),
            "replay of PaintRect must match direct paint"
        );
    }

    #[test]
    fn replay_ellipse_matches_direct() {
        // Direct paint
        let mut img_a = emImage::new(64, 64, 4);
        img_a.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_a);
            p.SetCanvasColor(emColor::WHITE);
            p.PaintEllipse(32.0, 32.0, 20.0, 15.0, emColor::RED, emColor::WHITE);
        }

        // Record + replay
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 64, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.PaintEllipse(32.0, 32.0, 20.0, 15.0, emColor::RED, emColor::WHITE);
            *draw_list.ops_mut() = ops;
        }
        let mut img_b = emImage::new(64, 64, 4);
        img_b.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_b);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        assert!(
            images_equal(img_a.GetMap(), img_b.GetMap()),
            "replay of PaintEllipse must match direct paint"
        );
    }

    #[test]
    fn replay_state_push_pop() {
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 32, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.push_state();
            p.SetCanvasColor(emColor::RED);
            p.PaintRect(0.0, 0.0, 32.0, 32.0, emColor::BLUE, emColor::RED);
            p.pop_state();
            p.PaintRect(32.0, 0.0, 32.0, 32.0, emColor::GREEN, emColor::WHITE);
            *draw_list.ops_mut() = ops;
        }

        let mut img = emImage::new(64, 32, 4);
        img.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        let map = img.GetMap();
        let px_left = pixel_at(map, 16, 16, 64);
        assert_ne!(px_left, WHITE, "blue rect area should not be white");

        let px_right = pixel_at(map, 48, 16, 64);
        assert_ne!(px_right, WHITE, "green rect area should not be white");
    }

    #[test]
    fn replay_clip_rect() {
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 64, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.SetClipping(10.0, 10.0, 20.0, 20.0);
            p.PaintRect(0.0, 0.0, 64.0, 64.0, emColor::RED, emColor::WHITE);
            *draw_list.ops_mut() = ops;
        }

        let mut img = emImage::new(64, 64, 4);
        img.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        let map = img.GetMap();
        let inside = pixel_at(map, 15, 15, 64);
        assert_ne!(inside, WHITE, "pixel inside clip region should not be white");

        let outside = pixel_at(map, 5, 5, 64);
        assert_eq!(outside, WHITE, "pixel outside clip region should be white");
    }

    #[test]
    fn replay_offset_translation() {
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 64, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.set_offset(10.0, 10.0);
            p.PaintRect(0.0, 0.0, 20.0, 20.0, emColor::RED, emColor::WHITE);
            *draw_list.ops_mut() = ops;
        }

        // Replay with tile_offset=(0,0)
        let mut img1 = emImage::new(64, 64, 4);
        img1.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img1);
            draw_list.replay(&mut p, (0.0, 0.0));
        }
        let map1 = img1.GetMap();
        let shifted = pixel_at(map1, 15, 15, 64);
        assert_ne!(shifted, WHITE, "pixel at (15,15) should be painted (rect shifted to 10,10)");
        let before = pixel_at(map1, 5, 5, 64);
        assert_eq!(before, WHITE, "pixel at (5,5) should be white (before offset)");

        // Replay with tile_offset=(5,5)
        let mut img2 = emImage::new(64, 64, 4);
        img2.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img2);
            draw_list.replay(&mut p, (5.0, 5.0));
        }
        let map2 = img2.GetMap();
        let tile_shifted = pixel_at(map2, 10, 10, 64);
        assert_ne!(
            tile_shifted, WHITE,
            "pixel at (10,10) should be painted when tile_offset=(5,5)"
        );
    }

    #[test]
    fn replay_gradient_matches_direct() {
        // Direct paint
        let mut img_a = emImage::new(64, 64, 4);
        img_a.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_a);
            p.SetCanvasColor(emColor::WHITE);
            p.paint_linear_gradient(
                0.0,
                0.0,
                64.0,
                32.0,
                emColor::RED,
                emColor::BLUE,
                true,
                emColor::WHITE,
            );
        }

        // Record + replay
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 64, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.paint_linear_gradient(
                0.0,
                0.0,
                64.0,
                32.0,
                emColor::RED,
                emColor::BLUE,
                true,
                emColor::WHITE,
            );
            *draw_list.ops_mut() = ops;
        }
        let mut img_b = emImage::new(64, 64, 4);
        img_b.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_b);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        assert!(
            images_equal(img_a.GetMap(), img_b.GetMap()),
            "replay of paint_linear_gradient must match direct paint"
        );
    }

    #[test]
    fn replay_polygon_matches_direct() {
        let triangle = [(10.0, 10.0), (50.0, 10.0), (30.0, 50.0)];

        // Direct paint
        let mut img_a = emImage::new(64, 64, 4);
        img_a.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_a);
            p.SetCanvasColor(emColor::WHITE);
            p.PaintPolygon(&triangle, emColor::GREEN, emColor::WHITE);
        }

        // Record + replay
        let mut draw_list = DrawList::new();
        {
            let mut ops = Vec::new();
            let mut p = emPainter::new_recording(64, 64, &mut ops);
            p.SetCanvasColor(emColor::WHITE);
            p.PaintPolygon(&triangle, emColor::GREEN, emColor::WHITE);
            *draw_list.ops_mut() = ops;
        }
        let mut img_b = emImage::new(64, 64, 4);
        img_b.fill(emColor::WHITE);
        {
            let mut p = emPainter::new(&mut img_b);
            draw_list.replay(&mut p, (0.0, 0.0));
        }

        assert!(
            images_equal(img_a.GetMap(), img_b.GetMap()),
            "replay of PaintPolygon must match direct paint"
        );
    }
}
