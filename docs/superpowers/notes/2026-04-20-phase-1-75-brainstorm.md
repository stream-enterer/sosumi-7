# Phase 1.75 — Brainstorm: Options for the Phase-1.5 Task-2 Blocker

**Captured:** 2026-04-20
**Context:** Phase 1.5 halted at Task 2 ("delete `sub_scheduler`") on a structural issue the original plan didn't anticipate: `PanelCycleEngine::Cycle` resolves its panel via `ctx.tree.take_behavior(panel_id)` using a slab-local `PanelId`, so a panel living in a different `PanelTree` cannot be reached from a shared scheduler's awake-engine dispatch loop. This note captures the option analysis that led to Option Y3 being picked.

## The blocker (precisely)

- `PanelId` is a `slab::Slab<PanelData>` key, scoped to a single `PanelTree` instance.
- `EngineScheduler`'s awake-engine dispatch takes `&mut PanelTree` (one tree) and dispatches `PanelCycleEngine::Cycle` which does `ctx.tree.take_behavior(panel_id)`.
- Sub-tree panels (inside `emSubViewPanel::sub_tree`) have IDs in a different slab — invisible to the outer tree.
- Phase 1.5 Task 2 as written assumed sub-tree panel engines could just register on the outer scheduler. They can't without a cross-tree dispatch mechanism that the plan never specified.

## Options considered

### Option A — `PanelCycleEngine` gains `Option<Weak<RefCell<PanelTree>>>`; resolves via `self.tree.upgrade()` when set
**Rejected.** Re-introduces `Rc<RefCell<PanelTree>>`. Direct regression against Phase 1.5's deletion of `PanelTree::sched_rc`. Violates CLAUDE.md: "The entire goal is to CLEAN UP divergences, workarounds, and hacks; not to create new ones."

### Option B — Delete `sub_scheduler`; `emSubViewPanel::Cycle` recursively calls `ectx.scheduler.DoTimeSlice(&mut self.sub_tree, ...)`
**Rejected.** Re-entrant `DoTimeSlice` clobbers the outer scheduler's mid-iteration state (`current_awake_idx`, `deadline`, priority queue). Fundamentally unsound.

### Option C — `PanelScope`-typed resolver on `EngineCtx` (`ctx.tree_for_scope(scope) -> &mut PanelTree`)
**Rejected.** Requires the owning `emSubViewPanel`'s behavior to be in-tree while its sub-panel adapter resolves, but the outer `PanelCycleEngine::Cycle` has already `take()`'d that behavior. Rebuilds Option B's re-entrancy hazard with a scope-typed wrapper.

### Option D — `emView::DoSlice(ectx, inner_pctx)` with outer-scheduler wake tracking
**Rejected.** Sub-tree `PanelId`s are slab-local; outer scheduler's priority-ordered awake queue cannot index them without scope partitioning. Collapses into Option C.

### Option E — Merge `sub_tree` into the outer `PanelTree` (single tree)
**Rejected.** ~30 call sites rewired; paint/input/visit/notice paths gutted. Per-sub-view `emView` state (CoordSys, tallness, focus) unchanged, so the dual-view structure remains. Enormous surface spent to delete only the scheduler divergence — most of the dual-view divergence survives. Also: C++ itself has multiple logical trees (one per `emView`), so "one tree" is not actually the C++ shape.

### Option F — Charter `sub_scheduler` as forced divergence; don't narrow the type
**Rejected.** Leaves one `Rc<RefCell<EngineScheduler>>` + multiple `try_borrow_mut` sites chartered rather than cleaned. Underreaches.

### Option G — Narrow `sub_scheduler: Rc<RefCell<EngineScheduler>>` → plain `EngineScheduler`; charter the field; amend spec §3.3
**Rejected after explicit user pushback.** Eliminates the Rc/RefCell wrapper cleanly, but **weakens a claimed observable invariant** in spec §3.3 ("outer and sub-view engines at priority P interleave in priority order within a slice"). That weakening is drift — the exact failure mode the original objective (session `05a884ab`, 2026-04-19) prohibited: "YOU ALWAYS WANT THE PORT TO BE AS GOOD AS POSSIBLE, NEVER TO BE REALISTIC. LOSS OF KNOWLEDGE IS A SERIOUS RISK AND DEFERRING ITEMS OR CONSIDERING THEM OUT OF SCOPE IS ABSOLUTELY DETRIMENTAL."

Was initially drafted as the Phase 1.75 plan (commit `c41baff`). User's question "Is this the right direction for our overall goals?" caught the drift. User chose option (ii) — re-brainstorm spec-pure.

### Option Y — Unified scheduler with `TreeLocation` + `PanelBehavior: Any` downcast
**Rejected.** Requires ~50 `PanelBehavior` impls to gain `fn as_any_mut(&mut self) -> &mut dyn Any { self }` boilerplate. Estimated scope 1500-2500 lines. User pushback on scope: "How did we get to a 1500-2500 line diff from something that was branched off from several preceding plans?"

### Option Y2 — Unified scheduler with `TreeLocation`; `sub_tree` moved to `PanelData::sub_tree: Option<Box<PanelTree>>` side-slot
**Rejected.** Avoids `Any`/downcast by moving `sub_tree` ownership into `PanelData`. But introduces ownership churn: `emSubViewPanel`'s sub_tree accessors all need `outer_tree + outer_id` to reach the sub_tree. ~40-50 call sites. Still ~600-900 lines.

### Option Y3 — Unified scheduler with `TreeLocation`; `PanelBehavior::as_sub_view_panel_mut()` with `None` default (single override on `emSubViewPanel`)  ✅ **PICKED**
- `EngineScheduler` gains `engine_locations: SecondaryMap<EngineId, TreeLocation>`.
- `DoTimeSlice` walks the `TreeLocation` per engine: `take_behavior` the owner, `.as_sub_view_panel_mut()` → `Some(sv)` → `&mut sv.sub_tree`, recurse, dispatch, put behaviors back.
- ONE trait method added with `None` default — ~50 other impls unchanged.
- `sub_tree` stays on `emSubViewPanel` (no ownership churn).
- `sub_scheduler` deleted outright.
- Spec §3.3 observational invariant preserved verbatim — §3.3 prose only clarified to describe the dispatch mechanism.
- Estimated scope 400-700 lines, concentrated in `emScheduler.rs` dispatch rewrite.

## Why Y3 over Y2

Both achieve the same cross-tree dispatch. Y2 moves `sub_tree` off `emSubViewPanel`; Y3 keeps it. Y3 wins on scope (~200 fewer lines of accessor churn) and on ownership locality (`emSubViewPanel`'s fields — sub_view, animator, sub_tree — belong together). The `take/put` behavior walk in Y3's dispatch is not a new pattern; it mirrors the existing pre-Phase-1.5 path for outer-tree panel cycles.

## Why Y3 over G (the critical choice)

G is smaller (~300 lines). Y3 is larger (400-700). But:

1. G weakens a spec invariant; Y3 preserves it.
2. G's charter would formalize the compounding-PARTIAL pattern (Phase 1 PARTIAL → Phase 1.5 PARTIAL → Phase 1.75 chartered-divergence). Y3 breaks the chain by actually shipping a COMPLETE closeout.
3. The original objective prohibits G-style drift literally: "DEFERRING ITEMS OR CONSIDERING THEM OUT OF SCOPE IS ABSOLUTELY DETRIMENTAL."
4. Y3's size (Phase-1.5-Task-1 scale) is proportionate to the problem. G's smaller size is only smaller because it stops short of solving the problem.

## Future-session auditability

If a future session revisits this decision, the rejection rationales above are the record. Do not re-derive them from scratch. If a new candidate appears (e.g., the C++ scheduler gains a feature, or `slotmap` adds cross-slab keys), evaluate it against the same criteria: does it preserve the §3.3 observable invariant without introducing `Rc<RefCell<PanelTree>>` or re-entrant `DoTimeSlice`?
