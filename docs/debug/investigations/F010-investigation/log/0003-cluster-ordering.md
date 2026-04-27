---
id: 0003
type: decide
timestamp: 2026-04-26T11:30:00Z
hypothesis_ids: []
supersedes: null
artifacts: []
---

# Cluster ordering for phase 3 execution

Cheapest expected experiment first per spec Section 5:

1. `same-observable-with-H1` (H1, P8) — pure unit tests, no GUI rebuild required.
2. H2 (singleton, tile pre-fill) — small instrumentation + unit test.
3. `dispatch-cluster` (P3, B2, B3) — single instrumentation pass + GUI rebuild + reproduce.
4. `invalidation-cluster` (P2, B1, B8) — multiple instrumentation sites + GUI rebuild.
5. Tier-2 standalones (H3, H4, H5, H6, P1) — varied costs but unit-testable.
6. Tier-3 standalones (H7, H8, H9, H10, P7) — mostly static analysis or single tests.
7. Remaining blind spots (B4, B6, B7) — multi-machine for B6, otherwise modest.
8. `order-config-cluster` (P4, B5, H11, P5) — multi-build-config; heaviest. Last.

Order is fixed at end of phase 1 (this entry locks it). Deviations during phase 3 require new `decide` entries.
