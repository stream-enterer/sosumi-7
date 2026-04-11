# Panel Expansion Delegation Fix — Design Spec

**Date**: 2026-04-11
**Status**: Approved

## Problem

The `test_panel.rs` wrapper panels for container widgets (ColorFieldPanel, ListBoxPanel) don't delegate `auto_expand()` or `LayoutChildren()` to their inner widgets. This prevents the panel tree from expanding during `settle()`. Commit 34b46c5 fixed the same wrappers in `composition.rs`, but `test_panel.rs` has its own copies that remain broken.

Result: tktest_1x produces 2322 Rust ops vs 5470 C++ ops. The dominant gap is 3222 missing PaintRect and 645 missing PaintText — consistent with entire widget subtrees not being created.

## Root Cause

Container widgets (emColorField, emListBox) implement auto-expansion internally — they create child panels (color sliders, list items) when the view condition exceeds a threshold. The wrapper panels in `test_panel.rs` don't delegate these methods, so the panel tree stays shallow.

## Audit Results

All 10 wrapper panel types in `test_panel.rs` were checked against their C++ widget implementations:

| Panel | Inner Widget | Container? | Needs Fix |
|-------|-------------|-----------|-----------|
| ButtonPanel | emButton | No | No |
| CheckButtonPanel | emCheckButton | No | No |
| CheckBoxPanel | emCheckBox | No | No |
| RadioButtonPanel | emRadioButton | No | No |
| RadioBoxPanel | emRadioBox | No | No |
| TextFieldPanel | emTextField | No | No |
| ScalarFieldPanel | emScalarField | No | No |
| **ColorFieldPanel** | emColorField | **Yes** | **Yes** |
| **ListBoxPanel** | emListBox | **Yes** | **Yes** |
| SplitterPanel | emSplitter | Layout-only | No (already delegates LayoutChildren) |

## Fix

### ColorFieldPanel (test_panel.rs:256)

Add `auto_expand()` and `LayoutChildren()` delegation, matching the composition.rs pattern from commit 34b46c5:

```rust
fn auto_expand(&self) -> bool { true }
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    if ctx.children().is_empty() {
        self.widget.create_expansion_children(ctx);
    }
    let rect = ctx.layout_rect();
    self.widget.LayoutChildren(ctx, rect.w, rect.h);
}
```

### ListBoxPanel (test_panel.rs:272)

```rust
fn auto_expand(&self) -> bool { true }
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    if ctx.children().is_empty() {
        self.widget.create_item_children(ctx);
    }
    let rect = ctx.layout_rect();
    self.widget.layout_item_children(ctx, rect.w, rect.h);
}
```

### Auto-expansion thresholds

ColorField panels need `SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt)` matching C++ `emColorField.cpp:36`. This must be set in `TkTestPanel::create_all_categories()` after each `set_behavior()` call for color fields (lines 1325, 1332, 1342).

ListBox panels use the default threshold from emRasterGroup inheritance — no explicit threshold needed.

## Non-scope

- SplitterPanel already delegates LayoutChildren (L304) and emSplitter doesn't auto-create children
- emFileSelectionBox is used directly (not via a wrapper panel) in TkTestPanel
- Recording format differences (ClipRect, SetCanvasColor ops present in Rust but not C++) are a separate issue
- Remaining pixel-level divergences from canvas_color/clip bugs are pre-existing

## Verification

1. `DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1`
2. Op count comparison via `python3` script
3. Full test suite: `cargo-nextest ntr`
4. `cargo clippy -- -D warnings`
