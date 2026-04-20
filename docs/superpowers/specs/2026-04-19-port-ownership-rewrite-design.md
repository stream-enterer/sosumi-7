# Port Ownership Rewrite — Overarching Design Spec

**Date:** 2026-04-19.
**Source document:** `docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json` (37 entries, 10 buckets, `commit_sha: 06d4e5a`).
**Scope:** The overarching architectural decisions that, taken together, resolve every open JSON entry, close the `Rc<RefCell<EngineScheduler>>` smuggled-idiom cascade at its root, re-establish observational-port discipline across the whole tree, and amend CLAUDE.md to prevent the smuggle from recurring. This spec will spawn multiple implementation plans; it does not itself implement anything.

**Supersedes for ownership purposes:**
- CLAUDE.md §"Code Rules" line 47 (`Ownership: Rc/RefCell shared state, Weak parent refs`) — replaced by §3 below.
- `2026-04-19-scheduler-refcell-workaround-ledger.md` §6.5 decision criteria — upgraded from "re-evaluate when X" to "execute per §10".
- `2026-04-18-emview-sp4-update-engine-routing-design.md` §35 — the phrase "idiom adaptation forced by RefCell<EngineScheduler>" is rejected as incoherent under the new framework; the workaround machinery it introduced is scheduled for deletion in Phase 1.

---

## 1. Goal and non-goals

**Goal.** Produce the architectural decisions from which every resolution falls out, so future contributors do not need to re-derive them per case. Specifically:

1. Close the `Rc<RefCell<EngineScheduler>>` cascade by removing the first-order choice that created it, not by fixing each downstream site.
2. Establish a disciplined ownership model that, applied uniformly, prevents future cascades.
3. Make the forced-divergence perimeter operational — a single test-of-forcedness resolves any new case to one of six verdicts without ambiguity.
4. Bind every open JSON entry to the design section that resolves it; no item is deferred, out-of-scope, or future-work.
5. Amend CLAUDE.md to reflect the new model; preserve all ported knowledge by rewriting (not deleting) in-code rationales at affected sites.

**Non-goals.** This spec does not:
- Write code. It writes the principles. Plans will write the code.
- Commit to specific commit sequences. Phases are described in work-units, not commit boundaries.
- Rewrite C++ emCore. The port's authority order (C++ > goldens > Rust idiom) stands.

---

## 2. Principles

Six principles. Every resolution in this spec, and every future resolution under this framework, is a consequence of one or more of these.

### P1 — Event-loop-threaded mutable state is the default

The framework owns mutable state as plain values and threads `&mut` references through each tick. This is the model C++ uses (with raw pointers), translated to Rust's aliasing discipline.

**Concrete rule.** `emGUIFramework` owns `scheduler: EngineScheduler`, `windows: HashMap<WindowId, emWindow>`, and `root_context: emContext` as plain values. Each tick it passes `&mut` through `DoTimeSlice` into `EngineCtx`, which exposes the full scheduler API (create_signal, register_engine, remove_engine, fire, wake_up, connect, disconnect, remove_signal) as ctx methods. Any code reached from inside `emEngine::Cycle` that wants to touch the scheduler does so through ctx, inline, synchronously.

**What this replaces.** The `Rc<RefCell<EngineScheduler>>` + `DoTimeSlice takes borrow_mut()` pattern. The SchedOp enum, pending_sched_ops queue, queue_or_apply_sched_op entry point, five drain sites, register_pending_engines catch-up sweep, close_signal_pending cache, SVPUpdSlice try_borrow fallback, and per-sub-view schedulers — all deleted.

**Observational argument.** C++ `emScheduler` is reached re-entrantly from `Cycle` and observable what-fires-when is defined by the call order. Rust `&mut EngineScheduler` threaded through ctx is reached re-entrantly from `Cycle` in the same call order. The observable surface is preserved and strengthened: SP4.5-FIX-3's +1 slice drift becomes delta=0 by construction; SP4.5-FIX-2's latent re-entrant-borrow panic cannot occur.

### P2 — Single-owner composition is the default for tree-shaped state

Views, panel trees, contexts, and their descendants are owned by their structural parents as plain values, not shared through `Rc<RefCell<>>`. Back-references use IDs resolved through ctx, not `Weak<RefCell<>>`.

**Concrete rule.** `emWindow` owns `view: emView`. `emSubViewPanel` owns `sub_view: emView` and `sub_tree: PanelTree`. `emView` owns its ports (`HomeViewPort`, `CurrentViewPort`, `DummyViewPort`) as plain values. `PanelCycleEngine` holds `(window_id, panel_id)` and looks up its view via `ctx.view_mut(window_id)`; no `Weak<RefCell<emView>>` field. Every place that currently holds `Rc<RefCell<emView>>` or `Weak<RefCell<emView>>` is rewritten to an ID plus ctx-resolution, or inline `&mut` borrow through the owning window.

**What this replaces.** 18 `Rc<RefCell<emView>>` sites (SP5-era), 5 `Weak<RefCell<emView>>` back-refs, the `view_rc()` accessor on emWindow and emSubViewPanel, and the tangle of borrow-ordering hazards that forced `self.CoreConfig.borrow()` to run before `self.VisitingVA.borrow_mut()` (SP3 plan note, E038).

**Observational argument.** Ownership by composition mirrors C++ where `emView` is a subobject of the owning `emWindow`. The id+ctx pattern restores the C++ raw-pointer lookup shape. Borrow-ordering hazards (E038) disappear because there is no RefCell chain through which to re-borrow; all access goes through a single `&mut` path.

### P3 — `Rc<RefCell<T>>` is a justified exception, not the default

`Rc<RefCell<T>>` is admissible only where (a) the value must be referenced across closure boundaries held by an external library (notably winit callbacks that can't receive `&mut framework`), or (b) the value is a typed singleton registered in a context registry for dynamic lookup (e.g., `emClipboard`, future plugin models) and is not reached re-entrantly during Cycle. Nothing else qualifies.

**Concrete rule.** Every remaining `Rc<RefCell<T>>` in the tree after migration must be justified by a same-file comment citing (a) or (b) above. The comment is machine-checked by a lint added in Phase 5 (§10.5).

**What this replaces.** The blanket "Rust: Rc/RefCell shared state" policy from CLAUDE.md:47 and `emRef.no_rs`. The current **284** `Rc<RefCell<T>>` declarations (measured 2026-04-19; the spec was originally drafted against a stale 156-site count and the figure has been re-baselined) are migrated to:
- **Plain values** where the type has a single owner (most cases — views, panel trees, scheduler internals, config panels holding config-copy state).
- **`Rc<T>`** (immutable) where the type is shared-immutable (contexts post-init, look/theme data).
- **`Rc<RefCell<T>>` retained** only at the justified cross-closure or context-registry sites. Expected remaining count proportional to original target: ≤ 60, all chartered (§3.6). Phase 5 closeout enforces the ≤60 ceiling; intermediate phases must monotonically reduce `rc_refcell_total`.

**Observational argument.** RefCell interior mutability has no observable semantics of its own; it is purely a borrow-check accommodation. Removing it where unjustified cannot change observable behavior. Retaining it only where structurally required preserves the cases it was introduced to handle.

### P4 — Forced divergence is operationally defined

A divergence is *forced* iff one of the following passes its explicit test. Anything else is either idiom-adaptation (if below the observable surface) or a fidelity-bug (if on the observable surface).

1. **Language-forced.** The C++ shape uses a language feature Rust does not have: single inheritance with virtual methods, name/arity overloading, implicit conversion operators, mutable methods on value types with compiler-generated copy semantics, template-specialization ADL. Test: try writing the C++ shape in Rust using the ownership model of P1–P3; if it does not compile, it is language-forced. If it compiles and passes goldens, it is not forced.
2. **Dependency-forced.** A required dependency (wgpu or winit) imposes a shape C++ does not impose. Test: can the dependency's public API admit the C++ shape directly? If not, dependency-forced. Currently: winit's async surface lifecycle, absence of X11 display handle.
3. **Upstream-gap-forced.** C++ emCore itself ships the shape as a no-op or stub. Test: read the C++ implementation; if all platform backends are no-ops, upstream-gap.
4. **Performance-forced.** The C++ shape ported directly degrades measurable performance below an observable threshold. Test: benchmark both shapes on a golden-test-exercised path; if the C++-mirrored shape exceeds a documented threshold, performance-forced. Threshold must be stated in the site-specific DIVERGED block with the measurement.

Every other divergence is either **idiom** (below surface, no observable change, no structural commitment) or **fidelity-bug** (must be fixed to restore observational equivalence). There is no "idiom adaptation forced by X" verdict; that phrasing is explicitly rejected as the smuggling vehicle.

**Concrete rule.** Every `DIVERGED:` block must name which of the four forced-verdicts applies and cite the test that establishes it. Blocks that name none of the four are idiom or fidelity-bugs — and idiom is not annotated with `DIVERGED:` (see P5). Phase 5 adds lint enforcement.

**What this replaces.** The current ad-hoc classification where "Rust has no X" was treated as forced even when X had a Rust encoding. Specifically: every "Rust has no inheritance" that actually was inheritance-collapse (B03) stays forced. Every "Rust's RefCell forbids inner borrows" downstream of the scheduler choice (E001, E005, E006, E012) is reclassified once Phase 1 removes the first-order choice — the downstream sites cease to exist.

### P5 — Annotation vocabulary: `DIVERGED:` is forced only; `RUST_ONLY:` is chartered only

`DIVERGED:` marks a forced divergence per P4. Nothing else. Below-surface idiom-adaptations (`emColor::SetRed` returning a new value because `emColor: Copy`; `emArray::Add_one` because Rust has no overloading) require a DIVERGED tag because overloading and Copy-mutation are P4.1 language-forced. But not every renamed method is "forced"; some are cleanup, and cleanup should be unannotated.

`RUST_ONLY:` marks code with no C++ analogue, with a chartered justification: either (a) language-forced utility (e.g., newtype to prevent fixed/int mixing — Rust has no way to express this in C++'s bare-int style safely; Fixed12 is justified), or (b) performance-forced alternative (with benchmark in the annotation), or (c) dependency-forced (wgpu/winit). Debug traces and test-only helpers are not chartered and must be `#[cfg(debug_assertions)]` or `#[cfg(test)]` gated without an annotation.

`IDIOM:` is deleted from the vocabulary. The sole current use (E001, `emView.rs:186`) is reclassified as smuggled-idiom by P4 and resolved by Phase 1.

`UPSTREAM-GAP:` is preserved with no change; C++-ships-as-no-op is a distinct category.

`BLOCKED:` and `TODO:` are preserved but phased out in favor of issue-tracker references by Phase 5.

**What this replaces.** The current vocabulary where `DIVERGED:` is used for both forced divergences and below-surface adaptations that happen to be named differently; the current state where `RUST_ONLY:` includes debug aids, widget helpers, and asset embeds indiscriminately.

### P6 — Migration is total; workarounds are deleted in the same phase that obsoletes them

When a phase lands a new ownership shape, the workaround machinery that shape makes unnecessary is deleted in the same phase. No compat shims. No hybrid state. No "retained for backwards compatibility" (there is no external API surface with a back-compat contract — the library is an in-tree port).

**What this replaces.** The current pattern (from §3.3 of the emView closeout) of layering new mechanisms on top of old — `SchedOp` atop `borrow_mut()`, `register_pending_engines` atop `register_engine_for`, per-sub-view scheduler atop the single scheduler — without deleting the layer underneath. Phase 1 ships as a single cliff: SchedOp and all 12 B01 mechanisms disappear together when the event-loop-threaded model lands.

---

## 3. The new ownership model — concrete shapes

This section specifies the canonical Rust shapes for each C++ ownership pattern. The migration plans derive from these.

### 3.1 Framework-owned roots

```rust
pub struct emGUIFramework {
    pub scheduler: EngineScheduler,                        // was: Rc<RefCell<…>>
    pub windows: HashMap<WindowId, emWindow>,              // was: HashMap<WindowId, Rc<RefCell<emWindow>>>
    pub root_context: Rc<emContext>,                        // Rc<T>: see §3.4 for emContext's narrow interior mutation
    pub clipboard: RefCell<Option<Box<dyn emClipboard>>>,  // chartered §3.6(a): mutated from winit callbacks without &mut framework access
    pub framework_actions: Vec<DeferredAction>,             // drained by framework after DoTimeSlice; used to close windows, etc.
    pub pending_inputs: Vec<(WindowId, InputEvent)>,        // §4 D4.9: input queue drained by InputDispatchEngine each slice
    pub context_winit: EventLoop<UserEvent>,                // winit-owned
}
```

The core design decision (Q1 from the original draft — lifted to §3.1.1 concrete):

**§3.1.1 The `EngineCtx` disjoint-borrow shape.**

`EngineCtx` is a *borrow bundle*, not a state container. Its critical property is that view access and scheduler access can be held simultaneously, which requires them to live in disjoint fields of ctx that the borrow checker can split. The naïve `fn view_mut(&mut self, wid) -> Option<&mut emView>` is unsound because the returned `&mut emView` re-borrows ctx through `&mut self`, blocking any further ctx use for the view's lifetime. The correct shape is closure-based disjoint borrowing, where `with_view_mut` constructs a new `SchedCtx` from ctx's non-windows fields and hands it to the closure alongside the view reference:

```rust
pub struct EngineCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub windows: &'a mut HashMap<WindowId, emWindow>,
    pub root_context: &'a Rc<emContext>,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub engine_id: EngineId,  // always populated: EngineCtx exists only during DoTimeSlice dispatch
}

/// Disjoint sub-view of EngineCtx that excludes `windows`. Built by
/// `EngineCtx::with_view_mut` so code inside a view-access closure can
/// still touch the scheduler without re-borrowing ctx's windows field.
pub struct SchedCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
    pub current_engine: Option<EngineId>,
}

impl<'a> EngineCtx<'a> {
    pub fn with_view_mut<R>(
        &mut self,
        window: WindowId,
        f: impl FnOnce(&mut emView, &mut SchedCtx<'_>) -> R,
    ) -> Option<R> {
        let win = self.windows.get_mut(&window)?;
        let mut sched = SchedCtx {
            scheduler: &mut *self.scheduler,
            framework_actions: &mut *self.framework_actions,
            root_context: self.root_context,
            current_engine: Some(self.engine_id),
        };
        Some(f(&mut win.view, &mut sched))
    }

    pub fn with_window_mut<R>(
        &mut self,
        window: WindowId,
        f: impl FnOnce(&mut emWindow, &mut SchedCtx<'_>) -> R,
    ) -> Option<R> { … }

    pub fn framework_action(&mut self, a: DeferredAction) { self.framework_actions.push(a); }
}

impl SchedCtx<'_> {
    pub fn create_signal(&mut self) -> SignalId { self.scheduler.create_signal() }
    pub fn register_engine(&mut self, e: Box<dyn emEngine>, pri: Priority) -> EngineId { … }
    pub fn remove_engine(&mut self, id: EngineId) { … }
    pub fn fire(&mut self, sig: SignalId) { … }
    pub fn wake_up(&mut self, eng: EngineId) { … }
    pub fn connect(&mut self, sig: SignalId, eng: EngineId) { … }
    pub fn disconnect(&mut self, sig: SignalId, eng: EngineId) { … }
    pub fn remove_signal(&mut self, sig: SignalId) { … }
}

// EngineCtx also exposes the same scheduler API directly for engines that
// don't need view access. Implementation: re-borrow self.scheduler; safe
// because when ctx.fire() is called outside a with_view_mut block, no
// view reference is live.
impl EngineCtx<'_> {
    pub fn create_signal(&mut self) -> SignalId { self.scheduler.create_signal() }
    pub fn fire(&mut self, sig: SignalId) { self.scheduler.fire(sig) }
    // etc — same surface as SchedCtx
}
```

**Soundness.** Inside `with_view_mut`'s closure, `view` and `sched` are two distinct `&mut` references derived from field-disjoint re-borrows of `*self`. Rust's NLL admits this (splitting `&mut self.windows` away from `&mut self.scheduler` + `&mut self.framework_actions`). Calling `sched.fire(sig)` while `view` is live compiles and runs.

**The engine-in-slot pattern is preserved.** `EngineScheduler::DoTimeSlice` still uses the SP4 take/put pattern: when an engine's `Cycle` runs, its `behavior: Option<Box<dyn emEngine>>` is `None` in its slot. `ctx.scheduler` exposes the full public API; calling `ctx.register_engine(other)` or `ctx.remove_engine(other_id)` during Cycle reaches back into `self.inner.engines` but never conflicts with the *current* engine's slot (which is already emptied). Self-removal (`ctx.remove_engine(self.engine_id)`) is admissible because the slot's `behavior: None` means no double-drop.

**`set_engine_priority(self.engine_id, ...)` during own Cycle** is preserved from current semantics: scheduler's priority-table mutation touches `self.inner.engines.get_mut(id)` which has `behavior: None` and remains a valid slot reference. No new hazard.

Priority re-ascent is unchanged — it's `EngineCtxInner::wake_up` behaviour, now reachable inline via `ctx.wake_up(new_eid)`.

**2026-04-19 reconciliation:** spec previously had `current_engine: Option<EngineId>` on `EngineCtx`. Implementation (Chunk 1, `c402cf6`) established that `EngineCtx` is constructed only during engine dispatch; the `Option` never has a `None` state. Field renamed to `engine_id: EngineId` and spec updated to match. `SchedCtx::current_engine` retains `Option<EngineId>` because `SchedCtx` is also reached through `InitCtx` paths at framework boot, where no engine is dispatching.

### 3.2 Window-owned views

```rust
pub struct emWindow {
    pub view: emView,                          // was: Rc<RefCell<emView>>
    pub active_animator: Option<Box<dyn emViewAnimator>>,
    pub surface: OsSurface,                    // Pending/Materialized — dependency-forced
    // all other fields plain
}
```

Dispatch paths:
- `App::window_event` holds `&mut emGUIFramework`; it indexes `framework.windows.get_mut(&window_id)`, receives `&mut emWindow`, then `&mut window.view`. Input dispatch runs under a single `&mut emView` for the whole event.
- `emWindow::dispatch_input` becomes a method taking `&mut self, ctx: &mut EngineCtx<'_>`. The active animator and the view are reached by field access. The input dispatch runs inside a `DoTimeSlice` slice so ctx is available for any scheduler touch.

`emViewPort` likewise becomes owned by `emView` as plain values (three of them: Home, Current, Dummy). The back-reference to `emWindow` becomes a `window_id: Option<WindowId>` resolved via ctx. `SwapViewPorts` becomes a `std::mem::swap` on two `&mut emViewPort` borrows obtained from distinct fields of `emView` — compile-checked disjoint.

### 3.3 Sub-views

`emSubViewPanel` owns `sub_view: emView` and `sub_tree: PanelTree` as plain values. Its `PanelBehavior::Cycle` receives `(&mut self, ectx: &mut EngineCtx<'_>, pctx: &mut PanelCtx<'_>)`.

**The `PanelCtx` split.** The existing `EngineCtx` in the current code conflates scheduler access with tree+panel access. This spec separates them:

```rust
pub struct PanelCtx<'a> {
    pub tree: &'a mut PanelTree,
    pub current_panel: PanelId,
}
```

Engines that touch only the scheduler (`UpdateEngineClass`, `VisitingVAEngine`, `StartupEngine`, plugin loader engines, image-loader engines) take only `&mut EngineCtx`. Engines that walk a panel tree (`PanelCycleEngine`, callbacks reached through panel behaviors) take both. The split is field-disjoint so both can be held simultaneously; `PanelCtx::tree` is re-borrowed from a `&mut PanelTree` handed in separately from the scheduler.

The `PanelCtx` struct lives in `emEngineCtx.rs` (Phase 1.75 Task 5 absorbed the previously-separate `emPanelCtx.rs` module into `emEngineCtx.rs`; `emPanelCtx.rs` is deleted).

**Unified cross-tree dispatch (Phase 1.75).** There is a single `EngineScheduler` in the process (owned by `emGUIFramework`). Every engine — outer-tree and sub-tree alike — registers with that scheduler, and `EngineScheduler::DoTimeSlice` walks one priority queue containing all awake engines, regardless of which tree they live in.

The observable invariant this preserves: **any invariant C++ has about "outer and sub-view engines at priority P interleave in priority order within a slice" holds in Rust too — because they share the scheduler.** (SP8's per-sub-view scheduler, now deleted in Phase 1.75 Task 4, was the structural divergence that violated this; before its deletion, sub-views had their own clock and the interleave observability was lost.)

The Rust-side mechanism that makes one-queue cross-tree dispatch work under strict-ownership `PanelTree`s:

1. Each engine carries a `TreeLocation` stored in `EngineScheduler::engine_locations: SecondaryMap<EngineId, TreeLocation>`, populated at registration time:

   ```rust
   pub enum TreeLocation {
       Outer,
       SubView { outer_panel_id: PanelId, rest: Box<TreeLocation> },
   }
   ```

   `TreeLocation::Outer` tags engines living in the outer `PanelTree`. `TreeLocation::SubView { outer_panel_id, rest }` tags engines living inside the sub-tree of the `emSubViewPanel` at `outer_panel_id` in the outer tree; `rest` is the location *within* that sub-tree (`Outer` for a direct child, nested `SubView` for sub-views-inside-sub-views).

2. `PanelBehavior` gains one trait method with a `None` default:

   ```rust
   fn as_sub_view_panel_mut(&mut self) -> Option<&mut emSubViewPanel> { None }
   ```

   Only `emSubViewPanel` overrides it (returns `Some(self)`). No `Any`, no downcasting, no per-impl boilerplate for the ~50 non-sub-view `PanelBehavior` impls.

3. Per-dispatch, `DoTimeSlice` resolves an engine's home tree by walking its `TreeLocation` through a nested take/put of outer-tree panel behaviors (`dispatch_with_resolved_tree` in `emScheduler.rs`):

   ```text
   TreeLocation::Outer
       → f(outer_tree)
   TreeLocation::SubView { outer_panel_id, rest }
       → take_behavior(outer_panel_id)
       → behavior.as_sub_view_panel_mut().sub_tree_mut()  // reach inner tree
       → recurse with rest
       → put_behavior(outer_panel_id)
   ```

   The dispatcher `take`s the outer `emSubViewPanel`'s behavior from its slot (same mechanism the outer scheduler already uses when driving any panel's `Cycle`), downcasts via `as_sub_view_panel_mut` to reach `emSubViewPanel::sub_tree`, and continues the walk. Cost per sub-tree engine dispatch: one behavior-slot swap per `SubView` nesting level — identical to the take/put profile the outer scheduler already pays for outer panels.

4. With the home tree resolved to a concrete `&mut PanelTree`, `EngineCtx::tree` is populated and the engine's `Cycle` runs exactly as it would for an outer engine. Outer and sub-view engines at the same priority P fire in the order `DoTimeSlice` drains the priority-P queue — the interleave is literal, not simulated.

**Nested sub-views (sub-view inside sub-view).** Admissible by the same recursive walk: a `TreeLocation::SubView { outer_panel_id, rest: Box::new(SubView { outer_panel_id: inner_id, rest: Box::new(Outer) }) }` resolves two take/put levels deep. Depth bounded by actual nesting in the panel tree (typically ≤3 in practice; no architectural limit). Confirmed against C++ `emSubViewPanel::Cycle` which uses an equivalent nested-Cycle pattern.

**Phase-1.76 known departure — `PanelBehavior::Input`.** Task 5 found that threading the scheduler through `PanelBehavior::Input` cascades into >100 impls (every widget forwarding input to child widgets, test panels, etc.) — out of scope for Phase 1.75. As a result, the inner sub-view-panel mouse/touch dispatch path at `emSubViewPanel::Input` (`crates/emcore/src/emSubViewPanel.rs` around line 301) currently constructs a throwaway local `EngineScheduler` whose `wake_up` calls land on a dropped value (observationally no-ops). Observable impact is narrow: it only affects signal-driven reactions to sub-view input events that fire *solely* during input dispatch (not during the post-frame ctx), and the goldens held 237/6 across Tasks 3–6 with this silent-drop in place. Phase 1.76 will widen `PanelBehavior::Input` to receive a scheduler (or introduce an `InputWithCtx` sibling) and retire the throwaway.

### 3.4 Contexts

Contexts are a tree of `Rc<emContext>`. Each emContext has three pieces of interior mutable state, each with a distinct, narrow justification:

```rust
pub struct emContext {
    parent: Option<Weak<emContext>>,                    // Weak to avoid Rc cycle through children
    children: RefCell<Vec<Weak<emContext>>>,            // grows during NewChild
    models: RefCell<HashMap<TypeId, Rc<dyn Any>>>,      // written during register_model / acquire
    // clipboard: MOVED OUT. See emGUIFramework::clipboard in §3.1.
}
```

**Justification for the retained `RefCell`s.**
- `children`: mutated by `emContext::NewChild`. Typical path is framework init and plugin-load (before any Cycle). Lazy `NewChild` from inside a Cycle is permitted via the RefCell — no re-entrancy hazard because NewChild takes the children-list borrow, inserts one entry, and releases. Chartered under §3.6(b) "context-registry interior state."
- `models`: mutated by `emContext::register_model` and `acquire<T>()`. The typical case is plugin-init (before Cycle), but `Acquire`-from-Cycle is a real pattern (e.g., `emFpPluginList::Acquire` called from `emDirEntryPanel` behaviors). The RefCell supports both — chartered under §3.6(b). The "init-only" ideal is a discipline encouraged by documentation but not enforced; late Acquire is admissible.

**Clipboard moved out of emContext.** The original draft placed clipboard on the context. The critique (Finding 3) correctly identified that clipboard is mutated from Input handlers during Cycle (text-field Ctrl-C / Ctrl-V), so an interior-mutable slot on the context is a real re-entrancy vector.

Resolution: the clipboard lives on `emGUIFramework::clipboard: RefCell<Option<Box<dyn emClipboard>>>` (§3.1), reached via `ctx.root_context.get_clipboard()` which delegates to the framework through a thin accessor. During Cycle, ctx exposes `ctx.clipboard_mut()` (via `SchedCtx` or via `EngineCtx` directly) which borrows the framework's clipboard RefCell for the duration of the call. Because only one Input handler runs at a time within a slice, there is no re-entrant borrow hazard. Chartered under §3.6(a) "cross-closure" — the clipboard is mutated from winit text-event callbacks that don't hold `&mut framework`.

**Scheduler is not in emContext.** The current `scheduler: Option<Rc<RefCell<EngineScheduler>>>` field on emContext is deleted. Scheduler reached via ctx during Cycle, or via `&mut EngineScheduler` at framework level. This is a divergence from C++ `emContext::GetScheduler()` but a below-surface one: no caller observes *where* the scheduler comes from, only that it exists and behaves correctly. Accessor `emContext::GetScheduler()` is deleted; callers migrate to ctx-based access or direct framework reference.

**Plugin model registration — typical pattern and late-registration case.** Typical: plugins register their models at plugin-load time (before any `Cycle`). Late registration — `Acquire`-from-Cycle — is admissible; the retained `RefCell` on `models` supports it. The RefCell borrow is short (one insert), non-nested, and single-threaded, so re-entrancy is not a hazard in practice. If a future contributor writes an `Acquire` call chain that re-enters `register_model` while the map is already borrowed, the panic is immediate and local (standard RefCell panic), which is an acceptable tripwire rather than a silent failure. Plans should still prefer init-time registration where feasible.

Views hold `context: Rc<emContext>` (E016, preserved-forced composition). `GetContext`, `GetRootContext`, `LookupInherited<T>` are read-only accessors through the Rc chain (no RefCell touch).

### 3.5 Widget event model: signals, not callbacks

Widgets that expose events (emCheckButton::GetCheckSignal, emRadioButton::GetCheckSignal, emButton::GetClickSignal, text-field modification signals, etc.) allocate a `SignalId` at construction and fire it inline from their `Input` or `Handle` handler. The signal is allocated through ctx during the widget's creation path.

**Callback signature changes.** Existing callback fields currently have shapes like `Option<Box<dyn FnMut(bool)>>`. Under the new model, callbacks may need scheduler access (e.g., config-panel callbacks that call `config.SetValue(...)` which must fire a change-notification signal internally via emRec — §7). The callback signature therefore becomes:

```rust
pub type WidgetCallback<Args> = Box<dyn for<'a, 'b> FnMut(Args, &'b mut SchedCtx<'a>)>;

pub struct emCheckButton {
    pub check_signal: SignalId,
    pub on_check: Option<WidgetCallback<bool>>,
    // ...
}
```

The closure receives a `&mut SchedCtx` — disjoint-borrow sibling of the widget itself (which is reached via the outer view+tree path). This lets callbacks fire scheduler signals, mutate config state (via emRec), and queue framework actions while the widget remains borrowed up the stack.

**Migration scale.** ~40-50 existing callback install sites (grep: `on_\w+\s*=\s*Some(Box::new`) across `emCoreConfigPanel.rs` (largest cluster), `emScalarField.rs`, `emFileSelectionBox.rs`, `emRadioButton.rs`, `emTextField.rs`. Each site updates its closure to take the new signature. Most closure bodies that currently write into an `Rc<RefCell<>>` captured by `move` migrate to calling `sched.fire(cfg_signal)` after the config write — cleaner than the current shape.

**Both surfaces fire.** The widget's Input handler fires the signal first (`sched.fire(self.check_signal)`) then invokes `on_check` if present. Signal is the primary C++-fidelity surface; callback is the Rust ergonomic convenience. Applications can use either; tests prefer signal-based verification.

`B08` thereby closes on both axes: C++ observers wire via signal (full fidelity), ergonomic Rust users wire via callback with full scheduler access.

### 3.6 Cross-closure state (justified Rc<RefCell<>> sites)

**Charter categories.**
- (a) **Cross-closure:** value mutated from a closure held by an external library (winit, wgpu) that cannot receive `&mut framework` at its call site.
- (b) **Context-registry interior state:** state of a `Rc<emContext>` or an `Rc<emContext>`-registered model that is mutated at init time and then read thereafter; RefCell is the interior-mutability encoding of init.
- (c) **Shared mutable widget state** across sibling widgets that have no single owner (e.g., radio groups where N buttons share one group and mutate it through user-interaction closures).

**Chartered sites post-migration.**

| Category | Site | Rationale |
|---|---|---|
| (a) | `emGUIFramework::clipboard: RefCell<Option<Box<dyn emClipboard>>>` (1 decl) | Winit text-event callbacks write clipboard without `&mut framework`. |
| (b) | `emContext::children: RefCell<Vec<Weak<emContext>>>` (1 decl per context instance) | Children list grows during NewChild at init time. |
| (b) | `emContext::models: RefCell<HashMap<TypeId, Rc<dyn Any>>>` (1 decl per context instance) | Model registry populated at plugin init. |
| (a) | `emFileModel<T>` observer list: `clients: Vec<Weak<RefCell<dyn FileModelClient>>>` (1 pattern per model type) | Observers held by arbitrary panel trees; C++ observer-pattern refcount encoding. |
| (c) | `emRadioButton`/`emRadioBox` shared `group: Rc<RefCell<RadioGroup>>` (1 decl per radio widget, ~4-8 widgets per group) | Sibling buttons mutate the shared group's selection. |
| (a) | `emCrossPtr` backing store: `target: Weak<RefCell<T>>` (used wherever an auto-nullifying cross-reference is needed, e.g., popup→parent, dialog→owner) | C++ `emCrossPtr` equivalent — chartered as cross-closure since cross-ptrs are by design wired through arbitrary user-code paths. |
| (a) | `emMiniIpc` server inner: `Rc<RefCell<MiniIpcServerInner>>` (1 decl per server) | Cross-closure event handlers. |
| (a) | `emScreen::Install` return type: `Rc<RefCell<emScreen>>` (chartered per active screen) | Screen mutates across windows, passed to winit closures. |

**Realistic post-migration count.** Current: 155 `Rc<RefCell<T>>` declarations. After migration:
- Deleted outright: scheduler (1), views (~18 SP5-era sites), windows at framework (1), popup window (1), viewport (~6), per-sub-view scheduler (1), pending_framework_actions (2), CoreConfig panel holders (~40 — migrated to `Rc<emConfigModel<T>>` without RefCell per §7 D7.3), plugin Acquire returns (~10 — likewise migrated to `Rc<T>`).
- Retained per charter: clipboard (1), emContext::children (per-context, ~10 active), emContext::models (per-context, ~10 active), emFileModel observers (1 per file model, ~10), radio group (per widget, ~4-8 active × N groups), emCrossPtr (per cross-ref site, ~10 active), emMiniIpc (~2), emScreen (1-2).

**Realistic residual: 30-60 declarations, all chartered.** Each remaining site is explicitly chartered under (a), (b), or (c). The reduction from 155 unexplained to 30-60 chartered is the headline metric; the exact final count depends on per-site migration choices and will be recorded in the Phase 5 closeout.

All other current `Rc<RefCell<>>` declarations are migrated to plain owned state, `Rc<T>`, or deleted.

### 3.7 Deferred framework actions and popup cancellation

The current `Rc<RefCell<Vec<DeferredAction>>>` on `emGUIFramework` is replaced by a `Vec<DeferredAction>` owned by the framework and passed as `&mut` into `EngineCtx::framework_actions`. Engines enqueue via `ctx.framework_action(action)`. The framework drains after `DoTimeSlice` returns.

**Popup cancellation redesign.** The current code (`emGUIFramework.rs:206-229`) uses `Rc::strong_count(&win_rc) == 1` to detect that a popup was cancelled (removed from `emView::PopupWindow`) between the input event and the deferred materialization. Under plain-owned windows this refcount signal vanishes.

Replacement: **pending popups live in a separate map until materialized.**

```rust
pub struct emGUIFramework {
    pub windows: HashMap<WindowId, emWindow>,              // materialized windows
    pub pending_popups: HashMap<WindowId, emWindow>,       // popups not yet winit-materialized
    // ...
}
```

Flow:
1. Popup creation (inside `RawVisitAbs` or its cousin): framework allocates `WindowId`, inserts the `Pending`-state `emWindow` into `pending_popups`. `emView::PopupWindow = Some(WindowId)` (an ID, not an Rc).
2. Popup teardown (before materialization): view's `PopupWindow = None`. Framework's drain path removes the corresponding entry from `pending_popups`.
3. Deferred `DeferredAction::MaterializePopup(WindowId)` drain (called post-slice): framework looks up `pending_popups.remove(&wid)`. If present, materialize its winit surface and move it to `windows`. If absent, popup was cancelled; no-op.

The cancellation signal is now explicit map presence/absence, not implicit refcount. Observable effect: identical to current behavior — cancelled popups don't materialize, surviving ones do. But the new shape eliminates two failure modes: (a) silent breakage if a future code path takes an extra Rc clone (impossible now — no Rc), (b) ambiguity about who "owns" a pending popup (the framework does, period).

---

## 4. Scheduler redesign (detail)

The scheduler is the largest migration. Concrete design decisions:

**D4.1.** `EngineScheduler` loses its `Rc<RefCell<>>` wrapper at every public ownership site. `emGUIFramework::scheduler: EngineScheduler` (plain value). All tests that currently do `sched.borrow_mut().DoTimeSlice(...)` migrate to `framework.scheduler.DoTimeSlice(...)` or equivalent direct-access.

**D4.2.** `DoTimeSlice` signature becomes:
```rust
pub fn DoTimeSlice(
    &mut self,
    tree: &mut PanelTree,
    windows: &mut HashMap<WindowId, emWindow>,
    root_context: &Rc<emContext>,
) -> bool
```
`self` is the scheduler, passed as `&mut`. The windows and tree are borrowed *from* the framework on entry. `&mut emView` access during Cycle goes through `windows.get_mut(wid).unwrap().view`.

**D4.3.** `EngineCtx` is refactored (per §3.3) into `EngineCtx` (scheduler+windows+root_context) and `PanelCtx` (tree+current_panel). `emEngine::Cycle(&mut self, ectx, pctx)` takes both where needed; scheduler-only engines take only `EngineCtx`. This is the core API-surface change, migrating ~15 engine impls.

**D4.4.** All 6 `SchedOp` variants, the `queue_or_apply_sched_op` helper, the `pending_sched_ops` field, the 5 drain sites, and all `try_borrow*` re-entrancy defenses are deleted in the same phase as D4.1-D4.3 land. Each of the 9 production call sites currently doing `self.queue_or_apply_sched_op(SchedOp::Fire(sig))` becomes `ctx.fire(sig)`.

**D4.5.** `close_signal_pending: bool` on emView is deleted. `UpdateEngineClass::Cycle`'s pre-compute at `emView.rs:257-261` becomes an inline `ctx.IsSignaled(close_sig)` check at the top of `emView::Update`, after which emView reads the probe directly.

**D4.6.** `register_engine_for`'s silent-return on `try_borrow_mut` failure (SP4.5-FIX-1) is deleted along with `register_pending_engines`. Engine registration runs inline during panel construction via ctx. SP4.5-FIX-3's +1 slice drift becomes delta=0 by construction. **Delivered in Phase 1.75 Task 5 (continuation)**: `register_pending_engines` and the `try_borrow_mut` deferral are gone; Phase 1.75 Task 6 encodes the post-migration synchronous-registration contract as `phase_1_75_task6_spawn_and_wake_child_in_same_slice_delta_zero` in `emPanelTree.rs`, asserting `delta == 0`. The three originally-planned `sp4_5_fix_1_timing_*_baseline_slices` fixtures were deleted (obsolete under synchronous registration) rather than re-asserted; see Phase-1.75 ledger for the rationale.

**D4.7.** `SP4.5-FIX-2`: popup-creation signal allocation inside `RawVisitAbs` is an inline `ctx.create_signal() × 4`. The `RefCell` re-entrancy hazard that required the pre-allocate-at-construction recommendation is absent. No pre-allocation needed; no latent panic. (Phase 1.75 Task 6 audit confirmed the four inline sites in `emView.rs` `RawVisitAbs`; no pre-allocation block exists anywhere in the file.)

**D4.8.** Per-sub-view scheduler (E005, SP8) is deleted. `emSubViewPanel::sub_scheduler` field removed; the outer scheduler drives both trees via the unified cross-tree dispatch walk described in §3.3 (`TreeLocation` + `as_sub_view_panel_mut`). The SP8 `DIVERGED:` block is removed. **Delivered in Phase 1.75 Task 4** (keystone step of the phase).

**D4.9. Input dispatch as an engine.** The current input-dispatch path (`App::window_event` → `emWindow::dispatch_input`) runs outside any `DoTimeSlice`, so ctx is not in scope. Under the new model, input handlers must have ctx access (for `ctx.fire`, `ctx.create_signal`, etc.) to enable widget signals (§3.5) and popup creation (D4.7).

Resolution: winit input events are enqueued onto `emGUIFramework::pending_inputs: Vec<(WindowId, InputEvent)>`. A framework-owned `InputDispatchEngine` cycles on each `DoTimeSlice` tick at top priority. Its `Cycle(&mut ectx)` drains `pending_inputs` and routes each event through `ctx.with_view_mut(wid, |view, sched| view.dispatch_input(event, sched))`. This gives every input handler a ctx, preserves C++ input→dispatch→update timing (the dispatch engine cycles before the per-view UpdateEngine in each slice), and obviates the need for a separate "non-Cycle ctx flavor."

This is a meaningful shape change: input is no longer handled synchronously in the winit callback. The winit callback enqueues and calls `framework.scheduler.DoTimeSlice(...)` to drain-and-dispatch. Net latency from winit event to dispatch: one DoTimeSlice invocation (microseconds). Observable: identical to C++, where emX11Screen's event loop enqueues events into emScheduler's signal system and DoTimeSlice drains them.

**D4.10. Pre-DoTimeSlice construction: `InitCtx`.** Some widget construction happens at framework init (before any DoTimeSlice runs) — e.g., `App::new` constructs the root panel tree, which constructs root-panel widgets. At this point ctx doesn't exist because there's no running scheduler cycle.

Resolution: introduce `InitCtx<'a>` — a mini-ctx with `&mut scheduler`, `&mut framework_actions`, and `root_context`, but no windows and no current_engine. Widget constructors take a generic trait bound `C: ConstructCtx` where `ConstructCtx` is implemented by both `SchedCtx` (reached during Cycle) and `InitCtx` (reached at framework init). Both expose `create_signal`, `register_engine`, etc.

```rust
pub trait ConstructCtx {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(&mut self, e: Box<dyn emEngine>, pri: Priority) -> EngineId;
    // ... other scheduler API needed at construction time
}

impl ConstructCtx for SchedCtx<'_> { /* delegate to self.scheduler */ }
impl ConstructCtx for InitCtx<'_> { /* delegate to self.scheduler */ }

pub struct emCheckButton {
    pub check_signal: SignalId,
    pub on_check: Option<WidgetCallback<bool>>,
    // ...
}

impl emCheckButton {
    pub fn new<C: ConstructCtx>(ctx: &mut C, caption: &str) -> Self {
        Self {
            check_signal: ctx.create_signal(),
            on_check: None,
            // ...
        }
    }
}
```

Tests that construct widgets pass an `InitCtx` built from the test's scheduler. Production framework init passes `InitCtx`. Panel-tree creation during Cycle passes `SchedCtx`.

**D4.11.** `register_engine_for` (`emPanelTree.rs:558-598`) becomes a function on `PanelTree` that takes `&mut SchedCtx` (or generic `&mut impl ConstructCtx`). Called inline during panel construction. `register_pending_engines` (the catch-up sweep) is deleted. Engines register and are woken in the same call — `register_engine_for` calls `ctx.register_engine(adapter, Priority::Medium)` and then `ctx.wake_up(eid)`. This closes SP4.5-FIX-3 (E008): delta=0 by construction, because register-and-wake-up happen in the same Cycle and priority re-ascent fires the new engine within that slice. **Delivered in Phase 1.75 Task 5 (continuation)**; the `register_engine_for` signature now threads a `TreeLocation` alongside the ctx so the outer scheduler's `engine_locations` map is populated synchronously at registration (see §3.3).

---

## 5. View and window redesign (detail)

**D5.1.** `emView` ceases to be `Rc<RefCell<emView>>` at any public ownership site. `emWindow::view: emView`. `emSubViewPanel::sub_view: emView`. Test code that constructs a bare view uses a stack-owned value or one owned by a test `emWindow::new_for_test`.

**D5.2.** `Weak<RefCell<emView>>` back-refs (SP5's `emPanel::View` field; `UpdateEngineClass::view`; `VisitingVAEngine::view`; `PanelCycleEngine::view`) become `window_id: WindowId` (or `panel_scope: PanelScope::{Toplevel(WindowId), SubView(PanelId)}` for engines that live in sub-views). The engine resolves `&mut emView` through ctx at Cycle entry.

**D5.3.** `emViewPort` becomes a plain owned struct (three per view). The `window: Option<Weak<RefCell<emWindow>>>` back-ref becomes `window_id: Option<WindowId>`, resolved via ctx at the three sites that currently do `window.upgrade()`.

**D5.4.** `emViewPort` home geometry fields (home_x, home_y, home_width, home_height) are preserved with the rewritten rationale (already landed in audit): geometry lives on the port so `SwapViewPorts` moves it atomically when Home and Current exchange identities.

**`SwapViewPorts` mechanics (corrected).** The operation swaps `CurrentViewPort` between **this view** and **the popup window's view** — a cross-emWindow swap, not an intra-emView swap. Both live in `emGUIFramework::windows: HashMap<WindowId, emWindow>`. Concretely:

```rust
// Inside framework-level code (e.g., emView::SwapViewPorts called from a method that
// already has &mut framework reach):
let [this_win, popup_win] = framework.windows
    .get_disjoint_mut([&this_wid, &popup_wid])
    .expect("disjoint window IDs");
std::mem::swap(
    &mut this_win.view.CurrentViewPort,
    &mut popup_win.view.CurrentViewPort,
);
```

`HashMap::get_disjoint_mut` (stable since 1.86) supplies the two disjoint `&mut emWindow` borrows. Inside each, direct field access reaches `CurrentViewPort`. The swap is a single `mem::swap` call; surrounding logic in the current `SwapViewPorts` (geometry updates, signal fires) runs after the swap completes, with the two window borrows still live for the duration of that logic.

**Cross-emWindow swap is a preserved design intent**, not a divergence: C++ swaps raw pointers between emView and its PopupWindow's emView; the two views live in two emWindows. Rust mirrors the same operation using `get_disjoint_mut` for the mutable-borrow acquisition.

**D5.5.** NoticeList ring relocation (E006) is revisited. In the new model, `emView` is accessed only through `&mut self` and there is no RefCell on the view. The re-entrancy argument that justified relocation to `PanelTree` no longer applies. The ring is moved back to `emView`, matching C++ `emView.h:576` exactly. The DIVERGED block at `emView.rs:3465` is deleted.

**D5.6.** Focus storage on `emViewPort::focused` (current: duplicated from emView to support SwapViewPorts) is consolidated to live on `emView` alone. SwapViewPorts swaps the two port identifiers but leaves focus on the view (matching C++ `emView::Focused`). The `focused: bool` field on `emViewPort` and its `DIVERGED:` block at `emViewPort.rs:53` are deleted.

---

## 6. Widget event model (detail)

**D6.1.** Every widget that exposes an event in C++ via `GetXxxSignal` gains a `xxx_signal: SignalId` field allocated at construction via `ctx.create_signal()`. The widget's `Input` or `Handle` method fires it via `ctx.fire(xxx_signal)` at the same moment C++ calls `Signal(GetCheckSignal())`.

**D6.2.** Existing `on_xxx: Option<Box<dyn FnMut(...)>>` callback fields are retained as a convenience layer. The Input handler fires both — signal first (matches C++), then callback (Rust ergonomic). Tests verify the signal fires on a beat with `ctx.IsSignaled` checks; Rust applications can use whichever they prefer.

**D6.3.** The affected widgets: `emCheckButton::check_signal`, `emCheckBox::check_signal`, `emRadioButton::check_signal`, `emButton::click_signal`, `emTextField::text_modified_signal`, `emTextField::selection_modified_signal`, `emColorField::color_signal`, `emFileSelectionBox::selection_signal`, plus any C++ `GetXxxSignal` or `XxxChangedSignal` method.

**D6.4.** C++ `emCheckButton::CheckChanged` virtual override is preserved as a `fn check_changed(&mut self, ctx: &mut EngineCtx<'_>)` method on the widget with a default implementation that fires the signal; subclasses override. This closes the "virtual method folded into callback" smuggle (current E025 / B08).

**D6.5.** The DIVERGED blocks at `emCheckButton::GetCheckSignal`, `emCheckButton::CheckChanged`, `emCheckButton::Clicked`, `emCheckBox::GetCheckSignal`, `emCheckBox::CheckChanged`, `emCheckBox::Clicked` are deleted (or rewritten to note the signal-plus-callback dual surface with "callback is Rust ergonomic convenience" as the idiom rationale).

---

## 7. emRec / emModel / emRef infrastructure (detail)

This is the largest and highest-cost sub-project.

**D7.1.** `emRec` scalar-field infrastructure (emCoreConfig's `VisitSpeed`, `ScrollRotatedEnabled`, `MouseWheelZoomPercent`, …) is ported. **Scope honestly sized.**

C++ `emRec.h` is ~1900 lines; `emRec.cpp` ~2900 lines. The hierarchy includes: `emRecNode` (base), `emRec` (abstract with persistence/change-notification/undo), `emRecListener`, concrete types (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emFlagsRec`, `emAlignmentRec`, `emStringRec`, `emColorRec`, `emStructRec`, `emUnionRec`, `emArrayRec`, `emTArrayRec<T>`), plus IO (`emRecReader`, `emRecWriter`, `emRecFileReader`, `emRecFileWriter`, `emRecMemReader`, `emRecMemWriter`).

The Rust port requires all of the above concrete types that `emCoreConfig` uses. Grep of C++ `emCoreConfig.h` confirms usage of at least `emDoubleRec`, `emIntRec`, `emBoolRec`, `emEnumRec`, `emFlagsRec`, `emAlignmentRec`, `emColorRec`, `emStringRec`. `emStructRec` and `emUnionRec` are used by upstream consumers (emFpPlugin config, emFileManAppli config). `emTArrayRec<T>` for bookmark lists etc.

The port provides:
- `emRec<T>` trait: `GetValue()`, `SetValue(ctx)`, `GetMinValue()`, `GetMaxValue()`, `GetDefaultValue()`, `GetValueSignal()`.
- Concrete types listed above.
- Change-notification via allocated signal fired through `ctx` passed to `SetValue`.
- Persistence hooks (`TryRead`/`TryWrite` from/to `emFileStream`) wired to `emConfigModel::LoadAndSave`.

**SetValue takes ctx.** Because setting an emRec value must fire its change-notification signal, the setter signature is `fn SetValue(&mut self, value: T, ctx: &mut SchedCtx<'_>)`. This propagates the ctx-through-callback requirement from §3.5 all the way down through config-panel interactions.

**Implementation phasing.** Because of the scope, Phase 4 is subdivided (§10):
- Phase 4a: `emRec` trait and primitive concrete types (`emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`). Change-notification. No persistence yet.
- Phase 4b: compound concrete types (`emFlagsRec`, `emAlignmentRec`, `emColorRec`, `emStructRec`, `emUnionRec`, `emTArrayRec<T>`).
- Phase 4c: persistence IO; `emConfigModel::LoadAndSave` wired.
- Phase 4d: migrate `emCoreConfig` fields from flattened to emRec-typed. Migrate the ~40 `emCoreConfigPanel` call sites.

**D7.2.** `emCoreConfig::VISIT_SPEED_MAX` constant and the `VisitSpeed_GetMaxValue()` method are deleted; replaced by `emCoreConfig::VisitSpeed: emDoubleRec` matching C++ exactly. The DIVERGED block at `emCoreConfig.rs:239-250` is deleted.

**D7.3.** `emConfigModel<T>` becomes a first-class generic model wrapper. The current `Rc<RefCell<emConfigModel<emCoreConfig>>>` sharing pattern is replaced: `emConfigModel<T>` is owned by its registering context as `Rc<emConfigModel<T>>` (Rc<T> immutable — the inner emRec fields handle mutation internally via their own event-loop-aware mechanism, typically via a scheduler signal). The ~40 `Rc<RefCell<emConfigModel<emCoreConfig>>>` sites in `emCoreConfigPanel.rs` migrate to `Rc<emConfigModel<emCoreConfig>>` reads + `ctx`-mediated writes through the registered model's `SetValue`.

**D7.4.** `emRef<T>` → `Rc<T>` throughout. The mapping note in `emRef.no_rs` is updated: `emRef<T>` → `Rc<T>` by default, `Rc<RefCell<T>>` only at the chartered categories in §3.6. The **284** current `Rc<RefCell<T>>` declarations (re-baselined 2026-04-19; original draft assumed 155) reduce to ≤ 60 (all chartered).

**D7.5.** `emFileModel` async loading (E028): ported. `emImageFileModel`, `emTgaImageFileModel`, `emBmpImageFileModel` etc. become real models loading via a scheduler engine rather than the current synchronous `load_image_from_file` stub. Scheduler-attached loader engines use the standard emCore pattern.

---

## 8. Resolution of each JSON entry

| Entry | Bucket | Resolution section | Resolution summary |
|---|---|---|---|
| E001 | B01 | §4 D4.4 | SchedOp and IDIOM block deleted; inline ctx calls replace deferral. |
| E002 | B01 | §4 D4.1 | `Rc<RefCell<EngineScheduler>>` → `EngineScheduler` plain value. |
| E003 | B01 | §4 D4.4 | SchedOp enum deleted. |
| E004 | B01 | §4 D4.4 | `pending_sched_ops` field deleted. |
| E005 | B01 | §4 D4.8 / §3.3 | Per-sub-view scheduler deleted; extended-ctx pattern replaces. |
| E006 | B01 | §5 D5.5 | NoticeList ring moved back to emView per C++. |
| E007 | B01 | §4 D4.4 | `queue_or_apply_sched_op` deleted. |
| E008 | B01 | §4 D4.6 | `register_engine_for` inline; SP4.5-FIX-3 +1 drift becomes 0 by construction. |
| E009 | B01 | §4 D4.4 | SVPUpdSlice try_borrow fallback deleted; inline read via ctx. |
| E010 | B01 | §4 D4.5 | `close_signal_pending` field deleted; inline probe restored. |
| E011 | B01 | §4 D4.7 | SP4.5-FIX-2 popup-creation: inline `ctx.create_signal() × 4`. |
| E012 | B01 | §3.3 / §4 D4.3 | PanelCycleEngine preserved; holds IDs not Weak; registration inline. |
| E014 | B02 | §5 D5.3 | `window: Option<Weak<RefCell<emWindow>>>` → `window_id: Option<WindowId>`. |
| E015 | B02 | §5 D5.1 / §6 | Animator-forward location stays on dispatch-owner; rewritten rationale. |
| E016 | B03 | §3.4 | `emView::Context: Rc<emContext>` — preserved, language-forced composition. |
| E017 | B03 | §3.4 / §7 | `emFileModel` base-class→trait, preserved, language-forced. |
| E018 | B03 | preserved | `emScreen` inheritance collapse; method on correct owner. |
| E019 | B03 | preserved | Virtual override → trait methods; language-forced aggregate. |
| E020 | B03 | preserved | Dummy-base-class → Option<backend>; language-forced. |
| E021 | B04 | preserved | Overload splits/renames; language-forced aggregate. |
| E022 | B05 | preserved | Copy-returns-new, Display trait, etc.; language-forced. |
| E023 | B06 | preserved | Container/iterator shape differences; idiom aggregate. Performance deltas documented per P4.4. |
| E024 | B07 | §6 | emCrossPtr mostly Weak; emFileDialog polling shifts to signal-based per §6. |
| E025 | B08 | §6 | Widget signals allocated and fired via ctx; callback retained as convenience. |
| E026 | B09 | §7 D7.1-D7.2 | emRec infrastructure ported; VisitSpeed flattening reverted. |
| E027 | B09 | §7 D7.4 | emRef→Rc<T> mapping; RefCell only at chartered sites. |
| E028 | B09 | §7 D7.5 | Async emImageFileModel ported. |
| E029 | B10 | preserved | wgpu/winit surface lifecycle; dependency-forced. |
| E030 | B10 | preserved | X11 handle absence; dependency-forced. |
| E031 | — | preserved | Upstream-gap; preserved. |
| E032 | — | preserved | Rect newtype; language-forced utility chartered. |
| E033 | — | preserved | Toolkit images embed; dependency/install-forced. |
| E034 | — | preserved | Fixed12 newtype; language-forced utility per P5. |
| E035 | — | obsoleted | Per-wave DIVERGED tally becomes meaningful again once P5 fires out idiom-tagged entries. |
| E036 | — | obsoleted | "Idiom adaptation forced by RefCell" framing rejected by P4. |
| E037 | — | obsoleted | Per-sub-project DIVERGED cap remains useful; acknowledged as now-effective under P4/P5. |
| E038 | — | §5 D5.1 | Borrow-ordering hazard disappears with emView plain ownership. |

Every entry mapped. No "deferred". No "out of scope".

---

## 9. CLAUDE.md amendments

Three edits, named and justified.

### 9.1 Replace ownership line

**Current (CLAUDE.md:47):**
> - **Ownership**: `Rc`/`RefCell` shared state, `Weak` parent refs.

**Replacement:**
> - **Ownership**: Plain owned values are the default. `Rc<T>` (immutable) where the value is shared-read after init. `Rc<RefCell<T>>` requires a justification comment citing one of: (a) cross-closure reference held by winit/wgpu callbacks, or (b) context-registry typed singleton. `Weak<RefCell<T>>` is acceptable only as the pair of an (a)-justified `Rc<RefCell<T>>`. Engine and panel back-references to their owning view or window are IDs (`WindowId`, `PanelId`) resolved through `EngineCtx`, not `Weak<>`.

**Justification.** The old line sanctioned the smuggled-idiom cascade documented in the scheduler-refcell-workaround ledger and in the raw-material JSON. The new line encodes the result of this spec's §3.

### 9.2 Add annotation vocabulary section

New section after §"File and Name Correspondence":

> ## Annotation Vocabulary
>
> - `DIVERGED:` marks a forced divergence per Port Ideology §"Forced divergence". Every block must name which forced category applies (language-forced, dependency-forced, upstream-gap-forced, performance-forced) and cite the test result. Blocks without a category are treated as fidelity-bugs and are fixed, not annotated.
> - `RUST_ONLY:` marks code with no C++ analogue with a chartered justification: language-forced utility (typed wrapper that C++ inline code implicitly provides), dependency-forced alternative, or performance-forced alternative (with benchmark).
> - `IDIOM:` is retired. Below-surface adaptations that preserve observable behavior and introduce no structural commitment are unannotated. If the adaptation needs a comment, write prose explaining the rationale without the tag.
> - `UPSTREAM-GAP:` marks code that mirrors a C++ no-op/stub because upstream itself is a no-op. Preserves upstream semantics.
> - `SPLIT:` marks file splits forced by "one primary type per file". Unchanged.
>
> Annotation lint runs as a standalone `cargo xtask annotations` binary (stable-rustc compatible; text-scan over `rg -n 'DIVERGED:'` / `RUST_ONLY:` matches, validating each hit carries a required category tag). Invoked from the pre-commit hook and from CI. Not a clippy lint — stable Rust does not admit custom clippy lints without switching to nightly, which is out of scope.

### 9.3 Strengthen forced-divergence test

**Current (CLAUDE.md Port Ideology §"Forced divergence"):**
> - **Forced divergence** — Rust or a required dependency (winit/wgpu) makes the C++ shape literally impossible. Not "awkward", not "would require refactoring" — impossible. Minimize the concession.

**Replacement:**
> - **Forced divergence** — one of the following four categories applies:
>   1. **Language-forced.** Try writing the C++ shape in Rust under the project's canonical ownership model (CLAUDE.md §Ownership). If it does not compile, language-forced.
>   2. **Dependency-forced.** A required dependency (wgpu, winit) cannot be made to admit the C++ shape through its public API.
>   3. **Upstream-gap-forced.** C++ emCore itself ships the shape as a no-op.
>   4. **Performance-forced.** Benchmark demonstrates the C++-mirrored shape crossing a documented degradation threshold; the alternative must ship the benchmark and threshold.
>
>   "Idiom adaptation *forced by* a project-internal ownership choice" is not a valid framing. If a Rust choice makes a C++ shape impossible, revisit the Rust choice before marking forced.

**Justification.** The old test admitted the smuggling pattern by treating "impossible under current Rust structure" as synonymous with "impossible in Rust". The new test decouples them.

---

## 10. Phased rollout

Each phase is independently shippable: `cargo test`, `cargo clippy -D warnings`, and `cargo test --test golden -- --test-threads=1` all green at phase boundaries. No hybrid states.

### Phase 1 — Scheduler event-loop-threading (the core)

Implements §3.1, §4 in full. Scope:

- Rewrite `EngineScheduler` owner from `Rc<RefCell<>>` to plain value at `emGUIFramework`.
- Rewrite `EngineCtx` to expose the full scheduler API.
- Migrate 9 `queue_or_apply_sched_op` call sites to `ctx.fire/wake_up/connect/disconnect/remove_signal/remove_engine` inline.
- Delete SchedOp enum, pending_sched_ops field, queue_or_apply_sched_op helper, all 5 drain sites, close_signal_pending, SVPUpdSlice try_borrow fallback.
- Migrate 5 scheduler-touching engine types: `UpdateEngineClass`, `VisitingVAEngine`, `StartupEngine`, `PanelCycleEngine`, `PriSchedAgent`.
- Delete `register_pending_engines` and the `try_borrow_mut`-deferral pathway; synchronous registration via ctx at construction time. **Delivered in Phase 1.75 Task 5 (continuation)** (see ledger; commit `eb5ed94b` and surrounding Task 5 commits).
- Delete per-sub-view scheduler (SP8) and route all sub-tree engines through the single outer `EngineScheduler` via the `TreeLocation` + `as_sub_view_panel_mut` walk described in §3.3. **Delivered in Phase 1.75 Tasks 2–4** (keystone: Task 4).
- Delete `emPanelCtx.rs`; absorb `PanelCtx` into `emEngineCtx.rs`. **Delivered in Phase 1.75 Task 5.**

**Split with Phase 1.75.** Phase 1 originally planned to land all of the above as one cliff. In practice, the scheduler/ctx core landed in Phase 1 proper, and the sub-view unification + `register_pending_engines` / `emPanelCtx.rs` deletions landed in **Phase 1.75** (`port-rewrite/phase-1-75`) because the Phase 1.5 keystone migration had to ship PARTIAL. Phase 1.75 closes Phase 1.5's deferred Tasks 2–5 under the same observational invariants; there is no hybrid state remaining at the Phase 1.75 closeout.

**JSON entries closed:** E001, E002, E003, E004, E005, E007, E008, E009, E010, E011, E036 (framing rejected).

**Gate:** all goldens 237/6 baseline; SP4.5-FIX-3 timing fixtures assert `delta == 0`; SP4.5-FIX-2 regression test (driven by popup-zoom + outside-home + SVPChoiceInvalid) runs without panic and produces the expected popup.

**Risk mitigations:** the phase is large (~1500 LOC changed, ~800 LOC deleted). Break into sub-phases aligned with engine-type migrations, but all sub-phases must land before the gate; no intermediate ships.

### Phase 2 — View/window composition and back-ref migration

Implements §3.2, §5 in full. Scope:

- Rewrite `emWindow::view` from `Rc<RefCell<emView>>` to `emView`.
- Rewrite `emSubViewPanel::sub_view` and `sub_tree` to plain values.
- Rewrite `emViewPort::window` and the three emView ports to plain values.
- Migrate 5 `Weak<RefCell<emView>>` back-refs to `WindowId`/`PanelScope` resolved via ctx.
- Delete NoticeList relocation (E006); move ring back to emView.
- Delete `emViewPort::focused` duplication; consolidate on emView.
- Delete borrow-ordering hazard documentation (E038).

**JSON entries closed:** E006, E014, E015, E038.

**Gate:** all goldens pass; `emView` field borrow-ordering comments in SP3 plan are deleted because the hazard disappears; `rg "Rc<RefCell<emView>>"` returns no matches in production code.

### Phase 3 — Widget signal model (B08)

Implements §6 and the callback-signature migration from §3.5 in full. Scope:

- Allocate `SignalId` at every widget construction that exposes a C++ GetXxxSignal.
- Fire the signal inline from the widget's Input/Handle handler.
- Preserve callback fields as convenience; migrate every existing callback from `Box<dyn FnMut(Args)>` to `Box<dyn for<'a, 'b> FnMut(Args, &'b mut SchedCtx<'a>)>`. ~40-50 install sites across `emCoreConfigPanel.rs`, `emScalarField.rs`, `emFileSelectionBox.rs`, `emRadioButton.rs`, `emTextField.rs`, emmain application code.
- Rewrite affected `DIVERGED:` blocks or delete where the observable surface now matches C++.
- **emFpPlugin API signature change.** `emFpPlugin::CreateFilePanel`, `CreateFilePanelWithStat`, `TryCreateFilePanel`, and `SearchPlugin` currently take `&self`. They must migrate to take `&mut C: ConstructCtx` so plugin-created widgets can allocate their signals via ctx. Affects plugin trait + all implementors; mechanical but pervasive.

**JSON entries closed:** E024 (emFileDialog polling model), E025 (all widget signals).

**Gate:** for each affected widget, a new test verifies the C++ signal is fired (via `ctx.IsSignaled(widget.check_signal)` or equivalent) on user-event dispatch. Existing callback-based tests continue to pass.

### Phase 4 — emRec infrastructure (split: 4a, 4b, 4c, 4d)

Implements §7.1-7.3, sized per §7 D7.1. Four sub-phases:

**Phase 4a — emRec trait + primitive concrete types.** `emRec<T>` trait (`GetValue`, `SetValue(ctx)`, `Get[Min|Max|Default]Value`, `GetValueSignal`). Concrete: `emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`. Change-notification via allocated signals. No persistence yet.

**Phase 4b — compound concrete types.** `emFlagsRec`, `emAlignmentRec`, `emColorRec`, `emStructRec`, `emUnionRec`, `emTArrayRec<T>`. Each handles its own internal change-notification composition.

**Phase 4c — persistence IO.** `emRecReader`, `emRecWriter`, `emRecFileReader`, `emRecFileWriter`, `emRecMemReader`, `emRecMemWriter`. `emConfigModel::LoadAndSave` wired through the IO classes.

**Phase 4d — migrate emCoreConfig and its panel.** `emCoreConfig::VisitSpeed` et al migrate from flattened f64 + const to emRec-typed. ~40 `emCoreConfigPanel` sites migrate from `Rc<RefCell<emConfigModel<emCoreConfig>>>` to `Rc<emConfigModel<emCoreConfig>>` + ctx-mediated `SetValue` calls. All callback signatures updated (already handled by Phase 3).

**JSON entries closed:** E026 (4a-4d complete), E027 (4d complete).

**Gate (per sub-phase).** Each sub-phase ships green: `cargo test`, clippy, goldens. Phase 4d's gate includes a new test verifying `VisitSpeed` change-notification fires its signal correctly.

### Phase 5 — Async plugins + annotation lint + CLAUDE.md deltas

Implements §7.5, §9, P5/P6 enforcement. Scope:

- Port `emFileModel` async subsystem; replace `load_image_from_file` stub with real `emImageFileModel` using scheduler engine.
- Add lint: every `DIVERGED:` block must cite a forced category; every `RUST_ONLY:` block must cite a charter category. Fail CI on missing.
- Amend CLAUDE.md per §9.
- Close final `E035, E037` (governance entries become effective).
- Re-run provenance audit: every DIVERGED block in-tree has a rewritten rationale conforming to new vocabulary.

**JSON entries closed:** E028, E035, E037; plus the lint enforces that no new smuggled-idiom entries can be added without detection.

**Gate:** `cargo clippy -D warnings` passes (unchanged from baseline); `cargo xtask annotations` passes; every in-tree DIVERGED block carries a forced-category citation; `cargo test` and goldens green.

---

## 11. Non-regression invariants

These invariants must hold after every phase. Plans enforce them by test; Phase 5 lint enforces them statically.

**I1.** No `Rc<RefCell<EngineScheduler>>` declaration anywhere in the tree after Phase 1. Grep-enforceable.

**I2.** No `Rc<RefCell<emView>>` declaration in production code after Phase 2. `cfg(test)` helpers exempt during the phase; Phase 5 lint removes the exemption.

**I3.** Every `DIVERGED:` block cites a P4 forced category. Phase 5 `cargo xtask annotations`.

**I4.** Every `RUST_ONLY:` block cites a P5 charter category. Phase 5 `cargo xtask annotations`.

**I5.** No `IDIOM:` blocks anywhere. Phase 1 deletes the sole existing one; lint forbids reintroduction.

**I6.** Golden tests 237/6 baseline preserved across every phase.

**I7.** Nextest count preserved or increased across every phase (migrations add regression tests, do not remove them).

**I8.** Every observable ordering invariant in emCore (signal-drain before timer-fire, wake-up priority re-ascent, focus change before layout, etc.) retains its current test coverage or strengthens it.

---

## 12. What this spec explicitly rejects

To pre-empt satisficing during implementation:

- **Rejected:** "port the scheduler event-loop-threading only for the top-level view; keep `Rc<RefCell<>>` for sub-views." — Hybrid state; violates P6.
- **Rejected:** "keep SchedOp as a convenience for callers that don't have ctx handy." — Every caller reachable from Cycle has ctx. "Don't have ctx" would be a design failure, not a valid reason to keep SchedOp.
- **Rejected:** "`emRec` infrastructure is out of scope for the immediate rewrite." — E026 is bound to §7; deferring would leave the observable surface (signal-on-config-change) missing.
- **Rejected:** "keep the current `Rc<RefCell<emConfigModel<T>>>` pattern because the 40 sites in emCoreConfigPanel are a lot to change." — Bulk-count is never a justification under the observational-port discipline.
- **Rejected:** "add a new `ctx.defer()` for cases we haven't migrated yet." — No deferral. If a site can't migrate, the migration plan is wrong.
- **Rejected:** "one-phase partial scheduler migration that retains SchedOp for scheduler-touching-from-inside-borrow_mut cases." — The borrow_mut is gone in Phase 1; there is no case to retain for.
- **Rejected:** marking any new case "idiom adaptation forced by project-internal ownership choice X." — Framing explicitly rejected by P4 and codified in CLAUDE.md delta §9.3.

---

## 13. Open questions

Minimized per user instruction. Two residual decisions that plans must make; both preserve observational equivalence regardless of resolution.

**Q1.** The sub-tree nested-cycling pattern (§3.3) requires a concrete lifetime/borrow shape for passing an outer `EngineCtx` through a `PanelCycleEngine::Cycle` that recurses into sub-tree cycling. Two candidates:
  (a) explicit lifetime bound on a `ctx.with_sub_tree<'sub>(tree: &'sub mut PanelTree, f: impl FnOnce(&mut PanelCtx<'sub>))`.
  (b) pass sub_tree as an extra parameter to a separate `Cycle_for_sub_tree` path.

Pick during Phase 1 implementation. Both compile under the take/put pattern (§3.3).

**Q2.** `InitCtx` vs `SchedCtx` unification: the `ConstructCtx` trait (D4.10) abstracts over both. Implementation choice: monomorphize every widget constructor per-ctx-type, or erase to `dyn ConstructCtx` for constructor signatures. Monomorphization is simpler and incurs no perf cost; erasure reduces code bloat. Pick during Phase 3 implementation.

Everything else is determined.

---

## 14. Traceability appendix — JSON entry → phase

| Phase | JSON entries closed |
|---|---|
| 1 | E001, E002, E003, E004, E005, E007, E008, E009, E010, E011, E036 |
| 2 | E006, E014, E015, E038 |
| 3 | E024, E025 |
| 4 | E026, E027 |
| 5 | E028, E035, E037 |
| Preserved with rewritten rationale | E012, E016, E017, E018, E019, E020, E021, E022, E023, E029, E030, E031, E032, E033, E034 |

All 37 entries accounted for.
