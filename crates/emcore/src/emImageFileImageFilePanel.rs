// SPLIT: Split from emImageFile.h — panel type extracted
use std::cell::RefCell;
use std::rc::Rc;

use slotmap::Key as _;

use crate::emColor::emColor;
use crate::emEngineCtx::{EngineCtx, PanelCtx};
use crate::emFilePanel::emFilePanel;
use crate::emImage::emImage;
use crate::emImageFile::emImageFileModel;
use crate::emPainter::emPainter;
use crate::emPanel::{PanelBehavior, PanelState};
use crate::emSignal::SignalId;

/// A panel that displays an image file with aspect-ratio preservation.
///
/// Port of C++ `emImageFilePanel`. Wraps a `emFilePanel` for status display
/// and holds a cached copy of the current image for painting.
pub struct emImageFilePanel {
    file_panel: emFilePanel,
    current_image: Option<emImage>,
    /// Typed handle to the currently-bound emImageFileModel, if any.
    /// Enables subscription to GetChangeSignal() without going through
    /// the type-erased dyn FileModelState held by emFilePanel.
    image_model: Option<Rc<RefCell<emImageFileModel>>>,
    /// The change signal we are currently subscribed to, or null.
    /// Used to detect model swaps in Cycle and re-bind.
    subscribed_change_signal: SignalId,
    /// B-007 row -139: true after first-Cycle init has run.
    subscribed_init: bool,
}

impl Default for emImageFilePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl emImageFilePanel {
    pub fn new() -> Self {
        Self {
            file_panel: emFilePanel::new(),
            current_image: None,
            image_model: None,
            subscribed_change_signal: SignalId::null(),
            subscribed_init: false,
        }
    }

    pub fn with_model() -> Self {
        Self {
            file_panel: emFilePanel::new(),
            current_image: None,
            image_model: None,
            subscribed_change_signal: SignalId::null(),
            subscribed_init: false,
        }
    }

    pub fn file_panel(&self) -> &emFilePanel {
        &self.file_panel
    }

    pub fn file_panel_mut(&mut self) -> &mut emFilePanel {
        &mut self.file_panel
    }

    /// Bind an `emImageFileModel` to this panel. Updates the underlying
    /// `emFilePanel` (type-erased model) and stores the typed handle for
    /// signal subscription in Cycle.
    ///
    /// B-007 row -139 option B: subscription is handled in Cycle-time re-bind
    /// (not here) because EngineCtx is needed for connect/disconnect and callers
    /// of SetImageFileModel do not hold an EngineCtx.
    pub fn SetImageFileModel(&mut self, model: Option<Rc<RefCell<emImageFileModel>>>) {
        // emImageFileModel implements FileModelState (via the impl in emImageFile.rs).
        let dyn_model = model
            .as_ref()
            .map(|m| m.clone() as Rc<RefCell<dyn crate::emFileModel::FileModelState>>);
        self.file_panel.SetFileModel(dyn_model);
        self.image_model = model;
        // Reset subscribed_init so Cycle re-binds to the new model's signal.
        self.subscribed_init = false;
    }

    /// Update the cached image for painting.
    pub fn set_current_image(&mut self, image: Option<emImage>) {
        self.current_image = image;
    }

    /// Test accessor: returns a reference to the current_image Option.
    /// Used by tests to verify signal-driven cache invalidation.
    pub fn current_image_for_test(&self) -> &Option<emImage> {
        &self.current_image
    }

    /// Calculate the aspect-ratio-preserving rectangle for the image within
    /// the panel bounds. Returns `(x, y, w, h)` or `None` if no image.
    ///
    /// Port of C++ `emImageFilePanel::GetEssenceRect`. The image is centered
    /// within panel width 1.0 and proportional height.
    pub fn GetEssenceRect(&self, panel_w: f64, panel_h: f64) -> Option<(f64, f64, f64, f64)> {
        let image = self.current_image.as_ref()?;
        let iw = image.GetWidth() as f64;
        let ih = image.GetHeight() as f64;
        if iw <= 0.0 || ih <= 0.0 || panel_w <= 0.0 || panel_h <= 0.0 {
            return None;
        }

        let image_aspect = iw / ih;
        let panel_aspect = panel_w / panel_h;

        if image_aspect > panel_aspect {
            // emImage is wider than panel — fit to width, center vertically
            let w = panel_w;
            let h = panel_w / image_aspect;
            let x = 0.0;
            let y = (panel_h - h) * 0.5;
            Some((x, y, w, h))
        } else {
            // emImage is taller than panel — fit to height, center horizontally
            let h = panel_h;
            let w = panel_h * image_aspect;
            let x = (panel_w - w) * 0.5;
            let y = 0.0;
            Some((x, y, w, h))
        }
    }
}

impl PanelBehavior for emImageFilePanel {
    /// B-007 row -139: D-006 first-Cycle init + model-swap re-bind + IsSignaled reaction.
    ///
    /// Mirrors C++ `emImageFilePanel::SetFileModel` subscribe pair + `Cycle` reaction:
    ///   emImageFile.cpp:120-141 (SetFileModel subscribe/unsubscribe)
    ///   + Cycle reaction that re-reads the image from the (now-loaded) model.
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, _pctx: &mut PanelCtx) -> bool {
        let eid = ectx.id();

        // Detect model swap: if the model changed since we last subscribed,
        // disconnect from the old signal and reset init.
        if let Some(model_rc) = &self.image_model {
            let current_sig = model_rc.borrow().GetChangeSignal();
            if current_sig != self.subscribed_change_signal {
                // Disconnect from the old signal if we had one.
                if !self.subscribed_change_signal.is_null() {
                    ectx.disconnect(self.subscribed_change_signal, eid);
                }
                self.subscribed_init = false;
                self.subscribed_change_signal = SignalId::null();
            }
        }

        // B-007 row -139: D-006 first-Cycle init — subscribe to model's change signal.
        // Mirrors C++ emImageFile.cpp:139 AddWakeUpSignal(model.GetChangeSignal()).
        if !self.subscribed_init {
            if let Some(model_rc) = &self.image_model {
                let sig = model_rc.borrow().GetChangeSignal();
                if !sig.is_null() {
                    ectx.connect(sig, eid);
                    self.subscribed_change_signal = sig;
                }
            }
            self.subscribed_init = true;
        }

        // B-007 row -139: IsSignaled reaction.
        // Mirrors C++ emImageFilePanel::Cycle — if the model changed, invalidate
        // the cached image so Paint re-reads it on the next repaint.
        if !self.subscribed_change_signal.is_null()
            && ectx.IsSignaled(self.subscribed_change_signal)
        {
            self.current_image = None;
        }

        false
    }

    fn IsOpaque(&self) -> bool {
        if self.file_panel.GetVirFileState().is_good() {
            false
        } else {
            self.file_panel.IsOpaque()
        }
    }

    fn GetCanvasColor(&self) -> emColor {
        self.file_panel.GetCanvasColor()
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        if !self.file_panel.GetVirFileState().is_good() {
            self.file_panel.Paint(painter, canvas_color, w, h, state);
            return;
        }

        if let Some(ref image) = self.current_image {
            if let Some((ix, iy, iw, ih)) = self.GetEssenceRect(w, h) {
                painter.paint_image_full(ix, iy, iw, ih, image, 255, canvas_color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emEngine::{emEngine, Priority};
    use crate::emEngineCtx::{EngineCtx, PanelCtx};
    use crate::emImageFile::{emImageFileModel, ImageFileData};
    use crate::emPanel::PanelBehavior;
    use crate::emPanelScope::PanelScope;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::EngineScheduler;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::rc::Rc;

    /// B-007 row -139 click-through: emImageFilePanel::Cycle subscribes to
    /// model.GetChangeSignal() and clears current_image when signaled.
    ///
    /// Mirrors C++ emImageFile.cpp:139 + Cycle reaction.
    #[test]
    fn change_signal_clears_current_image_via_cycle() {
        let mut sched = EngineScheduler::new();
        let file_update_signal = sched.create_signal();
        sched.file_update_signal = file_update_signal;

        let change_sig = sched.create_signal();
        let data_change_sig = sched.create_signal();

        let mut model_inner = emImageFileModel::new(
            PathBuf::from("/tmp/b007_inner_row139.tga"),
            change_sig,
            data_change_sig,
        );
        model_inner
            .file_model_mut()
            .complete_load(ImageFileData::default());
        let model_rc = Rc::new(RefCell::new(model_inner));

        // Set up the panel with a bound model and a cached image.
        let panel_rc: Rc<RefCell<emImageFilePanel>> =
            Rc::new(RefCell::new(emImageFilePanel::new()));
        {
            let mut panel = panel_rc.borrow_mut();
            panel.SetImageFileModel(Some(model_rc.clone()));
            panel.set_current_image(Some(crate::emImage::emImage::new(4, 4, 4)));
        }

        assert!(
            panel_rc.borrow().current_image_for_test().is_some(),
            "panel must have current_image before signal"
        );

        struct PanelEngine {
            panel: Rc<RefCell<emImageFilePanel>>,
            tree: PanelTree,
            root: crate::emPanelTree::PanelId,
            cycles_run: u32,
        }
        impl emEngine for PanelEngine {
            fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
                let mut pctx = PanelCtx::new(&mut self.tree, self.root, 1.0);
                self.panel.borrow_mut().Cycle(ctx, &mut pctx);
                self.cycles_run += 1;
                self.cycles_run < 3
            }
        }

        let root_ctx = crate::emContext::emContext::NewRoot();
        let mut tree = PanelTree::new();
        let root = tree.create_root_deferred_view("b007_row139_inner");
        let engine = Box::new(PanelEngine {
            panel: panel_rc.clone(),
            tree,
            root,
            cycles_run: 0,
        });
        let eid = sched.register_engine(engine, Priority::Low, PanelScope::Framework);
        sched.wake_up(eid);

        // First slice: Cycle runs, subscribes to model's GetChangeSignal.
        {
            let mut windows: HashMap<winit::window::WindowId, crate::emWindow::emWindow> =
                HashMap::new();
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let mut pi: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> = Vec::new();
            let mut is = crate::emInputState::emInputState::new();
            let cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
            let pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            sched.DoTimeSlice(&mut windows, &root_ctx, &mut fw, &mut pi, &mut is, &cb, &pa);
        }

        // Fire the model's change signal.
        sched.fire(data_change_sig);

        // Second slice: Cycle observes signal, clears current_image.
        {
            let mut windows: HashMap<winit::window::WindowId, crate::emWindow::emWindow> =
                HashMap::new();
            let mut fw: Vec<crate::emEngineCtx::DeferredAction> = Vec::new();
            let mut pi: Vec<(winit::window::WindowId, crate::emInput::emInputEvent)> = Vec::new();
            let mut is = crate::emInputState::emInputState::new();
            let cb: RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> = RefCell::new(None);
            let pa: Rc<RefCell<Vec<crate::emEngineCtx::FrameworkDeferredAction>>> =
                Rc::new(RefCell::new(Vec::new()));
            sched.DoTimeSlice(&mut windows, &root_ctx, &mut fw, &mut pi, &mut is, &cb, &pa);
        }

        assert!(
            panel_rc.borrow().current_image_for_test().is_none(),
            "B-007 row -139: current_image must be None after change signal"
        );

        sched.remove_engine(eid);
        sched.clear_pending_for_tests();
    }
}
