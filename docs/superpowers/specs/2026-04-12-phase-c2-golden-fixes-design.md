# Phase C2: Remaining Golden Test Fixes — Design Spec

## Goal

Resolve all 7 remaining golden test failures, reaching 242/243 pass (1 ignored).

## Approach

Delete-and-rewrite from C++ for all divergent code. No Rust diagnosis — replace with fresh mechanical port. Anything noticed as unported during rewrite gets implemented immediately.

## Work Items

### 1. testpanel_root (4,020px)

Test harness `paint_primitives()` in `test_panel.rs` is missing many paint calls present in C++ `emTestPanel.cpp:276-460`. Delete and rewrite from C++. Implement any missing paint methods encountered.

### 2. testpanel_expanded (19,954px)

Cascade from testpanel_root + canvas_color propagation + glyph path differences. Re-run after item 1, diagnose and fix remaining pixels.

### 3. splitter_v_extreme_tall (84px)

Rust `y_accumulate_4ch` in `emPainterInterpolation.rs` uses u64 accumulators. C++ `emPainter_ScTlIntImg.cpp:268-377` uses `emUInt32` (u32). Delete and rewrite matching C++ types exactly.

### 4. composition_tktest_1x/2x (13,396px + 94px)

Area sampling init/transform divergence. Delete Rust implementation, rewrite from C++ `ScanlineTool::Init`. Match all integer types and operation order. FFI harness as fallback if rewrite alone doesn't resolve.

### 5. file_selection_box (14,123px)

Full rewrite of `emFileSelectionBox.rs` LayoutChildren + Cycle from C++ `emFileSelectionBox.cpp`. Rewrite test `settle()` to drive cycles like C++ `sched.Run()`.

### 6. border_roundrect_thin (2px)

Sub-pixel coverage at extreme thin panels. Delete Rust `fill_polygon_aa` scanline edge handling, rewrite from C++ `PaintPolygon`.

## Branching

Single branch `phase-c2-golden-fixes` off main. One commit per work item.

## Success Criteria

- 242/243 pass, 1 ignored
- No regressions
