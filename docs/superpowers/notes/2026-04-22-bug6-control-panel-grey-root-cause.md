# Bug #6 — Control Panel Grey Bar Root Cause

**Date:** 2026-04-22  
**Bug:** Top control panel shows as 80% grey center, with left/right 10% showing as "inverted" edge images.

## Root Cause Chain

### C++ correct behavior

`emMainPanel.cpp:44-48`:
```cpp
GetControlView().SetViewFlags(
    VF_POPUP_ZOOM          |
    VF_ROOT_SAME_TALLNESS  |
    VF_NO_ACTIVE_HIGHLIGHT
);
```

`emMainWindow.cpp:408-413`:
```cpp
MainWin.ControlPanel = new emMainControlPanel(
    MainWin.MainPanel->GetControlView(),  // ← View as parent = ROOT panel
    "ctrl", MainWin,
    MainWin.MainPanel->GetContentView()
);
```

`emView.cpp:1218-1228` (SetGeometry with VF_ROOT_SAME_TALLNESS):
```cpp
if (VFlags & VF_ROOT_SAME_TALLNESS) {
    RootPanel->Layout(0,0,1,GetHomeTallness());  // root height = HomeHeight/HomeWidth
}
```

So in C++:
1. Control view gets VF_ROOT_SAME_TALLNESS → SetGeometry updates root height to `control_h/control_w ≈ 0.0538`
2. emMainControlPanel IS the root panel of the control view (created with view as parent context)
3. RawZoomOut with root_h=0.0538 gives relA=1.0 → panel exactly fills view

### Rust defects

Two missing pieces:

1. **Missing flag call:** `set_sub_view_flags(VF_POPUP_ZOOM | VF_ROOT_SAME_TALLNESS | VF_NO_ACTIVE_HIGHLIGHT)` never called on control sub-view in emMainWindow state 5. Without `VF_ROOT_SAME_TALLNESS`, `SetGeometry` does not update root panel height.

2. **Wrong panel hierarchy:** Rust state 5 creates `emMainControlPanel` as a CHILD of sub_root (`create_child(sub_root, "ctrl")`), not as the sub_root behavior itself. Compare with content view (state 6) which correctly uses `set_behavior(sub_root, ...)`.

### Why center 80% is grey

Without VF_ROOT_SAME_TALLNESS, root panel stays at height=1.0 (set in `emSubViewPanel::new`). When geometry sync runs with control_w ≈ W pixels, control_h ≈ 0.0538*W pixels:

```
zoom_out_rel_a: a1 = W/0.0538W = 18.6, a2 = 0.0538, relA = 18.6
RawVisit: vw = sqrt(W * 0.0538W / 18.6) = sqrt(0.0538/18.6) * W ≈ 0.054 * W
```

Root panel appears at only ~5.4% of the control area's width. Sub-view background (grey `0x808080FF`) fills the remaining ~95%, which is the center portion of the control bar. The left/right edges are painted directly by `emMainPanel::Paint` (they don't use the sub-view).

## Files Involved

- `crates/emmain/src/emMainWindow.rs` — state 5: add `set_sub_view_flags(VF_POPUP_ZOOM | VF_ROOT_SAME_TALLNESS | VF_NO_ACTIVE_HIGHLIGHT)` on control sub-view; change to `set_behavior(sub_root, ...)` pattern matching state 6 (content view)
- `crates/emmain/src/emMainWindow.rs` — state 6: add `set_sub_view_flags(VF_ROOT_SAME_TALLNESS)` on content sub-view (C++ line: `GetContentView().SetViewFlags(VF_ROOT_SAME_TALLNESS)`)

## Fix

### Control sub-view (state 5)
Replace:
```rust
let child_id = sub_tree.create_child(sub_root, "ctrl", None);
sub_tree.set_behavior(child_id, Box::new(emMainControlPanel::new(ctrl_ctx, content_view_id)));
sub_tree.Layout(child_id, 0.0, 0.0, 1.0, 0.0538, 1.0, None);
```
With:
```rust
sub_tree.set_behavior(sub_root, Box::new(emMainControlPanel::new(ctrl_ctx, content_view_id)));
svp.sub_view.flags |= ViewFlags::POPUP_ZOOM | ViewFlags::ROOT_SAME_TALLNESS | ViewFlags::NO_ACTIVE_HIGHLIGHT;
```
(OR call `svp.set_sub_view_flags(...)` before the first geometry sync.)

### Content sub-view (state 6)
Add after `set_behavior(sub_root, ...)`:
```rust
svp.sub_view.flags |= ViewFlags::ROOT_SAME_TALLNESS;
```
