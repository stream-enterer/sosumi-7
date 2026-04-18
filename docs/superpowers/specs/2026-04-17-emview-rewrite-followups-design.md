# emView Rewrite — Followups Spec

**Date:** 2026-04-17
**Source note:** `docs/superpowers/notes/2026-04-17-emview-rewrite-followups.md`
**Source plan (closed):** `docs/superpowers/plans/2026-04-17-emview-viewing-subsystem-rewrite.md`
**Acceptance baseline:** commit `68c6c59` — 2403/2403 tests pass; 235/243 golden (8 pre-existing baseline failures unchanged); runtime smoke ALIVE ≥15s with no core dump.

---

## 1. Scope and guiding principle

A single unified spec covering all 11 follow-up items captured at the close of
the 9-phase emView viewing/geometry subsystem rewrite. Executed as one phased
plan against the acceptance baseline above.

**Guiding principle.** Every divergence from the C++ reference (`~/git/eaglemode-0.96.4/`)
that surfaced during the emView rewrite is closed. Where a Rust shape differs
from C++ for any reason other than a documented `DIVERGED:` rationale that
still holds, the Rust shape changes. No new `DIVERGED:` annotations are
introduced; existing `DIVERGED:` annotations touched by a phase are
re-evaluated and either justified explicitly or removed.

This spec inherits and reinforces three project-wide invariants from
`CLAUDE.md`:

- **Port fidelity.** Pixel arithmetic and golden-tested geometry preserve C++
  formulas exactly; idiomatic Rust elsewhere subject to File and Name
  Correspondence.
- **File and Name Correspondence (F&N).** `emFoo.h` ↔ `emFoo.rs`; primary
  type name matches file name; deviations marked `SPLIT:` or `DIVERGED:` at
  the point of divergence.
- **No backwards-compat shims.** Renamed code is replaced outright; no
  `_old`, no re-exports, no deprecated wrappers.

### 1.1 Pre-flight investigation result (binding)

A pre-spec investigation established that the follow-up note's "Backend gaps"
section describes **wiring TODOs against existing infrastructure**, not
genuinely missing primitives. Findings (binding for this spec):

- `winit` + `wgpu` plumbing is fully present in `crates/emcore/src/emWindow.rs`
  (the `ZuiWindow` type) and wired through `emGUIFramework.rs`. Window
  create/resize, surface, input dispatch, paint, cursor tracking, and
  decorations control all exist. Undecorated windows are already
  supported (`with_decorations(false)`, `emWindow.rs:85`).
- `crates/emcore/src/emScheduler.rs` (847 LOC) implements the full C++
  scheduler model (signal fire/pending, engine register/wake, time-slice with
  priority re-ascent, instant chaining, clock-based `IsSignaled`). Driven
  in production from `emGUIFramework::about_to_wait`.
- `UpdateEngineClass` and `EOIEngineClass` exist with correct fields and
  `Cycle()` methods but do not `impl emEngine` and are not registered with
  the scheduler — they are poked manually via `WakeUp()` and `tick_eoi()`.
- The `close_signal` and `geometry_signal` are already fired by the framework
  on OS events (`emGUIFramework.rs:228, 335`); only the corresponding drains
  / wake-ups inside `emView` are stubbed.
- `emScreen` already enumerates monitor geometry; only `emView.max_popup_rect`
  is unpopulated.

Estimated effort for the wiring portion: 2–3 focused days.

### 1.2 In scope (11 items)

1. `ZuiWindow` → `emWindow` rename with popup-stub merge.
2. `svp_update_count` → `SVPUpdCount` rename.
3. Remove `home_pixel_tallness`; route all readers through `HomePixelTallness`.
4. Remove `PanelTree::current_pixel_tallness`; readers consume
   `view.CurrentPixelTallness` directly.
5. `emViewPort` 7-method backend wiring (`PaintView`, `GetViewCursor`,
   `IsSoftKeyboardShown`, `ShowSoftKeyboard`, `GetInputClockMS`, `InputToView`,
   `InvalidateCursor`, `InvalidatePainting`).
6. Popup `emWindow` creates a real undecorated winit window (no separate
   stub).
7. Popup-close signal drain in `emView::Update`.
8. `SwapViewPorts` popup-branch close-signal wake-up + `GeometrySignal` fire.
9. `EOIEngineClass` / `UpdateEngineClass` `impl emEngine` + scheduler
   registration; remove manual `WakeUp` / `tick_eoi` paths.
10. `GetMaxPopupViewRect` populated from `emScreen` monitor data.
11. `invariant_equilibrium_at_target` factor=1.0 gap closed via visit-stack
    removal; `InvalidateHighlight` guard tightened to C++ shape.

### 1.3 Out of scope

- Anything not in the source follow-up note.
- Opportunistic renames elsewhere in the crate (only the four F&N
  corrections in items 1–4).
- New features, new platform backends, new tests beyond the phase-specific
  assertions and the closed gap test.
- Scheduler model changes beyond `impl emEngine` for the two existing
  engine classes.
- Soft-keyboard support (no-op matches C++ — see §2.5c).

---

## 2. Item-by-item resolution

### 2.1 Renames and duplicate removal (F&N drift)

Each item targets a specific F&N rule violation surfaced during the emView
rewrite review.

#### Item 1 — `ZuiWindow` → `emWindow` rename + popup-stub merge

**Current state.** `crates/emcore/src/emWindow.rs` contains two parallel
types: `ZuiWindow` (the heavyweight type at line 37, ~1300 LOC) and a minimal
popup `emWindow` stub (line 1422). C++ has one `emWindow` class.
F&N rule violation: file is `emWindow.rs`, primary type is `ZuiWindow`.

**Target state.** One `pub struct emWindow` in `emWindow.rs`. The popup stub
is deleted; popup creation becomes `emWindow::new_popup`, which constructs a
real undecorated winit window using the same machinery as the main window.

**Mechanism.**

1. Rename `ZuiWindow` → `emWindow` mechanically across all 36 files
   identified by `grep -l ZuiWindow`.
2. Delete the popup-stub struct (`emWindow.rs:1422`–end of struct).
3. Re-route the stub's two callers (`new_popup`, `SetViewPosSize`) to the
   renamed type. `new_popup` constructs a real `emWindow` via the existing
   `create()` path with `with_decorations(false)` and (where supported by
   the current `winit` version) `WindowLevel::Floating`.
4. The `current_view_port: Rc<RefCell<emViewPort>>` field from the old stub
   becomes a field on the unified `emWindow` (the main window already
   manages an `emViewPort` analog; this consolidates them).

**Files touched.** 36 (per `grep -l ZuiWindow` snapshot). Notable:
`crates/emmain/src/emMainWindow.rs`, `crates/eaglemode/tests/**`,
`crates/emcore/src/emGUIFramework.rs`, `crates/emcore/src/emScreen.rs`,
`examples/*.rs`.

**No `DIVERGED:` annotation needed** — final state matches C++ exactly.

#### Item 2 — `svp_update_count` → `SVPUpdCount` rename

**Current state.** Field at `crates/emcore/src/emView.rs:192`. Sibling
`SVPUpdSlice` (added Phase 1 of the prior rewrite) keeps the C++ name,
making the inconsistency conspicuous. No `DIVERGED:` comment justifies the
snake_case form.

**Target state.** Field renamed to `SVPUpdCount`. All readers updated.
No `DIVERGED:` annotation (none warranted; `SVPUpdSlice` sets the
precedent).

**Mechanism.** Mechanical rename of one field and its readers.

#### Item 3 — Remove `home_pixel_tallness` duplicate

**Current state.** Two fields hold the same value:
- `HomePixelTallness` (added Phase 1 of the prior rewrite, C++ name)
- `home_pixel_tallness` (Rust-invention, retained Phase 1 with a
  cross-reference comment for compatibility)

Both fields are read by different paths across three files (`emView.rs`,
`emViewAnimator.rs`, `emViewPort.rs`). Phase 6 of the prior rewrite
scheduled removal but deferred it because internal readers remained.

**Target state.** `home_pixel_tallness` field deleted. All readers route
through `HomePixelTallness`. The Phase-1 cross-reference comment is
deleted.

**Mechanism.** Audit each `home_pixel_tallness` reader (3 files). Replace
with `HomePixelTallness`. Delete the field.

**Verification.** `grep -c home_pixel_tallness crates/` = 0 after the
phase.

#### Item 4 — Remove `PanelTree::current_pixel_tallness`

**Current state.** Field at `crates/emcore/src/emPanelTree.rs:303`,
initialized to 1.0 in `PanelTree::new`, **no write path** (Phase 6 of the
prior rewrite removed the band-aid `tree.set_pixel_tallness(1.0)` call in
`Update` but did not add a write path for non-1.0 values). Read by
`RawVisitAbs` child-update logic (line 1168) and `emPanelCtx` (line 2474).

**Target state.** Field deleted. `RawVisitAbs` and `emPanelCtx` take
`current_pixel_tallness: f64` as a parameter from the call chain rooted at
`emView::Update`, where `view.CurrentPixelTallness` is read.

This matches C++: there is no `PanelTree` analog in C++, and
`emPanel::Layout` reads `View.CurrentPixelTallness` directly via the
`View&` it already holds.

**Mechanism.**

1. Delete the field and its initialization.
2. Add `current_pixel_tallness: f64` parameter to `RawVisitAbs` and to
   `emPanelCtx` construction.
3. At every call site for these two, source the value from
   `view.CurrentPixelTallness`.

### 2.2 Backend wiring (was "backend gaps")

#### Item 5 — `emViewPort` 7-method backend wiring

All seven methods at `crates/emcore/src/emViewPort.rs`, currently marked
`PHASE-5-TODO: backend …`. The pre-flight investigation confirmed that the
underlying capabilities exist in `emWindow` / `WgpuCompositor` / `TileCache`
/ `emScreen`; the methods are dispatch points that are not wired through
the `emViewPort` boundary.

**Architectural decision (binding):** `emViewPort` owns input dispatch,
matching C++. The current direct `emWindow → emView` input path is
replaced with `emWindow → emViewPort::InputToView → emView::Input`.

A `Weak<RefCell<emWindow>>` back-reference is added to `emViewPort` so
`PaintView` and the invalidation methods can dispatch into the owning
window. (C++ `emViewPort` holds a back-pointer for the same purpose.)

| # | Method | Target state |
|---|---|---|
| 5a | `PaintView` (line 138) | Dispatches to `emWindow::render()` (compositor + tile cache) via the `Weak<emWindow>` back-reference. |
| 5b | `GetViewCursor` (line 132) | Returns the cursor stored on `emViewPort` (`emWindow` mouse-tracking sets it on each input). Matches C++ shape. |
| 5c | `IsSoftKeyboardShown` (line 172) / `ShowSoftKeyboard` (line 180) | **No-op match to C++.** See §2.5c. |
| 5d | `GetInputClockMS` (line 187) | Reads scheduler clock (`emScheduler.clock`) and returns monotonic ms. C++ calls `emGetClockMS()` which is `gettimeofday`-based; the Rust scheduler's clock advances on every signal phase using `Instant`-based monotonic time, which is the closer-to-correct reading. |
| 5e | `InputToView` (line 197) | `emWindow::dispatch_input` invokes `emViewPort::InputToView`, which forwards to `emView::Input`. |
| 5f | `InvalidateCursor` (line 205) | Marks `emViewPort.cursor_dirty`; `emWindow` consumes the flag on next frame and updates the winit cursor. |
| 5g | `InvalidatePainting` (line 213) | Forwards rect to `emWindow` tile-cache invalidation. |

All `PHASE-5-TODO:` comments removed from `emViewPort.rs` after this phase.
A new input-routing test verifies `emWindow::dispatch_input` →
`emViewPort::InputToView` → `emView::Input` end-to-end.

##### 2.5c — Soft keyboard `UPSTREAM-GAP`

C++ ships `IsSoftKeyboardShown` / `ShowSoftKeyboard` as base-class no-ops:

```cpp
// emCore/emView.cpp:2667
bool emViewPort::IsSoftKeyboardShown() const { return false; }
void emViewPort::ShowSoftKeyboard(bool show) { }
```

Neither `emX11/` nor `emWnds/` overrides them. `emSubViewPanel`'s override
forwards back up to the parent view, terminating in the same no-op. Soft
keyboard support is **a known feature gap inherited from upstream Eagle Mode**,
present in neither C++ nor Rust.

**Action.** Implement Rust as no-op (`return false` / empty body).
Remove the misleading `PHASE-5-TODO: backend …` comments. Replace each
with:

```rust
// UPSTREAM-GAP: emCore ships this as a no-op; no platform backend
// (emX11, emWnds) overrides it. Soft-keyboard support is absent in
// upstream Eagle Mode.
```

**`UPSTREAM-GAP:` is a new comment marker introduced by this spec**,
distinct from:
- `PHASE-N-TODO:` — Rust catch-up work tracked in a plan,
- `DIVERGED:` — intentional Rust deviation from C++.

`UPSTREAM-GAP:` documents that a Rust no-op is faithful to a C++ no-op
that itself represents missing functionality, so future readers do not
re-flag it as a Rust-port omission.

#### Item 6 — Popup `emWindow` real winit window

**Current state.** Popup stub creates no OS window (`emWindow.rs:1447`,
`new_popup`).

**Target state.** `emWindow::new_popup` (on the unified `emWindow` type
from Item 1) creates a real undecorated winit window with
`WindowLevel::Floating` (or platform equivalent supported by the current
`winit` version), wires its own surface and compositor, and exposes
`close_signal` like the main window.

**Depends on Item 1** (one `emWindow` type) and **Item 5** (input/paint
chain so the popup actually paints and receives input).

#### Item 7 — Popup-close drain in `emView::Update`

**Current state.** `emView.rs:2122`:

```rust
// backend-gap: requires IsSignaled(PopupWindow->GetCloseSignal());
// call ZoomOut() when the popup window is closed.
```

**Target state.** Replace with the real check:

```rust
if scheduler.IsSignaled(popup.close_signal()) {
    self.ZoomOut();
}
```

Matches C++ `emView::Update`.

#### Item 8 — `SwapViewPorts` popup wiring

**Current state.** Two `// PHASE-5-TODO:` comments at `emView.rs:1616`
(close-signal wake-up) and `1675` (GeometrySignal fire).

**Target state.** Wire both:
- On entering the popup branch, `UpdateEngine.WakeUp()` is called when
  the popup's `close_signal` becomes pending.
- After the viewport swap, `GeometrySignal` is fired.

Both `PHASE-5-TODO:` comments removed.

**Depends on Item 7** (close-signal drain) and **Item 9** (engine
registration so wake-up actually schedules).

#### Item 9 — `impl emEngine` for `EOIEngineClass` and `UpdateEngineClass`

**Current state.** `emView.rs:206` (`EOIEngineClass`) and `emView.rs:245`
(`UpdateEngineClass`) both have the right fields and `Cycle()` methods
but neither implements `emEngine` nor is registered with the scheduler.
`UpdateEngineClass` is poked manually via `WakeUp()` calls scattered
through `emView.rs` (e.g. lines 1888, 3055). `EOIEngineClass.Cycle()` is
driven only by the test-harness `tick_eoi` (line 3017), called only from
test (line 5470).

**Target state.** Both classes `impl emEngine`. Both registered with the
scheduler at `emView::new` via `scheduler.register_engine(...)`. All
manual `UpdateEngine.WakeUp()` call sites in `emView.rs` deleted (the
scheduler drives them). The `tick_eoi` symbol is removed from the
codebase; tests drive EOI via `scheduler.DoTimeSlice()`.

#### Item 10 — `GetMaxPopupViewRect` from `emScreen` monitor data

**Current state.** `emView.rs:2955` falls back to the home rect because
`max_popup_rect` is never populated.

**Target state.** At `emView` construction, query the owning `emScreen`
for the monitor containing the home rect; populate `max_popup_rect`.
Fallback to home rect remains as the no-`emScreen`-available path
(exercised by a unit test).

### 2.3 Test gaps

#### Item 11a — Visit-stack removal; close `invariant_equilibrium_at_target` factor=1.0 gap

**Current state.** `emViewAnimator.rs:3320` skips the assertion at
factor=1.0 with a `KNOWN GAP (TODO phase 8)` comment. Root cause: at
factor=1.0, root-centering clamps `viewed_x=0` regardless of the
visit-stack `rel_x`. Rust's visit stack has no C++ analogue (C++ derives
rel coords from `ViewedX`/`ViewedY` on every read).

**Target state.** Delete the visit stack from `emPanelTree` / `emView`.
`rel_x` and `rel_y` are derived from `ViewedX` / `ViewedY` on every read
(matching C++). The `KNOWN GAP` skip is removed; the test asserts at
factor=1.0.

**This is the highest-risk item in the spec.** The visit-stack removal
touches animator math, all golden tests, and changes the read contract
for visit coordinates. Phase 11 is permitted to *change which* golden
tests fail but **not the total number of failures**.

#### Item 11b — `InvalidateHighlight` guard tightening

**Current state.** Phase 5 of the prior rewrite implemented
`InvalidateHighlight` using `self.active.is_some()` as a proxy for
"active panel is viewed." C++ guard checks `ActivePanel->Viewed` and
`VFlags`.

**Target state.** Replace `self.active.is_some()` with the C++ guard:
check `ActivePanel.Viewed` and `VFlags`. Borrow-flow refactor done
locally if needed (likely take a `&Panel` rather than re-borrowing
through `self`).

---

## 3. Phased execution

### 3.1 Phase ordering

Ordered to land mechanical / low-risk changes first, then semantic
changes that depend on them, then the highest-risk architectural item
last.

| Phase | Item(s) | Why this order |
|---|---|---|
| 1 | Item 2 (`SVPUpdCount` rename) | Pure mechanical. Establishes phase rhythm. |
| 2 | Item 3 (`home_pixel_tallness` removal) | Routes 3 reader files. No semantic change. |
| 3 | Item 4 (`PanelTree::current_pixel_tallness` removal) | Touches layout call chain. Lands before visit-stack removal. |
| 4 | Item 1 (`ZuiWindow` → `emWindow` + popup-stub merge) | Mechanical across 36 files but architecturally significant. Lands before backend wiring. |
| 5 | Item 5 (`emViewPort` 7-method wiring) | Depends on Phase 4. Establishes C++ input/paint chain. |
| 6 | Item 6 (real popup window) + Item 8 GeometrySignal half | Depends on Phases 4 + 5. |
| 7 | Item 9 (`impl emEngine` for EOI/Update engines) | Independent in principle; lands here so manual-wakeup audit is against final shape. |
| 8 | Item 7 (popup-close drain) + Item 8 close-signal half | Depends on Phases 6 + 7. |
| 9 | Item 10 (`GetMaxPopupViewRect`) | Independent; small. Keeps popup work contiguous. |
| 10 | Item 11b (`InvalidateHighlight` guard) | Small, isolated. Lands before visit-stack work. |
| 11 | Item 11a (visit-stack removal) | **Highest risk.** Lands last so prior phases' gates do not absorb golden churn. |

### 3.2 Per-phase acceptance gate

Every phase must pass **all** of:

1. `cargo check` clean.
2. `cargo clippy -- -D warnings` clean.
3. `cargo-nextest ntr` — full nextest pass, **no test count regression**.
4. `cargo test --test golden -- --test-threads=1` — golden status equals
   or improves the prior phase's baseline (current: 235/243; 8 baseline
   failures unchanged). Phase 11 is permitted to change *which* tests
   fail but not the *total* number.
5. Pre-commit hook clean (no `--no-verify`).
6. Runtime smoke: `cargo run --release --bin eaglemode` stays alive ≥15s
   with no panic / core dump (matches the emView-rewrite acceptance
   protocol).
7. Phase-specific assertion (§3.3) verified.

### 3.3 Phase-specific assertions

| Phase | Assertion |
|---|---|
| 1 | `grep -c svp_update_count crates/` = 0; `SVPUpdCount` reader count unchanged. |
| 2 | `grep -c home_pixel_tallness crates/` = 0; all prior readers reference `HomePixelTallness`. |
| 3 | `PanelTree` struct has no `current_pixel_tallness` field; `RawVisitAbs` + `emPanelCtx` signatures take it as a parameter. |
| 4 | `grep -c ZuiWindow .` = 0 outside `target/` and historical docs (`docs/superpowers/notes/`, `docs/superpowers/plans/`); `emWindow.rs` contains exactly one primary `pub struct emWindow`. |
| 5 | All 7 `PHASE-5-TODO:` comments removed from `emViewPort.rs`; soft-keyboard methods carry `UPSTREAM-GAP:` comments; `InputToView`-driven path verified by a new input-routing test (mouse click → `emWindow::dispatch_input` → `emViewPort::InputToView` → `emView::Input`). |
| 6 | Popup created in a runtime smoke run appears as a separate winit window (verified by logging window count, expecting ≥2 when popup open). |
| 7 | `tick_eoi` symbol removed from codebase; `UpdateEngineClass` and `EOIEngineClass` registered in `emView::new`; manual `UpdateEngine.WakeUp()` call sites in `emView.rs` deleted. |
| 8 | Opening then OS-closing a popup window during the smoke run results in `emView` zooming out (verified by a new behavioral test driving the scheduler explicitly). |
| 9 | `max_popup_rect` populated from `emScreen` for the primary monitor; fallback path exercised by a unit test that constructs an `emView` with no `emScreen` available. |
| 10 | `InvalidateHighlight` no longer references `self.active.is_some()`; checks `ActivePanel.Viewed` and `VFlags`. |
| 11 | `tests/golden` total pass count ≥ prior phase; `invariant_equilibrium_at_target` runs at factor=1.0 with no `KNOWN GAP` skip; visit-stack symbol removed from `emPanelTree` and `emView`. |

### 3.4 Hard rules (apply to every phase)

- No `--no-verify`. No `#[allow(...)]` / `#[expect(...)]` outside the
  F&N-mandated exceptions in `CLAUDE.md` (`non_snake_case` on the
  `emCore` module, `non_camel_case_types` on `em`-prefixed types,
  too-many-arguments).
- **No new `DIVERGED:` annotations.** Every existing `DIVERGED:`
  touched by a phase is re-evaluated; if the rationale no longer holds,
  the divergence is removed.
- No `PHASE-N-TODO:` comments left behind by the phase that owns them.
- No `_old` / backwards-compat shims. Renamed code is replaced
  outright (per `feedback_no_backcompat_renames.md`).
- Each phase is one commit (or a tight sequence of commits) on a single
  branch; phase boundaries are commit boundaries for `git bisect`.

### 3.5 Out-of-scope safeguards

- No opportunistic renames outside the 11 items.
- No new tests beyond those listed in §3.3 and the closed gap test
  (`invariant_equilibrium_at_target` at factor=1.0).
- No scheduler model changes beyond `impl emEngine` for the two
  existing engine classes.
- No new platform backends (soft keyboard remains a no-op per
  §2.5c `UPSTREAM-GAP`).

---

## 4. Acceptance state at spec close

The spec is complete when, on the merged branch:

- All 11 follow-up items resolved per §2.
- Per-phase gates §3.2 passed at every phase.
- Final phase-11 gate: visit stack removed, `invariant_equilibrium_at_target`
  asserts at factor=1.0, total nextest count ≥ baseline, golden total
  pass count ≥ 235 (failures may shift; total may not regress).
- `grep` produces zero hits for: `ZuiWindow`, `svp_update_count`,
  `home_pixel_tallness`, `tick_eoi`, `PHASE-5-TODO`, `backend-gap:`
  (in `crates/`).
- The source follow-up note (`docs/superpowers/notes/2026-04-17-emview-rewrite-followups.md`)
  is updated with a closing line pointing to this spec and its
  implementing plan.

---

## 5. References

- Source note: `docs/superpowers/notes/2026-04-17-emview-rewrite-followups.md`
- Closed plan: `docs/superpowers/plans/2026-04-17-emview-viewing-subsystem-rewrite.md`
- Closed spec: `docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md`
- C++ reference: `~/git/eaglemode-0.96.4/include/emCore/`,
  `~/git/eaglemode-0.96.4/src/emCore/`
- Project rules: `CLAUDE.md` (Code Rules, Port Fidelity, F&N).
