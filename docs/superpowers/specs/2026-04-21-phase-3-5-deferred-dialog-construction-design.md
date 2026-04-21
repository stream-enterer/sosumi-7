# Phase 3.5 — Deferred `emDialog` Construction + Consumer Migration

**Date:** 2026-04-21
**Branch (base):** `port-rewrite/phase-3-5-a-runtime-toplevel-windows` at `586d6af5` (tagged `port-rewrite-phase-3-5-a-complete`).
**New branch:** `port-rewrite/phase-3-5-deferred-dialog-construction`.
**Supersedes Task 5 of:** `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow-plan.md`.
**JSON entries:** closes unblocker for E024 via Phase 3.6 enablement.

## Problem

Phase 3.5.A shipped runtime top-level-window installation (`DialogId`, `PendingTopLevel`, `App::pending_top_level`, `install_pending_top_level`, `App::dialog_window_mut`, `App::allocate_dialog_id`, `emWindow::new_top_level_pending`), and the test-gated `DlgPanel` / `DlgButton` / `DialogPrivateEngine` trio in `emDialog.rs`. The legacy `emDialog` public API (`Finish`, `GetResult`, `AddOKButton`, `SetRootTitle`, `silent_cancel`, `on_finish=`, `Input`, `Paint`, `LayoutChildren`) is still in place; consumers (`emStocksListBox`, `emFileDialog`) still call it and still poll `GetResult` in their `Cycle` methods.

The blocker: those mutators and the underlying state they manipulate are, under the 3.5.A shape, owned by a `DlgPanel` behavior sitting inside `app.windows[wid].tree`. Reaching that `DlgPanel` requires `&mut App` — but every `Cycle` call site runs with `App` destructured by design (this destructuring is what 3.5.A built to enable per-window tree ownership). Consumers cannot keep calling the legacy mutators without either undoing 3.5.A's destructuring or introducing a deferred-mutation rail.

## Scope

One atomic reshape. `emDialog` becomes a handle to a not-yet-shown or already-shown top-level window, mutations happen on a self-owned builder state before handoff, observation happens via engine-scheduled `on_finish` callbacks, and consumers no longer poll `GetResult`.

Explicitly in scope:
- Delete legacy `emDialog` API (`Finish`, `GetResult`, `Input`, `Paint`, `LayoutChildren`, `CheckFinish`, `silent_cancel`, `preferred_size`, and the public getters `GetButton` / `GetButtonForResult` / `GetOKButton` / `GetCancelButton` / `IsAutoDeletionEnabled`) and legacy private fields (`result`, `on_finish`, `on_check_finish`, `auto_delete`, `buttons`, `border`). `look: Rc<emLook>` is retained on the handle (§3.1) so pre-show mutators can clone without touching the pending `DlgPanel`.
- Un-gate `DlgPanel`, `DlgButton`, `DialogPrivateEngine`.
- Expose `App::pending_actions` handle (the existing closure rail — `Rc<RefCell<Vec<Box<dyn FnOnce(&mut App, &ActiveEventLoop)>>>>`, drained in `about_to_wait` at `emGUIFramework.rs:906`) through `ConstructCtx`. This is the same rail `emView` already uses for popup materialization (`emView.rs:1997`); the enum rail (`emEngineCtx::DeferredAction`) is vestigial and not extended.
- Extend `ConstructCtx` trait with `fn pending_actions(&self) -> &Rc<RefCell<Vec<FrameworkDeferredAction>>>` and `fn root_context(&self) -> &Rc<emContext>`.
- Relocate `next_dialog_id` counter from `App` to `EngineScheduler`; expose `allocate_dialog_id()` on `ConstructCtx`.
- Migrate `emStocksListBox` (4 construct sites, 4 polling sites, 4 `silent_cancel` sites).
- Migrate `emFileDialog` *construction* only; delete dead `emFileDialog::set_mode` and `emFileDialog::dialog_mut`.

Explicitly out of scope (deferred to Phase 3.6):
- `emFileDialog::Cycle` / `CheckFinish` observer migration (the overwrite-dialog subscription path). Covered in `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-emfiledialog-e024.md`.
- Post-show dialog mutation (closure-rail routing via `App::mutate_dialog_by_id`). See `§Deferred to Phase 3.6` below.
- `emDialog::ShowMessage` static convenience — kept as a deprecated shim that panics until re-examined by Phase 3.6; no live callers.

## Design principles — precedent audit

Every structural decision below maps to an existing 3.5.A pattern or an existing Phase 3 ctx shape. No new ownership types, no new `Rc<RefCell>`, no `Any`.

| Concern | Existing precedent | 3.5 Task 5 reuse |
|---|---|---|
| Stable id across pending vs. materialized | `DialogId(u64)` + `App::allocate_dialog_id` (3.5.A Task 9) | Same allocator, relocated to scheduler |
| Pending → materialize via queue | `pending_top_level: Vec<PendingTopLevel>` drained by `install_pending_top_level` (3.5.A Task 9) | `emDialog::show`'s closure body calls `app.pending_top_level.push(p)` then `app.install_pending_top_level(el)` — both already exist |
| Closure-rail deferred action (`FnOnce(&mut App, &ActiveEventLoop)`) | `App::pending_actions` drained in `about_to_wait` at `emGUIFramework.rs:906`; used by `emView` for popup materialization (`emView.rs:1997` → `App::materialize_pending_popup`) | `emDialog::show` pushes a closure that calls `App::install_pending_top_level`; `silent_cancel` pushes a closure that removes the pending entry or closes the window |
| Widget callback with `SchedCtx` invocation | `WidgetCallbackRef<T>` (emEngineCtx.rs:32) already used by `DlgPanel::on_finish` | Consumer sets `on_finish` closure capturing `Rc<Cell<Option<DialogResult>>>`; no new mechanism |
| Engine-scope for dialog's private engine | `PanelScope::Toplevel(WindowId)` with deferred registration on materialize (3.5.A Task 10) | Unchanged; `PendingTopLevel::pending_private_engine` is already the rail |
| Per-window tree detach / restore | `emWindow::take_tree` / `put_tree` (3.5.A Task 7 / 9) | Used by `emDialog` mutators to reach `DlgPanel` pre-show |
| ctx-rail unification across four ctx variants | `ConstructCtx` trait with `create_signal` / `register_engine` / `wake_up` (Phase 1.5) | Extend with `pending_actions` + `allocate_dialog_id` + `root_context`; no new ctx type |

## Architecture

### 1. Single `emDialog` type with pre/post-show states

```rust
pub struct emDialog {
    pub dialog_id: DialogId,
    pub finish_signal: SignalId,
    pub close_signal: SignalId,
    /// PanelId of the `DlgPanel` root inside the pending or materialized window's tree.
    /// Stable across show(): pre-show the tree lives in `self.pending`, post-show in
    /// `app.windows[app.dialog_windows[dialog_id]].tree`.
    pub root_panel_id: PanelId,
    /// Shared look. Held on the handle so pre-show mutators (`AddCustomButton` etc.)
    /// can clone it without reaching into the pending `DlgPanel`. Keeps the legacy
    /// `emDialog::look()` semantics intact without re-exposing the accessor.
    look: Rc<emLook>,
    /// Pre-show builder state. `Some(_)` before `show()`, `None` after.
    pending: Option<PendingTopLevel>,
}
```

Identity fields (`dialog_id`, `finish_signal`, `close_signal`, `root_panel_id`) are stable across the show transition — consumers can subscribe to `finish_signal` pre-show and it will fire post-materialize.

### 2. Construction — `emDialog::new`

```rust
impl emDialog {
    pub fn new<C: ConstructCtx>(ctx: &mut C, title: &str, look: Rc<emLook>) -> Self {
        let dialog_id = ctx.allocate_dialog_id();
        let finish_signal = ctx.create_signal();
        let close_signal = ctx.create_signal();
        let flags_signal = ctx.create_signal();
        let focus_signal = ctx.create_signal();
        let geom_signal = ctx.create_signal();

        // Build tree locally.
        let mut tree = PanelTree::new();
        let root_panel_id = tree.create_root("dlg", false);
        let dlg_panel = DlgPanel::new(title, Rc::clone(&look), finish_signal);
        tree.set_behavior(root_panel_id, Box::new(dlg_panel));

        // Wrap in pending window.
        let mut window = emWindow::new_top_level_pending(
            Rc::clone(ctx.root_context()),
            WindowFlags::empty(),
            format!("emDialog-{}", dialog_id.0),
            close_signal, flags_signal, focus_signal, geom_signal,
            emColor::TRANSPARENT,
        );
        let _discarded = window.take_tree();
        window.put_tree(tree);

        // No pre-built engine. `PendingTopLevel` carries the construction
        // inputs (`close_signal`, `root_panel_id`); `install_pending_top_level`
        // builds `DialogPrivateEngine` once `window_id` is known.
        Self {
            dialog_id, finish_signal, close_signal, root_panel_id,
            look: Rc::clone(&look),
            pending: Some(PendingTopLevel {
                dialog_id,
                window,
                close_signal,
                private_engine_root_panel_id: root_panel_id,
            }),
        }
    }
}
```

Construction does not touch `&mut App`. It uses only `ctx.allocate_dialog_id`, `ctx.create_signal`, and the already-Rc'd `root_context` — all reachable through `ConstructCtx`.

`DialogPrivateEngine` is **built at install time**, not at construction. The 3.5.A field `PendingTopLevel::pending_private_engine: Option<Box<dyn emEngine>>` is replaced by `private_engine_root_panel_id: PanelId` (plus the already-present `close_signal`). Inside `install_pending_top_level`, once the winit surface is created and `materialized_wid` is known, the install path constructs `Box::new(DialogPrivateEngine { root_panel_id, window_id: materialized_wid, close_signal })` and calls `scheduler.register_engine(..., PanelScope::Toplevel(materialized_wid))`. `DialogPrivateEngine::window_id` narrows from `Option<WindowId>` to `WindowId` — no `Option`, no downcast, no builder struct. See Risks table for fallback options.

### 3. Mutators operate on pre-show builder state

```rust
impl emDialog {
    pub fn AddCustomButton<C: ConstructCtx>(&mut self, ctx: &mut C, label: &str, result: DialogResult) {
        // Emulates C++ emDialog::AddCustomButton (emDialog.cpp:86-98): allocate a
        // DlgButton with name=ButtonNum, append to ButtonsPanel children, push the
        // (click_signal, result) pair onto DlgPanel.button_signals so
        // DialogPrivateEngine::Cycle observes clicks.
        let look = self.look.clone();
        let btn = DlgButton::new(ctx, label, look, result.clone(), self.root_panel_id);
        let click_signal = btn.button.click_signal;
        let pending = self.pending.as_mut().expect("AddCustomButton after show");
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(self.root_panel_id).expect("DlgPanel root");
        let button_num = {
            let dlg = behavior.as_dlg_panel_mut().expect("root is DlgPanel");
            let n = dlg.button_signals.len();
            dlg.button_signals.push((click_signal, result));
            n
        };
        tree.put_behavior(self.root_panel_id, behavior);
        let btn_id = tree.create_child(self.root_panel_id, &button_num.to_string(), None);
        tree.set_behavior(btn_id, Box::new(btn));
    }

    pub fn SetRootTitle(&mut self, title: &str) {
        self.with_dlg_panel_mut(|p| p.SetTitle(title));
    }

    pub fn set_button_label_for_result(&mut self, result: &DialogResult, label: &str) {
        // Walk child DlgButtons, update the first match's caption.
        …
    }

    pub fn EnableAutoDeletion(&mut self, enabled: bool) {
        self.with_dlg_panel_mut(|p| p.auto_delete = enabled);
    }

    pub fn set_on_finish(&mut self, cb: WidgetCallbackRef<DialogResult>) {
        self.with_dlg_panel_mut(|p| p.on_finish = Some(cb));
    }

    pub fn set_on_check_finish(&mut self, cb: Box<dyn FnMut(&DialogResult) -> bool>) {
        self.with_dlg_panel_mut(|p| p.on_check_finish = Some(cb));
    }

    fn with_dlg_panel_mut<R>(&mut self, f: impl FnOnce(&mut DlgPanel) -> R) -> R {
        let pending = self.pending.as_mut().expect("dialog mutator after show");
        let tree = pending.window.tree_mut();
        let mut behavior = tree.take_behavior(self.root_panel_id).expect("DlgPanel root");
        let r = f(behavior.as_dlg_panel_mut().expect("root is DlgPanel"));
        tree.put_behavior(self.root_panel_id, behavior);
        r
    }

    /// `AddOKButton`, `AddCancelButton`, `AddOKCancelButtons`, `AddPositiveButton`,
    /// `AddNegativeButton` port as thin wrappers over `AddCustomButton`. Result
    /// numbering: `POSITIVE=Ok`, `NEGATIVE=Cancel`, `CUSTOM{1,2,…}=Custom(1..)`.
}
```

**Mutator contract:** pre-show only. Post-show mutation panics via `expect("mutator after show")`. Zero runtime cost on the correct path. Consumers only mutate between `new()` and `show()` (verified by audit — see §Audit below).

### 4. `show()` enqueues via the closure rail

```rust
impl emDialog {
    pub fn show<C: ConstructCtx>(&mut self, ctx: &mut C) {
        let pending = self.pending.take().expect("show called twice");
        let queue: &Rc<RefCell<Vec<FrameworkDeferredAction>>> = ctx.pending_actions();
        queue.borrow_mut().push(Box::new(move |app, el| {
            app.pending_top_level.push(pending);
            app.install_pending_top_level(el);
        }));
    }
}
```

After `show()`, `self.pending` is `None`. The returned `emDialog` is the stable handle: its `dialog_id`, `finish_signal`, `close_signal`, `root_panel_id` remain valid — consumers keep holding the struct.

Precedent: `emView::RawVisitAbs` pushes an identically-shaped closure onto `pending_framework_actions` to schedule popup materialization (`emView.rs:~1936-1975`); `App::materialize_pending_popup` is that closure's body. `App::install_pending_top_level` is the top-level-dialog analog and already exists.

### 5. Closure-rail deferral — no new `DeferredAction` enum variants

The enum rail (`emEngineCtx::DeferredAction::{CloseWindow, MaterializePopup}`) currently has **no drain site** in the framework loop — the only real emitter is `DialogPrivateEngine::Cycle`'s auto-delete path (`emDialog.rs:474`), and that push goes nowhere. Rather than introduce a first drain site with this phase, Task 5 uses the established closure rail for all deferred work. Three call sites route through it:

- **`emDialog::show`** (§4) — closure pushes the `PendingTopLevel` onto `app.pending_top_level` and calls `app.install_pending_top_level(el)` immediately.
- **`silent_cancel` replacement** in emStocksListBox (§10.4) — closure calls `app.close_dialog_by_id(did)`.
- **`DialogPrivateEngine` auto-delete** (rewrite of `emDialog.rs:474`) — closure calls `app.close_dialog_by_id(did)`.

`App::close_dialog_by_id` is defined in the `## App::close_dialog_by_id` section below.

**Latent bug fixed in passing:** The current `emDialog.rs:474` push of `DeferredAction::CloseWindow(wid)` lands on the undrained enum rail — meaning **auto-delete after `finish_state == 3` currently has no runtime effect**. Task 5.1's rewrite to the closure rail resolves this incidentally. The existing auto-delete test at `emDialog.rs:1298-1336` asserts internal state (FinishState counter) but not actual window removal; Task 5.1 extends the test to assert `app.windows.remove(&wid).is_none()` after the countdown closure drains.

### 6. `EngineScheduler::engines_for_scope` helper

`silent_cancel`'s drain closure needs to enumerate all engines registered at `PanelScope::Toplevel(wid)` for cleanup. The scheduler already tracks `PanelScope` per engine (Phase 3.5.A Task 6.2); Task 5.1 adds the read-only helper:
```rust
impl EngineScheduler {
    pub fn engines_for_scope(&self, scope: PanelScope) -> Vec<EngineId>;
}
```
No existing precedent uses a multi-engine scope query — this is new — but it is a thin `iter().filter().collect()` over existing state. Why the explicit helper rather than `Drop for emWindow`: emWindow holds no reference to `EngineScheduler`, and giving it one would require either an `Rc<RefCell<EngineScheduler>>` (violates single-threaded ownership) or a back-pointer through App (cyclic). The helper-plus-explicit-call pattern is forced by ownership, not chosen for taste.

### 7. `ConstructCtx` trait extensions

```rust
pub trait ConstructCtx {
    fn create_signal(&mut self) -> SignalId;
    fn register_engine(&mut self, b: Box<dyn emEngine>, p: Priority, s: PanelScope) -> EngineId;
    fn wake_up(&mut self, e: EngineId);
    // New in Phase 3.5 Task 5:
    fn pending_actions(&self) -> &Rc<RefCell<Vec<FrameworkDeferredAction>>>;
    fn allocate_dialog_id(&mut self) -> DialogId;
    fn root_context(&self) -> &Rc<emContext>;
}
```

Backing fields needed on each ctx:

- `InitCtx` — add `pending_actions: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>`. Plumbed from `App::pending_actions` at setup.
- `EngineCtx` — add the same field. Already has `scheduler` and `root_context`; `DoTimeSlice` supplies the handle from `&self.pending_actions`.
- `PanelCtx` / `SchedCtx` — same addition; constructed from `EngineCtx` or directly at the framework entry points.

Precedent: `emView::pending_framework_actions: Option<Rc<RefCell<Vec<DeferredAction>>>>` (emView field) already follows the same shape and is already populated from `App::pending_actions.clone()` (`emGUIFramework.rs:893`). The ctx fields are a thin generalization.

### 8. `next_dialog_id` counter relocation

Phase 3.5.A placed `next_dialog_id: u64` on `App` because the consumer had not yet landed. Under Phase 3.5, construction needs the counter reachable through ctx, and ctx does not carry `&mut App`. The counter moves to `EngineScheduler`:

```rust
impl EngineScheduler {
    pub fn allocate_dialog_id(&mut self) -> DialogId {
        let id = DialogId(self.next_dialog_id);
        self.next_dialog_id = self.next_dialog_id.checked_add(1)
            .expect("DialogId overflow");
        id
    }
}
```

`App::next_dialog_id` field deleted; `App::allocate_dialog_id()` becomes a one-line delegate to `self.scheduler.allocate_dialog_id()` (kept for the headless-install test path). Precedent: the scheduler already owns monotonic allocators for `SignalId` / `EngineId` / `TimerId`.

### 9. Consumer observation — `on_finish` + `Rc<Cell>`

Consumers observe dialog completion via the existing `on_finish` callback, set pre-show on the dialog. The closure captures an `Rc<Cell<Option<DialogResult>>>` owned by the consumer. `DialogPrivateEngine::Cycle` already fires `finish_signal` and invokes `on_finish` with a `SchedCtx` (`emDialog.rs:454-463`) — no change to that code path.

The consumer's own `Cycle` method reads the `Cell` on each invocation. Phase 3.5 does not change *when* the consumer's Cycle runs — the current invocation cadence (caller-driven from the owning panel's cycle path) is preserved. The Cell pattern is correct whether Cycle polls every slice or only on signal-driven wake-ups; the consumer sees the result whenever it next reads, and a latent wake-up optimization (connect the consumer's engine to `finish_signal`) is a future improvement that does not affect correctness.

**No cross-window tree read needed.** The `Cell` pattern confines all post-finish state access to the consumer's own memory — the consumer never dereferences `app.windows[dialog_wid]` to read state.

## Consumer migration — emStocksListBox (Task 5.2)

### 10.1 Field shape

```rust
pub struct emStocksListBox {
    …
    // Four dialog handles (same names as today):
    pub(crate) cut_stocks_dialog: Option<emDialog>,
    pub(crate) paste_stocks_dialog: Option<emDialog>,
    pub(crate) delete_stocks_dialog: Option<emDialog>,
    pub(crate) interest_dialog: Option<emDialog>,
    // Four result slots, written by on_finish closures:
    pub(crate) cut_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) paste_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) delete_stocks_result: Rc<Cell<Option<DialogResult>>>,
    pub(crate) interest_result: Rc<Cell<Option<DialogResult>>>,
    …
}
```

### 10.2 Construct-call-site shape (4 sites)

```rust
// Was (example: DeleteStocks):
// let mut dialog = emDialog::new(cc, &format!("Really delete {} stock(s)?", count), look.clone());
// dialog.AddCustomButton("Delete", DialogResult::Ok);
// dialog.AddCustomButton("Cancel", DialogResult::Cancel);
// self.delete_stocks_dialog = Some(dialog);

// Now:
let mut dialog = emDialog::new(cc, &format!("Really delete {} stock(s)?", count), look.clone());
dialog.AddCustomButton(cc, "Delete", DialogResult::Ok);
dialog.AddCustomButton(cc, "Cancel", DialogResult::Cancel);
let cell = Rc::clone(&self.delete_stocks_result);
dialog.set_on_finish(Box::new(move |r, _sched| cell.set(Some(r.clone()))));
dialog.show(cc);
self.delete_stocks_dialog = Some(dialog);
```

### 10.3 Polling-site rewrite (4 sites in `Cycle`)

```rust
// Was:
// if let Some(result) = self.delete_stocks_dialog.as_ref().and_then(|d| d.GetResult()) { … }

// Now:
if let Some(result) = self.delete_stocks_result.take() {
    let confirmed = result == DialogResult::Ok;
    self.delete_stocks_dialog = None;  // handle dropped
    if confirmed {
        self.DeleteStocks(cc, rec, false);
    }
} else if self.delete_stocks_dialog.is_some() {
    busy = true;
}
```

The `Cell::take` consumes the result atomically. The handle is dropped from its Option slot; on Drop we do **not** close the window (the auto-delete path inside `DialogPrivateEngine` handles window teardown 3 slices after the fire — unchanged 3.5.A semantics).

### 10.4 `silent_cancel` rewrite (4 sites)

```rust
// Was:
// if let Some(ref mut d) = self.delete_stocks_dialog { d.silent_cancel(); }

// Now:
if let Some(d) = self.delete_stocks_dialog.take() {
    let did = d.dialog_id;
    cc.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
        app.close_dialog_by_id(did);
    }));
}
```

`App::close_dialog_by_id(did)` is a new method added in Task 5.1 that encapsulates the pre-vs-post-materialize branch (full body in the `## App::close_dialog_by_id` section). The consumer-side closure is one-liner; the branching logic lives on `App` for reuse (auto-delete path + `silent_cancel` path + future phases).

Dropping the `emDialog` handle before the engine's Drop is harmless — `DialogPrivateEngine` lives in the scheduler by `EngineId` and is unregistered by `close_dialog_by_id`'s cleanup loop. The replacement-dialog construction happens immediately afterward with a fresh `DialogId` and fresh `Cell`; any latent signal fires from the old dialog are routed to the old engine (already removed) or fall on removed signals.

`emDialog::silent_cancel` is deleted.

## Consumer migration — emFileDialog (Task 5.3)

### 11.1 Deletions

- `emFileDialog::set_mode` — deleted (zero live callers; not an in-scope C++ parity concern).
- `emFileDialog::dialog_mut` — deleted (zero live callers; referenced only in a stale comment).

### 11.2 Field shape

```rust
pub struct emFileDialog {
    dialog: emDialog,                           // handle; pre- or post-show
    look: Rc<emLook>,                           // held directly so overwrite_dialog
                                                // construction doesn't rely on a
                                                // deleted emDialog::look() accessor
    fsb: …,
    mode: FileDialogMode,
    overwrite_dialog: Option<emDialog>,         // same shape
    overwrite_result: Rc<Cell<Option<DialogResult>>>,  // new
    …
}
```

The `self.dialog.look().clone()` call at `emFileDialog.rs:273` becomes `self.look.clone()`. The field is initialized in `emFileDialog::new` from the `look` parameter already passed in.

### 11.3 Construction

Same pattern as emStocksListBox: call `emDialog::new`, add buttons, set `on_finish` if needed, `show(ctx)`. `emFileDialog::CheckFinish` continues to build the `overwrite_dialog` inline when `FileDialogMode::Save` detects an existing target — it already has a `&mut C: ConstructCtx`, so the build + show path works.

### 11.4 Cycle migration — Cell shim now, full engine wiring Phase 3.6

Current `emFileDialog::Cycle` (emFileDialog.rs:329-370) polls `fsb.GetFileTriggerSignal` + `overwrite_dialog.GetResult`. Since `GetResult` is deleted in Task 5.1, Phase 3.5 Task 5.3 rewrites the overwrite-read site to `self.overwrite_result.take()` (same Cell pattern as emStocksListBox). Signature and caller contract of `emFileDialog::Cycle` are preserved. Full migration to signal-driven observation (registering `emFileDialog` as an `emEngine` with `AddWakeUpSignal`-equivalent subscription to `fsb.file_trigger_signal` and `overwrite_dialog.finish_signal`) is Phase 3.6's territory; the `Cell`-based shim is the minimum step needed to land Task 5.1's API deletions.

## Audit — "mutator after show" is a real contract

All in-tree live mutator call sites for `emDialog` ({`AddCustomButton`, `AddOKButton`, `AddCancelButton`, `AddOKCancelButtons`, `AddPositiveButton`, `AddNegativeButton`, `SetRootTitle`, `set_button_label_for_result`, `EnableAutoDeletion`, `set_on_finish`, `set_on_check_finish`}) occur between `let mut d = emDialog::new(…);` and storing the handle. Grep results:

- `emstocks/src/emStocksListBox.rs:488-494, 536-540, 573-576, 663-666` — all pre-show (followed immediately by `self.x_stocks_dialog = Some(dialog);` with no further mutation).
- `emcore/src/emFileDialog.rs:67-69, 273-276` — all pre-show (call chain is `emFileDialog::new` or inside `CheckFinish` immediately before `overwrite_dialog = Some(dlg);`).
- `emcore/src/emFileDialog.rs:92-98` — `set_mode` (deleted; not a live caller).

No post-show live callers. The `expect("… after show")` panic is a latent-bug trip wire, not a runtime cost on the correct path.

## `App::close_dialog_by_id` — the one new App-side method

All closure-rail close paths (`silent_cancel`, `DialogPrivateEngine` auto-delete, future Phase-3.6 consumers) share one branching body, factored onto App:

```rust
impl App {
    pub fn close_dialog_by_id(&mut self, did: DialogId) {
        if let Some(wid) = self.dialog_windows.remove(&did) {
            // Post-materialize.
            let engines: Vec<EngineId> = self.scheduler
                .engines_for_scope(PanelScope::Toplevel(wid));
            for eid in engines {
                self.scheduler.remove_engine(eid);
            }
            // Signals on DlgPanel (finish_signal, button click_signals) are
            // owned by the scheduler; SlotMap keys survive engine removal.
            // Explicit cleanup happens on window drop.
            self.windows.remove(&wid);
        } else if let Some(idx) = self.pending_top_level
            .iter()
            .position(|p| p.dialog_id == did)
        {
            // Pre-materialize.
            let pending = self.pending_top_level.swap_remove(idx);
            // The pending_private_engine was never registered; drop it.
            drop(pending);
            // Signals on the pending entry become orphaned; slotmap makes
            // fire-to-dead-signal a no-op. Explicit remove is optional;
            // Task 5.1 chooses explicit remove for testability.
            // (finish_signal, close_signal, flags/focus/geom_signals)
        }
        // Otherwise: unknown DialogId — no-op (idempotent).
    }
}
```

Consumers of `close_dialog_by_id`:
- `silent_cancel` replacement in emStocksListBox (§10.4).
- `DialogPrivateEngine::Cycle` auto-delete path — currently emits `DeferredAction::CloseWindow(wid)` on the undrained enum rail (`emDialog.rs:474`). Task 5.1 rewrites this to push a closure-rail action:
  ```rust
  ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
      app.close_dialog_by_id(did);
  }));
  ```
  Fixes the latent auto-delete bug simultaneously.

The Phase-3.5.A `install_pending_top_level` entry path is unchanged — `emDialog::show`'s closure body calls it directly (§4). No new drain site, no new enum variants.

## Deferred to Phase 3.6

### Post-show dialog mutation

If Phase 3.6 finds a live need to mutate a *shown* dialog (e.g. `emFileDialog` mode change while visible, or a new E024-driven title update), add an `App::mutate_dialog_by_id` method that walks `dialog_window_mut → tree_mut → take_behavior(root_panel_id) → apply → put_behavior`, and route through the closure rail:

```rust
impl App {
    pub fn mutate_dialog_by_id(
        &mut self,
        did: DialogId,
        f: impl FnOnce(&mut DlgPanel),
    ) {
        if let Some(DialogWindow::Materialized { window, .. }) = self.dialog_window_mut(did) {
            let mut tree = window.take_tree();
            // Root panel id is recoverable from DlgPanel via a convention check
            // or tracked alongside `dialog_windows`; the exact storage is a
            // Phase-3.6 design detail.
            if let Some(mut b) = tree.take_behavior(root_panel_id_for(did)) {
                if let Some(dlg) = b.as_dlg_panel_mut() {
                    f(dlg);
                }
                tree.put_behavior(root_panel_id_for(did), b);
            }
            window.put_tree(tree);
        }
    }
}
```

The `emDialog::{SetRootTitle, EnableAutoDeletion, …}` mutators route pre-show direct / post-show via-closure based on `self.pending.is_some()`:

```rust
pub fn SetRootTitle<C: ConstructCtx>(&mut self, ctx: &mut C, title: &str) {
    if self.pending.is_some() {
        self.with_dlg_panel_mut(|p| p.SetTitle(title));
    } else {
        let did = self.dialog_id;
        let title = title.to_string();
        ctx.pending_actions().borrow_mut().push(Box::new(move |app, _el| {
            app.mutate_dialog_by_id(did, |p| p.SetTitle(&title));
        }));
    }
}
```

**Not done in 3.5 because no live caller needs it.** Adding the post-show routing pre-emptively would violate "don't add features for hypothetical future requirements" (CLAUDE.md). Phase-3.5 mutators panic on post-show misuse (§3 `expect("mutator after show")`).

### emFileDialog `Cycle` / `CheckFinish` full observer migration

The `Cell` shim in §11.4 covers Phase 3.5's "no more `GetResult`" contract. The Phase 3.6 plan (`docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-emfiledialog-e024.md`) ports `emFileDialog` to a proper engine with `AddWakeUpSignal(fsb.file_trigger_signal)` + `AddWakeUpSignal(overwrite_dialog.finish_signal)` subscription.

### `emDialog::ShowMessage`

Static convenience method that builds an OK-only message dialog with auto-delete. Currently in the legacy `impl`; not deleted in Task 5.1 because the signature is part of the C++ API surface. Kept as a deprecated shim that `unimplemented!()`s until Phase 3.6 wires a real path (at which point it becomes `emDialog::new + AddOKButton + EnableAutoDeletion(true) + show`).

## Invariants carried from the original Phase 3.5 plan

| Invariant | Status |
|---|---|
| I5a — one dialog ⇔ one `emWindow` ⇔ one `PanelTree` ⇔ one `DlgPanel` root | Enforced by `emDialog::new` building a fresh tree + root |
| I5b — `close_signal` drives `DialogPrivateEngine` cancel path | Unchanged from 3.5.A Task 10 |
| I5c — `DialogPrivateEngine` scoped `Toplevel(wid)` post-materialize | Unchanged from 3.5.A Task 10 |
| I5d — `finish_signal` fires exactly once per dialog | Enforced by `FinishState==1 → 2` transition in `DialogPrivateEngine::Cycle` (emDialog.rs:441-464) |
| I5e — auto-delete drives window teardown after 3-slice countdown | Rewritten in Task 5.1: the countdown now pushes a closure-rail action that calls `App::close_dialog_by_id(did)` (fixes latent bug where the previous `DeferredAction::CloseWindow` push was undrained) |
| I5f — `emDialog` identity survives the show transition | NEW — `dialog_id` / `finish_signal` / `close_signal` / `root_panel_id` are stable |
| I5g — post-show mutator panics | NEW — `expect("mutator after show")` contract |
| I5h — construction does not require `&mut App` | NEW — `ConstructCtx` interface only |
| I5i — consumer observation via `on_finish` + `Rc<Cell>`; no `GetResult` polling | NEW — replaces 4 polling sites in emStocksListBox + the overwrite read in emFileDialog |
| I5j — cancel-in-flight resolves pre- and post-materialize uniformly | NEW — `App::close_dialog_by_id(DialogId)` supersedes `emDialog::silent_cancel`; reached via the closure rail |

## Phased execution — tasks

### Task 5.1 — `emDialog` reshape (foundational, high-risk)

**Changes:**
- `emEngineCtx.rs`: extend `ConstructCtx` trait with `pending_actions` / `allocate_dialog_id` / `root_context`; add the `pending_actions: &'a Rc<RefCell<Vec<FrameworkDeferredAction>>>` field to `InitCtx` / `EngineCtx` / `SchedCtx` (and `PanelCtx` via its inner ctx). No new `DeferredAction` enum variants.
- `emScheduler.rs`: add `next_dialog_id: u64` field, `allocate_dialog_id()` method, `engines_for_scope(scope: PanelScope) -> Vec<EngineId>` helper.
- `emGUIFramework.rs`: delete `App::next_dialog_id` field; `App::allocate_dialog_id` delegates to scheduler. Thread `self.pending_actions.clone()` into every ctx construction site. Add `App::close_dialog_by_id(did)` method (body in `## App::close_dialog_by_id` section). Reshape `PendingTopLevel`: drop `pending_private_engine: Option<Box<dyn emEngine>>`, add `private_engine_root_panel_id: PanelId` — `install_pending_top_level` constructs `DialogPrivateEngine` on the spot with `window_id = materialized_wid` and registers it at `PanelScope::Toplevel(wid)`.
- `emDialog.rs`: delete legacy `emDialog::{Finish, Input, Paint, LayoutChildren, CheckFinish, silent_cancel, result, on_finish, on_check_finish, auto_delete, buttons, border, preferred_size, GetButton, GetButtonForResult, GetOKButton, GetCancelButton, IsAutoDeletionEnabled}` and the legacy private fields (`look` kept — §3.1). Un-gate `DlgPanel`, `DlgButton`, `DialogPrivateEngine`. Narrow `DialogPrivateEngine::window_id` from `Option<WindowId>` to `WindowId` (now that construction happens at install time). Reshape `emDialog` struct + constructor. Implement all mutators on the new shape. Implement `show()`. Rewrite `DialogPrivateEngine::Cycle`'s auto-delete emission from `ctx.framework_action(DeferredAction::CloseWindow(wid))` to `ctx.pending_actions().borrow_mut().push(Box::new(move |app, _| app.close_dialog_by_id(did)))` — fixes latent undrained-push bug. Port `ShowMessage` as an `unimplemented!()` shim until Phase 3.6. Update the existing 15+ tests to the new API.
- `emWindow.rs`: narrow `tree`/`take_tree`/`put_tree` visibility per `project_phase35a_pub_narrow.md` (if no new cross-crate callers emerge).

**Gate:** `cargo check`, `cargo clippy -D warnings`, `cargo-nextest ntr` green. The existing `private_engine_observes_close_signal_sets_pending_cancel` test passes with the new `emDialog::new` → `show` → install path (replaces the manual `PendingTopLevel::push` at emDialog.rs:1235).

### Task 5.2 — emStocksListBox migration

**Changes:**
- `emStocksListBox.rs`: add four `Rc<Cell<Option<DialogResult>>>` fields alongside the existing `Option<emDialog>` fields. Rewrite the four construct sites (`DeleteStocks`, `CutStocks`, `PasteStocks`, `SetInterest`) to set `on_finish` + call `show(cc)`. Rewrite the four `Cycle` polling sites to `.take()` from the `Cell`. Replace four `d.silent_cancel()` calls with `cc.pending_actions().borrow_mut().push(Box::new(move |app, _el| app.close_dialog_by_id(did)))`.
- `emDialog.rs`: delete `emDialog::silent_cancel` method.

**Gate:** nextest green (including the emstocks integration tests), clippy green, emstocks goldens preserved (238/243).

### Task 5.3 — emFileDialog construction migration

**Changes:**
- `emFileDialog.rs`: delete `set_mode`, `dialog_mut`. Add `overwrite_result: Rc<Cell<Option<DialogResult>>>`. Rewrite `emFileDialog::new` to use new API + `show`. Rewrite `CheckFinish`'s overwrite-dialog creation to set `on_finish` + `show`. Rewrite the `Cycle` polling of `overwrite_dialog.GetResult` to `.take()` from `overwrite_result`. Preserve `emFileDialog::Cycle` signature (full engine migration is Phase 3.6).

**Gate:** nextest green, clippy green, existing emFileDialog tests pass.

### Task 5.4 — Closeout

**Changes:**
- `docs/superpowers/notes/<today>-phase-3-5-ledger.md`: ledger with per-task `COMPLETE.` entries (no self-SHA per `feedback_ledger_no_self_sha`).
- `MEMORY.md` updates: close out `project_phase35a_pub_narrow.md` or supersede with Phase 3.5 snapshot.
- Tag: `port-rewrite-phase-3-5-complete`.

**Gate:** full nextest + clippy + goldens green; tag applied on HEAD of `port-rewrite/phase-3-5-deferred-dialog-construction`.

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| `DialogPrivateEngine::window_id = None` at build time → install must know it | **Default: construct at install time.** Drop the pre-built `pending_private_engine: Option<Box<dyn emEngine>>` on `PendingTopLevel`; replace it with the raw construction inputs (`close_signal: SignalId`, `root_panel_id: PanelId` — already on `PendingTopLevel` in other forms). `install_pending_top_level` builds `DialogPrivateEngine { root_panel_id, window_id: Some(materialized_wid), close_signal }` at the point it knows `wid`, then calls `scheduler.register_engine(...)` at `PanelScope::Toplevel(wid)`. No `Option<WindowId>`, no downcast, no builder struct. The 3.5.A code that pre-boxes the engine is replaced, not extended. Fallbacks if construction-at-install proves awkward: (a) typed `set_window_id` method on `DialogPrivateEngine` + downcast through a narrow trait; (b) `PendingDialogPrivateEngine → Box<dyn emEngine>` builder finalizer. |
| Mutators requiring `&mut C` (e.g. `AddCustomButton` creating signals for `DlgButton::click_signal`) need ctx threaded through | API-level: `AddCustomButton(&mut self, ctx: &mut C, label, result)`. Consumer sites already have `cc` in scope. Matches C++ where the button's parent-context reach is implicit. |
| Consumer's `Rc<Cell>` may be fired-to after handle drop | Drop order is: consumer drops `emDialog` → `DialogPrivateEngine` still running → `on_finish` closure still holds `Rc<Cell>` (not `Weak`) → fires into a Cell nobody polls. Harmless. Cell is later dropped when consumer drops. |
| Closure-rail ordering: `close_dialog_by_id(did)` for an old dialog races with `install_pending_top_level` for a new dialog | `DialogId` is monotonic and freshly allocated at each `emDialog::new`. Consumer-level replacement uses `close_dialog_by_id(old_did)` (closure at time T) followed by `emDialog::new(→ new_did).show(ctx)` (closure at time T+ε) — different IDs; the two closures drain in FIFO order within one `about_to_wait` tick, so the old dialog's teardown runs before the new dialog's install. |
| Orphaned signals after pre-materialize close | `SlotMap::remove` on signals is cheap; `close_dialog_by_id` calls `scheduler.remove_signal` for finish / close / flags / focus / geom. Alternative: leak-tolerant (signals are small). Default: explicit remove. |
| `emStocksListBox::Cycle` takes `&mut C: ConstructCtx` but `pending_actions` was not on the trait | Task 5.1 adds `pending_actions` to `ConstructCtx`. Four implementors gain the `Rc<RefCell<...>>` handle at construction (threaded from `App::pending_actions.clone()`). |
| Phase 3.5 correctness of `emFileDialog` is contingent on Phase 3.6 landing | The `Cell` shim at §11.4 preserves `emFileDialog::Cycle`'s caller-driven polling cadence while deleting `GetResult`. If Phase 3.6 slips, `emFileDialog` stays in a "poll-where-C++-subscribed" state: correct but suboptimal — the Cycle runs whenever the enclosing panel cycles, not precisely when `fsb.file_trigger_signal` or `overwrite_dialog.finish_signal` fires. Acceptable for Phase 3.5 because `emFileDialog` has no live user-path consumers (the file-open flow isn't wired up yet); the correctness window is open until a real consumer lands. If 3.6 slips past that consumer, revisit. |

## Test strategy

- **Task 5.1 unit tests:** port the 15 existing `emDialog` tests to the new API. Add: `show() transitions pending.is_some() → false`; `show() enqueues a closure onto pending_actions`; `mutator after show panics`; `close_dialog_by_id(did) pre-materialize drops the pending entry`; `close_dialog_by_id(did) post-materialize removes from windows + unregisters Toplevel(wid) engines`; `auto-delete countdown enqueues close_dialog_by_id and fires window removal`.
- **Task 5.2 integration:** emstocks end-to-end: `DeleteStocks(ask=true)` → show dialog → simulate button click via `scheduler.fire(click_signal)` → one `DoTimeSlice` → verify result Cell set, dialog handle cleared, `DeleteStocks(ask=false)` subsequently invoked.
- **Task 5.3 integration:** emFileDialog Save-mode + existing target → `CheckFinish` produces overwrite dialog + `ConfirmOverwrite` result → fire overwrite `Ok` → next `Cycle` reads `overwrite_result`, calls `Finish(Ok)`.
- **Goldens:** 238/243 preserved at every task commit (gate).
- **Headless install path:** `App::install_pending_top_level_headless` continues to work for test fixtures.

## Open questions for implementation (non-blocking for this spec)

1. `DialogPrivateEngine` install-time construction: default is to store `(close_signal, root_panel_id)` on `PendingTopLevel` and build the engine inside `install_pending_top_level` once `window_id` is known (no `Option<WindowId>`, no downcast). Fallbacks documented in Risks. Task 5.1 validates the default; switches to a fallback only if construction-at-install conflicts with an unforeseen constraint.
2. `AddCustomButton` child-panel naming (`0`, `1`, …) matches C++ `emString::Format("%d", ButtonNum)` — preserve.
