//! Test that .emFpPlugin config files load from etc/emCore/FpPlugins/.
//!
//! Requires EM_DIR to be set to the repo root (done by .cargo/config.toml).

use emcore::emContext::emContext;
use emcore::emFpPlugin::emFpPluginList;

#[test]
fn load_plugins_from_etc_directory() {
    // EM_DIR should be set to repo root by .cargo/config.toml
    let ctx = emContext::NewRoot();
    let list = emFpPluginList::Acquire(&ctx);
    let list = list.borrow();
    // Should find emStocks.emFpPlugin
    assert!(list.plugin_count() > 0, "no plugins loaded — check EM_DIR");
    let plugins = list.plugins();
    let emstocks = plugins.iter().find(|p| p.library == "emStocks");
    assert!(emstocks.is_some(), "emStocks plugin not found");
    let p = emstocks.expect("just checked");
    assert_eq!(p.function, "emStocksFpPluginFunc");
    assert_eq!(p.file_types, vec![".emStocks"]);
}
