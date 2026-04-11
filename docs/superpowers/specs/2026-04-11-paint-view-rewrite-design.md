# Design: Rewrite emView::Paint + paint_panel_recursive to Match C++ PaintView

**Date:** 2026-04-11
**Status:** Draft

## Problem

An audit of the Rust `Paint` and `paint_panel_recursive` functions against C++ `emView::Paint` found 9 divergences and 8 missing logic blocks. Only 4 blocks matched. The paint traversal itself is suspected as the cause of composition_tktest failures (zero DrawOp mismatches but 6.74% pixel diffs).

## Current State: 229/243 golden tests pass

13 failing tests. The composition tests suggest the traversal order or state management is wrong, not individual paint operations.

## C++ Architecture (Ground Truth)

C++ `emView::Paint(painter, canvasColor)` is a single flat function with an iterative DFS loop:

1. Assert scale == 1.0
2. `EnterUserSpace()` (mutex, no-op for us)
3. If no SVP: `painter.Clear(BackgroundColor, canvasColor)` → done
4. Get origin (ox, oy) and render region (rx1..rx2, ry1..ry2) from painter clip minus origin
5. Check if SVP doesn't fully cover render region (opacity + bounds). If so, clear with resolved canvas color.
6. Clamp SVP clip to render region → (cx1, cy1, cx2, cy2)
7. If clip valid: copy painter → `pnt=painter`, set clip and transform on pnt, call `SVP->Paint(pnt, canvasColor)`
8. `LeaveUserSpace()` — but painting children does NOT re-enter user space
9. Iterative DFS over children:
   - Check `p->Viewed` (NOT visible)
   - Clamp clip to render region
   - If clip valid: `pnt.SetClipping(...)`, `pnt.SetTransformation(...)`, `EnterUserSpace()`, `p->Paint(pnt, p->CanvasColor)`, `LeaveUserSpace()`
   - Descend to FirstChild if exists, else Next sibling, else walk up to parent
10. After loop: `EnterUserSpace()`
11. `PaintHighlight(painter)` — only when SVP exists
12. ActiveAnimator paint (TODO)
13. StressTest overlay

### Key C++ Details

- **Painter copy**: `pnt=painter` copies all state. `pnt` is modified (SetClipping, SetTransformation) while `painter` stays pristine for PaintHighlight.
- **SetClipping(x1, y1, x2, y2)**: Absolute pixel coords, no intersection — just sets the values.
- **Clip clamping is manual**: `cx1=p->ClipX1; if (cx1<rx1) cx1=rx1;` — the Paint function clamps panel clips against the render region.
- **No "visible" check**: Only `p->Viewed` is checked, not visibility.
- **Children get `p->CanvasColor`**: Not a resolved/inherited value.
- **sy = ViewedWidth / CurrentPixelTallness**: For square pixels this equals ViewedWidth.
- **Iterative DFS**: FirstChild/Next/Parent pointer walking, not recursion.

## Divergences in Current Rust Code

| # | Divergence | Impact |
|---|-----------|--------|
| 1 | Background: unconditional PaintRect vs conditional Clear | Wrong pixels when SVP covers viewport |
| 2 | No render region (rx1..ry2) computation | Clip clamping wrong |
| 3 | No SVP opacity/coverage check | Background clear when not needed or vice versa |
| 4 | `GetClipX1()` returns `bool` not `f64` | Name correspondence violation, blocks clip accessors |
| 5 | Recursive DFS vs iterative | Different state lifecycle, stack overflow risk on deep trees |
| 6 | `visible` check that C++ doesn't do | Panels skipped that shouldn't be |
| 7 | Canvas color inheritance logic | Children should get raw `p->CanvasColor`, not resolved |
| 8 | PaintHighlight called unconditionally | Should only be called when SVP exists |
| 9 | SetClipping semantics differ (user coords + intersection vs pixel coords + set) | Already compensated by push/pop, but feed must be correct |
| 10 | Painter state lifecycle via push/pop vs copy | Functionally equivalent IF done correctly |

## Design

### Approach: Faithful Structural Port

Rewrite `Paint` and the child-painting loop to match C++ block-by-block. Keep push/pop state in place of painter copy (since emPainter can't be cloned due to `&mut` target). The state transitions must produce identical clip/transform values.

### Changes Required

#### 1. Fix `GetClipX1()` (emPainter.rs)

Rename the current `GetClipX1() -> bool` to `IsClipEmpty() -> bool`. Add proper C++ accessors:
- `GetClipX1() -> f64`
- `GetClipY1() -> f64`
- `GetClipX2() -> f64`
- `GetClipY2() -> f64`
- `GetOriginX() -> f64`
- `GetOriginY() -> f64`
- `GetScaleX() -> f64`
- `GetScaleY() -> f64`

Also add `SetClippingAbsolute(x1, y1, x2, y2)` that sets clip in pixel coords without intersection (matching C++ `SetClipping` semantics). The existing `SetClipping(x, y, w, h)` stays for other callers.

#### 2. Add `IsOpaque` accessor to PanelTree

```rust
pub fn IsOpaque(&mut self, id: PanelId) -> bool {
    // take_behavior, call IsOpaque, put_behavior
}
```

#### 3. Rewrite `Paint` function

Match C++ line by line:
- Skip EnterUserSpace/LeaveUserSpace (single-threaded)
- If no SVP: `painter.Clear(background_color)` with canvas color
- Compute render region from painter clip/origin
- Check SVP opacity + coverage → conditional clear
- Clamp SVP clip to render region
- Push state, set absolute clip, set transformation, call SVP Paint
- Iterative DFS for children (no recursion)
- Pop state after loop
- PaintHighlight only when SVP exists
- StressTest overlay

#### 4. Rewrite child painting as iterative DFS

Use `GetFirstChild`, `GetNext`, `GetParentContext` for tree walking. Match C++ exactly:

```
p = svp.FirstChild
loop:
  if p.Viewed:
    clamp clip, if valid:
      set clip, set transform, paint, descend to FirstChild if exists
  advance: Next sibling, or walk up to parent
  break when back at SVP
```

Each child paint needs push/pop state to isolate clip/transform changes. This replaces the recursive `paint_panel_recursive`.

#### 5. Keep `paint_panel_recursive` for `paint_sub_tree`

`paint_sub_tree` is used by `emSubViewPanel`. It can keep the recursive approach (or be rewritten later). The main `Paint` function gets the iterative rewrite.

Actually — `paint_sub_tree` delegates to `paint_panel_recursive`. Since we're rewriting `paint_panel_recursive`, we need to either:
- Extract the iterative DFS into a shared helper used by both `Paint` and `paint_sub_tree`
- Keep `paint_panel_recursive` as-is for `paint_sub_tree` and write the iterative loop inline in `Paint`

Simpler: write the iterative DFS as a private method `paint_tree_iterative(tree, painter, svp_id, rx1, ry1, rx2, ry2, ox, oy)` that both `Paint` and `paint_sub_tree` can call. But `paint_sub_tree` has different entry semantics (different base offset, no SVP special handling). 

Decision: Write the iterative child loop inline in `Paint`. Keep `paint_panel_recursive` for `paint_sub_tree` but fix its bugs (remove `visible` check, fix clip handling).

#### 6. Remove `visible` check from paint path

C++ only checks `Viewed`, not visible. The `visible` flag affects layout, not painting. Remove `p.visible` from paint path guards.

#### 7. Canvas color: match C++ exactly

- SVP: resolve `canvasColor` (if SVP's CanvasColor not opaque, use BackgroundColor)
- Children: pass `p->CanvasColor` directly, no fallback

#### 8. Clear semantics

Add a `ClearWithCanvas(color, canvas_color)` method to emPainter that matches C++ `Clear(texture, canvasColor)` — paints a rect over the entire clip region.

### What NOT to Change

- `paint_highlight` — stays as separate function, just ensure it's called only when SVP exists
- `StressTest` overlay — already matches
- `push_state`/`pop_state` — keeps replacing C++ painter copy
- `SetClipping(x, y, w, h)` existing method — other callers use it

### Verification

1. `cargo clippy -- -D warnings` must pass
2. `cargo-nextest ntr` must pass  
3. Golden tests: target improvement from 229/243, especially composition_tktest_1x/2x
4. DrawOp diff for failing tests to diagnose remaining issues

## Risks

- The iterative DFS needs correct push/pop pairing — off-by-one in the walk could leave state stack corrupted
- `paint_sub_tree` callers may be affected if `paint_panel_recursive` bugs are fixed — verify emSubViewPanel tests
- Adding new emPainter methods may affect DrawOp recording — ensure new state ops are recorded properly
