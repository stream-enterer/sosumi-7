# D4 — emRecListener Self-Update: Remove generation Counter — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `generation: Rc<Cell<u64>>` D-009 polling intermediary in `emCoreConfigPanel` with signal-driven in-place updates: scalar fields self-update via a per-field subscribe, checkbox groups subscribe to the config aggregate.

**Architecture:** Define a new `FactorFieldPanel` struct (local to `emCoreConfigPanel.rs`) that wraps `emScalarField` and subscribes to its specific config field's value signal in its first-Cycle init, calling `set_value_silent` on signal. Checkbox groups (`MouseMiscGroup`, `CpuGroup`) subscribe to the config aggregate signal and call `update_output()` in Cycle. The `generation` counter is then removed from all 11 structs that hold it.

**Tech Stack:** Rust, `emcore` crate (`emCoreConfigPanel.rs`, `emScalarField.rs`, `emCheckBox.rs`, `emRecNodeConfigModel.rs`).

**Spec:** `docs/superpowers/specs/2026-04-29-D4-rec-listener-self-update-design.md`

---

## File Map

| File | Role |
|---|---|
| `crates/emcore/src/emCoreConfigPanel.rs` | Main: new `FactorFieldPanel` struct, `make_factor_field` update, group subscribe/update_output, generation removal |
| `crates/emcore/src/emScalarField.rs` | Add `set_value_silent` |
| `crates/emcore/src/emCheckBox.rs` | Add `set_checked_silent` |
| `crates/emcore/src/emRecNodeConfigModel.rs` | Add `GetChangeSignal` |
| `crates/emcore/tests/rec_listener_b_d4.rs` | New test file — 3 tests |

---

## Task 1: Prerequisites — `set_value_silent`, `set_checked_silent`, `GetChangeSignal`

**Files:**
- Modify: `crates/emcore/src/emScalarField.rs`
- Modify: `crates/emcore/src/emCheckBox.rs`
- Modify: `crates/emcore/src/emRecNodeConfigModel.rs`

- [ ] **Step 1.1: Add `set_value_silent` to `emScalarField`**

In `crates/emcore/src/emScalarField.rs`, add after `set_initial_value`:

```rust
/// Set value without firing `value_signal` or calling `on_value`. Used by
/// `FactorFieldPanel::update_output` to sync display from config without
/// triggering the feedback loop that `SetValue` would cause.
pub fn set_value_silent(&mut self, val: f64) {
    let clamped = val.clamp(self.min, self.max);
    if (clamped - self.value).abs() > f64::EPSILON {
        self.value = clamped;
    }
}
```

- [ ] **Step 1.2: Add `set_checked_silent` to `emCheckBox`**

In `crates/emcore/src/emCheckBox.rs`, add after `set_checked_for_test`:

```rust
/// Set checked state without firing `check_signal`. Used by `update_output`
/// in panel groups to sync checkbox display from config after Reset without
/// causing a feedback loop or requiring `PanelCtx`.
pub fn set_checked_silent(&mut self, checked: bool) {
    self.checked = checked;
}
```

(If the field is named differently, match the existing field name from `IsChecked`'s body.)

- [ ] **Step 1.3: Add `GetChangeSignal` to `emRecNodeConfigModel`**

In `crates/emcore/src/emRecNodeConfigModel.rs`, add after `GetRec`:

```rust
/// Returns the aggregate signal that fires whenever any field in the
/// underlying record mutates. Per D-008: single combined accessor, no
/// separate Ensure* form.
///
/// C++ analogue: none — C++ `emRecListener` splices into the UpperNode chain
/// directly. Rust reifies the observable channel per D-008.
pub fn GetChangeSignal(&self) -> SignalId {
    self.value.listened_signal()
}
```

- [ ] **Step 1.4: Run `cargo check --workspace`**

Expected: compiles with no errors.

- [ ] **Step 1.5: Commit**

```bash
git add crates/emcore/src/emScalarField.rs \
        crates/emcore/src/emCheckBox.rs \
        crates/emcore/src/emRecNodeConfigModel.rs
git commit -m "feat(D4): add set_value_silent, set_checked_silent, GetChangeSignal"
```

---

## Task 2: Define `FactorFieldPanel` + update `make_factor_field`

**Files:**
- Modify: `crates/emcore/src/emCoreConfigPanel.rs`

- [ ] **Step 2.1: Add `EngineCtx` to imports**

In `crates/emcore/src/emCoreConfigPanel.rs`, change:
```rust
use crate::emEngineCtx::PanelCtx;
```
to:
```rust
use crate::emEngineCtx::{EngineCtx, PanelCtx};
```

Also add the cursor import (needed by `FactorFieldPanel::GetCursor`):
```rust
use crate::emCursor::emCursor;
```

(Add alongside the existing imports; skip if `emCursor` is already imported.)

- [ ] **Step 2.2: Define `FactorFieldPanel` struct**

In `crates/emcore/src/emCoreConfigPanel.rs`, replace the factory helper comment block just before `make_factor_field` with:

```rust
// ---------------------------------------------------------------------------
// Factory helper: build a FactorFieldPanel with factor-field config.
// Replaces the previous ScalarFieldPanel { scalar_field } literal construction
// for config-bound scalar fields in emCoreConfigPanel.
// ---------------------------------------------------------------------------

/// Config-bound scalar field panel. Equivalent to C++ `FactorField`:
/// `emScalarField + emRecListener`. Subscribes to `config_sig` in its first
/// Cycle and calls `set_value_silent` on signal — display-only update that
/// does not trigger the `on_value` feedback loop.
struct FactorFieldPanel {
    scalar_field: emScalarField,
    /// Value signal of the specific `emDoubleRec`/`emIntRec` this panel mirrors.
    /// `SignalId::null()` = no self-update wiring.
    config_sig: SignalId,
    /// Closure returning the current config value in slider units.
    get_config_val: Option<Box<dyn Fn() -> f64>>,
    subscribed_to_config: bool,
}

impl PanelBehavior for FactorFieldPanel {
    fn Paint(
        &mut self,
        painter: &mut emPainter,
        canvas_color: emColor,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale =
            state.viewed_rect.w * state.viewed_rect.h / w.max(1e-100) / h.max(1e-100);
        self.scalar_field
            .Paint(painter, canvas_color, w, h, state.enabled, pixel_scale);
    }

    /// D-006 first-Cycle init: subscribe to config_sig, react by calling
    /// set_value_silent (no on_value feedback, no value_signal fire).
    ///
    /// DIVERGED: language-forced 1-cycle delay vs C++ emRecListener::OnRecChanged
    /// which fires synchronously inside emRec::Changed().
    fn Cycle(&mut self, ectx: &mut EngineCtx<'_>, ctx: &mut PanelCtx<'_>) -> bool {
        if !self.subscribed_to_config && !self.config_sig.is_null() {
            ectx.connect(self.config_sig, ectx.id());
            self.subscribed_to_config = true;
        }
        if !self.config_sig.is_null() && ectx.IsSignaled(self.config_sig) {
            if let Some(ref get_val) = self.get_config_val {
                self.scalar_field.set_value_silent(get_val());
            }
        }
        false
    }

    fn Input(
        &mut self,
        event: &crate::emInput::emInputEvent,
        state: &PanelState,
        input_state: &crate::emInputState::emInputState,
        ctx: &mut PanelCtx,
    ) -> bool {
        self.scalar_field.Input(event, state, input_state, ctx)
    }

    fn GetCursor(&self) -> emCursor {
        self.scalar_field.GetCursor()
    }

    fn HasHowTo(&self) -> bool {
        self.scalar_field.HasHowTo()
    }

    fn GetHowTo(&self, enabled: bool, focusable: bool) -> String {
        self.scalar_field.GetHowTo(enabled, focusable)
    }
}
```

- [ ] **Step 2.3: Update `make_factor_field` to return `FactorFieldPanel`**

Replace the existing `make_factor_field` function:

```rust
#[allow(clippy::too_many_arguments)]
fn make_factor_field(
    ctx: &mut PanelCtx<'_>,
    caption: &str,
    description: &str,
    look: Rc<emLook>,
    cfg_min: f64,
    cfg_max: f64,
    cfg_value: f64,
    minimum_means_disabled: bool,
    field_sig: SignalId,
    get_val: Box<dyn Fn() -> f64>,
) -> FactorFieldPanel {
    let mut sched = ctx
        .as_sched_ctx()
        .expect("make_factor_field requires scheduler-reach PanelCtx");
    let mut sf = emScalarField::new(&mut sched, -200.0, 200.0, look);
    sf.SetCaption(caption);
    sf.border_mut().description = description.to_string();
    sf.set_initial_value(factor_cfg_to_val(cfg_value, cfg_min, cfg_max));
    sf.SetScaleMarkIntervals(&[100, 10]);
    sf.SetTextBoxTallness(0.3);
    sf.border_mut().SetBorderScaling(1.5);
    let (min, max, dis) = (cfg_min, cfg_max, minimum_means_disabled);
    sf.SetTextOfValueFunc(Box::new(move |v, mi| {
        factor_text_of_value(v, mi, dis, min, max)
    }));
    FactorFieldPanel {
        scalar_field: sf,
        config_sig: field_sig,
        get_config_val: Some(get_val),
        subscribed_to_config: false,
    }
}
```

- [ ] **Step 2.4: Run `cargo check --workspace`**

Expected: errors at all `make_factor_field` call sites (wrong argument count) — this is expected. Do not fix yet.

- [ ] **Step 2.5: Commit**

```bash
git add crates/emcore/src/emCoreConfigPanel.rs
git commit -m "feat(D4): define FactorFieldPanel + update make_factor_field signature"
```

---

## Task 3: Update all scalar field construction sites

**Files:**
- Modify: `crates/emcore/src/emCoreConfigPanel.rs`

For each group below, the `create_children` body changes in the same way:
1. Before `make_factor_field`: get the field signal via `c.FieldName.listened_signal()` and clone the config Rc.
2. After `make_factor_field`: the `on_value` wiring remains identical.
3. Inline `ScalarFieldPanel { scalar_field: sf }` constructions become `FactorFieldPanel { scalar_field: sf, config_sig: ..., get_config_val: Some(...), subscribed_to_config: false }`.

- [ ] **Step 3.1: Update `KBGroup::create_children`**

Replace the body of `KBGroup::create_children` with:

```rust
fn create_children(&self, ctx: &mut PanelCtx) {
    let cfg = self.config.borrow();
    let c = cfg.GetRec();

    let zoom_sig = c.KeyboardZoomSpeed.listened_signal();
    let zoom_config = Rc::clone(&self.config);
    let mut zoom = make_factor_field(
        ctx,
        "Keyboard zoom speed",
        "Speed of zooming by keyboard",
        self.look.clone(),
        0.25,
        4.0,
        *c.KeyboardZoomSpeed.GetValue(),
        false,
        zoom_sig,
        Box::new(move || {
            factor_cfg_to_val(*zoom_config.borrow().GetRec().KeyboardZoomSpeed.GetValue(), 0.25, 4.0)
        }),
    );
    let on_val_config = Rc::clone(&self.config);
    zoom.scalar_field.on_value = Some(Box::new(
        move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = on_val_config.borrow_mut();
            cm.modify(|c, sc| c.KeyboardZoomSpeed.SetValue(cfg_val, sc), sched);
            let _ = cm.TrySave(false);
        },
    ));
    ctx.create_child_with("zoom", Box::new(zoom));

    let scroll_sig = c.KeyboardScrollSpeed.listened_signal();
    let scroll_config = Rc::clone(&self.config);
    let mut scroll = make_factor_field(
        ctx,
        "Keyboard scroll speed",
        "Speed of scrolling by keyboard",
        self.look.clone(),
        0.25,
        4.0,
        *c.KeyboardScrollSpeed.GetValue(),
        false,
        scroll_sig,
        Box::new(move || {
            factor_cfg_to_val(*scroll_config.borrow().GetRec().KeyboardScrollSpeed.GetValue(), 0.25, 4.0)
        }),
    );
    let on_val_config = Rc::clone(&self.config);
    scroll.scalar_field.on_value = Some(Box::new(
        move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
            let cfg_val = factor_val_to_cfg(val, 0.25, 4.0);
            let mut cm = on_val_config.borrow_mut();
            cm.modify(|c, sc| c.KeyboardScrollSpeed.SetValue(cfg_val, sc), sched);
            let _ = cm.TrySave(false);
        },
    ));
    ctx.create_child_with("scroll", Box::new(scroll));
}
```

Note: the config borrow `let cfg = self.config.borrow(); let c = cfg.GetRec();` must be dropped before the closures run — the `zoom_config`/`scroll_config`/`on_val_config` clones are separate from `cfg`. The borrow ends when `cfg` and `c` go out of scope. If the borrow checker complains, wrap the signal/initial-value extraction in a block:

```rust
let (zoom_sig, zoom_init, scroll_sig, scroll_init) = {
    let cfg = self.config.borrow();
    let c = cfg.GetRec();
    (
        c.KeyboardZoomSpeed.listened_signal(),
        *c.KeyboardZoomSpeed.GetValue(),
        c.KeyboardScrollSpeed.listened_signal(),
        *c.KeyboardScrollSpeed.GetValue(),
    )
};
// Then use zoom_init/scroll_init for cfg_value, zoom_sig/scroll_sig for field_sig.
```

- [ ] **Step 3.2: Update `KineticGroup::create_children`**

Apply the same pattern for all 4 kinetic factor fields:
- `KineticZoomingAndScrolling` — range 0.25..2.0, `minimum_means_disabled: true`
- `MagnetismRadius` — range 0.25..4.0 (check current values in source), `minimum_means_disabled: true`
- `MagnetismSpeed` — range 0.25..4.0, `minimum_means_disabled: false`
- `VisitSpeed` — range 0.25..4.0, `minimum_means_disabled: false`

For each field `F` with range `(min, max)` and `dis`:
```rust
let f_sig = c.F.listened_signal();
let f_config = Rc::clone(&self.config);
let mut f_field = make_factor_field(
    ctx, caption, description, self.look.clone(),
    min, max, *c.F.GetValue(), dis,
    f_sig,
    Box::new(move || {
        factor_cfg_to_val(*f_config.borrow().GetRec().F.GetValue(), min, max)
    }),
);
let on_val_config = Rc::clone(&self.config);
f_field.scalar_field.on_value = Some(Box::new(
    move |val, sched| {
        let cfg_val = factor_val_to_cfg(val, min, max);
        let mut cm = on_val_config.borrow_mut();
        cm.modify(|c, sc| c.F.SetValue(cfg_val, sc), sched);
        let _ = cm.TrySave(false);
    },
));
ctx.create_child_with("name", Box::new(f_field));
```

Read the existing `KineticGroup::create_children` in the source and apply this pattern to each of the 4 fields exactly, preserving captions, descriptions, ranges, and `minimum_means_disabled` values.

- [ ] **Step 3.3: Update `MouseGroup`'s factor fields**

`MouseGroup` contains `MouseMiscGroup` and 4 factor fields (MouseWheelZoomSpeed, MouseWheelZoomAcceleration, MouseZoomSpeed, MouseScrollSpeed). Apply the same pattern to each factor field call to `make_factor_field`. The `MouseMiscGroup` child creation is unchanged.

Read `MouseGroup::create_children_impl` in the source and apply the pattern.

- [ ] **Step 3.4: Update `MemFieldLayoutPanel::create_children_impl`**

The memory field uses `mem_cfg_to_val`/`mem_val_to_cfg` conversion. Replace the `ScalarFieldPanel { scalar_field: sf }` construction:

```rust
fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
    let (mem_sig, init_mb) = {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();
        (
            c.MaxMegabytesPerView.listened_signal(),
            *c.MaxMegabytesPerView.GetValue() as i32,
        )
    };

    let min_val = mem_cfg_to_val(8);
    let max_val = mem_cfg_to_val(16384);
    let mut sf = {
        let mut sched = ctx.as_sched_ctx().expect("sched");
        emScalarField::new(&mut sched, min_val, max_val, self.look.clone())
    };
    sf.SetCaption("Max megabytes per view");
    sf.set_initial_value(mem_cfg_to_val(init_mb));
    sf.SetScaleMarkIntervals(&[100, 10]);
    sf.SetTextBoxTallness(0.3);
    sf.border_mut().SetBorderScaling(1.5);
    sf.SetTextOfValueFunc(Box::new(mem_text_of_value));

    let mem_config = Rc::clone(&self.config);
    sf.on_value = Some(Box::new(
        move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
            let mb = mem_val_to_cfg(val);
            let mut cm = mem_config.borrow_mut();
            cm.modify(|c, sc| c.MaxMegabytesPerView.SetValue(mb, sc), sched);
            let _ = cm.TrySave(false);
        },
    ));

    self.mem_sig = sf.value_signal;
    let update_config = Rc::clone(&self.config);
    let mem_id = ctx.create_child_with(
        "mem",
        Box::new(FactorFieldPanel {
            scalar_field: sf,
            config_sig: mem_sig,
            get_config_val: Some(Box::new(move || {
                mem_cfg_to_val(*update_config.borrow().GetRec().MaxMegabytesPerView.GetValue() as i32)
            })),
            subscribed_to_config: false,
        }),
    );
    self.mem_id = Some(mem_id);
}
```

Also update `set_mem_value_for_test` to downcast as `FactorFieldPanel` instead of `ScalarFieldPanel`:
```rust
pub fn set_mem_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
    let id = self.mem_id.expect("mem_id set in create_children");
    tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
        p.scalar_field.set_value_for_test(value);
    });
}
```

- [ ] **Step 3.5: Update `CpuGroup::create_children_impl` — threads field**

Change the MaxRenderThreads `ScalarFieldPanel { scalar_field: sf }` to `FactorFieldPanel`:

```rust
fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
    let (threads_sig, threads_init, simd_init) = {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();
        (
            c.MaxRenderThreads.listened_signal(),
            *c.MaxRenderThreads.GetValue() as f64,
            *c.AllowSIMD.GetValue(),
        )
    };

    let mut sf = {
        let mut sched = ctx.as_sched_ctx().expect("sched");
        emScalarField::new(&mut sched, 1.0, 32.0, self.look.clone())
    };
    sf.SetCaption("Max render threads");
    sf.set_initial_value(threads_init);
    sf.SetScaleMarkIntervals(&[1]);
    sf.border_mut().outer = OuterBorderType::None;
    sf.border_mut().inner = InnerBorderType::InputField;
    sf.border_mut().SetBorderScaling(1.5);

    let on_val_config = Rc::clone(&self.config);
    sf.on_value = Some(Box::new(
        move |val, sched: &mut crate::emEngineCtx::SchedCtx<'_>| {
            let v = val as i32;
            let mut cm = on_val_config.borrow_mut();
            cm.modify(|c, sc| c.MaxRenderThreads.SetValue(v, sc), sched);
            let _ = cm.TrySave(false);
        },
    ));

    self.threads_sig = sf.value_signal;
    let update_config = Rc::clone(&self.config);
    let threads_id = ctx.create_child_with(
        "MaxRenderThreads",
        Box::new(FactorFieldPanel {
            scalar_field: sf,
            config_sig: threads_sig,
            get_config_val: Some(Box::new(move || {
                *update_config.borrow().GetRec().MaxRenderThreads.GetValue() as f64
            })),
            subscribed_to_config: false,
        }),
    );
    self.threads_id = Some(threads_id);
    self.layout.set_child_constraint(
        threads_id,
        ChildConstraint { weight: 4.0, ..Default::default() },
    );

    // AllowSIMD checkbox — unchanged
    let mut cb = {
        let mut sched = ctx.as_sched_ctx().expect("sched");
        emCheckBox::new(&mut sched, "Allow SIMD", self.look.clone())
    };
    cb.SetChecked(simd_init, ctx);
    self.simd_sig = cb.check_signal;
    let simd_id = ctx.create_child_with("allowSIMD", Box::new(CheckBoxPanel { check_box: cb }));
    self.simd_id = Some(simd_id);
}
```

Also update `set_threads_value_for_test` to use `FactorFieldPanel`:
```rust
pub fn set_threads_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
    let id = self.threads_id.expect("threads_id set in create_children");
    tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
        p.scalar_field.set_value_for_test(value);
    });
}
```

- [ ] **Step 3.6: Update `PerformanceGroup::create_children_impl` — downscale/upscale fields**

Change both `ScalarFieldPanel { scalar_field: ds_sf }` and `ScalarFieldPanel { scalar_field: us_sf }` to `FactorFieldPanel`:

```rust
fn create_children_impl(&mut self, ctx: &mut PanelCtx) {
    // MaxMemTunnel and CpuGroup children — unchanged from current source.
    // (generation arg still present here; it is removed in Task 5.)
    ctx.create_child_with(
        "maxmem",
        Box::new(MaxMemTunnelPanel::new(
            Rc::clone(&self.config),
            self.look.clone(),
            Rc::clone(&self.generation), // removed in Task 5
        )),
    );
    ctx.create_child_with(
        "cpu",
        Box::new(CpuGroup::new(
            Rc::clone(&self.config),
            self.look.clone(),
            Rc::clone(&self.generation), // removed in Task 5
        )),
    );

    let (ds_sig, ds_init, us_sig, us_init) = {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();
        (
            c.DownscaleQuality.listened_signal(),
            *c.DownscaleQuality.GetValue() as f64,
            c.UpscaleQuality.listened_signal(),
            *c.UpscaleQuality.GetValue() as f64,
        )
    };

    let mut ds_sf = {
        let mut sched = ctx.as_sched_ctx().expect("sched");
        emScalarField::new(&mut sched, 2.0, 6.0, self.look.clone())
    };
    ds_sf.SetCaption("Downscale quality");
    ds_sf.border_mut().description = "Quality of image downscaling (antialiasing filter size)".to_string();
    ds_sf.set_initial_value(ds_init);
    ds_sf.SetScaleMarkIntervals(&[1]);
    ds_sf.SetTextBoxTallness(0.3);
    ds_sf.border_mut().SetBorderScaling(1.5);
    ds_sf.SetTextOfValueFunc(Box::new(downscale_text));
    // No on_value: user→config write is handled by PerformanceGroup::Cycle
    // via IsSignaled(downscale_sig), same as the existing B-010 pattern.

    self.downscale_sig = ds_sf.value_signal;
    let ds_update_config = Rc::clone(&self.config);
    let downscale_id = ctx.create_child_with(
        "downscaleQuality",
        Box::new(FactorFieldPanel {
            scalar_field: ds_sf,
            config_sig: ds_sig,
            get_config_val: Some(Box::new(move || {
                *ds_update_config.borrow().GetRec().DownscaleQuality.GetValue() as f64
            })),
            subscribed_to_config: false,
        }),
    );
    self.downscale_id = Some(downscale_id);

    let mut us_sf = {
        let mut sched = ctx.as_sched_ctx().expect("sched");
        emScalarField::new(&mut sched, 0.0, 5.0, self.look.clone())
    };
    us_sf.SetCaption("Upscale quality");
    us_sf.border_mut().description = "Quality of image upscaling (interpolation)".to_string();
    us_sf.set_initial_value(us_init);
    us_sf.SetScaleMarkIntervals(&[1]);
    us_sf.SetTextBoxTallness(0.3);
    us_sf.border_mut().SetBorderScaling(1.5);
    us_sf.SetTextOfValueFunc(Box::new(upscale_text));
    // No on_value: user→config write is handled by PerformanceGroup::Cycle
    // via IsSignaled(upscale_sig), same as the existing B-010 pattern.

    self.upscale_sig = us_sf.value_signal;
    let us_update_config = Rc::clone(&self.config);
    let upscale_id = ctx.create_child_with(
        "upscaleQuality",
        Box::new(FactorFieldPanel {
            scalar_field: us_sf,
            config_sig: us_sig,
            get_config_val: Some(Box::new(move || {
                *us_update_config.borrow().GetRec().UpscaleQuality.GetValue() as f64
            })),
            subscribed_to_config: false,
        }),
    );
    self.upscale_id = Some(upscale_id);
}
```

Also update `set_downscale_value_for_test` and `set_upscale_value_for_test` to use `FactorFieldPanel`:
```rust
pub fn set_downscale_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
    let id = self.downscale_id.expect("downscale_id set in create_children");
    tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
        p.scalar_field.set_value_for_test(value);
    });
}
pub fn set_upscale_value_for_test(&self, tree: &mut crate::emPanelTree::PanelTree, value: f64) {
    let id = self.upscale_id.expect("upscale_id set in create_children");
    tree.with_behavior_as::<FactorFieldPanel, _>(id, |p| {
        p.scalar_field.set_value_for_test(value);
    });
}
```

- [ ] **Step 3.7: Run `cargo check --workspace`**

Expected: zero errors related to `make_factor_field` or `ScalarFieldPanel` construction (all call sites updated). Fix any remaining type mismatches.

- [ ] **Step 3.8: Commit**

```bash
git add crates/emcore/src/emCoreConfigPanel.rs
git commit -m "feat(D4): migrate all scalar field constructions to FactorFieldPanel"
```

---

## Task 4: Group-level subscribe for `MouseMiscGroup` and `CpuGroup`

**Files:**
- Modify: `crates/emcore/src/emCoreConfigPanel.rs`

- [ ] **Step 4.1: Add `config_sig` + `subscribed_to_config` to `MouseMiscGroup`**

In both the `#[cfg(any(test, feature = "test-support"))]` and `#[cfg(not(...))]` struct definitions of `MouseMiscGroup`, add two fields after `subscribed_init`:

```rust
// Config aggregate subscribe for update_output after Reset.
// Distinct from subscribed_init which gates per-checkbox wakeup subscriptions.
subscribed_to_config: bool,
config_sig: SignalId,
```

In both `MouseMiscGroup::new()` implementations, add to the struct literal:
```rust
subscribed_to_config: false,
config_sig: config.borrow().GetChangeSignal(),
```

(Note: `config` parameter is `Rc<RefCell<emRecNodeConfigModel<emCoreConfig>>>` — borrow, call, then the borrow ends.)

- [ ] **Step 4.2: Add `update_output` to `MouseMiscGroup`**

Add this method to `impl MouseMiscGroup` (alongside the existing test accessors):

```rust
/// Sync checkbox display from config. Called on config-aggregate signal
/// (e.g. after Reset To Defaults). Uses set_checked_silent to avoid firing
/// check_signal (which would re-enter the config-write branch in Cycle).
fn update_output(&self, ctx: &mut PanelCtx) {
    let (stick_val, emu_val, pan_val) = {
        let cfg = self.config.borrow();
        let c = cfg.GetRec();
        (
            self.stick_possible && *c.StickMouseWhenNavigating.GetValue(),
            *c.EmulateMiddleButton.GetValue(),
            *c.PanFunction.GetValue(),
        )
    };
    if let Some(id) = self.stick_id {
        ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_silent(stick_val);
        });
    }
    if let Some(id) = self.emu_id {
        ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_silent(emu_val);
        });
    }
    if let Some(id) = self.pan_id {
        ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_silent(pan_val);
        });
    }
}
```

- [ ] **Step 4.3: Augment `MouseMiscGroup::Cycle` with config-aggregate subscribe**

Find the existing `fn Cycle` on `impl PanelBehavior for MouseMiscGroup`. At the TOP of the Cycle body, before the existing `subscribed_init` block, insert:

```rust
// Config aggregate subscribe — distinct from per-checkbox subscribed_init.
if !self.subscribed_to_config {
    ectx.connect(self.config_sig, ectx.id());
    self.subscribed_to_config = true;
}
if ectx.IsSignaled(self.config_sig) {
    self.update_output(ctx);
}
```

(The existing `subscribed_init` + per-checkbox `IsSignaled` branches remain unchanged after this block.)

- [ ] **Step 4.4: Add `config_sig` + `subscribed_to_config` + `update_output` to `CpuGroup`**

In both `CpuGroup` struct definitions, add after `subscribed_init`:
```rust
subscribed_to_config: bool,
config_sig: SignalId,
```

In both `CpuGroup::new()` implementations, add:
```rust
subscribed_to_config: false,
config_sig: config.borrow().GetChangeSignal(),
```

Add `update_output` to `impl CpuGroup` — handles only the AllowSIMD checkbox (the MaxRenderThreads `FactorFieldPanel` self-updates):

```rust
fn update_output(&self, ctx: &mut PanelCtx) {
    let simd_val = *self.config.borrow().GetRec().AllowSIMD.GetValue();
    if let Some(id) = self.simd_id {
        ctx.tree.with_behavior_as::<CheckBoxPanel, _>(id, |p| {
            p.check_box.set_checked_silent(simd_val);
        });
    }
}
```

Augment `CpuGroup::Cycle` at the top (same pattern as `MouseMiscGroup`):
```rust
if !self.subscribed_to_config {
    ectx.connect(self.config_sig, ectx.id());
    self.subscribed_to_config = true;
}
if ectx.IsSignaled(self.config_sig) {
    self.update_output(ctx);
}
```

- [ ] **Step 4.5: Run `cargo check --workspace`**

Expected: zero errors.

- [ ] **Step 4.6: Commit**

```bash
git add crates/emcore/src/emCoreConfigPanel.rs
git commit -m "feat(D4): add config-aggregate subscribe + update_output to MouseMiscGroup/CpuGroup"
```

---

## Task 5: Remove generation counter from all structs

**Files:**
- Modify: `crates/emcore/src/emCoreConfigPanel.rs`

The generation counter threads through 11 struct definitions and their `new()` call sites. Remove it everywhere.

- [ ] **Step 5.1: Remove from leaf group structs**

For each of `KBGroup`, `KineticGroup`, `MaxMemGroup`, `MaxMemInnerTunnelPanel`, `MaxMemTunnelPanel`:

In the struct definition, delete:
```rust
generation: Rc<Cell<u64>>,
last_generation: u64,
```

In `new()` parameters, delete `generation: Rc<Cell<u64>>`.

In `new()` body, delete `let gen = generation.get();`, delete the `generation` and `last_generation` fields from the struct literal.

In `LayoutChildren`, delete the rebuild block:
```rust
let gen = self.generation.get();
if gen != self.last_generation && ctx.child_count() > 0 {
    for id in ctx.children() {
        ctx.delete_child(id);
    }
    self.last_generation = gen;
}
```

- [ ] **Step 5.2: Remove from public group structs**

For each of `MouseMiscGroup` (both cfg variants), `CpuGroup` (both cfg variants), `PerformanceGroup` (both cfg variants):

Delete `generation: Rc<Cell<u64>>` and `last_generation: u64` from each struct definition.

Delete `generation: Rc<Cell<u64>>` from `new()` parameters.

Delete `let gen = generation.get();`, `generation`, `last_generation` from `new()` body.

Delete the LayoutChildren rebuild block from each.

- [ ] **Step 5.3: Remove from `MouseGroup`, `ContentPanel`, `ButtonsPanel`**

**`MouseGroup`:** Delete `generation` field and param. Delete the rebuild block from its `LayoutChildren`.

**`ContentPanel`:** Delete `generation` field and param. Delete the rebuild block from its `LayoutChildren` (if present; it may just forward to children).

**`ButtonsPanel` (both cfg variants):** Delete `generation` field and param. In `ButtonsPanel::Cycle`, delete the line:
```rust
self.generation.set(self.generation.get() + 1);
```

- [ ] **Step 5.4: Remove from `emCoreConfigPanel`**

In `pub struct emCoreConfigPanel`, delete `generation: Rc<Cell<u64>>`.

In `emCoreConfigPanel::new()`, delete `generation: Rc::new(Cell::new(0)),`.

In `emCoreConfigPanel`'s `create_children` or `AutoExpand` equivalent, remove all `Rc::clone(&self.generation)` args passed to child constructors.

- [ ] **Step 5.5: Remove `use std::cell::Cell` if no longer used**

Check if `Cell` is still used anywhere in the file (e.g., for `Rc<Cell<bool>>`). If the only use was `Rc<Cell<u64>>` for generation, remove `Cell` from the `use std::cell::{Cell, RefCell};` import. If still used, keep it.

- [ ] **Step 5.6: Run `cargo check --workspace`**

Expected: zero errors. If there are errors about missing `generation` args at call sites, trace back to the parent that was threading the value and remove those args.

- [ ] **Step 5.7: Run `cargo clippy --workspace -- -D warnings`**

Expected: zero warnings.

- [ ] **Step 5.8: Commit**

```bash
git add crates/emcore/src/emCoreConfigPanel.rs
git commit -m "feat(D4): remove generation counter from all CpuConfigPanel groups (D-009 fix)"
```

---

## Task 6: Tests

**Files:**
- Create: `crates/emcore/tests/rec_listener_b_d4.rs`
- Modify: `crates/emcore/src/lib.rs` or test entry point (add `mod` if needed — check if other test files in `crates/emcore/tests/` use a separate registration step or are auto-discovered by Cargo)

- [ ] **Step 6.1: Check test file discovery**

Run:
```bash
ls crates/emcore/tests/
```

Cargo discovers integration test files in `tests/` automatically. No `mod` declaration needed. Proceed.

- [ ] **Step 6.2: Scaffold the test file**

Create `crates/emcore/tests/rec_listener_b_d4.rs`:

```rust
//! Integration tests for D4 — emRecListener self-update / generation counter removal.
//!
//! RUST_ONLY: dependency-forced. These tests require the emcore test-support
//! infrastructure (EngineScheduler, PanelTree) which has no C++ analogue.

#![cfg(test)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emcore::emCoreConfig::emCoreConfig;
use emcore::emEngineCtx::{DeferredAction, FrameworkDeferredAction, SchedCtx};
use emcore::emRecNodeConfigModel::emRecNodeConfigModel;
use emcore::emScheduler::EngineScheduler;
use emcore::emContext::emContext;
use emcore::emClipboard::emClipboard;

fn make_sched_ctx<'a>(
    sched: &'a mut EngineScheduler,
    actions: &'a mut Vec<DeferredAction>,
    ctx_root: &'a Rc<emcore::emContext::emContext>,
    cb: &'a RefCell<Option<Box<dyn emClipboard>>>,
    pa: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
) -> SchedCtx<'a> {
    SchedCtx {
        scheduler: sched,
        framework_actions: actions,
        root_context: ctx_root,
        framework_clipboard: cb,
        current_engine: None,
        pending_actions: pa,
    }
}

fn run_slice(sched: &mut EngineScheduler) {
    let root = emcore::emContext::emContext::NewRoot();
    let mut actions: Vec<DeferredAction> = Vec::new();
    let mut pending_inputs = Vec::new();
    let mut input_state = emcore::emInputState::emInputState::new();
    let cb: RefCell<Option<Box<dyn emClipboard>>> = RefCell::new(None);
    let pa: Rc<RefCell<Vec<FrameworkDeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let mut windows = HashMap::new();
    sched.DoTimeSlice(
        &mut windows,
        &root,
        &mut actions,
        &mut pending_inputs,
        &mut input_state,
        &cb,
        &pa,
    );
}
```

(Adapt imports to match actual module paths used in other test files like `rc_shim_b010.rs` — open that file and copy its preamble pattern.)

- [ ] **Step 6.3: Write Test 1 — `scalar_field_panel_self_updates_on_field_change`**

Add to `rec_listener_b_d4.rs`:

```rust
/// Verifies FactorFieldPanel self-update: subscribes to field signal in Cycle,
/// calls set_value_silent on fire, displays new value without feedback.
#[test]
fn scalar_field_panel_self_updates_on_field_change() {
    // This test constructs FactorFieldPanel indirectly via a minimal config
    // and a KBGroup (which uses make_factor_field → FactorFieldPanel).
    // After config mutation, advance scheduler, assert scalar_field value updated.
    //
    // Construction requires a panel tree. Use the same test harness pattern
    // as rc_shim_b010.rs tests for MouseMiscGroup/KBGroup.
    // Consult that file for the exact tree/engine setup.
    //
    // Minimum assertions:
    // 1. Create config with KeyboardZoomSpeed = 1.0 (default/mid).
    // 2. Create KBGroup, call create_children.
    // 3. Mutate config: set KeyboardZoomSpeed to 4.0 (max).
    // 4. run_slice (fires field signal → FactorFieldPanel::Cycle → set_value_silent).
    // 5. Assert: zoom child's scalar_field.GetValue() == factor_cfg_to_val(4.0, 0.25, 4.0).
    //
    // If FactorFieldPanel is not pub, gate construction under test-support feature
    // (same pattern as KBGroup/MouseMiscGroup).
    todo!("implement after verifying test harness pattern from rc_shim_b010.rs");
}
```

Then replace the `todo!` with the actual implementation by reading `crates/emcore/tests/rc_shim_b010.rs` and adapting the `KBGroup` harness.

The key steps inside the test:
```rust
// 1. Setup scheduler + config
let mut sched = EngineScheduler::new();
// ... (copy the SchedCtx setup from rc_shim_b010.rs)
let config = Rc::new(RefCell::new(
    emRecNodeConfigModel::<emCoreConfig>::new(
        emCoreConfig::new(&mut sc),
        std::path::PathBuf::from("/tmp/test_d4.em"),
        &mut sc,
    )
));

// 2. Create KBGroup + children via test harness (see rc_shim_b010.rs pattern)
// let mut kb = KBGroup::new(Rc::clone(&config), look.clone());
// kb.create_children(&mut ctx);

// 3. Mutate config
{
    let mut cm = config.borrow_mut();
    cm.modify(|c, sc| c.KeyboardZoomSpeed.SetValue(4.0, sc), &mut sc);
}

// 4. Advance scheduler (FactorFieldPanel::Cycle fires)
let _ = sc;
run_slice(&mut sched);

// 5. Assert zoom child updated
// let zoom_val = /* read zoom child's scalar_field.GetValue() via with_behavior_as::<FactorFieldPanel> */;
// assert!((zoom_val - expected_slider_val).abs() < 0.01, "zoom field must update to 4.0 config value");
```

- [ ] **Step 6.4: Write Test 2 — `mouse_misc_group_update_output_on_config_change`**

```rust
/// Verifies MouseMiscGroup group-level update_output: subscribes to config
/// aggregate, on fire calls set_checked_silent on checkboxes.
#[test]
fn mouse_misc_group_update_output_on_config_change() {
    // Same harness as rc_shim_b010.rs MouseMiscGroup tests.
    // Steps:
    // 1. Create config with StickMouseWhenNavigating = false (default).
    // 2. Create MouseMiscGroup + create_children.
    // 3. Mutate config: StickMouseWhenNavigating = true.
    // 4. run_slice (config aggregate signal → MouseMiscGroup::Cycle → update_output).
    // 5. Assert: stick checkbox IsChecked() == true (if stick_possible; set to true in test).
}
```

Implement by adapting the existing `MouseMiscGroup` test harness from `rc_shim_b010.rs`.

- [ ] **Step 6.5: Write Test 3 — `reset_button_updates_in_place_no_rebuild`**

```rust
/// Regression guard for D-009 fix: after Reset click, scalar fields update
/// in place — no child destruction/recreation.
#[test]
fn reset_button_updates_in_place_no_rebuild() {
    // Steps:
    // 1. Create config. Set KeyboardZoomSpeed = 4.0 (non-default).
    // 2. Create ButtonsPanel + KBGroup side by side in a shared tree.
    //    Call create_children on both.
    // 3. Record child count of KBGroup subtree (expect 2 — zoom + scroll).
    // 4. Fire bt_reset_sig (ButtonsPanel::Cycle reacts → resets config to defaults).
    // 5. run_slice (FactorFieldPanel::Cycle fires → set_value_silent with default val).
    // 6. Assert A: zoom child still exists (child count still 2).
    // 7. Assert B: zoom scalar_field.GetValue() == factor_cfg_to_val(1.0, 0.25, 4.0) (default).
}
```

- [ ] **Step 6.6: Run tests to make sure they compile and pass**

```bash
cargo-nextest run --test rec_listener_b_d4
```

Expected: 3 tests pass.

- [ ] **Step 6.7: Commit**

```bash
git add crates/emcore/tests/rec_listener_b_d4.rs
git commit -m "test(D4): add 3 integration tests for FactorFieldPanel self-update and generation removal"
```

---

## Task 7: End-of-bucket gate

- [ ] **Step 7.1: Full test suite**

```bash
cargo-nextest ntr
```

Expected: 2892 + 3 new = 2895 tests pass (or more if other tests were added). Zero failures.

- [ ] **Step 7.2: Annotation lint**

```bash
cargo xtask annotations
```

Expected: zero errors. If any `DIVERGED:` annotation was added without a forced-category label, fix it.

- [ ] **Step 7.3: Verify generation counter gone**

```bash
rg -n 'generation.*Cell\|Rc<Cell<u64>>' crates/emcore/src/emCoreConfigPanel.rs
```

Expected: zero hits.

- [ ] **Step 7.4: Clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: zero warnings. (The `#[allow(clippy::too_many_arguments)]` on `make_factor_field` remains — it was already there and the new signature has 10 parameters.)

- [ ] **Step 7.5: Dispatch combined-reviewer**

Dispatch a single foreground `general-purpose` subagent as a combined reviewer. Hand it:
- The spec: `docs/superpowers/specs/2026-04-29-D4-rec-listener-self-update-design.md`
- The diff since the task-1 commit
- The decisions doc: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/decisions.md` (D-006, D-007, D-008, D-009 entries)

Review gates: fmt + clippy -D warnings + nextest all green; DIVERGED annotations have categories; FactorFieldPanel::Cycle has the DIVERGED language-forced comment; no feedback loop via on_value; generation counter absent.

- [ ] **Step 7.6: Final commit (if reviewer requests fixups)**

```bash
git add -p
git commit -m "fixup(D4): combined-reviewer fixups"
```
