# B-002-no-wire-emfileman — P-001 — wire missing accessor + subscribe across emfileman

**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** emfileman
**Row count:** 4
**Mechanical-vs-judgement:** balanced
**Cited decisions:** D-003-gap-blocked-fill-vs-stub — governs whether each missing accessor is filled in scope or escalated when the underlying model itself is unported.
**Prereq buckets:** none

## Pattern description

Rust path neither subscribes nor exposes the C++-side signal accessor — both ends of the wire are missing, so the consumer cannot observe model-change or timer events that fire in C++. Fix shape is to port the missing accessor on the model side and then wire the consumer subscribe at the panel side, both halves landing in the same scope. In this bucket the missing wires span an `emTimer`-driven idle expiry in `emDirPanel` and an `emRecFileModel`-inherited change signal that `emFileLinkPanel` needs to re-subscribe across `SetFileModel` swaps.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emDirPanel-432 | src/emFileMan/emDirPanel.cpp:432 | crates/emfileman/src/emDirPanel.rs:178 | missing | Rust uses Instant compare at next Input call; C++ uses 1000ms Timer + AddWakeUpSignal for idle ClearKeyWalkState. |
| emFileLinkPanel-56 | src/emFileMan/emFileLinkPanel.cpp:56 | crates/emfileman/src/emFileLinkPanel.rs:175 | missing | C++ subscribes to emFileLinkModel record-change signal (emRecFileModel::GetChangeSignal); Rust exposes only GetFileStateSignal. |
| emFileLinkPanel-72 | src/emFileMan/emFileLinkPanel.cpp:72 | crates/emfileman/src/emFileLinkPanel.rs:175 | missing | C++ AddWakeUpSignal inside SetFileModel re-attaches subscription on model swap; Rust has no analogous hook. |
| emFileLinkModel-accessor-model-change | n/a | crates/emfileman/src/emFileLinkModel.rs:265 | missing | emRec hierarchy lacks change-signal exposure; fix requires propagating GetChangeSignal up emStructRec-derived models (also affects emAutoplay, emVirtualCosmos). |

## C++ reference sites

- src/emFileMan/emDirPanel.cpp:432
- src/emFileMan/emFileLinkPanel.cpp:56
- src/emFileMan/emFileLinkPanel.cpp:72

## Open questions for the bucket-design brainstorm

- Per D-003: is each missing accessor a missing accessor on a ported model (fill in scope) or a missing model entirely (escalate as out-of-scope)? The `emFileLinkModel` change-signal accessor depends on `emRecFileModel` / `emStructRec` change-signal infrastructure — confirm whether that base infrastructure is ported before committing to fill-in-scope.
- The `emFileLinkModel-accessor-model-change` row implicates the broader emRec hierarchy and is referenced by emAutoplay and emVirtualCosmos consumers outside this bucket — decide whether the accessor port lands here (and is consumed later by other buckets) or is lifted into a shared prereq bucket.
- For `emDirPanel-432`: confirm the Rust replacement (Instant compare at next Input) is observable drift versus C++ timer-driven wakeup, and whether the fix requires porting the emTimer + AddWakeUpSignal pair or can reuse an existing Rust timer primitive.
- For `emFileLinkPanel-72`: confirm the re-subscribe-on-SetFileModel hook needs a dedicated Rust setter point, or whether subscription lifetime can be tied to the model handle directly.
