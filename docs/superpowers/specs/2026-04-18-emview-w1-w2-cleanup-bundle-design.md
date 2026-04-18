# emView W1+W2 Cleanup Bundle — Design

**Date:** 2026-04-18
**Parent:** `docs/superpowers/notes/2026-04-18-emview-subsystem-closeout.md` §8, items 1, 3, 4, 5, 6, 7, 8, 9.
**Scope:** One-shot bundled cleanup closing the surviving `PHASE-6-FOLLOWUP:` marker, eliminating silent drift in navigation methods, fixing `pump_visiting_va` public-API leakage, and clearing reviewer-flagged nits. Mostly mechanical; one non-mechanical task (caller audit).

This is sub-project 1 of 5 derived from the emView closeout residuals. Sibling sub-projects — item 2 audit, item 10 (`CoreConfig` ownership), items 14→11 (scheduler re-entrant borrow → Phase-8 test), item 12 (per-view notice dispatch, roadmap-gated), item 13 (surface-creation de-dup, optional) — each get their own spec.

---

## 1. Goals

- Close `PHASE-6-FOLLOWUP:` marker count from 1 to 0.
- Restore C++ fidelity on `emView::Input` (animator forwarding) and the `Visit*` navigation cluster (`NO_NAVIGATE` gate).
- Remove `pump_visiting_va` from the public `emcore` API surface while keeping cross-crate tests working.
- Clear the §5.2 minor-cleanups list and the §5.1 reviewer items that are one-liner fixes.
- Keep tests at 2425/2425 nextest, 237/6 golden, smoke exit 143/124.

## 2. Non-goals

- Scheduler re-entrant borrow (§8 item 14) — blocks Phase-8 test, separate spec.
- `CoreConfig` ownership on `emView` (§8 item 10) — separate spec.
- Per-view notice dispatch (§8 item 12) — roadmap-gated, separate spec.
- W3 surface-creation de-dup (§8 item 13) — optional, separate spec.
- `InvalidateHighlight` call-site audit (§8 item 2) — scoping question, separate spec.

## 3. Tasks

Ordered by suggested landing sequence; each task is independently committable.

### 3.1 `emView::Input` animator-forward port

**Source of truth:** C++ `emView.cpp:1004` — `ActiveAnimator->Input(event, state);` at the top of `emView::Input`, before the body processes the event.

**Change.** At the top of `emView::Input` (`crates/emcore/src/emView.rs:~3524`), before the existing body, forward the event to the active animator:

```rust
if let Some(anim) = self.active_animator.as_ref() {
    anim.borrow_mut().Input(event, state);
}
```

(Exact field and borrow shape to be confirmed at implementation time — the active-animator handle already exists from W4.)

**Marker cleanup.** Remove the `PHASE-6-FOLLOWUP:` comment at `emView.rs:~3626` (the VIF-chain note); the forward covers what that comment anticipated.

**Test.** Add `input_forwards_to_active_animator` as a behavioural test: install a recording animator, dispatch an input event to the view, assert the animator saw the event before any view-body side effect.

### 3.2 Navigation `NO_NAVIGATE` gate removal + caller audit

**Source of truth:** C++ `emView.cpp:564-762` — the seven navigation methods (`VisitNext/Prev/First/Last/In/Out/Neighbour`) contain no `NO_NAVIGATE`/`NO_USER_NAVIGATION` check. Gating happens at callers (input/keybinding sites).

**Change.**

1. Delete the leading `if self.flags.intersects(NO_NAVIGATE | NO_USER_NAVIGATION) { return; }` from each of the seven methods.
2. Audit every caller of each of the seven methods. Expected caller set: `emView::Input` key/mouse nav dispatch; any keybinding synthesizer.
3. At every user-nav caller, add/confirm the gate.
4. At every programmatic caller (tests, internal animator-driven calls), confirm no gate is needed.

**Stop condition.** If the audit reveals a programmatic caller that silently depends on the gate to avoid a bug, escalate — do not paper over with a broader gate. Such a caller is a Rust-only path (no C++ analogue) and must either be deleted or explicitly marked `DIVERGED:`.

**Test.** If the audit surfaces caller-site gaps, add a test at the first gap-site proving user-nav is gated there. Otherwise no new test — the existing suite + the behavioural-equivalence constraint covers it.

### 3.3 `pump_visiting_va` visibility fix (option a)

**Source of truth:** Not a C++-fidelity item; a Rust test-API hygiene fix. Currently `pub fn pump_visiting_va` on `emView` (from W4 commit `6642ec5`) leaks to every `emcore` consumer.

**Change.**

1. Add a `test-support` feature to `crates/emcore/Cargo.toml`:
   ```toml
   [features]
   test-support = []
   ```
2. Gate `pump_visiting_va`:
   ```rust
   #[cfg(any(test, feature = "test-support"))]
   pub fn pump_visiting_va(&mut self) { /* ... */ }
   ```
3. In every cross-crate `Cargo.toml` that currently calls `pump_visiting_va`, add `emcore = { ..., features = ["test-support"] }` to the dev-dependencies block (not the regular dependencies block).

**Test.** No new test. `cargo-nextest ntr` must pass — which proves the dev-dep plumbing is correct.

### 3.4 `VisitByIdentity` co-location

**Source of truth:** C++ `emView.cpp:492-523` colocates `Visit` and `VisitByIdentity`. File-and-Name Correspondence expects adjacency.

**Change.** Move the 7-arg `VisitByIdentity` method body from `emView.rs:~3072` to immediately after `Visit` at `~857`. Pure cut/paste; no logic change. Update any `use`/imports that break.

**Test.** None. `cargo check` is sufficient.

### 3.5 `DIVERGED:` suffix rename

**Source of truth:** File-and-Name Correspondence — suffixes should be semantic descriptors, not antonym heuristics. The W4 `Short` suffix is the latter.

**Change.** Rename the W4 arity-overload family:

| Current | New | Rationale |
|---|---|---|
| `VisitFullsizedByIdentity` | unchanged | already semantic |
| `VisitPanel` | unchanged | already semantic |
| `VisitByIdentityShort` | `VisitByIdentityBare` | "Bare" = without coords, parallel to other Bare/Full pairs |
| `SetGoalWithCoords` | `SetGoalCoords` | drop redundant "With" |

Update the `DIVERGED:` comments at each definition to reference both the C++ name and the new Rust name. Update all call sites via rename-refactor.

**Test.** None. `cargo check` + existing suite.

### 3.6 `GeometrySignal` double-fire comment

**Source of truth:** C++ `emView.cpp:1678` (`SwapViewPorts` fires) + `emView.cpp:1995` (explicit second fire).

**Change.** Add a one-line comment at `emView.rs:~1733` immediately above the explicit second `GeometrySignal` fire:

```rust
// DIVERGED: none — mirrors C++ emView.cpp:1678 + 1995; SwapViewPorts(true) already fired once,
// explicit second fire matches the C++ pair.
```

(Exact wording TBD at implementation time; the point is the comment ties both fires to their C++ counterparts.)

**Test.** None.

### 3.7 Re-entrancy doc comments on `PaintView` / `InvalidatePainting`

**Change.** Add a paragraph-length doc comment on each of `PaintView` and `InvalidatePainting` in `crates/emcore/src/emViewPort.rs` explaining:

- Both upgrade `self.window: Weak<RefCell<emWindow>>` via `upgrade()` and then `borrow_mut()`.
- Safe today because no existing call site holds an outstanding `&mut emWindow` when these methods are invoked.
- Future callers from inside `render()`, `dispatch_input()`, or `handle_touch()` — all of which run under an outstanding `&mut emWindow` — would panic with a re-entrant `RefCell` borrow.
- If such a caller is needed, redesign the back-reference (e.g., pass a token rather than upgrade-and-borrow).

**Test.** None. This is documentation.

### 3.8 Minor cleanups (§5.2 items)

Four one-liners (the fifth §5.2 item, the suffix rename, is task 3.5):

1. **`emSubViewPanel.rs:48`.** Add one-line comment tying the literal `1.0` to `CurrentPixelTallness`'s initial value.
2. **`emGUIFramework.rs:~393`.** Collapse `let mut win = rc.borrow_mut(); let win = &mut *win;` to `let win = &mut *rc.borrow_mut();`.
3. **`emGUIFramework.rs::dispatch_forward_events`.** The current doc-comment describes caller-side usage. Either move it to the single call site or delete it; the function-site comment should describe the function.
4. **`tests/unit/popup_window.rs`.** In `popup_window_creation_path_is_gated_on_display`, remove the dead `DISPLAY`/`WAYLAND_DISPLAY` gate (both branches run unconditionally today). Keep the reachability assertion; simplify the test to match what it actually exercises.

**Test.** None beyond the existing suite.

## 4. Landing order and commit structure

Suggested one commit per task, in the order above — eight commits. Tasks are independent; any subset can land first. Task 3.2 is the only one with non-zero risk; consider landing it last so bisection is clean if a regression surfaces.

If CI budget is tight, tasks 3.4, 3.5, 3.6, 3.7, 3.8 can fold into a single "cleanups" commit since they touch disjoint files and carry no behavioural risk.

## 5. Verification

After the bundle lands:

- `cargo clippy -- -D warnings` — clean.
- `cargo-nextest ntr` — 2425/2425 passing, 9 skipped, 0 failed (same as closeout baseline).
- `cargo test --test golden -- --test-threads=1` — 237 passed / 6 failed (same six pre-existing failures: `composition_tktest_{1x,2x}`, `notice_window_resize`, `testpanel_{expanded,root}`, `widget_file_selection_box`).
- `rg 'PHASE-6-FOLLOWUP:' crates/` — zero matches (was 1).
- `rg 'PHASE-W4-FOLLOWUP:' crates/` — 3 matches (unchanged; out of scope for this bundle).
- `timeout 20 cargo run --release --bin eaglemode` — exits 143 or 124.

## 6. Risks

- **Task 3.2 caller-audit surprise.** A programmatic caller may quietly depend on the gate. Mitigation: stop-condition in §3.2 escalates rather than papering over. Detection: if the audit's caller-set diverges from the expected set (input dispatch + keybindings), stop and re-scope.
- **Task 3.3 cross-crate dev-dep plumbing.** Forgetting `features = ["test-support"]` on one consumer's dev-dep block means that crate's tests fail to compile. Detection: `cargo-nextest ntr` covers every crate.
- **Task 3.5 rename churn.** If external consumers (plugins, downstream crates) reference the old names, renaming breaks them. Current external-consumer count is zero; landing now is cheap. Landing later is expensive.

## 7. Out of scope — sibling sub-projects

| Item | Sub-project |
|---|---|
| §8 item 2 (`InvalidateHighlight` call-site audit) | Separate, small scoping spec. |
| §8 item 10 (`CoreConfig` ownership on `emView`) | Separate architectural spec; closes 3 `PHASE-W4-FOLLOWUP:` markers. |
| §8 items 14→11 (scheduler re-entrant borrow → Phase-8 test) | Combined spec; 14 blocks 11. |
| §8 item 12 (per-view notice dispatch) | Roadmap-gated on multi-window; defer. |
| §8 item 13 (W3 surface-creation de-dup) | Optional; skip unless explicitly wanted. |

End of design.
