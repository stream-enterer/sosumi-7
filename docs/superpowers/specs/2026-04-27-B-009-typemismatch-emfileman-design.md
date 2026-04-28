# B-009-typemismatch-emfileman — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-009-typemismatch-emfileman.md`
**Pattern:** P-003-typemismatch-blocks-subscribe
**Scope:** emfileman, 14 rows (3 accessors + 11 consumers)
**Mechanical-vs-judgement:** judgement-heavy at the accessor (D-007/D-008 promotion), mechanical at consumers.

## Summary

Flip the three emfileman u64 accessors to `SignalId`, add lazy-allocation helpers, thread `&mut EngineCtx` through mutators, and migrate 11 consumer panels onto the D-006 first-Cycle subscribe shape. The bucket promotes two new global decisions — **D-007-mutator-fire-shape** and **D-008-signal-allocation-shape** — that are forced companions to the D-001 accessor flip.

## Decisions cited

- **D-001-typemismatch-accessor-policy.** Chosen direction: flip accessor to `SignalId`. Applies to all 3 accessor rows in this bucket.
- **D-003-gap-blocked-fill-vs-stub.** Fill in scope. The 3 "gap-blocked" rows in the bucket inventory are the accessors themselves (audit tagged the u64 mismatch as a gap); the fix is the flip, not separate gap-fill work.
- **D-005-poll-replacement-shape.** Direct subscribe (`subscribe → react in Cycle`) for the 4 polling consumer rows: emDirEntryAltPanel-36, emDirPanel-38, emFileManControlPanel-327, emFileManSelInfoPanel-37.
- **D-006-subscribe-shape.** First-Cycle init for all 11 consumer rows.
- **D-007-mutator-fire-shape (proposed; see §"Proposed D-007").** Mutators thread `&mut EngineCtx` to call `ectx.fire(signal)` synchronously.
- **D-008-signal-allocation-shape (proposed; see §"Proposed D-008").** Lazy allocation on first subscriber via `Ensure*Signal(&mut self, ectx) -> SignalId`.

D-002 and D-004 do not apply to this bucket.

## Proposed D-007-mutator-fire-shape

> **Question.** When a model accessor flips from a polled `u64` counter to a `SignalId`, how do mutators fire the signal given that they currently lack scheduler/`EngineCtx` access?
>
> **Options considered.**
>
> - **A. Thread `&mut EngineCtx` through every mutator signature.** Authentic to C++ (where `emSignal::Signal()` walks the scheduler graph synchronously). All B-009 production mutator callsites are in panel `Input`/`Cycle` bodies that already hold `&mut EngineCtx`; threading is mechanical.
> - **B. Model owns a tiny "publisher" engine.** Mutators set a `Cell<bool> dirty` flag; a publisher engine fires once per Cycle. Adds a per-model engine; one-tick observable defer relative to C++ synchronous fire.
> - **C. Hybrid `ensure_fired(ectx)` called from consumers.** Awkward; pushes responsibility onto the wrong side of the wire.
>
> **Chosen direction.** **A. Thread `&mut EngineCtx` through mutators.**
>
> **Why.** B is observably non-equivalent (one-tick fire defer). Per Port Ideology §"observational port" the defer requires a forced-category justification, and none exists — the only blocker is project-internal ownership. A's wider touch surface is mechanical: the B-009 mutator-callsite enumeration found every production call sits inside a panel `Input`/`Cycle` body that already has `&mut EngineCtx`. No exceptions.
>
> **Operational rule.**
> 1. When a `u64`-counter accessor flips to `SignalId`, the corresponding mutators (the methods that previously bumped the counter) gain a leading `ectx: &mut EngineCtx<'_>` parameter and call a private `fire_*_signal(&self, ectx)` helper.
> 2. The fire helper is a no-op when the signal is `SignalId::null()` (composes with D-008 lazy allocation).
> 3. All callers in the panel ecosystem are updated in the same bucket; `&mut EngineCtx` already threads to those callsites.
>
> **Working-memory session: please ratify and propagate citations to B-008 and B-004 (the prior sightings of this pattern).**

## Proposed D-008-signal-allocation-shape

> **Question.** Where and when is the `SignalId` allocated, given that `emFileManModel::Acquire(ctx: &Rc<emContext>)` and `emFileManViewConfig::Acquire(ctx)` have no scheduler access (`emContext` does not expose one)?
>
> **Options considered.**
>
> - **A1. Lazy allocation by first subscriber.** Field is `Cell<SignalId>` initialized `null`. Each consumer's first-Cycle init block calls a new `&mut self` method `Ensure*Signal(ectx) -> SignalId` that allocates on first call and caches the id thereafter. Mutators check `if !sig.is_null() { ectx.fire(sig) }`; a no-op when no subscriber has yet connected, matching C++ `emSignal::Signal()` with zero subscribers.
> - **A2. Eager allocation at Acquire.** Change `Acquire(ctx) → Acquire(ctx, scheduler)`. Bigger ripple: every Acquire callsite for FMModel and ViewConfig (8+ panels each) updates. Also: `set_behavior`/`new()` panel construction paths likely lack scheduler access (same constraint that drove D-006 first-Cycle init).
> - **A3. Expose scheduler through `emContext`.** Add `Rc<RefCell<Scheduler>>` (or equivalent) to `emContext`. Models call `ctx.scheduler().create_signal()` at Acquire time. Substantial framework change, out of B-009 natural scope.
>
> **Chosen direction.** **A1. Lazy allocation by first subscriber.**
>
> **Why.** Self-contained within B-009. Mirrors the D-006 first-Cycle init shape symmetrically — subscribers init, allocation occurs as a side-effect of init. The "no-op fire when no subscriber" semantic is C++-equivalent: `emSignal::Signal()` with zero subscribers is observably a no-op. A2 has a wider blast radius and conflicts with the construction-time scheduler-availability constraint that already forced D-006. A3 is the right long-term shape but out of scope for this bucket.
>
> **Relationship to D-006.** D-006 picks the consumer-side wiring shape (first-Cycle init + IsSignaled in Cycle). D-008 picks the producer-side allocation shape (lazy on first `Ensure*Signal` call). The two compose: the very same first-Cycle init block that wires the subscribe also triggers the allocation.
>
> **Operational rule.**
> 1. When a `u64`-counter accessor flips to `SignalId`, the model gains:
>    - a private `Cell<SignalId>` field initialized to `SignalId::null()` (replacing the old `Rc<Cell<u64>>` counter),
>    - a public `&self` `Ensure*Signal(ectx: &mut EngineCtx) -> SignalId` method that allocates lazily and caches via `Cell::set`,
>    - a private `fire_*_signal(&self, ectx: &mut EngineCtx)` helper that no-ops when null.
> 2. The accessor (`Get*Signal(&self) -> SignalId`) returns the cached id; `is_null()` until `Ensure*Signal` is called.
> 3. Each consumer's first-Cycle init calls `Ensure*Signal` on the model before `ectx.connect(...)`.
>
> **Future-candidate watch-list (not a decision).** Once enough models carry `SignalId::default()`/`null()` placeholders to justify a framework lift, the working-memory session may revisit A3. Current placeholder occupants: emFileLinkModel, emFileManTheme, emFileManConfig, emFileModel.
>
> **Working-memory session: please ratify alongside D-007 during reconciliation.**

## Cross-bucket interaction

**B-005 → B-009 (downstream simplification).** B-005's `emFileManControlPanel` first-Cycle init block contains:

```rust
ectx.connect(fmm.GetSelectionSignal(), eid);   // see D-001 — accessor returns u64 today; flip pending
ectx.connect(fmc.GetChangeSignal(), eid);      // see D-001 — accessor returns u64 today; flip pending
```

B-005's design doc explicitly flagged these as a hard prereq on D-001 (B-005 "Open item 2"). After B-009 ships:

- `emFileManModel::GetSelectionSignal()` returns `SignalId`.
- `emFileManViewConfig::GetChangeSignal()` returns `SignalId`.
- B-005's `connect` calls compile and fire correctly.
- The `// see D-001 …` annotations become obsolete.
- B-005's hard-prereq-on-D-001 edge resolves to "satisfied by B-009."

**Reconciliation work for the working-memory session:**
1. Strike the `// see D-001 …` annotations from B-005's design doc on the two affected lines.
2. Update inventory-enriched.json edge: B-005's D-001 hard prereq → satisfied-by-B-009.
3. Note that B-005 and B-009 can land in either order at the implementation stage, but B-005's hard-prereq tightening implies B-009 lands first.

No row reassignment between B-005 and B-009.

**B-009 inbound prereqs.** None.

## Audit-data corrections

- **emFileManControlPanel-522.** Bucket sketch row table line 27 and Open Question 5 describe a "sub-engine" routing for `GetCommandsSignal`. C++ `src/emFileMan/emFileManControlPanel.cpp:522` is `AddWakeUpSignal(FMModel->GetCommandsSignal())` directly on the panel's own engine, with `IsSignaled(FMModel->GetCommandsSignal())` at cpp:533 in the same panel's `Cycle()`. **No sub-engine.** Bucket sketch should be updated during reconciliation.

## File-by-file plan

> **SUPERSEDED post-merge (2026-04-28, 50994e26):** Two API shapes below are stale; cluster convention applies to all future "no-acc" / D-001-flip designs.
>
> 1. **D-008 A1 accessor form: split → combined.** All `Ensure*Signal(&self, ectx) -> SignalId` + `Get*Signal(&self) -> SignalId` pairs below were implemented as a **single combined accessor** `Get*Signal(&self, ectx: &mut impl SignalCtx) -> SignalId` (folds lazy-alloc into the call). Per D-008 A1 amendment from B-003 merge `eb9427db`; re-applied at B-014 and B-009. Future bucket designers: write the combined form directly.
> 2. **D-007 mutator signature: `&mut EngineCtx<'_>` → `&mut impl SignalCtx`.** All `(&mut self, ectx: &mut EngineCtx<'_>, …)` signatures below were implemented with the broader trait bound `&mut impl SignalCtx`. Forced because `PanelBehavior::Input` only receives `PanelCtx` (no `EngineCtx`); both `EngineCtx` and `SchedCtx` impl `SignalCtx`. Future bucket designers: write the trait bound directly.
>
> Blocks below preserved as historical record.

### `crates/emfileman/src/emFileManModel.rs` — 2 accessor rows + mutator threading

Replace counter field with signal field:

```rust
// Before
selection_generation: Rc<Cell<u64>>,
commands_generation: Rc<Cell<u64>>,

// After
selection_signal: Cell<SignalId>,   // D-008: null until first subscriber.
commands_signal: Cell<SignalId>,
```

`Acquire` initializer:

```rust
selection_signal: Cell::new(SignalId::null()),
commands_signal: Cell::new(SignalId::null()),
```

Replace private bumpers with private fire helpers + public ensure helpers:

```rust
fn fire_selection_signal(&self, ectx: &mut EngineCtx<'_>) {
    let s = self.selection_signal.get();
    if !s.is_null() { ectx.fire(s); }
}

fn fire_commands_signal(&self, ectx: &mut EngineCtx<'_>) {
    let s = self.commands_signal.get();
    if !s.is_null() { ectx.fire(s); }
}

pub fn EnsureSelectionSignal(&self, ectx: &mut EngineCtx<'_>) -> SignalId {
    let s = self.selection_signal.get();
    if s.is_null() {
        let new_id = ectx.create_signal();
        self.selection_signal.set(new_id);
        new_id
    } else { s }
}

pub fn EnsureCommandsSignal(&self, ectx: &mut EngineCtx<'_>) -> SignalId {
    let s = self.commands_signal.get();
    if s.is_null() {
        let new_id = ectx.create_signal();
        self.commands_signal.set(new_id);
        new_id
    } else { s }
}
```

Accessor flip:

```rust
pub fn GetSelectionSignal(&self) -> SignalId { self.selection_signal.get() }
pub fn GetCommandsSignal(&self) -> SignalId { self.commands_signal.get() }
```

Mutator signature changes (D-007):

| Method | New signature |
|---|---|
| `SelectAsSource` | `(&mut self, ectx: &mut EngineCtx<'_>, path: &str)` |
| `DeselectAsSource` | `(&mut self, ectx: &mut EngineCtx<'_>, path: &str)` |
| `ClearSourceSelection` | `(&mut self, ectx: &mut EngineCtx<'_>)` |
| `SelectAsTarget` | `(&mut self, ectx: &mut EngineCtx<'_>, path: &str)` |
| `DeselectAsTarget` | `(&mut self, ectx: &mut EngineCtx<'_>, path: &str)` |
| `ClearTargetSelection` | `(&mut self, ectx: &mut EngineCtx<'_>)` |
| `SwapSelection` | `(&mut self, ectx: &mut EngineCtx<'_>)` |
| `UpdateSelection` | `(&mut self, ectx: &mut EngineCtx<'_>)` |
| `HandleIpcMessage` | `(&mut self, ectx: &mut EngineCtx<'_>, args: &[&str])` |
| `set_command_root` | `(&mut self, ectx: &mut EngineCtx<'_>, root: CommandNode)` |

Each formerly-bumping body replaces `self.bump_*_generation()` with `self.fire_*_signal(ectx)`.

`SetShiftTgtSelPath` does not bump and keeps its current signature.

### `crates/emfileman/src/emFileManViewConfig.rs` — 1 accessor row + mutator threading

Replace counter field with signal field:

```rust
change_signal: Cell<SignalId>,   // was: change_generation: Cell<u64>
```

Add the symmetric helpers:

```rust
fn fire_change_signal(&self, ectx: &mut EngineCtx<'_>) {
    let s = self.change_signal.get();
    if !s.is_null() { ectx.fire(s); }
}

pub fn EnsureChangeSignal(&self, ectx: &mut EngineCtx<'_>) -> SignalId {
    let s = self.change_signal.get();
    if s.is_null() {
        let new_id = ectx.create_signal();
        self.change_signal.set(new_id);
        new_id
    } else { s }
}

pub fn GetChangeSignal(&self) -> SignalId { self.change_signal.get() }
```

Mutator signature changes (D-007), all six setters:

| Method | New signature |
|---|---|
| `SetSortCriterion` | `(&mut self, ectx: &mut EngineCtx<'_>, sc: SortCriterion)` |
| `SetNameSortingStyle` | `(&mut self, ectx: &mut EngineCtx<'_>, nss: NameSortingStyle)` |
| `SetSortDirectoriesFirst` | `(&mut self, ectx: &mut EngineCtx<'_>, b: bool)` |
| `SetShowHiddenFiles` | `(&mut self, ectx: &mut EngineCtx<'_>, b: bool)` |
| `SetThemeName` | `(&mut self, ectx: &mut EngineCtx<'_>, name: &str)` |
| `SetAutosave` | `(&mut self, ectx: &mut EngineCtx<'_>, b: bool)` |

Each formerly-bumping body replaces `self.bump_generation()` with `self.fire_change_signal(ectx)`. The `write_back_if_autosave` call inside these setters does not need ectx for itself — it cascades into `emFileManConfig::Set*`, whose `SignalId` plumbing is already correct (separate F010 concern, not B-009 scope).

### `crates/emfileman/src/emDirEntryAltPanel.rs` — 2 rows

`emDirEntryAltPanel-35` (Selection) + `emDirEntryAltPanel-36` (Change).

Add field: `subscribed_init: bool` (initialize false in constructor).

In `Cycle`, before existing body:

```rust
if !self.subscribed_init {
    let eid = ectx.id();
    let sel_sig = self.file_man.borrow().EnsureSelectionSignal(ectx);
    let chg_sig = self.config.borrow().EnsureChangeSignal(ectx);
    ectx.connect(sel_sig, eid);
    ectx.connect(chg_sig, eid);
    self.subscribed_init = true;
}

let sel_sig = self.file_man.borrow().GetSelectionSignal();
let chg_sig = self.config.borrow().GetChangeSignal();
if ectx.IsSignaled(sel_sig) {
    // mirrors C++ emDirEntryAltPanel.cpp:85 — repaint/relayout reaction.
}
if ectx.IsSignaled(chg_sig) {
    // mirrors C++ emDirEntryAltPanel.cpp:88 — replaces u64 polling at .rs:160.
}
```

Delete any `cached_*_gen: Cell<u64>` fields and their polling code. Update mutator callsites in this file (none expected for AltPanel — re-verify).

### `crates/emfileman/src/emDirEntryPanel.rs` — 2 rows

`emDirEntryPanel-55` (Selection) + `emDirEntryPanel-56` (Change). Same shape as DirEntryAltPanel; reactions mirror C++ emDirEntryPanel.cpp:152 (selection) and cpp:155 (`forceRelayout=true` on change). Mutator callsites: many `fm.SelectAsTarget`, `fm.ClearSourceSelection`, `fm.SwapSelection`, `fm.DeselectAsTarget` calls in `Input` (lines 324–411) — all already inside `Input` body which has `&mut EngineCtx`. Add `ectx` argument at each call.

### `crates/emfileman/src/emDirPanel.rs` — 1 row

`emDirPanel-38` (Change). First-Cycle init for change_signal only. Reaction mirrors C++ emDirPanel.cpp:78. Replaces u64 polling at .rs:331. Mutator callsites: `fm.SelectAsTarget` at .rs:172 inside `Input`. Add `ectx`.

### `crates/emfileman/src/emDirStatPanel.rs` — 1 row

`emDirStatPanel-39` (Change). Cycle currently never reads the change signal (sketcher noted: "Config acquired in new() but Cycle never reads"). Add first-Cycle init + IsSignaled branch for change_signal. Reaction mirrors C++ emDirStatPanel.cpp:61. No mutator callsites in this file.

### `crates/emfileman/src/emFileLinkPanel.rs` — 1 row

`emFileLinkPanel-55` (Change). Same situation as emDirStatPanel — Cycle currently never reads. Add first-Cycle init + IsSignaled branch. Reaction mirrors C++ emFileLinkPanel.cpp:95. No mutator callsites in this file. (Note: this is a different row from B-005's `emFileLinkPanel-53`, which targets the file model's UpdateSignal — distinct accessor.)

### `crates/emfileman/src/emFileManControlPanel.rs` — 3 rows

`emFileManControlPanel-326` (Selection) + `-327` (Change) + `-522` (Commands). The first-Cycle init connects all three:

```rust
if !self.subscribed_init {
    let eid = ectx.id();
    let sel_sig = self.file_man.borrow().EnsureSelectionSignal(ectx);
    let cmd_sig = self.file_man.borrow().EnsureCommandsSignal(ectx);
    let chg_sig = self.config.borrow().EnsureChangeSignal(ectx);
    ectx.connect(sel_sig, eid);
    ectx.connect(cmd_sig, eid);
    ectx.connect(chg_sig, eid);
    self.subscribed_init = true;
}
```

Cycle body adds three `IsSignaled` branches in C++ source order (cpp:366 selection, cpp:367 change, cpp:533 commands). Reactions:
- Selection (cpp:366): `UpdateButtonStates` (currently absent in Rust — add).
- Change (cpp:367): replaces u64 polling at .rs:305.
- Commands (cpp:533): mirrors C++ commands-react block (no sub-engine; same panel).

Mutator callsites in this file: numerous (`self.config.borrow_mut().SetSortCriterion(ectx, sc)`, `…SetThemeName(ectx, &name)`, `self.file_man.borrow_mut().ClearTargetSelection(ectx)`, `…SwapSelection(ectx)`, etc.) — all inside `Input` body. Add `ectx` argument at each call.

**Coordination with B-005.** B-005 also touches this panel's `Cycle` body for the 20 widget-signal subscribes. After both buckets land, the first-Cycle init block contains B-009's 3 model/config connects + B-005's 20 widget connects (23 total). Implementation order: B-009 first (per B-005's prereq edge); B-005 then drops its `// see D-001 …` annotations and finalizes.

### `crates/emfileman/src/emFileManSelInfoPanel.rs` — 1 row

`emFileManSelInfoPanel-37` (Selection). First-Cycle init for selection_signal only. Reaction mirrors C++ emFileManSelInfoPanel.cpp:54. Replaces u64 polling at .rs:650. Mutator callsite: `panel.file_man.borrow_mut().SelectAsTarget(...)` is in tests only (.rs:886) — production callsites don't appear in this file.

## Implementation ordering

Single PR. Inside the PR, commits in this order so intermediate states compile:

1. **C1 — Add allocation helpers + dual accessors.** Add `Cell<SignalId>` fields, `Ensure*Signal` helpers, `fire_*_signal` helpers. Keep existing `u64` accessors temporarily renamed (e.g., `GetSelectionSignal_u64`) so consumers still compile. Tests pass.
2. **C2 — Flip accessor return types.** `Get*Signal` returns `SignalId`. Compile breaks at every consumer; resolved by C3+.
3. **C3–C13 — Per-consumer migrations** (11 commits, one per audit row). Each commit: add `subscribed_init` flag (if absent), first-Cycle init, IsSignaled branch, delete obsolete `cached_*_gen` field, update mutator callsites in this panel to pass `ectx`.
4. **C14 — Mutator signature flip.** Change `emFileManModel`/`emFileManViewConfig` mutators to take `&mut EngineCtx<'_>`. All in-bucket panel callsites already touched in C3–C13; this commit adds the parameter at the call sites in lockstep with the model-side signature change.
5. **C15 — Delete legacy fields.** Remove `selection_generation`, `commands_generation`, `change_generation`, the temporary `*_u64` accessor aliases from C1, and any escaped `cached_*_gen` consumer fields.

Alternative: fold mutator flip (C14) into each per-consumer commit (C3–C13) so each row's diff is self-contained. Implementer's choice. Hard constraints: **the type-flip commit (C2) and the legacy-field-delete commit (C15) must each be atomic** to avoid cascading compile failures.

## Verification strategy

Behavioral tests in `crates/emfileman/tests/typemismatch_b009.rs` (RUST_ONLY: dependency-forced — no C++ test analogue; the C++ test surface is X11 integration). Per-row pattern:

```rust
// Group 1 — Selection (4 rows: AltPanel-35, EntryPanel-55, ControlPanel-326, SelInfoPanel-37)
let mut h = Harness::new();
let panel = h.create::<emFileManSelInfoPanel>();
h.run_cycle();  // first-Cycle init runs, signal allocated, connected
let model = emFileManModel::Acquire(&h.ctx);
model.borrow_mut().SelectAsSource(h.ectx_mut(), "/foo");
h.run_cycle();
assert!(panel.borrow().was_signaled_this_cycle());

// Group 2 — Change (6 rows): mutate via emFileManViewConfig::SetSortCriterion, assert reaction fires.
// Group 3 — Commands (1 row): mutate via emFileManModel::set_command_root, assert reaction fires.
```

Per row, the test asserts:
- (a) `Ensure*Signal` returned a non-null id after first Cycle.
- (b) Consumer's Cycle observed `IsSignaled(sig) == true` after the mutator.
- (c) Consumer's reaction (state change, repaint flag, IsSignaled-driven mutator call) fired in the same Cycle.

Regression test: a mutator call **before any subscriber wires up** is a clean no-op, proving the `if !sig.is_null()` guard in `fire_*_signal` matches C++ `emSignal::Signal()`-with-zero-subscribers semantics.

The harness reuses `emcore::emEngineCtx::PanelCtx`-with-real-scheduler fixtures already in use in B-005's test plan (`crates/emcore/src/emCheckButton.rs:522`-style).

`cargo clippy -D warnings` and `cargo-nextest ntr` pass.

**Before/after evidence per row** is the audit's four-question standard:
1. Is the signal connected? — `Ensure*Signal` + `connect` in first-Cycle init.
2. Does Cycle observe it? — `IsSignaled(sig)` branch.
3. Does the reaction fire the documented mutator? — branch body matches C++.
4. Is the C++ `IsSignaled` branch order preserved? — code review against C++ Cycle.

Tests answer (1)–(3); code review against C++ answers (4).

## Success criteria

- All 3 accessor rows return `SignalId`; their backing storage is `Cell<SignalId>` lazy-allocated via `Ensure*Signal`.
- All 11 consumer rows have a `connect(...)` call in their panel's first-Cycle init block.
- All 11 consumer rows have a corresponding `IsSignaled(...)` branch in their panel's Cycle body, in C++ source order.
- All formerly-bumping mutators on `emFileManModel` and `emFileManViewConfig` accept `ectx: &mut EngineCtx<'_>` and call the appropriate `fire_*_signal(ectx)` helper.
- Obsolete `cached_*_gen: Cell<u64>` consumer fields are deleted.
- Legacy `selection_generation` / `commands_generation` / `change_generation` model fields and any temporary `*_u64` accessor aliases are deleted.
- New `tests/typemismatch_b009.rs` covers all 14 rows; every assertion passes.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- B-009 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation).

## Open items deferred to working-memory session

1. **Promote D-007-mutator-fire-shape** into `decisions.md` with stable id; back-propagate citations to B-008 and B-004 (prior sightings) during reconciliation.
2. **Promote D-008-signal-allocation-shape** into `decisions.md` alongside D-007.
3. **A3 future-candidate watch-list note.** "Once enough models carry `SignalId::default()/null()` placeholders to justify it, consider exposing scheduler access through `emContext`. Current placeholders: emFileLinkModel, emFileManTheme, emFileManConfig, emFileModel." Not a decision; a watch-list note attached to D-008.
4. **B-005 cross-bucket simplification.** B-005's `emFileManControlPanel` `// see D-001 …` annotations on the `connect(GetSelectionSignal, …)` and `connect(GetChangeSignal, …)` lines become obsolete after B-009 lands. B-005's hard-prereq-on-D-001 edge (B-005 "Open item 2") resolves to "satisfied by B-009." Update B-005 design doc and inventory-enriched.json edge during reconciliation.
5. **Audit-data correction.** Bucket sketch row table line 27 (`emFileManControlPanel-522`) and Open Question 5 describe a "sub-engine" routing for `GetCommandsSignal` that does not exist. C++ `src/emFileMan/emFileManControlPanel.cpp:522` is `AddWakeUpSignal(FMModel->GetCommandsSignal())` directly on the panel's own engine. Update the bucket sketch.
6. **Inventory-enriched.json edges.** B-009 has no inbound prereqs. B-009 → B-005 outbound: B-005's hard prereq on D-001 is satisfied here. No new prereqs introduced.
7. **Within-bucket additions** (not new audit rows, no escalation needed): the three `Ensure*Signal` helpers, the three `fire_*_signal` helpers, the temporary `*_u64` accessor aliases in C1, and the `subscribed_init` fields where absent. All are instrumentation of B-009's own fix.
8. **No out-of-bucket flags surfaced.** Every line touched is either one of the 14 listed rows or a mutator callsite owned by a panel whose row is in this bucket.
