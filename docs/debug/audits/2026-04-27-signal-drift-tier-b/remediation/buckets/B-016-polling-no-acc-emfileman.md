# B-016-polling-no-acc-emfileman — P-007 — emfileman polling consumers + missing accessors

**Pattern:** P-007-polling-accessor-missing
**Scope:** emfileman
**Row count:** 3
**Mechanical-vs-judgement:** balanced
**Cited decisions:** D-005-poll-replacement-shape (governs subscribe-shape — direct subscribe collapsing the polling code into the callback, mirroring C++ Cycle()-driven re-read)
**Prereq buckets:** none

## Pattern description

Polling consumer plus missing accessor: the Rust consumer re-reads `vir-file-state` per `Cycle()` invocation while the upstream `emFilePanel` exposes neither a `GetVirFileStateSignal` returning `SignalId` nor any subscribe-able handle. Both ends of the wire are absent — the accessor must be ported and the consumer migrated to a direct subscribe in one motion. In this bucket all three rows poll the same `emFilePanel` vir-file-state surface from sibling fileman panels.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emDirPanel-37 | src/emFileMan/emDirPanel.cpp:37 | crates/emfileman/src/emDirPanel.rs:344 | missing | Cycle polls dir_model.borrow().get_file_state(); stay_awake=true while Loading/Waiting; no connect |
| emDirStatPanel-30 | src/emFileMan/emDirStatPanel.cpp:30 | crates/emfileman/src/emDirStatPanel.rs:109 | missing | Cycle polls vir-file-state every wake; no connect, no IsSignaled; depends on external wake |
| emFileLinkPanel-54 | src/emFileMan/emFileLinkPanel.cpp:54 | crates/emfileman/src/emFileLinkPanel.rs:175 | missing | Cycle polls via refresh_vir_file_state(); no connect or IsSignaled; SignalId not published |

## C++ reference sites

- src/emFileMan/emDirPanel.cpp:37
- src/emFileMan/emDirStatPanel.cpp:30
- src/emFileMan/emFileLinkPanel.cpp:54

## Open questions for the bucket-design brainstorm

- Per D-005: confirm the C++ original here uses a single `GetVirFileStateSignal` subscribe per consumer (vs. an aggregated signal), so the Rust port mirrors C++ subscribe-arity exactly.
- Accessor design: what is the canonical Rust shape for `emFilePanel::GetVirFileStateSignal` — `SignalId` returned by value, mirroring the other emFilePanel signal accessors? Confirm against neighboring accessors before porting.
- Should the accessor port land as a separate prereq commit, or in the same bucket PR as the three consumer migrations? (D-005 leaves PR-staging to bucket sketcher.)
- For `emDirStatPanel-30` (currently returns `false` / no stay-awake, depends on external wake): does collapsing the polling code into the subscribe callback change observable wake behavior vs. C++? Verify the C++ Cycle() does not also rely on an external wake source.
- Confirm none of the three sites poll *additional* signals beyond vir-file-state that would require multi-subscribe per D-005's deferred multi-source question.
