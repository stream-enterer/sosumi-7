# PaintRect Gap Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 1 actionable rendering bug in tktest_1x composition test (IsOpaque canvas clear) and document the remaining op count gap as expected differences.

**Architecture:** TkTestPanel::IsOpaque incorrectly returns `true` instead of delegating to `self.border.IsOpaque()`. This causes the compositor to skip the initial canvas clear, losing 1 PaintRect op. The remaining 3221 of the 3222 "missing" PaintRects are sub-op recording differences (3156), PaintImage naming differences (14), and unported file viewer stubs (51).

**Tech Stack:** Rust, cargo-nextest, golden tests

---

### Task 1: Fix TkTestPanel::IsOpaque

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs:934-937`

- [ ] **Step 1: Fix IsOpaque to delegate to border**

In `crates/eaglemode/tests/golden/composition.rs`, change TkTestPanel's `IsOpaque` from hardcoded `true` to delegating to `self.border.IsOpaque(&self.look)`, matching C++ `emRasterGroup` which inherits `emBorder::IsOpaque`:

```rust
impl PanelBehavior for TkTestPanel {
    fn IsOpaque(&self) -> bool {
        self.border.IsOpaque(&self.look)
    }
```

Since TkTestPanel uses `OuterBorderType::Group`, `emBorder::IsOpaque` returns `false` (only `Filled`/`MarginFilled`/`PopupRoot` with opaque bg return true). This matches C++ where `emRasterGroup` inherits `emBorder::IsOpaque`.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: PASS (no warnings)

- [ ] **Step 3: Run tests**

Run: `cargo-nextest ntr`
Expected: PASS

- [ ] **Step 4: Verify op count change**

Run:
```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 -c "
import json
rust = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.rust_ops.jsonl') if l.strip().startswith('{')]
from collections import Counter
ops = Counter(o.get('op','?') for o in rust)
print(f'Total Rust ops: {len(rust)}')
print(f'PaintRect: {ops[\"PaintRect\"]}')
"
```
Expected: PaintRect count increases from 81 to 82. Total ops increases from 2558 to 2559.

- [ ] **Step 5: Run full golden test suite**

Run: `cargo test --test golden -- --test-threads=1 2>&1 | grep -E 'FAILED|test result'`
Expected: No new failures. (Existing failures from known divergences are OK.)

- [ ] **Step 6: Commit**

```bash
git add crates/eaglemode/tests/golden/composition.rs
git commit -m "fix: delegate TkTestPanel::IsOpaque to border (adds canvas clear)"
```

### Task 2: Fix remaining widget panel IsOpaque implementations

**Context:** All 9 widget wrapper panels (ButtonPanel, CheckButtonPanel, CheckBoxPanel, RadioButtonPanel, RadioBoxPanel, TextFieldPanel, ScalarFieldPanel, ColorFieldPanel, ListBoxPanel) hardcode `IsOpaque` to `true`. In C++, these inherit `emBorder::IsOpaque` which returns `false` for their border types (OBT_GROUP). While this doesn't affect the depth-0 PaintRect count (the parent's canvas clear covers the area), it's architecturally wrong and could mask future rendering bugs.

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs:140-141,159-160,178-179,197-198,216-217,236-237,260-261,276-277,302-303`

- [ ] **Step 1: Check which widget panels have border access**

The widget wrapper panels hold typed widget structs (e.g. `emButton`, `emCheckButton`). To delegate IsOpaque, each widget type needs a method to access its border's IsOpaque. Check whether the widget types expose their border:

```bash
cd /home/a0/git/eaglemode-rs
grep -n 'pub.*border\b\|fn IsOpaque\|fn is_opaque' crates/emcore/src/emButton.rs crates/emcore/src/emCheckButton.rs crates/emcore/src/emCheckBox.rs crates/emcore/src/emRadioButton.rs crates/emcore/src/emRadioBox.rs crates/emcore/src/emTextField.rs crates/emcore/src/emScalarField.rs crates/emcore/src/emColorField.rs crates/emcore/src/emListBox.rs 2>/dev/null | head -30
```

If widgets expose `pub border: emBorder` or have an `IsOpaque` method, delegate. Otherwise just change to `false` — these are all OBT_GROUP-type widgets that are never opaque.

- [ ] **Step 2: Update all widget panel IsOpaque to return false**

For each widget panel struct in `crates/eaglemode/tests/golden/composition.rs`, change `IsOpaque` from `true` to `false`:

```rust
    fn IsOpaque(&self) -> bool {
        false
    }
```

Apply to: ButtonPanel (line 140), CheckButtonPanel (line 159), CheckBoxPanel (line 178), RadioButtonPanel (line 197), RadioBoxPanel (line 216), TextFieldPanel (line 236), ScalarFieldPanel (line 260), ColorFieldPanel (line 276), ListBoxPanel (line 302).

Note: CategoryPanel at line 375 and FileItemPanel at line ~97 should also be checked. CategoryPanel wraps emRasterGroup which uses OBT_GROUP → false. FileItemPanel uses OBT_GROUP → false.

- [ ] **Step 3: Run clippy and tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: PASS

- [ ] **Step 4: Verify op count**

Run:
```bash
DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1
python3 -c "
import json
rust = [json.loads(l) for l in open('crates/eaglemode/target/golden-divergence/tktest_1x.rust_ops.jsonl') if l.strip().startswith('{')]
from collections import Counter
ops = Counter(o.get('op','?') for o in rust)
print(f'Total Rust ops: {len(rust)}')
print(f'PaintRect: {ops[\"PaintRect\"]}')
"
```

Note: This may or may not increase op counts further — child panel IsOpaque mainly affects whether the compositor paints a canvas clear behind each child, and this only matters when the child doesn't fully cover its area.

- [ ] **Step 5: Run full golden test suite**

Run: `cargo test --test golden -- --test-threads=1 2>&1 | grep -E 'FAILED|test result'`
Expected: No new failures.

- [ ] **Step 6: Commit**

```bash
git add crates/eaglemode/tests/golden/composition.rs
git commit -m "fix: correct IsOpaque for all widget panels in composition test"
```
