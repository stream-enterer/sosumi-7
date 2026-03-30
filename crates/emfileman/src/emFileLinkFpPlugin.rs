use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;
use std::cell::RefCell;
use std::rc::Rc;

/// Entry point for the file link panel plugin.
/// Loaded via `emFileLink.emFpPlugin` config file.
#[no_mangle]
pub fn emFileLinkFpPluginFunc(
    _parent: &PanelParentArg,
    _name: &str,
    _path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Rc<RefCell<dyn PanelBehavior>>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emFileLinkFpPlugin: No properties allowed.".to_string();
        return None;
    }
    // TODO: create emFileLinkPanel with emFileLinkModel
    *error_buf = "emFileLinkFpPlugin: not yet implemented".to_string();
    None
}
