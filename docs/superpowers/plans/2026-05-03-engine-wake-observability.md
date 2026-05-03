# Engine Wake Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add seven new instrumentation line types to `instr/hang-2026-05-02`, extend `analyze_hang.py` with `idle` and `blink` commands, run two captures, write two findings docs.

**Architecture:** All source-side instrumentation lands as one commit on `instr/hang-2026-05-02` (no merge to main). Analyzer extension lands as a second commit on the same branch. Branch HEAD tagged `instr-7-loop-chain`. Two captures (idle, focused test-panel TextField) produce two findings docs committed directly to `main`. No fix code in scope.

**Tech Stack:** Rust (emCore instrumentation), Python (analyzer), bash (capture script), git.

**Spec:** `docs/superpowers/specs/2026-05-03-engine-wake-observability-design.md`

**Branch:** `instr/hang-2026-05-02` for all source/analyzer commits. Main for findings docs.

---

## File Structure

**Modified files (on `instr/hang-2026-05-02`):**

| File | Responsibility | Tasks |
|---|---|---|
| `crates/emcore/src/emScheduler.rs` | Generic `register_engine`, `register_engine_dyn`, type_name in `EngineData`, `STAYAWAKE` line, `WAKE` line, `#[track_caller]` | T4, T6, T7 |
| `crates/emcore/src/emPanel.rs` or `emPanelTree.rs` | Panel behavior type_name storage; generic put_behavior or equivalent | T5 |
| `crates/emcore/src/emView.rs` | `NOTICE` line at delivery site (line 4187) | T8 |
| `crates/emcore/src/emColorFieldFieldPanel.rs` | `BLINK_CYCLE` line in `TextFieldPanel::Cycle` | T9 |
| `crates/emcore/src/emEngineCtx.rs` | `INVAL_REQ` line in `PanelCtx::request_invalidate_self` | T10 |
| `crates/emcore/src/emPanelCycleEngine.rs` | `INVAL_DRAIN` line in drain check | T11 |
| `crates/emcore/src/emEngineCtx.rs` (and other layers) | Thread type_name through engine-registration trait methods | T4 |
| `scripts/analyze_hang.py` | Parsers for 7 new line types, `idle` and `blink` commands, validation pre-pass | T12, T13, T14, T15 |

**Created files:**

| File | Responsibility | Tasks |
|---|---|---|
| `scripts/run_blink_capture.sh` | Manual-capture launcher with click-timing instructions | T16 |
| `scripts/test_analyze_hang.py` | Pytest unit tests for analyzer | T12, T13, T14, T15 |
| `docs/scratch/<YYYY-MM-DD>-has-awake-findings.md` | A1 findings | T19 (committed to `main`) |
| `docs/scratch/<YYYY-MM-DD>-blink-findings.md` | A2 findings | T21 (committed to `main`) |

**Branches:**

- `instr/hang-2026-05-02` — instrumentation + analyzer commits, tagged `instr-7-loop-chain` at end. Never merges to main.
- `main` — findings docs only.

---

## Phase 1 — Audits (no code changes)

### Task 1: Audit `register_engine` call sites

**Files:** read-only

**Goal:** Confirm whether all call sites pass concrete types (so a generic `register_engine<E>` is sufficient) or whether some pass pre-erased `Box<dyn emEngine>` (requiring a `register_engine_dyn` parallel entry point).

- [ ] **Step 1: Run grep on the branch and capture output**

```bash
git checkout instr/hang-2026-05-02
git grep -n "\.register_engine(" -- 'crates/' > /tmp/register_engine_audit.txt
wc -l /tmp/register_engine_audit.txt
```

- [ ] **Step 2: For each call site, read the surrounding context to classify it**

```bash
# For each line in /tmp/register_engine_audit.txt, read 5 lines around it
while IFS=: read -r file line _; do
  echo "=== $file:$line ==="
  sed -n "$((line-3)),$((line+5))p" "$file"
done < /tmp/register_engine_audit.txt > /tmp/register_engine_audit_ctx.txt
```

- [ ] **Step 3: Classify each call site as concrete or pre-erased**

Open `/tmp/register_engine_audit_ctx.txt`. For each call site:
- **Concrete:** pattern is `.register_engine(Box::new(SomeType { ... }), ...)` or `.register_engine(Box::new(SomeType::new(...)), ...)`. The type at the `Box::new` is concrete and known at the call site.
- **Pre-erased:** pattern is `.register_engine(behavior, ...)` where `behavior` is a `Box<dyn emEngine>` from a function parameter, factory, or trait method.

Write the classification into `/tmp/register_engine_audit_classified.txt` (manual annotation):
```
crates/emcore/src/emEngineCtx.rs:266 | PRE-ERASED  | trait method delegates from Box<dyn emEngine> param
crates/emcore/src/emPanelTree.rs:1 | CONCRETE | Box::new(...) inline
...
```

- [ ] **Step 4: Verify no commit was made**

```bash
git status
```
Expected: clean working tree (audit is read-only).

**No commit for this task.** The classification file is consumed by Task 4.

---

### Task 2: Audit panel-behavior installation sites

**Files:** read-only

**Goal:** Identify how `PanelBehavior` is installed onto a panel so we can plumb a `behavior_type_name` field. Find the analogue of `register_engine` for panel behaviors.

- [ ] **Step 1: Find `put_behavior` and panel-creation sites**

```bash
git grep -n "put_behavior\|new_panel\|create_panel\|insert_panel" -- 'crates/emcore/src/emPanelTree.rs' 'crates/emcore/src/emPanel.rs' > /tmp/put_behavior_audit.txt
```

- [ ] **Step 2: Read the principal panel-creation function**

```bash
git grep -n "pub.*fn.*panel.*->.*PanelId" -- 'crates/emcore/src/emPanelTree.rs' | head -10
```

For each result, read the function body. Identify the single function that takes a `Box<dyn PanelBehavior>` (or generic `B: PanelBehavior`) and stores it into the tree. There may be multiple panel-creation paths; identify which is the "primary" one.

- [ ] **Step 3: Document findings**

Write `/tmp/panel_behavior_audit.txt` with:
- Primary panel-creation function name and signature
- Whether it currently takes a generic `B: PanelBehavior` or a pre-erased `Box<dyn PanelBehavior>`
- Other call sites (if any) that bypass the primary function

**No commit.** Findings consumed by Task 5.

---

### Task 3: Audit Notice delivery sites

**Files:** read-only

**Goal:** Confirm `emView.rs:4187` (`behavior.notice(flags, &state, &mut ctx);`) is the sole production Notice-delivery site, or identify the others.

- [ ] **Step 1: Grep for Notice invocations**

```bash
git grep -n "behavior\.notice(\|panel\.notice(\|\.notice(" -- 'crates/emcore/src/' > /tmp/notice_audit.txt
```

- [ ] **Step 2: Classify each result**

For each match in `/tmp/notice_audit.txt`, identify:
- Production-path delivery (the actual one fired by panel state changes)
- Test-only or specialized delivery (e.g., direct `panel.notice(NoticeFlags::MEMORY_LIMIT_CHANGED, ...)` calls in `emFilePanel.rs` are likely from test/refresh paths)

- [ ] **Step 3: Decide instrumentation point**

Default expectation (per spec): single instrumentation at `emView.rs:4187`. If audit reveals other production paths, instrument each. Document the decision in `/tmp/notice_audit_decision.txt`.

**No commit.** Findings consumed by Task 8.

---

## Phase 2 — Type-name plumbing

### Task 4: Refactor `register_engine` and add `EngineData.type_name`

**Files:**
- Modify: `crates/emcore/src/emScheduler.rs`
- Modify: `crates/emcore/src/emEngineCtx.rs` (trait method delegation)
- Modify: All call sites identified by Task 1 audit

**Goal:** Make `register_engine` generic in the engine type so `std::any::type_name::<E>()` is captured at the monomorphized call site. For pre-erased call sites, add `register_engine_dyn(behavior, type_name, priority, scope)`. Store the `&'static str` in `EngineData`.

- [ ] **Step 1: Write a failing test for type_name capture**

Create `crates/emcore/tests/unit/engine_type_name.rs`:

```rust
//! Verifies that register_engine captures the concrete engine's type_name.

use emcore::emEngineCtx::EngineCtx;
use emcore::emScheduler::Scheduler;
use emcore::emEngine;
use emcore::emPanelTree::PanelScope;
use emcore::emEngineCtx::Priority;

struct DummyEngine;

impl emEngine for DummyEngine {
    fn Cycle(&mut self, _ctx: &mut EngineCtx) -> bool { false }
}

#[test]
fn register_engine_captures_concrete_type_name() {
    let mut sched = Scheduler::new();
    let id = sched.register_engine(DummyEngine, Priority::Normal, PanelScope::Framework);
    let name = sched.engine_type_name(id).expect("engine_type_name returns Some for registered engine");
    // type_name format is "crate::module::TypeName" or similar; assert it ends with the concrete type
    assert!(name.ends_with("DummyEngine"), "got: {}", name);
}

#[test]
fn register_engine_dyn_uses_explicit_name() {
    let mut sched = Scheduler::new();
    let beh: Box<dyn emEngine> = Box::new(DummyEngine);
    let id = sched.register_engine_dyn(beh, "test::ExplicitName", Priority::Normal, PanelScope::Framework);
    assert_eq!(sched.engine_type_name(id), Some("test::ExplicitName"));
}
```

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p emcore --test engine_type_name 2>&1 | tail -20
```
Expected: compile error (`register_engine` signature mismatch, `register_engine_dyn` not found, `engine_type_name` not found).

- [ ] **Step 3: Update `EngineData` to store `type_name`**

In `crates/emcore/src/emScheduler.rs`, find the `EngineData` struct (around line 343 region). Add:

```rust
pub(crate) struct EngineData {
    pub priority: Priority,
    pub awake_state: i32,
    pub behavior: Option<Box<dyn emEngine>>,
    pub clock: u64,
    // RUST_ONLY: (language-forced-utility) Concrete type name captured at
    // the monomorphized register_engine<E> call site. Required because Rust
    // trait objects do not preserve concrete-type information through their
    // vtables; C++ has full RTTI. Used exclusively by instrumentation.
    pub type_name: &'static str,
}
```

- [ ] **Step 4: Refactor `register_engine` to be generic**

Replace the existing `register_engine` (around line 343):

```rust
pub fn register_engine<E: emEngine + 'static>(
    &mut self,
    behavior: E,
    priority: Priority,
    scope: PanelScope,
) -> EngineId {
    let type_name = std::any::type_name::<E>();
    self.register_engine_dyn(Box::new(behavior), type_name, priority, scope)
}

pub fn register_engine_dyn(
    &mut self,
    behavior: Box<dyn emEngine>,
    type_name: &'static str,
    priority: Priority,
    scope: PanelScope,
) -> EngineId {
    let id = self.inner.engines.insert(EngineData {
        priority,
        awake_state: -1,
        behavior: Some(behavior),
        clock: self.inner.clock,
        type_name,
    });
    self.inner.engine_scopes.insert(id, scope);
    id
}

/// Returns the concrete engine type name captured at registration time.
/// Returns `None` if the engine has been removed.
pub fn engine_type_name(&self, id: EngineId) -> Option<&'static str> {
    self.inner.engines.get(id).map(|e| e.type_name)
}
```

- [ ] **Step 5: Update concrete call sites (per Task 1 audit)**

For each call site classified as `CONCRETE` in `/tmp/register_engine_audit_classified.txt`, change `Box::new(...)` wrapping to direct value pass:

Before:
```rust
let id = sched.register_engine(Box::new(MyEngine::new()), Priority::Normal, scope);
```
After:
```rust
let id = sched.register_engine(MyEngine::new(), Priority::Normal, scope);
```

Use search-replace for mechanical sites; review compilation errors and fix unique cases manually.

- [ ] **Step 6: Update pre-erased call sites (per Task 1 audit) to use `register_engine_dyn`**

For each call site classified as `PRE-ERASED`, change:

Before:
```rust
let id = sched.register_engine(behavior, priority, scope);  // where behavior: Box<dyn emEngine>
```
After:
```rust
let id = sched.register_engine_dyn(behavior, type_name_arg, priority, scope);
```

This requires the caller to thread `type_name_arg` (a `&'static str`) from wherever `behavior` came from. For trait methods (e.g., `EngineCtx::register_engine`), update the trait signature:

```rust
// In emEngineCtx.rs, the trait method:
pub trait RegisterEngineSink {
    fn register_engine_dyn(
        &mut self,
        behavior: Box<dyn emEngine>,
        type_name: &'static str,
        priority: Priority,
        scope: PanelScope,
    ) -> EngineId;
}

// And a generic helper that captures type_name on the caller side:
pub fn register_engine<E: emEngine + 'static>(
    sink: &mut impl RegisterEngineSink,
    behavior: E,
    priority: Priority,
    scope: PanelScope,
) -> EngineId {
    let name = std::any::type_name::<E>();
    sink.register_engine_dyn(Box::new(behavior), name, priority, scope)
}
```

(Exact API depends on Task 1 audit results. The implementer adapts the pattern to actual code shape.)

- [ ] **Step 7: Run the type_name unit tests**

```bash
cargo test -p emcore --test engine_type_name 2>&1 | tail -20
```
Expected: PASS for both tests.

- [ ] **Step 8: Run full project build**

```bash
cargo check --workspace 2>&1 | tail -30
```
Expected: clean. If errors, they're at call sites that weren't updated — fix and re-run.

- [ ] **Step 9: Run clippy and tests**

```bash
cargo clippy --workspace -- -D warnings 2>&1 | tail -10
cargo-nextest ntr 2>&1 | tail -20
```
Expected: clean clippy; nextest passes (existing baseline).

- [ ] **Step 10: Commit (incremental — bundle the rest of source instrumentation in Task 12-final commit)**

Defer the commit. The plan groups all source-side instrumentation into one commit at end of Phase 5.

---

### Task 5: Add `behavior_type_name` to panel storage

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (or wherever panels store behaviors per Task 2)
- Modify: All panel-creation call sites identified by Task 2 audit

**Goal:** Capture concrete `PanelBehavior` type_name at panel creation, store next to the behavior. Used by Task 8 (NOTICE) to populate `recipient_type`.

- [ ] **Step 1: Write a failing test**

Create `crates/emcore/tests/unit/panel_behavior_type_name.rs`:

```rust
//! Verifies that panel creation captures the concrete behavior's type_name.

use emcore::emPanelTree::PanelTree;
use emcore::emPanel::{PanelBehavior, NoticeFlags, PanelState};
use emcore::emEngineCtx::PanelCtx;

struct DummyBehavior;

impl PanelBehavior for DummyBehavior {
    fn notice(&mut self, _flags: NoticeFlags, _state: &PanelState, _ctx: &mut PanelCtx) {}
}

#[test]
fn panel_creation_captures_concrete_behavior_type_name() {
    let mut tree = PanelTree::new();
    let id = tree.create_panel(DummyBehavior /* args per actual API */);
    let name = tree.behavior_type_name(id).expect("Some for created panel");
    assert!(name.ends_with("DummyBehavior"), "got: {}", name);
}
```

(Exact API may differ — Task 2 audit determined the actual creation function signature. Adapt the test to the real shape.)

- [ ] **Step 2: Run test, verify it fails**

```bash
cargo test -p emcore --test panel_behavior_type_name 2>&1 | tail -10
```
Expected: compile error.

- [ ] **Step 3: Add `behavior_type_name: &'static str` to the panel record**

In `emPanelTree.rs` (location identified by Task 2), add:

```rust
pub(crate) struct PanelData {
    // ... existing fields ...
    // RUST_ONLY: (language-forced-utility) Concrete behavior type name
    // captured at the monomorphized panel-creation call site. Required
    // because Rust trait objects do not preserve concrete-type info through
    // their vtables; C++ has full RTTI. Instrumentation-only field.
    pub behavior_type_name: &'static str,
}
```

- [ ] **Step 4: Refactor panel creation to be generic in the behavior type**

Identify the primary creation function from Task 2 audit. If its signature is `fn create_panel(&mut self, behavior: Box<dyn PanelBehavior>, ...) -> PanelId`, change to:

```rust
pub fn create_panel<B: PanelBehavior + 'static>(
    &mut self,
    behavior: B,
    /* other args unchanged */
) -> PanelId {
    let type_name = std::any::type_name::<B>();
    self.create_panel_dyn(Box::new(behavior), type_name, /* ... */)
}

pub fn create_panel_dyn(
    &mut self,
    behavior: Box<dyn PanelBehavior>,
    behavior_type_name: &'static str,
    /* other args unchanged */
) -> PanelId {
    /* existing body, plus store behavior_type_name in PanelData */
}

pub fn behavior_type_name(&self, id: PanelId) -> Option<&'static str> {
    self.panels.get(id).map(|p| p.behavior_type_name)
}
```

- [ ] **Step 5: Update all panel-creation call sites (per Task 2 audit)**

Mechanical: change `Box::new(MyBehavior { ... })` → `MyBehavior { ... }` at concrete sites. For pre-erased sites, route through `create_panel_dyn` with explicit type_name.

- [ ] **Step 6: Run tests**

```bash
cargo test -p emcore --test panel_behavior_type_name 2>&1 | tail -10
cargo check --workspace 2>&1 | tail -30
cargo clippy --workspace -- -D warnings 2>&1 | tail -10
cargo-nextest ntr 2>&1 | tail -20
```
Expected: type_name test passes; workspace builds clean; clippy clean; nextest passes.

- [ ] **Step 7: Defer commit (bundled at end of Phase 5).**

---

## Phase 3 — Cycle/Wake instrumentation

### Task 6: `STAYAWAKE` log line in `DoTimeSlice`

**Files:**
- Modify: `crates/emcore/src/emScheduler.rs` (around line 760, after the match-on-scope `Cycle` call)

**Goal:** Emit one `STAYAWAKE` line per `Cycle` call return.

- [ ] **Step 1: Add the log emission after `Cycle` returns**

Locate the line in `DoTimeSlice` where `let stay_awake = match scope { ... }` resolves (around line 760 on the instr branch). Immediately after, before the `cycled.set(...)` increment, insert:

```rust
// Phase A 7-LOOP-CHAIN: STAYAWAKE per Cycle return.
{
    let type_name = self
        .inner
        .engines
        .get(engine_id)
        .map(|e| e.type_name)
        .unwrap_or("<removed>");
    let line = format!(
        "STAYAWAKE|wall_us={}|slice={}|engine_id={:?}|engine_type={}|stay_awake={}\n",
        crate::emInstr::wall_us(),
        self.inner.time_slice_counter,
        engine_id,
        type_name,
        if stay_awake { "t" } else { "f" },
    );
    crate::emInstr::write_line(&line);
}
```

- [ ] **Step 2: Build and run a quick smoke test**

```bash
cargo build -p eaglemode --release 2>&1 | tail -10
```
Expected: clean build.

Run a 5-second smoke capture:

```bash
EM_INSTR_FD=1 cargo run -p eaglemode --release 2>/dev/null &
PID=$!
sleep 1
kill -USR1 $PID
sleep 3
kill -USR1 $PID
sleep 1
kill $PID
```

Inspect the captured stdout (will need to redirect `EM_INSTR_FD` to a file in the actual capture, but for smoke test stdout is fine if visible). Expected: at least one line beginning with `STAYAWAKE|`.

- [ ] **Step 3: Defer commit (bundled at end of Phase 5).**

---

### Task 7: `WAKE` log line + `#[track_caller]` on `wake_up_engine`

**Files:**
- Modify: `crates/emcore/src/emScheduler.rs` (around line 91, `wake_up_engine`)
- Modify: `crates/emcore/src/emScheduler.rs` (around line 378, `wake_up`)
- Modify: any other panel/engine wake helpers that delegate (e.g., `wake_up_panel`)

**Goal:** Capture original caller (file:line) for each wake_up_engine call. Emit `WAKE` line.

- [ ] **Step 1: Add `#[track_caller]` to `wake_up_engine` and emit the line**

Locate `pub(crate) fn wake_up_engine(&mut self, id: EngineId)` (around line 91). Replace with:

```rust
#[track_caller]
pub(crate) fn wake_up_engine(&mut self, id: EngineId) {
    // Phase A 7-LOOP-CHAIN: WAKE per wake_up_engine call.
    let caller = std::panic::Location::caller();
    let type_name = self
        .engines
        .get(id)
        .map(|e| e.type_name)
        .unwrap_or("<unregistered>");
    let line = format!(
        "WAKE|wall_us={}|engine_id={:?}|engine_type={}|caller={}:{}\n",
        crate::emInstr::wall_us(),
        id,
        type_name,
        caller.file(),
        caller.line(),
    );
    crate::emInstr::write_line(&line);

    // ... existing body unchanged ...
}
```

(The `slice` field is omitted from the WAKE line because `wake_up_engine` is on the inner scheduler that doesn't have direct access to `time_slice_counter`. The analyzer correlates by `wall_us` instead. This is a deliberate simplification.)

- [ ] **Step 2: Add `#[track_caller]` to all delegate wrappers**

Find every wrapper that calls `wake_up_engine` (e.g., `wake_up`, `wake_up_panel`, etc.):

```bash
git grep -n "wake_up_engine\|wake_up\b" -- 'crates/emcore/src/'
```

For each wrapper, add `#[track_caller]` on the wrapper's `fn` line. This propagates the caller location through.

- [ ] **Step 3: Smoke test**

```bash
cargo build -p eaglemode --release 2>&1 | tail -5
```
Expected: clean.

- [ ] **Step 4: Defer commit (bundled at end of Phase 5).**

---

## Phase 4 — Notice instrumentation

### Task 8: `NOTICE` log line at delivery site

**Files:**
- Modify: `crates/emcore/src/emView.rs:4187` (or any other production delivery sites identified by Task 3)

**Goal:** Emit `NOTICE` line for every Notice delivered to a panel behavior.

- [ ] **Step 1: Add the log emission immediately before `behavior.notice(...)`**

In `emView.rs`, find the line `behavior.notice(flags, &state, &mut ctx);` (around line 4187). Insert above it:

```rust
// Phase A 7-LOOP-CHAIN: NOTICE per delivered Notice.
{
    let recipient_type = tree.behavior_type_name(id).unwrap_or("<unknown>");
    let line = format!(
        "NOTICE|wall_us={}|recipient_panel_id={:?}|recipient_type={}|flags={:#x}\n",
        crate::emInstr::wall_us(),
        id,
        recipient_type,
        flags.bits(),
    );
    crate::emInstr::write_line(&line);
}
behavior.notice(flags, &state, &mut ctx);
```

(`flags.bits()` returns the underlying `u32`; the analyzer parses the hex form and decodes flag names. `slice` is omitted because the delivery path does not have direct access to the scheduler's slice counter; analyzer correlates by `wall_us`.)

- [ ] **Step 2: If Task 3 audit found additional production delivery points, instrument each**

Use the same pattern at each. If only one site exists (the expected case), this step is a no-op.

- [ ] **Step 3: Smoke test**

```bash
cargo build -p eaglemode --release 2>&1 | tail -5
```
Expected: clean.

- [ ] **Step 4: Defer commit (bundled at end of Phase 5).**

---

## Phase 5 — Blink-path instrumentation

### Task 9: `BLINK_CYCLE` line in `TextFieldPanel::Cycle`

**Files:**
- Modify: `crates/emcore/src/emColorFieldFieldPanel.rs` (TextFieldPanel::Cycle implementation, added by the prior fix)

**Goal:** Emit `BLINK_CYCLE` after `cycle_blink` returns.

- [ ] **Step 1: Add the log emission**

Open `emColorFieldFieldPanel.rs`. Find `TextFieldPanel`'s `Cycle` method (added by commit `044408b3`). The body looks roughly like:

```rust
fn Cycle(&mut self, /* args */) -> bool {
    let r = self.text_field.cycle_blink(self.is_focused);
    if r.flipped {
        ctx.request_invalidate_self();
    }
    r.busy
}
```

Insert log emission after `cycle_blink` returns:

```rust
fn Cycle(&mut self, /* args */) -> bool {
    let r = self.text_field.cycle_blink(self.is_focused);
    {
        let line = format!(
            "BLINK_CYCLE|wall_us={}|engine_id={:?}|panel_id={:?}|focused={}|flipped={}|busy={}\n",
            crate::emInstr::wall_us(),
            ctx.engine_id(),  // or however engine_id is reachable from ctx
            ctx.self_panel_id(),
            if self.is_focused { "t" } else { "f" },
            if r.flipped { "t" } else { "f" },
            if r.busy { "t" } else { "f" },
        );
        crate::emInstr::write_line(&line);
    }
    if r.flipped {
        ctx.request_invalidate_self();
    }
    r.busy
}
```

(`ctx.engine_id()` and `ctx.self_panel_id()` are placeholders; use the actual accessor names from `PanelCtx` / `EngineCtx`. The implementer reads `crates/emcore/src/emEngineCtx.rs` to find them — likely `ctx.engine_id` direct field access or a getter.)

- [ ] **Step 2: Build**

```bash
cargo build -p eaglemode --release 2>&1 | tail -5
```
Expected: clean. If `engine_id`/`self_panel_id` accessors don't exist on PanelCtx, expose them as getters at this point.

- [ ] **Step 3: Defer commit (bundled at end of Phase 5).**

---

### Task 10: `INVAL_REQ` line in `PanelCtx::request_invalidate_self`

**Files:**
- Modify: `crates/emcore/src/emEngineCtx.rs` (`request_invalidate_self`, added by the prior fix)

**Goal:** Emit `INVAL_REQ` per request_invalidate_self call. Capture caller via `#[track_caller]`.

- [ ] **Step 1: Add `#[track_caller]` and emit log**

Locate `pub fn request_invalidate_self(&mut self)` in `emEngineCtx.rs`. Modify:

```rust
#[track_caller]
pub fn request_invalidate_self(&mut self) {
    // Phase A 7-LOOP-CHAIN: INVAL_REQ per request.
    let caller = std::panic::Location::caller();
    let line = format!(
        "INVAL_REQ|wall_us={}|engine_id={:?}|panel_id={:?}|source={}:{}\n",
        crate::emInstr::wall_us(),
        self.engine_id,  // or however engine_id is stored on PanelCtx
        self.self_id,    // or however panel_id is stored
        caller.file(),
        caller.line(),
    );
    crate::emInstr::write_line(&line);

    // ... existing body unchanged ...
    self.invalidate_self_requested = true;
}
```

- [ ] **Step 2: Build**

```bash
cargo build -p eaglemode --release 2>&1 | tail -5
```
Expected: clean.

- [ ] **Step 3: Defer commit.**

---

### Task 11: `INVAL_DRAIN` line in `PanelCycleEngine` drain check

**Files:**
- Modify: `crates/emcore/src/emPanelCycleEngine.rs`

**Goal:** Emit `INVAL_DRAIN` per drain check (added by the prior fix). Reports whether the drain found a pending request.

- [ ] **Step 1: Locate the drain blocks**

Find the two drain blocks added by the prior fix (one in the Toplevel arm, one in the SubView arm of `PanelCycleEngine::Cycle`). They look roughly like:

```rust
let inval_requested = ctx.take_invalidate_self_request();
if inval_requested {
    // ... resolve view + invalidate ...
}
```

- [ ] **Step 2: Add log emission at each drain block**

Replace each block with:

```rust
let inval_requested = ctx.take_invalidate_self_request();
{
    let line = format!(
        "INVAL_DRAIN|wall_us={}|engine_id={:?}|panel_id={:?}|drained={}\n",
        crate::emInstr::wall_us(),
        engine_id,  // or accessor
        panel_id,   // or accessor
        if inval_requested { "t" } else { "f" },
    );
    crate::emInstr::write_line(&line);
}
if inval_requested {
    // ... resolve view + invalidate ...
}
```

- [ ] **Step 3: Build**

```bash
cargo build -p eaglemode --release 2>&1 | tail -5
```
Expected: clean.

- [ ] **Step 4: Now also add `REGISTER` line emission**

This is the last source-side line. Locate `register_engine_dyn` (where Task 4 left it). Add at end of body, before returning `id`:

```rust
{
    let scope_str = match scope {
        PanelScope::Framework => "Framework".to_string(),
        PanelScope::Toplevel(wid) => format!("Toplevel({:?})", wid),
        PanelScope::SubView { window_id, outer_panel_id } => {
            format!("SubView({:?},{:?})", window_id, outer_panel_id)
        }
    };
    let line = format!(
        "REGISTER|wall_us={}|engine_id={:?}|engine_type={}|scope={}\n",
        crate::emInstr::wall_us(),
        id,
        type_name,
        scope_str,
    );
    crate::emInstr::write_line(&line);
}
id
```

- [ ] **Step 5: Final source-side sanity build**

```bash
cargo build -p eaglemode --release 2>&1 | tail -5
cargo clippy --workspace -- -D warnings 2>&1 | tail -10
cargo-nextest ntr 2>&1 | tail -20
```
Expected: clean clippy, all tests pass. (Note: nextest may fail if the type_name capture broke a downstream test — fix and re-run.)

- [ ] **Step 6: Commit all source-side instrumentation as one commit**

```bash
git add crates/emcore/src/emScheduler.rs \
  crates/emcore/src/emEngineCtx.rs \
  crates/emcore/src/emPanelTree.rs \
  crates/emcore/src/emPanel.rs \
  crates/emcore/src/emView.rs \
  crates/emcore/src/emColorFieldFieldPanel.rs \
  crates/emcore/src/emPanelCycleEngine.rs \
  crates/emcore/tests/unit/engine_type_name.rs \
  crates/emcore/tests/unit/panel_behavior_type_name.rs
# Also add any other call-site files touched by Task 4/5 migrations:
git status --short | grep '^M' | awk '{print $2}'  # review and add
```

```bash
git commit -m "$(cat <<'EOF'
instr: phase A 7-LOOP-CHAIN — engine wake observability (REGISTER/STAYAWAKE/WAKE/NOTICE/BLINK_CYCLE/INVAL_REQ/INVAL_DRAIN)

Adds seven instrumentation line types on top of existing
wall_us/SLICE/CB/AW/RENDER/MARKER infrastructure. Captures concrete
engine and panel-behavior type names at registration via generic
register_engine<E> + register_engine_dyn parallel (and similarly for
panel creation). #[track_caller] on wake_up_engine and
request_invalidate_self captures originating call sites.

No behavior change. Logs only. Branch never merges to main.
EOF
)"
```

Expected: pre-commit hook passes (cargo fmt auto-applied, clippy clean, nextest passes), commit lands.

---

## Phase 6 — Analyzer

### Task 12: Parsers for new line types

**Files:**
- Modify: `scripts/analyze_hang.py`
- Create: `scripts/test_analyze_hang.py`

**Goal:** Parse `REGISTER`, `STAYAWAKE`, `WAKE`, `NOTICE`, `BLINK_CYCLE`, `INVAL_REQ`, `INVAL_DRAIN` lines into structured records.

- [ ] **Step 1: Write failing tests**

Create `scripts/test_analyze_hang.py`:

```python
"""Unit tests for analyze_hang.py extensions."""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze_hang import (
    parse_register, parse_stayawake, parse_wake,
    parse_notice, parse_blink_cycle, parse_inval_req, parse_inval_drain,
)

def test_parse_register():
    line = "REGISTER|wall_us=12345|engine_id=EngineId(7v3)|engine_type=emcore::FooEngine|scope=Toplevel(WindowId(1))"
    r = parse_register(line)
    assert r["wall_us"] == 12345
    assert r["engine_id"] == "EngineId(7v3)"
    assert r["engine_type"] == "emcore::FooEngine"
    assert r["scope"] == "Toplevel(WindowId(1))"

def test_parse_stayawake():
    line = "STAYAWAKE|wall_us=200|slice=42|engine_id=EngineId(7v3)|engine_type=emcore::FooEngine|stay_awake=t"
    r = parse_stayawake(line)
    assert r["wall_us"] == 200
    assert r["slice"] == 42
    assert r["stay_awake"] is True

def test_parse_wake():
    line = "WAKE|wall_us=300|engine_id=EngineId(7v3)|engine_type=emcore::FooEngine|caller=src/foo.rs:42"
    r = parse_wake(line)
    assert r["caller"] == "src/foo.rs:42"

def test_parse_notice():
    line = "NOTICE|wall_us=400|recipient_panel_id=PanelId(2v1)|recipient_type=emcore::TextFieldPanel|flags=0x4"
    r = parse_notice(line)
    assert r["flags"] == 0x4
    assert r["recipient_type"] == "emcore::TextFieldPanel"

def test_parse_blink_cycle():
    line = "BLINK_CYCLE|wall_us=500|engine_id=EngineId(9v2)|panel_id=PanelId(2v1)|focused=t|flipped=t|busy=t"
    r = parse_blink_cycle(line)
    assert r["focused"] is True
    assert r["flipped"] is True
    assert r["busy"] is True

def test_parse_inval_req():
    line = "INVAL_REQ|wall_us=600|engine_id=EngineId(9v2)|panel_id=PanelId(2v1)|source=src/textfield.rs:100"
    r = parse_inval_req(line)
    assert r["source"] == "src/textfield.rs:100"

def test_parse_inval_drain():
    line = "INVAL_DRAIN|wall_us=700|engine_id=EngineId(9v2)|panel_id=PanelId(2v1)|drained=f"
    r = parse_inval_drain(line)
    assert r["drained"] is False
```

- [ ] **Step 2: Run tests, verify they fail**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py -v 2>&1 | tail -10
```
Expected: ImportError or AttributeError (parsers not defined).

- [ ] **Step 3: Implement parsers in `scripts/analyze_hang.py`**

Add to `scripts/analyze_hang.py`:

```python
def _parse_kv_line(line, expected_prefix):
    """Parse a |-separated key=value line. Returns dict of fields."""
    line = line.rstrip("\n")
    parts = line.split("|")
    if not parts or parts[0] != expected_prefix:
        raise ValueError(f"expected {expected_prefix} prefix, got: {line[:80]}")
    out = {}
    for kv in parts[1:]:
        if "=" not in kv:
            continue
        k, _, v = kv.partition("=")
        out[k] = v
    return out

def _to_int(s):
    return int(s, 0)  # handles 0x prefix and decimal

def _to_bool_tf(s):
    return s == "t"

def parse_register(line):
    f = _parse_kv_line(line, "REGISTER")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "engine_type": f["engine_type"],
        "scope": f["scope"],
    }

def parse_stayawake(line):
    f = _parse_kv_line(line, "STAYAWAKE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "slice": _to_int(f["slice"]),
        "engine_id": f["engine_id"],
        "engine_type": f["engine_type"],
        "stay_awake": _to_bool_tf(f["stay_awake"]),
    }

def parse_wake(line):
    f = _parse_kv_line(line, "WAKE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "engine_type": f["engine_type"],
        "caller": f["caller"],
    }

def parse_notice(line):
    f = _parse_kv_line(line, "NOTICE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "recipient_panel_id": f["recipient_panel_id"],
        "recipient_type": f["recipient_type"],
        "flags": _to_int(f["flags"]),
    }

def parse_blink_cycle(line):
    f = _parse_kv_line(line, "BLINK_CYCLE")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "panel_id": f["panel_id"],
        "focused": _to_bool_tf(f["focused"]),
        "flipped": _to_bool_tf(f["flipped"]),
        "busy": _to_bool_tf(f["busy"]),
    }

def parse_inval_req(line):
    f = _parse_kv_line(line, "INVAL_REQ")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "panel_id": f["panel_id"],
        "source": f["source"],
    }

def parse_inval_drain(line):
    f = _parse_kv_line(line, "INVAL_DRAIN")
    return {
        "wall_us": _to_int(f["wall_us"]),
        "engine_id": f["engine_id"],
        "panel_id": f["panel_id"],
        "drained": _to_bool_tf(f["drained"]),
    }
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py -v 2>&1 | tail -15
```
Expected: 7 passed.

- [ ] **Step 5: Defer commit (bundled with rest of analyzer in Task 15).**

---

### Task 13: `idle` command

**Files:**
- Modify: `scripts/analyze_hang.py`
- Modify: `scripts/test_analyze_hang.py`

**Goal:** Aggregate per-engine-type stats from a captured log; classify; produce Markdown report.

- [ ] **Step 1: Write failing tests**

Append to `scripts/test_analyze_hang.py`:

```python
def test_idle_aggregation_self_perpetuating():
    # Synthetic log: engine X cycles 10 times, all stay_awake=t.
    log_lines = [
        "MARKER|wall_us=100|sig=USR1\n",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework\n",
    ]
    for i in range(10):
        log_lines.append(
            f"STAYAWAKE|wall_us={200+i*10}|slice={i}|engine_id=EngineId(1v1)|engine_type=test::Foo|stay_awake=t\n"
        )
    log_lines.append("MARKER|wall_us=400|sig=USR1\n")
    log = "".join(log_lines)
    from analyze_hang import idle_command_text
    out = idle_command_text(log, threshold=0.8)
    assert "test::Foo" in out
    assert "self-perpetuating" in out.lower()

def test_idle_aggregation_externally_rewoken():
    # Engine X: 10 cycles, all stay_awake=f, but a WAKE precedes each.
    log_lines = [
        "MARKER|wall_us=100|sig=USR1\n",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Bar|scope=Framework\n",
    ]
    for i in range(10):
        log_lines.append(
            f"WAKE|wall_us={150+i*10}|engine_id=EngineId(1v1)|engine_type=test::Bar|caller=src/x.rs:1\n"
        )
        log_lines.append(
            f"STAYAWAKE|wall_us={200+i*10}|slice={i}|engine_id=EngineId(1v1)|engine_type=test::Bar|stay_awake=f\n"
        )
    log_lines.append("MARKER|wall_us=400|sig=USR1\n")
    log = "".join(log_lines)
    from analyze_hang import idle_command_text
    out = idle_command_text(log, threshold=0.8)
    assert "externally-rewoken" in out.lower()
    assert "src/x.rs:1" in out  # caller breakdown present

def test_idle_aggregation_episodic():
    # Engine: 50% stay_awake=t. Below 80% threshold = episodic.
    log_lines = [
        "MARKER|wall_us=100|sig=USR1\n",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Mid|scope=Framework\n",
    ]
    for i in range(10):
        sa = "t" if i % 2 == 0 else "f"
        log_lines.append(
            f"STAYAWAKE|wall_us={200+i*10}|slice={i}|engine_id=EngineId(1v1)|engine_type=test::Mid|stay_awake={sa}\n"
        )
    log_lines.append("MARKER|wall_us=400|sig=USR1\n")
    log = "".join(log_lines)
    from analyze_hang import idle_command_text
    out = idle_command_text(log, threshold=0.8)
    assert "episodic" in out.lower()
```

- [ ] **Step 2: Run tests, verify they fail**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py::test_idle_aggregation_self_perpetuating -v 2>&1 | tail -5
```
Expected: ImportError on `idle_command_text`.

- [ ] **Step 3: Implement `idle_command_text`**

Append to `scripts/analyze_hang.py`:

```python
from collections import defaultdict

def idle_command_text(log_content, threshold=0.8):
    """Produce Markdown idle aggregation report from raw log content."""
    # Parse markers; extract bracketed window
    markers = []
    register_records = []
    stayawake_records = []
    wake_records = []
    for ln in log_content.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("MARKER|"):
            f = _parse_kv_line(ln, "MARKER")
            markers.append(_to_int(f["wall_us"]))
        elif ln.startswith("REGISTER|"):
            register_records.append(parse_register(ln))
        elif ln.startswith("STAYAWAKE|"):
            stayawake_records.append(parse_stayawake(ln))
        elif ln.startswith("WAKE|"):
            wake_records.append(parse_wake(ln))

    if len(markers) != 2:
        return f"capture invalid: expected 2 MARKER lines, got {len(markers)}\n"
    t_open, t_close = sorted(markers)
    in_window = lambda r: t_open <= r["wall_us"] <= t_close

    # Filter to in-window
    sa = [r for r in stayawake_records if in_window(r)]
    wk = [r for r in wake_records if in_window(r)]

    # engine_id -> type_name (from register; falls back to type seen in stayawake)
    eid_to_type = {r["engine_id"]: r["engine_type"] for r in register_records}
    for r in sa:
        eid_to_type.setdefault(r["engine_id"], r["engine_type"])

    # Per-engine-type aggregation
    by_type = defaultdict(lambda: {"cycles": 0, "stay_awake_t": 0, "ext_wakes": 0, "callers": defaultdict(int)})
    # cycles + stay_awake_t
    for r in sa:
        b = by_type[r["engine_type"]]
        b["cycles"] += 1
        if r["stay_awake"]:
            b["stay_awake_t"] += 1
    # ext_wakes: a WAKE for an engine whose most recent prior STAYAWAKE returned f
    sa_by_eid = defaultdict(list)
    for r in sa:
        sa_by_eid[r["engine_id"]].append(r)
    for r in wk:
        history = sa_by_eid.get(r["engine_id"], [])
        prior = [s for s in history if s["wall_us"] < r["wall_us"]]
        if prior and not prior[-1]["stay_awake"]:
            t = eid_to_type.get(r["engine_id"], "<unknown>")
            by_type[t]["ext_wakes"] += 1
            by_type[t]["callers"][r["caller"]] += 1

    # Slice count: number of distinct slice values in the window
    slice_count = len({r["slice"] for r in sa})

    # Build classification per type
    def classify(b):
        if b["cycles"] == 0:
            return "never-awake"
        sa_pct = b["stay_awake_t"] / b["cycles"] if b["cycles"] else 0.0
        if sa_pct >= threshold:
            return "self-perpetuating"
        if slice_count and b["ext_wakes"] >= threshold * slice_count:
            return "externally-rewoken"
        return "episodic"

    rows = []
    for t, b in sorted(by_type.items(), key=lambda kv: -kv[1]["stay_awake_t"]):
        sa_pct = (b["stay_awake_t"] / b["cycles"]) if b["cycles"] else 0.0
        rows.append({
            "type": t, "cycles": b["cycles"], "sa_pct": sa_pct,
            "ext_wakes": b["ext_wakes"], "classification": classify(b),
            "callers": dict(b["callers"]),
        })

    # Format report
    out = []
    out.append(f"## Window")
    out.append(f"{slice_count} slices, {(t_close - t_open) / 1_000_000:.2f}s\n")
    out.append("## Per-engine-type aggregation\n")
    out.append("| engine_type | cycles | stay_awake_pct | ext_wakes | classification |")
    out.append("|---|---:|---:|---:|---|")
    for r in rows:
        out.append(f"| `{r['type']}` | {r['cycles']} | {r['sa_pct']*100:.1f}% | {r['ext_wakes']} | {r['classification']} |")
    out.append("")

    offenders = [r for r in rows if r["classification"] in ("self-perpetuating", "externally-rewoken")]
    out.append("## Offenders")
    if not offenders:
        out.append(f"_None at threshold={threshold*100:.0f}%._")
    else:
        for r in offenders:
            out.append(f"- `{r['type']}` — {r['classification']} (cycles={r['cycles']}, stay_awake={r['sa_pct']*100:.1f}%, ext_wakes={r['ext_wakes']})")
    out.append("")

    out.append("## External-wake caller breakdown")
    any_ext = False
    for r in offenders:
        if r["callers"]:
            any_ext = True
            out.append(f"### `{r['type']}`")
            for caller, n in sorted(r["callers"].items(), key=lambda kv: -kv[1]):
                out.append(f"- `{caller}` — count={n}")
    if not any_ext:
        out.append("_None._")
    out.append("")

    out.append(f"_Next step: spec B1 — compare {{offenders}} to C++ ground truth._\n")
    return "\n".join(out)
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py::test_idle_aggregation_self_perpetuating test_analyze_hang.py::test_idle_aggregation_externally_rewoken test_analyze_hang.py::test_idle_aggregation_episodic -v 2>&1 | tail -10
```
Expected: 3 passed.

- [ ] **Step 5: Wire `idle` to the script's argparse**

Add to `analyze_hang.py`'s `main()` argparse:

```python
sp_idle = subparsers.add_parser("idle", help="A1 has_awake findings from idle capture")
sp_idle.add_argument("log", help="path to /tmp/em_instr.idle.log")
sp_idle.add_argument("--threshold", type=float, default=0.8)
```

And in the dispatch section:

```python
elif args.cmd == "idle":
    with open(args.log) as f:
        content = f.read()
    print(idle_command_text(content, threshold=args.threshold))
```

- [ ] **Step 6: Defer commit.**

---

### Task 14: `blink` command

**Files:**
- Modify: `scripts/analyze_hang.py`
- Modify: `scripts/test_analyze_hang.py`

**Goal:** Path-trace from focus-change moment through the cycle/invalidate/render chain. Identify first ✗ link.

- [ ] **Step 1: Write failing test**

Append to `scripts/test_analyze_hang.py`:

```python
def test_blink_path_trace_breaks_at_wake():
    # Synthetic: NOTICE FOCUS_CHANGED fires, but no WAKE follows.
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|scope=Toplevel(WindowId(1))",
        "NOTICE|wall_us=200|recipient_panel_id=PanelId(2v1)|recipient_type=emcore::TextFieldPanel|flags=0x4",
        # No WAKE follows the FOCUS_CHANGED.
        "MARKER|wall_us=10000|sig=USR1",
        "",
    ])
    from analyze_hang import blink_command_text
    out = blink_command_text(log, focus_changed_bit=0x4)
    assert "FOCUS_CHANGED" in out
    assert "✓" in out  # NOTICE ✓
    assert "✗" in out  # first break

def test_blink_path_trace_complete_chain():
    # Synthetic: full chain fires.
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|scope=Toplevel(WindowId(1))",
        "NOTICE|wall_us=200|recipient_panel_id=PanelId(2v1)|recipient_type=emcore::TextFieldPanel|flags=0x4",
        "WAKE|wall_us=210|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|caller=src/textfield.rs:1",
        "STAYAWAKE|wall_us=300|slice=1|engine_id=EngineId(7v3)|engine_type=emcore::PanelCycleEngine|stay_awake=t",
        "BLINK_CYCLE|wall_us=310|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|focused=t|flipped=f|busy=t",
        "BLINK_CYCLE|wall_us=810|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|focused=t|flipped=t|busy=t",
        "INVAL_REQ|wall_us=811|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|source=src/textfield.rs:50",
        "INVAL_DRAIN|wall_us=812|engine_id=EngineId(7v3)|panel_id=PanelId(2v1)|drained=t",
        "MARKER|wall_us=10000|sig=USR1",
        "",
    ])
    from analyze_hang import blink_command_text
    out = blink_command_text(log, focus_changed_bit=0x4)
    # All ✓ in path-trace section before "Identified break"
    # Test does not assert exact format; asserts presence of key markers
    assert "BLINK_CYCLE" in out
    assert "INVAL_DRAIN" in out
    # Should NOT find a break before INVAL_DRAIN
    # (We cross-check by ensuring "first ✗" or "no break" wording present)
    assert ("no break" in out.lower()) or ("contingency" in out.lower())
```

- [ ] **Step 2: Run tests, verify they fail**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py::test_blink_path_trace_breaks_at_wake -v 2>&1 | tail -5
```
Expected: ImportError on `blink_command_text`.

- [ ] **Step 3: Implement `blink_command_text`**

Append to `scripts/analyze_hang.py`. The implementation parses all line types, identifies the focus-change moment (first NOTICE with FOCUS_CHANGED bit set whose recipient_type ends with "TextFieldPanel"), then walks the chain in order:

```python
def blink_command_text(log_content, focus_changed_bit=0x4):
    """Produce Markdown blink path-trace report."""
    markers = []
    notices = []
    wakes = []
    stays = []
    blinks = []
    invreqs = []
    drains = []
    registers = []
    for ln in log_content.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        try:
            if ln.startswith("MARKER|"):
                markers.append(_to_int(_parse_kv_line(ln, "MARKER")["wall_us"]))
            elif ln.startswith("NOTICE|"):
                notices.append(parse_notice(ln))
            elif ln.startswith("WAKE|"):
                wakes.append(parse_wake(ln))
            elif ln.startswith("STAYAWAKE|"):
                stays.append(parse_stayawake(ln))
            elif ln.startswith("BLINK_CYCLE|"):
                blinks.append(parse_blink_cycle(ln))
            elif ln.startswith("INVAL_REQ|"):
                invreqs.append(parse_inval_req(ln))
            elif ln.startswith("INVAL_DRAIN|"):
                drains.append(parse_inval_drain(ln))
            elif ln.startswith("REGISTER|"):
                registers.append(parse_register(ln))
        except (ValueError, KeyError):
            pass  # malformed line, skip

    if len(markers) != 2:
        return f"capture invalid: expected 2 MARKER lines, got {len(markers)}\n"
    t_open, t_close = sorted(markers)

    # Locate focus-change
    focus_notices = [
        n for n in notices
        if t_open <= n["wall_us"] <= t_close
        and (n["flags"] & focus_changed_bit)
        and "TextFieldPanel" in n["recipient_type"]
    ]
    if not focus_notices:
        return "capture invalid: no NOTICE FOCUS_CHANGED to TextFieldPanel within window\n"
    focus = focus_notices[0]
    t_focus = focus["wall_us"]
    target_panel_id = focus["recipient_panel_id"]

    # Find the engine for this panel: latest REGISTER for a PanelCycleEngine whose scope mentions target_panel_id
    target_engine_id = None
    for r in registers:
        if "PanelCycleEngine" in r["engine_type"] and target_panel_id in r["scope"]:
            target_engine_id = r["engine_id"]
            break
    # Fallback: pick by latest BLINK_CYCLE for the panel
    if not target_engine_id:
        post_focus_blinks = [b for b in blinks if b["wall_us"] >= t_focus and b["panel_id"] == target_panel_id]
        if post_focus_blinks:
            target_engine_id = post_focus_blinks[0]["engine_id"]

    # Path-trace verdict
    out = []
    out.append("## Path-trace verdict (transition)\n")
    out.append(f"Focus-change identified at +{(t_focus - t_open)/1000:.1f}ms (`{target_panel_id}`, `{focus['recipient_type']}`).\n")

    chain = []  # list of (label, ok, evidence)

    chain.append(("NOTICE FOCUS_CHANGED → TextFieldPanel", True,
                  f"`NOTICE|wall_us={focus['wall_us']}|recipient_panel_id={focus['recipient_panel_id']}|flags={focus['flags']:#x}`"))

    if target_engine_id is None:
        chain.append(("Engine REGISTER for PanelCycleEngine", False, "no REGISTER record matches target panel"))
    else:
        post_wake = [w for w in wakes if w["wall_us"] >= t_focus and w["engine_id"] == target_engine_id]
        chain.append(("WAKE → PanelCycleEngine", bool(post_wake),
                      f"`{post_wake[0]['caller']}` at +{(post_wake[0]['wall_us']-t_focus)/1000:.1f}ms" if post_wake else "no WAKE within window"))
        if post_wake:
            t_wake = post_wake[0]["wall_us"]
            post_stay = [s for s in stays if s["wall_us"] >= t_wake and s["engine_id"] == target_engine_id]
            chain.append(("STAYAWAKE within 1 slice of WAKE", bool(post_stay),
                          f"slice={post_stay[0]['slice']}, stay_awake={post_stay[0]['stay_awake']}" if post_stay else "no STAYAWAKE for engine after WAKE"))

        post_blinks = [b for b in blinks if b["wall_us"] >= t_focus and b["engine_id"] == target_engine_id]
        focused_blinks = [b for b in post_blinks if b["focused"]]
        chain.append(("BLINK_CYCLE focused=true", bool(focused_blinks),
                      f"first focused=t at +{(focused_blinks[0]['wall_us']-t_focus)/1000:.1f}ms" if focused_blinks else "no BLINK_CYCLE focused=t"))
        flipped_blinks = [b for b in post_blinks if b["flipped"]]
        chain.append(("BLINK_CYCLE flipped=true at ~500ms cadence", bool(flipped_blinks),
                      f"{len(flipped_blinks)} flips in window" if flipped_blinks else "no BLINK_CYCLE flipped=t"))

        post_invreq = [i for i in invreqs if i["wall_us"] >= t_focus and i["engine_id"] == target_engine_id]
        chain.append(("INVAL_REQ from cycle_blink", bool(post_invreq),
                      f"first source={post_invreq[0]['source']}" if post_invreq else "no INVAL_REQ"))

        post_drain = [d for d in drains if d["wall_us"] >= t_focus and d["engine_id"] == target_engine_id and d["drained"]]
        chain.append(("INVAL_DRAIN drained=true", bool(post_drain),
                      f"first drain at +{(post_drain[0]['wall_us']-t_focus)/1000:.1f}ms" if post_drain else "no INVAL_DRAIN drained=t"))

    for label, ok, evidence in chain:
        marker = "✓" if ok else "✗"
        out.append(f"- {marker} **{label}** — {evidence}")
    out.append("")

    first_break = next((label for label, ok, _ in chain if not ok), None)
    out.append("## Identified break\n")
    if first_break:
        out.append(f"First ✗: **{first_break}**.\n")
        out.append(f"_Next step: spec B2 — investigate {first_break}._\n")
    else:
        out.append("No break in path-trace. If blink still not visually working, run A2-prod contingency capture.\n")
        out.append("_Next step: A2-prod follow-up capture._\n")

    return "\n".join(out)
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py -v 2>&1 | tail -15
```
Expected: all parser + idle + blink tests pass.

- [ ] **Step 5: Wire `blink` to argparse**

In `analyze_hang.py` main:

```python
sp_blink = subparsers.add_parser("blink", help="A2 path-trace findings from blink capture")
sp_blink.add_argument("log", help="path to /tmp/em_instr.blink.log")
sp_blink.add_argument("--focus-changed-bit", type=lambda s: int(s, 0), default=0x4)

# In dispatch:
elif args.cmd == "blink":
    with open(args.log) as f:
        content = f.read()
    print(blink_command_text(content, focus_changed_bit=args.focus_changed_bit))
```

(`focus_changed_bit` is configurable because the actual bit value of `NoticeFlags::FOCUS_CHANGED` may differ; default `0x4` is a placeholder. Implementer reads `crates/emcore/src/emPanel.rs:206` to get the real bit position and updates the default in this argparse + the test.)

- [ ] **Step 6: Defer commit.**

---

### Task 15: Validation pre-pass

**Files:**
- Modify: `scripts/analyze_hang.py`
- Modify: `scripts/test_analyze_hang.py`

**Goal:** Both `idle` and `blink` validate the capture before producing a report. Fail fast on invalid captures.

- [ ] **Step 1: Write failing tests**

Append to `scripts/test_analyze_hang.py`:

```python
def test_validate_capture_rejects_zero_markers():
    log = "REGISTER|wall_us=10|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework\n"
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="idle")
    assert not ok
    assert "MARKER" in reason

def test_validate_capture_rejects_missing_register():
    log = "MARKER|wall_us=100|sig=USR1\nMARKER|wall_us=200|sig=USR1\n"
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="idle")
    assert not ok
    assert "REGISTER" in reason

def test_validate_capture_blink_requires_focus_changed():
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework",
        "MARKER|wall_us=200|sig=USR1",
        "",
    ])
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="blink")
    assert not ok
    assert "FOCUS_CHANGED" in reason

def test_validate_capture_passes_valid_idle():
    log = "\n".join([
        "MARKER|wall_us=100|sig=USR1",
        "REGISTER|wall_us=50|engine_id=EngineId(1v1)|engine_type=test::Foo|scope=Framework",
        "MARKER|wall_us=200|sig=USR1",
        "",
    ])
    from analyze_hang import validate_capture
    ok, reason = validate_capture(log, kind="idle")
    assert ok
```

- [ ] **Step 2: Run tests, verify they fail**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py::test_validate_capture_rejects_zero_markers -v 2>&1 | tail -5
```
Expected: ImportError on `validate_capture`.

- [ ] **Step 3: Implement `validate_capture`**

Append to `analyze_hang.py`:

```python
def validate_capture(log_content, kind, focus_changed_bit=0x4):
    """Returns (ok, reason). For kind in {"idle", "blink"}."""
    markers = []
    has_register = False
    notices = []
    for ln in log_content.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        if ln.startswith("MARKER|"):
            markers.append(ln)
        elif ln.startswith("REGISTER|"):
            has_register = True
        elif ln.startswith("NOTICE|") and kind == "blink":
            try:
                n = parse_notice(ln)
                notices.append(n)
            except (KeyError, ValueError):
                pass

    if len(markers) != 2:
        return False, f"expected 2 MARKER lines, got {len(markers)}"
    if not has_register:
        return False, "no REGISTER lines (instrumentation did not initialize)"
    if kind == "blink":
        focus_changes = [
            n for n in notices
            if (n["flags"] & focus_changed_bit) and "TextFieldPanel" in n["recipient_type"]
        ]
        if not focus_changes:
            return False, "no NOTICE FOCUS_CHANGED to TextFieldPanel — click did not land on TextField"
    return True, ""
```

- [ ] **Step 4: Wire validation into both commands**

In `idle_command_text` and `blink_command_text`, add as the first action after parsing:

```python
ok, reason = validate_capture(log_content, kind="idle")  # or "blink"
if not ok:
    return f"capture invalid: {reason}\n"
```

- [ ] **Step 5: Run all tests**

```bash
cd scripts && python3 -m pytest test_analyze_hang.py -v 2>&1 | tail -20
```
Expected: all tests pass (parsers, idle, blink, validation).

- [ ] **Step 6: Commit analyzer**

```bash
git add scripts/analyze_hang.py scripts/test_analyze_hang.py
git commit -m "$(cat <<'EOF'
instr: analyze_hang.py — idle and blink commands

Adds parsers for REGISTER/STAYAWAKE/WAKE/NOTICE/BLINK_CYCLE/INVAL_REQ
/INVAL_DRAIN. Two new top-level commands: `idle` produces per-engine-type
aggregation with offender classification at configurable threshold;
`blink` produces path-trace verdict from focus-change moment through
cycle/invalidate/render chain. Both commands run a validation pre-pass
that fails fast on incomplete captures.

Pytest suite covers parsers, aggregation classifications, path-trace
verdicts (broken-chain and complete-chain cases), and validation.
EOF
)"
```

Expected: pre-commit hook passes; commit lands.

---

## Phase 7 — Capture infrastructure

### Task 16: `scripts/run_blink_capture.sh`

**Files:**
- Create: `scripts/run_blink_capture.sh`

**Goal:** Convenience launcher that sets `EM_INSTR_FD`, starts the GUI, and prints click-timing instructions to stdout.

- [ ] **Step 1: Create the script**

```bash
cat > scripts/run_blink_capture.sh <<'EOF'
#!/usr/bin/env bash
# Engine Wake Observability — A2 (blink) capture launcher.
# Sets up EM_INSTR_FD redirection and starts the GUI; user drives markers
# and clicks by hand. See plan: docs/superpowers/plans/2026-05-03-engine-wake-observability.md
set -euo pipefail

LOG=/tmp/em_instr.blink.log
: > "$LOG"

# fd 9 = log file
exec 9>>"$LOG"

cat <<INSTR
=========================================
A2 BLINK CAPTURE — manual procedure

1. After the GUI window appears, navigate to the runtime test panel
   (crates/emtest) that exposes TextField widgets.
2. Position so the chosen TextField is fully visible. Do NOT move the
   mouse after this point until you click the field.
3. Send open marker:
       kill -USR1 \$(pgrep -f 'target/release/eaglemode')
4. Wait ~5 seconds.
5. Single click into the TextField. Then DO NOT TYPE OR MOVE THE MOUSE.
6. Hold for ~60 seconds. Cursor should visibly blink IF the fix is
   working — this is what we are testing.
7. Send close marker:
       kill -USR1 \$(pgrep -f 'target/release/eaglemode')
8. Quit the GUI normally.

Log will be written to: $LOG
=========================================
INSTR

EM_INSTR_FD=9 cargo run -p eaglemode --release
EOF
chmod +x scripts/run_blink_capture.sh
```

- [ ] **Step 2: Lint check**

```bash
shellcheck scripts/run_blink_capture.sh 2>&1 | head -20
```
Expected: clean (or ignorable warnings only).

- [ ] **Step 3: Commit**

```bash
git add scripts/run_blink_capture.sh
git commit -m "$(cat <<'EOF'
instr: scripts/run_blink_capture.sh — manual A2 capture launcher

Sets EM_INSTR_FD=9 redirected to /tmp/em_instr.blink.log, prints
the click + marker procedure to stdout, runs the GUI in release.
Markers and click are driven by the user; the script does not
automate them.
EOF
)"
```

Expected: commit lands.

---

### Task 17: Tag the branch HEAD

**Files:** none (git operation)

**Goal:** Create immutable reference for findings docs to cite.

- [ ] **Step 1: Verify the two new commits are present**

```bash
git log --oneline main..instr/hang-2026-05-02 | head -5
```
Expected: at least two new commits (the source-side instr commit, the analyzer commit, and the capture script commit) on top of the prior 10 instrumentation commits.

- [ ] **Step 2: Build and run validation captures**

Before tagging, validate the instrumentation by running a quick smoke capture:

```bash
cargo build -p eaglemode --release 2>&1 | tail -3
LOG=/tmp/em_instr.smoke.log
: > "$LOG"
exec 9>>"$LOG"
EM_INSTR_FD=9 timeout 5 cargo run -p eaglemode --release 2>/dev/null || true
exec 9>&-
grep -c "REGISTER\|STAYAWAKE" "$LOG"
```
Expected: at least one `REGISTER` line and at least one `STAYAWAKE` line.

- [ ] **Step 3: Tag**

```bash
git tag -a instr-7-loop-chain -m "Engine wake observability instrumentation: REGISTER/STAYAWAKE/WAKE/NOTICE/BLINK_CYCLE/INVAL_REQ/INVAL_DRAIN + analyze_hang.py extensions"
git tag --list 'instr-*'
```
Expected: tag listed.

**No commit.** Tag is the artifact.

---

## Phase 8 — Captures and findings

> **Subagent pause points:** Tasks 18 and 20 require manual GUI interaction. The subagent **pauses**; the orchestrator (Claude) prompts the user to run the capture and provides the captured log path back to the subagent before resumption.

### Task 18: A1 idle capture

**Files:** none (produces `/tmp/em_instr.idle.log`)

**Goal:** Capture a 60s idle window with two SIGUSR1 markers.

- [ ] **Step 1: SUBAGENT PAUSE — request orchestrator-driven capture**

Subagent emits this message (verbatim) and stops:

> **PAUSE — A1 idle capture required.** Orchestrator: please prompt the user to run `bash scripts/run_hang_capture.sh` (or the project-standard idle capture wrapper), follow its instructions for two SIGUSR1 markers bracketing 60s of idle, and return the captured log path. The path is expected to be `/tmp/em_instr.idle.log`.

Orchestrator prompts user, user runs capture, orchestrator confirms log path back to subagent.

- [ ] **Step 2: Validate the capture**

After orchestrator confirms log path:

```bash
python3 scripts/analyze_hang.py idle /tmp/em_instr.idle.log > /tmp/a1_report.md 2>&1
echo "exit=$?"
head -20 /tmp/a1_report.md
```
Expected: exit=0, report begins with `## Window`.

If output starts with `capture invalid:`, return to Step 1 (re-run capture).

**No commit.** Report consumed by Task 19.

---

### Task 19: A1 findings doc → main

**Files:**
- Create: `docs/scratch/<YYYY-MM-DD>-has-awake-findings.md` on `main`

**Goal:** Paste analyzer report into the spec-defined template, fill Verdict + Next-steps, commit to main.

- [ ] **Step 1: Determine the date string and file path**

```bash
DATE=$(date +%Y-%m-%d)
FINDINGS=docs/scratch/${DATE}-has-awake-findings.md
echo "Will create: $FINDINGS"
```

- [ ] **Step 2: Switch to main and create the file**

```bash
git checkout main
```

- [ ] **Step 3: Build the findings doc from the spec template + analyzer report**

```bash
TAG=instr-7-loop-chain
TAG_SHA=$(git rev-parse "$TAG")
THRESHOLD=80
cat > "$FINDINGS" <<EOF
# has_awake idle findings — ${DATE}
Capture: /tmp/em_instr.idle.log
Branch: instr/hang-2026-05-02 @ ${TAG} (${TAG_SHA:0:8})
Threshold: ${THRESHOLD}%

EOF
cat /tmp/a1_report.md >> "$FINDINGS"
cat >> "$FINDINGS" <<'EOF'

## Verdict
<TODO: human-written one paragraph: which engines named, classification, fix candidate vs. wait-for-C++-comparison.>

## Next steps
- [ ] Spec B1 — C++ comparison for {offender list}
- [ ] OR: defer; document rationale
EOF
```

- [ ] **Step 4: Fill the Verdict section by hand**

Open `$FINDINGS` in editor. Replace `<TODO: ...>` with one paragraph describing the offenders identified by the analyzer and the user's call on whether to spec a fix or defer. **The Verdict must be filled before commit.**

- [ ] **Step 5: Verify no remaining TODO**

```bash
grep -c "<TODO" "$FINDINGS"
```
Expected: `0`.

- [ ] **Step 6: Commit to main**

```bash
git add "$FINDINGS"
git commit -m "scratch: A1 has_awake idle findings — engine wake observability capture"
```
Expected: commit lands. Pre-commit hook is docs-only and skips the build gate.

---

### Task 20: A2 blink capture

**Files:** none (produces `/tmp/em_instr.blink.log`)

**Goal:** Capture a ~90s window with focus on a test-panel TextField.

- [ ] **Step 1: Switch back to instrumentation branch**

```bash
git checkout instr/hang-2026-05-02
```

- [ ] **Step 2: SUBAGENT PAUSE — request orchestrator-driven capture**

Subagent emits this message (verbatim) and stops:

> **PAUSE — A2 blink capture required.** Orchestrator: please prompt the user to run `bash scripts/run_blink_capture.sh`, navigate to the runtime test panel TextField, send open marker, click into the TextField, hold focused for 60s without input or mouse motion, send close marker, quit the GUI. Return the captured log path. Expected path: `/tmp/em_instr.blink.log`.

Orchestrator prompts user, user runs capture, orchestrator confirms log path back to subagent.

- [ ] **Step 3: Validate the capture**

```bash
python3 scripts/analyze_hang.py blink /tmp/em_instr.blink.log > /tmp/a2_report.md 2>&1
echo "exit=$?"
head -30 /tmp/a2_report.md
```
Expected: exit=0; report begins with `## Path-trace verdict`.

If `capture invalid: no NOTICE FOCUS_CHANGED ...`, the click did not land on a TextField — return to Step 2.

**No commit.** Report consumed by Task 21.

---

### Task 21: A2 findings doc → main

**Files:**
- Create: `docs/scratch/<YYYY-MM-DD>-blink-findings.md` on `main`

**Goal:** Paste analyzer report into spec template, fill Verdict + Identified-break + Contingency-check + Next-steps, commit to main.

- [ ] **Step 1: Determine date and switch to main**

```bash
DATE=$(date +%Y-%m-%d)
FINDINGS=docs/scratch/${DATE}-blink-findings.md
git checkout main
```

- [ ] **Step 2: Build doc from template + analyzer report**

```bash
TAG=instr-7-loop-chain
TAG_SHA=$(git rev-parse "$TAG")
TEXTFIELD_NAME="<fill in: the visible label of the TextField clicked>"
cat > "$FINDINGS" <<EOF
# Blink path-trace findings — ${DATE}
Capture: /tmp/em_instr.blink.log (test-panel TextField: ${TEXTFIELD_NAME})
Branch: instr/hang-2026-05-02 @ ${TAG} (${TAG_SHA:0:8})

EOF
cat /tmp/a2_report.md >> "$FINDINGS"
cat >> "$FINDINGS" <<'EOF'

## Steady-state aggregation (post-click)
<TODO: paste analyzer's steady-state counts table if not already in
the report; otherwise note "not produced — see path-trace section above">

## Contingency check
<TODO: if every step ✓ but blink not visually working, run A2-prod
capture against default eaglemode binary's first reachable TextField;
re-run analyzer; if results differ → hypothesis #9 confirmed (test/prod
divergence). Document outcome here. If A2 path-trace already shows a
break (any ✗), write "not triggered.">

## Next steps
- [ ] Spec B2 — fix targeted at {broken-link layer}
- [ ] OR: A2-prod follow-up capture
- [ ] OR: defer; document rationale
EOF
```

- [ ] **Step 3: Fill TODO sections by hand**

Open `$FINDINGS`, replace each `<TODO: ...>` with its content per the template guidance. The `Identified break` section may already be filled by the analyzer; if so, add a one-paragraph human interpretation alongside it.

- [ ] **Step 4: Verify no remaining TODO**

```bash
grep -c "<TODO\|<fill" "$FINDINGS"
```
Expected: `0`.

- [ ] **Step 5: Commit**

```bash
git add "$FINDINGS"
git commit -m "scratch: A2 blink path-trace findings — engine wake observability capture"
```

---

### Task 22: A2-prod contingency (CONDITIONAL)

> **This task runs only if Task 21's findings doc reads "every step ✓ in the path-trace, but blink still visually not working in the default binary."** Otherwise skip to Phase 9 (close-out).

**Files:** none new; appends to `docs/scratch/<YYYY-MM-DD>-blink-findings.md`.

**Goal:** Re-run the A2 capture against the default `eaglemode` binary (not test panel) to confirm or refute hypothesis #9.

- [ ] **Step 1: Check whether contingency triggered**

```bash
DATE=$(date +%Y-%m-%d)
grep -A2 "## Identified break" "docs/scratch/${DATE}-blink-findings.md" | head -5
```

If output reads "no break" or "no ✗", continue. Otherwise stop — the contingency is not needed.

- [ ] **Step 2: Switch to instrumentation branch and identify default-binary TextField target**

```bash
git checkout instr/hang-2026-05-02
```

Read the launcher to identify the first TextField reachable in the startup cosmos. Document the panel as the click target for the contingency capture.

- [ ] **Step 3: SUBAGENT PAUSE — request orchestrator-driven contingency capture**

Subagent emits:

> **PAUSE — A2-prod contingency capture required.** Orchestrator: same procedure as A2 capture, but click into the TextField identified by the implementer as the first reachable in the default `eaglemode` binary's startup cosmos. Use a different log path: `/tmp/em_instr.blink.prod.log`. Return path back to subagent.

Orchestrator coordinates with user.

- [ ] **Step 4: Run analyzer on contingency log**

```bash
python3 scripts/analyze_hang.py blink /tmp/em_instr.blink.prod.log > /tmp/a2_prod_report.md
head -30 /tmp/a2_prod_report.md
```

- [ ] **Step 5: Append contingency findings to A2 doc**

Append a `## A2-prod contingency results` section to `docs/scratch/<YYYY-MM-DD>-blink-findings.md` with the contingency analyzer report. Add a verdict paragraph: if contingency shows the same ✗ break as A2 → blink bug spans both. If contingency shows different chain behavior → hypothesis #9 confirmed.

- [ ] **Step 6: Switch to main and commit the appendix**

```bash
git checkout main
git add "docs/scratch/${DATE}-blink-findings.md"
git commit -m "scratch: A2-prod contingency results appended to blink findings"
```

---

## Phase 9 — Close-out

### Task 23: Validate exit conditions

**Files:** none (git verification)

**Goal:** Confirm investigation is done per spec exit conditions.

- [ ] **Step 1: Both findings docs on main**

```bash
git log main --oneline | grep -E "scratch:.*has_awake|scratch:.*blink"
```
Expected: at least two matching commits.

- [ ] **Step 2: No fix code on main**

```bash
git diff main..main -- 'crates/' | head
```
Expected: no diff (we're comparing main to itself, sanity check the working tree is clean).

```bash
git status
```
Expected: clean.

- [ ] **Step 3: Instrumentation not merged to main**

```bash
git log main..instr-7-loop-chain --oneline | head
```
Expected: shows the new instrumentation commits ONLY on the branch (not merged into main).

- [ ] **Step 4: Findings docs have filled Verdict sections**

```bash
DATE=$(date +%Y-%m-%d)
grep -c "<TODO\|<fill\|TBD" "docs/scratch/${DATE}-has-awake-findings.md" "docs/scratch/${DATE}-blink-findings.md"
```
Expected: `0` for both files.

- [ ] **Step 5: Push findings to origin**

```bash
git push origin main
```
Expected: push succeeds.

**No commit.** This task validates the investigation is closed; no further code changes.

---

## Notes for the implementer

- **Pre-commit hook** runs `cargo fmt`, `clippy -D warnings`, and `cargo-nextest ntr` per commit. The pre-commit hook detects docs-only changes and skips the build gate; instrumentation-source commits run the full gate.
- **Type-name format**: `std::any::type_name::<E>()` returns strings like `eaglemode_emcore::emPanelCycleEngine::PanelCycleEngine` — long, but unambiguous. Analyzer matches on suffixes (`endswith`) where appropriate.
- **`#[track_caller]` propagation**: the attribute must be on every wrapper from the original call site down to where `Location::caller()` is invoked. If a wrapper is missing it, the caller resolves to the wrapper's own file:line, not the user's. Verify by inspecting WAKE caller fields after the smoke test — they should point at panel/engine code, not into `emScheduler.rs`.
- **NoticeFlags `FOCUS_CHANGED` bit value**: read from `crates/emcore/src/emPanel.rs:206` (`bitflags` declaration). Update `--focus-changed-bit` default in argparse and in the analyzer test cases to match. Default `0x4` in this plan is a placeholder.
- **Subagent pause/resume protocol**: at every PAUSE point, the subagent must output the pause message verbatim and then stop. The orchestrator's prompt is the subagent's resume signal; the orchestrator passes the captured log path forward in plain text.
