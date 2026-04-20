//! Verify all production plugins load via dlopen/dlsym.
//!
//! These tests do NOT use the static plugin resolver — they exercise
//! the dynamic loading path in TryCreateFilePanel exclusively.
//!
//! Requires:
//!   - EM_DIR set to repo root (for plugin config discovery)
//!   - LD_LIBRARY_PATH including target/debug/ (for dlopen)
//!   - cargo build (to produce .so files)

use emcore::emContext::emContext;
use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::test_view_harness::InitHarness;

/// Helper: create a plugin pointing at a specific library/function.
fn plugin_for(library: &str, function: &str) -> emFpPlugin {
    let mut p = emFpPlugin::new();
    p.library = library.to_string();
    p.function = function.to_string();
    p
}

#[test]
fn dynamic_load_dir_panel() {
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let plugin = plugin_for("emFileMan", "emDirFpPluginFunc");
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp");
    assert!(
        result.is_ok(),
        "emDirFpPluginFunc failed: {:?}",
        result.err()
    );
}

#[test]
fn dynamic_load_dir_stat_panel() {
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let plugin = plugin_for("emFileMan", "emDirStatFpPluginFunc");
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp");
    assert!(
        result.is_ok(),
        "emDirStatFpPluginFunc failed: {:?}",
        result.err()
    );
}

#[test]
fn dynamic_load_file_link_panel() {
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let plugin = plugin_for("emFileMan", "emFileLinkFpPluginFunc");
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    // emFileLink panels expect an .emFileLink file; missing file returns error panel.
    // What matters is that the symbol resolved — not that the panel content is valid.
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp/nonexistent.emFileLink");
    assert!(
        result.is_ok(),
        "emFileLinkFpPluginFunc failed: {:?}",
        result.err()
    );
}

#[test]
fn dynamic_load_stocks_panel() {
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let plugin = plugin_for("emStocks", "emStocksFpPluginFunc");
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp/test.emStocks");
    assert!(
        result.is_ok(),
        "emStocksFpPluginFunc failed: {:?}",
        result.err()
    );
}
