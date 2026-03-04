# emCore Feature Contract: Rust + wgpu Reimplementation

> **Purpose:** Guide an LLM-driven extraction and reimplementation of Eagle Mode's `emCore` library from C++ into idiomatic Rust, targeting a wgpu rendering backend. The end product is a standalone, reusable **zoomable UI framework library** (not the Eagle Mode file manager or sample applications).

> **Date:** 2026-03-04

---

## 1. What emCore Is

emCore is Eagle Mode's **core UI framework library**. It is both a library and an API:

- **As a library**, it provides ~80 source files implementing foundations (strings, containers, threading), a software rendering pipeline, a recursive zoomable panel system, a widget toolkit, layout managers, a model/signal/scheduling system, and windowing abstractions.
- **As an API**, it defines the public interfaces that applications build against: creating panels, painting content, handling input, managing shared state through models, and running the event loop.

emCore is **not** a complete application. The file manager (`emFileMan`), fractal viewer (`emFractal`), chess game (`SilChess`), and other modules in the Eagle Mode repository are consumers of emCore. Our reimplementation targets emCore itself, scoped down from these sample applications.

**Our deliverable:** A Rust crate (e.g., `em_core`) that provides equivalent functionality to the C++ emCore, with rendering via wgpu instead of CPU-based scanline rasterization.

---

## 2. Architectural Overview

emCore has five major subsystems that compose into a complete UI framework:

```
+------------------------------------------------------------------+
|                        Application Code                          |
|    (creates panels, handles input, paints content, uses models)  |
+------------------------------------------------------------------+
         |              |               |              |
+--------+--+  +--------+--+  +--------+--+  +--------+--+
| Widget    |  | Layout     |  | Model /   |  | Panel /   |
| Toolkit   |  | System     |  | Signal    |  | View /    |
| (emBorder,|  | (Linear,   |  | (emModel, |  | Window    |
|  emButton,|  |  Pack,     |  |  emSignal,|  | (emPanel, |
|  emText-  |  |  Raster)   |  |  emEngine)|  |  emView,  |
|  Field..)|  |            |  |           |  |  emWindow)|
+-----------+  +------------+  +-----------+  +-----------+
         |              |               |              |
+------------------------------------------------------------------+
|                     Scheduler / Event Loop                       |
|         (emScheduler, time slices, cooperative multitasking)     |
+------------------------------------------------------------------+
         |
+------------------------------------------------------------------+
|                     Rendering Backend                             |
|   C++: emPainter (CPU scanline rasterizer, AVX2 SIMD)           |
|   Rust: wgpu (GPU-accelerated, shader-based)                     |
+------------------------------------------------------------------+
         |
+------------------------------------------------------------------+
|                     Platform Abstraction                          |
|   C++: emX11/emWnds (X11, Windows)                              |
|   Rust: winit + wgpu (cross-platform)                            |
+------------------------------------------------------------------+
```

---

## 3. Subsystem Contracts

Each subsystem below specifies **what must be reimplemented**, the behavioral contract, and Rust-idiomatic design notes.

---

### 3.1 Foundation Types

**Scope:** Core types, containers, error handling, utilities.

These provide the building blocks all other subsystems depend on. Most map directly to Rust standard library types or well-known crates, but the *behavioral contracts* matter for API compatibility.

#### 3.1.1 Numeric Types & Utilities

| C++ Type/Function | Rust Equivalent | Notes |
|---|---|---|
| `emInt8..emInt64`, `emUInt8..emUInt64`, `emByte` | `i8..i64`, `u8..u64`, `u8` | Direct mapping |
| `emException` | `Result<T, E>` with custom error types | Replace throw/catch with `?` propagation |
| `emLog`, `emWarning`, `emFatalError` | `log` crate (`info!`, `warn!`, `error!`) + `panic!` | Use `tracing` or `log` facade |
| `emGetClockMS`, `emSleepMS` | `std::time::Instant`, `std::thread::sleep` | Direct mapping |
| `emCalcAdler32`, `emCalcCRC32`, `emCalcCRC64`, `emCalcHashCode` | `crc32fast`, `std::hash::Hash` | Use crates for checksums |
| `emGetIntRandom`, `emGetDblRandom` | `rand` crate | Use `rand::Rng` trait |
| `emEncodeUtf8Char`, `emDecodeUtf8Char` | Native `char`, `String` | Rust strings are UTF-8 natively |
| `emTryLoadFile`, `emTrySaveFile`, `emTryLoadDir` | `std::fs` functions returning `Result` | Direct mapping |
| `emTryOpenLib`, `emTryResolveSymbolFromLib` | `libloading` crate | For plugin system if retained |

#### 3.1.2 String (`emString`)

**C++ behavior:** Copy-on-write reference-counted string with printf-style formatting, null-terminated, thread-unsafe sharing.

**Rust mapping:** Use `String` (owned) and `&str` (borrowed). No COW needed -- Rust's ownership model handles this idiomatically.

**Contract:**
- Must support efficient creation, concatenation, substring extraction
- Must support `format!()` equivalent to `emString::Format()`
- Must be UTF-8 (Rust default; C++ emString was locale-dependent)
- Interop with C strings (`CString`/`CStr`) where platform APIs require it

#### 3.1.3 Containers

| C++ Type | Rust Equivalent | Behavioral Contract |
|---|---|---|
| `emArray<T>` (COW dynamic array) | `Vec<T>` | Growable, sortable, binary-searchable |
| `emAvlTreeMap<K,V>` (sorted COW map) | `BTreeMap<K,V>` | Ordered by key, O(log n) lookup |
| `emAvlTreeSet<T>` (sorted COW set) | `BTreeSet<T>` | Ordered, set algebra (union, intersect, difference) |
| `emList` (intrusive linked list) | `VecDeque<T>` or custom intrusive list | Internal scheduler use only |
| `emOwnPtrArray<T>` (owned pointer array) | `Vec<Box<T>>` | Ownership transfer on insert, drop on remove |

**Key Rust adaptation:** Eliminate all copy-on-write. Rust's ownership + borrowing replaces COW with zero-cost moves and explicit cloning.

#### 3.1.4 Smart Pointers & References

| C++ Type | Rust Equivalent | Contract |
|---|---|---|
| `emRef<T>` (intrusive refcount) | `Arc<T>` or `Rc<T>` | Shared ownership of models |
| `emOwnPtr<T>` (unique ownership) | `Box<T>` | Exclusive ownership |
| `emCrossPtr<T>` (weak auto-null) | `Weak<T>` (from `Rc`/`Arc`) | Must handle upgrade failure via `Option` |
| `emAnything` (type-erased value) | `Box<dyn Any>` with `downcast_ref` | Runtime type checking |

#### 3.1.5 Threading & Concurrency

| C++ Type | Rust Equivalent | Contract |
|---|---|---|
| `emThread` | `std::thread::JoinHandle` | Spawn, join, get hardware thread count |
| `emThreadMiniMutex` (spinlock) | `std::sync::Mutex` or `parking_lot::Mutex` | Lock/unlock |
| `emThreadMutex` (readers-writer) | `RwLock<T>` | Concurrent reads, exclusive writes |
| `emThreadRecursiveMutex` | `parking_lot::ReentrantMutex` | Recursive locking (avoid if possible) |
| `emThreadEvent` (semaphore) | `std::sync::mpsc` channels or `tokio::sync::Semaphore` | Thread signaling |
| RAII lock guards | `MutexGuard`, `RwLockReadGuard` | Automatic via Rust's `Drop` |

**Design note:** The emCore scheduler is fundamentally single-threaded (cooperative). Threading is used primarily for the render thread pool. Prefer `Send + Sync` trait bounds over manual locking where possible.

#### 3.1.6 Process & I/O

| C++ Type | Rust Equivalent | Contract |
|---|---|---|
| `emProcess` | `std::process::Command` + `Child` | Spawn with piped stdin/stdout/stderr, wait, signal |
| `emFileStream` | `BufReader<File>` / `BufWriter<File>` | Buffered I/O with byte-order conversion |
| `emTmpFile` | `tempfile` crate | Auto-cleanup on drop |

---

### 3.2 Scheduler & Cooperative Multitasking

This is the **heart of emCore's execution model**. Everything flows through the scheduler.

#### 3.2.1 Scheduler (`emScheduler`)

**Behavioral contract:**
- Runs a main loop divided into **time slices** (~10ms each)
- Each time slice has two phases:
  1. **Signal phase:** Process pending signals, wake connected engines
  2. **Engine phase:** Execute awake engines by priority (5 levels: VERY_LOW to VERY_HIGH)
- Engines within the same priority use **FIFO ordering with alternating time-slice fairness** (prevents starvation)
- Provides `IsTimeSliceAtEnd()` for engines to yield cooperatively
- Supports graceful termination via `InitiateTermination(returnCode)`

**Rust design:**
```
pub struct Scheduler {
    // Pending signals list
    // 10 engine wake queues (5 priorities x 2 time-slice parities)
    // Time slice counter, clock
}

impl Scheduler {
    pub fn run(&mut self) -> i32;          // Main loop, returns exit code
    pub fn is_time_slice_at_end(&self) -> bool;
    pub fn initiate_termination(&mut self, code: i32);
    pub fn do_time_slice(&mut self);       // Single iteration
}
```

**Critical invariant:** The scheduler is single-threaded. All engines run cooperatively on one thread. This eliminates the need for locks on shared state within the scheduler's domain.

#### 3.2.2 Engine (`emEngine`)

**Behavioral contract:**
- An engine is a unit of cooperative work tied to a scheduler
- Starts sleeping; wakes via `WakeUp()` or signal connection
- `Cycle()` called when awake; returns `true` to stay awake next slice, `false` to sleep
- Can connect to multiple signals via `AddWakeUpSignal()` (reference-counted connections)
- `IsSignaled(signal)` checks if a specific signal fired this cycle
- Priority affects execution order within a time slice

**Three execution patterns must be supported:**
1. **Polling:** `Cycle()` returns `true` to run every slice
2. **Event-driven:** Connected to signals, wakes only when signaled
3. **Long-running job:** Checks `IsTimeSliceAtEnd()` to yield mid-work

**Rust design:**
```
pub trait Engine {
    fn cycle(&mut self) -> bool;  // Return true to stay awake
}
```

Engine registration, wake-up queues, and signal connections managed by the Scheduler.

#### 3.2.3 Signal (`emSignal`)

**Behavioral contract:**
- Binary event (fired or not-fired per time slice)
- `Signal()` adds to scheduler's pending list
- Connected engines wake when signal is processed
- `IsPending()` returns true until processed
- `Abort()` cancels a pending signal
- One signal can wake multiple engines
- Signaling within a `Cycle()` wakes the target engine within the same time slice (instant chaining)

**Rust design:**
```
pub struct Signal { /* internal id, pending state, connection list */ }

impl Signal {
    pub fn fire(&self, scheduler: &mut Scheduler);
    pub fn is_pending(&self) -> bool;
    pub fn abort(&mut self);
}
```

#### 3.2.4 Timer (`emTimer`)

**Behavioral contract:**
- Arms a signal to fire after N milliseconds
- Supports one-shot and periodic modes
- Periodic mode maintains average rate (bounded by time-slice frequency)
- Multiple timers share a single `TimerCentral` engine per scheduler

---

### 3.3 Model / Context System

The model system provides **shared, named, reference-counted objects** accessible through a hierarchical context tree. This is emCore's dependency injection / service locator pattern.

#### 3.3.1 Context (`emContext`)

**Behavioral contract:**
- Contexts form a **tree** (root context at top, child contexts below)
- Each context contains an **AVL-tree registry** of models keyed by `(TypeId, name)`
- `Lookup(type, name)` finds a model in this context
- `LookupInherited(type, name)` searches this context, then parent, recursively to root
- Garbage collection: periodically removes unreferenced common models past their minimum lifetime

**Rust design:**
```
pub struct Context {
    parent: Option<Weak<Context>>,
    children: Vec<Arc<Context>>,
    models: BTreeMap<(TypeId, String), Arc<dyn Model>>,
    // GC state
}
```

#### 3.3.2 Model (`emModel`)

**Behavioral contract:**
- Created via `Acquire(context, name, common)` factory pattern
- **Common models** stay alive in context registry even after user references drop (cached, GC'd after timeout)
- **Private models** deleted immediately when last user reference drops
- Identity: `(concrete_type, context, name)` triple
- Models can optionally participate in scheduling (they extend Engine)
- `SetMinCommonLifetime(seconds)` controls GC delay

**Concrete model types to reimplement:**

| Model Type | Purpose | Contract |
|---|---|---|
| `emSigModel` | Named signal accessible by name | Public `Signal` field |
| `emVarModel<T>` | Named typed variable | Public `T` field, static Get/Set helpers |
| `emVarSigModel<T>` | Variable + change signal | Auto-fires signal on `Set()` |
| `emConfigModel` | File-backed configuration | Load/save to disk, auto-save timer, change signal |
| `emFileModel` | Async file loading | State machine: WAITING->LOADING->LOADED, priority-scheduled, progress reporting |
| `emCoreConfig` | Core framework settings | Mouse/keyboard zoom speeds, render threads, memory limits |

#### 3.3.3 Record System (`emRec`)

**Behavioral contract:**
- Hierarchical serializable data structures with change notification
- Leaf types: `bool`, `int`, `double`, `string`, `color`
- Container types: arrays (`emTArrayRec<T>`), structs (`emStructRec`)
- Text-based serialization format (key-value with nesting)
- Incremental I/O (load/save in steps for async operation)
- Change listeners propagate modifications up the tree

**Rust design:** Use `serde` for serialization with a custom text format matching the emRec format. Change notification via the Signal system.

```
pub trait Record: Serialize + Deserialize {
    fn set_to_default(&mut self);
    fn is_default(&self) -> bool;
    fn change_signal(&self) -> &Signal;
}
```

#### 3.3.4 Priority-Scheduled File Loading

**Behavioral contract:**
- Only one file model loads at a time (serialized via `emPriSchedAgent`)
- Clients specify memory limits and priorities
- File loading is incremental (`TryContinueLoading()` returns chunks)
- State machine: `FS_WAITING -> FS_LOADING -> FS_LOADED` (or `FS_LOAD_ERROR`, `FS_TOO_COSTLY`)
- Progress reporting via `GetFileProgress()` (0-100%)

---

### 3.4 Panel / View / Window System

This is the **defining feature** of emCore: the recursive, infinitely-zoomable panel hierarchy.

#### 3.4.1 Panel (`emPanel`)

**Behavioral contract -- the core abstraction:**

- Panels form a **tree**. Each panel has zero or more children.
- Every panel has its own **coordinate system**: width is always `1.0`, height is the panel's **tallness** (aspect ratio).
- A panel's position within its parent is set by the parent calling `Layout(x, y, w, h, canvasColor)`.
- Panels are the unit of **painting**, **input handling**, and **focus**.

**Lifecycle:**
1. Construction: becomes child of parent panel or root of a view
2. Layout: parent calls `Layout()` to position this panel
3. Child layout: `LayoutChildren()` called when layout or child list changes
4. Auto-expansion: when zoom level crosses threshold, `AutoExpand()` creates children dynamically
5. Painting: `Paint(painter, canvasColor)` called if visible
6. Auto-shrink: when zoom level drops, `AutoShrink()` destroys children
7. Destruction: destructor deletes all children

**Key properties:**
- `name: String` -- unique among siblings
- `identity: String` -- path from root (e.g., `"root:child1:leaf"`)
- `layout_rect: (x, y, w, h)` -- position in parent coordinates
- `canvas_color: Color` -- background color hint
- `focusable: bool` -- can receive focus
- `enable_switch: bool` -- enabled state (ANDed with ancestors)

**Viewing state (valid only when panel is visible in a view):**
- `is_viewed: bool` -- currently being painted
- `is_in_viewed_path: bool` -- self or descendant is viewed
- `viewed_rect: (x, y, w, h)` -- position in screen pixels
- `clip_rect: (x1, y1, x2, y2)` -- clipping bounds in view coordinates
- `view_condition: f64` -- size metric that increases as user zooms in

**Coordinate transforms:**
```
panel_to_view_x(x) = x * viewed_width + viewed_x
panel_to_view_y(y) = y * viewed_width / pixel_tallness + viewed_y
```

**Virtual methods (trait in Rust):**
```
pub trait PanelBehavior {
    fn paint(&self, painter: &mut Painter, canvas_color: Color);
    fn input(&mut self, event: &InputEvent, state: &InputState, mx: f64, my: f64);
    fn get_cursor(&self) -> Cursor;
    fn is_opaque(&self) -> bool;
    fn layout_children(&mut self);
    fn notice(&mut self, flags: NoticeFlags);
    fn auto_expand(&mut self);
    fn auto_shrink(&mut self);
    fn cycle(&mut self) -> bool;  // Engine integration
}
```

**Notice flags that must be supported:**
- `CHILD_LIST_CHANGED` -- children added/removed
- `LAYOUT_CHANGED` -- position/size changed
- `VIEWING_CHANGED` -- visibility or viewed rect changed
- `ENABLE_CHANGED` -- enabled state changed
- `ACTIVE_CHANGED` -- active state changed
- `FOCUS_CHANGED` -- focus state changed
- `VIEW_FOCUS_CHANGED` -- view gained/lost OS focus
- `UPDATE_PRIORITY_CHANGED` -- priority changed
- `MEMORY_LIMIT_CHANGED` -- memory limit changed

#### 3.4.2 View (`emView`)

**Behavioral contract:**

A view is a **viewport** into a panel tree. It manages navigation, animation, focus, and rendering.

**Core state:**
- Root panel (the tree being viewed)
- **Supreme viewed panel** -- the highest panel whose parent is NOT visible (determines what to render)
- Active panel -- the panel the user is "at" for navigation purposes
- Focused panel -- the panel receiving keyboard input
- Visit state -- which panel the camera is anchored to, with relative offset and zoom

**Navigation model (the infinite zoom):**
- `Visit(panel, rel_x, rel_y, rel_a, adherent)` -- smoothly animate camera to show `panel` at relative position and zoom level
- `rel_x, rel_y`: offset from panel center (in panel-widths/heights)
- `rel_a`: view area relative to panel area (1.0 = panel fills view)
- `VisitFullsized(panel)` -- zoom to fit panel exactly
- `VisitNext/Prev/In/Out/Left/Right/Up/Down()` -- directional navigation
- `Zoom(fix_x, fix_y, factor)` -- zoom around a point
- `Scroll(dx, dy)` -- pan the view

**View flags:**
- `POPUP_ZOOM` -- zoom creates popup window at cursor
- `ROOT_SAME_TALLNESS` -- root panel matches view aspect ratio
- `NO_ZOOM` -- disable zooming
- `NO_USER_NAVIGATION` -- disable all user navigation
- `NO_FOCUS_HIGHLIGHT` -- suppress focus visual
- `NO_ACTIVE_HIGHLIGHT` -- suppress active visual
- `EGO_MODE` -- first-person navigation mode

**View input filter chain:**
The view processes input through a chain of filters before delivering to panels:
1. `DefaultTouchVIF` -- convert multi-touch gestures to zoom/pan/mouse events
2. `CheatVIF` -- debug/cheat code handling
3. `KeyboardZoomScrollVIF` -- arrow keys, Page Up/Down for navigation
4. `MouseZoomScrollVIF` -- mouse wheel zoom, middle-button pan

Each filter can consume events or pass them through.

**Input delivery to panels:**
After VIF chain, events propagate **bottom-up** from the topmost panel at the cursor position up through ancestors. Any panel can consume (eat) the event.

**Rendering flow:**
1. Determine supreme viewed panel
2. Recursively paint from root down through viewed path
3. Each panel painted with appropriate coordinate transform and clip rect
4. Siblings painted in stacking order (first child = back, last child = front)

#### 3.4.3 View Animators

**Behavioral contract:**

Animators provide smooth, physically-modeled camera movement.

| Animator | Behavior |
|---|---|
| `KineticViewAnimator` | Velocity-based with friction (deceleration). Base for others. |
| `SpeedingViewAnimator` | Accelerates toward a target velocity (for keyboard navigation). |
| `SwipingViewAnimator` | Touch-drag with spring physics and momentum. |
| `MagneticViewAnimator` | Snaps view to "best" panel alignment automatically. |
| `VisitingViewAnimator` | Smooth animation for `Visit()` calls. Curved pathfinding through panel tree. Handles seeking non-existent panels. |

Animators have master/slave relationships and can overlay each other. Each produces velocity deltas that the view integrates per frame.

#### 3.4.4 Window (`emWindow`)

**Behavioral contract:**
- Extends View with OS window management
- Flags: `MODAL`, `UNDECORATED`, `POPUP`, `MAXIMIZED`, `FULLSCREEN`, `AUTO_DELETE`
- Position/size management (with border awareness)
- Window icon
- Close signal
- Transient window relationships (dialogs parented to owner)
- `WindowStateSaver` -- persists geometry to config file

**Rust mapping:** Use `winit` for window creation. The emWindow abstraction wraps a winit `Window` + a wgpu `Surface`.

#### 3.4.5 Screen (`emScreen`)

**Behavioral contract:**
- Desktop geometry: virtual desktop bounds, per-monitor rects
- DPI query
- Mouse pointer control (move, warp)
- Screensaver inhibition
- Window creation factory

**Rust mapping:** Use `winit` monitor enumeration and window building.

#### 3.4.6 Input System

**Behavioral contract:**

Input events carry:
- `key: InputKey` -- mouse buttons, touch, keyboard keys, modifiers
- `chars: String` -- UTF-8 text for keyboard events
- `repeat: u32` -- 1=single, 2=double-click
- `variant: u32` -- 0=left/main, 1=right/numpad

Input state tracks:
- Mouse position `(x, y)`
- Active touches with IDs and positions
- All key states as a bitfield
- Modifier state helpers: `shift()`, `ctrl()`, `alt()`, `meta()`

Hotkey type: combination of modifiers + key, parseable from strings like `"Ctrl+C"`.

Cursor types: `Normal`, `Invisible`, `Wait`, `Crosshair`, `Text`, `Hand`, `LeftRightArrow`, `UpDownArrow`, `LeftRightUpDownArrow`.

#### 3.4.7 Clipboard

**Behavioral contract:**
- `put_text(text, selection)` -- put text to clipboard or X11 selection
- `get_text(selection) -> String` -- retrieve text
- `clear(selection)` -- clear

**Rust mapping:** Use `arboard` or `clipboard` crate, or winit clipboard support.

---

### 3.5 Rendering System

**This is the subsystem with the largest architectural change: from CPU scanline rasterization to GPU-accelerated wgpu rendering.**

#### 3.5.1 Painter API (`emPainter`)

The Painter is the rendering interface that all panels use. **The public API must be preserved 1:1** even though the backend changes completely.

**Coordinate system:**
- User space: arbitrary floating-point coordinates set by origin + scale
- Transform: `x_pixels = x_user * scale_x + origin_x`
- Clipping: axis-aligned rectangle in pixel coordinates
- Nested painters inherit and intersect parent's clip rect

**Drawing primitives -- all must be supported:**

**Filled areas:**
- `paint_rect(x, y, w, h, texture, canvas_color)`
- `paint_polygon(points, texture, canvas_color)` -- convex/concave, with holes
- `paint_ellipse(x, y, w, h, texture, canvas_color)`
- `paint_ellipse_sector(x, y, w, h, start_angle, range_angle, texture, canvas_color)`
- `paint_bezier(points, texture, canvas_color)` -- closed cubic bezier path
- `paint_round_rect(x, y, w, h, rx, ry, texture, canvas_color)`

**Stroked lines:**
- `paint_line(x1, y1, x2, y2, thickness, stroke, start_end, end_end, canvas_color)`
- `paint_polyline(points, thickness, stroke, start_end, end_end, canvas_color)`
- `paint_bezier_line(points, thickness, stroke, start_end, end_end, canvas_color)`
- `paint_ellipse_arc(x, y, w, h, start, range, thickness, stroke, start_end, end_end, canvas_color)`

**Outlined shapes (closed stroked paths):**
- `paint_rect_outline`, `paint_polygon_outline`, `paint_bezier_outline`, `paint_ellipse_outline`, `paint_ellipse_sector_outline`, `paint_round_rect_outline`

**Images:**
- `paint_image(x, y, w, h, image, alpha, canvas_color, extension)` -- with optional source rect
- `paint_image_colored(x, y, w, h, image, color1, color2, canvas_color, extension)` -- color gradient mapping
- `paint_border_image(...)` -- 9-patch scaling for borders

**Text:**
- `paint_text(x, y, text, char_height, width_scale, color, canvas_color)`
- `paint_text_boxed(x, y, w, h, text, max_char_height, color, canvas_color, alignment, ...)` -- fitted text
- `get_text_size(text, char_height, ...) -> (width, height)`

#### 3.5.2 Texture System

Textures define how filled areas and strokes are colored:

| Type | Description |
|---|---|
| `Color` | Solid RGBA color |
| `Image` | Bitmap with interpolation |
| `ImageColored` | Bitmap with two-color gradient mapping |
| `LinearGradient` | Two-point linear gradient |
| `RadialGradient` | Elliptical radial gradient |

**Image extension modes:** `Tiled`, `Edge` (clamp), `Zero` (transparent outside bounds)

**Image quality levels:**
- Downscale: Nearest, 2x2 through 6x6 area sampling
- Upscale: Nearest, AreaSampling, Bilinear, Bicubic, Lanczos, Adaptive

#### 3.5.3 Stroke System

Line styling:
- Dash types: `Solid`, `Dashed`, `Dotted`, `DashDotted`
- Configurable dash/gap length factors
- Rounded or angular joins/caps

Line end decorations (16 types):
`Butt`, `Cap`, `Arrow`, `ContourArrow`, `LineArrow`, `Triangle`, `ContourTriangle`, `Square`, `ContourSquare`, `HalfSquare`, `Circle`, `ContourCircle`, `HalfCircle`, `Diamond`, `ContourDiamond`, `HalfDiamond`, `Stroke`

Each end has configurable inner color, width factor, and length factor.

#### 3.5.4 Canvas Color Blending

**Critical behavioral contract:**

emCore uses a non-standard blending formula for overlapping objects that share edges:

```
target_new = target_old + (source - canvas_color) * alpha
```

This prevents color bleeding at shared edges. Standard alpha blending:
```
target_new = target_old * (1 - alpha) + source * alpha
```

The canvas color formula is **essential for correct rendering** of the bordered panel system. In wgpu, this requires a **custom blend mode in the fragment shader**.

#### 3.5.5 Color System

`Color` is a 32-bit RGBA value:
- Bit layout: `R[31:24] G[23:16] B[15:8] A[7:0]`
- Alpha: 255 = opaque, 0 = transparent
- Named color constants, HSV conversion, blending operations

#### 3.5.6 Image Type

```
pub struct Image {
    width: u32,
    height: u32,
    channel_count: u8,  // 1 (grey), 2 (grey+alpha), 3 (RGB), 4 (RGBA)
    data: Vec<u8>,      // Row-major: (y*width + x) * channel_count + c
}
```

Must support: creation, resizing, pixel access, copy, fill, channel conversion.

#### 3.5.7 Font Cache & Text Rendering

**Behavioral contract:**
- LRU glyph cache with configurable memory limit
- Per-character metrics (dimensions, positioning)
- Lazy loading (glyphs loaded on demand)
- Text painted as colored images

**wgpu approach:** Use a glyph atlas with SDF (Signed Distance Field) or MSDF rendering for resolution-independent text. Consider `glyphon` or `cosmic-text` crates.

#### 3.5.8 Render Thread Pool

**C++ behavior:** Work-stealing pool that parallelizes scanline rendering across CPU cores.

**wgpu replacement:** GPU handles parallelism natively. The render thread pool is **not needed** for the GPU path. However, a single render thread for wgpu command buffer submission may be useful.

#### 3.5.9 wgpu Rendering Architecture

**Mapping from CPU pipeline to GPU pipeline:**

```
Panel::Paint() calls
    |
    v
Painter API (unchanged public interface)
    |
    v
Command Recording Layer (NEW)
    |-- Batches draw calls by texture/shader
    |-- Tessellates polygons, beziers, arcs to triangle meshes
    |-- Generates vertex + index buffers
    |
    v
wgpu Render Pass
    |
    +-- Vertex Shader: apply origin/scale transform (uniform matrix)
    +-- Rasterizer: hardware rasterization with MSAA
    +-- Fragment Shader:
    |     - Texture sampling (nearest, bilinear, etc.)
    |     - Canvas color blending: output = canvas + (sampled - canvas) * alpha
    |     - Gradient computation
    +-- Output to surface texture
```

**Key GPU pipeline components:**

1. **Geometry tessellation (CPU side):**
   - Polygons -> triangle fans/strips
   - Bezier curves -> adaptive line segments -> triangles
   - Rounded rects -> arc segments -> triangles
   - Line strokes -> quad strips with end caps
   - Use `lyon` crate for tessellation

2. **Shader programs:**
   - Solid color shader (most common)
   - Texture shader (image painting)
   - Gradient shader (linear + radial)
   - Canvas-color blend shader (custom blend mode)
   - SDF text shader

3. **Draw call batching:**
   - Sort by shader/texture to minimize state changes
   - Use instancing for repeated shapes (e.g., grid of panels)
   - Atlas textures for small images

4. **Anti-aliasing:**
   - MSAA (4x or 8x) via wgpu multisampling
   - Or shader-based AA for specific primitives

---

### 3.6 Widget Toolkit

All widgets inherit from `emBorder`, which inherits from `emPanel`. The widget toolkit provides ready-made UI components.

#### 3.6.1 Border (`emBorder`) -- Base Widget

**Behavioral contract:**
- Adds border chrome, labels (caption + description + icon), and content area to a panel
- 10 outer border types: `None`, `Filled`, `Margin`, `MarginFilled`, `Rect`, `RoundRect`, `Group`, `Instrument`, `InstrumentMoreRound`, `PopupRoot`
- 4 inner border types: `None`, `Group`, `InputField`, `OutputField`, `CustomRect`
- Auxiliary area support (expandable config panels)
- Look/theme system (`emLook`) with recursive application
- Content area computed from border type + label size
- Shared toolkit resources (pre-loaded border/button images)

**Key virtual methods:**
- `PaintContent(painter, x, y, w, h, canvas_color)` -- override for custom content
- `GetContentRect() -> (x, y, w, h)` -- query content area
- `HasHowTo() / GetHowTo()` -- tooltip system

#### 3.6.2 Widgets to Reimplement

**Essential (reimplement fully):**

| Widget | Purpose | Key Signals/State |
|---|---|---|
| `emButton` | Clickable button | `ClickSignal`, `PressStateSignal`, pressed state |
| `emCheckButton` | Toggle button | `CheckSignal`, checked state |
| `emCheckBox` | Small checkbox variant | Same as CheckButton, different visuals |
| `emRadioButton` | Mutual exclusion button | `Mechanism` for group coordination |
| `emRadioBox` | Small radio variant | Same as RadioButton, different visuals |
| `emLabel` | Non-focusable text display | Caption, description, icon |
| `emTextField` | Text input (single/multi-line) | `TextSignal`, `SelectionSignal`, undo/redo, clipboard, cursor, validation, password mode |
| `emScalarField` | Numeric input with scale | `ValueSignal`, min/max, scale marks, keyboard interval |
| `emColorField` | RGBA/HSV color editor | `ColorSignal`, expandable with slider children |
| `emSplitter` | Resizable two-panel divider | `PosSignal`, min/max position, orientation |
| `emListBox` | Selectable item list | `SelectionSignal`, `ItemTriggerSignal`, selection modes (read-only, single, multi, toggle), sorting |
| `emDialog` | Modal dialog window | `FinishSignal`, result code, OK/Cancel/custom buttons |

**Lower priority (reimplement if needed for game UI):**

| Widget | Purpose | Notes |
|---|---|---|
| `emFileSelectionBox` | File browser | Only if game needs file open/save |
| `emFileDialog` | File open/save dialog | Wraps FileSelectionBox in dialog |
| `emCoreConfigPanel` | Core settings editor | Only for debug/preferences |
| `emErrorPanel` | Error display | Simple text display |

#### 3.6.3 Look/Theme System (`emLook`)

**Behavioral contract:**
- Defines visual properties: background color, foreground color, button colors, etc.
- Applied recursively to widget trees
- Widgets query look for painting
- Must be extensible for custom themes

---

### 3.7 Layout System

Layout classes automatically position child panels. Each provides a different algorithm.

#### 3.7.1 Linear Layout (`emLinearLayout`)

**Behavioral contract:**
- Arranges children in a single row or column
- Orientation: fixed horizontal/vertical, or **adaptive** based on panel tallness vs threshold
- Per-child weight (proportion of space)
- Per-child min/max tallness constraints
- Configurable spacing: inner (between children) and outer (margins)
- Alignment within available space
- Minimum cell count (for empty padding)

#### 3.7.2 Raster Layout (`emRasterLayout`)

**Behavioral contract:**
- Grid layout with uniform cell sizing
- Row-by-row or column-by-column ordering
- Fixed or automatic column/row count
- Preferred/min/max child tallness
- Strict mode (fill container vs maximize panel size)
- Same spacing/alignment system as Linear

#### 3.7.3 Pack Layout (`emPackLayout`)

**Behavioral contract:**
- Recursive binary space partitioning
- Evaluates multiple split positions and orientations
- Minimizes deviation from preferred aspect ratios
- Per-child weight and preferred tallness
- Optimized for ~7 or fewer children
- Produces visually balanced, irregular layouts

#### 3.7.4 Group Variants

Each layout has a `Group` variant that adds:
- Group border (`OBT_GROUP`)
- Focusable by default
- Otherwise identical layout algorithm

---

## 4. Out of Scope

The following emCore-adjacent modules are **NOT** part of this reimplementation:

- **Platform backends**: `emX11`, `emWnds` (replaced by winit + wgpu)
- **Sample applications**: `emFileMan`, `emFractal`, `emMines`, `emNetwalk`, `emClock`, `SilChess`, etc.
- **File format codecs**: `emBmp`, `emGif`, `emJpeg`, `emPng`, `emTiff`, `emSvg`, etc. (use Rust `image` crate)
- **emFpPlugin**: Dynamic plugin loading for file panels (not needed for game UI)
- **emMiniIpc**: Inter-process communication (not needed unless multi-process architecture desired)
- **emInstallInfo**: Installation path resolution (replaced by Rust project structure)
- **emTiling**: Deprecated layout (replaced by Linear/Raster/Pack)

---

## 5. Dependency Map for Implementation Order

Implementation should proceed bottom-up through the dependency graph:

```
Phase 1: Foundation
    Types, Error handling, Logging
    String (just use std String)
    Containers (just use std collections)

Phase 2: Scheduler Core
    Signal
    Scheduler (event loop)
    Engine (cooperative tasks)
    Timer

Phase 3: Model System
    Context (registry)
    Model (base, ref-counted, GC)
    VarModel, SigModel, VarSigModel
    Record system (serialization)
    ConfigModel, FileModel

Phase 4: Rendering
    Color, Image
    Texture, Stroke, StrokeEnd types
    Painter API (trait definition)
    wgpu backend (shaders, tessellation, batching)
    Font cache / text rendering

Phase 5: Panel System
    Panel (core abstraction)
    View (viewport + navigation)
    View Animators (kinetic, visiting, magnetic, swiping)
    View Input Filters (mouse zoom, keyboard nav, touch)
    Input system (events, state, hotkeys)
    Cursor, Clipboard

Phase 6: Windowing
    Screen abstraction (via winit)
    Window (via winit + wgpu surface)
    WindowStateSaver

Phase 7: Widget Toolkit
    Border (base widget)
    Look/theme system
    Label, Button, CheckButton/Box, RadioButton/Box
    TextField, ScalarField, ColorField
    Splitter, ListBox
    Dialog

Phase 8: Layout System
    LinearLayout / LinearGroup
    RasterLayout / RasterGroup
    PackLayout / PackGroup
```

---

## 6. Rust Crate Structure (Suggested)

```
em_core/
  src/
    lib.rs
    foundation/       # Types, utilities, error handling
    scheduler/        # Scheduler, Engine, Signal, Timer
    model/            # Context, Model, VarModel, Record, FileModel
    render/           # Painter trait, Color, Image, Texture, Stroke
      wgpu_backend/   # wgpu implementation of Painter
      shaders/        # WGSL shader sources
    panel/            # Panel, View, ViewAnimator, InputFilter
    input/            # InputEvent, InputState, InputKey, Cursor, Clipboard
    window/           # Window, Screen, WindowStateSaver
    widgets/          # Border, Button, TextField, etc.
    layout/           # LinearLayout, RasterLayout, PackLayout
```

**Key Rust crate dependencies:**
- `wgpu` -- GPU rendering
- `winit` -- window creation and event loop
- `lyon` -- 2D tessellation (polygons, beziers, arcs)
- `glyphon` or `cosmic-text` -- text rendering
- `serde` -- serialization for Record system
- `log` + `tracing` -- logging
- `rand` -- random numbers
- `parking_lot` -- fast mutexes (if needed)
- `arboard` -- clipboard access

---

## 7. Key Behavioral Invariants

These invariants must hold across the entire reimplementation:

1. **Single-threaded scheduler:** All engine Cycle() calls happen on one thread. No locks needed for model/signal/panel state.

2. **Cooperative yielding:** Long operations must check `is_time_slice_at_end()` and yield. No blocking calls in engine code.

3. **Panel coordinate invariant:** A panel's width is always `1.0`. Height equals tallness. Children are positioned in parent coordinates via `Layout()`.

4. **Canvas color blending:** The formula `target += (source - canvas) * alpha` must be used wherever canvas color is specified. This is not standard alpha blending.

5. **Signal instant chaining:** A signal fired during `Cycle()` wakes the connected engine within the same time slice (not deferred to next slice).

6. **Model identity:** Two `Acquire()` calls with the same `(TypeId, context, name)` must return the same model instance.

7. **Focus follows zoom:** As the user zooms into a panel, the active panel updates to reflect the current navigation position.

8. **View condition monotonicity:** `get_view_condition()` increases monotonically as the user zooms into a panel. Auto-expansion triggers at threshold.

9. **Input bottom-up propagation:** Input events start at the deepest panel under the cursor and propagate upward. Any panel can consume the event.

10. **Child stacking order:** First child is drawn first (back), last child drawn last (front). Same order for input (front panel gets first chance).
