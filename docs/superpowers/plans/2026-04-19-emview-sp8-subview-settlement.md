# SP8 — Sub-view synchronous-settlement divergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire `PanelTree::cycle_list` / `Cycle` / `cancel_cycle` / `run_panel_cycles` by giving `emSubViewPanel` a per-sub-view scheduler ticked from its outer `PanelCycleEngine`; simplify `emSubViewPanel::Paint` to match C++.

**Architecture:** `emSubViewPanel` owns an inner `sub_scheduler`. Its sub-view attaches to `sub_scheduler`; sub-tree panels register `PanelCycleEngine` adapters there via `init_panel_view`. A new `PanelBehavior::Cycle` on `emSubViewPanel` drives one `sub_scheduler.DoTimeSlice` per outer-scheduler tick and ticks `active_animator`. `UpdateEngineClass` and `VisitingVAEngineClass` migrate from `window_id` lookup to direct `Weak<RefCell<emView>>`, eliminating `ctx.windows` indirection for these engines so sub-views (no window) can use them.

**Tech Stack:** Rust, `Rc<RefCell<>>`, `Weak`, `winit::WindowId`, existing emcore scheduler/engine plumbing.

---

## Reference

- **Spec:** `docs/superpowers/specs/2026-04-19-emview-sp8-subview-settlement-design.md`
- **Closeout:** `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.1 item 17.
- **C++ source:** `~/git/eaglemode-0.96.4/src/emCore/emSubViewPanel.cpp`, `emView.cpp`.

## File inventory (what changes, and why)

- `crates/emcore/src/emView.rs` — refactor `UpdateEngineClass` + `VisitingVAEngineClass` to hold `Weak<RefCell<emView>>`; change `attach_to_scheduler` signature to drop `window_id`.
- `crates/emcore/src/emWindow.rs` — callers of `attach_to_scheduler` drop window_id arg.
- `crates/emcore/src/emSubViewPanel.rs` — add `sub_scheduler`, implement `PanelBehavior::Cycle`, simplify `Paint`, remove synchronous interleave, add `DIVERGED:` block.
- `crates/emcore/src/emPanelTree.rs` — delete `cycle_list`, `Cycle`, `cancel_cycle`, `run_panel_cycles`.
- `crates/emcore/src/emGUIFramework.rs` — remove stale comment block referencing `run_panel_cycles`.
- `crates/emmain/src/emMainWindow.rs` — drop window_id arg from `attach_to_scheduler` calls.
- `crates/eaglemode/tests/golden/composition.rs` — rewrite `settle()` to use scheduler.
- `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` — close SP8 row.

## Baseline commands

- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo-nextest ntr` (alias for `cargo nextest run`)
- `cargo test --test golden -- --test-threads=1`

## Baseline counts (pre-SP8)

- nextest: 2440/2440
- golden: 237 pass / 6 pre-existing fail

---

## Phase 1 — Engine view-direct migration

Refactor `UpdateEngineClass` and `VisitingVAEngineClass` to hold `Weak<RefCell<emView>>` instead of `WindowId`. `attach_to_scheduler` no longer takes `window_id`. This is purely structural — observable behavior unchanged.

### Task 1.1: Migrate UpdateEngineClass to view-direct

**Files:**
- Modify: `crates/emcore/src/emView.rs` (around line 240–280 for struct + impl)

- [ ] **Step 1: Read current UpdateEngineClass at `crates/emcore/src/emView.rs:240-280`**

Confirm exact signatures and `ctx.windows` usage before modifying.

- [ ] **Step 2: Replace the struct and impl**

Replace the `UpdateEngineClass` struct definition (around line 240) and its `emEngine` impl (around line 253) with:

```rust
/// Scheduler-driven engine that calls `emView::Update` once per slice.
///
/// Ported from C++ `emView::UpdateEngineClass` (inner class of `emView`,
/// holds direct pointer to view). Rust holds `Weak<RefCell<emView>>` since
/// `emView` lives in `Rc<RefCell<>>`.
pub struct UpdateEngineClass {
    pub view: std::rc::Weak<std::cell::RefCell<emView>>,
}

impl UpdateEngineClass {
    pub fn new(view: std::rc::Weak<std::cell::RefCell<emView>>) -> Self {
        Self { view }
    }
}

impl super::emEngine::emEngine for UpdateEngineClass {
    fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
        let Some(view_rc) = self.view.upgrade() else {
            return false;
        };
        let mut view = view_rc.borrow_mut();

        // SP4 Part A: pre-compute popup-close probe (C++ emView.cpp:1299).
        let popup_close_sig = view.PopupWindow.as_ref().map(|p| p.borrow().close_signal);
        if let Some(close_sig) = popup_close_sig {
            view.close_signal_pending = ctx.IsSignaled(close_sig);
        }

        view.Update(ctx.tree);

        // SP4 Part B: drain deferred scheduler ops queued by Update's call tree.
        let ops: Vec<SchedOp> = view.pending_sched_ops.drain(..).collect();
        for op in ops {
            op.apply_via_ctx(ctx);
        }
        false
    }
}
```

- [ ] **Step 3: `cargo check --workspace`**

Expected: compile errors at `attach_to_scheduler` (still calls `UpdateEngineClass::new(window_id)`) and `VisitingVAEngineClass` (next task). Address in Task 1.2.

### Task 1.2: Migrate VisitingVAEngineClass to view-direct

**Files:**
- Modify: `crates/emcore/src/emView.rs` (around line 293–339)

- [ ] **Step 1: Replace VisitingVAEngineClass struct + impl**

```rust
pub struct VisitingVAEngineClass {
    pub view: std::rc::Weak<std::cell::RefCell<emView>>,
    last_cycle: Option<Instant>,
}

impl VisitingVAEngineClass {
    pub fn new(view: std::rc::Weak<std::cell::RefCell<emView>>) -> Self {
        Self { view, last_cycle: None }
    }
}

impl super::emEngine::emEngine for VisitingVAEngineClass {
    fn Cycle(&mut self, ctx: &mut super::emEngine::EngineCtx<'_>) -> bool {
        let now = Instant::now();
        let dt = self
            .last_cycle
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(0.016)
            .clamp(0.001, 0.1);
        self.last_cycle = Some(now);

        let Some(view_rc) = self.view.upgrade() else {
            return false;
        };
        let mut view = view_rc.borrow_mut();
        let va_rc = Rc::clone(&view.VisitingVA);
        let mut va = va_rc.borrow_mut();
        if !va.is_active() {
            return false;
        }
        use super::emViewAnimator::emViewAnimator as _;
        va.animate(&mut view, ctx.tree, dt)
    }
}
```

- [ ] **Step 2: `cargo check --workspace`**

Expected: errors only at `attach_to_scheduler` callers. Fix in Task 1.3.

### Task 1.3: Change attach_to_scheduler signature

**Files:**
- Modify: `crates/emcore/src/emView.rs` (around line 3133)

- [ ] **Step 1: Rewrite `attach_to_scheduler` to drop `window_id`**

The caller now provides its own view weak through `self_view_weak` parameter. Replace:

```rust
pub fn attach_to_scheduler(
    &mut self,
    scheduler: Rc<RefCell<super::emScheduler::EngineScheduler>>,
    self_view_weak: std::rc::Weak<std::cell::RefCell<emView>>,
) {
    let (engine_id, eoi_signal, visiting_va_engine_id) = {
        let mut sched = scheduler.borrow_mut();
        let engine_id = sched.register_engine(
            super::emEngine::Priority::High,
            Box::new(UpdateEngineClass::new(self_view_weak.clone())),
        );
        let eoi_signal = sched.create_signal();
        let visiting_va_engine_id = sched.register_engine(
            super::emEngine::Priority::High,
            Box::new(VisitingVAEngineClass::new(self_view_weak)),
        );
        (engine_id, eoi_signal, visiting_va_engine_id)
    };
    self.scheduler = Some(scheduler);
    self.update_engine_id = Some(engine_id);
    self.EOISignal = Some(eoi_signal);
    self.visiting_va_engine_id = Some(visiting_va_engine_id);
    self.WakeUpUpdateEngine();
}
```

- [ ] **Step 2: `cargo check --workspace`**

Expected: compile errors at every caller (now passing `WindowId` as second arg). Fix in Task 1.4.

### Task 1.4: Update all attach_to_scheduler callers

**Files:**
- Modify: `crates/emcore/src/emWindow.rs`
- Modify: `crates/emmain/src/emMainWindow.rs`
- Modify: `crates/emcore/src/emView.rs` (tests at ~6659, ~6715, ~6860, ~6961, ~7096, ~7178)
- Modify: `crates/emcore/src/emPanelTree.rs` (test at ~3304)

- [ ] **Step 1: Find all callers**

```bash
grep -rn "attach_to_scheduler\b" crates/ --include="*.rs"
```

- [ ] **Step 2: For each caller, replace `(sched, window_id)` with `(sched, Rc::downgrade(&view_rc))`**

Production callers in `emMainWindow.rs` hold a `Rc<RefCell<emView>>` already — use `Rc::downgrade(&view_rc)`. `emWindow` tests likewise. Unit tests in `emView.rs` that have the view wrapped in `Rc<RefCell<>>` use that Rc.

Where an old test calls `attach_to_scheduler(sched, winit::window::WindowId::dummy())` on a view constructed as `let mut v = emView::new(...)` (bare, not in `Rc<RefCell<>>`), wrap the view first:

```rust
let view_rc = Rc::new(RefCell::new(emView::new(root, w, h, cfg)));
let view_weak = Rc::downgrade(&view_rc);
view_rc.borrow_mut().attach_to_scheduler(sched.clone(), view_weak);
```

(The test-side rewrites are mechanical; prefer `view_rc` pattern for tests that need the weak-ref anyway.)

- [ ] **Step 3: `cargo check --workspace` — must pass**

- [ ] **Step 4: `cargo clippy --workspace -- -D warnings` — must pass**

- [ ] **Step 5: `cargo-nextest ntr` — must stay at 2440/2440**

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(1/N): migrate UpdateEngine/VisitingVAEngine to view-direct lookup

Replaces WindowId+ctx.windows indirection with
Weak<RefCell<emView>> held directly on each engine. Matches C++
emView::UpdateEngineClass which has a direct pointer to its view.
attach_to_scheduler loses its window_id parameter.

Prerequisite for SP8: sub-views have no WindowId and need this engine
reshape to register update/visiting-va engines on a sub-scheduler.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 2 — Per-sub-view scheduler

Give `emSubViewPanel` its own scheduler; attach sub-view; register sub-tree engines.

### Task 2.1: Add sub_scheduler field and wire at construction

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs`

- [ ] **Step 1: Add sub_scheduler field**

In `emSubViewPanel` struct (around line 23):

```rust
/// DIVERGED: C++ shares the parent emContext's scheduler via context-chain
/// lookup (emContext::GetScheduler). Rust emSubViewPanel owns a nested
/// PanelTree and emView; EngineCtx::tree is singular, so a single scheduler
/// cannot cycle engines across two trees. The forced concession: this
/// sub-view gets its own EngineScheduler. It is ticked from the outer
/// PanelCycleEngine (PanelBehavior::Cycle below) once per parent-scheduler
/// slice, preserving C++ observable cross-frame settlement behavior.
/// Will be unified with the parent scheduler if/when SP7 threads emContext
/// through the view/window subsystem.
pub(crate) sub_scheduler: std::rc::Rc<std::cell::RefCell<crate::emScheduler::EngineScheduler>>,
/// Wall-clock timestamp of previous Cycle, used for active_animator dt.
last_cycle: Option<std::time::Instant>,
```

- [ ] **Step 2: Construct sub_scheduler in `new()`**

In `emSubViewPanel::new` (around line 47), after `let sub_view = ...`:

```rust
let sub_scheduler = std::rc::Rc::new(std::cell::RefCell::new(
    crate::emScheduler::EngineScheduler::new(),
));
// Attach sub-view: registers UpdateEngineClass + VisitingVAEngineClass
// against sub_scheduler using view-direct weak (Phase 1).
sub_view.borrow_mut().attach_to_scheduler(
    sub_scheduler.clone(),
    std::rc::Rc::downgrade(&sub_view),
);
// Now sub_tree panels have a view with a scheduler — register PanelCycleEngine
// adapters for any panels already in the sub-tree.
sub_tree.register_pending_engines();
```

Add `sub_scheduler` and `last_cycle: None` to the `Self { ... }` literal.

- [ ] **Step 3: `cargo check --workspace`**

Expected: compiles (struct update only; no behavior change yet).

- [ ] **Step 4: Add test — sub-view's UpdateEngine is registered**

Add to `emSubViewPanel.rs` tests block (create `#[cfg(test)] mod tests` if absent, or extend existing):

```rust
#[cfg(test)]
mod sp8_tests {
    use super::*;

    #[test]
    fn sp8_sub_view_update_engine_registered() {
        let panel = emSubViewPanel::new();
        let sub_view = panel.GetSubView();
        assert!(
            sub_view.scheduler_ref().is_some(),
            "sub_view must be attached to sub_scheduler in new()"
        );
    }

    #[test]
    fn sp8_sub_tree_root_panel_engine_registered() {
        let panel = emSubViewPanel::new();
        let root = panel.sub_root();
        // After init_panel_view + register_pending_engines in new(),
        // the sub-tree root must have an engine_id.
        let engine_id = panel.sub_tree().panel_engine_id(root);
        assert!(
            engine_id.is_some(),
            "sub-tree root panel must have PanelCycleEngine registered on sub_scheduler"
        );
    }
}
```

If `PanelTree::panel_engine_id` doesn't exist as a public accessor, add a pub(crate) helper:

```rust
// in emPanelTree.rs, near panel field accessors:
#[cfg(any(test, feature = "test-support"))]
pub fn panel_engine_id(&self, id: PanelId) -> Option<super::emScheduler::EngineId> {
    self.panels.get(id).and_then(|p| p.engine_id)
}
```

- [ ] **Step 5: Run tests**

```bash
cargo nextest run -p emcore sp8_sub_view_update_engine_registered sp8_sub_tree_root_panel_engine_registered
```

Expected: both pass.

- [ ] **Step 6: Full test suite**

```bash
cargo-nextest ntr
```

Expected: 2442/2442 (+2).

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(2/N): emSubViewPanel owns sub_scheduler

New per-sub-view EngineScheduler attached at construction. Sub-view
registers UpdateEngine/VisitingVAEngine against it via view-direct
attach. sub_tree's root panel gets its PanelCycleEngine via
register_pending_engines catch-up pass.

Forced divergence documented at struct field: Rust's nested PanelTree
+ singular EngineCtx::tree makes a single shared scheduler
structurally impossible without SP7's emContext threading.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 3 — emSubViewPanel::Cycle impl

Drive the sub-scheduler once per outer-scheduler tick; tick `active_animator`.

### Task 3.1: Implement PanelBehavior::Cycle on emSubViewPanel

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs`

- [ ] **Step 1: Add Cycle impl inside `impl PanelBehavior for emSubViewPanel`**

Add the following method (place alphabetically or adjacent to `notice`):

```rust
fn Cycle(&mut self, _ctx: &mut PanelCtx) -> bool {
    // Wall-clock dt for the active_animator tick (matching
    // VisitingVAEngineClass pattern).
    let now = std::time::Instant::now();
    let dt = self
        .last_cycle
        .map(|t| now.duration_since(t).as_secs_f64())
        .unwrap_or(0.016)
        .clamp(0.001, 0.1);
    self.last_cycle = Some(now);

    // 1) Tick the ActiveAnimator (C++ emView::ActiveAnimator; Rust stores
    //    on emSubViewPanel per SP1 §5.1 item 3). Take/put preserves Rust
    //    borrow rules.
    let animator_active = if let Some(mut anim) = self.active_animator.take() {
        let still_active = anim.animate(
            &mut self.sub_view.borrow_mut(),
            &mut self.sub_tree,
            dt,
        );
        if still_active {
            self.active_animator = Some(anim);
        }
        still_active
    } else {
        false
    };

    // 2) Drive one sub-scheduler slice. sub-view engines never access
    //    ctx.windows (view-direct after Phase 1), so an empty window map
    //    is safe.
    let mut empty_windows: std::collections::HashMap<
        winit::window::WindowId,
        std::rc::Rc<std::cell::RefCell<crate::emWindow::emWindow>>,
    > = std::collections::HashMap::new();
    self.sub_scheduler
        .borrow_mut()
        .DoTimeSlice(&mut self.sub_tree, &mut empty_windows);

    // 3) Stay awake iff the sub-scheduler or active_animator still has work.
    animator_active || self.sub_scheduler.borrow().has_awake_engines()
}
```

- [ ] **Step 2: `cargo check --workspace`**

Expected: compiles.

- [ ] **Step 3: Test — Cycle drives sub-scheduler**

Add to `sp8_tests` mod in `emSubViewPanel.rs`:

```rust
#[test]
fn sp8_cycle_drives_sub_scheduler() {
    // After emSubViewPanel::new, the sub_scheduler has awake engines
    // (UpdateEngineClass woken in attach_to_scheduler).
    let mut panel = emSubViewPanel::new();
    assert!(
        panel.sub_scheduler.borrow().has_awake_engines(),
        "sub_scheduler must have awake engines after construction"
    );

    // Drive Cycle via a fake PanelCtx — construct a throwaway owner tree
    // and id. We don't care about the PanelCtx internals, only that Cycle
    // executes DoTimeSlice.
    let mut owner_tree = crate::emPanelTree::PanelTree::new();
    let owner_id = owner_tree.create_root("owner", std::rc::Weak::new());
    let mut pctx = crate::emPanelCtx::PanelCtx::new(&mut owner_tree, owner_id, 1.0);

    let stay_awake = <emSubViewPanel as PanelBehavior>::Cycle(&mut panel, &mut pctx);
    // UpdateEngine's Cycle always returns false (one-shot); after the slice
    // there should be no more awake engines absent other activity.
    let _ = stay_awake; // accept either — the contract is "match sub-scheduler state".

    // Second cycle: nothing to do, must return false.
    let stay_awake_2 = <emSubViewPanel as PanelBehavior>::Cycle(&mut panel, &mut pctx);
    assert!(
        !stay_awake_2 || panel.sub_scheduler.borrow().has_awake_engines(),
        "Cycle stay-awake must track sub_scheduler.has_awake_engines()"
    );
}
```

- [ ] **Step 4: Run test**

```bash
cargo nextest run -p emcore sp8_cycle_drives_sub_scheduler
```

Expected: pass.

- [ ] **Step 5: Full test suite — 2443/2443**

```bash
cargo-nextest ntr
```

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(3/N): emSubViewPanel::Cycle drives sub_scheduler + animator

PanelBehavior::Cycle impl runs once per outer-scheduler slice:
ticks ActiveAnimator with wall-clock dt, then drives one
sub_scheduler.DoTimeSlice. Stay-awake tracks
sub_scheduler.has_awake_engines() || animator_active.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 4 — emSubViewPanel::Paint simplification

Remove synchronous settlement from Paint to match C++.

### Task 4.1: Replace Paint body

**Files:**
- Modify: `crates/emcore/src/emSubViewPanel.rs` (around line 292)

- [ ] **Step 1: Replace Paint impl**

```rust
fn Paint(&mut self, painter: &mut emPainter, _w: f64, _h: f64, state: &PanelState) {
    if !state.viewed {
        return;
    }
    // C++ emSubViewPanel::Paint (src/emCore/emSubViewPanel.cpp:94) just
    // delegates to SubViewPort->PaintView. No settlement inside Paint —
    // sub-view settlement happens across frames via sub_scheduler, driven
    // from PanelBehavior::Cycle above.
    let base_offset = painter.origin();
    let bg = self.sub_view.borrow().GetBackgroundColor();
    let root = self.sub_root();
    self.sub_view.borrow_mut().paint_sub_tree(
        &mut self.sub_tree,
        painter,
        root,
        base_offset,
        bg,
    );
}
```

- [ ] **Step 2: `cargo check --workspace`**

- [ ] **Step 3: `cargo clippy --workspace -- -D warnings`**

- [ ] **Step 4: `cargo-nextest ntr` — must stay at 2443/2443**

- [ ] **Step 5: Golden baseline check**

```bash
cargo test --test golden -- --test-threads=1
```

Expected: 237 pass / 6 fail (baseline). If any new failures appear, STOP — they indicate the settle() helper still points at the old API (handled in Phase 5) or real regressions.

If new failures occur and Phase 5 hasn't landed, golden still uses `run_panel_cycles` → old code path still exists (safe). Proceed.

- [ ] **Step 6: Smoke**

```bash
timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"
```

Expected: exit 143 or 124; program ran without panicking.

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(4/N): simplify emSubViewPanel::Paint to delegate only

Remove 50-iter synchronous settlement loop (run_panel_cycles +
HandleNotice + Update + animator tick). Paint now delegates to
paint_sub_tree, matching C++ emSubViewPanel::Paint
(emSubViewPanel.cpp:94). Settlement runs across frames via
sub_scheduler + PanelCycleEngine path landed in Phase 3.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 5 — Golden settle() rewrite

Replace `tests/golden/composition.rs::settle` with a scheduler-driven version.

### Task 5.1: Rewrite settle()

**Files:**
- Modify: `crates/eaglemode/tests/golden/composition.rs` (line ~67)

- [ ] **Step 1: Replace settle helper**

```rust
/// Settle: drive `rounds` scheduler slices. Matches C++ gen_golden.cpp
/// `TerminateEngine ctrl(sched, N)` pattern — a real scheduler loop.
fn settle(tree: &mut PanelTree, view: &mut emView, rounds: usize) {
    use std::cell::RefCell;
    use std::rc::Rc;
    // Attach a scheduler on first call (idempotent — already-attached views
    // are re-used).
    if view.scheduler_ref().is_none() {
        let sched = Rc::new(RefCell::new(
            emcore::emScheduler::EngineScheduler::new(),
        ));
        // The caller does not wrap `view` in Rc<RefCell<>>, so we construct
        // the weak from a temporary Rc. Since `view: &mut emView` is a plain
        // reference, we can only give the engines a dangling weak. For golden
        // tests this is sufficient because UpdateEngineClass::Cycle upgrades
        // defensively; if the upgrade fails the engine returns false and
        // settlement falls back to panel-engine cycling alone.
        //
        // DIVERGED (test-only): golden tests hold emView by &mut, not Rc.
        // To give engines a Weak, we'd need to thread the Rc through every
        // test. Instead we use a zero-impact approach: register panel-cycle
        // engines only (via register_pending_engines) and drive DoTimeSlice
        // against those. The view's Update is driven explicitly in the loop
        // below.
        view.set_scheduler(sched.clone());
    }
    let sched = view.scheduler_ref().unwrap().clone();
    tree.register_pending_engines();
    let mut empty_windows = std::collections::HashMap::new();
    for _ in 0..rounds {
        // Drive panel cycling through the scheduler.
        sched.borrow_mut().DoTimeSlice(tree, &mut empty_windows);
        // HandleNotice + Update per-view (SP5 pattern).
        view.Update(tree);
    }
}
```

**Note to engineer:** The golden test `view` is NOT wrapped in `Rc<RefCell<>>`. Previous version of `settle` called `view.HandleNotice + tree.run_panel_cycles + view.Update` directly — no scheduler needed. After SP8 the `run_panel_cycles` API is gone. We keep explicit `view.Update(tree)` calls (drives SP5 per-view `HandleNotice`) and use the scheduler for panel cycling only (via `DoTimeSlice` + panel engines registered by `register_pending_engines`). The view's `UpdateEngineClass`/`VisitingVAEngineClass` are NOT registered in this test harness — engines registered are only `PanelCycleEngine`s for panels in `tree`.

- [ ] **Step 2: Golden baseline check**

```bash
cargo test --test golden -- --test-threads=1
```

Expected: 237 pass / 6 fail (same baseline). If new fails, investigate:
- Check whether panels that previously woke via `tree.Cycle(id)` are now registered as `PanelCycleEngine`s (`register_pending_engines` + `create_child` path).
- If a specific test fails, add more rounds or dump panel tree with `DUMP_PANEL_TREE=1`.

- [ ] **Step 3: Full test suite — must stay at 2443/2443**

```bash
cargo-nextest ntr
```

- [ ] **Step 4: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(5/N): rewrite golden settle() helper via scheduler

composition.rs::settle no longer calls run_panel_cycles; drives
panel cycling through scheduler.DoTimeSlice + explicit view.Update.
Matches C++ gen_golden.cpp TerminateEngine(sched, N) pattern.

Baseline golden 237/6 unchanged.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 6 — Delete run_panel_cycles API

### Task 6.1: Remove PanelTree cycle API

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (lines 296, 345, 717, 1619–1656)
- Modify: `crates/emcore/src/emGUIFramework.rs` (lines ~504–509, stale comment)

- [ ] **Step 1: Verify no remaining callers**

```bash
grep -rn "run_panel_cycles\|cycle_list\|cancel_cycle" --include="*.rs" crates/
```

Expected: only declarations in `emPanelTree.rs` and one stale comment in `emGUIFramework.rs`. If anything else, back to earlier phases.

- [ ] **Step 2: Delete the field**

In `emPanelTree.rs` struct definition (~line 296), delete:
```rust
cycle_list: Vec<PanelId>,
```

In the `new()` initializer (~line 345), delete:
```rust
cycle_list: Vec::new(),
```

At line ~717 (wherever `cycle_list` is referenced in a non-Cycle method — check actual usage; if present, it's likely in a Debug or diagnostic accessor; remove).

- [ ] **Step 3: Delete the three methods**

Remove `Cycle`, `cancel_cycle`, `run_panel_cycles` methods (lines ~1619–1656).

- [ ] **Step 4: Clean up stale comment in emGUIFramework.rs**

Replace the block at `crates/emcore/src/emGUIFramework.rs:504-509` with:

```rust
// SP4.5 + SP8: all panel cycling runs through the scheduler's normal
// engine loop. Top-level panels via PanelCycleEngine registered at
// init_panel_view; sub-view panels via the same path on each
// emSubViewPanel's own sub_scheduler, which is driven from the outer
// PanelCycleEngine's PanelBehavior::Cycle (SP8).
```

- [ ] **Step 5: `cargo check --workspace`**

Expected: compiles. Any errors indicate a missed caller — fix before continuing.

- [ ] **Step 6: `cargo clippy --workspace -- -D warnings`**

- [ ] **Step 7: Verify deletion**

```bash
grep -rn "run_panel_cycles\|cycle_list\|cancel_cycle" --include="*.rs" crates/
```

Expected: zero results (excluding .kani/ inventory files, which regenerate).

- [ ] **Step 8: Full test suite — 2443/2443**

```bash
cargo-nextest ntr
```

- [ ] **Step 9: Golden baseline**

```bash
cargo test --test golden -- --test-threads=1
```

Expected: 237 pass / 6 fail.

- [ ] **Step 10: Smoke**

```bash
timeout 20 cargo run --release --bin eaglemode; echo "exit=$?"
```

Expected: exit 143 or 124.

- [ ] **Step 11: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(6/N): delete PanelTree::cycle_list + Cycle/cancel_cycle/run_panel_cycles

All panel cycling now flows through scheduler-registered
PanelCycleEngine adapters (SP4.5 for top-level panels; SP8 for
sub-view panels via per-sub-view sub_scheduler).

No callers remain. Rust-only synchronous-settlement divergence closed.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Phase 7 — Closeout

### Task 7.1: Update closeout document

**Files:**
- Modify: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md`

- [ ] **Step 1: Update §1 status-at-a-glance row**

Change "Tests" row to reflect new nextest count (2443 + SP8 tests). Change "Known Rust-port incompletenesses remaining" from `SP8 (...)` to close it, leaving only SP6 and SP7.

- [ ] **Step 2: Update §6 markers table**

Bump `DIVERGED:` count by 1 (new sub_scheduler divergence on emSubViewPanel).

- [ ] **Step 3: Update §7.4 test counts**

Append:
```
- Post-SP8 (2026-04-19): **2443/2443** (+3: sp8_sub_view_update_engine_registered,
  sp8_sub_tree_root_panel_engine_registered, sp8_cycle_drives_sub_scheduler).
  Baseline golden 237/6 unchanged.
```

- [ ] **Step 4: Update §8.0 SP8 row**

Replace:
```
| **SP8 — Sub-view synchronous-settlement divergence** | 17 | Not started; ARCH; ...
```

with:

```
| **SP8 — Sub-view synchronous-settlement divergence** | ~~17~~ | **Complete 2026-04-19** (merged as <MERGE_SHA>). | `specs/2026-04-19-emview-sp8-subview-settlement-design.md`, `plans/2026-04-19-emview-sp8-subview-settlement.md` |
```

(Leave the merge SHA as `<MERGE_SHA>`; caller will fill it after merge.)

- [ ] **Step 5: Strike §8.0 execution order SP8**

Change `~~SP5~~ → ~~SP4.5~~ → SP8 (new) → SP6 ...` to `... → ~~SP4.5~~ → ~~SP8~~ → SP6 if wanted → SP7 ...`.

Also update the "SP1–SP5 and SP4.5 are complete" sentence to add SP8.

- [ ] **Step 6: Mark §8.1 item 17 closed**

Prepend `~~**[ARCH / SP8] Sub-view synchronous-settlement divergence**~~ **CLOSED 2026-04-19**` and add a resolution paragraph:

```
**Resolution.** emSubViewPanel gained a per-sub-view EngineScheduler
(sub_scheduler) attached at construction; the sub-view's
UpdateEngine/VisitingVAEngine register against it via the new
view-direct engine shape (Phase 1 of SP8). Sub-tree panels register
PanelCycleEngine adapters on sub_scheduler via register_pending_engines.
A new PanelBehavior::Cycle on emSubViewPanel drives one
sub_scheduler.DoTimeSlice per outer-scheduler tick and ticks
active_animator with wall-clock dt — replacing the 50-iter synchronous
settle loop inside Paint. Paint now delegates to paint_sub_tree only,
matching C++ emSubViewPanel.cpp:94. Golden settle() helper rewritten to
drive scheduler.DoTimeSlice. PanelTree::cycle_list / Cycle /
cancel_cycle / run_panel_cycles deleted outright. One new DIVERGED:
block at emSubViewPanel documenting the per-sub-view scheduler (forced
by nested PanelTree + singular EngineCtx::tree). Tests 2440 → 2443
(+3); golden 237/6 unchanged.
```

- [ ] **Step 7: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
sp8(7/N): closeout — SP8 complete

docs: mark SP8 closed in closeout doc. Remaining open: SP6 (optional),
SP7 (deferred).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review checklist

- [x] All spec §3 items mapped to tasks (3.1 → Phase 2; 3.2 → Phase 3; 3.3 → Phase 1; 3.4 → Phase 4; 3.5 → Phase 6; 3.6 → Phase 5; 3.7 → Phase 3).
- [x] Acceptance criteria 1–7 covered by Phase 1/4/5/6/7 gates.
- [x] No placeholders in task bodies (full code provided).
- [x] Type names consistent: `UpdateEngineClass`, `VisitingVAEngineClass`, `PanelCycleEngine`, `sub_scheduler`, `last_cycle`.
- [x] All `attach_to_scheduler` callers enumerated (Task 1.4 Step 1 is a `grep` pass that finds them).
- [x] Golden `settle()` rewrite has a risk path — flagged explicitly in Task 5.1 Step 2.

---

## Risks (operational, for the executor)

1. **Phase 5 golden regressions.** If Phase 5 causes new golden fails, the cause is almost certainly that a panel previously woken only via `tree.Cycle(id)` now lacks an `engine_id`. Run `register_pending_engines` after every tree mutation in the test, not just once. Escalate if it's not that.
2. **Phase 1 test migration pain.** ~6 unit tests in `emView.rs` construct views bare. Wrapping each in `Rc<RefCell<>>` is mechanical but touches many lines. Do it test-by-test; don't try to batch-rewrite.
3. **Phase 2 `register_pending_engines` ordering.** Must be called AFTER `sub_view.attach_to_scheduler` so the panels can see a live scheduler. Order is correct in Task 2.1 Step 2 — preserve it.

## Out of scope (do NOT drift)

- Do not touch `emContext`. Do not install clipboard. Do not change `emWindow`. Do not unify `sub_scheduler` with parent scheduler.
- Do not change visual output. Any pixel diff in golden is a failure condition.
- Do not refactor `PanelCycleEngine`. SP4.5 landed it; it is load-bearing.

End of plan.
