# F019 emFileModel lazy file_state_signal — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert `emFileModel<T>::file_state_signal` to a lazy `Cell<SignalId>`, drop the constructor's signal-id parameter, and retire `emDirPanel::Cycle`'s `stay_awake`-while-loading polling workaround.

**Architecture:** Mirror FU-005 (`emRecFileModel`) onto `emFileModel<T>`. Replace the eager `SignalId` field with `Cell<SignalId>` initialized null; expose a lazy combined-form accessor `GetFileStateSignal(&self, ectx) -> SignalId` that allocates on first call via `ectx.create_signal()`. The model's `Cycle` self-allocates before firing; `emFilePanel::Cycle`'s existing D-006 subscribe path picks the same id up. Once the signal is real, `emDirPanel::Cycle` returns `false` for observe-only paths.

**Tech Stack:** Rust 1.x, `cargo`, `cargo-nextest`, `cargo xtask annotations`. Crates: `emcore`, `emfileman`.

**Spec:** `docs/superpowers/specs/2026-05-02-F019-emfilemodel-lazy-file-state-signal-design.md`

---

## File structure

| File | Touched by | Responsibility |
|---|---|---|
| `crates/emcore/src/emFileModel.rs` | Task 1 | `emFileModel<T>` struct; `FileModelState` trait; lazy accessor; fire site |
| `crates/emcore/src/emImageFile.rs` | Task 1 | `emImageFileModel::new`; `register` (drops eager file-state signal) |
| `crates/emcore/src/emFilePanel.rs` | Task 1 | `Cycle` subscribe call site; test fixtures |
| `crates/emfileman/src/emDirModel.rs` | Task 1 | `Acquire` (drops `SignalId::default()` arg) |
| `crates/emfileman/src/emDirPanel.rs` | Task 2 | `Cycle` (drops `stay_awake`); reframed tests |
| `docs/debug/ISSUES.json` | Task 3 | F019 status flip |

Tasks 1 and 2 are **compile-cascading**: changing `emFileModel::new`'s signature requires all callers to update in the same commit. Task 1 is therefore a single multi-file commit. Task 2 can land independently after Task 1 because the polling retirement is internal to `emDirPanel::Cycle` and its tests.

---

## Task 1: Convert `emFileModel<T>` to lazy `Cell<SignalId>`

**Files:**
- Modify: `crates/emcore/src/emFileModel.rs` (lines 47, 50-66, 112-128, 130-149, 167-169, 519-528)
- Modify: `crates/emcore/src/emImageFile.rs` (lines 56-62, ~70-90 — `register`)
- Modify: `crates/emcore/src/emFilePanel.rs` (lines 528-541, 635, 945)
- Modify: `crates/emfileman/src/emDirModel.rs` (line 282)

### Step 1.1: Add new failing test for the lazy invariant

- [ ] Open `crates/emcore/src/emFileModel.rs`. Locate the `#[cfg(test)]` module (search for `#[cfg(test)]` near the bottom).

Add this test inside the test module:

```rust
#[test]
fn file_state_signal_is_null_until_first_get_with_ctx() {
    use crate::emEngineCtx::test_ctx::TestSchedCtx;
    let model: emFileModel<String> = emFileModel::new(std::path::PathBuf::from("x"));
    assert!(
        model.GetFileStateSignal_for_test().is_null(),
        "fresh emFileModel must hold null file_state_signal until GetFileStateSignal(ectx) is called"
    );
    let mut tcx = TestSchedCtx::new();
    let id1 = model.GetFileStateSignal(&mut tcx);
    assert!(!id1.is_null(), "first GetFileStateSignal(ectx) must allocate a real id");
    let id2 = model.GetFileStateSignal(&mut tcx);
    assert_eq!(id1, id2, "GetFileStateSignal(ectx) must be idempotent — second call returns the same id");
}
```

(`GetFileStateSignal_for_test` is a `#[doc(hidden)]` accessor we add in step 1.5 mirroring `emRecFileModel::file_state_signal_for_test` at `emRecFileModel.rs:107`.)

### Step 1.2: Run test to verify it fails to compile

Run:
```
cargo check -p emcore --tests 2>&1 | head -40
```
Expected: compilation error — `emFileModel::new` takes 2 args; `GetFileStateSignal` does not take ctx; `GetFileStateSignal_for_test` does not exist. This proves the invariant is currently absent.

### Step 1.3: Convert the struct field to `Cell<SignalId>`

- [ ] In `crates/emcore/src/emFileModel.rs` at line 117, replace:

```rust
file_state_signal: SignalId,
```

with:

```rust
/// Port of inherited C++ `emFileModel::FileStateSignal` (F019).
/// Lazy-allocated on first `GetFileStateSignal(&self, ectx)` call;
/// null until then. Mirrors `emRecFileModel::file_state_signal`
/// (FU-005) for `emFileModel<T>` callers without scheduler reach
/// at construction (notably `emDirModel::Acquire`).
file_state_signal: Cell<SignalId>,
```

Add `use std::cell::Cell;` to the file's import block if not already present.

### Step 1.4: Update the constructor

- [ ] Replace `pub fn new(path: PathBuf, file_state_signal: SignalId) -> Self {` (line 131) with:

```rust
pub fn new(path: PathBuf) -> Self {
    Self {
        data: None,
        path,
        state: FileState::Waiting,
        error_text: String::new(),
        file_state_signal: Cell::new(SignalId::null()),
        memory_limit: usize::MAX,
        memory_need: 0,
        file_progress: 0.0,
        last_mtime: 0,
        last_size: 0,
        out_of_date: false,
        ignore_update_signal: false,
        clients: Vec::new(),
        memory_limit_invalid: true,
        priority_invalid: true,
    }
}
```

Remove the old body (lines 132-149).

### Step 1.5: Replace eager accessor with lazy combined-form accessor

- [ ] Replace the inherent `GetFileStateSignal` (currently at line 167):

```rust
pub fn GetFileStateSignal(&self) -> SignalId {
    self.file_state_signal
}
```

with:

```rust
/// Port of C++ `emFileModel::GetFileStateSignal()` with lazy
/// allocation (F019). Allocates on first call; returns the live id
/// thereafter. Subscribers call this at first-Cycle subscribe time.
/// Mirrors `emRecFileModel::ensure_file_state_signal` (FU-005) and
/// `emFilePanel::ensure_vir_file_state_signal` (B-004).
pub fn GetFileStateSignal(&self, ectx: &mut impl SignalCtx) -> SignalId {
    let cur = self.file_state_signal.get();
    if cur.is_null() {
        let new_id = ectx.create_signal();
        self.file_state_signal.set(new_id);
        new_id
    } else {
        cur
    }
}

/// Test-only accessor for the raw `file_state_signal` slot (without
/// allocating). Mirrors `emRecFileModel::file_state_signal_for_test`.
#[doc(hidden)]
pub fn GetFileStateSignal_for_test(&self) -> SignalId {
    self.file_state_signal.get()
}
```

Add `use crate::emEngineCtx::SignalCtx;` to the import block if not already present (check existing imports first).

### Step 1.6: Update the `FileModelState` trait method to take ctx

- [ ] Replace the trait declaration at line 47:

```rust
fn GetFileStateSignal(&self) -> SignalId;
```

with:

```rust
fn GetFileStateSignal<C: SignalCtx>(&self, ectx: &mut C) -> SignalId;
```

- [ ] Replace the trait impl at line 63-65:

```rust
fn GetFileStateSignal(&self) -> SignalId {
    self.file_state_signal
}
```

with:

```rust
fn GetFileStateSignal<C: SignalCtx>(&self, ectx: &mut C) -> SignalId {
    emFileModel::GetFileStateSignal(self, ectx)
}
```

### Step 1.7: Update fire site inside `emFileModel::Cycle`

- [ ] Replace the fire site at line 525:

```rust
ctx.fire(self.file_state_signal);
```

with:

```rust
let sig = self.GetFileStateSignal(ctx);
ctx.fire(sig);
```

(Two-step form because `ctx` is reborrowed; calling the accessor and firing in one expression risks a borrow conflict on `ctx`.)

### Step 1.8: Cascade the constructor change to `emImageFile.rs`

- [ ] Open `crates/emcore/src/emImageFile.rs`. Replace `emImageFileModel::new` at line 56:

```rust
pub fn new(path: PathBuf, change_signal: SignalId, data_change_signal: SignalId) -> Self {
    Self {
        file_model: emFileModel::new(path, change_signal),
        data_change_signal,
        saving_quality: 100,
    }
}
```

with:

```rust
pub fn new(path: PathBuf, data_change_signal: SignalId) -> Self {
    Self {
        file_model: emFileModel::new(path),
        data_change_signal,
        saving_quality: 100,
    }
}
```

- [ ] Locate `register` (line ~73). Find the body that currently allocates `change_signal` upfront via `ctx.create_signal()`. Remove that allocation and update the `Self::new` call accordingly. The `data_change_signal` and `load_complete_signal` allocations stay — they are independent signals.

Specifically, replace:
```rust
let change_signal = ctx.create_signal();
let load_complete_signal = ctx.create_signal();

let mut model = Self::new(path, change_signal, load_complete_signal);
```

with:
```rust
let load_complete_signal = ctx.create_signal();

let mut model = Self::new(path, load_complete_signal);
```

(The file_model's signal is allocated lazily on first `GetFileStateSignal(ectx)` call by a subscriber.)

### Step 1.9: Update `emFilePanel::Cycle` subscribe site

- [ ] Open `crates/emcore/src/emFilePanel.rs`. Replace the block at lines 528-531:

```rust
let target_sig = self
    .model
    .as_ref()
    .map(|m| m.borrow().GetFileStateSignal())
    .unwrap_or_else(SignalId::null);
```

with:

```rust
let target_sig = self
    .model
    .as_ref()
    .map(|m| m.borrow().GetFileStateSignal(ectx))
    .unwrap_or_else(SignalId::null);
```

### Step 1.10: Update `emFilePanel.rs` test fixtures

- [ ] At `emFilePanel.rs:635`, find the `emFileModel::new` call inside the test fixture:

```rust
let model = Rc::new(RefCell::new(emFileModel::new(
    std::path::PathBuf::from("/tmp/x"),
    SignalId::default(),
)));
```

(Or whatever the current shape is — read the actual code.) Drop the second argument:

```rust
let model = Rc::new(RefCell::new(emFileModel::new(
    std::path::PathBuf::from("/tmp/x"),
)));
```

- [ ] At `emFileModel.rs:945`, the test-block `emFileModel::new` site, drop the signal arg the same way.

### Step 1.11: Update `emDirModel::Acquire`

- [ ] Open `crates/emfileman/src/emDirModel.rs` line 282. Replace:

```rust
file_model: emFileModel::new(PathBuf::from(name), SignalId::default()),
```

with:

```rust
file_model: emFileModel::new(PathBuf::from(name)),
```

If the `SignalId` import becomes unused after this change, remove it (cargo will warn).

### Step 1.12: Update any other emImageFileModel construction sites

- [ ] Run:
```
rg -n "emImageFileModel::new\(" crates/
```

For each hit, verify the call site no longer passes the `change_signal` arg. If any test/fixture still passes three args, drop the second positional. If the test was specifically asserting eager-signal behavior (unlikely), reframe per spec §7.

### Step 1.13: Verify the lazy-invariant test now compiles and passes

Run:
```
cargo nextest run -p emcore file_state_signal_is_null_until_first_get_with_ctx
```
Expected: PASS.

### Step 1.14: Full crate build + nextest

Run:
```
cargo build --workspace --all-targets 2>&1 | tail -20
```
Expected: clean build, no warnings.

Run:
```
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
```
Expected: clean.

Run:
```
cargo nextest run --workspace
```
Expected: all tests pass (note: `emDirPanel` `stay_awake` tests still pass because `emDirPanel::Cycle` still returns `stay_awake=true` while loading; that retirement happens in Task 2).

If any test fails, investigate before proceeding. Common failures:
- Test fixtures still passing the `SignalId` arg → fix the fixture.
- A test that relied on `GetFileStateSignal()` returning the cached id without ctx → that test needs to call the lazy accessor with a TestSchedCtx, or use `GetFileStateSignal_for_test()` for non-allocating reads.

### Step 1.15: Annotation lint

Run:
```
cargo xtask annotations
```
Expected: clean.

### Step 1.16: Commit

```bash
git add crates/emcore/src/emFileModel.rs \
        crates/emcore/src/emImageFile.rs \
        crates/emcore/src/emFilePanel.rs \
        crates/emfileman/src/emDirModel.rs
git commit -m "feat(emFileModel): F019 — lazy file_state_signal mirroring FU-005

Convert emFileModel<T>::file_state_signal to Cell<SignalId> with a lazy
combined-form accessor GetFileStateSignal(&self, ectx) that allocates on
first call. Drop the eager SignalId arg from emFileModel::new so callers
with no scheduler reach (notably emDirModel::Acquire) can construct
without a forever-null id. emFilePanel::Cycle's D-006 subscribe path
already picks up the lazy id at first-Cycle subscribe time. Mirrors
emRecFileModel's FU-005 shape.

Cascading sig changes: emImageFileModel::new, emFilePanel test fixtures,
emDirModel::Acquire."
```

---

## Task 2: Retire `emDirPanel::Cycle` `stay_awake`-while-loading polling

**Files:**
- Modify: `crates/emfileman/src/emDirPanel.rs` (lines 382-421 — Cycle body; lines 844-1100 — affected tests)

### Step 2.1: Read each `stay_awake` test to understand its observable property

- [ ] Read the tests at the line numbers below and write down (in your scratch notes) what observable property each was checking. Approximate cluster:
  - Line 851: a `stay_awake=true while loading "/tmp"` regression test
  - Line 936: PanelCycleEngine fire-and-requeue while loading
  - Line 1041, 1054, 1071: cross-engine wakeup tests assuming `stay_awake=true`
  - Line 1105: `stay_awake` interaction with notice/wake-up

For each, the underlying property is: **the panel makes progress while the model is loading and reflects state changes promptly.** After Task 1, the same property holds via `FileStateSignal`-driven wakes; the test must verify *that* path now.

### Step 2.2: Drop the `stay_awake` polling from `Cycle`

- [ ] Open `crates/emfileman/src/emDirPanel.rs`. Replace lines 382-421 (the entire `let stay_awake = …` block plus the `Loading | Waiting => true` arm and the trailing `changed || stay_awake`) with:

```rust
// Observe the model. On Loaded, materialize children if not yet
// built (child_count == 0 doubles as the "haven't built children
// yet" predicate). On error, surface the message via file_panel.
// While Loading or Waiting, return false — the panel is woken on
// FileStateSignal fires via the D-006 subscribe path established
// by emFilePanel::Cycle (B-015). The earlier stay_awake=true
// workaround (F017 compensation for the never-fired signal) is
// retired now that emFileModel<T> allocates its FileStateSignal
// lazily (F019).
let observed_state = self
    .dir_model
    .as_ref()
    .map(|dm| dm.borrow().get_file_state());
match &observed_state {
    Some(FileState::Loaded) => {
        if self.child_count == 0 {
            self.file_panel.clear_custom_error();
            self.update_children(ctx);
        }
    }
    Some(FileState::LoadError(e)) => {
        self.file_panel.set_custom_error(e);
    }
    _ => {}
}

// Same-Cycle drain: set_custom_error / clear_custom_error above flip
// pending_vir_state_fire; drain so VirFileStateSignal observers see
// the fire this tick (mirrors C++ where the signal would have fired
// synchronously inside emFilePanel::Cycle).
self.file_panel.fire_pending_vir_state(ectx);

// B-016 (3) MANDATORY emFilePanel::Cycle suffix — cycle_inner +
// conditional fire. Mirrors emImageFileImageFilePanel.rs:232-235.
let changed = self.file_panel.cycle_inner();
if changed && !self.file_panel.GetVirFileStateSignal().is_null() {
    ectx.fire(self.file_panel.GetVirFileStateSignal());
}
changed
```

### Step 2.3: Run the test suite and triage failures

Run:
```
cargo nextest run -p emfileman
```
Expected: a small cluster of `stay_awake`-named tests fail. Each must be reframed individually in steps 2.4-2.x.

### Step 2.4: Reframe each failing test

For each failing test:
- [ ] Identify the observable property the test was asserting (see Step 2.1 notes).
- [ ] Rewrite the assertion to verify that property via the FileStateSignal-driven wake path. Concretely: after invoking the loading transition, fire the model's FileStateSignal (or trigger the load progress that fires it), and assert the panel was scheduled/cycled in response.
- [ ] **Critical:** the reframed test must still fail under a regression that re-introduces the polling workaround. If it would pass either way, the assertion is too weak — strengthen it. A reframed test that says "Cycle was called at least once during the load" is too weak; a test that says "Cycle was called exactly N times where N matches state-change events, not 50ms slices" is right.

Run after each reframe:
```
cargo nextest run -p emfileman <test_name>
```

Repeat until all `stay_awake` tests pass under the new shape.

### Step 2.5: Add an integration-style proof-of-fix test

- [ ] Add a new test in the `emDirPanel.rs` test module:

```rust
#[test]
fn loading_dir_wakes_panel_via_filestatesignal_not_polling() {
    // Constructs emDirModel via Acquire (no scheduler reach);
    // observes that emDirPanel.Cycle is invoked once per state
    // change, not once per scheduler slice.
    // ...
    // Concrete construction follows the existing fixtures at the
    // top of the test module — copy the shape from one of the
    // /tmp loading tests that were reframed in Step 2.4.
}
```

The body details must mirror an existing test fixture (do not invent new infrastructure). The new assertion is on Cycle invocation count: while loading a directory of N entries, Cycle is invoked O(state-changes), not O(slices).

### Step 2.6: Full workspace nextest + clippy + annotations

Run:
```
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask annotations
```
Expected: all clean.

### Step 2.7: Commit

```bash
git add crates/emfileman/src/emDirPanel.rs
git commit -m "refactor(emDirPanel): F019 — retire stay_awake-while-loading polling

emDirPanel::Cycle no longer returns stay_awake=true during Loading or
Waiting. The F017 workaround was a compensation for emFileModel's
forever-null FileStateSignal (caused by emDirModel::Acquire's lack of
scheduler reach at construction). With F019's lazy file_state_signal
landing in the prior commit, the panel now wakes via the D-006
FileStateSignal connect/disconnect path established by emFilePanel
B-015. Cycle becomes pure observe-only.

Tests at lines 851, 936, 1041, 1054, 1071, 1105 reframed from polling
assertions to fire-driven wake assertions. New proof-of-fix test
asserts O(state-changes) Cycle invocations, not O(slices)."
```

---

## Task 3: Update `docs/debug/ISSUES.json` for F019

**Files:**
- Modify: `docs/debug/ISSUES.json` (F019 entry, line ~468)

### Step 3.1: Flip F019 to `needs-manual-verification`

- [ ] Open `docs/debug/ISSUES.json`. Locate the `F019` entry. Update these fields:
  - `status`: `"open"` → `"needs-manual-verification"`
  - `fixed_in_commit`: `null` → the SHA of Task 2's commit (run `git rev-parse HEAD` to get it)
  - `fixed_date`: `null` → today's date in `YYYY-MM-DD` form
  - `fix_note`: replace the current "Partial progress as of 2026-05-02..." note with:

```
F019 model-side fix landed 2026-05-02 (Task 1 commit + Task 2 commit). emFileModel<T> now holds Cell<SignalId> with a lazy combined-form GetFileStateSignal(&self, ectx) accessor mirroring FU-005 emRecFileModel. emDirModel::Acquire constructs file_model without any signal arg. emFilePanel::Cycle's D-006 subscribe (B-015) picks up the lazy id at first-Cycle subscribe time. emDirPanel::Cycle dropped its stay_awake-while-loading polling (F017 compensation) and is now pure observe-only — wakes occur via the FileStateSignal connect path. Manual verification: observe directory loading at parity with the F017 baseline (no observable regression in load speed or interactivity); confirm Cycle invocation count tracks state changes, not scheduler slices.
```

  - `repro`: leave unchanged (the repro narrative is still useful for verifying the fix held).

### Step 3.2: Validate JSON

Run:
```
python3 -c "import json; json.load(open('docs/debug/ISSUES.json')); print('OK')"
```
Expected: `OK`.

### Step 3.3: Commit

```bash
git add docs/debug/ISSUES.json
git commit -m "docs(F019): flip to needs-manual-verification — model-side lazy file_state_signal landed

Task 1 + Task 2 closed the model-side gap (emFileModel<T> lazy
Cell<SignalId>) and retired emDirPanel::Cycle's stay_awake polling
workaround. Awaiting manual verification of directory-loading parity
with the F017 baseline before flipping to closed."
```

---

## Self-review against spec

| Spec §   | Plan task |
|----------|-----------|
| §3 architecture (Cell field; new sig; lazy accessor; fire site) | Task 1 (steps 1.3-1.7) |
| §4 emFileModel.rs | Task 1 (steps 1.3-1.7) |
| §4 emImageFile.rs | Task 1 (step 1.8) |
| §4 emFilePanel.rs | Task 1 (steps 1.9, 1.10) |
| §4 emDirModel.rs | Task 1 (step 1.11) |
| §4 emDirPanel.rs | Task 2 (steps 2.2, 2.4) |
| §5 data flow (both orderings) | Task 1 step 1.7 (model self-allocates) + Task 2 step 2.5 (proof-of-fix integration test) |
| §6 borrow ordering | Task 1 step 1.9 |
| §7 unit tests added | Task 1 step 1.1 (lazy invariant + idempotency) |
| §7 tests updated | Task 1 step 1.10 + Task 2 step 2.4 |
| §7 integration test | Task 2 step 2.5 |
| §7 acceptance gates | Task 1 step 1.14, Task 2 step 2.6, Task 3 step 3.1 |
| §8 out of scope | Not in plan (correctly omitted) |
| §9 test reframe surface area risk | Task 2 step 2.1 (note observable property first) + 2.4 (regression-resistance check) |
| §9 trait sig change risk | Task 1 step 1.6 (compiler-enforced) |
| §9 ordering invariant risk | Task 1 step 1.1 (idempotency test) + Task 1 step 1.7 (model self-allocates) |
