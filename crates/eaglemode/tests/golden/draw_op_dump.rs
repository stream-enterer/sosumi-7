use std::io::Write;
use std::path::PathBuf;
use emcore::emImage::emImage;
use emcore::emPainterDrawList::{DrawOp, RecordedOp, RecordedState};

/// Returns true if DUMP_DRAW_OPS=1 is set in the environment.
pub fn dump_draw_ops_enabled() -> bool {
    std::env::var("DUMP_DRAW_OPS").as_deref() == Ok("1")
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
    let parts: Vec<String> = verts
        .iter()
        .map(|(x, y)| format!("[{x},{y}]"))
        .collect();
    format!("[{}]", parts.join(","))
}

/// Serializes `ops` to JSONL at `target/golden-divergence/{name}.rust_ops.jsonl`.
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
        state.scale_x, state.scale_y,
        state.offset_x, state.offset_y,
        state.clip_x1, state.clip_y1,
        state.clip_x2, state.clip_y2,
        state.alpha,
    )
}

fn serialize_op(seq: usize, depth: u32, op: &DrawOp, state: &RecordedState) -> String {
    let sf = state_fields(state);
    match op {
        DrawOp::PaintRect { x, y, w, h, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }
        DrawOp::PaintRoundRect { x, y, w, h, rx, ry, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("rx", *rx), ("ry", *ry)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintRoundRect","x":{x},"y":{y},"w":{w},"h":{h},"rx":{rx},"ry":{ry},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }
        DrawOp::PaintEllipse { cx, cy, rx, ry, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("cx", *cx), ("cy", *cy), ("rx", *rx), ("ry", *ry)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipse","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }
        DrawOp::PaintPolygon { vertices, color, canvas_color } => {
            let verts = vertices_json(vertices);
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintPolygon","vertices":{verts},"color":"{color}","canvas_color":"{canvas_color}",{sf}}}"#)
        }
        DrawOp::PaintPolyline { vertices, stroke, closed: _, canvas_color } => {
            let n = vertices.len();
            let thickness = stroke.width;
            let color = color_hex(stroke.color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("thickness", thickness)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintPolyline","n":{n},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }
        DrawOp::PaintSolidPolyline { vertices, stroke, closed, canvas_color } => {
            let verts = vertices_json(vertices);
            let stroke_color = color_hex(stroke.color);
            let stroke_width = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintSolidPolyline","vertices":{verts},"stroke_color":"{stroke_color}","stroke_width":{stroke_width},"closed":{closed},"canvas_color":"{canvas_color}",{sf}}}"#)
        }

        DrawOp::PaintImageFull { x, y, w, h, canvas_color, .. } => {
            let color = "00000000";
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }
        DrawOp::PaintImageColored {
            x, y, w, h, image_ptr, src_x, src_y, src_w, src_h,
            color1, color2, canvas_color, extension,
        } => {
            // SAFETY: image_ptr is valid for the lifetime of the DrawOp list.
            let (img_w, img_h, img_ch) = unsafe {
                let img: &emImage = &**image_ptr;
                (img.GetWidth(), img.GetHeight(), img.GetChannelCount())
            };
            let color1 = color_hex(*color1);
            let color2 = color_hex(*color2);
            let canvas_color = color_hex(*canvas_color);
            let extension = format!("{extension:?}");
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintImageColored","x":{x},"y":{y},"w":{w},"h":{h},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_x":{src_x},"src_y":{src_y},"src_w":{src_w},"src_h":{src_h},"color1":"{color1}","color2":"{color2}","canvas_color":"{canvas_color}","extension":"{extension}",{sf}}}"#)
        }
        DrawOp::PaintBorderImage {
            x, y, w, h, l, t, r, b, image_ptr, src_l, src_t, src_r, src_b,
            alpha, canvas_color, which_sub_rects,
        } => {
            // SAFETY: image_ptr is valid for the lifetime of the DrawOp list.
            let (img_w, img_h, img_ch) = unsafe {
                let img: &emImage = &**image_ptr;
                (img.GetWidth(), img.GetHeight(), img.GetChannelCount())
            };
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("l", *l), ("t", *t), ("r", *r), ("b", *b)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintBorderImage","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"alpha":{alpha},"canvas_color":"{canvas_color}","which_sub_rects":{which_sub_rects},{hf},{sf}}}"#)
        }
        DrawOp::PaintBorderImageColored {
            x, y, w, h, l, t, r, b, image_ptr, src_l, src_t, src_r, src_b,
            color1, color2, canvas_color, which_sub_rects, alpha,
        } => {
            // SAFETY: image_ptr is valid for the lifetime of the DrawOp list.
            let (img_w, img_h, img_ch) = unsafe {
                let img: &emImage = &**image_ptr;
                (img.GetWidth(), img.GetHeight(), img.GetChannelCount())
            };
            let color1 = color_hex(*color1);
            let color2 = color_hex(*color2);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("l", *l), ("t", *t), ("r", *r), ("b", *b)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintBorderImageColored","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"color1":"{color1}","color2":"{color2}","canvas_color":"{canvas_color}","which_sub_rects":{which_sub_rects},"alpha":{alpha},{hf},{sf}}}"#)
        }

        DrawOp::PaintText { x, y, text, char_height, width_scale, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let text = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
            let hf = hex_fields(&[("x", *x), ("y", *y), ("char_height", *char_height), ("width_scale", *width_scale)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintText","x":{x},"y":{y},"text":"{text}","char_height":{char_height},"width_scale":{width_scale},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }
        DrawOp::PaintTextBoxed {
            x, y, w, h, text, max_char_height, color, canvas_color,
            box_h_align, box_v_align, text_alignment, min_width_scale, formatted, rel_line_space,
        } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let text = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
            let box_h_align = format!("{box_h_align:?}");
            let box_v_align = format!("{box_v_align:?}");
            let text_alignment = format!("{text_alignment:?}");
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("max_char_height", *max_char_height)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintTextBoxed","x":{x},"y":{y},"w":{w},"h":{h},"text":"{text}","max_char_height":{max_char_height},"color":"{color}","canvas_color":"{canvas_color}","box_h_align":"{box_h_align}","box_v_align":"{box_v_align}","text_alignment":"{text_alignment}","min_width_scale":{min_width_scale},"formatted":{formatted},"rel_line_space":{rel_line_space},{hf},{sf}}}"#)
        }

        DrawOp::PaintLinearGradient { x, y, w, h, color_a, color_b, horizontal, canvas_color } => {
            let color_a = color_hex(*color_a);
            let color_b = color_hex(*color_b);
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintLinearGradient","x":{x},"y":{y},"w":{w},"h":{h},"color_a":"{color_a}","color_b":"{color_b}","horizontal":{horizontal},"canvas_color":"{canvas_color}",{sf}}}"#)
        }
        DrawOp::PaintRadialGradient { cx, cy, rx, ry, color_inner, color_outer, canvas_color } => {
            let color_inner = color_hex(*color_inner);
            let color_outer = color_hex(*color_outer);
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintRadialGradient","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color_inner":"{color_inner}","color_outer":"{color_outer}","canvas_color":"{canvas_color}",{sf}}}"#)
        }

        DrawOp::PaintRectOutline { x, y, w, h, stroke, canvas_color } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("thickness", thickness)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintRectOutline","x":{x},"y":{y},"w":{w},"h":{h},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
        }

        DrawOp::PaintRoundRectOutline { x, y, w, h, rx, ry, stroke } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("rx", *rx), ("ry", *ry), ("thickness", thickness)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintRoundRectOutline","x":{x},"y":{y},"w":{w},"h":{h},"rx":{rx},"ry":{ry},"thickness":{thickness},"color":"{color}",{hf},{sf}}}"#)
        }

        DrawOp::PaintEllipseOutline { cx, cy, rx, ry, stroke, canvas_color } => {
            let color = color_hex(stroke.color);
            let thickness = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("cx", *cx), ("cy", *cy), ("rx", *rx), ("ry", *ry), ("thickness", thickness)]);
            format!(r#"{{"seq":{seq},"depth":{depth},"op":"PaintEllipseOutline","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"thickness":{thickness},"color":"{color}","canvas_color":"{canvas_color}",{hf},{sf}}}"#)
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
