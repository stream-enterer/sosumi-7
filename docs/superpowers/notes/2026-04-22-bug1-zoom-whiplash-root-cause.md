# Bug #1 — Zoom Whiplash Root Cause

**Date:** 2026-04-22  
**Bug:** Mouse wheel causes camera to whiplash SE then NW per zoom step instead of smooth zoom toward cursor.

## Root Cause Chain

### C++ correct behavior

`emView.cpp:131-134` (`emView::SetViewFlags`):
```cpp
if ((viewFlags & VF_NO_ZOOM) != 0) {
    viewFlags &= ~(VF_POPUP_ZOOM | VF_EGO_MODE);
    viewFlags |= VF_NO_USER_NAVIGATION;  // ← VF_NO_ZOOM implies VF_NO_USER_NAVIGATION
}
```

`emMainWindow.cpp:52-57`:
```cpp
SetViewFlags(
    VF_ROOT_SAME_TALLNESS |
    VF_NO_ZOOM |           // ← outer view is non-zoomable
    VF_NO_FOCUS_HIGHLIGHT |
    VF_NO_ACTIVE_HIGHLIGHT
);
```

`emViewInputFilter.cpp:117-122` (`emMouseZoomScrollVIF::Input`):
```cpp
if ((GetView().GetViewFlags() & emView::VF_NO_USER_NAVIGATION) != 0) {
    if (MouseAnim.IsActive()) MouseAnim.Deactivate();
    if (WheelAnim.IsActive()) WheelAnim.Deactivate();
    ForwardInput(event, rwstate);  // ← forwards WITHOUT eating!
    return;
}
```

So in C++:
1. Outer view has VF_NO_ZOOM → implicitly has VF_NO_USER_NAVIGATION
2. Outer VIF sees VF_NO_USER_NAVIGATION → forwards wheel event without eating
3. RecurseInput dispatches non-eaten wheel event to panels
4. emSubViewPanel::Input receives non-eaten wheel event
5. SubViewPort->InputToView(event, state) → content sub-view's own VIF chain
6. Content VIF handles wheel and zooms the content view

### Rust defects

Three missing pieces:
1. **Missing flag:** Outer view never gets `NO_ZOOM | NO_USER_NAVIGATION` set. `emMainWindow` doesn't call `view.set_flags(...)`.
2. **Missing forward-without-eat:** Rust `emMouseZoomScrollVIF::filter` has no check for `NO_USER_NAVIGATION`. It always eats wheel events.
3. **Missing sub-view VIF chain:** Rust `emSubViewPanel` has no VIF chain. Even if events reached it, there's no VIF to handle wheel zoom.

### Why SE/NW oscillation

With the outer VIF eating wheel events and zooming `self.view` (outer view with emMainPanel as root):
- The fix point is at the cursor position (inside the content area, SE of view center)
- `RawScrollAndZoom` tries to zoom into the content sub-view child panel
- `RawVisitAbs` ancestor-clamping bounces it back (root panel is always full-size with NO_ZOOM or clamp logic)
- Spring applies zoom → clamped → spring applies more → clamped = oscillation

## Fix Required

### Step 1 (minimum hypothesis test — user must run)
Set `NO_ZOOM | NO_USER_NAVIGATION` on outer view in `emMainWindow`.
Add `NO_USER_NAVIGATION` early-return in `emMouseZoomScrollVIF::filter`.
**Predicted:** whiplash stops; zoom does nothing (no sub-view VIF yet).

### Step 2 (architectural fix)
Give `emSubViewPanel` its own VIF chain (Option D — matches C++ each-emView-has-own-VIFs).
Wire VIF construction to sub-view creation (analogous to C++ `emView::emView()` lines 91-94).
In `emSubViewPanel::Input`, call sub-view's VIF chain before panel RecurseInput.

## Files Involved

- `crates/emcore/src/emView.rs` — `SetViewFlags` must add NO_USER_NAVIGATION when NO_ZOOM is set
- `crates/emcore/src/emViewInputFilter.rs` — `emMouseZoomScrollVIF::filter`: check NO_USER_NAVIGATION early
- `crates/emmain/src/emMainWindow.rs` — set NO_ZOOM on outer view at creation
- `crates/emcore/src/emSubViewPanel.rs` — add VIF chain field + dispatch in `Input`
- `crates/emcore/src/emWindow.rs` — potentially adjust VIF chain scope
