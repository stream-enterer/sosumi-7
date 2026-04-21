# Phase 3.5 — Deferred `emDialog` Construction + Consumer Migration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reshape `emDialog` into a handle that construction builds synchronously through `ConstructCtx` and `show()` enqueues via the closure rail. Replace `GetResult` polling in consumers with `on_finish` + `Rc<Cell>`. Unify cancel-in-flight and auto-delete paths behind `App::close_dialog_by_id`, fixing the latent undrained-enum-rail auto-delete bug incidentally.

**Architecture:** Single `emDialog` type with pre/post-show states — pre-show holds the builder (`Option<PendingTopLevel>`), mutators apply synchronously to the in-handle `DlgPanel`; `show()` consumes the pending entry into a closure pushed onto `App::pending_actions` (`Rc<RefCell<Vec<Box<dyn FnOnce(&mut App, &ActiveEventLoop)>>>>`). `DialogPrivateEngine` is constructed at install time — not at `emDialog::new` — so `window_id: WindowId` (not `Option`). Consumer observation uses `on_finish` closures capturing `Rc<Cell<Option<DialogResult>>>`. `App::close_dialog_by_id(DialogId)` unifies cancel + auto-delete teardown. No new `DeferredAction` enum variants; closure rail only.

**Tech Stack:** Rust 1.82+, slotmap, winit. All work in `crates/emcore/src/`, `crates/emstocks/src/`, and test files. No new external crates.

**Authority:** CLAUDE.md Port Ideology (C++ source > golden tests > Rust idiom > plan). Spec: `docs/superpowers/specs/2026-04-21-phase-3-5-deferred-dialog-construction-design.md` at commit `55b3a76d`.

**Branch:** `port-rewrite/phase-3-5-deferred-dialog-construction` off `port-rewrite/phase-3-5-a-runtime-toplevel-windows` at `586d6af5` (tagged `port-rewrite-phase-3-5-a-complete`). Exit tag: `port-rewrite-phase-3-5-complete`.

**Baseline** (measured at `586d6af5`):
- nextest: 2492 passed / 0 failed / 9 skipped
- goldens: 237 passed / 6 failed (pre-existing)
- clippy: clean
- fmt: clean

**Gate commands** (run at the end of every committed task):
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo-nextest ntr`
- `cargo test --test golden -- --test-threads=1` (golden suite: **Tasks 15 and 22 only** — emStocksListBox and emFileDialog consumer migrations are the sub-tasks that can shift golden output. Pure `emDialog` reshape before them is behaviorally neutral for goldens because no golden test renders a dialog.)

Pre-commit hook runs fmt + clippy + nextest automatically. **Never bypass with `--no-verify`.** If the hook fails, fix root cause; amend (if pre-first-commit of task) or add a fix-up commit. For hook failures that produce code unfit to keep, `git restore` the failed changes and redo — never force-commit around a hook.

**Per-task reviewer dispatch:** After every implementer DONE report, dispatch BOTH:
1. Spec reviewer (sonnet) — checks work against spec.
2. Code-quality reviewer (`superpowers:code-reviewer` agent, default model) — checks port-ideology compliance (no `#[allow]` outside CLAUDE.md whitelist, no new `Rc<RefCell>`/`Arc`/`Mutex`/`Cow`/`Any`, `pub(crate)` default, `unsafe` requires destructure-first justification per `feedback_destructure_before_unsafe`).

Reviewer briefs explicitly call out:
- No `#[allow]`/`#[expect]` except `non_snake_case` on `emCore` module and `non_camel_case_types` on `em*` types.
- Legacy Rust code in the files being migrated is **defective by default** (`feedback_rust_is_defective.md`); rewrite from C++, don't preserve Rust structure.
- Port-rewrite ledger entries use `COMPLETE.` prefix with no self-SHA (`feedback_ledger_no_self_sha.md`).

**Post-phase:** Phase 3.6 (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-emfiledialog-e024.md`) takes `emFileDialog` from Cell-shim to full engine-subscription. Merge order: 3.5 onto 3.5.A's parent, then 3.5.A closed out separately (3.5.A's tag is preserved as an ancestor).

---

## File structure

**Files created:**
- `docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md` — phase ledger.

**Files modified (primary):**
- `crates/emcore/src/emEngineCtx.rs` — extend `ConstructCtx` trait with `pending_actions` / `allocate_dialog_id` / `root_context`; add `pending_actions: Rc<RefCell<Vec<FrameworkDeferredAction>>>` field to `InitCtx` / `EngineCtx` / `SchedCtx`; `PanelCtx` plumbs via its inner ctx. Update all four implementations of `ConstructCtx`.
- `crates/emcore/src/emScheduler.rs` — add `next_dialog_id: u64` field, `allocate_dialog_id()` method, `engines_for_scope(scope: PanelScope) -> Vec<EngineId>` helper. Update `DoTimeSlice` call-sites to thread `pending_actions` into every built `EngineCtx` / `SchedCtx`.
- `crates/emcore/src/emGUIFramework.rs` — delete `App::next_dialog_id` field; `App::allocate_dialog_id` delegates to scheduler. Reshape `PendingTopLevel`: drop `pending_private_engine: Option<Box<dyn emEngine>>`, add `private_engine_root_panel_id: PanelId`. Rewrite `install_pending_top_level` to construct `DialogPrivateEngine` on the spot from `materialized_wid`. Add `App::close_dialog_by_id(DialogId)` method. Thread `self.pending_actions.clone()` into every ctx construction site.
- `crates/emcore/src/emDialog.rs` — delete legacy `emDialog::{Finish, Input, Paint, LayoutChildren, CheckFinish, silent_cancel, result, on_finish, on_check_finish, auto_delete, buttons, border, preferred_size, GetButton, GetButtonForResult, GetOKButton, GetCancelButton, IsAutoDeletionEnabled}` and the matching private fields. Keep `look: Rc<emLook>` on the handle. Un-gate `DlgPanel`, `DlgButton`, `DialogPrivateEngine`. Narrow `DialogPrivateEngine::window_id: Option<WindowId>` to `window_id: WindowId`. Reshape `emDialog` struct + constructor. Implement mutators (`AddCustomButton`, `AddOKButton`, `AddCancelButton`, `AddOKCancelButtons`, `AddPositiveButton`, `AddNegativeButton`, `SetRootTitle`, `set_button_label_for_result`, `EnableAutoDeletion`, `set_on_finish`, `set_on_check_finish`). Implement `show()`. Rewrite `DialogPrivateEngine::Cycle` auto-delete to the closure rail. Port `ShowMessage` as `unimplemented!()` shim. Update all existing tests.
- `crates/emstocks/src/emStocksListBox.rs` — add 4 `Rc<Cell<Option<DialogResult>>>` fields. Rewrite 4 construct sites (`DeleteStocks`, `CutStocks`, `PasteStocks`, `SetInterest`) to call `.show(cc)` with an `on_finish` closure. Rewrite 4 `Cycle` polling sites to `.take()` from the Cell. Replace 4 `d.silent_cancel()` calls with closure-rail pushes of `App::close_dialog_by_id(did)`.
- `crates/emcore/src/emFileDialog.rs` — delete `set_mode`, `dialog_mut` (dead code). Add `look: Rc<emLook>` field. Add `overwrite_result: Rc<Cell<Option<DialogResult>>>` field. Rewrite `new` to call `.show(ctx)`. Rewrite `CheckFinish`'s overwrite-dialog creation to call `.show(ctx)` + set `on_finish`. Rewrite `Cycle` overwrite-polling to `.take()` from the Cell.
- `crates/emcore/src/emWindow.rs` — narrow `tree` / `take_tree` / `put_tree` visibility where no cross-crate caller uses them (see `project_phase35a_pub_narrow.md`).

**Files modified (test-only):**
- `crates/emcore/src/emDialog.rs` test module — port 15+ existing tests to the new API. Add new tests for `show() transitions pending → None`, `mutator after show panics`, `close_dialog_by_id` pre-/post-materialize, auto-delete end-to-end.
- `crates/emcore/src/emFileDialog.rs` test module — verify new shape; `CheckFinish → ConfirmOverwrite → fire Ok → Cycle → result` end-to-end.
- `crates/emstocks/src/emStocksListBox.rs` test module — verify Cell-based polling works.

**Ledger:**
- `docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md` — entries appended per committed task.

---

## Bootstrap decisions

- **B3.5a (baseline gate):** branch-start nextest 2492/0/9 at `586d6af5`. Every task ends gate-green; pre-commit hook enforces.
- **B3.5b (closure rail, not enum rail):** Per spec §5, no new `DeferredAction` enum variants. All deferred work uses `App::pending_actions` — the closure rail at `emGUIFramework.rs:906`. The one existing emitter of `DeferredAction::CloseWindow` (DialogPrivateEngine auto-delete at emDialog.rs:474) gets rewritten to the closure rail in Task 12 — fixing the latent undrained-push bug.
- **B3.5c (construction at install, not at new):** Per spec §2 and Risks-table default, `DialogPrivateEngine` is built inside `install_pending_top_level` from stored inputs (`close_signal`, `root_panel_id`) once `materialized_wid` is known. `DialogPrivateEngine::window_id: WindowId` (no `Option`). The 3.5.A field `PendingTopLevel::pending_private_engine: Option<Box<dyn emEngine>>` is **replaced** (not extended) by `private_engine_root_panel_id: PanelId`. Task 5 handles the reshape.
- **B3.5d (panic-on-post-show-mutation):** Per spec §3, post-show mutation panics via `self.pending.as_mut().expect("<fn name> after show")`. All live call-sites are pre-show per spec §Audit; the panic is a latent-bug tripwire, not a runtime cost. Phase 3.6 adds real post-show routing via `App::mutate_dialog_by_id` when a live caller needs it.
- **B3.5e (Cell shim for emFileDialog::Cycle):** Per spec §11.4, replace `overwrite_dialog.GetResult()` with `Rc<Cell>` read; keep `emFileDialog::Cycle` caller-driven. Phase 3.6 migrates to proper engine subscription. Risk is flagged in spec Risks table.
- **B3.5f (pre-commit hook):** runs fmt + clippy + nextest. Never bypass. Failures fix root cause.
- **B3.5g (task ordering — infrastructure first):** Tasks 1–4 extend ctx + scheduler + App without touching consumer behavior. Task 5 reshapes `PendingTopLevel` and rewrites `install_pending_top_level`. Tasks 6–11 reshape `emDialog` through successively larger commits, ending at the keystone Task 12 review. Tasks 14–17 migrate `emStocksListBox`. Tasks 19–21 migrate `emFileDialog`. Each commit compiles + passes nextest.
- **B3.5h (golden suite cadence):** Run at Task 15 (end of emStocksListBox migration) and Task 22 (end of emFileDialog migration). Both consumers can theoretically shift dialog rendering; pre-consumer tasks are dialog-only and goldens don't render dialogs. If goldens regress beyond 237/6 at either checkpoint, STOP and re-audit before proceeding.
- **B3.5i (reviewer dispatch per task):** Every task (not just keystones) dispatches spec + code-quality reviewers after the implementer's DONE report. Cost is ~30–90s/task; payoff is catching drift before it compounds (`feedback_review_every_task.md`).

---

## Task 1: Entry-state audit

**Files:**
- Read-only:
  - `docs/superpowers/specs/2026-04-21-phase-3-5-deferred-dialog-construction-design.md` (spec)
  - `crates/emcore/src/emDialog.rs` (legacy API + test-gated DlgPanel/DlgButton/DialogPrivateEngine)
  - `crates/emcore/src/emGUIFramework.rs` (`PendingTopLevel`, `install_pending_top_level`, `pending_actions`)
  - `crates/emcore/src/emEngineCtx.rs` (`ConstructCtx` trait, all four ctx struct shapes)
  - `crates/emcore/src/emScheduler.rs` (`EngineScheduler` state)
  - `crates/emstocks/src/emStocksListBox.rs` lines 45-76, 465-751 (dialog handles + Cycle polling)
  - `crates/emcore/src/emFileDialog.rs` (full file)
  - `~/git/eaglemode-0.96.4/src/emCore/emDialog.cpp` (C++ ground truth)
- Create: `docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md`

- [ ] **Step 1.1: Create branch.**

```bash
cd /home/a0/git/eaglemode-rs
git checkout -b port-rewrite/phase-3-5-deferred-dialog-construction port-rewrite-phase-3-5-a-complete
git status
```

Expected: `On branch port-rewrite/phase-3-5-deferred-dialog-construction`. Clean working tree (pre-existing untracked `.claude/` and the pre-existing `M crates/emcore/src/emDialog.rs` doc-comment edit are acceptable).

- [ ] **Step 1.2: Verify baseline gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: 2492 passed / 0 failed / 9 skipped. Clippy + fmt clean.

- [ ] **Step 1.3: Record audit findings in the ledger.**

Confirm by grep / read:
- `crates/emcore/src/emGUIFramework.rs` has `pub pending_actions: Rc<RefCell<Vec<DeferredAction>>>` at line ~188.
- `crates/emcore/src/emGUIFramework.rs` has `pub struct PendingTopLevel { pub dialog_id, pub window, pub close_signal, pub pending_private_engine: Option<Box<dyn emEngine>> }` at line ~44.
- `crates/emcore/src/emGUIFramework.rs` has `pub(crate) next_dialog_id: u64` at line ~166 and `pub fn allocate_dialog_id(&mut self) -> DialogId` at line ~473.
- `crates/emcore/src/emEngineCtx.rs` has `pub trait ConstructCtx { fn create_signal; fn register_engine; fn wake_up; }` at line ~102.
- `crates/emcore/src/emDialog.rs` has `#[cfg(test)]` on `DlgPanel` (line ~64), `DlgButton` (line ~242), `DialogPrivateEngine` (line ~333).
- `crates/emcore/src/emDialog.rs` has `DialogPrivateEngine::window_id: Option<winit::window::WindowId>` at line ~335.
- `crates/emcore/src/emDialog.rs` has 15+ `#[test]` functions in its test module (line 696+).
- `crates/emstocks/src/emStocksListBox.rs` has 4 `Option<emDialog>` fields (lines 47–50) and 4 `silent_cancel` call-sites (lines 485, 533, 571, 661).
- `crates/emcore/src/emFileDialog.rs:92` defines `set_mode` with zero live callers; line 180 defines `dialog_mut` with zero live callers.

Write the audit findings to the ledger.

- [ ] **Step 1.4: Initial ledger entry.**

Create `docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md`:

```markdown
# Phase 3.5 — Deferred emDialog Construction + Consumer Migration — Ledger

**Spec:** `docs/superpowers/specs/2026-04-21-phase-3-5-deferred-dialog-construction-design.md` (commit `55b3a76d`).

**Base:** `port-rewrite/phase-3-5-a-runtime-toplevel-windows` @ `586d6af5` (tagged `port-rewrite-phase-3-5-a-complete`). Baseline nextest 2492/0/9, goldens 237/6.

## Entry audit

- `App::pending_actions` closure rail present at `emGUIFramework.rs:188`.
- `PendingTopLevel` shape: `{dialog_id, window, close_signal, pending_private_engine: Option<Box<dyn emEngine>>}`. Phase 3.5 Task 5 replaces `pending_private_engine` with `private_engine_root_panel_id: PanelId`.
- `ConstructCtx` trait exposes `create_signal` / `register_engine` / `wake_up`. Phase 3.5 adds `pending_actions` / `allocate_dialog_id` / `root_context`.
- `DlgPanel` / `DlgButton` / `DialogPrivateEngine` `#[cfg(test)]`-gated. Phase 3.5 un-gates.
- `DialogPrivateEngine::window_id: Option<WindowId>` — Phase 3.5 narrows to `WindowId`.
- Legacy `emDialog` API on the struct itself; Phase 3.5 deletes.
- Consumer polling: `emStocksListBox` at 4 Cycle sites, `emFileDialog::Cycle` at the overwrite branch. Phase 3.5 replaces with `Rc<Cell>`.
- Dead API: `emFileDialog::{set_mode, dialog_mut}` — zero live callers. Phase 3.5 deletes.

## Task ledger

```

- [ ] **Step 1.5: Commit.**

```bash
git add docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md
git commit -m "phase-3.5 task 1: entry audit + ledger open"
```

Expected: commit succeeds; pre-commit hook passes (no code changes, fmt/clippy/nextest noop).

**Task 1 exit condition:** ledger file exists, audit findings recorded, branch created off `586d6af5`.

---

## Task 2: Extend `ConstructCtx` trait

**Files:**
- Modify: `crates/emcore/src/emEngineCtx.rs`

**Design:** Per spec §7, add three methods to `ConstructCtx`: `pending_actions(&self) -> &Rc<RefCell<Vec<FrameworkDeferredAction>>>`, `root_context(&self) -> &Rc<emContext>`, `allocate_dialog_id(&mut self) -> DialogId`. Back the first two with new ctx fields (all three struct types `InitCtx`/`EngineCtx`/`SchedCtx` grow a `pending_actions` field; `root_context` already exists on all three). Back the third with a scheduler method (see Task 3).

**Import context note:** `FrameworkDeferredAction` is the alias `crate::emGUIFramework::DeferredAction` — the closure-rail `Box<dyn FnOnce(&mut App, &ActiveEventLoop)>`. Import it into `emEngineCtx.rs` via `use crate::emGUIFramework::DeferredAction as FrameworkDeferredAction;`.

- [ ] **Step 2.1: Write the failing test for the trait methods' existence.**

Add to `crates/emcore/src/emEngineCtx.rs` test module:

```rust
#[test]
fn construct_ctx_exposes_pending_actions_and_root_context() {
    // Proves the trait extensions are present; use InitCtx as the concrete impl.
    let mut sched = EngineScheduler::new();
    let root = emContext::NewRoot();
    let pending_actions: Rc<RefCell<Vec<FrameworkDeferredAction>>> =
        Rc::new(RefCell::new(Vec::new()));
    let mut fw_actions: Vec<DeferredAction> = Vec::new();
    let mut ctx = InitCtx {
        scheduler: &mut sched,
        framework_actions: &mut fw_actions,
        root_context: &root,
        pending_actions: &pending_actions,
    };
    // Exercise both accessors.
    let _: &Rc<RefCell<Vec<FrameworkDeferredAction>>> = ctx.pending_actions();
    let _: &Rc<emContext> = ctx.root_context();
}
```

- [ ] **Step 2.2: Run test — expect compile failure.**

```bash
cargo check --tests
```

Expected: compile errors — `InitCtx` has no `pending_actions` field; `ConstructCtx` trait doesn't define `pending_actions()` / `root_context()`.

- [ ] **Step 2.3: Extend the trait definition.**

Edit `crates/emcore/src/emEngineCtx.rs` at line ~102:

```rust
pub trait ConstructCtx {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(
        &mut self,
        behavior: Box<dyn crate::emEngine::emEngine>,
        pri: Priority,
        scope: PanelScope,
    ) -> EngineId;
    fn wake_up(&mut self, eng: EngineId);
    // Phase 3.5 Task 2 — closure-rail + identity accessors.
    fn pending_actions(&self) -> &Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>>;
    fn root_context(&self) -> &Rc<emContext>;
    fn allocate_dialog_id(&mut self) -> crate::emGUIFramework::DialogId;
}
```

Add at the top:

```rust
use crate::emGUIFramework::DeferredAction as FrameworkDeferredAction;
```

- [ ] **Step 2.4: Add `pending_actions` field to `InitCtx`.**

Edit `InitCtx` struct:

```rust
pub struct InitCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
    /// Phase 3.5 Task 2: closure-rail handle. Plumbed from `App::pending_actions`
    /// at setup; lets construction code enqueue `FnOnce(&mut App, &ActiveEventLoop)`
    /// closures without a borrow of App.
    pub pending_actions: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
}
```

- [ ] **Step 2.5: Add `pending_actions` field to `EngineCtx`.**

Edit `EngineCtx` struct (around line 50):

```rust
pub struct EngineCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub tree: Option<&'a mut PanelTree>,
    pub windows: &'a mut HashMap<winit::window::WindowId, emWindow>,
    pub root_context: &'a Rc<emContext>,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub pending_inputs: &'a mut Vec<(winit::window::WindowId, emInputEvent)>,
    pub input_state: &'a mut emInputState,
    pub framework_clipboard: &'a RefCell<Option<Box<dyn emClipboard>>>,
    pub engine_id: EngineId,
    /// Phase 3.5 Task 2: closure-rail handle. See `InitCtx::pending_actions`.
    pub pending_actions: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
}
```

- [ ] **Step 2.6: Add `pending_actions` field to `SchedCtx`.**

```rust
pub struct SchedCtx<'a> {
    pub scheduler: &'a mut EngineScheduler,
    pub framework_actions: &'a mut Vec<DeferredAction>,
    pub root_context: &'a Rc<emContext>,
    pub framework_clipboard: &'a RefCell<Option<Box<dyn emClipboard>>>,
    pub current_engine: Option<EngineId>,
    /// Phase 3.5 Task 2: closure-rail handle. See `InitCtx::pending_actions`.
    pub pending_actions: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>,
}
```

- [ ] **Step 2.7: Implement the three new `ConstructCtx` methods on `InitCtx`.**

Find the `impl ConstructCtx for InitCtx` block (grep `impl ConstructCtx for InitCtx`) and add:

```rust
fn pending_actions(&self) -> &Rc<RefCell<Vec<FrameworkDeferredAction>>> {
    self.pending_actions
}
fn root_context(&self) -> &Rc<emContext> {
    self.root_context
}
fn allocate_dialog_id(&mut self) -> crate::emGUIFramework::DialogId {
    self.scheduler.allocate_dialog_id()
}
```

- [ ] **Step 2.8: Do the same for `EngineCtx`, `SchedCtx`, and `PanelCtx`.**

Search for all `impl ConstructCtx for` blocks:

```bash
rg -n 'impl ConstructCtx for' crates/emcore/src/emEngineCtx.rs
```

Expected: four impl blocks (`InitCtx`, `EngineCtx`, `SchedCtx`, `PanelCtx`). For `PanelCtx`, the impl delegates through its inner ctx (likely `self.sched.pending_actions` or similar — match existing patterns for `create_signal` / `register_engine`).

Each impl adds the same three methods. Match the existing delegation pattern for the other trait methods.

- [ ] **Step 2.9: Thread `pending_actions` through `PanelCtx::with_sched_reach` and `PanelCtx::new`.**

Grep for `pub fn with_sched_reach` and `pub fn new` on `PanelCtx`:

```bash
rg -n 'impl.*PanelCtx' crates/emcore/src/emEngineCtx.rs
```

For any constructor that builds a SchedCtx-like inner, add a `pending_actions: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>` parameter and thread it through. For constructors that run without a scheduler (`PanelCtx::new`), leave `pending_actions` as `None`-equivalent or provide a dummy — **audit the existing `framework_clipboard` plumbing and mirror it exactly**. The same `Option<...>` pattern `framework_clipboard` uses is the target.

- [ ] **Step 2.10: Update the test at Step 2.1 to compile.**

Verify the test body matches the final field shape. If necessary, adjust (e.g. if `PanelCtx` uses `Option`, the test uses `Some(&pending_actions)`).

- [ ] **Step 2.11: Run test.**

```bash
cargo-nextest ntr --no-capture -- construct_ctx_exposes_pending_actions_and_root_context
```

Expected: PASS.

- [ ] **Step 2.12: Run full test suite.**

```bash
cargo check --all-targets
cargo-nextest ntr
```

Expected: 2493/0/9 (+1 new test). Some unrelated tests may fail compilation because every construction site for `InitCtx` / `EngineCtx` / `SchedCtx` / `PanelCtx` now requires the new `pending_actions` field — this is expected. **Task 2 fixes only its own test; Task 3's step handles wider call-site migration once the scheduler and App are updated to supply the handle.**

If cargo check fails with errors in non-test files (e.g. `emScheduler.rs`, `emGUIFramework.rs`), those get fixed in Task 3.1-3.3. Do not proceed to Step 2.13 until `cargo check --lib` compiles cleanly (the test-scope mismatch is OK; library-scope must be clean).

- [ ] **Step 2.13: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: clean. Append to ledger:

```markdown
- **Task 2 — ConstructCtx trait extension:** COMPLETE. Added `pending_actions` / `root_context` / `allocate_dialog_id` to `ConstructCtx`. All four impls (`InitCtx` / `EngineCtx` / `SchedCtx` / `PanelCtx`) plumbed. New `pending_actions: &Rc<RefCell<Vec<FrameworkDeferredAction>>>` field on three ctx structs (PanelCtx delegates). Gate green — nextest 2493/0/9.
```

```bash
git add -A
git commit -m "phase-3.5 task 2: extend ConstructCtx with pending_actions + root_context + allocate_dialog_id"
```

**Task 2 exit condition:**
- `rg -n 'fn pending_actions' crates/emcore/src/emEngineCtx.rs` → ≥4 sites (trait + 3 impls minimum; PanelCtx delegation counts).
- `rg -n 'pub pending_actions' crates/emcore/src/emEngineCtx.rs` → ≥3 (field on InitCtx/EngineCtx/SchedCtx).
- `cargo check --lib` clean.

**Reviewer dispatch after commit:** spec + code-quality.

---

## Task 3: `EngineScheduler::allocate_dialog_id` + `engines_for_scope`

**Files:**
- Modify: `crates/emcore/src/emScheduler.rs`
- Modify: `crates/emcore/src/emGUIFramework.rs` (delete `App::next_dialog_id`, delegate `App::allocate_dialog_id`)

**Design:** Per spec §8, relocate `next_dialog_id: u64` from `App` to `EngineScheduler`. Per spec §6, add `engines_for_scope(scope: PanelScope) -> Vec<EngineId>` — used later by `App::close_dialog_by_id` to unregister per-window engines.

- [ ] **Step 3.1: Write the failing test for the scheduler's counter + scope query.**

Add to `crates/emcore/src/emScheduler.rs` test module:

```rust
#[test]
fn scheduler_allocates_monotonic_dialog_ids() {
    let mut s = EngineScheduler::new();
    let a = s.allocate_dialog_id();
    let b = s.allocate_dialog_id();
    let c = s.allocate_dialog_id();
    assert_eq!(a.0, 0);
    assert_eq!(b.0, 1);
    assert_eq!(c.0, 2);
}

#[test]
fn engines_for_scope_filters_correctly() {
    struct Noop;
    impl crate::emEngine::emEngine for Noop {
        fn Cycle(&mut self, _: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool { false }
    }
    let mut s = EngineScheduler::new();
    let wid = winit::window::WindowId::dummy();
    let e1 = s.register_engine(Box::new(Noop), crate::emEngine::Priority::Medium, crate::emPanelScope::PanelScope::Framework);
    let e2 = s.register_engine(Box::new(Noop), crate::emEngine::Priority::Medium, crate::emPanelScope::PanelScope::Toplevel(wid));
    let e3 = s.register_engine(Box::new(Noop), crate::emEngine::Priority::Medium, crate::emPanelScope::PanelScope::Toplevel(wid));

    let fw = s.engines_for_scope(crate::emPanelScope::PanelScope::Framework);
    let tl = s.engines_for_scope(crate::emPanelScope::PanelScope::Toplevel(wid));
    assert_eq!(fw, vec![e1]);
    assert_eq!(tl.len(), 2);
    assert!(tl.contains(&e2));
    assert!(tl.contains(&e3));
}
```

- [ ] **Step 3.2: Run — expect compile failure.**

```bash
cargo check --tests -p emcore
```

Expected: errors — `allocate_dialog_id` / `engines_for_scope` not defined.

- [ ] **Step 3.3: Add `next_dialog_id` field to `EngineScheduler`.**

Grep for the `EngineScheduler` struct definition:

```bash
rg -n 'pub struct EngineScheduler' crates/emcore/src/emScheduler.rs
```

Add the field at the end of the struct:

```rust
/// Phase 3.5 Task 3: monotonic counter feeding `allocate_dialog_id`.
/// Relocated from `App::next_dialog_id` (3.5.A Task 9) — construction
/// sites need DialogId allocation reachable through `ConstructCtx`,
/// which cannot borrow App.
next_dialog_id: u64,
```

Initialize in `EngineScheduler::new()`:

```rust
next_dialog_id: 0,
```

- [ ] **Step 3.4: Implement `allocate_dialog_id`.**

```rust
impl EngineScheduler {
    pub fn allocate_dialog_id(&mut self) -> crate::emGUIFramework::DialogId {
        let id = crate::emGUIFramework::DialogId(self.next_dialog_id);
        self.next_dialog_id = self.next_dialog_id
            .checked_add(1)
            .expect("DialogId overflow — u64 exhausted");
        id
    }
}
```

- [ ] **Step 3.5: Implement `engines_for_scope`.**

Find the scheduler's engine-scope storage. Per spec §6, the scheduler already tracks `PanelScope` per engine (Phase 3.5.A Task 6.2). Grep for where that's stored:

```bash
rg -n 'engine_scopes\|PanelScope' crates/emcore/src/emScheduler.rs | head -20
```

Add the helper:

```rust
impl EngineScheduler {
    pub fn engines_for_scope(&self, scope: crate::emPanelScope::PanelScope) -> Vec<EngineId> {
        // engine_scopes: SlotMap<EngineId, PanelScope> (or similar — match existing storage)
        self.engine_scopes
            .iter()
            .filter_map(|(eid, s)| if *s == scope { Some(eid) } else { None })
            .collect()
    }
}
```

Adjust field name if `engine_scopes` is different in the actual code. `PanelScope` must be `Eq` — verify at `emPanelScope.rs`; if not, use `matches!` with explicit variant comparison.

- [ ] **Step 3.6: Run the scheduler tests.**

```bash
cargo-nextest ntr -p emcore -- scheduler::
```

Expected: both new tests PASS.

- [ ] **Step 3.7: Delete `App::next_dialog_id` and redirect `App::allocate_dialog_id`.**

Edit `crates/emcore/src/emGUIFramework.rs`:

- Delete the field `pub(crate) next_dialog_id: u64` (around line 166).
- Delete the field initializer `next_dialog_id: 0,` in `App::new`.
- Replace the body of `App::allocate_dialog_id`:

```rust
/// Allocate a fresh `DialogId` via the scheduler's monotonic counter.
/// Phase 3.5 Task 3: counter relocated from App to EngineScheduler.
pub fn allocate_dialog_id(&mut self) -> DialogId {
    self.scheduler.allocate_dialog_id()
}
```

- [ ] **Step 3.8: Verify existing tests still pass.**

The 3.5.A tests at `emGUIFramework.rs:1189-1197` (`allocate_dialog_id_monotonic`) and `emGUIFramework.rs:1200-1253` (`dialog_window_mut_*`) must continue to pass.

```bash
cargo-nextest ntr -p emcore -- emGUIFramework::
```

Expected: all 3.5.A dialog-id tests pass unchanged.

- [ ] **Step 3.9: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 3 — scheduler dialog-id counter + engines_for_scope:** COMPLETE. `next_dialog_id` relocated from App to EngineScheduler; `App::allocate_dialog_id` delegates. `engines_for_scope(PanelScope) -> Vec<EngineId>` helper added (ownership-forced per spec §6; emWindow cannot borrow scheduler). Gate green.
```

```bash
git add -A
git commit -m "phase-3.5 task 3: relocate next_dialog_id to EngineScheduler; add engines_for_scope"
```

**Task 3 exit condition:**
- `rg -n 'next_dialog_id' crates/emcore/src/emGUIFramework.rs` → 0 matches.
- `rg -n 'next_dialog_id' crates/emcore/src/emScheduler.rs` → ≥2 matches (field + init).
- `rg -n 'engines_for_scope' crates/emcore/src/emScheduler.rs` → ≥1.

**Reviewer dispatch after commit.**

---

## Task 4: Thread `pending_actions` through `App` / scheduler construction sites

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs`
- Modify: `crates/emcore/src/emScheduler.rs`
- Potentially modify: every test harness and code path that constructs `EngineCtx` / `SchedCtx` / `InitCtx` / `PanelCtx`.

**Design:** Task 2 added the `pending_actions` field to all ctx structs but left construction sites unfixed (they pass a dummy or compile-failed). This task threads `App::pending_actions.clone()` into every legitimate construction site. Test harnesses that don't have an App use a test-owned `Rc<RefCell<Vec<_>>>`.

- [ ] **Step 4.1: List all construction sites.**

```bash
rg -n 'EngineCtx \{' crates/emcore crates/emstocks crates/eaglemode 2>/dev/null
rg -n 'SchedCtx \{' crates/emcore crates/emstocks crates/eaglemode 2>/dev/null
rg -n 'InitCtx \{' crates/emcore crates/emstocks crates/eaglemode 2>/dev/null
rg -n 'PanelCtx::new\b\|PanelCtx::with_sched_reach' crates/emcore crates/emstocks crates/eaglemode 2>/dev/null
```

Expected: many sites — the scheduler builds EngineCtx/SchedCtx per cycle dispatch; test harnesses build InitCtx in ~15+ test modules; `PanelCtx::new` is in `emEngineCtx.rs`; etc. Enumerate and commit the list to the ledger inline (comment-only, for reference).

- [ ] **Step 4.2: Thread through `EngineScheduler::DoTimeSlice`.**

Find `DoTimeSlice`:

```bash
rg -n 'fn DoTimeSlice' crates/emcore/src/emScheduler.rs
```

Add a `pending_actions: &Rc<RefCell<Vec<FrameworkDeferredAction>>>` parameter to its signature. Plumb into every `EngineCtx { ... pending_actions, ... }` and `SchedCtx { ... pending_actions, ... }` constructed inside.

- [ ] **Step 4.3: Update `App`'s `DoTimeSlice` call-site.**

In `crates/emcore/src/emGUIFramework.rs`, find the `DoTimeSlice` call:

```bash
rg -n '\.DoTimeSlice(' crates/emcore/src/emGUIFramework.rs
```

Pass `&self.pending_actions` as the new argument. **If a destructuring pattern like `let App { ref mut scheduler, ref pending_actions, .. } = *self;` is needed to satisfy the borrow checker**, prefer destructuring over `unsafe { &*ptr }` per `feedback_destructure_before_unsafe.md`.

- [ ] **Step 4.4: Update test harnesses.**

In `crates/emcore/src/test_view_harness.rs`, `crates/emcore/src/emDialog.rs` test module's `TestInit`, `crates/emcore/src/emFileDialog.rs`'s test module's `TestInit`, and every other test ctx builder:

Add a `pending_actions: Rc<RefCell<Vec<FrameworkDeferredAction>>>` field to the harness struct; initialize `Rc::new(RefCell::new(Vec::new()))`; pass `&self.pending_actions` into the constructed ctx.

**Keep the pattern identical** to how `framework_actions` is plumbed — that's established precedent. The Phase 3.5 addition is an exact parallel.

Grep for every TestInit to find them all:

```bash
rg -n 'struct TestInit' crates/
```

Expected: several. Fix each.

- [ ] **Step 4.5: Compile + test.**

```bash
cargo check --all-targets
cargo-nextest ntr
```

Expected: 2493/0/9. If compile fails, the error usually points directly to a missed construction site. Fix and retry.

- [ ] **Step 4.6: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 4 — pending_actions threaded through construction sites:** COMPLETE. DoTimeSlice signature takes `&Rc<RefCell<Vec<FrameworkDeferredAction>>>`; every EngineCtx/SchedCtx construction in the scheduler receives it. App's call-site passes `&self.pending_actions` (destructured). All TestInit harnesses plumb a local Rc. Gate green — nextest 2493/0/9.
```

```bash
git add -A
git commit -m "phase-3.5 task 4: thread pending_actions through DoTimeSlice + test harnesses"
```

**Task 4 exit condition:**
- `cargo check --all-targets` clean.
- Nextest 2493/0/9.

**Reviewer dispatch after commit.**

---

## Task 5: Reshape `PendingTopLevel` + rewrite `install_pending_top_level`

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs`
- Modify: `crates/emcore/src/emDialog.rs` (existing Task-10 install test and pre-boxed DialogPrivateEngine)

**Design:** Per spec §2 and Risks-table default, replace `PendingTopLevel::pending_private_engine: Option<Box<dyn emEngine>>` with `private_engine_root_panel_id: PanelId`. `install_pending_top_level` constructs `DialogPrivateEngine` at install time with the known `materialized_wid`. `DialogPrivateEngine::window_id` narrows from `Option<WindowId>` to `WindowId`.

- [ ] **Step 5.1: Write the failing test for the new PendingTopLevel shape.**

Add to `crates/emcore/src/emGUIFramework.rs` test module `pending_top_level_tests`:

```rust
#[test]
fn pending_top_level_carries_private_engine_root_panel_id() {
    let mut app = test_app();
    let did = app.allocate_dialog_id();
    let close_sig = app.scheduler.create_signal();
    let flags_sig = app.scheduler.create_signal();
    let focus_sig = app.scheduler.create_signal();
    let geom_sig = app.scheduler.create_signal();
    let mut window = crate::emWindow::emWindow::new_top_level_pending(
        Rc::clone(&app.context),
        crate::emWindow::WindowFlags::empty(),
        "test-dialog".to_string(),
        close_sig, flags_sig, focus_sig, geom_sig,
        emColor::TRANSPARENT,
    );
    // Give the window a tree with a root (mimics emDialog::new).
    let mut tree = crate::emPanelTree::PanelTree::new();
    let root_id = tree.create_root("dlg", false);
    let _ = window.take_tree();
    window.put_tree(tree);
    app.pending_top_level.push(PendingTopLevel {
        dialog_id: did,
        window,
        close_signal: close_sig,
        private_engine_root_panel_id: root_id,
    });
    assert_eq!(app.pending_top_level[0].private_engine_root_panel_id, root_id);
}
```

- [ ] **Step 5.2: Run — expect compile failure.**

Expected: `no field private_engine_root_panel_id` on `PendingTopLevel`.

- [ ] **Step 5.3: Reshape `PendingTopLevel`.**

Edit `crates/emcore/src/emGUIFramework.rs`:

```rust
pub struct PendingTopLevel {
    pub dialog_id: DialogId,
    pub window: emWindow,
    pub close_signal: SignalId,
    /// Phase 3.5 Task 5: root panel id for the soon-to-be-constructed
    /// `DialogPrivateEngine`. Replaces the 3.5.A `pending_private_engine:
    /// Option<Box<dyn emEngine>>` — we no longer pre-box the engine, we
    /// build it at `install_pending_top_level` time with the known
    /// `materialized_wid`.
    pub private_engine_root_panel_id: crate::emPanelTree::PanelId,
}
```

- [ ] **Step 5.4: Narrow `DialogPrivateEngine::window_id` to `WindowId`.**

Edit `crates/emcore/src/emDialog.rs` around line 333:

```rust
pub(crate) struct DialogPrivateEngine {
    pub(crate) root_panel_id: crate::emPanelTree::PanelId,
    /// Phase 3.5 Task 5: no longer `Option<WindowId>` — the engine is
    /// constructed at install time with `materialized_wid` known, so
    /// the field is always populated.
    pub(crate) window_id: winit::window::WindowId,
    pub(crate) close_signal: SignalId,
}
```

(Leave `#[cfg(test)]` in place for now — Task 6 un-gates.)

In `DialogPrivateEngine::Cycle`, find the site that reads `self.window_id`:

```bash
rg -n 'self\.window_id' crates/emcore/src/emDialog.rs
```

Expected match around line 473-474: `if let Some(wid) = self.window_id { ctx.framework_action(DeferredAction::CloseWindow(wid)); }`. Change to:

```rust
// Post-Task-5: window_id always populated (not Option).
ctx.framework_action(crate::emEngineCtx::DeferredAction::CloseWindow(self.window_id));
// NOTE: this emission still uses the undrained enum rail; Task 12 rewrites
// to the closure rail via App::close_dialog_by_id.
```

- [ ] **Step 5.5: Rewrite `install_pending_top_level` to construct the engine at install time.**

Find `install_pending_top_level`:

```bash
rg -n 'pub fn install_pending_top_level' crates/emcore/src/emGUIFramework.rs
```

The current impl pops a `PendingTopLevel`, creates the winit surface, moves `window` into `self.windows`, and registers `pending_private_engine` (if Some) at `Toplevel(wid)`. Rewrite the engine-registration section:

```rust
// Phase 3.5 Task 5: construct DialogPrivateEngine here, not in emDialog::new.
// `materialized_wid` is known at this point; pass it to the engine.
let engine = Box::new(crate::emDialog::DialogPrivateEngine {
    root_panel_id: pending.private_engine_root_panel_id,
    window_id: materialized_wid,
    close_signal: pending.close_signal,
});
let engine_id = self.scheduler.register_engine(
    engine,
    crate::emEngine::Priority::High,
    crate::emPanelScope::PanelScope::Toplevel(materialized_wid),
);
self.scheduler.connect(pending.close_signal, engine_id);
```

(Exact variable names will match the existing impl — `pending` may be named differently. Preserve the existing registration order: register first, then `connect`.)

- [ ] **Step 5.6: Update `install_pending_top_level_headless` mirror.**

```bash
rg -n 'install_pending_top_level_headless' crates/emcore/src/emGUIFramework.rs
```

Apply the same engine-construction change.

- [ ] **Step 5.7: Update the existing 3.5.A Task-10 test to build the new shape.**

In `crates/emcore/src/emDialog.rs` around line 1170-1250, the test `private_engine_observes_close_signal_sets_pending_cancel` currently builds `PendingTopLevel { pending_private_engine: Some(private_engine), ... }`. Change:

```rust
// Was:
// let private_engine = Box::new(DialogPrivateEngine {
//     root_panel_id: root_id,
//     window_id: Some(wid),
//     close_signal: close_sig,
// });
// app.pending_top_level.push(PendingTopLevel {
//     dialog_id, window, close_signal: close_sig,
//     pending_private_engine: Some(private_engine),
// });

// Now:
app.pending_top_level.push(PendingTopLevel {
    dialog_id, window, close_signal: close_sig,
    private_engine_root_panel_id: root_id,
});
```

The test already has `wid = WindowId::dummy()` which `install_pending_top_level_headless` uses directly; no change to that.

- [ ] **Step 5.8: Run the reshape tests.**

```bash
cargo-nextest ntr -p emcore -- emGUIFramework::pending_top_level_tests::
cargo-nextest ntr -p emcore -- emDialog::tests::private_engine_observes_close_signal_sets_pending_cancel
```

Expected: both pass.

- [ ] **Step 5.9: Full suite.**

```bash
cargo-nextest ntr
```

Expected: 2494/0/9 (+1 new test, existing Task-10 test migrated).

- [ ] **Step 5.10: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 5 — PendingTopLevel reshape + install-time engine construction:** COMPLETE. `pending_private_engine` replaced with `private_engine_root_panel_id: PanelId`. `DialogPrivateEngine::window_id` narrowed `Option<WindowId>` → `WindowId`. `install_pending_top_level` + `install_pending_top_level_headless` build the engine from stored inputs at install time. 3.5.A Task-10 test migrated. Gate green — nextest 2494/0/9.
```

```bash
git add -A
git commit -m "phase-3.5 task 5: PendingTopLevel carries panel_id, DialogPrivateEngine built at install"
```

**Task 5 exit condition:**
- `rg -n 'pending_private_engine' crates/emcore/src/` → 0 matches (field gone).
- `rg -n 'private_engine_root_panel_id' crates/emcore/src/emGUIFramework.rs` → ≥2 (field + init).
- `rg -n 'window_id: Option<winit' crates/emcore/src/emDialog.rs` → 0 (narrowed).

**Reviewer dispatch after commit.**

---

## Task 6: Un-gate `DlgPanel` / `DlgButton` / `DialogPrivateEngine`

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Remove `#[cfg(test)]` from the trio; they become real production code. Visibility narrows to `pub(crate)` as CLAUDE.md default (these are not library-public; only `emDialog` itself is the public handle).

- [ ] **Step 6.1: Remove `#[cfg(test)]` from `DlgPanel`.**

At line ~64 of `emDialog.rs`:

```rust
// Remove:
// #[cfg(test)]
// pub struct DlgPanel { ... }

// Keep:
pub(crate) struct DlgPanel { ... }
```

Do the same for all impl blocks attached to `DlgPanel` (constructor, `PanelBehavior` impl, etc.).

- [ ] **Step 6.2: Remove `#[cfg(test)]` from `DlgButton`.**

Same treatment at line ~241.

- [ ] **Step 6.3: Remove `#[cfg(test)]` from `DialogPrivateEngine`.**

Same at line ~332. Narrow to `pub(crate)`.

- [ ] **Step 6.4: Ensure `as_dlg_panel_mut` on `PanelBehavior` is not test-gated.**

The trait method `fn as_dlg_panel_mut(&mut self) -> Option<&mut DlgPanel>` lives on `PanelBehavior` in `emPanel.rs`. Grep:

```bash
rg -n 'fn as_dlg_panel_mut' crates/emcore/src/emPanel.rs
```

If `#[cfg(test)]`-gated, remove the gate. (It may already be ungated from 3.5.A; check.)

- [ ] **Step 6.5: Compile.**

```bash
cargo check --lib
```

Expected: clean. Dead-code lint may fire on `DialogPrivateEngine::root_panel_id` or similar — these fields are used by `Cycle`. If a genuinely unused field surfaces, investigate (likely a real bug, not a lint suppression target).

- [ ] **Step 6.6: Full test suite.**

```bash
cargo-nextest ntr
```

Expected: 2494/0/9. The tests that previously used `#[cfg(test)]`-gated types continue to work.

- [ ] **Step 6.7: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 6 — un-gate DlgPanel/DlgButton/DialogPrivateEngine:** COMPLETE. `#[cfg(test)]` removed; visibility at `pub(crate)`. No dead-code lints surface — all fields are used by constructors, Cycle, or the PanelBehavior impls. Gate green.
```

```bash
git add crates/emcore/src/emDialog.rs crates/emcore/src/emPanel.rs
git commit -m "phase-3.5 task 6: un-gate DlgPanel + DlgButton + DialogPrivateEngine as pub(crate)"
```

**Task 6 exit condition:**
- `rg -n '#\[cfg\(test\)\]\s*\n\s*(pub(\(crate\))?\s+)?struct Dlg\(Panel\|Button\)\|#\[cfg\(test\)\]\s*\n\s*(pub(\(crate\))?\s+)?struct DialogPrivateEngine' crates/emcore/src/emDialog.rs` → 0 (multi-line; adjust for `rg -U` if needed).

**Reviewer dispatch after commit.**

---

## Task 7: Reshape `emDialog` struct + new constructor

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Per spec §1 and §2. Replace the existing `emDialog` struct with the new handle shape: `{dialog_id, finish_signal, close_signal, root_panel_id, look, pending: Option<PendingTopLevel>}`. Rewrite `emDialog::new` per spec §2 to build the DlgPanel-rooted PanelTree, wrap in `emWindow::new_top_level_pending`, and stash as `self.pending = Some(PendingTopLevel {...})`.

**Important:** This task **does not yet delete** the legacy API (`Finish`, `GetResult`, `Input`, etc.). Those methods are still present after this task but now reach non-existent fields — they get deleted in Task 11. To allow the lib to compile between Task 7 and Task 11, **comment out** the legacy methods and fields that reference removed state, with a `// PHASE-3.5-DELETE:` prefix for visibility. Task 11 removes the commented blocks.

- [ ] **Step 7.1: Write the failing test for new construction.**

Add to `emDialog.rs` test module:

```rust
#[test]
fn new_builds_pending_with_populated_tree_and_identity() {
    let mut init = TestInit::new();
    let look = emLook::new();
    let dlg = emDialog::new(&mut init.ctx(), "Test Dialog", look);

    // Identity fields are stable across show().
    assert_eq!(dlg.dialog_id.0 % 1, 0); // just proves the field exists
    let _: SignalId = dlg.finish_signal;
    let _: SignalId = dlg.close_signal;
    let _: crate::emPanelTree::PanelId = dlg.root_panel_id;

    // Pre-show: pending is Some.
    assert!(dlg.pending.is_some());
    let pending = dlg.pending.as_ref().unwrap();
    assert_eq!(pending.dialog_id, dlg.dialog_id);
    assert_eq!(pending.close_signal, dlg.close_signal);
    assert_eq!(pending.private_engine_root_panel_id, dlg.root_panel_id);
}
```

- [ ] **Step 7.2: Run — expect compile failure.**

Expected: `emDialog` fields don't match the test.

- [ ] **Step 7.3: Reshape the struct.**

Edit `emDialog.rs` around line 36:

```rust
pub struct emDialog {
    /// Stable identity across pre-show / post-show transition.
    pub dialog_id: crate::emGUIFramework::DialogId,
    pub finish_signal: SignalId,
    pub close_signal: SignalId,
    /// PanelId of the DlgPanel root. Lives in `pending.window.tree` pre-show;
    /// in `app.windows[app.dialog_windows[dialog_id]].tree` post-show.
    pub root_panel_id: crate::emPanelTree::PanelId,
    /// Shared look held on the handle so pre-show mutators can clone without
    /// touching the pending DlgPanel. Keeps C++ `emDialog::GetLook()` semantics
    /// without re-exposing an accessor.
    pub(crate) look: Rc<emLook>,
    /// Pre-show builder state. `Some` before `show()`, `None` after.
    pub(crate) pending: Option<crate::emGUIFramework::PendingTopLevel>,
}
```

- [ ] **Step 7.4: Comment out legacy fields that referenced the removed shape.**

Delete or `// PHASE-3.5-DELETE:`-comment the fields:
- `border: emBorder`
- `buttons: Vec<(String, DialogResult)>`
- `result: Option<DialogResult>`
- `on_finish: Option<DialogFinishCb>`
- `on_check_finish: Option<DialogCheckFinishCb>`
- `auto_delete: bool`

(`finish_signal` stays in the new struct — do not comment.)

- [ ] **Step 7.5: Comment out legacy impl methods.**

Wrap with `// PHASE-3.5-DELETE:` comments (or use `#[cfg(any())]` to disable without deleting):

```rust
// PHASE-3.5-DELETE: Task 11 removes these legacy methods + fields.
#[cfg(any())]
mod legacy {
    // ... old impl emDialog { fn Finish, fn GetResult, fn Input, fn Paint,
    // fn LayoutChildren, fn CheckFinish, fn silent_cancel, fn set_button_label_for_result,
    // fn GetButton, fn GetButtonForResult, fn GetOKButton, fn GetCancelButton,
    // fn IsAutoDeletionEnabled, fn preferred_size, fn SetRootTitle (old),
    // fn AddCustomButton (old), fn look, fn ShowMessage (old), fn EnableAutoDeletion (old) ...
}
```

**Simpler approach (preferred):** move the entire old `impl emDialog { ... }` block into a `#[cfg(any())]` module; keep only the new struct definition. The new `impl emDialog` for `new` (Step 7.6), mutators (Task 8), and `show` (Task 9) get added incrementally.

- [ ] **Step 7.6: Write the new `emDialog::new`.**

Add a fresh `impl emDialog { ... }` block:

```rust
impl emDialog {
    pub fn new<C: ConstructCtx>(ctx: &mut C, title: &str, look: Rc<emLook>) -> Self {
        let dialog_id = ctx.allocate_dialog_id();
        let finish_signal = ctx.create_signal();
        let close_signal = ctx.create_signal();
        let flags_signal = ctx.create_signal();
        let focus_signal = ctx.create_signal();
        let geom_signal = ctx.create_signal();

        // Build the DlgPanel-rooted PanelTree out-of-band.
        let mut tree = crate::emPanelTree::PanelTree::new();
        let root_panel_id = tree.create_root("dlg", false);
        let dlg_panel = DlgPanel::new(title, Rc::clone(&look), finish_signal);
        tree.set_behavior(root_panel_id, Box::new(dlg_panel));

        // Wrap in a pending top-level window.
        let mut window = crate::emWindow::emWindow::new_top_level_pending(
            Rc::clone(ctx.root_context()),
            crate::emWindow::WindowFlags::empty(),
            format!("emDialog-{}", dialog_id.0),
            close_signal, flags_signal, focus_signal, geom_signal,
            crate::emColor::emColor::TRANSPARENT,
        );
        // `new_top_level_pending` creates an empty default tree; discard it
        // and put our populated tree in its place (3.5.A headless-install
        // test precedent at emDialog.rs:1201-1202).
        let _ = window.take_tree();
        window.put_tree(tree);

        Self {
            dialog_id,
            finish_signal,
            close_signal,
            root_panel_id,
            look: Rc::clone(&look),
            pending: Some(crate::emGUIFramework::PendingTopLevel {
                dialog_id,
                window,
                close_signal,
                private_engine_root_panel_id: root_panel_id,
            }),
        }
    }
}
```

- [ ] **Step 7.7: Compile + run the Step 7.1 test.**

```bash
cargo-nextest ntr -p emcore -- emDialog::tests::new_builds_pending_with_populated_tree_and_identity
```

Expected: PASS.

Some existing `emDialog`-using tests will fail compilation because the old API is commented out. That's expected and fine — Task 8 (mutators) and Task 9 (show) restore user-facing function.

Run the legacy tests with `--no-fail-fast`:

```bash
cargo check --tests -p emcore 2>&1 | head -40
```

If failures are only in `emDialog` tests (because of deleted mutators), note them — Task 12 ports them. If failures are in other files (e.g. `emStocksListBox` or `emFileDialog` tests), those are expected consumer-migration failures — do not fix them yet; Tasks 14+ / 19+ handle them.

**Important:** The library-level `cargo check --lib` must still be clean. Only the `--tests` check is allowed to fail at this waypoint because the test modules reference the commented-out API.

If `cargo check --lib` fails (i.e. non-test code references the commented-out API): two possible causes:
1. A test-only function slipped into a non-test context — investigate.
2. A consumer (`emStocksListBox`, `emFileDialog`) is using the old API from a non-`#[cfg(test)]` path. In that case, **temporarily add a stub** to `impl emDialog { ... }` that `unimplemented!()`s with a "will be implemented in Task 8" message, for the narrowest-possible signature. Remove the stub when Task 8/9 lands the real method.

- [ ] **Step 7.8: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
# Note: clippy --all-targets is allowed to fail at this waypoint because
# test modules reference legacy API. Check lib-only here; tests get fixed
# in subsequent tasks.
cargo-nextest ntr --lib
# Tests-only run may have failures for legacy emDialog API references.
# Do not gate on it here. Next tasks restore.
```

**If cargo clippy --lib fails or cargo-nextest ntr --lib has any failures OTHER than known-commented-out legacy test references, STOP and investigate.**

Append ledger:

```markdown
- **Task 7 — emDialog struct reshape + new():** COMPLETE. Struct now holds identity fields + `look` + `pending: Option<PendingTopLevel>`. `emDialog::new` builds DlgPanel-rooted tree, wraps in `new_top_level_pending`, stashes as pending. Legacy impl moved to `#[cfg(any())]` module (dead-code-gated; Task 11 deletes). cargo check --lib clean. Full nextest gated on Task 11 completion.
```

```bash
git add -A
git commit -m "phase-3.5 task 7: reshape emDialog struct; new() builds PendingTopLevel

Legacy impl moved under #[cfg(any())]; Task 11 deletes.
cargo check --lib clean; test-tree rebuild tasks follow."
```

**Task 7 exit condition:**
- `rg -n 'pub struct emDialog' crates/emcore/src/emDialog.rs` → 1 match; fields are: `dialog_id`, `finish_signal`, `close_signal`, `root_panel_id`, `look`, `pending`.
- `cargo check --lib` clean.

**Reviewer dispatch after commit.** Brief: "Verify Task 7 struct matches spec §1. Legacy impl in #[cfg(any())] block is deliberate staging; do not flag as dead code. Flag if the new constructor diverges from spec §2's pseudocode."

---

## Task 8: Implement mutators on `emDialog`

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Per spec §3. Mutators operate on pre-show state via `self.pending.as_mut().expect("<fn> after show")`. All 11 mutators listed in §3.

- [ ] **Step 8.1: Write failing test for `AddCustomButton`.**

```rust
#[test]
fn add_custom_button_appends_to_button_signals() {
    let mut init = TestInit::new();
    let look = emLook::new();
    let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
    dlg.AddCustomButton(&mut init.ctx(), "OK", DialogResult::Ok);
    dlg.AddCustomButton(&mut init.ctx(), "Cancel", DialogResult::Cancel);

    // Inspect the pending tree's DlgPanel.
    let pending = dlg.pending.as_mut().unwrap();
    let tree = pending.window.tree_mut();
    let behavior = tree.take_behavior(dlg.root_panel_id).expect("root is DlgPanel");
    let dlg_panel = behavior.as_dlg_panel_mut_view().expect("is DlgPanel");
    // NOTE: if `as_dlg_panel_mut_view` doesn't exist, use existing
    // as_dlg_panel_mut once behavior is mutable — grep for the name.
    assert_eq!(dlg_panel.button_signals.len(), 2);
    assert_eq!(dlg_panel.button_signals[0].1, DialogResult::Ok);
    assert_eq!(dlg_panel.button_signals[1].1, DialogResult::Cancel);
    // Restore — drop assertions invariants.
    // (Test context: mutable behavior borrow is tricky across assertions;
    //  refactor to collect button_signals as Vec<DialogResult> before asserting.)
}
```

(If the mutable-borrow dance makes the test awkward, collect the data into owned Vec before asserting.)

- [ ] **Step 8.2: Run — expect compile failure.**

Expected: `AddCustomButton` not defined.

- [ ] **Step 8.3: Implement `AddCustomButton`.**

Append to `impl emDialog`:

```rust
/// Port of C++ `emDialog::AddCustomButton` (emDialog.cpp:86-98).
/// Allocates a DlgButton child panel under the DlgPanel root, records its
/// click_signal + result pair on DlgPanel.button_signals so
/// DialogPrivateEngine::Cycle observes clicks.
///
/// Pre-show only. Panics if called after `show()`.
pub fn AddCustomButton<C: ConstructCtx>(
    &mut self,
    ctx: &mut C,
    label: &str,
    result: DialogResult,
) {
    // Build the DlgButton (needs ctx for click_signal allocation).
    let look = Rc::clone(&self.look);
    let btn = DlgButton::new(ctx, label, look, result.clone(), self.root_panel_id);
    let click_signal = btn.button.click_signal;

    let pending = self.pending.as_mut().expect("AddCustomButton after show");
    let tree = pending.window.tree_mut();

    // Take DlgPanel behavior to record the (click_signal, result) pair.
    let mut behavior = tree
        .take_behavior(self.root_panel_id)
        .expect("DlgPanel root present in pending tree");
    let button_num = {
        let dlg = behavior
            .as_dlg_panel_mut()
            .expect("root is DlgPanel");
        let n = dlg.button_signals.len();
        dlg.button_signals.push((click_signal, result));
        n
    };
    tree.put_behavior(self.root_panel_id, behavior);

    // Create the DlgButton child panel. C++ emDialog.cpp:63 names buttons
    // by ButtonNum: `emString::Format("%d", ButtonNum)`.
    let btn_id = tree.create_child(self.root_panel_id, &button_num.to_string(), None);
    tree.set_behavior(btn_id, Box::new(btn));
}
```

- [ ] **Step 8.4: Run test.**

```bash
cargo-nextest ntr -p emcore -- emDialog::tests::add_custom_button_appends_to_button_signals
```

Expected: PASS.

- [ ] **Step 8.5: Add convenience wrappers — `AddOKButton`, `AddCancelButton`, etc.**

```rust
/// Port of C++ `emDialog::AddOKButton` — `AddPositiveButton("OK")`.
pub fn AddOKButton<C: ConstructCtx>(&mut self, ctx: &mut C) {
    self.AddCustomButton(ctx, "OK", DialogResult::Ok);
}

/// Port of C++ `emDialog::AddCancelButton` — `AddNegativeButton("Cancel")`.
pub fn AddCancelButton<C: ConstructCtx>(&mut self, ctx: &mut C) {
    self.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
}

/// Port of C++ `emDialog::AddOKCancelButtons`.
pub fn AddOKCancelButtons<C: ConstructCtx>(&mut self, ctx: &mut C) {
    self.AddOKButton(ctx);
    self.AddCancelButton(ctx);
}

/// Port of C++ `emDialog::AddPositiveButton`. Generalization of AddOKButton.
pub fn AddPositiveButton<C: ConstructCtx>(&mut self, ctx: &mut C, label: &str) {
    self.AddCustomButton(ctx, label, DialogResult::Ok);
}

/// Port of C++ `emDialog::AddNegativeButton`. Generalization of AddCancelButton.
pub fn AddNegativeButton<C: ConstructCtx>(&mut self, ctx: &mut C, label: &str) {
    self.AddCustomButton(ctx, label, DialogResult::Cancel);
}
```

- [ ] **Step 8.6: Write test for `SetRootTitle`.**

```rust
#[test]
fn set_root_title_updates_dlg_panel_border_caption() {
    let mut init = TestInit::new();
    let look = emLook::new();
    let mut dlg = emDialog::new(&mut init.ctx(), "Old", look);
    dlg.SetRootTitle("New");

    let pending = dlg.pending.as_mut().unwrap();
    let tree = pending.window.tree_mut();
    let mut behavior = tree.take_behavior(dlg.root_panel_id).unwrap();
    let caption = behavior.as_dlg_panel_mut().unwrap().border.GetCaption().to_string();
    tree.put_behavior(dlg.root_panel_id, behavior);
    assert_eq!(caption, "New");
}

#[test]
#[should_panic(expected = "SetRootTitle after show")]
fn set_root_title_after_show_panics() {
    let mut init = TestInit::new();
    let look = emLook::new();
    let mut dlg = emDialog::new(&mut init.ctx(), "Old", look);
    // Simulate show() by taking pending.
    let _ = dlg.pending.take();
    dlg.SetRootTitle("New");  // panics
}
```

- [ ] **Step 8.7: Implement `SetRootTitle` + the `with_dlg_panel_mut` helper.**

```rust
impl emDialog {
    fn with_dlg_panel_mut<R>(
        &mut self,
        label: &'static str,
        f: impl FnOnce(&mut DlgPanel) -> R,
    ) -> R {
        let pending = self
            .pending
            .as_mut()
            .unwrap_or_else(|| panic!("{} after show", label));
        let tree = pending.window.tree_mut();
        let mut behavior = tree
            .take_behavior(self.root_panel_id)
            .expect("DlgPanel root present in pending tree");
        let r = f(behavior.as_dlg_panel_mut().expect("root is DlgPanel"));
        tree.put_behavior(self.root_panel_id, behavior);
        r
    }

    /// Port of C++ `emDialog::SetRootTitle` (emDialog.cpp:49-52).
    /// Pre-show only.
    pub fn SetRootTitle(&mut self, title: &str) {
        self.with_dlg_panel_mut("SetRootTitle", |p| p.SetTitle(title));
    }
}
```

- [ ] **Step 8.8: Run tests.**

```bash
cargo-nextest ntr -p emcore -- emDialog::tests::set_root_title_
```

Expected: both pass (including the panic test).

- [ ] **Step 8.9: Implement the remaining mutators with tests.**

For each of `set_button_label_for_result`, `EnableAutoDeletion`, `set_on_finish`, `set_on_check_finish`:

1. Write a test verifying the mutation lands on the pending DlgPanel (or DlgButton for `set_button_label_for_result`).
2. Run test — expect compile failure.
3. Implement via `with_dlg_panel_mut` or similar (for `set_button_label_for_result`, walk DlgButton children).
4. Run test — PASS.

`set_button_label_for_result` implementation sketch:

```rust
pub fn set_button_label_for_result(&mut self, result: &DialogResult, label: &str) {
    let pending = self.pending.as_mut().expect("set_button_label_for_result after show");
    let tree = pending.window.tree_mut();
    // Walk DlgPanel's children; find first DlgButton whose result matches.
    let children: Vec<crate::emPanelTree::PanelId> =
        tree.children_of(self.root_panel_id).collect();
    for cid in children {
        let mut behavior = match tree.take_behavior(cid) {
            Some(b) => b,
            None => continue,
        };
        let matched = if let Some(btn) = behavior.as_dlg_button_mut() {
            if *btn.result() == *result {
                btn.SetCaption(label);
                true
            } else {
                false
            }
        } else {
            false
        };
        tree.put_behavior(cid, behavior);
        if matched { break; }
    }
}
```

(If `as_dlg_button_mut` doesn't exist on `PanelBehavior`, add it — mirror `as_dlg_panel_mut` pattern. One line in the trait definition, one line in the `DlgButton` impl.)

`EnableAutoDeletion`:

```rust
/// Port of C++ `emDialog::EnableAutoDeletion` (emDialog.cpp:156-159).
pub fn EnableAutoDeletion(&mut self, enabled: bool) {
    self.with_dlg_panel_mut("EnableAutoDeletion", |p| p.auto_delete = enabled);
}
```

`set_on_finish`:

```rust
pub fn set_on_finish(&mut self, cb: DialogFinishCb) {
    self.with_dlg_panel_mut("set_on_finish", |p| p.on_finish = Some(cb));
}
```

`set_on_check_finish`:

```rust
pub fn set_on_check_finish(&mut self, cb: DialogCheckFinishCb) {
    self.with_dlg_panel_mut("set_on_check_finish", |p| p.on_check_finish = Some(cb));
}
```

- [ ] **Step 8.10: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
cargo-nextest ntr --lib
cargo-nextest ntr -p emcore -- emDialog::
```

Expected: clippy clean on lib. emDialog unit tests that reference only new API all pass. Other emDialog tests (old API) and consumer tests continue to fail — that's expected.

Append ledger:

```markdown
- **Task 8 — emDialog mutators:** COMPLETE. Implemented `AddCustomButton` + 4 convenience wrappers (`AddOKButton`, `AddCancelButton`, `AddOKCancelButtons`, `AddPositiveButton`, `AddNegativeButton`) + `SetRootTitle` + `set_button_label_for_result` + `EnableAutoDeletion` + `set_on_finish` + `set_on_check_finish`. All route through `with_dlg_panel_mut` for `self.pending.as_mut().expect("<fn> after show")` panic contract. Added `as_dlg_button_mut` to PanelBehavior trait. Gate green (lib).
```

```bash
git add -A
git commit -m "phase-3.5 task 8: emDialog mutators via with_dlg_panel_mut; panic on post-show misuse"
```

**Task 8 exit condition:**
- `rg -n 'pub fn (AddCustomButton|AddOKButton|AddCancelButton|SetRootTitle|set_button_label_for_result|EnableAutoDeletion|set_on_finish|set_on_check_finish)' crates/emcore/src/emDialog.rs` → ≥8 matches.
- `rg -n '"(AddCustomButton|SetRootTitle|EnableAutoDeletion|set_on_finish|set_on_check_finish|set_button_label_for_result) after show"' crates/emcore/src/emDialog.rs` → ≥5 panic messages.

**Reviewer dispatch after commit.**

---

## Task 9: Implement `emDialog::show` + `ShowMessage` shim

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Per spec §4. `show()` takes `pending` via `.take()` and pushes a closure onto `ctx.pending_actions()` that calls `app.pending_top_level.push(p)` then `app.install_pending_top_level(el)`.

- [ ] **Step 9.1: Write failing test for `show`.**

```rust
#[test]
fn show_drains_pending_into_closure_rail() {
    let mut init = TestInit::new();
    let look = emLook::new();
    let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
    assert!(dlg.pending.is_some());
    dlg.show(&mut init.ctx());
    assert!(dlg.pending.is_none());
    // Closure was pushed onto pending_actions.
    assert_eq!(init.pending_actions.borrow().len(), 1);
    // Identity fields still valid.
    let _: DialogId = dlg.dialog_id;
    let _: SignalId = dlg.finish_signal;
}

#[test]
#[should_panic(expected = "show called twice")]
fn show_twice_panics() {
    let mut init = TestInit::new();
    let look = emLook::new();
    let mut dlg = emDialog::new(&mut init.ctx(), "Test", look);
    dlg.show(&mut init.ctx());
    dlg.show(&mut init.ctx());  // panics
}
```

(TestInit must expose a field `pending_actions: Rc<RefCell<Vec<FrameworkDeferredAction>>>` — added in Task 4.)

- [ ] **Step 9.2: Run — expect compile failure.**

Expected: `show` not defined.

- [ ] **Step 9.3: Implement `show`.**

```rust
impl emDialog {
    /// Enqueue this pending dialog for installation on the next
    /// `about_to_wait` tick. Consumes `self.pending`; the handle remains
    /// valid — `dialog_id` / `finish_signal` / `close_signal` / `root_panel_id`
    /// are stable across the show transition.
    ///
    /// Port: C++ `emDialog` construction implicitly shows the dialog because
    /// construction creates the X window via `emWindow` base-class ctor.
    /// Rust splits the two-phase pattern (3.5.A `new_top_level_pending` +
    /// `install_pending_top_level`); `show()` is the explicit second phase.
    pub fn show<C: ConstructCtx>(&mut self, ctx: &mut C) {
        let pending = self.pending.take().expect("show called twice");
        let queue = ctx.pending_actions().clone();
        queue.borrow_mut().push(Box::new(move |app, el| {
            app.pending_top_level.push(pending);
            app.install_pending_top_level(el);
        }));
    }
}
```

- [ ] **Step 9.4: Run tests.**

```bash
cargo-nextest ntr -p emcore -- emDialog::tests::show_
```

Expected: both pass.

- [ ] **Step 9.5: Port `ShowMessage` as `unimplemented!()` shim.**

```rust
impl emDialog {
    /// Static convenience that builds an OK-only message dialog with
    /// auto-delete. Port of C++ `emDialog::ShowMessage` (emDialog.cpp:162-180).
    ///
    /// Phase 3.5: shimmed as `unimplemented!()` because no live caller exists
    /// in-tree. Phase 3.6 wires a real path: `new + AddOKButton +
    /// EnableAutoDeletion(true) + content label + show`.
    pub fn ShowMessage<C: ConstructCtx>(
        _ctx: &mut C,
        _title: &str,
        _message: &str,
    ) -> Self {
        unimplemented!(
            "emDialog::ShowMessage — Phase 3.6 impl; no live caller in 3.5"
        )
    }
}
```

- [ ] **Step 9.6: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
cargo-nextest ntr -p emcore -- emDialog::
```

Append ledger:

```markdown
- **Task 9 — emDialog::show + ShowMessage shim:** COMPLETE. `show` takes `pending`, pushes a closure onto `ctx.pending_actions()` that calls `install_pending_top_level`. Panic on double-show. `ShowMessage` shimmed to `unimplemented!()` pending Phase 3.6 live caller.
```

```bash
git add -A
git commit -m "phase-3.5 task 9: emDialog::show enqueues closure-rail install; ShowMessage shim"
```

**Task 9 exit condition:**
- `rg -n 'pub fn show' crates/emcore/src/emDialog.rs` → 1 match.
- `rg -n 'show called twice' crates/emcore/src/emDialog.rs` → 1 match.
- `rg -n 'install_pending_top_level\(el\)' crates/emcore/src/emDialog.rs` → 1 match (inside the closure).

**Reviewer dispatch after commit.**

---

## Task 10: `App::close_dialog_by_id` + `DialogPrivateEngine` auto-delete rewrite

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs`
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** Per spec "§App::close_dialog_by_id" + §5. Add the unified close method. Rewrite `DialogPrivateEngine::Cycle` auto-delete emission from enum-rail `DeferredAction::CloseWindow(wid)` to closure-rail `pending_actions().borrow_mut().push(Box::new(move |app, _el| app.close_dialog_by_id(did)))`.

- [ ] **Step 10.1: Write failing tests for `close_dialog_by_id`.**

```rust
#[test]
fn close_dialog_by_id_pre_materialize_drops_pending() {
    let mut app = test_app();
    let did = app.allocate_dialog_id();
    let close_sig = app.scheduler.create_signal();
    // ... build flags/focus/geom + window + tree like existing pending_top_level_tests
    let mut window = /* as existing test */;
    app.pending_top_level.push(PendingTopLevel {
        dialog_id: did, window, close_signal: close_sig,
        private_engine_root_panel_id: /* root id */,
    });
    assert_eq!(app.pending_top_level.len(), 1);

    app.close_dialog_by_id(did);

    assert_eq!(app.pending_top_level.len(), 0);
    // dialog_windows map unaffected (wasn't materialized).
    assert!(!app.dialog_windows.contains_key(&did));
}

#[test]
fn close_dialog_by_id_post_materialize_removes_window_and_engines() {
    // Build an App with a materialized dialog (use install_pending_top_level_headless).
    let mut app = test_app();
    /* ... set up + install ... */
    let did = /* captured */;
    let wid = /* captured */;
    assert!(app.windows.contains_key(&wid));
    assert!(!app.scheduler.engines_for_scope(
        crate::emPanelScope::PanelScope::Toplevel(wid)
    ).is_empty());

    app.close_dialog_by_id(did);

    assert!(!app.windows.contains_key(&wid));
    assert!(app.scheduler.engines_for_scope(
        crate::emPanelScope::PanelScope::Toplevel(wid)
    ).is_empty());
    assert!(!app.dialog_windows.contains_key(&did));
}

#[test]
fn close_dialog_by_id_unknown_is_noop() {
    let mut app = test_app();
    let did = app.allocate_dialog_id();
    // Never enqueued.
    app.close_dialog_by_id(did);  // no panic
    assert_eq!(app.pending_top_level.len(), 0);
}
```

- [ ] **Step 10.2: Run — expect compile failure.**

- [ ] **Step 10.3: Implement `App::close_dialog_by_id`.**

```rust
impl App {
    /// Unified close path for dialogs. Handles both pre-materialize
    /// (drops the pending_top_level entry) and post-materialize (removes
    /// the window + unregisters all Toplevel(wid) engines) cases.
    ///
    /// Consumers:
    /// - `emStocksListBox` `silent_cancel` replacement (Phase 3.5 Task 15).
    /// - `DialogPrivateEngine::Cycle` auto-delete (Phase 3.5 Task 10 rewrite).
    pub fn close_dialog_by_id(&mut self, did: DialogId) {
        if let Some(wid) = self.dialog_windows.remove(&did) {
            // Post-materialize: unregister engines + remove window.
            let engine_ids = self
                .scheduler
                .engines_for_scope(crate::emPanelScope::PanelScope::Toplevel(wid));
            for eid in engine_ids {
                self.scheduler.remove_engine(eid);
            }
            // Window dropped here — signals on DlgPanel / DlgButtons leak
            // to slotmap's dead-key semantics (fire-to-dead-signal is a no-op).
            self.windows.remove(&wid);
        } else if let Some(idx) = self
            .pending_top_level
            .iter()
            .position(|p| p.dialog_id == did)
        {
            // Pre-materialize: drop the entry.
            // `swap_remove` — order doesn't matter for pending_top_level.
            let _ = self.pending_top_level.swap_remove(idx);
        }
        // Else: unknown DialogId — idempotent no-op.
    }
}
```

- [ ] **Step 10.4: Run tests.**

```bash
cargo-nextest ntr -p emcore -- emGUIFramework::pending_top_level_tests::close_dialog_by_id
```

Expected: all 3 pass.

- [ ] **Step 10.5: Rewrite `DialogPrivateEngine::Cycle` auto-delete.**

Find the auto-delete branch in `emDialog.rs` (around line 471-477):

```rust
// Was (Task 5 updated the field narrowing but kept the enum-rail push):
// } else {
//     ctx.framework_action(crate::emEngineCtx::DeferredAction::CloseWindow(self.window_id));
//     false
// }
```

Replace with:

```rust
} else {
    // Phase 3.5 Task 10: closure-rail replaces undrained enum-rail push.
    // The previous `DeferredAction::CloseWindow` emission landed on
    // emEngineCtx::DeferredAction (the unread enum rail), so auto-delete
    // was never actually wired — latent bug. Use closure rail instead.
    let did = {
        // Reverse-lookup DialogId from window_id via ctx.windows map.
        // Alternative: store dialog_id on DialogPrivateEngine itself.
        // Cleaner: add dialog_id to DialogPrivateEngine — it's one more u64.
        self.dialog_id  // requires field addition; see Step 10.6
    };
    ctx.pending_actions
        .borrow_mut()
        .push(Box::new(move |app, _el| app.close_dialog_by_id(did)));
    false
}
```

- [ ] **Step 10.6: Add `dialog_id: DialogId` to `DialogPrivateEngine`.**

Edit `DialogPrivateEngine` struct (around line 333):

```rust
pub(crate) struct DialogPrivateEngine {
    pub(crate) dialog_id: crate::emGUIFramework::DialogId,
    pub(crate) root_panel_id: crate::emPanelTree::PanelId,
    pub(crate) window_id: winit::window::WindowId,
    pub(crate) close_signal: SignalId,
}
```

Update `install_pending_top_level` in `emGUIFramework.rs` to pass `dialog_id: pending.dialog_id`:

```rust
let engine = Box::new(crate::emDialog::DialogPrivateEngine {
    dialog_id: pending.dialog_id,
    root_panel_id: pending.private_engine_root_panel_id,
    window_id: materialized_wid,
    close_signal: pending.close_signal,
});
```

Update `install_pending_top_level_headless` similarly.

- [ ] **Step 10.7: Write auto-delete end-to-end test.**

```rust
#[test]
fn auto_delete_countdown_closes_window_via_closure_rail() {
    // Build an app + install a dialog + enable auto-delete + fire close_signal.
    // After 3 DoTimeSlice ticks + one pending_actions drain, the window
    // should be gone.
    /* ... */
    assert!(app.windows.contains_key(&wid), "pre-drain present");
    /* fire close_signal, DoTimeSlice, DoTimeSlice, DoTimeSlice */
    // Now one closure is queued on pending_actions. Drain manually to
    // simulate about_to_wait:
    let actions: Vec<_> = app.pending_actions.borrow_mut().drain(..).collect();
    for a in actions { a(&mut app, el); }
    assert!(!app.windows.contains_key(&wid), "post-drain removed");
}
```

(This test requires an ActiveEventLoop-substitute or a closure-rail-drain helper. Simplest: add a test helper `App::drain_pending_actions_for_tests()` that iterates without `&ActiveEventLoop` — accept that the closure body won't have `el` for test purposes. Alternative: skip the end-to-end closure drain in this test and rely on Step 10.1's `close_dialog_by_id_post_materialize_removes_window_and_engines` test as the true verification.)

**Recommended approach:** skip the full closure-rail drain test here. The pre/post-materialize unit tests plus a simpler "closure is pushed onto pending_actions" observation suffice:

```rust
#[test]
fn auto_delete_emits_close_closure_after_countdown() {
    let mut app = /* set up with auto_delete=true, run DoTimeSlice 3 times after close_signal fires */;
    /* ... */
    assert_eq!(app.pending_actions.borrow().len(), 1,
        "auto-delete must push one close closure");
    // That closure's body calls close_dialog_by_id — verified via the
    // dedicated close_dialog_by_id_post_materialize test.
}
```

- [ ] **Step 10.8: Update the existing 3.5.A Task-10 auto-delete test (emDialog.rs:~1296-1336).**

That test fires close_signal and asserts internal state (`finish_state == 0`, `finalized_result == Cancel`). Keep those assertions; add the new "pending_actions populated after 3-slice countdown" assertion.

**Actually re-examine:** the existing test disables auto-delete (hits the `!ADEnabled` branch). It doesn't test the full countdown. Add a new test alongside:

```rust
#[test]
fn private_engine_with_auto_delete_emits_close_closure() {
    /* same setup as existing test, but call dlg.EnableAutoDeletion(true)
       before show. Fire close_signal, run DoTimeSlice 4 times (one to
       finalize + three countdown). Observe pending_actions grows by 1. */
}
```

- [ ] **Step 10.9: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
cargo-nextest ntr -p emcore -- emGUIFramework:: emDialog::
```

Append ledger:

```markdown
- **Task 10 — App::close_dialog_by_id + auto-delete closure-rail rewrite:** COMPLETE. Unified close path handles pre- and post-materialize via engines_for_scope + dialog_windows branching. `DialogPrivateEngine.dialog_id` added; `Cycle` auto-delete branch pushes closure-rail action instead of undrained enum-rail `CloseWindow(wid)`. Latent auto-delete bug fixed incidentally.
```

```bash
git add -A
git commit -m "phase-3.5 task 10: App::close_dialog_by_id + auto-delete closure-rail (fixes latent undrained-push bug)"
```

**Task 10 exit condition:**
- `rg -n 'pub fn close_dialog_by_id' crates/emcore/src/emGUIFramework.rs` → 1.
- `rg -n 'DeferredAction::CloseWindow' crates/emcore/src/emDialog.rs` → 0 (auto-delete rewrote to closure rail).
- `rg -n 'dialog_id: crate::emGUIFramework::DialogId' crates/emcore/src/emDialog.rs` → 1 (on DialogPrivateEngine).

**Reviewer dispatch after commit.**

---

## Task 11: Delete legacy `emDialog` API + fields

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Design:** The `#[cfg(any())]` legacy module from Task 7 and any remaining commented-out code gets deleted. Real deletions: methods `Finish`, `Input`, `Paint`, `LayoutChildren`, `CheckFinish`, `silent_cancel`, `GetResult`, `GetButton`, `GetButtonForResult`, `GetOKButton`, `GetCancelButton`, `IsAutoDeletionEnabled`, `preferred_size`, old `SetRootTitle`, old `AddCustomButton`, old `EnableAutoDeletion`, `look()`. And the fields: `border`, `buttons`, `result`, `on_finish`, `on_check_finish`, `auto_delete`.

- [ ] **Step 11.1: Delete the `#[cfg(any())]` legacy block.**

In `emDialog.rs`, find the module wrapping the old impl and remove it entirely.

- [ ] **Step 11.2: Verify no reference to legacy symbols remains in the file.**

```bash
rg -n 'fn (Finish|Input|Paint|LayoutChildren|CheckFinish|silent_cancel|GetResult|GetButton|GetButtonForResult|GetOKButton|GetCancelButton|IsAutoDeletionEnabled|preferred_size)' crates/emcore/src/emDialog.rs
```

Expected: 0 matches in the `impl emDialog` scope. Matches elsewhere (e.g. the `DlgPanel::Paint`, `DlgPanel::LayoutChildren`, `DlgPanel::Input` PanelBehavior impls) are legitimate — those stay.

- [ ] **Step 11.3: Compile + verify no non-test external caller uses the deleted API.**

```bash
cargo check --lib 2>&1 | head -40
```

Expected: clean. If any library file (not a test module) references the deleted methods, investigate — those are real consumer-migration failures to fix in Tasks 14+/19+. For now, document as expected follow-up.

`cargo check --tests` may still have emDialog test-module compile errors (tests reference deleted API). Task 12 ports them.

- [ ] **Step 11.4: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
```

Append ledger:

```markdown
- **Task 11 — legacy emDialog API + fields deleted:** COMPLETE. Removed methods: Finish, Input, Paint, LayoutChildren, CheckFinish, silent_cancel, GetResult, GetButton{,ForResult}, GetOKButton, GetCancelButton, IsAutoDeletionEnabled, preferred_size, look(). Removed fields: border, buttons, result, on_finish, on_check_finish, auto_delete. cargo check --lib clean.
```

```bash
git add crates/emcore/src/emDialog.rs
git commit -m "phase-3.5 task 11: delete legacy emDialog API + fields"
```

**Task 11 exit condition:**
- `rg -c 'PHASE-3.5-DELETE' crates/emcore/src/emDialog.rs` → 0.
- `rg -c '#\[cfg\(any\(\)\)\]' crates/emcore/src/emDialog.rs` → 0.

**Reviewer dispatch after commit.**

---

## Task 12: Port existing emDialog tests to new API

**Files:**
- Modify: `crates/emcore/src/emDialog.rs` test module

**Design:** 15 legacy tests reference the deleted API. Port each to exercise the new shape. Some become redundant (e.g. tests that directly manipulate `result` field) and get deleted; others (Enter/Escape input, button carrying result payload, check_finish veto) port to the DlgPanel or end-to-end path.

**Keep tests that prove observable behavior. Delete tests that prove nothing but "this field exists."**

Target: Task-12 end nextest count = 2494 + N (N = new tests added across Tasks 5/7/8/9/10) minus any redundant legacy tests removed.

- [ ] **Step 12.1: List all failing tests after legacy deletion.**

```bash
cargo check --tests -p emcore 2>&1 | grep 'error\[' | head -40
```

Enumerate every emDialog test that fails compilation.

- [ ] **Step 12.2: Port each test. Grouping:**

**Input/keyboard tests** — Enter / Escape / Shift / Ctrl / release-ignored:
- Keep the DlgPanel-level tests (they already exist in the current test module, `dlg_panel_enter_sets_pending_ok` et al. — they're Phase-3.5.A-ported already).
- Delete the emDialog-level `enter_finishes_with_ok` / `escape_finishes_with_cancel` / `enter_with_modifier_is_ignored` / `release_event_is_ignored` tests — they tested the legacy `emDialog::Input` that no longer exists. DlgPanel's tests cover the same ground.

**Finish/check-finish tests** — `dialog_finish_fires_callback`, `check_finish_can_veto`, `dialog_custom_result`, `check_finish_lifecycle`:
- Replace with tests that go through the full pipeline: `new → show → install via headless → fire close_signal or button click_signal → DoTimeSlice → on_finish fires`. Mirror the existing `private_engine_observes_close_signal_sets_pending_cancel` test at line 1157.
- Delete the direct `Finish`-method-call tests.

**Button API tests** — `add_custom_button_lookup`, `set_button_label`:
- Port to the new mutators. Test assertions read from the pending DlgPanel's child DlgButton behaviors via `tree.take_behavior(child_id).as_dlg_button_mut()`.

**Auto-deletion toggle test** — `auto_deletion_toggle`:
- `EnableAutoDeletion(true/false)` now lives pre-show; test reads `dlg.pending.as_ref().unwrap().window.tree_ref().behaviors[root].auto_delete` or via a helper. Or delete the test and rely on Task 10's end-to-end auto-delete closure-rail test.

**`set_root_title`** — already ported in Task 8.

**`dialog_fires_finish_signal_on_input_enter`** — replace with end-to-end: fire Enter → scheduler drains → finish_signal fires → on_finish callback writes to test Cell.

Implementation: port each test minimally; do not expand scope.

- [ ] **Step 12.3: Run the full test suite.**

```bash
cargo-nextest ntr
```

Expected: green. If consumer tests (`emStocksListBox`, `emFileDialog`) still fail, **that is acceptable at Task 12** — Tasks 14-22 fix them. Note the count.

**Acceptance at this task:** `cargo-nextest ntr -p emcore -- emDialog::` is green. `-p emcore` test failures allowed only in `emFileDialog` tests. `-p emstocks` failures allowed.

- [ ] **Step 12.4: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
```

Append ledger:

```markdown
- **Task 12 — emDialog tests ported:** COMPLETE. Legacy Input-method tests deleted (DlgPanel-level tests cover the same ground). Finish/check_finish tests rewritten as end-to-end pipelines through install_pending_top_level_headless. Button API tests ported to new mutators. emDialog test module green.
```

```bash
git add crates/emcore/src/emDialog.rs
git commit -m "phase-3.5 task 12: port emDialog tests to new handle API"
```

**Task 12 exit condition:**
- `cargo-nextest ntr -p emcore -- emDialog::` all pass.
- Clippy clean on lib.

**Reviewer dispatch after commit.**

---

## Task 13: Task-5.1 keystone gate

**Files:** none modified.

**Design:** Task 5.1 (spec phase split) spans Tasks 2-12 of this plan. Before moving to consumer migrations, verify the lib-level + emDialog-level invariants are all green and no regressions landed in `-p emcore`.

- [ ] **Step 13.1: Full gate run.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr -p emcore
```

If `--all-targets` clippy complains about emStocksListBox / emFileDialog tests still referencing deleted API, **note it and proceed** — those fail-to-compile by design until Tasks 14+/19+ migrate them. For now, treat Task 13 as passing if:
- `cargo-nextest ntr -p emcore` has no failures other than those in `emStocksListBox` / `emFileDialog` test modules.
- `cargo check --lib` clean.
- `cargo clippy --lib` clean.

- [ ] **Step 13.2: Append ledger keystone entry.**

```markdown
### Keystone: Task 5.1 emDialog reshape COMPLETE

Ledger summary: Tasks 2-12 land the `emDialog` handle reshape, closure-rail install via `pending_actions`, install-time `DialogPrivateEngine` construction (no `Option<WindowId>`), `App::close_dialog_by_id` unification, auto-delete bug fix, and new mutator surface. Consumer migrations follow.

Invariants verified:
- I5a: emDialog owns one emWindow owning one PanelTree with one DlgPanel root — enforced by new().
- I5b: close_signal drives DialogPrivateEngine cancel — unchanged from 3.5.A.
- I5c: DialogPrivateEngine scoped Toplevel(wid) post-materialize — install_pending_top_level registers.
- I5d: finish_signal fires once — FinishState machine preserved.
- I5e: auto-delete drives window teardown — NOW via closure rail + close_dialog_by_id.
- I5f: identity survives show() — pending.take() leaves identity fields intact.
- I5g: post-show mutator panics — expect("<fn> after show") tripwires added.
- I5h: construction no &mut App — ConstructCtx only.
- I5i: on_finish + Rc<Cell> observation — shape ready; Task 14+ consumes.
- I5j: close_dialog_by_id supersedes silent_cancel — Task 14+ consumes.
```

- [ ] **Step 13.3: No commit (keystone is observational).**

**Task 13 exit condition:**
- Ledger keystone entry appended.
- Clean gate at lib + emcore-test level.

**Reviewer dispatch:** Keystone review — brief spec + code-quality reviewers with the full Task 2-12 diff and the invariants above. Ask explicitly: "Does the shape match spec §1-§9 and are all 10 invariants I5a-I5j honored? Flag any 'almost-right' drift."

---

## Task 14: `emStocksListBox` — add `Rc<Cell>` result fields

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs`

**Design:** Per spec §10.1. Add four `Rc<Cell<Option<DialogResult>>>` fields alongside the existing `Option<emDialog>` handles.

- [ ] **Step 14.1: Read current struct (lines 45-80).**

- [ ] **Step 14.2: Add fields + initialize in `new` / `Default`.**

```rust
pub struct emStocksListBox {
    // ... existing fields ...
    pub(crate) cut_stocks_dialog: Option<emDialog>,
    pub(crate) paste_stocks_dialog: Option<emDialog>,
    pub(crate) delete_stocks_dialog: Option<emDialog>,
    pub(crate) interest_dialog: Option<emDialog>,
    // Phase 3.5 Task 14: result slots written by on_finish closures.
    pub(crate) cut_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) paste_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) delete_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) interest_result: Rc<Cell<Option<DialogResult>>>,
    // ... existing fields ...
}
```

Initialize: `Rc::new(Cell::new(None))` for each.

- [ ] **Step 14.3: Compile.**

```bash
cargo check -p emstocks --lib
```

Expected: clean (new fields are additive).

- [ ] **Step 14.4: Gate + commit.**

```bash
cargo fmt
cargo clippy --lib -- -D warnings
```

Append ledger:

```markdown
- **Task 14 — emStocksListBox Rc<Cell> fields:** COMPLETE. Four `cut_stocks_result` / `paste_stocks_result` / `delete_stocks_result` / `interest_result` fields added. Initialized in constructors. Lib clean.
```

```bash
git add crates/emstocks/src/emStocksListBox.rs
git commit -m "phase-3.5 task 14: emStocksListBox adds Rc<Cell> dialog-result slots"
```

**Reviewer dispatch.**

---

## Task 15: `emStocksListBox` — migrate 4 construct + polling + silent_cancel sites

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs`

**Design:** Per spec §10.2, §10.3, §10.4. For each of the 4 dialog sites (`DeleteStocks`, `CutStocks`, `PasteStocks`, `SetInterest`):
1. Construct site: add `set_on_finish` closure capturing the Cell; call `show(cc)`.
2. Polling site in `Cycle`: read from `Cell.take()` instead of `dialog.GetResult()`.
3. `silent_cancel` call: replace with closure-rail `close_dialog_by_id`.

**Because this is a behavioral-risk change, this is the first task in the plan that runs the golden suite.**

- [ ] **Step 15.1: Migrate `DeleteStocks` construct site (emStocksListBox.rs:~485-497).**

```rust
// Was:
// if let Some(ref mut d) = self.delete_stocks_dialog {
//     d.silent_cancel();
// }
// let mut dialog = emDialog::new(cc, &format!("Really delete {} stock(s)?", count), look.clone());
// dialog.AddCustomButton("Delete", DialogResult::Ok);
// dialog.AddCustomButton("Cancel", DialogResult::Cancel);
// self.delete_stocks_dialog = Some(dialog);

// Now:
if let Some(old) = self.delete_stocks_dialog.take() {
    let did = old.dialog_id;
    cc.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
        app.close_dialog_by_id(did);
    }));
    self.delete_stocks_result.set(None);  // clear stale result, if any
}
let mut dialog = emDialog::new(cc, &format!("Really delete {} stock(s)?", count), look.clone());
dialog.AddCustomButton(cc, "Delete", DialogResult::Ok);
dialog.AddCustomButton(cc, "Cancel", DialogResult::Cancel);
let cell = Rc::clone(&self.delete_stocks_result);
dialog.set_on_finish(Box::new(move |r, _sched| cell.set(Some(r.clone()))));
dialog.show(cc);
self.delete_stocks_dialog = Some(dialog);
```

- [ ] **Step 15.2: Migrate `CutStocks` construct site (~530-542).** Same pattern.

- [ ] **Step 15.3: Migrate `PasteStocks` construct site (~568-577).** Same pattern.

- [ ] **Step 15.4: Migrate `SetInterest` construct site (~658-668).** Same pattern.

- [ ] **Step 15.5: Migrate `Cycle` polling sites (4 sites at emStocksListBox.rs:~694-748).**

```rust
// Was (example — delete):
// if let Some(result) = self.delete_stocks_dialog.as_ref().and_then(|d| d.GetResult()) {
//     let confirmed = *result == DialogResult::Ok;
//     self.delete_stocks_dialog = None;
//     if confirmed { self.DeleteStocks(cc, rec, false); }
// } else if self.delete_stocks_dialog.is_some() { busy = true; }

// Now:
if let Some(result) = self.delete_stocks_result.take() {
    let confirmed = result == DialogResult::Ok;
    self.delete_stocks_dialog = None;
    if confirmed { self.DeleteStocks(cc, rec, false); }
} else if self.delete_stocks_dialog.is_some() {
    busy = true;
}
```

Repeat for `cut_stocks_*`, `paste_stocks_*`, `interest_*`. Note the `interest_*` site also reads `interest_to_set` — keep that logic.

- [ ] **Step 15.6: Compile.**

```bash
cargo check -p emstocks
```

Expected: clean.

- [ ] **Step 15.7: Run full nextest.**

```bash
cargo-nextest ntr
```

Expected: all tests pass, including emstocks tests. Any emstocks-test failures here are real migration bugs — fix before proceeding.

- [ ] **Step 15.8: GOLDEN SUITE — first checkpoint.**

```bash
cargo test --test golden -- --test-threads=1
```

Expected: 237/6 preserved. If goldens shift, STOP — re-audit the Cycle rewrites. A golden regression at this task means the on_finish-triggered DeleteStocks/Cut/Paste/SetInterest calls no longer fire in the same ordering or frame as the legacy polling.

If green: proceed.

- [ ] **Step 15.9: Delete `emDialog::silent_cancel`.**

In `crates/emcore/src/emDialog.rs`, if a `pub fn silent_cancel` method still exists in the new impl (it shouldn't — Task 11 deleted it), delete it. This step is a safety check:

```bash
rg -n 'fn silent_cancel' crates/emcore/src/emDialog.rs
```

Expected: 0 matches. If non-zero, remove.

- [ ] **Step 15.10: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 15 — emStocksListBox migration:** COMPLETE. 4 construct sites converted to `show()` with `on_finish` Cell-writer closures. 4 Cycle polling sites read from Cell.take. 4 silent_cancel calls replaced with closure-rail `close_dialog_by_id`. Goldens preserved 237/6. Gate green.
```

```bash
git add -A
git commit -m "phase-3.5 task 15: emStocksListBox — on_finish + Cell observation, close_dialog_by_id for replacement"
```

**Task 15 exit condition:**
- `rg -n 'silent_cancel' crates/emstocks/src/emStocksListBox.rs` → 0 matches.
- `rg -n '\.GetResult\(\)' crates/emstocks/src/emStocksListBox.rs` → 0.
- Goldens 237/6.

**Reviewer dispatch — give reviewer the full diff; ask: "Did any of the 4 migrations drop a guard or change control flow? Does the silent_cancel replacement match spec §10.4?"**

---

## Task 16: Task 5.2 gate — emStocksListBox wrap-up

**Files:** none modified.

- [ ] **Step 16.1: Verify final Task-5.2 state.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```

- [ ] **Step 16.2: Append ledger wrap entry.**

```markdown
### Keystone: Task 5.2 emStocksListBox migration COMPLETE
```

No commit.

**Reviewer dispatch:** second-opinion spec + code-quality pass on Tasks 14+15 as a bundle.

---

## Task 17: `emFileDialog` — delete dead API

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`

**Design:** Per spec §11.1. Delete `set_mode` (emFileDialog.rs:92) and `dialog_mut` (emFileDialog.rs:180). Zero live callers.

- [ ] **Step 17.1: Confirm zero live callers (should already be the case).**

```bash
rg -n '\.set_mode\(|dialog_mut\(' crates/ 2>/dev/null | grep -v 'emFileDialog.rs'
```

Expected: 0 external matches.

- [ ] **Step 17.2: Delete.**

Remove the two methods entirely. Also remove their stale comment reference near emFileDialog.rs:614.

- [ ] **Step 17.3: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 17 — emFileDialog dead-code delete:** COMPLETE. Removed `set_mode` and `dialog_mut` (zero live callers, non-public-API concern).
```

```bash
git add crates/emcore/src/emFileDialog.rs
git commit -m "phase-3.5 task 17: delete dead emFileDialog::set_mode + dialog_mut"
```

**Reviewer dispatch.**

---

## Task 18: `emFileDialog` — migrate construction

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`

**Design:** Per spec §11.2, §11.3. Add `look: Rc<emLook>` field (so overwrite_dialog construction doesn't rely on the deleted `emDialog::look()` accessor). Rewrite `emFileDialog::new` to call `.AddCustomButton(ctx, ...)` and `.show(ctx)`.

- [ ] **Step 18.1: Add `look` field.**

```rust
pub struct emFileDialog {
    dialog: emDialog,
    look: Rc<emLook>,  // NEW
    fsb: /* ... */,
    mode: FileDialogMode,
    overwrite_dialog: Option<emDialog>,
    overwrite_result: Rc<Cell<Option<DialogResult>>>,  // NEW (Task 20)
    // ... existing
}
```

- [ ] **Step 18.2: Rewrite `emFileDialog::new`.**

```rust
impl emFileDialog {
    pub fn new<C: ConstructCtx>(ctx: &mut C, mode: FileDialogMode, look: Rc<emLook>) -> Self {
        let (title, ok_label) = mode_title_and_ok(mode);
        let mut dialog = emDialog::new(ctx, title, Rc::clone(&look));
        dialog.AddCustomButton(ctx, ok_label, DialogResult::Ok);
        dialog.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
        dialog.show(ctx);
        Self {
            dialog,
            look,
            // ... fsb init etc
            overwrite_dialog: None,
            overwrite_result: Rc::new(Cell::new(None)),
            // ...
        }
    }
}
```

**Wait — should `new` `show()` here?** The dialog becomes visible immediately on construction. That matches C++ `emDialog` semantics (visible on construction). Preserve.

- [ ] **Step 18.3: Replace `self.dialog.look().clone()` with `self.look.clone()` in emFileDialog.rs:~273.**

- [ ] **Step 18.4: Compile.**

```bash
cargo check -p emcore
```

Expected: clean.

- [ ] **Step 18.5: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 18 — emFileDialog construction:** COMPLETE. `new` uses new emDialog API with ctx-threading + show. `look` held on emFileDialog directly.
```

```bash
git add crates/emcore/src/emFileDialog.rs
git commit -m "phase-3.5 task 18: emFileDialog construction via new emDialog handle + show"
```

**Reviewer dispatch.**

---

## Task 19: `emFileDialog::CheckFinish` — overwrite dialog migration

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`

**Design:** Per spec §11.3. When `CheckFinish` encounters an existing save target, it creates an overwrite-confirmation dialog. Migrate this construction path.

- [ ] **Step 19.1: Rewrite overwrite_dialog creation (emFileDialog.rs:~270-277).**

```rust
// Was:
// let mut dlg = emDialog::new(ctx, "File Exists", self.dialog.look().clone());
// dlg.AddCustomButton("OK", DialogResult::Ok);
// dlg.AddCustomButton("Cancel", DialogResult::Cancel);
// self.overwrite_dialog = Some(dlg);

// Now:
let mut dlg = emDialog::new(ctx, "File Exists", self.look.clone());
dlg.AddCustomButton(ctx, "OK", DialogResult::Ok);
dlg.AddCustomButton(ctx, "Cancel", DialogResult::Cancel);
let cell = Rc::clone(&self.overwrite_result);
dlg.set_on_finish(Box::new(move |r, _sched| cell.set(Some(r.clone()))));
dlg.show(ctx);
self.overwrite_dialog = Some(dlg);
```

- [ ] **Step 19.2: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 19 — emFileDialog CheckFinish overwrite dialog:** COMPLETE. Overwrite dialog uses new handle + on_finish + show. Cell written by closure.
```

```bash
git add crates/emcore/src/emFileDialog.rs
git commit -m "phase-3.5 task 19: emFileDialog CheckFinish overwrite dialog uses new handle"
```

**Reviewer dispatch.**

---

## Task 20: `emFileDialog::Cycle` — Cell shim for `overwrite_result`

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`

**Design:** Per spec §11.4. Rewrite the overwrite-polling branch in `Cycle` to read from `self.overwrite_result.take()` instead of `overwrite_dialog.GetResult()`. Preserve `Cycle` signature and caller contract.

- [ ] **Step 20.1: Rewrite `Cycle` body (emFileDialog.rs:~329-370).**

```rust
// Was:
// } else if let Some(od) = self.overwrite_dialog.as_ref() {
//     if /* overwrite dialog fired */ {
//         match od.GetResult() {
//             Some(DialogResult::Ok) => Action::OverwriteConfirmed,
//             Some(DialogResult::Cancel) => Action::OverwriteCancelled,
//             _ => /* still pending */
//         }
//     }
// }

// Now:
if self.overwrite_dialog.is_some() {
    match self.overwrite_result.take() {
        Some(DialogResult::Ok) => Action::OverwriteConfirmed,
        Some(DialogResult::Cancel) => Action::OverwriteCancelled,
        Some(DialogResult::Custom(_)) | None => Action::NoAction,
    }
}
```

(Exact structure depends on the existing `Action` enum in emFileDialog.rs:328-332. Match the existing match arms.)

- [ ] **Step 20.2: `OverwriteConfirmed` / `OverwriteCancelled` execution.**

Match arm bodies stay as-is (except `od.GetResult` was the only legacy call). Verify:

```bash
rg -n '\.GetResult\(\)' crates/emcore/src/emFileDialog.rs
```

Expected: 0.

- [ ] **Step 20.3: Also migrate the `silent_cancel` call at emFileDialog.rs:~361-367 (in `OverwriteConfirmed` branch).**

```bash
rg -n 'silent_cancel\|\.overwrite_dialog\s*=\s*None' crates/emcore/src/emFileDialog.rs
```

If the current code does `self.overwrite_dialog = None` without a silent_cancel, good — the handle drops. But the window is still alive in `app.windows`. Enqueue a `close_dialog_by_id` closure:

```rust
Action::OverwriteConfirmed => {
    if let Some(od) = self.overwrite_dialog.take() {
        let did = od.dialog_id;
        ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
            app.close_dialog_by_id(did);
        }));
    }
    // Finish the outer dialog with Ok.
    // /* existing logic: self.dialog has no .Finish method — finish via closure rail too */
    let outer_did = self.dialog.dialog_id;
    // Finish via programmatic result injection. We no longer have emDialog::Finish.
    // Phase 3.6 provides DeferredAction::MutateDialog(did, Finish(Ok)); for 3.5,
    // we write directly into the outer dialog's DlgPanel.pending_result.
    /* ... requires post-show mutation — BUT emFileDialog calls this from inside
       CheckFinish which has ctx. The outer dialog has been shown. So we're
       in the post-show mutation case the spec explicitly defers to Phase 3.6.  */
```

**WAIT — this is a spec-level gap.** The current `emFileDialog::Cycle` does:
1. If fsb fires: call `self.dialog.Finish(DialogResult::Ok, ctx)`.
2. If overwrite Ok: call `self.dialog.Finish(DialogResult::Ok, ctx)`.

Both calls are **post-show mutations** of the outer dialog. Spec defers these to Phase 3.6. But they're live now!

**Resolution:** Phase 3.5 must ship a programmatic-finish path for the outer dialog. Spec §Deferred mentions `DialogMutation::Finish(DialogResult)`. To ship 3.5, we need the minimal form *now*.

Two options:
- **(a)** Add `DeferredAction::MutateDialog(DialogId, DialogMutation::Finish(DialogResult))` and the drain handler, scoping to exactly one mutation variant. Spec explicitly calls this out as Phase 3.6 scope.
- **(b)** Have `emFileDialog::Cycle` synthesize a fire of the outer dialog's close_signal instead — `close_signal` is already wired to DialogPrivateEngine's cancel path. But the result is Cancel, not Ok. Wrong semantics.
- **(c)** Expose programmatic finish via a new method `emDialog::finish_post_show(result, ctx)` that pushes a closure setting `DlgPanel.pending_result = Some(result)`. Minimum machinery.

**Go with (c)** — it's the most minimal. Add an `emDialog::finish_post_show(result, ctx)` method that mutates post-show via closure rail. This is NOT the general post-show mutation case — it's specifically the `Finish(r)` case, which is the one and only post-show write emFileDialog needs.

Update plan: insert a sub-step to add `emDialog::finish_post_show`. Do it here rather than earlier because only emFileDialog needs it.

- [ ] **Step 20.4: Add `emDialog::finish_post_show`.**

Edit `crates/emcore/src/emDialog.rs`, append to `impl emDialog`:

```rust
impl emDialog {
    /// Programmatically finish the dialog post-show with the given result.
    /// Port of C++ `emDialog::Finish(int)` (emDialog.cpp:146-153) for the
    /// post-show case. Phase 3.5 exposes this narrowly (`emFileDialog::Cycle`
    /// is the only live consumer); Phase 3.6 generalizes to full
    /// `App::mutate_dialog_by_id`.
    ///
    /// Pushes a closure that sets `DlgPanel.pending_result = Some(result)`,
    /// which `DialogPrivateEngine::Cycle` picks up on the next tick and
    /// routes through the normal finalize → fire(finish_signal) → invoke
    /// on_finish sequence. Matches C++ `Finish` behavior: CheckFinish still
    /// runs via DialogPrivateEngine's normal flow.
    pub fn finish_post_show<C: ConstructCtx>(&self, ctx: &mut C, result: DialogResult) {
        let did = self.dialog_id;
        let root_panel_id = self.root_panel_id;
        ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
            if let Some(&wid) = app.dialog_windows.get(&did) {
                if let Some(win) = app.windows.get_mut(&wid) {
                    let mut tree = win.take_tree();
                    if let Some(mut b) = tree.take_behavior(root_panel_id) {
                        if let Some(dlg) = b.as_dlg_panel_mut() {
                            if dlg.pending_result.is_none() && dlg.finalized_result.is_none() {
                                dlg.pending_result = Some(result);
                            }
                        }
                        tree.put_behavior(root_panel_id, b);
                    }
                    win.put_tree(tree);
                }
                // Wake DialogPrivateEngine: after finalize it's asleep, and no
                // signal firing would route it to read pending_result. Look up
                // engines scoped to Toplevel(wid) (there is exactly one — the
                // DialogPrivateEngine) and wake them.
                for eid in app
                    .scheduler
                    .engines_for_scope(crate::emPanelScope::PanelScope::Toplevel(wid))
                {
                    app.scheduler.wake_up(eid);
                }
            }
        }));
    }
}
```

**Engine-wake rationale:** after `FinishState == 0` / `!ADEnabled` branch returns false, `DialogPrivateEngine` sleeps until a connected signal fires. Direct mutation of `pending_result` via `finish_post_show` does not fire any connected signal, so the engine would never observe the mutation. The closure body explicitly calls `scheduler.wake_up(eid)` to force one cycle. `engines_for_scope(Toplevel(wid))` returns exactly one engine for a dialog window (the `DialogPrivateEngine`); the loop is a single-element iteration.

- [ ] **Step 20.5: Add test for `finish_post_show`.**

```rust
#[test]
fn finish_post_show_sets_pending_result() {
    /* Build + show + DoTimeSlice to install, then call dlg.finish_post_show(Ok),
       drain pending_actions, DoTimeSlice again, assert finalized_result == Ok. */
}
```

- [ ] **Step 20.6: Migrate `emFileDialog::Cycle` to use `finish_post_show`.**

```rust
Action::FinishOk => {
    // Was: self.dialog.Finish(DialogResult::Ok, ctx);
    self.dialog.finish_post_show(ctx, DialogResult::Ok);
}
Action::OverwriteConfirmed => {
    if let Some(od) = self.overwrite_dialog.take() {
        let did = od.dialog_id;
        ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
            app.close_dialog_by_id(did);
        }));
    }
    self.dialog.finish_post_show(ctx, DialogResult::Ok);
}
Action::OverwriteCancelled => {
    if let Some(od) = self.overwrite_dialog.take() {
        let did = od.dialog_id;
        ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
            app.close_dialog_by_id(did);
        }));
    }
    /* outer dialog stays; user may try save again */
}
```

- [ ] **Step 20.7: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 20 — emFileDialog Cycle Cell shim + finish_post_show:** COMPLETE. Overwrite-read site reads from `self.overwrite_result.take()`. Outer-dialog programmatic finish goes via new `emDialog::finish_post_show` (closure-rail mutation of DlgPanel.pending_result). Overwrite-dialog close uses close_dialog_by_id.

  Deviation from spec: spec §Deferred §"Post-show dialog mutation" said this lands in Phase 3.6; but emFileDialog's live Finish(Ok) calls force a minimal post-show mutation path in 3.5. Added narrowly-scoped `finish_post_show` only; general `App::mutate_dialog_by_id` still Phase 3.6.
```

```bash
git add -A
git commit -m "phase-3.5 task 20: emFileDialog Cycle Cell shim + emDialog::finish_post_show"
```

**Task 20 exit condition:**
- `rg -n '\.GetResult\(\)' crates/emcore/src/emFileDialog.rs` → 0.
- `rg -n '\.Finish\(' crates/emcore/src/emFileDialog.rs` → 0 (replaced by finish_post_show).
- `rg -n 'fn finish_post_show' crates/emcore/src/emDialog.rs` → 1.

**Reviewer dispatch — flag the spec deviation explicitly. Ask: is narrow `finish_post_show` the right 3.5-scope resolution, or should this block and Phase 3.6 land first?**

---

## Task 21: emFileDialog tests — verify migration

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs` test module

- [ ] **Step 21.1: Port existing emFileDialog tests to new shape.**

Tests reference deleted `set_mode`, `dialog_mut`, old `Finish`, `GetResult`. Port each:
- `dialog_mode` — still works, no dialog state manipulation.
- `dialog_cancel_always_allowed` — still works (CheckFinish Cancel short-circuit).
- `dialog_open_no_selection_error` — still works (CheckFinish returns Error without creating overwrite dialog).
- `test_force_overwrite_result` (if it exists) — port to set `self.overwrite_result.set(Some(result))` directly.

Add one end-to-end test:
- `save_existing_file_triggers_overwrite_dialog_and_confirms`: Save mode, CheckFinish returns ConfirmOverwrite, Cell receives Ok after button click, next Cycle finishes outer dialog with Ok via finish_post_show.

- [ ] **Step 21.2: Gate + commit.**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Append ledger:

```markdown
- **Task 21 — emFileDialog tests ported:** COMPLETE. Legacy GetResult / set_mode / dialog_mut test references removed. End-to-end overwrite-confirm test added.
```

```bash
git add crates/emcore/src/emFileDialog.rs
git commit -m "phase-3.5 task 21: emFileDialog tests ported to new handle + finish_post_show"
```

**Reviewer dispatch.**

---

## Task 22: Task 5.3 gate — emFileDialog wrap-up + second golden checkpoint

```
### Keystone: Task 5.3 emFileDialog migration COMPLETE

Tasks 17–21 landed: dead-API delete (set_mode, dialog_mut), new emDialog API + show in emFileDialog::new, CheckFinish overwrite dialog migrated to on_finish+Cell+show, emFileDialog::Cycle rewritten to Cell::take polling + closure-rail close_dialog_by_id + emDialog::finish_post_show for post-show programmatic finish, 11 tests restored + 1 new e2e test. Goldens preserved 237/6. Nextest 2510/0/9.

Stub deletion audit (Task 22):
- GetResult: deleted. Zero live callers (only comment refs in emFileDialog.rs).
- Finish: deleted. Zero live callers (only comment ref in emGUIFramework.rs).
- silent_cancel: deleted. Zero live callers (only comment ref in emGUIFramework.rs).
- look(): deleted. Zero live callers (rg confirmed no `.look()` calls in crates/).
```

- [ ] **Step 22.1: Full gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
cargo test --test golden -- --test-threads=1
```

Expected: clean. Goldens 237/6.

- [ ] **Step 22.2: Append keystone ledger.**

```markdown
### Keystone: Task 5.3 emFileDialog migration COMPLETE

Goldens preserved (237/6). All consumer migrations shipped. Ready for closeout.
```

No commit.

**Reviewer dispatch:** second-opinion spec + code-quality pass on Tasks 17-21 as a bundle.

---

## Task 23: `emWindow` `pub` audit — narrow visibility where possible

**Files:**
- Modify (possibly): `crates/emcore/src/emWindow.rs`

**Design:** Per `project_phase35a_pub_narrow.md`, `emWindow::{tree, take_tree, put_tree}` were widened to `pub` in 3.5.A Task 7 for emmain cross-crate callers. Task 23 audits whether any of them can narrow to `pub(crate)` now that Phase 3.5 landed new callers.

- [ ] **Step 23.1: Audit each.**

```bash
rg -n 'take_tree\(\)\|put_tree\(\|\.tree\b' crates/ --type rust | grep -v '//'
```

For each caller, check which crate it's in. If all callers are inside `emcore`, narrow to `pub(crate)`. If any are in `emmain`, keep `pub`.

From 3.5.A ledger: `emmain::emMainWindow.rs:190, 922, 979, 993, 1124` used `tree`, `take_tree`, `put_tree` — `pub` required. Task 23 likely doesn't narrow these.

But: Phase 3.5 doesn't add new cross-crate callers. So no change is expected. If zero narrowing applies, task is a no-op confirmation.

- [ ] **Step 23.2: Document and optionally commit.**

If narrowing applies: commit with `phase-3.5 task 23: narrow emWindow visibility where feasible`.

If no narrowing: append ledger note only, no commit:

```markdown
- **Task 23 — emWindow pub audit:** COMPLETE. No narrowings feasible in Phase 3.5 — cross-crate emmain callers documented in `project_phase35a_pub_narrow.md` remain. Visibility surface unchanged.
```

**Reviewer dispatch** (even if no commit — gets an observational stamp).

---

## Task 24: Closeout — ledger + tag

**Files:**
- Modify: `docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md`
- Modify: `/home/a0/.claude/projects/-home-a0-git-eaglemode-rs/memory/project_phase35a_pub_narrow.md` (closeout memory)
- Tag: `port-rewrite-phase-3-5-complete`

- [ ] **Step 24.1: Finalize ledger with closeout summary.**

Append:

```markdown
## Closeout

All tasks 1-23 green. Gate final: nextest <X>/0/9, goldens 237/6, clippy clean.

Invariants I5a-I5j verified. Spec compliance: all in-scope items shipped; deferred items (post-show mutation beyond `finish_post_show`, emFileDialog full engine migration) land in Phase 3.6.

Deviations from spec:
- Added narrow `emDialog::finish_post_show` in Task 20 instead of deferring wholesale to Phase 3.6, because emFileDialog::Cycle has live post-show Finish(Ok) calls that cannot ship without the path.

Follow-ups:
- Phase 3.6 emFileDialog full engine subscription + general `App::mutate_dialog_by_id`.
```

- [ ] **Step 24.2: Update memory.**

Update `project_phase35a_pub_narrow.md` status from "DONE — re-widened in Task 7" to include "Phase 3.5 Task 23 re-audit: no further narrowings applied; emmain cross-crate `pub` surface unchanged."

Or, if project memory feels stale, delete and replace with a new `project_phase35_completion.md` summarizing the final state.

- [ ] **Step 24.3: Commit + tag.**

```bash
git add docs/superpowers/notes/2026-04-23-phase-3-5-ledger.md
git commit -m "phase-3.5 closeout: ledger — all invariants verified, gate green"
git tag port-rewrite-phase-3-5-complete
```

**Task 24 exit condition:**
- Ledger closeout section populated.
- Tag `port-rewrite-phase-3-5-complete` at HEAD of the branch.
- Gate green.

- [ ] **Step 24.4: Final report.**

Report task-by-task summary to user. State: "Phase 3.5 complete at tag `port-rewrite-phase-3-5-complete`. Branch `port-rewrite/phase-3-5-deferred-dialog-construction` ready for review/merge. Phase 3.6 (emFileDialog full engine) unblocked."

---

## Self-review

After writing this plan, re-read the spec end-to-end and verify task coverage:

- **Spec §1 Single emDialog type:** Task 7 ✓
- **Spec §2 Construction:** Task 7 ✓
- **Spec §3 Mutators:** Task 8 ✓
- **Spec §4 show():** Task 9 ✓
- **Spec §5 Closure-rail deferral:** Task 10 (auto-delete rewrite) + Task 15 (silent_cancel) ✓
- **Spec §6 engines_for_scope:** Task 3 ✓
- **Spec §7 ConstructCtx extensions:** Task 2 + Task 4 ✓
- **Spec §8 next_dialog_id relocation:** Task 3 ✓
- **Spec §9 on_finish + Rc<Cell>:** Task 15 (emStocksListBox) + Task 19 (emFileDialog overwrite) + Task 20 (emFileDialog outer) ✓
- **Spec §10 emStocksListBox migration:** Tasks 14-15-16 ✓
- **Spec §11 emFileDialog migration:** Tasks 17-21-22 ✓
- **Spec §Audit mutator-after-show:** Task 8 panic contract + test ✓
- **Spec §App::close_dialog_by_id:** Task 10 ✓
- **Spec §Deferred post-show mutation:** Task 20 adds narrow `finish_post_show` (flagged as spec deviation) ✓
- **Spec §Deferred emFileDialog full observer migration:** confirmed deferred ✓
- **Spec §Deferred ShowMessage:** Task 9 shim ✓
- **Spec §Invariants I5a-I5j:** Task 13 keystone verifies ✓
- **Spec §Phased execution:** Task 5.1 → Tasks 2-13; Task 5.2 → Tasks 14-16; Task 5.3 → Tasks 17-22; Task 5.4 → Tasks 23-24 ✓
- **Spec §Risks — DialogPrivateEngine construction-at-install:** Task 5 ✓
- **Spec §Risks — mutators requiring ctx:** Task 8 `AddCustomButton(&mut self, ctx, ...)` signature ✓
- **Spec §Risks — Rc<Cell> post-handle-drop:** Task 15 handles via `.take()` before drop ✓
- **Spec §Risks — orphaned signals:** `close_dialog_by_id` lets slotmap handle (Task 10 comment) ✓
- **Spec §Test strategy:** Tests across Tasks 3, 5, 7, 8, 9, 10, 12, 15, 20, 21 ✓

No spec gaps detected.

**Placeholder scan:** `rg -n '(TBD|TODO|fill in|implement later|similar to)' docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-deferred-dialog-construction.md` — expected to return 0 matches of those placeholder phrases used as real placeholders (matches inside code comments explaining C++ parity are not placeholders).
