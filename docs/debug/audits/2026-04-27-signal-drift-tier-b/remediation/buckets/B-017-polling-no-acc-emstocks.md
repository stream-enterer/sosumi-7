# B-017-polling-no-acc-emstocks — P-007 — emstocks polling consumers + missing accessors

**Pattern:** P-007-polling-accessor-missing
**Scope:** emstocks
**Row count:** 3
**Mechanical-vs-judgement:** balanced
**Cited decisions:** D-004-stocks-application-strategy (confirms design-once / apply-mechanically across the in-bucket emstocks rows), D-005-poll-replacement-shape (direct-subscribe shape governs the polling-to-subscribe rewrite for each row)

**Prereq buckets:** none

## Pattern description

Polling consumer paired with a missing accessor: the Rust site re-reads source state each tick (or compares before/after) without ever calling `IsSignaled` on a corresponding signal, and the producing model also never exposed/allocated the signal accessor. Fix requires both adding the missing accessor on the producer and rewriting the consumer per D-005 (direct subscribe). In this bucket all three sites live in emstocks across a fetcher dialog, the file panel, and the file model itself (where one row is upstream-gap-adjacent because `emTimer::TimerCentral` is unported).

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emStocksFetchPricesDialog-62 | src/emStocks/emStocksFetchPricesDialog.cpp:62 | crates/emstocks/src/emStocksFetchPricesDialog.rs:91 | missing | Cycle polls fetcher.HasFinished() and unconditionally calls UpdateControls; no IsSignaled / connect on ChangeSignal |
| emStocksFilePanel-34 | src/emStocks/emStocksFilePanel.cpp:34 | crates/emstocks/src/emStocksFilePanel.rs:354 | missing | Cycle polls vir-file-state via before/after compare; same drift as emFileLinkPanel (F010 root cause) |
| emStocksFileModel-41 | src/emStocks/emStocksFileModel.cpp:41 | crates/emstocks/src/emStocksFileModel.rs:62 | missing | Save timer simulated via Instant; emTimer::TimerCentral unported, no SignalId allocated |

## C++ reference sites

- src/emStocks/emStocksFetchPricesDialog.cpp:62
- src/emStocks/emStocksFileModel.cpp:41
- src/emStocks/emStocksFilePanel.cpp:34

## Open questions for the bucket-design brainstorm

- PR-staging: should one of the three rows (e.g., emStocksFilePanel-34, which mirrors the F010 root-cause shape) pilot the pattern fix and merge before the other two land? (D-004 deferred item.)
- For each consumer that polls multiple sources, confirm whether the C++ original subscribes individually or to an aggregated signal; default mirror C++. (D-005 deferred item.)
- emStocksFileModel-41 depends on `emTimer::TimerCentral`, which is unported — does this row require a TimerCentral port (or a chartered RUST_ONLY substitute) as a prereq, or can the save-timer signal be allocated independently of TimerCentral?
- For each missing accessor, confirm the producer-side allocation (SignalId on the model) lands in the same change as the consumer rewrite, vs. split into a separate prep commit.
- Confirm Phase-3 clustering did not split other emstocks P-007 rows into adjacent buckets that should be merged here.
