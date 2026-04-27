---
id: 0007
type: observe
timestamp: 2026-04-26T18:00:23Z
hypothesis_ids: [H1]
supersedes: null
artifacts: [docs/debug/investigations/F010-investigation/artifacts/H1-ops.json]
---

# H1 falsification experiment result

Test: `cargo nextest run -p emcore --test f010_h1_clear_recording`

Outcome: PASS

Evidence (from artifacts/H1-ops.json):
- len_before: 0
- len_after: 0
- ops_added_by_clear: 0

Interpretation: H1 hypothesis upheld. emPainter::Clear records zero ops when
the painter target is a recording DrawList — the require_direct() dispatch
returns None for DrawList targets and Clear early-returns silently. The
recording-mode dispatch hole is real.

Per methodology, this experiment does NOT prove H1 is the F010 root cause; it
confirms that H1's predicted mechanism (Clear silently dropped in recording
mode) holds. The cluster's `same-observable-with-H1` resolution depends on
cross-falsification with P8 (entry 0008).
