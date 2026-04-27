# Signal-Drift Remediation — Work Order

**Generated:** 2026-04-27 from Phase 4 of the bookkeeping strategy.
**Total buckets:** 19
**Layers:** 1 (no cross-bucket prereqs — all 11 prereq edges in `inventory-enriched.json` are intra-B-009 consumer→accessor)

Buckets are ordered by topological layer over the prereq DAG (lower layer = no unmet prereqs). With a single layer, ordering reduces to mechanical-heavy first, then balanced, then judgement-heavy — mechanical work validates the underlying patterns cheaply before committing to judgement-laden buckets.

## Order

| # | Bucket | Layer | Mechanical-vs-judgement | Rows | Status | Design doc |
|---|---|---|---|---|---|---|
| 1 | B-005-typed-subscribe-emfileman | 0 | mechanical-heavy | 21 | designed | [d95d55a7](../../../../superpowers/specs/2026-04-27-B-005-typed-subscribe-emfileman-design.md) |
| 2 | B-006-typed-subscribe-mainctrl | 0 | mechanical-heavy | 3 | designed | [a13880c7](../../../../superpowers/specs/2026-04-27-B-006-typed-subscribe-mainctrl-design.md) |
| 3 | B-007-typed-subscribe-emcore | 0 | mechanical-heavy | 3 | pending | — |
| 4 | B-008-typed-subscribe-misc | 0 | mechanical-heavy | 3 | pending | — |
| 5 | B-015-polling-emcore-plus | 0 | mechanical-heavy | 10 | pending | — |
| 6 | B-019-stale-annotations | 0 | mechanical-heavy | 9 | pending | — |
| 7 | B-001-no-wire-emstocks | 0 | balanced | 71 | pending | — |
| 8 | B-002-no-wire-emfileman | 0 | balanced | 4 | pending | — |
| 9 | B-003-no-wire-autoplay | 0 | balanced | 3 | pending | — |
| 10 | B-004-no-wire-misc | 0 | balanced | 4 | pending | — |
| 11 | B-016-polling-no-acc-emfileman | 0 | balanced | 3 | pending | — |
| 12 | B-017-polling-no-acc-emstocks | 0 | balanced | 3 | pending | — |
| 13 | B-009-typemismatch-emfileman | 0 | judgement-heavy | 14 | pending | — |
| 14 | B-010-rc-shim-emcore | 0 | judgement-heavy | 15 | pending | — |
| 15 | B-011-rc-shim-autoplay | 0 | judgement-heavy | 7 | pending | — |
| 16 | B-012-rc-shim-mainctrl | 0 | judgement-heavy | 7 | pending | — |
| 17 | B-013-dialog-cells-emstocks | 0 | judgement-heavy | 4 | pending | — |
| 18 | B-014-rc-shim-no-acc-misc | 0 | judgement-heavy | 2 | pending | — |
| 19 | B-018-fileDialog-singleton | 0 | judgement-heavy | 1 | pending | — |

Total rows: 187 (178 actionable + 9 cleanup).

## Status legend

- `pending` — not yet picked up.
- `in-design` — a fan-out brainstorm session is currently working on this bucket.
- `designed` — design doc returned and reconciled into the spine.
- `merged` — implementation merged to `main`.

## Reconciliation log

### 2026-04-27 — B-005 design returned (d95d55a7)

- **D-006-subscribe-shape** added to `decisions.md` (resolved per fan-out's recommendation: first-Cycle init + IsSignaled top-of-Cycle, with deferred-queue B as documented fallback).
- **D-005** amended with a "see also D-006" pointer (D-005 picks reaction model, D-006 picks wiring shape; complementary).
- **Cross-bucket prereq surfaced:** B-005 → B-009. Two `emFileManControlPanel` rows in B-005 require B-009's accessor flip (D-001) before their Cycle-init connects can land. Remaining 19 B-005 rows are independent. Documented in B-005's `Prereq buckets:` line; not yet propagated to `inventory-enriched.json` row-level prereq_ids (the dependency is design-level, not row-level — a `B-005 cannot fully merge until B-009 merges` constraint, not a row-pair edge). Topo order in this table is unchanged because B-005 is already designed; merge order will sequence B-009 ahead of B-005's blocked rows when the time comes.
- **Stale prose** in B-005's bucket sketch was already cleaned up in 83eb06d3 before the design returned.
- **B-005 status:** in-design → designed.

### 2026-04-27 — B-006 design returned (a13880c7)

- **No new D-### entries** — D-006 covered the wiring shape verbatim.
- **Audit-data correction:** `emMainControlPanel-218` reclassified `gap-blocked → drifted`; `D-003` citation dropped. `GetWindowFlagsSignal` exists at `crates/emcore/src/emWindow.rs:1279`; audit-time tag was stale. D-003's "Affects" count amended from 16 → 15.
- **New row state — `resolved_by`:** `emMainControlPanel-217` annotated with `resolved_by` pointing to `crates/emmain/src/emMainWindow.rs:825` (`ControlPanelBridge`); the drift at the row's site is observably absorbed by an existing dependency-forced divergence at a different site. Row stays in bucket; design treats as no-action. Future buckets that surface similar drift-here-resolved-there cases use the same `resolved_by` field on the row.
- **Soft cross-bucket edge:** B-006 → B-012-rc-shim-mainctrl. Non-blocking. The 7 `BtNewWindow..BtQuit` click-flag polls in `emMainControlPanel.Cycle` will become D-006-shaped `IsSignaled` branches when B-012 lands. B-006 is observable-correct without it.
- **Implementation note from designer:** three widget handles (`bt_fullscreen`, `bt_auto_hide_control_view`, `bt_auto_hide_slider`) need to be hoisted from `LMainPanel`-local to `emMainControlPanel` fields as Step 1 of the implementation. Mechanical refactor; in-scope per the design doc.
- **B-006 status:** pending → designed.
