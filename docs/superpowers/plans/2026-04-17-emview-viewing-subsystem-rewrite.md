# emView Viewing/Geometry Subsystem Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the emView viewing/geometry subsystem's structural drift from the C++ reference by porting every missing C++ symbol, restoring the Home/Current viewport split, collapsing `Update` to the C++ drain-loop shape, and reconciling NoticeFlags names/values.

**Architecture:** Nine phases, one commit per phase (pre-commit hook enforced). Each phase is additive or mechanically-rewriting; no phase leaves the tree broken. Phase 0 audits prior Sonnet-4.6 work. Phases 1–6 reshape `emView` and related methods around Home/Current fields, `RawVisitAbs` extraction, Update drain-loop, popup infrastructure, input-recursion+engines, and geometry rewrite. Phase 7 renames/renumbers `NoticeFlags`. Phase 8 sweeps invention remnants.

**Tech Stack:** Rust 2021, bitflags, slotmap-indexed `PanelTree`, `Rc<RefCell<_>>` for `emWindow` ownership. C++ reference at `~/git/eaglemode-0.96.4/include/emCore/emView.h` + `src/emCore/emView.cpp`.

**Spec:** `docs/superpowers/plans/../specs/2026-04-17-emview-viewing-subsystem-design.md`

---

## Conventions used by every phase

**Before every task that references a C++ line number:** run `sed -n 'A,Bp' ~/git/eaglemode-0.96.4/src/emCore/emView.cpp` to re-read the cited range. The C++ is ground truth.

**Before every task that references a Rust line number:** run `git log -p -S '<unique substring>' -- crates/emcore/src/emView.rs | head -40` to verify the line still exists at the cited location. Line numbers in this plan are spec-write-time snapshots; they will drift within a phase.

**Commit message at end of every phase:**

```
<type>(emView): Phase N — <phase title>

<body explaining what was added/changed and why, enumerating any
in-phase discovery additions per the "scope up on missing" rule>

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
```

**Acceptance for every phase (run at phase end, before commit):**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo-nextest ntr
```

Expected: pre-commit hook passes, ≥235 golden passing / ≤8 failing (same baseline set as `.config/nextest.toml`), `emcore` lib ≥819 passing / 0 failing, `pipeline` + `behavioral` test crates 0 failing.

**Intra-phase test strategy:** every task that adds/changes behavior starts with a failing test before touching implementation. Tests accumulate within the phase; the single phase commit contains all tests added + implementation + any mechanical rewrites.

---

## Phase 0: Audit Sonnet 4.6 commits `bab81ec` and `3675687`

**Purpose:** Before we rewrite the change-block side effects in Phase 3, compare the Sonnet-4.6 additions against C++ `emView.cpp:1803-1806` and record the verdict. This is a doc-only phase — the faulty lines remain in place; they get rewritten in Phase 3 where the Home/Current split makes the correct rect available.

**Files:**
- Create: `docs/superpowers/notes/2026-04-17-phase0-sonnet46-audit.md`

- [ ] **Step 1: Re-read commit `bab81ec` in full**

Run:
```bash
cd /home/a0/git/eaglemode-rs
git show bab81ec --stat
git show bab81ec -- crates/emcore/src/emView.rs
```

Capture:
- Which lines of `emView.rs` the commit added.
- What side effects it set: `cursor_invalid = true`, a `dirty_rects.push(Rect::new(0.0, 0.0, viewport_width, viewport_height))`, and any `NOT PORTED` comments.
- What C++ lines it claims to port (`emView.cpp:1803-1806`).

- [ ] **Step 2: Re-read commit `3675687` in full**

Run:
```bash
git show 3675687 -- crates/emcore/src/emView.rs
```

Capture: the name and body of the test that asserts the change-block side effects (likely `test_update_change_block_side_effects`).

- [ ] **Step 3: Re-read C++ `emView.cpp:1803-1806`**

Run:
```bash
sed -n '1800,1810p' ~/git/eaglemode-0.96.4/src/emCore/emView.cpp
```

Expected output (verbatim):
```cpp
		RestartInputRecursion=true;
		CursorInvalid=true;
		UpdateEngine->WakeUp();
		InvalidatePainting();
```

Note: `InvalidatePainting()` with no args is the whole-view overload (emView.h: `void InvalidatePainting();` → invalidates `CurrentX,CurrentY,CurrentWidth,CurrentHeight`). Not `HomeX/Y/Width/Height`. This matters because during popup `Current*` differs from `Home*`.

- [ ] **Step 4: Write the audit note**

Create `docs/superpowers/notes/2026-04-17-phase0-sonnet46-audit.md`:

```markdown
# Phase 0 audit — Sonnet 4.6 commits bab81ec + 3675687

Date: 2026-04-17
Auditor: Opus 4.7 (this session)
Refs: spec 2026-04-17-emview-viewing-subsystem-design.md

## bab81ec — RawVisitAbs change-block side effects

C++ (emView.cpp:1803-1806):
- RestartInputRecursion = true
- CursorInvalid = true
- UpdateEngine->WakeUp()
- InvalidatePainting()  // whole-view: CurrentX/Y/Width/Height

Rust (bab81ec at emView.rs in Update change block):
- NOT PORTED comment for RestartInputRecursion (field does not exist)
- self.cursor_invalid = true — matches C++
- NOT PORTED comment for UpdateEngine->WakeUp (no UpdateEngine field)
- self.dirty_rects.push(Rect::new(0, 0, viewport_width, viewport_height))
  — WRONG in principle: C++ invalidates Current* rect, not viewport-sized
  rect. During popup Current* differs from Home*. Without the Home/Current
  split there is no Current rect to invalidate, so this substitute is the
  best the port could do in January's tree shape.

Verdict:
- cursor_invalid = true: KEEP.
- viewport-sized dirty rect: REPLACE in Phase 3 once Home/Current split
  exists — push invalidate_painting(Current rect) instead.
- RestartInputRecursion + UpdateEngine->WakeUp NOT PORTED comments:
  CLOSE in Phase 5 (adds those fields).

## 3675687 — test_update_change_block_side_effects

Current test asserts: dirty_rects contains exactly one rect covering
(0, 0, viewport_width, viewport_height).

Verdict: REWRITE in Phase 3. New assertion: dirty_rects contains one
rect equal to the current_rect (which in non-popup cases will equal
the home rect which equals 0,0,viewport_width,viewport_height — so
the baseline still passes — but the *test body* must read
self.current_x/y/width/height rather than self.viewport_width/height,
so it keeps passing when popup is introduced in Phase 4).
```

- [ ] **Step 5: Verify pre-commit hook still passes (no code changes yet)**

Run:
```bash
cargo fmt --check
cargo clippy -- -D warnings
```

Expected: both succeed with no output diff.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/notes/2026-04-17-phase0-sonnet46-audit.md
git commit -m "$(cat <<'EOF'
docs(emView): Phase 0 — audit Sonnet 4.6 commits bab81ec + 3675687

Records verdict on prior change-block side effects:
- cursor_invalid = true: correct, KEEP.
- Viewport-sized dirty rect push: incorrect (C++ uses Current rect,
  not Home/viewport). REPLACE in Phase 3 once Home/Current split exists.
- NOT PORTED comments for RestartInputRecursion + UpdateEngine->WakeUp:
  CLOSE in Phase 5.
- test_update_change_block_side_effects: REWRITE in Phase 3 against
  current_x/y/width/height (works identically today, stays correct
  when popup lands).

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

Expected: commit succeeds (no code change, hook trivially passes).

---

## Phase 1: Additive field-level port onto `emView`

**Purpose:** Add every C++ field from `emView.h:680-715` that is missing in Rust. Zero Rust-invention field removals in this phase — we *add*, we do not replace. Later phases remove Rust-invention fields as their last readers are rewritten. Also verifies the notice ring placed on `PanelTree` by commit `75c7c68` matches C++ semantics.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — struct def, constructor, new accessors/mutators
- Modify: `crates/emcore/src/emView.rs` tests module — add invariance tests for new fields

- [ ] **Step 1: Re-verify the C++ notice-ring semantics against `PanelTree::HandleNotice`**

Run:
```bash
sed -n '1282,1370p' ~/git/eaglemode-0.96.4/src/emCore/emView.cpp
sed -n '1430,1520p' /home/a0/git/eaglemode-rs/crates/emcore/src/emPanelTree.rs
```

Confirm three properties:
1. **Insertion order**: C++ `AddToNoticeList` inserts at the tail of a circular ring (`node->Next=&NoticeList; node->Prev=NoticeList.Prev; node->Prev->Next=node; NoticeList.Prev=node;`). The Rust `PanelTree` ring must also be tail-insertion FIFO. If it is LIFO or hashmap-order, flag and fix inline in this phase.
2. **Unlink-before-dispatch**: C++ unlinks the head node (`NoticeList.Next=n->Next; NoticeList.Next->Prev=&NoticeList; n->Prev=NULL; n->Next=NULL;`) *before* calling `p->HandleNotice()`. The Rust equivalent must also remove from the ring before the behavior callback runs, so the callback can re-insert the same panel without corrupting the ring.
3. **Wake-up signal**: `AddToNoticeList` calls `UpdateEngine->WakeUp()`. In Rust this becomes a `mark_notices_pending()` on the tree that bubbles up to `emView` so the frame loop knows to call `Update()` again. Confirm Rust has this wake-up path.

If any property diverges, **fix it in this phase, not defer**, and add a unit test in `crates/emcore/src/emPanelTree.rs` tests module named `test_notice_ring_fifo_tail_insertion` asserting enqueue-A then enqueue-B dispatches A before B.

- [ ] **Step 2: Write a failing test for the new field set**

Append to `crates/emcore/src/emView.rs` tests module:

```rust
#[test]
fn test_phase1_new_fields_default_initialized() {
    let (tree, _, _, _) = setup_tree();
    let root = tree.GetRootPanel().unwrap();
    let v = emView::new(root, 640.0, 480.0);

    // Home rect defaults to the viewport rect passed to new().
    assert_eq!(v.HomeX, 0.0);
    assert_eq!(v.HomeY, 0.0);
    assert_eq!(v.HomeWidth, 640.0);
    assert_eq!(v.HomeHeight, 480.0);
    assert_eq!(v.HomePixelTallness, 1.0);

    // Current rect starts identical to Home rect (no popup at construction).
    assert_eq!(v.CurrentX, 0.0);
    assert_eq!(v.CurrentY, 0.0);
    assert_eq!(v.CurrentWidth, 640.0);
    assert_eq!(v.CurrentHeight, 480.0);
    assert_eq!(v.CurrentPixelTallness, 1.0);

    // Invalidation / recursion flags all start false.
    assert!(!v.SVPChoiceInvalid);
    assert!(!v.SVPChoiceByOpacityInvalid);
    assert!(!v.RestartInputRecursion);
    assert_eq!(v.SettingGeometry, 0);
    assert_eq!(v.SVPUpdSlice, 0u64);
    assert!(!v.ZoomScrollInAction);

    // MinSVP / MaxSVP default to None (the "no SVP computed yet" state).
    assert!(v.MinSVP.is_none());
    assert!(v.MaxSVP.is_none());

    // LastMouse defaults to a sentinel far outside any viewport.
    assert_eq!(v.LastMouseX, -1.0e10);
    assert_eq!(v.LastMouseY, -1.0e10);

    // Signal fields exist (may be None until a scheduler wires them).
    let _: &Option<super::emSignal::SignalId> = &v.view_flags_signal;
    let _: &Option<super::emSignal::SignalId> = &v.focus_signal;
    let _: &Option<super::emSignal::SignalId> = &v.geometry_signal;
}
```

- [ ] **Step 3: Run the test to confirm it fails**

Run:
```bash
cargo test --lib --package emcore test_phase1_new_fields_default_initialized -- --nocapture
```

Expected: FAIL with "no field HomeX on emView" (or similar unknown-field errors).

- [ ] **Step 4: Add the new fields to the `emView` struct**

In `crates/emcore/src/emView.rs`, inside the `pub struct emView { ... }` definition (after the Rust-only fields, grouped together with a header comment), add:

```rust
    // === C++ Home/Current viewport split (emView.h:686-688) ===
    /// C++ HomeX — left edge of the home (non-popup) viewport rect in
    /// screen coords. Constant while popup active. 0 in non-popup Rust today.
    pub HomeX: f64,
    /// C++ HomeY — top edge of home viewport rect.
    pub HomeY: f64,
    /// C++ HomeWidth — width of home viewport rect.
    pub HomeWidth: f64,
    /// C++ HomeHeight — height of home viewport rect.
    pub HomeHeight: f64,
    /// C++ HomePixelTallness — pixel shape ratio of the home viewport
    /// (hardware property; 1.0 for square pixels).
    pub HomePixelTallness: f64,

    /// C++ CurrentX — left edge of the *current* viewport rect. Equals
    /// HomeX when no popup; set to popup-adjusted rect during popup zoom.
    pub CurrentX: f64,
    pub CurrentY: f64,
    pub CurrentWidth: f64,
    pub CurrentHeight: f64,
    pub CurrentPixelTallness: f64,

    // === C++ invalidation / recursion flags (emView.h:699-703) ===
    /// C++ SVPChoiceInvalid — next Update() must re-run RawVisitAbs to
    /// recompute the Supreme Viewed Panel.
    pub SVPChoiceInvalid: bool,
    /// C++ SVPChoiceByOpacityInvalid — opacity of a panel between MinSVP
    /// and MaxSVP has changed; Update() must walk that chain to see if
    /// the SVP needs re-choice.
    pub SVPChoiceByOpacityInvalid: bool,
    /// C++ RestartInputRecursion — signals that the in-progress input
    /// recursion (if any) should unwind and start over at the new SVP.
    pub RestartInputRecursion: bool,
    /// C++ SettingGeometry — reentrancy counter for SetGeometry. Nonzero
    /// means we are inside SetGeometry; certain invalidations are
    /// suppressed while it is nonzero.
    pub SettingGeometry: i32,
    /// C++ SVPUpdSlice — scheduler time-slice counter snapshot at the
    /// last RawVisitAbs change block. Used by the fp-instability throttle
    /// at emView.cpp:1734-1751.
    pub SVPUpdSlice: u64,
    /// C++ ZoomScrollInAction — set while a zoom/scroll gesture is in
    /// progress; suppresses certain side effects that would fight the
    /// gesture.
    pub ZoomScrollInAction: bool,

    // === C++ SVP bounds (emView.cpp:1696, 1725) ===
    /// C++ MaxSVP — topmost (root-ward) panel allowed as the SVP at the
    /// current zoom/rect. Set by RawVisitAbs.
    pub MaxSVP: Option<PanelId>,
    /// C++ MinSVP — deepest (leaf-ward) panel allowed as the SVP.
    pub MinSVP: Option<PanelId>,

    // === C++ last mouse position for CursorInvalid dispatch (emView.h:689) ===
    /// C++ LastMouseX — last known mouse X in screen coords. Default is a
    /// sentinel far outside any viewport so GetPanelAt returns None.
    pub LastMouseX: f64,
    pub LastMouseY: f64,

    // === C++ signals missing from Rust (emView.h:680, 683, 684) ===
    /// C++ ViewFlagsSignal — fired when VFlags changes.
    pub view_flags_signal: Option<super::emSignal::SignalId>,
    /// C++ FocusSignal — fired on focus gain/loss.
    pub focus_signal: Option<super::emSignal::SignalId>,
    /// C++ GeometrySignal — fired when Home/Current rect changes.
    pub geometry_signal: Option<super::emSignal::SignalId>,
```

DIVERGED: Rust `emView` keeps `viewport_width`, `viewport_height`, `visited_vw`, `visited_vh`, `viewing_dirty`, `force_viewing_update`, `pixel_tallness` for now. These are removed in later phases (Phase 3: `viewing_dirty` + `force_viewing_update`; Phase 6: `viewport_width/height`, `visited_vw/vh`, `pixel_tallness`) as their last readers are rewritten. Phase 8 is the final sweep.

Name convention: C++ struct-field names (`HomeX`, `SVPChoiceInvalid`, etc.) are kept with their original CamelCase per the File-and-Name-Correspondence rule. Clippy's `non_snake_case` warning on a struct field will fire — allow it narrowly with `#[allow(non_snake_case)]` on the struct, citing the F&N rule. Signal fields use `snake_case` (`view_flags_signal`) to match the existing `control_panel_signal` / `title_signal` pattern.

Add at top of the struct definition:

```rust
#[allow(non_snake_case)] // F&N rule: field names mirror C++ emView.h:680-715.
pub struct emView {
```

- [ ] **Step 5: Initialize the new fields in `emView::new`**

In `crates/emcore/src/emView.rs` constructor `pub fn new(root: PanelId, viewport_width: f64, viewport_height: f64) -> Self`, add initializers to the `Self { ... }` block. Place them in the same grouping as the struct definition:

```rust
            // Home rect defaults to the viewport rect passed to new().
            HomeX: 0.0,
            HomeY: 0.0,
            HomeWidth: viewport_width,
            HomeHeight: viewport_height,
            HomePixelTallness: 1.0,

            // Current rect starts equal to Home rect.
            CurrentX: 0.0,
            CurrentY: 0.0,
            CurrentWidth: viewport_width,
            CurrentHeight: viewport_height,
            CurrentPixelTallness: 1.0,

            SVPChoiceInvalid: false,
            SVPChoiceByOpacityInvalid: false,
            RestartInputRecursion: false,
            SettingGeometry: 0,
            SVPUpdSlice: 0,
            ZoomScrollInAction: false,

            MinSVP: None,
            MaxSVP: None,

            LastMouseX: -1.0e10,
            LastMouseY: -1.0e10,

            view_flags_signal: None,
            focus_signal: None,
            geometry_signal: None,
```

- [ ] **Step 6: Run the Phase 1 test to confirm it passes**

```bash
cargo test --lib --package emcore test_phase1_new_fields_default_initialized
```

Expected: PASS.

- [ ] **Step 7: Run full emcore test suite to confirm no regressions**

```bash
cargo-nextest ntr --package emcore
```

Expected: ≥819 passing, 0 failing. The new fields are unused by method bodies so behavior is unchanged.

- [ ] **Step 8: Run golden suite + pipeline + behavioral**

```bash
cargo-nextest ntr
```

Expected: ≥235 golden passing / ≤8 failing (same baseline); pipeline + behavioral 0 failing.

- [ ] **Step 9: Commit**

```bash
git add -u crates/emcore/src/emView.rs crates/emcore/src/emPanelTree.rs
git commit -m "$(cat <<'EOF'
feat(emView): Phase 1 — additive field-level port onto emView

Adds every C++ emView field from emView.h:680-715 that was missing
from the Rust port:

- Home rect: HomeX, HomeY, HomeWidth, HomeHeight, HomePixelTallness.
- Current rect: CurrentX, CurrentY, CurrentWidth, CurrentHeight,
  CurrentPixelTallness.
- Invalidation flags: SVPChoiceInvalid, SVPChoiceByOpacityInvalid,
  RestartInputRecursion.
- Reentrancy / throttle: SettingGeometry, SVPUpdSlice.
- Gesture gate: ZoomScrollInAction.
- SVP bounds: MinSVP, MaxSVP.
- Mouse state: LastMouseX, LastMouseY.
- Signals: view_flags_signal, focus_signal, geometry_signal.

Additive only: all new fields are initialized in emView::new but
unread by existing method bodies. Rust-invention fields
(viewport_width/height, visited_vw/vh, viewing_dirty,
force_viewing_update, pixel_tallness) are retained for now; later
phases remove them as their last readers are rewritten.

Verified the notice ring on PanelTree (commit 75c7c68) against C++
AddToNoticeList semantics: FIFO tail-insertion, unlink-before-
dispatch, wake-up signalling. Matches C++.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

Expected: pre-commit hook passes.

---

## Phase 2: Extract `RawVisitAbs`, add `FindBestSVP` / `FindBestSVPInTree`

**Purpose:** Move the change-block body that currently lives inline in `Update` (emView.rs:1341-1465 approximately) out into a named `RawVisitAbs` method with C++ signature. Add the two `FindBestSVP*` helpers so a future Phase 3 can call them. Also introduce the `RawVisit` / `RawZoomOut` private overloads (single-method with `forceViewingUpdate: bool` per spec D-1 recommendation).

**Files:**
- Modify: `crates/emcore/src/emView.rs` — add `RawVisitAbs`, `FindBestSVP`, `FindBestSVPInTree`, `RawVisit_forced` (internal), `RawZoomOut_forced` (internal)

- [ ] **Step 1: Re-read C++ `RawVisitAbs` in full**

Run:
```bash
sed -n '1543,1808p' ~/git/eaglemode-0.96.4/src/emCore/emView.cpp
```

Note the structure: signature `(emPanel*, double vx, double vy, double vw, bool forceViewingUpdate)`, VF_NO_ZOOM substitution, ancestor-clamp loop, vh computation, root-centering block, popup branch (to stub in this phase, wire in Phase 4), FindBestSVP call, MinSVP/MaxSVP computation, change-detect, SVPUpdSlice throttle, clear-old + set-new SVP chains, side effects.

- [ ] **Step 2: Re-read C++ `FindBestSVP` + `FindBestSVPInTree`**

Run:
```bash
sed -n '1828,1960p' ~/git/eaglemode-0.96.4/src/emCore/emView.cpp
```

Capture both methods' bodies; they are iterative walks with two passes (`MaxSVPSize` then `MaxSVPSearchSize`) and a tree-recursion.

- [ ] **Step 3: Write a failing test for `RawVisitAbs` parity with inline Update body**

Append to `crates/emcore/src/emView.rs` tests module:

```rust
#[test]
fn test_phase2_raw_visit_abs_matches_inline_update() {
    // Build the standard test tree: root with two children, one focused.
    let (mut tree, root, child_a, _child_b) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    v.SetActivePanel(child_a);
    // Run Update to populate SVP + viewed rects the old way.
    v.Update(&mut tree);
    let expected_svp = v.GetSupremeViewedPanel();
    let expected_vx = tree.GetRec(expected_svp.unwrap()).unwrap().viewed_x;
    let expected_vy = tree.GetRec(expected_svp.unwrap()).unwrap().viewed_y;
    let expected_vw = tree.GetRec(expected_svp.unwrap()).unwrap().viewed_width;

    // Rebuild view, call RawVisitAbs directly with the same rect; expect
    // the same SVP and same viewed rect.
    let mut tree2 = setup_tree().0;
    let root2 = tree2.GetRootPanel().unwrap();
    let mut v2 = emView::new(root2, 640.0, 480.0);
    v2.SetActivePanel(tree2.GetFirstChild(root2).unwrap());
    v2.RawVisitAbs(
        &mut tree2,
        expected_svp.unwrap(),
        expected_vx,
        expected_vy,
        expected_vw,
        false,
    );
    assert_eq!(v2.GetSupremeViewedPanel(), expected_svp);
    let svp = tree2.GetRec(expected_svp.unwrap()).unwrap();
    assert!((svp.viewed_x - expected_vx).abs() < 1e-9);
    assert!((svp.viewed_y - expected_vy).abs() < 1e-9);
    assert!((svp.viewed_width - expected_vw).abs() < 1e-9);
}
```

- [ ] **Step 4: Run the test to confirm it fails**

```bash
cargo test --lib --package emcore test_phase2_raw_visit_abs_matches_inline_update
```

Expected: FAIL with "no method named `RawVisitAbs`".

- [ ] **Step 5: Extract `RawVisitAbs` as a named method**

In `crates/emcore/src/emView.rs`, add a new method above `Update`:

```rust
    /// Port of C++ `emView::RawVisitAbs(panel, vx, vy, vw, forceViewingUpdate)`
    /// (emView.cpp:1543-1808). Sets the Supreme Viewed Panel to `panel` at
    /// the requested absolute viewport rect, propagates notices along the
    /// old and new SVP chains, and fires the CursorInvalid/WakeUp/
    /// InvalidatePainting side effects.
    ///
    /// DIVERGED: C++ `RawVisitAbs` is a private overload. Rust has no
    /// overloading — this is the sole entry point and public callers
    /// (RawVisit, Update) pass explicit bools.
    pub fn RawVisitAbs(
        &mut self,
        tree: &mut PanelTree,
        panel: PanelId,
        mut vx: f64,
        mut vy: f64,
        mut vw: f64,
        mut forceViewingUpdate: bool,
    ) {
        // emView.cpp:1554-1555
        self.SVPChoiceByOpacityInvalid = false;
        self.SVPChoiceInvalid = false;

        // emView.cpp:1557-1573: VF_NO_ZOOM branch
        let mut vp = if self.flags.contains(ViewFlags::NO_ZOOM) {
            let root = match tree.GetRootPanel() { Some(r) => r, None => return };
            let h = tree.get_height(root);
            if self.CurrentHeight * self.CurrentPixelTallness
                >= self.CurrentWidth * h
            {
                vw = self.CurrentWidth;
                vx = self.CurrentX;
                vy = self.CurrentY
                    + (self.CurrentHeight - vw * h / self.CurrentPixelTallness)
                        * 0.5;
            } else {
                vw = self.CurrentHeight * self.CurrentPixelTallness / h;
                vx = self.CurrentX + (self.CurrentWidth - vw) * 0.5;
                vy = self.CurrentY;
            }
            root
        } else {
            panel
        };

        // emView.cpp:1575-1584: ancestor-clamp loop — paste the full
        // loop here, translating parent/layout accessors via tree.
        // [full body per emView.cpp:1575-1808 — see spec §Control flow
        //  (RawVisitAbs), 13 steps. Do not elide the popup branch; leave
        //  a DIVERGED stub documenting Phase 4 wires it.]
        loop {
            let p = tree.GetRec(vp).and_then(|rec| rec.parent);
            let p = match p { Some(p) => p, None => break };
            let vp_rec = tree.GetRec(vp).unwrap();
            let w = vw / vp_rec.layout_rect.w.max(MIN_DIMENSION);
            let parent_h = tree.get_height(p);
            if w > MAX_SVP_SIZE || w * parent_h > MAX_SVP_SIZE { break; }
            vx -= vp_rec.layout_rect.x * w;
            vy -= vp_rec.layout_rect.y * w / self.CurrentPixelTallness;
            vw = w;
            vp = p;
        }

        let vp_h = tree.get_height(vp);
        let mut vh = vp_h * vw / self.HomePixelTallness;

        // emView.cpp:1588-1626: root-centering/clamping (paste literal).
        // [preserved verbatim — see spec §Control flow (RawVisitAbs), step 5]
        if Some(vp) == tree.GetRootPanel() {
            if vw < self.HomeWidth && vh < self.HomeHeight {
                vx = (self.HomeX + self.HomeWidth * 0.5 - vx) / vw;
                vy = (self.HomeY + self.HomeHeight * 0.5 - vy) / vh;
                if vh * self.HomeWidth < vw * self.HomeHeight {
                    vw = self.HomeWidth;
                    vh = vw * vp_h / self.HomePixelTallness;
                } else {
                    vh = self.HomeHeight;
                    vw = vh / vp_h * self.HomePixelTallness;
                }
                vx = self.HomeX + self.HomeWidth * 0.5 - vx * vw;
                vy = self.HomeY + self.HomeHeight * 0.5 - vy * vh;
            }

            let (x1, x2, y1, y2);
            if self.flags.contains(ViewFlags::EGO_MODE) {
                x1 = self.HomeX + self.HomeWidth * 0.5;
                x2 = x1;
                y1 = self.HomeY + self.HomeHeight * 0.5;
                y2 = y1;
            } else if vh * self.HomeWidth < vw * self.HomeHeight {
                x1 = self.HomeX;
                x2 = self.HomeX + self.HomeWidth;
                y1 = self.HomeY + self.HomeHeight * 0.5
                    - self.HomeWidth * vp_h / self.HomePixelTallness * 0.5;
                y2 = self.HomeY + self.HomeHeight * 0.5
                    + self.HomeWidth * vp_h / self.HomePixelTallness * 0.5;
            } else {
                x1 = self.HomeX + self.HomeWidth * 0.5
                    - self.HomeHeight / vp_h * self.HomePixelTallness * 0.5;
                x2 = self.HomeX + self.HomeWidth * 0.5
                    + self.HomeHeight / vp_h * self.HomePixelTallness * 0.5;
                y1 = self.HomeY;
                y2 = self.HomeY + self.HomeHeight;
            }
            if vx > x1 { vx = x1; }
            if vx < x2 - vw { vx = x2 - vw; }
            if vy > y1 { vy = y1; }
            if vy < y2 - vh { vy = y2 - vh; }
        }

        // emView.cpp:1628-1682: popup branch.
        // DIVERGED: Phase 4 wires this. For Phase 2 we leave a no-op that
        // mirrors the control flow of the non-popup path so callers see
        // identical behavior to the current inline Update body.
        if self.flags.contains(ViewFlags::POPUP_ZOOM) {
            // PHASE-4-TODO: port emView.cpp:1628-1682 popup branch.
        }

        // emView.cpp:1685: FindBestSVP(&vp, &vx, &vy, &vw)
        self.FindBestSVP(tree, &mut vp, &mut vx, &mut vy, &mut vw);

        // emView.cpp:1687-1696: MaxSVP walk.
        let mut p = vp;
        let mut w = vw;
        let mut last = p;
        loop {
            last = p;
            let parent = tree.GetRec(p).and_then(|r| r.parent);
            let parent = match parent { Some(pp) => pp, None => break };
            let layout_w = tree.GetRec(p).unwrap().layout_rect.w.max(MIN_DIMENSION);
            w /= layout_w;
            let parent_h = tree.get_height(parent);
            if w > MAX_SVP_SIZE || w * parent_h > MAX_SVP_SIZE { break; }
            p = parent;
        }
        self.MaxSVP = Some(last);

        // emView.cpp:1698-1725: MinSVP walk (descend into last-child
        // while current rect fully contains the child's rect).
        let mut sp = vp;
        let mut sx = vx;
        let mut sy = vy;
        let mut sw = vw;
        loop {
            let p = tree.GetLastChild(sp);
            let mut p = match p { Some(pp) => pp, None => break };
            let x1 = (self.CurrentX + 1e-4 - sx) / sw;
            let x2 = (self.CurrentX + self.CurrentWidth - 1e-4 - sx) / sw;
            let y1 = (self.CurrentY + 1e-4 - sy) * (self.CurrentPixelTallness / sw);
            let y2 = (self.CurrentY + self.CurrentHeight - 1e-4 - sy)
                * (self.CurrentPixelTallness / sw);
            let mut found = None;
            loop {
                let rec = tree.GetRec(p).unwrap();
                let lx = rec.layout_rect.x;
                let ly = rec.layout_rect.y;
                let lw = rec.layout_rect.w;
                let lh = rec.layout_rect.h;
                if lx < x2 && lx + lw > x1 && ly < y2 && ly + lh > y1 {
                    found = Some(p);
                    break;
                }
                let prev = tree.GetPrev(p);
                match prev { Some(pp) => p = pp, None => break }
            }
            let Some(p) = found else { break };
            let rec = tree.GetRec(p).unwrap();
            if rec.layout_rect.x > x1
                || rec.layout_rect.x + rec.layout_rect.w < x2
                || rec.layout_rect.y > y1
                || rec.layout_rect.y + rec.layout_rect.h < y2
            {
                break;
            }
            sp = p;
            sx += rec.layout_rect.x * sw;
            sy += rec.layout_rect.y * sw / self.CurrentPixelTallness;
            sw *= rec.layout_rect.w;
        }
        self.MinSVP = Some(sp);

        // emView.cpp:1727-1751: change detect + SVPUpdSlice throttle.
        let rect_moved = match self.supreme_viewed_panel.and_then(|id| tree.GetRec(id)) {
            Some(p) => {
                (p.viewed_x - vx).abs() >= 0.001
                    || (p.viewed_y - vy).abs() >= 0.001
                    || (p.viewed_width - vw).abs() >= 0.001
            }
            None => true,
        };
        let svp_changed = self.supreme_viewed_panel != Some(vp);
        if !forceViewingUpdate && !svp_changed && !rect_moved {
            return;
        }

        // SVPUpdSlice fp-instability throttle (emView.cpp:1734-1751).
        let slice = self.scheduler
            .as_ref()
            .map(|s| s.borrow().GetTimeSliceCounter())
            .unwrap_or(0);
        if self.SVPUpdSlice != slice {
            self.SVPUpdSlice = slice;
            self.svp_update_count = 0;
        }
        self.svp_update_count += 1;
        if self.svp_update_count > 1000
            && (self.svp_update_count % 1000 == 1 || self.svp_update_count > 10000)
        {
            // Per-spec "scope up on missing": emGetDblRandom helper does
            // not exist in the Rust crate. Port as a tiny module-local
            // helper — the call site is an fp-instability escape-hatch
            // that only fires after 1000+ retries in one time slice, so
            // the RNG quality is immaterial.
            vx += em_get_dbl_random(-0.01, 0.01);
            vy += em_get_dbl_random(-0.01, 0.01);
            vw *= em_get_dbl_random(0.9999999999, 1.0000000001);
        }

        // emView.cpp:1753-1772: clear old SVP chain.
        if let Some(osvp) = self.supreme_viewed_panel {
            if tree.contains(osvp) {
                if let Some(p) = tree.get_mut(osvp) {
                    p.in_viewed_path = false;
                    p.viewed = false;
                }
                tree.queue_notice(
                    osvp,
                    super::emPanel::NoticeFlags::VIEW_CHANGED
                        | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                        | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
                );
                tree.UpdateChildrenViewing(osvp);
                let mut cur = tree.GetRec(osvp).and_then(|p| p.parent);
                while let Some(pid) = cur {
                    let parent_of = tree.get_mut(pid).map(|p| {
                        p.in_viewed_path = false;
                        p.parent
                    });
                    tree.queue_notice(
                        pid,
                        super::emPanel::NoticeFlags::VIEW_CHANGED
                            | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                            | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
                    );
                    cur = parent_of.unwrap_or(None);
                }
            }
        }

        // emView.cpp:1774-1802: set new SVP chain.
        self.supreme_viewed_panel = Some(vp);
        let vp_h = tree.get_height(vp);
        let new_vh = vw * vp_h / self.CurrentPixelTallness;
        if let Some(p) = tree.get_mut(vp) {
            p.in_viewed_path = true;
            p.viewed = true;
            p.viewed_x = vx;
            p.viewed_y = vy;
            p.viewed_width = vw;
            p.viewed_height = new_vh;
            let mut cx1 = vx;
            let mut cy1 = vy;
            let mut cx2 = vx + vw;
            let mut cy2 = vy + new_vh;
            if cx1 < self.CurrentX { cx1 = self.CurrentX; }
            if cy1 < self.CurrentY { cy1 = self.CurrentY; }
            if cx2 > self.CurrentX + self.CurrentWidth { cx2 = self.CurrentX + self.CurrentWidth; }
            if cy2 > self.CurrentY + self.CurrentHeight { cy2 = self.CurrentY + self.CurrentHeight; }
            p.clip_x = cx1;
            p.clip_y = cy1;
            p.clip_w = (cx2 - cx1).max(0.0);
            p.clip_h = (cy2 - cy1).max(0.0);
        }
        tree.queue_notice(
            vp,
            super::emPanel::NoticeFlags::VIEW_CHANGED
                | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
        );
        tree.UpdateChildrenViewing(vp);
        let mut cur = tree.GetRec(vp).and_then(|p| p.parent);
        while let Some(pid) = cur {
            let parent_of = tree.get_mut(pid).map(|p| {
                p.in_viewed_path = true;
                p.parent
            });
            tree.queue_notice(
                pid,
                super::emPanel::NoticeFlags::VIEW_CHANGED
                    | super::emPanel::NoticeFlags::UPDATE_PRIORITY_CHANGED
                    | super::emPanel::NoticeFlags::MEMORY_LIMIT_CHANGED,
            );
            cur = parent_of.unwrap_or(None);
        }

        // emView.cpp:1803-1806: side effects.
        self.RestartInputRecursion = true;
        self.cursor_invalid = true;
        // UpdateEngine->WakeUp: Phase 5 replaces with engine call.
        // InvalidatePainting() whole-view — use Current rect (Phase 0 audit
        // verdict). During non-popup Current == Home, so rect = whole view.
        self.dirty_rects.push(Rect::new(
            self.CurrentX,
            self.CurrentY,
            self.CurrentWidth,
            self.CurrentHeight,
        ));
    }
```

Add a module-local `em_get_dbl_random(lo: f64, hi: f64) -> f64` helper at the top of `emView.rs` (near the other const/helper definitions):

```rust
/// Port of C++ `emGetDblRandom(double lo, double hi)` — uniform f64 in
/// `[lo, hi]`. Used by the SVPUpdSlice fp-instability escape hatch.
/// Quality-insensitive; a tiny xorshift is fine.
fn em_get_dbl_random(lo: f64, hi: f64) -> f64 {
    use std::cell::Cell;
    thread_local! {
        static RNG: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) };
    }
    RNG.with(|rng| {
        let mut x = rng.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        rng.set(x);
        let f = (x >> 11) as f64 / (1u64 << 53) as f64; // [0, 1)
        lo + (hi - lo) * f
    })
}
```

Add helper methods `FindBestSVP` and `FindBestSVPInTree`:

```rust
    /// Port of C++ `emView::FindBestSVP` (emView.cpp:1828-1880). Two-pass
    /// search for the best SVP along the ancestor chain of `panel`. The
    /// values are mutated in place.
    pub(crate) fn FindBestSVP(
        &self,
        tree: &PanelTree,
        panel: &mut PanelId,
        vx: &mut f64,
        vy: &mut f64,
        vw: &mut f64,
    ) {
        const MAX_SVP_SEARCH_SIZE: f64 = 1.0e14;
        // [paste emView.cpp:1831-1880 body translating Parent/LayoutX etc.
        //  through tree accessors. The two-pass (MaxSVPSize, MaxSVPSearchSize)
        //  loop with the b-contains check. Calls self.FindBestSVPInTree.]
        let mut vp = *panel;
        let mut vx_l = *vx;
        let mut vy_l = *vy;
        let mut vw_l = *vw;
        for i in 0..2 {
            let min_s = if i == 0 { MAX_SVP_SIZE } else { MAX_SVP_SEARCH_SIZE };
            let op = vp;
            loop {
                let parent = tree.GetRec(vp).and_then(|r| r.parent);
                let Some(p) = parent else { break };
                let lw = tree.GetRec(vp).unwrap().layout_rect.w.max(MIN_DIMENSION);
                let w = vw_l / lw;
                let parent_h = tree.get_height(p);
                if w > min_s || w * parent_h > min_s { break; }
                let lx = tree.GetRec(vp).unwrap().layout_rect.x;
                let ly = tree.GetRec(vp).unwrap().layout_rect.y;
                vx_l -= lx * w;
                vy_l -= ly * w / self.CurrentPixelTallness;
                vw_l = w;
                vp = p;
            }
            if op == vp && i > 0 { break; }
            let vp_h = tree.get_height(vp);
            let b_init = vx_l <= self.CurrentX + 1e-4
                && vx_l + vw_l >= self.CurrentX + self.CurrentWidth - 1e-4
                && vy_l <= self.CurrentY + 1e-4
                && vy_l + vp_h * vw_l / self.CurrentPixelTallness
                    >= self.CurrentY + self.CurrentHeight - 1e-4;
            let mut p = vp;
            let mut x = vx_l;
            let mut y = vy_l;
            let mut w = vw_l;
            let b = self.FindBestSVPInTree(tree, &mut p, &mut x, &mut y, &mut w, b_init);
            if *panel != p {
                *panel = p;
                *vx = x;
                *vy = y;
                *vw = w;
            }
            if b { break; }
        }
    }

    /// Port of C++ `emView::FindBestSVPInTree` (emView.cpp:1878-1960).
    /// Recursive descent picking the smallest opaque child that still
    /// contains the current rect. See translation table below.
    pub(crate) fn FindBestSVPInTree(
        &self,
        tree: &PanelTree,
        panel: &mut PanelId,
        vx: &mut f64,
        vy: &mut f64,
        vw: &mut f64,
        covering: bool,
    ) -> bool {
        // [body — translate emView.cpp:1878-1960 line-for-line using the
        //  translation table in Step 5 below.]
    }
```

**Translation table for `FindBestSVP` / `FindBestSVPInTree` C++ → Rust:**

| C++ | Rust |
|---|---|
| `*pPanel`, `*pVx`, `*pVy`, `*pVw` | `*panel`, `*vx`, `*vy`, `*vw` (`&mut` args) |
| `p->Parent` | `tree.GetRec(p).and_then(\|r\| r.parent)` |
| `p->LastChild` | `tree.GetLastChild(p)` |
| `p->Prev` | `tree.GetPrev(p)` |
| `p->LayoutX`, `LayoutY`, `LayoutWidth`, `LayoutHeight` | `tree.GetRec(p).unwrap().layout_rect.{x,y,w,h}` |
| `p->GetHeight()` | `tree.get_height(p)` |
| `p->CanvasColor.IsOpaque() \|\| ((const emPanel*)p)->IsOpaque()` | `{ let rec = tree.GetRec(p).unwrap(); rec.canvas_color.IsOpaque() \|\| rec.behavior.as_ref().map(\|b\| b.IsOpaque()).unwrap_or(false) }` |
| `MaxSVPSize` | `MAX_SVP_SIZE` (module const) |
| `MaxSVPSearchSize` | `MAX_SVP_SEARCH_SIZE` (add a new module const `= 1.0e14` — C++ emView.h:715) |
| `CurrentX`, `CurrentWidth`, `CurrentY`, `CurrentHeight`, `CurrentPixelTallness` | `self.CurrentX`, `self.CurrentWidth`, etc. (fields added in Phase 1) |
| recursive `FindBestSVPInTree(&cp, ...)` | `self.FindBestSVPInTree(tree, &mut cp, &mut cx, &mut cy, &mut cw, cc)` |

The recursion is bounded by panel tree depth — no stack overflow concern for realistic trees. Translate control flow (`return false`, `break`, `do { ... } while (p)`) preserving structure.

- [ ] **Step 6: Rewrite `Update`'s change block to delegate to `RawVisitAbs`**

In `crates/emcore/src/emView.rs` `Update`, replace the inline change block (the code from the comment `// === RawVisitAbs change block (C++ emView.cpp:1753-1807) ===` through the end of the active-path propagation block) with:

```rust
        // Delegate to the extracted RawVisitAbs.
        self.RawVisitAbs(tree, new_svp, new_vx, new_vy, new_vw, force);

        // Active-path propagation (C++ does this lazily in SetActivePanel
        // only — but Phase 3 will remove this block entirely. For Phase 2
        // we keep the existing per-frame rebuild to preserve test parity.)
        if let Some(active_id) = self.active {
            if tree.contains(active_id) {
                let mut cur = Some(active_id);
                while let Some(id) = cur {
                    if let Some(p) = tree.get_mut(id) {
                        p.in_active_path = true;
                        cur = p.parent;
                    } else { break }
                }
                if let Some(p) = tree.get_mut(active_id) {
                    p.is_active = true;
                }
            }
        }

        for target in tree.drain_navigation_requests() {
            self.VisitFullsized(tree, target);
        }
```

- [ ] **Step 7: Add `RawVisit` / `RawZoomOut` single-bool variants**

Per spec D1 (§Method signatures) recommendation (a): single method with `forceViewingUpdate: bool`. Adjust the existing `RawVisit` signature:

Find the existing:
```rust
    pub fn RawVisit(
        &mut self,
        tree: &PanelTree,
        panel: PanelId,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
    ) {
```

Change to:
```rust
    /// DIVERGED: C++ has public `RawVisit(panel, relX, relY, relA)` + private
    /// overload with extra `forceViewingUpdate` bool. Rust has no
    /// overloading — single method; existing no-arg callers pass `false`.
    pub fn RawVisit(
        &mut self,
        tree: &mut PanelTree,
        panel: PanelId,
        rel_x: f64,
        rel_y: f64,
        rel_a: f64,
        forceViewingUpdate: bool,
    ) {
```

…and add the `forceViewingUpdate` plumb-through in the body — it will end up passed to `RawVisitAbs` in Phase 3 when we wire `RawVisit` to call `RawVisitAbs` directly (C++ emView.cpp:1526-1540 shape). For Phase 2, store it in `self.force_viewing_update` as a transitional measure:

```rust
        if forceViewingUpdate {
            self.force_viewing_update = true;
        }
```

Update every call site in the crate to pass `false` as the new arg. Grep and fix:

```bash
grep -rn 'RawVisit(tree' crates/ tests/
```

Expected consumer count at spec-write time: <20 sites. Update each. Confirm with:

```bash
cargo check
```

Do the same for `RawZoomOut`:

```rust
    /// DIVERGED: C++ has public `RawZoomOut()` + private overload
    /// `RawZoomOut(forceViewingUpdate)`. Rust: single method, callers pass bool.
    pub fn RawZoomOut(&mut self, tree: &mut PanelTree, forceViewingUpdate: bool) {
```

Update all call sites.

- [ ] **Step 8: Run the Phase 2 test + full suite**

```bash
cargo test --lib --package emcore test_phase2_raw_visit_abs_matches_inline_update
cargo-nextest ntr
```

Expected: new test passes; golden suite ≥235/≤8; emcore lib all pass.

- [ ] **Step 9: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
feat(emView): Phase 2 — extract RawVisitAbs + add FindBestSVP{,InTree}

- Extracts the RawVisitAbs change block out of Update into a named
  method with C++ signature `(panel, vx, vy, vw, forceViewingUpdate)`
  per emView.cpp:1543-1808.
- Adds FindBestSVP + FindBestSVPInTree helpers, ported from
  emView.cpp:1828-1950.
- Update's inner change-block now delegates to RawVisitAbs.
- RawVisit and RawZoomOut gain explicit forceViewingUpdate bool
  arguments per spec D1(a); no overloading in Rust. All existing
  callers updated to pass false. DIVERGED comments cite the
  C++ overload-pair shape at each definition.
- Popup branch stubbed (PHASE-4-TODO) — wired in Phase 4.
- UpdateEngine->WakeUp side effect left as comment; Phase 5 wires.
- InvalidatePainting whole-view rect now uses CurrentX/Y/Width/Height
  (matches C++; identical to previous viewport-sized push in non-popup
  cases; Phase 0 audit confirmed this is the correct shape).

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

Expected: pre-commit hook passes.

---

## Phase 3: Collapse `Update` to C++ drain loop

**Purpose:** Rewrite the body of `emView::Update` to match the C++ drain-loop shape at `emView.cpp:1292-1370`. Remove the per-frame active-path clear (no C++ analog). Retire the Rust-only `viewing_dirty` and `force_viewing_update` fields in favor of `SVPChoiceInvalid`. Restore caller-trigger paths: `Scroll`/`Zoom` call `RawVisit(..., true)` directly; `SetGeometry` calls `RawZoomOut(true)` or `RawVisit(..., true)`; `emPanel::InvalidateViewing()` (or its Rust analog) is the only path that sets `SVPChoiceInvalid = true`. Rewrite the Phase 0 audited test to assert Current rect rather than viewport rect.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — Update body; Scroll/Zoom/RawScrollAndZoom/SetGeometry reset to C++ shape; remove `viewing_dirty` + `force_viewing_update` fields
- Modify: `crates/emcore/src/emPanel.rs` — if `InvalidateViewing` exists, change it to set `view.SVPChoiceInvalid = true`; if not, add the method
- Modify: `crates/emcore/src/emView.rs` tests module — rewrite the audited test

- [ ] **Step 1: Re-read C++ `emView::Update` (emView.cpp:1292-1370)**

Already captured above. Confirm the five drain conditions in order: PopupWindow close signal → NoticeList → SVPChoiceByOpacityInvalid → SVPChoiceInvalid → TitleInvalid → CursorInvalid → break.

- [ ] **Step 2: Rewrite the audited test against Current rect**

In `crates/emcore/src/emView.rs` tests, find `test_update_change_block_side_effects`. Replace body with:

```rust
#[test]
fn test_update_change_block_side_effects() {
    let (mut tree, root, _child_a, _child_b) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    // Force a full change block run.
    v.SVPChoiceInvalid = true;
    v.Update(&mut tree);

    // Verdict per Phase 0 audit: dirty_rects must contain a rect covering
    // the Current rect (not the viewport rect — these differ only under
    // popup, which is not in this test, but the test must read Current*
    // so that when popup lands in Phase 4 the test keeps passing).
    assert_eq!(v.dirty_rects_len_for_test(), 1);
    let rect = v.get_dirty_rect_for_test(0);
    assert_eq!(rect.x, v.CurrentX);
    assert_eq!(rect.y, v.CurrentY);
    assert_eq!(rect.w, v.CurrentWidth);
    assert_eq!(rect.h, v.CurrentHeight);

    // cursor_invalid set per emView.cpp:1804
    assert!(v.is_cursor_invalid());
    // RestartInputRecursion set per emView.cpp:1803
    assert!(v.RestartInputRecursion);
}
```

Add the test-only accessors if they do not already exist:

```rust
    #[cfg(test)]
    pub(crate) fn dirty_rects_len_for_test(&self) -> usize { self.dirty_rects.len() }
    #[cfg(test)]
    pub(crate) fn get_dirty_rect_for_test(&self, i: usize) -> Rect { self.dirty_rects[i] }
```

- [ ] **Step 3: Write a failing test for the new Update drain shape**

Append:

```rust
#[test]
fn test_phase3_update_drains_title_then_cursor() {
    let (mut tree, root, child_a, _child_b) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    v.SetActivePanel(child_a);
    // Bring viewing to steady state.
    v.SVPChoiceInvalid = true;
    v.Update(&mut tree);
    // Now flag title + cursor invalid; both should clear in a single
    // Update call per the drain loop.
    v.title_invalid = true;  // set the field directly — no view-level mark helper today
    v.mark_cursor_invalid();
    v.Update(&mut tree);
    assert!(!v.is_title_invalid());
    assert!(!v.is_cursor_invalid());
}

#[test]
fn test_phase3_update_does_not_touch_active_path_per_frame() {
    // C++ Update() does not mutate in_active_path. Only SetActivePanel
    // does. Verify that Update after SetActivePanel leaves in_active_path
    // unchanged across repeated calls.
    let (mut tree, root, child_a, _child_b) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    v.SetActivePanel(child_a);
    v.SVPChoiceInvalid = true;
    v.Update(&mut tree);
    let before = tree.GetRec(child_a).unwrap().in_active_path;
    // Second Update — no active-path mutation path should run.
    v.Update(&mut tree);
    let after = tree.GetRec(child_a).unwrap().in_active_path;
    assert_eq!(before, after);
    assert!(after, "active panel should still be in active path");
}
```

- [ ] **Step 4: Run tests to confirm they fail**

```bash
cargo test --lib --package emcore test_phase3 -- --nocapture
```

Expected: FAIL (Update still has old shape).

- [ ] **Step 5: Rewrite `Update` to C++ drain-loop shape**

Replace the entire body of `pub fn Update(&mut self, tree: &mut PanelTree)` with:

```rust
    /// Port of C++ `emView::Update` (emView.cpp:1292-1370). Drain loop
    /// dispatching in priority order: popup-close → notices →
    /// SVPChoiceByOpacityInvalid → SVPChoiceInvalid → TitleInvalid →
    /// CursorInvalid.
    ///
    /// DIVERGED: the notice-drain inner loop lives on PanelTree
    /// (commit 75c7c68) — `tree.HandleNotice` does the unlink-and-
    /// dispatch per node. Rest of the shape is identical.
    pub fn Update(&mut self, tree: &mut PanelTree) {
        // C++ emView.cpp:1299-1301: popup close.
        // PHASE-4-TODO: check self.PopupWindow close signal and ZoomOut.

        // First-frame zoom-out: C++ ZoomedOutBeforeSG.
        if self.zoomed_out_before_sg {
            self.zoomed_out_before_sg = false;
            let rel_a = self.zoom_out_rel_a(tree);
            if let Some(state) = self.visit_stack.last_mut() {
                state.rel_a = rel_a;
            }
            self.SVPChoiceInvalid = true;
        }

        loop {
            // Drain all pending notices (delegated).
            if tree.has_pending_notices() {
                tree.HandleNotice(self.window_focused, self.CurrentPixelTallness);
                continue;
            }

            if self.SVPChoiceByOpacityInvalid {
                self.SVPChoiceByOpacityInvalid = false;
                if !self.SVPChoiceInvalid
                    && self.MinSVP.is_some()
                    && self.MinSVP != self.MaxSVP
                {
                    let mut p = self.MinSVP;
                    let max = self.MaxSVP;
                    while p != max {
                        let opaque = p.and_then(|id| tree.GetRec(id)).map(|rec| {
                            rec.canvas_color.IsOpaque()
                                || rec.behavior.as_ref().map(|b| b.IsOpaque()).unwrap_or(false)
                        }).unwrap_or(false);
                        if opaque { break; }
                        p = p.and_then(|id| tree.GetRec(id).and_then(|r| r.parent));
                    }
                    if self.supreme_viewed_panel != p {
                        dlog!("SVP choice invalid by opacity.");
                        self.SVPChoiceInvalid = true;
                    }
                }
                continue;
            }

            if self.SVPChoiceInvalid {
                self.SVPChoiceInvalid = false;
                if let Some((panel, _, _, _)) = self.GetVisitedPanel(tree) {
                    let rec = tree.GetRec(panel).unwrap();
                    let (vx, vy, vw) = (rec.viewed_x, rec.viewed_y, rec.viewed_width);
                    self.RawVisitAbs(tree, panel, vx, vy, vw, false);
                }
                continue;
            }

            if self.title_invalid {
                self.title_invalid = false;
                let new_title = self.active
                    .and_then(|id| tree.GetRec(id))
                    .map(|p| p.title.clone())
                    .unwrap_or_default();
                if self.title != new_title {
                    self.title = new_title;
                    self.invalidate_view_title();
                }
                continue;
            }

            if self.cursor_invalid {
                self.cursor_invalid = false;
                let p = self.GetPanelAt(tree, self.LastMouseX, self.LastMouseY);
                let mut cur = p
                    .and_then(|id| tree.GetRec(id))
                    .and_then(|rec| rec.behavior.as_ref().map(|b| b.GetCursor()))
                    .unwrap_or(emCursor::Normal);
                if self.flags.contains(ViewFlags::EGO_MODE) && cur == emCursor::Normal {
                    cur = emCursor::Crosshair;
                }
                if self.cursor != cur {
                    self.cursor = cur;
                    // C++ `InvalidateCursor()` (view-level). In Rust: the
                    // cursor field IS the display state; no separate
                    // per-view cursor-dirty flag is needed because the
                    // frame compositor reads `self.cursor` each paint.
                    // If a future backend needs the dirty flag, add a
                    // `self.cursor_needs_redraw = true` alongside here.
                }
                continue;
            }

            break;
        }
    }
```

If `emCursor::Crosshair` does not exist in Rust, add it now (F&N rule: it is in C++ `emCursor.h`). If `tree.has_pending_notices()` does not exist, add it as a thin wrapper around the ring's non-empty check.

- [ ] **Step 6: Rewrite `Scroll` / `Zoom` / `RawScrollAndZoom` / `SetGeometry` to call `RawVisit(..., true)` directly**

In `Scroll`:
Find the line `self.viewport_changed = true; self.viewing_dirty = true;` and replace with:

```rust
        // C++ Scroll/Zoom/RawScrollAndZoom end with RawVisit(vp, rx, ry, ra, true).
        let visit = self.current_visit().clone();
        self.RawVisit(tree, visit.panel, visit.rel_x, visit.rel_y, visit.rel_a, true);
```

(Note: `Scroll` currently lacks `tree` as an argument. Update signature and all callers: `pub fn Scroll(&mut self, tree: &mut PanelTree, dx: f64, dy: f64)`.)

Apply the same pattern to `Zoom` and `RawScrollAndZoom`. Use `&mut PanelTree` everywhere needed.

In `SetGeometry`, replace:
```rust
        self.viewport_changed = true;
        self.viewing_dirty = true;
        self.force_viewing_update = true;
```
with (per C++ emView.cpp:1272-1277):
```rust
        if was_zoomed_out {
            self.RawZoomOut(tree, true);
        } else if let Some((panel, rx, ry, ra)) = visited_before {
            self.RawVisit(tree, panel, rx, ry, ra, true);
        }
```
(capture `let visited_before = self.GetVisitedPanel(tree);` at the top of the method before Home/Current are mutated — matching C++ emView.cpp:1256.)

- [ ] **Step 7: Route `InvalidateViewing` to set `SVPChoiceInvalid`**

Check `crates/emcore/src/emPanel.rs` for `InvalidateViewing`. If it exists, modify it to set the view's `SVPChoiceInvalid = true` (via a `&mut emView` argument, or via a `mark_svp_choice_invalid()` helper on the tree/view pair). If it does not exist, add it:

```rust
    /// Port of C++ `emPanel::InvalidateViewing` (emPanel.cpp). Marks the
    /// view's SVPChoiceInvalid flag so the next Update picks a new SVP.
    pub fn InvalidateViewing(tree: &mut PanelTree, view: &mut emView) {
        view.SVPChoiceInvalid = true;
    }
```

(Exact shape depends on how panels currently reach their view. Follow the existing `InvalidateCursor` / `InvalidateTitle` pattern on `emView`.)

- [ ] **Step 8: Remove Rust-invention fields `viewing_dirty` and `force_viewing_update`**

Grep for remaining readers:
```bash
grep -rn 'viewing_dirty\|force_viewing_update' crates/
```

If non-test readers remain, they are by definition the last readers; they should have been rewritten in Steps 5–7. Remove field declarations from the struct, remove initializers from `new`, remove the `mark_viewing_dirty` helper if present (rewrite callers to set `SVPChoiceInvalid = true` directly). Keep `#[cfg(test)]` test helpers if they are still referenced in unconverted tests — but prefer converting the tests in this step too.

- [ ] **Step 9: Run the suite**

```bash
cargo-nextest ntr
```

Expected: Phase 3 tests pass; full suite ≥235 golden / ≤8 baseline / emcore all pass.

- [ ] **Step 10: Runtime smoke (per spec acceptance for Phase 3)**

Run the full app and verify it stays ALIVE ≥15 s without a core dump, cursor/paint refresh correctly:

```bash
cargo run --bin eaglemode --release &
EM_PID=$!
sleep 15
if kill -0 $EM_PID 2>/dev/null; then
    echo "ALIVE"
    kill $EM_PID
else
    echo "DIED"; exit 1
fi
```

Expected: prints "ALIVE". No core dump in cwd or `/var/log/`.

- [ ] **Step 11: Commit**

```bash
git add -u
git commit -m "$(cat <<'EOF'
feat(emView): Phase 3 — collapse Update to C++ drain loop

Rewrites emView::Update to match C++ emView.cpp:1292-1370 exactly:
priority-ordered drain of popup-close, notices, SVPChoiceByOpacity-
Invalid, SVPChoiceInvalid (→ RawVisitAbs), TitleInvalid, CursorInvalid.
Notice drain delegates to PanelTree::HandleNotice (DIVERGED — ring
owned by PanelTree, per commit 75c7c68).

Caller-trigger paths restored:
- Scroll/Zoom/RawScrollAndZoom call RawVisit(..., true) directly.
- SetGeometry calls RawZoomOut(true) or RawVisit(..., true) directly
  per C++ emView.cpp:1272-1277.
- emPanel::InvalidateViewing sets view.SVPChoiceInvalid — the only
  path that flips it.

Rust-invention fields removed:
- viewing_dirty → SVPChoiceInvalid.
- force_viewing_update → forceViewingUpdate method argument only.

Per-frame active-path clear in Update removed — no C++ analog.
SetActivePanel remains the sole active-path mutator.

test_update_change_block_side_effects rewritten per Phase 0 audit:
asserts Current rect, not viewport rect. Passes today (non-popup),
stays correct when popup lands in Phase 4.

Runtime smoke: eaglemode stays ALIVE ≥15 s, no core dump.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

---

## Phase 4: Popup infrastructure

**Purpose:** Port `emViewPort` class, add popup-related fields and methods on `emView`, port the popup branch in `RawVisitAbs` (emView.cpp:1628-1682), add popup-window creation hooks. Per spec D3 this is in-scope, not deferred.

**Implementation note:** if `emViewPort` port grows beyond ~600 LOC it may be split into sub-commits Phase 4a (port `emViewPort`) and Phase 4b (popup wiring using it) — this is a runtime split decision; the spec permits it (§D3a). Default is one phase = one commit.

**Files:**
- Create: `crates/emcore/src/emViewPort.rs`
- Modify: `crates/emcore/src/emCore/mod.rs` or `crates/emcore/src/lib.rs` — wire `emViewPort` module
- Modify: `crates/emcore/src/emView.rs` — add `PopupWindow`, `HomeViewPort`, `CurrentViewPort`, `DummyViewPort` fields; add `SwapViewPorts` + `GetMaxPopupViewRect` methods; complete popup branch in `RawVisitAbs`
- Modify: `crates/emcore/src/emWindow.rs` — add popup constructor if missing

- [ ] **Step 1: Read C++ `emViewPort` class in full**

```bash
sed -n '719,795p' ~/git/eaglemode-0.96.4/include/emCore/emView.h
grep -n "emViewPort::" ~/git/eaglemode-0.96.4/src/emCore/emView.cpp | head -30
```

Capture all method signatures: `GetViewX`, `GetViewY`, `GetViewWidth`, `GetViewHeight`, `GetViewCursor`, `PaintView`, `SetViewGeometry`, `SetViewFocused`, `RequestFocus`, `IsSoftKeyboardShown`, `ShowSoftKeyboard`, `GetInputClockMS`, `InputToView`, `InvalidateCursor`, `InvalidatePainting`. Per spec D3a: port the field definitions and signatures of all methods; implement bodies only for methods called by emView viewing/geometry paths; document unimplemented methods with `DIVERGED:` citing the backend gap.

- [ ] **Step 2: Create `crates/emcore/src/emViewPort.rs`**

```rust
// Port of C++ emViewPort (emView.h:719-794). View↔OS connection class.
// Owns InvalidateCursor / InvalidatePainting / InvalidateTitle /
// RequestFocus / SetViewPosSize that emView currently open-codes.
//
// DIVERGED: not a trait, but a concrete struct with optional backend
// hooks. Rust has no dummy-base-class pattern; the "default implementation
// connects to nothing" model becomes an Option<Box<dyn BackendPort>>.

// [full body — see spec §D3a. For Phase 4 implement only:
//   GetViewX/Y/Width/Height — delegate to HomeView (the emView it belongs to)
//   InvalidatePainting — accumulate dirty rects on the backend
//   InvalidateCursor — backend cursor dirty flag
//   RequestFocus — backend request-focus hook
//   SetViewPosSize — used by SwapViewPorts + popup window placement
// Mark others PHASE-5-TODO or DIVERGED backend-gap as appropriate.]
```

- [ ] **Step 3: Write failing test for popup creation via `RawVisitAbs`**

```rust
#[test]
fn test_phase4_popup_zoom_creates_popup_window() {
    let (mut tree, root, child_a, _) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    v.SetViewFlags(ViewFlags::POPUP_ZOOM, &mut tree);
    // Visit a child with a rect that falls outside the home rect.
    v.RawVisit(&mut tree, child_a, 0.0, 0.0, 0.1, true);
    v.Update(&mut tree);

    // Expected: popup_window is Some, Current* != Home* after popup placement.
    assert!(v.PopupWindow.is_some());
    assert_ne!(v.CurrentWidth, v.HomeWidth);
}
```

- [ ] **Step 4: Run to confirm it fails**

```bash
cargo test --lib --package emcore test_phase4_popup_zoom_creates_popup_window
```

Expected: FAIL (no `PopupWindow` field, no popup branch in RawVisitAbs).

- [ ] **Step 5: Add popup fields to `emView`**

In the struct:

```rust
    // === C++ popup infrastructure (emView.h:708-713) ===
    /// C++ PopupWindow — owned handle to the popup window created when
    /// zooming past the home-rect edges under VF_POPUP_ZOOM.
    pub PopupWindow: Option<Rc<RefCell<super::emWindow::emWindow>>>,
    /// C++ HomeViewPort — the view-port that connects the emView to its
    /// *home* window (the original non-popup window).
    pub HomeViewPort: Rc<RefCell<super::emViewPort::emViewPort>>,
    /// C++ CurrentViewPort — currently-active view-port. Swapped with
    /// HomeViewPort by SwapViewPorts during popup push/pop.
    pub CurrentViewPort: Rc<RefCell<super::emViewPort::emViewPort>>,
    /// C++ DummyViewPort — the sentinel "no backend attached" port,
    /// returned by the accessors during construction before a real
    /// port is attached.
    pub DummyViewPort: Rc<RefCell<super::emViewPort::emViewPort>>,
```

Initialize in `new`:

```rust
            PopupWindow: None,
            HomeViewPort: Rc::new(RefCell::new(super::emViewPort::emViewPort::new_dummy())),
            CurrentViewPort: Rc::new(RefCell::new(super::emViewPort::emViewPort::new_dummy())),
            DummyViewPort: Rc::new(RefCell::new(super::emViewPort::emViewPort::new_dummy())),
```

- [ ] **Step 6: Implement `SwapViewPorts` and `GetMaxPopupViewRect`**

Per spec:

```rust
    /// Port of C++ `emView::SwapViewPorts(bool swapFocus)` (emView.cpp:~2600).
    pub fn SwapViewPorts(&mut self, swap_focus: bool) {
        std::mem::swap(&mut self.HomeViewPort, &mut self.CurrentViewPort);
        if swap_focus {
            let vp_focus = self.CurrentViewPort.borrow().is_focused();
            self.CurrentViewPort.borrow_mut().set_focused(self.window_focused);
            self.HomeViewPort.borrow_mut().set_focused(vp_focus);
        }
    }

    /// Port of C++ `emView::GetMaxPopupViewRect(pX, pY, pW, pH)`.
    /// Returns the maximum bounding rect for the popup window (usually
    /// the owning monitor's work area).
    pub fn GetMaxPopupViewRect(&self, out: &mut (f64, f64, f64, f64)) {
        // Delegate to the window backend via the view port if available.
        if let Some(ref rect) = self.max_popup_rect {
            *out = (rect.x, rect.y, rect.w, rect.h);
        } else {
            // Fallback: entire home rect.
            *out = (self.HomeX, self.HomeY, self.HomeWidth, self.HomeHeight);
        }
    }
```

- [ ] **Step 7: Port popup branch inside `RawVisitAbs`**

Replace the `PHASE-4-TODO` stub in `RawVisitAbs` (added in Phase 2) with the full popup branch per `emView.cpp:1628-1682`:

```rust
        // emView.cpp:1628-1682: popup branch.
        if self.flags.contains(ViewFlags::POPUP_ZOOM) {
            let outside_home = Some(vp) != tree.GetRootPanel()
                || vx < self.HomeX - 0.1
                || vx + vw > self.HomeX + self.HomeWidth + 0.1
                || vy < self.HomeY - 0.1
                || vy + vh > self.HomeY + self.HomeHeight + 0.1;

            if outside_home {
                if self.PopupWindow.is_none() {
                    // Rust `window_focused` is the analog of C++ `Focused`
                    // (the emView's own focus state). Not renamed — out of
                    // scope for this spec; existing code uses this name.
                    let was_focused = self.window_focused;
                    let popup = super::emWindow::emWindow::new_popup(
                        self,
                        super::emWindow::WindowFlags::POPUP,
                        "emViewPopup",
                    );
                    self.PopupWindow = Some(popup);
                    // UpdateEngine->AddWakeUpSignal — Phase 5 wires this.
                    if let Some(ref w) = self.PopupWindow {
                        w.borrow_mut().SetBackgroundColor(self.background_color);
                    }
                    self.SwapViewPorts(true);
                    if was_focused && !self.window_focused {
                        self.CurrentViewPort.borrow_mut().RequestFocus();
                    }
                }
                let mut sr = (0.0, 0.0, 0.0, 0.0);
                self.GetMaxPopupViewRect(&mut sr);
                let (sx, sy, sw, sh) = sr;
                let (x1, y1, x2, y2);
                if Some(vp) == tree.GetRootPanel() {
                    let mut ax1 = vx.floor();
                    let mut ay1 = vy.floor();
                    let mut ax2 = (vx + vw).ceil();
                    let mut ay2 = (vy + vh).ceil();
                    if ax1 < sx { ax1 = sx; }
                    if ay1 < sy { ay1 = sy; }
                    if ax2 > sx + sw { ax2 = sx + sw; }
                    if ay2 > sy + sh { ay2 = sy + sh; }
                    if ax2 < ax1 + 1.0 { ax2 = ax1 + 1.0; }
                    if ay2 < ay1 + 1.0 { ay2 = ay1 + 1.0; }
                    x1 = ax1; y1 = ay1; x2 = ax2; y2 = ay2;
                } else {
                    x1 = sx; y1 = sy; x2 = sx + sw; y2 = sy + sh;
                }
                if (x1 - self.CurrentX).abs() > 0.01
                    || (x2 - self.CurrentX - self.CurrentWidth).abs() > 0.01
                    || (y1 - self.CurrentY).abs() > 0.01
                    || (y2 - self.CurrentY - self.CurrentHeight).abs() > 0.01
                {
                    self.SwapViewPorts(false);
                    if let Some(ref w) = self.PopupWindow {
                        w.borrow_mut().SetViewPosSize(x1, y1, x2 - x1, y2 - y1);
                    }
                    self.SwapViewPorts(false);
                    forceViewingUpdate = true;
                }
            } else if self.PopupWindow.is_some() {
                self.SwapViewPorts(true);
                self.PopupWindow = None;
                // Signal(GeometrySignal) — Phase 5.
                forceViewingUpdate = true;
            }
        }
```

- [ ] **Step 8: Add `emWindow::new_popup` constructor if missing**

Grep:
```bash
grep -n "new_popup\|new_with_flags\|WindowFlags::POPUP" crates/emcore/src/emWindow.rs | head
```

If absent, add a minimal popup constructor that creates a window with `WindowFlags::POPUP`, no title, a parent pointer to the owning `emView`. The C++ constructor invocation is:

```cpp
PopupWindow=new emWindow(*this, 0, emWindow::WF_POPUP, "emViewPopup");
```

Port signature: `pub fn new_popup(owner: &emView, flags: WindowFlags, tag: &str) -> Rc<RefCell<emWindow>>`.

- [ ] **Step 9: Run tests**

```bash
cargo-nextest ntr
```

Expected: Phase 4 test passes; full suite ≥235/≤8/all-pass.

- [ ] **Step 10: Commit**

Single-commit phase. If at this point the diff exceeds ~1500 lines or the popup wiring proved more involved than expected, split into two commits:

1. `feat(emViewPort): Phase 4a — port emViewPort class` — `emViewPort.rs`, module wiring, initial dummy-port creation in `emView::new`, accessors.
2. `feat(emView): Phase 4b — popup branch in RawVisitAbs + SwapViewPorts` — fields, methods, popup wiring in `RawVisitAbs`.

Otherwise single commit:

```bash
git add -A crates/emcore/src/emViewPort.rs crates/emcore/src/emView.rs \
    crates/emcore/src/emWindow.rs crates/emcore/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(emView): Phase 4 — popup infrastructure (emViewPort, PopupWindow, ...)

Ports emViewPort class (emView.h:719-794) as a concrete struct with
optional backend hooks (DIVERGED: no dummy-base pattern in Rust).
Implements the methods called by emView viewing/geometry paths
(GetView{X,Y,Width,Height}, SetViewGeometry, InvalidatePainting,
InvalidateCursor, RequestFocus, SetViewPosSize); stubs the rest
with PHASE-5-TODO or DIVERGED-backend-gap.

Adds popup-related fields on emView: PopupWindow (Option<Rc<RefCell
<emWindow>>>), HomeViewPort, CurrentViewPort, DummyViewPort.

Implements SwapViewPorts(swap_focus) + GetMaxPopupViewRect.

Completes the popup branch inside RawVisitAbs (emView.cpp:1628-1682):
creates PopupWindow on push past home-rect edges, places it via
SetViewPosSize, tears down on return inside home. forceViewingUpdate
flipped on geometry change per C++.

Adds emWindow::new_popup constructor hook.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

---

## Phase 5: `RecurseInput`, `InvalidateHighlight`, `AddToNoticeList`, `UpdateEngineClass`, `EOIEngineClass`

**Purpose:** Port the input-recursion machinery, per-panel highlight invalidation, per-view notice-enqueue helper, and the two `emEngine` subclasses (`UpdateEngine`, `EOIEngine`) that the C++ view uses for scheduler integration.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — add `RecurseInput` (two overloads → single method + wrapper), `InvalidateHighlight`, `AddToNoticeList` on emView, `UpdateEngine: UpdateEngineClass`, `EOIEngine: EOIEngineClass` fields
- Modify: `crates/emcore/src/emEngine.rs` — confirm `Cycle` callback pattern works for subclasses
- Likely modify: engine-scheduler wiring

- [ ] **Step 1: Verify `emEngine` subclass pattern works**

Per spec open question #3: confirm Rust `emEngine.rs` supports subclass-style `Cycle()` callbacks compatible with C++ expectations. If not, decide whether to extend `emEngine` or inline the engine's tick into `Update`.

```bash
grep -n "pub fn Cycle\|trait.*Cycle\|impl.*for.*emEngine" crates/emcore/src/emEngine.rs | head
```

If pattern exists: use it. If not: add a trait-based extension in this step before moving on.

- [ ] **Step 2: Write a failing test for the EOI countdown replacement**

```rust
#[test]
fn test_phase5_eoi_engine_replaces_countdown() {
    let (mut tree, root, _, _) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    v.SignalEOIDelayed();
    assert!(v.eoi_delayed());
    // After enough scheduler ticks the engine fires and clears.
    for _ in 0..10 { v.tick_eoi(); }
    assert!(!v.eoi_delayed());
}
```

- [ ] **Step 3: Run to confirm it fails or passes trivially**

```bash
cargo test --lib --package emcore test_phase5_eoi_engine_replaces_countdown
```

Baseline: likely already passes because `eoi_countdown` exists. The goal is to reshape, not change behavior — so this test is a *parity* test. Keep it and make sure it still passes after the rewrite.

- [ ] **Step 4: Add `UpdateEngineClass` / `EOIEngineClass`**

Replace `eoi_countdown: Option<i32>` with an `EOIEngineClass` struct that owns the countdown. Mirror `UpdateEngineClass` as a wake-up+tick driver. Follow the existing `emEngine` subclass pattern.

- [ ] **Step 5: Add `RecurseInput` (two overloads → single method)**

Per spec D1, Rust uses a single method with explicit args for the overload case. Port C++ `emView::RecurseInput(emInputEvent&, emInputState&)` and the private overload `RecurseInput(emInputEvent&, emInputState&, emPanel*)`:

```rust
    /// DIVERGED: C++ has public RecurseInput(e, s) + private
    /// RecurseInput(e, s, panel). Rust: single method; public callers
    /// pass `None` for the start panel.
    pub fn RecurseInput(
        &mut self,
        tree: &mut PanelTree,
        event: &mut emInputEvent,
        state: &emInputState,
        start_panel: Option<PanelId>,
    ) {
        // [port C++ body]
    }
```

- [ ] **Step 6: Add `InvalidateHighlight` and `AddToNoticeList`**

```rust
    /// Port of C++ `emView::InvalidateHighlight`.
    pub fn InvalidateHighlight(&mut self) {
        // [port C++ body — typically sets a highlight-dirty flag
        //  that the paint pass consults.]
    }

    /// Port of C++ `emView::AddToNoticeList(PanelRingNode*)` (emView.cpp:1282).
    /// DIVERGED: Rust owns the ring on PanelTree, so AddToNoticeList
    /// delegates; it exists on emView for C++-signature parity.
    pub fn AddToNoticeList(&mut self, tree: &mut PanelTree, panel: PanelId) {
        tree.add_to_notice_list(panel);
        if let Some(ref engine) = self.UpdateEngine {
            engine.WakeUp();
        }
    }
```

- [ ] **Step 7: Close the Phase 2 `UpdateEngine->WakeUp` NOT PORTED comment**

In `RawVisitAbs`, replace the `// UpdateEngine->WakeUp: Phase 5 replaces with engine call.` comment with an actual call:

```rust
        if let Some(ref engine) = self.UpdateEngine {
            engine.WakeUp();
        }
```

- [ ] **Step 8: Run the suite**

```bash
cargo-nextest ntr
```

Expected: all Phase 5 tests pass; full suite baseline holds.

- [ ] **Step 9: Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(emView): Phase 5 — RecurseInput, InvalidateHighlight, engines

Ports input recursion (RecurseInput overload pair → single method),
InvalidateHighlight, AddToNoticeList on emView. Replaces the
eoi_countdown field with an EOIEngineClass emEngine subclass and
adds UpdateEngineClass for RawVisitAbs WakeUp dispatch.

Closes the UpdateEngine->WakeUp NOT PORTED comment inside
RawVisitAbs.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

---

## Phase 6: `SetGeometry` / `Scroll` / `Zoom` / `RawZoomOut` / `SetViewFlags` / constructor rewrite

**Purpose:** Rewrite geometry-mutating entry points against the new Home/Current fields. Remove the pt band-aid at `emView.rs:1181` (will have moved; grep to find its current line). Add `SetViewPortTallness`. Every `SetGeometry` caller passes explicit `pixelTallness`.

**Files:**
- Modify: `crates/emcore/src/emView.rs` — `SetGeometry`, `Scroll`, `Zoom`, `RawZoomOut`, `SetViewFlags`, `new`; add `SetViewPortTallness`
- Modify: every caller of `SetGeometry` in the workspace (run grep; expected <10 sites in `emWindow.rs`, `gen_golden.cpp`-equivalent test fixtures, etc.)
- Modify: every caller of `Scroll` / `Zoom` to pass the new `tree` argument (actually done in Phase 3; confirm)

- [ ] **Step 1: Grep current `SetGeometry` call sites**

```bash
grep -rn "\.SetGeometry\|set_geometry(" crates/ tests/ | grep -v docs
```

Capture the list. Every one of these will be updated.

- [ ] **Step 2: Grep current `tree.set_pixel_tallness` band-aid**

```bash
grep -n "tree.set_pixel_tallness" crates/emcore/src/emView.rs
```

Capture current line (spec-write-time was `emView.rs:1181`). This line is removed in this phase.

- [ ] **Step 3: Write a failing test for `SetGeometry` accepting explicit pt**

```rust
#[test]
fn test_phase6_set_geometry_accepts_pixel_tallness() {
    let (mut tree, root, _, _) = setup_tree();
    let mut v = emView::new(root, 640.0, 480.0);
    v.SetGeometry(&mut tree, 100.0, 50.0, 800.0, 600.0, 1.25);
    assert_eq!(v.HomeX, 100.0);
    assert_eq!(v.HomeY, 50.0);
    assert_eq!(v.HomeWidth, 800.0);
    assert_eq!(v.HomeHeight, 600.0);
    assert_eq!(v.HomePixelTallness, 1.25);
    assert_eq!(v.CurrentX, 100.0);  // Current tracks Home when no popup.
    assert_eq!(v.CurrentPixelTallness, 1.25);
}
```

- [ ] **Step 4: Run to confirm failure**

Expected: compile error ("too many arguments to SetGeometry") or assertion failure.

- [ ] **Step 5: Rewrite `SetGeometry` against Home/Current fields**

Per C++ emView.cpp:1238-1279 (already captured above). Signature:

```rust
    pub fn SetGeometry(
        &mut self,
        tree: &mut PanelTree,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        pixel_tallness: f64,
    ) {
        let width = width.max(1e-4);
        let height = height.max(1e-4);
        let pixel_tallness = pixel_tallness.max(1e-4);

        if self.CurrentX == x
            && self.CurrentY == y
            && self.CurrentWidth == width
            && self.CurrentHeight == height
            && self.CurrentPixelTallness == pixel_tallness
        {
            return;
        }

        self.zoomed_out_before_sg = self.IsZoomedOut(tree);
        self.SettingGeometry += 1;
        let visited_before = self.GetVisitedPanel(tree);

        // Home fields track Current on the home view port (no popup).
        // During popup, HomeView is the non-popup view and updates its
        // home fields; the current (popup) view updates only Current*.
        // Phase 4 set up HomeViewPort; here we do the simpler path where
        // both track together if CurrentViewPort == HomeViewPort.
        let is_home = Rc::ptr_eq(&self.HomeViewPort, &self.CurrentViewPort);
        if is_home {
            self.HomeX = x;
            self.HomeY = y;
            self.HomeWidth = width;
            self.HomeHeight = height;
            self.HomePixelTallness = pixel_tallness;
        }
        self.CurrentX = x;
        self.CurrentY = y;
        self.CurrentWidth = width;
        self.CurrentHeight = height;
        self.CurrentPixelTallness = pixel_tallness;

        // Signal(GeometrySignal) — fire.
        if let Some(sig) = self.geometry_signal {
            if let Some(sched) = &self.scheduler {
                sched.borrow_mut().signal(sig);
            }
        }

        if self.flags.contains(ViewFlags::ROOT_SAME_TALLNESS) {
            tree.Layout(self.root, 0.0, 0.0, 1.0, self.GetHomeTallness());
        }

        if self.zoomed_out_before_sg {
            self.RawZoomOut(tree, true);
        } else if let Some((panel, rx, ry, ra)) = visited_before {
            self.RawVisit(tree, panel, rx, ry, ra, true);
        }

        self.SettingGeometry -= 1;
    }

    /// C++ `emView::GetHomeTallness` (emView.h:874-876).
    pub fn GetHomeTallness(&self) -> f64 {
        self.HomeHeight / self.HomeWidth * self.HomePixelTallness
    }

    /// C++ `emView::SetViewPortTallness` (not in emView.h:571 — but on
    /// emViewPort; see emView.h:763). Adjusts the pixel tallness of the
    /// current view port.
    pub fn SetViewPortTallness(&mut self, tree: &mut PanelTree, tallness: f64) {
        self.SetGeometry(
            tree,
            self.CurrentX,
            self.CurrentY,
            self.CurrentWidth,
            self.CurrentHeight,
            tallness.max(1e-4),
        );
    }
```

- [ ] **Step 6: Remove the pt band-aid**

Delete the line `tree.set_pixel_tallness(1.0);` from `Update` (it moved; find current location by grep). Also delete `set_pixel_tallness` from `PanelTree` if it has no remaining callers:

```bash
grep -rn "set_pixel_tallness\b" crates/
```

If only the band-aid referenced it, remove the method too.

- [ ] **Step 7: Remove Rust-invention `viewport_width` / `viewport_height` / `pixel_tallness` / `visited_vw` / `visited_vh` fields**

Grep and rewrite remaining readers — by Phase 6 they should all be computable from `HomeWidth`/`HomeHeight`/`HomePixelTallness` and the SVP's `viewed_width/height`. The constructor `new(root, viewport_width, viewport_height)` keeps its signature (or add `pixel_tallness: f64` as a new 4th arg — pick per caller impact; document with DIVERGED if signature diverges from C++ which has no equivalent constructor). Update the `viewport_size()` accessor to return `(HomeWidth, HomeHeight)`.

```bash
grep -rn "viewport_width\|viewport_height\|visited_vw\|visited_vh\|\.pixel_tallness\b" crates/
```

Each hit gets rewritten.

- [ ] **Step 8: Update every caller of `SetGeometry` to pass `(x, y, w, h, pt)`**

From Step 1 list, update each one. Common case: callers in windowing code have a width+height and will pass `x=0, y=0, pt=1.0`.

- [ ] **Step 9: Run the suite + runtime smoke**

```bash
cargo-nextest ntr
cargo run --bin eaglemode --release &
EM_PID=$!
sleep 15
kill -0 $EM_PID 2>/dev/null && echo ALIVE && kill $EM_PID || (echo DIED; exit 1)
```

Expected: full suite passes; runtime smoke stays ALIVE ≥15 s.

- [ ] **Step 10: Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(emView): Phase 6 — rewrite geometry API against Home/Current split

- SetGeometry now takes explicit (x, y, width, height, pixelTallness)
  matching C++ emView.cpp:1238. All callers updated.
- SetViewPortTallness added (C++ emViewPort::SetViewGeometry pt path).
- GetHomeTallness helper added (C++ emView.h:874-876).
- Removes the tree.set_pixel_tallness(1.0) band-aid from Update —
  pt is now a first-class Home/Current property set at geometry change.
- Removes Rust-invention fields viewport_width, viewport_height,
  visited_vw, visited_vh, pixel_tallness from emView (their callers
  computed from Home*/Current*/SVP now).
- RawZoomOut(tree, forceViewingUpdate) rewritten against Home fields
  per C++ emView.cpp:1811-1825.

Runtime smoke: eaglemode stays ALIVE ≥15 s, no core dump.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

---

## Phase 7: NoticeFlags reconciliation

**Purpose:** Rename Rust `NoticeFlags` bits to match C++ names, delete Rust inventions, renumber bit values to match C++. Mechanical.

**Files:**
- Modify: `crates/emcore/src/emPanel.rs` — `NoticeFlags` definition
- Modify: every consumer (grep list — expected at least the files from spec §"Consumer sites"): `emSubViewPanel.rs`, `emPanelTree.rs`, `emView.rs`, `emVirtualCosmos.rs`, `emMainPanel.rs`, `emMainContentPanel.rs`, `emDirEntryPanel.rs`, `emDirEntryAltPanel.rs`, `emDirPanel.rs`, `emFileLinkPanel.rs`, `emFileManSelInfoPanel.rs`, Kani proofs, golden test fixtures
- Modify: `crates/eaglemode/tests/pipeline/notices.rs`, `crates/eaglemode/tests/unit/panel.rs`

- [ ] **Step 1: Grep the exact consumer list**

```bash
grep -rln "NoticeFlags::VISIBILITY\|NoticeFlags::VIEW_CHANGED\|NoticeFlags::CANVAS_CHANGED\|NoticeFlags::CHILDREN_CHANGED" crates/ tests/
```

Capture the list into a note for the commit message.

- [ ] **Step 2: Rewrite `NoticeFlags` in `emPanel.rs` to C++ names + bit values**

Replace the bitflags definition (file `crates/emcore/src/emPanel.rs` around line 173-202):

```rust
bitflags! {
    /// Port of C++ `emPanel::NoticeFlags` (emPanel.h:542-553). Names and
    /// bit values match C++ one-for-one.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct NoticeFlags: u32 {
        const CHILD_LIST_CHANGED      = 1 << 0;
        const LAYOUT_CHANGED          = 1 << 1;
        const VIEWING_CHANGED         = 1 << 2;
        const ENABLE_CHANGED          = 1 << 3;
        const ACTIVE_CHANGED          = 1 << 4;
        const FOCUS_CHANGED           = 1 << 5;
        const VIEW_FOCUS_CHANGED      = 1 << 6;
        const UPDATE_PRIORITY_CHANGED = 1 << 7;
        const MEMORY_LIMIT_CHANGED    = 1 << 8;
        const SOUGHT_NAME_CHANGED     = 1 << 9;
    }
}
```

The renames applied:
- `VISIBILITY` → `VIEWING_CHANGED`
- `VIEW_CHANGED` → `VIEWING_CHANGED` (merged; Rust invention deleted)
- `CANVAS_CHANGED` → `VIEWING_CHANGED` (merged; C++ folds canvas changes into NF_VIEWING_CHANGED)
- `CHILDREN_CHANGED` → `CHILD_LIST_CHANGED`

- [ ] **Step 3: Rewrite every consumer site**

For each file from Step 1, search-and-replace:
- `NoticeFlags::VISIBILITY` → `NoticeFlags::VIEWING_CHANGED`
- `NoticeFlags::VIEW_CHANGED` → `NoticeFlags::VIEWING_CHANGED`
- `NoticeFlags::CANVAS_CHANGED` → `NoticeFlags::VIEWING_CHANGED`
- `NoticeFlags::CHILDREN_CHANGED` → `NoticeFlags::CHILD_LIST_CHANGED`

Use a single sed or per-file Edit. A site that sets two of the merging bits at once collapses to a single `VIEWING_CHANGED`.

- [ ] **Step 4: Rewrite Kani proof**

```bash
grep -n "NoticeFlags\|proofs" crates/emcore/src/proofs.rs | head -20
```

Update the proof at around `proofs.rs:263` to reference the C++-matching bit values.

- [ ] **Step 5: Run the suite**

```bash
cargo-nextest ntr
```

Expected: no behavioral change; all tests pass.

- [ ] **Step 6: Verify final grep is clean**

```bash
grep -rn "NoticeFlags::VIEW_CHANGED\|NoticeFlags::VISIBILITY\|NoticeFlags::CANVAS_CHANGED\|NoticeFlags::CHILDREN_CHANGED" crates/ tests/ | grep -v target/
```

Expected: zero matches.

- [ ] **Step 7: Commit**

```bash
git commit -am "$(cat <<'EOF'
feat(emPanel): Phase 7 — NoticeFlags reconciliation with C++ emPanel.h

Renames Rust NoticeFlags bits to match C++ emPanel.h:542-553 names and
bit values one-for-one:

- VISIBILITY → VIEWING_CHANGED
- VIEW_CHANGED (Rust invention) → merged into VIEWING_CHANGED
- CANVAS_CHANGED (Rust invention) → merged into VIEWING_CHANGED
  (C++ folds canvas-color changes into NF_VIEWING_CHANGED)
- CHILDREN_CHANGED → CHILD_LIST_CHANGED

Bit values renumbered to match C++: CHILD_LIST_CHANGED=1<<0,
LAYOUT_CHANGED=1<<1, VIEWING_CHANGED=1<<2, ENABLE_CHANGED=1<<3,
ACTIVE_CHANGED=1<<4, FOCUS_CHANGED=1<<5, VIEW_FOCUS_CHANGED=1<<6,
UPDATE_PRIORITY_CHANGED=1<<7, MEMORY_LIMIT_CHANGED=1<<8,
SOUGHT_NAME_CHANGED=1<<9.

Rewrites consumer sites (N files — enumerate from git diff), Kani
proof, and all golden/pipeline/behavioral fixtures.

No behavioral change expected — notices fire on identical state
transitions; only the flag names and bit layout differ.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
```

---

## Phase 8: Final reconciliation and cleanup

**Purpose:** Delete any Rust-invention field/method without remaining readers, close any `DIVERGED:` comments that can now be closed, audit spec-cited line refs for staleness, run the final acceptance grep suite.

**Files:**
- Modify: `crates/emcore/src/emView.rs` and peers — remove dead code, close DIVERGED comments

- [ ] **Step 1: Remove any remaining Rust-invention field with zero non-test readers**

```bash
grep -rn "viewport_changed\|viewing_dirty\|force_viewing_update\|visited_vw\|visited_vh\|viewport_width\|viewport_height" crates/ | grep -v "#\[cfg(test)\]"
```

Any match outside `#[cfg(test)]` whose only role was to shadow a C++ field that now exists → delete.

- [ ] **Step 2: Close `DIVERGED:` comments that no longer apply**

```bash
grep -rn "DIVERGED:" crates/emcore/src/emView.rs crates/emcore/src/emViewPort.rs
```

For each, ask: "is this divergence still real?" — if the later phases restored the C++ shape, the comment is stale; remove it. If still divergent, leave it.

- [ ] **Step 3: Close `NOT PORTED` / `PHASE-N-TODO` comments**

```bash
grep -rn "NOT PORTED\|PHASE-[0-9]-TODO" crates/
```

Expected: zero matches in the touched subsystem. If any remain, the plan's acceptance criteria ("No NOT PORTED comments remain in the touched methods") is violated — fix now.

- [ ] **Step 4: Audit spec-cited emView.rs line refs**

The spec references `emView.rs:1181` for the pt band-aid, `emView.rs:1306-1311` for the per-frame active-path clear. After Phases 3+6 these lines no longer exist. Run:

```bash
grep -rn "emView.rs:[0-9]" docs/superpowers/specs/ docs/superpowers/plans/
```

For each citation, either update to the post-rewrite line number or mark the reference as obsolete-by-design in a Phase 8 note (the band-aid and per-frame-clear were the removals, so their references are correctly obsolete).

- [ ] **Step 5: Final acceptance grep suite**

```bash
grep -rn 'viewport_width\|viewport_height\|visited_vw\|visited_vh\|viewing_dirty' crates/ | grep -v "#\[cfg(test)\]" | grep -v docs
grep -rn 'NoticeFlags::VIEW_CHANGED\|NoticeFlags::VISIBILITY\|NoticeFlags::CANVAS_CHANGED\|NoticeFlags::CHILDREN_CHANGED' crates/
grep -n 'tree.set_pixel_tallness(1.0)' crates/emcore/src/emView.rs
```

Expected: zero matches in each command.

- [ ] **Step 6: Enumerate C++ emView methods and confirm every missing-scope entry is now present**

Per spec §Still missing. Each of: `RawVisitAbs`, `FindBestSVP`, `FindBestSVPInTree`, `RawZoomOut(forceViewingUpdate)`, `RawVisit(..., forceViewingUpdate)`, `SwapViewPorts`, `GetMaxPopupViewRect`, `RecurseInput`, `AddToNoticeList`, `InvalidateHighlight`, `SetViewPortTallness` must appear in `emView.rs`:

```bash
for name in RawVisitAbs FindBestSVP FindBestSVPInTree SwapViewPorts \
            GetMaxPopupViewRect RecurseInput AddToNoticeList \
            InvalidateHighlight SetViewPortTallness; do
    count=$(grep -c "fn $name\b" crates/emcore/src/emView.rs)
    echo "$name: $count"
done
```

Expected: every name has count ≥1.

- [ ] **Step 7: Full acceptance test run**

```bash
cargo-nextest ntr
```

Expected: ≥235 golden passing / ≤8 failing (same baseline); emcore lib ≥819 / 0; pipeline 0; behavioral 0.

- [ ] **Step 8: Runtime smoke (final)**

```bash
cargo run --bin eaglemode --release &
EM_PID=$!
sleep 15
kill -0 $EM_PID 2>/dev/null && echo ALIVE && kill $EM_PID || (echo DIED; exit 1)
```

Expected: ALIVE. No core dump.

- [ ] **Step 9: Commit (may be a zero-diff sanity commit)**

```bash
# Acceptable if nothing changed — skip the commit.
if git diff --quiet && git diff --cached --quiet; then
    echo "Phase 8 reached with nothing to clean up — done."
else
    git commit -am "$(cat <<'EOF'
chore(emView): Phase 8 — final cleanup + acceptance gate

- Removes Rust-invention fields/methods with zero remaining non-test
  readers (enumerate from git diff).
- Closes DIVERGED/NOT PORTED/PHASE-N-TODO comments that later phases
  made obsolete.
- Audits and updates spec-cited emView.rs line refs.

Final acceptance grep suite clean:
- No viewport_width/viewport_height/visited_vw/visited_vh/viewing_dirty
  outside #[cfg(test)].
- No NoticeFlags::VIEW_CHANGED/VISIBILITY/CANVAS_CHANGED/CHILDREN_CHANGED.
- No tree.set_pixel_tallness(1.0) in emView.rs.
- Every C++ emView method from spec "Still missing" table present.

Runtime smoke: eaglemode stays ALIVE ≥15 s, no core dump.

Refs: docs/superpowers/specs/2026-04-17-emview-viewing-subsystem-design.md
EOF
)"
fi
```

---

## In-Phase Discovery Rule

Per the user's "scope up on missing" feedback: if a phase reveals an additional missing C++ symbol not captured in the spec's Scope Verification tables, **port it in that same phase's commit, not a follow-up.** Enumerate any in-phase additions in the phase commit message body.

## Acceptance Criteria Recap

Per-phase (every commit):
- Pre-commit hook passes without `--no-verify`.
- Golden: ≥235 passing / ≤8 failing (same baseline set).
- emcore lib: ≥819 passing / 0 failing.
- pipeline: 0 failing.
- behavioral: 0 failing.

Runtime smoke (Phases 3, 6, 8 at minimum):
- `eaglemode` launches and stays ALIVE ≥15 s.
- No core dump.

Final (Phase 8):
- All per-phase criteria, plus the Step 5 + Step 6 greps both clean.
- No `NOT PORTED` comments in touched methods.
