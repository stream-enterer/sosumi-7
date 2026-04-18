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
| Commits on main | **~44** (14 original + W3 cluster + 14 W4 + merges) |
| Tests | 2425/2425 nextest, 9 skipped, 0 failed |
| Golden | 237 passed / 6 failed (baseline parity — same 6 pre-existing failures across all waves) |
| Smoke (`timeout 20 cargo run --release --bin eaglemode`) | exits 143 / 124 — program stays alive |
| Scaffolds still in tree | **0** (both `PopupPlaceholder` and the visit-stack scaffolding are gone) |
| Phase-follow-up markers | 1 `PHASE-6-FOLLOWUP` (VIF-chain migration, out of scope) + 3 `PHASE-W4-FOLLOWUP` (CoreConfig defaults) + 2 `UPSTREAM-GAP` (intentional) |
| Known Rust-port incompletenesses remaining | Animator-Input forward, `InvalidateHighlight` call sites, re-entrancy doc comments, per-view notice dispatch (was: multi-window pixel tallness), `emView::Update` scheduler re-entrant borrow, `CoreConfig` ownership on `emView`, four W4 polish items |

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
- Still open: 1 (VIF-chain migration at `emView.rs:3626`). Belongs to the unported `emView::Input` animator-forward work — see §8.

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
- **`pump_visiting_va` test helper.** W4 Phase 3 Task 3.4 added a bounded-iteration pump (1024 × 0.1 s) on `emView` to drive the animator to completion in tests. Currently `pub` to satisfy cross-crate tests — flagged as leaked API surface (§8 item 4).
- **Test migration cost.** 25 pre-existing tests asserted on the old eager `set_active_panel` side-effect. Per the plan's observational-port frame, these were migrated to drive the animator to convergence (15 per-site + 7 covered by a harness-level `tick()` pump + 3 emcore tests).

### 4.5 `DIVERGED:` annotations added by W4

Four new, all forced by Rust's inability to overload by arity:

- `emView::VisitFullsizedByIdentity` — C++ `VisitFullsized(identity, adherent, utilizeView, subject)` overload.
- `emView::VisitPanel` — C++ `Visit(panel, adherent)` overload.
- `emView::VisitByIdentityShort` — C++ `Visit(identity, adherent, subject)` overload.
- `emVisitingViewAnimator::SetGoalWithCoords` — C++ `SetGoal(identity, relX, relY, relA, adherent, subject)` 6-arg overload.

Per CLAUDE.md, `DIVERGED:` is the prescribed marker for name mismatches; the W4 plan's "no new DIVERGED" clause was retroactively amended — CLAUDE.md takes precedence.

### 4.6 `PHASE-W4-FOLLOWUP:` markers — 3, all CoreConfig defaults

All three mark hardcoded `SetAnimParamsByCoreConfig(1.0, 10.0)` calls where C++ would pass `emView`'s `CoreConfig` by reference. Rust `emView` does not yet own a `CoreConfig` field. Locations: `crates/emcore/src/emView.rs:897, 921, 3080`.

---

## 5. Reviewer findings carried forward

Everything below was flagged during one of the three waves and remains open. Grouped by severity, not by wave.

### 5.1 Important

1. **Latent re-entrancy hazard on `PaintView` / `InvalidatePainting`** (`emViewPort.rs` ~160, ~255). Both methods upgrade `self.window: Weak<RefCell<emWindow>>` and borrow. No current call site holds an existing `&mut emWindow`, but future calls from inside `render()` / `dispatch_input()` / `handle_touch()` would runtime-panic. Phase-6 mitigation (`debug_assert!(self.window.is_some() || cfg!(test), ...)`) catches the missing-backref mode, not re-entrancy. **Follow-up:** explicit doc comment on both methods; full audit when real callers are wired.

2. **Double-fire of `GeometrySignal` on popup teardown** (`emView.rs` ~1733). The popup-teardown branch calls `SwapViewPorts(true)` which fires once, then fires `GeometrySignal` a second time explicitly. C++ (`emView.cpp:1678 + 1995`) does the same double-fire, so behaviour matches; comment does not acknowledge `SwapViewPorts` already fired. **Follow-up:** one-line comment tying the double-fire to the C++ pair.

3. **`emView::Input` animator-forward is a structural divergence** (`emView.rs:3778`). ~~"missing"~~ — **revised 2026-04-18 during W1/W2 plan-writing.** C++ `emView::Input` (`emView.cpp:1004`) forwards to the active animator first via `ActiveAnimator->Input(event, state)`, because C++ `emView` owns the `ActiveAnimator` field. The Rust port places the forward on the animator-owner callers — `emWindow::dispatch_input` (`emWindow.rs:840-862`) and `emSubViewPanel::Behavior::Input` — because Rust `emView` does not own an animator slot. Observable behavior matches C++ (animator sees input first); only the *location* of the forward differs. The existing prose comment at the call site documents this. **Resolution:** promote the comment to a formal `DIVERGED:` block; drop the `PHASE-6-FOLLOWUP:` prefix. Covered by W1/W2 bundle Task 1.

4. **Multi-window framework ambiguity on pixel tallness** (`emGUIFramework.rs` ~362–367). Reads pixel tallness from `windows.values().next()` with `.unwrap_or(1.0)`. Any multi-window future silently picks an arbitrary window. Phase-3's `current_pixel_tallness` threading inherited this hazard. **Follow-up:** TODO marker plus a multi-window-design decision when that feature lands.

5. **Phase-8 test asserts across two engines.** (`test_phase8_popup_close_signal_zooms_out` inline in `emView.rs`.) Half A observes `SwapViewPorts` connecting `close_signal` → real update engine before the swap; Half B drains signal-vs-engine-clock using a dummy engine as `update_engine_id`. Both halves exercise production code paths, never in one integrated run. **Follow-up:** widen `PanelTree::get_mut` visibility (has other consequences) or build a test-support shim.

6. **`VisitByIdentity` non-adjacent placement** (W4 residual). `Visit` lives at `emView.rs:~857`; its identity-keyed partner `VisitByIdentity` (7-arg) lives at `~3072`, ~2200 lines away. C++ colocates them at `emView.cpp:492-523`. File-and-Name Correspondence expects adjacency. **Fix:** mechanical move.

7. **`pub fn pump_visiting_va` leaked public API** (W4 residual, commit `6642ec5`). Currently `pub` on `emView` so cross-crate tests can call it. Doc-comment marks it "Test-only" but the symbol is visible to every `emcore` consumer. **Fix options:** (a) `#[cfg(any(test, feature = "test-support"))]` + dev-dep feature; (b) `pub(crate)` helper re-exported from a test-support submodule; (c) accept and document as a sanctioned test API.

8. **Navigation methods carry undeclared Rust-only `NO_NAVIGATE` gate** (W4 residual). `VisitNext/Prev/First/Last/In/Out/Neighbour` each begin with `if self.flags.intersects(NO_NAVIGATE | NO_USER_NAVIGATION) { return; }`. C++ `emView.cpp:564-762` has no such gate — gating happens at the caller. Pre-wave behaviour, now living inside newly-authored bodies without a `DIVERGED:`. **Fix:** either remove the gate and gate at callers (matches C++) or add a cluster-level `DIVERGED:` comment.

### 5.2 Minor / style

Each of these is a one-liner; batchable in a single cleanup pass.

1. `emSubViewPanel.rs:48` — literal `1.0` for pixel tallness not symbolically tied to `CurrentPixelTallness`'s initial value. Add one-line comment.
2. `emGUIFramework.rs:~393` — `let mut win = rc.borrow_mut(); let win = &mut *win;` could be `&mut *rc.borrow_mut()`.
3. `emGUIFramework.rs` `dispatch_forward_events` — added doc-comment describes caller-side usage, not the function itself. Move to the call site or drop.
4. ~~`tests/unit/popup_window.rs` — dead `DISPLAY`/`WAYLAND_DISPLAY` gate.~~ **Stale as of 2026-04-18.** W1/W2 plan-writing re-read the file; the gate is already absent. The current test is `popup_window_creation_path_is_reachable` (a direct reachability assertion with no DISPLAY branching). Item closed without action.
5. **W4 `DIVERGED:` suffix consistency.** Four new suffixes (`WithCoords`, `Short`, `ByIdentity`, `ByIdentityShort`). `Short` is an antonym heuristic rather than a semantic descriptor. More uniform names (`VisitBare`/`VisitByIdentityBare` or `VisitCoords`/`VisitByIdentityCoords`) would make the overload family self-describing. Cheap rename now; expensive once external consumers reference the names.

---

## 6. Markers in tree — current snapshot

| Marker | Count | Status |
|---|---|---|
| `PHASE-5-TODO:` | 0 | Closed by Phases 6/8 |
| `PHASE-6-FOLLOWUP:` | 1 | VIF-chain migration at `emView.rs:~3626` (animator-forward work) |
| `PHASE-W4-FOLLOWUP:` | 3 | CoreConfig defaults at `emView.rs:897, 921, 3080` |
| `UPSTREAM-GAP:` | 2 | Intentional — `IsSoftKeyboardShown` / `ShowSoftKeyboard` in `emViewPort.rs`; no upstream backend overrides them |
| `backend-gap:` | 0 | Phase 8 cleared the last one |
| `KNOWN GAP` | 0 | W4 closed the `factor=1.0` marker |
| `DIVERGED:` (new this wave+W3+W4) | 5 | 1 W3 (`OsSurface` accessor inlining) + 4 W4 (arity-overload renames); all warranted per CLAUDE.md |

---

## 7. Tests

### 7.1 Added

| Test | File | Wave |
|---|---|---|
| `input_routes_through_viewport` + companion | `tests/unit/input_dispatch_chain.rs` | Original (Phase 5) |
| `popup_window_creation_path_is_gated_on_display` + companion | `tests/unit/popup_window.rs` | Original (Phase 6) |
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
- Post-W4: **2425/2425** (+10 W4 additions, −1 for the deletion, +some test-migration consolidations).
- Golden: 237/243 throughout all three waves (same 6 pre-existing failures: `composition_tktest_{1x,2x}`, `notice_window_resize`, `testpanel_{expanded,root}`, `widget_file_selection_box`).

### 7.5 Smoke exit-code note

Original plan expected `exit=124` from `timeout 20 cargo run --release ...`. Most phase runs saw `exit=143` (128+15 = SIGTERM — timeout didn't have to escalate to SIGKILL). Both are "stayed alive." No functional difference.

---

## 8. Open residuals — suggested follow-up scope

Numbered in rough priority / landing order. Items marked `[W#]` are cheap C++-mirror ports; items marked `[ARCH]` need a spec and may need a brainstorm.

### 8.0 Sub-project decomposition (added 2026-04-18)

Brainstorming on 2026-04-18 grouped the 14 residuals into six independently-schedulable sub-projects, each getting its own spec → plan → implementation cycle:

| Sub-project | Items | State | Artifacts |
|---|---|---|---|
| **SP1 — W1+W2 cleanup bundle** | 1, 3, 4, 5, 6, 7, 8, 9 | Spec + plan written 2026-04-18; awaiting execution | `specs/2026-04-18-emview-w1-w2-cleanup-bundle-design.md`, `plans/2026-04-18-emview-w1-w2-cleanup-bundle.md` |
| **SP2 — InvalidateHighlight scoping** | 2 | Not started; standalone scoping question | — |
| **SP3 — CoreConfig ownership** | 10 | Not started; ARCH | — |
| **SP4 — Scheduler re-entrant borrow → Phase-8 test** | 14 then 11 | Not started; 14 blocks 11 — one combined spec | — |
| **SP5 — Per-view notice dispatch** | 12 | Blocked on multi-window roadmap decision | — |
| **SP6 — W3 surface de-dup** | 13 | Optional; may skip entirely | — |

**Suggested execution order:** SP1 → SP3 → SP4 → (SP5 if unblocked) → SP6 if wanted. SP2 slots anywhere or folds into SP1.

### 8.1 Residual inventory

1. **[W1] `emView::Input` animator forward** — ~~port `emView.cpp:1004`'s `ActiveAnimator->Input(event, state)`~~ **revised 2026-04-18**: observable behavior already matches C++ via the animator-owner callers; resolution is to promote the existing in-code comment at `emView.rs:3778` to a formal `DIVERGED:` block and drop the `PHASE-6-FOLLOWUP:` prefix. Closes one `PHASE-6-FOLLOWUP:` marker. (§5.1 item 3.) Covered by W1/W2 bundle Task 1 — see `docs/superpowers/plans/2026-04-18-emview-w1-w2-cleanup-bundle.md`.
2. **[W1] `InvalidateHighlight` call-site audit** — the C++ equivalent is called from focus-state changes and similar; Rust has no production caller. (§5.1 item 3 residual, originally §4.8.)
3. **[W1] Re-entrancy doc comments** on `PaintView`/`InvalidatePainting`; full audit deferred until real callers wire. (§5.1 item 1.)
4. **[W1] GeometrySignal double-fire comment** — one line at `emView.rs:~1733`. (§5.1 item 2.)
5. **[W1] Minor cleanups batch** (§5.2, ~~five~~ four items post-stale-audit — the `popup_window.rs` gate item was closed without action on 2026-04-18).
6. **[W2] `VisitByIdentity` co-location** — mechanical move. (§5.1 item 6.)
7. **[W2] `pump_visiting_va` visibility fix** — pick (a)/(b)/(c). (§5.1 item 7.)
8. **[W2] Navigation method `NO_NAVIGATE` gate** — remove or add `DIVERGED:`. (§5.1 item 8.)
9. **[W2] `DIVERGED:` suffix rename** — while external consumers are still zero. (§5.2 item 5.)
10. **[ARCH] `CoreConfig` ownership on `emView`** — give `emView` a real `CoreConfig` field (C++ `emView.h` holds `emRef<emCoreConfig>`); replace the three hardcoded `SetAnimParamsByCoreConfig(1.0, 10.0)` calls with real config values. Closes all 3 `PHASE-W4-FOLLOWUP:` markers. (§4.6.)
11. **[W5a / DEFERRED] Phase-8 test promotion** — original intent: drive `close_signal` end-to-end through a single real engine. W5a investigation found this requires two production fixes, not a test-only change: (i) `UpdateEngineClass::Cycle` goes through `ctx.windows.get(&window_id)`, so bare-view tests can't receive the engine call; (ii) `emView::Update:~2288` does `sched.borrow()` while callers hold `sched.borrow_mut()` across `DoTimeSlice`, which panics re-entrantly. The single-engine rewrite is blocked on item 14 below. W5a landed a load-bearing doc comment at the test and a BUG block at the `Update` call site pointing at the successor workstream.
12. **[W5b / DEFERRED] Per-view notice dispatch (emView.cpp:1312 parity)** — successor to the multi-window-pixel-tallness item. W5b landed the classification pass (`DIVERGED:` note expanded in `emPanelTree.rs`; `TODO(per-view-notice-dispatch)` added at the `pixel_tallness` site in `emGUIFramework.rs`). The architectural fix remains open: move `NoticeList` ownership from `PanelTree` to `emView`, dispatch `HandleNotice` from `emView::Update` once per view per frame using that view's own `CurrentPixelTallness`, and establish panel→view ownership in `PanelTree` (or partition notices by walking each view's subtree from its root panel). `run_panel_cycles` is a separate Rust-only construct (C++ panels self-register as engines via `emEngine` inheritance, which the Rust port does not mirror) and stays out of this workstream. Pre-condition before scheduling: confirm multi-window support is on the near-term roadmap; until then the current global dispatch is a tolerable single-window shortcut.
13. **[ARCH] W3 surface-creation de-duplication** (optional) — extract `build_materialized_surface(gpu, winit_window) -> MaterializedSurface` to deduplicate ~50 lines between `materialize_popup_surface` and `emWindow::create()`. (§3.5 item 1.)
14. **[ARCH] emView::Update scheduler re-entrant borrow** — discovered during W5a investigation. `emView::Update` at `emView.rs:~2288` calls `self.scheduler.as_ref().unwrap().borrow()` to check `is_signaled_for_engine(close_signal, eng_id)`. Callers (including `emGUIFramework::about_to_wait:491` in production, and any principled single-engine test design) hold `sched.borrow_mut()` across `DoTimeSlice`, whose engine chain runs `UpdateEngineClass::Cycle` → `emView::Update`. The inner `borrow()` panics re-entrantly. In production this path fires whenever a popup's `close_signal` is pending when the update engine's cycle runs — it is not merely a test issue. Fix options: (a) add an `Option<&mut EngineCtxInner>` (or equivalent) parameter to `emView::Update` so the caller can pass scheduler context directly rather than reaching back through `self.scheduler` — cascades through ~20 call sites; (b) cache the signal's clock value in a new field on `emView` during signal processing so `Update` can check without going through the scheduler. Option (a) matches C++ structure more closely (`emView::Update` in C++ has direct access to `Scheduler` via the view context); option (b) is a smaller localized change. Blocks item 11.

---

## Appendix: historical sequencing (from the superseded roadmap)

For reference — the sequencing that the now-superseded roadmap doc proposed before W3 and W4 landed:

```
W1 (C++-mirror ports) ─┬─ W2 (cleanups) → W3 (popup arch) → W4 (visit-stack) → W5a (Phase-8 test promotion)
                       │                                                        └ W5b (multi-window pixel tallness)
```

Status against that plan: **W3 and W4 are done.** W1, W2, W5 (renamed to items 1–5, 11, 12 in §8 above) remain.

End of report.
