# emView Viewing Update: RawVisitAbs Rewrite

**Date:** 2026-04-17
**Status:** Approved for implementation (C++-parity auto-answers)

## Problem

Rust `emView::Update` recomputes viewing state every frame via a clear-then-rebuild
pattern (`clear_viewing_flags` + `compute_viewed_recursive`). This diverges from
C++ `emView::RawVisitAbs` (emView.cpp:1543–1808), which tracks
`SupremeViewedPanel` across frames and surgically updates only when the SVP or
viewed rect actually changes.

The divergence required a `prev_viewed` snapshot band-aid (commit b693d41) to
avoid firing `VIEW_CHANGED` notices every frame. Even with the band-aid:

- Runtime SIGSEGV within 3–6 s of startup (stack: `emSubViewPanel::Paint+356`,
  just after a `HandleNotice` call — suggests notice delivered at a
  semantically wrong moment corrupts sub-view lifecycle).
- Every-frame traversal of the entire panel tree is wasteful.
- Rust structure no longer overlays C++ 1:1 in the Update path.

## Goal

Replace the clear-then-rebuild recomputation with a transition-detecting port
of C++ `RawVisitAbs` + `emPanel::UpdateChildrenViewing` (emPanel.cpp:1454–1518).
Fire viewing-related notices only on actual transitions.

## Non-Goals

- Fixing the 8 pre-existing golden-test failures (`composition_*`,
  `notice_add_and_activate`, `notice_children_changed`, `notice_window_resize`,
  `testpanel_*`, `widget_file_selection_box`).
- Fixing the 6 pre-existing emcore lib test failures (emBorder/emPainter
  scaling).
- Separate investigation of any SIGSEGV that persists after this rewrite.

## Architecture

Two commits, each independently green.

### Commit 1 — `PanelTree::update_children_viewing`

Port C++ `emPanel::UpdateChildrenViewing` (emPanel.cpp:1454–1518) as
`pub(crate) fn update_children_viewing(&mut self, id: PanelId)` on `PanelTree`.

**Two branches, identical to C++:**

1. **Parent not Viewed**: iterate children; any child still `in_viewed_path`
   gets `viewed = false`, `in_viewed_path = false`, receives
   `VIEW_CHANGED | UPDATE_PRIORITY_CHANGED | MEMORY_LIMIT_CHANGED`, and recurses.
2. **Parent Viewed**: for each child compute absolute `viewed_x/y/width/height`
   from parent's viewed rect + child's `layout_rect`, intersect with parent's
   `clip_x1/y1/x2/y2` to produce the child's clip rect, set `viewed` based on
   non-empty clip, fire notice on transition, recurse.

No callers in this commit. Callable but unused; verified by cargo build.

### Commit 2 — Port `RawVisitAbs` into `emView::Update`

**State:** `self.svp` is renamed to `self.supreme_viewed_panel` and already
persists across `Update` calls as a field. The current code zeros it at the
start of Update (emView.rs:1308) — the rewrite snapshots it as `old_svp`
before any mutation, then assigns the new SVP at the end.

**Critical ordering constraint** (not obvious from C++ alone): the existing
Rust `Update` uses two mutation passes — `compute_viewed_recursive` fills the
whole tree with `viewed_x/y/width/height`, then the SVP-finder at
emView.rs:1309–1320 reads those fields to pick the SVP. The rewrite must
find the new SVP and its viewed rect *without* mutating the whole tree first.
Approach:

1. Keep the existing chain-walk math (emView.rs:1208–1298) that computes
   `root_abs` and `visited_vw/vh` — this is read-only and produces the
   equivalent of C++ `RawVisitAbs`'s (vx, vy, vw) arguments at the `visited`
   panel level.
2. Extend that read-only walk to also compute each ancestor's absolute viewed
   rect into a local `Vec`. This is cheap (ancestor chain is depth-bounded).
3. Pick the new SVP by walking the ancestor chain from `visited` upward,
   selecting the deepest whose computed area ≤ `MAX_SVP_SIZE` (existing rule
   at emView.rs:1309–1317). Capture its computed `(vx, vy, vw, vh)`.
4. Change-detect: `force_viewing_update || old_svp != new_svp ||
   |old_vx - new_vx| > 0.001 || …` (matches C++ emView.cpp:1727–1752).
5. **No change** → early return; no traversal, no notices.
6. **Change** (port emView.cpp:1753–1807 line-for-line):
   - **Old SVP clear** (if `old_svp` present and still in tree):
     `viewed = false`, `in_viewed_path = false`, queue
     `VIEW_CHANGED | UPDATE_PRIORITY_CHANGED | MEMORY_LIMIT_CHANGED`,
     call `PanelTree::UpdateChildrenViewing(old_svp)`.
     Walk parent chain of `old_svp` clearing `in_viewed_path` with the same
     notice on each level, stopping when a parent already has
     `in_viewed_path == false` (equivalent to C++'s walk-until-already-cleared).
   - **New SVP set**: write `viewed = true`, `in_viewed_path = true`,
     `viewed_x/y/width/height`, `clip_x1/y1/x2/y2` (clipped to viewport),
     queue the same notice, call `UpdateChildrenViewing(new_svp)`.
     Walk parent chain of `new_svp` setting `in_viewed_path = true` with
     notice, stopping when a parent already has `in_viewed_path == true`.
7. `self.supreme_viewed_panel = Some(new_svp)`.

**Preserve:** `zoomed_out_before_sg`, `in_active_path` active-path propagation
(emView.rs:1347–1363), `svp_update_count`, `drain_navigation_requests`.

**Remove:**
- `clear_viewing_flags` at emView.rs:1167 call site, and the method in
  emPanelTree.rs:2522. Grep confirmed no other callers.
- `compute_viewed_recursive` (emView.rs:1377), only callers are Update and
  itself.
- `PanelData.prev_viewed` field (emPanelTree.rs:201), its init
  (emPanelTree.rs:253), its write (emPanelTree.rs:2524), and its read at
  emView.rs:1387.
- The standalone `in_viewed_path` ancestor-propagation block at
  emView.rs:1323–1345. Its function is subsumed by the explicit
  walk-parents step inside the RawVisitAbs port.

## Naming — Match C++

No semantic divergences. C++ names are preserved modulo the established Rust
snake_case convention for struct fields (e.g. C++ `Viewed` → Rust `viewed`,
C++ `ClipX1` → Rust `clip_x1`, already applied across `PanelData`).

- Rename existing field `emView::svp: Option<PanelId>` →
  `supreme_viewed_panel: Option<PanelId>` (snake_case of C++
  `SupremeViewedPanel`). All internal references update; the public getter
  `GetSupremeViewedPanel` is unchanged.
- New method `PanelTree::UpdateChildrenViewing` — camelCase preserved because
  `PanelTree` already hosts C++ `emPanel` container methods with C++ names
  (`HandleNotice`, `BeFirst`, `BeLast`). No `DIVERGED:` comment.
- Every method/field touched keeps its C++ name (snake_case where already
  established).

## Verification

After each commit:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo build --release --bin eaglemode`
- `cargo test --release --test golden -- --test-threads=1`
  - Baseline: **235 passed, 8 failed**. Must not regress. Same 8 failures
    acceptable.
- `cargo test -p emcore --lib --release`
  - Baseline: **813 passed, 6 failed** (pre-existing).
- `cargo test --release --test pipeline --test behavioral`
  - Baseline: **378 + 312 passed, 0 failed**.
- Runtime smoke test:
  ```sh
  ulimit -c unlimited; rm -f /tmp/core.*
  env DISPLAY=:0 WAYLAND_DISPLAY=wayland-1 EM_DIR=/home/a0/git/eaglemode-rs \
      target/release/eaglemode 2>/tmp/rewrite.log &
  sleep 15; pgrep -af eaglemode; ls /tmp/core.* 2>&1
  WAYLAND_DISPLAY=wayland-1 grim -l 0 /tmp/shot.png
  ```
  Target: eaglemode alive after 15 s, no core dump, screenshot shows the
  `/home/a0` directory listing (bookmark `VisitAtProgramStart=yes`).

## Risks

1. **SVP `rel_a` convention mismatch.** Rust uses `rel_a = 1/ra`. Port formulas
   from C++ carefully at each site; unit-test individually if ambiguous.
2. **Notice ordering change.** Fewer/differently-timed notices may expose bugs
   in panel behaviors that incidentally depended on the every-frame firing.
   Mitigation: golden notice tests catch most; runtime smoke catches the rest.
3. **SIGSEGV may persist.** If so, it's a separate lifecycle bug in a sub-view
   panel's `notice()` handler, not in this rewrite. Flag as follow-up.
4. **`clear_viewing_flags` other callers.** Grep confirmed only one caller
   (emView.rs:1167). Safe to remove.
5. **SVP area computation before tree mutation.** Risk: the ancestor-rect
   `Vec` built in the read-only walk must agree with what
   `UpdateChildrenViewing` subsequently writes. Mitigation: reuse the same
   layout_rect scaling formula in both places; verify via the smoke test
   that viewed rects pixel-match the pre-rewrite output on a static frame.
6. **`queue_notice` invariant.** `queue_notice` (emPanelTree.rs:581) expects
   the panel to be in the tree. Every call site in the RawVisitAbs port
   must verify `tree.contains(id)` if the panel could have been removed
   mid-update — for this rewrite, SVP is read fresh so it's live.

## Pre-flight

- Revert or keep uncommitted instrumentation in `emPanelTree.rs` and
  `emViewAnimator.rs` (AE_NOTICE_TRACE / AE_ANIM_TRACE). Reverting keeps
  commits clean.
- Read `~/.claude/projects/-home-a0-git-eaglemode-rs/memory/MEMORY.md` and the
  four feedback files cited in the compaction prompt.
