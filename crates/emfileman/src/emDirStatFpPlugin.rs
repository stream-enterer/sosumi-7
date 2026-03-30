use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;
use std::cell::RefCell;
use std::rc::Rc;

/// Entry point for the directory statistics panel plugin.
/// Loaded via `emDirStat.emFpPlugin` config file.
#[no_mangle]
pub fn emDirStatFpPluginFunc(
    _parent: &PanelParentArg,
    _name: &str,
    _path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Rc<RefCell<dyn PanelBehavior>>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emDirStatFpPlugin: No properties allowed.".to_string();
        return None;
    }
    // TODO: create emDirStatPanel with emDirModel, updateFileModel=false
    *error_buf = "emDirStatFpPlugin: not yet implemented".to_string();
    None
}
