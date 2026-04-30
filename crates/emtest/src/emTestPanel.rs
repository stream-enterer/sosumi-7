use emcore::emEngineCtx::ConstructCtx;
use emcore::emPanel::PanelBehavior;

pub(crate) fn new_root_panel(_ctx: &mut dyn ConstructCtx) -> Box<dyn PanelBehavior> {
    Box::new(StubPanel)
}

struct StubPanel;

impl PanelBehavior for StubPanel {
    fn IsOpaque(&self) -> bool {
        true
    }
}
