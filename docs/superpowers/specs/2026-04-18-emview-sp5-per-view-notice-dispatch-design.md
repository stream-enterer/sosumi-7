# SP5 — Per-view notice dispatch (emView.cpp:1312 parity)

**Date:** 2026-04-18
**Scope:** Close the design-intent violation in the notice-dispatch driver. Restore per-view `HandleNotice` dispatch from `emView::Update`, using each view's own `CurrentPixelTallness`.
**Source:** `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.1 item 12.
**Precondition status:** Multi-window is not on the near-term roadmap. SP5 is designed now to specify the port-fidelity debt; implementation scheduling is a separate decision.

This spec is an **observational port** design per `CLAUDE.md`. Authority order: C++ source → golden tests → Rust idiom → LLM convenience.

---

## 1. Classification under the Port Ideology

The existing `DIVERGED:` block at `crates/emcore/src/emPanelTree.rs:291–315` classifies three axes of the notice-list divergence:

| Axis | C++ | Current Rust | Classification | SP5 target |
|---|---|---|---|---|
| **Storage shape** (where `NoticeList` lives) | per-view, on `emView` | global, on `PanelTree` | Previously labeled *forced*. **Re-classified** under strict CLAUDE.md: not forced — only the *data structure* is forced (ring vs Vec). Ownership location is design intent. | Move ownership to `emView`. |
| **Data structure** | `PanelRingNode *` intrusive sentinel | `Vec<PanelId>` + two `Option<PanelId>` fields | Forced (Rust ownership rules; unsafe + custom allocator otherwise). | Unchanged. |
| **Dispatch driver** | `emView::Update` calls `HandleNotice` per view with that view's `CurrentPixelTallness` (`emView.cpp:1312`) | `emGUIFramework::about_to_wait` calls `PanelTree::HandleNotice` once globally with an arbitrarily-chosen window's `CurrentPixelTallness` | Design-intent violation. | Dispatch per view from `emView::Update`. |
| **Panel→view linkage** | `emPanel::View` is `emView &` (direct reference member) | Not present; panels route notices through `PanelTree` without a view back-reference | Design intent. Rust can mirror via `Weak<RefCell<emView>>` without `unsafe`; earlier avoidance was Rust convenience (outranked by authority #1). | Add `View: Weak<RefCell<emView>>` to `emPanel`. |

The "panel→view linkage" row is the load-bearing structural change. It is not forced — `Weak<RefCell<emView>>` is a standard Rust pattern already used for `emWindow` in Phase 6. Preserving C++'s reference model costs wrapping `emView` in `Rc<RefCell<>>` at its two owner sites (`emWindow::view` and `emSubViewPanel`'s inner view). Under CLAUDE.md authority, Rust convenience (avoiding the cascade) cannot outrank C++ structure.

## 2. Target state

```
emWindow::view: Rc<RefCell<emView>>
emSubViewPanel inner view: Rc<RefCell<emView>>
emPanel::View: Weak<RefCell<emView>>        // 1:1 with C++ emPanel::View &
emView::NoticeList: Vec<PanelId>            // 1:1 with C++ emView::NoticeList
emView::notice_ring_{head,tail}: Option<PanelId>
emView::has_pending_notices: bool           // per-view

// Queue path:
emPanel-internal call → panel.View.upgrade().borrow_mut().AddToNoticeList(id)
                                          ↓
                                 pushes into that view's NoticeList

// Dispatch path:
UpdateEngineClass::Cycle → emView::Update(&mut self, &mut tree)
    ↓
  HandleNotice(&mut self, &mut tree, window_focused)   // new method on emView
    — drains self.NoticeList
    — runs HandleNotice/HandleLayout on each panel with self.CurrentPixelTallness
```

`emGUIFramework::about_to_wait` drops all notice-dispatch responsibility. The `pixel_tallness = windows.values().next()...` block and the `tree.HandleNotice(...)` call are deleted. `run_panel_cycles(pixel_tallness)` remains (Rust-only construct; out of scope per closeout doc §8.1 item 12).

## 3. Scope

### In scope

1. **`Rc<RefCell<emView>>` cascade.** Convert `emWindow::view: emView` → `Rc<RefCell<emView>>`. Same for `emSubViewPanel`'s inner view. Thread through every existing `view()` / `view_mut()` accessor site. Precedent: Phase-6 emWindow conversion (~27 files / ~160 lines).
2. **`emPanel::View` field.** Add `View: Weak<RefCell<emView>>` to `emPanel`. Set at every `emPanel::new` / `PanelTree` insert path. Non-dangling at construction is a debug-asserted invariant.
3. **NoticeList migration to `emView`.** Move `notice_ring_head_next`, `notice_ring_head_prev`, `has_pending_notices` from `PanelTree` to `emView`. Migrate the ~20 `add_to_notice_list(id)` call sites inside `PanelTree` to `self.panels[id].View.upgrade().expect(...).borrow_mut().AddToNoticeList(id)`.
4. **Dispatch relocation.** Port `PanelTree::HandleNotice` body to a new `emView::HandleNotice(&mut self, tree: &mut PanelTree, window_focused: bool)`. Call it from `emView::Update` at the slot matching `emView.cpp:1312`. Delete `PanelTree::HandleNotice` (dead after all call sites migrate).
5. **Framework-side cleanup.** In `emGUIFramework::about_to_wait`: delete the `pixel_tallness` block, the `tree.HandleNotice(...)` call, and the `TODO(per-view-notice-dispatch)` comment. Keep `run_panel_cycles(pixel_tallness)` — Rust-only, out of scope.
6. **SP4 test-hack removal.** Delete `test_window_id: Option<WindowId>` on `emWindow` (`crates/emcore/src/emWindow.rs:97`) and the `emWindow::new_for_test` constructor. These exist only to substitute for missing real view identity under `UpdateEngineClass::Cycle` routing. SP5's `Rc<RefCell<emView>>` ownership makes real identity available via the panel-graph back-reference; tests migrate to real `emWindow`-adjacent construction (or a replacement helper that yields a genuine `Rc<RefCell<emView>>` without wgpu). `emView::new_for_test` (an SP3 emCoreConfig-defaulting helper, orthogonal to the SP4 identity hack) is unaffected.
7. **Divergence annotations.** Update the existing `DIVERGED:` block at `emPanelTree.rs:291–315`: delete the "dispatch driver" paragraph (closed), retain only the data-structure idiom-adaptation note, and move it to `emView.rs` adjacent to the migrated fields. Update closeout doc §8.1 item 12 to "CLOSED by SP5".

### Out of scope (explicit)

- `run_panel_cycles` remains global. Rust-only construct (C++ panels self-register as engines via `emEngine` inheritance, not mirrored in Rust). Separate workstream, not SP5.
- SP7 (emContext threading). Orthogonal; SP5 uses direct `Rc<RefCell<emView>>` ownership, not context-tree lookup.
- Notice *queueing* semantics. Which code paths call `AddToNoticeList` and when stays byte-for-byte with current behavior (already matches C++).
- Multi-window runtime wiring beyond what SP5's unit-test demand requires.

## 4. Phases

Gated, executed in order.

### Phase 1 — `Rc<RefCell<emView>>` cascade

Convert `emWindow::view: emView` → `Rc<RefCell<emView>>`. Convert `emSubViewPanel`'s inner view similarly. Thread through all accessor sites.

**No behavioral change.** Pure ownership reshape.

Gate:
- `cargo check`, `cargo clippy -- -D warnings`, `cargo-nextest ntr` all green.
- Golden suite: 237 pass / 6 fail (same baseline six).
- Line-count report. If >200 lines changed, stop and escalate per CLAUDE.md's 50-line threshold convention.

### Phase 2 — `emPanel::View` field

Add `View: Weak<RefCell<emView>>` to `emPanel`. Populate at every construction path (`PanelTree::insert_root`, `insert_child`, and any test-construction paths). The `emView` creating the panel supplies its own `Rc::downgrade(self_rc)`.

Debug-assert centrally: on every `add_to_notice_list` entry, `panel.View.upgrade().is_some()`.

Gate:
- Every `emPanel` instance in every test has a non-dangling `View` by construction.
- nextest green; no new panics under test.
- Golden parity (single-window: behavior unchanged because dispatch path is unchanged until Phase 3).

### Phase 3 — NoticeList migration

Move `notice_ring_head_next`, `notice_ring_head_prev`, `has_pending_notices` from `PanelTree` to `emView`. Migrate all ~20 `add_to_notice_list(id)` call sites inside `PanelTree` (enumerated by grep in the plan) to route through `panel.View`.

`PanelTree::HandleNotice` temporarily retains its body but loses the ring fields (reads through the view instead). This is a stepping-stone state; Phase 4 deletes it.

Gate:
- nextest green.
- Golden parity (single-window: single view, dispatch still global but state lives per-view; output identical).

### Phase 4 — Dispatch relocation

Add `emView::HandleNotice(&mut self, tree: &mut PanelTree, window_focused: bool)` with the body ported from `PanelTree::HandleNotice`. Call it from `emView::Update` at the slot matching `emView.cpp:1312`. Delete `PanelTree::HandleNotice`.

In `emGUIFramework::about_to_wait`: delete the `pixel_tallness` block, the global `HandleNotice` call, and the `TODO(per-view-notice-dispatch)` comment. Keep `run_panel_cycles`.

**Add a multi-view unit test** in `emView.rs`: two `emView`s with distinct `CurrentPixelTallness`, a notice queued on a panel under each, each view's `Update` called, assert each `HandleNotice` ran with the correct pixel tallness for its view and did not see the other view's panels.

Gate:
- nextest green including the new multi-view test.
- Golden parity (single-window: one view, one Update, one HandleNotice — identical to pre-SP5).
- Smoke test `timeout 20 cargo run --release --bin eaglemode` exits 124/143.

### Phase 5 — SP4 test-hack removal

Delete `test_window_id: Option<WindowId>` on `emWindow` and the `emWindow::new_for_test` constructor. Migrate `emWindow::new_for_test` call sites (if any) to real `emWindow` construction or a replacement helper producing a genuine `Rc<RefCell<emView>>`. `emView::new_for_test` (SP3 helper) is unaffected and stays.

Gate:
- `grep -n "test_window_id" crates/` returns zero matches.
- `grep -n "emWindow::new_for_test\|Window::new_for_test" crates/` returns zero matches.
- nextest green.

### Phase 6 — Divergence-note consolidation

- Rewrite the `DIVERGED:` block. Delete the "dispatch driver" paragraph. Retain the data-structure note only (`PanelRingNode *` vs `Vec` + `Option<PanelId>`), moved to `emView.rs` adjacent to `NoticeList`.
- Update `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8.1 item 12 to "**CLOSED by SP5** on <date> (commit <sha>)".

Gate:
- Closeout-doc item 12 marked closed.
- No stray `TODO(per-view-notice-dispatch)` in-tree.
- `cargo check` + clippy + nextest + golden all green (final checkpoint).

## 5. Acceptance criteria (end-of-SP5)

- `emPanel::View: Weak<RefCell<emView>>` exists (1:1 with C++ `emPanel::View &`).
- `emView::NoticeList`, `HandleNotice` exist; `PanelTree::HandleNotice` does not.
- `emGUIFramework::about_to_wait` has no `pixel_tallness` computation and no `HandleNotice` call.
- `grep -n "test_window_id\|TODO(per-view-notice-dispatch)\|PanelTree::HandleNotice\|emWindow::new_for_test"` returns zero matches in `crates/`.
- Multi-view unit test exists and passes.
- Single-window golden parity preserved (237/6 baseline).
- Closeout doc §8.1 item 12 marked CLOSED.

## 6. Risks and mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| P1 cascade exceeds Phase-6 precedent (~160 lines) | M | P1 is a gated phase. If line count >200, stop and escalate per CLAUDE.md's 50-line-deviation convention. Do not silently proceed. |
| Notice ordering changes because per-view drain drops inter-view interleaving | L (single-window today) | Multi-view unit test in P4 explicitly asserts per-view ordering matches C++ (in-view order; C++ has no cross-view notice ordering guarantee because each view drains independently). |
| `emPanel::View.upgrade()` returns `None` during panel teardown | M | Centralize unwrap with `.expect("emPanel::View set at construction, cleared only on drop")`. If a genuine teardown race surfaces, defer to `if let Some` with an explicit `DIVERGED:` note tying it to the specific teardown path. |
| SP4's engine-only `Update` routing interacts with per-view `HandleNotice` timing | M | P4 places `HandleNotice` *inside* `Update`, before the existing body. Engine routing unchanged; no new re-entrancy. SP4's `queue_or_apply` mechanism handles any scheduler mutations fired during notice handling. |
| P5 test migration surfaces tests that cannot construct a real `emWindow` (no wgpu/winit in test harness) | M | If any such test exists, introduce a minimal `#[cfg(any(test, feature = "test-support"))]` helper that builds an `emView` wrapped in `Rc<RefCell<>>` without a window — but NOT as a `test_window_id`-style identity hack; the helper produces a real `Rc` so the `Weak<RefCell<emView>>` on panels is genuinely non-dangling. |

## 7. Blast-radius estimate

| Item | Touched files | Touched lines |
|---|---|---|
| P1 `Rc<RefCell<emView>>` cascade | ~30 | ~150–200 |
| P2 `emPanel::View` field | ~5 | ~30 |
| P3 NoticeList migration | ~3 | ~60 |
| P4 Dispatch relocation | 2 | net decrease (~−40) |
| P5 Test-hack removal | ~5 | ~−40 |
| P6 Doc/comment consolidation | 2 | ~−20 |
| **Total** | **~40** | **~170–220 net** |

Roughly one Phase-6-emWindow-sized change, plus a cleanup tail.

## 8. Dependencies and ordering

- **Hard dependencies:** none. SP1, SP3, SP4 are all complete on main.
- **Soft dependencies:** SP7 (emContext threading) naturally pairs with SP5 if a multi-window roadmap activates — SP7 can thread contexts through the `Rc<RefCell<emView>>` owners once they exist. SP5 does not block on SP7.
- **Ordering vs. other residuals:** SP6 (W3 surface de-dup) is optional and independent. SP5 can land before or after.

## 9. Non-goals (restated for clarity)

- Not porting C++'s intrusive `PanelRingNode *` ring (forced data-structure divergence; `Vec` adaptation preserved).
- Not moving `run_panel_cycles` per-view.
- Not introducing a `ViewId` handle type or view registry — C++ uses direct references, Rust uses `Weak<RefCell<emView>>` to match.
- Not adding `emContext` wiring (SP7).
- Not changing notice *queueing* semantics (who calls `AddToNoticeList` and when).
- Not widening any `pub(crate)` to `pub` for test convenience.
