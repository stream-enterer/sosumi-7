# FU-005 — emFileModel file-state-signal conflation fix

> **Origin:** Surfaced 2026-05-02 during the FU-001 brainstorm. The original FU-001 had a "GetFileStateSignal lift" item assuming a small accessor lift; verification showed the actual situation is a **signal-conflation bug spanning emFileModel + emRecFileModel + downstream consumers**, requiring its own brainstorm and consumer audit. Carved out as FU-005.

**Pattern:** Signal-shape fix at emcore base class — separate two C++ signals that the Rust port conflated into one never-fired field; wire firing at all state-transition sites; fix delegation chain in derived classes.
**Scope:** `emcore` (emFileModel, emRecFileModel) + downstream consumer audit (emstocks, possibly emfileman, possibly others).
**Row count:** estimated ~15 C++ fire sites mirror to ~10 Rust state-mutation sites + delegation fixes. Final count produced during brainstorm.
**Prereq buckets:** none.

## Pattern description

C++ has **two semantically distinct signals** on two classes:

- `emFileModel::FileStateSignal` (member: `FileStateSignal`, accessor: `GetFileStateSignal()`) — fires on file-state transitions (Waiting/Loading/Loaded/SaveError/etc). Defined on the **base class**. 15 fire sites in `emFileModel.cpp` (lines 42, 55, 65, 76, 123, 143, 154, 188, 259, 267, 292, 514, 524, 532, 539).
- `emRecFileModel::ChangeSignal` (member: `ChangeSignal`, accessor: `GetChangeSignal()`) — fires on data record changes. Defined on the **derived class** (`emRecFileModel`).

The Rust port has **one signal** field on `emFileModel<T>` (`change_signal: SignalId` at `emFileModel.rs:117`), allocated but **never fired anywhere in production**. `emFileModel::GetFileStateSignal()` returns this never-fired signal. `emRecFileModel::GetFileStateSignal()` (`emRecFileModel.rs:366`) explicitly returns `SignalId::default()` (null) and notes the standalone-port choice. Downstream `emStocksFileModel::GetFileStateSignal()` (`emStocksFileModel.rs:149`) and `emStocksPricesFetcher.rs:78,425` annotate `UPSTREAM-GAP` against the null.

Because nothing fires the signal, current behavior is "no consumer ever observes a file-state transition via the signal" — the wiring exists structurally but is silent. C++ consumers (file panels, fetchers) react to `FileStateSignal`; Rust ports either work around the null (cached path inspection) or have stub reactions awaiting a real signal.

## Work to do (high level — to be refined in brainstorm)

1. **Split the conflated signal at the base class.** Two design options to settle in brainstorm:
   - **(α)** Add a distinct `file_state_signal: SignalId` separate from `change_signal` on `emFileModel<T>`. Leave `change_signal` alone (or delete if it's truly unused — none of the 0 firing sites suggests it might be deletable).
   - **(β)** Rename `change_signal` → `file_state_signal` (matches its `GetFileStateSignal()` accessor), since it's never fired as a "change signal" anyway. Cleaner if no caller depends on the current name.
   
   Decision driver: enumerate every reference to `emFileModel<T>.change_signal` (field access vs accessor use) before choosing.

2. **Wire D-007 ectx-threading at every state-transition site** in `emFileModel.rs`. Approximately 10 sites identified during FU-001 brainstorm:
   - `Load` (`emFileModel.rs:189`) — Waiting/LoadError/TooCostly → Loading.
   - `set_progress` (~200) — Loading progress update.
   - `complete_load` (~207) — Loading → Loaded.
   - `set_load_error` (~213) — Loading → LoadError.
   - `set_too_costly` (~218) — Loading → TooCostly.
   - `mark_unsaved` (~223) — Loaded → Unsaved.
   - `Save` initiator (~232) — Unsaved/SaveError → Saving.
   - `complete_save` (~242) — Saving → Loaded.
   - `set_save_error` (~248) — Saving → SaveError.
   - `HardResetFileState` (~253) — any → Waiting.
   - Possibly more in lines 260-300+; full enumeration during Phase A.
   
   Each site: thread `&mut impl SignalCtx` (or upgrade signature to take it), fire `file_state_signal` at the C++-mirrored point.

3. **Fix `emRecFileModel::GetFileStateSignal`** (`emRecFileModel.rs:366`) to delegate to the base-class signal instead of returning `SignalId::default()`.

4. **Update downstream consumer annotations:**
   - `emStocksFileModel.rs:149` — UPSTREAM-GAP delegate becomes a real accessor.
   - `emStocksPricesFetcher.rs:78,425` — UPSTREAM-GAP comments dropped.
   - Any other emFileModel-derived models (`emFileLinkModel`, etc) audited for the same delegation pattern.

5. **Consumer audit.** Find every Rust site that calls `GetFileStateSignal()` or `connect`s to it. Verify each is correctly subscribed once the signal is real. May surface previously-stubbed reactions that now have meaningful signal behavior to wire.

## Phases (proposed; brainstorm refines)

1. **Phase A — Inventory.** Enumerate all `emFileModel<T>.change_signal` references and all `GetFileStateSignal()` callers. Decide between (α) and (β).
2. **Phase B — Split + base-class fire wiring.** Apply the chosen design to `emFileModel<T>`. Wire fires at all state-transition sites with D-007 ectx-threading.
3. **Phase C — Derived-class delegation.** Fix `emRecFileModel::GetFileStateSignal` to delegate to base class.
4. **Phase D — Consumer audit + UPSTREAM-GAP cleanup.** Drop UPSTREAM-GAP markers; audit other `emFileModel`-derived models for the same delegation; wire any previously-stubbed reactions that now carry real signal.
5. **Phase E — Reconciliation.** Run full nextest; verify `cargo xtask annotations` clean; close FU-005.

## Acceptance

- Two semantically distinct signals at the base class (or one renamed signal + documented absence of the other if Phase A decides ChangeSignal is truly Rust-only-redundant).
- All ~10 Rust state-transition sites fire the file-state signal; observable via D-007 ectx-threaded synchronous fire.
- `emRecFileModel::GetFileStateSignal` delegates to the base-class signal; null return removed.
- 3 UPSTREAM-GAP markers in emStocks removed.
- Consumer audit complete; any previously-stubbed reactions wired or deferred with explicit reason.
- `cargo-nextest ntr` green; `cargo clippy -D warnings` green; `cargo xtask annotations` clean.

## Out of scope

- Performance / memory layout changes to emFileModel beyond what the signal split requires.
- New file-state types or transitions beyond what C++ defines.
- emRecFileModel ChangeSignal lazy-allocation pattern (already in place via `GetChangeSignal(&self, ectx)` per B-002) — that's a separate signal, not affected by this fix.

## Notes

- Cross-references: this bucket retires the `UPSTREAM-GAP` comments cited in B-001-followup Phase E (commit `39a6fb97`) and removes the workaround signaled by FU-001's original "GetFileStateSignal lift" item.
- Brainstorm should pay attention to **whether `change_signal` is ever fired** in current source. If a fire site exists somewhere I didn't find, design (α) is forced; if not, (β) (rename) is cleaner.
- D-007 ectx-threading the 10 state-transition sites may require updating callers of `Load`/`Save`/`complete_load` etc. to pass ctx — non-trivial signature change. Brainstorm should surface the call-site count.

## References

- C++ source: `~/Projects/eaglemode-0.96.4/src/emCore/emFileModel.cpp` — 15 `Signal(FileStateSignal)` fire sites at lines 42, 55, 65, 76, 123, 143, 154, 188, 259, 267, 292, 514, 524, 532, 539.
- C++ header: `~/Projects/eaglemode-0.96.4/include/emCore/emFileModel.h:75,295,313` (FileStateSignal field + accessor).
- C++ derived class: `~/Projects/eaglemode-0.96.4/include/emCore/emRecFileModel.h:50,91,101` (ChangeSignal — separate signal).
- Rust source: `crates/emcore/src/emFileModel.rs:117,167` (single conflated signal); `crates/emcore/src/emRecFileModel.rs:366` (null delegate workaround).
- Downstream UPSTREAM-GAP markers: `crates/emstocks/src/emStocksFileModel.rs:149`, `crates/emstocks/src/emStocksPricesFetcher.rs:78,425`.
- FU-001 brainstorm origin: `docs/superpowers/specs/2026-05-02-FU-001-emstocks-reaction-bodies-design.md` (Summary section + Out-of-scope §"GetFileStateSignal conflation fix").
