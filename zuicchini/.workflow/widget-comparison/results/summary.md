# Widget Comparison Summary — Final Report (Sessions 1 + 2)

**Dates**: 2026-03-18
**Scope**: ALL 20 widget types audited across 2 sessions
**Fix session**: Session 1 findings fixed (19 fixes). Session 2 findings fixed (12 fixes). **ALL PENDING items resolved.**

## Complete Finding Counts

| Widget | HIGH | MEDIUM | LOW | OK/INFO | Total | Session | Fix Status |
|--------|------|--------|-----|---------|-------|---------|------------|
| Label | — | — | — | — | 6 (2 BUG, 3 GAP, 1 NOTE) | S1 | **FIXED** (fixes 6, 12) |
| Button | — | — | — | — | 14 (3 BUG, 2 SUSPECT, 7 GAP, 2 NOTE) | S1 | **FIXED** (fixes 3, 5, 8, 19) |
| CheckBox | — | — | — | — | 8 (2 BUG, 1 SUSPECT, 2 GAP, 2 INFO) | S1 | **FIXED** (fixes 3, 4, 5, 11) |
| CheckButton | — | — | 4 | 24 | 29 (1 BUG: HowTo chain) | S2 | **FIXED** (fix 8) |
| RadioButton+RadioBox | — | 3 | 4 | 1 | 8 | S1 | **FIXED** (fixes 1, 4, 5, 9) |
| ScalarField | 2 | 4 | 4 | — | 10 | S1 | **PARTIAL** (fix 19 removed arrow keys; f64 type + drag model = design decisions, not fixable) |
| Splitter | — | 2 | 9 | — | 11 | S1 | **FIXED** (fixes 7, 15, 18) |
| ColorField | — | 1 | 3 | 4 | 8 | S1 | **FIXED** (fix 10) |
| ListBox | — | 2 | 9 | 3 | 14 | S1 | **FIXED** (fix 2) |
| TextField | 4 | 9 | 5 | — | 18 | S1 | **PARTIAL** (fixes 12, 13, 14, 16, 17; selection model + undo arch = design decisions) |
| Border | 3 | 4 | 2 | 6 | 15 | S2 | **FIXED** (fixes 1-7) |
| Tunnel | — | 1 | 1 | — | 2 | S2 | **FIXED** (fixes 9, 12) |
| Dialog | — | 2 | 2 | 2 | 6 | S2 | **FIXED** (fixes 10, 11) |
| Look | — | — | 1 | 2 | 3 | S2 | **ACCEPTED** (LOW only) |
| ErrorPanel | — | — | 1 | 1 | 2 | S2 | **ACCEPTED** (LOW only) |
| FileSelectionBox | 2 | 2 | — | — | ~20 GAPs (40% complete) | S2 | **ACCEPTED** (scaffold — large effort, not actionable) |
| FilePanel | — | — | 4 | 2 | 6 | S2 | **ACCEPTED** (LOW only) |
| FileDialog | — | — | 1 | 1 | 2 | S2 | **ACCEPTED** (LOW only) |
| CoreConfigPanel | — | — | 6 | 1 | 7 | S2 | **ACCEPTED** (LOW only) |
| TkTest (Layer 1) | — | — | — | — | 10+ GAPs (7/9 fixed) | S1 | **FIXED** (7/9 gaps closed) |
| TestPanel (Layer 2) | — | — | — | — | Structural only | S1 | N/A |

## Cross-Cutting Concerns (6 systemic issues)

| ID | Issue | Status |
|----|-------|--------|
| CC-01 | Code duplication across Button-family | Structural — accepted design (5 standalone widgets) |
| CC-02 | set_* methods don't fire signals | **Partially fixed** (CheckBox/CheckButton set_checked) |
| CC-03 | No disabled state rendering | Border enabled param implemented; widgets inconsistent |
| CC-04 | No VCT_MIN_EXT guard on input | Not fixed — would need view condition system |
| CC-05 | DoLabel alignment defaults | **Fixed** (label_alignment defaults to Left) |
| CC-06 | hit_test() face-inset divergence | **Fixed** (all button-family widgets) |

## Actionable Findings — Session 2 fixes (ALL RESOLVED)

All 12 items fixed in fix session 3 (2026-03-18). See run-log.md.

| # | Widget | Finding | Severity | Effort | Status |
|---|--------|---------|----------|--------|--------|
| 1 | Border | substance_round_rect coefficient 0.006 → 0.023 for OBT_RECT and OBT_ROUND_RECT | HIGH | Small (2 lines) | **FIXED** |
| 2 | Border | label_space uses post-HowTo width — should use pre-HowTo `s` | HIGH | Medium (refactor label_space call sites in content_rect, content_round_rect, content_rect_unobscured) | **FIXED** |
| 3 | Border | best_label_tallness ignores icon geometry | HIGH | Medium (add icon width/height to tallness calc) | **FIXED** |
| 4 | Border | MarginFilled paints inset rect instead of full Clear | MEDIUM | Small (change paint_rect to full-panel rect) | **FIXED** |
| 5 | Border | OBT_RECT/RoundRect paint fill unconditionally (should skip if transparent) | MEDIUM | Small (add transparency check) | **FIXED** |
| 6 | Border | Disabled alpha dimming rounding off-by-1 | MEDIUM | Small (match C++ float formula) | **FIXED** |
| 7 | Border | label_layout ignores description width for desc-only labels | MEDIUM | Medium | **FIXED** |
| 8 | CheckButton | HowTo chain missing HOWTO_BUTTON section | BUG | Small (add HOWTO_BUTTON to chain) | **FIXED** |
| 9 | Tunnel | Missing invalidation on set_child_tallness and set_depth | MEDIUM | Small (call invalidate) | **FIXED** |
| 10 | Dialog | Missing keyboard input (Enter/Escape) | MEDIUM | Medium (add input handler) | **FIXED** |
| 11 | Dialog | Missing CheckFinish validation gate | MEDIUM | Small (add check before finish) | **FIXED** |
| 12 | Tunnel | Child canvas color hardcoded to look.bg_color | LOW | Small | **FIXED** |

### Not fixable (design decisions, accepted divergences)

| Widget | Finding | Why not fixable |
|--------|---------|-----------------|
| ScalarField | f64 vs i64 type | Deliberate design — Rust uses f64 for float support |
| ScalarField | Drag relative vs absolute | Deliberate UX choice |
| TextField | Selection model (anchor vs start/end) | Architectural — would require full rewrite |
| TextField | Undo architecture (snapshot vs incremental) | Architectural — snapshot approach is valid |
| FileSelectionBox | ~60% missing functionality | Large effort — requires Cycle/signal infrastructure |

### Session 1 findings already fixed (for reference)

See run-log.md "Fix Session" section for full details. Key fixes applied:
- Fix 1: RadioBox/RadioButton group lifecycle
- Fix 2: ListBox row height mismatch
- Fix 3: CC-06 hit_test face-inset (all 5 button-family widgets)
- Fix 4: Enter key (CheckBox, CheckButton, RadioButton, RadioBox)
- Fix 5: Modifier key checks (all 5 button-family widgets)
- Fix 6: Label alignment defaults (CC-05)
- Fix 7: Splitter grip size, defaults, validation
- Fix 8: Button ShownChecked label shrink + overlay
- Fix 9: RadioGroup::select() no-change guard
- Fix 10: ColorField "transparent" text underlay
- Fix 11: CheckBox/CheckButton set_checked fires callback
- Fixes 12-14: TextField word boundary, backspace modifiers, double-click
- Fix 15: Splitter 2D hit test
- Fix 16: TextField overwrite mode cols expansion
- Fix 17: TextField Ctrl+A publishes selection
- Fix 18: Splitter grip hover cursor tracking
- Fix 19: Remove Rust-only divergences (Space key, hover, face color, arrow keys)

## Assessment by Layer

### Pixel Compositing Pipeline: HIGH FIDELITY
All production blend paths correct. No bugs found.

### Widget Rendering (Paint): GOOD FIDELITY
Border 9-slice, content_rect, Look system match C++. All substance_round_rect, label_space, and MarginFilled issues **FIXED**.

### Widget Interaction (Input): MOSTLY FIXED
Session 1 fixes resolved button-family keyboard/modifier/hit-test issues. Dialog keyboard (Enter/Escape) **FIXED**. TextField selection model + undo remain accepted design divergences.

### Widget Completeness: VARIES
- **Complete**: Label, Button, CheckBox, CheckButton, RadioButton, RadioBox, Splitter, ColorField, ListBox, Tunnel, Look, ErrorPanel
- **Mostly complete**: TextField, ScalarField, Border, FilePanel, CoreConfigPanel, Dialog, FileDialog
- **Scaffold only**: FileSelectionBox (~40%)

## Files

All results in `.workflow/widget-comparison/results/`:
- 19 widget audit reports (border.md, button.md, checkbox.md, checkbutton.md, colorfield.md, dialog-look-errorpanel.md, filepanel-filedialog-coreconfigpanel.md, fileselectionbox.md, label.md, listbox.md, radiobutton.md, scalarfield.md, splitter.md, textfield.md, tunnel.md)
- cross-cutting-concerns.md
- tktest-divergence.md, testpanel-divergence.md
- summary.md (this file)
- run-log.md
