# B-019-stale-annotations — P-009 — clean up failed-revalidation DIVERGED annotations

**Pattern:** P-009-stale-annotation
**Scope:** all (emcore, emfileman, emmain:emMainControlPanel)
**Row count:** 9
**Mechanical-vs-judgement:** mechanical-heavy — per the P-009 catalog entry, annotation removal is mechanical; any underlying drift-fix joins its natural pattern bucket.
**Cited decisions:** D-001-typemismatch-accessor-policy — governs the one wrong-category cleanup item (emFileModel.rs:490) where the `u64`/`SignalId` accessor flip determines the corrected annotation text.
**Prereq buckets:** none

## Pattern description

This bucket is the audit's `preexisting-diverged.csv` cleanup track: pre-existing `DIVERGED:` annotations whose re-validation failed the four-question test (8 entries) or carried a wrong category tag (1 entry, `emFileModel.rs:490` → corrected to `language-forced`). It is separate from the drift-fix patterns P-001 through P-008 — the annotation cleanup itself lives here, while any underlying drift-fix the annotation was masking lands wherever its row would naturally bucket. Mechanical-heavy: most rows resolve to annotation removal or rewording.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| cleanup-emDirPanel-117 | — | crates/emfileman/src/emDirPanel.rs:117 | — | preexisting-diverged.csv cleanup item |
| cleanup-emMainControlPanel-35 | — | crates/emmain/src/emMainControlPanel.rs:35 | — | preexisting-diverged.csv cleanup item |
| cleanup-emMainControlPanel-303 | — | crates/emmain/src/emMainControlPanel.rs:303 | — | preexisting-diverged.csv cleanup item |
| cleanup-emMainControlPanel-320 | — | crates/emmain/src/emMainControlPanel.rs:320 | — | preexisting-diverged.csv cleanup item |
| cleanup-emDialog-35 | — | crates/emcore/src/emDialog.rs:35 | — | preexisting-diverged.csv cleanup item |
| cleanup-emDialog-523 | — | crates/emcore/src/emDialog.rs:523 | — | preexisting-diverged.csv cleanup item |
| cleanup-emFileDialog-68 | — | crates/emcore/src/emFileDialog.rs:68 | — | preexisting-diverged.csv cleanup item |
| cleanup-emFileDialog-140 | — | crates/emcore/src/emFileDialog.rs:140 | — | preexisting-diverged.csv cleanup item |
| cleanup-emFileModel-490 | — | crates/emcore/src/emFileModel.rs:490 | — | wrong-category cleanup; corrected: language-forced; interacts with D-001 |

## C++ reference sites

- N/A — cleanup items reference Rust annotations only.

## Open questions for the bucket-design brainstorm

- Per-row decision tree: for each of the 8 failed-revalidation rows, is the resolution (a) remove the annotation outright (no real divergence), (b) replace with a corrected category tag, or (c) keep but rewrite the justification — and what evidence drives that call?
- For `emFileModel.rs:490` (the wrong-category row), does D-001's chosen direction (flip accessor to `SignalId`) make the annotation moot (delete) or does the corrected `language-forced` text still apply to a residual divergence after the flip?
- Should this bucket land as a single PR (mechanical-heavy, scattered files) or split into per-file follow-ups (emcore vs emfileman vs emmain) to keep diffs reviewable?
- For rows that turn out to mask an underlying drift, do we file the cross-reference back to the natural P-001..P-008 bucket here, or only in the receiving bucket?
- Does the annotation-lint xtask (`cargo xtask annotations`) need any change to catch the wrong-category class proactively, or is the existing category-required check already sufficient post-cleanup?
- Sequencing relative to D-001's accessor flip: do we wait for the flip to land before touching `emFileModel.rs:490`, or co-locate the annotation correction in the same PR?
