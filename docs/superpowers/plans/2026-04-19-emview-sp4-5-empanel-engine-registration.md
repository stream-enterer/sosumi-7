# SP4.5 — emPanel Engine-Registration Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every `emPanel` a real scheduler engine (via a thin adapter), eliminate `PanelTree::cycle_list` and `run_panel_cycles`, and delete the framework's pick-first-window pixel-tallness shortcut. Closes closeout §8.1 item 16 as a full C++ port.

**Architecture:** Add `PanelCycleEngine` (adapter `impl emEngine`) and an `engine_id: Option<EngineId>` field on `PanelData`. Register the adapter eagerly in `PanelTree::init_panel_view` (the SP5-established "panel fully attached" moment), deregister in `PanelTree::remove`. Route WakeUp through `PanelCtx::wake_up{,_panel}` using SP4's existing `queue_or_apply_sched_op` pattern (extended with a `SchedOp::WakeUp`-for-panels case — already present — and a new `SchedOp::RemoveEngine`). Delete the Rust-only cycle machinery atomically once callers have migrated.

**Tech Stack:** Rust, slotmap, `Rc<RefCell<>>`/`Weak<RefCell<>>`, existing `EngineScheduler`/`emEngine` trait, existing SP5 `Weak<RefCell<emView>>` back-ref on panels.

**Spec:** `docs/superpowers/specs/2026-04-19-emview-sp4-5-empanel-engine-registration-design.md`

**Invariants (hold across every phase):**
- `cargo check` green after every task.
- Nextest green after every task (start: 2434/2434).
- Golden 237/6 unchanged — no pixel path touched.
- Smoke `timeout 20 cargo run --release --bin eaglemode` exits 124 or 143.
- Scheduler `Drop` assertion "no dangling engines" never fires in test teardown.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/emcore/src/emEngine.rs` | `emEngine` trait, `EngineId`, `EngineCtx`, scheduler internals | Unchanged |
| `crates/emcore/src/emScheduler.rs` | `EngineScheduler` (`register_engine`, `remove_engine`, `wake_up`, `do_time_slice`) | Unchanged |
| `crates/emcore/src/emPanel.rs` | `PanelBehavior` trait + `Cycle` method | Unchanged (signature stable) |
| `crates/emcore/src/emPanelCycleEngine.rs` | **New.** `PanelCycleEngine` adapter: `impl emEngine` that routes `Cycle` through `PanelTree::take_behavior` → `PanelBehavior::Cycle` | Create |
| `crates/emcore/src/lib.rs` | Module registration | Add `mod emPanelCycleEngine;` + re-export |
| `crates/emcore/src/emPanelTree.rs` | `PanelData::engine_id`, registration in `init_panel_view`, deregistration in `remove`; **delete** `cycle_list`/`Cycle`/`cancel_cycle`/`run_panel_cycles` | Modify |
| `crates/emcore/src/emPanelCtx.rs` | Add `wake_up`, `wake_up_panel` methods | Modify |
| `crates/emcore/src/emView.rs` | Extend `SchedOp` with `RemoveEngine(EngineId)`; `emView::scheduler_ref` accessor if missing | Modify |
| `crates/emcore/src/emGUIFramework.rs` | **Delete** pick-first-window pixel-tallness block + `tree.run_panel_cycles(...)` call | Modify |
| `crates/emcore/src/emFileSelectionBox.rs` | `ctx.tree.Cycle(ctx.id)` → `ctx.wake_up()` | Modify (1 call site) |
| `crates/emfileman/src/emDirEntryPanel.rs` | `ctx.tree.Cycle(child_id)` → `ctx.wake_up_panel(child_id)` | Modify (1 call site) |
| `crates/emfileman/src/emFileLinkPanel.rs` | same | Modify (1 call site) |
| `crates/emmain/src/emVirtualCosmos.rs` | same | Modify (1 call site) |
| `crates/emcore/tests/unit/sp4_5_panel_engine.rs` | **New.** Lifecycle + multi-view + mid-slice wake tests | Create |
| `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` | Mark SP4.5 complete in §8.0 and §8.1 item 16 | Modify (final phase) |

---

## Phase 1 — Adapter type + field (non-wired)

### Task 1.1: Add `engine_id: Option<EngineId>` to `PanelData`

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (struct definition near line 150; `PanelData::new` near line 228)

- [ ] **Step 1: Add import**

In `emPanelTree.rs`, near the existing imports at the top, add:
```rust
use super::emEngine::EngineId;
```

- [ ] **Step 2: Add the field to `PanelData`**

Find the `PanelData` struct (starts around line 150) and add — immediately after the `View:` field at the end of the struct — the new field:

```rust
    /// Scheduler engine handle for this panel (SP4.5).
    ///
    /// C++ `emPanel` inherits from `emEngine`; every panel is implicitly an
    /// engine from construction. In Rust the engine registration is done
    /// by `PanelTree::init_panel_view` via a `PanelCycleEngine` adapter.
    /// `None` until `init_panel_view` runs (panel not yet attached to a view).
    pub(crate) engine_id: Option<EngineId>,
```

- [ ] **Step 3: Initialize in `PanelData::new`**

Find `PanelData::new` (around line 228) and add `engine_id: None,` to the struct literal alongside the other field defaults.

- [ ] **Step 4: Build check**

Run: `cargo check --workspace`
Expected: PASS (field is present but unused — no warnings since it has `pub(crate)` and will be written by Task 2.2).

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emPanelTree.rs
git commit -m "sp4.5(1/n): add PanelData::engine_id field (unused)"
```

### Task 1.2: Create `PanelCycleEngine` adapter

**Files:**
- Create: `crates/emcore/src/emPanelCycleEngine.rs`
- Modify: `crates/emcore/src/lib.rs`

- [ ] **Step 1: Create the file**

Create `crates/emcore/src/emPanelCycleEngine.rs` with:

```rust
// DIVERGED: C++ emPanel inherits from emEngine directly (emPanel.h:33 —
// `class emPanel : public emEngine`). In Rust, `PanelBehavior` trait
// objects are owned by `PanelTree::panels` and are `take`n during cycling
// so the tree can lend a `PanelCtx`; a `PanelBehavior` therefore cannot
// simultaneously live in the scheduler's `Box<dyn emEngine>` slot-map.
// This adapter is the minimum concession: one adapter engine per panel,
// registered with the scheduler, whose `Cycle` drives the panel's
// `PanelBehavior::Cycle` via the standard take/put path.
//
// Observable behavior matches C++ (panel cycling runs through the
// scheduler's normal engine loop, uses the panel's own view's
// `CurrentPixelTallness`).

use std::cell::RefCell;
use std::rc::Weak;

use super::emEngine::{emEngine, EngineCtx};
use super::emPanelCtx::PanelCtx;
use super::emPanelTree::PanelId;
use super::emView::emView;

pub(crate) struct PanelCycleEngine {
    pub(crate) panel_id: PanelId,
    pub(crate) view: Weak<RefCell<emView>>,
}

impl emEngine for PanelCycleEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        // View gone (test teardown / window closed) → sleep.
        let Some(view_rc) = self.view.upgrade() else {
            return false;
        };
        let tallness = view_rc.borrow().GetCurrentPixelTallness();

        // Take the behavior off the tree, build a PanelCtx, drive Cycle,
        // put it back (if the panel still exists — behavior may have called
        // delete_self via ctx).
        let Some(mut behavior) = ctx.tree.take_behavior(self.panel_id) else {
            return false;
        };
        let mut pctx = PanelCtx::new(ctx.tree, self.panel_id, tallness);
        let stay_awake = behavior.Cycle(&mut pctx);
        if ctx.tree.panels.contains_key(self.panel_id) {
            ctx.tree.put_behavior(self.panel_id, behavior);
        }
        stay_awake
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/emcore/src/lib.rs`, add (in the module-declaration block, alphabetical with other `emPanel*` modules):
```rust
pub mod emPanelCycleEngine;
```

- [ ] **Step 3: Build check**

Run: `cargo check --workspace`
Expected: PASS. `PanelCycleEngine` is unused — may produce a `dead_code` warning at `pub(crate)`. If it does, leave it: Task 2.2 uses it within this same phase sequence.

If clippy complains about dead code: add `#[allow(dead_code)]` only if the warning is fatal under `-D warnings`. Remove the allow after Task 2.2.

- [ ] **Step 4: Test check**

Run: `cargo nextest run --workspace`
Expected: 2434 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emPanelCycleEngine.rs crates/emcore/src/lib.rs
git commit -m "sp4.5(2/n): add PanelCycleEngine adapter type (unused)"
```

---

## Phase 2 — Lifecycle: register in `init_panel_view`, deregister in `remove`

### Task 2.1: Add `SchedOp::RemoveEngine` variant

**Files:**
- Modify: `crates/emcore/src/emView.rs` (SchedOp enum around line 196, impl around line 204)

- [ ] **Step 1: Extend the enum**

Find the `SchedOp` enum (around line 196) and add a variant:

```rust
    /// Remove an engine from the scheduler. Panels deregister through this
    /// on tree removal (SP4.5). Queued when the scheduler is already
    /// borrowed (e.g. a panel deleting a sibling from inside `Cycle`).
    RemoveEngine(super::emEngine::EngineId),
```

- [ ] **Step 2: Extend `apply_to`**

In `impl SchedOp` at `apply_to` (around line 204), add:
```rust
            SchedOp::RemoveEngine(e) => sched.remove_engine(e),
```

- [ ] **Step 3: Extend `apply_to_ctx`**

At `apply_to_ctx` (around line 220), add — noting `EngineCtx` does **not** expose `remove_engine` directly, so the op cannot be applied mid-`Cycle` through `EngineCtx`. Route it through a thin new method on `EngineCtx` first.

In `crates/emcore/src/emEngine.rs`, after the existing `wake_up` method on `impl EngineCtx<'_>`:

```rust
    /// Remove an engine from the scheduler. Used by panel-tree removal
    /// reached through a SchedOp drain (SP4.5).
    pub fn remove_engine(&mut self, id: EngineId) {
        // Mirror EngineScheduler::remove_engine but against EngineCtxInner.
        // Drop from wake queues, then the slot-map entry.
        if let Some(eng) = self.scheduler.engines.get(id) {
            if eng.awake_state >= 0 {
                let queue_idx = (eng.priority as usize) * 2 + (eng.awake_state as usize);
                self.scheduler.wake_queues[queue_idx].retain(|&e| e != id);
            }
        }
        self.scheduler.engines.remove(id);
    }
```

Then in `emView.rs` `SchedOp::apply_to_ctx`:
```rust
            SchedOp::RemoveEngine(e) => ctx.remove_engine(e),
```

- [ ] **Step 4: Build check**

Run: `cargo check --workspace` — PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emView.rs crates/emcore/src/emEngine.rs
git commit -m "sp4.5(3/n): add SchedOp::RemoveEngine + EngineCtx::remove_engine"
```

### Task 2.2: Write the lifecycle test (registration via `init_panel_view`)

**Files:**
- Create: `crates/emcore/tests/unit/sp4_5_panel_engine.rs`
- Modify: `crates/emcore/tests/unit/main.rs` (or equivalent test-aggregator file — follow existing pattern)

- [ ] **Step 1: Check the test harness**

Run: `ls crates/emcore/tests/unit/ | head -20` and look at `main.rs` or `mod.rs` to see how existing per-file test modules are wired. Follow the same pattern. If `tests/unit/` doesn't exist as a harness, check `crates/emcore/tests/` for any `*.rs` integration-test entry points and mirror that.

- [ ] **Step 2: Create the test file**

Create `crates/emcore/tests/unit/sp4_5_panel_engine.rs` with:

```rust
//! SP4.5 — emPanel engine-registration lifecycle + dispatch tests.
//!
//! Asserts that every panel gets a scheduler engine at `init_panel_view`,
//! that removal deregisters it, and that panel cycling runs per-view with
//! the correct `CurrentPixelTallness`.

use std::cell::RefCell;
use std::rc::Rc;

use emcore::emPanelTree::PanelTree;
use emcore::emScheduler::EngineScheduler;
use emcore::emView::emView;

/// Build a minimal view+tree pair for testing. Returns (tree, view_rc,
/// scheduler_rc, root_id). Caller may further populate the tree.
fn make_view_for_test() -> (
    Rc<RefCell<PanelTree>>,
    Rc<RefCell<emView>>,
    Rc<RefCell<EngineScheduler>>,
    emcore::emPanelTree::PanelId,
) {
    // NOTE: the exact constructor names may differ; match them to what
    // `emView::new_for_test` / `PanelTree::create_root_deferred_view` provide.
    // SP5's test harness pattern is the reference.
    let sched = Rc::new(RefCell::new(EngineScheduler::new()));
    let (view_rc, tree_rc, root_id) = emView::new_for_test_with_tree(800.0, 600.0, sched.clone());
    (tree_rc, view_rc, sched, root_id)
}

#[test]
fn sp4_5_panel_engine_registered_at_init_panel_view() {
    let (tree_rc, _view, _sched, root) = make_view_for_test();
    let tree = tree_rc.borrow();
    let panel = tree.GetRec(root).expect("root present");
    assert!(
        panel.engine_id.is_some(),
        "root panel should have engine_id after init_panel_view"
    );
}

#[test]
fn sp4_5_child_panel_engine_registered_via_init_propagation() {
    let (tree_rc, view, sched, root) = make_view_for_test();
    let child = tree_rc.borrow_mut().create_child(root, "child");

    // create_child propagates View via parent_view clone (see emPanelTree.rs
    // `create_child` around line 552). SP4.5 makes it also register the
    // engine using that inherited view.
    let tree = tree_rc.borrow();
    let panel = tree.GetRec(child).expect("child present");
    assert!(
        panel.engine_id.is_some(),
        "child panel should have engine_id inherited via create_child"
    );
    // Engine present in scheduler.
    let eid = panel.engine_id.unwrap();
    assert!(
        sched.borrow().engine_exists(eid),
        "scheduler should have the registered engine"
    );
    drop(tree);
    let _ = view;
}

#[test]
fn sp4_5_panel_engine_deregistered_on_panel_removal() {
    let (tree_rc, _view, sched, root) = make_view_for_test();
    let child = tree_rc.borrow_mut().create_child(root, "child");
    let eid = tree_rc
        .borrow()
        .GetRec(child)
        .and_then(|p| p.engine_id)
        .expect("child has engine_id");

    tree_rc.borrow_mut().remove(child);

    assert!(
        !sched.borrow().engine_exists(eid),
        "scheduler must not hold a removed panel's engine"
    );
}
```

**Note for the engineer:** the helpers `emView::new_for_test_with_tree` and `EngineScheduler::engine_exists` may not exist. If they don't, either:
- (a) Add them in this same task as `#[cfg(any(test, feature = "test-support"))]`-gated helpers; or
- (b) Use whatever SP5's multi-view test (`sp5_per_view_notice_dispatch_uses_correct_pixel_tallness`) uses — read that test first (`cargo nextest list | grep sp5_per_view`, then find its file) and mirror its harness exactly.

Prefer (b) — SP5's harness already solved this problem.

- [ ] **Step 3: Wire into test harness**

If `tests/unit/` uses a `mod sp4_5_panel_engine;` declaration in `main.rs` (or equivalent), add it. Follow SP5's pattern verbatim.

- [ ] **Step 4: Run tests — expect FAIL**

Run: `cargo nextest run -p emcore sp4_5_panel_engine`
Expected: FAIL — `engine_id` is `None`; nothing registers it yet.

- [ ] **Step 5: Commit the failing tests**

```bash
git add crates/emcore/tests/unit/sp4_5_panel_engine.rs crates/emcore/tests/unit/main.rs
git commit -m "sp4.5(4/n): lifecycle tests (failing — TDD red)"
```

If the pre-commit hook runs nextest and fails on the new tests, use `git commit --no-verify` is **forbidden** per `CLAUDE.md`. Instead: skip the commit, proceed to Task 2.3 directly, and commit test + implementation together in one step (a TDD compromise allowed by the "don't skip hooks" rule). Adjust this and the next task's commit boundaries accordingly.

### Task 2.3: Register adapter engine in `init_panel_view`

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (`init_panel_view` around line 519, `create_child` around line 552)

- [ ] **Step 1: Import the adapter + scheduler**

At the top of `emPanelTree.rs`, add:
```rust
use super::emPanelCycleEngine::PanelCycleEngine;
use super::emEngine::Priority;
```

- [ ] **Step 2: Helper: register one panel**

Add a private helper on `impl PanelTree` near `init_panel_view`:

```rust
    /// Register `id`'s scheduler engine if the panel has a live view and
    /// does not already have one. Called from `init_panel_view` and its
    /// descendant walk.
    fn register_engine_for(&mut self, id: PanelId) {
        if self.panels.get(id).and_then(|p| p.engine_id).is_some() {
            return; // idempotent re-attachment guard
        }
        let Some(view_weak) = self
            .panels
            .get(id)
            .map(|p| p.View.clone())
            .filter(|w| w.strong_count() > 0)
        else {
            return; // no view yet (or view dropped)
        };
        let Some(view_rc) = view_weak.upgrade() else {
            return;
        };
        let Some(sched_rc) = view_rc.borrow().scheduler_ref().cloned() else {
            return; // unit-test bare view with no scheduler
        };
        let adapter = PanelCycleEngine {
            panel_id: id,
            view: view_weak,
        };
        let eid = sched_rc
            .borrow_mut()
            .register_engine(Priority::Medium, Box::new(adapter));
        self.panels[id].engine_id = Some(eid);
    }
```

**Note:** `emView::scheduler_ref()` returning `Option<&Rc<RefCell<EngineScheduler>>>` must exist. Check `emView.rs` — SP4 likely added it. If it doesn't exist, add:
```rust
pub fn scheduler_ref(&self) -> Option<&Rc<RefCell<super::emScheduler::EngineScheduler>>> {
    self.scheduler.as_ref()
}
```

- [ ] **Step 3: Register in `init_panel_view`**

Modify `init_panel_view` (around line 519) to register on each panel as it walks descendants:

```rust
    pub fn init_panel_view(
        &mut self,
        id: PanelId,
        view: std::rc::Weak<std::cell::RefCell<crate::emView::emView>>,
    ) {
        self.panels[id].View = view.clone();
        self.register_engine_for(id);
        let mut stack = vec![id];
        while let Some(p) = stack.pop() {
            let mut child = self.panels[p].first_child;
            while let Some(c) = child {
                self.panels[c].View = view.clone();
                self.register_engine_for(c);
                stack.push(c);
                child = self.panels[c].next_sibling;
            }
        }
    }
```

- [ ] **Step 4: Register in `create_child`**

Modify `create_child` (around line 552) so children created after the parent is attached get registered immediately. Find the line that sets `self.panels[id].View = parent_view;` and add immediately after it:

```rust
        if self.panels[id].View.strong_count() > 0 {
            self.register_engine_for(id);
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p emcore sp4_5_panel_engine`
Expected: registration tests PASS; removal test FAILS.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emPanelTree.rs crates/emcore/src/emView.rs
git commit -m "sp4.5(5/n): register PanelCycleEngine on init_panel_view + create_child"
```

### Task 2.4: Deregister on panel removal

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (`remove` method around line 593)

- [ ] **Step 1: Deregistration helper**

Add alongside `register_engine_for`:

```rust
    /// Deregister `id`'s scheduler engine. Uses `queue_or_apply_sched_op`
    /// on the owning view so a panel removed from inside a sibling's
    /// `Cycle` (scheduler borrowed) defers the removal to after the slice.
    fn deregister_engine_for(&mut self, id: PanelId) {
        let Some(eid) = self
            .panels
            .get_mut(id)
            .and_then(|p| p.engine_id.take())
        else {
            return;
        };
        let Some(view_rc) = self
            .panels
            .get(id)
            .and_then(|p| p.View.upgrade())
        else {
            // View gone; scheduler may still hold the entry. Best-effort:
            // nothing to reach the scheduler through. Accept the leak —
            // view teardown would have drained engines anyway.
            return;
        };
        view_rc
            .borrow_mut()
            .queue_or_apply_sched_op(crate::emView::SchedOp::RemoveEngine(eid));
    }
```

- [ ] **Step 2: Call from `remove`**

In `PanelTree::remove` (around line 593), **before** the arena removal block (before the `// Remove from arena and name index` comment around line 635), deregister engines for all panels being removed:

```rust
        // SP4.5: deregister scheduler engines for self and descendants.
        for &desc in &descendants {
            self.deregister_engine_for(desc);
        }
        self.deregister_engine_for(id);
```

The call must come after `remove_from_notice_list` (existing behavior) and before the arena `self.panels.remove(...)` calls so the `View` weak ref is still reachable.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p emcore sp4_5_panel_engine`
Expected: ALL lifecycle tests PASS (3/3).

- [ ] **Step 4: Full test pass**

Run: `cargo nextest run`
Expected: 2434 previous + 3 new = 2437 passed. (The 4th test — mid-slice wake — is added in Phase 5.)

Also run: `cargo test --test golden -- --test-threads=1`
Expected: 237 passed / 6 failed (unchanged baseline).

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emPanelTree.rs
git commit -m "sp4.5(6/n): deregister PanelCycleEngine on panel removal"
```

---

## Phase 3 — WakeUp surface + call-site migration

### Task 3.1: Add `PanelCtx::wake_up{,_panel}`

**Files:**
- Modify: `crates/emcore/src/emPanelCtx.rs`

- [ ] **Step 1: Add the methods**

At the end of `impl<'a> PanelCtx<'a>` (before the closing `}` around line 287), add:

```rust
    /// Wake this panel's scheduler engine.
    ///
    /// C++ parity: `emPanel` inherits `WakeUp()` from `emEngine`. Calls to
    /// `WakeUp()` on `this` from panel methods map here.
    pub fn wake_up(&mut self) {
        self.wake_up_panel(self.id);
    }

    /// Wake another panel's scheduler engine.
    ///
    /// C++ parity: `otherPanel->WakeUp()` from inside a panel method.
    pub fn wake_up_panel(&mut self, id: crate::emPanelTree::PanelId) {
        let Some(panel) = self.tree.GetRec(id) else {
            return;
        };
        let Some(eid) = panel.engine_id else {
            return; // no view yet — panel not attached
        };
        let Some(view_rc) = panel.View.upgrade() else {
            return;
        };
        view_rc
            .borrow_mut()
            .queue_or_apply_sched_op(crate::emView::SchedOp::WakeUp(eid));
    }
```

- [ ] **Step 2: Build check**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 3: Test pass**

Run: `cargo nextest run`
Expected: 2437 passed (no new tests exercise these yet).

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emPanelCtx.rs
git commit -m "sp4.5(7/n): PanelCtx::wake_up / wake_up_panel"
```

### Task 3.2: Migrate the 4 `ctx.tree.Cycle(...)` call sites

**Files:**
- Modify: `crates/emcore/src/emFileSelectionBox.rs:1206`
- Modify: `crates/emfileman/src/emDirEntryPanel.rs:208`
- Modify: `crates/emfileman/src/emFileLinkPanel.rs:129`
- Modify: `crates/emmain/src/emVirtualCosmos.rs:624`

- [ ] **Step 1: Migrate `emFileSelectionBox.rs:1206`**

Change:
```rust
        ctx.tree.Cycle(ctx.id);
```
to:
```rust
        ctx.wake_up();
```

- [ ] **Step 2: Migrate `emDirEntryPanel.rs:208`**

Change:
```rust
                ctx.tree.Cycle(child_id);
```
to:
```rust
                ctx.wake_up_panel(child_id);
```

- [ ] **Step 3: Migrate `emFileLinkPanel.rs:129`**

Same pattern as Step 2:
```rust
                ctx.wake_up_panel(child_id);
```

- [ ] **Step 4: Migrate `emVirtualCosmos.rs:624`**

Same pattern:
```rust
        ctx.wake_up_panel(child_id);
```

- [ ] **Step 5: Full test pass**

Run: `cargo nextest run && cargo test --test golden -- --test-threads=1`
Expected: 2437 passed; golden 237/6.

- [ ] **Step 6: Smoke test**

Run: `timeout 20 cargo run --release --bin eaglemode`
Expected: exits 124 or 143 (stays alive 20 s).

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/src/emFileSelectionBox.rs crates/emfileman/src/emDirEntryPanel.rs crates/emfileman/src/emFileLinkPanel.rs crates/emmain/src/emVirtualCosmos.rs
git commit -m "sp4.5(8/n): migrate 4 ctx.tree.Cycle(id) sites to ctx.wake_up*"
```

---

## Phase 4 — Delete the Rust-only cycle machinery

### Task 4.1: Delete `cycle_list`, `Cycle`, `cancel_cycle`, `run_panel_cycles`

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs`

- [ ] **Step 1: Delete the `cycle_list` field**

In `PanelTree` struct definition (around line 276), delete the `cycle_list: Vec<PanelId>,` line.

- [ ] **Step 2: Delete field initialization**

In `PanelTree::new` (the `cycle_list: Vec::new(),` line around line 334), delete it.

- [ ] **Step 3: Delete methods**

Delete entire method bodies for:
- `pub fn Cycle(&mut self, id: PanelId)` (around line 1540–1545)
- `pub fn cancel_cycle(&mut self, id: PanelId)` (around line 1547–1549)
- `pub fn run_panel_cycles(&mut self, current_pixel_tallness: f64)` (around line 1556–1574)
- The doc-comment lines immediately above each

Also delete the `self.cycle_list.retain(...)` block in `PanelTree::remove` (around line 634–636, the "Remove all descendants and the panel itself from the cycle list" block).

- [ ] **Step 4: Build check**

Run: `cargo check --workspace`
Expected: PASS. If any error references `cycle_list`, `PanelTree::Cycle`, `cancel_cycle`, or `run_panel_cycles` — that call site was missed and must be migrated or deleted. Re-grep:
```bash
```
Use Grep tool for: `cycle_list|tree\.Cycle\(|cancel_cycle|run_panel_cycles` across `crates/`. Expect zero matches outside deleted code.

- [ ] **Step 5: Test pass**

Run: `cargo nextest run`
Expected: 2437 passed.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emPanelTree.rs
git commit -m "sp4.5(9/n): delete PanelTree::cycle_list / Cycle / cancel_cycle / run_panel_cycles"
```

### Task 4.2: Delete framework pick-first-window tallness block

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs` (around lines 503–516)

- [ ] **Step 1: Delete the block**

In `emGUIFramework::about_to_wait`, delete this entire block (around lines 503–516):

```rust
        // run_panel_cycles is a Rust-only construct (emPanel does not register
        // per-view as an engine in the Rust port yet; SP4.5). Uses an arbitrary
        // window's pixel_tallness — the same shortcut the notice path used before SP5.
        let panel_cycle_pixel_tallness = self
            .windows
            .values()
            .next()
            .map(|rc| rc.borrow().view().GetCurrentPixelTallness())
            .unwrap_or(1.0);

        // Run per-frame panel cycles
        self.tree.run_panel_cycles(panel_cycle_pixel_tallness);
```

Replace with a one-line comment marking the deletion:

```rust
        // SP4.5: panel cycling runs through the scheduler's normal engine
        // loop (each emPanel registers a PanelCycleEngine). No framework
        // shortcut needed; per-view CurrentPixelTallness is read by each
        // adapter at Cycle time.
```

- [ ] **Step 2: Build check**

Run: `cargo check --workspace`
Expected: PASS. If `self.tree` is still referenced but otherwise unused in `about_to_wait`, that's fine (other code still uses it).

- [ ] **Step 3: Full verification**

Run: `cargo nextest run && cargo test --test golden -- --test-threads=1 && timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"`
Expected: 2437 nextest; 237/6 golden; smoke exits 124 or 143.

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emGUIFramework.rs
git commit -m "sp4.5(10/n): delete framework pick-first-window pixel-tallness shortcut"
```

---

## Phase 5 — Parity tests

### Task 5.1: Multi-view per-tallness dispatch test

**Files:**
- Modify: `crates/emcore/tests/unit/sp4_5_panel_engine.rs`

- [ ] **Step 1: Add the test**

Append to `sp4_5_panel_engine.rs`:

```rust
use std::cell::Cell;

/// Behavior that records its most-recent PanelCtx tallness on every Cycle.
struct TallnessRecordingBehavior {
    last_tallness: Rc<Cell<f64>>,
}
impl emcore::emPanel::PanelBehavior for TallnessRecordingBehavior {
    fn Cycle(&mut self, ctx: &mut emcore::emPanelCtx::PanelCtx) -> bool {
        self.last_tallness.set(ctx.current_pixel_tallness);
        false // sleep after one tick
    }
}

#[test]
fn sp4_5_panel_cycle_uses_per_view_pixel_tallness() {
    // Two views with distinct pixel tallness, each hosting one panel
    // registered for cycling. After a single DoTimeSlice, each panel's
    // recorded tallness must match its own view's — not a shared fallback.
    let sched = Rc::new(RefCell::new(EngineScheduler::new()));

    // Mirror SP5's two-view harness verbatim; read that test for the
    // precise construction calls. The shape below is the intended assertion.
    let (view_a, tree_a, root_a) = emView::new_for_test_with_tree(800.0, 600.0, sched.clone());
    let (view_b, tree_b, root_b) = emView::new_for_test_with_tree(800.0, 600.0, sched.clone());
    view_a.borrow_mut().SetCurrentPixelTallness(1.5);
    view_b.borrow_mut().SetCurrentPixelTallness(0.5);

    let rec_a = Rc::new(Cell::new(0.0));
    let rec_b = Rc::new(Cell::new(0.0));
    tree_a.borrow_mut().set_behavior(
        root_a,
        Box::new(TallnessRecordingBehavior { last_tallness: rec_a.clone() }),
    );
    tree_b.borrow_mut().set_behavior(
        root_b,
        Box::new(TallnessRecordingBehavior { last_tallness: rec_b.clone() }),
    );

    // Wake each via its PanelCtx (a fresh context, deadline irrelevant here).
    {
        let mut t = tree_a.borrow_mut();
        let mut ctx = emcore::emPanelCtx::PanelCtx::new(&mut t, root_a, 1.0);
        ctx.wake_up();
    }
    {
        let mut t = tree_b.borrow_mut();
        let mut ctx = emcore::emPanelCtx::PanelCtx::new(&mut t, root_b, 1.0);
        ctx.wake_up();
    }

    // Drain any queued SchedOps produced by wake_up calls (SP4 pattern).
    view_a.borrow_mut().drain_pending_sched_ops();
    view_b.borrow_mut().drain_pending_sched_ops();

    // Drive one slice.
    let mut tree_a_ref = tree_a.borrow_mut();
    let mut tree_b_ref = tree_b.borrow_mut();
    let mut windows = std::collections::HashMap::new();
    // HashMap construction may need real WindowIds — mirror SP5 exactly.
    sched
        .borrow_mut()
        .do_time_slice(&mut tree_a_ref, &mut windows);
    drop(tree_a_ref);
    sched
        .borrow_mut()
        .do_time_slice(&mut tree_b_ref, &mut windows);
    drop(tree_b_ref);

    assert_eq!(rec_a.get(), 1.5, "view A panel saw wrong tallness");
    assert_eq!(rec_b.get(), 0.5, "view B panel saw wrong tallness");
}
```

**Note:** `do_time_slice` takes both a tree and windows map. Each view has its own tree. SP5's two-view test solves this — **read `sp5_per_view_notice_dispatch_uses_correct_pixel_tallness` first and mirror its structure exactly**. The pseudocode above is a shape sketch, not a verbatim template.

`drain_pending_sched_ops` may or may not exist publicly — SP4 added the queue; check how existing SP4 tests drain it. If there's no public drainer, construction of a full test may require one; add it as `#[cfg(any(test, feature = "test-support"))]`.

- [ ] **Step 2: Run — expect PASS**

Run: `cargo nextest run -p emcore sp4_5_panel_cycle_uses_per_view_pixel_tallness`
Expected: PASS. Failure means the adapter isn't reading tallness from `self.view` — revisit `PanelCycleEngine::Cycle`.

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/tests/unit/sp4_5_panel_engine.rs
git commit -m "sp4.5(11/n): multi-view per-tallness dispatch test"
```

### Task 5.2: Mid-slice wake-up-panel test

**Files:**
- Modify: `crates/emcore/tests/unit/sp4_5_panel_engine.rs`

- [ ] **Step 1: Add the test**

Append:

```rust
/// Behavior A wakes panel B (by id) once from its first Cycle.
struct SiblingWaker {
    sibling: emcore::emPanelTree::PanelId,
    did_wake: Rc<Cell<bool>>,
}
impl emcore::emPanel::PanelBehavior for SiblingWaker {
    fn Cycle(&mut self, ctx: &mut emcore::emPanelCtx::PanelCtx) -> bool {
        if !self.did_wake.get() {
            ctx.wake_up_panel(self.sibling);
            self.did_wake.set(true);
        }
        false
    }
}

/// Behavior B records that its Cycle ran.
struct CycleRecorder {
    ran: Rc<Cell<bool>>,
}
impl emcore::emPanel::PanelBehavior for CycleRecorder {
    fn Cycle(&mut self, _ctx: &mut emcore::emPanelCtx::PanelCtx) -> bool {
        self.ran.set(true);
        false
    }
}

#[test]
fn sp4_5_wake_up_panel_from_cycle_reaches_sibling() {
    // Panel A's Cycle wakes sibling B. After a do_time_slice — possibly
    // plus a pending_sched_ops drain — B's Cycle must have run.
    // (C++ observable: mid-slice WakeUp; Rust observable via the SchedOp
    //  queue-drain pattern: same-or-next slice.)
    let (tree_rc, view_rc, sched, root) = make_view_for_test();

    let a = tree_rc.borrow_mut().create_child(root, "a");
    let b = tree_rc.borrow_mut().create_child(root, "b");
    let ran_b = Rc::new(Cell::new(false));
    let did_wake = Rc::new(Cell::new(false));
    tree_rc.borrow_mut().set_behavior(
        a,
        Box::new(SiblingWaker { sibling: b, did_wake: did_wake.clone() }),
    );
    tree_rc.borrow_mut().set_behavior(
        b,
        Box::new(CycleRecorder { ran: ran_b.clone() }),
    );

    // Wake A.
    {
        let mut t = tree_rc.borrow_mut();
        let mut ctx = emcore::emPanelCtx::PanelCtx::new(&mut t, a, 1.0);
        ctx.wake_up();
    }
    view_rc.borrow_mut().drain_pending_sched_ops();

    // Slice 1: A runs, queues WakeUp(B). B likely runs next slice.
    let mut windows = std::collections::HashMap::new();
    sched
        .borrow_mut()
        .do_time_slice(&mut tree_rc.borrow_mut(), &mut windows);
    view_rc.borrow_mut().drain_pending_sched_ops();
    sched
        .borrow_mut()
        .do_time_slice(&mut tree_rc.borrow_mut(), &mut windows);

    assert!(did_wake.get(), "A should have woken B");
    assert!(ran_b.get(), "B should have cycled after A woke it");
}
```

- [ ] **Step 2: Run — expect PASS**

Run: `cargo nextest run -p emcore sp4_5_wake_up_panel_from_cycle_reaches_sibling`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/tests/unit/sp4_5_panel_engine.rs
git commit -m "sp4.5(12/n): mid-slice wake_up_panel test"
```

### Task 5.3: Full verification sweep

- [ ] **Step 1: Full nextest**

Run: `cargo nextest run`
Expected: 2438 passed, 0 failed (2434 prior + 4 new).

- [ ] **Step 2: Golden**

Run: `cargo test --test golden -- --test-threads=1`
Expected: 237 passed / 6 failed (same 6 pre-existing — no new failures, no new passes).

- [ ] **Step 3: Smoke**

Run: `timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"`
Expected: `exit=124` or `exit=143`.

- [ ] **Step 4: Clippy under the hook**

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Residual grep**

Use Grep tool, pattern: `cycle_list|run_panel_cycles|PanelTree::Cycle\b|cancel_cycle`, path: `crates/`.
Expected: zero matches (anywhere outside deleted code).

- [ ] **Step 6: Divergence-marker inventory**

Use Grep: `DIVERGED:.*SP4\.5|PanelCycleEngine`, path: `crates/`.
Expected: exactly one annotated `DIVERGED:` location (the adapter struct in `emPanelCycleEngine.rs`).

---

## Phase 6 — Closeout doc update

### Task 6.1: Update `2026-04-18-emview-subsystem-closeout.md`

**Files:**
- Modify: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md`

- [ ] **Step 1: Mark §8.0 SP4.5 row complete**

In the §8.0 sub-project table, change the `SP4.5` row's **State** column from "Not started; ARCH; …" to a completed state mirroring SP5's row, naming this plan's final merge commit SHA and the new DIVERGED marker.

- [ ] **Step 2: Close §8.1 item 16**

Wrap the body of §8.1 item 16 in `~~…~~` strike-through and add a closing summary paragraph (same pattern as SP5's item 12 closure):

```markdown
**CLOSED 2026-04-19 by SP4.5** (merge commit `<sha>`, branch
`sp4.5-empanel-engine-registration`). Resolution: Option B (full port).
`emPanel` now owns an `engine_id: Option<EngineId>`; registration via
`PanelCycleEngine` adapter (one forced `DIVERGED:` annotation citing
`emPanel.h:33` and the single-inheritance + slot-map ownership reason)
happens in `PanelTree::init_panel_view` and `create_child`. Deregistration
via `queue_or_apply_sched_op(SchedOp::RemoveEngine(eid))` in
`PanelTree::remove`. `PanelTree::cycle_list`, `Cycle`, `cancel_cycle`, and
`run_panel_cycles` deleted; `emGUIFramework::about_to_wait` pick-first-
window pixel-tallness block deleted. Multi-window pixel-tallness bug
fixed by construction — each adapter reads its own view's tallness at
Cycle time. 4 call-site migrations (`emFileSelectionBox`, `emDirEntryPanel`,
`emFileLinkPanel`, `emVirtualCosmos`): `ctx.tree.Cycle(id)` →
`ctx.wake_up_panel(id)` / `ctx.wake_up()`. Tests: 2434 → 2438 (+4:
lifecycle registration, child-inheritance, removal-deregistration,
multi-view per-tallness, mid-slice sibling wake). Golden 237/6 unchanged.
```

- [ ] **Step 3: Update test counts in §1 and §7.4**

In §1 status row "Tests": update count from 2434/2434 to 2438/2438 and append the 4 new test names.

In §7.4, add a post-SP4.5 line:
```markdown
- Post-SP4.5 (2026-04-19): **2438/2438** (+4: `sp4_5_panel_engine_registered_at_init_panel_view`, `sp4_5_child_panel_engine_registered_via_init_propagation`, `sp4_5_panel_engine_deregistered_on_panel_removal`, `sp4_5_panel_cycle_uses_per_view_pixel_tallness`, `sp4_5_wake_up_panel_from_cycle_reaches_sibling`). Baseline golden 237/6 unchanged.
```

(Five tests, not four — the file inherits three lifecycle tests plus two parity tests. Correct the count if the count claim conflicts.)

- [ ] **Step 4: Update §6 markers table**

In the `DIVERGED:` row: bump the count by +1 and append to the list: `1 SP4.5 (PanelCycleEngine adapter: emPanel→emEngine single-inheritance + slot-map ownership)`.

- [ ] **Step 5: Update §8.0 suggested execution order**

Change:
```
~~SP1~~ → ~~SP3~~ → ~~SP4~~ → ~~SP5~~ → SP4.5 …
```
to:
```
~~SP1~~ → ~~SP3~~ → ~~SP4~~ → ~~SP5~~ → ~~SP4.5~~ → SP6 if wanted → SP7 when the motivation arrives.
```

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md
git commit -m "sp4.5(13/n): closeout doc — SP4.5 complete"
```

---

## Final state

| Axis | Before | After |
|---|---|---|
| `PanelTree::cycle_list` | field present | deleted |
| `PanelTree::{Cycle,cancel_cycle,run_panel_cycles}` | present | deleted |
| `emGUIFramework::about_to_wait` pick-first-window block | present | deleted |
| `PanelData::engine_id` | — | `Option<EngineId>`, set by `init_panel_view` |
| `PanelCycleEngine` adapter | — | `crates/emcore/src/emPanelCycleEngine.rs`, one `DIVERGED:` annotation |
| `ctx.tree.Cycle(id)` call sites | 4 | 0 (migrated to `ctx.wake_up{,_panel}`) |
| Nextest | 2434/2434 | 2438/2438 |
| Golden | 237/6 | 237/6 |
| Rust-only "cycle list" concept | present | eliminated |

---

## Self-review

**Spec coverage check** (against `docs/superpowers/specs/2026-04-19-emview-sp4-5-empanel-engine-registration-design.md`):

| Spec §/item | Task(s) |
|---|---|
| §4.1 per-panel engine, eager registration | 1.1, 2.3 |
| §4.2 adapter type with DIVERGED annotation | 1.2 |
| §4.3 `PanelCtx::wake_up{,_panel}` | 3.1 |
| §4.3 delete cycle_list family | 4.1 |
| §4.3 delete framework block | 4.2 |
| §4.3 migrate 4 call sites | 3.2 |
| §4.4 lifecycle (`init_panel_view`, `remove`, drop) | 2.3, 2.4 |
| §4.5 re-entrancy (queue-or-apply via SchedOp) | 2.1, 2.4, 3.1 |
| §5 one surviving DIVERGED | 1.2 (annotation), 5.3 (grep check) |
| §6 tests: 4 new | 2.2 (3 lifecycle), 5.1 (tallness), 5.2 (mid-slice) |
| §8 rollout phases | Phases 1–6 |

**Placeholder scan:** No "TBD"/"TODO"/"fill in" markers. Concrete code for every step. Test helpers (`new_for_test_with_tree`, `engine_exists`, `drain_pending_sched_ops`) flagged inline with explicit fallback instructions ("mirror SP5 harness").

**Type/name consistency:**
- `engine_id: Option<EngineId>` — used consistently.
- `PanelCycleEngine` — consistent.
- `wake_up()`, `wake_up_panel(id)` — consistent.
- `SchedOp::RemoveEngine(EngineId)` — consistent across Task 2.1, 2.4.
- `register_engine_for`, `deregister_engine_for` — consistent.
- `scheduler_ref()` — flagged as possibly-needs-adding in Task 2.3 with concrete signature.

End of plan.
