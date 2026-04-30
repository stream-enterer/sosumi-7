#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

mod emTestPanel;

use emcore::emEngineCtx::ConstructCtx;
use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;

#[no_mangle]
pub fn emTestPanelFpPluginFunc(
    ctx: &mut dyn ConstructCtx,
    _parent: &PanelParentArg,
    _name: &str,
    _path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emTestPanelFpPlugin: No properties allowed.".to_string();
        return None;
    }
    Some(emTestPanel::new_root_panel(ctx))
}
