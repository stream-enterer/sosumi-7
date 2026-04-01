# G1: 9-Slice Transform Setup Fix

## Objective

Fix `paint_9slice_section`'s transform setup to match C++ `PaintImage` → `PaintRect` → ScanlineTool::Init for each 9-slice section. Fix 23 G1 tests to pass at tol=0.

## Background

The area sampling inner loop (`interpolate_scanline_area_inner`) is now correct — it produces byte-identical output to C++ for the same inputs. The 23 G1 tests still fail because `paint_9slice_section` computes the `AreaSampleTransform` and `SectionBounds` differently from what C++ would produce for the same 9-slice section.

C++ routes each section through `PaintImage` → `PaintRect` → ScanlineTool::Init, which is the same well-tested path used for all image rendering. Rust's `paint_9slice_section` reimplements the transform setup inline, bypassing the normal `PaintImage` pipeline.

## Approach

Compare the Rust `paint_9slice_section` transform setup (lines 2904-2943 of `emPainter.rs`) against the C++ `PaintImage` → ScanlineTool::Init path. Fix divergences. Alternatively, refactor `paint_9slice_section` to delegate to the existing `PaintImage` code path rather than reimplementing the transform.

### C++ Reference

- `~/git/eaglemode-0.96.4/include/emCore/emPainter.h:1026-1037` — `PaintImage` (srcRect overload) → `PaintRect` with `emImageTexture`
- `~/git/eaglemode-0.96.4/src/emCore/emPainter.cpp:1892-1982` — `PaintBorderImage` calls `PaintImage` per section
- `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTl.cpp` — ScanlineTool::Init sets up TX/TY/TDX/TDY from texture parameters
- `~/git/eaglemode-0.96.4/include/emCore/emTexture.h` — `emImageTexture` constructor stores srcX/srcY/srcW/srcH

### Rust Files

- **Fix:** `crates/emcore/src/emPainter.rs` — `paint_9slice_section` (lines 2848-3000)
- **Reference:** `crates/emcore/src/emPainter.rs` — `area_sample_transform_24()`, `scale_transform_24()`, existing `PaintImage` / `paint_image_full` paths
- **Unchanged:** `crates/emcore/src/emPainterInterpolation.rs` — inner loop is correct

## The 23 G1 Tests

Same list as the G1 area sampling spec — all go through `PaintBorderImage` → `paint_9slice_section`.

## Verification

- `cargo test --test golden -- --test-threads=1` — 23 G1 tests must pass, no regressions
- `cargo clippy -- -D warnings` and `cargo-nextest ntr` must pass
- `parallel_benchmark` must pass

## Constraint

Full golden suite after every change. Pass count must never decrease.
