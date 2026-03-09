# Golden Parity Expansion Plan

Extending golden-master testing beyond emPainter primitives to cover
transforms, text, tiling, panel interaction, and window lifecycle.

## Context for the agent

You are continuing parity work on zuicchini, a Rust reimplementation of
C++ Eagle Mode's emCore UI framework. A golden-master test harness compares
zuicchini's output against binary snapshots generated from the real C++
library (linked against `libemCore.so`).

### What this plan adds

~42 new golden tests across 3 harness types: pixel comparison (transforms,
text), framebuffer comparison (tiling), and behavioral comparison (panel
interaction, window lifecycle).

### Do NOT trust this document's claims about code state

This plan was written at a point in time. The codebase may have changed
since then. API names, file paths, test counts, and gap statuses referenced
here may be stale. **Every phase starts with a discovery step that reads the
actual code.** If the code disagrees with this document, the code wins.

---

## Golden data pipeline

**You must understand this pipeline before writing any code.** Every golden
test is a two-stage process:

```
Stage 1 — C++ generator (produces ground truth):
  golden_gen/gen_golden.cpp  →  compile (make -C golden_gen)
                             →  run (make -C golden_gen run)
                             →  writes binary files to golden/<category>/

Stage 2 — Rust tests (compare against ground truth):
  tests/golden_parity/*.rs   →  loads golden/<category>/*.golden
                             →  runs zuicchini equivalent
                             →  compare_images() or compare_rects()
```

**Critical:** Stage 1 MUST happen before Stage 2 can be written or tested.
You cannot test a Rust function against a golden file that does not exist.
Always write and run the C++ generator function first.

### Golden file format — painter (pixel comparison)

Binary: `[u32 width][u32 height][width*height*4 bytes RGBA]`

Written by `dump_painter()` in gen_golden.cpp. Loaded by
`load_painter_golden()` in common.rs.

### Golden file format — layout (rect comparison)

Binary: `[u32 child_count][child_count * (f64 x, f64 y, f64 w, f64 h)]`

Written by `dump_layout()` in gen_golden.cpp. Loaded by
`load_layout_golden()` in common.rs.

### Golden file format — behavioral (NEW — to be designed in Phase 4)

Does not exist yet. Phase 4 must define this format and add corresponding
load/compare functions to common.rs.

---

## Onboarding: file map

Read these files before starting any phase. You do NOT need to read them all
at once — read each phase's files when you start that phase.

### Files you WILL modify

| File | Role |
|------|------|
| `golden_gen/gen_golden.cpp` | C++ golden data generator — add new `gen_*` functions |
| `golden_gen/golden_format.h` | Binary format helpers — extend if new format needed |
| `golden_gen/Makefile` | Build config — should not need changes |
| `tests/golden_parity/painter.rs` | Painter parity tests — add new test functions |
| `tests/golden_parity/common.rs` | Comparison functions — extend ONLY for new harness types |

### Files you MUST read before each phase (not modify)

**These provide the patterns you must follow.** Do not assume you already
know their contents — read them at the start of each phase.

| File | What to discover |
|------|-----------------|
| Existing tests in `painter.rs` | The exact test function pattern. It may have changed since this plan was written. Find the pattern by reading 2-3 existing tests, then replicate it exactly. |
| Existing generators in `gen_golden.cpp` | The exact generator function pattern. Read 2-3 existing `gen_*` functions and `main()`. Replicate the pattern. |
| `common.rs` | The comparison function signatures. Read the actual parameters — do not assume they match what this plan says. |

### C++ reference (read-only — do NOT modify)

The C++ source is at `~/.local/git/eaglemode-0.96.4/`. Read specific files
per-phase as directed below. **Do NOT read all of these up front.**

### Rust source (read before calling any API)

The Rust source is under `src/`. Each phase tells you which files to read.
**Do NOT assume API names from this plan.** Search the actual source for
the method you need. If this plan says "use `push_state()`" but the method
is actually called `save()`, use `save()`.

---

## Shared verification gate

Run after EVERY test you add or modify (not just after each phase).
Do not proceed if any command fails.

```bash
cargo test --test golden_parity -- --nocapture   # ALL parity tests pass
cargo test -p zuicchini                           # unit tests
cargo clippy --workspace -- -D warnings           # no warnings
```

**Before running Rust tests for a new golden file:** Rebuild and re-run the
C++ generator to ensure the golden file exists:
```bash
make -C golden_gen && make -C golden_gen run
```

---

## Per-test automation loop

For EVERY new test, follow these steps in order. Do not skip steps.

### Step 1 — Write C++ generator function

Add a `gen_<test_name>()` function to `gen_golden.cpp`. Follow the pattern
of existing generators exactly:

```cpp
static void gen_transform_translate() {
    emImage img = white_image();
    emPainter p = make_painter(img);
    // ... paint operations using C++ API ...
    dump_painter("transform_translate", img);
}
```

Then add a call to it in `main()`.

**Anti-pattern: Do NOT write the Rust test first.** The golden file must
exist before the Rust test can run.

**Anti-pattern: Do NOT guess the C++ API.** Read the C++ header
(`emPainter.h`) to find the exact function signatures. If you are unsure
how C++ handles transforms, read `emPainter.cpp` and trace the code path.

### Step 2 — Build and run generator

```bash
make -C golden_gen && make -C golden_gen run
```

Verify the golden file was created:
```bash
ls -la golden/painter/transform_translate.painter.golden
```

If compilation fails, fix the C++ code. If the golden file is not created,
check that you added the call in `main()`.

### Step 3 — Write Rust test

Add a test function to `painter.rs`. Follow the existing pattern exactly:

```rust
#[test]
fn painter_transform_translate() {
    require_golden!();
    let (ew, eh, expected) = load_painter_golden("transform_translate");
    let mut img = white_canvas(ew, eh);
    {
        let mut p = white_painter(&mut img);
        // ... equivalent paint operations using Rust API ...
    }
    compare_images(img.data(), &expected, ew, eh, 1, 0.5).unwrap();
}
```

**Anti-pattern: Do NOT invent Rust API calls.** Read the actual Rust source
to find the correct method names and signatures. For transforms, read
`src/render/painter.rs` and search for `push_state`, `pop_state`,
`translate`, `scale`.

**Anti-pattern: Do NOT use `compare_images` with loosened tolerances from
the start.** Start at `(1, 0.5)`. If it fails, investigate the divergence
first. Only loosen if the divergence is understood and documented.

### Step 4 — Run and verify

```bash
cargo test --test golden_parity painter_transform_translate -- --nocapture
```

### Step 5 — Evaluate

- **If it passes at (1, 0.5):** Run the full gate. Move to the next test.
- **If it fails:** Read the failure output. Determine:
  - Is the C++ generator doing the right thing? (Check the golden image.)
  - Is the Rust test calling the equivalent operations? (Check API usage.)
  - Is there a real algorithmic divergence? (This is a bug to fix.)

**Anti-pattern: Do NOT raise the tolerance to make a failing test pass.**
Transform math is exact — any divergence at (1, 0.5) is a bug in either
the generator or the Rust code.

### Step 6 — Gate

Run the full verification gate. Only proceed to the next test after all
existing tests still pass.

### Adapting the loop for new harness types

The examples above use painter harness functions (`dump_painter`,
`load_painter_golden`, `compare_images`). For Phases 3-5 which introduce
new harness types, substitute the appropriate dump/load/compare functions
as designed during that phase's infrastructure step. The pipeline order
(C++ generator → build → run → Rust test → verify → gate) remains the same
regardless of harness type.

---

## Rules

- **Stage 1 before Stage 2.** Write and run the C++ generator before
  writing the Rust test. No exceptions.
- **Read actual source, don't guess APIs.** Every C++ API call must be
  verified against the header. Every Rust API call must be verified against
  the source.
- **Copy existing patterns.** New tests must structurally match existing
  tests in `painter.rs`. New generators must structurally match existing
  generators in `gen_golden.cpp`.
- **Golden data is ground truth.** Do not modify existing golden files.
  Do not modify `dump_painter()` / `load_painter_golden()` / `compare_images()`.
- **Regeneration overwrites everything.** `make -C golden_gen run` re-runs
  ALL generators, overwriting all existing golden files. After every
  regeneration, run the full gate immediately to verify no existing test
  broke. If an existing test fails after regeneration, your C++ edit has
  corrupted shared state — revert and investigate.
- **One test at a time.** Do not batch-write all tests, then batch-test.
  Write one C++ generator → run → write one Rust test → verify → gate → next.
- **Fix source, not tests.** If a test fails, fix the Rust renderer or the
  C++ generator. Do not raise tolerances without a documented reason.
- **Run the gate after every test.** Not after each phase — after each test.
- **No `#[allow]` / `#[expect]`.** Fix warnings at the source.
- **Commit after each phase.** Not after each test. Include the phase name
  and list of tests added in the commit message.

### What NOT to do

- **Do not modify `common.rs`** except when adding a genuinely new harness
  type (Phases 3 and 4). For Phases 1-2 the existing `compare_images` is sufficient.
- **Do not "improve" the C++ generator.** It exists to exercise the real C++
  library. Keep generators minimal — just call the C++ API and dump output.
- **Do not add helper functions/abstractions** for "DRY" unless you have 5+
  tests using the exact same setup. A little repetition is fine.
- **Do not add explanatory comments on standard API calls.** The test code
  should be self-evident. Comments only for non-obvious C++ API quirks.
- **Do not refactor existing tests.** Only add new ones.
- **Do not add `#[ignore]`** to any test.

---

## Phase 0: Discover current state (run before every phase)

Before starting any phase, execute these discovery steps. Do NOT skip them
even if you think you know the answers from this plan document.

### Step 0a — Verify C++ build pipeline

```bash
make -C golden_gen && make -C golden_gen run
```

If this fails, fix the build before proceeding. Common causes: Eagle Mode
not installed at `~/.local/git/eaglemode-0.96.4/`, libraries not compiled,
missing headers. Nothing else in this plan works until the generator builds
and runs.

### Step 0b — Verify golden data exists

```bash
ls golden/painter/*.golden | wc -l
ls golden/layout/*.golden | wc -l
```

Confirm counts match the expected baseline (at time of writing: 29 painter,
13 layout — but these may have changed). If counts are 0, the generator did
not produce output — investigate before proceeding.

**Warning:** The `require_golden!()` macro in test files causes silent test
skips (not failures) when the `golden/` directory is absent. If golden data
is missing, `cargo test` will show ALL tests passing even though none
actually execute. Always verify golden files exist before trusting test
results.

### Step 0c — Establish test baseline

```bash
cargo test --test golden_parity 2>&1 | grep 'test result'
```

Record how many tests pass. This is your regression baseline.

### Step 0d — Read the existing test pattern

Read `tests/golden_parity/painter.rs` (at least the first 3 test functions).
Note:
- What imports are used
- What helper functions exist (e.g., `white_canvas`, `white_painter`)
- What macro gates the test (e.g., `require_golden!()`)
- What the `compare_images` call looks like (parameter order, types)
- Whether the pattern has changed since this plan was written

### Step 0e — Read the existing generator pattern

Read `golden_gen/gen_golden.cpp` (at least the first 3 `gen_*` functions
and `main()`). Note:
- What helper functions exist (e.g., `white_image`, `make_painter`, `dump_painter`)
- How `main()` calls the generators
- What includes are used

### Step 0f — Read the comparison function

Read `tests/golden_parity/common.rs`. Note the exact signature of
`compare_images` and any other comparison functions. Do not assume the
signature from this plan document.

### Step 0g — Discover the Rust APIs you will need

For the specific phase you are starting, search the Rust source for the
relevant APIs. For example, for Phase 1 (transforms):

```
Search src/render/painter.rs for: push, pop, translate, scale, origin, clip
```

Record the **actual** method names and signatures. If this plan says
`push_state()` but the code has `save_state()`, use `save_state()`.

### Step 0h — Discover the C++ APIs you will need

For the specific phase, read the relevant C++ headers. For Phase 1:

```
Read ~/.local/git/eaglemode-0.96.4/include/emCore/emPainter.h
Search for: PreparePainter, origin, scale, clip
```

Record the **actual** function signatures. The C++ API may use origin/scale
parameters at construction time, or it may have setter methods, or something
else entirely. Discover, don't assume.

---

## Phase 1: Transform stack (COMPLETE — pixel harness extension)

**Effort:** Low. Extends existing pixel comparison harness.
**Blocked by:** Nothing.
**Value:** Validates that push/pop/translate/scale compose correctly — the
foundation every panel's paint() relies on.

### STATUS: COMPLETE

All 7 transform golden tests pass at tolerance (1, 0.5).

### Discovery (Phase 1-specific)

Before writing any code, answer these questions by reading source:

1. **C++ transform mechanism:** Read `emPainter.h`. How does C++ set
   origin and scale on a painter? Is it at construction time? Via setters?
   Via a child-painter factory? Record the exact API.

2. **C++ nested transforms:** How does C++ compose multiple transforms
   (e.g., translate then scale then paint)? C++ may use constructor
   chaining (new `emPainter` from existing one), setter methods, or
   some other mechanism. It likely does NOT have a push/pop stack.
   Record the exact pattern.

3. **Rust transform mechanism:** Read `src/render/painter.rs`. Search for
   methods related to push/pop/translate/scale/origin/clip. Record the
   exact method names, parameters, and how they compose.

4. **Existing `make_painter` helper:** Read `gen_golden.cpp`'s
   `make_painter()` function. What origin and scale does it set? Can you
   create a painter with different origin/scale by calling different
   C++ APIs, or do you need a new helper?

**Do NOT proceed to writing tests until all 4 questions are answered with
specific function names and signatures from the actual source.**

### Test cases

Write these in order. Each test is one C++ generator function + one Rust
test function.

| # | Test | What it exercises |
|---|------|-------------------|
| 1 | `transform_translate` | Translate origin, paint rect, verify position shift |
| 2 | `transform_scale` | Scale 2×, paint rect, verify doubled size |
| 3 | `transform_nested` | Nested transform composition: translate then scale, paint inner, undo scale, paint outer |
| 4 | `transform_clip_interaction` | Set clip, translate so shape partially exits clip |
| 5 | `transform_ellipse_scaled` | Non-uniform scale on ellipse (different x/y scale) |
| 6 | `transform_fractional` | Fractional translate (sub-pixel offset) |
| 7 | `transform_identity_roundtrip` | scale(2) → scale(0.5) → paint — should match no-transform |

**The "C++ operations" and "Rust operations" columns are intentionally
omitted.** You must discover the actual API calls from the source in the
discovery step above. Do not guess.

### Tolerance target

(1, 0.5) for all. Transform math is exact arithmetic. Any divergence is a
bug, not an architectural gap. Do NOT accept looser tolerances without
finding and fixing the underlying cause.

---

## Phase 2: Text rendering (COMPLETE — pixel harness extension)

**Effort:** Medium.
**Blocked by:** Nothing (font system stabilized).
**Value:** Text is everywhere in the UI. Font data matches C++ exactly.

### STATUS: COMPLETE

Completed 2026-03-09. All 6 text golden tests pass at tolerance (1, 0.5%).

**What was done:**
- Ported the Eagle Mode grayscale font atlas (BasicLatin TGA, 128×224 cells)
- Rewrote `paint_image_colored` with three key fixes:
  1. Float-precision coordinate mapping (was truncating sub-pixel to integer)
  2. Weighted area sampling matching C++ DQ_3X3 downscale quality (was nearest-neighbor)
  3. PSF_INT_G2 opacity-based blending for IMAGE_COLORED textures (was lerp-based)
- Fixed alpha composition precision: `(a * b + 127) / 255` instead of `(a * b + 128) >> 8`
- Commit: `21f4ba9`

### Discovery (Phase 2-specific)

Before writing any code, answer these questions by reading source:

0. **Can C++ PaintText run in the generator?** The C++ font system
   (`emFontCache`) loads `.tga` glyph files from the Eagle Mode install
   path at runtime. If the resource path is missing, `PaintText` will
   crash with `emFatalError`. Write a minimal `gen_text_probe()` that
   calls `PaintText` with a single character, build and run. If it
   crashes, check that `EM_DIR` is set and that `$EM_DIR/res/emCore/font/`
   contains TGA glyph files. **STOP gate:** If PaintText crashes and you
   cannot resolve the resource path, PARK this phase.

1. **What font system does Rust currently use?** Read `src/render/` and
   search for font-related files (bitmap_font, font, glyph, etc.). Is it
   bitmap? TTF? SDF? Something else? If the font system has changed since
   this plan was written, reassess whether pixel comparison is appropriate.

2. **What font does C++ use?** Read `emPainter.h` and search for
   `PaintText`. Find how C++ gets its default font. Locate the C++ glyph
   data (it may be in a header, a resource file, or compiled into the
   binary).

3. **Do the glyph bitmaps match?** If both use bitmap fonts, compare the
   actual glyph data byte-for-byte. If they differ, pixel comparison will
   fail on every test. In that case, either fix the glyph data or fall
   back to bounding-box comparison.

4. **What Rust text API exists?** Search `src/render/painter.rs` for text
   painting methods. Record the actual function names (they may not be
   `paint_text_at` / `paint_text_fitted` as this plan guesses).

5. **What C++ text API exists?** Read `emPainter.h` for `PaintText` and
   related methods. Record the actual signatures.

**STOP gate:** If question 1 reveals the font system is unstable or being
redesigned, PARK this phase. Do not write tests against a moving target.

**STOP gate:** If question 3 reveals glyph data mismatch, either fix the
mismatch first or redesign this phase as bounding-box comparison (not
pixel comparison).

### Test cases

Only proceed after ALL discovery questions are answered and no STOP gate
was triggered.

| # | Test | What it exercises |
|---|------|-------------------|
| 1 | `text_basic` | Single line of ASCII at default size |
| 2 | `text_scaled` | Text with non-default width scaling |
| 3 | `text_fitted` | Auto-scale text to fit bounding box |
| 4 | `text_alignment` | Multiple alignment combinations |
| 5 | `text_clipped` | Text partially outside clip rect |
| 6 | `text_below_threshold` | Text too small to render (verify skip behavior) |

### Tolerance target

(1, 0.5) if glyph data matches. If glyphs differ by design, fall back to
layout-level comparison (bounding boxes) with appropriate tolerance.

---

## Phase 3: Tile compositor (framebuffer comparison — new harness type)

**Effort:** High. Requires scene-level golden generator + possibly new
infrastructure.
**Blocked by:** Phase 1 (transforms must be correct first).
**Value:** Validates the full render pipeline end-to-end.

### Discovery (Phase 3-specific)

This phase has the most open questions. Answer ALL of these before writing
any code.

1. **What does the Rust compositor look like now?** Read `src/render/`
   and search for compositor, tile_cache, tile, composit. The compositor
   may have been rewritten, removed, or restructured since this plan was
   written. Record what exists, what rendering backend it uses (wgpu? CPU?),
   and what its public API looks like.

2. **Does C++ have an equivalent compositor?** Search the C++ source for
   tiling, compositing, or viewport rendering. If C++ has a tile compositor,
   golden tests can compare tiled output. If C++ does NOT tile (just paints
   panels directly), then the golden comparison is full-scene rendering, not
   tile-level.

3. **Can the Rust compositor run without GPU?** If it uses wgpu, can it
   use a software backend? Or does a `SoftwareCompositor` need to be
   written? This is a blocking question — golden tests cannot depend on GPU
   availability.

4. **What is the tile size?** Read the tile cache code. It may not be
   256×256 as this plan assumed.

5. **What scene API exists?** How do you construct a panel tree and paint
   it via the compositor? Read the actual code paths.

**STOP gate:** If question 3 reveals no way to run without GPU, you must
either write a SoftwareCompositor or redesign this phase before proceeding.

### Design decisions to resolve

After discovery, resolve these before coding:

- **Software fallback:** If GPU is required, add a `SoftwareCompositor`
  that composites tiles into a CPU Image, OR use wgpu's software backend.
- **Scene definition:** Both C++ and Rust must construct identical panel
  layouts. Use hardcoded structs, not divergent code.
- **Comparison scope:** Is this comparing the final composited framebuffer,
  or individual tiles? The answer depends on what C++ does.

### Test file organization

- Create `tests/golden_parity/compositor.rs` for Phase 3 tests.
- Add `mod compositor;` to `tests/golden_parity/main.rs`.
- Golden files go in `golden/compositor/` (e.g., `composite_single_tile.compositor.golden`).
- Test function prefix: `compositor_` (e.g., `fn compositor_single_tile()`).
- Copy the `require_golden!()` macro into the new module (it is defined
  per-module, not in common.rs).

### Test cases

| # | Test | What it exercises |
|---|------|-------------------|
| 1 | `composite_single_tile` | One panel smaller than one tile |
| 2 | `composite_multi_tile` | Panel spanning 2×2 tiles |
| 3 | `composite_overlap` | Two panels overlapping across tile boundary |
| 4 | `composite_dirty_update` | Change one panel, verify only its tiles re-render |
| 5 | `composite_viewport_scroll` | Shift viewport, verify tile reuse |

### Tolerance target

(1, 0.5) — compositing should not introduce pixel error beyond what the
painter already produces.

---

## Phase 4: Panel interaction model (COMPLETE — behavioral harness)

**Effort:** High. New harness design required.
**Value:** Validates focus, activation, input dispatch, notice propagation.

### STATUS: COMPLETE (22 tests)

Completed 2026-03-09. Three sub-phases, each with dedicated golden format:

**Interaction tests (11):** Activation (5) + focus navigation (6). Binary
format: `[u32 count][count * (u8 is_active, u8 in_active_path)]`.
Added `View::remove_panel()` for C++ `~emPanel` active-path cleanup parity.

**Notice tests (7):** `notice_active_changed`, `notice_focus_changed`,
`notice_layout_changed`, `notice_children_changed`,
`notice_window_focus_gained`, `notice_window_focus_lost`,
`notice_window_resize`. Binary format:
`[u32 count][count * u32 accumulated_flags]`. C++ uses `RecordingPanel`
subclass to accumulate `Notice(flags)`. Rust uses `NoticeBehavior` +
`translate_cpp_notice_flags()` mapping.

**Input tests (4):** `input_mouse_hit`, `input_key_to_focused`,
`input_scroll_delta`, `input_drag_sequence`. Binary format:
`[u32 count][count * (u8 received_input, u8 is_active, u8 in_active_path)]`.
C++ uses `GoldenViewPort` subclass exposing `DoInputToView()` for headless
synthetic input injection. Rust uses `InputTrackingBehavior`.

### Why behavioral, not pixel

Interaction correctness is state transitions, not pixels:
- "Click panel B → B becomes focused → A gets NF_FOCUS_CHANGED"
- "Tab → focus moves to next focusable sibling"

Pixel comparison cannot express this. This phase needs an **event-replay
harness** that records state transitions.

### Discovery (Phase 4-specific)

This phase requires the most research. Answer ALL of these before designing
anything.

**C++ side:**

1. **Can C++ panel logic run without a display?** Read `emPanel.h` and
   `emView.h`. Does `emPanel` require an `emView`? Does `emView` require
   a window/display context? Can you create a headless panel tree for
   testing? **If not, this phase may be infeasible as designed.**

2. **Are input dispatch methods accessible?** `InputToView` on
   `emViewPort` and `Input` on `emPanel` may be `protected`, not public.
   Check the access level. If protected, you will need a custom
   `emViewPort` subclass that exposes the method to inject synthetic
   events. Plan this infrastructure before writing any test generators.

3. **How does C++ dispatch input?** Search `emPanel.h` / `emPanel.cpp`
   for `Input`, `HandleInput`, or equivalent. Find the exact method that
   receives mouse/keyboard events. What parameters does it take?

4. **How does C++ manage focus?** Search for `Focus`, `SetFocused`,
   `GetFocused`, `IsFocused` in panel-related headers. Record the actual
   API for querying and changing focus.

5. **How does C++ manage activation?** Search for `Active`, `Activate`,
   `IsActive`, `GetActivePanel`. Record the API.

6. **How does C++ dispatch notices?** Search for `Notice`, `HandleNotice`,
   `NF_FOCUS_CHANGED`, `NF_ACTIVE_CHANGED`. Can you observe which notices
   fire, or only infer from state changes?

7. **What state is queryable after each event?** List every piece of state
   you can read from a C++ panel tree: focused panel, active panel, notice
   flags, layout rects, visibility, etc.

**Rust side:**

8. **What panel API does Rust have?** Read `src/panel/` (or wherever panel
   code lives — find it first). Search for focus, activation, input
   dispatch, notice. Record what exists. The Rust API may not mirror C++.

9. **Can Rust panels run without a window?** Can you construct a panel
   tree and dispatch synthetic events in a unit test? If not, what
   infrastructure is needed?

**STOP gate:** If question 1 reveals C++ panels cannot run headlessly,
investigate whether you can subclass `emView` with a stub implementation.
If that's infeasible, this phase needs a fundamentally different approach
(e.g., instrumenting a running application instead of golden comparison).

### New harness design

**This requires modifying `common.rs` and `golden_format.h`.** Phase 3 and
Phase 4 are the two exceptions to the "do not modify common.rs" rule.

#### Golden format: structured binary

Design the format AFTER completing discovery. The format must capture
whatever state C++ actually exposes (per discovery question 6 above).

Principles:
- Self-describing (include event count, field sizes)
- Extensible (reserve space or use tagged fields)
- Deterministic (no timestamps, no pointer values, no memory addresses)

**Anti-pattern: Do NOT design this format before reading the C++ panel
API.** The format must match reality, not assumptions.

#### Generator and test patterns

Follow the same two-stage pipeline (C++ generator → Rust test) used by
painter tests. The generator builds a panel tree, replays events, records
state snapshots. The Rust test does the same and compares.

### Test file organization

- Create `tests/golden_parity/interaction.rs` for Phase 4 and Phase 5 tests.
- Add `mod interaction;` to `tests/golden_parity/main.rs`.
- Golden files go in `golden/behavioral/` (shared by Phases 4 and 5).
- Test function prefix: `interaction_` (e.g., `fn interaction_focus_click()`).
- Copy the `require_golden!()` macro into the new module.

### Test case categories

**Focus model (~6 tests):**

| Test | What it exercises |
|------|-------------------|
| `focus_click` | Click panel → it becomes focused |
| `focus_tab_forward` | Tab cycles through focusable siblings |
| `focus_tab_backward` | Shift+Tab reverse cycle |
| `focus_unfocusable_skip` | Tab skips non-focusable panels |
| `focus_nested` | Focus into child panel, then back to parent |
| `focus_remove_focused` | Remove focused panel → focus moves to parent |

**Activation model (~4 tests):**

| Test | What it exercises |
|------|-------------------|
| `activate_click` | Click activates panel and ancestors |
| `activate_path` | Active path includes all ancestors |
| `activate_switch` | Activate different panel → old one deactivated |
| `activate_remove` | Remove active panel → activation clears |

**Notice propagation (~4 tests):**

| Test | What it exercises |
|------|-------------------|
| `notice_focus_changed` | Focus change fires NF_FOCUS_CHANGED on old and new |
| `notice_active_changed` | Activation fires NF_ACTIVE_CHANGED |
| `notice_layout_changed` | Resize fires NF_LAYOUT_CHANGED |
| `notice_child_added` | Adding child fires appropriate notices |

**Input dispatch (~4 tests):**

| Test | What it exercises |
|------|-------------------|
| `input_mouse_hit` | Mouse event dispatched to correct panel by position |
| `input_key_to_focused` | Key event goes to focused panel |
| `input_scroll_delta` | Mouse wheel → correct scroll delta |
| `input_drag_sequence` | Mouse down → move → up sequence |

### Tolerance

Exact match. State transitions are deterministic. Any divergence is a bug.

---

## Phase 5: Window lifecycle (COMPLETE — behavioral harness extension)

**Effort:** Medium (extends Phase 4 harness).
**Value:** Validates resize, visibility, multi-window coordination.

### STATUS: COMPLETE (3/3 tests)

`notice_window_focus_gained` and `notice_window_focus_lost` in Phase 4's
notice test suite cover window focus gain/loss → `NF_VIEW_FOCUS_CHANGED`
propagation to panels.

`notice_window_resize` tests viewport resize with `VF_ROOT_SAME_TALLNESS` →
`NF_LAYOUT_CHANGED` on root. Fixed `set_viewport()` to take `&mut PanelTree`
and inline-update root layout rect (C++ `SetGeometry` parity). Removed
redundant per-frame sync in `app.rs`. Viewing-notice propagation gap closed:
`set_layout_rect` now queues VISIBILITY/UPDATE_PRIORITY/MEMORY_LIMIT alongside
LAYOUT_CHANGED (matching C++ `Layout()` behavior), and the per-frame
full-recalc notice loop in `update_viewing` was removed. Test uses
`NOTICE_FULL_MASK` with no exclusions.

### Discovery (Phase 5-specific, for `window_resize` only)

1. **How does C++ handle viewport resize?** Read `emView.h` / `emViewPort.h`
   for resize-related methods. Can the headless `GoldenViewPort` trigger a
   resize that propagates `NF_LAYOUT_CHANGED` to the panel tree?

2. **How does Rust handle resize?** Search `src/` for resize, set_geometry,
   or equivalent on View/ViewPort.

3. **Can the layout golden format capture child rects after resize?** Or
   does a new comparison approach (before/after rect snapshots) need to be
   designed?

### Tolerance

Exact match.

---

## Phase summary

| Phase | Tests | Harness | Effort | Status |
|-------|-------|---------|--------|--------|
| 1. Transforms | 7 | pixel (existing) | Low | COMPLETE |
| 2. Text | 6 | pixel (existing) | Medium | COMPLETE |
| 3. Tiling | ~5 | framebuffer (new) | High | NOT STARTED (software compositor decision) |
| 4. Interaction | 22 | behavioral (new) | High | COMPLETE |
| 5. Window | 3/3 | behavioral (Phase 4) | Medium | COMPLETE |

**Total:** 37 complete + ~5 remaining across 3 harness types.

### What this gives us when complete

Full golden parity with emCore's:
- 2D rendering pipeline (shapes, strokes, text, gradients, images,
  compositing, transforms, tiling)
- Layout algorithms (linear, raster, pack)
- Scheduler (signals, timers, priorities)
- Panel interaction model (focus, activation, input dispatch, notices)
- Window lifecycle (resize, visibility, focus)

### What remains outside golden parity scope

- **Performance** — not testable via golden comparison
- **emFileModel / emRec** — I/O format conformance, not behavioral parity
- **Platform integration** — X11/Win32/Wayland specifics
- **Application-level behavior** — egopol game logic above emCore

---

## Execution order

Phase 1 is **complete** (7 transform tests passing at tight tolerance).

Phase 2 is **complete** (6 text golden tests passing at tight tolerance).

Phase 4 is **complete** — 22 tests: 11 interaction (activation + focus),
7 notice (with `RecordingPanel` C++ subclass + `NoticeBehavior` Rust harness
+ flag translation), 4 input (with `GoldenViewPort` C++ subclass +
`InputTrackingBehavior` Rust harness). All pass with exact match.

Phase 5 is **complete** — 3/3 tests: `notice_window_focus_gained/lost` +
`notice_window_resize`. The resize test also fixed `set_viewport()` parity
with C++ `SetGeometry` (inline root layout update for `ROOT_SAME_TALLNESS`).

Phase 3 requires design decisions (software compositor, scene definition).
Unblocked by Phase 1 completion.

---

## Common LLM failure modes to avoid

These are mistakes that LLM agents consistently make on golden parity work.
Treat them as hard constraints.

### Discovery failures

1. **Trusting this plan instead of reading code.** This plan contains
   approximate API names, file paths, and assumptions that may be stale.
   If you write `push_state()` because the plan says so, but the actual
   method is `save()`, your code won't compile. Always read the source.

2. **Skipping Phase 0 discovery.** Every phase starts with discovery for
   a reason. If you skip it, you'll write code against imagined APIs and
   waste time on compile errors and logic mismatches.

3. **Assuming the codebase hasn't changed.** Tests may have been added,
   APIs renamed, files moved, or features removed since this plan was
   written. Run the baseline test suite and read current source before
   any phase.

### Pipeline failures

4. **Writing Rust tests before the golden file exists.** The test will
   panic with "Cannot read golden/...". Always write and run the C++
   generator first.

5. **Not rebuilding the C++ generator.** After modifying `gen_golden.cpp`,
   you MUST `make -C golden_gen && make -C golden_gen run` before testing.
   If you forget, the old golden file (or no file) will be compared against.

6. **Forgetting to add the `gen_*` call in `main()`.** The generator
   function exists but never runs. The golden file is not created. The
   Rust test panics.

### API failures

7. **Guessing C++ API signatures.** The C++ API has non-obvious signatures
   and patterns. Read the actual header before every C++ function call.

8. **Guessing Rust API names.** The Rust APIs may use different names than
   this plan suggests. Search the source for the actual name.

9. **Assuming transforms are matrices.** The C++ emPainter may use
   origin/scale, not affine matrices. Read the API to confirm. Do not
   fabricate a matrix-based API.

### Quality failures

10. **Batch-writing all tests then batch-testing.** This hides which test
    introduced a regression. Write one test, verify, gate, next.

11. **Raising tolerance to make a test pass.** Transforms and behavioral
    tests should pass at exact or near-exact tolerances. If they don't,
    there is a bug. Investigate, don't paper over.

12. **Adding abstractions "for reuse".** Do not create `TestScene` builders,
    `TransformHelper` structs, or other wrappers. Copy the existing flat
    test pattern.

13. **Modifying common.rs unnecessarily.** Only modify it when adding a
    genuinely new comparison function for a new harness type (Phases 3 or 4).

### Design failures

14. **Designing Phase 3/4/5 formats without reading C++ source.** The
    golden format must capture state that C++ actually exposes. Read the
    C++ APIs first, design the binary format second.

15. **Proceeding past a STOP gate.** If a discovery step reveals a
    blocking issue (e.g., C++ panels can't run headlessly, glyph data
    doesn't match), STOP. Do not work around it by writing tests that
    will never pass. Report the blocker and move to a different phase.

---

## CI considerations

Golden files are gitignored (`golden/` is in `.gitignore`). This means:

- Golden parity tests are **developer-local only**. CI pipelines that lack
  the C++ toolchain and Eagle Mode installation will silently skip all
  golden tests (via `require_golden!()`) and report green.
- To run golden tests in CI, the pipeline must: (a) have Eagle Mode
  `libemCore.so` and headers available, (b) build and run the C++ generator,
  (c) then run `cargo test --test golden_parity`.
- Regenerating golden files does not produce a git diff — there is nothing
  to commit. Golden data consistency is enforced by the gate checks, not
  by version control.

---

## Commands

```bash
# C++ generator
make -C golden_gen                                 # compile
make -C golden_gen run                             # generate golden files
ls golden/painter/                                 # verify files exist

# Rust tests
cargo test --test golden_parity -- --nocapture                   # all parity
cargo test --test golden_parity painter_<name> -- --nocapture    # single test
cargo test -p zuicchini                                          # unit tests
cargo clippy --workspace -- -D warnings                          # clippy
```
