# B-005-typed-subscribe-emfileman — P-002 — wire subscribe across emfileman (accessor ready)

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emfileman
**Row count:** 21
**Mechanical-vs-judgement:** mechanical-heavy per `pattern-catalog.md` P-002 entry — accessor present at every site; remediation is a wiring pass per row, with judgement concentrated in a shared subscribe-shape decision rather than per-row.
**Cited decisions:** none — packet `decisions: []` and no entry in `decisions.md` currently applies to a P-002 emfileman row. The bucket-design brainstorm is expected to surface a global subscribe-shape decision (working name `D-006-subscribe-shape`) for the working-memory session to absorb during reconciliation.
**Prereq buckets:** none

## Pattern description

P-002 covers sites where the C++ panel/cycle reacts to a signal via `AddWakeUpSignal` / connect-style subscription, the Rust port has the corresponding accessor exposed (signal id retrievable from the model or widget), but no subscription is wired — reactions are instead inferred from polling widget state inside `Input()` or are absent entirely. In this bucket the scope is `emfileman`: one `emFileLinkPanel` model-update-broadcast site plus 20 `emFileManControlPanel` widget-click / widget-check sites whose reactions currently live inline in `Input()` rather than in a signal-driven `Cycle`.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emFileLinkPanel-53 | src/emFileMan/emFileLinkPanel.cpp:53 | crates/emfileman/src/emFileLinkPanel.rs:175 | present | model-update-broadcast; Cycle does not connect to UpdateSignalModel; accessor at emFileModel.rs:343 |
| emFileManControlPanel-328 | src/emFileMan/emFileManControlPanel.cpp:328 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-check; theme-AR change handled via Input() inspection of theme_ar_group state |
| emFileManControlPanel-329 | src/emFileMan/emFileManControlPanel.cpp:329 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-check; theme style change handled via Input() inspection of theme_style_group |
| emFileManControlPanel-330 | src/emFileMan/emFileManControlPanel.cpp:330 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; reaction inferred from sort_group state read in Input() |
| emFileManControlPanel-331 | src/emFileMan/emFileManControlPanel.cpp:331 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; same drift pattern as -330 |
| emFileManControlPanel-332 | src/emFileMan/emFileManControlPanel.cpp:332 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; same drift pattern |
| emFileManControlPanel-333 | src/emFileMan/emFileManControlPanel.cpp:333 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; same drift pattern |
| emFileManControlPanel-334 | src/emFileMan/emFileManControlPanel.cpp:334 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; same drift pattern |
| emFileManControlPanel-335 | src/emFileMan/emFileManControlPanel.cpp:335 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; same drift pattern |
| emFileManControlPanel-336 | src/emFileMan/emFileManControlPanel.cpp:336 | crates/emfileman/src/emFileManControlPanel.rs:480 | present | widget-check; check_signal fired but never connected; reaction inline in Input() |
| emFileManControlPanel-337 | src/emFileMan/emFileManControlPanel.cpp:337 | crates/emfileman/src/emFileManControlPanel.rs:487 | present | widget-check; same Input()-inline reaction; no connect |
| emFileManControlPanel-338 | src/emFileMan/emFileManControlPanel.cpp:338 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; reaction inferred from nss_group state in Input/sync flow |
| emFileManControlPanel-339 | src/emFileMan/emFileManControlPanel.cpp:339 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; drifted/absent |
| emFileManControlPanel-340 | src/emFileMan/emFileManControlPanel.cpp:340 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; drifted/absent |
| emFileManControlPanel-341 | src/emFileMan/emFileManControlPanel.cpp:341 | crates/emfileman/src/emFileManControlPanel.rs:495 | present | widget-check; Input()-inline reaction |
| emFileManControlPanel-342 | src/emFileMan/emFileManControlPanel.cpp:342 | crates/emfileman/src/emFileManControlPanel.rs:503 | present | widget-click; drifted/absent |
| emFileManControlPanel-343 | src/emFileMan/emFileManControlPanel.cpp:343 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; drifted/absent |
| emFileManControlPanel-344 | src/emFileMan/emFileManControlPanel.cpp:344 | crates/emfileman/src/emFileManControlPanel.rs:518 | present | widget-click; drifted/absent |
| emFileManControlPanel-345 | src/emFileMan/emFileManControlPanel.cpp:345 | crates/emfileman/src/emFileManControlPanel.rs:522 | present | widget-click; drifted/absent |
| emFileManControlPanel-346 | src/emFileMan/emFileManControlPanel.cpp:346 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; drifted/absent |
| emFileManControlPanel-347 | src/emFileMan/emFileManControlPanel.cpp:347 | crates/emfileman/src/emFileManControlPanel.rs:300 | present | widget-click; drifted/absent |

## C++ reference sites

- src/emFileMan/emFileLinkPanel.cpp:53
- src/emFileMan/emFileManControlPanel.cpp:328
- src/emFileMan/emFileManControlPanel.cpp:329
- src/emFileMan/emFileManControlPanel.cpp:330
- src/emFileMan/emFileManControlPanel.cpp:331
- src/emFileMan/emFileManControlPanel.cpp:332
- src/emFileMan/emFileManControlPanel.cpp:333
- src/emFileMan/emFileManControlPanel.cpp:334
- src/emFileMan/emFileManControlPanel.cpp:335
- src/emFileMan/emFileManControlPanel.cpp:336
- src/emFileMan/emFileManControlPanel.cpp:337
- src/emFileMan/emFileManControlPanel.cpp:338
- src/emFileMan/emFileManControlPanel.cpp:339
- src/emFileMan/emFileManControlPanel.cpp:340
- src/emFileMan/emFileManControlPanel.cpp:341
- src/emFileMan/emFileManControlPanel.cpp:342
- src/emFileMan/emFileManControlPanel.cpp:343
- src/emFileMan/emFileManControlPanel.cpp:344
- src/emFileMan/emFileManControlPanel.cpp:345
- src/emFileMan/emFileManControlPanel.cpp:346
- src/emFileMan/emFileManControlPanel.cpp:347

## Open questions for the bucket-design brainstorm

- Pattern catalog entry for P-002-no-subscribe-accessor-present has not been authored — bucket design must either author it first or treat the pattern description above as provisional and reconcile.
- decisions.md contains no resolved D-### entries; the global "subscribe-shape" decision (typed AddWakeUpSignal equivalent vs. closure-callback vs. id-list polled in Cycle) needs to land before per-row wiring, since all 21 rows will adopt the same shape.
- For the 20 `emFileManControlPanel` widget-click/widget-check rows, decide whether the existing Input()-inline reactions stay (and signal subscription only supplements them) or are migrated wholesale into a Cycle handler — affects diff size and whether each row is a 1-line wiring or a logic relocation.
- For `emFileLinkPanel-53` (model-update-broadcast), confirm whether the Cycle should also call `InvalidatePainting`/equivalent on signal, matching C++; this is structurally different from the widget-signal rows and may warrant its own sub-decision.
- Should rows that share an identical Rust line (the many `emFileManControlPanel.rs:300` entries — likely a constructor block) be wired in a single edit or one-per-row to keep the audit trail per C++ site?
- Verification strategy: is there a golden or behavioral test that fires when these signals would have triggered a repaint/state-recompute, or does this bucket need a new test harness before remediation can be evidence-checked?
