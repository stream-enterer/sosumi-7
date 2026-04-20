# Phase 1.76 — `PanelBehavior::Input` throwaway scheduler elimination

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. This phase closes JSON entry **E039** (the sole `deferred-phase-1-76` item left by Phase 1.75). Scope is a single mechanical trait-signature cascade — **narrower than any prior phase in the series**. Follow the shared bootstrap/closeout ritual at `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md` with `<N>` = `1-76`.

**Goal.** Eliminate the `throwaway_sched_input` `EngineScheduler::new()` local at `crates/emcore/src/emSubViewPanel.rs:~301` (the last chartered hack from Phase 1.75). Do so by widening `PanelBehavior::Input` to accept a `&mut PanelCtx` parameter (matching the established `notice(&mut PanelCtx)` pattern from Phase 1.75 Task 5), then cascading the parameter through every `.Input(...)` caller. Wakes emitted by `set_active_panel`/`Update` inside sub-view input handling will then reach the real outer scheduler rather than a dropped local instance.

**Goal stated as invariants:**

- **I-1.76-throwaway.** `rg 'throwaway_sched_input|let mut \w* *= *crate::emScheduler::EngineScheduler::new\(\)' crates/emcore/src/emSubViewPanel.rs` returns 0 matches in production code (test-module uses OK if any).
- **I-1.76-signature.** `PanelBehavior::Input` trait method signature includes a `ctx: &mut PanelCtx` parameter. `emPanel.rs` default impl and all overrides match.
- **I-1.76-cascade.** Every production `.Input(&event, &state, &input_state)` call site either (a) passes the ctx along to the callee, or (b) constructs a `PanelCtx` from an available `SchedCtx`/`EngineCtx` at the boundary.
- **I-1.76-no-new-hacks.** No new throwaway `EngineScheduler` instances, no `Rc<RefCell<...>>`, no `Any`/downcast, no `#[allow(...)]` outside the CLAUDE.md whitelist are introduced by this phase.

**Entry precondition.** Phase 1.75 closeout `COMPLETE` at tag `port-rewrite-phase-1-75-complete` (`50cba0d3`). Main at or ahead of that tag (merge commit `458f1fe0`). Working tree clean. JSON entry `E039` status is `deferred-phase-1-76`.

**Baseline (Phase 1.75 exit).** nextest 2454/0/9, goldens 237/6 (inherited identical failure set), `rc_refcell_total=283`, `diverged_total=176`, `rust_only_total=17`, `idiom_total=0`, `try_borrow_total=0`.

**JSON entries closed at C5:** `E039`.

**Tech stack:** unchanged.

---

## Architecture

### The current hack (at `crates/emcore/src/emSubViewPanel.rs:301`)

```rust
let mut throwaway_sched_input = crate::emScheduler::EngineScheduler::new();
// ...
let mut sc = crate::emEngineCtx::SchedCtx {
    scheduler: &mut throwaway_sched_input,
    framework_actions: &mut fw_input,
    root_context: &root_ctx_for_input,
    current_engine: None,
};
self.sub_view.borrow_mut().set_active_panel(&mut self.sub_tree, panel, false, &mut sc);
// Second sc built over the same throwaway for sub_view.Update(...)
```

Observable effect: any signal fired or engine woken through `set_active_panel`/`Update` here lands on a local `EngineScheduler` that is dropped at the end of `Input`. Those wake-ups are silent no-ops.

### The fix — shape

`PanelBehavior::Input` gains `ctx: &mut PanelCtx` (matching the `notice(&mut PanelCtx)` pattern established in Phase 1.75 Task 5). `PanelCtx` already carries `Option<&mut EngineScheduler>`, so callers with scheduler access (e.g. `emWindow::dispatch_input`, which already receives `&mut SchedCtx<'_>`) construct a `PanelCtx` with `with_scheduler(...)` and pass it through. `emSubViewPanel::Input` then uses `ctx.scheduler.as_deref_mut().expect(...)` — same pattern as `notice` — to build the real `SchedCtx` for `set_active_panel`/`Update`, and deletes the throwaway.

### Signature change

**Before:**
```rust
pub trait PanelBehavior: AsAny {
    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
    ) -> bool { false }
    // ...
}
```

**After:**
```rust
pub trait PanelBehavior: AsAny {
    fn Input(
        &mut self,
        _event: &emInputEvent,
        _state: &PanelState,
        _input_state: &emInputState,
        _ctx: &mut PanelCtx,
    ) -> bool { false }
    // ...
}
```

### PanelCtx is panel-specific (critical: DO NOT pass through recursion)

`PanelCtx<'a>` at `crates/emcore/src/emEngineCtx.rs:275` holds:

```rust
pub struct PanelCtx<'a> {
    pub tree: &'a mut PanelTree,
    pub id: PanelId,
    pub current_pixel_tallness: f64,
    pub scheduler: Option<&'a mut EngineScheduler>,
}
```

It is **scoped to ONE panel** (the `id` field). This is the SAME pattern `HandleNotice` uses: a fresh `PanelCtx` is built per panel iteration, not threaded through recursion.

**Implication for Task 2**: at every `.Input(...)` dispatch loop, the caller must construct a new `PanelCtx` with the correct `tree`, `id`, and `pixel_tallness` for each panel being dispatched to. Do NOT just pass an outer ctx through — the `id` would be wrong and `tree` may refer to a different `PanelTree` (outer vs sub).

For `emSubViewPanel::Input`'s sub-tree dispatch at line 351:
- Outer `ctx: &mut PanelCtx` has `tree: &mut outer_tree, id: emSubViewPanel_id`.
- For each `panel_id` in `self.sub_tree.viewed_panels_dfs()`, build a **fresh** `PanelCtx { tree: &mut self.sub_tree, id: panel_id, current_pixel_tallness: pixel_tallness, scheduler: ctx.scheduler.as_deref_mut() }`.
- Pass THAT fresh ctx to `behavior.Input(...)`.

### Cascade scope (measured — recounted during plan review)

Raw greps inflate the count with non-`PanelBehavior::Input` methods. Exact scope:

- **`PanelBehavior::Input` trait default**: 1 (in `emPanel.rs`).
- **`PanelBehavior::Input` production overrides**: ~19 across `emSubViewPanel.rs`, `emTextField.rs`, `emFileManControlPanel.rs`, `emDirPanel.rs`, `emDirEntryPanel.rs`, `emDirEntryAltPanel.rs`, `emStocksFilePanel.rs`, `emMainPanel.rs`, `emStarFieldPanel.rs`, `emMainControlPanel.rs` (3), `emAutoplayControlPanel.rs` (4), `emFileManSelInfoPanel.rs` (if present — re-grep).
- **`PanelBehavior::Input` test overrides**: ~25 across `crates/eaglemode/tests/`.
- **`.Input(...)` call sites (ALL `fn Input` types, not just `PanelBehavior::Input`)**: 226 total, 174 production / 52 tests. **Not all are in scope** — see "Out of scope" below.

### Out of scope — `Input` methods that STAY unchanged

Do NOT change these signatures. They are NOT `PanelBehavior::Input`:

- `crates/emcore/src/emView.rs:4110` — `emView::Input` inherent method on `emView`, already takes `ctx: &mut SchedCtx<'_>`. Untouched.
- `crates/emcore/src/emColorField.rs:472` — `emColorField::Input` inherent helper `pub fn Input(...)`. Widget helper, not a trait method. **Decision**: leave unchanged for Phase 1.76; forwarding sites (e.g. `emAutoplayControlPanel.rs:235` `self.scalar_field.Input(...)`) drop `ctx` at the boundary. Observational impact: if `emColorField` internally wanted to fire signals during Input, those would be no-ops. Audit during Task 2: does `emColorField::Input` currently fire signals or wake engines? If yes → escalate scope; if no → document in ledger and proceed.
- `crates/emcore/src/emViewAnimator.rs:33, 2296` — `trait emViewAnimator::Input(&mut self, &mut emInputEvent, &emInputState)` — different trait, no return. Untouched.
- `crates/emcore/src/emTextField.rs` `pub fn Input` (if any) — separate from `PanelBehavior::Input`; audit per-site.
- Any `Input` method with a signature that returns `()` (not `bool`) or takes `&mut emInputEvent` (not `&emInputEvent`) — those are the `emViewAnimator::Input` variant.

### Sites that DO need construction of fresh `PanelCtx`

- `emWindow::dispatch_input:1036` — has `ctx: &mut SchedCtx<'_>` in scope (the outer dispatch ctx). For each panel iterated in the behavior dispatch, build `PanelCtx::with_scheduler(tree, panel_id, current_pixel_tallness, ctx.scheduler)`.
- `emSubViewPanel::Input:351` — constructs per-sub-panel PanelCtx from outer `ctx.scheduler.as_deref_mut()` (see "Implication for Task 2" above).
- `emViewPort::InputToView` — audit: does it dispatch to `behavior.Input(...)`, or does it prologue-stamp only? If it dispatches, it also constructs PanelCtx.
- Test harnesses that directly call `.Input(...)` on a panel: construct a test-mode `PanelCtx`. Two options:
  - `PanelCtx::new(tree, id, pixel_tallness)` — scheduler is `None`. **Only safe** if the panel's Input doesn't require a scheduler (leaf widgets). If you use this against `emSubViewPanel::Input`, its `expect("PanelCtx with a scheduler")` will panic.
  - `PanelCtx::with_scheduler(tree, id, pixel_tallness, &mut test_sched)` — required for `emSubViewPanel::Input` and any override that ends up calling scheduler-requiring code.

### Scheduler re-borrow pattern (avoid this footgun)

Inside `emSubViewPanel::Input`, two call sites need `SchedCtx` (`set_active_panel` and `sub_view.Update`). The scheduler field is `Option<&mut EngineScheduler>` — you can only take ONE mutable borrow at a time. Scope each `SchedCtx` construction in its own block:

```rust
// CORRECT — re-borrow in each scope:
if event.is_mouse_event() && event.variant == InputVariant::Press {
    // ... compute `panel` ...
    let mut sc = crate::emEngineCtx::SchedCtx {
        scheduler: ctx.scheduler.as_deref_mut().expect(
            "emSubViewPanel::Input requires PanelCtx with a scheduler (Phase 1.76)"
        ),
        framework_actions: &mut fw_input,
        root_context: &root_ctx_for_input,
        current_engine: None,
    };
    self.sub_view.borrow_mut().set_active_panel(&mut self.sub_tree, panel, false, &mut sc);
}
// `sc` dropped here → scheduler borrow released

{
    let mut sc = crate::emEngineCtx::SchedCtx {
        scheduler: ctx.scheduler.as_deref_mut().expect(
            "emSubViewPanel::Input requires PanelCtx with a scheduler (Phase 1.76)"
        ),
        framework_actions: &mut fw_input,
        root_context: &root_ctx_for_input,
        current_engine: None,
    };
    self.sub_view.borrow_mut().Update(&mut self.sub_tree, &mut sc);
}
```

```rust
// WRONG — second sc construction fails (sched already moved):
let sched = ctx.scheduler.as_deref_mut().expect("...");
let mut sc1 = SchedCtx { scheduler: sched, ... };
self.sub_view.borrow_mut().set_active_panel(..., &mut sc1);
let mut sc2 = SchedCtx { scheduler: sched, ... }; // E0382: sched moved
```

### Observable behavior audit (mandatory before final gate)

Today `throwaway_sched_input` silently drops wakes from `set_active_panel` and `Update` fired during mouse press / input dispatch. Post-fix those wakes fire on the real outer scheduler. Signals likely unmasked:

- `ACTIVE_CHANGED` — fires when `set_active_panel` changes the active panel during a mouse press.
- `FOCUS_CHANGED` — fires from focus updates inside `set_active_panel`.
- `VIEWING_CHANGED` — fires from `Update` if it triggers geometry recomputation.
- Timer signals associated with `UpdateEngineClass` — wakes mid-input cause `UpdateEngineClass` to re-run next slice.

**Before final commit**: compare Rust behavior to C++ `emSubViewPanel::Input` + `emViewPort::InputToView` (`~/git/eaglemode-0.96.4/src/emCore/emSubViewPanel.cpp`, `emViewPort.cpp`). Confirm:
1. C++ `emSubViewPanel::Input` does invoke `SetActivePanel` and `Update`-equivalents inline during mouse press.
2. The signals C++ fires during that inline work propagate to the shared scheduler (they do — all emEngines share one scheduler in C++).
3. No C++ guard suppresses signal propagation during input dispatch.

If any of the three are false → the Rust "unmasked wakes" are a divergence from C++, not a bug fix. STOP and escalate with C++ quote + diff.

If all three are true → the unmasked wakes are C++-correct. Goldens that shift are exposing latent bugs the throwaway was hiding; audit diffs, fix underlying issues, do NOT re-suppress.

---

## Companion documents

- Spec: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` §3.3 end (current "Phase-1.76 Input throwaway callout" — will be **deleted** in C8 when the residual is resolved).
- Phase 1.75 closeout: `docs/superpowers/notes/2026-04-20-phase-1-75-closeout.md` (authority for the deferral).
- Phase 1.75 ledger: `docs/superpowers/notes/2026-04-20-phase-1-75-ledger.md` (Task 5 continuation entry describes why the cascade was deferred).
- Bootstrap/closeout ritual: `docs/superpowers/plans/2026-04-19-port-rewrite-bootstrap-ritual.md`.
- JSON raw-material entry: `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` entry `E039`.

---

## Bootstrap (per shared ritual)

Run B1–B12 with `<N>` = `1-76`.

Deviations:
- **B2.** Confirm JSON entry `E039` is `status: deferred-phase-1-76` before entering the phase. Any other `deferred-phase-1-76` entries that have accumulated should be added to this phase's closeout list or explicitly deferred to a later phase with rationale.
- **B4.** Prior-phase closeout is `COMPLETE` (first in series). No PARTIAL to sanction. Standard chain.
- **B7.** Baseline = Phase 1.75 exit (see metrics above).
- **B9.** Branch: `port-rewrite/phase-1-76` off `main` at or after `458f1fe0`.
- **B10.** Brainstorm note is not required — scope is single-item and captured in this plan's Architecture section. Skip writing a separate brainstorm unless mid-phase a design question surfaces that warrants recording.
- **B11.** Commit: `phase-1-76: bootstrap — baseline captured, ledger opened`.

---

## File Structure

**Files modified (core):**

- `crates/emcore/src/emPanel.rs` — `PanelBehavior::Input` trait default gains `_ctx: &mut PanelCtx` parameter (line ~201 per phase-1.75 exit state; re-grep).
- `crates/emcore/src/emSubViewPanel.rs` — override `Input` gains `ctx: &mut PanelCtx`; delete `throwaway_sched_input` (lines 287–300 tombstone comment + line 301 declaration + lines 310–318 and 321–328 SchedCtx construction); build the two `SchedCtx` instances from `ctx.scheduler.as_deref_mut().expect("emSubViewPanel::Input requires PanelCtx with a scheduler (Phase 1.76)")` — same pattern as `notice`. Recursive `behavior.Input(&panel_ev, &panel_state, input_state, ctx)` call at line 351 passes `ctx` through.
- `crates/emcore/src/emWindow.rs` — `dispatch_input`'s panel-tree dispatch loop at line 1036: construct `PanelCtx::new_event(scheduler: Some(ctx.scheduler))` (or whichever PanelCtx constructor carries scheduler — use `PanelCtx::with_scheduler`) and pass to `behavior.Input(...)`. Re-check: if there's a `PanelCtx::new_event` convention post-1.75 absorption, use it; otherwise add a construction helper in `emEngineCtx.rs`.
- `crates/emcore/src/emViewPort.rs` — if `InputToView` dispatches into `behavior.Input(...)`, update similarly. If it only prologue-stamps and returns, no change.
- `crates/emcore/src/emPanelCycleEngine.rs` — if it calls `.Input(...)` during Cycle, update. (Unlikely per Phase 1.5/1.75 work; re-check.)

**Files modified (downstream — mechanical cascade):**

- All 19 production override files: append `_ctx: &mut PanelCtx` to each override signature. If the override body does not need the ctx, prefix the param with `_`. If the body forwards to an inner widget (`self.button.Input(event, state, input_state)` pattern at `emMainControlPanel.rs:68`, `emAutoplayControlPanel.rs:142`, `emFileManControlPanel.rs:390–504`, etc.), pass `ctx` through.

**Files modified (tests):**

- `crates/eaglemode/tests/support/mod.rs` (2 overrides) — update signature.
- `crates/eaglemode/tests/golden/composition.rs` (9 overrides) — update signature.
- `crates/eaglemode/tests/golden/test_panel.rs` (10 overrides) — update signature.
- `crates/eaglemode/tests/pipeline/calibration.rs` (4 overrides), `check.rs` (2 overrides) — update signature.
- Other test files with `.Input(...)` calls: synthesize a throwaway-in-test-only `PanelCtx` (test-module throwaways are acceptable — this phase's I-1.76-throwaway applies to production only).

**Files touched (docs):**

- `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` — delete the "Phase-1.76 Input throwaway callout" at the end of §3.3.
- `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` — entry `E039` → `status: resolved-phase-1-76`, `resolution_commit: <task-2-sha>`.
- `docs/superpowers/notes/2026-04-20-phase-1-76-ledger.md` (new).
- `docs/superpowers/notes/2026-04-20-phase-1-76-baseline.md` (new, at B10).
- `docs/superpowers/notes/2026-04-20-phase-1-76-exit.md` (new, at C2).
- `docs/superpowers/notes/2026-04-20-phase-1-76-closeout.md` (new, at C7).

**Files deleted:** none.

---

## Task sequencing

### Task 1 — Plumbing & construction helper (small, additive)

**Goal:** Before the atomic signature change, ensure a clean `PanelCtx` constructor exists for the input-dispatch callers. This task adds nothing observable; it only prepares the surface.

**Files:**
- Modify: `crates/emcore/src/emEngineCtx.rs` (PanelCtx constructor section).

**Steps:**

- [ ] **Step 1: Audit existing PanelCtx constructors.** Read `emEngineCtx.rs` for all existing `PanelCtx::new*` / `with_*` / `impl PanelCtx { ... }` surfaces. Grep: `rg -n 'impl PanelCtx|fn new|with_scheduler' crates/emcore/src/emEngineCtx.rs`.

- [ ] **Step 2: Verify a constructor exists that takes a scheduler.** Per Phase 1.75 Task 5, `PanelCtx::with_scheduler(...)` was added to satisfy `HandleNotice`. Confirm its signature:

  ```bash
  rg -n 'fn with_scheduler' crates/emcore/src/emEngineCtx.rs
  ```

  Expected: a constructor taking `scheduler: Option<&mut EngineScheduler>` (or equivalent) and the same ctx-fields `notice` uses.

- [ ] **Step 3: If constructor is sufficient, no code changes.** This step's output is an entry in the ledger confirming: "Task 1 — Audit complete; PanelCtx::with_scheduler already suffices for input-dispatch use (used by notice path in Phase 1.75). Proceed to Task 2 signature change." Commit:

  ```bash
  git add docs/superpowers/notes/2026-04-20-phase-1-76-ledger.md
  git commit -m "phase-1-76 task-1: audit PanelCtx constructors — no changes needed"
  ```

- [ ] **Step 4: If constructor needs extension** (e.g. input dispatch needs a field `notice` doesn't, like active-panel or focus state): add the new constructor or extend the existing one. Show the exact signature in the ledger entry. Run `cargo check` to confirm clean. Commit: `phase-1-76 task-1: extend PanelCtx constructor for input-dispatch use`.

### Task 2 — Signature cascade + throwaway deletion (KEYSTONE)

**Goal:** Single atomic signature change. After this task: the hack is gone.

This task is mechanical but touches ~60 files. Expect 300–600 lines of diff. `--no-verify` checkpoint commits permitted within this task; final commit passes full gate.

**Files:**
- Modify (trait): `crates/emcore/src/emPanel.rs:201`.
- Modify (throwaway site): `crates/emcore/src/emSubViewPanel.rs:231–363`.
- Modify (top-level caller): `crates/emcore/src/emWindow.rs:1036`.
- Modify (potentially): `crates/emcore/src/emViewPort.rs`, `crates/emcore/src/emView.rs` (if they dispatch Input).
- Modify (19 production override files — see File Structure).
- Modify (test files — see File Structure).

**Steps:**

- [ ] **Step 1: Update the trait default.** `crates/emcore/src/emPanel.rs` around line 201:

  ```rust
  fn Input(
      &mut self,
      _event: &emInputEvent,
      _state: &PanelState,
      _input_state: &emInputState,
      _ctx: &mut PanelCtx,
  ) -> bool {
      false
  }
  ```

  Run: `cargo check` — expect many errors (signature mismatches).

- [ ] **Step 2: Update `emSubViewPanel::Input`.** At `crates/emcore/src/emSubViewPanel.rs:231`, update the override signature to include `ctx: &mut PanelCtx`. Delete the tombstone comment at lines 287–300. Delete `let mut throwaway_sched_input = crate::emScheduler::EngineScheduler::new();` at line 301.

  Replace the two `SchedCtx { scheduler: &mut throwaway_sched_input, ... }` constructions with **two scoped blocks** that each re-borrow the ctx scheduler (see Architecture → "Scheduler re-borrow pattern" for the full explanation of why this is required and what the footgun looks like):

  ```rust
  // Mouse press → set_active_panel (first scope — borrow released at block end):
  if event.is_mouse_event() && event.variant == crate::emInput::InputVariant::Press {
      let panel = self
          .sub_view
          .borrow()
          .GetFocusablePanelAt(&self.sub_tree, sub_vx, sub_vy)
          .unwrap_or_else(|| self.sub_view.borrow().GetRootPanel());
      let mut sc = crate::emEngineCtx::SchedCtx {
          scheduler: ctx.scheduler.as_deref_mut().expect(
              "emSubViewPanel::Input requires PanelCtx with a scheduler (Phase 1.76)"
          ),
          framework_actions: &mut fw_input,
          root_context: &root_ctx_for_input,
          current_engine: None,
      };
      self.sub_view
          .borrow_mut()
          .set_active_panel(&mut self.sub_tree, panel, false, &mut sc);
  }

  // Update sub-view geometry (second scope — re-borrow fresh):
  {
      let mut sc = crate::emEngineCtx::SchedCtx {
          scheduler: ctx.scheduler.as_deref_mut().expect(
              "emSubViewPanel::Input requires PanelCtx with a scheduler (Phase 1.76)"
          ),
          framework_actions: &mut fw_input,
          root_context: &root_ctx_for_input,
          current_engine: None,
      };
      self.sub_view
          .borrow_mut()
          .Update(&mut self.sub_tree, &mut sc);
  }
  ```

  For the recursive sub-tree dispatch at line 351 (`behavior.Input(&panel_ev, &panel_state, input_state)`), construct a **fresh per-sub-panel PanelCtx** (PanelCtx is panel-specific — see Architecture → "PanelCtx is panel-specific"):

  ```rust
  for panel_id in viewed {
      let mut panel_ev = event.clone();
      panel_ev.mouse_x = self.sub_tree.ViewToPanelX(panel_id, sub_vx);
      panel_ev.mouse_y = self.sub_tree.ViewToPanelY(panel_id, sub_vy, pixel_tallness);
      if let Some(mut behavior) = self.sub_tree.take_behavior(panel_id) {
          let panel_state = self
              .sub_tree
              .build_panel_state(panel_id, wf, pixel_tallness);
          if panel_ev.is_keyboard_event() && !panel_state.in_active_path {
              self.sub_tree.put_behavior(panel_id, behavior);
              continue;
          }
          let consumed = {
              // Fresh PanelCtx per sub-panel: tree is the sub_tree,
              // id is the sub-panel, scheduler re-borrows from outer ctx.
              let mut panel_ctx = match ctx.scheduler.as_deref_mut() {
                  Some(sched) => crate::emEngineCtx::PanelCtx::with_scheduler(
                      &mut self.sub_tree, panel_id, pixel_tallness, sched,
                  ),
                  None => crate::emEngineCtx::PanelCtx::new(
                      &mut self.sub_tree, panel_id, pixel_tallness,
                  ),
              };
              behavior.Input(&panel_ev, &panel_state, input_state, &mut panel_ctx)
          };
          self.sub_tree.put_behavior(panel_id, behavior);
          if consumed {
              self.sub_view
                  .borrow_mut()
                  .InvalidatePainting(&self.sub_tree, panel_id);
              return true;
          }
      }
  }
  ```

  Note the `match ctx.scheduler.as_deref_mut()` — if outer `ctx.scheduler` is `None`, pass `None` through. This preserves the test-mode-no-scheduler path.

- [ ] **Step 3: Update `emWindow::dispatch_input`.** At `crates/emcore/src/emWindow.rs:1036`, the dispatch loop. `emWindow::dispatch_input` has signature `fn dispatch_input(&mut self, tree: &mut PanelTree, event: &emInputEvent, state: &mut emInputState, ctx: &mut SchedCtx<'_>)` — the scheduler is in `ctx.scheduler`. Construct a **fresh per-panel PanelCtx** at the behavior dispatch site:

  ```rust
  // Inside the dispatch loop at line ~1036, per-panel:
  let pixel_tallness = /* fetch current_pixel_tallness from the view */;
  let consumed = {
      let mut panel_ctx = crate::emEngineCtx::PanelCtx::with_scheduler(
          tree, panel_id, pixel_tallness, ctx.scheduler,
      );
      behavior.Input(&panel_ev, &panel_state, state, &mut panel_ctx)
  };
  ```

  Verify in `emWindow.rs` how `current_pixel_tallness` is accessed at this site — likely `self.view.borrow().GetCurrentPixelTallness()` or via `state.pixel_tallness`. Do not guess: re-read the surrounding code before writing.

  **If `emViewPort::InputToView` at `emViewPort.rs` also dispatches into `behavior.Input(...)`**, apply the same construction pattern there. Audit at Step 3 start; add a sub-step if needed.

- [ ] **Step 4: Mechanical cascade — production override files.** For each file in this list, append `_ctx: &mut PanelCtx` to the `Input` override signature. If the override body forwards to an inner widget's `Input`, pass `ctx` through:

  - `crates/emcore/src/emColorField.rs` (note: `pub fn Input` at line 472 is a helper, not a trait override — only update if its callers change)
  - `crates/emcore/src/emSubViewPanel.rs` (already done in Step 2)
  - `crates/emcore/src/emTextField.rs`
  - `crates/emcore/src/emView.rs` (if it has a `PanelBehavior::Input` override — re-grep)
  - `crates/emcore/src/emViewAnimator.rs` (only if trait override; free fn stays unchanged)
  - `crates/emfileman/src/emDirEntryAltPanel.rs`
  - `crates/emfileman/src/emDirEntryPanel.rs`
  - `crates/emfileman/src/emDirPanel.rs`
  - `crates/emfileman/src/emFileManControlPanel.rs` (1 override + 13 forwarding callers at lines 390–504)
  - `crates/emmain/src/emAutoplayControlPanel.rs` (4 overrides, each forwards to one inner widget)
  - `crates/emmain/src/emMainControlPanel.rs` (3 overrides, forwarding pattern)
  - `crates/emmain/src/emMainPanel.rs` (3 overrides)
  - `crates/emmain/src/emStarFieldPanel.rs` (1 override)
  - `crates/emstocks/src/emStocksFilePanel.rs` (1 override + extensive test-callers at lines 455–699)

  Re-grep mid-task if the file list drifts:
  ```bash
  rg -l 'fn Input\s*\(' crates/ | rg -v '/tests/|tests/'
  ```

  For each file: append the param, prefix with `_` if unused, and fix forwarding calls. Run `cargo check` periodically to verify progress shrinks the error list.

- [ ] **Step 5: Mechanical cascade — test files.** For each test file with `PanelBehavior::Input` overrides or callers:

  - `crates/eaglemode/tests/golden/composition.rs` (9 overrides, all stub-style)
  - `crates/eaglemode/tests/golden/test_panel.rs` (10 overrides)
  - `crates/eaglemode/tests/pipeline/calibration.rs` (4 overrides)
  - `crates/eaglemode/tests/pipeline/check.rs` (2 overrides)
  - `crates/eaglemode/tests/support/mod.rs` (2 overrides)

  Append `_ctx: &mut PanelCtx` to each override. For tests that directly call `.Input(...)` on a panel (e.g. `emStocksFilePanel.rs` test-module calls at lines 455–699), synthesize a test `PanelCtx`:

  ```rust
  let mut test_sched = crate::emScheduler::EngineScheduler::new();
  let root_ctx = crate::emContext::emContext::NewRoot();
  let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
  let mut pctx = crate::emEngineCtx::PanelCtx::with_scheduler(
      Some(&mut test_sched), &mut fw, &root_ctx, /* other fields per Task 1 audit */,
  );
  panel.Input(&event, &state, &input_state, &mut pctx);
  ```

  Test-module `EngineScheduler::new()` instances are permitted (the I-1.76-throwaway invariant applies to production only). They should be clearly named `test_sched` or similar — not `throwaway_sched_input`.

- [ ] **Step 6: Intermediate `--no-verify` checkpoint if needed.** If the cascade halts mid-way (e.g. context limit, cargo check error volume), commit the current state:

  ```bash
  git add -A
  git commit --no-verify -m "phase-1-76 task-2 (wip): signature cascade checkpoint — NN files remaining"
  ```

  Plan the resumption: track which files are done via `git status` / `rg -l 'fn Input\s*\(' crates/` delta.

- [ ] **Step 7: Full gate.** Once `cargo check` is clean:

  ```bash
  cargo fmt
  cargo clippy --all-targets --all-features -- -D warnings
  cargo-nextest ntr
  cargo test --test golden -- --test-threads=1
  ```

  Expected results:
  - fmt: clean (apply drift)
  - clippy: clean
  - nextest: 2454 passed / 0 failed / 9 skipped (baseline held)
  - goldens: 237 passed / 6 failed (identical failure set — inherited)

  If goldens shift, investigate: a real wake-up that was previously silent now fires. Pixel diffs may reveal a latent bug the throwaway was masking. Do NOT revert the fix to make goldens match — document any shift in the ledger and proceed only with reviewer audit rationale.

- [ ] **Step 8: Verify invariants.**

  ```bash
  # I-1.76-throwaway:
  rg -n 'throwaway_sched_input' crates/emcore/src/emSubViewPanel.rs
  # expect: 0 matches

  rg -n 'let mut \w* *= *crate::emScheduler::EngineScheduler::new\(\)' crates/emcore/src/emSubViewPanel.rs
  # expect: 0 matches in production (test-module OK, audit output)

  # I-1.76-signature:
  rg -n 'fn Input\s*\([^)]*&mut PanelCtx' crates/emcore/src/emPanel.rs
  # expect: 1 match (trait default)

  # I-1.76-no-new-hacks (whitelist audit):
  rg -n '#\[allow\(' crates/ | rg -v 'too_many_arguments|non_snake_case|non_camel_case_types'
  # every remaining match must be pre-existing; compare to baseline
  ```

- [ ] **Step 9: Final commit (passes full gate).**

  ```bash
  git add -A
  git commit -m "phase-1-76 task-2: widen PanelBehavior::Input with PanelCtx; delete throwaway scheduler"
  ```

  The pre-commit hook MUST pass without `--no-verify`. If it fails on a pre-existing issue, investigate and fix — do not bypass.

### Task 3 — Spec cleanup (docs only)

**Goal:** Remove the "Phase-1.76 Input throwaway callout" from spec §3.3 now that the residual is gone.

**Files:**
- Modify: `docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md`.

**Steps:**

- [ ] **Step 1: Locate callout.** `rg -n 'Phase.?1\.76|throwaway' docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md`. Identify the "Phase-1.76 Input throwaway callout" block at the end of §3.3 (written in Phase 1.75 Task 7).

- [ ] **Step 2: Delete the callout.** Remove the block verbatim. If the callout's closing prose ties into surrounding §3.3 content, smooth the prose so §3.3 reads cleanly without it. Do NOT add a "was-deferred-now-resolved" annotation in the spec itself; the closed JSON entry and Phase-1.76 closeout note are the resolution records.

- [ ] **Step 3: Verify §3.3 observable invariant still preserved.** `rg -n 'interleave in priority order' docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md` — the core §3.3 invariant (preserved verbatim in Phase 1.75) must remain intact.

- [ ] **Step 4: Ledger entry + commit.**

  ```bash
  git add docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md docs/superpowers/notes/2026-04-20-phase-1-76-ledger.md
  git commit -m "phase-1-76 task-3: remove Phase-1.76 Input throwaway callout from spec §3.3"
  ```

### Task 4 — Closeout (run C1–C11 per shared ritual)

**Goal:** Close Phase 1.76. Second COMPLETE in the port-rewrite series.

**Files:**
- Create: `docs/superpowers/notes/2026-04-20-phase-1-76-exit.md`
- Create: `docs/superpowers/notes/2026-04-20-phase-1-76-closeout.md`
- Modify: `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` (entry E039 → `resolved-phase-1-76`).
- Modify: `docs/superpowers/notes/2026-04-20-phase-1-76-ledger.md` (final entry).

**Steps:**

- [ ] **Step 1: Full gate.** Re-run:

  ```bash
  cargo fmt
  cargo clippy --all-targets --all-features -- -D warnings
  cargo-nextest ntr
  cargo test --test golden -- --test-threads=1
  ```

  Capture pass/fail counts for exit metrics.

- [ ] **Step 2: C4 — verify invariants.** For each Phase 1.76 invariant (I-1.76-throwaway, I-1.76-signature, I-1.76-cascade, I-1.76-no-new-hacks), run the grep/check and record the command + result in `2026-04-20-phase-1-76-exit.md` under `## Invariants`. Also re-verify Phase 1.75 carry-forwards (I1, I1c, I1d, I-Y3-dispatch, I-T3a, I-T3b, I-T3c, I-Spec-3.3-clarified, Task-10, Task-11) remain SAT.

- [ ] **Step 3: C5 — mark JSON entry E039 resolved.** Edit `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` entry E039:

  ```json
  "status": "resolved-phase-1-76",
  "resolution_commit": "<task-2 commit sha>"
  ```

  Also verify no other `deferred-phase-1-76` entries remain unclosed:

  ```bash
  rg '"status": "deferred-phase-1-76"' docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json
  # expect: 0 matches
  ```

- [ ] **Step 4: C6 — commit JSON.**

  ```bash
  git add docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json
  git commit -m "phase-1-76: mark JSON entry E039 resolved"
  ```

- [ ] **Step 5: C7 — write closeout note.** Create `docs/superpowers/notes/2026-04-20-phase-1-76-closeout.md` with:

  - **Status line:** `COMPLETE — all C1–C11 invariants SAT`.
  - Summary of what changed (one short paragraph).
  - Exit metric delta table (compare to baseline).
  - Invariants SAT list (Phase 1.76 new + Phase 1.75 carry-forwards).
  - "Next phase" pointer (if a Phase 2 or other follow-up is planned) or explicit "no deferrals" statement.

- [ ] **Step 6: C8 — exit metrics doc.** Create `docs/superpowers/notes/2026-04-20-phase-1-76-exit.md` with nextest, goldens, rc_refcell_total, diverged_total, rust_only_total, idiom_total, try_borrow_total — same format as `2026-04-20-phase-1-75-exit.md`.

- [ ] **Step 7: C8 continued — final commit.**

  ```bash
  git add docs/superpowers/notes/2026-04-20-phase-1-76-*.md
  git commit -m "phase-1-76: closeout COMPLETE — all C1-C11 SAT"
  ```

  Pre-commit hook must pass without `--no-verify`.

- [ ] **Step 8: C9 — merge to main (requires user confirmation per Phase 1.75 precedent).** Offer merge; do not execute without OK:

  ```bash
  git checkout main
  git merge --no-ff port-rewrite/phase-1-76 -m "Merge phase-1-76 COMPLETE: Input throwaway elimination"
  ```

- [ ] **Step 9: C10 — tag.**

  ```bash
  git tag port-rewrite-phase-1-76-complete
  ```

- [ ] **Step 10: C11 — announce.** Report phase complete with: tag SHA, exit metrics delta, E039 closed, no deferrals (or list remaining deferrals if any surfaced mid-phase).

---

---

## Known-but-descoped decisions (documented in case a reviewer queries them)

These were considered during planning and deliberately left OUT of Phase 1.76. If Task 2 reveals any of them as blockers, STOP and escalate — do not silently expand scope.

1. **`emColorField::Input` (inherent helper, `emColorField.rs:472`)**: called from forwarding widgets (e.g. `emAutoplayControlPanel.rs:235`). Not updated. If this helper internally wakes engines or fires signals, those would be silent no-ops on the forwarding path post-Phase-1.76. Audit during Task 2. If it does wake/fire, open Phase 1.77 (or widen 1.76 in-scope).
2. **`emView::Input` (inherent, `emView.rs:4110`)**: already takes `&mut SchedCtx` — no change needed.
3. **`emViewAnimator` trait `Input`**: different trait, different signature, not in scope.
4. **Observable behavior changes from unmasked wakes**: may shift goldens. Plan requires C++ audit before accepting the shift (see Architecture → "Observable behavior audit").

---

## Risk + halt-recovery

**Task 2 is the load-bearing step.** Expect 300–600 lines of diff. If the subagent halts mid-cascade, the branch carries `--no-verify` red commits from which the next session resumes — same pattern Phase 1.5 Task 1 and Phase 1.75 Task 4 used successfully.

**Goldens may shift.** Today's `throwaway_sched_input` eats signals. Post-fix, those signals fire on the real scheduler and may cause observable changes:
- If pixel diffs appear on mouse/touch interaction goldens that were passing: a latent bug in `set_active_panel` or `Update` signal propagation may be surfaced. Audit the diff, compare to C++ reference — pixel correctness to C++ wins.
- If pixel diffs appear on the 6 already-failing goldens: irrelevant (they were already failing).
- If new goldens start failing that weren't before: STOP and audit. Do not paper over with a `#[allow]` or an assertion rewrite.

**Forwarding widget chains.** Files like `emFileManControlPanel.rs` forward `Input` to 13+ inner widgets. Each forwarding site needs the ctx argument added. Mechanical but tedious. Use `cargo check` as a progress indicator — errors shrink as call sites are fixed.

**Trait object lifetimes.** `PanelCtx` has lifetime parameters; the new `&mut PanelCtx` argument must compose with `&mut dyn PanelBehavior` in the recursive dispatch at `emSubViewPanel::Input:351`. If the borrow checker complains, restructure the recursive call to re-borrow `ctx` fields inline rather than holding a long-lived `&mut PanelCtx`. Do NOT reach for `RefCell<PanelCtx>` or a clone-style workaround.

**Escalation triggers.**
- If the cascade reveals a `PanelBehavior::Input` caller that has NO scheduler access and CANNOT be given one (e.g. a Drop path), STOP and escalate BLOCKED. The plan assumes every caller has a scheduler in scope — if that's false, the design needs a rethink (possibly back to a sentinel `None` on the ctx's scheduler field, matching `notice`'s pattern).
- If goldens shift in a way that cannot be audited to a known-good C++ behavior, STOP and escalate.

---

## Self-review checklist (before Closeout)

- [ ] `rg -n 'throwaway_sched_input' crates/` empty.
- [ ] `rg -n 'EngineScheduler::new\(\)' crates/emcore/src/emSubViewPanel.rs` returns only `#[cfg(test)]`-module lines (or zero).
- [ ] `PanelBehavior::Input` trait default at `crates/emcore/src/emPanel.rs` has `_ctx: &mut PanelCtx`.
- [ ] Every `PanelBehavior::Input` override (19 production + ~25 test) has the new param.
- [ ] Every `.Input(...)` call site passes a `&mut PanelCtx`.
- [ ] `emWindow::dispatch_input` constructs `PanelCtx` carrying the outer `SchedCtx`'s scheduler.
- [ ] `emSubViewPanel::Input` uses `ctx.scheduler.as_deref_mut().expect(...)` like `notice` does — no inline `EngineScheduler::new()`.
- [ ] Spec §3.3 no longer contains the "Phase-1.76 Input throwaway callout" block.
- [ ] JSON entry E039 status = `resolved-phase-1-76`, `resolution_commit` set.
- [ ] Goldens 237/6 or better. Nextest ≥ 2454. Clippy clean. fmt clean.
- [ ] No new `#[allow(...)]` outside CLAUDE.md whitelist (`too_many_arguments`, `non_snake_case` on emCore module, `non_camel_case_types` on em-prefixed types).
- [ ] No new `Rc<RefCell<...>>` in this phase's diff.
- [ ] No `Any`/downcast introduced in this phase's diff.
- [ ] No `--no-verify` commits remain in the phase's history except intermediate Task 2 checkpoints (which must be followed by a green final commit at the task's end).
- [ ] Ledger `2026-04-20-phase-1-76-ledger.md` has an entry per task.
- [ ] Closeout note `2026-04-20-phase-1-76-closeout.md` written with status `COMPLETE — all C1–C11 invariants SAT`.
