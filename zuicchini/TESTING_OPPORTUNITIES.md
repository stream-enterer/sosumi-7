# Headless Testing Opportunities

Tiered by suitability for headless golden testing against C++ emCore.

## Tier 1 — High Value, Low Difficulty

### Layout rect goldens
- **C++ files:** `emLinearLayout.cpp`, `emRasterLayout.cpp`, `emPackLayout.cpp`
- **Type:** Numeric comparison (layout_rect values)
- **Why:** Pure geometry, deterministic, no pixel tolerance issues. Three complex layout engines with no golden verification beyond harness API tests.
- **Approach:** Create panel trees with known constraints, trigger layout, compare child rect positions/sizes between C++ and Rust.

### TestPanel full render
- **C++ files:** `emTestPanel.cpp` (in `src/emTest/`)
- **Type:** Pixel comparison
- **Why:** C++ already has a comprehensive test panel exercising every widget. Ready-made integration-level scene covering widget combinations.
- **Approach:** Render full TestPanel to large offscreen image in both C++ and Rust, pixel-diff.

### emRec serialization
- **C++ files:** `emRec.cpp`, `emRec.h`
- **Type:** Data parity (byte-level)
- **Why:** Pure data transform, no rendering. Deterministic read/write of emCore's record format.
- **Approach:** Serialize/deserialize identical structures in both, compare output bytes.

## Tier 2 — High Value, Medium Difficulty

### ColorField expanded layout
- **C++ files:** `emColorField.cpp`
- **Type:** Numeric + pixel
- **Why:** Harness tests verify API but no golden tests verify the expanded child panel layout or scalar field rendering.
- **Approach:** Expand color field, compare child panel rects and rendered scalar fields.

### ListBox with item panels
- **C++ files:** `emListBox.cpp`
- **Type:** Numeric + pixel
- **Why:** No golden test for a populated, scrolled, multi-selected list with item panels.
- **Approach:** Create list with N items, verify item positions, selection highlights, and scroll state.

### Splitter drag + layout sequences
- **C++ files:** `emSplitter.cpp`
- **Type:** Numeric (interaction + layout)
- **Why:** Phase 7 has basic splitter tests but no multi-step resize-then-verify-child-rects golden.
- **Approach:** Drag splitter, verify child panel rects update correctly in both C++ and Rust.

### FileModel lifecycle
- **C++ files:** `emFileModel.cpp`
- **Type:** State sequence comparison
- **Why:** Harness covers API surface but no behavioral test drives the full load/ready/save/flush cycle.
- **Approach:** Drive state machine through transitions, compare state sequence and timing.

## Tier 3 — High Value, High Difficulty (Blocked)

### Border 9-slice composition
- **C++ files:** `emBorder.cpp` (800+ lines)
- **Type:** Pixel comparison
- **Why:** 4 Phase 6 golden tests fail here (button ~60%, radiobutton ~57%, colorfield ~33%, listbox ~31%). Root cause: C++ fixed-point vs Rust float interpolation precision in 9-slice image scaling.
- **Blocker:** Requires resolving fixed-point/float rounding strategy before tests can pass. High value once unblocked since borders appear in every widget.

## Tier 4 — Moderate Value

### Text rendering edge cases
- **C++ files:** `emFontCache.cpp`
- **Type:** Pixel comparison with tolerance
- **Why:** Phase 2 golden covers basic text. Edge cases (long strings, empty strings, special chars, alignment combos) untested.
- **Caveat:** Font rendering has inherent platform variance, needs wide tolerance.

## Not Applicable

### emHmiDemo components
- **C++ files:** `src/emHmiDemo/` (17+ files: Pump, Tank, Conveyor, Mixer, Station)
- **Why not:** Application-level demo panels, not framework. Outside zuicchini's scope.

## Existing Coverage Reference

| Area | Golden Phase | Tests | Status |
|------|-------------|-------|--------|
| Painter primitives | Phase 1-3 | 18 | All pass |
| Widget rendering | Phase 6 | 16 written | 12 pass, 4 ignored (Tier 3 blocker) |
| Widget interaction | Phase 7 | 13 | All pass |
| Animator trajectories | Phase 8 | 10 | All pass |
| Input filters | Phase 9 | 8 | All pass |
| Focus navigation | Phase 4 | 41 | All pass |
| Window | Phase 5 | 3 | All pass |
| Harness API parity | — | 416 | All pass |
