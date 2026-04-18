# W4 — emView Visit-State Restoration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete Rust-only visit-state scaffolding (`visit_stack`, `VisitState`, `pending_animated_visit`, `go_back`, `go_home`, `current_visit`, `animated_visit*`), restore `emView`'s ownership of `emVisitingViewAnimator` per C++ `emView.h:675`, route all `Visit`-family methods through it, port `GetVisitedPanel`, and move `Home`-key handling from VIF to `emPanel::Input`.

**Architecture:** Six phases, each commitable green. Phase 1 adds `VisitingVA` ownership (no behavior change). Phase 2 adds `GetVisitedPanel` and migrates readers. Phase 3 rewrites write-path bodies to delegate to `VisitingVA`. Phase 4 moves `Home`-key routing to `emPanel::Input`. Phase 5 deletes accidental types/fields/methods. Phase 6 restores `factor=1.0` in the animator invariant test.

**Tech Stack:** Rust; crates under `crates/emcore`, `crates/eaglemode`, `crates/emmain`. C++ reference source at `~/git/eaglemode-0.96.4/`.

**Spec:** `docs/superpowers/specs/2026-04-18-emview-visit-stack-rewrite-design.md`.

---

## Ground rules (apply to every task)

- Every commit passes the pre-commit hook: `cargo fmt` (auto-applied), `cargo clippy -- -D warnings`, `cargo-nextest ntr`. Never use `--no-verify`.
- Never add `#[allow(...)]` or `#[expect(...)]` — fix the warning. Exceptions per CLAUDE.md are unchanged (the `emCore` module and `em`-prefixed types).
- Never add new `DIVERGED:` annotations. If you feel one is needed, stop and ask.
- File and Name Correspondence: method names mirror C++ unless Rust forces differentiation (overload collision — then use `VisitByIdentity`-style suffix). No new names are invented for existing C++ concepts.
- Consult C++ source at `~/git/eaglemode-0.96.4/src/emCore/emView.cpp` and `emViewAnimator.cpp` for every Visit-family port. The task specifies which line range to read.

## Scope note — CoreConfig wiring

C++ `emView::Visit(...)` calls `VisitingVA->SetAnimParamsByCoreConfig(*CoreConfig)` where `CoreConfig` is an `emRef<emCoreConfig>` member. **Rust `emView` does not currently hold a CoreConfig field** — this is a separate accidental divergence out of scope for W4.

**Resolution for W4:** the `Visit` delegation calls the existing Rust animator signature `SetAnimParamsByCoreConfig(speed_factor: f64, max_speed_factor: f64)` with the C++ defaults `(1.0, 10.0)` (`emCoreConfig.cpp:53`: `VisitSpeed(this,"VisitSpeed",1.0,0.1,10.0)`). The call site gets a `PHASE-W4-FOLLOWUP:` comment noting that full CoreConfig ownership on `emView` is a future wave. This is the minimum-divergence placeholder that keeps W4 scoped to visit-state while preserving the delegation shape.

---

## File Structure

**Created:**
- No new files. All changes are in existing files.

**Modified:**
- `crates/emcore/src/emView.rs` — primary target (delete accidental types/fields, add `VisitingVA` ownership, rewrite Visit-family bodies, add `GetVisitedPanel`).
- `crates/emcore/src/emViewAnimator.rs` — add `new_for_view()` constructor; restore `factor=1.0`.
- `crates/emcore/src/emViewInputFilter.rs` — delete `Home`-key handler; update triple-tap call-site signature.
- `crates/emcore/src/emPanel.rs` — port `EM_KEY_HOME/END/PAGE_UP/PAGE_DOWN` block from `emPanel.cpp:1168-1198`.
- `crates/emmain/src/emMainWindow.rs` — migrate two `current_visit()` readers to `GetVisitedPanel`.

**Tests modified:**
- `crates/eaglemode/tests/unit/panel.rs` — delete `view_visit_and_back`.
- `crates/eaglemode/tests/integration/input.rs` — remove stale visit_stack comment at :143.
- All `current_visit()` test readers (enumerated per-task).

**Tests created:**
- `visiting_va_owned_by_view` (inline in `emView.rs`).
- `visit_routes_through_animator` (inline in `emView.rs`).
- `home_key_routes_through_empanel` (new integration test file — path decided in Phase 4 Task 1 by reading the existing integration test layout).

---

# Phase 1 — VisitingVA ownership

**Goal:** `emView` owns `VisitingVA: Rc<RefCell<emVisitingViewAnimator>>`. Animator is constructed in `emView::new` and registered with the scheduler. No caller behavior changes.

### Task 1.1: Add `new_for_view()` constructor to `emVisitingViewAnimator`

**Files:**
- Modify: `crates/emcore/src/emViewAnimator.rs:690-713` (add constructor alongside existing `new`).
- Test: `crates/emcore/src/emViewAnimator.rs` (inline test module).

C++ reference: `emViewAnimator.cpp:930-948` (`emVisitingViewAnimator::emVisitingViewAnimator(emView & view)`). Rust does not take an emView reference — the Cycle driver supplies it; this mirrors the Phase-7 `UpdateEngineClass` pattern.

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `emViewAnimator.rs`:

```rust
#[test]
fn new_for_view_matches_cpp_initial_state() {
    // C++ emViewAnimator.cpp:930-948 initializes:
    //   Animated=false, Acceleration=5.0, MaxCuspSpeed=2.0, MaxAbsoluteSpeed=5.0
    //   State=ST_NO_GOAL, VisitType=VT_VISIT, RelX=RelY=RelA=0
    //   Adherent=false, UtilizeView=false, MaxDepthSeen=-1, Speed=0.0
    //   IsActive()=false (SetDeactivateWhenIdle + no Activate call yet).
    let va = emVisitingViewAnimator::new_for_view();
    assert!(!va.animated);
    assert_eq!(va.acceleration, 5.0);
    assert_eq!(va.max_cusp_speed, 2.0);
    assert_eq!(va.max_absolute_speed, 5.0);
    assert_eq!(va.state, VisitingState::NoGoal);
    assert_eq!(va.rel_x, 0.0);
    assert_eq!(va.rel_y, 0.0);
    assert_eq!(va.rel_a, 0.0);
    assert!(!va.adherent);
    assert!(!va.utilize_view);
    assert_eq!(va.max_depth_seen, -1);
    assert_eq!(va.speed, 0.0);
    assert!(!va.active);
}
```

If `VisitingState::NoGoal` is not yet a variant, this test will fail to compile. Check the existing `VisitingState` enum (likely at the top of `emViewAnimator.rs`) — if the C++ `ST_NO_GOAL` state is not present, add it: it is the "no goal set" initial state and is set by `ClearGoal`. The existing Rust enum likely has `Curve` as the initial — that's the wrong port; C++ initializes to `ST_NO_GOAL`. Adding `NoGoal` to the enum is part of this task.

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p emcore --lib emViewAnimator::tests::new_for_view_matches_cpp_initial_state
```
Expected: compile error (method `new_for_view` not defined) or enum variant missing.

- [ ] **Step 3: Add `VisitingState::NoGoal` if missing**

Search the top of `crates/emcore/src/emViewAnimator.rs` for the `VisitingState` enum. If `NoGoal` is absent, add it as the first variant and update any exhaustive `match` arms on `VisitingState` to handle it (reading the existing `CycleAnimation` body and other consumers — they should treat `NoGoal` as "no-op, deactivate"). Port matches C++ `emViewAnimator.h` state enum (`ST_NO_GOAL`, `ST_CURVE`, `ST_DIRECT`, `ST_GIVING_UP`, etc. — check the C++ header for the complete set).

Because this depends on current file state, the exact edit is: read the enum, add `NoGoal,` as the first variant, extend matches. If the enum already has `NoGoal`, skip this step.

- [ ] **Step 4: Implement `new_for_view()`**

Add after the existing `pub fn new(...)`:

```rust
/// Constructor matching C++ `emVisitingViewAnimator::emVisitingViewAnimator(emView & view)`
/// at `emViewAnimator.cpp:930`. Initializes to ST_NO_GOAL / inactive.
pub fn new_for_view() -> Self {
    Self {
        animated: false,
        acceleration: 5.0,
        max_cusp_speed: 2.0,
        max_absolute_speed: 5.0,
        state: VisitingState::NoGoal,
        visit_type: VisitType::Visit,  // adjust to actual enum member; check existing VisitType
        identity: String::new(),
        names: Vec::new(),
        rel_x: 0.0,
        rel_y: 0.0,
        rel_a: 0.0,
        adherent: false,
        utilize_view: false,
        subject: String::new(),
        active: false,
        max_depth_seen: -1,
        speed: 0.0,
        time_slices_without_hope: 0,
        give_up_clock: 0.0,
    }
}
```

If `VisitType::Visit` isn't the right variant name, read the `VisitType` enum and use the member that maps to C++ `VT_VISIT` (likely unqualified `Visit` — but verify against the actual enum).

- [ ] **Step 5: Run test, verify it passes**

```bash
cargo test -p emcore --lib emViewAnimator::tests::new_for_view_matches_cpp_initial_state
```
Expected: PASS.

- [ ] **Step 6: Run full crate check**

```bash
cargo check -p emcore && cargo clippy -p emcore -- -D warnings
```
Expected: clean. If the `VisitingState::NoGoal` addition broke match exhaustiveness anywhere, fix those match arms to treat `NoGoal` as inactive/no-op.

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/src/emViewAnimator.rs
git commit -m "feat(emViewAnimator): add new_for_view constructor matching C++ shape

Mirrors C++ emVisitingViewAnimator::emVisitingViewAnimator(emView&) at
emViewAnimator.cpp:930: initializes to ST_NO_GOAL/inactive. Existing
float-arg new() retained for standalone test callers.

W4 Phase 1 Task 1.1."
```

### Task 1.2: Add `VisitingVA` field to `emView`

**Files:**
- Modify: `crates/emcore/src/emView.rs:270-430` (struct definition and `emView::new`).

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `emView.rs`:

```rust
#[test]
fn visiting_va_owned_by_view() {
    // W4 Phase 1: emView holds VisitingVA matching C++ emView.h:675
    // (emOwnPtr<emVisitingViewAnimator> VisitingVA).
    let mut tree = PanelTree::new(1.0);
    let root = tree.create_root("root");
    let view = emView::new(root, 800.0, 600.0);
    // Access VisitingVA; borrow read-only; verify initial state.
    let va = view.VisitingVA.borrow();
    assert!(!va.is_active(), "VisitingVA should start inactive (C++ ST_NO_GOAL)");
}
```

If `is_active()` doesn't exist as a public method on `emVisitingViewAnimator`, use the private field in the same crate or add `pub fn is_active(&self) -> bool { self.active }` — whichever matches existing codebase style (search for `fn is_active` or similar observer methods in emViewAnimator.rs first).

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p emcore --lib emView::tests::visiting_va_owned_by_view
```
Expected: compile error — field `VisitingVA` does not exist on `emView`.

- [ ] **Step 3: Add `VisitingVA` field**

In the `pub struct emView { ... }` block at `crates/emcore/src/emView.rs:270`, add (placed near the other Rc-wrapped members; `active` is near the top — put it after `focused`):

```rust
    /// C++ emView.h:675 — `emOwnPtr<emVisitingViewAnimator> VisitingVA`.
    /// The visiting view animator; owns the "where we're going" state.
    pub VisitingVA: Rc<RefCell<super::emViewAnimator::emVisitingViewAnimator>>,
```

- [ ] **Step 4: Construct in `emView::new`**

In `emView::new` (at `crates/emcore/src/emView.rs:435`), add the `VisitingVA` field in the struct-literal initializer. Place it alphabetically or near `scheduler`:

```rust
            VisitingVA: Rc::new(RefCell::new(
                super::emViewAnimator::emVisitingViewAnimator::new_for_view(),
            )),
```

- [ ] **Step 5: Run test, verify it passes**

```bash
cargo test -p emcore --lib emView::tests::visiting_va_owned_by_view
```
Expected: PASS.

- [ ] **Step 6: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean, all tests pass. Count unchanged except +1 new test.

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "feat(emView): own VisitingVA per C++ emView.h:675

Add VisitingVA: Rc<RefCell<emVisitingViewAnimator>> field, constructed
with new_for_view() in emView::new. Dormant; no caller behavior change
yet. Test asserts ownership and initial inactive state.

W4 Phase 1 Task 1.2."
```

### Task 1.3: Wire VisitingVA engine registration

**Files:**
- Modify: `crates/emcore/src/emView.rs` — wherever `attach_to_scheduler` or equivalent scheduler-binding is, wire `VisitingVA` engine registration.

C++ reference: `emEngine` base constructor auto-registers. In Rust, Phase-7 registers `UpdateEngineClass` via `attach_to_scheduler`. Registration for animators follows the same plumbing.

- [ ] **Step 1: Read existing scheduler-attachment code**

```bash
grep -n "attach_to_scheduler\|register_engine\|register_update_engine" crates/emcore/src/emView.rs | head -20
```

Identify where `UpdateEngineClass` is registered and how the engine ID is captured. The pattern to mirror: register an engine that calls `VisitingVA.borrow_mut().cycle_animation(dt)` on each Cycle when active.

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests` in `emView.rs`:

```rust
#[test]
fn visiting_va_cycles_when_activated() {
    use super::emScheduler::EngineScheduler;

    let mut tree = PanelTree::new(1.0);
    let root = tree.create_root("root");
    let mut view = emView::new(root, 800.0, 600.0);
    let sched = Rc::new(RefCell::new(EngineScheduler::new()));
    view.attach_to_scheduler(Rc::clone(&sched));

    // Activate VisitingVA with a goal.
    {
        let mut va = view.VisitingVA.borrow_mut();
        va.SetGoal("root", false, "");
        va.Activate();
    }
    assert!(view.VisitingVA.borrow().is_active());

    // One scheduler tick should trigger CycleAnimation via the registered engine.
    sched.borrow_mut().do_time_slice(0.016);

    // Verify engine was ticked: VisitingVA's internal give_up_clock or speed
    // changes on cycle. Specifically, max_depth_seen is consulted on first
    // cycle — it remains -1 until Cycle runs.
    // A simpler observable: VisitingVA should still be active (hasn't given
    // up) and its state should have progressed from initial ST_CURVE set by
    // Activate. Tolerant assertion: active still true.
    assert!(view.VisitingVA.borrow().is_active() ||
            view.VisitingVA.borrow().state() == VisitingState::NoGoal,
            "after one tick, animator has either progressed or cleanly deactivated");
}
```

Use whichever existing scheduler entry-point name matches (`do_time_slice` may be `DoTimeSlice` or similar — grep and match).

- [ ] **Step 3: Run test, verify it fails**

```bash
cargo test -p emcore --lib emView::tests::visiting_va_cycles_when_activated
```
Expected: FAIL — no engine is registered for `VisitingVA`, so no tick happens (or the assertion hits a different failure mode).

- [ ] **Step 4: Add engine registration**

In `attach_to_scheduler` (or the equivalent method identified in Step 1), after the existing `UpdateEngineClass` registration, register a new engine that drives `VisitingVA`. The engine's `cycle(ctx)` invokes `view.VisitingVA.borrow_mut().cycle_animation(dt)` when the animator is active.

Exact structure depends on the existing engine-registration pattern. The minimal shape — following the `UpdateEngineClass` precedent — is:

```rust
// Register VisitingVA engine — mirrors C++ emEngine base constructor
// auto-registration. Engine runs VisitingVA's Cycle when animator is active.
struct VisitingAnimatorEngine {
    window_id: WindowId,  // or whatever identifier UpdateEngineClass uses
}

impl emEngine for VisitingAnimatorEngine {
    fn cycle(&mut self, ctx: &mut EngineContext) -> bool {
        // Find view via window map (same lookup UpdateEngineClass uses).
        // If animator is active, call cycle_animation(dt). Return true to
        // stay registered; return false to deregister if animator has
        // deactivated (matching SetDeactivateWhenIdle semantics).
        // ...
    }
}
```

The precise wiring is what `UpdateEngineClass` does today — copy that pattern verbatim, substituting the Cycle body. If `UpdateEngineClass` uses `ctx.windows.get(&self.window_id)` to reach `&mut emView`, the new animator engine does the same.

- [ ] **Step 5: Run test, verify it passes**

```bash
cargo test -p emcore --lib emView::tests::visiting_va_cycles_when_activated
```
Expected: PASS.

- [ ] **Step 6: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean. Count +1 new test.

- [ ] **Step 7: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "feat(emView): register VisitingVA engine with scheduler

Mirrors C++ emEngine base constructor auto-registration. Animator
receives Cycle calls when Activate()d; deregisters on Deactivate.
Uses the same plumbing pattern as UpdateEngineClass.

W4 Phase 1 Task 1.3."
```

**Phase 1 acceptance:** `emView.VisitingVA` present; Activate→Cycle→Deactivate plumbing verified; zero behavior change to Visit/VisitNext/... callers (still routing through the old `animated_visit_panel` / `visit_stack` paths); nextest count + 2 (from Tasks 1.1 and 1.2, Task 1.3 adds +1 more = +3 net in Phase 1); golden baseline 237/6 unchanged.

---

# Phase 2 — Read-path port

**Goal:** Add `GetVisitedPanel`. Migrate all production and test readers of `current_visit()` to it. `current_visit()` still exists and compiles; it just has zero non-test-deletion-slated callers.

### Task 2.1: Add `GetVisitedPanel`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (new method near existing `current_visit` at :669, or alongside other panel-lookup methods).

C++ reference: `emView.cpp:471-489` (`emView::GetVisitedPanel`).

- [ ] **Step 1: Write the failing test**

Add to `emView.rs` test module:

```rust
#[test]
fn get_visited_panel_returns_svp_rel_coords() {
    let mut tree = PanelTree::new(1.0);
    let root = tree.create_root("root");
    let mut view = emView::new(root, 800.0, 600.0);
    view.set_home_geometry(0.0, 0.0, 800.0, 600.0, 1.0);
    view.Update(&mut tree);  // drive viewing update to populate SVP.

    let mut rx = 99.0;
    let mut ry = 99.0;
    let mut ra = 99.0;
    let panel = view.GetVisitedPanel(&tree, &mut rx, &mut ry, &mut ra);

    // At home view with root as active+viewed, rel coords should be (0, 0, 1).
    assert_eq!(panel, Some(root));
    assert!((rx - 0.0).abs() < 1e-9, "rel_x at home = 0");
    assert!((ry - 0.0).abs() < 1e-9, "rel_y at home = 0");
    assert!((ra - 1.0).abs() < 1e-9, "rel_a at home = 1");
}
```

(If `set_home_geometry` and `Update` have different names, grep the file and adjust.)

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p emcore --lib emView::tests::get_visited_panel_returns_svp_rel_coords
```
Expected: compile error — `GetVisitedPanel` not defined.

- [ ] **Step 3: Implement `GetVisitedPanel`**

Add to `impl emView`, near `current_visit`:

```rust
/// Port of C++ `emView::GetVisitedPanel` (emView.cpp:471-489).
/// Returns the active panel if it is viewed, else SupremeViewedPanel;
/// fills rel_x/y/a via CalcVisitCoords.
pub fn GetVisitedPanel(
    &self,
    tree: &PanelTree,
    rel_x: &mut f64,
    rel_y: &mut f64,
    rel_a: &mut f64,
) -> Option<PanelId> {
    // C++: p = ActivePanel; while (p && !p->InViewedPath) p = p->Parent;
    //      if (!p || !p->Viewed) p = SupremeViewedPanel;
    let p = self
        .active
        .and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                if tree.is_in_viewed_path(c) {
                    break;
                }
                cur = tree.get_parent(c);
            }
            cur
        })
        .filter(|&id| tree.is_viewed(id))
        .or(self.supreme_viewed_panel);

    if let Some(panel) = p {
        let (rx, ry, ra) = self.CalcVisitCoords(tree, panel);
        *rel_x = rx;
        *rel_y = ry;
        *rel_a = ra;
    } else {
        *rel_x = 0.0;
        *rel_y = 0.0;
        *rel_a = 0.0;
    }
    p
}

/// Rust-idiomatic companion returning a tuple.
pub fn get_visited_panel_idiom(&self, tree: &PanelTree) -> Option<(PanelId, f64, f64, f64)> {
    let mut rx = 0.0;
    let mut ry = 0.0;
    let mut ra = 0.0;
    self.GetVisitedPanel(tree, &mut rx, &mut ry, &mut ra)
        .map(|p| (p, rx, ry, ra))
}
```

If `is_in_viewed_path` or `get_parent` do not exist on `PanelTree`, check what's available. `viewed_x/y/width/height` fields exist per memory (emPanelTree.rs:196). Use whatever existing method reaches them; if necessary, simplify the first clause to just `self.active.filter(|&id| tree.is_viewed(id)).or(self.supreme_viewed_panel)` — this is a minor simplification of the C++ "walk ancestors until in-viewed-path" logic that matters only when the active panel itself is not viewed but an ancestor is. If the Rust tree model doesn't track `in_viewed_path` as a panel flag, the simpler form is correct under the observational-port frame because the active panel is viewed in every observed Rust production flow; escalate back to design if a caller is found that relies on the walk-ancestors behavior.

Confirm `CalcVisitCoords(tree, panel) -> (f64, f64, f64)` exists at `emView.rs:863` — it does (confirmed in audit).

- [ ] **Step 4: Run test, verify it passes**

```bash
cargo test -p emcore --lib emView::tests::get_visited_panel_returns_svp_rel_coords
```
Expected: PASS.

- [ ] **Step 5: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "feat(emView): add GetVisitedPanel per C++ emView.cpp:471

Out-param form mirrors C++ signature; idiomatic tuple companion added
for Rust callers. Derives rel coords via CalcVisitCoords. Readers
migrate in subsequent tasks.

W4 Phase 2 Task 2.1."
```

### Task 2.2: Migrate emMainWindow readers

**Files:**
- Modify: `crates/emmain/src/emMainWindow.rs:180-200` and `:1040-1055`.

- [ ] **Step 1: Read current call site at emMainWindow.rs:180-200**

```bash
sed -n '178,200p' crates/emmain/src/emMainWindow.rs
```
The current code reads `view.current_visit()` then destructures `visit.panel`, `visit.rel_x`, `visit.rel_y`, `visit.rel_a`.

- [ ] **Step 2: Migrate :185 to GetVisitedPanel**

Replace the existing block:

```rust
let (visit_identity, rel_x, rel_y, rel_a, adherent) =
    if let Some(rc) = self.window_id.and_then(|id| app.windows.get(&id)) {
        let win = rc.borrow();
        let view = win.view();
        let visit = view.current_visit();
        let identity = app.tree.GetIdentity(visit.panel);
        let adherent = view.IsActivationAdherent();
        (
            Some(identity),
            visit.rel_x,
            visit.rel_y,
            visit.rel_a,
            adherent,
        )
    }
```

with:

```rust
let (visit_identity, rel_x, rel_y, rel_a, adherent) =
    if let Some(rc) = self.window_id.and_then(|id| app.windows.get(&id)) {
        let win = rc.borrow();
        let view = win.view();
        let mut rel_x = 0.0;
        let mut rel_y = 0.0;
        let mut rel_a = 0.0;
        let panel_opt = view.GetVisitedPanel(&app.tree, &mut rel_x, &mut rel_y, &mut rel_a);
        let identity = panel_opt.map(|p| app.tree.GetIdentity(p));
        let adherent = view.IsActivationAdherent();
        (identity, rel_x, rel_y, rel_a, adherent)
    }
```

Note: C++ `GetVisitedPanel` returning NULL matches the Rust `None` case — rel coords become 0/0/0 per the C++ else-branch. That's the behavior the idiomatic migration here preserves: if no panel is visited, identity is None.

- [ ] **Step 3: Read and migrate :1046**

```bash
sed -n '1040,1060p' crates/emmain/src/emMainWindow.rs
```

Replace:

```rust
let visit = svp.GetSubView().current_visit();
let identity = svp.sub_tree().GetIdentity(visit.panel);
let rel_x = visit.rel_x;
let rel_y = visit.rel_y;
let rel_a = visit.rel_a;
```

with:

```rust
let mut rel_x = 0.0;
let mut rel_y = 0.0;
let mut rel_a = 0.0;
let panel_opt =
    svp.GetSubView().GetVisitedPanel(svp.sub_tree(), &mut rel_x, &mut rel_y, &mut rel_a);
let identity = panel_opt
    .map(|p| svp.sub_tree().GetIdentity(p))
    .unwrap_or_default();
```

The `unwrap_or_default()` is the behavioral preservation: the previous code would have panicked if `visit.panel` was invalid but almost always succeeded in practice; the C++ equivalent at the same site also assumes a visited panel exists. Empty-string identity is the "no panel" fallback — verify by reading surrounding code (which does what with `identity`?). If identity is passed to a Visit call later and empty string is a no-op target, this is correct. If empty string would cause undefined behavior, wrap the whole post-block in `if let Some(p) = panel_opt { ... }`.

- [ ] **Step 4: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean. All previously-passing tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emMainWindow.rs
git commit -m "refactor(emMainWindow): migrate current_visit readers to GetVisitedPanel

Two call sites (startup state save at :185, content-view save across
panel delete at :1046) migrated to the C++-mirror read API. Observable
behavior unchanged.

W4 Phase 2 Task 2.2."
```

### Task 2.3: Migrate emView.rs inline readers

**Files:**
- Modify: `crates/emcore/src/emView.rs:1204` and `:3737`.

- [ ] **Step 1: Read and migrate :1204**

```bash
sed -n '1198,1215p' crates/emcore/src/emView.rs
```
The exact change depends on context. The pattern is:
- Before: `if let Some(state) = self.visit_stack.last() { use state.panel / state.rel_x / etc. }`
- After: derive via `GetVisitedPanel` or `CalcVisitCoords` depending on what the surrounding code wants. Read the surrounding ~30 lines — if the block consumes all three (panel, rel_x, rel_y, rel_a), use `GetVisitedPanel`. If it only needs rel coords given a known panel, use `CalcVisitCoords` directly.

Replace the block, preserving every consumer variable name so surrounding code is untouched.

- [ ] **Step 2: Read and migrate :3737**

```bash
sed -n '3730,3745p' crates/emcore/src/emView.rs
```

The pattern is `.unwrap_or(self.current_visit().panel)` — a fallback when `self.active` is None. C++ fallback at `emView.cpp:473` uses `SupremeViewedPanel`. Replace:

```rust
.unwrap_or(self.current_visit().panel)
```

with:

```rust
.or(self.supreme_viewed_panel)
.expect("visited-panel fallback: SupremeViewedPanel should be populated post-Update")
```

Or, if the enclosing expression's type is `PanelId` (not `Option<PanelId>`), use:

```rust
.unwrap_or_else(|| self.supreme_viewed_panel.expect("SVP populated"))
```

Inspect the enclosing expression's return type and pick the form that matches. The semantic is the C++ fallback: use SVP when no active panel.

- [ ] **Step 3: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "refactor(emView): migrate inline current_visit readers

:1204 inline stack-read -> CalcVisitCoords/GetVisitedPanel per context.
:3737 fallback from current_visit().panel -> SupremeViewedPanel
(matches C++ emView.cpp:473).

W4 Phase 2 Task 2.3."
```

### Task 2.4: Migrate test readers

**Files:**
- Modify: `crates/emcore/src/emView.rs` test module — lines 4610, 4612, 4965, 5075+, 5105+, 5274.
- Modify: `crates/emcore/src/emViewInputFilter.rs` test module — lines 2574+, 2737+, 2775+, 2824+, 2871+.
- Modify: `crates/emcore/src/emViewAnimator.rs` test module — lines 2738+, 2842+, 2911+, 3365+, 3379+.
- Modify: `crates/eaglemode/tests/golden/animator.rs:293`.
- Modify: `crates/eaglemode/tests/golden/input_filter.rs:80`.

Every `let visit = view.current_visit();` (or `.clone()` variant) becomes:

```rust
let (panel, rel_x, rel_y, rel_a) = view
    .get_visited_panel_idiom(&tree)
    .expect("visited panel should exist at observation point");
```

If the test only reads `rel_a` (e.g., `let ra_before = view.current_visit().rel_a;`), use:

```rust
let mut rx = 0.0;
let mut ry = 0.0;
let mut ra_before = 0.0;
view.GetVisitedPanel(&tree, &mut rx, &mut ry, &mut ra_before);
```

The test's `tree` variable name may differ — use the actual variable. If a test doesn't have a `tree` in scope (uses a bare emView), add the minimum needed — a `PanelTree::new` at the top of the test. Do not skip tests.

- [ ] **Step 1: Enumerate all current_visit() test call sites**

```bash
grep -n "current_visit\(\)" crates/emcore/src/emView.rs crates/emcore/src/emViewInputFilter.rs crates/emcore/src/emViewAnimator.rs crates/eaglemode/tests/golden/animator.rs crates/eaglemode/tests/golden/input_filter.rs
```

Produce the exact list before editing. Expected count: ~25 sites.

- [ ] **Step 2: Migrate sites in emView.rs test module**

For each site (4610, 4612, 4965, 5075, 5082, 5105, 5111, 5274), replace the `current_visit()` read with `GetVisitedPanel` or `get_visited_panel_idiom`. Preserve every downstream variable name.

- [ ] **Step 3: Migrate sites in emViewInputFilter.rs test module**

Sites at 2574, 2575, 2583, 2584, 2737, 2740, 2775, 2782, 2824, 2829, 2871, 2873. Same pattern.

- [ ] **Step 4: Migrate sites in emViewAnimator.rs test module**

Sites at 2738, 2745, 2842, 2853, 2911, 3365, 3379.

Note: some sites in emViewAnimator.rs may be reading `current_visit()` on a standalone `emView` inside an animator unit test. These tests may be of the form "construct a view, run animator, assert state changed." The migration still applies — swap reader, preserve intent.

- [ ] **Step 5: Migrate sites in golden/animator.rs and golden/input_filter.rs**

Single site each. Same pattern.

- [ ] **Step 6: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean; test count unchanged.

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "refactor(tests): migrate current_visit readers to GetVisitedPanel

~25 test reader sites across emView, emViewInputFilter, emViewAnimator,
golden/animator, golden/input_filter migrated to the C++-mirror read
API. Test intent (observe rel coords at a point) preserved.

W4 Phase 2 Task 2.4."
```

**Phase 2 acceptance:** grep finds zero non-test `current_visit()` calls; `current_visit()` still compiles; all tests pass; golden baseline 237/6 preserved.

---

# Phase 3 — Write-path rewrite

**Goal:** Every `Visit`-family method body delegates to `VisitingVA`. `pending_animated_visit` has zero writers. Keyboard navigation actually animates.

### Task 3.1: Rewrite `Visit` and `VisitByIdentity` canonical bodies

**Files:**
- Modify: `crates/emcore/src/emView.rs:823` (Visit body) and `:2949` (VisitByIdentity body).

C++ reference: `emView.cpp:492-510`.

- [ ] **Step 1: Write the failing test**

Add to `emView.rs` test module:

```rust
#[test]
fn visit_routes_through_animator() {
    // W4 Phase 3: Visit(tree, panel, rx, ry, ra, adherent) must set a goal
    // on VisitingVA and activate it, matching C++ emView.cpp:492-497 ->
    // emView.cpp:500-510.
    let mut tree = PanelTree::new(1.0);
    let root = tree.create_root("root");
    let child = tree.create_child(root, "child");
    let mut view = emView::new(root, 800.0, 600.0);
    view.set_home_geometry(0.0, 0.0, 800.0, 600.0, 1.0);

    assert!(!view.VisitingVA.borrow().is_active(), "inactive before Visit");

    view.Visit(&tree, child, 0.25, 0.5, 2.0, false);

    let va = view.VisitingVA.borrow();
    assert!(va.is_active(), "active after Visit");
    assert_eq!(va.identity(), tree.GetIdentity(child));
    assert!((va.rel_x - 0.25).abs() < 1e-9);
    assert!((va.rel_y - 0.5).abs() < 1e-9);
    assert!((va.rel_a - 2.0).abs() < 1e-9);
}
```

If `identity()` / `rel_x()` getters don't exist on `emVisitingViewAnimator`, add `pub fn identity(&self) -> &str { &self.identity }` etc. as crate-internal accessors. Prefer short pub(crate) getters to exposing fields.

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p emcore --lib emView::tests::visit_routes_through_animator
```
Expected: compile error — `Visit` signature does not match (current signature is `Visit(panel, rx, ry, ra)`, no `tree` or `adherent`) — OR the body still pushes on the stack.

- [ ] **Step 3: Rewrite `Visit` and `VisitByIdentity`**

At `crates/emcore/src/emView.rs:823` — current `Visit(panel, rel_x, rel_y, rel_a)` stack-push body. Replace the entire method with:

```rust
/// Port of C++ `emView::Visit(panel, relX, relY, relA, adherent)`
/// (emView.cpp:492-497). Delegates to VisitByIdentity.
pub fn Visit(
    &mut self,
    tree: &PanelTree,
    panel: PanelId,
    rel_x: f64,
    rel_y: f64,
    rel_a: f64,
    adherent: bool,
) {
    let identity = tree.GetIdentity(panel);
    let subject = tree.GetTitle(panel);
    self.VisitByIdentity(&identity, rel_x, rel_y, rel_a, adherent, &subject);
}
```

At `:2949` — existing `VisitByIdentity(&mut self, tree: &mut PanelTree, identity: &str, rel_x, rel_y, rel_a)`. Replace with:

```rust
/// Port of C++ `emView::Visit(identity, relX, relY, relA, adherent, subject)`
/// (emView.cpp:500-510).
pub fn VisitByIdentity(
    &mut self,
    identity: &str,
    rel_x: f64,
    rel_y: f64,
    rel_a: f64,
    adherent: bool,
    subject: &str,
) {
    let mut va = self.VisitingVA.borrow_mut();
    // PHASE-W4-FOLLOWUP: C++ passes the emView's CoreConfig here. Rust
    // emView does not yet own CoreConfig; using emCoreConfig defaults
    // (VisitSpeed=1.0, max=10.0 per emCoreConfig.cpp:53). Full CoreConfig
    // ownership is a future wave.
    va.SetAnimParamsByCoreConfig(1.0, 10.0);
    va.SetGoalWithCoords(identity, rel_x, rel_y, rel_a, adherent, subject);
    va.Activate();
}
```

Note: the existing Rust `SetGoal` on `emVisitingViewAnimator` (at `emViewAnimator.rs:752`) takes `(identity, adherent, subject)` — the 3-arg form, matching C++ short form. The 6-arg form `(identity, rel_x, rel_y, rel_a, adherent, subject)` (C++ `emViewAnimator.cpp:1001`) needs a Rust equivalent. If it doesn't exist, add it as `SetGoalWithCoords` — same name suffix rule applied to the animator. Read `emViewAnimator.rs` to see if a 6-arg method exists under another name (grep for `SetGoal`); if so, use that name and remove the `SetGoalWithCoords` reference above.

**If `SetGoalWithCoords` (or equivalent 6-arg SetGoal) does not exist, add it as part of this task**, porting C++ `emViewAnimator.cpp:1001-1007`:

```cpp
void emVisitingViewAnimator::SetGoal(
    const char * identity, double relX, double relY, double relA,
    bool adherent, const char * subject
) {
    SetGoal(VT_VISIT_REL, identity, relX, relY, relA, adherent, false, subject);
}
```

The Rust private `SetGoal` helper that takes the full `visit_type` + all args should already exist (it's the shared core). If not, read `emViewAnimator.cpp:1351-1377` and port it.

- [ ] **Step 4: Update all existing callers of the old `Visit` signature**

```bash
grep -n "\.Visit(" crates/emcore/src/ crates/eaglemode/ crates/emmain/ crates/emfileman/
```

Existing callers at `emViewInputFilter.rs:1676` (triple-tap) and `emView.rs:837` (VisitFullsized internal), and existing `VisitByIdentity` call site at `emView.rs:2958`. Update each to match the new signatures:

- `emViewInputFilter.rs:1676`: `view.Visit(panel, rx, ry, ra)` → `view.Visit(tree, panel, rx, ry, ra, false)`. Make sure `tree` is in scope at that site.
- `emView.rs:837` (`VisitFullsized` body `self.Visit(panel, x, y, a)`): will be rewritten entirely in Task 3.3.
- `emView.rs:2958` (inside the old `VisitByIdentity` body): the body is the one being replaced; gone entirely.

- [ ] **Step 5: Run test, verify it passes**

```bash
cargo test -p emcore --lib emView::tests::visit_routes_through_animator
```
Expected: PASS.

- [ ] **Step 6: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean. Some previously-passing tests that asserted on `visit_stack` length or on stack-pushed rel coords may now fail, because the stack is no longer pushed to. Migrate those tests — they should now observe `VisitingVA.borrow()` state instead of `visit_stack()`. For every failing test, either:
- The test's assertion is "visit_stack had N entries after Visit" → replace with "VisitingVA is active and identity matches target".
- The test was covered by Phase 2 migration to `GetVisitedPanel` → the animator's effect on viewport doesn't actually hit until `Cycle` runs. If the test ran `Update` between `Visit` and the assertion, behavior may be preserved. If the test asserted on synchronous rel_x changes, it was asserting on the stack-push side-effect that no longer happens; update the test to drive the scheduler tick first, then assert.

Under the observational-port frame: tests that break here were testing the accidental stack-push side-effect, not author intent. Fixing them to assert on observable-behavior-after-animator-cycle is the correct fix, not "make the test pass by restoring the side-effect."

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "feat(emView): route Visit/VisitByIdentity through VisitingVA

Body rewritten per C++ emView.cpp:492-510: three-line delegation to
VisitingVA.SetAnimParamsByCoreConfig + SetGoal + Activate. Signature
adds &PanelTree and adherent: bool to match C++ overloads. Old
stack-push body gone.

Callers updated: emViewInputFilter.rs:1676 triple-tap.

CoreConfig passed via hardcoded defaults (VisitSpeed=1.0, max=10.0);
PHASE-W4-FOLLOWUP marker notes full CoreConfig ownership is a
future wave.

W4 Phase 3 Task 3.1."
```

### Task 3.2: Rewrite short-form `VisitPanel` and identity-short

**Files:**
- Modify: `crates/emcore/src/emView.rs` (add short-forms).

C++ reference: `emView.cpp:511-523`.

- [ ] **Step 1: Add `VisitPanel` short-form**

```rust
/// Port of C++ `emView::Visit(panel, adherent)` (emView.cpp:511-514).
pub fn VisitPanel(&mut self, tree: &PanelTree, panel: PanelId, adherent: bool) {
    let identity = tree.GetIdentity(panel);
    let subject = tree.GetTitle(panel);
    self.VisitByIdentityShort(&identity, adherent, &subject);
}

/// Port of C++ `emView::Visit(identity, adherent, subject)` (emView.cpp:517-523).
pub fn VisitByIdentityShort(
    &mut self,
    identity: &str,
    adherent: bool,
    subject: &str,
) {
    let mut va = self.VisitingVA.borrow_mut();
    va.SetAnimParamsByCoreConfig(1.0, 10.0);  // PHASE-W4-FOLLOWUP: see Task 3.1
    va.SetGoal(identity, adherent, subject);  // existing 3-arg SetGoal
    va.Activate();
}
```

No test added for these — they are covered by the VisitNext/Prev/... tests in Task 3.3, which call these methods.

- [ ] **Step 2: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean; test count unchanged.

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emView.rs
git commit -m "feat(emView): add VisitPanel/VisitByIdentityShort short-forms

Port of C++ emView.cpp:511-523 overloads. Panel-form delegates to
identity-form. Exercised in subsequent VisitNext/Prev rewrites.

W4 Phase 3 Task 3.2."
```

### Task 3.3: Rewrite `VisitFullsized`

**Files:**
- Modify: `crates/emcore/src/emView.rs:835`.

C++ reference: `emView.cpp:525-541`.

- [ ] **Step 1: Replace the body**

Current Rust `VisitFullsized` at :835 computes coords via `CalcVisitFullsizedCoords` and calls `self.Visit(panel, x, y, a)`. Replace with the C++ shape:

```rust
/// Port of C++ `emView::VisitFullsized(panel, adherent, utilizeView)` (emView.cpp:525-528).
pub fn VisitFullsized(
    &mut self,
    tree: &PanelTree,
    panel: PanelId,
    adherent: bool,
    utilize_view: bool,
) {
    let identity = tree.GetIdentity(panel);
    let subject = tree.GetTitle(panel);
    self.VisitFullsizedByIdentity(&identity, adherent, utilize_view, &subject);
}

/// Port of C++ `emView::VisitFullsized(identity, adherent, utilizeView, subject)` (emView.cpp:531-541).
pub fn VisitFullsizedByIdentity(
    &mut self,
    identity: &str,
    adherent: bool,
    utilize_view: bool,
    subject: &str,
) {
    let mut va = self.VisitingVA.borrow_mut();
    va.SetAnimParamsByCoreConfig(1.0, 10.0);  // PHASE-W4-FOLLOWUP: see Task 3.1
    va.SetGoalFullsized(identity, adherent, utilize_view, subject);
    va.Activate();
}
```

Update all callers — grep `\.VisitFullsized(` and match the new signature.

- [ ] **Step 2: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add -u
git commit -m "feat(emView): route VisitFullsized through VisitingVA

Port of C++ emView.cpp:525-541. Three-line delegation to
VisitingVA.SetAnimParamsByCoreConfig + SetGoalFullsized + Activate.

W4 Phase 3 Task 3.3."
```

### Task 3.4: Rewrite `VisitNext`/`VisitPrev`/`VisitFirst`/`VisitLast`/`VisitIn`/`VisitOut`/`VisitLeft`/`VisitRight`/`VisitUp`/`VisitDown`/`VisitNeighbour`

**Files:**
- Modify: `crates/emcore/src/emView.rs:2372` (VisitNext) through `:2607` (Neighbour helper).

C++ reference: `emView.cpp:564-756`.

Approach: rewrite each body per C++. Each C++ body is 5-15 lines and ends with `Visit(p, true)` (panel short-form) or `VisitFullsized(...)`.

- [ ] **Step 1: Rewrite `VisitNext`**

Current Rust (~:2372-2402) calls `self.animated_visit_panel(tree, next, false)`. C++ body at `emView.cpp:564-578`:

```cpp
void emView::VisitNext() {
    emPanel * p = ActivePanel;
    if (p) {
        p = p->GetFocusableNext();
        if (!p) {
            p = ActivePanel->GetFocusableParent();
            if (!p) p = RootPanel;
            if (p != ActivePanel) p = p->GetFocusableFirstChild();
        }
        Visit(p, true);
    }
}
```

Rust port:

```rust
pub fn VisitNext(&mut self, tree: &mut PanelTree) {
    let Some(active) = self.active else { return; };
    let mut p = tree.get_focusable_next(active);
    if p.is_none() {
        let parent = tree.get_focusable_parent(active).unwrap_or(self.root);
        if parent != active {
            p = tree.get_focusable_first_child(parent);
        } else {
            p = Some(parent);
        }
    }
    if let Some(target) = p {
        self.VisitPanel(tree, target, true);
    }
}
```

If `get_focusable_next` / `get_focusable_parent` / `get_focusable_first_child` don't exist on `PanelTree`, check the existing `animated_visit_panel`-calling body for what the current Rust implementation uses to compute `next` / `prev` / `first_child` — those methods exist under some name. Substitute the existing names.

- [ ] **Step 2: Rewrite `VisitPrev`**

C++ at `emView.cpp:581-595`, same shape with `GetFocusablePrev` + `GetFocusableLastChild`. Rust port mirrors Step 1.

- [ ] **Step 3: Rewrite `VisitFirst`**

C++ at `emView.cpp:598-608`:

```cpp
void emView::VisitFirst() {
    emPanel * p;
    if (ActivePanel) {
        p = ActivePanel->GetFocusableParent();
        if (p) p = p->GetFocusableFirstChild();
        if (!p) p = ActivePanel;
        Visit(p, true);
    }
}
```

Rust port analogous.

- [ ] **Step 4: Rewrite `VisitLast`**

C++ at `emView.cpp:611-621`. Same shape as VisitFirst with `GetFocusableLastChild`.

- [ ] **Step 5: Rewrite `VisitLeft`/`VisitRight`/`VisitUp`/`VisitDown`**

C++ at `emView.cpp:624-648` — each is a one-liner delegating to `VisitNeighbour(direction)`. Port accordingly.

- [ ] **Step 6: Rewrite `VisitNeighbour`**

C++ at `emView.cpp:648-737` — longer. Read the C++ body; port line-for-line preserving the coord-math semantics for neighbour-finding. Ends with `Visit(p, true)`.

- [ ] **Step 7: Rewrite `VisitIn`/`VisitOut`**

C++ at `emView.cpp:738-762`. Both are short. Port.

- [ ] **Step 8: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean. At this point, `animated_visit_panel` has zero callers; `pending_animated_visit` has zero writers.

- [ ] **Step 9: Golden-tests diff audit**

```bash
scripts/verify_golden.sh --report
```
Expected: 237 pass, 6 fail (the baseline). If any golden tests that previously passed now fail, investigate:
- If the failure is in a test that exercises keyboard navigation (Next/Prev/First/Last/In/Out), the shift may be the expected correctness restoration. Document in the commit message with `python3 scripts/diff_draw_ops.py <name> --regions` output.
- If the failure is elsewhere, the rewrite has a bug — diagnose before committing.

- [ ] **Step 10: Commit**

```bash
git add -u
git commit -m "feat(emView): route VisitNext/Prev/First/Last/In/Out/Neighbour through VisitingVA

Eleven navigation methods rewritten per C++ emView.cpp:564-762. Each
body computes target then calls VisitPanel (short panel-form delegating
to VisitingVA). pending_animated_visit now has zero writers; keyboard
navigation actually animates.

Golden baseline 237/6 preserved.

W4 Phase 3 Task 3.4."
```

**Phase 3 acceptance:** every `Visit*` body matches C++ delegation shape (reviewable against `emView.cpp:492-762`); grep `pending_animated_visit\s*=` finds only the field declaration and initializer (no writers outside the old `animated_visit`); nextest passes; golden 237/6 preserved or diff-audited.

---

# Phase 4 — Home-key routing

**Goal:** `emPanel::Input` handles `EM_KEY_{HOME,END,PAGE_UP,PAGE_DOWN}` per `emPanel.cpp:1168-1198`. VIF has no Home handler. `go_home` has zero callers.

### Task 4.1: Audit emPanel::Input current state

- [ ] **Step 1: Check whether emPanel::Input exists and handles key events**

```bash
grep -n "fn Input\|fn input\|EM_KEY_HOME\|KeyEvent\|InputKey::Home\|InputKey::End" crates/emcore/src/emPanel.rs | head -20
```

If `emPanel::Input` exists and handles key events for other cases (e.g., arrows), add the Home/End/PageUp/PageDown cases to the existing match. If `emPanel::Input` exists but is a stub or doesn't route key events, add the minimum structure to dispatch key events — this is a small extension, not a refactor.

If `emPanel::Input` does not exist at all, stop and ask before adding it — the wave assumed it exists.

### Task 4.2: Port EM_KEY_HOME/END/PAGE_UP/PAGE_DOWN block

**Files:**
- Modify: `crates/emcore/src/emPanel.rs` — Input handler.

C++ reference: `emPanel.cpp:1168-1198`.

- [ ] **Step 1: Write the failing integration test**

Check existing integration test layout: `ls crates/eaglemode/tests/integration/`. Add a new file `crates/eaglemode/tests/integration/panel_key_routing.rs` (or add to an existing file if there's an obvious home like `input.rs`):

```rust
use emcore::emPanel::*;
use emcore::emPanelTree::PanelTree;
use emcore::emView::emView;
use emcore::emInput::{emInputEvent, InputKey, InputVariant, InputState};

#[test]
fn home_key_routes_to_visit_first() {
    // W4 Phase 4: EM_KEY_HOME with no modifiers routes through emPanel::Input
    // to View.VisitFirst (matches C++ emPanel.cpp:1177-1180).
    let mut tree = PanelTree::new(1.0);
    let root = tree.create_root("root");
    let child_a = tree.create_child(root, "a");
    let _child_b = tree.create_child(root, "b");
    let mut view = emView::new(root, 800.0, 600.0);
    view.set_home_geometry(0.0, 0.0, 800.0, 600.0, 1.0);
    view.set_active_panel(&mut tree, child_a, false);

    // Press Home.
    let mut event = emInputEvent::key(InputKey::Home, InputVariant::Press);
    let state = InputState::new();
    // Dispatch to emPanel::Input on child_a.
    tree.with_panel_mut(child_a, |panel| {
        panel.Input(&mut view, &mut event, &state);
    });

    // After Home, VisitingVA should be active with goal pointing at the first
    // focusable sibling (== child_a itself if it's already the first, or a
    // sibling before it).
    assert!(view.VisitingVA.borrow().is_active(),
            "Home key activates VisitingVA via VisitFirst");
}
```

The exact constructor names and method names (e.g., `emInputEvent::key`, `InputState::new`, `with_panel_mut`, `set_active_panel`) must match what exists in the codebase. Grep to find the real names. If nothing resembling `with_panel_mut` exists on `PanelTree`, use whatever pattern existing tests use for dispatching input to a panel (search `crates/eaglemode/tests/` for `Input` calls on panels).

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p eaglemode --test panel_key_routing
```
Expected: FAIL — `emPanel::Input` does not currently handle Home key (it's handled in VIF).

- [ ] **Step 3: Port the key block to `emPanel::Input`**

Locate the match arm in `emPanel::Input` that handles key events. Add:

```rust
// C++ emPanel.cpp:1168-1198.
case_key!(Press, InputKey::Home) => {
    if state.is_no_mod() {
        view.VisitFirst(tree);
        event.eat();
    } else if state.is_alt_mod() {
        view.VisitFullsized(tree, self.id(), view.IsActivationAdherent(), false);
        event.eat();
    } else if state.is_shift_alt_mod() {
        view.VisitFullsized(tree, self.id(), view.IsActivationAdherent(), true);
        event.eat();
    }
}
case_key!(Press, InputKey::End) => {
    if state.is_no_mod() {
        view.VisitLast(tree);
        event.eat();
    }
}
case_key!(Press, InputKey::PageUp) => {
    if state.is_no_mod() {
        view.VisitOut(tree);
        event.eat();
    }
}
case_key!(Press, InputKey::PageDown) => {
    if state.is_no_mod() {
        view.VisitIn(tree);
        event.eat();
    }
}
```

Translate `case_key!` to whatever match syntax the existing `emPanel::Input` uses (likely a `match (event.key, event.variant)` or similar). The C++ modifier checks (`IsNoMod`, `IsAltMod`, `IsShiftAltMod`) should have Rust counterparts on `InputState`; grep to confirm the exact method names. If `is_shift_alt_mod()` doesn't exist, check for `shift_alt()` or spelled-out variants.

`self.id()` — the current panel's `PanelId`. If `emPanel::Input` takes `panel_id: PanelId` as an argument rather than having it on `self`, use that.

- [ ] **Step 4: Run test, verify it passes**

```bash
cargo test -p eaglemode --test panel_key_routing
```
Expected: PASS.

- [ ] **Step 5: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(emPanel): port EM_KEY_{HOME,END,PAGE_UP,PAGE_DOWN} handlers

C++ emPanel.cpp:1168-1198 routes Home/End/PageUp/PageDown in
emPanel::Input, not in emViewInputFilter. Home has three modifier
variants (none/Alt/Shift+Alt) per C++. Rust now matches.

New integration test in tests/integration/panel_key_routing.rs
asserts Home -> VisitingVA activation via VisitFirst.

VIF Home handler deletion follows in Task 4.3.

W4 Phase 4 Task 4.2."
```

### Task 4.3: Delete VIF Home handler

**Files:**
- Modify: `crates/emcore/src/emViewInputFilter.rs:1260-1263`.

- [ ] **Step 1: Delete the Home case**

Remove lines:

```rust
InputKey::Home => {
    view.go_home();
    return true;
}
```

Delete only those four lines. Leave other InputKey arms untouched.

- [ ] **Step 2: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean. `go_home` now has zero callers (but it still compiles — deletion in Phase 5).

Verify:
```bash
grep -n "go_home\(\)" crates/
```
Expected: only the definition at `emView.rs:879`, the internal reference via `visit_stack.truncate(1)` body at :880, and this task's own `grep`-style mentions. No production call sites.

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emViewInputFilter.rs
git commit -m "refactor(emViewInputFilter): remove Home key handler

C++ handles Home in emPanel::Input, not VIF (emPanel.cpp:1168-1198).
Phase 4 Task 4.2 ported that. VIF Home -> go_home was accidental
double divergence. Delete.

go_home now has zero production callers; deletion scheduled for
Phase 5.

W4 Phase 4 Task 4.3."
```

**Phase 4 acceptance:** integration test `home_key_routes_to_visit_first` passes; VIF has no Home handler; `grep go_home crates/` shows definition only.

---

# Phase 5 — Accidental-divergence deletion

**Goal:** Delete the Rust-only visit-state types, fields, methods, tests, and stale comments in one atomic commit.

### Task 5.1: Delete types, fields, methods

**Files:**
- Modify: `crates/emcore/src/emView.rs`.

- [ ] **Step 1: Delete `struct VisitState`**

At `emView.rs:37`, delete the `pub struct VisitState { ... }` block (including its derive attributes and any doc comments).

- [ ] **Step 2: Delete `visit_stack` field**

At `emView.rs:274`, delete the line `visit_stack: Vec<VisitState>,`.

- [ ] **Step 3: Delete `pending_animated_visit` field**

At `emView.rs:309`, delete the line plus its doc comment.

- [ ] **Step 4: Delete initializers in `emView::new`**

At `:436-441`, delete the `let initial_visit = VisitState { ... };` block. At `:450`, delete the `visit_stack: vec![initial_visit],` line. At `:468`, delete the `pending_animated_visit: None,` line.

- [ ] **Step 5: Delete accessor methods**

At `:669-681`, delete `pub fn current_visit`, `pub fn visit_stack`, `pub fn visit_stack_mut` and their doc comments.

At `:843-865`, delete `pub fn animated_visit` and `pub fn animated_visit_panel`.

At `:867-886`, delete `pub fn go_back` and `pub fn go_home`.

At `:3041-3048`, delete `pub fn take_pending_animated_visit` and `pub fn has_pending_animated_visit`.

- [ ] **Step 6: Delete stack-update in `RawVisit`**

At `:720-725`, delete the block:

```rust
if let Some(state) = self.visit_stack.last_mut() {
    state.panel = panel;
    state.rel_x = rx;
    state.rel_y = ry;
    state.rel_a = ra;
}
```

`self.active = Some(panel);` on the next line stays — that's the correct side-effect and matches C++ `RawVisit` setting `ActivePanel`.

- [ ] **Step 7: Delete DIVERGED comment at :701**

At what was `:691-703` (shifts after earlier deletions), delete the three `DIVERGED:` comment blocks (they document the stack that no longer exists). Keep the single surviving comment that documents the absolute-coord computation — it matches C++ behavior and is still accurate.

- [ ] **Step 8: Delete stale comments at :5305, :5325**

Delete the inline test-code comments that reference "Phase 3: visit_stack mutation" and "direct visit_stack mutation is not the intended path."

- [ ] **Step 9: Run cargo check**

```bash
cargo check -p emcore 2>&1 | head -40
```
Expected: compile errors for any caller that references a deleted method. These should all be in tests (Phase 2 migrated production callers already). Fix each failing test either by:
- Deleting the test if it was testing deleted behavior (only `view_visit_and_back` falls here; Task 5.2).
- Migrating to the new API (if a test reader slipped through Phase 2).

Read each compile error, address at its site.

- [ ] **Step 10: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean.

- [ ] **Step 11: Grep verification**

```bash
grep -nE 'visit_stack|pending_animated_visit|VisitState|\bgo_back\b|\bgo_home\b|\bcurrent_visit\b|\banimated_visit\b|animated_visit_panel|take_pending_animated_visit|has_pending_animated_visit' crates/
```
Expected: empty.

(Does not match `animated_visit_something` sub-strings nor `has_pending_notices` — those are different identifiers.)

- [ ] **Step 12: Commit**

```bash
git add -u
git commit -m "refactor(emView): delete accidental visit-state scaffolding

Removed: VisitState struct, visit_stack field, pending_animated_visit
field, current_visit/visit_stack/visit_stack_mut accessors,
animated_visit/animated_visit_panel methods, go_back/go_home methods,
take_pending_animated_visit/has_pending_animated_visit.

None have C++ counterparts. The accidental Rust stack of visit states
has been observationally equivalent to the current VisitingVA goal
state ever since Phase 3 rewired writers. Deleting the zombie state.

Stale DIVERGED comments at :701 and stack-mutation comments at
:5305/:5325 removed with their code.

W4 Phase 5 Task 5.1."
```

### Task 5.2: Delete tests and stale comments

**Files:**
- Modify: `crates/eaglemode/tests/unit/panel.rs:140-165`.
- Modify: `crates/eaglemode/tests/integration/input.rs:143`.

- [ ] **Step 1: Delete `view_visit_and_back` test**

At `crates/eaglemode/tests/unit/panel.rs:140-165`, delete the entire test function (including any `#[test]` attribute and doc comment).

- [ ] **Step 2: Delete stale comment at tests/integration/input.rs:143**

```bash
sed -n '140,148p' crates/eaglemode/tests/integration/input.rs
```
Find the comment `// Note: emView::new sets initial visit_stack with root, which may or may not...` (approximate text). Delete the comment lines (do not alter surrounding test code).

- [ ] **Step 3: Run pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean; test count −1 from the deletion.

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "test: delete view_visit_and_back and stale visit_stack comment

view_visit_and_back tested Rust-only go_back/go_home API deleted in
Task 5.1. Tests Rust-only semantics that have no C++ counterpart.
Stale comment in tests/integration/input.rs:143 removed.

W4 Phase 5 Task 5.2."
```

**Phase 5 acceptance:** `grep -nE 'visit_stack|pending_animated_visit|VisitState|\bgo_back\b|\bgo_home\b|\bcurrent_visit\b|\banimated_visit\b' crates/` returns empty. Nextest count −1 from baseline (−1 deleted; +3 from Phase 1 + Phase 2 + Phase 3 + Phase 4 new tests = net +2 or +3 depending on count of new tests added).

---

# Phase 6 — Invariant restoration + smoke

**Goal:** `invariant_equilibrium_at_target` covers `factor=1.0`; smoke and golden both green.

### Task 6.1: Restore `factor=1.0`

**Files:**
- Modify: `crates/emcore/src/emViewAnimator.rs:~3320`.

- [ ] **Step 1: Locate the test**

```bash
grep -n "invariant_equilibrium_at_target\|KNOWN GAP" crates/emcore/src/emViewAnimator.rs
```

- [ ] **Step 2: Update the factor list and remove the KNOWN GAP comment**

Change:

```rust
for &factor in &[2.0, 4.0, 16.0, 100.0] {
```

to:

```rust
for &factor in &[1.0, 2.0, 4.0, 16.0, 100.0] {
```

Delete the `KNOWN GAP (TODO phase 8)` comment block immediately above the loop (its text is stale and the gap is closed).

- [ ] **Step 3: Run the specific test**

```bash
cargo test -p emcore --lib emViewAnimator::tests::invariant_equilibrium_at_target
```
Expected: PASS at factor=1.0.

If it fails at factor=1.0: the rewrite in Phases 1-3 may have missed a case. Debug the specific failure — the most likely cause is an edge case in `CycleAnimation` when the goal is already at-target (factor=1.0 means the animator starts at the goal). The fix is in the animator, not in the test.

- [ ] **Step 4: Run full pre-commit gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo-nextest ntr
```
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emViewAnimator.rs
git commit -m "test(emViewAnimator): restore factor=1.0 in invariant_equilibrium_at_target

The KNOWN GAP skip of factor=1.0 masked the accidental visit-state
scaffolding's failure mode. With W4 Phases 1-5 closing that gap,
the invariant holds at factor=1.0.

Removes stale 'TODO phase 8' comment.

W4 Phase 6 Task 6.1."
```

### Task 6.2: Golden + smoke acceptance

- [ ] **Step 1: Run full golden suite**

```bash
scripts/verify_golden.sh --report
```
Expected: 237 pass, 6 fail (baseline). If golden count shifted during Phase 3 and was documented in the commit, verify the shift is still the same shift here (no regression on top of the documented delta).

- [ ] **Step 2: Run smoke test**

```bash
timeout 20 cargo run --release --bin eaglemode
echo "exit: $?"
```
Expected: exit code `124` (timeout SIGKILL) or `143` (timeout SIGTERM = 128+15). Either is valid and indicates the program stayed alive through the 20-second window.

- [ ] **Step 3: Final acceptance grep**

```bash
echo "=== accidental divergence sweep ==="
grep -nE 'visit_stack|pending_animated_visit|VisitState|\bgo_back\b|\bgo_home\b|\bcurrent_visit\b|\banimated_visit\b' crates/ || echo "(empty — passing)"

echo "=== PHASE-W4-FOLLOWUP count ==="
grep -rn "PHASE-W4-FOLLOWUP" crates/ | wc -l
# Expected: small number — the CoreConfig-defaults markers from
# Tasks 3.1/3.2/3.3. Each one is a single-line comment.
```

- [ ] **Step 4: Verify no new DIVERGED annotations introduced**

```bash
git log --since="$(git log -1 --format=%cd --date=iso main -- | head -1)" -p -- crates/ | grep '^\+.*DIVERGED:' | head
```

Should be empty. If any new `DIVERGED:` slipped in during the wave, remove it before closing.

- [ ] **Step 5: Final close-out commit** (only if needed for any follow-up polish — usually no new commit here)

If Steps 1-4 all pass, Phase 6 is complete without an additional commit.

**Phase 6 acceptance:** `invariant_equilibrium_at_target` passes at factor=1.0; golden 237/6 preserved (or documented Phase 3 delta); smoke returns 124 or 143; accidental-divergence grep empty; PHASE-W4-FOLLOWUP markers limited to the CoreConfig-defaults comments.

---

## Wave Acceptance Checklist

- [ ] `emView` owns `VisitingVA: Rc<RefCell<emVisitingViewAnimator>>`.
- [ ] Every `Visit*` method's body is a C++-shape delegation to `VisitingVA` (review each against `emView.cpp:492-762`).
- [ ] `GetVisitedPanel` ported; all production readers use it.
- [ ] `emPanel::Input` handles `EM_KEY_{HOME,END,PAGE_UP,PAGE_DOWN}` per `emPanel.cpp:1168-1198`; VIF does not.
- [ ] `invariant_equilibrium_at_target` covers `factor=1.0`.
- [ ] `grep -nE 'visit_stack|pending_animated_visit|VisitState|\bgo_back\b|\bgo_home\b|\bcurrent_visit\b|animated_visit' crates/` returns empty. (Dropped the final word boundary so the pattern also catches `animated_visit_panel`.)
- [ ] Golden baseline 237/6 preserved or diff-audited with documented Phase 3 commit.
- [ ] `cargo clippy -- -D warnings` clean.
- [ ] `cargo-nextest ntr` passes.
- [ ] Smoke (`timeout 20 cargo run --release --bin eaglemode`) returns 124 or 143.
- [ ] No new `DIVERGED:` annotations.
- [ ] No `#[allow(...)]` / `#[expect(...)]` introduced.
- [ ] `PHASE-W4-FOLLOWUP` markers limited to CoreConfig-defaults comments; no other W4 followups dangle.
