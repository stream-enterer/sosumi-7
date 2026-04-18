# SP3 — CoreConfig ownership on emView — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `emView` a real `CoreConfig: Rc<RefCell<emCoreConfig>>` field; realign `emVisitingViewAnimator::SetAnimParamsByCoreConfig` to the C++ `(&emCoreConfig)` signature; close the 3 `PHASE-W4-FOLLOWUP:` markers at `crates/emcore/src/emView.rs:877, 923, 947`.

**Architecture:** Additive on `emCoreConfig` first (new const + getter — decoupled from everything), then a coordinated cut-over of `emView::new`'s signature and `SetAnimParamsByCoreConfig`'s signature. A `#[cfg(any(test, feature = "test-support"))] emView::new_for_test` helper absorbs the ~170 test-side call sites that don't care about config.

**Tech Stack:** Rust, `Rc<RefCell<T>>`, existing `test-support` cargo feature on `emcore`.

**Spec:** `docs/superpowers/specs/2026-04-18-emview-sp3-coreconfig-ownership-design.md`.

**C++ references:**
- `~/git/eaglemode-0.96.4/include/emCore/emView.h:664` — `emRef<emCoreConfig> CoreConfig;`
- `~/git/eaglemode-0.96.4/include/emCore/emCoreConfig.h:51` — `emDoubleRec VisitSpeed;`
- `~/git/eaglemode-0.96.4/src/emCore/emCoreConfig.cpp:53` — `VisitSpeed(this,"VisitSpeed",1.0,0.1,10.0)`
- `~/git/eaglemode-0.96.4/src/emCore/emView.cpp:35, :505, :519, :536` — acquire + three use sites
- `~/git/eaglemode-0.96.4/src/emCore/emViewAnimator.cpp:979-990` — animator body

---

## Scope check

Spec describes one coherent change — a single sub-project. No decomposition needed.

## File structure

| File | Action |
|---|---|
| `crates/emcore/src/emCoreConfig.rs` | add const + getter (additive) |
| `crates/emcore/src/emViewAnimator.rs` | change `SetAnimParamsByCoreConfig` signature + body; migrate 2 in-file tests |
| `crates/emcore/src/emView.rs` | add `CoreConfig` field; change `new` signature; add `new_for_test`; rewrite 3 call sites; migrate ~60 in-file test call sites |
| `crates/emcore/src/emWindow.rs` | 2 production call sites — construct default config |
| `crates/emcore/src/emSubViewPanel.rs` | 1 production call site — internal-default config with `DIVERGED:` note |
| `crates/eaglemode/tests/**`, `examples/**`, `crates/eaglemode/benches/**` | ~110 test/bench/example sites — migrate to `new_for_test` (or pass explicit default for non-test contexts) |

No new files.

---

## Task 1: Add `VISIT_SPEED_MAX` constant + `VisitSpeed_GetMaxValue` getter to `emCoreConfig`

**Files:**
- Modify: `crates/emcore/src/emCoreConfig.rs` (append inside the existing `impl emCoreConfig` block at line 210)

**Rationale:** Additive, compiles independently. Lands the infrastructure `SetAnimParamsByCoreConfig` will need in Task 2.

- [ ] **Step 1: Write the failing test**

Append inside `crates/emcore/src/emCoreConfig.rs` at the end of the file (before any existing `#[cfg(kani)]` block — find the line `#[cfg(kani)]` and insert the test module above it):

```rust
#[cfg(test)]
mod sp3_tests {
    use super::*;

    #[test]
    fn visit_speed_max_matches_cpp_schema() {
        // C++ emCoreConfig.cpp:53 — VisitSpeed(this,"VisitSpeed",1.0,0.1,10.0)
        assert_eq!(emCoreConfig::VISIT_SPEED_MAX, 10.0);
    }

    #[test]
    fn visit_speed_getmaxvalue_returns_const() {
        let cfg = emCoreConfig::default();
        assert_eq!(cfg.VisitSpeed_GetMaxValue(), emCoreConfig::VISIT_SPEED_MAX);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p emcore emCoreConfig::sp3_tests`
Expected: compile error — `VISIT_SPEED_MAX` and `VisitSpeed_GetMaxValue` not defined.

- [ ] **Step 3: Implement the const + getter**

In `crates/emcore/src/emCoreConfig.rs`, inside the existing `impl emCoreConfig` block (starting line 210), add (after the existing `Acquire` method, before the closing `}`):

```rust
    /// Upper bound for `VisitSpeed`.
    ///
    /// DIVERGED: C++ exposes this via `VisitSpeed.GetMaxValue()` on an
    /// `emRec`-typed field (`emDoubleRec VisitSpeed;` in emCoreConfig.h:51;
    /// bound declared by `VisitSpeed(this,"VisitSpeed",1.0,0.1,10.0)` in
    /// emCoreConfig.cpp:53). Rust flattens to a plain `f64` field + const
    /// because the `emRec`-backed scalar-field infrastructure is not ported.
    pub const VISIT_SPEED_MAX: f64 = 10.0;

    /// C++ `emCoreConfig::VisitSpeed.GetMaxValue()`.
    ///
    /// DIVERGED: flattened from member-of-emRec-field to method-on-config,
    /// see `VISIT_SPEED_MAX` docs.
    pub fn VisitSpeed_GetMaxValue(&self) -> f64 {
        Self::VISIT_SPEED_MAX
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p emcore emCoreConfig::sp3_tests`
Expected: `test result: ok. 2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emCoreConfig.rs
git commit -m "$(cat <<'EOF'
feat(emCoreConfig): add VISIT_SPEED_MAX + VisitSpeed_GetMaxValue (SP3 prep)

Additive infrastructure for SP3 — exposes the 10.0 upper bound on
VisitSpeed that SetAnimParamsByCoreConfig will consume once realigned
to the C++ (const emCoreConfig &) signature.

DIVERGED: C++ uses emRec-typed field; Rust flattens to plain f64 + const.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Realign `SetAnimParamsByCoreConfig` signature to `(&emCoreConfig)`

**Files:**
- Modify: `crates/emcore/src/emViewAnimator.rs:741-749` (the method), `:2922-2936` (the `visiting_set_anim_params` in-file test)

**Blast radius:** Changes the signature. Breaks the 3 call sites in `emView.rs:892, 924, 948` — those will be fixed in Task 3. Temporarily, the crate won't compile between Task 2 and Task 3 — so we land them as one commit via task sequencing. **This task writes the animator change but does not commit until Task 3 finishes.**

- [ ] **Step 1: Write the failing test**

In `crates/emcore/src/emViewAnimator.rs`, locate the `visiting_set_anim_params` test at line ~2922 and *replace* it with:

```rust
    #[test]
    fn visiting_set_anim_params_from_core_config() {
        use crate::emCoreConfig::emCoreConfig;
        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);

        // Below max: animated. visit_speed=2.0, max=10.0.
        let cfg = emCoreConfig {
            visit_speed: 2.0,
            ..Default::default()
        };
        anim.SetAnimParamsByCoreConfig(&cfg);
        assert!(anim.animated);
        assert!((anim.acceleration - 70.0).abs() < 0.01);
        assert!((anim.max_absolute_speed - 70.0).abs() < 0.01);
        assert!((anim.max_cusp_speed - 35.0).abs() < 0.01);

        // At max: not animated (instant). visit_speed == max == 10.0.
        let cfg_max = emCoreConfig {
            visit_speed: 10.0,
            ..Default::default()
        };
        anim.SetAnimParamsByCoreConfig(&cfg_max);
        assert!(!anim.animated);
    }

    #[test]
    fn visiting_set_anim_params_sub_max_still_animates() {
        use crate::emCoreConfig::emCoreConfig;
        let mut anim = emVisitingViewAnimator::new(0.0, 0.0, 1.0, 5.0);
        let cfg = emCoreConfig {
            visit_speed: 9.9999,
            ..Default::default()
        };
        anim.SetAnimParamsByCoreConfig(&cfg);
        // 9.9999 < 10.0 * 0.99999 = 9.9999 — boundary: strictly-less predicate makes this false;
        // C++ emViewAnimator.cpp:986 uses the same `f < fMax*0.99999` predicate.
        // Expect animated=false at 9.9999 (equal to fMax*0.99999 to f64 precision).
        assert!(!anim.animated);
    }
```

- [ ] **Step 2: Do NOT run tests yet**

Skip compile check — we expect compile failure from the 3 unchanged call sites in `emView.rs`. Proceed to Step 3 immediately.

- [ ] **Step 3: Change `SetAnimParamsByCoreConfig` signature + body**

In `crates/emcore/src/emViewAnimator.rs`, replace the method at lines 741-749:

```rust
    /// Configure animation parameters from the view's `emCoreConfig`.
    ///
    /// Mirrors C++ `emVisitingViewAnimator::SetAnimParamsByCoreConfig`
    /// (emViewAnimator.cpp:979-990): reads `VisitSpeed` and its max value
    /// from `coreConfig`, sets `animated` based on the strict-less-than
    /// predicate `f < fMax*0.99999`, and derives acceleration / speed
    /// bounds as linear multiples of `f`.
    pub fn SetAnimParamsByCoreConfig(&mut self, core_config: &crate::emCoreConfig::emCoreConfig) {
        let f = core_config.visit_speed;
        let f_max = core_config.VisitSpeed_GetMaxValue();
        self.animated = f < f_max * 0.99999;
        self.acceleration = 35.0 * f;
        self.max_absolute_speed = 35.0 * f;
        self.max_cusp_speed = self.max_absolute_speed * 0.5;
    }
```

- [ ] **Step 4: Do not commit.**

Proceed directly to Task 3. Compile check happens there.

---

## Task 3: Add `emView::CoreConfig` field + change `new` signature + add `new_for_test` + fix 3 call sites + delete `PHASE-W4-FOLLOWUP:` markers

**Files:**
- Modify: `crates/emcore/src/emView.rs`
  - Struct definition (find `pub struct emView { ` — add field)
  - `impl emView { pub fn new(...)` starting line 478
  - Three call sites at `:892, :924, :948` (line numbers before Task 2 edits; use the `SetAnimParamsByCoreConfig` grep to locate them)
  - Comment blocks at `:877-881` and `:923` and `:947` — delete the `PHASE-W4-FOLLOWUP:` text
  - All ~60 in-file test call sites of `emView::new` → replace with `emView::new_for_test`

**Blast radius:** This completes the signature-change cut-over started in Task 2. After this task's edits, the `emcore` crate compiles again, but downstream crates (`eaglemode`, benches, examples) still call the old 3-arg `emView::new`. Those are fixed in Tasks 4–6.

**Import note:** `emView.rs` must `use` `emCoreConfig` at the top. Verify the import list; if missing, add:
```rust
use crate::emCoreConfig::emCoreConfig;
```

- [ ] **Step 1: Write the failing test** (SP3 view-side tests)

In `crates/emcore/src/emView.rs`, locate the existing `#[cfg(test)] mod tests` block (search for a `mod tests` near the end of the file) and append these tests inside it:

```rust
    #[test]
    fn sp3_view_owns_corecfg() {
        use crate::emCoreConfig::emCoreConfig;
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut tree = PanelTree::new();
        let root = tree.create_root("");
        let cfg = Rc::new(RefCell::new(emCoreConfig::default()));
        let view = emView::new(root, 800.0, 600.0, Rc::clone(&cfg));
        assert!(Rc::ptr_eq(&view.CoreConfig, &cfg));

        cfg.borrow_mut().visit_speed = 7.5;
        assert_eq!(view.CoreConfig.borrow().visit_speed, 7.5);
    }

    #[test]
    fn sp3_visit_uses_view_corecfg() {
        use crate::emCoreConfig::emCoreConfig;
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut tree = PanelTree::new();
        let root = tree.create_root("");
        tree.Layout(root, 0.0, 0.0, 800.0, 600.0, 1.0);

        let cfg = Rc::new(RefCell::new(emCoreConfig {
            visit_speed: 10.0, // at max → animated=false
            ..Default::default()
        }));
        let mut view = emView::new(root, 800.0, 600.0, Rc::clone(&cfg));

        view.VisitByIdentityBare(":", false, "");

        let va = view.VisitingVA.borrow();
        assert!(!va.animated,
            "visit_speed=10.0 must deactivate animation via f < fMax*0.99999");
    }
```

- [ ] **Step 2: Do NOT run tests yet**

Proceed — we expect compile errors until the signature changes below land.

- [ ] **Step 3: Add `CoreConfig` field to the struct**

In `crates/emcore/src/emView.rs`, find the `pub struct emView {` definition. Locate the `VisitingVA:` field (the animator `Rc<RefCell<...>>`) and add immediately after it:

```rust
    /// Port of C++ `emView.h:664` — `emRef<emCoreConfig> CoreConfig`.
    /// Acquired at construction. SP-later will source this via
    /// `emCoreConfig::Acquire(ctx.GetRootContext())` once `emContext`
    /// is threaded through `emView::new`.
    pub CoreConfig: Rc<RefCell<crate::emCoreConfig::emCoreConfig>>,
```

- [ ] **Step 4: Change `emView::new` signature**

Replace the `new` function's signature at line 478:

```rust
    pub fn new(
        root: PanelId,
        viewport_width: f64,
        viewport_height: f64,
        core_config: Rc<RefCell<crate::emCoreConfig::emCoreConfig>>,
    ) -> Self {
```

Then inside the `Self { ... }` literal, add the field initializer (alongside `VisitingVA:`):

```rust
            CoreConfig: core_config,
```

- [ ] **Step 5: Add `new_for_test` helper**

Immediately after the `new` function's closing brace (inside the `impl emView` block), add:

```rust
    /// Test-only constructor that default-constructs `emCoreConfig`.
    /// Kept out of non-test builds to force production callers to
    /// commit to an explicit `CoreConfig` source.
    #[cfg(any(test, feature = "test-support"))]
    pub fn new_for_test(root: PanelId, viewport_width: f64, viewport_height: f64) -> Self {
        use std::cell::RefCell;
        use std::rc::Rc;
        let cfg = Rc::new(RefCell::new(
            crate::emCoreConfig::emCoreConfig::default(),
        ));
        Self::new(root, viewport_width, viewport_height, cfg)
    }
```

- [ ] **Step 6: Fix the 3 `SetAnimParamsByCoreConfig` call sites**

Replace the three `va.SetAnimParamsByCoreConfig(1.0, 10.0);` calls. Before Step 6, the file has (approximate lines):
- `:892` inside `VisitByIdentity`
- `:924` inside `VisitFullsizedByIdentity`
- `:948` inside `VisitByIdentityBare`

Each site becomes:

```rust
        va.SetAnimParamsByCoreConfig(&self.CoreConfig.borrow());
```

**Critical:** `self.CoreConfig.borrow()` must be called *before* `self.VisitingVA.borrow_mut()` OR the `va` binding must be dropped before this line — otherwise two borrows on `self`. Since `va` borrows `VisitingVA` (not `CoreConfig`), distinct `RefCell`s mean simultaneous borrows are fine. But the borrow of `self` itself is split: `.VisitingVA` is a reborrow of `&mut self`, and `.CoreConfig` is another reborrow. Rust's NLL should permit this since neither reborrow is stored past the method call. If the borrow checker complains, refactor each site to:

```rust
        let cfg = self.CoreConfig.borrow();
        let mut va = self.VisitingVA.borrow_mut();
        va.SetAnimParamsByCoreConfig(&cfg);
        va.SetGoalCoords(...); // etc — unchanged
        va.Activate();
```

Apply this pattern proactively to all three sites to avoid borrow-check churn.

- [ ] **Step 7: Delete `PHASE-W4-FOLLOWUP:` comments**

At site `:877-881` (before `VisitByIdentity`), the existing comment contains:
```
    /// PHASE-W4-FOLLOWUP: C++ passes this view's `CoreConfig` to
    /// `SetAnimParamsByCoreConfig`. Rust `emView` does not yet own a
    /// `emCoreConfig`, so we hardcode the stock defaults
    /// (`VisitSpeed=1.0`, `MaxVisitSpeed=10.0`) from emCoreConfig.cpp:53.
    /// Full `CoreConfig` ownership is a future wave.
```
Delete those 5 lines (keep the doc comment above them).

At `:923` (inside `VisitFullsizedByIdentity`): delete `        // PHASE-W4-FOLLOWUP: CoreConfig defaults — see Task 3.1.`

At `:947` (inside `VisitByIdentityBare`): delete `        // PHASE-W4-FOLLOWUP: CoreConfig defaults — see Task 3.1.`

- [ ] **Step 8: Migrate in-file test call sites of `emView::new` → `emView::new_for_test`**

There are approximately 60 in-file test call sites in `emView.rs`. Enumerate precisely:

```bash
rg -n 'emView::new\(' crates/emcore/src/emView.rs
```

For each match inside `#[cfg(test)]` blocks (most of them), rewrite:
- From: `emView::new(root, W, H)` (3 args)
- To: `emView::new_for_test(root, W, H)`

Exception: the two SP3 tests written in Step 1 (`sp3_view_owns_corecfg`, `sp3_visit_uses_view_corecfg`) intentionally use the full `emView::new` — leave them alone.

Use `sed` for the mechanical bulk:
```bash
# Preview the changes first:
rg -n 'emView::new\(([^,]+), ([^,]+), ([^)]+)\)' crates/emcore/src/emView.rs | head -20
```
Then manually apply or use a targeted sed pass. Avoid replacing the new 4-arg calls (SP3 tests); a precise sed guard:
```bash
sed -i 's/emView::new(root, 800\.0, 600\.0)/emView::new_for_test(root, 800.0, 600.0)/g' crates/emcore/src/emView.rs
```
(Adjust literal numeric args if other sizes appear — re-run `rg` post-sed to verify zero 3-arg `emView::new(` remain in the file, except the `new_for_test` body itself which calls `Self::new(root, viewport_width, viewport_height, cfg)` — 4 args, already correct.)

- [ ] **Step 9: Run emcore tests**

Run: `cargo test -p emcore --lib`
Expected: all existing tests pass; SP3 tests pass. If compile errors remain, re-check Step 8 migration coverage.

- [ ] **Step 10: Do NOT commit yet**

Downstream crates (`eaglemode`, benches, examples) still won't compile. Proceed to Task 4.

---

## Task 4: Migrate `emWindow.rs` + `emSubViewPanel.rs` + `emViewAnimator.rs` + `emViewInputFilter.rs` call sites

**Files:**
- Modify: `crates/emcore/src/emWindow.rs:193, 323`
- Modify: `crates/emcore/src/emSubViewPanel.rs:51`
- Modify: `crates/emcore/src/emViewAnimator.rs:2786, 3126, 3300, 3325, 3399, 3499`
- Modify: `crates/emcore/src/emViewInputFilter.rs:2537`

- [ ] **Step 1: Fix production `emWindow.rs` call sites**

In `crates/emcore/src/emWindow.rs`, line ~193 and ~323. Each `let view = emView::new(root_panel, w as f64, h as f64);` (or similar) becomes:

```rust
        let core_config = Rc::new(RefCell::new(emCoreConfig::default()));
        let view = emView::new(root_panel, w as f64, h as f64, core_config);
```

Add imports at the top of the file if missing:
```rust
use crate::emCoreConfig::emCoreConfig;
```
(`Rc` and `RefCell` likely already imported.)

- [ ] **Step 2: Fix `emSubViewPanel.rs:51`**

Replace the line at `:51`:

```rust
        // DIVERGED: C++ emSubViewPanel shares the parent context's emCoreConfig
        // singleton via the context chain (emView ctor: emCoreConfig::Acquire(
        // GetRootContext())). Rust emSubViewPanel::new has no parent/context
        // accessible, so it default-constructs a standalone config. Removed
        // by SP-later when emContext is threaded through emView::new.
        let core_config = std::rc::Rc::new(std::cell::RefCell::new(
            crate::emCoreConfig::emCoreConfig::default(),
        ));
        let sub_view = emView::new(root, 1.0, 1.0, core_config);
```

- [ ] **Step 3: Fix `emViewAnimator.rs` test call sites**

Six test sites (2786, 3126, 3300, 3325, 3399, 3499) — all inside `#[cfg(test)]` test fns. Replace each `emView::new(root, 800.0, 600.0)` with `emView::new_for_test(root, 800.0, 600.0)`.

Mechanical sed:
```bash
sed -i 's/emView::new(root, 800\.0, 600\.0)/emView::new_for_test(root, 800.0, 600.0)/g' crates/emcore/src/emViewAnimator.rs
```
Verify: `rg -n 'emView::new\(' crates/emcore/src/emViewAnimator.rs` — expect zero 3-arg matches.

- [ ] **Step 4: Fix `emViewInputFilter.rs:2537`**

Same pattern — replace `emView::new(root, 800.0, 600.0)` with `emView::new_for_test(root, 800.0, 600.0)`.

- [ ] **Step 5: Verify `emcore` still compiles**

Run: `cargo check -p emcore --all-features`
Expected: clean.

Run: `cargo check -p emcore --tests --all-features`
Expected: clean.

- [ ] **Step 6: Run emcore tests**

Run: `cargo test -p emcore`
Expected: all pass.

- [ ] **Step 7: Do NOT commit yet**

Downstream crates (`eaglemode`, benches, examples) still broken. Proceed.

---

## Task 5: Migrate `eaglemode` crate test/bench/support call sites to `new_for_test`

**Files (all tests/benches/support/examples — 27 files, ~110 call sites):**
- `crates/eaglemode/tests/support/mod.rs`
- `crates/eaglemode/tests/support/pipeline.rs`
- `crates/eaglemode/tests/unit/panel.rs`
- `crates/eaglemode/tests/unit/input_dispatch_chain.rs`
- `crates/eaglemode/tests/unit/max_popup_rect_fallback.rs`
- `crates/eaglemode/tests/integration/input.rs`
- `crates/eaglemode/tests/golden/*.rs` (compositor, input_filter, interaction, animator, composition, widget_interaction, widget, notice, parallel, test_panel)
- `crates/eaglemode/benches/common/mod.rs`
- `crates/eaglemode/benches/common/scaled.rs`
- `examples/bench_interaction.rs`
- `examples/profile_hotpaths.rs`
- `examples/profile_testpanel.rs`
- `examples/bench_zoom_animate.rs`
- `examples/bench_zoom_depth.rs`

**Blast radius:** Large mechanical sed. Test/bench/example code — not golden-tested code paths. `new_for_test` is the correct replacement for all of these (they don't configure `visit_speed`).

**Pre-requisite:** `new_for_test` must be visible to these callers. Options:
1. `eaglemode` dev-dep on `emcore` already enables `test-support` feature — verify via `grep -A3 'emcore.*test-support' crates/eaglemode/Cargo.toml`. If not, add `features = ["test-support"]` to the `emcore` dev-dep entry.
2. Benches and examples are non-`cfg(test)` contexts. If `new_for_test` is `#[cfg(any(test, feature = "test-support"))]`, benches compile with the feature if the `eaglemode` `[dependencies]` entry (not just dev-dep) enables it. Alternative: benches/examples pass an explicit `Rc::new(RefCell::new(emCoreConfig::default()))`.

- [ ] **Step 1: Verify / enable `test-support` feature wiring**

Check `crates/eaglemode/Cargo.toml`:
```bash
grep -B1 -A3 'emcore' crates/eaglemode/Cargo.toml
```

If the `[dev-dependencies]` emcore entry lacks `features = ["test-support"]`, add it. Tests use dev-deps; they will see `new_for_test`.

For benches and examples (non-test contexts): they compile against `[dependencies]`, not `[dev-dependencies]`. Safest plan:
- Tests (`crates/eaglemode/tests/**`): use `new_for_test` (dev-dep has `test-support`).
- Benches (`crates/eaglemode/benches/**`) and examples (`examples/**`): pass explicit `Rc::new(RefCell::new(emCoreConfig::default()))`.

- [ ] **Step 2: Mechanical sed on tests**

Run:
```bash
for f in crates/eaglemode/tests/support/mod.rs \
         crates/eaglemode/tests/support/pipeline.rs \
         crates/eaglemode/tests/unit/panel.rs \
         crates/eaglemode/tests/unit/input_dispatch_chain.rs \
         crates/eaglemode/tests/unit/max_popup_rect_fallback.rs \
         crates/eaglemode/tests/integration/input.rs \
         crates/eaglemode/tests/golden/compositor.rs \
         crates/eaglemode/tests/golden/input_filter.rs \
         crates/eaglemode/tests/golden/interaction.rs \
         crates/eaglemode/tests/golden/animator.rs \
         crates/eaglemode/tests/golden/composition.rs \
         crates/eaglemode/tests/golden/widget_interaction.rs \
         crates/eaglemode/tests/golden/widget.rs \
         crates/eaglemode/tests/golden/notice.rs \
         crates/eaglemode/tests/golden/parallel.rs \
         crates/eaglemode/tests/golden/test_panel.rs; do
    sed -i -E 's/(emcore::)?emView::(emView::)?new\(([^,]+),\s*([^,]+),\s*([^)]+)\)/\1emView::\2new_for_test(\3, \4, \5)/g' "$f"
done
```

Verify no 3-arg `emView::new(` remain in tests:
```bash
rg 'emView::(emView::)?new\b\(' crates/eaglemode/tests/ | grep -v 'new_for_test'
```
Expected: empty.

- [ ] **Step 3: Hand-fix benches + examples with explicit default config**

For each of:
- `crates/eaglemode/benches/common/mod.rs:765`
- `crates/eaglemode/benches/common/scaled.rs:80`
- `examples/bench_interaction.rs:701`
- `examples/bench_zoom_depth.rs:118, 200`
- `examples/bench_zoom_animate.rs:135`
- `examples/profile_hotpaths.rs:97`
- `examples/profile_testpanel.rs:674`

Transform (exact content varies per site — adjust the `_, _, _` placeholders to match the existing args at each site):

Before:
```rust
let mut view = emView::new(root, vw as f64, vh as f64);
```

After:
```rust
let core_config = std::rc::Rc::new(std::cell::RefCell::new(
    eaglemode_rs::emCore::emCoreConfig::emCoreConfig::default(),
));
let mut view = emView::new(root, vw as f64, vh as f64, core_config);
```

(Path prefix for `emCoreConfig` depends on how the file imports. Check the existing `emView` import — e.g., `use eaglemode_rs::emCore::emView::emView;` — and mirror it for `emCoreConfig`.)

Verify zero 3-arg stragglers:
```bash
rg 'emView::(emView::)?new\b\(' crates/eaglemode/benches/ examples/ | grep -v 'new_for_test\|new(.*,.*,.*,'
```
Expected: empty.

- [ ] **Step 4: Compile + full test suite**

Run: `cargo check --all-targets --all-features`
Expected: clean.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

Run: `cargo-nextest ntr`
Expected: all pass (same count as baseline pre-SP3, plus the 5 new SP3 tests — 2 in `emCoreConfig`, 2 in `emViewAnimator`, 2 in `emView`).

- [ ] **Step 5: Golden tests**

Run: `cargo test --test golden -- --test-threads=1`
Expected: same 237/243 baseline (no movement — defaults produce identical animator numbers to the removed hardcoded `(1.0, 10.0)`).

If any golden test moves, STOP and diagnose via `scripts/verify_golden.sh --report` before committing. A divergence here means the hardcoded pair was not equivalent to `default()` in some path — investigate before proceeding.

- [ ] **Step 6: Marker audit**

Run:
```bash
git grep -n PHASE-W4-FOLLOWUP crates/
```
Expected: empty.

Run:
```bash
git grep -nE 'SetAnimParamsByCoreConfig\s*\(\s*1\.0\s*,\s*10\.0\s*\)' crates/
```
Expected: empty.

- [ ] **Step 7: Commit the combined change**

```bash
git add crates/emcore/src/emViewAnimator.rs \
        crates/emcore/src/emView.rs \
        crates/emcore/src/emWindow.rs \
        crates/emcore/src/emSubViewPanel.rs \
        crates/emcore/src/emViewInputFilter.rs \
        crates/eaglemode/tests \
        crates/eaglemode/benches \
        examples \
        crates/eaglemode/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(emView): CoreConfig ownership + SetAnimParamsByCoreConfig realign (SP3)

- emView gains Rc<RefCell<emCoreConfig>> CoreConfig field; new() takes it.
- emVisitingViewAnimator::SetAnimParamsByCoreConfig signature realigned to
  C++ (const emCoreConfig &) — reads visit_speed + VisitSpeed_GetMaxValue().
- Three call sites in emView.rs (VisitByIdentity, VisitFullsizedByIdentity,
  VisitByIdentityBare) now pass self.CoreConfig.
- 3× PHASE-W4-FOLLOWUP: markers at emView.rs:877,923,947 deleted.
- emView::new_for_test helper (test-support feature) absorbs ~170 test
  call sites that don't configure visit_speed.
- emSubViewPanel::new default-constructs a local config with DIVERGED note:
  C++ shares parent context; SP-later (context threading) will fix.

Closes §8.1 item 10 / §4.6 of docs/superpowers/notes/2026-04-18-emview-
subsystem-closeout.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Update closeout doc

**Files:**
- Modify: `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md`

- [ ] **Step 1: Mark SP3 complete in §8.0 table**

Find the row for SP3 in the sub-project table (~§8.0) and update:
- State column: `Not started; ARCH` → `**Complete YYYY-MM-DD** (merged as <short-hash>).`
- Artifacts column: add `specs/2026-04-18-emview-sp3-coreconfig-ownership-design.md`, `plans/2026-04-18-emview-sp3-coreconfig-ownership.md`.

- [ ] **Step 2: Strike item 10 in §8.1**

Locate "10. **[ARCH] `CoreConfig` ownership on `emView`**" and update to:
```markdown
10. ~~**[ARCH] `CoreConfig` ownership on `emView`**~~ **CLOSED YYYY-MM-DD** (<short-hash>). `emView` gained `CoreConfig: Rc<RefCell<emCoreConfig>>`; `SetAnimParamsByCoreConfig` realigned to C++ `(&emCoreConfig)`; all 3 `PHASE-W4-FOLLOWUP:` markers deleted. Context threading (C++-shared singleton via `emCoreConfig::Acquire`) deferred to SP-later.
```

- [ ] **Step 3: Update §1 marker counts**

In the "Phase-follow-up markers" row:
- Before: `3 PHASE-W4-FOLLOWUP (CoreConfig defaults)`
- After: `0 PHASE-W4-FOLLOWUP (all cleared by SP3 on YYYY-MM-DD)`

In the "Known Rust-port incompletenesses remaining" row, remove `CoreConfig` ownership on `emView` (SP3),`.

- [ ] **Step 4: Update §6 markers table**

Row for `PHASE-W4-FOLLOWUP:` — update count from `3` to `0` with a `Closed by SP3` note.

- [ ] **Step 5: Commit closeout update**

```bash
git add docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md
git commit -m "$(cat <<'EOF'
docs(closeout): mark SP3 complete

CoreConfig ownership landed — closes §8.1 item 10 and all 3
PHASE-W4-FOLLOWUP markers.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review

**Spec coverage:**
- §2.1 (constructor-arg acquisition) → Task 3 Step 4 ✓
- §2.2 (animator signature realignment) → Task 2 Step 3 ✓
- §2.3 (const + getter) → Task 1 Step 3 ✓
- §2.4 (emSubViewPanel DIVERGED) → Task 4 Step 2 ✓
- §2.5 (new_for_test helper) → Task 3 Step 5 ✓
- §3.1–3.4 file-level changes → Tasks 1–4 ✓
- §3.5 call sites → Tasks 4–5 ✓
- §4 tests → Tasks 1 (2 tests), 2 (2 tests), 3 (2 tests) = 6 tests ✓ (spec listed 5; 6th is `visit_sub_max_still_animates` added for boundary symmetry)
- §5 markers closed/added → Task 3 Step 7 (closes), Tasks 1 Step 3 + Task 4 Step 2 (DIVERGED adds) ✓
- §7 verification gates → Task 5 Steps 4–6 ✓

**Placeholder scan:** no TBDs; all code blocks concrete; sed commands exact; commit messages written in full.

**Type consistency:**
- `Rc<RefCell<emCoreConfig>>` used consistently as the `CoreConfig` field type.
- `&emCoreConfig` used as the `SetAnimParamsByCoreConfig` parameter type.
- `emView::new` 4-arg signature consistent across all migration steps.
- `new_for_test` 3-arg signature consistent.

**Risk notes:**
- Task 2 + Task 3 uncommitted state — the repo is temporarily broken between Task 2 Step 3 and Task 3 Step 9. Executor must not interrupt. If interrupted, `git restore` all changes and restart from Task 2 Step 1.
- Task 5 Step 2's sed pattern assumes exact whitespace `(root, _, _, _)` — verify regex matches all sites by running the preview `rg` before the sed in-place pass. Any unmatched site needs manual migration.
- `Rc`/`RefCell` imports: most sites already have them from the existing `emView::new_dummy`-style patterns, but verify per-file.
