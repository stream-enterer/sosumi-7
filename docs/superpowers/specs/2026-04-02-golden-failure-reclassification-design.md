# Golden Failure Reclassification

## Objective

Reclassify all 37 remaining golden test failures into groups by root cause, incorporating findings from the G1 area sampling and 9-slice transform investigations. Produce an updated catalog that supersedes the 2026-04-01 catalog.

## Background

The original catalog (2026-04-01) classified 42 failures into 9 groups. Since then:
- **5 tests fixed** by the area sampling inner loop literal port (commit on main)
- **G1 hypothesis disproven:** The remaining 23 former-G1 tests do NOT fail due to area sampling carry-over or 9-slice transform parameters. The 9-slice investigation found three actual root causes:
  1. Missing HowTo pill text (~12 tests) — widgets never set `how_to_text` on their border
  2. Text rendering divergence — `PaintTextBoxed` produces different pixels from C++
  3. Large-divergence unknowns (~11 tests, max_diff 33-255) — systematic rendering differences, root cause unknown
- **G2-G9 unchanged:** 14 tests with original hypotheses still applicable (G3 lost 4 tests, G4 lost 1 test to the area sampling fix)
- **pixel_scale passthrough** and **paint_image_full tdx_init fix** merged — correctness improvements, no test impact

## Scope

Classification only — no code changes. Output is an updated catalog document.

## What's New vs Original Catalog

The original catalog's methodology (diff images + code path tracing) is still correct. What's new:

1. **23 former G1 tests need splitting** into sub-groups based on the three root causes identified by the 9-slice investigation
2. **The "large-divergence unknown" sub-group needs investigation** — the 9-slice agent found these tests have pixels ~80-100 channels lighter than expected but couldn't identify the root cause
3. **14 G2-G9 tests should be re-validated** — confirm hypotheses still hold with the current codebase (area sampling and tdx_init changes could theoretically affect some paths)

## Classification Methodology

Same as the original catalog spec:
1. Generate diff images (`DUMP_GOLDEN=1`)
2. Trace from test to paint call
3. Trace from divergent pixels to code path
4. Record the divergent code path
5. Group by shared code path

**Additional for former G1 tests:** Use the 9-slice investigation findings as a starting point. The agent already identified HowTo text and text rendering as root causes for ~12 tests. Verify these findings and investigate the ~11 large-divergence tests.

## Execution Constraints

- **Read-only.** No production code changes.
- **C++ source is truth.** Read `~/git/eaglemode-0.96.4/` for all comparisons.
- **Don't trust the old catalog's hypotheses.** The G1 hypothesis was wrong. Re-validate G2-G9.
- **Use the 9-slice investigation findings** as input, but verify them independently.

## Output

Updated catalog at `docs/superpowers/specs/2026-04-02-golden-failure-catalog.md` with:
- All 37 failures classified into groups
- Each group has: code path, C++ reference, spatial pattern, root cause hypothesis
- Priority ordering by test count and likely difficulty
- Notes on which hypotheses are verified vs speculative

## The 37 Failing Tests

| Test | max_diff | Former group |
|------|----------|-------------|
| testpanel_expanded | 255 | G1 |
| testpanel_root | 255 | G2 |
| composition_tktest_1x | 239 | G1 |
| composition_tktest_2x | 239 | G1 |
| widget_file_selection_box | 237 | G1 |
| widget_checkbox_checked | 236 | G9 |
| eagle_logo | 175 | G7 |
| composed_border_nest | 153 | G1 |
| widget_listbox | 136 | G1 |
| cosmos_item_border | 130 | G8 |
| starfield_small | 69 | G1 |
| colorfield_expanded | 54 | G1 |
| bezier_stroked | 53 | G2 |
| starfield_large | 53 | G1 |
| listbox_expanded | 33 | G1 |
| widget_button_normal | 31 | G1 |
| widget_radiobutton | 31 | G1 |
| widget_textfield_content | 26 | G1 |
| widget_textfield_empty | 26 | G1 |
| widget_textfield_single_char_square | 26 | G1 |
| widget_listbox_single | 25 | G1 |
| widget_listbox_empty | 25 | G1 |
| widget_colorfield | 24 | G1 |
| widget_colorfield_alpha_near | 24 | G1 |
| widget_colorfield_alpha_opaque | 24 | G1 |
| widget_colorfield_alpha_zero | 24 | G1 |
| golden_widget_border_roundrect_thin | 24 | G4 |
| widget_checkbox_unchecked | 22 | G1 |
| widget_splitter_v_extreme_tall | 19 | G1 |
| widget_scalarfield | 12 | G2 |
| widget_scalarfield_zero_range | 12 | G2 |
| widget_scalarfield_min_value | 12 | G2 |
| widget_scalarfield_max_value | 12 | G2 |
| multi_compose | 1 | G5 |
| image_scaled | 1 | G3 |
| gradient_radial | 1 | G6 |
| composed_splitter_content | 1 | G3 |

Note: `widget_splitter_h`, `widget_splitter_h_pos0`, `widget_splitter_h_pos1`, `widget_error_panel`, `widget_border_round_rect` — these were in the original 42 but are now passing (5 fixed by area sampling inner loop + tdx_init fix).

## Success Criteria

- Every one of the 37 failing tests appears in exactly one group
- Each group has an identified Rust code path and corresponding C++ reference
- Groups are priority-ordered
- No production code was changed
