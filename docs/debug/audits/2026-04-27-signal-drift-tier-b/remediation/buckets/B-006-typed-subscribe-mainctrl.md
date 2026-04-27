# B-006-typed-subscribe-mainctrl — P-002 — wire subscribe in emMainControlPanel

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emmain:emMainControlPanel
**Row count:** 3
**Mechanical-vs-judgement:** mechanical-heavy
**Cited decisions:** D-006-subscribe-shape (canonical wiring shape).
**Prereq buckets:** none.

**Reconciliation amendments (2026-04-27, post-design a13880c7):**
- Row 218 reclassified `gap-blocked → drifted` and `D-003` citation removed: `GetWindowFlagsSignal` already exists at `crates/emcore/src/emWindow.rs:1279` returning `SignalId`. Audit-time tag was stale.
- Row 217 marked `resolved_by` → `crates/emmain/src/emMainWindow.rs:825` (`ControlPanelBridge` engine; existing dependency-forced `DIVERGED` annotation captures the SubView/Framework scope split). Row stays in bucket for completeness; design-doc treats it as no-action.

## Pattern description

P-002 is the one-sided-wire pattern: the Rust signal accessor is already present on the upstream model, but the consumer panel never calls `subscribe`, so the wire dangles at the consumer end. Fixing it is mechanical — connect the existing accessor to the panel's signal handler and react in `Cycle()`. In this bucket the consumer is `emMainControlPanel`, which in C++ subscribes to three sources at construction (cycle/recreate-control-panel, window flags, and a config change) but in Rust subscribes to none.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emMainControlPanel-217 | src/emMain/emMainControlPanel.cpp:217 | crates/emmain/src/emMainControlPanel.rs:287 | present | ControlPanelBridge in emMainWindow.rs is the actual subscriber; C++ Cycle reaction (RecreateContentControlPanel) absent here. |
| emMainControlPanel-218 | src/emMain/emMainControlPanel.cpp:218 | crates/emmain/src/emMainControlPanel.rs:287 | present | emWindow.WindowFlags exists but no GetWindowFlagsSignal accessor — gap-blocked; D-003 applies. |
| emMainControlPanel-219 | src/emMain/emMainControlPanel.cpp:219 | crates/emmain/src/emMainControlPanel.rs:287 | present | Accessor exists; panel could subscribe but does not. |

## C++ reference sites

- src/emMain/emMainControlPanel.cpp:217
- src/emMain/emMainControlPanel.cpp:218
- src/emMain/emMainControlPanel.cpp:219

## Open questions for the bucket-design brainstorm

- Row 218: per D-003, confirm the gap is a missing accessor on a ported model (`emWindow` is ported — fill in scope) versus a missing model entirely (would escalate out of bucket). Initial read: accessor-only gap, in-scope fill.
- Row 217: the C++ subscribe drives `RecreateContentControlPanel`; in Rust the recreate path is currently routed through `ControlPanelBridge` in `emMainWindow.rs`. Decide whether to (a) move the subscribe back into `emMainControlPanel` to mirror C++ structurally or (b) document the bridge as a preserved-design-intent divergence. Per Port Ideology default, (a) wins absent a forced reason for (b).
- Confirm all three subscribes target distinct `SignalId`s in C++ (cycle, window-flags, config) and that the Rust panel's `Cycle()` can dispatch all three without re-entrancy issues.
- Naming for the new `GetWindowFlagsSignal` accessor on `emWindow` — match C++ exactly per File and Name Correspondence.
