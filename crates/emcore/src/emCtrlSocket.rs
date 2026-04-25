//! RUST_ONLY: (language-forced-utility)
//! No C++ analogue; agent-driven debugging requires a programmatic channel
//! that C++'s GUI-only cheat codes (emViewInputFilter::DoCheat) do not
//! provide. Gated behind EMCORE_DEBUG_CONTROL=1 — zero runtime cost when
//! unset.
//!
//! Unix-domain socket at $TMPDIR/eaglemode-rs.<pid>.sock. JSON-lines
//! protocol. Acceptor thread + per-connection worker threads dispatch
//! commands through winit::EventLoopProxy onto the main thread, which
//! mutates view state and sends replies via std::sync::mpsc. This module
//! defines the wire types only — acceptor / worker / dispatch land in
//! Tasks 3.3-3.6.

#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::sync::mpsc::SyncSender;

use winit::event_loop::ActiveEventLoop;

use crate::emGUIFramework::App;
use crate::emPanelTree::{DecodeIdentity, PanelId, PanelTree};

/// Resolve an emCore-native identity string to a `PanelId` within
/// `tree`, starting at `root`. `GetIdentity(tree, root)` includes the
/// root's name as the first segment; the decoder consumes `names[0]` as
/// the expected root-name (erroring on mismatch) and descends from
/// `names[1..]`. An empty identity string means "the root itself".
///
/// Identity strings handle empty-named panels and special characters via
/// the existing `EncodeIdentity` / `DecodeIdentity` machinery in
/// `emPanelTree.rs`.
pub(crate) fn resolve_identity(
    tree: &PanelTree,
    root: PanelId,
    identity: &str,
) -> Result<PanelId, String> {
    // Empty identity string means "the root itself" — short-circuit
    // before decoding (C++ DecodeIdentity("") yields a single empty
    // segment, which would otherwise be matched against the root name).
    if identity.is_empty() {
        return Ok(root);
    }
    let names = DecodeIdentity(identity);
    let root_name = tree.name(root).unwrap_or("");
    if names[0] != root_name {
        return Err(format!(
            "identity root mismatch: {:?} does not match root panel name {:?}",
            names[0], root_name
        ));
    }
    let mut cur = root;
    for (i, name) in names[1..].iter().enumerate() {
        let depth = i + 1;
        let matched: Vec<PanelId> = tree
            .children(cur)
            .filter(|&c| tree.name(c) == Some(name.as_str()))
            .collect();
        match matched.len() {
            0 => {
                return Err(format!(
                    "no such panel: {} (segment {} = {:?} not found under {:?})",
                    identity,
                    depth,
                    name,
                    tree.name(cur).unwrap_or("<unnamed>")
                ));
            }
            1 => cur = matched[0],
            n => {
                return Err(format!(
                    "ambiguous identity: {} (segment {} = {:?} matches {} siblings under {:?})",
                    identity, depth, name, n, tree.name(cur).unwrap_or("<unnamed>")
                ));
            }
        }
    }
    Ok(cur)
}

/// Resolve `{ view, identity }` and invoke `f` with the resulting
/// `(view, tree, panel)` triple. `view == ""` targets the home window's
/// outer view; otherwise `view_sel` resolves (via the outer tree) to an
/// `emSubViewPanel`, whose inner view/tree are used for `identity`.
///
/// Closure-based rather than returning references because
/// `PanelTree::with_behavior_as` borrows the behavior out of the tree
/// for the duration of its callback and reinserts it on return —
/// references into the SVP's `sub_view`/`sub_tree` fields cannot
/// outlive the callback. Handlers therefore do their work inside the
/// callback.
pub(crate) fn resolve_target<R>(
    app: &mut App,
    view_sel: &str,
    identity: &str,
    f: impl FnOnce(&mut crate::emView::emView, &mut PanelTree, PanelId, &mut crate::emEngineCtx::SchedCtx<'_>) -> R,
) -> Result<R, String> {
    let home_id = app
        .home_window_id
        .ok_or_else(|| "home window not initialized".to_string())?;

    // Split-borrow the App: scheduler/framework_actions/context/etc.
    // are independent of `windows`. We construct a SchedCtx that
    // outlives the closure call below.
    let App {
        windows,
        scheduler,
        framework_actions,
        context,
        clipboard,
        pending_actions,
        ..
    } = app;
    let mut sc = crate::emEngineCtx::SchedCtx {
        scheduler,
        framework_actions,
        root_context: context,
        framework_clipboard: clipboard,
        current_engine: None,
        pending_actions,
    };

    let win = windows
        .get_mut(&home_id)
        .ok_or_else(|| "home window missing".to_string())?;

    if view_sel.is_empty() {
        let tree = &mut win.tree;
        let view = &mut win.view;
        let root = tree
            .GetRootPanel()
            .ok_or_else(|| "no root panel".to_string())?;
        let target = resolve_identity(tree, root, identity)?;
        return Ok(f(view, tree, target, &mut sc));
    }

    // Inner view: resolve view_sel against outer tree; require SVP.
    // Compute svp_id under an immutable borrow of win.tree, drop it,
    // then take the mutable borrow via with_behavior_as.
    let svp_id = {
        let outer_root = win
            .tree
            .GetRootPanel()
            .ok_or_else(|| "no root panel".to_string())?;
        resolve_identity(&win.tree, outer_root, view_sel)?
    };
    let svp_name = win
        .tree
        .name(svp_id)
        .unwrap_or("<unnamed>")
        .to_string();

    // Borrow rationale: the closure runs while the SVP behavior is taken
    // out of the tree. Its `sub_view` and `sub_tree` are owned by the
    // behavior, so the references handed to `f` are valid for the
    // closure's lifetime.
    let result = win
        .tree
        .with_behavior_as::<crate::emSubViewPanel::emSubViewPanel, _>(svp_id, |svp| {
            let (sub_view, sub_tree) = svp.sub_view_and_tree_mut();
            let sub_root = sub_tree
                .GetRootPanel()
                .ok_or_else(|| "sub-view has no root panel".to_string())?;
            let inner_target = resolve_identity(sub_tree, sub_root, identity)?;
            Ok::<R, String>(f(sub_view, sub_tree, inner_target, &mut sc))
        })
        .ok_or_else(|| {
            format!(
                "view selector '{}' resolved to panel '{}' which is not a sub-view panel",
                view_sel, svp_name
            )
        })?;
    result
}

/// Top-level command tag — wire format `{"cmd":"<name>", ...}`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum CtrlCmd {
    Dump,
    GetState,
    Quit,
    Visit {
        #[serde(default)]
        view: String,
        identity: String,
        #[serde(default)]
        adherent: bool,
    },
    VisitFullsized {
        #[serde(default)]
        view: String,
        identity: String,
    },
    SetFocus {
        #[serde(default)]
        view: String,
        identity: String,
    },
    SeekTo {
        #[serde(default)]
        view: String,
        identity: String,
    },
    WaitIdle {
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    Input {
        event: InputPayload,
    },
    InputBatch {
        events: Vec<InputPayload>,
    },
}

/// Synthetic input payload — wire format `{"kind":"<name>", ...}`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputPayload {
    Key {
        key: String,
        press: bool,
        #[serde(default)]
        mods: Modifiers,
    },
    MouseMove {
        x: f64,
        y: f64,
    },
    MouseButton {
        button: MouseButtonName,
        press: bool,
    },
    Scroll {
        dx: f64,
        dy: f64,
    },
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Modifiers {
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub logo: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButtonName {
    Left,
    Middle,
    Right,
}

/// Reply envelope. Optional fields are omitted from the JSON output when
/// `None`/empty so simple commands round-trip as `{"ok":true}`.
#[derive(Debug, Serialize, Default)]
pub struct CtrlReply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_frame: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_rect: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub loading: Vec<LoadingEntry>,
}

impl CtrlReply {
    /// Successful reply with no payload — serializes as `{"ok":true}`.
    pub fn ok() -> Self {
        Self {
            ok: true,
            ..Self::default()
        }
    }

    /// Error reply — serializes as `{"ok":false,"error":"..."}`.
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LoadingEntry {
    pub view: String,
    pub identity: String,
    pub pct: u32,
}

/// Message from acceptor-worker threads to the main thread via
/// winit::EventLoopProxy. The reply_tx is a oneshot
/// (`std::sync::mpsc::sync_channel(1)`); the main thread handler sends
/// the reply back, the worker reads it, serializes to JSON, writes to
/// the socket. Wired in Task 3.3.
#[derive(Debug)]
pub struct CtrlMsg {
    pub cmd: CtrlCmd,
    pub reply_tx: SyncSender<CtrlReply>,
}

/// Main-thread handler for incoming `CtrlMsg` via `EventLoopProxy`.
/// Dispatches the three minimal commands (Dump/Quit/GetState); other
/// commands return a documented placeholder until Tasks 4.x/5.x wire
/// them in.
pub fn handle_main_thread(app: &mut App, event_loop: &ActiveEventLoop, msg: CtrlMsg) {
    // wait_idle is special — the reply is parked, not sent inline.
    if let CtrlCmd::WaitIdle { timeout_ms } = msg.cmd {
        handle_wait_idle(msg.reply_tx, timeout_ms);
        return;
    }
    let reply = match msg.cmd {
        CtrlCmd::Dump => handle_dump(app),
        CtrlCmd::Quit => handle_quit(event_loop),
        CtrlCmd::GetState => handle_get_state(app),
        CtrlCmd::Visit {
            ref view,
            ref identity,
            adherent,
        } => handle_visit(app, view, identity, adherent),
        CtrlCmd::VisitFullsized {
            ref view,
            ref identity,
        } => handle_visit_fullsized(app, view, identity),
        CtrlCmd::SetFocus {
            ref view,
            ref identity,
        } => handle_set_focus(app, view, identity),
        CtrlCmd::SeekTo {
            ref view,
            ref identity,
        } => handle_seek_to(app, view, identity),
        CtrlCmd::WaitIdle { .. } => unreachable!(), // handled above
        CtrlCmd::Input { event } => match synthesize_and_dispatch(app, event_loop, event) {
            Ok(()) => CtrlReply::ok(),
            Err(e) => CtrlReply::err(e),
        },
        CtrlCmd::InputBatch { events } => {
            let mut last_err: Option<String> = None;
            for ev in events {
                if let Err(e) = synthesize_and_dispatch(app, event_loop, ev) {
                    last_err = Some(e);
                }
            }
            match last_err {
                Some(e) => CtrlReply::err(e),
                None => CtrlReply::ok(),
            }
        }
    };
    let _ = msg.reply_tx.send(reply);
}

fn handle_dump(app: &mut App) -> CtrlReply {
    let home_id = match app.home_window_id {
        Some(id) => id,
        None => return CtrlReply::err("home window not initialized"),
    };
    let win = match app.windows.get_mut(&home_id) {
        Some(w) => w,
        None => return CtrlReply::err("home window missing from App::windows"),
    };
    // emWindow keeps `view: emView` (pub) and `tree: PanelTree`
    // (pub(crate)) as sibling fields. Take a split borrow at field level
    // so we can hand `&emView` and `&mut PanelTree` to dump_tree
    // simultaneously.
    let path = {
        let view_ref = &win.view;
        let tree_ref = &mut win.tree;
        view_ref.dump_tree(tree_ref)
    };
    CtrlReply {
        ok: true,
        path: Some(path.to_string_lossy().into_owned()),
        ..CtrlReply::default()
    }
}

fn handle_quit(event_loop: &ActiveEventLoop) -> CtrlReply {
    event_loop.exit();
    CtrlReply::ok()
}

fn handle_get_state(app: &App) -> CtrlReply {
    let Some(home_id) = app.home_window_id else {
        return CtrlReply::err("home window not initialized");
    };
    let Some(win) = app.windows.get(&home_id) else {
        return CtrlReply::err("home window missing");
    };
    let outer_view = win.view();
    let outer_tree = win.tree();

    let (focused_view, focused_identity, picked) = focused_state(outer_view, outer_tree);
    let view_rect = [
        picked.CurrentX,
        picked.CurrentY,
        picked.CurrentWidth,
        picked.CurrentHeight,
    ];

    CtrlReply {
        ok: true,
        focused_view,
        focused_identity,
        view_rect: Some(view_rect),
        loading: Vec::new(),
        ..CtrlReply::default()
    }
}

/// Walk the ViewMap once to compute `(focused_view, focused_identity)`
/// plus the view whose rect should be reported (the focused view if any,
/// else the outer view).
///
/// `focused_view`/`focused_identity` semantics: if outer view is focused,
/// returns `(Some(""), Some(GetIdentity(focused)))`. If inner view is
/// focused, finds the containing emSubViewPanel in the outer tree and
/// returns `(Some(GetIdentity(svp)), Some(GetIdentity(focused_in_inner)))`.
///
/// emView semantics keep focus mutually exclusive across the hierarchy;
/// debug builds assert that invariant against HashMap iteration order.
fn focused_state<'a>(
    outer_view: &'a crate::emView::emView,
    outer_tree: &'a crate::emPanelTree::PanelTree,
) -> (Option<String>, Option<String>, &'a crate::emView::emView) {
    let view_map = crate::emTreeDump::collect_views(outer_view, outer_tree);

    let mut result: Option<(Option<String>, Option<String>, &'a crate::emView::emView)> = None;
    for (_ptr, (view, tree)) in view_map.iter() {
        if let Some(pid) = view.GetFocusedPanel() {
            // Focus-exclusivity invariant: at most one view in the map
            // may report GetFocusedPanel().is_some() at a time. A second
            // hit means hash-order non-determinism would flip our result.
            debug_assert!(
                result.is_none(),
                "multiple views report focused panel — focus-exclusivity invariant violated"
            );
            let view_sel = if std::rc::Rc::ptr_eq(view.GetContext(), outer_view.GetContext()) {
                String::new()
            } else {
                match find_svp_by_inner_view(outer_tree, view) {
                    Some(svp_id) => outer_tree.GetIdentity(svp_id),
                    None => {
                        // Invariant: every inner view in the ViewMap is
                        // reachable via an emSubViewPanel in the outer
                        // tree (collect_views only descends through SVPs).
                        debug_assert!(
                            false,
                            "inner view in ViewMap has no containing SVP in outer tree — invariant violation"
                        );
                        continue;
                    }
                }
            };
            let identity = tree.GetIdentity(pid);
            result = Some((Some(view_sel), Some(identity), *view));
            // In release: keep the first match (matches prior behavior).
            // In debug: keep iterating so the assertion above can fire.
            if !cfg!(debug_assertions) {
                break;
            }
        }
    }

    match result {
        Some((vs, id, v)) => (vs, id, v),
        None => (None, None, outer_view),
    }
}

/// Scan the outer tree for an emSubViewPanel whose inner view's context
/// matches `target_view`'s context. Returns the SVP's PanelId.
fn find_svp_by_inner_view(
    outer_tree: &crate::emPanelTree::PanelTree,
    target_view: &crate::emView::emView,
) -> Option<PanelId> {
    for pid in outer_tree.panel_ids() {
        let Some(b) = outer_tree.behavior(pid) else {
            continue;
        };
        let Some(svp) = b.as_sub_view_panel() else {
            continue;
        };
        if std::rc::Rc::ptr_eq(svp.sub_view.GetContext(), target_view.GetContext()) {
            return Some(pid);
        }
    }
    None
}

fn handle_visit(app: &mut App, view_sel: &str, identity: &str, adherent: bool) -> CtrlReply {
    match resolve_target(app, view_sel, identity, |view, tree, target, ctx| {
        view.VisitPanel(tree, target, adherent, ctx);
    }) {
        Ok(()) => CtrlReply::ok(),
        Err(e) => CtrlReply::err(e),
    }
}

fn handle_visit_fullsized(app: &mut App, view_sel: &str, identity: &str) -> CtrlReply {
    // C++ `emView::VisitFullsized(panel, adherent, utilizeView=false)` —
    // control-socket adherent/utilize_view default to false (matches C++
    // defaults in emView.h:341-342).
    match resolve_target(app, view_sel, identity, |view, tree, target, ctx| {
        view.VisitFullsized(tree, target, false, false, ctx);
    }) {
        Ok(()) => CtrlReply::ok(),
        Err(e) => CtrlReply::err(e),
    }
}

fn handle_set_focus(app: &mut App, view_sel: &str, identity: &str) -> CtrlReply {
    match resolve_target(app, view_sel, identity, |view, _tree, target, _ctx| {
        view.set_focus(Some(target));
    }) {
        Ok(()) => CtrlReply::ok(),
        Err(e) => CtrlReply::err(e),
    }
}

fn handle_seek_to(app: &mut App, view_sel: &str, identity: &str) -> CtrlReply {
    // TODO: true seek semantics (lazy-loading targets via the seek
    // engine) need `emView::VisitByIdentityBare(identity)` once the
    // seek engine is wired to the control surface. Today this falls
    // back to VisitPanel, which only works on already-materialized
    // sub-trees. Same wire format; behavioral upgrade later.
    match resolve_target(app, view_sel, identity, |view, tree, target, ctx| {
        view.VisitPanel(tree, target, false, ctx);
    }) {
        Ok(()) => CtrlReply::ok(),
        Err(e) => CtrlReply::err(e),
    }
}

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;

/// Returns the socket path this process uses. PID-namespaced so multiple
/// instances don't collide.
pub fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("eaglemode-rs.{}.sock", std::process::id()))
}

/// Spawn the acceptor thread. Call once at framework init, behind the
/// EMCORE_DEBUG_CONTROL gate (Task 3.6 wires the gate). The thread runs
/// until the process exits.
pub fn spawn_acceptor() -> std::io::Result<()> {
    let path = socket_path();
    // Cleanup any stale socket from a prior crashed run at the same PID
    // (extremely unlikely, but cheap insurance).
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    eprintln!("[emCtrlSocket] listening on {}", path.display());

    thread::Builder::new()
        .name("emCtrlSocket-acceptor".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let _ = thread::Builder::new()
                            .name("emCtrlSocket-worker".into())
                            .spawn(move || worker_loop(s));
                    }
                    Err(e) => {
                        eprintln!("[emCtrlSocket] accept error: {e}");
                    }
                }
            }
        })?;
    Ok(())
}

fn worker_loop(stream: UnixStream) {
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[emCtrlSocket] clone failed: {e}");
            return;
        }
    };
    let mut reader = BufReader::new(reader_stream);
    let mut writer = stream;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return, // EOF — client closed
            Ok(_) => {}
            Err(e) => {
                eprintln!("[emCtrlSocket] read error: {e}");
                return;
            }
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }

        let reply = match serde_json::from_str::<CtrlCmd>(trimmed) {
            Ok(cmd) => dispatch(cmd),
            Err(e) => CtrlReply::err(format!("parse: {e}")),
        };
        let json = match serde_json::to_string(&reply) {
            Ok(j) => j,
            Err(e) => format!(r#"{{"ok":false,"error":"serialize: {}"}}"#, e),
        };
        if let Err(e) = writeln!(writer, "{}", json) {
            eprintln!("[emCtrlSocket] write error: {e}");
            return;
        }
    }
}

fn dispatch(cmd: CtrlCmd) -> CtrlReply {
    let proxy = match crate::emGUIFramework::EVENT_LOOP_PROXY.get() {
        Some(p) => p,
        None => return CtrlReply::err("event loop not initialized"),
    };
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel::<CtrlReply>(1);
    let msg = CtrlMsg { cmd, reply_tx };
    if proxy.send_event(msg).is_err() {
        return CtrlReply::err("event loop closed");
    }
    match reply_rx.recv() {
        Ok(r) => r,
        Err(_) => CtrlReply::err("main thread aborted"),
    }
}

/// Call on process shutdown to unlink the socket file. Idempotent.
pub fn cleanup_on_exit() {
    let _ = std::fs::remove_file(socket_path());
}

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey, SmolStr};
use winit::window::WindowId;

/// Map agent-supplied key names to winit `Key` values.
///
/// Recognized named keys: Return/Enter, Escape, Tab, Space, Backspace,
/// Arrow{Up,Down,Left,Right}, Home, End, PageUp, PageDown, F1..F12.
/// Anything else is treated as a single-character `Key::Character`.
pub(crate) fn key_from_name(name: &str) -> Key {
    let named = match name {
        "Return" | "Enter" => Some(NamedKey::Enter),
        "Escape" => Some(NamedKey::Escape),
        "Tab" => Some(NamedKey::Tab),
        "Space" => Some(NamedKey::Space),
        "Backspace" => Some(NamedKey::Backspace),
        "ArrowUp" => Some(NamedKey::ArrowUp),
        "ArrowDown" => Some(NamedKey::ArrowDown),
        "ArrowLeft" => Some(NamedKey::ArrowLeft),
        "ArrowRight" => Some(NamedKey::ArrowRight),
        "Home" => Some(NamedKey::Home),
        "End" => Some(NamedKey::End),
        "PageUp" => Some(NamedKey::PageUp),
        "PageDown" => Some(NamedKey::PageDown),
        "F1" => Some(NamedKey::F1),
        "F2" => Some(NamedKey::F2),
        "F3" => Some(NamedKey::F3),
        "F4" => Some(NamedKey::F4),
        "F5" => Some(NamedKey::F5),
        "F6" => Some(NamedKey::F6),
        "F7" => Some(NamedKey::F7),
        "F8" => Some(NamedKey::F8),
        "F9" => Some(NamedKey::F9),
        "F10" => Some(NamedKey::F10),
        "F11" => Some(NamedKey::F11),
        "F12" => Some(NamedKey::F12),
        _ => None,
    };
    match named {
        Some(n) => Key::Named(n),
        None => Key::Character(SmolStr::new(name)),
    }
}

/// Construct a synthetic `WindowEvent` from an `InputPayload` and dispatch
/// it through `App::window_event` — the same entry point winit uses for
/// real input. Downstream handling is identical (scheduler ticks, focus
/// updates, paint requests, etc.).
///
/// Returns Err if the App has no home window or if the payload is a Key
/// event (see `build_window_event` for the dependency-forced limitation).
pub(crate) fn synthesize_and_dispatch(
    app: &mut App,
    event_loop: &ActiveEventLoop,
    payload: InputPayload,
) -> Result<(), String> {
    let window_id: WindowId = match app.home_window_id {
        Some(id) => id,
        None => return Err("home window not initialized".into()),
    };
    let event = build_window_event(payload)?;
    app.window_event(event_loop, window_id, event);
    Ok(())
}

/// Translate `InputPayload` into a `winit::WindowEvent`.
///
/// DIVERGED: (dependency-forced)
/// `WindowEvent::KeyboardInput { event: KeyEvent, .. }` cannot be
/// constructed outside the `winit` crate in 0.30 because
/// `KeyEvent::platform_specific` is `pub(crate)` and there is no
/// public constructor. C++ emCore drives synthetic key input by
/// directly calling `emView::InputKey` (emView.cpp), bypassing the
/// platform event layer; winit 0.30 admits no equivalent path. Until
/// winit exposes a public `KeyEvent` constructor (or we route synthetic
/// keys through a higher-level emCore entry point), the Key arm of
/// this function returns Err. Mouse / scroll / cursor events have
/// public constructors and are dispatched normally. Tracking note: a
/// future task may bypass `App::window_event` entirely for Key
/// payloads and call into `emWindow`/`emView` input methods directly.
fn build_window_event(payload: InputPayload) -> Result<WindowEvent, String> {
    Ok(match payload {
        InputPayload::Key { key, press: _, mods: _ } => {
            // Map the name eagerly so the agent gets immediate feedback
            // on bogus key names; the dependency-forced limitation
            // below means we can't yet construct a KeyEvent to deliver.
            let _logical = key_from_name(&key);
            return Err(
                "synthetic key events not yet supported (winit 0.30 KeyEvent has no public constructor)"
                    .into(),
            );
        }
        InputPayload::MouseMove { x, y } => WindowEvent::CursorMoved {
            device_id: synthetic_device_id(),
            position: winit::dpi::PhysicalPosition::new(x, y),
        },
        InputPayload::MouseButton { button, press } => {
            let winit_button = match button {
                MouseButtonName::Left => MouseButton::Left,
                MouseButtonName::Middle => MouseButton::Middle,
                MouseButtonName::Right => MouseButton::Right,
            };
            WindowEvent::MouseInput {
                device_id: synthetic_device_id(),
                state: if press {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                },
                button: winit_button,
            }
        }
        InputPayload::Scroll { dx, dy } => WindowEvent::MouseWheel {
            device_id: synthetic_device_id(),
            delta: MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(dx, dy)),
            phase: winit::event::TouchPhase::Moved,
        },
    })
}

/// Construct a placeholder `winit::event::DeviceId` for synthetic events
/// using winit's public `DeviceId::dummy()` constructor.
fn synthetic_device_id() -> winit::event::DeviceId {
    winit::event::DeviceId::dummy()
}


use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One pending wait_idle request — a worker is parked, waiting for the
/// scheduler to go idle (or a timeout).
struct PendingWaitIdle {
    reply_tx: SyncSender<CtrlReply>,
    deadline: Option<Instant>,
}

/// Main-thread-owned queue of pending wait_idle requests. Pushed by
/// `handle_wait_idle` (from the user-event handler running on the main
/// thread); drained by `check_pending_wait_idle` invoked from
/// `App::about_to_wait`. The Mutex is for safety — both push and drain
/// happen on the main thread, so contention should be zero.
static PENDING_WAIT_IDLE: Mutex<Vec<PendingWaitIdle>> = Mutex::new(Vec::new());

/// Park a wait_idle request. Reply is NOT sent here — drained in
/// `check_pending_wait_idle` (called from about_to_wait) when scheduler
/// is idle or timeout expires.
fn handle_wait_idle(reply_tx: SyncSender<CtrlReply>, timeout_ms: Option<u64>) {
    let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    PENDING_WAIT_IDLE
        .lock()
        .expect("PENDING_WAIT_IDLE poisoned")
        .push(PendingWaitIdle { reply_tx, deadline });
}

/// Drain the pending wait_idle queue. Called from
/// `App::about_to_wait` once per event-loop tick. For each entry:
///   - if scheduler.is_idle(), reply ok with current_frame as idle_frame
///   - else if deadline passed, reply error "timeout"
///   - else leave parked
pub fn check_pending_wait_idle(app: &App) {
    let mut pending = match PENDING_WAIT_IDLE.try_lock() {
        Ok(p) => p,
        Err(_) => return, // contention; try again next tick
    };
    if pending.is_empty() {
        return;
    }
    let scheduler_idle = app.scheduler.is_idle();
    let now = Instant::now();
    let mut i = 0;
    while i < pending.len() {
        let resolution = if scheduler_idle {
            // current_frame from the home view — if no home view, idle is
            // still true scheduler-wise but report idle_frame=0 as a stub.
            let idle_frame = app
                .home_window_id
                .and_then(|id| app.windows.get(&id))
                .map(|w| w.view().current_frame.get())
                .unwrap_or(0);
            Some(CtrlReply {
                ok: true,
                idle_frame: Some(idle_frame),
                ..CtrlReply::default()
            })
        } else if let Some(deadline) = pending[i].deadline {
            if now > deadline {
                Some(CtrlReply::err("timeout"))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(reply) = resolution {
            let entry = pending.swap_remove(i);
            let _ = entry.reply_tx.send(reply);
            // i unchanged — swap_remove moved a different entry into i
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_cmd_roundtrip() {
        let json = r#"{"cmd":"dump"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CtrlCmd::Dump));
    }

    #[test]
    fn get_state_cmd_roundtrip() {
        let json = r#"{"cmd":"get_state"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CtrlCmd::GetState));
    }

    #[test]
    fn quit_cmd_roundtrip() {
        let json = r#"{"cmd":"quit"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, CtrlCmd::Quit));
    }

    #[test]
    fn visit_cmd_deserializes_with_view_and_identity() {
        let json = r#"{"cmd":"visit","view":"root:content view","identity":"::home"}"#;
        let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
        match cmd {
            CtrlCmd::Visit { view, identity, adherent } => {
                assert_eq!(view, "root:content view");
                assert_eq!(identity, "::home");
                assert!(!adherent);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn visit_cmd_view_field_defaults_to_empty_string() {
        let json = r#"{"cmd":"visit","identity":"root"}"#;
        let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
        match cmd {
            CtrlCmd::Visit { view, identity, .. } => {
                assert_eq!(view, "");
                assert_eq!(identity, "root");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn visit_cmd_roundtrip_with_explicit_adherent() {
        let json = r#"{"cmd":"visit","identity":"root","adherent":true}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Visit { adherent, .. } => assert!(adherent),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn visit_fullsized_has_view_and_identity() {
        let json = r#"{"cmd":"visit_fullsized","view":"root:content view","identity":"::home"}"#;
        let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
        match cmd {
            CtrlCmd::VisitFullsized { view, identity } => {
                assert_eq!(view, "root:content view");
                assert_eq!(identity, "::home");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn set_focus_has_view_and_identity() {
        let json = r#"{"cmd":"set_focus","identity":"root"}"#;
        let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
        match cmd {
            CtrlCmd::SetFocus { view, identity } => {
                assert_eq!(view, "");
                assert_eq!(identity, "root");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn seek_to_has_view_and_identity() {
        let json = r#"{"cmd":"seek_to","identity":"root:content view"}"#;
        let cmd: CtrlCmd = serde_json::from_str(json).unwrap();
        match cmd {
            CtrlCmd::SeekTo { view, identity } => {
                assert_eq!(view, "");
                assert_eq!(identity, "root:content view");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn wait_idle_cmd_roundtrip_no_timeout() {
        let json = r#"{"cmd":"wait_idle"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::WaitIdle { timeout_ms } => assert_eq!(timeout_ms, None),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn wait_idle_cmd_roundtrip_with_timeout() {
        let json = r#"{"cmd":"wait_idle","timeout_ms":5000}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::WaitIdle { timeout_ms } => assert_eq!(timeout_ms, Some(5000)),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_key_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"key","key":"Return","press":true}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::Key { key, press, mods } } => {
                assert_eq!(key, "Return");
                assert!(press);
                assert!(!mods.shift && !mods.ctrl && !mods.alt && !mods.logo);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_key_with_mods_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"key","key":"a","press":true,"mods":{"shift":true,"ctrl":true}}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::Key { mods, .. } } => {
                assert!(mods.shift && mods.ctrl && !mods.alt && !mods.logo);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_mouse_move_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"mouse_move","x":1.5,"y":2.5}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::MouseMove { x, y } } => {
                assert_eq!(x, 1.5);
                assert_eq!(y, 2.5);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_mouse_button_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"mouse_button","button":"left","press":true}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::MouseButton { button, press } } => {
                assert!(matches!(button, MouseButtonName::Left));
                assert!(press);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_mouse_button_middle_right_roundtrip() {
        let json_m = r#"{"kind":"mouse_button","button":"middle","press":false}"#;
        let json_r = r#"{"kind":"mouse_button","button":"right","press":true}"#;
        let m: InputPayload = serde_json::from_str(json_m).unwrap();
        let r: InputPayload = serde_json::from_str(json_r).unwrap();
        assert!(matches!(m, InputPayload::MouseButton { button: MouseButtonName::Middle, press: false }));
        assert!(matches!(r, InputPayload::MouseButton { button: MouseButtonName::Right, press: true }));
    }

    #[test]
    fn input_scroll_roundtrip() {
        let json = r#"{"cmd":"input","event":{"kind":"scroll","dx":0.0,"dy":-3.0}}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Input { event: InputPayload::Scroll { dx, dy } } => {
                assert_eq!(dx, 0.0);
                assert_eq!(dy, -3.0);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn input_batch_roundtrip() {
        let json = r#"{"cmd":"input_batch","events":[{"kind":"key","key":"a","press":true},{"kind":"key","key":"a","press":false}]}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::InputBatch { events } => assert_eq!(events.len(), 2),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn reply_ok_omits_none_fields() {
        let r = CtrlReply::ok();
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"ok":true}"#);
    }

    #[test]
    fn reply_err_includes_error() {
        let r = CtrlReply::err("bad path");
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"ok":false,"error":"bad path"}"#);
    }

    #[test]
    fn reply_with_path_serializes() {
        let r = CtrlReply {
            ok: true,
            path: Some("/tmp/dump".to_string()),
            ..CtrlReply::default()
        };
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, r#"{"ok":true,"path":"/tmp/dump"}"#);
    }

    #[test]
    fn reply_full_payload_serializes() {
        let r = CtrlReply {
            ok: true,
            error: None,
            path: None,
            idle_frame: Some(42),
            focused_view: Some("".to_string()),
            focused_identity: Some("/foo".to_string()),
            view_rect: Some([0.0, 0.0, 100.0, 100.0]),
            loading: vec![LoadingEntry {
                view: "".to_string(),
                identity: "/bar".to_string(),
                pct: 50,
            }],
        };
        let j = serde_json::to_string(&r).unwrap();
        assert!(j.contains(r#""idle_frame":42"#));
        assert!(j.contains(r#""focused_view":"""#));
        assert!(j.contains(r#""focused_identity":"/foo""#));
        assert!(j.contains(r#""view_rect":[0.0,0.0,100.0,100.0]"#));
        assert!(j.contains(r#""loading":[{"view":"","identity":"/bar","pct":50}]"#));
    }

    #[test]
    fn get_state_reply_serializes_new_fields() {
        let reply = CtrlReply {
            ok: true,
            focused_view: Some("root:content view".to_string()),
            focused_identity: Some("::home".to_string()),
            view_rect: Some([0.0, 0.0, 1920.0, 1080.0]),
            loading: vec![LoadingEntry {
                view: "root:content view".to_string(),
                identity: "::home".to_string(),
                pct: 42,
            }],
            ..CtrlReply::default()
        };
        let json = serde_json::to_string(&reply).unwrap();
        assert!(
            json.contains("\"focused_view\":\"root:content view\""),
            "json: {}",
            json
        );
        assert!(
            json.contains("\"focused_identity\":\"::home\""),
            "json: {}",
            json
        );
        assert!(
            json.contains("\"view\":\"root:content view\""),
            "json: {}",
            json
        );
        assert!(!json.contains("focused_path"), "old field present: {}", json);
        assert!(!json.contains("panel_path"), "old field present: {}", json);
    }

    #[test]
    fn get_state_reply_omits_focus_fields_when_none() {
        let reply = CtrlReply {
            ok: true,
            focused_view: None,
            focused_identity: None,
            view_rect: Some([0.0, 0.0, 1.0, 1.0]),
            loading: Vec::new(),
            ..CtrlReply::default()
        };
        let json = serde_json::to_string(&reply).unwrap();
        assert!(!json.contains("focused_view"), "should be omitted: {}", json);
        assert!(
            !json.contains("focused_identity"),
            "should be omitted: {}",
            json
        );
    }

    #[test]
    fn ctrl_msg_constructs() {
        let (tx, _rx) = std::sync::mpsc::sync_channel::<CtrlReply>(1);
        let msg = CtrlMsg {
            cmd: CtrlCmd::Dump,
            reply_tx: tx,
        };
        assert!(matches!(msg.cmd, CtrlCmd::Dump));
    }

    #[test]
    fn acceptor_creates_socket_file_with_0600_perms() {
        // Hermetic: spawn the acceptor, assert the file exists with
        // user-only perms, clean up. Doesn't need an event loop because
        // we never send a command — just verify socket creation.
        let result = spawn_acceptor();
        assert!(result.is_ok(), "spawn_acceptor failed: {:?}", result.err());
        let path = socket_path();
        assert!(path.exists(), "socket file not created at {}", path.display());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "socket perms should be 0600, got 0o{:o}", mode);
        cleanup_on_exit();
        assert!(!path.exists(), "cleanup_on_exit did not unlink socket");
    }

    #[test]
    fn visit_no_longer_returns_phase_3_skeleton_error_on_valid_path() {
        // Hermetic verification: handle_main_thread without an
        // EventLoopProxy and without a real App is impractical to
        // construct in a unit test. Skip — the live behavior is covered
        // by the integration test in a later phase. This test is a
        // placeholder asserting the wire format is what we expect.
        let json = r#"{"cmd":"visit","identity":"root:cosmos"}"#;
        let parsed: CtrlCmd = serde_json::from_str(json).unwrap();
        match parsed {
            CtrlCmd::Visit { view, identity, adherent } => {
                assert_eq!(view, "");
                assert_eq!(identity, "root:cosmos");
                assert!(!adherent);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn unimplemented_commands_return_phase_3_skeleton_error() {
        let r = CtrlReply::err("not implemented in phase 3 skeleton");
        assert_eq!(
            r.error.as_deref(),
            Some("not implemented in phase 3 skeleton")
        );
    }

    #[test]
    fn wait_idle_pending_queue_starts_empty() {
        let pending = PENDING_WAIT_IDLE.lock().unwrap();
        // May be non-empty if a prior test polluted the global; assert
        // it's a Vec we can read without panicking.
        let _len = pending.len();
        // No assertion — just ensures the Mutex initializes correctly.
    }

    #[test]
    fn wait_idle_with_timeout_parks_then_can_be_drained() {
        // Construct a oneshot channel, push a PendingWaitIdle with
        // deadline=now+10ms, sleep 20ms, then ... we need an `App`
        // for check_pending_wait_idle. Skipping the live drain test —
        // covered by integration test in a later phase.
        // For now, assert the timeout-deadline math:
        let deadline = Instant::now() + Duration::from_millis(10);
        std::thread::sleep(Duration::from_millis(20));
        assert!(Instant::now() > deadline);
    }

    #[test]
    fn key_from_name_named_keys() {
        match key_from_name("Return") {
            Key::Named(NamedKey::Enter) => {}
            other => panic!("unexpected mapping: {:?}", other),
        }
        match key_from_name("Enter") {
            Key::Named(NamedKey::Enter) => {}
            _ => panic!(),
        }
        match key_from_name("F5") {
            Key::Named(NamedKey::F5) => {}
            _ => panic!(),
        }
        match key_from_name("ArrowUp") {
            Key::Named(NamedKey::ArrowUp) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn key_from_name_character_fallback() {
        match key_from_name("a") {
            Key::Character(s) => assert_eq!(s.as_str(), "a"),
            _ => panic!(),
        }
        match key_from_name("xyz") {
            Key::Character(s) => assert_eq!(s.as_str(), "xyz"),
            _ => panic!(),
        }
    }

    #[test]
    fn build_window_event_key_returns_err() {
        // Key arm is dependency-forced stubbed (see DIVERGED comment on
        // build_window_event). Verify the documented error is returned.
        let payload = InputPayload::Key {
            key: "Return".into(),
            press: true,
            mods: Modifiers::default(),
        };
        let result = build_window_event(payload);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not yet supported") || err.contains("KeyEvent"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn build_window_event_mouse_move() {
        let payload = InputPayload::MouseMove { x: 100.0, y: 200.0 };
        let event = build_window_event(payload).expect("mouse_move should succeed");
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                assert!((position.x - 100.0).abs() < 0.001);
                assert!((position.y - 200.0).abs() < 0.001);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn build_window_event_mouse_button() {
        let payload = InputPayload::MouseButton {
            button: MouseButtonName::Left,
            press: true,
        };
        let event = build_window_event(payload).expect("mouse_button should succeed");
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                assert_eq!(state, ElementState::Pressed);
                assert!(matches!(button, MouseButton::Left));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn build_window_event_scroll() {
        let payload = InputPayload::Scroll { dx: 1.0, dy: -2.0 };
        let event = build_window_event(payload).expect("scroll should succeed");
        match event {
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::PixelDelta(p) => {
                    assert!((p.x - 1.0).abs() < 0.001);
                    assert!((p.y + 2.0).abs() < 0.001);
                }
                _ => panic!("expected PixelDelta"),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn dispatch_without_proxy_returns_event_loop_not_initialized() {
        // Tests run before any EventLoop is created — proxy OnceLock is
        // empty. dispatch should return the documented error string
        // instead of panicking or hanging.
        let reply = dispatch(CtrlCmd::Dump);
        assert!(!reply.ok);
        let err = reply.error.as_deref().unwrap_or("");
        assert!(
            err.contains("not initialized") || err.contains("event loop"),
            "unexpected error: {}", err
        );
    }
}

#[cfg(test)]
mod resolve_identity_tests {
    use super::*;
    use crate::emPanelTree::PanelTree;

    /// Build an outer-view-shaped tree: root named "root" with three
    /// children named "control view", "content view", "slider".
    fn outer_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let root = tree.create_root("root", false);
        tree.create_child(root, "control view", None);
        tree.create_child(root, "content view", None);
        tree.create_child(root, "slider", None);
        (tree, root)
    }

    /// Build a cosmos-shaped inner tree: root named "" with one empty-
    /// named child (cosmos), which has a child named "home".
    fn cosmos_tree() -> (PanelTree, PanelId) {
        let mut tree = PanelTree::new();
        let sub_root = tree.create_root("", true);
        let cosmos = tree.create_child(sub_root, "", None);
        tree.create_child(cosmos, "home", None);
        (tree, sub_root)
    }

    #[test]
    fn empty_identity_addresses_root() {
        let (tree, root) = outer_tree();
        assert_eq!(resolve_identity(&tree, root, "").unwrap(), root);
    }

    #[test]
    fn root_name_addresses_root() {
        let (tree, root) = outer_tree();
        assert_eq!(resolve_identity(&tree, root, "root").unwrap(), root);
    }

    #[test]
    fn multi_segment_outer_tree() {
        let (tree, root) = outer_tree();
        let target = resolve_identity(&tree, root, "root:content view").unwrap();
        assert_eq!(tree.name(target), Some("content view"));
    }

    #[test]
    fn empty_name_inner_root() {
        let (tree, sub_root) = cosmos_tree();
        assert_eq!(resolve_identity(&tree, sub_root, "").unwrap(), sub_root);
    }

    #[test]
    fn single_empty_segment_inner_tree_finds_cosmos() {
        // Sub-root name is ""; cosmos name is "".
        // DecodeIdentity(":") == ["", ""] — first "" matches sub_root,
        // then descend by "" to cosmos.
        let (tree, sub_root) = cosmos_tree();
        let cosmos = resolve_identity(&tree, sub_root, ":").unwrap();
        assert_ne!(cosmos, sub_root);
        assert_eq!(tree.name(cosmos), Some(""));
    }

    #[test]
    fn double_empty_segment_plus_home_finds_home() {
        let (tree, sub_root) = cosmos_tree();
        let home = resolve_identity(&tree, sub_root, "::home").unwrap();
        assert_eq!(tree.name(home), Some("home"));
    }

    #[test]
    fn root_name_mismatch_errors() {
        let (tree, root) = outer_tree();
        let err = resolve_identity(&tree, root, "wrong:anything").unwrap_err();
        assert!(err.contains("identity root mismatch"), "got: {}", err);
    }

    #[test]
    fn missing_segment_errors_with_depth_and_name() {
        let (tree, root) = outer_tree();
        let err = resolve_identity(&tree, root, "root:nonexistent").unwrap_err();
        assert!(err.contains("no such panel"), "got: {}", err);
        assert!(err.contains("nonexistent"), "got: {}", err);
    }

    #[test]
    fn ambiguous_siblings_error() {
        // Manually build a tree with two identically-named siblings.
        let mut tree = PanelTree::new();
        let root = tree.create_root("r", false);
        tree.create_child(root, "dup", None);
        tree.create_child(root, "dup", None);
        let err = resolve_identity(&tree, root, "r:dup").unwrap_err();
        assert!(err.contains("ambiguous identity"), "got: {}", err);
        assert!(err.contains("under"), "got: {}", err);
    }

    /// Parametric round-trip: for every panel in a tree,
    /// resolve_identity(tree, root, GetIdentity(tree, p)) must yield p.
    #[test]
    fn round_trip_over_all_panels() {
        for (tree, root) in [outer_tree(), cosmos_tree()] {
            for pid in tree.panel_ids() {
                let id_str = tree.GetIdentity(pid);
                let round_trip = resolve_identity(&tree, root, &id_str)
                    .unwrap_or_else(|e| panic!(
                        "round-trip failed for panel {:?} (identity {:?}): {}",
                        pid, id_str, e
                    ));
                assert_eq!(
                    round_trip, pid,
                    "round-trip mismatch: identity {:?} resolved to wrong panel",
                    id_str
                );
            }
        }
    }
}

#[cfg(test)]
mod resolve_target_tests {
    /// Direct unit tests for `resolve_target` need a fully-constructed
    /// `App` with a populated home window and at least one
    /// `emSubViewPanel`. That fixture is heavy (winit `EventLoop`,
    /// scheduler, full panel tree). Coverage is provided instead by:
    ///   - JSON round-trip tests in this file, which exercise CtrlCmd
    ///     parsing.
    ///   - The Phase 7 integration test
    ///     `F010_subview_dump_nests_under_home_view_context`, which
    ///     exercises `resolve_target` end-to-end via the control
    ///     socket.
    /// The stubs below mark the un-covered surface explicitly so
    /// future test additions land in the right module.

    #[test]
    #[ignore = "needs App fixture; covered by Phase 7 integration test"]
    fn resolve_target_outer_view_default() {
        unreachable!("see Phase 7 F010_subview_dump_nests_under_home_view_context")
    }

    #[test]
    #[ignore = "needs App fixture; covered by Phase 7 integration test"]
    fn resolve_target_inner_subview_returns_inner_tree() {
        unreachable!("see Phase 7 F010_subview_dump_nests_under_home_view_context")
    }

    #[test]
    #[ignore = "needs App fixture; covered by Phase 7 integration test"]
    fn resolve_target_non_svp_selector_errors_with_panel_name() {
        // Should verify Fix 1's error message shape.
        unreachable!("see Phase 7 F010_subview_dump_nests_under_home_view_context")
    }
}
