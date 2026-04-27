# Signal-Drift Remediation — Work Order

**Generated:** 2026-04-27 from Phase 4 of the bookkeeping strategy.
**Total buckets:** 19
**Layers:** 1 (no cross-bucket prereqs — all 11 prereq edges in `inventory-enriched.json` are intra-B-009 consumer→accessor)

Buckets are ordered by topological layer over the prereq DAG (lower layer = no unmet prereqs). With a single layer, ordering reduces to mechanical-heavy first, then balanced, then judgement-heavy — mechanical work validates the underlying patterns cheaply before committing to judgement-laden buckets.

## Order

| # | Bucket | Layer | Mechanical-vs-judgement | Rows | Status | Design doc |
|---|---|---|---|---|---|---|
| 1 | B-005-typed-subscribe-emfileman | 0 | mechanical-heavy | 21 | in-design | — |
| 2 | B-006-typed-subscribe-mainctrl | 0 | mechanical-heavy | 3 | pending | — |
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

(Phase 5 entries appended here as fan-out designs return and are reconciled. Each entry: date, bucket, what was reconciled, spine amendments.)
