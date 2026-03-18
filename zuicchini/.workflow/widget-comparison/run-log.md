# Widget Comparison Run Log

## 2026-03-18 — Session 1: Initial Dispatch

### Strategy

Three-layer approach, bottom-up:
1. **Layer 0 — Individual widgets**: Compare each em* class against its Rust port in isolation
2. **Layer 1 — TkTest compositions**: Compare widget compositions in test toolkit
3. **Layer 2 — TestPanel full integration**: Full panel tree with all widgets composed

### Calibration Batch (complete)

| Time | Widget | Status | BUG | SUSPECT | GAP | NOTE |
|------|--------|--------|-----|---------|-----|------|
| 10:58 | Label | DONE | 2 | 0 | 3 | 1 |
| 10:58 | Button | DONE | 3 | 2 | 7 | 2 |
| 10:58 | CheckBox | DONE | 2 | 1 | 2 | 1 |

**Total calibration findings**: 7 BUG, 3 SUSPECT, 12 GAP, 4 NOTE = 26 findings

### Calibration Assessment

Quality of subagent analysis: **HIGH**. Agents were thorough, read all relevant files, traced through C++ logic carefully, correctly identified alignment and hit-test bugs that are masked by golden tests. No false positives from rosetta-stone patterns. Good confidence calibration.

Key cross-cutting findings (see cross-cutting-concerns.md):
- CC-01: Button-family code duplication (fixes don't propagate)
- CC-02: set_* methods don't fire signals
- CC-03: No disabled state rendering across all widgets
- CC-04: No VCT_MIN_EXT guard on input

### Layer 1 Finding (manual)

TkTest composition divergence documented in results/tktest-divergence.md. Missing: Tunnels section, Test Dialog section, File Selection section, several individual widget variants (NoEOI button, custom scalar formatters, custom list box, single-column list).

### Batch 2 (complete)

| Time | Widget | Status | Findings |
|------|--------|--------|----------|
| 11:10 | RadioButton+RadioBox | DONE | 3 MEDIUM, 4 LOW, 1 CC — RadioBox group registration broken, Drop doesn't re-index |
| 11:10 | ScalarField | DONE | 2 HIGH, 4 MEDIUM, 4 LOW — f64 vs i64, absolute vs relative drag |
| 11:10 | Splitter | DONE | 2 MEDIUM, 9 LOW — drag math edge case, missing hover tracking |
| 11:10 | ColorField | DONE | 1 MEDIUM, 3 LOW, 4 CC — missing "transparent" text underlay |

### Batch 3 (complete)

| Time | Widget | Status | Findings |
|------|--------|--------|----------|
| 11:30 | TextField | DONE | 4 HIGH, 9 MEDIUM, 5 LOW — undo architecture, selection model, tab rendering, word boundary |
| 11:30 | ListBox | DONE | 2 MEDIUM, 9 LOW, 3 INFO — row height mismatch, arrow key addition, HowTo truncation |
| — | Border | Pending | Next session — highest remaining priority |

### Session 1 Complete

**Grand total**: ~107 findings across 9 widgets + 2 composition layers
**Result files**: 14 reports in `.workflow/widget-comparison/results/`
**Key outcome**: Pixel compositing pipeline is HIGH FIDELITY. Widget interaction layer has significant divergences, especially in TextField (undo, selection) and ScalarField (type, drag model).

### Future Batches

| Widget | Status | Notes |
|--------|--------|-------|
| ColorField | pending | Tier 2 |
| FileSelectionBox | pending | Tier 2 — inverse size asymmetry |
| CheckButton | pending | Tier 2 — needs CC-01 analysis |
| Dialog | pending | Tier 3 — size asymmetry, no golden tests |
| Look | pending | Tier 3 |
| Tunnel | pending | Tier 3 |
| FilePanel | pending | Tier 3 |
| FileDialog | pending | Tier 3 |
| ErrorPanel | pending | Tier 3 |
| CoreConfigPanel | pending | Tier 3 |

## 2026-03-18 — Fix Session: RadioButton/RadioBox Group Lifecycle

### Fix 1: RadioBox/RadioButton group lifecycle (findings #10, #11)

**Findings addressed**:
- #10: RadioBox doesn't register in group on construction
- #11: RadioButton Drop doesn't adjust selection
- (Implicit) RadioBox has no Drop impl

**Root cause**: RadioBox::new didn't call `group.register()`, RadioButton::Drop only decremented count without clearing stale selection, RadioBox had no Drop at all.

**Changes**:
- `radio_button.rs`: Added `register()` and `deregister(index)` methods to RadioGroup. Changed RadioButton::new to use `register()`. Changed RadioButton::Drop to use `deregister(self.index)` (clears selection if this button was selected).
- `radio_box.rs`: Added `register()` call in RadioBox::new. Added Drop impl using `deregister(self.index)`.

**Scope limitation**: Does NOT re-index other buttons on drop (C++ does via back-references in the Mechanism array; Rust's index-based design can't). Callers needing ordered removal should use `remove_by_index` + manual `set_index`. This matches actual usage patterns (buttons created/destroyed together).

**Tests**: cargo clippy clean, 1137/1137 tests pass (including all golden tests).

### Fix 2: ListBox row height mismatch (finding #12, LB-05)

**Finding addressed**: #12 — Hit test vs paint row height mismatch

**Root cause**: Paint used `ch / items.len()` (dynamic), input/scroll used constant `ROW_HEIGHT=17.0`. When the widget's content height doesn't equal `items.len() * 17.0`, clicks land on wrong items.

**Changes**:
- `list_box.rs`: Added `row_height()` helper that returns `visible_height / items.len()` (matching paint) with fallback to `ROW_HEIGHT` when empty or before first paint. Used it in click handler and `scroll_to_index`.

**Tests**: cargo clippy clean, 1137/1137 tests pass.

### Fix 3: CC-06 hit_test() face-inset divergence (all button-family widgets)

**Finding addressed**: CC-06 — hit_test() vs check_mouse() face-inset divergence

**Root cause**: All button-family widgets used `content_round_rect` in their `hit_test()` methods, but C++ `emButton::CheckMouse` tests against the face rect (which is inset from the content rect). This made the clickable area slightly larger than C++.

**Changes (non-boxed path — Button, RadioButton, CheckButton)**:
- Applied face inset: `d = (14/264) * r`, test against `(cr.x+d, cr.y+d, cr.w-2d, cr.h-2d)` with `fr = r-d`
- Also applied `r = max(r, min(w,h) * border_scaling * 0.223)` clamp matching paint path

**Changes (boxed path — CheckBox, RadioBox)**:
- Changed from `content_round_rect` to `content_rect` with `r = h * 0.2`
- Matches C++ emButton.cpp:276: explicit `r=h*0.2` on content rect for boxed hit test

**Files**: button.rs, radio_button.rs, check_button.rs, check_box.rs, radio_box.rs

**Tests**: cargo clippy clean, 1137/1137 tests pass.

### Fix 4: Enter key support for CheckBox, CheckButton, RadioButton, RadioBox

**Finding addressed**: CC-01 — Missing Enter key in check/radio widgets
**Change**: Added `InputKey::Enter |` alongside `InputKey::Space` in all four widgets.

### Fix 5: Modifier key checks on mouse press (all 5 button-family widgets)

**Finding addressed**: Button [GAP] "No modifier key checks", RadioButton [LOW] same
**Change**: Added `if event.ctrl || event.alt || event.meta { return false; }` gate before hit test in Button, CheckButton, CheckBox, RadioButton, RadioBox.

### Fix 6: Label alignment defaults (CC-05 for Label widget)

**Finding addressed**: Label [BUG] horizontal centering, [BUG] text line alignment
**Change**: Removed `cx += (cw - w2) * 0.5` centering. Changed text_alignment from Center to Left.

### Fix 7: Splitter grip size, defaults, and validation

**Finding addressed**: Splitter [MEDIUM] drag math, [LOW] defaults, [LOW] set_limits validation
**Changes**: Drag uses capped grip size from calc_grip_rect. Defaults changed to 0.0/1.0 matching C++. set_limits clamps to [0,1] and averages if min > max.

### Fix 8: Button ShownChecked label shrink and overlay

**Finding addressed**: Button [BUG] label shrink missing for checked, [BUG] missing checked overlay
**Change**: Added checked branch (0.983 shrink + ButtonChecked overlay) matching C++ emButton.cpp:377-409.

### Fix 9: RadioGroup::select() no-change guard

**Finding addressed**: RadioButton [LOW] select() bypasses guards
**Change**: Added early return when re-selecting already-selected button.

### Fix 10: ColorField "transparent" text underlay

**Finding addressed**: ColorField [MEDIUM] missing text underlay for non-opaque colors
**Change**: Added "transparent" text paint before color rect when alpha < 255.

### Fix 11: CheckBox/CheckButton set_checked fires callback (CC-02)

**Finding addressed**: CheckBox [BUG] SetChecked silent, CC-02
**Change**: set_checked now fires on_check when state changes.

### Fix 12: TextField Ctrl+arrow/Backspace/Delete word-index functions

**Finding addressed**: TextField [HIGH] Ctrl+Left/Right calls wrong word-boundary function
**Change**: Replaced prev/next_word_boundary with prev/next_word_index in all 4 Ctrl+word operations.

### Fix 13: TextField Backspace/Delete modifier guards

**Finding addressed**: TextField [HIGH] Backspace modifier handling too permissive
**Change**: Added `!alt && !meta && (!shift || ctrl)` guard matching C++ modifier handling.

### Fix 14: TextField double-click segment selection

**Finding addressed**: TextField [MEDIUM] Double-click on delimiters selects empty range
**Change**: Added prev_word_boundary_index, updated double-click and drag-by-words to use boundary-based segment selection matching C++.

### Fix 15: Splitter 2D hit test with exclusive upper bound

**Finding addressed**: Splitter [LOW] Hit test is 1D not 2D, [LOW] Inclusive upper bound
**Change**: Now checks both axes; changed `<=` to `<` matching C++.

### Fix 16: TextField overwrite mode cols expansion

**Finding addressed**: TextField [MEDIUM] Overwrite mode doesn't expand cols count
**Change**: Both paint paths now increment cols when overwrite mode, focused, and cursor at last column.

### Fix 17: TextField Ctrl+A publishes selection to clipboard

**Finding addressed**: TextField [MEDIUM] Ctrl+A doesn't publish selection
**Change**: Added `publish_selection()` call after `select_all()` in Ctrl+A handler.

### Fix 18: Splitter grip hover cursor tracking

**Finding addressed**: Splitter [MEDIUM] Missing MouseInGrip hover tracking
**Change**: Added mouse_in_grip field, tracked on Move events, gated get_cursor on it.

### All Fixes Summary

All 1137 tests pass after every fix. Total: 19 fixes across 12 source files.

### Notes

- Calibration batch validated methodology. Subagents are thorough and find real bugs.
- The alignment bugs in Label are systemic — they affect DoLabel which is used by ALL border-based widgets. This needs tracking as a cross-cutting concern.
- hit_test() vs check_mouse() mismatch in Button is the highest-confidence bug found so far.
- The missing input guards (modifier keys, min extent, enabled, clip rect, IsViewed) are systemic — they affect all interactive widgets. Should verify once definitively rather than repeating for each widget.
