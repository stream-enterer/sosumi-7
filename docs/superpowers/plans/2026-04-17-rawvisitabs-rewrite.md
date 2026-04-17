# RawVisitAbs Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Rust `emView::Update`'s clear-then-rebuild viewing recomputation with a transition-detecting port of C++ `emView::RawVisitAbs` + `emPanel::UpdateChildrenViewing`, firing viewing notices only on actual SVP transitions.

**Architecture:** Two atomic commits. (1) Port `UpdateChildrenViewing` as a `PanelTree` method, no callers. (2) Rewrite `emView::Update`'s viewing phase: compute new SVP from a read-only ancestor-rect walk, compare against persisted `supreme_viewed_panel`, on change run surgical clear-old/set-new with `UpdateChildrenViewing` recursion + parent-chain walks. Remove `clear_viewing_flags`, `compute_viewed_recursive`, `PanelData.prev_viewed`, and the standalone `in_viewed_path` propagation loop.

**Tech Stack:** Rust 2021, slotmap arena (`PanelId`), existing `queue_notice` mechanism, `NoticeFlags` bitflags.

**Spec:** `docs/superpowers/specs/2026-04-17-rawvisitabs-rewrite-design.md`

**C++ reference:**
- `~/git/eaglemode-0.96.4/src/emCore/emPanel.cpp:1454-1518` (`UpdateChildrenViewing`)
- `~/git/eaglemode-0.96.4/src/emCore/emView.cpp:1543-1808` (`RawVisitAbs`, esp. 1727-1807 change-block)

**Field-name mapping (C++ → Rust PanelData):**

| C++ | Rust |
|---|---|
| `Viewed` | `viewed` |
| `InViewedPath` | `in_viewed_path` |
| `ViewedX/Y/Width/Height` | `viewed_x/viewed_y/viewed_width/viewed_height` |
| `ClipX1/Y1/X2/Y2` | `clip_x/clip_y/clip_w/clip_h` (xywh, **not** x1y1x2y2) |
| `LayoutX/Y/Width/Height` | `layout_rect.x/y/w/h` |
| `View.CurrentPixelTallness` | `tree.current_pixel_tallness` |
| `FirstChild` / `p->Next` | `tree.first_child(id)` / `tree.next_sibling(id)` |
| `AddPendingNotice(f)` | `tree.queue_notice(id, f)` |
| `SupremeViewedPanel` | `emView::supreme_viewed_panel` (renamed from `svp`) |

---

## Commit 1 — Port `UpdateChildrenViewing` onto `PanelTree`

### Task 1: Add `UpdateChildrenViewing` method

**Files:**
- Modify: `crates/emcore/src/emPanelTree.rs` (insert method in `impl PanelTree`)

- [ ] **Step 1: Add the method**

Insert this method in `impl PanelTree` in `crates/emcore/src/emPanelTree.rs`, next to existing viewing-related methods (e.g. right after `clear_viewing_flags` at ~line 2538, or near `HandleNotice` at line 1591 — pick wherever the other `Handle*`/view methods live):

```rust
    /// Port of C++ `emPanel::UpdateChildrenViewing` (emPanel.cpp:1454-1518).
    ///
    /// Propagates viewing state from a panel to its immediate children,
    /// recursing into children whose state transitions. Fires
    /// `VIEW_CHANGED | UPDATE_PRIORITY_CHANGED | MEMORY_LIMIT_CHANGED` on
    /// every transition.
    ///
    /// Precondition: when called, `self.panels[id].in_viewed_path` and
    /// `viewed` already reflect `id`'s own new state. The method then
    /// updates each child based on whether `id` is Viewed.
    pub(crate) fn UpdateChildrenViewing(&mut self, id: PanelId) {
        let (id_viewed, id_in_path, pid_vx, pid_vy, pid_vw, pid_cx1, pid_cy1, pid_cx2, pid_cy2) = {
            let p = match self.panels.get(id) {
                Some(p) => p,
                None => return,
            };
            (
                p.viewed,
                p.in_viewed_path,
                p.viewed_x,
                p.viewed_y,
                p.viewed_width,
                p.clip_x,
                p.clip_y,
                p.clip_x + p.clip_w,
                p.clip_y + p.clip_h,
            )
        };

        if !id_viewed {
            // C++ fatal-errors if !Viewed && InViewedPath.
            debug_assert!(
                !id_in_path,
                "UpdateChildrenViewing called with !viewed && in_viewed_path (C++ emFatalError)"
            );
            // Parent not Viewed: un-view any still-InViewedPath descendants.
            let mut child_opt = self.first_child(id);
            while let Some(c) = child_opt {
                let next = self.next_sibling(c);
                let needs_recurse = match self.panels.get_mut(c) {
                    Some(cp) if cp.in_viewed_path => {
                        cp.viewed = false;
                        cp.in_viewed_path = false;
                        true
                    }
                    _ => false,
                };
                if needs_recurse {
                    self.queue_notice(
                        c,
                        NoticeFlags::VIEW_CHANGED
                            | NoticeFlags::UPDATE_PRIORITY_CHANGED
                            | NoticeFlags::MEMORY_LIMIT_CHANGED,
                    );
                    if self.first_child(c).is_some() {
                        self.UpdateChildrenViewing(c);
                    }
                }
                child_opt = next;
            }
            return;
        }

        // Parent Viewed: compute each child's viewed rect + clip, set Viewed.
        let pt = self.current_pixel_tallness;
        let mut child_opt = self.first_child(id);
        while let Some(c) = child_opt {
            let next = self.next_sibling(c);

            let (cx, cy, cw, ch, clip_x, clip_y, clip_w, clip_h, became_viewed, was_in_path) = {
                let cp = match self.panels.get_mut(c) {
                    Some(cp) => cp,
                    None => {
                        child_opt = next;
                        continue;
                    }
                };
                // C++ lines 1478-1485:
                // x1 = ViewedX + LayoutX*ViewedWidth
                // x2 = LayoutWidth*ViewedWidth
                // y1 = ViewedY + LayoutY*(ViewedWidth/CurrentPixelTallness)
                // y2 = LayoutHeight*(ViewedWidth/CurrentPixelTallness)
                let vx = pid_vx + cp.layout_rect.x * pid_vw;
                let vw = cp.layout_rect.w * pid_vw;
                let vy_scale = pid_vw / pt;
                let vy = pid_vy + cp.layout_rect.y * vy_scale;
                let vh = cp.layout_rect.h * vy_scale;
                cp.viewed_x = vx;
                cp.viewed_y = vy;
                cp.viewed_width = vw;
                cp.viewed_height = vh;

                // C++ lines 1486-1495: clip child rect against parent's ClipX1/Y1/X2/Y2.
                let mut x1 = vx;
                let mut y1 = vy;
                let mut x2 = vx + vw;
                let mut y2 = vy + vh;
                if x1 < pid_cx1 {
                    x1 = pid_cx1;
                }
                if x2 > pid_cx2 {
                    x2 = pid_cx2;
                }
                if y1 < pid_cy1 {
                    y1 = pid_cy1;
                }
                if y2 > pid_cy2 {
                    y2 = pid_cy2;
                }
                cp.clip_x = x1;
                cp.clip_y = y1;
                cp.clip_w = (x2 - x1).max(0.0);
                cp.clip_h = (y2 - y1).max(0.0);

                let non_empty = x1 < x2 && y1 < y2;
                let was_in_path = cp.in_viewed_path;
                if non_empty {
                    cp.in_viewed_path = true;
                    cp.viewed = true;
                } else if was_in_path {
                    cp.in_viewed_path = false;
                    cp.viewed = false;
                }
                (vx, vy, vw, vh, x1, y1, x2 - x1, y2 - y1, non_empty, was_in_path)
            };
            let _ = (cx, cy, cw, ch, clip_x, clip_y, clip_w, clip_h);

            if became_viewed {
                self.queue_notice(
                    c,
                    NoticeFlags::VIEW_CHANGED
                        | NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | NoticeFlags::MEMORY_LIMIT_CHANGED,
                );
                if self.first_child(c).is_some() {
                    self.UpdateChildrenViewing(c);
                }
            } else if was_in_path {
                self.queue_notice(
                    c,
                    NoticeFlags::VIEW_CHANGED
                        | NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | NoticeFlags::MEMORY_LIMIT_CHANGED,
                );
                if self.first_child(c).is_some() {
                    self.UpdateChildrenViewing(c);
                }
            }

            child_opt = next;
        }
    }
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo check -p emcore`
Expected: success. Unused-method warning OK (no callers yet).

- [ ] **Step 3: Commit**

```bash
git add crates/emcore/src/emPanelTree.rs
git commit -m "$(cat <<'EOF'
feat(emPanelTree): port emPanel::UpdateChildrenViewing from C++

Adds PanelTree::UpdateChildrenViewing, a structure-identical port of
emPanel.cpp:1454-1518. Two branches: parent-not-Viewed clears descendants'
in_viewed_path; parent-Viewed computes each child's viewed rect + clipped
clip rect, sets Viewed on non-empty clip, fires
VIEW_CHANGED|UPDATE_PRIORITY_CHANGED|MEMORY_LIMIT_CHANGED on transitions,
recurses. No callers yet; commit 2 wires it into emView::Update.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 2: Verify commit 1 baselines

- [ ] **Step 1: Run formatter and clippy**

```
cargo fmt
cargo clippy --all-targets -- -D warnings
```
Expected: clean. `UpdateChildrenViewing` name triggers `non_snake_case` — if clippy flags it, add `#[allow(non_snake_case)]` on the method (CLAUDE.md permits this narrowly for C++ name preservation on `em`-prefixed identifiers; `UpdateChildrenViewing` is an `emPanel` method). If the lint doesn't fire for methods in this crate, skip.

- [ ] **Step 2: Run golden tests**

Run: `cargo test --release --test golden -- --test-threads=1 2>&1 | tail -20`
Expected: 235 passed, 8 failed (same 8 as baseline: composition_*, notice_add_and_activate, notice_children_changed, notice_window_resize, testpanel_*, widget_file_selection_box).

- [ ] **Step 3: Run emcore lib tests**

Run: `cargo test -p emcore --lib --release 2>&1 | tail -5`
Expected: 813 passed, 6 failed (pre-existing emBorder/emPainter scaling).

- [ ] **Step 4: Run pipeline + behavioral tests**

Run: `cargo test --release --test pipeline --test behavioral 2>&1 | tail -10`
Expected: 378 + 312 passed, 0 failed.

If any baseline regresses, fix before proceeding. The method is unused — regressions would mean a surface-level breakage from the insertion itself (syntax, import).

---

## Commit 2 — Port `RawVisitAbs` logic into `emView::Update`

### Task 3: Rename `svp` → `supreme_viewed_panel`

**Files:**
- Modify: `crates/emcore/src/emView.rs` (field definition + all references)
- Modify: `crates/emcore/src/emViewAnimator.rs:2484` (`view.supreme_panel()` stays — not this field; verify)

- [ ] **Step 1: Rename the field**

In `crates/emcore/src/emView.rs`:

Change line 189:
```rust
    svp: Option<PanelId>,
```
to:
```rust
    supreme_viewed_panel: Option<PanelId>,
```

Change line 273 (in `new()`):
```rust
            svp: None,
```
to:
```rust
            supreme_viewed_panel: None,
```

- [ ] **Step 2: Rename all internal references**

Run from repo root:
```bash
grep -n '\bself\.svp\b' crates/emcore/src/emView.rs
```
Expected matches: lines 333, 530, 534, 539, 1081, 1308, 1313, 1318, 1699, 1700, 1704, 1705, 1321, 1366, 2356, 2407.

Replace every `self.svp` with `self.supreme_viewed_panel` in that file. (Confirm no other crate reads `emView.svp` directly — the field was private, so there shouldn't be any.)

Verify:
```bash
grep -rn '\.svp\b' crates/ | grep -v emViewAnimator
```
Expected: no matches. (`emViewAnimator` defines its own local `svp` variables — leave those alone.)

- [ ] **Step 3: Build**

Run: `cargo check -p emcore`
Expected: success.

- [ ] **Step 4: Commit the rename separately**

```bash
git add crates/emcore/src/emView.rs
git commit -m "$(cat <<'EOF'
refactor(emView): rename svp field to supreme_viewed_panel

Matches C++ emView::SupremeViewedPanel (snake_case per Rust convention).
Pure rename, no behavior change. Prepares for RawVisitAbs port.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

### Task 4: Rewrite `emView::Update` viewing phase

**Files:**
- Modify: `crates/emcore/src/emView.rs:1163-1375` (`Update` function body)
- Modify: `crates/emcore/src/emView.rs:1377-1450` (delete `compute_viewed_recursive`)
- Modify: `crates/emcore/src/emPanelTree.rs:2516-2538` (delete `clear_viewing_flags`)
- Modify: `crates/emcore/src/emPanelTree.rs:191-201` (delete `prev_viewed` field + docs)
- Modify: `crates/emcore/src/emPanelTree.rs:253` (delete `prev_viewed: false,` from init)

- [ ] **Step 1: Replace the `Update` viewing phase**

Open `crates/emcore/src/emView.rs`. Locate `pub fn Update(&mut self, tree: &mut PanelTree)` (line 1163).

Replace the body from **line 1167** (`tree.clear_viewing_flags();`) through **line 1345** (end of `in_viewed_path` ancestor-propagation block) with:

```rust
        let root = match tree.GetRootPanel() {
            Some(r) => r,
            None => return,
        };

        // C++ ZoomedOutBeforeSG: on the first update after construction,
        // compute the zoom-out relA so the root panel fits in the viewport.
        if self.zoomed_out_before_sg {
            self.zoomed_out_before_sg = false;
            let rel_a = self.zoom_out_rel_a(tree);
            if let Some(state) = self.visit_stack.last_mut() {
                state.rel_a = rel_a;
            }
        }

        let vw = self.viewport_width.max(1.0);
        let vh = self.viewport_height.max(1.0);

        let visit = self.current_visit().clone();
        let visited = visit.panel;
        let visited = if self.flags.contains(ViewFlags::NO_ZOOM) {
            root
        } else {
            visited
        };

        // Compute visited panel's natural-coordinate chain from root.
        let chain = tree.ancestors(visited);
        let mut chain_rev: Vec<PanelId> = chain;
        chain_rev.reverse();

        let root_lr = tree.GetRec(root).map(|p| p.layout_rect).unwrap_or_default();
        let root_norm_h = if root_lr.w > MIN_DIMENSION {
            (root_lr.h / root_lr.w).max(MIN_DIMENSION)
        } else {
            1.0
        };
        let mut norm_rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        norm_rects.push((0.0, 0.0, 1.0, root_norm_h));
        for i in 1..chain_rev.len() {
            let id = chain_rev[i];
            let lr = tree.GetRec(id).map(|p| p.layout_rect).unwrap_or_default();
            let (px, py, pw, _ph) = norm_rects[i - 1];
            norm_rects.push((px + lr.x * pw, py + lr.y * pw, lr.w * pw, lr.h * pw));
        }

        let (vnx, vny, vnw, vnh) = *norm_rects.last().unwrap_or(&(0.0, 0.0, 1.0, 1.0));

        let vnw_safe = vnw.max(MIN_DIMENSION);
        let vnh_safe = vnh.max(MIN_DIMENSION);
        let panel_aspect = vnw_safe / vnh_safe;

        let visited_vw = (visit.rel_a * vw * vh * panel_aspect).sqrt();
        let visited_vh = (visit.rel_a * vw * vh / panel_aspect).sqrt();
        self.visited_vw = visited_vw.max(1.0);
        self.visited_vh = visited_vh.max(1.0);

        let root_vw = visited_vw / vnw_safe;
        let root_vh_center = visited_vh / vnh_safe;

        let vcx = vw * 0.5 - visit.rel_x * visited_vw;
        let vcy = vh * 0.5 - visit.rel_y * visited_vh;

        let root_vx = vcx - (vnx + vnw_safe * 0.5) * root_vw;
        let root_vy = vcy - (vny + vnh_safe * 0.5) * root_vh_center;

        // Compute every chain panel's absolute (vx, vy, vw, vh) in read-only
        // mode — we don't mutate the tree here because SVP selection needs
        // these values before we decide what to clear/set.
        // chain_rects[i] corresponds to chain_rev[i].
        let pt = tree.current_pixel_tallness;
        let mut chain_rects: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(chain_rev.len());
        let root_actual_h = root_vw * root_norm_h;
        chain_rects.push((root_vx, root_vy, root_vw, root_actual_h));
        for i in 1..chain_rev.len() {
            let (px_abs, py_abs, pw_abs, _ph_abs) = chain_rects[i - 1];
            let id = chain_rev[i];
            let lr = tree.GetRec(id).map(|p| p.layout_rect).unwrap_or_default();
            // Match C++ UpdateChildrenViewing scaling:
            //   vx = parent_vx + LayoutX * parent_vw
            //   vw = LayoutWidth * parent_vw
            //   vy = parent_vy + LayoutY * (parent_vw / pixel_tallness)
            //   vh = LayoutHeight * (parent_vw / pixel_tallness)
            let cvx = px_abs + lr.x * pw_abs;
            let cvw = lr.w * pw_abs;
            let scale_y = pw_abs / pt;
            let cvy = py_abs + lr.y * scale_y;
            let cvh = lr.h * scale_y;
            chain_rects.push((cvx, cvy, cvw, cvh));
        }

        // Pick new SVP: deepest ancestor of `visited` whose computed area
        // ≤ MAX_SVP_SIZE. chain_rev is root..visited; walk from end toward start.
        let mut new_svp_idx = 0usize;
        for i in (0..chain_rev.len()).rev() {
            let (_, _, cvw, cvh) = chain_rects[i];
            if cvw * cvh <= MAX_SVP_SIZE {
                new_svp_idx = i;
                break;
            }
        }
        let new_svp = chain_rev[new_svp_idx];
        let (new_vx, new_vy, new_vw, new_vh) = chain_rects[new_svp_idx];

        let old_svp = self.supreme_viewed_panel;

        // Change detection (C++ emView.cpp:1727-1733):
        //   forceViewingUpdate || SVP != vp || |vp->ViewedX - vx| ≥ 0.001 || ...
        let force = self.force_viewing_update;
        let rect_moved = match old_svp.and_then(|id| tree.GetRec(id)) {
            Some(p) => {
                (p.viewed_x - new_vx).abs() >= 0.001
                    || (p.viewed_y - new_vy).abs() >= 0.001
                    || (p.viewed_width - new_vw).abs() >= 0.001
            }
            None => true,
        };
        let svp_changed = old_svp != Some(new_svp);

        if !force && !svp_changed && !rect_moved {
            // No-op: viewing state unchanged. Still do active-path propagation
            // and drain nav requests so those pathways aren't gated on SVP change.
            self.svp_update_count += 1;
            if let Some(active_id) = self.active {
                if tree.contains(active_id) {
                    let mut cur = Some(active_id);
                    while let Some(id) = cur {
                        if let Some(p) = tree.get_mut(id) {
                            p.in_active_path = true;
                            cur = p.parent;
                        } else {
                            break;
                        }
                    }
                    if let Some(p) = tree.get_mut(active_id) {
                        p.is_active = true;
                    }
                }
            }
            for target in tree.drain_navigation_requests() {
                self.VisitFullsized(tree, target);
            }
            return;
        }
        self.force_viewing_update = false;
        self.svp_update_count += 1;

        // === RawVisitAbs change block (C++ emView.cpp:1753-1807) ===

        // Old SVP clear.
        if let Some(osvp) = old_svp {
            if tree.contains(osvp) {
                if let Some(p) = tree.get_mut(osvp) {
                    p.in_viewed_path = false;
                    p.viewed = false;
                }
                tree.queue_notice(
                    osvp,
                    NoticeFlags::VIEW_CHANGED
                        | NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | NoticeFlags::MEMORY_LIMIT_CHANGED,
                );
                tree.UpdateChildrenViewing(osvp);

                // Walk old SVP parent chain clearing in_viewed_path, unconditional notice
                // (C++ emView.cpp:1763-1772 does not early-exit).
                let mut cur = tree.GetRec(osvp).and_then(|p| p.parent);
                while let Some(pid) = cur {
                    let parent_of = tree.get_mut(pid).map(|p| {
                        p.in_viewed_path = false;
                        p.parent
                    });
                    tree.queue_notice(
                        pid,
                        NoticeFlags::VIEW_CHANGED
                            | NoticeFlags::UPDATE_PRIORITY_CHANGED
                            | NoticeFlags::MEMORY_LIMIT_CHANGED,
                    );
                    cur = parent_of.unwrap_or(None);
                }
            }
        }

        // New SVP set. C++ line 1780: vp->ViewedHeight = vw * vp->GetHeight()
        // / CurrentPixelTallness. Our `GetHeight` equivalent is layout_rect.h
        // / layout_rect.w (panel aspect). `new_vh` from chain_rects would be
        // equivalent but we recompute to stay structurally identical to C++.
        self.supreme_viewed_panel = Some(new_svp);
        let _ = new_vh;
        let new_vh_from_height = {
            let lr = tree.GetRec(new_svp).map(|p| p.layout_rect).unwrap_or_default();
            let panel_h = if lr.w > MIN_DIMENSION {
                lr.h / lr.w
            } else {
                1.0
            };
            new_vw * panel_h / pt
        };
        if let Some(p) = tree.get_mut(new_svp) {
            p.in_viewed_path = true;
            p.viewed = true;
            p.viewed_x = new_vx;
            p.viewed_y = new_vy;
            p.viewed_width = new_vw;
            p.viewed_height = new_vh_from_height;

            // Clip against viewport (C++ CurrentX/Y/Width/Height). Rust
            // viewport is (0, 0, vw, vh).
            let mut cx1 = new_vx;
            let mut cy1 = new_vy;
            let mut cx2 = new_vx + new_vw;
            let mut cy2 = new_vy + new_vh_from_height;
            if cx1 < 0.0 {
                cx1 = 0.0;
            }
            if cy1 < 0.0 {
                cy1 = 0.0;
            }
            if cx2 > vw {
                cx2 = vw;
            }
            if cy2 > vh {
                cy2 = vh;
            }
            p.clip_x = cx1;
            p.clip_y = cy1;
            p.clip_w = (cx2 - cx1).max(0.0);
            p.clip_h = (cy2 - cy1).max(0.0);
        }
        tree.queue_notice(
            new_svp,
            NoticeFlags::VIEW_CHANGED
                | NoticeFlags::UPDATE_PRIORITY_CHANGED
                | NoticeFlags::MEMORY_LIMIT_CHANGED,
        );
        tree.UpdateChildrenViewing(new_svp);

        // Walk new SVP parent chain setting in_viewed_path, unconditional notice.
        let mut cur = tree.GetRec(new_svp).and_then(|p| p.parent);
        while let Some(pid) = cur {
            let parent_of = tree.get_mut(pid).map(|p| {
                p.in_viewed_path = true;
                p.parent
            });
            tree.queue_notice(
                pid,
                NoticeFlags::VIEW_CHANGED
                    | NoticeFlags::UPDATE_PRIORITY_CHANGED
                    | NoticeFlags::MEMORY_LIMIT_CHANGED,
            );
            cur = parent_of.unwrap_or(None);
        }

        // Active-path propagation (unchanged from prior code).
        if let Some(active_id) = self.active {
            if tree.contains(active_id) {
                let mut cur = Some(active_id);
                while let Some(id) = cur {
                    if let Some(p) = tree.get_mut(id) {
                        p.in_active_path = true;
                        cur = p.parent;
                    } else {
                        break;
                    }
                }
                if let Some(p) = tree.get_mut(active_id) {
                    p.is_active = true;
                }
            }
        }

        for target in tree.drain_navigation_requests() {
            self.VisitFullsized(tree, target);
        }
    }
```

Delete the now-orphaned `compute_viewed_recursive` function (was emView.rs:1377-1450).

- [ ] **Step 2: Add `force_viewing_update` field if missing**

Grep:
```bash
grep -n 'force_viewing_update' crates/emcore/src/emView.rs
```

If the field does not exist on `emView`, add it next to other flag fields (near `zoomed_out_before_sg`):

```rust
    pub(crate) force_viewing_update: bool,
```

And in `new()`:
```rust
            force_viewing_update: false,
```

Also in every `SetGeometry` / `SetViewPortTallness` / similar (grep for `zoomed_out_before_sg = true`), set `self.force_viewing_update = true` alongside. This mirrors C++ callers of `RawVisit(..., forceViewingUpdate=true)` (e.g. emView.cpp:RawZoomOut:1811, SetGeometry). If uncertain, start with a conservative assignment in `SetGeometry` and any method named `Invalidate*Viewing`; grep runtime behavior in smoke test.

- [ ] **Step 3: Build**

```
cargo check -p emcore
```
Expected: success. If `tree.get_mut` or `tree.GetRec` signatures differ, adjust (both exist per the existing file at line 1332/1310).

- [ ] **Step 4: Delete `PanelTree::clear_viewing_flags`**

In `crates/emcore/src/emPanelTree.rs`, delete lines 2516-2538 (the `clear_viewing_flags` method including its docstring).

- [ ] **Step 5: Delete `PanelData.prev_viewed`**

In `crates/emcore/src/emPanelTree.rs`:

- Delete lines 193-201 (docstring + field declaration for `prev_viewed`).
- Delete line 253 (`prev_viewed: false,`) from `PanelData::new`.

- [ ] **Step 6: Build**

```
cargo check -p emcore
```
Expected: success. Any `prev_viewed` reference remaining causes a compile error — fix.

- [ ] **Step 7: Format and lint**

```
cargo fmt
cargo clippy --all-targets -- -D warnings
```
Expected: clean. Likely clippy findings and fixes:

- `non_snake_case` on `UpdateChildrenViewing` — add `#[allow(non_snake_case)]` on the method (per CLAUDE.md exception for `em`-prefixed C++ names; if clippy insists treat it as such).
- Unused variables from the rewrite (e.g. `cx, cy, cw, ch` locals): prefix with `_` or remove.
- `clippy::too_many_lines` on `Update` — CLAUDE.md permits suppression only for too-many-arguments; if too-many-lines fires, split a helper rather than suppress. Minimal refactor: extract the new-SVP-set block into `fn apply_new_svp(&mut self, tree, new_svp, new_vx, new_vy, new_vw, vw, vh, pt)`.

### Task 5: Verify commit 2 baselines

- [ ] **Step 1: Golden tests**

Run: `cargo test --release --test golden -- --test-threads=1 2>&1 | tail -20`
Expected: **≥ 235 passed, ≤ 8 failed** (same 8 set). Passing more is bonus; regressions are not permitted.

- [ ] **Step 2: emcore lib tests**

Run: `cargo test -p emcore --lib --release 2>&1 | tail -5`
Expected: **813 passed, 6 failed** (pre-existing). No regressions.

- [ ] **Step 3: Pipeline + behavioral tests**

Run: `cargo test --release --test pipeline --test behavioral 2>&1 | tail -10`
Expected: **378 + 312 passed, 0 failed**.

If any notice-related test regresses (expected notice bitmask differs), dump actual vs expected: the rewrite fires notices on transitions only, so tests that previously saw every-frame notices will now see fewer. That's correct per C++. Adjust test expectations if they were asserting every-frame behavior — but only after verifying the expected value matches the C++ behavior.

- [ ] **Step 4: Runtime smoke test**

```bash
pkill -9 -f "target/release/eaglemode" 2>/dev/null; sleep 1
cargo build --release --bin eaglemode
ulimit -c unlimited; rm -f /tmp/core.*
env DISPLAY=:0 WAYLAND_DISPLAY=wayland-1 EM_DIR=/home/a0/git/eaglemode-rs \
    target/release/eaglemode 2>/tmp/rewrite.log &
APP_PID=$!
sleep 15
if kill -0 "$APP_PID" 2>/dev/null; then
    echo "ALIVE pid=$APP_PID"
    WAYLAND_DISPLAY=wayland-1 grim -l 0 /tmp/shot.png || true
    kill "$APP_PID" 2>/dev/null
else
    echo "DEAD before 15s"
    ls /tmp/core.* 2>&1 || true
    tail -40 /tmp/rewrite.log
fi
```
Expected: `ALIVE`. No core dump. `/tmp/shot.png` shows the `/home/a0` directory listing (bookmark `VisitAtProgramStart=yes`).

If SIGSEGV persists: record as a separate follow-up task. The rewrite's goal is architectural parity; the crash may be an independent lifecycle bug in a sub-view panel's `notice()` handler that just happened to be exposed after the prev_viewed band-aid started firing notices.

- [ ] **Step 5: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
refactor(emView): port RawVisitAbs viewing update from C++

Replaces per-frame clear-then-rebuild with surgical transition detection:
- Read-only ancestor-rect walk computes new SVP candidate + its rect.
- Change detection vs persisted supreme_viewed_panel: no-op on no change.
- On change: old-SVP clear + UpdateChildrenViewing recurse + walk-up
  clearing InViewedPath; new-SVP set + UpdateChildrenViewing + walk-up
  setting InViewedPath. Each mutation fires
  VIEW_CHANGED|UPDATE_PRIORITY_CHANGED|MEMORY_LIMIT_CHANGED.
- Removes clear_viewing_flags, compute_viewed_recursive, prev_viewed
  field, and standalone in_viewed_path propagation loop.
- Renames svp field to supreme_viewed_panel (snake_case of C++ name).

Mirrors emView.cpp:1727-1807 + emPanel.cpp:1454-1518.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Post-implementation

- [ ] **Discard uncommitted trace instrumentation** (AE_NOTICE_TRACE in `emPanelTree.rs`, AE_ANIM_TRACE in `emViewAnimator.rs`):

```bash
git diff --stat crates/emcore/src/emPanelTree.rs crates/emcore/src/emViewAnimator.rs
git checkout -- crates/emcore/src/emPanelTree.rs crates/emcore/src/emViewAnimator.rs
```
Only run `git checkout --` if the diff shows *only* the AE_*_TRACE blocks. If the rewrite touched either file, rebase the decision.

- [ ] **If SIGSEGV persists after smoke test:** file as separate investigation. Add stack trace (`gdb target/release/eaglemode /tmp/core.*`) to memory file `sigsegv_post_rawvisitabs.md` with hypothesis that it's lifecycle corruption in a sub-view panel's `notice()` handler, and move on.

- [ ] **If new golden tests pass:** update `memory/golden_test_status.md` with the new baseline count.
