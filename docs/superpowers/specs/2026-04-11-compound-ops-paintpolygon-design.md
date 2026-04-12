# Compound Ops → PaintPolygon Architectural Alignment

**Date**: 2026-04-11
**Type**: Architectural cleanup
**Goal**: Make Rust compound ops call `PaintPolygon` instead of `fill_polygon_aa`, matching C++ architecture.

## Context

In C++ `emPainter.cpp`, compound paint ops (PaintBezier, PaintEllipse, PaintEllipseSector, PaintRoundRect) all build vertex arrays and delegate to `PaintPolygon(xy, n, texture, canvasColor)`. PaintPolygon handles canvas_color setup and the actual polygon fill.

In Rust, PaintEllipseSector already follows this pattern. The other three (PaintBezier, PaintEllipse, PaintRoundRect) call `fill_polygon_aa` directly with manual canvas_color save/restore — duplicating what PaintPolygon does internally.

## Design

### Changes

Three functions in `crates/emcore/src/emPainter.rs`:

**PaintEllipse** (line 672):
- Change `let Some(proof)` → `let Some(_proof)` (proof no longer consumed)
- Remove `saved_canvas` / canvas_color save/restore
- Replace `self.fill_polygon_aa(proof, &verts, color, WindingRule::NonZero)` with `self.PaintPolygon(&verts, color, canvas_color)`

**PaintRoundRect** (line 1103):
- Same transformation as PaintEllipse

**PaintBezier** (line 2177):
- Same transformation as PaintEllipse

### Unchanged

- **PaintEllipseSector**: Already calls PaintPolygon (existence proof)
- **PaintPolygon**: No changes needed
- **fill_polygon_aa**: No changes needed
- **All outline ops**: Not in scope

### Recording behavior

Unchanged. In recording mode (`PaintTarget::DrawList`), compound ops record their own DrawOp and return early — PaintPolygon never executes. In direct mode (`PaintTarget::emImage`), both try_record calls return `Some(proof)` without side effects; PaintPolygon renders normally.

### Pixel output

Expected identical. Both the current path (manual canvas_color + fill_polygon_aa) and the new path (PaintPolygon doing the same) produce the same result.

## Verification

1. `cargo clippy -- -D warnings` — must pass
2. `cargo-nextest ntr` — must pass
3. Three golden tests — confirm diffs unchanged:
   - `painter_bezier_stroked`
   - `widget_border_roundrect_thin`
   - `widget_splitter_v_extreme_tall`
