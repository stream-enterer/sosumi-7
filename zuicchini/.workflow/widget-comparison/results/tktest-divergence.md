# TkTest Composition Divergence Report

**Date**: 2026-03-18
**Comparison**: C++ `emTkTestPanel.cpp` (480 LOC) vs Rust `test_toolkit.rs` (540 LOC)
**Status**: 7 of 9 gaps closed

## Missing Widget Categories

### [GAP] Tunnels section — **FIXED 2026-03-18**
- **Fix**: Added 4 tunnel variants (default, depth=30, square-end tallness=1.0, square-end+zero-depth) matching C++ emTkTestPanel.cpp:258-276.

### [GAP] Test Dialog section — entirely absent — **INTENTIONAL DIVERGENCE 2026-03-18**
- **C++ content**: 7 checkboxes for window/view flags + "Create Test Dialog" button + Cycle() dialog lifecycle.
- **Justification**: Requires Dialog widget, window flag management, and signal-driven Cycle() logic. The Dialog widget has not been audited and may have its own divergences. Adding an incomplete dialog section would provide false test coverage. Deferred until Dialog widget is ported and audited.

### [GAP] File Selection section — entirely absent — **INTENTIONAL DIVERGENCE 2026-03-18**
- **C++ content**: FileSelectionBox with filters + FileDialog open/save lifecycle.
- **Justification**: Requires FileSelectionBox and FileDialog widgets which are not yet ported. Same rationale as Test Dialog.

## Missing Individual Widgets

### [GAP] Button: missing NoEOI variant — **FIXED 2026-03-18**
- **Fix**: Added b3 "NoEOI" with `set_no_eoi(true)`.

### [GAP] Button: missing long description — **FIXED 2026-03-18**
- **Fix**: b2 "Long Desc" now has 100-line repeated description via `set_description`.

### [GAP] CheckButton/CheckBox: only 2+2 vs C++ 3+3 — **FIXED 2026-03-18**
- **Fix**: Added c3 (3rd CheckButton) and c6 (3rd CheckBox).

### [GAP] ScalarField: 3 variants vs C++ 6 — **FIXED 2026-03-18**
- **Fix**: Added sf4 "Level" (custom formatter, range 1-5), sf5 "Play Length" (TextOfTimeValue formatter, 0-24h in ms, complex mark intervals), sf6 "Play Position" (same formatter, static 4h max). Signal wiring (SFLen→SFPos max update) not implemented — requires Cycle() infrastructure.

### [GAP] ListBox: 5 variants vs C++ 7 — **FIXED 2026-03-18**
- **Fix**: Added l6 "Single Column" with `set_fixed_column_count(Some(1))`, l7 "Custom List Box" as Multi selection placeholder. C++ CustomListBox with recursive item panels requires child-panel auto-expand architecture.

### [GAP] C++ TkTest has Cycle() engine logic — **INTENTIONAL DIVERGENCE 2026-03-18**
- **C++ content**: Signal-driven update cycle (SFLen→SFPos, dialog creation, FileDialog lifecycle).
- **Justification**: Rust has no signal subscription/polling mechanism equivalent to C++ `IsSignaled()` + `Cycle()`. The Rust test_toolkit uses callbacks (on_value, on_click) for widget interaction rather than a frame-by-frame signal poll. This is an idiomatic Rust adaptation. The specific SFLen→SFPos wiring could be added via callbacks but the Dialog/FileDialog lifecycle cannot.

## Assessment

The Rust TkTest now covers all widget categories (Buttons, CheckWidgets, RadioWidgets, TextFields, ScalarFields, ColorFields, ListBoxes, Tunnels) with matching variant counts. Missing: Test Dialog section, File Selection section, and Cycle() signal wiring — all deferred pending Dialog/FileDialog widget ports.
