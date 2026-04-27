# B-013-dialog-cells-emstocks — P-005 — emstocks dialog-result Cells (D-002 keep-shim case)

**Pattern:** P-005-rc-shim-no-accessor
**Scope:** emstocks
**Row count:** 4
**Mechanical-vs-judgement:** judgement-heavy — per pattern catalog P-005, needs accessor design plus shim removal; here the judgement collapses to "confirm keep-shim" per D-002 rule 2.
**Cited decisions:** D-002-rc-shim-policy (governs convert-vs-keep triage; rule 2 puts these dialog-result Cells in the keep-shim column), D-004-stocks-application-strategy (confirms one-bucket-per-pattern emstocks slice; mechanical application across the 4 rows is the intent)
**Prereq buckets:** none

## Pattern description

Consumer routes around the upstream signal via `Rc<Cell<Option<DialogResult>>>` shared state populated by a one-shot dialog `set_on_finish` closure and observed in `Cycle()`; the upstream signal accessor is also missing on the dialog type. Per D-002, the dialog-result Cells in emstocks (`cut_stocks_result`, `paste_stocks_result`, `delete_stocks_result`, and the interest-change variant) match rule 2 — the C++ original uses a member field assigned post-finish after `IsFinished()`, so the shim *is* the contract rather than a signal-shim. Expected outcome for this bucket is therefore "annotate as DIVERGED with a load-bearing-shim rationale and keep the code," not a conversion to signal-subscribe.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emStocksListBox-189 | src/emStocks/emStocksListBox.cpp:189 | crates/emstocks/src/emStocksListBox.rs:54 | missing | Cut-confirmation dialog finish via Rc<Cell<Option<DialogResult>>> shim; observed in Cycle |
| emStocksListBox-287 | src/emStocks/emStocksListBox.cpp:287 | crates/emstocks/src/emStocksListBox.rs:55 | missing | Paste-confirmation dialog finish: rc_cell_shim. Same pattern as Cut/Delete/Interest |
| emStocksListBox-356 | src/emStocks/emStocksListBox.cpp:356 | crates/emstocks/src/emStocksListBox.rs:56 | missing | Delete-confirmation dialog finish: rc_cell_shim |
| emStocksListBox-443 | src/emStocks/emStocksListBox.cpp:443 | crates/emstocks/src/emStocksListBox.rs:57 | missing | Interest-change confirmation dialog finish: rc_cell_shim |

## C++ reference sites

- src/emStocks/emStocksListBox.cpp:189
- src/emStocks/emStocksListBox.cpp:287
- src/emStocks/emStocksListBox.cpp:356
- src/emStocks/emStocksListBox.cpp:443

## Open questions for the bucket-design brainstorm

- Confirm per D-002 rule 2 that each of the 4 rows' C++ original truly uses a post-finish member-field read (not a signal accessor + subscribe); spot-check the cited cpp:line sites before committing to keep-shim.
- Decide the exact `DIVERGED:` annotation category and citation text for the keep-shim outcome (language-forced vs preserved-design-intent framing) — the rc-shim shape mirrors C++ post-finish member-field semantics, so the annotation must explain why the shim *is* the C++ contract here.
- Whether all 4 dialog-result Cells share a single annotation block or each row carries its own — file is the same (`emStocksListBox.rs`), lines are adjacent (54-57), so a single block citing all four sites is plausible.
- Per D-004 operational consequence: confirm "mechanical application across all in-bucket rows" is the intent (i.e., the 4 rows get the identical keep-shim treatment, no per-row redesign).
- Whether the keep-shim outcome here sets precedent the working-memory session needs to back-propagate to the analogous emAutoplay flags-passing pattern flagged in D-002's open questions.

