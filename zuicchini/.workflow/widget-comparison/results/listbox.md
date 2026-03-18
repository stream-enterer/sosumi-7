# ListBox Audit Report

**Date**: 2026-03-18
**Agent**: Batch 3
**C++ files**: emListBox.cpp (1075 LOC) + emListBox.h (483 LOC) = 1558 LOC
**Rust file**: list_box.rs (1992 LOC)

## Findings: 14 total

### [MEDIUM] Arrow keys added — not in C++ — **INTENTIONAL DIVERGENCE 2026-03-18**
- **LB-03**: Rust adds explicit ArrowUp/ArrowDown with `focus_index`. C++ uses panel tree zoom-to-focus via child panels.
- **Justification**: C++ emListBox inherits emRasterGroup and creates child panels per item. Focus navigation happens via the panel tree's zoom-to-visit model. Rust ListBox paints items inline (no child panels), so there is no panel tree to navigate. Arrow keys are the minimal replacement for keyboard accessibility. Without them, the Rust ListBox has no keyboard navigation at all. The arrow behavior (move focus_index, auto-select in Single mode, scroll to visible) matches standard UI conventions and is consistent with how other Rust inline-paint widgets handle navigation.
- **Verified**: ArrowDown does not go past last item. ArrowUp does not go before first. scroll_to_index keeps the focused item visible.

### [MEDIUM] Hit test vs paint row height mismatch — **FIXED**
- **LB-05**: Input and scroll now use `row_height()` helper that matches paint's `visible_height / items.len()`. Falls back to `ROW_HEIGHT` when empty or before first paint.
- **Confidence**: high | **Coverage**: uncovered

### [LOW] add_item/insert_item don't accept data parameter (LB-01) — **FIXED**
- **Fix**: `add_item_with_data` and `insert_item_with_data` added, accepting an associated data value alongside the item label.
### [LOW] sort_items comparator lacks data access (LB-02) — **FIXED**
- **Fix**: `sort_items_with_data` added; comparator closure receives both items' data values enabling data-aware ordering.
### [LOW] focus_index concept not in C++ (LB-04) — **NOTE**
- Architectural necessity of the inline-paint model: without C++'s panel tree zoom-to-focus, a local `focus_index` is required to track keyboard focus within the list. Not a divergence that can be removed without a larger architectural change.
### [LOW] Custom item panels can't intercept input (LB-06) — **INTENTIONAL DIVERGENCE 2026-03-18**
- C++ ItemPanelInterface::ProcessItemInput lets custom item panels receive input because items are real child panels in the panel tree. Rust ListBox paints items inline — there are no child panels to receive input. All input is centralized in ListBox::input(). Custom rendering is supported via ItemPanelInterface but custom input handling would require the full child-panel architecture. No Rust code currently uses custom item panels with input, so this gap has zero impact.
- Architecture gap: inline paint model cannot delegate input events to sub-panels. Needs a deeper input dispatch architecture before it can be addressed.
### [LOW] Inline paint row height may differ from C++ RasterGroup layout (LB-08) — **NOTE**
- Inline paint uses dynamic `row_height` matching the content area height divided by item count, which mirrors how C++ RasterGroup lays out equal-height children. No behavioral divergence in practice.
### [LOW] canvasColor for text computed locally vs chained (LB-09) — **NOTE**
- Local computation (background color from look) produces the same result as C++'s border chain for the standard look. No pixel divergence observed.
### [LOW] prev_input_index adjustment correct but fragile (LB-12) — **NOTE**
- Implementation difference (manual index adjustment on item insertion/deletion) vs C++'s implicit panel-tree invalidation. Produces equivalent behavior; not a behavioral divergence but a structural fragility.
### [LOW] HowTo Multi mode missing keyboard section (LB-14) — **FIXED**
### [LOW] HowTo Toggle mode missing keyboard section (LB-15) — **FIXED**

### [INFO] Scroll model: traditional scrolling vs zoom-to-visit (LB-07)
### [INFO] set_items bulk replacement is Rust addition (LB-13)
### [INFO] prev_input stored as index, manually adjusted on move (LB-11)

## Summary

| Severity | Count |
|----------|-------|
| MEDIUM | 2 |
| LOW | 9 |
| INFO | 3 |

## Most Critical
1. **Row height mismatch (LB-05)** — clicks land on wrong items in non-expanded path
2. **Arrow key addition (LB-03)** — behavioral extension that may conflict with focus navigation
3. **Truncated HowTo text (LB-14, LB-15)** — easy fix, keyboard help text missing

## Overall: Good port. Core selection logic (all 4 modes), keywalk, paint pipeline faithful. Main gaps: hit test geometry, arrow key addition, HowTo truncation.
