# Scheduler RefCell — Divergence and Workaround Ledger

**Date:** 2026-04-19.
**Scope:** Documents the forced divergence introduced by holding `Rc<RefCell<EngineScheduler>>::borrow_mut()` for the lifetime of `DoTimeSlice`, the chain of workarounds it has forced across SP4, SP4.5, SP4.5-FIX-1, and the two open follow-ups (SP4.5-FIX-2, SP4.5-FIX-3), and the criteria under which the root ownership model would be rearchitected rather than patched.

This is a root-cause note. It does not propose landing a rearchitecture; it records the shape of the debt so future sub-projects can decide whether to keep paying interest or refinance.

---

## 1. The forced divergence

### 1.1 C++ model

`emScheduler` is reached by raw pointer from `emEngine`. There is no aliasing discipline between the scheduler and code running inside `emEngine::Cycle`. `Cycle` freely:

- creates and destroys signals (`emScheduler::CreateSignal`, `RemoveSignal`),
- registers and removes engines (`emScheduler::AddEngine`, `RemoveEngine`),
- wakes engines (`WakeUp`), fires signals (`Signal`),
- mutates timer and priority queues.

These calls happen inline, synchronously, from the middle of a `DoTimeSlice` iteration. The scheduler observes its own mutations and continues the slice. Priority re-ascent (C++ `emScheduler.cpp` priority-queue scan) lets a just-registered higher-or-equal-priority engine receive its first `Cycle` within the **same** `DoTimeSlice`.

### 1.2 Rust model (current)

`EngineScheduler` is shared via `Rc<RefCell<EngineScheduler>>`. `EngineScheduler::DoTimeSlice(&mut self, ...)` is called via `scheduler.borrow_mut().DoTimeSlice(...)` from the framework's event loop (`emScheduler.rs:278`; callers at `emView.rs:5071, 6652, 6757, 6834`, `emSubViewPanel.rs:325, 595`, `emPriSchedAgent.rs:190`, `emFileModel.rs:901`).

The `borrow_mut()` is held for the entire slice. Every `emEngine::Cycle` invocation runs *inside* that outer borrow. Any code reached transitively from `Cycle` that attempts `scheduler.borrow_mut()` — directly or via `register_engine`, `create_signal`, `wake_up`, etc. — panics on re-entrant `BorrowMutError`.

### 1.3 Why this is a forced divergence, not a design choice

Rust's aliasing model does not admit the C++ shape directly. To let `Cycle` mutate the scheduler, one of the following must hold:

1. The scheduler is not behind a `RefCell` at all, and `DoTimeSlice` takes `&mut self` passed as a parameter into every `Cycle` call. (Requires threading `&mut EngineScheduler` through every engine signature and every call site that might touch the scheduler, including `emView::Update` and the entire panel tree.)
2. The scheduler's *internal* state is sharded into interior cells (per-engine `RefCell`, per-queue `RefCell`) such that re-entrant operations touch disjoint cells. (Requires redesigning the scheduler internals, not its public API.)
3. Operations reachable from `Cycle` do not actually need scheduler access — they are deferred.

Option (3) — deferral — is what the current port does. The first two are real options, but each is a scheduler rearchitecture rather than a fix.

Status: this is classified as a **forced divergence** under the Port Ideology §"Forced divergence" clause (the C++ shape is literally impossible with the chosen `Rc<RefCell<>>` ownership). The follow-on question — whether the `Rc<RefCell<>>` choice itself is forced or is a first-order idiom adaptation — is discussed in §6.

---

## 2. The workaround machinery in the tree today

### 2.1 `SchedOp` deferral queue

Defined at `emView.rs:195-206`:

```rust
pub(crate) enum SchedOp {
    Fire(SignalId),
    WakeUp(EngineId),
    Connect(SignalId, EngineId),
    Disconnect(SignalId, EngineId),
    RemoveSignal(SignalId),
    RemoveEngine(EngineId),
}
```

Each `emView` carries `pending_sched_ops: Vec<SchedOp>` (`emView.rs:434`). The entry point is `queue_or_apply_sched_op` (`emView.rs:656`):

```rust
pub(crate) fn queue_or_apply_sched_op(&mut self, op: SchedOp) {
    match scheduler.try_borrow_mut() {
        Ok(mut s) => op.apply_to(&mut s),
        Err(_)    => self.pending_sched_ops.push(op),
    }
}
```

`SchedOp::apply_to(&mut EngineScheduler)` is used when `try_borrow_mut` succeeds (outside any slice). `SchedOp::apply_via_ctx(&mut EngineCtx)` is used when the queue is drained **inside** a slice, via the per-slice `EngineCtx` that already holds `&mut EngineScheduler` from `DoTimeSlice`. Both paths exist at `emView.rs:208-233`.

### 2.2 Drain points

Deferred ops are drained in five places:

1. `UpdateEngineClass::Cycle` (`emView.rs:266-269`) — primary path; drains immediately after `view.Update` returns, while still inside the same `DoTimeSlice`. This is what restores same-slice semantics for the operations deferred during `Update`.
2. `App::about_to_wait` — drains after each framework tick for any ops queued outside a slice.
3. `emSubViewPanel::Cycle` (`emSubViewPanel.rs:634-638`) — sub-view variant of (1).
4. `PanelTree::run_panel_cycles` internal settle (`emPanelTree.rs:3616-3617, 3645-3646`) — legacy synchronous settle path; deleted on main paths by SP8 but still present in `emSubViewPanel::Paint` and the golden `settle()` helper.
5. `emSubViewPanel::Paint` settle loop (`emPanelTree.rs:3742-3844` region) — repeated drain during the synchronous settle inside Paint.

All five drains are the same pattern: swap `pending_sched_ops` into a local `Vec`, iterate, apply.

### 2.3 Call sites that queue rather than apply

Everywhere in `emView` that would have called `scheduler.borrow_mut().X()` from a path reachable by `Cycle` now calls `queue_or_apply_sched_op` instead. Current count (`rg queue_or_apply_sched_op` in `emView.rs`): 8 production sites —

| Line | Operation | C++ analogue |
|---|---|---|
| `emView.rs:1126` | `SchedOp::Fire(sig)` | `Signal(geometry_signal)` in `SetGeometry` |
| `emView.rs:1584` | `SchedOp::Fire(sig)` | popup-teardown geometry double-fire |
| `emView.rs:1866` | `SchedOp::Connect(close_sig, eng_id)` | popup `LinkCrossPtr` at creation |
| `emView.rs:1953` | `SchedOp::Disconnect(close_sig, eng_id)` | popup teardown |
| `emView.rs:1954` | `SchedOp::RemoveSignal(close_sig)` | popup teardown |
| `emView.rs:1980` | `SchedOp::Fire(sig)` | popup flags/focus fan-out |
| `emView.rs:3051` | `SchedOp::Fire(sig)` | `SetViewFlags` |
| `emView.rs:3169` | `SchedOp::WakeUp(id)` | `WakeUpUpdateEngine` |
| `emView.rs:3355` | `SchedOp::Fire(sig)` | `SetFocused` |

Panel-tree deregistration goes through the same path at `emPanelTree.rs:626`:

```rust
view.queue_or_apply_sched_op(SchedOp::RemoveEngine(eid));
```

### 2.4 SVPUpdSlice throttle — latent-borrow `try_borrow` fallback

`emView::Update` reads the scheduler's time-slice counter for throttle logic. That read runs while `DoTimeSlice` already holds the borrow. Fix at `emView.rs:~2082-2090`: switch to `try_borrow().ok()` with a cached-field fallback on contention. Not a `SchedOp` — a read-only probe that can tolerate a stale value (worst case: 1000+ retries in one slice produce a slight throttle counter stall; observable consequence: none).

### 2.5 Popup-close probe

Same pattern, different shape. `emView::Update` needs to know whether the popup's `close_signal` is signaled *this slice*. The check originally took `scheduler.borrow()`, which failed under the outer `borrow_mut()`. Fix at `emView.rs:257-261` and `emView.rs:422-429`: pre-compute the result in `UpdateEngineClass::Cycle` using `ctx.IsSignaled(close_sig)` (ctx holds `&mut EngineScheduler` and offers read access without a second borrow), stash into `close_signal_pending: bool` on the view, and have `Update` read the cached bool.

This is structurally the same trick as FIX-2's recommended "pre-allocate at construction": move the scheduler access to a point in the program where the outer borrow doesn't exist.

### 2.6 `register_engine_for` — SP4.5-FIX-1 shape

`PanelTree::register_engine_for` (`emPanelTree.rs:558-600`) must allocate an `EngineId` and insert into the scheduler's engine slot-map. It is reachable from `create_child`, which is reachable from:

- `StartupEngine::Cycle` → panel construction (production startup),
- `emView::Update` → `emVirtualCosmosPanel::update_children` → `create_child` (input dispatch),
- sibling `emPanel::Cycle` creating children dynamically.

All three run inside `DoTimeSlice`. The fix at `emPanelTree.rs:573-600`:

```rust
let Ok(view_borrow) = view_rc.try_borrow() else { return; };
let ...
let Ok(mut sched) = sched_rc.try_borrow_mut() else { return; };
// ... register ...
```

On contention, return silently. The **catch-up** is `register_pending_engines()` (`emPanelTree.rs:605-611`), which re-scans the panel tree and re-attempts registration for every panel whose `engine_id` is still `None`. It runs after each `DoTimeSlice` in `App::about_to_wait` and `emSubViewPanel::Cycle`.

Observable consequence: a panel created at slice N first cycles at slice N+1. C++ delta=0 (measured, 169 panels); Rust delta=+1 (baseline-locked by fixtures at commits `b4681d3`, `66decfc`, `d4238d8`).

### 2.7 Per-sub-view scheduler (SP8)

`emSubViewPanel` owns `sub_scheduler: Rc<RefCell<EngineScheduler>>` (`emSubViewPanel.rs:~51`, one forced `DIVERGED:` block). The sub-view's engines register against the sub-scheduler, not the outer one, because `EngineCtx::tree` is singular — an outer-scheduler `Cycle` cannot recursively drive a nested `PanelTree`'s cycles without aliasing the tree through the ctx.

This is a second-generation workaround: the original `RefCell` divergence forced per-sub-view scheduler *ownership* to keep the contracting `borrow_mut()` windows disjoint. C++ has one scheduler per process; Rust now has one per sub-view.

### 2.8 Inventory summary

| Mechanism | Location | Purpose | Introduced by |
|---|---|---|---|
| `SchedOp` enum + `pending_sched_ops: Vec<SchedOp>` | `emView.rs:195-206, 434` | Defer writes from inside `Cycle` | SP4 |
| `queue_or_apply_sched_op` | `emView.rs:656` | Conditional defer | SP4 |
| `apply_to` / `apply_via_ctx` dispatch | `emView.rs:208-233` | Two drain modes | SP4 |
| Drain in `UpdateEngineClass::Cycle` | `emView.rs:266-269` | Same-slice drain | SP4 |
| Drain in `App::about_to_wait` | framework | Post-slice drain | SP4 |
| Drain in `emSubViewPanel::Cycle` | `emSubViewPanel.rs:634-638` | Sub-view drain | SP8 |
| Drain in `run_panel_cycles` settle | `emPanelTree.rs:3616-3845` | Settle-loop drain | pre-SP8 legacy |
| SVPUpdSlice `try_borrow` fallback | `emView.rs:~2082-2090` | Read-only probe | SP4 |
| `close_signal_pending: bool` + ctx pre-compute | `emView.rs:257-261, 422-429` | Move read out of `Update` | SP4 |
| `register_engine_for` silent-return | `emPanelTree.rs:558-600` | Defer registration | SP4.5-FIX-1 |
| `register_pending_engines()` sweep | `emPanelTree.rs:605-611` | Post-slice registration | SP4.5-FIX-1 |
| Per-sub-view `sub_scheduler` | `emSubViewPanel.rs:~51` | Disjoint borrow windows | SP8 |

Twelve mechanisms, one root cause.

---

## 3. The ledger — chronological

| Date | Event | What it added |
|---|---|---|
| 2026-04-17 (SP4 plan) | Popup-close signal inside `Update` hit re-entrant `borrow_mut` panic. | `SchedOp` enum, `pending_sched_ops`, 8 call-site migrations, dual-drain pattern. |
| 2026-04-18 (SP4 land) | Phase-8 test rewritten to use single-engine routing. SVPUpdSlice `try_borrow` fallback added. | Read-side `try_borrow`; `close_signal_pending` field. |
| 2026-04-19 (SP4.5 plan) | `emPanel` engine-registration port — adapters register per-panel. | Per-panel `PanelCycleEngine` registered via `register_engine_for`. |
| 2026-04-19 (SP4.5 land) | Tested in isolation, worked. | Direct `borrow_mut()` in `register_engine_for`. |
| 2026-04-19 (SP4.5-FIX-1) | Production startup panicked at first `StartupEngine::Cycle`; input dispatch panicked through `emView::Update`. | `try_borrow_mut` silent-return + `register_pending_engines()` post-slice sweep. |
| 2026-04-19 (FIX-1 follow-ups audit) | Part A scanned 145 rows across four files for the same shape. Found 1 vulnerable (popup signal allocation). Filed as FIX-2. | No production change; 2 regression tests + audit table. |
| 2026-04-19 (FIX-1 follow-ups timing) | Part B measured C++ delta=0 (169 panels), locked Rust baselines at delta=+1 across three paths. Filed as FIX-3. | No production change; 3 timing fixtures + test-support probe. |

Each entry is the same story: a C++ feature reaches the scheduler from inside `Cycle`; Rust panics; the workaround stack grows.

---

## 4. Open items downstream of the divergence

### 4.1 SP4.5-FIX-2 — popup signal allocation in `RawVisitAbs`

**Location:** `emView.rs:1828-1836`.

```rust
let (close_sig, flags_sig, focus_sig, geom_sig) =
    if let Some(sched) = self.scheduler.as_ref() {
        let mut s = sched.borrow_mut();
        (s.create_signal(), s.create_signal(),
         s.create_signal(), s.create_signal())
```

`borrow_mut()` here is the same hazard as every other site, but the deferral template doesn't fit: the four `SignalId` values must exist **synchronously** to wire the popup `emWindow` that is constructed in the next few statements. A post-slice drain produces IDs too late to stitch into the popup. The natural `WakeUpUpdateEngine` retry doesn't replay `RawVisitAbs` because the triggering `SVPChoiceInvalid` flag is cleared by `Update`'s loop at `emView.rs:~2506` before each retry call — the retry tick sees no work, so the popup silently never appears.

Two defer-pattern attempts (`a67bdc0`, `95d7266`) were reverted because they converted a panic into that silent missing popup.

**Shape of the fix (recommended).** Pre-allocate the four signals at `emView::new` / `attach_to_scheduler`. `RawVisitAbs` reads the pre-allocated IDs. Identical in spirit to §2.5's `close_signal_pending` trick: move the scheduler access to a point where the outer borrow doesn't exist. This is *not* a C++ port — C++ allocates on-demand because it can — but it is the smallest deviation that closes the panic without introducing a worse failure mode.

**Why the rearchitecture alternative would fix this naturally.** If the scheduler were sharded (§1.3 option 2) or passed as `&mut` through `Cycle`'s signature (option 1), the on-demand C++ allocation would work verbatim and no pre-allocation would be needed.

### 4.2 SP4.5-FIX-3 — same-slice registration

**Location:** `emPanelTree.rs:558-611`.

C++ registers and first-cycles a panel in the **same** `DoTimeSlice` (measured delta=0). Rust defers to a post-slice sweep (delta=+1). The fix shape is `SchedOp::RegisterPanelEngine(PanelId)` drained inside `UpdateEngineClass::Cycle`, relying on existing priority re-ascent at `emEngine.rs:236-241` to let the newly-registered engine run in the same outer slice.

This would add a 7th `SchedOp` variant — and a non-`Copy` one, since `apply_via_ctx` must build the adapter, insert into `EngineCtxInner::engines`, and write an `EngineId` back into `tree.panels[panel_id].engine_id`. The existing six variants are field-only `Copy`. This is the first case where the workaround machinery generalizes from "scheduler write with an ID" to "scheduler write whose result is an ID consumed by the tree."

**Why the rearchitecture alternative would fix this naturally.** With either §1.3 option 1 or 2, `register_engine_for` calls inline with no `try_borrow_mut` hedge. No deferral, no catch-up sweep, no baseline drift.

### 4.3 The pattern

Every open item has two fix shapes:

1. A new workaround entry (pre-allocate at construction; add a `SchedOp` variant that carries a closure and a write-back).
2. The rearchitecture that makes the workaround entry unnecessary.

(1) is what we land; (2) is what we track.

---

## 5. Expected future entries

None of the following are filed; they are predictions from the current shape of the code.

1. **Any new caller of `create_signal` from inside `Cycle`** will require either a new pre-allocation site (FIX-2 shape) or a new `SchedOp` variant that returns an ID (FIX-3 shape). Current known candidates: dynamic signal creation in dialog / progress UI, if ever ported.
2. **Any caller of `register_engine` from inside `Cycle` that is not a `PanelCycleEngine`** will repeat the FIX-1 → FIX-3 cycle. Candidates: animator engines created in response to user input, network-driven engines.
3. **Nested sub-views** (sub-view inside sub-view) will stack SP8's per-sub-view scheduler arrangement another level, with the same contracting-borrow-window logic.
4. **Any read-side scheduler access from a path reachable by `Cycle`** will repeat the SVPUpdSlice `try_borrow` pattern. The cached-field fallback is not generally safe — it only works for throttle counters and similar slack-tolerant values.

The ledger grows monotonically with the number of distinct scheduler-touching paths in the port. Every new emCore feature ported that reaches the scheduler via `Cycle` adds an entry.

---

## 6. Root-cause assessment

### 6.1 The bottom turtle

The question isn't "can we avoid `SchedOp`?" It's "is `Rc<RefCell<EngineScheduler>>` the right ownership model?"

The choice was made early (pre-SP1 era). It made sense when the framework was single-threaded and the scheduler was small. Under the observational-port discipline, this is an **idiom adaptation** of a C++ raw-pointer scheduler to Rust shared ownership — not a forced divergence. Once `DoTimeSlice` started growing re-entrant call chains (SP4 onward), the idiom adaptation became the load-bearing choice. Every subsequent forced divergence downstream (`SchedOp`, `register_pending_engines`, per-sub-view scheduler) derives from it.

So the honest classification stack is:

- **Idiom adaptation (first-order):** scheduler owned as `Rc<RefCell<EngineScheduler>>`. Not strictly forced — alternatives exist — but load-bearing by now.
- **Forced divergence (second-order):** re-entrant `borrow_mut` is impossible with that ownership. `SchedOp` + deferral is required to preserve observable semantics.
- **Workaround entries (third-order):** each mechanism in §2 is a consequence of the second-order divergence.

### 6.2 What the rearchitecture would look like

**Option A — thread `&mut EngineScheduler` through `Cycle`.**
Signature change: `fn Cycle(&mut self, sched: &mut EngineScheduler, ctx: &mut EngineCtx) -> bool`. Scheduler is no longer owned by `Rc<RefCell>` at all — it's owned by the framework's event loop and passed by `&mut` into each engine. `emView`, `emPanel`, and `PanelTree` lose all references to the scheduler; they receive it as a parameter when needed.

Blast radius: every `emEngine` implementation (currently ~6-8 concrete types), every method reachable from `Cycle` that touches the scheduler (~15 sites in `emView`, ~10 in `emPanelTree`, test harnesses). No `SchedOp`, no `pending_sched_ops`, no `register_pending_engines`, no `close_signal_pending`, no `try_borrow` fallbacks.

This matches C++ closely — the `emScheduler *` pointer in C++ is semantically a mutable borrow.

**Option B — shard the scheduler's interior.**
Keep `Rc<RefCell<>>` at the boundary, but move per-engine state, signal state, and timer state into independent interior cells such that `Cycle`-time mutations don't alias the outer cell. Harder to get right; easier migration path because public API can be preserved.

**Option C — hybrid.**
Event-loop holds `&mut EngineScheduler` directly (drops `Rc<RefCell>` at the top level); `EngineCtx` extends to expose the full scheduler API (not just the `Fire`/`WakeUp`/`Connect`/`Disconnect`/`RemoveSignal`/`RemoveEngine`/`IsSignaled` subset it exposes today); `Cycle` code calls through `ctx`. No user code holds a scheduler reference. `SchedOp` disappears — every call just goes through `ctx`.

Option C is probably the cleanest fit. `EngineCtx` already exists (`emEngine.rs`) and already carries the scheduler as `&mut`; the work is (a) broaden its surface, (b) rip out the dual-ownership cell, (c) delete the deferral machinery.

### 6.3 Cost of staying put

Each open FIX is 30–100 LOC. The machinery grows by one mechanism per discovered site. Running total: 12 mechanisms across 4 sub-projects in ~2 days. The marginal cost per entry is bounded (the pattern is understood; the new variants are mechanical). The cost is **not** the per-entry LOC — it's the continuing divergence from C++ structure, which makes future ports noisier and golden-test diagnosis harder. Every new C++ feature that touches the scheduler now requires re-deriving the deferral strategy.

### 6.4 Cost of unwinding

Option C is roughly:

- Delete `SchedOp`, `pending_sched_ops`, `queue_or_apply_sched_op`, five drain sites.
- Delete `register_pending_engines`, `close_signal_pending`, the SVPUpdSlice `try_borrow` fallback.
- Delete `emSubViewPanel::sub_scheduler` (if the ctx-threading makes the nested-tree problem tractable — needs verification).
- Broaden `EngineCtx`'s surface: add `create_signal`, `register_engine`, and similar.
- Migrate ~25-30 call sites from `queue_or_apply_sched_op(...)` / `scheduler.borrow_mut().X(...)` to `ctx.X(...)`.
- Migrate callers of `DoTimeSlice` to pass `&mut EngineScheduler` directly.

Rough estimate: 300-600 LOC deleted, 100-200 LOC added, ~40 touched call sites. A solid week of focused work with a good test safety net (the ~2451 tests + 237 golden currently exercise most paths).

### 6.5 Decision criterion

Rearchitect when any of:

1. A third forced-divergence entry is filed beyond FIX-2 and FIX-3 (signals a pattern that is not tapering off).
2. A new feature's port requires a `SchedOp` variant that is not mechanically derivable (e.g., carries a callback with captured tree-shape).
3. Multi-window work begins in earnest and the per-sub-view scheduler pattern is about to multiply.
4. Golden-test diagnosis becomes dominated by "which drain site ran when" questions.

Until one of these, continue paying the interest — the FIXs are tractable and localized, and the rearchitecture has no observable payoff today.

---

## 7. Filing

- **FIX-2 (open):** §8.1 item 16 in `2026-04-18-emview-subsystem-closeout.md`. This doc adds causal context; no change to the closeout status.
- **FIX-3 (open):** §8.1 item 16 of same. Ditto.
- **This doc:** linked from the closeout as the root-cause companion to §8.1 item 16.

No action items. The recommendation is to keep FIX-2 and FIX-3 as planned and re-evaluate the rearchitecture against §6.5 criteria when the next scheduler-reaching port is scoped.
