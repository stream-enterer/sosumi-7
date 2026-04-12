# Panel Tree Expansion Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix Rust golden test panel tree expansion so it produces the same child panels as C++.

**Architecture:** Two issues cause the Rust panel tree to be incomplete:

1. **HandleNotice flag bug** (`emPanelTree.rs:1142`): C++ triggers `LayoutChildren()` on both `NF_LAYOUT_CHANGED` and `NF_CHILD_LIST_CHANGED`. Rust only triggers on `LAYOUT_CHANGED`. Fix: use `intersects()` for the combined mask. Already applied.

2. **Composition test widget wrappers don't auto-expand** (`composition.rs`): In C++, widgets like `emColorField`, `emListBox`, and `emFileSelectionBox` are panel subclasses that override `AutoExpand()` to create sub-panels (ScalarFields, item panels, etc.). In Rust, the composition test wraps these widgets in simple `PanelBehavior` structs that lack `LayoutChildren`/`auto_expand()`, so no sub-panels are created. This is the primary cause of the op count gap (Rust 1668 vs C++ 5470 for tktest_1x). The Rust tree has 72 panels (all viewed), but C++ has ~140+ because widgets create sub-panels.

**Fix approach:** Update `ColorFieldPanel` and `ListBoxPanel` in `composition.rs` to delegate expansion to the widget's existing methods (`create_expansion_children`, `create_item_children`, `LayoutChildren`). `emFileSelectionBox` already implements `PanelBehavior` directly with proper expansion.

**Tech Stack:** Rust, emCore panel tree, golden tests

---

### Task 1: HandleNotice CHILDREN_CHANGED fix (already applied)

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs:1142`

This change is already applied in the working tree. Verify it's correct:

- [ ] **Step 1: Verify the fix**

Confirm line 1142 reads:
```rust
                    if flags.intersects(NoticeFlags::LAYOUT_CHANGED | NoticeFlags::CHILDREN_CHANGED) && self.GetFirstChild(id).is_some()
```

This matches C++ emPanel.cpp:1413:
```cpp
if (flags&(NF_LAYOUT_CHANGED|NF_CHILD_LIST_CHANGED)) {
    if (FirstChild) ChildrenLayoutInvalid=1;
}
```

- [ ] **Step 2: Verify clippy passes**

Run: `cargo clippy -- -D warnings`
Expected: PASS

---

### Task 2: Add auto-expansion to ColorFieldPanel in composition.rs

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs`

In C++, `emColorField` has `SetAutoExpansionThreshold(9, VCT_MIN_EXT)` and `AutoExpand()` which creates ScalarField/TextField children. The Rust `ColorFieldPanel` wrapper needs to delegate to `emColorField::create_expansion_children` and `emColorField::LayoutChildren`.

Reference: `widget.rs:581-602` (`ColorFieldExpandedBehavior`) already does this correctly.

- [ ] **Step 1: Update ColorFieldPanel to support expansion**

In `crates/eaglemode/tests/golden/composition.rs`, replace:

```rust
struct ColorFieldPanel {
    widget: emColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}
```

With:

```rust
struct ColorFieldPanel {
    widget: emColorField,
}
impl PanelBehavior for ColorFieldPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            self.widget.create_expansion_children(ctx);
        }
        let rect = ctx.layout_rect();
        self.widget.LayoutChildren(ctx, rect.w, rect.h);
    }
}
```

- [ ] **Step 2: Set correct auto-expansion threshold for color fields**

In `create_all_categories`, after creating each ColorField panel, set the C++ threshold. Find the section that creates cf1, cf2, cf3 and add threshold setting after each `set_behavior`:

```rust
// After: ctx.tree.set_behavior(id, Box::new(ColorFieldPanel { widget: cf1 }));
ctx.tree.SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);
```

Do this for all 3 color fields (cf1, cf2, cf3).

- [ ] **Step 3: Run clippy and check compilation**

Run: `cargo clippy -- -D warnings`
Expected: PASS

---

### Task 3: Add auto-expansion to ListBoxPanel in composition.rs

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs`

In C++, `emListBox` inherits from `emRasterGroup` and has `AutoExpand()` that creates item child panels. The Rust `ListBoxPanel` wrapper needs to delegate to `emListBox::create_item_children` and `emListBox::layout_item_children`.

Reference: `widget.rs:657-678` (`ListBoxExpandedBehavior`) already does this correctly.

- [ ] **Step 1: Update ListBoxPanel to support expansion**

In `crates/eaglemode/tests/golden/composition.rs`, replace:

```rust
struct ListBoxPanel {
    widget: emListBox,
}
impl PanelBehavior for ListBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
}
```

With:

```rust
struct ListBoxPanel {
    widget: emListBox,
}
impl PanelBehavior for ListBoxPanel {
    fn Paint(&mut self, p: &mut emPainter, w: f64, h: f64, s: &PanelState) {
        let pixel_scale = s.viewed_rect.w * s.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.widget.Paint(p, w, h, pixel_scale);
    }
    fn Input(&mut self, e: &emInputEvent, _s: &PanelState, _is: &emInputState) -> bool {
        self.widget.Input(e, _s, _is)
    }
    fn IsOpaque(&self) -> bool {
        true
    }
    fn auto_expand(&self) -> bool {
        true
    }
    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        if ctx.children().is_empty() {
            self.widget.create_item_children(ctx);
        }
        let rect = ctx.layout_rect();
        self.widget.layout_item_children(ctx, rect.w, rect.h);
    }
}
```

- [ ] **Step 2: Run clippy and check compilation**

Run: `cargo clippy -- -D warnings`
Expected: PASS

---

### Task 4: Verify expansion and compare op counts

**Files:** None (verification only)

- [ ] **Step 1: Compare tktest_1x op counts**

Run:
```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 -c "
import json
cpp = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.cpp_ops.jsonl') if l.strip().startswith('{')]
rust = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.rust_ops.jsonl') if l.strip().startswith('{')]
print(f'C++ ops: {len(cpp)}, Rust ops: {len(rust)}')
"
```

Expected: Rust op count significantly higher than 1668 (closer to C++ 5470).

- [ ] **Step 2: Run all golden tests**

Run: `cargo test --test golden -- --test-threads=1 2>&1 | grep -E 'FAILED|test result'`

Expected: No NEW failures. Some pre-existing failures may improve or remain.

- [ ] **Step 3: Run full test suite**

Run: `cargo-nextest ntr`
Expected: No regressions from the changes.

---

### Task 5: Commit

- [ ] **Step 1: Commit all changes**

```bash
git add crates/emcore/src/emPanelTree.rs crates/eaglemode/tests/golden/composition.rs
git commit -m "fix: match C++ panel tree expansion in HandleNotice and composition tests

- Trigger LayoutChildren on CHILDREN_CHANGED (matching C++ NF_CHILD_LIST_CHANGED)
- Add auto_expand/LayoutChildren to ColorFieldPanel and ListBoxPanel wrappers
  to create sub-panels matching C++ AutoExpand behavior
- Set correct auto-expansion thresholds for color fields (9.0, MinExt)"
```
