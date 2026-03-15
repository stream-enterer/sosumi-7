use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::foundation::{get_config_dir_overloadable, RecError, RecStruct, RecValue};
use crate::model::context::Context;
use crate::model::record::Record;

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

// ── FpPlugin ────────────────────────────────────────────────────────

/// A file panel plugin record.
///
/// Port of C++ `emFpPlugin`. Stores metadata about a plugin that can create
/// panels (and optionally models) for files of certain types. The metadata
/// is loaded from `.emFpPlugin` configuration files.
#[derive(Debug)]
pub struct FpPlugin {
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
    #[cfg(unix)]
    cached_library: RefCell<Option<CachedLibrary>>,
}

#[cfg(unix)]
struct CachedLibrary {
    lib_name: String,
    _library: libloading::Library,
}

#[cfg(unix)]
impl std::fmt::Debug for CachedLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedLibrary")
            .field("lib_name", &self.lib_name)
            .finish()
    }
}

impl FpPlugin {
    /// Create a new FpPlugin with default values.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Clone for FpPlugin {
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
            #[cfg(unix)]
            cached_library: RefCell::new(None),
        }
    }
}

impl FpPlugin {
    /// Look up a plugin property by name.
    ///
    /// Port of C++ `emFpPlugin::GetProperty`. Returns `None` if not found.
    pub fn get_property(&self, name: &str) -> Option<&FpPluginProperty> {
        self.properties.iter().rev().find(|p| p.name == name)
    }

    /// Resolve and load the dynamic library for this plugin.
    ///
    /// Port of C++ `emFpPlugin::TryCreateFilePanel` library resolution.
    /// The actual function lookup and call would happen at the call site.
    ///
    /// # Errors
    ///
    /// Returns an error if the library cannot be loaded.
    #[cfg(unix)]
    pub fn try_load_library(&self) -> Result<(), FpPluginError> {
        if self.library.is_empty() {
            return Err(FpPluginError::EmptyLibraryName);
        }

        let mut cached = self.cached_library.borrow_mut();

        // If already cached with the same library name, nothing to do.
        if let Some(ref cl) = *cached {
            if cl.lib_name == self.library {
                return Ok(());
            }
        }

        // Load the library. The C++ code uses emTryResolveSymbol which calls
        // emTryOpenLib with the pure library name. We use libloading which
        // expects a full path or system-resolvable name.
        let lib = unsafe { libloading::Library::new(&self.library) }.map_err(|e| {
            FpPluginError::LibraryLoad {
                library: self.library.clone(),
                message: e.to_string(),
            }
        })?;

        *cached = Some(CachedLibrary {
            lib_name: self.library.clone(),
            _library: lib,
        });

        Ok(())
    }

    #[cfg(not(unix))]
    pub fn try_load_library(&self) -> Result<(), FpPluginError> {
        Err(FpPluginError::UnsupportedPlatform)
    }

    /// Check if this plugin matches the given criteria.
    ///
    /// Port of C++ `emFpPluginList::IsMatchingPlugin`.
    pub fn is_matching(
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

impl Record for FpPlugin {
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
            #[cfg(unix)]
            cached_library: RefCell::new(None),
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
        s.set_value("FileTypes", RecValue::Array(ft_vals));

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
        s.set_value("ModelClasses", RecValue::Array(mc_vals));

        s.set_bool("ModelAbleToSave", self.model_able_to_save);

        let prop_vals: Vec<RecValue> = self
            .properties
            .iter()
            .map(|p| RecValue::Struct(p.to_rec()))
            .collect();
        s.set_value("Properties", RecValue::Array(prop_vals));

        s
    }

    fn set_to_default(&mut self) {
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

    fn is_default(&self) -> bool {
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

impl Default for FpPlugin {
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
            #[cfg(unix)]
            cached_library: RefCell::new(None),
        }
    }
}

// ── FpPluginError ───────────────────────────────────────────────────

/// Errors from FpPlugin operations.
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

// ── FpPluginList ────────────────────────────────────────────────────

/// A registry of file panel plugins, loaded from `.emFpPlugin` config files.
///
/// Port of C++ `emFpPluginList`. Registered as a common model in the root
/// context via `FpPluginList::acquire()`. Plugins are sorted by descending
/// priority.
pub struct FpPluginList {
    plugins: Vec<FpPlugin>,
}

impl FpPluginList {
    /// Acquire the plugin list from the root context (create if absent).
    ///
    /// Port of C++ `emFpPluginList::Acquire(rootContext)`.
    pub fn acquire(root_context: &Rc<Context>) -> Rc<RefCell<Self>> {
        root_context.acquire::<Self>("", Self::load_plugins)
    }

    /// Load plugins from the configuration directory.
    ///
    /// Port of the C++ `emFpPluginList` constructor.
    fn load_plugins() -> Self {
        let dir_path = match get_config_dir_overloadable("emCore", Some("FpPlugins")) {
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

    /// Create an `FpPluginList` from an explicit list of plugins (for testing).
    ///
    /// Plugins are sorted by descending priority.
    pub fn from_plugins(mut plugins: Vec<FpPlugin>) -> Self {
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
    pub fn plugins(&self) -> &[FpPlugin] {
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
    pub fn search_plugin(
        &self,
        model_class_name: Option<&str>,
        file_path: Option<&str>,
        require_able_to_save: bool,
        alternative: usize,
        stat_mode: FileStatMode,
    ) -> Option<&FpPlugin> {
        let file_name = file_path.map(extract_file_name);

        let mut skip = alternative;
        for plugin in &self.plugins {
            if plugin.is_matching(model_class_name, file_name, require_able_to_save, stat_mode) {
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
    pub fn search_plugins(
        &self,
        model_class_name: Option<&str>,
        file_path: Option<&str>,
        require_able_to_save: bool,
        stat_mode: FileStatMode,
    ) -> Vec<&FpPlugin> {
        let file_name = file_path.map(extract_file_name);

        self.plugins
            .iter()
            .filter(|plugin| {
                plugin.is_matching(model_class_name, file_name, require_able_to_save, stat_mode)
            })
            .collect()
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
fn load_plugins_from_dir(dir_path: &PathBuf) -> Vec<FpPlugin> {
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

/// Load a single FpPlugin from an emRec config file.
fn load_plugin_from_file(path: &Path) -> Result<FpPlugin, RecError> {
    use crate::model::rec_types::RecFileReader;

    let rec = RecFileReader::read_with_format(path, "emFpPlugin")?;
    FpPlugin::from_rec(&rec)
}
