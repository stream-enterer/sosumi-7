// Port of C++ emStocksFilePanel.h / emStocksFilePanel.cpp

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use emcore::emColor::emColor;
#[cfg(test)]
use emcore::emEngineCtx::DropOnlySignalCtx;
use emcore::emEngineCtx::PanelCtx;
use emcore::emFilePanel::emFilePanel;
use emcore::emInput::{emInputEvent, InputKey, InputVariant};
use emcore::emInputState::emInputState;
use emcore::emLook::emLook;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};
use emcore::emSignal::SignalId;

use super::emStocksConfig::{emStocksConfig, Sorting};
use super::emStocksFetchPricesDialog::emStocksFetchPricesDialog;
use super::emStocksFileModel::emStocksFileModel;
use super::emStocksListBox::emStocksListBox;
use super::emStocksRec::{emStocksRec, Interest};

/// Best-available `&mut emStocksRec` access for `Input` handlers. Production
/// `PanelCtx` instances under `PanelCycleEngine` carry full scheduler reach
/// (`as_sched_ctx() -> Some`), so the threaded ctx fires the synchronous
/// `ChangeSignal` per D-007. The else branch is reachable only from layout-
/// only / unit-test `PanelCtx` constructions; a regression that loses
/// scheduler reach in production must fail loudly rather than silently drop
/// ChangeSignal fires.
fn writable_rec<'a>(model: &'a mut emStocksFileModel, ctx: &mut PanelCtx) -> &'a mut emStocksRec {
    if let Some(mut sc) = ctx.as_sched_ctx() {
        model.GetWritableRec(&mut sc)
    } else {
        #[cfg(not(test))]
        panic!(
            "emStocksFilePanel::Input: PanelCtx::as_sched_ctx() returned None — \
             production PanelCycleEngine must thread scheduler reach (D-007)"
        );
        // Test-only path: ChangeSignal is necessarily null (no subscriber has
        // reached GetChangeSignal with a real ctx); the dropped fire is a
        // no-op per C++ "Signal()-with-zero-subscribers".
        #[cfg(test)]
        {
            let mut null = DropOnlySignalCtx;
            model.GetWritableRec(&mut null)
        }
    }
}

/// Port of C++ emStocksFilePanel.
pub struct emStocksFilePanel {
    pub(crate) bg_color: emColor,
    /// Cross-Cycle-shared config per CLAUDE.md §Ownership rule (a) — the
    /// emStocksListBox holds a clone of this `Rc<RefCell<>>` (Phase 3 of
    /// B-001) so its own `Cycle` can subscribe to `Config::GetChangeSignal`
    /// without being passed it per-call. Mirrors C++ `emStocksConfig & Config;`
    /// member reference on `emStocksFilePanel`/`emStocksListBox`.
    pub(crate) config: Rc<RefCell<emStocksConfig>>,
    pub(crate) fetch_dialog: Option<emStocksFetchPricesDialog>,
    /// Cross-Cycle-shared ListBox per CLAUDE.md §Ownership rule (a) — co-borrowed
    /// by `emStocksFilePanel::Cycle` and (B-001-followup Phase A) by
    /// `emStocksControlPanel::Cycle`, which holds a clone to mirror the C++
    /// `emStocksListBox & ListBox;` member reference.
    pub(crate) list_box: Option<Rc<RefCell<emStocksListBox>>>,
    /// Cross-Cycle-shared model per CLAUDE.md §Ownership rule (a) — same
    /// rationale as `config`. Mirrors C++ `emStocksFileModel & FileModel;`.
    pub(crate) model: Rc<RefCell<emStocksFileModel>>,
    pub(crate) file_panel: emFilePanel,
    /// B-017 row 2: cached `emFilePanel::GetVirFileStateSignal()` from the
    /// embedded `file_panel`. Captured at first-Cycle init time. Mirrors C++
    /// emStocksFilePanel.cpp:34 `AddWakeUpSignal(GetVirFileStateSignal())`.
    vir_file_state_sig: Option<SignalId>,
    /// D-006 first-Cycle init flag. Set after the panel allocates its model's
    /// SaveTimer infrastructure and connects subscribes.
    subscribed_init: bool,
    /// B-001 row `emStocksFilePanel-255` — deferred-attach init flag for the
    /// `ListBox::GetSelectedDateSignal` subscribe. Set true once the ListBox
    /// has been materialised (VFS Loaded) and the panel's engine has been
    /// connected to the signal. Mirrors C++
    /// `emStocksFilePanel.cpp:255 AddWakeUpSignal(ListBox->GetSelectedDateSignal())`,
    /// which lives inside `UpdateControls` immediately after the new
    /// `emStocksListBox` is constructed — i.e. attach-deferred per the design's
    /// "list_box_subscribed" two-tier pattern (§Sequencing → "Lazy-attached
    /// widgets / ListBox").
    selected_date_subscribed: bool,
    /// Cached `SelectedDateSignal` id captured at attach-time so the
    /// IsSignaled gate below the init block can read it without re-borrowing
    /// `self.list_box`.
    selected_date_sig: Option<SignalId>,
}

impl PanelBehavior for emStocksFilePanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        _canvas_color: emColor,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        if self.file_panel.GetVirFileState().is_good() {
            // C++: painter.Clear(BgColor, canvasColor) — canvasColor not passed
            // because Rust emPainter::Clear takes only one color argument.
            painter.Clear(self.bg_color);

            // C++: ListBox->Paint(...) checks GetItemCount()==0 and paints
            // "empty stock list" message.
            if let Some(lb) = self.list_box.as_ref() {
                lb.borrow().PaintEmptyMessage(painter, w, h, self.bg_color);
            }
        }
        // C++: if (!IsVFSGood()) emFilePanel::Paint(painter,canvasColor);
        // Base class paint for non-good state deferred until emFilePanel integration.
    }

    fn IsOpaque(&self) -> bool {
        // C++: if (!IsVFSGood()) return emFilePanel::IsOpaque(); else return false;
        false
    }

    fn Input(
        &mut self,
        event: &emInputEvent,
        _state: &PanelState,
        input_state: &emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        if !self.file_panel.GetVirFileState().is_good() || self.list_box.is_none() {
            return false;
        }
        if event.IsEmpty() || event.variant != InputVariant::Press {
            return false;
        }

        // ── Shift+Alt shortcuts: interest filter and sorting ──
        if input_state.IsShiftAltMod() {
            match event.key {
                // Interest filter
                InputKey::Key('H') => {
                    self.config.borrow_mut().min_visible_interest = Interest::High;
                    return true;
                }
                InputKey::Key('M') => {
                    self.config.borrow_mut().min_visible_interest = Interest::Medium;
                    return true;
                }
                InputKey::Key('L') => {
                    self.config.borrow_mut().min_visible_interest = Interest::Low;
                    return true;
                }
                // Sorting
                InputKey::Key('N') => {
                    self.config.borrow_mut().sorting = Sorting::ByName;
                    return true;
                }
                InputKey::Key('T') => {
                    self.config.borrow_mut().sorting = Sorting::ByTradeDate;
                    return true;
                }
                InputKey::Key('I') => {
                    self.config.borrow_mut().sorting = Sorting::ByInquiryDate;
                    return true;
                }
                InputKey::Key('A') => {
                    self.config.borrow_mut().sorting = Sorting::ByAchievement;
                    return true;
                }
                InputKey::Key('1') => {
                    self.config.borrow_mut().sorting = Sorting::ByOneWeekRise;
                    return true;
                }
                InputKey::Key('3') => {
                    self.config.borrow_mut().sorting = Sorting::ByThreeWeekRise;
                    return true;
                }
                InputKey::Key('9') => {
                    self.config.borrow_mut().sorting = Sorting::ByNineWeekRise;
                    return true;
                }
                InputKey::Key('D') => {
                    self.config.borrow_mut().sorting = Sorting::ByDividend;
                    return true;
                }
                InputKey::Key('P') => {
                    self.config.borrow_mut().sorting = Sorting::ByPurchaseValue;
                    return true;
                }
                InputKey::Key('V') => {
                    self.config.borrow_mut().sorting = Sorting::ByValue;
                    return true;
                }
                InputKey::Key('F') => {
                    self.config.borrow_mut().sorting = Sorting::ByDifference;
                    return true;
                }
                // OwnedSharesFirst toggle
                InputKey::Key('O') => {
                    let mut c = self.config.borrow_mut();
                    c.owned_shares_first = !c.owned_shares_first;
                    return true;
                }
                _ => {}
            }
        }

        // ── Ctrl shortcuts: ListBox operations ──
        if input_state.IsCtrlMod() {
            match event.key {
                InputKey::Key('J') => {
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let model = self.model.borrow();
                        // B-001 G4: GoBackInHistory may fire SelectedDateSignal.
                        // Mirrors `writable_rec` PanelCtx-as_sched_ctx pattern.
                        if let Some(mut sc) = ctx.as_sched_ctx() {
                            lb.GoBackInHistory(&mut sc, model.GetRec());
                        } else {
                            #[cfg(not(test))]
                            panic!(
                                "emStocksFilePanel::Input: PanelCtx::as_sched_ctx() returned None — \
                                 production PanelCycleEngine must thread scheduler reach (D-007)"
                            );
                            #[cfg(test)]
                            {
                                let mut null = emcore::emEngineCtx::DropOnlySignalCtx;
                                lb.GoBackInHistory(&mut null, model.GetRec());
                            }
                        }
                    }
                    return true;
                }
                InputKey::Key('K') => {
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let model = self.model.borrow();
                        if let Some(mut sc) = ctx.as_sched_ctx() {
                            lb.GoForwardInHistory(&mut sc, model.GetRec());
                        } else {
                            #[cfg(not(test))]
                            panic!(
                                "emStocksFilePanel::Input: PanelCtx::as_sched_ctx() returned None — \
                                 production PanelCycleEngine must thread scheduler reach (D-007)"
                            );
                            #[cfg(test)]
                            {
                                let mut null = emcore::emEngineCtx::DropOnlySignalCtx;
                                lb.GoForwardInHistory(&mut null, model.GetRec());
                            }
                        }
                    }
                    return true;
                }
                InputKey::Key('N') => {
                    // C++: ListBox->NewStock()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let mut model = self.model.borrow_mut();
                        let config = self.config.borrow();
                        let rec = writable_rec(&mut model, ctx);
                        lb.NewStock(rec, &config);
                    }
                    return true;
                }
                InputKey::Key('X') => {
                    // C++: ListBox->CutStocks()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let mut model = self.model.borrow_mut();
                        let rec = writable_rec(&mut model, ctx);
                        lb.CutStocks(ctx, rec, false);
                    }
                    return true;
                }
                InputKey::Key('C') => {
                    // C++: ListBox->CopyStocks()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        lb_rc.borrow_mut().CopyStocks(self.model.borrow().GetRec());
                    }
                    return true;
                }
                InputKey::Key('V') => {
                    // C++: ListBox->PasteStocks()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let mut model = self.model.borrow_mut();
                        let config = self.config.borrow();
                        let rec = writable_rec(&mut model, ctx);
                        let _ = lb.PasteStocks(ctx, rec, &config, false);
                    }
                    return true;
                }
                InputKey::Key('P') => {
                    // C++: ListBox->StartToFetchSharePrices()
                    // Collect ids first (ends the borrow of list_box) so we can
                    // mutate self.fetch_dialog afterwards without a conflict.
                    let ids = self
                        .list_box
                        .as_ref()
                        .map(|lb| lb.borrow().GetVisibleStockIds(self.model.borrow().GetRec()))
                        .unwrap_or_default();
                    if !ids.is_empty() {
                        let mut dialog = {
                            let cfg = self.config.borrow();
                            // B-001-followup Phase E.1: wire the FileModel ref
                            // into the dialog so the fetcher's proxy-engine
                            // `cycle()` can subscribe to FileModel signals
                            // (cpp:38-39).
                            emStocksFetchPricesDialog::new_with_model(
                                &cfg.api_script,
                                &cfg.api_script_interpreter,
                                &cfg.api_key,
                                self.model.clone(),
                            )
                        };
                        // B-001 G3: AddStockIds fires `Signal(ChangeSignal)`.
                        // Mirrors `writable_rec` precedent for PanelCtx ectx-reach.
                        if let Some(mut sc) = ctx.as_sched_ctx() {
                            dialog.AddStockIds(&mut sc, &ids);
                        } else {
                            #[cfg(not(test))]
                            panic!(
                                "emStocksFilePanel::Input: PanelCtx::as_sched_ctx() returned None — \
                                 production PanelCycleEngine must thread scheduler reach (D-007)"
                            );
                            #[cfg(test)]
                            {
                                let mut null = emcore::emEngineCtx::DropOnlySignalCtx;
                                dialog.AddStockIds(&mut null, &ids);
                            }
                        }
                        self.fetch_dialog = Some(dialog);
                    }
                    return true;
                }
                InputKey::Key('W') => {
                    // C++: ListBox->ShowFirstWebPages()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        lb_rc
                            .borrow_mut()
                            .ShowFirstWebPages(self.model.borrow().GetRec());
                    }
                    return true;
                }
                InputKey::Key('H') => {
                    // C++: ListBox->FindSelected()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let model = self.model.borrow();
                        let mut config = self.config.borrow_mut();
                        let _found = lb.FindSelected(model.GetRec(), &mut config);
                    }
                    return true;
                }
                InputKey::Key('G') => {
                    // C++: ListBox->FindNext()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let model = self.model.borrow();
                        let config = self.config.borrow();
                        let _found = lb.FindNext(model.GetRec(), &config);
                    }
                    return true;
                }
                _ => {}
            }
        }

        // ── Shift+Ctrl shortcuts ──
        if input_state.IsShiftCtrlMod() {
            match event.key {
                InputKey::Key('W') => {
                    // C++: ListBox->ShowAllWebPages()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        lb_rc
                            .borrow_mut()
                            .ShowAllWebPages(self.model.borrow().GetRec());
                    }
                    return true;
                }
                InputKey::Key('G') => {
                    // C++: ListBox->FindPrevious()
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let model = self.model.borrow();
                        let config = self.config.borrow();
                        let _found = lb.FindPrevious(model.GetRec(), &config);
                    }
                    return true;
                }
                _ => {}
            }
        }

        // ── No-modifier shortcuts ──
        if input_state.IsNoMod() && event.key == InputKey::Delete {
            // C++: ListBox->DeleteStocks()
            if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                let mut lb = lb_rc.borrow_mut();
                let mut model = self.model.borrow_mut();
                let rec = writable_rec(&mut model, ctx);
                lb.DeleteStocks(ctx, rec, false);
            }
            return true;
        }

        // ── Alt shortcuts: set interest on selected stocks ──
        if input_state.IsAltMod() {
            match event.key {
                InputKey::Key('H') => {
                    // C++: ListBox->SetInterest(HIGH_INTEREST)
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let mut model = self.model.borrow_mut();
                        let rec = writable_rec(&mut model, ctx);
                        lb.SetInterest(ctx, rec, Interest::High, false);
                    }
                    return true;
                }
                InputKey::Key('M') => {
                    // C++: ListBox->SetInterest(MEDIUM_INTEREST)
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let mut model = self.model.borrow_mut();
                        let rec = writable_rec(&mut model, ctx);
                        lb.SetInterest(ctx, rec, Interest::Medium, false);
                    }
                    return true;
                }
                InputKey::Key('L') => {
                    // C++: ListBox->SetInterest(LOW_INTEREST)
                    if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                        let mut lb = lb_rc.borrow_mut();
                        let mut model = self.model.borrow_mut();
                        let rec = writable_rec(&mut model, ctx);
                        lb.SetInterest(ctx, rec, Interest::Low, false);
                    }
                    return true;
                }
                _ => {}
            }
        }

        false
    }

    fn LayoutChildren(&mut self, ctx: &mut PanelCtx) {
        // C++: if (ListBox) ListBox->Layout(0.0, 0.0, 1.0, GetHeight(), BgColor);
        // ListBox is not a registered panel child (it's a struct field), so we
        // store the layout rect on it for use during painting.
        if let Some(lb_rc) = self.list_box.as_ref() {
            let rect = ctx.layout_rect();
            let mut list_box = lb_rc.borrow_mut();
            list_box.layout_x = 0.0;
            list_box.layout_y = 0.0;
            list_box.layout_w = rect.w; // width = 1.0
            list_box.layout_h = rect.h; // height = tallness
        }
    }

    fn Cycle(
        &mut self,
        ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        // B-017 row 2 + row 3: D-006 first-Cycle init.
        // Mirrors C++ emStocksFilePanel ctor `AddWakeUpSignal(GetVirFileStateSignal())`
        // (emStocksFilePanel.cpp:34) AND C++ emStocksFileModel ctor
        // `AddWakeUpSignal(SaveTimer.GetSignal())` (emStocksFileModel.cpp:21).
        // The model's SaveTimer subscribe is hosted on the panel because the
        // embedded model has no independent scheduler reach — see the
        // `emStocksFileModel` DIVERGED block for the I-3 by-value + proxy-engine
        // rationale.
        if !self.subscribed_init {
            let eid = ectx.engine_id;

            // Row 2: subscribe to file_panel.GetVirFileStateSignal().
            // The signal is allocated by `emFilePanel::Cycle` on its own
            // first-cycle prefix (`ensure_vir_file_state_signal`); we read it
            // here and capture for the IsSignaled gate. Note: `emStocksFilePanel`
            // does NOT delegate to `emFilePanel::Cycle` (composition shape — the
            // panel does not embed an `emFilePanel` in a way that lets us call
            // its `Cycle` from here), so we allocate the VFS signal directly via
            // the public `ensure_vir_file_state_signal` helper.
            let vfs_sig = self.file_panel.ensure_vir_file_state_signal(ectx);
            ectx.connect(vfs_sig, eid);
            self.vir_file_state_sig = Some(vfs_sig);

            // Row 3: allocate SaveTimer signal/timer on the model and connect
            // the panel's engine to it. After this point IsSignaled drives Save.
            self.model.borrow_mut().ensure_save_timer(ectx, eid);

            self.subscribed_init = true;
        }

        // Row 2 reaction: refresh VirFileState only when its signal fires.
        // Mirrors C++ emStocksFilePanel.cpp:60-65
        // `if (IsSignaled(GetVirFileStateSignal())) UpdateControls();` — the
        // Rust analogue of `UpdateControls` is the `refresh_vir_file_state`
        // re-read + lazy `list_box` materialization on Loaded.
        let vfs_fired = self
            .vir_file_state_sig
            .map(|s| ectx.IsSignaled(s))
            .unwrap_or(false);
        let mut state_changed = false;
        if vfs_fired {
            let old_state = self.file_panel.GetVirFileState();
            self.file_panel.refresh_vir_file_state();
            let new_state = self.file_panel.GetVirFileState();
            state_changed = old_state != new_state;
            if state_changed && new_state.is_good() && self.list_box.is_none() {
                let mut lb = emStocksListBox::new();
                // Phase 3: hand the ListBox cross-Cycle refs to the model and
                // config so its own Cycle (Phase 4.5) can subscribe to their
                // ChangeSignals without being passed them per-call. Mirrors
                // C++ emStocksListBox member references.
                lb.set_refs(self.model.clone(), self.config.clone());
                // B-001 rows -51 / -52 — pre-allocate the FileModel and
                // Config ChangeSignal ids and connect the panel's engine
                // (which is what drives `lb.Cycle` via the call site below).
                // Doing the allocation here — rather than from inside
                // `lb.Cycle` — sidesteps the runtime double-borrow of the
                // parent's `Rc<RefCell<>>`s (the panel already holds
                // `model.borrow_mut()` / `config.borrow()` across the
                // `lb.Cycle(...)` call below; see `wire_change_signals`
                // doc for full rationale).
                let eid = ectx.engine_id;
                let model_sig = self.model.borrow().GetChangeSignal(ectx);
                ectx.connect(model_sig, eid);
                let cfg_sig = self.config.borrow().GetChangeSignal(ectx);
                ectx.connect(cfg_sig, eid);
                lb.wire_change_signals(model_sig, cfg_sig);
                self.list_box = Some(Rc::new(RefCell::new(lb)));
            }
        }

        // B-001 row -255 — deferred-attach subscribe to ListBox's
        // `SelectedDateSignal`. Mirrors C++
        // `emStocksFilePanel::UpdateControls` (cpp:254-255):
        //
        //     ListBox=new emStocksListBox(this,"",*FileModel,*Config);
        //     AddWakeUpSignal(ListBox->GetSelectedDateSignal());
        //
        // The Rust `list_box` is materialised in the VFS-fired branch above; on
        // the first Cycle slice that observes `list_box.is_some()` and has not
        // yet wired the connect, we allocate the SelectedDateSignal (lazy
        // D-008 A1 via `GetSelectedDateSignal(ectx)`) and connect the panel
        // engine. C++ has no separate `IsSignaled(GetSelectedDateSignal())`
        // branch in `emStocksFilePanel::Cycle` (cpp:58-69 only checks
        // `GetVirFileStateSignal()`); the wake-up alone is the contract — it
        // keeps the panel cycling so that downstream consumers (control panel,
        // ItemChart, etc.) drain their own subscribed reactions.
        if !self.selected_date_subscribed {
            if let Some(lb_rc) = self.list_box.as_ref() {
                let sig = lb_rc.borrow().GetSelectedDateSignal(ectx);
                ectx.connect(sig, ectx.engine_id);
                self.selected_date_sig = Some(sig);
                self.selected_date_subscribed = true;
            }
        }

        // Row 3 reaction: signal-driven Save. Replaces the previous per-frame
        // `model.CheckSaveTimer(ectx)` Instant-poll. Mirrors C++
        // emStocksFileModel.cpp:33-38
        // `if (IsSignaled(SaveTimer.GetSignal())) Save(true);`. The model's
        // SaveTimer signal is allocated above in the first-Cycle init.
        let save_fired = self
            .model
            .borrow()
            .save_timer_signal()
            .map(|s| ectx.IsSignaled(s))
            .unwrap_or(false);
        if save_fired {
            self.model.borrow_mut().save_on_timer_fire(ectx);
        }

        // Poll fetch dialog
        if let Some(ref mut dialog) = self.fetch_dialog {
            if !dialog.Cycle(ectx) {
                // Dialog finished — clean up
                self.fetch_dialog = None;
            }
        }

        // Poll ListBox confirmation dialogs (C++: Cycle calls into ListBox state machine)
        //
        // C-1 RESOLUTION (Adversarial Review 2026-05-01, B-017 row 2):
        // The previous shape `lb.Cycle(ectx, model.GetWritableRec(ectx), config)`
        // takes two `&mut ectx` borrows simultaneously. The split-borrow shape
        // below sequences them: first call the rec-mutation half of the split
        // `GetWritableRec` (which only sets `dirty`/`dirty_since_last_arm` and
        // returns `&mut emStocksRec` — no scheduler touch), drive `lb.Cycle`,
        // then after lb.Cycle returns advance the SaveTimer via the
        // `touch_save_timer(ectx)` half, gated on `dirty_since_last_touch()`.
        // This keeps `&mut ectx` exclusive across both halves.
        let list_box_busy = {
            if let Some(lb_rc) = self.list_box.as_ref().cloned() {
                let mut lb = lb_rc.borrow_mut();
                let mut model = self.model.borrow_mut();
                let config = self.config.borrow();
                // Rec-mutation half: takes only `&mut model`, no ectx use.
                let rec = model.GetWritableRec(ectx);
                lb.Cycle(ectx, rec, &config)
            } else {
                false
            }
        };

        // Timer-arming half, sequenced after `lb.Cycle` returns. Gated on
        // the paired latch so we re-arm only when lb.Cycle actually wrote
        // through GetWritableRec.
        let need_touch = self.model.borrow_mut().dirty_since_last_touch();
        if need_touch {
            self.model.borrow_mut().touch_save_timer(ectx);
        }

        state_changed || save_fired || self.fetch_dialog.is_some() || list_box_busy
    }

    fn GetIconFileName(&self) -> Option<String> {
        Some("documents.tga".to_string())
    }

    /// Port of C++ `emStocksFilePanel::CreateControlPanel`
    /// (emStocksFilePanel.cpp:237-247):
    /// ```text
    /// if (FileModel && ListBox) {
    ///     return new emStocksControlPanel(parent,name,*FileModel,*Config,*ListBox);
    /// } else {
    ///     return emFilePanel::CreateControlPanel(parent,name);
    /// }
    /// ```
    /// Returning `None` from the trait override delegates upward through the
    /// tree walker, which is the Rust analogue of falling back to the
    /// `emFilePanel::CreateControlPanel` parent virtual.
    fn CreateControlPanel(
        &mut self,
        parent_ctx: &mut emcore::emEngineCtx::PanelCtx,
        name: &str,
        _self_is_active: bool,
    ) -> Option<emcore::emPanelTree::PanelId> {
        // VFS-not-good path: delegate upstream (mirrors C++ `emFilePanel::
        // CreateControlPanel` fallback when FileModel is absent or ListBox has
        // not yet been materialised).
        if !self.file_panel.GetVirFileState().is_good() {
            return None;
        }
        let list_box = self.list_box.as_ref()?.clone();
        let cp = super::emStocksControlPanel::emStocksControlPanel::new(
            emLook::new(),
            self.model.clone(),
            self.config.clone(),
            list_box,
        );
        Some(parent_ctx.create_child_with(name, Box::new(cp)))
    }
}

impl emStocksFilePanel {
    pub(crate) fn new() -> Self {
        Self {
            bg_color: emColor::from_packed(0x131520FF),
            config: Rc::new(RefCell::new(emStocksConfig::default())),
            fetch_dialog: None,
            list_box: None,
            model: Rc::new(RefCell::new(emStocksFileModel::new(PathBuf::from("")))),
            file_panel: emFilePanel::new(),
            vir_file_state_sig: None,
            subscribed_init: false,
            selected_date_subscribed: false,
            selected_date_sig: None,
        }
    }
}

impl Default for emStocksFilePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl emStocksFilePanel {
    /// B-017 row 2 test accessor: cached `VirFileStateSignal` id, or `None`
    /// before the panel's first Cycle has populated it.
    #[doc(hidden)]
    pub fn vir_file_state_signal_for_test(&self) -> Option<SignalId> {
        self.vir_file_state_sig
    }

    /// B-001 row -255 test accessor: cached `SelectedDateSignal` id captured
    /// at the deferred-attach init step, or `None` before the ListBox has
    /// been materialised and the subscribe wired.
    #[doc(hidden)]
    pub fn selected_date_signal_for_test(&self) -> Option<SignalId> {
        self.selected_date_sig
    }

    /// B-001 row -255 test accessor: reports whether the ListBox's
    /// SelectedDate subscribe has been wired by `Cycle`.
    #[doc(hidden)]
    pub fn selected_date_subscribed_for_test(&self) -> bool {
        self.selected_date_subscribed
    }

    /// B-017 row 3 test accessor: `SaveTimer` signal id allocated on the
    /// embedded model. Null until the panel's first Cycle.
    #[doc(hidden)]
    pub fn save_timer_signal_for_test(&self) -> SignalId {
        self.model.borrow().save_timer_signal_for_test()
    }

    /// B-017 row 3 test accessor: dirty flag (whether there are pending writes).
    #[doc(hidden)]
    pub fn model_dirty_for_test(&self) -> bool {
        self.model.borrow().dirty_for_test()
    }

    /// B-017 row 3 test accessor: mutate the model's rec, marking dirty.
    /// Threads `&mut impl SignalCtx` like the production `GetWritableRec`
    /// half-mutator. Used by external tests that cannot reach `pub(crate) model`.
    #[doc(hidden)]
    pub fn mark_rec_dirty_for_test<C: emcore::emEngineCtx::SignalCtx>(&mut self, ectx: &mut C) {
        let _ = self.model.borrow_mut().GetWritableRec(ectx);
    }
}

#[cfg(test)]
impl emStocksFilePanel {
    pub(crate) fn set_vfs_good_for_test(&mut self) {
        use emcore::emFileModel::{emFileModel, FileModelState};
        use emcore::emSignal::SignalId;
        use std::cell::RefCell;
        use std::path::PathBuf;
        use std::rc::Rc;
        let model = Rc::new(RefCell::new(emFileModel::<String>::new(
            PathBuf::from("/tmp/test"),
            SignalId::default(),
        )));
        model.borrow_mut().complete_load("test".to_string());
        self.file_panel
            .SetFileModel(Some(model as Rc<RefCell<dyn FileModelState>>));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emPanelTree::{PanelId, PanelTree};

    fn make_test_pctx<'a>(tree: &'a mut PanelTree, id: PanelId) -> PanelCtx<'a> {
        PanelCtx::new(tree, id, 1.0)
    }

    fn make_test_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("test", false);
        (tree, root)
    }

    #[test]
    fn file_panel_new() {
        let panel = emStocksFilePanel::new();
        assert_eq!(panel.bg_color, emColor::from_packed(0x131520FF));
    }

    #[test]
    fn file_panel_icon() {
        let panel = emStocksFilePanel::new();
        assert_eq!(panel.GetIconFileName(), Some("documents.tga".to_string()));
    }

    #[test]
    fn is_opaque_returns_false() {
        let panel = emStocksFilePanel::new();
        assert!(!panel.IsOpaque());
    }

    #[test]
    fn input_returns_false_when_vfs_not_good() {
        let (mut tree, root) = make_test_tree();
        let mut panel = emStocksFilePanel::new();
        panel.list_box = Some(Rc::new(RefCell::new(emStocksListBox::new())));
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('H'));
        let state = PanelState::default_for_test();
        assert!(!panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
    }

    #[test]
    fn input_returns_false_when_no_listbox() {
        let (mut tree, root) = make_test_tree();
        let mut panel = emStocksFilePanel::new();
        panel.set_vfs_good_for_test();
        panel.list_box = None;
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('H'));
        let state = PanelState::default_for_test();
        assert!(!panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
    }

    fn make_active_panel() -> emStocksFilePanel {
        let mut panel = emStocksFilePanel::new();
        panel.set_vfs_good_for_test();
        panel.list_box = Some(Rc::new(RefCell::new(emStocksListBox::new())));
        panel
    }

    #[test]
    fn shift_alt_h_sets_high_interest_filter() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('H'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert_eq!(panel.config.borrow().min_visible_interest, Interest::High);
    }

    #[test]
    fn shift_alt_m_sets_medium_interest_filter() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('M'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert_eq!(panel.config.borrow().min_visible_interest, Interest::Medium);
    }

    #[test]
    fn shift_alt_l_sets_low_interest_filter() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('L'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert_eq!(panel.config.borrow().min_visible_interest, Interest::Low);
    }

    #[test]
    fn shift_alt_n_sets_sort_by_name() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        panel.config.borrow_mut().sorting = Sorting::ByValue; // set non-default
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('N'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert_eq!(panel.config.borrow().sorting, Sorting::ByName);
    }

    #[test]
    fn shift_alt_sorting_keys() {
        let (mut tree, root) = make_test_tree();
        let cases: Vec<(char, Sorting)> = vec![
            ('T', Sorting::ByTradeDate),
            ('I', Sorting::ByInquiryDate),
            ('A', Sorting::ByAchievement),
            ('1', Sorting::ByOneWeekRise),
            ('3', Sorting::ByThreeWeekRise),
            ('9', Sorting::ByNineWeekRise),
            ('D', Sorting::ByDividend),
            ('P', Sorting::ByPurchaseValue),
            ('V', Sorting::ByValue),
            ('F', Sorting::ByDifference),
        ];
        for (key, expected_sorting) in cases {
            let mut panel = make_active_panel();
            let mut input_state = emInputState::new();
            input_state.press(InputKey::Shift);
            input_state.press(InputKey::Alt);
            let event = emInputEvent::press(InputKey::Key(key));
            let state = PanelState::default_for_test();
            assert!(
                panel.Input(
                    &event,
                    &state,
                    &input_state,
                    &mut make_test_pctx(&mut tree, root)
                ),
                "Shift+Alt+{key} should consume"
            );
            assert_eq!(
                panel.config.borrow().sorting,
                expected_sorting,
                "Shift+Alt+{key} should set {expected_sorting:?}"
            );
        }
    }

    #[test]
    fn shift_alt_o_toggles_owned_shares_first() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        assert!(!panel.config.borrow().owned_shares_first);
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::press(InputKey::Key('O'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert!(panel.config.borrow().owned_shares_first);
        // Toggle back
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert!(!panel.config.borrow().owned_shares_first);
    }

    #[test]
    fn ctrl_j_goes_back_in_history() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        // Set up rec with dates so GoBackInHistory works
        let mut stock = crate::emStocksRec::StockRec::default();
        stock.AddPrice("2024-06-14", "100");
        stock.AddPrice("2024-06-15", "101");
        let mut null = DropOnlySignalCtx;
        panel
            .model
            .borrow_mut()
            .GetWritableRec(&mut null)
            .stocks
            .push(stock);
        panel
            .list_box
            .as_ref()
            .unwrap()
            .borrow_mut()
            .SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-15");

        let mut input_state = emInputState::new();
        input_state.press(InputKey::Ctrl);
        let event = emInputEvent::press(InputKey::Key('J'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert_eq!(
            panel.list_box.as_ref().unwrap().borrow().GetSelectedDate(),
            "2024-06-14"
        );
    }

    #[test]
    fn ctrl_k_goes_forward_in_history() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let mut stock = crate::emStocksRec::StockRec::default();
        stock.AddPrice("2024-06-14", "100");
        stock.AddPrice("2024-06-15", "101");
        let mut null = DropOnlySignalCtx;
        panel
            .model
            .borrow_mut()
            .GetWritableRec(&mut null)
            .stocks
            .push(stock);
        panel
            .list_box
            .as_ref()
            .unwrap()
            .borrow_mut()
            .SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-14");

        let mut input_state = emInputState::new();
        input_state.press(InputKey::Ctrl);
        let event = emInputEvent::press(InputKey::Key('K'));
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert_eq!(
            panel.list_box.as_ref().unwrap().borrow().GetSelectedDate(),
            "2024-06-15"
        );
    }

    #[test]
    fn ctrl_shortcuts_consume_events() {
        let (mut tree, root) = make_test_tree();
        let ctrl_keys = ['N', 'X', 'C', 'V', 'P', 'W', 'H', 'G'];
        for key in ctrl_keys {
            let mut panel = make_active_panel();
            let mut input_state = emInputState::new();
            input_state.press(InputKey::Ctrl);
            let event = emInputEvent::press(InputKey::Key(key));
            let state = PanelState::default_for_test();
            assert!(
                panel.Input(
                    &event,
                    &state,
                    &input_state,
                    &mut make_test_pctx(&mut tree, root)
                ),
                "Ctrl+{key} should consume"
            );
        }
    }

    #[test]
    fn shift_ctrl_shortcuts_consume_events() {
        let (mut tree, root) = make_test_tree();
        let keys = ['W', 'G'];
        for key in keys {
            let mut panel = make_active_panel();
            let mut input_state = emInputState::new();
            input_state.press(InputKey::Shift);
            input_state.press(InputKey::Ctrl);
            let event = emInputEvent::press(InputKey::Key(key));
            let state = PanelState::default_for_test();
            assert!(
                panel.Input(
                    &event,
                    &state,
                    &input_state,
                    &mut make_test_pctx(&mut tree, root)
                ),
                "Shift+Ctrl+{key} should consume"
            );
        }
    }

    #[test]
    fn delete_consumes_event() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let input_state = emInputState::new();
        let event = emInputEvent::press(InputKey::Delete);
        let state = PanelState::default_for_test();
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
    }

    #[test]
    fn alt_interest_shortcuts_consume_events() {
        let (mut tree, root) = make_test_tree();
        let keys = ['H', 'M', 'L'];
        for key in keys {
            let mut panel = make_active_panel();
            let mut input_state = emInputState::new();
            input_state.press(InputKey::Alt);
            let event = emInputEvent::press(InputKey::Key(key));
            let state = PanelState::default_for_test();
            assert!(
                panel.Input(
                    &event,
                    &state,
                    &input_state,
                    &mut make_test_pctx(&mut tree, root)
                ),
                "Alt+{key} should consume"
            );
        }
    }

    #[test]
    fn unrecognized_key_not_consumed() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let input_state = emInputState::new();
        let event = emInputEvent::press(InputKey::Key('Z'));
        let state = PanelState::default_for_test();
        assert!(!panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
    }

    // ── B-001-followup A.4 — CreateControlPanel factory ─────────────────
    /// Mirrors C++ emStocksFilePanel.cpp:237-247 — VFS-good path with a
    /// materialised ListBox returns a freshly constructed
    /// `emStocksControlPanel`. The returned `PanelId` is a child of the
    /// supplied parent.
    #[test]
    fn create_control_panel_returns_some_when_vfs_good_and_listbox_set() {
        let mut tree = PanelTree::new();
        let parent = tree.create_root("fp", false);
        let mut panel = emStocksFilePanel::new();
        panel.set_vfs_good_for_test();
        panel.list_box = Some(Rc::new(RefCell::new(emStocksListBox::new())));

        let mut pctx = PanelCtx::new(&mut tree, parent, 1.0);
        let cp_id = panel.CreateControlPanel(&mut pctx, "ctrl", false);
        assert!(
            cp_id.is_some(),
            "VFS-good FilePanel with materialised ListBox must yield a ControlPanel"
        );
    }

    /// Mirrors C++ fallback path: no ListBox → return `None` so the tree
    /// walker falls back to `emFilePanel::CreateControlPanel`.
    #[test]
    fn create_control_panel_returns_none_without_listbox() {
        let mut tree = PanelTree::new();
        let parent = tree.create_root("fp", false);
        let mut panel = emStocksFilePanel::new();
        panel.set_vfs_good_for_test();
        panel.list_box = None;

        let mut pctx = PanelCtx::new(&mut tree, parent, 1.0);
        let cp_id = panel.CreateControlPanel(&mut pctx, "ctrl", false);
        assert!(
            cp_id.is_none(),
            "missing ListBox must return None to delegate upstream"
        );
    }

    /// VFS-not-good path returns None to delegate to the parent emFilePanel
    /// virtual.
    #[test]
    fn create_control_panel_returns_none_when_vfs_not_good() {
        let mut tree = PanelTree::new();
        let parent = tree.create_root("fp", false);
        let mut panel = emStocksFilePanel::new();
        // No set_vfs_good — file_panel state defaults to !is_good().
        panel.list_box = Some(Rc::new(RefCell::new(emStocksListBox::new())));

        let mut pctx = PanelCtx::new(&mut tree, parent, 1.0);
        let cp_id = panel.CreateControlPanel(&mut pctx, "ctrl", false);
        assert!(
            cp_id.is_none(),
            "VFS-not-good must return None to delegate upstream"
        );
    }

    // ── B-001 row -255 — SelectedDate subscribe (deferred-attach) ─────
    #[test]
    fn cycle_wires_selected_date_subscribe_after_listbox_materialises() {
        // Mirrors C++ emStocksFilePanel::UpdateControls (cpp:254-255):
        //   ListBox=new emStocksListBox(this,"",*FileModel,*Config);
        //   AddWakeUpSignal(ListBox->GetSelectedDateSignal());
        //
        // The in-crate path: pre-set list_box + run a Cycle. The Cycle's
        // deferred-attach init must allocate and connect SelectedDateSignal.
        use emcore::emEngine::Priority;
        use emcore::emPanelScope::PanelScope;
        use emcore::test_view_harness::TestViewHarness;

        struct NoopEngine;
        impl emcore::emEngine::emEngine for NoopEngine {
            fn Cycle(&mut self, _ctx: &mut emcore::emEngineCtx::EngineCtx<'_>) -> bool {
                false
            }
        }

        let mut h = TestViewHarness::new();
        let mut panel = emStocksFilePanel::default();
        let eid = h.scheduler.register_engine(
            Box::new(NoopEngine),
            Priority::Medium,
            PanelScope::Framework,
        );

        // Inject the list_box directly (skipping the VFS-Loaded materialise
        // path that is also tested elsewhere). The B-001-255 contract under
        // test is "Cycle wires the subscribe iff list_box is Some".
        panel.list_box = Some(Rc::new(RefCell::new(emStocksListBox::new())));

        // Pre-condition.
        assert!(!panel.selected_date_subscribed_for_test());
        assert!(panel.selected_date_signal_for_test().is_none());

        // First Cycle.
        let (mut tree, root) = make_test_tree();
        {
            let mut pctx = make_test_pctx(&mut tree, root);
            let mut ectx = h.engine_ctx(eid);
            let _ = panel.Cycle(&mut ectx, &mut pctx);
        }

        assert!(
            panel.selected_date_subscribed_for_test(),
            "selected_date_subscribed must flip to true on the Cycle slice that observes list_box.is_some()"
        );
        assert!(
            panel.selected_date_signal_for_test().is_some(),
            "selected_date_sig must be Some(_) after subscribe"
        );

        h.scheduler.remove_engine(eid);
        let mut eids: Vec<emcore::emEngine::EngineId> =
            h.scheduler.engines_for_scope(PanelScope::Framework);
        for wid in h.windows.keys().copied().collect::<Vec<_>>() {
            eids.extend(h.scheduler.engines_for_scope(PanelScope::Toplevel(wid)));
        }
        for eid in eids {
            h.scheduler.remove_engine(eid);
        }
        h.scheduler.flush_signals_for_test();
    }

    #[test]
    fn release_events_not_consumed() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        let mut input_state = emInputState::new();
        input_state.press(InputKey::Shift);
        input_state.press(InputKey::Alt);
        let event = emInputEvent::release(InputKey::Key('H'));
        let state = PanelState::default_for_test();
        assert!(!panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
    }
}
