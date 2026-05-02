# FU-001 — emstocks reaction-body completion + emCheckBox click_signal mirror — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the five `TODO(B-001-followup)` reaction-body stubs in emStocks ControlPanel/ItemPanel by porting the missing `emStocksListBox` / `emStocksFetchPricesDialog` / `emStocksPricesFetcher` methods, and by extending the B-012 `click_signal` mirror-sibling-port pattern to `emCheckBox`.

**Architecture:** Four ordered work units, one commit each. Unit 1 adds `emCheckBox::click_signal` (mirror of `emCheckButton::click_signal`, B-012 pattern, language-forced). Unit 2 ports `AddListBox` on the fetcher + dialog. Unit 3 ports the three missing ListBox methods (`StartToFetchAllSharePrices`, `StartToFetchSharePrices(&[String])`, `ShowWebPages`). Unit 4 wires the five reaction bodies and swaps `owned_shares_first.check_signal` → `click_signal` in ControlPanel. A final reconciliation phase updates the bucket file and runs the full gate.

**Tech Stack:** Rust 2024 edition, `cargo`, `cargo-nextest`, `cargo xtask annotations`. Crates touched: `emcore` (one widget), `emstocks` (5 files). C++ reference at `~/Projects/eaglemode-0.96.4/`.

**Non-interactive defaults made by the planner** (no human-in-the-loop):
- Method overload naming: keep `StartToFetchSharePrices(ectx, &[String])` for the array overload (mirrors C++ primary signature) and name the zero-arg variant `StartToFetchAllSharePrices` (descriptive, no Rust collision). Both methods carry `// C++ overload of StartToFetchSharePrices` cite-comments per the spec.
- `Raise()` path: emit a single `eprintln!` with a `// TODO(FU-001):` marker (view-parenting is an existing UPSTREAM-GAP).
- `emDialog::ShowMessage` fallback: log to stderr with `// TODO(FU-001): replace with emDialog::ShowMessage when ported`.
- Test-style: use the existing `TestInit` / `PanelCtx::with_sched_reach` pattern from `emCheckButton.rs` and `emCheckBox.rs` for widget unit tests; for emstocks methods, use the same patterns already in `crates/emstocks/src/emStocksListBox.rs` `#[cfg(test)] mod tests` (smoke + signal-presence assertions).
- `widget_checkbox_*` goldens: the paint algorithm is unchanged in Unit 1; goldens are not regenerated. If a regeneration is unexpectedly needed, the implementer pauses and notes the delta in the commit message (acceptance-criteria escape hatch).

---

## File map

| File | Role | Change kind |
|------|------|-------------|
| `crates/emcore/src/emCheckBox.rs` | Widget | Modify — add `click_signal` field + accessor + tests |
| `crates/emstocks/src/emStocksPricesFetcher.rs` | Fetcher engine | Modify — add `AddListBox` + tracked-listboxes collection |
| `crates/emstocks/src/emStocksFetchPricesDialog.rs` | Dialog | Modify — add `AddListBox` delegating to fetcher |
| `crates/emstocks/src/emStocksListBox.rs` | List box | Modify — add three methods (`StartToFetchAllSharePrices`, `StartToFetchSharePrices`, `ShowWebPages`) |
| `crates/emstocks/src/emStocksControlPanel.rs` | Control panel | Modify — swap `check_signal` → `click_signal` on `owned_shares_first`, wire `fetch_fired` reaction |
| `crates/emstocks/src/emStocksItemPanel.rs` | Item panel | Modify — wire `fetch_share_price_fired`, `show_all_fired`, per-page `show_web_fired[i]` reactions |
| `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-001-emstocks-reaction-bodies.md` | Bucket | Modify — append closure section |

No new files. No file splits. Name correspondence preserved (every modified file already exists 1:1 with its C++ counterpart).

---

## Phase 1 — Unit 1: emCheckBox::click_signal mirror (emcore)

**Files:**
- Modify: `crates/emcore/src/emCheckBox.rs`

### Task 1.1: Add `click_signal` field to `emCheckBox` struct

**Files:**
- Modify: `crates/emcore/src/emCheckBox.rs:25-45`

- [ ] **Step 1: Read the existing emCheckButton precedent**

The mirror-sibling-port pattern is codified at `crates/emcore/src/emCheckButton.rs:32-40` and is the ground truth for the field doc-block. Reproduce its DIVERGED block verbatim, adapting "emCheckButton" → "emCheckBox" and the "row 222/223" reference to ControlPanel `owned_shares_first` (cpp:135).

- [ ] **Step 2: Add the field** at the bottom of the `emCheckBox` struct, right after the existing `check_signal` field (currently line 44):

```rust
    /// Allocated at construction; fired from the user-toggle path only
    /// (`Input` → `toggle`). NOT fired from `SetChecked` (programmatic).
    /// Mirrors C++ `emButton::GetClickSignal()` — emCheckBox inherits emButton
    /// in C++ via emCheckButton; in Rust the structs are sibling ports so the
    /// inherited accessor is reproduced as a separate `pub` field.
    ///
    /// DIVERGED: (language-forced) Rust lacks public inheritance; the
    /// codebase has codified the mirror-sibling-port pattern at
    /// `emCheckButton.rs:32-40` (B-012). FU-001 extends it to the leaf
    /// `emCheckBox` widget so subscribers needing user-click semantics
    /// (e.g. emStocksControlPanel `owned_shares_first`, cpp:135) do not
    /// feedback-loop with a sibling row's programmatic `SetChecked`.
    pub click_signal: SignalId,
```

Use the `Edit` tool. The `old_string` should include the existing `pub check_signal: SignalId,` line plus its preceding doc-block plus the closing `}` of the struct so the insertion is unambiguous.

- [ ] **Step 3: Allocate the signal in `new`** at line 63 (`check_signal: ctx.create_signal(),`):

Add a new field-init line directly after it:

```rust
            check_signal: ctx.create_signal(),
            click_signal: ctx.create_signal(),
```

- [ ] **Step 4: Run `cargo check -p emcore`**

Run: `cargo check -p emcore`
Expected: clean, no errors. (Field is unused at this point — that's fine; it's `pub`, so no dead-code warning.)

- [ ] **Step 5: Commit (intermediate, NOT yet pushed; final commit is at end of phase)**

Defer commit until Task 1.5.

### Task 1.2: Fire `click_signal` from `toggle` only (NOT from `SetChecked`)

**Files:**
- Modify: `crates/emcore/src/emCheckBox.rs` — the `toggle` private helper (currently does not exist; the toggle logic is inlined into `Input` at lines 419 and 434 via direct `self.toggle(ctx)` calls but the function itself is the `SetChecked`-flavoured block at lines 76-87 reproduced inline). Inspect the file to confirm the actual location.

Read the current `Input` paths (mouse-release at line ~419 and Enter at line ~434). Both call `self.toggle(ctx)`. Search the file: there must be a `fn toggle` somewhere — if not, the inline path is `SetChecked(!self.checked, ctx)`.

If a `fn toggle` private helper does not exist (current file has only `SetChecked`), introduce one mirroring `emCheckButton.rs:470-480`:

- [ ] **Step 1: Verify whether `fn toggle` exists**

Run: `grep -n "fn toggle\b" crates/emcore/src/emCheckBox.rs`
- If output is non-empty: skip to Step 3.
- If empty: continue to Step 2.

- [ ] **Step 2: Add the private `toggle` helper**

Place it directly after the `set_checked_silent` method (line ~104). Use this exact body — it mirrors `emCheckButton.rs:470-480`, but unconditionally toggles and fires both signals:

```rust
    /// Internal toggle helper (private). Implements the action C++ does in
    /// its protected virtual `Clicked()` override. Fires BOTH `click_signal`
    /// and `check_signal` — user-click toggle path, distinct from
    /// programmatic `SetChecked` (which fires only `check_signal`).
    /// Mirrors emCheckButton::toggle (`emCheckButton.rs:470-480`) — B-012
    /// feedback-loop guard.
    fn toggle(&mut self, ctx: &mut PanelCtx<'_>) {
        self.checked = !self.checked;
        if let Some(mut sched) = ctx.as_sched_ctx() {
            sched.fire(self.click_signal);
            sched.fire(self.check_signal);
            if let Some(cb) = self.on_check.as_mut() {
                cb(self.checked, &mut sched);
            }
        }
    }
```

- [ ] **Step 3: Replace inline toggles in `Input`**

For each call site that currently does `self.SetChecked(!self.checked, ctx)` or equivalent inline mutate-and-fire, replace with `self.toggle(ctx);`. There are TWO such sites:

  - In the `MouseLeft` `Release` arm: `if hit { self.toggle(ctx); }` (already in this form per current file at line ~419 — confirm).
  - In the `Enter` arm: `self.toggle(ctx)` (already in this form per current file at line ~434 — confirm).

If they are already `self.toggle(ctx)` calls (because Step 2 was a no-op), no change is needed here.

- [ ] **Step 4: Confirm `SetChecked` does NOT fire `click_signal`**

Read `crates/emcore/src/emCheckBox.rs` lines 76-87. The body must remain firing only `check_signal` (and the `on_check` callback). Do NOT add a `click_signal` fire here. This is the B-012 feedback-loop guard.

- [ ] **Step 5: Run `cargo check -p emcore`**

Run: `cargo check -p emcore`
Expected: clean.

### Task 1.3: Add `GetClickSignal` accessor

**Files:**
- Modify: `crates/emcore/src/emCheckBox.rs`

- [ ] **Step 1: Verify accessor consistency**

Run: `grep -n "fn GetCheckSignal\|fn GetClickSignal" crates/emcore/src/emCheckBox.rs`

The C++ surface exposes `GetCheckSignal()` (inherited from emCheckButton) and `GetClickSignal()` (inherited from emButton). The Rust port already lets callers field-access `.check_signal`. Add accessor methods only for symmetry with the C++ public surface. If `GetCheckSignal` is missing, add it now too.

- [ ] **Step 2: Add the accessors**

Place directly after `set_checked_silent`:

```rust
    /// Mirrors C++ `emCheckButton::GetCheckSignal()`. Field access
    /// (`self.check_signal`) is also permitted; this accessor exists for
    /// C++ surface parity.
    pub fn GetCheckSignal(&self) -> SignalId {
        self.check_signal
    }

    /// Mirrors C++ `emButton::GetClickSignal()` (inherited via emCheckButton).
    /// Field access (`self.click_signal`) is also permitted.
    pub fn GetClickSignal(&self) -> SignalId {
        self.click_signal
    }
```

If `GetCheckSignal` already exists, add only `GetClickSignal`.

- [ ] **Step 3: Run `cargo check -p emcore`**

Run: `cargo check -p emcore`
Expected: clean.

### Task 1.4: Tests — user-toggle fires both, SetChecked fires only check_signal

**Files:**
- Modify: `crates/emcore/src/emCheckBox.rs` — extend `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block (after `check_box_fires_check_signal_on_toggle`, around line 627). Reuse `TestInit` and `PanelCtx::with_sched_reach` exactly as the existing test does:

```rust
    #[test]
    fn check_box_user_toggle_fires_both_signals() {
        let look = emLook::new();
        let mut init = TestInit::new();
        let mut cb = emCheckBox::new(&mut init.ctx(), "Enable", look);
        let click = cb.click_signal;
        let check = cb.check_signal;
        let ps = default_panel_state();
        let is = default_input_state();
        let (mut tree, tid) = test_tree();
        let fw_cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree, tid, 1.0, &mut init.sched, &mut init.fw,
                &init.root, &fw_cb, &init.pa,
            );
            cb.Input(&emInputEvent::press(InputKey::Enter), &ps, &is, &mut ctx);
        }
        assert!(init.sched.is_pending(click), "click_signal must fire on user toggle");
        assert!(init.sched.is_pending(check), "check_signal must fire on user toggle");
    }

    #[test]
    fn check_box_set_checked_fires_only_check_signal() {
        let look = emLook::new();
        let mut init = TestInit::new();
        let mut cb = emCheckBox::new(&mut init.ctx(), "Enable", look);
        let click = cb.click_signal;
        let check = cb.check_signal;
        let (mut tree, tid) = test_tree();
        let fw_cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        {
            let mut ctx = PanelCtx::with_sched_reach(
                &mut tree, tid, 1.0, &mut init.sched, &mut init.fw,
                &init.root, &fw_cb, &init.pa,
            );
            cb.SetChecked(true, &mut ctx);
        }
        assert!(init.sched.is_pending(check), "check_signal must fire on SetChecked");
        assert!(
            !init.sched.is_pending(click),
            "click_signal MUST NOT fire on programmatic SetChecked (B-012 feedback-loop guard)"
        );
    }
```

- [ ] **Step 2: Run the new tests, expect PASS**

Run: `cargo nextest run -p emcore check_box_user_toggle_fires_both_signals check_box_set_checked_fires_only_check_signal`
Expected: both PASS. (They are not in fact written-then-failing — the implementation in Tasks 1.1–1.3 already makes them pass. This is acceptable for the mirror-port pattern: the spec lists them as positive-confirmation tests, not red-green discovery.)

- [ ] **Step 3: If a test fails, diagnose**

If `check_box_user_toggle_fires_both_signals` fails: `toggle` is not firing `click_signal` — re-check Task 1.2.
If `check_box_set_checked_fires_only_check_signal` fails on the `!is_pending(click)` assertion: `SetChecked` is incorrectly firing `click_signal` — re-check Task 1.2 Step 4.

### Task 1.5: Phase 1 gate + commit

- [ ] **Step 1: Run the project gates**

Run in order:
```
cargo check
cargo clippy -- -D warnings
cargo nextest run -p emcore
cargo xtask annotations
```
Expected: all green.

- [ ] **Step 2: Stage and commit**

```bash
git add crates/emcore/src/emCheckBox.rs
git commit -m "$(cat <<'EOF'
feat(emCheckBox): mirror click_signal per B-012 mirror-sibling-port (FU-001 Unit 1)

Extends the click_signal mirror-sibling-port pattern (codified at
emCheckButton.rs:32-40 during B-012) to the leaf emCheckBox widget.
User-toggle path fires both click_signal and check_signal; programmatic
SetChecked fires only check_signal — preserves the B-012 feedback-loop
guard for subscribers that need user-click semantics
(e.g. emStocksControlPanel.cpp:135 owned_shares_first).

DIVERGED: (language-forced) per established convention. No paint-path
changes; widget_checkbox_* goldens unaffected.
EOF
)"
```

---

## Phase 2 — Unit 2: Fetcher + Dialog AddListBox ports

**Files:**
- Modify: `crates/emstocks/src/emStocksPricesFetcher.rs`
- Modify: `crates/emstocks/src/emStocksFetchPricesDialog.rs`

### Task 2.1: Add `list_boxes` tracked collection to `emStocksPricesFetcher`

**Files:**
- Modify: `crates/emstocks/src/emStocksPricesFetcher.rs:25-105`

- [ ] **Step 1: Decide the field type**

C++ uses `emArray<emCrossPtr<emStocksListBox>>`. The Rust port needs a tracking collection that can be iterated to skip duplicates per C++ `AddListBox` body. Use:

```rust
    /// Mirrors C++ `emArray<emCrossPtr<emStocksListBox>> ListBoxes`
    /// (emStocksPricesFetcher.h:88). Holds weak-style references for
    /// dedup in `AddListBox`. Rc<RefCell<>> per CLAUDE.md §Ownership (a)
    /// — engine-callback-held across the fetcher's `cycle()`.
    pub(crate) list_boxes: Vec<std::rc::Weak<std::cell::RefCell<super::emStocksListBox::emStocksListBox>>>,
```

Place it after `file_model` (line ~64).

- [ ] **Step 2: Initialise in `new`**

In the `new` constructor (line 87-105), add:
```rust
            list_boxes: Vec::new(),
```
right after the `file_model: None,` initializer.

- [ ] **Step 3: Run `cargo check -p emstocks`**

Run: `cargo check -p emstocks`
Expected: clean.

### Task 2.2: Write the failing AddListBox test on the fetcher

**Files:**
- Modify: `crates/emstocks/src/emStocksPricesFetcher.rs` — `#[cfg(test)] mod tests`

- [ ] **Step 1: Locate the existing tests block**

Run: `grep -n "^mod tests\|#\[cfg(test)\]" crates/emstocks/src/emStocksPricesFetcher.rs | head -5`

Add the new test inside that block.

- [ ] **Step 2: Write the test**

```rust
    #[test]
    fn add_list_box_appends_and_dedups() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut f = emStocksPricesFetcher::new("script", "", "key");
        let lb = Rc::new(RefCell::new(crate::emStocksListBox::emStocksListBox::new()));
        f.AddListBox(&lb);
        f.AddListBox(&lb); // dedup
        assert_eq!(
            f.list_boxes.iter().filter(|w| w.upgrade().is_some()).count(),
            1,
            "AddListBox must dedup repeat calls (C++ emStocksPricesFetcher.cpp:48-56)"
        );
    }
```

- [ ] **Step 3: Run the test, expect FAIL with "no method named AddListBox"**

Run: `cargo test -p emstocks add_list_box_appends_and_dedups -- --nocapture`
Expected: compile error — method missing.

### Task 2.3: Implement `emStocksPricesFetcher::AddListBox`

**Files:**
- Modify: `crates/emstocks/src/emStocksPricesFetcher.rs`

- [ ] **Step 1: Add the method**

In the `impl emStocksPricesFetcher` block, after `with_file_model` (line ~120), add:

```rust
    /// Port of C++ `emStocksPricesFetcher::AddListBox`
    /// (`emStocksPricesFetcher.cpp:48-56`). Dedups by Rc identity (C++ uses
    /// emCrossPtr equality on the underlying pointer).
    ///
    /// Does not fire any signal — C++ body has no `Signal(...)` call.
    /// Therefore no `SignalCtx` parameter is threaded (D-007).
    pub fn AddListBox(
        &mut self,
        list_box: &std::rc::Rc<std::cell::RefCell<super::emStocksListBox::emStocksListBox>>,
    ) {
        // Dedup: skip if any existing weak ref still points at the same
        // allocation as the incoming Rc.
        for w in &self.list_boxes {
            if let Some(existing) = w.upgrade() {
                if std::rc::Rc::ptr_eq(&existing, list_box) {
                    return;
                }
            }
        }
        self.list_boxes.push(std::rc::Rc::downgrade(list_box));
    }
```

- [ ] **Step 2: Re-run the test, expect PASS**

Run: `cargo test -p emstocks add_list_box_appends_and_dedups`
Expected: PASS.

### Task 2.4: Write the failing AddListBox test on the dialog

**Files:**
- Modify: `crates/emstocks/src/emStocksFetchPricesDialog.rs`

- [ ] **Step 1: Add the test**

In `#[cfg(test)] mod tests` (locate via `grep -n "mod tests\|#\[cfg(test)\]" crates/emstocks/src/emStocksFetchPricesDialog.rs`), add:

```rust
    #[test]
    fn dialog_add_list_box_delegates_to_fetcher() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut d = emStocksFetchPricesDialog::new("script", "", "key");
        let lb = Rc::new(RefCell::new(crate::emStocksListBox::emStocksListBox::new()));
        d.AddListBox(&lb);
        assert_eq!(
            d.fetcher.list_boxes.iter().filter(|w| w.upgrade().is_some()).count(),
            1,
        );
    }
```

- [ ] **Step 2: Run the test, expect FAIL**

Run: `cargo test -p emstocks dialog_add_list_box_delegates_to_fetcher`
Expected: compile error — method missing.

### Task 2.5: Implement `emStocksFetchPricesDialog::AddListBox`

**Files:**
- Modify: `crates/emstocks/src/emStocksFetchPricesDialog.rs`

- [ ] **Step 1: Add the method**

In `impl emStocksFetchPricesDialog`, alongside `AddStockIds` (line 130), add:

```rust
    /// Port of C++ `emStocksFetchPricesDialog::AddListBox` inline
    /// (`emStocksFetchPricesDialog.h:78-81`). Delegates to the fetcher.
    pub fn AddListBox(
        &mut self,
        list_box: &std::rc::Rc<std::cell::RefCell<super::emStocksListBox::emStocksListBox>>,
    ) {
        self.fetcher.AddListBox(list_box);
    }
```

- [ ] **Step 2: Run the test, expect PASS**

Run: `cargo test -p emstocks dialog_add_list_box_delegates_to_fetcher`
Expected: PASS.

### Task 2.6: Phase 2 gate + commit

- [ ] **Step 1: Run gates**

```
cargo check
cargo clippy -- -D warnings
cargo nextest run -p emstocks
cargo xtask annotations
```
Expected: all green.

- [ ] **Step 2: Commit**

```bash
git add crates/emstocks/src/emStocksPricesFetcher.rs crates/emstocks/src/emStocksFetchPricesDialog.rs
git commit -m "$(cat <<'EOF'
feat(emstocks): port AddListBox on PricesFetcher + FetchPricesDialog (FU-001 Unit 2)

Mirrors C++ emStocksPricesFetcher.cpp:48-56 and the inline
emStocksFetchPricesDialog.h:78-81 delegate. Dedup by Rc::ptr_eq
substitutes for emCrossPtr equality. No signal fired — matches C++.
EOF
)"
```

---

## Phase 3 — Unit 3: emStocksListBox method ports

**Depends on Unit 2.** `StartToFetchSharePrices` calls `dialog.AddListBox(...)`.

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs`

### Task 3.1: Confirm imports + helper signatures

- [ ] **Step 1: Inspect the head of emStocksListBox.rs**

Run: `head -40 crates/emstocks/src/emStocksListBox.rs`

Confirm `emStocksFetchPricesDialog`, `emStocksFileModel`, and `GetCurrentDate` are reachable. If not, add `use` statements (paths: `super::emStocksFetchPricesDialog::emStocksFetchPricesDialog;`, `super::emStocksFileModel`).

- [ ] **Step 2: Confirm the existing config + file_model fields and `SetSelectedDate` method**

Run:
```
grep -n "fn SetSelectedDate\|file_model\|config\b" crates/emstocks/src/emStocksListBox.rs | head -20
```

Note the actual field names and `SetSelectedDate` signature. Use them verbatim in Tasks 3.3 and 3.5.

### Task 3.2: Write the failing `StartToFetchAllSharePrices` test

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs` — `#[cfg(test)] mod tests`

- [ ] **Step 1: Add the test (skeleton — exact stock-rec construction follows the existing test patterns in the same module)**

Locate an existing test that constructs an `emStocksListBox` with items (e.g. one calling `GetItemCount`/`GetStockByItemIndex`). Mirror its setup. Then assert that after `StartToFetchAllSharePrices`, the file_model's `PricesFetchingDialog` becomes valid (a dialog was created or referenced) AND the dialog's fetcher has stock IDs queued matching the items.

Concretely, append:

```rust
    #[test]
    fn start_to_fetch_all_share_prices_forwards_all_visible_ids() {
        // Construct minimal fixture mirroring existing tests in this module.
        // Fixture must yield ≥1 item visible in the listbox.
        // Use the same TestInit/SignalCtx pattern as B-001-followup tests.
        let (mut lb, mut rec, file_model_rc, _config_rc, mut sched_ctx) =
            test_fixtures::list_box_with_two_stocks(); // helper exists or
                                                        // mirrors existing pattern
        lb.StartToFetchAllSharePrices(&mut sched_ctx);
        assert!(file_model_rc.borrow().PricesFetchingDialog.is_valid());
        // Each visible stock id must appear in the fetcher's queued ids.
        let expected: Vec<String> = lb.GetVisibleStockIds(&rec);
        let got = file_model_rc.borrow().PricesFetchingDialog
            .as_ref().expect("dialog").fetcher.stock_ids.clone();
        for id in expected {
            assert!(got.contains(&id), "queued ids must contain {}", id);
        }
        let _ = rec;
    }
```

If `test_fixtures::list_box_with_two_stocks` does not exist, write the fixture inline in the test using the construction shape used by an existing test in the same module (find via `grep -n "emStocksListBox::new\|fn .*test.*list_box" crates/emstocks/src/emStocksListBox.rs`).

- [ ] **Step 2: Run the test, expect FAIL (compile error)**

Run: `cargo test -p emstocks start_to_fetch_all_share_prices_forwards_all_visible_ids`
Expected: compile error — method missing.

### Task 3.3: Implement `StartToFetchAllSharePrices`

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs` — within `impl emStocksListBox`

- [ ] **Step 1: Add the method**

Place near `GetVisibleStockIds` (line ~1278). Body mirrors C++ `emStocksListBox.cpp:371-383`:

```rust
    /// Port of C++ `emStocksListBox::StartToFetchSharePrices()` zero-arg
    /// overload (`emStocksListBox.cpp:371-383`). Iterates all items and
    /// forwards their stock ids to the array overload.
    ///
    /// Rust has no function overloading; the array overload keeps the
    /// C++ name (`StartToFetchSharePrices`) and the zero-arg variant is
    /// renamed `StartToFetchAllSharePrices` (descriptive). Both methods
    /// carry this cite-comment.
    pub fn StartToFetchAllSharePrices(
        &mut self,
        ectx: &mut impl emcore::emEngineCtx::SignalCtx,
        rec: &emStocksRec,
    ) {
        let mut stock_ids: Vec<String> = Vec::with_capacity(self.GetItemCount());
        for i in 0..self.GetItemCount() {
            if let Some(stock) = self.GetStockByItemIndex(i, rec) {
                stock_ids.push(stock.id.clone());
            }
        }
        self.StartToFetchSharePrices(ectx, &stock_ids);
    }
```

Note: if the existing `GetItemCount` / `GetStockByItemIndex` Rust signatures differ (e.g. take `&emStocksRec`), thread `rec` as shown. Adjust signature in Task 3.2 test if needed.

- [ ] **Step 2: Defer compile** until Task 3.5 (`StartToFetchSharePrices` is also missing).

### Task 3.4: Write the failing `StartToFetchSharePrices` (array) test

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs` — tests

- [ ] **Step 1: Add the test**

```rust
    #[test]
    fn start_to_fetch_share_prices_creates_dialog_when_absent() {
        let (mut lb, _rec, file_model_rc, _config_rc, mut sched_ctx) =
            test_fixtures::list_box_with_two_stocks();
        assert!(!file_model_rc.borrow().PricesFetchingDialog.is_valid());
        let ids = vec!["AAPL".to_string()];
        lb.StartToFetchSharePrices(&mut sched_ctx, &ids);
        assert!(file_model_rc.borrow().PricesFetchingDialog.is_valid());
    }

    #[test]
    fn start_to_fetch_share_prices_reuses_existing_dialog() {
        let (mut lb, _rec, file_model_rc, _config_rc, mut sched_ctx) =
            test_fixtures::list_box_with_two_stocks();
        lb.StartToFetchSharePrices(&mut sched_ctx, &["AAPL".into()]);
        let first_ptr = file_model_rc.borrow().PricesFetchingDialog
            .as_ref().map(|d| d as *const _);
        lb.StartToFetchSharePrices(&mut sched_ctx, &["MSFT".into()]);
        let second_ptr = file_model_rc.borrow().PricesFetchingDialog
            .as_ref().map(|d| d as *const _);
        assert_eq!(first_ptr, second_ptr,
            "second call must reuse the existing dialog (C++ Raise() branch)");
    }
```

- [ ] **Step 2: Run, expect FAIL (compile error)**

Run: `cargo test -p emstocks start_to_fetch_share_prices`
Expected: compile error.

### Task 3.5: Implement `StartToFetchSharePrices` (array overload)

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs`

- [ ] **Step 1: Add the method**

Body mirrors C++ `emStocksListBox.cpp:386-410`:

```rust
    /// Port of C++ `emStocksListBox::StartToFetchSharePrices(const
    /// emArray<emString> & stockIds)` (`emStocksListBox.cpp:386-410`).
    ///
    /// C++ overload of StartToFetchSharePrices.
    pub fn StartToFetchSharePrices(
        &mut self,
        ectx: &mut impl emcore::emEngineCtx::SignalCtx,
        stock_ids: &[String],
    ) {
        // Acquire the FileModel + Config refs the C++ ctor closes over.
        let file_model_rc = self.file_model.clone();
        let api_script;
        let api_interpreter;
        let api_key;
        {
            let cfg = self.config.borrow();
            api_script = cfg.api_script.clone();
            api_interpreter = cfg.api_script_interpreter.clone();
            api_key = cfg.api_key.clone();
        }

        let already_open = file_model_rc.borrow().PricesFetchingDialog.is_valid();
        if already_open {
            // C++ FileModel.PricesFetchingDialog->Raise().
            // TODO(FU-001): port emDialog::Raise once view-parenting lands
            // (UPSTREAM-GAP — Rust dialog ctor takes no view).
            eprintln!("[FU-001] PricesFetchingDialog::Raise (no-op stub)");
        } else {
            let dialog = emStocksFetchPricesDialog::new_with_model(
                &api_script, &api_interpreter, &api_key, file_model_rc.clone(),
            );
            file_model_rc.borrow_mut().PricesFetchingDialog.set(dialog);
        }

        // C++: date = FileModel.GetLatestPricesDate(); if empty → GetCurrentDate().
        let mut date = file_model_rc.borrow().GetLatestPricesDate();
        if date.is_empty() {
            date = super::emStocksRec::GetCurrentDate();
        }
        self.SetSelectedDate(ectx, &date);

        // Both AddListBox and AddStockIds must run on the now-current dialog.
        // self_rc: the ListBox needs to pass an Rc<RefCell<Self>> to the
        // dialog. Take it from the precondition that callers always hold an
        // Rc<RefCell<emStocksListBox>>; use a side-channel field on the
        // ListBox if not already present (`self_weak: Weak<RefCell<Self>>`).
        let self_rc = self
            .self_weak
            .upgrade()
            .expect("emStocksListBox must be Rc<RefCell<>>-allocated for StartToFetchSharePrices");
        let mut model_borrow = file_model_rc.borrow_mut();
        if let Some(dialog) = model_borrow.PricesFetchingDialog.as_mut() {
            dialog.AddListBox(&self_rc);
            dialog.AddStockIds(ectx, stock_ids);
        }
    }
```

- [ ] **Step 2: Confirm the `self_weak` field exists**

Run: `grep -n "self_weak" crates/emstocks/src/emStocksListBox.rs`

If absent, this is a real implementation barrier requiring a wider change. In that case, the implementer should:
  (a) Add `self_weak: std::rc::Weak<std::cell::RefCell<emStocksListBox>>` as a field initialised after construction by the call site that produces the `Rc`. Search call sites: `grep -n "emStocksListBox::new" crates/emstocks/src/`. If the call sites already wrap in `Rc::new(RefCell::new(...))`, add a `set_self_weak(&mut self, w: Weak<...>)` initializer and call it post-construction.
  (b) If wiring `self_weak` would touch more than one call site, escalate: the design is sound but the prerequisite is missing. Pause and document in the bucket file rather than rewriting all sites silently.

If `self_weak` exists, no extra work.

- [ ] **Step 3: Inspect `emCrossPtr<T>` API surface**

Run: `grep -n "fn set\|fn is_valid\|fn as_mut\|impl.*emCrossPtr" crates/emcore/src/emCrossPtr.rs`

The method names used in Step 1 (`set`, `as_mut`, `is_valid`) must match the actual API. If they differ (e.g. `Set`, `IsValid`, `AsMut`), substitute. Re-read the Rust port's emCrossPtr shape and adapt.

- [ ] **Step 4: Run all four method tests**

Run:
```
cargo test -p emstocks start_to_fetch_share_prices
cargo test -p emstocks start_to_fetch_all_share_prices
```
Expected: PASS.

### Task 3.6: Write the failing ShowWebPages tests

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs` — tests

- [ ] **Step 1: Add the tests**

```rust
    #[test]
    fn show_web_pages_no_browser_logs_and_returns() {
        let (lb, _rec, _file_model_rc, config_rc, _sched_ctx) =
            test_fixtures::list_box_with_two_stocks();
        config_rc.borrow_mut().web_browser.clear();
        // Should not panic, should not spawn a process.
        lb.ShowWebPages(&["https://example.com".into()]);
    }

    #[test]
    fn show_web_pages_empty_list_is_noop() {
        let (lb, _rec, _file_model_rc, _config_rc, _sched_ctx) =
            test_fixtures::list_box_with_two_stocks();
        lb.ShowWebPages(&[]);
    }
```

The "happy path with mockable process spawn" from the spec is omitted — `emProcess::TryStartUnmanaged` is not mockable without a wrapper, and the spec leaves the depth at "unit-level reaction tests asserting the correct ListBox method was called" (deferred to Unit 4). The two negative-path tests above are sufficient acceptance for Unit 3.

- [ ] **Step 2: Run, expect FAIL (compile error)**

Run: `cargo test -p emstocks show_web_pages`
Expected: compile error.

### Task 3.7: Implement `ShowWebPages`

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs`

- [ ] **Step 1: Add the method**

Body mirrors C++ `emStocksListBox.cpp:496-501+`:

```rust
    /// Port of C++ `emStocksListBox::ShowWebPages(const emArray<emString>
    /// & webPages) const` (`emStocksListBox.cpp:496-501+`). `&self`
    /// matches the C++ `const`.
    pub fn ShowWebPages(&self, web_pages: &[String]) {
        if web_pages.is_empty() {
            return;
        }
        let cfg = self.config.borrow();
        let browser = cfg.web_browser.clone();
        drop(cfg);
        if browser.is_empty() {
            // TODO(FU-001): replace with emDialog::ShowMessage when ported.
            eprintln!(
                "[emStocksListBox::ShowWebPages] Web browser is not configured."
            );
            return;
        }
        let mut args: Vec<&str> = Vec::with_capacity(1 + web_pages.len());
        args.push(browser.as_str());
        for p in web_pages {
            args.push(p.as_str());
        }
        let env = std::collections::HashMap::new();
        if let Err(e) = emcore::emProcess::emProcess::TryStartUnmanaged(
            &args,
            &env,
            None,
            emcore::emProcess::StartFlags::DEFAULT,
        ) {
            // TODO(FU-001): replace with emDialog::ShowMessage when ported.
            eprintln!(
                "[emStocksListBox::ShowWebPages] Failed to start browser: {}",
                e
            );
        }
    }
```

- [ ] **Step 2: Verify `StartFlags::DEFAULT` exists**

Run: `grep -n "DEFAULT\|StartFlags" crates/emcore/src/emProcess.rs | head -10`

If `DEFAULT` is not the correct name, substitute the empty-flags constructor used by other call sites in the codebase (e.g. `StartFlags::empty()`).

- [ ] **Step 3: Run tests, expect PASS**

Run: `cargo test -p emstocks show_web_pages`
Expected: PASS.

### Task 3.8: Phase 3 gate + commit

- [ ] **Step 1: Run gates**

```
cargo check
cargo clippy -- -D warnings
cargo nextest run -p emstocks
cargo xtask annotations
```

- [ ] **Step 2: Commit**

```bash
git add crates/emstocks/src/emStocksListBox.rs
git commit -m "$(cat <<'EOF'
feat(emstocks): port StartToFetchSharePrices x2 + ShowWebPages on ListBox (FU-001 Unit 3)

Ports three previously-missing C++ methods:
- StartToFetchAllSharePrices: zero-arg overload (cpp:371-383)
- StartToFetchSharePrices(&[String]): array overload (cpp:386-410)
- ShowWebPages(&[String]): const browser-spawn (cpp:496-501+)

Naming: array overload keeps C++ name; zero-arg renamed to
StartToFetchAllSharePrices (Rust no overloading, descriptive).
Raise() and emDialog::ShowMessage are TODO(FU-001)-stubbed pending
view-parenting and emDialog ports respectively.
EOF
)"
```

---

## Phase 4 — Unit 4: Reaction-body completion

**Depends on Units 1 + 3.**

**Files:**
- Modify: `crates/emstocks/src/emStocksControlPanel.rs`
- Modify: `crates/emstocks/src/emStocksItemPanel.rs`

### Task 4.1: Swap `owned_shares_first.check_signal` → `click_signal`

**Files:**
- Modify: `crates/emstocks/src/emStocksControlPanel.rs` lines 829-835 + 916

- [ ] **Step 1: Read the current block**

Read lines 825-840. Confirm the `check_signal` connect site (line 835) and the multi-line TODO comment block (lines 829-834).

- [ ] **Step 2: Replace the connect + remove TODO**

Use `Edit`. `old_string`:

```rust
                // Row -566 — C++ cpp:135 wires `OwnedSharesFirst->GetClickSignal()`
                // (B-001 design C-3). The Rust `emCheckBox` exposes only
                // `check_signal` (no inherited GetClickSignal accessor); using
                // it preserves the toggle reaction observably equivalently for
                // keyboard- and click-driven toggles. TODO: when emCheckBox
                // gains a click-signal accessor (B-001 prereq miss), swap.
                ectx.connect(w.owned_shares_first.check_signal, eid);
```

`new_string`:
```rust
                // Row -566 — C++ cpp:135 wires `OwnedSharesFirst->GetClickSignal()`
                // (B-001 design C-3). Per FU-001 Unit 1, emCheckBox now mirrors
                // click_signal (B-012 mirror-sibling-port pattern); use it
                // exactly as C++ does — preserves user-click semantics,
                // suppresses programmatic-SetChecked feedback.
                ectx.connect(w.owned_shares_first.click_signal, eid);
```

- [ ] **Step 3: Update the corresponding `IsSignaled` site**

Read line 916. Currently:
```rust
            let owned_first_fired = ectx.IsSignaled(w.owned_shares_first.check_signal);
```
Change `check_signal` → `click_signal`. Use `Edit` with the full line as `old_string`.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p emstocks owned_shares_first`
Expected: any pre-existing tests covering this row still pass. If they assert on `check_signal`, update them to `click_signal` (the asserted behavior is identical except for which signal carries it).

### Task 4.2: Wire `fetch_fired` reaction in ControlPanel

**Files:**
- Modify: `crates/emstocks/src/emStocksControlPanel.rs` line ~1045

- [ ] **Step 1: Inspect surrounding context for `rec` access**

Read lines 1030-1060. The `StartToFetchAllSharePrices` method needs `&emStocksRec`. Confirm whether the surrounding scope has an `&emStocksRec` borrow available; if not, the method must accept whatever is in scope. Adjust as Task 3.3 already exposed `rec: &emStocksRec`.

- [ ] **Step 2: Replace the stub**

Use `Edit`. `old_string`:
```rust
            let _ = fetch_fired; // TODO: wire to FetchPricesDialog when emStocksFilePanel surfaces it.
```

`new_string`:
```rust
            // FU-001 Unit 4 — reaction wired against the ListBox.
            if fetch_fired {
                self.list_box
                    .borrow_mut()
                    .StartToFetchAllSharePrices(ectx, rec);
            }
```

If `rec` is not in scope at this point, walk up the enclosing function signature to confirm. If the method does not take `rec`, pass it through (this is a small-radius extension consistent with Task 3.3's signature).

- [ ] **Step 3: Run `cargo check -p emstocks`**

Expected: clean.

### Task 4.3: Wire `fetch_share_price_fired` reaction in ItemPanel

**Files:**
- Modify: `crates/emstocks/src/emStocksItemPanel.rs` line 1022

- [ ] **Step 1: Inspect lines 1015-1027** to see the actual variable names for the current stock and rec.

- [ ] **Step 2: Replace the stub**

`old_string`:
```rust
        let _ = fetch_share_price_fired; // TODO: wire to StartToFetchSharePrices when ListBox exposes it.
```

`new_string`:
```rust
        // FU-001 Unit 4 — wire to ListBox.StartToFetchSharePrices(array).
        if fetch_share_price_fired {
            let id = stock.id.clone();
            self.list_box.borrow_mut().StartToFetchSharePrices(ectx, &[id]);
        }
```

If the variable is not named `stock` in this scope, substitute the actual binding (read lines 980-1015 to find it).

### Task 4.4: Wire `show_all_fired` reaction in ItemPanel

**Files:**
- Modify: `crates/emstocks/src/emStocksItemPanel.rs` line 1023

- [ ] **Step 1: Replace the stub**

`old_string`:
```rust
        let _ = show_all_fired; // TODO: wire to ShowWebPages when ListBox exposes it.
```

`new_string`:
```rust
        // FU-001 Unit 4 — wire to ListBox.ShowWebPages with all non-empty pages.
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

### Task 4.5: Wire per-page `show_web_fired[i]` reaction in ItemPanel

**Files:**
- Modify: `crates/emstocks/src/emStocksItemPanel.rs` lines 1024-1026

- [ ] **Step 1: Inspect the loop**

Read lines 1024-1027. Confirm whether the loop iterator binds index `i` (e.g. `for (i, &fired) in show_web_fired.iter().enumerate()`) or just `&fired`. If the latter, adjust the loop to enumerate.

- [ ] **Step 2: Replace the loop body (and possibly the loop header)**

If the loop is `for &fired in show_web_fired.iter()`, change it to `for (i, &fired) in show_web_fired.iter().enumerate()` and replace the body:

`old_string`:
```rust
        for &fired in show_web_fired.iter() {
            let _ = fired; // TODO: wire to ShowWebPages.
        }
```

`new_string`:
```rust
        for (i, &fired) in show_web_fired.iter().enumerate() {
            // FU-001 Unit 4 — single-page show.
            if fired && i < stock.web_pages.len() && !stock.web_pages[i].is_empty() {
                self.list_box
                    .borrow()
                    .ShowWebPages(&[stock.web_pages[i].clone()]);
            }
        }
```

### Task 4.6: Confirm no `TODO(B-001-followup)` markers remain

- [ ] **Step 1: Grep for the markers**

Run:
```
grep -n "TODO.*wire\|TODO.*FetchPricesDialog\|TODO.*StartToFetchSharePrices\|TODO.*ShowWebPages\|TODO.*click-signal accessor" crates/emstocks/src/emStocksControlPanel.rs crates/emstocks/src/emStocksItemPanel.rs
```
Expected: zero hits (the five markers from the spec are removed; only the new `TODO(FU-001):` markers in `emStocksListBox.rs` for `Raise` and `ShowMessage` remain — those are out of scope per the spec).

### Task 4.7: Phase 4 gate + commit

- [ ] **Step 1: Run gates**

```
cargo check
cargo clippy -- -D warnings
cargo nextest run
cargo xtask annotations
```
Expected: all green.

- [ ] **Step 2: Commit**

```bash
git add crates/emstocks/src/emStocksControlPanel.rs crates/emstocks/src/emStocksItemPanel.rs
git commit -m "$(cat <<'EOF'
feat(emstocks): wire FU-001 reaction bodies + swap owned_shares_first.click_signal (Unit 4)

Closes the five TODO stubs from B-001-followup:
- ControlPanel owned_shares_first: connect/IsSignaled now use click_signal
  (B-012 mirror-sibling-port — see Unit 1).
- ControlPanel fetch_fired → ListBox.StartToFetchAllSharePrices.
- ItemPanel fetch_share_price_fired → ListBox.StartToFetchSharePrices(&[id]).
- ItemPanel show_all_fired → ListBox.ShowWebPages(non_empty_pages).
- ItemPanel show_web_fired[i] → ListBox.ShowWebPages(&[pages[i]]).

Each reaction matches C++ at the cited line (cpp:135, cpp:566, cpp:586,
cpp:650+). Raise() and emDialog::ShowMessage remain TODO(FU-001) in
emStocksListBox per spec scope.
EOF
)"
```

---

## Phase 5 — Reconciliation + final gate

### Task 5.1: Update the FU-001 bucket file

**Files:**
- Modify: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-001-emstocks-reaction-bodies.md`

- [ ] **Step 1: Read current bucket file**

Read the bucket file. Identify its closure-section convention (mirror an already-closed bucket if one exists).

- [ ] **Step 2: Append a closure section**

Append to the bottom:

```markdown

## Closure (2026-05-02)

**Status:** Resolved. All five reaction-body stubs replaced; emCheckBox click_signal mirror landed.

**Commits (this branch):**
- Unit 1 (emCheckBox click_signal mirror) — emcore-isolated.
- Unit 2 (PricesFetcher + FetchPricesDialog AddListBox).
- Unit 3 (ListBox StartToFetchSharePrices x2 + ShowWebPages).
- Unit 4 (Reaction-body wiring + owned_shares_first signal swap).

**Out of scope (deferred, tracked at source):**
- emDialog::Raise port — UPSTREAM-GAP (view-parenting). TODO(FU-001) at emStocksListBox::StartToFetchSharePrices.
- emDialog::ShowMessage port — separate emcore work. TODO(FU-001) at emStocksListBox::ShowWebPages.
- GetFileStateSignal conflation — split out as FU-005.

**Verification:** cargo check, cargo clippy -D warnings, cargo nextest run, cargo xtask annotations — all green.
```

### Task 5.2: Final gate

- [ ] **Step 1: Full project gate**

Run:
```
cargo check
cargo clippy -- -D warnings
cargo-nextest ntr
cargo xtask annotations
```
Expected: all green.

- [ ] **Step 2: Confirm acceptance criteria**

Walk through the spec acceptance list:
- [ ] All 5 `TODO(B-001-followup)` markers removed (Task 4.6 grep is clean).
- [ ] `emCheckBox.click_signal` field + `GetClickSignal` accessor exist (Tasks 1.1, 1.3).
- [ ] B-012 fire rules preserved (Task 1.4 tests pass).
- [ ] `emStocksListBox` exposes `StartToFetchSharePrices`, `StartToFetchAllSharePrices`, `ShowWebPages` (Tasks 3.3, 3.5, 3.7).
- [ ] `emStocksFetchPricesDialog` and `emStocksPricesFetcher` expose `AddListBox` (Tasks 2.3, 2.5).
- [ ] All reactions match C++ at cited line numbers (Tasks 4.1–4.5).
- [ ] `cargo-nextest ntr` green; `cargo clippy -D warnings` green; `cargo xtask annotations` clean.
- [ ] `widget_checkbox_*` goldens unchanged (Unit 1 paint path is untouched). If they regenerated unexpectedly, the implementer documented the delta in the Unit 1 commit message.

### Task 5.3: Commit reconciliation

- [ ] **Step 1: Commit the bucket update**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-001-emstocks-reaction-bodies.md
git commit -m "$(cat <<'EOF'
docs(FU-001): close bucket — all reaction bodies wired, click_signal mirror landed

5 TODO(B-001-followup) stubs resolved across emStocksControlPanel and
emStocksItemPanel. emCheckBox click_signal mirror (B-012 pattern) added.
Raise() and emDialog::ShowMessage TODOs remain at source-call-sites
pending separate ports.
EOF
)"
```

- [ ] **Step 2: Do NOT push.** The plan ends here.

---

## Self-review notes

- Spec coverage: every spec section maps to a numbered task (Unit 1 → Phase 1; Unit 2 → Phase 2; Unit 3 → Phase 3; Unit 4 → Phase 4; out-of-scope items tracked as `TODO(FU-001)` at source). Acceptance-criteria items mapped in Task 5.2.
- Placeholders: the only "TODO" tokens in this plan are inside code blocks and represent intentional, scoped, source-tagged deferrals (`TODO(FU-001)`) that the spec explicitly puts out of scope. They are not plan-level placeholders.
- Type consistency: `StartToFetchSharePrices(ectx, &[String])` and `StartToFetchAllSharePrices(ectx, rec)` signatures threaded through Tasks 3.2/3.3/3.5/4.2/4.3 are consistent. Task 3.5 flags an implementation prerequisite (`self_weak`) and includes an escalation path if it is missing — so the plan does not silently assume infrastructure that may not exist.
- Risk: Task 3.5's `self_weak` assumption is the highest-uncertainty point. Step 2 in that task explicitly verifies and escalates rather than rewriting silently.
