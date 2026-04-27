# B-011-rc-shim-autoplay — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm) — **deferred-to-B-003**
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-011-rc-shim-autoplay.md`
**Pattern:** P-004-rc-shim-instead-of-signal
**Scope:** `emmain:emAutoplay`, 7 rows
**Cited decisions:** D-002 §1 R-A (ratified during B-003 reconciliation, see `decisions.md` D-002).
**Prereq buckets:** **B-003 (hard).** B-011 closes when B-003 merges; this bucket adds no implementation work beyond what B-003 already specifies.
**New global decisions:** none.

---

## 0. Bucket overview

All 7 rows in this bucket are the seven widget callback closures (`on_check` / `on_click` / `on_value`) on `emAutoplayControlPanel` that today write into `AutoplayFlags`. The corresponding C++ `AddWakeUpSignal(BtX->GetXxxSignal())` calls in `emAutoplayControlPanel`'s constructor (cpp:1255–1329) are the widget fan-out that **B-003 § "Per-panel consumer wiring" already designs in full** under R-A: drop `AutoplayFlags`, hold widget instances as fields, subscribe each widget signal in first-Cycle init, react in `IsSignaled` branches mirroring `emAutoplay.cpp:1184–1218`.

Removing `AutoplayFlags` per R-A removes these 7 closures by construction. **There is no independent fix work for B-011.** This design doc records the per-row triage so the bucket has a citation target and the inventory can carry the prereq edges; the implementer does the work as part of executing B-003's plan.

---

## 1. Per-row triage table

All 7 rows: **D-002 §1 R-A apply** — drop the `AutoplayFlags` shim; widget signal subscribe + `IsSignaled` reaction in `Cycle()` per B-003. Forced-category: none (R-A is the convert path, not a divergence).

| Row | C++ site | Rust site (today) | Widget | C++ signal | Reaction in B-003 §Cycle |
|---|---|---|---|---|---|
| emAutoplay-1255 | `emAutoplay.cpp:1255` | `emAutoplayControlPanel.rs:580` (`btn_autoplay.on_check`) | `BtAutoplay` (`emCheckButton`) | `GetCheckSignal()` | `Model.SetAutoplaying(ectx, checked)` |
| emAutoplay-1270 | `emAutoplay.cpp:1270` | `emAutoplayControlPanel.rs:487` (`btn_prev.on_click`) | `BtPrev` (`emButton`) | `GetClickSignal()` | `Model.SkipToPreviousItem(ectx)` |
| emAutoplay-1282 | `emAutoplay.cpp:1282` | `emAutoplayControlPanel.rs:499` (`btn_next.on_click`) | `BtNext` (`emButton`) | `GetClickSignal()` | `Model.SkipToNextItem(ectx)` |
| emAutoplay-1294 | `emAutoplay.cpp:1294` | `emAutoplayControlPanel.rs:627` (`btn_cont.on_click`) | `BtContinueLast` (`emButton`) | `GetClickSignal()` | `Model.ContinueLastAutoplay(ectx)` |
| emAutoplay-1314 | `emAutoplay.cpp:1314` | `emAutoplayControlPanel.rs:321` (`sf.on_value`) | `SfDuration` (`emScalarField`) | `GetValueSignal()` | `Model.SetDurationMS(ectx, DurationValueToMS(val))` |
| emAutoplay-1321 | `emAutoplay.cpp:1321` | `emAutoplayControlPanel.rs:347` (`cb_recursive.on_check`) | `CbRecursive` (`emCheckBox`) | `GetCheckSignal()` | `Model.SetRecursive(ectx, checked)` |
| emAutoplay-1329 | `emAutoplay.cpp:1329` | `emAutoplayControlPanel.rs:378` (`cb_loop.on_check`) | `CbLoop` (`emCheckBox`) | `GetCheckSignal()` | `Model.SetLoop(ectx, checked)` |

Each Rust line in column 3 is the closure that **disappears** when `AutoplayFlags` is dropped. Each C++ signal in column 5 is one of the entries in B-003's `WidgetSignalIds` struct (`bt_autoplay_check`, `bt_prev_click`, `bt_next_click`, `bt_continue_last_click`, `sf_duration_value`, `cb_recursive_check`, `cb_loop_check`). The reactions in column 6 reproduce B-003 § "Per-panel consumer wiring," which itself mirrors `emAutoplay.cpp:1184–1218` source order.

**Disposition:** all 7 rows = **R-A apply**. No row needs rule-1/rule-2 escalation; no row is half-convert; no row reclassifies to a different pattern.

---

## 2. Audit-data corrections (for reconciliation)

1. **Split-file line drift.** The 7 rows tag `rust_file = "emAutoplay.rs"` with the line numbers in column 3 above. The actual code lives in `emAutoplayControlPanel.rs` at the same line numbers (C++ original is one file; Rust port splits into `emAutoplay.rs` + `emAutoplayControlPanel.rs` — cf. SPLIT marker discipline). Reconciler should patch `inventory-enriched.json` `rust_file` to `crates/emmain/src/emAutoplayControlPanel.rs` for all 7 rows. Same anomaly B-003 hit (anomaly §2 in B-003 design).

2. **"Accessor present" interpretation.** The audit tagged all 7 rows `accessor_status = present`. This is technically correct but refers to **widget-side** accessors (`emCheckButton::check_signal`, `emButton::click_signal`, `emScalarField::value_signal`), not Model-side. P-004 entries elsewhere in the catalog typically mean Model-side accessor present; the heuristic doesn't distinguish. No reclassification needed — the rows really are P-004 (rc-shim around a present accessor) — but a future audit re-run should not re-flag these as P-002-shaped if it tightens the heuristic to require Model-side accessors.

3. **Bucket-file open questions §3 and §4** (`emAutoplay-1255` shape and `emAutoplay-1314` value-changed analogue) are answered in B-003 — the per-widget signal mapping in column 5 of the table above is the canonical answer; none of these rows aggregate into a single AutoplayFlags-level signal, and `SfDuration::GetValueSignal()` is the structural analogue per C++ cpp:1314.

---

## 3. Cross-bucket prereq edges

**Hard prereq:** B-003 → B-011. All 7 rows are fixed by B-003's R-A application. The reconciler should encode this in `inventory-enriched.json` by setting each B-011 row's `prereq_ids` to point at the B-003 row that centralizes the wiring — most natural target is `emAutoplay-1171` (B-003's consumer-side ControlPanel.Cycle row), since B-003 § "Per-panel consumer wiring" §emAutoplayControlPanel is exactly the design that subsumes these 7 rows. Uniform mapping: all 7 → `emAutoplay-1171`.

**No other prereq edges.** No new accessor work (R-A removes the need); no D-007 mutator-fire work in scope (the inbound widget→Model side uses `&mut Model` calls inside `IsSignaled` branches, threading ectx that the panel's `Cycle` already holds — already designed in B-003 § Per-panel consumer wiring).

---

## 4. Status flow

- B-011 status: `pending → designed (deferred-to-B-003)` upon merge of this design doc.
- B-011 status: `designed → merged` together with B-003's implementation PR. No separate B-011 PR.
- If the implementer of B-003 stages the AutoplayFlags removal as a follow-up (e.g., B-003 lands accessor adds + Cycle scaffolding first, AutoplayFlags removal second), B-011 follows the AutoplayFlags-removal stage, not the accessor stage.

---

## 5. Verification

No verification additional to B-003. B-003's `crates/emmain/tests/typed_subscribe_b003.rs` already exercises every widget signal end-to-end (see B-003 § Verification strategy: "Click signal: invoking widget click drives ViewModel" — that pattern repeats per widget). When B-003's tests pass, all 7 B-011 rows are fixed by construction.

---

## 6. Out of scope / not introduced

- No new global decisions (D-### catalog unchanged).
- No new accessor (R-A obviates).
- No half-convert framing (B-013 precedent does not apply here — no cell-as-internal-buffer; the `progress` cell B-013-style channel is an outbound concern handled separately by B-014's emAutoplay-1172 row, not by these inbound rows).
- No P-005 reclassification candidates surfaced (cf. B-013 P-005→P-004; B-014 P-005→P-001). All 7 rows are correctly P-004.

---

## 7. Reconciliation handoff

For the working-memory session reconciling this design back into the spine:

- Set `inventory-enriched.json` `prereq_ids` on each of the 7 B-011 rows → `["emAutoplay-1171"]` (uniform mapping).
- Patch `inventory-enriched.json` `rust_file` for the 7 rows → `crates/emmain/src/emAutoplayControlPanel.rs`.
- Add reconciliation log entry to `work-order.md`: B-011 marked `designed (deferred-to-B-003)`.
- No `decisions.md` change (D-002 already records R-A's ratification from B-003).
- No `pattern-catalog.md` change (no row count drift; no reclassification).
- B-011 bucket sketch: append a "designed deferred-to-B-003" header pointer to this design doc.
