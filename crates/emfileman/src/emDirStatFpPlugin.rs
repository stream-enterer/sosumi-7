use std::rc::Rc;

use emcore::emEngineCtx::ConstructCtx;
use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;

use crate::emDirStatPanel::emDirStatPanel;

/// Entry point for the directory statistics panel plugin.
/// Loaded via `emDirStat.emFpPlugin` config file.
#[no_mangle]
pub fn emDirStatFpPluginFunc(
    _ctx: &mut dyn ConstructCtx,
    parent: &PanelParentArg,
    _name: &str,
    _path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emDirStatFpPlugin: No properties allowed.".to_string();
        return None;
    }
    Some(Box::new(emDirStatPanel::new(Rc::clone(
        parent.root_context(),
    ))))
}
