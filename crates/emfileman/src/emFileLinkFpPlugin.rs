use std::rc::Rc;

use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;

use crate::emFileLinkModel::emFileLinkModel;
use crate::emFileLinkPanel::emFileLinkPanel;

/// Entry point for the file link panel plugin.
/// Loaded via `emFileLink.emFpPlugin` config file.
#[no_mangle]
pub fn emFileLinkFpPluginFunc(
    parent: &PanelParentArg,
    _name: &str,
    path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emFileLinkFpPlugin: No properties allowed.".to_string();
        return None;
    }
    // C++: new emFileLinkPanel(parent, name,
    //        emFileLinkModel::Acquire(parent.GetRootContext(), path))
    let model = emFileLinkModel::Acquire(parent.root_context(), path, false);
    let mut panel = emFileLinkPanel::new(
        Rc::clone(parent.root_context()),
        true,
    );
    panel.set_link_model(model);
    Some(Box::new(panel))
}
