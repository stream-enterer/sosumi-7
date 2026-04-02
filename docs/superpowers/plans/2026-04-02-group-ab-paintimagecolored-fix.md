# Group A+B: PaintImageColored Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Literal port of C++ `PaintScanlineIntG1`/`PaintScanlineIntG2`/`PaintScanlineIntG1G2` color mapping and compositing into the Rust `PaintImageColored` pipeline. Fix 20 Group A+B golden tests.

**Architecture:** The Rust `PaintImageColored` uses a two-step pipeline: (1) `lum_to_color` converts grayscale to straight-alpha RGBA, (2) `blend_scanline` composites. The C++ does this in one step: the paint scanline function applies Color1/Color2 via hash table lookup to produce premultiplied packed pixels, then composites using hash-table canvas blend — all inline. These are structurally different pipelines that cannot be reconciled by tweaking formulas. The fix is to replace the Rust two-step with a literal port of the C++ single-step pipeline.

**Tech Stack:** Rust, C++ reference at `~/git/eaglemode-0.96.4/`

**Key files:**
- C++ scanline functions: `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlPSInt.cpp` — lines 280-480 (all HAVE_GC1/GC2 variants)
- C++ hash setup: same file lines 205-226 (h1R/h2R/hR/hcR pointer setup)
- C++ ScanlineTool init: `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTl.cpp:136-154` (Color1/Color2 dispatch, o1/o2 opacity)
- Rust to replace: `crates/emcore/src/emPainter.rs` — `PaintImageColored` (line 1331), `lum_to_color` closure (line 1405), `PaintBorderImageColored` (line 2592)
- Rust hash: `crates/emcore/src/emColor.rs` — `blend_hash_lookup` (line 60)

---

### Task 1: Read and understand the C++ paint scanline pipeline for colored images

This is a read-only research task. The implementer must understand the full C++ pipeline before writing any Rust.

**Files:**
- Read: `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTlPSInt.cpp` lines 205-480
- Read: `~/git/eaglemode-0.96.4/src/emCore/emPainter_ScTl.cpp` lines 100-160
- Read: `crates/emcore/src/emPainter.rs` lines 1331-1530 (current Rust PaintImageColored)

- [ ] **Step 1: Understand the C++ opacity setup**

In `emPainter_ScTl.cpp`, the ScanlineTool Init sets up opacity values for the colored image cases. Find where `o`, `o1`, `o2` are computed. These come from:
- The per-pixel coverage (from the rasterizer's opacity values: `opacityBeg`, `opacity`, `opacityEnd`)
- `Color1.GetAlpha()` and `Color2.GetAlpha()`
- The texture's `Alpha` value

The paint scanline function signature is:
```cpp
void PaintScanline(const ScanlineTool & sct, int x, int y, int w, int opacityBeg, int opacity, int opacityEnd);
```

The opacity values `o`, `o1`, `o2` are derived from `opacity` (the per-pixel opacity from the rasterizer, 0-4096 range) combined with Color1/Color2 alpha inside the function body. Find this computation.

- [ ] **Step 2: Understand the three C++ color mapping variants**

The C++ has three paint scanline variants for colored images:
- `PaintScanlineIntG1` (HAVE_GC1 only): color1 has alpha, color2 is transparent
- `PaintScanlineIntG2` (HAVE_GC2 only): color2 has alpha, color1 is transparent
- `PaintScanlineIntG1G2` (both): both colors have alpha

For CHANNELS=1 (font glyphs), HAVE_GC2 only (the font glyph case):
```cpp
unsigned g = s[0];  // interpolated grayscale byte
unsigned a = (g * o2 + 0x800) >> 12;
if (!a) continue;
Pixel pix = h2R[a] + h2G[a] + h2B[a];
// Then compositing...
```

For CHANNELS=1, HAVE_GC1 && HAVE_GC2 (both colors):
```cpp
unsigned g = s[0];
unsigned a1 = ((a-g) * o1 + 0x800) >> 12;
unsigned a2 = (g * o2 + 0x800) >> 12;
a = a1 + a2;
Pixel pix = hR[((c1R*a1 + c2R*a2)*257 + 0x8073) >> 16]
          + hG[((c1G*a1 + c2G*a2)*257 + 0x8073) >> 16]
          + hB[((c1B*a1 + c2B*a2)*257 + 0x8073) >> 16];
```

Read and document all three variants for CHANNELS=1. Note: CHANNELS is always 1 for font glyphs because the font atlas is 1-channel. But `PaintBorderImageColored` uses multi-channel images, so also document the CHANNELS=3/4 variants.

- [ ] **Step 3: Understand the C++ compositing step**

After computing `pix`, the C++ composites onto the destination:

Full opacity (`a >= 255` or `CHANNELS=1/3` without alpha):
```cpp
*p = pix;  // direct write
```

Partial opacity with canvas (HAVE_CVC):
```cpp
pix -= hcR[a] + hcG[a] + hcB[a];  // subtract canvas hash
*p += pix;  // add to dest
```

Partial opacity without canvas:
```cpp
unsigned t = (255-a) * 257;
Pixel v = *p;
*p = (Pixel)(
    (((v>>rsh)&rmsk)*t+0x8073)>>16)<<rsh) +
    (((v>>gsh)&gmsk)*t+0x8073)>>16)<<gsh) +
    (((v>>bsh)&bmsk)*t+0x8073)>>16)<<bsh)
) + pix;
```

This is `dest = dest*(1-a) + pix` using the hash formula for `dest*(1-a)`.

- [ ] **Step 4: Understand the hash pointer setup**

`emPainter_ScTlPSInt.cpp` lines 205-226:
```cpp
// For all image types:
const Pixel * hR = (const Pixel*)pf.RedHash + 255*256;  // hash row for color=255
// For HAVE_GC1:
const Pixel * h1R = (const Pixel*)pf.RedHash + Color1.GetRed()*256;
// For HAVE_GC2:
const Pixel * h2R = (const Pixel*)pf.RedHash + Color2.GetRed()*256;
// For canvas (HAVE_CVC):
const Pixel * hcR = (const Pixel*)pf.RedHash + canvasColor.GetRed()*256;
```

So `h2R[a]` = `BLEND_HASH[Color2.Red][a] << redShift`. In Rust terms: `blend_hash_lookup(Color2.GetRed(), a)`.

And `hR[index]` for the `HAVE_GC1 && HAVE_GC2` case: `hR` points to the hash row for color=255, so `hR[index]` = `blend_hash_lookup(255, index)`. For range=255 this is identity: `hR[index] = index << shift` (as established in earlier analysis).

---

### Task 2: Write the literal port

Replace the `lum_to_color` → `blend_scanline` two-step in `PaintImageColored` with a single-step pipeline that matches C++.

**Files:**
- Modify: `crates/emcore/src/emPainter.rs` — `PaintImageColored`, `PaintBorderImageColored`

- [ ] **Step 1: Design the replacement pipeline**

The new pipeline for each output pixel:
1. Get interpolated grayscale `g` (from area sampling or adaptive interpolation — unchanged)
2. Compute color-mapped alpha and premultiplied channel values using C++ formulas
3. Composite onto destination using C++ hash-based compositing

This replaces:
1. Get interpolated grayscale `g` (same)
2. `lum_to_color(g)` → straight-alpha RGBA
3. `blend_scanline` → compositing

The key: steps 2+3 must be a single operation matching C++ exactly, not two separate operations with an intermediate straight-alpha color.

- [ ] **Step 2: Implement for HAVE_GC2 case (color1=TRANSPARENT)**

This is the font glyph case. For each output pixel with interpolated grayscale `g` and per-pixel coverage `cov`:

```rust
// C++ opacity: o2 = (coverage * Color2.GetAlpha() + 0x800) >> 12
// But coverage comes from the rasterizer — need to match C++ coverage scaling
let o2 = /* ... match C++ o2 computation ... */;
let a = ((g as u32 * o2 + 0x800) >> 12) as u8;
if a == 0 { continue; }

// Hash lookup per channel
let pix_r = blend_hash_lookup(color2.GetRed(), a);
let pix_g = blend_hash_lookup(color2.GetGreen(), a);
let pix_b = blend_hash_lookup(color2.GetBlue(), a);

// Composite onto dest (match C++ canvas blend or direct write)
if canvas_is_opaque {
    // Canvas blend: dest += pix - hash(canvas, a)
    let cvs_r = blend_hash_lookup(canvas.GetRed(), a);
    let cvs_g = blend_hash_lookup(canvas.GetGreen(), a);
    let cvs_b = blend_hash_lookup(canvas.GetBlue(), a);
    dest[off]   = (dest[off] as i32 + pix_r as i32 - cvs_r as i32).clamp(0, 255) as u8;
    dest[off+1] = (dest[off+1] as i32 + pix_g as i32 - cvs_g as i32).clamp(0, 255) as u8;
    dest[off+2] = (dest[off+2] as i32 + pix_b as i32 - cvs_b as i32).clamp(0, 255) as u8;
} else {
    // Source-over: dest = dest*(1-a) + pix
    // ... match C++ formula exactly ...
}
```

Note: the exact `o2` computation depends on how C++ combines per-pixel coverage with Color2 alpha. This must be read from the C++ source, not guessed.

- [ ] **Step 3: Implement for HAVE_GC1 case (color2=TRANSPARENT)**

Same structure but using `inv = 255 - g` and `Color1`.

- [ ] **Step 4: Implement for HAVE_GC1 && HAVE_GC2 case (both colors)**

Uses the hash formula `((c1R*a1 + c2R*a2)*257 + 0x8073) >> 16` then hash lookup.

- [ ] **Step 5: Apply the same changes to PaintBorderImageColored**

`PaintBorderImageColored` (emPainter.rs:2592) has its own color mapping code. Apply the same literal port.

- [ ] **Step 6: Delete lum_to_color**

Once all color mapping goes through the new hash-based pipeline, `lum_to_color` is dead code. Delete it.

- [ ] **Step 7: Run the full golden suite**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | grep 'test result:'
```

Pass count must be >= 204.

---

### Task 3: Debug remaining failures

If some Group A+B tests still fail after Task 2, debug them.

- [ ] **Step 1: Identify remaining failures**

```bash
cargo test --test golden -- --test-threads=1 2>&1 | grep FAILED
```

- [ ] **Step 2: Generate diff images and trace divergences**

```bash
DUMP_GOLDEN=1 cargo test --test golden <name> -- --test-threads=1
```

For each remaining failure, trace the specific pixel through the new pipeline and compare against C++.

- [ ] **Step 3: Fix and re-run**

Fix the divergence. Run full suite. Pass count must never decrease.

---

### Task 4: Final verification and commit

- [ ] **Step 1: Run full golden suite**

```bash
cargo test --test golden -- --test-threads=1
```

Expected: >= 224 passed (204 + 20 Group A+B), <= 17 failed.

- [ ] **Step 2: Run clippy, nextest, parallel_benchmark**

```bash
cargo clippy -- -D warnings && cargo-nextest ntr
cargo test --test golden parallel_benchmark -- --test-threads=1
```

All must pass.

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emPainter.rs
git commit -m "fix(colored): literal port of C++ PaintScanlineInt color mapping pipeline

Replace lum_to_color → blend_scanline two-step with C++ single-step:
grayscale → hash-table color mapping → hash-table compositing. Matches
PaintScanlineIntG1/G2/G1G2 from emPainter_ScTlPSInt.cpp exactly.

C++ reference: emPainter_ScTlPSInt.cpp lines 280-480
Fixes: 20 Group A+B tests (HowTo pill text glyph rendering)"
```

---

## Escape Hatches

If the literal port approach hits a wall:

1. **If the o1/o2 opacity computation can't be determined from reading C++ source:** Add `eprintln!` debug prints to the C++ golden generator (`tests/golden/gen/gen_golden.cpp`) that dump the ScanlineTool's o1/o2/o values for a specific pixel coordinate during golden image generation. Rebuild and run the generator to get ground truth values.

2. **If the hash-based compositing is too entangled with the existing blend pipeline to port cleanly:** Instead of replacing `blend_scanline`, add a new `blend_scanline_colored` function that does color mapping + compositing in one pass. Keep the existing `blend_scanline` for non-colored image paths.

3. **If coverage/opacity scaling doesn't match because the Rust rasterizer produces different coverage values from C++:** The color mapping fix can still work — the coverage divergence is a separate G2 issue. Use the Rust coverage values as-is and verify that the color mapping + compositing match C++ for the SAME coverage input. Don't try to fix both at once.

4. **If the fix resolves Group A (15 widget tests) but not Group B (5 composite tests):** Group B composites multiple widgets. If individual widget rendering is now correct but compositing amplifies residual divergences from OTHER groups (G2 polygon, G4 roundrect, etc.), that's expected. Mark Group B as "blocked by other groups" and move on.

5. **If the approach is fundamentally wrong** (e.g., the divergence isn't in color mapping at all): Stop, generate diff images for a simple test (widget_checkbox_unchecked), dump both C++ and Rust intermediate values at the divergent pixel, and report what you find. Don't continue porting code that doesn't address the actual divergence.

---

## Critical Rules

1. **Full suite after every code change.** Pass count must never decrease below 204.
2. **Literal port, not formula tweaking.** The `lum_to_color` → `blend_scanline` pipeline is structurally wrong. Replace it with the C++ pipeline structure: hash-table color mapping + hash-table compositing in one step.
3. **Read the actual C++ source.** `emPainter_ScTlPSInt.cpp` lines 280-480 define the exact pipeline. Read all three variants (G1, G2, G1G2) and both compositing paths (canvas, non-canvas).
4. **Coverage/opacity matters.** The C++ `o`, `o1`, `o2` values combine per-pixel coverage with Color alpha. The exact formula must be ported — don't assume it's `(coverage * alpha + 127) / 255`.
5. **Hash table is required.** The C++ uses `blend_hash_lookup(color, alpha)` for color mapping. Direct arithmetic `(color * alpha + 127) / 255` produces ±1 differences for ~0.2% of inputs. Use the existing Rust `blend_hash_lookup`.
