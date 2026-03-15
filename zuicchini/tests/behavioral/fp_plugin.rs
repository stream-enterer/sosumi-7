use zuicchini::foundation::RecStruct;
use zuicchini::model::{FileStatMode, FpPlugin, FpPluginList, FpPluginProperty, Record};

// ── Helper ──────────────────────────────────────────────────────────

fn make_plugin(file_types: &[&str], priority: f64, library: &str, function: &str) -> FpPlugin {
    let mut p = FpPlugin::new();
    p.file_types = file_types.iter().map(|s| s.to_string()).collect();
    p.priority = priority;
    p.library = library.to_string();
    p.function = function.to_string();
    p
}

fn make_plugin_full(
    file_types: &[&str],
    priority: f64,
    library: &str,
    function: &str,
    model_classes: &[&str],
    model_able_to_save: bool,
) -> FpPlugin {
    let mut p = make_plugin(file_types, priority, library, function);
    p.model_function = "model_fn".to_string();
    p.model_classes = model_classes.iter().map(|s| s.to_string()).collect();
    p.model_able_to_save = model_able_to_save;
    p
}

// ── Record serialization round-trip ─────────────────────────────────

#[test]
fn record_round_trip_default() {
    let plugin = FpPlugin::default();
    let rec = plugin.to_rec();
    let restored = FpPlugin::from_rec(&rec).unwrap();

    assert_eq!(restored.file_types, plugin.file_types);
    assert_eq!(restored.file_format_name, plugin.file_format_name);
    assert_eq!(restored.priority, plugin.priority);
    assert_eq!(restored.library, plugin.library);
    assert_eq!(restored.function, plugin.function);
    assert_eq!(restored.model_function, plugin.model_function);
    assert_eq!(restored.model_classes, plugin.model_classes);
    assert_eq!(restored.model_able_to_save, plugin.model_able_to_save);
    assert_eq!(restored.properties, plugin.properties);
}

#[test]
fn record_round_trip_with_data() {
    let mut plugin = FpPlugin::new();
    plugin.file_types = vec![".png".to_string(), ".jpg".to_string()];
    plugin.file_format_name = "Image File".to_string();
    plugin.priority = 5.0;
    plugin.library = "libImageViewer".to_string();
    plugin.function = "CreateImagePanel".to_string();
    plugin.model_function = "AcquireImageModel".to_string();
    plugin.model_classes = vec!["emImageFileModel".to_string()];
    plugin.model_able_to_save = true;
    plugin.properties = vec![
        FpPluginProperty {
            name: "MaxWidth".to_string(),
            value: "4096".to_string(),
        },
        FpPluginProperty {
            name: "Format".to_string(),
            value: "RGBA".to_string(),
        },
    ];
    let plugin = plugin;

    let rec = plugin.to_rec();
    let restored = FpPlugin::from_rec(&rec).unwrap();

    assert_eq!(restored.file_types, vec![".png", ".jpg"]);
    assert_eq!(restored.file_format_name, "Image File");
    assert_eq!(restored.priority, 5.0);
    assert_eq!(restored.library, "libImageViewer");
    assert_eq!(restored.function, "CreateImagePanel");
    assert_eq!(restored.model_function, "AcquireImageModel");
    assert_eq!(restored.model_classes, vec!["emImageFileModel"]);
    assert!(restored.model_able_to_save);
    assert_eq!(restored.properties.len(), 2);
    assert_eq!(restored.properties[0].name, "MaxWidth");
    assert_eq!(restored.properties[0].value, "4096");
    assert_eq!(restored.properties[1].name, "Format");
    assert_eq!(restored.properties[1].value, "RGBA");
}

#[test]
fn from_rec_missing_fields_uses_defaults() {
    let rec = RecStruct::new();
    let plugin = FpPlugin::from_rec(&rec).unwrap();

    assert!(plugin.file_types.is_empty());
    assert_eq!(plugin.priority, 1.0);
    assert_eq!(plugin.library, "unknown");
    assert_eq!(plugin.function, "unknown");
    assert!(!plugin.model_able_to_save);
}

// ── is_default / set_to_default ─────────────────────────────────────

#[test]
fn default_is_default() {
    let plugin = FpPlugin::default();
    assert!(plugin.is_default());
}

#[test]
fn set_to_default_restores() {
    let mut plugin = FpPlugin::new();
    plugin.file_types = vec![".txt".to_string()];
    plugin.priority = 10.0;
    plugin.library = "foo".to_string();
    assert!(!plugin.is_default());
    plugin.set_to_default();
    assert!(plugin.is_default());
}

// ── GetProperty ─────────────────────────────────────────────────────

#[test]
fn get_property_found() {
    let mut plugin = FpPlugin::new();
    plugin.properties = vec![
        FpPluginProperty {
            name: "A".to_string(),
            value: "1".to_string(),
        },
        FpPluginProperty {
            name: "B".to_string(),
            value: "2".to_string(),
        },
    ];
    let prop = plugin.get_property("B").unwrap();
    assert_eq!(prop.value, "2");
}

#[test]
fn get_property_not_found() {
    let plugin = FpPlugin::default();
    assert!(plugin.get_property("missing").is_none());
}

// ── is_matching ─────────────────────────────────────────────────────

#[test]
fn matches_by_extension() {
    let plugin = make_plugin(&[".png", ".jpg"], 1.0, "lib", "fn");

    assert!(plugin.is_matching(None, Some("photo.png"), false, FileStatMode::Regular));
    assert!(plugin.is_matching(None, Some("photo.JPG"), false, FileStatMode::Regular));
    assert!(!plugin.is_matching(None, Some("photo.gif"), false, FileStatMode::Regular));
    // Extension match only for regular files.
    assert!(!plugin.is_matching(None, Some("photo.png"), false, FileStatMode::Directory));
}

#[test]
fn matches_file_wildcard() {
    let plugin = make_plugin(&["file"], 1.0, "lib", "fn");

    assert!(plugin.is_matching(None, Some("anything.xyz"), false, FileStatMode::Regular));
    assert!(!plugin.is_matching(None, Some("anything.xyz"), false, FileStatMode::Directory));
}

#[test]
fn matches_directory_wildcard() {
    let plugin = make_plugin(&["directory"], 1.0, "lib", "fn");

    assert!(plugin.is_matching(None, Some("somedir"), false, FileStatMode::Directory));
    assert!(!plugin.is_matching(None, Some("somedir"), false, FileStatMode::Regular));
}

#[test]
fn matches_model_class() {
    let plugin = make_plugin_full(&[".png"], 1.0, "lib", "fn", &["ImageModel"], false);

    assert!(plugin.is_matching(Some("ImageModel"), None, false, FileStatMode::Regular));
    assert!(!plugin.is_matching(Some("OtherModel"), None, false, FileStatMode::Regular));
}

#[test]
fn matches_require_able_to_save() {
    let saveable = make_plugin_full(&[".png"], 1.0, "lib", "fn", &["M"], true);
    let read_only = make_plugin_full(&[".png"], 1.0, "lib", "fn", &["M"], false);

    assert!(saveable.is_matching(None, None, true, FileStatMode::Regular));
    assert!(!read_only.is_matching(None, None, true, FileStatMode::Regular));
    // Without the flag, both match.
    assert!(read_only.is_matching(None, None, false, FileStatMode::Regular));
}

#[test]
fn no_filters_matches_everything() {
    let plugin = make_plugin(&[".png"], 1.0, "lib", "fn");
    assert!(plugin.is_matching(None, None, false, FileStatMode::Regular));
}

#[test]
fn extension_must_be_shorter_than_filename() {
    // C++ requires typeLen < fileNameLen (strict less-than).
    let plugin = make_plugin(&[".png"], 1.0, "lib", "fn");
    // File name is exactly ".png" — length 4, extension ".png" length 4 → no match.
    assert!(!plugin.is_matching(None, Some(".png"), false, FileStatMode::Regular));
    // "a.png" — length 5 > 4 → match.
    assert!(plugin.is_matching(None, Some("a.png"), false, FileStatMode::Regular));
}

// ── FpPluginList search ─────────────────────────────────────────────

#[test]
fn search_plugin_by_extension() {
    let list = FpPluginList::from_plugins(vec![
        make_plugin(&[".png"], 1.0, "libA", "fnA"),
        make_plugin(&[".txt"], 2.0, "libB", "fnB"),
    ]);

    let found = list
        .search_plugin(
            None,
            Some("/path/to/file.png"),
            false,
            0,
            FileStatMode::Regular,
        )
        .unwrap();
    assert_eq!(found.library, "libA");

    let found = list
        .search_plugin(
            None,
            Some("/path/to/file.txt"),
            false,
            0,
            FileStatMode::Regular,
        )
        .unwrap();
    assert_eq!(found.library, "libB");

    assert!(list
        .search_plugin(
            None,
            Some("/path/to/file.gif"),
            false,
            0,
            FileStatMode::Regular
        )
        .is_none());
}

#[test]
fn search_plugin_priority_ordering() {
    let list = FpPluginList::from_plugins(vec![
        make_plugin(&[".png"], 1.0, "libLow", "fn"),
        make_plugin(&[".png"], 5.0, "libHigh", "fn"),
        make_plugin(&[".png"], 3.0, "libMid", "fn"),
    ]);

    // alternative=0 should return highest priority.
    let found = list
        .search_plugin(None, Some("img.png"), false, 0, FileStatMode::Regular)
        .unwrap();
    assert_eq!(found.library, "libHigh");

    // alternative=1 should return second highest.
    let found = list
        .search_plugin(None, Some("img.png"), false, 1, FileStatMode::Regular)
        .unwrap();
    assert_eq!(found.library, "libMid");

    // alternative=2 should return lowest.
    let found = list
        .search_plugin(None, Some("img.png"), false, 2, FileStatMode::Regular)
        .unwrap();
    assert_eq!(found.library, "libLow");

    // alternative=3 should return None (only 3 plugins).
    assert!(list
        .search_plugin(None, Some("img.png"), false, 3, FileStatMode::Regular)
        .is_none());
}

#[test]
fn search_plugins_returns_all_sorted() {
    let list = FpPluginList::from_plugins(vec![
        make_plugin(&[".png"], 1.0, "libLow", "fn"),
        make_plugin(&[".png"], 5.0, "libHigh", "fn"),
        make_plugin(&[".jpg"], 3.0, "libJpg", "fn"),
        make_plugin(&[".png"], 3.0, "libMid", "fn"),
    ]);

    let results = list.search_plugins(None, Some("img.png"), false, FileStatMode::Regular);
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].library, "libHigh");
    assert_eq!(results[1].library, "libMid");
    assert_eq!(results[2].library, "libLow");
}

#[test]
fn search_plugin_no_file_path_matches_all() {
    let list = FpPluginList::from_plugins(vec![
        make_plugin(&[".png"], 1.0, "libA", "fn"),
        make_plugin(&[".txt"], 2.0, "libB", "fn"),
    ]);

    // No file path filter: all plugins match, highest priority first.
    let found = list
        .search_plugin(None, None, false, 0, FileStatMode::Regular)
        .unwrap();
    assert_eq!(found.library, "libB");
}

#[test]
fn search_plugin_with_model_class_filter() {
    let list = FpPluginList::from_plugins(vec![
        make_plugin_full(&[".png"], 5.0, "libA", "fn", &["ImageModel"], false),
        make_plugin_full(&[".png"], 3.0, "libB", "fn", &["TextModel"], false),
    ]);

    let found = list
        .search_plugin(
            Some("TextModel"),
            Some("x.png"),
            false,
            0,
            FileStatMode::Regular,
        )
        .unwrap();
    assert_eq!(found.library, "libB");
}

#[test]
fn search_plugin_case_insensitive_extension() {
    let list = FpPluginList::from_plugins(vec![make_plugin(&[".PNG"], 1.0, "lib", "fn")]);

    // File has lowercase extension, plugin has uppercase — should still match.
    let found = list.search_plugin(None, Some("image.png"), false, 0, FileStatMode::Regular);
    assert!(found.is_some());
}

#[test]
fn search_plugin_extracts_filename_from_path() {
    let list = FpPluginList::from_plugins(vec![make_plugin(&[".txt"], 1.0, "lib", "fn")]);

    // Full path — extension matching should use just the file name.
    let found = list.search_plugin(
        None,
        Some("/deep/nested/path/readme.txt"),
        false,
        0,
        FileStatMode::Regular,
    );
    assert!(found.is_some());
}

#[test]
fn plugin_count() {
    let list = FpPluginList::from_plugins(vec![
        make_plugin(&[".a"], 1.0, "l1", "f1"),
        make_plugin(&[".b"], 2.0, "l2", "f2"),
    ]);
    assert_eq!(list.plugin_count(), 2);
}

#[test]
fn empty_plugin_list() {
    let list = FpPluginList::from_plugins(vec![]);
    assert_eq!(list.plugin_count(), 0);
    assert!(list
        .search_plugin(None, Some("x.txt"), false, 0, FileStatMode::Regular)
        .is_none());
    assert!(list
        .search_plugins(None, Some("x.txt"), false, FileStatMode::Regular)
        .is_empty());
}
