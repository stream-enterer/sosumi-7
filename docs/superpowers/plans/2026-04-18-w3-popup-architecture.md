# W3 — Popup Creation Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete `PopupPlaceholder`; restore `emView::PopupWindow` to hold a real `emWindow`; split `emWindow` into a synchronous emCore-level struct + a deferred winit/wgpu surface materialization.

**Architecture:** Decouple the `emWindow` object (emCore entity with signals, viewport, view, flags) from the OS surface (`winit::Window` + `wgpu::Surface`). Popup creation in `emView::RawVisitAbs` is synchronous for the struct and every emCore observable; OS surface creation is deferred to the next `about_to_wait` drain via the existing `pending_actions` queue. Preserves C++ author's atomic popup-entry invariant within winit's smallest-necessary concession.

**Tech Stack:** Rust, winit, wgpu, single-threaded `Rc<RefCell<T>>` ownership.

**Spec:** `docs/superpowers/specs/2026-04-18-w3-popup-architecture-design.md`

**Commands:**
```bash
cargo check
cargo clippy -- -D warnings
cargo-nextest ntr
```
Pre-commit runs fmt → clippy → nextest. Do not skip with `--no-verify`.

---

## File Structure

### Modified files

| File | Role |
|---|---|
| `crates/emcore/src/emWindow.rs` | Add `OsSurface` enum, refactor surface-dependent fields into `Materialized` variant, add `new_popup_pending` constructor, guard surface-touching methods |
| `crates/emcore/src/emGUIFramework.rs` | Add `materialize_popup_surface` method; enqueue via `pending_actions`; handle iteration-safe insertion into `self.windows` |
| `crates/emcore/src/emView.rs` | Delete `PopupPlaceholder` (lines ~1-70); change `PopupWindow: Option<Rc<RefCell<emWindow>>>` at ~line 467; rewrite `RawVisitAbs` popup entry branch at ~line 1666-1745; verify destruction branch symmetry |
| `crates/eaglemode/tests/unit/popup_window.rs` | Remove dead DISPLAY/WAYLAND_DISPLAY gate; retarget function-pointer assertion |

### New files

| File | Role |
|---|---|
| `crates/eaglemode/tests/unit/popup_materialization.rs` | DISPLAY-gated test: verify Pending→Materialized transition after one `about_to_wait` pass |
| `crates/eaglemode/tests/unit/popup_cancel_before_materialize.rs` | Non-gated test: verify drop-while-pending cancels cleanly, no winit window created |

### Test files touched inline

- `crates/emcore/src/emView.rs` inline tests — `test_phase4_popup_zoom_creates_popup_window` should pass unchanged after the refactor. Run it to verify.

---

## Architectural pre-reads

Before starting Phase 1, the implementer must read:

1. **Spec:** `docs/superpowers/specs/2026-04-18-w3-popup-architecture-design.md` — the full observational-port rationale.
2. **C++ reference:** `~/git/eaglemode-0.96.4/src/emCore/emView.cpp:1628-1683` — the popup entry/exit branch in `RawVisitAbs`.
3. **Current Rust state:**
   - `crates/emcore/src/emView.rs:1-70` (the `PopupPlaceholder` scaffold to be deleted)
   - `crates/emcore/src/emView.rs:1666-1745` (the current popup branch using `PopupPlaceholder`)
   - `crates/emcore/src/emWindow.rs:39-219` (the `emWindow` struct and `create` method)
   - `crates/emcore/src/emGUIFramework.rs:317-360` (`about_to_wait`, `pending_actions` drain)

**Invariant preserved throughout:** After `emView::RawVisitAbs` returns in the popup-entry branch, `self.PopupWindow.is_some()` must be true and every `emWindow` struct field (close_signal, flags_signal, focus_signal, geometry_signal, background color, view, flags, viewport-with-window-backref) must be in its post-construction state. Only the OS surface materialization is deferred.

---

## Phase 1: Refactor `emWindow` for Pending/Materialized surface states

**Goal:** Move the six surface-dependent fields into a new `OsSurface` enum while keeping all non-popup call sites working unchanged. Establish `Pending` as a representable state with no callers yet.

### Task 1: Introduce `OsSurface` enum with both variants present

**Files:**
- Modify: `crates/emcore/src/emWindow.rs`

- [ ] **Step 1: Add the `OsSurface` enum above the `emWindow` struct**

Insert immediately after the `WindowFlags` `bitflags!` block (around line 36, before `/// An eaglemode-rs window:`):

```rust
/// The OS-level surface state of an `emWindow`.
///
/// Popup windows created from `emView::RawVisitAbs` are constructed in
/// `Pending` state (no winit/wgpu objects yet) because `RawVisitAbs`
/// runs inside `emView::Update` where `&ActiveEventLoop` is unavailable.
/// The `emGUIFramework::about_to_wait` drain materializes the surface on
/// the next tick, transitioning to `Materialized`.
///
/// Non-popup windows (the home window, duplicate/ccw children) enter
/// `Materialized` directly at construction time.
pub(crate) enum OsSurface {
    Pending {
        flags: WindowFlags,
        caption: String,
        requested_pos_size: Option<(i32, i32, i32, i32)>,
    },
    Materialized {
        winit_window: Arc<winit::window::Window>,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
        compositor: WgpuCompositor,
        tile_cache: TileCache,
        viewport_buffer: crate::emImage::emImage,
    },
}
```

- [ ] **Step 2: Replace the six surface-dependent fields on `emWindow` with one `os_surface` field**

Delete lines 40-44 and 49 of the current `emWindow` struct:
```rust
    pub winit_window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    compositor: WgpuCompositor,
    tile_cache: TileCache,
    ...
    viewport_buffer: crate::emImage::emImage,
```

Replace with:
```rust
    pub(crate) os_surface: OsSurface,
```

- [ ] **Step 3: Add accessor methods for the materialized-only fields**

In the `impl emWindow { ... }` block, add these accessors near the top (right after `pub fn create` or before `resize`):

```rust
/// Borrow the materialized surface fields. Panics if called while
/// `os_surface` is `Pending` — call sites that can only reach a
/// materialized window (render, resize, request_redraw after
/// materialization) use this; call sites that must tolerate both
/// states branch on `os_surface` explicitly.
fn materialized(&self) -> (&Arc<winit::window::Window>, &wgpu::Surface<'static>, &wgpu::SurfaceConfiguration, &WgpuCompositor, &TileCache, &crate::emImage::emImage) {
    match &self.os_surface {
        OsSurface::Materialized { winit_window, surface, surface_config, compositor, tile_cache, viewport_buffer } =>
            (winit_window, surface, surface_config, compositor, tile_cache, viewport_buffer),
        OsSurface::Pending { .. } => panic!("emWindow::materialized() called while Pending"),
    }
}

fn materialized_mut(&mut self) -> (&mut Arc<winit::window::Window>, &mut wgpu::Surface<'static>, &mut wgpu::SurfaceConfiguration, &mut WgpuCompositor, &mut TileCache, &mut crate::emImage::emImage) {
    match &mut self.os_surface {
        OsSurface::Materialized { winit_window, surface, surface_config, compositor, tile_cache, viewport_buffer } =>
            (winit_window, surface, surface_config, compositor, tile_cache, viewport_buffer),
        OsSurface::Pending { .. } => panic!("emWindow::materialized_mut() called while Pending"),
    }
}

/// True iff the OS surface has been created.
pub fn is_materialized(&self) -> bool {
    matches!(self.os_surface, OsSurface::Materialized { .. })
}

/// True iff any surface field is accessible on this window today.
/// Used by call sites that must branch on surface state.
pub(crate) fn winit_window_if_materialized(&self) -> Option<&Arc<winit::window::Window>> {
    match &self.os_surface {
        OsSurface::Materialized { winit_window, .. } => Some(winit_window),
        OsSurface::Pending { .. } => None,
    }
}
```

- [ ] **Step 4: Update `create()` to build `OsSurface::Materialized`**

In the `create()` constructor at `crates/emcore/src/emWindow.rs:87-218`, change the final `Rc::new(RefCell::new(Self { ... }))` (around line 155-184) from:
```rust
let window = Rc::new(RefCell::new(Self {
    winit_window,
    surface,
    surface_config,
    compositor,
    tile_cache,
    viewport_buffer,
    view,
    ...
```
to:
```rust
let window = Rc::new(RefCell::new(Self {
    os_surface: OsSurface::Materialized {
        winit_window,
        surface,
        surface_config,
        compositor,
        tile_cache,
        viewport_buffer,
    },
    view,
    ...
```

Leave every other field initializer untouched.

- [ ] **Step 5: Update every `self.winit_window` / `self.surface` / `self.surface_config` / `self.compositor` / `self.tile_cache` / `self.viewport_buffer` access inside `impl emWindow`**

Search the file for each of these six field names. At every access site, replace with the corresponding destructuring. Two patterns apply:

**Pattern A (method that assumes materialized, e.g. `render`, `resize`):** destructure at top via `materialized_mut()`:
```rust
pub fn render(&mut self, tree: &mut crate::emPanelTree::PanelTree, gpu: &GpuContext) {
    let (winit_window, surface, surface_config, compositor, tile_cache, viewport_buffer) = self.materialized_mut();
    // ... rest of method uses locals directly
}
```

Note: `materialized_mut` returns six distinct mutable borrows at once; this works because they're all disjoint fields of the same enum variant. If borrow-checker objects to multiple calls, destructure once at method top.

**Pattern B (method that must tolerate Pending):** inline match:
```rust
match &self.os_surface {
    OsSurface::Materialized { winit_window, .. } => { /* use */ },
    OsSurface::Pending { .. } => { /* no-op or alternate */ },
}
```

Known methods that use Pattern A (always materialized in callers that reach them):
- `resize` (line 247)
- `render` (line 272)
- `render_parallel` (line 386)
- Any `handle_*` or `dispatch_*` methods touching surface fields

Known methods that need Pattern B:
- `request_redraw` (grep for it) — no-op while Pending
- Any method setting cursor or window attributes on `winit_window`

- [ ] **Step 6: Run `cargo check`**

```bash
cargo check
```
Expected: compiles. Compilation errors here indicate missed access sites from Step 5 — resolve each by applying Pattern A or B.

- [ ] **Step 7: Run clippy + tests**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean, all 2409+ tests pass. No test should fail — no popup code paths have changed yet; only struct layout has changed.

- [ ] **Step 8: Commit**

```bash
git add crates/emcore/src/emWindow.rs
git commit -m "refactor(emWindow): split surface-dependent fields into OsSurface enum

Introduces OsSurface { Pending, Materialized }. All existing call paths
construct Materialized directly. No behavior change; Pending has no
callers yet. Prepares for W3 deferred popup surface creation.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 2: Add `emWindow::new_popup_pending` constructor

**Goal:** Provide a synchronous constructor that builds an `emWindow` in `Pending` state, callable from anywhere (no `&ActiveEventLoop` required). No callers yet.

### Task 2: Implement `new_popup_pending`

**Files:**
- Modify: `crates/emcore/src/emWindow.rs`
- Test: `crates/emcore/src/emWindow.rs` inline `#[cfg(test)]` module

- [ ] **Step 1: Write failing test in `emWindow.rs`**

Append to the existing `#[cfg(test)] mod tests` block (or create one if none exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::emPanelTree::PanelTree;
    use crate::emScheduler::emScheduler;

    #[test]
    fn new_popup_pending_constructs_without_event_loop() {
        let mut scheduler = emScheduler::new();
        let mut tree = PanelTree::new();
        let root = tree.CreateRoot("root".to_string());

        let close_sig = scheduler.create_signal();
        let flags_sig = scheduler.create_signal();
        let focus_sig = scheduler.create_signal();
        let geom_sig = scheduler.create_signal();
        let bg_color = crate::emColor::emColor::from_rgb(0, 0, 0);

        let popup = emWindow::new_popup_pending(
            root,
            WindowFlags::POPUP | WindowFlags::UNDECORATED | WindowFlags::AUTO_DELETE,
            "emViewPopup".to_string(),
            close_sig,
            flags_sig,
            focus_sig,
            geom_sig,
            bg_color,
        );

        let p = popup.borrow();
        assert!(!p.is_materialized(), "new_popup_pending must start in Pending state");
        match &p.os_surface {
            OsSurface::Pending { flags, caption, requested_pos_size } => {
                assert!(flags.contains(WindowFlags::POPUP));
                assert_eq!(caption, "emViewPopup");
                assert!(requested_pos_size.is_none());
            }
            OsSurface::Materialized { .. } => panic!("expected Pending"),
        }
        assert_eq!(p.close_signal, close_sig);
        assert_eq!(p.flags_signal, flags_sig);
        assert_eq!(p.focus_signal, focus_sig);
        assert_eq!(p.geometry_signal, geom_sig);
        assert!(p.winit_window_if_materialized().is_none());
    }
}
```

Note: this test references `new_popup_pending` which does not yet exist — compilation will fail at this step.

- [ ] **Step 2: Verify test fails to compile**

```bash
cargo test -p emcore --lib emWindow::tests::new_popup_pending
```
Expected: compile error `no function or associated item named 'new_popup_pending'`.

- [ ] **Step 3: Implement `new_popup_pending`**

Add this constructor to `impl emWindow` block immediately after the existing `new_popup` method (around line 244):

```rust
/// Construct an `emWindow` in `Pending` state — no winit/wgpu objects
/// yet. Callable from any context (does NOT require `&ActiveEventLoop`
/// or `&GpuContext`). The OS surface is materialized later by
/// `emGUIFramework::materialize_popup_surface` drained from
/// `about_to_wait`.
///
/// Mirrors the first five side-effects of C++ `emView::RawVisitAbs`
/// popup-entry (emView.cpp:1636-1643) at the struct level: the
/// `emWindow` object exists and every emCore observer sees a
/// fully-wired popup immediately.
#[allow(clippy::too_many_arguments)]
pub fn new_popup_pending(
    root_panel: PanelId,
    flags: WindowFlags,
    caption: String,
    close_signal: SignalId,
    flags_signal: SignalId,
    focus_signal: SignalId,
    geometry_signal: SignalId,
    background_color: crate::emColor::emColor,
) -> Rc<RefCell<Self>> {
    // Placeholder geometry — real position/size lands via SetViewPosSize
    // before materialization. emView uses 1x1 until set.
    let view = emView::new(root_panel, 1.0, 1.0);
    view.SetBackgroundColor(background_color);

    let vif_chain: Vec<Box<dyn emViewInputFilter>> = vec![
        {
            let mut mouse_vif = emMouseZoomScrollVIF::new();
            let zflpp = view.GetZoomFactorLogarithmPerPixel();
            mouse_vif.set_mouse_anim_params(1.0, 0.25, zflpp);
            mouse_vif.set_wheel_anim_params(1.0, 0.25, zflpp);
            Box::new(mouse_vif)
        },
        Box::new(emKeyboardZoomScrollVIF::new()),
    ];

    let window = Rc::new(RefCell::new(Self {
        os_surface: OsSurface::Pending {
            flags,
            caption,
            requested_pos_size: None,
        },
        view,
        flags,
        close_signal,
        flags_signal,
        focus_signal,
        geometry_signal,
        root_panel,
        vif_chain,
        cheat_vif: emCheatVIF::new(),
        touch_vif: emDefaultTouchVIF::new(),
        active_animator: None,
        window_icon: None,
        last_mouse_pos: (0.0, 0.0),
        screensaver_inhibit_count: 0,
        screensaver_cookie: None,
        flags_changed: false,
        focus_changed: false,
        geometry_changed: false,
        wm_res_name: String::from("eaglemode-rs"),
        render_pool: emRenderThreadPool::new(
            crate::emCoreConfig::emCoreConfig::default().max_render_threads,
        ),
    }));

    // Wire the emViewPort back-reference, same as `create()`.
    {
        let win_ref = window.borrow();
        let vp = win_ref.view.CurrentViewPort.clone();
        vp.borrow_mut().window = Some(Rc::downgrade(&window));
    }

    window
}
```

Note: if `emView::SetBackgroundColor` does not exist in that exact form, locate the equivalent emView API for setting background color. The C++ call at `emView.cpp:1643` is `PopupWindow->SetBackgroundColor(GetBackgroundColor())` on the `emWindow`, not the `emView`. Check `emWindow.rs` for an existing `SetBackgroundColor` method — if present, store `background_color` via the existing path; if the background color currently lives on `emView`, use the view-level setter. The test above reads the signal fields to verify wiring; adjust the background-color invariant check if it differs in the struct.

- [ ] **Step 4: Verify test passes**

```bash
cargo test -p emcore --lib emWindow::tests::new_popup_pending
```
Expected: PASS.

- [ ] **Step 5: Run full test suite + clippy**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: all pass (2410+ tests — one new test).

- [ ] **Step 6: Commit**

```bash
git add crates/emcore/src/emWindow.rs
git commit -m "feat(emWindow): add new_popup_pending constructor for W3 deferred creation

Builds an emWindow in OsSurface::Pending state without requiring
&ActiveEventLoop or &GpuContext. Called by emView::RawVisitAbs for
synchronous popup-entry; OS surface materializes later.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 3: Add `emGUIFramework::materialize_popup_surface`

**Goal:** Framework-side method that takes a `Pending` popup, creates the OS surface, inserts into `self.windows`. Called from the `pending_actions` drain at `about_to_wait:322-325`.

### Task 3: Implement `materialize_popup_surface`

**Files:**
- Modify: `crates/emcore/src/emGUIFramework.rs`

- [ ] **Step 1: Locate the existing `pending_actions` drain in `about_to_wait`**

Open `crates/emcore/src/emGUIFramework.rs` and read lines 317-330. Confirm:
- `pending_actions: Vec<DeferredAction>` field exists on the framework struct
- `DeferredAction` is a type alias for `Box<dyn FnOnce(&mut emGUIFramework, &ActiveEventLoop)>` (or similar)
- The drain at line 322-325 runs before the scheduler tick

Record the exact `DeferredAction` signature — the closure in Task 4 must match it.

- [ ] **Step 2: Add `materialize_popup_surface` method**

In `impl emGUIFramework` (same file), add this method. Place it near other window-construction helpers (search for `fn new_window` or `fn handle_duplicate`):

```rust
/// Materialize a popup window's OS surface, transitioning it from
/// `OsSurface::Pending` to `OsSurface::Materialized`. Called from the
/// `pending_actions` drain in `about_to_wait` where `&ActiveEventLoop`
/// is available.
///
/// **Cancellation:** if the popup was already dropped from
/// `emView::PopupWindow` before this method runs (e.g. a popup-exit
/// happened in the same frame as popup-entry), `win_rc` is the only
/// remaining strong reference and the materialization is skipped.
/// The Rc drops at function end; no winit window is created.
pub(crate) fn materialize_popup_surface(
    &mut self,
    win_rc: Rc<RefCell<crate::emWindow::emWindow>>,
    event_loop: &winit::event_loop::ActiveEventLoop,
) {
    use crate::emWindow::{OsSurface, WindowFlags};

    // Cancellation check: if we're the only strong ref, the popup was
    // dropped before materialization. Abort silently.
    if Rc::strong_count(&win_rc) == 1 {
        return;
    }

    // Extract Pending params.
    let (flags, caption, requested_pos_size) = {
        let w = win_rc.borrow();
        match &w.os_surface {
            OsSurface::Pending { flags, caption, requested_pos_size } =>
                (*flags, caption.clone(), *requested_pos_size),
            OsSurface::Materialized { .. } => {
                log::warn!("materialize_popup_surface called on already-materialized window");
                return;
            }
        }
    };

    // Build winit window attributes — mirrors emWindow::create lines 88-101.
    let mut attrs = winit::window::WindowAttributes::default().with_title(caption.as_str());
    if flags.contains(WindowFlags::UNDECORATED) {
        attrs = attrs.with_decorations(false);
    }
    if flags.contains(WindowFlags::POPUP) {
        attrs = attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
    }
    if flags.contains(WindowFlags::MAXIMIZED) {
        attrs = attrs.with_maximized(true);
    }
    if flags.contains(WindowFlags::FULLSCREEN) {
        attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    }

    let winit_window = Arc::new(
        event_loop.create_window(attrs).expect("failed to create popup window"),
    );

    // Apply requested_pos_size if set. winit outer_position/set_outer_position
    // is synchronous on X11/Windows; Wayland may ignore positioning.
    if let Some((x, y, w, h)) = requested_pos_size {
        let _ = winit_window.request_inner_size(winit::dpi::PhysicalSize::new(w as u32, h as u32));
        winit_window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    }

    let size = winit_window.inner_size();
    let w = size.width.max(1);
    let h = size.height.max(1);

    let gpu = &self.gpu;
    let surface = gpu.instance.create_surface(winit_window.clone())
        .expect("failed to create popup surface");
    let caps = surface.get_capabilities(&gpu.adapter);
    let format = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: w,
        height: h,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&gpu.device, &surface_config);

    let compositor = crate::emViewRendererCompositor::WgpuCompositor::new(&gpu.device, format, w, h);
    let tile_cache = crate::emViewRendererTileCache::TileCache::new(w, h, 256);
    let viewport_buffer = crate::emImage::emImage::new(w, h, 4);

    // Transition state.
    {
        let mut w_mut = win_rc.borrow_mut();
        w_mut.os_surface = OsSurface::Materialized {
            winit_window: winit_window.clone(),
            surface,
            surface_config,
            compositor,
            tile_cache,
            viewport_buffer,
        };

        // Resize view to actual surface size (Pending had 1x1).
        // Use PanelTree from self.tree for the SetGeometry call.
    }

    // SetGeometry needs &mut tree — release win_rc borrow first.
    {
        let mut w_mut = win_rc.borrow_mut();
        w_mut.view.SetGeometry(&mut self.tree, 0.0, 0.0, w as f64, h as f64, 1.0);
    }

    // Insert into windows map under the new WindowId.
    let window_id = winit_window.id();
    self.windows.insert(window_id, win_rc.clone());

    // First redraw.
    winit_window.request_redraw();
}
```

Note: if `self.gpu` or `self.tree` have different field names in the current tree, adjust. Use `grep -n 'gpu:\|tree:' crates/emcore/src/emGUIFramework.rs` if unsure.

Note: `gpu.instance`, `gpu.device`, `gpu.adapter`, `gpu.queue` — confirm field names match `GpuContext` definition (see `crates/emcore/src/emGUIFramework.rs` for `struct GpuContext`).

- [ ] **Step 3: Run `cargo check`**

```bash
cargo check
```
Expected: compiles. Resolve any missing imports (`Rc`, `Arc`, `RefCell`, `wgpu`, `winit::event_loop::ActiveEventLoop`) at the top of the file.

- [ ] **Step 4: Run clippy + tests**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: all pass. `materialize_popup_surface` has no callers yet; it's dead code. Clippy may warn about dead code — if so, add `#[allow(dead_code)]` on the method for this task, to be removed in Phase 4 when the caller lands.

- [ ] **Step 5: Commit**

```bash
git add crates/emcore/src/emGUIFramework.rs
git commit -m "feat(emGUIFramework): add materialize_popup_surface for W3 deferred creation

Transitions a Pending popup emWindow to Materialized by creating the
winit Window + wgpu surface + compositor/tile cache. Silently cancels
if the popup was dropped before drain (strong_count == 1).

Caller wiring lands in the next commit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 4: Rewrite `emView::RawVisitAbs` popup branch + delete `PopupPlaceholder`

**Goal:** The observable change. `PopupPlaceholder` is gone; `PopupWindow: Option<Rc<RefCell<emWindow>>>`; popup entry constructs a real `emWindow` synchronously and enqueues surface materialization.

### Task 4: Change `PopupWindow` type and delete `PopupPlaceholder`

**Files:**
- Modify: `crates/emcore/src/emView.rs`

- [ ] **Step 1: Verify Phase-4 test state before change**

```bash
cargo-nextest ntr -p emcore test_phase4_popup_zoom_creates_popup_window
```
Expected: PASS (test currently passes against `PopupPlaceholder`). Record that it passes.

- [ ] **Step 2: Change `PopupWindow` field type**

At `crates/emcore/src/emView.rs:467`, change:
```rust
pub PopupWindow: Option<Rc<RefCell<PopupPlaceholder>>>,
```
to:
```rust
pub PopupWindow: Option<Rc<RefCell<crate::emWindow::emWindow>>>,
```

Note: pay attention to the existing import of `PopupPlaceholder` at the top of `emView.rs`. Remove it.

- [ ] **Step 3: Rewrite `RawVisitAbs` popup entry branch**

At `crates/emcore/src/emView.rs:1666-1710` (the current `if self.flags.contains(ViewFlags::POPUP_ZOOM) { ... if self.PopupWindow.is_none() { ... PopupPlaceholder::new_popup(...) ... }`), replace the popup-entry body with:

```rust
// emView.cpp:1628-1682: popup branch.
if self.flags.contains(ViewFlags::POPUP_ZOOM) {
    let outside_home = tree.GetRootPanel() != Some(vp)
        || vx < self.HomeX - 0.1
        || vx + vw > self.HomeX + self.HomeWidth + 0.1
        || vy < self.HomeY - 0.1
        || vy + vw * vp_h / self.HomePixelTallness > self.HomeY + self.HomeHeight + 0.1;

    if outside_home {
        if self.PopupWindow.is_none() {
            // C++ (emView.cpp:1638): wasFocused=Focused;
            let was_focused = self.window_focused;

            // C++ (emView.cpp:1639-1643): PopupWindow=new emWindow(...)
            // Rust: construct emWindow struct synchronously in Pending
            // state. OS surface materializes at next about_to_wait.
            let (close_sig, flags_sig, focus_sig, geom_sig) = {
                let mut sched = self.scheduler.as_ref()
                    .expect("scheduler must be wired before RawVisitAbs popup entry")
                    .borrow_mut();
                (sched.create_signal(), sched.create_signal(),
                 sched.create_signal(), sched.create_signal())
            };

            let popup = crate::emWindow::emWindow::new_popup_pending(
                tree.GetRootPanel().expect("root panel must exist"),
                crate::emWindow::WindowFlags::POPUP
                    | crate::emWindow::WindowFlags::UNDECORATED
                    | crate::emWindow::WindowFlags::AUTO_DELETE,
                "emViewPopup".to_string(),
                close_sig, flags_sig, focus_sig, geom_sig,
                self.background_color,
            );

            self.PopupWindow = Some(popup.clone());

            // C++ (emView.cpp:1642): UpdateEngine->AddWakeUpSignal(close_signal)
            if let Some(eng_id) = self.update_engine_id {
                if let Some(sched_weak) = self.scheduler.as_ref() {
                    sched_weak.borrow_mut().connect(close_sig, eng_id);
                }
            }

            // Enqueue deferred surface materialization.
            if let Some(fw_actions) = self.pending_framework_actions.as_ref() {
                let popup_for_closure = popup.clone();
                fw_actions.borrow_mut().push(Box::new(move |fw, el| {
                    fw.materialize_popup_surface(popup_for_closure, el);
                }));
            }

            // C++ (emView.cpp:1644): SwapViewPorts(true)
            self.SwapViewPorts(true);

            // C++ (emView.cpp:1645): if (wasFocused && !Focused) CurrentViewPort->RequestFocus()
            if was_focused && !self.window_focused {
                self.CurrentViewPort.borrow_mut().RequestFocus();
            }
        }

        // Geometry update — unchanged from current emView.rs popup branch.
        // [preserve the existing GetMaxPopupViewRect + SetViewPosSize block
        //  at lines ~1711-1745]
    } else if self.PopupWindow.is_some() {
        // [preserve existing exit/destroy block at lines ~1720+]
    }
}
```

Note on `self.pending_framework_actions`: this is a new back-channel from `emView` to `emGUIFramework`. It likely does not exist yet on `emView`. See Step 4.

- [ ] **Step 4: Wire `pending_framework_actions` back-channel**

This is the crux of the "how does `emView` enqueue into `emGUIFramework::pending_actions`" question.

**Approach:** `emGUIFramework` wraps `pending_actions` in `Rc<RefCell<Vec<DeferredAction>>>`, clones the `Rc` into every `emView` it owns via a setter at framework startup.

In `crates/emcore/src/emGUIFramework.rs`:
- Change `pending_actions: Vec<DeferredAction>` field to `pending_actions: Rc<RefCell<Vec<DeferredAction>>>`.
- Update the drain at line 322-325:
  ```rust
  let actions: Vec<DeferredAction> = self.pending_actions.borrow_mut().drain(..).collect();
  for action in actions {
      action(self, event_loop);
  }
  ```
- After window creation in `resumed` (or wherever home windows are created), call `win.borrow_mut().view_mut().set_pending_framework_actions(self.pending_actions.clone())`.

In `crates/emcore/src/emView.rs`:
- Add field: `pending_framework_actions: Option<Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>>>,`
- Add setter: `pub fn set_pending_framework_actions(&mut self, actions: Rc<RefCell<Vec<crate::emGUIFramework::DeferredAction>>>) { self.pending_framework_actions = Some(actions); }`
- Initialize field to `None` in `emView::new`.

Verify `DeferredAction` is `pub` (or `pub(crate)`) and exported from `emGUIFramework`. If not, make it so.

- [ ] **Step 5: Delete `PopupPlaceholder`**

Delete `crates/emcore/src/emView.rs` lines 1-70 (the `PopupPlaceholder` struct, its `impl` block, and any associated doc comments).

Remove any `use` of `PopupPlaceholder` remaining in the file.

Grep the whole crate for other references:
```bash
cargo check 2>&1 | grep -i PopupPlaceholder
```
Expected after edits: zero hits. If any remain, they're stale callers to delete.

- [ ] **Step 6: Verify compilation**

```bash
cargo check
```
Expected: compiles. Most likely errors: missing `scheduler` / `update_engine_id` / `background_color` field access on `emView` — these all currently exist on the struct; confirm names and adjust.

- [ ] **Step 7: Run the Phase-4 test**

```bash
cargo-nextest ntr -p emcore test_phase4_popup_zoom_creates_popup_window
```
Expected: PASS. The test asserts `v.PopupWindow.is_some()` after `RawVisit`, which holds because `new_popup_pending` populates synchronously.

If FAIL: most likely cause is missing `scheduler` wiring in the test setup — the new path requires `self.scheduler.as_ref()` to be `Some`. The test setup may need to wire a scheduler. Check `emView::new` call sites in the test and thread a scheduler through.

- [ ] **Step 8: Run full clippy + nextest**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: all 2410+ tests pass. If any popup-related test fails, diagnose: the most common cause at this step is a destruction-branch mismatch (Step 9 below).

- [ ] **Step 9: Audit destruction branch for symmetry**

Read `crates/emcore/src/emView.rs` popup-exit branch (around line 1720, the `else if self.PopupWindow.is_some()` clause). Current code likely does:
```rust
self.SwapViewPorts(true);
self.PopupWindow = None;
self.Signal(self.geometry_signal);
forceViewingUpdate = true;
```

Verify no action is needed — the destruction branch assigns `None`, which drops the `Rc`. If the popup was still `Pending`, the still-pending `DeferredAction` closure holds the last strong ref and the `strong_count == 1` check in `materialize_popup_surface` handles cancellation. If `Materialized`, the winit window gets dropped when the `Rc` drops; `self.windows` in the framework still holds it. Add a follow-up `DeferredAction` to remove from `self.windows`:

In the destruction branch, if the popup was materialized, enqueue a cleanup closure:
```rust
if let Some(popup) = self.PopupWindow.take() {
    if let Some(winit_window) = popup.borrow().winit_window_if_materialized().cloned() {
        let window_id = winit_window.id();
        if let Some(fw_actions) = self.pending_framework_actions.as_ref() {
            fw_actions.borrow_mut().push(Box::new(move |fw, _el| {
                fw.windows.remove(&window_id);
            }));
        }
    }
}
```

- [ ] **Step 10: Commit**

```bash
git add crates/emcore/src/emView.rs crates/emcore/src/emGUIFramework.rs
git commit -m "feat(emView): delete PopupPlaceholder; wire real popup via deferred materialization

RawVisitAbs popup-entry constructs an emWindow in OsSurface::Pending
synchronously and enqueues materialization into emGUIFramework's
pending_actions. PopupWindow field type reverts to
Option<Rc<RefCell<emWindow>>> — matches C++ emView::PopupWindow
signature exactly.

Clears 4 PHASE-6-FOLLOWUP markers (struct doc, new_popup,
SetViewPosSize, RawVisitAbs call site). The 5th marker at line 3611
(VIF-chain migration) is input-dispatch work unrelated to popup and
survives W3.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 5: New tests

**Goal:** Two new unit tests covering (a) the Pending→Materialized transition and (b) drop-before-materialize cancellation.

### Task 5: Write DISPLAY-gated materialization test

**Files:**
- Create: `crates/eaglemode/tests/unit/popup_materialization.rs`

- [ ] **Step 1: Check how other DISPLAY-gated tests in this crate are structured**

```bash
cat crates/eaglemode/tests/unit/popup_window.rs
```
Record the skip pattern and any helper functions used to construct a framework with a real event loop.

- [ ] **Step 2: Write the test**

```rust
//! W3: verify a popup constructed via `new_popup_pending` is materialized
//! into a real winit window on the next `about_to_wait` drain.

use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn popup_surface_materializes_on_about_to_wait() {
    if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
        eprintln!("skipping: no DISPLAY or WAYLAND_DISPLAY");
        return;
    }

    // Build framework with a home window.
    // [Use the same helpers popup_window.rs uses to construct a framework
    //  with a running event loop. If those helpers don't exist, construct
    //  an event loop via `winit::event_loop::EventLoop::new()`, then drive
    //  it manually through `resumed` → trigger popup entry → `about_to_wait`.]

    // Drive one full cycle that triggers popup entry:
    //   1. Set VF_POPUP_ZOOM on the view's flags.
    //   2. Call emView::Visit or RawVisit such that vx/vy is outside HomeX/Y/W/H.
    //   3. Immediately after: assert v.PopupWindow.is_some() (struct present).
    //   4. Drill into the PopupWindow's os_surface; assert it is Pending.
    //   5. Run one about_to_wait pass.
    //   6. Assert os_surface is now Materialized.
    //   7. Assert fw.windows contains the popup's WindowId.
}
```

Note: this test's driving harness is the most nontrivial part. The subagent implementing this task should spend the first 10-15 minutes reading existing DISPLAY-gated tests in the crate (especially `popup_window.rs` and any framework-driving tests) to find the idiomatic pattern. If no existing pattern drives a full `about_to_wait` cycle from a test, building that harness is this task's main work — flag it as potentially in-scope for a small helper in a test-support module.

- [ ] **Step 3: Run the test**

```bash
cargo-nextest ntr -p eaglemode popup_materialization
```
Expected on DISPLAY-capable host: PASS. On headless CI: skipped (prints "skipping" message).

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/unit/popup_materialization.rs
git commit -m "test(eaglemode): W3 popup surface materializes on about_to_wait

DISPLAY-gated unit test covering Pending → Materialized transition.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

### Task 6: Write non-gated cancellation test

**Files:**
- Create: `crates/eaglemode/tests/unit/popup_cancel_before_materialize.rs`

- [ ] **Step 1: Write the test**

```rust
//! W3: verify a popup dropped before its surface materializes cancels
//! cleanly. No winit Window is ever created; no panic.

#[test]
fn popup_dropped_before_materialize_cancels_cleanly() {
    // Does NOT require DISPLAY — cancellation short-circuits before any
    // winit call via the strong_count == 1 check.

    // Build framework with a home window (may need DISPLAY for the home
    // window; if so, gate this test too and document why).

    // Drive:
    //   1. Enter popup mode (VF_POPUP_ZOOM + Visit outside home).
    //      Assert PopupWindow is Some, Pending, pending_actions has 1 entry.
    //   2. Immediately exit popup mode (Visit back inside home, OR
    //      clear VF_POPUP_ZOOM and trigger an Update that calls RawVisitAbs).
    //      Assert PopupWindow is None after the Update.
    //   3. Run one about_to_wait pass. The deferred materialize closure
    //      runs, hits strong_count == 1, returns without creating a winit
    //      window.
    //   4. Assert fw.windows unchanged (same set as before popup entry),
    //      no panic occurred.
}
```

Note: if the home window itself requires DISPLAY (likely it does — `emGUIFramework::resumed` creates it with `&ActiveEventLoop`), this test becomes DISPLAY-gated as well. The cancellation *path* does not require DISPLAY, but the *test harness* does. Document this in a comment and use the same skip-if-no-display pattern.

- [ ] **Step 2: Run the test**

```bash
cargo-nextest ntr -p eaglemode popup_cancel_before_materialize
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/eaglemode/tests/unit/popup_cancel_before_materialize.rs
git commit -m "test(eaglemode): W3 popup dropped-before-materialize cancels cleanly

Verifies the strong_count == 1 cancellation path in
materialize_popup_surface.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 6: Cleanup `popup_window.rs` test

### Task 7: Remove dead DISPLAY gate, retarget function-pointer assertion

**Files:**
- Modify: `crates/eaglemode/tests/unit/popup_window.rs`

- [ ] **Step 1: Read current test**

```bash
cat crates/eaglemode/tests/unit/popup_window.rs
```
Record the `popup_window_creation_path_is_gated_on_display` test body.

- [ ] **Step 2: Remove the dead DISPLAY branch**

The test (per execution-debt §4.6) has a dead DISPLAY gate: both branches `eprintln!` and the function-pointer assertion run unconditionally. Simplify to just the reachability assertion. Retarget the function-pointer check from `emWindow::new_popup` (still present, still reachable) OR `emGUIFramework::materialize_popup_surface` (the new reachable entry point). Choose whichever the test was trying to assert — if the original intent was "popup creation path exists," `materialize_popup_surface` is now the more representative target.

Suggested body:
```rust
#[test]
fn popup_window_creation_path_is_gated_on_display() {
    // Assert the popup materialization function pointer is reachable.
    // The DISPLAY gate was dead in the pre-W3 version; simplifying to
    // the one assertion that was actually doing work.
    let _: fn(&mut crate::emGUIFramework::emGUIFramework,
              std::rc::Rc<std::cell::RefCell<crate::emWindow::emWindow>>,
              &winit::event_loop::ActiveEventLoop)
        = crate::emGUIFramework::emGUIFramework::materialize_popup_surface;
}
```

Adjust fully-qualified paths to match the actual `use` structure of the test file.

- [ ] **Step 3: Run test**

```bash
cargo-nextest ntr -p eaglemode popup_window
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/eaglemode/tests/unit/popup_window.rs
git commit -m "test(eaglemode): remove dead DISPLAY gate; retarget function-pointer assertion

Pre-W3 version had both gate branches running unconditionally. Retarget
to the new materialize_popup_surface entry point.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Phase 7: Acceptance gate

### Task 8: Marker audit + smoke test + full verification

**Files:** none modified

- [ ] **Step 1: Confirm exactly one PHASE-6-FOLLOWUP marker remains**

```bash
grep -rn "PHASE-6-FOLLOWUP" crates/
```
Expected output: exactly one line —
```
crates/emcore/src/emView.rs:3611:    /// PHASE-6-FOLLOWUP: migrate the VIF-chain + panel-broadcast dispatch
```

If more than one line is returned: investigate and clean up the unexpected survivors. They should all have been handled in Phase 4.

If zero lines are returned: even better (the VIF-chain marker may have been consolidated into a clearer comment); note the change in the commit message.

- [ ] **Step 2: Confirm `PopupPlaceholder` has no remaining references**

```bash
grep -rn "PopupPlaceholder" crates/
```
Expected: zero lines.

- [ ] **Step 3: Run full test suite and clippy**

```bash
cargo clippy -- -D warnings
cargo-nextest ntr
```
Expected: clippy clean; 2411+ tests pass (2409 baseline + 2 new W3 tests, minus the `popup_window.rs` simplification which may stay at 2 or collapse to 1).

- [ ] **Step 4: Run smoke test**

```bash
timeout 20 cargo run --release --bin eaglemode
echo "exit=$?"
```
Expected: `exit=124` or `exit=143`. Both indicate the program stayed alive through 20 seconds.

- [ ] **Step 5: Run golden test baseline**

```bash
cargo test --test golden -- --test-threads=1
```
Expected: 237 passed / 6 failed (same as pre-W3 baseline). No new regressions.

- [ ] **Step 6: Final summary**

Write a short close-out noting:
- Number of PHASE-6-FOLLOWUP markers cleared (4) and remaining (1, VIF-chain, out of scope).
- Nextest count before/after.
- Golden count before/after.
- Any scope expansions encountered.
- Any items deferred for follow-up.

Append the close-out to `docs/superpowers/notes/` with filename `YYYY-MM-DD-w3-popup-architecture-closeout.md`. Commit it separately:

```bash
git add docs/superpowers/notes/YYYY-MM-DD-w3-popup-architecture-closeout.md
git commit -m "docs: W3 popup architecture close-out notes

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Risk register

| # | Risk | Mitigation |
|---|---|---|
| R1 | `emWindow` field-access refactor (Phase 1, Step 5) misses a call site → runtime panic on `materialized()` | `cargo check` + full nextest must pass at end of Phase 1. Panic is compile-checked at pattern-A sites; pattern-B sites rely on caller discipline. |
| R2 | `pending_framework_actions` back-channel plumbing is invasive (Phase 4, Step 4) — touches `emView::new` call sites | Keep the field `Option<Rc<RefCell<...>>>` and `None` by default. Tests that don't drive the framework lifecycle leave it `None`; the `if let Some(...)` guards keep behavior stable. |
| R3 | Phase-4 test requires scheduler wiring that wasn't required before | If the test fails at Phase 4 Step 7, extend the test setup to wire a real `emScheduler` via `emView::set_scheduler` (or equivalent). Do not weaken the assertion — the synchronous `PopupWindow.is_some()` is the load-bearing invariant. |
| R4 | `materialize_popup_surface` creates a winit window during `about_to_wait` → inserting into `self.windows` while the framework iterates `self.windows.values()` at line 395 | The `pending_actions` drain runs at line 322, **before** the window-iteration loop at line 395. Materialization is complete before iteration starts on the same tick. Confirm this ordering is preserved in Phase 4. |
| R5 | Dropping the popup while `Materialized` — winit window destruction timing | Destruction on Rc drop is fine for winit; the Step 9 deferred-removal closure in Phase 4 ensures `self.windows` doesn't hold a dangling entry. |
| R6 | First-frame popup paint timing — user perception | Acknowledged design concession (~16.7 ms). No mitigation required; matches the spec's Section 2. |

---

## Self-review notes

Spec coverage check:

| Spec section | Implementing task(s) |
|---|---|
| §Architecture — OsSurface split | Task 1 |
| §Architecture — PopupWindow type reverts | Task 4 Step 2 |
| §Architecture — materialize_popup_surface | Task 3 |
| §Architecture — pending_actions reuse | Task 3, Task 4 Step 4 |
| §Lifecycle — Creation | Task 4 Step 3 |
| §Lifecycle — Destruction symmetry | Task 4 Step 9 |
| §Lifecycle — Drop-while-pending cancellation | Task 3 (strong_count check), Task 6 (test) |
| §Data model — OsSurface enum | Task 1 Step 1 |
| §Data model — method behavior while Pending | Task 1 Step 5 (Pattern B) |
| §Data model — new_popup_pending | Task 2 |
| §Data model — PopupPlaceholder deletion | Task 4 Step 5 |
| §Testing — Phase-4 test unchanged | Task 4 Step 7 verification |
| §Testing — popup_window.rs cleanup | Task 7 |
| §Testing — materialization test | Task 5 |
| §Testing — cancellation test | Task 6 |
| §Acceptance — marker audit | Task 8 Step 1 |
| §Acceptance — smoke + golden | Task 8 Steps 4, 5 |
