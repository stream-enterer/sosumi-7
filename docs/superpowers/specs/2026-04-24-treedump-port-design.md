# 2026-04-24 — Tree Dump Port + Agent Control Channel

**Status:** design approved, ready for writing-plans
**Primary motivation:** unblock F010 (and subsequent runtime-observable bugs) by giving an autonomous agent the same state-introspection surface C++ has via `emTreeDump`, plus a way to drive the running app so reproduction does not require a human in the loop.

---

## Motivation

F010 has reached the blocked exit of the debug harness. All remaining hypotheses live in the paint/view layer and are observable only at runtime — a human must currently run the app, reproduce the symptom, and report what they see. Three of the four open questions (Q1, Q2, Q4 in `docs/debug/ISSUES.json#F010.blocked_question`) are state questions, not pixel questions: *"did Paint run", "is the panel in the viewed path", "is loading_done true"*. They are answerable mechanically from a tree snapshot taken at the right moment — no screenshot, no image interpretation.

C++ emCore ships `emTreeDump` for exactly this purpose: a cheat keystroke (`td!`) serializes the full object graph (context, view, window, panels, models) to a file. The package lives at `~/Projects/eaglemode-0.96.4/include/emTreeDump/` and `~/Projects/eaglemode-0.96.4/src/emTreeDump/`, and is invoked from `emViewInputFilter.cpp:746` via the `emTreeDumpFileFromRootContext` FFI entry point.

The Rust port already wires the cheat (`emViewInputFilter.rs:2441` → `CheatAction::TreeDump`; `emWindow.rs:1091` dispatch; `emView::dump_tree` at `emView.rs:4979` writes emRec to `$TMPDIR/debug.emTreeDump`), but the dump diverges from C++ in three load-bearing ways: (1) schema — generic `RecStruct` instead of the typed `emTreeDumpRec` shape (Frame, BgColor, FgColor, Title, Text, Commands, Files, Children) which the C++ `emTreeDumpFilePanel` uses to render saved dumps visually inside the app; (2) fields — missing `EssenceXYWH`, `IsInViewedPath`, `ViewedXYWH`, `ClipX1Y1X2Y2`, `EnableSwitch`, `InFocusedPath`, `UpdatePriority`, `MemoryLimit`, view-level `CurrentXYWH`, `ActivationAdherent`, `PoppedUp`, `BackgroundColor`, plus entire branches for `emWindow`, `emContext`, `emModel`, `emFileModel`, and the General Info root-context header; (3) subtype fields — the current Rust dump captures only generic panel fields, with no mechanism for `emDirPanel` (or any other behavior) to surface subtype-specific state like `loading_pct` and `loading_done` that F010 needs.

The dump alone is not sufficient for an autonomous agent. The agent must also be able to **drive the app into the state worth dumping**, which means navigating the view (zoom to `/home`), waiting for loading to complete, and capturing the dump at the right moment — without a human pressing keys. This document specifies both the dump extension and the control channel required to drive the app.

---

## Goals

- Rust tree dump matches the C++ `emTreeDumpRec` schema byte-for-byte (Frame / BgColor / FgColor / Title / Text / Commands / Files / Children) so a future port of `emTreeDumpFilePanel` can consume the same file.
- Rust dump covers every field emitted by C++'s `emTreeDumpFromObject` cascade: General Info root header, emEngine, emContext, emView, emWindow, emPanel, emModel, emFileModel.
- `PanelBehavior` gains a `dump_state` extension point so subtype-specific state (e.g., `emDirPanel` loading state) is visible in the dump without centralizing knowledge of concrete behaviors in `emcore`.
- Each `PanelData` carries a `paint_count` and `last_paint_frame`, bumped by the paint driver — correct-by-construction, invisible to behaviors, cheap at runtime.
- An opt-in Unix-domain control channel exposes: tree dump on demand, high-level navigation (`visit`, `visit_fullsized`, `set_focus`, `seek_to`), low-level synthetic input (`input`, `input_batch`), idle detection (`wait_idle`), lightweight state probe (`get_state`), and clean shutdown (`quit`).
- Control-channel input injection uses the same entry point winit uses (`App::window_event`), so synthetic input is indistinguishable from real input to everything downstream of that function.
- Existing `td!` cheat continues to work, writes to the same path, now carries the richer content.
- Zero code cost when the control channel is disabled; the socket, acceptor, and JSON stack do not run unless `EMCORE_DEBUG_CONTROL=1`.

## Non-goals

- Porting `emTreeDumpFilePanel` / `emTreeDumpFileModel` / `emTreeDumpRecPanel` / `emTreeDumpControlPanel` / `emTreeDumpFpPlugin`. The schema is faithful so this is a future mechanical port; it is not in this design.
- Rendering screenshots from the running app. F010 and the foreseeable next issues are state-observable; pixel-level observation can wait.
- Clipboard, IME composition, touch events, file-drop in the synthetic-input vocabulary.
- Multi-view / multi-window navigation addressing. The control channel operates on the currently-focused view; a design for multi-window addressing is a follow-up.
- An in-app UI for driving the channel. The interface is the Unix socket; humans can use `socat`/`nc`.
- A cargo feature gate. The gate is a runtime env var (see §Gate rationale).

---

## Design overview

Four coupled pieces, one gate:

- **(A) Tree dump extension** — new `crates/emcore/src/emTreeDump.rs` mirroring the C++ `emTreeDumpUtil.cpp` walker, emitting the `emTreeDumpRec` schema. Adds `PanelBehavior::dump_state`.
- **(B) Paint counter** — two `u64` fields on `PanelData`, bumped in the paint driver's single `behavior.Paint()` call site.
- **(C) Control channel** — new `crates/emcore/src/emCtrlSocket.rs`. Unix-domain socket, JSON-lines, acceptor thread + per-connection worker threads, main-thread dispatch via `winit::EventLoopProxy` + custom `UserEvent`.
- **(D) Input injection** — synthetic `WindowEvent` construction, dispatched by direct call into `App::window_event` on the main thread.

The gate: `EMCORE_DEBUG_CONTROL=1` at process start enables the control-channel acceptor. Pieces (A) and (B) are always on because `td!` needs (A) and (A) needs (B) for field completeness. (A) and (B) have negligible runtime cost (two `u64` adds per paint call; dump only runs when triggered).

---

## (A) Tree dump extension

### Schema

Matches `emTreeDumpRec` in `~/Projects/eaglemode-0.96.4/include/emTreeDump/emTreeDumpRec.h` exactly:

```
emTreeDumpRec {
    Frame: enum { FRAME_NONE=0, FRAME_RECTANGLE=1, FRAME_ROUND_RECT=2, FRAME_ELLIPSE=3, FRAME_HEXAGON=4 }
    BgColor: u32   // 0xRRGGBB — C++ emColor packed
    FgColor: u32
    Title:   String
    Text:    String            // multi-line; per-object fields concatenated in the C++ format
    Commands: [ { Caption: String, Args: [String] } ]   // always empty
    Files:    [String]                                   // always empty
    Children: [emTreeDumpRec]
}
```

Serialized via the existing `emRec` writer with format name `"emTreeDump"` (unchanged from current Rust, unchanged from C++). `Commands` and `Files` remain in the schema but empty — C++ populates them only inside `emTreeDumpFilePanel` / `emTreeDumpControlPanel`, neither of which is in scope here. Keeping the fields present means a future port reads the same file format.

### Root record — General Info

`emTreeDump::dump_from_root_context(root: &emRootContext) -> RecStruct` mirrors C++ `emTreeDumpFromRootContext` at `src/emTreeDump/emTreeDumpUtil.cpp:360`:

- **Title:** `"Tree Dump\nof the top-level objects\nof a running emCore-based program"`
- **Text:** time, host name, user name, process id, current directory, utf8 flag, byte order, `sizeof(ptr)`, `sizeof(long)`, signed/unsigned char, CPU-TSC, plus install paths (Bin, Include, Lib, HtmlDoc, PdfDoc, PsDoc, UserConfig, HostConfig, Tmp, Res, Home).
- **BgColor:** 0x444466, **FgColor:** 0xBBBBEE, **Frame:** FRAME_RECTANGLE — C++ constants preserved.
- **Children[0]:** the result of `dump_object(root_context)`.

Install-path equivalents are sourced from the existing Rust `emCoreConfig` / `emInstallInfo` paths — no new path logic.

### Object walker

C++ `emTreeDumpFromObject` uses a `dynamic_cast` cascade because it receives a polymorphic `emRecordable *`. Rust has no `dynamic_cast`, but also does not need it: each caller already knows the concrete type. The Rust walker replaces the cascade with named entry points that produce the same Frame/Bg/Fg/Title/Text for each object kind:

```rust
fn dump_object_engine(e: &dyn emEngineLike) -> (FieldAppend, VisualStyle) { ... }
fn dump_context(c: &emContext) -> RecStruct { ... }
fn dump_view(v: &emView, tree: &mut PanelTree) -> RecStruct { ... }
fn dump_window(w: &emWindow) -> RecStruct { ... }
fn dump_panel(p: &PanelData, b: &dyn PanelBehavior, tree: &mut PanelTree, current_frame: u64) -> RecStruct { ... }
fn dump_model(m: &emModel) -> RecStruct { ... }
fn dump_file_model(fm: &emFileModel) -> RecStruct { ... }
```

C++ accumulates fields from every applicable cast (a view is also a context is also an engine, so Text contains engine fields + context fields + view fields in that order). The Rust walker preserves that accumulation order — `dump_view` begins by appending engine fields, then context fields, then view fields, producing byte-identical Text for equivalent scenes.

Visual style per C++ constants:
- emEngine: Bg 0x000000, Fg 0xEEEEEE, Frame RECTANGLE.
- emContext: Bg 0x777777, Fg 0xEEEEEE, Frame ELLIPSE (root) / ELLIPSE (child).
- emView: Bg 0x448888, Fg 0xEEEEEE (0xEEEE44 if focused), Frame ROUND_RECT.
- emWindow: Bg 0x222288 (overlays view; view branch still runs).
- emPanel: Bg per state (viewed 0x338833 / in_viewed_path 0x225522 / else 0x445544), Fg per state (in_focused_path 0xEEEE44 / in_active_path 0xEEEE88 / else 0xEEEEEE), Frame RECTANGLE.
- emModel: Bg 0x440000, Fg 0xBBBBBB, Frame HEXAGON.
- emFileModel: Bg 0x440033, Fg 0xBBBBBB, Frame HEXAGON (overlays emModel).

### Per-object field sets

Each Text block mirrors the C++ `emString::Format` call in `emTreeDumpUtil.cpp` line-for-line. For brevity, the fields only (labels reproduced verbatim from C++):

- **emEngine:** `Engine Priority`.
- **emContext:** `Common Models: N`, `Private Models: N (not listed)`; children include sorted common models and every child context.
- **emView:** `View Flags` (enumerated names: VF_POPUP_ZOOM, VF_ROOT_SAME_TALLNESS, VF_NO_ZOOM, VF_NO_USER_NAVIGATION, VF_NO_FOCUS_HIGHLIGHT, VF_NO_ACTIVE_HIGHLIGHT, VF_EGO_MODE, VF_STRESS_TEST; `0` if none), `Title`, `Focused`, `Activation Adherent`, `Popped Up`, `Background Color: 0xRRGGBBAA`, `Home XYWH`, `Current XYWH`. One child = the root panel.
- **emWindow:** `Window Flags` (WF_MODAL, WF_UNDECORATED, WF_POPUP, WF_FULLSCREEN), `WMResName`.
- **emPanel:** `Name`, `Title`, `Layout XYWH`, `Height`, `Essence XYWH`, `Viewed`, `InViewedPath`, `Viewed XYWH` (or `-` if not viewed), `Clip X1Y1X2Y2` (or `-`), `EnableSwitch`, `Enabled`, `Focusable`, `Active`, `InActivePath`, `Focused`, `InFocusedPath`, `Update Priority`, `Memory Limit`, `PaintCount`, `LastPaintFrame: N (current: M)`. Children: every panel child in order.
- **emModel:** `Name`, `Min Common Lifetime`.
- **emFileModel:** `File Path`, `File State` (`FS_WAITING` / `FS_LOADING` / `FS_LOADED` / `FS_UNSAVED` / `FS_SAVING` / `FS_TOO_COSTLY` / `FS_LOAD_ERROR` / `FS_SAVE_ERROR`), `Memory Need`.

Any field the Rust port does not yet have a source for (e.g., CPU-TSC if no equivalent exists) is rendered as the label plus `"-"` or `"n/a"`, with a `DIVERGED:` comment naming the upstream-gap category.

### `PanelBehavior::dump_state`

```rust
/// Subtype-specific fields appended to the emPanel Text block, in insertion order.
/// Labels match the per-object field style ("Key: value"). Default: empty.
///
/// Rust analog of C++'s dynamic_cast cascade in emTreeDumpFromObject:
/// each concrete panel class in C++ adds its own fields via a centralized cascade.
/// Rust decentralizes this because PanelBehavior is the unifying trait C++ lacks.
fn dump_state(&self) -> Vec<(&'static str, String)> { vec![] }
```

Initial overrides (in their existing files):
- `emDirPanel::dump_state` — `loading_pct`, `loading_done`, `loading_cycle_state`, `entries_count`, `error_state_if_any`.
- `emFilePanel::dump_state` — `file_state`, `file_path`, `memory_need`.
- Other behaviors that have debuggable state add overrides as needed; list extends during implementation.

Each pair `(label, value)` appends to the panel's Text as `"\n<label>: <value>"`. Order is insertion order. No sorting.

### File layout

- **New:** `crates/emcore/src/emTreeDump.rs` — walker, entry points, visual-style constants.
- **Modified:** `crates/emcore/src/emPanel.rs` — add `dump_state` to `PanelBehavior`, add `paint_count` / `last_paint_frame` to `PanelData`.
- **Modified:** `crates/emcore/src/emView.rs` — `dump_tree` becomes a thin shim: resolves `root_context`, calls `emTreeDump::dump_from_root_context`, writes to `$TMPDIR/debug.emTreeDump`. No walker logic in this file.
- **Modified:** `crates/emfileman/src/emDirPanel.rs`, `crates/emfileman/src/emFilePanel.rs`, etc. — override `dump_state` per the list above.
- **Marker files** (`.no_rs`): `emTreeDumpRec.no_rs`, `emTreeDumpFileModel.no_rs`, `emTreeDumpFilePanel.no_rs`, `emTreeDumpRecPanel.no_rs`, `emTreeDumpControlPanel.no_rs`, `emTreeDumpFpPlugin.no_rs` — document that the in-app renderer is not ported and why (future work; schema is faithful so the port is mechanical).

---

## (B) Paint counter

### Fields on `PanelData`

```rust
// RUST_ONLY: (language-forced utility)
// C++ relies on gdb for per-panel paint inspection; the Rust port lacks
// that path, so paint attribution is baked into the data model. Bumped by
// the paint driver, never by behaviors — correct-by-construction.
pub(crate) paint_count: u64,
pub(crate) last_paint_frame: u64,
```

Two `u64` per panel. Cost: 16 bytes × panel count. For a typical emCore scene this is <1 KB — immaterial.

### Framework frame counter

A `u64` on `emView` (or wherever the view's per-frame paint driver currently lives), incremented once per complete paint pass. Monotonic, wrapping on overflow (`wrapping_add(1)`). Starts at 0. The exact location is determined during implementation by the view's current paint-driver call site; the spec does not pre-commit.

### Increment site

The single function that invokes `behavior.Paint()`. Before the call:

```rust
panel.paint_count = panel.paint_count.wrapping_add(1);
panel.last_paint_frame = view.current_frame;
behavior.Paint(painter, w, h, state);
```

Behaviors never touch the counter. The default no-op `Paint()` still counts as "painted" — the correct semantics: the view considered the panel eligible and invoked its Paint method. If a panel is skipped this frame (e.g. behind an `IsOpaque` sibling), `last_paint_frame` stays at its previous value and diverges from `view.current_frame`.

### Dump integration

In the emPanel branch of the walker, append to Text:

```
PaintCount: 42
LastPaintFrame: 1337 (current: 1340)
```

Showing `current` alongside `LastPaintFrame` makes "painted N frames ago" answerable at a glance.

---

## (C) Control channel

### Gate

`EMCORE_DEBUG_CONTROL=1` in the process environment at startup. Checked once, early in `emGUIFramework` initialization. When unset: acceptor thread is not spawned, socket file is not created, `serde_json` parsing does not run. The dead code still ships in the binary — we accept this for the agent-UX win of not needing a rebuild. (Rationale: the debug-build / release-build separation does not help an autonomous agent iterate; a single binary with a runtime gate is strictly better for my workflow.)

### Socket

Path: `$TMPDIR/eaglemode-rs.<pid>.sock`. On startup, after winit event loop is created: stale socket at that path is unlinked (the PID-namespaced path makes collision with another running instance impossible; a file at this path means the previous process did not shut down cleanly). Socket is created with mode `0600` (user-only). Path is logged to stderr: `[emCtrlSocket] listening on <path>`. On clean shutdown and on the acceptor thread's exit, the socket is unlinked.

### Protocol

Newline-delimited JSON (JSON-lines). One command per line, one reply per line. Strict sync: the worker does not accept the next command until it has sent the reply for the current one. UTF-8. Example transcript:

```
→ {"cmd":"visit","panel_path":"/cosmos/home"}
← {"ok":true}
→ {"cmd":"wait_idle","timeout_ms":30000}
← {"ok":true,"idle_frame":1840}
→ {"cmd":"dump"}
← {"ok":true,"path":"/tmp/debug.emTreeDump"}
→ {"cmd":"get_state"}
← {"ok":true,"focused_path":"/cosmos/home","view_rect":[0,0,1920,1080],"loading":[]}
→ {"cmd":"quit"}
← {"ok":true}
```

### Command set

- **`dump`** — invoke the Section-A walker, write to `$TMPDIR/debug.emTreeDump`, return `{ok, path}`.
- **`visit { panel_path, adherent?: bool }`** — resolve `panel_path` (see §Path resolution); call the Rust analog of `emView::Visit` on the currently-focused view with the resolved panel. Reply when the call returns (not when animation settles — use `wait_idle` for that).
- **`visit_fullsized { panel_path }`** — as above, `VisitFullsized`.
- **`set_focus { panel_path }`** — resolve, call `emView::Focus` (or equivalent).
- **`seek_to { panel_path }`** — uses the existing seek engine (`emView::Seek` equivalent). Resolves immediately even if the target is not yet materialized; the seek engine drives loading.
- **`input { event: InputPayload }`** — synthesize and dispatch one `WindowEvent`. See §(D).
- **`input_batch { events: [InputPayload, ...] }`** — dispatch N events in order, one main-thread hop, one reply.
- **`wait_idle { timeout_ms?: u64 }`** — block the worker until the main-thread scheduler is idle (§Idle detection) or the timeout expires. Reply: `{ok, idle_frame}` or `{ok:false, error:"timeout"}`.
- **`get_state`** — synchronous, non-file probe. Returns `{ok, focused_path, view_rect:[x,y,w,h], loading:[{panel_path, pct}, ...]}`. For polling between `visit` and `wait_idle` without the weight of a full dump.
- **`quit`** — clean shutdown: reply first (flushes), then signal the event loop to exit. Acceptor thread unlinks socket, exits.

### Threading model

One acceptor thread (spawned at framework init if gate is set). Accepts connections; spawns a worker thread per connection. Worker:

1. Reads a JSON line.
2. Parses into `CtrlCmd` enum.
3. Constructs `UserEvent::Ctrl { cmd: CtrlCmd, reply_tx: SyncSender<CtrlReply> }`. `reply_tx` is the sender of a `std::sync::mpsc::sync_channel(1)`.
4. `event_loop_proxy.send_event(user_event)` — wakes the main thread.
5. `reply_rx.recv()` — blocks until the main thread replies.
6. Serializes reply as JSON, writes to socket.
7. Loops.

Main thread receives the user event in `App::user_event` (winit 0.30+ `ApplicationHandler<T>`; `type UserEvent = CtrlMsg` added to the impl). Dispatches on `cmd`, executes synchronously on the main thread (all app state mutation stays single-threaded), sends reply. For `wait_idle`, the command is parked in a main-thread queue checked every `about_to_wait`; reply is sent when `scheduler.is_idle()` or timeout.

No tokio, no async. Standard library only. serde + serde_json for (de)serialization.

### Winit integration

- `type UserEvent = CtrlMsg` on `App` in `emGUIFramework.rs`.
- `event_loop.create_proxy()` captured during `App::new` / framework init, stored in a `OnceLock<EventLoopProxy<CtrlMsg>>` accessible to the acceptor thread.
- `App::user_event(&mut self, event_loop: &ActiveEventLoop, user_event: CtrlMsg)` added; matches on `CtrlMsg` variants and dispatches.
- Existing `ControlFlow` usage: unchanged. `wait_idle` does not need `ControlFlow::Poll` — the scheduler already drives animation frames when active, and `about_to_wait` runs between every pass, so the idle-check tick is already there.

### Idle detection

New method `emScheduler::is_idle(&self, view: &emView) -> bool`, true when **all** of:

- Scheduler engine queue is empty.
- No panel has pending notice flags.
- No pending AutoExpand work queued.
- No active view animator (magnetic, speedo, etc.).
- No window has a pending redraw request.

Checked in `about_to_wait`. If a `wait_idle` reply is pending, its deadline is checked; when either `is_idle()` returns true (reply `{ok, idle_frame}`) or deadline expires (reply `{ok:false, error:"timeout"}`), the `reply_tx` is sent and the pending entry is removed.

### View selection and path resolution

"Currently-focused view" means the view returned by iterating the framework's registered views and selecting the one whose `IsFocused()` is true. If none is focused (no window has focus), commands that target a view fail with `{ok:false, error:"no focused view"}`. If exactly one top-level view exists (the common case for the main app), it is used unconditionally.

Panel paths use `/` as separator. Root is `/`. `/cosmos/home` = `root_panel.child_by_name("cosmos").child_by_name("home")`. Starts from the selected view's root panel. Missing segment → `{ok:false, error:"no such panel: /cosmos/foo (segment 'foo' not found under 'cosmos')"}`.

Panel names in emCore can in principle contain any string. For the initial scope we assume names do not contain `/`; if a name does, resolution may ambiguously match. Detect and error if an unescaped `/` appears in a child's name during traversal. A richer escape syntax is a follow-up.

### Error handling

- Malformed JSON: `{ok:false, error:"parse: <serde_json message>"}`. Connection stays open.
- Unknown `cmd`: `{ok:false, error:"unknown command: <cmd>"}`.
- Socket I/O error on write: worker exits, connection closes. Acceptor continues.
- Main-thread panic during dispatch: `reply_tx` drops, worker sees `RecvError`, replies `{ok:false, error:"main thread aborted"}`, connection closes. Acceptor continues until the process actually exits.
- Invalid panel path: see §Path resolution.
- `visit` when no focused view exists: `{ok:false, error:"no focused view"}`.

### File layout

- **New:** `crates/emcore/src/emCtrlSocket.rs` — acceptor, worker, `CtrlCmd` / `CtrlReply` enums, JSON (de)serialization, socket lifecycle. Top-of-file annotation: `RUST_ONLY:` category `language-forced utility — no C++ analogue; agent-driven debugging requires a programmatic channel that C++'s GUI-only cheat codes do not provide`.
- **New marker file:** `emCtrlSocket.rust_only`.
- **Modified:** `crates/emcore/src/emGUIFramework.rs` — add `type UserEvent = CtrlMsg`, `App::user_event` method, `OnceLock<EventLoopProxy<CtrlMsg>>`, gate check + acceptor spawn.
- **Modified:** `crates/emcore/Cargo.toml` — add `serde = { workspace = true, features = ["derive"] }` and `serde_json = { workspace = true }` (add to workspace `Cargo.toml` if not already present).

---

## (D) Input injection

### Payload

```rust
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum InputPayload {
    Key { key: String, press: bool, mods: Modifiers },
    MouseMove { x: f64, y: f64 },
    MouseButton { button: MouseButtonName, press: bool },
    Scroll { dx: f64, dy: f64 },
}

#[derive(serde::Deserialize, Default)]
struct Modifiers {
    #[serde(default)] shift: bool,
    #[serde(default)] ctrl: bool,
    #[serde(default)] alt: bool,
    #[serde(default)] logo: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum MouseButtonName { Left, Middle, Right }
```

`key` values are winit key names: `"Return"`, `"Escape"`, `"F1"`, `"a"`, `"ArrowLeft"`, etc. Unknown name → command errors.

### Entry point

`emCtrlSocket::synthesize_and_dispatch(app: &mut App, window_id: WindowId, payload: InputPayload)`:

1. Construct the matching `winit::event::WindowEvent` variant — `KeyboardInput`, `CursorMoved`, `MouseInput`, `MouseWheel`.
2. Call `app.window_event(event_loop, window_id, event)` directly. This is the same method winit itself calls; downstream handling is identical.

No attempt to re-enter winit. The direct method call is safe because `App::window_event` has no implicit dependency on being called from inside winit's internal event pump — it operates on `self` and `event_loop`, both of which are available in the `user_event` handler.

### Coordinates

`MouseMove { x, y }` are logical pixel coordinates in the window's client area (winit `PhysicalPosition` with scale factor applied — the same space winit delivers in `CursorMoved`). `get_state` includes the current window size so the agent can map panel coords → window coords if needed.

### Out of scope

- Clipboard events.
- IME (`Ime::Preedit`, `Ime::Commit`).
- Touch.
- File drop / drag.
- Pointer-capture semantics (assumes cursor is always at the position from the most recent `MouseMove`).

---

## Headless operation

The agent needs `eaglemode` to launch without a human-attended desktop. Approach:

1. **Verify Xvfb path first.** Run the binary under `Xvfb :99 & DISPLAY=:99 eaglemode`. Expected to work because wgpu on Linux supports llvmpipe, and winit supports X11. If this works, no app changes are required — the agent sets `DISPLAY` in its environment.
2. **Fallback: `--headless` flag.** If (1) fails (GPU init refuses without a real display, or some other X dep is missing), add `--headless` which creates an offscreen wgpu surface and a dummy winit event source. Larger, tracked as a follow-up if needed.

This verification happens as the first implementation milestone of piece (C) — before any control-channel code is written, confirm the binary launches under Xvfb. If (1) fails, the design adds `--headless` as a required piece of (C); if (1) works, `--headless` is out of scope.

**Verification status (2026-04-24):** Xvfb is **not installed** on the development machine where Phase 3 implementation is occurring. Verification deferred — phase 3 control-channel code lands without runtime gating, and integration tests that spawn the binary are marked `#[ignore]` so they can be enabled by anyone who has Xvfb (or a real display) available. If a future session reaches the runtime-driving stage and Xvfb is still unavailable, escalate to a `--headless` decision then. For the immediate goal (F010 unblocking), the agent can drive the app from a desktop session — Xvfb is a CI-friendly nice-to-have, not a blocker.

---

## Annotation summary

- `crates/emcore/src/emTreeDump.rs` — new file; name correspondence to `src/emTreeDump/emTreeDumpUtil.cpp`; no `RUST_ONLY:` needed (1:1 with C++). If any field is rendered as `"-"` due to upstream gaps in the Rust port, tag the site `DIVERGED: (upstream-gap-forced) <field> — not yet ported in Rust emCore; C++ emits via <source>`.
- `crates/emcore/src/emPanel.rs::PanelData::paint_count` / `last_paint_frame` — `RUST_ONLY:` category **language-forced utility**.
- `crates/emcore/src/emPanel.rs::PanelBehavior::dump_state` — no annotation needed; it is the Rust analog of the C++ dynamic_cast cascade and preserves observable output, so it qualifies as idiom adaptation (unannotated per Annotation Vocabulary).
- `crates/emcore/src/emCtrlSocket.rs` — `RUST_ONLY:` at top of file, category **language-forced utility** with the reason noted in §(C) file layout.
- Marker files per §(A) file layout.

## Testing summary

### Unit tests

- **Tree dump schema:** construct a minimal view+panel tree, call `dump_from_root_context`, parse the written emRec, assert every expected key is present with expected type.
- **Tree dump field fidelity:** table-driven test per object kind (context, view, window, panel, model, file_model) verifying Frame/BgColor/FgColor and Text label strings match the values in `emTreeDumpUtil.cpp`.
- **Paint counter:** panel with N `Paint()` calls reports `paint_count == N`. Panel skipped this frame (IsOpaque sibling) reports `last_paint_frame < current_frame`.
- **`dump_state` override:** behavior returning `vec![("foo", "42".into())]` appears in dump Text as `"\nfoo: 42"`.
- **Idle detection:** scheduler with queued cycles reports `is_idle() == false`; empty scheduler reports true. Animator active / inactive cases.
- **JSON protocol:** round-trip each `CtrlCmd` variant and each `CtrlReply` variant.
- **Path resolution:** valid path, missing segment, unescaped `/` in a child name.
- **Input synthesis:** each `InputPayload` variant produces the expected `WindowEvent` variant and fields.

### Integration tests

- **Control-channel end-to-end:** spawn the binary with `EMCORE_DEBUG_CONTROL=1` under Xvfb, connect to the socket, send `dump`, assert the file exists and parses as emRec `emTreeDump`.
- **Navigate-and-dump:** spawn, `visit /cosmos` then `wait_idle` then `dump`, assert the dumped `focused_path` reflects the visit.
- **Quit:** `quit` causes process exit within 2 s; socket file is unlinked.
- **Input batch:** `input_batch` of 5 key events causes 5 focus moves observable in `get_state` before/after.

### Golden tests

Not applicable to this work — tree dump and control channel do not affect the pixel-output surface the golden suite covers.

---

## Files created / modified (summary)

**Created:**
- `crates/emcore/src/emTreeDump.rs`
- `crates/emcore/src/emCtrlSocket.rs`
- `crates/emcore/src/emCtrlSocket.rust_only`
- `crates/emcore/src/emTreeDumpRec.no_rs`
- `crates/emcore/src/emTreeDumpFileModel.no_rs`
- `crates/emcore/src/emTreeDumpFilePanel.no_rs`
- `crates/emcore/src/emTreeDumpRecPanel.no_rs`
- `crates/emcore/src/emTreeDumpControlPanel.no_rs`
- `crates/emcore/src/emTreeDumpFpPlugin.no_rs`

**Modified:**
- `crates/emcore/src/emPanel.rs` — `paint_count`, `last_paint_frame`, `dump_state`.
- `crates/emcore/src/emView.rs` — `dump_tree` becomes shim; frame counter field + increment.
- `crates/emcore/src/emGUIFramework.rs` — `UserEvent` assoc type, `user_event` handler, proxy storage, gate check + acceptor spawn, paint-driver counter increment.
- `crates/emcore/src/emScheduler*.rs` — `is_idle` method.
- `crates/emcore/Cargo.toml` — serde / serde_json deps.
- `Cargo.toml` (workspace) — serde / serde_json workspace entries if missing.
- `crates/emfileman/src/emDirPanel.rs` — `dump_state` override.
- `crates/emfileman/src/emFilePanel.rs` — `dump_state` override.
- Other `PanelBehavior` impls as needed for completeness (list settled during implementation).

---

## Out of scope / follow-ups

- Port of `emTreeDumpFilePanel` + `emTreeDumpFileModel` + `emTreeDumpControlPanel` + `emTreeDumpRecPanel` + `emTreeDumpFpPlugin` (render dumps visually inside the app). Schema fidelity in this design keeps that a mechanical future port.
- `--headless` flag, if Xvfb verification fails.
- Multi-window / multi-view path addressing (currently: operates on focused view).
- Richer input synthesis (clipboard, IME, touch, file drop).
- Screenshot capability via the control channel (dump wgpu framebuffer to PNG).
- Deterministic replay mode (seeded animations, scripted input file).
- Panel-name escape syntax in paths.
- Reinstating the `td!` cheat-code behavior to also trigger a dump via the control channel (currently `td!` just writes the file; it does not notify any listeners).

---

## Implementation ordering (for writing-plans)

Suggested phasing; writing-plans will finalize:

1. **Piece A + B, no control channel.** `emTreeDump.rs`, `PanelBehavior::dump_state`, paint counter, frame counter, shim `dump_tree`. `td!` cheat exercises it. Useful on its own for any human-driven debugging session. Unblocks nothing for F010 but is foundation.
2. **Headless verification (Xvfb).** Before piece C, confirm the binary launches under Xvfb. Sets whether `--headless` is needed.
3. **Piece C skeleton — dump only.** Gate, socket, acceptor, JSON protocol, `dump` + `get_state` + `quit` commands. No navigation yet. Agent can dump on demand.
4. **Piece C — navigation.** `visit`, `visit_fullsized`, `set_focus`, `seek_to`, `wait_idle`, `is_idle` on scheduler. Agent can drive the app end-to-end.
5. **Piece D — low-level input.** `input`, `input_batch`. Completes the control surface.
6. **F010 re-engagement.** With the new tooling, re-enter the debug harness on F010; expect resolution or a new root-cause hypothesis grounded in dump evidence.
