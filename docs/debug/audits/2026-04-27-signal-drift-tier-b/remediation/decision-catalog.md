# Decision Catalog

Cross-cutting design decisions referenced by enriched rows. Each `D-###` is resolved at sketch resolution in `decisions.md` (Phase 2). This catalog is the input list — `decisions.md` is the resolved spine.

---

## D-001-typemismatch-accessor-policy

**Question:** For accessors returning `u64` where `SignalId` is expected (all in emfileman: `GetSelectionSignal`, `GetCommandsSignal`, two flavors of `GetChangeSignal`), do we flip the accessor type or adapt consumers?

**Affected rows:** P-003 (14 rows) + cleanup item `emFileModel.rs:490` (corrected to `language-forced`).

## D-002-rc-shim-policy

**Question:** For consumers using `Rc<RefCell<>>` / `Rc<Cell<>>` shared state in click-handler closures instead of subscribing to a signal, is conversion to signal-subscribe always correct, or are there load-bearing cases (cross-panel coordination, dialog-result handoff) that justify keeping the shim?

**Affected rows:** P-004 (29) + P-005 (6) = 35 rows. Concentrated in emcore (emCoreConfigPanel), emmain (emAutoplay), emstocks (dialog-result Cells).

## D-003-gap-blocked-fill-vs-stub

**Question:** For the 16 gap-blocked rows where the upstream-gap is the blocker, do we fill the gap (port the missing infrastructure) or stub at the consumer (no-op signal subscription that becomes live when the gap fills)?

**Affected rows:** all gap-blocked rows across P-001/P-002/P-003 (10 + 3 + 3 = 16).

## D-004-stocks-application-strategy

**Question:** ~78 emstocks rows likely repeat the same drift across panels (emStocksControlPanel: 37, emStocksItemPanel: 25, etc.). Do we design the fix once and apply mechanically across all stocks panels (single design, batch PR), or design per-panel?

**Affected rows:** all 78 emstocks rows.

## D-005-poll-replacement-shape

**Question:** When replacing a polling consumer with a subscribe, do we go direct (subscribe → react in callback) or use a subscribe-plus-idempotent-re-poll wrapper (subscribe → mark dirty → re-poll on next tick)? The shape choice affects bucket implementation cost across all polling rows.

**Affected rows:** P-006 (10) + P-007 (6) + the 4 polling rows in P-003 (subset) = ~20 rows.
