# B-006-typed-subscribe-mainctrl — P-002 — wire subscribe in emMainControlPanel

**Pattern:** P-002-no-subscribe-accessor-present
**Scope:** emmain:emMainControlPanel
**Row count:** 3
**Mechanical-vs-judgement:** mechanical-heavy
**Cited decisions:** D-003-gap-blocked-fill-vs-stub — applies to row 218 where the WindowFlags signal accessor is missing on emWindow and must be filled in-bucket before the panel can subscribe.
**Prereq buckets:** none

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
