# B-007-typed-subscribe-emcore — P-002 — wire subscribe in emcore

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emcore
**Row count:** 3
**Mechanical-vs-judgement:** mechanical-heavy — per pattern catalog, accessor is ready and the work is to connect the consumer's subscribe call.
**Cited decisions:** D-006-subscribe-shape (canonical wiring shape).
**Prereq buckets:** none.

**Reconciliation amendments (2026-04-27, post-design 8b220ebb):**
- `emFileSelectionBox-64` reclassified `gap-blocked → drifted`; `D-003` citation removed. The shared `FileModelsUpdateSignalModel` broadcast IS ported as `App::file_update_signal` at `crates/emcore/src/emGUIFramework.rs:227`; audit-time gap-blocked tag was stale.
- `emFileModel-103` retains `drifted` verdict but design surfaces a latent semantic mis-port: `emFileModel::AcquireUpdateSignalModel` at `emFileModel.rs:343` returns the dead per-model `update_signal` instead of the shared root-context broadcast. Bug fix, not annotated DIVERGED. Captured in row's `reconciliation` field.
- `emImageFile-139` consumer-side fix lands in the SPLIT panel file (`emImageFileImageFilePanel.rs`), not the audit anchor (`emImageFile.rs:85`). Bookkeeping note — design doc's per-row section has the actual implementation site.

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
