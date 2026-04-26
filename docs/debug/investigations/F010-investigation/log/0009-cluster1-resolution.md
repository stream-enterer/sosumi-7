---
id: 0009
type: confirm
timestamp: 2026-04-26T18:00:23Z
hypothesis_ids: [H1, P8]
supersedes: null
artifacts:
  - docs/debug/investigations/F010-investigation/log/0007-h1-experiment.md
  - docs/debug/investigations/F010-investigation/log/0008-p8-experiment.md
---

# Cluster `same-observable-with-H1` resolution

Outcome: confirm — H1 confirmed, P8 falsified.

H1: confirmed per entry 0007 (Clear records zero ops in recording mode).
P8: falsified per entry 0008 (rect 700×500 is non-degenerate at symptomatic
zoom).

Per plan Task 3.1 step 6 four-row outcome table, this is row 1:
- H1 confirmed (test PASS, ops_added_by_clear == 0)
- P8 falsified (test PASS, rect non-degenerate)
- → **H1 confirmed; cluster resolved.** Advance to next cluster.

Per plan Task 3.1 step 8 (defense-in-depth), the methodology continues to all
remaining clusters regardless of cluster 1's outcome. A confirmed hypothesis
from cluster 1 is one piece of evidence, not the full picture; subsequent
cluster falsifications strengthen H1 by elimination, while any cluster that
ALSO confirms triggers an `escalate` entry per multi-cause situation handling.
Phase 3 / Task 3.2 (H2 singleton) is next.
