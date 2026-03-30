// Selection subsystem of emFileManModel.
// Command tree and IPC will be added in Tasks 8 and 9.

use emcore::emStd2::emCalcHashCode;
use std::path::Path;

struct SelEntry {
    hash_code: i32,
    path: String,
}

pub struct SelectionManager {
    sel: [Vec<SelEntry>; 2], // 0=source, 1=target
    #[allow(dead_code)]
    shift_tgt_sel_path: String,
    sel_cmd_counter: u32,
}

/// Binary search over a sorted `Vec<SelEntry>`, ordered by `(hash_code, path)`.
/// Returns `Ok(index)` if found, `Err(insert_pos)` if not.
fn search_selection(sel: &[SelEntry], hash_code: i32, path: &str) -> Result<usize, usize> {
    let mut lo: usize = 0;
    let mut hi: usize = sel.len();
    while lo < hi {
        let mid = (lo + hi) >> 1;
        let entry = &sel[mid];
        if entry.hash_code > hash_code {
            hi = mid;
        } else if entry.hash_code < hash_code {
            lo = mid + 1;
        } else {
            match entry.path.as_str().cmp(path) {
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Equal => return Ok(mid),
            }
        }
    }
    Err(hi)
}

impl SelectionManager {
    pub fn new() -> Self {
        Self {
            sel: [Vec::new(), Vec::new()],
            shift_tgt_sel_path: String::new(),
            sel_cmd_counter: 0,
        }
    }

    // --- Source selection ---

    pub fn GetSourceSelectionCount(&self) -> usize {
        self.sel[0].len()
    }

    pub fn GetSourceSelection(&self, index: usize) -> &str {
        &self.sel[0][index].path
    }

    pub fn IsSelectedAsSource(&self, path: &str) -> bool {
        let hash_code = emCalcHashCode(path.as_bytes(), 0);
        search_selection(&self.sel[0], hash_code, path).is_ok()
    }

    pub fn SelectAsSource(&mut self, path: &str) {
        let hash_code = emCalcHashCode(path.as_bytes(), 0);
        if let Err(pos) = search_selection(&self.sel[0], hash_code, path) {
            self.sel[0].insert(
                pos,
                SelEntry {
                    hash_code,
                    path: path.to_string(),
                },
            );
            self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
        }
    }

    pub fn DeselectAsSource(&mut self, path: &str) {
        let hash_code = emCalcHashCode(path.as_bytes(), 0);
        if let Ok(pos) = search_selection(&self.sel[0], hash_code, path) {
            self.sel[0].remove(pos);
            self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
        }
    }

    pub fn ClearSourceSelection(&mut self) {
        if !self.sel[0].is_empty() {
            self.sel[0].clear();
            self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
        }
    }

    // --- Target selection ---

    pub fn GetTargetSelectionCount(&self) -> usize {
        self.sel[1].len()
    }

    pub fn GetTargetSelection(&self, index: usize) -> &str {
        &self.sel[1][index].path
    }

    pub fn IsSelectedAsTarget(&self, path: &str) -> bool {
        let hash_code = emCalcHashCode(path.as_bytes(), 0);
        search_selection(&self.sel[1], hash_code, path).is_ok()
    }

    pub fn SelectAsTarget(&mut self, path: &str) {
        let hash_code = emCalcHashCode(path.as_bytes(), 0);
        if let Err(pos) = search_selection(&self.sel[1], hash_code, path) {
            self.sel[1].insert(
                pos,
                SelEntry {
                    hash_code,
                    path: path.to_string(),
                },
            );
            self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
        }
    }

    pub fn DeselectAsTarget(&mut self, path: &str) {
        let hash_code = emCalcHashCode(path.as_bytes(), 0);
        if let Ok(pos) = search_selection(&self.sel[1], hash_code, path) {
            self.sel[1].remove(pos);
            self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
        }
    }

    pub fn ClearTargetSelection(&mut self) {
        if !self.sel[1].is_empty() {
            self.sel[1].clear();
            self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
        }
    }

    // --- Cross-selection operations ---

    pub fn SwapSelection(&mut self) {
        self.sel.swap(0, 1);
        self.sel_cmd_counter = self.sel_cmd_counter.wrapping_add(1);
    }

    /// Returns true if any selected path (source or target) is within the
    /// given directory tree. A path is "in" the dir tree if it starts with
    /// `dir_path` followed by `'/'`, or equals `dir_path` exactly.
    pub fn IsAnySelectionInDirTree(&self, dir_path: &str) -> bool {
        for arr in &self.sel {
            for entry in arr {
                if entry.path == dir_path
                    || (entry.path.starts_with(dir_path)
                        && entry.path.as_bytes().get(dir_path.len()) == Some(&b'/'))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Remove entries whose paths no longer exist on the filesystem.
    pub fn UpdateSelection(&mut self) {
        for arr in &mut self.sel {
            arr.retain(|entry| Path::new(&entry.path).exists());
        }
    }

    pub fn GetCommandRunId(&self) -> String {
        format!("{}", self.sel_cmd_counter)
    }
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selections() {
        let m = SelectionManager::new();
        assert_eq!(m.GetSourceSelectionCount(), 0);
        assert_eq!(m.GetTargetSelectionCount(), 0);
        assert!(!m.IsSelectedAsSource("/foo"));
        assert!(!m.IsSelectedAsTarget("/foo"));
    }

    #[test]
    fn select_and_deselect_source() {
        let mut m = SelectionManager::new();
        m.SelectAsSource("/foo");
        assert!(m.IsSelectedAsSource("/foo"));
        assert_eq!(m.GetSourceSelectionCount(), 1);

        m.DeselectAsSource("/foo");
        assert!(!m.IsSelectedAsSource("/foo"));
        assert_eq!(m.GetSourceSelectionCount(), 0);
    }

    #[test]
    fn select_and_deselect_target() {
        let mut m = SelectionManager::new();
        m.SelectAsTarget("/bar");
        assert!(m.IsSelectedAsTarget("/bar"));
        assert_eq!(m.GetTargetSelectionCount(), 1);

        m.DeselectAsTarget("/bar");
        assert!(!m.IsSelectedAsTarget("/bar"));
    }

    #[test]
    fn duplicate_select_is_idempotent() {
        let mut m = SelectionManager::new();
        m.SelectAsSource("/foo");
        m.SelectAsSource("/foo");
        assert_eq!(m.GetSourceSelectionCount(), 1);
    }

    #[test]
    fn swap_selection() {
        let mut m = SelectionManager::new();
        m.SelectAsSource("/src1");
        m.SelectAsTarget("/tgt1");
        m.SwapSelection();
        assert!(m.IsSelectedAsTarget("/src1"));
        assert!(m.IsSelectedAsSource("/tgt1"));
    }

    #[test]
    fn clear_selections() {
        let mut m = SelectionManager::new();
        m.SelectAsSource("/s1");
        m.SelectAsSource("/s2");
        m.SelectAsTarget("/t1");
        m.ClearSourceSelection();
        assert_eq!(m.GetSourceSelectionCount(), 0);
        assert_eq!(m.GetTargetSelectionCount(), 1);
        m.ClearTargetSelection();
        assert_eq!(m.GetTargetSelectionCount(), 0);
    }

    #[test]
    fn hash_binary_search_ordering() {
        let mut m = SelectionManager::new();
        m.SelectAsTarget("/z/last");
        m.SelectAsTarget("/a/first");
        m.SelectAsTarget("/m/middle");
        assert_eq!(m.GetTargetSelectionCount(), 3);
        assert!(m.IsSelectedAsTarget("/a/first"));
        assert!(m.IsSelectedAsTarget("/m/middle"));
        assert!(m.IsSelectedAsTarget("/z/last"));
    }

    #[test]
    fn get_selection_by_index() {
        let mut m = SelectionManager::new();
        m.SelectAsSource("/b");
        m.SelectAsSource("/a");
        assert_eq!(m.GetSourceSelectionCount(), 2);
        let s0 = m.GetSourceSelection(0);
        let s1 = m.GetSourceSelection(1);
        assert!(s0 == "/a" || s0 == "/b");
        assert!(s1 == "/a" || s1 == "/b");
        assert_ne!(s0, s1);
    }

    #[test]
    fn is_any_selection_in_dir_tree() {
        let mut m = SelectionManager::new();
        m.SelectAsTarget("/home/user/docs/file.txt");
        assert!(m.IsAnySelectionInDirTree("/home/user/docs"));
        assert!(m.IsAnySelectionInDirTree("/home/user"));
        assert!(m.IsAnySelectionInDirTree("/home"));
        assert!(!m.IsAnySelectionInDirTree("/tmp"));
    }

    #[test]
    fn update_selection_removes_nonexistent() {
        let mut m = SelectionManager::new();
        m.SelectAsTarget("/dev/null"); // exists
        m.SelectAsTarget("/nonexistent_emfileman_test"); // doesn't exist
        assert_eq!(m.GetTargetSelectionCount(), 2);
        m.UpdateSelection();
        assert_eq!(m.GetTargetSelectionCount(), 1);
        assert!(m.IsSelectedAsTarget("/dev/null"));
    }

    #[test]
    fn command_run_id_changes() {
        let mut m = SelectionManager::new();
        let id1 = m.GetCommandRunId();
        m.SelectAsSource("/foo");
        let id2 = m.GetCommandRunId();
        assert_ne!(id1, id2);
    }
}
