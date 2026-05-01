# B-001-no-wire-emstocks — Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Bucket:** `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/buckets/B-001-no-wire-emstocks.md`
**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** emstocks, 71 rows
**Mechanical-vs-judgement:** balanced — accessor groups are the judgement axis (5 model accessors + N widget signals); per-panel wiring is mechanical once accessors land.

## Goal and scope

Wire the missing P-001 sites across emstocks: both halves of the wire (model-side accessor + consumer-side subscribe). The bucket contains 71 rows clustered around three categories of accessor:

1. **Model/data accessors** — `emStocksFileModel::GetChangeSignal`, `emStocksConfig::GetChangeSignal`, `emStocksPricesFetcher::GetChangeSignal`, `emStocksListBox::GetSelectedDateSignal`, `emListBox::GetSelectionSignal` (already ported), `emListBox::GetItemTriggerSignal` (already ported).
2. **Widget signal accessors** — every widget kind used here (`emTextField::text_signal`, `emCheckBox::check_signal`, `emScalarField::value_signal`, `emButton::click_signal`, `emRadioButton::check_signal`) is already ported and accessible. The "missing accessor" tag in the audit row is, in these cases, an artifact of "the *widget instance* doesn't exist on the panel, so the signal isn't reachable." The fix is to add the widget field, not to add a new accessor.
3. **Cross-panel reaction wires** — ControlPanel/ItemPanel/ItemChart/FilePanel reacting to ListBox/FileModel/Config signals.

All 71 rows are wired in this bucket using **D-006-subscribe-shape** (first-Cycle init + IsSignaled top-of-Cycle). Three accessor-side gap-blocked rows are filled in scope per **D-003-gap-blocked-fill-vs-stub**. **D-004-stocks-application-strategy** — confirmed: design once, apply mechanically.

## Cited decisions

- **D-006-subscribe-shape** — canonical wiring pattern (subscribed_init flag + ectx.connect in first Cycle + IsSignaled at top).
- **D-003-gap-blocked-fill-vs-stub** — fill the three accessor-side gap rows (FileModel/Config/PricesFetcher) in this bucket; both halves live in emstocks scope.
- **D-004-stocks-application-strategy** — operationally a non-decision; this design is the canonical "design once" artifact.

## Audit-data anomalies (corrections)

The following audit rows are stale or mis-tagged. They remain in this bucket but the design records the correction so the working-memory session can patch `inventory-enriched.json`:

1. **emStocksFileModel-accessor-model-change** — tagged "accessor missing." The Rust `emStocksFileModel` composes `emRecFileModel<emStocksRec>` (line 21), which transitively contains `emFileModel::change_signal` with the `GetChangeSignal()` accessor at `emcore/src/emFileModel.rs:64`. The accessor *exists* on the embedded base; what's missing is a *delegating* accessor on `emStocksFileModel`. Fix is a one-line forward, not a new SignalId.

2. **emStocksConfig-accessor-config-change** — tagged "accessor missing." This is genuine: `emStocksConfig` is currently a plain data `struct` (not composed with `emConfigModel`). The C++ `emStocksConfig : public emConfigModel` inherits `GetChangeSignal`. Rust must either (a) compose with `emConfigModel`, or (b) add a `change_signal: SignalId` field directly. See decision below.

3. **emStocksPricesFetcher-accessor-model-change** — tagged "accessor missing." Genuine: the Rust struct has no SignalId field. Add a `change_signal: SignalId` field and `GetChangeSignal()` accessor.

4. **emStocksListBox-53** — `GetItemTriggerSignal` is *inherited*. The Rust `emStocksListBox` holds an `Option<emListBox>` (`list_box: Option<emListBox>`), and `emListBox.item_trigger_signal` is already a public `SignalId` (`emcore/src/emListBox.rs:312`). No new accessor needed — only a consumer subscribe.

5. **emStocksListBox-51 / -52** — tagged "ListBox holds no FileModel/Config ref." Confirmed by reading source: `emStocksListBox` does *not* hold `FileModel`/`Config` references; the parent `emStocksFilePanel` passes `rec` and `config` per-Cycle into `emStocksListBox::Cycle(ectx, rec, config)`. C++ `emStocksListBox` holds these as members and subscribes in its own Cycle. Rust ListBox cannot subscribe directly without holding refs. Two options: (a) add the refs and mirror C++, (b) move the subscription up to `emStocksFilePanel::Cycle` and have *it* react on ListBox's behalf. The design picks (a) below — see §"emStocksListBox" — because the C++ contract is "ListBox reacts to model/config changes by re-sorting visible items"; that reaction logically belongs in ListBox, and moving it changes structure.

6. **Subscriptions for action buttons that don't exist as Rust fields** — `emStocksControlPanel-650/-658/-666/.../-772` (NewStock, CutStocks, CopyStocks, PasteStocks, DeleteStocks, SelectAll, ClearSelection, SetHighInterest, SetMediumInterest, SetLowInterest, ShowFirstWebPages, ShowAllWebPages, FindSelected, FindNext, FindPrevious — 15 click rows) reference C++ `emButton` instances that do *not* currently exist as fields on Rust `ControlWidgets`. The audit's "missing subscribe" is downstream of "missing widget." Adding the subscribe requires first adding the `emButton` field. This is in-scope for the bucket — both halves are within the same panel — but the implementation step is "add widget field + subscribe," not just "subscribe."

These corrections do not move any rows out of B-001.

## Accessor groups

Group rows by the C++ signal they target. For each group: which model exposes the signal, what its Rust state is today, what fix the accessor needs, and which rows depend on it.

### G1 — `emStocksFileModel.GetChangeSignal()` (FileModel→change broadcast)

**C++ source.** Inherited from `emRecFileModel` / `emFileModel`. Fired when `emFileModel::Signal()` runs (i.e., on every `emFileModel::Save`/`Load`/explicit `Signal()` call).

**Rust state today.** Underlying SignalId exists at `emcore/src/emFileModel.rs:117` (`change_signal: SignalId`). Accessor `emFileModel::GetChangeSignal()` returns `SignalId` (line 64). `emStocksFileModel` composes `emRecFileModel<emStocksRec>` but exposes no delegating accessor.

**Fix.** Add a delegating accessor on `emStocksFileModel`. Per **D-008 A1 combined form** (third-precedent-confirmed at B-003 `eb9427db`, B-014 `c2871547`, B-009 `50994e26`), accessors take `&mut impl SignalCtx` and lazy-allocate via `Cell<SignalId>` on the underlying owner. For a delegating forward, the inner accessor performs the lazy alloc; the wrapper just forwards:

```rust
/// Port of inherited C++ emFileModel::GetChangeSignal.
pub fn GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
    self.file_model.GetChangeSignal(ectx)
}
```

(If `emFileModel::GetChangeSignal` is not yet on the D-008 A1 combined form, flip it in this bucket alongside the delegating add — its current `&self`-only signature is itself a B-001 fix.)

**Rows depending on G1 (consumer subscribes):**
- `emStocksControlPanel-74` (outer ControlPanel.Cycle)
- `emStocksControlPanel-1144` (inner CategoryPanel.Cycle subscribes outer FileModel)
- `emStocksItemPanel-831` (inner CategoryPanel.Cycle)
- `emStocksListBox-51` (ListBox.Cycle — but see §"emStocksListBox" — needs FileModel ref added)

### G2 — `emStocksConfig.GetChangeSignal()` (Config→change broadcast)

**C++ source.** Inherited from `emConfigModel`. Fired on `emConfigModel::Signal()` calls (e.g., when config setters mutate state).

**Rust state today.** `emcore::emConfigModel::GetChangeSignal()` returns `SignalId` (line 68). `emStocksConfig` is a plain struct — *does not* compose `emConfigModel`. There is no SignalId on it.

**Fix.** Two choices — design picks **(B)**:

- (A) Compose `emStocksConfig` with `emcore::emConfigModel::emConfigModel`. Mirrors C++ `class emStocksConfig : public emConfigModel`. Larger blast radius — the existing plain struct is read/written across the codebase as a value type with `Default` and `Clone`. Compositional ownership of an `emConfigModel` (which holds a SignalId, scheduler-bound) breaks the value-type usage.
- (B) Add a `change_signal: Cell<SignalId>` field directly to `emStocksConfig` (D-008 A1 lazy-alloc), plus a `Signal(&mut self, ectx: &mut impl SignalCtx)` mutator that fires it (D-007 post-B-009 amendment, `decisions.md:170` — broader bound than `&mut EngineCtx` because mutator may be reached from `PanelBehavior::Input` callsites that only carry `PanelCtx`). Add a `GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` accessor (D-008 A1 combined form) that lazy-allocates the SignalId on first call. Skips the full `emConfigModel` port (consistent with current Rust: `emStocksConfig` is config data, not a configmodel singleton).

  **Derive incompatibility (forced).** The existing `#[derive(Debug, Clone, PartialEq)]` (line 127) does not derive cleanly through `Cell<SignalId>`: `Cell<T>` deliberately omits `PartialEq` (interior mutability). Hand-write `Clone` (semantics: a cloned `emStocksConfig` is a distinct broadcast endpoint — `change_signal` resets to `Cell::new(SignalId::null())`) and hand-write `PartialEq` (excluding `change_signal` from the comparison). The hand-written `Clone` carries `// DIVERGED: language-forced — Rust Cell<SignalId> is not Clone-derivable; clone produces a fresh broadcast endpoint, mirroring C++ copy-construction of an `emConfigModel` member which would re-init its signal.`

**Why (B).** `emStocksConfig` is currently used as a plain Rust value passed by reference into Cycle methods (e.g., `lb.Cycle(ectx, rec, config)`). The C++ multi-inheritance of `emConfigModel` is itself a Rust language-forced divergence per the existing `emStocksFileModel` precedent (composition vs. MI). Going the full `emConfigModel`-composition path would force a re-architecting of every emstocks call-site that holds `emStocksConfig` by value or `&`. Option (B) is the smallest viable shape — adds the SignalId field plus accessor, keeps the value-type flow.

**Caveat.** Option (B) requires mutator sites (every `config = new_config` write in `ReadFromWidgets` / config-load code) to *also* call `config.Signal(ectx)` where `ectx: &mut impl SignalCtx` (D-007 post-B-009 amendment). The implementer must enumerate those sites — primarily in `emStocksControlPanel::ReadFromWidgets` and the file-load path — and add the fire. Some of those sites are reachable from `PanelBehavior::Input` (which carries `PanelCtx` only), so the broader `impl SignalCtx` bound is mandatory; a narrowed `&mut EngineCtx<'_>` would block compile mid-Phase-4. Without the fire, G2 subscribers will never wake.

**Rows depending on G2:**
- `emStocksControlPanel-75`, `-1014` (CategoryPanel inner Cycle subscribes Config)
- `emStocksItemPanel-74`, `-832`
- `emStocksItemChart-64`
- `emStocksListBox-52`

### G3 — `emStocksPricesFetcher.GetChangeSignal()`

**C++ source.** Owned: `emSignal ChangeSignal;` (`emStocksPricesFetcher.h:103`); accessor at line 66. Fired when fetch progresses or completes.

**Rust state today.** `emStocksPricesFetcher` (struct at `emStocksPricesFetcher.rs:18`) has no SignalId field, no accessor.

**Fix.** Add `change_signal: Cell<SignalId>` to the struct (D-008 A1 lazy-alloc — no constructor signature change required). Add `GetChangeSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` accessor (D-008 A1 combined form). Add a private `signal_change(&mut self, ectx: &mut impl SignalCtx)` (D-007 post-B-009 broader bound) and call it at every C++ `Signal(ChangeSignal)` site:

- `emStocksPricesFetcher.cpp:70` (StartProcess setup)
- `emStocksPricesFetcher.cpp:134` (PollProcess progress)
- `emStocksPricesFetcher.cpp:264` (SetFailed)
- `emStocksPricesFetcher.cpp:272` (HasFinished transition)

**Cascade — `Cycle` ectx threading.** Threading `&mut impl SignalCtx` into the four mutator sites forces `emStocksPricesFetcher::Cycle` to gain ectx, which in turn forces `emStocksFetchPricesDialog::Cycle` (currently `pub fn Cycle(&mut self) -> bool` at `emStocksFetchPricesDialog.rs:91`) to gain ectx, and every caller of the dialog's `Cycle` likewise. This cascade is in-scope for B-001 even though the FetchPricesDialog *consumer* row lives in B-017 — the cascade is structural, not optional.

**Rows depending on G3.** None of the 71 rows directly subscribe to G3 in this bucket; the consumer (FetchPricesDialog) is B-017 row 1 per the bucket-sketch reconciliation. Per D-003, land the accessor here so B-017 is unblocked.

### G4 — `emStocksListBox.GetSelectedDateSignal()`

**C++ source.** Owned: `emSignal SelectedDateSignal;` (`emStocksListBox.h:89`); accessor at line 42. Fired when the selected-date cursor changes.

**Rust state today.** `emStocksListBox` has no `selected_date_signal` field; mutating `selected_date: String` is unsignalled. The setter path needs auditing — search for writes to `self.selected_date`.

**Fix.** Add `selected_date_signal: Cell<SignalId>` to `emStocksListBox` (D-008 A1 lazy-alloc — no `new` signature change required). Add `GetSelectedDateSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` accessor (D-008 A1 combined form). Add a `signal_selected_date(&mut self, ectx: &mut impl SignalCtx)` helper (D-007 post-B-009 broader bound). Wire it at every `selected_date` mutation site.

**Rows depending on G4 (consumer subscribes):**
- `emStocksControlPanel-77`
- `emStocksFilePanel-255`
- `emStocksItemChart-65`
- `emStocksItemPanel-75`

### G5 — `emListBox::GetSelectionSignal()` (inherited via `emStocksListBox`)

**Rust state today.** `selection_signal: SignalId` exists on `emListBox` (line 310). `emStocksListBox` exposes the inner `Option<emListBox>` as `pub(crate) list_box`.

**Fix.** Add a delegating accessor on `emStocksListBox` in D-008 A1 combined form:

```rust
pub fn GetSelectionSignal(&self, ectx: &mut impl SignalCtx) -> Option<SignalId> {
    self.list_box.as_ref().map(|lb| lb.GetSelectionSignal(ectx))
}
```

The `Option` wrapper is necessary because the inner emListBox is lazy-attached. Consumer subscribers must early-return if `None`, or the panel must defer subscribe until ListBox is attached (see §"Sequencing" below). (If the inner `emListBox::GetSelectionSignal` is not yet on D-008 A1 form, flip it in this bucket — same pattern applies to `GetItemTriggerSignal` for G6.)

**Rows depending on G5:**
- `emStocksControlPanel-76`, `-1072` (FileSelectionBox-selection inside FileFieldPanel popup — different scope, see anomaly below), `-1143`
- `emStocksItemPanel-922`

### G6 — `emListBox::GetItemTriggerSignal()` (inherited via `emStocksListBox`)

Already accessible at `emListBox.item_trigger_signal`. Add delegating accessor analogous to G5. One row: `emStocksListBox-53`. Consumer is the parent `emStocksFilePanel` (or whatever houses the ListBox); the C++ call `AddWakeUpSignal(GetItemTriggerSignal())` is *self-subscribe* (ListBox subscribes to its own item-trigger, then in Cycle reacts e.g. by activating on Enter). Needs a Cycle on `emStocksListBox` that subscribes to its own signal and reacts; today `emStocksListBox::Cycle` exists but doesn't subscribe.

### G7 — Widget signals on widgets that *do* exist as panel fields

These rows subscribe to already-existing widget SignalIds. No accessor work; pure consumer wiring.

**ControlPanel (existing widgets):**
- `-413` `widgets.api_key.text_signal`
- `-427` `widgets.auto_update_dates.check_signal`
- `-435` `widgets.triggering_opens_web_page.check_signal`
- `-448` `widgets.chart_period.value_signal`
- `-466` per-button `widgets._min_visible_interest_buttons[i].check_signal` (3 buttons)
- `-557` per-button `widgets._sorting_buttons[i].check_signal` (11 buttons)
- `-566` `widgets.owned_shares_first.GetClickSignal(ectx)` — **definitive** per `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp:566`: C++ wires `OwnedSharesFirst->GetClickSignal()` (inherited from `emButton` via `emCheckButton`). Use `GetClickSignal`, NOT `GetCheckSignal`. Rust `emCheckBox` exposes both via inheritance; mirror the C++ contract exactly. Failure mode: `GetCheckSignal` may appear to work for keyboard-toggle but click-cycle observable behavior diverges.
- (`-626` is a G8 widget-add row — see §G8; not duplicated here.)
- `-756` `widgets.search_text.text_signal`

**ItemPanel (existing widgets):**
- `-342` `name.text_signal`, `-357` `symbol`, `-364` `wkn`, `-371` `isin`, `-395` `comment`
- `-432` `owning_shares.check_signal`
- `-441` `own_shares.text_signal`, `-446` `trade_price`, `-451` `trade_date`
- `-454` `update_trade_date.click_signal`
- `-490` per-button `_interest_buttons[i].check_signal` (3 buttons)
- `-504` `expected_dividend.text_signal`, `-509` `desired_price`, `-518` `inquiry_date`
- `-527` `_update_inquiry_date.click_signal`
- `-408` per-WebPage `web_pages[i].text_signal` (loop over NUM_WEB_PAGES)
- `-415` per-button `_show_web_page[i].click_signal`
- `-421` `_show_all_web_pages.click_signal`
- `-467` `_fetch_share_price.click_signal`
- `-914` inner CategoryPanel TextField — needs widget audit on `CategoryPanel`
- `-922` inner CategoryPanel ListBox-selection — covered by G5 if the inner ListBox is `emStocksListBox`-based, otherwise needs separate accessor (audit reading suggests it's the embedded `emListBox`)

### G8 — Widget signals on widgets that **do not exist as Rust fields today**

These rows look like consumer-only wiring in the audit, but in fact the widget itself is missing. The fix has two halves: (a) add the `emButton`/`emTextField` field to `ControlWidgets`, instantiate it in `ControlWidgets::new`; (b) subscribe to its `click_signal`/`text_signal` in Cycle and react.

**ControlPanel rows:**
- `-586` `FetchSharePrices` (button)
- `-600` `DeleteSharePrices`
- `-609` `GoBackInHistory`
- `-618` `GoForwardInHistory`
- `-626` `SelectedDate` (TextField — currently a `String`)
- `-650` `NewStock`
- `-658` `CutStocks`
- `-666` `CopyStocks`
- `-674` `PasteStocks`
- `-682` `DeleteStocks`
- `-690` `SelectAll`
- `-698` `ClearSelection`
- `-706` `SetHighInterest`
- `-714` `SetMediumInterest`
- `-722` `SetLowInterest`
- `-730` `ShowFirstWebPages`
- `-738` `ShowAllWebPages`
- `-749` `FindSelected`
- `-764` `FindNext`
- `-772` `FindPrevious`

This is 20 widget-add operations. Each is a small mechanical edit (add field, instantiate in `ControlWidgets::new`, subscribe in Cycle, react). The reaction targets a method that already exists on `emStocksListBox` (e.g., `CutStocks`, `PasteStocks`, `SelectAll`) or `emStocksControlPanel` itself.

### G9 — Inner `FileFieldPanel` widget signals

- `emStocksControlPanel-1064` (TextField text-changed inside FileFieldPanel popup)
- `emStocksControlPanel-1072` (FileSelectionBox-selection inside FileFieldPanel popup)

`FileFieldPanel` is a Rust-side helper struct in emstocks (verify location). Whether it exposes the inner widgets must be confirmed by reading. If the inner widgets are private/`_`-prefixed, this group needs widget exposure plus subscribe. Flag for the implementer.

## Per-panel consumer wiring

**M-001 enforcement.** Per `decisions.md` M-001, every panel's Cycle wiring task starts with a direct read of the C++ Cycle branch structure at the cited source line. Do not infer branch order or reaction targets from audit metadata or grep — open the cpp file, transcribe the branches, then port. Each Phase-4 sub-task in the plan begins with this M-001 pre-check.

For every panel below, follow the D-006 shape exactly:

```rust
pub struct PanelXYZ {
    // ... existing fields
    /// First-Cycle init flag for D-006-subscribe-shape.
    subscribed_init: bool,
}

fn Cycle(&mut self, ectx: &mut EngineCtx, pctx: &mut PanelCtx) -> bool {
    if !self.subscribed_init {
        let eid = ectx.id();
        // ectx.connect(...) for every reactive signal — see per-panel list below.
        self.subscribed_init = true;
    }
    // IsSignaled checks at top, in C++ source order.
    // ...
    false // or whatever the existing Cycle returned
}
```

### emStocksControlPanel (37 rows)

Outer panel. Subscribes to G1, G2, G5, G7 (existing widgets), G8 (added widgets). The Cycle body mirrors C++ `emStocksControlPanel.cpp:97–`.

Connect-list (in C++ source order; all accessor calls take `ectx: &mut impl SignalCtx`):
1. `self.file_model.borrow().GetChangeSignal(ectx)` — G1 row -74
2. `self.config.borrow().GetChangeSignal(ectx)` — G2 row -75
3. `self.list_box.GetSelectionSignal(ectx).expect(...)` — G5 row -76 (defer to second-Cycle if ListBox not yet attached; see §Sequencing)
4. `self.list_box.GetSelectedDateSignal(ectx)` — G4 row -77
5. (in `AutoExpand` path, after widgets exist) all G7/G8 widget signals enumerated above

Inner `ControlCategoryPanel` rows -1014 (Config), -1143 (self-selection), -1144 (outer FileModel-change) all live in the same inner Cycle; land together. -1143 and -1144 are paired wires to two model-side targets and react in the same Cycle pass (M-001: read C++ source for branch order before implementing).

IsSignaled branches (in C++ order, at top of Cycle). D-008 A1 accessors take `&mut impl SignalCtx`, so cache the SignalId to a local before the borrow-conflicting `ectx.IsSignaled` call:

```rust
let fm_sig = self.file_model.borrow().GetChangeSignal(ectx);
if ectx.IsSignaled(fm_sig) { update_controls_needed = true; }
let cfg_sig = self.config.borrow().GetChangeSignal(ectx);
if ectx.IsSignaled(cfg_sig) { update_controls_needed = true; }
let date_sig = self.list_box.GetSelectedDateSignal(ectx);
if ectx.IsSignaled(date_sig) { update_controls_needed = true; /* update SelectedDate display */ }
```

For each widget signal: corresponding mutator on Config or ListBox (mirror C++ `emStocksControlPanel::Cycle`). Mutator signatures take `&mut impl SignalCtx` per D-007 post-B-009 amendment.

Note the existing `update_controls_needed` flag matches C++'s `UpdateControlsNeeded` — reuse it.

### emStocksItemPanel (25 rows + 1 inner CategoryPanel)

Same shape. Connect-list:
1. `config.GetChangeSignal()` — row -74
2. `list_box.GetSelectedDateSignal()` — row -75
3. After AutoExpand — every widget signal in G7 list (12 rows of TextField/CheckBox/Button/RadioButton)

The inner `CategoryPanel` (rows -831, -832, -914, -922) is a sub-panel with its own Cycle; treat as a separate panel applying the same D-006 shape. Currently the Rust `CategoryPanel` (in `emStocksItemPanel.rs`, line 32) is a plain struct with no Cycle. Either (a) give it its own Cycle and a back-reference to outer FileModel/Config (mirror C++), or (b) move the subscribe up to `emStocksItemPanel::Cycle` and dispatch into the CategoryPanel's reactor. Design picks (a) for fidelity to C++ structure; flag for the implementer if back-reference plumbing is a structural problem.

**Disambiguation — two distinct CategoryPanel types.** `emStocksControlPanel.rs:71-76` documents `ControlCategoryPanel` as *a different type* from `emStocksItemPanel::CategoryPanel`, mirroring two separate C++ inner classes. ControlPanel rows -1014/-1143/-1144 target `ControlCategoryPanel`; ItemPanel rows -831/-832/-914/-922 target `emStocksItemPanel::CategoryPanel`. Two separate Cycle implementations are required; do not collapse.

### emStocksItemChart (2 rows: -64, -65)

`emStocksItemChart::new` currently takes no args and has no Cycle (`crates/emstocks/src/emStocksItemChart.rs:38-93`). The C++ chart is itself an `emPanel` with engine via base (`~/Projects/eaglemode-0.96.4/src/emStocks/emStocksItemChart.cpp:55-75`); the Rust port is a plain struct. Add:

- `subscribed_init: bool`
- A `Cycle(&mut self, ectx, ..., config: &emStocksConfig, list_box: &emStocksListBox)` method.

**Engine attachment pre-condition.** `ectx.connect(...)` is unreachable on a non-engine-bearing struct. Lift `emStocksItemChart` to engine-bearing before wiring (either by promoting it to a panel mirroring the C++ shape, or by adopting whatever engine-attachment lifecycle the surrounding emstocks code uses for sub-widgets). Without engine attachment, no `ectx.connect` is reachable and Phase 4 wiring will fail. Implementer triage required at task entry.

C++ Cycle body (cpp:93–94) is a straight `IsSignaled` OR-check that triggers `UpdateData()`. Mirror exactly.

### emStocksFilePanel (2 rows: -255 and the existing Cycle integration)

Existing `Cycle` (line 349). Add `subscribed_init` field; connect G4 (`list_box.GetSelectedDateSignal()` — row -255) when ListBox is attached. Note the panel already calls `lb.Cycle(...)` — keep that as the engine-driven cycle; the new top-of-Cycle `IsSignaled` branch reacts to the date change (e.g., trigger ItemChart UpdateData).

### emStocksListBox (3 rows: -51, -52, -53)

To subscribe to G1/G2 in its own Cycle, ListBox must hold refs to FileModel and Config. Today the parent passes them per-call. The design adds:
```rust
pub struct emStocksListBox {
    // ... existing
    file_model_ref: Option<Rc<RefCell<emStocksFileModel>>>, // (a) cross-Cycle reference, justified per CLAUDE.md §Ownership
    config_ref: Option<Rc<RefCell<emStocksConfig>>>,
    subscribed_init: bool,
}
```

The parent `emStocksFilePanel` sets these refs at attach time (alongside `attach_list_box`). After attach, `emStocksListBox::Cycle` performs the D-006 init/check pattern itself.

If this Rc<RefCell<>> addition pushes against the project's ownership defaults: the C++ original holds `FileModel` and `Config` as references in `emStocksListBox`; the Rust port currently routes around that by passing per-call. Since the C++ shape is observable (the ListBox subscribes from its own scope), preserving the C++ shape is design-intent per CLAUDE.md §"Port Ideology." Add the Rc<RefCell<>> with a `// (a) cross-Cycle reference per CLAUDE.md §Ownership` justification comment.

**Parent-side ownership shape pre-condition.** Before adding the refs to ListBox, verify `emStocksFilePanel`'s current ownership of `emStocksConfig`/`emStocksFileModel`. If the parent holds `emStocksConfig` by value or by `&` (not by `Rc<RefCell<>>`), the "set them at the same site that calls `attach_list_box`" step is non-trivial and may force a parent-side ownership refactor. Run `rg -n 'lb\.Cycle\(' crates/emstocks/src/emStocksFilePanel.rs` and inspect the surrounding scope before Phase 3. If the parent shape needs refactoring, surface it before starting the ListBox phase.

For row -53 (self-subscribe to `item_trigger_signal`), connect `self.list_box.as_ref().unwrap().item_trigger_signal` and react.

### emStocksFetchPricesDialog (1 row)

Single row: subscribes to PricesFetcher's G3. Implementer confirms target signal (likely `fetcher.GetChangeSignal()`) and reaction.

### Other tail rows

- `emStocksConfig-accessor` — accessor add only (G2 fix).
- `emStocksFileModel-accessor` — accessor add only (G1 fix).
- `emStocksPricesFetcher-accessor` — accessor add only (G3 fix).

## Sequencing

**Within the bucket:**

1. **Land accessor adds first** (G1, G2, G3, G4, G5/G6 delegating accessors). These are leaf changes — no consumers yet, no cycle changes. Safe to land independently. Tests: a unit-level "accessor returns same SignalId across calls" sanity check per accessor.
2. **Land widget adds (G8) inside ControlWidgets/ItemWidgets** — instantiate the missing buttons/textfields, no Cycle wiring yet. Pre-condition for the consumer wiring stage.
3. **Land per-panel Cycle wiring** — one PR per panel (ControlPanel, ItemPanel, ItemChart, FilePanel, ListBox, FetchPricesDialog). Each PR is the D-006 init block plus the IsSignaled reactions for that panel. Rows in the same panel can't be split without leaving an inconsistent intermediate state.
4. **Inner CategoryPanel wiring** lands in the same PR as its outer panel.

**Lazy-attached widgets / ListBox.** ControlPanel and ItemPanel use lazy AutoExpand: widgets are `None` until first expand. The first-Cycle init can't connect a `None` widget. Use one of two shapes:
- (Preferred) Move widget-signal connects into a separate `subscribed_widgets: bool` flag; reset to `false` on AutoShrink, run on the first Cycle after AutoExpand. Two-tier init: model-level signals on first Cycle (always), widget-level signals on first Cycle-after-AutoExpand.
- (Alternative) Always force AutoExpand at panel construction (eager). Larger memory footprint; unlikely acceptable.

**Not a DIVERGED block.** The two-tier flag is structurally faithful to C++: read `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp:380-790` — `AddWakeUpSignal` calls (cpp:413-772) live inside `AutoExpand` after widget construction, exactly mirroring the Rust `subscribed_widgets` reset/run pattern. This is preserved-design-intent + below-surface adaptation; the `subscribed_widgets` field is **NOT** annotated `DIVERGED:`. (Do not add a DIVERGED block here.)

The ListBox attach in `emStocksFilePanel` follows the same pattern: a `list_box_subscribed: bool` separate from `subscribed_init` allows attach-deferred subscribe.

**Cross-bucket prereqs.** None. P-001 in emstocks does not consume any P-003 type-mismatch accessor; G1–G6 are all `SignalId`-typed (or trivially adaptable). The bucket can land without waiting on any other bucket.

## Verification strategy

**Behavioral tests** (per D-006 / B-005 precedent), one new file per panel:
- `crates/emstocks/tests/typed_subscribe_b001.rs` — fires each subscribed signal, runs Cycle, asserts the documented reaction (config setter ran, ListBox sort flag flipped, ItemChart `UpdateData` invoked, etc.).

Per-row pattern:
```rust
let mut h = Harness::new();
let panel = h.create_control_panel();
h.fire(panel.widgets.as_ref().unwrap().auto_update_dates.check_signal);
h.run_cycle();
assert!(panel.update_controls_needed);
```

For accessor rows (G1/G2/G3/G4): assert that firing `model.signal_change()` propagates to a subscriber's Cycle.

**No new pixel-level golden tests.** The drift surface is signal flow, not paint. Existing emstocks goldens remain the regression backstop for paint output.

**Annotation checks.** Where G2 picks Option (B), if any DIVERGED-tagged code is added, run `cargo xtask annotations`.

## Open items deferred to working-memory session

1. **Reconcile audit-data corrections** into `inventory-enriched.json`:
   - Tag the 3 accessor-side rows (FileModel/Config/PricesFetcher) with the corrected nuance (FileModel is a delegating-accessor add, not a SignalId add).
   - Tag the 20 G8 rows with "missing-widget + missing-subscribe" rather than just "missing-subscribe" — the accessor *exists* on the widget type; the widget instance is what's missing.
   - `emStocksListBox-53` accessor exists (inherited); only consumer-side wiring needed.
2. **G3 consumer absence.** B-001 has no consumer of `PricesFetcher::GetChangeSignal`. Confirm with cpp grep of `AddWakeUpSignal(.*PricesFetcher`. If a consumer exists in C++ that the audit missed, escalate as a B-001 amendment.
3. **G2 design choice** — confirm Option (B) (add SignalId field directly to plain struct) is acceptable; the alternative (compose with `emConfigModel`) has wider blast radius beyond B-001. No new D-### needed; this is a per-bucket within-D-006 detail.
4. **emStocksListBox Rc<RefCell<>> additions** for FileModel/Config — flag in case the working-memory session wants to escalate to a global decision. The justification chain: C++ holds these as members; preserving observable signal-flow requires Rust to do the same.
5. **AutoExpand-deferred widget subscribe** is a local pattern (two-tier init flag). If multiple buckets rediscover it, may warrant promotion to D-007. Not proposing that here — single occurrence.

## Success criteria

- All 71 rows have a `connect(...)` call in their panel's first-Cycle init block (or a deferred-init equivalent for AutoExpand-gated widgets).
- All 71 rows have a corresponding `IsSignaled(...)` branch in the panel's Cycle body, in C++ source order.
- Three accessor-side rows have a Rust accessor matching the C++ contract (or its delegating equivalent for inherited signals).
- 20 G8 widget instances exist as fields on `ControlWidgets`.
- `cargo clippy -D warnings` and `cargo-nextest ntr` pass.
- New `tests/typed_subscribe_b001.rs` covers all 71 rows.
- B-001 status in `work-order.md` flips `pending → designed`.

---

## Adversarial Review — 2026-05-01

Review performed during plan-writing for `docs/superpowers/plans/2026-05-01-B-001-no-wire-emstocks.md`. Findings below cite real `file:line` and were verified by opening both Rust and C++ sources. Severity legend: **Critical** = breaks landing if not addressed before implementation; **Important** = will surface mid-flight as a B-005-style mistake unless preemptively patched; **Minor** = cosmetic / clarification.

### Critical

**C-1. D-008 A1 accessor signature in design doc is the retired split form, not the combined form.** §G2 shows a fix snippet (`pub fn GetChangeSignal(&self) -> SignalId`) and §G3 narrates "add `GetChangeSignal()` accessor" — both omit the `ectx: &mut impl SignalCtx` parameter. The combined form (`fn GetXxxSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` with `Cell<SignalId>` lazy alloc) is the third-precedent-confirmed shape per `decisions.md` D-008 and the work-order entries for B-003 (`eb9427db`), B-014 (`c2871547`), and B-009 (`50994e26`). The design's snippets, taken at face value, would produce a Rust compile error for the `&self`-only signature (no way to allocate). The plan supersedes with the combined form; design doc snippet is stale and should be patched or annotated as superseded.

**C-2. Mutator-fire bound is `&mut impl SignalCtx`, not `&mut EngineCtx`.** §G2 caveat says "every `config = new_config` write in `ReadFromWidgets` … must *also* call `config.Signal(ectx)`" but does not specify the bound. Per D-007 amendment (post-B-009 merge `50994e26`, `decisions.md:170`), mutator signatures use `&mut impl SignalCtx` because `PanelBehavior::Input` only carries `PanelCtx`. Some emstocks Input handlers may end up calling Config setters; if the design's narrowed `EngineCtx` bound is used, the implementer will hit a borrow/trait failure mid-Phase-4. Plan uses the broader bound throughout.

**C-3. Row -566 (`OwnedSharesFirst`) signal kind is `GetClickSignal`, not `GetCheckSignal`.** Confirmed at `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp:566` and the Rust D22 divergence at `crates/emstocks/src/emStocksControlPanel.rs:194-195` (`owned_shares_first: emCheckBox`). C++ wires `OwnedSharesFirst->GetClickSignal()` (inherited from `emButton` via `emCheckButton`). The design doc §G7 entry for `-566` says "C++ uses ClickSignal here on a checkbox; verify against C++" — verified: it IS `GetClickSignal`. Design entry is correctly suspicious but ambiguous — make it definitive: the connect call must be `widgets.owned_shares_first.GetClickSignal()`, not `GetCheckSignal()`. Rust `emCheckBox` exposes both via inheritance but the C++ contract is click; mirror exactly. (Failure mode: if implementer uses `GetCheckSignal`, the reaction may still work for keyboard-toggle but observable behavior on click cycle differs.)

### Important

**I-1. `emStocksItemChart::new` signature change blocks callers.** §emStocksItemChart says "`emStocksItemChart::new` currently takes no args and has no Cycle. Add … a `Cycle` method." But adding a Cycle that does `ectx.connect(...)` requires either (a) routing the chart through an engine attachment lifecycle, or (b) lazy `ConstructCtx`-time engine acquire. The design does not specify which. Read `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksItemChart.cpp:55-75` shows the C++ chart is itself an `emPanel` with engine via base — Rust `emStocksItemChart` at `crates/emstocks/src/emStocksItemChart.rs:38-93` is currently a plain struct, not a panel. The plan flags this for implementer triage in Task 4.3 but the design doc should explicitly note: "lift `emStocksItemChart` to engine-bearing if not already; without engine attachment, no `ectx.connect` is reachable."

**I-2. `emStocksFetchPricesDialog::Cycle` signature cascade is undocumented.** Threading G3 mutator-fire through `emStocksPricesFetcher::Cycle` (4 fire sites) cascades to `emStocksFetchPricesDialog::Cycle` (currently `pub fn Cycle(&mut self) -> bool`, line 91) which takes no `ectx`. Every caller of the dialog's `Cycle` then needs ectx too. Design §"emStocksFetchPricesDialog" says "Single row: subscribes to PricesFetcher's G3" — but B-001's row table does NOT contain a FetchPricesDialog row (it's in B-017). The sweep is structural: even without an in-bucket consumer, threading ectx into `emStocksPricesFetcher` mutators forces the dialog Cycle to gain ectx. Plan documents this in Phase 1 Task 1.3 but design doc misses the cascade.

**I-3. Inner CategoryPanel ownership in `emStocksControlPanel` is two distinct types.** `emStocksControlPanel.rs:71-76` documents `ControlCategoryPanel` is *a different type* from `emStocksItemPanel::CategoryPanel`. The design doc §emStocksItemPanel uses bare "CategoryPanel" without disambiguating. Rows -1014/-1143/-1144 in ControlPanel target `ControlCategoryPanel`; rows -831/-832/-914/-922 in ItemPanel target `emStocksItemPanel::CategoryPanel`. Two separate Cycle implementations are required (mirroring two C++ inner classes). Plan calls these out explicitly per task; design doc should add a one-line note.

**I-4. `emStocksConfig` `Clone`+`PartialEq` derives are incompatible with `Cell<SignalId>`.** §G2 Option B says "add a `change_signal: SignalId` field directly" — but the existing `#[derive(Debug, Clone, PartialEq)]` (line 127) does not derive through `Cell<SignalId>` cleanly: `Cell<T>` does not impl `PartialEq` (intentionally — interior mutability). Implementer must hand-write `Clone` (with semantics: clone resets to null per design — a cloned Config is a distinct broadcast endpoint) and hand-write `PartialEq` (excluding `change_signal`). The hand-written `Clone` is itself an annotation-required divergence (`DIVERGED: language-forced` — C++ copy-construction inheriting `emConfigModel` is well-defined; Rust must explicitly reset). Plan documents in Task 1.2 Step 4. Design doc is silent.

**I-5. ListBox `Rc<RefCell<emStocksConfig>>` may force callsite refactor.** §emStocksListBox says "C++ original holds `FileModel` and `Config` as references; Rust port currently routes around that by passing per-call." But the parent `emStocksFilePanel` may not currently hold `Rc<RefCell<emStocksConfig>>` — it may hold `emStocksConfig` by value or via a different shape. If the parent holds by value, the Phase 3 step "Set them in `emStocksFilePanel` at the same site that calls `attach_list_box`" is non-trivial. Plan flags as a Phase 3 pre-check (`rg -n 'lb\.Cycle\(' crates/emstocks/src/emStocksFilePanel.rs`); design doc should include the parent-side ownership shape verification as a §Sequencing pre-condition.

**I-6. `subscribed_widgets` reset on AutoShrink is not symmetric with C++.** Design §Sequencing says "reset to `false` on AutoShrink, run on the first Cycle after AutoExpand." C++ does not have an AutoShrink/AutoExpand divide for subscriptions — its `AutoExpand`/`AutoShrink` rebuild widgets, but `AddWakeUpSignal` is called inside `AutoExpand` after widget construction, mirroring the Rust `subscribed_widgets` reset. Verify: read `~/Projects/eaglemode-0.96.4/src/emStocks/emStocksControlPanel.cpp:380-790` (`AutoExpand`) and confirm widget signal subscribes happen there (they do, per the cpp:413-772 `AddWakeUpSignal` calls). The Rust two-tier flag is structurally faithful. Design's "DIVERGED candidate?" framing is unwarranted — this is preserved-design-intent + below-surface adaptation, no annotation required. Plan reflects this; design doc's "If multiple buckets rediscover, may warrant promotion to D-007" wording is fine but the framing implicitly invites a DIVERGED block which the implementer should NOT add.

### Minor

**M-1. Row -626 double role.** §G7 lists `-626 widgets.selected_date — but Rust has selected_date: String not a TextField; needs widget add` — and §G8 lists `-626 SelectedDate (TextField — currently a String)`. Same row in two groups. Plan resolves by treating it as a Phase 2 G8 widget-add row (Task 2.1). Design doc should pick one home.

**M-2. The 1144-vs-1143 pairing in `ControlCategoryPanel`.** Rows -1143 (self-selection) and -1144 (subscribes outer FileModel-change) both belong to inner `ControlCategoryPanel`. Both subscribe targets are model-side, both react in the same inner Cycle. Reasonable to land together; plan does so within Task 4.1.

**M-3. PricesFetcher ChangeSignal fires at 4 sites, not "every internal state transition."** §G3 says "call it from every internal state transition that the C++ original signals (consult `emStocksPricesFetcher.cpp` for `Signal(ChangeSignal)` call sites)." Confirmed: 4 sites at cpp:70 (StartProcess setup), :134 (PollProcess progress), :264 (SetFailed), :272 (HasFinished transition). Plan enumerates these in Task 1.3 Step 4; design doc could pre-list them.

**M-4. M-001 enforcement.** Design doc does not cite `decisions.md` M-001 (verify C++ Cycle branch structure directly). Every Phase 4 task in the plan adds an M-001 pre-check. Design should reference M-001 in §"Per-panel consumer wiring."

### Coverage check

All 71 rows accounted for in the bucket sketch table at `buckets/B-001-no-wire-emstocks.md` are mapped to a phase/task in the plan. No rows recommended for deferral or escalation. G3 (`emStocksPricesFetcher.GetChangeSignal`) consumer absence is already resolved per the bucket sketch reconciliation (B-017 row 1 is the consumer; landing the accessor here is correct and the sketch has been corrected).

### Recommendation

No rows to escalate or defer. Two design-doc patches suggested before implementation (post this review): (1) update G2/G3 accessor snippets to D-008 A1 combined form; (2) state row -566 signal kind explicitly as `GetClickSignal`. Plan is safe to execute as-is — it supersedes the stale snippets correctly.

---

## Amendment Log — 2026-05-01

This log records body-level amendments folding the Adversarial Review findings into the design itself. The Adversarial Review section above is preserved verbatim as audit trail; readers of the design body should now find corrected guidance inline without needing to cross-reference the review.

### Critical (3/3 resolved)

- **C-1 — D-008 A1 retired-split form.** Updated G1 snippet, G2 fix narration, G3 fix narration, G4 fix narration, and G5 snippet to D-008 A1 combined form (`fn GetXxxSignal(&self, ectx: &mut impl SignalCtx) -> SignalId` with `Cell<SignalId>` lazy alloc). G6 narration updated by reference within G5.
- **C-2 — Mutator-fire bound.** Replaced `&mut EngineCtx` with `&mut impl SignalCtx` in G2 fix narration, G2 caveat, G3 fix narration, G4 fix narration, and the §emStocksControlPanel IsSignaled snippet, citing D-007 post-B-009 amendment at `decisions.md:170`.
- **C-3 — Row -566 signal kind.** §G7 entry rewritten as definitive: `widgets.owned_shares_first.GetClickSignal(ectx)`, with C++ source citation (`emStocksControlPanel.cpp:566`) and explicit failure-mode warning against `GetCheckSignal`.

### Important (6/6 resolved)

- **I-1 — ItemChart engine attachment.** §emStocksItemChart amended with explicit "Engine attachment pre-condition" subsection: lift to engine-bearing before wiring, or `ectx.connect` is unreachable. Cites C++ panel base and current Rust plain-struct shape.
- **I-2 — FetchPricesDialog Cycle ectx cascade.** §G3 Fix amended with "Cascade — `Cycle` ectx threading" subsection: enumerates the cascade through `emStocksPricesFetcher::Cycle` to `emStocksFetchPricesDialog::Cycle:91` and its callers, marked structural in-scope for B-001 even though the consumer row is B-017.
- **I-3 — Two distinct CategoryPanel types.** §emStocksItemPanel amended with "Disambiguation" subsection naming `ControlCategoryPanel` (rows -1014/-1143/-1144) versus `emStocksItemPanel::CategoryPanel` (rows -831/-832/-914/-922) as separate types per `emStocksControlPanel.rs:71-76`.
- **I-4 — `Cell<SignalId>` Clone/PartialEq derive incompatibility.** §G2 Option (B) amended with "Derive incompatibility (forced)" paragraph: hand-written `Clone` (DIVERGED: language-forced; clone produces fresh broadcast endpoint) and hand-written `PartialEq` (excluding `change_signal`).
- **I-5 — ListBox parent ownership shape.** §emStocksListBox amended with "Parent-side ownership shape pre-condition" paragraph: requires verifying parent's current ownership of `emStocksConfig`/`emStocksFileModel` before Phase 3 with the documented `rg` pre-check.
- **I-6 — `subscribed_widgets` reset NOT DIVERGED.** §Sequencing amended with "Not a DIVERGED block" paragraph: explicit instruction NOT to annotate the two-tier flag, citing C++ AddWakeUpSignal sites at `emStocksControlPanel.cpp:380-790` (cpp:413-772 inside AutoExpand) as preserved-design-intent equivalence.

### Minor (4/4 resolved)

- **M-1 — Row -626 double-listing.** §G7 entry for -626 removed (note added pointing to G8). G8 retains -626 as canonical home (TextField widget-add).
- **M-2 — Paired -1143/-1144 wiring.** §emStocksControlPanel Connect-list amended to explicitly group `ControlCategoryPanel` rows -1014/-1143/-1144 in the same inner Cycle, with M-001 reference.
- **M-3 — PricesFetcher fire sites enumeration.** §G3 Fix amended to pre-list the four `Signal(ChangeSignal)` sites at `emStocksPricesFetcher.cpp:70/134/264/272`.
- **M-4 — M-001 reference.** §Per-panel consumer wiring prefaced with explicit M-001 enforcement paragraph requiring direct C++ Cycle-branch read before each panel task.

### Notes-only acknowledgements

None. All findings folded into the body. The Adversarial Review section retained verbatim as the audit trail for which findings were addressed and how.

### Dispatchability

Design body now matches the plan's superseding guidance throughout: D-008 A1 combined-form accessors, `&mut impl SignalCtx` mutator bound, definitive row -566 click-signal, ItemChart engine attachment pre-condition, FetchPricesDialog cascade, two distinct CategoryPanel types, `Cell<SignalId>` derive handling, ListBox parent ownership pre-check, and the `subscribed_widgets` non-DIVERGED instruction. Implementer reading the design body alone will receive corrected guidance without needing to cross-reference the Adversarial Review.
