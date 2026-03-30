use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;
use std::cell::RefCell;
use std::rc::Rc;

/// Entry point for the directory panel plugin.
/// Loaded via `emDir.emFpPlugin` config file.
#[no_mangle]
pub fn emDirFpPluginFunc(
    _parent: &PanelParentArg,
    _name: &str,
    _path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Rc<RefCell<dyn PanelBehavior>>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emDirFpPlugin: No properties allowed.".to_string();
        return None;
    }
    // TODO: return new emDirPanel when panel integration is complete
    *error_buf = "emDirFpPlugin: not yet implemented".to_string();
    None
}
