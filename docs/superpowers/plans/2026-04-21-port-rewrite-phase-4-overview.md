# Phase 4 Family — Overview & Execution Order

**Purpose.** The Phase 4 family ports the entire emRec / emCoreConfig stack from C++. It was originally three plans (4a, 4b, 4c, 4d). Pre-execution audits during 4b execution split the work into six phases. This file is the canonical execution order for any agent resuming work in the Phase 4 series.

**Last revised:** 2026-04-21.

## Execution chain

| # | Phase | Plan file | Status | Ships |
|---|---|---|---|---|
| 1 | **4a** | `2026-04-19-port-rewrite-phase-4a-emrec-trait-primitives.md` | ✅ COMPLETE — merged + tagged `port-rewrite-phase-4a-complete` | `emRec<T>` + `emRecNode` traits; `emBoolRec`, `emIntRec`, `emDoubleRec`, `emEnumRec`, `emStringRec`; `emRecParser` (split out of legacy `emRec.rs`) |
| 2 | **4b** | `2026-04-19-port-rewrite-phase-4b-emrec-compound.md` | 🟡 READY FOR CLOSEOUT — branch `port-rewrite/phase-4b` | `emFlagsRec` only (commits `280a23b3` + `7223846c`). Listener-tree work was carved out into Phase 4c after the ADR collapsed it to a small per-primitive retrofit |
| 3 | **4b.1** | `2026-04-21-port-rewrite-phase-4b-1-color-alignment-rec.md` | ⏳ PENDING (after 4b) | New `emAlignment` Rust type; `emAlignmentRec` + `emColorRec` ports; migrates 3 production consumers (`emVirtualCosmos`, `emBookmarks`, `emFileManTheme`); deletes legacy parser-era counterparts from `emRecRecTypes.rs` |
| 4 | **4c** | `2026-04-21-port-rewrite-phase-4c-emrec-compound-types.md` | ⏳ PENDING (after 4b; may run in parallel with 4b.1 or before/after) | **Listener tree retrofit** (per ADR `2026-04-21-phase-4b-listener-tree-adr.md`): `aggregate_signals: Vec<SignalId>` field on every primitive; `register_aggregate` method. Plus `emRecListener` and the structural compounds: `emStructRec`, `emUnionRec`, `emArrayRec`, `emTArrayRec<T>` |
| 5 | **4d** | `2026-04-19-port-rewrite-phase-4d-emrec-persistence.md` | ⏳ PENDING (after 4c) | `emRecReader`, `emRecWriter`, `emRecFileReader`, `emRecFileWriter`, `emRecMemReader`, `emRecMemWriter`. `TryRead`/`TryWrite` on every concrete type from 4a/4b/4b.1/4c |
| 6 | **4e** | `2026-04-19-port-rewrite-phase-4e-emcoreconfig-migration.md` | ⏳ PENDING (after 4d). Closes JSON entries **E026** + **E027** | `emCoreConfig` rewritten as an `emStructRec` with typed fields; `emCoreConfigPanel` migrated off `Rc<RefCell<emConfigModel<T>>>`; deletes `VISIT_SPEED_MAX` etc. |

## Dependency rationale

```
4a (primitives) → 4b (emFlagsRec) → 4b.1 (Color/Alignment) ──┐
                                                              ├→ 4c (listener tree + compounds) → 4d (persistence) → 4e (emCoreConfig)
                                                              ┘
```

- **4b before 4b.1**: 4b.1's new `emAlignmentRec`/`emColorRec` follow the Phase 4a/4b primitive pattern — straightforward by-value primitives. They do NOT need the listener-tree retrofit at construction time; they receive it as part of Phase 4c's I4c-1 alongside the other primitives.
- **4b and 4b.1 before 4c**: Phase 4c's listener-tree retrofit (I4c-1) modifies every primitive that exists. If 4b.1 runs after 4c, 4b.1 must add the `aggregate_signals` field to its new types itself; if 4b.1 runs before 4c, 4c picks them up automatically. Either order works; prefer 4b.1 first to avoid the retrofit-during-port mode-switch.
- **4c before 4d**: persistence (`TryRead`/`TryWrite`) needs every concrete type to exist first, including the structural compounds.
- **4d before 4e**: emCoreConfig migration uses the persistence stack.

The listener-tree representation is settled at ADR `docs/superpowers/notes/2026-04-21-phase-4b-listener-tree-adr.md`. Future phases consume it; they do not redesign it.

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
