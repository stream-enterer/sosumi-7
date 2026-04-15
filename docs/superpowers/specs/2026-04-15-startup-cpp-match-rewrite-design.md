# Startup C++ Match Rewrite Design

**Date:** 2026-04-15
**Status:** Draft
**Supersedes:** `2026-04-15-startup-architecture-rewrite-design.md` (failed implementation)

## Goal

Rewrite the Rust startup and control panel architecture to exactly match C++ Eagle Mode. Eliminate all DIVERGED patterns that affect the startup visual sequence and control panel lifecycle. The app must show: eagle image with gradient background → startup overlay → zoom animation into eagle's eye → cosmos with starfield + items.

## Problem

The previous rewrite (commits b826b33–8a812fa) got the engine architecture right (EngineCtx widening, event loop pumping) but the panel creation was wrong. Instead of the eagle image and cosmos, the app showed grey background with two empty blue rectangles. Root causes:

1. **`creation_stage` indirection:** C++ StartupEngine directly creates panels in sub-views (`new emMainControlPanel(GetControlView(), ...)`, `new emMainContentPanel(GetContentView(), "")`). Rust gates creation through `advance_creation_stage()` → `LayoutChildren`, which broke the notice chain.

2. **ZuiWindow's separate control system:** Rust ZuiWindow has `control_tree`, `control_view`, `control_panel_id`, `control_strip_height` for per-panel context controls. This doesn't exist in C++. In C++, per-panel controls live inside `emMainControlPanel` as `ContentControlPanel`, managed by `RecreateContentControlPanel()` triggered by `ControlPanelSignal`.

3. **emGUIFramework drives control lifecycle:** The `about_to_wait()` loop manages control panel creation/destruction each frame. In C++, `emMainControlPanel::Cycle()` handles this via signals.

## Design

Seven changes. All applied simultaneously (big bang).

### 1. emMainPanel: Eliminate creation_stage

**Delete:**
- `creation_stage: u8` field (line 330)
- `advance_creation_stage()` method (lines 535-539)
- `creation_stage()` getter (lines 542-544)
- `control_panel_created: Option<PanelId>` field (line 318)
- `content_panel_created: Option<PanelId>` field (line 319)
- Creation-gated blocks in `LayoutChildren` (lines 778-814)
- Tests: `test_creation_stage_initial`, `test_advance_creation_stage`, `test_advance_creation_stage_saturates_at_2`

**Keep:** Sub-view panel creation in `LayoutChildren` (lines 749-776) — ControlViewPanel, ContentViewPanel, SliderPanel, StartupOverlayPanel are still created here on first layout. This matches C++ where `emMainPanel` constructor creates these (emMainPanel.cpp:39-42).

**Add:** Public methods to expose sub-view panel IDs for StartupEngine:
```rust
pub fn GetControlViewPanelId(&self) -> Option<PanelId> {
    self.control_view_panel
}
pub fn GetContentViewPanelId(&self) -> Option<PanelId> {
    self.content_view_panel
}
```

**LayoutChildren becomes pure positioning:** After initial child creation, it only calls `update_coordinates()` and positions children. Matches C++ emMainPanel.cpp:225-231.

**Files:** `crates/emmain/src/emMainPanel.rs`

### 2. StartupEngine: Direct Panel Creation

Rewrite states 5 and 6 to directly create panels inside sub-views, matching C++ emMainWindow.cpp:407-422.

**State 5 — Create emMainControlPanel:**
```rust
// Get control sub-view panel ID from emMainPanel
let ctrl_view_id = ctx.tree.with_behavior_as::<emMainPanel, _>(
    self.main_panel_id, |mp| mp.GetControlViewPanelId()
).flatten();

// Create emMainControlPanel inside the control sub-view's sub-tree
if let Some(ctrl_id) = ctrl_view_id {
    ctx.tree.with_behavior_as::<emSubViewPanel, _>(ctrl_id, |svp| {
        let sub_tree = svp.sub_tree_mut();
        let sub_root = sub_tree.GetRootPanel().expect("sub-view has root");
        let child_id = sub_tree.create_child(sub_root, "ctrl");
        sub_tree.set_behavior(child_id, Box::new(
            emMainControlPanel::new(ctx_clone, content_view_id)
        ));
        sub_tree.Layout(child_id, 0.0, 0.0, 1.0, control_tallness);
    });
}
```

**State 6 — Create emMainContentPanel:** Same pattern, creating inside content sub-view's sub-tree.

**No advance_creation_stage() calls.** No LAYOUT_CHANGED notice needed.

**Files:** `crates/emmain/src/emMainWindow.rs`

### 3. ZuiWindow: Remove Control System

**Delete from ZuiWindow struct:**
- `control_tree: PanelTree` (line 64)
- `control_view: emView` (line 66)
- `control_panel_id: Option<PanelId>` (line 68)
- `control_strip_height: u32` (line 70)

**Delete methods:**
- `show_control_strip()` (lines 221-234)
- `hide_control_strip()` (lines 237-245)
- `content_height()` (if it exists — computes height minus control strip)

**Update `create()`:** Remove control_tree/control_view initialization.

**Update `render()`:** Remove second paint pass for control strip. Only `self.view.Paint(tree, ...)`.

**Update `resize()`:** Full height goes to main view. No control strip subtraction.

**Files:** `crates/emcore/src/emWindow.rs`

### 4. emGUIFramework: Remove Control Panel Lifecycle

**Delete from `about_to_wait()`:**
- The entire block at lines 408-445: `is_control_panel_invalid()` check, `create_control_panel_in()`, `show_control_strip()`/`hide_control_strip()`, `HandleNotice` for control_tree, `control_view.update()`

The `about_to_wait()` loop should only handle: flags signals, scheduler time slice, event loop pumping, panel cycles, notices, view updates, animations, dirty rects, viewport changes.

**Files:** `crates/emcore/src/emGUIFramework.rs`

### 5. emMainControlPanel: Add ContentControlPanel Lifecycle

Match C++ emMainControlPanel (emMainControlPanel.h:59-64, emMainControlPanel.cpp:217-324).

**Add to emMainControlPanel struct:**
- `content_control_panel: Option<PanelId>` — ID of dynamically created per-panel control panel
- `content_view_id: Option<PanelId>` — ID of the content sub-view panel (needed to get active panel)

**Constructor change:** `emMainControlPanel::new()` takes an additional parameter: the content sub-view panel ID. This replaces C++'s `emView & ContentView` parameter.

C++ constructor (emMainControlPanel.cpp:217):
```cpp
AddWakeUpSignal(ContentView.GetControlPanelSignal());
```

Rust equivalent: The existing `control_panel_invalid` flag on emView serves as the signal. emMainControlPanel's `Cycle()` checks this flag via the content sub-view.

**Add `RecreateContentControlPanel()`:** Match C++ emMainControlPanel.cpp:317-324:
```rust
fn RecreateContentControlPanel(&mut self, ctx: &mut PanelCtx) {
    // Delete old
    if let Some(old_id) = self.content_control_panel.take() {
        ctx.remove_child(old_id);
    }
    // Get active panel from content sub-view, create its control panel as our child
    if let Some(content_id) = self.content_view_id {
        let active = ctx.tree.with_behavior_as::<emSubViewPanel, _>(content_id, |svp| {
            svp.sub_view().GetActivePanel()
        }).flatten();
        if let Some(active_id) = active {
            self.content_control_panel = ctx.tree.with_behavior_as::<emSubViewPanel, _>(
                content_id, |svp| {
                    let sub_tree = svp.sub_tree_mut();
                    sub_tree.create_control_panel(active_id, /* parent= */ self_id, "context")
                }
            ).flatten();
        }
    }
}
```

**Note:** The exact `create_control_panel` call may need adjustment — C++ calls `ContentView.CreateControlPanel(*this, "context")` which forwards to the active panel's `CreateControlPanel(parent, name)`. The existing `PanelTree::create_control_panel_in()` provides this. The control panel is created as a child of `emMainControlPanel` itself, positioned via layout weight 21.32 (replacing the current bookmarks-only slot).

**Cycle() change:** Check if the content view's active panel changed (poll `control_panel_invalid` flag). If so, call `RecreateContentControlPanel()`.

**Layout change:** The content control panel needs to be positioned as a child of the top-level layout (child 1, weight 21.32 — currently occupied by bookmarks panel). The bookmarks panel moves inside the general panel where it belongs per C++ structure.

**Signal mechanism:** C++ `emMainControlPanel` holds `emView & ContentView` and calls `IsSignaled(ContentView.GetControlPanelSignal())`. Rust cannot hold a view reference. Instead:

1. The content sub-view's `emView` gets a `control_panel_signal: SignalId` (see Section 7)
2. emMainControlPanel's constructor receives this SignalId and registers it via `AddWakeUpSignal`
3. emMainControlPanel's `Cycle()` checks `ctx.IsSignaled(self.control_panel_signal)`
4. When signaled, call `RecreateContentControlPanel()` which accesses the content sub-view via `ctx.tree.with_behavior_as::<emSubViewPanel, _>(content_view_id, ...)`

This exactly matches C++ signal-driven lifecycle with no polling or shared flags.

**Files:** `crates/emmain/src/emMainControlPanel.rs`, `crates/emcore/src/emView.rs` (add ControlPanelSignal as a real SignalId)

### 6. ToggleControlView

Match C++ emMainWindow.cpp:144-158.

**Current Rust (DIVERGED):** Calls `DoubleClickSlider()` on emMainPanel's slider.

**New (matching C++):** Toggle the main view's focus between ControlViewPanel and ContentViewPanel.

C++ behavior:
- If ContentView is focused → visit ControlView, zoom to ":"
- If ControlView is focused → visit ContentView

**Trigger:** F11 or Esc with no modifiers, in emMainWindow `handle_input()`.

**Implementation:** The main ZuiWindow view sees emMainPanel as its root. The sub-view panels are children. To "toggle" between views, navigate the main view to zoom into either the control or content sub-view panel. This is a `Visit()` call targeting the sub-view panel.

C++ also has this in emMainControlPanel::Input (emMainControlPanel.cpp:301-308): Esc in control panel triggers ToggleControlView. This should be ported too.

**Files:** `crates/emmain/src/emMainWindow.rs`, `crates/emmain/src/emMainControlPanel.rs`

### 7. emView: Add ControlPanelSignal

C++ `emView` has `ControlPanelSignal` (emView.h:682) that fires when:
- Active panel changes (emView.cpp:308)
- `InvalidateControlPanel()` is called on a viewed panel

Rust already has `control_panel_invalid: bool` flag. Add a real `SignalId` (`control_panel_signal`) that fires alongside this flag, so emMainControlPanel can use `AddWakeUpSignal` / `IsSignaled` like C++.

**Add to emView:**
- `control_panel_signal: Option<SignalId>` — allocated lazily or passed in
- `GetControlPanelSignal() -> SignalId`
- Fire the signal in `SetActivePanel()` and `InvalidateControlPanel()`

**Files:** `crates/emcore/src/emView.rs`

## What This Preserves

- **EngineCtx widening** (commits 1-2): Engines have full tree+windows access. Correct and useful.
- **Event loop pumping** (commit 2): `request_redraw()` when engines are awake. Correct.
- **MainWindowEngine** (commit 5): Close signal handling. Correct.
- **BookmarksModel integration** (commit 4): Loading and hotkey handling. Correct.
- **Input blocking during startup** (commit 3): `startup_engine_id.is_some()` check. Correct.
- **emInputHotkey.rs** (commit 4): Hotkey conversion. Correct.

## What This Deletes

- `creation_stage` mechanism in emMainPanel (replaced by direct creation)
- `control_tree`, `control_view`, `control_panel_id`, `control_strip_height` from ZuiWindow
- Control panel lifecycle from `about_to_wait()` in emGUIFramework
- `show_control_strip()` / `hide_control_strip()` from ZuiWindow
- `advance_creation_stage()` and related tests

## Blast Radius

| File | Change |
|------|--------|
| `emMainPanel.rs` | Delete creation_stage, simplify LayoutChildren, add sub-view ID getters |
| `emMainWindow.rs` | Rewrite StartupEngine states 5-6, add ToggleControlView |
| `emMainControlPanel.rs` | Add ContentControlPanel lifecycle, restructure layout |
| `emWindow.rs` | Remove control_tree/control_view/control_strip_height, simplify render/resize |
| `emGUIFramework.rs` | Remove control panel lifecycle from about_to_wait |
| `emView.rs` | Add ControlPanelSignal (real SignalId), fire on active panel change |

## Testing Strategy

- Golden tests: 239 pass, 4 fail baseline — no new failures
- Full suite: no new failures
- `cargo clippy -- -D warnings` clean
- Manual verification:
  - App launches, eagle image with gradient visible
  - Startup overlay appears
  - Zoom animation plays (zoom to ":", then to start location)
  - Cosmos visible (starfield + Home/Root/Stocks1 items)
  - Control panel shows bookmarks and buttons
  - F11/Esc toggles between control and content views
  - Per-panel context controls appear when zooming into panels
  - Input blocked during startup animation

## Success Criteria

1. Runtime rendering matches C++ Eagle Mode startup sequence
2. No creation_stage mechanism remains
3. No control_tree/control_view on ZuiWindow
4. Per-panel controls managed by emMainControlPanel
5. ToggleControlView works with F11/Esc
6. All existing tests pass (golden + full suite)
7. No new clippy warnings
