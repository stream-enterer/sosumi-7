# Splitter Audit Report

**Date**: 2026-03-18
**Agent**: Batch 2
**C++ files**: emSplitter.cpp (271 LOC) + emSplitter.h (139 LOC) = 410 LOC
**Rust file**: splitter.rs (293 LOC)

## Findings: 11 total

### [MEDIUM] Drag math uses uncapped grip size during move — **FIXED**
- **Fix**: Drag now uses capped grip size from calc_grip_rect (gw/gh).
- **Confidence**: medium | **Coverage**: partially covered

### [MEDIUM] Missing MouseInGrip hover tracking — **FIXED**
- **Fix**: Added mouse_in_grip tracking on Move events, gated get_cursor on it.
- **Confidence**: high | **Coverage**: uncovered

### [LOW] Default min/max position differs (0.05/0.95 vs 0.0/1.0) — **FIXED**
- **Fix**: Changed to 0.0/1.0 matching C++ defaults.
- **Confidence**: high | **Coverage**: covered (overridden)

### [LOW] set_limits has no min>max validation — **FIXED**
- **Fix**: Clamps to [0,1], averages if inverted, matching C++ SetMinMaxPos.
- **Confidence**: high | **Coverage**: partially covered

### [LOW] Hit test is 1D not 2D — **FIXED**
- **Fix**: Now checks both axes.
- **Confidence**: medium | **Coverage**: uncovered

### [LOW] Inclusive vs exclusive upper bound in hit test — **FIXED**
- **Fix**: Changed `<=` to `<` matching C++.
- **Confidence**: low | **Coverage**: uncovered

### [LOW] Missing IsEnabled() check on press (see CC-03)

### [LOW] Missing borderScaling factor in grip size
- Rust uses fixed 0.015. C++ multiplies by GetBorderScaling(). Latent: no callers change scaling.
- **Confidence**: high | **Coverage**: covered (default scaling)

### [LOW] canvas_color passed as TRANSPARENT
- Same pattern as Label. Optimization difference, not pixel difference.
- **Confidence**: low | **Coverage**: covered

### [LOW] Missing disabled state alpha (255 vs 64) (see CC-03)

### [LOW] Missing Focus()/Activate() calls on drag
- **Confidence**: high | **Coverage**: uncovered

## Summary

| Severity | Count |
|----------|-------|
| MEDIUM | 2 |
| LOW | 9 |

## Overall: Functionally correct for common case. Well-covered by golden tests. Main gap is cursor/hover behavior.
