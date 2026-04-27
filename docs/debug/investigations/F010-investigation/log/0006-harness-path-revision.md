---
id: 0006
type: revise
timestamp: 2026-04-26T17:57:13Z
hypothesis_ids: [H1, P8]
supersedes: 0005
artifacts: [crates/emcore/tests/f010_h1_clear_recording.rs, crates/emcore/tests/f010_p8_zero_area.rs]
---

# Harness path bug — artifact path resolves against repo root

Both harness tests (f010_h1_clear_recording.rs and f010_p8_zero_area.rs) wrote
their JSON artifacts using a bare relative path
(`"docs/debug/investigations/F010-investigation/artifacts/..."`) which resolved
against the test's cwd — `crates/emcore/` when run via `cargo nextest run -p
emcore` — placing artifacts at `crates/emcore/docs/...` instead of the
repo-root `docs/...`.

## Fix

Replaced the bare string path with:

```rust
let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../docs/debug/investigations/F010-investigation/artifacts/<file>.json");
```

`env!("CARGO_MANIFEST_DIR")` expands to the absolute path of `crates/emcore/`
at compile time, making the resulting path repo-root anchored regardless of the
process cwd at test execution time.

Stale artifacts at the wrong location (`crates/emcore/docs/`) have been
deleted.

## Re-run results

Both tests pass and artifacts appear at the correct location:

- `docs/debug/investigations/F010-investigation/artifacts/H1-ops.json`
  `{"len_before":0,"len_after":0,"ops_added_by_clear":0}`
  → H1 consistent (Clear records no ops).

- `docs/debug/investigations/F010-investigation/artifacts/P8-rect.json`
  `{"clip":{"x1":50.0,"y1":40.0,"x2":750.0,"y2":540.0},"rect":{"x":50,"y":40,"w":700,"h":500}}`
  → P8 falsified (rect is non-degenerate: w=700, h=500 > 0).

Cluster 1 execution may now proceed under Task 3.1 with the corrected harness.
