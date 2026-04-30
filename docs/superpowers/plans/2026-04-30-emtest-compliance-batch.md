# emTestPanel Spec-Compliance Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the spec-compliance gaps in `crates/emtest/src/emTestPanel.rs` identified in `docs/emtest-panel-audit.md` (groups A–E, H, I), excluding the PolyDrawPanel full port (Plan 3) and the AutoExpand restructure (Plan 1, prerequisite).

**Architecture:** Sequential compliance fixes ordered by dependency. The ConstructCtx `view_context()` extension (Task 2) is a framework change to `emcore` that gates several emTestPanel fixes; it lands first. TestPanel signal/Cycle restructure (Task 3) replaces the Cell-based polling intermediary pattern prohibited by CLAUDE.md. Paint fixes (Task 5) are mechanical parameter corrections against C++ ground truth. CanvasPanel interaction (Task 6) restores missing Focus/Eat/InvalidatePainting/GetHeight semantics. CustomListBox port (Task 7) adds the recursive item panel. Dialog fixes (Task 8) clean up construction ordering and un-diverge CbTopLev.

**Tech Stack:** Rust, `crates/emcore/src/emEngineCtx.rs`, `crates/emcore/src/emView.rs`, `crates/emtest/src/emTestPanel.rs`. C++ ground truth: `~/Projects/eaglemode-0.96.4/src/emTest/emTestPanel.cpp`.

**Prerequisite:** Plan 1 (2026-04-30-autoexpand-restructure.md) must be merged before this plan starts.

---

## Background for the implementer

Read `docs/emtest-panel-audit.md` before starting. Each task below references findings by their audit ID (C-n, I-n, M-n). C++ line references are in the form `cpp:NNN`.

Run tests with: `cargo-nextest ntr`
Check annotations with: `cargo xtask annotations`
C++ source: `~/Projects/eaglemode-0.96.4/src/emTest/emTestPanel.cpp`

---

### Task 1: Annotation fixes (M-1, M-3, M-4, M-5)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

These are annotation-only changes — no behavior change, no test needed beyond `cargo xtask annotations`.

- [ ] **Step 1: Add `RUST_ONLY:` annotation to `make_star`** (M-1)

Find `make_star` (around line 2433). Add a comment above the function:

```rust
// RUST_ONLY: language-forced utility. C++ inlines polygon vertices manually
// (emTestPanel.cpp:372–413); Rust extracts the computation to avoid repetition.
fn make_star(cx: f64, cy: f64, rx: f64, ry: f64, n: usize) -> Vec<(f64, f64)> {
```

- [ ] **Step 2: Add `DIVERGED:` annotation to `emGetInsResImage` call** (M-3)

Find the `emGetInsResImage("emTest", "icons/teddy.tga")` call in `TestPanel::new` (around line 486). Add a comment above it:

```rust
// DIVERGED: (dependency-forced) C++ emGetInsResImage(GetRootContext(), "icons", "teddy.tga")
// resolves to $EM_DIR/res/icons/teddy.tga (monolithic install). Rust cdylib resources
// live under res/emTest/; call uses ("emTest", "icons/teddy.tga") → $EM_DIR/res/emTest/icons/teddy.tga.
let test_image = emGetInsResImage("emTest", "icons/teddy.tga");
```

- [ ] **Step 3: Remove stale task reference comment** (M-4)

Find and remove the comment at around line 1227 that contains "flat placeholder; Task 11 restructures". The line is:
```rust
// PolyDraw — C++ name "PolyDraw" (flat placeholder; Task 11 restructures).
```
Replace with:
```rust
// PolyDraw — C++ name "PolyDraw" (emTestPanel.cpp:490).
```

- [ ] **Step 4: Audit existing DIVERGED blocks for missing category tags** (M-5)

Run:
```bash
cargo xtask annotations
```

If the annotation linter reports any DIVERGED blocks without a category tag (language-forced / dependency-forced / upstream-gap-forced / performance-forced), add the missing category. The linter output identifies exact line numbers. Fix each one it reports. Re-run until clean.

- [ ] **Step 5: Verify and commit**

```bash
cargo xtask annotations
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "chore(emtest): annotation fixes — RUST_ONLY make_star, DIVERGED emGetInsResImage, stale comment (M-1 M-3 M-4 M-5)"
```

---

### Task 2: Add `view_context()` to `ConstructCtx` (I-6 prerequisite)

**Files:**
- Modify: `crates/emcore/src/emEngineCtx.rs`
- Modify: `crates/emcore/src/emView.rs`

C++ rationale: `emView : public emContext` — a view IS a context. `TestPanel` accesses it via `GetView()`. In Rust, `emView` holds `Context: Rc<emContext>` (a child of the root context, created in `emView::new`). This task threads the view context through `HandleNotice` and `handle_notice_one` to `PanelCtx`, and adds `view_context()` to `ConstructCtx`. The emVarModel fix (I-6) uses this in Task 3.

- [ ] **Step 1: Add `view_context` field to `PanelCtx`**

In `emEngineCtx.rs`, find the `PanelCtx` struct (around line 491). Add a field alongside `root_context`:

```rust
pub root_context: Option<&'a Rc<emContext>>,
pub view_context: Option<&'a Rc<emContext>>,   // add this
```

In `PanelCtx::with_scheduler` (the constructor used inside `handle_notice_one`), set the new field:

```rust
pub(crate) fn with_scheduler(
    tree: &'a mut PanelTree,
    id: PanelId,
    pixel_tallness: f64,
    sched: &'a mut EngineScheduler,
) -> Self {
    Self {
        // ... existing fields ...
        root_context: None,
        view_context: None,   // add this
    }
}
```

Also update `PanelCtx::with_sched_reach` (the full constructor) to include `view_context: None` in its initializer.

- [ ] **Step 2: Add `view_context()` to `ConstructCtx` trait**

In `emEngineCtx.rs`, find the `ConstructCtx` trait (around line 144). Add one method:

```rust
pub trait ConstructCtx {
    // ... existing methods ...
    fn root_context(&self) -> &Rc<emContext>;
    fn view_context(&self) -> Option<&Rc<emContext>>;   // add this
}
```

Implement it for each `ConstructCtx` implementor in the same file:

```rust
// For EngineCtx:
fn view_context(&self) -> Option<&Rc<emContext>> {
    self.view_context
}

// For SchedCtx:
fn view_context(&self) -> Option<&Rc<emContext>> {
    self.view_context
}

// For PanelCtx (when used as ConstructCtx):
fn view_context(&self) -> Option<&Rc<emContext>> {
    self.view_context
}
```

Add `view_context: Option<&'a Rc<emContext>>` to `EngineCtx` and `SchedCtx` structs as well,
with `None` as the default in their constructors.

- [ ] **Step 3: Thread `view_context` through `HandleNotice` and `handle_notice_one`**

In `emView.rs`, find `HandleNotice` signature (around line 3888):

```rust
// OLD
pub fn HandleNotice(
    &mut self,
    tree: &mut PanelTree,
    sched: &mut crate::emScheduler::EngineScheduler,
    root_context: Option<&Rc<crate::emContext::emContext>>,
) -> bool {

// NEW
pub fn HandleNotice(
    &mut self,
    tree: &mut PanelTree,
    sched: &mut crate::emScheduler::EngineScheduler,
    root_context: Option<&Rc<crate::emContext::emContext>>,
    view_context: Option<&Rc<crate::emContext::emContext>>,
) -> bool {
```

Update the call to `handle_notice_one` inside `HandleNotice` to pass `view_context`:
```rust
self.handle_notice_one(tree, id, sched, root_context, view_context);
```

Find `handle_notice_one` signature and add the parameter:
```rust
fn handle_notice_one(
    &mut self,
    tree: &mut PanelTree,
    id: PanelId,
    sched: &mut crate::emScheduler::EngineScheduler,
    root_context: Option<&Rc<crate::emContext::emContext>>,
    view_context: Option<&Rc<crate::emContext::emContext>>,
) {
```

In `handle_notice_one`, wherever `ctx.root_context = root_context;` is set (four places), also set:
```rust
ctx.view_context = view_context;
```

- [ ] **Step 4: Update all `HandleNotice` call sites**

There are multiple call sites. Search for `HandleNotice(` across the codebase:

```bash
grep -rn "HandleNotice(" crates/
```

For each call site, add the `view_context` argument:
- In `emView::Update` (inside the view itself, around line 2641): pass `Some(&self.Context)`.
- In `emSubViewPanel.rs`: pass `Some(ectx.root_context)` — wait, that's root. Pass `None` here since sub-views don't have a parent view context in scope. Or: thread the sub-view's own Context. Use `Some(&self.sub_view.Context)` if accessible.
- In `tests/support/mod.rs`, `tests/support/pipeline.rs`, `tests/golden/*.rs`, `tests/unit/panel.rs`, `crates/emcore/src/emPanelTree.rs` (test usage), `crates/emcore/src/emView.rs` internal tests: pass `None`.

The `emView::Update` call is the production path; use `Some(&self.Context)` there.

- [ ] **Step 5: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green. The view_context is now available in PanelCtx inside AutoExpand, notice, and LayoutChildren calls.

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emEngineCtx.rs crates/emcore/src/emView.rs
git commit -m "feat(emcore): add view_context() to ConstructCtx, thread through HandleNotice (I-6 prereq)"
```

---

### Task 3: Fix TestPanel Cycle / signal wiring (C-13, C-14, I-3, I-4, absorbs I-5 partial)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:62–71` (Cycle), `emTestPanel.cpp:495` (AddWakeUpSignal in AutoExpand).

The current Rust TestPanel uses an `on_color` callback on `emColorField` that writes to `bg_shared: Cell<emColor>` — a polling intermediary. C++ uses `AddWakeUpSignal(BgColorField->GetColorSignal())` + `Cycle()` which checks `IsSignaled`. CLAUDE.md §"Polling intermediaries" prohibits the Cell pattern.

The fix: add a proper `Cycle` to TestPanel, store the color field's signal ID, and wire it via `AddWakeUpSignal` in AutoExpand. Remove the `on_color` callback.

- [ ] **Step 1: Add signal ID field to `TestPanel`**

In the `TestPanel` struct, add:
```rust
/// Color field signal — wired in AutoExpand, checked in Cycle.
/// None until AutoExpand fires.
bg_color_signal: Option<SignalId>,
```

In `TestPanel::new`, initialize:
```rust
bg_color_signal: None,
```

- [ ] **Step 2: Rewrite `TestPanel::AutoExpand` to wire signal instead of callback**

In `TestPanel::AutoExpand` (added in Plan 1), replace the BgColorField creation block:

```rust
// OLD (callback-based, polling intermediary)
let bg_for_cf = bg_shared.clone();
let mut cf = emColorField::new(ctx, emLook::new());
cf.SetCaption("Background Color");
cf.SetEditable(true);
cf.set_initial_alpha_enabled(true);
cf.set_initial_color(bg_shared.get());
cf.on_color = Some(Box::new(move |color, _sched: &mut SchedCtx<'_>| {
    bg_for_cf.set(color);
}));
ctx.create_child_with("BgColorField", Box::new(ColorFieldPanel { widget: cf }));

// NEW (signal-based, matches C++ AddWakeUpSignal pattern)
let mut cf = emColorField::new(ctx, emLook::new());
cf.SetCaption("Background Color");
cf.SetEditable(true);
cf.set_initial_alpha_enabled(true);
cf.set_initial_color(bg_shared.get());
let color_signal = cf.GetColorSignal();
// C++ emTestPanel.cpp:495: AddWakeUpSignal(BgColorField->GetColorSignal())
ctx.add_wake_up_signal(color_signal);
self.bg_color_signal = Some(color_signal);
ctx.create_child_with("BgColorField", Box::new(ColorFieldPanel { widget: cf }));
```

Check whether `PanelCtx` exposes `add_wake_up_signal`. If not, use `ctx.connect(color_signal, ctx.engine_id())` — search emEngineCtx.rs for how signal-to-engine connection is done in similar widget panels.

- [ ] **Step 3: Add `Cycle` to `TestPanel`**

In the `PanelBehavior` impl for `TestPanel`, add:

```rust
fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx) -> bool {
    // C++ emTestPanel.cpp:62–71.
    if let Some(sig) = self.bg_color_signal {
        if ectx.IsSignaled(sig) {
            // Read new color from BgColorField widget.
            if let Some(child) = ctx.find_child_by_name("BgColorField") {
                if let Some(behavior) = ctx.tree.get_behavior(child) {
                    if let Some(cf_panel) = behavior.downcast_ref::<ColorFieldPanel>() {
                        self.bg_shared.set(cf_panel.widget.GetColor());
                    }
                }
            }
            ctx.UpdateControlPanel();
            ctx.InvalidatePainting();
            ctx.InvalidateChildrenLayout();
        }
    }
    false
}
```

Note: `ctx.UpdateControlPanel()`, `ctx.InvalidatePainting()`, `ctx.InvalidateChildrenLayout()` — check PanelCtx API for the correct method names. They may be `ctx.tree.InvalidatePainting(ctx.id, ...)` etc. Search emEngineCtx.rs for how existing panels call these.

- [ ] **Step 4: Remove `bg_shared: BgShared` intermediary from `TestPanel`**

After the Cycle fix, `bg_shared` is no longer written by a callback. But it is still read by `Paint` and `Drop`. Keep the `bg_shared: Cell<emColor>` field but it is now only written by `Cycle` (synchronously). Update the comment in the `BgShared` type alias and struct field to reflect the new wiring.

Remove the `BgShared` type alias comment that references "callback" and update to say "written in Cycle via IsSignaled".

- [ ] **Step 5: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green. The `on_color` callback is gone; signal-based Cycle handles color updates.

- [ ] **Step 6: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): TestPanel — replace on_color callback with Cycle+IsSignaled signal wiring (C-13 C-14 I-3 I-4)"
```

---

### Task 4: Fix TestPanel Notice and Input (C-12, C-15)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `cpp:74–78` (Notice), `cpp:88–105` (Input).

- [ ] **Step 1: Add `notice()` to `TestPanel`** (C-12)

In the `PanelBehavior` impl for `TestPanel`, add:

```rust
fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, ctx: &mut PanelCtx) {
    // C++ emTestPanel.cpp:74–78.
    ctx.UpdateControlPanel();
    ctx.InvalidatePainting();
}
```

Verify `UpdateControlPanel` and `InvalidatePainting` exist on `PanelCtx`. If not, use the tree-level equivalents (search for how other panels in the file call these).

- [ ] **Step 2: Fix `TestPanel::Input`** (C-15)

Find `TestPanel::Input` (around line 1145). Current Rust:
- Builds a partial log string with event key/chars/repeat/variant but omits `STATE: pressed=`.
- Does not call `InvalidatePainting()`.
- Returns `false` without forwarding to base.

Replace the entire method body with:

```rust
fn Input(
    &mut self,
    event: &emInputEvent,
    state: &PanelState,
    input_state: &emInputState,
    ctx: &mut PanelCtx,
) -> bool {
    // C++ emTestPanel.cpp:88–105.
    let mut pressed = String::new();
    let mut k = 0;
    for i in 0..256u16 {
        if input_state.Get(i.into()) {
            if k > 0 { pressed.push(','); }
            pressed.push_str(&i.to_string());
            k += 1;
        }
    }
    let log = format!(
        "EVENT: key={} chars=\"{}\" repeat={} variant={} STATE: pressed={}  mouse={},{}",
        event.key as i32,
        event.GetChars(),
        event.GetRepeat() as i32,
        event.GetVariant() as i32,
        pressed,
        event.mouse_x,
        event.mouse_y,
    );
    if self.input_log.len() >= MAX_LOG_ENTRIES {
        self.input_log.remove(0);
    }
    self.input_log.push(log);
    ctx.InvalidatePainting();
    // C++ emPanel::Input forwarding — base handles cursor and bookkeeping.
    // Rust emPanel base Input is a no-op in the current port; call is preserved for fidelity.
    false
}
```

Check `emInputState::Get` signature in `emInputState.rs` — it may take `InputKey` enum rather than `u16`. Adapt the loop accordingly: iterate over all `InputKey` variants or use the numeric range if the type allows.

Check `event.GetChars()`, `event.GetRepeat()`, `event.GetVariant()`, `event.mouse_x`, `event.mouse_y` — verify field/method names match the Rust `emInputEvent` type in `emInput.rs`.

- [ ] **Step 3: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): TestPanel — add notice(), fix Input state log + InvalidatePainting (C-12 C-15)"
```

---

### Task 5: Fix emVarModel context scope and count param (I-6, I-19)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`
- Modify: `crates/emcore/src/emVarModel.rs`

**Prerequisite:** Task 2 (view_context() in ConstructCtx) must be merged.

C++ reference: `cpp:32–48` (constructor: `GetAndRemove(GetView(), ...)`) and `cpp:44–47` (destructor: `Set(GetView(), key, BgColor, 10)`).

- [ ] **Step 1: Add count parameter to `emVarModel::Set`**

In `crates/emcore/src/emVarModel.rs`, find `pub fn Set`. Add a `count: usize` parameter:

```rust
// OLD
pub fn Set(ctx: &Rc<emContext>, key: &str, value: emColor)

// NEW
pub fn Set(ctx: &Rc<emContext>, key: &str, value: emColor, count: usize)
```

Update the implementation to use `count` for eviction control (how many VarModel entries of this key can coexist). Check the current implementation to understand the data structure and add eviction logic matching C++ behavior. If the count parameter is not yet exercisable in tests (single-panel scenario), add it to the signature and pass it through; implement eviction in a follow-up if the data structure doesn't support it yet.

Update all callers of `emVarModel::Set` to pass the count. In `TestPanel::Drop`:
```rust
emVarModel::Set(&view_ctx_or_root, &self.identity_key, bg, 10);
```

- [ ] **Step 2: Switch TestPanel VarModel to view context**

In `TestPanel`, add field:
```rust
/// View-scoped context for VarModel storage — set in AutoExpand when view_context is available.
/// Matches C++ GetView() scope. Falls back to root_ctx if view context not available.
view_ctx: Option<Rc<emContext>>,
```

In `TestPanel::new`, initialize: `view_ctx: None`.

In `TestPanel::AutoExpand`, after the threshold-set block, capture the view context:
```rust
if let Some(vc) = ctx.view_context() {
    self.view_ctx = Some(vc.clone());
}
```

Replace the `emVarModel::GetAndRemove` call in `AutoExpand`:
```rust
// OLD
let bg = emVarModel::GetAndRemove(&self.root_ctx, &key, self.bg_shared.get());

// NEW
let ctx_for_var = self.view_ctx.as_ref().unwrap_or(&self.root_ctx);
let bg = emVarModel::GetAndRemove(ctx_for_var, &key, self.bg_shared.get());
self.identity_key = key;
```

In `TestPanel::Drop`:
```rust
// OLD
emVarModel::Set(&self.root_ctx, &self.identity_key, bg);

// NEW
let ctx_for_var = self.view_ctx.as_ref().unwrap_or(&self.root_ctx);
emVarModel::Set(ctx_for_var, &self.identity_key, bg, 10);
```

- [ ] **Step 3: Run tests**

```bash
cargo-nextest ntr
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/emcore/src/emVarModel.rs crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest/emcore): emVarModel view-scope storage + count param (I-6 I-19)"
```

---

### Task 6: Fix TestPanel structural gaps (I-1 threshold annotation, I-10 IsViewFocused)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

- [ ] **Step 1: Fix IsViewFocused mapping** (I-10)

Find the Paint method around line 1040:
```rust
// Probably uses state.window_focused somewhere for focused/path coloring.
```

Search for `window_focused` in TestPanel::Paint. C++ `cpp:148`: `IsViewFocused()` is per-view focus, not per-window. Check if `PanelState` has an `is_view_focused()` method (check `emPanel.rs`). If yes, use it instead of `state.window_focused`. If `PanelState` only has `window_focused`, this must be fixed in the PanelState construction first — check `build_panel_state` in emPanelTree.rs to see if view focus is separately tracked. Fix whichever layer needs it.

- [ ] **Step 2: Verify AE threshold annotation from Plan 1**

Confirm that `TestPanel::AutoExpand` (from Plan 1 Task 2) sets the threshold with a comment explaining why it's done in AutoExpand rather than new(). If the comment is missing, add:

```rust
// C++ emTestPanel constructor (cpp:39): SetAutoExpansionThreshold(900.0).
// Rust: set here because panels lack tree access during construction.
// First expansion fires at default 150.0 (view area >> 150 in any real view);
// subsequent shrink/re-expand decisions use 900.0.
ctx.tree.SetAutoExpansionThreshold(ctx.id, 900.0, ViewConditionType::Area, ctx.scheduler.as_deref_mut());
```

- [ ] **Step 3: Run tests and commit**

```bash
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): IsViewFocused mapping + AE threshold comment (I-10)"
```

---

### Task 7: Fix TestPanel Paint (C-16 through C-22, C-25)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ paint section: `emTestPanel.cpp:405–478`. Read this section fully before starting.

These are pixel-level corrections; each is a direct match to C++ values. All changes are in `paint_primitives` or the paint section of `TestPanel::Paint`.

- [ ] **Step 1: Fix linear gradient params** (C-18)

Find `paint_linear_gradient` call (around line 964). C++ (cpp:415–419):
```cpp
painter.PaintRect(0.2, 0.94, 0.02, 0.01,
    emLinearGradientTexture(0.207, 0.944, 0x00000080, 0.213, 0.946, 0x80808080));
```

The Rust `PaintRect` with `emLinearGradientTexture` needs separate rect coords and gradient endpoint coords. Replace the collapsed `paint_linear_gradient` call with a `PaintRect` call that takes explicit gradient endpoints. Check `emPainter.rs` for the `PaintRect` overload that accepts `emLinearGradientTexture`. If a wrapper exists, pass:
- rect: `(0.2, 0.94, 0.02, 0.01)`
- gradient from: `(0.207, 0.944, 0x00000080)`
- gradient to: `(0.213, 0.946, 0x80808080)`

- [ ] **Step 2: Fix radial gradient params** (C-19)

C++ (cpp:420–423):
```cpp
painter.PaintRect(0.221, 0.94, 0.008, 0.01,
    emRadialGradientTexture(0.223, 0.941, 0.004, 0.008, 0xFF8800FF, 0x005500FF));
```

Replace the `paint_radial_gradient` call with a `PaintRect` that takes `emRadialGradientTexture`:
- rect: `(0.221, 0.94, 0.008, 0.01)`
- origin: `(0.223, 0.941)`, radii: `(0.004, 0.008)`
- inner color: `0xFF8800FF`, outer color: `0x005500FF`

- [ ] **Step 3: Fix gradient ellipse** (C-22)

C++ (cpp:425–428):
```cpp
painter.PaintEllipse(0.23, 0.94, 0.02, 0.01,
    emRadialGradientTexture(0.23, 0.94, 0.02, 0.01, 0, 0x00cc88FF));
```

Find the solid `PaintEllipse` call with `emColor::rgba(0, 0xCC, 0x88, 0xFF)` (around line 984). Replace with `PaintEllipse` using radial gradient texture:
- ellipse bounds: `(0.23, 0.94, 0.02, 0.01)`
- gradient origin: `(0.23, 0.94)`, radii: `(0.02, 0.01)`
- inner color: `emColor(0)` (transparent black), outer color: `emColor(0x00cc88FF)`

- [ ] **Step 4: Fix image tile formula** (C-20)

C++ (cpp:430–435):
```cpp
painter.PaintRect(0.26, 0.94, 0.02, 0.01,
    emImageTexture(0.26, 0.94, 0.001,
        0.001 * TestImage.GetHeight() / TestImage.GetWidth(), TestImage));
```

Find `paint_image_scaled` call (around line 994). Replace with a `PaintRect` + `emImageTexture` where texture width is `0.001` (not the rect width `0.02`) and texture height is `0.001 * image.GetHeight() / image.GetWidth()`. Check `emPainter.rs` for the correct call signature.

- [ ] **Step 5: Add emImageColoredTexture polygon** (C-16)

C++ (cpp:441–451): `PaintRect(0.2625, 0.942, 0.02, 0.01, emImageColoredTexture(1.0005, 0.942, 0.001, 0.001*ratio, TestImage, 0x00FFFFFF, 0xFF0000FF))`.

After the image-tile rect, add:
```rust
// C++ cpp:441–451 — emImageColoredTexture rect.
let ratio = self.test_image.GetHeight() as f64 / self.test_image.GetWidth() as f64;
p.PaintRect(
    0.2625, 0.942, 0.02, 0.01,
    &emImageColoredTexture::new(
        1.0005, 0.942, 0.001, 0.001 * ratio,
        &self.test_image,
        emColor(0x00FFFFFF),
        emColor(0xFF0000FF),
    ),
    bg,
);
```

Check `emImageColoredTexture` exists in `emPainter.rs` or `emTexture.rs`. If not, file an audit note and skip — it may require a new texture type.

- [ ] **Step 6: Add EXTEND_TILED / EXTEND_EDGE / EXTEND_ZERO rects** (C-17)

C++ (cpp:453–478): three `PaintRect` calls at `(0.275, 0.907, 0.002, 0.002)`, `(0.275, 0.910, 0.002, 0.002)`, `(0.275, 0.913, 0.002, 0.002)` using `emImageTexture` with `EXTEND_TILED`, `EXTEND_EDGE`, and `EXTEND_ZERO` flags respectively. Each uses the same sub-image crop `(50, 10, 110, 110)` with alpha 255.

After the colored-texture rect, add:
```rust
// C++ cpp:453–478 — EXTEND_TILED / EXTEND_EDGE / EXTEND_ZERO demo rects.
for (y_off, extend) in [
    (0.907, ImageExtension::Repeat),   // EXTEND_TILED
    (0.910, ImageExtension::Edge),      // EXTEND_EDGE
    (0.913, ImageExtension::Zero),      // EXTEND_ZERO (transparent outside)
] {
    p.PaintRect(
        0.275, y_off, 0.002, 0.002,
        &emImageTexture::new_cropped(
            0.2755, y_off + 0.0005, 0.001, 0.001,
            &self.test_image,
            50, 10, 110, 110,
            255,
            extend,
        ),
        bg,
    );
}
```

Check the exact `emImageTexture` constructor name in Rust. Use `ImageExtension` variants or whatever enum the Rust port uses for extend modes.

- [ ] **Step 7: Fix text alignment** (C-21)

C++ (cpp:134–141): `PaintTextBoxed(... EM_ALIGN_CENTER ... 0.2)` where `EM_ALIGN_CENTER` is the inner horizontal alignment and `0.2` is `formatTallness`.

Find `PaintTextBoxed` for the "Test Panel" caption in `TestPanel::Paint` (around line 1062). Fix:
- Inner horizontal alignment: change from `Left` to `Center` (or `AlignCenter` — check `emPainter.rs` enum name).
- `formatTallness`: change from `0.5` to `0.2`.

- [ ] **Step 8: Add DIVERGED annotation for C-25 (sub-painter)**

Find the `push_state / SetClipping / pop_state` block (around line 575). Add comment:

```rust
// DIVERGED: (language-forced) C++ creates a sub-painter with restricted origin/scale
// (emTestPanel.cpp:225–231) — both clips AND re-maps coordinate origin. Rust cannot create
// a sub-painter (would require a second exclusive &mut emImage borrow; borrow checker
// forbids it). push_state/SetClipping/pop_state clips correctly but does not shift origin.
// Add to golden verification to confirm pixel equivalence.
p.push_state();
```

- [ ] **Step 9: Run tests and commit**

```bash
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): TestPanel paint — gradient params, image tile, extend modes, text alignment, DIVERGED sub-painter (C-16 C-17 C-18 C-19 C-20 C-21 C-22 C-25)"
```

---

### Task 8: Fix CanvasPanel interaction (C-7, C-8, C-9, C-10, C-11, C-26)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:1306–1495` (CanvasPanel::Input and Paint). Read the full Input method before starting.

- [ ] **Step 1: Fix unconditional Eat and Focus on left-press** (C-8)

Find `CanvasPanel::Input` (around line 2285). In the left-press branch, before the vertex search:

C++ (cpp:1315–1317):
```cpp
event.Eat(); Focus();
```
Both happen unconditionally before any vertex hit-test. In Rust, find where the left-press is detected and add these calls immediately after the press check, before the vertex loop:

```rust
if event.variant == InputVariant::Press && event.key == InputKey::MouseLeft {
    event.Eat();          // C++ cpp:1315: event.Eat() unconditional on left-press
    ctx.Focus();          // C++ cpp:1317: Focus() unconditional on left-press
    // ... vertex hit-test follows
}
```

Check `ctx.Focus()` and `event.Eat()` — verify the Rust method names in `PanelCtx` and `emInputEvent`.

- [ ] **Step 2: Fix y-bound to use GetHeight() in three places** (C-7)

C++ uses `GetHeight()` for the y dimension. In Rust's CanvasPanel, the panel height is not 1.0 in general — it's `state.layout_rect.h` or obtained from the painter height. Check how `CanvasPanel::Input` receives panel dimensions. The state has `layout_rect: Rect` with `.h` as the panel height.

Replace the three occurrences:
- Drag y-clamp: `.clamp(0.0, 1.0)` → `.clamp(0.0, panel_h)` where `panel_h = state.layout_rect.h`
- ShowHandles check: `(0.0..1.0).contains(&my)` → `my >= 0.0 && my < panel_h`
- Initial vertex y in Setup (if called from PolyDrawPanel — relevant to Plan 3, mark with TODO)

In `CanvasPanel::Input`, add `let panel_h = state.layout_rect.h;` near the top, then use it.

- [ ] **Step 3: Add four InvalidatePainting calls** (C-9)

C++ (cpp:1332, 1338, 1344–1347, 1357–1359). Find the drag logic in `CanvasPanel::Input` and add:

```rust
// (a) When drag starts (vertex hit):
if best_i.is_some() {
    self.drag_idx = best_i;
    ctx.InvalidatePainting();    // C++ cpp:1332
}

// (b) When drag stops (button release):
if event.variant == InputVariant::Release && event.key == InputKey::MouseLeft {
    if self.drag_idx.is_some() {
        self.drag_idx = None;
        ctx.InvalidatePainting(); // C++ cpp:1338
    }
}

// (c) When vertex position changes during drag:
if let Some(idx) = self.drag_idx {
    let x = /* computed new x */;
    let y = /* computed new y */;
    if self.vertices[idx].0 != x || self.vertices[idx].1 != y {  // C++ cpp:1344 change-guard
        self.vertices[idx] = (x, y);
        ctx.InvalidatePainting(); // C++ cpp:1347
    }
    // Remove unconditional overwrite that was here before
}

// (d) When ShowHandles changes:
let new_show = /* compute from cursor proximity */;
if new_show != self.show_handles {
    self.show_handles = new_show;
    ctx.InvalidatePainting(); // C++ cpp:1359
}
```

Verify `ctx.InvalidatePainting()` is the correct Rust method name.

- [ ] **Step 4: Fix handle radius formula** (C-10)

C++ (cpp:1464): `r = emMin(ViewToPanelDeltaX(12.0), 0.05)`.

Find the handle radius calculation in `CanvasPanel::Paint` (around line 2391). Replace:
```rust
// OLD: r = (0.05).min(12.0 / w.max(1.0))
// NEW: use ViewToPanelDeltaX equivalent
let r = (ctx_or_state.ViewToPanelDeltaX(12.0)).min(0.05);
```

Check how `PanelState` or `PanelCtx` exposes `ViewToPanelDeltaX`. It converts 12 view pixels to panel-space units. If not directly available, compute: `12.0 / (state.viewed_rect.w / w)` where `w` is the panel width. Check emPanel.rs for the C++ equivalent formula.

- [ ] **Step 5: Fix help-text geometry** (C-11)

C++ (cpp:1485–1490):
```cpp
painter.PaintTextBoxed(0.0, GetHeight()-0.03, 1.0, 0.03, ..., 0.03);
```
All coords are in panel space; `GetHeight()` is the panel height; `0.03` is fixed.

Find the help-text `PaintTextBoxed` in `CanvasPanel::Paint` (around line 2412). The Rust version multiplies by `h`. Replace with:
```rust
// C++ cpp:1485–1490: coords in panel space, GetHeight() is panel h, 0.03 is fixed.
p.PaintTextBoxed(
    0.0, h - 0.03, 1.0, 0.03,
    /* alignment, text, format_tallness: */ ...,
    0.03,
    // ...
);
```

Remove any `* h` multiplications that were incorrectly scaling these fixed coords.

- [ ] **Step 6: Add base Input forwarding** (C-26)

C++ (cpp:1361): `emPanel::Input(event, state, mx, my)` at the end of `CanvasPanel::Input`.

In Rust, the base `PanelBehavior::Input` is a no-op in the current port. Add at the end of `CanvasPanel::Input` (before the final `false` return or after the main logic):

```rust
// C++ cpp:1361: emPanel::Input(event, state, mx, my) — base handles cursor + bookkeeping.
// Rust base Input is currently a no-op; call preserved for fidelity.
// If base Input semantics are added later, this ensures correct dispatch.
let _ = (event, state, input_state); // used above; base call is no-op
```

This is a structural preservation marker. If the Rust base has actual behavior, call it here.

- [ ] **Step 7: Run tests**

```bash
cargo-nextest ntr
```

- [ ] **Step 8: Commit**

```bash
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): CanvasPanel — Eat+Focus unconditional, GetHeight y-bound, InvalidatePainting x4, handle radius, help-text, base forwarding (C-7 C-8 C-9 C-10 C-11 C-26)"
```

---

### Task 9: CustomListBox completeness (C-23, C-24, I-11, I-12, I-13)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

C++ reference: `emTestPanel.cpp:932–994` (CustomItemPanel + CustomListBox). Read fully before starting.

- [ ] **Step 1: Add `AutoExpand` to `CustomItemBehavior`** (C-23)

C++ (cpp:941–956): `CustomItemPanel::AutoExpand` creates:
- `emLabel(this,"t","This is a custom list\nbox item panel...")` with look set to listbox's look
- `CustomListBox(this,"l","Child List Box")` with multi-selection, items 1–7, item 0 selected

Find `CustomItemBehavior` in `emTestPanel.rs`. Add `AutoExpand`:

```rust
fn AutoExpand(&mut self, ctx: &mut PanelCtx) {
    // C++ CustomItemPanel::AutoExpand (cpp:941–956): recursive label + list box.
    // Create label
    let mut label = emLabel::new();
    label.SetCaption("This is a custom list\nbox item panel (it is\nrecursive...)");
    label.SetLook(self.look.clone());
    ctx.create_child_with("t", Box::new(LabelPanel { widget: label }));

    // Create recursive CustomListBox
    let mut lb = emListBox::new(self.look.clone());
    lb.SetCaption("Child List Box");
    lb.SetLook(self.look.clone());
    lb.SetSelectionType(SelectionType::Multi);
    for i in 1..=7 {
        lb.AddItem(&format!("{i}"), &format!("Item {i}"));
    }
    lb.SetSelectedIndex(0);
    ctx.create_child_with("l", Box::new(ListBoxPanel { widget: lb }));
}
```

Verify `SelectionType::Multi` is the correct enum variant — check `emListBox.rs` for selection type names.

- [ ] **Step 2: Add `Input` to `CustomItemBehavior`** (C-24)

C++ (cpp:932–938):
```cpp
void CustomItemPanel::Input(...) {
    ProcessItemInput(this, event, state);
    emLinearGroup::Input(event, state, mx, my);
}
```

Add `Input` to `CustomItemBehavior`:
```rust
fn Input(
    &mut self,
    event: &emInputEvent,
    state: &PanelState,
    input_state: &emInputState,
    ctx: &mut PanelCtx,
) -> bool {
    // C++ cpp:932–938: ProcessItemInput then forward to emLinearGroup base.
    self.process_item_input(event, state, ctx);
    self.group.Input(event, state, input_state, ctx)
}
```

Check the Rust equivalent of `ProcessItemInput` — it's likely a method on `emListBox::ItemPanelInterface` or a trait. Search for `process_item_input` or `ProcessItemInput` in `emListBox.rs`.

- [ ] **Step 3: Add `ItemTextChanged` override** (I-11)

C++ (cpp:959–962): `ItemTextChanged` overrides `SetCaption(GetItemText())`.

Add to `CustomItemBehavior`:
```rust
fn item_text_changed(&mut self, new_text: &str) {
    // C++ cpp:959–962: SetCaption(GetItemText()).
    self.group.SetCaption(new_text);
}
```

Check the Rust trait method name for item-text-change notification in `emListBox.rs` / `emPanel.rs`.

- [ ] **Step 4: Fix CustomListBox layout properties** (I-12)

C++ (cpp:992–994):
```cpp
SetChildTallness(0.4);
SetAlignment(EM_ALIGN_TOP_LEFT);
SetStrictRaster();
```

Find where `CustomListBox` (or its Rust equivalent) is created/configured. Add these calls to the constructor or init method:
```rust
lb.SetChildTallness(0.4);
lb.SetAlignment(Alignment::TopLeft);  // check enum name in emListBox.rs
lb.SetStrictRaster(true);             // check method name
```

- [ ] **Step 5: Fix look capture in `CustomItemBehavior`** (I-13)

C++ (cpp:980): `SetLook(GetListBox().GetLook())` — the listbox's LIVE look.

Find where `CustomItemBehavior` is constructed and check if `lb7_look` is captured at factory time. The fix is to pass the listbox's look at item-creation time (when the listbox's look is already set) rather than at factory creation. Verify the construction sequence to ensure the look is valid when items are created.

- [ ] **Step 6: Run tests and commit**

```bash
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): CustomListBox — recursive AutoExpand, Input forwarding, ItemTextChanged, layout props, look capture (C-23 C-24 I-11 I-12 I-13)"
```

---

### Task 10: Dialog and structural fixes (I-7, I-9)

**Files:**
- Modify: `crates/emtest/src/emTestPanel.rs`

**Prerequisite:** Task 2 (view_context()) for I-9.

- [ ] **Step 1: Fix dialog construction order** (I-7)

C++ (cpp:799–803): `AddNegativeButton("Close")`, `EnableAutoDeletion()`, `SetRootTitle("Test Dialog")`, then content is added.

Find `emDialog::new` call in `TkTestPanel` (around line 2158). Current Rust inserts `set_view_window_flags` between the title and content. Move `set_view_window_flags` to before `AddNegativeButton` so it fires at construction, matching C++ flag-before-content ordering:

```rust
let mut dlg = emDialog::new(ctx, "Test Dialog", emLook::new());
dlg.set_view_window_flags(/* flags from checkboxes */);  // move here — before buttons
dlg.AddNegativeButton("Close");
dlg.EnableAutoDeletion(ctx, true);
dlg.set_root_title("Test Dialog");
dlg.set_content_behavior(Box::new(/* content panel */));
dlg.show(ctx);
```

- [ ] **Step 2: Un-diverge CbTopLev** (I-9)

C++ (cpp:790): if `CbTopLev` checked, dialog parented to `GetRootContext()`; else `GetView()`.

Find the `DIVERGED` block for `cb_toplev` (around line 2150). With `view_context()` now available in `ConstructCtx`, the two-path construction is possible:

```rust
// C++ cpp:789–799: ctx = CbTopLev->IsChecked() ? &GetRootContext() : &GetView()
let use_toplev = self.cb_toplev.get();
// Both paths use the same emDialog::new call; the distinction is which emContext
// is used for the window's parent. Thread view_context vs root_context through
// the dialog constructor — check emDialog::new to see how context affects parenting.
```

Remove the DIVERGED block comment and implement the two-path parenting. If `emDialog::new` always uses `root_context()` internally and there's no way to pass the view context as the parent window, update the DIVERGED block to accurately state this as a remaining dependency-forced divergence (do not remove it). Only remove the DIVERGED block if the two-path behavior is actually implemented.

- [ ] **Step 3: Run tests and commit**

```bash
cargo-nextest ntr
git add crates/emtest/src/emTestPanel.rs
git commit -m "fix(emtest): dialog construction order, un-diverge CbTopLev after view_context() (I-7 I-9)"
```

---

## Self-review

**Spec coverage:**

| Audit finding | Task |
|---|---|
| M-1 make_star RUST_ONLY | Task 1 |
| M-3 emGetInsResImage DIVERGED | Task 1 |
| M-4 stale comment | Task 1 |
| M-5 DIVERGED category audit | Task 1 |
| I-6 emVarModel view_context | Tasks 2 + 5 |
| I-19 emVarModel::Set count param | Task 5 |
| C-13 TestPanel Cycle | Task 3 |
| C-14 AddWakeUpSignal | Task 3 |
| I-3 bg_shared Cell intermediary | Task 3 |
| I-4 deferred signals_connected | Task 3 |
| C-12 TestPanel Notice | Task 4 |
| C-15 TestPanel Input | Task 4 |
| I-10 IsViewFocused | Task 6 |
| C-16 emImageColoredTexture | Task 7 |
| C-17 EXTEND modes | Task 7 |
| C-18 linear gradient params | Task 7 |
| C-19 radial gradient params | Task 7 |
| C-20 image tile formula | Task 7 |
| C-21 text alignment | Task 7 |
| C-22 gradient ellipse | Task 7 |
| C-25 sub-painter DIVERGED annotation | Task 7 |
| C-7 GetHeight y-bound | Task 8 |
| C-8 Eat+Focus unconditional | Task 8 |
| C-9 InvalidatePainting x4 | Task 8 |
| C-10 handle radius | Task 8 |
| C-11 help-text geometry | Task 8 |
| C-26 base Input forwarding | Task 8 |
| C-23 CustomItemPanel AutoExpand | Task 9 |
| C-24 CustomItemPanel Input | Task 9 |
| I-11 ItemTextChanged | Task 9 |
| I-12 CustomListBox layout | Task 9 |
| I-13 look capture | Task 9 |
| I-7 dialog construction order | Task 10 |
| I-9 CbTopLev un-diverge | Task 10 |

**Out of scope (covered elsewhere):**
- I-1 AE threshold on root: Plan 1 Task 6
- I-2 MAX_DEPTH removal: Plan 1 Task 2
- I-5 sf5↔sf6 Cell: partially addressed by Task 3 (Cycle restructure); the sf5/sf6 intermediary in `ScalarFieldWithDynamicMax` is a separate signal-pipe pattern — see I-5 in the audit. If not resolved by the Cycle restructure, add a follow-up task targeting `ScalarFieldWithDynamicMax::Cycle` directly.
- M-2, M-7: golden verification items — run `cargo test --test golden` after paint fixes land; compare with C++ baselines.
- C-1–C-6, I-14, I-15: PolyDrawPanel full port (Plan 3).
- I-8, I-16, I-17, I-18: closed as non-bugs (audit decisions, 2026-04-30).
