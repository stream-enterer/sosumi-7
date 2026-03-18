# ScalarField Audit Report

**Date**: 2026-03-18
**Agent**: Batch 2
**C++ files**: emScalarField.cpp (527 LOC) + emScalarField.h (236 LOC) = 763 LOC
**Rust file**: scalar_field.rs (982 LOC)

## Findings: 10 total (+ systemic CC refs)

### [HIGH] Value type is f64 instead of i64 — fundamental type mismatch
- **C++**: `emInt64` (i64) for value/min/max
- **Rust**: `f64`
- Affects snapping, comparison, mark iteration precision. `StepByKeyboard` does integer division in C++, Rust casts to i64 (truncates fractional parts). Deliberate design decision but changes integer-snapping semantics.
- **Confidence**: high | **Coverage**: covered (render), but f64 precision differences may not surface in current tests

### [HIGH] Drag behavior completely different — absolute vs relative — **FIXED**
- **Fix**: Drag now uses absolute positioning via `check_mouse`, converting mouse position to value on every frame, matching C++ `CheckMouse` behavior.

### [MEDIUM] hit_test uses normalized space, input uses panel-space coords — **FIXED**
- **Fix**: `hit_test` removed; `check_mouse` now handles both hit detection and value computation in panel-space coords, matching C++.

### [MEDIUM] check_mouse doesn't apply marks_never_hidden culling — **NOTE**
- `MarksNeverHidden` is not used in C++ `DoScalarField`; Rust layout matches C++ in this regard. Not an actionable divergence.

### [MEDIUM] Arrow keys (Left/Right) accepted as increment/decrement — **FIXED**
- **Fix**: Removed ArrowLeft/ArrowRight, only +/- character keys matching C++.

### [MEDIUM] Missing IsEnabled() check on input (only checks editable) — **FIXED**
- **Fix**: Input gating now checks both `is_editable()` and `is_enabled()` matching C++.
- See CC-03
- **Confidence**: high | **Coverage**: uncovered

### [LOW] VCT_MIN_EXT missing (see CC-04) — **FIXED**

### [LOW] set_* methods don't fire signals (see CC-02)

### [LOW] HowTo text built at paint-time (string alloc per frame)
- Functionally produces same text but allocates every frame
- **Confidence**: medium | **Coverage**: uncovered

### [LOW] preferred_size uses hardcoded dims vs C++ tallness-based
- Design difference, not a bug
- **Confidence**: low | **Coverage**: N/A

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 2 |
| MEDIUM | 4 |
| LOW | 4 |

## Most Critical
1. **f64 vs i64** — fundamental type change affects snapping behavior
2. **Drag is relative, not absolute** — user-facing interaction change. C++ click-on-scale positions the needle there. Rust only drags from current position.

## Recommended Tests
- Drag-to-position, decrement key, StepByKeyboard with intervals, disabled input blocking, custom formatter rendering, mark culling
