# Main App Launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Get the `eaglemode` binary running with full C++ emMain fidelity, using ported plugins (emStocks, emFileMan).

**Architecture:** New `crates/emmain/` crate ports 10 C++ emMain files 1:1. emFileMan's 4 deferred panel lifecycle methods get wired. Static plugin registry bridges plugins without dynamic loading. Binary entry point handles IPC single-instance and boots the full panel tree.

**Tech Stack:** Rust, emcore (winit/wgpu), emstocks, emfileman. Config via `#%rec:...%#` format. IPC via Unix FIFOs (emMiniIpc).

**Spec:** `docs/superpowers/specs/2026-03-31-main-app-launch-design.md`
**C++ Reference:** `~/git/eaglemode-0.96.4/src/emMain/` and `~/git/eaglemode-0.96.4/include/emMain/`

---

## Phase 1: Foundation (emmain crate + config + static registry)

### Task 1: Create emmain crate skeleton

**Files:**
- Create: `crates/emmain/Cargo.toml`
- Create: `crates/emmain/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "emmain"
version = "0.1.0"
edition = "2024"

[dependencies]
emcore = { path = "../emcore" }
emstocks = { path = "../emstocks" }
emfileman = { path = "../emfileman" }
log = { workspace = true }
```

- [ ] **Step 2: Create lib.rs with module stubs**

```rust
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

pub mod emMainConfig;
```

- [ ] **Step 3: Add to workspace Cargo.toml**

Add `"crates/emmain"` to the `members` list in the root `Cargo.toml`.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p emmain`
Expected: success (empty crate with one module stub)

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/ Cargo.toml
git commit -m "feat(emmain): create crate skeleton"
```

---

### Task 2: emMainConfig — config record

**Files:**
- Create: `crates/emmain/src/emMainConfig.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emMain/emMainConfig.cpp` and `~/git/eaglemode-0.96.4/include/emMain/emMainConfig.h`

The C++ `emMainConfig` is an `emConfigModel` with 3 fields: `AutoHideControlView` (bool, default false), `AutoHideSlider` (bool, default false), `ControlViewSize` (double, range 0.0..1.0, default 0.515). Format name `"emMainConfig"`.

- [ ] **Step 1: Write test for emMainConfig record round-trip**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRec::{Record, RecStruct};

    #[test]
    fn test_defaults() {
        let config = emMainConfigRec::default();
        assert!(!config.AutoHideControlView);
        assert!(!config.AutoHideSlider);
        assert!((config.ControlViewSize - 0.515).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip() {
        let mut config = emMainConfigRec::default();
        config.AutoHideControlView = true;
        config.ControlViewSize = 0.75;
        let rec = config.to_rec();
        let loaded = emMainConfigRec::from_rec(&rec).unwrap();
        assert!(loaded.AutoHideControlView);
        assert!(!loaded.AutoHideSlider);
        assert!((loaded.ControlViewSize - 0.75).abs() < 1e-10);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo-nextest ntr -p emmain`
Expected: FAIL — `emMainConfigRec` not defined

- [ ] **Step 3: Implement emMainConfigRec**

Read the C++ source at `~/git/eaglemode-0.96.4/include/emMain/emMainConfig.h` and `~/git/eaglemode-0.96.4/src/emMain/emMainConfig.cpp`. Port the struct and `Record` impl. Key details:

- Format name: `"emMainConfig"`
- Field `ControlViewSize` is clamped to `0.0..=1.0`
- `emConfigModel<emMainConfigRec>` wraps this for file I/O at path `emGetConfigDirOverloadable("emMain", None)` + `/config.rec`

```rust
use emcore::emRec::{Record, RecError, RecStruct};

pub struct emMainConfigRec {
    pub AutoHideControlView: bool,
    pub AutoHideSlider: bool,
    pub ControlViewSize: f64,
}

impl Default for emMainConfigRec {
    fn default() -> Self {
        Self {
            AutoHideControlView: false,
            AutoHideSlider: false,
            ControlViewSize: 0.515,
        }
    }
}

impl Record for emMainConfigRec {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
        Ok(Self {
            AutoHideControlView: rec.get_bool("AutoHideControlView").unwrap_or(false),
            AutoHideSlider: rec.get_bool("AutoHideSlider").unwrap_or(false),
            ControlViewSize: rec.get_double("ControlViewSize")
                .unwrap_or(0.515)
                .clamp(0.0, 1.0),
        })
    }

    fn to_rec(&self) -> RecStruct {
        let mut rec = RecStruct::new();
        rec.set_bool("AutoHideControlView", self.AutoHideControlView);
        rec.set_bool("AutoHideSlider", self.AutoHideSlider);
        rec.set_double("ControlViewSize", self.ControlViewSize);
        rec
    }

    fn SetToDefault(&mut self) {
        *self = Self::default();
    }

    fn IsSetToDefault(&self) -> bool {
        *self == Self::default()
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emMainConfig.rs crates/emmain/src/lib.rs
git commit -m "feat(emMainConfig): port config record with 3 fields"
```

---

### Task 3: Static plugin registry

**Files:**
- Create: `crates/emmain/src/static_plugins.rs`
- Modify: `crates/emmain/src/lib.rs`
- Modify: `crates/emcore/src/emFpPlugin.rs` (add static registry hook)

**Context:** The C++ version discovers plugins via `.emFpPlugin` files and loads `.so` libraries dynamically. For the initial launch we statically link all plugins. `emFpPlugin::TryCreateFilePanel()` needs a fallback path that checks a static registry before attempting `emTryResolveSymbol`.

- [ ] **Step 1: Create static_plugins.rs**

Read `crates/emcore/src/emFpPlugin.rs` to find the exact `emFpPluginFunc` type signature, then write the registry:

```rust
use emcore::emFpPlugin::emFpPluginFunc;

/// Static plugin function registry for statically linked plugins.
/// Falls back to this when dynamic symbol resolution fails.
pub fn resolve_static_plugin(function_name: &str) -> Option<emFpPluginFunc> {
    match function_name {
        "emDirFpPluginFunc" => Some(emfileman::emDirFpPluginFunc),
        "emStocksFpPluginFunc" => Some(emstocks::emStocksFpPluginFunc),
        "emDirStatFpPluginFunc" => Some(emfileman::emDirStatFpPluginFunc),
        "emFileLinkFpPluginFunc" => Some(emfileman::emFileLinkFpPluginFunc),
        _ => None,
    }
}
```

Note: Check if `emFileManFpPluginFunc` exists in emfileman — if so, add it too.

- [ ] **Step 2: Add static resolver hook to emFpPlugin**

Read `emFpPlugin.rs` and find `TryCreateFilePanel()`. Add a `pub static STATIC_PLUGIN_RESOLVER` or a setter function so emmain can register its static resolver at startup. The resolver is called when `emTryResolveSymbol` fails.

The exact approach depends on the current code structure — read the function, identify where dynamic loading happens, and add the fallback. Pattern:

```rust
// In emFpPlugin.rs, add:
use std::cell::RefCell;

thread_local! {
    static STATIC_RESOLVER: RefCell<Option<fn(&str) -> Option<emFpPluginFunc>>> =
        RefCell::new(None);
}

pub fn set_static_plugin_resolver(resolver: fn(&str) -> Option<emFpPluginFunc>) {
    STATIC_RESOLVER.with(|r| *r.borrow_mut() = Some(resolver));
}

// In TryCreateFilePanel, after dynamic resolution fails:
// if let Some(resolver) = STATIC_RESOLVER.with(|r| *r.borrow()) {
//     if let Some(func) = resolver(&self.function) {
//         return func(parent, name, path, self, error_buf);
//     }
// }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p emmain`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add crates/emmain/src/static_plugins.rs crates/emmain/src/lib.rs crates/emcore/src/emFpPlugin.rs
git commit -m "feat(emmain): static plugin registry with emFpPlugin fallback hook"
```

---

## Phase 2: Data Models (bookmarks, autoplay, cosmos model)

### Task 4: emBookmarks — record types and model

**Files:**
- Create: `crates/emmain/src/emBookmarks.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emBookmarks.h` and `~/git/eaglemode-0.96.4/src/emMain/emBookmarks.cpp`

The bookmark system has: `emBookmarkRec` (a single bookmark with location), `emBookmarkGroupRec` (a group containing bookmarks/subgroups), `emBookmarksRec` (the root, an array of bookmark/group unions), and `emBookmarksModel` (config model that loads/saves the file).

- [ ] **Step 1: Write test for bookmark record round-trip**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRec::{Record, RecStruct, RecValue};

    #[test]
    fn test_bookmark_rec_defaults() {
        let bm = emBookmarkRec::default();
        assert_eq!(bm.Name, "");
        assert_eq!(bm.LocationIdentity, "");
        assert!((bm.LocationRelX - 0.5).abs() < 1e-10);
        assert!((bm.LocationRelY - 0.5).abs() < 1e-10);
        assert!((bm.LocationRelA - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_bookmark_rec_round_trip() {
        let mut bm = emBookmarkRec::default();
        bm.Name = "Home".to_string();
        bm.LocationIdentity = "::VcItem:Home:".to_string();
        bm.Hotkey = "F5".to_string();
        let rec = bm.to_rec();
        let loaded = emBookmarkRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.Name, "Home");
        assert_eq!(loaded.LocationIdentity, "::VcItem:Home:");
        assert_eq!(loaded.Hotkey, "F5");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo-nextest ntr -p emmain`
Expected: FAIL

- [ ] **Step 3: Implement bookmark record types**

Read the C++ header and source thoroughly. Port:

- `emBookmarkRec`: Name, Description, Icon, Hotkey, LocationIdentity, LocationRelX (f64), LocationRelY (f64), LocationRelA (f64), BackgroundColor (emColor), TextColor (emColor)
- `emBookmarkGroupRec`: Name, Description, Icon, BackgroundColor, TextColor, Bookmarks (Vec of union entries)
- `emBookmarkEntryUnion`: enum with `Bookmark(emBookmarkRec)` and `Group(emBookmarkGroupRec)` variants
- `emBookmarksRec`: Vec of `emBookmarkEntryUnion` — the root record
- `emBookmarksModel`: wraps `emConfigModel<emBookmarksRec>`, loads from `~/.eaglemode/emMain/Bookmarks.emBookmarks` with format name `"emBookmarks"`

Follow the exact C++ field names. The union uses the C++ `emUnionRec` pattern — in `emRec` this maps to `RecValue::Union(variant_name, inner)`.

- [ ] **Step 4: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emBookmarks.rs crates/emmain/src/lib.rs
git commit -m "feat(emBookmarks): port bookmark records, group hierarchy, and model"
```

---

### Task 5: emAutoplay — config and view animator

**Files:**
- Create: `crates/emmain/src/emAutoplay.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emAutoplay.h` and `~/git/eaglemode-0.96.4/src/emMain/emAutoplay.cpp`

- [ ] **Step 1: Write test for autoplay config**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRec::{Record, RecStruct};

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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo-nextest ntr -p emmain`
Expected: FAIL

- [ ] **Step 3: Implement emAutoplay**

Read the C++ source. Port:

- `emAutoplayConfigRec`: DurationMS (i32, default 5000), Recursive (bool), Loop (bool), LastLocationValid (bool), LastLocation (String)
- `emAutoplayViewModel`: a view animator that traverses panels. State machine with states: `NoGoal`, `Seeking`, `Viewing`, `Done`. Uses `emVisitingViewAnimator` to drive navigation between child panels.

The view animator is the complex part. Read the C++ `emAutoplayViewAnimator` class carefully — it tracks `CameFrom` direction, `CurrentPanelState`, `SkipItemCount`, and drives the visiting animator. Port the state machine faithfully.

- [ ] **Step 4: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emAutoplay.rs crates/emmain/src/lib.rs
git commit -m "feat(emAutoplay): port autoplay config and view animator"
```

---

### Task 6: emVirtualCosmos — model and item record

**Files:**
- Create: `crates/emmain/src/emVirtualCosmos.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emVirtualCosmos.h` and `~/git/eaglemode-0.96.4/src/emMain/emVirtualCosmos.cpp`

This task ports just the data model (`emVirtualCosmosItemRec` and `emVirtualCosmosModel`). The panel classes come in Phase 4.

- [ ] **Step 1: Write test for item record parsing**

Create a test `.emVcItem` file in a temp dir and verify the model loads it:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use emcore::emRec::{Record, RecStruct};

    #[test]
    fn test_item_rec_defaults() {
        let item = emVirtualCosmosItemRec::default();
        assert_eq!(item.Title, "");
        assert!((item.PosX).abs() < 1e-10);
        assert!((item.PosY).abs() < 1e-10);
        assert!((item.Width - 0.1).abs() < 1e-10);
        assert!((item.ContentTallness - 1.0).abs() < 1e-10);
        assert!((item.BorderScaling - 1.0).abs() < 1e-10);
        assert!(item.Focusable);
        assert_eq!(item.FileName, "unnamed");
        assert!(!item.CopyToUser);
        assert_eq!(item.Alternative, 0);
    }

    #[test]
    fn test_item_rec_round_trip() {
        let mut item = emVirtualCosmosItemRec::default();
        item.Title = "Home".to_string();
        item.PosX = 0.5;
        item.PosY = 0.3;
        item.Width = 0.2;
        item.FileName = "Home".to_string();
        let rec = item.to_rec();
        let loaded = emVirtualCosmosItemRec::from_rec(&rec).unwrap();
        assert_eq!(loaded.Title, "Home");
        assert!((loaded.PosX - 0.5).abs() < 1e-10);
        assert_eq!(loaded.FileName, "Home");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo-nextest ntr -p emmain`
Expected: FAIL

- [ ] **Step 3: Implement emVirtualCosmosItemRec and emVirtualCosmosModel**

Read the C++ source. Port:

**emVirtualCosmosItemRec fields** (exact C++ names):
- Title (String), PosX (f64), PosY (f64), Width (f64, range 1e-10..1.0, default 0.1)
- ContentTallness (f64, range 1e-10..1e10, default 1.0), BorderScaling (f64, range 0.0..1e10, default 1.0)
- BackgroundColor (emColor, default 0xAAAAAAFF), BorderColor (emColor, default 0xAAAAAAFF), TitleColor (emColor, default 0x000000FF)
- Focusable (bool, default true), FileName (String, default "unnamed"), CopyToUser (bool, default false), Alternative (i32, range 0..i32::MAX, default 0)

**emVirtualCosmosModel**:
- Loads all `.emVcItem` files from `emGetConfigDirOverloadable("emMain", Some("VcItems"))`
- Also loads from a `VcItemFiles` dir for the actual content files referenced by `FileName`
- Tracks modification times, reloads on change
- Stores items as `Vec<LoadedItem>` where `LoadedItem` holds the record + file path + mtime
- Provides a change signal
- `Reload()` method rescans the directory

- [ ] **Step 4: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emVirtualCosmos.rs crates/emmain/src/lib.rs
git commit -m "feat(emVirtualCosmos): port item record and model with directory scanning"
```

---

## Phase 3: emFileMan Completion

### Task 7: emDirPanel model wiring refinement

**Files:**
- Modify: `crates/emfileman/src/emDirPanel.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emFileMan/emDirPanel.cpp`

The current Rust `Cycle()` manually drives `emDirModel` loading. Read the current implementation and the C++ reference. The main gap is ensuring the busy return value propagates correctly and config change signals trigger `InvalidatePainting` + `UpdateChildren` + `InvalidateChildrenLayout`.

- [ ] **Step 1: Read current implementation**

Read `crates/emfileman/src/emDirPanel.rs` fully. Identify the exact `Cycle()` and `notice()` methods. Compare with C++ `emDirPanel::Cycle()` (lines 71-88) and `Notice()` (lines 91-113).

- [ ] **Step 2: Wire config change signal in Cycle**

The C++ `Cycle()` checks `IsSignaled(Config->GetChangeSignal())` and calls `InvalidatePainting()` + `UpdateChildren()` + `InvalidateChildrenLayout()`. Ensure the Rust version does the same — if it doesn't react to config changes (sort criteria, theme, etc.), add it.

- [ ] **Step 3: Ensure busy state propagation**

The C++ returns `emFilePanel::Cycle()` busy state. The Rust version should return `true` from `Cycle()` while loading is in progress (state is `Loading`), `false` otherwise.

- [ ] **Step 4: Run tests**

Run: `cargo-nextest ntr -p emfileman`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/emfileman/src/emDirPanel.rs
git commit -m "fix(emDirPanel): wire config change signal and busy state in Cycle"
```

---

### Task 8: emDirEntryPanel — UpdateContentPanel wiring

**Files:**
- Modify: `crates/emfileman/src/emDirEntryPanel.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp` lines 728-840

The Rust `update_content_panel()` exists at lines ~146-193 and is called from `LayoutChildren()` via dirty flags. Read both the current Rust and the C++ reference.

- [ ] **Step 1: Read current implementation and compare with C++**

Read `crates/emfileman/src/emDirEntryPanel.rs` lines 140-200. Compare with C++ `UpdateContentPanel`. Key differences to check:
- Does Rust use theme content coordinates (DirContentX/Y/W/H vs FileContentX/Y/W/H)?
- Does Rust check `MinContentVW` threshold correctly?
- Does Rust call `emFpPluginList::CreateFilePanel` or `CreateFilePanelWithStat`?
- Does Rust handle the destroy condition (not in active/viewed path)?

- [ ] **Step 2: Fix any gaps found**

Common gaps based on investigation:
1. The C++ checks `GetSoughtName()` to create panels being navigated to even when not viewed. If `PanelCtx` doesn't have `GetSoughtName()`, add a `sought_name` field or skip this (document as DIVERGED).
2. The destroy condition should check `!IsInActivePath() && (!IsInViewedPath() || IsViewed())`.
3. Ensure `BeFirst()` is called on the created panel (so it renders behind the info overlay).

- [ ] **Step 3: Run tests**

Run: `cargo-nextest ntr -p emfileman`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/emfileman/src/emDirEntryPanel.rs
git commit -m "fix(emDirEntryPanel): complete UpdateContentPanel with destroy condition and theme coords"
```

---

### Task 9: emDirEntryPanel — UpdateAltPanel wiring

**Files:**
- Modify: `crates/emfileman/src/emDirEntryPanel.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp` lines 842-900

- [ ] **Step 1: Read current implementation**

Read lines ~197-227 of emDirEntryPanel.rs. The alt panel shows `emDirEntryAltPanel` with `Alternative=1` when zoomed enough (`AltW * viewed_width >= MinAltVW`).

- [ ] **Step 2: Fix any gaps**

Compare with C++. Key details:
- Alt panel uses theme coordinates: `AltX`, `AltY`, `AltW`, `AltH`
- Threshold: `GetViewedWidth() * theme->AltW >= theme->MinAltVW`
- Creates `emDirEntryAltPanel` with `Alternative=1`
- Same destroy condition as content panel
- Layout at `(AltX, AltY, AltW, AltH)` with `BgColor`

- [ ] **Step 3: Run tests**

Run: `cargo-nextest ntr -p emfileman`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/emfileman/src/emDirEntryPanel.rs
git commit -m "fix(emDirEntryPanel): complete UpdateAltPanel with theme coordinates"
```

---

### Task 10: emFileLinkPanel — UpdateDataAndChildPanel wiring

**Files:**
- Modify: `crates/emfileman/src/emFileLinkPanel.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emFileMan/emFileLinkPanel.cpp` lines 234-305

- [ ] **Step 1: Read current implementation**

Read lines ~91-132 of emFileLinkPanel.rs. Compare with C++ `UpdateDataAndChildPanel`.

- [ ] **Step 2: Fix gaps**

Key differences found in investigation:
1. Add `ViewCondition` check — C++ uses `GetViewCondition() >= 60.0` as creation threshold, not just `viewed` boolean. Check if `PanelState` provides view condition or compute from `viewed_rect`.
2. Add `IsVFSGood()` check — only read model when virtual file state is good
3. Add `DirEntryUpToDate` tracking flag — avoid reconstructing `emDirEntry` on every cycle
4. Add `UpdateDirEntry()` call on existing child when dir entry changes (instead of recreating)

- [ ] **Step 3: Run tests**

Run: `cargo-nextest ntr -p emfileman`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/emfileman/src/emFileLinkPanel.rs
git commit -m "fix(emFileLinkPanel): complete UpdateDataAndChildPanel with view condition and VFS checks"
```

---

### Task 11: emDirEntryPanel — shift-click range selection

**Files:**
- Modify: `crates/emfileman/src/emDirEntryPanel.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emFileMan/emDirEntryPanel.cpp` — Input handler, selection with Shift modifier

- [ ] **Step 1: Read current Input implementation**

Read the `Input()` method. Currently Ctrl+click toggles selection, regular click does SelectSolely. Shift+click should select all siblings between the last-selected entry and this one.

- [ ] **Step 2: Implement range selection**

The approach: when Shift is held during click:
1. Find the "anchor" entry (last selected entry in the parent `emDirPanel`)
2. Enumerate sibling panels from the parent via `PanelCtx`
3. Select all entries between anchor and clicked entry

Check what API `PanelCtx` provides for sibling enumeration. If the parent panel tracks `last_selected_index`, use that. Otherwise, store it as a field on `emDirPanel`.

Read the C++ implementation to see exactly how it finds the range. Port that logic.

- [ ] **Step 3: Run tests**

Run: `cargo-nextest ntr -p emfileman`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/emfileman/src/emDirEntryPanel.rs crates/emfileman/src/emDirPanel.rs
git commit -m "feat(emDirEntryPanel): implement shift-click range selection"
```

---

## Phase 4: Leaf Panels (starfield, content panel, cosmos panels)

**Gate:** Phase 2 tasks (4-6) must be complete. Phase 3 tasks (7-11) should be complete but are not strictly blocking — cosmos panels don't depend on emFileMan.

### Task 12: emStarFieldPanel — fractal starfield

**Files:**
- Create: `crates/emmain/src/emStarFieldPanel.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emStarFieldPanel.h` and `~/git/eaglemode-0.96.4/src/emMain/emStarFieldPanel.cpp`

- [ ] **Step 1: Write test for deterministic star generation**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_star_count_depth_0() {
        let panel = emStarFieldPanel::new(0, 0x12345678);
        assert_eq!(panel.stars.len(), 0);
    }

    #[test]
    fn test_star_count_depth_1() {
        let panel = emStarFieldPanel::new(1, 0x12345678);
        // Depth 1: count = min(1*3, 400) * random(0.5..1.0) = 1..3
        assert!(panel.stars.len() >= 1 && panel.stars.len() <= 3);
    }

    #[test]
    fn test_deterministic_stars() {
        let p1 = emStarFieldPanel::new(5, 0xABCDABCD);
        let p2 = emStarFieldPanel::new(5, 0xABCDABCD);
        assert_eq!(p1.stars.len(), p2.stars.len());
        for (a, b) in p1.stars.iter().zip(p2.stars.iter()) {
            assert!((a.x - b.x).abs() < 1e-10);
            assert!((a.y - b.y).abs() < 1e-10);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo-nextest ntr -p emmain`
Expected: FAIL

- [ ] **Step 3: Implement emStarFieldPanel**

Read the C++ source carefully. Port:

- `Star` struct: x (f64), y (f64), radius (f64), color (emColor)
- Constructor: seeded RNG from `randomSeed`, generate stars based on depth
- `PanelBehavior::Paint`: draw each star as colored rect (small) or image (large)
- `PanelBehavior::LayoutChildren`: when zoomed in enough, create 4 child `emStarFieldPanel`s at quadrant positions with XOR'd seeds (0x74fc8324, 0x058f56a9, 0xfc863e37, 0x8bef7891)
- Easter egg: at depth > 50, random chance (1/11213) to create TicTacToe panel (stub as empty panel for now — no emMines/game port)
- Star shape image: loaded from `emRes` (Star.tga)

The C++ uses `emGetRandom()` with the seed — port that PRNG exactly (it's a simple LCG or similar).

- [ ] **Step 4: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emStarFieldPanel.rs crates/emmain/src/lib.rs
git commit -m "feat(emStarFieldPanel): port fractal starfield with recursive quadrants"
```

---

### Task 13: emVirtualCosmos — panel and item panel

**Files:**
- Modify: `crates/emmain/src/emVirtualCosmos.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emMain/emVirtualCosmos.cpp`

This adds `emVirtualCosmosPanel` and `emVirtualCosmosItemPanel` to the file that already has the model (Task 6).

- [ ] **Step 1: Implement emVirtualCosmosPanel**

Read the C++ source. Port:

- `PanelBehavior::LayoutChildren`: create `emStarFieldPanel` as background (name "sf", position 0,0,1,1, depth=50, seed from context hash). For each item in model, create/update `emVirtualCosmosItemPanel` child at item's position.
- `PanelBehavior::Cycle`: check model change signal, recreate children if items changed
- `PanelBehavior::GetCanvasColor`: return black (0x000000FF) — space background
- `PanelBehavior::notice`: on layout change, reposition children from model positions

The C++ maps item positions using: `child.Layout(item.PosX, item.PosY * GetHeight(), item.Width, item.Width * item.ContentTallness)`.

- [ ] **Step 2: Implement emVirtualCosmosItemPanel**

Read the C++ source. Port:

- Holds reference to its `emVirtualCosmosItemRec`
- `PanelBehavior::Paint`: draw border (using `BorderScaling`), title text, background color
- `PanelBehavior::LayoutChildren`: when auto-expand triggers (viewed width large enough), call `emFpPluginList::CreateFilePanel()` with the item's resolved file path. The file path is resolved from `FileName` relative to the `VcItemFiles` directory.
- When zoomed out past threshold, destroy the content panel
- `PanelBehavior::GetCanvasColor`: return item's `BackgroundColor`
- `PanelBehavior::auto_expand`: return `true`
- `PanelBehavior::get_title`: return item's `Title`
- `PanelBehavior::IsOpaque`: return `true` when border covers everything

- [ ] **Step 3: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS (unit tests from Task 6 still pass, new code compiles)

- [ ] **Step 4: Commit**

```bash
git add crates/emmain/src/emVirtualCosmos.rs
git commit -m "feat(emVirtualCosmos): port cosmos panel and item panel with plugin loading"
```

---

### Task 14: emMainContentPanel — content container with logo

**Files:**
- Create: `crates/emmain/src/emMainContentPanel.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emMain/emMainContentPanel.cpp`

- [ ] **Step 1: Implement emMainContentPanel**

Read the C++ source. Port:

- `PanelBehavior::Paint`: draw gradient background (top: emColor(145,171,242), bottom: emColor(225,221,183)), then paint Eagle Mode logo at specific coordinates (78450, 47690 in internal coords, scaled by min(1.0/180000.0, h/120000.0))
- `PanelBehavior::LayoutChildren`: create `emVirtualCosmosPanel` as single child, positioned to overlay the logo area
- `PanelBehavior::GetCanvasColor`: return the background gradient top color
- Logo painting: the C++ `PaintEagle()` draws the eagle shape using painter primitives. Port the exact coordinates.

The eagle logo is decorative — if the exact shape is complex, you can start with a simplified version (text "Eagle Mode" or a solid rect) and refine later. But read the C++ to see if it's a resource image or procedural.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p emmain`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emMainContentPanel.rs crates/emmain/src/lib.rs
git commit -m "feat(emMainContentPanel): port content container with gradient and logo"
```

---

## Phase 5: Layout Panels (main panel, control panel, bookmarks panel)

**Gate:** Phase 4 tasks (12-14) must be complete.

### Task 15: emBookmarksPanel — bookmark UI

**Files:**
- Modify: `crates/emmain/src/emBookmarks.rs` (add panel to existing file)

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emMain/emBookmarks.cpp` — bookmark panel section

- [ ] **Step 1: Implement emBookmarksPanel**

Read the C++ source. Port:

- `emBookmarksPanel` is a panel that renders bookmark buttons from the `emBookmarksModel`
- Each bookmark entry becomes a button. Clicking triggers `emVisitingViewAnimator` to navigate to the bookmark's location.
- Groups render as collapsible sections with nested bookmark buttons
- Buttons show bookmark name, optional icon, optional hotkey label
- Right-click on a bookmark allows editing (name, description, icon) or deletion — port the basic UI, wire editing later if complex

The panel uses `emLinearLayout` to stack buttons vertically. Each button's `on_click` captures the bookmark's `LocationIdentity`, `LocationRelX`, `LocationRelY`, `LocationRelA` and calls `view.Visit()`.

For the initial port, the navigation action will need a reference to the content view's animator. This is wired through `emMainWindow` → `emMainControlPanel` → `emBookmarksPanel`. Use `Rc<RefCell<...>>` for the shared view reference.

- [ ] **Step 2: Run tests**

Run: `cargo-nextest ntr -p emmain`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emBookmarks.rs
git commit -m "feat(emBookmarksPanel): port bookmark button UI with navigation"
```

---

### Task 16: emMainControlPanel — sidebar controls

**Files:**
- Create: `crates/emmain/src/emMainControlPanel.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emMainControlPanel.h` and `~/git/eaglemode-0.96.4/src/emMain/emMainControlPanel.cpp`

- [ ] **Step 1: Implement emMainControlPanel**

Read the C++ source. Port the widget tree layout:

```
emMainControlPanel (emLinearGroup, vertical)
├── lMain (emLinearLayout, horizontal)
│   ├── lAbtCfgCmd (emLinearLayout, vertical)
│   │   ├── lAbtCfg (emLinearLayout, vertical)
│   │   │   ├── grAbout (emLinearGroup, "About Eagle Mode")
│   │   │   │   ├── iconLabel (emLabel with eaglemode icon)
│   │   │   │   └── textLabel (emLabel with about text)
│   │   │   └── coreConfigPanel (emCoreConfigPanel)
│   │   └── grCommands (emPackGroup, "Main Commands")
│   │       ├── BtNewWindow (emButton, "New Window", F4)
│   │       ├── BtFullscreen (emCheckButton, "Fullscreen", F11)
│   │       ├── BtReload (emButton, "Reload")
│   │       ├── BtClose (emButton, "Close")
│   │       └── BtQuit (emButton, "Quit")
│   └── bookmarksPanel (emBookmarksPanel)
```

Port the exact layout weights from C++:
- MainGroup: ChildWeight(0)=11.37, ChildWeight(1)=21.32, InnerSpace=0.0098
- lMain: ChildWeight(0)=4.71, ChildWeight(1)=6.5, InnerSpace=0.0281
- lAbtCfgCmd: ChildWeight(0)=1.5, ChildWeight(1)=3.05, InnerSpace=0.07
- grCommands: PrefChildTallness=0.7

Button callbacks:
- BtNewWindow: needs reference to `emMain` to call `NewWindow()`
- BtFullscreen: toggles `emWindow::WF_FULLSCREEN` on the main window
- BtReload: signals file model reload
- BtClose: closes current window
- BtQuit: closes all windows (exits)

Wire callbacks via `Rc<RefCell<...>>` references passed at construction.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p emmain`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emMainControlPanel.rs crates/emmain/src/lib.rs
git commit -m "feat(emMainControlPanel): port sidebar with buttons, bookmarks, config panel"
```

---

### Task 17: emMainPanel — split layout with slider

**Files:**
- Create: `crates/emmain/src/emMainPanel.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emMainPanel.h` and `~/git/eaglemode-0.96.4/src/emMain/emMainPanel.cpp`

- [ ] **Step 1: Implement emMainPanel**

Read the C++ source carefully. This is the most complex layout panel. Port:

**Children:**
- `ControlViewPanel` (emSubViewPanel) — hosts control panel
- `ContentViewPanel` (emSubViewPanel) — hosts content panel
- `Slider` — a thin panel for drag interaction
- `StartupOverlay` — optional panel shown during startup

**Coordinate calculation** (`UpdateCoordinates`):
```
SliderMinY = 0.0
SliderMaxY = min(ControlTallness, h * 0.5)
SliderW = min(min(1.0, h) * 0.1, max(1.0, h) * 0.02)
SliderH = SliderW * 1.2
SliderX = 1.0 - SliderW
SliderY = (SliderMaxY - SliderMinY) * UnifiedSliderPos + SliderMinY
spaceFac = 1.015

ControlX = 0
ControlW = SliderX
ControlY = SliderY + SliderH * 0.5 - ControlH * 0.5  (adjusted)
ControlH = computed from slider position

ContentX = 0
ContentW = SliderX
ContentY = ControlY + ControlH * spaceFac
ContentH = h - ContentY
```

Port these exact formulas. The `UnifiedSliderPos` maps to `emMainConfig.ControlViewSize`.

**Slider drag** (`DragSlider`):
- Track mouse delta in `Input()`
- Clamp slider Y to `[SliderMinY, SliderMaxY]`
- Update `UnifiedSliderPos` and save to config

**Auto-hide:**
- When fullscreen and `AutoHideControlView` is true, collapse control view
- When fullscreen and `AutoHideSlider` is true, hide slider after 5s timeout
- Use `emTimer` for the 5s delay

**Paint:**
- Draw black separator between control and content
- Draw control edge texture (Slider.tga) at edges

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p emmain`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emMainPanel.rs crates/emmain/src/lib.rs
git commit -m "feat(emMainPanel): port split layout with slider drag and auto-hide"
```

---

## Phase 6: Window and Boot

**Gate:** Phase 5 tasks (15-17) must be complete.

### Task 18: emMainWindow — window lifecycle and startup animation

**Files:**
- Create: `crates/emmain/src/emMainWindow.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/include/emMain/emMainWindow.h` and `~/git/eaglemode-0.96.4/src/emMain/emMainWindow.cpp`

- [ ] **Step 1: Implement emMainWindow**

Read the C++ source. Port:

**Construction:**
- Creates main `emWindow` with `emWindowStateSaver` (path: `~/.eaglemode/emMain/winstate.rec`)
- Creates `emMainPanel` as root panel
- Creates separate control `emWindow` (detached sidebar)
- Creates `emBookmarksModel` (shared)
- Creates `emAutoplayViewModel`
- Creates `StartupEngine`

**StartupEngine:**
- An `emEngine` that runs at construction, drives the startup animation
- Takes visit parameters: identity, relX, relY, relA, adherent, subject
- Phase 1: wait for window to be ready
- Phase 2: animate zoom from overview to visit target using `emVisitingViewAnimator`
- Phase 3: settle and deactivate
- Total duration: ~2 seconds
- Read the C++ `StartupEngineClass` carefully — it has specific timing and phase transitions

**Window flags:**
- Content view: `VF_ROOT_SAME_TALLNESS | VF_NO_ZOOM` (initially, removed after startup)
- `wmResName` parameter for WM_CLASS

**Key methods:**
- `SetControlVisibility(bool)`: shows/hides the detached control window
- `GetControlWindow()`: returns reference to control window
- `GetContentView()`: returns content view reference
- Closes cleanly when `ToClose` flag is set

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p emmain`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emMainWindow.rs crates/emmain/src/lib.rs
git commit -m "feat(emMainWindow): port window lifecycle with startup animation"
```

---

### Task 19: emMain — IPC server engine and window factory

**Files:**
- Create: `crates/emmain/src/emMain.rs`
- Modify: `crates/emmain/src/lib.rs`

**C++ Reference:** `~/git/eaglemode-0.96.4/src/emMain/emMain.cpp`

- [ ] **Step 1: Implement emMain engine**

Read the C++ source. Port:

**Server name calculation:**
C++ derives name from DISPLAY environment variable: `eaglemode_on_{hostname}:{display}.{screen}`. Port the `CalcServerName()` logic exactly.

**emMain struct:**
```rust
pub struct emMain {
    ipc_server: emMiniIpcServer,
    windows: Vec<emMainWindow>,
    context: Rc<emContext>,
    // ... other state
}
```

**emEngine impl:**
- `Cycle()`: check for pending IPC messages (server polls automatically via timer), check if any windows want to close, clean up closed windows

**NewWindow method:**
Parses window-specific args from IPC message:
- `-geometry WxH+X+Y`
- `-cecolor <color>`
- `-wmresname <name>`
- `-maximized`, `-fullscreen`, `-undecorated`
- `-visit <identity>`
Creates `emMainWindow` with those parameters.

**IPC callback:**
- `"NewWindow"` → calls `NewWindow()` with remaining args
- `"ReloadFiles"` → signals file model update

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p emmain`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emMain.rs crates/emmain/src/lib.rs
git commit -m "feat(emMain): port IPC server engine with window factory"
```

---

### Task 20: Binary entry point

**Files:**
- Modify: `crates/eaglemode/src/main.rs`
- Modify: `crates/eaglemode/Cargo.toml`

- [ ] **Step 1: Update Cargo.toml dependencies**

Add `emmain` dependency:

```toml
[dependencies]
emmain = { path = "../emmain" }
emcore = { path = "../emcore" }
```

- [ ] **Step 2: Implement main.rs boot sequence**

Replace the placeholder with the real boot sequence. Read the C++ `wrapped_main()` in `emMain.cpp` lines 456-602 for the exact flow:

```rust
fn main() {
    // 1. Parse CLI args
    let args = parse_args();

    // 2. Register static plugin resolver
    emcore::emFpPlugin::set_static_plugin_resolver(
        emmain::static_plugins::resolve_static_plugin
    );

    // 3. Try IPC client (unless -noclient)
    if !args.no_client {
        let server_name = emmain::emMain::CalcServerName();
        let ipc_args = build_ipc_args(&args);
        match emcore::emMiniIpc::emMiniIpcClient::TrySend(&server_name, &ipc_args) {
            Ok(()) => {
                // Existing server handled our request
                return;
            }
            Err(_) => {
                // No server, continue to start our own
            }
        }
    }

    // 4. Start GUI framework with setup callback
    let setup = Box::new(move |app: &mut App, event_loop: &ActiveEventLoop| {
        let em_main = emmain::emMain::new(app, &args);
        em_main.NewWindow(app, event_loop, &args);
    });

    emcore::emGUIFramework::App::new(setup).run();
}
```

Adapt to the exact `App::new()` and `run()` signatures from `emGUIFramework.rs`. The setup callback receives the `App` and `ActiveEventLoop` and creates the `emMain` engine + first window.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p eaglemode`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/src/main.rs crates/eaglemode/Cargo.toml
git commit -m "feat(eaglemode): wire binary entry point with IPC client and GUI framework"
```

---

## Phase 7: Config Files

### Task 21: Cosmos item files

**Files:**
- Create: `etc/emMain/VcItems/Home.emVcItem`
- Create: `etc/emMain/VcItems/Root.emVcItem`
- Create: `etc/emMain/VcItems/Stocks1.emVcItem`

- [ ] **Step 1: Read C++ defaults and create item files**

Read the C++ item files from `~/git/eaglemode-0.96.4/etc/emMain/VcItems/` to get exact positions. Copy the ones we need, preserving all position/color/size values.

For any items that reference directories (like `Home`, `Root`), copy them verbatim. For `Stocks1`, copy it. Skip all items that reference unported plugins (clock, games, etc.).

Also check if there are other directory-pointing items in the C++ defaults (like tmp, etc) and include those.

Example `Home.emVcItem` (adapt from C++ original):
```
#%rec:emVirtualCosmosItem%#

Title = "Home"
PosX = <copy from C++>
PosY = <copy from C++>
Width = <copy from C++>
ContentTallness = <copy from C++>
BorderScaling = <copy from C++>
BackgroundColor = { <copy from C++> }
BorderColor = { <copy from C++> }
TitleColor = { <copy from C++> }
Focusable = true
FileName = "Home"
CopyToUser = false
Alternative = 0
```

- [ ] **Step 2: Create default Stocks data file**

If the C++ ships a default `Stocks1.emStocks` file in `VcItemFiles`, copy it. Otherwise create a minimal one.

- [ ] **Step 3: Commit**

```bash
git add etc/
git commit -m "feat: add cosmos item files for Home, Root, and Stocks"
```

---

### Task 22: Plugin registration files

**Files:**
- Create: `etc/emCore/FpPlugins/emDir.emFpPlugin`
- Create: `etc/emCore/FpPlugins/emStocks.emFpPlugin`
- Create: `etc/emCore/FpPlugins/emDirStat.emFpPlugin`
- Create: `etc/emCore/FpPlugins/emFileLink.emFpPlugin`

- [ ] **Step 1: Read C++ plugin files and create Rust equivalents**

Read the C++ `.emFpPlugin` files from `~/git/eaglemode-0.96.4/etc/emCore/FpPlugins/`. Copy the ones for our ported plugins, updating the `Library` field to match our crate names.

Example `emDir.emFpPlugin`:
```
#%rec:emFpPlugin%#

FileTypes = { "directory" }
FileFormatName = "emDir"
Priority = 1.0
Library = "emfileman"
Function = "emDirFpPluginFunc"
```

Do the same for emStocks, emDirStat, emFileLink. Check the C++ originals for exact `FileTypes` and `Priority` values.

- [ ] **Step 2: Commit**

```bash
git add etc/
git commit -m "feat: add FpPlugin registration files for ported plugins"
```

---

### Task 23: Default bookmarks file

**Files:**
- Create: `etc/emMain/Bookmarks.emBookmarks`

- [ ] **Step 1: Create minimal default bookmarks**

Read the C++ default bookmarks from `~/git/eaglemode-0.96.4/etc/emMain/` if they exist. Create a minimal file with a "Home" bookmark:

```
#%rec:emBookmarks%#

{
  Bookmark {
    Name = "Home"
    Description = "Your home directory"
    Hotkey = "F5"
    LocationIdentity = "::VcItem:Home:"
    LocationRelX = 0.5
    LocationRelY = 0.5
    LocationRelA = 0.01
  }
}
```

Adjust the `LocationIdentity` format to match what `emVirtualCosmosItemPanel` produces as its panel identity string. Read the C++ to see the exact identity format.

- [ ] **Step 2: Commit**

```bash
git add etc/
git commit -m "feat: add default bookmarks file with Home bookmark"
```

---

## Phase 8: Integration and Smoke Test

**Gate:** All previous phases must be complete.

### Task 24: Compilation and link test

- [ ] **Step 1: Full build**

Run: `cargo build -p eaglemode`
Expected: success. Fix any compilation errors.

- [ ] **Step 2: Run all tests**

Run: `cargo-nextest ntr`
Expected: all existing tests pass, new tests pass

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings. Fix any that appear.

- [ ] **Step 4: Commit any fixes**

```bash
git add -u
git commit -m "fix: resolve compilation and clippy issues from integration"
```

---

### Task 25: Runtime smoke test

- [ ] **Step 1: Run the binary**

Run: `cargo run -p eaglemode`

Expected behavior:
- Window opens with starfield background
- Cosmos items visible (Home, Root, Stocks)
- Control panel sidebar on left with buttons and bookmarks
- Can zoom/pan in the cosmos
- Zooming into Home shows directory listing
- Zooming into a directory entry shows its children

- [ ] **Step 2: Test IPC single-instance**

In a second terminal:
```bash
cargo run -p eaglemode
```

Expected: second invocation opens a new window in the first process (via IPC), then exits.

- [ ] **Step 3: Test persistence**

1. Resize the window, close it
2. Relaunch — window should restore previous size/position
3. Change control panel size (drag slider), close, relaunch — slider position preserved

- [ ] **Step 4: Fix any runtime issues found**

Debug and fix. Commit each fix separately with descriptive messages.

- [ ] **Step 5: Final commit**

```bash
git add -u
git commit -m "fix: runtime integration fixes from smoke testing"
```

---

## Task Dependency Graph

```
Phase 1 (Foundation):  [T1] → [T2] → [T3]
Phase 2 (Data Models): [T4, T5, T6] (parallel, after T1)
Phase 3 (emFileMan):   [T7] → [T8] → [T9] → [T10] → [T11] (sequential)
Phase 4 (Leaf Panels):  [T12] (after T1), [T13] (after T6,T12), [T14] (after T13)
Phase 5 (Layout):       [T15] (after T4), [T16] (after T15), [T17] (after T16)
Phase 6 (Window/Boot):  [T18] (after T14,T17), [T19] (after T18), [T20] (after T19,T3)
Phase 7 (Config Files): [T21, T22, T23] (parallel, after T6)
Phase 8 (Integration):  [T24] (after all), [T25] (after T24)
```

**Parallelizable groups:**
- T4, T5, T6 can run in parallel (independent data models)
- T7-T11 are sequential (each builds on prior)
- T12 can run in parallel with T4-T6
- T21, T22, T23 can run in parallel (independent config files)
- Phase 3 (T7-T11) is independent of Phases 4-6 and can run in parallel

**Critical path:** T1 → T2 → T3 → T6 → T13 → T14 → T17 → T18 → T19 → T20 → T24 → T25
