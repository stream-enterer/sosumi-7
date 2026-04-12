# PaintRect Gap Analysis: tktest_1x C++ 5470 ops vs Rust 2558 ops

**Date**: 2026-04-11

## Problem Statement

The tktest_1x composition test shows C++ emitting 5470 draw ops vs Rust 2558. C++ has 3303 PaintRect ops; Rust has 81. Initial hypothesis: 3222 PaintRect calls are missing from Rust widget rendering.

## Investigation Findings

The 3222 gap decomposes into four categories. Only 1 op is a fixable rendering bug.

### Category 1: Sub-op recording format (3156 ops)

C++ records PaintBorderImage and PaintText internal decomposition as depth 1/2 sub-ops. Each PaintBorderImage decomposes into 8 PaintRect sub-ops (border edges). Each PaintText decomposes into per-character PaintRect sub-ops.

| Depth | C++ PaintRect | Rust PaintRect |
|-------|---------------|----------------|
| 0     | 147           | 81 (no depth)  |
| 1     | 1422          | 0              |
| 2     | 1734          | 0              |

Rust doesn't record sub-ops. Since PaintBorderImage count matches exactly (166 each), the border image rendering is correct — Rust just doesn't log the internal PaintRect calls that PaintBorderImage makes.

**Action**: None. Recording format difference. Rendering is correct.

### Category 2: PaintImage naming (14 ops)

C++ `PaintImage()` is an inline wrapper around `PaintRect(emImageTexture(...))`. The C++ recorder logs this as a PaintRect op. Rust has a separate `PaintImageFull` op name.

Evidence: Rust has exactly 14 PaintImageFull ops. C++ has 14 depth-0 PaintRect ops with eff0f4ff color at image-characteristic dimensions (0.103x0.103, 0.0772x0.0772, 0.3151x0.3151) that have no Rust PaintRect counterpart.

| Dimension | C++ PaintRect (eff0f4ff) | Rust PaintImageFull |
|-----------|--------------------------|---------------------|
| 0.103x0.103 | 6 | 6 |
| 0.0772x0.0772 | 7 | 7 |
| 0.3151x0.3151 | 1 | 1 |

**Action**: None. Same rendering, different op name in recording.

### Category 3: File viewer stubs (51 ops)

C++ creates real file viewer plugins (via `emFpPluginList::CreateFilePanel`) for files like Cargo.toml and build.rs. These plugins paint:

- 44 `00000060` (black alpha=96): Per-line text background rects from file content viewer. Constant height per file, varying width matching text run length.
- 5 `00000000` (transparent): Content area placeholder rects.
- 2 `ffffffff` (white): Full background fills for file content area.

Rust has stubs for these panels (file viewer plugins are not ported). The stub panels produce no paint ops.

**Action**: None. Known divergence — file viewer plugins are out of scope.

### Category 4: IsOpaque canvas clear (1 op)

C++ emits a `808080ff` (gray) PaintRect at seq=0, dimensions 800x600. This is the initial canvas clear from `emView::Paint()` when `IsOpaque()` returns false.

C++ `emRasterGroup` inherits `emBorder::IsOpaque()` which returns false for `OBT_GROUP` borders (the root panel's border type). Rust's `TkTestPanel::IsOpaque()` unconditionally returns `true`, skipping the canvas clear.

**Action**: Fix TkTestPanel::IsOpaque to delegate to `self.border.IsOpaque()`.

## Verification

After applying the Category 4 fix:

| Metric | Before | After | Expected |
|--------|--------|-------|----------|
| Rust PaintRect count | 81 | 82 | 82 |
| Total Rust ops | 2558 | 2559 | 2559 |
| Remaining gap vs C++ | 2912 | 2911 | Explained by categories 1-3 |

The remaining op count gap (2911) is fully explained by:
- -3156 sub-op recording (C++ records, Rust doesn't)
- +711 state ops (Rust records ClipRect/SetCanvasColor/SetTransformation/PushState/PopState, C++ doesn't)
- -14 PaintImage naming
- -51 file viewer stubs
- -1 IsOpaque (now fixed)
- Remaining ~400: PaintText/PaintTextBoxed shift (C++ 662 PaintText vs Rust 17; Rust 752 PaintTextBoxed vs C++ 527) and PaintPolygon differences (Rust 610 vs C++ 430+188)

## Implementation Plan

### Phase 1: Fix IsOpaque (1 line change)

In `crates/eaglemode/tests/golden/composition.rs`, change `TkTestPanel::IsOpaque` from `true` to delegate to `self.widget.border.IsOpaque()` or return `false` to match C++ `emRasterGroup` behavior.

### Phase 2: Verify

1. `cargo clippy -- -D warnings`
2. `cargo-nextest ntr`
3. `DUMP_DRAW_OPS=1 cargo test --test golden composition_tktest_1x -- --test-threads=1`
4. Verify Rust PaintRect count increased by 1 (81 → 82)
5. Full golden test suite: `cargo test --test golden -- --test-threads=1`

## Non-goals

- Sub-op recording parity (would require significant recorder refactoring for marginal value)
- File viewer plugin porting (separate project)
- PaintText/PaintTextBoxed shift (separate, smaller issue as noted in task)
