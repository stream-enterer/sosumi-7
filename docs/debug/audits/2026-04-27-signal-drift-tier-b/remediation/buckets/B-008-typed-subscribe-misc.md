# B-008-typed-subscribe-misc — P-002 — wire subscribe (misc small scopes)

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** misc (emMainPanel, emVirtualCosmos)
**Row count:** 3
**Mechanical-vs-judgement:** mechanical-heavy
**Cited decisions:** D-003-gap-blocked-fill-vs-stub — these 3 rows are the P-002 gap-blocked subset; per-bucket sketcher must distinguish missing-accessor-on-ported-model (fill in scope) from missing-model-entirely (escalate).
**Prereq buckets:** none

## Pattern description

Accessor exists in Rust; the consumer omits the subscribe call, leaving a one-sided wire. The catalog classifies P-002 as mechanical-heavy — the accessor is ready, just connect. This bucket collects the three gap-tagged P-002 rows in the small misc scopes (emMainPanel, emVirtualCosmos), where the gap is on the source side (signal-emitting infrastructure on `emWindow` flag changes and on `emVirtualCosmosModel`) rather than on the consumer.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emMainPanel-67 | src/emMain/emMainPanel.cpp:67 | crates/emmain/src/emMainPanel.rs:658 | present | EOI signal exists on view but emMainPanel does not subscribe. |
| emMainPanel-69 | src/emMain/emMainPanel.cpp:69 | crates/emmain/src/emMainPanel.rs:658 | present | emWindow has WindowFlags bitflags but no SignalId accessor for flag changes. Same gap as emMainControlPanel-218. |
| emVirtualCosmos-104 | src/emMain/emVirtualCosmos.cpp:104 | crates/emmain/src/emVirtualCosmos.rs:213 | present | Site is in emVirtualCosmosModel constructor (not panel). Same file maps both model and panel; classified as model-side drift in this panel-row file by Task-4 scope. |

## C++ reference sites

- src/emMain/emMainPanel.cpp:67
- src/emMain/emMainPanel.cpp:69
- src/emMain/emVirtualCosmos.cpp:104

## Open questions for the bucket-design brainstorm

- Per D-003: is the emWindow WindowFlags-change signal a missing accessor on a ported model (fill in scope) or a missing model entirely (escalate)? Same question shared with B-007 / emMainControlPanel-218.
- Per D-003: is emVirtualCosmosModel ported at all? Row notes_short is truncated mid-sentence ("has no en…"); confirm whether the gap is "no signal accessor on a ported model" vs "model not yet ported."
- emMainPanel-67 EOI signal: confirm the view-side accessor exists in Rust before classifying as pure consumer-side wiring vs. gap-fill.
- Should this misc bucket merge with the larger P-002 emstocks/emfileman buckets, or stay separate because the gap-fill scope is on different model files?

