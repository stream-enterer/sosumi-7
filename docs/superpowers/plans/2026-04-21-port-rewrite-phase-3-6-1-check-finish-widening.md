# Phase 3.6.1 — Widen `DialogCheckFinishCb`; close P3 divergence — Plan

**Spec:** `docs/superpowers/specs/2026-04-21-phase-3-6-1-check-finish-widening-design.md`
**Branch:** `port-rewrite/phase-3-6-1-check-finish-widening` off `main` (post-3.6-merge, `3ff16780`)
**Baseline:** nextest 2512/0/9, clippy clean, goldens 237/6 preserved.
**Exit tag:** `port-rewrite-phase-3-6-1-complete`.

**Gate after each committed task:**
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo-nextest ntr`

---

## Task 1 — Widen the callback type + migrate existing callers

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`
- Modify: `docs/superpowers/notes/2026-04-21-phase-3-6-1-ledger.md` (create)

### Step 1.1 — Widen the type alias

In `emDialog.rs` near the existing `DialogCheckFinishCb` type alias (around line 34-38), change:

```rust
type DialogCheckFinishCb = Box<dyn FnMut(&DialogResult) -> bool>;
```

to:

```rust
/// DIVERGED: C++ `emDialog::CheckFinish` is a virtual method with no
/// extra args — subclasses reach into self's fields directly. Rust uses
/// a callback slot on `DlgPanel`; the closure needs `&mut DlgPanel` +
/// `&mut EngineCtx<'_>` to read tree state (e.g. emFileDialog's fsb
/// child panel) and spawn transient sub-dialogs. Matches `DialogCycleExt`
/// (Phase 3.6 Task 2).
pub(crate) type DialogCheckFinishCb =
    Box<dyn FnMut(&DialogResult, &mut DlgPanel, &mut crate::emEngineCtx::EngineCtx<'_>) -> bool>;
```

Flip visibility to `pub(crate)` (matches `DialogCycleExt`).

### Step 1.2 — Swap-out at the call site

At `emDialog.rs:890-894` inside `DialogPrivateEngine::Cycle` step 3, replace:

```rust
let vetoed = if let Some(cb) = dlg.on_check_finish.as_mut() {
    !cb(&pending)
} else {
    false
};
```

with the swap-out pattern (mirrors `on_cycle_ext` at :973-977):

```rust
let vetoed = if let Some(mut cb) = dlg.on_check_finish.take() {
    let vetoed = !cb(&pending, dlg, ctx);
    dlg.on_check_finish = Some(cb);
    vetoed
} else {
    false
};
```

### Step 1.3 — Migrate existing test callers

Grep `rg -n 'set_on_check_finish\|on_check_finish' crates/` to find callers.
Expected sites (in `emDialog.rs` tests):
- Line ~1813: `Box::new(|_r| true)` → `Box::new(|_r, _dlg, _ctx| true)`.
- Line ~1834: same pattern.
- Line ~2119: `Box::new(move |_r| { ... })` → `Box::new(move |_r, _dlg, _ctx| { ... })`.

Signature-only changes; no behavioral change.

### Step 1.4 — Add one test asserting widened args reachable

Append a `#[cfg(test)]` test inside `emDialog.rs` that:
1. Builds a dialog, sets an `on_check_finish` closure capturing an `Rc<Cell<bool>>` flag.
2. The closure mutates `dlg.finalized_result` or similar readable field via the new `&mut DlgPanel` arg (or sets a flag via the existing close capture) — just enough to prove the DlgPanel arg is genuinely mutable.
3. Also reads `ctx.engine_id` to prove the EngineCtx arg is reachable.
4. Drives the dialog to fire pending_result → step 3 → closure → returns true (don't veto).
5. Asserts the closure ran AND the DlgPanel/EngineCtx observations landed.

Use `Rc<Cell<T>>` for capture (not `RefCell`).

### Step 1.5 — Gate

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: 2513/0/9 (baseline 2512 + 1 new test).

### Step 1.6 — Two-commit cadence

Commit 1 (code):
```
phase-3.6.1 task 1: widen DialogCheckFinishCb to (result, &mut DlgPanel, &mut EngineCtx)

Mirrors DialogCycleExt shape. Swap-out pattern at DialogPrivateEngine::Cycle
step 3 avoids double-borrow. Existing test closures migrated to new signature
(no behavior change). One new test asserts widened args are reachable.

emFileDialog does NOT yet install on_check_finish — Task 2 lands that.
Gate green — nextest 2513/0/9.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Commit 2 (ledger):
```
phase-3.6.1 task 1 ledger: callback widening landed
```
Ledger file `docs/superpowers/notes/2026-04-21-phase-3-6-1-ledger.md` created with:
```
# Phase 3.6.1 — DialogCheckFinishCb widening — Ledger

**Started:** 2026-04-21
**Branch:** port-rewrite/phase-3-6-1-check-finish-widening (off main post-3.6-merge)
**Baseline:** nextest 2512/0/9, clippy clean, goldens 237/6.
**Plan:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-1-check-finish-widening.md
**Spec:** docs/superpowers/specs/2026-04-21-phase-3-6-1-check-finish-widening-design.md

## Task log

- **Task 1 — Widen DialogCheckFinishCb:** COMPLETE. Type alias now
  `FnMut(&DialogResult, &mut DlgPanel, &mut EngineCtx<'_>) -> bool` and
  `pub(crate)`. Swap-out pattern at DialogPrivateEngine::Cycle step 3
  mirrors on_cycle_ext. DIVERGED marker added at the alias explaining
  the C++ virtual-method → Rust callback-slot widening. Existing test
  closures migrated (signature-only). One new test asserts widened args.
  Gate green — nextest 2513/0/9.
```

---

## Task 2 — Install `emFileDialog`'s validation closure; close P3

**Files:**
- Modify: `crates/emcore/src/emFileDialog.rs`
- Modify: `crates/emcore/src/emDialog.rs` (if a helper free fn needs a shared home)
- Modify: `docs/superpowers/notes/2026-04-21-phase-3-6-1-ledger.md`

### Step 2.1 — Extract shared `file_dialog_check_finish` free fn

Current state: `emFileDialog::CheckFinish(&mut self, ctx, result) -> FileDialogCheckResult` contains the full validation body (fsb read via tree take/put, dir-check, Open-mode existence-check, Save-mode overwrite-check + OD spawn).

Refactor the body into a free function (or a `pub(crate)` method on `emFileDialog` that takes specifically what the closure can capture):

```rust
pub(crate) fn run_file_dialog_check_finish(
    ctx: &mut crate::emEngineCtx::EngineCtx<'_>,
    outer_dlg: &mut DlgPanel,
    fsb_panel_id: PanelId,
    mode: FileDialogMode,
    dir_allowed: bool,
    look: Rc<emLook>,
    result: &DialogResult,
) -> bool  // true = allow finish, false = veto
```

Body performs the same read + dir-check + Open/Save logic, but instead of
returning `FileDialogCheckResult::ConfirmOverwrite(paths)`, it directly
spawns OD via `emDialog::new(ctx, ...)` + `AddCustomButton` + OD finish
signal subscription + park on `outer_dlg.overwrite_dialog`, then returns
`false` (veto).

The existing `emFileDialog::CheckFinish(&mut self, ctx, result)` public
method stays — its body becomes a thin wrapper that reaches outer_dlg via
the outer's tree-take and calls `run_file_dialog_check_finish`. Returns
`FileDialogCheckResult` for external test callers. Preserve current test
behavior.

### Step 2.2 — Install `on_check_finish` in `emFileDialog::new`

In `emFileDialog::new` (after `on_cycle_ext` installation), build and
install a `DialogCheckFinishCb` closure. Captures: `fsb_panel_id`,
`mode`, `dir_allowed` (initial value — note: `dir_allowed` can change via
`set_directory_result_allowed`; closure captures a _copy_ of the initial,
which is a divergence if the setter is called mid-life). Resolution:
make `dir_allowed` live on `DlgPanel` (not on `emFileDialog`) so the
closure can read it through its `&mut DlgPanel` arg fresh on each fire.
Same reasoning as `overwrite_confirmed` / `overwrite_asked` in Phase 3.6
Task 3 fix.

**Sub-step: move `dir_allowed` and `mode` onto `DlgPanel`**:
- Add `pub(crate) file_dialog_dir_allowed: bool` and `pub(crate) file_dialog_mode: Option<FileDialogMode>` to `DlgPanel` (Option because non-file-dialog DlgPanels have no mode). Initialise to `false` / `None` in `DlgPanel::new`.
- `DIVERGED:` comments on both: "Rust-only — the `on_check_finish` closure has `&mut DlgPanel` but not `&mut emFileDialog`; file-dialog state the closure reads per-call lives on DlgPanel to avoid stale closure captures."
- In `emFileDialog::new`, write these fields via `with_dlg_panel_mut` pre-show.
- In `emFileDialog::set_mode` and `emFileDialog::set_directory_result_allowed`, update the DlgPanel-side fields too (pre-show via `with_dlg_panel_mut`; post-show via `pending_actions → mutate_dialog_by_id`).
- The original fields on `emFileDialog` stay as the authoritative pre-show write target + the outward API; the DlgPanel copies are the closure's read path.

### Step 2.3 — Closure body

```rust
let on_check_finish: DialogCheckFinishCb = Box::new(
    move |result: &DialogResult,
          outer_dlg: &mut DlgPanel,
          ctx: &mut crate::emEngineCtx::EngineCtx<'_>|
    -> bool {
        let fsb_id = outer_dlg.fsb_panel_id_for_check_finish;  // new field? see below
        let mode = outer_dlg.file_dialog_mode.expect("emFileDialog mode set");
        let dir_allowed = outer_dlg.file_dialog_dir_allowed;
        let look_rc = outer_dlg.look.clone();
        run_file_dialog_check_finish(ctx, outer_dlg, fsb_id, mode, dir_allowed, look_rc, result)
    }
);
```

The closure captures nothing — all state is on `outer_dlg`. That avoids
stale-capture divergence.

Add `outer_dlg.fsb_panel_id: Option<PanelId>` field too (same reasoning).

### Step 2.4 — Simplify `on_cycle_ext` file-trigger branch

Remove the P3 `DIVERGED:` marker on the closure (the one explaining
"file-trigger path doesn't re-enter CheckFinish"). That divergence is
now closed — setting `pending_result = Ok` on file-trigger triggers base
cycle step 3 which runs the widened `on_check_finish`. Single funnel,
matches C++.

The file-trigger branch becomes:
```rust
if ctx.scheduler.is_signaled_for_engine(closure_fsb_sig, ctx.engine_id) {
    dlg.pending_result = Some(DialogResult::Ok);
}
```

(likely unchanged from Phase 3.6 Task 3 fix — just delete the DIVERGED
comment that explained the validation skip).

### Step 2.5 — Gate

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: 2513/0/9 (no new tests in Task 2 — the validation funnel is
already exercised by existing `CheckFinish` tests + the Task-1 widened-args
test; full end-to-end is E040-dependent). If any existing test regresses
due to validation-funnel re-entry firing unexpectedly, investigate.

### Step 2.6 — Two-commit cadence

Commit 1 (code):
```
phase-3.6.1 task 2: emFileDialog installs on_check_finish; P3 divergence closed

Validation funnel now runs through DialogPrivateEngine::Cycle step 3:
both file-trigger and button-click OK paths re-enter the widened
on_check_finish closure, which reads fsb state via tree take/put and
spawns OD when Save-mode overwrite conflicts arise.

mode + dir_allowed + fsb_panel_id relocated to DlgPanel (closure has
&mut DlgPanel, not &mut emFileDialog). emFileDialog public API (set_mode,
set_directory_result_allowed) writes to both struct + DlgPanel via
with_dlg_panel_mut / mutate_dialog_by_id.

run_file_dialog_check_finish free fn extracted; emFileDialog::CheckFinish
public wrapper retained for external test callers. P3 DIVERGED marker on
on_cycle_ext's file-trigger branch removed.

Gate green — nextest 2513/0/9.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

Commit 2 (ledger):
```
- **Task 2 — emFileDialog validation funnel:** COMPLETE. on_check_finish
  installed on outer DlgPanel; reads fsb state + spawns OD; returns
  false on any error/dir/overwrite-needed path. mode + dir_allowed +
  fsb_panel_id mirrored to DlgPanel so the 'static closure reads fresh
  state per-call. run_file_dialog_check_finish free fn shared between
  the closure and the public emFileDialog::CheckFinish wrapper.
  P3 DIVERGED marker on on_cycle_ext removed. Gate green.
```

---

## Closeout

### Step C.1 — Run invariant checks

- `rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs` — expect 1.
- `rg -n 'pub fn Cycle.*PanelCtx' crates/emcore/src/emDialog.rs crates/emcore/src/emFileDialog.rs` — expect 0.
- `rg -n 'DIVERGED:.*P3' crates/emcore/src/emFileDialog.rs` — expect 0 (closed).
- `rg -n '\.Cycle\(' crates/emcore/src/emFileDialog.rs` — expect 0.
- Goldens: `cargo test --test golden -- --test-threads=1` — expect 237/6.

### Step C.2 — Closeout note

`docs/superpowers/notes/2026-04-21-phase-3-6-1-closeout.md` summarising scope + delivery + E040 status.

### Step C.3 — Tag + merge

```bash
git tag port-rewrite-phase-3-6-1-complete
```

Then merge to main per repo convention (ask user first — if explicitly authorized for unattended workflow, proceed).

## Non-goals (repeated for clarity)

- No changes to the winit WindowId infra. E040 stays open.
- No new production features beyond the funnel closure + DlgPanel field mirrors.
- No changes to emStocksListBox or other dialog consumers (they don't install `on_check_finish`).
