//! Plugin entry point for .emStocks files.
//!
//! Port of C++ `emStocksFpPlugin.cpp`. Exports `emStocksFpPluginFunc`
//! which is resolved via dlsym when the plugin manager loads this library.

use emcore::emEngineCtx::ConstructCtx;
use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;

use crate::emStocksFilePanel::emStocksFilePanel;

/// Plugin entry point for .emStocks files.
/// Port of C++ `emStocksFpPluginFunc` in emStocksFpPlugin.cpp.
///
/// Called by the plugin manager when a .emStocks file needs to be displayed.
#[no_mangle]
pub fn emStocksFpPluginFunc(
    _ctx: &mut dyn ConstructCtx,
    _parent: &PanelParentArg,
    _name: &str,
    _path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>> {
    if !plugin.properties.is_empty() {
        *error_buf = "emStocksFpPlugin: No properties allowed.".to_string();
        return None;
    }

    Some(Box::new(emStocksFilePanel::new()))
}

/// The file extension this plugin handles.
pub const EMSTOCKS_EXTENSION: &str = "emStocks";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_constant() {
        assert_eq!(EMSTOCKS_EXTENSION, "emStocks");
    }

    #[test]
    fn plugin_func_rejects_properties() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let parent = PanelParentArg::new(ctx);
        let mut plugin = emFpPlugin::new();
        plugin
            .properties
            .push(emcore::emFpPlugin::FpPluginProperty {
                name: "bad".to_string(),
                value: "prop".to_string(),
            });
        let mut err = String::new();
        let mut h = emcore::test_view_harness::InitHarness::new();
        let mut ic = h.ctx();
        let result = emStocksFpPluginFunc(
            &mut ic,
            &parent,
            "test",
            "/tmp/test.emStocks",
            &plugin,
            &mut err,
        );
        assert!(result.is_none());
        assert!(err.contains("No properties allowed"));
    }

    #[test]
    fn plugin_func_creates_panel() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let parent = PanelParentArg::new(ctx);
        let plugin = emFpPlugin::new();
        let mut err = String::new();
        let mut h = emcore::test_view_harness::InitHarness::new();
        let mut ic = h.ctx();
        let result = emStocksFpPluginFunc(
            &mut ic,
            &parent,
            "test",
            "/tmp/test.emStocks",
            &plugin,
            &mut err,
        );
        assert!(result.is_some());
        assert!(err.is_empty());
    }
}
