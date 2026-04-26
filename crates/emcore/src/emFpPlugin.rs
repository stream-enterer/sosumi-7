use std::any::Any;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::emContext::emContext;
use crate::emEngineCtx::ConstructCtx;
use crate::emInstallInfo::emGetConfigDirOverloadable;
use crate::emPanel::PanelBehavior;
use crate::emPanelTree::PanelId;
use crate::emRecParser::{RecError, RecStruct, RecValue};
use crate::emRecRecord::Record;

// ── Plugin function types ───────────────────────────────────────────
// Port of C++ emFpPluginFunc and emFpPluginModelFunc from emFpPlugin.h.
// Uses Rust calling convention with #[no_mangle] for symbol lookup.
// Types cross the dylib boundary safely because host and plugins link
// the same libemcore.so.

/// Type of the plugin function for creating a file panel.
/// Port of C++ `emFpPluginFunc`.
/// DIVERGED: (language-forced) C++ returns emPanel* (raw pointer to Rc-managed panel). Rust returns
/// Box<dyn PanelBehavior> — ownership transfers to caller who installs it in the
/// panel tree via set_behavior.
/// DIVERGED: (language-forced) added `ctx: &mut dyn ConstructCtx` first parameter — Phase-3 Task 5
/// (I3d). C++ constructs widgets under the implicit scheduler singleton; the
/// Rust ownership rewrite (spec §4 D4.9 / §6 D6.1) threads the
/// scheduler/signal/engine surface through ConstructCtx so plugin-created
/// widgets can allocate SignalIds at construction. Current plugin
/// implementations prefix the arg `_ctx` because existing file-panel
/// constructors allocate signals lazily at Input time; Phase-4+ widget
/// migrations will drop the underscore as widgets adopt construction-time
/// signal allocation per spec §6 D6.1.
pub type emFpPluginFunc = fn(
    ctx: &mut dyn ConstructCtx,
    parent: &PanelParentArg,
    name: &str,
    path: &str,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Box<dyn PanelBehavior>>;

/// Type of the plugin model function for acquiring file models.
/// Port of C++ `emFpPluginModelFunc`.
/// DIVERGED: (language-forced) added `ctx: &mut dyn ConstructCtx` first parameter — see
/// `emFpPluginFunc` above for rationale (Phase-3 Task 5 / I3d). Current
/// model-function implementations prefix the arg `_ctx` for the same
/// reason: model constructors do not yet allocate signals at construction
/// time; Phase-4+ migrations will drop the underscore as they adopt
/// construction-time signal allocation per spec §6 D6.1.
pub type emFpPluginModelFunc = fn(
    ctx: &mut dyn ConstructCtx,
    context: &Rc<emContext>,
    class_name: &str,
    name: &str,
    common: bool,
    plugin: &emFpPlugin,
    error_buf: &mut String,
) -> Option<Rc<RefCell<dyn Any>>>;

/// Parent argument for panel creation.
/// DIVERGED: (language-forced) C++ emPanel::ParentArg carries full parent panel reference with
/// layout constraint forwarding. This version carries parent panel ID for tree
/// integration but does not forward layout constraints. Full constraint
/// forwarding deferred to panel framework completion.
pub struct PanelParentArg {
    root_context: Rc<emContext>,
    parent_panel: Option<PanelId>,
}

impl PanelParentArg {
    pub fn new(root_context: Rc<emContext>) -> Self {
        Self {
            root_context,
            parent_panel: None,
        }
    }

    pub fn with_parent(root_context: Rc<emContext>, parent: PanelId) -> Self {
        Self {
            root_context,
            parent_panel: Some(parent),
        }
    }

    pub fn root_context(&self) -> &Rc<emContext> {
        &self.root_context
    }

    pub fn parent_panel(&self) -> Option<PanelId> {
        self.parent_panel
    }
}

// ── FpPluginProperty ────────────────────────────────────────────────

/// A name/value property pair attached to a plugin.
///
/// Port of C++ `emFpPlugin::PropertyRec`.
#[derive(Clone, Debug, PartialEq)]
pub struct FpPluginProperty {
    pub name: String,
    pub value: String,
}

impl FpPluginProperty {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let name = rec.get_str("name").unwrap_or("").to_string();
        let value = rec.get_str("value").unwrap_or("").to_string();
        Ok(Self { name, value })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_str("Name", &self.name);
        s.set_str("Value", &self.value);
        s
    }
}

// ── FileStatMode ────────────────────────────────────────────────────

/// Simplified file stat mode for plugin matching.
///
/// Port of the `S_IFREG` / `S_IFDIR` distinction used in C++ `emFpPluginList`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatMode {
    Regular,
    Directory,
}

// ── emFpPlugin ────────────────────────────────────────────────────────

/// A file panel plugin record.
///
/// Port of C++ `emFpPlugin`. Stores metadata about a plugin that can create
/// panels (and optionally models) for files of certain types. The metadata
/// is loaded from `.emFpPlugin` configuration files.
#[derive(Debug)]
pub struct emFpPlugin {
    /// File types this plugin handles (e.g. `".png"`, `"file"`, `"directory"`).
    pub file_types: Vec<String>,
    /// Human-readable name for the file format.
    pub file_format_name: String,
    /// Priority (higher = preferred when multiple plugins match).
    pub priority: f64,
    /// Dynamic library name (pure name, resolved via platform conventions).
    pub library: String,
    /// Name of the panel-creation function in the library.
    pub function: String,
    /// Name of the model-acquisition function (empty if none).
    pub model_function: String,
    /// Model class names supported by the model function.
    pub model_classes: Vec<String>,
    /// Whether the model supports saving.
    pub model_able_to_save: bool,
    /// Plugin-defined name/value properties.
    pub properties: Vec<FpPluginProperty>,

    // Cached resolved function pointers are not serialized and are managed
    // by the dynamic loading layer at runtime.
    cached: RefCell<CachedFunctions>,
}

/// Cached resolved function pointers. Port of C++ CachedFunc/CachedModelFunc
/// fields on emFpPlugin.
#[derive(Default)]
struct CachedFunctions {
    lib_name: String,
    func_name: String,
    func: Option<emFpPluginFunc>,
    model_func_name: String,
    model_func: Option<emFpPluginModelFunc>,
}

// Default is derived — all fields are String::new() / None.

impl std::fmt::Debug for CachedFunctions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedFunctions")
            .field("lib_name", &self.lib_name)
            .finish()
    }
}

impl emFpPlugin {
    /// Create a new emFpPlugin with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Test-support constructor: create a plugin that handles "directory" file types
    /// and invokes a statically-linked function directly (no dlopen).
    ///
    /// Pre-populates the function cache so `TryCreateFilePanel` never tries to
    /// resolve a dynamic library. Use this in tests to inject emDirFpPlugin without
    /// a .emFpPlugin config file on disk.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test_directory_handler(function_name: &str, func: emFpPluginFunc) -> Self {
        let p = Self {
            file_types: vec!["directory".to_string()],
            function: function_name.to_string(),
            library: "__test__".to_string(),
            ..Self::default()
        };
        *p.cached.borrow_mut() = CachedFunctions {
            lib_name: "__test__".to_string(),
            func_name: function_name.to_string(),
            func: Some(func),
            model_func_name: String::new(),
            model_func: None,
        };
        p
    }
}

impl Clone for emFpPlugin {
    fn clone(&self) -> Self {
        Self {
            file_types: self.file_types.clone(),
            file_format_name: self.file_format_name.clone(),
            priority: self.priority,
            library: self.library.clone(),
            function: self.function.clone(),
            model_function: self.model_function.clone(),
            model_classes: self.model_classes.clone(),
            model_able_to_save: self.model_able_to_save,
            properties: self.properties.clone(),
            cached: RefCell::new(CachedFunctions::default()),
        }
    }
}

impl emFpPlugin {
    /// emLook up a plugin property by name.
    ///
    /// Port of C++ `emFpPlugin::GetProperty`. Returns `None` if not found.
    pub fn GetProperty(&self, name: &str) -> Option<&FpPluginProperty> {
        self.properties.iter().rev().find(|p| p.name == name)
    }

    /// Create a file panel via this plugin's function.
    /// Port of C++ `emFpPlugin::TryCreateFilePanel`.
    pub fn TryCreateFilePanel(
        &self,
        ctx: &mut dyn ConstructCtx,
        parent: &PanelParentArg,
        name: &str,
        path: &str,
    ) -> Result<Box<dyn PanelBehavior>, FpPluginError> {
        use crate::emStd2::{emTryResolveSymbol, LibError};

        let mut cached = self.cached.borrow_mut();

        // Invalidate cache if library changed (matches C++ CachedLibName check)
        if cached.lib_name != self.library {
            *cached = CachedFunctions::default();
            cached.lib_name = self.library.clone();
        }

        // Resolve function if not cached or function name changed
        if cached.func.is_none() || cached.func_name != self.function {
            if self.function.is_empty() {
                return Err(FpPluginError::EmptyFunctionName);
            }

            let ptr = unsafe { emTryResolveSymbol(&self.library, false, &self.function) }.map_err(
                |e| match e {
                    LibError::LibraryLoad { library, message } => {
                        FpPluginError::LibraryLoad { library, message }
                    }
                    LibError::SymbolResolve {
                        library,
                        symbol,
                        message,
                    } => FpPluginError::SymbolResolve {
                        library,
                        symbol,
                        message,
                    },
                },
            )?;

            cached.func = Some(unsafe { std::mem::transmute::<*const (), emFpPluginFunc>(ptr) });
            cached.func_name = self.function.clone();
        }

        let func = cached.func.expect("func was just set");
        drop(cached); // release borrow before calling plugin function

        let mut error_buf = String::new();
        match func(ctx, parent, name, path, self, &mut error_buf) {
            Some(panel) => Ok(panel),
            None => Err(FpPluginError::PluginFunctionFailed {
                function: self.function.clone(),
                message: if error_buf.is_empty() {
                    format!(
                        "Plugin function {} in {} failed.",
                        self.function, self.library
                    )
                } else {
                    error_buf
                },
            }),
        }
    }

    /// Acquire a model via this plugin's model function.
    /// Port of C++ `emFpPlugin::TryAcquireModelImpl`.
    pub fn TryAcquireModel(
        &self,
        ctx: &mut dyn ConstructCtx,
        context: &Rc<emContext>,
        class_name: &str,
        name: &str,
        common: bool,
    ) -> Result<Rc<RefCell<dyn Any>>, FpPluginError> {
        use crate::emStd2::{emTryResolveSymbol, LibError};

        let mut cached = self.cached.borrow_mut();

        if cached.lib_name != self.library {
            *cached = CachedFunctions::default();
            cached.lib_name = self.library.clone();
        }

        if cached.model_func.is_none() || cached.model_func_name != self.model_function {
            if self.model_function.is_empty() {
                return Err(FpPluginError::EmptyFunctionName);
            }

            let ptr = unsafe { emTryResolveSymbol(&self.library, false, &self.model_function) }
                .map_err(|e| match e {
                    LibError::LibraryLoad { library, message } => {
                        FpPluginError::LibraryLoad { library, message }
                    }
                    LibError::SymbolResolve {
                        library,
                        symbol,
                        message,
                    } => FpPluginError::SymbolResolve {
                        library,
                        symbol,
                        message,
                    },
                })?;

            cached.model_func =
                Some(unsafe { std::mem::transmute::<*const (), emFpPluginModelFunc>(ptr) });
            cached.model_func_name = self.model_function.clone();
        }

        let func = cached.model_func.expect("model_func was just set");
        drop(cached);

        let mut error_buf = String::new();
        match func(ctx, context, class_name, name, common, self, &mut error_buf) {
            Some(model) => Ok(model),
            None => Err(FpPluginError::PluginFunctionFailed {
                function: self.model_function.clone(),
                message: if error_buf.is_empty() {
                    format!(
                        "Plugin model function {} in {} failed.",
                        self.model_function, self.library
                    )
                } else {
                    error_buf
                },
            }),
        }
    }

    /// Check if this plugin matches the given criteria.
    ///
    /// Port of C++ `emFpPluginList::IsMatchingPlugin`.
    pub fn IsMatchingPlugin(
        &self,
        model_class_name: Option<&str>,
        file_name: Option<&str>,
        require_able_to_save: bool,
        stat_mode: FileStatMode,
    ) -> bool {
        // Check model class name filter.
        if let Some(class_name) = model_class_name {
            if !self.model_classes.iter().any(|c| c == class_name) {
                return false;
            }
        }

        // Check file type filter.
        if let Some(file_name) = file_name {
            let file_name_len = file_name.len();
            let mut matched = false;
            for ft in &self.file_types {
                if ft.starts_with('.') {
                    // Extension match — only for regular files.
                    if stat_mode == FileStatMode::Regular {
                        let type_len = ft.len();
                        if type_len < file_name_len
                            && file_name[file_name_len - type_len..].eq_ignore_ascii_case(ft)
                        {
                            matched = true;
                            break;
                        }
                    }
                } else if (ft == "file" && stat_mode == FileStatMode::Regular)
                    || (ft == "directory" && stat_mode == FileStatMode::Directory)
                {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }

        // Check able-to-save filter.
        if require_able_to_save && !self.model_able_to_save {
            return false;
        }

        true
    }
}

impl Record for emFpPlugin {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        // FileTypes — array of strings.
        let file_types = match rec.get_array("filetypes") {
            Some(arr) => arr
                .iter()
                .filter_map(|v| match v {
                    RecValue::Str(s) => Some(s.clone()),
                    RecValue::Ident(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            None => Vec::new(),
        };

        let file_format_name = rec.get_str("fileformatname").unwrap_or("").to_string();

        let priority = rec.get_double("priority").unwrap_or(1.0);

        let library = rec.get_str("library").unwrap_or("unknown").to_string();

        let function = rec.get_str("function").unwrap_or("unknown").to_string();

        let model_function = rec.get_str("modelfunction").unwrap_or("").to_string();

        let model_classes = match rec.get_array("modelclasses") {
            Some(arr) => arr
                .iter()
                .filter_map(|v| match v {
                    RecValue::Str(s) => Some(s.clone()),
                    RecValue::Ident(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            None => Vec::new(),
        };

        let model_able_to_save = rec.get_bool("modelabletosave").unwrap_or(false);

        let properties = match rec.get_array("properties") {
            Some(arr) => arr
                .iter()
                .filter_map(|v| match v {
                    RecValue::Struct(s) => FpPluginProperty::from_rec(s).ok(),
                    _ => None,
                })
                .collect(),
            None => Vec::new(),
        };

        Ok(Self {
            file_types,
            file_format_name,
            priority,
            library,
            function,
            model_function,
            model_classes,
            model_able_to_save,
            properties,
            cached: RefCell::new(CachedFunctions::default()),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();

        // FileTypes
        let ft_vals: Vec<RecValue> = self
            .file_types
            .iter()
            .map(|t| RecValue::Str(t.clone()))
            .collect();
        s.SetValue("FileTypes", RecValue::Array(ft_vals));

        s.set_str("FileFormatName", &self.file_format_name);
        s.set_double("Priority", self.priority);
        s.set_str("Library", &self.library);
        s.set_str("Function", &self.function);
        s.set_str("ModelFunction", &self.model_function);

        let mc_vals: Vec<RecValue> = self
            .model_classes
            .iter()
            .map(|c| RecValue::Str(c.clone()))
            .collect();
        s.SetValue("ModelClasses", RecValue::Array(mc_vals));

        s.set_bool("ModelAbleToSave", self.model_able_to_save);

        let prop_vals: Vec<RecValue> = self
            .properties
            .iter()
            .map(|p| RecValue::Struct(p.to_rec()))
            .collect();
        s.SetValue("Properties", RecValue::Array(prop_vals));

        s
    }

    fn SetToDefault(&mut self) {
        self.file_types.clear();
        self.file_format_name.clear();
        self.priority = 1.0;
        self.library = "unknown".to_string();
        self.function = "unknown".to_string();
        self.model_function.clear();
        self.model_classes.clear();
        self.model_able_to_save = false;
        self.properties.clear();
    }

    fn IsSetToDefault(&self) -> bool {
        self.file_types.is_empty()
            && self.file_format_name.is_empty()
            && self.priority == 1.0
            && self.library == "unknown"
            && self.function == "unknown"
            && self.model_function.is_empty()
            && self.model_classes.is_empty()
            && !self.model_able_to_save
            && self.properties.is_empty()
    }
}

impl Default for emFpPlugin {
    fn default() -> Self {
        Self {
            file_types: Vec::new(),
            file_format_name: String::new(),
            priority: 1.0,
            library: "unknown".to_string(),
            function: "unknown".to_string(),
            model_function: String::new(),
            model_classes: Vec::new(),
            model_able_to_save: false,
            properties: Vec::new(),
            cached: RefCell::new(CachedFunctions::default()),
        }
    }
}

// ── FpPluginError ───────────────────────────────────────────────────

/// Errors from emFpPlugin operations.
#[derive(Debug)]
pub enum FpPluginError {
    /// The library name field is empty.
    EmptyLibraryName,
    /// The function name field is empty.
    EmptyFunctionName,
    /// Failed to load a dynamic library.
    LibraryLoad { library: String, message: String },
    /// Failed to resolve a symbol in a library.
    SymbolResolve {
        library: String,
        symbol: String,
        message: String,
    },
    /// The plugin function returned null/error.
    PluginFunctionFailed { function: String, message: String },
    /// No suitable plugin found.
    NoPluginFound,
    /// Platform not supported for dynamic loading.
    UnsupportedPlatform,
    /// Error loading plugin config files.
    ConfigLoad(String),
}

impl std::fmt::Display for FpPluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyLibraryName => write!(f, "plugin library name is empty"),
            Self::EmptyFunctionName => write!(f, "plugin function name is empty"),
            Self::LibraryLoad { library, message } => {
                write!(f, "failed to load library {library}: {message}")
            }
            Self::SymbolResolve {
                library,
                symbol,
                message,
            } => write!(
                f,
                "failed to resolve symbol {symbol} in {library}: {message}"
            ),
            Self::PluginFunctionFailed { function, message } => {
                write!(f, "plugin function {function} failed: {message}")
            }
            Self::NoPluginFound => write!(f, "no suitable plugin found"),
            Self::UnsupportedPlatform => {
                write!(f, "dynamic loading not supported on this platform")
            }
            Self::ConfigLoad(msg) => write!(f, "plugin config load error: {msg}"),
        }
    }
}

impl std::error::Error for FpPluginError {}

// ── emFpPluginList ────────────────────────────────────────────────────

/// A registry of file panel plugins, loaded from `.emFpPlugin` config files.
///
/// Port of C++ `emFpPluginList`. Registered as a common model in the root
/// context via `emFpPluginList::acquire()`. Plugins are sorted by descending
/// priority.
pub struct emFpPluginList {
    plugins: Vec<emFpPlugin>,
}

impl emFpPluginList {
    /// Acquire the plugin list from the root context (create if absent).
    ///
    /// Port of C++ `emFpPluginList::Acquire(rootContext)`.
    pub fn Acquire(root_context: &Rc<emContext>) -> Rc<RefCell<Self>> {
        root_context.acquire::<Self>("", Self::load_plugins)
    }

    /// Load plugins from the configuration directory.
    ///
    /// Port of the C++ `emFpPluginList` constructor.
    fn load_plugins() -> Self {
        let dir_path = match emGetConfigDirOverloadable("emCore", Some("FpPlugins")) {
            Ok(p) => p,
            Err(e) => {
                log::error!("FpPluginList: cannot resolve plugin directory: {e}");
                return Self {
                    plugins: Vec::new(),
                };
            }
        };

        let mut plugins = load_plugins_from_dir(&dir_path);

        // Sort by descending priority (stable sort preserves file-name order
        // for equal priorities, matching C++ behavior since files are read
        // in sorted order).
        plugins.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Self { plugins }
    }

    /// Create an `emFpPluginList` from an explicit list of plugins (for testing).
    ///
    /// Plugins are sorted by descending priority.
    pub fn from_plugins(mut plugins: Vec<emFpPlugin>) -> Self {
        plugins.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Self { plugins }
    }

    /// Return the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Return a slice of all plugins (sorted by descending priority).
    pub fn plugins(&self) -> &[emFpPlugin] {
        &self.plugins
    }

    /// Search for the Nth matching plugin.
    ///
    /// Port of C++ `emFpPluginList::SearchPlugin`.
    ///
    /// # Arguments
    /// - `model_class_name` — if `Some`, only plugins whose `model_classes`
    ///   contains this name are considered.
    /// - `file_path` — if `Some`, only plugins whose `file_types` matches the
    ///   file extension are considered. The file name is extracted from the path.
    /// - `require_able_to_save` — if `true`, only plugins with
    ///   `model_able_to_save == true` are considered.
    /// - `alternative` — selects the Nth match (0 = highest priority).
    /// - `stat_mode` — whether the file is a regular file or directory.
    pub fn SearchPlugin(
        &self,
        model_class_name: Option<&str>,
        file_path: Option<&str>,
        require_able_to_save: bool,
        alternative: usize,
        stat_mode: FileStatMode,
    ) -> Option<&emFpPlugin> {
        let file_name = file_path.map(extract_file_name);

        let mut skip = alternative;
        for plugin in &self.plugins {
            if plugin.IsMatchingPlugin(model_class_name, file_name, require_able_to_save, stat_mode)
            {
                if skip == 0 {
                    return Some(plugin);
                }
                skip -= 1;
            }
        }

        None
    }

    /// Search for all matching plugins, sorted by descending priority.
    ///
    /// Port of C++ `emFpPluginList::SearchPlugins`.
    pub fn SearchPlugins(
        &self,
        model_class_name: Option<&str>,
        file_path: Option<&str>,
        require_able_to_save: bool,
        stat_mode: FileStatMode,
    ) -> Vec<&emFpPlugin> {
        let file_name = file_path.map(extract_file_name);

        self.plugins
            .iter()
            .filter(|plugin| {
                plugin.IsMatchingPlugin(
                    model_class_name,
                    file_name,
                    require_able_to_save,
                    stat_mode,
                )
            })
            .collect()
    }

    /// Create a panel for a file. Port of C++ `emFpPluginList::CreateFilePanel`.
    ///
    /// Calls the appropriate plugin. On failure, returns an emErrorPanel.
    pub fn CreateFilePanel(
        &self,
        ctx: &mut dyn ConstructCtx,
        parent: &PanelParentArg,
        name: &str,
        path: &str,
        alternative: usize,
    ) -> Box<dyn PanelBehavior> {
        let abs_path = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                return Box::new(crate::emErrorPanel::emErrorPanel::new(&e.to_string()));
            }
        };

        let metadata = std::fs::metadata(&abs_path);

        match metadata {
            Err(e) => Box::new(crate::emErrorPanel::emErrorPanel::new(&e.to_string())),
            Ok(meta) => {
                let stat_mode = if meta.is_dir() {
                    FileStatMode::Directory
                } else {
                    FileStatMode::Regular
                };
                self.CreateFilePanelWithStat(
                    ctx,
                    parent,
                    name,
                    &abs_path,
                    None,
                    stat_mode,
                    alternative,
                )
            }
        }
    }

    /// Create a panel with pre-computed stat information.
    /// Port of C++ `emFpPluginList::CreateFilePanel` (stat overload).
    #[allow(clippy::too_many_arguments)]
    pub fn CreateFilePanelWithStat(
        &self,
        ctx: &mut dyn ConstructCtx,
        parent: &PanelParentArg,
        name: &str,
        absolute_path: &str,
        stat_err: Option<&std::io::Error>,
        stat_mode: FileStatMode,
        alternative: usize,
    ) -> Box<dyn PanelBehavior> {
        if let Some(err) = stat_err {
            return Box::new(crate::emErrorPanel::emErrorPanel::new(&err.to_string()));
        }

        let plugin = self.SearchPlugin(None, Some(absolute_path), false, alternative, stat_mode);
        match plugin {
            None => {
                let msg = if alternative == 0 {
                    "This file type cannot be shown."
                } else {
                    "No alternative file panel plugin available."
                };
                Box::new(crate::emErrorPanel::emErrorPanel::new(msg))
            }
            Some(plugin) => match plugin.TryCreateFilePanel(ctx, parent, name, absolute_path) {
                Ok(panel) => panel,
                Err(e) => Box::new(crate::emErrorPanel::emErrorPanel::new(&e.to_string())),
            },
        }
    }

    /// Acquire a model via the best matching plugin.
    /// Port of C++ `emFpPluginList::TryAcquireModel`.
    #[allow(clippy::too_many_arguments)]
    pub fn TryAcquireModelFromPlugin(
        &self,
        ctx: &mut dyn ConstructCtx,
        context: &Rc<emContext>,
        class_name: &str,
        name: &str,
        name_is_file_path: bool,
        common: bool,
        alternative: usize,
        stat_mode: FileStatMode,
    ) -> Result<Rc<RefCell<dyn Any>>, FpPluginError> {
        let file_path = if name_is_file_path { Some(name) } else { None };
        let plugin = self.SearchPlugin(Some(class_name), file_path, false, alternative, stat_mode);

        match plugin {
            None => Err(FpPluginError::NoPluginFound),
            Some(plugin) => plugin.TryAcquireModel(ctx, context, class_name, name, common),
        }
    }
}

/// Extract the file name (last component) from a path string.
///
/// Port of C++ `emGetNameInPath`.
fn extract_file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// Load all `.emFpPlugin` files from a directory.
fn load_plugins_from_dir(dir_path: &PathBuf) -> Vec<emFpPlugin> {
    let mut plugins = Vec::new();

    let entries = match std::fs::read_dir(dir_path) {
        Ok(entries) => entries,
        Err(e) => {
            log::error!(
                "FpPluginList: cannot read plugin directory {}: {e}",
                dir_path.display()
            );
            return plugins;
        }
    };

    // Collect and sort entries by file name (matches C++ `dirList.Sort`).
    let mut file_paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    file_paths.sort();

    for path in file_paths {
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("emFpPlugin") {
            continue;
        }

        match load_plugin_from_file(&path) {
            Ok(plugin) => plugins.push(plugin),
            Err(e) => {
                log::error!(
                    "FpPluginList: failed to load plugin {}: {e}",
                    path.display()
                );
            }
        }
    }

    plugins
}

/// Load a single emFpPlugin from an emRec config file.
fn load_plugin_from_file(path: &Path) -> Result<emFpPlugin, RecError> {
    use crate::emRecRecTypes::emRecFileReader;

    let rec = emRecFileReader::read_with_format(path, "emFpPlugin")?;
    emFpPlugin::from_rec(&rec)
}
