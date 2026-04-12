# Phase C2: Remaining Golden Test Fixes — Design Spec

## Goal

Resolve all 7 remaining golden test failures, reaching 242/243 pass (1 ignored).

## Approach: Steelmanned Delete-and-Rewrite

1. **C++ is source of truth** — read C++ exhaustively for every function that produces the failing output.
2. **Read Rust only for boundaries** — function signatures, callers, integration points. Not to diagnose bugs.
3. **Delete and rewrite from C++**, porting ALL code paths in each touched function (not just the failing path).
4. **Calibrate scope to divergence boundary** — a type fix stays a type fix; an architectural mismatch gets architectural rewrite. Don't over-scope or under-scope.
5. **If Rust architecture differs from C++ in ways that cause the failure**, refactor to match C++ architecture.
6. **Gate each commit on zero regressions** — run ALL golden tests after each fix.
7. **Subagent tasks are self-contained** — include exact C++ code to port, exact Rust replacement target with line numbers, exact verification command with expected pixel count.
8. **Anything unported in a touched function gets ported** — but don't hunt outside the replacement boundary.

### Anti-patterns blocked

- No Rust code-reading to diagnose bugs (read C++ instead)
- No analysis paralysis on complex code (C++ is the answer, port it)
- No partial ports or TODOs in touched functions
- No scorched earth beyond the divergence boundary
- No trusting subagent claims without pixel-count verification
- No fixing one test without checking all 243

## Work Items (attack order)

### 1. testpanel_root (4,020px)

**Nature**: Test harness gap — missing paint calls, not rendering bug.
**Scope**: Add missing primitives to `test_panel.rs:paint_primitives()` from C++ `emTestPanel.cpp:276-460`. Implement any missing paint methods encountered in emPainter.
**Calibration**: Addition only. No deletion of existing calls.

### 2. testpanel_expanded (19,954px)

**Nature**: Cascade from testpanel_root + possible independent causes.
**Scope**: Re-run after item 1. If pixels remain, diagnose independently using DrawOp diff.
**Calibration**: Depends on item 1 results. May be zero additional work.

### 3. splitter_v_extreme_tall (84px)

**Nature**: Single type mismatch — u64 vs u32 accumulator.
**Scope**: Change accumulator types in `emPainterInterpolation.rs:y_accumulate_4ch` to match C++ `emUInt32`. Verify all accumulator arithmetic matches C++ wrapping behavior.
**Calibration**: Type annotation change + wrapping arithmetic. Minimal scope.

### 4. composition_tktest_1x/2x (13,396px + 94px)

**Nature**: Area sampling math pipeline divergence.
**Scope**: Delete and rewrite area sampling init/transform from C++ `ScanlineTool::Init` in `emPainter_ScTlIntImg.cpp`. Match all integer types and operation order.
**Calibration**: Medium scope — focused on init+transform functions, not the entire interpolation module.
**Fallback**: FFI harness to isolate exact divergence point if rewrite alone doesn't resolve.

### 5. file_selection_box (14,123px)

**Nature**: Widget lifecycle/layout architecture mismatch.
**Scope**: Rewrite `emFileSelectionBox.rs` LayoutChildren + Cycle from C++ `emFileSelectionBox.cpp`. Rewrite test `settle()` to drive cycles like C++ `sched.Run()`.
**Calibration**: Architectural rewrite of layout+cycle. Other FSB code (construction, input handling) left alone unless it feeds the failure.

### 6. border_roundrect_thin (2px)

**Nature**: Sub-pixel polygon coverage edge case.
**Scope**: Rewrite polygon scanline edge handling in `fill_polygon_aa` from C++ `PaintPolygon`.
**Calibration**: High regression risk — 235 passing tests use polygon fill. Extra verification required.

## Branching

Single branch `phase-c2-golden-fixes` off main. One commit per work item.

## Success Criteria

- 242/243 pass, 1 ignored
- Zero regressions in currently-passing tests after each commit
