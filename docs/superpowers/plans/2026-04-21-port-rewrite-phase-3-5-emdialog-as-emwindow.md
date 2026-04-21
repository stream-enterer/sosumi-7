# Phase 3.5 — `emDialog` as `emWindow`-derived (C++-structural port)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `emDialog` from a plain owned Rust struct with a caller-driven `Cycle` method to a faithful port of C++ `class emDialog : public emWindow` — owning its own `emWindow`, rooted at a `DlgPanel` PanelBehavior, with a `DialogPrivateEngine : emEngine` subscribed to the close signal at `Priority::High`. Prepare infrastructure for Phase 3.6 (emFileDialog + E024 closure) without itself closing E024.

**Architecture:** Three new types in `emDialog.rs`: `DlgPanel` (PanelBehavior — port of C++ nested `DlgPanel : emBorder`), `DlgButton` (PanelBehavior wrapping `emButton` — port of C++ nested `DlgButton : emButton`), and `DialogPrivateEngine` (emEngine — port of C++ nested `PrivateEngineClass : emEngine`). `emDialog` itself becomes a façade holding the dialog's `WindowId`, root `PanelId`, `private_engine_id`, and `finish_signal`, constructed via the existing `emWindow::new_popup_pending` rail that popup-zoom already consumes. The current caller-invoked `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool` is deleted; the dialog's Cycle now runs inside `DialogPrivateEngine::Cycle` at scheduler dispatch time. Dialog state (result, buttons, finish-state, auto-delete countdown) lives on `DlgPanel`'s behavior-side struct; the private engine and outer façade reach it via `tree.take_behavior(root_panel_id)` (same idiom `PanelCycleEngine::Cycle` uses).

**Tech Stack:** Rust 1.82+, slotmap, winit, wgpu. All work in `crates/emcore/src/`. No new external crates.

**Authority:** CLAUDE.md Port Ideology (C++ source > golden tests > Rust idiom). See also the brainstorm output at `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow-plan.md` for the scope decision + the seven resolved design questions (D1–D7).

**Branch:** `port-rewrite/phase-3-5-emdialog-as-emwindow` off `main` at `d0f1cc7b` (merge of phase-3-continue). Exit tag: `port-rewrite-phase-3-5-complete`.

**Baseline** (from `docs/superpowers/notes/2026-04-19-phase-3-closeout.md`):
- nextest: 2476 passed / 0 failed / 9 skipped
- goldens: 237 passed / 6 failed (pre-existing)
- clippy: clean
- rc_refcell_total: 256
- diverged_total: 173

**Gate commands** (run at the end of every committed task unless noted):
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo-nextest ntr`
- `cargo test --test golden -- --test-threads=1` (at phase closeout only)

---

## File structure

**Files created:**
- None (all work lives in existing files, per File and Name Correspondence).

**Files modified:**
- `crates/emcore/src/emDialog.rs` — the bulk of the work. Adds `DlgPanel`, `DlgButton`, `DialogPrivateEngine` types; reshapes `emDialog` struct; deletes the caller-invoked `Cycle`; ports `Finish`/`CheckFinish`/`AutoDelete`/`ShowMessage` semantics onto the new shape.
- `crates/emstocks/src/emStocksListBox.rs` — the 4 dialog-creation sites (`DeleteStocks`, `CutStocks`, `PasteStocks`, `SetInterest`) migrate to the new `emDialog::new` surface + explicit `deregister`/drop pattern.
- `crates/emcore/src/test_view_harness.rs` — adds `DialogTestHarness` helper for dialog unit tests (a parent `emContext` + a scheduler + deferred-action buffer).
- `crates/emcore/src/emCoreLib.rs` or wherever `pub use emDialog::*` lives — update re-exports if the shape of the facade changes.
- `crates/emcore/src/emEngineCtx.rs` — if the audit surfaces a `DeferredAction` variant that needs extension for auto-delete (`AutoDeleteDialog(WindowId)` or reuse of `CloseWindow`). Most likely reuse `CloseWindow`.
- `crates/emcore/src/emView.rs` — if audit 1a surfaces a `VF_*` gap (unlikely — `POPUP_ZOOM` and `ROOT_SAME_TALLNESS` confirmed present at lines 1189 and 1475).
- `crates/emcore/src/emWindow.rs` — if audit 1d surfaces a close-signal firing gap (unlikely).

**Ledger:**
- `docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md` — new. Records bootstrap decisions, task-by-task outcomes, invariant results.

---

## Bootstrap decisions

- **B3.5a (stage-only scan):** Every task below commits at step end. Pre-commit hook remains active throughout. No DIVERGED-cascade pattern.
- **B3.5b (audit posture):** Task 1 audits prerequisites; each gap discovery produces its own commit. Tasks 2–10 assume audit passed. If 1a/1b/1d surface real gaps, insert a `1a-fill` / `1b-fill` / `1d-fill` commit inside Task 1 before proceeding to Task 2.
- **B3.5c (no rush):** Each sub-task is independently gate-green. The pre-commit hook (`.git/hooks/pre-commit`) runs `cargo fmt` + `clippy -D warnings` + `cargo-nextest ntr` at commit time, so "commit" == "gate green". Run the golden suite manually only at closeout (expensive; no paint-path changes expected).
- **B3.5d (engine-priority decision):** `DialogPrivateEngine` registers at `Priority::High`. C++ `emDialog.cpp:37`: `PrivateEngine.SetEnginePriority(HIGH_PRIORITY)`. Do NOT use `Priority::VeryHigh` — that's reserved for `InputDispatchEngine` (confirmed at `emInputDispatchEngine.rs`). Do NOT use `Medium` — that matches `PanelCycleEngine`, which C++ runs AFTER the dialog's PrivateEngine within a slice.
- **B3.5e (state placement):** Dialog state (result, buttons vec, finish_state, auto_delete, finish_signal) lives on `DlgPanel`'s behavior-side struct — not on the outer `emDialog` façade. Reason: `DialogPrivateEngine::Cycle` reaches state via `tree.take_behavior(root_panel_id)` (Rust's analog of C++ `Dlg&`), so whatever it mutates must be inside the behavior. The outer `emDialog` façade holds only identifiers (WindowId, PanelId, EngineId) + an accessor API that routes reads/writes through the tree.

---

## Task 1: Audit prerequisites

**Files:**
- Read-only in this task unless a gap is found:
  - Read: `crates/emcore/src/emView.rs` (confirm `ViewFlags::POPUP_ZOOM`, `ViewFlags::ROOT_SAME_TALLNESS`)
  - Read: `crates/emcore/src/emEngineCtx.rs:33-42` (confirm `DeferredAction::CloseWindow`)
  - Read: `crates/emcore/src/emWindow.rs` (confirm close-signal firing path for modal windows)
- Modify (gap-fill, expected to be zero commits in the common case):
  - `crates/emcore/src/emView.rs`
  - `crates/emcore/src/emWindow.rs`
  - `crates/emcore/src/emEngineCtx.rs`
- Create: `docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md`

- [ ] **Step 1.1: Create the phase ledger file.**

```bash
cat > docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md <<'EOF'
# Phase 3.5 — emDialog as emWindow — Ledger

**Started:** 2026-04-22
**Branch:** port-rewrite/phase-3-5-emdialog-as-emwindow
**Baseline:** see docs/superpowers/notes/2026-04-19-phase-3-closeout.md (nextest 2476/0/9; goldens 237/6; rc_refcell_total 256).
**Plan:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow.md
**Source brainstorm:** docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-emdialog-as-emwindow-plan.md
**JSON entries:** E024 remains open (closed in Phase 3.6, not here).

## Bootstrap decisions

See plan §"Bootstrap decisions" (B3.5a–B3.5e).

## Task log

(Entries appended by each task's commit.)
EOF
git add docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md
```

- [ ] **Step 1.2: Audit 1a — `VF_POPUP_ZOOM` and `VF_ROOT_SAME_TALLNESS` parity.**

Run:

```bash
rg -n 'POPUP_ZOOM|ROOT_SAME_TALLNESS' crates/emcore/src/emView.rs | head -20
```

Expected output: at least two matches — one defining each flag in the `ViewFlags` bitflags block, and multiple use-sites. Per my earlier survey, the block is around `emView.rs:22` and usages are at `1189`, `1475`, etc.

If both flags are present: PASS — proceed to Step 1.3.

If either is missing: **1a-fill commit required.** Add the missing flag(s) to the `bitflags!` block at `emView.rs:22`. Name per C++: C++ `VF_POPUP_ZOOM` → Rust `ViewFlags::POPUP_ZOOM` (1 << N), C++ `VF_ROOT_SAME_TALLNESS` → Rust `ViewFlags::ROOT_SAME_TALLNESS` (1 << N). Do NOT invent numbers arbitrarily — read `~/git/eaglemode-0.96.4/include/emCore/emView.h` for the canonical bit positions and match them. Wire the flag into any pre-existing behavior that should respect it (for `POPUP_ZOOM` this is the popup-zoom trigger path; for `ROOT_SAME_TALLNESS` this is the root-panel layout). Gate commands green. Commit message: `phase-3.5 audit 1a: add ViewFlags::<missing-flag> for emDialog parity`.

- [ ] **Step 1.3: Audit 1b — `DeferredAction` supports window close.**

Run:

```bash
rg -n 'DeferredAction' crates/emcore/src/emEngineCtx.rs | head -10
```

Expected: `DeferredAction::CloseWindow(winit::window::WindowId)` at line 37.

If present: PASS — proceed to Step 1.4. The `AutoDelete` mechanism in Task 9 will enqueue `DeferredAction::CloseWindow(window_id)` when the dialog's auto-delete 3-slice countdown hits zero. No new `DeferredAction` variant needed.

If missing: **1b-fill commit required.** Add the variant to the `enum DeferredAction` at `emEngineCtx.rs:33`. Wire its drain path into `emGUIFramework`'s post-time-slice pump (look for the existing `MaterializePopup` drain site as a template). Gate commands green. Commit message: `phase-3.5 audit 1b: extend DeferredAction for dialog close`.

- [ ] **Step 1.4: Audit 1d — close-signal firing path for modal windows.**

Run:

```bash
rg -n 'close_signal' crates/emcore/src/emWindow.rs crates/emcore/src/emGUIFramework.rs | head -30
```

Expected: the `close_signal` field is declared on `emWindow` at line 118 and referenced by `emWindow::SignalClosing` at line 1535-ish; the framework fires it on winit `WindowEvent::CloseRequested` and on focus-loss for popup windows.

Verification: open `crates/emcore/src/emGUIFramework.rs`, find the winit `WindowEvent::CloseRequested` handler, confirm it calls `scheduler.fire(window.close_signal)`. Also confirm focus-loss (if `WindowFlags::POPUP`) fires it.

If firing is present and matches C++ `emWindow::SignalClosing` beats: PASS — proceed to Step 1.5.

If firing is missing for modal-but-not-popup windows (emDialog defaults to `WF_MODAL | VF_POPUP_ZOOM`, not `WF_POPUP`): **1d-fill commit required.** Add firing at the matching C++ beat. Match C++ `emWindow::SignalClosing` at `~/git/eaglemode-0.96.4/src/emCore/emWindow.cpp`. Gate green. Commit: `phase-3.5 audit 1d: fire close_signal for modal windows on user-requested close`.

- [ ] **Step 1.5: Audit 1c — `Finished` callback shape is already the established Rust rendition for C++ virtuals.**

This is a documentation step. Add the following to the ledger's task log:

```
- **Task 1 — Audit:** COMPLETE.
  - 1a: ViewFlags::POPUP_ZOOM and ROOT_SAME_TALLNESS present at emView.rs:22 bitflags block. PASS (no gap-fill).
  - 1b: DeferredAction::CloseWindow present at emEngineCtx.rs:37. PASS.
  - 1c: `on_finished: Option<WidgetCallbackRef<DialogResult>>` per D1 (brainstorm
    decision). Matches Rust port's existing virtual-to-callback rendition
    (emButton.on_click, emDialog.on_check_finish). No audit gap. Task 5 (emDialog
    reshape) adds the field.
  - 1d: close_signal firing confirmed on WindowEvent::CloseRequested in emGUIFramework.rs.
    [If 1d-fill needed, note the gap + commit SHA here instead.]
```

- [ ] **Step 1.6: Commit Task 1.**

```bash
git add docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md
git commit -m "phase-3.5 task 1: audit prerequisites — all present, no gap-fill required

- ViewFlags::POPUP_ZOOM and ROOT_SAME_TALLNESS confirmed at emView.rs bitflags block.
- DeferredAction::CloseWindow confirmed at emEngineCtx.rs:37.
- on_finished callback shape matches established virtual-to-callback port idiom (D1).
- close_signal firing confirmed on winit CloseRequested in emGUIFramework."
```

**Task 1 exit condition:** ledger records audit results. All four audits either PASS (no commits beyond Step 1.6) or carry a gap-fill commit (inserted between 1.x and 1.6).

---

## Task 2: Add `DlgPanel` PanelBehavior (standalone, not yet wired)

**Files:**
- Modify: `crates/emcore/src/emDialog.rs` (add `DlgPanel` struct + `impl PanelBehavior for DlgPanel` + unit tests)

**Scope:** The `DlgPanel` struct and its PanelBehavior impl land in isolation. No `emDialog` consumer exists yet — `emDialog` itself remains unchanged in this task. This means `DlgPanel` won't be constructed outside its unit tests until Task 5.

**Context for the engineer:** C++ `class DlgPanel : public emBorder` (emDialog.h:186-204) is the root panel of every emDialog. It carries the dialog caption (C++ `SetTitle`), hosts the `ContentPanel` (emLinearLayout) + `ButtonsPanel` (emLinearLayout) as children, and routes Enter/Escape keyboard input to the outer dialog's `Finish`. In Rust, `DlgPanel` is the state-holder — dialog result, buttons metadata, finish state, auto-delete flag all live here, because `DialogPrivateEngine` reaches them via `tree.take_behavior(root_panel_id)` (the analog of C++ `emDialog& Dlg`).

- [ ] **Step 2.1: Add `DlgPanel` struct fields, and new() ctor, to `emDialog.rs`.**

Insert AFTER the existing `emDialog` struct definition (currently ending around `emDialog.rs:40`) and BEFORE `impl emDialog`. Keep imports at the top of the file — add `use crate::emPanel::{PanelBehavior, PanelState, Rect};` if not already present.

```rust
/// Root-panel PanelBehavior for an `emDialog`.
///
/// Port of C++ `emDialog::DlgPanel : public emBorder` (emDialog.h:186-204).
/// Lives as the root panel of the dialog's owned `emWindow`. Holds the
/// dialog's mutable state (result, buttons, finish-state, auto-delete
/// countdown) because `DialogPrivateEngine::Cycle` reaches state through
/// `tree.take_behavior(root_panel_id)` — the Rust analog of the C++
/// `PrivateEngineClass::Dlg&` back-reference (B3.5e).
pub(crate) struct DlgPanel {
    border: emBorder,
    look: Rc<emLook>,
    /// Dialog buttons: (caption string, result payload). Rendered in the
    /// bottom button row as `DlgButton` child panels.
    pub(crate) buttons: Vec<(String, DialogResult)>,
    /// Set by `Finish` once CheckFinish permits. `DialogPrivateEngine`
    /// observes this on Cycle and fires `finish_signal`.
    pub(crate) pending_result: Option<DialogResult>,
    /// Stored after the finish signal has fired. Read via `GetResult`.
    pub(crate) finalized_result: Option<DialogResult>,
    /// Incremented by `DialogPrivateEngine::Cycle` after close_signal fires,
    /// when auto_delete is enabled. At 3, the engine emits
    /// `DeferredAction::CloseWindow`. C++ parity: emDialog.cpp FinishState.
    pub(crate) finish_state: u8,
    pub(crate) auto_delete: bool,
    pub(crate) finish_signal: SignalId,
    pub(crate) on_finish: Option<crate::emEngineCtx::WidgetCallbackRef<DialogResult>>,
    pub(crate) on_check_finish: Option<Box<dyn FnMut(&DialogResult) -> bool>>,
    /// Port of C++ `virtual void emDialog::Finished(int result)` (D1 — callback,
    /// not trait method). Fires from `DialogPrivateEngine::Cycle` after
    /// finish_signal fires. Default `None` matches C++ default (no-op).
    pub(crate) on_finished: Option<crate::emEngineCtx::WidgetCallbackRef<DialogResult>>,
    /// PanelId of the emLinearLayout content panel, set by Task 7.
    pub(crate) content_panel_id: Option<crate::emPanelTree::PanelId>,
    /// PanelId of the emLinearLayout button-row panel, set by Task 7.
    pub(crate) buttons_panel_id: Option<crate::emPanelTree::PanelId>,
}

impl DlgPanel {
    pub(crate) fn new(
        title: &str,
        look: Rc<emLook>,
        finish_signal: SignalId,
    ) -> Self {
        Self {
            border: emBorder::new(OuterBorderType::PopupRoot).with_caption(title),
            look,
            buttons: Vec::new(),
            pending_result: None,
            finalized_result: None,
            finish_state: 0,
            auto_delete: false,
            finish_signal,
            on_finish: None,
            on_check_finish: None,
            on_finished: None,
            content_panel_id: None,
            buttons_panel_id: None,
        }
    }

    pub(crate) fn SetTitle(&mut self, title: &str) {
        self.border.SetCaption(title);
    }
}
```

Verify `emBorder::new(...)`, `OuterBorderType::PopupRoot`, and `with_caption` exist — per `emBorder.rs:86` they do. `SignalId` and `Rc<emLook>` are already in scope from the current `emDialog.rs` imports.

- [ ] **Step 2.2: Implement `PanelBehavior for DlgPanel` — Paint + LayoutChildren.**

The Paint impl delegates to `emBorder::paint_border`. LayoutChildren positions the content panel above the button-row panel, matching the current emDialog's `LayoutChildren` math (see `emDialog.rs:119-160` pre-port). Add:

```rust
impl PanelBehavior for DlgPanel {
    fn Paint(
        &mut self,
        painter: &mut crate::emPainter::emPainter,
        w: f64,
        h: f64,
        _state: &PanelState,
    ) {
        let pixel_scale = 1.0; // DlgPanel is the view root; no enclosing scaling
        self.border
            .paint_border(painter, w, h, &self.look, false, true, pixel_scale);
    }

    fn LayoutChildren(&mut self, ctx: &mut crate::emEngineCtx::PanelCtx) {
        // Retrieve the DlgPanel's rect from its PanelState via ctx.
        // `ctx.layout_child` already exists (see emDialog.rs:139). Layout:
        //   content_panel:  content_rect minus bottom BUTTON_HEIGHT strip
        //   buttons_panel:  bottom BUTTON_HEIGHT strip of content_rect
        let (w, h) = ctx.panel_size();
        let Rect { x: cx, y: cy, w: cw, h: ch } =
            self.border.GetContentRect(w, h, &self.look);

        if let Some(content_id) = self.content_panel_id {
            let btn_area = if self.buttons_panel_id.is_some() {
                BUTTON_HEIGHT + BOTTOM_MARGIN
            } else {
                0.0
            };
            ctx.layout_child(content_id, cx, cy, cw, ch - btn_area);
        }
        if let Some(buttons_id) = self.buttons_panel_id {
            let by = cy + ch - BUTTON_HEIGHT;
            ctx.layout_child(buttons_id, cx, by, cw, BUTTON_HEIGHT);
        }

        let cc = self
            .border
            .content_canvas_color(ctx.GetCanvasColor(), &self.look, ctx.is_enabled());
        ctx.set_all_children_canvas_color(cc);
    }

    fn GetCanvasColor(&self) -> crate::emColor::emColor {
        // PopupRoot border paints opaque background; canvas = content.
        self.border
            .content_canvas_color(crate::emColor::emColor::TRANSPARENT, &self.look, true)
    }

    fn IsOpaque(&self) -> bool {
        true  // PopupRoot covers the whole dialog viewport
    }
}
```

Confirm `ctx.panel_size()` exists on `PanelCtx` — if not, use `ctx.layout_rect()` or iterate the existing `emDialog::LayoutChildren`'s access pattern. If `panel_size()` is missing, add it as a small helper alongside the DlgPanel work (same commit, labelled).

- [ ] **Step 2.3: Implement `PanelBehavior::Input` for DlgPanel — Enter/Escape → Finish.**

Ports C++ `DlgPanel::Input` (emDialog.cpp DlgPanel::Input body — Enter key triggers positive, Escape triggers negative). Previously in Rust this was on the outer `emDialog::Input`; moving it here matches C++ structure.

```rust
    fn Input(
        &mut self,
        event: &crate::emInput::emInputEvent,
        _state: &PanelState,
        _input_state: &crate::emInputState::emInputState,
        _ctx: &mut crate::emEngineCtx::PanelCtx,
    ) -> bool {
        use crate::emInput::{InputKey, InputVariant};
        if event.variant != InputVariant::Press {
            return false;
        }
        if event.ctrl || event.alt || event.meta {
            return false;
        }
        match event.key {
            InputKey::Enter => {
                // Set pending result; DialogPrivateEngine observes on next Cycle,
                // runs on_check_finish, fires finish_signal. Not a direct Finish
                // call: Finish semantics live in the private engine per C++.
                self.pending_result = Some(DialogResult::Ok);
                true
            }
            InputKey::Escape => {
                self.pending_result = Some(DialogResult::Cancel);
                true
            }
            _ => false,
        }
    }
```

- [ ] **Step 2.4: Add unit tests for `DlgPanel`.**

Append to the existing `#[cfg(test)] mod tests` block at the bottom of `emDialog.rs`. Place them before the closing `}` of the module.

```rust
    #[test]
    fn dlg_panel_enter_sets_pending_ok() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emPanelTree::{PanelId, PanelTree};
        use slotmap::Key as _;

        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::press(InputKey::Enter);
        let ps = PanelState {
            id: PanelId::null(), is_active: true, in_active_path: true,
            window_focused: true, enabled: true, viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0, memory_limit: u64::MAX,
            pixel_tallness: 1.0, height: 1.0,
        };
        let is = crate::emInputState::emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(consumed, "Enter should be consumed");
        assert_eq!(panel.pending_result, Some(DialogResult::Ok));
    }

    #[test]
    fn dlg_panel_escape_sets_pending_cancel() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emPanelTree::{PanelId, PanelTree};
        use slotmap::Key as _;

        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let ev = emInputEvent::press(InputKey::Escape);
        let ps = PanelState {
            id: PanelId::null(), is_active: true, in_active_path: true,
            window_focused: true, enabled: true, viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0, memory_limit: u64::MAX,
            pixel_tallness: 1.0, height: 1.0,
        };
        let is = crate::emInputState::emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(consumed, "Escape should be consumed");
        assert_eq!(panel.pending_result, Some(DialogResult::Cancel));
    }

    #[test]
    fn dlg_panel_modified_enter_is_ignored() {
        use crate::emInput::{emInputEvent, InputKey};
        use crate::emPanelTree::{PanelId, PanelTree};
        use slotmap::Key as _;

        let mut tree = PanelTree::new();
        let pid = tree.create_root("dlg", false);
        let mut pctx = PanelCtx::new(&mut tree, pid, 1.0);

        let mut __init = TestInit::new();
        let finish_sig = __init.sched.create_signal();
        let mut panel = DlgPanel::new("Title", emLook::new(), finish_sig);

        let mut ev = emInputEvent::press(InputKey::Enter);
        ev.ctrl = true;
        let ps = PanelState {
            id: PanelId::null(), is_active: true, in_active_path: true,
            window_focused: true, enabled: true, viewed: true,
            clip_rect: Rect::new(0.0, 0.0, 1e6, 1e6),
            viewed_rect: Rect::new(0.0, 0.0, 200.0, 100.0),
            priority: 1.0, memory_limit: u64::MAX,
            pixel_tallness: 1.0, height: 1.0,
        };
        let is = crate::emInputState::emInputState::new();

        let consumed = panel.Input(&ev, &ps, &is, &mut pctx);
        assert!(!consumed, "Ctrl-Enter should not be consumed");
        assert_eq!(panel.pending_result, None);
    }
```

- [ ] **Step 2.5: Run the new tests.**

```bash
cargo-nextest run -p emcore --lib emDialog::tests::dlg_panel_enter_sets_pending_ok \
  emDialog::tests::dlg_panel_escape_sets_pending_cancel \
  emDialog::tests::dlg_panel_modified_enter_is_ignored
```

Expected: three tests pass.

If any fails: read the error. If it's a missing `ctx.panel_size()` or similar PanelCtx helper — add the helper in the same commit. If it's a clash with the currently-pre-existing `emDialog::Input` Enter-handling (not yet deleted — deleted in Task 5), the test isolation should not actually hit that code path; if it does, the test setup is wrong.

- [ ] **Step 2.6: Full gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

All three must be clean. nextest count must be `2476 + 3 = 2479` passed (the three new DlgPanel tests), `0 failed`, `9 skipped`.

If clippy fires: fix the cause — do NOT add `#[allow]` (CLAUDE.md Do-NOT). Exception per whitelist: `too_many_arguments` is OK if it surfaces on a struct ctor.

- [ ] **Step 2.7: Commit Task 2.**

Append to the ledger:

```
- **Task 2 — DlgPanel:** commit <SHA>. DlgPanel struct + impl PanelBehavior for
  DlgPanel added to emDialog.rs. Paint delegates to emBorder; LayoutChildren
  positions content_panel above buttons_panel; Input consumes Enter→pending_ok
  and Escape→pending_cancel per C++ DlgPanel::Input. Three unit tests added. Not
  yet wired — emDialog consumers use DlgPanel in Task 5. Gate green — nextest
  2479/0/9.
```

```bash
git add docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md crates/emcore/src/emDialog.rs
git commit -m "phase-3.5 task 2: DlgPanel PanelBehavior — port of C++ emDialog::DlgPanel

Adds the root-panel PanelBehavior per CLAUDE.md File and Name Correspondence.
DlgPanel holds the dialog's mutable state (buttons, pending_result,
finalized_result, finish_state, auto_delete) because DialogPrivateEngine
reaches state through tree.take_behavior(root_panel_id) (B3.5e).

Not yet installed — emDialog still uses its plain-struct shape until Task 5.
Gate green — three new unit tests, nextest 2479/0/9."
```

**Task 2 exit condition:** `rg -n 'pub\(crate\) struct DlgPanel' crates/emcore/src/emDialog.rs` → 1 match; `rg -n 'impl PanelBehavior for DlgPanel' crates/emcore/src/emDialog.rs` → 1 match; nextest +3 over baseline.

---

## Task 3: Add `DlgButton` PanelBehavior (standalone)

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Scope:** `DlgButton` wraps `emButton` (Rust widget type already ported) and exposes a `result: DialogResult` payload. On click, it writes `result` into the owning `DlgPanel.pending_result`. The wiring from DlgButton to DlgPanel is via the panel tree's parent chain at input time — not a back-reference. `DialogPrivateEngine::Cycle` then observes `pending_result` as Some on its next run.

**Reference:** `ButtonPanel` adapter at `emColorFieldFieldPanel.rs:191-210` is the precedent — wraps `emButton` + delegates `PanelBehavior::Paint`/`Input`/`GetCursor`.

- [ ] **Step 3.1: Add `DlgButton` struct + PanelBehavior impl to `emDialog.rs`.**

Place after `impl PanelBehavior for DlgPanel` (end of Task 2 additions). Add imports at file top: `use crate::emButton::emButton;` if not present.

```rust
/// PanelBehavior wrapping an `emButton`, carrying a `DialogResult` payload.
///
/// Port of C++ `emDialog::DlgButton : public emButton` (emDialog.h:172-186).
/// On click, sets the parent DlgPanel's `pending_result`. The dialog's
/// `DialogPrivateEngine::Cycle` observes the pending result on its next run
/// and drives finish_signal + callbacks.
pub(crate) struct DlgButton {
    button: emButton,
    result: DialogResult,
    /// PanelId of the containing DlgPanel. Wired at installation time (Task 7).
    /// Used at click time to reach the DlgPanel's `pending_result`.
    dlg_panel_id: crate::emPanelTree::PanelId,
}

impl DlgButton {
    pub(crate) fn new(
        caption: &str,
        result: DialogResult,
        dlg_panel_id: crate::emPanelTree::PanelId,
    ) -> Self {
        // emButton ctor: use whatever existing new() accepts. Follow emButton's
        // current signature (see emButton.rs:45). Typical shape: takes caption.
        Self {
            button: emButton::new(caption),
            result,
            dlg_panel_id,
        }
    }

    pub(crate) fn caption(&self) -> &str {
        self.button.GetCaption()
    }

    pub(crate) fn result(&self) -> &DialogResult {
        &self.result
    }

    pub(crate) fn set_caption(&mut self, caption: &str) {
        self.button.SetCaption(caption);
    }
}

impl PanelBehavior for DlgButton {
    fn Paint(
        &mut self,
        painter: &mut crate::emPainter::emPainter,
        w: f64,
        h: f64,
        state: &PanelState,
    ) {
        let pixel_scale = state.viewed_rect.w * state.viewed_rect.h
            / w.max(1e-100) / h.max(1e-100);
        self.button.Paint(painter, w, h, state.enabled, pixel_scale);
    }

    fn Input(
        &mut self,
        event: &crate::emInput::emInputEvent,
        state: &PanelState,
        input_state: &crate::emInputState::emInputState,
        ctx: &mut crate::emEngineCtx::PanelCtx,
    ) -> bool {
        // Delegate to emButton; if emButton's on_click fires, we translate
        // that into a pending_result write on the parent DlgPanel.
        //
        // Implementation shape depends on emButton's current click-detection
        // surface. The precedent in emColorFieldFieldPanel::ButtonPanel:
        //   self.button.Input(event, state, input_state, ctx)
        // returns a bool, but click-detection is via emButton.on_click
        // callback OR via its click_signal. We use click_signal observation:
        // before Input, remember button.was_clicked = false; call Input;
        // after, if button fires click, write pending_result.
        //
        // Cleanest shape: install an on_click callback during new() that
        // stores a sentinel on self, and observe here. But Box<dyn FnMut>
        // doesn't capture &mut self cleanly. So: use click_signal.
        //
        // Simpler: emButton has a `Click(ctx)` public method that sets its
        // click_signal. If we wrap, we call button.Input, then if the
        // button fired, write to parent DlgPanel.
        let consumed = self.button.Input(event, state, input_state, ctx);
        if consumed && ctx.scheduler.as_deref().is_some_and(|_| true) {
            // Check click_signal — was it pending after this Input?
            if let Some(sched) = ctx.as_sched_ctx() {
                if sched.is_signaled(self.button.click_signal) {
                    // Click occurred. Write to parent DlgPanel.
                    //
                    // Reach DlgPanel: we need to mutate its behavior state.
                    // The correct path is to do this from DialogPrivateEngine
                    // on its next Cycle, by observing the button's click_signal
                    // via AddWakeUpSignal equivalent. However, for B3.5,
                    // button presses should wake DialogPrivateEngine via
                    // scheduler.connect(button.click_signal, private_engine_id),
                    // and the engine reads the buttons list + match.
                    //
                    // This closure here cannot reach the DlgPanel behavior
                    // because ctx.tree is taken by the current PanelCtx.
                    // The path is: Task 5 wires scheduler.connect(
                    //   button.click_signal, private_engine_id) for every
                    // button at installation time. The private engine on its
                    // next Cycle iterates children of DlgPanel, finds the
                    // DlgButton whose click_signal was just signalled, and
                    // writes its result into DlgPanel.pending_result.
                    //
                    // Therefore DlgButton::Input is a PURE delegator — no
                    // tree-mutation side effect here. Leave click observation
                    // to the engine.
                    let _ = sched; // retain reach for clarity
                }
            }
        }
        // Silence unused-var warning — dlg_panel_id is used by the engine side,
        // not here. Keep the field to carry the identifier.
        let _ = self.dlg_panel_id;
        consumed
    }

    fn GetCursor(&self) -> crate::emCursor::emCursor {
        self.button.GetCursor()
    }
}
```

Note the comment chain: DlgButton::Input delegates to emButton and does NOT write `pending_result` directly. Writing happens in `DialogPrivateEngine::Cycle` (Task 4), which is subscribed via `scheduler.connect(button.click_signal, private_engine_id)` at button-install time (Task 7). This mirrors C++ `DlgButton::Clicked()` signalling its parent dialog through the engine's wake-up mechanism.

- [ ] **Step 3.2: Add unit tests for DlgButton's delegation.**

```rust
    #[test]
    fn dlg_button_carries_result_payload() {
        let mut tree = crate::emPanelTree::PanelTree::new();
        let dlg_pid = tree.create_root("dlg", false);
        let btn = DlgButton::new("OK", DialogResult::Ok, dlg_pid);
        assert_eq!(btn.result(), &DialogResult::Ok);
        assert_eq!(btn.caption(), "OK");
    }

    #[test]
    fn dlg_button_set_caption_updates_emButton() {
        let mut tree = crate::emPanelTree::PanelTree::new();
        let dlg_pid = tree.create_root("dlg", false);
        let mut btn = DlgButton::new("OK", DialogResult::Ok, dlg_pid);
        btn.set_caption("Apply");
        assert_eq!(btn.caption(), "Apply");
    }
```

- [ ] **Step 3.3: Run new tests + full gate.**

```bash
cargo-nextest run -p emcore --lib emDialog::tests::dlg_button_carries_result_payload \
  emDialog::tests::dlg_button_set_caption_updates_emButton
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: `2479 + 2 = 2481` passed / `0 failed` / `9 skipped`.

**Potential gotcha:** `emButton::new(caption)` signature. If `emButton::new` requires `ctx: &mut impl ConstructCtx` (per Phase 3 B3.4b — every widget ctor takes a ctx for signal allocation), `DlgButton::new` must also take ctx and thread it through. Check `emButton.rs:45` — if `emButton::new(ctx: &mut C, caption: &str) -> Self`, update `DlgButton::new` accordingly:

```rust
pub(crate) fn new<C: crate::emEngineCtx::ConstructCtx>(
    ctx: &mut C,
    caption: &str,
    result: DialogResult,
    dlg_panel_id: crate::emPanelTree::PanelId,
) -> Self {
    Self {
        button: emButton::new(ctx, caption),
        result,
        dlg_panel_id,
    }
}
```

And update the test construction:

```rust
let mut __init = TestInit::new();
let btn = DlgButton::new(&mut __init.ctx(), "OK", DialogResult::Ok, dlg_pid);
```

- [ ] **Step 3.4: Commit Task 3.**

Update ledger:

```
- **Task 3 — DlgButton:** commit <SHA>. DlgButton PanelBehavior wrapping emButton
  + DialogResult payload. Input delegates to emButton; click observation is the
  engine's responsibility (Task 4 + Task 7 wire scheduler.connect for each
  button's click_signal). Two unit tests. Not yet installed. Gate green —
  nextest 2481/0/9.
```

```bash
git add crates/emcore/src/emDialog.rs docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md
git commit -m "phase-3.5 task 3: DlgButton PanelBehavior — port of C++ emDialog::DlgButton

Follows emColorFieldFieldPanel::ButtonPanel adapter precedent. Carries
DialogResult payload + parent DlgPanel PanelId. Click observation is wired
through scheduler.connect(button.click_signal, dialog_private_engine_id)
at installation time (Task 7) — matches C++ PrivateEngine.AddWakeUpSignal
pattern.

Gate green — nextest 2481/0/9."
```

**Task 3 exit condition:** `rg -n 'pub\(crate\) struct DlgButton' crates/emcore/src/emDialog.rs` → 1; nextest +2 over Task 2.

---

## Task 4: Add `DialogPrivateEngine` struct + `impl emEngine` (standalone)

**Files:**
- Modify: `crates/emcore/src/emDialog.rs`

**Scope:** Add the engine type. Register it with the scheduler, subscribe to signals, and drive DlgPanel state on Cycle. NOT yet consumed by `emDialog::new` (that's Task 5).

**Cycle body maps to C++ `emDialog::PrivateCycle()` (emDialog.cpp:194-206)**:
1. If close_signal is signalled: set `DlgPanel.pending_result = Some(DialogResult::Cancel)` (C++ NEGATIVE on close).
2. Observe `DlgPanel.pending_result`. If `Some(r)`:
   a. Run `on_check_finish(&r)` — if returns false, clear pending_result and return.
   b. Move r from `pending_result` to `finalized_result`.
   c. Fire `finish_signal`.
   d. Invoke `on_finish(&r, &mut sched)`.
   e. Invoke `on_finished(&r, &mut sched)` (Finished virtual per D1).
3. Observe button click_signals via `ctx.is_signaled(btn.click_signal)` for each button on DlgPanel.buttons. If any is signalled, write that button's result into `DlgPanel.pending_result`. (Next Cycle pass executes step 2.)
4. If `auto_delete` + `finalized_result.is_some()`: increment `finish_state`. At `finish_state == 3`, issue `framework_actions.push(DeferredAction::CloseWindow(window_id))`. C++ parity: three time slices after close signal.

Return `true` to stay awake if `auto_delete` countdown is mid-progress; else `false`.

- [ ] **Step 4.1: Add `DialogPrivateEngine` struct to `emDialog.rs`.**

Place after the `DlgButton` impl.

```rust
/// Engine driving an emDialog's finish lifecycle.
///
/// Port of C++ `emDialog::PrivateEngineClass : public emEngine`
/// (emDialog.h:203-210; emDialog.cpp:194-206 `PrivateCycle`).
/// Registered at `Priority::High` matching C++ `HIGH_PRIORITY`.
/// Subscribed via `scheduler.connect(...)` to:
///   - the owning emWindow's close_signal (equivalent of
///     `AddWakeUpSignal(GetCloseSignal())` at emDialog.cpp:38)
///   - each DlgButton's click_signal (wired at button-install time, Task 7)
pub(crate) struct DialogPrivateEngine {
    /// PanelId of the DlgPanel — the dialog's root panel. Used to reach
    /// DlgPanel state via `tree.take_behavior(root_panel_id)` during Cycle
    /// (Rust analog of C++ `PrivateEngineClass::Dlg&`).
    pub(crate) root_panel_id: crate::emPanelTree::PanelId,
    /// WindowId of the dialog's emWindow. Used to enqueue
    /// DeferredAction::CloseWindow for auto-delete.
    pub(crate) window_id: Option<winit::window::WindowId>,
    /// Close-signal cached for IsSignaled probe inside Cycle. C++ uses
    /// `PrivateEngine.IsSignaled(GetCloseSignal())` at emDialog.cpp:196;
    /// Rust caches the id at construction.
    pub(crate) close_signal: SignalId,
}

impl DialogPrivateEngine {
    pub(crate) fn new(
        root_panel_id: crate::emPanelTree::PanelId,
        close_signal: SignalId,
    ) -> Self {
        Self {
            root_panel_id,
            window_id: None,
            close_signal,
        }
    }

    /// Register this engine with the scheduler at `Priority::High`, connect
    /// to close_signal, and return the allocated `EngineId`. Matches C++:
    ///   PrivateEngine.SetEnginePriority(HIGH_PRIORITY);
    ///   PrivateEngine.AddWakeUpSignal(GetCloseSignal());
    pub(crate) fn install(
        self,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        tree_location: crate::emEngine::TreeLocation,
    ) -> crate::emEngine::EngineId {
        let close_sig = self.close_signal;
        let eid = scheduler.register_engine(
            Box::new(self),
            crate::emEngine::Priority::High,
            tree_location,
        );
        scheduler.connect(close_sig, eid);
        eid
    }
}

impl crate::emEngine::emEngine for DialogPrivateEngine {
    fn Cycle(&mut self, ctx: &mut crate::emEngineCtx::EngineCtx<'_>) -> bool {
        // Take the DlgPanel behavior off the tree — C++ analog of
        // `emDialog& Dlg;` back-reference, but borrow-safe.
        let Some(mut behavior) = ctx.tree.take_behavior(self.root_panel_id) else {
            // Panel removed — dialog is being torn down. Engine will be
            // deregistered externally (deregister() helper); stay asleep.
            return false;
        };

        // Coerce to DlgPanel. If it's not a DlgPanel, something is deeply
        // wrong (engine was registered against a non-dialog panel).
        //
        // Access via AsAny downcast is not allowed (CLAUDE.md Do-NOT lists Any).
        // Add a PanelBehavior trait method `fn as_dlg_panel_mut(&mut self) -> Option<&mut DlgPanel>`
        // defaulting to None, and override it in DlgPanel. Pattern matches the
        // existing `fn as_sub_view_panel_mut` at emPanel.rs:371.

        let stay_awake = {
            let Some(dlg_panel) = behavior.as_dlg_panel_mut() else {
                panic!("DialogPrivateEngine::Cycle: root_panel behavior is not DlgPanel");
            };

            // Step 1: observe close_signal
            if ctx.scheduler.is_signaled_for_engine(self.close_signal, ctx.engine_id)
                && dlg_panel.pending_result.is_none()
                && dlg_panel.finalized_result.is_none()
            {
                dlg_panel.pending_result = Some(DialogResult::Cancel);
            }

            // Step 2: observe button click_signals.
            // Iterate DlgPanel.buttons and for each, check if its corresponding
            // DlgButton child has click_signal pending. For this we need each
            // button's click_signal cached alongside its result in DlgPanel.buttons.
            //
            // Rework: DlgPanel.buttons is `Vec<(String, DialogResult)>`. To
            // observe, we need the signal too. Extend the buttons vec to
            // `Vec<(String, DialogResult, Option<SignalId>)>` where the signal
            // is the installed DlgButton's click_signal, Some after Task 7
            // wires it. Or a parallel vec `button_click_signals: Vec<SignalId>`.
            //
            // For this task (DialogPrivateEngine in isolation), the engine
            // iterates no buttons — the field exists, Task 7 populates it.
            let button_signals: Vec<(SignalId, DialogResult)> = dlg_panel
                .buttons
                .iter()
                .filter_map(|(_, r)| {
                    // Until Task 7, buttons carry no signal. Once Task 7 adds
                    // a parallel `button_signals` vec, zip here.
                    None::<(SignalId, DialogResult)>.map(|(s, _)| (s, r.clone()))
                })
                .collect();
            for (sig, result) in button_signals {
                if ctx.scheduler.is_signaled_for_engine(sig, ctx.engine_id) {
                    dlg_panel.pending_result = Some(result);
                    break;
                }
            }

            // Step 3: resolve pending_result → finalized + fire + callbacks
            if let Some(result) = dlg_panel.pending_result.clone() {
                // CheckFinish veto
                let veto = dlg_panel
                    .on_check_finish
                    .as_mut()
                    .is_some_and(|cb| !cb(&result));
                if veto {
                    dlg_panel.pending_result = None;
                } else {
                    dlg_panel.pending_result = None;
                    dlg_panel.finalized_result = Some(result.clone());
                    ctx.scheduler.fire(dlg_panel.finish_signal);
                    // Build a SchedCtx to invoke callbacks.
                    let sched_fa: *mut Vec<_> = &mut *ctx.framework_actions;
                    let root_ctx = ctx.root_context;
                    let sched_fw_cb = ctx.framework_clipboard;
                    let sched_sched: *mut _ = &mut *ctx.scheduler;
                    // SAFETY: aliased borrow within single-threaded re-entry —
                    // mirrors emPanelCycleEngine::Cycle aliasing pattern.
                    let mut sched_ctx = crate::emEngineCtx::SchedCtx {
                        scheduler: unsafe { &mut *sched_sched },
                        framework_actions: unsafe { &mut *sched_fa },
                        root_context: root_ctx,
                        framework_clipboard: sched_fw_cb,
                    };
                    if let Some(cb) = dlg_panel.on_finish.as_mut() {
                        cb(&result, &mut sched_ctx);
                    }
                    if let Some(cb) = dlg_panel.on_finished.as_mut() {
                        cb(&result, &mut sched_ctx);
                    }
                }
            }

            // Step 4: auto-delete countdown
            if dlg_panel.auto_delete && dlg_panel.finalized_result.is_some() {
                dlg_panel.finish_state = dlg_panel.finish_state.saturating_add(1);
                if dlg_panel.finish_state >= 3 {
                    if let Some(wid) = self.window_id {
                        ctx.framework_actions
                            .push(crate::emEngineCtx::DeferredAction::CloseWindow(wid));
                    }
                    false // engine can sleep; window-close torches everything
                } else {
                    true // stay awake for next slice countdown
                }
            } else {
                false
            }
        };

        ctx.tree.put_behavior(self.root_panel_id, behavior);
        stay_awake
    }
}
```

**Important:** the `as_dlg_panel_mut` trait method does not exist yet. Add it in the same commit:

- Open `crates/emcore/src/emPanel.rs`, navigate to `pub trait PanelBehavior` (around line 196), and after `fn as_sub_view_panel_mut` (around line 371), add:

```rust
    /// Downcast to `DlgPanel` without `Any`. Used by `DialogPrivateEngine`
    /// to reach the dialog's state during Cycle (B3.5e). Pattern matches
    /// `as_sub_view_panel_mut`.
    ///
    /// The default returns `None`; only `emDialog::DlgPanel` overrides.
    fn as_dlg_panel_mut(&mut self) -> Option<&mut crate::emDialog::DlgPanel> {
        None
    }
```

- Back in `emDialog.rs`, inside `impl PanelBehavior for DlgPanel`, override:

```rust
    fn as_dlg_panel_mut(&mut self) -> Option<&mut DlgPanel> {
        Some(self)
    }
```

- [ ] **Step 4.2: Add unit tests for DialogPrivateEngine.**

Use the existing `TestInit` helper at the bottom of `emDialog.rs`. Add:

```rust
    #[test]
    fn private_engine_observes_close_signal_sets_pending_cancel() {
        use crate::emPanel::PanelBehavior;
        use crate::emPanelTree::PanelTree;

        let mut __init = TestInit::new();
        let mut tree = PanelTree::new();
        let root = tree.create_root("dlg", true);

        let finish_sig = __init.sched.create_signal();
        let close_sig = __init.sched.create_signal();
        let dlg_panel = DlgPanel::new("T", emLook::new(), finish_sig);
        tree.set_behavior(root, Box::new(dlg_panel));
        tree.init_panel_view(root, Some(&mut __init.sched));

        let eng = DialogPrivateEngine::new(root, close_sig);
        let eid = eng.install(&mut __init.sched, crate::emEngine::TreeLocation::Outer);

        // Fire close signal, run one time slice — engine observes and sets
        // pending_result. Because pending_result is set inside the same Cycle,
        // step 3 (finalize) will also run — so after the slice, finalized_result
        // should be Some(Cancel).
        __init.sched.fire(close_sig);
        let mut pending_inputs = Vec::new();
        let mut input_state = crate::emInputState::emInputState::new();
        let fw_cb: std::cell::RefCell<Option<Box<dyn crate::emClipboard::emClipboard>>> =
            std::cell::RefCell::new(None);
        let mut windows = std::collections::HashMap::new();
        __init.sched.DoTimeSlice(
            &mut tree,
            &mut windows,
            &__init.root,
            &mut __init.fw,
            &mut pending_inputs,
            &mut input_state,
            &fw_cb,
            std::time::Instant::now() + std::time::Duration::from_millis(50),
        );

        // The DlgPanel behavior has been put back. Read its finalized_result.
        let behavior = tree.take_behavior(root).expect("DlgPanel present");
        let dp = behavior
            .as_dlg_panel_mut_const()
            .unwrap_or_else(|| panic!("behavior is not DlgPanel"));
        // NOTE: as_dlg_panel_mut_const does not exist. The test must take + mutate.
        // Simpler: downcast via the mut helper, check, put back.
        drop(dp); // placeholder — simplify:
        // Replace the above with: behavior, then as_dlg_panel_mut, immutable read:
        let mut behavior = behavior;
        let dlg = behavior.as_dlg_panel_mut().expect("DlgPanel");
        assert_eq!(dlg.finalized_result, Some(DialogResult::Cancel));
        assert!(__init.sched.is_pending(finish_sig));
        drop(behavior);

        // Cleanup — put back a throwaway to let tree drop cleanly.
        let placeholder = DlgPanel::new("_", emLook::new(), finish_sig);
        tree.set_behavior(root, Box::new(placeholder));
    }
```

Note: the `DoTimeSlice` signature — confirm exact parameter order from the current source. If parameters differ (this plan was written against a snapshot), adjust to the actual signature found via `rg 'pub fn DoTimeSlice' crates/emcore/src/emScheduler.rs`.

Also: remove the `as_dlg_panel_mut_const` dead-code block I left as an intermediate — the simpler shape is what matters (take, downcast, assert, put back).

- [ ] **Step 4.3: Run new test + gate.**

```bash
cargo-nextest run -p emcore --lib emDialog::tests::private_engine_observes_close_signal_sets_pending_cancel
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: `2481 + 1 = 2482` passed / `0 failed` / `9 skipped`.

- [ ] **Step 4.4: Commit Task 4.**

```
- **Task 4 — DialogPrivateEngine:** commit <SHA>. Engine struct + impl emEngine.
  install() helper registers at Priority::High and connects close_signal
  (AddWakeUpSignal port). Cycle body observes close_signal + buttons (button
  signals vec added in Task 7) + pending_result → on_check_finish veto →
  finalized_result + fire finish_signal + on_finish + on_finished + auto-delete
  countdown. Added `as_dlg_panel_mut` trait method on PanelBehavior (pattern
  mirrors as_sub_view_panel_mut). One integration test covering close→cancel
  path. Not yet installed. Gate green — nextest 2482/0/9.
```

```bash
git add crates/emcore/src/emDialog.rs crates/emcore/src/emPanel.rs docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md
git commit -m "phase-3.5 task 4: DialogPrivateEngine — port of C++ emDialog::PrivateEngineClass

Engine registered at Priority::High (matches C++ HIGH_PRIORITY).
install() helper connects close_signal (AddWakeUpSignal port per D2:
single engine type, not per-dialog-type).

Cycle body ports emDialog.cpp:194-206 PrivateCycle: observe close_signal
sets pending_result to Cancel; observe pending_result runs on_check_finish
veto; finalize + fire finish_signal + invoke on_finish + on_finished;
auto-delete countdown enqueues DeferredAction::CloseWindow at 3 slices.

Button click_signals are iterated but the signal vec is empty until Task 7
populates it.

Gate green — nextest 2482/0/9."
```

**Task 4 exit condition:** `rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs` → 1 match. `rg -n 'fn as_dlg_panel_mut' crates/emcore/src/` → ≥2 matches (trait default + override).

---

## Task 5: Reshape `emDialog` struct — façade over emWindow + DlgPanel + DialogPrivateEngine

**Files:**
- Modify: `crates/emcore/src/emDialog.rs` — delete the plain-struct fields (`border`, `buttons`, `result`, `finish_signal` on emDialog), replace with `window_id`, `root_panel_id`, `private_engine_id`, `caption` (façade) and route public methods through `tree.take_behavior(root_panel_id)` to reach `DlgPanel`'s state. Delete `pub fn Cycle(&mut self, ctx: &mut PanelCtx) -> bool`. Delete `silent_cancel()`.

**Scope:** This is the keystone structural task. The `emDialog` struct goes from ~200 LOC of plain-struct code to a ~400 LOC façade. All accessor/mutator public methods change signature to take `(&mut self, tree: &mut PanelTree, ...)` or similar so they can route through the tree.

**Constructor signature:**

```rust
impl emDialog {
    pub fn new(
        parent_context: std::rc::Rc<crate::emContext::emContext>,
        title: &str,
        look: std::rc::Rc<emLook>,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        framework_actions: &mut Vec<crate::emEngineCtx::DeferredAction>,
        tree_location: crate::emEngine::TreeLocation,
    ) -> Self { ... }
}
```

This is a **caller-API breaking change**. Every call site updates in Task 10.

- [ ] **Step 5.1: Write the target `emDialog::new` body.**

Replace the existing `impl emDialog { pub fn new(...) { ... } ... }` block entirely. The new struct + ctor:

```rust
/// Modal dialog as an emWindow-rooted view with a DlgPanel root.
///
/// Port of C++ `class emDialog : public emWindow` (emDialog.h:37).
/// Owns a `winit::WindowId` into `App::windows` (not the emWindow directly),
/// the `PanelId` of its DlgPanel root, and the `EngineId` of its
/// DialogPrivateEngine. All mutations route through the tree to the DlgPanel
/// behavior-side state.
pub struct emDialog {
    /// WindowId into App::windows. `None` until the pending emWindow is
    /// materialized by the framework's about_to_wait drain (typically one
    /// tick after construction). Operations that require an OS surface
    /// (set geometry, etc.) guard on this.
    window_id: Option<winit::window::WindowId>,
    root_panel_id: crate::emPanelTree::PanelId,
    private_engine_id: crate::emEngine::EngineId,
    /// Signal fired by DialogPrivateEngine when the dialog finishes.
    /// Alias for `DlgPanel.finish_signal` cached on the façade for
    /// `GetFinishSignal()` calls without a tree borrow.
    finish_signal: SignalId,
    close_signal: SignalId,
    /// Parent context — retained for future child-popup parenting.
    #[allow(dead_code)]
    parent_context: std::rc::Rc<crate::emContext::emContext>,
}

impl emDialog {
    /// Construct an emDialog. Creates the dialog's emWindow in Pending state
    /// (winit surface materialized on the next framework tick), installs
    /// DlgPanel as its root, registers DialogPrivateEngine at Priority::High
    /// subscribed to close_signal.
    ///
    /// Port of C++ `emDialog::emDialog(emContext& parent, ViewFlags, WindowFlags, wmResName)`
    /// (emDialog.cpp:25-39). Default C++ flags: `VF_POPUP_ZOOM | VF_ROOT_SAME_TALLNESS`,
    /// `WF_MODAL`. Rust ports the same defaults.
    pub fn new(
        parent_context: std::rc::Rc<crate::emContext::emContext>,
        title: &str,
        look: std::rc::Rc<emLook>,
        scheduler: &mut crate::emScheduler::EngineScheduler,
    ) -> Self {
        // Allocate signals for the emWindow.
        let close_signal = scheduler.create_signal();
        let flags_signal = scheduler.create_signal();
        let focus_signal = scheduler.create_signal();
        let geometry_signal = scheduler.create_signal();
        let finish_signal = scheduler.create_signal();

        // Create the dialog's panel tree root. Per new_popup_pending's contract,
        // callers pass a pre-existing root PanelId; we build a fresh PanelTree
        // for this dialog's sub-view... wait — that's the question:
        // does emWindow create its own PanelTree or use a caller-supplied one?
        //
        // See emWindow::new_popup_pending at emWindow.rs:320: it takes a root_panel
        // PanelId but the PanelTree lives... actually inspect: emView owns the
        // PanelTree? Search the code to determine.
        //
        // *** IMPLEMENTOR: before writing the ctor body, run:
        //   rg -nU 'struct emView[\s\S]*?\}' crates/emcore/src/emView.rs | head -100
        // to confirm where the PanelTree lives for a popup window's view.
        //
        // Placeholder shape (subject to confirmation during implementation):
        // create a PanelTree, create root, install DlgPanel, pass root PanelId
        // to new_popup_pending.

        let mut tree = crate::emPanelTree::PanelTree::new();
        let root_panel_id = tree.create_root("dialog_root", true);
        let dlg_panel = DlgPanel::new(title, look.clone(), finish_signal);
        tree.set_behavior(root_panel_id, Box::new(dlg_panel));
        tree.init_panel_view(root_panel_id, Some(scheduler));

        // Register DialogPrivateEngine.
        let eng = DialogPrivateEngine::new(root_panel_id, close_signal);
        let private_engine_id = eng.install(
            scheduler,
            crate::emEngine::TreeLocation::Outer, // dialogs are top-level views
        );

        // TODO (in Step 5.2): construct the emWindow via emWindow::new_popup_pending.
        // This requires handing the PanelTree to the emWindow for ownership —
        // see Step 5.2 for the actual wiring, which may require threading an
        // App::pending_popup_windows registration through framework_actions.

        Self {
            window_id: None, // set post-materialization via wire_window_id
            root_panel_id,
            private_engine_id,
            finish_signal,
            close_signal,
            parent_context,
        }
    }
}
```

Note: I've left TODOs and audit prompts in the body. Before writing the real Step 5.2, confirm:

```bash
rg -nU 'pub struct emView[\s\S]*?\{[\s\S]{1,2000}' crates/emcore/src/emView.rs | head -80
```

to find where the PanelTree lives for an emWindow's view. If `emView` doesn't own the PanelTree (it's passed in at each method call), the emWindow stores a pointer to a tree that lives externally — then the dialog's PanelTree must be installed into the framework's tree registry. This is a design piece that must be settled before Step 5.2 can be completed.

**STOP and investigate.** Given the uncertainty, the safer path is to insert a micro-task 5a here:

- [ ] **Step 5.1a: Confirm emWindow / tree / App ownership model (HIGH RISK — likely surfaces a gap).**

Already-known facts from the plan-author's investigation:

- There is **one** `App::tree: PanelTree` (emGUIFramework.rs:93) shared by the whole framework. `PanelTree::create_root` asserts there is only one root.
- `App::windows: HashMap<WindowId, emWindow>` holds **top-level** windows only (emGUIFramework.rs:100). Popup windows live on `emView::PopupWindow` (owned by the launching view, C++-parity per emView.h:670).
- `emView::PopupWindow` is `Option<emWindow>` — **single slot**. A view can host at most one popup at a time.
- Top-level window creation happens via `App::run` + setup callback — currently the one home window at startup. There is no existing path for adding top-level windows at runtime.
- Popup window creation happens via `emView::RawVisitAbs` during popup-zoom; the popup is constructed via `emWindow::new_popup_pending` and stored in `emView::PopupWindow`.

**The three possible paths for emDialog's emWindow, and their costs:**

| Path | Store where? | Cost | Works for Phase 3.5? |
|---|---|---|---|
| (i) Dialog-as-popup on launching emView | `emView::PopupWindow` | Reuses existing popup materialization. BUT: single-slot — collides with popup-zoom AND breaks dialog-on-dialog stacking. Each dialog also needs to create a new **root_panel**, which conflicts with the single-root assertion on App::tree. | **NO** — single-slot popup + single-root tree are hard blockers. |
| (ii) Dialog-as-top-level in `App::windows` | `App::windows` | Requires a runtime-add-top-level-window path. DeferredAction-based enqueue is the natural mechanism (see `App::pending_actions: Rc<RefCell<Vec<DeferredAction>>>` at emGUIFramework.rs:122 — closures that get `&mut App` + `&ActiveEventLoop`). Each dialog also needs its own PanelTree — requires lifting the one-tree-per-App assumption OR reusing App::tree with the dialog's root panel as a child (not a root) of the existing tree. | **YES but requires prereq sub-phase 3.5.A.** |
| (iii) Dialog panels in App::tree as a non-root subtree + no emWindow | Reuse existing tree; no new window | Violates D4 (dialog must own its own emWindow). | NO. |

**Decision required at Step 5.1a time:**

Given that (i) and (iii) are blocked, (ii) is the only path — and it forces a prereq sub-phase. The right split is:

**Phase 3.5.A — runtime top-level emWindow install path.** One sub-phase that extends `App::windows` + `App::pending_actions` with a clean API for "queue a new top-level emWindow for materialization on the next event-loop tick" AND lifts the single-tree assumption (either: each dialog gets its own PanelTree stored on the App, OR: dialogs install under App::tree as subtrees with a non-asserting `create_nonroot_subroot` helper). Then resume Phase 3.5 Task 5.2.

Run the following to triple-check before splitting:

```bash
# Verify there is exactly one tree:
rg -n 'App::tree\|self.tree:\|pub tree: PanelTree\|trees:' crates/emcore/src/emGUIFramework.rs | head -5
# Verify PopupWindow is a single-slot Option:
rg -nU 'PopupWindow:\s*Option' crates/emcore/src/emView.rs | head -3
# Verify runtime-add-top-level path presence:
rg -nU 'fn add_window|fn install_window|windows\.insert' crates/emcore/src/emGUIFramework.rs | head -5
```

If the last grep returns only places where windows.insert runs at startup (not as a runtime-add API): **3.5.A is required.** Document findings in the ledger, commit what you have so far in 3.5, and switch to writing + executing 3.5.A first.

If the first two greps reveal the facts are different from what's documented above (e.g., multiple trees exist, or PopupWindow is a Vec): the plan may be recoverable inside Task 5 without a 3.5.A split. Update the ledger with the reality, pick the appropriate path, and proceed.

- [ ] **Step 5.1b: If a prereq sub-phase is needed, pause and write it.**

Only applies if Step 5.1a surfaces a gap. Write a new sub-phase plan document at `docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-5-a-dynamic-window-tree.md` using the brainstorming+writing-plans flow. Implement that sub-phase, merge, tag, then resume this plan at Step 5.2.

*If no gap:* skip this step and proceed.

- [ ] **Step 5.2: Finalize `emDialog::new` ctor.**

With the tree-ownership question settled, write the final ctor body. The shape below is a best-guess against the code I've surveyed; adjust to match the confirmed API:

```rust
    pub fn new(
        parent_context: std::rc::Rc<crate::emContext::emContext>,
        title: &str,
        look: std::rc::Rc<emLook>,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        pending_windows: &mut Vec<crate::emGUIFramework::PendingWindow>, // or equivalent
    ) -> Self {
        let close_signal = scheduler.create_signal();
        let flags_signal = scheduler.create_signal();
        let focus_signal = scheduler.create_signal();
        let geometry_signal = scheduler.create_signal();
        let finish_signal = scheduler.create_signal();

        let mut tree = crate::emPanelTree::PanelTree::new();
        let root_panel_id = tree.create_root("dialog_root", true);
        let dlg_panel = DlgPanel::new(title, look.clone(), finish_signal);
        tree.set_behavior(root_panel_id, Box::new(dlg_panel));
        tree.init_panel_view(root_panel_id, Some(scheduler));

        let eng = DialogPrivateEngine::new(root_panel_id, close_signal);
        let private_engine_id = eng.install(
            scheduler,
            crate::emEngine::TreeLocation::Outer,
        );

        let window = crate::emWindow::emWindow::new_popup_pending(
            std::rc::Rc::clone(&parent_context),
            root_panel_id,
            crate::emWindow::WindowFlags::MODAL,
            title.to_string(),
            close_signal,
            flags_signal,
            focus_signal,
            geometry_signal,
            look.background_color(),
        );

        // Enqueue the pending window for framework materialization.
        // Exact API TBD by Step 5.1a — adjust:
        pending_windows.push(crate::emGUIFramework::PendingWindow { window, tree });

        Self {
            window_id: None,
            root_panel_id,
            private_engine_id,
            finish_signal,
            close_signal,
            parent_context,
        }
    }
```

**Implementor's note:** `look.background_color()` is a placeholder — confirm emLook's API. If there's no method, use `emColor::from_rgba(0, 0, 0, 255)` (black default matching emDialog's background).

- [ ] **Step 5.3: Port `emDialog::Finish` as a method routing through the tree.**

The Rust method no longer fires finish_signal directly — it sets `DlgPanel.pending_result` and wakes the engine. Matches C++ `emDialog::Finish(int result)` at `emDialog.cpp` (calls `PrivateEngine.WakeUp()`).

```rust
impl emDialog {
    /// Request the dialog to finish with `result`. Actual finish runs on the
    /// next DialogPrivateEngine Cycle (fires finish_signal, invokes callbacks).
    ///
    /// Port of C++ `emDialog::Finish(int)` at emDialog.cpp:145+.
    pub fn Finish(
        &mut self,
        result: DialogResult,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
    ) {
        let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
            return;
        };
        if let Some(dlg) = behavior.as_dlg_panel_mut() {
            dlg.pending_result = Some(result);
        }
        tree.put_behavior(self.root_panel_id, behavior);
        scheduler.wake_up(self.private_engine_id);
    }

    pub fn GetResult(&self, tree: &crate::emPanelTree::PanelTree) -> Option<DialogResult> {
        // Take-peek-put is mutate-only; we want an immutable peek. Use the
        // tree's read-only behavior accessor (add one if missing in emPanelTree.rs):
        //   `pub fn behavior_ref(&self, id: PanelId) -> Option<&dyn PanelBehavior>`
        // For now, use a take/put pattern — but that requires &mut tree, which
        // the signature forbids. Alternative: cache finalized_result on the
        // façade when finish_signal fires.
        //
        // Simplest shape: `GetResult` takes `&mut tree`. Updates all callers
        // (Task 10). Matches how `Finish` already requires &mut.
        None // placeholder — see below
    }
}
```

Given the &mut-tree constraint, `GetResult` also takes `&mut tree`:

```rust
    pub fn GetResult(
        &self,
        tree: &mut crate::emPanelTree::PanelTree,
    ) -> Option<DialogResult> {
        let mut behavior = tree.take_behavior(self.root_panel_id)?;
        let result = behavior
            .as_dlg_panel_mut()
            .and_then(|dlg| dlg.finalized_result.clone());
        tree.put_behavior(self.root_panel_id, behavior);
        result
    }
```

- [ ] **Step 5.4: Port the remaining emDialog public API through the tree.**

Each of these previously-plain-struct methods now routes through `tree.take_behavior(root_panel_id) → DlgPanel → put_behavior`:

- `AddCustomButton(&mut self, tree, caption, result)` — pushes to `DlgPanel.buttons` and creates a DlgButton child panel under the buttons_panel (created by Step 5.5). Also wires `scheduler.connect(dlg_button.click_signal, self.private_engine_id)`.
- `AddPositiveButton(&mut self, tree, scheduler, caption)` → AddCustomButton with `DialogResult::Ok`.
- `AddNegativeButton(&mut self, tree, scheduler, caption)` → `DialogResult::Cancel`.
- `AddOKButton`/`AddCancelButton`/`AddOKCancelButtons` — thin wrappers.
- `GetButton(&self, tree, index) -> Option<(String, DialogResult)>` — read from `DlgPanel.buttons`.
- `GetButtonForResult(&self, tree, result)` — find in `DlgPanel.buttons`.
- `GetOKButton`/`GetCancelButton` — thin.
- `SetRootTitle(&mut self, tree, title)` — `DlgPanel.border.SetCaption(title)`.
- `EnableAutoDeletion(&mut self, tree, enabled)` — `DlgPanel.auto_delete = enabled`.
- `IsAutoDeletionEnabled(&self, tree) -> bool`.
- `GetFinishSignal(&self) -> SignalId` — direct return `self.finish_signal`.
- `CheckFinish(&self, tree) -> bool` — true iff `DlgPanel.finalized_result.is_some()`.
- `set_button_label_for_result(&mut self, tree, result, label)` — mutate DlgPanel.buttons + propagate caption to the installed DlgButton via tree.
- `private_engine_id(&self) -> EngineId` — return `self.private_engine_id`. `pub(crate)` visibility. Consumed by Phase 3.6 (emFileDialog calls `scheduler.connect(fsb.file_trigger_signal, dialog.private_engine_id())`).
- `root_panel_id(&self) -> PanelId` — return `self.root_panel_id`. `pub(crate)` visibility. Consumed by Phase 3.6 (emFileDialog accesses outer DlgPanel via `tree.take_behavior(dialog.root_panel_id())`).

Each method follows the template:

```rust
    pub fn METHOD_NAME(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        /* other args */
    ) /* -> ReturnType */ {
        let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
            return /* default */;
        };
        let retval = behavior
            .as_dlg_panel_mut()
            .map(|dlg| {
                // mutate dlg.FIELD
                /* return value */
            })
            .unwrap_or(/* default */);
        tree.put_behavior(self.root_panel_id, behavior);
        retval
    }
```

Implement all listed methods. Run `cargo check` after each small group to catch signature issues early.

- [ ] **Step 5.5: Port `emDialog::AddCustomButton` with DlgButton installation.**

Most complex of Task 5. Full impl:

```rust
    pub fn AddCustomButton(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        framework_actions: &mut Vec<crate::emEngineCtx::DeferredAction>,
        root_context: &std::rc::Rc<crate::emContext::emContext>,
        caption: &str,
        result: DialogResult,
    ) {
        // 1. Ensure buttons_panel exists (lazy-install on first button).
        let buttons_panel_id = self.ensure_buttons_panel(tree, scheduler);

        // 2. Create DlgButton child panel under buttons_panel.
        let button_child = tree.create_child(
            buttons_panel_id,
            &format!("btn_{}", /* next index */ self.button_count(tree)),
            Some(scheduler),
        );
        let mut init = crate::emEngineCtx::InitCtx {
            scheduler,
            framework_actions,
            root_context,
        };
        let dlg_button = DlgButton::new(&mut init, caption, result.clone(), self.root_panel_id);
        let click_sig = dlg_button.button.click_signal;
        tree.set_behavior(button_child, Box::new(dlg_button));

        // 3. Wire button's click_signal to private engine.
        scheduler.connect(click_sig, self.private_engine_id);

        // 4. Update DlgPanel.buttons + button_signals vec.
        let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
            return;
        };
        if let Some(dlg) = behavior.as_dlg_panel_mut() {
            dlg.buttons.push((caption.to_string(), result));
            dlg.button_signals.push((click_sig, dlg.buttons.last().unwrap().1.clone()));
        }
        tree.put_behavior(self.root_panel_id, behavior);
    }
```

Extend `DlgPanel` (from Task 2) to have a `button_signals: Vec<(SignalId, DialogResult)>` parallel vec. Update Task 4's Cycle body to use this vec instead of the placeholder `None::<...>` filter. Back-edit both earlier task outputs:

```rust
// in DlgPanel struct (Task 2 patch):
pub(crate) button_signals: Vec<(SignalId, DialogResult)>,
// in DlgPanel::new:
button_signals: Vec::new(),
```

```rust
// in DialogPrivateEngine::Cycle (Task 4 patch — fix):
let button_signals: Vec<(SignalId, DialogResult)> = dlg_panel
    .button_signals
    .clone();
```

(Clone needed because the immutable iteration happens before we mutate pending_result in the loop body.)

- [ ] **Step 5.6: Add `ensure_buttons_panel` and `ensure_content_panel` helpers.**

```rust
impl emDialog {
    fn ensure_buttons_panel(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
    ) -> crate::emPanelTree::PanelId {
        // Check DlgPanel.buttons_panel_id first.
        let existing = {
            let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
                panic!("emDialog root missing");
            };
            let id = behavior
                .as_dlg_panel_mut()
                .and_then(|dlg| dlg.buttons_panel_id);
            tree.put_behavior(self.root_panel_id, behavior);
            id
        };
        if let Some(id) = existing {
            return id;
        }

        // Create emLinearLayout horizontal child panel under root.
        let btn_panel_id = tree.create_child(self.root_panel_id, "buttons", Some(scheduler));
        let layout = crate::emLinearLayout::emLinearLayout::horizontal()
            .with_spacing(crate::emTiling::Spacing::default());
        tree.set_behavior(btn_panel_id, Box::new(layout));

        // Record on DlgPanel.
        let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
            panic!("emDialog root vanished mid-ctor");
        };
        if let Some(dlg) = behavior.as_dlg_panel_mut() {
            dlg.buttons_panel_id = Some(btn_panel_id);
        }
        tree.put_behavior(self.root_panel_id, behavior);
        btn_panel_id
    }

    pub fn GetContentPanel(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
    ) -> crate::emPanelTree::PanelId {
        let existing = {
            let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
                panic!("emDialog root missing");
            };
            let id = behavior
                .as_dlg_panel_mut()
                .and_then(|dlg| dlg.content_panel_id);
            tree.put_behavior(self.root_panel_id, behavior);
            id
        };
        if let Some(id) = existing {
            return id;
        }

        let content_id = tree.create_child(self.root_panel_id, "content", Some(scheduler));
        let layout = crate::emLinearLayout::emLinearLayout::vertical();
        tree.set_behavior(content_id, Box::new(layout));

        let Some(mut behavior) = tree.take_behavior(self.root_panel_id) else {
            panic!("emDialog root vanished");
        };
        if let Some(dlg) = behavior.as_dlg_panel_mut() {
            dlg.content_panel_id = Some(content_id);
        }
        tree.put_behavior(self.root_panel_id, behavior);
        content_id
    }
}
```

- [ ] **Step 5.7: Add `emDialog::deregister` (D3 primary path).**

```rust
impl emDialog {
    /// Tear down the dialog's scheduler registrations and panel tree.
    /// MUST be called before dropping emDialog — Drop cannot reach the
    /// scheduler without violating CLAUDE.md Do-NOT (no Rc<Scheduler>,
    /// no captured raw pointer). D3.
    ///
    /// Port of the part of C++ `emDialog::~emDialog` (emDialog.cpp:43-46)
    /// that tears down PrivateEngine and DlgPanel.
    pub fn deregister(
        &mut self,
        tree: &mut crate::emPanelTree::PanelTree,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        framework_actions: &mut Vec<crate::emEngineCtx::DeferredAction>,
    ) {
        // Remove panel subtree (DlgPanel + content_panel + buttons_panel +
        // all DlgButton children). emPanelTree::remove is recursive.
        tree.remove(self.root_panel_id, Some(scheduler));

        // Deregister DialogPrivateEngine. emPanelTree::remove already removed
        // the per-panel PanelCycleEngines; DialogPrivateEngine is a
        // standalone registration, separately removed here.
        scheduler.remove_engine(self.private_engine_id);

        // Close the emWindow (if materialized).
        if let Some(wid) = self.window_id {
            framework_actions.push(crate::emEngineCtx::DeferredAction::CloseWindow(wid));
        }
    }

    /// Post-materialization hook — framework calls this once the pending
    /// emWindow's winit surface is ready. Populates `window_id` so that
    /// `deregister`'s DeferredAction::CloseWindow can target the right window.
    pub fn wire_window_id(&mut self, wid: winit::window::WindowId) {
        self.window_id = Some(wid);
    }
}
```

- [ ] **Step 5.8: Port `emDialog::ShowMessage` to the new shape.**

Replace the existing `ShowMessage` with:

```rust
impl emDialog {
    /// Static convenience: create a message dialog with a single OK button.
    /// Port of C++ `emDialog::ShowMessage(emContext&, const emString& title,
    /// const emString& message)` (emDialog.cpp:280+).
    pub fn ShowMessage(
        parent_context: std::rc::Rc<crate::emContext::emContext>,
        title: &str,
        message: &str,
        look: std::rc::Rc<emLook>,
        scheduler: &mut crate::emScheduler::EngineScheduler,
        tree_registry: &mut crate::emGUIFramework::PendingWindowQueue, // or equivalent
        framework_actions: &mut Vec<crate::emEngineCtx::DeferredAction>,
        root_context: &std::rc::Rc<crate::emContext::emContext>,
    ) -> Self {
        let mut dlg = Self::new(parent_context, title, look, scheduler, tree_registry);
        // TODO: install a Label panel with `message` as the content.
        dlg.AddOKButton(/* tree, scheduler, framework_actions, root_context */);
        // ShowMessage in C++ also calls `EnableAutoDeletion` so the dialog
        // removes itself post-close.
        // dlg.EnableAutoDeletion(tree, true);
        dlg
    }
}
```

ShowMessage's exact signature depends on the tree-registry API confirmed in Step 5.1a. This is mostly a convenience wrapper so precise plumbing settles during implementation.

- [ ] **Step 5.9: Delete the old Cycle method + silent_cancel + old plain-struct fields.**

- Delete `pub fn Cycle(&mut self, ctx: &mut PanelCtx<'_>) -> bool` from the pre-port `impl emDialog` (currently at `emDialog.rs:87-100` per the Task-6 version).

  Verify:
  ```bash
  rg -n 'pub fn Cycle' crates/emcore/src/emDialog.rs
  ```
  Expected after deletion: 0 matches.

- Delete `pub fn silent_cancel(&mut self)` from the pre-port emDialog. Verify:
  ```bash
  rg -n 'silent_cancel' crates/emcore/src/emDialog.rs
  ```
  Expected: 0 matches. (emStocksListBox still has references — those are migrated in Task 10.)

- Delete the pre-port emDialog fields (`border`, `buttons: Vec<...>`, `result`, `on_finish`, `on_check_finish`, `auto_delete`, `finish_signal` as-a-field) — replaced by the new façade fields.

- [ ] **Step 5.10: Update `emDialog` unit tests.**

Existing tests at `emDialog.rs:249-525` assume the plain-struct shape (`dlg.on_finish = Some(...)`, direct mutation). Rewrite each to the new API:

```rust
    #[test]
    fn dialog_fires_finish_signal_on_input_enter() {
        let mut harness = DialogTestHarness::new();
        let mut dlg = emDialog::new(
            std::rc::Rc::clone(&harness.root),
            "Test",
            emLook::new(),
            &mut harness.sched,
            &mut harness.pending_windows,
        );

        // Simulate Enter via Input on the root panel.
        // (More direct: set pending_result and wake engine.)
        dlg.Finish(
            DialogResult::Ok,
            &mut harness.tree_of(&dlg),
            &mut harness.sched,
        );

        let finish_sig = dlg.GetFinishSignal();
        harness.run_one_slice();

        assert!(harness.sched.is_pending(finish_sig));
        assert_eq!(
            dlg.GetResult(&mut harness.tree_of(&dlg)),
            Some(DialogResult::Ok),
        );

        dlg.deregister(
            &mut harness.tree_of(&dlg),
            &mut harness.sched,
            &mut harness.framework_actions,
        );
    }
```

Similar rewrites for:
- `dialog_finish_fires_callback`
- `check_finish_can_veto`
- `dialog_custom_result`
- `enter_finishes_with_ok` / `escape_finishes_with_cancel` — now test that Enter on DlgPanel sets pending, next slice finalizes
- `enter_with_modifier_is_ignored` / `release_event_is_ignored` — as-is, on DlgPanel
- `add_custom_button_lookup`, `set_button_label`, `auto_deletion_toggle`, `check_finish_lifecycle`, `set_root_title`

All 11 existing tests rewrite.

**The `DialogTestHarness`** (new, in `test_view_harness.rs`) bundles:
- `sched: EngineScheduler`
- `root: Rc<emContext>`
- `framework_actions: Vec<DeferredAction>`
- `pending_windows: Vec<PendingWindow>` (or whatever Step 5.1a confirmed)
- A method `tree_of(dlg: &emDialog) -> &mut PanelTree` — reaches into pending_windows to find the dialog's tree by WindowId match.
- `run_one_slice(&mut self)` — calls `EngineScheduler::DoTimeSlice` with dummy pending_inputs + input_state + framework_clipboard.

Define this harness in `crates/emcore/src/test_view_harness.rs` (which already exists — the plugin code reads from it). Add it under `#[cfg(any(test, feature = "test-support"))]`.

- [ ] **Step 5.11: Run all emDialog tests + full gate.**

```bash
cargo-nextest run -p emcore --lib emDialog::tests
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Expected: all 11 rewritten emDialog tests + 5 new DlgPanel/DlgButton/Private-engine tests pass. Total nextest expected around `2476 + 5 new` for the new tests = `2481` baseline, but note:
- Old tests were 11 (they pass today).
- Rewritten tests are 11 (with new bodies).
- New DlgPanel/DlgButton/Engine tests: 3 + 2 + 1 = 6 net new.

So at Task 5 exit: `2476 (baseline) + 6 (new) = 2482` passed. If numbers differ, count mismatch is OK as long as nothing regressed (0 failed).

- [ ] **Step 5.12: Commit Task 5.**

```
- **Task 5 — emDialog reshape:** commit <SHA>. emDialog is now a façade over
  emWindow + DlgPanel + DialogPrivateEngine. Ctor takes a scheduler +
  pending-windows queue + parent context; creates signals, installs DlgPanel
  in a fresh PanelTree, registers DialogPrivateEngine at Priority::High
  connected to close_signal, queues the emWindow for framework materialization.
  All public methods (AddCustomButton, AddPositiveButton, AddNegativeButton,
  AddOKButton, AddCancelButton, AddOKCancelButtons, GetButton, GetButtonForResult,
  GetOKButton, GetCancelButton, Finish, GetResult, CheckFinish, SetRootTitle,
  EnableAutoDeletion, IsAutoDeletionEnabled, GetFinishSignal, GetContentPanel,
  set_button_label_for_result) route through tree.take_behavior(root_panel_id).
  `pub fn Cycle(&mut self, ctx)` DELETED. silent_cancel DELETED. Added
  `deregister(tree, scheduler, framework_actions)` — D3 primary path.
  Added `wire_window_id(winit::WindowId)` post-materialization hook.
  ShowMessage ported to new shape. DialogTestHarness helper in
  test_view_harness.rs. 11 existing emDialog tests rewritten.
  Gate green — nextest 2482/0/9, goldens 237/6 (no paint change).
```

```bash
git add crates/emcore/src/emDialog.rs \
        crates/emcore/src/test_view_harness.rs \
        crates/emcore/src/emPanel.rs \
        docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md
# plus any framework plumbing files touched in Step 5.1a's prereq
git commit -m "phase-3.5 task 5: emDialog reshaped — façade over emWindow + DlgPanel + DialogPrivateEngine

Ports C++ class emDialog : public emWindow (emDialog.h:37). emDialog
is now a small façade holding WindowId (lazy — set on framework
materialization), PanelId of its DlgPanel root, EngineId of its
DialogPrivateEngine (Priority::High, connected to close_signal per
AddWakeUpSignal).

All public methods route through tree.take_behavior(root_panel_id) to
mutate DlgPanel state. pub fn Cycle and silent_cancel DELETED — dialog
Cycle now runs inside DialogPrivateEngine at scheduler dispatch time.

Added deregister(tree, scheduler, framework_actions) helper — D3 primary
path per Do-NOT constraints (Drop has no scheduler reach).

Gate green — nextest 2482/0/9, goldens preserved."
```

**Task 5 exit condition (invariants):**
- `rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emDialog.rs` → 0
- `rg -n 'silent_cancel' crates/emcore/src/emDialog.rs` → 0
- `rg -n 'window: emWindow\|window_id:' crates/emcore/src/emDialog.rs` → ≥1
- `rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs` → 1

---

## Task 6: Migrate `emStocksListBox` dialog sites + delete `silent_cancel`

**Files:**
- Modify: `crates/emstocks/src/emStocksListBox.rs` — 4 dialog-creation sites

**Scope:** The 4 sites (`DeleteStocks`, `CutStocks`, `PasteStocks`, `SetInterest`) currently do:
```rust
if let Some(ref mut d) = self.delete_stocks_dialog {
    d.silent_cancel();
}
let mut dialog = emDialog::new(cc, "...", look);
dialog.AddCustomButton(...);
...
self.delete_stocks_dialog = Some(dialog);
```

New shape (per D3):
```rust
if let Some(mut d) = self.delete_stocks_dialog.take() {
    d.deregister(tree, scheduler, framework_actions);
    // d drops here — its emWindow is torn down by the CloseWindow DeferredAction.
}
let mut dialog = emDialog::new(parent_context, "...", look, scheduler, pending_windows);
dialog.AddCustomButton(tree, scheduler, framework_actions, root_context, ..., DialogResult::Ok);
dialog.AddCustomButton(tree, scheduler, framework_actions, root_context, "Cancel", DialogResult::Cancel);
self.delete_stocks_dialog = Some(dialog);
```

Each call site now needs access to:
- `scheduler: &mut EngineScheduler`
- `tree: &mut PanelTree`
- `framework_actions: &mut Vec<DeferredAction>`
- `root_context: &Rc<emContext>`
- `pending_windows: &mut PendingWindowQueue` (or however Step 5.1a resolved)

These already flow through the existing `cc: &mut C: ConstructCtx` parameter at the call site in some form. The migration shape depends on the exact call-site signatures in emStocksListBox. Plan:

- [ ] **Step 6.1: Find the 4 call sites.**

```bash
rg -n 'emDialog::new\|silent_cancel' crates/emstocks/src/emStocksListBox.rs
```

Expected: 4 `emDialog::new` + 4 `silent_cancel` sites (one pair per stock-list action).

- [ ] **Step 6.2: Extend the call-site signatures to thread `scheduler` + `tree` + `framework_actions` + `root_context` + `pending_windows` if not already present.**

Each calling method (e.g., `pub fn DeleteStocks<C: ConstructCtx>(&mut self, cc: &mut C, ...)`) expands to:

```rust
pub fn DeleteStocks(
    &mut self,
    tree: &mut crate::emPanelTree::PanelTree,
    scheduler: &mut crate::emScheduler::EngineScheduler,
    framework_actions: &mut Vec<emcore::emEngineCtx::DeferredAction>,
    root_context: &std::rc::Rc<emcore::emContext::emContext>,
    pending_windows: &mut /* PendingWindowQueue-or-equiv */,
    rec: &mut emStocksRec,
    ask: bool,
) {
    ...
}
```

**Implementor:** for each of the 4 methods, update the signature + all in-crate call sites (look in `emStocksItemPanel`, `emStocksControlPanel`, `emStocksFpPlugin`). Expect ~8-15 call-site updates per method.

- [ ] **Step 6.3: Rewrite the 4 dialog-creation bodies.**

Example for DeleteStocks (currently around emStocksListBox.rs:477-496):

```rust
pub fn DeleteStocks(
    &mut self,
    tree: &mut crate::emPanelTree::PanelTree,
    scheduler: &mut emcore::emScheduler::EngineScheduler,
    framework_actions: &mut Vec<emcore::emEngineCtx::DeferredAction>,
    root_context: &std::rc::Rc<emcore::emContext::emContext>,
    pending_windows: &mut /* PendingWindowQueue */,
    rec: &mut emStocksRec,
    ask: bool,
) {
    if self.GetSelectionCount() == 0 {
        return;
    }
    if ask {
        if let Some(ref look) = self.look {
            // Replace in-flight dialog: deregister then drop (D3).
            if let Some(mut d) = self.delete_stocks_dialog.take() {
                d.deregister(tree, scheduler, framework_actions);
                // d drops at end of scope — deregister issued CloseWindow action.
            }
            let count = self.GetSelectionCount();
            let mut dialog = emDialog::new(
                std::rc::Rc::clone(root_context),
                &format!("Really delete {} stock(s)?", count),
                look.clone(),
                scheduler,
                pending_windows,
            );
            dialog.AddCustomButton(
                tree, scheduler, framework_actions, root_context,
                "Delete", DialogResult::Ok,
            );
            dialog.AddCustomButton(
                tree, scheduler, framework_actions, root_context,
                "Cancel", DialogResult::Cancel,
            );
            self.delete_stocks_dialog = Some(dialog);
        }
        return;
    }
    // ask=false path unchanged.
    /* ... existing body ... */
}
```

Replicate for `CutStocks`, `PasteStocks`, `SetInterest`.

- [ ] **Step 6.4: Verify silent_cancel has zero remaining callers.**

```bash
rg -n 'silent_cancel' crates/
```

Expected: 0 matches. If any remain, trace + migrate + re-run.

- [ ] **Step 6.5: Full gate.**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo-nextest ntr
```

Any nextest regression must be resolved before commit. Most common failure class: call-site signatures mismatched (add missing args); integration tests constructing emDialog directly (migrate to DialogTestHarness).

- [ ] **Step 6.6: Commit Task 6.**

```
- **Task 6 — Consumer migration:** commit <SHA>. 4 emStocksListBox dialog sites
  migrated (DeleteStocks, CutStocks, PasteStocks, SetInterest). Each uses
  dialog.deregister(tree, scheduler, framework_actions) + drop instead of
  silent_cancel when replacing an in-flight dialog. silent_cancel removed.
  ~N in-crate call sites updated to thread scheduler+tree+framework_actions+
  root_context+pending_windows. Gate green — nextest (count)/0/9.
```

```bash
git add crates/emstocks/src/ docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md
git commit -m "phase-3.5 task 6: migrate emStocksListBox dialog sites to new emDialog API

4 sites (DeleteStocks/CutStocks/PasteStocks/SetInterest) use
dialog.deregister() + drop instead of silent_cancel when replacing an
in-flight dialog. Matches C++ delete OldDialog semantics (D3).

Method signatures extended to thread scheduler+tree+framework_actions+
root_context+pending_windows through to the dialog-creation sites.

Gate green."
```

**Task 6 exit condition:** `rg -n 'silent_cancel' crates/` → 0 matches.

---

## Task 7: Phase closeout — ledger + invariant sweep + tag

**Files:**
- Modify: `docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md`

- [ ] **Step 7.1: Run invariants from the brainstorm §5.**

```bash
# I5a: emDialog window identity
rg -n 'window_id:' crates/emcore/src/emDialog.rs | head -5
# Expected: ≥1 match

# I5b: DialogPrivateEngine is an emEngine (single type per D2)
rg -n 'impl emEngine for DialogPrivateEngine' crates/emcore/src/emDialog.rs
# Expected: exactly 1

# I5c: engine registration + wake-up subscription
rg -nU 'DialogPrivateEngine::install\|register_engine.*DialogPrivateEngine' crates/emcore/src/emDialog.rs
# Expected: ≥1
rg -n 'scheduler\.connect' crates/emcore/src/emDialog.rs
# Expected: ≥1 (close_signal + every button's click_signal; note that click_signal
# connect happens in AddCustomButton so this grep may return multiple)

# I5d: no caller-invoked dialog Cycle
rg -n 'pub fn Cycle\s*\(.*PanelCtx' crates/emcore/src/emDialog.rs
# Expected: 0

# I5e: silent_cancel deleted
rg -n 'silent_cancel' crates/
# Expected: 0

# I5f: no new Rc<RefCell< in emDialog.rs
rg -n 'Rc<RefCell<' crates/emcore/src/emDialog.rs
# Expected: baseline (no increase)

# I5g: no new unsafe in emDialog.rs
rg -n 'unsafe\s*\{' crates/emcore/src/emDialog.rs
# Expected: 0 or documented aliased-borrow unsafe (from DialogPrivateEngine::Cycle Step 4.1 — SchedCtx construction)

# I5i (golden preservation — slow; run last):
cargo test --test golden -- --test-threads=1
# Expected: 237 passed / 6 failed (baseline preserved)

# I5j (nextest delta):
cargo-nextest ntr
# Expected: baseline (2476) + net delta from new tests
```

Record results in the ledger.

- [ ] **Step 7.2: Confirm E024 remains open (closure is Phase 3.6).**

```bash
python3 -c "
import json
with open('docs/superpowers/notes/2026-04-19-port-divergence-raw-material.json') as f:
    d = json.load(f)
e = next(x for x in d['entries'] if x['id']=='E024')
print('status:', e.get('status','open'))
print('phase_3_progress:', e.get('phase_3_progress',''))
"
```

Expected: `status: open`, `phase_3_progress` unchanged from Phase 3 closeout. Do NOT touch the JSON in this phase — 3.6 handles the flip.

- [ ] **Step 7.3: Write phase closeout note.**

Create `docs/superpowers/notes/2026-04-22-phase-3-5-closeout.md`:

```markdown
# Phase 3.5 — emDialog as emWindow — Closeout

**Branch:** port-rewrite/phase-3-5-emdialog-as-emwindow
**Commits:** <SHA-range>
**Status:** COMPLETE
**Merged to main:** <SHA> (pending user confirmation)

## Summary

Phase 3.5 ported `emDialog` from a plain Rust struct with a caller-driven
`Cycle` method to the C++-structural port of `class emDialog : public emWindow`.
DlgPanel, DlgButton, and DialogPrivateEngine are now in emDialog.rs as
private-module types matching the C++ nested classes. emDialog itself is a
small façade over WindowId + PanelId + EngineId + finish_signal; all state
mutation routes through tree.take_behavior(root_panel_id) to the DlgPanel
behavior where the actual dialog state lives.

Pre-port emDialog's `pub fn Cycle(&mut self, ctx) -> bool` is DELETED.
`silent_cancel()` is DELETED. Callers tear down via `dialog.deregister(tree,
scheduler, framework_actions)` + drop.

E024 remains OPEN — its closure requires emFileDialog riding the 3.5
infrastructure, which is Phase 3.6.

## Delta from Phase 3 baseline

<from ledger>

## Invariants

<from ledger>

## Next phase

Phase 3.6 — emFileDialog consumes 3.5, E024 closes.
```

- [ ] **Step 7.4: Tag + final commit.**

```bash
git add docs/superpowers/notes/2026-04-22-phase-3-5-ledger.md \
        docs/superpowers/notes/2026-04-22-phase-3-5-closeout.md
git commit -m "phase-3.5 closeout — emDialog as emWindow-derived; all invariants pass

Invariants I5a, I5b, I5c (close_signal only), I5d (emDialog.rs), I5f, I5g,
I5i, I5j all green. E024 remains open per scope — Phase 3.6 closes it.

Next: Phase 3.6 plan at docs/superpowers/plans/2026-04-21-port-rewrite-phase-3-6-emfiledialog-e024.md"

git tag port-rewrite-phase-3-5-complete
```

- [ ] **Step 7.5: Merge to main.**

```bash
git checkout main
git merge --no-ff port-rewrite/phase-3-5-emdialog-as-emwindow
```

(If merge conflicts: the 3.5 branch cut from `d0f1cc7b` and main hasn't moved — conflicts should be impossible. If any arise, the user has committed other work in parallel; resolve per conflict-resolution conventions.)

---

## Self-review — guardrails against drift + implicit assumptions check

### Implicit assumptions this plan makes — audit status

| # | Assumption | Verified? | Risk if false | Mitigation |
|---|---|---|---|---|
| A1 | `emWindow::new_popup_pending` + framework materialization drain is reachable by non-popup-zoom consumers (i.e. we can enqueue a pending emWindow from arbitrary code) AND each emWindow can have its own PanelTree root. | **HIGH RISK — Step 5.1a confirmed both assumptions are LIKELY FALSE in current code.** App has one shared `PanelTree`, with `create_root` asserting a single root. Popup windows live in single-slot `emView::PopupWindow`, not in App::windows. No runtime top-level-window-install path exists today. | **Phase 3.5.A — runtime top-level emWindow install + multi-tree-or-non-root-subtree support — is a FORCED prereq sub-phase.** Phase 3.5 body cannot execute Task 5.2+ until 3.5.A lands. | Step 5.1a/5.1b formalises the split: first run the greps; on confirming the gap, commit whatever partial 3.5 work is complete (Tasks 1–4), tag `port-rewrite-phase-3-5-partial-before-3-5-A`, switch to writing + implementing 3.5.A, merge, then resume. |
| A2 | Rust `emLook::background_color()` returns an `emColor` suitable for emWindow's background. | **UNVERIFIED.** | Step 5.2's ctor call to `look.background_color()` may not compile. | Fallback: use `emColor::BLACK` or whatever is canonically used by popup-zoom's existing emWindow. |
| A3 | `emButton::new` signature: either `new(caption: &str)` or `new<C: ConstructCtx>(ctx: &mut C, caption: &str)`. | **UNVERIFIED — confirm at Step 3.1.** | DlgButton::new signature wrong → compile failure. | Step 3.3's gotcha note covers both shapes. |
| A4 | `ctx.panel_size()` helper on PanelCtx exists. | **UNVERIFIED.** | Step 2.2's LayoutChildren may not compile. | Step 2.2 notes: if missing, add it inline in the same commit. |
| A5 | `DoTimeSlice` signature matches the Step 4.2 test body. | **UNVERIFIED.** | Step 4.2 test won't compile. | Step 4.2 notes: adjust to actual signature discovered via `rg 'pub fn DoTimeSlice'`. |
| A6 | Rust `emButton` has a `click_signal: SignalId` field (Phase 3 B3.4b landed this). | **VERIFIED** — confirmed via Phase-3 ledger (B3.4b added all widget `*_signal: SignalId` fields). | — | — |
| A7 | `DialogPrivateEngine` can call `ctx.scheduler.fire` AND hold a SchedCtx alias for the on_finish callback — aliasing pattern is safe by the existing `PanelCycleEngine::Cycle` precedent. | **VERIFIED** — emPanelCycleEngine.rs:83-123 uses the same aliased-borrow pattern with SAFETY comments. | — | — |
| A8 | Per-panel `PanelCycleEngine` does NOT conflict with the dialog's `DialogPrivateEngine`. DlgPanel has its own PanelCycleEngine (allocated by `register_engine_for` when `init_panel_view` runs); the DialogPrivateEngine is a SEPARATE registration at HIGH_PRIORITY. | **VERIFIED** — this matches C++ (emPanel inherits emEngine AND emDialog holds a distinct PrivateEngine member). | — | — |
| A9 | The `emStocksListBox` 4 call-site method signatures can be extended with 5 extra args without breaking callers beyond emstocks+emmain crates. | **UNVERIFIED.** | Task 6 may surface ripple beyond those crates. | Task 6 Step 6.2 explicitly allocates time for the signature ripple; if scope balloons, split into 6a/6b. |
| A10 | `DlgPanel` can be installed as root of an emWindow that still uses popup-zoom's `VF_POPUP_ZOOM` view flag — i.e. a dialog-at-startup doesn't conflict with the popup-zoom machinery already consuming new_popup_pending. | **UNVERIFIED.** | Dialog and popup-zoom may compete. | If symptom appears: confirm whether emDialog needs `VF_POPUP_ZOOM` at all (C++ has it by default, but it's orthogonal to popup-zoom-initiated-from-emView behavior). |
| A11 | `remove_engine` (scheduler API) correctly deregisters the engine AND removes its signal connections. | **VERIFIED** — emScheduler.rs:330-341 iterates `signals` and removes connected_engines. | — | — |
| A12 | `tree.remove(root_panel_id)` recursively removes the whole dialog subtree including DlgButtons and their click_signal connections. | **VERIFIED** — emPanelTree.rs:671-707 collects descendants + deregisters each panel's engine. Signal connections through per-panel PanelCycleEngine are torn down when the engine is removed. DlgButton's click_signal is still in the signals slotmap but has no connections — dead signal. Clean. | — | — |

### Drift risks

| Risk | Guardrail |
|---|---|
| Implementor writes `dlg_button.click_signal` in the outer emDialog.rs but DlgButton wraps emButton in a field `self.button` — the access would be `self.button.click_signal`. | Each code block in this plan uses the exact field-access path; if the compiler complains, that's the right error to debug. No shortcut allowed. |
| Implementor collapses `DlgPanel.button_signals` into `DlgPanel.buttons` (tuples with 3 elements instead of 2 + parallel vec). | Defensible — both shapes are equivalent. If chosen, update both Task 4's Cycle body and Task 5.5's AddCustomButton consistently. Flag the change in the ledger. |
| Implementor adds `Rc<RefCell<>>` as "just a temporary" to avoid the tree-take/put dance. | Gate I5f catches at commit time (clippy or the rg check). The pattern is explicitly forbidden; the take/put idiom is mandatory. |
| Implementor uses `Box<dyn Any>` + downcast_ref to reach DlgPanel from behavior. | Violates Do-NOT list (no Any). Use `as_dlg_panel_mut` per Step 4.1. |
| Implementor silently changes `Priority::High` to `Priority::VeryHigh` or `Medium`. | B3.5d spells out the decision. Any change requires ledger entry citing C++ line that permits the change. |
| Task 5.1a reveals a gap, and implementor inlines the fix rather than splitting to 3.5.A. | Step 5.1b explicitly instructs: *split, don't inline.* A multi-commit prereq inside Task 5 loses the gate-green-per-task property. |
| Implementor rewrites a test to pass rather than fixing the underlying port bug. | Tests assert C++-parity beats (signal pending after input, finalized_result after slice). A test change must cite the C++ line that says the new assertion is also correct. |
| AutoDelete countdown advances in the wrong slice count. | Test the count explicitly: `assert_eq!(dlg.finish_state, 0)` before 1st slice, `1` after 1st, `2` after 2nd, `3` after 3rd (then DeferredAction issued). |
| Callback `on_finish` + `on_finished` invocation order swaps. | C++ order (emDialog.cpp:~200-206): on_check_finish → set result → fire finish_signal → on_finish (signal-listener-equivalent) → Finished (subclass hook). Task 4 Step 4.1's Cycle body preserves this order. Regression test explicitly recording the order of callback invocations catches swaps. |

### Gotchas that didn't make it into the plan body

- `DialogTestHarness::tree_of(dlg)` — needs an API to reach the tree for a given dialog. Implementor should note this: if `pending_windows` is a `Vec<(emWindow, PanelTree)>`, find by WindowId. If some other shape, adapt.
- The `ButtonPanel` adapter precedent (`emColorFieldFieldPanel.rs:191`) uses `state.viewed_rect` for pixel_scale. For DlgPanel (root), `viewed_rect` is the whole window viewport; `pixel_scale = 1.0` is a simplification that may not match C++ precisely. If golden tests regress on paint paths involving dialogs, revisit. Not expected — dialogs have no golden tests today.
- The `Rc::clone(&parent_context)` in `emDialog::new` assumes the parent context outlives the dialog. It does: emContext is Rc'd and the caller holds one; the dialog holds another Rc clone. Drop order is irrelevant for correctness.

---

## Execution handoff

Plan complete. Given unattended execution, I'll produce Phase 3.6 next, with its own self-review. When both plans are written + reviewed, execution uses `superpowers:subagent-driven-development` (per-task subagent dispatch + review) by default.
