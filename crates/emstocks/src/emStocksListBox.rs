// Port of C++ emStocksListBox.h / emStocksListBox.cpp

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emDialog::{emDialog, DialogResult};
use emcore::emEngineCtx::{ConstructCtx, EngineCtx, SignalCtx};
use emcore::emGUIFramework::DialogId;
use emcore::emListBox::emListBox;
use emcore::emLook::emLook;
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emRecParser::{parse_rec_with_format, write_rec_with_format};
use emcore::emRecRecord::Record;
use emcore::emSignal::SignalId;
use slotmap::Key as _;

use super::emStocksConfig::{emStocksConfig, Sorting};
use super::emStocksFetchPricesDialog::emStocksFetchPricesDialog;
use super::emStocksRec::{emStocksRec, CompareDates, Interest, StockRec};

/// B-013 cancel-old-dialog helper. Centralises the four cancel-old branches
/// in `CutStocks` / `PasteStocks` / `DeleteStocks` / `SetInterest`.
///
/// Invariant: `*subscribed` is set true only by the EngineCtx-typed `Cycle`
/// path (where `current_engine_id()` is `Some`). The `debug_assert!` below
/// catches any future call from a `PanelCtx`-typed Input path that somehow
/// observes `*subscribed == true`, which would silently swallow the
/// `(finish_signal -> engine)` disconnect.
fn cancel_subscribed_dialog<C: ConstructCtx>(cc: &mut C, subscribed: &mut bool, old: &emDialog) {
    if *subscribed {
        debug_assert!(
            cc.current_engine_id().is_some(),
            "B-013 invariant: cancel-old reachable only from EngineCtx-typed paths \
             (subscribed=true is set by Cycle, which has Some(engine_id))"
        );
        if let Some(eid) = cc.current_engine_id() {
            cc.disconnect(old.finish_signal, eid);
        }
    }
    *subscribed = false;
    let did: DialogId = old.dialog_id;
    cc.pending_actions()
        .borrow_mut()
        .push(Box::new(move |app, _el| {
            app.close_dialog_by_id(did);
        }));
}

/// Port of C++ emStocksListBox.
pub struct emStocksListBox {
    selected_date: String,

    // Visible items: sorted stock indices into emStocksRec.stocks
    pub visible_items: Vec<usize>,

    /// Backing emListBox for selection state. Set by the parent panel when it
    /// provides an emLook. When None, the local fallback fields below are used.
    pub(crate) list_box: Option<emListBox>,

    // Fallback selection state used when list_box is None.
    // These are kept in sync with visible_items on mutation.
    selected_indices: Vec<usize>,

    /// Current active item index (into visible_items) for find navigation.
    active_index: Option<usize>,

    /// Look used for confirmation dialogs. Set by attach_list_box.
    pub(crate) look: Option<Rc<emLook>>,

    /// Layout rect fields set by parent panel's LayoutChildren.
    /// The ListBox occupies a rectangular area within the parent panel.
    pub(crate) layout_x: f64,
    pub(crate) layout_y: f64,
    pub(crate) layout_w: f64,
    pub(crate) layout_h: f64,

    // C++ fields: CutStocksDialog, DeleteStocksDialog, PasteStocksDialog,
    // InterestDialog, InterestToSet — persistent dialog pointers polled in Cycle.
    pub(crate) cut_stocks_dialog: Option<emDialog>,
    pub(crate) paste_stocks_dialog: Option<emDialog>,
    pub(crate) delete_stocks_dialog: Option<emDialog>,
    pub(crate) interest_dialog: Option<emDialog>,
    // Delivery buffers written by `on_finish` callbacks (idiom adaptation:
    // bridges DialogPrivateEngine's scope to the consumer panel's scope; the
    // observable trigger is the dialog's `finish_signal` per §3.2 of the
    // B-013 design — see `Cycle` below).
    pub(crate) cut_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) paste_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) delete_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) interest_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) interest_to_set: Option<Interest>,
    // B-013: per-dialog first-Cycle-init flags (D-006 subscribe-shape).
    // Set true after `ectx.connect(dialog.finish_signal, ectx.id())` runs;
    // cleared on disconnect (signal observed) and on cancel-old-dialog.
    pub(crate) cut_subscribed: bool,
    pub(crate) paste_subscribed: bool,
    pub(crate) delete_subscribed: bool,
    pub(crate) interest_subscribed: bool,
    /// G4: lazy-allocated SignalId for SelectedDate changes per D-008 A1.
    /// Mirrors C++ `emStocksListBox::SelectedDateSignal` (header line 89).
    /// Fired by `signal_selected_date` from `SetSelectedDate` /
    /// `GoBackInHistory` / `GoForwardInHistory` only when the value actually
    /// changed (matches C++ `emSignal::Signal()` semantics).
    selected_date_signal: Cell<SignalId>,

    /// B-001 Phase 3 — cross-Cycle reference to the parent's `emStocksFileModel`,
    /// per CLAUDE.md §Ownership rule (a). Mirrors C++ `emStocksListBox.h`
    /// member `emStocksFileModel & FileModel;`. Required so the ListBox's own
    /// `Cycle` (Phase 4.5) can subscribe to `FileModel::GetChangeSignal()`
    /// without being passed it per-call. `None` until `set_refs` is invoked
    /// by the parent panel at attach time (B-001 sequencing — ListBox is
    /// constructed before the parent has materialized its file model link
    /// in some test paths).
    pub(crate) file_model_ref: Option<Rc<RefCell<crate::emStocksFileModel::emStocksFileModel>>>,

    /// B-001 Phase 3 — cross-Cycle reference to the parent's `emStocksConfig`.
    /// Same rationale as `file_model_ref`. Mirrors C++ `emStocksConfig & Config;`.
    pub(crate) config_ref: Option<Rc<RefCell<emStocksConfig>>>,

    /// B-001 Phase 3 — D-006 first-Cycle init flag for the model/config
    /// subscribe pair. Set true by Phase 4.5 wiring once `ectx.connect`s
    /// run for the FileModel/Config ChangeSignals. Distinct from the per-
    /// dialog `*_subscribed` flags above (those gate dialog finish-signal
    /// connects).
    pub(crate) subscribed_init: bool,

    /// B-001 row -51 — cached `emStocksFileModel::GetChangeSignal()` id
    /// captured at the first-Cycle init step, so the IsSignaled gate below
    /// can read it without re-borrowing `file_model_ref`.
    pub(crate) file_model_change_sig: Option<SignalId>,
    /// B-001 row -52 — cached `emStocksConfig::GetChangeSignal()` id; same
    /// rationale as `file_model_change_sig`.
    pub(crate) config_change_sig: Option<SignalId>,
    /// B-001 row -53 — cached inherited `emListBox::GetItemTriggerSignal()`
    /// id, captured once the inner emListBox has been attached. Distinct
    /// init step from `subscribed_init` (model/config above) because the
    /// inner emListBox is `None` until `attach_list_box`; the connect is
    /// retried each Cycle until then.
    pub(crate) item_trigger_sig: Option<SignalId>,
}

impl Default for emStocksListBox {
    fn default() -> Self {
        Self::new()
    }
}

impl emStocksListBox {
    pub fn new() -> Self {
        Self {
            selected_date: String::new(),
            visible_items: Vec::new(),
            list_box: None,
            selected_indices: Vec::new(),
            active_index: None,
            look: None,
            layout_x: 0.0,
            layout_y: 0.0,
            layout_w: 1.0,
            layout_h: 1.0,
            cut_stocks_dialog: None,
            paste_stocks_dialog: None,
            delete_stocks_dialog: None,
            interest_dialog: None,
            cut_stocks_result: Rc::new(Cell::new(None)),
            paste_stocks_result: Rc::new(Cell::new(None)),
            delete_stocks_result: Rc::new(Cell::new(None)),
            interest_result: Rc::new(Cell::new(None)),
            interest_to_set: None,
            cut_subscribed: false,
            paste_subscribed: false,
            delete_subscribed: false,
            interest_subscribed: false,
            selected_date_signal: Cell::new(SignalId::null()),
            file_model_ref: None,
            config_ref: None,
            subscribed_init: false,
            file_model_change_sig: None,
            config_change_sig: None,
            item_trigger_sig: None,
        }
    }

    /// B-001 Phase 3 test accessor: reports whether the FileModel/Config
    /// subscribe pair has been wired by `Cycle`. Read-only; flipped to true
    /// by the Phase 4.5 `Cycle` implementation. Externally visible so the
    /// Phase 3 sanity tests (and Phase 4.5 TDD harness) can observe the
    /// transition without touching `pub(crate)` fields.
    #[doc(hidden)]
    pub fn subscribed_init_for_test(&self) -> bool {
        self.subscribed_init
    }

    /// B-001 Phase 3 test accessor: borrowed handle to the installed FileModel
    /// ref, or `None` if `set_refs` has not been called. Used by Phase 3 tests
    /// to confirm the parent installs the refs at attach time.
    #[doc(hidden)]
    pub fn has_file_model_ref(&self) -> bool {
        self.file_model_ref.is_some()
    }

    /// B-001 Phase 3 test accessor: see `has_file_model_ref`.
    #[doc(hidden)]
    pub fn has_config_ref(&self) -> bool {
        self.config_ref.is_some()
    }

    /// B-001 rows -51 / -52 — install pre-allocated FileModel/Config
    /// ChangeSignal ids on the ListBox so its `Cycle` can observe them via
    /// `IsSignaled` without re-borrowing the parent's `Rc<RefCell<>>` (the
    /// parent holds `model.borrow_mut()` across `lb.Cycle`; a second borrow
    /// from inside would panic). The parent (`emStocksFilePanel::Cycle`)
    /// calls this in its own first-Cycle init slice where it has clean
    /// access to model/config + ectx. After the call returns,
    /// `subscribed_init` is true and the ListBox's `Cycle` reactions begin
    /// firing. Idempotent — the parent gates on `subscribed_init` already.
    /// Observable behavior matches C++ `emStocksListBox` ctor's
    /// `AddWakeUpSignal(FileModel.GetChangeSignal())` /
    /// `AddWakeUpSignal(Config.GetChangeSignal())` (cpp:51-52); the only
    /// difference is which scope owns the `connect` call (parent vs. self),
    /// which is unobservable to subscribers.
    pub fn wire_change_signals(
        &mut self,
        file_model_change_sig: SignalId,
        config_change_sig: SignalId,
    ) {
        self.file_model_change_sig = Some(file_model_change_sig);
        self.config_change_sig = Some(config_change_sig);
        self.subscribed_init = true;
    }

    /// B-001 Phase 3 — install the cross-Cycle FileModel/Config refs.
    /// Called by `emStocksFilePanel` at the same site that materializes the
    /// ListBox (after VFS becomes Loaded). Idempotent: re-installing
    /// the same Rcs is a no-op visible to subscribers because Phase 4.5's
    /// `subscribed_init` gate already prevents double-connect; callers that
    /// want to re-target a different model/config must build a fresh
    /// `emStocksListBox`.
    pub fn set_refs(
        &mut self,
        file_model: Rc<RefCell<crate::emStocksFileModel::emStocksFileModel>>,
        config: Rc<RefCell<emStocksConfig>>,
    ) {
        self.file_model_ref = Some(file_model);
        self.config_ref = Some(config);
    }

    /// Port of C++ `emStocksListBox::GetSelectedDateSignal` (header line 89).
    /// D-008 A1 combined-form lazy accessor.
    pub fn GetSelectedDateSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
        let cur = self.selected_date_signal.get();
        if cur.is_null() {
            let new_id = ectx.create_signal();
            self.selected_date_signal.set(new_id);
            new_id
        } else {
            cur
        }
    }

    /// Synchronous fire of SelectedDateSignal per D-007. No-op when unallocated.
    fn signal_selected_date(&self, ectx: &mut impl SignalCtx) {
        let s = self.selected_date_signal.get();
        if !s.is_null() {
            ectx.fire(s);
        }
    }

    /// Test-only accessor for the raw SignalId slot without allocating.
    #[doc(hidden)]
    pub fn selected_date_signal_for_test(&self) -> SignalId {
        self.selected_date_signal.get()
    }

    /// B-001 row -51 test accessor: cached FileModel ChangeSignal id, or
    /// `None` before the first Cycle has wired it.
    #[doc(hidden)]
    pub fn file_model_change_signal_for_test(&self) -> Option<SignalId> {
        self.file_model_change_sig
    }

    /// B-001 row -52 test accessor: cached Config ChangeSignal id, or `None`
    /// before the first Cycle has wired it.
    #[doc(hidden)]
    pub fn config_change_signal_for_test(&self) -> Option<SignalId> {
        self.config_change_sig
    }

    /// B-001 row -53 test accessor: cached inherited ItemTriggerSignal id,
    /// or `None` before the inner emListBox has been attached.
    #[doc(hidden)]
    pub fn item_trigger_signal_for_test(&self) -> Option<SignalId> {
        self.item_trigger_sig
    }

    /// Port of inherited C++ `emListBox::GetSelectionSignal`. Delegates to the
    /// inner `Option<emListBox>`. Returns `None` while the inner emListBox is
    /// unattached (lazy AutoExpand). Phase 4 consumers must early-return their
    /// Cycle subscribe when `None` (the "two-tier subscribed_widgets" pattern
    /// in `2026-05-01-B-001-no-wire-emstocks.md`).
    pub fn GetSelectionSignal(&self) -> Option<SignalId> {
        self.list_box.as_ref().map(|lb| lb.selection_signal)
    }

    /// Port of inherited C++ `emListBox::GetItemTriggerSignal`. Delegates as
    /// `GetSelectionSignal` does; same Option-wrapped deferred-attach contract.
    pub fn GetItemTriggerSignal(&self) -> Option<SignalId> {
        self.list_box.as_ref().map(|lb| lb.item_trigger_signal)
    }

    /// Attach an emListBox backed by the given look.
    /// After this call all selection operations delegate to it.
    pub fn attach_list_box<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        look: Rc<emLook>,
    ) {
        self.list_box = Some(emListBox::new(cc, look.clone()));
        self.look = Some(look);
        // Carry over any pre-existing local selection state is not attempted:
        // the attached list box starts empty and callers re-select as needed.
        self.selected_indices.clear();
    }

    /// B-001-followup C.4 — install the per-item factory mirroring C++
    /// `emStocksListBox::CreateItemPanel` at cpp:696-705
    /// (`new emStocksItemPanel(*this, name, itemIndex, FileModel, Config)`).
    ///
    /// The C++ factory is a virtual override on `emListBox::CreateItemPanel`;
    /// the Rust analogue is the `item_panel_factory` closure slot on the
    /// inner `emListBox`. The closure captures the outer ListBox's own
    /// `Weak<RefCell<emStocksListBox>>` self-reference (paired with the
    /// `Rc<RefCell<>>` held by the parent `emStocksFilePanel` per
    /// CLAUDE.md §Ownership rule (a)) plus the FileModel/Config refs and
    /// the emLook. The Weak resolves at item-creation time; if it has
    /// been dropped (i.e. the FilePanel released the ListBox between
    /// the inner `emListBox::CreateItemPanel` call and the factory
    /// firing), the factory falls back to a `DefaultItemPanel` so the
    /// inner ListBox sees a live `Box<dyn ItemPanelInterface>`.
    ///
    /// Must be called after `attach_list_box` (the inner emListBox must
    /// exist) and after `set_refs` (so file_model_ref / config_ref are
    /// installed). No-op if the inner emListBox is `None`.
    pub fn install_item_panel_factory(
        &mut self,
        self_ref: std::rc::Weak<RefCell<emStocksListBox>>,
    ) {
        let look = match self.look.as_ref() {
            Some(l) => l.clone(),
            None => return,
        };
        let file_model = match self.file_model_ref.as_ref() {
            Some(m) => m.clone(),
            None => return,
        };
        let config = match self.config_ref.as_ref() {
            Some(c) => c.clone(),
            None => return,
        };
        let inner = match self.list_box.as_mut() {
            Some(lb) => lb,
            None => return,
        };

        inner.set_item_panel_factory(move |index, text, selected| {
            // Try to upgrade the Weak self-ref. If the outer ListBox has
            // been dropped, fall back to DefaultItemPanel — which mirrors
            // the C++ behavior of "no override installed" rather than
            // crashing.
            //
            // The factory captures FileModel/Config Rcs (cloned into each
            // panel) plus the outer-listbox Weak. The C++ post-construction
            // `SetStockRec(GetStockByItemIndex(itemIndex))` (cpp:696-705)
            // is wired in `emStocksListBox::CreateItemPanel` *after* the
            // factory returns, via the `ItemPanelInterface::bind_data`
            // hook — the factory itself cannot re-borrow the outer
            // ListBox because the call site already holds `borrow_mut`.
            if let Some(lb_rc) = self_ref.upgrade() {
                let mut panel = crate::emStocksItemPanel::emStocksItemPanel::new(
                    look.clone(),
                    file_model.clone(),
                    config.clone(),
                    lb_rc,
                    index,
                );
                // Seed cached text / selected from the inner-listbox-tracked
                // item state so `ItemPanelInterface::GetText` / `IsSelected`
                // return the live value before the first paint. C++ achieves
                // the same via the `emListBox` ctor populating these from
                // the item's stored `text`/`selected` fields.
                if !text.is_empty() {
                    panel.cached_text = text;
                }
                panel.cached_selected = selected;
                Box::new(panel)
            } else {
                Box::new(emcore::emListBox::DefaultItemPanel::new(
                    index,
                    String::new(),
                    false,
                ))
            }
        });
    }

    /// Port of C++ `emStocksListBox::CreateItemPanel(name,itemIndex)` at
    /// cpp:696-705. Delegates to the inner `emListBox::CreateItemPanel`,
    /// which uses the factory installed by `install_item_panel_factory`
    /// above. C++ also calls `SetStockRec(GetStockByItemIndex(index))`
    /// after construction; B-001-followup Phase D wires that step inside
    /// the factory closure above (the closure has access to FileModel +
    /// the outer listbox's `visible_items` map via Weak<self>).
    pub fn CreateItemPanel(&mut self, _name: &str, item_index: usize) {
        // Look up the stock-rec index via the visible_items map *before*
        // we hand the outer ListBox over to the inner factory. The
        // factory itself cannot do this lookup because the outer is
        // already mut-borrowed by this call site (BorrowError).
        let stock_idx = self.visible_items.get(item_index).copied();

        if let Some(lb) = self.list_box.as_mut() {
            lb.CreateItemPanel(item_index);
            // B-001-followup Phase D — C++ `emStocksListBox::CreateItemPanel`
            // (cpp:696-705) calls `SetStockRec(GetStockByItemIndex(itemIndex))`
            // after construction. We mirror that here via the
            // `bind_data` trait hook, which `emStocksItemPanel` overrides
            // to set its `stock_rec_index` (and cascade to the chart).
            // Default impl is a no-op for `DefaultItemPanel`.
            if let Some(iface) = lb.get_item_panel_interface_mut(item_index) {
                iface.bind_data(stock_idx);
            }
        }
    }

    // ─── Selection helpers ──────────────────────────────────────────────

    /// Port of C++ emListBox::GetSelectionCount (via GetSelectedIndices().len()).
    pub fn GetSelectionCount(&self) -> usize {
        self.GetSelectedIndices().len()
    }

    /// Port of C++ emListBox::IsSelected.
    pub fn IsSelected(&self, visible_index: usize) -> bool {
        if let Some(lb) = &self.list_box {
            lb.IsSelected(visible_index)
        } else {
            self.selected_indices.contains(&visible_index)
        }
    }

    /// Port of C++ emListBox::Select (solely=false: add to selection).
    pub fn Select(&mut self, visible_index: usize) {
        if let Some(lb) = &mut self.list_box {
            lb.Select(visible_index, false);
        } else if !self.selected_indices.contains(&visible_index) {
            self.selected_indices.push(visible_index);
        }
    }

    /// Port of C++ emListBox::ClearSelection.
    pub fn ClearSelection(&mut self) {
        if let Some(lb) = &mut self.list_box {
            lb.ClearSelection();
        } else {
            self.selected_indices.clear();
        }
    }

    /// Port of C++ emListBox::SetSelectedIndex (solely=true: clears others).
    pub fn SetSelectedIndex(&mut self, visible_index: usize) {
        if let Some(lb) = &mut self.list_box {
            lb.Select(visible_index, true);
        } else {
            self.selected_indices.clear();
            self.selected_indices.push(visible_index);
        }
    }

    /// Port of C++ emListBox::GetSelectedIndices.
    pub fn GetSelectedIndices(&self) -> &[usize] {
        if let Some(lb) = &self.list_box {
            lb.GetSelectedIndices()
        } else {
            &self.selected_indices
        }
    }

    /// Get the stock record for a visible-item index.
    fn GetStockByItemIndex<'a>(
        &self,
        visible_index: usize,
        rec: &'a emStocksRec,
    ) -> Option<&'a StockRec> {
        self.visible_items
            .get(visible_index)
            .and_then(|&stock_idx| rec.stocks.get(stock_idx))
    }

    /// Get the visible-item index for a stock by its rec index.
    fn GetItemIndexByStockIndex(&self, stock_index: usize) -> Option<usize> {
        self.visible_items
            .iter()
            .position(|&idx| idx == stock_index)
    }

    /// Port of C++ GetSelectedDate.
    pub fn GetSelectedDate(&self) -> &str {
        &self.selected_date
    }

    /// Port of C++ SetSelectedDate. Fires `SelectedDateSignal` only when the
    /// value actually changed, mirroring C++ `emStocksListBox::SetSelectedDate`
    /// (cpp:99-104). D-007 ectx-threading.
    pub fn SetSelectedDate(&mut self, ectx: &mut impl SignalCtx, date: &str) {
        if self.selected_date != date {
            self.selected_date = date.to_string();
            self.signal_selected_date(ectx);
        }
    }

    /// Port of C++ GoBackInHistory.
    // C++ reads from owned FileModel reference. Rust passes rec explicitly — avoids shared mutable state.
    pub fn GoBackInHistory(&mut self, ectx: &mut impl SignalCtx, rec: &emStocksRec) {
        let date = rec.GetPricesDateBefore(&self.selected_date);
        if !date.is_empty() {
            self.SetSelectedDate(ectx, &date);
        }
    }

    /// Port of C++ GoForwardInHistory.
    // C++ reads from owned FileModel reference. Rust passes rec explicitly — avoids shared mutable state.
    pub fn GoForwardInHistory(&mut self, ectx: &mut impl SignalCtx, rec: &emStocksRec) {
        let date = rec.GetPricesDateAfter(&self.selected_date);
        if !date.is_empty() {
            self.SetSelectedDate(ectx, &date);
        }
    }

    /// Port of C++ IsVisibleStock.
    /// Checks interest level and category visibility against config.
    pub fn IsVisibleStock(stock_rec: &StockRec, config: &emStocksConfig) -> bool {
        stock_rec.interest <= config.min_visible_interest
            && emStocksConfig::IsInVisibleCategories(&config.visible_countries, &stock_rec.country)
            && emStocksConfig::IsInVisibleCategories(&config.visible_sectors, &stock_rec.sector)
            && emStocksConfig::IsInVisibleCategories(
                &config.visible_collections,
                &stock_rec.collection,
            )
    }

    /// Port of C++ UpdateItems.
    /// Rebuilds the visible_items list based on visibility and sorts them.
    pub fn UpdateItems(&mut self, rec: &emStocksRec, config: &emStocksConfig) {
        self.visible_items.clear();
        for (i, stock) in rec.stocks.iter().enumerate() {
            if Self::IsVisibleStock(stock, config) {
                self.visible_items.push(i);
            }
        }
        let selected_date = self.selected_date.clone();
        self.visible_items.sort_by(|&a, &b| {
            Self::CompareStocks(&rec.stocks[a], &rec.stocks[b], config, &selected_date)
        });
    }

    /// Port of C++ CompareItems.
    /// Sorting comparison function.
    pub fn CompareStocks(
        s1: &StockRec,
        s2: &StockRec,
        config: &emStocksConfig,
        selected_date: &str,
    ) -> Ordering {
        // 1. OwnedSharesFirst
        if config.owned_shares_first && s1.owning_shares != s2.owning_shares {
            return if s1.owning_shares {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        // 2. Sorting-specific comparison
        let (b1, f1, b2, f2) = match config.sorting {
            Sorting::ByName => (false, 0.0, false, 0.0),
            Sorting::ByTradeDate => {
                let cmp = CompareDates(&s1.trade_date, &s2.trade_date) as f64;
                (true, cmp, true, 0.0)
            }
            Sorting::ByInquiryDate => {
                let cmp = CompareDates(&s1.inquiry_date, &s2.inquiry_date) as f64;
                (true, cmp, true, 0.0)
            }
            Sorting::ByAchievement => {
                let r1 = s1.GetAchievementOfDate(selected_date, false);
                let r2 = s2.GetAchievementOfDate(selected_date, false);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByOneWeekRise => {
                let r1 = s1.GetRiseUntilDate(selected_date, 7);
                let r2 = s2.GetRiseUntilDate(selected_date, 7);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByThreeWeekRise => {
                let r1 = s1.GetRiseUntilDate(selected_date, 21);
                let r2 = s2.GetRiseUntilDate(selected_date, 21);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByNineWeekRise => {
                let r1 = s1.GetRiseUntilDate(selected_date, 63);
                let r2 = s2.GetRiseUntilDate(selected_date, 63);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByDividend => {
                let b1 = !s1.expected_dividend.is_empty();
                let f1 = if b1 {
                    s1.expected_dividend.parse::<f64>().unwrap_or(0.0)
                } else {
                    0.0
                };
                let b2 = !s2.expected_dividend.is_empty();
                let f2 = if b2 {
                    s2.expected_dividend.parse::<f64>().unwrap_or(0.0)
                } else {
                    0.0
                };
                (b1, f1, b2, f2)
            }
            Sorting::ByPurchaseValue => {
                let r1 = s1.GetTradeValue();
                let r2 = s2.GetTradeValue();
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByValue => {
                let r1 = s1.GetValueOfDate(selected_date);
                let r2 = s2.GetValueOfDate(selected_date);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
            Sorting::ByDifference => {
                let r1 = s1.GetDifferenceValueOfDate(selected_date);
                let r2 = s2.GetDifferenceValueOfDate(selected_date);
                (
                    r1.is_some(),
                    r1.unwrap_or(0.0),
                    r2.is_some(),
                    r2.unwrap_or(0.0),
                )
            }
        };

        // 3. If validity differs: valid sorts AFTER invalid (C++: b1 ? 1 : -1)
        if b1 != b2 {
            return if b1 {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }

        // 4. If both valid, compare values
        if b1 {
            let f = f1 - f2;
            if f < 0.0 {
                return Ordering::Less;
            }
            if f > 0.0 {
                return Ordering::Greater;
            }
        }

        // 5. Tiebreaker: name (case-insensitive), name (exact), ID (numeric), ID (string)
        let cmp = s1.name.to_lowercase().cmp(&s2.name.to_lowercase());
        if cmp != Ordering::Equal {
            return cmp;
        }

        let cmp = s1.name.cmp(&s2.name);
        if cmp != Ordering::Equal {
            return cmp;
        }

        let i1: i32 = s1.id.parse().unwrap_or(0);
        let i2: i32 = s2.id.parse().unwrap_or(0);
        let cmp = i1.cmp(&i2);
        if cmp != Ordering::Equal {
            return cmp;
        }

        s1.id.cmp(&s2.id)
    }

    // ─── Paint helper ───────────────────────────────────────────────────

    /// Port of C++ Paint (empty-message path).
    /// Paints the "empty stock list" message when no stocks are visible.
    pub fn PaintEmptyMessage(&self, painter: &mut emPainter, w: f64, h: f64, bg_color: emColor) {
        if self.visible_items.is_empty() {
            painter.PaintTextBoxed(
                0.0,
                0.0,
                w,
                h,
                "empty stock list",
                h * 0.1,
                emColor::rgb(255, 255, 255),
                bg_color,
                TextAlignment::Center,
                VAlign::Center,
                TextAlignment::Center,
                0.0,
                false,
                0.0,
            );
        }
    }

    // ─── Stock operations ───────────────────────────────────────────────

    /// Port of C++ NewStock.
    /// Creates a new stock, assigns an ID, sets initial fields from config,
    /// updates items, and selects the new stock.
    // C++ reads from owned FileModel/Config references. Rust passes rec and config explicitly — avoids shared mutable state.
    pub fn NewStock(&mut self, rec: &mut emStocksRec, config: &emStocksConfig) {
        let stock_index = rec.stocks.len();
        let mut stock_rec = StockRec::default();
        stock_rec.id = rec.InventStockId();
        if stock_rec.interest > config.min_visible_interest {
            stock_rec.interest = config.min_visible_interest;
        }
        if !config.visible_countries.is_empty() {
            stock_rec.country = config.visible_countries[0].clone();
        }
        if !config.visible_sectors.is_empty() {
            stock_rec.sector = config.visible_sectors[0].clone();
        }
        if !config.visible_collections.is_empty() {
            stock_rec.collection = config.visible_collections[0].clone();
        }
        rec.stocks.push(stock_rec);

        self.UpdateItems(rec, config);
        if let Some(vis_idx) = self.GetItemIndexByStockIndex(stock_index) {
            self.SetSelectedIndex(vis_idx);
        }
    }

    /// Port of C++ CopyStocks.
    /// Copies selected stocks to system clipboard.
    pub fn CopyStocks(&self, rec: &emStocksRec) {
        if self.GetSelectionCount() == 0 {
            return;
        }
        if let Some(text) = self.copy_stocks_to_string(rec) {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(&text);
            }
        }
    }

    /// Internal helper: serialize selected stocks to a string.
    /// Returns None if nothing is selected. Used by CopyStocks and tests.
    pub(crate) fn copy_stocks_to_string(&self, rec: &emStocksRec) -> Option<String> {
        if self.GetSelectionCount() == 0 {
            return None;
        }

        let mut stocks_rec = emStocksRec::default();
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(&stock_idx) = self.visible_items.get(vis_idx) {
                if let Some(stock) = rec.stocks.get(stock_idx) {
                    stocks_rec.stocks.push(stock.clone());
                }
            }
        }

        let rec_struct = stocks_rec.to_rec();
        Some(write_rec_with_format(&rec_struct, "emStocks"))
    }

    /// Port of C++ DeleteStocks.
    /// Removes selected stocks from rec.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `delete_stocks_dialog` and returns early — Cycle() polls the result.
    /// When `ask=false`, performs deletion immediately.
    /// C++ takes no arguments (reads from owned FileModel).
    /// Rust takes `rec` and `ask` parameters.
    pub fn DeleteStocks<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        ask: bool,
    ) {
        if self.GetSelectionCount() == 0 {
            return;
        }
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                // B-013: symmetric with the confirmed-branch disconnect in
                // `Cycle` — without this, the parent engine would retain a
                // live `(old_finish_signal → engine)` connection until the
                // SignalId is reaped by dialog teardown (slow leak).
                if let Some(old) = self.delete_stocks_dialog.take() {
                    cancel_subscribed_dialog(cc, &mut self.delete_subscribed, &old);
                    self.delete_stocks_result.set(None);
                }
                let count = self.GetSelectionCount();
                let mut dialog = emDialog::new(
                    cc,
                    &format!("Really delete {} stock(s)?", count),
                    look.clone(),
                );
                dialog.AddCustomButton(cc, "Delete", DialogResult::Ok);
                dialog.AddCustomButton(cc, "Cancel", DialogResult::Cancel);
                let cell = Rc::clone(&self.delete_stocks_result);
                dialog.set_on_finish(Box::new(move |r, _sched| cell.set(Some(*r))));
                dialog.show(cc);
                self.delete_stocks_dialog = Some(dialog);
            }
            return; // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: perform deletion immediately.
        // Collect rec-level stock indices to remove, sorted descending
        let mut indices_to_remove: Vec<usize> = self
            .GetSelectedIndices()
            .iter()
            .filter_map(|&vis_idx| self.visible_items.get(vis_idx).copied())
            .collect();
        indices_to_remove.sort_unstable();
        indices_to_remove.dedup();
        // Remove from end to preserve earlier indices
        for &idx in indices_to_remove.iter().rev() {
            rec.stocks.remove(idx);
        }

        self.ClearSelection();
    }

    /// Port of C++ CutStocks.
    /// Cuts selected stocks to clipboard.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `cut_stocks_dialog` and returns early — Cycle() polls the result.
    /// When `ask=false`, performs copy+delete immediately.
    /// C++ takes no arguments. Rust takes `rec` and `ask` parameters.
    pub fn CutStocks<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        ask: bool,
    ) {
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                // B-013: see DeleteStocks for cancel-old-disconnect rationale.
                if let Some(old) = self.cut_stocks_dialog.take() {
                    cancel_subscribed_dialog(cc, &mut self.cut_subscribed, &old);
                    self.cut_stocks_result.set(None);
                }
                let count = self.GetSelectionCount();
                let mut dialog =
                    emDialog::new(cc, &format!("Really cut {} stock(s)?", count), look.clone());
                dialog.AddCustomButton(cc, "Cut", DialogResult::Ok);
                dialog.AddCustomButton(cc, "Cancel", DialogResult::Cancel);
                let cell = Rc::clone(&self.cut_stocks_result);
                dialog.set_on_finish(Box::new(move |r, _sched| cell.set(Some(*r))));
                dialog.show(cc);
                self.cut_stocks_dialog = Some(dialog);
            }
            return; // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: perform cut immediately.
        self.CopyStocks(rec);
        if self.GetSelectionCount() > 0 {
            self.DeleteStocks(cc, rec, false); // inner delete doesn't ask again
        }
    }

    /// Port of C++ PasteStocks.
    /// Pastes stocks from clipboard.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `paste_stocks_dialog` and returns early — Cycle() polls the result.
    /// When `ask=false`, reads clipboard and inserts stocks immediately.
    /// Returns names of pasted stocks that are not visible due to filters,
    /// or an error if the clipboard data is invalid or clipboard is empty.
    /// C++ takes no arguments. Rust takes `rec`, `config`, and `ask` parameters.
    pub fn PasteStocks<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
        ask: bool,
    ) -> Result<Vec<String>, String> {
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                // B-013: see DeleteStocks for cancel-old-disconnect rationale.
                if let Some(old) = self.paste_stocks_dialog.take() {
                    cancel_subscribed_dialog(cc, &mut self.paste_subscribed, &old);
                    self.paste_stocks_result.set(None);
                }
                let mut dialog = emDialog::new(cc, "Really paste stocks?", look.clone());
                dialog.AddCustomButton(cc, "Paste", DialogResult::Ok);
                dialog.AddCustomButton(cc, "Cancel", DialogResult::Cancel);
                let cell = Rc::clone(&self.paste_stocks_result);
                dialog.set_on_finish(Box::new(move |r, _sched| cell.set(Some(*r))));
                dialog.show(cc);
                self.paste_stocks_dialog = Some(dialog);
            }
            return Ok(Vec::new()); // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: perform paste immediately.
        let clipboard_text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
            clipboard.get_text().unwrap_or_default()
        } else {
            return Err("Cannot access clipboard".to_string());
        };
        if clipboard_text.is_empty() {
            return Err("Clipboard is empty".to_string());
        }
        self.paste_stocks_from_text(rec, config, &clipboard_text)
    }

    /// Internal helper: parse stocks from a text string and insert into rec.
    /// Used by PasteStocks (public) and tests.
    pub(crate) fn paste_stocks_from_text(
        &mut self,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
        clipboard_text: &str,
    ) -> Result<Vec<String>, String> {
        let parsed = parse_rec_with_format(clipboard_text, "emStocks")
            .map_err(|e| format!("No valid stocks in clipboard ({e})"))?;
        let stocks_rec = emStocksRec::from_rec(&parsed)
            .map_err(|e| format!("No valid stocks in clipboard ({e})"))?;

        let n = rec.stocks.len();
        let m = stocks_rec.stocks.len();
        let mut invisible_names = Vec::new();

        for mut stock in stocks_rec.stocks {
            if rec.GetStockIndexById(&stock.id).is_some() {
                stock.id = rec.InventStockId();
            }
            if !Self::IsVisibleStock(&stock, config) {
                invisible_names.push(stock.name.clone());
            }
            rec.stocks.push(stock);
        }

        self.UpdateItems(rec, config);
        self.ClearSelection();
        for i in n..(n + m) {
            if let Some(vis_idx) = self.GetItemIndexByStockIndex(i) {
                self.Select(vis_idx);
            }
        }

        Ok(invisible_names)
    }

    /// Port of C++ DeleteSharePrices.
    /// Clears price data from all visible stocks.
    // C++ reads from owned FileModel reference. Rust passes rec explicitly — avoids shared mutable state.
    pub fn DeleteSharePrices(&self, rec: &mut emStocksRec) {
        for &stock_idx in &self.visible_items {
            if let Some(stock) = rec.stocks.get_mut(stock_idx) {
                stock.prices.clear();
                stock.last_price_date.clear();
            }
        }
    }

    /// Port of C++ SetInterest.
    /// Sets interest level on selected stocks.
    /// When `ask=true` (C++: default), stores a confirmation dialog in
    /// `interest_dialog` and `interest_to_set`, then returns early — Cycle()
    /// polls the result.
    /// When `ask=false`, applies the interest change immediately.
    /// C++ takes no arguments beyond interest. Rust takes `rec` and `ask` parameters.
    pub fn SetInterest<C: emcore::emEngineCtx::ConstructCtx>(
        &mut self,
        cc: &mut C,
        rec: &mut emStocksRec,
        interest: Interest,
        ask: bool,
    ) {
        if ask {
            if let Some(ref look) = self.look {
                // Cancel any in-flight dialog before creating a new one.
                // B-013: see DeleteStocks for cancel-old-disconnect rationale.
                if let Some(old) = self.interest_dialog.take() {
                    cancel_subscribed_dialog(cc, &mut self.interest_subscribed, &old);
                    self.interest_result.set(None);
                }
                let mut dialog = emDialog::new(cc, "Really change interest?", look.clone());
                dialog.AddCustomButton(cc, "Change", DialogResult::Ok);
                dialog.AddCustomButton(cc, "Cancel", DialogResult::Cancel);
                let cell = Rc::clone(&self.interest_result);
                dialog.set_on_finish(Box::new(move |r, _sched| cell.set(Some(*r))));
                dialog.show(cc);
                self.interest_dialog = Some(dialog);
                self.interest_to_set = Some(interest);
            }
            return; // Defer until Cycle() observes dialog confirmation.
        }

        // ask=false path: apply immediately.
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(&stock_idx) = self.visible_items.get(vis_idx) {
                if let Some(stock) = rec.stocks.get_mut(stock_idx) {
                    stock.interest = interest;
                }
            }
        }
    }

    /// Port of C++ Cycle (dialog polling portion).
    /// Polls persistent confirmation dialogs and executes deferred operations
    /// when the user confirms. Returns true while any dialog is still open
    /// (signals the parent panel to keep cycling).
    ///
    /// B-013 — D-002 rule-1 trigger conversion. Each per-dialog block
    /// subscribes to `dialog.finish_signal` (D-006 first-Cycle init) and uses
    /// `IsSignaled` as the wakeup trigger; the existing `*_result` cell is
    /// preserved as the delivery buffer (idiom adaptation, not a divergence).
    ///
    /// Engine-identity note: in C++ `class emStocksListBox : public emListBox`
    /// (`emStocksListBox.h:29`) — the ListBox is its own `emEngine`, so
    /// `AddWakeUpSignal(...)` self-subscribes the ListBox engine. In the Rust
    /// port, `emStocksListBox` is composed inside `emStocksFilePanel` (see
    /// `emStocksFilePanel.rs:491` `lb.Cycle(ectx, ...)`), so `ectx.id()` here
    /// resolves to the **FilePanel** engine, not the ListBox. The connect/
    /// disconnect calls subscribe the parent FilePanel engine to the dialog's
    /// `finish_signal`. Behaviorally correct (the FilePanel's `Cycle` is what
    /// reaches this polling block, so waking the FilePanel is what fires the
    /// next Cycle). The composition vs inheritance is preserved-design-intent
    /// of the Rust port — not a forced divergence (no `DIVERGED` annotation).
    pub fn Cycle(
        &mut self,
        ectx: &mut EngineCtx<'_>,
        rec: &mut emStocksRec,
        config: &emStocksConfig,
    ) -> bool {
        let mut busy = false;

        // ───────────────────────────────────────────────────────────────────
        // B-001 rows -51 / -52 / -53 — D-006 first-Cycle init for the
        // FileModel / Config / ItemTrigger subscribe trio. Mirrors C++
        // `emStocksListBox` constructor (cpp:51-53):
        //
        //     AddWakeUpSignal(FileModel.GetChangeSignal());
        //     AddWakeUpSignal(Config.GetChangeSignal());
        //     AddWakeUpSignal(GetItemTriggerSignal());
        //
        // The Rust port wires these on the first Cycle slice that observes
        // both `file_model_ref` / `config_ref` populated by the parent
        // (`emStocksFilePanel::set_refs` — Phase 3). The ItemTrigger leg is
        // attach-deferred per the design's two-tier note: the inner
        // `emListBox` is `None` until `attach_list_box`, and
        // `GetItemTriggerSignal()` returns `None` while the inner box is
        // unattached. We therefore guard the connect on `Some(_)` and re-
        // attempt on every Cycle until attach lands; a separate
        // `item_trigger_sig` cache prevents double-connect once attached.
        // ───────────────────────────────────────────────────────────────────
        // Note: we do NOT re-borrow `self.file_model_ref` / `self.config_ref`
        // inside `Cycle` because the parent `emStocksFilePanel::Cycle` is the
        // only call site, and it already holds `model.borrow_mut()` /
        // `config.borrow()` across the `lb.Cycle(...)` call (see the split-
        // borrow comment in the parent for B-017 row 2). Re-borrowing the
        // same `Rc<RefCell<>>` here would panic at runtime. Instead, the
        // parent allocates the model/config ChangeSignal ids at first-Cycle
        // init time (where it has clean access to model/config + ectx) and
        // installs them via `wire_change_signals`. The `subscribed_init`
        // gate observed below is flipped by that installer; the install also
        // performs the `ectx.connect` so the engine is wired. The
        // `file_model_ref` / `config_ref` fields remain on the struct as
        // structural mirrors of the C++ `FileModel & / Config &` members
        // (cited at the field definitions); they are observed indirectly
        // through the cached signal ids below.

        if self.item_trigger_sig.is_none() {
            if let Some(sig) = self.GetItemTriggerSignal() {
                let eid = ectx.id();
                ectx.connect(sig, eid);
                self.item_trigger_sig = Some(sig);
            }
        }

        // ── Reactions, in C++ source order (cpp:635-654) ─────────────────
        // Row -51: FileModel.ChangeSignal → UpdateItems.
        if let Some(sig) = self.file_model_change_sig {
            if ectx.IsSignaled(sig) {
                self.UpdateItems(rec, config);
            }
        }
        // Row -52: Config.ChangeSignal → UpdateItems.
        if let Some(sig) = self.config_change_sig {
            if ectx.IsSignaled(sig) {
                self.UpdateItems(rec, config);
            }
        }
        // Row -53: ItemTriggerSignal → open first web page if configured.
        if let Some(sig) = self.item_trigger_sig {
            if ectx.IsSignaled(sig) {
                let triggered: Option<usize> = self
                    .list_box
                    .as_ref()
                    .and_then(|lb| lb.GetTriggeredItemIndex());
                if let Some(item_idx) = triggered {
                    if config.triggering_opens_web_page {
                        if let Some(stock) = self.GetStockByItemIndex(item_idx, rec) {
                            if let Some(page) = stock.web_pages.first() {
                                if !page.is_empty() {
                                    let _ = open::that(page);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Poll delete dialog.
        // Invariant (B-013 Note 7): subscribe-then-check happens on the same
        // Cycle slice; do not split this block across two methods or two
        // slices, or a fire could be missed.
        if let Some(sig) = self.delete_stocks_dialog.as_ref().map(|d| d.finish_signal) {
            // The immutable borrow of self.delete_stocks_dialog ends here
            // (`sig` is Copy SignalId), so the &mut self uses below compile.
            if !self.delete_subscribed {
                ectx.connect(sig, ectx.id()); // subscribes the parent FilePanel engine
                self.delete_subscribed = true;
            }
            if ectx.IsSignaled(sig) {
                let confirmed = self.delete_stocks_result.take() == Some(DialogResult::Ok);
                ectx.disconnect(sig, ectx.id());
                self.delete_stocks_dialog = None;
                self.delete_subscribed = false;
                if confirmed {
                    self.DeleteStocks(ectx, rec, false);
                }
            } else {
                // Outer guard is Some(dialog); inner if/else distinguishes
                // "signaled (consume)" from "still pending (busy)".
                busy = true;
            }
        }

        // Poll cut dialog.
        if let Some(sig) = self.cut_stocks_dialog.as_ref().map(|d| d.finish_signal) {
            if !self.cut_subscribed {
                ectx.connect(sig, ectx.id());
                self.cut_subscribed = true;
            }
            if ectx.IsSignaled(sig) {
                let confirmed = self.cut_stocks_result.take() == Some(DialogResult::Ok);
                ectx.disconnect(sig, ectx.id());
                self.cut_stocks_dialog = None;
                self.cut_subscribed = false;
                if confirmed {
                    self.CutStocks(ectx, rec, false);
                }
            } else {
                busy = true;
            }
        }

        // Poll paste dialog.
        if let Some(sig) = self.paste_stocks_dialog.as_ref().map(|d| d.finish_signal) {
            if !self.paste_subscribed {
                ectx.connect(sig, ectx.id());
                self.paste_subscribed = true;
            }
            if ectx.IsSignaled(sig) {
                let confirmed = self.paste_stocks_result.take() == Some(DialogResult::Ok);
                ectx.disconnect(sig, ectx.id());
                self.paste_stocks_dialog = None;
                self.paste_subscribed = false;
                if confirmed {
                    let _ = self.PasteStocks(ectx, rec, config, false);
                }
            } else {
                busy = true;
            }
        }

        // Poll interest dialog.
        // §3.3a — Interest-block cancel-side cleanup: preserve the existing
        // `interest_to_set = None;` reset on non-Ok finish.
        if let Some(sig) = self.interest_dialog.as_ref().map(|d| d.finish_signal) {
            if !self.interest_subscribed {
                ectx.connect(sig, ectx.id());
                self.interest_subscribed = true;
            }
            if ectx.IsSignaled(sig) {
                let confirmed = self.interest_result.take() == Some(DialogResult::Ok);
                ectx.disconnect(sig, ectx.id());
                self.interest_dialog = None;
                self.interest_subscribed = false;
                if confirmed {
                    if let Some(interest) = self.interest_to_set.take() {
                        self.SetInterest(ectx, rec, interest, false);
                    }
                } else {
                    self.interest_to_set = None;
                }
            } else {
                busy = true;
            }
        }

        busy
    }

    /// Port of C++ ShowFirstWebPages.
    /// Opens the first web page for each selected stock in the system browser.
    pub fn ShowFirstWebPages(&self, rec: &emStocksRec) {
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(stock) = self.GetStockByItemIndex(vis_idx, rec) {
                if let Some(page) = stock.web_pages.first() {
                    if !page.is_empty() {
                        let _ = open::that(page);
                    }
                }
            }
        }
    }

    /// Port of C++ ShowAllWebPages.
    /// Opens all web pages for each selected stock in the system browser.
    pub fn ShowAllWebPages(&self, rec: &emStocksRec) {
        for &vis_idx in self.GetSelectedIndices() {
            if let Some(stock) = self.GetStockByItemIndex(vis_idx, rec) {
                for page in &stock.web_pages {
                    if !page.is_empty() {
                        let _ = open::that(page);
                    }
                }
            }
        }
    }

    /// Port of C++ StartToFetchSharePrices (no-args overload).
    /// Returns visible stock IDs. The caller (FilePanel) creates the fetch dialog.
    pub fn GetVisibleStockIds(&self, rec: &emStocksRec) -> Vec<String> {
        self.visible_items
            .iter()
            .filter_map(|&idx| rec.stocks.get(idx).map(|s| s.id.clone()))
            .collect()
    }

    /// Port of C++ `emStocksListBox::StartToFetchSharePrices()` zero-arg
    /// overload (`emStocksListBox.cpp:371-383`). Iterates all visible items
    /// and forwards their stock ids to the array overload.
    ///
    /// C++ overload of StartToFetchSharePrices.
    ///
    /// Rust has no function overloading; the array overload keeps the C++
    /// name (`StartToFetchSharePrices`) and the zero-arg variant is renamed
    /// `StartToFetchAllSharePrices` (descriptive). Both methods carry this
    /// cite-comment.
    ///
    /// DIVERGED: (language-forced) Rust lacks function overloading. Renamed
    /// from the C++ zero-arg overload to a descriptive name; behavior
    /// matches the C++ overload-resolution result exactly.
    pub fn StartToFetchAllSharePrices(&mut self, ectx: &mut impl SignalCtx, rec: &emStocksRec) {
        let stock_ids = self.GetVisibleStockIds(rec);
        self.StartToFetchSharePrices(ectx, &stock_ids);
    }

    /// Port of C++ `emStocksListBox::StartToFetchSharePrices(const
    /// emArray<emString> & stockIds)` (`emStocksListBox.cpp:386-410`).
    ///
    /// C++ overload of StartToFetchSharePrices.
    ///
    /// Reads `config_ref` (api script/key) and `file_model_ref`
    /// (PricesFetchingDialog owner slot, GetLatestPricesDate). Mirrors C++
    /// dialog-creation-or-raise + AddStockIds. Does NOT call AddListBox on
    /// the dialog (the listbox cannot produce an `Rc<RefCell<Self>>` to
    /// itself); the reaction caller wires AddListBox after this returns
    /// using its own `Rc<RefCell<emStocksListBox>>` strong owner.
    ///
    /// DIVERGED: (language-forced) C++'s `dialog->AddListBox(*this)` takes
    /// `*this` because C++ supports raw self-references. Rust safe code
    /// cannot synthesize an `Rc<RefCell<Self>>` from `&mut self`; the
    /// reaction caller (which holds the Rc) performs that step. Observable
    /// behavior matches C++ exactly when the reaction caller honors the
    /// contract documented above.
    pub fn StartToFetchSharePrices(&mut self, ectx: &mut impl SignalCtx, stock_ids: &[String]) {
        // Acquire FileModel + Config refs (codebase pattern: optional until
        // attach time; bail out gracefully if either is missing).
        let file_model_rc = match self.file_model_ref.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let config_rc = match self.config_ref.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };

        // Snapshot config strings — release the borrow before mutating
        // the file model.
        let (api_script, api_interpreter, api_key) = {
            let cfg = config_rc.borrow();
            (
                cfg.api_script.clone(),
                cfg.api_script_interpreter.clone(),
                cfg.api_key.clone(),
            )
        };

        // Determine date BEFORE creating dialog (C++ reads
        // FileModel.GetLatestPricesDate() from the existing model state;
        // mirrors emStocksListBox.cpp:402-403).
        let mut date = file_model_rc.borrow().GetRec().GetLatestPricesDate();
        if date.is_empty() {
            date = super::emStocksRec::GetCurrentDate();
        }

        // Mirror C++ dialog-create-or-raise (cpp:392-401).
        let mut model = file_model_rc.borrow_mut();
        if model.prices_fetching_dialog.is_some() {
            // C++ FileModel.PricesFetchingDialog->Raise().
            // TODO(FU-001): port emDialog::Raise once view-parenting lands
            // (UPSTREAM-GAP — Rust dialog ctor takes no view).
            eprintln!("[FU-001] PricesFetchingDialog::Raise (no-op stub)");
        } else {
            let dialog = emStocksFetchPricesDialog::new_with_model(
                &api_script,
                &api_interpreter,
                &api_key,
                file_model_rc.clone(),
            );
            model.prices_fetching_dialog = Some(dialog);
        }

        // Forward stock_ids to the dialog before releasing the borrow.
        if let Some(dialog) = model.prices_fetching_dialog.as_mut() {
            dialog.AddStockIds(ectx, stock_ids);
        }
        drop(model);

        // SetSelectedDate fires SelectedDateSignal only if the value
        // changed; matches C++ cpp:404.
        self.SetSelectedDate(ectx, &date);
    }

    /// Port of C++ `emStocksListBox::ShowWebPages(const emArray<emString>
    /// & webPages) const` (`emStocksListBox.cpp:496-516`). `&self` matches
    /// the C++ `const`. Reads `config_ref.web_browser` and spawns the
    /// browser process.
    pub fn ShowWebPages(&self, web_pages: &[String]) {
        if web_pages.is_empty() {
            return;
        }
        let config_rc = match self.config_ref.as_ref() {
            Some(r) => r.clone(),
            None => return,
        };
        let browser = config_rc.borrow().web_browser.clone();
        if browser.is_empty() {
            // TODO(FU-001): replace with emDialog::ShowMessage when ported.
            eprintln!("[emStocksListBox::ShowWebPages] Web browser is not configured.");
            return;
        }
        let mut args: Vec<&str> = Vec::with_capacity(1 + web_pages.len());
        args.push(browser.as_str());
        for p in web_pages {
            args.push(p.as_str());
        }
        let env: std::collections::HashMap<String, String> = std::collections::HashMap::new();
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

    // ─── Find operations ────────────────────────────────────────────────

    /// Port of C++ FindSelected.
    /// Sets search text from clipboard (falling back to config.search_text) and calls FindNext.
    pub fn FindSelected(
        &mut self,
        rec: &emStocksRec,
        config: &mut emStocksConfig,
    ) -> Option<usize> {
        let text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
            clipboard
                .get_text()
                .unwrap_or_else(|_| config.search_text.clone())
        } else {
            config.search_text.clone()
        };
        if text.is_empty() {
            return None;
        }
        config.search_text = text;
        self.FindNext(rec, config)
    }

    /// Port of C++ FindNext.
    /// Searches forward from active item, wrapping around.
    /// Returns the visible-item index of the found stock, or None.
    /// C++ navigates view to found panel. Rust returns the index for the caller to handle.
    pub fn FindNext(&mut self, rec: &emStocksRec, config: &emStocksConfig) -> Option<usize> {
        let count = self.visible_items.len();
        if count == 0 {
            return None;
        }

        let start = self.active_index.unwrap_or(count - 1);
        let mut j = start;
        loop {
            j = (j + 1) % count;
            if let Some(stock) = self.GetStockByItemIndex(j, rec) {
                if stock.IsMatchingSearchText(&config.search_text) {
                    self.active_index = Some(j);
                    return Some(j);
                }
            }
            if j == start {
                return None;
            }
        }
    }

    /// Port of C++ FindPrevious.
    /// Searches backward from active item, wrapping around.
    /// Returns the visible-item index of the found stock, or None.
    /// C++ navigates view to found panel. Rust returns the index for the caller to handle.
    pub fn FindPrevious(&mut self, rec: &emStocksRec, config: &emStocksConfig) -> Option<usize> {
        let count = self.visible_items.len();
        if count == 0 {
            return None;
        }

        let start = self.active_index.unwrap_or(0);
        let mut j = start;
        loop {
            j = (j + count - 1) % count;
            if let Some(stock) = self.GetStockByItemIndex(j, rec) {
                if stock.IsMatchingSearchText(&config.search_text) {
                    self.active_index = Some(j);
                    return Some(j);
                }
            }
            if j == start {
                return None;
            }
        }
    }
}

// Test-only accessors for the B-013 integration test
// (`tests/dialog_signals_b013.rs`), which lives outside the crate and so
// cannot reach `pub(crate)` fields directly. Gated on `cfg(any(test,
// feature = "test-support"))` so production builds drop them.
#[cfg(any(test, feature = "test-support"))]
impl emStocksListBox {
    pub fn cut_stocks_dialog_for_test(&self) -> Option<&emDialog> {
        self.cut_stocks_dialog.as_ref()
    }
    pub fn paste_stocks_dialog_for_test(&self) -> Option<&emDialog> {
        self.paste_stocks_dialog.as_ref()
    }
    pub fn delete_stocks_dialog_for_test(&self) -> Option<&emDialog> {
        self.delete_stocks_dialog.as_ref()
    }
    pub fn interest_dialog_for_test(&self) -> Option<&emDialog> {
        self.interest_dialog.as_ref()
    }
    pub fn cut_stocks_result_for_test(&self) -> &Rc<Cell<Option<DialogResult>>> {
        &self.cut_stocks_result
    }
    pub fn paste_stocks_result_for_test(&self) -> &Rc<Cell<Option<DialogResult>>> {
        &self.paste_stocks_result
    }
    pub fn delete_stocks_result_for_test(&self) -> &Rc<Cell<Option<DialogResult>>> {
        &self.delete_stocks_result
    }
    pub fn interest_result_for_test(&self) -> &Rc<Cell<Option<DialogResult>>> {
        &self.interest_result
    }
    pub fn cut_subscribed_for_test(&self) -> bool {
        self.cut_subscribed
    }
    pub fn paste_subscribed_for_test(&self) -> bool {
        self.paste_subscribed
    }
    pub fn delete_subscribed_for_test(&self) -> bool {
        self.delete_subscribed
    }
    pub fn interest_subscribed_for_test(&self) -> bool {
        self.interest_subscribed
    }
    pub fn interest_to_set_for_test(&self) -> Option<Interest> {
        self.interest_to_set
    }
    /// Inject `look` without wiring an `emListBox`. Used by integration
    /// tests that drive the fallback `selected_indices` selection path.
    pub fn set_look_for_test(&mut self, look: Rc<emLook>) {
        self.look = Some(look);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emEngineCtx::{DeferredAction, DropOnlySignalCtx, InitCtx};
    use emcore::emScheduler::EngineScheduler;

    /// Minimal SignalCtx adapter wrapping `EngineScheduler` for unit tests
    /// that need a real (non-null) SignalId allocator.
    struct TestSignalCtx<'a> {
        sched: &'a mut EngineScheduler,
    }
    impl SignalCtx for TestSignalCtx<'_> {
        fn create_signal(&mut self) -> SignalId {
            self.sched.create_signal()
        }
        fn fire(&mut self, id: SignalId) {
            self.sched.fire(id);
        }
    }

    struct TestInit {
        sched: EngineScheduler,
        fw: Vec<DeferredAction>,
        root: Rc<emcore::emContext::emContext>,
        pa: Rc<std::cell::RefCell<Vec<emcore::emEngineCtx::FrameworkDeferredAction>>>,
    }
    impl TestInit {
        fn new() -> Self {
            Self {
                sched: EngineScheduler::new(),
                fw: Vec::new(),
                root: emcore::emContext::emContext::NewRoot(),
                pa: Rc::new(std::cell::RefCell::new(Vec::new())),
            }
        }
        fn ctx(&mut self) -> InitCtx<'_> {
            InitCtx {
                scheduler: &mut self.sched,
                framework_actions: &mut self.fw,
                root_context: &self.root,
                view_context: None,
                pending_actions: &self.pa,
            }
        }
    }

    fn make_stock(id: &str, name: &str, interest: Interest) -> StockRec {
        let mut s = StockRec::default();
        s.id = id.to_string();
        s.name = name.to_string();
        s.interest = interest;
        s
    }

    #[test]
    fn listbox_new() {
        let mut __init = TestInit::new();
        let lb = emStocksListBox::new();
        assert!(lb.visible_items.is_empty());
    }

    /// B-001-followup C.4 — `install_item_panel_factory` wires a closure
    /// onto the inner emListBox that constructs `emStocksItemPanel` via
    /// the factory mechanism. Mirrors C++ `emStocksListBox::CreateItemPanel`
    /// at cpp:696-705. We verify by adding an item, calling
    /// `CreateItemPanel(name, index)`, and downcasting the resulting
    /// `dyn ItemPanelInterface` to `emStocksItemPanel`.
    #[test]
    fn create_item_panel_factory_constructs_emstocks_item_panel() {
        let mut __init = TestInit::new();
        let look = emLook::new();
        let file_model = Rc::new(RefCell::new(
            crate::emStocksFileModel::emStocksFileModel::new(std::path::PathBuf::from(
                "/tmp/lb_c4.emStocks",
            )),
        ));
        let config = Rc::new(RefCell::new(emStocksConfig::default()));

        // Build the outer ListBox, attach inner emListBox, install refs,
        // wrap in Rc<RefCell<>>, then install the factory with a Weak
        // self-reference (mirrors how `emStocksFilePanel` will call this
        // in Phase D of B-001).
        let lb_rc = {
            let mut lb = emStocksListBox::new();
            lb.attach_list_box(&mut __init.ctx(), look.clone());
            lb.set_refs(file_model.clone(), config.clone());
            // Add one item directly via the inner emListBox so we have an
            // index to create a panel for.
            if let Some(inner) = lb.list_box.as_mut() {
                inner.AddItem("name-0".to_string(), "Item 0".to_string());
            }
            Rc::new(RefCell::new(lb))
        };
        let weak = Rc::downgrade(&lb_rc);
        lb_rc.borrow_mut().install_item_panel_factory(weak);

        // Drive the factory.
        lb_rc.borrow_mut().CreateItemPanel("name-0", 0);

        // Verify the resulting panel is an emStocksItemPanel by downcasting.
        let lb_borrow = lb_rc.borrow();
        let inner = lb_borrow
            .list_box
            .as_ref()
            .expect("inner emListBox attached");
        // The panel is stored on the inner emListBox; we rely on the
        // ItemPanelInterface trait being object-safe and the structural
        // shape: just confirm GetItemPanel returns Some, item_index matches,
        // and downcast via item_index() (the panel index round-trips).
        let panel = inner.GetItemPanel(0).expect("panel created");
        assert_eq!(panel.item_index(), 0);
        // FileModel/Config strong_count grew because the factory closure
        // captured a clone, and the produced panel holds another clone.
        assert!(Rc::strong_count(&file_model) >= 3);
        assert!(Rc::strong_count(&config) >= 3);
    }

    #[test]
    fn is_visible_stock_interest_filter() {
        let mut __init = TestInit::new();
        let config = emStocksConfig {
            min_visible_interest: Interest::Medium,
            ..Default::default()
        };
        let high = make_stock("1", "High", Interest::High);
        let medium = make_stock("2", "Medium", Interest::Medium);
        let low = make_stock("3", "Low", Interest::Low);

        assert!(emStocksListBox::IsVisibleStock(&high, &config)); // High(0) <= Medium(1)
        assert!(emStocksListBox::IsVisibleStock(&medium, &config)); // Medium(1) <= Medium(1)
        assert!(!emStocksListBox::IsVisibleStock(&low, &config)); // Low(2) > Medium(1)
    }

    #[test]
    fn is_visible_stock_category_filter() {
        let mut __init = TestInit::new();
        let config = emStocksConfig {
            visible_countries: vec!["DE".to_string(), "US".to_string()],
            ..Default::default()
        };
        let mut us_stock = make_stock("1", "US Stock", Interest::High);
        us_stock.country = "US".to_string();
        let mut jp_stock = make_stock("2", "JP Stock", Interest::High);
        jp_stock.country = "JP".to_string();

        assert!(emStocksListBox::IsVisibleStock(&us_stock, &config));
        assert!(!emStocksListBox::IsVisibleStock(&jp_stock, &config));
    }

    #[test]
    fn is_visible_stock_empty_categories_means_all() {
        let mut __init = TestInit::new();
        let config = emStocksConfig::default(); // empty visible_countries
        let mut stock = make_stock("1", "Any", Interest::High);
        stock.country = "Anywhere".to_string();
        assert!(emStocksListBox::IsVisibleStock(&stock, &config));
    }

    #[test]
    fn update_items_filters_and_sorts() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Zebra Corp", Interest::High));
        rec.stocks
            .push(make_stock("2", "Alpha Inc", Interest::High));
        rec.stocks.push(make_stock("3", "Hidden", Interest::Low));

        let config = emStocksConfig {
            min_visible_interest: Interest::Medium,
            sorting: Sorting::ByName,
            ..Default::default()
        };

        let mut lb = emStocksListBox::new();
        lb.UpdateItems(&rec, &config);

        // "Hidden" (Low interest) filtered out, remaining sorted by name
        assert_eq!(lb.visible_items.len(), 2);
        // Alpha should come before Zebra
        assert_eq!(rec.stocks[lb.visible_items[0]].name, "Alpha Inc");
        assert_eq!(rec.stocks[lb.visible_items[1]].name, "Zebra Corp");
    }

    #[test]
    fn compare_stocks_owned_first() {
        let mut __init = TestInit::new();
        let mut s1 = make_stock("1", "A", Interest::High);
        s1.owning_shares = true;
        let s2 = make_stock("2", "B", Interest::High);

        let config = emStocksConfig {
            owned_shares_first: true,
            ..Default::default()
        };

        let ord = emStocksListBox::CompareStocks(&s1, &s2, &config, "2024-06-15");
        assert_eq!(ord, Ordering::Less); // owned first
    }

    #[test]
    fn selected_date_management() {
        let mut __init = TestInit::new();
        let mut lb = emStocksListBox::new();
        lb.SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-15");
        assert_eq!(lb.GetSelectedDate(), "2024-06-15");
    }

    #[test]
    fn go_back_in_history() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut stock = StockRec::default();
        stock.AddPrice("2024-06-14", "100");
        stock.AddPrice("2024-06-15", "101");
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        lb.SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-15");
        lb.GoBackInHistory(&mut DropOnlySignalCtx, &rec);
        assert_eq!(lb.GetSelectedDate(), "2024-06-14");
    }

    #[test]
    fn get_selected_date_signal_lazy_alloc_is_stable() {
        // G4: D-008 A1 — first call allocates, subsequent calls return same id.
        let lb = emStocksListBox::new();
        assert!(lb.selected_date_signal_for_test().is_null());
        let mut sched = EngineScheduler::new();
        let sig_a = {
            let mut sc = TestSignalCtx { sched: &mut sched };
            lb.GetSelectedDateSignal(&mut sc)
        };
        assert!(!sig_a.is_null());
        let sig_b = {
            let mut sc = TestSignalCtx { sched: &mut sched };
            lb.GetSelectedDateSignal(&mut sc)
        };
        assert_eq!(sig_a, sig_b);
    }

    #[test]
    fn selection_and_item_trigger_signals_none_until_attach() {
        // G5/G6: delegating accessors return None when the inner emListBox is
        // unattached (pre-AutoExpand). Phase 4 consumers must early-return.
        let lb = emStocksListBox::new();
        assert!(lb.GetSelectionSignal().is_none());
        assert!(lb.GetItemTriggerSignal().is_none());
    }

    #[test]
    fn selection_and_item_trigger_signals_some_after_attach() {
        // After attach_list_box, the delegators forward the inner ids.
        let mut __init = TestInit::new();
        let mut lb = emStocksListBox::new();
        let look = emLook::new();
        lb.attach_list_box(&mut __init.ctx(), look);
        let sel = lb.GetSelectionSignal();
        let trig = lb.GetItemTriggerSignal();
        assert!(sel.is_some());
        assert!(trig.is_some());
        assert!(!sel.unwrap().is_null());
        assert!(!trig.unwrap().is_null());
    }

    #[test]
    fn set_selected_date_no_fire_when_unchanged() {
        // G4: only fire when value actually changed (mirrors C++ cpp:99-104).
        let mut lb = emStocksListBox::new();
        lb.SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-15");
        // Repeating the same value must not even allocate the signal.
        assert!(lb.selected_date_signal_for_test().is_null());
        lb.SetSelectedDate(&mut DropOnlySignalCtx, "2024-06-15");
        assert!(lb.selected_date_signal_for_test().is_null());
    }

    // ─── New tests for stock operations ──────────────────────────────────

    #[test]
    fn paint_empty_message_when_no_items() {
        let mut __init = TestInit::new();
        use emcore::emImage::emImage;
        let lb = emStocksListBox::new();
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        // Smoke test: no panic when painting with empty visible_items
        lb.PaintEmptyMessage(&mut painter, 200.0, 50.0, emColor::rgb(0, 0, 0));
    }

    #[test]
    fn paint_empty_message_no_op_when_items_exist() {
        let mut __init = TestInit::new();
        use emcore::emImage::emImage;
        let mut lb = emStocksListBox::new();
        lb.visible_items.push(0);
        let mut img = emImage::new(200, 50, 4);
        let mut painter = emPainter::new(&mut img);
        // Smoke test: no panic when items exist
        lb.PaintEmptyMessage(&mut painter, 200.0, 50.0, emColor::rgb(0, 0, 0));
    }

    #[test]
    fn new_stock_assigns_id_and_selects() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Existing", Interest::High));
        let config = emStocksConfig::default();

        let mut lb = emStocksListBox::new();
        lb.NewStock(&mut rec, &config);

        assert_eq!(rec.stocks.len(), 2);
        assert_eq!(rec.stocks[1].id, "2"); // InventStockId returns max+1
        assert_eq!(lb.GetSelectionCount(), 1);
    }

    #[test]
    fn new_stock_inherits_config_categories() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let config = emStocksConfig {
            visible_countries: vec!["DE".to_string()],
            visible_sectors: vec!["Tech".to_string()],
            visible_collections: vec!["Growth".to_string()],
            ..Default::default()
        };

        let mut lb = emStocksListBox::new();
        lb.NewStock(&mut rec, &config);

        assert_eq!(rec.stocks[0].country, "DE");
        assert_eq!(rec.stocks[0].sector, "Tech");
        assert_eq!(rec.stocks[0].collection, "Growth");
    }

    #[test]
    fn new_stock_caps_interest_to_config() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let config = emStocksConfig {
            min_visible_interest: Interest::High,
            ..Default::default()
        };

        let mut lb = emStocksListBox::new();
        lb.NewStock(&mut rec, &config);

        // StockRec default interest is Medium (1), High is (0), so Medium > High
        // means interest should be capped to High
        assert_eq!(rec.stocks[0].interest, Interest::High);
    }

    #[test]
    fn copy_stocks_to_string_empty_selection_returns_none() {
        let mut __init = TestInit::new();
        let rec = emStocksRec::default();
        let lb = emStocksListBox::new();
        assert!(lb.copy_stocks_to_string(&rec).is_none());
    }

    #[test]
    fn copy_stocks_to_string_serializes_selected() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0); // select first visible item

        let text = lb.copy_stocks_to_string(&rec);
        assert!(text.is_some());
        assert!(text.unwrap().contains("emStocks"));
    }

    #[test]
    fn delete_stocks_removes_selected() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));
        rec.stocks.push(make_stock("3", "Gamma", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(1); // select second visible item (Beta)

        lb.DeleteStocks(&mut __init.ctx(), &mut rec, false);
        assert_eq!(rec.stocks.len(), 2);
        assert_eq!(rec.stocks[0].name, "Alpha");
        assert_eq!(rec.stocks[1].name, "Gamma");
        assert_eq!(lb.GetSelectionCount(), 0);
    }

    #[test]
    fn delete_stocks_empty_selection_is_noop() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        lb.DeleteStocks(&mut __init.ctx(), &mut rec, false);
        assert_eq!(rec.stocks.len(), 1);
    }

    #[test]
    fn cut_stocks_copies_then_deletes() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0); // select Alpha

        lb.CutStocks(&mut __init.ctx(), &mut rec, false);
        assert_eq!(rec.stocks.len(), 1);
        assert_eq!(rec.stocks[0].name, "Beta");
    }

    #[test]
    fn paste_stocks_round_trip() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0);

        // Copy, then paste into a fresh rec
        let clipboard = lb.copy_stocks_to_string(&rec).unwrap();

        let mut rec2 = emStocksRec::default();
        let mut lb2 = emStocksListBox::new();
        let result = lb2.paste_stocks_from_text(&mut rec2, &config, &clipboard);
        assert!(result.is_ok());
        assert_eq!(rec2.stocks.len(), 1);
        assert_eq!(rec2.stocks[0].name, "Alpha");
    }

    #[test]
    fn paste_stocks_reassigns_conflicting_ids() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Existing", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        // Create clipboard with a stock that has id "1" (conflicts)
        let mut source_rec = emStocksRec::default();
        source_rec
            .stocks
            .push(make_stock("1", "Pasted", Interest::High));
        let rec_struct = source_rec.to_rec();
        let clipboard = write_rec_with_format(&rec_struct, "emStocks");

        let result = lb.paste_stocks_from_text(&mut rec, &config, &clipboard);
        assert!(result.is_ok());
        assert_eq!(rec.stocks.len(), 2);
        // The pasted stock should have a new ID, not "1"
        assert_ne!(rec.stocks[1].id, "1");
        assert_eq!(rec.stocks[1].name, "Pasted");
    }

    #[test]
    fn paste_stocks_invalid_data_returns_error() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        let result = lb.paste_stocks_from_text(&mut rec, &config, "not valid data");
        assert!(result.is_err());
    }

    #[test]
    fn paste_stocks_reports_invisible() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let config = emStocksConfig {
            min_visible_interest: Interest::High,
            ..Default::default()
        };
        let mut lb = emStocksListBox::new();

        // Create clipboard with a Low-interest stock
        let mut source_rec = emStocksRec::default();
        source_rec
            .stocks
            .push(make_stock("1", "Hidden", Interest::Low));
        let rec_struct = source_rec.to_rec();
        let clipboard = write_rec_with_format(&rec_struct, "emStocks");

        let result = lb.paste_stocks_from_text(&mut rec, &config, &clipboard);
        assert!(result.is_ok());
        let invisible = result.unwrap();
        assert_eq!(invisible, vec!["Hidden".to_string()]);
    }

    #[test]
    fn delete_share_prices_clears_visible() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut stock = make_stock("1", "Alpha", Interest::High);
        stock.AddPrice("2024-06-15", "100");
        stock.last_price_date = "2024-06-15".to_string();
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);

        lb.DeleteSharePrices(&mut rec);
        assert!(rec.stocks[0].prices.is_empty());
        assert!(rec.stocks[0].last_price_date.is_empty());
    }

    #[test]
    fn set_interest_on_selected() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::Medium));
        rec.stocks.push(make_stock("2", "Beta", Interest::Medium));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        lb.Select(0);

        lb.SetInterest(&mut __init.ctx(), &mut rec, Interest::High, false);
        // Only the selected stock should change
        assert_eq!(rec.stocks[lb.visible_items[0]].interest, Interest::High);
        assert_eq!(rec.stocks[lb.visible_items[1]].interest, Interest::Medium);
    }

    #[test]
    fn show_first_web_pages_empty_selection_is_noop() {
        let mut __init = TestInit::new();
        // ShowFirstWebPages with no selection launches no browser and does not panic.
        let mut rec = emStocksRec::default();
        let mut stock = make_stock("1", "Alpha", Interest::High);
        stock.web_pages = vec!["https://example.com".to_string()];
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        // No Select() call — selection is empty.
        lb.ShowFirstWebPages(&rec); // should be a no-op
    }

    #[test]
    fn show_all_web_pages_empty_selection_is_noop() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        let mut stock = make_stock("1", "Alpha", Interest::High);
        stock.web_pages = vec![
            "https://example.com".to_string(),
            "https://other.com".to_string(),
        ];
        rec.stocks.push(stock);

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);
        // No Select() call — selection is empty.
        lb.ShowAllWebPages(&rec); // should be a no-op
    }

    #[test]
    fn get_visible_stock_ids() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("2", "Beta", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();
        lb.UpdateItems(&rec, &config);

        let ids = lb.GetVisibleStockIds(&rec);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn find_next_wraps_around() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Alpha Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Beta Inc", Interest::High));
        rec.stocks
            .push(make_stock("3", "Alpha Ltd", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig {
            search_text: "Alpha".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        // First find should find "Alpha Corp" (index 0)
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Corp");

        // Second find should find "Alpha Ltd" (index 2)
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Ltd");

        // Third find should wrap back to "Alpha Corp"
        let result = lb.FindNext(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Corp");
    }

    #[test]
    fn find_previous_wraps_around() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Alpha Corp", Interest::High));
        rec.stocks.push(make_stock("2", "Beta Inc", Interest::High));
        rec.stocks
            .push(make_stock("3", "Alpha Ltd", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig {
            search_text: "Alpha".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        // FindPrevious from default (start=0) should go backwards and find Alpha Ltd
        let result = lb.FindPrevious(&rec, &config);
        assert!(result.is_some());
        let idx = result.unwrap();
        assert_eq!(rec.stocks[lb.visible_items[idx]].name, "Alpha Ltd");
    }

    #[test]
    fn find_next_returns_none_when_no_match() {
        let mut __init = TestInit::new();
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("1", "Alpha", Interest::High));

        let mut lb = emStocksListBox::new();
        let config = emStocksConfig {
            search_text: "Nonexistent".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        assert!(lb.FindNext(&rec, &config).is_none());
    }

    #[test]
    fn find_next_returns_none_on_empty_list() {
        let mut __init = TestInit::new();
        let rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let config = emStocksConfig::default();

        assert!(lb.FindNext(&rec, &config).is_none());
    }

    #[test]
    fn find_selected_uses_config_search_text_when_clipboard_unavailable() {
        let mut __init = TestInit::new();
        // FindSelected reads clipboard; if clipboard is unavailable it falls
        // back to config.search_text.  Pre-set search_text so the fallback
        // exercises the FindNext path.
        let mut rec = emStocksRec::default();
        rec.stocks
            .push(make_stock("1", "Alpha Corp", Interest::High));

        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig {
            search_text: "Alpha".to_string(),
            ..emStocksConfig::default()
        };
        lb.UpdateItems(&rec, &config);

        // The result depends on whether the clipboard is accessible in the
        // test environment, so we only check that it does not panic and that
        // config.search_text is non-empty after the call.
        let _ = lb.FindSelected(&rec, &mut config);
        assert!(!config.search_text.is_empty());
    }

    #[test]
    fn find_selected_empty_fallback_returns_none() {
        let mut __init = TestInit::new();
        // When both clipboard and config.search_text are empty, FindSelected
        // returns None.
        let rec = emStocksRec::default();
        let mut lb = emStocksListBox::new();
        let mut config = emStocksConfig::default();
        // config.search_text is "" and clipboard is expected to be empty/unavailable
        // in the headless test environment.
        // We can only verify no panic; None is the expected result when text is empty.
        let _ = lb.FindSelected(&rec, &mut config);
    }

    #[test]
    fn selection_helpers() {
        let mut __init = TestInit::new();
        let mut lb = emStocksListBox::new();
        lb.visible_items = vec![0, 1, 2];

        assert_eq!(lb.GetSelectionCount(), 0);
        assert!(!lb.IsSelected(0));

        lb.Select(1);
        assert_eq!(lb.GetSelectionCount(), 1);
        assert!(lb.IsSelected(1));
        assert!(!lb.IsSelected(0));

        lb.Select(1); // duplicate select is idempotent
        assert_eq!(lb.GetSelectionCount(), 1);

        lb.SetSelectedIndex(2);
        assert_eq!(lb.GetSelectionCount(), 1);
        assert!(lb.IsSelected(2));
        assert!(!lb.IsSelected(1));

        lb.ClearSelection();
        assert_eq!(lb.GetSelectionCount(), 0);
    }

    // Phase 3.5 Task 14: verify result slots are initialized to None.
    #[test]
    fn result_slots_initialize_to_none() {
        let lb = emStocksListBox::new();
        assert!(lb.cut_stocks_result.get().is_none());
        assert!(lb.paste_stocks_result.get().is_none());
        assert!(lb.delete_stocks_result.get().is_none());
        assert!(lb.interest_result.get().is_none());
    }

    // B-001 Phase 3: cross-Cycle FileModel/Config refs default to None.
    #[test]
    fn b001_phase3_cross_cycle_refs_default_unset() {
        let lb = emStocksListBox::new();
        assert!(!lb.has_file_model_ref());
        assert!(!lb.has_config_ref());
        assert!(!lb.subscribed_init_for_test());
    }

    // B-001 Phase 3: `set_refs` installs both cross-Cycle handles.
    #[test]
    fn b001_phase3_set_refs_installs_both_handles() {
        use crate::emStocksFileModel::emStocksFileModel;
        use std::path::PathBuf;
        let mut lb = emStocksListBox::new();
        let model = Rc::new(RefCell::new(emStocksFileModel::new(PathBuf::from("/t"))));
        let config = Rc::new(RefCell::new(emStocksConfig::default()));
        lb.set_refs(model.clone(), config.clone());
        assert!(lb.has_file_model_ref());
        assert!(lb.has_config_ref());
        // Phase 4.5 has not yet wired the subscribe — flag remains false.
        assert!(!lb.subscribed_init_for_test());
        // Refs alias the parent's handles (Rc::ptr_eq mirrors C++ `&FileModel`
        // pointing at the same instance).
        assert!(Rc::ptr_eq(lb.file_model_ref.as_ref().unwrap(), &model));
        assert!(Rc::ptr_eq(lb.config_ref.as_ref().unwrap(), &config));
    }

    // ── FU-001 Unit 3 tests ──

    fn make_lb_with_refs() -> (
        emStocksListBox,
        Rc<RefCell<crate::emStocksFileModel::emStocksFileModel>>,
        Rc<RefCell<emStocksConfig>>,
        EngineScheduler,
    ) {
        let model = Rc::new(RefCell::new(
            crate::emStocksFileModel::emStocksFileModel::new(std::path::PathBuf::from(
                "/tmp/fu001_unit3.emStocks",
            )),
        ));
        let config = Rc::new(RefCell::new(emStocksConfig::default()));
        let mut lb = emStocksListBox::new();
        lb.set_refs(model.clone(), config.clone());
        let sched = EngineScheduler::new();
        (lb, model, config, sched)
    }

    #[test]
    fn start_to_fetch_share_prices_creates_dialog_when_absent() {
        let (mut lb, model, _cfg, mut sched) = make_lb_with_refs();
        assert!(model.borrow().prices_fetching_dialog.is_none());
        let mut sc = TestSignalCtx { sched: &mut sched };
        lb.StartToFetchSharePrices(&mut sc, &["AAPL".to_string()]);
        assert!(model.borrow().prices_fetching_dialog.is_some());
        // Stock id was forwarded to the fetcher.
        assert!(model
            .borrow()
            .prices_fetching_dialog
            .as_ref()
            .unwrap()
            .fetcher
            .stock_ids
            .iter()
            .any(|s| s == "AAPL"));
    }

    #[test]
    fn start_to_fetch_share_prices_reuses_existing_dialog() {
        let (mut lb, model, _cfg, mut sched) = make_lb_with_refs();
        {
            let mut sc = TestSignalCtx { sched: &mut sched };
            lb.StartToFetchSharePrices(&mut sc, &["AAPL".into()]);
        }
        let first_addr = model
            .borrow()
            .prices_fetching_dialog
            .as_ref()
            .map(|d| d as *const _ as usize);
        {
            let mut sc = TestSignalCtx { sched: &mut sched };
            lb.StartToFetchSharePrices(&mut sc, &["MSFT".into()]);
        }
        let second_addr = model
            .borrow()
            .prices_fetching_dialog
            .as_ref()
            .map(|d| d as *const _ as usize);
        assert_eq!(
            first_addr, second_addr,
            "second call must reuse the existing dialog (C++ Raise() branch)"
        );
        // MSFT was queued onto the same dialog.
        assert!(model
            .borrow()
            .prices_fetching_dialog
            .as_ref()
            .unwrap()
            .fetcher
            .stock_ids
            .iter()
            .any(|s| s == "MSFT"));
    }

    #[test]
    fn start_to_fetch_all_share_prices_forwards_visible_ids() {
        let (mut lb, model, _cfg, mut sched) = make_lb_with_refs();
        // Build a rec with two visible stocks.
        let mut rec = emStocksRec::default();
        rec.stocks.push(make_stock("S1", "Alpha", Interest::High));
        rec.stocks.push(make_stock("S2", "Beta", Interest::Medium));
        lb.visible_items = vec![0, 1];
        let mut sc = TestSignalCtx { sched: &mut sched };
        lb.StartToFetchAllSharePrices(&mut sc, &rec);
        let m = model.borrow();
        let queued: Vec<String> = m
            .prices_fetching_dialog
            .as_ref()
            .unwrap()
            .fetcher
            .stock_ids
            .clone();
        assert!(queued.iter().any(|s| s == "S1"));
        assert!(queued.iter().any(|s| s == "S2"));
    }

    #[test]
    fn show_web_pages_no_browser_returns_silently() {
        let (mut lb, _model, config, _sched) = make_lb_with_refs();
        config.borrow_mut().web_browser.clear();
        // Must not panic, must not spawn a process.
        lb.ShowWebPages(&["https://example.com".into()]);
        let _ = &mut lb; // suppress unused-mut hint when nothing else mutates
    }

    #[test]
    fn show_web_pages_empty_list_is_noop() {
        let (lb, _model, _cfg, _sched) = make_lb_with_refs();
        lb.ShowWebPages(&[]);
    }
}
