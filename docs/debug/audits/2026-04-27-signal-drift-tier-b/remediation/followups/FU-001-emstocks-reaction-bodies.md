# FU-001 — emstocks reaction-body completion + emFileModel state-signal lift

**Pattern:** reaction-body stubs + accessor-lift (post-D-006 wiring).
**Scope:** `emstocks` (consumers), `emcore` (one accessor lift, one accessor add).
**Row count:** 5 reaction stubs + 1 upstream lift + 1 emcore accessor.
**Prereq buckets:** none. All Tier-B subscribe wiring is in place.

## Pattern description

Tier-B B-001-followup completed signal subscribes for emstocks ControlPanel / ItemPanel / ItemChart, but five reaction bodies remained as stubs with `TODO(B-001-followup)` markers because their port-side dependencies were absent. Two upstream gaps in `emcore` block the same surface: `emCheckBox` lacks the inherited `GetClickSignal()` accessor that emstocks ControlPanel needs, and `emRecFileModel::GetFileStateSignal` has not been lifted to `emFileModel` (so `emStocksFileModel::GetFileStateSignal()` returns null).

## Items

| ID | Site | Type | Notes / Unblocks |
|---|---|---|---|
| FU-001-1 | `crates/emcore/src/emCheckBox.rs` | Accessor add | `pub fn GetClickSignal(&self) -> SignalId` — mirrors `emButton::GetClickSignal()`. C++ precedent in `emCheckBox.h`. |
| FU-001-2 | `crates/emstocks/src/emStocksControlPanel.rs` row -566 | Reaction body | Subscribe to `emCheckBox` click via FU-001-1; reaction reads checkbox state and updates `emStocksConfig`. C++ at `emStocksControlPanel.cpp:566`. |
| FU-001-3 | `crates/emstocks/src/emStocksListBox.rs` | Method port | Port `emStocksListBox::StartToFetchSharePrices(stockId)` from C++. |
| FU-001-4 | `crates/emstocks/src/emStocksControlPanel.rs` row -586 | Reaction body | Calls FU-001-3 on click. C++ at `emStocksControlPanel.cpp:586`. |
| FU-001-5 | `crates/emstocks/src/emStocksListBox.rs` | Method port | Port `ShowWebPage(stockId)` and `ShowAllWebPages()` from C++. |
| FU-001-6 | `crates/emstocks/src/emStocksItemPanel.rs` | Reaction bodies (×3) | FetchSharePrice / ShowWebPage / ShowAllWebPages. Each calls into FU-001-3 / FU-001-5. |
| FU-001-7 | `crates/emcore/src/emFileModel.rs` + `emRecFileModel.rs` | Accessor lift | Lift `GetFileStateSignal()` from `emRecFileModel` to `emFileModel`. Update `emStocksFileModel.rs:149` UPSTREAM-GAP delegate to a real accessor. Removes B-017 row 1's null-signal residual. |

## Acceptance

- All 5 `TODO(B-001-followup)` markers in `emStocksControlPanel.rs` and `emStocksItemPanel.rs` removed.
- 3 `UPSTREAM-GAP` markers in `emStocksFileModel.rs` and `emStocksPricesFetcher.rs` removed (FU-001-7).
- Reaction bodies match C++ behavior at the cited line numbers.
- `cargo-nextest ntr` green; `cargo xtask annotations` clean.

## Notes

- FU-001-1 may grow if other emCheckBox consumers surface during execution — keep accessor minimal (1 line); do not add reactive-state APIs.
- FU-001-7 changes the `emFileModel` public surface in emcore. Audit consumers before lifting; plan needs verification step.
- Single bucket recommended (cohesive scope, shared C++ files); do not split per row.
