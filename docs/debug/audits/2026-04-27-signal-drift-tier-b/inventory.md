# Signal-Drift Audit (Tier B) — Inventory

**Date:** 2026-04-27

This document is the rendered output of Tasks 1–7 of the Signal-Drift Tier-B Audit. The underlying methodology is defined in the [design spec](../../superpowers/specs/2026-04-27-signal-drift-tier-b-audit-design.md) and the [implementation plan](../../superpowers/plans/2026-04-27-signal-drift-tier-b-audit.md). All rows were generated mechanically from `inventory.json`; the preexisting-annotation revalidation results come from `preexisting-diverged.csv` (Task 6).

---

## Stats

| Metric | Value |
|--------|-------|
| Tier-B files | 34 |
| Total inventory rows | 212 |
| Panel rows | 194 |
| Model-internal rows | 7 |
| Cross-link model rows | 11 |
| Faithful | 8 (3.8%) |
| Drifted | 162 (76.4%) |
| Gap-blocked | 16 (7.5%) |
| Forced | 0 (0.0%) |
| Unported | 26 (12.3%) |
| Preexisting signal-related annotations | 15 |
| — verified | 6 |
| — reclassified to drifted (failed) | 8 |
| — wrong category | 1 |

---

## 1. Summary Table

Sorted descending by `drifted`.

| File | Total | Faithful | Drifted | Forced | Gap-blocked | Unported |
|------|-------|----------|---------|--------|-------------|----------|
| `emStocksControlPanel.rs` | 37 | 0 | 37 | 0 | 0 | 0 |
| `emStocksItemPanel.rs` | 25 | 0 | 25 | 0 | 0 | 0 |
| `emFileManControlPanel.rs` | 23 | 0 | 23 | 0 | 0 | 0 |
| `emCoreConfigPanel.rs` | 9 | 0 | 9 | 0 | 0 | 0 |
| `emAutoplay.rs` | 11 | 0 | 9 | 0 | 2 | 0 |
| `emMainControlPanel.rs` | 10 | 0 | 9 | 0 | 1 | 0 |
| `emColorField.rs` | 8 | 0 | 8 | 0 | 0 | 0 |
| `emStocksListBox.rs` | 7 | 0 | 7 | 0 | 0 | 0 |
| `emFileSelectionBox.rs` | 7 | 0 | 6 | 0 | 1 | 0 |
| `emDirPanel.rs` | 3 | 0 | 3 | 0 | 0 | 0 |
| `emFileLinkPanel.rs` | 5 | 0 | 3 | 0 | 2 | 0 |
| `emImageFile.rs` | 2 | 0 | 2 | 0 | 0 | 0 |
| `emDirEntryAltPanel.rs` | 2 | 0 | 2 | 0 | 0 | 0 |
| `emDirEntryPanel.rs` | 2 | 0 | 2 | 0 | 0 | 0 |
| `emDirStatPanel.rs` | 2 | 0 | 2 | 0 | 0 | 0 |
| `emMainPanel.rs` | 3 | 0 | 2 | 0 | 1 | 0 |
| `emVirtualCosmos.rs` | 3 | 0 | 2 | 0 | 1 | 0 |
| `emStocksFilePanel.rs` | 2 | 0 | 2 | 0 | 0 | 0 |
| `emStocksItemChart.rs` | 2 | 0 | 2 | 0 | 0 | 0 |
| `emFilePanel.rs` | 2 | 0 | 1 | 0 | 1 | 0 |
| `emFileManSelInfoPanel.rs` | 1 | 0 | 1 | 0 | 0 | 0 |
| `emBookmarks.rs` | 22 | 0 | 1 | 0 | 0 | 21 |
| `emFileDialog.rs` | 2 | 1 | 1 | 0 | 0 | 0 |
| `emStocksFetchPricesDialog.rs` | 1 | 0 | 1 | 0 | 0 | 0 |
| `emFileModel.rs` | 1 | 0 | 1 | 0 | 0 | 0 |
| `emStocksFileModel.rs` | 2 | 0 | 1 | 0 | 1 | 0 |
| `emDialog.rs` | 1 | 1 | 0 | 0 | 0 | 0 |
| `emMiniIpc.rs` | 1 | 1 | 0 | 0 | 0 | 0 |
| `emWindowStateSaver.rs` | 3 | 3 | 0 | 0 | 0 | 0 |
| `emMainWindow.rs` | 2 | 2 | 0 | 0 | 0 | 0 |
| `emConfigModel.rs` | 1 | 0 | 0 | 0 | 0 | 1 |
| `emFileManModel.rs` | 3 | 0 | 0 | 0 | 2 | 1 |
| `emFileManViewConfig.rs` | 2 | 0 | 0 | 0 | 1 | 1 |
| `emStocksPricesFetcher.rs` | 3 | 0 | 0 | 0 | 1 | 2 |

## 2. Drifted-Row Drill-Down

162 drifted rows across all files.

| ID | Signal | Evidence kind | Evidence location | Notes (excerpt) |
|----|--------|---------------|-------------------|-----------------|
| `emColorField-245` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:277` | emColorField::Cycle (Rust line 271) compares cached `sf_*` / `tf_name` fields ag… |
| `emColorField-255` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:277` | Green ScalarField. Same polling-based Cycle as emColorField-245. |
| `emColorField-265` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:277` | Blue ScalarField. Same polling-based Cycle. |
| `emColorField-277` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:277` | Alpha ScalarField. Same polling-based Cycle. |
| `emColorField-288` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:282` | Hue ScalarField. Same polling-based Cycle. |
| `emColorField-298` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:282` | Saturation ScalarField. Same polling-based Cycle. |
| `emColorField-308` | `sf->GetValueSignal()` | `polling` | `emColorField.rs:282` | Value (brightness) ScalarField. Same polling-based Cycle. |
| `emColorField-320` | `tf->GetTextSignal()` | `polling` | `emColorField.rs:285` | Name TextField. Polled via `sync_from_children` (line 342) which calls `tfp.text… |
| `emCoreConfigPanel-80` | `ResetButton->GetClickSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:1539` | Reset button. No Cycle/IsSignaled — substitute is a synchronous closure callback… |
| `emCoreConfigPanel-299` | `StickBox->GetCheckSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:341` | StickBox. on_check closure substitutes for AddWakeUpSignal+IsSignaled. No MouseM… |
| `emCoreConfigPanel-300` | `EmuBox->GetCheckSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:363` | EmuBox. Same on_check closure pattern. |
| `emCoreConfigPanel-301` | `PanBox->GetCheckSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:378` | PanBox. Same on_check closure pattern. |
| `emCoreConfigPanel-563` | `MemField->GetValueSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:801` | MemField scalar (max megabytes per view). on_value closure pattern. |
| `emCoreConfigPanel-746` | `MaxRenderThreadsField->GetValueSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:1039` | MaxRenderThreads. on_value closure pattern. |
| `emCoreConfigPanel-755` | `AllowSIMDBox->GetCheckSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:1066` | AllowSIMD. on_check closure pattern. |
| `emCoreConfigPanel-773` | `DownscaleQualityField->GetValueSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:1205` | DownscaleQuality. on_value closure pattern. |
| `emCoreConfigPanel-791` | `UpscaleQualityField->GetValueSignal()` | `rc_cell_shim` | `emCoreConfigPanel.rs:1233` | UpscaleQuality. on_value closure pattern. |
| `emFileDialog-196` | `OverwriteDialog->GetFinishSignal()` | `connect_call` | `emFileDialog.rs:516` | IsSignaled site: crates/emcore/src/emFileDialog.rs:169 `if ctx.IsSignaled(od_sig… |
| `emFilePanel-50` | `fileModel->GetFileStateSignal()` | `polling` | `emFilePanel.rs:138` | C++ AddWakeUpSignal/RemoveWakeUpSignal pair on (un)set of FileModel (emFilePanel… |
| `emFileSelectionBox-514` | `ParentDirField->GetTextSignal()` | `rc_cell_shim` | `emFileSelectionBox.rs:1101` | Rust uses an `events: RefCell<Events>` aggregator: widget closures push events; … |
| `emFileSelectionBox-521` | `HiddenCheckBox->GetCheckSignal()` | `rc_cell_shim` | `emFileSelectionBox.rs:1120` | Same events-aggregator pattern; Cycle reacts at line 1520. |
| `emFileSelectionBox-531` | `FilesLB->GetSelectionSignal()` | `rc_cell_shim` | `emFileSelectionBox.rs:1494` | FilesLB selection_changed flag drained by Cycle at line 1532. Substitute is rc_c… |
| `emFileSelectionBox-532` | `FilesLB->GetItemTriggerSignal()` | `rc_cell_shim` | `emFileSelectionBox.rs:1547` | ItemTrigger drained by Cycle. Same events-aggregator pattern. |
| `emFileSelectionBox-540` | `NameField->GetTextSignal()` | `rc_cell_shim` | `emFileSelectionBox.rs:1205` | Cycle drains name_text_changed at line 1576. |
| `emFileSelectionBox-550` | `FiltersLB->GetSelectionSignal()` | `rc_cell_shim` | `emFileSelectionBox.rs:1606` | FiltersLB filter_index_changed drained by Cycle. Same events-aggregator pattern. |
| `emImageFile-117` | `GetVirFileStateSignal()` | `absent` | `emImageFileImageFilePanel.rs:85` | C++ site is in emImageFilePanel constructor; Rust port lives in SPLIT file `crat… |
| `emImageFile-139` | `((const emImageFileModel*)GetFileModel())->GetChan` | `absent` | `emImageFileImageFilePanel.rs:85` | Accessor exists (`GetChangeSignal` returns `data_change_signal`) but no panel-si… |
| `emDirEntryAltPanel-35` | `FileMan->GetSelectionSignal()` | `absent` | `emDirEntryAltPanel.rs:154` | Rust panel never connect()s to selection signal nor polls GetSelectionSignal in … |
| `emDirEntryAltPanel-36` | `Config->GetChangeSignal()` | `polling` | `emDirEntryAltPanel.rs:160` | Rust polls u64 generation counter rather than subscribing to a SignalId. Cycle c… |
| `emDirEntryPanel-55` | `FileMan->GetSelectionSignal()` | `absent` | `emDirEntryPanel.rs:878` | Rust Cycle unconditionally calls update_bg_color() and update_content_panel/upda… |
| `emDirEntryPanel-56` | `Config->GetChangeSignal()` | `absent` | `emDirEntryPanel.rs:878` | Rust Cycle calls update_content_panel/update_alt_panel with forceRelayout=true u… |
| `emDirPanel-37` | `GetVirFileStateSignal()` | `polling` | `emDirPanel.rs:344` | Cycle polls dir_model.borrow().get_file_state() and returns stay_awake=true whil… |
| `emDirPanel-38` | `Config->GetChangeSignal()` | `polling` | `emDirPanel.rs:331` | Polls u64 generation counter. emFileManViewConfig::GetChangeSignal returns u64 (… |
| `emDirPanel-432` | `KeyWalkState->Timer.GetSignal()` | `absent` | `emDirPanel.rs:178` | Rust replaces emTimer + AddWakeUpSignal with std::time::Instant comparison check… |
| `emDirStatPanel-30` | `GetVirFileStateSignal()` | `polling` | `emDirStatPanel.rs:109` | Cycle polls vir-file-state every wake. No connect, no IsSignaled. Returns false … |
| `emDirStatPanel-39` | `Config->GetChangeSignal()` | `absent` | `emDirStatPanel.rs:109` | Rust panel acquires config in new() but Cycle never reads GetChangeSignal nor co… |
| `emFileLinkPanel-53` | `UpdateSignalModel->Sig` | `absent` | `emFileLinkPanel.rs:175` | Rust Cycle does not connect to or react to UpdateSignalModel. Accessor exists at… |
| `emFileLinkPanel-54` | `GetVirFileStateSignal()` | `polling` | `emFileLinkPanel.rs:175` | Cycle polls via refresh_vir_file_state(); no connect or IsSignaled. emFilePanel … |
| `emFileLinkPanel-55` | `Config->GetChangeSignal()` | `absent` | `emFileLinkPanel.rs:175` | Config field exists on the Rust panel but Cycle never reads its GetChangeSignal.… |
| `emFileManControlPanel-326` | `FMModel->GetSelectionSignal()` | `absent` | `emFileManControlPanel.rs:300` | C++ Cycle reacts to selection signal to UpdateButtonStates. Rust Cycle ignores t… |
| `emFileManControlPanel-327` | `FMVConfig->GetChangeSignal()` | `polling` | `emFileManControlPanel.rs:305` | u64 gen-counter polling. Verdict drifted. |
| `emFileManControlPanel-328` | `RbmAspect.GetCheckSignal()` | `absent` | `emFileManControlPanel.rs:300` | C++ uses signal subscription to update theme on aspect-ratio change. Rust handle… |
| `emFileManControlPanel-329` | `RbmTheme.GetCheckSignal()` | `absent` | `emFileManControlPanel.rs:300` | Theme style change handled via Input() inspection of theme_style_group state (li… |
| `emFileManControlPanel-330` | `RbSortByName->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Click signal fires on widget but panel does not connect; reaction inferred from … |
| `emFileManControlPanel-331` | `RbSortByDate->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Same drift pattern as -330. Verdict drifted/absent. |
| `emFileManControlPanel-332` | `RbSortBySize->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-333` | `RbSortByEnding->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-334` | `RbSortByClass->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-335` | `RbSortByVersion->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-336` | `CbSortDirectoriesFirst->GetCheckSignal()` | `absent` | `emFileManControlPanel.rs:480` | Reaction occurs in Input() handler immediately after dispatching to the widget —… |
| `emFileManControlPanel-337` | `CbShowHiddenFiles->GetCheckSignal()` | `absent` | `emFileManControlPanel.rs:487` | Same Input()-inline reaction; no connect. Verdict drifted/absent. |
| `emFileManControlPanel-338` | `RbPerLocale->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Reaction (if any) inferred from nss_group state in Input/sync flow. No Cycle sub… |
| `emFileManControlPanel-339` | `RbCaseSensitive->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Verdict drifted/absent. |
| `emFileManControlPanel-340` | `RbCaseInsensitive->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Verdict drifted/absent. |
| `emFileManControlPanel-341` | `CbAutosave->GetCheckSignal()` | `absent` | `emFileManControlPanel.rs:495` | Input()-inline reaction. Verdict drifted/absent. |
| `emFileManControlPanel-342` | `BtSaveAsDefault->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:503` | Verdict drifted/absent. |
| `emFileManControlPanel-343` | `BtSelectAll->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Verdict drifted/absent. |
| `emFileManControlPanel-344` | `BtClearSelection->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:518` | Verdict drifted/absent. |
| `emFileManControlPanel-345` | `BtSwapSelection->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:522` | Verdict drifted/absent. |
| `emFileManControlPanel-346` | `BtPaths2Clipboard->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Verdict drifted/absent. |
| `emFileManControlPanel-347` | `BtNames2Clipboard->GetClickSignal()` | `absent` | `emFileManControlPanel.rs:300` | Verdict drifted/absent. |
| `emFileManControlPanel-522` | `FMModel->GetCommandsSignal()` | `absent` | `emFileManControlPanel.rs:300` | GetCommandsSignal returns u64 (emFileManModel.rs:543), not SignalId. The C++ com… |
| `emFileManSelInfoPanel-37` | `FileMan->GetSelectionSignal()` | `polling` | `emFileManSelInfoPanel.rs:650` | u64 generation-counter polling. Cycle returns work_on_details() (busy-stay-awake… |
| `emAutoplay-1171` | `Model->GetChangeSignal()` | `absent` | `emAutoplayControlPanel.rs:658` | C++ port of emAutoplayControlPanel was split into crates/emmain/src/emAutoplayCo… |
| `emAutoplay-1172` | `Model->GetProgressSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:104` | emAutoplayViewModel has no GetProgressSignal accessor. C++ UpdateProgress (emAut… |
| `emAutoplay-1255` | `BtAutoplay->GetCheckSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:580` | Rust check_signal SignalId is allocated and fired (emCheckButton.rs:70) but the … |
| `emAutoplay-1270` | `BtPrev->GetClickSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:487` |  |
| `emAutoplay-1282` | `BtNext->GetClickSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:499` |  |
| `emAutoplay-1294` | `BtContinueLast->GetClickSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:627` |  |
| `emAutoplay-1314` | `SfDuration->GetValueSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:321` | C++ widget-check vocabulary chosen as nearest fit; emScalarField value-changed s… |
| `emAutoplay-1321` | `CbRecursive->GetCheckSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:347` |  |
| `emAutoplay-1329` | `CbLoop->GetCheckSignal()` | `rc_cell_shim` | `emAutoplayControlPanel.rs:378` |  |
| `emBookmarks-1479` | `GetClickSignal()` | `absent` | `emBookmarks.rs:528` | Bookmark click navigation is unimplemented in Rust. Accessor 'present' on emButt… |
| `emMainControlPanel-217` | `ContentView.GetControlPanelSignal()` | `absent` | `emMainControlPanel.rs:287` | ControlPanelBridge in emMainWindow.rs is the actual subscriber; from emMainContr… |
| `emMainControlPanel-219` | `MainConfig->GetChangeSignal()` | `absent` | `emMainControlPanel.rs:287` | Accessor exists; panel could subscribe but does not. |
| `emMainControlPanel-220` | `BtNewWindow->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:296` |  |
| `emMainControlPanel-221` | `BtFullscreen->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:301` |  |
| `emMainControlPanel-222` | `BtAutoHideControlView->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:311` |  |
| `emMainControlPanel-223` | `BtAutoHideSlider->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:315` |  |
| `emMainControlPanel-224` | `BtReload->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:319` |  |
| `emMainControlPanel-225` | `BtClose->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:328` |  |
| `emMainControlPanel-226` | `BtQuit->GetClickSignal()` | `rc_cell_shim` | `emMainControlPanel.rs:334` |  |
| `emMainPanel-67` | `GetControlView().GetEOISignal()` | `absent` | `emMainPanel.rs:658` | EOI signal exists on view but emMainPanel does not subscribe. |
| `emMainPanel-68` | `SliderTimer.GetSignal()` | `polling` | `emMainPanel.rs:663` | emTimer infrastructure exists; emMainPanel chose wall-clock polling instead. Cyc… |
| `emVirtualCosmos-104` | `FileUpdateSignalModel->Sig` | `absent` | `emVirtualCosmos.rs:213` | Site is in emVirtualCosmosModel constructor (not panel). Same file maps both mod… |
| `emVirtualCosmos-575` | `Model->GetChangeSignal()` | `rc_cell_shim` | `emVirtualCosmos.rs:821` | emVirtualCosmosModel exposes no GetChangeSignal accessor and never fires one (it… |
| `emStocksControlPanel-74` | `FileModel->GetChangeSignal()` | `absent` | `emStocksControlPanel.rs:387` | emStocksControlPanel has no Cycle/cycle/on_cycle_ext implementation and contains… |
| `emStocksControlPanel-75` | `Config->GetChangeSignal()` | `absent` | `emStocksControlPanel.rs:387` | No Cycle override and no connect(...) for config change; Config field exists on … |
| `emStocksControlPanel-76` | `ListBox->GetSelectionSignal()` | `absent` | `emStocksControlPanel.rs:387` | No Cycle override; no listbox-selection subscription. |
| `emStocksControlPanel-77` | `ListBox->GetSelectedDateSignal()` | `absent` | `emStocksControlPanel.rs:387` | Selected-date selection signal — no subscription, no reaction. signal_kind=selec… |
| `emStocksControlPanel-413` | `ApiKey->GetTextSignal()` | `absent` | `emStocksControlPanel.rs:387` | Widget GetTextSignal — outside §5.4 vocabulary; recorded as 'other' with descrip… |
| `emStocksControlPanel-427` | `AutoUpdateDates->GetCheckSignal()` | `absent` | `emStocksControlPanel.rs:387` | No subscription to checkbox check signal. |
| `emStocksControlPanel-435` | `TriggeringOpensWebPage->GetCheckSignal()` | `absent` | `emStocksControlPanel.rs:387` | No subscription to checkbox check signal. |
| `emStocksControlPanel-448` | `ChartPeriod->GetValueSignal()` | `absent` | `emStocksControlPanel.rs:387` | Scalar field GetValueSignal — outside §5.4 vocabulary; recorded as 'other' with … |
| `emStocksControlPanel-466` | `MinVisibleInterest->GetCheckSignal()` | `absent` | `emStocksControlPanel.rs:387` | Radio-buttons / interest-filter check signal. Source comments confirm: 'Individu… |
| `emStocksControlPanel-557` | `Sorting->GetCheckSignal()` | `absent` | `emStocksControlPanel.rs:387` | Sorting radio buttons; line 192 of rust file: 'stored for future signal wiring'. |
| `emStocksControlPanel-566` | `OwnedSharesFirst->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-586` | `FetchSharePrices->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. Note: emStocksFilePanel does spawn fetch_dialog elsewhere… |
| `emStocksControlPanel-600` | `DeleteSharePrices->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-609` | `GoBackInHistory->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-618` | `GoForwardInHistory->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-626` | `SelectedDate->GetTextSignal()` | `absent` | `emStocksControlPanel.rs:387` | Widget GetTextSignal — recorded as 'other' / 'widget-text'. |
| `emStocksControlPanel-650` | `NewStock->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-658` | `CutStocks->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. (Note: ListBox handles its own CutStocks dialog confirmat… |
| `emStocksControlPanel-666` | `CopyStocks->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-674` | `PasteStocks->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-682` | `DeleteStocks->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-690` | `SelectAll->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-698` | `ClearSelection->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-706` | `SetHighInterest->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-714` | `SetMediumInterest->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-722` | `SetLowInterest->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-730` | `ShowFirstWebPages->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-738` | `ShowAllWebPages->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-749` | `FindSelected->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-756` | `SearchText->GetTextSignal()` | `absent` | `emStocksControlPanel.rs:387` | Widget GetTextSignal — 'other' / 'widget-text'. |
| `emStocksControlPanel-764` | `FindNext->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-772` | `FindPrevious->GetClickSignal()` | `absent` | `emStocksControlPanel.rs:387` | No click subscription. |
| `emStocksControlPanel-1014` | `ControlPanel.Config->GetChangeSignal()` | `absent` | `emStocksControlPanel.rs:387` | Subscription site is the inner CategoryPanel constructor; Rust ControlCategoryPa… |
| `emStocksControlPanel-1064` | `TextField->GetTextSignal()` | `absent` | `emStocksControlPanel.rs:387` | Inner FileFieldPanel TextField text-changed signal — 'other' / 'widget-text'. Ru… |
| `emStocksControlPanel-1072` | `FileSelectionBox->GetSelectionSignal()` | `absent` | `emStocksControlPanel.rs:387` | FileSelectionBox-selection inside the FileFieldPanel popup; no subscription rail… |
| `emStocksControlPanel-1143` | `GetSelectionSignal()` | `absent` | `emStocksControlPanel.rs:387` | Inner CategoryPanel self-selection signal; no subscription rail in Rust ControlC… |
| `emStocksControlPanel-1144` | `ControlPanel.FileModel->GetChangeSignal()` | `absent` | `emStocksControlPanel.rs:387` | Inner CategoryPanel subscribes to outer ControlPanel.FileModel change in C++; no… |
| `emStocksFetchPricesDialog-62` | `Fetcher.GetChangeSignal()` | `polling` | `emStocksFetchPricesDialog.rs:91` | Cycle (line 91) polls fetcher.HasFinished() and unconditionally calls UpdateCont… |
| `emStocksFilePanel-34` | `GetVirFileStateSignal()` | `polling` | `emStocksFilePanel.rs:354` | Cycle (line 349) polls vir-file-state via before/after compare rather than IsSig… |
| `emStocksFilePanel-255` | `ListBox->GetSelectedDateSignal()` | `absent` | `emStocksFilePanel.rs:349` | Cycle does not react to ListBox selected-date changes. The ListBox::Cycle delega… |
| `emStocksItemChart-64` | `Config.GetChangeSignal()` | `absent` | `emStocksItemChart.rs:91` | ItemChart has no Cycle / cycle / connect; UpdateData is called manually by the p… |
| `emStocksItemChart-65` | `ListBox.GetSelectedDateSignal()` | `absent` | `emStocksItemChart.rs:91` | ItemChart does not subscribe to listbox selected-date; reaction is fully absent. |
| `emStocksItemPanel-74` | `Config.GetChangeSignal()` | `absent` | `emStocksItemPanel.rs:204` | ItemPanel has no Cycle override and no connect calls; reaction surface is absent… |
| `emStocksItemPanel-75` | `ListBox.GetSelectedDateSignal()` | `absent` | `emStocksItemPanel.rs:204` | Reaction absent. |
| `emStocksItemPanel-342` | `Name->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-357` | `Symbol->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-364` | `WKN->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-371` | `ISIN->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-395` | `Comment->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-408` | `WebPage[i]->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Per-WebPage TextField change signal in a loop — 'other' / 'widget-text'. |
| `emStocksItemPanel-415` | `ShowWebPage[i]->GetClickSignal()` | `absent` | `emStocksItemPanel.rs:204` | Per-ShowWebPage button click in a loop. Source comment line 109: 'ShowWebPage bu… |
| `emStocksItemPanel-421` | `ShowAllWebPages->GetClickSignal()` | `absent` | `emStocksItemPanel.rs:204` | ShowAllWebPages button — 'stored for future signal wiring' (line 112). |
| `emStocksItemPanel-432` | `OwningShares->GetCheckSignal()` | `absent` | `emStocksItemPanel.rs:204` | Checkbox check signal — absent. |
| `emStocksItemPanel-441` | `OwnShares->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-446` | `TradePrice->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-451` | `TradeDate->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-454` | `UpdateTradeDate->GetClickSignal()` | `absent` | `emStocksItemPanel.rs:204` | Click subscription absent. |
| `emStocksItemPanel-467` | `FetchSharePrice->GetClickSignal()` | `absent` | `emStocksItemPanel.rs:204` | Source comment line 82: 'FetchSharePrice button - stored for future signal wirin… |
| `emStocksItemPanel-490` | `Interest->GetCheckSignal()` | `absent` | `emStocksItemPanel.rs:204` | Interest radio group — check signal absent. |
| `emStocksItemPanel-504` | `ExpectedDividend->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-509` | `DesiredPrice->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-518` | `InquiryDate->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-527` | `UpdateInquiryDate->GetClickSignal()` | `absent` | `emStocksItemPanel.rs:204` | Source comment line 100: 'UpdateInquiryDate button - stored for future signal wi… |
| `emStocksItemPanel-831` | `ItemPanel.FileModel.GetChangeSignal()` | `absent` | `emStocksItemPanel.rs:204` | Inner CategoryPanel subscribes to outer ItemPanel.FileModel change in C++; Rust … |
| `emStocksItemPanel-832` | `ItemPanel.Config.GetChangeSignal()` | `absent` | `emStocksItemPanel.rs:204` | Inner CategoryPanel subscribes to outer ItemPanel.Config in C++; Rust CategoryPa… |
| `emStocksItemPanel-914` | `TextField->GetTextSignal()` | `absent` | `emStocksItemPanel.rs:204` | Inner CategoryPanel TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-922` | `ListBox->GetSelectionSignal()` | `absent` | `emStocksItemPanel.rs:204` | Inner CategoryPanel ListBox-selection signal — absent in Rust. |
| `emStocksListBox-51` | `FileModel.GetChangeSignal()` | `absent` | `emStocksListBox.rs:733` | Rust ListBox holds no FileModel reference; rec is passed in by caller (lines 188… |
| `emStocksListBox-52` | `Config.GetChangeSignal()` | `absent` | `emStocksListBox.rs:733` | Config is passed by reference per-call rather than held; no Config-change reacti… |
| `emStocksListBox-53` | `GetItemTriggerSignal()` | `absent` | `emStocksListBox.rs:733` | Inherited emListBox item-trigger signal — outside §5.4 vocabulary; recorded as '… |
| `emStocksListBox-189` | `CutStocksDialog->GetFinishSignal()` | `rc_cell_shim` | `emStocksListBox.rs:54` | Cut-confirmation dialog finish handled via Rc<Cell<Option<DialogResult>>> shim p… |
| `emStocksListBox-287` | `PasteStocksDialog->GetFinishSignal()` | `rc_cell_shim` | `emStocksListBox.rs:55` | Paste-confirmation dialog finish: rc_cell_shim. Same pattern as Cut/Delete/Inter… |
| `emStocksListBox-356` | `DeleteStocksDialog->GetFinishSignal()` | `rc_cell_shim` | `emStocksListBox.rs:56` | Delete-confirmation dialog finish: rc_cell_shim. |
| `emStocksListBox-443` | `InterestDialog->GetFinishSignal()` | `rc_cell_shim` | `emStocksListBox.rs:57` | Interest-change confirmation dialog finish: rc_cell_shim. |
| `emFileModel-103` | `UpdateSignalModel->Sig` | `absent` | `emFileModel.rs:483` | Model-internal: emFileModel subscribes to its shared UpdateSignalModel to react … |
| `emStocksFileModel-41` | `SaveTimer.GetSignal()` | `polling` | `emStocksFileModel.rs:62` | Model-internal: file 18 comment notes emTimer::TimerCentral is unported. Save ti… |

## 3. Gap-Blocked Rows

16 gap-blocked rows. These identify missing or type-mismatched accessors for the next fix session.

| ID | Signal | Accessor status | Accessor (or gap) | Notes (excerpt) |
|----|--------|-----------------|-------------------|-----------------|
| `emFileSelectionBox-64` | `FileModelsUpdateSignalModel->Sig` | present | emFileModel::AcquireUpdateSignalModel (crates/emcore/src/emF | C++ subscribes to global `FileModelsUpdateSignalModel->Sig` to invalidate listin… |
| `emFileLinkPanel-56` | `Model->GetChangeSignal()` | missing | — | C++ subscribes to emFileLinkModel's record-change signal (inherited from emRecFi… |
| `emFileLinkPanel-72` | `Model->GetChangeSignal()` | missing | — | C++ AddWakeUpSignal at line 72 is inside SetFileModel — re-attaches subscription… |
| `emMainControlPanel-218` | `MainWin.GetWindowFlagsSignal()` | present | emWindow::GetWindowFlagsSignal (crates/emcore/src/emWindow.r | emWindow.WindowFlags exists (crates/emcore/src/emWindow.rs:27) but no GetWindowF… |
| `emMainPanel-69` | `GetWindow()->GetWindowFlagsSignal()` | present | emWindow::GetWindowFlagsSignal (crates/emcore/src/emWindow.r | emWindow has WindowFlags bitflags but no SignalId accessor for flag changes. Sam… |
| `emFileManViewConfig-accessor-config-change` | `Config->GetChangeSignal()` | type-mismatch | emFileManViewConfig::GetChangeSignal | C++ emFileManViewConfig::GetChangeSignal() returns const emSignal&. Rust returns… |
| `emFileManModel-accessor-command` | `FMModel->GetCommandsSignal()` | type-mismatch | emFileManModel::GetCommandsSignal | C++ returns const emSignal&. Rust returns generation u64; consumer polls. Fix: a… |
| `emFileManModel-accessor-selection` | `FileMan->GetSelectionSignal()` | type-mismatch | emFileManModel::GetSelectionSignal | C++ returns const emSignal&. Rust returns generation u64; consumers compare cach… |
| `emFileLinkModel-accessor-model-change` | `Model->GetChangeSignal()` | missing | — | emFileLinkPanel.Model, emAutoplay.Model, emVirtualCosmos.Model all reference an … |
| `emAutoplayViewModel-accessor-model-change` | `Model->GetChangeSignal()` | missing | — | emAutoplay-1171 panel references the view-model; the config-side accessor at lin… |
| `emAutoplayViewModel-accessor-model-state` | `Model->GetProgressSignal()` | missing | — |  |
| `emVirtualCosmosModel-accessor-model-change` | `Model->GetChangeSignal()` | missing | — |  |
| `emFilePanel-accessor-vir-file-state` | `GetVirFileStateSignal()` | missing | — | emFilePanel-derived panels (image, dir, dirstat, filelink, stocksfile) all subsc… |
| `emStocksFileModel-accessor-model-change` | `FileModel->GetChangeSignal()` | missing | — |  |
| `emStocksConfig-accessor-config-change` | `Config->GetChangeSignal()` | missing | — |  |
| `emStocksPricesFetcher-accessor-model-change` | `Fetcher.GetChangeSignal()` | missing | — |  |

## 4. Forced Rows

(none) — inventory contains 0 forced rows.

## 5. Annotations Reclassified to Drifted

8 signal-related annotations failed revalidation (Task 6). These blocks were claimed `language-forced` but the review found the polling/shim substitution is not forced by language constraints — a scheduler `connect()` or direct signal subscription is achievable.

| Location | Claimed category | Notes |
|----------|------------------|-------|
| `emDirPanel.rs:117` | language-forced | see Task 6 report |
| `emMainControlPanel.rs:35` | language-forced | see Task 6 report |
| `emMainControlPanel.rs:303` | language-forced | see Task 6 report |
| `emMainControlPanel.rs:320` | language-forced | see Task 6 report |
| `emDialog.rs:35` | language-forced | see Task 6 report |
| `emDialog.rs:523` | language-forced | see Task 6 report |
| `emFileDialog.rs:68` | language-forced | see Task 6 report |
| `emFileDialog.rs:140` | language-forced | see Task 6 report |

### Also: wrong-category row

- **`emFileModel.rs:490`** — claimed `upstream-gap-forced`, rewritten to `language-forced`. see Task 6 report

This block at `emFileModel.rs:490` is not reclassified to *drifted* (the underlying behavior divergence is accepted), but the annotation category label was wrong (`upstream-gap-forced` → `language-forced`).

## 6. Annotations Verified

6 signal-related annotations passed revalidation (Task 6). These blocks accurately describe the language-forced constraint and the observable behavior is preserved.

| Location | Claimed category | Annotation summary (excerpt) |
|----------|------------------|------------------------------|
| `emImageFile.rs:69` | language-forced | /// DIVERGED: (language-forced) C++ creates file models via `emModel::Acquire(context, name)`,     /// a context-registe… |
| `emFileLinkPanel.rs:299` | language-forced | /// DIVERGED: (language-forced) C++ calls UpdateDataAndChildPanel from Cycle() and Notice().     /// Rust defers to Layo… |
| `emMainPanel.rs:29` | language-forced | /// DIVERGED: (language-forced) C++ SliderPanel holds a `MainPanel&` and calls /// `MainPanel.DragSlider(dy)` / `MainPan… |
| `emDialog.rs:818` | language-forced | // DIVERGED: (language-forced) DlgButton click observation — C++ emDialog.cpp:236 `DlgButton::Clicked()` walks     // pa… |
| `emFileDialog.rs:40` | language-forced | /// DIVERGED: (language-forced) Rust uses composition for C++ inheritance (idiom adaptation — /// observable behavior id… |
| `emMainWindow.rs:819` | language-forced | /// DIVERGED: (language-forced) C++ emMainControlPanel directly subscribes to /// ContentView.GetControlPanelSignal() an… |

## 7. Per-File Deep-Dive

Row-by-row detail for all Tier-B files. Format: `id | signal_kind | signal_expression | verdict | evidence_kind | evidence_location | accessor_status | notes-snippet`.

### `emStocksControlPanel.rs` (↔ `emStocksControlPanel.cpp`)

Rows: 37 — faithful: 0, drifted: 37, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksControlPanel-74` | model-change | `FileModel->GetChangeSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | emStocksControlPanel has no Cycle/cycle/on_cycle_ext impleme… |
| `emStocksControlPanel-75` | config-change | `Config->GetChangeSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No Cycle override and no connect(...) for config change; Con… |
| `emStocksControlPanel-76` | selection | `ListBox->GetSelectionSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No Cycle override; no listbox-selection subscription. |
| `emStocksControlPanel-77` | selection | `ListBox->GetSelectedDateSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Selected-date selection signal — no subscription, no reactio… |
| `emStocksControlPanel-413` | other | `ApiKey->GetTextSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Widget GetTextSignal — outside §5.4 vocabulary; recorded as … |
| `emStocksControlPanel-427` | widget-check | `AutoUpdateDates->GetCheckSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No subscription to checkbox check signal. |
| `emStocksControlPanel-435` | widget-check | `TriggeringOpensWebPage->GetCheckSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No subscription to checkbox check signal. |
| `emStocksControlPanel-448` | other | `ChartPeriod->GetValueSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Scalar field GetValueSignal — outside §5.4 vocabulary; recor… |
| `emStocksControlPanel-466` | widget-check | `MinVisibleInterest->GetCheckSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Radio-buttons / interest-filter check signal. Source comment… |
| `emStocksControlPanel-557` | widget-check | `Sorting->GetCheckSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Sorting radio buttons; line 192 of rust file: 'stored for fu… |
| `emStocksControlPanel-566` | widget-click | `OwnedSharesFirst->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-586` | widget-click | `FetchSharePrices->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. Note: emStocksFilePanel does spawn fe… |
| `emStocksControlPanel-600` | widget-click | `DeleteSharePrices->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-609` | widget-click | `GoBackInHistory->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-618` | widget-click | `GoForwardInHistory->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-626` | other | `SelectedDate->GetTextSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Widget GetTextSignal — recorded as 'other' / 'widget-text'. |
| `emStocksControlPanel-650` | widget-click | `NewStock->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-658` | widget-click | `CutStocks->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. (Note: ListBox handles its own CutSto… |
| `emStocksControlPanel-666` | widget-click | `CopyStocks->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-674` | widget-click | `PasteStocks->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-682` | widget-click | `DeleteStocks->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-690` | widget-click | `SelectAll->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-698` | widget-click | `ClearSelection->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-706` | widget-click | `SetHighInterest->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-714` | widget-click | `SetMediumInterest->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-722` | widget-click | `SetLowInterest->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-730` | widget-click | `ShowFirstWebPages->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-738` | widget-click | `ShowAllWebPages->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-749` | widget-click | `FindSelected->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-756` | other | `SearchText->GetTextSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Widget GetTextSignal — 'other' / 'widget-text'. |
| `emStocksControlPanel-764` | widget-click | `FindNext->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-772` | widget-click | `FindPrevious->GetClickSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | No click subscription. |
| `emStocksControlPanel-1014` | config-change | `ControlPanel.Config->GetChangeSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Subscription site is the inner CategoryPanel constructor; Ru… |
| `emStocksControlPanel-1064` | other | `TextField->GetTextSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Inner FileFieldPanel TextField text-changed signal — 'other'… |
| `emStocksControlPanel-1072` | selection | `FileSelectionBox->GetSelectionSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | FileSelectionBox-selection inside the FileFieldPanel popup; … |
| `emStocksControlPanel-1143` | selection | `GetSelectionSignal()` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Inner CategoryPanel self-selection signal; no subscription r… |
| `emStocksControlPanel-1144` | model-change | `ControlPanel.FileModel->GetChangeSignal(` | **drifted** | absent | `emStocksControlPanel.rs:387` | missing | Inner CategoryPanel subscribes to outer ControlPanel.FileMod… |

### `emStocksItemPanel.rs` (↔ `emStocksItemPanel.cpp`)

Rows: 25 — faithful: 0, drifted: 25, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksItemPanel-74` | config-change | `Config.GetChangeSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | ItemPanel has no Cycle override and no connect calls; reacti… |
| `emStocksItemPanel-75` | selection | `ListBox.GetSelectedDateSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Reaction absent. |
| `emStocksItemPanel-342` | other | `Name->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-357` | other | `Symbol->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-364` | other | `WKN->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-371` | other | `ISIN->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-395` | other | `Comment->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-408` | other | `WebPage[i]->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Per-WebPage TextField change signal in a loop — 'other' / 'w… |
| `emStocksItemPanel-415` | widget-click | `ShowWebPage[i]->GetClickSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Per-ShowWebPage button click in a loop. Source comment line … |
| `emStocksItemPanel-421` | widget-click | `ShowAllWebPages->GetClickSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | ShowAllWebPages button — 'stored for future signal wiring' (… |
| `emStocksItemPanel-432` | widget-check | `OwningShares->GetCheckSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Checkbox check signal — absent. |
| `emStocksItemPanel-441` | other | `OwnShares->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-446` | other | `TradePrice->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-451` | other | `TradeDate->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-454` | widget-click | `UpdateTradeDate->GetClickSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Click subscription absent. |
| `emStocksItemPanel-467` | widget-click | `FetchSharePrice->GetClickSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Source comment line 82: 'FetchSharePrice button - stored for… |
| `emStocksItemPanel-490` | widget-check | `Interest->GetCheckSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Interest radio group — check signal absent. |
| `emStocksItemPanel-504` | other | `ExpectedDividend->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-509` | other | `DesiredPrice->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-518` | other | `InquiryDate->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Widget TextField change signal — 'other' / 'widget-text'. |
| `emStocksItemPanel-527` | widget-click | `UpdateInquiryDate->GetClickSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Source comment line 100: 'UpdateInquiryDate button - stored … |
| `emStocksItemPanel-831` | model-change | `ItemPanel.FileModel.GetChangeSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Inner CategoryPanel subscribes to outer ItemPanel.FileModel … |
| `emStocksItemPanel-832` | config-change | `ItemPanel.Config.GetChangeSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Inner CategoryPanel subscribes to outer ItemPanel.Config in … |
| `emStocksItemPanel-914` | other | `TextField->GetTextSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Inner CategoryPanel TextField change signal — 'other' / 'wid… |
| `emStocksItemPanel-922` | selection | `ListBox->GetSelectionSignal()` | **drifted** | absent | `emStocksItemPanel.rs:204` | missing | Inner CategoryPanel ListBox-selection signal — absent in Rus… |

### `emFileManControlPanel.rs` (↔ `emFileManControlPanel.cpp`)

Rows: 23 — faithful: 0, drifted: 23, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileManControlPanel-326` | selection | `FMModel->GetSelectionSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | type-mismatch | C++ Cycle reacts to selection signal to UpdateButtonStates. … |
| `emFileManControlPanel-327` | config-change | `FMVConfig->GetChangeSignal()` | **drifted** | polling | `emFileManControlPanel.rs:305` | type-mismatch | u64 gen-counter polling. Verdict drifted. |
| `emFileManControlPanel-328` | widget-check | `RbmAspect.GetCheckSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | C++ uses signal subscription to update theme on aspect-ratio… |
| `emFileManControlPanel-329` | widget-check | `RbmTheme.GetCheckSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Theme style change handled via Input() inspection of theme_s… |
| `emFileManControlPanel-330` | widget-click | `RbSortByName->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Click signal fires on widget but panel does not connect; rea… |
| `emFileManControlPanel-331` | widget-click | `RbSortByDate->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Same drift pattern as -330. Verdict drifted/absent. |
| `emFileManControlPanel-332` | widget-click | `RbSortBySize->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-333` | widget-click | `RbSortByEnding->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-334` | widget-click | `RbSortByClass->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-335` | widget-click | `RbSortByVersion->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Same drift pattern. Verdict drifted/absent. |
| `emFileManControlPanel-336` | widget-check | `CbSortDirectoriesFirst->GetCheckSignal()` | **drifted** | absent | `emFileManControlPanel.rs:480` | present | Reaction occurs in Input() handler immediately after dispatc… |
| `emFileManControlPanel-337` | widget-check | `CbShowHiddenFiles->GetCheckSignal()` | **drifted** | absent | `emFileManControlPanel.rs:487` | present | Same Input()-inline reaction; no connect. Verdict drifted/ab… |
| `emFileManControlPanel-338` | widget-click | `RbPerLocale->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Reaction (if any) inferred from nss_group state in Input/syn… |
| `emFileManControlPanel-339` | widget-click | `RbCaseSensitive->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Verdict drifted/absent. |
| `emFileManControlPanel-340` | widget-click | `RbCaseInsensitive->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Verdict drifted/absent. |
| `emFileManControlPanel-341` | widget-check | `CbAutosave->GetCheckSignal()` | **drifted** | absent | `emFileManControlPanel.rs:495` | present | Input()-inline reaction. Verdict drifted/absent. |
| `emFileManControlPanel-342` | widget-click | `BtSaveAsDefault->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:503` | present | Verdict drifted/absent. |
| `emFileManControlPanel-343` | widget-click | `BtSelectAll->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Verdict drifted/absent. |
| `emFileManControlPanel-344` | widget-click | `BtClearSelection->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:518` | present | Verdict drifted/absent. |
| `emFileManControlPanel-345` | widget-click | `BtSwapSelection->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:522` | present | Verdict drifted/absent. |
| `emFileManControlPanel-346` | widget-click | `BtPaths2Clipboard->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Verdict drifted/absent. |
| `emFileManControlPanel-347` | widget-click | `BtNames2Clipboard->GetClickSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | present | Verdict drifted/absent. |
| `emFileManControlPanel-522` | command | `FMModel->GetCommandsSignal()` | **drifted** | absent | `emFileManControlPanel.rs:300` | type-mismatch | GetCommandsSignal returns u64 (emFileManModel.rs:543), not S… |

### `emCoreConfigPanel.rs` (↔ `emCoreConfigPanel.cpp`)

Rows: 9 — faithful: 0, drifted: 9, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emCoreConfigPanel-80` | widget-click | `ResetButton->GetClickSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:1539` | present | Reset button. No Cycle/IsSignaled — substitute is a synchron… |
| `emCoreConfigPanel-299` | widget-check | `StickBox->GetCheckSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:341` | present | StickBox. on_check closure substitutes for AddWakeUpSignal+I… |
| `emCoreConfigPanel-300` | widget-check | `EmuBox->GetCheckSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:363` | present | EmuBox. Same on_check closure pattern. |
| `emCoreConfigPanel-301` | widget-check | `PanBox->GetCheckSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:378` | present | PanBox. Same on_check closure pattern. |
| `emCoreConfigPanel-563` | widget-click | `MemField->GetValueSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:801` | present | MemField scalar (max megabytes per view). on_value closure p… |
| `emCoreConfigPanel-746` | widget-click | `MaxRenderThreadsField->GetValueSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:1039` | present | MaxRenderThreads. on_value closure pattern. |
| `emCoreConfigPanel-755` | widget-check | `AllowSIMDBox->GetCheckSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:1066` | present | AllowSIMD. on_check closure pattern. |
| `emCoreConfigPanel-773` | widget-click | `DownscaleQualityField->GetValueSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:1205` | present | DownscaleQuality. on_value closure pattern. |
| `emCoreConfigPanel-791` | widget-click | `UpscaleQualityField->GetValueSignal()` | **drifted** | rc_cell_shim | `emCoreConfigPanel.rs:1233` | present | UpscaleQuality. on_value closure pattern. |

### `emAutoplay.rs` (↔ `emAutoplay.cpp`)

Rows: 11 — faithful: 0, drifted: 9, forced: 0, gap-blocked: 2, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emAutoplay-1171` | model-change | `Model->GetChangeSignal()` | **drifted** | absent | `emAutoplayControlPanel.rs:658` | missing | C++ port of emAutoplayControlPanel was split into crates/emm… |
| `emAutoplay-1172` | model-state | `Model->GetProgressSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:104` | missing | emAutoplayViewModel has no GetProgressSignal accessor. C++ U… |
| `emAutoplay-1255` | widget-check | `BtAutoplay->GetCheckSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:580` | present | Rust check_signal SignalId is allocated and fired (emCheckBu… |
| `emAutoplay-1270` | widget-click | `BtPrev->GetClickSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:487` | present |  |
| `emAutoplay-1282` | widget-click | `BtNext->GetClickSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:499` | present |  |
| `emAutoplay-1294` | widget-click | `BtContinueLast->GetClickSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:627` | present |  |
| `emAutoplay-1314` | widget-click | `SfDuration->GetValueSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:321` | present | C++ widget-check vocabulary chosen as nearest fit; emScalarF… |
| `emAutoplay-1321` | widget-check | `CbRecursive->GetCheckSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:347` | present |  |
| `emAutoplay-1329` | widget-check | `CbLoop->GetCheckSignal()` | **drifted** | rc_cell_shim | `emAutoplayControlPanel.rs:378` | present |  |
| `emAutoplayViewModel-accessor-model-change` | model-change | `Model->GetChangeSignal()` | **gap-blocked** | absent | `emAutoplay.rs:800` | missing | emAutoplay-1171 panel references the view-model; the config-… |
| `emAutoplayViewModel-accessor-model-state` | model-state | `Model->GetProgressSignal()` | **gap-blocked** | absent | `emAutoplay.rs:800` | missing |  |

### `emMainControlPanel.rs` (↔ `emMainControlPanel.cpp`)

Rows: 10 — faithful: 0, drifted: 9, forced: 0, gap-blocked: 1, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emMainControlPanel-217` | model-state | `ContentView.GetControlPanelSignal()` | **drifted** | absent | `emMainControlPanel.rs:287` | present | ControlPanelBridge in emMainWindow.rs is the actual subscrib… |
| `emMainControlPanel-218` | model-state | `MainWin.GetWindowFlagsSignal()` | **gap-blocked** | absent | `emMainControlPanel.rs:287` | present | emWindow.WindowFlags exists (crates/emcore/src/emWindow.rs:2… |
| `emMainControlPanel-219` | config-change | `MainConfig->GetChangeSignal()` | **drifted** | absent | `emMainControlPanel.rs:287` | present | Accessor exists; panel could subscribe but does not. |
| `emMainControlPanel-220` | widget-click | `BtNewWindow->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:296` | present |  |
| `emMainControlPanel-221` | widget-click | `BtFullscreen->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:301` | present |  |
| `emMainControlPanel-222` | widget-click | `BtAutoHideControlView->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:311` | present |  |
| `emMainControlPanel-223` | widget-click | `BtAutoHideSlider->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:315` | present |  |
| `emMainControlPanel-224` | widget-click | `BtReload->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:319` | present |  |
| `emMainControlPanel-225` | widget-click | `BtClose->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:328` | present |  |
| `emMainControlPanel-226` | widget-click | `BtQuit->GetClickSignal()` | **drifted** | rc_cell_shim | `emMainControlPanel.rs:334` | present |  |

### `emColorField.rs` (↔ `emColorField.cpp`)

Rows: 8 — faithful: 0, drifted: 8, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emColorField-245` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:277` | present | emColorField::Cycle (Rust line 271) compares cached `sf_*` /… |
| `emColorField-255` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:277` | present | Green ScalarField. Same polling-based Cycle as emColorField-… |
| `emColorField-265` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:277` | present | Blue ScalarField. Same polling-based Cycle. |
| `emColorField-277` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:277` | present | Alpha ScalarField. Same polling-based Cycle. |
| `emColorField-288` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:282` | present | Hue ScalarField. Same polling-based Cycle. |
| `emColorField-298` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:282` | present | Saturation ScalarField. Same polling-based Cycle. |
| `emColorField-308` | widget-click | `sf->GetValueSignal()` | **drifted** | polling | `emColorField.rs:282` | present | Value (brightness) ScalarField. Same polling-based Cycle. |
| `emColorField-320` | widget-click | `tf->GetTextSignal()` | **drifted** | polling | `emColorField.rs:285` | present | Name TextField. Polled via `sync_from_children` (line 342) w… |

### `emStocksListBox.rs` (↔ `emStocksListBox.cpp`)

Rows: 7 — faithful: 0, drifted: 7, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksListBox-51` | model-change | `FileModel.GetChangeSignal()` | **drifted** | absent | `emStocksListBox.rs:733` | missing | Rust ListBox holds no FileModel reference; rec is passed in … |
| `emStocksListBox-52` | config-change | `Config.GetChangeSignal()` | **drifted** | absent | `emStocksListBox.rs:733` | missing | Config is passed by reference per-call rather than held; no … |
| `emStocksListBox-53` | other | `GetItemTriggerSignal()` | **drifted** | absent | `emStocksListBox.rs:733` | missing | Inherited emListBox item-trigger signal — outside §5.4 vocab… |
| `emStocksListBox-189` | finish | `CutStocksDialog->GetFinishSignal()` | **drifted** | rc_cell_shim | `emStocksListBox.rs:54` | missing | Cut-confirmation dialog finish handled via Rc<Cell<Option<Di… |
| `emStocksListBox-287` | finish | `PasteStocksDialog->GetFinishSignal()` | **drifted** | rc_cell_shim | `emStocksListBox.rs:55` | missing | Paste-confirmation dialog finish: rc_cell_shim. Same pattern… |
| `emStocksListBox-356` | finish | `DeleteStocksDialog->GetFinishSignal()` | **drifted** | rc_cell_shim | `emStocksListBox.rs:56` | missing | Delete-confirmation dialog finish: rc_cell_shim. |
| `emStocksListBox-443` | finish | `InterestDialog->GetFinishSignal()` | **drifted** | rc_cell_shim | `emStocksListBox.rs:57` | missing | Interest-change confirmation dialog finish: rc_cell_shim. |

### `emFileSelectionBox.rs` (↔ `emFileSelectionBox.cpp`)

Rows: 7 — faithful: 0, drifted: 6, forced: 0, gap-blocked: 1, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileSelectionBox-64` | model-update-broadcast | `FileModelsUpdateSignalModel->Sig` | **gap-blocked** | absent | `emFileSelectionBox.rs:1494` | present | C++ subscribes to global `FileModelsUpdateSignalModel->Sig` … |
| `emFileSelectionBox-514` | widget-click | `ParentDirField->GetTextSignal()` | **drifted** | rc_cell_shim | `emFileSelectionBox.rs:1101` | present | Rust uses an `events: RefCell<Events>` aggregator: widget cl… |
| `emFileSelectionBox-521` | widget-check | `HiddenCheckBox->GetCheckSignal()` | **drifted** | rc_cell_shim | `emFileSelectionBox.rs:1120` | present | Same events-aggregator pattern; Cycle reacts at line 1520. |
| `emFileSelectionBox-531` | selection | `FilesLB->GetSelectionSignal()` | **drifted** | rc_cell_shim | `emFileSelectionBox.rs:1494` | present | FilesLB selection_changed flag drained by Cycle at line 1532… |
| `emFileSelectionBox-532` | widget-click | `FilesLB->GetItemTriggerSignal()` | **drifted** | rc_cell_shim | `emFileSelectionBox.rs:1547` | present | ItemTrigger drained by Cycle. Same events-aggregator pattern… |
| `emFileSelectionBox-540` | widget-click | `NameField->GetTextSignal()` | **drifted** | rc_cell_shim | `emFileSelectionBox.rs:1205` | present | Cycle drains name_text_changed at line 1576. |
| `emFileSelectionBox-550` | selection | `FiltersLB->GetSelectionSignal()` | **drifted** | rc_cell_shim | `emFileSelectionBox.rs:1606` | present | FiltersLB filter_index_changed drained by Cycle. Same events… |

### `emDirPanel.rs` (↔ `emDirPanel.cpp`)

Rows: 3 — faithful: 0, drifted: 3, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emDirPanel-37` | vir-file-state | `GetVirFileStateSignal()` | **drifted** | polling | `emDirPanel.rs:344` | missing | Cycle polls dir_model.borrow().get_file_state() and returns … |
| `emDirPanel-38` | config-change | `Config->GetChangeSignal()` | **drifted** | polling | `emDirPanel.rs:331` | type-mismatch | Polls u64 generation counter. emFileManViewConfig::GetChange… |
| `emDirPanel-432` | timer | `KeyWalkState->Timer.GetSignal()` | **drifted** | absent | `emDirPanel.rs:178` | missing | Rust replaces emTimer + AddWakeUpSignal with std::time::Inst… |

### `emFileLinkPanel.rs` (↔ `emFileLinkPanel.cpp`)

Rows: 5 — faithful: 0, drifted: 3, forced: 0, gap-blocked: 2, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileLinkPanel-53` | model-update-broadcast | `UpdateSignalModel->Sig` | **drifted** | absent | `emFileLinkPanel.rs:175` | present | Rust Cycle does not connect to or react to UpdateSignalModel… |
| `emFileLinkPanel-54` | vir-file-state | `GetVirFileStateSignal()` | **drifted** | polling | `emFileLinkPanel.rs:175` | missing | Cycle polls via refresh_vir_file_state(); no connect or IsSi… |
| `emFileLinkPanel-55` | config-change | `Config->GetChangeSignal()` | **drifted** | absent | `emFileLinkPanel.rs:175` | type-mismatch | Config field exists on the Rust panel but Cycle never reads … |
| `emFileLinkPanel-56` | model-change | `Model->GetChangeSignal()` | **gap-blocked** | absent | `emFileLinkPanel.rs:175` | missing | C++ subscribes to emFileLinkModel's record-change signal (in… |
| `emFileLinkPanel-72` | model-change | `Model->GetChangeSignal()` | **gap-blocked** | absent | `emFileLinkPanel.rs:175` | missing | C++ AddWakeUpSignal at line 72 is inside SetFileModel — re-a… |

### `emImageFile.rs` (↔ `emImageFile.cpp`)

Rows: 2 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emImageFile-117` | vir-file-state | `GetVirFileStateSignal()` | **drifted** | absent | `emImageFileImageFilePanel.rs:85` | missing | C++ site is in emImageFilePanel constructor; Rust port lives… |
| `emImageFile-139` | model-change | `((const emImageFileModel*)GetFileModel()` | **drifted** | absent | `emImageFileImageFilePanel.rs:85` | present | Accessor exists (`GetChangeSignal` returns `data_change_sign… |

### `emDirEntryAltPanel.rs` (↔ `emDirEntryAltPanel.cpp`)

Rows: 2 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emDirEntryAltPanel-35` | selection | `FileMan->GetSelectionSignal()` | **drifted** | absent | `emDirEntryAltPanel.rs:154` | type-mismatch | Rust panel never connect()s to selection signal nor polls Ge… |
| `emDirEntryAltPanel-36` | config-change | `Config->GetChangeSignal()` | **drifted** | polling | `emDirEntryAltPanel.rs:160` | type-mismatch | Rust polls u64 generation counter rather than subscribing to… |

### `emDirEntryPanel.rs` (↔ `emDirEntryPanel.cpp`)

Rows: 2 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emDirEntryPanel-55` | selection | `FileMan->GetSelectionSignal()` | **drifted** | absent | `emDirEntryPanel.rs:878` | type-mismatch | Rust Cycle unconditionally calls update_bg_color() and updat… |
| `emDirEntryPanel-56` | config-change | `Config->GetChangeSignal()` | **drifted** | absent | `emDirEntryPanel.rs:878` | type-mismatch | Rust Cycle calls update_content_panel/update_alt_panel with … |

### `emDirStatPanel.rs` (↔ `emDirStatPanel.cpp`)

Rows: 2 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emDirStatPanel-30` | vir-file-state | `GetVirFileStateSignal()` | **drifted** | polling | `emDirStatPanel.rs:109` | missing | Cycle polls vir-file-state every wake. No connect, no IsSign… |
| `emDirStatPanel-39` | config-change | `Config->GetChangeSignal()` | **drifted** | absent | `emDirStatPanel.rs:109` | type-mismatch | Rust panel acquires config in new() but Cycle never reads Ge… |

### `emMainPanel.rs` (↔ `emMainPanel.cpp`)

Rows: 3 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 1, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emMainPanel-67` | model-state | `GetControlView().GetEOISignal()` | **drifted** | absent | `emMainPanel.rs:658` | present | EOI signal exists on view but emMainPanel does not subscribe… |
| `emMainPanel-68` | timer | `SliderTimer.GetSignal()` | **drifted** | polling | `emMainPanel.rs:663` | present | emTimer infrastructure exists; emMainPanel chose wall-clock … |
| `emMainPanel-69` | model-state | `GetWindow()->GetWindowFlagsSignal()` | **gap-blocked** | absent | `emMainPanel.rs:658` | present | emWindow has WindowFlags bitflags but no SignalId accessor f… |

### `emVirtualCosmos.rs` (↔ `emVirtualCosmos.cpp`)

Rows: 3 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 1, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emVirtualCosmos-104` | model-update-broadcast | `FileUpdateSignalModel->Sig` | **drifted** | absent | `emVirtualCosmos.rs:213` | present | Site is in emVirtualCosmosModel constructor (not panel). Sam… |
| `emVirtualCosmos-575` | model-change | `Model->GetChangeSignal()` | **drifted** | rc_cell_shim | `emVirtualCosmos.rs:821` | missing | emVirtualCosmosModel exposes no GetChangeSignal accessor and… |
| `emVirtualCosmosModel-accessor-model-change` | model-change | `Model->GetChangeSignal()` | **gap-blocked** | absent | `emVirtualCosmos.rs:213` | missing |  |

### `emStocksFilePanel.rs` (↔ `emStocksFilePanel.cpp`)

Rows: 2 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksFilePanel-34` | vir-file-state | `GetVirFileStateSignal()` | **drifted** | polling | `emStocksFilePanel.rs:354` | missing | Cycle (line 349) polls vir-file-state via before/after compa… |
| `emStocksFilePanel-255` | selection | `ListBox->GetSelectedDateSignal()` | **drifted** | absent | `emStocksFilePanel.rs:349` | missing | Cycle does not react to ListBox selected-date changes. The L… |

### `emStocksItemChart.rs` (↔ `emStocksItemChart.cpp`)

Rows: 2 — faithful: 0, drifted: 2, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksItemChart-64` | config-change | `Config.GetChangeSignal()` | **drifted** | absent | `emStocksItemChart.rs:91` | missing | ItemChart has no Cycle / cycle / connect; UpdateData is call… |
| `emStocksItemChart-65` | selection | `ListBox.GetSelectedDateSignal()` | **drifted** | absent | `emStocksItemChart.rs:91` | missing | ItemChart does not subscribe to listbox selected-date; react… |

### `emFilePanel.rs` (↔ `emFilePanel.cpp`)

Rows: 2 — faithful: 0, drifted: 1, forced: 0, gap-blocked: 1, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFilePanel-50` | model-state | `fileModel->GetFileStateSignal()` | **drifted** | polling | `emFilePanel.rs:138` | present | C++ AddWakeUpSignal/RemoveWakeUpSignal pair on (un)set of Fi… |
| `emFilePanel-accessor-vir-file-state` | vir-file-state | `GetVirFileStateSignal()` | **gap-blocked** | absent | `emFilePanel.rs:100` | missing | emFilePanel-derived panels (image, dir, dirstat, filelink, s… |

### `emFileManSelInfoPanel.rs` (↔ `emFileManSelInfoPanel.cpp`)

Rows: 1 — faithful: 0, drifted: 1, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileManSelInfoPanel-37` | selection | `FileMan->GetSelectionSignal()` | **drifted** | polling | `emFileManSelInfoPanel.rs:650` | type-mismatch | u64 generation-counter polling. Cycle returns work_on_detail… |

### `emBookmarks.rs` (↔ `emBookmarks.cpp`)

Rows: 22 — faithful: 0, drifted: 1, forced: 0, gap-blocked: 0, unported: 21

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emBookmarks-1033` | widget-click | `BtNewBookmarkBefore->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present | All emBookmarkEntryAuxPanel AddWakeUpSignal sites (1033-1228… |
| `emBookmarks-1041` | widget-click | `BtNewGroupBefore->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1049` | widget-click | `BtPasteBefore->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1071` | widget-click | `BtCut->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1082` | widget-click | `BtCopy->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1094` | widget-click | `TfName->GetTextSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing | C++ emTextField::GetTextSignal — Rust has no emTextField por… |
| `emBookmarks-1106` | widget-click | `TfDescription->GetTextSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing |  |
| `emBookmarks-1123` | selection | `FlbIcon->GetSelectionSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing |  |
| `emBookmarks-1124` | selection | `FlbIcon->GetFileTriggerSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing |  |
| `emBookmarks-1135` | widget-click | `CfBgColor->GetColorSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing | emColorField has a Rust port (emColorField.rs) but is unused… |
| `emBookmarks-1146` | widget-click | `CfFgColor->GetColorSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing |  |
| `emBookmarks-1159` | widget-click | `BtPasteColors->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1175` | widget-click | `BtSetLocation->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1185` | widget-click | `TfHotkey->GetTextSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing |  |
| `emBookmarks-1194` | widget-click | `RbVisitAtProgramStart->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | missing |  |
| `emBookmarks-1212` | widget-click | `BtNewBookmarkAfter->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1220` | widget-click | `BtNewGroupAfter->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1228` | widget-click | `BtPasteAfter->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1431` | widget-click | `BtNewBookmark->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1439` | widget-click | `BtNewGroup->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1447` | widget-click | `BtPaste->GetClickSignal()` | **unported** | absent | `emBookmarks.rs:0` | present |  |
| `emBookmarks-1479` | widget-click | `GetClickSignal()` | **drifted** | absent | `emBookmarks.rs:528` | missing | Bookmark click navigation is unimplemented in Rust. Accessor… |

### `emFileDialog.rs` (↔ `emFileDialog.cpp`)

Rows: 2 — faithful: 1, drifted: 1, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileDialog-42` | widget-click | `Fsb->GetFileTriggerSignal()` | **faithful** | connect_call | `emFileDialog.rs:110` | present | IsSignaled site: crates/emcore/src/emFileDialog.rs:151 `if c… |
| `emFileDialog-196` | finish | `OverwriteDialog->GetFinishSignal()` | **drifted** | connect_call | `emFileDialog.rs:516` | present | IsSignaled site: crates/emcore/src/emFileDialog.rs:169 `if c… |

### `emStocksFetchPricesDialog.rs` (↔ `emStocksFetchPricesDialog.cpp`)

Rows: 1 — faithful: 0, drifted: 1, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksFetchPricesDialog-62` | model-change | `Fetcher.GetChangeSignal()` | **drifted** | polling | `emStocksFetchPricesDialog.rs:91` | missing | Cycle (line 91) polls fetcher.HasFinished() and unconditiona… |

### `emFileModel.rs` (↔ `emFileModel.cpp`)

Rows: 1 — faithful: 0, drifted: 1, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileModel-103` | model-update-broadcast | `UpdateSignalModel->Sig` | **drifted** | absent | `emFileModel.rs:483` | present | Model-internal: emFileModel subscribes to its shared UpdateS… |

### `emStocksFileModel.rs` (↔ `emStocksFileModel.cpp`)

Rows: 2 — faithful: 0, drifted: 1, forced: 0, gap-blocked: 1, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksFileModel-accessor-model-change` | model-change | `FileModel->GetChangeSignal()` | **gap-blocked** | absent | `emStocksFileModel.rs:14` | missing |  |
| `emStocksFileModel-41` | timer | `SaveTimer.GetSignal()` | **drifted** | polling | `emStocksFileModel.rs:62` | missing | Model-internal: file 18 comment notes emTimer::TimerCentral … |

### `emDialog.rs` (↔ `emDialog.cpp`)

Rows: 1 — faithful: 1, drifted: 0, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emDialog-38` | close | `GetCloseSignal()` | **faithful** | connect_call | `emGUIFramework.rs:671` | present | IsSignaled site: crates/emcore/src/emDialog.rs:909 `if ctx.I… |

### `emMiniIpc.rs` (↔ `emMiniIpc.cpp`)

Rows: 1 — faithful: 1, drifted: 0, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emMiniIpc-807` | timer | `Timer.GetSignal()` | **faithful** | connect_call | `emMiniIpc.rs:366` | present | IsSignaled site: crates/emcore/src/emMiniIpc.rs:325 `if ctx.… |

### `emWindowStateSaver.rs` (↔ `emWindowStateSaver.cpp`)

Rows: 3 — faithful: 3, drifted: 0, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emWindowStateSaver-39` | other | `Window.GetWindowFlagsSignal()` | **faithful** | connect_call | `emMainWindow.rs:1207` | present | IsSignaled site: crates/emcore/src/emWindowStateSaver.rs:239… |
| `emWindowStateSaver-40` | other | `Window.GetGeometrySignal()` | **faithful** | connect_call | `emMainWindow.rs:1209` | present | IsSignaled site: crates/emcore/src/emWindowStateSaver.rs:241… |
| `emWindowStateSaver-41` | other | `Window.GetFocusSignal()` | **faithful** | connect_call | `emMainWindow.rs:1208` | present | IsSignaled site: crates/emcore/src/emWindowStateSaver.rs:240… |

### `emMainWindow.rs` (↔ `emMainWindow.cpp`)

Rows: 2 — faithful: 2, drifted: 0, forced: 0, gap-blocked: 0, unported: 0

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emMainWindow-72` | close | `GetCloseSignal()` | **faithful** | connect_call | `emMainWindow.rs:1113` | present | IsSignaled at crates/emmain/src/emMainWindow.rs:352 inside M… |
| `emMainWindow-378` | model-state | `MainWin.MainPanel->GetContentView().GetT` | **faithful** | connect_call | `emMainWindow.rs:1114` | present | IsSignaled at crates/emmain/src/emMainWindow.rs:360 inside M… |

### `emConfigModel.rs` (↔ `emConfigModel.cpp`)

Rows: 1 — faithful: 0, drifted: 0, forced: 0, gap-blocked: 0, unported: 1

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emConfigModel-61` | timer | `AutoSaveTimer.GetSignal()` | **unported** | absent | `emConfigModel.rs:1` | missing | Model-internal subscription. Auto-save timer not yet ported;… |

### `emFileManModel.rs` (↔ `emFileManModel.cpp`)

Rows: 3 — faithful: 0, drifted: 0, forced: 0, gap-blocked: 2, unported: 1

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileManModel-accessor-command` | command | `FMModel->GetCommandsSignal()` | **gap-blocked** | absent | `emFileManModel.rs:543` | type-mismatch | C++ returns const emSignal&. Rust returns generation u64; co… |
| `emFileManModel-accessor-selection` | selection | `FileMan->GetSelectionSignal()` | **gap-blocked** | absent | `emFileManModel.rs:540` | type-mismatch | C++ returns const emSignal&. Rust returns generation u64; co… |
| `emFileManModel-454` | model-update-broadcast | `FileUpdateSignalModel->Sig` | **unported** | absent | `emFileManModel.rs:519` | present | Model-internal: emFileManModel subscribes to the shared file… |

### `emFileManViewConfig.rs` (↔ `emFileManViewConfig.cpp`)

Rows: 2 — faithful: 0, drifted: 0, forced: 0, gap-blocked: 1, unported: 1

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emFileManViewConfig-accessor-config-change` | config-change | `Config->GetChangeSignal()` | **gap-blocked** | absent | `emFileManViewConfig.rs:428` | type-mismatch | C++ emFileManViewConfig::GetChangeSignal() returns const emS… |
| `emFileManViewConfig-281` | config-change | `FileManConfig->GetChangeSignal()` | **unported** | absent | `emFileManViewConfig.rs:350` | missing | emFileManConfig is the shared sibling (not emFileManViewConf… |

### `emStocksPricesFetcher.rs` (↔ `emStocksPricesFetcher.cpp`)

Rows: 3 — faithful: 0, drifted: 0, forced: 0, gap-blocked: 1, unported: 2

| ID | kind | signal | verdict | ev-kind | ev-loc | acc-status | notes |
|----|------|--------|---------|---------|--------|------------|-------|
| `emStocksPricesFetcher-accessor-model-change` | model-change | `Fetcher.GetChangeSignal()` | **gap-blocked** | absent | `emStocksPricesFetcher.rs:18` | missing |  |
| `emStocksPricesFetcher-38` | model-change | `FileModel->GetChangeSignal()` | **unported** | absent | `emStocksPricesFetcher.rs:273` | missing | Fetcher does not hold a FileModel ref in Rust; signal subscr… |
| `emStocksPricesFetcher-39` | model-state | `FileModel->GetFileStateSignal()` | **unported** | absent | `emStocksPricesFetcher.rs:273` | present | Note: emStocksFileModel does not derive from emFileModel in … |

