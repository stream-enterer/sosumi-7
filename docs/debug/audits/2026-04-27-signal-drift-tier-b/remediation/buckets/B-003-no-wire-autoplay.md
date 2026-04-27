# B-003-no-wire-autoplay — P-001 — wire missing accessor + subscribe in emAutoplay

**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** emmain:emAutoplay
**Row count:** 3
**Mechanical-vs-judgement:** balanced — wiring is mechanical once the accessor shape is decided; the accessor shape is a per-scope judgement call.
**Cited decisions:** D-003-gap-blocked-fill-vs-stub — bucket fills the missing emAutoplayViewModel accessors in scope, then wires the panel subscribe.
**Prereq buckets:** none

## Pattern description

Rust path neither subscribes nor exposes the C++-side signal accessor; both ends of the wire are missing. Within this bucket the missing accessors live on `emAutoplayViewModel` (model-change and model-state signals) and the unwired consumer is the `emAutoplayControlPanel` port at `emAutoplay.rs:658`. Per D-003, the gap-fill (accessor port) and the consumer wire belong in the same bucket because both halves live in the same scope.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emAutoplay-1171 | src/emMain/emAutoplay.cpp:1171 | crates/emmain/src/emAutoplay.rs:658 | missing | ControlPanel port split into emAutoplayControlPanel.rs; references view-model |
| emAutoplayViewModel-accessor-model-change | n/a | crates/emmain/src/emAutoplay.rs:800 | missing | Panel references view-model; config-side accessor at line 129 is a different object |
| emAutoplayViewModel-accessor-model-state | n/a | crates/emmain/src/emAutoplay.rs:800 | missing | model-state signal accessor absent on view-model |

## C++ reference sites

- src/emMain/emAutoplay.cpp:1171

## Open questions for the bucket-design brainstorm

- Per D-003: confirm whether the gap is a missing accessor on a ported model (fill in scope) or a missing model entirely (escalate). Is `emAutoplayViewModel` ported, with only the accessor methods absent, or is the view-model itself unported?
- Two view-model accessors are missing (model-change, model-state). Confirm both are present in the C++ original and clarify which signal each consumer site at `emAutoplay.rs:658` actually needs to subscribe to.
- The CSV `rust_file=emAutoplay.rs` reflects the C++ basename mapping but the actual port lives in `emAutoplayControlPanel.rs` (split file). Confirm the wire-up site is the split file, not the basename file, before bucket execution.
- The config-side accessor at `emAutoplay.rs:129` is noted as a different object from the view-model. Confirm no cross-wiring is needed between the config accessor and the view-model accessors.
