use crate::emCore::emColor::emColor;
use crate::emCore::emImage::emImage;

use super::emPainter::{TextAlignment, VAlign};
use crate::emCore::emStroke::emStroke;
use super::emTexture::{ImageExtension, ImageQuality, emTexture};

/// A recorded drawing operation for parallel tile rendering.
///
/// During the recording phase, a single-threaded tree walk captures all
/// draw operations into a `DrawList`. During the replay phase, multiple
/// threads independently replay the list into their own tile buffers.
pub(crate) enum DrawOp {
    // State management
    PushState,
    PopState,
    SetOffset(f64, f64),
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
        radius: f64,
        color: emColor,
    },
    PaintRectOutlined {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        stroke: emStroke,
        canvas_color: emColor,
    },
    PaintRoundRectOutlined {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
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
        stroke_color: emColor,
        thickness: f64,
        canvas_color: emColor,
    },
    PaintPolylineWithoutArrows {
        vertices: Vec<(f64, f64)>,
        stroke: emStroke,
        closed: bool,
        canvas_color: emColor,
    },
    PaintPolylineWithArrows {
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

/// A list of recorded drawing operations for a frame.
pub(crate) struct DrawList {
    ops: Vec<DrawOp>,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            ops: Vec::with_capacity(256),
        }
    }

    pub fn ops_mut(&mut self) -> &mut Vec<DrawOp> {
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

        for op in &self.ops {
            match op {
                DrawOp::PushState => painter.push_state(),
                DrawOp::PopState => painter.pop_state(),
                DrawOp::SetOffset(x, y) => {
                    painter.set_offset(x - tile_offset.0, y - tile_offset.1);
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
                    radius,
                    color,
                } => painter.PaintRoundRect(*x, *y, *w, *h, *radius, *color),

                DrawOp::PaintRectOutlined {
                    x,
                    y,
                    w,
                    h,
                    stroke,
                    canvas_color,
                } => painter.PaintRectOutline(*x, *y, *w, *h, stroke, *canvas_color),

                DrawOp::PaintRoundRectOutlined {
                    x,
                    y,
                    w,
                    h,
                    radius,
                    stroke,
                } => painter.PaintRoundRectOutline(*x, *y, *w, *h, *radius, stroke),

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
                    stroke_color,
                    thickness,
                    canvas_color,
                } => painter.PaintPolyline(vertices, *stroke_color, *thickness, *canvas_color),

                DrawOp::PaintPolylineWithoutArrows {
                    vertices,
                    stroke,
                    closed,
                    canvas_color,
                } => painter.PaintPolylineWithoutArrows(vertices, stroke, *closed, *canvas_color),

                DrawOp::PaintPolylineWithArrows {
                    vertices,
                    stroke,
                    closed,
                    canvas_color,
                } => painter.PaintPolylineWithArrows(vertices, stroke, *closed, *canvas_color),

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
