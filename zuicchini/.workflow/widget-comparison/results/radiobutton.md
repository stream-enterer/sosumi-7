# RadioButton + RadioBox Audit Report

**Date**: 2026-03-18
**Agent**: Batch 2 (partial extraction — agent ran long)
**C++ files**: emRadioButton (520 LOC) + emRadioBox (89 LOC) = 609 LOC
**Rust files**: radio_button.rs (819 LOC) + radio_box.rs (350 LOC) = 1169 LOC

## Findings: 8 total

### [MEDIUM] RadioBox missing group registration on construction — **FIXED**
- **C++**: constructor calls `Mech->Add(this)` — increments mechanism count
- **Rust**: RadioBox::new now calls `group.borrow_mut().register()`
- **Fix**: Added `register()` call in RadioBox::new + Drop impl with `deregister()`
- **Confidence**: high | **Coverage**: uncovered (no RadioBox golden tests)

### [MEDIUM] RadioBox missing Drop implementation — **FIXED**
- **C++**: `~emRadioButton()` calls `Mech->Remove(this)` — deregisters, adjusts indices
- **Rust**: RadioBox now has Drop that calls `deregister(self.index)`
- **Note**: Does not re-index other buttons (architectural limitation of index-based design)
- **Confidence**: high | **Coverage**: uncovered

### [MEDIUM] RadioButton Drop doesn't re-index or adjust selection — **PARTIALLY FIXED**
- **C++**: `RemoveByIndex` decrements CheckIndex if removed button before checked, clears if checked
- **Rust**: Drop now calls `deregister(self.index)` which clears selection if this button was selected
- **Remaining**: Does not re-index other buttons or decrement selection index for buttons after the dropped one (requires back-references that Rust design doesn't have). Use `remove_by_index` + manual `set_index` for ordered removal.
- **Confidence**: high | **Coverage**: uncovered

### [LOW] RadioButton face color changes on press (C++ doesn't) — **FIXED**
- **Fix**: Face color always ButtonBgColor. Pressed visual from overlay only.

### [LOW] Missing Enter key input (see CC-01 pattern) — **FIXED**
- **Fix**: Added Enter alongside Space in RadioButton and RadioBox input handlers.

### [LOW] Missing modifier key checks on mouse input — **FIXED**
- **Fix**: Added ctrl/alt/meta rejection in RadioButton and RadioBox mouse press.

### [LOW] RadioGroup::select() bypasses set_check_index guards — **FIXED**
- **Fix**: Added no-change early return in select().

### [INFO] RadioBox hit_test uses wrong geometry (see CC-06) — **FIXED**
- Uses content_round_rect instead of content_rect + r=h*0.2

## Summary

| Severity | Count |
|----------|-------|
| MEDIUM | 3 |
| LOW | 4 |
| INFO | 1 |

## Most Critical
1. **RadioBox doesn't register in group** — RadioBox selection is broken by design
2. **RadioButton Drop doesn't re-index** — removing a button corrupts the group's selection state
3. **RadioBox has no Drop** — leaks registration

## Size Asymmetry
Same pattern as CheckBox (CC-01): Rust flattens C++ inheritance into standalone widgets, duplicating paint/input/toggle logic. RadioButton is 819 LOC because it reimplements all of emButton + emCheckButton + emRadioButton paint/input/state.
