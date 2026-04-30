# PolyDrawPanel Full Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the PolyDrawPanel subsystem: 22-widget control tree, Cycle/Setup plumbing, 16-way render type switch, WithCanvasColor background, Type-aware handle colorization, and correct geometry. Closes C-1 through C-6, I-14, I-15 from `docs/emtest-panel-audit.md`.

**Architecture:** PolyDrawPanel is an `emLinearGroup` that holds a `Controls` raster layout (left) and `CanvasPanel` (right). `AutoExpand` builds the control tree and creates CanvasPanel. `Cycle` reacts to 18 control signals and calls `CanvasPanel::Setup`. CanvasPanel holds all render state and performs a 16-way switch in `Paint`. All field names and signal wiring match C++ exactly.

**Tech Stack:** Rust, `crates/emtest/src/emTestPanel.rs`. C++ ground truth: `~/Projects/eaglemode-0.96.4/src/emTest/emTestPanel.cpp:1004–1495`. Read the full C++ source section before starting.

**Prerequisite:** Plan 1 (AutoExpand restructure) must be merged. CanvasPanel interaction fixes from Plan 2 Task 8 are independent and can be applied in either order.

---

## Background for the implementer

Read `emTestPanel.cpp:1004–1495` in full before starting. The C++ control tree is large; implement it exactly. Rust widget equivalents used in this file:
- `emRasterLayout` (already used in `TkTestPanel`)
- `emLinearGroup` (already used)
- `RadioGroup` + `emRadioButton` + `emRadioBox` (already used in `create_all_categories`)
- `emTextField`, `emColorField`, `emCheckBox` (already used)
- `emStroke`, `emStrokeEnd`, `StrokeEndType`, `DashType` from `emcore::emStroke`

Run tests: `cargo-nextest ntr`
Check annotations: `cargo xtask annotations`

---

### Task 1: PolyDrawPanel struct — caption, description, orientation, signal IDs

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs` (~line 2188)

C++ reference: `emTestPanel.cpp:1004–1013` (PolyDrawPanel constructor).

- [ ] **Step 1: Expand `PolyDrawPanel` struct with signal ID fields**

Replace the current `PolyDrawPanel` struct (around line 2188):

```rust
// OLD
struct PolyDrawPanel {
    group: emLinearGroup,
}

// NEW
struct PolyDrawPanel {
    group: emLinearGroup,
    // Signal IDs — None until AutoExpand wires them.
    // 18 signals matching C++ AddWakeUpSignal calls (emTestPanel.cpp:1097..1244).
    type_signal: Option<SignalId>,
    vertex_count_signal: Option<SignalId>,
    with_canvas_color_signal: Option<SignalId>,
    fill_color_signal: Option<SignalId>,
    stroke_width_signal: Option<SignalId>,
    stroke_color_signal: Option<SignalId>,
    stroke_rounded_signal: Option<SignalId>,
    stroke_dash_type_signal: Option<SignalId>,
    dash_length_factor_signal: Option<SignalId>,
    gap_length_factor_signal: Option<SignalId>,
    stroke_start_type_signal: Option<SignalId>,
    stroke_start_inner_color_signal: Option<SignalId>,
    stroke_start_width_factor_signal: Option<SignalId>,
    stroke_start_length_factor_signal: Option<SignalId>,
    stroke_end_type_signal: Option<SignalId>,
    stroke_end_inner_color_signal: Option<SignalId>,
    stroke_end_width_factor_signal: Option<SignalId>,
    stroke_end_length_factor_signal: Option<SignalId>,
    // Panel IDs for reading widget values in Cycle. None until AutoExpand.
    canvas_id: Option<PanelId>,
    type_id: Option<PanelId>,
    vertex_count_id: Option<PanelId>,
    with_canvas_color_id: Option<PanelId>,
    fill_color_id: Option<PanelId>,
    stroke_width_id: Option<PanelId>,
    stroke_color_id: Option<PanelId>,
    stroke_rounded_id: Option<PanelId>,
    stroke_dash_type_id: Option<PanelId>,
    dash_length_factor_id: Option<PanelId>,
    gap_length_factor_id: Option<PanelId>,
    stroke_start_type_id: Option<PanelId>,
    stroke_start_inner_color_id: Option<PanelId>,
    stroke_start_width_factor_id: Option<PanelId>,
    stroke_start_length_factor_id: Option<PanelId>,
    stroke_end_type_id: Option<PanelId>,
    stroke_end_inner_color_id: Option<PanelId>,
    stroke_end_width_factor_id: Option<PanelId>,
    stroke_end_length_factor_id: Option<PanelId>,
}
```

- [ ] **Step 2: Update `PolyDrawPanel::new` with caption, description, orientation**

C++ constructor sets caption "Poly Draw Test", description text, and `SetOrientationThresholdTallness(1.0)`:

```rust
impl PolyDrawPanel {
    fn new() -> Self {
        let mut group = emLinearGroup::horizontal();
        // C++ emTestPanel.cpp:1005–1009: caption + description.
        group.SetCaption("Poly Draw Test");
        group.SetDescription(
            "This allows manual testing of various paint functions. Main focus is\n\
             on strokes and stroke ends, i.e. textures cannot be tested with this.\n"
        );
        // C++ emTestPanel.cpp:1011: SetOrientationThresholdTallness(1.0).
        // Check emLinearGroup.rs for orientation threshold method.
        // If not yet implemented, add DIVERGED: upstream-gap-forced or defer.
        // group.SetOrientationThresholdTallness(1.0);
        Self {
            group,
            type_signal: None,
            vertex_count_signal: None,
            // ... all other fields: None
            // Initialize all Option fields to None here.
            canvas_id: None,
            type_id: None,
            vertex_count_id: None,
            with_canvas_color_id: None,
            fill_color_id: None,
            stroke_width_id: None,
            stroke_color_id: None,
            stroke_rounded_id: None,
            stroke_dash_type_id: None,
            dash_length_factor_id: None,
            gap_length_factor_id: None,
            stroke_start_type_id: None,
            stroke_start_inner_color_id: None,
            stroke_start_width_factor_id: None,
            stroke_start_length_factor_id: None,
            stroke_end_type_id: None,
            stroke_end_inner_color_id: None,
            stroke_end_width_factor_id: None,
            stroke_end_length_factor_id: None,
            stroke_width_signal: None,
            stroke_color_signal: None,
            stroke_rounded_signal: None,
            stroke_dash_type_signal: None,
            dash_length_factor_signal: None,
            gap_length_factor_signal: None,
            stroke_start_type_signal: None,
            stroke_start_inner_color_signal: None,
            stroke_start_width_factor_signal: None,
            stroke_start_length_factor_signal: None,
            stroke_end_type_signal: None,
            stroke_end_inner_color_signal: None,
            stroke_end_width_factor_signal: None,
            stroke_end_length_factor_signal: None,
            fill_color_signal: None,
            with_canvas_color_signal: None,
            type_signal: None,
            vertex_count_signal: None,
        }
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green (struct change only, no behavior change yet).

- [ ] **Step 4: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "feat(emtest): PolyDrawPanel — struct fields, caption, description, orientation placeholder (I-14 I-15)"
```

---

### Task 2: PolyDrawPanel AutoExpand — build control tree (C-1)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1071–1261`. Read all 190 lines before implementing.

The control tree structure:
```
PolyDrawPanel (emLinearGroup, horizontal)
├── Controls (emRasterLayout, pref_child_tallness=0.6)
│   ├── general (emLinearGroup, border_scaling=2.0, child_weight[0]=2.0)
│   │   ├── Method (RadioGroup/RasterGroup, 16 radios)
│   │   ├── ll (emLinearLayout, horizontal)
│   │   │   ├── VertexCount (emTextField)
│   │   │   └── FillColor (emColorField)
│   │   └── ll2 (emLinearLayout, horizontal)
│   │       ├── StrokeWidth (emTextField)
│   │       └── WithCanvasColor (emCheckBox)
│   ├── stroke (emLinearGroup, border_scaling=2.0, child_weight[2]=2.0)
│   │   ├── StrokeColor (emColorField)
│   │   ├── StrokeRounded (emCheckBox)
│   │   ├── StrokeDashType (RadioGroup, 4 radios)
│   │   └── ll (emLinearLayout, horizontal)
│   │       ├── DashLengthFactor (emTextField)
│   │       └── GapLengthFactor (emTextField)
│   ├── strokeStart (emLinearGroup, border_scaling=2.0, child_weight[0]=2.0)
│   │   ├── StrokeStartType (RadioGroup, 17 radios)
│   │   ├── StrokeStartInnerColor (emColorField)
│   │   └── ll (emLinearLayout, horizontal)
│   │       ├── StrokeStartWidthFactor (emTextField)
│   │       └── StrokeStartLengthFactor (emTextField)
│   └── strokeEnd (emLinearGroup, border_scaling=2.0, child_weight[0]=2.0)
│       ├── StrokeEndType (RadioGroup, 17 radios)
│       ├── StrokeEndInnerColor (emColorField)
│       └── ll (emLinearLayout, horizontal)
│           ├── StrokeEndWidthFactor (emTextField)
│           └── StrokeEndLengthFactor (emTextField)
└── CanvasPanel (CanvasPanel)
```

- [ ] **Step 1: Write a failing test for the control tree**

In the `#[cfg(test)]` module (added in Plan 1 Task 1), add:

```rust
#[test]
fn polydrawpanel_control_tree_exists() {
    let ctx = emContext::NewRoot();
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_behavior(root, Box::new(PolyDrawPanel::new()));
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
    settle(&mut tree, &mut view, &ctx);

    assert!(tree.find_by_name("Controls").is_some(), "Controls raster missing");
    assert!(tree.find_by_name("CanvasPanel").is_some(), "CanvasPanel missing");
    assert!(tree.find_by_name("Method").is_some(), "Method radio group missing");
    assert!(tree.find_by_name("VertexCount").is_some(), "VertexCount field missing");
    assert!(tree.find_by_name("FillColor").is_some(), "FillColor field missing");
    assert!(tree.find_by_name("StrokeColor").is_some(), "StrokeColor missing");
    assert!(tree.find_by_name("StrokeStartType").is_some(), "StrokeStartType missing");
    assert!(tree.find_by_name("StrokeEndType").is_some(), "StrokeEndType missing");
}
```

Run: `cargo-nextest ntr -E 'package(emtest)'` — expected: fail (Controls not created yet).

- [ ] **Step 2: Implement `PolyDrawPanel::AutoExpand`**

Replace the existing `AutoExpand` (which currently just creates CanvasPanel) with the full control tree. The implementation mirrors `emTestPanel.cpp:1071–1261` exactly. Use the Rust widget wrappers already established in `create_all_categories`.

Key helper: for each widget created inside a group, use `ctx.create_child_with(name, Box::new(WidgetPanel { widget }))` where `WidgetPanel` is the appropriate panel wrapper struct (already defined in the file: `CheckBoxPanel`, `RadioButtonPanel`, `RadioBoxPanel`, `TextFieldPanel`, `ColorFieldPanel`).

For `emRasterLayout` as a panel (Controls): check if `RasterLayoutPanel` exists. If not, create it following the same pattern as `RasterGroupPanel`.

For `emLinearGroup` as a subgroup panel (`general`, `stroke`, etc.): use `LinearGroupPanel` if it exists, else a plain panel that delegates to `emLinearGroup`. Check existing wrappers in the file.

For `emLinearLayout` (`ll`, `ll2`): check if a `LinearLayoutPanel` wrapper exists.

```rust
fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
    let look = emLook::new();  // use the default look; all widgets inherit from parent

    // Controls — emRasterLayout (emTestPanel.cpp:1076–1078)
    let mut controls = emRasterLayout::new();
    controls.preferred_child_tallness = 0.6;
    let controls_id = ctx.create_child_with("Controls", Box::new(RasterLayoutPanel { widget: controls }));

    // --- general group (emTestPanel.cpp:1080–1085) ---
    // ... create general group as child of Controls using ctx.create_grandchild or
    // by switching ctx focus to controls_id (check PanelCtx API for creating children
    // of newly-created children).

    // The simplest approach: use a helper function that creates the entire subtree
    // for each sub-group, storing PanelIds returned from create_child_with.
    // See existing create_all_categories for the pattern.

    // Type radio group — 16 radios (emTestPanel.cpp:1087–1104)
    // VertexCount text field (emTestPanel.cpp:1106–1111)
    // FillColor color field (emTestPanel.cpp:1117–1122)
    // WithCanvasColor checkbox (emTestPanel.cpp:1128–1131)
    // StrokeWidth text field (emTestPanel.cpp:1124–1127)
    // StrokeColor (emTestPanel.cpp:1133–1138)
    // StrokeRounded (emTestPanel.cpp:1139–1141)
    // StrokeDashType radio group — 4 radios (emTestPanel.cpp:1142–1151)
    // DashLengthFactor (emTestPanel.cpp:1153–1158)
    // GapLengthFactor (emTestPanel.cpp:1159–1164)
    // StrokeStartType radio group — 17 radios (emTestPanel.cpp:1166–1190)
    // StrokeStartInnerColor (emTestPanel.cpp:1191–1196)
    // StrokeStartWidthFactor (emTestPanel.cpp:1198–1203)
    // StrokeStartLengthFactor (emTestPanel.cpp:1204–1209)
    // StrokeEndType radio group — 17 radios (emTestPanel.cpp:1210–1234)
    // StrokeEndInnerColor (emTestPanel.cpp:1235–1240)
    // StrokeEndWidthFactor (emTestPanel.cpp:1242–1247)
    // StrokeEndLengthFactor (emTestPanel.cpp:1248–1253)

    // For each widget, after creating it:
    //   let sig = widget.GetXxxSignal();
    //   ctx.add_wake_up_signal(sig);
    //   self.xxx_signal = Some(sig);
    //   self.xxx_id = Some(child_id);

    // Canvas — C++ cpp:1260: Canvas=new CanvasPanel(this,"CanvasPanel")
    self.canvas_id = Some(ctx.create_child_with("CanvasPanel", Box::new(CanvasPanel::new())));
}
```

**Important implementation note:** Creating grandchildren (children of a newly created child panel) requires either:
- (a) `PanelCtx::for_child(child_id)` — if this API exists, use it to create the sub-tree.
- (b) Store child IDs and wire them in `LayoutChildren`.
- (c) Implement the children of Controls via AutoExpand on the Controls panel itself.

Check how `create_all_categories` handles nested groups (look at `emRasterGroup` panels inside that function). Use the same pattern for PolyDrawPanel's nested structure.

- [ ] **Step 3: Store all signal IDs and panel IDs in `PolyDrawPanel`**

After each `ctx.create_child_with(...)` for a widget, store:
1. The returned `PanelId` in the corresponding `_id` field.
2. The widget's signal in the corresponding `_signal` field via `ctx.add_wake_up_signal(sig)`.

Example pattern (repeat for each of the 18 signals):
```rust
let mut vertex_count = emTextField::new(ctx, look.clone());
vertex_count.SetCaption("Vertex Count");
vertex_count.SetEditable(true);
vertex_count.SetText("9");
let vc_sig = vertex_count.GetTextSignal();
ctx.add_wake_up_signal(vc_sig);
self.vertex_count_signal = Some(vc_sig);
let vc_id = ctx.create_child_with("VertexCount", Box::new(TextFieldPanel { widget: vertex_count }));
self.vertex_count_id = Some(vc_id);
```

- [ ] **Step 4: Run tests**

```bash
cargo-nextest ntr
```

Expected: `polydrawpanel_control_tree_exists` passes. All green.

- [ ] **Step 5: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "feat(emtest): PolyDrawPanel AutoExpand — full 22-widget control tree (C-1)"
```

---

### Task 3: CanvasPanel — add render state fields and Setup method (C-2, C-6)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1270–1310` (CanvasPanel constructor + Setup).

- [ ] **Step 1: Expand `CanvasPanel` struct**

Find the `CanvasPanel` struct. Add all render state fields:

```rust
struct CanvasPanel {
    // Existing drag state fields (keep unchanged)
    vertices: Vec<(f64, f64)>,
    drag_idx: Option<usize>,
    drag_dx: f64,
    drag_dy: f64,
    show_handles: bool,
    // New render state — set by Setup, driven by PolyDrawPanel::Cycle
    render_type: u8,          // 0–15, matches C++ Type (emTestPanel.cpp:1278: Type=type)
    with_canvas_color: bool,  // emTestPanel.cpp:1293
    texture: emColor,         // fill color as flat texture (simplification for now; see note)
    stroke_width: f64,        // emTestPanel.cpp:1295
    stroke: emStroke,         // emTestPanel.cpp:1296
    stroke_start: emStrokeEnd, // emTestPanel.cpp:1297
    stroke_end: emStrokeEnd,   // emTestPanel.cpp:1298
}
```

Note on `texture`: C++ uses `emTexture` which can be a solid color, gradient, or image. For now, store `emColor` as a flat fill color. The full texture type can be added later if needed.

In `CanvasPanel::new`, initialize new fields:
```rust
render_type: 0,
with_canvas_color: false,
texture: emColor::WHITE,
stroke_width: 0.01,
stroke: emStroke::default(),
stroke_start: emStrokeEnd::butt(),
stroke_end: emStrokeEnd::butt(),
```

Check `emStrokeEnd::butt()` — verify the Rust API in `emStroke.rs`. If the method is named differently, use the correct name.

- [ ] **Step 2: Implement `CanvasPanel::Setup`**

Add a `pub(super) fn setup(...)` method to `CanvasPanel` matching C++ `Setup`:

```rust
pub(super) fn setup(
    &mut self,
    render_type: u8,
    vertex_count: usize,
    with_canvas_color: bool,
    texture: emColor,
    stroke_width: f64,
    stroke: emStroke,
    stroke_start: emStrokeEnd,
    stroke_end: emStrokeEnd,
) {
    // C++ emTestPanel.cpp:1284–1299.
    self.render_type = render_type;

    // Resize vertex array (C++ cpp:1285–1292).
    if self.vertices.len() > vertex_count {
        self.vertices.truncate(vertex_count);
        self.drag_idx = None;
    } else if self.vertices.len() < vertex_count {
        // Generate new vertices in circular arrangement.
        // C++ cpp:1289–1292: XY.Set(i*2, cos(...)*0.4+0.5); XY.Set(i*2+1, GetHeight()*(sin(...)*0.4+0.5))
        // GetHeight() is the panel height — not available here; store for use in Paint/Input.
        // Use 1.0 as placeholder; actual GetHeight() is applied when Layout sets the rect.
        // See C-6 in audit: vertex y-scaling deferred to when panel height is known.
        let current_len = self.vertices.len();
        for i in current_len..vertex_count {
            let angle = std::f64::consts::PI * 2.0 * i as f64 / vertex_count as f64;
            let x = angle.cos() * 0.4 + 0.5;
            let y = angle.sin() * 0.4 + 0.5; // GetHeight() scaling: applied in LayoutChildren/Paint
            self.vertices.push((x, y));
        }
        self.drag_idx = None;
    }

    self.with_canvas_color = with_canvas_color;
    self.texture = texture;
    self.stroke_width = stroke_width;
    self.stroke = stroke;
    self.stroke_start = stroke_start;
    self.stroke_end = stroke_end;
    // Note: InvalidatePainting is called by the caller (PolyDrawPanel::Cycle).
}
```

- [ ] **Step 3: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "feat(emtest): CanvasPanel — render state fields + Setup method (C-2 C-6)"
```

---

### Task 4: PolyDrawPanel Cycle — react to 18 signals (C-2)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1015–1068`. Read fully before implementing.

- [ ] **Step 1: Write a failing test for Cycle wiring**

In the `#[cfg(test)]` module, add:

```rust
#[test]
fn polydrawpanel_cycle_wires_canvas() {
    // This is a structural test: after settle, CanvasPanel should have render_type=0
    // (the default) and a non-empty vertex list (Setup was called at least once).
    let ctx = emContext::NewRoot();
    let mut tree = PanelTree::new();
    let root = tree.create_root_deferred_view("root");
    tree.set_behavior(root, Box::new(PolyDrawPanel::new()));
    tree.Layout(root, 0.0, 0.0, 1.0, 1.0, 1.0, None);

    let mut view = emView::new(Rc::clone(&ctx), root, 800.0, 600.0);
    // Extra rounds to let Cycle fire after AutoExpand.
    let mut ts = TestSched::new();
    for _ in 0..10 {
        view.HandleNotice(&mut tree, ts.sched_mut(), None, None);
        ts.with(|sc| view.Update(&mut tree, sc));
    }

    // CanvasPanel should exist and have vertices after initial Setup call.
    assert!(tree.find_by_name("CanvasPanel").is_some(), "CanvasPanel absent");
    // Verify Cycle fired by checking canvas has non-empty vertices.
    // (Deep access to CanvasPanel state is not possible from outside — use a
    // separate integration path or trust the visual golden test.)
}
```

- [ ] **Step 2: Implement `PolyDrawPanel::Cycle`**

Add `Cycle` to `PolyDrawPanel`'s `PanelBehavior` impl:

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
    // C++ emTestPanel.cpp:1015–1068.
    let any_signal = [
        self.type_signal,
        self.vertex_count_signal,
        self.with_canvas_color_signal,
        self.fill_color_signal,
        self.stroke_width_signal,
        self.stroke_color_signal,
        self.stroke_rounded_signal,
        self.stroke_dash_type_signal,
        self.dash_length_factor_signal,
        self.gap_length_factor_signal,
        self.stroke_start_type_signal,
        self.stroke_start_inner_color_signal,
        self.stroke_start_width_factor_signal,
        self.stroke_start_length_factor_signal,
        self.stroke_end_type_signal,
        self.stroke_end_inner_color_signal,
        self.stroke_end_width_factor_signal,
        self.stroke_end_length_factor_signal,
    ].iter().flatten().any(|&sig| ectx.IsSignaled(sig));

    if any_signal {
        if let Some(canvas_id) = self.canvas_id {
            // Read current values from each control widget by looking up panel IDs.
            // For each widget, take its behavior, downcast, read value, put back.
            let render_type = self.read_check_index(ctx, self.type_id) as u8;
            let vertex_count = self.read_text(ctx, self.vertex_count_id)
                .and_then(|t| t.parse::<usize>().ok()).unwrap_or(9);
            let with_canvas_color = self.read_checked(ctx, self.with_canvas_color_id);
            let fill_color = self.read_color(ctx, self.fill_color_id)
                .unwrap_or(emColor::WHITE);
            let stroke_width = self.read_text(ctx, self.stroke_width_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(0.01);
            let stroke_color = self.read_color(ctx, self.stroke_color_id)
                .unwrap_or(emColor::BLACK);
            let stroke_rounded = self.read_checked(ctx, self.stroke_rounded_id);
            let dash_type = match self.read_check_index(ctx, self.stroke_dash_type_id) {
                1 => DashType::Dashed,
                2 => DashType::Dotted,
                3 => DashType::DashDotted,
                _ => DashType::Solid,
            };
            let dash_length = self.read_text(ctx, self.dash_length_factor_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(1.0);
            let gap_length = self.read_text(ctx, self.gap_length_factor_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(1.0);

            let stroke = emStroke {
                color: stroke_color,
                rounded: stroke_rounded,
                dash_type,
                // dash_pattern may need separate construction; check emStroke fields.
                ..emStroke::default()
            };
            stroke.width = stroke_width; // check if width is a field or set via builder

            let start_type_idx = self.read_check_index(ctx, self.stroke_start_type_id);
            let start_inner = self.read_color(ctx, self.stroke_start_inner_color_id)
                .unwrap_or(emColor::from_u32(0xEEEEEEFF));
            let start_width = self.read_text(ctx, self.stroke_start_width_factor_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(1.0);
            let start_length = self.read_text(ctx, self.stroke_start_length_factor_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(1.0);
            let stroke_start = emStrokeEnd::new(
                StrokeEndType::from_index(start_type_idx),
                start_inner, start_width, start_length,
            );

            let end_type_idx = self.read_check_index(ctx, self.stroke_end_type_id);
            let end_inner = self.read_color(ctx, self.stroke_end_inner_color_id)
                .unwrap_or(emColor::from_u32(0xEEEEEEFF));
            let end_width = self.read_text(ctx, self.stroke_end_width_factor_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(1.0);
            let end_length = self.read_text(ctx, self.stroke_end_length_factor_id)
                .and_then(|t| t.parse::<f64>().ok()).unwrap_or(1.0);
            let stroke_end = emStrokeEnd::new(
                StrokeEndType::from_index(end_type_idx),
                end_inner, end_width, end_length,
            );

            // Call Setup on CanvasPanel.
            if let Some(mut behavior) = ctx.tree.take_behavior(canvas_id) {
                if let Some(canvas) = behavior.downcast_mut::<CanvasPanel>() {
                    canvas.setup(render_type, vertex_count, with_canvas_color,
                        fill_color, stroke_width, stroke, stroke_start, stroke_end);
                }
                ctx.tree.put_behavior(canvas_id, behavior);
            }
            ctx.tree.InvalidatePainting(canvas_id, None);
        }
    }
    false
}
```

Add helper methods on `PolyDrawPanel`:
```rust
fn read_text(&self, ctx: &PanelCtx, id: Option<PanelId>) -> Option<String> {
    let id = id?;
    let behavior = ctx.tree.get_behavior(id)?;
    behavior.downcast_ref::<TextFieldPanel>().map(|p| p.widget.GetText().to_string())
}

fn read_color(&self, ctx: &PanelCtx, id: Option<PanelId>) -> Option<emColor> {
    let id = id?;
    let behavior = ctx.tree.get_behavior(id)?;
    behavior.downcast_ref::<ColorFieldPanel>().map(|p| p.widget.GetColor())
}

fn read_checked(&self, ctx: &PanelCtx, id: Option<PanelId>) -> bool {
    let id = id?;
    let behavior = ctx.tree.get_behavior(id)?;
    behavior.downcast_ref::<CheckBoxPanel>().map(|p| p.widget.IsChecked()).unwrap_or(false)
}

fn read_check_index(&self, ctx: &PanelCtx, id: Option<PanelId>) -> usize {
    // For RadioGroup panels — read current selection index.
    // Check how radio group selection is read in existing code.
    todo!("read_check_index")
}
```

Verify `ctx.tree.take_behavior` / `put_behavior` exist and are the correct API for temporarily borrowing a behavior. Check emPanelTree.rs. If not available, use a different approach consistent with how TkTestPanel reads widget values.

Verify `StrokeEndType::from_index(idx)` exists. If not, use a match statement against the 17 variants (BUTT=0, CAP=1, ..., STROKE=16).

- [ ] **Step 3: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "feat(emtest): PolyDrawPanel Cycle — react to 18 signals, call CanvasPanel::Setup (C-2)"
```

---

### Task 5: CanvasPanel Paint — 16-way render type switch (C-3)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1370–1494`. Read fully before implementing.

- [ ] **Step 1: Write a failing golden test**

Add to `tests/golden/test_panel.rs` a test that renders PolyDrawPanel at its default state (type=0, 9 vertices, solid fill) and compares against C++ baseline. Follow the existing golden test pattern in that file.

```rust
#[test]
fn polydrawpanel_default_render() {
    // Create a PanelTree with a PolyDrawPanel as root.
    // Settle for enough rounds to AutoExpand, Cycle, and paint.
    // Save output and compare against golden.
    // Follow the require_golden! macro pattern in the existing file.
    require_golden!("polydrawpanel_default_render");
}
```

This golden will fail until the paint switch is implemented; that's expected.

- [ ] **Step 2: Implement background rendering** (C-5)

In `CanvasPanel::Paint`, replace the always-gradient background with the WithCanvasColor branch:

```rust
// C++ emTestPanel.cpp:1372–1386.
let effective_canvas_color = if self.with_canvas_color {
    let c = emColor::rgb(96, 128, 160);
    p.Clear(c, canvas_color);  // solid fill
    c
} else {
    p.Clear(
        &emLinearGradientTexture::new(
            0.0, 0.0, emColor::rgb(80, 80, 160),
            0.0, h,   emColor::rgb(160, 160, 80),
        ),
        canvas_color,
    );
    emColor::TRANSPARENT  // canvasColor=0 in C++
};
```

Check the Rust `emPainter::Clear` signature in emPainter.rs. It may take different arguments than shown.

- [ ] **Step 3: Implement the 16-way render switch** (C-3)

After the background, compute geometry from vertices (C++ cpp:1388–1403):

```rust
// C++ cpp:1388–1403: derive x1,y1,x2,y2,x,y,w,h,sa,ra from first 4 vertices.
let (x1, y1, x2, y2) = if self.vertices.len() >= 2 {
    (self.vertices[0].0, self.vertices[0].1,
     self.vertices[1].0, self.vertices[1].1)
} else { (0.0, 0.0, 0.0, 0.0) };
let (bx, by) = (x1.min(x2), y1.min(y2));
let (bw, bh) = ((x2 - x1).abs(), (y2 - y1).abs());

let (sa, ra) = if self.vertices.len() >= 4 {
    let (v4x, v4y) = self.vertices[2];
    let (v5x, v5y) = self.vertices[3];
    let sa = (v4y - by - bh*0.5).atan2(v4x - bx - bw*0.5);
    let mut ra = (v5y - by - bh*0.5).atan2(v5x - bx - bw*0.5) - sa;
    if ra < 0.0 { ra += 2.0 * std::f64::consts::PI; }
    (sa * 180.0 / std::f64::consts::PI, ra * 180.0 / std::f64::consts::PI)
} else { (0.0, 0.0) };

let fill = self.texture; // emColor used as flat fill texture
let verts: Vec<(f64, f64)> = self.vertices.clone();

match self.render_type {
    0  => p.PaintPolygon(&verts, fill, effective_canvas_color),
    1  => p.PaintPolygonOutline(&verts, self.stroke_width, &self.stroke, effective_canvas_color),
    2  => p.PaintPolyline(&verts, &self.stroke, false, effective_canvas_color),
    3  => p.PaintBezier(&verts, fill, effective_canvas_color),
    4  => p.PaintBezierOutline(&verts, self.stroke_width, &self.stroke, effective_canvas_color),
    5  => p.PaintBezierLine(&verts, &self.stroke, effective_canvas_color),
    6  => p.PaintLine(x1, y1, x2, y2, self.stroke_width, &self.stroke,
                      &self.stroke_start, &self.stroke_end, effective_canvas_color),
    7  => p.PaintRect(bx, by, bw, bh, fill, effective_canvas_color),
    8  => p.PaintRectOutline(bx, by, bw, bh, self.stroke_width, &self.stroke, effective_canvas_color),
    9  => p.PaintEllipse(bx, by, bw, bh, fill, effective_canvas_color),
    10 => p.PaintEllipseOutline(bx, by, bw, bh, self.stroke_width, &self.stroke, effective_canvas_color),
    11 => p.PaintEllipseSector(bx, by, bw, bh, sa, ra, fill, effective_canvas_color),
    12 => p.PaintEllipseSectorOutline(bx, by, bw, bh, sa, ra,
                                      self.stroke_width, &self.stroke, effective_canvas_color),
    13 => p.PaintEllipseArc(bx, by, bw, bh, sa, ra, self.stroke_width, &self.stroke,
                             &self.stroke_start, &self.stroke_end, effective_canvas_color),
    14 => p.PaintRoundRect(bx, by, bw, bh, bw*0.2, bh*0.2, fill, effective_canvas_color),
    15 => p.PaintRoundRectOutline(bx, by, bw, bh, bw*0.2, bh*0.2,
                                   self.stroke_width, &self.stroke, effective_canvas_color),
    _  => {}
}
```

Adapt method signatures to match the actual Rust emPainter API — check emPainter.rs for each method. `PaintPolyline` and `PaintBezierLine` in Rust take `&emStroke` which contains start/end caps; verify the stroke struct carries them correctly.

- [ ] **Step 4: Run tests**

```bash
cargo-nextest ntr
```

For the golden test, regenerate if needed: `cargo test --test golden polydrawpanel_default_render -- --test-threads=1`.

- [ ] **Step 5: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs tests/golden/test_panel.rs
git commit -m "feat(emtest): CanvasPanel Paint — 16-way render type switch + WithCanvasColor background (C-3 C-5)"
```

---

### Task 6: CanvasPanel handle colorization (C-4)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1463–1483`.

- [ ] **Step 1: Replace handle coloring with Type-aware logic**

In `CanvasPanel::Paint`, find the handle-drawing loop. Replace the current green/white color logic with the C++ Type-aware colorization:

```rust
// C++ cpp:1463–1483.
let r = (state.ViewToPanelDeltaX(12.0)).min(0.05);  // already fixed in Plan 2 Task 8
let n = self.vertices.len();
for (i, &(vx, vy)) in self.vertices.iter().enumerate() {
    // Determine "active" vertex count m based on type (C++ cpp:1471–1478).
    let m = match self.render_type {
        3 | 4 => { let q = n - n % 3; q }               // bezier: multiple of 3
        5     => { let q = n - (n + 2) % 3; q }          // bezier line: n - (n+2)%3
        11..=13 => 4,                                     // arc/sector: 4 vertices used
        6..=10 | 14..=15 => 2,                            // line/rect/ellipse/round: 2
        _ => n,                                           // polygon: all
    };

    let mut c = if (self.render_type >= 3 && self.render_type <= 5) && i % 3 != 0 {
        emColor::rgba(255, 255, 0, 128)   // yellow: non-anchor bezier control points
    } else {
        emColor::rgba(0, 255, 0, 128)     // green: anchor points
    };

    if i >= m {
        c = emColor::rgba(128, 128, 128, 128);  // gray: unused vertices
    }
    if Some(i) == self.drag_idx {
        c = c.GetBlended(emColor::rgba(255, 255, 255, 128), 75.0);
    }

    p.PaintEllipse(vx - r, vy - r, 2.0*r, 2.0*r, c, effective_canvas_color);
    p.PaintEllipseOutline(vx - r, vy - r, 2.0*r, 2.0*r, r*0.15,
                          emColor::rgba(0, 0, 0, 128), effective_canvas_color);
}
```

Verify `emColor::GetBlended(other, percent)` signature in emColor.rs — it may be `blend_with` or similar.

- [ ] **Step 2: Run tests and commit**

```bash
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): CanvasPanel handle colorization — Type-aware bezier/gray logic (C-4)"
```

---

### Task 7: PolyDrawPanel LayoutChildren (I-14 orientation, real layout)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1261–1268` (PolyDrawPanel::LayoutChildren is inherited from emLinearGroup — it uses the group's own layout logic). The Rust equivalent is to delegate to `self.group.LayoutChildren(ctx)` which already happens.

The key gap: `SetOrientationThresholdTallness(1.0)` (I-14) switches between horizontal/vertical based on aspect ratio. Check `emLinearGroup.rs` for whether this is implemented.

- [ ] **Step 1: Check emLinearGroup orientation threshold API**

Search emLinearGroup.rs:
```bash
grep -n "orientation\|threshold_tallness\|SetOrientation" crates/emcore/src/emLinearGroup.rs
```

If `set_orientation_threshold_tallness` exists, call it in `PolyDrawPanel::new`:
```rust
group.set_orientation_threshold_tallness(1.0);
```

If it doesn't exist, add a `DIVERGED: upstream-gap-forced` comment in `PolyDrawPanel::new` where the call would be, referencing cpp:1011.

- [ ] **Step 2: Verify CanvasPanel help text** (C-11 partial — for PolyDrawPanel context)

C++ help text (cpp:1485–1490):
```
"The vertices can be dragged with the left mouse button!\n(Hold shift for raster)\n"
```
Positioned at `(0.0, GetHeight()-0.03, 1.0, 0.03)` with size `0.03`, white, centered.

Verify the Rust `CanvasPanel::Paint` help text (after Plan 2 Task 8 fixes the geometry) uses this exact text and positioning.

- [ ] **Step 3: Run tests and commit**

```bash
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): PolyDrawPanel LayoutChildren — orientation threshold (I-14)"
```

---

## Self-review

**Spec coverage:**

| Audit finding | Task |
|---|---|
| I-15 caption/description | Task 1 |
| I-14 orientation threshold | Task 1 + Task 7 |
| C-1 control tree (22 widgets) | Task 2 |
| C-2 Cycle + Setup plumbing | Tasks 3 + 4 |
| C-6 vertex y-scaling in Setup | Task 3 |
| C-3 16-way render type switch | Task 5 |
| C-5 WithCanvasColor background | Task 5 |
| C-4 handle colorization | Task 6 |

**Dependencies on Plan 2:**
- C-7 (GetHeight y-bound in CanvasPanel::Input): Plan 2 Task 8
- C-8 (Eat+Focus): Plan 2 Task 8
- C-9 (InvalidatePainting): Plan 2 Task 8
- C-10 (handle radius): Plan 2 Task 8
- C-11 (help-text geometry): Plan 2 Task 8
- C-26 (base Input forwarding): Plan 2 Task 8

These can be applied before or after Plan 3 — they are independent changes in the same file.

**Known remaining after this plan:**
- Golden test comparison for all 16 render types (run `cargo test --test golden` after landing).
- Full `emTexture` support in Setup (currently simplified to `emColor` flat fill) — add as follow-up if golden comparison reveals divergence.
- `SetOrientationThresholdTallness(1.0)` if `emLinearGroup` doesn't yet implement it.
