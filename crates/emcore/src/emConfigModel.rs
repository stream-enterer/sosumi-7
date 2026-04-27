use std::path::{Path, PathBuf};

use crate::emRecParser::{parse_rec, write_rec, write_rec_with_format, RecError};
use crate::emSignal::SignalId;

use crate::emRecRecord::Record;

/// A configuration record backed by a file path with emRec serialization.
///
/// Tracks a dirty flag for unsaved changes. `load()` reads from disk,
/// `save()` writes to disk. `load_or_install()` handles first-run by
/// creating a default config file if none exists.
///
/// TODO(phase-4d-followup): retire this type in favour of
/// `emRecNodeConfigModel<T: emRecNode>` (see `emRecNodeConfigModel.rs`).
/// Migrate callers (emView, emWindowStateSaver, emCoreConfigPanel,
/// emMainConfig, emBookmarks, emAutoplay, emFileManConfig,
/// emFileManTheme, emCoreConfig, emFileLinkModel, emStocksFileModel)
/// one-by-one, then delete this file + its companion `emRecRecord`
/// `Record` trait + the legacy `emRecParser::parse_rec`/`write_rec`
/// tree parser. Blocked until every caller is ported; Phase 4e starts
/// with emCoreConfig.
pub struct emConfigModel<T: Record> {
    value: T,
    path: PathBuf,
    change_signal: SignalId,
    dirty: bool,
    /// Optional format name for `#%rec:FormatName%#` header.
    format_name: Option<String>,
}

impl<T: Record> emConfigModel<T> {
    pub fn new(value: T, path: PathBuf, signal_id: SignalId) -> Self {
        Self {
            value,
            path,
            change_signal: signal_id,
            dirty: false,
            format_name: None,
        }
    }

    /// Set the format name for the `#%rec:FormatName%#` header in saved files.
    pub fn with_format_name(mut self, name: &str) -> Self {
        self.format_name = Some(name.to_string());
        self
    }

    pub fn GetRec(&self) -> &T {
        &self.value
    }

    /// Replace the value. Returns `true` if dirty flag was set (always, since
    /// Record types don't require PartialEq).
    pub fn Set(&mut self, new_value: T) -> bool {
        self.value = new_value;
        self.dirty = true;
        true
    }

    /// Modify the value in place. Returns `true` (marks dirty).
    pub fn modify<F: FnOnce(&mut T)>(&mut self, f: F) -> bool {
        f(&mut self.value);
        self.dirty = true;
        true
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.change_signal
    }

    /// Test helper: replace the stored signal ID. Used when the model was
    /// created with `SignalId::null()` (no scheduler available at construction
    /// time) and must later be wired to a real scheduler signal for testing.
    /// pub so downstream crate tests (emmain) can call via emMainConfig.
    pub fn set_change_signal(&mut self, sig: SignalId) {
        self.change_signal = sig;
    }

    pub fn GetInstallPath(&self) -> &Path {
        &self.path
    }

    pub fn IsUnsaved(&self) -> bool {
        self.dirty
    }

    /// Reset the value to its default. Returns `true` if dirty flag was set.
    pub fn SetToDefault(&mut self) -> bool {
        self.value.SetToDefault();
        self.dirty = true;
        true
    }

    /// Load the configuration from disk. Parses emRec and deserializes.
    pub fn TryLoad(&mut self) -> Result<(), RecError> {
        let contents = std::fs::read_to_string(&self.path).map_err(RecError::Io)?;
        let rec = parse_rec(&contents)?;
        self.value = T::from_rec(&rec)?;
        self.dirty = false;
        Ok(())
    }

    /// Save the configuration to disk as emRec.
    pub fn Save(&mut self) -> Result<(), RecError> {
        let rec = self.value.to_rec();
        let contents = if let Some(ref fmt) = self.format_name {
            write_rec_with_format(&rec, fmt)
        } else {
            write_rec(&rec)
        };

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(RecError::Io)?;
        }

        std::fs::write(&self.path, contents).map_err(RecError::Io)?;
        self.dirty = false;
        Ok(())
    }

    /// Load from disk, or create a default config file if none exists.
    pub fn TryLoadOrInstall(&mut self) -> Result<(), RecError> {
        if self.path.exists() {
            self.TryLoad()
        } else {
            self.value.SetToDefault();
            self.dirty = true;
            self.Save()
        }
    }
}
