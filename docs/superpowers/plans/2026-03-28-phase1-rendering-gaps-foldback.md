# Phase 1: Rendering Gaps & .rust_only Fold-back

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close visible rendering gaps and eliminate all .rust_only files by folding code back into C++ header counterparts.

**Architecture:** Five fold-back operations (toolkit_images -> emBorder, widget_utils -> 8 callers, fixed -> emPainter, rect -> emPanel, emPainterDrawList deferred to Phase 4), plus emCrossPtr port with PanelPointerCache/OverwriteDialog features, plus missing toolkit images (Tunnel, Dir, DirUp).

**Tech Stack:** Rust, TGA image files from C++ source tree, existing golden/behavioral/pipeline test infrastructure.

**Spec:** `docs/superpowers/specs/2026-03-28-port-completion-design.md`

**Key rules from spec:**
- `// RUST_ONLY: <origin_file> -- <reason>` comment at every fold-back insertion point
- emPainter firewall: additive-only appends allowed, no modifications to existing lines
- No test assumed correct: audit tests before relying on them
- No standalone reimplementations: emTunnel's independent Tunnel.tga loading must use shared ToolkitImages after fold

---

## Task 1: Fold toolkit_images.rs into emBorder.rs

**Files:**
- Move from: `src/emCore/toolkit_images.rs`
- Move into: `src/emCore/emBorder.rs`
- Modify: `src/emCore/mod.rs`
- Modify: 8 callers that import from toolkit_images
- Delete: `src/emCore/toolkit_images.rs`
- Delete: `src/emCore/toolkit_images.rust_only`

**Context:** C++ has `struct TkResources` as a nested struct inside emBorder (include/emCore/emBorder.h:321-341). Rust extracted it into a separate file. This fold-back restores C++ correspondence.

- [ ] **Step 1: Read current state of emBorder.rs and toolkit_images.rs**

Read `src/emCore/emBorder.rs` (full file) and `src/emCore/toolkit_images.rs` to understand current structure. Identify where in emBorder.rs the ToolkitImages struct and accessor should be placed.

In emBorder.rs, find the section where `super::toolkit_images::with_toolkit_images` is called (around line 1798). The struct and accessor should go above the first usage, in a clearly demarcated section.

- [ ] **Step 2: Add ToolkitImages struct and accessor to emBorder.rs**

At an appropriate location near the top of emBorder.rs (after imports, before the main emBorder impl), add:

```rust
// RUST_ONLY: toolkit_images.rs -- compile-time TGA atlas extracted from
// C++ emBorder::TkResources (emBorder.h:321-341). C++ loads images at
// runtime via emGetResImage(); Rust embeds them via include_bytes!().

use std::cell::OnceCell;
use crate::emCore::emResTga::load_tga;

pub(crate) struct ToolkitImages {
    // ... all 15 fields from toolkit_images.rs, unchanged ...
}

fn decode(data: &[u8], name: &str, expected_w: u32, expected_h: u32) -> emImage {
    // ... unchanged from toolkit_images.rs ...
}

impl ToolkitImages {
    fn TryLoad() -> Self {
        // ... unchanged from toolkit_images.rs ...
    }
}

thread_local! {
    static TOOLKIT: OnceCell<ToolkitImages> = const { OnceCell::new() };
}

pub(crate) fn with_toolkit_images<R>(f: impl FnOnce(&ToolkitImages) -> R) -> R {
    TOOLKIT.with(|cell| f(cell.get_or_init(ToolkitImages::TryLoad)))
}
```

Copy the exact content from `src/emCore/toolkit_images.rs`, preserving all field names, decode logic, and include_bytes! paths. Add the `RUST_ONLY` comment at the insertion point.

- [ ] **Step 3: Update emBorder.rs internal references**

In emBorder.rs, change all occurrences of:
```rust
super::toolkit_images::with_toolkit_images(|img| {
```
to:
```rust
with_toolkit_images(|img| {
```
(Since `with_toolkit_images` is now defined in the same file, it no longer needs a module path.)

- [ ] **Step 4: Update external callers' imports**

These 7 files import from toolkit_images and need updating:

| File | Old import | New import |
|------|-----------|------------|
| `emButton.rs` | `use crate::emCore::toolkit_images::with_toolkit_images;` | `use crate::emCore::emBorder::with_toolkit_images;` |
| `emCheckBox.rs` | `use crate::emCore::toolkit_images::with_toolkit_images;` | `use crate::emCore::emBorder::with_toolkit_images;` |
| `emCheckButton.rs` | `use crate::emCore::toolkit_images::with_toolkit_images;` | `use crate::emCore::emBorder::with_toolkit_images;` |
| `emRadioBox.rs` | `use crate::emCore::toolkit_images::with_toolkit_images;` | `use crate::emCore::emBorder::with_toolkit_images;` |
| `emRadioButton.rs` | `use crate::emCore::toolkit_images::with_toolkit_images;` | `use crate::emCore::emBorder::with_toolkit_images;` |
| `emSplitter.rs` | `use crate::emCore::toolkit_images::with_toolkit_images;` | `use crate::emCore::emBorder::with_toolkit_images;` |

- [ ] **Step 5: Update mod.rs**

In `src/emCore/mod.rs`, remove:
```rust
pub mod toolkit_images;
```

The `with_toolkit_images` function is now exported from `emBorder` as `pub(crate)`.

- [ ] **Step 6: Delete old files**

Delete `src/emCore/toolkit_images.rs` and `src/emCore/toolkit_images.rust_only`.

- [ ] **Step 7: Audit existing tests touching toolkit_images**

Per "no test assumed correct" rule: grep tests/ for `toolkit_images` or `with_toolkit_images` or `ToolkitImages`. For any test found, verify it tests against C++ behavior, not just Rust's current output. If golden tests reference toolkit image rendering, verify the reference data provenance.

```bash
grep -rn "toolkit_images\|with_toolkit_images\|ToolkitImages" tests/
```

- [ ] **Step 8: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass. Any failure indicates a missed import update or incorrect move.

- [ ] **Step 9: Commit**

```bash
git add src/emCore/emBorder.rs src/emCore/mod.rs \
  src/emCore/emButton.rs src/emCore/emCheckBox.rs \
  src/emCore/emCheckButton.rs src/emCore/emRadioBox.rs \
  src/emCore/emRadioButton.rs src/emCore/emSplitter.rs
git rm src/emCore/toolkit_images.rs src/emCore/toolkit_images.rust_only
git commit -m "refactor: fold toolkit_images.rs into emBorder.rs

C++ has TkResources as a nested struct in emBorder (emBorder.h:321-341).
Restores file correspondence by moving ToolkitImages struct and accessor
into emBorder.rs with RUST_ONLY comment."
```

---

## Task 2: Inline widget_utils.rs into callers

**Files:**
- Delete: `src/emCore/widget_utils.rs`
- Delete: `src/emCore/widget_utils.rust_only`
- Modify: `src/emCore/mod.rs`
- Modify: `src/emCore/emButton.rs`
- Modify: `src/emCore/emCheckBox.rs`
- Modify: `src/emCore/emCheckButton.rs`
- Modify: `src/emCore/emRadioBox.rs`
- Modify: `src/emCore/emRadioButton.rs`
- Modify: `src/emCore/emColorField.rs`
- Modify: `src/emCore/emListBox.rs`
- Modify: `src/emCore/emTextField.rs`
- Modify: `src/emCore/emWindow.rs`

**Context:** C++ has the rounded-rect hit-test formula duplicated inline in each widget's `CheckMouse` method. Rust extracted it into a shared function. This fold-back restores C++ correspondence by inlining back into each caller.

`widget_utils.rs` contains two functions:
1. `check_mouse_round_rect(mx, my, rect, r) -> bool` — used by 8 widget files
2. `trace_input_enabled() -> bool` — used by 6 files (emButton, emCheckBox, emCheckButton, emRadioBox, emRadioButton, emWindow)

- [ ] **Step 1: Read each caller to understand context**

Read the following call sites to understand how each widget uses the functions:
- `emButton.rs` around lines 294, 301
- `emCheckBox.rs` around lines 220, 259
- `emCheckButton.rs` around lines 203, 210
- `emRadioBox.rs` around lines 226, 267
- `emRadioButton.rs` around lines 432, 444
- `emColorField.rs` around line 465
- `emListBox.rs` around line 1156
- `emTextField.rs` around line 2092
- `emWindow.rs` around line 793

- [ ] **Step 2: Inline check_mouse_round_rect into each caller**

For each of the 8 files that call `check_mouse_round_rect`, replace the function call with the inline formula. The pattern at each call site is:

```rust
// Before:
super::widget_utils::check_mouse_round_rect(mx, my, &rect, r)

// After (inline the formula directly):
{
    // RUST_ONLY: widget_utils.rs -- C++ inlines this formula per widget
    let dx = ((rect.x - mx).max(mx - rect.x - rect.w) + r).max(0.0);
    let dy = ((rect.y - my).max(my - rect.y - rect.h) + r).max(0.0);
    dx * dx + dy * dy <= r * r
}
```

Each call site has slightly different variable names for the rect and radius. Read the actual call site and adapt. The `RUST_ONLY` comment goes at each inline site.

- [ ] **Step 3: Inline trace_input_enabled into each caller**

For each of the 6 files that call `trace_input_enabled`, replace with inline code:

```rust
// Before:
let trace = super::widget_utils::trace_input_enabled();

// After:
// RUST_ONLY: widget_utils.rs -- debug trace aid, no C++ equivalent
let trace = {
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("TRACE_INPUT").is_ok())
};
```

Note: since each call site defines its own `static ENABLED`, these are technically separate statics. However, `OnceLock` is cheap and the env var check is idempotent, so this is functionally equivalent. If this is a concern, a single `static` could be placed in the crate root — but that diverges further from C++ where no such global exists.

- [ ] **Step 4: Remove widget_utils imports from all callers**

In each of the 8+ files, remove any line like:
```rust
use crate::emCore::widget_utils::check_mouse_round_rect;
```
or
```rust
use crate::emCore::widget_utils::trace_input_enabled;
```

Also check if `use crate::emCore::rect::Rect;` was only needed because of widget_utils. If so, the import might need to stay (since the inline formula uses `rect.x`, `rect.y`, etc. from a `Rect` parameter that's already in scope at the call site).

- [ ] **Step 5: Update mod.rs**

In `src/emCore/mod.rs`, remove:
```rust
pub(crate) mod widget_utils;
```

- [ ] **Step 6: Delete old files**

Delete `src/emCore/widget_utils.rs` and `src/emCore/widget_utils.rust_only`.

- [ ] **Step 7: Audit existing tests touching widget_utils callers**

Per "no test assumed correct" rule: grep tests/ for each widget that was modified (emButton, emCheckBox, etc.). For any test found that exercises CheckMouse or hit-testing, verify it tests against C++ behavior.

```bash
grep -rn "check_mouse\|CheckMouse\|hit.test" tests/
```

- [ ] **Step 8: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git rm src/emCore/widget_utils.rs src/emCore/widget_utils.rust_only
git add src/emCore/mod.rs src/emCore/emButton.rs src/emCore/emCheckBox.rs \
  src/emCore/emCheckButton.rs src/emCore/emRadioBox.rs \
  src/emCore/emRadioButton.rs src/emCore/emColorField.rs \
  src/emCore/emListBox.rs src/emCore/emTextField.rs src/emCore/emWindow.rs
git commit -m "refactor: inline widget_utils.rs back into each caller

C++ duplicates the rounded-rect hit-test formula in each widget's
CheckMouse method. Restores this pattern by inlining check_mouse_round_rect
and trace_input_enabled into each call site."
```

---

## Task 3: Fold fixed.rs into emPainter.rs

**Files:**
- Move from: `src/emCore/fixed.rs`
- Append to: `src/emCore/emPainter.rs`
- Modify: `src/emCore/emPainterScanline.rs` (import path change only)
- Modify: `src/emCore/mod.rs`
- Delete: `src/emCore/fixed.rs`
- Delete: `src/emCore/fixed.rust_only`

**Context:** C++ has bare `int` with inline shifts in emPainter.cpp:358-374. Rust extracted this into a Fixed12 newtype. Fold-back appends the type to emPainter.rs per the additive-only exception to the emPainter firewall.

**Firewall note:** This task modifies two emPainter* files:
1. `emPainter.rs` — additive-only append (allowed per spec)
2. `emPainterScanline.rs` — one-line import path change (minimal, documented here as a firewall exception; the change is `use crate::emCore::fixed::Fixed12` -> `use super::Fixed12`)

If the import change causes any complication, abort and log to `docs/empainter-deferred-refactors.log`.

- [ ] **Step 1: Read the end of emPainter.rs**

Read `src/emCore/emPainter.rs` lines 6600-6630 to confirm the file ends with Kani proofs and understand where to insert Fixed12.

The Fixed12 type and its tests should be inserted BEFORE the `#[cfg(kani)]` block at the end, so the file structure is: implementation -> Fixed12 type -> tests -> kani proofs.

- [ ] **Step 2: Append Fixed12 to emPainter.rs**

Insert the complete Fixed12 definition (struct, consts, impl, trait impls, Display) before the Kani section. Find the boundary between the last non-kani code and the `#[cfg(kani)]` block.

```rust
// ---------------------------------------------------------------------------
// RUST_ONLY: fixed.rs -- Fixed-point newtype for sub-pixel rasterization.
// C++ uses bare int with inline shifts (emPainter.cpp:358-374). Rust wraps
// in a newtype to prevent mixing fixed and integer values. ceil() and round()
// use i64 promotion to fix signed-overflow UB present in C++.
// ---------------------------------------------------------------------------

/// Fixed-point number with 12 fractional bits (4096 sub-pixel grid).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fixed12(i32);

// ... complete content from fixed.rs (struct, consts, impl, all trait impls) ...
// ... include the #[cfg(test)] mod tests block ...
```

Copy the EXACT content from `src/emCore/fixed.rs`. Do not modify any logic. The `pub struct` visibility stays `pub` (it's used by emPainterScanline.rs which is a sibling module).

- [ ] **Step 3: Update emPainter.rs import**

In emPainter.rs, remove:
```rust
use crate::emCore::fixed::Fixed12;
```

Fixed12 is now defined in the same file, so no import is needed. Any existing usage of `Fixed12` in emPainter.rs continues to work.

- [ ] **Step 4: Update emPainterScanline.rs import**

In `src/emCore/emPainterScanline.rs`, change:
```rust
use crate::emCore::fixed::Fixed12;
```
to:
```rust
use crate::emCore::emPainter::Fixed12;
```

This is the ONLY change to emPainterScanline.rs. Verify no other lines are modified.

- [ ] **Step 5: Update mod.rs**

In `src/emCore/mod.rs`, remove:
```rust
pub(crate) mod fixed;
```
(or whatever visibility it currently has)

- [ ] **Step 6: Delete old files**

Delete `src/emCore/fixed.rs` and `src/emCore/fixed.rust_only`.

- [ ] **Step 7: Audit existing tests touching Fixed12**

Per "no test assumed correct" rule: grep tests/ for `Fixed12` or `fixed`. Verify any Kani proofs for Fixed12 still reference the correct module path.

```bash
grep -rn "Fixed12\|fixed::" tests/
```

- [ ] **Step 8: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass including Fixed12 unit tests (now part of emPainter.rs).

- [ ] **Step 9: Commit**

```bash
git rm src/emCore/fixed.rs src/emCore/fixed.rust_only
git add src/emCore/emPainter.rs src/emCore/emPainterScanline.rs src/emCore/mod.rs
git commit -m "refactor: fold fixed.rs (Fixed12) into emPainter.rs

C++ uses bare int with inline shifts in emPainter.cpp:358-374. Appends
Fixed12 newtype to emPainter.rs (additive-only, no existing lines modified).
Updates emPainterScanline.rs import path."
```

---

## Task 4: Fold rect.rs into emPanel.rs

**Files:**
- Move from: `src/emCore/rect.rs`
- Move into: `src/emCore/emPanel.rs`
- Modify: `src/emCore/mod.rs`
- Modify: 27+ source files that import `rect::Rect`
- Modify: 6+ test files that import `rect::Rect`
- Delete: `src/emCore/rect.rs`
- Delete: `src/emCore/rect.rust_only`

**Context:** C++ has `GetLayoutX()`, `GetLayoutY()`, `GetLayoutWidth()`, `GetLayoutHeight()` as separate methods on emPanel (emPanel.h). Rust consolidated these into a `Rect` struct. The fold target is emPanel.rs since that's where the C++ concept originates. PixelRect is dead code and will be removed.

- [ ] **Step 1: Read rect.rs and verify PixelRect is dead code**

Read `src/emCore/rect.rs` fully. Grep for `PixelRect` across the entire codebase to confirm it has zero usage (only definition and documentation references).

Expected: PixelRect is defined but never imported or instantiated. Confirm before removing.

- [ ] **Step 2: Read emPanel.rs to find insertion point**

Read `src/emCore/emPanel.rs` to find where the Rect struct should be placed. It should go near the top (after imports, before the emPanel struct definition) since many emPanel methods return or accept Rect.

- [ ] **Step 3: Add Rect to emPanel.rs (without PixelRect)**

Insert Rect struct and its impl at the top of emPanel.rs:

```rust
// RUST_ONLY: rect.rs -- Consolidates C++ pattern of passing 4 separate
// doubles (GetLayoutX/Y/Width/Height in emPanel.h) into a typed struct.
// C++ has no dedicated layout rect type.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        // ... exact code from rect.rs ...
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        // ... exact code from rect.rs ...
    }

    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        // ... exact code from rect.rs ...
    }

    pub fn area(&self) -> f64 {
        // ... exact code from rect.rs ...
    }
}
```

Copy exact implementation from rect.rs. Do NOT include PixelRect.

- [ ] **Step 4: Update all 27+ source file imports**

Every file that has:
```rust
use crate::emCore::rect::Rect;
```
changes to:
```rust
use crate::emCore::emPanel::Rect;
```

Files to update (source):
emBorder.rs, emButton.rs, emCheckBox.rs, emCheckButton.rs, emDialog.rs,
emLinearLayout.rs, emListBox.rs, emPackLayout.rs, emPanel.rs (internal,
no import needed), emPanelCtx.rs, emPanelTree.rs, emRadioBox.rs,
emRadioButton.rs, emRasterLayout.rs, emScalarField.rs, emSplitter.rs,
emTextField.rs, emTunnel.rs, emView.rs, emViewAnimator.rs.

Note: widget_utils.rs was already deleted in Task 2. If Task 2 has not
been completed yet, also update widget_utils.rs (or coordinate ordering).

- [ ] **Step 5: Update test file imports**

Files to update (tests):
- `tests/unit/panel.rs`
- `tests/kani/proofs_layer3.rs`
- `tests/kani/proofs_generated.rs`
- `tests/golden/widget_interaction.rs`
- `tests/golden/layout.rs`
- `tests/pipeline/listbox.rs`

Same change: `rect::Rect` -> `emPanel::Rect`.

- [ ] **Step 6: Update mod.rs**

In `src/emCore/mod.rs`, remove:
```rust
pub(crate) mod rect;
```
(or whatever visibility it currently has)

Ensure emPanel re-exports Rect as `pub` (or `pub(crate)` — match current visibility of the Rect struct in rect.rs).

- [ ] **Step 7: Delete old files**

Delete `src/emCore/rect.rs` and `src/emCore/rect.rust_only`.

- [ ] **Step 8: Audit existing tests touching Rect**

Per "no test assumed correct" rule: the 6 test files that import Rect need auditing. Verify assertions match C++ layout behavior, not just Rust's current output.

```bash
grep -rn "Rect\|rect::" tests/
```

Pay special attention to `tests/kani/proofs_layer3.rs` and `tests/kani/proofs_generated.rs` — Kani proofs referencing Rect must have correct module paths after the move.

- [ ] **Step 9: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass.

- [ ] **Step 10: Commit**

```bash
git rm src/emCore/rect.rs src/emCore/rect.rust_only
git add src/emCore/emPanel.rs src/emCore/mod.rs \
  src/emCore/emBorder.rs src/emCore/emButton.rs \
  # ... all modified files ...
git commit -m "refactor: fold rect.rs (Rect) into emPanel.rs, remove dead PixelRect

C++ defines GetLayoutX/Y/Width/Height as separate emPanel methods
(emPanel.h). Rust's Rect struct consolidates these into a typed struct.
Moves Rect into emPanel.rs where the C++ concept originates. Removes
PixelRect which was never used."
```

---

## Task 5: Copy missing TGA files from C++ source

**Files:**
- Copy: `~/git/eaglemode-0.96.4/res/emCore/toolkit/Dir.tga` -> `res/toolkit/Dir.tga`
- Copy: `~/git/eaglemode-0.96.4/res/emCore/toolkit/DirUp.tga` -> `res/toolkit/DirUp.tga`

**Context:** Tunnel.tga already exists in Rust's `res/toolkit/`. Dir.tga and DirUp.tga are missing entirely. These are needed for file selection icons.

- [ ] **Step 1: Verify source files exist**

```bash
ls -la ~/git/eaglemode-0.96.4/res/emCore/toolkit/Dir.tga \
       ~/git/eaglemode-0.96.4/res/emCore/toolkit/DirUp.tga \
       ~/git/eaglemode-0.96.4/res/emCore/toolkit/Tunnel.tga
```

Expected: All three files exist.

- [ ] **Step 2: Verify Tunnel.tga already exists in Rust**

```bash
ls -la /home/a0/git/eaglemode-rs/res/toolkit/Tunnel.tga
```

Expected: File exists.

- [ ] **Step 3: Copy missing TGA files**

```bash
cp ~/git/eaglemode-0.96.4/res/emCore/toolkit/Dir.tga \
   /home/a0/git/eaglemode-rs/res/toolkit/Dir.tga
cp ~/git/eaglemode-0.96.4/res/emCore/toolkit/DirUp.tga \
   /home/a0/git/eaglemode-rs/res/toolkit/DirUp.tga
```

- [ ] **Step 4: Verify copies are identical**

```bash
diff ~/git/eaglemode-0.96.4/res/emCore/toolkit/Dir.tga \
     /home/a0/git/eaglemode-rs/res/toolkit/Dir.tga
diff ~/git/eaglemode-0.96.4/res/emCore/toolkit/DirUp.tga \
     /home/a0/git/eaglemode-rs/res/toolkit/DirUp.tga
```

Expected: No diff output (files identical).

- [ ] **Step 5: Commit**

```bash
git add res/toolkit/Dir.tga res/toolkit/DirUp.tga
git commit -m "assets: add Dir.tga and DirUp.tga from C++ source

Copies toolkit images for file selection icons from
eaglemode-0.96.4/res/emCore/toolkit/. Tunnel.tga already exists."
```

---

## Task 6: Add missing images to ToolkitImages struct

**Files:**
- Modify: `src/emCore/emBorder.rs` (ToolkitImages struct, after Task 1 fold)

**Context:** After Task 1, ToolkitImages is in emBorder.rs. It has 15 images but is missing 3: Tunnel, Dir, DirUp. C++ TkResources has all 18 (emBorder.h:321-341). After Task 5, all TGA files exist.

**Prerequisite:** Task 1 (toolkit_images folded into emBorder.rs) and Task 5 (TGA files copied).

- [ ] **Step 1: Read C++ TkResources to get exact image specs**

Read `~/git/eaglemode-0.96.4/include/emCore/emBorder.h` around lines 321-341 to get the exact field names for the 3 missing images. Also read `~/git/eaglemode-0.96.4/src/emCore/emBorder.cpp` around lines 44-63 to get the filenames and expected dimensions.

- [ ] **Step 2: Add 3 new fields to ToolkitImages struct**

In `src/emCore/emBorder.rs`, add to the ToolkitImages struct:

```rust
pub(crate) struct ToolkitImages {
    // ... existing 15 fields ...
    pub tunnel: emImage,
    pub dir: emImage,
    pub dir_up: emImage,
}
```

Field names should match C++ TkResources field names per file correspondence. Check the C++ header for the exact names (likely `ImgTunnel`, `ImgDir`, `ImgDirUp`). Use the C++ names with appropriate Rust casing per project convention.

- [ ] **Step 3: Add decode calls in TryLoad**

In the `TryLoad()` method, add:

```rust
tunnel: decode(
    include_bytes!("../../res/toolkit/Tunnel.tga"),
    "Tunnel",
    EXPECTED_W,  // get from C++ source or by reading the TGA header
    EXPECTED_H,
),
dir: decode(
    include_bytes!("../../res/toolkit/Dir.tga"),
    "Dir",
    EXPECTED_W,
    EXPECTED_H,
),
dir_up: decode(
    include_bytes!("../../res/toolkit/DirUp.tga"),
    "DirUp",
    EXPECTED_W,
    EXPECTED_H,
),
```

Get exact dimensions from C++ source or by decoding the TGA headers (the decode function will assert dimensions match).

- [ ] **Step 4: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass. The new fields don't affect existing code paths (no callers reference them yet).

- [ ] **Step 5: Commit**

```bash
git add src/emCore/emBorder.rs
git commit -m "feat: add Tunnel, Dir, DirUp images to ToolkitImages

Completes the C++ TkResources image set (18/18). Images are embedded
at compile time via include_bytes!()."
```

---

## Task 7: Refactor emTunnel.rs to use shared ToolkitImages

**Files:**
- Modify: `src/emCore/emTunnel.rs`

**Context:** emTunnel.rs loads Tunnel.tga independently via its own thread-local OnceCell + include_bytes!. This is a standalone reimplementation of toolkit image loading (spec: "no standalone reimplementations"). After Task 6, Tunnel is available in ToolkitImages.

**Prerequisite:** Task 1 (fold into emBorder) and Task 6 (Tunnel added to struct).

- [ ] **Step 1: Read emTunnel.rs tunnel_image function**

Read `src/emCore/emTunnel.rs` lines 1-30 to understand the current loading pattern and where the image is used.

- [ ] **Step 2: Replace standalone loading with shared accessor**

Remove the `tunnel_image()` function and its thread-local. Replace all call sites with `with_toolkit_images`:

```rust
// Before:
fn tunnel_image() -> emImage {
    thread_local! { ... }
    ...
}

// somewhere later:
let img = tunnel_image();

// After:
use crate::emCore::emBorder::with_toolkit_images;

// somewhere later:
with_toolkit_images(|imgs| {
    // use imgs.tunnel instead of tunnel_image()
    ...
})
```

Read the full call sites to understand how tunnel_image() is used and adapt the with_toolkit_images closure pattern accordingly. The image may need to be cloned out of the closure if it's used outside, or the rendering code may need to move inside the closure.

- [ ] **Step 3: Remove dead imports**

Remove `OnceCell`, `load_tga`, and `include_bytes!` imports if they're no longer needed after the refactor. Keep any that are still used for other purposes.

- [ ] **Step 4: Audit existing emTunnel tests**

Before running tests, check if any tests for emTunnel exist and whether they test the image loading path. Per the "no test assumed correct" rule, verify that any golden tests for tunnel rendering were generated from C++ output.

```bash
grep -r "tunnel" tests/ --include='*.rs' -l
```

- [ ] **Step 5: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass. Tunnel rendering should be identical since the same TGA file is used.

- [ ] **Step 6: Commit**

```bash
git add src/emCore/emTunnel.rs
git commit -m "refactor: use shared ToolkitImages for Tunnel in emTunnel.rs

Removes standalone Tunnel.tga loading (thread-local OnceCell + include_bytes).
Uses with_toolkit_images() accessor from emBorder, matching C++ pattern
where emTunnel gets its image from TkResources."
```

---

## Task 8: Port emCrossPtr

**Files:**
- Create: `src/emCore/emCrossPtr.rs`
- Modify: `src/emCore/mod.rs`
- Delete: `src/emCore/emCrossPtr.no_rs`
- Test: `tests/behavioral/cross_ptr.rs` (new)
- Modify: `tests/behavioral/main.rs`

**Context:** C++ emCrossPtr is a weak reference with explicit invalidation via intrusive linked list. emCrossPtrList is embedded in target objects; its destructor calls BreakCrossPtrs() which sets all pointers to NULL before cleanup completes. Current Rust uses bare Weak<T> which invalidates only when the last Rc drops.

**C++ API surface to port:**

emCrossPtr<T>:
- `new()` — null pointer
- `new(target)` — links to target
- `get() -> Option<Rc<RefCell<T>>>` — returns target if valid
- `set(target)` — rebind
- `reset()` — unbind
- `is_valid() -> bool` — true if target alive and not explicitly invalidated

emCrossPtrList:
- `new()` — empty list
- `link(ptr)` — register a cross pointer
- `break_cross_ptrs()` — invalidate all pointers (called early in drop)

- [ ] **Step 1: Investigate C++ BreakCrossPtrs usage**

Grep the C++ source to find every call to BreakCrossPtrs():

```bash
grep -rn "BreakCrossPtrs" ~/git/eaglemode-0.96.4/include/ ~/git/eaglemode-0.96.4/src/
```

Document which destructors call it and whether any code checks a cross pointer between BreakCrossPtrs and the end of the destructor. This determines whether the timing difference matters in practice.

Regardless of the finding, we port the explicit invalidation per the spec.

- [ ] **Step 2: Write failing behavioral tests**

Create `tests/behavioral/cross_ptr.rs`:

```rust
use eaglemode_rs::emCore::emCrossPtr::{emCrossPtr, emCrossPtrList};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn null_cross_ptr() {
    let ptr: emCrossPtr<String> = emCrossPtr::new();
    assert!(!ptr.is_valid());
    assert!(ptr.get().is_none());
}

#[test]
fn link_and_access() {
    let target = Rc::new(RefCell::new("hello".to_string()));
    let mut list = emCrossPtrList::new();
    let ptr = emCrossPtr::from_target(&target, &mut list);
    assert!(ptr.is_valid());
    assert_eq!(*ptr.get().unwrap().borrow(), "hello");
}

#[test]
fn break_cross_ptrs_invalidates() {
    let target = Rc::new(RefCell::new(42));
    let mut list = emCrossPtrList::new();
    let ptr = emCrossPtr::from_target(&target, &mut list);
    assert!(ptr.is_valid());

    list.break_cross_ptrs();
    // Target still exists (Rc is alive) but cross pointer is invalid
    assert!(!ptr.is_valid());
    assert!(ptr.get().is_none());
    // Target itself is still accessible
    assert_eq!(*target.borrow(), 42);
}

#[test]
fn multiple_ptrs_all_invalidated() {
    let target = Rc::new(RefCell::new(0));
    let mut list = emCrossPtrList::new();
    let p1 = emCrossPtr::from_target(&target, &mut list);
    let p2 = emCrossPtr::from_target(&target, &mut list);
    let p3 = emCrossPtr::from_target(&target, &mut list);

    list.break_cross_ptrs();
    assert!(!p1.is_valid());
    assert!(!p2.is_valid());
    assert!(!p3.is_valid());
}

#[test]
fn rebind_to_different_target() {
    let t1 = Rc::new(RefCell::new(1));
    let t2 = Rc::new(RefCell::new(2));
    let mut list1 = emCrossPtrList::new();
    let mut list2 = emCrossPtrList::new();

    let mut ptr = emCrossPtr::from_target(&t1, &mut list1);
    assert_eq!(*ptr.get().unwrap().borrow(), 1);

    ptr.set(&t2, &mut list2);
    assert_eq!(*ptr.get().unwrap().borrow(), 2);

    list1.break_cross_ptrs();
    // ptr was rebound to t2, so breaking list1 doesn't affect it
    assert!(ptr.is_valid());
}

#[test]
fn drop_list_breaks_ptrs() {
    let target = Rc::new(RefCell::new("test".to_string()));
    let ptr;
    {
        let mut list = emCrossPtrList::new();
        ptr = emCrossPtr::from_target(&target, &mut list);
        assert!(ptr.is_valid());
        // list drops here — should call break_cross_ptrs
    }
    assert!(!ptr.is_valid());
}
```

Add `mod cross_ptr;` to `tests/behavioral/main.rs`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --test behavioral cross_ptr`
Expected: Compilation error — `emCrossPtr` module doesn't exist yet.

- [ ] **Step 4: Implement emCrossPtr**

Create `src/emCore/emCrossPtr.rs`:

```rust
//! emCrossPtr — weak reference with explicit invalidation.
//!
//! C++ emCrossPtr (emCrossPtr.h) uses an intrusive linked list so that
//! a target object can invalidate all pointers to itself early in its
//! destructor (before cleanup), via BreakCrossPtrs(). Rust's Weak<T>
//! only invalidates when the last Rc drops.
//!
//! This module replicates the C++ semantics: emCrossPtrList is embedded
//! in the target and calls break_cross_ptrs() on Drop. emCrossPtr checks
//! both the explicit invalidation flag AND Weak::upgrade().

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// Shared invalidation flag. When break_cross_ptrs() is called, the flag
/// is set to false, causing all emCrossPtrs sharing this flag to report
/// invalid even if the Rc target is still alive.
struct InvalidationFlag(Cell<bool>);

/// A weak reference that supports explicit early invalidation.
///
/// Corresponds to C++ `emCrossPtr<CLS>` (emCrossPtr.h).
pub struct emCrossPtr<T> {
    target: Weak<RefCell<T>>,
    valid: Option<Rc<InvalidationFlag>>,
}

/// Embedded in the target object. Manages all cross pointers pointing to it.
/// Calls break_cross_ptrs() on Drop, matching C++ emCrossPtrList destructor.
///
/// Corresponds to C++ `emCrossPtrList` (emCrossPtr.h).
pub struct emCrossPtrList {
    /// Shared flag — all emCrossPtrs linked to this list hold an Rc to it.
    /// When break_cross_ptrs() sets it to false, all pointers see it.
    flag: Rc<InvalidationFlag>,
}

impl<T> emCrossPtr<T> {
    /// Create a null cross pointer.
    pub fn new() -> Self {
        Self {
            target: Weak::new(),
            valid: None,
        }
    }

    /// Create a cross pointer linked to a target.
    pub fn from_target(target: &Rc<RefCell<T>>, list: &emCrossPtrList) -> Self {
        Self {
            target: Rc::downgrade(target),
            valid: Some(Rc::clone(&list.flag)),
        }
    }

    /// Check if the pointer is valid (target alive AND not explicitly invalidated).
    pub fn is_valid(&self) -> bool {
        match &self.valid {
            Some(flag) => flag.0.get() && self.target.strong_count() > 0,
            None => false,
        }
    }

    /// Get the target if valid.
    pub fn get(&self) -> Option<Rc<RefCell<T>>> {
        if self.is_valid() {
            self.target.upgrade()
        } else {
            None
        }
    }

    /// Rebind to a different target.
    pub fn set(&mut self, target: &Rc<RefCell<T>>, list: &emCrossPtrList) {
        self.target = Rc::downgrade(target);
        self.valid = Some(Rc::clone(&list.flag));
    }

    /// Unbind (set to null).
    pub fn reset(&mut self) {
        self.target = Weak::new();
        self.valid = None;
    }
}

impl<T> Default for emCrossPtr<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl emCrossPtrList {
    /// Create an empty cross pointer list.
    pub fn new() -> Self {
        Self {
            flag: Rc::new(InvalidationFlag(Cell::new(true))),
        }
    }

    /// Invalidate all cross pointers linked to this list.
    /// Matches C++ BreakCrossPtrs() semantics.
    pub fn break_cross_ptrs(&self) {
        self.flag.0.set(false);
    }
}

impl Default for emCrossPtrList {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for emCrossPtrList {
    fn drop(&mut self) {
        self.break_cross_ptrs();
    }
}
```

**Design notes:**
- Instead of C++'s intrusive linked list (which would require unsafe in Rust), we use a shared `Rc<InvalidationFlag>`. All emCrossPtrs linked to the same list share the same flag. When `break_cross_ptrs()` sets the flag to false, all pointers see it immediately.
- This is O(1) for break_cross_ptrs (vs C++'s O(n) linked list walk), and doesn't require unsafe.
- The `is_valid()` check is: flag is true AND Weak has a live target. This means invalidation happens at whichever comes first: explicit break OR Rc drop.

- [ ] **Step 5: Add to mod.rs**

In `src/emCore/mod.rs`, add:
```rust
pub mod emCrossPtr;
```

- [ ] **Step 6: Run tests**

Run: `cargo test --test behavioral cross_ptr -v`
Expected: All 6 tests pass.

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: Full suite passes.

- [ ] **Step 7: Delete marker file**

Delete `src/emCore/emCrossPtr.no_rs`.

- [ ] **Step 8: Commit**

```bash
git rm src/emCore/emCrossPtr.no_rs
git add src/emCore/emCrossPtr.rs src/emCore/mod.rs \
  tests/behavioral/cross_ptr.rs tests/behavioral/main.rs
git commit -m "feat: port emCrossPtr with explicit invalidation

Implements emCrossPtr<T> and emCrossPtrList matching C++ semantics
(emCrossPtr.h). Uses shared Rc<InvalidationFlag> instead of C++ intrusive
linked list. break_cross_ptrs() invalidates all pointers immediately,
matching C++ early-destructor timing."
```

---

## Task 9: Implement PanelPointerCache in emBorder

**Files:**
- Modify: `src/emCore/emBorder.rs`
- Test: `tests/behavioral/panel_pointer_cache.rs` (new) or extend existing emBorder tests

**Context:** C++ emBorder::AuxData has `emCrossPtr<emPanel> PanelPointerCache` which caches a child panel reference by name. When the child is deleted, the cross pointer auto-nullifies, forcing a fresh lookup. Rust emBorder currently has no equivalent.

**Prerequisite:** Task 8 (emCrossPtr ported).

- [ ] **Step 1: Read C++ implementation details**

Read `~/git/eaglemode-0.96.4/include/emCore/emBorder.h` around line 410 (AuxData struct) and `~/git/eaglemode-0.96.4/src/emCore/emBorder.cpp` around lines 254, 298-301, 414-418 to understand:
- How PanelPointerCache is used in GetAuxPanel()
- When it's reset (panel name changes)
- How it interacts with LayoutChildren()

- [ ] **Step 2: Read Rust emBorder.rs AuxData equivalent**

Read `src/emCore/emBorder.rs` to find:
- Whether an AuxData equivalent exists
- Where GetAuxPanel or equivalent is implemented
- Where LayoutChildren references aux panels

This determines where PanelPointerCache needs to be added.

- [ ] **Step 3: Write failing test**

Write a behavioral test that:
1. Creates an emBorder with an aux panel
2. Calls GetAuxPanel() — should return the panel
3. Deletes the aux panel
4. Calls GetAuxPanel() — should return None (cache invalidated)
5. Adds a new panel with the same name
6. Calls GetAuxPanel() — should return the new panel (cache refreshed)

The exact test depends on what the Rust emBorder API looks like. Adapt after reading the code.

- [ ] **Step 4: Implement PanelPointerCache**

Add `emCrossPtr<emPanel>` to the Rust AuxData equivalent. Implement the caching pattern:
- On first GetAuxPanel() call, look up child by name and cache
- On subsequent calls, return cached pointer if valid
- On panel name change, reset cache
- Cross pointer auto-invalidates if child panel is dropped

- [ ] **Step 5: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/emCore/emBorder.rs tests/behavioral/...
git commit -m "feat: implement PanelPointerCache in emBorder using emCrossPtr

Caches child panel reference by name, matching C++ emBorder::AuxData
pattern. Cross pointer auto-invalidates when child panel is deleted."
```

---

## Task 10: Implement OverwriteDialog in emFileDialog

**Files:**
- Modify: `src/emCore/emFileDialog.rs`
- Test: extend existing behavioral tests or create new

**Context:** C++ emFileDialog has `emCrossPtr<emDialog> OverwriteDialog` to track a dynamically-created overwrite confirmation dialog. Rust has stub fields (`overwrite_asked`, `overwrite_confirmed`) but no cross-pointer-based dialog tracking.

**Prerequisite:** Task 8 (emCrossPtr ported).

- [ ] **Step 1: Read C++ implementation**

Read `~/git/eaglemode-0.96.4/src/emCore/emFileDialog.cpp` around lines 90-103 (Cycle checking dialog), 186-195 (CheckFinish creating dialog) to understand the full dialog lifecycle.

- [ ] **Step 2: Read Rust emFileDialog.rs**

Read `src/emCore/emFileDialog.rs` fully to understand:
- Current state of the overwrite flow
- What `overwrite_asked` and `overwrite_confirmed` fields do
- Where the dialog would be created and checked
- Whether emDialog is ported and available

- [ ] **Step 3: Write failing test**

Write a behavioral test covering the overwrite dialog lifecycle:
1. Set up emFileDialog in save mode
2. Trigger save to existing file
3. Verify overwrite dialog is created
4. Confirm/cancel the dialog
5. Verify dialog is cleaned up (cross pointer invalidated)

Adapt based on what the Rust API looks like.

- [ ] **Step 4: Implement OverwriteDialog**

Add `emCrossPtr<emDialog>` field. Implement:
- Dialog creation in the finish-check path
- Dialog lifecycle tracking via cross pointer
- Cleanup when dialog is dismissed

- [ ] **Step 5: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/emCore/emFileDialog.rs tests/...
git commit -m "feat: implement OverwriteDialog in emFileDialog using emCrossPtr

Tracks dynamically-created overwrite confirmation dialog via emCrossPtr,
matching C++ emFileDialog pattern. Cross pointer auto-invalidates when
dialog is dismissed."
```

---

## Task 11: Update CORRESPONDENCE.md

**Files:**
- Modify: `src/emCore/CORRESPONDENCE.md`

**Prerequisite:** All previous tasks completed.

- [ ] **Step 1: Read current CORRESPONDENCE.md**

Read `src/emCore/CORRESPONDENCE.md` to understand current structure.

- [ ] **Step 2: Update state of the port section**

Update the opening section to reflect:
- .rust_only file count is now 1 (emPainterDrawList.rs, deferred to Phase 4)
- emCrossPtr.no_rs is deleted (ported)
- Concrete rendering gaps closed: ImgTunnel, ImgDir, ImgDirUp, PanelPointerCache, OverwriteDialog
- Update any counts that changed

- [ ] **Step 3: Update patterns section**

In the cross-file patterns section:
- Update "Concrete rendering/feature gaps" to remove closed items
- Update emCrossPtr BreakCrossPtrs timing section with investigation results
- Add any new findings from Phase 1 work

- [ ] **Step 4: Run tests**

Run: `cargo clippy -- -D warnings && cargo-nextest ntr`
Expected: All tests pass (no code changes, just docs).

- [ ] **Step 5: Commit**

```bash
git add src/emCore/CORRESPONDENCE.md
git commit -m "docs: update CORRESPONDENCE.md for Phase 1 completion

Reflects .rust_only fold-backs, emCrossPtr port, closed rendering gaps,
and investigation findings."
```

---

## Task 12: Phase 1 review checkpoint

**No files changed.** This is a review gate.

- [ ] **Step 1: Verify all .rust_only files resolved**

```bash
ls src/emCore/*.rust_only
```

Expected: Only `emPainterDrawList.rust_only` remains (deferred to Phase 4).

- [ ] **Step 2: Verify emCrossPtr.no_rs deleted**

```bash
ls src/emCore/emCrossPtr.no_rs
```

Expected: File not found.

- [ ] **Step 3: Verify full test suite passes**

```bash
cargo clippy -- -D warnings && cargo-nextest ntr && cargo test --test golden -- --test-threads=1
```

Expected: All tests pass including golden tests.

- [ ] **Step 4: Review empainter-deferred-refactors.log**

Check if `docs/empainter-deferred-refactors.log` was created during any task. If so, review its contents and document for Phase 4.

- [ ] **Step 5: Report findings**

Summarize to user:
- What was completed
- What was deferred
- Any surprises from the emCrossPtr investigation
- Readiness for Phase 2
