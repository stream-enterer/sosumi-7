# Design: Per-child canvas_color propagation

## Problem

10 golden tests fail because child panels receive wrong canvas_color. Rust uses `set_all_children_canvas_color(cc)` which blanket-sets canvas_color on ALL children. C++ passes canvas_color per-child via `child->Layout(x,y,w,h,cc)` — some children explicitly receive `cc`, others get transparent (0) by default.

DrawOp diff confirms: one `STATE:canvas_color` MISMATCH in widget_colorfield — C++ has `00000000`, Rust has `a7a9b0ff`.

## Affected tests

- colorfield (5 tests)
- composite (3 tests)
- testpanel (2 tests)

## Root cause

Two C++ patterns exist:

1. **Layout widgets** (emLinearLayout, emRasterLayout, emPackLayout, emSplitter, emTiling, emTunnel): Pass `cc` to every child. Rust's blanket call is correct here.

2. **Content widgets** (emColorField): Call `emBorder::LayoutChildren()` (sets cc on aux panel), then layout additional children WITHOUT canvas_color — those stay transparent. Rust's blanket call overwrites them incorrectly.

## Solution: Approach B — delete blanket function, use per-child calls

### Changes

1. **Delete `set_all_children_canvas_color`** from `emPanelCtx`.

2. **Replace each call site** with the C++ pattern:

| Rust file | C++ counterpart | C++ pattern | Rust fix |
|-----------|-----------------|-------------|----------|
| emColorField.rs | emColorField.cpp | emBorder::LayoutChildren() sets aux panel cc; Exp->Layout child gets NO cc | Remove `set_all_children_canvas_color` call entirely |
| emDialog.rs | emDialog.cpp | Both ContentPanel and ButtonsPanel get cc | `set_child_canvas_color` for each |
| emLinearGroup.rs | emLinearLayout.cpp | All children get cc in loop | `set_child_canvas_color` per child in loop, or use `layout_child_canvas` |
| emRasterGroup.rs | emRasterLayout.cpp | All children get cc in loop | Same |
| emPackGroup.rs | emPackLayout.cpp | All children get cc in loop | Same |
| emListBox.rs | inherits emRasterGroup | All children get cc | Same |
| emCoreConfigPanel.rs (8 sites) | Check C++ for each subpanel | Likely all children get cc | Per-child calls |
| composition.rs (test) | Test harness | Check if all children should get cc | Match test intent |
| test_panel.rs (2 sites) | Test harness | Check if all children should get cc | Match test intent |

3. **Verify**:
   - `python3 scripts/diff_draw_ops.py widget_colorfield` — STATE:canvas_color mismatch gone
   - `cargo test --test golden -- --test-threads=1` — 239+ pass (up from 229)
   - `cargo clippy -- -D warnings` — clean

## Key insight

For layout widgets where ALL children get cc, the behavioral change is nil — we're just calling `set_child_canvas_color` per child instead of blanket. The fix is in content widgets like emColorField where C++ intentionally leaves some children at transparent.

## Files touched

- `crates/emcore/src/emPanelCtx.rs` — delete `set_all_children_canvas_color`
- ~10 files replacing call sites with per-child calls
