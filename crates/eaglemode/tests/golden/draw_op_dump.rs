use emcore::emColor::emColor;
use emcore::emImage::emImage;
use emcore::emPainter::emPainter;
use emcore::emPainterDrawList::{DrawOp, RecordedOp, RecordedState};
use emcore::emTexture::emTexture;
use std::io::Write;
use std::path::PathBuf;

/// Returns true if DUMP_DRAW_OPS=1 is set in the environment.
pub fn dump_draw_ops_enabled() -> bool {
    std::env::var("DUMP_DRAW_OPS").as_deref() == Ok("1")
}

/// Install an op-logging callback on a **direct-mode** painter.
///
/// This logs draw ops from inside the actual rendering path (not a separate
/// recording pass), eliminating double-pass recording noise. The callback
/// serializes each op to JSONL and writes it to the output file. State ops
/// (PushState, PopState, SetOffset, etc.) are filtered out to match the
/// existing JSONL format.
///
/// The file is flushed when the painter is dropped or `clear_op_log()` is called.
pub fn install_direct_op_logger(painter: &mut emPainter, name: &str) {
    let path = output_path(name);
    let _ = std::fs::create_dir_all(path.parent().expect("path has parent"));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open rust_ops.jsonl for direct logging");
    let mut seq = 0usize;
    painter.set_op_log(move |op, depth, state| {
        if is_state_op(op) {
            return;
        }
        let line = serialize_op(seq, depth, op, &state);
        writeln!(file, "{line}").expect("write op line");
        seq += 1;
    });
}

fn output_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("golden-divergence")
        .join(format!("{name}.rust_ops.jsonl"))
}

fn color_hex(c: emcore::emColor::emColor) -> String {
    format!("{:08x}", c.GetPacked())
}

fn hex_f64(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn hex_fields(pairs: &[(&str, f64)]) -> String {
    pairs
        .iter()
        .map(|(name, val)| format!(r#""{name}_hex":"{}""#, hex_f64(*val)))
        .collect::<Vec<_>>()
        .join(",")
}

fn vertices_json(verts: &[(f64, f64)]) -> String {
    let parts: Vec<String> = verts.iter().map(|(x, y)| format!("[{x},{y}]")).collect();
    format!("[{}]", parts.join(","))
}

/// Extract the base color from a texture, matching C++ `texture.GetColor()`.
/// C++ GetColor() returns Color1, which is set for color/gradient/image-colored textures
/// but uninitialized (garbage) for plain image textures.
fn texture_color(tex: &emTexture) -> emColor {
    match tex {
        emTexture::SolidColor(c) => *c,
        emTexture::LinearGradient { color_a, .. } => *color_a,
        emTexture::RadialGradient { color_inner, .. } => *color_inner,
        emTexture::ImageColored { color, .. } => *color,
        emTexture::ImageColoredGradient { color1, .. } => *color1,
        // IMAGE type: Color1 is uninitialized in C++
        emTexture::emImage { .. } => emColor::TRANSPARENT,
    }
}

/// Serializes `ops` to JSONL at `target/golden-divergence/{name}.rust_ops.jsonl`.
/// Kept for backward compatibility with recording-painter workflows.
#[allow(dead_code)]
pub fn dump_draw_ops(name: &str, ops: &[RecordedOp]) {
    let path = output_path(name);
    let _ = std::fs::create_dir_all(path.parent().expect("path has parent"));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open rust_ops.jsonl");

    let mut seq = 0;
    for recorded in ops.iter() {
        if is_state_op(&recorded.op) {
            continue;
        }
        let line = serialize_op(seq, recorded.depth, &recorded.op, &recorded.state);
        writeln!(f, "{line}").expect("write line");
        seq += 1;
    }
}

fn is_state_op(op: &DrawOp) -> bool {
    matches!(
        op,
        DrawOp::PushState
            | DrawOp::PopState
            | DrawOp::SetOffset(..)
            | DrawOp::SetScaling(..)
            | DrawOp::SetTransformation { .. }
            | DrawOp::ClipRect { .. }
            | DrawOp::SetCanvasColor(..)
            | DrawOp::SetAlpha(..)
    )
}

fn state_fields(state: &RecordedState) -> String {
    format!(
        r#""state_sx":{},"state_sy":{},"state_ox":{},"state_oy":{},"state_clip_x1":{},"state_clip_y1":{},"state_clip_x2":{},"state_clip_y2":{},"state_alpha":{}"#,
        state.scale_x,
        state.scale_y,
        state.offset_x,
        state.offset_y,
        state.clip_x1,
        state.clip_y1,
        state.clip_x2,
        state.clip_y2,
        state.alpha,
    )
}

fn serialize_op(seq: usize, depth: u32, op: &DrawOp, state: &RecordedState) -> String {
    let sf = state_fields(state);
    match op {
        DrawOp::PaintRect {
            x,
            y,
            w,
            h,
            color,
            canvas_color,
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintRoundRect {
            x,
            y,
            w,
            h,
            rx,
            ry,
            color,
            canvas_color,
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("rx", *rx),
                ("ry", *ry),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRoundRect","x":{x},"y":{y},"w":{w},"h":{h},"rx":{rx},"ry":{ry},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintEllipse {
            x,
            y,
            w,
            h,
            color,
            canvas_color,
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipse","x":{x},"y":{y},"w":{w},"h":{h},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintPolygon {
            vertices,
            color,
            canvas_color,
        } => {
            let verts = vertices_json(vertices);
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintPolygon","vertices":{verts},"color":"{color}","canvas_color":"{canvas_color}",{sf}}}"#
            )
        }
        DrawOp::PaintPolyline {
            vertices,
            stroke,
            closed: _,
            canvas_color,
        } => {
            let n = vertices.len();
            let thickness = stroke.width;
            let color = color_hex(stroke.color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("thickness", thickness)]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintPolyline","n":{n},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintSolidPolyline {
            vertices,
            stroke,
            closed,
            canvas_color,
        } => {
            let verts = vertices_json(vertices);
            let stroke_color = color_hex(stroke.color);
            let stroke_width = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintSolidPolyline","vertices":{verts},"stroke_color":"{stroke_color}","stroke_width":{stroke_width},"closed":{closed},"canvas_color":"{canvas_color}",{sf}}}"#
            )
        }

        DrawOp::PaintImageFull {
            x,
            y,
            w,
            h,
            alpha: _,
            canvas_color,
            ..
        } => {
            // C++ PaintImage dispatches to PaintRect(emImageTexture) internally.
            // C++ logs texture.GetColor() which returns uninitialized Color1 (garbage).
            // We output color=00000000 as a placeholder — the field is meaningless for IMAGE textures.
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"00000000","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintImageColored {
            x,
            y,
            w,
            h,
            color1,
            canvas_color,
            ..
        } => {
            // C++ PaintImageColored dispatches to PaintRect(emImageColoredTexture).
            // C++ logs texture.GetColor() which returns Color1. Match C++ PaintRect format.
            let color = color_hex(*color1);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
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
            // SAFETY: image_ptr is valid for the lifetime of the DrawOp list.
            let (img_w, img_h, img_ch) = unsafe {
                let img: &emImage = &**image_ptr;
                (img.GetWidth(), img.GetHeight(), img.GetChannelCount())
            };
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("l", *l),
                ("t", *t),
                ("r", *r),
                ("b", *b),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintBorderImage","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"alpha":{alpha},"canvas_color":"{canvas_color}","which_sub_rects":{which_sub_rects},{hf},{sf}}}"#
            )
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
            // SAFETY: image_ptr is valid for the lifetime of the DrawOp list.
            let (img_w, img_h, img_ch) = unsafe {
                let img: &emImage = &**image_ptr;
                (img.GetWidth(), img.GetHeight(), img.GetChannelCount())
            };
            let color1 = color_hex(*color1);
            let color2 = color_hex(*color2);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("l", *l),
                ("t", *t),
                ("r", *r),
                ("b", *b),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintBorderImageColored","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"color1":"{color1}","color2":"{color2}","canvas_color":"{canvas_color}","which_sub_rects":{which_sub_rects},"alpha":{alpha},{hf},{sf}}}"#
            )
        }

        DrawOp::PaintText {
            x,
            y,
            text,
            char_height,
            width_scale,
            color,
            canvas_color,
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let text = text
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("char_height", *char_height),
                ("width_scale", *width_scale),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintText","x":{x},"y":{y},"text":"{text}","char_height":{char_height},"width_scale":{width_scale},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
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
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let text = text
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            let box_h_align = format!("{box_h_align:?}");
            let box_v_align = format!("{box_v_align:?}");
            let text_alignment = format!("{text_alignment:?}");
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("max_char_height", *max_char_height),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintTextBoxed","x":{x},"y":{y},"w":{w},"h":{h},"text":"{text}","max_char_height":{max_char_height},"color":"{color}","canvas_color":"{canvas_color}","box_h_align":"{box_h_align}","box_v_align":"{box_v_align}","text_alignment":"{text_alignment}","min_width_scale":{min_width_scale},"formatted":{formatted},"rel_line_space":{rel_line_space},{hf},{sf}}}"#
            )
        }

        DrawOp::PaintLinearGradient {
            x,
            y,
            w,
            h,
            color_a,
            color_b,
            horizontal,
            canvas_color,
        } => {
            let color_a = color_hex(*color_a);
            let color_b = color_hex(*color_b);
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintLinearGradient","x":{x},"y":{y},"w":{w},"h":{h},"color_a":"{color_a}","color_b":"{color_b}","horizontal":{horizontal},"canvas_color":"{canvas_color}",{sf}}}"#
            )
        }
        DrawOp::PaintRadialGradient {
            cx,
            cy,
            rx,
            ry,
            color_inner,
            color_outer,
            canvas_color,
        } => {
            let color_inner = color_hex(*color_inner);
            let color_outer = color_hex(*color_outer);
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRadialGradient","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color_inner":"{color_inner}","color_outer":"{color_outer}","canvas_color":"{canvas_color}",{sf}}}"#
            )
        }

        DrawOp::PaintRectOutline {
            x,
            y,
            w,
            h,
            stroke,
            canvas_color,
        } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("thickness", thickness),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRectOutline","x":{x},"y":{y},"w":{w},"h":{h},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }

        DrawOp::PaintRoundRectOutline {
            x,
            y,
            w,
            h,
            rx,
            ry,
            stroke,
            canvas_color,
        } => {
            let color = color_hex(stroke.color);
            let canvas_color = color_hex(*canvas_color);
            let thickness = stroke.width;
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("rx", *rx),
                ("ry", *ry),
                ("thickness", thickness),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRoundRectOutline","x":{x},"y":{y},"w":{w},"h":{h},"rx":{rx},"ry":{ry},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }

        DrawOp::PaintEllipseOutline {
            x,
            y,
            w,
            h,
            stroke,
            canvas_color,
        } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("thickness", thickness),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipseOutline","x":{x},"y":{y},"w":{w},"h":{h},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }

        // C++ PaintLine takes stroke params; Rust splits into PaintLine (bare) + PaintLineStroked.
        // Serialize as "PaintLine" to match C++ recording.
        DrawOp::PaintLineStroked {
            x0,
            y0,
            x1,
            y1,
            stroke,
            canvas_color,
        } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x1", *x0),
                ("y1", *y0),
                ("x2", *x1),
                ("y2", *y1),
                ("thickness", thickness),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintLine","x1":{x0},"y1":{y0},"x2":{x1},"y2":{y1},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }

        DrawOp::PaintEllipseSector {
            x,
            y,
            w,
            h,
            start_angle,
            sweep_angle,
            color,
            canvas_color,
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipseSector","x":{x},"y":{y},"w":{w},"h":{h},"start_angle":{start_angle},"range_angle":{sweep_angle},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintEllipseSectorOutline {
            x,
            y,
            w,
            h,
            start_angle,
            sweep_angle,
            stroke,
            canvas_color,
        } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("thickness", thickness),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipseSectorOutline","x":{x},"y":{y},"w":{w},"h":{h},"start_angle":{start_angle},"range_angle":{sweep_angle},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintEllipseArc {
            x,
            y,
            w,
            h,
            start_angle,
            range_angle,
            stroke,
            canvas_color,
        } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *x),
                ("y", *y),
                ("w", *w),
                ("h", *h),
                ("thickness", thickness),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipseArc","x":{x},"y":{y},"w":{w},"h":{h},"start_angle":{start_angle},"range_angle":{range_angle},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }

        DrawOp::PaintPolygonOutline {
            vertices,
            stroke_color,
            thickness,
            canvas_color,
        } => {
            let _verts = vertices_json(vertices);
            let color = color_hex(*stroke_color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("thickness", *thickness)]);
            // C++ PaintPolyline doesn't log vertices — just n, thickness, color, canvas_color.
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintPolyline","n":{},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#,
                vertices.len()
            )
        }

        DrawOp::PaintPolygonTextured {
            vertices,
            texture,
            canvas_color,
        } => {
            let verts = vertices_json(vertices);
            let color = color_hex(texture_color(texture));
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintPolygon","vertices":{verts},"n":{},"color":"{color}","canvas_color":"{canvas_color}",{sf}}}"#,
                vertices.len()
            )
        }

        DrawOp::PaintBezier {
            points,
            color,
            canvas_color,
        } => {
            let n = points.len();
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintBezier","n":{n},"color":"{color}","canvas_color":"{canvas_color}",{sf}}}"#
            )
        }
        DrawOp::PaintBezierOutline {
            points,
            stroke,
            canvas_color,
        } => {
            let n = points.len();
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintBezierLine","n":{n},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{sf}}}"#
            )
        }
        DrawOp::PaintBezierLine {
            points,
            stroke,
            canvas_color,
        } => {
            let n = points.len();
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintBezierLine","n":{n},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{sf}}}"#
            )
        }

        DrawOp::PaintImageTextured {
            rect_x,
            rect_y,
            rect_w,
            rect_h,
            ..
        } => {
            // C++ PaintImage(srcRect) dispatches to PaintRect(emImageTexture).
            // texture.GetColor() returns uninitialized Color1 (garbage). Match C++ PaintRect format.
            let hf = hex_fields(&[
                ("x", *rect_x),
                ("y", *rect_y),
                ("w", *rect_w),
                ("h", *rect_h),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{rect_x},"y":{rect_y},"w":{rect_w},"h":{rect_h},"color":"00000000","canvas_color":"00000000",{hf},{sf}}}"#
            )
        }
        DrawOp::PaintImageColoredTextured {
            rect_x,
            rect_y,
            rect_w,
            rect_h,
            color1,
            canvas_color,
            ..
        } => {
            // C++ PaintImageColored dispatches to PaintRect(emImageColoredTexture).
            // texture.GetColor() returns Color1. Match C++ PaintRect format.
            let color = color_hex(*color1);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[
                ("x", *rect_x),
                ("y", *rect_y),
                ("w", *rect_w),
                ("h", *rect_h),
            ]);
            format!(
                r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{rect_x},"y":{rect_y},"w":{rect_w},"h":{rect_h},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#
            )
        }

        // Catch-all for variants not individually serialized above.
        other => {
            let variant = variant_name(other);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"{variant}","_unserialized":true,{sf}}}"#)
        }
    }
}

fn variant_name(op: &DrawOp) -> &'static str {
    match op {
        DrawOp::PushState => "PushState",
        DrawOp::PopState => "PopState",
        DrawOp::SetOffset(..) => "SetOffset",
        DrawOp::SetScaling(..) => "SetScaling",
        DrawOp::SetTransformation { .. } => "SetTransformation",
        DrawOp::ClipRect { .. } => "ClipRect",
        DrawOp::SetCanvasColor(..) => "SetCanvasColor",
        DrawOp::SetAlpha(..) => "SetAlpha",
        DrawOp::PaintRect { .. } => "PaintRect",
        DrawOp::PaintRoundRect { .. } => "PaintRoundRect",
        DrawOp::PaintRectOutline { .. } => "PaintRectOutline",
        DrawOp::PaintRoundRectOutline { .. } => "PaintRoundRectOutline",
        DrawOp::PaintEllipse { .. } => "PaintEllipse",
        DrawOp::PaintPolygon { .. } => "PaintPolygon",
        DrawOp::PaintSolidPolyline { .. } => "PaintSolidPolyline",
        DrawOp::PaintImageFull { .. } => "PaintRect",
        DrawOp::PaintImageColored { .. } => "PaintImageColored",
        DrawOp::PaintImageScaled { .. } => "PaintImageScaled",
        DrawOp::PaintImageTextured { .. } => "PaintImageTextured",
        DrawOp::PaintImageColoredTextured { .. } => "PaintImageColoredTextured",
        DrawOp::PaintBorderImage { .. } => "PaintBorderImage",
        DrawOp::PaintText { .. } => "PaintText",
        DrawOp::PaintTextBoxed { .. } => "PaintTextBoxed",
        DrawOp::PaintEllipseSector { .. } => "PaintEllipseSector",
        DrawOp::PaintEllipseArc { .. } => "PaintEllipseArc",
        DrawOp::PaintEllipseSectorOutline { .. } => "PaintEllipseSectorOutline",
        DrawOp::PaintEllipseOutline { .. } => "PaintEllipseOutline",
        DrawOp::PaintLinearGradient { .. } => "PaintLinearGradient",
        DrawOp::PaintRadialGradient { .. } => "PaintRadialGradient",
        DrawOp::PaintLine { .. } => "PaintLine",
        DrawOp::PaintLineStroked { .. } => "PaintLineStroked",
        DrawOp::PaintPolygonEvenOdd { .. } => "PaintPolygonEvenOdd",
        DrawOp::PaintPolygonTextured { .. } => "PaintPolygonTextured",
        DrawOp::PaintPolygonTexturedEvenOdd { .. } => "PaintPolygonTexturedEvenOdd",
        DrawOp::PaintPolygonOutline { .. } => "PaintPolygonOutline",
        DrawOp::PaintPolyline { .. } => "PaintPolyline",
        DrawOp::PaintDashedPolyline { .. } => "PaintDashedPolyline",
        DrawOp::PaintBezier { .. } => "PaintBezier",
        DrawOp::PaintBezierOutline { .. } => "PaintBezierOutline",
        DrawOp::PaintBezierLine { .. } => "PaintBezierLine",
        DrawOp::PaintImageSimple { .. } => "PaintImageSimple",
        DrawOp::PaintBorderImageColored { .. } => "PaintBorderImageColored",
        DrawOp::PaintEdgeCorrection { .. } => "PaintEdgeCorrection",
    }
}
