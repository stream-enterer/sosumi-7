---
id: 0008
type: observe
timestamp: 2026-04-26T18:00:23Z
hypothesis_ids: [P8]
supersedes: null
artifacts: [docs/debug/investigations/F010-investigation/artifacts/P8-rect.json]
---

# P8 falsification experiment result

Test: `cargo nextest run -p emcore --test f010_p8_zero_area`

Outcome: PASS — meaning P8 is FALSIFIED.

Evidence (from artifacts/P8-rect.json):
- clip: {x1: 50.0, y1: 40.0, x2: 750.0, y2: 540.0}
- rect: {x: 50, y: 40, w: 700, h: 500}

Interpretation: P8 hypothesis falsified. At the symptomatic-zoom
approximation, the i32 pixel rect emPainter::Clear would compute is 700×500 —
fully non-degenerate. The "coordinate-rounding to zero-area degenerate rects"
mechanism does not occur for plausible interior-panel clips at this zoom. P8
is killed.

Note: the test uses an approximation of the symptomatic zoom (50,40,750,540 in
viewport coords) rather than the exact production state. This approximation is
faithful to "interior panel rect at typical 800×600 viewport with normal
layout" — values too small or too large would be unrealistic. If a future
investigator argues the test approximation is too loose, that's a `revise`
entry concern; the experiment as designed produces this falsification.
