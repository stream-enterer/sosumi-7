use std::rc::Rc;

use emcore::emEngineCtx::ConstructCtx;
use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;

use crate::emDirPanel::emDirPanel;

/// Entry point for the directory panel plugin.
/// Loaded via `emDir.emFpPlugin` config file.
#[no_mangle]
pub fn emDirFpPluginFunc(
    _ctx: &mut dyn ConstructCtx,
    parent: &PanelParentArg,
    _name: &str,
    path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emDirFpPlugin: No properties allowed.".to_string();
        return None;
    }
    Some(Box::new(emDirPanel::new(
        Rc::clone(parent.root_context()),
        path.to_string(),
    )))
}
