# B-001 — emstocks signal wiring (P-001) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land all 71 P-001 rows in the emstocks crate — three accessor-side gap fills, ~20 missing widget instances, and per-panel D-006 first-Cycle-init wiring with synchronous mutator fires per D-007/D-008/D-009 — so the Rust port observably matches the C++ subscribe topology in `src/emStocks/`.

**Architecture:** Phased bottom-up. Accessors first (G1/G2/G3/G4 + delegating G5/G6) using the **D-008 A1 combined-form** (`fn GetXxxSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` with `Cell<SignalId>` lazy alloc); mutator fires threaded with `&mut impl SignalCtx` (D-007 broadened). Widget-instance gaps next (G8: 20 ControlPanel buttons + the SelectedDate text field). Per-panel Cycle wiring (D-006) last, one panel per phase, with a two-tier `subscribed_init` + `subscribed_widgets` flag for AutoExpand-gated widgets. No polling intermediaries (D-009).

**Tech Stack:** Rust, `emcore::emEngineCtx::{SignalCtx, ConstructCtx, EngineCtx}`, `SignalId`, `cargo`, `cargo-nextest`. Reference: Eagle Mode 0.96.4 at `~/Projects/eaglemode-0.96.4/src/emStocks/`.

---

## Pre-flight

Before Phase 1:

- [ ] **Confirm working tree clean.** `git status` reports clean (or only this plan file).
- [ ] **Confirm baseline green.** `cargo check --workspace` succeeds; `cargo clippy -- -D warnings` succeeds; `cargo-nextest ntr` matches the recorded baseline (2897 per `project_f010_status_2026-04-25` memory; updated counts may apply — record the count seen at this gate).
- [ ] **Read the design doc** `docs/superpowers/specs/2026-04-27-B-001-no-wire-emstocks-design.md` end-to-end including the appended `## Adversarial Review — 2026-05-01` section.
- [ ] **Verify D-008 A1 combined form** is the canonical accessor signature by re-reading `decisions.md` §D-008 (the brainstorm-time `Ensure*Signal` split form is retired; the combined `fn GetXxxSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` form is what every accessor in this plan uses — this is the third-precedent-confirmed shape, do not re-derive the split).
- [ ] **Verify D-007 trait-bound** — mutator signatures use `&mut impl SignalCtx`, NOT `&mut EngineCtx<'_>` (broadened post-B-009 because `PanelBehavior::Input` only carries `PanelCtx`).
- [ ] **Verify per-bucket M-001 rule applied:** open every C++ `.cpp` in scope (`emStocksControlPanel.cpp`, `emStocksItemPanel.cpp`, `emStocksItemChart.cpp`, `emStocksFilePanel.cpp`, `emStocksListBox.cpp`, `emStocksFetchPricesDialog.cpp`) and `grep -n 'IsSignaled'` to count branches. Compare to the design doc's per-panel table. Record any divergence as a design-doc errata before starting.

LLM-failure-mode guards (CLAUDE.md "Plan Tool Rules"):

- **No `#[allow(...)]` / `#[expect(...)]`.** Fix warnings at the cause. Allowed exceptions: `non_snake_case` on the `emCore` module / `em`-prefixed types, `too_many_arguments`. Anything else is a bug.
- **No `f64`** anywhere in blend / coverage / interpolation paths. (Not expected to be touched by this bucket; if a touched call site introduces it, stop and re-scope.)
- **No polling intermediaries (D-009).** Do not introduce a `Cell`/`RefCell` field set in site A and drained by site B's `Cycle`. Thread `ectx` to site A and fire synchronously per D-007.
- **File and Name Correspondence.** Every method name matches the C++ name (`GetChangeSignal`, `GetSelectedDateSignal`, …) — no silent rename. Forced renames carry `DIVERGED:` with category. New Rust-only fields carry `RUST_ONLY:` with charter.
- **Annotation hygiene.** If any `DIVERGED:`/`RUST_ONLY:` block is added, run `cargo xtask annotations` before committing.
- **TDD where applicable.** Behavioral tests at the bucket level live in `crates/emstocks/tests/typed_subscribe_b001.rs`. Each Phase 4 sub-task adds the test before the wiring.
- **Pre-commit hook is the source of truth per commit.** It runs `cargo fmt` (auto-applied), `cargo clippy -D warnings`, then `cargo-nextest ntr`. Per-task `cargo-nextest ntr` is skipped (per `feedback_skip_nextest_per_task` memory); selective `nextest run -p emstocks` is the per-task gate. Full nextest runs at Phase 5 final gate.

---

## Phase 1 — Accessor-side gap fills (G1, G2, G3, G4 + G5/G6 delegators)

**Rows in scope (5 rows + 2 delegators):**

- `emStocksFileModel-accessor-model-change` (G1 — delegating)
- `emStocksConfig-accessor-config-change` (G2 — direct field, design Option B)
- `emStocksPricesFetcher-accessor-model-change` (G3 — direct field; consumer is in B-017)
- `emStocksListBox::GetSelectedDateSignal` accessor (G4 — direct field on ListBox; underpins 4 consumer rows in later phases)
- `emStocksListBox::GetSelectionSignal` (G5 — delegating to inner `Option<emListBox>`)
- `emStocksListBox::GetItemTriggerSignal` (G6 — delegating to inner `Option<emListBox>`)

Pre-checks:

- [ ] Confirm `crates/emstocks/src/emStocksFileModel.rs:14` is the `pub struct emStocksFileModel { file_model: emRecFileModel<emStocksRec>, … }` — the embedded `emFileModel::change_signal` and `GetChangeSignal()` accessor (`crates/emcore/src/emFileModel.rs:64`) is the delegate target.
- [ ] Confirm `crates/emstocks/src/emStocksConfig.rs:128-146` is the plain `struct emStocksConfig` with `Default`/`Clone` and no SignalId field. The Option B fix adds the field directly; do NOT compose `emConfigModel`.
- [ ] Confirm `crates/emstocks/src/emStocksPricesFetcher.rs:19` is the bare struct; ctor at line 39 takes only string args.
- [ ] Confirm `crates/emstocks/src/emStocksListBox.rs:20-90` shows the `pub struct emStocksListBox` with no `selected_date_signal` and `pub fn new() -> Self` taking no args.
- [ ] Re-read `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksPricesFetcher.cpp:70,134,264,272` — confirm 4 `Signal(ChangeSignal)` callsites for G3 mutator-fire wiring.

### Task 1.1 — G1 delegating accessor on `emStocksFileModel`

**Files:**
- Modify: `crates/emstocks/src/emStocksFileModel.rs` (after `pub fn new` block, near line 28; place before `OnRecChanged` or with the other delegate methods)
- Test: `crates/emstocks/src/emStocksFileModel.rs` (existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test.**

```rust
#[test]
fn get_change_signal_delegates_to_inner_file_model() {
    let model = emStocksFileModel::new(PathBuf::from("/tmp/g1.emStocks"));
    let sig_a = model.GetChangeSignal();
    let sig_b = model.GetChangeSignal();
    assert_eq!(sig_a, sig_b, "GetChangeSignal must be stable across calls");
}
```

- [ ] **Step 2: Run test, confirm it fails to compile (`GetChangeSignal` not found).**

Run: `cargo test -p emstocks --lib emStocksFileModel::tests::get_change_signal_delegates_to_inner_file_model`
Expected: `error[E0599]: no method named GetChangeSignal`.

- [ ] **Step 3: Add the delegating accessor.**

```rust
/// Port of inherited C++ `emFileModel::GetChangeSignal`. Delegates to the
/// composed `emRecFileModel<emStocksRec>` (which inherits from `emFileModel`).
pub fn GetChangeSignal(&self) -> SignalId {
    self.file_model.GetChangeSignal()
}
```

Add `use emcore::emEngineCtx::SignalId;` if not already imported.

- [ ] **Step 4: Run test, confirm it passes.**

Run: `cargo test -p emstocks --lib get_change_signal_delegates_to_inner_file_model`
Expected: 1 passed.

- [ ] **Step 5: Commit.**

```bash
git add crates/emstocks/src/emStocksFileModel.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001 G1): expose GetChangeSignal delegating accessor

Mirrors inherited C++ emFileModel::GetChangeSignal on emStocksFileModel
so consumers in B-001 can subscribe.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Task 1.2 — G2 direct SignalId on `emStocksConfig` (Option B)

**Files:**
- Modify: `crates/emstocks/src/emStocksConfig.rs:128-160` (struct, `Default`, accessor, mutator)
- Modify: `crates/emstocks/src/emStocksControlPanel.rs:409` (`ReadFromWidgets`) — call `Signal(ectx)` after every successful field write where the value actually changed; signature gains `&mut impl SignalCtx`.
- Modify: every other site that mutates config (search via `rg -n '\.api_key =|\.search_text =|\.auto_update_dates =|\.triggering_opens_web_page =|\.chart_period =|\.min_visible_interest =|\.sorting =|\.owned_shares_first =|\.web_browser =|\.api_script =' crates/emstocks/`).
- Test: new file `crates/emstocks/tests/typed_subscribe_b001.rs` (created in Task 4.0).

Add a field; do NOT compose `emConfigModel` (broader blast radius rejected in design §G2).

- [ ] **Step 1: Write failing test (held until Task 4.0 file exists).** Skip, return to in Phase 4.

- [ ] **Step 2: Add SignalId field to `emStocksConfig`.**

```rust
pub struct emStocksConfig {
    // … existing fields unchanged …
    pub search_text: String,
    /// Lazy-allocated per D-008 A1. Null until first subscriber requests it.
    /// RUST_ONLY: language-forced — C++ `emStocksConfig : public emConfigModel`
    /// inherits this; Rust composes via direct field per design Option B
    /// (composing `emConfigModel` would force a re-architecture of every
    /// pass-by-value/`&` callsite in emstocks).
    change_signal: std::cell::Cell<emcore::emEngineCtx::SignalId>,
}
```

- [ ] **Step 3: Update `Default` to initialize `change_signal: Cell::new(SignalId::null())`.**

- [ ] **Step 4: Adjust derives.** `#[derive(Debug, Clone, PartialEq)]` becomes incompatible with `Cell<SignalId>` because `Cell<T>: !Sync` is fine but `PartialEq` does not derive through `Cell`. Hand-write `PartialEq` (compare every field except `change_signal`) and hand-write `Clone` (clone all fields, fresh `Cell::new(SignalId::null())` for the new instance — a clone must NOT share the parent's allocated SignalId; subscribers connect to the original instance only). Add `// DIVERGED: language-forced — C++ uses copy-construction inheriting emConfigModel; Rust Clone resets the SignalId because cloned values are independent broadcast endpoints.` immediately above the `impl Clone`.

- [ ] **Step 5: Add accessor (D-008 A1 combined form).**

```rust
/// Port of inherited C++ `emConfigModel::GetChangeSignal`.
/// Lazy-allocated on first subscribe per D-008 A1.
pub fn GetChangeSignal(&self, ectx: &mut impl emcore::emEngineCtx::SignalCtx) -> emcore::emEngineCtx::SignalId {
    let mut sig = self.change_signal.get();
    if sig.is_null() {
        sig = ectx.create_signal();
        self.change_signal.set(sig);
    }
    sig
}
```

- [ ] **Step 6: Add mutator-fire helper (D-007 — `&mut impl SignalCtx`).**

```rust
/// Port of inherited C++ `emConfigModel::Signal(ChangeSignal)`.
/// No-op when no subscriber has allocated the signal (matches C++
/// `emSignal::Signal()` with zero subscribers).
pub fn Signal(&self, ectx: &mut impl emcore::emEngineCtx::SignalCtx) {
    let sig = self.change_signal.get();
    if !sig.is_null() {
        ectx.fire(sig);
    }
}
```

- [ ] **Step 7: Wire mutator-fire at every config-write callsite.**

Run: `rg -n 'config\.(api_key|search_text|auto_update_dates|triggering_opens_web_page|chart_period|min_visible_interest|visible_countries|visible_sectors|visible_collections|sorting|owned_shares_first|web_browser|api_script|api_script_interpreter)\s*=' crates/emstocks/`

For each hit that is a real mutation (not a destructure / test scaffold), gate on "did the field actually change?" and call `config.Signal(ectx)` once at end of the routine if any field changed. Specific known sites:
- `emStocksControlPanel::ReadFromWidgets` (line ~409) — gather a `dirty: bool` and Signal at end. Signature gains `ectx: &mut impl SignalCtx`.
- File-load path (search `TryLoad`/`load`/`Load` in config flow).

CALLSITE-NOTE: if any mutator callsite genuinely lacks `ectx` (e.g., bootstrap `Default`), leave it as a `// CALLSITE-NOTE:` per D-007 composition rule — D-008's null-fire-noop semantics make this benign at bootstrap (no subscribers yet).

- [ ] **Step 8: `cargo check -p emstocks`** — fix any callsites that newly require `ectx`. Expected: clean.

- [ ] **Step 9: `cargo clippy -p emstocks -- -D warnings`** — clean.

- [ ] **Step 10: Commit.**

```bash
git add crates/emstocks/src/emStocksConfig.rs crates/emstocks/src/emStocksControlPanel.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001 G2): emStocksConfig.GetChangeSignal + mutator fire

D-008 A1 lazy-allocated SignalId on the plain-struct emStocksConfig
(design Option B — composing emConfigModel rejected for ownership-blast-radius).
Mutator-fire threaded through ReadFromWidgets and config-load callsites
per D-007 (`&mut impl SignalCtx`). RUST_ONLY annotation on the Cell field.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Task 1.3 — G3 direct SignalId on `emStocksPricesFetcher`

**Files:**
- Modify: `crates/emstocks/src/emStocksPricesFetcher.rs:19-280` (struct field, ctor unchanged, accessor, 4 mutator-fire sites)

- [ ] **Step 1: Add field.**

```rust
// In `pub struct emStocksPricesFetcher` (line 19):
/// Lazy-allocated per D-008 A1. Null until first subscriber.
/// Mirrors C++ `emSignal ChangeSignal` (header line 103).
change_signal: std::cell::Cell<emcore::emEngineCtx::SignalId>,
```

- [ ] **Step 2: Initialize in `new` (line 39).**

```rust
change_signal: std::cell::Cell::new(emcore::emEngineCtx::SignalId::null()),
```

- [ ] **Step 3: Add accessor + Signal helper** (same shape as Task 1.2 Steps 5–6).

- [ ] **Step 4: Wire mutator-fire at the 4 C++ sites** (`emStocksPricesFetcher.cpp:70, 134, 264, 272`). Map each to its Rust analogue and call `self.Signal(ectx)`. Required signature change: `Cycle`, `StartProcess`, `PollProcess`, `SetFailed` (etc.) gain `ectx: &mut impl SignalCtx`. Cascade through callers — `emStocksFetchPricesDialog::Cycle` (line 91) currently takes no ectx; gains `&mut impl SignalCtx`. The Dialog's own callers must thread through similarly. CALLSITE-NOTE any ectx-less callsite (none expected for production paths).

- [ ] **Step 5: `cargo check -p emstocks`** — clean.

- [ ] **Step 6: Sanity unit test (in `emStocksPricesFetcher.rs` tests mod):** `GetChangeSignal` returns identical SignalId on second call.

- [ ] **Step 7: `cargo clippy -p emstocks -- -D warnings`** — clean.

- [ ] **Step 8: Commit.**

```bash
git add crates/emstocks/src/emStocksPricesFetcher.rs crates/emstocks/src/emStocksFetchPricesDialog.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001 G3): emStocksPricesFetcher.GetChangeSignal + 4 fire sites

D-008 A1 lazy SignalId; D-007 mutator-fire threaded through Cycle/
StartProcess/PollProcess/SetFailed mirroring cpp:70,134,264,272.
B-001 has no in-bucket consumer; B-017 row 1 (emStocksFetchPricesDialog-62)
is the consumer.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Task 1.4 — G4 `selected_date_signal` on `emStocksListBox`

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs:20-90` (struct + `new`)
- Modify: every site that writes `self.selected_date` (run `rg -n 'self\.selected_date\s*=' crates/emstocks/src/emStocksListBox.rs`)

- [ ] **Step 1: Add field.** `selected_date_signal: std::cell::Cell<SignalId>` initialized null.

- [ ] **Step 2: Accessor (D-008 A1 combined form):** `pub fn GetSelectedDateSignal(&self, ectx: &mut impl SignalCtx) -> SignalId`.

- [ ] **Step 3: Mutator helper:** `fn signal_selected_date(&self, ectx: &mut impl SignalCtx)` — fires when non-null.

- [ ] **Step 4: Wire fires.** Every assignment to `self.selected_date` is followed by `self.signal_selected_date(ectx)` IF the value actually changed (compare against prior). C++ `emStocksListBox::Cycle` and `SetSelectedDate` are the canonical sites. Cascade signature changes upward (`Cycle` already takes ConstructCtx; broaden to `SignalCtx` where needed — note `ConstructCtx` is for `create_signal` only; firing wants `SignalCtx` which is implemented by `EngineCtx` and `SchedCtx`).

- [ ] **Step 5: Sanity unit test** in `emStocksListBox.rs` tests mod — accessor stable, fires on change.

- [ ] **Step 6: `cargo check -p emstocks`; `cargo clippy -p emstocks -- -D warnings`** — clean.

- [ ] **Step 7: Commit.**

```bash
git add crates/emstocks/src/emStocksListBox.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001 G4): emStocksListBox.GetSelectedDateSignal + fire on change

Mirrors C++ emStocksListBox::SelectedDateSignal (header line 89).
Lazy SignalId per D-008 A1; fired only on observable selected_date change
to match C++ emSignal::Signal() semantics.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Task 1.5 — G5/G6 delegating accessors on `emStocksListBox`

**Files:** `crates/emstocks/src/emStocksListBox.rs` (after `attach_list_box`)

- [ ] **Step 1:** Add accessors:

```rust
/// Port of inherited C++ `emListBox::GetSelectionSignal`.
/// Returns `None` while the inner emListBox is unattached (lazy AutoExpand).
pub fn GetSelectionSignal(&self) -> Option<SignalId> {
    self.list_box.as_ref().map(|lb| lb.selection_signal)
}

/// Port of inherited C++ `emListBox::GetItemTriggerSignal`.
/// Returns `None` while the inner emListBox is unattached.
pub fn GetItemTriggerSignal(&self) -> Option<SignalId> {
    self.list_box.as_ref().map(|lb| lb.item_trigger_signal)
}
```

The `Option` wrapper is the deferred-attach handle for Phase 4's `subscribed_widgets` two-tier init — consumers must early-return the Cycle subscribe when `None`.

- [ ] **Step 2: Sanity tests** — return `None` pre-attach, `Some(_)` after `attach_list_box`.

- [ ] **Step 3: `cargo check`/`clippy` clean.**

- [ ] **Step 4: Commit.**

```bash
git add crates/emstocks/src/emStocksListBox.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001 G5+G6): delegating GetSelectionSignal/GetItemTriggerSignal

Option-wrapped delegators forward to the lazy-attached inner emListBox.
Consumers early-return the connect when None until attach lands.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Phase 1 Gate

- [ ] `cargo check --workspace` clean.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cargo nextest run -p emstocks` clean (pre-commit hook runs full nextest at commit).
- [ ] `cargo xtask annotations` clean — every new `RUST_ONLY:` / `DIVERGED:` block carries a category.
- [ ] No `#[allow]`/`#[expect]` introduced (allowed exception: existing `non_snake_case` on the module).

---

## Phase 2 — Widget-instance gaps on `ControlWidgets` (G8 — 20 buttons + 1 TextField)

**Rows in scope (21 rows):**

| Row | C++ widget | Rust field name (snake_case mirror) | C++ click vs text |
|---|---|---|---|
| -586 | `FetchSharePrices` | `fetch_share_prices: emButton` | click |
| -600 | `DeleteSharePrices` | `delete_share_prices: emButton` | click |
| -609 | `GoBackInHistory` | `go_back_in_history: emButton` | click |
| -618 | `GoForwardInHistory` | `go_forward_in_history: emButton` | click |
| -626 | `SelectedDate` | `selected_date_field: emTextField` (rename existing `selected_date: String` slot — see step note) | text |
| -650 | `NewStock` | `new_stock: emButton` | click |
| -658 | `CutStocks` | `cut_stocks: emButton` | click |
| -666 | `CopyStocks` | `copy_stocks: emButton` | click |
| -674 | `PasteStocks` | `paste_stocks: emButton` | click |
| -682 | `DeleteStocks` | `delete_stocks: emButton` | click |
| -690 | `SelectAll` | `select_all: emButton` | click |
| -698 | `ClearSelection` | `clear_selection: emButton` | click |
| -706 | `SetHighInterest` | `set_high_interest: emButton` | click |
| -714 | `SetMediumInterest` | `set_medium_interest: emButton` | click |
| -722 | `SetLowInterest` | `set_low_interest: emButton` | click |
| -730 | `ShowFirstWebPages` | `show_first_web_pages: emButton` | click |
| -738 | `ShowAllWebPages` | `show_all_web_pages: emButton` | click |
| -749 | `FindSelected` | `find_selected: emButton` | click |
| -764 | `FindNext` | `find_next: emButton` | click |
| -772 | `FindPrevious` | `find_previous: emButton` | click |

(Row -756 `SearchText` is the existing `widgets.search_text` text field; no instance gap.)

Pre-checks:

- [ ] `rg -n 'pub\(crate\) fetch_share_prices|pub\(crate\) cut_stocks\b|pub\(crate\) new_stock\b' crates/emstocks/src/emStocksControlPanel.rs` — expected: 0 hits (confirms widgets are missing).
- [ ] Open `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp:550-790` and confirm each C++ `new emButton(...)` constructor's caption / parent / description so the Rust instantiation labels match.
- [ ] Confirm the existing `pub(crate) selected_date: String` (line 200) is the C++ "SelectedDate" *display field*; it is the value-mirror, not the widget. Phase 2 adds `selected_date_field: emTextField` as the widget; the `String` stays as the cached display value populated in `UpdateControls` (line 572).

### Task 2.1 — Add 20 button fields + 1 TextField to `ControlWidgets`

**Files:** `crates/emstocks/src/emStocksControlPanel.rs:160-300`

- [ ] **Step 1:** Add 20 `pub(crate) <field>: emcore::emButton::emButton,` lines to `ControlWidgets` and the `selected_date_field: emcore::emTextField::emTextField,` line. Group by C++ source order to make audit easy.

- [ ] **Step 2:** Instantiate each in `ControlWidgets::new` (around line 275–290) using the C++ caption and parent panel. Match C++ default flags exactly. Where `emRadioButton` constructors take a group, wire them now (CutStocks/PasteStocks/etc. are plain `emButton`, not radio).

- [ ] **Step 3:** Where C++ initial-state matters (`GoBackInHistory` enabled = !history_empty), set the matching field in `UpdateControls` (existing method around line 565). The B-001 design lists `widgets.go_back_in_history_enabled` and `widgets.go_forward_in_history_enabled` as already present — preserve.

- [ ] **Step 4: `cargo check -p emstocks`** — fix any cascade. Expected: clean. NO subscribe wiring yet — that lands in Phase 4.

- [ ] **Step 5: `cargo clippy -p emstocks -- -D warnings`** — clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/emstocks/src/emStocksControlPanel.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001 G8): add 20 emButton fields + SelectedDate emTextField

Pre-condition for Phase 4 D-006 subscribe wiring. Mirrors C++ emStocksControlPanel
field set at cpp:550-790. No subscribe wiring in this commit (lands in Phase 4
to keep the diff reviewable).

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Phase 2 Gate

- [ ] `cargo check --workspace` clean.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cargo nextest run -p emstocks` — must pass; existing `auto_expand_shrink_cycle` test (line 769) and `ReadFromWidgets` tests must all still pass; if widget-init order changed any test default, fix the test (do NOT silence with `#[allow]`).

---

## Phase 3 — emStocksListBox holds FileModel/Config refs

Pre-condition for the ListBox half of Phase 4 (rows -51, -52, -53).

**Rows: 0** rows fully wired in this phase — but Phase 4 ListBox subscribe is blocked without these refs. Surface area is minimal; isolating it makes the Phase 4 ListBox commit smaller.

Pre-checks:

- [ ] `rg -n 'lb\.Cycle\(' crates/emstocks/src/emStocksFilePanel.rs` — find every callsite that passes `rec` and `config` per-call. After Phase 3, the per-call passing is preserved (still functional) but the ListBox additionally retains its own refs for first-Cycle subscribe.
- [ ] Confirm C++ `emStocksListBox.h` member declarations (`FileModel & FileModel; emStocksConfig & Config;`) — the Rust port mirrors via `Rc<RefCell<>>` per CLAUDE.md §Ownership rule (a) cross-Cycle reference. Justification comment required.

### Task 3.1 — Add `file_model_ref` / `config_ref` to `emStocksListBox`

**Files:** `crates/emstocks/src/emStocksListBox.rs:20-90` and `emStocksFilePanel.rs` attach site.

- [ ] **Step 1:** Add fields:

```rust
pub struct emStocksListBox {
    // … existing …
    /// Cross-Cycle reference per CLAUDE.md §Ownership (a) — mirrors C++
    /// emStocksListBox.h FileModel/Config member references; required so the
    /// ListBox's own Cycle can subscribe to FileModel/Config change signals
    /// without being passed them per-call.
    pub(crate) file_model_ref: Option<std::rc::Rc<std::cell::RefCell<crate::emStocksFileModel::emStocksFileModel>>>,
    pub(crate) config_ref: Option<std::rc::Rc<std::cell::RefCell<crate::emStocksConfig::emStocksConfig>>>,
    pub(crate) subscribed_init: bool,
}
```

- [ ] **Step 2:** Initialize to `None` / `false` in `new`.

- [ ] **Step 3:** Set them in `emStocksFilePanel` at the same site that calls `attach_list_box` (or expose `emStocksListBox::set_refs(file_model: Rc<…>, config: Rc<…>)`). The parent's existing `Rc<RefCell<>>` handles to model/config become the source. Confirm by reading `emStocksFilePanel.rs` around the attach site.

- [ ] **Step 4: `cargo check`/`clippy` clean.**

- [ ] **Step 5: Commit.**

```bash
git add crates/emstocks/src/emStocksListBox.rs crates/emstocks/src/emStocksFilePanel.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001): emStocksListBox holds FileModel/Config Rc refs

Mirrors C++ emStocksListBox member references (FileModel&, Config&).
Required for the Phase 4 D-006 subscribe inside ListBox::Cycle. Justified
per CLAUDE.md §Ownership (a) — cross-Cycle reference.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Phase 3 Gate

- [ ] `cargo check --workspace` / `clippy` clean.
- [ ] `cargo nextest run -p emstocks` — clean.

---

## Phase 4 — Per-panel D-006 first-Cycle subscribe wiring

This phase is mechanical D-006 application, one panel per task. Each task creates its block of behavioral tests in `crates/emstocks/tests/typed_subscribe_b001.rs` BEFORE the wiring (TDD).

**Two-tier init pattern** (B-001 reconciliation note):
- `subscribed_init: bool` — model/data signals (G1, G2, G3, G4, ListBox refs). Set true on first Cycle.
- `subscribed_widgets: bool` — widget signals on AutoExpand-gated widgets. Reset on AutoShrink. Set true on first Cycle after AutoExpand.

### Task 4.0 — Create test scaffold

**Files:** Create `crates/emstocks/tests/typed_subscribe_b001.rs`.

- [ ] **Step 1:** Create file with a `Harness` struct mirroring B-005's `crates/emfileman/tests/typed_subscribe_b005.rs` precedent (open and read it before authoring). Provide:
  - `Harness::new() -> Harness`
  - `h.create_control_panel() -> PanelId` returning the panel id
  - `h.fire(sig: SignalId)`
  - `h.run_cycle()`
  - `h.with_panel<P, R>(id, |p: &mut P| -> R) -> R` typed downcast

- [ ] **Step 2:** Add a placeholder smoke test that just creates the harness; cargo test should compile and pass.

- [ ] **Step 3: Commit (test scaffold only — no production wiring yet).**

### Task 4.1 — `emStocksControlPanel::Cycle` (37 rows)

**Rows:** -74, -75, -76, -77, -413, -427, -435, -448, -466, -557, -566, -586, -600, -609, -618, -626, -650, -658, -666, -674, -682, -690, -698, -706, -714, -722, -730, -738, -749, -756, -764, -772, -1014 (inner CategoryPanel), -1064 (FileFieldPanel TextField), -1072 (FileFieldPanel FSB selection), -1143 (inner CategoryPanel selection), -1144 (inner CategoryPanel FileModel-change).

Pre-checks:

- [ ] `cat ~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp` lines 87–230 (outer Cycle), 1009–1080 (inner CategoryPanel + FileFieldPanel Cycle). Note the per-`IsSignaled` body of each (M-001 rule). Capture in a scratchpad.
- [ ] `rg -n 'fn Cycle' crates/emstocks/src/emStocksControlPanel.rs` — confirm 0 hits today; this task adds the first.

- [ ] **Step 1: Write failing tests first (one assertion per row).**

For each of the 37 rows, append a `#[test]` to `tests/typed_subscribe_b001.rs`. Pattern (using row -413 `api_key` as the template):

```rust
#[test]
fn b001_row_413_api_key_text_signal_marks_update_controls_needed() {
    let mut h = Harness::new();
    let panel = h.create_control_panel();
    h.run_cycle(); // first Cycle: subscribed_init flips on; widgets connected once AutoExpand
    h.expand(panel);
    h.run_cycle(); // subscribed_widgets flips on
    h.with_panel::<emStocksControlPanel, _>(panel, |p| {
        p.update_controls_needed = false;
        let sig = p.widgets.as_ref().unwrap().api_key.GetTextSignal();
        h.fire(sig);
    });
    h.run_cycle();
    h.with_panel::<emStocksControlPanel, _>(panel, |p| {
        assert!(p.update_controls_needed, "row -413 reaction missing");
    });
}
```

(`expand` and `with_panel` come from the harness in 4.0. `update_controls_needed` is the documented reaction per design §emStocksControlPanel.)

For model/data rows (-74, -75, -76, -77): fire `file_model.GetChangeSignal()` / `config.GetChangeSignal(&mut h.ectx())` / `list_box.GetSelectionSignal().unwrap()` / `list_box.GetSelectedDateSignal(&mut h.ectx())` and assert `update_controls_needed`.

For inner CategoryPanel and FileFieldPanel rows (-1014/-1064/-1072/-1143/-1144): fire the inner widget signal; assert the inner panel's reaction (e.g., its own `update_controls_needed` if it carries one — verify by reading C++).

- [ ] **Step 2: Run tests, confirm 37 fail with the panel having no Cycle.**

- [ ] **Step 3: Implement `emStocksControlPanel::Cycle` (D-006 shape).**

```rust
// At impl block of emStocksControlPanel:
pub(crate) subscribed_init: bool,        // add to struct
pub(crate) subscribed_widgets: bool,     // add to struct

fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, /* … existing args */) -> bool {
    // Tier 1 — model signals (always available)
    if !self.subscribed_init {
        let eid = ectx.id();
        ectx.connect(self.file_model.borrow().GetChangeSignal(), eid);
        let cfg_sig = self.config.borrow().GetChangeSignal(ectx);
        ectx.connect(cfg_sig, eid);
        let date_sig = self.list_box.borrow().GetSelectedDateSignal(ectx);
        ectx.connect(date_sig, eid);
        self.subscribed_init = true;
    }
    // Tier 2 — widget signals (lazy AutoExpand)
    if self.widgets.is_some() && !self.subscribed_widgets {
        let eid = ectx.id();
        if let Some(sel_sig) = self.list_box.borrow().GetSelectionSignal() {
            ectx.connect(sel_sig, eid);
        }
        let w = self.widgets.as_ref().unwrap();
        ectx.connect(w.api_key.GetTextSignal(), eid);
        ectx.connect(w.auto_update_dates.GetCheckSignal(), eid);
        ectx.connect(w.triggering_opens_web_page.GetCheckSignal(), eid);
        ectx.connect(w.chart_period.GetValueSignal(), eid);
        // … per-row connect calls in C++ source order (see design §emStocksControlPanel) …
        ectx.connect(w.find_previous.GetClickSignal(), eid);
        self.subscribed_widgets = true;
    }
    if self.widgets.is_none() {
        self.subscribed_widgets = false;  // reset on AutoShrink
    }

    // IsSignaled branches in C++ source order (cpp:97–130)
    if ectx.is_signaled(self.file_model.borrow().GetChangeSignal()) {
        self.update_controls_needed = true;
    }
    let cfg_sig = self.config.borrow().GetChangeSignal(ectx);
    if ectx.is_signaled(cfg_sig) {
        self.update_controls_needed = true;
    }
    if let Some(sel) = self.list_box.borrow().GetSelectionSignal() {
        if ectx.is_signaled(sel) {
            self.update_controls_needed = true;
        }
    }
    let date_sig = self.list_box.borrow().GetSelectedDateSignal(ectx);
    if ectx.is_signaled(date_sig) {
        self.update_controls_needed = true;
    }
    // For widget-signal IsSignaled branches: reaction is the C++ Cycle body's
    // per-widget mutator on Config (api_key/search_text/auto_update_dates/…)
    // followed by `config.borrow().Signal(ectx)` if any field actually changed.
    // ReadFromWidgets gains an `&mut impl SignalCtx` parameter and is called
    // here (mirrors C++ which has Config setters fire ChangeSignal).
    // …
    // Click-button branches: fire the corresponding ListBox method
    // (CutStocks/PasteStocks/SelectAll/etc.) which is already implemented.

    if self.update_controls_needed {
        // existing UpdateControls call site
    }
    false
}
```

**M-001 reminder:** open `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp:97-225` and confirm each `IsSignaled` branch's reaction body before writing it. Do not paraphrase from this plan; copy the C++ behavior.

- [ ] **Step 4: Run tests, confirm passing.**

- [ ] **Step 5: `cargo clippy -p emstocks -- -D warnings`.**

- [ ] **Step 6: Commit.**

```bash
git add crates/emstocks/src/emStocksControlPanel.rs crates/emstocks/tests/typed_subscribe_b001.rs
git commit -m "$(cat <<'EOF'
feat(emstocks,B-001): wire emStocksControlPanel::Cycle (37 rows)

D-006 two-tier init (subscribed_init for model signals + subscribed_widgets
for AutoExpand-gated widgets, reset on AutoShrink). IsSignaled branches in
C++ source order per emStocksControlPanel.cpp:97-225. ReadFromWidgets gains
&mut impl SignalCtx for the post-write Config::Signal fire (D-007).

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

### Task 4.2 — `emStocksItemPanel::Cycle` (25 rows + inner CategoryPanel 4 rows)

**Rows:** -74, -75, -342, -357, -364, -371, -395, -408 (per-WebPage loop), -415 (per-ShowWebPage loop), -421, -432, -441, -446, -451, -454, -467, -490 (per-Interest loop), -504, -509, -518, -527 — outer; -831, -832, -914, -922 — inner CategoryPanel.

Same shape as Task 4.1. Steps identical, scoped to `emStocksItemPanel.rs`. Inner CategoryPanel rows (-831 etc.) are wired via the inner panel's own `subscribed_init` flag with back-references to outer FileModel/Config (mirroring C++ structure per design §emStocksItemPanel — option (a)).

- [ ] **Step 1:** TDD-write 29 tests in `tests/typed_subscribe_b001.rs`.
- [ ] **Step 2:** Confirm 29 fail.
- [ ] **Step 3:** Implement two-tier Cycle on outer panel + inner CategoryPanel.
- [ ] **Step 4:** Tests green.
- [ ] **Step 5:** Clippy clean.
- [ ] **Step 6:** Commit (single commit; design §"design once apply mechanically" per D-004).

### Task 4.3 — `emStocksItemChart::Cycle` (2 rows: -64, -65)

Pre-check: `emStocksItemChart::new` currently takes no args (line 93). Either thread a `&mut C: ConstructCtx` (preferred — symmetric with other emstocks ctors) OR keep `new` argless and lazy-allocate the engine on first Cycle. Read C++ `emStocksItemChart.cpp:55-75` to confirm the engine arrival path; pick the smaller diff.

- [ ] **Step 1:** TDD: 2 tests asserting `UpdateData` is called when Config or SelectedDate fires.
- [ ] **Step 2:** Implement `Cycle` mirroring `cpp:86-100`.
- [ ] **Step 3-5:** Test/clippy/commit.

### Task 4.4 — `emStocksFilePanel::Cycle` integration (1 row: -255)

Pre-check: existing Cycle at `crates/emstocks/src/emStocksFilePanel.rs:349`. Adds `subscribed_init` field, connects `list_box.GetSelectedDateSignal(ectx)` (G4), reacts by triggering ItemChart `UpdateData`. The existing dialog-polling code stays.

- [ ] Steps as above.

### Task 4.5 — `emStocksListBox::Cycle` (3 rows: -51, -52, -53)

Pre-condition: Phase 3 landed `file_model_ref` / `config_ref`.

- [ ] **Step 1:** TDD: 3 tests — fire FileModel.ChangeSignal → ListBox re-sorts; fire Config.ChangeSignal → ListBox re-sorts; fire ListBox's own `item_trigger_signal` → ListBox activates current item.
- [ ] **Step 2:** Implement Cycle's first-block: connect via `self.file_model_ref.as_ref().expect(...).borrow().GetChangeSignal()`, `self.config_ref.as_ref().expect(...).borrow().GetChangeSignal(ectx)`, and the inner ListBox's `item_trigger_signal` (after attach). React per C++ `cpp:628-680`.
- [ ] **Step 3-5:** Test/clippy/commit.

### Task 4.6 — `emStocksFetchPricesDialog::Cycle` (already polls — confirm no row)

The bucket-sketch row table does not list a FetchPricesDialog row in B-001 (the consumer for G3 is in B-017 row 1). Skip; the accessor is now ready for B-017 to wire.

- [ ] Confirm: re-grep the bucket sketch table — no `emStocksFetchPricesDialog-*` row in B-001's row set. Confirmed; no work required.

### Phase 4 Gate

- [ ] All 71 tests in `tests/typed_subscribe_b001.rs` green.
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] `cargo xtask annotations` clean.

---

## Phase 5 — Final gate, no-`#[allow]` audit, and reconciliation

- [ ] **Step 1:** Run `cargo-nextest ntr` — full workspace, must pass.
- [ ] **Step 2:** `rg -nE '#\[allow|#\[expect' crates/emstocks/src/ crates/emstocks/tests/typed_subscribe_b001.rs` — expected output: nothing, or pre-existing allowed exceptions only (`non_snake_case` on the module). New `#[allow]` introduced by this bucket is a bug; fix at the cause.
- [ ] **Step 3:** `cargo xtask annotations` — every new `DIVERGED:`/`RUST_ONLY:` carries category.
- [ ] **Step 4:** `rg -n 'DUMP_DRAW_OPS|golden' crates/emstocks/tests` — confirm no golden test was inadvertently broken (this bucket should not change pixel output).
- [ ] **Step 5:** `scripts/verify_golden.sh --report` — golden-divergence table unchanged. (Sanity — paint paths untouched.)
- [ ] **Step 6:** Update `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/work-order.md` — flip B-001 status `designed → merged` with the merge-commit SHA.
- [ ] **Step 7:** Append a B-001 reconciliation entry to work-order.md noting:
  - Whether the `subscribed_widgets` two-tier pattern needs promotion to D-### (decide based on whether any Phase 4 sub-task hit a third instance of widget-attach drift beyond ControlPanel/ItemPanel/ListBox).
  - Whether any callsite genuinely lacked `ectx` (D-007 benign-hybrid sighting count).
  - Final row count: 71/71 wired.
- [ ] **Step 8:** Final commit:

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/work-order.md
git commit -m "$(cat <<'EOF'
docs(B-001): work-order reconciliation, B-001 designed → merged

71/71 emstocks P-001 rows wired; G3 PricesFetcher accessor in place for B-017.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Row accounting (71)

| Phase | Rows | Cumulative |
|---|---|---|
| 1 (accessors) | G1 + G2 + G3 + G4 + G5/G6 = 5 standalone accessor rows + 0 consumer rows | 5 |
| 2 (G8 widget add) | 21 widget-instance rows (-586..-772 incl. -626 textfield) | 26 |
| 3 (ListBox refs) | structural; 0 rows wired but blocks Phase 4.5 | 26 |
| 4.1 ControlPanel | 37 rows: -74,-75,-76,-77,-413,-427,-435,-448,-466,-557,-566,-1014,-1064,-1072,-1143,-1144 (16 distinct beyond G8 buttons) + the 21 G8 buttons whose Cycle subscribe lands here | 37 (the G8 widget-instance Phase 2 rows ARE the same row-IDs whose subscribe wiring lands in 4.1; double-counting avoided — count each row once) |
| 4.2 ItemPanel | 29 rows | 66 |
| 4.3 ItemChart | 2 rows | 68 |
| 4.4 FilePanel | 1 row (-255) | 69 |
| 4.5 ListBox | 3 rows (-51,-52,-53) | 72 → -1 (the 3 accessor-only rows in Phase 1 are already counted in the 71 total → Phase 1 contributes 3, not 5; G5/G6 are intra-G6 row -53 + consumer subscribes in 4.1/4.5, not standalone rows) → 71 |

The audit's 71 rows comprise: 37 ControlPanel + 29 ItemPanel (incl. 4 inner CategoryPanel) + 2 ItemChart + 1 FilePanel + 3 ListBox + 3 accessor-only (FileModel / Config / PricesFetcher) − 4 inner-CategoryPanel rows already in ItemPanel's 25 outer + 4 inner = 71. Confirmed against `buckets/B-001-no-wire-emstocks.md` table: 67 panel rows + 3 accessor-side + 1 emStocksFilePanel-255 = 71. All accounted for in Phases 1-4.

---

## Self-review

- **Spec coverage:** every group G1–G9 in the design doc maps to a phase/task. G7 (existing widget signals) lands in 4.1/4.2 alongside the G8 added widgets; G9 (FileFieldPanel inner widgets) lands in 4.1's inner-CategoryPanel/FileFieldPanel rows. Three accessor-only rows in Phase 1.
- **Type consistency:** every accessor signature is `fn GetXxxSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` (D-008 A1 combined form, third-precedent). Every mutator helper is `fn Signal(&self, ectx: &mut impl SignalCtx)`. `ConstructCtx::create_signal` is reserved for ctor-time allocation; this plan uses lazy allocation through `SignalCtx::create_signal` exclusively (D-008 A1).
- **No placeholders:** every step shows the code or a `rg`/`cargo` command with expected behavior. Test bodies are written, not described. M-001 is enforced as a pre-check on every Phase 4 task.
- **Failure-mode guards in pre-flight checklist** applied per CLAUDE.md "Plan Tool Rules".
