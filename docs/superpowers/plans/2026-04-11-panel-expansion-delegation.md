# Panel Expansion Delegation Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix ColorFieldPanel and ListBoxPanel in test_panel.rs to delegate auto_expand/LayoutChildren to their inner widgets, closing the panel tree expansion gap vs C++.

**Architecture:** Add `auto_expand()` and `LayoutChildren()` methods to two PanelBehavior impls, plus `SetAutoExpansionThreshold` calls for color fields. Pattern identical to commit 34b46c5 which fixed the same wrappers in composition.rs.

**Tech Stack:** Rust, emcore panel tree, golden tests

---

### Task 1: Add expansion delegation to ColorFieldPanel

**Files:**
- Modify: `crates/eaglemode/tests/golden/test_panel.rs:259-270`

- [ ] **Step 1: Add auto_expand and LayoutChildren to ColorFieldPanel**

In `test_panel.rs`, add two methods to the `impl PanelBehavior for ColorFieldPanel` block (after the `IsOpaque` method at line 269):

```rust
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
```

- [ ] **Step 2: Add SetAutoExpansionThreshold for each color field**

In `TkTestPanel::create_all_categories()`, after each `set_behavior` call for cf1/cf2/cf3 (lines 1325, 1332, 1342), add the threshold. After line 1325:

```rust
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);
```

After line 1332 (which will shift down by 2 lines after the first insertion):

```rust
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);
```

After line 1342 (shifted down by 4 lines):

```rust
            ctx.tree
                .SetAutoExpansionThreshold(id, 9.0, ViewConditionType::MinExt);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: success, no errors

---

### Task 2: Add expansion delegation to ListBoxPanel

**Files:**
- Modify: `crates/eaglemode/tests/golden/test_panel.rs:275-286`

- [ ] **Step 1: Add auto_expand and LayoutChildren to ListBoxPanel**

In `test_panel.rs`, add two methods to the `impl PanelBehavior for ListBoxPanel` block (after the `IsOpaque` method at line 284):

```rust
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: success, no errors

---

### Task 3: Verify and commit

**Files:**
- Modified: `crates/eaglemode/tests/golden/test_panel.rs`

- [ ] **Step 1: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings, no errors

- [ ] **Step 2: Run golden test to measure op count change**

Run:
```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1 2>&1 | tail -5
```

Then compare ops:
```bash
python3 -c "
import json
cpp = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.cpp_ops.jsonl') if l.strip().startswith('{')]
rust = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.rust_ops.jsonl') if l.strip().startswith('{')]
print(f'C++ ops: {len(cpp)}, Rust ops: {len(rust)}')
from collections import Counter
cc = Counter(o['op'] for o in cpp)
rc = Counter(o['op'] for o in rust)
for op in sorted(set(list(cc.keys()) + list(rc.keys()))):
    if cc[op] != rc[op]:
        print(f'  {op}: C++={cc[op]} Rust={rc[op]} delta={cc[op]-rc[op]}')
"
```

Expected: Rust ops should increase significantly from 2322 (closer to C++ 5470). PaintRect and PaintText gaps should shrink.

- [ ] **Step 3: Run full test suite**

Run: `cargo-nextest ntr`
Expected: no new failures (13 pre-existing failures OK)

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/golden/test_panel.rs
git commit -m "fix: add expansion delegation to ColorFieldPanel and ListBoxPanel in test_panel.rs

Match composition.rs fix from 34b46c5. ColorField panels get
auto_expand + LayoutChildren + SetAutoExpansionThreshold(9.0, MinExt).
ListBox panels get auto_expand + LayoutChildren with create_item_children.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
