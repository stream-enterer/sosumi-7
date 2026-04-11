# Init State Comparison: C++ vs Rust

## Methodology

Compared C++ `ScanlineTool::Init` derived state (from `/tmp/init_dump.txt`, 9810 entries
dumped via `EM_DUMP_INIT=1`) against Rust `paint_9slice_section` + `area_sample_transform_24`
for the `widget_checkbox_unchecked` golden test (800x600 viewport, divergent pixels at
x=24-35, y=288-305, max_diff=13).

C++ source: `emPainter_ScTl.cpp` lines 25-396 (Init function).
Rust source: `emPainter.rs` lines 2838-3003 (paint_9slice_section) and lines 5936-5980
(area_sample_transform_24).

## Entries overlapping divergent region (x=[24,36], y=[288,306])

| Line | ImgW | ImgH | Ch | ImgDX | TDX | Pixel extent | Type |
|------|------|------|----|-------|-----|-------------|------|
| 92 | 143 | 20 | 4 | 8 | 30925166 | 10..88, 88..512 | group_border left side |
| 107 | 128 | 224 | 1 | 1 | 31412659 | 24..92, 180..420 | Ch=1 gradient overlay |
| 121 | 340 | 24 | 4 | 4 | 36789500 | 10..165, 165..435 | button_border left side |
| 897 | 128 | 224 | 1 | 1 | 38010460 | 33..89, 251..349 | Ch=1 inner border |
| 2286 | 286 | 20 | 4 | 4 | 46537719 | 14..117, 117..683 | outer_border left side |
| 9790 | 113 | 20 | 4 | 8 | 26481263 | 26..97, 151..303 | checkbox btn left side |
| 9792 | 113 | 113 | 4 | 8 | 26481263 | 26..97, 303..374 | checkbox btn BL corner |

Multiple layers are composited at the divergent location. The checkbox button border
(lines 9790, 9792) is the most specific to the checkbox widget.

## Structural Formula Comparison

### Transform computation (Init)

Every step maps 1:1 between C++ and Rust:

| Step | C++ (emPainter_ScTl.cpp) | Rust (emPainter.rs) | Match? |
|------|--------------------------|---------------------|--------|
| tw (dest pixel width) | `texture.GetW() * ScaleX` (line 285) | `dw * scale_x` (line 5945) | YES (same double value) |
| Initial tdx | `(((emInt64)ImgW)<<24)/tw` (line 287) | `((sw_u as i64) << 24) as f64 / dw_px` (line 2899) | YES |
| TDX (initial integer) | `(emInt64)tdx` (line 293) | `tdx_init as i64` (line 2901) | YES |
| Stride n | `(TDX/3+0xFFFFFF)>>24` (line 314) | `((tdx_i / 3 + 0xFFFFFF) >> 24)` (line 2905) | YES |
| Reduced ImgW | `(ImgW+n-1)/n` (line 318) | `sw_u.div_ceil(stride_x)` (line 2917) | YES |
| Reduced tdx | `ImgW*((emInt64)1<<24)/tw` (line 323) | `((src_w as i64) << 24) as f64 / tw` (line 5947) | YES |
| TDX (final) | `(emInt64)tdx` (line 324) | `tdx_f64 as i64` (line 5949) | YES |
| TX | `(emInt64)(tx * tdx)` (line 338) | `(tx_sub * tdx_f64) as i64` (line 5954) | YES |
| ODX | `(((emInt64)1<<40)-1)/TDX+1` (line 340) | `((1i64 << 40) - 1) / tdx + 1` (line 5959) | YES |
| off_x | `(ImgW_orig-(ImgW_red-1)*n-1)/2` (line 319) | `(sw_u-(red_w-1)*stride_x-1)/2` (line 2920) | YES |

**Key finding: Issue #2 ("Rust does NOT recompute tdx after stride reduction") is NOT
present in the current code.** Line 2923 calls `area_sample_transform_24(red_w, red_h, ...)`
with the REDUCED dimensions, which recomputes tdx from red_w. The tdx_init at line 2899
is only used for stride computation, not for the final transform.

### Downscale branch condition

| | C++ | Rust |
|--|-----|------|
| Condition | `TDX > 0xFFFF00 \|\| TDY > 0xFFFF00` (line 296) | `ratio_x > 1.0 \|\| ratio_y > 1.0` (line 2885) |
| Effect | Enters area sampling when any axis ratio >= 0.999985 | Enters area sampling when any axis ratio > 1.0 |
| Gap | ratio in (0.999985, 1.0] treated as downscale in C++, upscale in Rust |

This is a micro-divergence. For the checkbox test, all border sections have ratios well
above 1.0 (large source images downscaled to small dest), so this condition does not
trigger differently.

### Interpolation kernel (area sampling)

The Rust code in `interpolate_scanline_area_inner` (emPainterInterpolation.rs:1266)
is a literal translation of C++ `InterpolateImageAreaSampled` (emPainter_ScTlIntImg.cpp:677).

| Operation | C++ | Rust | Match? |
|-----------|-----|------|--------|
| Y weight (oy1) | `((0x1000000-(ty1&0xffffff))*(emInt64)ody+0xffffff)>>24` | `((0x100_0000 - (ty1 & 0xFF_FFFF)) as u64 * ody as u64 + 0xFF_FFFF) >> 24` | YES |
| READ_PREMUL_MUL (4ch) | `cy_a=p[3]*oy1; cy_r=p[0]*cy_a` (u32) | `ca = p[3] as u64 * oy1 as u64; cr = p[0] as u64 * ca` (u64) | YES (no overflow) |
| FINPREMUL (4ch RGB) | `(cy_r + 0x7F7F) / 0xFF00` | `(cr + 0x7F7F) / 0xFF00` | YES |
| FINPREMUL (4ch alpha) | `(cy_a + 0x7F) >> 8` | `(ca + 0x7F) >> 8` | YES |
| Output (WRITE_NO_ROUND_SHR) | `(cyx >> 24) as u8` | `(cyx[ch] >> 24) as u8` | YES |
| u32 overflow risk | max cyx = 0xFF7FFFFF, fits u32 | u64, no risk | N/A (both safe) |
| pCy carry | `pCy` pointer comparison | `pcy_col` index comparison | Equivalent |

### Pixel access (stride + section)

| | C++ | Rust |
|--|-----|------|
| Access | `ImgMap + col * ImgDX` where ImgMap is pre-offset by `(t>>1)*channels` | `image.GetPixel(sec.ox + off_x + col * stride_x, sec.oy + off_y + row * stride_y)` |
| Boundary | Implicit via tx1/tx2 range checks | `.clamp(0, sec.w - 1)` in read_area_pixel |

These are equivalent for interior pixels. Edge pixels may differ in boundary handling
but this should only affect pixels at the very edge of each section.

### Blend/compositing

| | C++ (emPainter_ScTlPSInt.cpp) | Rust (emPainterScanlineTool.rs) |
|--|-------------------------------|----------------------------------|
| Source-over, a >= 255 | `*p = pix` (RGB only, alpha=0) | `dest[0..3] = pm[0..3]; dest[3] = 255` | DIVERGENCE (alpha) |
| Source-over, a < 255 | `*p = blended_rgb + pix` (alpha=0) | `dest[ch] = ((old * t + 0x8073) >> 16) + src_ch` | DIVERGENCE (alpha) |
| RGB hash lookup | `hR[r] + hG[g] + hB[b]` (identity for range=255) | Direct `pm[0], pm[1], pm[2]` | Equivalent |
| Blinn div255 | `(x * 257 + 0x8073) >> 16` | `(x * 257 + 0x8073) >> 16` | YES |

**Alpha channel divergence**: C++ non-CVC source-over path writes alpha=0 to dest.
Rust writes a proper composited alpha. **However**, the golden test comparison
(`compare_images` at common.rs:151) only checks channels 0..3 (R, G, B), NOT channel 3
(alpha). So this alpha divergence does not cause test failures.

## Divergences found

### 1. Alpha channel writing (cosmetic, not tested)

- **C++ line 477-481**: `*p = (packed_blended_rgb) + pix` where pix has alpha byte = 0
- **Rust line 407-410**: `dest[3] = ((dest[3] * t + 0x8073) >> 16) + a`
- **Impact**: None on golden tests (only RGB compared)
- **Cause**: C++ packs RGB into a u32 Pixel via hash tables that only cover R/G/B shifts.
  The alpha byte of the output pixel is always 0. Rust processes each channel separately
  and correctly blends alpha.

### 2. Downscale threshold micro-gap (theoretical)

- **C++ line 296**: `TDX > 0xFFFF00` (ratio >= 0.999985)
- **Rust line 2885**: `ratio_x > 1.0`
- **Impact**: None for the checkbox test (all sections have ratio >> 1.0)
- **Cause**: Different threshold formulation

### 3. Remaining RGB divergence source: NOT in Init or transform

All Init-derived values (TDX, TDY, TX, TY, ODX, ODY, stride, off_x, off_y) are computed
by structurally identical formulas. Given identical inputs, they produce identical outputs.

The max_diff=13 divergence in RGB channels must come from one of these remaining areas:

**Candidate A: Coverage/sub-pixel edge computation**
The `SubPixelEdges` struct computes per-pixel opacity (0-4096) at the boundary of each
9-slice section. If the coverage computation differs from C++ opacity handling
(`opacityBeg/opacity/opacityEnd` in PaintScanline), pixels at section boundaries would
have different opacity scaling, causing RGB differences up to ~13 LSB.

**Candidate B: Carry state across batch boundaries**
The area sampling carry state (`cy`, `pcy_col`) is reset at the start of each scanline
row in Rust (line 2936: `AreaSampleCarryState::new()`). In C++, the carry state flows
naturally within the `PaintScanline` / `Interpolate` call chain. If batching boundaries
differ, the carry-over optimization produces slightly different column weights at batch
edges.

**Candidate C: The opacity/coverage pipeline differs from C++ PaintScanline**
C++ `PaintScanlineInt` receives `(opacityBeg, opacity, opacityEnd)` and applies them
as per-pixel multipliers INSIDE the same function that does compositing. Rust separates
interpolation and blending into two steps with a coverage array intermediary. The opacity
application formula `(src * o + 0x800) >> 12` in Rust vs the C++ inline opacity handling
may differ in edge cases, especially at sub-pixel boundaries where opacity varies.

## Conclusion

The Init/transform formulas are structurally identical and produce identical derived
values. **Issue #2 (tdx not recomputed after reduction) does NOT exist in the current
code** -- the Rust code correctly passes `red_w` to `area_sample_transform_24` which
recomputes tdx.

The max_diff=13 RGB divergence is NOT caused by the transform computation. It is most
likely caused by differences in the **coverage/opacity pipeline** (how sub-pixel edge
coverage is computed and applied), or by **carry state differences** at batch boundaries
in the area sampling loop. A previous attempt to "fix" issue #2 (which was already correct)
would have changed the tdx value to something WRONG, explaining why it made max_diff worse
(13 -> 22).

## Files referenced

- C++ Init: `/home/a0/git/eaglemode-0.96.4/src/emCore/emPainter_ScTl.cpp` lines 25-396
- C++ area sampling: `/home/a0/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlIntImg.cpp` lines 598-828
- C++ blend: `/home/a0/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlPSInt.cpp` lines 155-497
- Rust paint_9slice_section: `/home/a0/git/eaglemode-rs/crates/emcore/src/emPainter.rs` lines 2838-3003
- Rust area_sample_transform_24: `/home/a0/git/eaglemode-rs/crates/emcore/src/emPainter.rs` lines 5936-5980
- Rust interpolation: `/home/a0/git/eaglemode-rs/crates/emcore/src/emPainterInterpolation.rs` lines 1225-1465
- Rust blend: `/home/a0/git/eaglemode-rs/crates/emcore/src/emPainterScanlineTool.rs` lines 268-412
- Golden comparison: `/home/a0/git/eaglemode-rs/crates/eaglemode/tests/golden/common.rs` lines 130-185
