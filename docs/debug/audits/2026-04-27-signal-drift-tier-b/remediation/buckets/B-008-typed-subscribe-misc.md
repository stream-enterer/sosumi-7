# B-008-typed-subscribe-misc — P-002 — wire subscribe (misc small scopes)

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** misc (emMainPanel, emVirtualCosmos)
**Row count:** 3
**Mechanical-vs-judgement:** mechanical-heavy
**Cited decisions:** D-006-subscribe-shape (canonical wiring shape).
**Prereq buckets:** B-007-typed-subscribe-emcore (hard, for `emVirtualCosmos-104` only — depends on B-007 step 1 re-pointing `emFileModel::AcquireUpdateSignalModel` to `App::file_update_signal`).

**Reconciliation amendments (2026-04-27, post-design 4c4141f1):**
- `emMainPanel-69` reclassified `gap-blocked → drifted`; `D-003` citation removed. `GetWindowFlagsSignal` exists at `crates/emcore/src/emWindow.rs:1279` (same stale tag as `emMainControlPanel-218`).
- `emMainPanel-67` and `emVirtualCosmos-104` were already `drifted` in the spine — designer's intuition that all three were stale-gap-blocked was partially correct, but only -69 actually had the stale tag.
- `emVirtualCosmos-104` carries a hard cross-bucket prereq on `emFileModel-103` (B-007 row), encoded in `inventory-enriched.json` `prereq_ids`.
- **Back-propagation from D-007 promotion (2026-04-27, post-B-009):** B-008's `Acquire`-side encounter with the mutator-fire ectx-threading pattern was the first sighting that B-009's promotion of D-007 cites. B-008's design implementer should follow D-007's chosen direction (thread `&mut EngineCtx<'_>`) for any mutator-side work, and D-008 (lazy `Ensure*Signal` allocation) for any signal allocation.

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

