# Tier-B Follow-up Buckets

Post-remediation work surfaced during Tier-B execution (2026-04-27 → 2026-05-01). Tier-B itself is **closed** (19/19 buckets, 212/212 rows, 3003 nextest tests passing). The buckets below cover residual stubs and architectural gaps that were intentionally scoped out, all currently tracked as `TODO(B-0XX-followup)` markers in source or as `UPSTREAM-GAP` annotations.

## Buckets

| ID | Title | Scope | Axis | Prereqs |
|---|---|---|---|---|
| [FU-001](FU-001-emstocks-reaction-bodies.md) | emstocks reaction-body completion + emFileModel state-signal lift | emstocks + emcore | reaction-body wiring, accessor lift | none |
| [FU-002](FU-002-app-bound-reactions.md) | App-bound reaction wiring (mainctrl) | emmain | Cycle-side `&mut App` access | architectural decision on App threading |
| [FU-003](FU-003-emview-multiview-port.md) | emView multi-view content/control split | emcore (emView) | upstream port | none (large standalone) |
| [FU-004](FU-004-d009-polling-sweep.md) | D-009 polling-intermediary sweep | tree-wide | structural drift | none |

## Selection rationale

- **FU-001** is the smallest and most concrete — single bucket, clear C++ precedents for every gap. Recommended first.
- **FU-002** unblocks B-012 reaction bodies but requires picking an App-threading model; design before implementation.
- **FU-003** is a large upstream port. B-004 row 1479 is the only Tier-B tie-in; gate other emView-dependent work on this.
- **FU-004** is discovery-led — start with a fresh scan, not a row list. Promote sightings (e.g., `emCoreConfigPanel` reset closure) to a real bucket once enumerated.

## Out of scope

Anything not surfaced as a TODO marker, UPSTREAM-GAP, or design-doc Amendment Log entry within Tier-B. New audits should not start here — they should start from a fresh inventory pass.
