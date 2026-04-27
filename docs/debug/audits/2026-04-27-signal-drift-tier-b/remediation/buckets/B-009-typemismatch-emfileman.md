# B-009-typemismatch-emfileman — P-003 — flip emfileman u64 accessors to SignalId + migrate consumers

**Pattern:** P-003-typemismatch-blocks-subscribe
**Scope:** emfileman
**Row count:** 14
**Mechanical-vs-judgement:** judgement-heavy
**Cited decisions:** D-001-typemismatch-accessor-policy (governs the accessor-flip vs adapt-consumer call for all 3 emfileman u64 accessors in this bucket), D-003-gap-blocked-fill-vs-stub (applies to the 3 gap-blocked rows where the accessor must be ported in-bucket), D-005-poll-replacement-shape (direct-subscribe shape for the 4 polling consumer rows being migrated)
**Prereq buckets:** none

## Pattern description

Accessor exists in Rust but returns `u64` where C++ exposes `const emSignal&`, blocking idiomatic subscribe; consumers either poll the generation counter or omit the reaction entirely. All occurrences live in emfileman across 3 distinct accessors (`emFileManModel::GetSelectionSignal`, `emFileManModel::GetCommandsSignal`, `emFileManViewConfig::GetChangeSignal`). This bucket flips those three accessors to `SignalId` and migrates the 11 consumer call sites in the same scope.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emDirEntryAltPanel-35 | src/emFileMan/emDirEntryAltPanel.cpp:35 | crates/emfileman/src/emDirEntryAltPanel.rs:154 | type-mismatch | Selection signal: never connect()s nor polls; only config-change polled |
| emDirEntryAltPanel-36 | src/emFileMan/emDirEntryAltPanel.cpp:36 | crates/emfileman/src/emDirEntryAltPanel.rs:160 | type-mismatch | Polls u64 config gen in Cycle; no connect() in file |
| emDirEntryPanel-55 | src/emFileMan/emDirEntryPanel.cpp:55 | crates/emfileman/src/emDirEntryPanel.rs:878 | type-mismatch | Cycle unconditionally updates; author comment notes absent IsSignaled |
| emDirEntryPanel-56 | src/emFileMan/emDirEntryPanel.cpp:56 | crates/emfileman/src/emDirEntryPanel.rs:878 | type-mismatch | Unconditional forceRelayout=true; no connect or polled config gen |
| emDirPanel-38 | src/emFileMan/emDirPanel.cpp:38 | crates/emfileman/src/emDirPanel.rs:331 | type-mismatch | Polls u64 config gen; no connect/IsSignaled |
| emDirStatPanel-39 | src/emFileMan/emDirStatPanel.cpp:39 | crates/emfileman/src/emDirStatPanel.rs:109 | type-mismatch | Config acquired in new() but Cycle never reads GetChangeSignal |
| emFileLinkPanel-55 | src/emFileMan/emFileLinkPanel.cpp:55 | crates/emfileman/src/emFileLinkPanel.rs:175 | type-mismatch | Config field exists but Cycle never reads GetChangeSignal |
| emFileManControlPanel-326 | src/emFileMan/emFileManControlPanel.cpp:326 | crates/emfileman/src/emFileManControlPanel.rs:300 | type-mismatch | C++ reacts to selection signal for UpdateButtonStates; Rust ignores it |
| emFileManControlPanel-327 | src/emFileMan/emFileManControlPanel.cpp:327 | crates/emfileman/src/emFileManControlPanel.rs:305 | type-mismatch | u64 gen-counter polling for config-change |
| emFileManControlPanel-522 | src/emFileMan/emFileManControlPanel.cpp:522 | crates/emfileman/src/emFileManControlPanel.rs:300 | type-mismatch | C++ commands-signal subscription on sub-engine absent in Rust |
| emFileManSelInfoPanel-37 | src/emFileMan/emFileManSelInfoPanel.cpp:37 | crates/emfileman/src/emFileManSelInfoPanel.rs:650 | type-mismatch | u64 selection-gen polling; Cycle stays awake while scanning details |
| emFileManViewConfig-accessor-config-change |  | crates/emfileman/src/emFileManViewConfig.rs:428 | type-mismatch | C++ returns const emSignal&; Rust returns u64 generation; allocate SignalId |
| emFileManModel-accessor-command |  | crates/emfileman/src/emFileManModel.rs:543 | type-mismatch | C++ returns const emSignal&; Rust returns u64; allocate SignalId for commands |
| emFileManModel-accessor-selection |  | crates/emfileman/src/emFileManModel.rs:540 | type-mismatch | C++ returns const emSignal&; Rust returns u64; consumers poll cached gen |

## C++ reference sites

- src/emFileMan/emDirEntryAltPanel.cpp:35
- src/emFileMan/emDirEntryAltPanel.cpp:36
- src/emFileMan/emDirEntryPanel.cpp:55
- src/emFileMan/emDirEntryPanel.cpp:56
- src/emFileMan/emDirPanel.cpp:38
- src/emFileMan/emDirStatPanel.cpp:39
- src/emFileMan/emFileLinkPanel.cpp:55
- src/emFileMan/emFileManControlPanel.cpp:326
- src/emFileMan/emFileManControlPanel.cpp:327
- src/emFileMan/emFileManControlPanel.cpp:522
- src/emFileMan/emFileManSelInfoPanel.cpp:37

## Open questions for the bucket-design brainstorm

- Single PR vs staged (accessor-flip then per-consumer follow-ups) — D-001 leaves to bucket; mechanical nature of the consumer migration argues for one PR but bucket sketcher decides.
- Whether `emFileManViewConfig::GetChangeSignal` and `emFileManModel::GetSelectionSignal` / `GetCommandsSignal` flip in one bucket-internal commit or separate commits per accessor (D-001 deferral).
- For the 3 gap-blocked rows (D-003 deferral): confirm each accessor lives on a ported model (in-scope fill) rather than a missing model entirely (escalate); the three accessor rows here all sit on already-ported `emFileManModel` / `emFileManViewConfig`, so likely safe — confirm during sketch.
- For the 4 polling consumers (emDirEntryAltPanel-36, emDirPanel-38, emFileManControlPanel-327, emFileManSelInfoPanel-37): D-005 picks direct-subscribe; confirm the C++ original subscribes to a single signal vs aggregated (default mirror C++).
- Whether emFileManControlPanel-522 (`GetCommandsSignal` subscription on a sub-engine in C++) fits the same direct-subscribe shape or needs sub-engine routing not yet present in the Rust port.
- Drop/retain order for the obsolete `cached_change_gen` / generation-counter fields on consumer panels once subscriptions land (cleanup hygiene, not design).
