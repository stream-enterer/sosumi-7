# Phase 4 Family — Overview & Execution Order

**Purpose.** The Phase 4 family ports the entire emRec / emCoreConfig stack from C++. It was originally three plans (4a, 4b, 4c, 4d). Pre-execution audits during 4b execution split the work into six phases. This file is the canonical execution order for any agent resuming work in the Phase 4 series.

**Last revised:** 2026-04-21.

## Execution chain

| # | Phase | Plan file | Status | Ships |
|---|---|---|---|---|
| 1 | **4a** | `2026-04-19-port-rewrite-phase-4a-emrec-trait-primitives.md` | ✅ COMPLETE — merged + tagged `port-rewrite-phase-4a-complete` | `emRec<T>` + `emRecNode` traits; `emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`; `emRecParser` (split out of legacy `emRec.rs`) |
| 2 | **4b** | `2026-04-19-port-rewrite-phase-4b-emrec-compound.md` | 🟡 IN PROGRESS — branch `port-rewrite/phase-4b` | Listener-tree machinery (`UpperNode`, `IsListener`, `ChildChanged`, `Changed`, `BeTheParentOf`); `emRecListener`; parent-aware ctors retrofitted onto all six primitives; `emFlagsRec` (already shipped at commits `280a23b3` + `7223846c`) |
| 3 | **4b.1** | `2026-04-21-port-rewrite-phase-4b-1-color-alignment-rec.md` | ⏳ PENDING (after 4b) | New `emAlignment` Rust type; `emAlignmentRec` + `emColorRec` ports; migrates 3 production consumers (`emVirtualCosmos`, `emBookmarks`, `emFileManTheme`); deletes legacy parser-era counterparts from `emRecRecTypes.rs` |
| 4 | **4c** | `2026-04-21-port-rewrite-phase-4c-emrec-compound-types.md` | ⏳ PENDING (after 4b.1) | Structural compounds: `emStructRec`, `emUnionRec`, `emArrayRec`, `emTArrayRec<T>`. All built on the listener tree from 4b — no per-compound `aggregate_signal` |
| 5 | **4d** | `2026-04-19-port-rewrite-phase-4d-emrec-persistence.md` | ⏳ PENDING (after 4c) | `emRecReader`, `emRecWriter`, `emRecFileReader`, `emRecFileWriter`, `emRecMemReader`, `emRecMemWriter`. `TryRead`/`TryWrite` on every concrete type from 4a/4b/4b.1/4c |
| 6 | **4e** | `2026-04-19-port-rewrite-phase-4e-emcoreconfig-migration.md` | ⏳ PENDING (after 4d). Closes JSON entries **E026** + **E027** | `emCoreConfig` rewritten as an `emStructRec` with typed fields; `emCoreConfigPanel` migrated off `Rc<RefCell<emConfigModel<T>>>`; deletes `VISIT_SPEED_MAX` etc. |

## Dependency rationale

```
4a (primitives) → 4b (listener tree) → 4b.1 (Color/Alignment) → 4c (structural compounds) → 4d (persistence) → 4e (emCoreConfig)
                       ↘ retrofits 4a primitives with parent-aware ctors
```

- **4b before 4b.1**: 4b.1's new `emAlignmentRec`/`emColorRec` need the parent-aware ctor pattern from 4b.
- **4b before 4c**: 4c's structural compounds (emStructRec etc.) propagate aggregate change *via* the 4b listener tree. The original 4b plan had this backwards (struct-owned aggregate signal); audit corrected it.
- **4b.1 before 4c**: not strictly required, but 4b.1 is a small focused migration; landing it before the larger 4c keeps the legacy `emRecRecTypes.rs` types from accumulating more dependents.
- **4c before 4d**: persistence (`TryRead`/`TryWrite`) needs every concrete type to exist first.
- **4d before 4e**: emCoreConfig migration uses the persistence stack.

## How this overview gets updated

Each phase's Closeout (C7) writes a closeout note. After the Closeout commits, also append a row update here:

- Status column: `✅ COMPLETE — merged + tagged port-rewrite-phase-<N>-complete (sha)`.
- If the phase delegated work to a new sub-phase (as 4b did to 4b.1 and 4c, and as old 4c/4d shifted to 4d/4e), add the new row in execution order and re-renumber all downstream rows in the same commit. Note the JSON-entry closures column as well.

## Renaming history

The 4c/4d slots used to belong to persistence and emCoreConfig respectively. They were renamed in commit `<TBD-this-commit>` to make room for the carved-out compound-types phase (now 4c) that the rewritten 4b plan depends on. Old filenames:
- `2026-04-19-port-rewrite-phase-4c-emrec-persistence.md` → renamed to `…phase-4d-emrec-persistence.md`.
- `2026-04-19-port-rewrite-phase-4d-emcoreconfig-migration.md` → renamed to `…phase-4e-emcoreconfig-migration.md`.
- `2026-04-21-port-rewrite-phase-4b-prime-color-alignment-rec.md` → renamed to `…phase-4b-1-color-alignment-rec.md` (decimal-subphase convention to match `phase-1-5`, `phase-1-75`, `phase-1-76`).

The original Phase 4b plan's title was "emRec Compound Types"; its scope was reduced to listener tree + emFlagsRec only, and the bulk of the original tasks moved into 4c (structural compounds) and 4b.1 (Color/AlignmentRec). The plan file kept its original filename (`…phase-4b-emrec-compound.md`) for git-history continuity; only its content was rewritten.
