# Button Audit Report

**Date**: 2026-03-18
**Agent**: Calibration batch, button auditor
**C++ files**: emButton.cpp (452 LOC), emButton.h (171 LOC) = 623 LOC
**Rust file**: button.rs (557 LOC)

## Findings: 14 total

### [BUG] hit_test() does not match C++ CheckMouse face inset formula — **FIXED**
- **C++**: emButton.cpp:354-358 — `d = (1-(264-14)/264)*r`, tests face (fx,fy,fw,fh,fr)
- **Fix**: Applied face inset d=(14/264)*r and r clamp in hit_test() for Button, RadioButton, CheckButton (non-boxed path). CheckBox, RadioBox use content_rect with r=h*0.2 (boxed path).
- **Confidence**: high | **Coverage**: widget_button_click may not catch corners

### [BUG] Label shrink missing for ShownChecked state — **FIXED**
- **C++**: emButton.cpp:377-383 — shrinks by 0.98 (pressed) or 0.983 (checked)
- **Fix**: Added checked branch with s=0.983, pressed takes priority with s=0.98
- **Confidence**: medium | **Coverage**: uncovered

### [BUG] Missing checked-state border image overlay — **FIXED**
- **C++**: emButton.cpp:402-410 — three overlay states (pressed, checked, normal)
- **Fix**: Added ButtonChecked overlay branch between pressed and normal
- **Confidence**: medium | **Coverage**: uncovered

### [SUSPECT] Keyboard: Rust handles Space (C++ doesn't); different press/release cycle — **FIXED**
- **Fix**: Removed Space, Enter is now instant Click() on press with no visual state change.
- Modifier gated on NoMod/ShiftMod matching C++.

### [SUSPECT] Keyboard: press/release visual state divergence — **FIXED**
- **Fix**: Enter does instant Click(), no Pressed state change.

### [GAP] No modifier key checks on mouse press — **FIXED**
- **C++**: `state.IsNoMod() || state.IsShiftMod()` gate (emButton.cpp:81-83)
- **Fix**: Added ctrl/alt/meta check before hit test in all 5 button-family widgets
- **Confidence**: high | **Coverage**: uncovered

### [GAP] No VCT_MIN_EXT minimum extent check
- **C++**: requires `GetViewCondition(VCT_MIN_EXT)>=8.0` (emButton.cpp:84-85)
- **Rust**: no such guard — tiny buttons can be clicked
- Cross-cutting: CC-04
- **Confidence**: high | **Coverage**: uncovered

### [GAP] No enabled/disabled state
- **C++**: gates input on IsEnabled(), dims colors by 75% transparency
- **Rust**: no enabled concept at all
- Cross-cutting: CC-03
- **Confidence**: high | **Coverage**: uncovered

### [GAP] No clip rect check on mouse release
- **C++**: emButton.cpp:101-109 — verifies release position against clip rect
- **Rust**: no clip rect verification
- **Confidence**: high | **Coverage**: uncovered

### [GAP] No IsViewed() check on mouse release
- **C++**: emButton.cpp:101
- **Rust**: no equivalent
- **Confidence**: high | **Coverage**: uncovered

### [GAP] No Focus() call on mouse press
- **C++**: emButton.cpp:86
- **Rust**: no focus management in button
- **Confidence**: medium | **Coverage**: uncovered

### [GAP] Boxed/RadioBox paint path missing from base Button
- shown_boxed/shown_radioed flags exist but are dead code in paint()
- Intentional — separate widgets — but API is misleading
- **Confidence**: medium | **Coverage**: uncovered

### [NOTE] Hover state is Rust-only addition — **FIXED**
- **Fix**: Removed hover field, update_hover, is_hovered. Face color always ButtonBgColor.

### [NOTE] Click() API: no shift parameter, no enabled check, no EOI signal
- **C++**: Click(bool shift) — gates on IsEnabled(), fires EOI when !shift && !NoEOI
- **Rust**: click() — just invokes callback unconditionally
- **Confidence**: high | **Coverage**: uncovered

## Summary

| Severity | Count |
|----------|-------|
| BUG | 3 |
| SUSPECT | 2 |
| GAP | 7 |
| NOTE | 2 |

## Most Critical

1. **hit_test() vs check_mouse() mismatch** — actual input dispatch uses wrong formula
2. **Keyboard handling diverges** — Space added, press/release visual state differs
3. **No modifier/extent/enabled guards** — multiple missing input safety checks

## Cross-cutting: CC-01 (code duplication), CC-03 (disabled state), CC-04 (min extent)
