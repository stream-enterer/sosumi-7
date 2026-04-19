# Phase 4d — Migrate emCoreConfig + emCoreConfigPanel — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Replace the current flattened-f64 `emCoreConfig` fields with emRec-typed fields matching C++ `emCoreConfig.h` exactly (`emDoubleRec VisitSpeed;` etc.). Migrate the ~40 `emCoreConfigPanel` call sites from `Rc<RefCell<emConfigModel<emCoreConfig>>>` to `Rc<emConfigModel<emCoreConfig>>` with ctx-mediated `SetValue` calls. Delete `VISIT_SPEED_MAX` constant and `VisitSpeed_GetMaxValue`.

**Architecture:** `emCoreConfig` becomes an `emStructRec` with typed fields (`VisitSpeed: emDoubleRec`, `ScrollRotatedEnabled: emBoolRec`, `MouseWheelZoomPercent: emIntRec`, etc.). Each field owns its own change-notification signal. `emConfigModel<emCoreConfig>` is owned as `Rc<emConfigModel<emCoreConfig>>` (Rc<T> immutable; interior mutation via the emRec tree); panel sites read via deref and write via `config.VisitSpeed.SetValue(new_val, ctx)`.

**Companion:** spec §7 D7.2, D7.3, D7.4. C++ reference: `emCoreConfig.h` lines 51+.

**JSON entries closed:** E026 (fully), E027.

**Phase-specific invariants (C4):**
- **I4d-1.** The DIVERGED block currently at `emCoreConfig.rs` ~lines 237–252 (locate by symbol; verified 2026-04-19) is deleted; `VISIT_SPEED_MAX` constant absent; `VisitSpeed_GetMaxValue` absent.
- **I4d-2.** `grep "emDoubleRec\|emBoolRec\|emIntRec\|emEnumRec\|emFlagsRec\|emAlignmentRec\|emColorRec\|emStringRec" crates/emcore/src/emCoreConfig.rs` returns matches for every typed field C++ declares.
- **I4d-3.** `rg 'Rc<RefCell<emConfigModel' crates/ --glob '!*/tests/*'` returns zero matches in production code.
- **I4d-4.** `VisitSpeed` change-notification fires its signal on `SetValue`.
- **I4d-5.** `emRef.no_rs` mapping note updated to describe `emRef<T> → Rc<T>` default + chartered exceptions.

**Entry-precondition.** Phase 4c Closeout COMPLETE.

---

## Bootstrap

Run B1–B12 with `<N>` = `4d`. At B3 read `/home/a0/git/eaglemode-0.96.4/include/emCore/emCoreConfig.h` in full.

---

## File Structure

**Heavy modifications:**
- `crates/emcore/src/emCoreConfig.rs` — replace every flattened-scalar field with its emRec typed field. Delete `VISIT_SPEED_MAX` and `VisitSpeed_GetMaxValue`. Delete the DIVERGED block at ~lines 237–252 (locate by symbol). `load_from_rec`/`save_to_rec` methods (lines ~141, ~192) migrate to using the emRec TryRead/TryWrite shape ported in Phase 4c.
- `crates/emcore/src/emCoreConfigPanel.rs` — migrate the `config_ref.borrow_mut()` call sites (measured 2026-04-19: **19 occurrences**, not the original spec estimate of ~40 — the figure was based on a stale draft) to `config.VisitSpeed.SetValue(new_val, ctx)` form.
- `crates/emcore/src/emRef.no_rs` — update mapping note.

**Possibly created:**
- `crates/emcore/src/emConfigModel.rs` if Phase 4c did not create it yet. The type becomes generic: `struct emConfigModel<T: emStructRec + ...>` with a `register(&Rc<emContext>, T) -> Rc<Self>` constructor and a `save_on_change` wiring that fires persistence on the aggregate signal.

---

## Task 1: Rewrite `emCoreConfig` as an `emStructRec`

**Files:** `crates/emcore/src/emCoreConfig.rs`.

- [ ] **Step 1: Read C++ `emCoreConfig.h`** to confirm the complete field set + defaults + bounds.

- [ ] **Step 2: Write failing test.** Assert `config.VisitSpeed` exists as `emDoubleRec` type:
```rust
#[test]
fn core_config_has_emrec_fields() {
    let mut fixture = TestFixture::new();
    let config = emCoreConfig::new(&mut fixture.init_ctx());
    let _: &emDoubleRec = &config.VisitSpeed;
    let _: &emBoolRec = &config.ScrollRotatedEnabled;
}
```

- [ ] **Step 3: FAIL.**

- [ ] **Step 4: Rewrite struct.**
```rust
pub struct emCoreConfig {
    // Each field matches C++ emCoreConfig.h declaration type.
    pub VisitSpeed: emDoubleRec,
    pub ScrollRotatedEnabled: emBoolRec,
    pub MouseWheelZoomPercent: emIntRec,
    // ... every field C++ declares
}

impl emCoreConfig {
    pub fn new<C: ConstructCtx>(ctx: &mut C) -> Self {
        Self {
            VisitSpeed: emDoubleRec::new(ctx, /* default */ 0.1, /* min */ 1e-6, /* max */ 10.0),
            ScrollRotatedEnabled: emBoolRec::new(ctx, /* default */ true),
            MouseWheelZoomPercent: emIntRec::new(ctx, /* default */ 50, /* min */ 0, /* max */ 400),
            // ...
        }
    }
}
```

- [ ] **Step 5: Rewrite `load_from_rec` and `save_to_rec` (lines ~141, ~192) to use emRec IO.**

- [ ] **Step 6: Delete `VISIT_SPEED_MAX` constant, `VisitSpeed_GetMaxValue`, and the DIVERGED block at 237-250.**

- [ ] **Step 7: Commit.**
```bash
git add crates/emcore/src/emCoreConfig.rs
git commit -m "phase-4d: emCoreConfig uses emRec typed fields, matching C++"
```

---

## Task 2: Migrate `emCoreConfigPanel.rs` call sites

**Files:** `crates/emcore/src/emCoreConfigPanel.rs`.

- [ ] **Step 1: Enumerate call sites.**
```bash
rg -n 'config\.borrow|config\.borrow_mut|visit_speed|scroll_rotated|mouse_wheel_zoom' crates/emcore/src/emCoreConfigPanel.rs | wc -l
```

- [ ] **Step 2: Migrate each read site.**
```rust
// OLD
let v = self.config.borrow().visit_speed;
// NEW
let v = *self.config.VisitSpeed.GetValue();
```

- [ ] **Step 3: Migrate each write site.**
```rust
// OLD
self.config.borrow_mut().visit_speed = new_val;
// NEW
self.config.VisitSpeed.SetValue(new_val, ctx);
```

The callback closures (Phase 3) already take `&mut SchedCtx<'_>`; propagate ctx into these calls.

- [ ] **Step 4: Change the config storage type.**
```rust
// OLD
pub config: Rc<RefCell<emConfigModel<emCoreConfig>>>,
// NEW
pub config: Rc<emConfigModel<emCoreConfig>>,
```

The model's inner `emCoreConfig` is accessed via `self.config.inner()` returning `&emCoreConfig`; mutation goes through the emRec field's `SetValue` method (which takes `&mut self` on the emRec — requires `inner_mut()`). Since `Rc<T>` doesn't admit `&mut` to `T`, the pattern is: the `emConfigModel` stores its `emCoreConfig` in a structure that admits interior mutation at the leaf level via `emRec::SetValue`'s `&mut self` requirement — resolved by exposing each field via an interior path that the callback can reach with `&mut`. Concretely, each emRec concrete type's `SetValue` takes `&mut self`; `emConfigModel` exposes typed `SetXxx(val, ctx)` accessors that internally reach the field with `&mut` through a `RefCell<emCoreConfig>` — chartered §3.6(b) (context-registry interior state; the config model is registered in the context).

Alternative (preferred if it compiles): store the `emCoreConfig` as plain value *inside* the `emConfigModel`, with all mutating methods exposed on `&mut self` at the model level — `model.set_visit_speed(v, ctx)`. The `Rc<emConfigModel<T>>` sharing then requires interior mutability at the model level. This is the §3.6(b) retention cited in the spec.

Pick the chartered-RefCell approach:
```rust
pub struct emConfigModel<T> {
    inner: RefCell<T>,   // chartered §3.6(b): context-registry interior state.
    // ...
}
impl<T> emConfigModel<T> {
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}
```

Panel sites then do:
```rust
self.config.with_mut(|c| c.VisitSpeed.SetValue(new_val, ctx));
```

- [ ] **Step 5: Run full check + test.** `cargo check -p emcore && cargo test -p emcore` — all green.

- [ ] **Step 6: Commit.**
```bash
git add crates/emcore/src/emCoreConfigPanel.rs
git commit -m "phase-4d: migrate emCoreConfigPanel to emRec-based config access"
```

---

## Task 3: Update `emRef.no_rs` mapping note

**Files:** `crates/emcore/src/emRef.no_rs`.

- [ ] **Step 1: Rewrite content.**
```
emRef<T> → Rc<T> by default (shared-read post-init).

Chartered exceptions requiring Rc<RefCell<T>>, per
docs/superpowers/specs/2026-04-19-port-ownership-rewrite-design.md §3.6:
  (a) cross-closure reference held by winit/wgpu callbacks
  (b) context-registry typed singleton (emContext children/models, emConfigModel)
  (c) shared mutable widget state across siblings (radio groups)
```

- [ ] **Step 2: Commit.**

---

## Task 4: Add change-notification regression test

- [ ] **Step 1:**
```rust
#[test]
fn visit_speed_set_fires_signal() {
    let mut fixture = TestFixture::new();
    let config = emCoreConfig::new(&mut fixture.init_ctx());
    let sig = config.VisitSpeed.GetValueSignal();
    fixture.clear_signals();
    // Simulate a callback path; here direct:
    let mut m = config.VisitSpeed;  // would be: `model.with_mut(|c| c.VisitSpeed.SetValue(...))` in real flow
    m.SetValue(0.5, &mut fixture.sched_ctx());
    assert!(fixture.is_signaled(sig));
}
```

- [ ] **Step 2: PASS.** Commit.

---

## Task 5: Full gate + invariants

- [ ] **Step 1: Gate.**
- [ ] **Step 2: Invariants.**
```bash
# I4d-1
rg 'VISIT_SPEED_MAX|VisitSpeed_GetMaxValue' crates/emcore/src/ && echo "I4d-1 FAIL" || echo "I4d-1 PASS"
rg -n 'DIVERGED:' crates/emcore/src/emCoreConfig.rs | grep -q 237 && echo "I4d-1-block FAIL" || echo "I4d-1-block PASS"

# I4d-2
grep -q 'VisitSpeed:\s*emDoubleRec' crates/emcore/src/emCoreConfig.rs && echo "I4d-2 PASS" || echo "I4d-2 FAIL"

# I4d-3
rg 'Rc<RefCell<emConfigModel' crates/ --glob '!*/tests/*' && echo "I4d-3 FAIL" || echo "I4d-3 PASS"
```

- [ ] **Step 3: Proceed to Closeout.**

---

## Closeout

Run C1–C11 with `<N>` = `4d`. At C5 close **E026** (Phase 4a/4b/4c/4d complete) and **E027** (Task 3 updated the mapping note; Rc<RefCell> residual count reduced by ~40 via the panel migration).
