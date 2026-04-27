---
id: 0005
type: decide
timestamp: 2026-04-26T17:43:52Z
hypothesis_ids: [H1, P8]
supersedes: null
artifacts: [crates/emcore/tests/f010_h1_clear_recording.rs, crates/emcore/tests/f010_p8_zero_area.rs]
---

# Cluster `same-observable-with-H1` harness lock

Two tests created. Both compile and run. H1's test inspects the recording
painter's ops vec after Clear; P8's test inspects the i32 rect computed
from the painter's current clip at the symptomatic zoom.

Phase 3 cluster execution may begin.
