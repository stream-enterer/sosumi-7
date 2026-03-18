# CheckBox Audit Report

**Date**: 2026-03-18
**Agent**: Calibration batch, checkbox auditor
**C++ files**: emCheckBox.cpp (38 LOC), emCheckBox.h (52 LOC), inherits emCheckButton (190 LOC) → emButton (623 LOC)
**Rust file**: check_box.rs (346 LOC)

## Size Asymmetry Verdict

Category **(b): code duplication**. Rust CheckBox is 4x larger because it inlines the entire ShownBoxed paint path, input handling, toggle logic, box hit test, and geometry calculations that C++ inherits from emButton and emCheckButton. No shared base widget in Rust — Button, CheckButton, and CheckBox are three independent standalone implementations.

## Findings

### [INFO] Size asymmetry — code duplication, not behavioral divergence
- **Confidence**: high | **Coverage**: covered

### [INFO] CheckBox label paint uses FgColor instead of explicit color parameter
- C++: `GetLook().GetFgColor()` in ShownBoxed path (emButton.cpp:290-307)
- Rust: calls `paint_label` (not `paint_label_colored`) — check_box.rs:127-130
- Probably correct but needs verification that `paint_label` uses FgColor
- **Confidence**: medium | **Coverage**: covered (pixel golden)

### [SUSPECT] CheckBox outer hit test radius differs from C++ — **FIXED**
- **Fix**: Changed to content_rect with r=h*0.2 (CC-06 boxed path fix).
- **Confidence**: medium | **Coverage**: covered (widget_checkbox_toggle)

### [NOTE] Box hit test equivalent but redundantly computed — **CLOSED 2026-03-18**
- Rust recomputes bx/by instead of using box_label_geometry values.
- **Status**: Code quality note, not a behavioral divergence. Output is identical. Kept for potential future cleanup but has zero user-visible impact.

### [GAP] Missing disabled visual overlay — **FIXED**
- **Fix**: Gray overlay 0x888888E0 added when disabled; label dim implemented.
- **Confidence**: high | **Coverage**: uncovered

### [GAP] Missing ShownRadioed path — **NOTE**
- C++: `ShownBoxed + ShownRadioed` → circle instead of rounded rect
- RadioBox is a separate widget that handles the ShownRadioed paint path. CheckBox only needs the ShownBoxed (rounded rect) path. The missing path in CheckBox is intentional given the widget split.
- **Confidence**: high | **Coverage**: uncovered

### [BUG] Missing Enter key support — **FIXED**
- C++: emButton::Input handles EM_KEY_ENTER (emButton.cpp:113-120)
- **Fix**: Added `InputKey::Enter | InputKey::Space` pattern in CheckBox and CheckButton
- **Confidence**: high | **Coverage**: uncovered

### [BUG] SetChecked does not fire callback — **FIXED**
- C++: SetChecked fires CheckSignal + calls CheckChanged() (emCheckButton.cpp:39-48)
- **Fix**: set_checked now fires on_check callback when state changes
- **Confidence**: high | **Coverage**: uncovered

## Summary

| Severity | Count |
|----------|-------|
| BUG | 2 |
| SUSPECT | 1 |
| GAP | 2 |
| NOTE | 1 |
| INFO | 2 |

## Cross-cutting: See CC-01, CC-02, CC-03, CC-04
