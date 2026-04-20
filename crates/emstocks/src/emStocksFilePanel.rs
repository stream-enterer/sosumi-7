// Port of C++ emStocksFilePanel.h / emStocksFilePanel.cpp

use std::path::PathBuf;

use emcore::emColor::emColor;
use emcore::emEngineCtx::PanelCtx;
use emcore::emFilePanel::emFilePanel;
use emcore::emInput::{emInputEvent, InputKey, InputVariant};
use emcore::emInputState::emInputState;
use emcore::emPainter::emPainter;
use emcore::emPanel::{PanelBehavior, PanelState};

use super::emStocksConfig::{emStocksConfig, Sorting};
use super::emStocksFetchPricesDialog::emStocksFetchPricesDialog;
use super::emStocksFileModel::emStocksFileModel;
use super::emStocksListBox::emStocksListBox;
use super::emStocksRec::Interest;

/// Port of C++ emStocksFilePanel.
pub struct emStocksFilePanel {
    pub(crate) bg_color: emColor,
    pub(crate) config: emStocksConfig,
    pub(crate) fetch_dialog: Option<emStocksFetchPricesDialog>,
    pub(crate) list_box: Option<emStocksListBox>,
    pub(crate) model: emStocksFileModel,
    pub(crate) file_panel: emFilePanel,
}

impl PanelBehavior for emStocksFilePanel {
    fn Paint(&mut self, painter: &mut emPainter, w: f64, h: f64, _state: &PanelState) {
        if self.file_panel.GetVirFileState().is_good() {
            // C++: painter.Clear(BgColor, canvasColor) — canvasColor not passed
            // because Rust emPainter::Clear takes only one color argument.
            painter.Clear(self.bg_color);

            // C++: ListBox->Paint(...) checks GetItemCount()==0 and paints
            // "empty stock list" message.
            if let Some(ref list_box) = self.list_box {
                list_box.PaintEmptyMessage(painter, w, h, self.bg_color);
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
        _ctx: &mut PanelCtx,
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
                    self.config.min_visible_interest = Interest::High;
                    return true;
                }
                InputKey::Key('M') => {
                    self.config.min_visible_interest = Interest::Medium;
                    return true;
                }
                InputKey::Key('L') => {
                    self.config.min_visible_interest = Interest::Low;
                    return true;
                }
                // Sorting
                InputKey::Key('N') => {
                    self.config.sorting = Sorting::ByName;
                    return true;
                }
                InputKey::Key('T') => {
                    self.config.sorting = Sorting::ByTradeDate;
                    return true;
                }
                InputKey::Key('I') => {
                    self.config.sorting = Sorting::ByInquiryDate;
                    return true;
                }
                InputKey::Key('A') => {
                    self.config.sorting = Sorting::ByAchievement;
                    return true;
                }
                InputKey::Key('1') => {
                    self.config.sorting = Sorting::ByOneWeekRise;
                    return true;
                }
                InputKey::Key('3') => {
                    self.config.sorting = Sorting::ByThreeWeekRise;
                    return true;
                }
                InputKey::Key('9') => {
                    self.config.sorting = Sorting::ByNineWeekRise;
                    return true;
                }
                InputKey::Key('D') => {
                    self.config.sorting = Sorting::ByDividend;
                    return true;
                }
                InputKey::Key('P') => {
                    self.config.sorting = Sorting::ByPurchaseValue;
                    return true;
                }
                InputKey::Key('V') => {
                    self.config.sorting = Sorting::ByValue;
                    return true;
                }
                InputKey::Key('F') => {
                    self.config.sorting = Sorting::ByDifference;
                    return true;
                }
                // OwnedSharesFirst toggle
                InputKey::Key('O') => {
                    self.config.owned_shares_first = !self.config.owned_shares_first;
                    return true;
                }
                _ => {}
            }
        }

        // ── Ctrl shortcuts: ListBox operations ──
        if input_state.IsCtrlMod() {
            match event.key {
                InputKey::Key('J') => {
                    if let Some(ref mut list_box) = self.list_box {
                        list_box.GoBackInHistory(self.model.GetRec());
                    }
                    return true;
                }
                InputKey::Key('K') => {
                    if let Some(ref mut list_box) = self.list_box {
                        list_box.GoForwardInHistory(self.model.GetRec());
                    }
                    return true;
                }
                InputKey::Key('N') => {
                    // C++: ListBox->NewStock()
                    let Self {
                        list_box,
                        model,
                        config,
                        ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        lb.NewStock(model.GetWritableRec(), config);
                    }
                    return true;
                }
                InputKey::Key('X') => {
                    // C++: ListBox->CutStocks()
                    let Self {
                        list_box, model, ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        lb.CutStocks(model.GetWritableRec(), false);
                    }
                    return true;
                }
                InputKey::Key('C') => {
                    // C++: ListBox->CopyStocks()
                    if let Some(ref mut list_box) = self.list_box {
                        list_box.CopyStocks(self.model.GetRec());
                    }
                    return true;
                }
                InputKey::Key('V') => {
                    // C++: ListBox->PasteStocks()
                    let Self {
                        list_box,
                        model,
                        config,
                        ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        let _ = lb.PasteStocks(model.GetWritableRec(), config, false);
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
                        .map(|lb| lb.GetVisibleStockIds(self.model.GetRec()))
                        .unwrap_or_default();
                    if !ids.is_empty() {
                        let mut dialog = emStocksFetchPricesDialog::new(
                            &self.config.api_script,
                            &self.config.api_script_interpreter,
                            &self.config.api_key,
                        );
                        dialog.AddStockIds(&ids);
                        self.fetch_dialog = Some(dialog);
                    }
                    return true;
                }
                InputKey::Key('W') => {
                    // C++: ListBox->ShowFirstWebPages()
                    if let Some(ref mut list_box) = self.list_box {
                        list_box.ShowFirstWebPages(self.model.GetRec());
                    }
                    return true;
                }
                InputKey::Key('H') => {
                    // C++: ListBox->FindSelected()
                    let Self {
                        list_box,
                        model,
                        config,
                        ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        let _found = lb.FindSelected(model.GetRec(), config);
                    }
                    return true;
                }
                InputKey::Key('G') => {
                    // C++: ListBox->FindNext()
                    let Self {
                        list_box,
                        model,
                        config,
                        ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        let _found = lb.FindNext(model.GetRec(), config);
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
                    if let Some(ref mut list_box) = self.list_box {
                        list_box.ShowAllWebPages(self.model.GetRec());
                    }
                    return true;
                }
                InputKey::Key('G') => {
                    // C++: ListBox->FindPrevious()
                    let Self {
                        list_box,
                        model,
                        config,
                        ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        let _found = lb.FindPrevious(model.GetRec(), config);
                    }
                    return true;
                }
                _ => {}
            }
        }

        // ── No-modifier shortcuts ──
        if input_state.IsNoMod() && event.key == InputKey::Delete {
            // C++: ListBox->DeleteStocks()
            let Self {
                list_box, model, ..
            } = self;
            if let Some(lb) = list_box.as_mut() {
                lb.DeleteStocks(model.GetWritableRec(), false);
            }
            return true;
        }

        // ── Alt shortcuts: set interest on selected stocks ──
        if input_state.IsAltMod() {
            match event.key {
                InputKey::Key('H') => {
                    // C++: ListBox->SetInterest(HIGH_INTEREST)
                    let Self {
                        list_box, model, ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        lb.SetInterest(model.GetWritableRec(), Interest::High, false);
                    }
                    return true;
                }
                InputKey::Key('M') => {
                    // C++: ListBox->SetInterest(MEDIUM_INTEREST)
                    let Self {
                        list_box, model, ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        lb.SetInterest(model.GetWritableRec(), Interest::Medium, false);
                    }
                    return true;
                }
                InputKey::Key('L') => {
                    // C++: ListBox->SetInterest(LOW_INTEREST)
                    let Self {
                        list_box, model, ..
                    } = self;
                    if let Some(lb) = list_box.as_mut() {
                        lb.SetInterest(model.GetWritableRec(), Interest::Low, false);
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
        if let Some(ref mut list_box) = self.list_box {
            let rect = ctx.layout_rect();
            list_box.layout_x = 0.0;
            list_box.layout_y = 0.0;
            list_box.layout_w = rect.w; // width = 1.0
            list_box.layout_h = rect.h; // height = tallness
        }
    }

    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        let old_state = self.file_panel.GetVirFileState();
        self.file_panel.refresh_vir_file_state();
        let new_state = self.file_panel.GetVirFileState();
        let state_changed = old_state != new_state;
        if state_changed && new_state.is_good() && self.list_box.is_none() {
            self.list_box = Some(emStocksListBox::new());
        }
        self.model.CheckSaveTimer();

        // Poll fetch dialog
        if let Some(ref mut dialog) = self.fetch_dialog {
            if !dialog.Cycle() {
                // Dialog finished — clean up
                self.fetch_dialog = None;
            }
        }

        // Poll ListBox confirmation dialogs (C++: Cycle calls into ListBox state machine)
        let list_box_busy = {
            let Self {
                list_box,
                model,
                config,
                ..
            } = self;
            if let Some(lb) = list_box.as_mut() {
                lb.Cycle(model.GetWritableRec(), config)
            } else {
                false
            }
        };

        state_changed || self.fetch_dialog.is_some() || list_box_busy
    }

    fn GetIconFileName(&self) -> Option<String> {
        Some("documents.tga".to_string())
    }
}

impl emStocksFilePanel {
    pub(crate) fn new() -> Self {
        Self {
            bg_color: emColor::from_packed(0x131520FF),
            config: emStocksConfig::default(),
            fetch_dialog: None,
            list_box: None,
            model: emStocksFileModel::new(PathBuf::from("")),
            file_panel: emFilePanel::new(),
        }
    }
}

impl Default for emStocksFilePanel {
    fn default() -> Self {
        Self::new()
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
        panel.list_box = Some(emStocksListBox::new());
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
        panel.list_box = Some(emStocksListBox::new());
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
        assert_eq!(panel.config.min_visible_interest, Interest::High);
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
        assert_eq!(panel.config.min_visible_interest, Interest::Medium);
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
        assert_eq!(panel.config.min_visible_interest, Interest::Low);
    }

    #[test]
    fn shift_alt_n_sets_sort_by_name() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        panel.config.sorting = Sorting::ByValue; // set non-default
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
        assert_eq!(panel.config.sorting, Sorting::ByName);
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
                panel.config.sorting, expected_sorting,
                "Shift+Alt+{key} should set {expected_sorting:?}"
            );
        }
    }

    #[test]
    fn shift_alt_o_toggles_owned_shares_first() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        assert!(!panel.config.owned_shares_first);
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
        assert!(panel.config.owned_shares_first);
        // Toggle back
        assert!(panel.Input(
            &event,
            &state,
            &input_state,
            &mut make_test_pctx(&mut tree, root)
        ));
        assert!(!panel.config.owned_shares_first);
    }

    #[test]
    fn ctrl_j_goes_back_in_history() {
        let (mut tree, root) = make_test_tree();
        let mut panel = make_active_panel();
        // Set up rec with dates so GoBackInHistory works
        let mut stock = crate::emStocksRec::StockRec::default();
        stock.AddPrice("2024-06-14", "100");
        stock.AddPrice("2024-06-15", "101");
        panel.model.GetWritableRec().stocks.push(stock);
        panel
            .list_box
            .as_mut()
            .unwrap()
            .SetSelectedDate("2024-06-15");

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
            panel.list_box.as_ref().unwrap().GetSelectedDate(),
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
        panel.model.GetWritableRec().stocks.push(stock);
        panel
            .list_box
            .as_mut()
            .unwrap()
            .SetSelectedDate("2024-06-14");

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
            panel.list_box.as_ref().unwrap().GetSelectedDate(),
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
