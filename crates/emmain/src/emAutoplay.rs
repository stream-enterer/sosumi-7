use emcore::emConfigModel::emConfigModel;
use emcore::emContext::emContext;
use emcore::emInstallInfo::{emGetInstallPath, InstallDirType};
use emcore::emPanelTree::{AutoplayHandlingFlags, DecodeIdentity, EncodeIdentity, PanelId, PanelTree};
use emcore::emRec::{RecError, RecStruct};
use emcore::emRecRecord::Record;
use emcore::emSignal::SignalId;
use slotmap::Key as _;
use std::cell::RefCell;
use std::rc::Rc;

//==============================================================================
//========================== emAutoplayConfigRec ===============================
//==============================================================================

/// Record type for emAutoplayConfig fields.
///
/// Port of C++ `emAutoplayConfig` data fields. Holds autoplay timing and
/// navigation options.
// DIVERGED: C++ `emAutoplayConfig` is a single class; Rust splits the record data into
// `emAutoplayConfigRec` and the model wrapper into `emAutoplayConfig` (one primary type per file).
#[derive(Debug, Clone, PartialEq)]
pub struct emAutoplayConfigRec {
    pub DurationMS: i32,
    pub Recursive: bool,
    // NOTE: `Loop` is a valid Rust identifier (not a keyword — `loop` is the keyword).
    pub Loop: bool,
    pub LastLocationValid: bool,
    pub LastLocation: String,
}

impl Default for emAutoplayConfigRec {
    fn default() -> Self {
        Self {
            DurationMS: 5000,
            Recursive: false,
            Loop: false,
            LastLocationValid: false,
            LastLocation: String::new(),
        }
    }
}

impl Record for emAutoplayConfigRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        let d = Self::default();
        Ok(Self {
            DurationMS: rec
                .get_int("DurationMS")
                .unwrap_or(d.DurationMS)
                .clamp(100, 600_000),
            Recursive: rec.get_bool("Recursive").unwrap_or(d.Recursive),
            Loop: rec.get_bool("Loop").unwrap_or(d.Loop),
            LastLocationValid: rec
                .get_bool("LastLocationValid")
                .unwrap_or(d.LastLocationValid),
            LastLocation: rec
                .get_str("LastLocation")
                .map(|s| s.to_string())
                .unwrap_or(d.LastLocation),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut s = RecStruct::new();
        s.set_int("DurationMS", self.DurationMS);
        s.set_bool("Recursive", self.Recursive);
        s.set_bool("Loop", self.Loop);
        s.set_bool("LastLocationValid", self.LastLocationValid);
        s.set_str("LastLocation", &self.LastLocation);
        s
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}

//==============================================================================
//=========================== emAutoplayConfig =================================
//==============================================================================

/// Model wrapper for emAutoplayConfig.
///
/// Port of C++ `emAutoplayConfig` (extends emConfigModel). Backed by a
/// `emConfigModel` for file persistence at `emMain/autoplay.rec` under the
/// user config directory. Format name is `"emAutoplayConfig"`.
pub struct emAutoplayConfig {
    config_model: emConfigModel<emAutoplayConfigRec>,
}

impl emAutoplayConfig {
    /// Acquire the singleton `emAutoplayConfig` from the context registry.
    ///
    /// Port of C++ `emAutoplayConfig::Acquire`.
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<Self>("", || {
            let path =
                emGetInstallPath(InstallDirType::UserConfig, "emMain", Some("autoplay.rec"))
                    .unwrap_or_else(|_| {
                        let home =
                            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                        std::path::PathBuf::from(home)
                            .join(".eaglemode")
                            .join("emMain")
                            .join("autoplay.rec")
                    });

            let mut model = emConfigModel::new(
                emAutoplayConfigRec::default(),
                path,
                SignalId::null(),
            );

            if let Err(e) = model.TryLoadOrInstall() {
                log::warn!("AutoplayConfig: failed to load or install config: {e}");
            }

            Self {
                config_model: model,
            }
        })
    }

    pub fn GetFormatName(&self) -> &str {
        "emAutoplayConfig"
    }

    pub fn GetChangeSignal(&self) -> SignalId {
        self.config_model.GetChangeSignal()
    }

    pub fn GetDurationMS(&self) -> i32 {
        self.config_model.GetRec().DurationMS
    }

    pub fn SetDurationMS(&mut self, ms: i32) {
        self.config_model
            .modify(|d| d.DurationMS = ms.clamp(100, 600_000));
    }

    pub fn IsRecursive(&self) -> bool {
        self.config_model.GetRec().Recursive
    }

    pub fn SetRecursive(&mut self, recursive: bool) {
        self.config_model.modify(|d| d.Recursive = recursive);
    }

    pub fn IsLoop(&self) -> bool {
        self.config_model.GetRec().Loop
    }

    pub fn SetLoop(&mut self, lp: bool) {
        self.config_model.modify(|d| d.Loop = lp);
    }

    pub fn IsLastLocationValid(&self) -> bool {
        self.config_model.GetRec().LastLocationValid
    }

    pub fn SetLastLocationValid(&mut self, valid: bool) {
        self.config_model
            .modify(|d| d.LastLocationValid = valid);
    }

    pub fn GetLastLocation(&self) -> &str {
        // Safety: &str tied to lifetime of self, which holds the RefCell value.
        // We return from the borrowed record — the model is not mutably accessed
        // while this reference lives.
        let rec = self.config_model.GetRec();
        // DIVERGED: C++ returns const emString&; Rust borrows &str from the model.
        // We can't return a borrow of the inner data directly because GetRec() returns
        // a reference. We return an owned &str via a static-lifetime-alike trick:
        // actually, GetRec() returns &T so this is fine.
        &rec.LastLocation
    }

    pub fn SetLastLocation(&mut self, location: &str) {
        let s = location.to_string();
        self.config_model.modify(|d| d.LastLocation = s);
    }

    pub fn IsUnsaved(&self) -> bool {
        self.config_model.IsUnsaved()
    }
}

//==============================================================================
//========================= emAutoplayViewAnimator ============================
//==============================================================================

/// Autoplay navigation state machine states.
///
/// Port of C++ `emAutoplayViewAnimator` anonymous enum `State` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplayState {
    NoGoal,
    Unfinished,
    GivenUp,
    GoalReached,
}

/// Indicates where traversal came from when entering a panel.
///
/// Port of C++ `emAutoplayViewAnimator::CameFrom` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameFromType {
    None,
    Parent,
    Child,
}

/// Visit state of the current panel during traversal.
///
/// Port of C++ `emAutoplayViewAnimator::CurrentPanelState` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentPanelState {
    NotVisited,
    Visiting,
    Visited,
}

/// View animator for autoplay traversal of panel trees.
///
/// Port of C++ `emAutoplayViewAnimator`. Implements the state-machine structure
/// for navigating items in autoplay mode. Panel-dependent traversal methods
/// are stubbed pending the panel tree API.
pub struct emAutoplayViewAnimator {
    pub(crate) Recursive: bool,
    pub(crate) Loop: bool,
    pub(crate) State: AutoplayState,
    pub(crate) Backwards: bool,
    pub(crate) SkipItemCount: i32,
    pub(crate) SkipCurrent: bool,
    pub(crate) NextLoopEndless: bool,
    pub(crate) CameFrom: CameFromType,
    pub(crate) CameFromChildName: String,
    pub(crate) CurrentPanelIdentity: String,
    pub(crate) CurrentPanelState: CurrentPanelState,
    pub(crate) OneMoreWakeUp: bool,
}

impl emAutoplayViewAnimator {
    /// Construct a new animator in the idle (no-goal) state.
    ///
    /// Port of C++ `emAutoplayViewAnimator::emAutoplayViewAnimator`.
    pub fn new() -> Self {
        Self {
            Recursive: false,
            Loop: false,
            State: AutoplayState::NoGoal,
            Backwards: false,
            SkipItemCount: 0,
            SkipCurrent: false,
            NextLoopEndless: false,
            CameFrom: CameFromType::None,
            CameFromChildName: String::new(),
            CurrentPanelIdentity: String::new(),
            CurrentPanelState: CurrentPanelState::NotVisited,
            OneMoreWakeUp: false,
        }
    }

    pub fn IsRecursive(&self) -> bool {
        self.Recursive
    }

    pub fn SetRecursive(&mut self, recursive: bool) {
        self.Recursive = recursive;
    }

    pub fn IsLoop(&self) -> bool {
        self.Loop
    }

    pub fn SetLoop(&mut self, lp: bool) {
        self.Loop = lp;
    }

    /// Returns true if a goal has been set (state != NoGoal).
    pub fn HasGoal(&self) -> bool {
        self.State != AutoplayState::NoGoal
    }

    /// Returns true if the goal was successfully reached.
    pub fn HasReachedGoal(&self) -> bool {
        self.State == AutoplayState::GoalReached
    }

    /// Returns true if traversal gave up (e.g. no items found).
    pub fn HasGivenUp(&self) -> bool {
        self.State == AutoplayState::GivenUp
    }

    /// Returns the panel identity string of the current target panel.
    pub fn GetCurrentPanelIdentity(&self) -> &str {
        &self.CurrentPanelIdentity
    }

    /// Reset the animator to the no-goal idle state.
    ///
    /// Port of C++ `emAutoplayViewAnimator::ClearGoal`.
    pub fn ClearGoal(&mut self) {
        if self.State != AutoplayState::NoGoal {
            self.State = AutoplayState::NoGoal;
            self.OneMoreWakeUp = false;
            self.Backwards = false;
            self.SkipItemCount = 0;
            self.SkipCurrent = false;
            self.NextLoopEndless = false;
            self.CameFrom = CameFromType::None;
            self.CameFromChildName.clear();
            self.CurrentPanelIdentity.clear();
            self.CurrentPanelState = CurrentPanelState::NotVisited;
        }
    }

    /// Set goal to display the item at the given panel identity.
    ///
    /// Port of C++ `emAutoplayViewAnimator::SetGoalToItemAt(const emString&)`.
    /// Panel tree traversal is stubbed pending panel API availability.
    pub fn SetGoalToItemAt(&mut self, _panel_identity: &str) {
        log::warn!(
            "emAutoplayViewAnimator::SetGoalToItemAt: panel traversal not yet implemented"
        );
    }

    /// Set goal to the previous item relative to the given panel identity.
    ///
    /// Port of C++ `emAutoplayViewAnimator::SetGoalToPreviousItemOf`.
    /// Panel tree traversal is stubbed pending panel API availability.
    pub fn SetGoalToPreviousItemOf(&mut self, _panel_identity: &str) {
        log::warn!(
            "emAutoplayViewAnimator::SetGoalToPreviousItemOf: panel traversal not yet implemented"
        );
    }

    /// Set goal to the next item relative to the given panel identity.
    ///
    /// Port of C++ `emAutoplayViewAnimator::SetGoalToNextItemOf`.
    /// Panel tree traversal is stubbed pending panel API availability.
    pub fn SetGoalToNextItemOf(&mut self, _panel_identity: &str) {
        log::warn!(
            "emAutoplayViewAnimator::SetGoalToNextItemOf: panel traversal not yet implemented"
        );
    }

    /// Skip backwards to the previous item in the current traversal.
    ///
    /// Port of C++ `emAutoplayViewAnimator::SkipToPreviousItem`.
    /// Panel tree traversal is stubbed pending panel API availability.
    pub fn SkipToPreviousItem(&mut self) {
        log::warn!(
            "emAutoplayViewAnimator::SkipToPreviousItem: panel traversal not yet implemented"
        );
    }

    /// Skip forward to the next item in the current traversal.
    ///
    /// Port of C++ `emAutoplayViewAnimator::SkipToNextItem`.
    /// Panel tree traversal is stubbed pending panel API availability.
    pub fn SkipToNextItem(&mut self) {
        log::warn!(
            "emAutoplayViewAnimator::SkipToNextItem: panel traversal not yet implemented"
        );
    }

    //------------------------------------------------------------------
    // Traversal helpers (C++ emAutoplay.cpp:519-618)
    //------------------------------------------------------------------

    /// Static check: is a panel with the given focusability and flags an item?
    ///
    /// Port of the core logic in C++ `emAutoplayViewAnimator::IsItem`.
    pub fn is_item_check(focusable: bool, flags: AutoplayHandlingFlags) -> bool {
        focusable && flags.contains(AutoplayHandlingFlags::ITEM)
    }

    /// Check if a panel in the tree is an autoplay item.
    ///
    /// Port of C++ `emAutoplayViewAnimator::IsItem`.
    pub fn IsItem(tree: &PanelTree, panel: PanelId) -> bool {
        Self::is_item_check(tree.focusable(panel), tree.GetAutoplayHandling(panel))
    }

    /// Static check: is the current panel a cutoff point for traversal?
    ///
    /// Port of C++ `emAutoplayViewAnimator::IsCutoff`.
    pub fn is_cutoff_check(&self, flags: AutoplayHandlingFlags) -> bool {
        if flags.contains(AutoplayHandlingFlags::CUTOFF) {
            return true;
        }
        // The remaining checks require the panel to be focusable, but
        // this static helper doesn't have that info — it only fires on
        // CUTOFF. The tree version handles the rest.
        false
    }

    /// Check if a panel in the tree is a traversal cutoff point.
    ///
    /// Port of C++ `emAutoplayViewAnimator::IsCutoff`.
    pub fn IsCutoff(&self, tree: &PanelTree, panel: PanelId) -> bool {
        let f = tree.GetAutoplayHandling(panel);
        if f.contains(AutoplayHandlingFlags::CUTOFF) {
            return true;
        }
        if tree.focusable(panel) {
            if f.contains(AutoplayHandlingFlags::DIRECTORY) && !self.Recursive {
                return true;
            }
            if f.contains(AutoplayHandlingFlags::ITEM) {
                if !self.Recursive {
                    return true;
                }
                // Walk ancestors checking for CUTOFF_AT_SUBITEMS
                let mut q = tree.GetParentContext(panel);
                while let Some(qid) = q {
                    let qf = tree.GetAutoplayHandling(qid);
                    if qf.contains(AutoplayHandlingFlags::CUTOFF_AT_SUBITEMS) {
                        return true;
                    }
                    if tree.focusable(qid)
                        && qf.intersects(
                            AutoplayHandlingFlags::ITEM | AutoplayHandlingFlags::DIRECTORY,
                        )
                    {
                        break;
                    }
                    q = tree.GetParentContext(qid);
                }
            }
        }
        false
    }

    /// Navigate to the parent of the current panel.
    ///
    /// Port of C++ `emAutoplayViewAnimator::GoParent`.
    pub fn go_parent(&mut self, tree: &PanelTree, current: PanelId) {
        if let Some(parent) = tree.GetParentContext(current) {
            let child_name = tree
                .name(current)
                .unwrap_or("")
                .to_string();
            self.SkipCurrent = false;
            self.CameFrom = CameFromType::Child;
            self.CameFromChildName = child_name;
            self.CurrentPanelIdentity = tree.GetIdentity(parent);
            self.CurrentPanelState = CurrentPanelState::NotVisited;
        }
    }

    /// Navigate to a child panel.
    ///
    /// Port of C++ `emAutoplayViewAnimator::GoChild`.
    pub fn go_child(&mut self, tree: &PanelTree, child: PanelId) {
        self.SkipCurrent = false;
        self.CameFrom = CameFromType::Parent;
        self.CameFromChildName.clear();
        self.CurrentPanelIdentity = tree.GetIdentity(child);
        self.CurrentPanelState = CurrentPanelState::NotVisited;
    }

    /// Re-enter the current panel (reset skip flag).
    ///
    /// Port of C++ `emAutoplayViewAnimator::GoSame`.
    pub fn go_same(&mut self) {
        self.SkipCurrent = false;
    }

    /// Invert the traversal direction (forward ↔ backward).
    ///
    /// Port of C++ `emAutoplayViewAnimator::InvertDirection`.
    pub fn InvertDirection(&mut self) {
        self.Backwards = !self.Backwards;
        self.NextLoopEndless = false;

        if self.CameFrom == CameFromType::Parent {
            let names = DecodeIdentity(&self.CurrentPanelIdentity);
            let cnt = names.len();
            if cnt > 0 {
                self.CameFrom = CameFromType::Child;
                self.CameFromChildName = names[cnt - 1].clone();
                let parent_names: Vec<&str> =
                    names[..cnt - 1].iter().map(|s| s.as_str()).collect();
                self.CurrentPanelIdentity = EncodeIdentity(&parent_names);
                self.CurrentPanelState = CurrentPanelState::NotVisited;
            }
        } else if self.CameFrom == CameFromType::Child {
            let mut names = DecodeIdentity(&self.CurrentPanelIdentity);
            names.push(self.CameFromChildName.clone());
            self.CameFrom = CameFromType::Parent;
            self.CameFromChildName.clear();
            let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            self.CurrentPanelIdentity = EncodeIdentity(&name_refs);
            self.CurrentPanelState = CurrentPanelState::NotVisited;
        }
    }
}

impl Default for emAutoplayViewAnimator {
    fn default() -> Self {
        Self::new()
    }
}

//==============================================================================
//========================== emAutoplayViewModel ==============================
//==============================================================================

/// View model for the autoplay UI state.
///
/// Port of C++ `emAutoplayViewModel`. Holds the observable autoplay UI state
/// used by control panels to display and modify playback settings.
pub struct emAutoplayViewModel {
    pub(crate) DurationMS: i32,
    pub(crate) Recursive: bool,
    pub(crate) Loop: bool,
    pub(crate) Autoplaying: bool,
    pub(crate) LastLocationValid: bool,
    pub(crate) LastLocation: String,
    pub(crate) ItemProgress: f64,
    pub(crate) PlayingItem: bool,
    pub(crate) PlaybackActive: bool,
}

impl emAutoplayViewModel {
    /// Construct a new view model with default values.
    pub fn new() -> Self {
        Self {
            DurationMS: 5000,
            Recursive: false,
            Loop: false,
            Autoplaying: false,
            LastLocationValid: false,
            LastLocation: String::new(),
            ItemProgress: 0.0,
            PlayingItem: false,
            PlaybackActive: false,
        }
    }

    pub fn GetDurationMS(&self) -> i32 {
        self.DurationMS
    }

    pub fn SetDurationMS(&mut self, ms: i32) {
        self.DurationMS = ms.clamp(100, 600_000);
    }

    pub fn IsRecursive(&self) -> bool {
        self.Recursive
    }

    pub fn SetRecursive(&mut self, recursive: bool) {
        self.Recursive = recursive;
    }

    pub fn IsLoop(&self) -> bool {
        self.Loop
    }

    pub fn SetLoop(&mut self, lp: bool) {
        self.Loop = lp;
    }

    pub fn IsAutoplaying(&self) -> bool {
        self.Autoplaying
    }

    pub fn SetAutoplaying(&mut self, autoplaying: bool) {
        self.Autoplaying = autoplaying;
    }

    /// Returns the fractional progress through the current item (0.0..=1.0).
    pub fn GetItemProgress(&self) -> f64 {
        self.ItemProgress
    }

    pub fn IsLastLocationValid(&self) -> bool {
        self.LastLocationValid
    }

    pub fn SetLastLocationValid(&mut self, valid: bool) {
        self.LastLocationValid = valid;
    }

    pub fn GetLastLocation(&self) -> &str {
        &self.LastLocation
    }

    pub fn SetLastLocation(&mut self, location: &str) {
        self.LastLocation = location.to_string();
    }

    pub fn IsPlayingItem(&self) -> bool {
        self.PlayingItem
    }

    pub fn IsPlaybackActive(&self) -> bool {
        self.PlaybackActive
    }
}

impl Default for emAutoplayViewModel {
    fn default() -> Self {
        Self::new()
    }
}

//==============================================================================
//================================ Tests =======================================
//==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autoplay_config_defaults() {
        let config = emAutoplayConfigRec::default();
        assert_eq!(config.DurationMS, 5000);
        assert!(!config.Recursive);
        assert!(!config.Loop);
        assert!(!config.LastLocationValid);
        assert_eq!(config.LastLocation, "");
    }

    #[test]
    fn test_autoplay_config_round_trip() {
        let mut config = emAutoplayConfigRec::default();
        config.DurationMS = 3000;
        config.Recursive = true;
        let rec = config.to_rec();
        let loaded = emAutoplayConfigRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.DurationMS, 3000);
        assert!(loaded.Recursive);
    }

    #[test]
    fn test_autoplay_config_clamp_duration() {
        let mut rec = RecStruct::new();
        rec.set_int("DurationMS", 50); // below min 100
        let config = emAutoplayConfigRec::from_rec(&rec).unwrap();
        assert_eq!(config.DurationMS, 100);
    }

    #[test]
    fn test_view_animator_initial_state() {
        let va = emAutoplayViewAnimator::new();
        assert_eq!(va.State, AutoplayState::NoGoal);
        assert!(!va.Recursive);
        assert!(!va.Loop);
        assert!(!va.HasGoal());
    }

    #[test]
    fn test_view_animator_clear_goal() {
        let mut va = emAutoplayViewAnimator::new();
        va.State = AutoplayState::Unfinished;
        va.ClearGoal();
        assert_eq!(va.State, AutoplayState::NoGoal);
    }

    #[test]
    fn test_view_model_defaults() {
        let vm = emAutoplayViewModel::new();
        assert_eq!(vm.GetDurationMS(), 5000);
        assert!(!vm.IsAutoplaying());
        assert!((vm.GetItemProgress()).abs() < 1e-10);
    }

    #[test]
    fn test_autoplay_config_set_to_default() {
        let mut config = emAutoplayConfigRec::default();
        config.DurationMS = 3000;
        config.Recursive = true;
        assert!(!config.IsSetToDefault());
        config.SetToDefault();
        assert!(config.IsSetToDefault());
    }

    #[test]
    fn test_view_model_setters() {
        let mut vm = emAutoplayViewModel::new();
        vm.SetDurationMS(2000);
        vm.SetRecursive(true);
        vm.SetLoop(true);
        vm.SetAutoplaying(true);
        assert_eq!(vm.GetDurationMS(), 2000);
        assert!(vm.IsRecursive());
        assert!(vm.IsLoop());
        assert!(vm.IsAutoplaying());
    }

    #[test]
    fn test_view_model_clamp_duration() {
        let mut vm = emAutoplayViewModel::new();
        vm.SetDurationMS(50); // below min
        assert_eq!(vm.GetDurationMS(), 100);
        vm.SetDurationMS(700_000); // above max
        assert_eq!(vm.GetDurationMS(), 600_000);
    }

    // ── Traversal helper tests ─────────────────────────────────────────

    #[test]
    fn test_is_item_check() {
        assert!(emAutoplayViewAnimator::is_item_check(
            true,
            AutoplayHandlingFlags::ITEM
        ));
        assert!(!emAutoplayViewAnimator::is_item_check(
            false,
            AutoplayHandlingFlags::ITEM
        ));
        assert!(!emAutoplayViewAnimator::is_item_check(
            true,
            AutoplayHandlingFlags::DIRECTORY
        ));
        assert!(!emAutoplayViewAnimator::is_item_check(
            true,
            AutoplayHandlingFlags::empty()
        ));
        // ITEM combined with others still counts
        assert!(emAutoplayViewAnimator::is_item_check(
            true,
            AutoplayHandlingFlags::ITEM | AutoplayHandlingFlags::DIRECTORY
        ));
    }

    #[test]
    fn test_is_item_with_tree() {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let child = tree.create_child(root, "child");
        tree.set_focusable(child, true);
        tree.SetAutoplayHandling(child, AutoplayHandlingFlags::ITEM);
        assert!(emAutoplayViewAnimator::IsItem(&tree, child));

        // Not focusable → not an item (use child since root can't be unfocusable)
        tree.set_focusable(child, false);
        assert!(!emAutoplayViewAnimator::IsItem(&tree, child));
    }

    #[test]
    fn test_is_cutoff_check_cutoff_flag() {
        let va = emAutoplayViewAnimator::new();
        assert!(va.is_cutoff_check(AutoplayHandlingFlags::CUTOFF));
        assert!(!va.is_cutoff_check(AutoplayHandlingFlags::empty()));
        assert!(!va.is_cutoff_check(AutoplayHandlingFlags::ITEM));
    }

    #[test]
    fn test_is_cutoff_with_tree_cutoff_flag() {
        let va = emAutoplayViewAnimator::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.SetAutoplayHandling(root, AutoplayHandlingFlags::CUTOFF);
        assert!(va.IsCutoff(&tree, root));
    }

    #[test]
    fn test_is_cutoff_directory_non_recursive() {
        let va = emAutoplayViewAnimator::new(); // Recursive=false
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.SetAutoplayHandling(root, AutoplayHandlingFlags::DIRECTORY);
        assert!(va.IsCutoff(&tree, root));
    }

    #[test]
    fn test_is_cutoff_directory_recursive() {
        let mut va = emAutoplayViewAnimator::new();
        va.Recursive = true;
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.SetAutoplayHandling(root, AutoplayHandlingFlags::DIRECTORY);
        assert!(!va.IsCutoff(&tree, root));
    }

    #[test]
    fn test_is_cutoff_item_non_recursive() {
        let va = emAutoplayViewAnimator::new(); // Recursive=false
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.SetAutoplayHandling(root, AutoplayHandlingFlags::ITEM);
        assert!(va.IsCutoff(&tree, root));
    }

    #[test]
    fn test_is_cutoff_item_recursive_with_cutoff_at_subitems() {
        let mut va = emAutoplayViewAnimator::new();
        va.Recursive = true;
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.SetAutoplayHandling(root, AutoplayHandlingFlags::CUTOFF_AT_SUBITEMS);

        let child = tree.create_child(root, "child");
        tree.set_focusable(child, true);
        tree.SetAutoplayHandling(child, AutoplayHandlingFlags::ITEM);
        // child is an ITEM, recursive, parent has CUTOFF_AT_SUBITEMS → cutoff
        assert!(va.IsCutoff(&tree, child));
    }

    #[test]
    fn test_is_cutoff_item_recursive_no_cutoff_at_subitems() {
        let mut va = emAutoplayViewAnimator::new();
        va.Recursive = true;
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        tree.set_focusable(root, true);
        tree.SetAutoplayHandling(root, AutoplayHandlingFlags::DIRECTORY);

        let child = tree.create_child(root, "child");
        tree.set_focusable(child, true);
        tree.SetAutoplayHandling(child, AutoplayHandlingFlags::ITEM);
        // Parent is DIRECTORY+focusable → stops ancestor walk, no cutoff
        assert!(!va.IsCutoff(&tree, child));
    }

    #[test]
    fn test_go_parent() {
        let mut va = emAutoplayViewAnimator::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let child = tree.create_child(root, "child");

        va.go_parent(&tree, child);
        assert_eq!(va.CameFrom, CameFromType::Child);
        assert_eq!(va.CameFromChildName, "child");
        assert_eq!(va.CurrentPanelIdentity, tree.GetIdentity(root));
        assert_eq!(va.CurrentPanelState, CurrentPanelState::NotVisited);
        assert!(!va.SkipCurrent);
    }

    #[test]
    fn test_go_child() {
        let mut va = emAutoplayViewAnimator::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("root");
        let child = tree.create_child(root, "child");

        va.go_child(&tree, child);
        assert_eq!(va.CameFrom, CameFromType::Parent);
        assert!(va.CameFromChildName.is_empty());
        assert_eq!(va.CurrentPanelIdentity, tree.GetIdentity(child));
        assert_eq!(va.CurrentPanelState, CurrentPanelState::NotVisited);
    }

    #[test]
    fn test_go_same() {
        let mut va = emAutoplayViewAnimator::new();
        va.SkipCurrent = true;
        va.go_same();
        assert!(!va.SkipCurrent);
    }

    #[test]
    fn test_invert_direction_from_parent() {
        let mut va = emAutoplayViewAnimator::new();
        va.CameFrom = CameFromType::Parent;
        va.CurrentPanelIdentity = "root:child".to_string();
        va.Backwards = false;

        va.InvertDirection();

        assert!(va.Backwards);
        assert_eq!(va.CameFrom, CameFromType::Child);
        assert_eq!(va.CameFromChildName, "child");
        assert_eq!(va.CurrentPanelIdentity, "root");
        assert_eq!(va.CurrentPanelState, CurrentPanelState::NotVisited);
    }

    #[test]
    fn test_invert_direction_from_child() {
        let mut va = emAutoplayViewAnimator::new();
        va.CameFrom = CameFromType::Child;
        va.CameFromChildName = "child".to_string();
        va.CurrentPanelIdentity = "root".to_string();
        va.Backwards = true;

        va.InvertDirection();

        assert!(!va.Backwards);
        assert_eq!(va.CameFrom, CameFromType::Parent);
        assert!(va.CameFromChildName.is_empty());
        assert_eq!(va.CurrentPanelIdentity, "root:child");
        assert_eq!(va.CurrentPanelState, CurrentPanelState::NotVisited);
    }

    #[test]
    fn test_invert_direction_from_none() {
        let mut va = emAutoplayViewAnimator::new();
        va.CameFrom = CameFromType::None;
        va.Backwards = false;

        va.InvertDirection();

        assert!(va.Backwards);
        assert_eq!(va.CameFrom, CameFromType::None);
        assert!(!va.NextLoopEndless);
    }

    #[test]
    fn test_invert_direction_clears_next_loop_endless() {
        let mut va = emAutoplayViewAnimator::new();
        va.NextLoopEndless = true;
        va.CameFrom = CameFromType::None;

        va.InvertDirection();

        assert!(!va.NextLoopEndless);
    }
}
