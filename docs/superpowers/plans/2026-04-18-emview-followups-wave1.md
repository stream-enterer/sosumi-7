# emView Followups — Wave 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close four bounded C++-mirror gaps in `emView`/`emViewPort`/`emViewAnimator` surfaced by the Phase 1–10 execution-debt report: animator input-forwarding, `InvalidateHighlight` call-site parity, `PaintView`/`InvalidatePainting` re-entrancy doc, and a GeometrySignal double-fire note.

**Architecture:** Pure residual C++ port, zero new architecture. `ActiveAnimator` lives on `emWindow` / `emSubViewPanel` in Rust (not on `emView`, by the Phase 5/6 decision), so animator-forward happens in the caller sites that own the animator slot. `InvalidateHighlight` calls are inserted into the existing Rust `set_active_panel` / `SetFocused` at the same logical points as C++.

**Tech Stack:** Rust (crates/emcore), `cargo-nextest ntr` for testing.

**Source roadmap:** `docs/superpowers/notes/2026-04-18-emview-followups-roadmap.md` §W1.
**Source report:** `docs/superpowers/notes/2026-04-18-emview-followups-execution-debt.md` §§3.1, 3.2, 3.3, 4.8.

**C++ reference anchors:**
- `emView.cpp:1004` — `if (ActiveAnimator) ActiveAnimator->Input(event,state);`
- `emView.cpp:284, 305, 312` — `InvalidateHighlight` from `SetActivePanel`.
- `emView.cpp:1211, 1213` — `InvalidateHighlight` from `SetFocused`.
- `emView.cpp:1678, 1995` — the two `Signal(GeometrySignal)` fires on popup teardown.
- `emViewAnimator.cpp:111` — base `Input` forwards to slave.
- `emViewAnimator.cpp:1066` — `emVisitingViewAnimator::Input` eats event + deactivates when in `ST_SEEK | ST_GIVING_UP`.

**Rules of engagement:**
- No new `DIVERGED:` / `RUST-DIVERGED:` annotations; any that appear must be pre-existing.
- No `#[allow(...)]` / `#[expect(...)]`. Fix the cause.
- Pre-commit hook runs fmt + `clippy -D warnings` + nextest. Do not `--no-verify`.
- Every commit must leave `cargo-nextest ntr` at 2409/2409 pass + golden at baseline parity (237/243).

---

## File Structure

| File | Role in this plan |
|---|---|
| `crates/emcore/src/emView.rs` | W1b: add `self.InvalidateHighlight(tree)` calls in `set_active_panel` and `SetFocused`. W1d: add one-line comment at the explicit GeometrySignal fire site in the popup-teardown branch. |
| `crates/emcore/src/emViewPort.rs` | W1c: add doc-comment warnings on `PaintView` and `InvalidatePainting`. |
| `crates/emcore/src/emViewAnimator.rs` | W1a: add `fn Input(...)` to the `emViewAnimator` trait (default no-op); override in `emVisitingViewAnimator`. |
| `crates/emcore/src/emWindow.rs` | W1a: in `dispatch_input`, forward the event to `self.active_animator` before invoking `vp.InputToView`. |
| `crates/emcore/src/emSubViewPanel.rs` | W1a: in the `Behavior::Input` impl, forward the event to `self.active_animator` before panel broadcast. |
| `crates/emcore/src/emView.rs` (tests) | W1a unit test: visiting-animator consumes input. W1b unit tests: `set_active_panel` / `SetFocused` push dirty rect. |

Tasks are ordered smallest-to-largest to keep early commits trivial.

---

## Task 1: W1d — GeometrySignal double-fire comment

**Files:**
- Modify: `crates/emcore/src/emView.rs` — the popup-teardown branch's explicit `Signal(GeometrySignal)` fire (inside `SwapViewPorts`/`Update` popup-close path; single-line comment above the explicit fire). Current comment reads `// C++ (emView.cpp:1678): Signal(GeometrySignal).`

- [ ] **Step 1: Locate the site**

Run:
```
grep -n "C++ (emView.cpp:1678): Signal(GeometrySignal)" crates/emcore/src/emView.rs
```
Expected: exactly one hit, around line 1764. The next lines fire `self.geometry_signal` via the scheduler. The line immediately above begins with `self.PopupWindow = None;` and the `SwapViewPorts(true)` call a few lines earlier already fires `GeometrySignal` internally (see `SwapViewPorts` epilogue). This is the explicit second fire.

- [ ] **Step 2: Replace the comment with a double-fire note**

Replace this block:

```rust
                self.PopupWindow = None;
                // C++ (emView.cpp:1678): Signal(GeometrySignal).
                if let (Some(sig), Some(sched)) = (self.geometry_signal, &self.scheduler) {
                    sched.borrow_mut().fire(sig);
                }
```

with:

```rust
                self.PopupWindow = None;
                // C++ emView.cpp:1678 + 1995 — GeometrySignal fires twice on
                // popup teardown: once inside SwapViewPorts(true) above
                // (matching emView.cpp:1995), and once explicitly here
                // (matching emView.cpp:1678). Keep both; do not dedup.
                if let (Some(sig), Some(sched)) = (self.geometry_signal, &self.scheduler) {
                    sched.borrow_mut().fire(sig);
                }
```

- [ ] **Step 3: Verify build and tests**

Run:
```
cargo-nextest ntr
```
Expected: 2409/2409 pass.

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "docs(emView): note GeometrySignal double-fire on popup teardown"
```

---

## Task 2: W1c — PaintView / InvalidatePainting re-entrancy doc

**Files:**
- Modify: `crates/emcore/src/emViewPort.rs` — method `PaintView` (~line 163) and method `InvalidatePainting` (~line 257).

- [ ] **Step 1: Add re-entrancy warning to `PaintView`**

Replace this block:

```rust
    /// Port of C++ `emViewPort::PaintView`.
    ///
    /// Requests a redraw on the owning `emWindow`. No-op for dummy ports.
    pub fn PaintView(&self) {
```

with:

```rust
    /// Port of C++ `emViewPort::PaintView`.
    ///
    /// Requests a redraw on the owning `emWindow`. No-op for dummy ports.
    ///
    /// **Re-entrancy warning:** the back-reference is upgraded and the owning
    /// `emWindow` is borrowed shared. Callers must NOT already hold a
    /// `rc.borrow_mut()` on the same window (e.g. from inside `render`,
    /// `dispatch_input`, or `handle_touch`) — the runtime `RefCell` check
    /// would panic rather than being caught at compile time. A full audit is
    /// required when production call sites are first wired.
    pub fn PaintView(&self) {
```

- [ ] **Step 2: Add re-entrancy warning to `InvalidatePainting`**

Replace this block:

```rust
    /// Port of C++ `emViewPort::InvalidatePainting(x, y, w, h)`.
    ///
    /// Delegates to the owning `emWindow`'s tile cache. No-op for dummy
    /// ports.
    pub fn InvalidatePainting(&mut self, x: f64, y: f64, w: f64, h: f64) {
```

with:

```rust
    /// Port of C++ `emViewPort::InvalidatePainting(x, y, w, h)`.
    ///
    /// Delegates to the owning `emWindow`'s tile cache. No-op for dummy
    /// ports.
    ///
    /// **Re-entrancy warning:** the back-reference is upgraded and the owning
    /// `emWindow` is borrowed mutably (`rc.borrow_mut()`). Callers must NOT
    /// already hold any borrow on the same window (e.g. from inside `render`,
    /// `dispatch_input`, or `handle_touch`) — the runtime `RefCell` check
    /// would panic rather than being caught at compile time. A full audit is
    /// required when production call sites are first wired.
    pub fn InvalidatePainting(&mut self, x: f64, y: f64, w: f64, h: f64) {
```

- [ ] **Step 3: Verify build and tests**

Run:
```
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean, 2409/2409 pass.

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emViewPort.rs
git commit -m "docs(emViewPort): warn callers about re-entrant borrows on PaintView/InvalidatePainting"
```

---

## Task 3: W1b — Mirror InvalidateHighlight call sites

**C++ call sites (exhaustive):**
- `emView.cpp:284` — inside `SetActivePanel`, if `ActivePanel != panel` AND previous `ActivePanel` existed.
- `emView.cpp:305` — inside `SetActivePanel`, after installing the new active panel (unconditional on the transition branch).
- `emView.cpp:312` — inside `SetActivePanel`, on the else-if branch where only `ActivationAdherent` changed.
- `emView.cpp:1211` — inside `SetFocused`, before changing `Focused` (only when already `Focused`).
- `emView.cpp:1213` — inside `SetFocused`, after changing `Focused` (only when now `Focused`).

**Rust target methods:**
- `emView::set_active_panel(&mut self, tree: &mut PanelTree, panel: PanelId, adherent: bool)` (emView.rs:1374).
- `emView::SetFocused(&mut self, tree: &mut PanelTree, focused: bool)` (emView.rs:594).

`InvalidateHighlight(tree: &PanelTree)` at emView.rs:3169 takes `&PanelTree` (immutable), which is compatible with the `&mut PanelTree` these methods hold.

**Files:**
- Modify: `crates/emcore/src/emView.rs` at both sites and tests at end of file.
- Test: `crates/emcore/src/emView.rs` (inline module). Pattern: reuse the existing `test_phase7_invalidate_highlight_dirties_view` (emView.rs:5731-5741) style — build a view with an active viewed panel, assert `dirty_rects` grows after the transition.

### Task 3.1: Write the failing tests

- [ ] **Step 1: Locate the test module**

Run:
```
grep -n "fn test_phase7_invalidate_highlight_dirties_view" crates/emcore/src/emView.rs
```
Expected: one hit around line 5731. The surrounding `#[cfg(test)] mod tests { ... }` block owns it — add the new tests there.

- [ ] **Step 2: Add three failing tests**

Insert the following three tests directly after `test_phase7_invalidate_highlight_dirties_view`. Do not copy the existing test — the setup below is self-contained so the tests can be read in isolation.

```rust
    /// W1b: set_active_panel on a transition must push a dirty rect,
    /// matching C++ emView.cpp:284 (old active) and emView.cpp:305 (new
    /// active). Checks the transition case where both branches contribute.
    #[test]
    fn set_active_panel_transition_invalidates_highlight() {
        let mut v = emView::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", 100.0, 100.0);
        let a = tree.create_child(root, "a").expect("child a");
        let b = tree.create_child(root, "b").expect("child b");
        tree.get_mut(a).unwrap().focusable = true;
        tree.get_mut(b).unwrap().focusable = true;
        // Make both panels viewed so InvalidateHighlight isn't a no-op.
        tree.get_mut(a).unwrap().viewed_width = 10.0;
        tree.get_mut(b).unwrap().viewed_width = 10.0;
        v.supreme_viewed_panel = Some(root);
        v.set_active_panel(&mut tree, a, false);
        v.dirty_rects.clear();
        v.set_active_panel(&mut tree, b, false);
        assert!(
            !v.dirty_rects.is_empty(),
            "set_active_panel transition should InvalidateHighlight"
        );
    }

    /// W1b: set_active_panel with only ActivationAdherent changing must
    /// still push a dirty rect, matching C++ emView.cpp:312.
    #[test]
    fn set_active_panel_adherent_only_invalidates_highlight() {
        let mut v = emView::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", 100.0, 100.0);
        let a = tree.create_child(root, "a").expect("child a");
        tree.get_mut(a).unwrap().focusable = true;
        tree.get_mut(a).unwrap().viewed_width = 10.0;
        v.supreme_viewed_panel = Some(root);
        v.set_active_panel(&mut tree, a, false);
        v.dirty_rects.clear();
        v.set_active_panel(&mut tree, a, true);
        assert!(
            !v.dirty_rects.is_empty(),
            "set_active_panel adherent-only change should InvalidateHighlight"
        );
    }

    /// W1b: SetFocused must InvalidateHighlight twice — once if already
    /// focused (C++ emView.cpp:1211) and once if now focused (C++
    /// emView.cpp:1213). Observed as at least one dirty rect post-call.
    #[test]
    fn set_focused_invalidates_highlight() {
        let mut v = emView::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", 100.0, 100.0);
        let a = tree.create_child(root, "a").expect("child a");
        tree.get_mut(a).unwrap().focusable = true;
        tree.get_mut(a).unwrap().viewed_width = 10.0;
        v.supreme_viewed_panel = Some(root);
        v.set_active_panel(&mut tree, a, false);
        v.SetFocused(&mut tree, true);
        v.dirty_rects.clear();
        v.SetFocused(&mut tree, false);
        assert!(
            !v.dirty_rects.is_empty(),
            "SetFocused(false) while focused should InvalidateHighlight"
        );
        v.dirty_rects.clear();
        v.SetFocused(&mut tree, true);
        assert!(
            !v.dirty_rects.is_empty(),
            "SetFocused(true) should InvalidateHighlight"
        );
    }
```

> **Note on field access:** the tests reach into `v.supreme_viewed_panel`, `v.dirty_rects`, and `tree.get_mut(...).viewed_width` / `.focusable`. These are all `pub(crate)` and already used by the existing `test_phase7_invalidate_highlight_dirties_view`. If any of these fields have been renamed since that test was written, grep the test file for the up-to-date names and use those — do not widen visibility.

- [ ] **Step 3: Run the tests to confirm they fail**

Run:
```
cargo-nextest ntr -E 'test(set_active_panel_transition_invalidates_highlight) | test(set_active_panel_adherent_only_invalidates_highlight) | test(set_focused_invalidates_highlight)'
```
Expected: all three FAIL with `dirty_rects` empty after the mutating call.

### Task 3.2: Add the InvalidateHighlight calls

- [ ] **Step 4: Patch `set_active_panel`**

Locate `set_active_panel` at emView.rs:1374. The current structure is:

```rust
        if self.active == Some(target) {
            if self.activation_adherent != adherent {
                self.activation_adherent = adherent;
            }
            return;
        }

        // Build notice flags: always ACTIVE_CHANGED, add FOCUS_CHANGED if focused
        let mut flags = super::emPanel::NoticeFlags::ACTIVE_CHANGED;
        // ...
        // Clear old active path
        if let Some(old_active) = self.active {
            // ... sets pending_notices on old path
        }

        // Set new active path
        self.active = Some(target);
        // ... sets in_active_path on new path
        self.activation_adherent = adherent;
        self.control_panel_invalid = true;
```

Apply three edits in sequence, mirroring C++ emView.cpp:284/305/312:

1. At the top of the "transition" branch (after `if self.active == Some(target) { ... return; }`, but before "Build notice flags"), add an `InvalidateHighlight` call guarded on `self.active.is_some()` (C++ emView.cpp:284 — only if previous `ActivePanel` existed):

```rust
        // C++ emView.cpp:284: InvalidateHighlight for the outgoing active panel.
        if self.active.is_some() {
            self.InvalidateHighlight(tree);
        }
```

Note: `InvalidateHighlight` takes `&PanelTree`; passing `tree` (which is `&mut PanelTree`) reborrows as shared. This compiles fine because we do not hold any active borrow on `tree` at this point.

2. After `self.activation_adherent = adherent;` (and before `self.control_panel_invalid = true;`), add (C++ emView.cpp:305):

```rust
        // C++ emView.cpp:305: InvalidateHighlight for the new active panel.
        self.InvalidateHighlight(tree);
```

3. Change the early-return adherent-only branch to also invalidate (C++ emView.cpp:312). Replace:

```rust
        if self.active == Some(target) {
            if self.activation_adherent != adherent {
                self.activation_adherent = adherent;
            }
            return;
        }
```

with:

```rust
        if self.active == Some(target) {
            if self.activation_adherent != adherent {
                self.activation_adherent = adherent;
                // C++ emView.cpp:312: InvalidateHighlight on adherent-only change.
                self.InvalidateHighlight(tree);
            }
            return;
        }
```

- [ ] **Step 5: Patch `SetFocused`**

Locate `SetFocused` at emView.rs:594. Replace:

```rust
    pub fn SetFocused(&mut self, tree: &mut PanelTree, focused: bool) {
        if self.window_focused == focused {
            return;
        }
        self.window_focused = focused;
```

with:

```rust
    pub fn SetFocused(&mut self, tree: &mut PanelTree, focused: bool) {
        if self.window_focused == focused {
            return;
        }
        // C++ emView.cpp:1211: InvalidateHighlight before clearing focus.
        if self.window_focused {
            self.InvalidateHighlight(tree);
        }
        self.window_focused = focused;
        // C++ emView.cpp:1213: InvalidateHighlight after acquiring focus.
        if self.window_focused {
            self.InvalidateHighlight(tree);
        }
```

- [ ] **Step 6: Run the new tests**

Run:
```
cargo-nextest ntr -E 'test(set_active_panel_transition_invalidates_highlight) | test(set_active_panel_adherent_only_invalidates_highlight) | test(set_focused_invalidates_highlight)'
```
Expected: all three PASS.

- [ ] **Step 7: Full suite + golden baseline parity**

Run:
```
cargo clippy -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```
Expected: clippy clean, nextest 2409 + 3 new = 2412/2412 pass, golden 237/243 (baseline parity; the 6 pre-existing failures named in the execution-debt report §8.4 are not this task's concern, but their set must not grow).

> **If golden regresses:** the most likely cause is a previously-unexercised highlight path now dirtying the view during test setup. Verify by re-running with `DUMP_GOLDEN=1 cargo test --test golden <failing_name> -- --test-threads=1` and diffing. Do NOT quiet the new `InvalidateHighlight` calls — the C++ parity is the point. Investigate the golden divergence instead (see `scripts/diff_draw_ops.py`).

- [ ] **Step 8: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "feat(emView): mirror C++ InvalidateHighlight call sites in set_active_panel and SetFocused"
```

---

## Task 4: W1a — Port animator Input forwarding

**C++ contract:**
- `emView::Input` forwards to `ActiveAnimator->Input(event, state)` first (emView.cpp:1004).
- `emViewAnimator::Input(event, state)` (base, emViewAnimator.cpp:111) forwards to `ActiveSlave->Input(event, state)` if present; otherwise no-op.
- `emVisitingViewAnimator::Input(event, state)` (emViewAnimator.cpp:1066):
  - Returns early if inactive or if state is not `ST_SEEK | ST_GIVING_UP`.
  - Otherwise, if `!event.IsEmpty()`: calls `event.Eat()` and `Deactivate()`.
- Other `emViewAnimator` subclasses (`emKineticViewAnimator`, `emSpeedingViewAnimator`, `emSwipingViewAnimator`, `emMagneticViewAnimator`) do NOT override `Input`; they inherit the base slave-forward. Since the Rust `emViewAnimator` trait has no slave concept, the base default is a true no-op — acceptable.

**Rust architectural constraint (do not change):**
- `active_animator` lives on `emWindow` (emWindow.rs:59) and on `emSubViewPanel` (emSubViewPanel.rs:30), not on `emView`. This was the Phase 5/6 design decision and is out of scope for this wave.
- Forwarding therefore happens in the two callers that own an animator slot: `emWindow::dispatch_input` and `emSubViewPanel::Behavior::Input`. Inside `emView::Input`, we replace the `PHASE-6-FOLLOWUP: forward to active animator first ...` comment with a note that the forward happens in the caller.

### Task 4.1: Extend the trait and override on emVisitingViewAnimator

**Files:**
- Modify: `crates/emcore/src/emViewAnimator.rs`.

- [ ] **Step 1: Write the failing unit test**

Add this test to `crates/emcore/src/emViewAnimator.rs` in the existing `#[cfg(test)] mod tests { ... }` block (find it with `grep -n "^mod tests\|^#\[cfg(test)\]" crates/emcore/src/emViewAnimator.rs`).

```rust
    /// W1a: emVisitingViewAnimator::Input must eat non-empty events and
    /// deactivate when in Seek or GivingUp states (C++ emViewAnimator.cpp:1066).
    #[test]
    fn visiting_animator_input_eats_event_and_deactivates_in_seek() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emInputState::emInputState;

        let mut anim = emVisitingViewAnimator::new();
        anim.active = true;
        anim.state = VisitingState::Seek;
        let mut ev = emInputEvent::press(InputKey::Enter);
        let st = emInputState::default();
        emViewAnimator::Input(&mut anim, &mut ev, &st);
        assert!(ev.IsEmpty(), "event should be eaten");
        assert!(!anim.active, "animator should deactivate");
    }

    /// W1a: emVisitingViewAnimator::Input is a no-op in inactive state,
    /// mirroring C++ early return.
    #[test]
    fn visiting_animator_input_noop_when_inactive() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emInputState::emInputState;

        let mut anim = emVisitingViewAnimator::new();
        anim.active = false;
        anim.state = VisitingState::Seek;
        let mut ev = emInputEvent::press(InputKey::Enter);
        let st = emInputState::default();
        emViewAnimator::Input(&mut anim, &mut ev, &st);
        assert!(!ev.IsEmpty(), "event must not be eaten when animator inactive");
    }
```

> **If any symbol above does not exist under the given name** (`emVisitingViewAnimator::new`, `VisitingState::Seek`, `emInputState::default`, `emInputEvent::press`, `InputKey::Enter`, `emInputEvent::IsEmpty`, the `active`/`state` field names on `emVisitingViewAnimator`): grep the current source for the actual name and adjust the test. Do not widen visibility — all of these are already used inside the existing animator test suite in this same file and the emInput tests.

- [ ] **Step 2: Run the tests to confirm they fail**

Run:
```
cargo-nextest ntr -E 'test(visiting_animator_input)'
```
Expected: compile error — `emViewAnimator::Input` does not exist on the trait.

- [ ] **Step 3: Add the trait method with a default no-op body**

In `crates/emcore/src/emViewAnimator.rs`, locate the `pub trait emViewAnimator { ... }` definition (line 9). Add a new method at the end of the trait, before the closing brace:

```rust
    /// Port of C++ `emViewAnimator::Input(emInputEvent&, const emInputState&)`
    /// (emViewAnimator.cpp:111). Base default forwards to the active slave in
    /// C++; the Rust trait has no slave concept, so the default is a no-op.
    /// Subclasses that want to consume input (e.g. `emVisitingViewAnimator`)
    /// override this.
    fn Input(
        &mut self,
        _event: &mut crate::emInput::emInputEvent,
        _state: &crate::emInputState::emInputState,
    ) {
    }
```

- [ ] **Step 4: Override on emVisitingViewAnimator**

Find the `impl emViewAnimator for emVisitingViewAnimator { ... }` block (line 2010). Inside the impl block, add:

```rust
    /// Port of C++ `emVisitingViewAnimator::Input` (emViewAnimator.cpp:1066).
    fn Input(
        &mut self,
        event: &mut crate::emInput::emInputEvent,
        _state: &crate::emInputState::emInputState,
    ) {
        // C++ emViewAnimator.cpp:1068: no-op unless active and in Seek/GivingUp.
        if !self.active
            || (self.state != VisitingState::Seek && self.state != VisitingState::GivingUp)
        {
            return;
        }
        // C++ emViewAnimator.cpp:1070-1073: eat event and deactivate.
        if !event.IsEmpty() {
            event.eat();
            self.active = false;
            self.state = VisitingState::NoGoal;
        }
    }
```

> **On `Deactivate()` fidelity:** C++ `emVisitingViewAnimator::Deactivate` resets additional fields beyond `active`/`state`. Inspect the existing `emVisitingViewAnimator` Rust code: if there is already a dedicated `deactivate()` method on the type, call it instead of setting `active = false` / `state = ...` inline. If not, the two assignments above are the minimum to match the observable behaviour (animator stops advancing and its state machine will early-return from subsequent `animate()`); defer a full `deactivate()` helper to a later wave — do not invent one here.

- [ ] **Step 5: Run the tests and the full suite**

Run:
```
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean, 2412 + 2 = 2414/2414 pass.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emViewAnimator.rs
git commit -m "feat(emViewAnimator): port base and VisitingViewAnimator Input overrides"
```

### Task 4.2: Forward from emWindow::dispatch_input

**Files:**
- Modify: `crates/emcore/src/emWindow.rs` — `dispatch_input` at line 639.

- [ ] **Step 1: Locate the forward point**

Run:
```
grep -n "vp.InputToView" crates/emcore/src/emWindow.rs
```
Expected: one hit at emWindow.rs:662 inside `dispatch_input`.

- [ ] **Step 2: Forward to `active_animator` before `InputToView`**

Locate the block (around line 658-663):

```rust
        // Phase 5 (emview-rewrite-followups): route through emViewPort.
        // ...
        {
            let vp = self.view.CurrentViewPort.clone();
            let mut vp = vp.borrow_mut();
            vp.input_clock_ms = crate::emScheduler::emGetClockMS();
            vp.InputToView(&mut self.view, tree, event, state);
        }
```

The incoming parameters are `event: &emInputEvent` and `state: &mut emInputState`. The trait's `Input` takes `&mut emInputEvent`, so we need a mutable copy to pass to the animator and the downstream dispatch. The existing `emWindow::dispatch_input` already clones the event for VIF iteration in the enclosing code — clone once up-front. Since mutating the event (to `eat()`) must propagate to downstream logic in this method, we rebind to a local `event` shadow.

Replace the block above with:

```rust
        // C++ emView.cpp:1004: forward input to ActiveAnimator first.
        // Rust-arch note: the animator lives on emWindow (not emView) by the
        // Phase 5/6 decision, so this forward happens in the caller. A
        // `visiting` animator may eat the event here, in which case the VIF
        // chain and panel broadcast below see an empty event.
        let mut event = event.clone();
        if let Some(mut anim) = self.active_animator.take() {
            emViewAnimator::Input(anim.as_mut(), &mut event, state);
            self.active_animator = Some(anim);
        }

        // Phase 5 (emview-rewrite-followups): route through emViewPort.
        // ...
        {
            let vp = self.view.CurrentViewPort.clone();
            let mut vp = vp.borrow_mut();
            vp.input_clock_ms = crate::emScheduler::emGetClockMS();
            vp.InputToView(&mut self.view, tree, &event, state);
        }
```

Keep the remainder of `dispatch_input` (VIF iteration, cursor warp, cheat-VIF, panel broadcast) using the local `event` shadow. If later code inside this method still references the original `event` parameter name, the shadow covers it automatically; verify no borrow of the original outlives the shadow point. The existing VIF-chain loop (`for vif in &mut self.vif_chain { if vif.filter(event, state, ...) }`) already operates on this `event` binding — no edit needed there.

> **Why `take()` + `Some(anim)` reinsert:** we cannot hold `&mut self.active_animator` while also calling methods that take `&mut self` later. The take/reinsert is a borrow-check workaround already used elsewhere in this codebase (see `emGUIFramework.rs:406` and `emSubViewPanel.rs:254`). Do not introduce `Rc<RefCell<...>>` or other sharing constructs here.

- [ ] **Step 3: Add the `use` import if needed**

If the file does not already import `emViewAnimator`, add at the top of the file (in the `use crate::...` block that already imports from `emViewAnimator`):

```rust
use crate::emViewAnimator::emViewAnimator;
```

Check current imports first:
```
grep -n "emViewAnimator" crates/emcore/src/emWindow.rs
```
Only add the import if the trait name is not already in scope.

- [ ] **Step 4: Run clippy and full test suite**

```
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean, 2414/2414 pass.

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emWindow.rs
git commit -m "feat(emWindow): forward input to active_animator before InputToView (C++ emView.cpp:1004)"
```

### Task 4.3: Forward from emSubViewPanel::Input

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs` — the `Behavior::Input` impl (around line 144-219).

**Rationale:** The sub-view's `emView` also has an animator — stored on `emSubViewPanel.active_animator`. C++ `emSubViewPanel::Input` (emSubViewPanel.cpp:69-78) routes through `SubViewPort->InputToView`, which in C++ dispatches to the sub-`emView::Input` which forwards to its `ActiveAnimator`. In Rust, the sub-view's `Input` is inlined into `emSubViewPanel::Behavior::Input`, so we forward directly here.

- [ ] **Step 1: Add the animator forward at the top of the Input impl**

Locate the Rust `Behavior::Input` impl in `emSubViewPanel.rs:144-219`. The function begins:

```rust
    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // C++ emSubViewPanel::Input:
        //   ...
        if event.is_mouse_event() || event.is_touch_event() {
            self.sub_view
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }
```

Insert, immediately inside the function body (before the mouse/touch focus block), an animator forward. Since `event: &emInputEvent` is shared, clone to a local mutable and use the clone both for the forward AND for the downstream dispatch (so that an `eat()` here propagates).

Replace the function body's opening block:

```rust
    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // C++ emSubViewPanel::Input:
        //   ...
        if event.is_mouse_event() || event.is_touch_event() {
            self.sub_view
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }
```

with:

```rust
    fn Input(
        &mut self,
        event: &emInputEvent,
        state: &PanelState,
        input_state: &emInputState,
    ) -> bool {
        // C++ emView.cpp:1004 via emSubViewPanel.cpp:77: forward input to
        // the sub-view's ActiveAnimator first. Rust stores the sub-view's
        // animator on emSubViewPanel (not on the sub-view's emView), so this
        // forward happens here.
        let mut event_local = event.clone();
        if let Some(mut anim) = self.active_animator.take() {
            crate::emViewAnimator::emViewAnimator::Input(
                anim.as_mut(),
                &mut event_local,
                input_state,
            );
            self.active_animator = Some(anim);
        }
        let event = &event_local;

        // C++ emSubViewPanel::Input:
        //   ...
        if event.is_mouse_event() || event.is_touch_event() {
            self.sub_view
                .SetFocused(&mut self.sub_tree, state.is_focused());
        }
```

The `let event = &event_local;` shadow ensures every subsequent reference to `event` in the function body operates on the possibly-eaten clone without changing any other line.

- [ ] **Step 2: Clean up the stale emView::Input comment**

In `crates/emcore/src/emView.rs`, locate `emView::Input` at line 3595. The doc block and inline comment currently reference `PHASE-6-FOLLOWUP: forward to active animator first (C++ emView.cpp:1004)`. Update to reflect that the forward lives in the callers now.

Replace the inline block:

```rust
    pub fn Input(
        &mut self,
        _tree: &mut PanelTree,
        _event: &crate::emInput::emInputEvent,
        state: &crate::emInputState::emInputState,
    ) {
        // PHASE-6-FOLLOWUP: forward to active animator first (C++ emView.cpp:1004)
        // emView.cpp:1004: forward to active animator first.
        // Animator resolution: emWindow holds the active animator; input
        // forwards to emView which wakes the engine to trigger animation.

        // emView.cpp:1006-1014: cursor-invalid on mouse move.
```

with:

```rust
    pub fn Input(
        &mut self,
        _tree: &mut PanelTree,
        _event: &crate::emInput::emInputEvent,
        state: &crate::emInputState::emInputState,
    ) {
        // C++ emView.cpp:1004: forward input to ActiveAnimator first.
        // Rust-arch note: the active animator lives on emWindow
        // (see emWindow::dispatch_input) and on emSubViewPanel
        // (see emSubViewPanel::Behavior::Input), not on emView — by the
        // Phase 5/6 design decision. Those callers forward the event to
        // their animator slot BEFORE invoking this method, so by the time
        // Input runs here the event may already have been eaten.

        // emView.cpp:1006-1014: cursor-invalid on mouse move.
```

Additionally, update the top-of-method doc comment. Find this text inside the `///` doc block (emView.rs:3590-3594):

```rust
    /// PHASE-6-FOLLOWUP: migrate the VIF-chain + panel-broadcast dispatch
    /// from `emWindow::dispatch_input` into this method; invoke
    /// `RecurseInput` once its Rust port exists. Also forward to the active
    /// animator first (C++ emView.cpp:1004) — the animator currently lives
    /// on `emWindow`, not `emView`, so this routing cannot happen here yet.
```

Replace with:

```rust
    /// PHASE-6-FOLLOWUP: migrate the VIF-chain + panel-broadcast dispatch
    /// from `emWindow::dispatch_input` into this method; invoke
    /// `RecurseInput` once its Rust port exists. The animator forward
    /// (C++ emView.cpp:1004) is handled by the caller sites
    /// (`emWindow::dispatch_input`, `emSubViewPanel::Behavior::Input`)
    /// because the animator lives on those owners, not on `emView`.
```

- [ ] **Step 3: Verify all PHASE-6-FOLLOWUP markers for W1a are cleared**

Run:
```
grep -n "PHASE-6-FOLLOWUP" crates/emcore/src/emView.rs
```
Expected: the animator-forward markers previously at `emView::Input` (execution-debt §2.2 row "~3524") are gone. Other `PHASE-6-FOLLOWUP` markers (PopupPlaceholder, RawVisitAbs, VIF-chain migration note) remain — those are Wave 3 / Wave 4 concerns and MUST NOT be touched in this plan.

- [ ] **Step 4: Run clippy, full tests, and golden baseline**

```
cargo clippy -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```
Expected: clippy clean, nextest 2414/2414 pass, golden 237/243 (baseline parity; the set of failing goldens must not grow — compare against execution-debt §8.4).

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emSubViewPanel.rs crates/emcore/src/emView.rs
git commit -m "feat(emSubViewPanel): forward input to active_animator; clean emView::Input docs"
```

---

## Acceptance Criteria

- [ ] `cargo clippy -- -D warnings` clean with no new `#[allow]`/`#[expect]`.
- [ ] `cargo-nextest ntr` passes all tests (baseline 2409 + 5 new = 2414).
- [ ] `cargo test --test golden -- --test-threads=1`: 237 passed / 6 failed, same 6 tests as execution-debt §8.4. No new golden regressions.
- [ ] No new `DIVERGED:`/`RUST-DIVERGED:` annotations.
- [ ] The three `PHASE-6-FOLLOWUP:` markers at `emView::Input` animator-forward are gone (at emView.rs:~3524 doc block and inline comment).
- [ ] The four reviewer findings W1a / W1b / W1c / W1d from `docs/superpowers/notes/2026-04-18-emview-followups-execution-debt.md` §§3.1, 3.2, 3.3, 4.8 are each addressed by a commit with a matching message.
- [ ] Total commit count: 5 (one per subtask — 1 for W1d, 1 for W1c, 1 for W1b, 3 for W1a = 5 commits).

## Out of scope (explicit)

The following are named in the execution-debt report but belong to later waves. Do NOT touch them in this plan:
- PopupPlaceholder removal / real popup creation (W3, execution-debt §2.2).
- Phase 11 visit-stack rewrite (W4, execution-debt §1).
- Phase-8 two-engine test promotion (W5a, execution-debt §2.3).
- Multi-window pixel-tallness design (W5b, execution-debt §3.4).
- Any residual `PHASE-6-FOLLOWUP:` markers other than the animator-forward ones at `emView::Input`.
- Minor cleanups (W2 housekeeping, execution-debt §4.3–§4.7, §7.4) — these land in a separate opportunistic PR, not here.
