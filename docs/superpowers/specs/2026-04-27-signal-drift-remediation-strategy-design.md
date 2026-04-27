# Signal-Drift Remediation Bookkeeping Strategy

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Source audit:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/`
**Scope:** Convert 178 actionable rows (162 drifted + 16 gap-blocked) plus 9 cleanup items from `preexisting-diverged.csv` into a tiered, ordered work order suitable for fan-out brainstorming across separate sessions.

## Goal

Produce a control surface — not a static work order — that lets a long-running "working memory" session dispatch per-bucket design brainstorms to other sessions and reconcile their outputs against a shared spine.

## Non-goals

- Designing the fixes themselves. Per-bucket designs happen in fan-out sessions.
- Resolving the audit's headline finding ("0 forced") at the convention level. That is a global decision (D-###) extracted in Phase 2, not a strategy concern.
- Creating new audit data. The strategy consumes existing audit artifacts unchanged.

## Constraint that shapes the strategy

The dominant lossiness mode to prevent is **scope-creep across buckets** (rows misallocated; later buckets discover work that belongs in already-frozen earlier buckets). This rules out "design bucket 1 fully, then bucket 2 fully" sequencing. Bucket assignments must stay mutable until *every* bucket exists at sketch resolution.

A secondary lossiness mode arises from fan-out itself: a per-bucket design produced in another session can introduce cross-bucket impact (touch rows in another bucket; surface a new global decision; invalidate a prereq edge). The strategy includes a standing reconciliation responsibility to absorb these.

## Phases

### Phase 1 — Enrich (mechanical)

Walk every row in `inventory.json` (filter to verdicts `drifted` and `gap-blocked`) plus every entry in `preexisting-diverged.csv`. For each row attach orthogonal tags:

- `pattern-id` — drift shape (e.g. `u64-where-SignalId-expected`, `missing-subscribe-call`, `wrong-emit-site`). Derived from `evidence_kind` + `accessor_status` + targeted manual look.
- `prereq-ids` — other rows or upstream-gap fills that must land first.
- `decision-id`(s) — global design decisions this row's fix depends on (`D-###`).
- `scope-key` — file / panel / subsystem, for grouping affinity.

**Output:** `inventory-enriched.json`.
**Gate:** every actionable row has all four tags; row count matches source.

### Phase 2 — Extract global decisions

Enumerate every distinct `decision-id` referenced in Phase 1. For each, write an ADR-style entry with a stable `D-###` ID at sketch resolution (one paragraph: question, options considered, chosen direction, why). Full design fidelity is not required at this phase — only enough that Phase 3 sketches can cite a stable answer.

**Output:** `decisions.md`.
**Gate:** every `decision-id` referenced in `inventory-enriched.json` has a corresponding `D-###` entry.

### Phase 3 — Cluster, sketch all, then freeze

Cluster enriched rows into candidate buckets. Primary axis: `pattern-id`. Tiebreaker: `scope-key`. Each candidate bucket gets a one-page brainstorm-launcher file containing:

- Row list with refs into `inventory-enriched.json`.
- Pattern description.
- C++ sites touched (paths + line numbers from `cpp-sites.csv`).
- Prereq buckets.
- Cited `D-###` entries from `decisions.md`.
- Mechanical-vs-judgement ratio estimate.

**Critical gate:** every bucket file is sketched *before* any bucket is frozen. This is what makes scope-creep impossible — a row can't be misallocated to bucket-X while bucket-Y is unsketched, because the freeze step sees both at once.

**Output:** `buckets/B-###-<slug>.md` (one file per bucket).
**Gate:** every actionable row appears in exactly one bucket; row coverage = 100%.

### Phase 4 — Tier and order

Tier the frozen buckets by:
1. Topological layer over the prereq DAG (lower layer = no unmet prereqs).
2. Within a layer, mechanical-heavy buckets first (they validate the pattern cheaply before judgement-heavy work begins).

**Output:** `work-order.md` — ordered list of `B-###` entries with status column (`pending`, `in-design`, `designed`, `merged`).
**Gate:** topological order is acyclic; every bucket appears exactly once.

### Phase 5 — Reconciliation (standing, not one-shot)

Owned by this session for the lifetime of the remediation effort. Triggered when a per-bucket design returns from a fan-out session. Checks:

- Did the design touch rows assigned to another bucket? → reassign and update affected bucket files.
- Did it surface a new global decision? → add a `D-###` entry; check whether existing buckets need re-citation.
- Did it invalidate a prereq edge? → update DAG; re-tier `work-order.md`.
- Update `work-order.md` status column.

Phase 5 has no terminal gate; it ends when `work-order.md` shows every bucket `merged`.

## Artifact layout

```
docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/
├── inventory-enriched.json     # Phase 1 output; data layer
├── decisions.md                # Phase 2 output; spine, mutable
├── buckets/
│   ├── B-001-<slug>.md         # Phase 3 output; brainstorm-launcher per bucket
│   ├── B-002-<slug>.md
│   └── ...
└── work-order.md               # Phase 4 output; dispatcher + status
```

Co-located with the source audit because these are downstream audit artifacts. Per-bucket *designs* (produced by fan-out sessions) land in the standard `docs/superpowers/specs/` flow and are referenced from the bucket file's status entry.

## Roles and invariants

**This session (working memory):** owns `decisions.md`, `work-order.md`, the prereq DAG, and cross-bucket invariants. Stays open across the remediation effort.

**Per-bucket fan-out sessions:** own their design doc; read `decisions.md` + their bucket file as input; propose amendments to the spine back to this session.

**Invariants:**
- Each row belongs to exactly one bucket (single-assignment, late-frozen).
- All buckets exist at sketch resolution before any bucket is frozen.
- Global decisions are resolved (at sketch resolution) before any bucket is sketched.
- Per-bucket designs are reconciled here before they're treated as final.

## Success criteria

- All 178 actionable rows + 9 cleanup items appear in exactly one bucket.
- Every `decision-id` referenced by any row has a `D-###` entry.
- `work-order.md` is acyclic and totally ordered.
- A fresh Claude session, given only `decisions.md` + a single `B-###-<slug>.md`, can run a useful brainstorm without re-reading the audit.
