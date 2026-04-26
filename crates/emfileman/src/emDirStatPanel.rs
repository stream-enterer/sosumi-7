use std::cell::RefCell;
use std::rc::Rc;

use emcore::emColor::emColor;
use emcore::emContext::emContext;
use emcore::emEngineCtx::PanelCtx;
use emcore::emFilePanel::{emFilePanel, VirtualFileState};
use emcore::emPainter::{emPainter, TextAlignment, VAlign};
use emcore::emPanel::{PanelBehavior, PanelState};

use crate::emDirEntry::emDirEntry;
use crate::emFileManViewConfig::emFileManViewConfig;

pub struct DirStatistics {
    pub total_count: i32,
    pub file_count: i32,
    pub sub_dir_count: i32,
    pub other_type_count: i32,
    pub hidden_count: i32,
}

impl DirStatistics {
    pub fn from_entries(entries: &[emDirEntry]) -> Self {
        let mut s = Self {
            total_count: 0,
            file_count: 0,
            sub_dir_count: 0,
            other_type_count: 0,
            hidden_count: 0,
        };
        for e in entries {
            s.total_count += 1;
            if e.IsHidden() {
                s.hidden_count += 1;
            }
            if e.IsDirectory() {
                s.sub_dir_count += 1;
            } else if e.IsRegularFile() {
                s.file_count += 1;
            } else {
                s.other_type_count += 1;
            }
        }
        s
    }

    pub fn format_text(&self) -> String {
        format!(
            "Directory Statistics\n\
             ~~~~~~~~~~~~~~~~~~~~\n\
             \n\
             Total Entries : {:5}\n\
             Hidden Entries: {:5}\n\
             Regular Files : {:5}\n\
             Subdirectories: {:5}\n\
             Other Types   : {:5}",
            self.total_count,
            self.hidden_count,
            self.file_count,
            self.sub_dir_count,
            self.other_type_count
        )
    }
}

/// Directory statistics panel.
/// Port of C++ `emDirStatPanel` (extends emFilePanel).
pub struct emDirStatPanel {
    pub(crate) file_panel: emFilePanel,
    config: Rc<RefCell<emFileManViewConfig>>,
    stats: DirStatistics,
}

impl emDirStatPanel {
    pub fn new(ctx: Rc<emContext>) -> Self {
        let config = emFileManViewConfig::Acquire(&ctx);
        Self {
            file_panel: emFilePanel::new(),
            config,
            stats: DirStatistics {
                total_count: -1,
                file_count: -1,
                sub_dir_count: -1,
                other_type_count: -1,
                hidden_count: -1,
            },
        }
    }

    fn update_statistics(&mut self) {
        if self.file_panel.GetVirFileState() != VirtualFileState::Loaded {
            self.stats = DirStatistics {
                total_count: -1,
                file_count: -1,
                sub_dir_count: -1,
                other_type_count: -1,
                hidden_count: -1,
            };
        }
    }

    /// Update statistics from a slice of entries (called by parent when model loads).
    pub fn set_entries(&mut self, entries: &[crate::emDirEntry::emDirEntry]) {
        self.stats = DirStatistics::from_entries(entries);
    }
}

impl PanelBehavior for emDirStatPanel {
    fn Cycle(
        &mut self,
        _ectx: &mut emcore::emEngineCtx::EngineCtx<'_>,
        _ctx: &mut PanelCtx,
    ) -> bool {
        self.file_panel.refresh_vir_file_state();
        self.update_statistics();
        false
    }

    fn IsOpaque(&self) -> bool {
        if self.file_panel.GetVirFileState() != VirtualFileState::Loaded {
            return false;
        }
        let config = self.config.borrow();
        let theme = config.GetTheme();
        let bg = theme.GetRec().BackgroundColor;
        (bg >> 24) == 0xFF
    }

    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        if self.file_panel.GetVirFileState() != VirtualFileState::Loaded {
            self.file_panel.paint_status(painter, canvas_color, w, h);
            return;
        }

        let config = self.config.borrow();
        let theme = config.GetTheme();
        let bg_color = emColor::from_packed(theme.GetRec().BackgroundColor);
        painter.Clear(bg_color);

        let text = self.stats.format_text();
        let name_color = emColor::from_packed(theme.GetRec().DirNameColor);
        painter.PaintTextBoxed(
            0.02,
            0.02,
            w - 0.04,
            state.height - 0.04,
            &text,
            state.height,
            name_color,
            bg_color,
            TextAlignment::Left,
            VAlign::Top,
            TextAlignment::Left,
            0.5,
            false,
            1.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;
    use crate::emDirEntry::emDirEntry;

    #[test]
    fn count_entries() {
        let entries = vec![
            emDirEntry::from_path("/tmp"),
            emDirEntry::from_path("/dev/null"),
        ];
        let stats = DirStatistics::from_entries(&entries);
        assert_eq!(stats.total_count, 2);
        assert!(stats.sub_dir_count >= 1); // /tmp is a directory
    }

    #[test]
    fn empty_entries() {
        let stats = DirStatistics::from_entries(&[]);
        assert_eq!(stats.total_count, 0);
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.sub_dir_count, 0);
        assert_eq!(stats.other_type_count, 0);
        assert_eq!(stats.hidden_count, 0);
    }

    #[test]
    fn hidden_count() {
        let dir = std::env::temp_dir();
        let hidden = dir.join(".test_hidden_stat_emfileman");
        std::fs::write(&hidden, "x").unwrap();
        let entries = vec![emDirEntry::from_path(hidden.to_str().unwrap())];
        let stats = DirStatistics::from_entries(&entries);
        assert_eq!(stats.hidden_count, 1);
        std::fs::remove_file(&hidden).unwrap();
    }

    #[test]
    fn format_text_output() {
        let stats = DirStatistics {
            total_count: 10,
            file_count: 5,
            sub_dir_count: 3,
            other_type_count: 2,
            hidden_count: 1,
        };
        let text = stats.format_text();
        assert!(text.contains("Total Entries :    10"));
        assert!(text.contains("Hidden Entries:     1"));
    }

    #[test]
    fn panel_implements_panel_behavior() {
        use emcore::emPanel::PanelBehavior;

        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emDirStatPanel::new(Rc::clone(&ctx));
        let _: Box<dyn PanelBehavior> = Box::new(panel);
    }

    #[test]
    fn panel_initial_vfs_is_no_model() {
        let ctx = emcore::emContext::emContext::NewRoot();
        let panel = emDirStatPanel::new(Rc::clone(&ctx));
        assert_eq!(
            panel.file_panel.GetVirFileState(),
            emcore::emFilePanel::VirtualFileState::NoFileModel
        );
    }
}
