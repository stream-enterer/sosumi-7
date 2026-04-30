# AutoExpand/LayoutChildren Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move first-time child creation from `LayoutChildren` to `AutoExpand` on all four emTestPanel types so that `handle_notice_one` — which gates `LayoutChildren` on `GetFirstChild(id).is_some()` — can call it for positioning after children exist.

**Architecture:** In C++, `emTestPanel::AutoExpand` creates children and `LayoutChildren` positions them. In Rust, both happened inside `LayoutChildren` behind an `is_empty()` / `children_created` guard, which is never reached because `handle_notice_one` requires existing children before calling `LayoutChildren`. The fix: implement `AutoExpand` on each panel type for first-time creation, leave `LayoutChildren` for positioning only.

**Tech Stack:** Rust, `crates/emtest/src/emTestPanel.rs`, `crates/emtest/src/lib.rs`. C++ ground truth: `~/Projects/eaglemode-0.96.4/src/emTest/emTestPanel.cpp`.

---

## Background for the implementer

`emcore::emView::handle_notice_one` (emView.rs ~line 4164) calls `LayoutChildren` only when
`tree.GetFirstChild(id).is_some()`. It calls `AutoExpand` when the view-area condition meets
`ae_threshold_value` (default 150.0). So any panel that creates its first children inside
`LayoutChildren` will never have them created — `LayoutChildren` is never reached with an
empty child list.

The four affected panels and their current anti-patterns:
- **TestPanel**: `LayoutChildren` checks `ctx.children().is_empty()` and creates when empty.
- **TkTestGrpPanel**: `LayoutChildren` checks `self.children_created` flag and creates on first call.
- **TkTestPanel**: `LayoutChildren` checks `self.children_created` flag and creates on first call.
- **PolyDrawPanel**: `LayoutChildren` checks `ctx.children().is_empty()` and creates when empty.

After this plan, each panel's `AutoExpand` creates children (called once when AE threshold met),
and `LayoutChildren` only positions them (called whenever layout changes). `MAX_DEPTH` and the
`depth` field are removed — the AE threshold is the correct expansion guard, as in C++.

Run tests with: `cargo-nextest ntr`

---

### Task 1: Write a failing test for TestPanel child creation

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (add `#[cfg(test)]` module at bottom)

- [ ] **Step 1: Add test module with failing test**

At the very bottom of `crates/emtest/src/emTestPanel.rs`, after the last `}`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emContext::emContext;
    use emcore::emPanelTree::{PanelTree, ViewConditionType};
    use emcore::emView::emView;
    use emcore::test_view_harness::TestSched;
    use std::rc::Rc;

    /// Drive 5 HandleNotice + Update rounds.
    fn settle(tree: &mut PanelTree, view: &mut emView, ctx: &Rc<emContext>) {
        let mut ts = TestSched::new();
        for _ in 0..5 {
            view.HandleNotice(tree, ts.sched_mut(), Some(ctx));
            ts.with(|sc| view.Update(tree, sc));
        }
    }

    #[test]
    fn test_panel_auto_expands_children() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(TestPanel::new(ctx.clone(), DEFAULT_BG)));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);
        // Default threshold 150.0; 800×600 view with 1×1 panel → area 480 000 >> 150.

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(
            tree.GetFirstChild(root).is_some(),
            "TestPanel should have children after AutoExpand fires"
        );
        assert!(tree.find_by_name("TkTestGrp").is_some(), "TkTestGrp missing");
        assert!(tree.find_by_name("BgColorField").is_some(), "BgColorField missing");
        assert!(tree.find_by_name("PolyDraw").is_some(), "PolyDraw missing");
        assert!(tree.find_by_name("1").is_some(), "TP1 missing");
        assert!(tree.find_by_name("4").is_some(), "TP4 missing");
    }

    #[test]
    fn tktestgrp_auto_expands_children() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(TkTestGrpPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(tree.find_by_name("t1a").is_some(), "t1a missing");
        assert!(tree.find_by_name("t1b").is_some(), "t1b missing");
        assert!(tree.find_by_name("t2a").is_some(), "t2a missing");
        assert!(tree.find_by_name("t2b").is_some(), "t2b missing");
    }

    #[test]
    fn polydrawpanel_auto_expands_canvas() {
        let ctx = emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("root");
        tree.set_behavior(root, Box::new(PolyDrawPanel::new()));
        tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

        let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
        settle(&mut tree, &mut view, &ctx);

        assert!(tree.find_by_name("CanvasPanel").is_some(), "CanvasPanel missing");
    }
}
```

Note: `TestPanel::new` currently takes `(depth: u32, root_ctx, initial_bg)`. The test calls
`TestPanel::new(ctx.clone(), DEFAULT_BG)` because the `depth` parameter is removed in Task 2.
**Write the test with the new signature now** — it won't compile until Task 2 is done, which is
expected for failing-test-first TDD.

- [ ] **Step 2: Run tests, confirm compile error (expected)**

```bash
cargo-nextest ntr -E 'package(emtest)'
```

Expected: compile error — `TestPanel::new` still takes 3 arguments. This confirms the test is
written against the target API, not the current one.

---

### Task 2: Fix TestPanel — AutoExpand creates, LayoutChildren positions

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (lines ~54, ~471–495, ~1163–1235)
- Modify: `crates/emtest/src/lib.rs` (line 23 — `new_root_panel`)

C++ reference: `emTestPanel.cpp:25–45` (constructor sets threshold, loads image, restores BgColor),
`emTestPanel.cpp:480–497` (AutoExpand creates children), `emTestPanel.cpp:499–510` (LayoutChildren positions).

- [ ] **Step 1: Remove `MAX_DEPTH` constant and `depth` field**

In `emTestPanel.rs`, delete line 54:
```rust
const MAX_DEPTH: u32 = 10;
```

In the `TestPanel` struct (around line 471), remove the `depth` field:
```rust
// Remove this line:
depth: u32,
```

- [ ] **Step 2: Update `TestPanel::new` to drop the `depth` parameter**

Find `TestPanel::new` (around line 484–495). Change:
```rust
// OLD
pub(crate) fn new(depth: u32, root_ctx: Rc<emContext>, initial_bg: emColor) -> Self {
    let test_image = emGetInsResImage("emTest", "icons/teddy.tga");
    Self {
        depth,
        root_ctx,
```
To:
```rust
// NEW
pub(crate) fn new(root_ctx: Rc<emContext>, initial_bg: emColor) -> Self {
    let test_image = emGetInsResImage("emTest", "icons/teddy.tga");
    Self {
        root_ctx,
```

- [ ] **Step 3: Update `new_root_panel` in `lib.rs`**

In `crates/emtest/src/lib.rs` line 23, change:
```rust
// OLD
Some(emTestPanel::new_root_panel(ctx))
```
The signature of `new_root_panel` itself must also be updated. In `emTestPanel.rs` around line 2447:
```rust
// OLD
pub(crate) fn new_root_panel(ctx: &mut dyn ConstructCtx) -> Box<dyn PanelBehavior> {
    let root_ctx = ctx.root_context().clone();
    Box::new(TestPanel::new(0, root_ctx, DEFAULT_BG))
}

// NEW
pub(crate) fn new_root_panel(ctx: &mut dyn ConstructCtx) -> Box<dyn PanelBehavior> {
    let root_ctx = ctx.root_context().clone();
    Box::new(TestPanel::new(root_ctx, DEFAULT_BG))
}
```

- [ ] **Step 4: Add `AutoExpand` to `TestPanel`**

In `emTestPanel.rs`, add `AutoExpand` to the `PanelBehavior` impl for `TestPanel`, before
the existing `LayoutChildren` (around line 1163). The identity-key init and VarModel restore
move here from `LayoutChildren`:

```rust
fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
    // C++ emTestPanel constructor: SetAutoExpansionThreshold(900.0).
    // Done here because Rust panels don't have tree access during construction.
    // Setting inside AutoExpand makes future AutoShrink/re-expand decisions use
    // the correct threshold; the first expansion fires at the default 150.0.
    ctx.tree.SetAutoExpansionThreshold(
        ctx.id,
        900.0,
        ViewConditionType::Area,
        ctx.scheduler.as_deref_mut(),
    );

    // C++ emTestPanel constructor: BgColor = emVarModel<emColor>::GetAndRemove(GetView(), ...).
    // Identity is available here (panel is in the tree before AutoExpand fires).
    if self.identity_key.is_empty() {
        let identity = ctx.tree.GetIdentity(ctx.id);
        let key = format!("emTestPanel - BgColor of {identity}");
        let bg = emVarModel::GetAndRemove(&self.root_ctx, &key, self.bg_shared.get());
        self.bg_shared.set(bg);
        self.identity_key = key;
    }

    let root_ctx = self.root_ctx.clone();
    let bg_shared = self.bg_shared.clone();

    // C++ AutoExpand (emTestPanel.cpp:480–497): creates TkTestGrp, TP1–TP4,
    // BgColorField, PolyDraw; calls AddWakeUpSignal on BgColorField's color signal.
    let tktest_id = ctx.create_child_with("TkTestGrp", Box::new(TkTestGrpPanel::new()));
    ctx.tree.SetAutoExpansionThreshold(
        tktest_id,
        900.0,
        ViewConditionType::Area,
        ctx.scheduler.as_deref_mut(),
    );

    for i in 1..=4u32 {
        let tp_id = ctx.create_child_with(
            &format!("{i}"),
            Box::new(TestPanel::new(root_ctx.clone(), DEFAULT_BG)),
        );
        ctx.tree.SetAutoExpansionThreshold(
            tp_id,
            900.0,
            ViewConditionType::Area,
            ctx.scheduler.as_deref_mut(),
        );
    }

    let bg_for_cf = bg_shared.clone();
    let mut cf = emColorField::new(ctx, emLook::new());
    cf.SetCaption("Background Color");
    cf.SetEditable(true);
    cf.set_initial_alpha_enabled(true);
    cf.set_initial_color(bg_shared.get());
    cf.on_color = Some(Box::new(move |color, _sched: &mut SchedCtx<'_>| {
        bg_for_cf.set(color);
    }));
    ctx.create_child_with("BgColorField", Box::new(ColorFieldPanel { widget: cf }));

    ctx.create_child_with("PolyDraw", Box::new(PolyDrawPanel::new()));
}
```

- [ ] **Step 5: Simplify `LayoutChildren` to positioning only**

Replace the entire `LayoutChildren` body (currently lines ~1163–1234) with:

```rust
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    let bg = self.bg_color();
    for &(name, x, y, cw, ch) in &CHILD_LAYOUT {
        if let Some(child) = ctx.find_child_by_name(name) {
            ctx.layout_child_canvas(child, x, y, cw, ch, bg);
        }
    }
}
```

The old body had: identity-key init (moved to AutoExpand), `is_empty()` guard, child creation
block, and the positioning loop. Now only the positioning loop remains.

- [ ] **Step 6: Run tests**

```bash
cargo-nextest ntr
```

Expected: `test_panel_auto_expands_children` passes. Other tests green.

- [ ] **Step 7: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs crates/emtest/src/lib.rs
git commit -m "fix(emtest): TestPanel — AutoExpand creates children, LayoutChildren positions"
```

---

### Task 3: Fix TkTestGrpPanel — AutoExpand creates, LayoutChildren positions

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (lines ~1251–1339)

C++ reference: `emTestPanel.cpp:880–910` (`TkTestGrp` constructor + `AutoExpand` creates splitter
hierarchy; Rust diverges from splitter to 2×2 grid per existing DIVERGED annotation).

- [ ] **Step 1: Remove `children_created` field from `TkTestGrpPanel`**

In the `TkTestGrpPanel` struct (around line 1251), remove:
```rust
children_created: bool,
```

In `TkTestGrpPanel::new()`, remove:
```rust
children_created: false,
```

- [ ] **Step 2: Add `AutoExpand` to `TkTestGrpPanel`**

Add before the existing `LayoutChildren` in the `PanelBehavior` impl:

```rust
fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
    // C++ TkTestGrp::AutoExpand (emTestPanel.cpp:882–910): creates sp → sp1/sp2 → t1a/t1b/t2a/t2b.
    // DIVERGED: (dependency-forced) emSplitter not yet ported; 2×2 grid used instead.
    ctx.create_child_with("t1a", Box::new(TkTestPanel::new(self.look.clone())));
    ctx.create_child_with("t1b", Box::new(TkTestPanel::new(self.look.clone())));
    ctx.create_child_with("t2a", Box::new(TkTestPanel::new(self.look.clone())));
    let t2b_id = ctx.create_child_with(
        "t2b",
        Box::new(TkTestPanel::new(self.look.clone()).with_caption("Disabled")),
    );
    ctx.tree
        .SetEnableSwitch(t2b_id, false, ctx.scheduler.as_deref_mut());
}
```

- [ ] **Step 3: Remove `children_created` guard from `LayoutChildren`**

The current `LayoutChildren` has:
```rust
if !self.children_created {
    self.children_created = true;
    ctx.create_child_with("t1a", ...);
    // ... t1b, t2a, t2b + SetEnableSwitch
}
// positioning follows
```

Remove the entire `if !self.children_created { ... }` block. Keep only the positioning code:

```rust
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    let rect = ctx.layout_rect();
    let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
    let left_w = cr.w * 0.8;
    let right_w = cr.w * 0.2;
    let top_h = cr.h * 0.8;
    let bot_h = cr.h * 0.2;

    if let Some(id) = ctx.find_child_by_name("t1a") {
        ctx.layout_child(id, cr.x, cr.y, left_w, top_h);
    }
    if let Some(id) = ctx.find_child_by_name("t1b") {
        ctx.layout_child(id, cr.x, cr.y + top_h, left_w, bot_h);
    }
    if let Some(id) = ctx.find_child_by_name("t2a") {
        ctx.layout_child(id, cr.x + left_w, cr.y, right_w, top_h);
    }
    if let Some(id) = ctx.find_child_by_name("t2b") {
        ctx.layout_child(id, cr.x + left_w, cr.y + top_h, right_w, bot_h);
    }

    let cc = self
        .border
        .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
    ctx.set_all_children_canvas_color(cc);
}
```

Also remove the now-unused `auto_expand(&self) -> bool { true }` override from `TkTestGrpPanel`
— `auto_expand()` is not called by the view dispatch; it was cosmetic.

- [ ] **Step 4: Run tests**

```bash
cargo-nextest ntr
```

Expected: `tktestgrp_auto_expands_children` passes. All green.

- [ ] **Step 5: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): TkTestGrpPanel — AutoExpand creates children, LayoutChildren positions"
```

---

### Task 4: Fix TkTestPanel — AutoExpand creates, LayoutChildren positions

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (lines ~1416–1451, ~2024–2040)

C++ reference: `emTestPanel.cpp:535–610` — `TkTest::TkTest` constructor creates ALL widget
children directly. In Rust the equivalent is `AutoExpand` because children require tree access.

- [ ] **Step 1: Remove `children_created` field from `TkTestPanel`**

In the `TkTestPanel` struct (around line 1416), remove:
```rust
children_created: bool,
```

In `TkTestPanel::new()` (around line 1466), remove:
```rust
children_created: false,
```

- [ ] **Step 2: Add `AutoExpand` to `TkTestPanel`**

Add before `LayoutChildren` in the `PanelBehavior` impl for `TkTestPanel`:

```rust
fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
    // C++ TkTest::TkTest constructor creates all widget children immediately.
    // In Rust, AutoExpand is the equivalent since tree access requires ctx.
    self.create_all_categories(ctx);
    // Wake engine so Cycle runs to connect signals on the first frame after expansion.
    ctx.wake_up();
}
```

- [ ] **Step 3: Remove `children_created` guard from `LayoutChildren`**

Current `LayoutChildren` (around line 2024):
```rust
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    let rect = ctx.layout_rect();
    if !self.children_created {
        self.children_created = true;
        self.create_all_categories(ctx);
        ctx.wake_up();
    }
    let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
    self.layout.do_layout_skip(ctx, None, Some(cr));
    let cc = self.border.content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
    ctx.set_all_children_canvas_color(cc);
}
```

Replace with (remove the `if !self.children_created` block):
```rust
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    let rect = ctx.layout_rect();
    let cr = self.border.GetContentRect(rect.w, rect.h, &self.look);
    self.layout.do_layout_skip(ctx, None, Some(cr));
    let cc = self
        .border
        .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
    ctx.set_all_children_canvas_color(cc);
}
```

- [ ] **Step 4: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): TkTestPanel — AutoExpand creates widget children, LayoutChildren positions"
```

---

### Task 5: Fix PolyDrawPanel — AutoExpand creates CanvasPanel, LayoutChildren delegates

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (lines ~2200–2228)

C++ reference: `emTestPanel.cpp:1260` — `PolyDrawPanel::AutoExpand` creates `Canvas = new CanvasPanel(this,"CanvasPanel")`.

- [ ] **Step 1: Add `AutoExpand` to `PolyDrawPanel`**

In the `PanelBehavior` impl for `PolyDrawPanel`, add before `LayoutChildren`:

```rust
fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
    // C++ PolyDrawPanel::AutoExpand (emTestPanel.cpp:1260): creates CanvasPanel.
    // Control widgets (C-1) are deferred to the PolyDrawPanel full-port plan.
    ctx.create_child_with("CanvasPanel", Box::new(CanvasPanel::new()));
}
```

- [ ] **Step 2: Simplify `LayoutChildren` — remove creation guard**

Current:
```rust
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    if ctx.children().is_empty() {
        ctx.create_child_with("CanvasPanel", Box::new(CanvasPanel::new()));
    }
    self.group.LayoutChildren(ctx);
}
```

Replace with:
```rust
fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
    self.group.LayoutChildren(ctx);
}
```

- [ ] **Step 3: Remove the unused `auto_expand` override**

Remove from `PolyDrawPanel`'s `PanelBehavior` impl:
```rust
fn auto_expand(&self) -> bool {
    true
}
```

This method is not called by the view dispatch; removal is safe.

- [ ] **Step 4: Run tests**

```bash
cargo-nextest ntr
```

Expected: `polydrawpanel_auto_expands_canvas` passes. All green.

- [ ] **Step 5: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): PolyDrawPanel — AutoExpand creates CanvasPanel, LayoutChildren delegates"
```

---

### Task 6: Set AE threshold on root TestPanel (I-1)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (TestPanel::AutoExpand, added in Task 2)

C++ reference: `emTestPanel.cpp:39` — `SetAutoExpansionThreshold(900.0)` in constructor applies
to every TestPanel instance including the root. In Task 2, `AutoExpand` sets the threshold to
900.0 on `ctx.id` at the start. This covers all instances.

- [ ] **Step 1: Verify the threshold is set correctly in AutoExpand**

In `TestPanel::AutoExpand` (added in Task 2), confirm the first lines are:
```rust
ctx.tree.SetAutoExpansionThreshold(
    ctx.id,
    900.0,
    ViewConditionType::Area,
    ctx.scheduler.as_deref_mut(),
);
```

This was added as part of Task 2. No additional change needed — just verify it is present.

- [ ] **Step 2: Add a regression test for threshold value**

In the `#[cfg(test)]` module added in Task 1, add a new test:

```rust
#[test]
fn test_panel_ae_threshold_is_900() {
    let ctx = emContext::NewRoot();
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_behavior(root, Box::new(TestPanel::new(ctx.clone(), DEFAULT_BG)));
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
    settle(&mut tree, &mut view, &ctx);

    // After AutoExpand fires, the panel should have reset its own threshold to 900.0.
    assert_eq!(
        tree.get_ae_threshold_value(root),
        900.0,
        "TestPanel AE threshold should be 900.0 after AutoExpand (C++ constructor value)"
    );
}
```

Note: `tree.get_ae_threshold_value(root)` — check if this method exists in `PanelTree`. If it
doesn't, use `tree.panels.get(root).map(|p| p.ae_threshold_value)` or add a thin accessor. Look
for existing `ae_threshold` accessor methods in `emPanelTree.rs` before adding one.

- [ ] **Step 3: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "test(emtest): assert TestPanel AE threshold is 900.0 after AutoExpand (I-1)"
```

---

## Self-review

**Spec coverage check:**

| Requirement | Task |
|---|---|
| TestPanel: create in AutoExpand, position in LayoutChildren | Task 2 |
| TkTestGrpPanel: same | Task 3 |
| TkTestPanel: same | Task 4 |
| PolyDrawPanel: same | Task 5 |
| Remove MAX_DEPTH / depth field (I-2) | Task 2 Step 1 |
| Set AE threshold 900.0 on root TestPanel (I-1) | Task 2 Step 4, Task 6 |
| Failing test first | Task 1 |
| All four panels tested | Tasks 1 (TestPanel, TkTestGrp, PolyDraw) + integration via Task 4 |

**Known non-gaps (out of scope for this plan):**
- TkTestGrp uses 2×2 grid instead of emSplitter — pre-existing DIVERGED annotation, unchanged.
- PolyDrawPanel control widgets (C-1) — deferred to PolyDrawPanel full-port plan.
- All other audit findings — deferred to emTestPanel compliance batch plan.
