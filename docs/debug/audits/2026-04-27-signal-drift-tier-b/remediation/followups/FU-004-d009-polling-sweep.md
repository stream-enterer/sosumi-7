# FU-004 — D-009 polling-intermediary sweep

**Pattern:** D-009 (CLAUDE.md §"Polling intermediaries") — `Cell` / `RefCell` field set at one site and drained by another's `Cycle`, producing one-tick timing drift from C++.
**Scope:** tree-wide (discovery pass).
**Row count:** unknown — bucket starts with a fresh enumeration phase.
**Prereq buckets:** none.

## Pattern description

D-009 was promoted to a CLAUDE.md rule during Tier-B. B-015/B-016/B-017 each removed concrete D-009 instances by switching to D-007 ectx-threading (synchronous fire). One additional sighting was noted in passing during B-010 design review:

- `emCoreConfigPanel` reset closure: a counter is incremented at one site and polled by a closure on a later Cycle. Pre-existing drift, not in B-010 scope, not bucketed.

This is the only known sighting at Tier-B close, but no tree-wide scan was performed. The pattern is structurally easy to grep (`Cell::set` + remote `Cell::take`/`Cell::get` in a `Cycle`/`cycle` body), so a sweep is cheap.

## Phases

1. **Enumerate.** Grep the tree for the D-009 shape:
   - `Cell<bool>` / `Cell<Option<...>>` fields with a remote `set` site and a `Cycle`-side `take`/`get` drain.
   - `RefCell` fields written under one event handler and read by `Cycle` to derive a side effect.
   - Closure registries (`Rc<RefCell<Vec<Box<dyn Fn>>>>`) populated outside Cycle and invoked in Cycle.

   Produce a row list with C++ counterpart (C++ either does not have the intermediary, fires synchronously, or — if C++ also defers — confirm and skip).

2. **Classify.** For each sighting:
   - **D-007 candidate:** rewrite to ectx-threaded synchronous fire at the mutation site.
   - **Forced retention:** intermediary is required by ownership constraints (rare; needs `DIVERGED:` annotation with category cite).
   - **C++-mirrored deferral:** C++ also defers; not a D-009 violation, drop from bucket.

3. **Remediate.** Apply D-007 to candidates; annotate the rest. One commit per file/site.

## Known starting point

- `crates/emcore/src/emCoreConfigPanel.rs` — reset-closure counter (B-010 design noted this; verify with current source).

## Acceptance

- Discovery phase produces an exhaustive list (not "everything found in a quick grep" — proper sweep).
- Every sighting is either remediated to D-007 or carries an annotated forced-retention justification.
- No new D-009 violations land during the sweep (`grep` clean for the shape after merge).

## Notes

- Discovery-led: do not pre-write a row table. The first phase produces one.
- D-009 is a CLAUDE.md-level rule now; new code is expected to comply. This bucket clears the legacy debt, then ongoing compliance is a code-review concern, not an audit concern.

## Inventory (2026-05-02)

The verified row table lives in [FU-004-inventory.md](FU-004-inventory.md) — moved to a sibling file because the table plus per-row evidence prose exceeded the bucket-file size threshold (~80 lines). See that file for the 17-row inventory, the D-007 candidates section, the needs-deeper-audit section, and the closure note.

## Closure (2026-05-02)

Inventory verified per `docs/superpowers/specs/2026-05-02-FU-004-d009-inventory-design.md`.

- Candidates enumerated: 17.
- `C++-mirrored`: 9.
- `forced retention`: 1.
- `vestigial`: 1.
- `in-progress-migration`: 3.
- `D-007 candidate`: 1.
- `needs deeper audit`: 2.

Status: **open** — 1 D-007 candidate (`emVirtualCosmosPanel::needs_update`) pending its own follow-up spec; 2 needs-deeper-audit candidates (`emVirtualCosmosItemPanel::update_needed` port-completion fidelity-bug; `emView::needs_animator_abort` VIEW-003 dispatch-chain audit) pending their own follow-up buckets.
