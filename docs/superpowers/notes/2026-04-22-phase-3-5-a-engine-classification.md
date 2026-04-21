# Phase 3.5.A — Engine Classification Sheet

**Produced:** Task 2 of Phase 3.5.A plan (commit base: 75ae0428).
**Authority for:** Tasks 6, 8, 10 migrations.

Classification key:
- **Framework** — `Cycle` body does NOT dereference `ctx.tree`. Engine is window-agnostic or routes through `ctx.windows[wid]` (post-3.5.A `ctx.windows[wid].tree`).
- **Toplevel(wid)** — per-window engine; `Cycle` body walks "the tree we own".
- **SubView{window_id, outer_panel_id, rest}** — engine on a panel inside an `emSubViewPanel`.
- **scope-based** — engine carries a `PanelScope` field; classification depends per instance on the registering panel's scope (Toplevel vs SubView).

## Production engines

| Engine | File | Line | Current registration | Post-3.5.A PanelScope | Cycle-body summary |
|---|---|---|---|---|---|
| `PanelCycleEngine` | crates/emcore/src/emPanelCycleEngine.rs | 42 | Carries `self.scope: PanelScope` (Phase 1.5 migrated) | **scope-based** (Toplevel *or* SubView per panel's scope) | Takes panel behavior off the tree via `ctx.tree.take_behavior`, drives its `Cycle`, reinstalls. Uses `ctx.tree.cached_pixel_tallness`. |
| `InputDispatchEngine` | crates/emcore/src/emInputDispatchEngine.rs | 18 | `TreeLocation::Outer` (emGUIFramework) | **Framework** | Drains `ctx.take_pending_inputs()`, resolves `(wid, event)` via `ctx.windows`, calls `win.dispatch_input(tree, ...)`. Tree access today is the legacy single-tree; post-3.5.A routes through `windows[wid].tree`. |
| `MiniIpcEngine` | crates/emcore/src/emMiniIpc.rs | 322 | `TreeLocation::Outer` (emMiniIpc.rs:364) | **Framework** | FIFO polling on `timer_signal`; calls `inner.poll()`. No `ctx.tree` / `ctx.windows` access. |
| `emWindowStateSaver` | crates/emcore/src/emWindowStateSaver.rs | 237 | Registered per-window at window creation | **Framework** | Reads `ctx.windows.get(&self.window_id)` for focus/geometry; saves config. No `ctx.tree` access. |
| `PriSchedEngine` | crates/emcore/src/emPriSchedAgent.rs | 41 | `TreeLocation::Outer` | **Framework** | Scans internal `PriSchedModelInner.agents`; no `ctx` access at all. |
| `UpdateEngineClass` | crates/emcore/src/emView.rs | 199 | Carries `scope: PanelScope` (already migrated) | **scope-based** — unchanged | Matches on `self.scope`: Toplevel → resolves `ctx.windows.get_mut(wid)` then `view.Update(ctx.tree, ...)`; SubView → resolves sub-view via `ctx.tree.panels[pid]` then `sub_view.Update(sub_tree, ...)`. |
| `VisitingVAEngineClass` | crates/emcore/src/emView.rs | 268 | Carries `scope: PanelScope` (already migrated) | **scope-based** — unchanged | Computes `dt`; resolves view/sub-view the same way as `UpdateEngineClass`; forwards to `VisitingVA.animate(view, tree, dt, sc)`. |
| `EOIEngineClass` | crates/emcore/src/emView.rs | 355 | Registered via `emView`; `TreeLocation::Outer` | **Framework** *(plan starting point said Toplevel; reclassified — see note)* | Decrements `self.CountDown`; when ≤ 0, calls `ctx.fire(self.eoi_signal)`. No `ctx.tree` / `ctx.windows` dereference. Per the classification invariant ("Framework iff Cycle does not touch ctx.tree"), this is Framework. |
| `StartupEngine` | crates/emmain/src/emMainWindow.rs | 453 | Registered by `create_main_window` at window-build time | **Toplevel(wid)** | 11-state startup sequence. Heavy `ctx.tree.create_child`, `ctx.tree.with_behavior_as::<emMainPanel,_>`, `ctx.tree.with_behavior_as::<emSubViewPanel,_>`, `ctx.tree.take_behavior` / `put_behavior`, `ctx.tree.remove`. Post-3.5.A expects `windows[wid].tree`. |
| `MainWindowEngine` | crates/emmain/src/emMainWindow.rs | 345 | Registered per main window | **Framework** | Polls close / title / reload signals; `ctx.windows.get(&wid)` for title update; fires `file_update_signal`. No `ctx.tree`. |
| `ControlPanelBridge` | crates/emmain/src/emMainWindow.rs | 752 | Registered per main window | **Framework** | Single `ctx.IsSignaled` probe; `log::debug!`. No `ctx.tree` / `ctx.windows` access. |
| `emStocksPricesFetcher` | crates/emstocks/src/emStocksPricesFetcher.rs | 473 | Registered by caller (stocks plugin) | **Framework** | Returns `self.current_process_active`. `_ctx` unused — the real fetch driving happens via a separate direct `Cycle(&mut rec)` method (comment at line 475-478). |
| `DialogPrivateEngine` | crates/emcore/src/emDialog.rs | 362 (impl `#[cfg(test)]`) | Task 4 test paths register at Outer; production registration deferred (see plan §B3.5a.f) | **Toplevel(dialog_window_id)** (once production impl lands) | Detaches `DlgPanel` behavior via `ctx.tree.take_behavior(root_panel_id)`, drives private logic, reinstalls. Heavy tree coupling. NOTE: the `impl emEngine` is currently under `#[cfg(test)]` — the engine only appears in the scheduler from test code at this commit. |

## Test engines

### emcore unit tests

| Engine | File | Line | Post-3.5.A PanelScope | Rationale |
|---|---|---|---|---|
| `CountingEngine` | crates/emcore/src/emScheduler.rs | 760 | **Framework** | `_ctx` unused; increments `count`. |
| `PollingEngine` | crates/emcore/src/emScheduler.rs | 772 | **Framework** | `_ctx` unused; decrements counter. |
| `OrderEngine` (site 1) | crates/emcore/src/emScheduler.rs | 875 | **Framework** | `_ctx` unused; pushes label. |
| `CheckSignalEngine` | crates/emcore/src/emScheduler.rs | 922 | **Framework** | Reads `ctx.IsSignaled(...)`; no tree access. |
| `OrderEngine` (site 2) | crates/emcore/src/emScheduler.rs | 989 | **Framework** | `_ctx` unused; pushes label. |
| `FiringEngine` (site 1) | crates/emcore/src/emScheduler.rs | 1039 | **Framework** | Calls `ctx.fire`; no tree access. |
| `ReceivingEngine` | crates/emcore/src/emScheduler.rs | 1050 | **Framework** | `_ctx` unused; pushes "B". |
| `FiringEngine` (site 2) | crates/emcore/src/emScheduler.rs | 1097 | **Framework** | Calls `ctx.fire`; no tree access. |
| `HighEngine` | crates/emcore/src/emScheduler.rs | 1108 | **Framework** | `_ctx` unused. |
| `ProbePointerEngine` | crates/emcore/src/emScheduler.rs | 1237 | **Toplevel(WindowId::dummy())** *(plan starting point said Framework; reclassified — see note)* | Captures `ctx.tree as *mut PanelTree`. This is a tree dereference — the whole point of the test is to verify that a SubView-located engine receives the correct tree. Any non-Some tree assignment post-3.5.A would break the test. |
| `ChildSpawnEngine` | crates/emcore/src/emPanelTree.rs | 3416 | **Toplevel(WindowId::dummy())** | Calls `ctx.tree.create_child(self.parent, ..., Some(ctx.scheduler))`. |
| `SpawnEngineWithProbe` | crates/emcore/src/emPanelTree.rs | 3867 | **Toplevel(WindowId::dummy())** | Calls `ctx.tree.create_child(...)`, `ctx.tree.GetRec(...)`. |
| `NoopEngine` (emSubViewPanel) | crates/emcore/src/emSubViewPanel.rs | 519 | **Framework** | `_ctx` unused; returns false. |
| `NoopEngine` (emEngineCtx) | crates/emcore/src/emEngineCtx.rs | 774 | **Framework** | `_ctx` unused; returns false. |
| `FinishProbe` | crates/emcore/src/emDialog.rs | 1190 | **Framework** | `_ctx` unused; bumps `*self.hits`. |

### emfileman unit tests

| Engine | File | Line | Post-3.5.A PanelScope | Rationale |
|---|---|---|---|---|
| `NoopEngineForTest` | crates/emfileman/src/emFileManControlPanel.rs | 567 | **Framework** | `_ctx` unused. |

### eaglemode integration tests

| Engine | File | Line | Post-3.5.A PanelScope | Rationale |
|---|---|---|---|---|
| `SignalFiringEngine` | crates/eaglemode/tests/integration/signals.rs | 18 | **Framework** | `ctx.fire(sig)`; no tree access. |
| `CounterEngine` | crates/eaglemode/tests/integration/signals.rs | 37 | **Framework** | `_ctx` unused. |
| `FlagEngine` | crates/eaglemode/tests/integration/signals.rs | 107 | **Framework** | `_ctx` unused. |
| `DummyEngine` | crates/eaglemode/tests/integration/lifecycle.rs | 86 | **Framework** | `_ctx` unused. |

### eaglemode unit tests

| Engine | File | Line | Post-3.5.A PanelScope | Rationale |
|---|---|---|---|---|
| `RecordingEngine` | crates/eaglemode/tests/unit/scheduler.rs | 40 | **Framework** | `_ctx` unused; records label. |
| `FiringEngine` | crates/eaglemode/tests/unit/scheduler.rs | 184 | **Framework** | `ctx.fire`; no tree. |
| `CheckSignalEngine` | crates/eaglemode/tests/unit/scheduler.rs | 234 | **Framework** | `ctx.IsSignaled`; no tree. |

### eaglemode golden tests

| Engine | File | Line | Post-3.5.A PanelScope | Rationale |
|---|---|---|---|---|
| `RecordingEngine` | crates/eaglemode/tests/golden/scheduler.rs | 44 | **Framework** | `_ctx` unused; records label. |
| `MultiSigEngine` | crates/eaglemode/tests/golden/scheduler.rs | 393 | **Framework** | `ctx.IsSignaled` only; no tree. |

### examples

| Engine | File | Line | Post-3.5.A PanelScope | Rationale |
|---|---|---|---|---|
| `CounterEngine` | examples/signal_timer_demo.rs | 42 | **Framework** | `ctx.IsSignaled` only; no tree. |

## Aggregate counts

- Production engines: 12 (PanelCycleEngine, InputDispatchEngine, MiniIpcEngine, emWindowStateSaver, PriSchedEngine, UpdateEngineClass, VisitingVAEngineClass, EOIEngineClass, StartupEngine, MainWindowEngine, ControlPanelBridge, emStocksPricesFetcher). DialogPrivateEngine counts as a 13th *production* struct but its `impl emEngine` is currently `#[cfg(test)]`; it is listed in the production table because its production use is imminent (plan §B3.5a.f).
- Test engines: 27 (16 in emcore src + 1 in emfileman + 4 in eaglemode integration + 3 in eaglemode unit + 2 in eaglemode golden + 1 in examples).
- **Framework**: 8 production Framework engines (InputDispatchEngine, MiniIpcEngine, emWindowStateSaver, PriSchedEngine, EOIEngineClass, MainWindowEngine, ControlPanelBridge, emStocksPricesFetcher — including `EOIEngineClass` reclassified from Toplevel, see §Deviations 1) + 24 test = 32 Framework.
- **Toplevel**: StartupEngine + DialogPrivateEngine (test-gated) among production = 2; test: ProbePointerEngine, ChildSpawnEngine, SpawnEngineWithProbe = 3. Total 5 Toplevel.
- **scope-based** (Toplevel or SubView per instance): PanelCycleEngine, UpdateEngineClass, VisitingVAEngineClass = 3 production.
- **Total impl emEngine sites counted:** 39 (12 production + 27 test). Matches `rg -n 'impl [\w:]*emEngine[\w:]* for' crates/ examples/` output at commit 75ae0428.

## Deviations from the plan's starting-point tables

1. **EOIEngineClass: Toplevel → Framework.** Plan (line 236) classified as Toplevel on the rationale that the registering `emView` provides its window. The Cycle body at emView.rs:355-366 does not touch `ctx.tree` or `ctx.windows` — only `self.CountDown` and `ctx.fire(self.eoi_signal)`. Per the Framework invariant (Cycle body does not access `ctx.tree`), this is Framework. Reclassifying as Framework avoids the Task 6 migration rule mandating `ctx.tree.as_deref_mut().expect(...)` in a body that would immediately panic.

2. **ProbePointerEngine: Framework → Toplevel.** Plan (line 249) classified as Framework ("Test; no tree"). The Cycle body at emScheduler.rs:1237-1242 stores `ctx.tree as *mut PanelTree` — that is a tree dereference; the test's entire purpose is to assert the pointer identity of the sub-view tree it receives. Classifying as Framework post-3.5.A would leave `ctx.tree = None` and break the test. Reclassifying as Toplevel; the test's registered location is actually `TreeLocation::SubView { ... }` (line 1250), so at migration time the concrete variant should follow the test's registration — SubView. Noting here as Toplevel for the general "touches tree" rule; the test-specific SubView retargeting is a Task 6 detail.

3. **New engines not in plan tables, added to production sheet:** `StartupEngine` (emmain/src/emMainWindow.rs:453), `MainWindowEngine` (line 345), `ControlPanelBridge` (line 752), `emStocksPricesFetcher` (emstocks). Plan's table stopped at emcore production engines; these emmain/emstocks engines exist and must be classified for Task 6 coverage.

4. **New engines not in plan tables, added to test sheet:** `NoopEngineForTest` (emfileman), all four `eaglemode/tests/integration/signals.rs` + `lifecycle.rs` engines, three `eaglemode/tests/unit/scheduler.rs` engines, two `eaglemode/tests/golden/scheduler.rs` engines, and the `examples/signal_timer_demo.rs` CounterEngine.

5. **File paths corrected.** Plan's table used bare filenames (e.g. `emPanelCycleEngine.rs`); this sheet qualifies with crate path (`crates/emcore/src/...`) to disambiguate emmain/emcore/emstocks engines. Line numbers verified against `rg` output at HEAD 75ae0428.

6. **DialogPrivateEngine impl is `#[cfg(test)]` at this commit.** Plan (line 233) lists it as a production Toplevel engine. The struct is production; the `impl emEngine for DialogPrivateEngine` at emDialog.rs:361 carries `#[cfg(test)]`. Production registration is deferred per plan bootstrap §B3.5a.f. Sheet keeps it in the production table with that caveat, because Task 6 migration will target it as Toplevel.

## Classification invariants

- A **Framework** engine's `Cycle` body MUST NOT call `ctx.tree.as_deref_mut().expect(...)` (post-3.5.A `ctx.tree` is `None` for Framework-dispatched engines). During Task 6, any Framework-classified engine whose body dereferences `ctx.tree` is a misclassification — re-audit before committing.
- A **Toplevel**-scoped engine's `Cycle` body MUST resolve its tree via `ctx.tree.as_deref_mut().expect("window-scoped engine: tree is Some")` (or equivalent `windows[wid].tree` pass-through per Task 6's adopted pattern).
- **SubView** engines follow the existing `PanelCycleEngine` / `UpdateEngineClass` pattern: resolve by walking `ctx.tree.panels[outer_panel_id]` (where `ctx.tree` is the owning window's tree) down through the `SubView` rest chain.
- **scope-based** engines (PanelCycleEngine, UpdateEngineClass, VisitingVAEngineClass) dispatch on `self.scope` internally; Task 6 migration is in the dispatch layer (EngineScheduler), not in these engines' bodies.

## Re-audit triggers

Revisit this sheet, in the same commit as the precipitating change, when any of these occur:

- A new `impl emEngine for` site is introduced anywhere in the workspace.
- An existing `impl emEngine for` body starts or stops dereferencing `ctx.tree`.
- A `register_engine` call-site moves an existing engine's registration between `Outer` / per-window / `SubView` locations.
- Any reclassification decision documented above (§Deviations 1-6) is overturned by new evidence from C++ or from runtime observation.

## Cross-check: register_engine call sites

`rg -n '\.register_engine\(' crates/ examples/` at HEAD 75ae0428: **69 call sites.** Spot-check confirms every first-arg `Box::new(SomeEngine { ... })` corresponds to one of the 39 types enumerated above. (Multiple call sites per type are common — e.g., `PanelCycleEngine` is registered once per panel via `emPanel::SetEngine`.)
