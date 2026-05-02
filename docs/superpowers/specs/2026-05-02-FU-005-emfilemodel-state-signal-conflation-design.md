# FU-005 — emFileModel file-state-signal conflation fix

**Bucket:** [FU-005](../../debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-005-emfilemodel-state-signal-conflation.md)
**Date:** 2026-05-02
**Scope:** `emcore` (emFileModel, emRecFileModel) + `emstocks` comment cleanup.
**Prereqs:** none.

## Summary

The Rust port has two real bugs at the emFileModel/emRecFileModel boundary:

1. **`emRecFileModel::GetFileStateSignal` returns `SignalId::default()` (null)** instead of delegating to the base-class signal. Downstream consumers (emStocksFileModel, emFileLinkModel, any model wrapping emRecFileModel) inherit the null through their delegation chain. `connect`-side null-safety masks the bug — subscriptions are silently inert. C++ consumers react to FileStateSignal at every state-transition; Rust counterparts have the wiring but no fires.

2. **emRecFileModel state-mutation methods fire only `ChangeSignal` via `signal_change(ectx)`, not `FileStateSignal`.** C++ fires both at state-transitions; Rust fires only one. Even with bug 1 fixed (delegation working), the underlying signal still wouldn't fire from emRecFileModel-driven mutations because emRecFileModel mutates `self.state` directly rather than going through `emFileModel<T>::Load`/`complete_load` (which would trigger the base-class fire at `emFileModel.rs:525`).

A naming collision compounds the confusion: both `emFileModel<T>` and `emRecFileModel` have fields named `change_signal`, but they're semantically different signals. Rename the base-class field to `file_state_signal` for clarity.

## Design intent (per Port Ideology)

C++ has two distinct signals:
- `emFileModel::FileStateSignal` (base class) — fires on state transitions.
- `emRecFileModel::ChangeSignal` (derived class) — fires on data-record changes.

The Rust port keeps two distinct signals (`emFileModel<T>::change_signal` and `emRecFileModel.change_signal`). The conflation isn't structural — it's two separate fields that happen to share a name. Naming hygiene + delegation fix + parallel fires resolve the bug; no signal merging or splitting is needed.

**Granularity choice:** C++ fires FileStateSignal at every state-transition (could be multiple per method call). The Rust port already batches ChangeSignal fires once-per-method via `signal_change(ectx)` at the trailing edge of state-mutation methods. We mirror this batching for FileStateSignal — fire once alongside each existing `signal_change` call. Observable difference: a single `Save()` that transitions Unsaved→Saving→Loaded fires FileStateSignal once in Rust vs potentially multiple times in C++. Semantically equivalent for typical reactions (which read current state on wake, not transition count) and consistent with how ChangeSignal already works.

## Work units

Three units, one commit per unit. Phase 1 must merge before Phase 2.

### Unit 1 — Rename + delegate

**Files:** `crates/emcore/src/emFileModel.rs`, `crates/emcore/src/emRecFileModel.rs`.

**Changes in `emFileModel.rs`:**

Rename `emFileModel<T>.change_signal` → `file_state_signal`. Five sites:
- Line 64 (trait `FileModelState::GetFileStateSignal` impl body returning `self.change_signal`).
- Line 117 (struct field declaration).
- Line 137 (ctor body initializing the field).
- Line 168 (`emFileModel<T>::GetFileStateSignal` accessor body).
- Line 525 (`ctx.fire(self.change_signal)` inside the load time-slice driver).

The constructor parameter `signal_id: SignalId` (line 131) and any other locals/params that carry the signal value — verify with grep during implementation; no field-access from outside the file (verified via prior grep showing all `\.change_signal` references in other files refer to *different* `change_signal` fields on unrelated structs).

**Changes in `emRecFileModel.rs`:**

- **Line 366**: replace `SignalId::default()` return with delegation:
  ```rust
  fn GetFileStateSignal(&self) -> SignalId {
      self.file_model.GetFileStateSignal()
  }
  ```
- **Lines 358-368** (the surrounding doc comment block): rewrite to describe the delegation rather than the null workaround. Remove "the standalone-port emRecFileModel" framing — there's no standalone-port choice; just a missing delegate that this fix corrects.

**Behavior change in Phase 1 alone:** subscriptions to `GetFileStateSignal()` through emRecFileModel-derived consumers no longer return null. The signal is real but still doesn't fire from emRecFileModel-driven mutations until Unit 2 lands. Net behavior is unchanged from before (still no fires) — Unit 1 alone is plumbing-only. **Phase 1 must merge before Phase 2** to ensure subscribers connect to the real signal before fires start.

**Tests:** existing unit tests pass. No new tests in Unit 1.

### Unit 2 — Wire FileStateSignal fires at emRecFileModel state-transition sites

**File:** `crates/emcore/src/emRecFileModel.rs`.

**Verification step (Phase 1.5, before implementation):** grep emRecFileModel.rs for state-mutation sites that don't currently call `signal_change(ectx)` but do mutate `self.state`. Spot-check at the time of the brainstorm showed no orphan sites, but verify before implementing in case the file has changed.

**Change pattern:** at every existing `signal_change(ectx)` call site, add a parallel FileStateSignal fire. Recommended factoring: a small helper to avoid duplication:

```rust
fn signal_state_change(&self, ectx: &mut impl SignalCtx) {
    let s = self.file_model.GetFileStateSignal();
    if !s.is_null() {
        ectx.fire(s);
    }
}
```

Then at each existing `signal_change(ectx)` site (currently lines 138, 156, 163, 169, 184, 205, 224, 234, 248 — 9 sites; reverify count before implementing), add a `self.signal_state_change(ectx)` call immediately after.

**Optional alternative pattern:** fold both fires into a renamed helper:

```rust
fn signal_change_and_state(&self, ectx: &mut impl SignalCtx) {
    self.signal_change(ectx);
    self.signal_state_change(ectx);
}
```

Choose during implementation based on whichever produces less churn at call sites.

**Behavior change in Phase 2:** previously-inert FileStateSignal subscriptions become live. Consumers that subscribe via `GetFileStateSignal()` (audited below) now receive wake-ups at emRecFileModel state-transitions.

**Tests:** see "Behavioral risk and test impact" below.

### Unit 3 — Downstream UPSTREAM-GAP comment cleanup

**Files:** `crates/emstocks/src/emStocksFileModel.rs:146-166`, `crates/emstocks/src/emStocksPricesFetcher.rs:71-90,420-435`.

- `emStocksFileModel.rs:149` — replace the UPSTREAM-GAP block (lines 149-162) with a simple delegation doc-comment:
  ```rust
  /// Port of inherited C++ `emFileModel::GetFileStateSignal`. Delegates
  /// to the composed `emRecFileModel<emStocksRec>`, which delegates to
  /// the base `emFileModel<T>::file_state_signal` (FU-005).
  ```
- `emStocksPricesFetcher.rs:78` — drop the UPSTREAM-GAP block; replace with a one-line delegation comment.
- `emStocksPricesFetcher.rs:425` (first-Cycle subscribe) — drop the "currently delegates to a null SignalId per the UPSTREAM-GAP" sentence; the subscribe is now a real wire.

No behavior change in Unit 3.

## Behavioral risk and test impact

**Phase 2 activates previously-inert subscriptions.** Known affected subscribers:

- `emStocksPricesFetcher` — first-Cycle subscribe at `emStocksPricesFetcher.rs:431` (`ectx.connect(state_sig, eid)`). Body is the existing fetcher Cycle, gated by `file_state_ok` and the fetcher state machine (lines 437-449). Activation means the engine wakes more often during state transitions — not new logic, just earlier wakeups. Mirrors C++ behavior (`AddWakeUpSignal(FileModel->GetFileStateSignal())` at `emStocksPricesFetcher.cpp:39`).
- Any `emFileLinkModel` consumer that subscribes to FileStateSignal — verify by grepping `emFileLinkModel` users in the consumer chain. Likely candidates: `emFileLinkPanel`, `emFileMan*` panels.
- `emDirModel` (`emDirModel.rs` `impl FileModelState`) delegates correctly to `self.file_model.GetFileStateSignal()` (the base `emFileModel<T>` directly, not via emRecFileModel). Already returned a non-null real signal pre-fix; behavior unchanged for emDirModel consumers.

**Test-impact audit (mandatory before merging Phase 2):**

- Grep tests for `GetFileStateSignal` and any subscriber-fire-count assertions. Tests that previously passed by relying on the null behavior (e.g., "subscriber receives 0 fires" or "subscription is inert") become wrong assertions after the fix. Update those assertions to match the new — correct — behavior; do not preserve old assertions.
- Tests of `emStocksPricesFetcher` Cycle behavior may see timing changes (more wakeups during state transitions). Verify these tests pass; adjust if they were timing-sensitive.

## Phase ordering

1. **Phase 1 — Unit 1.** Rename + delegate. Plumbing-only; no behavior change. Verify all tests still pass.
2. **Phase 1.5 — Pre-implementation verification.** Grep emRecFileModel.rs for state-mutation sites that don't currently call `signal_change`. Grep emFileLinkModel and downstream for FileStateSignal subscribers. Output informs Unit 2; no commit.
3. **Phase 2 — Unit 2.** FileStateSignal fires at emRecFileModel state-transition sites. **Behavior-changing commit.** Run full nextest before/after; document any test changes in the commit message.
4. **Phase 3 — Unit 3.** Comment cleanup. Trivial.
5. **Phase 4 — Reconciliation.** Update FU-005 bucket file with closure section noting the actual scope (smaller than the bucket file's initial framing). Verify `cargo xtask annotations` clean and full `cargo-nextest ntr` green.

## Acceptance criteria

- `emFileModel<T>.change_signal` field renamed to `file_state_signal` (5 sites).
- `emRecFileModel::GetFileStateSignal` delegates to `self.file_model.GetFileStateSignal()`; null returns removed.
- emRecFileModel state-mutation methods fire FileStateSignal in addition to ChangeSignal at every existing `signal_change(ectx)` call site.
- 3 UPSTREAM-GAP markers in emstocks removed (`emStocksFileModel.rs:149`, `emStocksPricesFetcher.rs:78`, `emStocksPricesFetcher.rs:425`).
- No production caller of `GetFileStateSignal()` anywhere in the consumer chain returns `SignalId::default()` — verified by grep on subscribe sites.
- Tests updated to match new behavior; no test asserts the old null-subscription behavior.
- `cargo-nextest ntr` green; `cargo clippy -D warnings` green; `cargo xtask annotations` clean.

## Out of scope

- `emFileModel<T>` base-class state-mutation method ctx threading (Load, complete_load, etc.). The current "caller fires after" convention is sufficient and consistent with existing usage at `emImageFile.rs:296-300` and the loading time-slice at `emFileModel.rs:525`.
- `emRecFileModel.change_signal` rename — name is semantically correct (it IS the C++ ChangeSignal port). Only the base class's was misnamed.
- Per-state-transition fire granularity matching C++ exactly (multiple fires per method call). The Rust convention of "fire once per method via signal_change" is preserved for both signals.
- Lazy-allocation pattern for `file_state_signal` (it's eager-allocated at `emFileModel<T>::new`; no change).
- New file-state types or transitions beyond what C++ defines.

## References

- C++ source:
  - `~/Projects/eaglemode-0.96.4/include/emCore/emFileModel.h:75,295,313` — FileStateSignal field + accessor.
  - `~/Projects/eaglemode-0.96.4/src/emCore/emFileModel.cpp` — 15 `Signal(FileStateSignal)` fire sites at lines 42, 55, 65, 76, 123, 143, 154, 188, 259, 267, 292, 514, 524, 532, 539.
  - `~/Projects/eaglemode-0.96.4/include/emCore/emRecFileModel.h:50,91,101` — ChangeSignal (separate signal on derived class).
  - `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksPricesFetcher.cpp:39` — `AddWakeUpSignal(FileModel->GetFileStateSignal())` C++ subscribe site.
- Rust source:
  - `crates/emcore/src/emFileModel.rs:117,137,168,525` — current `change_signal` field + accessor + fire site.
  - `crates/emcore/src/emRecFileModel.rs:30-77` — emRecFileModel's separate `ChangeSignal` port.
  - `crates/emcore/src/emRecFileModel.rs:138,156,163,169,184,205,224,234,248` — current `signal_change(ectx)` call sites where Unit 2 adds parallel FileStateSignal fires.
  - `crates/emcore/src/emRecFileModel.rs:366` — null return to fix.
- Bucket file: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-005-emfilemodel-state-signal-conflation.md`.
- Brainstorm scratch: `docs/scratch/2026-05-02-future-work-dump.md`.
