# Phase 3 — Widget Signal Model + emFpPlugin API — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Every widget with a C++ `GetXxxSignal` in emCore allocates a `SignalId` at construction via `ctx.create_signal()` and fires it inline from its Input handler, matching C++ observable behaviour. Callback fields migrate to `Box<dyn for<'a, 'b> FnMut(Args, &'b mut SchedCtx<'a>)>`. Clipboard relocates from `emContext` to `emGUIFramework::clipboard` chartered RefCell. `emFpPlugin::CreateFilePanel`/`TryCreateFilePanel`/`SearchPlugin` accept `&mut impl ConstructCtx`. `InputDispatchEngine` runs as a framework-owned top-priority scheduler engine that drains `pending_inputs`.

**Architecture:** Widgets hold a pre-allocated `SignalId` owned by the scheduler; the widget's `Input`/`Handle` method calls `ctx.fire(signal)` at the same beat C++ calls `Signal(GetXxxSignal())`. Callbacks remain as convenience. Construction happens under `&mut impl ConstructCtx` (either `InitCtx` at framework boot or `SchedCtx` during Cycle). Input dispatch synchronously enqueues events and cycles `InputDispatchEngine` on next tick.

**Companion:** spec §2 P1/P3, §3.1, §3.4 (clipboard), §3.5, §6 D6.1–D6.5, §4 D4.9/D4.10.

**Spec sections:** §3.5, §6, §4 D4.9, §4 D4.10 (if not fully landed by Phase 1).

**JSON entries closed:** E024, E025.

**Phase-specific invariants (C4):**
- **I3a.** Every widget in `emCoreConfigPanel.rs`, `emScalarField.rs`, `emFileSelectionBox.rs`, `emRadioButton.rs`, `emTextField.rs`, `emCheckButton.rs`, `emCheckBox.rs`, `emButton.rs`, `emColorField.rs` has a `*_signal: SignalId` field for every C++ `GetXxxSignal` method it exposes.
- **I3b.** `rg 'Box<dyn FnMut\(' crates/emcore/src/` returns only signatures with `SchedCtx` or `&mut impl ConstructCtx` parameters (grep with multiline to validate).
- **I3c.** Clipboard lives on `emGUIFramework`, not `emContext`. `rg 'clipboard' crates/emcore/src/emContext.rs` returns zero matches. `rg 'clipboard' crates/emcore/src/emGUIFramework.rs` matches the chartered field.
- **I3d.** `emFpPlugin::CreateFilePanel` and siblings take `&mut impl ConstructCtx` (or `&mut SchedCtx<'_>`); `rg 'fn CreateFilePanel|fn TryCreateFilePanel|fn SearchPlugin' crates/emcore/src/emFpPlugin.rs` shows the new signatures.
- **I3e.** `InputDispatchEngine` exists as a framework-owned engine registered at framework init at top priority.

**Entry-precondition.** Phase 2 Closeout COMPLETE.

---

## Bootstrap (per shared ritual)

Run B1–B12 with `<N>` = `3`. Verify Phase 2 closeout status.

---

## File Structure

**New files:**
- `crates/emcore/src/emInputDispatchEngine.rs` — framework-owned engine that drains `pending_inputs` each tick. Marker: `emInputDispatchEngine.rust_only` with rationale: `Rust-only: bridges winit's async-callback input model to emCore's cycle-driven dispatch.`
- `crates/emcore/src/emClipboard.rs` (if not already present) — trait definition moved out of `emContext.rs`. Existing trait object on `emGUIFramework::clipboard` supports both platform backends and test fakes.

**Heavy modifications:**
- `crates/emcore/src/emCheckButton.rs` — `check_signal: SignalId` field, allocated in `new<C: ConstructCtx>(ctx)`. Input handler fires signal before callback. Delete the DIVERGED blocks for `GetCheckSignal`/`CheckChanged`/`Clicked`.
- `crates/emcore/src/emCheckBox.rs` — analogous.
- `crates/emcore/src/emButton.rs` — `click_signal`.
- `crates/emcore/src/emRadioButton.rs` — `check_signal`. Radio group remains chartered `Rc<RefCell<RadioGroup>>` per spec §3.6(c).
- `crates/emcore/src/emTextField.rs` — `text_modified_signal`, `selection_modified_signal`.
- `crates/emcore/src/emColorField.rs` — `color_signal`.
- `crates/emcore/src/emScalarField.rs` — scalar change signal.
- `crates/emcore/src/emFileSelectionBox.rs` — `selection_signal`.
- `crates/emcore/src/emCoreConfigPanel.rs` — the largest cluster (~40 callback install sites). Each `on_xxx = Some(Box::new(move |args| ...))` migrates to `Box::new(move |args, sched| ...)` and captures fewer `Rc<RefCell<>>` clones (the config model becomes `Rc<emConfigModel<T>>` in Phase 4d; in Phase 3 leave the capture shape alone and only migrate the closure signature).
- `crates/emcore/src/emFpPlugin.rs` — `CreateFilePanel`, `CreateFilePanelWithStat`, `TryCreateFilePanel`, `SearchPlugin` take `&mut impl ConstructCtx`. Plugin trait + every implementor updated.
- `crates/emcore/src/emContext.rs` — delete clipboard field and accessors.
- `crates/emcore/src/emGUIFramework.rs` — add `clipboard: RefCell<Option<Box<dyn emClipboard>>>` field per §3.1; `SchedCtx::clipboard_mut` delegates here.
- `crates/emcore/src/emEngineCtx.rs` — add `clipboard_mut` accessor on `EngineCtx` and `SchedCtx` that returns a `RefMut<'_, Option<Box<dyn emClipboard>>>`; register the InputDispatchEngine.

**Test files:**
- For each migrated widget, add a signal-dispatch test verifying `ctx.IsSignaled(widget.check_signal)` fires on user-event dispatch.

---

## Task 1: `InputDispatchEngine`

**Files:**
- Create: `crates/emcore/src/emInputDispatchEngine.rs` + marker.
- Modify: `crates/emcore/src/lib.rs` — register module.
- Modify: `crates/emcore/src/emGUIFramework.rs` — register at framework init; replace the winit input callback with a simple enqueue.

- [ ] **Step 1: Write failing test.**
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn input_dispatch_drains_pending_inputs() {
        let mut framework = emGUIFramework::new_for_test();
        let wid = framework.register_test_window();
        framework.pending_inputs.push((wid, InputEvent::Key('a')));
        framework.scheduler.DoTimeSlice(&mut framework.windows, &framework.root_context);
        assert!(framework.pending_inputs.is_empty());
    }
}
```

- [ ] **Step 2: Run — FAIL** (no dispatch engine).

- [ ] **Step 3: Implement.**
```rust
pub struct InputDispatchEngine;

impl emEngine for InputDispatchEngine {
    fn Cycle(&mut self, ctx: &mut EngineCtx<'_>) {
        // Drain pending_inputs: we need access to the framework's
        // pending_inputs buffer via ctx. Exposed as ctx.take_pending_inputs().
        let events = ctx.take_pending_inputs();  // Vec<(WindowId, InputEvent)>
        for (wid, event) in events {
            ctx.with_view_mut(wid, |view, sched| view.dispatch_input(event, sched));
        }
    }
}
```

`EngineCtx` gains a `pending_inputs: &'a mut Vec<(WindowId, InputEvent)>` field and a `take_pending_inputs` method. `DoTimeSlice`'s signature extends to carry it. (This is a Phase-1 revisit — acceptable because Phase 1 landed the ctx scaffolding; Phase 3's `take_pending_inputs` addition is a narrow surface extension.)

- [ ] **Step 4: Register at framework init.**
```rust
let dispatch_id = framework.scheduler.register_engine(
    Box::new(InputDispatchEngine),
    Priority::Top,
);
framework.scheduler.wake_up(dispatch_id);   // always live, re-woken each tick if pending_inputs non-empty
```

- [ ] **Step 5: Winit callback enqueues.**
```rust
// was: immediate dispatch
// now: enqueue + schedule drain
framework.pending_inputs.push((wid, event));
framework.scheduler.wake_up(framework.dispatch_engine_id);
```

- [ ] **Step 6:** `cargo test -p emcore input_dispatch_drains_pending_inputs` — PASS.

- [ ] **Step 7: Commit.**
```bash
git add crates/emcore/src/emInputDispatchEngine.rs crates/emcore/src/emInputDispatchEngine.rust_only crates/emcore/src/lib.rs crates/emcore/src/emGUIFramework.rs crates/emcore/src/emEngineCtx.rs
git commit -m "phase-3: InputDispatchEngine drains pending_inputs at top priority"
```

---

## Task 2: Relocate clipboard from `emContext` to `emGUIFramework`

**Files:**
- Modify: `crates/emcore/src/emContext.rs` — delete clipboard field + accessors.
- Modify: `crates/emcore/src/emGUIFramework.rs` — add chartered clipboard field.
- Modify: `crates/emcore/src/emEngineCtx.rs` — add `clipboard_mut` accessor on `EngineCtx` and `SchedCtx`.
- Modify: callers that do `context.get_clipboard()` → ctx-based resolution.

- [ ] **Step 1:** Add field:
```rust
pub struct emGUIFramework {
    // ...
    /// Chartered §3.6(a): mutated from winit text-event callbacks that lack &mut framework reach.
    pub clipboard: RefCell<Option<Box<dyn emClipboard>>>,
    // ...
}
```

- [ ] **Step 2:** `EngineCtx` / `SchedCtx` grow:
```rust
pub fn clipboard_mut(&self) -> std::cell::RefMut<'_, Option<Box<dyn emClipboard>>> {
    self.framework_clipboard.borrow_mut()
}
```
(A `framework_clipboard: &'a RefCell<...>` field is added to both ctx structs.)

- [ ] **Step 3:** Delete `emContext::clipboard` field + `get_clipboard` accessor.

- [ ] **Step 4:** Migrate callers. `rg -n 'get_clipboard|\.clipboard' crates/` — each ctx-side access swaps to `ctx.clipboard_mut()`; winit-side access uses `framework.clipboard.borrow_mut()` directly.

- [ ] **Step 5:** Full check + nextest.
- [ ] **Step 6: Commit.**
```bash
git add -A
git commit -m "phase-3: relocate clipboard to emGUIFramework, chartered §3.6(a)"
```

---

## Task 3: Migrate widget callback signatures to `FnMut(Args, &mut SchedCtx<'_>)`

**Files:**
- Modify: `emCheckButton.rs`, `emCheckBox.rs`, `emButton.rs`, `emRadioButton.rs`, `emTextField.rs`, `emColorField.rs`, `emScalarField.rs`, `emFileSelectionBox.rs`, `emCoreConfigPanel.rs`.

- [ ] **Step 1: Define widget callback type in `emEngineCtx.rs`.**
```rust
pub type WidgetCallback<Args> = Box<dyn for<'a, 'b> FnMut(Args, &'b mut SchedCtx<'a>)>;
```

- [ ] **Step 2: For each widget file, change field types.**
```rust
// OLD
pub on_check: Option<Box<dyn FnMut(bool)>>,
// NEW
pub on_check: Option<WidgetCallback<bool>>,
```

- [ ] **Step 3: For each widget file, change install-site closure signatures.**

Before:
```rust
button.on_check = Some(Box::new(|checked: bool| {
    let state = state.borrow_mut();
    state.apply(checked);
}));
```
After:
```rust
button.on_check = Some(Box::new(|checked: bool, sched: &mut SchedCtx<'_>| {
    let state = state.borrow_mut();
    state.apply(checked);
    sched.fire(state.change_signal);
}));
```

- [ ] **Step 4: For each widget, update the handler that invokes the callback.** Fire signal *first*, then callback:
```rust
fn handle_check(&mut self, new_state: bool, ctx: &mut SchedCtx<'_>) {
    ctx.fire(self.check_signal);                   // signal first (C++ parity)
    if let Some(cb) = self.on_check.as_mut() {
        cb(new_state, ctx);                         // then callback (Rust ergonomic)
    }
}
```

- [ ] **Step 5: Run widget tests.** `cargo test -p emcore --test '*' 2>&1 | tail`. PASS.

- [ ] **Step 6: Commit.**
```bash
git add crates/emcore/src/emCheckButton.rs crates/emcore/src/emCheckBox.rs crates/emcore/src/emButton.rs crates/emcore/src/emRadioButton.rs crates/emcore/src/emTextField.rs crates/emcore/src/emColorField.rs crates/emcore/src/emScalarField.rs crates/emcore/src/emFileSelectionBox.rs crates/emcore/src/emEngineCtx.rs
git commit -m "phase-3: widget callbacks take SchedCtx; signals fire before callbacks"
```

---

## Task 4: Allocate `*_signal: SignalId` fields at widget construction

**Files:** same widget set as Task 3.

- [ ] **Step 1: For each widget, add a `*_signal: SignalId` field per C++ `GetXxxSignal`.**

- [ ] **Step 2: Constructor takes `&mut impl ConstructCtx`.**
```rust
impl emCheckButton {
    pub fn new<C: ConstructCtx>(ctx: &mut C, caption: &str) -> Self {
        Self {
            check_signal: ctx.create_signal(),
            on_check: None,
            caption: caption.to_string(),
            // ...
        }
    }
}
```

- [ ] **Step 3:** Callers (typically panel-construction code in `emCoreConfigPanel.rs`, test fixtures, etc.) pass `&mut InitCtx` at framework init or `&mut SchedCtx` during Cycle.

- [ ] **Step 4: Delete DIVERGED blocks for `GetCheckSignal`, `CheckChanged`, `Clicked` (checkbutton), and analogous blocks for checkbox/button/radio.**

- [ ] **Step 5: Add signal-dispatch regression tests.** For each widget:
```rust
#[test]
fn check_button_fires_signal_on_click() {
    let mut fixture = TestFixture::new();
    let button = emCheckButton::new(&mut fixture.init_ctx(), "test");
    fixture.dispatch_input(InputEvent::Click { button: MouseButton::Left, x: 5, y: 5 });
    assert!(fixture.is_signaled(button.check_signal));
}
```

- [ ] **Step 6:** Full check.
- [ ] **Step 7: Commit.**
```bash
git add -A
git commit -m "phase-3: widget *_signal fields allocated via ctx; signal-dispatch tests added"
```

---

## Task 5: Migrate `emFpPlugin` API to take `&mut impl ConstructCtx`

**Files:**
- Modify: `crates/emcore/src/emFpPlugin.rs`.
- Modify: every implementor of the plugin trait.
- Modify: `crates/eaglemode/tests/integration/dynamic_plugins.rs`, `crates/emcore/tests/plugin_invocation.rs`, `crates/eaglemode/tests/behavioral/fp_plugin.rs`.

- [ ] **Step 1: Update trait.**
```rust
pub trait emFpPlugin {
    fn CreateFilePanel(&self, ctx: &mut dyn ConstructCtxObj, /* args */) -> Box<dyn emFilePanel>;
    fn TryCreateFilePanel(&self, ctx: &mut dyn ConstructCtxObj, /* args */) -> Option<Box<dyn emFilePanel>>;
    fn CreateFilePanelWithStat(&self, ctx: &mut dyn ConstructCtxObj, /* args */) -> Box<dyn emFilePanel>;
    fn SearchPlugin(&self, ctx: &mut dyn ConstructCtxObj, /* args */) -> Option<PluginMatch>;
}
```

The `ConstructCtxObj` object-safe shim is added to `emEngineCtx.rs` because `ConstructCtx` has generic methods that are not directly dyn-compatible. Implementation: wrap `ConstructCtx` in an object-safe trait object with the same surface.

```rust
pub trait ConstructCtxObj {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(&mut self, e: Box<dyn emEngine>, p: Priority) -> EngineId;
    fn wake_up(&mut self, e: EngineId);
}
impl<T: ConstructCtx> ConstructCtxObj for T { /* delegate */ }
```

- [ ] **Step 2: Update every implementor.** `rg -n 'impl emFpPlugin for' crates/` — enumerate and migrate each.

- [ ] **Step 3: Update test fixtures.** Tests that call `plugin.CreateFilePanel(...)` now thread an `&mut InitCtx` or `&mut SchedCtx` through.

- [ ] **Step 4: Run plugin tests.**
```bash
cargo test -p eaglemode fp_plugin plugin_invocation dynamic_plugins 2>&1 | tail
```
Expected: PASS.

- [ ] **Step 5: Commit.**
```bash
git add -A
git commit -m "phase-3: emFpPlugin API takes &mut impl ConstructCtx"
```

---

## Task 6: emFileDialog polling → signal-based

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs` (locate with `rg -ln 'emFileDialog'`).

- [ ] **Step 1: Locate current polling pattern.** `rg -n 'fn poll|fn check' crates/emcore/src/emFileDialog.rs`.

- [ ] **Step 2: Replace with signal-based model.** Allocate a `result_signal: SignalId` at dialog construction; fire it from the OK/Cancel handlers; observers connect via `ctx.connect`. Polling methods deleted.

- [ ] **Step 3: Update tests.** `rg -n 'file_dialog' crates/`.

- [ ] **Step 4: Run.** Expected PASS.

- [ ] **Step 5: Commit.**
```bash
git add crates/emcore/src/emFileDialog.rs
git commit -m "phase-3: emFileDialog polling → signal-based (closes E024)"
```

---

## Task 7: Full gate and invariants

- [ ] **Step 1: Gate.**
```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo-nextest ntr && cargo test --test golden -- --test-threads=1
```

- [ ] **Step 2: Invariants.**
```bash
# I3a: enumerate widget files; each must have a *_signal field
for f in emCheckButton emCheckBox emButton emRadioButton emTextField emColorField emScalarField emFileSelectionBox; do
    rg -q '_signal:\s*SignalId' "crates/emcore/src/$f.rs" && echo "I3a $f PASS" || echo "I3a $f FAIL"
done

# I3c: clipboard not in emContext
rg 'clipboard' crates/emcore/src/emContext.rs && echo "I3c FAIL" || echo "I3c PASS"
rg 'clipboard' crates/emcore/src/emGUIFramework.rs | grep -q . && echo "I3c-gwi PASS" || echo "I3c-gwi FAIL"

# I3d: plugin API takes ctx
rg -q 'fn CreateFilePanel.*ConstructCtx' crates/emcore/src/emFpPlugin.rs && echo "I3d PASS" || echo "I3d FAIL"

# I3e: InputDispatchEngine exists
rg -q 'struct InputDispatchEngine' crates/emcore/src/emInputDispatchEngine.rs && echo "I3e PASS" || echo "I3e FAIL"
```

- [ ] **Step 3: Proceed to Closeout.**

---

## Closeout (per shared ritual)

Run C1–C11 with `<N>` = `3`. At C5 close E024 (Task 6) and E025 (Tasks 3+4).
