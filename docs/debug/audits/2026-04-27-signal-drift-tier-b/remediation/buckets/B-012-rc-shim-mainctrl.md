# B-012-rc-shim-mainctrl — P-004 — convert rc-shim consumers in emMainControlPanel

**Pattern:** P-004-rc-shim-instead-of-signal
**Scope:** emmain:emMainControlPanel
**Row count:** 7
**Mechanical-vs-judgement:** judgement-heavy
**Cited decisions:** D-002-rc-shim-policy — applies the per-row triage rule (default convert; keep only for dialog-result/post-cycle handoff) to each of the 7 click-handler shims here.
**Prereq buckets:** none

## Pattern description

Accessor is present but the Rust consumer routes around the signal by sharing `Rc<RefCell<>>` / `Rc<Cell<>>` state into click-handler closures, hiding the signal from any other observer. This observably changes timing (closures fire vs signals fire), so it is a drift, not a below-surface adaptation. In this bucket the 7 sites are all `widget-click` handlers in `emMainControlPanel` — each must be triaged against the C++ original to decide convert (rule 1) vs keep (rule 2).

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emMainControlPanel-220 | src/emMain/emMainControlPanel.cpp:220 | crates/emmain/src/emMainControlPanel.rs:296 | present | widget-click rc_cell_shim |
| emMainControlPanel-221 | src/emMain/emMainControlPanel.cpp:221 | crates/emmain/src/emMainControlPanel.rs:301 | present | widget-click rc_cell_shim |
| emMainControlPanel-222 | src/emMain/emMainControlPanel.cpp:222 | crates/emmain/src/emMainControlPanel.rs:311 | present | widget-click rc_cell_shim |
| emMainControlPanel-223 | src/emMain/emMainControlPanel.cpp:223 | crates/emmain/src/emMainControlPanel.rs:315 | present | widget-click rc_cell_shim |
| emMainControlPanel-224 | src/emMain/emMainControlPanel.cpp:224 | crates/emmain/src/emMainControlPanel.rs:319 | present | widget-click rc_cell_shim |
| emMainControlPanel-225 | src/emMain/emMainControlPanel.cpp:225 | crates/emmain/src/emMainControlPanel.rs:328 | present | widget-click rc_cell_shim |
| emMainControlPanel-226 | src/emMain/emMainControlPanel.cpp:226 | crates/emmain/src/emMainControlPanel.rs:334 | present | widget-click rc_cell_shim |

## C++ reference sites

- src/emMain/emMainControlPanel.cpp:220
- src/emMain/emMainControlPanel.cpp:221
- src/emMain/emMainControlPanel.cpp:222
- src/emMain/emMainControlPanel.cpp:223
- src/emMain/emMainControlPanel.cpp:224
- src/emMain/emMainControlPanel.cpp:225
- src/emMain/emMainControlPanel.cpp:226

## Open questions for the bucket-design brainstorm

- For each of the 7 rows, does the C++ original use a signal accessor + subscribe at the consumer site (rule 1, convert) or a member field assigned post-finish/post-cycle (rule 2, keep)? Confirm row-by-row before bucketing into convert-set vs keep-set.
- Are any rows ambiguous enough to require escalation to the working-memory session per D-002's rule 3?
- Do the 7 click handlers share a common observer (e.g., a sibling control panel or the main view) such that conversion can reuse a single signal subscription, or does each need its own?
