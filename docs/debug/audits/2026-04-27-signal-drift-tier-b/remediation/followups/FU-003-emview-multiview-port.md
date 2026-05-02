# FU-003 — bookmark navigation completion

> **Scope correction (2026-05-02).** This bucket was originally framed as "emView multi-view content/control split port — large standalone upstream port." Research showed that framing was based on incomplete reading: the multi-view infrastructure (`emSubViewPanel`, `emView::VisitByIdentity`, sub-view dispatch via `home_tree.with_behavior_as::<emSubViewPanel>(...)`) is already ported and is already used correctly in `emBookmarks.rs`'s click reaction. The actual remaining work is small. This file has been rewritten with the corrected scope. The original framing is preserved in `docs/scratch/2026-05-02-future-work-dump.md` for reference, alongside speculative items that were carved off.

**Pattern:** Wire-up — leverage existing emView/emSubViewPanel infrastructure for two sites currently using stubs/stale workarounds.
**Scope:** `emmain` (emBookmarks, emMainWindow).
**Row count:** 2 sites.
**Prereqs:** none.

## Pattern description

The Rust port has full `emView` + `emSubViewPanel` infrastructure: identity-based visit (`VisitByIdentity`), sub-view dispatch (`emSubViewPanel.visit_by_identity`), and the cross-reference path through `with_main_window` + `home_tree.with_behavior_as`. The click reaction at `emBookmarks.rs:748-780` uses this correctly. Two related sites do not yet use it:

- `emMainWindow.rs:337` — bookmark *hotkey* handler is a `log::info!` BLOCKED stub. The infrastructure exists; the wiring is missing.
- `emBookmarks.rs:723-733` — `DIVERGED:` comment claims "Rust `emView` has not ported the multi-view content/control split." That sentence is stale; the dispatch below it works correctly. The only true remaining divergence is *per-bookmark configurable target view* (multi-window setups), which is carved off as a future-work hook.

## Items

| ID | Site | Type | Notes |
|---|---|---|---|
| FU-003-A | `crates/emmain/src/emMainWindow.rs:330-344` | Wire-up | Replace the `log::info!` BLOCKED stub at line 337 with the same closure pattern used at `emBookmarks.rs:748-780`: push to `pending_actions`, resolve `main_panel_id` → `GetContentViewPanelId()` → `emSubViewPanel.visit_by_identity(...)`. Pull visit fields from `rec.entry` (`LocationIdentity`, `LocationRelX/Y/A`, `Name` as subject). |
| FU-003-B1 | `crates/emmain/src/emBookmarks.rs:723-733` | Comment correction | Rewrite the `DIVERGED:` block to accurately reflect current state: home-window content sub-view dispatch works; per-bookmark configurable target view does not. No behavior change. |

## Out of scope (carved off)

- **B2 — per-bookmark target view (multi-window).** Add a per-button `ContentView` reference field on `emBookmarkEntryUnion::Bookmark` and route the reaction body through it. Speculative; only matters in multi-window installs that configure bookmarks to target a specific window's content view. Tracked in `docs/scratch/2026-05-02-future-work-dump.md`.
- **C — `emFileManControlPanel::select_all` introspection.** Different axis (`ContentView.GetActivePanel()` introspection, not navigation). Tracked in scratch for separate bucketing if it surfaces a real bug.
- All other BLOCKED / `upstream-gap-forced` markers found in the multi-view sweep (animator registry, emCoreConfigPanel construction, bookmark mutation API, bookmark editing aux panels, PaintEllipseArc, Linux-only attribute, emLinearGroup orientation, CurrentViewPort delegation, emContext introspection, CPU TSC) — all unrelated axes; not part of this bucket.

## Acceptance

- `emMainWindow.rs:337` BLOCKED comment removed; bookmark hotkey navigation produces the same observable behavior as bookmark click navigation.
- `emBookmarks.rs:723-733` DIVERGED comment accurately describes current state.
- `cargo-nextest ntr` green; `cargo clippy -D warnings` green; `cargo xtask annotations` clean (the DIVERGED block changes wording but keeps a category cite).

## Notes

- Spec authoring is a single short pass — pattern is already proven in the click reaction at `emBookmarks.rs:748-780`. Plan can be terse.
- Item A is a copy-paste-and-adapt; Item B1 is a comment edit. ~30 LoC total.
- Watch for hotkey-context vs Cycle-context differences when adapting the click closure: hotkey path is in `handle_input(&mut self, event, input_state, app: &mut App)` which already has `&mut App`, so the closure may simplify (no `pending_actions` needed if we can dispatch synchronously). Verify during plan-writing.

## References

- C++ source:
  - `~/Projects/eaglemode-0.96.4/include/emMain/emMainPanel.h` — `GetControlView()` / `GetContentView()` returning sub-view emView refs.
  - `~/Projects/eaglemode-0.96.4/src/emMain/emBookmarks.cpp:1470,1523-1535` — emBookmarkButton ContentView field and click reaction.
- Rust precedent: `crates/emmain/src/emBookmarks.rs:748-780` — working multi-view dispatch via `pending_actions` + `with_main_window` + `home_tree.with_behavior_as::<emSubViewPanel>(...)`.
- Sweep results: `docs/scratch/2026-05-02-future-work-dump.md` (FU-003 carve-offs).

## Closure (2026-05-02)

Both call sites identified in the rescoped spec are resolved:

- `crates/emmain/src/emMainWindow.rs:330-344` — bookmark hotkey now
  dispatches synchronously through `emSubViewPanel::visit_by_identity`,
  matching the click-reaction path. BLOCKED comment removed.
- `crates/emmain/src/emBookmarks.rs:723-733` — DIVERGED comment
  rewritten to accurately describe the residual per-bookmark
  configurable-target-view divergence (multi-window only; tracked as
  future-work B2 in `docs/scratch/2026-05-02-future-work-dump.md`).

Out-of-scope items (B2, FileMan select_all ContentView introspection,
and other unrelated BLOCKED markers found during the FU-003 sweep)
remain in the scratch dump.

Acceptance gates run at closure: `cargo-nextest ntr` green, `cargo
clippy -- -D warnings` green, `cargo xtask annotations` clean.
