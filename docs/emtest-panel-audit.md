# emTestPanel Spec-Compliance Audit

C++ ground truth: `~/Projects/eaglemode-0.96.4/src/emTest/emTestPanel.cpp` + `.h`
Rust port: `crates/emtest/src/emTestPanel.rs` + `lib.rs`
Audit conducted: 2026-04-30

**Known issues excluded from this audit (already identified before audit):**
1. AutoExpand/LayoutChildren mismatch — all four panels (`TestPanel`, `TkTestGrpPanel`,
   `TkTestPanel`, `PolyDrawPanel`) create first children inside `LayoutChildren`. Rust
   emCore only calls `LayoutChildren` when `GetFirstChild(id).is_some()`, so no children
   are ever created. Fix: implement `AutoExpand` for first-time child creation; leave
   `LayoutChildren` for positioning only.
2. Missing emSplitter hierarchy in `TkTestGrpPanel` — C++ `TkTestGrp::AutoExpand` builds
   `sp → sp1 + sp2 → TkTest panels`; Rust lays out t1a/t1b/t2a/t2b in a hardcoded 2×2
   grid. DIVERGED annotation present; emSplitter not yet ported.

---

## Scope & Policy Decisions (2026-04-30)

Recorded after verify-then-decide investigation and scoping review.

**Verified not bugs — closed:**
- **I-8**: `EnableAutoDeletion(true)` is correct — Rust API is explicit-bool, C++ no-arg defaults to true. Equivalent.
- **I-16 / I-17**: `viewed_rect.w` is view pixels — equivalent to C++ `GetViewCondition(VCT_WIDTH)`. Guard placement verified correct.
- **I-18**: `memory_limit` is `u64`; formats correctly with `{}`. No divergence.

**Verified language-forced:**
- **C-25**: Sub-painter requires a second exclusive `&mut emImage` borrow — Rust borrow checker forbids it. DIVERGED annotation needed (language-forced). Current workaround (push_state/SetClipping/pop_state) clips correctly but does **not** shift the coordinate origin; add to golden verification to confirm pixel equivalence.
- **I-9**: Will be un-DIVERGED when ConstructCtx gains `view_context()` (follows from I-6 decision below); existing DIVERGED block remains until that PR lands.

**Scope decisions:**
- **PolyDrawPanel full port** (C-1, C-2, C-3–C-6, I-14, I-15): port now; requires its own plan.
- **CustomListBox recursion** (C-23, C-24, I-11–I-13): port now.

**Policy decisions:**
- **I-6**: Extend `ConstructCtx` with `view_context()` — restores C++ view-scoped emVarModel storage and un-DIVERGES I-9.
- **I-2**: Remove `MAX_DEPTH=10` cap — rely on AE threshold like C++.

---

## CRITICAL — observable behavior differs or feature missing entirely

### C-1. PolyDrawPanel control sub-tree entirely missing
- **C++** (cpp:1071–1261): `PolyDrawPanel::AutoExpand` builds a `Controls` raster layout
  with four sub-groups (general / stroke / strokeStart / strokeEnd) containing ~22
  widgets: `Type` radio group (16 method radios), `VertexCount`, `WithCanvasColor`,
  `FillColor`, `StrokeWidth`, `StrokeColor`, `StrokeRounded`, `StrokeDashType` (4 radios),
  `DashLengthFactor`, `GapLengthFactor`, `StrokeStartType` (17 radios),
  `StrokeStartInnerColor`, `StrokeStartWidthFactor`, `StrokeStartLengthFactor`,
  `StrokeEndType` (17 radios), `StrokeEndInnerColor`, `StrokeEndWidthFactor`,
  `StrokeEndLengthFactor`. All wired with `AddWakeUpSignal`.
- **Rust** (emTestPanel.rs:2188–2228): `PolyDrawPanel::LayoutChildren` creates only
  `CanvasPanel`. No control widgets at all. Comment at lines 2178–2186 calls this
  "deferred to a later task."
- **Decision**: Full port planned (scope decision 1a). Requires its own plan.

### C-2. PolyDrawPanel has no Cycle / CanvasPanel::Setup plumbing
- **C++** (cpp:1015–1068): `PolyDrawPanel::Cycle` reacts to all 18 control signals and
  calls `Canvas->Setup(...)` to update Type, vertex count, textures, stroke, stroke ends.
- **Rust**: `PolyDrawPanel` has no `Cycle`. `CanvasPanel::Setup` does not exist.
  `CanvasPanel` fields `_stroke_width` and `_stroke_color` are dead (`_`-prefixed). `Type`
  is hardcoded to `PaintPolygon`.

### C-3. CanvasPanel renders only one of 16 Type cases
- **C++** (cpp:1405–1461): 16-way switch on `Type` covering PaintPolygon / PolygonOutline
  / Polyline / Bezier / BezierOutline / BezierLine / Line / Rect / RectOutline / Ellipse /
  EllipseOutline / EllipseSector / EllipseSectorOutline / EllipseArc / RoundRect /
  RoundRectOutline.
- **Rust** (emTestPanel.rs:2387): unconditionally `PaintPolygon`. Comment at line 2240
  acknowledges as deferred.

### C-4. CanvasPanel handle drawing does not match C++
- **C++** (cpp:1463–1483): handle color depends on `Type` and vertex index — yellow for
  non-anchor bezier control points; gray for unused vertices when `m` truncates count;
  outline alpha 128.
- **Rust** (emTestPanel.rs:2390–2409): green/white based on drag state only; no Type-aware
  colorization; no `m`-truncation gray coloring.

### C-5. CanvasPanel WithCanvasColor background path missing
- **C++** (cpp:1372–1386): if `WithCanvasColor`, paints solid `emColor(96,128,160)`
  background and forwards `canvasColor`; otherwise paints linear gradient and zeros canvas
  color.
- **Rust** (emTestPanel.rs:2367–2377): always paints linear gradient; no solid path;
  canvas color not zeroed.

### C-6. CanvasPanel Setup vertex re-layout missing; y-coordinate formula wrong
- **C++** (cpp:1284–1295): when vertex count grows, generates points using
  `cos/sin*0.4+0.5` for x and `GetHeight()*(sin*0.4+0.5)` for y.
- **Rust** (emTestPanel.rs, constructor): pre-creates 9 vertices as
  `(cos·0.4+0.5, sin·0.4+0.5)` — drops the `GetHeight()` y-scaling. Never resizes
  vertex array because `Setup` is never called.

### C-7. CanvasPanel GetHeight() used as y-bound throughout; Rust hardcodes 1.0
- **C++** uses `GetHeight()` in three places: (a) drag y-clamp (cpp:1341):
  `y = emMin(emMax(my+DragDY, 0.0), GetHeight())`; (b) ShowHandles bounds check (cpp:1354):
  `my >= 0.0 && my < GetHeight()`; (c) vertex initial layout (cpp:1292): covered in C-6.
- **Rust**: drag clamp (emTestPanel.rs:2335) uses `.clamp(0.0, 1.0)`; ShowHandles check
  uses `(0.0..1.0).contains(&my)`. When the canvas panel has non-unit height, both sites
  diverge. The snapping path also inherits the wrong y-bound since `raw_y` is already
  clamped to 1.0 before grid-rounding.

### C-8. CanvasPanel: event.Eat() and Focus() unconditional on left-press; Rust omits both
- **C++** (cpp:1315–1317): `event.Eat(); Focus();` on ANY left-press regardless of whether
  a vertex was hit (bestI may be -1). Both calls happen before the vertex search result is
  tested.
- **Rust** (emTestPanel.rs:2295–2324): `Focus()` is never called. The event is considered
  consumed only when a vertex is hit (`return best_i.is_some()`). Clicking the canvas on
  empty space neither acquires focus nor eats the event.

### C-9. CanvasPanel missing InvalidatePainting calls in four places
- **C++** calls `InvalidatePainting()` when: (a) vertex hit and drag starts (cpp:1332);
  (b) drag stops (cpp:1338); (c) vertex position actually changed during drag — guarded by
  `if (XY[DragIdx*2]!=x || XY[DragIdx*2+1]!=y)` (cpp:1344–1347); (d) ShowHandles value
  changes (cpp:1357–1359).
- **Rust**: no `InvalidatePainting` calls in `Input`. Also, `self.vertices[idx]` is always
  overwritten unconditionally (no change-guard equivalent to the C++ check).

### C-10. CanvasPanel handle radius formula wrong
- **C++** (cpp:1464): `r = emMin(ViewToPanelDeltaX(12.0), 0.05)`.
- **Rust** (emTestPanel.rs:2391): `r = (0.05).min(12.0 / w.max(1.0))` — uses local panel
  width `w` instead of a view-pixel-to-panel mapping. Produces different values.

### C-11. CanvasPanel help-text geometry wrong when h ≠ 1
- **C++** (cpp:1485–1490): `PaintTextBoxed(0.0, GetHeight()-0.03, 1.0, 0.03, ..., 0.03)`
  — fixed 0.03 in panel coords.
- **Rust** (emTestPanel.rs:2412–2427): multiplies everything by `h`; produces wrong sizes
  when h ≠ 1.

### C-12. TestPanel Notice() missing
- **C++** (cpp:74–78): `Notice(flags)` calls `UpdateControlPanel()` and
  `InvalidatePainting()` on every notice.
- **Rust**: no `notice()` impl on `TestPanel`. Control panel will not update on state
  change; painting will not invalidate.

### C-13. TestPanel Cycle() missing
- **C++** (cpp:62–71): `Cycle()` watches `BgColorField->GetColorSignal()`; on signal:
  assigns `BgColor`, `UpdateControlPanel()`, `InvalidatePainting()`,
  `InvalidateChildrenLayout()`.
- **Rust**: `TestPanel` has no `Cycle`. Replaced by `on_color` callback that writes to
  `bg_shared` (lines 1222–1224). Neither `InvalidatePainting()` nor
  `InvalidateChildrenLayout()` is called. Children's canvas color stays stale until
  something else triggers relayout. DIVERGED block at lines 467–469 present but the
  observable consequence (stale canvas color) makes this a fidelity bug.

### C-14. AddWakeUpSignal(BgColorField->GetColorSignal()) not honored
- **C++** (cpp:495): explicitly added.
- **Rust**: no `AddWakeUpSignal`/connect on color field signal in `TestPanel`; bypassed
  with `on_color` callback.

### C-15. TestPanel Input() missing InvalidatePainting and full state log
- **C++** (cpp:88–105): appends log entry with full `STATE: pressed=k1,k2,... mouse=mx,my`
  (scans all 256 input keys), calls `InvalidatePainting()`, then forwards to
  `emPanel::Input`.
- **Rust** (emTestPanel.rs:1145–1161): omits `STATE: pressed=...` entirely (`_input_state`
  is discarded); no `InvalidatePainting`; returns false without forwarding to base.

### C-16. TestPanel paint_primitives: emImageColoredTexture polygon missing
- **C++** (cpp:441–451): `PaintPolygon` with `emImageColoredTexture` — colored-image
  texture fill on star polygon.
- **Rust**: no `paint_polygon_textured` call using `emImageColoredTexture`. Only plain
  `emImage` variant is used.

### C-17. TestPanel paint_primitives: EXTEND_TILED/EDGE/ZERO rect modes missing
- **C++** (cpp:453–478): three `PaintRect` calls demonstrating `EXTEND_TILED`,
  `EXTEND_EDGE`, `EXTEND_ZERO` extend modes.
- **Rust**: missing entirely.

### C-18. TestPanel paint_primitives: linear gradient parameter mismatch
- **C++** (cpp:415–419): `PaintRect(0.207, 0.944, 0.013, 0.006, emLinearGradientTexture(...))`
  with explicit gradient-line endpoints separate from rect bounds.
- **Rust** (emTestPanel.rs:964–973): `paint_linear_gradient(0.2, 0.94, 0.02, 0.01, ...)`
  with collapsed signature; rect bounds and gradient-line endpoints differ.

### C-19. TestPanel paint_primitives: radial gradient parameter mismatch
- **C++** (cpp:420–423): rect `(0.221, 0.94, 0.008, 0.01)`, gradient origin/radii
  `(0.223, 0.941, 0.004, 0.008)` — separate rect and gradient coords.
- **Rust** (emTestPanel.rs:974–982): `paint_radial_gradient(0.225, 0.946, 0.004, 0.008, ...)`
  — different numbers; collapsed signature.

### C-20. TestPanel paint: image-texture tile pattern wrong
- **C++** (cpp:430–439): `PaintRect(0.26, 0.94, 0.02, 0.01, emImageTexture(0.26, 0.94, 0.001, 0.001*ratio, TestImage))`
  — texture coords use width 0.001, not rect width 0.02.
- **Rust** (emTestPanel.rs:994–1002): `paint_image_scaled(0.26, 0.94, 0.02, 0.01, ...)` —
  uses rect dimensions for texture, so tile pattern differs.

### C-21. TestPanel "Test Panel" caption text alignment wrong
- **C++** (cpp:134–141): inner horizontal alignment is `EM_ALIGN_CENTER` (default);
  formatTallness is 0.2 (default).
- **Rust** (emTestPanel.rs:1062–1077): inner align is `Left`; formatTallness is 0.5.
  Both wrong.

### C-22. TestPanel ellipse demo is solid-fill instead of gradient
- **C++** (cpp:425–428): `PaintEllipse(...)` with `emRadialGradientTexture(...)`.
- **Rust** (emTestPanel.rs:984–991): solid `emColor::rgba(0, 0xCC, 0x88, 0xFF)`. Comment
  at line 983 labels it "solid fallback." Observable difference.

### C-23. CustomItemPanel AutoExpand recursion entirely absent
- **C++** (cpp:941–956): `CustomItemPanel::AutoExpand` creates `emLabel(this,"t",...)` +
  `CustomListBox(this,"l",...)` with 7 items, multi-selection — fully recursive.
- **Rust**: `CustomItemBehavior` has no `AutoExpand`. Items never recurse. Recursive list
  box is missing entirely.
- **Decision**: Port now (scope decision 2a).

### C-24. CustomItemPanel Input / ProcessItemInput missing
- **C++** (cpp:932–938): `Input` calls `ProcessItemInput(this, event, state)` then
  `emLinearGroup::Input(...)`.
- **Rust**: `CustomItemBehavior` has no `Input` impl. Per-item input (toggle on click) does
  not reach `ProcessItemInput`.

### C-25. TestPanel paint: clipping mechanism wrong (sub-painter vs. push_state/SetClipping)
- **C++** (cpp:225–231): creates a new `emPainter` with restricted origin/scale — both
  clips AND re-maps coordinate origin.
- **Rust** (emTestPanel.rs:574–585): `push_state / SetClipping / pop_state` — clips without
  shifting origin. The polygon coordinates inside would need adjusting to compensate.
  Observable for golden tests.
- **Decision**: Language-forced DIVERGED — sub-painter would require a second exclusive
  `&mut emImage` borrow; Rust borrow checker forbids it. Add `DIVERGED: language-forced`
  annotation. Add to golden verification to confirm pixel equivalence of current workaround.

### C-26. CanvasPanel emPanel::Input base-class forwarding missing
- **C++** (cpp:1361): `emPanel::Input(event, state, mx, my)` called at the end of
  `CanvasPanel::Input`. The base class handles cursor style changes and additional
  per-panel bookkeeping.
- **Rust** (emTestPanel.rs:~2345): returns `false` with no equivalent forwarding. If the
  Rust emCore base Input path handles anything observable (cursor changes, etc.), it is
  skipped.

---

## IMPORTANT — structural gaps that will compound

### I-1. SetAutoExpansionThreshold(900.0) not set on root TestPanel itself
- **C++** (cpp:39): `SetAutoExpansionThreshold(900.0, VCT_AREA)` called in
  `emTestPanel` constructor — applies to every instance including root.
- **Rust** (emTestPanel.rs:1192, 1206): threshold is set on `tktest_id` and each TP1–TP4
  child but NOT on `ctx.id` (the TestPanel itself). Root TestPanel uses default threshold
  (150.0 area). Observable: root panel expands at a different zoom level than C++.
  No DIVERGED annotation.

### I-2. MAX_DEPTH = 10 hard cap with no DIVERGED annotation
- **C++**: no explicit depth limit. Recursive `emTestPanel` children are always created in
  `AutoExpand`; Eagle Mode relies on the area threshold to stop expansion at deep zoom-out.
- **Rust** (emTestPanel.rs:54, 1200–1213): hard cap at depth 10. Observable at deep zoom.
  No DIVERGED annotation — per Port Ideology this is silent drift.
- **Decision**: Remove cap (policy decision 3a) — rely on AE threshold like C++.

### I-3. TestPanel Cycle/Notice bg_shared Cell is a polling intermediary
- `bg_shared` is a `Cell` written in `on_color`, read by `Paint` and `Drop`. No
  `InvalidatePainting()` or `InvalidateChildrenLayout()` after write. CLAUDE.md
  "Polling intermediaries" rule: remove it; fire synchronously via `ectx`.

### I-4. signals_connected / deferred-connect pattern
- **C++**: signals wired in constructor; fire from frame 1.
- **Rust** (emTestPanel.rs:2042–2061 `TkTestPanel::Cycle`; lines 298–303
  `ScalarFieldWithDynamicMax::Cycle`): defers `ectx.connect` to first Cycle. A signal
  fired synchronously in tests before the first engine cycle would be missed.

### I-5. sf5↔sf6 Cell-based value pipe is a polling intermediary
- **C++** (cpp:638): `AddWakeUpSignal(SFLen->GetValueSignal())` wired in constructor;
  `Cycle` handles from frame 1.
- **Rust** (emTestPanel.rs:261–269): `sf6_max` `Cell` written in sf5's `on_value`,
  drained by sf6's `Cycle`. DIVERGED block present but CLAUDE.md explicitly prohibits
  this pattern. One-tick drift.

### I-6. emVarModel storage context: GetView() vs root_ctx
- **C++** (cpp:32–48): `emVarModel<emColor>::GetAndRemove(GetView(), ...)` in constructor;
  `Set(GetView(), ...)` in destructor. Storage is scoped to the emView context.
- **Rust** (emTestPanel.rs:1171, 1013): `emVarModel::GetAndRemove(&self.root_ctx, ...)` and
  `Set(&self.root_ctx, ...)`. Storage is scoped to the root context (application-global).
- emView and root context are different levels in the context hierarchy. Within a pure Rust
  session this is self-consistent, but: (a) BgColor persistence is app-global in Rust vs
  view-scoped in C++ — if two views show the same panel path, they share a stored color in
  Rust but are independent in C++; (b) no DIVERGED annotation explaining the scope change.
- **Decision**: Extend `ConstructCtx` with `view_context()` (option a) — restores C++
  view-scoped storage. Also un-DIVERGES I-9 as a side effect.

### I-7. dialog construction order differs
- **C++** (cpp:799): flags set at `emDialog` construction before content panel created.
- **Rust** (emTestPanel.rs:2158–2166): `emDialog::new` (no flags), then buttons/title,
  then `set_view_window_flags`, then `set_content_behavior`, then `show`. Flags applied
  after layout state built. May affect popup-zoom-aware initial sizing.

### I-8. EnableAutoDeletion called with explicit `true` arg *(CLOSED — not a bug)*
- **C++** (cpp:801): `EnableAutoDeletion()` — no-arg.
- **Rust** (emTestPanel.rs:2160): `EnableAutoDeletion(true)`. Rust API takes explicit bool;
  C++ no-arg defaults to enabled. Call passes `true` — equivalent behavior. No action needed.

### I-9. CbTopLev semantics ignored
- **C++** (cpp:790): when checked, dialog parented to `GetRootContext()` (top-level);
  else `GetView()`.
- **Rust** (emTestPanel.rs:2150–2157): `cb_toplev` read but value ignored; always uses
  same parent. Existing DIVERGED block is correct — `ConstructCtx` exposes only
  `root_context()`, no `view_context()` variant.
- **Decision**: Un-DIVERGE when `ConstructCtx` gains `view_context()` (follows from I-6
  decision). Until that PR lands, existing DIVERGED annotation stands.

### I-10. IsViewFocused() mapped to state.window_focused
- **C++** (cpp:148): `IsViewFocused()` — per-view focus.
- **Rust** (emTestPanel.rs:1091): `state.window_focused` — per-window. Diverges when
  multiple views share a window.

### I-11. CustomItemPanel ItemTextChanged / caption update missing
- **C++** (cpp:959–962): `ItemTextChanged` overrides `SetCaption(GetItemText())`.
- **Rust**: `CustomItemBehavior` stores text once at construction (line 1864); no observer
  for item-text changes. Renaming an item won't update display.

### I-12. CustomListBox layout properties missing
- **C++** (cpp:992–994): `SetChildTallness(0.4)`, `SetAlignment(EM_ALIGN_TOP_LEFT)`,
  `SetStrictRaster()`.
- **Rust** (emTestPanel.rs:1854): creates a plain `emListBox` with no override. Visual
  layout will differ.

### I-13. CustomItemBehavior captures outer look at factory creation
- **C++** (cpp:980): `SetLook(GetListBox().GetLook())` — listbox's live look.
- **Rust** (emTestPanel.rs:1853–1869): `lb7_look = look.clone()` captured once at factory
  creation. If look is later changed on the listbox, items don't update.

### I-14. PolyDrawPanel SetOrientationThresholdTallness(1.0) not applied
- **C++** (cpp:1011): `SetOrientationThresholdTallness(1.0)` — switches between
  horizontal/vertical layout based on aspect ratio.
- **Rust** (emTestPanel.rs:2192–2197): acknowledged missing in comment. Always horizontal
  regardless of aspect.

### I-15. PolyDrawPanel caption and description missing
- **C++** (cpp:1004–1010): `emLinearGroup(parent, name, "Poly Draw Test",
  "This allows manual testing...")`.
- **Rust** (emTestPanel.rs:2192–2197): `emLinearGroup::horizontal()` only — no caption
  and no description set.

### I-16. TestPanel viewed-rect width threshold *(CLOSED — not a bug)*
- **C++** (cpp:143): `if (GetViewCondition(VCT_WIDTH) < 25.0) return` — view-pixel width
  of the panel.
- **Rust** (emTestPanel.rs:1079): `if state.viewed_rect.w < 25.0` — verified: `viewed_rect`
  is in absolute view pixels. Semantically equivalent to C++. No action needed.

### I-17. emTestPanel paint: early-return guard placement *(CLOSED — not a bug)*
- **C++** (cpp:143–145): entire Paint (except outer rect outline) is skipped if view too
  small.
- **Rust** (emTestPanel.rs:1079–1081): verified equivalent — same check, same placement.
  No action needed.

### I-18. TestPanel pri/memlim format *(CLOSED — not a bug)*
- **C++** (cpp:151–156): `(unsigned long)GetMemoryLimit()` — unsigned.
- **Rust** (emTestPanel.rs:1111): `memory_limit` is `u64`; prints correctly with `{}`.
  No action needed.

### I-19. emVarModel::Set count parameter missing
- **C++** (cpp:47): `Set(GetView(), key, BgColor, 10)` — count=10 controls how many
  VarModel instances of this key can coexist before eviction.
- **Rust**: `Set(ctx, key, value)` — no count parameter in the Rust emVarModel::Set
  signature. Lifetime behavior for deeply-nested panels may differ.

---

## MINOR — cosmetic, naming, or non-observable

### M-1. make_star polygon helper has no RUST_ONLY annotation
- C++ inlines the textured-polygon vertices manually (cpp:372–413). Rust introduces
  `make_star` (emTestPanel.rs:2433–2435) with a comment but no `RUST_ONLY:` annotation
  per CLAUDE.md.

### M-2. emTestPanel.rs paint_primitives: sub-painter coordinate system needs golden verification
- Rust paints in `[0,1] × [0, h/w]` panel space via `painter.scale(w, w)` (lines
  1049–1050). C++ uses `[0,1] × [0, h]` with no scale. Verify with goldens that rendered
  pixel positions match.

### M-3. emGetInsResImage category differs; resource resolves correctly but needs DIVERGED annotation
- **C++**: `emGetInsResImage(GetRootContext(), "icons", "teddy.tga")` → `$EM_DIR/res/icons/teddy.tga`.
- **Rust**: `emGetInsResImage("emTest", "icons/teddy.tga")` → `$EM_DIR/res/emTest/icons/teddy.tga`.
  The Rust resource IS present at `res/emTest/icons/teddy.tga`; it resolves correctly. This
  is a dependency-forced divergence (cdylib resource layout differs from C++ monolith) but
  carries no `DIVERGED: dependency-forced` annotation at the call site.

### M-4. Stale task reference in comment
- `emTestPanel.rs:1227–1228`: "flat placeholder; Task 11 restructures" — leftover
  task reference; should be removed.

### M-5. DIVERGED annotation categories audit
- All present DIVERGED blocks should carry one of the four required categories
  (language-forced / dependency-forced / upstream-gap-forced / performance-forced).
  Lines 261–269, 1291–1294, 1376–1381, 2150–2157 include categories. Verify none are
  missing their category tag (annotation lint catches this at pre-commit).

### M-6. AddNegativeButton / EnableAutoDeletion / SetRootTitle order vs C++
- **C++** (cpp:800–803): `AddNegativeButton("Close")`, `EnableAutoDeletion()`,
  `SetRootTitle("Test Dialog")`, then content added.
- **Rust** (emTestPanel.rs:2159–2165): same logical order but with `set_view_window_flags`
  inserted between title and content. Observable only if flag application timing matters
  for initial sizing.

### M-7. paint_polygon_even_odd vs C++ built-in even-odd needs golden verification
- C++ relies on `PaintPolygon`'s built-in even-odd winding. Rust calls
  `paint_polygon_even_odd`. Verify with goldens that pixel output matches.

### M-8. emColor::rgba(187, 255, 255, 255) — C++ uses 3-arg (alpha=255 default)
- Equivalent. No action needed.

### M-9. emCrossPtr → name-lookup via find_child_by_name
- Idiom adaptation, no action needed.

### M-10. TkTest SetBorderScaling / SetPrefChildTallness per-group
- `make_category` always sets `border_scaling=2.5` and optional pct; matches C++.
  dlgs/fileChoosers inlined with correct values. No gaps found.

### M-11. RadioGroup shared across r1-r6 confirmed correct
- All six radio widgets created under one `RadioGroup` per C++ `emRadioButton::RasterGroup`
  pattern. Confirmed correct.

---

## SUMMARY

**PolyDrawPanel/CanvasPanel** (C-1 through C-11, C-26): largest gap. The entire control
sub-tree and all 15 non-PaintPolygon render types are missing. CanvasPanel drag (missing
unconditional Focus/Eat, missing InvalidatePainting in four places, wrong y-bound in two
places, missing base-Input forwarding), handle radius, background, and help-text all
diverge. Complete unported subsystem.

**CustomListBox recursion** (C-23, C-24, I-11, I-12, I-13): `CustomItemPanel::AutoExpand`
(recursive list + label) is absent; per-item input missing; caption update missing; layout
properties missing.

**TestPanel signal/notice flows** (C-12, C-13, C-14, C-15, I-3, I-4, I-5): `Cycle` and
`Notice` both absent; `Input` missing `InvalidatePainting` and state log; three Cell-based
polling-intermediary patterns violate CLAUDE.md.

**TestPanel Paint** (C-16 through C-22, C-25, M-2): seven paint-call-level divergences —
missing texture modes, wrong alignment params, solid fallback for gradient ellipse, image
tile formula, gradient parameter shapes, clipping mechanism.

**Structural** (I-1, I-2, I-6, I-7 through I-10, I-14 through I-19): auto-expansion
threshold not set on root; no-DIVERGED depth cap; emVarModel stored at wrong context scope
(root vs view) with missing count param; dialog flag ordering; IsViewFocused semantics;
PolyDrawPanel caption/description/orientation; viewed-rect type ambiguity.

**Annotation gaps** (M-1, M-3, M-5): emGetInsResImage call and make_star helper missing
DIVERGED/RUST_ONLY annotations; stale task-reference comment.
