# Pattern Catalog

Drift shapes derived from the Tier-B audit's 178 actionable rows + 9 cleanup items. Each pattern is a `(evidence-kind, accessor-status)` cell or merged group; pattern-ids are the primary clustering axis for buckets.

Cell histogram (verdict × evidence-kind × accessor-status):

| n | verdict | evidence | accessor | → pattern |
|---|---|---|---|---|
| 72 | drifted | absent | missing | P-001 |
| 10 | gap-blocked | absent | missing | P-001 (gap-tagged) |
| 27 | drifted | absent | present | P-002 |
| 3 | gap-blocked | absent | present | P-002 (gap-tagged) |
| 7 | drifted | absent | type-mismatch | P-003 |
| 4 | drifted | polling | type-mismatch | P-003 |
| 3 | gap-blocked | absent | type-mismatch | P-003 (gap-tagged) |
| 29 | drifted | rc_cell_shim | present | P-004 |
| 6 | drifted | rc_cell_shim | missing | P-005 |
| 10 | drifted | polling | present | P-006 |
| 6 | drifted | polling | missing | P-007 |
| 1 | drifted | connect_call | present | P-008 |

Cleanup items (preexisting-diverged.csv): P-009.

Total = 178 actionable + 9 cleanup = 187 rows.

---

## P-001-no-subscribe-no-accessor

**Shape:** Rust path neither subscribes nor exposes the C++-side signal accessor. Both ends of the wire are missing.
**Evidence-kind:** absent
**Accessor-status:** missing
**Sample rows:** see `inventory-enriched.json` filter.
**Mechanical-vs-judgement:** balanced — wiring is mechanical once the accessor shape is decided; the accessor shape is a per-scope judgement call.
**Row count:** 83 (73 drifted + 10 gap-blocked). +1 from B-014 reclassification of emVirtualCosmos-575.

## P-002-no-subscribe-accessor-present

**Shape:** Accessor exists in Rust; consumer omits the subscribe call. One-sided wire.
**Evidence-kind:** absent
**Accessor-status:** present
**Mechanical-vs-judgement:** mechanical-heavy — the accessor is ready, just connect.
**Row count:** 30 (27 drifted + 3 gap-blocked).

## P-003-typemismatch-blocks-subscribe

**Shape:** Accessor exists but returns `u64` where `SignalId` is expected, blocking idiomatic subscribe. All emfileman; 4 distinct accessors (`GetSelectionSignal`, `GetChangeSignal`, `GetCommandsSignal`, fileman `ViewConfig::GetChangeSignal`).
**Evidence-kind:** absent or polling
**Accessor-status:** type-mismatch
**Mechanical-vs-judgement:** judgement-heavy at the accessor (decide D-001), then mechanical at consumers.
**Row count:** 14 (11 drifted + 3 gap-blocked).
**Cited decision:** D-001-typemismatch-accessor-policy.
**Status (post-B-009 merge 50994e26):** closed. B-009 flipped 3 of the 4 distinct accessors (`GetSelectionSignal`, `GetCommandsSignal`, `GetChangeSignal` on `emFileManViewConfig`) and migrated 11 consumers. The 4th nominal accessor (`emFileManViewConfig::GetChangeSignal` listed twice in the cluster row) is the same accessor; no separate flip needed.

## P-004-rc-shim-instead-of-signal

**Shape:** Accessor present but consumer routes around the signal via `Rc<RefCell<>>` / `Rc<Cell<>>` shared state in click-handler closures. Common in emCoreConfigPanel button handlers and emAutoplay flag-passing.
**Evidence-kind:** rc_cell_shim
**Accessor-status:** present
**Mechanical-vs-judgement:** judgement-heavy — closure-share is sometimes load-bearing for cross-panel coordination; per-row triage required.
**Row count:** 29 (15 emcore + 16 emmain + 4 emstocks; emstocks rows are dialog-result Cells specifically).
**Cited decision:** D-002-rc-shim-policy.

## P-005-rc-shim-no-accessor

**Shape:** Same rc-shim consumer pattern as P-004, but the upstream signal accessor is also missing.
**Evidence-kind:** rc_cell_shim
**Accessor-status:** missing
**Mechanical-vs-judgement:** judgement-heavy — needs accessor design plus shim removal.
**Row count:** 1 (was 6 originally; B-013 moved 4 emstocks rows P-005 → P-004; B-014 moved emVirtualCosmos-575 P-005 → P-001).
**Cited decision:** D-002-rc-shim-policy.

## P-006-polling-accessor-present

**Shape:** Consumer polls cached state per-frame instead of subscribing to an existing accessor (e.g., `emColorField::Cycle` field-comparison polling).
**Evidence-kind:** polling
**Accessor-status:** present
**Mechanical-vs-judgement:** mechanical-heavy if D-005 picks direct subscribe.
**Row count:** 10.
**Cited decision:** D-005-poll-replacement-shape.

## P-007-polling-accessor-missing

**Shape:** Polling consumer plus missing accessor.
**Evidence-kind:** polling
**Accessor-status:** missing
**Mechanical-vs-judgement:** balanced.
**Row count:** 6.
**Cited decision:** D-005-poll-replacement-shape.

## P-008-connect-with-poll-fallback **— RETIRED 2026-04-27**

**Shape:** Outlier. emFileDialog-196 has a `scheduler.connect(...)` call but a nearby `IsSignaled(...)` poll site causes the verdict to flip to drifted.
**Evidence-kind:** connect_call
**Accessor-status:** present
**Mechanical-vs-judgement:** judgement-heavy — single-row diagnosis.
**Row count:** 0 (was 1; B-018 brainstorm `04059bac` reclassified emFileDialog-196 drifted → faithful as verified observable equivalence to C++).

**Retirement reason:** P-008 was a category error in the audit's classification scheme. `AddWakeUpSignal + IsSignaled-in-Cycle` is the canonical emEngine subscription pattern (subscription arming + wakeup-cause check inside Cycle), not "hybrid drift." The audit's framing — "either the connect is redundant or the poll is redundant" — assumed `IsSignaled` was a polling-style "is this signal currently set" check, when it's actually a wakeup-cause probe that depends on the connect having woken the engine. The Rust rs:169/516/733 trio mirrors C++ cpp:90/196 exactly (rs:516/733 split is idiom adaptation for Rust's outer-engine lifecycle, mutually exclusive per dialog spawn). Pattern retired; entry preserved for audit trail.

## P-009-stale-annotation

**Shape:** Pre-existing `DIVERGED:` annotation re-validation failed against the four-question test (8 entries) or category was wrong (1 entry: emFileModel.rs:490 → `language-forced`). Cleanup is annotation removal/correction plus, in some cases, the underlying drift-fix that the annotation was masking.
**Source:** `preexisting-diverged.csv` filter `signal_related == 'true' AND (revalidation_result != 'verified' OR corrected_category != '')`.
**Mechanical-vs-judgement:** mechanical-heavy for annotation removal; underlying drift-fix joins another pattern bucket where applicable.
**Row count:** 9.
