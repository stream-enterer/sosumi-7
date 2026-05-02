# FU-001 — emstocks reaction-body completion + emCheckBox click_signal mirror

**Bucket:** [FU-001](../../debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-001-emstocks-reaction-bodies.md)
**Date:** 2026-05-02
**Scope:** `emcore` (one widget), `emstocks` (ListBox, FetchPricesDialog, PricesFetcher, ControlPanel, ItemPanel).
**Prereqs:** none — Tier-B subscribe wiring is already in place.

## Summary

Tier-B B-001-followup completed signal subscribes for emstocks ControlPanel / ItemPanel / ItemChart but left five reaction bodies as `TODO` stubs. Three causes: (1) `emCheckBox` is missing the `click_signal` mirror field that `emCheckButton` already has (B-012 established the mirror-sibling-port convention but missed the leaf widget), (2) `emStocksListBox` lacks two C++ methods (`StartToFetchSharePrices` overloads, `ShowWebPages`), (3) `emStocksFetchPricesDialog` and `emStocksPricesFetcher` lack `AddListBox`. This bucket closes all three.

The originally-listed `GetFileStateSignal` lift (FU-001-7) is split out as FU-005 — the issue is a signal-conflation bug in `emFileModel<T>` requiring its own brainstorm and consumer audit.

## Design intent (per Port Ideology)

The Rust port has codified a **mirror-sibling-port pattern** for C++ public inheritance: where C++ uses inheritance to provide an accessor on a derived class, Rust mirrors the field on each sibling struct with a `DIVERGED: (language-forced)` annotation. This was deliberated and applied during B-012 to `emCheckButton::click_signal`. FU-001 extends the same pattern to `emCheckBox::click_signal` — not a redesign, a missed application.

A composition-based alternative (`emCheckBox` embeds `emCheckButton`) was considered and rejected: it would override B-012's deliberate convention, expand the public API of `emCheckButton` (whose constructor hardcodes its border type), and risk widget-state golden regression. Revisiting mirror-vs-embed at the widget-hierarchy level is a separate brainstorm if pursued.

## Work units

Four units, one commit per unit:
- **Unit 1** — emcore widget mirror (independent).
- **Unit 2** — emstocks fetcher/dialog `AddListBox` ports (depends on nothing in this bucket).
- **Unit 3** — emstocks ListBox method ports (depends on Unit 2 — `AddListBox` is called from `StartToFetchSharePrices`).
- **Unit 4** — reaction-body completion (depends on Unit 1 for the click_signal swap, on Unit 3 for the ListBox calls).

### Unit 1 — `emCheckBox::click_signal` mirror (emcore)

**Files:** `crates/emcore/src/emCheckBox.rs`.

**Changes:**

- Add `pub click_signal: SignalId` field with `DIVERGED: (language-forced)` annotation citing the established mirror-sibling-port pattern (cite emCheckButton.rs:32-40).
- Allocate via `ctx.create_signal()` in `new`.
- Fire from the user-toggle path in `Input` (where `check_signal` is fired today). **Do NOT fire from `SetChecked`** — preserves B-012's feedback-loop guard.
- Add `pub fn GetClickSignal(&self) -> SignalId` and `pub fn GetCheckSignal(&self) -> SignalId` accessors mirroring C++ surface. (Consistent with the existing accessor pattern; consumers may also use field access.)

**Tests:** two unit tests in emCheckBox module:
- User-toggle (Input path): both `click_signal` and `check_signal` fire.
- Programmatic `SetChecked`: only `check_signal` fires; `click_signal` does NOT fire.

Existing `widget_checkbox_*` compositor goldens are paint-only and remain valid (paint algorithm unchanged). `widget_checkbox_toggle.widget_state.golden` is reviewed for impact; if it captured the missing-signal state, regenerate with a noted delta in the commit message.

### Unit 2 — Fetcher + Dialog `AddListBox` ports (emstocks)

**Files:** `crates/emstocks/src/emStocksPricesFetcher.rs`, `crates/emstocks/src/emStocksFetchPricesDialog.rs`.

**Changes:**

- Port `emStocksPricesFetcher::AddListBox(&mut self, ...)` from C++ (`emStocksPricesFetcher.cpp:48`). Body matches C++ exactly.
- Port `emStocksFetchPricesDialog::AddListBox(&mut self, ...)` from C++ (header inline at `emStocksFetchPricesDialog.h`). Body delegates to `Fetcher.AddListBox(...)`.
- Method signatures take whatever ListBox handle the Rust port already uses (likely `&emStocksListBox` or `Rc<RefCell<emStocksListBox>>`); resolve in implementation.
- D-007 ectx threading: only required if these methods fire signals. Verify against C++; thread `&mut impl SignalCtx` only where C++ fires.

**Tests:** unit tests asserting AddListBox propagates to the fetcher's tracked-listboxes collection.

### Unit 3 — `emStocksListBox` method ports (emstocks)

**Files:** `crates/emstocks/src/emStocksListBox.rs`.

**Changes — three new methods, mirroring C++ exactly:**

1. `pub fn StartToFetchAllSharePrices(&mut self, ectx: &mut impl SignalCtx)` — zero-arg-equivalent. Iterates `GetItemCount()` items, collects each item's stock ID via `GetStockByItemIndex`, forwards to the array overload. C++ at `emStocksListBox.cpp:371-383` (`StartToFetchSharePrices()` in C++).

2. `pub fn StartToFetchSharePrices(&mut self, ectx: &mut impl SignalCtx, stock_ids: &[String])` — array overload. Mirrors C++ at `emStocksListBox.cpp:386-...`:
   - If `self.file_model.borrow().PricesFetchingDialog.is_valid()` → **flagged TODO `Raise()`** (depends on view-parenting upstream gap; emit log line for now).
   - Else → construct via `emStocksFetchPricesDialog::new_with_model(api_script, api_interpreter, api_key, file_model_rc)` and store in `self.file_model.borrow_mut().PricesFetchingDialog`.
   - Compute `date = file_model.GetLatestPricesDate(); if date.is_empty() { date = GetCurrentDate(); }`.
   - Call `self.SetSelectedDate(ectx, &date)`.
   - Call `dialog.AddListBox(self)` and `dialog.AddStockIds(ectx, stock_ids)`.

   **Method overload resolution.** Rust has no function overloading. Convention in this design: keep `StartToFetchSharePrices` as the array-overload name (matches the most-used C++ signature); name the zero-arg variant `StartToFetchAllSharePrices` (descriptive, no naming collision). Both methods carry a `// C++ overload of StartToFetchSharePrices` comment with the C++ line cite.

3. `pub fn ShowWebPages(&self, web_pages: &[String])` — `&self` (C++ `const`).
   - Read `self.config.borrow().web_browser`.
   - If empty: log error to stderr with `// TODO: emDialog::ShowMessage when ported`. Return.
   - Build `args = [web_browser, ...web_pages]`. Call `emProcess::TryStartUnmanaged(&args)`.
   - On error: log to stderr with same TODO. Return.

**Tests:** unit tests for each method:
- Zero-arg StartToFetchSharePrices iterates and forwards.
- Array StartToFetchSharePrices: dialog-construction-on-empty path; dialog-already-exists path (TODO Raise stub).
- ShowWebPages: empty-config path; happy path with mockable process spawn.

### Unit 4 — Reaction-body completion (emstocks)

**Files:** `crates/emstocks/src/emStocksControlPanel.rs`, `crates/emstocks/src/emStocksItemPanel.rs`.

**Changes — five reaction sites:**

1. `emStocksControlPanel.rs` line ~835: replace
   ```rust
   ectx.connect(w.owned_shares_first.check_signal, eid);
   ```
   with
   ```rust
   ectx.connect(w.owned_shares_first.click_signal, eid);
   ```
   Remove the multi-line TODO comment (lines 829-834). Update the reaction body keyed off this signal to match C++ semantics (user-click only, no programmatic-SetChecked feedback).

2. `emStocksControlPanel.rs` line ~1045: replace
   ```rust
   let _ = fetch_fired; // TODO: wire to FetchPricesDialog when emStocksFilePanel surfaces it.
   ```
   with
   ```rust
   if fetch_fired {
       self.list_box.borrow_mut().StartToFetchAllSharePrices(ectx);
   }
   ```

3. `emStocksItemPanel.rs` line ~1022: replace
   ```rust
   let _ = fetch_share_price_fired; // TODO: wire to StartToFetchSharePrices when ListBox exposes it.
   ```
   with
   ```rust
   if fetch_share_price_fired {
       let id = stock.id.clone();
       self.list_box.borrow_mut().StartToFetchSharePrices(ectx, &[id]);
   }
   ```

4. `emStocksItemPanel.rs` line ~1023: replace
   ```rust
   let _ = show_all_fired; // TODO: wire to ShowWebPages when ListBox exposes it.
   ```
   with
   ```rust
   if show_all_fired {
       let urls: Vec<String> = stock
           .web_pages
           .iter()
           .filter(|s| !s.is_empty())
           .cloned()
           .collect();
       if !urls.is_empty() {
           self.list_box.borrow().ShowWebPages(&urls);
       }
   }
   ```

5. `emStocksItemPanel.rs` line ~1025 (loop body): replace
   ```rust
   let _ = fired; // TODO: wire to ShowWebPages.
   ```
   with
   ```rust
   if fired && !stock.web_pages[i].is_empty() {
       self.list_box.borrow().ShowWebPages(&[stock.web_pages[i].clone()]);
   }
   ```

**Tests:** integration tests via existing emStocks test patterns where coverage exists; otherwise unit-level reaction tests asserting the correct ListBox method was called.

## Out of scope (flagged, deferred)

- `emStocksFetchPricesDialog::Raise()` — depends on view-parenting (Rust dialog ctor doesn't take a view; existing UPSTREAM-GAP).
- `emDialog::ShowMessage` for `ShowWebPages` error path — separate emcore port.
- emCheckBox embed-not-mirror redesign — separate widget-hierarchy brainstorm if revisited.
- `GetFileStateSignal` conflation fix — split out as FU-005.

## Phase ordering

1. **Phase 1** — Unit 1 (emCheckBox click_signal mirror). emcore-isolated.
2. **Phase 2** — Unit 2 (Fetcher + Dialog AddListBox ports).
3. **Phase 3** — Unit 3 (ListBox method ports).
4. **Phase 4** — Unit 4 (reaction-body completion + ControlPanel signal swap). Removes all 5 TODO markers.
5. **Phase 5** — Reconciliation: update FU-001 bucket file with closure section; verify `cargo xtask annotations` clean; full `cargo-nextest ntr`.

Each phase is a single commit. Phase 1 is independent of 2-5; 2 must precede 3; 3 must precede 4.

## Acceptance criteria

- All 5 `TODO(B-001-followup)` / equivalent markers in `emStocksControlPanel.rs` and `emStocksItemPanel.rs` removed.
- `emCheckBox` exposes `click_signal` field + `GetClickSignal()` accessor; B-012 fire-rules preserved (user-toggle fires both, `SetChecked` fires only `check_signal`).
- `emStocksListBox` exposes `StartToFetchSharePrices` (array), `StartToFetchAllSharePrices` (zero-arg equivalent), and `ShowWebPages`.
- `emStocksFetchPricesDialog` and `emStocksPricesFetcher` expose `AddListBox`.
- All reaction bodies match C++ behavior at cited line numbers.
- `cargo-nextest ntr` green; `cargo clippy -D warnings` green; `cargo xtask annotations` clean.
- `widget_checkbox_*` goldens unchanged or regenerated with reviewed delta + commit-message rationale.

## References

- C++ source: `~/Projects/eaglemode-0.96.4/`:
  - `src/emStocks/emStocksListBox.cpp:371-501` (StartToFetchSharePrices, ShowWebPages, DeleteSharePrices).
  - `src/emStocks/emStocksControlPanel.cpp:566,586,650+` (reactions).
  - `src/emStocks/emStocksItemPanel.cpp` (FetchSharePrice, ShowWebPage[i], ShowAllWebPages reactions).
  - `src/emStocks/emStocksPricesFetcher.cpp:48` (AddListBox).
  - `include/emStocks/emStocksFetchPricesDialog.h` (AddListBox inline).
- Rust precedent: `crates/emcore/src/emCheckButton.rs:32-40` (click_signal mirror pattern, B-012 codification).
- Existing FU-001 bucket: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-001-emstocks-reaction-bodies.md`.
