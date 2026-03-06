Verification Harness — Full Results

Pipeline Completion Status

┌─────────────────────┬───────────────────────────────────────────────────────────────────────────────────┐
│        Phase        │                                      Status                                       │
├─────────────────────┼───────────────────────────────────────────────────────────────────────────────────┤
│ Phase 1 (Extract)   │ 12/13 complete — emPainter.cpp agent stuck (3644 lines too large for single pass) │
├─────────────────────┼───────────────────────────────────────────────────────────────────────────────────┤
│ Phase 2 (Match)     │ 6/7 complete — painter blocked on extract                                         │
├─────────────────────┼───────────────────────────────────────────────────────────────────────────────────┤
│ Phase 3 (Verify)    │ All non-painter subsystems done                                                   │
├─────────────────────┼───────────────────────────────────────────────────────────────────────────────────┤
│ Tier 3 (API checks) │ 3/3 complete                                                                      │
└─────────────────────┴───────────────────────────────────────────────────────────────────────────────────┘

---
Tier 1: Scheduler — CLEAN

┌────────────────┬─────────┬────────────┬──────────┬──────────┐
│      File      │ CORRECT │ BEHAVIORAL │ COSMETIC │ CRITICAL │
├────────────────┼─────────┼────────────┼──────────┼──────────┤
│ emScheduler    │ 17      │ 2          │ 2        │ 0        │
├────────────────┼─────────┼────────────┼──────────┼──────────┤
│ emEngine       │ 26      │ 0          │ 0        │ 0        │
├────────────────┼─────────┼────────────┼──────────┼──────────┤
│ emSignal/Timer │ 26      │ 5          │ 1        │ 0        │
└────────────────┴─────────┴────────────┴──────────┴──────────┘

Actionable findings:
1. Timer: no 1ms minimum clamp for zero-period periodic timers — could spin-fire
2. Timer: no IsRunning() API — TimerEntry::active exists but isn't exposed
3. Timer: no abortSignal on cancel — must abort separately
4. Clock counter double-increment — cosmetic, comparisons still work

---
Tier 1: View — EARLY STAGE

┌───────────────────┬─────────────────────────────────────────────┐
│       File        │                   Status                    │
├───────────────────┼─────────────────────────────────────────────┤
│ emView            │ ~5% fidelity, 29 BEHAVIORAL, 1 CRITICAL     │
├───────────────────┼─────────────────────────────────────────────┤
│ emViewAnimator    │ Skeletal scaffold, 6 DEFICIENT of 15 mapped │
├───────────────────┼─────────────────────────────────────────────┤
│ emViewInputFilter │ 2 of 5 PARTIAL                              │
└───────────────────┴─────────────────────────────────────────────┘

CRITICAL: Core view mechanics entirely missing — no ViewPort, no per-panel viewing flags, no coordinate transforms, no
SVP system, no input dispatch.

Specific bugs:
- SpeedingViewAnimator: exact == 0.0 float comparison for idle check
- VisitingViewAnimator: convergence check tests pre-scroll state (always runs one extra frame)
- KeyboardZoomScrollVIF: Arrow keys process without Alt modifier (conflicts with widget focus), PageUp/PageDown do
scroll instead of zoom, no IsFocused() guard
- Neither VIF checks VF_NO_USER_NAVIGATION

---
Tier 2: Panel — SIGNIFICANT GAPS

┌─────────────────┬───────┐
│     Verdict     │ Count │
├─────────────────┼───────┤
│ Pass            │ 24    │
├─────────────────┼───────┤
│ Flag            │ 8     │
├─────────────────┼───────┤
│ Missing methods │ 39    │
└─────────────────┴───────┘

Critical issues (3):
1. create_child doesn't inherit parent's computed enabled state
2. create_root silently overwrites self.root if called twice (orphans previous root)
3. set_layout_rect has no minimum width/height clamp — zero width → division by zero

Moderate:
- deliver_notices iterates in arbitrary SlotMap order (should be parent-before-child)
- set_focusable missing root-unfocusable guard and active-panel migration
- PanelBehavior::cycle returns () not bool — loses wake/sleep optimization

---
Tier 2: Border — FUNCTIONAL BUT SIMPLIFIED

┌─────────┬────────────┬──────────┬─────────────────┐
│ CORRECT │ BEHAVIORAL │ COSMETIC │ Missing Methods │
├─────────┼────────────┼──────────┼─────────────────┤
│ 31      │ 24         │ 8        │ 25              │
└─────────┴────────────┴──────────┴─────────────────┘

Key gaps: No icon support, no how-to system, no disabled-state blending (75% transparency / 80% field blend), fixed
pixel insets instead of proportional BorderScaling, round rects drawn as rectangles, no bg fill for
OBT_RECT/OBT_GROUP.

---
Tier 2: Layout — DIVERGENT ALGORITHMS

┌─────────┬────────────┬────────────────────────────────────────────┐
│ CORRECT │ DIVERGENCE │                   Count                    │
├─────────┼────────────┼────────────────────────────────────────────┤
│ 42      │ 9          │ (+ 3 MISSING, 27 INTENTIONAL for emTiling) │
└─────────┴────────────┴────────────────────────────────────────────┘

Key divergences:
1. LinearLayout: Relative spacing → absolute spacing, no iterative force redistribution (CalculateForce), no
tallness-based constraints
2. RasterLayout defaults: prefCT 0.2→1.0, minCT 1E-4→0.0, maxCT 1E4→∞, log-error→linear-error for auto-grid
3. PackLayout scoring: ratio-cubed (scale-invariant) → absolute difference (not scale-invariant), Pack3 tests 6
arrangements vs Rust's 4
4. PackLayout defaults: DefaultPCT 0.2→1.0

These will produce visibly different layouts for the same child configuration.

---
Tier 2: TextField — 2 BUGS FOUND

┌─────────────────┬─────┬───────┐
│     CORRECT     │ BUG │ MINOR │
├─────────────────┼─────┼───────┤
│ 18              │ 2   │ 1     │
├─────────────────┼─────┼───────┤
│ Missing methods │ 52  │       │
└─────────────────┴─────┴───────┘

BUG-001: Double undo entry on type-over selection — save_undo() called at line 231 before delete_selection() at line
232, which calls save_undo() again. User must press undo twice.

BUG-002: Selected text color not swapped — Rust paints all text with input_fg_color regardless of selection. Selected
text may be unreadable.

MINOR: set_text() doesn't clear undo/redo stacks (C++ does), allowing undo past programmatic replacement.

---
Tier 3: API Coverage

┌────────────────┬────────┬─────────┬─────────────┐
│   Subsystem    │ Mapped │ Missing │ Intentional │
├────────────────┼────────┼─────────┼─────────────┤
│ Color          │ 12     │ 9       │ 9           │
├────────────────┼────────┼─────────┼─────────────┤
│ Image          │ 8      │ 19      │ 5           │
├────────────────┼────────┼─────────┼─────────────┤
│ Simple Widgets │ ~20    │ 3       │ many        │
├────────────────┼────────┼─────────┼─────────────┤
│ Window/Screen  │ 7      │ 20      │ 32          │
└────────────────┴────────┴─────────┴─────────────┘

Image is notably skeletal (no blit, affine transform, cropping, channel conversion, interpolation).

---
Blocked: Painter

The emPainter.cpp (3644 lines, 735+ branches) extract agent failed to complete. The ScTl companion files were
extracted successfully. To finish the painter verification, a chunked re-extraction is needed.

---
Priority Action Items

Fix now (bugs):
1. TextField double undo on type-over (text_field.rs:231-232)
2. TextField selected text color not swapped (text_field.rs:145-151)
3. Panel create_child not inheriting parent enabled state
4. Panel set_layout_rect missing min-dimension clamp (div-by-zero risk)

Fix soon (correctness):
5. Timer: add 1ms floor for zero-period periodic timers
6. Panel create_root should error on double-call, not silently orphan
7. Panel deliver_notices should iterate parent-before-child
8. KeyboardZoomScrollVIF: require Alt for arrow keys, zoom on PageUp/Down

Design review needed:
9. Layout default values (prefCT 1.0 vs C++ 0.2) — intentional or accidental?
10. Layout scoring (linear vs log-error for raster, absolute vs ratio-cubed for pack)
11. View subsystem is ~5% complete — needs architectural plan before filling gaps
