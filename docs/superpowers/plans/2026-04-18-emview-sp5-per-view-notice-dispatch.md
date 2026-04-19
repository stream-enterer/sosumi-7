# SP5 — Per-view Notice Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore per-view `HandleNotice` dispatch from `emView::Update` (C++ `emView.cpp:1312`), with each view using its own `CurrentPixelTallness`. Panels hold `Weak<RefCell<emView>>` back-references (1:1 with C++ `emPanel::View &`); `NoticeList` ownership moves from `PanelTree` to `emView`.

**Architecture:** Cascade `emView` into `Rc<RefCell<emView>>` at its two owner sites (`emWindow::view`, `emSubViewPanel::sub_view`). Add `View: Weak<RefCell<emView>>` to `emPanel`. Migrate NoticeList state and `HandleNotice` body onto `emView`. Delete framework-side global notice dispatch and the SP4 `test_window_id` test-identity hack.

**Tech Stack:** Rust (emcore, eaglemode, emmain, emfileman workspace); winit/wgpu; cargo-nextest.

**Spec:** `docs/superpowers/specs/2026-04-18-emview-sp5-per-view-notice-dispatch-design.md`

**Authority order (CLAUDE.md):** C++ source → golden tests → Rust idiom → LLM convenience. Every structural choice in this plan has been justified under that order in the spec.

---

## File Structure

### Modified

| File | Responsibility change |
|---|---|
| `crates/emcore/src/emWindow.rs` | `view: emView` → `view: Rc<RefCell<emView>>`. Delete `test_window_id` field and `new_for_test` constructor. Update `view()` / `view_mut()` accessors to return the `Rc` (or borrow it). |
| `crates/emcore/src/emSubViewPanel.rs` | `sub_view: emView` → `sub_view: Rc<RefCell<emView>>`. Update `GetSubView` / `sub_view_mut` / `view_and_tree_mut`. |
| `crates/emcore/src/emPanel.rs` | Add `View: Weak<RefCell<emView>>` field to `PanelData`. |
| `crates/emcore/src/emPanelTree.rs` | `create_root(name)` → `create_root(name, view: Weak<RefCell<emView>>)`. `create_child` inherits parent's View. Delete `notice_ring_head_next`, `notice_ring_head_prev`, `has_pending_notices`, `HandleNotice`. Migrate ~20 `add_to_notice_list(id)` call sites to route through `panel.View`. |
| `crates/emcore/src/emView.rs` | Add `NoticeList`, `notice_ring_{head_next,head_prev}`, `has_pending_notices` fields. Add `AddToNoticeList(&mut self, PanelId)` and `HandleNotice(&mut self, &mut PanelTree, window_focused: bool) -> bool`. Port body from old `PanelTree::HandleNotice`. Call `HandleNotice` from `Update` at the `emView.cpp:1312` slot. |
| `crates/emcore/src/emGUIFramework.rs` | Delete `pixel_tallness = windows.values().next()...` block, global `tree.HandleNotice(...)` call, and `TODO(per-view-notice-dispatch)` comment. Keep `run_panel_cycles`. |
| Test files in `crates/eaglemode/tests/**`, `crates/emmain/src/**`, `examples/**`, `crates/emcore/src/emView.rs` tests | Update `create_root(name)` call sites to pass `Rc::downgrade(&view_rc)`. |

### Tests

| File | Purpose |
|---|---|
| `crates/emcore/src/emView.rs` (inline `#[cfg(test)]`) | Add `sp5_per_view_notice_dispatch_uses_correct_pixel_tallness` (two views, distinct `CurrentPixelTallness`, assert per-view dispatch). |

### Not modified

- `run_panel_cycles` stays in `PanelTree`. Out of scope.
- No change to `emContext` or SP7 work.
- No change to notice *queueing* semantics.

---

## Phase 0 — Baseline snapshot

**Purpose:** Capture the pre-SP5 state so regressions surface immediately.

- [ ] **Step 0.1: Confirm clean tree**

```bash
git status
git log --oneline -5
```

Expected: clean working tree, HEAD at or after `ad2603a` (SP4 closeout).

- [ ] **Step 0.2: Record baseline test counts**

```bash
cargo-nextest ntr 2>&1 | tail -5
```

Expected: `2432 tests ... 0 failed ... 9 skipped`. Record the exact count in a scratch file (`/tmp/sp5_baseline.txt`) — every phase gate re-checks against this.

- [ ] **Step 0.3: Record golden baseline**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 pass / 6 fail (baseline six: `composition_tktest_{1x,2x}`, `notice_window_resize`, `testpanel_{expanded,root}`, `widget_file_selection_box`).

- [ ] **Step 0.4: Record smoke baseline**

```bash
timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"
```

Expected: `exit=124` or `exit=143`. Program stayed alive.

---

## Phase 1 — `Rc<RefCell<emView>>` cascade

**Purpose:** Wrap `emView` in `Rc<RefCell<>>` at its two owner sites so panels can hold `Weak<RefCell<emView>>` back-references in Phase 2. No behavioral change in this phase.

**Expected blast radius:** ~30 files, ~150–200 lines. If >200, stop and escalate.

### Task 1.1 — Convert `emWindow::view`

**Files:**
- Modify: `crates/emcore/src/emWindow.rs:72` (field declaration), `:684` (constructor signature), `:1334` (`view()` accessor), and all internal uses of `self.view` / `&self.view` / `&mut self.view`.

- [ ] **Step 1: Change the field**

```rust
// crates/emcore/src/emWindow.rs:72
// Before:
view: emView,
// After:
view: Rc<RefCell<emView>>,
```

- [ ] **Step 2: Update every `emWindow` constructor to wrap the view**

Find every `emWindow` constructor (`new`, `create`, `new_popup_pending`, `new_for_test`). At each site where an `emView` is being stored into the field, wrap it:

```rust
view: Rc::new(RefCell::new(emView::new(root, width, height, core_config))),
```

Add `use std::cell::RefCell;` and `use std::rc::Rc;` at the top of `emWindow.rs` if not already present.

- [ ] **Step 3: Update accessors**

```rust
// crates/emcore/src/emWindow.rs:1334
// Before:
pub fn view(&self) -> &emView { &self.view }
// After:
pub fn view(&self) -> std::cell::Ref<'_, emView> { self.view.borrow() }
pub fn view_mut(&mut self) -> std::cell::RefMut<'_, emView> { self.view.borrow_mut() }
pub(crate) fn view_rc(&self) -> &Rc<RefCell<emView>> { &self.view }
```

(Exact names: if `view_mut` already exists, update it; otherwise add it.)

- [ ] **Step 4: Thread updates through internal uses**

For every `self.view.<method>` inside `emWindow`, decide whether the surrounding function already holds `&mut self` or `&self`, and replace with `self.view.borrow()` / `self.view.borrow_mut()` as appropriate. No borrow should be held across `await` or across a nested call that would reborrow.

- [ ] **Step 5: Fix call sites in other modules**

Run `cargo check` and fix each compile error in sequence. Typical fixes: `win.view()` returning a `Ref` instead of `&emView` — callers using `.method()` still compile via `Deref`; callers that stored `&emView` need a local binding (`let v = win.view(); v.method()`).

- [ ] **Step 6: Verify tests green**

```bash
cargo-nextest ntr 2>&1 | tail -5
```

Expected: 2432 pass / 0 fail (matches baseline).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(1.1/n): wrap emWindow::view in Rc<RefCell<emView>>

Cascade step 1 of SP5. Owner-side wrap; no behavioral change.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.2 — Convert `emSubViewPanel::sub_view`

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs:22` (field), `:59` (constructor), `:90-101` (accessors).

- [ ] **Step 1: Change the field**

```rust
// crates/emcore/src/emSubViewPanel.rs:22
// Before:
sub_view: emView,
// After:
sub_view: Rc<RefCell<emView>>,
```

Add `use std::cell::RefCell;` and `use std::rc::Rc;` if not present.

- [ ] **Step 2: Update constructor**

```rust
// crates/emcore/src/emSubViewPanel.rs:59
// Before:
let sub_view = emView::new(root, 1.0, 1.0, core_config);
// After:
let sub_view = Rc::new(RefCell::new(emView::new(root, 1.0, 1.0, core_config)));
```

- [ ] **Step 3: Update accessors**

```rust
// Before:
pub fn GetSubView(&self) -> &emView { &self.sub_view }
pub fn sub_view_mut(&mut self) -> &mut emView { &mut self.sub_view }
pub fn view_and_tree_mut(&mut self) -> (&mut emView, &mut PanelTree) { ... }

// After:
pub fn GetSubView(&self) -> std::cell::Ref<'_, emView> { self.sub_view.borrow() }
pub fn sub_view_mut(&mut self) -> std::cell::RefMut<'_, emView> { self.sub_view.borrow_mut() }
pub(crate) fn sub_view_rc(&self) -> &Rc<RefCell<emView>> { &self.sub_view }
```

For `view_and_tree_mut`: if this returns two borrows simultaneously, callers will need to restructure. Find each caller (`grep -rn "view_and_tree_mut"`), inline the logic to acquire `sub_view.borrow_mut()` plus the tree reference separately, and delete `view_and_tree_mut` if no longer needed.

- [ ] **Step 4: Cargo check and fix callers**

```bash
cargo check 2>&1 | head -40
```

Fix each error. Typical patterns same as Task 1.1 Step 5.

- [ ] **Step 5: Tests green**

```bash
cargo-nextest ntr 2>&1 | tail -5
```

Expected: 2432 pass / 0 fail.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(1.2/n): wrap emSubViewPanel::sub_view in Rc<RefCell<emView>>

Cascade step 2 of SP5. Owner-side wrap; no behavioral change.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 1.3 — Phase-1 gate

- [ ] **Step 1: Full test suite**

```bash
cargo-nextest ntr
```

Expected: 2432 pass / 0 fail.

- [ ] **Step 2: Golden parity**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 pass / 6 fail (same baseline six).

- [ ] **Step 3: Smoke**

```bash
timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"
```

Expected: `exit=124` or `exit=143`.

- [ ] **Step 4: Line-count check**

```bash
git diff --stat HEAD~2 HEAD
```

Expected: fewer than ~200 lines changed. If more, stop and review — phase 1 cascade should match the Phase-6-emWindow precedent (~160 lines). Excess indicates scope creep.

---

## Phase 2 — `emPanel::View` field

**Purpose:** Add `View: Weak<RefCell<emView>>` to `PanelData`, set it at construction. Centrally debug-assert that `upgrade()` succeeds on any `add_to_notice_list` call.

### Task 2.1 — Add the field

**Files:**
- Modify: `crates/emcore/src/emPanel.rs` (`PanelData` struct).

- [ ] **Step 1: Add field to `PanelData`**

Locate `struct PanelData` in `emPanel.rs`. Add:

```rust
/// 1:1 with C++ `emPanel::View &` (emPanel.h).
/// Set at construction by `PanelTree::create_root` / `create_child`;
/// never mutated thereafter.
pub(crate) View: std::rc::Weak<std::cell::RefCell<crate::emView::emView>>,
```

Add a default (`Weak::new()`) in `PanelData::new(name)` so the struct-default case still compiles. Construction callers will overwrite it.

- [ ] **Step 2: Cargo check**

```bash
cargo check 2>&1 | tail -10
```

Expected: clean (only a warning about unused field, which the upcoming tasks will resolve).

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(2.1/n): add emPanel::View: Weak<RefCell<emView>>

Field only; population in create_root/create_child follows.
Port of C++ emPanel::View & (emPanel.h).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 2.2 — Populate at `create_root`

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs:445` (`create_root` signature + body).

- [ ] **Step 1: Change `create_root` signature**

```rust
// crates/emcore/src/emPanelTree.rs:445
// Before:
pub fn create_root(&mut self, name: &str) -> PanelId {
    ...
    let id = self.panels.insert(PanelData::new(name.to_string()));
    ...

// After:
pub fn create_root(
    &mut self,
    name: &str,
    view: std::rc::Weak<std::cell::RefCell<crate::emView::emView>>,
) -> PanelId {
    ...
    let id = self.panels.insert(PanelData::new(name.to_string()));
    self.panels[id].View = view;
    ...
```

- [ ] **Step 2: Migrate call sites**

There are ~30 call sites across `crates/` and `examples/`. For each, pass `Rc::downgrade(&view_rc)` where `view_rc` is the `Rc<RefCell<emView>>` that owns or will own this tree's panels.

**In production (`crates/emmain/src/emMainWindow.rs:765,966` and similar):** the call is `app.tree.create_root("root")`. The view is owned by `app.window.view_rc()` (post Phase 1). Update to:

```rust
let view_rc = app.windows.values().next().expect("main window").borrow().view_rc().clone();
let root_id = app.tree.create_root("root", Rc::downgrade(&view_rc));
```

(Exact accessor chain depends on what Phase 1 left; adjust.)

**In tests and examples** where there is no window yet, construct the view first, wrap in `Rc`, then create the root passing the weak:

```rust
let cfg = Rc::new(RefCell::new(emCoreConfig::default()));
let mut tree = PanelTree::new();
// The view needs a root PanelId. Chicken-and-egg: the common pattern is
// to insert a placeholder root first, construct the view with it, then
// overwrite View on the placeholder via a helper. Use the helper below.
```

To avoid the chicken-and-egg, add a helper on `PanelTree`:

```rust
/// Create a root panel whose View will be set by the caller once the
/// emView Rc exists. Marks the invariant: View MUST be set before any
/// add_to_notice_list call for this panel.
#[cfg(any(test, feature = "test-support"))]
pub fn create_root_deferred_view(&mut self, name: &str) -> PanelId {
    self.create_root(name, std::rc::Weak::new())
}

#[cfg(any(test, feature = "test-support"))]
pub fn set_panel_view(
    &mut self,
    id: PanelId,
    view: std::rc::Weak<std::cell::RefCell<crate::emView::emView>>,
) {
    self.panels[id].View = view;
    // Propagate to all descendants already created.
    let mut stack = vec![id];
    while let Some(p) = stack.pop() {
        let mut child = self.panels[p].first_child;
        while let Some(c) = child {
            self.panels[c].View = self.panels[id].View.clone();
            stack.push(c);
            child = self.panels[c].next_sibling;
        }
    }
}
```

Then the test pattern becomes:

```rust
let cfg = Rc::new(RefCell::new(emCoreConfig::default()));
let mut tree = PanelTree::new();
let root = tree.create_root_deferred_view("root");
let view = Rc::new(RefCell::new(emView::new(root, 800.0, 600.0, cfg)));
tree.set_panel_view(root, Rc::downgrade(&view));
```

Update the 30+ call sites mechanically. For tests using `emView::new_for_test(root, w, h)` followed by `tree.create_root(...)`, reorder: create tree, create root deferred, construct the view, set_panel_view.

- [ ] **Step 3: Cargo check & fix**

```bash
cargo check 2>&1 | tail -20
```

Iterate until clean. Typical: test harnesses need a small restructure of the setup sequence.

- [ ] **Step 4: Tests green**

```bash
cargo-nextest ntr
```

Expected: 2432 pass / 0 fail.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(2.2/n): populate emPanel::View at create_root; migrate call sites

Root panels receive their owning view's Weak at construction.
Children inherit in task 2.3.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 2.3 — Inherit at `create_child`

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs:469` (`create_child` body).

- [ ] **Step 1: Inherit parent's View**

```rust
// crates/emcore/src/emPanelTree.rs:469
pub fn create_child(&mut self, parent: PanelId, name: &str) -> PanelId {
    let created_by_ae = self.panels[parent].ae_calling;
    let parent_view = self.panels[parent].View.clone();

    let id = self.panels.insert(PanelData::new(name.to_string()));
    self.panels[id].View = parent_view;
    // ... existing linkage code ...
```

- [ ] **Step 2: Debug-assert non-dangling View at `add_to_notice_list` entry**

```rust
// crates/emcore/src/emPanelTree.rs:344
pub(crate) fn add_to_notice_list(&mut self, id: PanelId) {
    debug_assert!(
        self.panels[id].View.upgrade().is_some(),
        "emPanel::View must be set (non-dangling) before add_to_notice_list; \
         panel name = {:?}",
        self.panels[id].name
    );
    // ... existing body unchanged ...
```

(Keep the body using the existing `notice_ring_*` fields on `PanelTree`; Phase 3 migrates the state.)

- [ ] **Step 3: Tests green**

```bash
cargo-nextest ntr
```

Expected: 2432 pass / 0 fail. If any test panics on the `debug_assert`, that test has a panel whose View wasn't set — fix the test setup to use `set_panel_view` before the first notice.

- [ ] **Step 4: Golden parity**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 / 6.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(2.3/n): emPanel::View inherits from parent in create_child

Every panel has non-dangling View by construction;
debug_assert added at add_to_notice_list entry.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 3 — NoticeList migration to `emView`

**Purpose:** Move the notice-ring state (`notice_ring_head_next`, `notice_ring_head_prev`, `has_pending_notices`) from `PanelTree` to `emView`. Rewrite `add_to_notice_list` to route through `panel.View.upgrade()`.

### Task 3.1 — Add NoticeList state to `emView`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (struct `emView`, constructor).

- [ ] **Step 1: Add fields**

Locate `struct emView`. Add (placement near other ring-related fields if any; otherwise with the other internal state):

```rust
/// 1:1 with C++ `emView::NoticeList` (emView.h:576).
///
/// DIVERGED: C++ uses `PanelRingNode *` intrusive sentinel linkage
/// (emView.h:576, emPanel.h:823). Rust uses `Option<PanelId>` head/tail
/// into the panel arena. Semantics: panels with queued notices form a
/// doubly-linked list via `emPanel::notice_{prev,next}_in_ring`;
/// `AddToNoticeList` links at the tail; `HandleNotice` drains from the
/// head. Data-structure divergence is *forced* (Rust ownership rules
/// make intrusive raw-pointer rings impractical without `unsafe`);
/// ownership location (per-view) matches C++ exactly.
pub(crate) notice_ring_head_next: Option<PanelId>,
pub(crate) notice_ring_head_prev: Option<PanelId>,
pub(crate) has_pending_notices: bool,
```

Initialize to `None` / `None` / `false` in `emView::new`.

- [ ] **Step 2: Add `AddToNoticeList` method on `emView`**

```rust
/// 1:1 with C++ `emView::AddToNoticeList(PanelRingNode *)` (emView.cpp:1282).
/// The PanelTree parameter gives access to the panel arena for ring linkage.
pub fn AddToNoticeList(&mut self, tree: &mut PanelTree, id: PanelId) {
    // Port the body of the old PanelTree::add_to_notice_list here,
    // but read/write self.notice_ring_head_next / _prev and
    // self.has_pending_notices instead of the tree's fields.
    // Body is a direct move of the existing implementation with field-path swap.
    {
        let p = &tree.panels[id];
        if p.notice_prev_in_ring.is_some() || p.notice_next_in_ring.is_some() {
            return;
        }
        if self.notice_ring_head_next == Some(id) {
            return;
        }
    }
    match self.notice_ring_head_prev {
        Some(old_tail) => {
            tree.panels[old_tail].notice_next_in_ring = Some(id);
            tree.panels[id].notice_prev_in_ring = Some(old_tail);
            tree.panels[id].notice_next_in_ring = None;
            self.notice_ring_head_prev = Some(id);
        }
        None => {
            tree.panels[id].notice_prev_in_ring = None;
            tree.panels[id].notice_next_in_ring = None;
            self.notice_ring_head_next = Some(id);
            self.notice_ring_head_prev = Some(id);
        }
    }
    self.has_pending_notices = true;
    // Existing side-effect: wake the view's update engine.
    // Port the `WakeUpUpdateEngine`-equivalent call that the old
    // emView::AddToNoticeList (emView.rs:3447) performs.
    // If the old method did `view.wake_up_update_engine()` or similar,
    // inline that here.
    self.wake_up_update_engine(/* scheduler access */);
}
```

(Exact wake-up plumbing: inspect the existing `emView::AddToNoticeList` at `emView.rs:3447` and fold its wake-up call into this new body. The old method will be deleted in Task 3.3.)

- [ ] **Step 3: Tests green (sanity check — the new fields exist but aren't used yet)**

```bash
cargo check
cargo-nextest ntr 2>&1 | tail -5
```

Expected: 2432 / 0.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(3.1/n): add NoticeList state and AddToNoticeList to emView

Per-view fields exist but are not yet used; task 3.2 migrates
call sites.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 3.2 — Route `add_to_notice_list` calls through `emView`

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs:344` (delete `add_to_notice_list`), `:464,:497,:501,:539,:596,:662,:828,:1020,:1103,:1145,:1234,:1257,:1272,:1310,:1348,:1538,:1653,:1940` (19 call sites).

- [ ] **Step 1: Rewrite each call site**

Each call is `self.add_to_notice_list(id)` inside a `PanelTree` method. The `PanelTree` method cannot directly borrow `emView`, so each method must receive the view as a parameter, or the add path must be extracted to a helper that the caller invokes outside the tree method.

**Decision:** Add a thin shim on `PanelTree` that takes the view:

```rust
/// Thin router to emView::AddToNoticeList. Kept on PanelTree so
/// existing in-tree callers don't need to reach out for the view.
pub(crate) fn add_to_notice_list(&mut self, view: &mut crate::emView::emView, id: PanelId) {
    view.AddToNoticeList(self, id);
}
```

Then every call site changes from `self.add_to_notice_list(id)` to `self.add_to_notice_list(view, id)`, which means each enclosing `PanelTree` method needs a `view: &mut emView` parameter threaded in. There are ~10 such methods; each must be updated.

**Alternative (adopt this):** Route through the panel's own View (`Weak<RefCell<emView>>`), using `upgrade()`:

```rust
pub(crate) fn add_to_notice_list(&mut self, id: PanelId) {
    let view_rc = self.panels[id].View.upgrade().expect(
        "emPanel::View non-dangling; ensured at create_root/create_child"
    );
    let mut view_ref = view_rc.borrow_mut();
    view_ref.AddToNoticeList(self, id);
}
```

This keeps the existing call-site signatures unchanged — `self.add_to_notice_list(id)` stays. Every in-tree caller just works.

**Hazard:** If any caller already holds a `view.borrow_mut()` up the stack, `view_rc.borrow_mut()` in this helper will panic (re-entrant borrow). Before committing, grep for in-tree callers that are themselves invoked from inside a `view.borrow_mut()` block:

```bash
grep -rn "borrow_mut()" crates/emcore/src/ | grep -v "^crates/emcore/src/emView.rs" | head
```

If any `PanelTree` method that calls `add_to_notice_list` is itself called while a `view.borrow_mut()` is live, restructure that outer site to release the borrow before the tree call. SP4's `queue_or_apply` pattern is precedent.

- [ ] **Step 2: Implement the reroute**

Replace `PanelTree::add_to_notice_list` body with the upgrade-borrow-delegate version shown above.

- [ ] **Step 3: Delete the old ring fields from `PanelTree`**

Remove from `struct PanelTree`:

```rust
pub(crate) notice_ring_head_next: Option<PanelId>,
pub(crate) notice_ring_head_prev: Option<PanelId>,
has_pending_notices: bool,
```

And from `PanelTree::new()`:

```rust
notice_ring_head_next: None,
notice_ring_head_prev: None,
has_pending_notices: false,
```

- [ ] **Step 4: Cargo check & fix**

```bash
cargo check 2>&1 | tail -30
```

Typical remaining errors:
- `PanelTree::HandleNotice` still reads the deleted fields → leave broken for now; Task 4 will delete or reroute `HandleNotice`.
- If compile errors block task 3 progress, add temporary field-reads via `tree.panels[id].View.upgrade().unwrap().borrow().has_pending_notices` and similar. These die in Task 4.

Actually cleaner: do **not** delete `has_pending_notices` on `PanelTree` in this task — leave it as a Rust-side bool that `PanelTree::HandleNotice` checks, updated from `PanelTree::add_to_notice_list` until task 4 removes `PanelTree::HandleNotice` entirely. Defer field deletion to Task 4.

Revised Step 3: keep the `has_pending_notices` bool on `PanelTree` as a cache mirror of the views' per-view flags, set-only (never cleared) until Task 4. Delete only `notice_ring_head_next` and `notice_ring_head_prev` here — those are now genuinely unused.

- [ ] **Step 5: Tests green**

```bash
cargo-nextest ntr
```

Expected: 2432 / 0.

- [ ] **Step 6: Golden parity**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 / 6.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(3.2/n): route add_to_notice_list through emView::AddToNoticeList

PanelTree::add_to_notice_list now upgrades the panel's View weak
and delegates. Ring fields moved to emView. PanelTree retains
has_pending_notices cache until Phase 4 retires PanelTree::HandleNotice.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 3.3 — Delete old `emView::AddToNoticeList`

**Files:**
- Modify: `crates/emcore/src/emView.rs:3447` (delete old method).

- [ ] **Step 1: Delete the old delegate**

The old method at `emView.rs:3447`:

```rust
pub fn AddToNoticeList(&mut self, tree: &mut PanelTree, panel: PanelId) {
    tree.add_to_notice_list(panel);
    // ... wake-up plumbing ...
}
```

If its body has been folded into the new `AddToNoticeList` in Task 3.1, delete the old method outright. Otherwise, merge any remaining wake-up logic into the new method and then delete.

- [ ] **Step 2: Tests green**

```bash
cargo-nextest ntr
```

Expected: 2432 / 0. The Phase-7 test `test_phase7_add_to_notice_list_wakes_update_engine` must still pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(3.3/n): delete stale emView::AddToNoticeList shim

The Task 3.1 version is now canonical.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 4 — Dispatch relocation

**Purpose:** Port `PanelTree::HandleNotice` body to `emView::HandleNotice`; call it from `emView::Update` at the `emView.cpp:1312` slot; delete the framework-side global dispatch.

### Task 4.1 — Port `HandleNotice` to `emView`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (add `HandleNotice`), `crates/emcore/src/emPanelTree.rs:1519` (source body for the port).

- [ ] **Step 1: Read the existing `PanelTree::HandleNotice`**

```bash
sed -n '1519,1700p' crates/emcore/src/emPanelTree.rs
```

Understand what it does: drains the notice ring, builds per-panel `PanelCtx`, calls `HandleNotice` / layout on each. It reads `self.notice_ring_head_next` (now on `emView`) and `pixel_tallness` (now on `emView` as `self.CurrentPixelTallness`).

- [ ] **Step 2: Add `emView::HandleNotice`**

```rust
// crates/emcore/src/emView.rs
/// 1:1 with C++ `emView::HandleNotice` dispatch driven from
/// `emView::Update` (emView.cpp:1312).
///
/// Drains this view's notice ring, dispatching HandleNotice/HandleLayout
/// on each panel using the view's own `CurrentPixelTallness`.
/// Returns true if any notices were handled (matching the old
/// PanelTree::HandleNotice return contract).
pub fn HandleNotice(&mut self, tree: &mut PanelTree, window_focused: bool) -> bool {
    // Port of the body at emPanelTree.rs:1519 with two substitutions:
    //   self.notice_ring_head_next  (was tree.notice_ring_head_next)
    //   self.CurrentPixelTallness   (was the pixel_tallness parameter)
    //
    // Walk the ring head-to-tail; for each panel id: unlink it, rebuild
    // PanelCtx, invoke handle_notice_one-equivalent, loop until the ring
    // is empty. Follow the existing PanelTree::HandleNotice body exactly
    // — this is a mechanical move, not a redesign.
    //
    // NOTE: because HandleNotice itself is inside a `self` (&mut emView)
    // borrow and `PanelCtx::new(tree, id, pixel_tallness)` does not take
    // the view, re-entrancy concerns are scoped to the panel callback
    // layer only. Any scheduler mutations fired from panel HandleNotice
    // handlers must route through SP4's queue_or_apply path (already in
    // place for the signal sites); this method adds no new re-entrancy
    // surface.
    // <body ported from PanelTree::HandleNotice:1519..1700>
}
```

Port the body verbatim, substituting field accesses as noted. Delete nothing from `PanelTree` yet.

- [ ] **Step 3: Tests green**

```bash
cargo-nextest ntr 2>&1 | tail -5
```

Expected: 2432 / 0 (still via framework → PanelTree::HandleNotice route; new emView method exists but not called).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(4.1/n): add emView::HandleNotice (per-view dispatch)

Body ported from PanelTree::HandleNotice. Not yet wired into
emView::Update; framework still drives global dispatch.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 4.2 — Call `HandleNotice` from `emView::Update`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (`Update` body).

- [ ] **Step 1: Locate the `emView.cpp:1312` slot in `emView::Update`**

The C++ call happens early in `emView::Update` — before the main per-frame work. Find the equivalent spot in Rust's `emView::Update` (post-SP4, it's the sole Update path via `UpdateEngineClass::Cycle`).

- [ ] **Step 2: Insert the call**

```rust
pub fn Update(&mut self, tree: &mut PanelTree) {
    // C++ emView.cpp:1312: HandleNotice driven per-view from Update.
    if self.has_pending_notices {
        let focused = self.window_focused;
        self.HandleNotice(tree, focused);
    }
    // ... rest of existing Update body ...
}
```

- [ ] **Step 3: Tests green**

```bash
cargo-nextest ntr 2>&1 | tail -5
```

Expected: 2432 / 0. The framework *still* calls `PanelTree::HandleNotice` in parallel, but it's now an idempotent no-op (the view already drained its ring on the engine cycle). **Double-dispatch here is the transitional state** — the next task removes the framework-side call.

Actually verify: `PanelTree::HandleNotice` iterates all views' panels — but the ring state is now per-view. The framework's call must become a no-op or be deleted. If tests fail because of double-dispatch side effects (e.g., a signal fired twice), move directly to Task 4.3 and commit them together.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(4.2/n): dispatch HandleNotice from emView::Update

C++ emView.cpp:1312 parity. Framework global dispatch removed
in task 4.3.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 4.3 — Delete framework global dispatch

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs:509-527` (delete block).
- Modify: `crates/emcore/src/emPanelTree.rs:1519` (delete `HandleNotice`).

- [ ] **Step 1: Delete framework block**

Remove these lines from `emGUIFramework::about_to_wait`:

```rust
// DELETE:
// TODO(per-view-notice-dispatch): ...comment...
let pixel_tallness = self
    .windows
    .values()
    .next()
    .map(|rc| rc.borrow().view().GetCurrentPixelTallness())
    .unwrap_or(1.0);

// KEEP:
self.tree.run_panel_cycles(pixel_tallness);

// DELETE:
let had_notices = self.tree.HandleNotice(window_focused, pixel_tallness);
```

`run_panel_cycles` still needs a pixel_tallness. Keep the `pixel_tallness = …` computation, but rename its local binding to make its reduced purpose obvious, and drop the TODO comment (which referenced notice dispatch):

```rust
// run_panel_cycles is a Rust-only construct (panels don't implement
// emEngine in the Rust port). It still picks an arbitrary window's
// pixel_tallness — same shortcut the notice path used before SP5.
// Closing this requires making emPanel register per-view as an engine,
// a separate workstream.
let panel_cycle_pixel_tallness = self
    .windows
    .values()
    .next()
    .map(|rc| rc.borrow().view().GetCurrentPixelTallness())
    .unwrap_or(1.0);
self.tree.run_panel_cycles(panel_cycle_pixel_tallness);
```

- [ ] **Step 2: Delete `PanelTree::HandleNotice`**

Remove the `pub fn HandleNotice(...)` method and its private helper `handle_notice_one`. Delete the now-unused `has_pending_notices` field on `PanelTree` and its `new()` initializer.

- [ ] **Step 3: Tests green**

```bash
cargo-nextest ntr
```

Expected: 2432 / 0. If any test was relying on the framework-side dispatch timing (fires notices on panels whose view's Update hasn't run this frame), investigate per-case — likely the test registers its view's engine and the per-view path covers it.

- [ ] **Step 4: Golden parity**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 / 6 (same baseline).

- [ ] **Step 5: Smoke**

```bash
timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"
```

Expected: `exit=124` or `exit=143`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(4.3/n): delete framework-side global notice dispatch

emView::Update is now the sole driver. PanelTree::HandleNotice and
has_pending_notices deleted. emGUIFramework retains a reduced
pixel_tallness pick for run_panel_cycles (Rust-only construct;
separate debt).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 4.4 — Multi-view unit test

**Files:**
- Modify: `crates/emcore/src/emView.rs` (`#[cfg(test)]` module).

- [ ] **Step 1: Write the failing test**

Append to `emView.rs`'s test module:

```rust
#[test]
fn sp5_per_view_notice_dispatch_uses_correct_pixel_tallness() {
    // Two independent views with different CurrentPixelTallness.
    // A panel under each view queues a notice. Each view's
    // HandleNotice fires with its own pixel tallness; neither
    // sees the other's panels.

    let cfg = Rc::new(RefCell::new(crate::emCoreConfig::emCoreConfig::default()));
    let mut tree_a = PanelTree::new();
    let mut tree_b = PanelTree::new();

    let root_a = tree_a.create_root_deferred_view("root_a");
    let view_a = Rc::new(RefCell::new(emView::new(root_a, 800.0, 600.0, cfg.clone())));
    tree_a.set_panel_view(root_a, Rc::downgrade(&view_a));

    let root_b = tree_b.create_root_deferred_view("root_b");
    let view_b = Rc::new(RefCell::new(emView::new(root_b, 1920.0, 1080.0, cfg.clone())));
    tree_b.set_panel_view(root_b, Rc::downgrade(&view_b));

    // Force distinct CurrentPixelTallness values via SetGeometry.
    view_a.borrow_mut().SetGeometry(0.0, 0.0, 800.0, 600.0, 1.0);   // tallness 1.0
    view_b.borrow_mut().SetGeometry(0.0, 0.0, 1920.0, 1080.0, 2.0); // tallness 2.0

    // Queue a notice on each root (simulate any flag-setting notice).
    {
        let mut va = view_a.borrow_mut();
        tree_a.panels[root_a].pending_notices = NoticeFlags::SOUGHT_NAME_CHANGED;
        va.AddToNoticeList(&mut tree_a, root_a);
    }
    {
        let mut vb = view_b.borrow_mut();
        tree_b.panels[root_b].pending_notices = NoticeFlags::SOUGHT_NAME_CHANGED;
        vb.AddToNoticeList(&mut tree_b, root_b);
    }

    assert!(view_a.borrow().has_pending_notices);
    assert!(view_b.borrow().has_pending_notices);

    // Drive each view's HandleNotice.
    view_a.borrow_mut().HandleNotice(&mut tree_a, true);
    view_b.borrow_mut().HandleNotice(&mut tree_b, true);

    // Each view's notice ring is empty; each view's pixel tallness is untouched.
    assert!(!view_a.borrow().has_pending_notices);
    assert!(!view_b.borrow().has_pending_notices);
    assert_eq!(view_a.borrow().CurrentPixelTallness, 1.0);
    assert_eq!(view_b.borrow().CurrentPixelTallness, 2.0);

    // Cross-tree isolation: tree_a's ring fields (on view_a) never
    // referenced root_b, and vice versa. (Structural — compiler-enforced
    // via the Weak<emView> on each panel.) This is implicit; explicit
    // assertion would require exposing internals we don't need to expose.
}
```

(Exact `NoticeFlags` variant name: use whatever flag PanelTree tests currently use; the important point is that it causes `AddToNoticeList` to be called, not what the flag is.)

- [ ] **Step 2: Run the new test; expect it to pass (since Phase 4.3 already wired everything)**

```bash
cargo test -p emcore sp5_per_view_notice_dispatch_uses_correct_pixel_tallness
```

Expected: 1 passed.

(If the test is being written **before** Phase 4.3 lands, it would fail; in that case move the test to Phase 4.2 and make its passing part of 4.3's gate. The TDD-strict ordering is left to the implementer's judgment.)

- [ ] **Step 3: Full suite**

```bash
cargo-nextest ntr
```

Expected: 2433 / 0 (+1 from the new test).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(4.4/n): per-view notice dispatch multi-view test

Two views with distinct CurrentPixelTallness, distinct notice
rings, each drained in isolation.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 5 — SP4 test-hack removal

**Purpose:** Delete `emWindow::test_window_id` and `emWindow::new_for_test`, both of which exist only to substitute for missing real view identity under `UpdateEngineClass::Cycle` routing. SP5's `Rc<RefCell<emView>>` ownership now provides real identity.

### Task 5.1 — Survey callers

- [ ] **Step 1: Find every use**

```bash
grep -rn "test_window_id\|emWindow::new_for_test\|Window::new_for_test" crates/ examples/ 2>/dev/null
```

Record the list. Expect ~5–10 sites in test support code and a handful of test files.

### Task 5.2 — Migrate or rewrite each caller

For each caller that used `emWindow::new_for_test` to obtain an `emWindow` with a synthetic `WindowId`:

**Decision tree per caller:**

1. Does the test actually need an `emWindow` at all? If the test only drives `emView::Update` / `UpdateEngineClass::Cycle`, it may not need a window — it just needs an engine registered for the view. With SP5, the engine can resolve the view via the panel-graph back-reference without a window at all.

2. If the test does need an `emWindow`: construct a real `emWindow` via `emWindow::new_popup_pending` (a real pending window, never materialized) or via the materialization test-helper path from W3 (the DISPLAY-gated integration tests already do this).

3. If neither fits: add a new test helper that wraps a pre-built `Rc<RefCell<emView>>` in a fake window stand-in — but the stand-in must carry a real `WindowId` from winit (obtainable only by creating a real window) or must not be needed for `UpdateEngineClass::Cycle` routing. Post-SP5, routing by `WindowId` may itself be obsolete if the engine can resolve the view directly; inspect `UpdateEngineClass::Cycle` in `emGUIFramework.rs` to confirm.

**Concrete migration pattern (most tests):**

```rust
// Before (used new_for_test):
let window = emWindow::new_for_test(/* synthetic id */, view, gpu_stub);

// After (no window; routes directly through view):
let cfg = Rc::new(RefCell::new(emCoreConfig::default()));
let mut tree = PanelTree::new();
let root = tree.create_root_deferred_view("root");
let view = Rc::new(RefCell::new(emView::new(root, 800.0, 600.0, cfg)));
tree.set_panel_view(root, Rc::downgrade(&view));
// Register the view's engine directly against the scheduler.
// (Details depend on UpdateEngineClass API post-SP4.)
```

- [ ] **Step 1: Migrate each caller**

Iterate through the list from Task 5.1. For each: apply the decision tree, rewrite the test setup.

- [ ] **Step 2: Delete the hacks**

Once no caller remains:

```rust
// crates/emcore/src/emWindow.rs
// Delete lines 97, 246, 377, 460, 479-486 (test_window_id field + all uses).
// Delete the entire emWindow::new_for_test constructor.
```

- [ ] **Step 3: Verify deletion**

```bash
grep -rn "test_window_id\|emWindow::new_for_test\|Window::new_for_test" crates/ examples/ 2>/dev/null
```

Expected: no matches.

- [ ] **Step 4: Tests green**

```bash
cargo-nextest ntr
```

Expected: 2433 / 0.

- [ ] **Step 5: Golden parity**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 / 6.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(5/n): delete emWindow::test_window_id and emWindow::new_for_test

SP4 test-identity hack retired. SP5's Rc<RefCell<emView>> ownership
provides real view identity via the panel-graph back-reference.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 6 — Divergence-note consolidation & closeout update

**Purpose:** Update `DIVERGED:` annotations and mark the closeout item closed.

### Task 6.1 — Rewrite the divergence block

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs:289-315` (delete dispatch-driver paragraph; retain only data-structure note; move near the `panels` field or delete entirely if nothing remains relevant).
- Modify: `crates/emcore/src/emView.rs` (add the data-structure divergence note adjacent to the new `notice_ring_head_next` fields; Task 3.1 placeholder wording already includes it — promote/tighten).

- [ ] **Step 1: Rewrite**

Replace the `emPanelTree.rs:289-315` block with:

```rust
/// Head of the notice-delivery ring.
///
/// NOTE: After SP5 (2026-04-18), notice-ring state lives on `emView`;
/// `PanelTree` no longer owns a head or tail. Per-panel ring linkage
/// (`notice_prev_in_ring`, `notice_next_in_ring` on `PanelData`)
/// remains here because it is arena-local.
```

(Or delete the block entirely if no fields remain on `PanelTree`.)

- [ ] **Step 2: Ensure emView.rs divergence note is final**

The note added in Task 3.1 should read:

```rust
/// 1:1 with C++ `emView::NoticeList` (emView.h:576).
///
/// DIVERGED: C++ uses `PanelRingNode *` intrusive sentinel linkage
/// (emView.h:576, emPanel.h:823). Rust uses `Option<PanelId>` head/tail
/// into the panel arena. Forced data-structure divergence (Rust
/// ownership rules make intrusive raw-pointer rings impractical
/// without `unsafe`). Ownership location (per-view) matches C++.
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
sp5(6.1/n): consolidate DIVERGED notes; dispatch-driver divergence closed

Data-structure divergence (Vec/arena-index vs PanelRingNode*) is the
only remaining forced divergence; lives on emView adjacent to NoticeList.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 6.2 — Update closeout doc

**Files:**
- Modify: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` (§8.0 SP5 row; §8.1 item 12; add new residual for `run_panel_cycles`).

- [ ] **Step 1: Mark §8.0 SP5 row complete**

Change `SP5 — Per-view notice dispatch | 12 | Blocked on multi-window roadmap decision | —` to:

```
SP5 — Per-view notice dispatch | 12 | **Complete 2026-04-18** (merged as `<SHA>`). | `specs/2026-04-18-emview-sp5-per-view-notice-dispatch-design.md`, `plans/2026-04-18-emview-sp5-per-view-notice-dispatch.md`
```

- [ ] **Step 2: Mark §8.1 item 12 closed**

Replace item 12's body with a CLOSED block mirroring the style of other closed items (e.g., item 11's SP4 block). Include: resolution summary (per-view dispatch, Rc<RefCell<emView>> cascade, Weak<RefCell<emView>> on emPanel, SP4 test hacks deleted), line-count note, test count delta, landing date.

- [ ] **Step 3: Add a new residual for `run_panel_cycles`**

Add to §8.1 as a new item (e.g., item 16):

```markdown
16. **[W5c / ARCH] `emPanel` engine registration port** — `emGUIFramework::about_to_wait` still calls `self.tree.run_panel_cycles(pixel_tallness)` with an arbitrarily-chosen window's pixel tallness (the same Rust-only-construct shortcut SP5 closed for notice dispatch). Fix: make `emPanel` register per-view as an engine via the scheduler, the way C++ panels participate in `emEngine`. Rust-only construct; load-bearing for multi-window correctness. Defer until multi-window roadmap activates; SP5 flagged this as next in line after per-view notice dispatch.
```

- [ ] **Step 4: Update §7 test counts**

Append: "Post-SP5: **2433/2433** (+1 multi-view notice-dispatch test; net zero from test-hack removal migrating existing tests)."

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
docs(closeout): mark SP5 complete; flag emPanel engine registration as follow-up

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 6.3 — Final full gate

- [ ] **Step 1: Full nextest**

```bash
cargo-nextest ntr 2>&1 | tail -5
```

Expected: 2433 / 0 / 9 skipped.

- [ ] **Step 2: Clippy**

```bash
cargo clippy -- -D warnings
```

Expected: clean.

- [ ] **Step 3: Golden full**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | tail -5
```

Expected: 237 / 6 (same baseline).

- [ ] **Step 4: Smoke**

```bash
timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"
```

Expected: `exit=124` or `exit=143`.

- [ ] **Step 5: Grep sentinel check**

```bash
grep -rn "test_window_id\|TODO(per-view-notice-dispatch)\|PanelTree::HandleNotice\|emWindow::new_for_test" crates/ examples/ 2>/dev/null
```

Expected: no matches.

- [ ] **Step 6: Mark plan complete; reconcile closeout doc; branch merge per finishing-a-development-branch skill.**

---

## Risks and rollback

| Phase | Risk | Rollback |
|---|---|---|
| 1 | Cascade exceeds 200 lines | Revert P1 commits; reconsider scope with user. |
| 2 | `debug_assert` fires in tests whose setup forgot `set_panel_view` | Fix test setup (not the assert); the assert is a genuine invariant. |
| 3 | Re-entrant `view.borrow_mut()` panic on `add_to_notice_list` path | Per-site investigation; restructure outer call site (SP4 `queue_or_apply` precedent). |
| 4 | Golden drift on single-window tests | Investigate via `scripts/diff_draw_ops.py <failing>`. Expected zero drift; any drift is a port bug. |
| 5 | A test genuinely cannot construct a non-hack window | Add a minimally-scoped `#[cfg(test)]` helper that yields `Rc<RefCell<emView>>` + fake `WindowId` *if and only if* `UpdateEngineClass::Cycle` still routes by `WindowId`. Document as a residual hack in the closeout doc; prefer not to reintroduce. |

Every phase commits incrementally. Rollback is `git revert` of the last N commits.
