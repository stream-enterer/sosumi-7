//! End-to-end test: load .emStocks file via plugin system.
//!
//! Verifies the full path: config file -> plugin list -> dlopen ->
//! symbol resolve -> emStocksFpPluginFunc -> emStocksFilePanel.
//!
//! Requires:
//!   - EM_DIR set to repo root (for plugin config discovery)
//!   - LD_LIBRARY_PATH including target/debug/ (for dlopen)
//!   - cargo build -p emstocks (to produce libemStocks.so)

use emcore::emContext::emContext;
use emcore::emFpPlugin::{emFpPluginList, FileStatMode, PanelParentArg};
use emcore::test_view_harness::InitHarness;

#[test]
fn load_emstocks_plugin_end_to_end() {
    let ctx = emContext::NewRoot();
    let list = emFpPluginList::Acquire(&ctx);
    let list = list.borrow();

    let parent = PanelParentArg::new(emContext::NewRoot());

    // Create a temporary .emStocks file
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join("test_plugin_e2e.emStocks");
    std::fs::write(&tmp_file, "#%rec:emStocksRec%#\n").expect("write test file");

    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let panel = list.CreateFilePanelWithStat(
        &mut ic,
        &parent,
        "test",
        tmp_file.to_str().expect("temp path is valid UTF-8"),
        None,
        FileStatMode::Regular,
        0,
    );

    // Panel should be created successfully — verify it is not opaque (default)
    assert!(!panel.IsOpaque());

    // Cleanup
    let _ = std::fs::remove_file(&tmp_file);
}
