# TextField Audit Report

**Date**: 2026-03-18
**Agent**: Batch 3
**C++ files**: emTextField.cpp (1847 LOC) + emTextField.h (427 LOC) = 2274 LOC
**Rust file**: text_field.rs (3378 LOC) — largest widget in the codebase

## Findings: 18 total

### [HIGH] Selection model: anchor-based vs start/end indexed
- C++ tracks `SelectionStartIndex`/`SelectionEndIndex` (always ordered) + `CursorIndex`
- Rust uses `selection_anchor: Option<usize>` derived from min/max of anchor + cursor
- Missing publish-to-clipboard-on-select. `EmptySelection()` vs `deselect()` differs.
- `ModifySelection` uses closest-endpoint logic in C++; Rust uses extend boolean
- **Confidence**: high | **Coverage**: partially covered

### [HIGH] Undo/redo architecture completely different
- **C++**: Incremental edits `(pos, removeLen, insertText)` with positional merge, MAX_UNDOS=200
- **Rust**: Full text snapshots `(text, cursor)` with MAX_UNDO=100
- C++ Undo selects the undone text (MF_SELECT); Rust Undo clears selection
- O(edit-size) per entry (C++) vs O(text-length) per entry (Rust)
- **Confidence**: high | **Coverage**: uncovered

### [HIGH] Backspace/Delete modifier handling more permissive — **FIXED**
- C++ plain Backspace requires `IsNoMod()` — no modifiers allowed
- **Fix**: Added `!alt && !meta && (!shift || ctrl)` guard on both Backspace and Delete
- **Confidence**: medium | **Coverage**: partially covered

### [HIGH] Ctrl+Left/Right calls wrong word-boundary function — **FIXED**
- C++ uses `GetPrevWordIndex`/`GetNextWordIndex`
- **Fix**: Replaced prev/next_word_boundary with prev/next_word_index in all 4 Ctrl+word ops
- **Confidence**: medium | **Coverage**: covered (widget_textfield_cursor_nav)

### [MEDIUM] Tab rendering in multi-line not expanded during paint
- C++ processes tabs char-by-char: `col=(col+7)&~7`, paints segments between tabs
- Rust splits on '\n' and paints each row as a single string — tabs not expanded
- `calc_total_cols_rows` correctly handles tabs for width, but paint doesn't use tab-expanded positioning
- **Confidence**: high | **Coverage**: uncovered

### [MEDIUM] Overwrite mode doesn't expand cols count for cursor — **FIXED**
- C++ increments `cols` when cursor is at last column in overwrite mode + focused
- **Fix**: Both paint paths increment cols matching C++ emTextField.cpp:920-922
- **Confidence**: high | **Coverage**: uncovered

### [MEDIUM] Double-click word selection differs on delimiters — **FIXED**
- C++ double-click on delimiter selects from delimiter to next word boundary (always non-empty)
- **Fix**: Added prev_word_boundary_index, updated double-click and drag-by-words to use boundary-based segment selection
- **Confidence**: high | **Coverage**: uncovered

### [MEDIUM] DM_MOVE: no live drag feedback
- C++ continuously moves selected text to drag position on every mouse motion event
- Rust does nothing during drag, applies move only on mouse release
- UX regression: no real-time visual feedback during text drag
- **Confidence**: high | **Coverage**: uncovered

### [MEDIUM] Ctrl+A doesn't publish selection to clipboard — **FIXED**
- C++ `SelectAll(true)` publishes to clipboard
- **Fix**: Added publish_selection() after select_all() in Ctrl+A handler
- **Confidence**: high | **Coverage**: uncovered

### [MEDIUM] Password mode paints as single string vs individual chars
- C++ paints each `*` individually at column positions
- Rust creates `"*".repeat(n)` and paints as single string
- Cumulative rounding differences possible between N individual chars vs one N-char string
- **Confidence**: medium | **Coverage**: uncovered

### [MEDIUM] Selection polygon uses measured text width vs column grid
- C++ computes highlight positions as `tx + col * cw` (monospace column grid)
- Rust uses `Painter::measure_text_width` (actual pixel measurement)
- Same result for monospace fonts; could diverge for variable-width
- **Confidence**: medium | **Coverage**: covered

### [MEDIUM] Ctrl+Shift+A doesn't clear clipboard selection
- C++ `EmptySelection()` clears clipboard via `Clipboard->Clear(true,SelectionId)`
- Rust `deselect()` only sets `selection_published = false`
- **Confidence**: medium | **Coverage**: uncovered

### [MEDIUM] Disabled state color blending absent (see CC-03)

### [LOW] Undo select-after-undo behavior
- C++ highlights restored text after undo; Rust clears selection
- **Confidence**: high | **Coverage**: uncovered

### [LOW] Validation model differs
- C++ validation is pre-edit hook (can modify position/length/text)
- Rust validation is post-edit boolean (accept/reject only)
- C++ subclasses can do auto-formatting; Rust cannot
- **Confidence**: high | **Coverage**: uncovered

### [LOW] max_length is Rust-only addition (not a divergence)
### [LOW] Home/End in single-line (no divergence found)
### [LOW] GetRowEndIndex (no divergence found)

## Summary

| Severity | Count |
|----------|-------|
| HIGH | 4 |
| MEDIUM | 9 |
| LOW | 5 |

## Most Critical

1. **Undo/redo architecture** — completely redesigned, visible behavioral differences (select-after-undo)
2. **Selection model** — anchor vs start/end, clipboard publishing missing
3. **Tab rendering** — not expanded during multi-line paint
4. **Word boundary function** — keyboard nav calls wrong function
5. **Backspace modifier handling** — too permissive
6. **Double-click on delimiters** — selects empty range instead of word boundary
7. **Drag-move** — no live visual feedback (UX regression)

## Recommended Tests
- Undo/redo visual state (does undone text get selected?)
- Tab character rendering in multi-line
- Double-click on spaces/delimiters
- Shift+Backspace behavior
- Ctrl+Left/Right on "hello  .  world" edge cases
- Password mode pixel comparison
- Drag-move visual feedback

## Overall: Functional reimplementation that captures major behaviors but with significant architectural differences in undo/redo and selection. Paint pipeline geometry is faithful for common cases. Interaction layer has the most divergences.
