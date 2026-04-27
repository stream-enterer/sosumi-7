use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use crate::emEngine::{emEngine, Priority};
use crate::emEngineCtx::{ConstructCtx, EngineCtx};
use crate::emFileModel::{emFileModel, FileState};
use crate::emImage::emImage;
use crate::emPanelScope::PanelScope;
use crate::emSignal::SignalId;

/// Load an image from a path, parsing as TGA. Returns `Ok(emImage)` or an
/// error string.
///
/// Used internally by `LoaderEngine::Cycle`.
fn load_image_sync(path: &Path) -> Result<emImage, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    crate::emResTga::load_tga(&data).map_err(|e| e.to_string())
}

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
    pub fn new(path: PathBuf, change_signal: SignalId, data_change_signal: SignalId) -> Self {
        Self {
            file_model: emFileModel::new(path, change_signal),
            data_change_signal,
            saving_quality: 100,
        }
    }

    /// Scheduler-driven factory: registers a `LoaderEngine` that will load
    /// the image on the next time-slice and fire a completion signal.
    ///
    /// DIVERGED: (language-forced) C++ creates file models via `emModel::Acquire(context, name)`,
    /// a context-registered, path-keyed shared-instance pattern backed by virtual inheritance
    /// (`emModel : emEngine`, `emFileModel : emModel`). Rust has no virtual base-class
    /// inheritance; the `emModel` context-registry infrastructure is not yet ported, so
    /// callers receive an explicit `Rc<RefCell<Self>>` and manage sharing themselves.
    /// The observable loading contract — async engine fires on schedule, signals on completion —
    /// is preserved.
    pub fn register<C: ConstructCtx>(ctx: &mut C, path: PathBuf) -> Rc<RefCell<Self>> {
        let change_signal = ctx.create_signal();
        let load_complete_signal = ctx.create_signal();

        let mut model = Self::new(path, change_signal, load_complete_signal);
        model.file_model.Load();

        let model_rc = Rc::new(RefCell::new(model));
        let model_weak = Rc::downgrade(&model_rc);

        let engine = Box::new(LoaderEngine {
            model_weak,
            load_complete_signal,
        });
        let eid = ctx.register_engine(engine, Priority::Low, PanelScope::Framework);
        ctx.wake_up(eid);

        model_rc
    }

    pub fn state(&self) -> &FileState {
        self.file_model.GetFileState()
    }

    pub fn path(&self) -> &Path {
        self.file_model.GetFilePath()
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
        self.file_model.GetMap().map(|d| &d.image)
    }

    pub fn GetComment(&self) -> Option<&str> {
        self.file_model.GetMap().map(|d| d.comment.as_str())
    }

    pub fn GetFileFormatInfo(&self) -> Option<&str> {
        self.file_model.GetMap().map(|d| d.format_info.as_str())
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
        if let Some(data) = self.file_model.GetMap() {
            if data.image == image {
                return false;
            }
        }
        if let Some(data) = self.file_model.GetWritableMap() {
            data.image = image;
            self.file_model.SetUnsavedState();
            true
        } else {
            false
        }
    }

    /// Set the comment. Returns `true` if the comment changed.
    pub fn set_comment(&mut self, comment: String) -> bool {
        if let Some(data) = self.file_model.GetMap() {
            if data.comment == comment {
                return false;
            }
        }
        if let Some(data) = self.file_model.GetWritableMap() {
            data.comment = comment;
            self.file_model.SetUnsavedState();
            true
        } else {
            false
        }
    }

    /// Set the format info. Returns `true` if the format info changed.
    pub fn SetFileFormatInfo(&mut self, info: String) -> bool {
        if let Some(data) = self.file_model.GetMap() {
            if data.format_info == info {
                return false;
            }
        }
        if let Some(data) = self.file_model.GetWritableMap() {
            data.format_info = info;
            self.file_model.SetUnsavedState();
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

// ---------------------------------------------------------------------------
// LoaderEngine — one-shot engine that drives async image loading.
// ---------------------------------------------------------------------------

/// One-shot scheduler engine that loads an image file on the next time-slice.
///
/// Port of C++ `emImageFileModel`'s internal async load path: the model
/// registers itself with the scheduler, which calls back on the next
/// `DoTimeSlice`. `LoaderEngine` does the synchronous I/O + TGA parse, stores
/// the result in the model, fires the load-complete signal, and removes itself.
struct LoaderEngine {
    model_weak: Weak<RefCell<emImageFileModel>>,
    load_complete_signal: SignalId,
}

impl emEngine for LoaderEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
        let engine_id = ctx.engine_id;

        let Some(model_rc) = self.model_weak.upgrade() else {
            // Model was dropped before we ran — clean up and exit.
            ctx.remove_engine(engine_id);
            return false;
        };

        let path = model_rc.borrow().path().to_path_buf();
        let result = load_image_sync(&path);

        let file_state_signal;
        {
            let mut m = model_rc.borrow_mut();
            match result {
                Ok(img) => m.file_model_mut().complete_load(ImageFileData {
                    image: img,
                    comment: String::new(),
                    format_info: String::new(),
                }),
                Err(e) => m.file_model_mut().fail_load(e),
            }
            // Mirror C++ emFileModel::Cycle: fire FileStateSignal on every state transition.
            file_state_signal = m.file_model().GetFileStateSignal();
        }

        ctx.fire(file_state_signal);
        ctx.fire(self.load_complete_signal);
        ctx.remove_engine(engine_id);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_file_data_default() {
        let d = ImageFileData::default();
        assert_eq!(d.image.GetWidth(), 0);
        assert_eq!(d.image.GetHeight(), 0);
        assert!(d.comment.is_empty());
        assert!(d.format_info.is_empty());
    }
}
