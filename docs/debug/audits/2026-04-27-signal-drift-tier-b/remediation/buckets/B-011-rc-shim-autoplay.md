# B-011-rc-shim-autoplay — P-004 — convert AutoplayFlags rc-shim to signal subscribe

**Pattern:** P-004-rc-shim-instead-of-signal
**Scope:** emmain:emAutoplay
**Row count:** 7
**Mechanical-vs-judgement:** judgement-heavy
**Cited decisions:** D-002-rc-shim-policy §1 R-A (drop AutoplayFlags entirely; ratified during B-003 reconciliation).
**Prereq buckets:** B-003-no-wire-autoplay (hard, all 7 rows — B-003's R-A removes the AutoplayFlags struct, which removes these 7 closures by construction).

**Reconciliation amendments (2026-04-27, post-design cf9e1cc4):**
- **All 7 rows are the same closures B-003's R-A removes.** Implementation merges with B-003's PR; B-011 carries no independent wiring work. Status flips designed → merged jointly with B-003.
- **Hard prereq edge encoded:** all 7 rows in `inventory-enriched.json` have `prereq_ids: ['emAutoplay-1171']` (B-003's `emAutoplayControlPanel::Cycle` wiring row centralizes the fix).
- **Audit-data correction (split-file line drift):** all 7 rows had `rust_file = crates/emmain/src/emAutoplay.rs` but the actual code lives in `crates/emmain/src/emAutoplayControlPanel.rs` at the same line numbers (B-003 anomaly §2 again). Inventory amended in this reconciliation.
- **"Accessor present" interpretation note:** the audit tag is technically correct but refers to widget-side accessors (`emCheckButton::check_signal`, `emButton::click_signal`, `emScalarField::value_signal`), not model-side. No reclassification; flagged for future audit-heuristic tightening.
- **No new D-### entries; no new global decisions.**

## Pattern description

Accessor present but the consumer routes around the signal via `Rc<RefCell<>>` / `Rc<Cell<>>` shared state captured in click-handler closures, hiding the signal from any downstream observer. Per D-002 the default is to convert each row to a signal subscribe, with per-row escalation when no C++ analogue exists. In this bucket every row sits inside `emAutoplay`, a Rust-only panel whose `AutoplayFlags { progress: Rc<Cell<f64>> }` shim has no direct C++ counterpart, so D-002's triage rule has to be adapted before mechanical conversion can proceed.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emAutoplay-1255 | src/emMain/emAutoplay.cpp:1255 | crates/emmain/src/emAutoplay.rs:580 | present | check_signal SignalId fired (emCheckButton.rs:70) but consumer uses on_check closure not connect()+IsSignaled |
| emAutoplay-1270 | src/emMain/emAutoplay.cpp:1270 | crates/emmain/src/emAutoplay.rs:487 | present | rc_cell_shim on widget-click |
| emAutoplay-1282 | src/emMain/emAutoplay.cpp:1282 | crates/emmain/src/emAutoplay.rs:499 | present | rc_cell_shim on widget-click |
| emAutoplay-1294 | src/emMain/emAutoplay.cpp:1294 | crates/emmain/src/emAutoplay.rs:627 | present | rc_cell_shim on widget-click |
| emAutoplay-1314 | src/emMain/emAutoplay.cpp:1314 | crates/emmain/src/emAutoplay.rs:321 | present | C++ widget-check vocabulary nearest fit; emScalarField value-changed structurally analogous |
| emAutoplay-1321 | src/emMain/emAutoplay.cpp:1321 | crates/emmain/src/emAutoplay.rs:347 | present | rc_cell_shim on widget-check |
| emAutoplay-1329 | src/emMain/emAutoplay.cpp:1329 | crates/emmain/src/emAutoplay.rs:378 | present | rc_cell_shim on widget-check |

## C++ reference sites

- src/emMain/emAutoplay.cpp:1255
- src/emMain/emAutoplay.cpp:1270
- src/emMain/emAutoplay.cpp:1282
- src/emMain/emAutoplay.cpp:1294
- src/emMain/emAutoplay.cpp:1314
- src/emMain/emAutoplay.cpp:1321
- src/emMain/emAutoplay.cpp:1329

## Open questions for the bucket-design brainstorm

- D-002 explicitly defers the `AutoplayFlags { progress: Rc<Cell<f64>> }` flags-passing pattern: does it fall under rule 1 (convert — C++ original uses signal accessor + subscribe) or rule 2 (keep — member field assigned post-finish/post-cycle)? emAutoplay has no C++ analogue so the rule needs adaptation; this must be resolved by the working-memory session before bucket execution.
- Because emAutoplay is Rust-only, the cited C++ line numbers (`emAutoplay.cpp:1255–1329`) are approximations or absent — confirm during brainstorm whether any neighbouring C++ panel (e.g. the original autoplay screensaver code) is the right fidelity reference, and whether the four forced-divergence categories permit the shim at all.
- For the `check_signal`/`on_check` row (emAutoplay-1255), is the right shape per-widget `connect(check_signal)` mirroring emCheckButton's signal, or a single AutoplayFlags-level signal that all four flag widgets fire into?
- For the emScalarField row (emAutoplay-1314) tagged with the "widget-check vocabulary nearest fit" note, confirm the value-changed signal is the structural analogue intended by D-002 rule 1 before mechanical conversion.
- If escalation concludes the shim is load-bearing (rule 2 / Rust-only design intent), how is that recorded — `RUST_ONLY:` with which forced category, or a new annotation describing the no-C++-analogue carve-out?
