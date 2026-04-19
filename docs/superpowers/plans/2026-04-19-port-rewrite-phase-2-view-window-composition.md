# Phase 2 — View/Window Composition + Back-Ref Migration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans.

**Goal:** Replace `Rc<RefCell<emView>>` / `Weak<RefCell<emView>>` / `Rc<RefCell<emWindow>>` / `Weak<RefCell<emWindow>>` with composition-owned `emView`/`emWindow` plain values and ID-based back-references resolved through `EngineCtx`. Relocate NoticeList back to emView. Consolidate focus on emView. Install pending-popups map for cancellation. Enable `SwapViewPorts` via `HashMap::get_disjoint_mut`.

**Architecture:** `emWindow` owns `view: emView`. `emSubViewPanel` owns `sub_view: emView` + `sub_tree: PanelTree`. Engines previously holding `Weak<RefCell<emView>>` now hold `window_id: WindowId` (top-level) or `PanelScope::SubView(PanelId)` (sub-view) and resolve through `ctx.with_view_mut`. Framework's `windows: HashMap<WindowId, emWindow>` supports both direct field access and cross-window disjoint borrows via `get_disjoint_mut`.

**Tech Stack:** Rust stable 1.86+ (for `HashMap::get_disjoint_mut`). Existing crates.

**Companion documents:**
- Spec: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §2 P2, §3.2, §3.7 (popup), §5 D5.1–D5.6.
- Bootstrap/closeout ritual: `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md`.

**Spec sections implemented:** §3.2, §3.7 (popup cancellation), §5 D5.1–D5.6.

**JSON entries closed:** E006, E014, E015, E038.

**Phase-specific invariants (C4):**
- **I2.** `rg 'Rc<RefCell<emView>>' crates/` returns zero matches in production (non-test) code. `#[cfg(test)]` helpers under `crates/eaglemode/tests/` may retain shape only where they model external C++-test fixtures — enumerate such holdouts in the closeout and schedule for removal by Phase 5.
- **I2a.** `rg 'Weak<RefCell<emView>>' crates/` returns zero matches anywhere.
- **I2b.** `rg 'Rc<RefCell<emWindow>>|Weak<RefCell<emWindow>>' crates/` returns zero matches.
- **I2c.** `rg 'view_rc|sub_view_rc' crates/` returns zero matches (the accessors are deleted).
- **I6 (partial).** Golden baseline 237/6 (or better) preserved.
- **NoticeList location check.** `rg 'notice_list|NoticeList' crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs` shows NoticeList on `emView` and **not** on `PanelTree`.

**Entry-precondition.** Phase 1 Closeout COMPLETE. `EngineCtx::windows: &mut HashMap<WindowId, emWindow>` already carries plain-value `emWindow` in its type (Phase 1 Task 2 wrote `HashMap<WindowId, emWindow>`). Phase 2 changes what lives *inside* `emWindow`.

---

## Bootstrap (per shared ritual)

Run B1–B12 with `<N>` = `2`. At B4, verify `docs/superpowers/notes/2026-04-19-phase-1-closeout.md` has `Status: COMPLETE`.

---

## File Structure

**Files heavily modified:**
- `crates/emcore/src/emWindow.rs` — `view: Rc<RefCell<emView>>` → `view: emView`; delete `view_rc` accessor; constructor takes `emView` by value.
- `crates/emcore/src/emSubViewPanel.rs` — `sub_view: Rc<RefCell<emView>>` → `sub_view: emView`; delete `sub_view_rc`; `sub_tree` already plain (confirm).
- `crates/emcore/src/emView.rs` — re-home NoticeList here (moving back from `PanelTree` per SP5). Consolidate focus. Reinstate any `DIVERGED:` deletions.
- `crates/emcore/src/emViewPort.rs` — `window: Option<Weak<RefCell<emWindow>>>` → `window_id: Option<WindowId>`. Delete `focused: bool` duplicate field.
- `crates/emcore/src/emPanelCycleEngine.rs:34` — `view: Weak<RefCell<emView>>` → `window_id: WindowId` (top-level) or `scope: PanelScope` (sub-view).
- `crates/emcore/src/emPanelTree.rs` — NoticeList field deleted; `register_engine_for`'s behavior already ctx-threaded in Phase 1.
- `crates/emcore/src/emFileModel.rs:900` — the `HashMap<WindowId, Rc<RefCell<emWindow>>>` inside `FileModelClient` drops the Rc/RefCell wrapper (already migrated by Phase 1's `windows` signature change — confirm).
- `crates/emcore/src/emGUIFramework.rs` — add `pending_popups: HashMap<WindowId, emWindow>` for popup cancellation (§3.7); migrate `emGUIFramework.rs:208` (`win_rc: Rc<RefCell<emWindow>>`) parameter to `win: emWindow` plain.
- `crates/emcore/src/emGUIFramework.rs:419` — comment about "closures own their captured Rc<RefCell<emWindow>>" — rewrite to document the pending-popups-map model.

**New files:**
- `crates/emcore/src/emPanelScope.rs` — `pub enum PanelScope { Toplevel(WindowId), SubView(PanelId) }` plus resolver methods that return `&mut emView` through ctx.
- Marker: `crates/emcore/src/emPanelScope.rust_only`.

**Test files modified:**
- `crates/eaglemode/tests/unit/popup_materialization.rs:43` — `popup_window: Option<Rc<RefCell<emWindow>>>` becomes `popup_window_id: Option<WindowId>` plus framework access.
- `crates/eaglemode/tests/unit/popup_cancel_before_materialize.rs:53` — `popup_weak: Option<Weak<RefCell<emWindow>>>` becomes `popup_window_id: Option<WindowId>`; cancellation tested via `framework.pending_popups.contains_key(&wid)`.
- `crates/eaglemode/tests/golden/composition.rs:75` — rewrite the comment and fixture to match new composition.

**Files where DIVERGED blocks are deleted:**
- `emView.rs:238` — "Rust holds `Weak<RefCell<emView>>` since …" — delete.
- `emView.rs:3465` (NoticeList SP5 relocation DIVERGED block) — delete.
- `emViewPort.rs:5` — "`connects to nothing` model becomes a `Weak<RefCell<emWindow>>` back-reference" — delete; rewrite comment to describe WindowId-based resolution.
- `emViewPort.rs:43` — the rewritten rationale (already landed per commit history) stays; the portion referencing `Weak<RefCell<emView>>` as an alternative is deleted.
- `emViewPort.rs:244` — "the back-reference is `Weak<RefCell<emWindow>>`" — rewrite.
- `emViewPort.rs:53` (per spec §5 D5.6) — delete `focused: bool` field + its DIVERGED block.
- `emSubViewPanel.rs:117–121` — "sub_view_rc for SP5 Task 2.2" accessor and comment — delete.

---

## Task 1: Introduce `PanelScope`

**Files:**
- Create: `crates/emcore/src/emPanelScope.rs`
- Create: `crates/emcore/src/emPanelScope.rust_only`
- Modify: `crates/emcore/src/lib.rs` — add module.

- [ ] **Step 1: Write the failing test.** In `emPanelScope.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_variants_exist() {
        let _ = PanelScope::Toplevel(winit::window::WindowId(0));
        let _ = PanelScope::SubView(crate::emPanelTree::PanelId(0));
    }
}
```
- [ ] **Step 2: Run — expect FAIL** (module missing).
- [ ] **Step 3: Implement.**
```rust
//! PanelScope — identifies where a panel-associated engine resolves its view.
//!
//! See `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.2.

use winit::window::WindowId;
use crate::emPanelTree::PanelId;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PanelScope {
    Toplevel(WindowId),
    SubView(PanelId),
}

impl PanelScope {
    /// Resolve to a `&mut emView` through `EngineCtx`. Returns `None` if
    /// the target window or sub-view panel no longer exists (e.g. the
    /// owning panel was removed between engine registration and Cycle).
    pub fn resolve_view<'c, R>(
        self,
        ctx: &mut crate::emEngineCtx::EngineCtx<'_>,
        f: impl FnOnce(&mut crate::emView::emView, &mut crate::emEngineCtx::SchedCtx<'_>) -> R,
    ) -> Option<R> {
        match self {
            PanelScope::Toplevel(wid) => ctx.with_view_mut(wid, f),
            PanelScope::SubView(_pid) => {
                // Sub-view resolution threads through the owning panel's sub_view.
                // Phase 2 Task 5 wires this; stubbed here so callers compile.
                None
            }
        }
    }
}
```
Marker file: `Rust-only: C++ engines hold a raw emView* directly; Rust translates to an ID-scoped identifier resolved through EngineCtx.`

- [ ] **Step 4: Run — expect PASS.**
- [ ] **Step 5: Commit.**
```bash
git add crates/emcore/src/emPanelScope.rs crates/emcore/src/emPanelScope.rust_only crates/emcore/src/lib.rs
git commit -m "phase-2: introduce PanelScope"
```

---

## Task 2: Migrate `emWindow::view` from `Rc<RefCell<emView>>` to `emView`

**Files:**
- Modify: `crates/emcore/src/emWindow.rs:117, 1265`.

- [ ] **Step 1: Write failing test.** In `emWindow.rs`:
```rust
#[cfg(test)]
#[test]
fn window_view_is_plain() {
    let win = emWindow::new_for_test();
    let _: &emView = &win.view;
}
```
- [ ] **Step 2: Run — FAIL** (currently `Rc<RefCell<>>`).
- [ ] **Step 3: Rewrite.**
```rust
pub struct emWindow {
    pub view: emView,                      // was: Rc<RefCell<emView>>
    // rest unchanged
}
// delete fn view_rc(&self) -> &Rc<RefCell<emView>>
```
Constructors: build `emView` by value; store directly.

- [ ] **Step 4: Fix in-file callers.** `rg -n 'self\.view\.borrow\(\)|self\.view\.borrow_mut\(\)' crates/emcore/src/emWindow.rs`. Replace with direct field access.

- [ ] **Step 5: Compile smoke.** `cargo check -p emcore 2>&1 | tail -30`. Expected: breakages in external callers (Tasks 3–7 fix them).

- [ ] **Step 6: Commit only after Task 7 lands.** Stage only.

---

## Task 3: Migrate `emSubViewPanel::sub_view`

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs:25, 117, 121`.

- [ ] **Step 1:** Failing test analogous to Task 2 on `sub_view` being plain.
- [ ] **Step 2:** Rewrite:
```rust
pub struct emSubViewPanel {
    pub sub_view: emView,                  // was: Rc<RefCell<emView>>
    pub sub_tree: PanelTree,
    pub sub_root: PanelId,
    // rest unchanged
}
// delete fn sub_view_rc(&self) -> &Rc<RefCell<emView>>
```
- [ ] **Step 3:** Fix in-file callers.
- [ ] **Step 4:** Compile smoke — same caveat as Task 2.
- [ ] **Step 5:** Stage; no commit until Task 7.

---

## Task 4: Migrate `emViewPort` back-reference and remove `focused` duplicate

**Files:**
- Modify: `crates/emcore/src/emViewPort.rs:5, 43, 53, 244`.

- [ ] **Step 1:** Rewrite the `window` field:
```rust
pub struct emViewPort {
    pub window_id: Option<WindowId>,       // was: Option<Weak<RefCell<emWindow>>>
    // home geometry fields unchanged (see D5.4 preserved rationale)
    // delete: focused: bool  (per D5.6)
    // rest unchanged
}
```
- [ ] **Step 2:** Delete the three DIVERGED blocks listed in File Structure.
- [ ] **Step 3:** Fix callers that did `self.window.upgrade()?.borrow()`. Each becomes a ctx-threaded resolution: `ctx.windows.get(&wid)` — but since a port lives *inside* a window's `emView`, the typical caller already has `&mut emWindow` on the stack; the back-ref is used only for cross-window operations (e.g. SwapViewPorts — Task 8).

- [ ] **Step 4:** For focus consolidation: every `port.focused = true/false` site becomes a mutation of the owning `emView::focused`. `rg -n '\.focused' crates/emcore/src/emViewPort.rs crates/emcore/src/emView.rs`. Update each site.

- [ ] **Step 5:** Compile smoke; stage.

---

## Task 5: Migrate `emPanelCycleEngine` from `Weak<RefCell<emView>>` to `PanelScope`

**Files:**
- Modify: `crates/emcore/src/emPanelCycleEngine.rs:34`.

- [ ] **Step 1:**
```rust
pub struct emPanelCycleEngine {
    pub(crate) scope: PanelScope,           // was: view: Weak<RefCell<emView>>
    pub(crate) panel: PanelId,
    // rest unchanged
}
```

- [ ] **Step 2:** `Cycle` becomes:
```rust
impl emEngine for emPanelCycleEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) {
        self.scope.resolve_view(ctx, |view, sched| {
            view.cycle_panel(self.panel, sched);
        });
    }
}
```

- [ ] **Step 3:** `PanelScope::SubView` resolution — extend Task 1's stub:
```rust
PanelScope::SubView(pid) => {
    // Find the owning sub-view panel in one of the top-level windows.
    // Concrete lookup: iterate ctx.windows, probe each window.view's
    // PanelTree for `pid`. If found in a top-level tree, the sub-view's
    // emSubViewPanel::sub_view is the target emView.
    for (_wid, win) in ctx.windows.iter_mut() {
        if let Some(behavior) = win.view.panel_tree_mut()
            .panels.get_mut(&pid)
            .and_then(|p| p.behavior.as_mut())
        {
            if let Some(svp) = behavior.as_any_mut().downcast_mut::<emSubViewPanel>() {
                // Found. Build a SchedCtx and invoke f with &mut svp.sub_view.
                // (Implementation threads the same scheduler+framework_actions as
                // EngineCtx::with_view_mut.)
                // ... concrete borrow gymnastics per spec §3.3.
                let mut sched = SchedCtx {
                    scheduler: &mut *ctx.scheduler,
                    framework_actions: &mut *ctx.framework_actions,
                    root_context: ctx.root_context,
                    current_engine: ctx.current_engine,
                };
                return Some(f(&mut svp.sub_view, &mut sched));
            }
        }
    }
    None
}
```
Note: this is the Q1 resolution per spec §13. Pick the `with_sub_tree` closure variant; the above sketch applies that pattern. Test coverage is Task 9's regression test for sub-view cycling.

- [ ] **Step 4:** Compile smoke. Stage.

---

## Task 6: Relocate NoticeList back to `emView`

**Files:**
- Modify: `crates/emcore/src/emView.rs`, `crates/emcore/src/emPanelTree.rs`.

- [ ] **Step 1: Locate current NoticeList field** on `PanelTree` (per SP5 relocation).

- [ ] **Step 2: Move the field + methods + accessors to `emView`.** Rename any references from `tree.notice_list` to `view.notice_list`.

- [ ] **Step 3: Delete the DIVERGED block at `emView.rs:3465`** (SP5 relocation rationale).

- [ ] **Step 4: Update the per-view notice dispatch** so the ring-walker reads from `view.notice_list` directly.

- [ ] **Step 5: Run notice-dispatch tests.** `cargo test -p emcore notice 2>&1 | tail`. Expected: PASS.

- [ ] **Step 6:** Stage.

---

## Task 7: Migrate framework `windows` field and fix all emView/emWindow callers

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs:92, 208, 419`.
- Modify: `crates/emcore/src/emFileModel.rs:900`.
- Modify: call sites tree-wide.

- [ ] **Step 1:** `emGUIFramework::windows` already plain `HashMap<WindowId, emWindow>` post-Phase-1. Confirm via `rg -n 'windows.*HashMap' crates/emcore/src/emGUIFramework.rs`.

- [ ] **Step 2:** `emGUIFramework.rs:208` — `win_rc: Rc<RefCell<emWindow>>` parameter → `win: emWindow` (or `wid: WindowId` + framework lookup). Rewrite the function.

- [ ] **Step 3:** `emGUIFramework.rs:419` comment rewrite — describe pending-popups-map model (Task 8 sibling).

- [ ] **Step 4:** `emFileModel.rs:900` — `HashMap<WindowId, Rc<RefCell<emWindow>>>` → `HashMap<WindowId, emWindow>` inside the client struct. Migrate accesses.

- [ ] **Step 5: Full compile.**
```bash
cargo check -p emcore 2>&1 | tail -40
```
At this point Tasks 2–7 combined should compile. If errors remain, fix them — they are the "downstream breakages" from earlier tasks.

- [ ] **Step 6: Run nextest.**
```bash
cargo-nextest ntr 2>&1 | tail -10
```
Expect: green.

- [ ] **Step 7: Commit Tasks 2–7 as one atomic change.**
```bash
git add -A
git commit -m "phase-2: emView/emWindow plain composition; NoticeList back to emView; PanelCycleEngine scope-based"
```

---

## Task 8: Popup cancellation via `pending_popups` map

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs` — add `pending_popups` field; rewrite `emGUIFramework.rs:206–229` popup-cancellation logic.
- Modify: `crates/eaglemode/tests/unit/popup_materialization.rs`, `popup_cancel_before_materialize.rs` — migrate fixtures to ID + pending-map presence.

- [ ] **Step 1:** Add field:
```rust
pub pending_popups: HashMap<WindowId, emWindow>,   // popups not yet winit-materialized (§3.7)
```

- [ ] **Step 2:** Rewrite popup creation flow:
```rust
// In RawVisitAbs or equivalent popup trigger:
let popup_wid = framework.alloc_window_id();
let popup = emWindow::new_popup(/* args */);
framework.pending_popups.insert(popup_wid, popup);
view.PopupWindow = Some(popup_wid);  // ID, not Rc
framework.framework_action(DeferredAction::MaterializePopup(popup_wid));
```

- [ ] **Step 3:** Rewrite teardown: when `view.PopupWindow = None` is set (popup cancelled), also call `framework.pending_popups.remove(&popup_wid)`.

- [ ] **Step 4:** Rewrite `MaterializePopup` drain:
```rust
DeferredAction::MaterializePopup(wid) => {
    if let Some(popup_win) = framework.pending_popups.remove(&wid) {
        let materialized = popup_win.materialize_surface(&event_loop);
        framework.windows.insert(wid, materialized);
    }
    // absent: popup cancelled before materialization — no-op, observable match
}
```

- [ ] **Step 5:** Rewrite the 419-line comment to document this flow.

- [ ] **Step 6:** Migrate test fixtures:
```rust
// popup_materialization.rs:43
popup_window_id: Option<WindowId>,   // was: popup_window: Option<Rc<RefCell<emWindow>>>

// popup_cancel_before_materialize.rs:53
popup_window_id: Option<WindowId>,   // was: popup_weak: Option<Weak<RefCell<emWindow>>>
// Test assertion changes from weak.upgrade().is_none() to !framework.pending_popups.contains_key(&wid).
```

- [ ] **Step 7: Run both popup tests.**
```bash
cargo test -p eaglemode popup_materialization popup_cancel_before_materialize 2>&1 | tail -10
```
Expected: PASS.

- [ ] **Step 8: Commit.**
```bash
git add crates/emcore/src/emGUIFramework.rs crates/eaglemode/tests/unit/popup_materialization.rs crates/eaglemode/tests/unit/popup_cancel_before_materialize.rs
git commit -m "phase-2: popup cancellation via pending_popups map"
```

---

## Task 9: SwapViewPorts via `HashMap::get_disjoint_mut`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (SwapViewPorts method).

- [ ] **Step 1: Locate.** `rg -n 'fn SwapViewPorts' crates/emcore/src/emView.rs`.

- [ ] **Step 2: Rewrite.** SwapViewPorts must now be called from a scope that holds `&mut framework.windows` (framework-level code), because it's a cross-window operation. The method migrates from `&mut self` to a free function taking the windows map:

```rust
pub fn SwapViewPorts(
    windows: &mut HashMap<WindowId, emWindow>,
    this_wid: WindowId,
    popup_wid: WindowId,
    ctx: &mut SchedCtx<'_>,
) {
    let [this_win, popup_win] = windows.get_disjoint_mut([&this_wid, &popup_wid])
        .expect("disjoint window IDs");
    std::mem::swap(
        &mut this_win.view.CurrentViewPort,
        &mut popup_win.view.CurrentViewPort,
    );
    // surrounding logic (geometry updates, signal fires) runs after swap
    // with both window borrows still live.
    this_win.view.fire_swap_signal(ctx);
    popup_win.view.fire_swap_signal(ctx);
}
```

- [ ] **Step 3: Update callers** of the old `view.SwapViewPorts(...)`: they now call `emView::SwapViewPorts(windows, this_wid, popup_wid, ctx)` from framework scope.

- [ ] **Step 4: Run the popup-zoom swap-viewport regression test.** Identify via `rg -n 'swap_view_ports|SwapViewPorts' crates/eaglemode/tests/`.
```bash
cargo test -p eaglemode swap 2>&1 | tail
```
Expected: PASS.

- [ ] **Step 5: Commit.**
```bash
git add crates/emcore/src/emView.rs
git commit -m "phase-2: SwapViewPorts via HashMap::get_disjoint_mut"
```

---

## Task 10: Delete remaining DIVERGED blocks listed in File Structure

**Files:**
- Modify: `crates/emcore/src/emView.rs:238`, `emViewPort.rs:5, 43, 244`, `emSubViewPanel.rs:117-121`.

- [ ] **Step 1: Delete each block.** Keep any prose that now explains WindowId-based resolution; purge the obsolete "Rust has no X" framing.

- [ ] **Step 2: Full gate.**
```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo-nextest ntr && cargo test --test golden -- --test-threads=1
```
Expected: all green.

- [ ] **Step 3: Commit.**
```bash
git add -A
git commit -m "phase-2: delete obsoleted DIVERGED blocks for emView/emViewPort/emSubViewPanel"
```

---

## Task 11: Run invariant assertions I2, I2a, I2b, I2c, I6

- [ ] **Step 1: Run checks.**
```bash
rg 'Rc<RefCell<emView>>' crates/ --glob '!*/tests/*' && echo "I2 FAIL" || echo "I2 PASS"
rg 'Weak<RefCell<emView>>' crates/ && echo "I2a FAIL" || echo "I2a PASS"
rg 'Rc<RefCell<emWindow>>|Weak<RefCell<emWindow>>' crates/ && echo "I2b FAIL" || echo "I2b PASS"
rg 'view_rc|sub_view_rc' crates/ && echo "I2c FAIL" || echo "I2c PASS"
```
All PASS required. If I2 has test-only holdouts, enumerate in the Closeout and defer Phase-5 removal.

- [ ] **Step 2: Golden check.** `cargo test --test golden -- --test-threads=1 2>&1 | tail -5` — 237/6 baseline or better.

- [ ] **Step 3: Proceed to Closeout.**

---

## Closeout (per shared ritual)

Run C1–C11 with `<N>` = `2`. At C4 use I2/I2a/I2b/I2c/I6. At C5 close E006 (Task 6), E014 (Task 4+5), E015 (Task 4 + focus consolidation in Task 4 Step 4), E038 (borrow-ordering hazard dissolves — Task 7 Step 5 proves via green compile).
