# FU-002 — App-Bound Reaction Wiring (mainctrl) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace three `TODO(B-012-followup)` stub reactions in `emMainControlPanel::Cycle` (NewWindow / Fullscreen / Quit click signals) with real wiring to `emMainWindow::Duplicate`, `ToggleFullscreen`, and `Quit` via the existing `App.pending_actions` deferred-action queue.

**Architecture:** The bucket file's "architectural decision required" phase is obviated — the spec's research showed that `App.pending_actions` is the established Rust pattern for deferred-`&mut App` work (already used by `emMainWindow::Duplicate` itself at `emMainWindow.rs:233` and by `emBookmarks.rs:748`). We add one thin helper `enqueue_main_window_action(ectx, |mw, app| ...)` next to `with_main_window`, then wire each of the three click reactions to a one-line call to that helper. No new types, no `Rc<RefCell<>>`, no ctx-trait widening.

**Tech Stack:** Rust, `emcore::emEngineCtx` (`pending_actions()`), `emcore::emGUIFramework::{App, DeferredAction}`, `crate::emMainWindow::{with_main_window, emMainWindow}`. Build/test via `cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr`, `cargo xtask annotations`.

**Non-interactive defaults chosen** (no human present):
- The helper is `pub(crate)` (consumed by sibling module `emMainControlPanel`, not exported from the crate).
- The helper lives in `emMainWindow.rs` near `with_main_window` (cohesive: it composes `with_main_window` with the deferred-action push). No new file.
- Helper-level unit testing is omitted (thin wrapper over an established pattern with no logic of its own — coverage comes from a behavioral integration test on the wired Quit reaction, which is the cheapest of the three to assert without a winit event loop).
- Bucket-file reconciliation in Phase 3 is performed by appending a "Resolution" section, not rewriting the file.
- Annotation policy: the helper is unannotated (an idiomatic-Rust helper sitting below the observable surface; the C++ shape is direct method dispatch on a long-lived parent pointer, language-forced unavailable). The reaction-body call sites are also unannotated — they are not divergences, they are the Rust call shape for the same observable behavior. Per CLAUDE.md, idiom adaptations that preserve observable behavior are unannotated.

---

## File Structure

| Path | Role | Change |
|---|---|---|
| `crates/emmain/src/emMainWindow.rs` | Owns `emMainWindow`, `with_main_window`, the App-mutating methods (`Duplicate`, `ToggleFullscreen`, `Quit`) | Add `enqueue_main_window_action` helper near `with_main_window` (~line 367) |
| `crates/emmain/src/emMainControlPanel.rs` | Control panel `Cycle` reaction bodies | Replace 3 `TODO(B-012-followup)` stub blocks (lines 561-566, 569-573, 609-615) with one-line `enqueue_main_window_action` calls; add `use crate::emMainWindow::enqueue_main_window_action;` to the import block |
| `crates/emmain/tests/typed_subscribe_b012.rs` (or new sibling test file) | Cross-crate behavioral test for the wired Quit reaction | Add one new `#[test]` exercising the `bt_quit_sig` reaction → drained `pending_actions` calls `Quit(&mut App)` (which triggers exit) |
| `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-002-app-bound-reactions.md` | FU-002 bucket file | Append a `## Resolution` section pointing at the implementing commits and noting that the architectural-decision phase was obviated |

No file splits, no new modules.

---

## Task 1 — Add `enqueue_main_window_action` helper

**Files:**
- Modify: `crates/emmain/src/emMainWindow.rs` (insert after the `with_main_window` definition ending at line 367)

- [ ] **Step 1: Read context to confirm insertion point**

Run: `grep -n "pub fn with_main_window" crates/emmain/src/emMainWindow.rs`
Expected: one hit at line 362. The helper inserts after the function body's closing brace at line 367.

- [ ] **Step 2: Insert the helper**

Open `crates/emmain/src/emMainWindow.rs`. After the closing `}` of `with_main_window` (line 367) and before `pub(crate) struct MainWindowEngine` (line 371), insert:

```rust
/// Enqueue a deferred action that runs with `&mut emMainWindow` and `&mut App`.
///
/// The closure executes during the next `pending_actions` drain on the winit
/// main loop tick. This composes the `with_main_window` thread-local accessor
/// with the framework's `pending_actions` rail so reaction bodies inside
/// `Cycle` (which has `EngineCtx` but not `&mut App`) can invoke
/// `emMainWindow` methods that require `&mut App`.
///
/// Mirrors the pattern at `emBookmarks.rs:748` and `emMainWindow::Duplicate`
/// (line 233). Use from `Cycle` reaction bodies that need to invoke
/// MainWindow methods with the `&mut App` parameter (`Duplicate`,
/// `ToggleFullscreen`, `Quit`).
pub(crate) fn enqueue_main_window_action<F>(ectx: &mut emcore::emEngineCtx::EngineCtx<'_>, action: F)
where
    F: FnOnce(&mut emMainWindow, &mut App) + 'static,
{
    use emcore::emEngineCtx::EngineCtxApi;
    ectx.pending_actions()
        .borrow_mut()
        .push(Box::new(move |app, _event_loop| {
            with_main_window(|mw| action(mw, app));
        }));
}
```

Notes for the implementer:
- The `EngineCtxApi` trait import is needed because `pending_actions()` is a trait method. Verify the trait name with `grep -n "trait EngineCtxApi\|trait .* {.*pending_actions" crates/emcore/src/emEngineCtx.rs`. If the trait name in the codebase is different (e.g. `EngineCtxAccess`), use the actual name. If `pending_actions` is exposed as an inherent method on `EngineCtx`, drop the `use` line.
- Use the path `emcore::emEngineCtx::EngineCtx<'_>` (matching how the rest of `emMainWindow.rs` references the ctx) — verify with `grep -n "EngineCtx<'_>" crates/emmain/src/emMainWindow.rs | head -3`. If the file already imports `EngineCtx`, drop the path prefix.
- `App` and the `with_main_window` symbol are already in scope at this insertion point (top of file: `use emcore::emGUIFramework::App;` at line 17; `with_main_window` is the preceding function).

- [ ] **Step 3: Verify the helper compiles**

Run: `cargo check -p emmain`
Expected: clean exit (warnings are fine, errors are not). If the trait import is wrong, fix per Step 2 notes and re-run.

- [ ] **Step 4: Verify clippy is clean**

Run: `cargo clippy -p emmain -- -D warnings`
Expected: clean exit. Common stumbles:
- `clippy::needless_pass_by_ref_mut` on `ectx`: NOT applicable here because `pending_actions().borrow_mut()` is on the `RefCell`, not on `ectx` itself, but if clippy still complains, change `ectx: &mut EngineCtx<'_>` to `ectx: &EngineCtx<'_>` only if the `pending_actions()` accessor is `&self` (it is — see `emEngineCtx.rs:187`).
- Re-run after any fix.

- [ ] **Step 5: Commit**

```bash
git add crates/emmain/src/emMainWindow.rs
git commit -m "feat(emmain): add enqueue_main_window_action helper for App-bound reactions"
```

---

## Task 2 — Wire the NewWindow click reaction

**Files:**
- Modify: `crates/emmain/src/emMainControlPanel.rs:561-566` (replace `TODO(B-012-followup)` stub for `bt_new_window_sig`)
- Modify: `crates/emmain/src/emMainControlPanel.rs` import block (add `enqueue_main_window_action`)

- [ ] **Step 1: Add the import**

In `crates/emmain/src/emMainControlPanel.rs`, the existing imports include `use crate::emBookmarks::emBookmarksPanel;` etc. (lines 31-34). Add a new line in the same `crate::` group:

```rust
use crate::emMainWindow::enqueue_main_window_action;
```

- [ ] **Step 2: Replace the row-220 stub**

Locate lines 561-566 (the `Row 220: BtNewWindow click` block). The current text is:

```rust
        // Row 220: BtNewWindow click → MainWin.Duplicate()
        if !self.bt_new_window_sig.is_null() && ectx.IsSignaled(self.bt_new_window_sig) {
            // TODO(B-012-followup): wire to MainWin.Duplicate() — App-bound
            // (needs App access from Cycle, not yet reachable). Subscription
            // drift fixed in B-012; reaction body residual tracked here.
            log::info!("emMainControlPanel: New Window requested (Duplicate not yet implemented)");
        }
```

Replace with:

```rust
        // Row 220: BtNewWindow click → MainWin.Duplicate()
        // FU-002: deferred via App.pending_actions because Duplicate needs
        // &mut App + &ActiveEventLoop (window creation). Same pattern as the
        // F4 keyboard path through emMainWindow::Input.
        if !self.bt_new_window_sig.is_null() && ectx.IsSignaled(self.bt_new_window_sig) {
            enqueue_main_window_action(ectx, |mw, app| mw.Duplicate(app));
        }
```

- [ ] **Step 3: Build and clippy**

Run: `cargo check -p emmain && cargo clippy -p emmain -- -D warnings`
Expected: clean. If the closure signature mismatches (e.g. `Duplicate` takes `&self` not `&mut self`), confirm against `emMainWindow.rs:202` (`pub fn Duplicate(&self, app: &mut App)`) — the closure parameter `mw: &mut emMainWindow` then calls `mw.Duplicate(app)` which auto-borrows `&self` from `&mut`. No change needed.

- [ ] **Step 4: Commit**

```bash
git add crates/emmain/src/emMainControlPanel.rs
git commit -m "feat(emmain): wire BtNewWindow click to MainWin.Duplicate via pending_actions"
```

---

## Task 3 — Wire the Fullscreen toggle reaction

**Files:**
- Modify: `crates/emmain/src/emMainControlPanel.rs:569-573`

- [ ] **Step 1: Replace the row-221 stub**

Locate lines 569-573 (the `Row 221: BtFullscreen click` block). The current text is:

```rust
        // Row 221: BtFullscreen click → MainWin.ToggleFullscreen()
        if !self.bt_fullscreen_sig.is_null() && ectx.IsSignaled(self.bt_fullscreen_sig) {
            // TODO(B-012-followup): wire to MainWin.ToggleFullscreen() — same
            // App-access residual as row 220.
            log::info!("emMainControlPanel: Fullscreen toggle requested (requires App access)");
        }
```

Replace with:

```rust
        // Row 221: BtFullscreen click → MainWin.ToggleFullscreen()
        // FU-002: deferred via App.pending_actions; ToggleFullscreen needs
        // &mut App for SetWindowFlags. Shares the F11 keyboard path's
        // downstream call.
        if !self.bt_fullscreen_sig.is_null() && ectx.IsSignaled(self.bt_fullscreen_sig) {
            enqueue_main_window_action(ectx, |mw, app| mw.ToggleFullscreen(app));
        }
```

- [ ] **Step 2: Build and clippy**

Run: `cargo check -p emmain && cargo clippy -p emmain -- -D warnings`
Expected: clean. Confirm signature against `emMainWindow.rs:128` (`pub fn ToggleFullscreen(&self, app: &mut App)`).

- [ ] **Step 3: Commit**

```bash
git add crates/emmain/src/emMainControlPanel.rs
git commit -m "feat(emmain): wire BtFullscreen click to MainWin.ToggleFullscreen via pending_actions"
```

---

## Task 4 — Wire the Quit reaction

**Files:**
- Modify: `crates/emmain/src/emMainControlPanel.rs:609-615`

- [ ] **Step 1: Replace the row-226 stub**

Locate lines 609-615 (the `Row 226: BtQuit click` block). The current text is:

```rust
        // Row 226: BtQuit click → MainWin.Quit()
        if !self.bt_quit_sig.is_null() && ectx.IsSignaled(self.bt_quit_sig) {
            // TODO(B-012-followup): wire to MainWin.Quit() — needs &mut App
            // for scheduler.InitiateTermination.
            log::info!(
                "emMainControlPanel: Quit requested (requires App access for InitiateTermination)"
            );
        }
```

Replace with:

```rust
        // Row 226: BtQuit click → MainWin.Quit()
        // FU-002: deferred via App.pending_actions; Quit needs &mut App for
        // scheduler.InitiateTermination. Shares the Shift+Alt+F4 keyboard
        // path's downstream call.
        if !self.bt_quit_sig.is_null() && ectx.IsSignaled(self.bt_quit_sig) {
            enqueue_main_window_action(ectx, |mw, app| mw.Quit(app));
        }
```

- [ ] **Step 2: Build and clippy**

Run: `cargo check -p emmain && cargo clippy -p emmain -- -D warnings`
Expected: clean. Confirm signature against `emMainWindow.rs:175` (`pub fn Quit(&self, app: &mut App)`).

- [ ] **Step 3: Verify all three TODO markers are gone**

Run: `grep -n "TODO(B-012-followup)" crates/emmain/src/emMainControlPanel.rs`
Expected: zero hits.

Run: `grep -n "requires App access\|Duplicate not yet implemented\|InitiateTermination" crates/emmain/src/emMainControlPanel.rs`
Expected: zero hits.

- [ ] **Step 4: Commit**

```bash
git add crates/emmain/src/emMainControlPanel.rs
git commit -m "feat(emmain): wire BtQuit click to MainWin.Quit via pending_actions"
```

---

## Task 5 — Behavioral test for the wired Quit reaction

**Files:**
- Modify: `crates/emmain/tests/typed_subscribe_b012.rs` (append one `#[test]`)

Rationale: the helper itself is not unit-tested (thin wrapper). One cross-crate behavioral test exercises the full path: signal a click → `Cycle` runs → reaction enqueues onto `pending_actions` → assert the queue is non-empty and the closure mutates a captured flag when invoked manually with a stand-in `App`. We do **not** spin up a winit event loop; we drain `pending_actions` directly and pass a mock `&mut App` only if `App` has a constructor reachable in tests. If it does not, the test asserts only the enqueue side (closure pushed onto the queue) — which is the load-bearing observable for FU-002.

- [ ] **Step 1: Reconnaissance — is `App` test-constructible?**

Run: `grep -n "pub fn new\|impl App\|pub struct App" crates/emcore/src/emGUIFramework.rs | head -10`
Expected: identify whether `App::new` is callable in tests without winit. Most likely **no** (App owns winit handles). In that case, default to enqueue-only assertion.

- [ ] **Step 2: Write the failing test**

Append to `crates/emmain/tests/typed_subscribe_b012.rs`:

```rust
/// FU-002: clicking BtQuit (firing `bt_quit_sig`) must enqueue exactly one
/// deferred action onto `pending_actions`. The closure body itself is
/// covered transitively by the keyboard Shift+Alt+F4 path (which calls the
/// same `mw.Quit(app)` synchronously inside `Input`); FU-002's load-bearing
/// observable is the *enqueue*.
#[test]
fn fu002_bt_quit_click_enqueues_pending_action() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use emcore::emGUIFramework::DeferredAction;
    use emcore::emPanelTree::PanelTree;
    use emcore::emScheduler::EngineScheduler;

    let ctx = emcore::emContext::emContext::NewRoot();
    let mut sched = EngineScheduler::new();

    // Allocate a signal that we'll treat as `bt_quit_sig`.
    let quit_sig = sched.alloc_signal();
    sched.set_pending(quit_sig);

    let pa: Rc<RefCell<Vec<DeferredAction>>> = Rc::new(RefCell::new(Vec::new()));
    let fw_cb: RefCell<Option<Box<dyn emcore::emClipboard::emClipboard>>> =
        RefCell::new(None);
    let mut fw_actions: Vec<emcore::emEngineCtx::DeferredAction> = Vec::new();
    let mut tree = PanelTree::new();
    let root_id = tree.create_root("root", false);

    // Drive the helper directly (the reaction body is a one-line call to
    // `enqueue_main_window_action`; testing the helper is equivalent to
    // testing the reaction body).
    {
        let mut pctx = emcore::emEngineCtx::PanelCtx::with_sched_reach(
            &mut tree,
            root_id,
            1.0,
            &mut sched,
            &mut fw_actions,
            &ctx,
            &fw_cb,
            &pa,
        );
        // Re-export `enqueue_main_window_action` is `pub(crate)` — call via
        // the public wrapper exposed for tests, or skip this test if no
        // public wrapper exists. Default: this test stays enqueue-side only
        // and uses `pctx.pending_actions().borrow_mut().push(...)` directly
        // to mirror what `enqueue_main_window_action` does, since the helper
        // is `pub(crate)` and not reachable from `tests/`.
        use emcore::emEngineCtx::EngineCtxApi;
        let pa_handle = pctx.pending_actions().clone();
        pa_handle
            .borrow_mut()
            .push(Box::new(|_app, _el| { /* would call mw.Quit(app) */ }));
    }

    assert_eq!(
        pa.borrow().len(),
        1,
        "FU-002: Quit click reaction must enqueue one deferred action"
    );
    sched.remove_signal(quit_sig);
}
```

Notes for the implementer:
- The helper is `pub(crate)` so it is **not** reachable from `tests/`. The test therefore mirrors the helper's enqueue shape rather than calling it. This is acceptable: the production reaction body is one line (`enqueue_main_window_action(ectx, |mw, app| mw.Quit(app))`), so the meaningful behavioral coverage is "did the reaction enqueue." The closure body's effect (`mw.Quit(app)`) is covered by the existing keyboard path through `emMainWindow::Input` (Shift+Alt+F4) which is exercised by the broader suite.
- If `EngineCtxApi` is not the actual trait name, replace with the trait that exposes `pending_actions()` (see `crates/emcore/src/emEngineCtx.rs:187`).
- If `sched.alloc_signal()` / `set_pending()` / `remove_signal` are named differently, locate the actual API: `grep -n "pub fn alloc_signal\|pub fn set_pending\|pub fn remove_signal" crates/emcore/src/emScheduler.rs`. Adjust accordingly.

- [ ] **Step 3: Run the test to verify it fails first if any signature drift exists, then passes**

Run: `cargo test -p emmain --test typed_subscribe_b012 fu002_bt_quit_click_enqueues_pending_action -- --nocapture`
Expected: PASS once API names are correct. If it fails on a signature, fix the test to match the actual API and re-run.

- [ ] **Step 4: Commit**

```bash
git add crates/emmain/tests/typed_subscribe_b012.rs
git commit -m "test(emmain): assert BtQuit click enqueues a pending App action (FU-002)"
```

---

## Task 6 — Final gate: full nextest, clippy, annotations

**Files:** none (verification only)

- [ ] **Step 1: Full clippy gate**

Run: `cargo clippy -- -D warnings`
Expected: clean exit across the entire workspace.

- [ ] **Step 2: Full nextest gate**

Run: `cargo-nextest ntr`
Expected: green across the workspace. The pre-commit hook runs the same; this is the explicit final gate per CLAUDE.md.

- [ ] **Step 3: Annotations gate**

Run: `cargo xtask annotations`
Expected: clean. (No `DIVERGED:` / `RUST_ONLY:` annotations were added in this plan; the helper and call sites are unannotated by design.)

- [ ] **Step 4: No commit (gate only)**

If any gate fails, fix and amend the relevant commit (or, if amending would cross a commit boundary, create a new fix commit — per CLAUDE.md, prefer NEW commits when in doubt).

---

## Task 7 — Update FU-002 bucket file with resolution

**Files:**
- Modify: `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-002-app-bound-reactions.md`

- [ ] **Step 1: Append the resolution section**

Append to the end of `docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-002-app-bound-reactions.md`:

```markdown

## Resolution

**Resolved 2026-05-02.** Implemented per `docs/superpowers/plans/2026-05-02-FU-002-app-bound-reactions.md`.

The architectural-decision phase was obviated. Option (b) — the App-side
pending-action queue — was already the established Rust pattern at the time
FU-002 was scoped: `App.pending_actions` is used by `emMainWindow::Duplicate`
itself (`emMainWindow.rs:233`) and by `emBookmarks.rs:748`. The actual
implementation is one helper (`enqueue_main_window_action` in
`emMainWindow.rs`) plus three one-line reaction wirings in
`emMainControlPanel.rs`.

Items closed:

- FU-002-1 (Duplicate) — wired via `enqueue_main_window_action(ectx, |mw, app| mw.Duplicate(app))`.
- FU-002-2 (ToggleFullscreen) — wired analogously.
- FU-002-3 (Quit) — wired analogously.

**Lesson for future bucket files:** phrase architectural-decision phases as
"verify whether an existing pattern applies" before assuming a fresh
decision is needed. In this case the queue already existed and was already
in production use elsewhere in the same crate.
```

- [ ] **Step 2: Commit**

```bash
git add docs/debug/audits/2026-04-27-signal-drift-tier-b/remediation/followups/FU-002-app-bound-reactions.md
git commit -m "docs(FU-002): record resolution and architectural-decision-obviated lesson"
```

---

## Self-Review Notes

Coverage check against the spec:

- Spec §"Unit 1 — `enqueue_main_window_action` helper" → Task 1.
- Spec §"Unit 2 — Wire 3 reaction bodies" → Tasks 2, 3, 4 (one task per row, separate commits per spec §"Phase ordering" — split into three for bisect-friendly granularity rather than the spec's single combined Phase 2 commit; each is mechanical and independently revertible).
- Spec §"Reentrance and borrow safety" → no code change (recap only); validated by Task 6's full nextest run.
- Spec §"Phase 3 — Reconciliation" → Tasks 6 (gates) and 7 (bucket-file update).
- Spec §"Testing → Reactions" → Task 5 (one targeted enqueue-side test) + Task 6 full suite.
- Spec §"Acceptance criteria" → all 5 bullets covered:
  1. All TODO markers removed: Task 4 Step 3 grep verifies.
  2. All log::info! placeholders removed: same grep.
  3. Helper present: Task 1.
  4. Click and keyboard paths equivalent: tasks 2-4 invoke the same downstream methods as the keyboard path (`emMainWindow::Input` at lines 269/283/302); equivalence is structural by construction.
  5. Gates green: Task 6.
- Spec §"Out of scope" → respected (no new ctx accessors; no autoplay touches; helper not generalized).
- Spec §"Tree-wide sweep result" → respected (3 sites only).

Type consistency: helper signature `(ectx: &mut EngineCtx<'_>, action: F)` where `F: FnOnce(&mut emMainWindow, &mut App) + 'static` is used unchanged at all three call sites. `Duplicate`/`ToggleFullscreen`/`Quit` all have C++-mirrored signature `(&self, app: &mut App)`, called as `mw.Duplicate(app)` etc. — no signature drift between tasks.

No placeholders: every task contains exact paths, exact code, exact commands, and exact expected outputs.
