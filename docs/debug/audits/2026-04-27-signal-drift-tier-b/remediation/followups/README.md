# Tier-B Follow-up Buckets

**Status: Closed 2026-05-02.** All 5 buckets implemented, merged to `main`, and pushed. Tier-B itself was closed 2026-05-01 (19/19 buckets, 212/212 rows, 3003 nextest tests passing). The buckets below cover residual stubs and architectural gaps that were intentionally scoped out, all originally tracked as `TODO(B-0XX-followup)` markers or `UPSTREAM-GAP` annotations.

## Buckets

| ID | Title | Status | Spec | Plan | Merge |
|---|---|---|---|---|---|
| [FU-001](FU-001-emstocks-reaction-bodies.md) | emstocks reaction-body completion + emCheckBox click_signal mirror | Closed 2026-05-02 | [spec](../../../../superpowers/specs/2026-05-02-FU-001-emstocks-reaction-bodies-design.md) | [plan](../../../../superpowers/plans/2026-05-02-FU-001-emstocks-reaction-bodies.md) | `a4a44d8f` |
| [FU-002](FU-002-app-bound-reactions.md) | App-bound reaction wiring (mainctrl) | Closed 2026-05-02 | [spec](../../../../superpowers/specs/2026-05-02-FU-002-app-bound-reactions-design.md) | [plan](../../../../superpowers/plans/2026-05-02-FU-002-app-bound-reactions.md) | `30d2b79f` |
| [FU-003](FU-003-emview-multiview-port.md) | bookmark navigation completion (rescoped 2026-05-02 from "emView multi-view port") | Closed 2026-05-02 | [spec](../../../../superpowers/specs/2026-05-02-FU-003-bookmark-navigation-completion-design.md) | [plan](../../../../superpowers/plans/2026-05-02-FU-003-bookmark-navigation-completion.md) | `2281f9ae` |
| [FU-004](FU-004-d009-polling-sweep.md) | D-009 polling-intermediary sweep / inventory | Closed 2026-05-02 | [spec](../../../../superpowers/specs/2026-05-02-FU-004-d009-polling-sweep-design.md) | [plan](../../../../superpowers/plans/2026-05-02-FU-004-d009-inventory.md) | `6e3f6f59` |
| [FU-005](FU-005-emfilemodel-state-signal-conflation.md) | emFileModel file-state-signal conflation fix (carved off FU-001 2026-05-02) | Closed 2026-05-02 | [spec](../../../../superpowers/specs/2026-05-02-FU-005-emfilemodel-state-signal-conflation-design.md) | [plan](../../../../superpowers/plans/2026-05-02-FU-005-emfilemodel-state-signal-conflation.md) | `e0e01500` |

Residual divergences surfaced post-implementation are recorded in `docs/scratch/2026-05-02-future-work-dump.md` (commit `0707e79e`).

## Selection rationale

- **FU-001** is the smallest and most concrete — single bucket, clear C++ precedents for every gap. Recommended first.
- **FU-002** unblocks B-012 reaction bodies but requires picking an App-threading model; design before implementation.
- **FU-003** was rescoped 2026-05-02 from "large upstream port" to a small wire-up bucket — research showed the multi-view infrastructure is already ported; only bookmark hotkey wiring + a stale comment need fixing.
- **FU-004** is discovery-led — start with a fresh scan, not a row list. Promote sightings (e.g., `emCoreConfigPanel` reset closure) to a real bucket once enumerated.
- **FU-005** is well-scoped (signal-shape fix at base class + ~10 fire-site wirings + delegation chain). Brainstorm should settle the (α) split-vs-(β) rename design choice before plan-writing. Removes the 3 UPSTREAM-GAP markers downstream.

## Out of scope

Anything not surfaced as a TODO marker, UPSTREAM-GAP, or design-doc Amendment Log entry within Tier-B. New audits should not start here — they should start from a fresh inventory pass.
