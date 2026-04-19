# SP7 — emContext threading through the view/window subsystem

**Date:** 2026-04-19
**Plan:** `docs/superpowers/plans/2026-04-19-emview-sp7-emcontext-threading.md` (to be written)
**Closes:** §8.1 item 15 of `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md`.

## 1. Motivation

`emContext` exists (`crates/emcore/src/emContext.rs`, ~393 lines) with root/child nesting, scheduler lookup, typed-singleton `acquire`, and clipboard hooks — but **no caller in the view/window/panel subsystem threads one through**:

- `emView::new(root, w, h, core_config)` does not take a context.
- `emWindow::create` and `emWindow::new_popup_pending` do not take a context.
- `emSubViewPanel::new()` builds a child `emView` with no context in scope.
- SP3 landed a direct-injection `CoreConfig: Rc<RefCell<emCoreConfig>>` field as a bridge — acquisition through `emCoreConfig::Acquire(GetRootContext())` is still absent.

C++ `class emView : public emContext` makes every view a context node. Every panel reaches its config, clipboard, and model registry through its view's `GetRootContext()`. Rust has no inheritance — the port is **forced-diverged to composition** (emView owns `Rc<emContext>`), but the observable effect must match C++: panels, sub-views, and popup windows find the same root context, the same `emCoreConfig` singleton, and the same scheduler.

Per CLAUDE.md, `emCoreConfig::Acquire(root_ctx)` is the correct acquisition shape — SP3's direct-injection is the stand-in. SP7 replaces that with real context-rooted acquisition.

## 2. Scope (full, per user direction)

**In scope:**

1. Add `emContext::GetRootContext(&self) -> Rc<emContext>` walking up the parent chain.
2. Port `emCoreConfig::Acquire(&Rc<emContext>) -> Rc<RefCell<emCoreConfig>>` — registers into the root context's model registry under `(TypeId::<emCoreConfig>, "")` and returns the shared singleton.
3. `emView::new(parent_context: Rc<emContext>, root, w, h)` — drops the `core_config` parameter. Body creates a child context of `parent_context`, calls `emCoreConfig::Acquire(ctx.GetRootContext())`, stores both `Context` and `CoreConfig` on the view.
4. Expose `emView::GetContext(&self) -> &Rc<emContext>` and `emView::GetRootContext(&self) -> Rc<emContext>`.
5. `emWindow::create` and `emWindow::new_popup_pending` take `parent_context: Rc<emContext>` and thread it to `emView::new`.
6. `emSubViewPanel` threads the outer view's context as the inner view's parent. Forced pattern: read via `Panel.View.upgrade().borrow().GetContext().clone()` at init time.
7. Migrate all `emView::new` / `emWindow::create` / `emWindow::new_popup_pending` call sites. Production uses `app.context`; tests construct `emContext::NewRoot()` inline.
8. `emMainWindow::new` and its downstream wiring already pass `app.context` — threading continues through from there into `emWindow::create`.
9. Full test suite + golden baseline parity (237/6 pre-existing failures).

**Explicitly out of scope:**

- Real `emClipboard` implementation. The wiring point (`emContext::set_clipboard`) already exists; no clipboard backend is installed today. SP7 leaves that state unchanged — installing a real clipboard is a separate sub-project (needs winit/arboard decisions, platform matrix, etc.). Recorded in §6 for closeout-doc carry-forward.
- Per-window sub-context nesting for `emMainWindow` / control-window pairings. C++ nests each window under the root; the Rust port currently threads root directly. This is sufficient for acquisition parity. (If future multi-window work needs per-window scoping for model lifetime, a follow-up sub-project can layer child contexts in.)
- Migrating the ~38 ad-hoc `NewRoot()` calls in `emmain`/`emfileman` production code: audit confirmed all are inside `#[cfg(test)]` modules. Test-local roots stay test-local — the closeout-doc "production NewRoot" claim was stale.

## 3. Architecture

### 3.1 Ownership shape

```
App { context: Rc<emContext>      // root, holds scheduler
      scheduler: Rc<RefCell<EngineScheduler>>
      ...
    }

emWindow::create(parent_context: Rc<emContext>, ...) {
    let view = emView::new(parent_context, ...);  // view's ctx = child of parent
    ...
}

emView::new(parent_context: Rc<emContext>, ...) -> emView {
    let ctx = emContext::NewChild(&parent_context);
    let core_config = emCoreConfig::Acquire(&ctx.GetRootContext());
    emView { Context: ctx, CoreConfig: core_config, ... }
}

emSubViewPanel::Paint/init time:
    let outer_ctx = self.View.upgrade().borrow().GetContext().clone();
    let inner_view = emView::new(outer_ctx, ...);
```

### 3.2 DIVERGED points

One new `DIVERGED:` block:

- `emView::Context: Rc<emContext>` — composition instead of `class emView : public emContext`. Cause: Rust has no inheritance; `emContext` is a concrete struct with `RefCell` state; virtual `emEngine` + context behaviour cannot be expressed through a single vtable. Mitigation: delegating accessors (`GetContext`, `GetRootContext`, `LookupInherited<T>` passthrough if any caller needs it).

One existing SP3 `DIVERGED:` note at `emSubViewPanel::new` (default-constructing a local `emCoreConfig`) is removed — acquisition now goes through the threaded context.

### 3.3 emCoreConfig::Acquire port shape

C++ pattern (`EM_IMPL_ACQUIRE_COMMON`): `emRef<emCoreConfig> emCoreConfig::Acquire(emRootContext & rootContext)` — named singleton under `""`.

Rust:

```rust
impl emCoreConfig {
    pub fn Acquire(ctx: &Rc<emContext>) -> Rc<RefCell<Self>> {
        ctx.acquire::<emCoreConfig>("", emCoreConfig::default)
    }
}
```

`acquire` already walks the root in the sense that the caller is expected to pass a root; we add a one-line helper that internally does `let root = ctx.GetRootContext(); root.acquire::<emCoreConfig>("", emCoreConfig::default)` so the call shape `emCoreConfig::Acquire(&ctx)` works from any context.

### 3.4 Test-side shape

The 181 `emView::new` test call sites update mechanically:

```rust
// before:
let view = emView::new(root, w, h, core_config);

// after:
let ctx = emContext::NewRoot();
let view = emView::new(ctx, root, w, h);
```

No `new_for_tests` helper is needed — inline `emContext::NewRoot()` is one line and makes test intent explicit. If migration churn warrants a helper later, add it post-hoc.

## 4. Phasing

Each phase is atomic, gated by `cargo check + clippy -D warnings + nextest + golden` before merge. Details in the plan doc; summary here:

1. **emContext::GetRootContext** + **emCoreConfig::Acquire(&Rc<emContext>)** port (pure additions; no caller changes).
2. **emView::new signature change** — take parent context, drop `core_config` param. Store `Context` field. Update all production + test call sites in one pass (181 sites, mechanical).
3. **emWindow::create + new_popup_pending** — take parent context; thread through.
4. **emSubViewPanel** — outer view's context threads to inner view.
5. **SP3 `DIVERGED:` note removal** in `emSubViewPanel::new`; bridge-era comment cleanup in `emView` field docs; closeout item 15 closure; residual audit (ensure no `default_for_tests`-style holes).
6. **Smoke + golden + closeout update.**

## 5. Risk register

| Risk | Mitigation |
|---|---|
| R1 — `emCoreConfig::Acquire` called from inside `acquire` closure re-enters `RefCell` on registry | `acquire` inserts after creating the value; no re-entrance into the registry happens during `create()` because `emCoreConfig::default()` takes no context. |
| R2 — SubView context threading borrows outer view at construction time | Outer view's `Context` field is `Rc<emContext>`; cloning the Rc requires a single immutable borrow of the outer view. `emSubViewPanel::new` currently has no access to the outer view at all — we shift construction of the inner view to a lazy init (or pass the context explicitly at `Paint`-time setup). The cleanest fix: add a `&Rc<emContext>` parameter to `emSubViewPanel::new` (callers already hold one via `PanelCtx`). |
| R3 — Test call-site churn introduces typos/duplication | One mechanical sed-style pass, covered by a single `cargo check` gate; no manual per-site reasoning. |
| R4 — emCoreConfig singleton shared across multiple test roots | Tests that construct independent roots get independent singletons — matches C++ (each root has its own registry). Acceptable. |
| R5 — Popup window's parent context wrong (attaches to app root instead of owner view) | C++ popup parent context is the owning view (child chain: root → owner view → popup view). Rust `new_popup_pending` will take `parent_context: Rc<emContext>` and callers (`emView::RawVisitAbs`) pass `self.Context.clone()`. |

## 6. Closeout-doc carry-forward

After implementation, closeout doc updates:

- §1 status table: "Known Rust-port incompletenesses remaining" drops SP7; SP7 closed.
- §6 markers table: no new `DIVERGED:` markers expected beyond the one `emView::Context` composition note (may absorb SP3's `emSubViewPanel::new` note rather than adding a fresh one).
- §8.0 sub-project table: SP7 state flips to **Complete YYYY-MM-DD** with artifacts.
- §8.1 item 15: mark closed with commit SHAs and residuals-if-any.
- New residual item (if clipboard wiring remains deferred): **Real emClipboard implementation** — noted as a future sub-project (SP9?), wiring point in place (`emContext::set_clipboard`), no backend installed. Not blocking.

## 7. Tests

- 2443 → ~2445 (expect +2 for `emContext::GetRootContext` unit + `emCoreConfig::Acquire` inheritance-via-child-context unit).
- Golden 237/6 unchanged; no pixel changes expected.
- One new behavioural test: `emCoreConfig_is_singleton_across_sibling_views` — two views constructed with the same parent context see the same `CoreConfig` singleton (via `Rc::ptr_eq`).

## 8. Rollback

Single-branch development (`sp7-emcontext-threading`). Each phase is a separate commit; if Phase N fails golden, revert to Phase N-1 tip. No upstream merge until all phases + golden pass.
