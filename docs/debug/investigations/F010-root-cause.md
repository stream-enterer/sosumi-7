# F010 — Directory listing loads slowly and renders blank — Root Cause

**Date:** 2026-04-24
**Issue:** Programmatic `VisitPanel`/`VisitFullsized` on the inner content sub-view never advances the view animator, so cosmos never zooms into view from `StartupEngine`'s `VisitFullsized(":")` and the agent-driven `visit` command is a silent no-op. The user-visible "blank after long Loading NN%" symptom is a downstream consequence: cosmos materialises only when the user's mouse-wheel zoom takes the synchronous `RawVisit` path, and even then any animator-driven zoom adjustments (focus follow, fullsized snap) are dropped.

## Root Cause Chain

1. **Symptom (E3.1, runtime capture):** Inner content sub-view `Current XYWH` equals `Home XYWH = 0,0,1285,700.556505` throughout the canonical capture sequence — before any visit, after `visit identity="root:content view"`, and after `visit view="root:content view" identity=":"`. `wait_idle` reports `ok` after each command. Cosmos panel: `Viewed: no, PaintCount: 0, LastPaintFrame: 0`.

2. **Direct cause (code, `crates/emcore/src/emView.rs:281-353`):** `VisitingVAEngineClass::Cycle` only runs when the wrapper engine is in a scheduler wake queue. Once running, it observes `va.is_active()` and forwards to `va.animate(...)`. The wrapper engine self-rewakes by returning the result of `animate` (per `emScheduler.rs:666-674`).

3. **Missing wake (code, `crates/emcore/src/emViewAnimator.rs:896-898`):** `emVisitingViewAnimator::Activate` is two lines and only sets `self.active = true`. It does NOT wake the wrapper engine. Because `EngineScheduler::register_engine` (`emScheduler.rs:313-327`) sets `awake_state: -1` (sleeping) and `RegisterEngines` does not call `wake_up` on `visiting_va_engine_id`, the wrapper engine is asleep at registration and stays asleep until something wakes it. Searching production code (`grep visiting_va_engine_id` minus tests/teardown) finds no path that wakes it after `Activate`.

4. **C++ reference (`~/Projects/eaglemode-0.96.4/src/emCore/emViewAnimator.cpp:68-84` and `:1040-1044`):** In C++, `emViewAnimator` derives from `emEngine`, so the animator IS the engine. `emViewAnimator::Activate` calls `WakeUp()` on line 81 to enqueue itself on the scheduler. `emVisitingViewAnimator::Activate` chains directly to it. Activation in C++ guarantees the next-slice cycle.

5. **Why outer-view zoom still works:** The user's mouse-wheel zoom on the outer view goes through `emView::Zoom` (`emView.rs:1296-…`), which calls `RawVisit` directly, synchronously, without using `VisitingVA`. So mouse-wheel zoom on the outer view is unaffected. Any *programmatic* `Visit*` call (`StartupEngine`'s `VisitFullsized(":")` on the inner view, control-channel `visit`/`visit-fullsized`/`seek-to`, focus follow-zoom) goes through the animator and is silently dropped.

6. **Why the existing regression test passes:** The test `visiting_va_cycles_when_activated` (`emView.rs:7398-7474`) at line 7454 calls `sched.borrow_mut().wake_up(visiting_id);` *manually* before `DoTimeSlice`. The manual wake_up was a workaround for the missing wake in `Activate`, not validation that `Activate` itself wakes — the test only confirmed registration + manual wake produces a Cycle.

## Fix Direction

Plumb `&mut SchedCtx<'_>` through the Visit-family methods on `emView` (`VisitByIdentity`, `VisitFullsized`, `VisitFullsizedByIdentity`, `VisitPanel`, `VisitByIdentityBare`) and through the public navigation helpers that wrap them (`VisitNext`, `VisitPrev`, `VisitFirst`, `VisitLast`, `VisitIn`, `VisitOut`, `VisitLeft`, `VisitRight`, `VisitUp`, `VisitDown`, `VisitNeighbour`). After `va.Activate()`, call a new helper `emView::wake_visiting_va_engine(&self, &mut SchedCtx)` that does `if let Some(id) = self.visiting_va_engine_id { ctx.wake_up(id); }` — mirror of `WakeUpUpdateEngine`.

Update production callers to pass `ctx`:
- `emCtrlSocket::handle_visit`, `handle_visit_fullsized`, `handle_seek_to` (close gap #3 from the F010 handoff: route `seek_to` through `VisitByIdentityBare` once the seek engine lands).
- `emViewInputFilter` mouse-zoom and focus-follow paths.
- `emWindow` keyboard navigation block (Tab/Arrow/Home/End/PageUp/PageDown).
- `emSubViewPanel::VisitByIdentity` delegation.
- Internal navigation helpers within `emView`.

Update the existing test `visiting_va_cycles_when_activated` to remove the manual `wake_up(visiting_id)` workaround once `Activate` (via the Visit* surface) handles the wake.

Rejected alternatives:
- **Always-poll the wrapper engine** (return `true` from `Cycle` even when `!is_active`): breaks `EngineScheduler::is_idle()` (line 734-737) which means `emCtrlSocket::wait_idle` in tests and the agent control channel would never return idle.
- **Wake from `UpdateEngineClass::Cycle`**: `UpdateEngine` is itself only woken on demand (notices/signals). After `Activate`, `UpdateEngine` may not run for a long time.
- **Store `engine_id` on the animator and wake from inside `Activate`**: still requires scheduler access, so callers still need to pass `SchedCtx`.

## Verification

The implementation pass should:

1. Add a unit test mirroring `visiting_va_cycles_when_activated` but **without** the manual `wake_up` line; assert that after `view.VisitByIdentityBare(..., &mut sc)`, a single `DoTimeSlice` cycles the animator (or at least removes it from the active state via `animate`'s convergence).

2. Pass `cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr`.

3. Manual verification (via `repro` field on F010): launch the app, run the canonical capture sequence:
   - Capture baseline tree dump.
   - `visit view="root:content view" identity=":"` (cosmos).
   - `wait_idle`.
   - Capture new tree dump.

   Expect: inner content view's `Current XYWH` ≠ `Home XYWH`, cosmos panel `Viewed: yes` and `PaintCount > 0`. Then zooming further should walk into a directory listing without the long "Loading NN%" / blank end-state — because animator-driven zoom adjustments (focus follow, fullsized snap) will now actually run.

---

## Phase 5 addendum (2026-04-25) — Second root cause

The Phase 4 wake fix (Visit* methods now call `wake_visiting_va_engine`)
landed and was necessary, but did NOT resolve F010's user-visible
symptom. A second, distinct cause was found and fixed.

### Second root-cause chain

7. **Symptom (E5.1, runtime):** Post-Phase-4 e2e
   `f010_subview_dump_nests_under_home_view_context` still failed —
   `dump.contains("emDirPanel")` assertion. Cosmos still `Viewed: no,
   PaintCount: 0`. Inner content view never zoomed to cosmos.

8. **Direct cause (code, `crates/emmain/src/emMainWindow.rs:611-628`):**
   `StartupEngine` state 7 — the framework-startup zoom-to-cosmos —
   constructs an `emVisitingViewAnimator`, calls `SetAnimated(false)`
   and `SetGoalFullsized(":", false, false, "")`, then stuffs the
   animator into `svp.active_animator`. **It does NOT call
   `animator.Activate()`.** `SetGoalFullsized` only sets
   `state = Curve` via `activate_goal`; it does NOT set
   `self.active = true` (that is `Activate()`'s job —
   `emViewAnimator.rs:493` for the trait base, `:896` for the
   visiting variant).

9. **Effect:** When `SVP::Cycle` (`emSubViewPanel.rs:464-528`)
   eventually runs, `anim.animate(...)` early-returns at
   `if !self.active { return false; }` (`emViewAnimator.rs:2117`).
   The animator is dropped (not put back), `animator_active = false`,
   `SVP::Cycle` returns `keep_awake = false`, the engine sleeps.
   Cosmos never zooms. emVirtualCosmosPanel auto-expand never fires
   (it only triggers when cosmos is viewed). No "home"/"work" panels.
   No emDirPanel. Blank.

10. **Wake gap:** Even if `Activate()` flips `self.active = true`,
    `SVP::Cycle` itself must run for the animator to tick. The
    SVP's `PanelCycleEngine` adapter is registered with the outer
    scheduler, but installing `svp.active_animator = Some(...)` does
    NOT wake it. C++ collapses these two roles (animator IS engine,
    so `Activate → WakeUp` wakes the cycle); Rust splits them, so
    the wake must be issued explicitly.

11. **C++ reference (`emMainWindow.cpp:423-432`):**
    ```cpp
    case 7:
        VisitingVA=new emVisitingViewAnimator(MainWin.MainPanel->GetContentView());
        VisitingVA->SetAnimated(false);
        VisitingVA->SetGoalFullsized(":",false);
        VisitingVA->Activate();   // <-- omitted in Rust port
        ...
    ```
    State 9 (`-visit` CLI arg) had the same omission
    (`emMainWindow.cpp:439-454`).

### Fix

Two-part change in `crates/emmain/src/emMainWindow.rs`:

1. After `SetGoalFullsized(":")` (state 7) and after `SetGoalCoords(...)`
   (state 9), call `animator.Activate()` to mirror the C++ flow.

2. After installing the animator on `svp.active_animator`, call
   `tree.wake_panel_cycle_engine(svp_id, ctx.scheduler)` to schedule
   `SVP::Cycle` for the next slice. C++ does not need this because
   `Activate` wakes the animator's own engine; Rust splits them, so
   the host engine wake is explicit.

A small new public helper `PanelTree::wake_panel_cycle_engine`
(production counterpart to the test-only `panel_engine_id_pub`)
exposes the wake without leaking `EngineId` internals.

### Verification

- `cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr` —
  all green (2791/2791).
- Instrumented e2e capture (`/tmp/F010_dlog_after_wake.stderr`) shows
  the inner sub-view animator now activates and `animate` runs:
  `VisitingVA::animate id=: state=Curve nep.depth=1 ...` and
  `active panel changed to PanelId(2v1)` (cosmos). Pre-fix this
  trace was absent.
- Manual GUI verification still required (issue `repro = launch app,
  navigate into cosmos`). Status set to `needs-manual-verification`.

### What this does NOT address

- The e2e test `f010_subview_dump_nests_under_home_view_context`
  asserts that `dump` contains `emDirPanel` after sending
  `visit view="root:content view" identity="::home"`. That visit's
  `resolve_identity` fails today because `home` is not a child of
  cosmos at the time of the visit — emVirtualCosmosPanel only
  auto-expands its children once cosmos becomes Viewed, which the
  test does not wait for. Updating the test to wait for cosmos
  auto-expansion (or to send a follow-up visit after a delay) is
  out of scope for F010 itself. The blank-listing user symptom is
  addressed by the StartupEngine fix because the user's mouse-zoom
  flow (which works through `RawVisit`) reaches cosmos, which then
  auto-expands normally.

---

## Phase 6 addendum (2026-04-25) — Render-chain cluster X+Y+Z

After Phase 5 landed, manual verification rejected the result: cosmos
now reaches the dir-panel level, but the panel itself renders wrong.
The issue was reframed as a cluster of three independent dir-panel
render-chain divergences vs C++ Card-Blue (slow-loading symptom split
off as F017; loading-state black background split off as F018).

### Hypothesis X — emDirPanel::Paint state-gated Clear

10. **Symptom:** Loaded directory panel background was black instead
    of `DirContentColor` light grey.

11. **Root cause** (`crates/emfileman/src/emDirPanel.rs:477-491`):
    Rust port omitted the C++ switch on `GetVirFileState()`. C++
    (`emDirPanel.cpp:159-170`) calls
    `painter.Clear(Config->GetTheme().DirContentColor.Get())` only
    on `VFS_LOADED` / `VFS_NO_FILE_MODEL`; default arm delegates to
    `emFilePanel::Paint`. Rust had neither.

12. **Fix (commit 08127c43):** ported the C++ switch verbatim,
    using Rust `VirtualFileState::Loaded` / `NoFileModel`. Existing
    dir-panel painting body runs inside the `Loaded`/`NoFileModel`
    arm after the `Clear` call.

13. **Note:** the symptom of "black during loading" is NOT addressed
    by this fix — that is a separate `emFilePanel::Paint` divergence
    tracked as F018.

### Hypothesis Y — emDirEntryPanel paints OuterBorder/InnerBorder

14. **Symptom:** Directory entries rendered with flat chrome — no
    Card-Blue gradient outer border, no inner border around the
    content area.

15. **Root cause:** `emDirEntryPanel::Paint`
    (`crates/emfileman/src/emDirEntryPanel.rs`) lacked the
    `PaintBorderImage` calls that C++
    (`emDirEntryPanel.cpp:318-335`) emits for OuterBorderImg and
    the inner border (file vs dir variant). Theme data and
    `.tga` assets were already present (`emFileManThemeData` had
    all OuterBorder*/FileInnerBorder*/DirInnerBorder* fields;
    `res/emFileMan/themes/` contained `CardOuterBorder.tga`,
    `CardInnerBorder.tga`, etc., byte-identical to upstream).

16. **Fix (commit 594d0b51):** ported the C++ `PaintBorderImage`
    call sequence. Theme image loading uses the existing
    `ImageFileRec` lazy-load infrastructure
    (`GetOuterBorderImage`/`GetDirInnerBorderImage`); no new
    asset-loading code introduced.

### Hypothesis Z — emDirEntryPanel::PaintInfo six-field info pane

17. **Symptom:** Each entry's left info pane showed only Time;
    Type, Permissions, Owner, Group, Size labels and values were
    all missing.

18. **Root cause:** Rust port stubbed `PaintInfo` to a single
    `PaintTextBoxed` of `FormatTime(st_mtime)`. C++
    (`emDirEntryPanel.cpp:484-725`) implements three layout modes
    keyed on aspect ratio (tall `t > 0.9`, medium
    `0.04 < t ≤ 0.9`, wide `t ≤ 0.04`) and paints six labels
    plus six field values with carefully-tuned magic constants
    (`0.087`, `0.483`, `7.6666`, `1.4`, `1.03`, `0.025`, `0.75`).

19. **Fix (commit 4f2d520f):** ported the entire `PaintInfo` body
    into Rust `paint_info` (`crates/emfileman/src/emDirEntryPanel.rs`).
    All three layout modes preserved with C++ line citations.
    Magic constants kept exact. Six fields ported:
    - **Type** (cpp:578-634) — branches on
      `stat.st_mode & libc::S_IFMT` for regular/dir/FIFO/blk/chr/sock
      plus the symlink branch (label half + target/error half).
    - **Permissions** (cpp:650-668) — Unix branch only; three
      `PaintText` calls for owner/group/other rwx groups. Windows
      attribute branch (cpp:642-648) and Windows drive-type
      extension (cpp:611-626) intentionally omitted; marked
      `// DIVERGED: (upstream-gap-forced)` because both are gated
      on `#if defined(_WIN32)` upstream and ship as no-ops on the
      project's Linux build.
    - **Owner / Group** (cpp:670-684) — single `PaintTextBoxed`
      from `entry.GetOwner()` / `GetGroup()`.
    - **Size** (cpp:689-709) — `em_uint64_to_str` (new helper,
      port of `emStd1.cpp:200-214`) with thousands-separator
      digit-chunk loop (`j = (len-i) - (len-i-1)/3*3`) and
      magnitude suffix `kMGTPEZY` painted as a separate
      `PaintText` at `(by[4] + bh[4]*0.75, bh[4]/5)`.
    - **Time** (cpp:714-721) — `FormatTime(st_mtime, …)` with the
      C++ `bw[5]/bh[5] < 6.0` compact-mode flag.

20. **Documented design choice — alignment:** C++ `PaintInfo` takes
    an `alignment: emAlignment` argument and switches between
    top/bottom/center positioning (cpp:514-516, 549-551). The Rust
    port hardcodes the center branch because the only caller
    (`Paint`) passes `EM_ALIGN_CENTER`. Comments at the centering
    branches in tall and wide modes explicitly document this and
    the plumbing path forward if a non-center caller is added.

### Verification

- Static checks clean: `cargo check`, `cargo clippy -- -D warnings`,
  `cargo xtask annotations`.
- Test suite: `cargo-nextest ntr` — 2249 passed, 2 pre-existing
  `emcore::plugin_invocation` failures unrelated. emfileman test
  count grew 167 → 171 (4 new `paint_info` tests covering tall,
  medium, wide regimes plus a field-content assertion).
- Divergence report: 1 exact-match, 0 divergent (zero divergence
  on the existing golden corpus).
- Verification gap: no `emDirEntryPanel`-level golden test
  infrastructure exists. Building one requires panel-tree
  integration, theme/asset loading, and a corresponding C++-side
  `gen_golden` harness — out of scope for F010. The four new unit
  tests cover layout-mode op counts and field-content strings;
  pixel-exact equivalence to C++ awaits the broader test
  infrastructure.

### Status

- F010 status flipped to `needs-manual-verification`.
- Awaiting human GUI confirmation:
  - **X verified**: light-grey `DirContentColor` background on
    loaded panels.
  - **Y verified**: Card-Blue gradient outer border + inner border
    around content area on dir entries.
  - **Z verified**: each entry's left info pane shows Type,
    Permissions (rwxr-x-r-x style), Owner, Group, Size (with
    thousands separator + magnitude suffix), Time — labels and
    values both visible across all three layout modes.
- F017 (slow loading) and F018 (loading-state black bg) remain
  open separately.
