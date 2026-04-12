# Canvas Color Fixes Design

## Problem

27 golden tests fail. The majority of widget test failures are caused by Rust paint calls passing `emColor::TRANSPARENT` (0x00000000) as the `canvas_color` parameter where C++ passes the tracked `canvasColor` variable. This causes semi-transparent content to blend against transparent-black instead of the panel background.

## Root Cause

Confirmed mechanically via DrawOp JSONL parameter diffs (hex-f64 bit-level comparison). The f64 ULP geometry hypothesis was disproven — border geometry is bit-identical at panel-local scale.

## Fixes

All 13 fixes follow the same pattern: replace `emColor::TRANSPARENT` with `painter.GetCanvasColor()` at the canvas_color parameter position.

### Widget face PaintRoundRect (4 fixes)

| File | Line | Context |
|------|------|---------|
| emCheckButton.rs | 88 | Face PaintRoundRect canvas |
| emRadioButton.rs | 316 | Face PaintRoundRect canvas |
| emCheckBox.rs | 152 | Face PaintRoundRect canvas |
| emRadioBox.rs | 154 | Face PaintRoundRect canvas |

### ListBox/FileSelectionBox highlight PaintRoundRect (5 fixes)

| File | Line | Context |
|------|------|---------|
| emListBox.rs | 193 | DefaultItemPanelBehavior highlight |
| emListBox.rs | 1108 | Inline item rendering highlight |
| emFileSelectionBox.rs | 121 | Selection highlight |
| emFileSelectionBox.rs | 181 | Icon body PaintRoundRect |
| emFileSelectionBox.rs | 188 | Folder tab PaintRoundRect |

### Outer border initial fills (4 fixes)

| File | Line | Context |
|------|------|---------|
| emBorder.rs | 1733 | OuterBorderType::Filled PaintRect |
| emBorder.rs | 1743 | OuterBorderType::MarginFilled PaintRect |
| emBorder.rs | 1759 | OuterBorderType::Rect PaintRect |
| emBorder.rs | 1953 | OuterBorderType::PopupRoot PaintRect |

### Confirmed correct — do NOT change

- emButton.rs:241,261,282 — PaintBorderImage overlay (C++ also passes 0)
- emCheckButton.rs:143,163,184 — same
- emRadioButton.rs:372,392,413 — same
- emCheckBox.rs:178,201,210 — same
- emRadioBox.rs:182,204,213 — same
- emScalarField.rs:428,479,494 — C++ sets canvasColor=0 before these
- emColorField.rs:453 — PaintRectOutline (C++ has no canvasColor)
- emBorder.rs:1626 — color1 arg, not canvas
- emFileSelectionBox.rs:205 — PaintText "Parent Directory" (C++ passes 0)
- emViewAnimator.rs all calls — C++ uses default 0
- emView.rs overlay calls — C++ uses default 0

## Out of Scope

- Category B (testpanel/tktest): Missing CustomRect panel types — requires new implementation
- Category C (bezier, starfield, roundrect_thin): Sub-pixel/AA primitive issues — separate investigation
- Clipping architecture: Rust SetClipping transforms vs C++ absolute coords — separate if needed

## Verification

1. Apply all 13 fixes
2. Run `cargo test --test golden -- --test-threads=1`
3. Any still-failing widget tests: run DrawOp diff (`DUMP_DRAW_OPS=1` + `python3 scripts/diff_draw_ops.py`)
4. Commit passing fixes
