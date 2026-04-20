use emcore::emEngineCtx::ConstructCtx;
use emcore::emErrorPanel::emErrorPanel;
use emcore::emFpPlugin::{emFpPlugin, PanelParentArg};
use emcore::emPanel::PanelBehavior;

/// Test plugin function that creates a simple error panel with a success message.
/// Used by behavioral tests to validate the full dlopen -> resolve -> call path.
#[no_mangle]
pub fn test_plugin_func(
    _ctx: &mut dyn ConstructCtx,
    _parent: &PanelParentArg,
    _name: &str,
    path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>> {
    // Check properties — if any property named "fail" exists, return error
    if plugin.GetProperty("fail").is_some() {
        *error_buf = "test_plugin: instructed to fail".to_string();
        return None;
    }

    // Return an error panel as a simple PanelBehavior implementor
    Some(Box::new(emErrorPanel::new(&format!(
        "test_plugin loaded: {path}"
    ))))
}
