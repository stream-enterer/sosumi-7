---
id: 0004
type: decide
timestamp: 2026-04-26T12:00:00Z
hypothesis_ids: []
supersedes: null
artifacts: [docs/debug/investigations/F010-investigation/forbidden-fix-shapes.md]
---

# Phase 1 lock

Pre-registration table is locked. 26 entries in
`docs/debug/investigations/F010-investigation/hypotheses/`:

- 18 hypotheses: H1-H11, P1-P5, P7, P8 (P6 was extracted to methodology constraint M1 per synthesis-v2.md)
- 8 blind spots: B1-B8

Cross-falsification audit (entry 0002) passed. Cluster ordering (entry 0003) fixed.

Forbidden-fix-shapes handoff document committed.

Phase 2 may begin. Per spec Section 4, harness construction is per-cluster, ordered cheapest-first. Pre-registration entries may be revised in phase 2 only via new `revise` entries with `supersedes:` references; existing entries are immutable.
