# F010 — forbidden fix-shapes (handoff to fix-spec phase)

Per spec Section 8 / M1. The fix-spec phase MUST run this checklist on its proposed fix; any "yes" answer means the fix is an avoidance fix and is forbidden.

## Four-question avoidance test

For the proposed fix:

1. **Does it introduce a feature flag, environment variable, or build-config gate around the broken code path?** (Yes = avoidance.)

2. **Does it change dispatch (which path is taken at runtime) without changing the path itself?** (Yes = avoidance.)

3. **Does it add a workaround at a higher layer that prevents calls from reaching the broken layer?** (Yes = avoidance.)

4. **Does it deprecate or disable the broken path without removing it?** (Yes = avoidance.)

## Concrete F010-specific avoidance shapes (forbidden)

- Forcing `render_pool.GetThreadCount() = 1` at startup to bypass the display-list branch.
- Setting `dirty_count` ceiling to force the per-tile direct branch.
- Disabling `emPainter::new_recording` and routing all painters through `emPainter::new`.
- Adding a `cfg(feature = "f010_workaround")` arm that skips the broken site.
- Replacing `painter.Clear(color)` calls in panels with `painter.PaintRect` to dodge the recording-mode hole — without fixing the recording-mode hole.

## Permitted fix-shapes

- Adding a `DrawOp::Clear { color }` variant + record path + replay handler at the painter layer (fixes the broken path itself).
- Migrating panels from `painter.Clear(color)` (single-arg) to `painter.ClearWithCanvas(color, canvas_color)` (two-arg) **only if** ClearWithCanvas's record-path semantics match C++'s two-arg Clear contract — that's a structural change that adds canvas-color discipline rather than dodging.
- Fixing the panel state-machine if a state-machine bug is the converged cause.
- Fixing the theme parser if theme parse is the converged cause.

The fix-spec must explicitly answer the four-question test in its own document and demonstrate which permitted shape applies.
