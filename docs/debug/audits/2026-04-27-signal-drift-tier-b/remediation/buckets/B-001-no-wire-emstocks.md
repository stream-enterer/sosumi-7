# B-001-no-wire-emstocks — P-001 — wire missing accessor + subscribe across emstocks

**Pattern:** P-001-no-subscribe-no-accessor
**Scope:** emstocks
**Row count:** 71
**Mechanical-vs-judgement:** balanced — wiring is mechanical once the accessor shape is decided; the accessor shape is a per-scope judgement call.
**Cited decisions:** D-003-gap-blocked-fill-vs-stub (gap-fill in scope), D-004-stocks-application-strategy (design-once / apply-mechanically), D-006-subscribe-shape (canonical wiring shape, with local two-tier `subscribed_init` + `subscribed_widgets` extension for lazy-attached widgets).
**Prereq buckets:** none.

**Reconciliation amendments (2026-04-27, post-design 456fa5f7):**
- **9 accessor groups (G1..G9).** Largest: G2 `Config.GetChangeSignal` (6 consumers), G1 `FileModel.GetChangeSignal` (4 consumers, delegating). Design organized by accessor group, not by panel.
- **Row classification refinements (no row moves):**
  - `emStocksListBox-53` (`GetItemTriggerSignal`): accessor inherited and present on `emListBox.item_trigger_signal` — shape-equivalent to P-002 but stays in B-001 (design-once unaffected).
  - 20 `emStocksControlPanel` rows + `-626`: drift includes "missing widget instance"; widget-add absorbed into bucket scope.
  - `emStocksFileModel-accessor-model-change`: delegating one-line accessor on composed `emRecFileModel<emStocksRec>`, not new SignalId allocation.
- **Coverage flag (G3 `PricesFetcher.GetChangeSignal`):** accessor ported per D-003 but no in-bucket consumer. If C++ has a missed `AddWakeUpSignal(...PricesFetcher.GetChangeSignal())` site, surface as a B-001 amendment.
- **Two-tier init pattern (local, not D-### worthy yet):** lazy-attached widgets and ListBox break the single `subscribed_init: bool` from D-006. Design uses `subscribed_init` for model signals + `subscribed_widgets` for AutoExpand-gated widget signals (reset on AutoShrink). If a second bucket rediscovers, promote to D-###.

## Pattern description

Rust path neither subscribes nor exposes the C++-side signal accessor. Both ends of the wire are missing, so the consumer has no way to react and the model has no way to broadcast. In this bucket the gap concentrates in emstocks panels (ControlPanel, ItemPanel, ItemChart, FilePanel, ListBox) plus three accessor-side rows on FileModel / Config / PricesFetcher whose absence blocks every consumer subscribe in the same scope.

## Rows

| ID | C++ site | Rust site | Accessor status | Notes |
|---|---|---|---|---|
| emStocksControlPanel-74 | src/emStocks/emStocksControlPanel.cpp:74 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No Cycle/cycle/on_cycle_ext, zero connect/IsSignaled — reaction surface absent |
| emStocksControlPanel-75 | src/emStocks/emStocksControlPanel.cpp:75 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No Cycle override and no connect for config change; Config field exists but no rail |
| emStocksControlPanel-76 | src/emStocks/emStocksControlPanel.cpp:76 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No Cycle override; no listbox-selection subscription |
| emStocksControlPanel-77 | src/emStocks/emStocksControlPanel.cpp:77 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Selected-date selection signal — no subscription, no reaction |
| emStocksControlPanel-413 | src/emStocks/emStocksControlPanel.cpp:413 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Widget GetTextSignal — 'other' / 'widget-text'; text-field value-changed broadcast |
| emStocksControlPanel-427 | src/emStocks/emStocksControlPanel.cpp:427 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No subscription to checkbox check signal |
| emStocksControlPanel-435 | src/emStocks/emStocksControlPanel.cpp:435 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No subscription to checkbox check signal |
| emStocksControlPanel-448 | src/emStocks/emStocksControlPanel.cpp:448 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Scalar field GetValueSignal — 'other' / 'widget-value' |
| emStocksControlPanel-466 | src/emStocks/emStocksControlPanel.cpp:466 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Interest-level radio buttons — 'stored for future signal wiring' (line 182) |
| emStocksControlPanel-557 | src/emStocks/emStocksControlPanel.cpp:557 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Sorting radio buttons; line 192: 'stored for future signal wiring' |
| emStocksControlPanel-566 | src/emStocks/emStocksControlPanel.cpp:566 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-586 | src/emStocks/emStocksControlPanel.cpp:586 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription; FilePanel spawns fetch_dialog elsewhere, ControlPanel rail absent |
| emStocksControlPanel-600 | src/emStocks/emStocksControlPanel.cpp:600 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-609 | src/emStocks/emStocksControlPanel.cpp:609 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-618 | src/emStocks/emStocksControlPanel.cpp:618 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-626 | src/emStocks/emStocksControlPanel.cpp:626 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Widget GetTextSignal — 'other' / 'widget-text' |
| emStocksControlPanel-650 | src/emStocks/emStocksControlPanel.cpp:650 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-658 | src/emStocks/emStocksControlPanel.cpp:658 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription; ListBox owns CutStocks dialog confirm via rc_cell_shim |
| emStocksControlPanel-666 | src/emStocks/emStocksControlPanel.cpp:666 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-674 | src/emStocks/emStocksControlPanel.cpp:674 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-682 | src/emStocks/emStocksControlPanel.cpp:682 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-690 | src/emStocks/emStocksControlPanel.cpp:690 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-698 | src/emStocks/emStocksControlPanel.cpp:698 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-706 | src/emStocks/emStocksControlPanel.cpp:706 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-714 | src/emStocks/emStocksControlPanel.cpp:714 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-722 | src/emStocks/emStocksControlPanel.cpp:722 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-730 | src/emStocks/emStocksControlPanel.cpp:730 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-738 | src/emStocks/emStocksControlPanel.cpp:738 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-749 | src/emStocks/emStocksControlPanel.cpp:749 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-756 | src/emStocks/emStocksControlPanel.cpp:756 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Widget GetTextSignal — 'other' / 'widget-text' |
| emStocksControlPanel-764 | src/emStocks/emStocksControlPanel.cpp:764 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-772 | src/emStocks/emStocksControlPanel.cpp:772 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | No click subscription |
| emStocksControlPanel-1014 | src/emStocks/emStocksControlPanel.cpp:1014 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Inner CategoryPanel ctor; Rust inner struct holds no Config ref / no subscribe |
| emStocksControlPanel-1064 | src/emStocks/emStocksControlPanel.cpp:1064 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Inner FileFieldPanel TextField text-changed — 'other' / 'widget-text'; no rail |
| emStocksControlPanel-1072 | src/emStocks/emStocksControlPanel.cpp:1072 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | FileSelectionBox-selection inside FileFieldPanel popup; no rail in Rust |
| emStocksControlPanel-1143 | src/emStocks/emStocksControlPanel.cpp:1143 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Inner CategoryPanel self-selection signal; no rail in Rust |
| emStocksControlPanel-1144 | src/emStocks/emStocksControlPanel.cpp:1144 | crates/emstocks/src/emStocksControlPanel.rs:387 | missing | Inner CategoryPanel subscribes outer FileModel-change in C++; no analogue in Rust |
| emStocksFilePanel-255 | src/emStocks/emStocksFilePanel.cpp:255 | crates/emstocks/src/emStocksFilePanel.rs:349 | missing | Cycle does not react to ListBox selected-date; only handles dialog polling |
| emStocksItemChart-64 | src/emStocks/emStocksItemChart.cpp:64 | crates/emstocks/src/emStocksItemChart.rs:91 | missing | No Cycle/cycle/connect; UpdateData called manually by parent; no rail |
| emStocksItemChart-65 | src/emStocks/emStocksItemChart.cpp:65 | crates/emstocks/src/emStocksItemChart.rs:91 | missing | ItemChart does not subscribe to listbox selected-date; reaction absent |
| emStocksItemPanel-74 | src/emStocks/emStocksItemPanel.cpp:74 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | No Cycle override and no connect calls; reaction surface absent |
| emStocksItemPanel-75 | src/emStocks/emStocksItemPanel.cpp:75 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Reaction absent |
| emStocksItemPanel-342 | src/emStocks/emStocksItemPanel.cpp:342 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-357 | src/emStocks/emStocksItemPanel.cpp:357 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-364 | src/emStocks/emStocksItemPanel.cpp:364 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-371 | src/emStocks/emStocksItemPanel.cpp:371 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-395 | src/emStocks/emStocksItemPanel.cpp:395 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-408 | src/emStocks/emStocksItemPanel.cpp:408 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Per-WebPage TextField change in a loop — 'other' / 'widget-text' |
| emStocksItemPanel-415 | src/emStocks/emStocksItemPanel.cpp:415 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Per-ShowWebPage button click in loop; line 109: 'stored for future signal wiring' |
| emStocksItemPanel-421 | src/emStocks/emStocksItemPanel.cpp:421 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | ShowAllWebPages button — 'stored for future signal wiring' (line 112) |
| emStocksItemPanel-432 | src/emStocks/emStocksItemPanel.cpp:432 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Checkbox check signal — absent |
| emStocksItemPanel-441 | src/emStocks/emStocksItemPanel.cpp:441 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-446 | src/emStocks/emStocksItemPanel.cpp:446 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-451 | src/emStocks/emStocksItemPanel.cpp:451 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-454 | src/emStocks/emStocksItemPanel.cpp:454 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Click subscription absent |
| emStocksItemPanel-467 | src/emStocks/emStocksItemPanel.cpp:467 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Line 82: 'FetchSharePrice button - stored for future signal wiring' |
| emStocksItemPanel-490 | src/emStocks/emStocksItemPanel.cpp:490 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Interest radio group — check signal absent |
| emStocksItemPanel-504 | src/emStocks/emStocksItemPanel.cpp:504 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-509 | src/emStocks/emStocksItemPanel.cpp:509 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-518 | src/emStocks/emStocksItemPanel.cpp:518 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Widget TextField change signal — 'other' / 'widget-text' |
| emStocksItemPanel-527 | src/emStocks/emStocksItemPanel.cpp:527 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Line 100: 'UpdateInquiryDate button - stored for future signal wiring' |
| emStocksItemPanel-831 | src/emStocks/emStocksItemPanel.cpp:831 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Inner CategoryPanel subscribes outer FileModel-change in C++; Rust holds no refs |
| emStocksItemPanel-832 | src/emStocks/emStocksItemPanel.cpp:832 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Inner CategoryPanel subscribes outer Config in C++; Rust does not |
| emStocksItemPanel-914 | src/emStocks/emStocksItemPanel.cpp:914 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Inner CategoryPanel TextField change — 'other' / 'widget-text' |
| emStocksItemPanel-922 | src/emStocks/emStocksItemPanel.cpp:922 | crates/emstocks/src/emStocksItemPanel.rs:204 | missing | Inner CategoryPanel ListBox-selection signal — absent in Rust |
| emStocksListBox-51 | src/emStocks/emStocksListBox.cpp:51 | crates/emstocks/src/emStocksListBox.rs:733 | missing | ListBox holds no FileModel ref; rec passed in by caller; no FileModel-change reaction |
| emStocksListBox-52 | src/emStocks/emStocksListBox.cpp:52 | crates/emstocks/src/emStocksListBox.rs:733 | missing | Config passed by reference per-call rather than held; no Config-change reaction |
| emStocksListBox-53 | src/emStocks/emStocksListBox.cpp:53 | crates/emstocks/src/emStocksListBox.rs:733 | missing | Inherited emListBox item-trigger — 'other' / 'item-trigger' (dblclick/Enter broadcast) |
| emStocksFileModel-accessor-model-change | (no C++ site) | crates/emstocks/src/emStocksFileModel.rs:14 | missing | Accessor-side gap: model-change SignalId accessor missing |
| emStocksConfig-accessor-config-change | (no C++ site) | crates/emstocks/src/emStocksConfig.rs:146 | missing | Accessor-side gap: config-change SignalId accessor missing |
| emStocksPricesFetcher-accessor-model-change | (no C++ site) | crates/emstocks/src/emStocksPricesFetcher.rs:18 | missing | Accessor-side gap: model-change SignalId accessor missing |

## C++ reference sites

- src/emStocks/emStocksControlPanel.cpp:74
- src/emStocks/emStocksControlPanel.cpp:75
- src/emStocks/emStocksControlPanel.cpp:76
- src/emStocks/emStocksControlPanel.cpp:77
- src/emStocks/emStocksControlPanel.cpp:413
- src/emStocks/emStocksControlPanel.cpp:427
- src/emStocks/emStocksControlPanel.cpp:435
- src/emStocks/emStocksControlPanel.cpp:448
- src/emStocks/emStocksControlPanel.cpp:466
- src/emStocks/emStocksControlPanel.cpp:557
- src/emStocks/emStocksControlPanel.cpp:566
- src/emStocks/emStocksControlPanel.cpp:586
- src/emStocks/emStocksControlPanel.cpp:600
- src/emStocks/emStocksControlPanel.cpp:609
- src/emStocks/emStocksControlPanel.cpp:618
- src/emStocks/emStocksControlPanel.cpp:626
- src/emStocks/emStocksControlPanel.cpp:650
- src/emStocks/emStocksControlPanel.cpp:658
- src/emStocks/emStocksControlPanel.cpp:666
- src/emStocks/emStocksControlPanel.cpp:674
- src/emStocks/emStocksControlPanel.cpp:682
- src/emStocks/emStocksControlPanel.cpp:690
- src/emStocks/emStocksControlPanel.cpp:698
- src/emStocks/emStocksControlPanel.cpp:706
- src/emStocks/emStocksControlPanel.cpp:714
- src/emStocks/emStocksControlPanel.cpp:722
- src/emStocks/emStocksControlPanel.cpp:730
- src/emStocks/emStocksControlPanel.cpp:738
- src/emStocks/emStocksControlPanel.cpp:749
- src/emStocks/emStocksControlPanel.cpp:756
- src/emStocks/emStocksControlPanel.cpp:764
- src/emStocks/emStocksControlPanel.cpp:772
- src/emStocks/emStocksControlPanel.cpp:1014
- src/emStocks/emStocksControlPanel.cpp:1064
- src/emStocks/emStocksControlPanel.cpp:1072
- src/emStocks/emStocksControlPanel.cpp:1143
- src/emStocks/emStocksControlPanel.cpp:1144
- src/emStocks/emStocksFilePanel.cpp:255
- src/emStocks/emStocksItemChart.cpp:64
- src/emStocks/emStocksItemChart.cpp:65
- src/emStocks/emStocksItemPanel.cpp:74
- src/emStocks/emStocksItemPanel.cpp:75
- src/emStocks/emStocksItemPanel.cpp:342
- src/emStocks/emStocksItemPanel.cpp:357
- src/emStocks/emStocksItemPanel.cpp:364
- src/emStocks/emStocksItemPanel.cpp:371
- src/emStocks/emStocksItemPanel.cpp:395
- src/emStocks/emStocksItemPanel.cpp:408
- src/emStocks/emStocksItemPanel.cpp:415
- src/emStocks/emStocksItemPanel.cpp:421
- src/emStocks/emStocksItemPanel.cpp:432
- src/emStocks/emStocksItemPanel.cpp:441
- src/emStocks/emStocksItemPanel.cpp:446
- src/emStocks/emStocksItemPanel.cpp:451
- src/emStocks/emStocksItemPanel.cpp:454
- src/emStocks/emStocksItemPanel.cpp:467
- src/emStocks/emStocksItemPanel.cpp:490
- src/emStocks/emStocksItemPanel.cpp:504
- src/emStocks/emStocksItemPanel.cpp:509
- src/emStocks/emStocksItemPanel.cpp:518
- src/emStocks/emStocksItemPanel.cpp:527
- src/emStocks/emStocksItemPanel.cpp:831
- src/emStocks/emStocksItemPanel.cpp:832
- src/emStocks/emStocksItemPanel.cpp:914
- src/emStocks/emStocksItemPanel.cpp:922
- src/emStocks/emStocksListBox.cpp:51
- src/emStocks/emStocksListBox.cpp:52
- src/emStocks/emStocksListBox.cpp:53

## Open questions for the bucket-design brainstorm

- Per D-003: are any of the three accessor-side gap rows (FileModel/Config/PricesFetcher) blocked by a missing *model* infrastructure rather than just a missing accessor on a ported model? If so, escalate — bucket cannot complete without out-of-scope porting.
- Per D-004: confirm "yes, mechanical application across all in-bucket rows is the intent" for the 71 emstocks rows; flag any sub-split that would break the design-once / apply-mechanically rule (e.g., whether the inner CategoryPanel / FileFieldPanel rows in ControlPanel and ItemPanel should be separated from outer-panel rows by enclosing scope).
- Should emStocksControlPanel-37 (the largest single-file slice) pilot the pattern fix and merge before the rest of the stocks rows in the same bucket land? PR-staging concern, not design concern.
- The three accessor-side gap rows likely need to land first as prereqs within this bucket — ordering inside the bucket: accessors before consumers, or single PR?
- Several rows reference inner panels (CategoryPanel inside ControlPanel/ItemPanel, FileFieldPanel) whose Rust structs do not currently hold the outer-model references the C++ versions subscribe to. Does the fix add those references, or restructure the inner-panel ownership?
- The "widget-text" / "widget-value" / "item-trigger" `signal_kind=other` rows fall outside §5.4 vocabulary — does the bucket-design need to define a wiring template for these widget-broadcast signals before mechanical application, or is the existing widget signal infrastructure sufficient?
