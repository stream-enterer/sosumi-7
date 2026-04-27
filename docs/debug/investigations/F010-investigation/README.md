# F010 X+Z investigation

Per spec `docs/superpowers/specs/2026-04-26-F010-investigation-methodology-design.md` and plan `docs/superpowers/plans/2026-04-26-F010-investigation-methodology.md`.

## Layout

- `hypotheses/` — pre-registration entries, one YAML per hypothesis. ID-named (H1.yaml, P1.yaml, B1.yaml, etc.). Locked at end of phase 1.
- `log/` — append-only investigation log, one markdown per entry, named `NNNN-<short>.md` with monotonic 4-digit counter. Never edit prior entries; corrections are new entries with `supersedes:`.
- `harness/` — Rust test fixtures specific to F010 falsification experiments. Built per-cluster in phase 2. Each cluster's components are locked at start of that cluster's phase-3 execution.
- `artifacts/` — experiment outputs (op-stream JSON, image diffs, instrumentation traces). Cited from `observe` log entries via the `artifacts:` frontmatter field.

## Phase status

Update this section as phases complete.

- [ ] Phase 0 — setup
- [ ] Phase 1 — pre-registration drafting and lock
- [ ] Phase 2 — harness construction (per-cluster, ordered cheapest-first)
- [ ] Phase 3 — cluster-first execution
- [ ] Phase 4 — termination gate and handoff
