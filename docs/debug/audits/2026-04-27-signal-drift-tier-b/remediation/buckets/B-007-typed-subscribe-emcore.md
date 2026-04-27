# B-007-typed-subscribe-emcore — P-002 — wire subscribe in emcore

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emcore
**Row count:** 3
**Mechanical-vs-judgement:** mechanical-heavy — per pattern catalog, accessor is ready and the work is to connect the consumer's subscribe call.
**Cited decisions:** D-003-gap-blocked-fill-vs-stub — two of three rows reference upstream broadcast models (`FileModelsUpdateSignalModel`, shared `UpdateSignalModel`) that may be unported; per D-003 we fill the gap inside this bucket where the gap is a missing accessor on a ported model, and escalate where the entire upstream model is missing.
**Prereq buckets:** none

## Pattern description

Accessor exists in Rust; consumer omits the subscribe call, leaving a one-sided wire where the signal is exposed but never observed. Mechanical fix: add the missing `AddWakeUpSignal`/subscribe call at the consumer site so the existing accessor's signal feeds the existing reaction path. In this bucket the three emcore consumers all sit near file-model update broadcasts, so each row also requires a check that the upstream broadcast model itself is ported before the wire can carry traffic.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emFileSelectionBox-64 | src/emCore/emFileSelectionBox.cpp:64 | crates/emcore/src/emFileSelectionBox.rs:1494 | present | Subscribes to global `FileModelsUpdateSignalModel->Sig`; Rust lacks that broadcast model. |
| emImageFile-139 | src/emCore/emImageFile.cpp:139 | crates/emcore/src/emImageFile.rs:85 | present | `GetChangeSignal` exists; AddWakeUpSignal/RemoveWakeUpSignal pair around SetFileModel absent. |
| emFileModel-103 | src/emCore/emFileModel.cpp:103 | crates/emcore/src/emFileModel.rs:483 | present | Model-internal subscribe to shared `UpdateSignalModel`; Rust accessor exists, wake-up subscription absent. |

## C++ reference sites

- src/emCore/emFileModel.cpp:103
- src/emCore/emFileSelectionBox.cpp:64
- src/emCore/emImageFile.cpp:139

## Open questions for the bucket-design brainstorm

- Per D-003: is `FileModelsUpdateSignalModel` (referenced by emFileSelectionBox-64) a missing accessor on a ported model, or a missing model entirely? If the latter, escalate — the bucket cannot complete without out-of-scope porting.
- Per D-003: same check for the shared `UpdateSignalModel` referenced by emFileModel-103 — ported model with missing accessor (fill in scope) or missing model entirely (escalate)?
- Does emImageFile-139 require any upstream-gap fill, or is it a pure consumer-side wire-up (the notes suggest the accessor and reaction path both exist)?
- Should the gap-fill (if needed) for the cross-process file-mtime broadcast be co-designed with the consumer wires in emFileSelectionBox/emFileModel, or split into a prereq bucket?
