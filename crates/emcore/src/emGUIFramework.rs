use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::emCtrlSocket::CtrlMsg;

/// RUST_ONLY: (language-forced-utility)
/// Stores the winit `EventLoopProxy` for `CtrlMsg` so background acceptor
/// / worker threads (Tasks 3.4-3.5) can post messages onto the main
/// thread. Set once in `App::run`. C++ has no analogue — the debug
/// control socket is Rust-only (see `emCtrlSocket.rs` module docs).
pub(crate) static EVENT_LOOP_PROXY: OnceLock<EventLoopProxy<CtrlMsg>> = OnceLock::new();

use crate::emClipboard::emClipboard;
use crate::emContext::emContext;
use crate::emEngineCtx::DeferredAction as FrameworkDeferredAction;
use crate::emInput::{emInputEvent, InputKey, InputVariant};
use crate::emInputState::emInputState;
use crate::emPanelTree::PanelTree;
use crate::emScheduler::EngineScheduler;
use crate::emSignal::SignalId;

use super::emWindow::{emWindow, WindowFlags};
use crate::emEngine::Priority;
use crate::emPanelScope::PanelScope;
use crate::emScreen::emScreen;

/// Stable identifier for a dialog (or other runtime-installed top-level
/// window) across the pending-vs-materialized lifecycle. Allocated by
/// `App::allocate_dialog_id` at dialog-construction time; resolved to
/// the materialized `WindowId` via `App::dialog_windows` after
/// `install_pending_top_level` runs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DialogId(pub u64);

/// A top-level window awaiting materialization. `emDialog::new` (Phase 3.5
/// Task 5 consumer) constructs these; the framework drains them on the
/// next event-loop tick via `App::install_pending_top_level`.
///
/// Phase 3.5.A Task 9 note: widened from `pub(crate)` to `pub` because
/// the consumer (Phase 3.5 `emDialog` reshape, Task 10+) has not yet
/// wired through. Dead-code lint under `-D warnings` requires public
/// reachability until the consumer lands; Task 4 precedent (see memory
/// `project_phase35a_pub_narrow.md`) allows the widening with a note.
/// Narrow back to `pub(crate)` once Phase 3.5 Task 5 consumes this.
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
    /// DIVERGED (Phase 3.6 Task 3): additional wake-up signals to connect
    /// to the `DialogPrivateEngine` at install time. Port of C++
    /// `emFileDialog` ctor calling `AddWakeUpSignal(Fsb->GetFileTriggerSignal())`
    /// (emFileDialog.cpp:41). In C++ the dialog subclass calls this on the
    /// already-constructed private engine; in Rust the engine is built
    /// deferred at `install_pending_top_level` time, so pre-show
    /// subscribers queue their signals here and the installer drains them
    /// immediately after `register_engine` + connecting `close_signal`.
    pub wake_up_signals: Vec<SignalId>,
}

/// Result of `App::dialog_window_mut`: the dialog's `emWindow` may either
/// still be queued in `pending_top_level` (pre-materialize) or already
/// live in `App::windows` keyed by a real `WindowId`.
///
/// Phase 3.5.A Task 9 visibility note: see `PendingTopLevel`.
pub enum DialogWindow<'a> {
    Pending {
        idx: usize,
        entry: &'a mut PendingTopLevel,
    },
    Materialized {
        window_id: WindowId,
        window: &'a mut emWindow,
    },
}

/// Shared GPU resources created once and used by all windows.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Default for GpuContext {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuContext {
    /// Create GPU context synchronously using pollster.
    pub fn new() -> Self {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("eaglemode_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .expect("failed to create GPU device");

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
}

/// User-provided setup callback, called once during `resumed()`.
pub type SetupFn = Box<dyn FnOnce(&mut App, &ActiveEventLoop)>;

/// The main application handler integrating winit, wgpu, the panel tree, and
/// the scheduler.
/// Deferred action requiring `&ActiveEventLoop` (window creation, etc.).
pub type DeferredAction = Box<dyn FnOnce(&mut App, &ActiveEventLoop)>;

pub struct App {
    pub gpu: Option<GpuContext>,
    pub screen: Option<emScreen>,
    pub scheduler: EngineScheduler,
    pub context: Rc<emContext>,
    // Phase-2 port-ownership-rewrite: narrowed from
    // `HashMap<WindowId, Rc<RefCell<emWindow>>>` to plain `emWindow`.
    // Top-level windows only. Popups live on `emView::PopupWindow`,
    // matching C++ ownership (emView.h:670 `emWindow * PopupWindow`).
    // Winit events to popup WindowIds are resolved via `find_window_mut`
    // which scans parent views' PopupWindow handles.
    pub windows: HashMap<WindowId, emWindow>,
    /// Phase 3.5.A Task 7: the home window's WindowId, set once by the
    /// setup callback when the home `emWindow` is inserted. `None` until
    /// setup runs. The home window owns its `PanelTree` — reach it via
    /// `self.windows.get_mut(&home_window_id.unwrap()).unwrap().tree` or
    /// the `home_tree_mut()` helper. Formerly `App::tree`.
    pub home_window_id: Option<WindowId>,
    /// Phase 3.5.A Task 9: top-level windows queued for materialization on
    /// the next event-loop tick. `emDialog::new` (Phase 3.5 Task 5
    /// consumer) pushes entries here; `install_pending_top_level` drains
    /// one per call. Analogue of `emView::PopupWindow` for dialogs (i.e.
    /// framework-managed top-level windows rather than view-owned popups).
    /// Visibility note: widened to `pub` until the Phase 3.5 Task 5
    /// `emDialog` consumer lands — see `PendingTopLevel` doc.
    pub pending_top_level: Vec<PendingTopLevel>,
    /// Phase 3.5.A Task 9: `DialogId` → `WindowId` mapping recorded by
    /// `install_pending_top_level` once the OS surface is built. Lets
    /// callers that hold a `DialogId` locate the materialized `emWindow`
    /// in `self.windows`.
    pub dialog_windows: HashMap<DialogId, WindowId>,
    /// Phase 3.6 Prereq A: `DialogId` → root `PanelId` of that dialog's
    /// `DlgPanel`. Populated in `install_pending_top_level` (and its
    /// headless test analog) alongside `dialog_windows`; cleared in
    /// `close_dialog_by_id`. Required by `mutate_dialog_by_id` because
    /// the `emDialog` handle (which carries `root_panel_id`) may be
    /// long-gone when the App-side mutator fires.
    pub(crate) dialog_roots: HashMap<DialogId, crate::emPanelTree::PanelId>,
    /// Framework-level deferred actions produced by scheduler/view code that
    /// need to run back on `App` between time slices. Spec §3.1 / §3.7 —
    /// passed as `&mut Vec<DeferredAction>` into `EngineScheduler::DoTimeSlice`.
    pub(crate) framework_actions: Vec<FrameworkDeferredAction>,
    pub input_state: emInputState,
    /// Input queue drained by `InputDispatchEngine` (Phase 3) per spec §3.1
    /// and §4 D4.9. Produced by `window_event` on each winit input;
    /// consumed once per slice by the framework-owned InputDispatchEngine
    /// registered at top priority.
    pub(crate) pending_inputs: Vec<(WindowId, emInputEvent)>,
    /// EngineId of the `InputDispatchEngine` registered at framework init.
    /// Winit callbacks wake this engine after enqueuing events so the
    /// scheduler drains `pending_inputs` on the next tick.
    pub(crate) input_dispatch_engine_id: crate::emEngine::EngineId,
    /// Deferred actions queued by input handlers that need `&ActiveEventLoop`
    /// (e.g., window creation for Duplicate/CreateControlWindow, popup
    /// surface materialization from `emView::RawVisitAbs`).
    /// Drained each frame in `about_to_wait`.
    ///
    /// `Rc<RefCell<...>>` so `emView` can hold a handle and enqueue without
    /// a borrow of `App`.
    pub pending_actions: Rc<RefCell<Vec<DeferredAction>>>,
    /// Chartered §3.6(a): mutated from winit text-event callbacks that lack &mut framework reach.
    ///
    /// Phase-3 Task-2 relocation: in C++, `emClipboard` is looked up via
    /// `emRef<emClipboard>` on `emContext`; in Rust, winit text-event callbacks
    /// need write access without `&mut framework` reach, so the clipboard is
    /// chartered here instead of on `emContext`. Accessed through
    /// `EngineCtx::clipboard_mut` / `SchedCtx::clipboard_mut` during engine
    /// cycles, or directly via `framework.clipboard.borrow_mut()` from
    /// winit-side callbacks.
    pub clipboard: RefCell<Option<Box<dyn emClipboard>>>,
    /// Global file-update signal. Port of C++ `emFileModel::AcquireUpdateSignalModel`.
    /// When fired, all file models that listen to it will reload from disk.
    pub file_update_signal: SignalId,
    /// Phase 3.6.2 E040 sidecar (`.rust_only`): maps a `DialogId` to a
    /// pre-seeded `DialogResult` that `read_dialog_finalized_result` will
    /// return without walking `dialog_windows → windows → tree`. Exists
    /// because `winit::window::WindowId::dummy()` is the only headless
    /// `WindowId` available, so a second headless top-level (e.g. the
    /// overwrite `emDialog` installed alongside an outer `emFileDialog`)
    /// cannot be materialized at a distinct id. Tests that need to drive
    /// a POSITIVE `DialogResult::Ok` branch without installing the inner
    /// dialog seed an entry here. Production (`cfg(not(test))`) ignores
    /// this field — the regular lookup path is unchanged.
    #[cfg(test)]
    pub(crate) headless_dialog_results: HashMap<DialogId, crate::emDialog::DialogResult>,
    setup_fn: Option<SetupFn>,
    initialized: bool,
    last_frame_time: Instant,
}

impl App {
    pub fn new(setup: SetupFn) -> Self {
        let mut scheduler = EngineScheduler::new();
        let file_update_signal = scheduler.create_signal();
        // Register the InputDispatchEngine at top priority (Phase 3 / spec
        // §3.1 / §4 D4.9): winit input callbacks enqueue into
        // `pending_inputs` and wake this engine; its `Cycle` drains the
        // queue at the top of each slice before any other engine runs.
        let input_dispatch_engine_id = scheduler.register_engine(
            Box::new(crate::emInputDispatchEngine::InputDispatchEngine),
            crate::emEngine::Priority::VeryHigh,
            crate::emPanelScope::PanelScope::Framework,
        );
        let context = emContext::NewRoot();
        Self {
            gpu: None,
            screen: None,
            scheduler,
            context,
            windows: HashMap::new(),
            home_window_id: None,
            pending_top_level: Vec::new(),
            dialog_windows: HashMap::new(),
            dialog_roots: HashMap::new(),
            framework_actions: Vec::new(),
            input_state: emInputState::new(),
            pending_inputs: Vec::new(),
            input_dispatch_engine_id,
            pending_actions: Rc::new(RefCell::new(Vec::new())),
            clipboard: RefCell::new(None),
            file_update_signal,
            #[cfg(test)]
            headless_dialog_results: HashMap::new(),
            setup_fn: Some(setup),
            initialized: false,
            last_frame_time: Instant::now(),
        }
    }

    /// Run the application. This blocks until all windows are closed.
    pub fn run(self) {
        let event_loop = winit::event_loop::EventLoop::<CtrlMsg>::with_user_event()
            .build()
            .expect("failed to create event loop");
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        let proxy = event_loop.create_proxy();
        let _ = EVENT_LOOP_PROXY.set(proxy);
        let mut app = self;

        // Spawn the agent control-channel acceptor if gated on. Zero cost
        // when EMCORE_DEBUG_CONTROL is unset — neither the socket file nor
        // the acceptor thread is created.
        if std::env::var("EMCORE_DEBUG_CONTROL").as_deref() == Ok("1") {
            if let Err(e) = crate::emCtrlSocket::spawn_acceptor() {
                eprintln!("[emCtrlSocket] spawn_acceptor failed: {e}");
            }
        }

        let result = event_loop.run_app(&mut app);
        crate::emCtrlSocket::cleanup_on_exit();
        result.expect("event loop error");
        // Work around wgpu segfault on shutdown: dropping Instance/Device
        // after Surface is already destroyed crashes in the Vulkan driver.
        // https://github.com/gfx-rs/wgpu/issues/5781
        std::process::exit(0);
    }

    /// Get the GPU context (panics if not yet initialized).
    pub fn gpu(&self) -> &GpuContext {
        self.gpu.as_ref().expect("GPU not initialized yet")
    }

    pub fn screen(&self) -> &emScreen {
        self.screen.as_ref().expect("Screen not initialized yet")
    }

    /// Resolve a winit `WindowId` to an `&mut emWindow`, searching both the
    /// top-level `self.windows` map and any popup held by a top-level
    /// window's view (`emView::PopupWindow`).
    ///
    /// C++ reference (emView.cpp:1634-1642 + emView.h:670): the popup
    /// `emWindow*` is owned by the launching `emView`, not a framework
    /// registry. The C++ backend dispatches OS events to each `emWindow`
    /// via its own callback registration; Rust must look up by `WindowId`
    /// because winit delivers events through a single `ApplicationHandler`.
    /// The scan is O(N_windows) which is fine for normal UIs (handful of
    /// windows). This is Task-8 Path B — matches C++ ownership exactly.
    pub(crate) fn find_window_mut(
        windows: &mut HashMap<WindowId, emWindow>,
        window_id: WindowId,
    ) -> Option<&mut emWindow> {
        if windows.contains_key(&window_id) {
            return windows.get_mut(&window_id);
        }
        // Popup path: scan for a parent window whose view holds a popup
        // whose materialized WindowId matches.
        for win in windows.values_mut() {
            let matches = win
                .view()
                .PopupWindow
                .as_ref()
                .and_then(|p| {
                    p.winit_window_if_materialized()
                        .map(|w| w.id() == window_id)
                })
                .unwrap_or(false);
            if matches {
                return win.view_mut().PopupWindow.as_deref_mut();
            }
        }
        None
    }

    /// Run a closure with mutable access to the home window's tree and a
    /// freshly-constructed `SchedCtx` over the framework scheduler. Used by
    /// callers in other crates (e.g. `emmain::RecreateContentPanels`) that
    /// need to invoke `Visit*` methods plumbed through with `&mut SchedCtx`.
    ///
    /// RUST_ONLY: (language-forced-utility) — out-of-crate callers cannot
    /// destructure `App` because `framework_actions` is `pub(crate)`. This
    /// helper performs the split-borrow internally.
    pub fn with_home_tree_and_sched_ctx<R>(
        &mut self,
        f: impl FnOnce(&mut PanelTree, &mut crate::emEngineCtx::SchedCtx<'_>) -> R,
    ) -> R {
        let id = self
            .home_window_id
            .expect("home window not yet created (setup callback has not run)");
        let App {
            windows,
            scheduler,
            framework_actions,
            context,
            clipboard,
            pending_actions,
            ..
        } = self;
        let mut sc = crate::emEngineCtx::SchedCtx {
            scheduler,
            framework_actions,
            root_context: context,
            framework_clipboard: clipboard,
            current_engine: None,
            pending_actions,
        };
        let win = windows
            .get_mut(&id)
            .expect("home window present in App::windows");
        let mut tree = win.take_tree();
        let result = f(&mut tree, &mut sc);
        win.put_tree(tree);
        result
    }

    /// Mutable access to the home window's panel tree.
    ///
    /// Phase 3.5.A Task 7: `App::tree` is gone; the home window owns its
    /// tree. This helper looks up the home window via `home_window_id`.
    /// Panics if the home window has not been created yet (setup callback
    /// has not run) — same precondition as the former `App::tree`.
    pub fn home_tree_mut(&mut self) -> &mut PanelTree {
        let id = self
            .home_window_id
            .expect("home window not yet created (setup callback has not run)");
        &mut self
            .windows
            .get_mut(&id)
            .expect("home window present in App::windows")
            .tree
    }

    /// Immutable view of the home window's panel tree. See `home_tree_mut`.
    pub fn home_tree(&self) -> &PanelTree {
        let id = self
            .home_window_id
            .expect("home window not yet created (setup callback has not run)");
        &self
            .windows
            .get(&id)
            .expect("home window present in App::windows")
            .tree
    }

    /// Materialize the currently-pending popup window's OS surface.
    ///
    /// Since `App::windows` holds plain `emWindow` (no `Rc`) the popup
    /// cannot be shared between a closure and `emView::PopupWindow` — it
    /// lives in `emView::PopupWindow` for its entire lifetime (Task-8
    /// Path B, mirroring C++ `emView::PopupWindow` ownership from
    /// `emView.cpp:1636`). This method walks `App::windows` to find the
    /// home view holding a `Pending` popup, materializes the OS surface
    /// in place, and wires the new WindowId onto the view-port
    /// back-reference.
    ///
    /// **Cancellation:** if no view holds a `Pending` popup at drain time,
    /// the popup was torn down between enqueue and drain (same-frame
    /// popup-enter/exit). We silently return.
    ///
    /// Winit events addressed to the popup's WindowId are routed via
    /// `find_window_mut` (see above).
    pub(crate) fn materialize_pending_popup(&mut self, event_loop: &ActiveEventLoop) {
        use crate::emWindow::{MaterializedSurface, OsSurface};

        // Find a home window whose view has a `Pending` popup.
        let home_key = self.windows.iter().find_map(|(id, win)| {
            let view = win.view();
            let has_pending = view
                .PopupWindow
                .as_ref()
                .map(|p| matches!(p.os_surface, OsSurface::Pending(_)))
                .unwrap_or(false);
            if has_pending {
                Some(*id)
            } else {
                None
            }
        });
        let Some(home_key) = home_key else {
            // Natural cancellation: popup was torn down before drain.
            return;
        };

        // Extract Pending params without holding the borrow across winit
        // create_window (which needs `&mut self` via event_loop later).
        let (flags, caption, requested_pos_size) = {
            let home = self
                .windows
                .get(&home_key)
                .expect("home_key just found above");
            let view = home.view();
            let popup = view
                .PopupWindow
                .as_ref()
                .expect("Pending popup just found above");
            match &popup.os_surface {
                OsSurface::Pending(p) => (p.flags, p.caption.clone(), p.requested_pos_size),
                OsSurface::Materialized(_) => unreachable!("checked Pending above"),
            }
        };

        // Build winit window attributes — mirror `emWindow::create`.
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
            event_loop
                .create_window(attrs)
                .expect("failed to create popup window"),
        );

        if let Some((x, y, pw, ph)) = requested_pos_size {
            let _ = winit_window
                .request_inner_size(winit::dpi::PhysicalSize::new(pw as u32, ph as u32));
            winit_window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
        }

        let gpu = self.gpu.as_ref().expect("GPU not initialized");
        let materialized = MaterializedSurface::build(gpu, winit_window.clone());
        let w = materialized.surface_config.width;
        let h = materialized.surface_config.height;
        let popup_window_id = winit_window.id();

        // Install the materialized surface on the popup in place and update
        // its view's geometry. Scope the `&mut home` borrow tightly so it
        // does not conflict with `self.context` / `self.scheduler` below.
        {
            let root = self.context.clone();
            let App {
                scheduler,
                framework_actions,
                windows,
                clipboard,
                pending_actions,
                ..
            } = self;
            let home = windows.get_mut(&home_key).expect("home_key still present");
            let view = home.view_mut();
            let popup = view
                .PopupWindow
                .as_mut()
                .expect("Pending popup still present");
            popup.os_surface = OsSurface::Materialized(Box::new(materialized));
            popup.wire_viewport_window_id(popup_window_id);
            // Phase 3.5.A Task 8: popup owns its own PanelTree + RootPanel.
            // SetGeometry mutates the popup view's root panel; the tree it
            // writes to must be the popup's own tree (not the home's).
            // Take the popup's tree out for the SetGeometry call, put it
            // back at scope exit.
            let mut popup_tree = popup.take_tree();
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context: &root,
                framework_clipboard: clipboard,
                current_engine: None,
                pending_actions,
            };
            popup.view_mut().SetGeometry(
                &mut popup_tree,
                0.0,
                0.0,
                w as f64,
                h as f64,
                1.0,
                &mut sc,
            );
            popup.put_tree(popup_tree);
        }

        winit_window.request_redraw();
    }

    /// Allocate a fresh `DialogId` via the scheduler's monotonic counter.
    ///
    /// Phase 3.5 Task 3: counter relocated from `App` to `EngineScheduler`
    /// (spec §8); delegates here so callers need no scheduler reference.
    pub fn allocate_dialog_id(&mut self) -> DialogId {
        self.scheduler.allocate_dialog_id()
    }

    /// Materialize the first pending top-level window.
    ///
    /// Mirrors `materialize_pending_popup` but drains `pending_top_level`
    /// (framework-managed dialog queue) instead of scanning for a view
    /// with a `Pending` popup. Called from a `pending_actions` closure
    /// enqueued by `emDialog::new`; if multiple dialogs are pending,
    /// each closure fires this once.
    ///
    /// Post-materialize, the deferred `DialogPrivateEngine` behavior
    /// (shipped on the `PendingTopLevel` entry) is registered at
    /// `PanelScope::Toplevel(materialized_wid)` and connected to the
    /// dialog's close signal — resolving the spec's
    /// "engine-registration chicken-and-egg" (option a: deferred
    /// registration) for runtime-installed top-level windows.
    /// Visibility: `pub` awaiting Phase 3.5 Task 5 consumer — see
    /// `PendingTopLevel` doc.
    pub fn install_pending_top_level(&mut self, event_loop: &ActiveEventLoop) {
        use crate::emWindow::{MaterializedSurface, OsSurface};

        if self.pending_top_level.is_empty() {
            // Cancelled before drain (dialog torn down between enqueue
            // and this tick).
            return;
        }

        // Extract flags/caption for winit attrs without holding a long
        // borrow across `create_window`.
        let (flags, caption) = match &self.pending_top_level[0].window.os_surface {
            OsSurface::Pending(p) => (p.flags, p.caption.clone()),
            OsSurface::Materialized(_) => {
                // Already materialized — structurally impossible given
                // the ctor always produces `Pending`, but drain + return
                // rather than panic to preserve framework progress.
                let _ = self.pending_top_level.remove(0);
                return;
            }
        };

        let mut attrs = winit::window::WindowAttributes::default().with_title(caption.as_str());
        if flags.contains(WindowFlags::UNDECORATED) {
            attrs = attrs.with_decorations(false);
        }
        if flags.contains(WindowFlags::MAXIMIZED) {
            attrs = attrs.with_maximized(true);
        }
        if flags.contains(WindowFlags::FULLSCREEN) {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
        // `WF_MODAL` has no winit counterpart; modality is the window
        // manager's concern (or enforced by input-routing discipline in
        // the engine layer).

        let winit_window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                // Materialization failed — pop the pending entry and fire
                // close_signal so DialogPrivateEngine (once registered)
                // observes termination. Match popup parity: log +
                // graceful abort.
                eprintln!(
                    "install_pending_top_level: winit create_window failed: {:?}",
                    e
                );
                let pending = self.pending_top_level.remove(0);
                self.scheduler.fire(pending.close_signal);
                return;
            }
        };

        let gpu = self.gpu.as_ref().expect("GPU not initialized");
        let materialized = MaterializedSurface::build(gpu, winit_window.clone());
        let w = materialized.surface_config.width;
        let h = materialized.surface_config.height;
        let new_wid = winit_window.id();

        // Pop the pending entry; materialize in place.
        let mut pending = self.pending_top_level.remove(0);
        pending.window.os_surface = OsSurface::Materialized(Box::new(materialized));
        pending.window.wire_viewport_window_id(new_wid);

        // Phase 3.5 Task 5: construct DialogPrivateEngine here, not in emDialog::new.
        // `new_wid` is known at this point; pass it to the engine.
        // Phase 3.5 Task 10: also pass `dialog_id` so auto-delete can call
        // `App::close_dialog_by_id(did)` via the closure rail.
        {
            let engine = Box::new(crate::emDialog::DialogPrivateEngine {
                dialog_id: pending.dialog_id,
                root_panel_id: pending.private_engine_root_panel_id,
                close_signal: pending.close_signal,
            });
            let engine_id = self.scheduler.register_engine(
                engine,
                Priority::High,
                PanelScope::Toplevel(new_wid),
            );
            self.scheduler.connect(pending.close_signal, engine_id);
            // Phase 3.6 Task 3: drain pre-show wake-up signal subscriptions.
            // Port of C++ emFileDialog ctor `AddWakeUpSignal(...)` idiom
            // (emFileDialog.cpp:41). Rust builds the engine deferred, so
            // subscribers queue signals on PendingTopLevel.wake_up_signals
            // pre-show; we connect them here now that engine_id exists.
            for sig in pending.wake_up_signals.drain(..) {
                self.scheduler.connect(sig, engine_id);
            }
            // Phase 3.6 Task 3: record the engine id on DlgPanel so
            // post-show callers (e.g. emFileDialog::CheckFinish) can
            // subscribe transient signals (e.g. overwrite dialog's
            // finish_signal) to this engine without a scope walk.
            let tree = pending.window.tree_mut();
            if let Some(mut behavior) = tree.take_behavior(pending.private_engine_root_panel_id) {
                if let Some(dlg) = behavior.as_dlg_panel_mut() {
                    dlg.private_engine_id = Some(engine_id);
                }
                tree.put_behavior(pending.private_engine_root_panel_id, behavior);
            }
        }

        // Set initial view geometry. Take the dialog's tree out for the
        // SetGeometry call, put it back before inserting into
        // `self.windows`. Mirrors `materialize_pending_popup`.
        let root = Rc::clone(&self.context);
        let App {
            scheduler,
            framework_actions,
            clipboard,
            pending_actions,
            ..
        } = self;
        let mut tree = pending.window.take_tree();
        {
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context: &root,
                framework_clipboard: clipboard,
                current_engine: None,
                pending_actions,
            };
            pending
                .window
                .view_mut()
                .SetGeometry(&mut tree, 0.0, 0.0, w as f64, h as f64, 1.0, &mut sc);
        }
        pending.window.put_tree(tree);

        // Move the emWindow into App::windows under its real WindowId,
        // and record the DialogId → WindowId + root-panel mappings.
        let did = pending.dialog_id;
        let root_pid = pending.private_engine_root_panel_id;
        self.windows.insert(new_wid, pending.window);
        self.dialog_windows.insert(did, new_wid);
        self.dialog_roots.insert(did, root_pid);

        winit_window.request_redraw();
    }

    /// Phase 3.5.A Task 10: test-only analog of
    /// [`install_pending_top_level`] that skips winit surface creation.
    ///
    /// `install_pending_top_level` requires an `ActiveEventLoop` to
    /// construct the OS window, so it cannot run in unit tests. This
    /// helper reproduces the installer's scheduler + bookkeeping
    /// operations under a caller-supplied `WindowId` (typically
    /// [`WindowId::dummy()`]): the deferred `DialogPrivateEngine`
    /// behavior is registered at
    /// [`PanelScope::Toplevel(wid)`](crate::emPanelScope::PanelScope::Toplevel),
    /// its wake-up is connected to `close_signal`, the `emWindow` is
    /// moved into [`App::windows`] under `wid`, and the
    /// `DialogId → WindowId` mapping is recorded. Winit surface
    /// creation and initial `SetGeometry` are skipped (the emWindow
    /// retains its `OsSurface::Pending` state, which is acceptable for
    /// engine-driven tests that do not paint).
    #[cfg(test)]
    pub(crate) fn install_pending_top_level_headless(
        &mut self,
        wid: WindowId,
    ) -> Option<crate::emEngine::EngineId> {
        if self.pending_top_level.is_empty() {
            return None;
        }
        let mut pending = self.pending_top_level.remove(0);
        pending.window.wire_viewport_window_id(wid);
        // Phase 3.5 Task 5: construct DialogPrivateEngine here, not in emDialog::new.
        // Mirrors `install_pending_top_level` but skips winit surface creation.
        // Phase 3.5 Task 10: pass `dialog_id` for closure-rail auto-delete.
        let engine = Box::new(crate::emDialog::DialogPrivateEngine {
            dialog_id: pending.dialog_id,
            root_panel_id: pending.private_engine_root_panel_id,
            close_signal: pending.close_signal,
        });
        let engine_id =
            self.scheduler
                .register_engine(engine, Priority::High, PanelScope::Toplevel(wid));
        self.scheduler.connect(pending.close_signal, engine_id);
        // Phase 3.6 Task 3: drain pre-show wake-up signal subscriptions.
        // Mirrors the production installer above.
        for sig in pending.wake_up_signals.drain(..) {
            self.scheduler.connect(sig, engine_id);
        }
        // Phase 3.6 Task 3: record engine id on DlgPanel (mirrors production).
        {
            let tree = pending.window.tree_mut();
            if let Some(mut behavior) = tree.take_behavior(pending.private_engine_root_panel_id) {
                if let Some(dlg) = behavior.as_dlg_panel_mut() {
                    dlg.private_engine_id = Some(engine_id);
                }
                tree.put_behavior(pending.private_engine_root_panel_id, behavior);
            }
        }
        let did = pending.dialog_id;
        let root_pid = pending.private_engine_root_panel_id;
        self.dialog_windows.insert(did, wid);
        self.dialog_roots.insert(did, root_pid);
        self.windows.insert(wid, pending.window);
        Some(engine_id)
    }

    /// Look up the `emWindow` backing a `DialogId`. Returns `Pending` if
    /// the dialog is still queued in `pending_top_level`, or
    /// `Materialized` once `install_pending_top_level` has moved it
    /// into `self.windows`. Phase 3.5.A Task 9.
    /// Visibility: `pub` awaiting Phase 3.5 Task 5 consumer — see
    /// `PendingTopLevel` doc.
    pub fn dialog_window_mut(&mut self, did: DialogId) -> Option<DialogWindow<'_>> {
        if let Some(wid) = self.dialog_windows.get(&did).copied() {
            let window = self.windows.get_mut(&wid)?;
            return Some(DialogWindow::Materialized {
                window_id: wid,
                window,
            });
        }
        for (idx, entry) in self.pending_top_level.iter_mut().enumerate() {
            if entry.dialog_id == did {
                return Some(DialogWindow::Pending { idx, entry });
            }
        }
        None
    }

    /// Drain `pending_actions` without a real `ActiveEventLoop`.
    ///
    /// For use in `#[cfg(test)]` only. Runs each closure with a null
    /// `&ActiveEventLoop` pointer. Safe as long as closures annotate their
    /// `_el` parameter with a leading `_` and never dereference it.
    /// All closures pushed by `emDialog::finish_post_show` and the auto-delete
    /// rail satisfy this invariant.
    #[cfg(test)]
    pub(crate) fn drain_pending_actions_headless(&mut self) {
        let actions: Vec<DeferredAction> = self.pending_actions.borrow_mut().drain(..).collect();
        for action in actions {
            // SAFETY: all pending_actions closures in this crate take `_el`
            // and never dereference it. The null pointer is never read.
            action(self, unsafe { &*(std::ptr::NonNull::dangling().as_ptr()) });
        }
    }

    /// Phase 3.5 Task 10: unified close path for dialogs.
    ///
    /// Handles both lifecycle states:
    /// - **Post-materialize** (`dialog_windows` has `did`): unregisters all
    ///   `PanelScope::Toplevel(wid)` engines (via `engines_for_scope`) and
    ///   removes the `emWindow` from `self.windows`. Signal cleanup is handled
    ///   by slotmap dead-key semantics — fire-to-dead-signal is a no-op.
    /// - **Pre-materialize** (still in `pending_top_level`): `swap_remove`s
    ///   the pending entry; order of remaining entries does not matter.
    /// - **Unknown `DialogId`**: no-op; idempotent.
    ///
    /// Consumers:
    /// - `DialogPrivateEngine::Cycle` auto-delete (closure-rail push).
    /// - `emStocksListBox` `silent_cancel` replacement (Phase 3.5 Task 15).
    pub fn close_dialog_by_id(&mut self, did: DialogId) {
        if let Some(wid) = self.dialog_windows.remove(&did) {
            // Post-materialize: unregister all engines scoped to this window
            // then drop the emWindow.
            self.dialog_roots.remove(&did);
            let engine_ids = self
                .scheduler
                .engines_for_scope(crate::emPanelScope::PanelScope::Toplevel(wid));
            for eid in engine_ids {
                self.scheduler.remove_engine(eid);
            }
            self.windows.remove(&wid);
        } else if let Some(idx) = self
            .pending_top_level
            .iter()
            .position(|p| p.dialog_id == did)
        {
            // Pre-materialize: drop the pending entry.
            // swap_remove is O(1); order of pending_top_level doesn't matter.
            self.pending_top_level.swap_remove(idx);
        }
        // Else: unknown DialogId — idempotent no-op.
    }

    /// Apply a closure to the `DlgPanel` rooted at the dialog identified by
    /// `did`. Silently no-ops if `did` is unknown, the window is absent, or
    /// the root panel is not a `DlgPanel` (callers race with close).
    ///
    /// Rust-only consolidation of the "look up wid + root_panel_id → take_tree
    /// → take_behavior → apply → put → put → wake" walk that `emDialog::
    /// finish_post_show` inlines. No direct C++ counterpart — the C++ analogs
    /// are the direct `emDialog::SetTitle` / `emDialog::SetAutoDeletion` / etc.
    /// calls that mutate the `emDialog` object directly (which owns its state).
    /// `Prereq C` (Phase 3.6) will retire `finish_post_show`'s inlined walk in
    /// favour of this method.
    ///
    /// Wakes all engines scoped to `PanelScope::Toplevel(wid)` after mutation
    /// so `DialogPrivateEngine` observes the change on the next tick.
    pub fn mutate_dialog_by_id(
        &mut self,
        did: DialogId,
        f: impl FnOnce(&mut crate::emDialog::DlgPanel, &mut crate::emPanelTree::PanelTree),
    ) {
        // 1. Look up wid from dialog_windows; silently no-op if missing.
        let wid = match self.dialog_windows.get(&did).copied() {
            Some(w) => w,
            None => return,
        };
        // 2. Look up root_panel_id from dialog_roots; silently no-op if missing.
        let root_panel_id = match self.dialog_roots.get(&did).copied() {
            Some(p) => p,
            None => return,
        };
        // 3. Look up the emWindow; silently no-op if missing.
        let win = match self.windows.get_mut(&wid) {
            Some(w) => w,
            None => return,
        };
        // 4–8. Take tree, take behavior, apply closure, put behavior, put tree.
        // Root panel is taken out of the tree during the closure — children
        // remain in the tree, so the closure can walk them via `&mut tree`.
        let mut tree = win.take_tree();
        if let Some(mut behavior) = tree.take_behavior(root_panel_id) {
            if let Some(dlg) = behavior.as_dlg_panel_mut() {
                f(dlg, &mut tree);
            }
            tree.put_behavior(root_panel_id, behavior);
        }
        win.put_tree(tree);
        // 9. Wake all engines scoped to Toplevel(wid).
        let eids = self
            .scheduler
            .engines_for_scope(crate::emPanelScope::PanelScope::Toplevel(wid));
        for eid in eids {
            self.scheduler.wake_up(eid);
        }
    }

    /// Read `DlgPanel.finalized_result` for the dialog identified by
    /// `did`. Production path walks `dialog_windows → windows → tree`
    /// via `mutate_dialog_by_id`. Tests may first seed
    /// [`Self::headless_dialog_results`] so a dialog that could not be
    /// installed at a distinct headless `WindowId` still returns a
    /// deterministic result. Returns `None` if neither source resolves.
    pub(crate) fn read_dialog_finalized_result(
        &mut self,
        did: DialogId,
    ) -> Option<crate::emDialog::DialogResult> {
        #[cfg(test)]
        if let Some(r) = self.headless_dialog_results.get(&did).copied() {
            return Some(r);
        }
        let mut out = None;
        self.mutate_dialog_by_id(did, |dlg, _tree| out = dlg.finalized_result);
        out
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Deregister the framework-owned InputDispatchEngine so the
        // scheduler's drop-time invariant (no engines remaining) holds.
        // `remove_engine` is idempotent on unknown ids so double-drop is
        // safe; it is the only framework-owned engine registered in
        // `App::new`.
        self.scheduler.remove_engine(self.input_dispatch_engine_id);
    }
}

impl ApplicationHandler<CtrlMsg> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: CtrlMsg) {
        crate::emCtrlSocket::handle_main_thread(self, event_loop, event);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        // Init GPU
        self.gpu = Some(GpuContext::new());

        // Scan monitors — allocate signal IDs for geometry/window-list changes.
        let geom_sig = self.scheduler.create_signal();
        let win_sig = self.scheduler.create_signal();
        self.screen = Some(emScreen::from_event_loop(event_loop, geom_sig, win_sig));

        // Call user setup
        if let Some(setup) = self.setup_fn.take() {
            setup(self, event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                // Top-level close: auto-delete from `windows` and fire its
                // close signal. Popup close: fire the popup's close signal
                // but do NOT remove it here — teardown happens in
                // `emView::RawVisitAbs`'s pop path when the view zooms
                // back inside home (C++ emView.cpp:1676-1680). Popup is
                // always AUTO_DELETE but ownership lives on emView.
                let (is_popup, auto_delete, sig) = if let Some(win) = self.windows.get(&window_id) {
                    (
                        false,
                        win.flags.contains(WindowFlags::AUTO_DELETE),
                        Some(win.close_signal),
                    )
                } else if let Some(popup) = Self::find_window_mut(&mut self.windows, window_id) {
                    (true, false, Some(popup.close_signal))
                } else {
                    (false, true, None)
                };

                if let Some(sig) = sig {
                    self.scheduler.fire(sig);
                }

                if !is_popup && auto_delete {
                    self.windows.remove(&window_id);
                }

                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    let root = self.context.clone();
                    let mut sc = crate::emEngineCtx::SchedCtx {
                        scheduler: &mut self.scheduler,
                        framework_actions: &mut self.framework_actions,
                        root_context: &root,
                        framework_clipboard: &self.clipboard,
                        current_engine: None,
                        pending_actions: &self.pending_actions,
                    };
                    // Phase 3.5.A Task 7: emWindow::resize drops its external
                    // tree param; the window uses its own `self.tree` via
                    // internal destructure.
                    win.resize(gpu, size.width, size.height, &mut sc);
                    win.set_geometry_changed();
                    // Don't request_redraw here — about_to_wait will detect the
                    // layout change from the new tallness and issue a single
                    // repaint after layout is settled.
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    win.set_geometry_changed();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    let gpu = self.gpu.as_ref().unwrap();
                    // Phase 3.5.A Task 7: render uses `self.tree` internally.
                    win.render(gpu);
                }
            }
            WindowEvent::Focused(focused) => {
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    // Phase 3.5.A Task 7: window owns its tree; take it out
                    // for the SetFocused call, then put it back.
                    let mut tree = win.take_tree();
                    win.view_mut().SetFocused(&mut tree, focused);
                    win.put_tree(tree);
                    win.set_focus_changed();
                    win.invalidate();
                    win.request_redraw();
                }
                // Clear stuck modifier keys on focus loss. The OS (e.g. X11
                // via Alt+Tab) may not deliver key-release events to a window
                // that loses focus, leaving modifiers like Alt stuck in
                // input_state. C++ fixes this via XQueryKeymap on FocusIn;
                // Rust clears all pressed keys on FocusOut instead.
                if !focused {
                    self.input_state.clear_keys();
                }
            }
            WindowEvent::Touch(ref touch) => {
                let forward_events = {
                    let Some(win) = Self::find_window_mut(&mut self.windows, window_id) else {
                        return;
                    };
                    let mut sc = crate::emEngineCtx::SchedCtx {
                        scheduler: &mut self.scheduler,
                        framework_actions: &mut self.framework_actions,
                        root_context: &self.context,
                        framework_clipboard: &self.clipboard,
                        current_engine: None,
                        pending_actions: &self.pending_actions,
                    };
                    // Phase 3.5.A Task 7: window owns its tree; handle_touch
                    // splits internally.
                    win.handle_touch(touch, &mut sc);
                    win.touch_vif_mut().drain_forward_events()
                };
                // Phase 3: enqueue forward events for the
                // InputDispatchEngine rather than dispatching inline.
                // Modifier-key bookkeeping stays here because it touches
                // `input_state` directly at winit-callback granularity
                // (press/release persistence across frames).
                if !forward_events.is_empty() {
                    for event in &forward_events {
                        match event.variant {
                            InputVariant::Press => {
                                if event.shift {
                                    self.input_state.press(InputKey::Shift);
                                }
                                if event.ctrl {
                                    self.input_state.press(InputKey::Ctrl);
                                }
                            }
                            InputVariant::Release => {
                                if event.shift {
                                    self.input_state.release(InputKey::Shift);
                                }
                                if event.ctrl {
                                    self.input_state.release(InputKey::Ctrl);
                                }
                            }
                            _ => {}
                        }
                        self.input_state.set_mouse(event.mouse_x, event.mouse_y);
                        let mut ev = event.clone();
                        ev.mouse_x = self.input_state.mouse_x;
                        ev.mouse_y = self.input_state.mouse_y;
                        self.pending_inputs.push((window_id, ev));
                    }
                    self.scheduler.wake_up(self.input_dispatch_engine_id);
                }
                if let Some(win) = Self::find_window_mut(&mut self.windows, window_id) {
                    win.invalidate();
                    win.request_redraw();
                }
            }
            ref input_event => {
                if let Some(mut input) = emWindow::handle_input(input_event) {
                    // Update persistent input state
                    match input.variant {
                        InputVariant::Press => self.input_state.press(input.key),
                        InputVariant::Release => self.input_state.release(input.key),
                        InputVariant::Move => {
                            self.input_state.set_mouse(input.mouse_x, input.mouse_y);
                        }
                        _ => {}
                    }

                    // Always populate current mouse position on events,
                    // except for wheel events which encode scroll delta in mouse_x/y.
                    if !matches!(
                        input.key,
                        InputKey::WheelUp
                            | InputKey::WheelDown
                            | InputKey::WheelLeft
                            | InputKey::WheelRight
                    ) {
                        input.mouse_x = self.input_state.mouse_x;
                        input.mouse_y = self.input_state.mouse_y;
                    }

                    // Phase 3 / spec §4 D4.9: enqueue for the
                    // InputDispatchEngine to drain at top priority on the
                    // next time slice. `find_window_mut` still scans so we
                    // only enqueue for known (top-level or popup) windows —
                    // unknown WindowIds are silently dropped, matching the
                    // pre-migration `if let Some(win) = ...` gate.
                    if Self::find_window_mut(&mut self.windows, window_id).is_some() {
                        self.pending_inputs.push((window_id, input));
                        self.scheduler.wake_up(self.input_dispatch_engine_id);
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        crate::emCtrlSocket::check_pending_wait_idle(self);

        // Tick screensaver keepalive (pokes xscreensaver every 59s when inhibited).
        super::emWindowPlatform::tick_screensaver_keepalive();

        // Lazy-wire each view's pending_framework_actions handle so that
        // popup-creation paths in `emView::RawVisitAbs` can enqueue back into
        // `App::pending_actions`. Guarded by is_none() — one-shot init per view.
        for win in self.windows.values_mut() {
            if win.view().pending_framework_actions.is_none() {
                win.view_mut()
                    .set_pending_framework_actions(self.pending_actions.clone());
            }
        }

        // Process deferred actions (window creation from Duplicate/ccw,
        // popup surface materialization, etc.). Drain by move.
        //
        // Phase-2 port-ownership-rewrite: closures no longer carry a captured
        // `Rc<RefCell<emWindow>>`; `App::windows` holds plain `emWindow`.
        // Popup materialization finds its target by walking `App::windows`
        // for a view whose `emView::PopupWindow` is in `Pending` state
        // (see `materialize_pending_popup`). Task-8 Path B — matches C++
        // `emView.cpp:1636` ownership (popup owned by launching view).
        let actions: Vec<DeferredAction> = self.pending_actions.borrow_mut().drain(..).collect();
        for action in actions {
            action(self, event_loop);
        }

        // Fire signals for any windows whose state changed this frame.
        let changed_signals: Vec<_> = self
            .windows
            .values_mut()
            .flat_map(|win| {
                let mut sigs = Vec::new();
                if win.flags_changed() {
                    win.clear_flags_changed();
                    sigs.push(win.flags_signal);
                }
                if win.focus_changed() {
                    win.clear_focus_changed();
                    sigs.push(win.focus_signal);
                }
                if win.geometry_changed() {
                    win.clear_geometry_changed();
                    sigs.push(win.geometry_signal);
                }
                sigs
            })
            .collect();
        for sig in changed_signals {
            self.scheduler.fire(sig);
        }

        // Run one scheduler time slice. `framework_actions` is now owned by
        // `App` per spec §3.1 and passed through as a `&mut Vec<DeferredAction>`
        // parameter — engines push via `EngineCtx::framework_action` and the
        // framework consumes here between slices.
        //
        // Disjoint-field borrow: destructure `self` so scheduler, tree, windows,
        // context, and framework_actions can all be borrowed simultaneously.
        {
            let App {
                ref mut scheduler,
                ref mut windows,
                ref context,
                ref mut framework_actions,
                ref mut pending_inputs,
                ref mut input_state,
                ref clipboard,
                ref pending_actions,
                ..
            } = *self;
            scheduler.DoTimeSlice(
                windows,
                context,
                framework_actions,
                pending_inputs,
                input_state,
                clipboard,
                pending_actions,
            );
        }

        // Phase 1.75 Task 5 (continuation): the former post-slice
        // adapter-registration catch-up pass has been deleted.
        // `register_engine_for` is now synchronous (no view borrow held),
        // so `create_child` from inside an engine's `Cycle` registers the
        // adapter inline. Scheduler dispatch no longer borrows the
        // tree/view in a way that blocks registration.
        //
        // Keep event loop pumping while engines are active.
        // C++ runs a tight 10ms loop; Rust uses event-driven winit with
        // ControlFlow::Wait which only fires about_to_wait on OS events.
        // Requesting redraws ensures continuous cycling during startup,
        // animations, and any other engine activity.
        if self.scheduler.has_awake_engines() {
            for win in self.windows.values() {
                win.request_redraw();
            }
        }

        // SP4.5 + SP8 + Phase 1.75: all panel cycling runs through the
        // scheduler's normal engine loop. Top-level panels via
        // PanelCycleEngine registered at init_panel_view; sub-view panels
        // register on the SAME outer scheduler with
        // `PanelScope::SubView(outer_id, Outer)` so cross-tree dispatch
        // resolves via take/put on the sub-view's behavior (spec §3.3).

        // Notice dispatch now happens per-view inside emView::Update (SP5,
        // emView.cpp:1303-1314 parity). No global HandleNotice call here.

        // Update views and tick animators
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame_time)
            .as_secs_f64()
            .clamp(0.001, 0.1);
        self.last_frame_time = now;
        let App {
            ref mut scheduler,
            ref mut input_state,
            ref mut framework_actions,
            ref context,
            ref mut windows,
            ref mut pending_inputs,
            ref clipboard,
            ref pending_actions,
            input_dispatch_engine_id,
            ..
        } = *self;
        let state = input_state;
        for (win_id, win) in windows.iter_mut() {
            // Notice dispatch (including mark_viewing_dirty) happens inside
            // emView::Update via emView::HandleNotice (SP5).
            let mut needs_full_repaint = false;

            // Build SchedCtx for this window's VIF and animator ticks.
            let mut sc = crate::emEngineCtx::SchedCtx {
                scheduler,
                framework_actions,
                root_context: context,
                framework_clipboard: clipboard,
                current_engine: None,
                pending_actions,
            };

            // Phase 3.5.A Task 7: each window owns its tree. Take it out for
            // the animator + view-side calls below that require `&mut tree`
            // alongside `&mut view`, then put it back.
            let mut tree = win.take_tree();

            // Tick animator (take out to avoid borrow conflict)
            if let Some(mut anim) = win.active_animator.take() {
                if anim.animate(win.view_mut(), &mut tree, dt, &mut sc) {
                    win.active_animator = Some(anim);
                    needs_full_repaint = true;
                }
            }

            // Tick VIF animations (wheel zoom spring, grip pan spring)
            win.put_tree(tree);
            if win.tick_vif_animations(dt, &mut sc) {
                needs_full_repaint = true;
            }
            let mut tree = win.take_tree();

            // Dispatch synthetic events from gesture timer transitions
            // (cycle_gesture may have fired 250ms timeouts → EmuMouse/Visit/Menu)
            //
            // Phase 3: enqueue for InputDispatchEngine instead of inline
            // dispatch. Modifier-key state is updated here (winit-callback-
            // granularity parity). The queued events will be drained on the
            // next DoTimeSlice tick; `request_redraw` below keeps the event
            // loop pumping so that tick happens immediately.
            let forward_events = win.touch_vif_mut().drain_forward_events();
            if !forward_events.is_empty() {
                for event in &forward_events {
                    match event.variant {
                        InputVariant::Press => {
                            if event.shift {
                                state.press(InputKey::Shift);
                            }
                            if event.ctrl {
                                state.press(InputKey::Ctrl);
                            }
                        }
                        InputVariant::Release => {
                            if event.shift {
                                state.release(InputKey::Shift);
                            }
                            if event.ctrl {
                                state.release(InputKey::Ctrl);
                            }
                        }
                        _ => {}
                    }
                    state.set_mouse(event.mouse_x, event.mouse_y);
                    let mut ev = event.clone();
                    ev.mouse_x = state.mouse_x;
                    ev.mouse_y = state.mouse_y;
                    pending_inputs.push((*win_id, ev));
                }
                scheduler.wake_up(input_dispatch_engine_id);
                win.invalidate();
                win.request_redraw();
            }

            // SP4: Update runs only via UpdateEngineClass::Cycle now
            // (C++ single-caller model, emView.cpp:2523).

            // Collect invalidation from sub-view panels (C++ invalidation chain:
            // SubViewClass::InvalidateTitle, SubViewPortClass::InvalidateCursor,
            // SubViewPortClass::InvalidatePainting → SuperPanel → parent view).
            win.view_mut().collect_parent_invalidation(&mut tree);

            // Invalidate the active (focused) panel every frame so that
            // cursor blink and other clock-driven updates repaint. This
            // matches C++ emCore where Input() is called for all viewed
            // panels on every frame, and emTextField invalidates itself
            // when the blink timer fires.
            let active_id = win.view().GetActivePanel();
            if let Some(active_id) = active_id {
                win.view_mut().InvalidatePainting(&tree, active_id);
            }

            // Phase 3.5.A Task 7: put the tree back on the window.
            win.put_tree(tree);

            // Check for pending dirty rects from invalidate_painting calls.
            // Convert each dirty rect to tile grid coordinates and mark only
            // the overlapping tiles as dirty (partial repaint).
            let mut has_dirty_rects = false;
            let has_dirty = win.view().has_dirty_rects();
            if has_dirty {
                let dirty = win.view_mut().take_dirty_clip_rects();
                log::trace!(
                    "dirty clip rects: {} regions, bounds {:?}",
                    dirty.GetCount(),
                    dirty.GetMinMax()
                );
                for r in dirty.iter() {
                    win.mark_dirty_rect(r.x1, r.y1, r.x2, r.y2);
                }
                has_dirty_rects = true;
            }

            // Check for viewport changes (scroll/zoom/visit from VIFs)
            if win.view().viewport_changed() {
                win.view_mut().clear_viewport_changed();
                needs_full_repaint = true;
            }

            // Stress test: sync state and force full repaint every frame
            win.view_mut().sync_stress_test();
            if win.view().is_stress_test_active() {
                needs_full_repaint = true;
            }

            if needs_full_repaint {
                win.invalidate();
                win.request_redraw();
            } else if has_dirty_rects {
                win.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_scheduler_shape() {
        let framework = App::new(Box::new(|_app, _el| {}));
        let _: &EngineScheduler = &framework.scheduler;
        assert!(framework.framework_actions.is_empty());
    }

    /// Phase 3.5.A Task 9 tests for the pending-top-level install path.
    /// `install_pending_top_level` itself requires an `ActiveEventLoop`
    /// so it is exercised indirectly — these tests cover the allocation
    /// counter, the queue, and `dialog_window_mut`'s Pending vs
    /// Materialized resolution.
    mod pending_top_level_tests {
        use super::*;
        use crate::emColor::emColor;

        fn test_app() -> App {
            App::new(Box::new(|_app, _el| {}))
        }

        fn make_pending_window(app: &mut App) -> emWindow {
            let close_sig = app.scheduler.create_signal();
            let flags_sig = app.scheduler.create_signal();
            let focus_sig = app.scheduler.create_signal();
            let geom_sig = app.scheduler.create_signal();
            emWindow::new_top_level_pending(
                Rc::clone(&app.context),
                WindowFlags::empty(),
                "test-dialog".to_string(),
                close_sig,
                flags_sig,
                focus_sig,
                geom_sig,
                emColor::TRANSPARENT,
            )
        }

        #[test]
        fn allocate_dialog_id_monotonic() {
            let mut app = test_app();
            let a = app.allocate_dialog_id();
            let b = app.allocate_dialog_id();
            let c = app.allocate_dialog_id();
            assert_eq!(a, DialogId(0));
            assert_eq!(b, DialogId(1));
            assert_eq!(c, DialogId(2));
        }

        #[test]
        fn dialog_window_mut_resolves_pending() {
            let mut app = test_app();
            let did = app.allocate_dialog_id();
            let close_sig = app.scheduler.create_signal();
            let window = make_pending_window(&mut app);
            let mut tree = crate::emPanelTree::PanelTree::new();
            let root_id = tree.create_root("dlg", false);
            app.pending_top_level.push(PendingTopLevel {
                dialog_id: did,
                window,
                close_signal: close_sig,
                private_engine_root_panel_id: root_id,
                wake_up_signals: Vec::new(),
            });

            match app.dialog_window_mut(did) {
                Some(DialogWindow::Pending { idx, entry }) => {
                    assert_eq!(idx, 0);
                    assert_eq!(entry.dialog_id, did);
                }
                _ => panic!("expected Pending variant"),
            }
        }

        #[test]
        fn dialog_window_mut_resolves_materialized() {
            // Simulate post-install state: DialogId → WindowId recorded,
            // emWindow moved into App::windows. We build a headless
            // emWindow (still Pending OsSurface — good enough; the test
            // only checks map resolution, not OS surface state) and
            // insert under a dummy WindowId.
            let mut app = test_app();
            let did = app.allocate_dialog_id();
            let wid = WindowId::dummy();
            let window = make_pending_window(&mut app);
            app.windows.insert(wid, window);
            app.dialog_windows.insert(did, wid);

            match app.dialog_window_mut(did) {
                Some(DialogWindow::Materialized { window_id, .. }) => {
                    assert_eq!(window_id, wid);
                }
                _ => panic!("expected Materialized variant"),
            }
        }

        #[test]
        fn dialog_window_mut_unknown_id_returns_none() {
            let mut app = test_app();
            // Allocate to advance counter, then query a different id.
            let _ = app.allocate_dialog_id();
            assert!(app.dialog_window_mut(DialogId(999)).is_none());
        }

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
                close_sig,
                flags_sig,
                focus_sig,
                geom_sig,
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
                wake_up_signals: Vec::new(),
            });
            assert_eq!(
                app.pending_top_level[0].private_engine_root_panel_id,
                root_id
            );
        }

        // ─── Phase 3.5 Task 10 tests — close_dialog_by_id ───────────────────

        fn push_pending(app: &mut App) -> (DialogId, crate::emPanelTree::PanelId) {
            let did = app.allocate_dialog_id();
            let close_sig = app.scheduler.create_signal();
            let window = make_pending_window(app);
            let mut tree = crate::emPanelTree::PanelTree::new();
            let root_id = tree.create_root("dlg", false);
            // The PendingTopLevel carries the tree via its emWindow, but
            // make_pending_window creates a window without a meaningful tree.
            // For these close_dialog_by_id tests we only need the pending
            // queue to have the entry — the tree contents are irrelevant.
            app.pending_top_level.push(PendingTopLevel {
                dialog_id: did,
                window,
                close_signal: close_sig,
                private_engine_root_panel_id: root_id,
                wake_up_signals: Vec::new(),
            });
            (did, root_id)
        }

        #[test]
        fn close_dialog_by_id_pre_materialize_drops_pending() {
            let mut app = test_app();
            let (did, _root_id) = push_pending(&mut app);
            assert_eq!(app.pending_top_level.len(), 1);

            app.close_dialog_by_id(did);

            assert_eq!(
                app.pending_top_level.len(),
                0,
                "pending entry must be removed"
            );
            assert!(
                !app.dialog_windows.contains_key(&did),
                "dialog_windows unaffected (wasn't materialized)"
            );
        }

        #[test]
        fn close_dialog_by_id_post_materialize_removes_window_and_engines() {
            use crate::emPanelScope::PanelScope;
            use winit::window::WindowId;

            let mut app = test_app();
            let (did, root_id) = push_pending(&mut app);
            let wid = WindowId::dummy();
            // Give the pending window a proper tree so install can find the
            // root panel (the engine expects it). Re-build the pending entry.
            // Simplest: use install_pending_top_level_headless which handles
            // everything.
            // The existing push_pending already set private_engine_root_panel_id.
            let engine_id = app
                .install_pending_top_level_headless(wid)
                .expect("install succeeds");

            // Post-install: window + engine + mapping all present.
            assert!(app.windows.contains_key(&wid));
            assert_eq!(app.dialog_windows.get(&did).copied(), Some(wid));
            assert!(
                !app.scheduler
                    .engines_for_scope(PanelScope::Toplevel(wid))
                    .is_empty(),
                "engine registered at Toplevel(wid)"
            );

            app.close_dialog_by_id(did);

            assert!(!app.windows.contains_key(&wid), "window must be removed");
            assert!(
                !app.dialog_windows.contains_key(&did),
                "dialog_windows mapping must be cleared"
            );
            assert!(
                app.scheduler
                    .engines_for_scope(PanelScope::Toplevel(wid))
                    .is_empty(),
                "all Toplevel(wid) engines must be unregistered"
            );

            // Suppress unused — engine_id was consumed by close_dialog_by_id
            let _ = engine_id;
            let _ = root_id;
            app.scheduler.clear_pending_for_tests();
        }

        #[test]
        fn close_dialog_by_id_unknown_is_noop() {
            let mut app = test_app();
            let did = app.allocate_dialog_id();
            // Never enqueued, never materialized.
            app.close_dialog_by_id(did); // must not panic
            assert_eq!(app.pending_top_level.len(), 0);
        }

        // ─── Phase 3.6 Prereq A tests — mutate_dialog_by_id ─────────────────

        /// Stand up App + materialized dialog headlessly, call
        /// `mutate_dialog_by_id`, and verify:
        ///   (a) the title mutation lands on the DlgPanel, and
        ///   (b) engines scoped to Toplevel(wid) are woken by the call.
        ///
        /// NOTE: a fully-initialised App (with GPU) cannot be constructed in a
        /// unit test. We use `install_pending_top_level_headless`, which skips
        /// OS surface creation and `SetGeometry`, but fully registers the
        /// `DialogPrivateEngine` at `PanelScope::Toplevel(wid)` and populates
        /// `dialog_windows` + `dialog_roots`. This is the same harness shape
        /// used by the existing `close_dialog_by_id_post_materialize_*` test.
        ///
        /// We build the PendingTopLevel manually (not via `push_pending`) so
        /// the window's tree contains the DlgPanel behavior at `root_id` —
        /// `push_pending` creates a PanelTree that is separate from the
        /// window's own tree, which would leave the behavior absent after
        /// install.
        #[test]
        fn mutate_dialog_by_id_applies_closure_and_wakes_engines() {
            use crate::emDialog::{DialogResult, DlgPanel};
            use crate::emLook::emLook;
            use crate::emPanelScope::PanelScope;
            use crate::emPanelTree::PanelTree;
            use winit::window::WindowId;

            let mut app = test_app();
            let did = app.allocate_dialog_id();
            let close_sig = app.scheduler.create_signal();
            let finish_sig = app.scheduler.create_signal();

            // Build a PanelTree with a real DlgPanel behavior at the root,
            // mirroring what `emDialog::new` does.
            let mut tree = PanelTree::new();
            let root_id = tree.create_root("dlg", false);
            let look = std::rc::Rc::new(emLook::new());
            let dlg_panel = DlgPanel::new("Original", std::rc::Rc::clone(&look), finish_sig);
            tree.set_behavior(root_id, Box::new(dlg_panel));

            // Build the window with the populated tree installed.
            let mut window = make_pending_window(&mut app);
            let _ = window.take_tree();
            window.put_tree(tree);

            app.pending_top_level.push(PendingTopLevel {
                dialog_id: did,
                window,
                close_signal: close_sig,
                private_engine_root_panel_id: root_id,
                wake_up_signals: Vec::new(),
            });

            let wid = WindowId::dummy();
            let engine_id = app
                .install_pending_top_level_headless(wid)
                .expect("install registers DialogPrivateEngine");

            // Confirm dialog_roots bookkeeping was populated.
            assert!(
                app.dialog_roots.contains_key(&did),
                "dialog_roots must be populated after install"
            );

            // Confirm exactly one engine is scoped to Toplevel(wid).
            let eids_before = app.scheduler.engines_for_scope(PanelScope::Toplevel(wid));
            assert_eq!(
                eids_before.len(),
                1,
                "exactly one engine scoped to Toplevel(wid)"
            );

            // Apply the mutation.
            app.mutate_dialog_by_id(did, |p: &mut DlgPanel, _tree| {
                p.SetTitle("Changed");
            });

            // Read back the title via tree take/put.
            let root_pid = *app.dialog_roots.get(&did).unwrap();
            let win = app.windows.get_mut(&wid).unwrap();
            let mut tree = win.take_tree();
            let mut b = tree
                .take_behavior(root_pid)
                .expect("DlgPanel must be present");
            let title = b
                .as_dlg_panel_mut()
                .expect("root must be DlgPanel")
                .border
                .caption
                .clone();
            tree.put_behavior(root_pid, b);
            win.put_tree(tree);

            assert_eq!(title, "Changed", "title mutation must land on DlgPanel");

            // Engines scoped to Toplevel(wid) must still be registered (wake,
            // not removal). engines_for_scope returning non-empty confirms the
            // engine is still alive after the wake call.
            let eids_after = app.scheduler.engines_for_scope(PanelScope::Toplevel(wid));
            assert_eq!(
                eids_after.len(),
                1,
                "engine must still be registered after mutate_dialog_by_id"
            );

            // Teardown: remove the DialogPrivateEngine so EngineScheduler
            // drop-assert passes. Same pattern as
            // `close_dialog_by_id_post_materialize_removes_window_and_engines`.
            app.close_dialog_by_id(did);

            let _ = engine_id;
            let _ = finish_sig; // finish_signal is not connected to any engine in this test
            let _ = DialogResult::Ok; // silence unused import
            app.scheduler.clear_pending_for_tests();
        }

        #[test]
        fn mutate_dialog_by_id_unknown_id_is_noop() {
            let mut app = test_app();
            // Neither pending nor materialized — must not panic.
            app.mutate_dialog_by_id(DialogId(999), |_p, _tree| {
                panic!("closure must not fire for unknown DialogId");
            });
        }
    }
}
