use std::path::{Path, PathBuf};

use crate::emCore::emImage::emImage;
use crate::emCore::emFileModel::{emFileModel, FileState};
use crate::emCore::emSignal::SignalId;

/// Data payload for an image file model.
///
/// Port of C++ `emImageFileModel`'s protected data members.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageFileData {
    pub image: emImage,
    pub comment: String,
    pub format_info: String,
}

impl Default for ImageFileData {
    fn default() -> Self {
        Self {
            image: emImage::new(0, 0, 4),
            comment: String::new(),
            format_info: String::new(),
        }
    }
}

/// A file model that holds image data, comment, and format info.
///
/// Port of C++ `emImageFileModel`. Wraps `emFileModel<ImageFileData>` and adds
/// a data-change signal and saving quality. Setters check equality before
/// marking the model as unsaved.
pub struct emImageFileModel {
    file_model: emFileModel<ImageFileData>,
    data_change_signal: SignalId,
    saving_quality: u32,
}

impl emImageFileModel {
    pub fn new(
        path: PathBuf,
        change_signal: SignalId,
        update_signal: SignalId,
        data_change_signal: SignalId,
    ) -> Self {
        Self {
            file_model: emFileModel::new(path, change_signal, update_signal),
            data_change_signal,
            saving_quality: 100,
        }
    }

    pub fn state(&self) -> &FileState {
        self.file_model.state()
    }

    pub fn path(&self) -> &Path {
        self.file_model.path()
    }

    pub fn file_model(&self) -> &emFileModel<ImageFileData> {
        &self.file_model
    }

    pub fn file_model_mut(&mut self) -> &mut emFileModel<ImageFileData> {
        &mut self.file_model
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.data_change_signal
    }

    pub fn GetImage(&self) -> Option<&emImage> {
        self.file_model.data().map(|d| &d.image)
    }

    pub fn GetComment(&self) -> Option<&str> {
        self.file_model.data().map(|d| d.comment.as_str())
    }

    pub fn GetFileFormatInfo(&self) -> Option<&str> {
        self.file_model.data().map(|d| d.format_info.as_str())
    }

    pub fn GetSavingQuality(&self) -> u32 {
        self.saving_quality
    }

    pub fn set_saving_quality(&mut self, quality: u32) {
        self.saving_quality = quality.min(100);
    }

    /// Set the image. Returns `true` if the image changed (and the model was
    /// marked unsaved). Returns `false` if the value was identical.
    pub fn set_image(&mut self, image: emImage) -> bool {
        if let Some(data) = self.file_model.data() {
            if data.image == image {
                return false;
            }
        }
        if let Some(data) = self.file_model.data_mut() {
            data.image = image;
            self.file_model.mark_unsaved();
            true
        } else {
            false
        }
    }

    /// Set the comment. Returns `true` if the comment changed.
    pub fn set_comment(&mut self, comment: String) -> bool {
        if let Some(data) = self.file_model.data() {
            if data.comment == comment {
                return false;
            }
        }
        if let Some(data) = self.file_model.data_mut() {
            data.comment = comment;
            self.file_model.mark_unsaved();
            true
        } else {
            false
        }
    }

    /// Set the format info. Returns `true` if the format info changed.
    pub fn SetFileFormatInfo(&mut self, info: String) -> bool {
        if let Some(data) = self.file_model.data() {
            if data.format_info == info {
                return false;
            }
        }
        if let Some(data) = self.file_model.data_mut() {
            data.format_info = info;
            self.file_model.mark_unsaved();
            true
        } else {
            false
        }
    }

    /// Reset all data to defaults. Port of C++ `emImageFileModel::ResetData`.
    pub fn reset_data(&mut self) {
        self.file_model.reset_data();
    }
}
