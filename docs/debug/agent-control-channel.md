# Agent Control Channel

Programmatic control surface for driving the running `eaglemode` binary from outside the process. Built for autonomous agents but usable from a shell with `socat`. Replaces the "ask a human to repro and report" loop with "drive the app, dump state, read the file".

The channel is opt-in and OS-quiet when off — no socket file, no acceptor thread, no JSON code path runs unless gated on.

## Quick start

```bash
EMCORE_DEBUG_CONTROL=1 cargo run --bin eaglemode &
APP_PID=$!
SOCK=/tmp/eaglemode-rs.${APP_PID}.sock
while [ ! -S "$SOCK" ]; do sleep 0.1; done

# Zoom outer view → content sub-view panel
printf '{"cmd":"visit","identity":"root:content view"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":30000}\n'         | socat -t35 - UNIX-CONNECT:$SOCK
# Zoom content sub-view → home directory inside cosmos
printf '{"cmd":"visit","view":"root:content view","identity":"::home"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":30000}\n'         | socat -t35 - UNIX-CONNECT:$SOCK
printf '{"cmd":"dump"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
cat /tmp/debug.emTreeDump | head -80

# Clean shutdown
printf '{"cmd":"quit"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
wait $APP_PID
```

(`nc` on Fedora is `ncat` and behaves differently from BSD/OpenBSD `nc -U`; use `socat` — it is the canonical client now.)

Works under a real display or under `Xvfb :99 -screen 0 1920x1080x24 & DISPLAY=:99 …`.

## Wire protocol

Unix-domain socket at `$TMPDIR/eaglemode-rs.<pid>.sock`. Mode `0600`. JSON-lines: one command per line, one reply per line, strict synchronous (next command waits for previous reply). Connections are independent — open as many as you want; each gets its own worker thread.

Replies follow the shape `{"ok": true, ...}` or `{"ok": false, "error": "<message>"}`. Optional fields are omitted when absent so the simple success case is `{"ok":true}`.

### Commands

| Command           | Payload                                                  | Reply fields                                                                  | Notes                                                                                                            |
|-------------------|----------------------------------------------------------|-------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------|
| `dump`            | —                                                        | `path`                                                                        | Writes full tree dump to `$TMPDIR/debug.emTreeDump` (emRec format `emTreeDump`). Returns the absolute path.       |
| `get_state`       | —                                                        | `focused_view`, `focused_identity`, `view_rect: [x, y, w, h]`, `loading: []`  | Lightweight probe; no file write. The pair `(focused_view, focused_identity)` round-trips into `visit`.           |
| `quit`            | —                                                        | —                                                                             | Replies first, then signals the event loop to exit. Socket file is unlinked on exit.                              |
| `visit`           | `view?: String, identity: String, adherent?: bool`       | —                                                                             | `emView::VisitPanel` on the resolved view. `view` defaults to `""` (outer view). `adherent` forwards to C++.      |
| `visit_fullsized` | `view?: String, identity: String`                        | —                                                                             | `emView::VisitFullsized` (utilize_view = false) on the resolved view.                                             |
| `set_focus`       | `view?: String, identity: String`                        | —                                                                             | Sets the focused panel on the resolved view without animating.                                                    |
| `seek_to`         | `view?: String, identity: String`                        | —                                                                             | Currently delegates to `VisitPanel` on the resolved view; true seek-engine wiring is a follow-up.                 |
| `wait_idle`       | `timeout_ms?: u64`                                       | `idle_frame: u64` (or `error: "timeout"`)                                     | Parks reply until `EngineScheduler::is_idle()`. Drained from `about_to_wait`. Returns the view's frame on resolve.|
| `input`           | `event: InputPayload`                                    | —                                                                             | One synthetic `WindowEvent` dispatched through `App::window_event`. See *Input* below.                            |
| `input_batch`     | `events: [InputPayload, ...]`                            | —                                                                             | N events, one round-trip. Stops on first error.                                                                   |

### Identity addressing

Panels are addressed by `(view, identity)`. Identities come from `emPanel::GetIdentity` — colon-separated, backslash-escaped via `EncodeIdentity`/`DecodeIdentity` in `emPanelTree.rs:71-142`. The `view` selector is the outer-view identity of the containing `emSubViewPanel`; omit it (or pass `""`) to address the outer view itself.

Examples from the live emMainPanel tree:

| Target                                            | `view`               | `identity`            |
|---------------------------------------------------|----------------------|-----------------------|
| Outer view's root (emMainPanel)                   | `""`                 | `"root"`              |
| Control sub-view panel                            | `""`                 | `"root:control view"` |
| Content sub-view panel                            | `""`                 | `"root:content view"` |
| Slider                                            | `""`                 | `"root:slider"`       |
| Inner content view's root (sub-tree root)         | `"root:content view"`| `""`                  |
| Cosmos panel (sub-root's empty-named child)       | `"root:content view"`| `":"`                 |
| Home directory under cosmos                       | `"root:content view"`| `"::home"`            |

Empty `identity` addresses the view's root. Empty segments (`""`) are addressable — they encode unnamed panels, which emCore uses for "the singleton child of a parent that doesn't need to be addressed by name" (both C++ `emMainContentPanel.cpp:29` and Rust `emMainWindow.rs:588` give cosmos the empty name).

### Input payload

```json
{"kind": "key",          "key": "Return",   "press": true, "mods": {"shift": false, "ctrl": false}}
{"kind": "mouse_move",   "x": 100.0, "y": 200.0}
{"kind": "mouse_button", "button": "left",  "press": true}
{"kind": "scroll",       "dx": 0.0,  "dy": -120.0}
```

`button` ∈ `left | middle | right`. `mods` is optional; defaults to all-false. Key names: `Return`/`Enter`, `Escape`, `Tab`, `Space`, `Backspace`, `Arrow{Up,Down,Left,Right}`, `Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`. Anything else falls back to `Key::Character(name)`.

**Today, only mouse / scroll / cursor events dispatch end-to-end.** Synthesized key events return an error — see *Known gaps*.

## What's in a dump

`/tmp/debug.emTreeDump` is an emRec file with format name `emTreeDump`. Schema (matches C++ `emTreeDumpRec`):

```
Frame:     FRAME_RECTANGLE | FRAME_ROUND_RECT | FRAME_ELLIPSE | FRAME_HEXAGON | FRAME_NONE
BgColor:   0xRRGGBB
FgColor:   0xRRGGBB
Title:     <multi-line; first line names the object kind>
Text:      <multi-line; per-object fields concatenated>
Commands:  []          # always empty in this port
Files:     []          # always empty in this port
Children:  [emTreeDumpRec, ...]
```

The root rec carries the General Info header (time, host, user, pid, cwd, install paths). Children walk Context → View → Panel tree. Each emPanel rec's `Text` includes:

```
Name, Title, Layout XYWH, Height, Essence XYWH,
Viewed (yes/no), InViewedPath (yes/no), Viewed XYWH (or '-'),
Clip X1Y1X2Y2 (or '-'), EnableSwitch, Enabled, Focusable,
Active, InActivePath, Focused, InFocusedPath,
Update Priority, Memory Limit,
PaintCount, LastPaintFrame: N (current: M),
<subtype-specific pairs from PanelBehavior::dump_state>
```

`PaintCount` and `LastPaintFrame` are RUST_ONLY additions. They answer "did Paint run on this panel?" and "how recently?". The frame counter is monotonic and only advances when at least one panel was painted in a pass.

`PanelBehavior::dump_state` overrides surface subtype-specific fields. Current overrides:

- **emDirPanel:** `path`, `loading_done`, `loading_error`, `content_complete`, `child_count`, `has_dir_model`, `scroll_target`. (This is the F010 evidence surface.)
- **emFilePanel:** `has_file_model`, `custom_error`, `last_vir_file_state`, `cached_memory_limit`, `cached_priority`, `cached_in_active_path`.

Add overrides on more behaviors as new debug needs surface — the hook is a one-method trait override, no central registration.

## Triggers

The dump can be produced four ways:

1. **`td!` cheat code** — type the cheat in the running app (existing C++-port mechanism). Writes the same file via the same walker.
2. **`dump` command on the control socket** — programmatic trigger. Returns the path.
3. **Direct call** — `view.dump_tree(&mut tree)` from any code holding the view.
4. **Library FFI symbol** (existing emTreeDump-package marker, not invoked yet) — reserved for future.

All four write to `$TMPDIR/debug.emTreeDump` and produce identical output.

## End-to-end recipes

### F010-style: zoom into a directory and see why it renders blank

```bash
EMCORE_DEBUG_CONTROL=1 cargo run --bin eaglemode &
APP_PID=$!
SOCK=/tmp/eaglemode-rs.${APP_PID}.sock
while [ ! -S "$SOCK" ]; do sleep 0.1; done

# Snapshot before
printf '{"cmd":"dump"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
cp /tmp/debug.emTreeDump /tmp/before.emTreeDump

# Two-call navigation: outer → content SVP, then inner → home
printf '{"cmd":"visit","identity":"root:content view"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":30000}\n'         | socat -t35 - UNIX-CONNECT:$SOCK
printf '{"cmd":"visit","view":"root:content view","identity":"::home"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
printf '{"cmd":"wait_idle","timeout_ms":30000}\n'         | socat -t35 - UNIX-CONNECT:$SOCK

# Snapshot after loading completes
printf '{"cmd":"dump"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
cp /tmp/debug.emTreeDump /tmp/after.emTreeDump
```

In `/tmp/after.emTreeDump`, look for the `emDirPanel` rec under the inner content view's branch — each sub-view appears as a child context of the home view per Phase 0's nested topology fix. Read:
- `loading_done: true` — the load reached completion.
- `child_count: N` — entries were created.
- Each `emDirEntryPanel` child should have `LastPaintFrame: ≈current` if visible.
- If `Viewed: yes` but `LastPaintFrame: 0` or far behind current, Paint isn't running.
- If `child_count: 0` despite `loading_done: true`, the load completed but produced no entries.

### Polling pattern (no `wait_idle` needed)

```bash
while true; do
  reply=$(printf '{"cmd":"get_state"}\n' | socat -t2 - UNIX-CONNECT:$SOCK)
  echo "$reply"
  echo "$reply" | grep -q '"loading":\[\]' && break
  sleep 0.5
done
```

### One-shot: launch, dump, exit

```bash
EMCORE_DEBUG_CONTROL=1 cargo run --bin eaglemode &
APP_PID=$!
SOCK=/tmp/eaglemode-rs.${APP_PID}.sock
while [ ! -S "$SOCK" ]; do sleep 0.1; done
sleep 1   # let the home view materialize
printf '{"cmd":"dump"}\n{"cmd":"quit"}\n' | socat -t2 - UNIX-CONNECT:$SOCK
wait $APP_PID
```

## Architecture notes

- The acceptor thread runs only when `EMCORE_DEBUG_CONTROL=1` at process start. Off by default; zero runtime cost.
- Each connection gets a dedicated worker thread that reads JSON, parses to `CtrlCmd`, posts a `CtrlMsg` via `EventLoopProxy`, blocks on a `sync_channel(1)` for the reply.
- The main thread dispatches commands in `App::user_event` (winit `ApplicationHandler<CtrlMsg>`). All app-state mutation stays single-threaded.
- `wait_idle` is the only async-shaped command — its reply is parked in `PENDING_WAIT_IDLE` (a `Mutex<Vec<...>>`) and drained from `App::about_to_wait` once `EngineScheduler::is_idle()` returns true (or the deadline elapses).
- Synthetic input is dispatched by **direct call into `App::window_event`** — the same handler winit invokes for real input. Downstream handling is identical (scheduler ticks, focus, paint requests).
- Socket is unlinked on clean exit (`cleanup_on_exit`). On `SIGKILL` the file lingers; the next launch removes it before binding (PID-namespaced path makes collision with another live process impossible).

## Source map

| File                                              | Role                                                                |
|---------------------------------------------------|---------------------------------------------------------------------|
| `crates/emcore/src/emTreeDump.rs`                 | Walker — produces emTreeDumpRec output.                              |
| `crates/emcore/src/emCtrlSocket.rs`               | Wire types, acceptor, worker, dispatch, command handlers.            |
| `crates/emcore/src/emGUIFramework.rs`             | winit `ApplicationHandler<CtrlMsg>` impl, gate, proxy storage.       |
| `crates/emcore/src/emPanel.rs`                    | `PanelBehavior::dump_state` trait method.                            |
| `crates/emcore/src/emPanelTree.rs`                | `paint_count` / `last_paint_frame` on `PanelData`.                   |
| `crates/emcore/src/emView.rs`                     | `current_frame` counter, paint-driver counter bump, `dump_tree` shim.|
| `crates/emcore/src/emScheduler.rs`                | `EngineScheduler::is_idle` predicate.                                |
| `crates/emfileman/src/emDirPanel.rs`              | `dump_state` override (loading state — F010 surface).                |
| `crates/emcore/src/emFilePanel.rs`                | `dump_state` override (file model state).                            |

Spec: `docs/superpowers/specs/2026-04-24-treedump-port-design.md`. Plan: `docs/superpowers/plans/2026-04-24-treedump-port.md`.

---

## Known gaps and deferrals

1. **Synthetic keyboard input is non-functional.** winit 0.30 makes `KeyEvent::platform_specific` `pub(crate)` with no public constructor, so the `Input { kind: "key" }` path returns an error. Mouse / cursor / scroll events work fully. Annotated `DIVERGED: (dependency-forced)` in `emCtrlSocket.rs::build_window_event`. Workarounds for future tasks: (a) bypass `App::window_event` and call `emView::input` / cheat-code dispatch directly with synthesized `emInputEvent`; (b) wait for upstream winit to expose a synthesis API. F010-style debugging does not require keyboard input — mouse zoom + dump is the canonical workflow.

2. **Xvfb headless verification deferred.** The dev machine where Phase 1–5 was implemented does not have Xvfb installed. Integration tests that spawn the binary are marked `#[ignore]` so they can be opted into when Xvfb (or a real display) is available. If Xvfb turns out to fail under wgpu+llvmpipe, fall back to the `--headless` flag path described in the spec.

3. **`seek_to` does not yet use the seek engine.** Currently resolves `(view, identity)` and calls `VisitPanel` on the resolved view — same destination but without the seek-engine's lazy-load behavior. Loading-deep-into-an-uninitialized-subtree may behave differently from `td!` + manual zoom. Tracked as a follow-up; integrate `emView::SeekByIdentity` (or its native equivalent) once the seek engine grows the cross-sub-view dispatch path.

4. **Sibling windows and popups are not enumerated by the dump.** Phase 3 of the tree-dump-subview-crossing work landed `dump_from_root_context_with_home`, so the cascade now reaches sub-views and child contexts — `dump_context_with_cascade` walks `ctx.live_children()` and emits each sub-view's View + panel-tree recs under its parent context. What is still missing: sibling windows, popups, common models on non-home contexts, and private models. The Rust port's `emContext` does not yet enumerate these in a uniform way (C++ uses `dynamic_cast` over `emRecordable`; Rust models are `Rc<RefCell<T>>` keyed by `TypeId`). They will be wired once the port grows the matching enumeration APIs.

5. **`emView` reports placeholder counts for context fields.** The `Common Models: N / Private Models: N` lines in the view branch use `view.GetContext().common_model_count()` and a `0` placeholder for private. Annotated `UPSTREAM-GAP:` — Rust `emContext` does not track private models the same way C++ does; once it does, fill in the real count.

6. **`Engine Priority` is omitted from the emPanel branch.** C++ inherits `emPanel : emEngine` and emits engine priority for every panel. Rust uses separate `PanelBehavior` and engine traits, so panels are not engines and have no priority to report. Adding it back would require either inheritance-equivalent plumbing or an honest "panels aren't engines in this port" annotation; we chose the latter (omission with a doc comment).

7. **`%G` formatter and `printf` may differ in edge cases.** `fmt_g` implements a Rust port of C's `%.9G` semantics (9 significant digits, scientific when exponent < −4 or ≥ 9, strip trailing zeros, 2-digit minimum exponent). Spot-checked against glibc's `printf("%.9G", x)`; round-half-to-even at the 9th significant digit follows C. Unusual inputs (NaN, ±Inf, denormals) emit Rust-style `"nan"`/`"inf"` rather than C's `"NAN"`/`"INF"` — defensive only, the dump's numeric inputs never reach those values.

8. **Pre-commit hook is not active in the development environment.** `core.hooksPath` points at a path that does not exist on this machine (`/home/a0/git/eaglemode-rs/.git/hooks`). Gates (`cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr`, `cargo xtask annotations`) were run manually for every commit in the implementation chain. Anyone who fixes the hook configuration will get the gates auto-run on commit.

9. **Panel names containing `/` break the path resolver.** No escape syntax. If a future panel naming convention introduces such names, add escaping or replace `/` with a different separator. Today's emCore panel names are typed identifiers without slashes.

10. **`pub` visibility on several `emTreeDump` items is broader than CLAUDE.md prefers.** `dump_window`, `dump_model`, `dump_file_model`, `FileStateLabel`, several `VisualStyle` constructors, and the `Frame` enum are `pub` rather than `pub(crate)` because their only callers today are `#[cfg(test)]`, and `cargo clippy --lib -- -D warnings` flags `pub(crate)` items used only from tests as dead code. Downgrade to `pub(crate)` once the unwrappers materialize (Tasks above note when each becomes reachable).
