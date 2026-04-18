# emView Subsystem Rewrite — Closeout Report

**Date:** 2026-04-18 (living document; folds in three earlier notes).
**Scope:** Complete closeout for the emView viewing/geometry subsystem rewrite — the original 10-phase wave plus its two follow-up waves (W3 popup architecture, W4 visit-stack rewrite).
**Commits covered:** `fc57a6a`..`d3c0580` on `main`.
**Supersedes:**
- `2026-04-18-emview-followups-execution-debt.md` (umbrella inventory)
- `2026-04-18-emview-followups-roadmap.md` (residual sequencing)
- `2026-04-18-w3-popup-architecture-closeout.md` (W3 wave closeout)

This document is the source of truth for residual work. It catalogues the state of the tree; it is not a process retrospective.

---

## 1. Status at a glance

| Axis | Status |
|---|---|
| Phases delivered | **Original 10/10** + W3 + W4 |
| Commits on main | **~51** (14 original + W3 cluster + 14 W4 + SP1 bundle + SP3 bundle + merges) |
| Tests | 2432/2432 nextest, 9 skipped, 0 failed |
| Golden | 237 passed / 6 failed (baseline parity — same 6 pre-existing failures across all waves) |
| Smoke (`timeout 20 cargo run --release --bin eaglemode`) | exits 143 / 124 — program stays alive |
| Scaffolds still in tree | **0** (both `PopupPlaceholder` and the visit-stack scaffolding are gone) |
| Phase-follow-up markers | ~~1 `PHASE-6-FOLLOWUP`~~ **0** (closed by SP1) + ~~3 `PHASE-W4-FOLLOWUP`~~ **0** (closed by SP3) + 2 `UPSTREAM-GAP` (intentional) |
| Known Rust-port incompletenesses remaining | Per-view notice dispatch (SP5, blocked on multi-window roadmap), W3 surface de-dup (SP6, optional), `emContext` threading (SP7, ARCH). SP1 (animator-forward DIVERGED, re-entrancy docs, W4 polish), SP2 (`InvalidateHighlight` audit — landed in SP1 as W1b), SP3 (CoreConfig ownership), and SP4 (`emView::Update` engine-only routing + Phase-8 single-engine test) all closed 2026-04-18. |

The subsystem is structurally aligned with C++ emCore on every path the original plan targeted. Remaining debt is enumerated in §8.

---

## 2. Original wave — 10-phase emView viewing/geometry rewrite

Plan: `docs/superpowers/plans/2026-04-17-emview-rewrite-followups.md`. Commit range: roughly `fc57a6a`..`94fee79`, 14 commits. Closed at baseline 2409/2409 nextest; 237/6 golden.

### 2.1 Deferrals absorbed into later phases of the same wave

**Phase 5 — deferred back-reference wiring (Task 5.3).** `emGUIFramework` stored `HashMap<WindowId, emWindow>` by value. Converting to `Rc<RefCell<emWindow>>` would have cascaded through ~30 call sites, exceeding the plan's 50-line escalation threshold. Consequence at the Phase-5 commit (`f1f6de0`): `emViewPort::window: Weak<RefCell<emWindow>>` stayed `None` for framework-created windows, making `PaintView` and `InvalidatePainting` no-ops in production. **Resolution:** absorbed into Phase 6 (`c053689`) — `Rc<RefCell<>>` conversion landed cleanly across ~27 files / ~160 lines.

**Phase 8 — two-engine test split.** The plan wanted a behavioural test driving the scheduler end-to-end for `close_signal` zoom-out. Blocker: `PanelTree::get_mut` is `pub(crate)`, infeasible from the behavioural-test folder. Fallback landed as `test_phase8_popup_close_signal_zooms_out` inline in `emView.rs`, using a dormant `NoopEngine` to isolate signal-clock drain from `WakeUpUpdateEngine` side-effects. Reviewer flagged that the test asserts across two different engines rather than one integrated run; still **open** (see §8, item *Phase-8 test promotion*).

### 2.2 Plan violations corrected mid-wave

Three follow-up commits walked back hard-rule violations introduced by implementer subagents. All fixes landed the same day. Pattern observation (not an action item): three of six phases doing real work each introduced at least one forbidden `DIVERGED:` annotation on first pass — subagents reached for the marker when a signature or field looked "different" from C++, even when the plan forbade new ones.

| Fix | Phase | Reverted addition |
|---|---|---|
| `b5f9c42` | 2 | `DIVERGED:` on `SetViewGeometry` in `emViewPort.rs` |
| `282cc57` | 4 | `DIVERGED:` on `PopupPlaceholder` in `emView.rs` |
| `866d51b` | 5 | Three `DIVERGED:`/`RUST-DIVERGED:` notes around `emCursor::to_winit_cursor`, `emViewPort::cursor`, and cursor-resolution |

### 2.3 Scope expansions / sanctioned divergences

**Phase 3 touched 41 files** (plan named 3). Expansion was mechanical: a new `1.0` pixel-tallness arg had to reach every `PanelTree::Layout` / `emPanelCtx::new` call site. A one-off script (`/tmp/fix_layout.py`) did the test-side propagation. No functional risk.

**Phase 7 deviated from plan's `UpdateEngineClass` shape.** Plan proposed `UpdateEngineClass { view: Weak<RefCell<emView>>, tree: Weak<RefCell<PanelTree>> }`; implementation used `{ window_id: WindowId }` + lookup via `ctx.windows`. This avoids wrapping `emView` in `Rc<RefCell<>>`. The plan listed this as an acceptable alternative.

**Phase 8 scheduler API.** Added `is_signaled_for_engine(signal, engine)` (query-shaped) instead of the plan-anticipated `add_wake_up_signal(engine_id, signal)` (connection-shaped). Semantically equivalent and matches an existing `connect/disconnect` pattern.

---

## 3. Wave W3 — Popup architecture

Closed-out 2026-04-18. `PopupPlaceholder` stub deleted; real `emWindow` restored across the popup path with deferred OS-surface materialization wired end-to-end. Popup windows construct in a `Pending` state and materialize their winit/wgpu surface on the first `about_to_wait` tick via a back-channel drained in `App`.

### 3.1 Phase summary

| Phase | Task | Commit | Outcome |
|---|---|---|---|
| 1 | `OsSurface` enum refactor | `6506900` | OK (`DIVERGED:` on missing accessor methods) |
| 2 | `new_popup_pending` | `0b303f9` | OK |
| 3 | `materialize_popup_surface` | `7edec0b` | OK |
| 4 | Delete `PopupPlaceholder`, rewrite popup branch | `483d440` + `612f2b2` | OK (483 lands, 612 fixes lazy-wire guard + signal-fire safety doc) |
| 5 | Materialization integration test | `49c385d` | OK (DISPLAY-gated) |
| 5 | Cancellation test | `b8da326` | OK (DISPLAY-gated) |
| 6 | `popup_window.rs` cleanup | `31ff689` | OK (retarget to `new_popup_pending`) |

### 3.2 Architectural changes

- `OsSurface { Pending(Box<PendingSurface>), Materialized(Box<MaterializedSurface>) }` — boxed to satisfy `clippy::large_enum_variant`; tuple-pattern access only.
- `App::pending_actions: Rc<RefCell<Vec<DeferredAction>>>` (was plain `Vec`). Drains by-move so `Rc::strong_count == 1` cancellation in `materialize_popup_surface` works.
- `emView::pending_framework_actions: Option<Rc<RefCell<...>>>` back-channel; lazily wired (`is_none`-guarded) at the top of `App::about_to_wait`.
- `emWindow::SetViewPosSize` is Pending-tolerant (stashes pos/size into `PendingSurface.requested_pos_size`, applied on materialization). Signature shifted `&self → &mut self`.

### 3.3 Scope expansions

- **Lazy-wire location.** Plan said "wire at every window-creation site"; landed as a single touch-point at the top of `about_to_wait`, idempotent. Better than the plan spec.
- **`SetViewPosSize` Pending-tolerance** was needed at runtime and not anticipated in the plan.

### 3.4 `PHASE-6-FOLLOWUP:` markers

- Cleared by W3: 4 (struct doc, `new_popup`, `SetViewPosSize`, `RawVisitAbs` call site).
- Still open at W3 close: 1 (VIF-chain migration at `emView.rs:~3626`, belonged to the unported `emView::Input` animator-forward work). **Cleared by SP1 on 2026-04-18** (`eb2f0fe`) — reclassified as structural divergence and promoted to a `DIVERGED:` block at `emView.rs:~3749`.

### 3.5 Items deferred from W3 to follow-up

1. `materialize_popup_surface` duplicates ~50 lines of `emWindow::create()`'s wgpu surface init. A `build_materialized_surface(gpu, winit_window) -> MaterializedSurface` helper would deduplicate. Plan flagged this as optional.
2. Plan-listed `materialized()`/`materialized_mut()` accessor methods on `emWindow` were inlined as `match` at call sites (`DIVERGED:` comment present); idiomatic for Rust borrow rules.

### 3.6 Risk-register outcomes

| Risk | Outcome |
|---|---|
| R1 — missed field-access site | None; `cargo check` + nextest caught nothing |
| R2 — back-channel plumbing invasive | Mitigated via lazy-wire approach |
| R3 — test scheduler wiring | `SignalId::default()` fallback for scheduler-less unit tests |
| R4 — windows-map iteration mid-insert | Drain runs at `~:455` before window iteration |
| R5 — winit window destruction timing | Cleanup closure on popup-exit; signal fire is defensive (lookup-or-noop) |
| R6 — first-frame popup paint timing | Accepted ~16.7 ms concession |

---

## 4. Wave W4 — Visit-stack rewrite

Plan: `docs/superpowers/plans/2026-04-18-emview-visit-stack-rewrite.md`. 14 commits `2201d04`..`a4251fa` on branch `w4-emview-visit-stack-rewrite`, merged to main as `d3c0580`. Closed the original wave's deferred Phase 11.

### 4.1 What the wave did

Deleted all Rust-only visit-state scaffolding (`VisitState`, `visit_stack`, `pending_animated_visit`, `current_visit`, `visit_stack_mut`, `animated_visit`, `animated_visit_panel`, `go_back`, `go_home`, `take_pending_animated_visit`, `has_pending_animated_visit`) and routed every `Visit`-family method through `emVisitingViewAnimator` per C++ `emView.h:675`.

### 4.2 Phase/commit summary

| Phase | Commit | Outcome |
|---|---|---|
| 1 — VisitingVA ownership + engine | `2201d04`, `dbd5a0a` | Field + engine registered |
| 2 — Read-path port | `8171b0f`, `e150527`, `b174091`, `ccfc0af` | `GetVisitedPanel` + all readers migrated |
| 3 — Write-path rewrite | `5b3a968`, `941646f`, `e29dfdb`, `6642ec5` | All Visit-family routes through animator |
| 4 — Home-key routing | `a6d029d`, `f753767` | Home/End/PageUp/PageDown added to `emWindow::dispatch_input` fallback (alongside arrow keys, matching the existing Rust divergence from C++'s `emPanel::Input`); VIF's `InputKey::Home → go_home()` handler — a Rust-only accident layered on top of the Rust-only `go_home()` itself — deleted |
| 5 — Deletion | `196da77` | VisitState, visit_stack, pending_animated_visit, 9 methods deleted atomically |
| 6 — Invariant restoration | `a4251fa` | `factor=1.0` restored to `invariant_equilibrium_at_target` |

### 4.3 Plan-defect resolutions (Phase 11 subquestions from the original wave's deferral)

The original wave's Phase 11 audit surfaced six subquestions that made a direct rewrite infeasible. W4 resolved all of them:

| Subquestion | Original state | W4 resolution |
|---|---|---|
| `ViewedX/Y/Width/Height` not on `emView` | Plan assumed they were | Neither of the plan's two options chosen; `GetVisitedPanel(&tree, ...)` takes `&PanelTree` and derives rel coords live via `CalcVisitCoords`. Call-site threading was absorbed into Visit-family signature changes, avoiding the ~30-site cascade the audit feared. |
| `visit_stack` backs Rust-only nav | `go_back`/`go_home`/`Visit` all pushed the stack | `go_back`/`go_home` deleted (no C++ analogue); `Visit` repurposed to delegate to animator. No Rust-only nav feature survives. |
| `.panel` field semantic ambiguity | Plan punted | Moot — audit found it was always "active panel"; now the field is gone. |
| `pending_animated_visit` separate field | Not covered by plan scope | Deleted atomically alongside `visit_stack`. |
| Writer inconsistency at `Visit` | Stack rel coords drifted from viewport | Structurally impossible post-W4 — animator owns goal state; viewport coords are the sole observable source of truth. |
| `invariant_equilibrium_at_target` `factor=1.0` skip | `KNOWN GAP (TODO phase 8)` | Restored; test now runs all five factors. |

### 4.4 Scope expansions during W4

- **`emMainWindow::RecreateContentPanels` snapshot.** Task 3.1 surfaced that the pre-wave Rust code was dropping `title` and `adherent` silently across content rebuild, against C++ `emMainWindow.cpp:297,301,304`. Fixed in commit `5b3a968` as a coupled C++-fidelity repair forced by the new `Visit` signature.
- **`pump_visiting_va` test helper.** W4 Phase 3 Task 3.4 added a bounded-iteration pump (1024 × 0.1 s) on `emView` to drive the animator to completion in tests. ~~Currently `pub` to satisfy cross-crate tests — flagged as leaked API surface~~ **Closed by SP1 on 2026-04-18** (`509535d`): gated behind a `test-support` cargo feature.
- **Test migration cost.** 25 pre-existing tests asserted on the old eager `set_active_panel` side-effect. Per the plan's observational-port frame, these were migrated to drive the animator to convergence (15 per-site + 7 covered by a harness-level `tick()` pump + 3 emcore tests).

### 4.5 `DIVERGED:` annotations added by W4

Four new, all forced by Rust's inability to overload by arity:

- `emView::VisitFullsizedByIdentity` — C++ `VisitFullsized(identity, adherent, utilizeView, subject)` overload.
- `emView::VisitPanel` — C++ `Visit(panel, adherent)` overload.
- `emView::VisitByIdentityBare` — C++ `Visit(identity, adherent, subject)` overload. *(Renamed from `VisitByIdentityShort` by SP1 on 2026-04-18 — `eea7269`.)*
- `emVisitingViewAnimator::SetGoalCoords` — C++ `SetGoal(identity, relX, relY, relA, adherent, subject)` 6-arg overload. *(Renamed from `SetGoalWithCoords` by SP1 on 2026-04-18 — `eea7269`.)*

Per CLAUDE.md, `DIVERGED:` is the prescribed marker for name mismatches; the W4 plan's "no new DIVERGED" clause was retroactively amended — CLAUDE.md takes precedence.

### 4.6 `PHASE-W4-FOLLOWUP:` markers — ~~3, all CoreConfig defaults~~ **0, closed by SP3**

~~All three mark hardcoded `SetAnimParamsByCoreConfig(1.0, 10.0)` calls where C++ would pass `emView`'s `CoreConfig` by reference.~~ **Closed by SP3 on 2026-04-18** (`c6bb071`): `emView` now owns `CoreConfig: Rc<RefCell<emCoreConfig>>`; `SetAnimParamsByCoreConfig` realigned to `(&emCoreConfig)`; all three call sites pass `self.CoreConfig`.

---

## 5. Reviewer findings carried forward

Everything below was flagged during one of the three waves. Items closed by SP1 or SP3 on 2026-04-18 are marked inline; the rest are open and covered by SP4–SP7 (see §8.0).

### 5.1 Important

1. **Latent re-entrancy hazard on `PaintView` / `InvalidatePainting`** (`emViewPort.rs` ~160, ~255). Both methods upgrade `self.window: Weak<RefCell<emWindow>>` and borrow. No current call site holds an existing `&mut emWindow`, but future calls from inside `render()` / `dispatch_input()` / `handle_touch()` would runtime-panic. Phase-6 mitigation (`debug_assert!(self.window.is_some() || cfg!(test), ...)`) catches the missing-backref mode, not re-entrancy. ~~**Follow-up:** explicit doc comment on both methods~~ **Closed by SP1 on 2026-04-18:** SP1 Task 7 audit confirmed detailed re-entrancy warnings already present at `emViewPort.rs:164-174` (PaintView) and `~265-275` (InvalidatePainting). Full audit still deferred until real callers wire.

2. **Double-fire of `GeometrySignal` on popup teardown** (`emView.rs` ~1733). The popup-teardown branch calls `SwapViewPorts(true)` which fires once, then fires `GeometrySignal` a second time explicitly. C++ (`emView.cpp:1678 + 1995`) does the same double-fire, so behaviour matches. ~~**Follow-up:** one-line comment tying the double-fire to the C++ pair.~~ **Closed by SP1 on 2026-04-18** (SP1 Task 6).

3. **`emView::Input` animator-forward is a structural divergence** (`emView.rs:~3749`). C++ `emView::Input` (`emView.cpp:1004`) forwards to the active animator first via `ActiveAnimator->Input(event, state)`, because C++ `emView` owns the `ActiveAnimator` field. The Rust port places the forward on the animator-owner callers — `emWindow::dispatch_input` and `emSubViewPanel::Behavior::Input` — because Rust `emView` does not own an animator slot. Observable behavior matches C++ (animator sees input first); only the *location* of the forward differs. ~~**Resolution:** promote the comment to a formal `DIVERGED:` block.~~ **Closed by SP1 on 2026-04-18** (`eb2f0fe`).

4. **Multi-window framework ambiguity on pixel tallness** (`emGUIFramework.rs` ~362–367). Reads pixel tallness from `windows.values().next()` with `.unwrap_or(1.0)`. Any multi-window future silently picks an arbitrary window. Phase-3's `current_pixel_tallness` threading inherited this hazard. **Open — covered by SP5** (per-view notice dispatch supersedes and resolves this).

5. ~~**Phase-8 test asserts across two engines.**~~ **CLOSED 2026-04-18 by SP4** (`c78c36b`). The Phase-8 test now runs a single real `UpdateEngineClass` via one `DoTimeSlice`, asserting both signal-clock drain and `VisitFullsized` zoom-out in one integrated run. Enabled by the engine-only routing and cached-field probe landed earlier in SP4.

6. **`VisitByIdentity` non-adjacent placement** (W4 residual). C++ colocates `Visit` and `VisitByIdentity` at `emView.cpp:492-523`. ~~Rust port had them ~2200 lines apart.~~ **Closed by SP1 on 2026-04-18** (`93b6b04`): mechanical move; now adjacent.

7. **`pub fn pump_visiting_va` leaked public API** (W4 residual, originally commit `6642ec5`). ~~Currently `pub` on `emView`; Doc-comment marks it "Test-only" but the symbol is visible to every `emcore` consumer.~~ **Closed by SP1 on 2026-04-18** (`509535d`): resolution (a) — `#[cfg(any(test, feature = "test-support"))]` + `test-support` cargo feature on `emcore`, enabled on `eaglemode`'s dev-dep.

8. **Navigation methods carry undeclared Rust-only `NO_NAVIGATE` gate** (W4 residual). `VisitNext/Prev/First/Last/In/Out/Neighbour` each began with `if self.flags.intersects(NO_NAVIGATE | NO_USER_NAVIGATION) { return; }`. C++ `emView.cpp:564-762` has no such gate — gating happens at the caller. ~~**Fix:** either remove the gate and gate at callers, or add a cluster-level `DIVERGED:` comment.~~ **Closed by SP1 on 2026-04-18** (`4d47097`): internal gate removed from all seven methods; user-nav callers gate on `NO_USER_NAVIGATION`, matching C++ exactly.

### 5.2 Minor / style

All items in this section closed by SP1 on 2026-04-18.

1. ~~`emSubViewPanel.rs:48` — literal `1.0` for pixel tallness~~ **Closed by SP1 Task 8**: existing comment already ties the literal to `CurrentPixelTallness`.
2. ~~`emGUIFramework.rs:~393` — borrow pattern collapse.~~ **Closed by SP1 Task 8**: audit found the referenced pattern does not exist in the file; item was based on a misread.
3. ~~`emGUIFramework.rs` `dispatch_forward_events` doc placement.~~ **Closed by SP1 Task 8.**
4. ~~`tests/unit/popup_window.rs` — dead `DISPLAY`/`WAYLAND_DISPLAY` gate.~~ **Stale as of 2026-04-18.** Gate already absent in the file. Current test: `popup_window_creation_path_is_reachable`.
5. ~~**W4 `DIVERGED:` suffix consistency.**~~ **Closed by SP1 on 2026-04-18** (`eea7269`): `VisitByIdentityShort` → `VisitByIdentityBare`; `SetGoalWithCoords` → `SetGoalCoords`.

---

## 6. Markers in tree — current snapshot

| Marker | Count | Status |
|---|---|---|
| `PHASE-5-TODO:` | 0 | Closed by Phases 6/8 |
| `PHASE-6-FOLLOWUP:` | ~~1~~ **0** | Closed by SP1 (2026-04-18) — promoted to `DIVERGED:` block at `emView.rs:~3778` |
| `PHASE-W4-FOLLOWUP:` | ~~3~~ **0** | Closed by SP3 on 2026-04-18 (`c6bb071`) — `emView` now owns `CoreConfig` |
| `UPSTREAM-GAP:` | 2 | Intentional — `IsSoftKeyboardShown` / `ShowSoftKeyboard` in `emViewPort.rs`; no upstream backend overrides them |
| `backend-gap:` | 0 | Phase 8 cleared the last one |
| `KNOWN GAP` | 0 | W4 closed the `factor=1.0` marker |
| `DIVERGED:` (new this wave+W3+W4+SP1+SP3) | 8 | 1 W3 (`OsSurface` accessor inlining) + 4 W4 (arity-overload renames; `VisitByIdentityBare`/`SetGoalCoords` post-SP1) + 1 SP1 (`emView::Input` animator-forward location) + 2 SP3 (`emCoreConfig::VISIT_SPEED_MAX`/`VisitSpeed_GetMaxValue` flattening; `emSubViewPanel` standalone config); all warranted per CLAUDE.md |

---

## 7. Tests

### 7.1 Added

| Test | File | Wave |
|---|---|---|
| `input_routes_through_viewport` + companion | `tests/unit/input_dispatch_chain.rs` | Original (Phase 5) |
| `popup_window_creation_path_is_gated_on_display` + companion — W3 retargeted to `popup_window_creation_path_is_reachable` | `tests/unit/popup_window.rs` | Original (Phase 6), retargeted W3 |
| `test_phase8_popup_close_signal_zooms_out` | inline in `emView.rs` | Original (Phase 8) |
| `max_popup_rect_falls_back_to_home` | `tests/unit/max_popup_rect_fallback.rs` | Original (Phase 9) |
| Popup materialization + cancellation tests (DISPLAY-gated) | W3 integration | W3 |
| `new_popup_pending` inline test | `emWindow.rs` | W3 |
| `visiting_va_owned_by_view`, `visiting_va_cycles_when_activated` | `emView.rs` tests | W4 Phase 1 |
| `get_visited_panel_returns_svp_rel_coords` | `emView.rs` tests | W4 Phase 2 |
| `visit_routes_through_animator`, `visit_panel_short_form_routes_through_animator`, `new_for_view_matches_cpp_initial_state` | `emView.rs` / `emViewAnimator.rs` tests | W4 Phase 1/3 |
| `home_end_pageup_pagedown_route_through_animator`, `home_with_modifier_does_not_navigate_siblings` | `tests/pipeline/focus.rs` | W4 Phase 4 |

### 7.2 Rewritten

- `test_phase7_eoi_engine_fires_via_scheduler` (original Phase 7) replaced the former `tick_eoi`-based test — drives the scheduler explicitly.
- 25 pre-existing tests migrated to drive the animator via `pump_visiting_va` after `Visit`-family calls (W4 Phase 3 Task 3.4).

### 7.3 Deleted

- `view_visit_and_navigation` (renamed from `view_visit_and_back` mid-W4; tested deleted `go_back`/`go_home` API) — W4 Phase 5 Task 5.2, commit `196da77`.

### 7.4 Counts

- Original wave close: 2409/2409.
- Post-W3: 2418/2418 (+9 from W3 additions).
- Post-W4: 2425/2425 (+10 W4 additions, −1 for the deletion, +some test-migration consolidations).
- Post-SP3: **2429/2429** (+2 emCoreConfig SP3 tests + 1 emViewAnimator SP3 test + 1 emView SP3 test).
- Golden: 237/243 throughout all three waves (same 6 pre-existing failures: `composition_tktest_{1x,2x}`, `notice_window_resize`, `testpanel_{expanded,root}`, `widget_file_selection_box`).

### 7.5 Smoke exit-code note

Original plan expected `exit=124` from `timeout 20 cargo run --release ...`. Most phase runs saw `exit=143` (128+15 = SIGTERM — timeout didn't have to escalate to SIGKILL). Both are "stayed alive." No functional difference.

---

## 8. Open residuals — suggested follow-up scope

Numbered in rough priority / landing order. Items marked `[W#]` are cheap C++-mirror ports; items marked `[ARCH]` need a spec and may need a brainstorm.

### 8.0 Sub-project decomposition (added 2026-04-18)

Brainstorming on 2026-04-18 grouped the residuals into seven independently-schedulable sub-projects, each getting its own spec → plan → implementation cycle (SP7 was surfaced during SP3 brainstorming):

| Sub-project | Items | State | Artifacts |
|---|---|---|---|
| **SP1 — W1+W2 cleanup bundle** | ~~1, 3, 4, 5, 6, 7, 8, 9~~ | **Complete 2026-04-18** (merged as `50d50cf`). | `specs/2026-04-18-emview-w1-w2-cleanup-bundle-design.md`, `plans/2026-04-18-emview-w1-w2-cleanup-bundle.md` |
| **SP2 — InvalidateHighlight scoping** | ~~2~~ | **Complete 2026-04-18** — audit found all 5 C++ call sites already mirrored in Rust as part of SP1's W1b task; no additional work needed. | `plans/2026-04-18-emview-followups-wave1.md` Task 3 |
| **SP3 — CoreConfig ownership** | ~~10~~ | **Complete 2026-04-18** (merged as `c6bb071`). | `specs/2026-04-18-emview-sp3-coreconfig-ownership-design.md`, `plans/2026-04-18-emview-sp3-coreconfig-ownership.md` |
| **SP4 — Scheduler re-entrant borrow → Phase-8 test** | ~~14 then 11~~ | **Complete 2026-04-18** (merged as `c78c36b`). | `specs/2026-04-18-emview-sp4-update-engine-only-routing.md`, `plans/2026-04-18-emview-sp4-engine-only-update-routing.md` |
| **SP5 — Per-view notice dispatch** | 12 | Blocked on multi-window roadmap decision | — |
| **SP6 — W3 surface de-dup** | 13 | Optional; may skip entirely | — |
| **SP7 — emContext threading through view/window subsystem** | 15 | Not started; ARCH; surfaced 2026-04-18 during SP3 brainstorming | — |

**Suggested execution order:** ~~SP1~~ → ~~SP3~~ → ~~SP4~~ → (SP5 if unblocked) → SP6 if wanted → SP7 when the motivation arrives. (SP2 turned out to be already done — landed in SP1 as W1b.) SP1, SP3, and SP4 landed 2026-04-18.

**SP4 divergences from plan.** A few implementation details departed from the SP4 plan and are worth recording:

- **Latent-borrow fix at SVPUpdSlice throttle.** During Phase 5, `emView::Update`'s `GetTimeSliceCounter` lookup (`emView.rs:~2082`) was changed from `.borrow()` to `.try_borrow().ok()`, falling back to the cached `self.SVPUpdSlice` on contention. Not in the plan but necessary: the SVPUpdSlice path runs while the scheduler is already borrowed_mut via `DoTimeSlice`, and a plain `borrow()` panics re-entrantly. Harmless throttle-counter stall at worst (triggers only after 1000+ retries in one slice). Landed as part of `03b526d`.
- **Test-support plumbing on `emWindow`.** To let bare-view tests register under a real `WindowId` so `UpdateEngineClass::Cycle` can route correctly, `emWindow::id()` and a `test_window_id: Option<WindowId>` field on `emView` were added (commit `61282db`), plus a `emWindow::new_for_test` constructor. Test-only (`#[cfg(any(test, feature = "test-support"))]`).
- **Task 5.2 trigger mechanism.** The plan suggested driving the Task 5.2 same-slice signal-propagation test via the `SVPChoiceInvalid` path. Investigation showed that path does not actually reach `SetActivePanelBestPossible` under the test setup; the test was rewritten to fire `geometry_signal` via popup teardown, which does reach the queued-signal path. Observable semantics (signal fired from inside `Update` reaches its receiver in the same slice) are what matters, and are asserted identically.
- **Phase-8 test rewrite dependency.** The Phase-8 single-engine rewrite (commit `03b526d`) required the SVPUpdSlice `try_borrow` fix above to avoid spurious panics under the new single-`DoTimeSlice` harness — originally assumed to be orthogonal.

### 8.1 Residual inventory

1. ~~**[W1] `emView::Input` animator forward**~~ **CLOSED 2026-04-18** (`eb2f0fe`). Resolution: structural divergence promoted to formal `DIVERGED:` block at `emView.rs:3778`; `PHASE-6-FOLLOWUP:` prefix removed. Observable behavior already matched C++ via the animator-owner callers. (§5.1 item 3.)
2. ~~**[W1] `InvalidateHighlight` call-site audit**~~ **CLOSED 2026-04-18** — audit on 2026-04-18 found all 5 C++ call sites (`emView.cpp:284, 305, 312, 1211, 1213`) already mirrored in Rust at `emView.rs:594, 599, 1414, 1421, 1457`, each with explicit C++-line comments. Landed as part of SP1's W1b task (`plans/2026-04-18-emview-followups-wave1.md` Task 3); tests at `emView.rs:5986-6036`. The "Rust has no production caller" claim was stale at the time this doc was written.
3. ~~**[W1] Re-entrancy doc comments**~~ **CLOSED 2026-04-18** (SP1 Task 7). Audit confirmed detailed re-entrancy warnings already present at `emViewPort.rs:164-174` (`PaintView`) and `~265-275` (`InvalidatePainting`). No code change required. Full audit still deferred until real callers wire. (§5.1 item 1.)
4. ~~**[W1] GeometrySignal double-fire comment**~~ **CLOSED 2026-04-18** (SP1 Task 6). (§5.1 item 2.)
5. ~~**[W1] Minor cleanups batch**~~ **CLOSED 2026-04-18** (SP1 Task 8). Original §5.2 list was five items; post-audit, the `popup_window.rs` DISPLAY-gate item was stale (file already clean) and the suffix rename became SP1 Task 5. Remaining three items addressed or confirmed already-done.
6. ~~**[W2] `VisitByIdentity` co-location**~~ **CLOSED 2026-04-18** (`93b6b04`). Mechanical move; `VisitByIdentity` now adjacent to `Visit` in `emView.rs`. (§5.1 item 6.)
7. ~~**[W2] `pump_visiting_va` visibility fix**~~ **CLOSED 2026-04-18** (`509535d`). Resolution: option (a) — `#[cfg(any(test, feature = "test-support"))]` gate + `test-support` cargo feature on `emcore`; `eaglemode` enables it on its dev-dep. (§5.1 item 7.)
8. ~~**[W2] Navigation method `NO_NAVIGATE` gate**~~ **CLOSED 2026-04-18** (`4d47097`). Resolution: removed the internal gate from all seven `Visit{Next,Prev,First,Last,In,Out,Neighbour}` methods; user-nav callers gate on `NO_USER_NAVIGATION`. Matches C++ `emView.cpp:564-762` exactly. (§5.1 item 8.)
9. ~~**[W2] `DIVERGED:` suffix rename**~~ **CLOSED 2026-04-18** (`eea7269`). `VisitByIdentityShort` → `VisitByIdentityBare`; `SetGoalWithCoords` → `SetGoalCoords`. (§5.2 item 5.)
10. ~~**[ARCH] `CoreConfig` ownership on `emView`**~~ **CLOSED 2026-04-18** (`c6bb071`). `emView` gained `CoreConfig: Rc<RefCell<emCoreConfig>>`; `SetAnimParamsByCoreConfig` realigned to C++ `(&emCoreConfig)` signature, reading `visit_speed` + `VisitSpeed_GetMaxValue()`; all 3 `PHASE-W4-FOLLOWUP:` markers deleted; `emView::new_for_test` (test-support feature) absorbs test-side call sites; `emSubViewPanel::new` default-constructs a local config with `DIVERGED:` note pending SP7 (emContext threading). Also ports C++ `emVisitingViewAnimator::IsAnimated` accessor.
11. ~~**[W5a / DEFERRED] Phase-8 test promotion**~~ **CLOSED 2026-04-18 by SP4** (`c78c36b`). Both blockers resolved: (i) `emWindow::new_for_test` plus a `test_window_id` field on `emView` let bare-view tests register under a real `WindowId` so `UpdateEngineClass::Cycle` routes correctly (commit `61282db`); (ii) the re-entrant borrow (item 14) was closed via cached-field probe + `SchedOp` queue. The Phase-8 test (`test_phase8_popup_close_signal_zooms_out`) now runs exactly one `DoTimeSlice` through a single real engine, asserting both signal-clock drain and `VisitFullsized` zoom-out.
12. **[W5b / DEFERRED] Per-view notice dispatch (emView.cpp:1312 parity)** — successor to the multi-window-pixel-tallness item. W5b landed the classification pass (`DIVERGED:` note expanded in `emPanelTree.rs`; `TODO(per-view-notice-dispatch)` added at the `pixel_tallness` site in `emGUIFramework.rs`). The architectural fix remains open: move `NoticeList` ownership from `PanelTree` to `emView`, dispatch `HandleNotice` from `emView::Update` once per view per frame using that view's own `CurrentPixelTallness`, and establish panel→view ownership in `PanelTree` (or partition notices by walking each view's subtree from its root panel). `run_panel_cycles` is a separate Rust-only construct (C++ panels self-register as engines via `emEngine` inheritance, which the Rust port does not mirror) and stays out of this workstream. Pre-condition before scheduling: confirm multi-window support is on the near-term roadmap; until then the current global dispatch is a tolerable single-window shortcut.
13. **[ARCH] W3 surface-creation de-duplication** (optional) — extract `build_materialized_surface(gpu, winit_window) -> MaterializedSurface` to deduplicate ~50 lines between `materialize_popup_surface` and `emWindow::create()`. (§3.5 item 1.)
14. ~~**[ARCH] emView::Update scheduler re-entrant borrow**~~ **CLOSED 2026-04-18 by SP4** (`c78c36b`). Chosen fix: hybrid — option (b)'s cached-field probe (`popup_close_signal_fired: bool`, refreshed at signal-processing time) for the specific popup-close check, plus a structural `SchedOp` enum + `pending_sched_ops: Vec<SchedOp>` queue so scheduler mutations issued from inside `Update` (connect/disconnect/remove_signal/wake_up/fire) are deferred until after the borrow unwinds. `emView::update()` was deleted outright; `UpdateEngineClass::Cycle` is now the sole routing path, matching C++ one-engine-per-view semantics. 8 previous scheduler-borrow sites migrated to `queue_or_apply`; direct `view_mut().update(tree)` calls removed from `emGUIFramework::about_to_wait`. Smoke + 2432/2432 nextest + 237/6 golden parity preserved.

15. **[ARCH] `emContext` threading through view/window subsystem** — surfaced 2026-04-18 during SP3 brainstorming (expanded from what was then called "option C" for SP3 — acquiring `CoreConfig` the C++ way rather than constructing it ad-hoc). SP3 landed the direct-injection approach (`emView::new` takes `Rc<RefCell<emCoreConfig>>`); this item covers the broader architectural gap of routing acquisition through a context tree.

    **Gap.** C++ `emView::emView(emContext & parentContext, ViewFlags)` takes a parent context, calls `emCoreConfig::Acquire(GetRootContext())` at construction (`emView.cpp:35`), and participates in a tree of contexts that inherit services (clipboard, config models, the model registry). The Rust port has an `emContext` type (`crates/emcore/src/emContext.rs`, ~393 lines) with `NewRoot`, `NewChild`, scheduler lookup via parent chain, and typed-singleton model Acquire — but **no caller in the view/window/panel subsystem ever threads one through.** Specifically:
    - `emView::new(root, w, h)` takes no context. ~15 production + test call sites.
    - `emWindow::create` takes `&GpuContext` (wgpu) but no `emContext`; `emWindow::new_popup_pending` likewise.
    - `emSubViewPanel.rs:51` builds a child `emView` inside a panel with no context in scope.
    - The ~38 existing `emContext::NewRoot()` calls in `crates/emmain/`, `crates/emfileman/`, and a few unit tests all construct **ad-hoc root contexts** local to model-specific code paths (primarily for `emClipboard` and config-model `Acquire` lookups). They are not linked to each other or to a process-wide root. There is currently no single root `emContext` for an eaglemode process.
    - `emGUIFramework` / `App` has no root context member; `emCoreConfig::Acquire` is effectively unused at runtime in the main binary.

    **Why this is a separate sub-project, not a piggyback on SP3.** SP3's charter (§8.1 item 10) is to close three `PHASE-W4-FOLLOWUP:` markers. Doing that correctly requires the view to *have* a `CoreConfig`; it does not require the view to *acquire* that config from a context. Routing through `Acquire(GetRootContext())` — the way C++ does it — forces all of the following decisions, none of which are CoreConfig-specific:
    1. **Where the root `emContext` lives.** `emGUIFramework` is the natural owner (one root per process, scheduler already attached); but that means adding an `emContext` field there and wiring `emContext::NewRootWithScheduler` into framework construction.
    2. **How contexts relate to the panel/view tree.** C++ nests: `RootContext → WindowContext → (views and panels hang off that)`. Rust would need to decide whether each `emWindow` owns a child context, whether `emView` owns one (per C++), whether `PanelTree` does, and how `emSubViewPanel`'s inner view inherits.
    3. **How existing ad-hoc `NewRoot()` call sites migrate.** The ~38 scattered `NewRoot` calls in `emmain`/`emfileman`/tests need to be classified: (a) production paths that should be pulled into the shared root, (b) test harnesses that can keep standalone roots, (c) cases where the ad-hoc root is masking a real missing link. Some of these are load-bearing (they serve as the registry that `Acquire` populates) and cannot be removed without a replacement.
    4. **Clipboard and other services.** `emContext::set_clipboard` is already wired, but nothing currently installs a real clipboard at process start because there is no canonical root to install it on. Threading fixes this incidentally.
    5. **Test ergonomics.** ~15 `emView::new` call sites across unit and integration tests don't build contexts today. A helper like `emView::new_for_tests(root, w, h)` (constructs a throwaway root context internally) keeps migration cost bounded, but is itself a small API decision.

    **Scope of SP7.**
    - Add a root `emContext` to `emGUIFramework` / `App`, created alongside the scheduler via `emContext::NewRootWithScheduler`.
    - Change `emView::new` to take `&Rc<emContext>` (matching C++'s `emContext & parentContext`); store `CoreConfig` on `emView` by calling `emCoreConfig::Acquire` at construction. This replaces SP3's direct-injection approach.
    - Thread contexts through `emWindow::create` and `emWindow::new_popup_pending`; decide whether popup windows nest under their parent's context (likely yes — matches C++ popup lifetime).
    - Thread contexts through `emSubViewPanel` so inner views inherit.
    - Migrate the ad-hoc `NewRoot()` call sites in `emmain`/`emfileman` production code to use the process root (tests can keep their own).
    - Install a real `emClipboard` implementation at the process root as part of this wiring (currently unwired).
    - Provide an `emView::new_for_tests(...)` or equivalent to keep the ~15 test call sites ergonomic.

    **Blast radius estimate.** ~15 `emView::new` sites + ~2 `emWindow::create`/`new_popup_pending` sites + ~38 `NewRoot()` sites (not all migrate) + `emGUIFramework` constructor + `emSubViewPanel` + test-harness helpers. Roughly 50–80 touched call sites, most mechanical. The *non-mechanical* work is the three decisions above (where the root lives; how contexts nest; which ad-hoc `NewRoot`s migrate).

    **Dependencies and ordering.** No hard blockers from SP3–SP6. Naturally pairs with whatever multi-window roadmap decision unblocks SP5, because per-window context nesting is the same question asked from a different direction. Defer until there is a motivating feature (real clipboard support, config persistence in the main binary, or the multi-window work for SP5); otherwise the churn buys nothing observable today.

    **Discovery trail.** SP3 brainstorming (2026-04-18) proposed three scopes: (A) minimal CoreConfig field, (B) also realign `SetAnimParamsByCoreConfig` signature to match C++, (C) also route acquisition through `emContext`. Investigation of (C) found that `emContext` is not threaded anywhere in the view/window subsystem today, making (C) a sub-project in its own right. SP3 adopted (B); (C) was extracted as this item.

---

## Appendix: historical sequencing (from the superseded roadmap)

For reference — the sequencing that the now-superseded roadmap doc proposed before W3 and W4 landed:

```
W1 (C++-mirror ports) ─┬─ W2 (cleanups) → W3 (popup arch) → W4 (visit-stack) → W5a (Phase-8 test promotion)
                       │                                                        └ W5b (multi-window pixel tallness)
```

Status against that plan: **W3, W4, W1, and W2 are done** (W1+W2 landed together as SP1 on 2026-04-18). W5 remains, now decomposed as SP4 (see §8.0); SP2, SP3, SP5, SP6 cover the additional residuals that surfaced during W3/W4 closeout.

End of report.
