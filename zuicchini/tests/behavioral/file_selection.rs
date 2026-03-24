use std::fs;
use std::path::{Path, PathBuf};

use zuicchini::emCore::emFileSelectionBox::emFileSelectionBox;

/// RAII guard that removes a temporary directory on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("zuicchini_test_{name}_{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).expect("cleanup pre-existing tempdir");
        }
        fs::create_dir_all(&path).expect("create tempdir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn lists_directory_contents() {
    let tmp = TempDir::new("lists_dir");
    fs::write(tmp.path().join("alpha.txt"), "a").unwrap();
    fs::write(tmp.path().join("bravo.txt"), "b").unwrap();
    fs::write(tmp.path().join("charlie.rs"), "c").unwrap();
    fs::create_dir(tmp.path().join("delta_dir")).unwrap();

    let mut fsb = emFileSelectionBox::new("test");
    fsb.set_parent_directory(tmp.path());
    fsb.reload_listing();

    let listing = fsb.GetListing();
    // ".." entry is prepended when not at root, so expect 5 entries total.
    let names: Vec<&str> = listing.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&".."), "listing should contain '..' entry");
    assert!(names.contains(&"alpha.txt"), "listing should contain alpha.txt");
    assert!(names.contains(&"bravo.txt"), "listing should contain bravo.txt");
    assert!(names.contains(&"charlie.rs"), "listing should contain charlie.rs");
    assert!(names.contains(&"delta_dir"), "listing should contain delta_dir");
    assert_eq!(names.len(), 5);

    // Verify directory flag on delta_dir.
    let delta = listing.iter().find(|(n, _)| n == "delta_dir").unwrap();
    assert!(delta.1.is_directory);
}

#[test]
fn filter_applies() {
    let tmp = TempDir::new("filter");
    fs::write(tmp.path().join("foo.txt"), "").unwrap();
    fs::write(tmp.path().join("bar.rs"), "").unwrap();
    fs::write(tmp.path().join("baz.txt"), "").unwrap();

    let mut fsb = emFileSelectionBox::new("test");
    fsb.set_parent_directory(tmp.path());
    fsb.set_filters(&["Text (*.txt)".to_string()]);
    fsb.set_selected_filter_index(0);
    fsb.reload_listing();

    let listing = fsb.GetListing();
    let names: Vec<&str> = listing.iter().map(|(n, _)| n.as_str()).collect();

    // .txt files should appear; .rs file should be filtered out.
    // ".." also appears (directory, passes filter).
    assert!(names.contains(&"foo.txt"));
    assert!(names.contains(&"baz.txt"));
    assert!(!names.contains(&"bar.rs"), "bar.rs should be filtered out");
    assert!(names.contains(&".."));
}

#[test]
fn select_returns_path() {
    let tmp = TempDir::new("select_path");

    let mut fsb = emFileSelectionBox::new("test");
    fsb.set_parent_directory(tmp.path());
    fsb.set_selected_name("foo.txt");

    let canonical_tmp = fs::canonicalize(tmp.path()).unwrap();
    assert_eq!(fsb.GetSelectedPath(), canonical_tmp.join("foo.txt"));
}

#[test]
fn navigate_subdir() {
    let tmp = TempDir::new("nav_sub");
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("inner.txt"), "").unwrap();

    let mut fsb = emFileSelectionBox::new("test");
    fsb.set_parent_directory(tmp.path());
    fsb.enter_sub_dir("sub");

    let canonical_sub = fs::canonicalize(&sub).unwrap();
    assert_eq!(
        fs::canonicalize(fsb.GetParentDirectory()).unwrap(),
        canonical_sub,
    );
}

#[test]
fn navigate_parent() {
    let tmp = TempDir::new("nav_parent");
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).unwrap();

    let mut fsb = emFileSelectionBox::new("test");
    fsb.set_parent_directory(&sub);
    fsb.enter_sub_dir("..");

    // enter_sub_dir("..") joins ".." to parent_dir; the resulting path contains
    // ".." but canonicalizes to the original tmp dir.
    let canonical_tmp = fs::canonicalize(tmp.path()).unwrap();
    assert_eq!(
        fs::canonicalize(fsb.GetParentDirectory()).unwrap(),
        canonical_tmp,
    );
}
