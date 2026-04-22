//! emTmpFile — temporary file path holder with RAII deletion, and
//! emTmpFileMaster — singleton manager for crash-resilient temp cleanup.
//!
//! C++ emTmpFile.h provides emTmpFile (RAII path holder) and
//! emTmpFileMaster (IPC-based singleton for crash-resilient cleanup).
//!
//! DIVERGED: (language-forced) C++ emTmpFileMaster uses emMiniIpc for the singleton.
//! Rust uses flock-based file locking instead.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Temporary file/directory path holder. Deletes the file or directory
/// tree on drop. Matches C++ `emTmpFile`.
pub struct emTmpFile {
    path: PathBuf,
}

impl emTmpFile {
    /// Construct with empty path (no file to delete). C++ `emTmpFile()`.
    pub fn new() -> Self {
        Self {
            path: PathBuf::new(),
        }
    }

    /// Construct with an explicit path. C++ `emTmpFile(const emString&)`.
    pub fn from_custom_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Set a custom path. Calls Discard() first. C++ `SetupCustomPath`.
    pub fn SetupCustomPath(&mut self, path: PathBuf) {
        self.Discard();
        self.path = path;
    }

    /// Get the current path. C++ `GetPath`.
    pub fn GetPath(&self) -> &Path {
        &self.path
    }

    /// Delete the file/directory and clear the path. C++ `Discard`.
    pub fn Discard(&mut self) {
        if !self.path.as_os_str().is_empty() {
            if self.path.is_dir() {
                let _ = std::fs::remove_dir_all(&self.path);
            } else if self.path.exists() {
                let _ = std::fs::remove_file(&self.path);
            }
            self.path = PathBuf::new();
        }
    }
}

impl Default for emTmpFile {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for emTmpFile {
    fn drop(&mut self) {
        self.Discard();
    }
}

/// Singleton manager for temporary file cleanup.
/// Uses file locking to ensure only one master per temp directory.
///
/// DIVERGED: (language-forced) C++ emTmpFileMaster uses emMiniIpc for the singleton.
/// Rust uses flock-based file locking instead.
pub struct emTmpFileMaster {
    /// Held open to maintain the flock; not read or written after acquisition.
    _lock_file: std::fs::File,
    lock_path: PathBuf,
    registered: HashSet<PathBuf>,
    base_dir: PathBuf,
}

impl emTmpFileMaster {
    /// Try to acquire the master lock for the given temp directory.
    /// Returns `None` if another process holds the lock.
    pub fn acquire(base_dir: &Path) -> Option<Self> {
        let lock_path = base_dir.join(".emTmpFileMaster.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .ok()?;

        // Try non-blocking exclusive lock
        use std::os::unix::io::AsRawFd;
        let rc = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            return None; // Another process holds the lock
        }

        let mut master = Self {
            _lock_file: lock_file,
            lock_path,
            registered: HashSet::new(),
            base_dir: base_dir.to_path_buf(),
        };

        // Clean orphaned temp files (from crashed processes)
        master.clean_orphans();

        Some(master)
    }

    /// Register a temp file path for cleanup tracking.
    pub fn register(&mut self, path: &Path) {
        self.registered.insert(path.to_path_buf());
    }

    /// Unregister a temp file path.
    pub fn unregister(&mut self, path: &Path) {
        self.registered.remove(path);
    }

    /// Check if a path is registered.
    pub fn is_registered(&self, path: &Path) -> bool {
        self.registered.contains(path)
    }

    /// Scan for and remove orphaned temp files.
    fn clean_orphans(&mut self) {
        if let Ok(entries) = std::fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Match emTmpFile naming pattern: em_tmp_*
                if name_str.starts_with("em_tmp_") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

impl Drop for emTmpFileMaster {
    fn drop(&mut self) {
        // Clean up all registered temp files
        for path in &self.registered {
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }

        // Release the lock file
        let _ = std::fs::remove_file(&self.lock_path);
    }
}
