use std::io::Write;
use std::path::PathBuf;
use emcore::emImage::emImage;
use emcore::emPainterDrawList::DrawOp;

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
pub fn dump_draw_ops(name: &str, ops: &[DrawOp]) {
    let path = output_path(name);
    let _ = std::fs::create_dir_all(path.parent().expect("path has parent"));
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .expect("open rust_ops.jsonl");

    for (seq, op) in ops.iter().enumerate() {
        let line = serialize_op(seq, op);
        writeln!(f, "{line}").expect("write line");
    }
}

fn serialize_op(seq: usize, op: &DrawOp) -> String {
    match op {
        DrawOp::PushState => {
            format!(r#"{{"seq":{seq},"op":"PushState"}}"#)
        }
        DrawOp::PopState => {
            format!(r#"{{"seq":{seq},"op":"PopState"}}"#)
        }
        DrawOp::SetOffset(dx, dy) => {
            format!(r#"{{"seq":{seq},"op":"SetOffset","dx":{dx},"dy":{dy}}}"#)
        }
        DrawOp::SetTransformation { ox, oy, sx, sy } => {
            format!(r#"{{"seq":{seq},"op":"SetTransformation","ox":{ox},"oy":{oy},"sx":{sx},"sy":{sy}}}"#)
        }
        DrawOp::ClipRect { x, y, w, h } => {
            format!(r#"{{"seq":{seq},"op":"ClipRect","x":{x},"y":{y},"w":{w},"h":{h}}}"#)
        }
        DrawOp::SetCanvasColor(c) => {
            let color = color_hex(*c);
            format!(r#"{{"seq":{seq},"op":"SetCanvasColor","color":"{color}"}}"#)
        }
        DrawOp::SetAlpha(a) => {
            format!(r#"{{"seq":{seq},"op":"SetAlpha","alpha":{a}}}"#)
        }

        DrawOp::PaintRect { x, y, w, h, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h)]);
            format!(r#"{{"seq":{seq},"op":"PaintRect","x":{x},"y":{y},"w":{w},"h":{h},"color":"{color}","canvas_color":"{canvas_color}",{hf}}}"#)
        }
        DrawOp::PaintRoundRect { x, y, w, h, radius, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("x", *x), ("y", *y), ("w", *w), ("h", *h), ("radius", *radius)]);
            format!(r#"{{"seq":{seq},"op":"PaintRoundRect","x":{x},"y":{y},"w":{w},"h":{h},"radius":{radius},"color":"{color}","canvas_color":"{canvas_color}",{hf}}}"#)
        }
        DrawOp::PaintEllipse { cx, cy, rx, ry, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let hf = hex_fields(&[("cx", *cx), ("cy", *cy), ("rx", *rx), ("ry", *ry)]);
            format!(r#"{{"seq":{seq},"op":"PaintEllipse","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color":"{color}","canvas_color":"{canvas_color}",{hf}}}"#)
        }
        DrawOp::PaintPolygon { vertices, color, canvas_color } => {
            let verts = vertices_json(vertices);
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"op":"PaintPolygon","vertices":{verts},"color":"{color}","canvas_color":"{canvas_color}"}}"#)
        }
        DrawOp::PaintSolidPolyline { vertices, stroke, closed, canvas_color } => {
            let verts = vertices_json(vertices);
            let stroke_color = color_hex(stroke.color);
            let stroke_width = stroke.width;
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"op":"PaintSolidPolyline","vertices":{verts},"stroke_color":"{stroke_color}","stroke_width":{stroke_width},"closed":{closed},"canvas_color":"{canvas_color}"}}"#)
        }

        DrawOp::PaintImageFull { x, y, w, h, image_ptr, alpha, canvas_color } => {
            // SAFETY: image_ptr is valid for the lifetime of the DrawOp list (owned by panel behavior).
            let (img_w, img_h, img_ch) = unsafe {
                let img: &emImage = &**image_ptr;
                (img.GetWidth(), img.GetHeight(), img.GetChannelCount())
            };
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"op":"PaintImageFull","x":{x},"y":{y},"w":{w},"h":{h},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"alpha":{alpha},"canvas_color":"{canvas_color}"}}"#)
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
            format!(r#"{{"seq":{seq},"op":"PaintImageColored","x":{x},"y":{y},"w":{w},"h":{h},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_x":{src_x},"src_y":{src_y},"src_w":{src_w},"src_h":{src_h},"color1":"{color1}","color2":"{color2}","canvas_color":"{canvas_color}","extension":"{extension}"}}"#)
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
            format!(r#"{{"seq":{seq},"op":"PaintBorderImage","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"alpha":{alpha},"canvas_color":"{canvas_color}","which_sub_rects":{which_sub_rects},{hf}}}"#)
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
            format!(r#"{{"seq":{seq},"op":"PaintBorderImageColored","x":{x},"y":{y},"w":{w},"h":{h},"l":{l},"t":{t},"r":{r},"b":{b},"img_w":{img_w},"img_h":{img_h},"img_ch":{img_ch},"src_l":{src_l},"src_t":{src_t},"src_r":{src_r},"src_b":{src_b},"color1":"{color1}","color2":"{color2}","canvas_color":"{canvas_color}","which_sub_rects":{which_sub_rects},"alpha":{alpha},{hf}}}"#)
        }

        DrawOp::PaintText { x, y, text, char_height, width_scale, color, canvas_color } => {
            let color = color_hex(*color);
            let canvas_color = color_hex(*canvas_color);
            let text = text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
            let hf = hex_fields(&[("x", *x), ("y", *y), ("char_height", *char_height), ("width_scale", *width_scale)]);
            format!(r#"{{"seq":{seq},"op":"PaintText","x":{x},"y":{y},"text":"{text}","char_height":{char_height},"width_scale":{width_scale},"color":"{color}","canvas_color":"{canvas_color}",{hf}}}"#)
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
            format!(r#"{{"seq":{seq},"op":"PaintTextBoxed","x":{x},"y":{y},"w":{w},"h":{h},"text":"{text}","max_char_height":{max_char_height},"color":"{color}","canvas_color":"{canvas_color}","box_h_align":"{box_h_align}","box_v_align":"{box_v_align}","text_alignment":"{text_alignment}","min_width_scale":{min_width_scale},"formatted":{formatted},"rel_line_space":{rel_line_space},{hf}}}"#)
        }

        DrawOp::PaintLinearGradient { x, y, w, h, color_a, color_b, horizontal, canvas_color } => {
            let color_a = color_hex(*color_a);
            let color_b = color_hex(*color_b);
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"op":"PaintLinearGradient","x":{x},"y":{y},"w":{w},"h":{h},"color_a":"{color_a}","color_b":"{color_b}","horizontal":{horizontal},"canvas_color":"{canvas_color}"}}"#)
        }
        DrawOp::PaintRadialGradient { cx, cy, rx, ry, color_inner, color_outer, canvas_color } => {
            let color_inner = color_hex(*color_inner);
            let color_outer = color_hex(*color_outer);
            let canvas_color = color_hex(*canvas_color);
            format!(r#"{{"seq":{seq},"op":"PaintRadialGradient","cx":{cx},"cy":{cy},"rx":{rx},"ry":{ry},"color_inner":"{color_inner}","color_outer":"{color_outer}","canvas_color":"{canvas_color}"}}"#)
        }

        // Catch-all for variants not individually serialized above.
        other => {
            let variant = variant_name(other);
            format!(r#"{{"seq":{seq},"op":"{variant}","_unserialized":true}}"#)
        }
    }
}

fn variant_name(op: &DrawOp) -> &'static str {
    match op {
        DrawOp::PushState => "PushState",
        DrawOp::PopState => "PopState",
        DrawOp::SetOffset(..) => "SetOffset",
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
        DrawOp::PaintImageFull { .. } => "PaintImageFull",
        DrawOp::PaintImageColored { .. } => "PaintImageColored",
        DrawOp::PaintImageScaled { .. } => "PaintImageScaled",
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
        DrawOp::PaintPolylineWithoutArrows { .. } => "PaintPolylineWithoutArrows",
        DrawOp::PaintPolylineWithArrows { .. } => "PaintPolylineWithArrows",
        DrawOp::PaintDashedPolyline { .. } => "PaintDashedPolyline",
        DrawOp::PaintBezier { .. } => "PaintBezier",
        DrawOp::PaintBezierOutline { .. } => "PaintBezierOutline",
        DrawOp::PaintBezierLine { .. } => "PaintBezierLine",
        DrawOp::PaintImageSimple { .. } => "PaintImageSimple",
        DrawOp::PaintBorderImageColored { .. } => "PaintBorderImageColored",
        DrawOp::PaintEdgeCorrection { .. } => "PaintEdgeCorrection",
    }
}
