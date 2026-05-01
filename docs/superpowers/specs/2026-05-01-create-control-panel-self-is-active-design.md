# Design: `CreateControlPanel` `self_is_active` parameter

**Date:** 2026-05-01  
**Bug fixed:** `invalid SlotMap key used` panic in `PanelTree::remove`, triggered by zoom-out after extensive zoom-in.

## Problem

`emDirPanel::CreateControlPanel` and `emDirEntryPanel::CreateControlPanel` in the C++ source guard their implementation with `if (IsActive())`, returning `NULL` when the panel is not the active (leaf) panel. The Rust port omitted this guard.

`create_control_panel_in` walks up the content sub-tree from the active panel, calling `CreateControlPanel` on each behavior. Without the guard, a non-active `emDirPanel` or `emDirEntryPanel` ancestor creates a child in the wrong tree (content sub-tree instead of ctrl sub-tree). The returned `PanelId` is then stored as `content_ctrl_panel` and passed to `PanelTree::remove` on the ctrl sub-tree, which panics on the invalid key.

In C++, behaviors call `this->IsActive()` directly because the behavior and the panel are the same object. In Rust, the behavior struct is separate from `PanelData`; `CreateControlPanel` receives a `PanelCtx` pointing into the target (ctrl) tree, not the content tree, so there is no direct path to `is_active`.

## Design

Add `self_is_active: bool` to the `PanelBehavior::CreateControlPanel` signature. The framework passes the content-tree panel's `is_active` at the call site; behaviors use it exactly as they would call `this->IsActive()` in C++.

### Trait change (`crates/emcore/src/emPanel.rs:399`)

```rust
fn CreateControlPanel(
    &mut self,
    _parent_ctx: &mut PanelCtx,
    _name: &str,
    _self_is_active: bool,
) -> Option<PanelId> {
    None
}
```

### Call site in `create_control_panel_in` (`crates/emcore/src/emPanelTree.rs`)

```rust
let self_is_active = self.panels.get(cur).map(|p| p.is_active).unwrap_or(false);
let result = behavior.CreateControlPanel(&mut ctx, name, self_is_active);
```

### Call site in `PanelTree::CreateControlPanel` (non-cross-tree variant, same file)

Same treatment: read `self.panels.get(cur).map(|p| p.is_active).unwrap_or(false)` and pass it.

### `emDirPanel::CreateControlPanel` (`crates/emfileman/src/emDirPanel.rs:530`)

```rust
fn CreateControlPanel(&mut self, parent_ctx: &mut PanelCtx, name: &str, self_is_active: bool) -> Option<PanelId> {
    if !self_is_active { return None; }  // C++: if (IsActive())
    // ... existing body unchanged
}
```

### `emDirEntryPanel::CreateControlPanel` (`crates/emfileman/src/emDirEntryPanel.rs:1201`)

Same guard added at the top of the existing body.

### Two test stubs (`crates/emcore/src/emPanelTree.rs`, lines ~3138 and ~3170)

Add `_self_is_active: bool` to the parameter list; no body change.

## Scope

6 locations touched total. No behavioral change beyond restoring the C++ guard. Future `PanelBehavior` implementations that check their own internal state (e.g., `HaveControlPanel` in `emNetwalkPanel`) instead of `IsActive()` receive the parameter and may ignore it — it is always correct to ignore it if the behavior has a tighter or orthogonal condition.

## Non-goals

- No change to `notice()` wiring, no cached `is_active` field on behaviors.
- No change to the walk logic in `create_control_panel_in` (still calls all behaviors in the ancestor chain; each behavior decides).
