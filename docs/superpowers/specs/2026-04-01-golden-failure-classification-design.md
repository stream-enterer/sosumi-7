# Golden Failure Classification

## Objective

Classify all 42 golden test failures by shared rendering code path, producing a prioritized fix queue grouped by root cause.

## Scope

This spec covers **classification only** — not fixing. The output is a catalog that maps each failure to its divergent code path and groups failures that share a root cause. A follow-up spec per category handles fixes.

**Non-goals:** Writing fixes, changing rendering code, modifying test infrastructure, performance work.

## Current State

241 golden tests exist. 199 pass at tol=0, 42 fail (measured 2026-04-01). Tolerances are permanently zeroed (commit `cceddbc`). The gradient hash formula is restored (commit `3a2c1c1`).

### Failure Distribution by max_diff

| max_diff | Count | Likely class |
|----------|-------|--------------|
| 1 | 8 | Arithmetic (blend/coverage rounding) |
| 12–33 | 14 | Medium structural or accumulated arithmetic |
| 53–255 | 20 | Structural (wrong rendering logic) |

### Previous Work

- **Previous spec** (`2026-04-01-zero-tolerance-golden-parity-design.md`): Superseded. Its Phase 1 (gradient hash formula) was completed and is correct. Its Phase 2 methodology (per-test audit) is replaced by this group-by-code-path approach.
- **Previous plan** (`2026-04-01-zero-tolerance-golden-parity.md`): Superseded. Task 1 was completed. Tasks 2-3 are replaced.
- **Failure report** (`2026-04-01-zero-tolerance-failure-report.md`): Its "architectural mismatch" thesis is likely wrong for full-opacity paths. Mathematical analysis shows the C++ hash table is an identity function for range=255 at full opacity (`hR[x] = x << shift`). The thesis may hold for partial-opacity canvas-blend paths. Classification will determine which.
- **Permanent changes kept:** Tolerances at zero (commit `cceddbc`), gradient hash formula restored (commit `3a2c1c1`).

## Classification Methodology

For each of the 42 failing tests:

### Step 1: Generate diff images

`DUMP_GOLDEN=1 cargo test --test golden <name>` produces actual/expected/diff PNGs. This gives the spatial location of divergent pixels.

### Step 2: Trace from test to paint call

Read the test source to find which widget/panel is rendered and what `Paint` methods it calls.

### Step 3: Trace from divergent pixels to code path

Using the diff image coordinates, identify which rendering primitive (gradient, ellipse, roundrect, border image, solid rect, text, image blit) produced those pixels. Follow the call chain to the specific `emPainter` method and blend function.

### Step 4: Record the divergent code path

The key output per test: which function(s) in the Rust rendering pipeline produce the wrong pixels, and what the corresponding C++ function is.

### Step 5: Group by shared code path

Tests that diverge at the same function get grouped together. Each group is one fix target.

## Execution Constraints

- **Read-only.** No production code changes during classification. Only output is the catalog document.
- **C++ source is truth.** When tracing, read the actual C++ at `~/git/eaglemode-0.96.4/` — don't trust Rust comments or prior documentation about what C++ does.
- **Diff images first.** Always generate and examine the diff image before tracing code. The spatial pattern tells you where to look.
- **Group aggressively.** If two tests render the same widget type and diverge at the same coordinates relative to the widget, they share a root cause even if max_diff differs (the difference is just how many layers of compositing amplify the error).

## Output Format

The catalog is a markdown table:

| Group | Code Path | C++ Reference | Tests | max_diff range | Likely cause |
|-------|-----------|---------------|-------|----------------|--------------|
| G1 | `PaintBorderImage` → `blit_span_textured` | `emPainter.cpp:1234` | widget_checkbox_checked, ... | 22-236 | TBD after trace |

Each group also has a prose section with:
- Diff image description (spatial pattern of divergent pixels)
- The traced call chain from test → paint method → blend function
- The corresponding C++ code path and file/line
- Hypothesis for the divergence (to be confirmed during fix phase)

## Priority Ordering

After classification, groups are ordered by:
1. **Number of tests affected** (descending) — fixing one code path that affects 15 tests is higher priority than one affecting 2
2. **Likely difficulty** (ascending) — arithmetic fixes before structural rewrites
3. **max_diff** (ascending) — max_diff=1 groups are likely simpler formula issues

## Deliverables

1. **Catalog file:** `docs/superpowers/specs/2026-04-01-golden-failure-catalog.md` — the grouped table plus per-test trace notes
2. **Memory update:** Update `divergence_inventory.md` to reflect the catalog (replacing stale data)
3. **Next step:** One follow-up spec per group (or per cluster of related groups), each scoped to fix that specific code path

## Success Criteria

- Every one of the 42 failing tests appears in exactly one group
- Every group has an identified Rust code path and corresponding C++ reference file/line
- Groups are priority-ordered
- No production code was changed

## All 42 Failing Tests (measured 2026-04-01, after gradient hash fix)

| Test | max_diff | fail% |
|------|----------|-------|
| testpanel_expanded | 255 | 4.56% |
| testpanel_root | 255 | 2.79% |
| composition_tktest_1x | 239 | 8.76% |
| composition_tktest_2x | 239 | 2.09% |
| widget_file_selection_box | 237 | 2.96% |
| widget_checkbox_checked | 236 | 0.07% |
| eagle_logo | 175 | 55.23% |
| composed_border_nest | 153 | 2.07% |
| widget_listbox | 136 | 0.04% |
| cosmos_item_border | 130 | 0.67% |
| widget_border_round_rect | 79 | 0.003% |
| starfield_small | 69 | 0.03% |
| colorfield_expanded | 54 | 0.75% |
| bezier_stroked | 53 | 0.18% |
| starfield_large | 53 | 0.02% |
| listbox_expanded | 33 | 0.07% |
| widget_button_normal | 31 | 0.03% |
| widget_radiobutton | 31 | 0.04% |
| widget_textfield_content | 26 | 0.04% |
| widget_textfield_empty | 26 | 0.04% |
| widget_textfield_single_char_square | 26 | 0.05% |
| widget_listbox_single | 25 | 0.08% |
| widget_listbox_empty | 25 | 0.03% |
| widget_colorfield | 24 | 0.27% |
| widget_colorfield_alpha_near | 24 | 0.71% |
| widget_colorfield_alpha_opaque | 24 | 0.27% |
| widget_colorfield_alpha_zero | 24 | 0.57% |
| golden_widget_border_roundrect_thin | 24 | 0.0008% |
| widget_checkbox_unchecked | 22 | 0.04% |
| widget_splitter_v_extreme_tall | 19 | 0.02% |
| widget_scalarfield | 12 | 0.25% |
| widget_scalarfield_zero_range | 12 | 0.20% |
| widget_scalarfield_min_value | 12 | 0.07% |
| widget_scalarfield_max_value | 12 | 0.06% |
| composed_splitter_content | 1 | 0.002% |
| widget_splitter_h | 1 | 0.0002% |
| widget_splitter_h_pos0 | 1 | 0.0002% |
| widget_splitter_h_pos1 | 1 | 0.0002% |
| widget_error_panel | 1 | 0.0006% |
| multi_compose | 1 | 7.18% |
| image_scaled | 1 | 0.75% |
| gradient_radial | 1 | 0.05% |
