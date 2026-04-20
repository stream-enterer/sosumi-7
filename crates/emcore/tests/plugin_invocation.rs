//! Behavioral tests for plugin invocation via dlopen.
//!
//! These tests require `test_plugin` to be built first:
//!   cargo build -p test_plugin
//!
//! They also require LD_LIBRARY_PATH to include target/debug/.

use emcore::emContext::emContext;
use emcore::emFpPlugin::{emFpPlugin, emFpPluginList, FileStatMode, FpPluginError, PanelParentArg};
use emcore::test_view_harness::InitHarness;

fn make_test_plugin() -> emFpPlugin {
    let mut p = emFpPlugin::new();
    p.file_types = vec![".test".to_string()];
    p.priority = 1.0;
    p.library = "test_plugin".to_string();
    p.function = "test_plugin_func".to_string();
    p
}

#[test]
fn try_create_file_panel_loads_plugin() {
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let plugin = make_test_plugin();
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp/test.test");
    assert!(
        result.is_ok(),
        "TryCreateFilePanel failed: {:?}",
        result.err()
    );
}

#[test]
fn try_create_file_panel_empty_function_errors() {
    let mut plugin = make_test_plugin();
    plugin.function = String::new();
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp/test.test");
    assert!(matches!(result, Err(FpPluginError::EmptyFunctionName)));
}

#[test]
fn try_create_file_panel_missing_library_errors() {
    let mut plugin = make_test_plugin();
    plugin.library = "nonexistent_library_xyz".to_string();
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp/test.test");
    assert!(matches!(result, Err(FpPluginError::LibraryLoad { .. })));
}

#[test]
fn try_create_file_panel_missing_symbol_errors() {
    let mut plugin = make_test_plugin();
    plugin.function = "nonexistent_function_xyz".to_string();
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let result = plugin.TryCreateFilePanel(&mut ic, &parent, "test", "/tmp/test.test");
    assert!(matches!(result, Err(FpPluginError::SymbolResolve { .. })));
}

#[test]
fn plugin_list_no_matching_plugin_returns_error_panel() {
    let list = emFpPluginList::from_plugins(vec![]);
    let ctx = emContext::NewRoot();
    let parent = PanelParentArg::new(ctx);
    let mut h = InitHarness::new();
    let mut ic = h.ctx();
    let _panel = list.CreateFilePanelWithStat(
        &mut ic,
        &parent,
        "test",
        "/tmp/data.unknown",
        None,
        FileStatMode::Regular,
        0,
    );
    // Returns an error panel — "This file type cannot be shown."
    // (we just verify it doesn't panic)
}
