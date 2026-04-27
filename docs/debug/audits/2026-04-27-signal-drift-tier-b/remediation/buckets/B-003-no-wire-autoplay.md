# B-003-no-wire-autoplay — P-001 — wire missing accessor + subscribe in emAutoplay

**Pattern:** P-001-no-subscribe-no-accessor

**Reconciliation amendments (2026-04-27, post-design 703fa462):**
- **D-002 §1 deferred question resolved here.** Working-memory ratified R-A (drop AutoplayFlags entirely) per B-003 designer's recommendation. AutoplayFlags inbound `Cell`s are dead code; existing `DIVERGED` annotation at `emAutoplayControlPanel.rs:84` is factually wrong. R-A matches C++; outbound `progress: Rc<Cell<f64>>` replaced with `Rc<RefCell<emAutoplayViewModel>>` reading `GetItemProgress()` in `Paint`. See `decisions.md` D-002 entry.
- **Row renamed:** `emAutoplayViewModel-accessor-model-state` → `emAutoplayViewModel-accessor-progress` (C++ second signal is `ProgressSignal`, not state). Updated in `inventory-enriched.json` and the row table above.
- **2 accessor groups (G1, G2):** G1 `emAutoplayViewModel.GetChangeSignal()` (1 row, 6 emit sites in mutator methods), G2 `emAutoplayViewModel.GetProgressSignal()` (1 row, emit at `SetItemProgress` only).
- **Wire scope:** `emAutoplay-1171` expands to a Cycle fan-out: 2 model subscribes + 7 widget subscribes. `emAutoplayControlPanel` has no `Cycle` method today; adding it is in-scope. Audit's row count of 1 is correct (single C++ site) but implementer scope is larger.
**Scope:** emmain:emAutoplay
**Row count:** 3
**Mechanical-vs-judgement:** balanced — wiring is mechanical once the accessor shape is decided; the accessor shape is a per-scope judgement call.
**Cited decisions:** D-003-gap-blocked-fill-vs-stub — bucket fills the missing emAutoplayViewModel accessors in scope, then wires the panel subscribe.
**Prereq buckets:** none

## Pattern description

Rust path neither subscribes nor exposes the C++-side signal accessor; both ends of the wire are missing. Within this bucket the missing accessors live on `emAutoplayViewModel` (model-change and model-state signals) and the unwired consumer is the `emAutoplayControlPanel` port at `emAutoplay.rs:658`. Per D-003, the gap-fill (accessor port) and the consumer wire belong in the same bucket because both halves live in the same scope.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emAutoplay-1171 | src/emMain/emAutoplay.cpp:1171 | crates/emmain/src/emAutoplay.rs:658 | missing | ControlPanel port split into emAutoplayControlPanel.rs; references view-model |
| emAutoplayViewModel-accessor-model-change | n/a | crates/emmain/src/emAutoplay.rs:800 | missing | Panel references view-model; config-side accessor at line 129 is a different object |
| emAutoplayViewModel-accessor-progress | n/a | crates/emmain/src/emAutoplay.rs:800 | missing | progress signal accessor absent on view-model (renamed from -accessor-model-state per B-003 design 703fa462) |

## C++ reference sites

- src/emMain/emAutoplay.cpp:1171

## Open questions for the bucket-design brainstorm

- Per D-003: confirm whether the gap is a missing accessor on a ported model (fill in scope) or a missing model entirely (escalate). Is `emAutoplayViewModel` ported, with only the accessor methods absent, or is the view-model itself unported?
- Two view-model accessors are missing (model-change, model-state). Confirm both are present in the C++ original and clarify which signal each consumer site at `emAutoplay.rs:658` actually needs to subscribe to.
- The CSV `rust_file=emAutoplay.rs` reflects the C++ basename mapping but the actual port lives in `emAutoplayControlPanel.rs` (split file). Confirm the wire-up site is the split file, not the basename file, before bucket execution.
- The config-side accessor at `emAutoplay.rs:129` is noted as a different object from the view-model. Confirm no cross-wiring is needed between the config accessor and the view-model accessors.
