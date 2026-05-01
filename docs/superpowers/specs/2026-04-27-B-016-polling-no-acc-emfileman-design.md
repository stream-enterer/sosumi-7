# B-016-polling-no-acc-emfileman — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-016-polling-no-acc-emfileman.md`
**Pattern:** P-007-polling-accessor-missing
**Scope:** emfileman (`emDirPanel`, `emDirStatPanel`, `emFileLinkPanel`) — 3 rows
**Mechanical-vs-judgement:** mechanical-heavy once the soft prereq lands (B-004 G1 ports `emFilePanel::GetVirFileStateSignal`). The three rows reduce to one D-006 first-Cycle init + one or two `IsSignaled` branches per panel, mirroring the C++ ctors and `Cycle` bodies.

## Goal & scope

Replace per-`Cycle` `vir_file_state` polling in three sibling fileman panels with subscribe-driven Cycle reactions. All three rows apply **D-005-poll-replacement-shape** (direct subscribe; consumer reaction collapsed into the Cycle body) wired through **D-006-subscribe-shape** (first-Cycle init block + `IsSignaled` checks at the top of `Cycle`). The polling code already inside each `Cycle` becomes signal-gated (it runs only when the subscribed signal fires, instead of every frame).

The three rows:

| Row | C++ ref | Rust target | Subscribed signals (per C++ ctor) |
|---|---|---|---|
| `emDirPanel-37` | `emDirPanel.cpp:37` | `emDirPanel.rs:344` (`Cycle`) | `GetVirFileStateSignal()` (cpp:37), `Config->GetChangeSignal()` (cpp:38) |
| `emDirStatPanel-30` | `emDirStatPanel.cpp:30` | `emDirStatPanel.rs:109` (`Cycle`) | `GetVirFileStateSignal()` (cpp:30), `Config->GetChangeSignal()` (cpp:39) |
| `emFileLinkPanel-54` | `emFileLinkPanel.cpp:54` | `emFileLinkPanel.rs:175` (`Cycle`) | `UpdateSignalModel->Sig` (cpp:53), `GetVirFileStateSignal()` (cpp:54), `Config->GetChangeSignal()` (cpp:55), `Model->GetChangeSignal()` (cpp:56) — see scope note |

**Scope note on emFileLinkPanel-54.** The audit row is specifically the *vir-file-state* polling read at `emFileLinkPanel.rs:175` (`refresh_vir_file_state()`). The C++ ctor subscribes to four signals on the surrounding lines; only the `GetVirFileStateSignal` connection is in B-016's row scope. The other three (`UpdateSignalModel->Sig`, `Config->GetChangeSignal`, `Model->GetChangeSignal`) belong to other audit rows or future buckets — this design wires only what row `-54` covers, but documents the surrounding structure so the implementer's first-Cycle init block has the right shape and does not need to be rewritten when the other connections land.

## Decisions cited

- **D-005-poll-replacement-shape** — primary citation for the reaction model. Direct subscribe; the Rust `Cycle` body for each panel is invoked by the engine when the subscribed `vir_file_state_signal` (or `Config->GetChangeSignal`) fires, and re-reads `file_panel.GetVirFileState()` inline. Mirrors C++ `Cycle` shape exactly: subscribe in ctor, `IsSignaled` branches at top of `Cycle`, react.
- **D-006-subscribe-shape** — wiring shape for all three rows. First-Cycle init block calling `ectx.connect(sig, ectx.id())` for each subscribed signal, gated on `subscribed_init: bool`, then `IsSignaled` checks, then the existing reactions. The C++ `AddWakeUpSignal` calls live in the panel ctor; per D-006 §"why first-Cycle init mirrors C++," the Rust port issues the equivalent `connect` calls on the first `Cycle` invocation because `ConstructCtx` does not expose `connect`.

D-001, D-002, D-003, D-004 do not apply: no type-mismatched accessors in scope (the `Config->GetChangeSignal` call sites in this bucket *do* return `u64` per the existing emfileman convention, but B-016's three rows do not subscribe to that accessor — they only *read* it on the side, identical to the existing `last_config_gen` shim. Flipping that accessor's type is owned by the D-001 / B-005 family, not B-016.); no `Rc<RefCell>` shim consumers; no gap-blocked rows once B-004 G1 lands; no stocks panels.

## Soft prereq edge — B-004 G1

B-016's wire requires `emFilePanel::GetVirFileStateSignal() -> SignalId` to exist. **B-004 G1 ports exactly this accessor** (`docs/superpowers/specs/2026-04-27-B-004-no-wire-misc-design.md` §G1, design committed 7:06a 2026-04-27). G1 adds:

- `vir_file_state_signal: SignalId` field on `emFilePanel`
- `GetVirFileStateSignal(&self) -> SignalId` accessor
- Fires on every `last_vir_file_state` mutation (in `SetFileModel`, `set_custom_error`, `clear_custom_error`, and `cycle_inner`)
- Constructor signature change: `emFilePanel::new(cc: &mut C: ConstructCtx)`

**Edge:** **B-004 G1 → B-016 (all three rows).** B-016 cannot land until G1 is merged. This is the soft edge flagged in the bucket sketch and confirmed here. If G1 is delayed, B-016 implementer can stage the wiring to land in the same PR by including G1's diff inline; bucket sketcher's preference is to land G1 first (it has its own consumer in `emImageFile-117` and is independently shippable).

No other cross-bucket prereqs.

## Audit-data corrections

Re-validation against the actual Rust source plus the B-019 reconciliation note:

1. **The "emDirModel doesn't implement FileModelState" framing is false and is being struck (per bucket-sketch inbound note).** `emDirModel` *does* implement `FileModelState` — `GetFileStateSignal` is at `emDirModel.rs:413`, delegating to `self.file_model.GetFileStateSignal()`. The polling at `emDirPanel.rs:344` is therefore not justified by any missing-trait gap; it is plain drift caused by the absence of `emFilePanel::GetVirFileStateSignal`. B-016's design treats it as drift accordingly. The stale `cleanup-emDirPanel-117` annotation row removed by B-019 carried the false framing; B-016 does not preserve it.

2. **All three rows poll only `vir_file_state`.** Re-reading each `Cycle` body confirms:
   - `emDirPanel.rs:344` — calls `self.dir_model.borrow().get_file_state()` (which is the same data path as `file_panel.GetVirFileState`'s underlying source, consulted directly to drive `stay_awake`). Also reads `self.config.borrow().GetChangeSignal()` as a `u64` generation counter (not via subscribe).
   - `emDirStatPanel.rs:109` — calls `self.file_panel.refresh_vir_file_state()` then `update_statistics()`. No other polled signals in this row's `Cycle` body.
   - `emFileLinkPanel.rs:175` — calls `self.file_panel.refresh_vir_file_state()`. The other three C++ subscribes (`UpdateSignalModel->Sig`, `Config->GetChangeSignal`, `Model->GetChangeSignal`) are *not* exercised by the current Rust `Cycle` body; the panel does not react to them at all today. They are out of B-016's row scope.

   **Confirmation that none of the three rows requires multi-source subscribe in B-016 scope.** D-005's deferred multi-source open question (resolved for B-015) holds here: `emDirPanel` and `emDirStatPanel` do subscribe to two signals in C++ (vir-file-state + config change), but the bucket sketcher noted only the vir-file-state polling read; the config-change subscribe is functionally shimmed via the `last_config_gen` u64 generation counter (`emDirPanel.rs:331`), which is a separate audit concern owned by the emFileMan config-signal cluster. B-016's three rows are vir-file-state-only.

3. **No accessor reclassifications.** All three rows are correctly tagged `accessor missing` *as of audit time*. Once B-004 G1 lands, the accessor exists; B-016 then proceeds as a P-006-shaped fix (polling, accessor present) on top of the freshly-landed accessor. The bucket retains its P-007 tag because at audit time the accessor was missing and B-016 owns the consumer-side wire that completes the gap-fill.

4. **Sketch open-question §3 — PR staging.** Resolved: **separate PRs.** B-004 G1 lands first as a standalone commit (its own consumer is `emImageFile-117`, in B-004); B-016 lands as a follow-up commit subscribing the three sibling consumers. This matches the natural dependency boundary and avoids cross-bucket PR coupling.

5. **Sketch open-question §4 — `emDirStatPanel` external-wake dependency.** Resolved: **collapsing into subscribe is observably equivalent to C++.** C++ `emDirStatPanel::Cycle` (`emDirStatPanel.cpp:52-65`) is invoked by the C++ scheduler whenever `VirFileStateSignal` or `Config->GetChangeSignal` fires (because both are subscribed in the ctor); it has no other wake source. The Rust port's current "external wake" reliance is incidental — Cycle returns `false` and depends on whatever else schedules the panel. After the wire, Cycle is invoked exactly when one of the two subscribed signals fires, identical to C++. No observable behavior change beyond eliminating the per-frame redundant work.

These corrections do not move any rows out of B-016.

## Accessor groups

Following the B-001 / B-008 organising convention. One accessor group covers all three rows.

### G1 — `emFilePanel::GetVirFileStateSignal` (panel-side virtual-file-state broadcast)

**Status.** **Ported by B-004 G1** (soft prereq). B-016 consumes the accessor; it does not port it.

**Accessor surface (post-B-004 G1):**

```rust
impl emFilePanel {
    pub fn GetVirFileStateSignal(&self) -> SignalId { self.vir_file_state_signal }
}
```

**Fires on (per B-004 G1):** `SetFileModel`, `set_custom_error`, `clear_custom_error`, `cycle_inner` (when `last_vir_file_state` mutates). Mirrors C++ `Signal(VirFileStateSignal)` call sites at `emFilePanel.cpp:51,78,87,158,179`.

**Subscribers wired by B-016:** all three rows in the bucket.

## Mandatory `emFilePanel::Cycle` prefix in derived panels (post-2026-05-01 amendment)

**Critical wiring requirement** (resolves Adversarial Review C-1, I-1).

Because the Rust port composes `emFilePanel` as a **field** (`pub(crate) file_panel: emFilePanel`) rather than as a base class, the engine never invokes `<emFilePanel as PanelBehavior>::Cycle` for `emDirPanel`, `emDirStatPanel`, or `emFileLinkPanel`. That `Cycle` is the only production fire site for `VirFileStateSignal` (`crates/emcore/src/emFilePanel.rs:471-509`) and the only drain point for `pending_vir_state_fire` (set by `set_custom_error` / `clear_custom_error` at `emFilePanel.rs:104,116` and by `SetFileModel`).

C++ derived panels start every `Cycle` with `busy = emFilePanel::Cycle()` (`emDirPanel.cpp:74`, `emDirStatPanel.cpp:55`, `emFileLinkPanel.cpp:81`). The Rust composition must reproduce that prefix explicitly. The implementer-of-record is `emImageFileImageFilePanel.rs:211-235` (verified): it runs the four-line preamble at the top of its derived `Cycle`:

```rust
self.file_panel.ensure_vir_file_state_signal(ectx);
self.file_panel.fire_pending_vir_state(ectx);
// ... subscribe-init block (extends existing one) ...
// ... IsSignaled-gated reactions ...
let changed = self.file_panel.cycle_inner();
if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
    ectx.fire(self.file_panel.GetVirFileStateSignal());
}
```

**Every B-016 row's `Cycle` MUST adopt this prefix verbatim.** Without it:
- The subscribe is a dead wire — the signal emitter never runs (C-1).
- `set_custom_error` / `clear_custom_error` in derived `Cycle` bodies (e.g., `emDirPanel.rs:355-361`) flip `pending_vir_state_fire` but the flag is never drained → no fire (I-1).

The `cycle_inner` return value supersedes any local `stay_awake` calculation derived from `observed_state`; the existing per-state busy/awake logic remains as additional `||` terms in the final return only where C++ explicitly OR's them (e.g., C++ `emDirPanel::Cycle` returns `busy` from the base call, with no further OR on observed_state).

## Wiring-shape application (D-006)

### `emDirPanel::Cycle` (row `emDirPanel-37`)

**C++ ref:** ctor `emDirPanel.cpp:37-38` (`AddWakeUpSignal(GetVirFileStateSignal())` + `AddWakeUpSignal(Config->GetChangeSignal())`); reaction `emDirPanel.cpp:71-86` (Cycle: `IsSignaled(GetVirFileStateSignal()) || IsSignaled(Config->GetChangeSignal()) → InvalidatePainting + UpdateChildren + InvalidateChildrenLayout`).

**Rust target:** `crates/emfileman/src/emDirPanel.rs:344` (existing polling site inside `Cycle`).

**B-016 row scope:** the `vir_file_state` subscribe (cpp:37) only. The `Config->GetChangeSignal` subscribe (cpp:38) is *not* in this row's scope; the existing `last_config_gen` u64 generation shim continues unchanged (audit-tracked under the emFileMan config-signal cluster, not B-016).

**Field changes:** **None.** Per Adversarial Review I-2 + M-2: an existing `subscribed_init: bool` flag already lives at `emDirPanel.rs:327` (added by B-009 for the Config->GetChangeSignal subscribe). B-016 **extends** that block with one additional `connect` call. `GetVirFileStateSignal()` returns a plain `SignalId` (eagerly initialized to `SignalId::null()` per `emFilePanel.rs:92`); store nothing — re-fetch each Cycle (idempotent, mirrors the existing `chg_sig` re-call at `emDirPanel.rs:336`).

**Cycle wiring (D-006 first-Cycle init, extending the existing post-B-009 block):**

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
    let eid = ectx.id();

    // (1) MANDATORY emFilePanel::Cycle prefix — see "Mandatory prefix" section above.
    // Mirrors emImageFileImageFilePanel.rs:211-212.
    self.file_panel.ensure_vir_file_state_signal(ectx);
    self.file_panel.fire_pending_vir_state(ectx);

    // Lazy emDirModel registration unchanged (existing block at :320-329).
    if self.dir_model.is_none() {
        // ... existing body ...
    }

    // (2) Extend the EXISTING subscribed_init block at emDirPanel.rs:327.
    //     Do NOT add a new flag — the post-B-009 block already gates Config->GetChangeSignal.
    if !self.subscribed_init {
        // existing: ectx.connect(self.config.borrow().GetChangeSignal(ectx), eid);
        ectx.connect(self.file_panel.GetVirFileStateSignal(), eid); // NEW (B-016)
        self.subscribed_init = true;
    }

    // IsSignaled-gated reaction (C++ emDirPanel.cpp:75-82). Re-fetch SignalId each Cycle.
    let vfs_fired = ectx.IsSignaled(self.file_panel.GetVirFileStateSignal());

    // Config->GetChangeSignal is ALREADY a D-006 subscribe post-B-009 (NOT a u64 shim).
    // Existing IsSignaled branch at emDirPanel.rs:~336 — leave untouched.
    let cfg_changed = ectx.IsSignaled(self.config.borrow().GetChangeSignal(ectx));
    if cfg_changed {
        self.child_count = 0;
    }

    if vfs_fired || cfg_changed {
        // C++ Cycle reaction: InvalidatePainting + UpdateChildren + InvalidateChildrenLayout.
        // The existing observed_state match-arm body already handles the materialization
        // and stay_awake decision; preserve it. Add InvalidatePainting + InvalidateChildrenLayout
        // calls if the existing body does not already cover them (verify against current Rust
        // panel-invalidation API; if absent in Rust, the cycle body's existing behavior is
        // semantically equivalent).
    }

    // Existing observed_state match-arm body preserved (Loaded → update_children,
    // LoadError → set_custom_error, Loading/Waiting → stay_awake). Note that
    // set_custom_error / clear_custom_error inside this body flip
    // pending_vir_state_fire; the (1) prefix above drains it on the NEXT Cycle.
    // For same-Cycle observability, an additional fire_pending_vir_state(ectx)
    // call at the end of this match-arm is permitted (mirrors C++ where
    // VirFileStateSignal would have fired synchronously inside emFilePanel::Cycle).

    // (3) MANDATORY emFilePanel::Cycle suffix — cycle_inner + conditional fire.
    let changed = self.file_panel.cycle_inner();
    if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
        ectx.fire(self.file_panel.GetVirFileStateSignal());
    }
    changed || /* existing stay_awake terms */ false
}
```

**Reaction:** existing `observed_state` match-arm body is preserved verbatim. The win is invocation timing: `Cycle` is now scheduled by the engine when `vir_file_state_signal` fires (i.e., when `emFilePanel::cycle_inner` mutates `last_vir_file_state`, which itself fires inside `emFilePanel::Cycle` per B-004 G1's mutator audit). Per-frame polling is eliminated; the body still re-reads via `dir_model.borrow().get_file_state()` (semantically equivalent to a poll, observably correct now that `Cycle` is signal-driven).

**No `stay_awake` removal.** The current code returns `true` while `Loading`/`Waiting`. Mirror C++: C++ `emFilePanel::Cycle` returns `busy=true` while loading is in progress (its own per-Cycle re-entry until Loaded). Rust's `stay_awake` mirrors this. Keep the existing return-value logic.

### `emDirStatPanel::Cycle` (row `emDirStatPanel-30`)

**C++ ref:** ctor `emDirStatPanel.cpp:30,39` (`AddWakeUpSignal(GetVirFileStateSignal())` + `AddWakeUpSignal(Config->GetChangeSignal())`); reaction `emDirStatPanel.cpp:52-66` (Cycle: `IsSignaled(GetVirFileStateSignal()) → UpdateStatistics + InvalidatePainting`; `IsSignaled(Config->GetChangeSignal()) → InvalidatePainting`).

**Rust target:** `crates/emfileman/src/emDirStatPanel.rs:109` (existing `Cycle` body calling `refresh_vir_file_state` + `update_statistics`).

**B-016 row scope:** the `vir_file_state` subscribe only. No config-signal handling in this row's scope.

**Field changes:** **None.** Existing `subscribed_init` block is at `emDirStatPanel.rs:120` (post-B-009). Extend it.

**Cycle wiring (D-006, with mandatory prefix/suffix):**

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, _ctx: &mut PanelCtx) -> bool {
    let eid = ectx.id();

    // (1) MANDATORY prefix.
    self.file_panel.ensure_vir_file_state_signal(ectx);
    self.file_panel.fire_pending_vir_state(ectx);

    // (2) Extend existing post-B-009 subscribed_init block at emDirStatPanel.rs:120.
    if !self.subscribed_init {
        // existing config subscribe preserved
        ectx.connect(self.file_panel.GetVirFileStateSignal(), eid); // NEW
        self.subscribed_init = true;
    }

    let vfs_fired = ectx.IsSignaled(self.file_panel.GetVirFileStateSignal());
    if vfs_fired {
        // C++ emDirStatPanel.cpp:57-60: UpdateStatistics + InvalidatePainting.
        self.file_panel.refresh_vir_file_state();
        self.update_statistics();
    }

    // (3) MANDATORY suffix.
    let changed = self.file_panel.cycle_inner();
    if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
        ectx.fire(self.file_panel.GetVirFileStateSignal());
    }
    changed
}
```

**Reaction:** preserve the existing `refresh_vir_file_state` + `update_statistics` calls, but only on `vfs_fired`. The redundant per-frame invocation is eliminated. Removing `refresh_vir_file_state` from the unconditional path is safe because B-004 G1's `cycle_inner` is the canonical refresh site; this panel observes the broadcast and re-syncs locally.

### `emFileLinkPanel::Cycle` (row `emFileLinkPanel-54`)

**C++ ref:** ctor `emFileLinkPanel.cpp:53-56` (four subscribes); reaction `emFileLinkPanel.cpp:77-108` (per-signal `IsSignaled` branches → `doUpdate=true` → `UpdateDataAndChildPanel`).

**Rust target:** `crates/emfileman/src/emFileLinkPanel.rs:175` (existing `Cycle` calling `refresh_vir_file_state`).

**B-016 row scope:** the `GetVirFileStateSignal` subscribe (cpp:54) only. The other three C++ subscribes are out of scope — they are not exercised by the current Rust `Cycle` body. The first-Cycle init block in B-016 wires only the vir-file-state signal; the structure is shaped to admit future additions (other three connections added on the same `subscribed_init` flag) without rewrite.

**Field changes:** **None.** Existing `subscribed_init` block at `emFileLinkPanel.rs:186` (post-B-009). Extend.

**Cycle wiring (D-006, with mandatory prefix/suffix + I-3 branch-fidelity restoration):**

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, _ctx: &mut PanelCtx) -> bool {
    let eid = ectx.id();

    // (1) MANDATORY prefix.
    self.file_panel.ensure_vir_file_state_signal(ectx);
    self.file_panel.fire_pending_vir_state(ectx);

    // (2) Extend existing post-B-009 subscribed_init block at emFileLinkPanel.rs:186.
    if !self.subscribed_init {
        // existing config/model/update subscribes preserved
        ectx.connect(self.file_panel.GetVirFileStateSignal(), eid); // NEW (B-016)
        self.subscribed_init = true;
    }

    // (I-3) Per-branch fidelity to C++ emFileLinkPanel.cpp:84-101.
    // 4 distinct branches with 3 distinct flags (do_update, dir_entry_up_to_date,
    // invalidate_layout). Replace the current collapsed `needs_update` flag.
    let vfs_fired = ectx.IsSignaled(self.file_panel.GetVirFileStateSignal());
    if vfs_fired {
        self.file_panel.refresh_vir_file_state();
        // C++ cpp:85-88: InvalidatePainting + doUpdate=true.
    }
    // (UpdateSignalModel, Config, Model branches: out of B-016 row scope but
    // the I-3 fidelity restoration MUST land in the same PR — see I-3 note below.)

    // (3) MANDATORY suffix.
    let changed = self.file_panel.cycle_inner();
    if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
        ectx.fire(self.file_panel.GetVirFileStateSignal());
    }
    changed
}
```

**Reaction:** `refresh_vir_file_state`, signal-gated, with C++-faithful per-branch flag mutations.

### I-3 disposition (emFileLinkPanel branch fidelity)

**Decision:** **Fix the M-001 violation in this PR.** The pre-existing collapse of 4 C++ `IsSignaled` branches into one `needs_update` flag at `emFileLinkPanel.rs:199-222` is a **fidelity bug**, not a forced divergence — there is no language-, dependency-, upstream-gap-, or performance-forced category that justifies merging branches with distinct flag mutations (M-001 explicitly mandates per-branch porting). Per Port Ideology §"Forced divergence", lacking a forced category means the bug is fixed, not annotated. B-016 implementer must restore the 3 distinct flags (`do_update`, `dir_entry_up_to_date`, plus the layout-invalidation case) and the C++ branch shape verbatim, even though only the VFS branch is in B-016's row scope — touching this `Cycle` body without restoring the others would ratify the drift.

If implementer encounters a load-bearing reason to defer (e.g., the other three subscribes aren't yet wired and reading-without-firing the flags is observably indistinguishable today), document it as a **forward-pointer audit row** in the bucket sketch and leave a `// TODO(M-001): restore per-branch flags` marker — but do not mark `DIVERGED:`. A `DIVERGED:` annotation here would be invalid (no forced category applies).

## Implementation sequencing

1. **B-004 G1 lands first** (soft cross-bucket prereq). G1 ports `emFilePanel::vir_file_state_signal` + `GetVirFileStateSignal()` + the four mutator fires. B-004 lands its own consumer (`emImageFile-117`) in the same PR.
2. **B-016 row 1 — `emDirPanel-37`.** Add `vir_file_state_sig` + `subscribed_init` fields; add first-Cycle init block; gate the existing observed_state body on `vfs_fired || cfg_changed`. Leave `last_config_gen` shim untouched. Add a test (see Verification).
3. **B-016 row 2 — `emDirStatPanel-30`.** Add fields + init block; gate `refresh_vir_file_state` + `update_statistics` on `vfs_fired`.
4. **B-016 row 3 — `emFileLinkPanel-54`.** Add fields + init block; gate `refresh_vir_file_state` on `vfs_fired`. Document the three out-of-scope subscribes inline so the future bucket has a turnkey extension point.

Rows 2, 3, and 4 are independent of each other and can land in any order or as one combined PR. They all depend only on row 1 of the prereq (B-004 G1).

## Verification strategy

C++ → Rust observable contract: each polled re-read becomes a signal-driven Cycle invocation; observable behavior identical (state mutation matches; what changes is invocation timing — no longer per-frame).

**Pre-fix observable behavior:**
- All three panels rely on `Cycle` being invoked every frame to detect file-state transitions. With `Cycle` returning `false` and no other wake source on the panel, future scheduler optimizations could starve the polling. The drift is "live behavior accidentally works due to over-scheduling."

**Post-fix observable behavior:** all three panels fire their existing reactions on `vir_file_state_signal` arrival, independent of any other wake source. Matches C++ `Cycle` invocation cadence exactly.

**New test file:** additions to the existing emfileman test suite, or new `crates/emfileman/tests/polling_b016.rs`. RUST_ONLY: dependency-forced — no C++ test analogue, mirrors B-005/B-008/B-015 test rationale.

Test pattern per row:

```rust
// emDirPanel — fire VirFileStateSignal, assert Cycle reacts.
let mut h = Harness::new();
let panel = h.create_dir_panel("/tmp/test");
h.run_cycle(); // first-Cycle init wires the subscribe + lazy emDirModel
let model = panel.borrow().dir_model_for_test().unwrap();
model.borrow_mut().set_state(FileState::Loaded);
// SetFileModel/cycle_inner fires VirFileStateSignal per B-004 G1.
h.run_cycle();
assert_eq!(panel.borrow().child_count_for_test(), expected_count);

// emDirStatPanel — fire VirFileStateSignal, assert update_statistics ran.
let mut h = Harness::new();
let panel = h.create_dir_stat_panel(model);
h.run_cycle(); // init
model.borrow_mut().set_state(FileState::Loaded);
h.fire(panel.borrow().file_panel.GetVirFileStateSignal());
h.run_cycle();
assert_eq!(panel.borrow().stats_for_test().total_count, model_total);

// emFileLinkPanel — fire VirFileStateSignal, assert refresh_vir_file_state ran.
let mut h = Harness::new();
let panel = h.create_file_link_panel(model);
h.run_cycle();
model.borrow_mut().set_state(FileState::Loaded);
h.fire(panel.borrow().file_panel.GetVirFileStateSignal());
h.run_cycle();
assert!(matches!(panel.borrow().file_panel.GetVirFileState(), VirtualFileState::Loaded));
```

**Four-question audit-trail evidence per row:** (1) signal connected? — D-006 init block calls `ectx.connect(GetVirFileStateSignal(), ectx.id())`. (2) Cycle observes? — `IsSignaled` branch on `vir_file_state_sig`. (3) reaction fires documented mutator? — assertions above (`update_statistics`, `update_children`, `refresh_vir_file_state`). (4) C++ branch order preserved? — code review against C++ `emDirPanel::Cycle` (cpp:71-86), C++ `emDirStatPanel::Cycle` (cpp:52-66), C++ `emFileLinkPanel::Cycle` (cpp:77-108).

## Cross-bucket prereq edges

- **B-004 G1 → B-016 (all three rows). Soft, blocking.** B-016 cannot land until `emFilePanel::GetVirFileStateSignal` exists. B-004 G1 ports it and is independently shippable. If G1 slips, B-016 implementer either waits or merges G1's diff inline.
- **B-016 has no outbound prereq edges.** No other bucket depends on B-016 completing.

## Out-of-scope adjacency

- **`Config->GetChangeSignal` subscribes (cpp:38, cpp:39, cpp:55).** Not in B-016's row scope. The existing `last_config_gen` u64 shim in `emDirPanel.rs` is functionally correct (re-checks per Cycle; works because the panel is awake whenever its surroundings are). Replacing it with a true subscribe belongs to the emFileMan config-signal cluster (D-001 family).
- **`emFileLinkPanel`'s `UpdateSignalModel->Sig` and `Model->GetChangeSignal` subscribes (cpp:53, cpp:56).** Not in B-016's row scope. The Rust panel currently does not react to either; the future bucket that wires them adds two `connect` calls to the same `subscribed_init` block plus two `IsSignaled` branches in `Cycle`. B-016's structure is shaped to absorb these additions without rewrite.
- **`emDirPanel::stay_awake` semantics during Loading/Waiting.** Preserved as-is. Returning `true` from Cycle while loading is in progress mirrors C++ `busy=emFilePanel::Cycle()` returning `true` during loading. Below-surface; no annotation.

## Open questions for the implementer

1. ~~**`emFilePanel::SetFileModel` signature.**~~ **STRUCK 2026-05-01 (M-1).** Resolved by B-004 emcore-slice merge (commit 9b8ee012): `emFilePanel::new()` signature unchanged; `SetFileModel` uses the deferred `pending_vir_state_fire` flag path. No caller change required.
2. **`InvalidatePainting` + `InvalidateChildrenLayout` parity.** C++ `emDirPanel::Cycle` calls `InvalidatePainting()` and `InvalidateChildrenLayout()` after a vir-file-state or config-change fire. Rust panel infra may or may not require explicit invalidation calls (some ports do this implicitly via the engine's dirty-tracking). Verify against the existing Rust panel invalidation API (look at `emPanel::InvalidatePainting` / `InvalidateChildrenLayout` if they exist; if absent, the engine's existing wake-on-Cycle dirty marking covers it). If absent in Rust, no annotation needed — below-surface.
3. ~~**`vir_file_state_sig: Option<SignalId>` vs `SignalId`.**~~ **STRUCK 2026-05-01 (M-2).** Resolved: store **no field** at all. `GetVirFileStateSignal()` returns plain `SignalId` (eagerly nulled at construction per `emFilePanel.rs:92`); re-fetch each Cycle (idempotent, mirrors `chg_sig` re-call at `emDirPanel.rs:336`). Caching `Option<SignalId>` invites two failure modes (cache-of-null staleness if Cycle order swaps, reallocation aliasing) — both eliminated by the no-cache shape.
4. **Test harness fire helpers.** B-005's harness exposes `Harness::fire(SignalId)`. Confirm the harness exposes a way to fire `vir_file_state_signal` indirectly via a model state mutation (the natural path: `model.set_state(...)` triggers `cycle_inner` which fires the signal per B-004 G1). The test pattern above assumes this; if the indirect path is awkward, fall back to direct `h.fire(panel.borrow().file_panel.GetVirFileStateSignal())`.

## Open items deferred to working-memory session

1. **No new D-### proposed.** This bucket reuses D-005 (reaction model: direct subscribe) and D-006 (wiring shape: first-Cycle init + IsSignaled at top of Cycle). The accessor port itself is owned by B-004 G1 (already designed) and falls under D-003 option A. No global decision is surfaced.
2. **D-005 open question §1 (subscribe-arity per consumer).** Confirmed: each B-016 consumer subscribes to a single `GetVirFileStateSignal` for its row scope, mirroring C++ exactly. C++ `emDirPanel`/`emDirStatPanel` ctors do also subscribe to `Config->GetChangeSignal` — out of B-016's row scope but documented above so the future bucket extends the same first-Cycle init block instead of rewriting it.
3. **B-019 inbound note honored.** The bucket-design does not preserve the "emDirModel doesn't implement FileModelState" framing (which was false; see Audit-Data Corrections §1). Working-memory session may strike that line from any remaining audit notes that still cite it.
4. **Cross-bucket prereq edge B-004 G1 → B-016 (all three rows).** Add to `work-order.md` DAG. B-016 cannot reach `merged` before B-004 G1's accessor is in tree. No row reclassifications.
5. **No row reclassifications.** All three rows verified `accessor missing` at audit time and `accessor present (post-G1)` at execution time. Bucket retains its P-007 tag because the audit-time accessor status defines the bucket; the post-G1 P-006 shape is the implementation reality.

## Success criteria

- `emDirPanel`, `emDirStatPanel`, and `emFileLinkPanel` each subscribe to `emFilePanel::GetVirFileStateSignal()` in a D-006 first-Cycle init block.
- Each panel's `Cycle` body runs its existing reaction (observed_state match-arm / `update_statistics` / `refresh_vir_file_state`) only when the subscribed signal fires (or, for `emDirPanel`, when the existing `last_config_gen` shim flips).
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New tests cover: each of the three panels' subscribe + signal-driven reaction.
- B-016 status in `work-order.md` flips `pending → designed` (working-memory session reconciliation), and per-row commits flip to `merged` as they land (after B-004 G1 lands).

## Adversarial Review — 2026-05-01

### Summary
- Critical: 1 | Important: 3 | Minor: 2 | Notes: 2

### Findings

**[Critical] C-1 — `vir_file_state_signal` is never fired from these three panels; the proposed subscribe is a dead wire.**

Verified at `crates/emcore/src/emFilePanel.rs:471-509`: the only production fire site for `VirFileStateSignal` is inside `<emFilePanel as PanelBehavior>::Cycle` (calls `fire_pending_vir_state` + `cycle_inner` and fires on transition). In the three B-016 targets, `emFilePanel` is held as a **field** (`pub(crate) file_panel: emFilePanel`), not as a base class — so `<emFilePanel as PanelBehavior>::Cycle` is never invoked by the engine for these panels. Confirmed by grep: only `emImageFileImageFilePanel.rs:212,232` calls `file_panel.fire_pending_vir_state(ectx)` + `file_panel.cycle_inner()` from its derived Cycle. The three B-016 panels do **not** make those calls today (they call `refresh_vir_file_state()` which only mutates `last_vir_file_state` — no signal fire, no pending flag).

C++ derived Cycle starts with `busy=emFilePanel::Cycle()` (verified: `emDirPanel.cpp:74`, `emDirStatPanel.cpp:55`, `emFileLinkPanel.cpp:81`); that call site is what fires `VirFileStateSignal` in C++. The Rust composition pattern silently drops it.

Consequence: subscribing to `self.file_panel.GetVirFileStateSignal()` in `emDirPanel::Cycle` will never fire. The `vfs_fired` branch is unreachable; the panel reverts to its current "alive only because of external wakes" behavior, defeating the bucket's stated goal. The design's "Reaction" section asserts `Cycle is now scheduled by the engine when vir_file_state_signal fires (i.e., when emFilePanel::cycle_inner mutates last_vir_file_state, which itself fires inside emFilePanel::Cycle per B-004 G1's mutator audit)" — that fire chain is broken in composition mode.

Required fix: each B-016 panel's `Cycle` must, at top, call `self.file_panel.fire_pending_vir_state(ectx)` and invoke `self.file_panel.cycle_inner()` (mirroring `emImageFilePanel` precedent at `emImageFileImageFilePanel.rs:212,232`, which is the implementer-of-record for the embedded-file_panel pattern). Without this, the subscribe is connected to a signal whose emitter is never run.

**[Important] I-1 — Design omits mutator-callsite enumeration for `SetFileModel`/`set_custom_error`/`clear_custom_error`.**

`emDirPanel.rs:355-361` (visible in current Cycle) calls `self.file_panel.clear_custom_error()` and `self.file_panel.set_custom_error(e)` directly. Both set `pending_vir_state_fire = true` but do not fire (verified at `emFilePanel.rs:104,116`). The pending flag is drained only inside `<emFilePanel as PanelBehavior>::Cycle` via `fire_pending_vir_state(ectx)`. Same composition gap as C-1: the pending flag is set, never drained → never fires. The design must mandate calling `file_panel.fire_pending_vir_state(ectx)` after every such mutation in the same Cycle invocation (or equivalently at top-of-Cycle every tick), not just at first-Cycle init. Currently neither the design nor any sibling code does this for the three target panels.

**[Important] I-2 — `emDirPanel::Cycle` row scope conflicts with B-009-merged Config subscribe (already in tree).**

The design (lines 134-140) describes the "existing config-change generation-counter shim" with `last_config_gen: u64` polling. That shim is already replaced: `emDirPanel.rs:325-336` (read above) now uses D-006 first-Cycle init for `Config->GetChangeSignal()` (post-B-009 SignalId-typed accessor with combined-form `GetChangeSignal(ectx)`). Same in `emDirStatPanel.rs:118-126` and `emFileLinkPanel.rs:185-196`. The design's "Cycle wiring" pseudocode introduces a second `subscribed_init` block — there is already exactly one. Implementer must extend the existing block with `connect(GetVirFileStateSignal(), eid)`, not add a new flag/block. Mis-describing the current shape risks duplicate fields and dead code.

**[Important] I-3 — B-015 reconciliation lesson (M-001) not applied: `emFileLinkPanel::Cycle` C++ branch structure is partially preserved.**

Per M-001 (decisions.md line 245), C++ `Cycle` branches must be ported individually. C++ `emFileLinkPanel.cpp:84-101` has 4 distinct `IsSignaled` branches with distinct flag mutations: VFS sets `doUpdate=true`+InvalidatePainting; UpdateSignal sets `DirEntryUpToDate=false`+`doUpdate=true`; Config sets InvalidatePainting+InvalidateChildrenLayout (no doUpdate); Model sets `doUpdate=true`. The Rust port today (lines 199-222) collapses everything into a single `needs_update` flag for VFS+UpdateSignal+Config — losing the `DirEntryUpToDate=false` distinction (UpdateSignal-only) and the "Config does NOT trigger doUpdate" distinction. The B-016 design preserves this collapse rather than fixing it (and B-015's blocker 2 was exactly this kind of design-doc oversight). Add a row note: when wiring VFS, also restore branch fidelity to `cpp:84-101` per M-001.

**[Minor] M-1 — `SetFileModel` open-question §1 is already resolved.**

Open question 1 (line 328) cites B-004 G1 as undecided on `SetFileModel` ectx threading. Resolved per work-order line 393: `emFilePanel::new()` signature unchanged; `SetFileModel` uses deferred `pending_vir_state_fire` flag. Strike the open question and reference work-order entry "B-004 emcore-slice merged" (commit 9b8ee012).

**[Minor] M-2 — `Option<SignalId>` field is unnecessary given B-004's null-aware accessor.**

Open question 3 (line 330) — `GetVirFileStateSignal()` returns `SignalId` directly (eagerly initialized to `SignalId::null()` per `emFilePanel.rs:92`; allocated on first `<emFilePanel as PanelBehavior>::Cycle` via `ensure_vir_file_state_signal`). Storing `Option<SignalId>` adds two failure modes: (a) caching `null` if the Cycle wires connect before `<emFilePanel as PanelBehavior>::Cycle` allocates, and (b) staleness if reallocation ever occurs. Plain `SignalId` field, re-fetched each Cycle (idempotent per B-014 precedent — same as the existing `chg_sig` re-call pattern at `emDirPanel.rs:336`), avoids both. Note: this also depends on resolving C-1 — without `<emFilePanel as PanelBehavior>::Cycle` running, the signal is never allocated regardless.

### Notes

- **N-1 — D-009 compliance: clean.** Proposed wire has no Cell-as-polling-intermediary. The `pending_vir_state_fire` flag inside `emFilePanel` is a per-instance defer-to-own-Cycle drain (D-006 Option B language-forced override, decisions.md:144), not a cross-engine intermediary.
- **N-2 — Verification harness gap.** Test pattern at line 286 calls `model.borrow_mut().set_state(FileState::Loaded)` and expects `VirFileStateSignal` to fire on next Cycle. With C-1 unfixed this fires only inside `<emFilePanel as PanelBehavior>::Cycle`, which the Harness does not invoke for `emDirPanel`. Tests must explicitly drive the embedded `file_panel`'s pending-fire drain (or, post-fix, the new `emDirPanel::Cycle` does).

### Recommended Pre-Implementation Actions

1. **Resolve C-1 before dispatch.** Amend "Wiring-shape application" sections for all three rows to require, at top of `Cycle`: `self.file_panel.fire_pending_vir_state(ectx);` followed by `let changed = self.file_panel.cycle_inner(); if changed { ectx.fire(self.file_panel.GetVirFileStateSignal()); }`. Mirror `emImageFileImageFilePanel.rs:212,232`. Without this, the subscribe is dead.
2. **Update I-2:** Replace "Field changes" pseudocode with "extend existing `subscribed_init` block at `emDirPanel.rs:327` / `emDirStatPanel.rs:120` / `emFileLinkPanel.rs:186` with one additional `ectx.connect(self.file_panel.GetVirFileStateSignal(), eid)` line." No new field, no new flag.
3. **Address I-3:** Add a note that `emFileLinkPanel::Cycle` must restore per-branch flag distinctions per M-001 (4 branches, 3 flags). Alternatively scope this as out-of-bucket if the design refuses, with a forward-pointer audit row.
4. **Strike M-1, M-2 open questions** per resolution above.
5. **Re-read `emImageFileImageFilePanel::Cycle`** as the implementer-of-record reference. Its 4-line preamble (`fire_pending_vir_state` + `cycle_inner` + conditional `ectx.fire`) is the canonical B-004-aware embedded-`file_panel` Cycle prefix; B-016 must adopt it verbatim.

## Amendment Log — 2026-05-01

Adversarial Review findings (above) folded into the design body:

- **C-1 (Critical) resolved.** Added new top-level section "Mandatory `emFilePanel::Cycle` prefix in derived panels (post-2026-05-01 amendment)" before "Wiring-shape application (D-006)". Mandates the `ensure_vir_file_state_signal` + `fire_pending_vir_state` + `cycle_inner` + conditional `ectx.fire` quartet at top/bottom of every B-016 derived `Cycle`, mirroring `emImageFileImageFilePanel.rs:211-235` (verified). All three per-row Cycle wiring blocks rewritten to include items (1) prefix, (2) extend-existing-init, (3) suffix.
- **I-1 (Important) resolved.** Mandatory prefix drains `pending_vir_state_fire` set by `set_custom_error` / `clear_custom_error` (`emFilePanel.rs:104,116`). Added inline note in `emDirPanel` wiring re: same-Cycle drain via an additional `fire_pending_vir_state(ectx)` after the observed_state match-arm if same-tick observability is needed.
- **I-2 (Important) resolved.** All "Field changes" subsections rewritten to **None** — the existing post-B-009 `subscribed_init` block at `emDirPanel.rs:327` / `emDirStatPanel.rs:120` / `emFileLinkPanel.rs:186` is **extended** with one `connect(GetVirFileStateSignal(), eid)` line. No new field, no new flag. Replaced the `last_config_gen` u64 shim language (which described pre-B-009 state) with the actual current `IsSignaled(GetChangeSignal(ectx))` shape.
- **I-3 (Important) resolved — fix not annotate.** Added "I-3 disposition" subsection under `emFileLinkPanel::Cycle`. Decision: **fix the M-001 violation in this PR.** No forced category applies (not language/dependency/upstream-gap/performance-forced); per Port Ideology a `DIVERGED:` annotation would be invalid. Implementer must restore the 3 distinct C++ flags (`do_update`, `dir_entry_up_to_date`, layout-invalidate) and the 4-branch shape from `emFileLinkPanel.cpp:84-101`. A `// TODO(M-001)` marker plus a forward-pointer audit row is the only acceptable defer path; a `DIVERGED:` is not.
- **M-1, M-2 (Minor) struck.** Open questions §1 (SetFileModel ectx threading) and §3 (Option vs plain SignalId) marked struck inline with resolutions. M-1 cites B-004 emcore-slice merge commit `9b8ee012`. M-2 mandates **no field** — re-fetch `GetVirFileStateSignal()` per Cycle (idempotent, sibling of `chg_sig` re-call at `emDirPanel.rs:336`).
- **N-1, N-2 (Notes) acknowledged.** D-009 compliance unchanged. N-2's harness gap is now moot once the mandatory prefix lands: `emDirPanel::Cycle` itself runs `cycle_inner` + fires, so model-state mutations followed by an `emDirPanel` Cycle reach the signal naturally.

Adversarial Review section above is preserved verbatim (no edits).

**Dispatch readiness:** Design is dispatch-ready. C-1 fix is fully specified; I-1/I-2/I-3 each have concrete implementer instructions with file:line targets; M-1/M-2 struck. No deferrals.
